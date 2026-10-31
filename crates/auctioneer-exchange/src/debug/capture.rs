//! Capacity-bounded sanitized capture, separate from permission and bid validity.

use super::dto::{DebugHeaders, DebugHttpCall, DebugInfo, DebugRequest, DebugResponse};
use super::metadata::{DebugError, DebugMethod, elapsed_ms, endpoint_origin, sanitized_header};
use crate::BidderCallResult;
use auctioneer_bidder::{
    BidderError, BidderHttpRequest, DebugBodyProjection, DebugBodySchema, DebugBodyStatus,
    project_request_body,
};
use auctioneer_config::types::MAX_BIDDER_NAME_BYTES;
use auctioneer_runtime::debug_budget::{
    MAX_DEBUG_CALL_ROWS, MAX_DEBUG_RETAINED_BYTES, MAX_DEBUG_SERIALIZED_BYTES,
};
use auctioneer_runtime::{Clock, MonotonicInstant, serialized_json_bytes};
use serde_json::Value;
use std::collections::BTreeMap;
use std::mem::size_of;

// At most three static keys occupy a BTreeMap leaf. Charge a full 1 KiB
// per key for its inline key/value arrays and edges, not just visible strings.
const HEADER_NODE_BYTES: u64 = 1024;

struct CaptureLimits {
    rows: u32,
    retained: u64,
    serialized: u64,
}

pub struct TentativeRequest {
    request: DebugRequest,
}

struct Row {
    bidder: String,
    call: DebugHttpCall,
}

pub struct Capture {
    rows: Vec<Row>,
    limits: CaptureLimits,
    dropped: u32,
    truncated: bool,
    exhausted: bool,
    incomplete: bool,
}

impl Capture {
    #[must_use]
    pub fn new(max_outbound_requests: u32) -> Self {
        Self::with_limits(CaptureLimits {
            rows: max_outbound_requests.min(MAX_DEBUG_CALL_ROWS),
            retained: MAX_DEBUG_RETAINED_BYTES,
            serialized: MAX_DEBUG_SERIALIZED_BYTES,
        })
    }

    fn with_limits(limits: CaptureLimits) -> Self {
        Self {
            rows: Vec::new(),
            limits,
            dropped: 0,
            truncated: false,
            exhausted: false,
            incomplete: false,
        }
    }

    /// Call only for a debug-permitted call, before compression. A discarded
    /// tentative value has no warning/counter effect and owns no raw bytes.
    pub fn prepare_request(
        &mut self,
        request: &BidderHttpRequest,
        schema: DebugBodySchema,
        max_nodes: u32,
        clock: &dyn Clock,
        deadline: MonotonicInstant,
    ) -> Option<TentativeRequest> {
        if !self.available(clock, deadline)
            || self.exhausted
            || self.rows.len() >= usize::try_from(self.limits.rows).ok()?
        {
            return None;
        }
        let origin = endpoint_origin(request.url())?;
        if !self.available(clock, deadline) {
            return None;
        }
        let projection = compact_body(project_request_body(
            schema,
            request.body(),
            request.headers(),
            max_nodes,
        ));
        if !self.available(clock, deadline) {
            return None;
        }
        Some(TentativeRequest {
            request: DebugRequest {
                method: DebugMethod::from_token(request.method()),
                endpoint_origin: origin,
                headers: BTreeMap::new(),
                body_status: projection.body_status,
                body: projection.body,
            },
        })
    }

    /// Commit only after successful transport finalization, using final headers.
    /// None means this permitted finalized row is omitted, not that dispatch is denied.
    pub fn commit_request<'a, H>(
        &mut self,
        call_index: usize,
        bidder: &str,
        incoming: Option<TentativeRequest>,
        headers: H,
        clock: &dyn Clock,
        deadline: MonotonicInstant,
    ) -> Option<usize>
    where
        H: IntoIterator<Item = (&'a str, &'a str)>,
    {
        let Some(mut tentative) = incoming else {
            self.drop_call();
            return None;
        };
        if !self.available(clock, deadline)
            || self.exhausted
            || self.rows.len() >= usize::try_from(self.limits.rows).ok()?
        {
            self.drop_call();
            return None;
        }
        let Ok(index) = u32::try_from(call_index) else {
            self.incomplete = true;
            self.drop_call();
            return None;
        };
        if self
            .rows
            .last()
            .is_some_and(|row| row.call.call_index >= index)
            || bidder.len() > MAX_BIDDER_NAME_BYTES
        {
            self.incomplete = true;
            self.drop_call();
            return None;
        }
        if !self.reserve_rows() {
            self.exhausted = true;
            self.drop_call();
            return None;
        }
        let prospective = self
            .retained_bytes()?
            .checked_add(u64::try_from(bidder.len()).ok()?)?
            .checked_add(request_bytes(&tentative.request)?)?;
        if prospective > self.limits.retained {
            self.exhausted = true;
            self.drop_call();
            return None;
        }
        let Some(final_headers) =
            project_headers(headers, self.limits.retained.saturating_sub(prospective))
        else {
            self.exhausted = true;
            self.drop_call();
            return None;
        };
        if !self.available(clock, deadline) {
            self.drop_call();
            return None;
        }
        tentative.request.headers = final_headers;
        let mut name = String::new();
        if name.try_reserve_exact(bidder.len()).is_err() {
            self.drop_call();
            return None;
        }
        name.push_str(bidder);
        self.truncated |= omitted_body(tentative.request.body_status);
        self.rows.push(Row {
            bidder: name,
            call: DebugHttpCall {
                call_index: index,
                request: tentative.request,
                response: None,
                elapsed_ms: 0,
                error: None,
            },
        });
        self.enforce_retained();
        self.rows
            .last()
            .filter(|row| row.call.call_index == index)?;
        self.rows.len().checked_sub(1)
    }

    pub fn complete(
        &mut self,
        association: Option<usize>,
        result: &BidderCallResult,
        schema: DebugBodySchema,
        clock: &dyn Clock,
        deadline: MonotonicInstant,
    ) {
        let Some(index) = association else {
            return;
        };
        if self.rows.get(index).is_none() || !self.available(clock, deadline) {
            return;
        }
        let (captured_response, error) = match &result.result {
            Ok(response) => {
                let headers = project_headers(
                    response
                        .headers()
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.as_str())),
                    self.limits.retained,
                );
                if !self.available(clock, deadline) {
                    return;
                }
                let projection = compact_body(response.project_debug_body(schema));
                if !self.available(clock, deadline) {
                    return;
                }
                let (safe_headers, safe_projection) = match headers {
                    Some(accepted_headers) => (accepted_headers, projection),
                    None if projection.body_status == DebugBodyStatus::Empty => {
                        (BTreeMap::new(), projection)
                    }
                    None => (
                        BTreeMap::new(),
                        DebugBodyProjection {
                            body_status: DebugBodyStatus::Unavailable,
                            body: None,
                        },
                    ),
                };
                self.truncated |= omitted_body(safe_projection.body_status);
                (
                    Some(DebugResponse {
                        status: response.status(),
                        headers: safe_headers,
                        body_status: safe_projection.body_status,
                        body: safe_projection.body,
                    }),
                    None,
                )
            }
            Err(error) => (None, Some(DebugError::from_transport(error))),
        };
        if let Some(row) = self.rows.get_mut(index) {
            row.call.elapsed_ms = elapsed_ms(result.elapsed);
            row.call.response = captured_response;
            row.call.error = error;
        }
        self.enforce_retained();
    }

    pub fn adapter_errors(&mut self, association: Option<usize>, errors: &[BidderError]) {
        if let Some(row) = association.and_then(|index| self.rows.get_mut(index))
            && let Some(category) = DebugError::from_adapter_errors(errors)
        {
            row.call.error = Some(category);
        }
    }

    /// Exact bounded fitting is independent of lexical group order. There are
    /// at most three removals per row; no post-deadline trimming is attempted.
    #[must_use]
    pub fn finish(self, clock: &dyn Clock, deadline: MonotonicInstant) -> Option<Value> {
        if self.incomplete || clock.now_monotonic() >= deadline {
            return None;
        }
        let mut info = DebugInfo {
            httpcalls: BTreeMap::new(),
            truncated: self.truncated,
            dropped_calls: self.dropped,
        };
        for row in self.rows {
            if clock.now_monotonic() >= deadline {
                return None;
            }
            let group = info.httpcalls.entry(row.bidder).or_default();
            group.try_reserve_exact(1).ok()?;
            group.push(row.call);
        }
        for _ in 0..=MAX_DEBUG_CALL_ROWS.saturating_mul(3) {
            if clock.now_monotonic() >= deadline {
                return None;
            }
            if serialized_json_bytes(&info).ok()? <= self.limits.serialized {
                let value = serde_json::to_value(info).ok()?;
                return (clock.now_monotonic() < deadline).then_some(value);
            }
            if clock.now_monotonic() >= deadline || !trim_highest(&mut info) {
                return None;
            }
        }
        None
    }

    fn available(&mut self, clock: &dyn Clock, deadline: MonotonicInstant) -> bool {
        if clock.now_monotonic() >= deadline {
            self.incomplete = true;
        }
        !self.incomplete
    }

    fn drop_call(&mut self) {
        self.truncated = true;
        if let Some(dropped) = self.dropped.checked_add(1) {
            self.dropped = dropped;
        } else {
            self.incomplete = true;
        }
    }

    fn reserve_rows(&mut self) -> bool {
        if self.rows.capacity() > 0 {
            return true;
        }
        let Ok(rows) = usize::try_from(self.limits.rows) else {
            return false;
        };
        let Some(bytes) = rows
            .checked_mul(size_of::<Row>())
            .and_then(|bytes| u64::try_from(bytes).ok())
        else {
            return false;
        };
        bytes <= self.limits.retained && self.rows.try_reserve_exact(rows).is_ok()
    }

    fn retained_bytes(&self) -> Option<u64> {
        let slots = u64::try_from(self.rows.capacity().checked_mul(size_of::<Row>())?).ok()?;
        self.rows.iter().try_fold(slots, |total, row| {
            total
                .checked_add(u64::try_from(row.bidder.capacity()).ok()?)?
                .checked_add(request_bytes(&row.call.request)?)?
                .checked_add(row.call.response.as_ref().map_or(Some(0), response_bytes)?)
        })
    }

    fn enforce_retained(&mut self) {
        while self
            .retained_bytes()
            .is_none_or(|bytes| bytes > self.limits.retained)
        {
            if self.rows.pop().is_none() {
                self.incomplete = true;
                break;
            }
            self.exhausted = true;
            self.drop_call();
        }
    }
}

fn compact_body(mut projection: DebugBodyProjection) -> DebugBodyProjection {
    if let Some(body) = projection.body.take() {
        let mut compact = String::new();
        if compact.try_reserve_exact(body.len()).is_err() {
            projection.body_status = DebugBodyStatus::Unavailable;
        } else {
            compact.push_str(&body);
            projection.body = Some(compact);
        }
    }
    projection
}

fn omitted_body(status: DebugBodyStatus) -> bool {
    match status {
        DebugBodyStatus::Captured | DebugBodyStatus::Empty => false,
        DebugBodyStatus::Unavailable
        | DebugBodyStatus::OmittedType
        | DebugBodyStatus::OmittedSize
        | DebugBodyStatus::OmittedStructure
        | DebugBodyStatus::OmittedPolicy => true,
    }
}

fn header_bytes(headers: &DebugHeaders) -> Option<u64> {
    headers.values().try_fold(0_u64, |total, values| {
        total
            .checked_add(HEADER_NODE_BYTES)?
            .checked_add(u64::try_from(values.capacity().checked_mul(size_of::<&str>())?).ok()?)
    })
}

fn body_bytes(input: Option<&String>) -> Option<u64> {
    input.map_or(Some(0), |body| u64::try_from(body.capacity()).ok())
}
fn request_bytes(request: &DebugRequest) -> Option<u64> {
    u64::try_from(request.endpoint_origin.capacity())
        .ok()?
        .checked_add(header_bytes(&request.headers)?)?
        .checked_add(body_bytes(request.body.as_ref())?)
}
fn response_bytes(response: &DebugResponse) -> Option<u64> {
    header_bytes(&response.headers)?.checked_add(body_bytes(response.body.as_ref())?)
}

fn project_headers<'a, H>(headers: H, limit: u64) -> Option<DebugHeaders>
where
    H: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut safe = DebugHeaders::new();
    for (name, value) in headers {
        let Some((safe_name, safe_value)) = sanitized_header(name, value) else {
            continue;
        };
        let mut proposed = header_bytes(&safe)?;
        if !safe.contains_key(safe_name) {
            proposed = proposed.checked_add(HEADER_NODE_BYTES)?;
        }
        if proposed.checked_add(u64::try_from(size_of::<&str>()).ok()?)? > limit {
            return None;
        }
        let values = safe.entry(safe_name).or_default();
        values.try_reserve_exact(1).ok()?;
        values.push(safe_value);
        if header_bytes(&safe)? > limit {
            return None;
        }
    }
    Some(safe)
}

fn trim_highest(info: &mut DebugInfo) -> bool {
    let Some(index) = info
        .httpcalls
        .values()
        .filter_map(|rows| rows.last())
        .map(|row| row.call_index)
        .max()
    else {
        return false;
    };
    for rows in info.httpcalls.values_mut() {
        let Some(last) = rows.last_mut().filter(|row| row.call_index == index) else {
            continue;
        };
        info.truncated = true;
        if let Some(response) = last.response.as_mut()
            && response.body.take().is_some()
        {
            response.body_status = DebugBodyStatus::OmittedSize;
            return true;
        }
        if last.request.body.take().is_some() {
            last.request.body_status = DebugBodyStatus::OmittedSize;
            return true;
        }
        rows.pop();
        let Some(dropped) = info.dropped_calls.checked_add(1) else {
            return false;
        };
        info.dropped_calls = dropped;
        info.httpcalls.retain(|_, group| !group.is_empty());
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{Capture, CaptureLimits, TentativeRequest};
    use crate::ExchangeEndpointValidator;
    use crate::debug::dto::{DebugHeaders, DebugRequest};
    use crate::debug::metadata::DebugMethod;
    use crate::{BidderCallResult, FanoutError, TimeoutCause};
    use auctioneer_bidder::{
        BidderError, BidderErrorKind, BidderHttpRequest, BidderHttpResponse, BidderName,
        BidderRequestInfo, CallBudget, Compression, DebugBodyEligibility, DebugBodySchema,
        DebugBodyStatus, EntryPoint, FinalizeError, OutboundRequestSink, SinkLimits,
        TransportFinalize, finalize_for_transport,
    };
    use auctioneer_config::types::BidderInfo;
    use auctioneer_runtime::{Clock, FakeClock, MonotonicInstant};
    use serde_json::{Value, json};
    use std::iter;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    fn clock() -> FakeClock {
        FakeClock::new(time::OffsetDateTime::UNIX_EPOCH)
    }

    fn deadline(clock: &FakeClock) -> MonotonicInstant {
        clock.origin().checked_add(Duration::from_secs(10)).unwrap()
    }

    fn outbound_request() -> BidderHttpRequest {
        let endpoint = "https://bidder.example/private?uid=SECRET_URL";
        let bidder = BidderInfo {
            endpoint: endpoint.into(),
            ..BidderInfo::default()
        };
        let info = BidderRequestInfo::new(
            BidderName::new("fake_fx"),
            BidderName::new("fake_fx"),
            "account",
            None,
            endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let budget = CallBudget::new(1);
        let validator = ExchangeEndpointValidator::new(&bidder);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &validator);
        let body = serde_json::to_vec(
            &json!({"id":"SECRET_ID", "test":1_i32,"user":{"id":"SECRET_USER"}}),
        )
        .unwrap();
        sink.push_request(
            &info,
            |meta| {
                meta.set_method("POST")?;
                meta.set_url(endpoint)?;
                meta.header("Content-Type", "application/json; token=SECRET_HEADER")
            },
            |writer| writer.write_all(&body),
        )
        .unwrap();
        sink.into_outcome().accepted.pop().unwrap()
    }

    #[test]
    fn request_capture_precedes_compression_but_commits_only_final_headers() {
        let clock = clock();
        let mut capture = Capture::new(1);
        let request = outbound_request();
        let pending = capture.prepare_request(
            &request,
            DebugBodySchema::OpenRtb,
            4096,
            &clock,
            deadline(&clock),
        );
        let (sent, finalized) = finalize_for_transport(
            request,
            &TransportFinalize {
                gpc: Some("1"),
                compression: Some(Compression::Gzip),
                max_encoded_bytes: 4096,
            },
        );
        assert!(finalized.unwrap().body().starts_with(&[0x1f, 0x8b]));
        capture.commit_request(
            0,
            "fake_",
            pending,
            sent.headers(),
            &clock,
            deadline(&clock),
        );
        let result = finish(capture, &clock);
        let projected = &result["httpcalls"]["fake_"][0]["request"];
        assert_eq!(
            projected["headers"],
            json!({"content-type":["application/json"],"content-encoding":["gzip"],"sec-gpc":["1"]})
        );
        assert_eq!(
            serde_json::from_str::<Value>(projected["body"].as_str().unwrap()).unwrap(),
            json!({"id":"[REDACTED]","test":1_i32,"user":"[REDACTED]"})
        );
        assert!(!result.to_string().contains("SECRET"));
    }

    #[test]
    fn real_finalization_failure_rolls_back_without_call_or_omission_count() {
        let clock = clock();
        let mut capture = Capture::new(1);
        let request = outbound_request();
        let pending = capture.prepare_request(
            &request,
            DebugBodySchema::OpenRtb,
            4096,
            &clock,
            deadline(&clock),
        );
        assert!(pending.is_some());
        let (_, result) = finalize_for_transport(
            request,
            &TransportFinalize {
                gpc: None,
                compression: None,
                max_encoded_bytes: 0,
            },
        );
        assert_eq!(
            result.unwrap_err(),
            FinalizeError::OversizeEncoded { limit: 0 }
        );
        drop(pending);
        assert_eq!(
            finish(capture, &clock),
            json!({"httpcalls":{},"truncated":false,"dropped_calls":0_i32})
        );
    }

    #[test]
    fn response_completion_order_cannot_change_the_retained_prefix() {
        let clock = clock();
        let mut expected = None;
        for reverse in [false, true] {
            let mut capture = Capture::new(2);
            let first = commit(&mut capture, 0, "fake_z", None, &clock);
            let last = commit(&mut capture, 1, "fake_a", None, &clock);
            let outcome = response(None);
            let initial = capture.retained_bytes().unwrap();
            capture.complete(
                first,
                &outcome,
                DebugBodySchema::OpenRtb,
                &clock,
                deadline(&clock),
            );
            let one_response = capture.retained_bytes().unwrap();
            assert!(one_response > initial);
            capture.rows[0].call.response = None;
            capture.rows[0].call.elapsed_ms = 0;
            capture.limits.retained = one_response;
            for slot in if reverse {
                [last, first]
            } else {
                [first, last]
            } {
                capture.complete(
                    slot,
                    &outcome,
                    DebugBodySchema::OpenRtb,
                    &clock,
                    deadline(&clock),
                );
            }
            let actual = finish(capture, &clock);
            assert!(actual["httpcalls"].get("fake_a").is_none());
            assert_eq!(
                actual["httpcalls"]["fake_z"][0]["response"]["body_status"],
                "empty"
            );
            assert_eq!(actual["dropped_calls"], 1_i32);
            if let Some(previous) = &expected {
                assert_eq!(&actual, previous);
            }
            expected = Some(actual);
        }
    }

    fn tentative(body: Option<String>) -> TentativeRequest {
        TentativeRequest {
            request: DebugRequest {
                method: DebugMethod::Post,
                endpoint_origin: "https://bidder.example".into(),
                headers: DebugHeaders::new(),
                body_status: if body.is_some() {
                    DebugBodyStatus::Captured
                } else {
                    DebugBodyStatus::Empty
                },
                body,
            },
        }
    }

    fn commit(
        capture: &mut Capture,
        index: usize,
        bidder: &str,
        body: Option<String>,
        clock: &FakeClock,
    ) -> Option<usize> {
        capture.commit_request(
            index,
            bidder,
            Some(tentative(body)),
            iter::empty(),
            clock,
            deadline(clock),
        )
    }

    fn finish(capture: Capture, clock: &FakeClock) -> Value {
        capture.finish(clock, deadline(clock)).unwrap()
    }

    #[test]
    fn an_unused_collector_is_successfully_empty_without_allocated_rows() {
        let clock = clock();
        let capture = Capture::new(64);
        assert_eq!(capture.rows.capacity(), 0);
        assert_eq!(
            finish(capture, &clock),
            json!({"httpcalls":{},"truncated":false,"dropped_calls":0_i32})
        );
    }

    #[test]
    fn finalized_calls_have_original_indices_and_later_calls_are_counted_once() {
        let clock = clock();
        for cap in [0_u32, 1, 63, 64, 65, 100] {
            let mut capture = Capture::new(cap);
            for index in 0..66_usize {
                commit(&mut capture, index, "fake_", None, &clock);
            }
            let result = finish(capture, &clock);
            let kept = usize::try_from(cap.min(64)).unwrap();
            assert_eq!(
                result["httpcalls"]["fake_"].as_array().map_or(0, Vec::len),
                kept
            );
            assert_eq!(result["dropped_calls"], 66_usize - kept);
            assert_eq!(result["truncated"], kept < 66);
        }
    }

    #[test]
    fn refused_tentative_work_only_counts_if_transport_finalization_succeeds() {
        let clock = clock();
        let mut capture = Capture::new(10);
        drop(tentative(Some("safe".into())));
        assert_eq!(capture.rows.len(), 0);
        capture.commit_request(3, "fake_", None, iter::empty(), &clock, deadline(&clock));
        let result = finish(capture, &clock);
        assert_eq!(
            result,
            json!({"httpcalls":{},"truncated":true,"dropped_calls":1_i32})
        );
    }

    #[test]
    fn capacity_accounts_for_string_capacity_not_just_visible_length() {
        let clock = clock();
        let mut capture = Capture::with_limits(CaptureLimits {
            rows: 1,
            retained: 1024,
            serialized: 64 * 1024,
        });
        let mut body = String::with_capacity(2048);
        body.push('x');
        assert_eq!(commit(&mut capture, 0, "fake_", Some(body), &clock), None);
        assert_eq!(finish(capture, &clock)["dropped_calls"], 1_i32);
    }

    #[test]
    fn projection_or_allocation_refusal_does_not_masquerade_as_empty_success() {
        let clock = clock();
        let mut capture = Capture::with_limits(CaptureLimits {
            rows: 1,
            retained: 0,
            serialized: 64 * 1024,
        });
        assert_eq!(commit(&mut capture, 0, "fake_", None, &clock), None);
        assert_eq!(
            finish(capture, &clock),
            json!({"httpcalls":{},"truncated":true,"dropped_calls":1_i32})
        );
    }

    fn response(body: Option<String>) -> BidderCallResult {
        let bytes = body.map_or_else(Vec::new, String::into_bytes);
        BidderCallResult {
            result: Ok(BidderHttpResponse::from_transport(
                200,
                vec![("content-type".into(), "application/json".into())],
                bytes,
                DebugBodyEligibility::SingleTextValue,
            )
            .within_node_budget(4096)),
            elapsed: Duration::from_micros(17_345),
        }
    }

    #[test]
    fn known_empty_response_survives_header_capture_refusal_without_truncation() {
        let clock = clock();
        let mut capture = Capture::with_limits(CaptureLimits {
            rows: 1,
            retained: 1024,
            serialized: 64 * 1024,
        });
        let row = commit(&mut capture, 0, "fake_", None, &clock);
        assert_eq!(row, Some(0));
        let outcome = response(None);
        capture.complete(
            row,
            &outcome,
            DebugBodySchema::OpenRtb,
            &clock,
            deadline(&clock),
        );
        let actual = finish(capture, &clock);
        assert_eq!(
            actual["httpcalls"]["fake_"][0]["response"],
            json!({"status": 200_i32, "headers": {}, "body_status": "empty"})
        );
        assert_eq!(actual["truncated"], false);
        assert_eq!(actual["dropped_calls"], 0_i32);
    }

    #[test]
    fn per_call_elapsed_and_transport_absence_are_not_aggregate_or_empty_bodies() {
        let clock = clock();
        let mut capture = Capture::new(4);
        let first = commit(&mut capture, 0, "fake_z", None, &clock);
        let second = commit(&mut capture, 2, "fake_a", None, &clock);
        for association in [second, first] {
            capture.complete(
                association,
                &BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: Duration::from_micros(17_345),
                },
                DebugBodySchema::OpenRtb,
                &clock,
                deadline(&clock),
            );
        }
        let result = finish(capture, &clock);
        for name in ["fake_a", "fake_z"] {
            assert_eq!(result["httpcalls"][name][0]["elapsed_ms"], 17_i32);
            assert_eq!(result["httpcalls"][name][0]["error"], "timeout");
            assert!(result["httpcalls"][name][0].get("response").is_none());
        }
        assert_eq!(result["truncated"], false);
    }

    #[test]
    fn response_body_is_projected_and_parse_provenance_is_separate() {
        let clock = clock();
        let mut capture = Capture::new(1);
        let row = commit(&mut capture, 0, "fake_", None, &clock);
        capture.complete(
            row,
            &response(Some(
                serde_json::to_string(&json!({"id":"SECRET", "cur":"USD", "secret-key":"SECRET"}))
                    .unwrap(),
            )),
            DebugBodySchema::OpenRtb,
            &clock,
            deadline(&clock),
        );
        capture.adapter_errors(
            row,
            &[BidderError {
                code: 999,
                message: "SECRET".into(),
                kind: BidderErrorKind::ResponseParse,
            }],
        );
        let result = finish(capture, &clock);
        let captured_row = &result["httpcalls"]["fake_"][0];
        assert_eq!(captured_row["error"], "response_parse");
        assert_eq!(captured_row["response"]["body_status"], "captured");
        assert_eq!(
            serde_json::from_str::<Value>(captured_row["response"]["body"].as_str().unwrap())
                .unwrap(),
            json!({"id":"[REDACTED]", "cur":"USD"})
        );
        assert!(!result.to_string().contains("SECRET"));
    }

    #[test]
    fn trimming_uses_highest_global_index_response_then_request_then_row() {
        let clock = clock();
        let populated = || {
            let mut capture = Capture::new(2);
            commit(&mut capture, 0, "fake_z", Some("x".repeat(100)), &clock);
            let high = commit(&mut capture, 1, "fake_a", Some("x".repeat(100)), &clock);
            capture.complete(
                high,
                &response(Some(
                    serde_json::to_string(&json!({"id":"private", "cur":"USD"})).unwrap(),
                )),
                DebugBodySchema::OpenRtb,
                &clock,
                deadline(&clock),
            );
            capture
        };
        let mut expected = finish(populated(), &clock);
        for step in 0..3_u32 {
            expected["truncated"] = json!(true);
            match step {
                0 => {
                    assert!(
                        expected["httpcalls"]["fake_a"][0]["response"]
                            .as_object_mut()
                            .unwrap()
                            .remove("body")
                            .is_some()
                    );
                    expected["httpcalls"]["fake_a"][0]["response"]["body_status"] =
                        json!("omitted_size");
                }
                1 => {
                    assert!(
                        expected["httpcalls"]["fake_a"][0]["request"]
                            .as_object_mut()
                            .unwrap()
                            .remove("body")
                            .is_some()
                    );
                    expected["httpcalls"]["fake_a"][0]["request"]["body_status"] =
                        json!("omitted_size");
                }
                _ => {
                    assert!(
                        expected["httpcalls"]
                            .as_object_mut()
                            .unwrap()
                            .remove("fake_a")
                            .is_some()
                    );
                    expected["dropped_calls"] = json!(1_i32);
                }
            }
            let mut capture = populated();
            capture.limits.serialized = u64::try_from(expected.to_string().len()).unwrap();
            assert_eq!(finish(capture, &clock), expected);
        }
    }

    #[test]
    fn serialized_escaping_limit_is_independent_of_retained_storage() {
        let clock = clock();
        let mut capture = Capture::new(8);
        for index in 0..8_usize {
            commit(
                &mut capture,
                index,
                "fake_",
                Some("\0".repeat(8000)),
                &clock,
            );
        }
        let result = finish(capture, &clock);
        assert!(result.to_string().len() <= 64 * 1024);
        assert_eq!(result["truncated"], true);
    }

    #[test]
    fn expired_assembly_is_omitted_wholly_instead_of_running_a_trimming_sweep() {
        let clock = clock();
        let mut capture = Capture::new(1);
        commit(&mut capture, 0, "fake_", None, &clock);
        clock.advance(Duration::from_secs(10));
        assert_eq!(capture.finish(&clock, deadline(&clock)), None);
    }

    struct ExpiringClock {
        before: MonotonicInstant,
        after: MonotonicInstant,
        reads: AtomicUsize,
        expire_at: usize,
    }

    impl Clock for ExpiringClock {
        fn now(&self) -> time::OffsetDateTime {
            time::OffsetDateTime::UNIX_EPOCH
        }
        fn now_monotonic(&self) -> MonotonicInstant {
            if self.reads.fetch_add(1, Ordering::Relaxed) >= self.expire_at {
                self.after
            } else {
                self.before
            }
        }
    }

    #[test]
    fn expiry_during_request_projection_cannot_commit_partial_debug() {
        let source_clock = clock();
        let clock = ExpiringClock {
            before: source_clock.origin(),
            after: deadline(&source_clock),
            reads: AtomicUsize::new(0),
            expire_at: 2,
        };
        let mut capture = Capture::new(1);
        let request = outbound_request();
        assert!(
            capture
                .prepare_request(
                    &request,
                    DebugBodySchema::OpenRtb,
                    4096,
                    &clock,
                    clock.after
                )
                .is_none()
        );
        assert_eq!(clock.reads.load(Ordering::Relaxed), 3);
        assert!(capture.incomplete);
        assert_eq!(capture.finish(&clock, clock.after), None);
    }

    #[test]
    fn exact_fitting_recounts_dropped_call_digit_growth() {
        let clock = clock();
        for refused in [8_usize, 9, 98, 99] {
            let mut capture = Capture::new(1);
            commit(&mut capture, 0, "fake_", Some("safe".repeat(100)), &clock);
            for index in 1..=refused {
                commit(&mut capture, index, "fake_", None, &clock);
            }
            let expected = json!({"httpcalls":{},"truncated":true,"dropped_calls":refused + 1});
            capture.limits.serialized = u64::try_from(expected.to_string().len()).unwrap();
            assert_eq!(finish(capture, &clock), expected);
        }
    }
}

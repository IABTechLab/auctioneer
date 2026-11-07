//! Snapshot fixture: an auction whose budget runs out MID-FLIGHT, while
//! some calls are already in the air.
//!
//! Three bidders over three imps, one outcome each, so the three
//! timeout surfaces are separable rather than co-varying:
//!
//! - `fake_on_time` answers inside its own per-bidder budget with one banner
//!   bid. It is the control: its bid has to survive the auction
//!   unmodified, which proves the timeout handling folds per-slot
//!   results rather than discarding the whole batch.
//! - `fake_bidder_timeout` reports `TimeoutCause::Bidder` — its own per-bidder
//!   deadline elapsed while the auction still had budget.
//! - `fake_auction_deadline` is still unresolved when the whole-auction deadline
//!   passes and reports `TimeoutCause::AuctionDeadline`.
//!
//! These symbolic roles are bound to default-active serving bidders before dispatch;
//! owner parameters and an independent expected-response projection use that map.
//!
//! The fanout advances the shared `FakeClock` past the composed auction
//! deadline before it hands the batch back, so the post-fanout clock
//! re-read in `run_auction` is what raises the auction-wide entry. That
//! is the only admissible evidence for it: a slot's own `TimeoutCause`
//! is provenance about which composed budget was smaller at dispatch,
//! not proof that THIS auction expired, and `mid_fanout_code_12.rs`
//! pins that distinction on its own.
//!
//! # The four surfaces this recorded response binds
//!
//! 1. `ext.errors[<bidder>][0].code == 1` for each timed-out bidder —
//!    both of them, whichever cause they carried. The on-time bidder
//!    has no `ext.errors` entry at all.
//! 2. `ext.errors.prebid` carries code 12 EXACTLY ONCE, however many
//!    bidders timed out. Two timed-out bidders here rather than one, so
//!    a per-bidder fold into the global array would show up as a
//!    duplicate.
//! 3. `ext.prebid.seatnonbid[<bidder>].nonbid[].statuscode == 101` for
//!    each timed-out bidder, and no row for the bidder that answered.
//!    The request fixture sets `ext.prebid.returnallbidstatus` because
//!    the block is suppressed without it.
//! 4. `nbr` is absent. It is a separate taxonomy: only the "no live
//!    bidders" case sets it, and one bidder dispatched and bid here.
//!
//! The clock is a `FakeClock` pinned at the UNIX epoch, so
//! `ext.prebid.auctiontimestamp`, every `responsetimemillis` figure and
//! the composed deadline are all fixed and the recorded answer can be
//! compared whole.

mod support;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use crate::serving_catalog::{ServingBidder, default_active_site_media};
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_config_data::types::MediaType;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_core::test_support::config_store::auction_context_for_route;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, TimeoutCause,
    };
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::mem;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// Response cap wide enough that nothing here trips it.
    const WIDE_CAP: usize = 1 << 20;

    /// The budget the request fixture declares, in the unit the clock
    /// speaks. The fanout spends exactly this much, which lands the
    /// clock ON the deadline — the expiry check has to treat "at the
    /// deadline" as expired, not only "past" it.
    const AUCTION_BUDGET: Duration = Duration::from_secs(1);

    /// The imp each bidder is bound to by the request fixture. The
    /// binding is one imp per bidder, so a `seatnonbid` row names
    /// exactly one impid and the surfaces stay separable.
    const ONTIME_IMP: &str = "imp-ontime";
    const BIDDER_TIMEOUT_IMP: &str = "imp-bidder-timeout";
    const AUCTION_DEADLINE_IMP: &str = "imp-auction-deadline";
    const ONTIME_ROLE: &str = "fake_on_time";
    const BIDDER_TIMEOUT_ROLE: &str = "fake_bidder_timeout";
    const AUCTION_DEADLINE_ROLE: &str = "fake_auction_deadline";

    struct FixtureCase {
        names: BTreeMap<&'static str, &'static str>,
        request: String,
        expected: Value,
    }

    impl FixtureCase {
        fn new() -> Self {
            let mut rows: Vec<_> = default_active_site_media(&[MediaType::Banner])
                .into_iter()
                .filter(|row| {
                    !row.info.disabled && row.info.uses_https && !row.info.endpoint.contains("{{")
                })
                .collect();
            rows.sort_by_key(ServingBidder::name);
            let ontime_index = rows
                .iter()
                .position(|row| {
                    row.owner_expected_request_body.as_ref() == Some(&row.owner_request)
                })
                .expect("mid-flight control requires a default-active banner passthrough");
            let ontime_bidder = rows.remove(ontime_index);
            let mut timed_out = rows.into_iter();
            let roles = BTreeMap::from([
                (ONTIME_ROLE, ontime_bidder),
                (
                    BIDDER_TIMEOUT_ROLE,
                    timed_out
                        .next()
                        .expect("mid-flight coverage requires a distinct bidder-timeout base"),
                ),
                (
                    AUCTION_DEADLINE_ROLE,
                    timed_out
                        .next()
                        .expect("mid-flight coverage requires a third default-active banner base"),
                ),
            ]);
            let mut request: Value =
                serde_json::from_str(&fixture("mid_flight_timeout.request.json"))
                    .expect("symbolic request JSON");
            let mut used = BTreeSet::new();
            for imp in request["imp"].as_array_mut().expect("fixture impressions") {
                let bidders = imp["ext"]["prebid"]["bidder"]
                    .as_object_mut()
                    .expect("symbolic bidder parameters");
                for (role, placeholder) in mem::take(bidders) {
                    assert_eq!(placeholder, json!({}), "params come from the bidder owner");
                    assert!(used.insert(role.clone()), "one impression per role");
                    let row = roles.get(role.as_str()).expect("declared fixture role");
                    bidders.insert(row.name().to_owned(), row.params.clone());
                }
            }
            assert_eq!(
                used,
                roles
                    .keys()
                    .map(|role| (*role).to_owned())
                    .collect::<BTreeSet<_>>()
            );
            let names = roles
                .iter()
                .map(|(role, row)| (*role, row.name()))
                .collect();
            let template: Value =
                serde_json::from_str(&fixture("mid_flight_timeout.response.json"))
                    .expect("symbolic response JSON");
            let mut expected = project_expected(&template, &names);
            expected["ext"]["prebid"]["seatnonbid"]
                .as_array_mut()
                .expect("recorded non-bid rows")
                .sort_by(|left, right| left["seat"].as_str().cmp(&right["seat"].as_str()));
            Self {
                names,
                request: serde_json::to_string(&request).expect("projected request JSON"),
                expected,
            }
        }

        fn name(&self, role: &str) -> &'static str {
            self.names[role]
        }
    }

    // Only authored role tokens and targeting suffixes are projected. All prices,
    // errors, timings and other expected fields remain in the recorded fixture.
    fn project_expected(source_value: &Value, names: &BTreeMap<&str, &str>) -> Value {
        match source_value {
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(source_key, field_value)| {
                        let projected_key =
                            if let Some(mapped_name) = names.get(source_key.as_str()) {
                                (*mapped_name).to_owned()
                            } else if let Some(targeting_prefix) = ["hb_bidder", "hb_pb", "hb_size"]
                                .into_iter()
                                .find(|candidate_prefix| {
                                    source_key == &format!("{candidate_prefix}_{ONTIME_ROLE}")
                                })
                            {
                                let mut targeting_key =
                                    format!("{targeting_prefix}_{}", names[ONTIME_ROLE]);
                                assert!(
                                    targeting_key.is_ascii(),
                                    "default-active targeting keys are ASCII"
                                );
                                targeting_key.truncate(20);
                                targeting_key
                            } else {
                                source_key.clone()
                            };
                        (projected_key, project_expected(field_value, names))
                    })
                    .collect(),
            ),
            Value::Array(elements) => Value::Array(
                elements
                    .iter()
                    .map(|element| project_expected(element, names))
                    .collect(),
            ),
            Value::String(text) => names.get(text.as_str()).map_or_else(
                || Value::String(text.clone()),
                |mapped_text| json!(mapped_text),
            ),
            Value::Null | Value::Bool(_) | Value::Number(_) => source_value.clone(),
        }
    }

    #[test]
    fn symbolic_projection_changes_only_identity_tokens() {
        let names = BTreeMap::from([
            (ONTIME_ROLE, "fake_st"),
            (BIDDER_TIMEOUT_ROLE, "fake_z_timeout"),
            (AUCTION_DEADLINE_ROLE, "fake_a_timeout"),
        ]);
        let template = json!({
            "seat": "fake_on_time",
            "targeting": {"hb_bidder_fake_on_time": "fake_on_time", "hb_pb_fake_on_time": "2.20", "hb_size_fake_on_time": "300x250"},
            "errors": {"fake_bidder_timeout": [{"code": 1_i32, "message": "bidder timeout"}], "prebid": [{"code": 12_i32, "message": "tmax expired"}]},
            "responsetimemillis": {"fake_on_time": 10_i32, "fake_bidder_timeout": 1000_i32, "fake_auction_deadline": 1000_i32}
        });
        assert_eq!(
            project_expected(&template, &names),
            json!({
                "seat": "fake_st",
                "targeting": {"hb_bidder_fake_st": "fake_st", "hb_pb_fake_st": "2.20", "hb_size_fake_st": "300x250"},
                "errors": {"fake_z_timeout": [{"code": 1_i32, "message": "bidder timeout"}], "prebid": [{"code": 12_i32, "message": "tmax expired"}]},
                "responsetimemillis": {"fake_st": 10_i32, "fake_z_timeout": 1000_i32, "fake_a_timeout": 1000_i32}
            })
        );
    }

    /// Answers the on-time bidder's call with a bid, times the other two
    /// out with DIFFERENT causes, and spends the whole auction budget on
    /// the shared clock before returning.
    ///
    /// The spend happens before any result is built so the batch is
    /// indistinguishable from one that ran to the deadline: by the time
    /// `run_auction` re-reads the clock, the budget is gone.
    struct MidFlightFanout {
        clock: Arc<FakeClock>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for MidFlightFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.clock.advance(AUCTION_BUDGET);
            assert_eq!(calls.len(), 3, "every role must reach the real fanout hook");
            let mut seen = BTreeSet::new();
            let results = calls
                .iter()
                .map(|call| {
                    let imps: Vec<_> = call.request.covered_imp_ids().collect();
                    assert_eq!(imps.len(), 1, "one impression per role call");
                    let imp = imps[0];
                    assert!(seen.insert(imp), "one call per role impression");
                    if imp == ONTIME_IMP {
                        return BidderCallResult {
                            result: Ok(BidderHttpResponse::new(
                                200,
                                vec![],
                                format!(
                                    "{{\"id\":\"r\",\"seatbid\":[{{\"seat\":\"s\",\"bid\":\
                                     [{{\"id\":\"bid-{ONTIME_IMP}\",\"impid\":\"{ONTIME_IMP}\",\
                                     \"price\":2.25,\"w\":300,\"h\":250,\
                                     \"crid\":\"creative-{ONTIME_IMP}\"}}]}}],\"cur\":\"USD\"}}"
                                )
                                .into_bytes(),
                            )),
                            elapsed: Duration::from_millis(10),
                        };
                    }
                    let cause = match imp {
                        BIDDER_TIMEOUT_IMP => TimeoutCause::Bidder,
                        AUCTION_DEADLINE_IMP => TimeoutCause::AuctionDeadline,
                        _ => panic!("unexpected role impression {imp}"),
                    };
                    BidderCallResult {
                        result: Err(FanoutError::Timeout(cause)),
                        elapsed: AUCTION_BUDGET,
                    }
                })
                .collect();
            assert_eq!(
                seen,
                BTreeSet::from([ONTIME_IMP, BIDDER_TIMEOUT_IMP, AUCTION_DEADLINE_IMP])
            );
            Ok(results)
        }
    }

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/tests/fixtures/mid_flight_timeout/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("fixture {path} unreadable: {err}"))
    }

    /// POST the request fixture through the real auction handler with the
    /// mid-flight fanout installed, and hand back the serialized answer.
    ///
    /// The one `FakeClock` drives BOTH wall and monotonic time, so the
    /// budget the fanout spends is the same budget `run_auction` measures
    /// its deadline against.
    fn drive_handler(request_body: &str) -> (StatusCode, serde_json::Value) {
        static AUCTIONS: Mutex<()> = Mutex::new(());
        let _serial = AUCTIONS.lock().unwrap();
        config_funding::install();
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(MidFlightFanout {
            clock: Arc::<FakeClock>::clone(&clock),
        });
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(request_body.as_bytes().to_vec()))
            .expect("request builds");
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        request
            .extensions_mut()
            .insert(TestClockHandle(handler_clock));
        let monotonic_source = Arc::<FakeClock>::clone(&clock);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                monotonic_source.now_monotonic()
            })));
        request.extensions_mut().insert(TestRngHandle(rng));

        block_on(async {
            let (ctx, mut terminal) = auction_context_for_route(
                ctx_with_config("schema_version = 1\n", request)
                    .into_request()
                    .unwrap(),
                "/openrtb2/auction",
                MonotonicClock::default(),
            );
            *terminal.response_mut() = auction(ctx).await.expect("the handler answers");
            let response = mem::take(terminal.response_mut());
            let status = response.status();
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            (
                status,
                serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                    panic!(
                        "the response is JSON: {err}; body {}",
                        String::from_utf8_lossy(&bytes),
                    )
                }),
            )
        })
    }

    /// Every `ext.errors.prebid` code in the response, in order.
    fn prebid_error_codes(actual: &serde_json::Value) -> Vec<u64> {
        actual
            .pointer("/ext/errors/prebid")
            .and_then(serde_json::Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.get("code").and_then(serde_json::Value::as_u64))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The `seatnonbid` statuscodes recorded for `seat`, in order. An
    /// empty vector means the seat has no block at all.
    fn statuscodes_for(actual: &serde_json::Value, seat: &str) -> Vec<u64> {
        actual
            .pointer("/ext/prebid/seatnonbid")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter(|block| block.get("seat").and_then(serde_json::Value::as_str) == Some(seat))
            .filter_map(|block| block.get("nonbid").and_then(serde_json::Value::as_array))
            .flatten()
            .filter_map(|row| row.get("statuscode").and_then(serde_json::Value::as_u64))
            .collect()
    }

    /// The `ext.errors` codes recorded for `bidder`, in order.
    fn bidder_error_codes(actual: &serde_json::Value, bidder: &str) -> Vec<u64> {
        actual
            .pointer(&format!("/ext/errors/{bidder}"))
            .and_then(serde_json::Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.get("code").and_then(serde_json::Value::as_u64))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn mid_flight_timeout_matches_the_recorded_answer() {
        let case = FixtureCase::new();
        let (status, actual) = drive_handler(&case.request);
        assert_eq!(
            status,
            StatusCode::OK,
            "an auction that ran out of budget mid-flight is still a 200 carrying an error \
             block, not a transport failure",
        );
        let expected = case.expected;
        assert_eq!(
            actual,
            expected,
            "recorded answer drifted; actual was {}",
            serde_json::to_string_pretty(&actual).expect("actual re-serialises"),
        );
    }

    /// The whole-response comparison above would also pass if the recorded response
    /// itself were wrong, so each surface is named here too. A future
    /// re-record has to break one of these rows to change the contract.
    #[test]
    fn each_timed_out_bidder_reports_code_one_and_the_on_time_bidder_reports_nothing() {
        let case = FixtureCase::new();
        let (_status, actual) = drive_handler(&case.request);
        assert_eq!(
            bidder_error_codes(&actual, case.name(BIDDER_TIMEOUT_ROLE)),
            vec![1_u64],
            "the bidder whose own budget elapsed reports exactly one code 1; got {actual}",
        );
        assert_eq!(
            bidder_error_codes(&actual, case.name(AUCTION_DEADLINE_ROLE)),
            vec![1_u64],
            "the bidder unresolved at the auction deadline reports code 1 on the PER-BIDDER \
             surface too -- the cause grades the global surface, not this one; got {actual}",
        );
        assert_eq!(
            bidder_error_codes(&actual, case.name(ONTIME_ROLE)),
            Vec::<u64>::new(),
            "the bidder that answered inside its budget has no error entry; got {actual}",
        );
    }

    #[test]
    fn the_auction_wide_timeout_is_recorded_exactly_once() {
        let case = FixtureCase::new();
        let (_status, actual) = drive_handler(&case.request);
        assert_eq!(
            prebid_error_codes(&actual),
            vec![12_u64],
            "two bidders timed out but the auction expired once, so `ext.errors.prebid` carries \
             a single code 12 -- a per-bidder fold would duplicate it; got {actual}",
        );
    }

    #[test]
    fn each_timed_out_bidder_gets_a_statuscode_101_non_bid_row() {
        let case = FixtureCase::new();
        let (_status, actual) = drive_handler(&case.request);
        assert_eq!(
            statuscodes_for(&actual, case.name(BIDDER_TIMEOUT_ROLE)),
            vec![101_u64],
            "got {actual}",
        );
        assert_eq!(
            statuscodes_for(&actual, case.name(AUCTION_DEADLINE_ROLE)),
            vec![101_u64],
            "got {actual}",
        );
        assert_eq!(
            statuscodes_for(&actual, case.name(ONTIME_ROLE)),
            Vec::<u64>::new(),
            "the bidder that bid has no non-bid row; got {actual}",
        );
    }

    /// `nbr` belongs to a different taxonomy and only the "no live
    /// bidders" case sets it. One bidder dispatched and bid here, so it
    /// must be absent — a timeout is not a reason to refuse the auction.
    #[test]
    fn a_mid_flight_timeout_sets_no_nbr() {
        let case = FixtureCase::new();
        let (_status, actual) = drive_handler(&case.request);
        assert_eq!(
            actual.get("nbr"),
            None,
            "a timed-out auction that still carries a bid must not set `nbr`; got {actual}",
        );
    }

    /// The control: the on-time bidder's bid reaches the wire with its
    /// own price, size and creative id intact. Without this row every
    /// assertion above would still pass on a response that dropped the
    /// whole batch the moment the budget expired.
    #[test]
    fn the_on_time_bid_survives_the_timeout_unmodified() {
        let case = FixtureCase::new();
        let (_status, actual) = drive_handler(&case.request);
        let seatbids = actual
            .get("seatbid")
            .and_then(serde_json::Value::as_array)
            .expect("the response carries a seatbid array");
        assert_eq!(
            seatbids.len(),
            1,
            "only the on-time bidder has a seat row; got {actual}",
        );
        let seat = seatbids.first().expect("one seat row");
        assert_eq!(
            seat.get("seat").and_then(serde_json::Value::as_str),
            Some(case.name(ONTIME_ROLE)),
        );
        let bids = seat
            .get("bid")
            .and_then(serde_json::Value::as_array)
            .expect("the seat row carries bids");
        assert_eq!(bids.len(), 1, "one imp, one bid; got {actual}");
        let bid = bids.first().expect("one bid");
        assert_eq!(
            bid.get("impid").and_then(serde_json::Value::as_str),
            Some(ONTIME_IMP),
        );
        assert_eq!(
            bid.get("price").and_then(serde_json::Value::as_f64),
            Some(2.25_f64),
            "the price the bidder returned reaches the wire unchanged; got {actual}",
        );
        assert_eq!(
            bid.get("crid").and_then(serde_json::Value::as_str),
            Some("creative-imp-ontime"),
        );
        assert_eq!(bid.get("w").and_then(serde_json::Value::as_u64), Some(300));
        assert_eq!(bid.get("h").and_then(serde_json::Value::as_u64), Some(250));
    }
}

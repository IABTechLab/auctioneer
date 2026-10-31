use super::super::{
    HarnessError, Result,
    http::{self, RequestSpec},
    process::Supervisor,
};
use crate::qualification::{
    profile::{Digest, LatencyEnd, LatencyStart, Population},
    report::{Counts, Latency, LatencyBucket},
};
use futures::future::{Either, join_all, select};
use serde::Serialize;
use serde_json::Value;
use std::iter::repeat_with;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::time::{Instant, sleep_until};

const BUCKETS: [u64; 4] = [500_000, 1_000_000, 2_000_000, 120_000_000];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Success,
    Rejected,
    Error,
    Timeout,
    Disconnect,
    Interrupted,
}

#[derive(Clone, Debug, Default)]
pub struct Auction {
    pub bids: u32,
    pub debug_captures: u32,
    pub debug_truncated: bool,
    pub debug_dropped_calls: u32,
    pub capture_bytes: u64,
    pub debug_output_bytes: u64,
    pub debug_denials: u32,
    pub privacy_blocked: bool,
    pub privacy_refused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigStatus {
    Ok,
    Cached,
    Degraded,
    Missing,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub category: Category,
    pub status: u16,
    pub latency_us: u64,
    pub slow_reads: u32,
    pub body_sha256: Digest,
    pub config_status: ConfigStatus,
    pub auction: Auction,
}

pub struct Options {
    pub duration_ms: u64,
    pub workers: u32,
    pub max_requests: u32,
    pub timeout_ms: u64,
    pub pace: bool,
}

pub struct LoadResult {
    pub outcomes: Counts,
    pub latency: Latency,
    pub load_window_ms: u64,
    pub duration_ms: u64,
    pub results: Vec<(u32, Outcome)>,
}

fn grouped<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get("ext")
        .and_then(|ext| ext.get(key))
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|groups| groups.values())
        .filter_map(Value::as_array)
        .flatten()
}

fn inspected_auction(value: &Value) -> Result<(Auction, Category)> {
    let count = |length: usize| {
        u32::try_from(length).map_err(|_error| HarnessError::new("invalid_child_output"))
    };
    let bids = value
        .get("seatbid")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|seat| seat.get("bid").and_then(Value::as_array))
        .map(Vec::len)
        .sum::<usize>();
    let mut view = Auction {
        bids: count(bids)?,
        ..Auction::default()
    };
    let ext = value.get("ext");
    if let Some(debug) = ext.and_then(|extension| extension.get("debug")) {
        view.debug_output_bytes = auctioneer_runtime::serialized_json_bytes(debug)
            .map_err(|_error| HarnessError::new("invalid_child_output"))?;
        view.debug_truncated = debug.get("truncated").and_then(Value::as_bool) == Some(true);
        view.debug_dropped_calls = debug
            .get("dropped_calls")
            .and_then(Value::as_u64)
            .and_then(|number| u32::try_from(number).ok())
            .unwrap_or(0);
        for row in debug
            .get("httpcalls")
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|calls| calls.values())
            .filter_map(Value::as_array)
            .flatten()
        {
            view.debug_captures = view
                .debug_captures
                .checked_add(1)
                .ok_or(HarnessError::new("invalid_child_output"))?;
            for side in ["request", "response"] {
                if let Some(body) = row
                    .get(side)
                    .and_then(|message| message.get("body"))
                    .and_then(Value::as_str)
                {
                    view.capture_bytes = view
                        .capture_bytes
                        .checked_add(
                            u64::try_from(body.len())
                                .map_err(|_error| HarnessError::new("invalid_child_output"))?,
                        )
                        .ok_or(HarnessError::new("invalid_child_output"))?;
                }
            }
        }
    }
    for warning in grouped(value, "warnings") {
        match warning.get("code").and_then(Value::as_u64) {
            Some(10002) => view.debug_denials = view.debug_denials.saturating_add(1),
            Some(10017) => view.privacy_blocked = true,
            Some(10001 | 10018 | 10019) => view.privacy_refused = true,
            _ => {}
        }
    }
    let mut category = Category::Success;
    for error in grouped(value, "errors") {
        if error.get("code").and_then(Value::as_u64) == Some(1) {
            category = Category::Timeout;
            break;
        }
        category = Category::Error;
    }
    Ok((view, category))
}

fn inspect(response: &http::Response, auction: bool) -> Result<Outcome> {
    let category = match response.category {
        http::Category::Success => Category::Success,
        http::Category::Rejected => Category::Rejected,
        http::Category::Timeout => Category::Timeout,
        http::Category::Disconnect => Category::Disconnect,
        http::Category::Interrupted => Category::Interrupted,
        http::Category::HttpError | http::Category::NetworkError | http::Category::BodyLimit => {
            Category::Error
        }
    };
    let config_status = match response
        .headers
        .get("x-auctioneer-config-status")
        .and_then(|value| value.to_str().ok())
    {
        Some("ok") => ConfigStatus::Ok,
        Some("ok-cached") => ConfigStatus::Cached,
        Some("degraded") => ConfigStatus::Degraded,
        _ => ConfigStatus::Missing,
    };
    let mut result = Outcome {
        category,
        status: response.status,
        latency_us: response.latency_us,
        slow_reads: response.slow_reads,
        body_sha256: Digest::of(&response.body),
        config_status,
        auction: Auction::default(),
    };
    if auction && category == Category::Success {
        let parsed = auctioneer_runtime::count_json_nodes(&response.body, 0x0001_0000)
            .ok()
            .and_then(|_nodes| serde_json::from_slice::<Value>(&response.body).ok())
            .filter(Value::is_object);
        if let Some(value) = parsed {
            (result.auction, result.category) = inspected_auction(&value)?;
        } else {
            result.category = Category::Error;
        }
    }
    Ok(result)
}

pub async fn call(
    origin: &str,
    path: &str,
    mut spec: RequestSpec,
    supervisor: &Supervisor,
) -> Result<Outcome> {
    supervisor.check_active().await?;
    if !matches!(
        path,
        "/status" | "/info/bidders" | "/getuids" | "/openrtb2/auction"
    ) {
        return Err(HarnessError::new("invalid_input"));
    }
    spec.inspect = true;
    let response = http::request(&format!("{origin}{path}"), spec, supervisor).await?;
    supervisor.check_active().await?;
    inspect(&response, path == "/openrtb2/auction")
}

pub async fn pause_until(end: Instant, supervisor: &Supervisor) -> Result<()> {
    match select(Box::pin(supervisor.cancelled()), Box::pin(sleep_until(end))).await {
        Either::Left((error, _sleep)) => Err(error),
        Either::Right(((), _cancelled)) => supervisor.check_active().await,
    }
}

async fn worker<F>(
    origin: &str,
    options: &Options,
    supervisor: &Supervisor,
    make: &F,
    counter: &AtomicU32,
    start: Instant,
    end: Instant,
) -> Result<Vec<(u32, Outcome)>>
where
    F: Fn(u32) -> Result<RequestSpec> + Sync,
{
    let mut rows = Vec::new();
    while Instant::now() < end {
        supervisor.check_active().await?;
        let next = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value
                .checked_add(1)
                .filter(|next| *next <= options.max_requests)
        });
        let Ok(previous) = next else {
            break;
        };
        let id = previous
            .checked_add(1)
            .ok_or(HarnessError::new("invalid_input"))?;
        if options.pace {
            let offset = u64::from(previous)
                .checked_mul(options.duration_ms)
                .and_then(|elapsed| elapsed.checked_div(u64::from(options.max_requests)))
                .ok_or(HarnessError::new("invalid_input"))?;
            let at = start
                .checked_add(Duration::from_millis(offset))
                .ok_or(HarnessError::new("invalid_input"))?;
            pause_until(at, supervisor).await?;
        }
        let mut spec = make(id)?;
        spec.timeout_ms = options.timeout_ms;
        rows.push((
            id,
            call(origin, "/openrtb2/auction", spec, supervisor).await?,
        ));
    }
    Ok(rows)
}

fn add(outcomes: &mut Counts, histogram: &mut [LatencyBucket], row: &Outcome) -> Result<()> {
    outcomes.offered = outcomes
        .offered
        .checked_add(1)
        .ok_or(HarnessError::new("invalid_input"))?;
    if row.category != Category::Rejected {
        outcomes.admitted = outcomes
            .admitted
            .checked_add(1)
            .ok_or(HarnessError::new("invalid_input"))?;
    }
    let counter = match row.category {
        Category::Success => &mut outcomes.success,
        Category::Rejected => &mut outcomes.rejected,
        Category::Timeout => &mut outcomes.timeouts,
        Category::Error | Category::Disconnect | Category::Interrupted => &mut outcomes.errors,
    };
    *counter = counter
        .checked_add(1)
        .ok_or(HarnessError::new("invalid_input"))?;
    let bucket = histogram
        .iter_mut()
        .find(|bucket| row.latency_us <= bucket.upper_us)
        .ok_or(HarnessError::new("latency_overflow"))?;
    bucket.count = bucket
        .count
        .checked_add(1)
        .ok_or(HarnessError::new("latency_overflow"))?;
    Ok(())
}

fn latency(histogram: Vec<LatencyBucket>, samples: u64) -> Result<Latency> {
    let rank = samples
        .checked_mul(99)
        .ok_or(HarnessError::new("latency_overflow"))?
        .div_ceil(100);
    let mut cumulative = 0_u64;
    let mut value_us = 0;
    for row in &histogram {
        cumulative = cumulative
            .checked_add(row.count)
            .ok_or(HarnessError::new("latency_overflow"))?;
        if value_us == 0 && cumulative >= rank && row.count != 0 {
            value_us = row.upper_us;
        }
    }
    if cumulative != samples {
        return Err(HarnessError::new("latency_overflow"));
    }
    Ok(Latency {
        histogram: histogram.into_iter().filter(|row| row.count != 0).collect(),
        start: LatencyStart::ClientDispatch,
        end: LatencyEnd::TerminalOutcome,
        population: Population::AllOffered,
        percentile: 99,
        samples,
        value_us,
    })
}

fn milliseconds(elapsed: Duration) -> Result<u64> {
    u64::try_from(elapsed.as_millis()).map_err(|_error| HarnessError::new("invalid_input"))
}

pub async fn run<F>(
    origin: &str,
    options: Options,
    supervisor: &Supervisor,
    make: F,
) -> Result<LoadResult>
where
    F: Fn(u32) -> Result<RequestSpec> + Sync,
{
    if !(1..=60_000).contains(&options.duration_ms)
        || !(1..=32).contains(&options.workers)
        || !(1..=10_000).contains(&options.max_requests)
        || !(1..=10_000).contains(&options.timeout_ms)
    {
        return Err(HarnessError::new("invalid_input"));
    }
    let start = Instant::now();
    let end = start
        .checked_add(Duration::from_millis(options.duration_ms))
        .ok_or(HarnessError::new("invalid_input"))?;
    let counter = AtomicU32::new(0);
    let workers =
        usize::try_from(options.workers).map_err(|_error| HarnessError::new("invalid_input"))?;
    // These futures remain owned and joined, including after a sibling fails.
    let batches = join_all(
        repeat_with(|| worker(origin, &options, supervisor, &make, &counter, start, end))
            .take(workers),
    )
    .await;
    let mut results = Vec::new();
    for batch in batches {
        results.extend(batch?);
    }
    results.sort_by_key(|(id, _row)| *id);
    if usize::try_from(counter.load(Ordering::Acquire)).ok() != Some(results.len()) {
        return Err(HarnessError::new("workload_underload"));
    }
    let load_window_ms = milliseconds(start.elapsed())?;
    pause_until(end, supervisor).await?;
    let mut outcomes = Counts {
        offered: 0,
        admitted: 0,
        success: 0,
        rejected: 0,
        errors: 0,
        timeouts: 0,
    };
    let mut histogram: Vec<_> = BUCKETS
        .into_iter()
        .map(|upper_us| LatencyBucket { upper_us, count: 0 })
        .collect();
    for (_id, row) in &results {
        add(&mut outcomes, &mut histogram, row)?;
    }
    Ok(LoadResult {
        latency: latency(histogram, outcomes.offered)?,
        outcomes,
        load_window_ms,
        duration_ms: milliseconds(start.elapsed())?,
        results,
    })
}

pub fn merge(load: &mut LoadResult, primary: &[Outcome], elapsed_ms: u64) -> Result<()> {
    let mut histogram = BUCKETS
        .into_iter()
        .map(|upper_us| LatencyBucket {
            upper_us,
            count: load
                .latency
                .histogram
                .iter()
                .find(|row| row.upper_us == upper_us)
                .map_or(0, |row| row.count),
        })
        .collect::<Vec<_>>();
    for row in primary {
        add(&mut load.outcomes, &mut histogram, row)?;
    }
    load.latency = latency(histogram, load.outcomes.offered)?;
    load.load_window_ms = load
        .load_window_ms
        .checked_add(elapsed_ms)
        .ok_or(HarnessError::new("invalid_input"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::http::{HeaderMap, Method};
    use serde_json::json;

    fn response(category: http::Category, status: u16, body: &Value) -> http::Response {
        http::Response {
            category,
            status,
            bytes: 0,
            latency_us: 600_000,
            slow_reads: 0,
            body: serde_json::to_vec(body).unwrap(),
            headers: HeaderMap::new(),
        }
    }

    #[test]
    fn status_rejection_cannot_be_reclassified_by_an_auction_error_or_bid_body() {
        for status in [429, 503] {
            let result = inspect(
                &response(
                    http::Category::Rejected,
                    status,
                    &json!({"seatbid":[{"bid":[{"price":9.5_f64}]}],"ext":{"errors":{"fake_bidder":[{"code":1_i32}]}}}),
                ),
                true,
            ).unwrap();
            assert_eq!(result.category, Category::Rejected);
            assert_eq!(result.status, status);
            assert_eq!(result.auction.bids, 0);
        }
        let timeout = inspect(
            &response(
                http::Category::Success,
                200,
                &json!({"ext":{"errors":{"fake_bidder":[{"code":1_i32}]}}}),
            ),
            true,
        )
        .unwrap();
        assert_eq!(timeout.category, Category::Timeout);
        let failure = inspect(
            &response(
                http::Category::Success,
                200,
                &json!({"ext":{"errors":{"fake_bidder":[{"code":4_i32}]}}}),
            ),
            true,
        )
        .unwrap();
        assert_eq!(failure.category, Category::Error);
    }

    #[test]
    fn auction_inspection_retains_only_safe_counters_and_exact_debug_sizes() {
        let debug = json!({"httpcalls":{"fake_00":[{"request":{"body":"ab"},"response":{"body":"cde"}}]},"truncated":true,"dropped_calls":3_i32});
        let size = u64::try_from(serde_json::to_vec(&debug).unwrap().len()).unwrap();
        let result = inspect(
            &response(
                http::Category::Success,
                200,
                &json!({"seatbid":[{"bid":[{},{}]},{"bid":[{}]}],
                    "ext":{"debug":debug,"warnings":{"fake_00":[{"code":10_002_i32},{"code":10_017_i32},{"code":10_019_i32}]}}}),
            ),
            true,
        ).unwrap();
        assert_eq!(result.category, Category::Success);
        assert_eq!(result.auction.bids, 3);
        assert_eq!(result.auction.debug_captures, 1);
        assert_eq!(result.auction.capture_bytes, 5);
        assert_eq!(result.auction.debug_output_bytes, size);
        assert!(result.auction.debug_truncated);
        assert_eq!(result.auction.debug_dropped_calls, 3);
        assert_eq!(result.auction.debug_denials, 1);
        assert!(result.auction.privacy_blocked);
        assert!(result.auction.privacy_refused);
    }

    #[tokio::test]
    async fn real_load_counts_each_offered_request_without_padding_missing_work() {
        use super::super::super::fixture::protocol;
        use bytes::Bytes;
        use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, duplex};
        let (mut controls, input) = duplex(8192);
        let (output, replies) = duplex(8192);
        let supervisor = Supervisor::new();
        let child_scope = supervisor.clone();
        let child = tokio::spawn(async move { protocol::serve(input, output, &child_scope).await });
        controls.write_all(b"{\"kind\":\"native\",\"port\":0,\"body_cap\":4096,\"max_requests\":100,\"max_sockets\":4}\n").await.unwrap();
        let mut reader = BufReader::new(replies);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let ready: Value = serde_json::from_str(&line).unwrap();
        let result = run(
            &format!("http://127.0.0.1:{}", ready["port"]),
            Options {
                duration_ms: 100,
                workers: 2,
                max_requests: 4,
                timeout_ms: 2500,
                pace: false,
            },
            &supervisor,
            |id| {
                Ok(RequestSpec {
                    method: Method::POST,
                    body: Bytes::from(
                        serde_json::to_vec(&json!({"id":format!("q{id}"),"imp":[{"id":"i0"}]}))
                            .unwrap(),
                    ),
                    ..RequestSpec::default()
                })
            },
        )
        .await
        .unwrap();
        assert_eq!(result.outcomes.offered, 4);
        assert_eq!(result.outcomes.admitted, 4);
        assert_eq!(result.outcomes.success, 4);
        assert_eq!(result.outcomes.rejected, 0);
        assert_eq!(result.outcomes.errors, 0);
        assert_eq!(result.outcomes.timeouts, 0);
        assert_eq!(result.latency.samples, 4);
        assert_eq!(
            result
                .latency
                .histogram
                .iter()
                .map(|row| row.count)
                .sum::<u64>(),
            4
        );
        assert_eq!(
            result.results.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert!(result.duration_ms >= 100);
        drop(controls);
        child.await.unwrap().unwrap();
    }
}

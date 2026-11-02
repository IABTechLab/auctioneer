use super::super::{
    HarnessError, Result,
    fixture::{Mode, Settings, Stats},
    process::Supervisor,
};
use super::super::{
    http::{Progress, RequestSpec},
    process::ManagedTask,
};
use super::{
    consents,
    geometry::{AuctionOptions, make_auction},
    load::{self, Category, ConfigStatus},
};
use super::{consents::Consent, load::Outcome, snapshots::InstalledVariant};
use crate::qualification::profile::{Digest, Load};
use bytes::Bytes;
use http::Method;
use serde::Serialize;
use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::{sync::Mutex, time::Instant};

pub use super::super::fixture::client::FixtureControl as Control;

pub struct Context<'a> {
    pub origin: &'a str,
    pub load: Load,
    pub seed: u32,
    pub timeout_ms: u64,
    pub supervisor: &'a Supervisor,
}

pub struct Identity {
    pub resources_sha256: Digest,
    pub review_sha256: Digest,
}

#[derive(Serialize)]
pub struct AdmissionProof {
    pub method: &'static str,
    pub application_concurrency: u32,
    pub second_constructed: bool,
    pub first_response_started: bool,
    pub probe_status: u16,
    pub probe_body_sha256: Digest,
    pub status_healthy: bool,
    pub probe_requests: u32,
    pub first_calls: u32,
    pub second_calls: u32,
    pub resources_sha256: Digest,
    pub review_sha256: Digest,
}

pub struct Admission {
    pub proof: AdmissionProof,
    pub sha256: Digest,
    pub results: Vec<Outcome>,
    pub stats: Stats,
    pub duration_ms: u64,
}

pub struct Overlap {
    pub result: Outcome,
    pub parses: u64,
    pub stats: Stats,
    pub duration_ms: u64,
}

pub(super) struct Primary {
    progress: Progress,
    done: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<Outcome>>>>,
    task: ManagedTask,
}

impl Primary {
    pub(super) async fn start(context: &Context<'_>, id: u32, debug: bool) -> Result<Self> {
        let progress = Progress::default();
        let done = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(None));
        let completion = Arc::clone(&done);
        let output = Arc::clone(&result);
        let origin = context.origin.to_owned();
        let scope = context.supervisor.clone();
        let spec = RequestSpec {
            method: Method::POST,
            body: Bytes::from(make_auction(
                context.load,
                AuctionOptions {
                    id,
                    debug,
                    dense_debug: debug,
                    ..AuctionOptions::default()
                },
            )?),
            timeout_ms: context.timeout_ms,
            progress: progress.clone(),
            ..RequestSpec::default()
        };
        let task = context
            .supervisor
            .task(async move {
                let outcome = load::call(&origin, "/openrtb2/auction", spec, &scope).await;
                *output.lock().await = Some(outcome);
                completion.store(true, Ordering::Release);
            })
            .await?;
        Ok(Self {
            progress,
            done,
            result,
            task,
        })
    }

    pub(super) fn held(&self) -> bool {
        self.progress.sent.load(Ordering::Acquire)
            && !self.progress.headers.load(Ordering::Acquire)
            && !self.done.load(Ordering::Acquire)
    }
    fn started_response(&self) -> bool {
        self.progress.headers.load(Ordering::Acquire) || self.done.load(Ordering::Acquire)
    }

    pub(super) async fn drain(&self, supervisor: &Supervisor) -> Result<Outcome> {
        supervisor.join_task(&self.task).await?;
        self.result
            .lock()
            .await
            .take()
            .ok_or(HarnessError::new("invalid_child_output"))?
    }
}

pub async fn idle<C: Control>(fixture: &mut C, supervisor: &Supervisor) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(2500))
        .ok_or(HarnessError::new("fixture_busy"))?;
    loop {
        supervisor.check_active().await?;
        if fixture.stats().await?.current_active_calls == 0 {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(HarnessError::new("fixture_busy"));
        }
        let next_poll = Instant::now()
            .checked_add(Duration::from_millis(25))
            .ok_or(HarnessError::new("fixture_busy"))?;
        load::pause_until(next_poll, supervisor).await?;
    }
}

pub fn settings(load: Load, mode: Mode, seed: u32, delay_ms: u64, active_streams: u32) -> Settings {
    Settings {
        mode,
        encoded_bytes: load.encoded_bytes,
        decoded_bytes: load.decoded_bytes,
        seed,
        delay_ms,
        stall_after_calls: if matches!(mode, Mode::Stall) {
            load.calls.saturating_sub(active_streams)
        } else {
            0
        },
    }
}

fn oracle_error(error: HarnessError, category: &'static str) -> HarnessError {
    if matches!(
        error.code,
        "interrupted" | "run_deadline" | "cleanup_failed"
    ) {
        error
    } else {
        let wrapped = HarnessError::new(category);
        if let Some(stage) = error.stage {
            wrapped.at(stage)
        } else {
            wrapped
        }
    }
}

async fn wait_for<C: Control, F: Fn(&Stats) -> bool>(
    context: &Context<'_>,
    fixture: &mut C,
    ids: [u32; 2],
    primaries: &[Primary],
    deadline: Instant,
    condition: F,
) -> Result<Stats> {
    loop {
        context.supervisor.check_active().await?;
        if Instant::now() >= deadline || primaries.iter().any(Primary::started_response) {
            return Err(HarnessError::new("admission_oracle_failed"));
        }
        let [first, second] = ids;
        let stats = fixture.probe(first, second).await?;
        if Instant::now() >= deadline {
            return Err(HarnessError::new("admission_oracle_failed"));
        }
        if condition(&stats) {
            return Ok(stats);
        }
        let next_poll = Instant::now()
            .checked_add(Duration::from_millis(5))
            .ok_or(HarnessError::new("admission_oracle_failed"))?;
        load::pause_until(next_poll, context.supervisor).await?;
    }
}

pub fn status_observed(result: &Outcome, expected: &InstalledVariant) -> bool {
    result.category == Category::Success
        && result.body_sha256 == expected.banner_sha256
        && matches!(
            result.config_status,
            ConfigStatus::Ok | ConfigStatus::Cached
        )
}

async fn challenge(
    context: &Context<'_>,
    primaries: &[Primary],
    expected: &InstalledVariant,
    probes: &mut u32,
) -> Result<()> {
    if !primaries.iter().all(Primary::held) {
        return Err(HarnessError::new("admission_oracle_failed"));
    }
    let status = load::call(
        context.origin,
        "/status",
        RequestSpec {
            timeout_ms: 100,
            body_cap: 4096,
            ..RequestSpec::default()
        },
        context.supervisor,
    )
    .await?;
    *probes = probes
        .checked_add(1)
        .ok_or(HarnessError::new("admission_oracle_failed"))?;
    let id = 620_000_u32
        .checked_add(*probes)
        .ok_or(HarnessError::new("admission_oracle_failed"))?;
    let probe = load::call(
        context.origin,
        "/openrtb2/auction",
        RequestSpec {
            method: Method::POST,
            timeout_ms: 80,
            body: Bytes::from(make_auction(
                context.load,
                AuctionOptions {
                    id,
                    ..AuctionOptions::default()
                },
            )?),
            ..RequestSpec::default()
        },
        context.supervisor,
    )
    .await?;
    if !primaries.iter().all(Primary::held)
        || !status_observed(&status, expected)
        || probe.category != Category::Rejected
        || probe.status != 503
        || probe.body_sha256 != Digest::of(b"overloaded\n")
    {
        return Err(HarnessError::new("admission_oracle_failed"));
    }
    Ok(())
}

async fn phases<C: Control>(
    context: &Context<'_>,
    fixture: &mut C,
    expected: &InstalledVariant,
    primaries: &mut Vec<Primary>,
    debug: bool,
) -> Result<u32> {
    let ids = if debug {
        [710_001, 710_002]
    } else {
        [610_001, 610_002]
    };
    let [first, second] = ids;
    let half_timeout = context
        .timeout_ms
        .checked_div(2)
        .ok_or(HarnessError::new("admission_oracle_failed"))?;
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(250_u64.min(half_timeout)))
        .ok_or(HarnessError::new("admission_oracle_failed"))?;
    fixture
        .configure(&settings(context.load, Mode::Park, context.seed, 10, 0))
        .await
        .map_err(|error| error.at("admission_fixture_configure"))?;
    primaries.push(
        Primary::start(context, first, debug)
            .await
            .map_err(|error| error.at("admission_first_parked"))?,
    );
    wait_for(context, fixture, ids, primaries, deadline, |stats| {
        stats.probe_first_calls > 0
    })
    .await
    .map_err(|error| error.at("admission_first_parked"))?;
    primaries.push(
        Primary::start(context, second, debug)
            .await
            .map_err(|error| error.at("admission_second_sent"))?,
    );
    wait_for(context, fixture, ids, primaries, deadline, |_stats| {
        primaries
            .last()
            .is_some_and(|primary| primary.progress.sent.load(Ordering::Acquire))
    })
    .await
    .map_err(|error| error.at("admission_second_sent"))?;
    let mut probes = 0;
    challenge(context, primaries, expected, &mut probes)
        .await
        .map_err(|error| error.at("admission_first_challenge"))?;
    fixture
        .release(false)
        .await
        .map_err(|error| error.at("admission_wave_handoff"))?;
    wait_for(context, fixture, ids, primaries, deadline, |stats| {
        stats.probe_second_calls > 0
    })
    .await
    .map_err(|error| error.at("admission_wave_handoff"))?;
    challenge(context, primaries, expected, &mut probes)
        .await
        .map_err(|error| error.at("admission_second_challenge"))?;
    Ok(probes)
}

pub async fn admission<C: Control>(
    context: &Context<'_>,
    fixture: &mut C,
    expected: &InstalledVariant,
    identity: &Identity,
    debug: bool,
) -> Result<Admission> {
    if context.load.concurrency != 2 {
        return Err(HarnessError::new("admission_oracle_failed"));
    }
    let start = Instant::now();
    let mut primaries = Vec::new();
    let phase = phases(context, fixture, expected, &mut primaries, debug).await;
    let released = fixture.release(true).await;
    let mut results = Vec::new();
    let mut drain_error = None;
    for primary in &primaries {
        match primary.drain(context.supervisor).await {
            Ok(result) => results.push(result),
            Err(error) => {
                drain_error = Some(error);
            }
        }
    }
    if let Some(error) = drain_error {
        return Err(oracle_error(
            error.at("admission_primary_drain"),
            "admission_oracle_failed",
        ));
    }
    let probes = phase.map_err(|error| oracle_error(error, "admission_oracle_failed"))?;
    released.map_err(|error| {
        oracle_error(
            error.at("admission_primary_drain"),
            "admission_oracle_failed",
        )
    })?;
    let [first, second] = if debug {
        [710_001, 710_002]
    } else {
        [610_001, 610_002]
    };
    let stats = fixture.probe(first, second).await?;
    if results.len() != 2
        || results.iter().any(|result| {
            result.category != Category::Success
                || result.auction.bids == 0
                || (debug && result.auction.debug_captures != context.load.calls)
        })
        || stats.probe_first_calls < context.load.calls
        || stats.probe_second_calls < context.load.calls
    {
        return Err(HarnessError::new("admission_oracle_failed").at("admission_primary_completion"));
    }
    let proof = AdmissionProof {
        method: "parked_two_auctions_v1",
        application_concurrency: 2,
        second_constructed: true,
        first_response_started: false,
        probe_status: 503,
        probe_body_sha256: Digest::of(b"overloaded\n"),
        status_healthy: true,
        probe_requests: probes,
        first_calls: stats.probe_first_calls,
        second_calls: stats.probe_second_calls,
        resources_sha256: identity.resources_sha256.clone(),
        review_sha256: identity.review_sha256.clone(),
    };
    Ok(Admission {
        sha256: Digest::canonical(&proof).map_err(|_error| HarnessError::new("invalid_input"))?,
        proof,
        results,
        stats,
        duration_ms: u64::try_from(start.elapsed().as_millis())
            .map_err(|_error| HarnessError::new("invalid_input"))?,
    })
}
async fn still_held<C: Control>(primary: &Primary, fixture: &mut C) -> Result<bool> {
    let active = fixture.stats().await?.current_active_calls;
    Ok(primary.held() && active > 0)
}

async fn lookups<C: Control>(
    context: &Context<'_>,
    fixture: &mut C,
    primary: &Primary,
    consents: &[Consent],
    seen: &mut BTreeSet<u16>,
) -> Result<u64> {
    let half_timeout = context
        .timeout_ms
        .checked_div(2)
        .ok_or(HarnessError::new("gvl_overlap_failed"))?;
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(250_u64.min(half_timeout)))
        .ok_or(HarnessError::new("gvl_overlap_failed"))?;
    while !still_held(primary, fixture).await? {
        context.supervisor.check_active().await?;
        if Instant::now() >= deadline || primary.started_response() {
            return Err(HarnessError::new("gvl_overlap_failed"));
        }
        let next_poll = Instant::now()
            .checked_add(Duration::from_millis(5))
            .ok_or(HarnessError::new("gvl_overlap_failed"))?;
        load::pause_until(next_poll, context.supervisor).await?;
    }
    let mut parses = 0_u64;
    for (index, consent) in consents.iter().enumerate() {
        context.supervisor.check_active().await?;
        if !still_held(primary, fixture).await? {
            return Err(HarnessError::new("gvl_overlap_failed"));
        }
        let id = 900_001_u32
            .checked_add(
                u32::try_from(index).map_err(|_error| HarnessError::new("gvl_overlap_failed"))?,
            )
            .ok_or(HarnessError::new("gvl_overlap_failed"))?;
        let spec = RequestSpec {
            method: Method::POST,
            body: Bytes::from(make_auction(
                context.load,
                AuctionOptions {
                    id,
                    consent: Some(consent.value()),
                    ..AuctionOptions::default()
                },
            )?),
            timeout_ms: context.timeout_ms,
            ..RequestSpec::default()
        };
        let result = load::call(
            context.origin,
            "/openrtb2/auction",
            spec,
            context.supervisor,
        )
        .await?;
        if !still_held(primary, fixture).await?
            || consents::credit(&result, consent, Some(consent.value()), seen) != 1
        {
            return Err(HarnessError::new("gvl_overlap_failed"));
        }
        parses = parses
            .checked_add(1)
            .ok_or(HarnessError::new("gvl_overlap_failed"))?;
    }
    Ok(parses)
}

pub async fn gvl_overlap<C: Control>(
    context: &Context<'_>,
    fixture: &mut C,
    consents: &[Consent],
    seen: &mut BTreeSet<u16>,
) -> Result<Overlap> {
    if consents.is_empty() || consents.len() > 20 {
        return Err(HarnessError::new("gvl_overlap_failed"));
    }
    let start = Instant::now();
    fixture
        .configure(&settings(context.load, Mode::Park, context.seed, 10, 0))
        .await?;
    let primary = Primary::start(context, 890_000, false).await?;
    let phase = lookups(context, fixture, &primary, consents, seen).await;
    let released = fixture.release(true).await;
    let result = primary
        .drain(context.supervisor)
        .await
        .map_err(|error| oracle_error(error, "gvl_overlap_failed"))?;
    let parses = phase.map_err(|error| oracle_error(error, "gvl_overlap_failed"))?;
    released.map_err(|error| oracle_error(error, "gvl_overlap_failed"))?;
    let coverage = fixture.coverage(&[890_000], context.load.calls).await?;
    if result.category != Category::Success
        || result.auction.bids == 0
        || coverage.complete_auctions != 1
    {
        return Err(HarnessError::new("gvl_overlap_failed"));
    }
    Ok(Overlap {
        result,
        parses,
        stats: fixture.stats().await?,
        duration_ms: u64::try_from(start.elapsed().as_millis())
            .map_err(|_error| HarnessError::new("invalid_input"))?,
    })
}

pub fn attested_load(
    load: Load,
    ordinary: &Stats,
    decoded: u64,
    admission: &Admission,
) -> (Load, bool) {
    let concurrency = admission.proof.application_concurrency;
    (
        Load {
            concurrency,
            calls: ordinary.max_calls_per_auction,
            request_bytes: if ordinary.calls > 0 {
                load.request_bytes
            } else {
                0
            },
            nodes: if ordinary.calls > 0 { load.nodes } else { 0 },
            impressions: if ordinary.calls > 0 {
                load.impressions
            } else {
                0
            },
            encoded_bytes: if decoded > 0 {
                ordinary.max_encoded_bytes
            } else {
                0
            },
            decoded_bytes: if decoded > 0 {
                ordinary.max_decoded_bytes
            } else {
                0
            },
        },
        concurrency >= load.concurrency,
    )
}

pub fn merge_stats(left: &Stats, right: &Stats) -> Result<Stats> {
    let sum = |a: u32, b: u32| {
        a.checked_add(b)
            .ok_or(HarnessError::new("invalid_child_output"))
    };
    Ok(Stats {
        calls: sum(left.calls, right.calls)?,
        completed: sum(left.completed, right.completed)?,
        fpd_expansions: sum(left.fpd_expansions, right.fpd_expansions)?,
        gzip_responses: sum(left.gzip_responses, right.gzip_responses)?,
        stalled_reads: sum(left.stalled_reads, right.stalled_reads)?,
        disconnected: sum(left.disconnected, right.disconnected)?,
        completed_auctions: sum(left.completed_auctions, right.completed_auctions)?,
        max_encoded_bytes: left.max_encoded_bytes.max(right.max_encoded_bytes),
        max_decoded_bytes: left.max_decoded_bytes.max(right.max_decoded_bytes),
        max_calls_per_auction: left.max_calls_per_auction.max(right.max_calls_per_auction),
        max_active_auctions: left.max_active_auctions.max(right.max_active_auctions),
        max_active_calls: left.max_active_calls.max(right.max_active_calls),
        current_active_calls: left.current_active_calls.max(right.current_active_calls),
        max_active_debug_auctions: left
            .max_active_debug_auctions
            .max(right.max_active_debug_auctions),
        probe_first_calls: left.probe_first_calls.max(right.probe_first_calls),
        probe_second_calls: left.probe_second_calls.max(right.probe_second_calls),
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::fixture::Coverage;
    use super::*;
    use bytes::Bytes;
    use http::{Request, Response, StatusCode};
    use http_body_util::{BodyExt as _, Full};
    use hyper::{body::Incoming, server::conn::http1, service::service_fn};
    use hyper_util::rt::TokioIo;
    use serde_json::{Value, json};
    use std::convert::Infallible;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::{
        net::TcpListener,
        sync::Notify,
        task::{JoinHandle, JoinSet},
    };

    struct State {
        arrivals: Mutex<BTreeSet<u32>>,
        wave: AtomicBool,
        released: AtomicBool,
        changed: Notify,
        completed: AtomicU32,
        serial: bool,
        bad_body: bool,
        lookup_releases: bool,
    }

    struct Fixture(Arc<State>);

    impl Fixture {
        fn observed(&self) -> Stats {
            let arrivals = self.0.arrivals.lock().unwrap();
            let second = self.0.wave.load(Ordering::Acquire) && !self.0.serial;
            let first_calls = u32::from(arrivals.contains(&610_001) || arrivals.contains(&710_001))
                .checked_mul(4)
                .unwrap();
            let second_calls =
                u32::from(second && (arrivals.contains(&610_002) || arrivals.contains(&710_002)))
                    .checked_mul(4)
                    .unwrap();
            Stats {
                calls: first_calls.checked_add(second_calls).unwrap(),
                max_calls_per_auction: 4,
                current_active_calls: if self.0.released.load(Ordering::Acquire) {
                    0
                } else {
                    u32::try_from(arrivals.len()).unwrap()
                },
                probe_first_calls: first_calls,
                probe_second_calls: second_calls,
                ..Stats::default()
            }
        }
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "immediate test controls must read and mutate state only when polled"
    )]
    impl Control for Fixture {
        async fn configure(&mut self, _settings: &Settings) -> Result<Stats> {
            Ok(self.observed())
        }
        async fn stats(&mut self) -> Result<Stats> {
            Ok(self.observed())
        }
        async fn probe(&mut self, _first: u32, _second: u32) -> Result<Stats> {
            Ok(self.observed())
        }
        async fn release(&mut self, all: bool) -> Result<Stats> {
            self.0.wave.store(true, Ordering::Release);
            if all {
                self.0.released.store(true, Ordering::Release);
                self.0.changed.notify_waiters();
            }
            Ok(self.observed())
        }
        async fn coverage(&mut self, ids: &[u32], _calls: u32) -> Result<Coverage> {
            Ok(Coverage {
                auctions: u32::try_from(ids.len()).unwrap(),
                complete_auctions: u32::try_from(
                    ids.iter()
                        .filter(|id| self.0.arrivals.lock().unwrap().contains(id))
                        .count(),
                )
                .unwrap(),
            })
        }
    }

    async fn route(request: Request<Incoming>, state: Arc<State>) -> Response<Full<Bytes>> {
        if request.uri().path() == "/status" {
            return Response::builder()
                .header("x-auctioneer-config-status", "ok")
                .body(Full::new(Bytes::from_static(b"q2_a")))
                .unwrap();
        }
        let body = request.into_body().collect().await.unwrap().to_bytes();
        let payload: Value = serde_json::from_slice(&body).unwrap();
        let id = payload["id"]
            .as_str()
            .unwrap()
            .strip_prefix('q')
            .unwrap()
            .parse::<u32>()
            .unwrap();
        if (620_001..630_000).contains(&id) {
            return Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(Full::new(Bytes::from_static(if state.bad_body {
                    b"wrong"
                } else {
                    b"overloaded\n"
                })))
                .unwrap();
        }
        if payload["regs"]["gdpr"] == 1_i32 {
            if state.lookup_releases {
                state.released.store(true, Ordering::Release);
                state.changed.notify_waiters();
            }
            return Response::new(Full::new(Bytes::from(
                serde_json::to_vec(&json!({"ext":{"warnings":{"fake_bidder":[{"code":10017}]}}}))
                    .unwrap(),
            )));
        }
        state.arrivals.lock().unwrap().insert(id);
        loop {
            let changed = state.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if state.released.load(Ordering::Acquire) {
                break;
            }
            changed.await;
        }
        state.completed.fetch_add(1, Ordering::AcqRel);
        let mut response = json!({"seatbid":[{"bid":[{"price":1_i32}]}]});
        if payload["ext"]["prebid"]["debug"] == true {
            response["ext"] = json!({"debug":{"httpcalls":{"fake_00":vec![json!({});4]}}});
        }
        Response::new(Full::new(Bytes::from(
            serde_json::to_vec(&response).unwrap(),
        )))
    }

    async fn application(
        serial: bool,
        bad_body: bool,
        lookup_releases: bool,
    ) -> (String, Fixture, JoinHandle<()>) {
        let state = Arc::new(State {
            arrivals: Mutex::new(BTreeSet::new()),
            wave: AtomicBool::new(false),
            released: AtomicBool::new(false),
            changed: Notify::new(),
            completed: AtomicU32::new(0),
            serial,
            bad_body,
            lookup_releases,
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let shared = Arc::clone(&state);
        let task = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            loop {
                let (stream, _peer) = listener.accept().await.unwrap();
                let current = Arc::clone(&shared);
                tasks.spawn(async move {
                    let _closed = http1::Builder::new()
                        .serve_connection(
                            TokioIo::new(stream),
                            service_fn(move |request| {
                                let route_state = Arc::clone(&current);
                                async move {
                                    Ok::<_, Infallible>(route(request, route_state).await)
                                }
                            }),
                        )
                        .await;
                });
                while tasks.try_join_next().is_some() {}
            }
        });
        (origin, Fixture(state), task)
    }

    fn load() -> Load {
        Load {
            concurrency: 2,
            calls: 4,
            request_bytes: 4096,
            nodes: 128,
            impressions: 1,
            encoded_bytes: 1024,
            decoded_bytes: 2048,
        }
    }
    fn expected() -> InstalledVariant {
        InstalledVariant {
            id: 1,
            dir: "unused".into(),
            config_sha256: Digest::of(b"config"),
            settings_sha256: Digest::of(b"settings"),
            nodes: 128,
            banner_sha256: Digest::of(b"q2_a"),
            store_sha256: Digest::of(b"store"),
        }
    }

    fn admission_evidence() -> Admission {
        Admission {
            proof: AdmissionProof {
                method: "parked_two_auctions_v1",
                application_concurrency: 2,
                second_constructed: true,
                first_response_started: false,
                probe_status: 503,
                probe_body_sha256: Digest::of(b"overloaded\n"),
                status_healthy: true,
                probe_requests: 2,
                first_calls: 16,
                second_calls: 16,
                resources_sha256: Digest::of(b"unit-resources"),
                review_sha256: Digest::of(b"unit-review"),
            },
            sha256: Digest::of(b"unit-admission-proof"),
            results: Vec::new(),
            stats: Stats {
                calls: 800,
                completed: 800,
                max_calls_per_auction: 16,
                max_encoded_bytes: 0x4000,
                max_decoded_bytes: 0x8000,
                max_active_auctions: 8,
                max_active_calls: 32,
                completed_auctions: 200,
                probe_first_calls: 16,
                probe_second_calls: 16,
                ..Stats::default()
            },
            duration_ms: 50,
        }
    }

    #[test]
    fn merge_stats_sums_counters_and_takes_each_maximum_without_cross_credit() {
        let left = Stats {
            calls: 11,
            completed: 7,
            fpd_expansions: 3,
            gzip_responses: 5,
            stalled_reads: 2,
            disconnected: 1,
            completed_auctions: 4,
            max_encoded_bytes: 300,
            max_decoded_bytes: 900,
            max_calls_per_auction: 3,
            max_active_auctions: 2,
            max_active_calls: 6,
            current_active_calls: 2,
            max_active_debug_auctions: 4,
            probe_first_calls: 1,
            probe_second_calls: 8,
        };
        let right = Stats {
            calls: 13,
            completed: 9,
            fpd_expansions: 2,
            gzip_responses: 7,
            stalled_reads: 1,
            disconnected: 4,
            completed_auctions: 3,
            max_encoded_bytes: 500,
            max_decoded_bytes: 700,
            max_calls_per_auction: 5,
            max_active_auctions: 1,
            max_active_calls: 8,
            current_active_calls: 0,
            max_active_debug_auctions: 2,
            probe_first_calls: 6,
            probe_second_calls: 5,
        };
        let expected_stats = Stats {
            calls: 24,
            completed: 16,
            fpd_expansions: 5,
            gzip_responses: 12,
            stalled_reads: 3,
            disconnected: 5,
            completed_auctions: 7,
            max_encoded_bytes: 500,
            max_decoded_bytes: 900,
            max_calls_per_auction: 5,
            max_active_auctions: 2,
            max_active_calls: 8,
            current_active_calls: 2,
            max_active_debug_auctions: 4,
            probe_first_calls: 6,
            probe_second_calls: 8,
        };
        let expected_value = serde_json::to_value(expected_stats).unwrap();
        for (first, second) in [(&left, &right), (&right, &left)] {
            assert_eq!(
                serde_json::to_value(merge_stats(first, second).unwrap()).unwrap(),
                expected_value
            );
        }
    }

    #[test]
    fn merge_stats_refuses_each_additive_counter_overflow_independently() {
        for field in [
            "calls",
            "completed",
            "fpd_expansions",
            "gzip_responses",
            "stalled_reads",
            "disconnected",
            "completed_auctions",
        ] {
            let mut maximum_value = serde_json::to_value(Stats::default()).unwrap();
            maximum_value[field] = json!(u32::MAX);
            let mut increment_value = serde_json::to_value(Stats::default()).unwrap();
            increment_value[field] = json!(1_u32);
            let maximum: Stats = serde_json::from_value(maximum_value).unwrap();
            let increment: Stats = serde_json::from_value(increment_value).unwrap();
            assert_eq!(
                merge_stats(&maximum, &increment).unwrap_err(),
                HarnessError::new("invalid_child_output")
            );
        }
    }

    #[test]
    fn attested_load_cannot_credit_admission_only_dispatch_or_decode() {
        let ordinary = Stats::default();
        let evidence = admission_evidence();
        let aggregated = merge_stats(&ordinary, &evidence.stats).unwrap();
        assert_eq!(aggregated.calls, 800);
        assert_eq!(aggregated.max_calls_per_auction, 16);
        assert_eq!(aggregated.max_encoded_bytes, 0x4000);
        assert_eq!(aggregated.max_decoded_bytes, 0x8000);
        let (achieved, simultaneous) = attested_load(load(), &ordinary, 0, &evidence);
        assert_eq!(
            achieved,
            Load {
                concurrency: 2,
                calls: 0,
                request_bytes: 0,
                nodes: 0,
                impressions: 0,
                encoded_bytes: 0,
                decoded_bytes: 0,
            }
        );
        assert!(simultaneous);
    }

    #[test]
    fn attested_load_requires_ordinary_decode_despite_dispatch_and_admission_responses() {
        let ordinary = Stats {
            calls: 2,
            max_calls_per_auction: 2,
            max_encoded_bytes: 257,
            max_decoded_bytes: 513,
            ..Stats::default()
        };
        let evidence = admission_evidence();
        let aggregated = merge_stats(&ordinary, &evidence.stats).unwrap();
        assert_eq!(aggregated.max_calls_per_auction, 16);
        assert_eq!(aggregated.max_encoded_bytes, 0x4000);
        assert_eq!(aggregated.max_decoded_bytes, 0x8000);
        let (without_decode, simultaneous) = attested_load(load(), &ordinary, 0, &evidence);
        assert_eq!(
            without_decode,
            Load {
                concurrency: 2,
                calls: 2,
                request_bytes: 4096,
                nodes: 128,
                impressions: 1,
                encoded_bytes: 0,
                decoded_bytes: 0,
            }
        );
        assert!(simultaneous);
        let (with_decode, decoded_simultaneous) = attested_load(load(), &ordinary, 1, &evidence);
        assert_eq!(
            with_decode,
            Load {
                concurrency: 2,
                calls: 2,
                request_bytes: 4096,
                nodes: 128,
                impressions: 1,
                encoded_bytes: 257,
                decoded_bytes: 513,
            }
        );
        assert!(decoded_simultaneous);
    }

    #[test]
    fn attested_load_concurrency_comes_from_the_proof_not_fixture_activity() {
        let ordinary = Stats {
            calls: 4,
            max_calls_per_auction: 4,
            max_active_auctions: 9,
            ..Stats::default()
        };
        let mut evidence = admission_evidence();
        evidence.proof.application_concurrency = 1;
        let (serialized, simultaneous) = attested_load(load(), &ordinary, 0, &evidence);
        assert_eq!(
            serialized,
            Load {
                concurrency: 1,
                calls: 4,
                request_bytes: 4096,
                nodes: 128,
                impressions: 1,
                encoded_bytes: 0,
                decoded_bytes: 0,
            }
        );
        assert!(!simultaneous);
        evidence.proof.application_concurrency = 2;
        let (concurrent, witnessed) = attested_load(load(), &ordinary, 0, &evidence);
        assert_eq!(concurrent.concurrency, 2);
        assert!(witnessed);
    }

    #[tokio::test]
    async fn two_primary_requests_are_constructed_without_headers_and_are_drained_after_both_challenges()
     {
        for debug in [false, true] {
            let (origin, mut fixture, server) = application(false, false, false).await;
            let supervisor = Supervisor::new();
            let context = Context {
                origin: &origin,
                load: load(),
                seed: 7,
                timeout_ms: 1500,
                supervisor: &supervisor,
            };
            let identity = Identity {
                resources_sha256: Digest::of(b"resources"),
                review_sha256: Digest::of(b"review"),
            };
            let pending_result =
                admission(&context, &mut fixture, &expected(), &identity, debug).await;
            server.abort();
            let _joined = server.await;
            supervisor.cleanup().await.unwrap();
            let result = pending_result.unwrap();
            assert_eq!(result.proof.application_concurrency, 2);
            assert_eq!(result.proof.probe_requests, 2);
            assert_eq!(result.proof.probe_status, 503);
            assert_eq!(result.proof.probe_body_sha256, Digest::of(b"overloaded\n"));
            assert!(result.proof.second_constructed);
            assert!(!result.proof.first_response_started);
            assert_eq!(result.proof.first_calls, 4);
            assert_eq!(result.proof.second_calls, 4);
            assert_eq!(result.results.len(), 2);
            assert_eq!(fixture.0.completed.load(Ordering::Acquire), 2);
            assert!(fixture.0.released.load(Ordering::Acquire));
        }
    }

    #[tokio::test]
    async fn serialized_construction_or_a_wrong_overload_body_refuses_and_drains_held_work() {
        for (serial, bad_body) in [(true, false), (false, true)] {
            let (origin, mut fixture, server) = application(serial, bad_body, false).await;
            let supervisor = Supervisor::new();
            let context = Context {
                origin: &origin,
                load: load(),
                seed: 7,
                timeout_ms: 1500,
                supervisor: &supervisor,
            };
            let identity = Identity {
                resources_sha256: Digest::of(b"resources"),
                review_sha256: Digest::of(b"review"),
            };
            let result = admission(&context, &mut fixture, &expected(), &identity, false).await;
            server.abort();
            let _joined = server.await;
            supervisor.cleanup().await.unwrap();
            let error = result.err().unwrap();
            assert_eq!(error.code, "admission_oracle_failed");
            assert_eq!(
                error.stage,
                Some(if serial {
                    "admission_wave_handoff"
                } else {
                    "admission_first_challenge"
                })
            );
            assert!(fixture.0.released.load(Ordering::Acquire));
            assert_eq!(fixture.0.completed.load(Ordering::Acquire), 2);
        }
    }

    #[tokio::test]
    async fn gvl_credit_requires_an_ordinary_request_to_remain_held_across_each_lookup() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let consent_fixtures = consents::fixtures(&root).unwrap();
        let first = &consent_fixtures[..1];
        for lookup_releases in [false, true] {
            let (origin, mut fixture, server) = application(false, false, lookup_releases).await;
            let supervisor = Supervisor::new();
            let context = Context {
                origin: &origin,
                load: load(),
                seed: 7,
                timeout_ms: 1500,
                supervisor: &supervisor,
            };
            let result = gvl_overlap(&context, &mut fixture, first, &mut BTreeSet::new()).await;
            server.abort();
            let _joined = server.await;
            supervisor.cleanup().await.unwrap();
            if lookup_releases {
                assert!(matches!(
                    result,
                    Err(HarnessError {
                        code: "gvl_overlap_failed",
                        ..
                    })
                ));
            } else {
                let overlap = result.unwrap();
                assert_eq!(overlap.parses, 1);
                assert_eq!(overlap.result.auction.bids, 1);
            }
            assert!(fixture.0.released.load(Ordering::Acquire));
            assert_eq!(fixture.0.completed.load(Ordering::Acquire), 1);
        }
    }
}

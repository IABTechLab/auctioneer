use super::super::{
    HarnessError, Result,
    fixture::{
        self,
        client::{Client, FixtureControl as _},
    },
    http::RequestSpec,
    meter::{self, NativeMeter},
    process::Supervisor,
};
use super::{
    consents,
    geometry::{self, AuctionOptions},
    inputs::{self, Candidate},
    load::{self, Category, Outcome},
    oracles,
    snapshots::InstalledVariant,
};
use crate::qualification::{profile, report};
use bytes::Bytes;
use futures::future::join;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::Instant;

pub struct Measured {
    pub observation: report::ScenarioObservation,
    pub window: meter::Window,
    pub attestation: serde_json::Value,
}

pub struct Context<'a> {
    pub candidate: &'a Candidate,
    pub scenario: &'a profile::Scenario,
    pub row: &'a inputs::Row,
    pub repetition: u8,
    pub origin: &'a str,
    pub fixture: &'a mut Client,
    pub meter: &'a NativeMeter,
    pub variant: &'a InstalledVariant,
    pub resources: &'a profile::ResourceExport,
    pub supervisor: &'a Supervisor,
    pub consents: &'a [consents::Consent],
    pub parsed_versions: &'a mut BTreeSet<u16>,
}

pub async fn run(mut context: Context<'_>) -> Result<Measured> {
    if context.row.subcase != context.scenario.subcase
        || context.row.variant_id != context.scenario.variant_id
        || context.variant.id != context.scenario.variant_id
        || context.repetition == 0
        || context.repetition > context.scenario.repetitions
    {
        return Err(HarnessError::new("invalid_input"));
    }
    let budget = Budget::new(
        context.row.subcase,
        context.scenario.load,
        context.candidate.max_requests,
        context.repetition,
        context.consents.len(),
    )?;
    let settings = fixture_settings(
        context.scenario.load,
        context.row.mode,
        context.row.subcase,
        context.candidate.seed,
        context.resources.limits.max_active_body_streams,
    )?;
    let warm = warmup(&mut context, &settings).await?;
    let oracle_context = oracles::Context {
        origin: context.origin,
        load: context.scenario.load,
        seed: context.candidate.seed,
        timeout_ms: context.candidate.request_timeout_ms,
        supervisor: context.supervisor,
    };
    let meter = context.meter;
    let duration_ms = context.scenario.duration_ms;
    let variant = context.variant;
    let max_requests = context.candidate.max_requests;
    let auxiliary_enabled = matches!(
        context.row.subcase,
        profile::Subcase::AuxiliaryOverlap
            | profile::Subcase::ConfigSnapshot
            | profile::Subcase::GvlMissOverlap
    );
    let stop = AtomicBool::new(false);
    let main = async {
        let measured = Box::pin(meter.window(
            duration_ms,
            measured_work(&mut context, &settings, &budget, &oracle_context),
        ))
        .await;
        // Store after every window result, including errors, before joining aux.
        stop.store(true, Ordering::Release);
        measured
    };
    let auxiliary_future = auxiliary_work(
        &oracle_context,
        variant,
        &stop,
        max_requests,
        auxiliary_enabled,
    );
    let (measured, auxiliary_result) = Box::pin(join(main, auxiliary_future)).await;
    let (window, work) = measured?;
    finalize(
        context,
        warm.duration_ms,
        window,
        work,
        &budget,
        auxiliary_result?,
    )
    .await
}

async fn warmup(
    context: &mut Context<'_>,
    settings: &fixture::Settings,
) -> Result<load::LoadResult> {
    let candidate = context.candidate;
    let scenario = context.scenario;
    oracles::idle(context.fixture, context.supervisor).await?;
    context.fixture.configure(settings).await?;
    let warm = load::run(
        context.origin,
        load::Options {
            duration_ms: scenario.warmup_ms,
            workers: scenario.load.concurrency,
            max_requests: candidate.max_requests,
            timeout_ms: candidate.request_timeout_ms,
            pace: false,
        },
        context.supervisor,
        |id| {
            auction_request(
                candidate.base_load,
                AuctionOptions {
                    id,
                    ..AuctionOptions::default()
                },
                candidate.request_timeout_ms,
            )
        },
    )
    .await?;
    oracles::idle(context.fixture, context.supervisor).await?;
    context.fixture.configure(settings).await?;
    Ok(warm)
}

async fn measured_work(
    context: &mut Context<'_>,
    settings: &fixture::Settings,
    budget: &Budget,
    oracle_context: &oracles::Context<'_>,
) -> Result<Work> {
    let candidate = context.candidate;
    let scenario = context.scenario;
    let subcase = context.row.subcase;
    let mut counters = Counters::new();
    let resources_sha256 = profile::Digest::canonical(context.resources)
        .map_err(|_error| HarnessError::new("invalid_input"))?;
    let identity = oracles::Identity {
        review_sha256: profile::Digest::canonical(&json!({
            "method": candidate.admission_oracle,
            "fixture": scenario.fixture,
            "resources_sha256": resources_sha256,
            "source_bound_finalization": true,
        }))
        .map_err(|_error| HarnessError::new("invalid_input"))?,
        resources_sha256,
    };
    let admission = oracles::admission(
        oracle_context,
        context.fixture,
        context.variant,
        &identity,
        scenario.debug_workload.is_some(),
    )
    .await?;
    for result in &admission.results {
        counters.record(result, subcase)?;
    }
    oracles::idle(context.fixture, context.supervisor).await?;
    let overlap = if budget.gvl_workers != 0 {
        let selected = context
            .consents
            .get(budget.consent_range.clone())
            .ok_or(HarnessError::new("reference_unavailable"))?;
        let overlap = oracles::gvl_overlap(
            oracle_context,
            context.fixture,
            selected,
            context.parsed_versions,
        )
        .await?;
        add_counter(&mut counters.effects.gvl_parses, overlap.parses)?;
        counters.record(&overlap.result, subcase)?;
        Some(overlap)
    } else {
        None
    };
    oracles::idle(context.fixture, context.supervisor).await?;
    context.fixture.configure(settings).await?;
    let mut result = load::run(
        context.origin,
        load::Options {
            duration_ms: scenario.duration_ms,
            workers: budget.ordinary_workers,
            max_requests: budget.ordinary_max_requests,
            timeout_ms: candidate.request_timeout_ms,
            pace: true,
        },
        context.supervisor,
        |id| ordinary_request(scenario.load, subcase, id, candidate.request_timeout_ms),
    )
    .await?;
    let mut ordinary = Ordinary::default();
    for (id, outcome) in &result.results {
        counters.record(outcome, subcase)?;
        ordinary.record(*id, outcome, subcase)?;
    }
    let coverage = context
        .fixture
        .coverage(&ordinary.successful_ids, scenario.load.calls)
        .await?;
    ordinary.validate_coverage(&coverage)?;
    let mut primaries = admission.results.clone();
    let mut elapsed_ms = admission.duration_ms;
    if let Some(held) = &overlap {
        primaries.push(held.result.clone());
        elapsed_ms = elapsed_ms
            .checked_add(held.duration_ms)
            .ok_or(HarnessError::new("invalid_input"))?;
    }
    load::merge(&mut result, &primaries, elapsed_ms)?;
    Ok(Work {
        result,
        admission,
        overlap,
        ordinary,
        coverage,
        counters,
    })
}

async fn finalize(
    mut context: Context<'_>,
    warmup_ms: u64,
    window: meter::Window,
    mut work: Work,
    budget: &Budget,
    auxiliary: Auxiliary,
) -> Result<Measured> {
    let scenario = context.scenario;
    add_counter(
        &mut work.counters.effects.auxiliary_requests,
        auxiliary.requests,
    )?;
    add_counter(
        &mut work.counters.effects.config_extractions,
        auxiliary.configs,
    )?;
    oracles::idle(context.fixture, context.supervisor).await?;
    let ordinary_stats = context.fixture.stats().await?;
    let ordinary_decoded =
        work.ordinary
            .reconcile(&work.coverage, &ordinary_stats, scenario.load)?;
    let oracle_stats = if let Some(overlap) = &work.overlap {
        oracles::merge_stats(&work.admission.stats, &overlap.stats)?
    } else {
        work.admission.stats.clone()
    };
    let stats = oracles::merge_stats(&ordinary_stats, &oracle_stats)?;
    work.counters.fixture_effects(
        context.row.subcase,
        &ordinary_stats,
        &stats,
        ordinary_decoded,
        context.resources.limits.max_active_body_streams,
    )?;
    let recovered = recover(&mut context, &mut work.counters).await?;
    let debug = work.counters.finish_debug(
        scenario
            .debug_workload
            .as_ref()
            .map(|workload| &workload.reference_sha256),
        work.admission.proof.application_concurrency,
        &work.admission.results,
    )?;
    let (achieved, simultaneous) = oracles::attested_load(
        scenario.load,
        &ordinary_stats,
        ordinary_decoded,
        &work.admission,
    );
    let attestation = attestation(&stats, &ordinary_stats, &work, budget)?;
    Ok(Measured {
        observation: report::ScenarioObservation {
            effects: work.counters.effects,
            subcase: scenario.subcase,
            variant_id: scenario.variant_id,
            repetition: context.repetition,
            fixture: scenario.fixture.clone(),
            seed: scenario.seed,
            warmup_ms,
            duration_ms: window.elapsed_ms,
            load_window_ms: work.result.load_window_ms,
            achieved,
            simultaneous,
            outcomes: work.result.outcomes,
            latency: work.result.latency,
            memory: None,
            recovered,
            debug,
        },
        window,
        attestation,
    })
}

async fn recover(context: &mut Context<'_>, counters: &mut Counters) -> Result<bool> {
    let candidate = context.candidate;
    let status = load::call(
        context.origin,
        "/status",
        RequestSpec::default(),
        context.supervisor,
    )
    .await?;
    let observed = oracles::status_observed(&status, context.variant);
    if observed {
        add_counter(&mut counters.effects.config_extractions, 1)?;
    }
    context
        .fixture
        .configure(&oracles::settings(
            candidate.base_load,
            fixture::Mode::Gzip,
            candidate.seed,
            10,
            0,
        ))
        .await?;
    let recovery = load::call(
        context.origin,
        "/openrtb2/auction",
        auction_request(
            candidate.base_load,
            AuctionOptions {
                id: 999_997,
                ..AuctionOptions::default()
            },
            1000,
        )?,
        context.supervisor,
    )
    .await?;
    let recovered = recovery.category == Category::Success && recovery.auction.bids > 0 && observed;
    counters.effects.recovery_successes = u64::from(recovered);
    Ok(recovered)
}

struct Work {
    result: load::LoadResult,
    admission: oracles::Admission,
    overlap: Option<oracles::Overlap>,
    ordinary: Ordinary,
    coverage: fixture::Coverage,
    counters: Counters,
}

#[derive(Clone, Copy, Default)]
struct Auxiliary {
    requests: u64,
    configs: u64,
}

async fn auxiliary_work(
    context: &oracles::Context<'_>,
    variant: &InstalledVariant,
    stop: &AtomicBool,
    max_requests: u32,
    enabled: bool,
) -> Result<Auxiliary> {
    let mut counts = Auxiliary::default();
    if enabled {
        for index in 0..max_requests {
            if stop.load(Ordering::Acquire) {
                break;
            }
            let path = match index.checked_rem(3) {
                Some(0) => "/status",
                Some(1) => "/info/bidders",
                _ => "/getuids",
            };
            let outcome = load::call(
                context.origin,
                path,
                RequestSpec {
                    timeout_ms: context.timeout_ms,
                    ..RequestSpec::default()
                },
                context.supervisor,
            )
            .await?;
            if outcome.category == Category::Success {
                add_counter(&mut counts.requests, 1)?;
            }
            if path == "/status" && oracles::status_observed(&outcome, variant) {
                add_counter(&mut counts.configs, 1)?;
            }
            let next = Instant::now()
                .checked_add(Duration::from_millis(25))
                .ok_or(HarnessError::new("invalid_input"))?;
            load::pause_until(next, context.supervisor).await?;
        }
    }
    Ok(counts)
}

fn attestation(
    stats: &fixture::Stats,
    ordinary: &fixture::Stats,
    work: &Work,
    budget: &Budget,
) -> Result<Value> {
    let mut value =
        serde_json::to_value(stats).map_err(|_error| HarnessError::new("invalid_child_output"))?;
    let fields = value
        .as_object_mut()
        .ok_or(HarnessError::new("invalid_child_output"))?;
    fields.insert(
        "ordinary_dispatch".to_owned(),
        json!({
            "successful_auctions": work.ordinary.successful_ids.len(), "calls": ordinary.calls,
        }),
    );
    fields.insert(
        "gvl_requests".to_owned(),
        json!(work.overlap.as_ref().map_or(0, |overlap| overlap.parses)),
    );
    fields.insert(
        "worker_budget".to_owned(),
        json!({ "ordinary": budget.ordinary_workers, "gvl": budget.gvl_workers }),
    );
    let mut proof = serde_json::to_value(&work.admission.proof)
        .map_err(|_error| HarnessError::new("invalid_child_output"))?;
    proof
        .as_object_mut()
        .ok_or(HarnessError::new("invalid_child_output"))?
        .insert("sha256".to_owned(), json!(work.admission.sha256));
    fields.insert("admission_oracle".to_owned(), proof);
    Ok(value)
}

struct Budget {
    ordinary_workers: u32,
    gvl_workers: u32,
    ordinary_max_requests: u32,
    consent_range: Range<usize>,
}

impl Budget {
    fn new(
        subcase: profile::Subcase,
        load: profile::Load,
        max_requests: u32,
        repetition: u8,
        consent_count: usize,
    ) -> Result<Self> {
        if repetition == 0 || !(1..=10_000).contains(&max_requests) {
            return Err(HarnessError::new("invalid_input"));
        }
        let gvl_workers = u32::from(matches!(
            subcase,
            profile::Subcase::GvlColdParse | profile::Subcase::GvlMissOverlap
        ));
        let ordinary_workers = if gvl_workers != 0 {
            load.concurrency.checked_sub(1)
        } else if subcase == profile::Subcase::ExcessAdmission {
            load.concurrency.checked_mul(2)
        } else {
            Some(load.concurrency)
        }
        .filter(|workers| (1..=32).contains(workers))
        .ok_or(HarnessError::new("invalid_input"))?;
        let ordinary_max_requests = max_requests
            .checked_sub(2)
            .and_then(|requests| requests.checked_sub(gvl_workers))
            .filter(|requests| *requests != 0)
            .ok_or(HarnessError::new("invalid_input"))?;
        let repetition_index = usize::from(
            repetition
                .checked_sub(1)
                .ok_or(HarnessError::new("invalid_input"))?,
        );
        let consent_range = if subcase == profile::Subcase::GvlColdParse {
            repetition_index..usize::from(repetition)
        } else if subcase == profile::Subcase::GvlMissOverlap {
            let start = repetition_index
                .checked_mul(6)
                .and_then(|index| index.checked_add(2))
                .ok_or(HarnessError::new("invalid_input"))?;
            let end = start
                .checked_add(6)
                .ok_or(HarnessError::new("invalid_input"))?;
            start..end
        } else {
            0..0
        };
        if consent_range.end > consent_count {
            return Err(HarnessError::new("reference_unavailable"));
        }
        Ok(Self {
            ordinary_workers,
            gvl_workers,
            ordinary_max_requests,
            consent_range,
        })
    }
}

#[derive(Default)]
struct DebugRange {
    auctions: u64,
    min_call_rows: u32,
    max_call_rows: u32,
    min_capture_bytes: u64,
    max_capture_bytes: u64,
    min_output_bytes: u64,
    max_output_bytes: u64,
}

struct Counters {
    effects: profile::Effects,
    debug: DebugRange,
}

impl Counters {
    fn new() -> Self {
        Self {
            effects: profile::Effects {
                gvl_parses: 0,
                fpd_expansions: 0,
                decoded_responses: 0,
                slow_reads: 0,
                debug_captures: 0,
                debug_denials: 0,
                debug_output_bytes: 0,
                auxiliary_requests: 0,
                config_extractions: 0,
                queued_calls: 0,
                cancellations: 0,
                stalled_reads: 0,
                recovery_successes: 0,
            },
            debug: DebugRange::default(),
        }
    }

    fn record(&mut self, outcome: &Outcome, subcase: profile::Subcase) -> Result<()> {
        let auction = &outcome.auction;
        add_counter(
            &mut self.effects.debug_captures,
            u64::from(auction.debug_captures),
        )?;
        add_counter(
            &mut self.effects.debug_denials,
            u64::from(auction.debug_denials),
        )?;
        add_counter(
            &mut self.effects.debug_output_bytes,
            auction.debug_output_bytes,
        )?;
        add_counter(&mut self.effects.slow_reads, u64::from(outcome.slow_reads))?;
        if subcase == profile::Subcase::Cancellation && outcome.category == Category::Disconnect {
            add_counter(&mut self.effects.cancellations, 1)?;
        }
        if auction.debug_output_bytes > 0 {
            let range = &mut self.debug;
            if range.auctions == 0 {
                range.min_call_rows = auction.debug_captures;
                range.min_capture_bytes = auction.capture_bytes;
                range.min_output_bytes = auction.debug_output_bytes;
            }
            add_counter(&mut range.auctions, 1)?;
            range.min_call_rows = range.min_call_rows.min(auction.debug_captures);
            range.max_call_rows = range.max_call_rows.max(auction.debug_captures);
            range.min_capture_bytes = range.min_capture_bytes.min(auction.capture_bytes);
            range.max_capture_bytes = range.max_capture_bytes.max(auction.capture_bytes);
            range.min_output_bytes = range.min_output_bytes.min(auction.debug_output_bytes);
            range.max_output_bytes = range.max_output_bytes.max(auction.debug_output_bytes);
        }
        Ok(())
    }

    fn finish_debug(
        &self,
        reference: Option<&profile::Digest>,
        simultaneous: u32,
        oracle_results: &[Outcome],
    ) -> Result<Option<report::DebugObservation>> {
        let Some(reference_sha256) = reference.filter(|_reference| self.debug.auctions > 0) else {
            return Ok(None);
        };
        if simultaneous != 2 || oracle_results.len() != 2 {
            return Err(HarnessError::new("admission_oracle_failed"));
        }
        let minimum_rows = oracle_results
            .iter()
            .map(|row| row.auction.debug_captures)
            .min()
            .ok_or(HarnessError::new("admission_oracle_failed"))?;
        let simultaneous_finalized_calls = u64::from(simultaneous)
            .checked_mul(u64::from(minimum_rows))
            .ok_or(HarnessError::new("invalid_child_output"))?;
        Ok(Some(report::DebugObservation {
            reference_sha256: reference_sha256.clone(),
            auctions: self.debug.auctions,
            simultaneous_auctions: simultaneous,
            simultaneous_finalized_calls,
            min_call_rows: self.debug.min_call_rows,
            max_call_rows: self.debug.max_call_rows,
            capture_bytes: profile::ByteBounds {
                min: self.debug.min_capture_bytes,
                max: self.debug.max_capture_bytes,
            },
            output_bytes: profile::ByteBounds {
                min: self.debug.min_output_bytes,
                max: self.debug.max_output_bytes,
            },
        }))
    }

    fn fixture_effects(
        &mut self,
        subcase: profile::Subcase,
        ordinary: &fixture::Stats,
        combined: &fixture::Stats,
        ordinary_decoded: u64,
        active_streams: u32,
    ) -> Result<()> {
        if active_streams == 0 || ordinary.gzip_responses > ordinary.calls {
            return Err(HarnessError::new("invalid_child_output"));
        }
        self.effects.fpd_expansions = u64::from(combined.fpd_expansions);
        self.effects.decoded_responses = ordinary_decoded.min(u64::from(ordinary.gzip_responses));
        self.effects.stalled_reads = u64::from(combined.stalled_reads);
        // Later fixture arrivals prove queued work, not an SDK scheduler count.
        self.effects.queued_calls = if subcase == profile::Subcase::OutboundQueue {
            u64::from(
                combined
                    .max_calls_per_auction
                    .saturating_sub(active_streams),
            )
        } else {
            0
        };
        Ok(())
    }
}

#[derive(Default)]
struct Ordinary {
    successful_ids: Vec<u32>,
    offered: u64,
    admitted: u64,
    decoded: u64,
    invalid: bool,
}

impl Ordinary {
    fn record(&mut self, id: u32, outcome: &Outcome, subcase: profile::Subcase) -> Result<()> {
        add_counter(&mut self.offered, 1)?;
        if outcome.category != Category::Rejected {
            add_counter(&mut self.admitted, 1)?;
        }
        add_counter(&mut self.decoded, u64::from(outcome.auction.bids))?;
        if outcome.category == Category::Success {
            self.invalid |= outcome.auction.bids == 0;
            self.successful_ids.push(actual_id(subcase, id)?);
        }
        Ok(())
    }

    fn validate_coverage(&self, coverage: &fixture::Coverage) -> Result<()> {
        let count = u32::try_from(self.successful_ids.len())
            .map_err(|_error| HarnessError::new("workload_underload"))?;
        let unique = self.successful_ids.iter().copied().collect::<BTreeSet<_>>();
        // The shipped fixture rejects repeated IDs and echoes their count.
        // Check both again at this deserialized child-protocol boundary rather
        // than trusting that a future fixture still implements those invariants.
        if self.invalid
            || self.offered == 0
            || self.admitted == 0
            || unique.len() != self.successful_ids.len()
            || coverage.auctions != count
            || coverage.complete_auctions != count
        {
            return Err(HarnessError::new("workload_underload"));
        }
        Ok(())
    }

    fn reconcile(
        &self,
        coverage: &fixture::Coverage,
        stats: &fixture::Stats,
        load: profile::Load,
    ) -> Result<u64> {
        self.validate_coverage(coverage)?;
        let minimum_calls = coverage
            .auctions
            .checked_mul(load.calls)
            .ok_or(HarnessError::new("workload_underload"))?;
        let decoded = self.decoded.min(u64::from(stats.gzip_responses));
        if decoded == 0
            || stats.calls == 0
            || stats.calls < minimum_calls
            || stats.max_calls_per_auction != load.calls
        {
            return Err(HarnessError::new("workload_underload"));
        }
        Ok(decoded)
    }
}

const fn requested_debug(subcase: profile::Subcase) -> bool {
    matches!(
        subcase,
        profile::Subcase::Denied
            | profile::Subcase::Permitted
            | profile::Subcase::ConcurrentCapture
    )
}

fn actual_id(subcase: profile::Subcase, id: u32) -> Result<u32> {
    let actual = if requested_debug(subcase) {
        id.checked_add(700_000)
            .ok_or(HarnessError::new("invalid_input"))?
    } else {
        id
    };
    if id == 0 || actual > 99_999_999 {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(actual)
}

fn auction_request(
    load: profile::Load,
    options: AuctionOptions<'_>,
    timeout_ms: u64,
) -> Result<RequestSpec> {
    Ok(RequestSpec {
        method: ::http::Method::POST,
        body: Bytes::from(geometry::make_auction(load, options)?),
        timeout_ms,
        ..RequestSpec::default()
    })
}

fn ordinary_request(
    load: profile::Load,
    subcase: profile::Subcase,
    id: u32,
    timeout_ms: u64,
) -> Result<RequestSpec> {
    let debug = requested_debug(subcase);
    let mut request = auction_request(
        load,
        AuctionOptions {
            id: actual_id(subcase, id)?,
            fpd: subcase == profile::Subcase::FpdExpansion,
            debug,
            dense_debug: debug,
            ..AuctionOptions::default()
        },
        timeout_ms,
    )?;
    if subcase == profile::Subcase::Denied {
        let mut value: Value = serde_json::from_slice(&request.body)
            .map_err(|_error| HarnessError::new("unreachable_shape"))?;
        let publisher = value
            .pointer_mut("/site/publisher/id")
            .filter(|publisher_id| publisher_id.as_str() == Some("q2_allowed"))
            .ok_or(HarnessError::new("unreachable_shape"))?;
        *publisher = Value::String("q2_refused".to_owned());
        let bytes =
            serde_json::to_vec(&value).map_err(|_error| HarnessError::new("unreachable_shape"))?;
        if bytes.len() != request.body.len() {
            return Err(HarnessError::new("unreachable_shape"));
        }
        request.body = Bytes::from(bytes);
    }
    if u64::try_from(request.body.len()).ok() != Some(load.request_bytes)
        || auctioneer_runtime::count_json_nodes(&request.body, load.nodes).ok() != Some(load.nodes)
    {
        return Err(HarnessError::new("unreachable_shape"));
    }
    request.slow_read_ms = if subcase == profile::Subcase::SlowReader {
        100
    } else {
        0
    };
    request.disconnect_ms = if subcase == profile::Subcase::Cancellation && !id.is_multiple_of(3) {
        30
    } else {
        0
    };
    Ok(request)
}

fn fixture_settings(
    load: profile::Load,
    mode: fixture::Mode,
    subcase: profile::Subcase,
    seed: u32,
    active_streams: u32,
) -> Result<fixture::Settings> {
    if active_streams == 0 {
        return Err(HarnessError::new("invalid_input"));
    }
    let (effective_mode, delay_ms) = if subcase == profile::Subcase::Cancellation {
        (fixture::Mode::Cancel, 200)
    } else {
        (mode, 10)
    };
    Ok(oracles::settings(
        load,
        effective_mode,
        seed,
        delay_ms,
        active_streams,
    ))
}

fn add_counter(counter: &mut u64, amount: u64) -> Result<()> {
    *counter = counter
        .checked_add(amount)
        .ok_or(HarnessError::new("invalid_child_output"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::geometry::{self, AuctionOptions};
    use super::*;
    use load::{Auction, Category, ConfigStatus};
    use profile::{ByteBounds, Digest, Subcase};
    use serde_json::{Value, json};

    fn shape() -> profile::Load {
        profile::Load {
            concurrency: 2,
            calls: 16,
            request_bytes: 0x8000_u64,
            nodes: 2048,
            impressions: 8,
            encoded_bytes: 0x4000_u64,
            decoded_bytes: 0x8000_u64,
        }
    }

    fn outcome(category: Category, bids: u32) -> Outcome {
        Outcome {
            category,
            status: if category == Category::Rejected {
                503
            } else {
                200
            },
            latency_us: 1234,
            slow_reads: 0,
            body_sha256: Digest::of(b"safe numeric fixture"),
            config_status: ConfigStatus::Ok,
            auction: Auction {
                bids,
                ..Auction::default()
            },
        }
    }

    fn debug(rows: u32, capture: u64, output: u64) -> Outcome {
        let mut row = outcome(Category::Success, 1);
        row.auction.debug_captures = rows;
        row.auction.capture_bytes = capture;
        row.auction.debug_output_bytes = output;
        row
    }

    fn complete(auctions: u32) -> fixture::Coverage {
        fixture::Coverage {
            auctions,
            complete_auctions: auctions,
        }
    }

    fn dispatched() -> fixture::Stats {
        fixture::Stats {
            calls: 32,
            gzip_responses: 32,
            max_calls_per_auction: 16,
            max_encoded_bytes: 0x4000_u64,
            max_decoded_bytes: 0x8000_u64,
            ..fixture::Stats::default()
        }
    }

    fn ordinary_pair() -> Ordinary {
        let mut ordinary = Ordinary::default();
        ordinary
            .record(1, &outcome(Category::Success, 16), Subcase::MaximumShape)
            .unwrap();
        ordinary
            .record(4, &outcome(Category::Success, 16), Subcase::MaximumShape)
            .unwrap();
        ordinary
    }

    #[test]
    fn outcome_counters_accumulate_all_primary_work_without_inventing_cancellations() {
        let mut counters = Counters::new();
        let mut first = debug(4, 19, 101);
        first.auction.debug_denials = 2;
        first.slow_reads = 3;
        counters.record(&first, Subcase::Permitted).unwrap();
        let mut second = debug(7, 31, 209);
        second.slow_reads = 5;
        counters.record(&second, Subcase::Permitted).unwrap();
        counters
            .record(&outcome(Category::Disconnect, 0), Subcase::Cancellation)
            .unwrap();
        counters
            .record(&outcome(Category::Disconnect, 0), Subcase::SlowReader)
            .unwrap();
        counters
            .record(&outcome(Category::Timeout, 0), Subcase::Cancellation)
            .unwrap();
        assert_eq!(counters.effects.debug_captures, 11);
        assert_eq!(counters.effects.debug_denials, 2);
        assert_eq!(counters.effects.debug_output_bytes, 310);
        assert_eq!(counters.effects.slow_reads, 8);
        assert_eq!(counters.effects.cancellations, 1);
        assert_eq!(counters.effects.decoded_responses, 0);
        assert_eq!(counters.effects.gvl_parses, 0);
    }

    #[test]
    fn outcome_counters_refuse_overflow_instead_of_saturating() {
        for field in [
            "captures",
            "denials",
            "output",
            "reads",
            "cancellations",
            "auctions",
        ] {
            let mut counters = Counters::new();
            let mut row = debug(1, 1, 1);
            row.auction.debug_denials = 1;
            row.slow_reads = 1;
            row.category = Category::Disconnect;
            match field {
                "captures" => counters.effects.debug_captures = u64::MAX,
                "denials" => counters.effects.debug_denials = u64::MAX,
                "output" => counters.effects.debug_output_bytes = u64::MAX,
                "reads" => counters.effects.slow_reads = u64::MAX,
                "cancellations" => counters.effects.cancellations = u64::MAX,
                _ => counters.debug.auctions = u64::MAX,
            }
            assert!(
                counters.record(&row, Subcase::Cancellation).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn debug_ranges_count_only_actual_outputs_and_bind_two_finalized_oracle_populations() {
        let reference = Digest::of(b"reviewed debug reference");
        let oracle = [debug(7, 40, 200), debug(3, 20, 120)];
        let mut counters = Counters::new();
        counters
            .record(&oracle[0], Subcase::ConcurrentCapture)
            .unwrap();
        counters
            .record(&oracle[1], Subcase::ConcurrentCapture)
            .unwrap();
        counters
            .record(&debug(5, 30, 160), Subcase::ConcurrentCapture)
            .unwrap();
        counters
            .record(&debug(0, 0, 0), Subcase::ConcurrentCapture)
            .unwrap();
        let observed = counters
            .finish_debug(Some(&reference), 2, &oracle)
            .unwrap()
            .unwrap();
        assert_eq!(observed.reference_sha256, reference);
        assert_eq!(observed.auctions, 3);
        assert_eq!(observed.min_call_rows, 3);
        assert_eq!(observed.max_call_rows, 7);
        assert_eq!(observed.capture_bytes, ByteBounds { min: 20, max: 40 });
        assert_eq!(observed.output_bytes, ByteBounds { min: 120, max: 200 });
        assert_eq!(observed.simultaneous_auctions, 2);
        assert_eq!(observed.simultaneous_finalized_calls, 6);
        assert!(counters.finish_debug(None, 2, &oracle).unwrap().is_none());
        assert!(
            Counters::new()
                .finish_debug(Some(&reference), 2, &oracle)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn capture_release_keeps_oracle_ranges_when_ordinary_outputs_are_absent() {
        let reference = Digest::of(b"capture release reference");
        let oracle = [debug(8, 80, 180), debug(8, 90, 190)];
        let mut counters = Counters::new();
        for row in &oracle {
            counters.record(row, Subcase::CaptureRelease).unwrap();
        }
        for _ in 0_u8..3_u8 {
            counters
                .record(&outcome(Category::Success, 16), Subcase::CaptureRelease)
                .unwrap();
        }
        let observed = counters
            .finish_debug(Some(&reference), 2, &oracle)
            .unwrap()
            .unwrap();
        assert_eq!(observed.auctions, 2);
        assert_eq!(observed.min_call_rows, 8);
        assert_eq!(observed.output_bytes, ByteBounds { min: 180, max: 190 });
        assert_eq!(observed.simultaneous_finalized_calls, 16);
        assert_eq!(counters.effects.debug_output_bytes, 370);
    }

    #[test]
    fn denied_request_changes_only_the_publisher_and_preserves_byte_and_node_geometry() {
        let allowed = geometry::make_auction(
            shape(),
            AuctionOptions {
                id: 700_009,
                debug: true,
                dense_debug: true,
                ..AuctionOptions::default()
            },
        )
        .unwrap();
        let request = ordinary_request(shape(), Subcase::Denied, 9, 876).unwrap();
        assert_eq!(request.method, ::http::Method::POST);
        assert_eq!(request.timeout_ms, 876);
        assert_eq!(allowed.len(), 0x8000_usize);
        assert_eq!(request.body.len(), 0x8000_usize);
        assert_eq!(
            auctioneer_runtime::count_json_nodes(&request.body, 2048).unwrap(),
            2048
        );
        let mut expected: Value = serde_json::from_slice(&allowed).unwrap();
        assert_eq!(expected["site"]["publisher"]["id"], "q2_allowed");
        expected["site"]["publisher"]["id"] = json!("q2_refused");
        let actual: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual["id"], "q700009");
        assert_eq!(
            actual["imp"][0]["banner"]["format"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
    }

    #[test]
    fn ordinary_debug_is_requested_only_for_denied_permitted_and_concurrent_capture() {
        for (subcase, requested) in [
            (Subcase::Denied, true),
            (Subcase::Permitted, true),
            (Subcase::ConcurrentCapture, true),
            (Subcase::CaptureRelease, false),
            (Subcase::Unrequested, false),
            (Subcase::MaximumShape, false),
        ] {
            let request = ordinary_request(shape(), subcase, 11, 500).unwrap();
            let value: Value = serde_json::from_slice(&request.body).unwrap();
            assert_eq!(value["id"], if requested { "q700011" } else { "q11" });
            assert_eq!(value["ext"]["prebid"]["debug"], requested);
            assert_eq!(value["imp"][0]["banner"]["format"].is_array(), requested);
            assert_eq!(request.body.len(), 0x8000_usize);
            assert_eq!(
                auctioneer_runtime::count_json_nodes(&request.body, 2048).unwrap(),
                2048
            );
        }
    }

    #[test]
    fn ordinary_request_preserves_fpd_slow_read_and_cancellation_policies() {
        let fpd = ordinary_request(shape(), Subcase::FpdExpansion, 2, 999).unwrap();
        let value: Value = serde_json::from_slice(&fpd.body).unwrap();
        assert_eq!(
            value["imp"][0]["ext"]["prebid"]["imp"]
                .as_object()
                .unwrap()
                .len(),
            16
        );
        assert_eq!(value["ext"]["prebid"]["debug"], false);
        let slow = ordinary_request(shape(), Subcase::SlowReader, 2, 999).unwrap();
        assert_eq!(slow.slow_read_ms, 100);
        assert_eq!(slow.disconnect_ms, 0);
        for (id, disconnect) in [(1, 30), (2, 30), (3, 0), (6, 0)] {
            let request = ordinary_request(shape(), Subcase::Cancellation, id, 999).unwrap();
            assert_eq!(request.disconnect_ms, disconnect);
            assert_eq!(request.slow_read_ms, 0);
            assert_eq!(request.timeout_ms, 999);
        }
    }

    #[test]
    fn fixture_stalls_hold_only_the_final_funded_wave_and_queue_credit_is_a_lower_bound() {
        let settings =
            fixture_settings(shape(), fixture::Mode::Stall, Subcase::OutboundQueue, 7, 4).unwrap();
        assert!(matches!(settings.mode, fixture::Mode::Stall));
        assert_eq!(settings.stall_after_calls, 12);
        assert_eq!(settings.delay_ms, 10);
        assert_eq!(settings.seed, 7);
        let cancellation =
            fixture_settings(shape(), fixture::Mode::Stall, Subcase::Cancellation, 7, 4).unwrap();
        assert!(matches!(cancellation.mode, fixture::Mode::Cancel));
        assert_eq!(cancellation.delay_ms, 200);
        assert_eq!(cancellation.stall_after_calls, 0);
        let under_wave = fixture_settings(
            profile::Load {
                calls: 3,
                ..shape()
            },
            fixture::Mode::Stall,
            Subcase::StalledIo,
            7,
            4,
        )
        .unwrap();
        assert_eq!(under_wave.stall_after_calls, 0);
        let ordinary = fixture::Stats {
            gzip_responses: 3,
            ..dispatched()
        };
        let combined = fixture::Stats {
            fpd_expansions: 19,
            stalled_reads: 6,
            gzip_responses: 99,
            ..dispatched()
        };
        let mut counters = Counters::new();
        counters
            .fixture_effects(Subcase::OutboundQueue, &ordinary, &combined, 7, 4)
            .unwrap();
        assert_eq!(counters.effects.queued_calls, 12);
        assert_eq!(counters.effects.decoded_responses, 3);
        assert_eq!(counters.effects.fpd_expansions, 19);
        assert_eq!(counters.effects.stalled_reads, 6);
        counters
            .fixture_effects(Subcase::StalledIo, &ordinary, &combined, 7, 4)
            .unwrap();
        assert_eq!(counters.effects.queued_calls, 0);
        counters
            .fixture_effects(
                Subcase::OutboundQueue,
                &ordinary,
                &fixture::Stats {
                    max_calls_per_auction: 3,
                    ..combined
                },
                7,
                4,
            )
            .unwrap();
        assert_eq!(counters.effects.queued_calls, 0);
    }

    #[test]
    fn ordinary_successes_require_bids_and_exact_coverage_of_actual_debug_ids() {
        let mut ordinary = Ordinary::default();
        ordinary
            .record(2, &outcome(Category::Rejected, 0), Subcase::Permitted)
            .unwrap();
        ordinary
            .record(3, &outcome(Category::Success, 16), Subcase::Permitted)
            .unwrap();
        ordinary
            .record(8, &outcome(Category::Success, 16), Subcase::Permitted)
            .unwrap();
        assert_eq!(ordinary.successful_ids, vec![700_003, 700_008]);
        assert_eq!(ordinary.decoded, 32);
        assert_eq!(
            ordinary
                .reconcile(&complete(2), &dispatched(), shape())
                .unwrap(),
            32
        );
        for coverage in [
            complete(1),
            complete(3),
            fixture::Coverage {
                auctions: 2,
                complete_auctions: 1,
            },
        ] {
            assert_eq!(
                ordinary
                    .reconcile(&coverage, &dispatched(), shape())
                    .unwrap_err(),
                HarnessError::new("workload_underload")
            );
        }
        ordinary
            .record(9, &outcome(Category::Success, 0), Subcase::Permitted)
            .unwrap();
        assert_eq!(
            ordinary
                .reconcile(
                    &complete(3),
                    &fixture::Stats {
                        calls: 48,
                        gzip_responses: 48,
                        ..dispatched()
                    },
                    shape(),
                )
                .unwrap_err(),
            HarnessError::new("workload_underload")
        );
    }

    #[test]
    fn a_successful_empty_auction_cannot_borrow_sibling_decode_credit() {
        for (bids, expected) in [
            (0, Err(HarnessError::new("workload_underload"))),
            (1, Ok(33)),
        ] {
            let mut ordinary = ordinary_pair();
            ordinary
                .record(9, &outcome(Category::Success, bids), Subcase::MaximumShape)
                .unwrap();
            assert_eq!(ordinary.successful_ids, vec![1, 4, 9]);
            // Three complete auctions have all 48 calls; siblings supply decode
            // credit, so only the empty successful auction can explain refusal.
            let stats = fixture::Stats {
                calls: 48,
                gzip_responses: 48,
                ..dispatched()
            };
            assert_eq!(ordinary.reconcile(&complete(3), &stats, shape()), expected);
        }
    }

    #[test]
    fn deserialized_child_coverage_must_match_the_requested_id_count() {
        let ordinary = ordinary_pair();
        assert_eq!(ordinary.validate_coverage(&complete(2)), Ok(()));
        for (auctions, complete_auctions) in [(1_u32, 2_u32), (3, 2), (2, 1), (2, 3)] {
            let coverage: fixture::Coverage = serde_json::from_value(json!({
                "auctions":auctions,"complete_auctions":complete_auctions
            }))
            .unwrap();
            assert_eq!(
                ordinary.validate_coverage(&coverage),
                Err(HarnessError::new("workload_underload"))
            );
        }
    }

    #[test]
    fn repeated_success_ids_cannot_be_validated_by_a_claimed_complete_child_reply() {
        let mut ordinary = ordinary_pair();
        assert_eq!(ordinary.validate_coverage(&complete(2)), Ok(()));
        ordinary.successful_ids[1] = ordinary.successful_ids[0];
        assert_eq!(
            ordinary.validate_coverage(&complete(2)),
            Err(HarnessError::new("workload_underload"))
        );
    }

    #[test]
    fn timeout_ordinary_bids_allow_zero_success_ids_with_exact_empty_coverage() {
        for subcase in [Subcase::StalledIo, Subcase::Timeout] {
            let mut ordinary = Ordinary::default();
            ordinary
                .record(1, &outcome(Category::Timeout, 16), subcase)
                .unwrap();
            ordinary
                .record(2, &outcome(Category::Timeout, 8), subcase)
                .unwrap();
            assert!(ordinary.successful_ids.is_empty());
            assert_eq!(ordinary.decoded, 24);
            ordinary.validate_coverage(&complete(0)).unwrap();
            assert_eq!(
                ordinary
                    .reconcile(&complete(0), &dispatched(), shape())
                    .unwrap(),
                24
            );
            assert_eq!(
                ordinary
                    .reconcile(&complete(1), &dispatched(), shape())
                    .unwrap_err(),
                HarnessError::new("workload_underload")
            );
        }
    }

    #[test]
    fn empty_ordinary_population_cannot_borrow_oracle_dispatch_or_decode() {
        for decoded in [0, 32] {
            let empty = Ordinary {
                decoded,
                ..Ordinary::default()
            };
            assert_eq!(
                empty
                    .reconcile(&complete(0), &dispatched(), shape())
                    .unwrap_err(),
                HarnessError::new("workload_underload")
            );
        }
    }

    #[test]
    fn all_rejected_ordinary_population_cannot_borrow_oracle_credit() {
        for bids in [0, 16] {
            let mut ordinary = Ordinary::default();
            ordinary
                .record(1, &outcome(Category::Rejected, bids), Subcase::MaximumShape)
                .unwrap();
            ordinary
                .record(2, &outcome(Category::Rejected, bids), Subcase::MaximumShape)
                .unwrap();
            assert!(ordinary.successful_ids.is_empty());
            assert_eq!(
                ordinary
                    .reconcile(&complete(0), &dispatched(), shape())
                    .unwrap_err(),
                HarnessError::new("workload_underload")
            );
        }
    }

    #[test]
    fn zero_success_ordinary_work_requires_independent_dispatch_and_decode() {
        let mut ordinary = Ordinary::default();
        ordinary
            .record(1, &outcome(Category::Timeout, 16), Subcase::Timeout)
            .unwrap();
        for stats in [
            fixture::Stats {
                calls: 0,
                ..dispatched()
            },
            fixture::Stats {
                gzip_responses: 0,
                ..dispatched()
            },
            fixture::Stats {
                max_calls_per_auction: 2,
                ..dispatched()
            },
            fixture::Stats {
                max_calls_per_auction: 15,
                ..dispatched()
            },
            fixture::Stats {
                max_calls_per_auction: 17,
                ..dispatched()
            },
        ] {
            assert_eq!(
                ordinary
                    .reconcile(&complete(0), &stats, shape())
                    .unwrap_err(),
                HarnessError::new("workload_underload")
            );
        }
        let mut undecoded = Ordinary::default();
        undecoded
            .record(1, &outcome(Category::Timeout, 0), Subcase::Timeout)
            .unwrap();
        assert_eq!(
            undecoded
                .reconcile(&complete(0), &dispatched(), shape())
                .unwrap_err(),
            HarnessError::new("workload_underload")
        );
    }

    #[test]
    fn successful_ordinary_work_requires_exact_dispatch_and_decode_evidence() {
        let ordinary = ordinary_pair();
        for stats in [
            fixture::Stats::default(),
            fixture::Stats {
                gzip_responses: 0,
                ..dispatched()
            },
            fixture::Stats {
                max_calls_per_auction: 2,
                ..dispatched()
            },
            fixture::Stats {
                max_calls_per_auction: 15,
                ..dispatched()
            },
            fixture::Stats {
                max_calls_per_auction: 17,
                ..dispatched()
            },
            fixture::Stats {
                calls: 31,
                ..dispatched()
            },
        ] {
            assert_eq!(
                ordinary
                    .reconcile(&complete(2), &stats, shape())
                    .unwrap_err(),
                HarnessError::new("workload_underload")
            );
        }
        assert_eq!(
            ordinary
                .reconcile(
                    &complete(2),
                    &fixture::Stats {
                        gzip_responses: 3,
                        ..dispatched()
                    },
                    shape()
                )
                .unwrap(),
            3
        );
    }

    #[test]
    fn ordinary_coverage_retains_dispatched_ids_even_when_the_load_stops_early() {
        let mut ordinary = Ordinary::default();
        ordinary
            .record(1, &outcome(Category::Timeout, 0), Subcase::CaptureRelease)
            .unwrap();
        ordinary
            .record(4, &outcome(Category::Success, 16), Subcase::CaptureRelease)
            .unwrap();
        ordinary
            .record(9, &outcome(Category::Success, 16), Subcase::CaptureRelease)
            .unwrap();
        assert_eq!(ordinary.successful_ids, vec![4, 9]);
        assert_eq!(
            ordinary
                .reconcile(&complete(2), &dispatched(), shape())
                .unwrap(),
            32
        );
    }

    #[test]
    fn ordinary_bid_accumulation_and_actual_id_conversion_are_checked() {
        let mut overflowing = Ordinary {
            decoded: u64::MAX,
            ..Ordinary::default()
        };
        assert!(
            overflowing
                .record(1, &outcome(Category::Success, 1), Subcase::MaximumShape)
                .is_err()
        );
        assert!(ordinary_request(shape(), Subcase::Permitted, u32::MAX, 500).is_err());
        let mut ordinary = Ordinary::default();
        assert!(
            ordinary
                .record(u32::MAX, &outcome(Category::Success, 1), Subcase::Permitted)
                .is_err()
        );
    }

    #[test]
    fn worker_and_consent_budgets_reserve_primaries_without_increasing_application_population() {
        for (subcase, workers, gvl, requests, range) in [
            (Subcase::MaximumShape, 2, 0, 62, 0..0),
            (Subcase::ExcessAdmission, 4, 0, 62, 0..0),
            (Subcase::GvlColdParse, 1, 1, 61, 0..1),
            (Subcase::GvlMissOverlap, 1, 1, 61, 2..8),
        ] {
            let budget = Budget::new(subcase, shape(), 64, 1, 14).unwrap();
            assert_eq!(budget.ordinary_workers, workers);
            assert_eq!(budget.gvl_workers, gvl);
            assert_eq!(budget.ordinary_max_requests, requests);
            assert_eq!(budget.consent_range, range);
        }
        assert_eq!(
            Budget::new(Subcase::GvlColdParse, shape(), 64, 2, 14)
                .unwrap()
                .consent_range,
            1..2
        );
        assert_eq!(
            Budget::new(Subcase::GvlMissOverlap, shape(), 64, 2, 14)
                .unwrap()
                .consent_range,
            8..14
        );
        let mut indices = BTreeSet::<usize>::new();
        for range in [
            0_usize..1_usize,
            1_usize..2_usize,
            2_usize..8_usize,
            8_usize..14_usize,
        ] {
            for index in range {
                assert!(indices.insert(index));
            }
        }
        assert_eq!(indices, (0_usize..14_usize).collect::<BTreeSet<_>>());
    }

    #[test]
    fn invalid_budgets_refuse_underflow_zero_workers_and_missing_consent_rows() {
        for (subcase, concurrency, max_requests, repetition, consents) in [
            (Subcase::MaximumShape, 2, 2, 1, 14),
            (Subcase::GvlColdParse, 2, 3, 1, 14),
            (Subcase::GvlColdParse, 1, 64, 1, 14),
            (Subcase::GvlColdParse, 2, 64, 0, 14),
            (Subcase::GvlMissOverlap, 2, 64, 2, 13),
            (Subcase::GvlColdParse, 2, 64, 2, 1),
            (Subcase::ExcessAdmission, u32::MAX, 64, 1, 14),
        ] {
            assert!(
                Budget::new(
                    subcase,
                    profile::Load {
                        concurrency,
                        ..shape()
                    },
                    max_requests,
                    repetition,
                    consents
                )
                .is_err()
            );
        }
    }
}

use super::Error;
use super::profile::{
    Backend, Digest, Feature, MaximumMode, Meter, Population, Profile, Scenario, Scope, Subcase,
};
use super::report::{
    Accounted, EvidenceStatus, Latency, Memory, MeterLimitation, Observations, PeakMethod,
    Qualification, Reason, Report, ResultStatus, ScenarioObservation, ScenarioResult,
    VariantObservation,
};

pub fn evaluate(
    profile: Profile,
    observations: Observations,
    profile_sha256: Digest,
) -> Result<Report, Error> {
    let accounted = account(&profile)?;
    let mut local = ResultStatus::Pass;
    let mut reasons = Vec::new();
    if !accounted.fits {
        record(
            &mut local,
            &mut reasons,
            ResultStatus::Fail,
            Reason::EnvelopeOverBudget,
        );
    }
    if observations.profile_sha256 != profile_sha256
        || observations.identity != profile.identity
        || observations.resources != profile.resources
        || observations.hardware != profile.hardware
        || observations.platform != profile.platform
        || observations.backend != profile.backend
    {
        record(
            &mut local,
            &mut reasons,
            ResultStatus::Fail,
            Reason::IdentityMismatch,
        );
    }
    check_variants(&profile, &observations, &mut local, &mut reasons);
    let mut scenarios = Vec::new();
    for scenario in &profile.scenarios {
        for repetition in 1..=scenario.repetitions {
            let matching: Vec<_> = observations
                .scenarios
                .iter()
                .filter(|o| {
                    o.subcase == scenario.subcase
                        && o.variant_id == scenario.variant_id
                        && o.repetition == repetition
                })
                .collect();
            let (status, reason) = match matching.as_slice() {
                [] => (ResultStatus::Pending, Reason::MissingEvidence),
                [row] => check_scenario(&profile, scenario, row),
                _ => (ResultStatus::Fail, Reason::DuplicateEvidence),
            };
            local = combine(local, status);
            scenarios.push(ScenarioResult {
                subcase: scenario.subcase,
                variant_id: scenario.variant_id,
                repetition,
                status,
                reason,
            });
        }
    }
    if observations.scenarios.iter().any(|o| {
        !profile.scenarios.iter().any(|s| {
            s.subcase == o.subcase
                && s.variant_id == o.variant_id
                && (1..=s.repetitions).contains(&o.repetition)
        })
    }) {
        record(
            &mut local,
            &mut reasons,
            ResultStatus::Fail,
            Reason::UnexpectedEvidence,
        );
    }
    let overall = qualification(combine(local, check_capabilities(&profile, &observations)));
    let publisher_pilot_ready =
        overall == Qualification::Qualified && profile.scope == Scope::Deployment;
    Ok(Report {
        schema_version: super::profile::RESOURCE_SCHEMA_VERSION,
        profile_sha256,
        profile,
        observations,
        accounted,
        scenarios,
        local_acceptance: local,
        profile_qualification: overall,
        publisher_pilot_ready,
        reasons,
    })
}

fn account(profile: &Profile) -> Result<Accounted, Error> {
    let derived = profile.derive()?;
    let resources = &profile.resources;
    let platform_bytes = profile
        .platform
        .memory_bytes
        .min(resources.limits.isolate_total_bytes);
    let fits = derived
        .total_bytes
        .checked_add(profile.headroom_bytes)
        .is_some_and(|total| total <= platform_bytes);
    Ok(Accounted {
        total_bytes: derived.total_bytes,
        catalog_resident_term: derived.catalog_resident_term,
        catalog_initialization_term: derived.catalog_initialization_term,
        schema_validation_term: derived.schema_validation_term,
        catalog_context_term: derived.catalog_context_term,
        debug_per_auction_bytes: derived.debug_term,
        per_auction_bytes: derived.per_auction_peak,
        baked_config_bytes: resources.baked_config_bytes,
        platform_bytes,
        required_headroom_bytes: profile.headroom_bytes,
        effective_profile_sha256: resources.digest()?,
        fits,
    })
}

fn check_capabilities(profile: &Profile, observations: &Observations) -> ResultStatus {
    let mut capabilities = ResultStatus::Pass;
    for required in &profile.required_capabilities {
        let rows: Vec<_> = observations
            .capabilities
            .iter()
            .filter(|c| c.capability == *required)
            .collect();
        let status = match rows.as_slice() {
            [] => ResultStatus::Pending,
            [row] => match row.status {
                EvidenceStatus::Verified if row.evidence_sha256.is_some() => ResultStatus::Pass,
                EvidenceStatus::Verified | EvidenceStatus::Pending => ResultStatus::Pending,
                EvidenceStatus::Unsupported => ResultStatus::Unsupported,
                EvidenceStatus::Failed => ResultStatus::Fail,
            },
            _ => ResultStatus::Fail,
        };
        capabilities = combine(capabilities, status);
    }
    if profile.identity.build_features.iter().any(|feature| {
        matches!(
            feature,
            Feature::InsecureBidders | Feature::ConformanceSuite | Feature::TestHarnessOverride
        )
    }) {
        capabilities = combine(capabilities, ResultStatus::Unsupported);
    }
    capabilities
}

fn qualification(status: ResultStatus) -> Qualification {
    match status {
        ResultStatus::Pass => Qualification::Qualified,
        ResultStatus::Fail => Qualification::Nonqualified,
        ResultStatus::Pending => Qualification::Pending,
        ResultStatus::Unsupported => Qualification::Unsupported,
    }
}

fn combine(left: ResultStatus, right: ResultStatus) -> ResultStatus {
    match (left, right) {
        (ResultStatus::Fail, _) | (_, ResultStatus::Fail) => ResultStatus::Fail,
        (ResultStatus::Unsupported, _) | (_, ResultStatus::Unsupported) => {
            ResultStatus::Unsupported
        }
        (ResultStatus::Pending, _) | (_, ResultStatus::Pending) => ResultStatus::Pending,
        _ => ResultStatus::Pass,
    }
}

fn record(
    local: &mut ResultStatus,
    reasons: &mut Vec<Reason>,
    status: ResultStatus,
    reason: Reason,
) {
    *local = combine(*local, status);
    reasons.push(reason);
}

fn check_variants(
    profile: &Profile,
    observations: &Observations,
    local: &mut ResultStatus,
    reasons: &mut Vec<Reason>,
) {
    for variant in &profile.variants {
        let rows: Vec<_> = observations
            .variants
            .iter()
            .filter(|o| o.id == variant.id)
            .collect();
        match rows.as_slice() {
            [] => record(
                local,
                reasons,
                ResultStatus::Pending,
                Reason::MissingEvidence,
            ),
            [row] => {
                let cold_min = variant.cold_window_ms;
                let warm_min = variant.warm_window_ms;
                let reused_startup = observations
                    .variants
                    .iter()
                    .any(|v| v.id != row.id && v.startup_sha256 == row.startup_sha256);
                if row.config_sha256 != variant.config_sha256
                    || row.startup_snapshot_sha256 != variant.config_sha256
                    || row.settings_sha256 != variant.settings_sha256
                    || !row.effective_settings_observed
                    || row.cold_window_ms < cold_min
                    || row.warm_window_ms < warm_min
                    || (profile.backend == Backend::StartupSnapshot
                        && (!row.restarted
                            || !row.input_immutable
                            || row.process_id == 0
                            || reused_startup))
                {
                    record(local, reasons, ResultStatus::Fail, Reason::StartupEvidence);
                }
                for (memory, min_window, max_window, max_peak) in [
                    (
                        &row.cold_memory,
                        cold_min,
                        row.cold_window_ms,
                        variant.max_cold_peak_bytes,
                    ),
                    (
                        &row.warm_memory,
                        warm_min,
                        row.warm_window_ms,
                        variant.max_warm_peak_bytes,
                    ),
                ] {
                    let (status, reason) = check_memory(
                        profile,
                        memory.as_ref(),
                        min_window,
                        max_window,
                        max_peak,
                        max_peak,
                    );
                    if status != ResultStatus::Pass {
                        record(local, reasons, status, reason);
                    }
                }
                if !lifetime_peaks_agree(profile, observations, row) {
                    record(local, reasons, ResultStatus::Fail, Reason::PeakUnsupported);
                }
            }
            _ => record(
                local,
                reasons,
                ResultStatus::Fail,
                Reason::DuplicateEvidence,
            ),
        }
    }
    if observations
        .variants
        .iter()
        .any(|v| !profile.variants.iter().any(|p| p.id == v.id))
    {
        record(
            local,
            reasons,
            ResultStatus::Fail,
            Reason::UnexpectedEvidence,
        );
    }
}

fn lifetime_peaks_agree(
    profile: &Profile,
    observations: &Observations,
    variant: &VariantObservation,
) -> bool {
    // Cold, warm, then the predeclared scenario/repetition schedule share this
    // startup's lifetime meter. Evidence array order is not execution order.
    let scenario_memory = profile
        .scenarios
        .iter()
        .filter(|s| s.variant_id == variant.id)
        .flat_map(|s| {
            (1..=s.repetitions).filter_map(move |repetition| {
                observations.scenarios.iter().find(|row| {
                    row.variant_id == s.variant_id
                        && row.subcase == s.subcase
                        && row.repetition == repetition
                })
            })
        })
        .filter_map(|row| row.memory.as_ref());
    let mut previous = 0;
    variant
        .cold_memory
        .iter()
        .chain(variant.warm_memory.iter())
        .chain(scenario_memory)
        .filter(|memory| {
            memory.source == Meter::ProcessRss
                && memory.method == PeakMethod::HighWater
                && memory.limitation == MeterLimitation::ProcessLifetimeHighWater
        })
        .all(|memory| {
            let monotonic = memory.peak_bytes >= previous;
            previous = memory.peak_bytes;
            monotonic
        })
}

fn check_scenario(
    profile: &Profile,
    scenario: &Scenario,
    row: &ScenarioObservation,
) -> (ResultStatus, Reason) {
    if row.fixture != scenario.fixture || row.seed != scenario.seed {
        return (ResultStatus::Fail, Reason::IdentityMismatch);
    }
    if row.warmup_ms < scenario.warmup_ms
        || row.duration_ms < scenario.duration_ms
        || row.load_window_ms < scenario.duration_ms
        || row.load_window_ms > row.duration_ms
    {
        return (ResultStatus::Fail, Reason::WindowShortened);
    }
    let load = row.achieved;
    let planned = scenario.load;
    let limits = profile.resources.limits;
    if load.concurrency < planned.concurrency
        || load.calls < planned.calls
        || load.request_bytes < planned.request_bytes
        || load.nodes < planned.nodes
        || load.impressions < planned.impressions
        || load.encoded_bytes < planned.encoded_bytes
        || load.decoded_bytes < planned.decoded_bytes
        || load.concurrency > limits.max_concurrent_auctions
        || load.calls > limits.max_outbound_requests
        || load.request_bytes > limits.inbound_request_bytes
        || load.nodes > limits.max_parsed_nodes
        || load.impressions > limits.max_impressions_per_auction
        || load.encoded_bytes > limits.max_transport_bytes
        || load.decoded_bytes > limits.max_decoded_bytes
        || (!row.simultaneous && scenario.maximum_mode != MaximumMode::ReviewedNonSimultaneous)
        || !row.effects.meets(scenario.effects)
        || !debug_agrees(scenario, row)
        || (matches!(scenario.subcase, Subcase::Unrequested | Subcase::Denied)
            && (row.effects.debug_captures != 0 || row.effects.debug_output_bytes != 0))
    {
        return (ResultStatus::Fail, Reason::Underload);
    }
    let c = &row.outcomes;
    let bounds = &scenario.outcomes;
    if c.admitted.checked_add(c.rejected) != Some(c.offered)
        || c.success
            .checked_add(c.errors)
            .and_then(|n| n.checked_add(c.timeouts))
            != Some(c.admitted)
        || c.offered < bounds.min_offered
        || c.admitted < bounds.min_admitted
        || c.success < bounds.min_success
        || c.rejected < bounds.min_rejected
        || c.errors < bounds.min_errors
        || c.timeouts < bounds.min_timeouts
        || c.rejected > bounds.max_rejected
        || c.errors > bounds.max_errors
        || c.timeouts > bounds.max_timeouts
        || !populations_agree(row)
    {
        return (ResultStatus::Fail, Reason::OutcomeBounds);
    }
    let latency = &row.latency;
    let expected = &scenario.latency;
    let population = match expected.population {
        Population::AllOffered => c.offered,
        Population::AllAdmitted => c.admitted,
        Population::SuccessOnly => 0,
    };
    if latency.start != expected.start
        || latency.end != expected.end
        || latency.population != expected.population
        || latency.percentile != expected.percentile
        || latency.samples != population
        || latency.samples == 0
        || latency.histogram.len() > usize::from(expected.max_histogram_buckets)
    {
        return (ResultStatus::Fail, Reason::LatencyPopulation);
    }
    let Some(percentile_us) = histogram_percentile(latency) else {
        return (ResultStatus::Fail, Reason::LatencyPopulation);
    };
    if latency.value_us != percentile_us || percentile_us > expected.threshold_us {
        return (ResultStatus::Fail, Reason::LatencyExceeded);
    }
    let memory_result = check_memory(
        profile,
        row.memory.as_ref(),
        scenario.meter_window_ms,
        row.duration_ms,
        scenario.max_peak_bytes,
        scenario.max_retained_growth_bytes,
    );
    if memory_result.0 != ResultStatus::Pass {
        return memory_result;
    }
    if !row.recovered {
        return (ResultStatus::Fail, Reason::RecoveryFailed);
    }
    (ResultStatus::Pass, Reason::Accepted)
}

fn debug_agrees(scenario: &Scenario, row: &ScenarioObservation) -> bool {
    let Some(workload) = &scenario.debug_workload else {
        return row.debug.is_none();
    };
    let Some(debug) = &row.debug else {
        return false;
    };
    let r = &workload.reference;
    let aggregate_within = |actual, min, max| {
        debug
            .auctions
            .checked_mul(min)
            .is_some_and(|lower| actual >= lower)
            && debug
                .auctions
                .checked_mul(max)
                .is_none_or(|upper| actual <= upper)
    };
    debug.reference_sha256 == workload.reference_sha256
        && debug.auctions >= u64::from(scenario.load.concurrency)
        && debug.auctions <= row.outcomes.admitted
        && (scenario.subcase == Subcase::CaptureRelease || debug.auctions == row.outcomes.admitted)
        && debug.simultaneous_auctions >= scenario.load.concurrency
        && debug.simultaneous_auctions <= row.achieved.concurrency
        && u64::from(debug.simultaneous_auctions) <= debug.auctions
        && debug.simultaneous_finalized_calls
            >= u64::from(scenario.load.concurrency).saturating_mul(u64::from(
                scenario
                    .load
                    .calls
                    .min(auctioneer_runtime::MAX_DEBUG_CALL_ROWS),
            ))
        && debug.simultaneous_finalized_calls
            <= u64::from(debug.simultaneous_auctions).saturating_mul(u64::from(
                row.achieved
                    .calls
                    .min(auctioneer_runtime::MAX_DEBUG_CALL_ROWS),
            ))
        && debug.min_call_rows >= r.min_call_rows
        && debug.min_call_rows <= debug.max_call_rows
        && debug.max_call_rows <= r.max_call_rows
        && debug.capture_bytes.within(r.capture_bytes)
        && debug.output_bytes.within(r.output_bytes)
        && aggregate_within(
            row.effects.debug_captures,
            u64::from(debug.min_call_rows),
            u64::from(debug.max_call_rows),
        )
        && aggregate_within(
            row.effects.debug_output_bytes,
            debug.output_bytes.min,
            debug.output_bytes.max,
        )
}

fn populations_agree(row: &ScenarioObservation) -> bool {
    let admitted = row.outcomes.admitted;
    let calls = u64::from(row.achieved.calls);
    let effects = row.effects;
    let within = |count, per_auction| {
        admitted
            .checked_mul(per_auction)
            .is_none_or(|upper| count <= upper)
    };
    let fpd_per_auction = calls
        .checked_mul(u64::from(row.achieved.impressions))
        .and_then(|work| work.checked_mul(u64::from(row.achieved.nodes)));
    u64::from(row.achieved.concurrency) <= admitted
        && within(effects.decoded_responses, calls)
        && within(
            effects.debug_captures,
            calls.min(u64::from(auctioneer_runtime::MAX_DEBUG_CALL_ROWS)),
        )
        && within(
            effects.debug_output_bytes,
            auctioneer_runtime::MAX_DEBUG_SERIALIZED_BYTES,
        )
        && within(effects.debug_denials, calls.saturating_add(1))
        && within(effects.queued_calls, calls)
        && within(effects.cancellations, calls.saturating_add(1))
        && fpd_per_auction.is_none_or(|work| within(effects.fpd_expansions, work))
}

fn histogram_percentile(latency: &Latency) -> Option<u64> {
    if latency.histogram.is_empty() || latency.histogram.len() > 32 {
        return None;
    }
    let rank = latency
        .samples
        .checked_mul(u64::from(latency.percentile))?
        .div_ceil(100);
    let mut previous = 0_u64;
    let mut count = 0_u64;
    let mut percentile = None;
    for bucket in &latency.histogram {
        if bucket.upper_us <= previous {
            return None;
        }
        previous = bucket.upper_us;
        count = count.checked_add(bucket.count)?;
        if count >= rank && percentile.is_none() {
            percentile = Some(bucket.upper_us);
        }
    }
    if count == latency.samples {
        percentile
    } else {
        None
    }
}

fn check_memory(
    profile: &Profile,
    observed_memory: Option<&Memory>,
    min_window: u64,
    max_window: u64,
    max_peak: u64,
    max_growth: u64,
) -> (ResultStatus, Reason) {
    let Some(memory) = observed_memory else {
        return (ResultStatus::Pending, Reason::PeakMissing);
    };
    if memory.method == PeakMethod::Unsupported {
        return (ResultStatus::Unsupported, Reason::PeakUnsupported);
    }
    let limitation = match memory.source {
        Meter::ProcessRss => MeterLimitation::ProcessLifetimeHighWater,
        Meter::Allocator => MeterLimitation::RequestedAllocationsOnly,
        Meter::WasmGuest => MeterLimitation::GuestLinearMemoryOnly,
        Meter::Provider => MeterLimitation::ProviderReportedOnly,
    };
    if memory.source != profile.hardware.meter
        || memory.method != PeakMethod::HighWater
        || memory.limitation != limitation
        || !memory.service_only
        || memory.window_ms < min_window
        || memory.window_ms > max_window
    {
        return (ResultStatus::Fail, Reason::PeakUnsupported);
    }
    if memory.peak_bytes == 0
        || memory.peak_bytes > max_peak
        || memory
            .peak_bytes
            .checked_add(profile.headroom_bytes)
            .is_none_or(|n| {
                n > profile
                    .platform
                    .memory_bytes
                    .min(profile.resources.limits.isolate_total_bytes)
            })
        || memory.retained_growth_bytes > max_growth
    {
        return (ResultStatus::Fail, Reason::PeakExceeded);
    }
    (ResultStatus::Pass, Reason::Accepted)
}

#[cfg(test)]
pub mod tests {
    use super::super::profile::tests::{candidate, candidate_with_call_limit, digest};
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::iter::repeat_n;

    pub(crate) fn observations(p: &Value) -> Value {
        let mut rows: Vec<_> = p["scenarios"].as_array().unwrap().iter().map(|s| json!({
            "subcase":s["subcase"],"variant_id":s["variant_id"],"repetition":1_i32,
            "fixture":s["fixture"],"seed":s["seed"],"warmup_ms":10_i32,"duration_ms":100_i32,
            "load_window_ms":100_i32,"achieved":s["load"],"effects":s["effects"],"simultaneous":true,
            "outcomes":{"offered":10_i32,"admitted":10_i32,"success":10_i32,"rejected":0_i32,"errors":0_i32,"timeouts":0_i32},
            "latency":{"start":"client_dispatch","end":"terminal_outcome","population":"all_offered",
                "percentile":99_i32,"samples":10_i32,"value_us":1_000_i32},
            "memory":{"source":"process_rss","method":"high_water","units":"bytes","window_ms":100_i32,
                "peak_bytes":1_000_000_i32,"retained_growth_bytes":0_i32,"service_only":true},"recovered":true,"debug":null
        })).collect();
        let mut variants: Vec<_> = p["variants"].as_array().unwrap().iter().map(|v| json!({
            "id":v["id"],"process_id":v["id"],"startup_sha256":format!("{:064x}", v["id"].as_u64().unwrap()),
            "config_sha256":v["config_sha256"],"startup_snapshot_sha256":v["config_sha256"],
            "settings_sha256":v["settings_sha256"],"restarted":true,"input_immutable":true,
            "effective_settings_observed":true,"cold_window_ms":100_i32,"warm_window_ms":100_i32
        })).collect();
        for v in &mut variants {
            let memory = json!({"source":"process_rss","method":"high_water","units":"bytes","window_ms":100_i32,
                "peak_bytes":1_000_000_i32,"retained_growth_bytes":0_i32,"service_only":true,"limitation":"process_lifetime_high_water"});
            v["cold_memory"] = memory.clone();
            v["warm_memory"] = memory;
        }
        for (row, s) in rows.iter_mut().zip(p["scenarios"].as_array().unwrap()) {
            row["memory"]["limitation"] = json!("process_lifetime_high_water");
            let rejected = s["outcomes"]["min_rejected"].as_u64().unwrap();
            let errors = s["outcomes"]["min_errors"].as_u64().unwrap();
            let timeouts = s["outcomes"]["min_timeouts"].as_u64().unwrap();
            row["outcomes"] = json!({"offered":10_u64.saturating_add(rejected),"admitted":10_i32,"success":10_u64.saturating_sub(errors).saturating_sub(timeouts),
                "rejected":rejected,"errors":errors,"timeouts":timeouts});
            row["latency"]["samples"] = json!(10_u64.saturating_add(rejected));
            row["latency"]["histogram"] =
                json!([{"upper_us":1_000_u64,"count":10_u64.saturating_add(rejected)}]);
            if let Some(workload) = s.get("debug_workload").filter(|value| !value.is_null()) {
                let r = &workload["reference"];
                row["debug"] = json!({"reference_sha256":workload["reference_sha256"],"auctions":10_u64,
                    "simultaneous_auctions":s["load"]["concurrency"],"simultaneous_finalized_calls":s["load"]["concurrency"].as_u64().unwrap().saturating_mul(s["load"]["calls"].as_u64().unwrap().min(64)),
                    "min_call_rows":r["min_call_rows"],"max_call_rows":r["max_call_rows"],"capture_bytes":r["capture_bytes"],"output_bytes":r["output_bytes"]});
                row["effects"]["debug_captures"] =
                    json!(10_u64.saturating_mul(r["min_call_rows"].as_u64().unwrap()));
                row["effects"]["debug_output_bytes"] =
                    json!(10_u64.saturating_mul(r["output_bytes"]["min"].as_u64().unwrap()));
            }
        }
        let extra_rows: Vec<_> = rows
            .iter()
            .zip(p["scenarios"].as_array().unwrap())
            .flat_map(|(row, s)| {
                (2..=s["repetitions"].as_u64().unwrap()).map(|repetition| {
                    let mut repeated = row.clone();
                    repeated["repetition"] = json!(repetition);
                    repeated
                })
            })
            .collect();
        rows.extend(extra_rows);
        let capabilities: Vec<_> = p["required_capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                json!({
                    "capability":c,"status":"verified","evidence_sha256":digest()
                })
            })
            .collect();
        json!({"schema_version":2,"profile_sha256":hex::encode(Sha256::digest(serde_json::to_vec(p).unwrap())),
            "identity":p["identity"],"resources":p["resources"],"hardware":p["hardware"],"platform":p["platform"],
            "backend":p["backend"],"variants":variants,"capabilities":capabilities,"scenarios":rows})
    }

    fn checked(p: &Value, o: &Value) -> Value {
        let bytes = serde_json::to_vec(p).unwrap();
        let profile = super::super::profile::parse(&bytes).unwrap();
        let observations = super::super::report::parse(&serde_json::to_vec(o).unwrap()).unwrap();
        serde_json::to_value(
            super::evaluate(profile, observations, super::Digest::of(&bytes)).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn qualification_complete_matrix_qualifies() {
        let p = candidate();
        let r = checked(&p, &observations(&p));
        assert_eq!(r["local_acceptance"], "pass");
        assert_eq!(r["profile_qualification"], "qualified");
        assert!(r["accounted"]["debug_per_auction_bytes"].as_u64().unwrap() > 0);
    }

    #[test]
    fn qualification_latency_threshold_is_inclusive_and_independent_of_reported_value() {
        let p = candidate();
        for (percentile, reported, status, reason) in [
            (9_999_u64, 9_999_u64, "pass", "accepted"),
            (10_000, 10_000, "pass", "accepted"),
            (10_001, 10_001, "fail", "latency_exceeded"),
            (1_000, 999, "fail", "latency_exceeded"),
            (1_000, 1_001, "fail", "latency_exceeded"),
        ] {
            let mut o = observations(&p);
            o["scenarios"][0]["latency"]["value_us"] = json!(reported);
            o["scenarios"][0]["latency"]["histogram"][0]["upper_us"] = json!(percentile);
            let r = checked(&p, &o);
            assert_eq!(r["local_acceptance"], status, "{percentile}/{reported}");
            assert_eq!(r["scenarios"][0]["reason"], reason);
            assert_eq!(
                r["profile_qualification"],
                if status == "pass" {
                    "qualified"
                } else {
                    "nonqualified"
                }
            );
            assert_eq!(r["publisher_pilot_ready"], status == "pass");
            assert_eq!(r["reasons"], json!([]));
        }
    }

    #[test]
    fn qualification_refuses_each_unplanned_scenario_identity() {
        let p = candidate();
        for (field, value) in [("variant_id", 99_u64), ("repetition", 99)] {
            let mut o = observations(&p);
            let mut extra = o["scenarios"][0].clone();
            extra[field] = json!(value);
            o["scenarios"].as_array_mut().unwrap().push(extra);
            let r = checked(&p, &o);
            assert_eq!(r["local_acceptance"], "fail", "{field}");
            assert_eq!(r["profile_qualification"], "nonqualified");
            assert_eq!(r["publisher_pilot_ready"], false);
            assert_eq!(r["reasons"], json!(["unexpected_evidence"]));
            assert!(
                r["scenarios"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|row| row["reason"] == "accepted")
            );
        }
    }

    #[test]
    fn qualification_refuses_an_unplanned_startup_variant() {
        let p = candidate();
        let mut o = observations(&p);
        let mut extra = o["variants"][0].clone();
        extra["id"] = json!(99_u16);
        extra["process_id"] = json!(99_u32);
        extra["startup_sha256"] = json!("d".repeat(64));
        o["variants"].as_array_mut().unwrap().push(extra);
        let r = checked(&p, &o);
        assert_eq!(r["local_acceptance"], "fail");
        assert_eq!(r["profile_qualification"], "nonqualified");
        assert_eq!(r["publisher_pilot_ready"], false);
        assert_eq!(r["reasons"], json!(["unexpected_evidence"]));
        assert!(
            r["scenarios"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["reason"] == "accepted")
        );
    }

    #[test]
    fn qualification_capabilities_require_successful_digest_bound_evidence() {
        let p = candidate();
        for index in 0..6 {
            for (status, digest_present, qualification) in [
                ("verified", true, "qualified"),
                ("verified", false, "pending"),
                ("pending", true, "pending"),
                ("pending", false, "pending"),
                ("unsupported", true, "unsupported"),
                ("unsupported", false, "unsupported"),
                ("failed", true, "nonqualified"),
                ("failed", false, "nonqualified"),
            ] {
                let mut o = observations(&p);
                o["capabilities"][index]["status"] = json!(status);
                if !digest_present {
                    o["capabilities"][index]["evidence_sha256"] = Value::Null;
                }
                let r = checked(&p, &o);
                assert_eq!(r["local_acceptance"], "pass");
                assert_eq!(
                    r["profile_qualification"], qualification,
                    "{index}/{status}/{digest_present}"
                );
                assert_eq!(r["publisher_pilot_ready"], qualification == "qualified");
                assert_eq!(r["reasons"], json!([]));
            }
            for (copies, qualification) in [(0_usize, "pending"), (2, "nonqualified")] {
                let mut o = observations(&p);
                let rows = o["capabilities"].as_array_mut().unwrap();
                let row = rows.remove(index);
                if copies == 2 {
                    // Stay within the six-row input cap. The other missing
                    // capability is pending, and cannot explain a failure.
                    rows.remove(0);
                }
                rows.extend(repeat_n(row, copies));
                let r = checked(&p, &o);
                assert_eq!(r["local_acceptance"], "pass");
                assert_eq!(r["profile_qualification"], qualification);
                assert_eq!(r["publisher_pilot_ready"], false);
            }
        }
    }

    #[test]
    fn qualification_achieved_call_count_cannot_exceed_the_installed_limit() {
        let mut p = candidate_with_call_limit(4);
        p["scenarios"][0]["load"]["calls"] = json!(2_u32);
        for (calls, status, reason) in [
            (0_u32, "fail", "underload"),
            (1, "fail", "underload"),
            (2, "pass", "accepted"),
            (3, "pass", "accepted"),
            (4, "pass", "accepted"),
            (5, "fail", "underload"),
        ] {
            let mut o = observations(&p);
            o["scenarios"][0]["achieved"]["calls"] = json!(calls);
            let r = checked(&p, &o);
            assert_eq!(r["local_acceptance"], status, "calls {calls}");
            assert_eq!(r["scenarios"][0]["reason"], reason);
            assert_eq!(
                r["profile_qualification"],
                if status == "pass" {
                    "qualified"
                } else {
                    "nonqualified"
                }
            );
        }
    }

    #[test]
    fn qualification_result_mapping_never_promotes_a_refusal() {
        use super::{Qualification, ResultStatus};

        for (status, expected) in [
            (ResultStatus::Pass, Qualification::Qualified),
            (ResultStatus::Fail, Qualification::Nonqualified),
            (ResultStatus::Pending, Qualification::Pending),
            (ResultStatus::Unsupported, Qualification::Unsupported),
        ] {
            assert_eq!(super::qualification(status), expected);
        }
    }

    #[test]
    fn qualification_memory_validates_each_meter_property_independently() {
        let p = candidate();
        let profile = super::super::profile::parse(&serde_json::to_vec(&p).unwrap()).unwrap();
        let baseline = observations(&p)["scenarios"][0]["memory"].clone();
        for (field, value) in [
            ("source", json!("allocator")),
            ("method", json!("sampled_peak")),
            ("method", json!("sampled_average")),
            ("limitation", json!("requested_allocations_only")),
            ("service_only", json!(false)),
            ("window_ms", json!(99_u64)),
            ("window_ms", json!(201_u64)),
        ] {
            let mut value_map = baseline.clone();
            value_map[field] = value;
            if field == "source" {
                // A valid allocator limitation leaves only the profile/source mismatch.
                value_map["limitation"] = json!("requested_allocations_only");
            }
            let memory = serde_json::from_value(value_map).unwrap();
            assert_eq!(
                serde_json::to_value(super::check_memory(
                    &profile,
                    Some(&memory),
                    100,
                    200,
                    1_000_000,
                    100
                ))
                .unwrap(),
                json!(["fail", "peak_unsupported"]),
                "{field}: {memory:?}"
            );
        }
        for window in [100_u64, 150, 200] {
            let mut value = baseline.clone();
            value["window_ms"] = json!(window);
            let memory = serde_json::from_value(value).unwrap();
            assert_eq!(
                serde_json::to_value(super::check_memory(
                    &profile,
                    Some(&memory),
                    100,
                    200,
                    1_000_000,
                    100
                ))
                .unwrap(),
                json!(["pass", "accepted"])
            );
        }
    }

    #[test]
    fn qualification_memory_peak_and_growth_bounds_are_inclusive_and_nonzero() {
        let p = candidate();
        let profile = super::super::profile::parse(&serde_json::to_vec(&p).unwrap()).unwrap();
        let baseline = observations(&p)["scenarios"][0]["memory"].clone();
        for (peak, growth, expected) in [
            (0_u64, 0_u64, ["fail", "peak_exceeded"]),
            (1, 0, ["pass", "accepted"]),
            (999_999, 99, ["pass", "accepted"]),
            (1_000_000, 100, ["pass", "accepted"]),
            (1_000_001, 0, ["fail", "peak_exceeded"]),
            (1_000_000, 101, ["fail", "peak_exceeded"]),
        ] {
            let mut value = baseline.clone();
            value["peak_bytes"] = json!(peak);
            value["retained_growth_bytes"] = json!(growth);
            let memory = serde_json::from_value(value).unwrap();
            assert_eq!(
                serde_json::to_value(super::check_memory(
                    &profile,
                    Some(&memory),
                    100,
                    100,
                    1_000_000,
                    100
                ))
                .unwrap(),
                json!(expected),
                "peak {peak}, growth {growth}"
            );
        }
    }

    #[test]
    fn qualification_accounting_uses_the_smaller_of_independent_memory_ceilings() {
        let p = candidate();
        let mut profile = super::super::profile::parse(&serde_json::to_vec(&p).unwrap()).unwrap();
        let required = profile.derive().unwrap().total_bytes + profile.headroom_bytes;
        for (platform, isolate) in [
            (required, required + 1),
            (required + 1, required),
            (required - 1, required + 1),
            (required + 1, required - 1),
        ] {
            profile.platform.memory_bytes = platform;
            profile.resources.limits.isolate_total_bytes = isolate;
            let accounted = super::account(&profile).unwrap();
            let ceiling = if platform < isolate {
                platform
            } else {
                isolate
            };
            assert_eq!(accounted.platform_bytes, ceiling);
            assert_eq!(accounted.fits, ceiling >= required);
        }
    }

    #[test]
    fn qualification_measured_peak_and_headroom_use_both_independent_ceilings() {
        let p = candidate();
        let mut profile = super::super::profile::parse(&serde_json::to_vec(&p).unwrap()).unwrap();
        profile.headroom_bytes = 0x1000_0000;
        let baseline = observations(&p)["scenarios"][0]["memory"].clone();
        for (platform, isolate) in [
            (0x0001_0000_0000, 0x0002_0000_0000),
            (0x0002_0000_0000, 0x0001_0000_0000),
        ] {
            profile.platform.memory_bytes = platform;
            profile.resources.limits.isolate_total_bytes = isolate;
            for (peak, expected) in [
                (4_026_531_839_u64, ["pass", "accepted"]),
                (0xF000_0000, ["pass", "accepted"]),
                (4_026_531_841, ["fail", "peak_exceeded"]),
                (u64::MAX, ["fail", "peak_exceeded"]),
            ] {
                let mut value = baseline.clone();
                value["peak_bytes"] = json!(peak);
                let memory = serde_json::from_value(value).unwrap();
                assert_eq!(
                    serde_json::to_value(super::check_memory(
                        &profile,
                        Some(&memory),
                        100,
                        100,
                        u64::MAX,
                        100
                    ))
                    .unwrap(),
                    json!(expected),
                    "platform {platform}, isolate {isolate}, peak {peak}"
                );
            }
        }
    }

    #[test]
    fn qualification_lifetime_high_water_cannot_decrease_after_cold_startup() {
        let mut p = candidate();
        p["variants"][0]["max_cold_peak_bytes"] = json!(200_000_000_u64);
        let mut o = observations(&p);
        o["variants"][0]["cold_memory"]["peak_bytes"] = json!(150_000_000_u64);
        let r = checked(&p, &o);
        assert_eq!(r["observations"], o);
        assert_eq!(r["local_acceptance"], "fail");
        assert_eq!(r["profile_qualification"], "nonqualified");
        assert_eq!(r["publisher_pilot_ready"], false);
    }

    #[test]
    fn qualification_lifetime_high_water_cannot_decrease_after_warmup_or_scenarios() {
        let p = candidate();
        for path in [
            "/variants/0/warm_memory/peak_bytes",
            "/scenarios/0/memory/peak_bytes",
        ] {
            let mut o = observations(&p);
            *o.pointer_mut(path).unwrap() = json!(2_000_000_u64);
            assert_eq!(checked(&p, &o)["local_acceptance"], "fail", "{path}");
        }
    }

    #[test]
    fn qualification_lifetime_high_water_uses_schedule_order_not_evidence_order() {
        let mut p = candidate();
        p["scenarios"][1]["repetitions"] = json!(2_u8);
        let mut o = observations(&p);
        o["scenarios"][1]["memory"]["peak_bytes"] = json!(2_000_000_u64);
        o["scenarios"].as_array_mut().unwrap().reverse();
        assert_eq!(checked(&p, &o)["local_acceptance"], "fail");
    }

    #[test]
    fn qualification_lifetime_high_water_accepts_increases_and_independent_processes() {
        let p = candidate();
        let mut o = observations(&p);
        o["variants"][0]["warm_memory"]["peak_bytes"] = json!(2_000_000_u64);
        for (peak, row) in (3_000_000_u64..).zip(o["scenarios"].as_array_mut().unwrap()) {
            if row["variant_id"] == 1_u16 {
                row["memory"]["peak_bytes"] = json!(peak);
            }
        }
        o["scenarios"].as_array_mut().unwrap().reverse();
        assert_eq!(checked(&p, &o)["local_acceptance"], "pass");
    }

    #[test]
    fn qualification_window_meters_can_decrease_between_measurements() {
        for (source, limitation) in [
            ("allocator", "requested_allocations_only"),
            ("wasm_guest", "guest_linear_memory_only"),
            ("provider", "provider_reported_only"),
        ] {
            let mut p = candidate();
            p["identity"]["target"] = json!("spin");
            p["hardware"]["meter"] = json!(source);
            let mut o = observations(&p);
            for row in o["variants"].as_array_mut().unwrap() {
                for key in ["cold_memory", "warm_memory"] {
                    row[key]["source"] = json!(source);
                    row[key]["limitation"] = json!(limitation);
                }
            }
            for row in o["scenarios"].as_array_mut().unwrap() {
                row["memory"]["source"] = json!(source);
                row["memory"]["limitation"] = json!(limitation);
            }
            o["variants"][0]["cold_memory"]["peak_bytes"] = json!(3_000_000_u64);
            o["variants"][0]["warm_memory"]["peak_bytes"] = json!(2_000_000_u64);
            o["scenarios"][0]["memory"]["peak_bytes"] = json!(2_000_000_u64);
            assert_eq!(checked(&p, &o)["local_acceptance"], "pass", "{source}");
        }
    }

    #[test]
    fn qualification_test_harness_override_is_local_only() {
        for features in [
            json!(["test_harness_override"]),
            json!([
                "insecure_bidders",
                "conformance_suite",
                "test_harness_override"
            ]),
        ] {
            let mut p = candidate();
            p["identity"]["build_features"] = features;
            let r = checked(&p, &observations(&p));
            assert_eq!(r["local_acceptance"], "pass");
            assert_eq!(r["profile_qualification"], "unsupported");
            assert_eq!(r["publisher_pilot_ready"], false);
        }
    }

    #[test]
    fn qualification_maximum_debug_checks_reference_ranges_and_simultaneous_width() {
        let p = candidate();
        for (path, replacement) in [
            ("/debug", Value::Null),
            ("/debug/reference_sha256", json!("b".repeat(64))),
            ("/debug/simultaneous_auctions", json!(0_u32)),
            ("/debug/simultaneous_finalized_calls", json!(0_u64)),
            ("/debug/auctions", json!(1_u64)),
            ("/debug/capture_bytes/min", json!(0_u64)),
            ("/debug/output_bytes/min", json!(1_u64)),
            ("/debug/output_bytes/max", json!(1_u64)),
            ("/effects/debug_output_bytes", json!(1_u64)),
        ] {
            let mut o = observations(&p);
            *o["scenarios"][11].pointer_mut(path).unwrap() = replacement;
            assert_eq!(checked(&p, &o)["local_acceptance"], "fail", "{path}");
        }
    }

    #[test]
    fn qualification_concurrency_cannot_exceed_the_total_admitted_population() {
        let mut p = candidate();
        p["resources"]["limits"]["max_concurrent_auctions"] = json!(2_u32);
        for s in p["scenarios"].as_array_mut().unwrap() {
            if matches!(
                s["subcase"].as_str().unwrap(),
                "maximum_shape" | "concurrent_capture" | "warm_baseline"
            ) {
                s["load"]["concurrency"] = json!(2_u32);
            }
            if s["subcase"] == "concurrent_capture" {
                s["effects"]["debug_captures"] = json!(2_u64);
            }
        }
        for field in ["min_offered", "min_admitted", "min_success"] {
            p["scenarios"][1]["outcomes"][field] = json!(1_u64);
        }
        super::super::profile::tests::debug_references(&mut p);
        let mut o = observations(&p);
        o["scenarios"][1]["outcomes"] = json!({"offered":1_u64,"admitted":1_u64,"success":1_u64,
            "rejected":0_u64,"errors":0_u64,"timeouts":0_u64});
        o["scenarios"][1]["latency"]["samples"] = json!(1_u64);
        o["scenarios"][1]["latency"]["histogram"] = json!([{"upper_us":1_000_u64,"count":1_u64}]);
        assert_eq!(checked(&p, &o)["local_acceptance"], "fail");
    }

    #[test]
    fn qualification_effect_counts_cannot_exceed_their_auction_call_populations() {
        let p = candidate();
        for (field, impossible) in [
            ("decoded_responses", 11_u64),
            ("debug_captures", 11_u64),
            ("debug_denials", 21_u64),
            ("debug_output_bytes", 655_361_u64),
            ("queued_calls", 11_u64),
            ("cancellations", 21_u64),
            ("fpd_expansions", 321_u64),
        ] {
            let mut o = observations(&p);
            o["scenarios"][0]["effects"][field] = json!(impossible);
            assert_eq!(checked(&p, &o)["local_acceptance"], "fail", "{field}");
        }
    }

    #[test]
    fn qualification_failed_admissions_can_still_exercise_calls_and_later_recovery() {
        let mut p = candidate();
        p["scenarios"][21]["outcomes"]["min_success"] = json!(0_u64);
        p["scenarios"][21]["outcomes"]["max_errors"] = json!(10_u64);
        let mut o = observations(&p);
        o["scenarios"][21]["outcomes"]["success"] = json!(0_u64);
        o["scenarios"][21]["outcomes"]["errors"] = json!(10_u64);
        o["scenarios"][21]["effects"]["cancellations"] = json!(10_u64);
        o["scenarios"][21]["effects"]["recovery_successes"] = json!(1_u64);
        assert_eq!(checked(&p, &o)["local_acceptance"], "pass");
    }

    #[test]
    fn qualification_local_acceptance_does_not_upgrade_sdk_capabilities() {
        for status in ["pending", "unsupported"] {
            let p = candidate();
            let mut o = observations(&p);
            o["capabilities"][0]["status"] = json!(status);
            o["capabilities"][0]["evidence_sha256"] = Value::Null;
            let r = checked(&p, &o);
            assert_eq!(r["local_acceptance"], "pass");
            assert_eq!(r["profile_qualification"], status);
            assert_eq!(r["publisher_pilot_ready"], false);
        }
    }

    #[test]
    fn qualification_missing_rows_cannot_pass_local_acceptance() {
        let p = candidate();
        let mut o = observations(&p);
        o["scenarios"].as_array_mut().unwrap().pop();
        assert_eq!(checked(&p, &o)["local_acceptance"], "pending");
    }

    #[test]
    fn qualification_repetitions_and_duplicate_evidence_are_not_optional() {
        let mut p = candidate();
        p["scenarios"][1]["repetitions"] = json!(2_i32);
        let mut o = observations(&p);
        o["scenarios"].as_array_mut().unwrap().pop();
        assert_eq!(checked(&p, &o)["local_acceptance"], "pending");
        let row = o["scenarios"][0].clone();
        o["scenarios"].as_array_mut().unwrap().push(row);
        assert_eq!(checked(&p, &o)["local_acceptance"], "fail");
    }

    #[test]
    fn qualification_repeated_cold_samples_bind_distinct_process_variants() {
        let mut p = candidate();
        let mut variant = p["variants"][0].clone();
        variant["id"] = json!(4_u16);
        p["variants"].as_array_mut().unwrap().push(variant);
        let mut cold = p["scenarios"][0].clone();
        cold["variant_id"] = json!(4_u16);
        p["scenarios"].as_array_mut().unwrap().push(cold);
        let mut o = observations(&p);
        assert_eq!(checked(&p, &o)["local_acceptance"], "pass");
        o["variants"][3]["startup_sha256"] = o["variants"][0]["startup_sha256"].clone();
        assert_eq!(checked(&p, &o)["local_acceptance"], "fail");
    }

    #[test]
    fn qualification_derives_latency_from_complete_histogram_populations() {
        let p = candidate();
        for (histogram, samples, expected) in [
            (
                json!([{"upper_us":1_000_u64,"count":9_u64}]),
                10_u64,
                "fail",
            ),
            (
                json!([{"upper_us":20_000_u64,"count":10_u64}]),
                10_u64,
                "fail",
            ),
            (
                json!([{"upper_us":2_000_u64,"count":5_u64},{"upper_us":1_000_u64,"count":5_u64}]),
                10_u64,
                "fail",
            ),
            (
                json!([{"upper_us":1_000_u64,"count":10_u64}]),
                9_u64,
                "fail",
            ),
            (
                json!([{"upper_us":1_000_u64,"count":10_u64}]),
                10_u64,
                "pass",
            ),
        ] {
            let mut o = observations(&p);
            o["scenarios"][0]["latency"]["histogram"] = histogram;
            o["scenarios"][0]["latency"]["samples"] = json!(samples);
            assert_eq!(checked(&p, &o)["local_acceptance"], expected);
        }
    }

    #[test]
    fn qualification_requires_subcase_counters_and_fresh_variant_peaks() {
        let p = candidate();
        for (path, value, expected) in [
            ("/scenarios/2/effects/gvl_parses", json!(0_i32), "fail"),
            ("/scenarios/4/effects/fpd_expansions", json!(0_i32), "fail"),
            ("/scenarios/10/effects/debug_captures", json!(0_i32), "fail"),
            ("/scenarios/8/effects/debug_captures", json!(1_i32), "fail"),
            ("/scenarios/0/memory", Value::Null, "pending"),
            (
                "/scenarios/0/memory/method",
                json!("unsupported"),
                "unsupported",
            ),
            ("/variants/1/cold_memory", Value::Null, "pending"),
            ("/variants/2/warm_memory", Value::Null, "pending"),
            ("/variants/1/cold_memory/window_ms", json!(99_i32), "fail"),
            (
                "/variants/2/warm_memory/peak_bytes",
                json!(100_000_001_i32),
                "fail",
            ),
            (
                "/variants/1/startup_sha256",
                json!(format!("{:064x}", 1_i32)),
                "fail",
            ),
        ] {
            let mut o = observations(&p);
            *o.pointer_mut(path).unwrap() = value;
            assert_eq!(checked(&p, &o)["local_acceptance"], expected, "{path}");
        }
    }

    #[test]
    fn qualification_binds_every_identity_and_never_promotes_missing_sdk_evidence() {
        let p = candidate();
        for path in [
            "/profile_sha256",
            "/identity/artifact_sha256",
            "/identity/harness_sha256",
            "/identity/reference_data_sha256",
            "/identity/baked_config_sha256",
            "/hardware/machine_sha256",
        ] {
            let mut o = observations(&p);
            *o.pointer_mut(path).unwrap() = json!("d".repeat(64));
            assert_eq!(checked(&p, &o)["local_acceptance"], "fail", "{path}");
        }
        let mut o = observations(&p);
        o["capabilities"] = json!([]);
        let r = checked(&p, &o);
        assert_eq!(r["local_acceptance"], "pass");
        assert_eq!(r["profile_qualification"], "pending");
        let mut development_profile = p;
        development_profile["identity"]["build_features"] = json!(["insecure_bidders"]);
        let development_result = checked(&development_profile, &observations(&development_profile));
        assert_eq!(development_result["local_acceptance"], "pass");
        assert_eq!(development_result["profile_qualification"], "unsupported");
        assert_eq!(development_result["publisher_pilot_ready"], false);
    }

    #[test]
    fn qualification_preserves_all_safe_inputs_without_copying_secret_canaries() {
        let p = candidate();
        let o = observations(&p);
        let r = checked(&p, &o);
        assert_eq!(r["profile"], p);
        assert_eq!(r["observations"], o);
        let bytes = serde_json::to_vec(&r).unwrap();
        assert!(bytes.len() <= super::super::MAX_OUTPUT_BYTES);
        assert!(!String::from_utf8(bytes).unwrap().contains("SECRET_CANARY"));
    }

    #[test]
    fn qualification_rejects_underload_short_windows_failures_and_biased_latency() {
        let p = candidate();
        let mutations = [
            ("/scenarios/0/achieved/concurrency", json!(0_i32)),
            ("/scenarios/0/achieved/calls", json!(0_i32)),
            ("/scenarios/0/achieved/request_bytes", json!(1_i32)),
            ("/scenarios/0/achieved/nodes", json!(1_i32)),
            ("/scenarios/0/duration_ms", json!(99_i32)),
            ("/scenarios/0/warmup_ms", json!(9_i32)),
            ("/scenarios/0/load_window_ms", json!(99_i32)),
            ("/scenarios/0/latency/population", json!("success_only")),
            ("/scenarios/0/latency/samples", json!(9_i32)),
            ("/scenarios/0/latency/value_us", json!(10_001_i32)),
            ("/scenarios/0/memory/method", json!("sampled_average")),
            ("/scenarios/0/memory/peak_bytes", json!(100_000_001_i32)),
            ("/scenarios/0/memory/window_ms", json!(99_i32)),
            ("/scenarios/0/memory/service_only", json!(false)),
            ("/scenarios/0/recovered", json!(false)),
            ("/variants/1/restarted", json!(false)),
            ("/variants/1/effective_settings_observed", json!(false)),
            ("/variants/1/startup_snapshot_sha256", json!(digest())),
            ("/resources/sdk/ingress/max_connections", json!(255_i32)),
        ];
        for (path, value) in mutations {
            let mut o = observations(&p);
            *o.pointer_mut(path).unwrap() = value;
            assert_eq!(checked(&p, &o)["local_acceptance"], "fail", "{path}");
        }
        for (field, success, admitted, rejected) in [
            ("errors", 9_i32, 10_i32, 0_i32),
            ("timeouts", 9_i32, 10_i32, 0_i32),
            ("rejected", 10_i32, 10_i32, 1_i32),
        ] {
            let mut o = observations(&p);
            o["scenarios"][0]["outcomes"] = json!({"offered":admitted+rejected,"admitted":admitted,
                "success":success,"errors":0_i32,"timeouts":0_i32,"rejected":rejected});
            o["scenarios"][0]["outcomes"][field] = json!(1_i32);
            assert_eq!(checked(&p, &o)["local_acceptance"], "fail", "{field}");
        }
    }

    #[test]
    fn qualification_accounted_ceiling_and_headroom_are_independent_of_measurements() {
        let mut p = candidate();
        p["resources"]["limits"]["isolate_total_bytes"] = json!(500_000_000_i32);
        p["platform"]["memory_bytes"] = json!(500_000_000_i32);
        p["headroom_bytes"] = json!(0x1000_0000_u64);
        super::super::profile::tests::debug_references(&mut p);
        let over_cap = checked(&p, &observations(&p));
        assert_eq!(over_cap["local_acceptance"], "fail");
        assert!(
            over_cap["reasons"]
                .as_array()
                .unwrap()
                .contains(&json!("envelope_over_budget"))
        );
        let mut no_headroom = candidate();
        no_headroom["headroom_bytes"] = no_headroom["platform"]["memory_bytes"].clone();
        assert_eq!(
            checked(&no_headroom, &observations(&no_headroom))["local_acceptance"],
            "fail"
        );
    }

    #[test]
    fn qualification_measured_peaks_cannot_bypass_a_smaller_installed_ceiling() {
        let mut p = candidate();
        p["resources"]["limits"]["isolate_total_bytes"] = json!(4_000_000_000_u64);
        p["headroom_bytes"] = json!(0x1000_0000_u64);
        let index = p["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .rposition(|scenario| scenario["variant_id"] == 1_i32)
            .unwrap();
        assert_eq!(p["scenarios"][index]["subcase"], "healthy_recovery");
        p["scenarios"][index]["max_peak_bytes"] = json!(5_000_000_000_u64);
        super::super::profile::tests::debug_references(&mut p);
        for (peak, local, reason) in [
            (3_731_564_543_u64, "pass", "accepted"),
            (3_731_564_544, "pass", "accepted"),
            (3_731_564_545, "fail", "peak_exceeded"),
        ] {
            let mut o = observations(&p);
            o["scenarios"][index]["memory"]["peak_bytes"] = json!(peak);
            let r = checked(&p, &o);
            assert_eq!(r["accounted"]["fits"], true);
            assert_eq!(r["accounted"]["required_headroom_bytes"], 0x1000_0000_u64);
            assert_eq!(r["reasons"], json!([]));
            assert_eq!(r["scenarios"][index]["status"], local);
            assert_eq!(r["scenarios"][index]["reason"], reason);
            assert_eq!(r["local_acceptance"], local);
        }
    }
}

use serde::{Deserialize, Serialize};

use super::profile::{
    Backend, ByteBounds, Capability, Digest, Effects, Fixture, Hardware, Identity, LatencyEnd,
    LatencyStart, Load, Meter, Platform, Population, Profile, ResourceExport, Subcase,
};
use super::{Error, MAX_ROWS, decode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Verified,
    Failed,
    Pending,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Pass,
    Fail,
    Pending,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Qualification {
    Qualified,
    Nonqualified,
    Pending,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeakMethod {
    HighWater,
    SampledPeak,
    SampledAverage,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    Bytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeterLimitation {
    ProcessLifetimeHighWater,
    RequestedAllocationsOnly,
    GuestLinearMemoryOnly,
    ProviderReportedOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    pub limitation: MeterLimitation,
    pub source: Meter,
    pub method: PeakMethod,
    pub units: Units,
    pub window_ms: u64,
    pub peak_bytes: u64,
    pub retained_growth_bytes: u64,
    pub service_only: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub offered: u64,
    pub admitted: u64,
    pub success: u64,
    pub rejected: u64,
    pub errors: u64,
    pub timeouts: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Latency {
    pub histogram: Vec<LatencyBucket>,
    pub start: LatencyStart,
    pub end: LatencyEnd,
    pub population: Population,
    pub percentile: u8,
    pub samples: u64,
    pub value_us: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Noncumulative bucket counts include every outcome in the declared population.
pub struct LatencyBucket {
    pub upper_us: u64,
    pub count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioObservation {
    pub effects: Effects,
    pub subcase: Subcase,
    pub variant_id: u16,
    pub repetition: u8,
    pub fixture: Fixture,
    pub seed: u64,
    pub warmup_ms: u64,
    pub duration_ms: u64,
    pub load_window_ms: u64,
    pub achieved: Load,
    pub simultaneous: bool,
    pub outcomes: Counts,
    pub latency: Latency,
    pub memory: Option<Memory>,
    pub recovered: bool,
    pub debug: Option<DebugObservation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugObservation {
    pub reference_sha256: Digest,
    pub auctions: u64,
    pub simultaneous_auctions: u32,
    pub simultaneous_finalized_calls: u64,
    pub min_call_rows: u32,
    pub max_call_rows: u32,
    pub capture_bytes: ByteBounds,
    pub output_bytes: ByteBounds,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantObservation {
    pub cold_memory: Option<Memory>,
    pub warm_memory: Option<Memory>,
    pub id: u16,
    pub process_id: u32,
    pub startup_sha256: Digest,
    pub config_sha256: Digest,
    pub startup_snapshot_sha256: Digest,
    pub settings_sha256: Digest,
    pub restarted: bool,
    pub input_immutable: bool,
    pub effective_settings_observed: bool,
    pub cold_window_ms: u64,
    pub warm_window_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityEvidence {
    pub capability: Capability,
    pub status: EvidenceStatus,
    pub evidence_sha256: Option<Digest>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observations {
    pub schema_version: u16,
    pub profile_sha256: Digest,
    pub identity: Identity,
    pub resources: ResourceExport,
    pub hardware: Hardware,
    pub platform: Platform,
    pub backend: Backend,
    pub variants: Vec<VariantObservation>,
    pub capabilities: Vec<CapabilityEvidence>,
    pub scenarios: Vec<ScenarioObservation>,
}

pub fn parse(bytes: &[u8]) -> Result<Observations, Error> {
    let observations: Observations = decode(bytes)?;
    if observations.schema_version != super::profile::RESOURCE_SCHEMA_VERSION
        || observations.scenarios.len() > MAX_ROWS
        || observations.variants.len() > MAX_ROWS
        || observations.capabilities.len() > 6
        || observations.identity.build_features.len() > 3
        || observations
            .scenarios
            .iter()
            .any(|s| s.latency.histogram.len() > 32)
    {
        return Err(Error::InvalidObservations);
    }
    Ok(observations)
}

#[derive(Debug, Serialize)]
pub struct Accounted {
    pub total_bytes: u64,
    pub catalog_resident_term: u64,
    pub catalog_initialization_term: u64,
    pub schema_validation_term: u64,
    pub catalog_context_term: u64,
    pub debug_per_auction_bytes: u64,
    pub per_auction_bytes: u64,
    pub baked_config_bytes: u64,
    pub platform_bytes: u64,
    pub required_headroom_bytes: u64,
    pub effective_profile_sha256: Digest,
    pub fits: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Accepted,
    MissingEvidence,
    DuplicateEvidence,
    UnexpectedEvidence,
    IdentityMismatch,
    EnvelopeOverBudget,
    StartupEvidence,
    Underload,
    WindowShortened,
    OutcomeBounds,
    LatencyPopulation,
    LatencyExceeded,
    PeakMissing,
    PeakUnsupported,
    PeakExceeded,
    RecoveryFailed,
}

#[derive(Debug, Serialize)]
pub struct ScenarioResult {
    pub subcase: Subcase,
    pub variant_id: u16,
    pub repetition: u8,
    pub status: ResultStatus,
    pub reason: Reason,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u16,
    pub profile_sha256: Digest,
    pub profile: Profile,
    pub observations: Observations,
    pub accounted: Accounted,
    pub scenarios: Vec<ScenarioResult>,
    pub local_acceptance: ResultStatus,
    pub profile_qualification: Qualification,
    pub publisher_pilot_ready: bool,
    pub reasons: Vec<Reason>,
}

/// Admit only schedules whose complete evidence fits every existing CLI cap.
/// Numeric maxima and longest tokens also cover completed failing measurements.
pub fn ensure_capacity(profile: &Profile) -> Result<(), Error> {
    let digest = Digest::of(b"qualification_capacity_bound");
    let observations = capacity_observations(profile, &digest);
    let bytes = super::serialize(&observations).map_err(|_error| Error::ProfileCapacity)?;
    auctioneer_runtime::count_json_nodes(&bytes, super::MAX_NODES)
        .map_err(|_error| Error::ProfileCapacity)?;
    let scenarios = observations
        .scenarios
        .iter()
        .map(|row| ScenarioResult {
            subcase: row.subcase,
            variant_id: row.variant_id,
            repetition: row.repetition,
            status: ResultStatus::Unsupported,
            reason: Reason::EnvelopeOverBudget,
        })
        .collect();
    let report = Report {
        schema_version: super::profile::RESOURCE_SCHEMA_VERSION,
        profile_sha256: digest.clone(),
        profile: profile.clone(),
        observations,
        accounted: Accounted {
            total_bytes: u64::MAX,
            catalog_resident_term: u64::MAX,
            catalog_initialization_term: u64::MAX,
            schema_validation_term: u64::MAX,
            catalog_context_term: u64::MAX,
            debug_per_auction_bytes: u64::MAX,
            per_auction_bytes: u64::MAX,
            baked_config_bytes: u64::MAX,
            platform_bytes: u64::MAX,
            required_headroom_bytes: u64::MAX,
            effective_profile_sha256: digest,
            fits: false,
        },
        scenarios,
        local_acceptance: ResultStatus::Unsupported,
        profile_qualification: Qualification::Nonqualified,
        publisher_pilot_ready: false,
        reasons: vec![
            Reason::EnvelopeOverBudget;
            4_usize.saturating_add(3_usize.saturating_mul(profile.variants.len()))
        ],
    };
    let size = auctioneer_runtime::serialized_json_bytes(&report)
        .map_err(|_error| Error::ProfileCapacity)?;
    if size > u64::try_from(super::MAX_OUTPUT_BYTES).map_err(|_error| Error::ProfileCapacity)? {
        return Err(Error::ProfileCapacity);
    }
    Ok(())
}

fn capacity_observations(profile: &Profile, digest: &Digest) -> Observations {
    let scenarios = profile
        .scenarios
        .iter()
        .flat_map(|s| {
            (1..=s.repetitions).map(|repetition| capacity_scenario(s, repetition, digest))
        })
        .collect();
    let variants = profile
        .variants
        .iter()
        .map(|v| VariantObservation {
            cold_memory: Some(capacity_memory()),
            warm_memory: Some(capacity_memory()),
            id: v.id,
            process_id: u32::MAX,
            startup_sha256: digest.clone(),
            config_sha256: digest.clone(),
            startup_snapshot_sha256: digest.clone(),
            settings_sha256: digest.clone(),
            restarted: false,
            input_immutable: false,
            effective_settings_observed: false,
            cold_window_ms: u64::MAX,
            warm_window_ms: u64::MAX,
        })
        .collect();
    let capabilities = profile
        .required_capabilities
        .iter()
        .map(|c| CapabilityEvidence {
            capability: *c,
            status: EvidenceStatus::Unsupported,
            evidence_sha256: Some(digest.clone()),
        })
        .collect();
    Observations {
        schema_version: super::profile::RESOURCE_SCHEMA_VERSION,
        profile_sha256: digest.clone(),
        identity: profile.identity.clone(),
        resources: profile.resources.clone(),
        hardware: profile.hardware.clone(),
        platform: profile.platform.clone(),
        backend: profile.backend,
        variants,
        capabilities,
        scenarios,
    }
}

fn capacity_scenario(
    s: &super::profile::Scenario,
    repetition: u8,
    digest: &Digest,
) -> ScenarioObservation {
    let bounds = ByteBounds {
        min: u64::MAX,
        max: u64::MAX,
    };
    ScenarioObservation {
        effects: Effects {
            gvl_parses: u64::MAX,
            fpd_expansions: u64::MAX,
            decoded_responses: u64::MAX,
            slow_reads: u64::MAX,
            debug_captures: u64::MAX,
            debug_denials: u64::MAX,
            debug_output_bytes: u64::MAX,
            auxiliary_requests: u64::MAX,
            config_extractions: u64::MAX,
            queued_calls: u64::MAX,
            cancellations: u64::MAX,
            stalled_reads: u64::MAX,
            recovery_successes: u64::MAX,
        },
        subcase: s.subcase,
        variant_id: s.variant_id,
        repetition,
        fixture: s.fixture.clone(),
        seed: u64::MAX,
        warmup_ms: u64::MAX,
        duration_ms: u64::MAX,
        load_window_ms: u64::MAX,
        achieved: Load {
            concurrency: u32::MAX,
            calls: u32::MAX,
            request_bytes: u64::MAX,
            nodes: u32::MAX,
            impressions: u16::MAX,
            encoded_bytes: u64::MAX,
            decoded_bytes: u64::MAX,
        },
        simultaneous: false,
        outcomes: Counts {
            offered: u64::MAX,
            admitted: u64::MAX,
            success: u64::MAX,
            rejected: u64::MAX,
            errors: u64::MAX,
            timeouts: u64::MAX,
        },
        latency: Latency {
            histogram: vec![
                LatencyBucket {
                    upper_us: u64::MAX,
                    count: u64::MAX
                };
                usize::from(s.latency.max_histogram_buckets)
            ],
            start: LatencyStart::ClientDispatch,
            end: LatencyEnd::TerminalOutcome,
            population: Population::AllAdmitted,
            percentile: u8::MAX,
            samples: u64::MAX,
            value_us: u64::MAX,
        },
        memory: Some(capacity_memory()),
        recovered: false,
        debug: s.debug_workload.as_ref().map(|_| DebugObservation {
            reference_sha256: digest.clone(),
            auctions: u64::MAX,
            simultaneous_auctions: u32::MAX,
            simultaneous_finalized_calls: u64::MAX,
            min_call_rows: u32::MAX,
            max_call_rows: u32::MAX,
            capture_bytes: bounds,
            output_bytes: bounds,
        }),
    }
}

fn capacity_memory() -> Memory {
    Memory {
        limitation: MeterLimitation::ProcessLifetimeHighWater,
        source: Meter::ProcessRss,
        method: PeakMethod::SampledAverage,
        units: Units::Bytes,
        window_ms: u64::MAX,
        peak_bytes: u64::MAX,
        retained_growth_bytes: u64::MAX,
        service_only: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::{
        evaluate::{evaluate, tests::observations},
        profile::{
            parse as parse_profile,
            tests::{candidate, repeated_candidate},
        },
    };
    use serde_json::{Value, json};

    #[test]
    fn observations_require_current_version_and_explicit_catalog_costs() {
        let value = observations(&candidate());
        let parsed = parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(parsed.schema_version, 2);
        assert_eq!(serde_json::to_value(parsed).unwrap(), value);
        for version in [0_u16, 1, 3, u16::MAX] {
            let mut obsolete = value.clone();
            obsolete["schema_version"] = json!(version);
            assert_eq!(
                parse(&serde_json::to_vec(&obsolete).unwrap()).unwrap_err(),
                Error::InvalidObservations
            );
        }
        let mut positional = value.clone();
        positional["resources"]["catalog_costs"] = json!([0_u64, 0_u64, 0_u64, 0_u64]);
        assert_eq!(
            parse(&serde_json::to_vec(&positional).unwrap()).unwrap_err(),
            Error::InvalidJson
        );
        for field in [
            "resident_bytes",
            "initialization_bytes",
            "schema_validation_per_auction_bytes",
            "context_per_auction_bytes",
        ] {
            let mut missing = value.clone();
            assert!(
                missing["resources"]["catalog_costs"]
                    .as_object_mut()
                    .unwrap()
                    .remove(field)
                    .is_some()
            );
            assert_eq!(
                parse(&serde_json::to_vec(&missing).unwrap()).unwrap_err(),
                Error::InvalidJson
            );
        }
    }

    #[test]
    fn qualification_observations_preserve_three_build_modes_and_bound_the_list() {
        let modes = [
            "insecure_bidders",
            "conformance_suite",
            "test_harness_override",
        ];
        for count in 0..=3 {
            let mut o = observations(&candidate());
            o["identity"]["build_features"] = json!(&modes[..count]);
            let parsed = parse(&serde_json::to_vec(&o).unwrap()).unwrap();
            assert_eq!(
                serde_json::to_value(parsed.identity.build_features).unwrap(),
                json!(&modes[..count])
            );
        }
        let mut o = observations(&candidate());
        // Repeating a known mode reaches the collection cap, not enum decoding.
        o["identity"]["build_features"] = json!([
            "insecure_bidders",
            "conformance_suite",
            "test_harness_override",
            "insecure_bidders"
        ]);
        assert!(matches!(
            parse(&serde_json::to_vec(&o).unwrap()),
            Err(super::Error::InvalidObservations)
        ));
    }

    #[test]
    fn qualification_observations_reject_unknown_secret_bearing_fields() {
        for path in [
            "",
            "/identity",
            "/resources/sdk/ingress",
            "/hardware",
            "/variants/0",
            "/scenarios/0",
            "/scenarios/0/memory",
            "/scenarios/0/outcomes",
            "/scenarios/0/effects",
            "/scenarios/10/debug",
            "/scenarios/10/debug/capture_bytes",
            "/capabilities/0",
        ] {
            let mut o = observations(&candidate());
            o.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("SECRET_CANARY".into(), json!("SECRET_CANARY"));
            assert!(parse(&serde_json::to_vec(&o).unwrap()).is_err(), "{path}");
        }
    }

    #[test]
    fn qualification_observations_reject_supplied_qualification_and_totals() {
        for key in [
            "qualified",
            "local_acceptance",
            "profile_qualification",
            "accounted_total_bytes",
        ] {
            let mut o = observations(&candidate());
            o[key] = json!(true);
            parse(&serde_json::to_vec(&o).unwrap()).unwrap_err();
        }
    }

    #[test]
    fn qualification_observation_histograms_have_a_hard_collection_cap() {
        let mut o = observations(&candidate());
        o["scenarios"][0]["latency"]["histogram"] =
            json!(vec![json!({"upper_us":1_u64,"count":1_u64}); 32]);
        parse(&serde_json::to_vec(&o).unwrap()).unwrap();
        o["scenarios"][0]["latency"]["histogram"]
            .as_array_mut()
            .unwrap()
            .push(json!({"upper_us":1_u64,"count":1_u64}));
        parse(&serde_json::to_vec(&o).unwrap()).unwrap_err();
    }

    #[test]
    fn qualification_unrepresentable_sixty_four_row_schedule_is_refused_before_running() {
        let mut p = candidate();
        p["scenarios"][1]["repetitions"] = json!(41_u8);
        assert_eq!(
            parse_profile(&serde_json::to_vec(&p).unwrap()).unwrap_err(),
            Error::ProfileCapacity
        );
    }

    #[test]
    fn qualification_complete_repeated_matrix_fits_with_its_maximum_histograms() {
        let mut p = repeated_candidate();
        for s in p["scenarios"].as_array_mut().unwrap() {
            s["latency"]["max_histogram_buckets"] = json!(4_u8);
        }
        let bytes = serde_json::to_vec(&p).unwrap();
        let profile = parse_profile(&bytes).unwrap();
        let worst = capacity_observations(&profile, &Digest::of(b"capacity_test"));
        assert_eq!(worst.scenarios.len(), 48);
        assert_eq!(worst.variants.len(), 6);
        parse(&super::super::serialize(&worst).unwrap()).unwrap();
        let mut o = observations(&p);
        for row in o["scenarios"].as_array_mut().unwrap() {
            for index in 2..=4_u64 {
                row["latency"]["histogram"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"upper_us":index * 1_000,"count":0_u64}));
            }
        }
        let observations = parse(&serde_json::to_vec(&o).unwrap()).unwrap();
        let result = evaluate(profile, observations, Digest::of(&bytes)).unwrap();
        assert_eq!(result.local_acceptance, ResultStatus::Pass);
        assert_eq!(result.scenarios.len(), 48);
        assert_eq!(result.observations.scenarios.len(), 48);
        super::super::serialize(&result).unwrap();
    }

    #[test]
    fn qualification_native_single_restart_schedule_fits_eight_full_buckets() {
        let mut p = candidate();
        for s in p["scenarios"].as_array_mut().unwrap() {
            if !matches!(
                s["subcase"].as_str().unwrap(),
                "cold_startup" | "config_variant" | "config_restoration"
            ) {
                s["repetitions"] = json!(2_u8);
            }
            s["latency"]["max_histogram_buckets"] = json!(8_u8);
        }
        let bytes = serde_json::to_vec(&p).unwrap();
        let profile = parse_profile(&bytes).unwrap();
        let worst = capacity_observations(&profile, &Digest::of(b"capacity_test"));
        assert_eq!(worst.scenarios.len(), 45);
        assert_eq!(worst.variants.len(), 3);
        parse(&super::super::serialize(&worst).unwrap()).unwrap();
        let mut o = observations(&p);
        for row in o["scenarios"].as_array_mut().unwrap() {
            for index in 2..=8_u64 {
                row["latency"]["histogram"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"upper_us":index * 1_000,"count":0_u64}));
            }
        }
        let result = evaluate(
            profile,
            parse(&serde_json::to_vec(&o).unwrap()).unwrap(),
            Digest::of(&bytes),
        )
        .unwrap();
        assert_eq!(result.local_acceptance, ResultStatus::Pass);
        super::super::serialize(&result).unwrap();
    }

    #[test]
    fn qualification_full_histograms_and_variant_geometry_are_checked_before_acceptance() {
        let mut p = candidate();
        for s in p["scenarios"].as_array_mut().unwrap() {
            s["latency"]["max_histogram_buckets"] = json!(32_u8);
        }
        let profile = parse_profile(&serde_json::to_vec(&p).unwrap()).unwrap();
        parse(
            &super::super::serialize(&capacity_observations(
                &profile,
                &Digest::of(b"capacity_test"),
            ))
            .unwrap(),
        )
        .unwrap();
        let mut repeated = repeated_candidate();
        for s in repeated["scenarios"].as_array_mut().unwrap() {
            s["latency"]["max_histogram_buckets"] = json!(32_u8);
        }
        assert_eq!(
            parse_profile(&serde_json::to_vec(&repeated).unwrap()).unwrap_err(),
            Error::ProfileCapacity
        );
        let originals = p["variants"].clone();
        let mut variants = Vec::new();
        for (index, s) in p["scenarios"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            let mut v = originals
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == s["variant_id"])
                .unwrap()
                .clone();
            v["id"] = json!(index + 1);
            s["variant_id"] = json!(index + 1);
            variants.push(v);
        }
        p["variants"] = json!(variants);
        // The 64-row/64-variant/32-bucket corner is refused, never truncated.
        for id in 25..=64_u16 {
            let mut s = p["scenarios"][1].clone();
            s["variant_id"] = json!(id);
            let mut v = originals[0].clone();
            v["id"] = json!(id);
            p["scenarios"].as_array_mut().unwrap().push(s);
            p["variants"].as_array_mut().unwrap().push(v);
        }
        let corner: Profile = serde_json::from_value(p).unwrap();
        assert_eq!(corner.validate().unwrap_err(), Error::ProfileCapacity);
        parse(
            &serde_json::to_vec(&capacity_observations(
                &corner,
                &Digest::of(b"capacity_test"),
            ))
            .unwrap(),
        )
        .unwrap_err();
    }

    #[test]
    fn qualification_digest_lengths_are_exact_and_duplicates_are_rejected() {
        serde_json::from_value::<Digest>(json!("a".repeat(64))).unwrap();
        serde_json::from_value::<Digest>(json!("a".repeat(65))).unwrap_err();
        serde_json::from_value::<Digest>(json!("A".repeat(64))).unwrap_err();
        let bytes = serde_json::to_vec(&observations(&candidate())).unwrap();
        let mut duplicate = b"{\"schema_version\":2,".to_vec();
        duplicate.extend_from_slice(&bytes[1..]);
        parse(&duplicate).unwrap_err();
        let mut nonfinite = observations(&candidate());
        nonfinite["scenarios"][0]["latency"]["value_us"] = Value::Null;
        parse(&serde_json::to_vec(&nonfinite).unwrap()).unwrap_err();
    }
}

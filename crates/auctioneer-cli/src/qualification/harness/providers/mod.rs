use crate::qualification::harness::{HarnessError, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::num::FpCategory;

mod archive;
pub mod artifacts;
pub mod preparation;
pub mod sdk;

#[cfg(test)]
mod recovery;

const SDK_PIN: &str = "598f0a2a490eb83dd31c06c024a0cefa8e049628";
const OVERRIDE_FEATURE: &str = "auctioneer-config-data/test-harness-override";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OfflineInput {
    schema_version: Value,
    kind: Value,
    provider: Value,
    environment: Value,
    identity: Value,
    expected_identity: Value,
    cases: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvironmentInput {
    kind: Value,
    deployment_sha256: Value,
    binding_sha256: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityInput {
    auctioneer_commit: Value,
    edgezero_pin: Value,
    artifact_sha256: Value,
    source_sha256: Value,
    build_features: Value,
    target: Value,
    runtime_version: Value,
    harness_sha256: Value,
    fixtures_sha256: Value,
    reference_data_sha256: Value,
    baked_config_sha256: Value,
    fixture_metadata_sha256: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeInput {
    major: Value,
    minor: Value,
    patch: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseInput {
    id: Value,
    kind: Value,
    transport: Value,
    status: Value,
    dispatch_before: Value,
    dispatch_after: Value,
    fixture_case_id: Value,
    bid_count: Value,
    failure: Value,
    assertions_verified: Value,
    mutation: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MutationInput {
    isolated: Value,
    authorized: Value,
    addressing_verified: Value,
    known_good_observed: Value,
    recovery_armed: Value,
    injection: Value,
    write_visible: Value,
    handler_observed: Value,
    refusal: Value,
    restoration: Value,
    recovered: Value,
    interrupted: Value,
}

#[derive(Clone, Copy)]
enum Provider {
    Cloudflare,
    Fastly,
    Spin,
}

impl Provider {
    fn parse(value: &Value) -> Result<Self> {
        match value.as_str() {
            Some("cloudflare") => Ok(Self::Cloudflare),
            Some("fastly") => Ok(Self::Fastly),
            Some("spin") => Ok(Self::Spin),
            Some(_) | None => Err(HarnessError::new("invalid_input")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Cloudflare => "cloudflare",
            Self::Fastly => "fastly",
            Self::Spin => "spin",
        }
    }

    fn target(self) -> &'static str {
        match self {
            Self::Cloudflare => "wasm32-unknown-unknown",
            Self::Fastly => "wasm32-wasip1",
            Self::Spin => "wasm32-wasip2",
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum CaseKind {
    Banner,
    BidderFailure,
    BidderTimeout,
    MalformedRequest,
    MalformedConfig,
    DigestMismatch,
    DebugDenied,
    DebugRedacted,
    InputCap,
    OutputCap,
    Headers,
    Cancellation,
    Completion,
    ConfigReplacement,
}

const CASES: [CaseKind; 14] = [
    CaseKind::Banner,
    CaseKind::BidderFailure,
    CaseKind::BidderTimeout,
    CaseKind::MalformedRequest,
    CaseKind::MalformedConfig,
    CaseKind::DigestMismatch,
    CaseKind::DebugDenied,
    CaseKind::DebugRedacted,
    CaseKind::InputCap,
    CaseKind::OutputCap,
    CaseKind::Headers,
    CaseKind::Cancellation,
    CaseKind::Completion,
    CaseKind::ConfigReplacement,
];

impl CaseKind {
    fn name(self) -> &'static str {
        match self {
            Self::Banner => "banner",
            Self::BidderFailure => "bidder_failure",
            Self::BidderTimeout => "bidder_timeout",
            Self::MalformedRequest => "malformed_request",
            Self::MalformedConfig => "malformed_config",
            Self::DigestMismatch => "digest_mismatch",
            Self::DebugDenied => "debug_denied",
            Self::DebugRedacted => "debug_redacted",
            Self::InputCap => "input_cap",
            Self::OutputCap => "output_cap",
            Self::Headers => "headers",
            Self::Cancellation => "cancellation",
            Self::Completion => "completion",
            Self::ConfigReplacement => "config_replacement",
        }
    }

    fn no_dispatch(self) -> bool {
        matches!(
            self,
            Self::MalformedRequest | Self::MalformedConfig | Self::DigestMismatch | Self::InputCap
        )
    }

    fn status(self) -> u32 {
        match self {
            Self::MalformedConfig | Self::DigestMismatch => 500,
            Self::MalformedRequest | Self::InputCap => 400,
            Self::Banner
            | Self::BidderFailure
            | Self::BidderTimeout
            | Self::DebugDenied
            | Self::DebugRedacted
            | Self::OutputCap
            | Self::Headers
            | Self::Cancellation
            | Self::Completion
            | Self::ConfigReplacement => 200,
        }
    }

    fn bids(self) -> u32 {
        u32::from(matches!(
            self,
            Self::Banner
                | Self::DebugDenied
                | Self::DebugRedacted
                | Self::Headers
                | Self::Completion
                | Self::ConfigReplacement
        ))
    }

    fn failure(self) -> &'static str {
        match self {
            Self::BidderFailure => "bidder_failure",
            Self::BidderTimeout => "bidder_timeout",
            Self::Banner
            | Self::MalformedRequest
            | Self::MalformedConfig
            | Self::DigestMismatch
            | Self::DebugDenied
            | Self::DebugRedacted
            | Self::InputCap
            | Self::OutputCap
            | Self::Headers
            | Self::Cancellation
            | Self::Completion
            | Self::ConfigReplacement => "none",
        }
    }

    fn requires_mutation(self) -> bool {
        matches!(
            self,
            Self::MalformedConfig | Self::DigestMismatch | Self::ConfigReplacement
        )
    }

    fn refusal(self) -> &'static str {
        match self {
            Self::MalformedConfig => "malformed",
            Self::DigestMismatch => "integrity",
            Self::Banner
            | Self::BidderFailure
            | Self::BidderTimeout
            | Self::MalformedRequest
            | Self::DebugDenied
            | Self::DebugRedacted
            | Self::InputCap
            | Self::OutputCap
            | Self::Headers
            | Self::Cancellation
            | Self::Completion
            | Self::ConfigReplacement => "none",
        }
    }
}

#[derive(PartialEq, Eq, Serialize)]
struct RuntimeVersion {
    major: u32,
    minor: u32,
    patch: u32,
    // Structural identity distinguishes -0, even though JSON reports serialize it as 0.
    #[serde(skip)]
    negative_zero: [bool; 3],
}

#[derive(PartialEq, Eq, Serialize)]
struct Identity {
    auctioneer_commit: String,
    edgezero_pin: String,
    artifact_sha256: String,
    source_sha256: String,
    build_features: [String; 2],
    target: String,
    runtime_version: RuntimeVersion,
    harness_sha256: String,
    fixtures_sha256: String,
    reference_data_sha256: String,
    baked_config_sha256: String,
    fixture_metadata_sha256: String,
}

#[derive(Serialize)]
struct Environment {
    kind: &'static str,
    deployment_sha256: String,
    binding_sha256: String,
}

#[derive(Serialize)]
struct VerifiedCase {
    id: u8,
    kind: CaseKind,
    status: &'static str,
    dispatches: u32,
}

#[derive(Serialize)]
struct OfflineReport {
    schema_version: u8,
    kind: &'static str,
    offline_checks: &'static str,
    deployed_handler_acceptance: &'static str,
    resource_qualification: &'static str,
    publisher_pilot_ready: bool,
    identity: Identity,
    environment: Environment,
    cases: Vec<VerifiedCase>,
}

// Value fields defer semantic validation until the source contract's ordered gates.
fn scoped<T: DeserializeOwned>(value: Value, fields: &[&str], code: &'static str) -> Result<T> {
    let object = value.as_object().ok_or(HarnessError::new(code))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(HarnessError::new(code));
    }
    serde_json::from_value(value).map_err(|_error| HarnessError::new(code))
}

fn integer(raw_value: &Value, min: u32, max: u32) -> Result<u32> {
    let error = HarnessError::new("invalid_input");
    let number = raw_value.as_number().ok_or(error)?;
    let parsed = if let Some(unsigned) = number.as_u64() {
        u32::try_from(unsigned).map_err(|_error| error)?
    } else {
        // Integral JSON floats retain the source contract without lossy casts.
        let float_value = number.as_f64().ok_or(error)?;
        if !float_value.is_finite()
            || float_value < f64::from(min)
            || float_value > f64::from(max)
            || float_value.fract().classify() != FpCategory::Zero
        {
            return Err(error);
        }
        if float_value.is_sign_negative() {
            0
        } else {
            format!("{float_value:.0}")
                .parse::<u32>()
                .map_err(|_error| error)?
        }
    };
    if !(min..=max).contains(&parsed) {
        return Err(error);
    }
    Ok(parsed)
}

fn hex_string(value: &Value, len: usize, code: &'static str) -> Result<String> {
    value
        .as_str()
        .filter(|text| {
            text.len() == len
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .map(str::to_owned)
        .ok_or(HarnessError::new(code))
}

fn digest(value: &Value) -> Result<String> {
    hex_string(value, 64, "invalid_input")
}

fn validate_environment(value: Value) -> Result<Environment> {
    let raw: EnvironmentInput = scoped(
        value,
        &["kind", "deployment_sha256", "binding_sha256"],
        "missing_environment",
    )?;
    if raw.kind != "isolated_staging" {
        return Err(HarnessError::new("missing_environment"));
    }
    Ok(Environment {
        kind: "isolated_staging",
        deployment_sha256: digest(&raw.deployment_sha256)?,
        binding_sha256: digest(&raw.binding_sha256)?,
    })
}

fn validate_identity(value: Value, provider: Provider) -> Result<Identity> {
    let raw: IdentityInput = scoped(
        value,
        &[
            "auctioneer_commit",
            "edgezero_pin",
            "artifact_sha256",
            "source_sha256",
            "build_features",
            "target",
            "runtime_version",
            "harness_sha256",
            "fixtures_sha256",
            "reference_data_sha256",
            "baked_config_sha256",
            "fixture_metadata_sha256",
        ],
        "invalid_input",
    )?;
    let auctioneer_commit = hex_string(&raw.auctioneer_commit, 40, "identity_mismatch")?;
    if raw.edgezero_pin != SDK_PIN || raw.target != provider.target() {
        return Err(HarnessError::new("identity_mismatch"));
    }
    let artifact_sha256 = digest(&raw.artifact_sha256)?;
    let source_sha256 = digest(&raw.source_sha256)?;
    let harness_sha256 = digest(&raw.harness_sha256)?;
    let fixtures_sha256 = digest(&raw.fixtures_sha256)?;
    let reference_data_sha256 = digest(&raw.reference_data_sha256)?;
    let baked_config_sha256 = digest(&raw.baked_config_sha256)?;
    let fixture_metadata_sha256 = digest(&raw.fixture_metadata_sha256)?;
    let build_features: [String; 2] = serde_json::from_value(raw.build_features)
        .map_err(|_error| HarnessError::new("build_feature_missing"))?;
    if !(build_features == [provider.name(), OVERRIDE_FEATURE]
        || build_features == [OVERRIDE_FEATURE, provider.name()])
    {
        return Err(HarnessError::new("build_feature_missing"));
    }
    let runtime: RuntimeInput = scoped(
        raw.runtime_version,
        &["major", "minor", "patch"],
        "invalid_input",
    )?;
    Ok(Identity {
        auctioneer_commit,
        edgezero_pin: SDK_PIN.to_owned(),
        artifact_sha256,
        source_sha256,
        build_features,
        target: provider.target().to_owned(),
        runtime_version: RuntimeVersion {
            major: integer(&runtime.major, 0, u32::from(u16::MAX))?,
            minor: integer(&runtime.minor, 0, u32::from(u16::MAX))?,
            patch: integer(&runtime.patch, 0, u32::from(u16::MAX))?,
            negative_zero: [&runtime.major, &runtime.minor, &runtime.patch].map(|component| {
                component.as_f64().is_some_and(|number| {
                    number.is_sign_negative() && number.classify() == FpCategory::Zero
                })
            }),
        },
        harness_sha256,
        fixtures_sha256,
        reference_data_sha256,
        baked_config_sha256,
        fixture_metadata_sha256,
    })
}

fn mutation_gate(value: Value) -> Result<MutationInput> {
    let mutation: MutationInput = scoped(
        value,
        &[
            "isolated",
            "authorized",
            "addressing_verified",
            "known_good_observed",
            "recovery_armed",
            "injection",
            "write_visible",
            "handler_observed",
            "refusal",
            "restoration",
            "recovered",
            "interrupted",
        ],
        "invalid_input",
    )?;
    if mutation.isolated != true
        || mutation.authorized != true
        || mutation.addressing_verified != true
    {
        return Err(HarnessError::new("mutation_not_isolated"));
    }
    if mutation.known_good_observed != true || mutation.recovery_armed != true {
        return Err(HarnessError::new("restoration_failed"));
    }
    Ok(mutation)
}

fn validate_mutation(value: Value, kind: CaseKind) -> Result<()> {
    let mutation = mutation_gate(value)?;
    if mutation.restoration != "typed_push"
        || mutation.recovered != true
        || !mutation.interrupted.is_boolean()
    {
        return Err(HarnessError::new("restoration_failed"));
    }
    if mutation.injection != "provider_write"
        || mutation.write_visible != true
        || mutation.handler_observed != true
        || mutation.refusal != kind.refusal()
    {
        return Err(HarnessError::new("injection_unobserved"));
    }
    Ok(())
}

fn evaluate_case(value: Value, kind: CaseKind, id: u8) -> Result<VerifiedCase> {
    let case: CaseInput = scoped(
        value,
        &[
            "id",
            "kind",
            "transport",
            "status",
            "dispatch_before",
            "dispatch_after",
            "fixture_case_id",
            "bid_count",
            "failure",
            "assertions_verified",
            "mutation",
        ],
        "invalid_input",
    )?;
    if integer(&case.id, 1, 14).ok() != Some(u32::from(id)) || case.kind != kind.name() {
        return Err(HarnessError::new("missing_cases"));
    }
    let before = integer(&case.dispatch_before, 0, 1_000_000)?;
    let after = integer(&case.dispatch_after, 0, 1_000_000)?;
    let bids = integer(&case.bid_count, 0, 1_000_000)?;
    let fixture_id = integer(&case.fixture_case_id, 0, 1_000_000)?;
    let status = integer(&case.status, 0, 599)?;
    if case.transport == "timeout" {
        return Err(HarnessError::new("request_timeout"));
    }
    if case.transport != "http" {
        return Err(HarnessError::new("unexpected_transport"));
    }
    if case.mutation.is_null() {
        if kind.requires_mutation() {
            return Err(HarnessError::new("injection_unobserved"));
        }
    } else {
        validate_mutation(case.mutation, kind)?;
    }
    if status != kind.status() {
        return Err(HarnessError::new("http_refusal"));
    }
    let dispatches = after.checked_sub(before);
    if !kind.no_dispatch() && (dispatches != Some(1) || fixture_id != u32::from(id)) {
        return Err(HarnessError::new("dispatch_missing"));
    }
    if kind.no_dispatch() && after != before {
        return Err(HarnessError::new("unexpected_outcome"));
    }
    if bids != kind.bids() || case.failure != kind.failure() || case.assertions_verified != true {
        return Err(HarnessError::new("unexpected_outcome"));
    }
    Ok(VerifiedCase {
        id,
        kind,
        status: "verified",
        dispatches: u32::from(!kind.no_dispatch()),
    })
}

/// Evaluate supplied offline evidence without granting deployed-handler or publisher acceptance.
///
/// # Errors
/// Returns only a fixed failure category for invalid inputs or unverified controls.
pub fn evaluate_offline(value: Value) -> Result<Value> {
    let input: OfflineInput = scoped(
        value,
        &[
            "schema_version",
            "kind",
            "provider",
            "environment",
            "identity",
            "expected_identity",
            "cases",
        ],
        "invalid_input",
    )?;
    if integer(&input.schema_version, 1, 1).is_err() || input.kind != "offline_handler_control" {
        return Err(HarnessError::new("invalid_input"));
    }
    let provider = Provider::parse(&input.provider)?;
    let environment = validate_environment(input.environment)?;
    let identity = validate_identity(input.identity, provider)?;
    let expected_identity = validate_identity(input.expected_identity, provider)?;
    if identity != expected_identity {
        return Err(HarnessError::new("identity_mismatch"));
    }
    let Value::Array(case_inputs) = input.cases else {
        return Err(HarnessError::new("missing_cases"));
    };
    if case_inputs.len() != CASES.len() {
        return Err(HarnessError::new("missing_cases"));
    }
    let verified_cases = CASES
        .into_iter()
        .zip(case_inputs)
        .zip(1_u8..)
        .map(|((kind, case_input), id)| evaluate_case(case_input, kind, id))
        .collect::<Result<Vec<_>>>()?;
    serde_json::to_value(OfflineReport {
        schema_version: 1,
        kind: "offline_handler_control",
        offline_checks: "pass",
        deployed_handler_acceptance: "pending",
        resource_qualification: "not_evaluated",
        publisher_pilot_ready: false,
        identity,
        environment,
        cases: verified_cases,
    })
    .map_err(|_error| HarnessError::new("invalid_input"))
}

#[cfg(test)]
mod tests {
    use crate::qualification::harness::providers::evaluate_offline;
    use serde_json::{Value, json};

    const CASE_KINDS: [&str; 14] = [
        "banner",
        "bidder_failure",
        "bidder_timeout",
        "malformed_request",
        "malformed_config",
        "digest_mismatch",
        "debug_denied",
        "debug_redacted",
        "input_cap",
        "output_cap",
        "headers",
        "cancellation",
        "completion",
        "config_replacement",
    ];
    const HASH_FIELDS: [&str; 7] = [
        "artifact_sha256",
        "source_sha256",
        "harness_sha256",
        "fixtures_sha256",
        "reference_data_sha256",
        "baked_config_sha256",
        "fixture_metadata_sha256",
    ];
    const CASE_FIELDS: [&str; 11] = [
        "id",
        "kind",
        "transport",
        "status",
        "dispatch_before",
        "dispatch_after",
        "fixture_case_id",
        "bid_count",
        "failure",
        "assertions_verified",
        "mutation",
    ];
    const MUTATION_FIELDS: [&str; 12] = [
        "isolated",
        "authorized",
        "addressing_verified",
        "known_good_observed",
        "recovery_armed",
        "injection",
        "write_visible",
        "handler_observed",
        "refusal",
        "restoration",
        "recovered",
        "interrupted",
    ];

    pub(super) fn proof() -> Value {
        json!({"isolated":true,"authorized":true,"addressing_verified":true,
            "known_good_observed":true,"recovery_armed":true,"injection":"provider_write",
            "write_visible":true,"handler_observed":true,"refusal":"integrity",
            "restoration":"typed_push","recovered":true,"interrupted":false})
    }

    fn identity() -> Value {
        json!({"auctioneer_commit":"a".repeat(40_usize),
            "edgezero_pin":"598f0a2a490eb83dd31c06c024a0cefa8e049628",
            "artifact_sha256":"a".repeat(64_usize),"source_sha256":"a".repeat(64_usize),
            "build_features":["spin","auctioneer-config-data/test-harness-override"],"target":"wasm32-wasip2",
            "runtime_version":{"major":3_usize,"minor":0_usize,"patch":0_usize},"harness_sha256":"a".repeat(64_usize),
            "fixtures_sha256":"a".repeat(64_usize),"reference_data_sha256":"a".repeat(64_usize),
            "baked_config_sha256":"a".repeat(64_usize),"fixture_metadata_sha256":"a".repeat(64_usize)})
    }

    fn input() -> Value {
        let cases: Vec<_> = CASE_KINDS.iter().enumerate().map(|(i, kind)| {
            // Expectations are independent of the evaluator's case table.
            let status = [200_u32,200,200,400,500,500,200,200,400,200,200,200,200,200][i];
            let dispatches = [1_u32,1,1,0,0,0,1,1,0,1,1,1,1,1][i];
            let bids = [1_u32,0,0,0,0,0,1,1,0,0,1,0,1,1][i];
            let mut mutation = Value::Null;
            if [4,5,13].contains(&i) {
                mutation = proof();
                mutation["refusal"] = json!(match i { 4_usize => "malformed", 5_usize => "integrity", _ => "none" });
            }
            json!({"id":i.checked_add(1_usize).unwrap(),"kind":kind,"transport":"http","status":status,
                "dispatch_before":0_usize,"dispatch_after":dispatches,"fixture_case_id":i.checked_add(1_usize).unwrap(),
                "bid_count":bids,"failure":match i {1_usize=>"bidder_failure",2_usize=>"bidder_timeout",_=>"none"},
                "assertions_verified":true,"mutation":mutation})
        }).collect();
        json!({"schema_version":1_usize,"kind":"offline_handler_control","provider":"spin",
            "environment":{"kind":"isolated_staging","deployment_sha256":"a".repeat(64_usize),"binding_sha256":"a".repeat(64_usize)},
            "identity":identity(),"expected_identity":identity(),"cases":cases})
    }

    fn refuses(value: Value, code: &str) {
        let error = evaluate_offline(value).unwrap_err();
        assert_eq!(error.code, code);
        let output = serde_json::to_string(&error).unwrap();
        assert!(output.len() < 128);
        for canary in [
            "SECRET_CANARY",
            "password",
            "https:",
            "provider_error",
            "stack",
            "control.json",
        ] {
            assert!(!output.contains(canary));
        }
    }

    fn mirrored(value: &mut Value) {
        value["expected_identity"] = value["identity"].clone();
    }

    #[test]
    fn offline_report_verifies_fourteen_ordered_cases_without_live_acceptance() {
        let report = evaluate_offline(input()).unwrap();
        assert_eq!(report["schema_version"], 1_u32);
        assert_eq!(report["kind"], "offline_handler_control");
        assert_eq!(report["offline_checks"], "pass");
        assert_eq!(report["deployed_handler_acceptance"], "pending");
        assert_eq!(report["resource_qualification"], "not_evaluated");
        assert_eq!(report["publisher_pilot_ready"], false);
        assert_eq!(report["identity"], identity());
        assert_eq!(report.as_object().unwrap().len(), 9);
        let rows = report["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 14);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(
                *row,
                json!({"id":i.checked_add(1_usize).unwrap(),"kind":CASE_KINDS[i],"status":"verified",
                "dispatches":([1_usize,1_usize,1_usize,0_usize,0_usize,0_usize,1_usize,1_usize,0_usize,1_usize,1_usize,1_usize,1_usize,1_usize][i])})
            );
        }
    }

    #[test]
    fn identities_bind_every_digest_commit_sdk_target_and_runtime_component() {
        let mut checked = 0_usize;
        for field in HASH_FIELDS {
            let mut changed_digest = input();
            changed_digest["identity"][field] = json!("b".repeat(64_usize));
            refuses(changed_digest, "identity_mismatch");
            checked = checked.checked_add(1_usize).unwrap();
            for bad in [
                json!(null),
                json!(9_usize),
                json!(["a".repeat(64_usize)]),
                json!("A".repeat(64_usize)),
                json!("g".repeat(64_usize)),
                json!("a".repeat(63_usize)),
                json!("a".repeat(65_usize)),
            ] {
                let mut malformed_digest = input();
                malformed_digest["identity"][field] = bad;
                mirrored(&mut malformed_digest);
                refuses(malformed_digest, "invalid_input");
                checked = checked.checked_add(1_usize).unwrap();
            }
        }
        assert_eq!(checked, 56);
        for (field, bad) in [
            ("auctioneer_commit", json!(["a".repeat(40_usize)])),
            ("auctioneer_commit", json!("A".repeat(40_usize))),
            ("auctioneer_commit", json!("g".repeat(40_usize))),
            ("auctioneer_commit", json!("a".repeat(39_usize))),
            ("edgezero_pin", json!("b".repeat(40_usize))),
            ("target", json!("wasm32-wasip1")),
            ("target", json!(["wasm32-wasip2"])),
        ] {
            let mut malformed_binding = input();
            malformed_binding["identity"][field] = bad;
            mirrored(&mut malformed_binding);
            refuses(malformed_binding, "identity_mismatch");
        }
        for component in ["major", "minor", "patch"] {
            let mut max_runtime = input();
            max_runtime["identity"]["runtime_version"][component] = json!(u16::MAX);
            mirrored(&mut max_runtime);
            evaluate_offline(max_runtime).unwrap();
            for bad in [
                json!(-1_i32),
                json!(1_u32 << 16_u32),
                json!(0.5_f64),
                json!("3"),
                json!(true),
                json!(null),
            ] {
                let mut malformed_runtime = input();
                malformed_runtime["identity"]["runtime_version"][component] = bad;
                mirrored(&mut malformed_runtime);
                refuses(malformed_runtime, "invalid_input");
            }
        }
        let mut joined_runtime = input();
        joined_runtime["identity"]["runtime_version"] = json!({"major,minor,patch":1_usize});
        mirrored(&mut joined_runtime);
        refuses(joined_runtime, "invalid_input");
    }

    #[test]
    fn features_require_exact_provider_and_override_without_development_features() {
        for (provider, target) in [
            ("cloudflare", "wasm32-unknown-unknown"),
            ("fastly", "wasm32-wasip1"),
            ("spin", "wasm32-wasip2"),
        ] {
            for features in [
                json!([provider, "auctioneer-config-data/test-harness-override"]),
                json!(["auctioneer-config-data/test-harness-override", provider]),
            ] {
                let mut provider_evidence = input();
                provider_evidence["provider"] = json!(provider);
                provider_evidence["identity"]["target"] = json!(target);
                provider_evidence["identity"]["build_features"] = features;
                mirrored(&mut provider_evidence);
                evaluate_offline(provider_evidence).unwrap();
            }
        }
        for features in [
            json!(["spin"]),
            json!(["spin", "spin"]),
            json!(["fastly", "auctioneer-config-data/test-harness-override"]),
            json!([
                "spin",
                "auctioneer-config-data/test-harness-override",
                "insecure-bidders"
            ]),
            json!([
                "spin",
                "auctioneer-config-data/test-harness-override",
                "conformance-suite"
            ]),
            json!([
                "spin",
                "auctioneer-config-data/test-harness-override",
                "test-harness-override"
            ]),
            json!(null),
            json!("spin,auctioneer-config-data/test-harness-override"),
            json!(["spin", true]),
        ] {
            let mut invalid_features = input();
            invalid_features["identity"]["build_features"] = features;
            mirrored(&mut invalid_features);
            refuses(invalid_features, "build_feature_missing");
        }
        let mut reordered_features = input();
        reordered_features["expected_identity"]["build_features"] =
            json!(["auctioneer-config-data/test-harness-override", "spin"]);
        refuses(reordered_features, "identity_mismatch");
    }

    #[test]
    fn structural_identity_ignores_object_key_order_but_not_values() {
        let mut reordered = input();
        let original = reordered["identity"].as_object().unwrap();
        let identity_fields: serde_json::Map<_, _> = original
            .iter()
            .rev()
            .map(|(key, member)| (key.clone(), member.clone()))
            .collect();
        reordered["expected_identity"] = Value::Object(identity_fields);
        evaluate_offline(reordered).unwrap();
    }

    #[test]
    fn runtime_identity_compares_integral_numbers_without_losing_signed_zero() {
        let mut integral_runtime = input();
        integral_runtime["expected_identity"]["runtime_version"]["major"] = json!(3.0_f64);
        evaluate_offline(integral_runtime).unwrap();
        for component in ["major", "minor", "patch"] {
            let mut signed_zero = input();
            signed_zero["identity"]["runtime_version"][component] = json!(-0.0_f64);
            signed_zero["expected_identity"]["runtime_version"][component] = json!(0_usize);
            refuses(signed_zero, "identity_mismatch");
            let mut matching_signed_zero = input();
            matching_signed_zero["identity"]["runtime_version"][component] = json!(-0.0_f64);
            mirrored(&mut matching_signed_zero);
            evaluate_offline(matching_signed_zero).unwrap();
        }
    }

    #[test]
    fn expected_identity_is_independently_validated_before_comparison() {
        for (field, bad, code) in [
            (
                "artifact_sha256",
                json!("g".repeat(64_usize)),
                "invalid_input",
            ),
            (
                "auctioneer_commit",
                json!(["a".repeat(40_usize)]),
                "identity_mismatch",
            ),
            ("edgezero_pin", json!(null), "identity_mismatch"),
            ("target", json!(null), "identity_mismatch"),
            ("build_features", json!(["spin"]), "build_feature_missing"),
            (
                "runtime_version",
                json!({"major,minor,patch":1_usize}),
                "invalid_input",
            ),
        ] {
            let mut invalid_expected = input();
            invalid_expected["expected_identity"][field] = bad;
            refuses(invalid_expected, code);
        }
    }

    #[test]
    fn required_scope_checks_refuse_missing_extra_and_comma_joined_keys() {
        assert_eq!(
            input()["cases"][0].as_object().unwrap().len(),
            CASE_FIELDS.len()
        );
        assert_eq!(proof().as_object().unwrap().len(), MUTATION_FIELDS.len());
        let scopes = [
            "",
            "/environment",
            "/identity",
            "/expected_identity",
            "/identity/runtime_version",
            "/cases/0",
            "/cases/4/mutation",
        ];
        let mut checked = 0_usize;
        for scope in scopes {
            let original = input();
            let fields: Vec<_> = original
                .pointer(scope)
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            let code = if scope == "/environment" {
                "missing_environment"
            } else {
                "invalid_input"
            };
            for field in &fields {
                let mut missing_field = input();
                missing_field
                    .pointer_mut(scope)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(field);
                refuses(missing_field, code);
                checked = checked.checked_add(1_usize).unwrap();
            }
            let mut extra_field = input();
            extra_field.pointer_mut(scope).unwrap()["SECRET_CANARY"] =
                json!("https://user:password@provider.invalid/control.json");
            refuses(extra_field, code);
            checked = checked.checked_add(1_usize).unwrap();
            let mut joined_fields = input();
            *joined_fields.pointer_mut(scope).unwrap() = json!({fields.join(","):1_usize});
            refuses(joined_fields, code);
            checked = checked.checked_add(1_usize).unwrap();
        }
        assert_eq!(checked, 74);
    }

    #[test]
    fn malformed_top_level_environment_identity_and_case_containers_refuse_safely() {
        for bad in [json!(null), json!([]), json!(true), json!("SECRET_CANARY")] {
            refuses(bad, "invalid_input");
        }
        for (field, bad, code) in [
            ("schema_version", json!(2_usize), "invalid_input"),
            ("schema_version", json!("1"), "invalid_input"),
            ("kind", json!("live"), "invalid_input"),
            ("provider", json!("unknown"), "invalid_input"),
            ("provider", json!(["spin"]), "invalid_input"),
            ("environment", json!(null), "missing_environment"),
            ("environment", json!([]), "missing_environment"),
            ("identity", json!(null), "invalid_input"),
            ("expected_identity", json!([]), "invalid_input"),
            ("cases", json!(null), "missing_cases"),
            ("cases", json!({}), "missing_cases"),
        ] {
            let mut bad_container = input();
            bad_container[field] = bad;
            refuses(bad_container, code);
        }
        let mut production_environment = input();
        production_environment["environment"]["kind"] = json!("production");
        refuses(production_environment, "missing_environment");
        for field in ["deployment_sha256", "binding_sha256"] {
            for bad in [
                json!(null),
                json!("SECRET_CANARY"),
                json!("A".repeat(64_usize)),
            ] {
                let mut bad_digest = input();
                bad_digest["environment"][field] = bad;
                refuses(bad_digest, "invalid_input");
            }
        }
        for scope in ["/cases/0", "/cases/4/mutation", "/identity/runtime_version"] {
            for bad in [json!(null), json!([]), json!(false)] {
                let code = if scope == "/cases/4/mutation" && bad.is_null() {
                    "injection_unobserved"
                } else {
                    "invalid_input"
                };
                let mut bad_scope = input();
                *bad_scope.pointer_mut(scope).unwrap() = bad;
                refuses(bad_scope, code);
            }
        }
    }

    #[test]
    fn every_case_requires_exact_order_dispatch_status_and_outcome() {
        let mut checked = 0_usize;
        for i in 0..14 {
            for (field, bad, code) in [
                ("id", json!(0_usize), "missing_cases"),
                ("kind", json!("SECRET_CANARY"), "missing_cases"),
                ("status", json!(503_usize), "http_refusal"),
                ("transport", json!("timeout"), "request_timeout"),
                ("transport", json!("reset"), "unexpected_transport"),
                ("transport", json!(null), "unexpected_transport"),
                ("bid_count", json!(99_usize), "unexpected_outcome"),
                (
                    "bid_count",
                    json!(usize::from(
                        [
                            1_usize, 0_usize, 0_usize, 0_usize, 0_usize, 0_usize, 1_usize, 1_usize,
                            0_usize, 0_usize, 1_usize, 0_usize, 1_usize, 1_usize
                        ][i] == 0_usize
                    )),
                    "unexpected_outcome",
                ),
                ("failure", json!("SECRET_CANARY"), "unexpected_outcome"),
                (
                    "failure",
                    json!(if [1_usize, 2_usize].contains(&i) {
                        "none"
                    } else {
                        "bidder_failure"
                    }),
                    "unexpected_outcome",
                ),
                ("assertions_verified", json!(1_usize), "unexpected_outcome"),
                ("assertions_verified", json!(false), "unexpected_outcome"),
            ] {
                let mut bad_case = input();
                bad_case["cases"][i][field] = bad;
                refuses(bad_case, code);
                checked = checked.checked_add(1_usize).unwrap();
            }
            let no_dispatch = [3, 4, 5, 8].contains(&i);
            let mut wrong_dispatch = input();
            wrong_dispatch["cases"][i]["dispatch_after"] = json!(usize::from(no_dispatch));
            refuses(
                wrong_dispatch,
                if no_dispatch {
                    "unexpected_outcome"
                } else {
                    "dispatch_missing"
                },
            );
            checked = checked.checked_add(1_usize).unwrap();
            if !no_dispatch {
                let mut wrong_fixture = input();
                wrong_fixture["cases"][i]["fixture_case_id"] = json!(0_usize);
                refuses(wrong_fixture, "dispatch_missing");
                checked = checked.checked_add(1_usize).unwrap();
                let mut backward_dispatch = input();
                backward_dispatch["cases"][i]["dispatch_before"] = json!(2_usize);
                backward_dispatch["cases"][i]["dispatch_after"] = json!(1_usize);
                refuses(backward_dispatch, "dispatch_missing");
                checked = checked.checked_add(1_usize).unwrap();
            }
        }
        assert_eq!(checked, 202);
        let mut missing_case = input();
        missing_case["cases"].as_array_mut().unwrap().pop();
        refuses(missing_case, "missing_cases");
        let mut duplicate_case = input();
        duplicate_case["cases"][1] = duplicate_case["cases"][0].clone();
        refuses(duplicate_case, "missing_cases");
        let mut extra_case = input();
        extra_case["cases"]
            .as_array_mut()
            .unwrap()
            .push(json!(null));
        refuses(extra_case, "missing_cases");
    }

    #[test]
    fn counters_and_status_use_bounded_integer_contract_before_transport_checks() {
        let mut checked = 0_usize;
        for field in [
            "dispatch_before",
            "dispatch_after",
            "fixture_case_id",
            "bid_count",
            "status",
        ] {
            for bad in [
                json!(-1_i32),
                json!(1.5_f64),
                json!(true),
                json!(null),
                json!("1"),
                json!(1_000_001_usize),
            ] {
                let mut invalid_counter = input();
                invalid_counter["cases"][0][field] = bad;
                invalid_counter["cases"][0]["transport"] = json!("timeout");
                refuses(invalid_counter, "invalid_input");
                checked = checked.checked_add(1_usize).unwrap();
            }
        }
        assert_eq!(checked, 30);
        let mut invalid_status = input();
        invalid_status["cases"][0]["status"] = json!(600_usize);
        refuses(invalid_status, "invalid_input");
        let mut max_counters = input();
        max_counters["cases"][0]["dispatch_before"] = json!(999_999_usize);
        max_counters["cases"][0]["dispatch_after"] = json!(1_000_000_usize);
        evaluate_offline(max_counters).unwrap();
        let mut integral_counters = input();
        integral_counters["schema_version"] = json!(1.0_f64);
        integral_counters["cases"][0]["id"] = json!(1.0_f64);
        integral_counters["cases"][0]["status"] = json!(200.0_f64);
        integral_counters["cases"][0]["dispatch_before"] = json!(-0.0_f64);
        integral_counters["cases"][0]["dispatch_after"] = json!(1.0_f64);
        evaluate_offline(integral_counters).unwrap();
    }

    #[test]
    fn mutation_controls_require_isolation_armed_restore_and_observed_typed_recovery() {
        let mut checked = 0_usize;
        for i in [4, 5, 13] {
            let mut missing_proof = input();
            missing_proof["cases"][i]["mutation"] = Value::Null;
            refuses(missing_proof, "injection_unobserved");
            checked = checked.checked_add(1_usize).unwrap();
            for (field, code) in [
                ("isolated", "mutation_not_isolated"),
                ("authorized", "mutation_not_isolated"),
                ("addressing_verified", "mutation_not_isolated"),
                ("known_good_observed", "restoration_failed"),
                ("recovery_armed", "restoration_failed"),
                ("recovered", "restoration_failed"),
                ("write_visible", "injection_unobserved"),
                ("handler_observed", "injection_unobserved"),
            ] {
                for bad in [json!(false), json!(1_usize), json!(null), json!("true")] {
                    let mut invalid_flag = input();
                    invalid_flag["cases"][i]["mutation"][field] = bad;
                    refuses(invalid_flag, code);
                    checked = checked.checked_add(1_usize).unwrap();
                }
            }
            for (field, bad, code) in [
                ("restoration", json!("direct_write"), "restoration_failed"),
                ("interrupted", json!(1_usize), "restoration_failed"),
                (
                    "injection",
                    json!("push_validation"),
                    "injection_unobserved",
                ),
                ("refusal", json!("missing_chunk"), "injection_unobserved"),
            ] {
                let mut invalid_proof = input();
                invalid_proof["cases"][i]["mutation"][field] = bad;
                refuses(invalid_proof, code);
                checked = checked.checked_add(1_usize).unwrap();
            }
            let mut interrupted = input();
            interrupted["cases"][i]["mutation"]["interrupted"] = json!(true);
            let report = evaluate_offline(interrupted.clone()).unwrap();
            assert_eq!(report["deployed_handler_acceptance"], "pending");
            assert_eq!(report["publisher_pilot_ready"], false);
            interrupted["cases"][i]["mutation"]["recovered"] = json!(false);
            refuses(interrupted, "restoration_failed");
            checked = checked.checked_add(1_usize).unwrap();
        }
        assert_eq!(checked, 114);
        // An unsolicited proof is still validated on an ordinary case.
        let mut unexpected_proof = input();
        unexpected_proof["cases"][0]["mutation"] = proof();
        refuses(unexpected_proof, "injection_unobserved");
        let mut ordinary_proof = input();
        ordinary_proof["cases"][0]["mutation"] = proof();
        ordinary_proof["cases"][0]["mutation"]["refusal"] = json!("none");
        evaluate_offline(ordinary_proof).unwrap();
    }

    #[test]
    fn failure_precedence_and_untrusted_report_fields_remain_fixed() {
        let mut forged_readiness = input();
        forged_readiness["publisher_pilot_ready"] = json!(true);
        refuses(forged_readiness, "invalid_input");
        let mut secret_error = input();
        secret_error["cases"][0]["provider_error"] =
            json!("SECRET_CANARY https://user:password@provider.invalid/stack");
        refuses(secret_error, "invalid_input");
        let mut unauthorized = input();
        unauthorized["cases"][4]["mutation"]["isolated"] = json!(false);
        unauthorized["cases"][4]["mutation"]["recovered"] = json!(false);
        refuses(unauthorized, "mutation_not_isolated");
        let mut failed_restore = input();
        failed_restore["cases"][4]["mutation"]["restoration"] = json!("direct_write");
        failed_restore["cases"][4]["mutation"]["injection"] = json!("push_validation");
        refuses(failed_restore, "restoration_failed");
        let mut timed_out = input();
        timed_out["cases"][4]["transport"] = json!("timeout");
        timed_out["cases"][4]["mutation"] = json!({});
        refuses(timed_out, "request_timeout");
    }
}

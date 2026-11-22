//! Exact closed-pair observations in fresh processes of this existing test binary.
//! The reference observation is only an equal-input proxy of current constructors.
//! No historical code, Cargo child build, cloned cold metadata or common C/I credit.

#![cfg(all(
    test,
    not(target_arch = "wasm32"),
    not(feature = "schema-artifact-staging")
))]

use super::{expanded::digest, growth::read_regular as model_bytes};
use crate::schema_build;
use crate::schema_meter::{Metrics, measure};
use auctioneer_bidder::{compiled_bidders, configured_registry};
use auctioneer_config::codegen::storage::metadata_source_sha256;
use auctioneer_config_data::{bidder_implementation_bindings, bidder_metadata_storage, bidders};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::hint::black_box;
use std::io::Read as _;
use std::path::Path;
use std::process::Command;
use std::ptr;
use std::str;
use std::time::Duration;

const CHILD_ENV: &str = "AUCTIONEER__TEST__METADATA_REGISTRY_CHILD";
const MARKER: &str = "AUCTIONEER_METADATA_REGISTRY_REPORT=";
const STAGING: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/schema-staging.json"));
const LAYOUT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/schema-layout-build.toml"));

macro_rules! models {
    ($($path:literal),+ $(,)?) => {
        &[$(($path, include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"), "/../../", $path
        )).as_slice())),+]
    };
}
const MODELS: &[(&str, &[u8])] = models!(
    "Cargo.toml",
    "Cargo.lock",
    "crates/auctioneer-config-data/build.rs",
    "crates/auctioneer-config-data/Cargo.toml",
    "crates/auctioneer-config-data/src/lib.rs",
    "crates/auctioneer-config/Cargo.toml",
    "crates/auctioneer-config/src/lib.rs",
    "crates/auctioneer-config/src/codegen/mod.rs",
    "crates/auctioneer-config/src/codegen/storage.rs",
    "crates/auctioneer-config/src/types.rs",
    "crates/auctioneer-config/src/bindings.rs",
    "crates/auctioneer-config/src/usersync.rs",
    "crates/auctioneer-config/src/activity.rs",
    "crates/auctioneer-config/src/limits.rs",
    "crates/auctioneer-config/src/url_safety.rs",
    "crates/auctioneer-bidder/Cargo.toml",
    "crates/auctioneer-bidder/src/lib.rs",
    "crates/auctioneer-bidder/src/types.rs",
    "crates/auctioneer-bidder/src/schema/mod.rs",
    "crates/auctioneer-bidder/src/registry/mod.rs",
    "crates/auctioneer-bidder/src/registry/catalog.rs",
    "crates/auctioneer-bidder/tests/schema_build/mod.rs",
    "crates/auctioneer-bidder/tests/schema_meter/mod.rs",
    "crates/auctioneer-bidder/tests/schema_residency/growth.rs",
    "crates/auctioneer-bidder/tests/schema_residency/metadata_registry.rs",
);

/// Original current-generator strings, in native production order.
/// The caller first verifies complete reference/current manifest witness equality
/// and the current generator/feature tuple. No input discovery or fallback occurs.
pub(super) struct Reconstruction {
    pub bindings: String,
    pub constructors: Vec<String>,
    pub declaration_count: u64,
    pub input_fingerprint: String,
    pub measurement_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Slot {
    ReferenceProxy,
    Candidate,
    BaselineFirst,
    BaselineSecond,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request {
    slot: Slot,
    input_fingerprint: String,
    measurement_fingerprint: String,
    context: Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ChildReport {
    request: Request,
    observation: Value,
}

fn fingerprint(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid matched metadata fingerprint".into());
    }
    Ok(())
}

fn construction_sha(input: &Reconstruction) -> Result<[u8; 32], String> {
    let declarations = usize::try_from(input.declaration_count)
        .map_err(|error| format!("metadata declaration count: {error}"))?;
    metadata_source_sha256(&input.constructors, &input.bindings, declarations)
        .map_err(|error| error.to_string())
}

fn executable_sha() -> Result<String, String> {
    let mut file = fs::File::open(env::current_exe().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let length = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if length == 0 {
            break;
        }
        hasher.update(
            buffer
                .get(..length)
                .ok_or("invalid executable read length")?,
        );
    }
    Ok(hex::encode(hasher.finalize()))
}

fn context() -> Result<Value, String> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("missing metadata measurement workspace")?;
    let mut sources = BTreeMap::new();
    for &(relative, compiled) in MODELS {
        if model_bytes(&workspace.join(relative))? != compiled {
            return Err(format!(
                "metadata model differs from compiled bytes: {relative}"
            ));
        }
        sources.insert(relative, digest(compiled));
    }
    for (name, compiled) in [
        ("schema-staging.json", STAGING),
        ("schema-layout-build.toml", LAYOUT),
    ] {
        if model_bytes(&Path::new(env!("OUT_DIR")).join(name))? != compiled {
            return Err(format!(
                "metadata build output differs from compiled bytes: {name}"
            ));
        }
    }
    let manifest: Value = serde_json::from_slice(STAGING).map_err(|error| error.to_string())?;
    let descriptor = bidder_metadata_storage(); // Static; never forces bidders().
    Ok(
        json!({"executable_sha256":executable_sha()?,"model_sources":sources,
        "staging_sha256":digest(STAGING),"layout_toml":str::from_utf8(LAYOUT)
            .map_err(|error| error.to_string())?,
        "build":manifest.get("build").unwrap_or(&Value::Null),
        "environment":manifest.get("build_environment").unwrap_or(&Value::Null),
        "compiled_constructor_sha256":hex::encode(descriptor.source_sha256),
        "compiled_publication_count":descriptor.active_bidders,
        "compiled_binding_count":descriptor.active_bindings,"declarations":descriptor.declarations,
        "discovered_factory_keys":compiled_bidders().iter().map(|row| row.key).collect::<Vec<_>>(),
        "development":cfg!(feature = "insecure-bidders")}),
    )
}

fn exact_test() -> Result<String, String> {
    let (_target, module) = module_path!()
        .split_once("::")
        .ok_or("missing test module owner")?;
    Ok(format!("{module}::tests::cold_process"))
}

fn parse_child_report(success: bool, stdout: &str, stderr: &str) -> Result<ChildReport, String> {
    let running = stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| line.starts_with("running "))
        .collect::<Vec<_>>();
    let summaries = stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| line.starts_with("test result:"))
        .collect::<Vec<_>>();
    if !success || running.as_slice() != ["running 1 test"] || summaries.len() != 1 {
        return Err("matched metadata exact child did not execute once".into());
    }
    let suffix = summaries
        .first()
        .ok_or("matched metadata exact child did not execute once")?
        .strip_prefix("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ")
        .ok_or("matched metadata exact child did not execute once")?;
    let (filtered, elapsed) = suffix
        .split_once(" filtered out; finished in ")
        .ok_or("invalid matched metadata test summary")?;
    if filtered.parse::<usize>().is_err()
        || elapsed
            .strip_suffix('s')
            .and_then(|seconds| seconds.parse::<f64>().ok())
            .is_none_or(|seconds| !seconds.is_finite() || seconds < 0.0_f64)
    {
        return Err("invalid matched metadata test summary".into());
    }
    if stdout.contains(MARKER) || stderr.matches(MARKER).count() != 1 {
        return Err("missing or duplicate typed metadata child report".into());
    }
    let encoded = stderr
        .lines()
        .find_map(|line| line.strip_prefix(MARKER))
        .ok_or("missing or duplicate typed metadata child report")?;
    serde_json::from_str(encoded)
        .ok()
        .ok_or_else(|| "invalid typed metadata child report".to_owned())
}

fn child_request(value: Result<String, env::VarError>) -> Result<Option<Request>, String> {
    let encoded = match value {
        Ok(encoded) => encoded,
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(_error) => return Err("invalid metadata child sentinel".into()),
    };
    // A sentinel may contain arbitrary environment data; never echo its contents.
    let request: Request = serde_json::from_str(&encoded)
        .ok()
        .ok_or("invalid metadata child sentinel")?;
    if fingerprint(&request.input_fingerprint).is_err()
        || fingerprint(&request.measurement_fingerprint).is_err()
    {
        return Err("invalid metadata child sentinel".into());
    }
    Ok(Some(request))
}

fn child(request: &Request, timeout: Duration) -> Result<Value, String> {
    fingerprint(&request.input_fingerprint)?;
    fingerprint(&request.measurement_fingerprint)?;
    let mut command = Command::new(env::current_exe().map_err(|error| error.to_string())?);
    command
        .args(["--exact", &exact_test()?, "--nocapture", "--test-threads=1"])
        .env(
            CHILD_ENV,
            serde_json::to_string(request).map_err(|error| error.to_string())?,
        );
    let output = schema_build::run(&mut command, timeout)
        .map_err(|error| format!("matched metadata child runner: {error:?}"))?;
    let stdout = str::from_utf8(&output.stdout).map_err(|error| error.to_string())?;
    let stderr = str::from_utf8(&output.stderr).map_err(|error| error.to_string())?;
    let report = parse_child_report(output.status.success(), stdout, stderr)?;
    if report.request != *request || context()? != request.context {
        return Err("matched metadata child fingerprint or frozen context mismatch".into());
    }
    Ok(report.observation)
}

/// Observe current constructors twice for an exact closed equal-input pair.
/// The first observation proxies the reference input; no reference code executes.
/// Nonzero metadata/registry deltas are unsupported; byte drift always refuses.
/// Original strings must match each other AND the compiled production descriptor's
/// cryptographic source binding. The caller rechecks its frozen source snapshots
/// after return, before credit. Timeout is the existing owner's child budget.
pub(super) fn observe_matched(
    reference: &Reconstruction,
    candidate: &Reconstruction,
    timeout: Duration,
) -> Result<Value, String> {
    for value in [
        &reference.input_fingerprint,
        &candidate.input_fingerprint,
        &reference.measurement_fingerprint,
        &candidate.measurement_fingerprint,
    ] {
        fingerprint(value)?;
    }
    if reference.bindings != candidate.bindings
        || reference.constructors != candidate.constructors
        || reference.declaration_count != candidate.declaration_count
    {
        return Err("matched metadata constructor bytes differ".into());
    }
    if reference.measurement_fingerprint != candidate.measurement_fingerprint {
        return Err("matched metadata measurement tuple differs".into());
    }
    let descriptor = bidder_metadata_storage();
    if construction_sha(reference)? != descriptor.source_sha256
        || reference.declaration_count != descriptor.declarations
        || u64::try_from(reference.constructors.len()).map_err(|error| error.to_string())?
            != descriptor.active_bidders
        || descriptor.active_bindings != descriptor.active_bidders
    {
        return Err(
            "matched metadata bytes do not bind to compiled production constructors".into(),
        );
    }
    let frozen = context()?;
    let first = child(
        &Request {
            slot: Slot::ReferenceProxy,
            input_fingerprint: reference.input_fingerprint.clone(),
            measurement_fingerprint: reference.measurement_fingerprint.clone(),
            context: frozen.clone(),
        },
        timeout,
    )?;
    let second = child(
        &Request {
            slot: Slot::Candidate,
            input_fingerprint: candidate.input_fingerprint.clone(),
            measurement_fingerprint: candidate.measurement_fingerprint.clone(),
            context: frozen.clone(),
        },
        timeout,
    )?;
    if context()? != frozen {
        return Err("matched metadata measurement sources changed".into());
    }
    Ok(
        json!({"label":"same-build data-only source reconstruction relative to d26fcf7",
        "scope":{"reference":"equal-input-proxy-of-candidate",
            "candidate":"actual-current-constructor",
            "supports_nonzero_metadata_registry_delta":false},
        "reference":first,"candidate":second,"measurement":frozen,
        "proof":"reference is an equal-input proxy of candidate; original constructor/binding bytes and declaration counts equal; compiled storage source binding matches",
        "common_C_I_qualified":false,"matched_reconstruction_complete":false,"physical_or_serving_qualification":false}),
    )
}

fn zero(metrics: Metrics) -> bool {
    metrics.json()
        == json!({"allocated_bytes":0_u64,"peak_live_bytes":0_i64,
        "retained_bytes":0_i64,"allocation_calls":0_u64})
}

fn allocator_control() -> Result<(), String> {
    let (positive, allocated) = measure(|| black_box(vec![black_box(1_u8); 37]));
    let ((), released) = measure(|| drop(black_box(positive)));
    if allocated.json()
        != json!({"allocated_bytes":37_u64,"peak_live_bytes":37_i64,
        "retained_bytes":37_i64,"allocation_calls":1_u64})
        || released.json()
            != json!({"allocated_bytes":0_u64,"peak_live_bytes":0_i64,
            "retained_bytes":-37_i64,"allocation_calls":0_u64})
    {
        return Err("metadata allocator control metrics mismatch".into());
    }
    Ok(())
}

fn observe_current() -> Result<Value, String> {
    allocator_control()?;
    let (infos, metadata) = measure(|| black_box(bidders()));
    let (again, repeat) = measure(|| black_box(bidders()));
    if !ptr::eq(infos, again)
        || !zero(repeat)
        || metadata
            .json()
            .get("allocated_bytes")
            .and_then(Value::as_u64)
            .is_none_or(|value| value == 0)
        || metadata
            .json()
            .get("retained_bytes")
            .and_then(Value::as_i64)
            .is_none_or(|value| value <= 0)
    {
        return Err("production metadata was not cold followed by a zero repeat".into());
    }
    let (constructed, registry_cold) = measure(|| black_box(configured_registry()));
    let registry =
        constructed.map_err(|error| format!("production registry constructor: {error:?}"))?;
    let bindings = bidder_implementation_bindings();
    if registry.infos().len() != infos.len() || bindings.len() != infos.len() {
        return Err("production registry publication membership mismatch".into());
    }
    for info in infos {
        let (borrowed, _bidder) = registry
            .get(info.name.as_str())
            .ok_or("missing production registry row")?;
        if !ptr::eq(borrowed, info)
            || !bindings.iter().any(|row| {
                row.bidder == info.name.as_str()
                    && registry.implementation(row.bidder) == Some(row.implementation)
            })
        {
            return Err("production registry did not borrow exact metadata and binding".into());
        }
    }
    let ((), lookup) = measure(|| {
        for info in infos {
            black_box(registry.get(black_box(info.name.as_str())));
        }
    });
    if !zero(lookup) {
        return Err("production registry lookup allocated".into());
    }
    let ((), release) = measure(|| drop(black_box(registry)));
    let mut combined = metadata;
    combined.append(registry_cold);
    let metadata_json = metadata.json();
    let registry_json = registry_cold.json();
    let release_json = release.json();
    if registry_json
        .get("retained_bytes")
        .and_then(Value::as_i64)
        .and_then(|value| value.checked_add(release_json.get("retained_bytes")?.as_i64()?))
        != Some(0)
        || registry_json
            .get("allocated_bytes")
            .and_then(Value::as_u64)
            .is_none_or(|value| value == 0)
    {
        return Err("production registry release did not balance its actual constructor".into());
    }
    let prefix = metadata_json
        .get("retained_bytes")
        .and_then(Value::as_i64)
        .ok_or("invalid metadata prefix")?;
    let peak = registry_json
        .get("peak_live_bytes")
        .and_then(Value::as_i64)
        .ok_or("invalid registry peak")?;
    if u64::try_from(infos.len()).map_err(|error| error.to_string())?
        != bidder_metadata_storage().active_bidders
    {
        return Err("compiled publication descriptor count mismatch".into());
    }
    Ok(
        json!({"metadata_then_registry":combined.json(),"compiled_publication_count":infos.len(),
        "public_names":infos.iter().map(|info| info.name.as_str()).collect::<Vec<_>>(),
        "discovered_factory_count":compiled_bidders().len(),
        "discovered_factory_keys":compiled_bidders().iter().map(|row| row.key).collect::<Vec<_>>(),
        "metadata":{"cold":metadata_json,"repeat":repeat.json(),
            "lifetime":"actual production singleton retained to process exit; no clone or fake release"},
        "registry":{"cold":registry_json,"lookup":lookup.json(),"release":release_json,
            "metadata_prefix_retained_bytes":prefix,
            "prefix_plus_registry_peak_bytes":prefix.checked_add(peak).ok_or("metadata prefix overflow")?}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write as _};

    #[test]
    fn cold_process() -> Result<(), String> {
        if let Some(request) = child_request(env::var(CHILD_ENV))? {
            if context()? != request.context {
                return Err("metadata child fingerprint does not match this executable".into());
            }
            let observation = observe_current()?;
            if context()? != request.context {
                return Err("metadata child frozen sources changed".into());
            }
            writeln!(
                io::stderr().lock(),
                "{MARKER}{}",
                serde_json::to_string(&ChildReport {
                    request,
                    observation
                })
                .map_err(|error| error.to_string())?
            )
            .map_err(|error| error.to_string())?;
        } else {
            // Always execute two actual fresh children on standalone dispatch.
            // CURRENT baseline only: this is never a reference-pair fallback.
            let frozen = context()?;
            let baseline = digest(&serde_json::to_vec(&frozen).map_err(|error| error.to_string())?);
            for slot in [Slot::BaselineFirst, Slot::BaselineSecond] {
                let report = child(
                    &Request {
                        slot,
                        input_fingerprint: baseline.clone(),
                        measurement_fingerprint: baseline.clone(),
                        context: frozen.clone(),
                    },
                    Duration::from_secs(300),
                )?;
                if report
                    .get("compiled_publication_count")
                    .and_then(Value::as_u64)
                    .is_none_or(|count| count == 0)
                    || report
                        .get("discovered_factory_count")
                        .and_then(Value::as_u64)
                        .is_none_or(|count| count == 0)
                {
                    return Err(
                        "baseline metadata child publication or factory count is not positive"
                            .into(),
                    );
                }
            }
        }
        Ok(())
    }

    fn protocol_fixture() -> (Request, String, String) {
        let request = Request {
            slot: Slot::ReferenceProxy,
            input_fingerprint: "1".repeat(64),
            measurement_fingerprint: "2".repeat(64),
            context: json!({"fixture":true}),
        };
        let encoded = serde_json::to_string(&ChildReport {
            request: request.clone(),
            observation: json!({"fixture":true}),
        })
        .unwrap();
        let stdout = concat!(
            "running 1 test\n",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ",
            "3 filtered out; finished in 0.00s\n"
        )
        .to_owned();
        (request, stdout, format!("{MARKER}{encoded}\n"))
    }

    #[test]
    fn child_protocol_requires_one_running_one_result_and_one_typed_report() {
        let (expected, stdout, stderr) = protocol_fixture();
        let report = parse_child_report(true, &stdout, &stderr).unwrap();
        assert_eq!(report.request, expected);
        assert_eq!(report.observation, json!({"fixture":true}));
        let summary = stdout
            .lines()
            .find(|line| line.starts_with("test result:"))
            .unwrap();
        for changed in [
            stdout.replace("running 1 test\n", ""),
            stdout.replace("running 1 test", "running 0 tests"),
            format!("running 1 test\n{stdout}"),
            format!("{stdout}running 0 tests\n"),
            stdout.replace(summary, ""),
            stdout.replace("1 passed; 0 failed", "0 passed; 0 failed"),
            stdout.replace("1 passed; 0 failed", "0 passed; 1 failed"),
            stdout.replace("test result: ok.", "test result: FAILED."),
            format!("{stdout}{summary}\n"),
            format!(
                "{stdout}test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s\n"
            ),
        ] {
            assert!(
                parse_child_report(true, &changed, &stderr).is_err(),
                "{changed}"
            );
        }
        assert_eq!(
            parse_child_report(false, &stdout, &stderr).unwrap_err(),
            "matched metadata exact child did not execute once"
        );
        for changed in [
            String::new(),
            format!("{stderr}{stderr}"),
            format!("{stderr}embedded {MARKER}not a second report\n"),
            format!("{MARKER}not json\n"),
            format!("{stderr}running 1 test\n"),
            format!("{stderr}{summary}\n"),
        ] {
            assert!(
                parse_child_report(true, &stdout, &changed).is_err(),
                "{changed}"
            );
        }
        assert_eq!(
            parse_child_report(true, &format!("{stdout}{stderr}"), "").unwrap_err(),
            "missing or duplicate typed metadata child report"
        );
        assert_eq!(
            parse_child_report(true, &format!("{stdout}{stderr}"), &stderr).unwrap_err(),
            "missing or duplicate typed metadata child report"
        );
    }

    #[test]
    fn invalid_child_sentinel_has_a_fixed_diagnostic_without_value_disclosure() {
        use std::ffi::OsString;

        let private = "fixture sentinel contents must not be echoed";
        for value in [
            Ok(private.to_owned()),
            Err(env::VarError::NotUnicode(OsString::from(private))),
        ] {
            assert_eq!(
                child_request(value).unwrap_err(),
                "invalid metadata child sentinel"
            );
        }
        let (mut request, _stdout, _stderr) = protocol_fixture();
        request.input_fingerprint = private.into();
        assert_eq!(
            child_request(Ok(serde_json::to_string(&request).unwrap())).unwrap_err(),
            "invalid metadata child sentinel"
        );
        assert!(
            child_request(Err(env::VarError::NotPresent))
                .unwrap()
                .is_none()
        );
        let (valid, _valid_stdout, _valid_stderr) = protocol_fixture();
        let mut legacy = serde_json::to_value(&valid).unwrap();
        assert_eq!(legacy["slot"], "reference_proxy");
        legacy["slot"] = json!("reference");
        assert_eq!(
            child_request(Ok(serde_json::to_string(&legacy).unwrap())).unwrap_err(),
            "invalid metadata child sentinel"
        );
        assert_eq!(
            child_request(Ok(serde_json::to_string(&valid).unwrap())).unwrap(),
            Some(valid)
        );
    }

    fn reconstruction() -> Reconstruction {
        Reconstruction {
            bindings: "fixture binding bytes".into(),
            constructors: vec!["fixture constructor bytes".into()],
            declaration_count: 1,
            input_fingerprint: "1".repeat(64),
            measurement_fingerprint: "2".repeat(64),
        }
    }

    #[test]
    fn unequal_source_bytes_and_measurement_refuse_before_a_process_can_start() {
        let reference = reconstruction();
        let mut changed = reconstruction();
        changed.bindings.push('x');
        assert_eq!(
            observe_matched(&reference, &changed, Duration::ZERO).unwrap_err(),
            "matched metadata constructor bytes differ"
        );
        let mut other_constructors = reconstruction();
        assert_eq!(other_constructors.constructors, reference.constructors);
        other_constructors.constructors.first_mut().unwrap().push('x');
        assert_eq!(
            observe_matched(&reference, &other_constructors, Duration::ZERO).unwrap_err(),
            "matched metadata constructor bytes differ"
        );
        let mut other_declarations = reconstruction();
        assert_eq!(other_declarations.declaration_count, 1_u64);
        other_declarations.declaration_count = 2_u64;
        assert_eq!(
            observe_matched(&reference, &other_declarations, Duration::ZERO).unwrap_err(),
            "matched metadata constructor bytes differ"
        );
        let mut other_measurement = reconstruction();
        other_measurement.measurement_fingerprint = "3".repeat(64);
        assert_eq!(
            observe_matched(&reference, &other_measurement, Duration::ZERO).unwrap_err(),
            "matched metadata measurement tuple differs"
        );
    }

    #[test]
    fn equal_fixture_bytes_cannot_substitute_for_compiled_production_sources() {
        let reference = reconstruction();
        let candidate = reconstruction();
        assert_eq!(
            observe_matched(&reference, &candidate, Duration::ZERO).unwrap_err(),
            "matched metadata bytes do not bind to compiled production constructors"
        );
    }
}

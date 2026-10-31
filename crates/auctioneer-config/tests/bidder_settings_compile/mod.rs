//! Compiled consumers for fixture-owned settings, independent of shipped bidders.

use super::{CHILD_TIMEOUT, Harness, bounded_child_output, without_compiler_wrappers};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Only this child build may supply the consumer's direct dependencies.
pub(super) fn record_dependency(harness: &Harness, event: &serde_json::Value) {
    if event.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
        return;
    }
    let Some(name @ ("serde" | "serde_json" | "auctioneer_config")) = event
        .get("target")
        .and_then(|target| target.get("name"))
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let Some(filenames) = event.get("filenames").and_then(serde_json::Value::as_array) else {
        return;
    };
    for filename in filenames.iter().filter_map(serde_json::Value::as_str) {
        let path = PathBuf::from(filename);
        if path.extension().is_none_or(|extension| extension != "rlib") {
            continue;
        }
        harness
            .settings_dependencies
            .borrow_mut()
            .entry(name.to_owned())
            .or_default()
            .insert(path);
    }
}

fn write_schema(harness: &Harness, key: &str, fields: &str) {
    let directory = harness.fixture(key);
    fs::create_dir_all(&directory).expect("create settings owner");
    fs::write(
        directory.join("bidder.toml"),
        format!(
            "key = '{key}'\nfactory = false\nrequired_feature = 'insecure-bidders'\n\
             [settings.fields]\n{fields}\n[provenance]\norigin = 'local'\n"
        ),
    )
    .expect("write settings declaration");
}

fn save_settings(harness: &Harness, name: &str) -> PathBuf {
    let generated = harness.generated.borrow();
    let source = generated
        .as_ref()
        .expect("current child build must report generated output")
        .with_file_name("bidder_settings.rs");
    let directory = harness.target_dir.join("settings_consumers").join(name);
    fs::create_dir_all(&directory).expect("create settings snapshot directory");
    for filename in ["bidder_settings.rs", "bidder_endpoint.rs"] {
        fs::copy(source.with_file_name(filename), directory.join(filename))
            .expect("save current generated bindings");
    }
    directory
}

fn consumer_compiler(
    directory: &Path,
    source: &Path,
    output: &Path,
    libraries: &[(&str, &Path)],
    inputs: &[(&str, &Path)],
) -> Command {
    let mut command = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(directory)
        .args([
            "--edition=2024",
            "-Dwarnings",
            "--crate-name",
            "settings_probe",
        ])
        .arg(source)
        .arg("-o")
        .arg(output);
    for (variable, path) in inputs {
        command.env(variable, path);
    }
    for (name, library) in libraries {
        let mut external = OsString::from(name);
        external.push("=");
        external.push(library);
        command.arg("--extern").arg(external);
        let mut search = OsString::from("dependency=");
        search.push(library.parent().expect("dependency has parent directory"));
        command.arg("-L").arg(search);
    }
    without_compiler_wrappers(&mut command);
    command
}

fn compatibility_probe(directory: &Path) -> PathBuf {
    let compatibility_source = directory.join("compatibility.rs");
    fs::write(
        &compatibility_source,
        r#"
#[derive(serde::Serialize, serde::Deserialize)]
struct Probe { value: bool }
fn config_traits<T: serde::Serialize + serde::de::DeserializeOwned>() {}
fn main() {
    config_traits::<auctioneer_config::BidderSettings>();
    let probe: Probe = serde_json::from_str("{\"value\":true}").unwrap();
    assert!(probe.value);
    assert_eq!(serde_json::to_string(&probe).unwrap(), "{\"value\":true}");
}
"#,
    )
    .expect("write dependency compatibility probe");
    compatibility_source
}

fn execute_consumer(harness: &Harness, source: &str, inputs: &[(&str, &Path)]) {
    let directory = harness.target_dir.join("settings_consumers/consumer");
    fs::create_dir_all(&directory).expect("create consumer directory");
    let source_path = directory.join("main.rs");
    fs::write(&source_path, source).expect("write executable settings consumer");
    let compatibility_source = compatibility_probe(&directory);
    let dependencies = harness.settings_dependencies.borrow();
    let serde_libraries = dependencies.get("serde").expect("child must report serde");
    let json_libraries = dependencies
        .get("serde_json")
        .expect("child must report serde_json");
    let config_libraries = dependencies
        .get("auctioneer_config")
        .expect("child must report auctioneer_config");
    assert_eq!(config_libraries.len(), 1, "one child configuration library");
    let config_library = config_libraries
        .iter()
        .next()
        .expect("configuration library");
    let pairs = serde_libraries
        .len()
        .checked_mul(json_libraries.len())
        .expect("dependency pair count");
    assert!(
        (1..=16).contains(&pairs),
        "unexpected dependency variant count: {pairs}"
    );
    let mut compatible = false;
    let mut rejected = Vec::new();
    // Host/build features can produce distinct rlibs. Establish compatibility
    // independently so a generated-consumer error can never be skipped as a variant.
    for (index, (serde_path, json_path)) in serde_libraries
        .iter()
        .flat_map(|serde_path| {
            json_libraries
                .iter()
                .map(move |json_path| (serde_path, json_path))
        })
        .enumerate()
    {
        let attempt = directory.join(format!("pair_{index}"));
        fs::create_dir_all(&attempt).expect("create dependency pair directory");
        let libraries = [
            ("serde", serde_path.as_path()),
            ("serde_json", json_path.as_path()),
            ("auctioneer_config", config_library.as_path()),
        ];
        let mut probe = consumer_compiler(
            &attempt,
            &compatibility_source,
            &attempt.join("compatibility.rmeta"),
            &libraries,
            &[],
        );
        probe.arg("--emit=metadata");
        let compatibility = bounded_child_output(&mut probe, &attempt, CHILD_TIMEOUT)
            .expect("spawn bounded dependency compatibility compiler");
        if !compatibility.status.success() {
            rejected.push(format!(
                "{} + {}:\n{}\n",
                serde_path.display(),
                json_path.display(),
                String::from_utf8_lossy(&compatibility.stderr)
            ));
            continue;
        }
        compatible = true;
        let binary = attempt.join(format!("probe{}", env::consts::EXE_SUFFIX));
        let mut command = consumer_compiler(&attempt, &source_path, &binary, &libraries, inputs);
        let compilation = bounded_child_output(&mut command, &attempt, CHILD_TIMEOUT)
            .expect("spawn bounded settings compiler");
        assert!(
            compilation.status.success(),
            "settings consumer must compile with {} + {}: {}",
            serde_path.display(),
            json_path.display(),
            String::from_utf8_lossy(&compilation.stderr)
        );
        let execution = bounded_child_output(
            Command::new(&binary).current_dir(&attempt),
            &attempt,
            CHILD_TIMEOUT,
        )
        .expect("execute bounded settings consumer");
        assert!(
            execution.status.success(),
            "settings consumer must execute: {}",
            String::from_utf8_lossy(&execution.stderr)
        );
        assert_eq!(
            execution.stdout, b"settings consumer passed\n",
            "the compiled settings consumer must reach its success marker"
        );
    }
    assert!(
        compatible,
        "no compatible child dependency pair:\n{}",
        rejected.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::super::{cargo_build_with_bidder_dir, harness_dirs, write_endpoint_contract};
    use super::*;

    #[test]
    fn generated_synthetic_settings_preserve_wire_resolution_and_redaction_semantics() {
        let harness = harness_dirs("settings_semantics");
        write_schema(
            &harness,
            "fake_integer_probe",
            "primary_number = 'optional_i64'\nsecondary_number = 'optional_i64'",
        );
        write_schema(&harness, "fake_string_probe", "token = 'optional_string'");
        write_endpoint_contract(
            &harness,
            "fake_string_probe",
            &[("a_route", "ZuluRoute"), ("z_token", "AlphaToken")],
        )
        .unwrap();
        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(status.success(), "synthetic settings must build: {stderr}");
        let generated = save_settings(&harness, "semantics");
        execute_consumer(
            &harness,
            include_str!("bidder_settings_semantics.rs"),
            &[("SETTINGS_BASE", &generated)],
        );
    }

    const SECRET_REFERENCE_CONSUMER: &str = r#"
pub use auctioneer_config::bidder_settings;
pub mod generated {
    include!(concat!(env!("SETTINGS_BASE"), "/bidder_settings.rs"));
}
use generated::{BidderSettings, fake_credential_probe};
use serde_json::{Value, json};

const ERROR: &str = "invalid credential reference";
const CANARY: &str = "private-reference-canary";

fn wire(values: Value) -> Value {
    json!({"implementation": "fake_credential_probe", "values": values})
}

fn main() {
    assert_eq!(bidder_settings::MAX_CREDENTIAL_REFERENCE_BYTES, 128);
    let mut invalid = vec![
        json!(format!("{CANARY}{}", "x".repeat(129))),
        json!("x".repeat(129)),
        json!(""),
        json!(format!("{CANARY}{}", "\u{00e9}".repeat(64))),
        json!(format!("{}x", "\u{00e9}".repeat(64))),
        json!("\u{00e9}".repeat(65)),
        json!(7), json!(1.5_f64), json!(true), json!([CANARY]), json!({"private-reference-canary": CANARY}),
    ];
    for control in (0_u8..=31).chain(std::iter::once(127)) {
        invalid.push(json!(format!("{CANARY}{}suffix", char::from(control))));
    }
    for field in ["credential_reference", "secondary_reference"] {
        for value in &invalid {
            let values = json!({(field): value});
            let error = serde_json::from_value::<BidderSettings>(wire(values.clone()))
                .expect_err("invalid credential references must be rejected at admission");
            assert!(error.is_data());
            assert_eq!(error.to_string(), ERROR);
            let error = serde_json::from_value::<fake_credential_probe::Settings>(values.clone())
                .expect_err("standalone settings must enforce the same admission");
            assert_eq!(error.to_string(), ERROR);
            let text = serde_json::to_string(&wire(values)).unwrap();
            let error = serde_json::from_str::<BidderSettings>(&text).unwrap_err().to_string();
            assert!(error.starts_with(ERROR));
            assert!(!error.contains(CANARY));
        }
    }
    for value in [
        "a".to_owned(), "x".repeat(128), "\u{00e9}".repeat(64),
        " ".to_owned(), " fake:/reference?version=2 ".to_owned(), "\u{0085}".to_owned(),
    ] {
        let original = wire(json!({"credential_reference": value}));
        let settings: BidderSettings = serde_json::from_value(original.clone()).unwrap();
        let typed = settings.as_fake_credential_probe().unwrap();
        let view = fake_credential_probe::resolve(Some(typed), None);
        let borrowed: &str = view.credential_reference().unwrap();
        assert_eq!(borrowed, value);
        assert_eq!(borrowed.as_ptr(), typed.credential_reference.as_deref().unwrap().as_ptr());
        assert_eq!(serde_json::to_value(&settings).unwrap(), original);
    }
    for values in [json!({}), json!({"credential_reference": null, "secondary_reference": null})] {
        let settings: BidderSettings = serde_json::from_value(wire(values)).unwrap();
        let typed = settings.as_fake_credential_probe().unwrap();
        assert_eq!(typed.credential_reference, None);
        assert_eq!(typed.secondary_reference, None);
        assert_eq!(serde_json::to_value(&settings).unwrap(), wire(json!({})));
    }
    let base: fake_credential_probe::Settings = serde_json::from_value(json!({
        "credential_reference": "fake-base", "secondary_reference": "fake-inherited",
        "note": "base-note", "source": 7,
    })).unwrap();
    let requested: fake_credential_probe::Settings = serde_json::from_value(json!({
        "credential_reference": "fake-requested", "secondary_reference": null,
        "note": "", "source": 0,
    })).unwrap();
    let view = fake_credential_probe::resolve(Some(&requested), Some(&base));
    let requested_reference: Option<&str> = view.credential_reference();
    assert_eq!(requested_reference, Some("fake-requested"));
    assert_eq!(requested_reference.unwrap().as_ptr(), requested.credential_reference.as_deref().unwrap().as_ptr());
    assert_eq!(view.secondary_reference(), Some("fake-inherited"));
    assert_eq!(view.secondary_reference().unwrap().as_ptr(), base.secondary_reference.as_deref().unwrap().as_ptr());
    assert_eq!(view.note(), Some(""));
    assert_eq!(view.source(), Some(0));
    let absent = fake_credential_probe::Settings::default();
    assert_eq!(fake_credential_probe::resolve(Some(&absent), Some(&base)).credential_reference(), Some("fake-base"));
    assert_eq!(fake_credential_probe::resolve(None, Some(&base)).credential_reference(), Some("fake-base"));
    assert_eq!(fake_credential_probe::resolve(None, None).credential_reference(), None);
    let settings: BidderSettings = serde_json::from_value(wire(json!({
        "credential_reference": CANARY,
    }))).unwrap();
    let typed = settings.as_fake_credential_probe().unwrap();
    for debug in [format!("{settings:?}"), format!("{typed:?}"), format!("{:?}", fake_credential_probe::resolve(Some(typed), None))] {
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(CANARY));
    }
    let plain: fake_credential_probe::Settings = serde_json::from_value(json!({"note": "\n"})).unwrap();
    assert_eq!(plain.note.as_deref(), Some("\n"));
    let reference_only: generated::fake_reference_only_probe::Settings = serde_json::from_value(json!({
        "credential_reference": "fake-reference-only",
    })).unwrap();
    let borrowed: Option<&str> = generated::fake_reference_only_probe::resolve(Some(&reference_only), None).credential_reference();
    assert_eq!(borrowed, Some("fake-reference-only"));
    assert_eq!(borrowed.unwrap().as_ptr(), reference_only.credential_reference.as_deref().unwrap().as_ptr());
    println!("settings consumer passed");
}
"#;

    #[test]
    fn generated_secret_references_reject_invalid_values_at_admission() {
        let harness = harness_dirs("settings_secret_references");
        write_schema(
            &harness,
            "fake_credential_probe",
            "credential_reference = 'optional_secret_reference'\nsecondary_reference = 'optional_secret_reference'\n\
             note = 'optional_string'\nsource = 'optional_i64'",
        );
        write_schema(
            &harness,
            "fake_reference_only_probe",
            "credential_reference = 'optional_secret_reference'",
        );
        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(status.success(), "synthetic settings must build: {stderr}");
        let generated = save_settings(&harness, "secret_references");
        execute_consumer(
            &harness,
            SECRET_REFERENCE_CONSUMER,
            &[("SETTINGS_BASE", &generated)],
        );
    }

    #[test]
    fn generated_settings_compile_with_dependency_and_helper_names() {
        let harness = harness_dirs("settings_namespaces");
        for key in ["std", "serde", "deserialize_object"] {
            write_schema(
                &harness,
                key,
                "resolve = 'optional_i64'\nrequested = 'optional_i64'\nbase = 'optional_i64'\n\
                 clone = 'optional_string'\nfmt = 'optional_string'\ndeserialize = 'optional_string'",
            );
        }
        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(status.success(), "namespace fixture must build: {stderr}");
        let generated = save_settings(&harness, "namespaces");
        execute_consumer(
            &harness,
            r#"
pub mod generated {
    include!(concat!(env!("SETTINGS_BASE"), "/bidder_settings.rs"));
}
macro_rules! check {
    ($key:literal, $module:ident, $accessor:ident) => {{
        let wire = serde_json::json!({"implementation": $key, "values": {
            "resolve": 0, "requested": 11, "base": 12,
            "clone": "", "fmt": "private-fmt", "deserialize": "private-deserialize"
        }});
        let settings: generated::BidderSettings = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(settings.implementation_key(), $key);
        let requested = settings.$accessor().unwrap();
        let base = generated::$module::Settings {
            resolve: Some(41), requested: Some(42), base: Some(43),
            clone: Some("base".into()), fmt: None, deserialize: None,
        };
        let view = generated::$module::resolve(Some(requested), Some(&base));
        assert_eq!(view.resolve(), Some(0));
        assert_eq!(view.requested(), Some(11));
        assert_eq!(view.base(), Some(12));
        assert_eq!(view.clone(), Some(""));
        assert_eq!(view.fmt(), Some("private-fmt"));
        assert_eq!(view.deserialize(), Some("private-deserialize"));
        assert_eq!(serde_json::to_value(&settings).unwrap(), wire);
        for debug in [format!("{settings:?}"), format!("{requested:?}"), format!("{view:?}")] {
            assert!(!debug.contains("private-"));
            assert!(debug.contains("<redacted>"));
        }
    }};
}
fn main() {
    check!("std", std, as_std);
    check!("serde", serde, as_serde);
    check!("deserialize_object", deserialize_object, as_deserialize_object);
    println!("settings consumer passed");
}
"#,
            &[("SETTINGS_BASE", &generated)],
        );
    }

    #[test]
    fn generated_settings_compile_without_any_declared_schema() {
        let harness = harness_dirs("settings_empty");
        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(status.success(), "empty catalog must build: {stderr}");
        let generated = save_settings(&harness, "empty");
        execute_consumer(
            &harness,
            r#"
pub mod generated {
    include!(concat!(env!("SETTINGS_BASE"), "/bidder_settings.rs"));
}
fn main() {
    fn traits<T: Clone + std::fmt::Debug + serde::Serialize + serde::de::DeserializeOwned>() {}
    traits::<generated::BidderSettings>();
    for key in ["fake_probe", "std", "serde", "V0"] {
        assert!(serde_json::from_value::<generated::BidderSettings>(serde_json::json!({
            "implementation": key, "values": {}
        })).is_err());
    }
    for wire in [serde_json::json!({}), serde_json::json!([]), serde_json::json!(null)] {
        assert!(serde_json::from_value::<generated::BidderSettings>(wire).is_err());
    }
    println!("settings consumer passed");
}
"#,
            &[("SETTINGS_BASE", &generated)],
        );
    }

    #[test]
    fn generated_settings_wire_and_accessors_survive_preceding_schema_insertion() {
        let harness = harness_dirs("settings_insertion");
        for key in ["fake_middle_probe", "fake_zeta_probe"] {
            write_schema(
                &harness,
                key,
                "number = 'optional_i64'\ntext = 'optional_string'",
            );
        }
        let (baseline_status, baseline_stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(
            baseline_status.success(),
            "baseline schemas must build: {baseline_stderr}"
        );
        let baseline = save_settings(&harness, "baseline");
        write_schema(&harness, "fake_aaa_probe", "token = 'optional_string'");
        let (expanded_status, expanded_stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(
            expanded_status.success(),
            "inserted schema must build: {expanded_stderr}"
        );
        let expanded = save_settings(&harness, "expanded");
        assert_ne!(
            fs::read(baseline.join("bidder_settings.rs")).unwrap(),
            fs::read(expanded.join("bidder_settings.rs")).unwrap(),
            "insertion must change generated output"
        );
        execute_consumer(
            &harness,
            r#"
pub mod baseline {
    include!(concat!(env!("SETTINGS_BASE"), "/bidder_settings.rs"));
}
pub mod expanded {
    include!(concat!(env!("SETTINGS_EXPANDED"), "/bidder_settings.rs"));
}
macro_rules! check {
    ($key:literal, $module:ident, $accessor:ident) => {{
        for (number, text) in [(0, ""), (i64::MIN, "min"), (i64::MAX, "max")] {
            let wire = serde_json::json!({"implementation": $key, "values": {
                "number": number, "text": text
            }});
            let before: baseline::BidderSettings = serde_json::from_value(wire.clone()).unwrap();
            let after: expanded::BidderSettings = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(before.implementation_key(), $key);
            assert_eq!(after.implementation_key(), $key);
            assert_eq!(serde_json::to_value(&before).unwrap(), wire);
            assert_eq!(serde_json::to_value(&after).unwrap(), wire);
            let a = baseline::$module::resolve(before.$accessor(), None);
            let b = expanded::$module::resolve(after.$accessor(), None);
            assert_eq!(a.number(), Some(number));
            assert_eq!(b.number(), a.number());
            assert_eq!(a.text(), Some(text));
            assert_eq!(b.text(), a.text());
        }
    }};
}
fn main() {
    check!("fake_middle_probe", fake_middle_probe, as_fake_middle_probe);
    check!("fake_zeta_probe", fake_zeta_probe, as_fake_zeta_probe);
    let wire = serde_json::json!({"implementation": "fake_aaa_probe", "values": {"token": "new"}});
    assert!(serde_json::from_value::<baseline::BidderSettings>(wire.clone()).is_err());
    let inserted: expanded::BidderSettings = serde_json::from_value(wire).unwrap();
    assert_eq!(inserted.as_fake_aaa_probe().unwrap().token.as_deref(), Some("new"));
    println!("settings consumer passed");
}
"#,
            &[
                ("SETTINGS_BASE", &baseline),
                ("SETTINGS_EXPANDED", &expanded),
            ],
        );
        fs::remove_dir_all(harness.fixture("fake_aaa_probe")).unwrap();
        let (restored_status, restored_stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(
            restored_status.success(),
            "schema removal must build: {restored_stderr}"
        );
        let restored = save_settings(&harness, "restored");
        assert_eq!(
            fs::read(baseline.join("bidder_settings.rs")).unwrap(),
            fs::read(restored.join("bidder_settings.rs")).unwrap()
        );
    }
}

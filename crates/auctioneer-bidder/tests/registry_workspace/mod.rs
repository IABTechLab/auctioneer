use super::build_workspace::{BuildWorkspace, logs, require_success};
use serde_json::{Value as Json, json};
use std::fs;
use std::io::{Write as _, stderr};
use std::path::PathBuf;
use toml::Value;

const PROBE: &str = "registry-discovery-probe";
const BIDDER_SOURCE: &str = include_str!("registry_bidder.rs");

pub struct Harness {
    workspace: BuildWorkspace,
}

impl Harness {
    pub fn report(&self) {
        self.workspace.report();
    }

    pub fn new() -> Self {
        Self {
            workspace: BuildWorkspace::new_synthetic(PROBE, include_str!("registry_probe.rs")),
        }
    }

    pub fn implementation(&self, key: &str) -> PathBuf {
        self.workspace.bidder().join("src/bidders").join(key)
    }

    pub fn add_source(&self, key: &str, marker: &str, required_feature: Option<&str>) {
        let directory = self.implementation(key);
        fs::create_dir_all(directory.join("protocol")).unwrap();
        let mut declaration = toml::Table::new();
        declaration.insert("key".into(), key.into());
        declaration.insert("factory".into(), true.into());
        declaration.insert("definitions".into(), Value::Table(toml::Table::new()));
        declaration.insert(
            "provenance".into(),
            Value::Table(toml::Table::from_iter([("origin".into(), "local".into())])),
        );
        if let Some(feature) = required_feature {
            declaration.insert("required_feature".into(), feature.into());
        }
        fs::write(
            directory.join("bidder.toml"),
            toml::to_string(&declaration).unwrap(),
        )
        .unwrap();
        let mut source = BIDDER_SOURCE.to_owned();
        if required_feature == Some("insecure-bidders") {
            source.push_str("\n#[cfg(not(feature = \"insecure-bidders\"))]\ncompile_error!(\"inactive discovery module compiled\");\n");
        }
        fs::write(directory.join("mod.rs"), source).unwrap();
        fs::write(directory.join("protocol/mod.rs"), "pub mod marker;\n").unwrap();
        self.edit_marker(key, marker);
    }

    pub fn edit_marker(&self, key: &str, marker: &str) {
        fs::write(
            self.implementation(key).join("protocol/marker.rs"),
            format!("pub const MARKER: &str = {marker:?};\n"),
        )
        .unwrap();
    }

    pub fn manifest_path(&self, key: &str) -> PathBuf {
        self.implementation(key).join("bidder.toml")
    }

    pub fn remove_metadata(&self, name: &str, implementation: &str) {
        let path = self.manifest_path(implementation);
        let mut manifest: Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(
            manifest["definitions"]
                .as_table_mut()
                .unwrap()
                .remove(name)
                .is_some()
        );
        fs::write(path, toml::to_string(&manifest).unwrap()).unwrap();
    }

    pub fn add_metadata(&self, name: &str, implementation: &str, endpoint: &str, feature: bool) {
        let mut metadata: Value = toml::from_str(
            r#"
endpoint = "https://fixture.discovery.invalid/openrtb2"
[maintainer]
email = "discovery@fixture.invalid"
[capabilities.site]
mediaTypes = ["banner"]
"#,
        )
        .unwrap();
        metadata["endpoint"] = endpoint.into();
        let path = self.manifest_path(implementation);
        let mut manifest: Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            manifest.get("required_feature").and_then(Value::as_str),
            feature.then_some("insecure-bidders")
        );
        manifest["definitions"]
            .as_table_mut()
            .unwrap()
            .insert(name.into(), metadata);
        fs::write(path, toml::to_string(&manifest).unwrap()).unwrap();
    }

    pub fn run(&self, expected: &Expected<'_>, label: &str) {
        let mut arguments = vec!["--quiet", "--bin", PROBE];
        if expected.feature {
            arguments.extend(["--features", "insecure-bidders"]);
        }
        let output = self.workspace.cargo("run", &arguments, true);
        require_success(&output, label);
        let actual: Json = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!("invalid probe JSON ({label}): {error}\n{}", logs(&output))
        });
        assert_eq!(actual, expected.artifact(), "artifact stage: {label}");
        assert!(self.workspace.target().join("debug").join(PROBE).is_file());
        writeln!(
            stderr(),
            "registry artifact passed: {label}; {} synthetic executions",
            expected.execution_count
        )
        .unwrap();
    }

    pub fn refuse_missing(&self, key: &str, filename: &str) {
        let missing = self.implementation(key).join(filename);
        let saved = fs::read(&missing).unwrap();
        fs::remove_file(&missing).unwrap();
        let output = self
            .workspace
            .cargo("build", &["--quiet", "--bin", PROBE], true);
        let diagnostic = format!("Error: \"{}: missing input\"", missing.display());
        assert!(
            !output.status.success(),
            "missing {filename} compiled successfully"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .any(|line| line.trim() == diagnostic),
            "missing exact refusal {diagnostic}\n{}",
            logs(&output)
        );
        writeln!(stderr(), "registry intended rejection: {diagnostic}").unwrap();
        fs::write(missing, saved).unwrap();
    }
}

// These are independent fixture expectations, never derived from generated views.
pub struct Expected<'a> {
    pub factories: &'a [&'a str],
    pub definitions: &'a [&'a str],
    pub bindings: &'a [(&'a str, &'a str)],
    pub declarations: &'a [(&'a str, Option<&'a str>)],
    pub metadata: &'a [(&'a str, &'a str, Option<&'a str>)],
    pub executions: &'a [Execution<'a>],
    pub execution_count: usize,
    pub feature: bool,
}

pub struct Execution<'a> {
    pub name: &'a str,
    pub key: &'a str,
    pub marker: &'a str,
    pub endpoint: &'a str,
    pub ordinal: usize,
}

impl Expected<'_> {
    fn artifact(&self) -> Json {
        assert!(
            self.execution_count > 0,
            "zero-execution expectations are forbidden"
        );
        assert_eq!(self.executions.len(), self.execution_count);
        assert_eq!(self.definitions.len(), self.execution_count);
        let mut factories = self.factories.to_vec();
        factories.sort_unstable();
        let mut names = self.definitions.to_vec();
        names.sort_unstable();
        let mut bindings = self.bindings.to_vec();
        bindings.sort_unstable();
        let mut declarations = self.declarations.to_vec();
        declarations.sort_unstable();
        let mut metadata = self.metadata.to_vec();
        metadata.sort_unstable();
        let lookups = [
            "fake_discovery_alpha",
            "fake_discovery_beta",
            "fake_discovery_fake_gamma",
            "fake_discovery_mirror",
            "fake_discovery_optional",
        ]
        .map(|name| (name, self.definitions.contains(&name)));
        json!({
            "factories": factories,
            "plan_factories": factories,
            "definitions": names,
            "metadata_names": names,
            "bindings": bindings,
            "declarations": declarations,
            "metadata_declarations": metadata,
            "lookup": lookups,
            "metadata_lookup": lookups,
            "overrides": false,
            "execution_count": self.execution_count,
            "executed": self.executions.iter().map(Execution::artifact).collect::<Vec<_>>(),
        })
    }
}

impl Execution<'_> {
    fn artifact(&self) -> Json {
        json!({
            "name": self.name, "implementation": self.key, "method": "POST",
            "endpoint": self.endpoint, "imp_ids": ["discovery-imp"],
            "content_type": "application/json",
            "module": format!("auctioneer_bidder::bidders::{}", self.key),
            "constructors": self.ordinal, "executions": self.ordinal,
            "body": {"marker": self.marker, "public": self.name, "core": self.name, "request_id": "discovery-request"},
        })
    }
}

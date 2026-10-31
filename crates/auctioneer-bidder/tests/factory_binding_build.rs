//! Exercise the real factory/metadata build gate with synthetic declarations in an isolated Cargo workspace.

#![cfg(all(unix, not(target_arch = "wasm32")))]

#[cfg(test)]
mod build_workspace;

#[cfg(test)]
mod tests {
    use super::build_workspace::{BuildWorkspace, logs, require_success};
    use serde_json::{Value as Json, json};
    use std::fs;
    use std::io::{Write as _, stderr};
    use std::process::Output;
    use toml::Value;

    const PROBE: &str = "factory-binding-probe";
    const FACTORY_SOURCE: &str = "pub(crate) use crate::fake_synthetic::create_bidder;\n";
    const PROBE_SOURCE: &str = r#"
mod allocation_support;
use allocation_support as endpoint_allocation;

use auctioneer_bidder::{BidderName, BidderRequestInfo, CallBudget, EndpointBuilder,
    EndpointRejectReason, EndpointValidator, EntryPoint, META_BIDDER_REGION_BYTES,
    META_BYTES_CAPACITY, OutboundRequestMetaBuilder, OutboundRequestSink, SinkAbort, SinkLimits};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};

struct Accept;
impl EndpointValidator for Accept {
    fn validate(&self, _: &url::Url, _: &BidderRequestInfo<'_>) -> Result<(), EndpointRejectReason> {
        Ok(())
    }
}

fn verify_endpoint_binding() {
    use endpoint_allocation::alloc_track::{reset_on_this_thread, peak_on_this_thread};
    let prefix = "https://binding.example.test/";
    let info = BidderRequestInfo::new(
        BidderName::new("fake_binding_alpha"), BidderName::new("fake_binding_alpha"), "account", None,
        "https://binding.example.test/{{.SyntheticToken}}", &["SyntheticToken"],
        None, None, EntryPoint::Auction, &[],
    );
    let fitting = META_BIDDER_REGION_BYTES - 4 - prefix.len();
    for width in [1, fitting - 1, fitting, fitting + 1, 300 * 1024] {
        let input = "a".repeat(width);
        reset_on_this_thread();
        let copied = std::hint::black_box(input.clone());
        let clone_peak = peak_on_this_thread();
        assert!(clone_peak >= i64::try_from(width).unwrap(), "the allocator must detect a real clone");
        drop(copied);
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let pointer = pool.as_ptr();
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
        meta.set_method("POST").unwrap();
        reset_on_this_thread();
        let pairs = auctioneer_config::bidder_endpoint::fake_binding_alpha::Values { token: &input }.as_pairs();
        let result = EndpointBuilder::from_request(&info, &pairs).expand_into(&mut meta);
        let peak = peak_on_this_thread();
        assert_eq!(peak, 0, "generated endpoint binding width {width}");
        if width <= fitting {
            result.unwrap();
            assert_eq!(meta.written(), 4 + prefix.len() + width);
        } else {
            assert_eq!(result, Err(SinkAbort::MetaOverCap {
                limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
            }));
            assert_eq!(meta.written(), 4);
        }
        drop(meta);
        assert_eq!(pool.as_ptr(), pointer);
        assert_eq!(pool.capacity(), META_BYTES_CAPACITY);
    }
}

fn main() {
    verify_endpoint_binding();
    let factories = auctioneer_bidder::compiled_bidders();
    let request = BidRequest { id: "binding-request".into(), imp: vec![Imp {
        id: "binding-imp".into(), banner: Some(Default::default()), ..Imp::default()
    }], ..BidRequest::default() };
    let mut executed = Vec::new();
    for factory in factories {
        let bidder = (factory.factory)();
        let info = BidderRequestInfo::new(
            BidderName::new(factory.key), BidderName::new(factory.key), "account", None,
            "https://binding.example.test/openrtb", &[], None, None, EntryPoint::Auction, &[],
        );
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 1), &budget, &Accept);
        bidder.make_requests(&request, &info, &mut sink).expect("synthetic construction");
        let outcome = sink.into_outcome();
        assert!(outcome.errors.is_empty());
        assert_eq!(outcome.accepted.len(), 1);
        let sent = &outcome.accepted[0];
        assert_eq!(sent.method(), "POST");
        assert_eq!(sent.url(), "https://binding.example.test/openrtb");
        assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["binding-imp"]);
        let body: serde_json::Value = serde_json::from_slice(sent.body()).unwrap();
        assert_eq!(body["id"], "binding-request");
        assert_eq!(body["imp"][0]["id"], "binding-imp");
        executed.push(factory.key);
    }
    let values = auctioneer_config::bidder_endpoint::fake_binding_alpha::Values { token: "a b/c" };
    assert_eq!(values.as_pairs(), [("SyntheticToken", "a b/c")]);
    println!("{}", serde_json::json!({
        "compiled": factories.iter().map(|row| row.key).collect::<Vec<_>>(),
        "executed": executed,
        "declarations": auctioneer_bidder::bidder_declarations().iter()
            .map(|row| (row.key, row.required_feature)).collect::<Vec<_>>(),
        "metadata": auctioneer_config::bidder_metadata_declarations().iter()
            .map(|row| (row.bidder, row.implementation, row.required_feature))
            .collect::<Vec<_>>(),
        "bindings": auctioneer_config::bidder_implementation_bindings().iter()
            .map(|row| (row.bidder, row.implementation)).collect::<Vec<_>>(),
        "overrides": auctioneer_config::TEST_HARNESS_OVERRIDE_ENABLED,
    }));
}
"#;
    #[derive(Clone, Copy)]
    enum Mutation {
        PlainGainsFeature,
        OptionalLosesFeature,
        OptionalChangesFeature,
        MissingConfigForwarding,
        MissingPlainFactory,
        ConflictingImplementation,
    }

    impl Mutation {
        fn label(self) -> &'static str {
            match self {
                Self::PlainGainsFeature => "fake_binding_alpha gains required_feature",
                Self::OptionalLosesFeature => "fake_binding_optional loses required_feature",
                Self::OptionalChangesFeature => "fake_binding_optional changes required_feature",
                Self::MissingConfigForwarding => "insecure-bidders lacks config forwarding",
                Self::MissingPlainFactory => "fake_binding_alpha factory removed",
                Self::ConflictingImplementation => "definition names another implementation",
            }
        }

        fn diagnostic(self) -> &'static str {
            match self {
                Self::PlainGainsFeature
                | Self::OptionalLosesFeature
                | Self::OptionalChangesFeature => {
                    "definitions.required_feature must match the owning feature"
                }
                Self::MissingConfigForwarding => {
                    "missing config feature forwarding: insecure-bidders"
                }
                Self::MissingPlainFactory => "fake_binding_alpha/mod.rs: missing input",
                Self::ConflictingImplementation => {
                    "definitions.implementation must match the owning key"
                }
            }
        }
    }

    struct Harness {
        workspace: BuildWorkspace,
        manifest: String,
        fake_binding_alpha: String,
        fake_binding_alpha_module: Vec<u8>,
        fake_binding_optional: String,
    }

    impl Harness {
        fn new() -> Self {
            let workspace = BuildWorkspace::new_synthetic(PROBE, PROBE_SOURCE);
            fs::create_dir_all(workspace.bidder().join("src/bin/allocation_support")).unwrap();
            fs::write(
                workspace.bidder().join("src/bin/allocation_support/mod.rs"),
                include_str!("endpoint_expansion_bound.rs"),
            )
            .unwrap();
            for (key, feature) in [
                ("fake_binding_alpha", None),
                ("fake_binding_optional", Some("insecure-bidders")),
            ] {
                let directory = workspace.bidder().join("src/bidders").join(key);
                fs::create_dir_all(&directory).unwrap();
                let mut declaration: Value = toml::from_str(
                    r#"
factory = true
[endpoint.fields]
token = "SyntheticToken"
[provenance]
origin = "local"
"#,
                )
                .unwrap();
                let fields = declaration.as_table_mut().unwrap();
                fields.insert("key".into(), key.into());
                if let Some(required_feature) = feature {
                    fields.insert("required_feature".into(), required_feature.into());
                }
                fields.insert(
                    "definitions".into(),
                    Value::Table(toml::Table::from_iter([(
                        key.into(),
                        toml::from_str::<Value>(
                            r#"
endpoint = "https://binding.example.test/openrtb?token={{.SyntheticToken}}"
[maintainer]
email = "binding@example.test"
[capabilities.site]
mediaTypes = ["banner"]
"#,
                        )
                        .unwrap(),
                    )])),
                );
                fs::write(
                    directory.join("bidder.toml"),
                    toml::to_string(&declaration).unwrap(),
                )
                .unwrap();
                fs::write(directory.join("mod.rs"), FACTORY_SOURCE).unwrap();
            }
            let manifest = fs::read_to_string(workspace.bidder().join("Cargo.toml")).unwrap();
            let fake_binding_alpha = fs::read_to_string(
                workspace
                    .bidder()
                    .join("src/bidders/fake_binding_alpha/bidder.toml"),
            )
            .unwrap();
            let fake_binding_optional = fs::read_to_string(
                workspace
                    .bidder()
                    .join("src/bidders/fake_binding_optional/bidder.toml"),
            )
            .unwrap();
            let fake_binding_alpha_module = fs::read(
                workspace
                    .bidder()
                    .join("src/bidders/fake_binding_alpha/mod.rs"),
            )
            .unwrap();
            Self {
                workspace,
                manifest,
                fake_binding_alpha,
                fake_binding_alpha_module,
                fake_binding_optional,
            }
        }

        fn check(&self) -> Output {
            self.workspace.cargo("check", &["--lib"], true)
        }

        fn executable(&self, insecure: bool) {
            let mut arguments = vec!["--quiet", "--bin", PROBE];
            if insecure {
                arguments.extend(["--features", "insecure-bidders"]);
            }
            let output = self.workspace.cargo("run", &arguments, true);
            require_success(&output, "real factory executable control");
            let actual: Json = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
                panic!(
                    "invalid executable control output: {error}\n{}",
                    logs(&output)
                )
            });
            let compiled = if insecure {
                vec!["fake_binding_alpha", "fake_binding_optional"]
            } else {
                vec!["fake_binding_alpha"]
            };
            let bindings: Vec<_> = compiled.iter().map(|key| (*key, *key)).collect();
            assert_eq!(actual["compiled"], json!(compiled));
            assert_eq!(actual["executed"], json!(compiled));
            assert_eq!(actual["bindings"], json!(bindings));
            assert_eq!(
                actual["declarations"],
                json!([
                    ["fake_binding_alpha", null],
                    ["fake_binding_optional", "insecure-bidders"]
                ])
            );
            assert_eq!(
                actual["metadata"],
                json!([
                    ["fake_binding_alpha", "fake_binding_alpha", null],
                    [
                        "fake_binding_optional",
                        "fake_binding_optional",
                        "insecure-bidders"
                    ]
                ])
            );
            assert_eq!(actual["overrides"], false);
            assert!(self.workspace.target().join("debug").join(PROBE).is_file());
            writeln!(
                stderr(),
                "factory binding executable control passed: insecure-bidders={insecure}"
            )
            .unwrap();
        }

        fn mutate(&self, mutation: Mutation) {
            let mut manifest: Value = toml::from_str(&self.manifest).unwrap();
            match mutation {
                Mutation::PlainGainsFeature => {
                    self.set_definition_feature("fake_binding_alpha", "insecure-bidders");
                }
                Mutation::OptionalLosesFeature => {
                    self.set_definition_feature("fake_binding_optional", "insecure-bidders");
                    let path = self
                        .workspace
                        .bidder()
                        .join("src/bidders/fake_binding_optional/bidder.toml");
                    let mut declaration: Value =
                        toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
                    assert!(
                        declaration
                            .as_table_mut()
                            .unwrap()
                            .remove("required_feature")
                            .is_some()
                    );
                    fs::write(path, toml::to_string(&declaration).unwrap()).unwrap();
                }
                Mutation::OptionalChangesFeature => {
                    manifest["features"]
                        .as_table_mut()
                        .unwrap()
                        .insert("factory-binding-alternate".into(), Value::Array(Vec::new()));
                    self.set_definition_feature(
                        "fake_binding_optional",
                        "factory-binding-alternate",
                    );
                }
                Mutation::MissingConfigForwarding => {
                    manifest["features"]["insecure-bidders"] = Value::Array(Vec::new());
                }
                Mutation::MissingPlainFactory => {
                    let directory = self
                        .workspace
                        .bidder()
                        .join("src/bidders/fake_binding_alpha");
                    let declaration: Value =
                        toml::from_str(&fs::read_to_string(directory.join("bidder.toml")).unwrap())
                            .unwrap();
                    assert_eq!(declaration["factory"].as_bool(), Some(true));
                    assert!(
                        declaration["definitions"]
                            .get("fake_binding_alpha")
                            .is_some()
                    );
                    fs::remove_file(directory.join("mod.rs")).unwrap();
                }
                Mutation::ConflictingImplementation => {
                    let path = self
                        .workspace
                        .bidder()
                        .join("src/bidders/fake_binding_alpha/bidder.toml");
                    let mut declaration: Value =
                        toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
                    declaration["definitions"]["fake_binding_alpha"]
                        .as_table_mut()
                        .unwrap()
                        .insert("implementation".into(), "fake_binding_optional".into());
                    fs::write(path, toml::to_string(&declaration).unwrap()).unwrap();
                }
            }
            fs::write(
                self.workspace.bidder().join("Cargo.toml"),
                toml::to_string(&manifest).unwrap(),
            )
            .unwrap();
        }

        fn set_definition_feature(&self, key: &str, required_feature: &str) {
            let path = self
                .workspace
                .bidder()
                .join("src/bidders")
                .join(key)
                .join("bidder.toml");
            let mut declaration: Value =
                toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            declaration["definitions"][key]
                .as_table_mut()
                .unwrap()
                .insert("required_feature".into(), required_feature.into());
            fs::write(path, toml::to_string(&declaration).unwrap()).unwrap();
        }

        fn restore(&self) {
            fs::write(
                self.workspace
                    .bidder()
                    .join("src/bidders/fake_binding_alpha/mod.rs"),
                &self.fake_binding_alpha_module,
            )
            .unwrap();
            fs::write(self.workspace.bidder().join("Cargo.toml"), &self.manifest).unwrap();
            for (key, source) in [
                ("fake_binding_alpha", &self.fake_binding_alpha),
                ("fake_binding_optional", &self.fake_binding_optional),
            ] {
                fs::write(
                    self.workspace
                        .bidder()
                        .join("src/bidders")
                        .join(key)
                        .join("bidder.toml"),
                    source,
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn real_build_reconciles_metadata_before_feature_filtering() {
        let harness = Harness::new();
        for insecure in [false, true, false] {
            harness.executable(insecure);
        }
        let mismatch = harness.workspace.cargo(
            "check",
            &["--lib", "--features", "auctioneer-config/insecure-bidders"],
            true,
        );
        assert!(
            !mismatch.status.success(),
            "config-only feature activation must not build"
        );
        assert!(
            logs(&mismatch).contains("compiled bidder bindings differ from config"),
            "{}",
            logs(&mismatch)
        );
        harness.executable(false);

        let mut failures = Vec::new();
        let cases = [
            Mutation::OptionalLosesFeature,
            Mutation::PlainGainsFeature,
            Mutation::OptionalChangesFeature,
            Mutation::MissingConfigForwarding,
            Mutation::MissingPlainFactory,
            Mutation::ConflictingImplementation,
        ];
        for mutation in cases {
            harness.mutate(mutation);
            let output = harness.check();
            let label = mutation.label();
            let diagnostic = mutation.diagnostic();
            if output.status.success() {
                failures.push(format!(
                    "{label}: invalid source built successfully; missing diagnostic: {diagnostic}"
                ));
                writeln!(stderr(), "factory binding behavioral RED: {label} compiled").unwrap();
            } else if !String::from_utf8_lossy(&output.stderr).contains(diagnostic) {
                failures.push(format!(
                    "{label}: build failed without intended diagnostic: {diagnostic}\n{}",
                    logs(&output)
                ));
                writeln!(stderr(), "factory binding unexpected diagnostic: {label}").unwrap();
            } else {
                writeln!(
                    stderr(),
                    "factory binding intended rejection: {label}: {diagnostic}"
                )
                .unwrap();
            }
            harness.restore();
            require_success(&harness.check(), "restored source in the same target cache");
        }
        harness.executable(false);
        harness.workspace.report();
        assert!(
            failures.is_empty(),
            "{} of {} invalid cases missed the production gate:\n{}",
            failures.len(),
            cases.len(),
            failures.join("\n\n")
        );
    }
}

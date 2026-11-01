//! Production source/capture/resource conformance, separate from shared fake controls.

#![cfg(all(test, not(target_arch = "wasm32"), unix))]

pub(crate) const SCHEMA_SOURCE_CONFORMANCE: bool = true;

use std::path::Path;

#[cfg(test)]
mod schema_meter;

#[cfg(test)]
mod schema_matchers;

#[cfg(test)]
mod schema_formats;

mod schema_backtrack_bounds;
mod schema_build;
mod schema_bundle_heap;
mod schema_cache_model;
mod schema_compiler_layout;
use schema_cache_model as schema_cache_bounds;
mod schema_fallback_heap;
mod schema_heap_layout;
mod schema_hybrid_bounds;
mod schema_hybrid_capacity;
mod schema_hybrid_graph;
mod schema_hybrid_heap;
mod schema_hybrid_population;
mod schema_hybrid_prefix;
mod schema_literal_heap;
mod schema_meta_eager;
mod schema_meta_heap;
mod schema_metadata_heap;
mod schema_nfa_construction;
mod schema_nfa_heap;
mod schema_onepass_bounds;
mod schema_onepass_construction;
mod schema_pool_heap;
mod schema_profile;
mod schema_program_heap;
mod schema_qualification;
mod schema_reference;
mod schema_replay;
mod schema_resource_layout;
mod schema_selected_half_search;
mod schema_selected_semantics;
mod schema_selection;
mod schema_selection_source;
mod schema_syntax_construction;
mod schema_vendor;

// Keep all 37 case tests (29 build, 7 cache, 1 inventory). Sharing support removes
// 124 duplicate instances; one new profile-population test changes 393 to 270.
mod schema_build_cases;
mod schema_cache_bounds_cases;
mod schema_source_inventory_cases;

fn bundle_patterns(
    root: &Path,
    schema: &schema_qualification::SchemaInventory,
) -> Result<Vec<(String, String)>, String> {
    use quote::quote;
    use schema_qualification::Draft;

    let schema_path = root
        .join(&schema.witness.path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let path_literal = syn::LitStr::new(
        schema_path.to_str().ok_or("non UTF-8 schema path")?,
        proc_macro2::Span::call_site(),
    );
    let draft = match schema.audit.draft {
        Draft::Draft4 => quote!(Draft4),
        Draft::Draft202012 => quote!(Draft202012),
        Draft::Unspecified | Draft::Other(_) => return Err("unqualified schema draft".to_owned()),
    };
    let expanded = jsonschema_macros_core::expand(
        quote! {
            path = #path_literal, draft = #draft, validate_formats = true,
            ignore_unknown_formats = false,
            formats = {
                "uuid" => crate::reference_uuid_format,
                "hostname" => crate::reference_hostname_format
            },
            pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
            methods = { is_valid = true, validate = false, iter_errors = false }
        },
        quote!(
            pub struct BundleProof;
        ),
    );
    let mut syntax = syn::parse2(expanded).map_err(|error| error.to_string())?;
    reject_compiler_errors(&mut syntax)?;
    schema_cache_bounds::emitted_patterns(&syntax).map_err(|error| format!("{error:?}"))
}

fn reject_compiler_errors(syntax: &mut syn::File) -> Result<(), String> {
    use syn::visit_mut::VisitMut;

    struct CompilerErrors(bool);
    impl VisitMut for CompilerErrors {
        fn visit_macro_mut(&mut self, i: &mut syn::Macro) {
            if i.path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "compile_error")
            {
                self.0 = true;
            }
        }
    }
    let mut errors = CompilerErrors(false);
    errors.visit_file_mut(syntax);
    if errors.0 {
        Err("schema compiler emitted compile_error!".to_owned())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::schema_build::application::Adapter;

    #[test]
    fn priority_contract_approval_pins_reviewed_inputs_without_acceptance_grants() {
        let review: toml::Value = toml::from_str(include_str!(
            "conformance/fixtures/waves/priority-approval.toml"
        ))
        .unwrap();
        assert_eq!(
            review
                .as_table()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            [
                "schema_version",
                "kind",
                "review_status",
                "reviewer",
                "approved_on",
                "scope",
                "promotion_allowed",
                "resource_qualification_granted",
                "onboarding_acceptance_granted",
                "contracts"
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(review["schema_version"].as_integer(), Some(1));
        for (key, value) in [
            ("kind", "priority_wave_contract_review"),
            ("review_status", "approved"),
            ("reviewer", "AG"),
            ("scope", "contract_requirements_only"),
        ] {
            assert_eq!(review[key].as_str(), Some(value), "{key}");
        }
        assert_eq!(
            review["approved_on"].as_datetime().unwrap().to_string(),
            "2026-10-30"
        );
        for key in [
            "promotion_allowed",
            "resource_qualification_granted",
            "onboarding_acceptance_granted",
        ] {
            assert_eq!(review[key].as_bool(), Some(false), "{key}");
        }
        let inputs = [
            (
                "priority.toml",
                include_bytes!("conformance/fixtures/waves/priority.toml").as_slice(),
            ),
            (
                "priority-supplemental.toml",
                include_bytes!("conformance/fixtures/waves/priority-supplemental.toml").as_slice(),
            ),
        ];
        let records = review["contracts"].as_array().unwrap();
        assert_eq!(records.len(), inputs.len());
        let mut seen = BTreeSet::new();
        for record in records {
            assert_eq!(
                record
                    .as_table()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                ["source_path", "source_bytes", "source_sha256"]
                    .into_iter()
                    .collect()
            );
            let name = record["source_path"].as_str().unwrap();
            assert!(seen.insert(name), "duplicate approval input {name}");
            let (_, bytes) = inputs
                .iter()
                .find(|(path, _)| *path == name)
                .expect("reviewed contract input");
            assert_eq!(
                record["source_bytes"].as_integer(),
                Some(i64::try_from(bytes.len()).unwrap())
            );
            assert_eq!(
                record["source_sha256"].as_str(),
                Some(format!("{:x}", Sha256::digest(bytes)).as_str())
            );
        }
    }

    fn selected_regex_patterns(selection: &Value) -> BTreeMap<String, &str> {
        let mut owners = BTreeMap::new();
        for schema in selection["schemas"].as_array().unwrap() {
            for helper in schema["generated_helpers"].as_array().unwrap() {
                let owner = format!(
                    "{}:{}",
                    schema["witness"]["name"].as_str().unwrap(),
                    helper[0].as_str().unwrap()
                );
                assert!(owners.insert(owner, helper[1].as_str().unwrap()).is_none());
            }
        }
        for helper in selection["shared_regex_helpers"].as_array().unwrap() {
            assert!(
                owners
                    .insert(
                        helper[0].as_str().unwrap().to_owned(),
                        helper[1].as_str().unwrap()
                    )
                    .is_none()
            );
        }
        // Pin the reached pattern multiset, retaining distinct owners of the
        // same digit pattern without coupling shared proofs to bidder names.
        let mut actual = owners.values().copied().collect::<Vec<_>>();
        actual.sort_unstable();
        let mut expected = vec![
            "^[0-9]+$",
            "^[0-9]+$",
            r"^[0-9]+(\.[0-9]+)?$",
            r"\.[a-zA-Z]{2,3}$",
            HOSTNAME_SOURCE,
        ];
        expected.sort_unstable();
        assert_eq!(actual, expected);
        assert_eq!(owners["shared-hostname"], HOSTNAME_SOURCE);
        owners
    }

    fn compiler_private_layout_report(
        package: &str,
        target: &str,
        features: &[&str],
        names: &[&str],
    ) -> Value {
        use super::schema_build::compiler_layout::{Request, inspect};
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        inspect(
            &Request {
                workspace: &workspace,
                package,
                target,
                features,
                qualified_types: names,
            },
            |bytes, requested| {
                let fields = super::schema_compiler_layout::layouts(bytes, requested)
                    .map_err(|error| format!("compiler layout reader: {error:?}"))?;
                let rows = fields
                    .into_iter()
                    .map(|(name, field)| {
                        (
                            name,
                            json!({"bytes": field.bytes, "alignment": field.alignment}),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                Ok(json!(rows))
            },
        )
        .unwrap_or_else(|error| panic!("{package}, {target}: {error}"))
    }

    fn compiler_facts(report: &Value) -> BTreeMap<String, super::schema_heap_layout::NativeField> {
        use super::schema_heap_layout::NativeField;
        use super::schema_syntax_construction::ABI_POINTER;
        let mut facts = report["layouts"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, row)| {
                (
                    name.clone(),
                    NativeField {
                        bytes: usize::try_from(row["bytes"].as_u64().unwrap()).unwrap(),
                        alignment: usize::try_from(row["alignment"].as_u64().unwrap()).unwrap(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let bits = report["binding"]["abi_and_producers"]["abi"]["pointer_bits"]
            .as_u64()
            .expect("verified compiler artifact ABI");
        assert!(matches!(bits, 32 | 64));
        let word = usize::try_from(bits.checked_div(8).unwrap()).unwrap();
        assert!(
            facts
                .insert(
                    ABI_POINTER.into(),
                    NativeField {
                        bytes: word,
                        alignment: word,
                    }
                )
                .is_none()
        );
        facts
    }

    fn syntax_components(report: &Value) -> Value {
        use super::schema_selection_source::{Lane, selected_cohort};
        use super::schema_syntax_construction::SyntaxConstructionCertificate;
        let facts = compiler_facts(report);
        let source = Path::new(report["binding"]["source"]["source_root"].as_str().unwrap());
        let mut certificates = Vec::new();
        for lane in [Lane::Secure, Lane::Development] {
            let cohort = selected_cohort(lane).unwrap();
            check_current(&cohort.manifest["source"]).unwrap();
            let patterns = selected_regex_patterns(&cohort.manifest["selection"]);
            assert_eq!(patterns.len(), 5);
            for (owner, pattern) in patterns {
                let certificate =
                    SyntaxConstructionCertificate::from_pattern(pattern, &facts, source)
                        .unwrap_or_else(|error| panic!("{owner}: {error:?}"));
                assert!(certificate.peak_requested_bytes > 0);
                assert!(certificate.work_upper_bound > 0);
                certificates.push(json!({"owner":owner, "pattern":pattern,
                    "lane":cohort.manifest["lane"],
                    "cohort_sha256":format!("{:x}", Sha256::digest(serde_json::to_vec(&cohort.manifest).unwrap())),
                    "peak_requested_bytes":certificate.peak_requested_bytes,
                    "source_record_visit_upper_bound":certificate.work_upper_bound,
                    "bound_class":"conditional_target_syntax_component",
                    "target":report["binding"]["target"],
                    "resource_qualification":false}));
            }
        }
        assert_eq!(certificates.len(), 10);
        json!(certificates)
    }

    fn nfa_layout_requests() -> BTreeMap<&'static str, &'static str> {
        use super::schema_nfa_heap::target::{GROUP_INFO_INNER, NFA_INNER};
        use regex_automata::nfa::thompson::{State, Transition};
        use regex_automata::util::primitives::{SmallIndex, StateID};
        use std::{
            any::type_name,
            collections::HashMap,
            sync::{Arc, atomic::AtomicUsize},
        };
        // DWARF includes default generic arguments which type_name omits.
        // These are exact compiler requests, never layout guesses or fallbacks.
        BTreeMap::from([
            (NFA_INNER, NFA_INNER),
            (GROUP_INFO_INNER, GROUP_INFO_INNER),
            (type_name::<State>(), type_name::<State>()),
            (type_name::<Transition>(), type_name::<Transition>()),
            (type_name::<StateID>(), type_name::<StateID>()),
            (
                type_name::<(SmallIndex, SmallIndex)>(),
                type_name::<(SmallIndex, SmallIndex)>(),
            ),
            (type_name::<AtomicUsize>(), type_name::<AtomicUsize>()),
            (
                type_name::<HashMap<Arc<str>, SmallIndex>>(),
                "std::collections::hash::map::HashMap<alloc::sync::Arc<str, alloc::alloc::Global>, regex_automata::util::primitives::SmallIndex, std::hash::random::RandomState, alloc::alloc::Global>",
            ),
            (
                type_name::<Vec<Option<Arc<str>>>>(),
                "alloc::vec::Vec<core::option::Option<alloc::sync::Arc<str, alloc::alloc::Global>>, alloc::alloc::Global>",
            ),
            (
                type_name::<Option<Arc<str>>>(),
                "core::option::Option<alloc::sync::Arc<str, alloc::alloc::Global>>",
            ),
        ])
    }

    fn nfa_components(report: &Value) -> Value {
        use super::schema_cache_bounds::{forward_nfa, reverse_nfa};
        use super::schema_nfa_heap::target::{ABI_POINTER, fact_keys, project_nfa};
        use super::schema_selection_source::{Lane, selected_cohort};
        let reported = compiler_facts(report);
        let mapping = nfa_layout_requests();
        assert_eq!(
            mapping.keys().copied().collect::<BTreeSet<_>>(),
            fact_keys()
                .into_iter()
                .filter(|name| *name != ABI_POINTER)
                .collect()
        );
        let mut facts = BTreeMap::from([(ABI_POINTER.to_owned(), reported[ABI_POINTER])]);
        for (role, compiler_name) in mapping {
            assert!(
                facts
                    .insert(role.to_owned(), reported[compiler_name])
                    .is_none()
            );
        }
        let mut certificates = Vec::new();
        for lane in [Lane::Secure, Lane::Development] {
            let cohort = selected_cohort(lane).unwrap();
            check_current(&cohort.manifest["source"]).unwrap();
            let patterns = selected_regex_patterns(&cohort.manifest["selection"]);
            assert_eq!(patterns.len(), 5);
            for (owner, pattern) in patterns {
                for reverse in [false, true] {
                    let nfa = if reverse {
                        reverse_nfa(pattern)
                    } else {
                        forward_nfa(pattern)
                    }
                    .unwrap();
                    let certificate = project_nfa(&nfa, &facts)
                        .unwrap_or_else(|error| panic!("{owner}, reverse={reverse}: {error:?}"));
                    assert!(certificate.requested_heap_bytes > 0);
                    certificates.push(json!({"owner":owner,"pattern":pattern,"reverse":reverse,
                        "lane":cohort.manifest["lane"],
                        "cohort_sha256":format!("{:x}", Sha256::digest(serde_json::to_vec(&cohort.manifest).unwrap())),
                        "target":report["binding"]["target"],"states":certificate.states,
                        "arc_bytes":certificate.arc_bytes,"states_bytes":certificate.states_bytes,
                        "start_patterns_bytes":certificate.start_patterns_bytes,
                        "sparse_bytes":certificate.sparse_bytes,"union_bytes":certificate.union_bytes,
                        "group_info_bytes":certificate.group_info.requested_heap_bytes,
                        "requested_heap_bytes":certificate.requested_heap_bytes,
                        "bound_class":"conditional_target_retained_nfa_component",
                        "resource_qualification":false}));
                }
            }
        }
        assert_eq!(certificates.len(), 20);
        json!(certificates)
    }

    fn repository_rust_release() -> String {
        let pins = include_str!("../../../.tool-versions")
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                (fields.next() == Some("rust")).then(|| {
                    let version = fields.next().expect("Rust toolchain pin");
                    assert_eq!(fields.next(), None);
                    version
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(pins.len(), 1);
        format!("release: {}", pins[0])
    }

    fn attach_target_components(report: &mut Value, package: &str) {
        if package == "regex-syntax" {
            report["target_syntax_model_sources"] = json!({
                "syntax_construction": format!("{:x}", Sha256::digest(include_bytes!("schema_syntax_construction/mod.rs"))),
                "heap_layout": format!("{:x}", Sha256::digest(include_bytes!("schema_heap_layout/mod.rs"))),
                "producer": format!("{:x}", Sha256::digest(include_bytes!("schema_source_conformance.rs"))),
            });
            report["target_syntax_components"] = syntax_components(report);
        } else {
            assert_eq!(package, "regex-automata");
            report["target_nfa_model_sources"] = json!({
                "target_projection": format!("{:x}", Sha256::digest(include_bytes!("schema_nfa_heap/target.rs"))),
                "native_owner_source": format!("{:x}", Sha256::digest(include_bytes!("schema_nfa_heap/mod.rs"))),
                "topology_constructor": format!("{:x}", Sha256::digest(include_bytes!("schema_cache_model/mod.rs"))),
                "producer": format!("{:x}", Sha256::digest(include_bytes!("schema_source_conformance.rs"))),
            });
            report["target_nfa_layout_names"] = json!(nfa_layout_requests());
            report["target_nfa_components"] = nfa_components(report);
        }
    }

    #[test]
    fn compiler_private_layout_facts_cover_native_and_every_guest_abi() {
        use super::schema_build::schema_regex_vendor::FEATURES;
        use super::schema_syntax_construction::{PAYLOAD_TYPES, PRIVATE_TYPES};
        use std::io::{Write as _, stderr};
        use std::mem::{align_of, size_of};

        let syntax_features = [
            // The application graph's regex dependency enables this Cargo
            // alias. Its std/unicode closure is unchanged; replay its exact cfg.
            "default",
            "std",
            "unicode",
            "unicode-age",
            "unicode-bool",
            "unicode-case",
            "unicode-gencat",
            "unicode-perl",
            "unicode-script",
            "unicode-segment",
        ];
        let mut syntax_names = PRIVATE_TYPES.to_vec();
        syntax_names.extend(PAYLOAD_TYPES);
        syntax_names.push("regex_syntax::hir::literal::State");
        let mut engine_names = vec![
            "regex_automata::nfa::thompson::range_trie::State",
            "regex_automata::dfa::onepass::Epsilons",
            "regex_automata::dfa::onepass::Transition",
            "regex_automata::nfa::thompson::builder::State",
            "regex_automata::nfa::thompson::nfa::State",
            "regex_automata::nfa::thompson::nfa::Transition",
        ];
        for name in nfa_layout_requests().into_values() {
            if !engine_names.contains(&name) {
                engine_names.push(name);
            }
        }
        let release = repository_rust_release();
        let mut checked = 0_usize;
        for target in [
            "native",
            "wasm32-unknown-unknown",
            "wasm32-wasip1",
            "wasm32-wasip2",
        ] {
            for (package, features, names) in [
                (
                    "regex-syntax",
                    syntax_features.as_slice(),
                    syntax_names.as_slice(),
                ),
                (
                    "regex-automata",
                    FEATURES.as_slice(),
                    engine_names.as_slice(),
                ),
            ] {
                let mut report = compiler_private_layout_report(package, target, features, names);
                assert_eq!(report["qualification"], "Unqualified");
                assert_eq!(report["execution"], "not_executed");
                assert_eq!(report["resource_qualification"], false);
                assert_eq!(report["serving_qualification"], false);
                assert!(
                    report["binding"]["compiler_verbose"]
                        .as_str()
                        .unwrap()
                        .lines()
                        .any(|line| line == release)
                );
                let rows = report["layouts"].as_object().unwrap();
                assert_eq!(rows.len(), names.len());
                for name in names {
                    let bytes = rows[*name]["bytes"].as_u64().unwrap();
                    let alignment = rows[*name]["alignment"].as_u64().unwrap();
                    assert!(bytes > 0);
                    assert!(alignment.is_power_of_two());
                    assert!(bytes.is_multiple_of(alignment));
                }
                if target == "native" && package == "regex-syntax" {
                    use regex_syntax::hir::Hir;
                    assert_eq!(
                        rows["regex_syntax::hir::Hir"],
                        json!({
                            "bytes": size_of::<Hir>(),
                            "alignment": align_of::<Hir>(),
                        })
                    );
                }
                attach_target_components(&mut report, package);
                writeln!(
                    stderr().lock(),
                    "{}",
                    serde_json::to_string(&report).unwrap()
                )
                .unwrap();
                checked += 1;
            }
        }
        assert_eq!(checked, 8);
    }

    #[test]
    fn actual_application_artifacts_axum_retain_every_selected_validator_in_both_feature_lanes() {
        assert_actual_application_artifacts(Adapter::Axum, "auctioneer-adapter-axum");
    }

    #[test]
    fn actual_application_artifacts_cloudflare_retain_every_selected_validator_in_both_feature_lanes()
     {
        assert_actual_application_artifacts(Adapter::Cloudflare, "auctioneer-adapter-cloudflare");
    }

    #[test]
    fn actual_application_artifacts_fastly_retain_every_selected_validator_in_both_feature_lanes() {
        assert_actual_application_artifacts(Adapter::Fastly, "auctioneer-adapter-fastly");
    }

    #[test]
    fn actual_application_artifacts_spin_retain_every_selected_validator_in_both_feature_lanes() {
        assert_actual_application_artifacts(Adapter::Spin, "auctioneer-adapter-spin");
    }

    fn assert_actual_application_artifacts(adapter: Adapter, package: &str) {
        use super::schema_build::application::inspect;
        use super::schema_selection_source::{Lane, selected_cohort};
        use std::io::{Write as _, stderr};

        for (lane, development) in [(Lane::Secure, false), (Lane::Development, true)] {
            let cohort = selected_cohort(lane).expect("selected application source cohort");
            let report = inspect(
                adapter,
                &cohort.root,
                &cohort.schemas,
                &cohort.manifest,
                development,
            )
            .expect("actual linked application schema membership");
            assert_eq!(
                report["lane"],
                if development { "development" } else { "secure" }
            );
            assert_eq!(report["qualification"], "Unqualified");
            assert_eq!(report["resource_qualification"], false);
            assert_eq!(report["linked_or_serving_qualification"], false);
            assert_eq!(report["final_admitted_artifact_recheck_required"], true);
            let applications = report["applications"].as_array().unwrap();
            assert_eq!(applications.len(), 1);
            let expected = cohort
                .schemas
                .iter()
                .map(|row| row.witness.name.as_str())
                .collect::<BTreeSet<_>>();
            for application in applications {
                assert_eq!(application["package"], package);
                assert_eq!(application["qualification"], "Unqualified");
                let rows = application["linked_records"].as_array().unwrap();
                assert_eq!(rows.len(), expected.len());
                assert_eq!(
                    rows.iter()
                        .map(|row| row["owner"].as_str().unwrap())
                        .collect::<BTreeSet<_>>(),
                    expected
                );
                assert_eq!(application["refusal_controls"].as_array().unwrap().len(), 8);
            }
            writeln!(
                stderr().lock(),
                "{}",
                serde_json::to_string(&report).unwrap()
            )
            .unwrap();
        }
    }

    #[test]
    fn normal_application_dependency_graphs_keep_schema_runtime_native_only() {
        use super::schema_build::successful;
        use std::{path::Path, process::Command, str::from_utf8, time::Duration};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let applications = [
            ("auctioneer-adapter-axum", None),
            (
                "auctioneer-adapter-cloudflare",
                Some("wasm32-unknown-unknown"),
            ),
            ("auctioneer-adapter-fastly", Some("wasm32-wasip1")),
            ("auctioneer-adapter-spin", Some("wasm32-wasip2")),
        ];
        assert_eq!(applications.len(), 4);
        for (package, target) in applications {
            let mut command = Command::new("cargo");
            command.current_dir(&root).args([
                "tree",
                "--locked",
                "--offline",
                "-p",
                package,
                "--edges",
                "normal,no-proc-macro",
                "--prefix",
                "none",
                "--format",
                "{p}",
            ]);
            if let Some(triple) = target {
                command.args(["--target", triple]);
            }
            let output = successful(&mut command, Duration::from_secs(30)).unwrap();
            let dependencies = from_utf8(&output.stdout).unwrap();
            assert!(
                dependencies
                    .lines()
                    .any(|line| line.split_whitespace().next() == Some(package)),
                "{package}: graph must include the selected root"
            );
            // Controlled-native serving deliberately links the schema runtime.
            // Guests must still exclude it outside artifact inspection.
            assert_eq!(
                dependencies
                    .lines()
                    .any(|line| line.split_whitespace().next() == Some("jsonschema")),
                target.is_none(),
                "{package}: normal schema runtime must be native-only"
            );
        }
    }

    #[test]
    fn staged_snapshot_compiler_inputs_are_watched_and_changed_bytes_refuse() {
        use auctioneer_config_schema::bidder_discovery::scan;
        use auctioneer_config_schema::schema_staging::BuildContext;
        use auctioneer_config_schema::schema_staging_build::{
            BuildRequest, generate, write_snapshots,
        };
        use std::{collections::BTreeMap, fs, path::Path};

        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let discovery = scan(crate_dir).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let out_dir = temporary.path().canonicalize().unwrap();
        let environment = BTreeMap::new();
        let generated = generate(BuildRequest {
            crate_dir,
            out_dir: &out_dir,
            discovery: &discovery,
            metadata: auctioneer_config::bidder_metadata_declarations(),
            context: BuildContext {
                target: "fake_target",
                profile: "fake_profile",
                enabled_features: &[],
                compiler_verbose: "fake compiler identity",
                encoded_rustflags: "",
            },
            build_environment: &environment,
        })
        .unwrap();
        assert!(!generated.snapshots.is_empty());
        write_snapshots(&out_dir, &generated.snapshots).unwrap();
        for snapshot in &generated.snapshots {
            let path = out_dir.join(&snapshot.relative_path);
            assert!(
                generated.rerun_paths.contains(&path),
                "compiler input must rerun identity validation: {}",
                path.display()
            );
            fs::write(
                &path,
                serde_json::to_vec(&serde_json::json!(false)).unwrap(),
            )
            .unwrap();
            assert_eq!(
                write_snapshots(&out_dir, &generated.snapshots).unwrap_err(),
                "conflicting existing schema snapshot bytes"
            );
            fs::write(&path, &snapshot.bytes).unwrap();
        }
        write_snapshots(&out_dir, &generated.snapshots).unwrap();
    }

    #[test]
    fn staged_artifact_metadata_matches_the_source_bound_cohort_without_qualification() {
        use super::schema_selection_source::{Lane, selected_cohort};
        use auctioneer_bidder::test_support::schema_staging_manifest;
        use std::env::var;
        let expected_mode = var("AUCTIONEER__TEST__EXPECT_SCHEMA_ARTIFACT").ok();
        let captured_manifest = schema_staging_manifest();
        if expected_mode.is_none() {
            assert!(
                captured_manifest.is_none(),
                "normal builds must not embed staging metadata"
            );
            return;
        }
        assert_eq!(expected_mode.as_deref(), Some("unqualified"));
        let manifest: serde_json::Value =
            serde_json::from_str(captured_manifest.expect("actual staged build metadata")).unwrap();
        assert_eq!(manifest["schema_version"], 1_u8);
        assert_eq!(manifest["kind"], "unqualified_non_serving_schema_staging");
        assert_eq!(manifest["identity"]["qualification"], "Unqualified");
        assert_eq!(manifest["linked_or_serving_qualification"], false);
        assert_eq!(manifest["artifact_inspection"], "pending");
        let lane = if cfg!(feature = "insecure-bidders") {
            Lane::Development
        } else {
            Lane::Secure
        };
        let cohort = selected_cohort(lane).unwrap();
        let owners = cohort
            .schemas
            .iter()
            .map(|row| row.witness.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            manifest["selection"]["schema_owners"],
            serde_json::json!(owners)
        );
        let members = cohort.manifest["selection"]["members"].as_array().unwrap().iter().map(|row| serde_json::json!({
            "name":row["name"],"implementation":row["implementation"],"is_root":row["is_root"],
            "ancestry":row["ancestry"],"schema_owner":row["effective_schema_owner"]
        })).collect::<Vec<_>>();
        assert_eq!(manifest["selection"]["members"], serde_json::json!(members));
        let schemas = manifest["schemas"].as_array().unwrap();
        assert_eq!(schemas.len(), cohort.schemas.len());
        for (actual, expected_schema) in schemas.iter().zip(&cohort.schemas) {
            let expected_witness = serde_json::to_value(&expected_schema.witness).unwrap();
            for (field, value) in expected_witness.as_object().unwrap() {
                assert_eq!(&actual[field], value, "schema witness {field}");
            }
        }
        assert_eq!(
            manifest["build"]["enabled_features"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row == "insecure-bidders"),
            cfg!(feature = "insecure-bidders")
        );
    }

    #[test]
    fn selected_schema_probe_links_and_executes_the_source_bound_cohort() {
        use super::schema_selection_source::{Lane, selected_cohort};
        use std::io::{Write as _, stderr};
        let runtime = super::schema_build::guest::Runtime::discover()
            .expect("repository-pinned local WASI runner");
        for lane in [Lane::Secure, Lane::Development] {
            let cohort = selected_cohort(lane).expect("selected source cohort");
            let semantics = super::schema_selected_semantics::for_cohort(&cohort)
                .expect("source-bound selected semantic cases");
            let (catalog, mut report) = link_selected_semantics(&cohort, &semantics);
            assert_eq!(report["schema_version"], 1_u8);
            assert_eq!(report["application_or_resource_qualification"], false);
            assert_eq!(catalog.reports().len(), 4);
            assert_eq!(
                catalog
                    .reports()
                    .iter()
                    .map(|row| row.target.as_str())
                    .collect::<Vec<_>>(),
                [
                    "native",
                    "wasm32-unknown-unknown",
                    "wasm32-wasip1",
                    "wasm32-wasip2"
                ]
            );
            assert_eq!(report["binding"], cohort.manifest);
            assert_eq!(
                report["generated_source_sha256"],
                report["probe_sources_sha256"]["src/main.rs"]
            );
            for artifact in catalog.reports() {
                assert_eq!(artifact.validators, cohort.schemas.len());
                assert!(artifact.artifact_bytes > 0);
            }
            let initialized = catalog.initialize_native().unwrap();
            let names: Vec<_> = cohort
                .schemas
                .iter()
                .map(|row| row.witness.name.as_str())
                .collect();
            assert_eq!(initialized["names"], serde_json::json!(names));
            for pass in ["first", "repeated"] {
                assert_eq!(
                    initialized[pass],
                    serde_json::json!(vec![true; names.len()])
                );
            }
            let mut checked = 0_usize;
            for row in &cohort.schemas {
                assert!(
                    !catalog
                        .check_native(&row.witness.name, &serde_json::Value::Null)
                        .unwrap()
                );
                checked += 1;
            }
            assert_eq!(checked, cohort.schemas.len());
            report["native_capture_execution"] = assert_selected_parameter_cases(&catalog, &names);
            assert_eq!(report["selected_semantic_inputs"], json!(semantics));
            report["native_selected_semantics"] = assert_selected_semantic_cases(
                &semantics,
                |name, inputs| {
                    inputs
                        .iter()
                        .map(|input| catalog.check_native_raw(name, input))
                        .collect()
                },
                |name, inputs| catalog.check_native_sequence(name, inputs),
            );
            report["native_program_heap_candidate"] =
                assert_selected_program_heap(&cohort.manifest, &catalog);
            report["native_cache_heap_candidate"] = assert_selected_cache_heap(&cohort, &catalog);
            assert_eq!(
                report["native_cache_heap_candidate"]["kind"],
                "conditional_native_schema_cache_heap_candidate"
            );
            report["wasi_execution"] =
                assert_selected_wasi_execution(&runtime, &catalog, &report, &names, &semantics);
            report["unknown_execution"] =
                assert_selected_bare_execution(&runtime, &catalog, &report, &names, &semantics);
            report["pending_gaps"] = json!([
                "native_sections",
                "remaining_required_semantics",
                "serving_integration",
                "resident_resource_bounds",
                "all_target_qualification"
            ]);
            writeln!(stderr(), "{}", serde_json::to_string(&report).unwrap()).unwrap();
        }
    }

    fn link_selected_semantics(
        cohort: &super::schema_selection_source::SelectedCohort,
        semantics: &super::schema_selected_semantics::Batch,
    ) -> (super::schema_build::Catalog, Value) {
        let witness = json!(semantics);
        let rows = semantics
            .rows
            .iter()
            .map(|row| super::schema_build::bare::SemanticInput {
                schema: &row.schema,
                input_json: &row.input_json,
            })
            .collect::<Vec<_>>();
        let names = cohort
            .schemas
            .iter()
            .map(|schema| schema.witness.name.as_str())
            .collect::<Vec<_>>();
        let data = super::schema_build::bare::semantic_data(&names, &rows).unwrap();
        let linked = super::schema_build::selected::link(
            &cohort.root,
            &cohort.schemas,
            &cohort.manifest,
            Some(super::schema_build::selected::SemanticInputs {
                data: &data,
                witness: &witness,
            }),
        )
        .expect("linked selected schema probe");
        assert_eq!(
            json!(super::schema_selected_semantics::for_cohort(cohort).unwrap()),
            witness,
            "selected semantic inputs must not change during child builds"
        );
        linked
    }

    fn assert_selected_wasi_execution(
        runtime: &super::schema_build::guest::Runtime,
        catalog: &super::schema_build::Catalog,
        report: &Value,
        names: &[&str],
        semantics: &super::schema_selected_semantics::Batch,
    ) -> Value {
        runtime.verify().unwrap();
        let mut rows = Vec::new();
        let native = catalog
            .reports()
            .iter()
            .find(|row| row.target == "native")
            .unwrap();
        let selected = report["binding"]["selection"]["schemas"]
            .as_array()
            .unwrap();
        let helper_counts: Vec<_> = selected
            .iter()
            .map(|row| {
                (
                    row["witness"]["name"].as_str().unwrap(),
                    row["generated_helpers"].as_array().unwrap().len(),
                )
            })
            .collect();
        assert_eq!(
            helper_counts
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            names
        );
        let native_layouts = catalog.native_cache_layouts().unwrap();
        native_layouts
            .validate(size_of::<usize>(), &helper_counts)
            .unwrap();
        for target in ["wasm32-wasip1", "wasm32-wasip2"] {
            let artifact = catalog
                .reports()
                .iter()
                .find(|row| row.target == target)
                .unwrap();
            let inventory = runtime.inventory(catalog, target).unwrap();
            assert_eq!(inventory["names"], json!(names));
            assert_eq!(inventory["results"], json!(native.results));
            let membership: Value =
                serde_json::from_str(inventory["membership"].as_str().unwrap()).unwrap();
            assert_eq!(membership, report["membership"]);
            let layouts = runtime.cache_layouts(catalog, target).unwrap();
            layouts.validate(4, &helper_counts).unwrap();
            let initialized = runtime.initialize(catalog, target).unwrap();
            assert_eq!(initialized["names"], json!(names));
            for phase in ["first", "repeated"] {
                assert_eq!(initialized[phase], json!(vec![true; names.len()]));
            }
            let cases = assert_parameter_cases(
                names,
                |name, inputs| runtime.raw_table(catalog, target, name, inputs),
                |name, inputs| runtime.sequence(catalog, target, name, inputs),
            );
            let semantic_cases = assert_selected_semantic_cases(
                semantics,
                |name, inputs| runtime.raw_table(catalog, target, name, inputs),
                |name, inputs| runtime.sequence(catalog, target, name, inputs),
            );
            rows.push(json!({"target": target, "status": "executed",
                "artifact_sha256": artifact.sha256, "artifact_bytes": artifact.artifact_bytes,
                "membership": membership, "generated_regex_initialization": initialized, "captured_cases": cases,
                "selected_semantics": semantic_cases,
                "cache_layouts": layouts,
                "cache_reuse_scope": "one command invocation",
                "raw_capture_dispatch": "raw-string table; scalar fallback for oversized encoded tables",
                "initialization_scope": "generated hooks; excludes custom formats and search caches"}));
        }
        assert_eq!(rows.len(), 2);
        runtime.verify().unwrap();
        json!({"schema_version": 3_u8, "kind": "local_wasi_execution_observation",
            "application_or_resource_qualification": false, "runtime": runtime.identity(),
            "native_release_cache_layouts": native_layouts,
            "targets": rows,
            "scope": "WASIp1/P2 command probes; unknown target has a separate integer-ABI observation",
            "excludes": ["provider_handler_execution", "guest_memory_and_work_bounds", "production_host_guarantees"]})
    }

    #[derive(Clone, Copy)]
    enum BareInputs {
        Captured,
        SelectedSemantics,
    }

    fn bare_rows(
        runtime: &super::schema_build::guest::Runtime,
        catalog: &super::schema_build::Catalog,
        names: &[&str],
        name: &str,
        expected: usize,
        reuse: bool,
        inputs: BareInputs,
    ) -> Result<Vec<super::schema_build::RawVerdict>, String> {
        use super::schema_build::bare::{Call, decode};
        let ordinal = names
            .iter()
            .position(|candidate| *candidate == name)
            .ok_or("unknown bare schema")?;
        let schema = u32::try_from(ordinal).map_err(|_error| "bare schema index overflow")?;
        let count_call = match inputs {
            BareInputs::Captured => Call::CaseCount(schema),
            BareInputs::SelectedSemantics => Call::SemanticCount(schema),
        };
        let count = runtime.bare_scalar(catalog, count_call)?;
        if count != u64::try_from(expected).map_err(|_error| "bare case count overflow")? {
            return Err("bare case count mismatch".to_owned());
        }
        let mut results = Vec::new();
        for offset in (0..expected).step_by(16) {
            let start = u32::try_from(offset).map_err(|_error| "bare case index overflow")?;
            let call = match inputs {
                BareInputs::Captured => Call::Replay {
                    schema,
                    start,
                    reuse,
                },
                BareInputs::SelectedSemantics => Call::SemanticReplay {
                    schema,
                    start,
                    reuse,
                },
            };
            let packet = runtime.bare_scalar(catalog, call)?;
            let remaining = expected
                .checked_sub(offset)
                .ok_or("bare case offset overflow")?;
            results.extend(decode(packet, remaining.min(16))?);
        }
        if results.len() != expected {
            return Err("bare replay count mismatch".to_owned());
        }
        Ok(results)
    }

    struct BareInitialization {
        case_counts: Vec<Value>,
        empty_results: Vec<bool>,
        initialized: Vec<u64>,
        layouts: super::schema_build::CacheLayouts,
    }

    fn assert_bare_initialization(
        runtime: &super::schema_build::guest::Runtime,
        catalog: &super::schema_build::Catalog,
        report: &Value,
        names: &[&str],
        corpus: &super::schema_reference::Corpus,
    ) -> BareInitialization {
        use super::schema_build::{
            CacheLayout, CacheLayouts,
            bare::{Call, LayoutField},
        };
        let native = catalog
            .reports()
            .iter()
            .find(|row| row.target == "native")
            .unwrap();
        assert_eq!(native.names, names);
        let mut case_counts = Vec::new();
        let mut empty_results = Vec::new();
        let mut initialized = Vec::new();
        let mut layout_rows = Vec::new();
        let read_layout = |schema, field| {
            usize::try_from(
                runtime
                    .bare_scalar(catalog, Call::Layout { schema, field })
                    .unwrap(),
            )
            .unwrap()
        };
        for (index, name) in names.iter().enumerate() {
            let ordinal = u32::try_from(index).unwrap();
            let cases = corpus
                .cases
                .iter()
                .filter(|case| case.schema == *name)
                .count();
            assert_eq!(
                runtime
                    .bare_scalar(catalog, Call::CaseCount(ordinal))
                    .unwrap(),
                u64::try_from(cases).unwrap()
            );
            case_counts.push(json!({"name":name,"cases":cases}));
            let init = runtime
                .bare_scalar(catalog, Call::Initialize(ordinal))
                .unwrap();
            assert_eq!(init, 3_u64);
            initialized.push(init);
            let empty = runtime.bare_scalar(catalog, Call::Empty(ordinal)).unwrap();
            assert_eq!(empty, u64::from(native.results[index]));
            empty_results.push(empty == 1);
            layout_rows.push(CacheLayout {
                name: (*name).to_owned(),
                inline_bytes: read_layout(ordinal, LayoutField::WrapperBytes),
                alignment: read_layout(ordinal, LayoutField::WrapperAlignment),
            });
        }
        let layouts = CacheLayouts {
            schema_version: 2,
            word_bytes: read_layout(0, LayoutField::Word),
            cache_bytes: read_layout(0, LayoutField::CacheBytes),
            cache_alignment: read_layout(0, LayoutField::CacheAlignment),
            rows: layout_rows,
        };
        let helpers = report["binding"]["selection"]["schemas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["witness"]["name"].as_str().unwrap(),
                    row["generated_helpers"].as_array().unwrap().len(),
                )
            })
            .collect::<Vec<_>>();
        layouts.validate(4, &helpers).unwrap();
        BareInitialization {
            case_counts,
            empty_results,
            initialized,
            layouts,
        }
    }

    fn assert_bare_captured_cases(
        runtime: &super::schema_build::guest::Runtime,
        catalog: &super::schema_build::Catalog,
        names: &[&str],
        corpus: &super::schema_reference::Corpus,
    ) -> Value {
        use super::schema_build::RawVerdict;
        assert_parameter_cases(
            names,
            |name, inputs| {
                let original = corpus
                    .cases
                    .iter()
                    .filter(|case| case.schema == name)
                    .map(|case| case.input.as_str())
                    .collect::<Vec<_>>();
                if inputs != original {
                    return Err("bare replay requires the original captured inputs".to_owned());
                }
                bare_rows(
                    runtime,
                    catalog,
                    names,
                    name,
                    inputs.len(),
                    false,
                    BareInputs::Captured,
                )
            },
            |name, inputs| {
                let original = corpus
                    .cases
                    .iter()
                    .filter(|case| case.schema == name)
                    .filter_map(|case| serde_json::from_str::<Value>(&case.input).ok())
                    .collect::<Vec<_>>();
                if inputs != original {
                    return Err("bare replay requires the original captured inputs".to_owned());
                }
                let raw_count = corpus
                    .cases
                    .iter()
                    .filter(|case| case.schema == name)
                    .count();
                let reused = bare_rows(
                    runtime,
                    catalog,
                    names,
                    name,
                    raw_count,
                    true,
                    BareInputs::Captured,
                )?;
                let expected_raw = corpus
                    .cases
                    .iter()
                    .filter(|case| case.schema == name)
                    .map(|case| match serde_json::from_str::<Value>(&case.input) {
                        Ok(_) => RawVerdict::Schema { valid: case.valid },
                        Err(_) => RawVerdict::InvalidJson,
                    })
                    .collect::<Vec<_>>();
                if reused != expected_raw {
                    return Err("bare reused raw verdict mismatch".to_owned());
                }
                Ok(reused
                    .into_iter()
                    .filter_map(|row| match row {
                        RawVerdict::Schema { valid } => Some(valid),
                        RawVerdict::InvalidJson => None,
                    })
                    .collect())
            },
        )
    }

    fn assert_bare_refusals(
        runtime: &super::schema_build::guest::Runtime,
        catalog: &super::schema_build::Catalog,
        names: &[&str],
        corpus: &super::schema_reference::Corpus,
    ) {
        use super::schema_build::bare::{Call, LayoutField};
        let (nonempty_index, nonempty_count) = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                (
                    index,
                    corpus
                        .cases
                        .iter()
                        .filter(|case| case.schema == *name)
                        .count(),
                )
            })
            .find(|(_, population)| *population > 0)
            .expect("selected cohort has captured inputs");
        let aligned_past_end = u32::try_from(nonempty_count.next_multiple_of(16)).unwrap();
        for call in [
            Call::CaseCount(u32::MAX),
            Call::Initialize(u32::MAX),
            Call::Empty(u32::MAX),
            Call::Layout {
                schema: u32::MAX,
                field: LayoutField::Word,
            },
            Call::Replay {
                schema: 0,
                start: 1,
                reuse: false,
            },
            Call::Replay {
                schema: 0,
                start: u32::MAX,
                reuse: true,
            },
            Call::Replay {
                schema: u32::try_from(nonempty_index).unwrap(),
                start: aligned_past_end,
                reuse: false,
            },
        ] {
            assert_eq!(
                runtime.bare_scalar(catalog, call).unwrap_err(),
                "bare probe refused"
            );
        }
    }

    fn assert_selected_bare_execution(
        runtime: &super::schema_build::guest::Runtime,
        catalog: &super::schema_build::Catalog,
        report: &Value,
        names: &[&str],
        semantics: &super::schema_selected_semantics::Batch,
    ) -> Value {
        use super::schema_build::bare::{self, Call};
        runtime.verify().unwrap();
        let version = runtime.bare_scalar(catalog, Call::Version).unwrap();
        let count = runtime.bare_scalar(catalog, Call::SchemaCount).unwrap();
        bare::check_header(version, count, names.len()).unwrap();
        let corpus = super::schema_reference::captured().unwrap();
        let BareInitialization {
            case_counts,
            empty_results,
            initialized,
            layouts,
        } = assert_bare_initialization(runtime, catalog, report, names, &corpus);
        let cases = assert_bare_captured_cases(runtime, catalog, names, &corpus);
        let semantic_cases = assert_bare_semantic_cases(runtime, catalog, names, semantics);
        assert_bare_refusals(runtime, catalog, names, &corpus);
        runtime.verify().unwrap();
        let artifact = catalog
            .reports()
            .iter()
            .find(|row| row.target == "wasm32-unknown-unknown")
            .unwrap();
        json!({"schema_version":1_u8,"kind":"local_integer_abi_guest_execution_observation",
            "application_or_resource_qualification":false,"target":"wasm32-unknown-unknown","status":"executed",
            "abi_version":version,"runtime":runtime.identity(),"artifact_sha256":artifact.sha256,"artifact_bytes":artifact.artifact_bytes,
            "unavailable_import_policy":"trap_on_call_never_default_values",
            "trapped_imports":report["bare_trapped_imports"],
            "input_data":report["bare_input_data"],"schema_names":names,"raw_case_counts":case_counts,
            "empty_results":empty_results,"generated_initializer_first_repeat_codes":initialized,
            "cache_layouts":layouts,"captured_cases":cases,
            "selected_semantics":semantic_cases,"semantic_input_data":report["bare_semantic_data"],
            "cache_reuse_scope":"fresh owner per parseable raw row; reused mode replays the original prefix per batch",
            "probe_overhead":"compiled capture table and observation parser; not adapter residency",
            "excludes":["provider_handler_execution","guest_heap_and_work_bounds","actual_application_correspondence","production_host_guarantees"]})
    }

    fn assert_selected_cache_heap(
        cohort: &super::schema_selection_source::SelectedCohort,
        catalog: &super::schema_build::Catalog,
    ) -> Value {
        use regex_automata::meta::Cache;
        let layouts = catalog.native_cache_layouts().unwrap();
        assert_eq!(layouts.schema_version, 2);
        assert_eq!(layouts.word_bytes, size_of::<usize>());
        assert_eq!(layouts.cache_bytes, size_of::<Cache>());
        assert_eq!(layouts.cache_alignment, align_of::<Cache>());
        assert_eq!(layouts.rows.len(), cohort.schemas.len());
        let selected_schemas = cohort.manifest["selection"]["schemas"].as_array().unwrap();
        assert_eq!(selected_schemas.len(), cohort.schemas.len());
        let observed = catalog.native_allocations("explicit-caches").unwrap();
        let observations = observed["schemas"].as_array().unwrap();
        assert_eq!(observations.len(), cohort.schemas.len());
        let corpus = super::schema_reference::captured().unwrap();
        let authored =
            super::schema_build::workload_data_for(&cohort.root, &cohort.schemas).unwrap();
        let mut rows = Vec::new();
        let mut maximum_bundle = 0_usize;
        let mut maximum_observed = 0_usize;
        let mut helpers = 0_usize;
        let mut hostname_owners = 0_usize;
        let mut captured_cases = 0_usize;
        let mut authored_cases = 0_usize;
        let mut validation_calls = 0_usize;
        for (((schema, binding), layout), observation) in cohort
            .schemas
            .iter()
            .zip(selected_schemas)
            .zip(&layouts.rows)
            .zip(observations)
        {
            let name = &schema.witness.name;
            assert_eq!(binding["witness"]["name"], *name);
            assert_eq!(&layout.name, name);
            assert_eq!(observation["name"], *name);
            let (bundle, pattern_rows) = selected_cache_bundle(schema, binding, layout, &layouts);
            helpers = helpers.checked_add(pattern_rows.len()).unwrap();
            hostname_owners = hostname_owners
                .checked_add(usize::from(bundle.format_bytes > 0))
                .unwrap();
            maximum_bundle = maximum_bundle.max(bundle.peak);
            let inputs = selected_cache_inputs(name, &corpus, authored.get(name));
            captured_cases = captured_cases.checked_add(inputs.captured).unwrap();
            authored_cases = authored_cases.checked_add(inputs.authored).unwrap();
            let (observed_peak, calls) =
                assert_cache_workloads(observation, &inputs, bundle.peak, authored.get(name));
            validation_calls = validation_calls.checked_add(calls).unwrap();
            if !pattern_rows.is_empty() {
                assert!(observed_peak > 0, "non-vacuous selected cache observation");
                assert!(
                    inputs
                        .expected
                        .iter()
                        .any(|(_, valid)| *valid == Some(true))
                );
                assert!(
                    inputs
                        .expected
                        .iter()
                        .any(|(_, valid)| *valid == Some(false))
                );
            }
            maximum_observed = maximum_observed.max(observed_peak);
            rows.push(json!({"name": name, "patterns": pattern_rows,
                "pattern_heap_bytes": bundle.pattern_bytes, "wrapper_inline_bytes": bundle.inline_bytes,
                "callback_storage_bytes": bundle.format_bytes, "storage_peak_bytes": bundle.peak,
                "captured_parseable_cases": inputs.captured, "authored_cases": inputs.authored,
                "observed_requested_heap_peak_bytes": observed_peak}));
        }
        assert_eq!(helpers, 4);
        assert_eq!(hostname_owners, 1);
        assert_eq!(captured_cases, 225);
        assert!(authored_cases > 0);
        assert_hostname_cache_observations(&observed);
        json!({"schema_version": 1_u8, "kind": "conditional_native_schema_cache_heap_candidate",
            "application_or_resource_qualification": false, "identity": native_layout_identity(),
            "layout_profile_correspondence_with_child": "not_established",
            "scope": "one fixed cache per active schema owner; serial earliest searches without reset, clone or rebinding",
            "max_input_bytes": 0x0001_0000_u64, "schemas": rows, "regex_helpers": helpers, "hostname_schemas": hostname_owners,
            "maximum_bundle_storage_bytes": maximum_bundle, "maximum_observed_requested_heap_bytes": maximum_observed,
            "captured_parseable_cases": captured_cases, "authored_cases": authored_cases, "observed_validation_calls": validation_calls,
            "probe_enum_inline_bytes": observed["owner_inline_bytes"],
            "probe_enum_storage": "outside measurement windows; not the production owner representation",
            "derivation_sources_sha256": {
                "cache": format!("{:x}", Sha256::digest(include_bytes!("schema_meta_heap/mod.rs"))),
                "bundle": format!("{:x}", Sha256::digest(include_bytes!("schema_bundle_heap/mod.rs")))},
            "observations": observed,
            "excludes": ["constructor_peak_and_work", "compiled_programs_and_other_schema_state", "complete_call_frames",
                "work_bound", "allocator_and_physical_memory", "guest_layouts", "serving_profile_qualification"]})
    }

    #[test]
    fn selected_half_search_certificates_cover_five_actual_owners_in_both_lanes() {
        use super::schema_selected_half_search::{
            HalfDirection, HalfSearchCertificate, assert_directional_replay,
            assert_source_components,
        };
        use super::schema_selection_source::{Lane, selected_cohort};
        use std::collections::BTreeMap;

        let mut refused = Vec::new();
        for lane in [Lane::Secure, Lane::Development] {
            let cohort = selected_cohort(lane).unwrap();
            check_current(&cohort.manifest["source"]).unwrap();
            let selection = &cohort.manifest["selection"];
            let owners = selected_regex_patterns(selection)
                .into_iter()
                .map(|(owner, pattern)| {
                    let bytes = if owner == "shared-hostname" {
                        255
                    } else {
                        0x0001_0000_usize
                    };
                    (owner, (pattern.to_owned(), bytes))
                })
                .collect::<BTreeMap<_, _>>();
            assert_eq!(owners.len(), 5);
            let mut forward = 0_usize;
            let mut reverse = 0_usize;
            for (owner, (pattern, bytes)) in owners {
                let inputs: Vec<String> =
                    ["", "123", "123.45", "a.co", "a.example", "!", "\u{00e9}"]
                        .into_iter()
                        .map(str::to_owned)
                        .chain(["a".repeat(bytes)])
                        .collect();
                let expected = if pattern == r"\.[a-zA-Z]{2,3}$" {
                    reverse += 1;
                    HalfDirection::ReverseAnchored
                } else {
                    forward += 1;
                    HalfDirection::CoreForward
                };
                // The shared callback actually constructs/drops per call. The
                // generated owners instead demonstrate a conditional, finite
                // changed-input reuse budget; no production call cap is inferred.
                let histories: Vec<&[String]> = if owner == "shared-hostname" {
                    inputs.chunks(1).collect()
                } else {
                    vec![inputs.as_slice()]
                };
                let calls = if owner == "shared-hostname" {
                    1
                } else {
                    inputs.len()
                };
                match HalfSearchCertificate::from_pattern(&pattern, bytes, Some(calls)) {
                    Ok(certificate) => {
                        assert_eq!(certificate.direction, expected, "{lane:?} {owner}");
                        assert_source_components(&pattern, bytes, calls, &certificate);
                        for history in histories {
                            assert_directional_replay(&pattern, history, &certificate);
                        }
                    }
                    Err(reason) => refused.push(format!("{lane:?} {owner}: {reason:?}")),
                }
            }
            assert_eq!((forward, reverse), (4, 1));
        }
        assert!(
            refused.is_empty(),
            "missing half-search certificates: {refused:?}"
        );
    }

    fn selected_cache_bundle(
        schema: &super::schema_qualification::SchemaInventory,
        binding: &Value,
        layout: &super::schema_build::CacheLayout,
        layouts: &super::schema_build::CacheLayouts,
    ) -> (super::schema_bundle_heap::CacheBundleHeap, Vec<Value>) {
        use super::schema_bundle_heap::{CacheBundleHeap, format_peak};
        use super::schema_meta_heap::MetaCacheHeap;
        let patterns: Vec<(String, String)> =
            serde_json::from_value(binding["generated_helpers"].clone()).unwrap();
        assert_eq!(
            layout.inline_bytes,
            patterns.len().checked_mul(layouts.cache_bytes).unwrap()
        );
        assert_eq!(
            layout.alignment,
            if patterns.is_empty() {
                1
            } else {
                layouts.cache_alignment
            }
        );
        let mut rows = Vec::new();
        let mut peaks = Vec::new();
        for (helper, pattern) in patterns {
            let heap = MetaCacheHeap::from_pattern(&pattern, 0x0001_0000)
                .expect("selected serial native cache owner");
            peaks.push(heap.peak);
            rows.push(json!({"helper": helper,
                "pattern_sha256": format!("{:x}", Sha256::digest(pattern.as_bytes())),
                "eager_bytes": heap.eager_bytes, "forward_bytes": heap.forward_bytes,
                "reverse_bytes": heap.reverse_bytes, "optional_prefix_peak_bytes": heap.prefix_peak_bytes,
                "fallback_bytes": heap.fallback_bytes, "growth_overlap_bytes": heap.growth_overlap,
                "requested_heap_peak_bytes": heap.peak}));
        }
        let callback = format_peak(schema.audit.formats.iter().map(|row| row.value.as_str()))
            .expect("selected format cache owner");
        (
            CacheBundleHeap::compose(&peaks, layout.inline_bytes, callback).unwrap(),
            rows,
        )
    }

    struct CacheInputs {
        expected: Vec<(String, Option<bool>)>,
        input_bytes: Vec<Option<usize>>,
        captured: usize,
        authored: usize,
    }

    fn selected_cache_inputs(
        name: &str,
        corpus: &super::schema_reference::Corpus,
        authored: Option<&Value>,
    ) -> CacheInputs {
        let mut inputs = CacheInputs {
            expected: vec![("empty".to_owned(), None)],
            input_bytes: vec![Some(2_usize)],
            captured: 0,
            authored: 0,
        };
        for case in corpus.cases.iter().filter(|case| case.schema == name) {
            let Ok(value) = serde_json::from_str::<Value>(&case.input) else {
                assert!(!case.valid);
                continue;
            };
            inputs.expected.push((
                format!("captured_{:?}_{}", case.group, case.index),
                Some(case.valid),
            ));
            inputs
                .input_bytes
                .push(Some(serde_json::to_vec(&value).unwrap().len()));
            inputs.captured = inputs.captured.checked_add(1).unwrap();
        }
        if let Some(data) = authored {
            for row in data["rows"].as_array().unwrap() {
                inputs.expected.push((
                    format!("authored_{}", row["id"].as_str().unwrap()),
                    Some(row["valid"].as_bool().unwrap()),
                ));
                // Transforms stay owned by the existing child fixture runtime.
                inputs.input_bytes.push(None);
                inputs.authored = inputs.authored.checked_add(1).unwrap();
            }
        }
        inputs
    }

    fn assert_cache_workloads(
        observation: &Value,
        inputs: &CacheInputs,
        bound: usize,
        authored: Option<&Value>,
    ) -> (usize, usize) {
        let populations = observation["populations"].as_array().unwrap();
        assert_eq!(populations.len(), 3);
        let mut observed_peak = 0_usize;
        let mut calls = 0_usize;
        for (population, count) in populations.iter().zip([1_usize, 2, 4]) {
            observed_peak = observed_peak.max(assert_cache_population(
                population,
                count,
                bound,
                &inputs.expected,
            ));
            let searches = population["searches"].as_array().unwrap();
            for (index, (search, expected_bytes)) in
                searches.iter().zip(&inputs.input_bytes).enumerate()
            {
                if let Some(bytes) = expected_bytes {
                    assert_eq!(search["input_bytes"], *bytes);
                }
                assert_eq!(
                    search["phase"],
                    if index == 0 {
                        "first-search"
                    } else {
                        "changed-input"
                    }
                );
                assert!(search["input_bytes"].as_u64().unwrap() <= 0x0001_0000);
                assert!(search["input_nodes"].as_u64().unwrap() <= 4096);
                assert!(search["input_depth"].as_u64().unwrap() <= 32);
            }
            if let Some(data) = authored {
                for (declaration, field, maximum) in [
                    ("maximum_string", "input_bytes", 0x0001_0000_u64),
                    ("maximum_collections", "input_nodes", 4096_u64),
                ] {
                    if data[declaration].as_bool() == Some(true) {
                        assert_eq!(
                            searches
                                .iter()
                                .map(|row| row[field].as_u64().unwrap())
                                .max(),
                            Some(maximum)
                        );
                    }
                }
            }
            calls = calls
                .checked_add(
                    inputs
                        .expected
                        .len()
                        .checked_mul(count)
                        .unwrap()
                        .checked_mul(2)
                        .unwrap(),
                )
                .unwrap();
        }
        (observed_peak, calls)
    }

    fn assert_hostname_cache_observations(observed: &Value) {
        let callback = super::schema_bundle_heap::format_peak(["hostname"]).unwrap();
        let hostname = observed["hostname"]["searches"].as_array().unwrap();
        assert_eq!(hostname.len(), 5);
        for row in hostname {
            let valid = row["expected"].as_bool().unwrap();
            for phase in ["cold", "warm"] {
                assert_eq!(row[phase]["valid"], valid);
                let (retained, peak) = assert_cache_phase(&row[phase]["metrics"], 0, callback);
                assert_eq!(retained, 0);
                if row["input_bytes"].as_u64().unwrap() <= 255 {
                    assert!(peak > 0, "callback cache is exercised");
                } else {
                    assert_eq!(
                        row[phase]["metrics"],
                        json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
                        "peak_live_bytes": 0_i64, "retained_bytes": 0_i64})
                    );
                }
            }
        }
    }

    fn assert_selected_onepass_attempt(owner: &str, hir: &Hir, nfa: &NFA) -> bool {
        use super::schema_onepass_bounds::candidate_onepass;
        use super::schema_onepass_construction::OnePassConstructionCertificate;
        // wrappers.rs::OnePassEngine::new skips construction without explicit
        // captures or Unicode word boundaries; anchor presence alone is not enough.
        assert!(Regex::config().get_onepass());
        assert_eq!(Regex::config().get_onepass_size_limit(), Some(0x0010_0000));
        assert!(!hir.properties().look_set().contains_word_unicode());
        if hir.properties().explicit_captures_len() == 0 {
            assert_eq!(nfa.group_info().explicit_slot_len(), 0);
            return false;
        }
        let construction = OnePassConstructionCertificate::from_nfa(nfa)
            .unwrap_or_else(|error| panic!("{owner}: {error:?}"));
        assert!(construction.peak_requested_bytes > 0);
        assert!(construction.work_upper_bound > 0);
        assert_eq!(construction.nfa_states, nfa.states().len());
        match candidate_onepass(nfa) {
            Ok(compiled) => {
                assert!(compiled.memory_usage() <= construction.retained_tables_upper_bytes);
            }
            Err(_error) => assert_eq!(
                construction.failure_prefix_peak_bytes,
                construction.peak_requested_bytes
            ),
        }
        true
    }

    #[test]
    fn selected_nfa_construction_certificates_cover_five_emitted_owners_in_both_lanes() {
        use super::schema_cache_bounds::{forward_nfa, reverse_nfa};
        use super::schema_nfa_construction::{
            NfaConstructionCertificate, source_trie_state_layout,
        };
        use super::schema_nfa_heap::NfaRetainedHeap;
        use super::schema_selection_source::{Lane, selected_cohort};
        use regex_automata::util::syntax::{Config, parse_with};

        for lane in [Lane::Secure, Lane::Development] {
            let cohort = selected_cohort(lane).unwrap();
            check_current(&cohort.manifest["source"]).unwrap();
            let selection = &cohort.manifest["selection"];
            let owners = selected_regex_patterns(selection);
            assert_eq!(owners.len(), 5);
            let mut onepass_attempts = 0_usize;
            let mut onepass_skips = 0_usize;
            for (owner, pattern) in owners {
                let hir = parse_with(pattern, &Config::default().utf8(true)).unwrap();
                for reverse in [false, true] {
                    let certificate = NfaConstructionCertificate::from_hir(
                        &hir,
                        reverse,
                        source_trie_state_layout().unwrap(),
                    )
                    .unwrap_or_else(|error| panic!("{owner}, reverse={reverse}: {error:?}"));
                    let nfa = if reverse {
                        reverse_nfa(pattern)
                    } else {
                        forward_nfa(pattern)
                    }
                    .unwrap();
                    assert_eq!(
                        certificate
                            .counts
                            .states
                            .checked_sub(certificate.counts.empties),
                        Some(nfa.states().len()),
                        "{owner}, reverse={reverse}"
                    );
                    assert_eq!(
                        certificate.final_nfa_bytes,
                        NfaRetainedHeap::from_nfa(&nfa)
                            .unwrap()
                            .requested_heap_bytes
                    );
                    assert!(certificate.intermediate_peak_bytes > 0);
                    assert!(certificate.conversion_scratch_bytes > 0);
                    assert_eq!(
                        certificate.initialization_scratch_bytes,
                        certificate
                            .intermediate_peak_bytes
                            .checked_add(certificate.conversion_scratch_bytes)
                            .unwrap()
                    );
                    assert!(certificate.work_upper_bound > 0);
                    assert!(
                        certificate.peak_requested_bytes >= certificate.failure_prefix_peak_bytes
                    );
                    if !reverse {
                        if assert_selected_onepass_attempt(&owner, &hir, &nfa) {
                            onepass_attempts += 1;
                        } else {
                            onepass_skips += 1;
                        }
                    }
                }
            }
            assert_eq!((onepass_attempts, onepass_skips), (2, 3));
        }
    }

    fn assert_selected_program_heap(
        binding: &Value,
        catalog: &super::schema_build::Catalog,
    ) -> Value {
        use super::schema_literal_heap::AutomaticPrefilterHeap;
        use super::schema_program_heap::NativeProgramHeap;
        use regex_automata::MatchKind;
        let selection = &binding["selection"];
        let mut inputs = Vec::new();
        for schema in selection["schemas"].as_array().unwrap() {
            let owner = schema["witness"]["name"].as_str().unwrap();
            let pairs: Vec<(String, String)> =
                serde_json::from_value(schema["generated_helpers"].clone()).unwrap();
            inputs.extend(
                pairs
                    .into_iter()
                    .map(|(name, pattern)| (Some(owner.to_owned()), name, pattern)),
            );
        }
        let shared: Vec<(String, String)> =
            serde_json::from_value(selection["shared_regex_helpers"].clone()).unwrap();
        inputs.extend(
            shared
                .into_iter()
                .map(|(name, pattern)| (None, name, pattern)),
        );
        let config = Regex::config()
            .match_kind(MatchKind::LeftmostFirst)
            .utf8_empty(true)
            .nfa_size_limit(Some(0x0001_0000))
            .hybrid_cache_capacity(0x0001_0000)
            .pool_capacity(1);
        let mut owners = BTreeSet::new();
        let mut groups = BTreeMap::<Option<String>, usize>::new();
        let mut rows = Vec::new();
        let mut inline = 0_usize;
        let mut scalar = 0_usize;
        for (owner, name, pattern) in &inputs {
            assert!(
                owners.insert((owner, name)),
                "distinct source helper owners"
            );
            let heap = NativeProgramHeap::from_pattern(pattern, &config)
                .expect("selected native program owner");
            let group = groups.entry(owner.clone()).or_default();
            *group = group.checked_add(heap.total).unwrap();
            inline = inline.checked_add(heap.static_inline_bytes).unwrap();
            let disposition = match heap.automatic_prefilter {
                AutomaticPrefilterHeap::ExcludedStartAnchored => {
                    json!({"kind": "excluded_start_anchored"})
                }
                AutomaticPrefilterHeap::SingleByte {
                    byte,
                    requested_bytes,
                } => {
                    scalar = scalar.checked_add(1).unwrap();
                    json!({"kind": "single_byte_memchr", "byte": byte, "requested_bytes": requested_bytes})
                }
            };
            rows.push(json!({"owner": owner, "helper": name,
                "pattern_sha256": format!("{:x}", Sha256::digest(pattern.as_bytes())),
                "metadata_bytes": heap.metadata_bytes, "strategy_arc_bytes": heap.strategy_arc_bytes,
                "nfa_bytes": heap.nfa_bytes, "onepass_tables_bytes": heap.onepass_tables_bytes,
                "automatic_prefilter": disposition, "automatic_prefilter_bytes": heap.automatic_prefilter_bytes,
                "unused_pool_bytes": heap.unused_pool_bytes, "requested_heap_bytes": heap.total,
                "static_inline_bytes": heap.static_inline_bytes}));
        }
        assert_eq!(rows.len(), 5);
        assert_eq!(scalar, 1);
        let total = groups
            .values()
            .try_fold(0_usize, |sum, next| sum.checked_add(*next))
            .unwrap();
        let observed = catalog
            .native_allocations("constructors")
            .expect("phase-isolated program observation");
        assert_program_constructor_observations(&observed, &groups, catalog);
        json!({"schema_version": 1_u8, "kind": "conditional_native_regex_retained_heap_candidate",
            "application_or_resource_qualification": false, "identity": native_layout_identity(),
            "scope": "regex helpers and shared hostname only; not all schema state",
            "layout_profile_correspondence_with_child": "not_established",
            "owners": rows, "requested_heap_bytes": total, "static_inline_bytes": inline,
            "derivation_sources_sha256": {
                "program": format!("{:x}", Sha256::digest(include_bytes!("schema_program_heap/mod.rs"))),
                "literal": format!("{:x}", Sha256::digest(include_bytes!("schema_literal_heap/mod.rs"))),
                "metadata": format!("{:x}", Sha256::digest(include_bytes!("schema_metadata_heap/mod.rs"))),
                "nfa": format!("{:x}", Sha256::digest(include_bytes!("schema_nfa_heap/mod.rs"))),
                "onepass": format!("{:x}", Sha256::digest(include_bytes!("schema_onepass_bounds/mod.rs"))),
                "pool": format!("{:x}", Sha256::digest(include_bytes!("schema_pool_heap/mod.rs"))),
                "arc": format!("{:x}", Sha256::digest(include_bytes!("schema_heap_layout/mod.rs")))},
            "constructor_observations": observed,
            "excludes": ["non_regex_schema_state", "compiled_code_and_data_residency", "constructor_peak_and_work", "search_cache_and_work",
                "allocator_and_physical_memory", "guest_layouts", "serving_profile_qualification"]})
    }

    fn assert_program_constructor_observations(
        observed: &Value,
        groups: &BTreeMap<Option<String>, usize>,
        catalog: &super::schema_build::Catalog,
    ) {
        let rows = observed["schemas"].as_array().unwrap();
        let names = &catalog
            .reports()
            .iter()
            .find(|row| row.target == "native")
            .unwrap()
            .names;
        assert_eq!(rows.len(), names.len());
        let mut retained = 0_usize;
        for (row, name) in rows.iter().zip(names) {
            assert_eq!(row["name"], *name);
            let bound = groups.get(&Some(name.clone())).copied().unwrap_or(0);
            retained = retained
                .checked_add(assert_program_observation_row(row, bound))
                .unwrap();
        }
        let format =
            assert_program_observation_row(&observed["formats"], *groups.get(&None).unwrap());
        retained = retained.checked_add(format).unwrap();
        assert_eq!(observed["first_pass"]["retained_bytes"], retained);
        assert_eq!(
            observed["repeat"],
            json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
            "peak_live_bytes": 0_i64, "retained_bytes": 0_i64})
        );
    }

    fn assert_program_observation_row(row: &Value, bound: usize) -> usize {
        assert_eq!(row["first"]["initialized"], true);
        assert_eq!(row["repeat"]["initialized"], true);
        let measured = row["first"]["metrics"]["retained_bytes"].as_i64().unwrap();
        let retained = usize::try_from(measured).unwrap();
        assert_eq!(
            retained == 0,
            bound == 0,
            "non-vacuous program construction"
        );
        assert!(
            retained <= bound,
            "observed {retained} exceeds conditional candidate {bound}"
        );
        if retained > 0 {
            assert!(
                row["first"]["metrics"]["allocation_calls"]
                    .as_u64()
                    .unwrap()
                    > 0
            );
            assert!(
                row["first"]["metrics"]["allocated_bytes"].as_u64().unwrap()
                    >= u64::try_from(retained).unwrap()
            );
        }
        assert_eq!(
            row["repeat"]["metrics"],
            json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
            "peak_live_bytes": 0_i64, "retained_bytes": 0_i64})
        );
        retained
    }

    #[test]
    fn fake_program_observation_guard_has_independent_tight_and_zero_controls() {
        use std::panic::catch_unwind;
        let row = json!({"first": {"initialized": true, "metrics": {"retained_bytes": 24_i64,
            "allocated_bytes": 24_u64, "allocation_calls": 1_u64}},
            "repeat": {"initialized": true, "metrics": {"allocated_bytes": 0_u64,
                "allocation_calls": 0_u64, "peak_live_bytes": 0_i64, "retained_bytes": 0_i64}}});
        assert_eq!(assert_program_observation_row(&row, 24), 24);
        assert_eq!(assert_program_observation_row(&row, 25), 24);
        let mut zero = row.clone();
        zero["first"]["metrics"]["retained_bytes"] = json!(0_i64);
        assert_eq!(assert_program_observation_row(&zero, 0), 0);
        let mut cases = vec![(row.clone(), 23_usize), (row.clone(), 0), (zero, 24)];
        for (pointer, value) in [
            ("/first/initialized", json!(false)),
            ("/repeat/initialized", json!(false)),
            ("/first/metrics/retained_bytes", json!(-1_i64)),
            ("/first/metrics/allocation_calls", json!(0_u64)),
            ("/first/metrics/allocated_bytes", json!(23_u64)),
            ("/repeat/metrics/allocated_bytes", json!(1_u64)),
            ("/repeat/metrics/allocation_calls", json!(1_u64)),
            ("/repeat/metrics/peak_live_bytes", json!(1_i64)),
            ("/repeat/metrics/retained_bytes", json!(1_i64)),
        ] {
            let mut changed = row.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            cases.push((changed, 24));
        }
        for (changed, bound) in cases {
            catch_unwind(|| assert_program_observation_row(&changed, bound)).unwrap_err();
        }
    }

    fn assert_cache_phase(metrics: &Value, before: usize, bound: usize) -> (usize, usize) {
        let allocated = metrics["allocated_bytes"].as_u64().unwrap();
        let calls = metrics["allocation_calls"].as_u64().unwrap();
        let incremental_peak =
            usize::try_from(metrics["peak_live_bytes"].as_i64().unwrap()).unwrap();
        let delta = metrics["retained_bytes"].as_i64().unwrap();
        assert_eq!(allocated == 0, calls == 0);
        assert!(allocated >= u64::try_from(incremental_peak).unwrap());
        assert!(i64::try_from(incremental_peak).unwrap() >= delta);
        let retained =
            usize::try_from(i64::try_from(before).unwrap().checked_add(delta).unwrap()).unwrap();
        let peak = before.checked_add(incremental_peak).unwrap();
        assert!(retained <= bound, "retained {retained} exceeds {bound}");
        assert!(peak <= bound, "phase peak {peak} exceeds {bound}");
        (retained, peak)
    }

    #[test]
    fn fake_cache_phase_accounts_for_prior_storage_and_transient_growth_independently() {
        use std::panic::catch_unwind;
        let growth = json!({"allocated_bytes": 23_u64, "allocation_calls": 1_u64,
            "peak_live_bytes": 23_i64, "retained_bytes": 7_i64});
        assert_eq!(assert_cache_phase(&growth, 11, 34), (18, 34));
        assert_eq!(assert_cache_phase(&growth, 0, 23), (7, 23));
        assert_eq!(assert_cache_phase(&growth, 11, 35), (18, 34));
        catch_unwind(|| assert_cache_phase(&growth, 11, 33)).unwrap_err();
        let release = json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
            "peak_live_bytes": 0_i64, "retained_bytes": -18_i64});
        assert_eq!(assert_cache_phase(&release, 18, 18), (0, 18));
        catch_unwind(|| assert_cache_phase(&release, 17, 18)).unwrap_err();
        let zero = json!({"allocated_bytes": 0_u64, "allocation_calls": 0_u64,
            "peak_live_bytes": 0_i64, "retained_bytes": 0_i64});
        assert_eq!(assert_cache_phase(&zero, 0, 0), (0, 0));
        let mut cases = Vec::new();
        for (field, changed) in [
            ("allocated_bytes", json!(22_u64)),
            ("allocation_calls", json!(0_u64)),
            ("peak_live_bytes", json!(-1_i64)),
            ("retained_bytes", json!(24_i64)),
        ] {
            let mut changed_metrics = growth.clone();
            changed_metrics[field] = changed;
            cases.push(changed_metrics);
        }
        for changed in cases {
            catch_unwind(|| assert_cache_phase(&changed, 11, 34)).unwrap_err();
        }
        catch_unwind(|| assert_cache_phase(&growth, usize::MAX, usize::MAX)).unwrap_err();
    }

    fn assert_cache_population(
        row: &Value,
        owners: usize,
        bound: usize,
        expected: &[(String, Option<bool>)],
    ) -> usize {
        assert!(owners > 0 && !expected.is_empty());
        assert_eq!(row["owners"], owners);
        let construction = row["construction"].as_array().unwrap();
        assert_eq!(construction.len(), owners);
        let mut retained = Vec::with_capacity(owners);
        let mut maximum = 0_usize;
        for metrics in construction {
            let (live, peak) = assert_cache_phase(metrics, 0, bound);
            retained.push(live);
            maximum = maximum.max(peak);
        }
        let searches = row["searches"].as_array().unwrap();
        assert_eq!(searches.len(), expected.len());
        for (search, (id, verdict)) in searches.iter().zip(expected) {
            assert_eq!(search["id"], *id);
            assert_eq!(search["expected"], json!(verdict));
            for phase in ["cold", "warm"] {
                let results = search[phase].as_array().unwrap();
                assert_eq!(results.len(), owners);
                for (result, before) in results.iter().zip(&mut retained) {
                    let actual = result["valid"].as_bool().unwrap();
                    if let Some(required) = verdict {
                        assert_eq!(actual, *required, "{id} {phase}");
                    }
                    let (live, peak) = assert_cache_phase(&result["metrics"], *before, bound);
                    *before = live;
                    maximum = maximum.max(peak);
                }
            }
        }
        let release = row["release"].as_array().unwrap();
        assert_eq!(release.len(), owners);
        // The probe pops owners in reverse construction order.
        for (metrics, before) in release.iter().zip(retained.into_iter().rev()) {
            assert_eq!(metrics["allocated_bytes"], 0_u64);
            assert_eq!(metrics["allocation_calls"], 0_u64);
            let (live, peak) = assert_cache_phase(metrics, before, bound);
            assert_eq!(live, 0);
            maximum = maximum.max(peak);
        }
        maximum
    }

    #[test]
    fn fake_cache_population_covers_every_owner_input_verdict_and_release() {
        use std::panic::catch_unwind;
        let metrics = |allocated: u64, peak: i64, retained: i64| {
            json!({"allocated_bytes": allocated, "allocation_calls": u64::from(allocated > 0),
                "peak_live_bytes": peak, "retained_bytes": retained})
        };
        let zero = metrics(0, 0, 0);
        let search = |valid, metric: &Value| json!({"valid": valid, "metrics": metric});
        let row = json!({"owners": 2_u64,
            "construction": [metrics(5, 5, 5), metrics(7, 7, 7)],
            "searches": [
                {"id": "empty", "expected": null,
                    "cold": [search(false, &metrics(7, 7, 3)), search(false, &metrics(2, 2, 2))],
                    "warm": [search(false, &zero), search(false, &zero)]},
                {"id": "fake_case", "expected": true,
                    "cold": [search(true, &metrics(7, 7, 0)), search(true, &metrics(6, 6, 0))],
                    "warm": [search(true, &zero), search(true, &zero)]}],
            "release": [metrics(0, 0, -9), metrics(0, 0, -8)]});
        let expected = [
            ("empty".to_owned(), None),
            ("fake_case".to_owned(), Some(true)),
        ];
        assert_eq!(assert_cache_population(&row, 2, 15, &expected), 15);
        catch_unwind(|| assert_cache_population(&row, 2, 14, &expected)).unwrap_err();
        let mut mutations = Vec::new();
        for (pointer, value) in [
            ("/owners", json!(1_u64)),
            ("/construction", json!([metrics(5, 5, 5)])),
            ("/searches/0/id", json!("fake_skipped")),
            ("/searches/1/expected", json!(false)),
            ("/searches/1/cold/0/valid", json!(false)),
            ("/searches/1/warm/1/valid", json!(false)),
            ("/searches/0/warm", json!([])),
            ("/release/0/retained_bytes", json!(-8_i64)),
            ("/release/0/allocated_bytes", json!(1_u64)),
            ("/release", json!([])),
        ] {
            let mut changed = row.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            mutations.push(changed);
        }
        let mut missing = row.clone();
        missing["searches"].as_array_mut().unwrap().pop();
        mutations.push(missing);
        let mut reordered = row.clone();
        reordered["release"].as_array_mut().unwrap().reverse();
        mutations.push(reordered);
        for changed in mutations {
            catch_unwind(|| assert_cache_population(&changed, 2, 15, &expected)).unwrap_err();
        }
        catch_unwind(|| assert_cache_population(&row, 0, 15, &expected)).unwrap_err();
        catch_unwind(|| assert_cache_population(&row, 2, 15, &[])).unwrap_err();
    }

    fn assert_selected_semantic_cases(
        batch: &super::schema_selected_semantics::Batch,
        raw: impl Fn(&str, &[&str]) -> Result<Vec<super::schema_build::RawVerdict>, String>,
        sequence: impl Fn(&str, &[Value]) -> Result<Vec<bool>, String>,
    ) -> Value {
        use super::schema_build::RawVerdict;
        let mut groups = BTreeMap::<&str, Vec<_>>::new();
        for row in &batch.rows {
            groups.entry(&row.schema).or_default().push(row);
        }
        assert!(!groups.is_empty(), "selected semantic cases must execute");
        assert_eq!(
            groups.keys().copied().collect::<Vec<_>>(),
            batch
                .schema_sha256
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        let mut checked = 0_usize;
        let mut accepted = 0_usize;
        let mut counts = BTreeMap::new();
        for (name, cases) in groups {
            let inputs = cases
                .iter()
                .map(|row| row.input_json.as_str())
                .collect::<Vec<_>>();
            let actual = raw(name, &inputs).expect("selected raw semantic dispatch");
            assert_eq!(actual.len(), cases.len(), "{name}");
            let mut values = Vec::new();
            let mut expected_sequence = Vec::new();
            let mut accepts = 0_usize;
            for (case, result) in cases.iter().zip(actual) {
                let expected = if let Ok(value) = serde_json::from_str::<Value>(&case.input_json) {
                    values.push(value);
                    expected_sequence.push(case.valid);
                    RawVerdict::Schema { valid: case.valid }
                } else {
                    assert!(!case.valid, "{}: accepted case must be JSON", case.id);
                    RawVerdict::InvalidJson
                };
                assert_eq!(result, expected, "{}", case.id);
                accepts = accepts.checked_add(usize::from(case.valid)).unwrap();
                accepted = accepted.checked_add(usize::from(case.valid)).unwrap();
                checked = checked.checked_add(1).unwrap();
            }
            assert_eq!(
                sequence(name, &values).expect("selected reused-cache semantic dispatch"),
                expected_sequence,
                "{name} reused-cache semantic cases"
            );
            counts.insert(
                name,
                json!({"rows":cases.len(),"accepts":accepts,"rejects":cases.len().checked_sub(accepts).unwrap()}),
            );
        }
        assert_eq!(checked, batch.rows.len());
        assert_eq!(
            checked,
            batch
                .counts
                .authored
                .checked_add(batch.counts.imported)
                .unwrap()
        );
        assert_eq!(accepted, batch.counts.accepts);
        assert_eq!(checked.checked_sub(accepted).unwrap(), batch.counts.rejects);
        json!({"kind":"selected_schema_semantic_execution","lane":batch.lane,
            "counts":batch.counts,"by_schema":counts,"checked":checked,
            "expectations":"owner-authored schema-derived rows and separately attributed pinned captures",
            "scope":"behavioral probes only; not resource or serving qualification"})
    }

    #[test]
    fn fake_selected_semantic_driver_requires_all_rows_both_dispatch_modes_and_exact_counts() {
        use super::schema_build::RawVerdict;
        use super::schema_selected_semantics::{Batch, Counts, Origin, Row};
        use std::panic::catch_unwind;
        let mut batch = Batch {
            lane: "fake_lane".to_owned(),
            rows: [
                ("fake_a", "object", json!({}).to_string(), true),
                ("fake_a", "unfinished", "{".to_owned(), false),
                ("fake_b", "scalar", json!(0_u64).to_string(), false),
            ]
            .into_iter()
            .map(|(schema, id, input_json, valid)| Row {
                id: format!("{schema}/{id}"),
                schema: schema.to_owned(),
                input_json,
                valid,
                origin: Origin::AuthoredSchemaDerived,
            })
            .collect(),
            counts: Counts {
                authored: 3,
                imported: 0,
                accepts: 1,
                rejects: 2,
            },
            schema_sha256: ["fake_a", "fake_b"]
                .into_iter()
                .map(|name| (name.to_owned(), "fake_digest".to_owned()))
                .collect(),
            data_sha256: BTreeMap::new(),
            imported_sha256: BTreeMap::new(),
        };
        let raw = |name: &str, inputs: &[&str]| match name {
            "fake_a" => {
                assert_eq!(inputs, ["{}", "{"]);
                Ok(vec![
                    RawVerdict::Schema { valid: true },
                    RawVerdict::InvalidJson,
                ])
            }
            "fake_b" => {
                assert_eq!(inputs, ["0"]);
                Ok(vec![RawVerdict::Schema { valid: false }])
            }
            _ => panic!("unselected semantic owner"),
        };
        let sequence = |name: &str, values: &[Value]| match name {
            "fake_a" => {
                assert_eq!(values, [json!({})]);
                Ok(vec![true])
            }
            "fake_b" => {
                assert_eq!(values, [json!(0_u64)]);
                Ok(vec![false])
            }
            _ => panic!("unselected semantic owner"),
        };
        let report = assert_selected_semantic_cases(&batch, raw, sequence);
        assert_eq!(report["checked"], 3_usize);
        assert_eq!(
            report["by_schema"]["fake_a"],
            json!({"rows":2_u64,"accepts":1_u64,"rejects":1_u64})
        );
        for mode in ["missing", "extra", "verdict", "raw_class"] {
            catch_unwind(|| {
                assert_selected_semantic_cases(
                    &batch,
                    |name, inputs| {
                        let mut results = raw(name, inputs)?;
                        match mode {
                            "missing" => {
                                results.pop();
                            }
                            "extra" => results.push(RawVerdict::InvalidJson),
                            "verdict" => results[0] = RawVerdict::Schema { valid: false },
                            "raw_class" => results[1] = RawVerdict::Schema { valid: false },
                            _ => panic!("unknown semantic mutation"),
                        }
                        Ok(results)
                    },
                    sequence,
                )
            })
            .expect_err("raw count, schema verdict and lexical class must be checked");
        }
        catch_unwind(|| assert_selected_semantic_cases(&batch, raw, |_, _| Ok(vec![false])))
            .expect_err("reused-cache verdict must be checked independently");
        batch.counts.accepts = 0;
        catch_unwind(|| assert_selected_semantic_cases(&batch, raw, sequence))
            .expect_err("reported counts must describe executed rows");
        batch.counts.accepts = 1;
        batch.rows.retain(|row| row.schema != "fake_b");
        catch_unwind(|| assert_selected_semantic_cases(&batch, raw, sequence))
            .expect_err("a selected owner cannot silently lose all rows");
    }

    fn assert_bare_semantic_cases(
        runtime: &super::schema_build::guest::Runtime,
        catalog: &super::schema_build::Catalog,
        names: &[&str],
        batch: &super::schema_selected_semantics::Batch,
    ) -> Value {
        use super::schema_build::RawVerdict;
        let originals = |name: &str| {
            batch
                .rows
                .iter()
                .filter(|row| row.schema == name)
                .map(|row| row.input_json.as_str())
                .collect::<Vec<_>>()
        };
        assert_selected_semantic_cases(
            batch,
            |name, inputs| {
                if inputs != originals(name) {
                    return Err("bare semantics requires its bound raw input table".to_owned());
                }
                bare_rows(
                    runtime,
                    catalog,
                    names,
                    name,
                    inputs.len(),
                    false,
                    BareInputs::SelectedSemantics,
                )
            },
            |name, inputs| {
                let raw = originals(name);
                let values = raw
                    .iter()
                    .filter_map(|input| serde_json::from_str::<Value>(input).ok())
                    .collect::<Vec<_>>();
                if inputs != values {
                    return Err("bare semantics requires its bound parsed input table".to_owned());
                }
                let results = bare_rows(
                    runtime,
                    catalog,
                    names,
                    name,
                    raw.len(),
                    true,
                    BareInputs::SelectedSemantics,
                )?;
                let mut valid = Vec::new();
                for (input, result) in raw.into_iter().zip(results) {
                    match (serde_json::from_str::<Value>(input), result) {
                        (Ok(_), RawVerdict::Schema { valid: verdict }) => valid.push(verdict),
                        (Err(_), RawVerdict::InvalidJson) => (),
                        _ => return Err("bare semantics raw verdict class mismatch".to_owned()),
                    }
                }
                Ok(valid)
            },
        )
    }

    fn assert_selected_parameter_cases(
        catalog: &super::schema_build::Catalog,
        names: &[&str],
    ) -> Value {
        assert_parameter_cases(
            names,
            |name, inputs| {
                inputs
                    .iter()
                    .map(|input| catalog.check_native_raw(name, input))
                    .collect()
            },
            |name, inputs| catalog.check_native_sequence(name, inputs),
        )
    }

    fn assert_parameter_cases(
        names: &[&str],
        raw: impl Fn(&str, &[&str]) -> Result<Vec<super::schema_build::RawVerdict>, String>,
        sequence: impl Fn(&str, &[Value]) -> Result<Vec<bool>, String>,
    ) -> Value {
        use super::schema_build::RawVerdict;
        let corpus = super::schema_reference::captured().unwrap();
        let mut counts = BTreeMap::<&str, [usize; 3]>::new();
        let mut sequences = BTreeMap::<&str, Vec<(Value, bool)>>::new();
        let mut groups = BTreeMap::<&str, Vec<_>>::new();
        for case in &corpus.cases {
            if names.contains(&case.schema.as_str()) {
                groups.entry(&case.schema).or_default().push(case);
            }
        }
        for (name, cases) in groups {
            let inputs: Vec<_> = cases.iter().map(|case| case.input.as_str()).collect();
            let results = raw(name, &inputs).unwrap();
            assert_eq!(results.len(), cases.len(), "{name}");
            for (case, actual) in cases.into_iter().zip(results) {
                let expected = if let Ok(value) = serde_json::from_str::<Value>(&case.input) {
                    sequences
                        .entry(&case.schema)
                        .or_default()
                        .push((value, case.valid));
                    RawVerdict::Schema { valid: case.valid }
                } else {
                    assert!(!case.valid, "reference acceptance requires valid JSON");
                    RawVerdict::InvalidJson
                };
                assert_eq!(
                    actual, expected,
                    "{} {:?} {}",
                    case.schema, case.group, case.index
                );
                let index = match actual {
                    RawVerdict::Schema { valid: true } => 0,
                    RawVerdict::Schema { valid: false } => 1,
                    RawVerdict::InvalidJson => 2,
                };
                let row = counts.entry(&case.schema).or_default();
                row[index] = row[index].checked_add(1).unwrap();
            }
        }
        let expected_counts: BTreeMap<_, _> = super::schema_reference::verdict_counts()
            .into_iter()
            .filter(|(name, _)| names.contains(name))
            .collect();
        assert!(
            !expected_counts.is_empty(),
            "selected captured cases must execute"
        );
        assert_eq!(counts, expected_counts);
        for (name, rows) in &sequences {
            let inputs: Vec<_> = rows.iter().map(|(value, _)| value.clone()).collect();
            let expected: Vec<_> = rows.iter().map(|(_, valid)| *valid).collect();
            assert_eq!(sequence(name, &inputs).unwrap(), expected, "{name}");
        }
        let uncovered: Vec<_> = names
            .iter()
            .filter(|name| !counts.contains_key(**name))
            .collect();
        serde_json::json!({"origin": "pinned_external_parameter_tables",
            "verdict_counts_by_schema": counts, "schemas_without_captured_parameter_cases": uncovered,
            "scope": "captured subset only; not complete schema semantics or resource qualification"})
    }

    use super::schema_formats::HOSTNAME_SOURCE;
    use super::schema_selection_source::produce as selected_source_manifest;
    fn check_current(manifest: &Value) -> Result<(), &'static str> {
        // The producer independently reconciles these generated declarations
        // and target-gated bindings against bidder-owned source manifests.
        let declarations = auctioneer_config::bidder_metadata_declarations();
        let published: BTreeSet<_> = declarations.iter().map(|row| row.bidder).collect();
        let planned = super::schema_selection_source::planned_source_roots()
            .map_err(|_error| "planned source/publication membership")?;
        let active: Vec<_> = auctioneer_config::bidder_implementation_bindings()
            .iter()
            .map(|row| (row.bidder, row.implementation))
            .collect();
        let active_count = match (
            cfg!(target_arch = "wasm32"),
            cfg!(feature = "insecure-bidders"),
        ) {
            (false, false) => 17,
            (false, true) => 18,
            (true, false) => 3,
            (true, true) => 4,
        };
        if manifest["planned_roots"]
            .as_array()
            .ok_or("planned roots")?
            .len()
            != 14
            || manifest["planned_roots"] != json!(planned)
            || manifest["published_roots_all_features"]
                .as_array()
                .ok_or("published roots")?
                .len()
                != 18
            || manifest["published_roots_all_features"] != json!(published)
            || manifest["actual_active_published_bindings"]
                .as_array()
                .ok_or("active bindings")?
                .len()
                != active_count
            || manifest["actual_active_published_bindings"] != json!(active)
        {
            return Err("root/publication membership");
        }
        for (lane_name, count, roots, members) in
            [("secure", 15_usize, 17, 17), ("development", 16, 18, 18)]
        {
            let lane = &manifest["lanes"][lane_name];
            // Inspection S remains published declarations plus planned roots,
            // even on guests where native-only serving bindings are absent.
            let mut expected_roots: BTreeSet<_> = planned.iter().map(String::as_str).collect();
            expected_roots.extend(
                declarations
                    .iter()
                    .filter(|row| row.required_feature.is_none() || lane_name == "development")
                    .map(|row| row.bidder),
            );
            if lane["roots"].as_array().ok_or("roots")?.len() != roots
                || lane["roots"] != json!(expected_roots)
                || lane["members"].as_array().ok_or("members")?.len() != members
                || lane["schema_count"] != json!(count)
                || lane["schemas"].as_array().ok_or("schemas")?.len() != count
            {
                return Err("root/schema membership");
            }
            let schemas = lane["schemas"].as_array().ok_or("schemas")?;
            let helpers: Vec<_> = schemas
                .iter()
                .flat_map(|schema| {
                    schema["generated_helpers"]
                        .as_array()
                        .expect("helper array")
                        .iter()
                        .cloned()
                })
                .collect();
            if lane["generated_helper_count"] != 4_u64
                || helpers
                    != vec![
                        json!(["__JSONSCHEMA_REGEX_0_REGEX", r"^[0-9]+(\.[0-9]+)?$"]),
                        json!(["__JSONSCHEMA_REGEX_1_REGEX", r"\.[a-zA-Z]{2,3}$"]),
                        json!(["__JSONSCHEMA_REGEX_2_REGEX", "^[0-9]+$"]),
                        json!(["__JSONSCHEMA_REGEX_0_REGEX", "^[0-9]+$"]),
                    ]
            {
                return Err("generated helper membership");
            }
            if lane["shared_formats"] != json!(["hostname", "uuid"])
                || lane["shared_regex_helpers"] != json!([["shared-hostname", HOSTNAME_SOURCE]])
            {
                return Err("shared format membership");
            }
        }
        Ok(())
    }

    #[test]
    fn schema_selection_source_and_expanded_generated_membership_manifest() {
        use std::io::{Write as _, stderr};
        let manifest =
            selected_source_manifest(false).expect("selected source/generated membership");
        assert_eq!(
            manifest["kind"],
            "selected_source_and_expanded_generated_membership"
        );
        assert_eq!(manifest["linked_or_serving_qualification"], false);
        assert_eq!(manifest["retained_schema_witness_count"], 263_u64);
        for field in [
            "workspace_manifest_sha256",
            "lock_sha256",
            "producer_source_sha256",
            "expansion_source_sha256",
        ] {
            assert_eq!(manifest[field].as_str().unwrap().len(), 64);
        }
        assert!(
            manifest["compiler_vendor"]["verified_root"]
                .as_str()
                .is_some()
        );
        assert_eq!(
            manifest["compiler_vendor"]["verifier_source_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert_eq!(
            manifest["regex_vendor"]["verified_source"]["tree_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        assert_eq!(
            manifest["regex_vendor"]["verifier_source_sha256"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
        check_current(&manifest).expect("exact observed membership");
        let secure = &manifest["lanes"]["secure"];
        let members = secure["members"].as_array().expect("members");
        let schema_free: Vec<_> = members
            .iter()
            .filter(|row| row["effective_schema_owner"].is_null())
            .collect();
        assert_eq!(schema_free.len(), 1);
        assert_eq!(schema_free[0]["published"], true);
        assert_eq!(schema_free[0]["is_root"], true);
        let aliases: Vec<_> = members
            .iter()
            .filter(|row| !row["ancestry"].as_array().unwrap().is_empty())
            .collect();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0]["is_root"], true);
        assert_eq!(
            aliases[0]["effective_schema_owner"],
            aliases[0]["ancestry"].as_array().unwrap()[0]
        );
        let expected_owners: BTreeSet<_> = members
            .iter()
            .filter(|row| row["is_root"] == true)
            .filter_map(|row| row["effective_schema_owner"].as_str())
            .collect();
        let names: BTreeSet<_> = secure["schemas"]
            .as_array()
            .expect("schemas")
            .iter()
            .map(|row| row["witness"]["name"].as_str().expect("schema identity"))
            .collect();
        assert_eq!(
            names, expected_owners,
            "factory siblings cannot expand effective owners"
        );
        writeln!(
            stderr(),
            "{}",
            serde_json::to_string(&manifest).expect("manifest JSON")
        )
        .expect("manifest output");
    }

    #[test]
    fn schema_selection_manifest_is_order_independent_and_detects_omissions() {
        let original = selected_source_manifest(false).expect("manifest");
        let reordered = selected_source_manifest(true).expect("reordered manifest");
        assert_eq!(
            serde_json::to_vec(&original).unwrap(),
            serde_json::to_vec(&reordered).unwrap()
        );
        for field in [
            "planned_roots",
            "published_roots_all_features",
            "actual_active_published_bindings",
        ] {
            let mut missing_publication = original.clone();
            missing_publication[field]
                .as_array_mut()
                .unwrap()
                .pop()
                .unwrap();
            check_current(&missing_publication).expect_err("missing publication");
            let mut replaced_publication = original.clone();
            replaced_publication[field].as_array_mut().unwrap()[0] = json!("fake_substitute");
            check_current(&replaced_publication).expect_err("same-count publication substitution");
        }
        for field in ["roots", "schemas"] {
            let mut missing = original.clone();
            missing["lanes"]["secure"][field]
                .as_array_mut()
                .unwrap()
                .pop()
                .unwrap();
            check_current(&missing).expect_err(field);
        }
        let mut missing = original.clone();
        let patterned = missing["lanes"]["secure"]["schemas"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| !row["generated_helpers"].as_array().unwrap().is_empty())
            .unwrap();
        patterned["generated_helpers"]
            .as_array_mut()
            .unwrap()
            .pop()
            .unwrap();
        check_current(&missing).expect_err("missing generated helper");
        let mut missing_shared = original;
        missing_shared["lanes"]["secure"]["shared_regex_helpers"]
            .as_array_mut()
            .unwrap()
            .pop()
            .unwrap();
        check_current(&missing_shared).expect_err("missing shared helper");
    }
    use super::bundle_patterns;
    use regex_automata::{meta::Regex, nfa::thompson::NFA, util::prefilter::Prefilter};
    use regex_syntax::hir::Hir;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::{fs, hint, path::Path, str};

    #[test]
    fn schema_selection_fake_compile_error_ast_guard() {
        let mut positive = syn::parse_quote! {
            fn fake_positive() { let _ = "compile_error!(fake text)"; }
        };
        super::reject_compiler_errors(&mut positive).expect("ordinary generated AST");
        for mut error in [
            syn::parse_quote! { compile_error!("fake unsupported schema"); },
            syn::parse_quote! { mod fake_module { fn fake_body() { compile_error!("fake nested error"); } } },
        ] {
            assert!(
                super::reject_compiler_errors(&mut error)
                    .unwrap_err()
                    .contains("compile_error")
            );
        }
    }

    #[test]
    fn fake_regex_resource_layout_public_api() {
        use regex_automata::{MatchKind, meta::Regex, util::prefilter::Prefilter};
        use std::{alloc::Layout, fmt::Debug};

        fn assert_owner_traits<T: Send + Sync + Unpin + Clone>() {}
        fn assert_layouts(_: &[Layout]) {}
        fn assert_report_traits<T: Copy + Clone + Debug + Eq + Send + Sync + Unpin + 'static>(
            report: T,
        ) {
            let copied = report;
            assert_eq!(report, copied);
        }

        assert_owner_traits::<Regex>();
        assert_owner_traits::<Prefilter>();
        let regex = Regex::new("fake_[a-z]+").unwrap();
        let report = regex.resource_layout();
        let value = report.value();
        let regex_payload = report.regex_payload();
        let metadata_payload = report.metadata_payload();
        let strategy_payload = report.strategy_payload();
        assert_layouts(&[value, regex_payload, metadata_payload, strategy_payload]);
        assert_eq!(value, Layout::new::<Regex>());
        assert_eq!(report, regex.resource_layout());
        assert_report_traits(report);
        hint::black_box((regex_payload, metadata_payload, strategy_payload));

        let prefilter = Prefilter::new(MatchKind::LeftmostFirst, &["fake_literal"]).unwrap();
        let prefilter_report = prefilter.resource_layout();
        let prefilter_value = prefilter_report.value();
        let matcher_payload = prefilter_report.matcher_payload();
        assert_layouts(&[prefilter_value, matcher_payload]);
        assert_eq!(prefilter_value, Layout::new::<Prefilter>());
        assert_eq!(prefilter_report, prefilter.resource_layout());
        assert_report_traits(prefilter_report);
        hint::black_box(matcher_payload);
    }

    fn native_layout_identity() -> super::schema_resource_layout::Identity {
        use super::schema_build::{self, schema_regex_vendor};
        use super::schema_resource_layout::Identity;

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = schema_regex_vendor::verify(&root).unwrap();
        let features = schema_build::resolved_engine_features(&source).unwrap();
        let build: toml::Value = toml::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/schema-layout-build.toml"
        )))
        .unwrap();
        let (compiler, target) = native_layout_build_identity(&build).unwrap();
        assert!(compiler.lines().any(|line| line == "release: 1.98.1"));
        Identity {
            engine_version: "0.4.18".to_owned(),
            archive_sha256: source.archive_sha256,
            patch_sha256: source.patch_sha256,
            tree_sha256: source.tree_sha256,
            target: target.to_owned(),
            compiler: compiler.to_owned(),
            features,
        }
    }

    fn native_layout_build_identity(build: &toml::Value) -> Result<(&str, &str), &'static str> {
        use std::env::consts;

        let field = |key| {
            build
                .get(key)
                .and_then(toml::Value::as_str)
                .ok_or("missing build field")
        };
        if field("arch")? != consts::ARCH
            || field("pointer_width")? != usize::BITS.to_string()
            || field("endian")?
                != if cfg!(target_endian = "little") {
                    "little"
                } else {
                    "big"
                }
        {
            return Err("build ABI mismatch");
        }
        Ok((field("compiler")?, field("target")?))
    }

    #[test]
    fn native_layout_identity_uses_build_target_and_refuses_another_binary_abi() {
        let actual: toml::Value = toml::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/schema-layout-build.toml"
        )))
        .unwrap();
        native_layout_build_identity(&actual).unwrap();
        for key in ["arch", "pointer_width", "endian"] {
            let mut changed = actual.clone();
            changed[key] = toml::Value::String("fake_other_abi".to_owned());
            assert_eq!(
                native_layout_build_identity(&changed),
                Err("build ABI mismatch")
            );
        }
        // A compiler's host is not the target that produced these layouts.
        let mut cross = actual;
        cross["compiler"] =
            toml::Value::String("fake_compiler\nhost: fake_other_host\n".to_owned());
        cross["target"] = toml::Value::String("fake_actual_build_target".to_owned());
        assert_eq!(
            native_layout_build_identity(&cross).unwrap().1,
            "fake_actual_build_target"
        );
    }

    fn assert_layout_queries_allocate_nothing(candidates: &[Regex], prefilter: &Prefilter) {
        use super::schema_meter::measure;
        use serde_json::json;

        let ((), query_metrics) = measure(|| {
            for _ in 0_usize..100 {
                for candidate in candidates {
                    hint::black_box(candidate.resource_layout());
                }
                hint::black_box(prefilter.resource_layout());
            }
        });
        assert_eq!(
            query_metrics.json(),
            json!({
                "allocated_bytes": 0_u64, "peak_live_bytes": 0_i64,
                "retained_bytes": 0_i64, "allocation_calls": 0_u64
            })
        );
        let (positive, construction_metrics) =
            measure(|| hint::black_box(Box::new(hint::black_box([17_u8; 257]))));
        assert_eq!(
            construction_metrics.json(),
            json!({
                "allocated_bytes": 257_u64, "peak_live_bytes": 257_i64,
                "retained_bytes": 257_i64, "allocation_calls": 1_u64
            })
        );
        let ((), release_metrics) = measure(|| drop(hint::black_box(positive)));
        assert_eq!(release_metrics.json()["retained_bytes"], -257_i64);
    }

    fn native_layout_bindings(
        patterns: &[(String, String)],
    ) -> Vec<super::schema_resource_layout::ExpectedOwner> {
        use super::schema_resource_layout::{ExpectedOwner, OwnerKind};
        use serde_json::json;
        let constructor = json!({
            "match_kind": "leftmost_first", "utf8_empty": true,
            "nfa_size_limit": 0x0001_0000_usize, "hybrid_cache_capacity": 0x0001_0000_usize,
            "pool_capacity": 1_usize, "syntax_utf8": true
        });
        let mut owners: Vec<_> = patterns
            .iter()
            .map(|(name, pattern)| ExpectedOwner {
                name: name.clone(),
                kind: OwnerKind::Regex,
                pattern_config_digest: format!(
                    "{:x}",
                    Sha256::digest(
                        serde_json::to_vec(
                            &json!({"pattern": pattern, "constructor": constructor})
                        )
                        .unwrap()
                    )
                ),
            })
            .collect();
        owners.push(ExpectedOwner {
            name: "fake_public_prefilter".to_owned(),
            kind: OwnerKind::PublicPrefilter,
            pattern_config_digest: format!(
                "{:x}",
                Sha256::digest(
                    serde_json::to_vec(
                        &json!({"match_kind": "leftmost_first", "needles": ["fake_layout_literal"]})
                    )
                    .unwrap()
                )
            ),
        });
        owners
    }

    #[test]
    fn native_layout_candidates_bind_the_helper_census_without_allocating_queries() {
        use super::schema_heap_layout::native_arc_value_layout;
        use super::schema_metadata_heap::NativeImmutableRegexMetadata;
        use super::schema_resource_layout::{CandidateOrigin, CandidateReport, OwnerRow};
        use regex_automata::{MatchKind, util::syntax};
        use std::io::{Write as _, stderr};

        let expected_identity = native_layout_identity();
        let (patterns, _) = source_static_patterns();
        let expected_owners = native_layout_bindings(&patterns);
        let (prefilter_binding, bindings) = expected_owners.split_last().unwrap();
        let candidates: Vec<_> = patterns
            .iter()
            .map(|(_, pattern)| {
                Regex::builder()
                    .configure(
                        Regex::config()
                            .match_kind(MatchKind::LeftmostFirst)
                            .utf8_empty(true)
                            .nfa_size_limit(Some(0x0001_0000))
                            .hybrid_cache_capacity(0x0001_0000)
                            .pool_capacity(1),
                    )
                    .syntax(syntax::Config::default().utf8(true))
                    .build(pattern)
                    .unwrap()
            })
            .collect();
        let prefilter = Prefilter::new(MatchKind::LeftmostFirst, &["fake_layout_literal"]).unwrap();

        assert_layout_queries_allocate_nothing(&candidates, &prefilter);

        let mut owners: Vec<_> = bindings
            .iter()
            .zip(&candidates)
            .map(|(binding, candidate)| {
                OwnerRow::regex(
                    &binding.name,
                    &binding.pattern_config_digest,
                    candidate.resource_layout(),
                )
            })
            .collect();
        owners.push(OwnerRow::public_prefilter(
            &prefilter_binding.name,
            &prefilter_binding.pattern_config_digest,
            prefilter.resource_layout(),
        ));
        let report = CandidateReport {
            identity: expected_identity.clone(),
            origin: CandidateOrigin::ReconstructedCandidate,
            owners,
        };
        let charges = report
            .validate(&expected_identity, &expected_owners)
            .unwrap();
        assert_eq!(charges.regex_owners, 24);
        assert_eq!(charges.public_prefilter_owners, 1);
        // Compare only the old root/metadata components; no full-heap credit.
        for ((name, pattern), candidate) in patterns.iter().zip(&candidates) {
            let old = NativeImmutableRegexMetadata::from_pattern(
                pattern,
                candidate.get_config(),
                candidate.pattern_len(),
            )
            .unwrap();
            let layout = candidate.resource_layout();
            assert!(
                native_arc_value_layout(layout.regex_payload())
                    .unwrap()
                    .size()
                    <= old.regex_arc_bytes,
                "{name}"
            );
            assert!(
                native_arc_value_layout(layout.metadata_payload())
                    .unwrap()
                    .size()
                    <= old.info_arc_bytes,
                "{name}"
            );
        }
        writeln!(
            stderr(),
            "native reconstructed shallow layouts: regex_arcs={} public_prefilter_arc={} report={}",
            charges.regex_arc_bytes,
            charges.public_prefilter_arc_bytes,
            serde_json::to_string(&report).unwrap()
        )
        .unwrap();
    }

    fn source_static_patterns() -> (Vec<(String, String)>, bool) {
        let inputs = super::schema_profile::inputs();
        let mut patterns = Vec::new();
        let mut hostname = false;
        for schema in &inputs.snapshot.schemas {
            for (name, pattern) in bundle_patterns(&inputs.snapshot.root, schema).unwrap() {
                patterns.push((format!("{}::{name}", schema.witness.name), pattern));
            }
            hostname |= schema
                .audit
                .formats
                .iter()
                .any(|format| format.value == "hostname");
        }
        assert_eq!(patterns.len(), 23);
        assert_eq!(patterns.len(), super::schema_profile::helper_count());
        assert!(hostname);
        patterns.push((
            "shared-hostname".to_owned(),
            super::schema_formats::HOSTNAME_SOURCE.to_owned(),
        ));
        assert_eq!(patterns.len(), 24);
        (patterns, hostname)
    }

    #[test]
    fn source_literal_prefix_dfa_buffers_fit_independent_start_envelopes() {
        use super::schema_literal_heap::{LiteralDfaHeap, LiteralStart};
        use super::schema_meter::measure;
        use aho_corasick::{MatchKind, StartKind, dfa::DFA};
        use regex_syntax::{Parser, hir::Look, hir::literal};
        use std::io::{Write as _, stderr};

        let (patterns, _) = source_static_patterns();
        let mut anchored = 0_usize;
        let mut unavailable = 0_usize;
        let mut finite = 0_usize;
        let mut instances = 0_usize;
        let mut requested = 0_usize;
        let mut observed = 0_i64;
        for (name, pattern) in &patterns {
            let hir = Parser::new().parse(pattern).unwrap();
            // Match the locked meta constructor's anchored-start exclusion.
            if hir.properties().look_set_prefix().contains(Look::Start) {
                anchored += 1;
                continue;
            }
            let mut extractor = literal::Extractor::new();
            extractor.kind(literal::ExtractKind::Prefix);
            let mut prefixes = literal::Seq::empty();
            prefixes.union(&mut extractor.extract(&hir));
            prefixes.optimize_for_prefix_by_preference();
            let Some(literals) = prefixes.literals() else {
                unavailable += 1;
                continue;
            };
            // Empty needles do not produce a usable automatic prefilter.
            if literals.is_empty() || literals.iter().any(|literal| literal.as_bytes().is_empty()) {
                unavailable += 1;
                continue;
            }
            finite += 1;
            let needles: Vec<_> = literals.iter().map(literal::Literal::as_bytes).collect();
            let lengths: Vec<_> = needles.iter().map(|needle| needle.len()).collect();
            for (mode, start) in [
                (LiteralStart::Anchored, StartKind::Anchored),
                (LiteralStart::Both, StartKind::Both),
            ] {
                let bound = LiteralDfaHeap::from_lengths(&lengths, mode).unwrap();
                let (dfa, construction) = measure(|| {
                    DFA::builder()
                        .match_kind(MatchKind::LeftmostFirst)
                        .start_kind(start)
                        .byte_classes(false)
                        .prefilter(false)
                        .build(&needles)
                        .unwrap()
                });
                let retained = construction.json()["retained_bytes"].as_i64().unwrap();
                assert!(retained > 0, "{name} {mode:?}");
                assert!(
                    usize::try_from(retained).unwrap() <= bound.total,
                    "{name} {mode:?}"
                );
                let ((), release) = measure(|| drop(hint::black_box(dfa)));
                assert_eq!(
                    release.json()["retained_bytes"],
                    -retained,
                    "{name} {mode:?}"
                );
                assert_eq!(release.json()["allocation_calls"], 0_u64);
                requested = requested.checked_add(bound.total).unwrap();
                observed = observed.checked_add(retained).unwrap();
                instances += 1;
            }
        }
        assert_eq!((anchored, unavailable, finite, instances), (17, 3, 4, 8));
        writeln!(stderr(), "literal DFA component: anchored={anchored} unavailable={unavailable} finite={finite} separately_constructed={instances} requested={requested} observed={observed}; retained buffers only, not hidden matcher selection or full programs").unwrap();
    }

    #[test]
    fn onepass_retained_public_global_shrinks_leave_exact_u64_and_state_id_buffers() {
        use super::schema_meter::measure;
        use regex_automata::util::primitives::StateID;

        assert_eq!(size_of::<StateID>(), 4);
        let ((table, starts), construction) = measure(|| {
            let mut table = Vec::with_capacity(hint::black_box(31));
            table.resize(8, hint::black_box(0_u64));
            let mut starts = Vec::with_capacity(hint::black_box(17));
            starts.resize(2, StateID::must(0));
            table.shrink_to_fit();
            starts.shrink_to_fit();
            hint::black_box((table, starts))
        });
        assert_eq!((table.capacity(), starts.capacity()), (8, 2));
        assert_eq!(construction.json()["retained_bytes"], 72_i64);
        let ((), release) = measure(|| drop(hint::black_box((table, starts))));
        assert_eq!(release.json()["retained_bytes"], -72_i64);
        assert_eq!(release.json()["allocation_calls"], 0_u64);
    }

    #[test]
    fn onepass_retained_constructor_and_release_exclude_the_live_shared_nfa() {
        use super::schema_cache_bounds::forward_nfa;
        use super::schema_meter::measure;
        use super::schema_onepass_bounds::{OnePassRetainedTables, candidate_onepass};
        use regex_automata::nfa::thompson::NFA;

        for nfa in [
            NFA::never_match(),
            forward_nfa("^a+$").unwrap(),
            forward_nfa("^(a+)(b+)$").unwrap(),
            NFA::new_many(&["^(a+)$", "^(b+)(c+)$"]).unwrap(),
        ] {
            let (dfa, construction) = measure(|| hint::black_box(candidate_onepass(&nfa).unwrap()));
            let table_bytes = 8 * dfa.state_len() * dfa.stride();
            let starts_bytes = 4 * (nfa.pattern_len() + 1);
            let tables = OnePassRetainedTables::from_dfa(&dfa).unwrap();
            assert_eq!(tables.table_bytes, table_bytes);
            assert_eq!(tables.starts_bytes, starts_bytes);
            let retained = i64::try_from(table_bytes + starts_bytes).unwrap();
            assert_eq!(construction.json()["retained_bytes"], retained);
            let ((), release) = measure(|| drop(hint::black_box(dfa)));
            assert_eq!(release.json()["retained_bytes"], -retained);
            assert_eq!(release.json()["allocation_calls"], 0_u64);
            hint::black_box(&nfa);
        }
    }

    #[test]
    fn onepass_retained_failed_candidate_drops_its_error_inside_measurement() {
        use super::schema_cache_bounds::forward_nfa;
        use super::schema_meter::measure;
        use super::schema_onepass_bounds::candidate_onepass;

        let nfa = forward_nfa("^(a*a)$").unwrap();
        let ((), construction) = measure(|| drop(candidate_onepass(&nfa).unwrap_err()));
        assert_eq!(construction.json()["retained_bytes"], 0_i64);
        assert!(construction.json()["allocation_calls"].as_u64().unwrap() > 0);
        hint::black_box(&nfa);
    }

    fn onepass_constructor_outcomes(patterns: &[(String, String)]) -> BTreeMap<String, bool> {
        use super::schema_cache_bounds::forward_nfa;
        use super::schema_onepass_bounds::candidate_onepass;

        patterns
            .iter()
            .map(|(name, pattern)| {
                let nfa = forward_nfa(pattern).unwrap();
                let built = match candidate_onepass(&nfa) {
                    Ok(dfa) => {
                        drop(dfa);
                        true
                    }
                    Err(error) => {
                        drop(error);
                        false
                    }
                };
                (name.clone(), built)
            })
            .collect()
    }

    #[test]
    fn onepass_retained_source_cohort_binds_all_independent_constructor_outcomes() {
        use super::schema_cache_bounds::forward_nfa;
        use super::schema_meter::measure;
        use super::schema_onepass_bounds::{
            OnePassRetainedTables, OnePassTables, candidate_onepass, nfa_table_upper_bound,
            retained_candidate,
        };
        use std::io::{Write as _, stderr};

        let (patterns, hostname) = source_static_patterns();
        assert!(hostname);
        // Establish every expected identity/outcome before consulting the model.
        let expected = onepass_constructor_outcomes(&patterns);
        assert_eq!(expected.len(), 24);
        assert!(expected.values().any(|built| *built));
        assert!(expected.values().any(|built| !*built));
        let mut observed = BTreeMap::new();
        let (mut requested_total, mut structural_total) = (0_usize, 0_usize);
        for (name, pattern) in &patterns {
            let nfa = forward_nfa(pattern).unwrap();
            let model = retained_candidate(pattern).unwrap();
            let modeled_success = matches!(&model, OnePassTables::Candidate(_));
            assert_eq!(modeled_success, expected[name], "{name}");
            assert!(observed.insert(name.clone(), modeled_success).is_none());
            let (actual, construction) = measure(|| match candidate_onepass(&nfa) {
                Ok(dfa) => Some(hint::black_box(dfa)),
                Err(error) => {
                    drop(error);
                    None
                }
            });
            match (actual, model) {
                (Some(dfa), OnePassTables::Candidate(tables)) => {
                    let stride = 1 << nfa.byte_classes().stride2();
                    assert_eq!(dfa.stride(), stride, "{name}");
                    assert!(dfa.state_len() <= nfa.states().len() + 1, "{name}");
                    let upper =
                        nfa_table_upper_bound(nfa.states().len(), stride, nfa.pattern_len())
                            .unwrap();
                    requested_total = requested_total.checked_add(tables.total).unwrap();
                    structural_total = structural_total.checked_add(upper.total).unwrap();
                    assert_eq!(
                        tables,
                        OnePassRetainedTables::from_dfa(&dfa).unwrap(),
                        "{name}"
                    );
                    assert!(tables.total <= upper.total, "{name}");
                    let retained =
                        i64::try_from(8 * dfa.state_len() * stride + 4 * (nfa.pattern_len() + 1))
                            .unwrap();
                    assert_eq!(i64::try_from(tables.total).unwrap(), retained, "{name}");
                    assert_eq!(construction.json()["retained_bytes"], retained, "{name}");
                    let ((), release) = measure(|| drop(hint::black_box(dfa)));
                    assert_eq!(release.json()["retained_bytes"], -retained, "{name}");
                    assert_eq!(release.json()["allocation_calls"], 0_u64, "{name}");
                }
                (None, OnePassTables::Absent(error)) => {
                    let independent = candidate_onepass(&nfa).unwrap_err();
                    assert_eq!(error.to_string(), independent.to_string(), "{name}");
                    assert_eq!(construction.json()["retained_bytes"], 0_i64, "{name}");
                }
                _ => panic!("independent constructor/model mismatch: {name}"),
            }
            hint::black_box(&nfa);
        }
        assert_eq!(observed, expected);
        let candidates = observed.values().filter(|built| **built).count();
        let absent = observed.len().checked_sub(candidates).unwrap();
        writeln!(stderr(), "onepass_retained: statics={} candidates={candidates} constructor_absent={absent} requested={requested_total} candidate_structural_upper={structural_total}; conditional raw-candidate buffers only, excludes shared NFA/inline storage/constructor peaks", observed.len()).unwrap();
    }

    #[test]
    fn nfa_heap_actual_empty_group_info_is_one_arc_and_releases_completely() {
        use super::schema_meter::measure;
        use super::schema_nfa_heap::GroupInfoRetainedHeap;
        use regex_automata::util::captures::GroupInfo;

        drop(GroupInfo::empty()); // Initializer observation only, not readiness.
        let (info, construction) = measure(|| hint::black_box(GroupInfo::empty()));
        let bound = GroupInfoRetainedHeap::from_info(&info).unwrap();
        let ((), release) = measure(|| drop(hint::black_box(info)));
        let observed = construction.json();
        let released = release.json();
        let retained = observed["retained_bytes"].as_i64().unwrap();
        assert_eq!(retained, 96_i64);
        assert_eq!(observed["allocation_calls"], 1_u64);
        assert_eq!(observed["allocated_bytes"], 96_u64);
        assert!(usize::try_from(retained).unwrap() <= bound.arc_bytes);
        assert_eq!(bound.requested_heap_bytes, bound.arc_bytes);
        assert_eq!(released["retained_bytes"], retained.checked_neg().unwrap());
        assert_eq!(released["allocation_calls"], 0_u64);
    }

    #[test]
    fn nfa_heap_actual_group_info_borrows_inputs_and_retains_inner_vectors() {
        use super::schema_meter::measure;
        use super::schema_nfa_heap::GroupInfoRetainedHeap;
        use regex_automata::util::captures::GroupInfo;

        for lengths in [vec![1_usize], vec![2, 5, 9]] {
            let groups: Vec<Vec<Option<&str>>> =
                lengths.iter().map(|len| vec![None; *len]).collect();
            let (info, construction) = measure(|| {
                GroupInfo::new(groups.iter().map(|group| group.iter().copied())).unwrap()
            });
            let bound = GroupInfoRetainedHeap::from_info(&info).unwrap();
            let retained = construction.json()["retained_bytes"].as_i64().unwrap();
            let ((), release) = measure(|| drop(hint::black_box(info)));
            assert!(retained > 0_i64);
            assert!(usize::try_from(retained).unwrap() <= bound.requested_heap_bytes);
            assert!(bound.name_entries_bytes > 0_usize);
            assert_eq!(
                release.json()["retained_bytes"],
                retained.checked_neg().unwrap()
            );
            assert_eq!(release.json()["allocation_calls"], 0_u64);
        }
    }

    fn expected_nfa_prefixes(
        pattern: &str,
    ) -> (Option<super::schema_nfa_heap::PrefixAbsence>, Vec<Hir>) {
        use super::schema_hybrid_prefix::candidates;
        use super::schema_nfa_heap::PrefixAbsence;
        use regex_automata::util::syntax;
        use regex_syntax::hir::Look;

        // Expected eligibility comes from HIR, never the subject's branch.
        let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true)).unwrap();
        let prefixes = candidates(pattern).unwrap();
        if hir.properties().look_set_prefix().contains(Look::Start) {
            (Some(PrefixAbsence::Anchored), Vec::new())
        } else if prefixes.is_empty() {
            (Some(PrefixAbsence::NoConcat), Vec::new())
        } else {
            (None, prefixes)
        }
    }

    #[test]
    fn nfa_heap_source_trees_cover_every_static_and_release_retained_not_peak() {
        use super::schema_cache_bounds::{forward_nfa, reverse_nfa};
        use super::schema_hybrid_prefix::reverse_prefix_nfa;
        use super::schema_meter::{Metrics, measure};
        use super::schema_nfa_heap::{NfaRetainedHeap, PatternNfaEnvelope, PrefixTrees};
        use regex_automata::nfa::thompson::NFA;
        use std::io::{Write as _, stderr};

        let (patterns, hostname) = source_static_patterns();
        assert!(hostname);
        let mut trees = 0_usize;
        let mut states = 0_usize;
        let mut total = 0_usize;
        let mut actual_retained = 0_i64;
        let mut prefix_candidates = 0_usize;
        let mut forward_total = 0_usize;
        let mut reverse_total = 0_usize;
        let mut prefix_total = 0_usize;
        for (name, pattern) in &patterns {
            let (absent, eligible) = expected_nfa_prefixes(pattern);
            let expected_count = eligible.len();
            let envelope = PatternNfaEnvelope::from_pattern(pattern).unwrap();
            total = total.checked_add(envelope.requested_heap_bytes).unwrap();
            forward_total = forward_total
                .checked_add(envelope.forward.requested_heap_bytes)
                .unwrap();
            reverse_total = reverse_total
                .checked_add(envelope.reverse.requested_heap_bytes)
                .unwrap();
            // The oracle/projection stays outside the actual constructor window.
            let mut observe = |nfa: NFA, construction: Metrics| {
                let tree = NfaRetainedHeap::from_nfa(&nfa).unwrap();
                let retained = construction.json()["retained_bytes"].as_i64().unwrap();
                assert!(retained > 0_i64, "{name}");
                assert!(
                    usize::try_from(retained).unwrap() <= tree.requested_heap_bytes,
                    "{name}"
                );
                let ((), release) = measure(|| drop(hint::black_box(nfa)));
                assert_eq!(
                    release.json()["retained_bytes"],
                    retained.checked_neg().unwrap(),
                    "{name}"
                );
                assert_eq!(release.json()["allocation_calls"], 0_u64, "{name}");
                trees = trees.checked_add(1).unwrap();
                states = states.checked_add(tree.states).unwrap();
                actual_retained = actual_retained.checked_add(retained).unwrap();
                tree.requested_heap_bytes
            };
            let (forward, forward_metrics) = measure(|| forward_nfa(pattern).unwrap());
            assert_eq!(
                observe(forward, forward_metrics),
                envelope.forward.requested_heap_bytes
            );
            let (reverse, reverse_metrics) = measure(|| reverse_nfa(pattern).unwrap());
            assert_eq!(
                observe(reverse, reverse_metrics),
                envelope.reverse.requested_heap_bytes
            );
            let mut maximum = 0_usize;
            for prefix in eligible {
                let (nfa, metrics) = measure(|| reverse_prefix_nfa(&prefix).unwrap());
                maximum = maximum.max(observe(nfa, metrics));
                prefix_candidates = prefix_candidates.checked_add(1).unwrap();
            }
            let expected = absent.map_or(
                PrefixTrees::Candidates {
                    count: expected_count,
                    max_tree_bytes: maximum,
                },
                PrefixTrees::Absent,
            );
            assert_eq!(envelope.prefix, expected, "{name}");
            prefix_total = prefix_total.checked_add(maximum).unwrap();
        }
        assert_eq!(patterns.len(), 24);
        assert_eq!(prefix_candidates, 5);
        assert_eq!(trees, 53);
        assert!(states > trees);
        assert!(total > 0_usize);
        assert_eq!(
            forward_total
                .checked_add(reverse_total)
                .unwrap()
                .checked_add(prefix_total)
                .unwrap(),
            total
        );
        writeln!(stderr(), "nfa_heap: statics={} trees={trees} states={states} prefix_candidates={prefix_candidates} forward={forward_total} reverse={reverse_total} max_prefix={prefix_total} construction_envelope={total} observed_all_candidate_retained={actual_retained}", patterns.len()).unwrap();
    }

    #[test]
    fn schema_metadata_heap_public_arc_requests_and_release_corroborate_header_and_alignment() {
        use super::schema_heap_layout::{NativeField, native_arc_layout};
        use super::schema_meter::measure;
        use std::sync::Arc;

        #[repr(C, align(64))]
        struct Aligned([u8; 64]);

        fn observe<T>(value: T) {
            let layout = native_arc_layout(NativeField::of::<T>()).unwrap();
            let (owner, construction_metrics) = measure(|| Arc::new(hint::black_box(value)));
            let ((), release_metrics) = measure(|| drop(hint::black_box(owner)));
            let construction = construction_metrics.json();
            let release = release_metrics.json();
            let requested = u64::try_from(layout.size()).unwrap();
            let retained = i64::try_from(layout.size()).unwrap();
            assert_eq!(construction["allocation_calls"], 1_u64);
            assert_eq!(construction["allocated_bytes"], requested);
            assert_eq!(construction["retained_bytes"], retained);
            assert_eq!(construction["peak_live_bytes"], retained);
            assert_eq!(release["allocation_calls"], 0_u64);
            assert_eq!(release["allocated_bytes"], 0_u64);
            assert_eq!(release["peak_live_bytes"], 0_i64);
            assert_eq!(release["retained_bytes"], retained.checked_neg().unwrap());
        }

        assert_eq!(size_of::<Aligned>(), 64);
        observe(7_u8);
        observe(11_u64);
        observe([13_u8; 17]);
        observe(Aligned([17_u8; 64]));
    }

    #[test]
    #[expect(
        clippy::vec_init_then_push,
        reason = "observe RegexInfo::new's fresh Vec push, including capacity-four growth"
    )]
    fn schema_metadata_heap_source_statics_reconcile_public_property_owner_requests() {
        use super::schema_metadata_heap::NativeImmutableRegexMetadata;
        use regex_automata::{MatchKind, meta::Regex, util::syntax};
        use regex_syntax::{ParserBuilder, hir::Properties};
        use std::io::{Write as _, stderr};
        use std::sync::LazyLock;

        let (patterns, hostname) = source_static_patterns();
        let mut heap_total = 0_usize;
        let mut inline_total = 0_usize;
        let mut observed_properties = 0_u64;
        let mut first = None;
        for (name, pattern) in &patterns {
            let original = Regex::builder()
                .configure(
                    Regex::config()
                        .match_kind(MatchKind::LeftmostFirst)
                        .utf8_empty(true)
                        .nfa_size_limit(Some(0x0001_0000))
                        .hybrid_cache_capacity(0x0001_0000)
                        .pool_capacity(1),
                )
                .syntax(syntax::Config::default().utf8(true))
                .build(pattern)
                .unwrap();
            assert_eq!(original.pattern_len(), 1);
            assert!(original.get_config().get_prefilter().is_none());
            let bound = NativeImmutableRegexMetadata::from_pattern(
                pattern,
                original.get_config(),
                original.pattern_len(),
            )
            .unwrap();
            let mut parser = ParserBuilder::new().utf8(true).build();
            let hir = parser.parse(pattern).unwrap();
            // Observation only: regex-automata 0.4.18 RegexInfo::new's public
            // owner sequence. HIR parsing and all engine construction are out.
            let ((props, union), construction_metrics) = super::schema_meter::measure(|| {
                let mut props = Vec::new();
                props.push(hint::black_box(hir.properties()).clone());
                let union = Properties::union(&props);
                hint::black_box((props, union))
            });
            assert_eq!(props.len(), 1, "{name}");
            assert_eq!(props.capacity(), 4, "{name}");
            assert_eq!(bound.props_capacity, 4);
            assert_eq!(props[0].memory_usage(), bound.props_box_bytes);
            assert_eq!(union.memory_usage(), bound.props_union_box_bytes);
            let properties_bytes = bound
                .props_vec_bytes
                .checked_add(bound.props_box_bytes)
                .unwrap()
                .checked_add(bound.props_union_box_bytes)
                .unwrap();
            let ((), release_metrics) = super::schema_meter::measure(|| {
                drop(hint::black_box((props, union)));
            });
            let construction = construction_metrics.json();
            let release = release_metrics.json();
            let requested = u64::try_from(properties_bytes).unwrap();
            let retained = i64::try_from(properties_bytes).unwrap();
            assert_eq!(construction["allocation_calls"], 3_u64, "{name}");
            assert_eq!(construction["allocated_bytes"], requested, "{name}");
            assert_eq!(construction["retained_bytes"], retained, "{name}");
            assert_eq!(construction["peak_live_bytes"], retained, "{name}");
            assert_eq!(release["allocation_calls"], 0_u64, "{name}");
            assert_eq!(release["allocated_bytes"], 0_u64, "{name}");
            assert_eq!(release["peak_live_bytes"], 0_i64, "{name}");
            assert_eq!(
                release["retained_bytes"],
                retained.checked_neg().unwrap(),
                "{name}"
            );
            assert_eq!(
                bound.static_inline_bytes,
                size_of::<LazyLock<Option<Regex>>>()
            );
            heap_total = heap_total.checked_add(bound.requested_heap_bytes).unwrap();
            inline_total = inline_total.checked_add(bound.static_inline_bytes).unwrap();
            observed_properties = observed_properties.checked_add(requested).unwrap();
            if let Some(previous) = first {
                assert_eq!(bound, previous);
            } else {
                first = Some(bound);
            }
        }
        let bound = first.unwrap();
        let population = bound.population(23, hostname).unwrap();
        assert_eq!(population.owners, 24);
        assert_eq!(population.requested_heap_bytes, heap_total);
        assert_eq!(population.static_inline_bytes, inline_total);
        writeln!(stderr(), "native immutable regex metadata: helpers=23 hostname-statics=1 owners=24 regex-arc={} info-arc={} props-capacity={} props-vec={} props-box={} union-box={} heap-per-owner={} heap-total={heap_total} static-inline-per-owner={} static-inline-total={inline_total} observed-property-bytes={observed_properties}; metadata only, excludes strategy/graphs/onepass/automatic-prefilters/pools/caches/constructor-scratch, not a full-program/physical/guest/readiness bound", bound.regex_arc_bytes, bound.info_arc_bytes, bound.props_capacity, bound.props_vec_bytes, bound.props_box_bytes, bound.props_union_box_bytes, bound.requested_heap_bytes, bound.static_inline_bytes).unwrap();
    }

    #[test]
    fn schema_pool_heap_public_constructor_never_calls_its_factory() {
        use regex_automata::{meta::Regex, util::pool::Pool};
        use std::{cell::Cell, hint::black_box};

        let regex = Regex::new("^synthetic+$").unwrap();
        for capacity in [0_usize, 1, 2, 7] {
            let calls = Cell::new(0_usize);
            let (pool, construction) = super::schema_meter::measure(|| {
                black_box(Pool::with_capacity(black_box(capacity), || {
                    calls.set(calls.get().checked_add(1).unwrap());
                    regex.create_cache()
                }))
            });
            assert_eq!(calls.get(), 0);
            assert_eq!(construction.json()["allocation_calls"], 2_u64);
            // Positive control is outside the unused-constructor measurement.
            drop(black_box(pool.get()));
            assert_eq!(calls.get(), 1);
            drop(black_box(pool.get()));
            assert_eq!(calls.get(), 1);
            drop(pool);
        }
    }

    #[test]
    fn schema_pool_heap_source_statics_and_actual_clone_requests_reconcile() {
        use super::schema_pool_heap::UnusedPoolHeap;
        use regex_automata::{MatchKind, meta::Regex, util::syntax};
        use std::io::{Write as _, stderr};

        let (patterns, hostname) = source_static_patterns();
        let mut requested_total = 0_usize;
        let mut observed_total = 0_u64;
        for (name, pattern) in &patterns {
            // Same strict builder as bundle_patterns; compilation and programs
            // remain outside the measurement. Clone shares both program Arcs.
            let original = Regex::builder()
                .configure(
                    Regex::config()
                        .match_kind(MatchKind::LeftmostFirst)
                        .utf8_empty(true)
                        .nfa_size_limit(Some(0x0001_0000))
                        .hybrid_cache_capacity(0x0001_0000)
                        .pool_capacity(1),
                )
                .syntax(syntax::Config::default().utf8(true))
                .build(pattern)
                .unwrap();
            let bound = UnusedPoolHeap::native(original.get_config()).unwrap();
            let (cloned, construction_metrics) =
                super::schema_meter::measure(|| hint::black_box(original.clone()));
            let ((), release_metrics) =
                super::schema_meter::measure(|| drop(hint::black_box(cloned)));
            let construction = construction_metrics.json();
            let release = release_metrics.json();
            let requested = u64::try_from(bound.requested_bytes).unwrap();
            let retained = i64::try_from(bound.requested_bytes).unwrap();
            assert_eq!(construction["allocation_calls"], 3_u64, "{name}");
            assert_eq!(construction["allocated_bytes"], requested, "{name}");
            assert_eq!(construction["retained_bytes"], retained, "{name}");
            assert_eq!(construction["peak_live_bytes"], retained, "{name}");
            assert_eq!(release["allocation_calls"], 0_u64, "{name}");
            assert_eq!(release["allocated_bytes"], 0_u64, "{name}");
            assert_eq!(release["peak_live_bytes"], 0_i64, "{name}");
            assert_eq!(
                release["retained_bytes"],
                retained.checked_neg().unwrap(),
                "{name}"
            );
            requested_total = requested_total.checked_add(bound.requested_bytes).unwrap();
            observed_total = observed_total
                .checked_add(construction["allocated_bytes"].as_u64().unwrap())
                .unwrap();
        }
        let bound = UnusedPoolHeap::native(&Regex::config().pool_capacity(1)).unwrap();
        assert_eq!(bound.population(23, hostname).unwrap(), requested_total);
        assert_eq!(u64::try_from(requested_total).unwrap(), observed_total);
        // Counts and constructors do not prove absence of future acquisition.
        // The lifetime bound remains conditional on no get/clone/reset/ambient
        // search, and excludes programs, explicit caches, inline Regex, and RSS.
        writeln!(stderr(), "unused native meta pools: helpers=23 hostname-statics=1 owners=24 box={} shard-stride={} shards={} shard-bytes={} closure={} per-owner={} requested-total={requested_total} observed-total={observed_total}; conditional unused-pool bookkeeping only", bound.boxed_pool_bytes, bound.shard_stride, bound.shards, bound.shard_bytes, bound.closure_bytes, bound.requested_bytes).unwrap();
    }

    #[test]
    fn every_compiled_schema_cache_layout_funds_sibling_and_callback_storage() {
        use super::schema_bundle_heap::{CacheBundleHeap, format_peak};
        use super::schema_meta_heap::MetaCacheHeap;
        use regex_automata::meta::Cache;
        use std::collections::BTreeMap;
        use std::io::{Write as _, stderr};

        let catalog = super::schema_build::linked_explicit_catalog(
            super::schema_build::LinkTargets::NativeOnly,
        )
        .unwrap();
        assert_eq!(
            catalog
                .reports()
                .iter()
                .map(|row| row.target.as_str())
                .collect::<Vec<_>>(),
            ["native"]
        );
        let layouts = catalog.native_cache_layouts().unwrap();
        assert_eq!(layouts.schema_version, 2);
        assert_eq!(layouts.word_bytes, size_of::<usize>());
        assert_eq!(layouts.cache_bytes, size_of::<Cache>());
        assert_eq!(layouts.cache_alignment, align_of::<Cache>());
        let inputs = super::schema_profile::inputs();
        let schemas = &inputs.snapshot.schemas;
        assert_eq!(layouts.rows.len(), schemas.len());
        let mut actual = BTreeMap::new();
        for row in &layouts.rows {
            assert!(actual.insert(row.name.as_str(), row).is_none());
        }
        assert_eq!(
            actual.keys().copied().collect::<Vec<_>>(),
            schemas
                .iter()
                .map(|schema| schema.witness.name.as_str())
                .collect::<Vec<_>>()
        );
        let mut helpers = 0_usize;
        let mut hostname_schemas = 0_usize;
        let mut empty_bundles = 0_usize;
        let mut maximum = 0_usize;
        for schema in schemas {
            let patterns = bundle_patterns(&inputs.snapshot.root, schema).unwrap();
            let inline = actual[schema.witness.name.as_str()];
            assert_eq!(
                inline.inline_bytes,
                patterns.len().checked_mul(layouts.cache_bytes).unwrap()
            );
            assert_eq!(
                inline.alignment,
                if patterns.is_empty() {
                    1
                } else {
                    layouts.cache_alignment
                }
            );
            helpers = helpers.checked_add(patterns.len()).unwrap();
            empty_bundles = empty_bundles
                .checked_add(usize::from(patterns.is_empty()))
                .unwrap();
            let peaks: Vec<_> = patterns
                .iter()
                .map(|(_, pattern)| {
                    MetaCacheHeap::from_pattern(pattern, 0x0001_0000)
                        .unwrap()
                        .peak
                })
                .collect();
            let callback = format_peak(
                schema
                    .audit
                    .formats
                    .iter()
                    .map(|format| format.value.as_str()),
            )
            .unwrap();
            hostname_schemas = hostname_schemas
                .checked_add(usize::from(callback > 0))
                .unwrap();
            let bound = CacheBundleHeap::compose(&peaks, inline.inline_bytes, callback).unwrap();
            assert!(
                bound.peak <= 512 * 1024,
                "{}: {bound:?}",
                schema.witness.name
            );
            maximum = maximum.max(bound.peak);
        }
        assert_eq!(helpers, super::schema_profile::helper_count());
        assert_eq!(hostname_schemas, 2);
        assert_eq!(empty_bundles, 247);
        assert!(maximum > 0);
        writeln!(stderr(), "native cache bundles: schemas={} helpers={helpers} hostname-schemas={hostname_schemas} empty-bundles={empty_bundles} maximum-storage-bytes={maximum}", schemas.len()).unwrap();
    }

    #[test]
    fn hostname_fresh_cache_and_guard_observations_fit_the_narrowed_owner_bound() {
        use super::schema_cache_bounds::{forward_nfa, reverse_nfa};
        use super::schema_heap_layout::HeapLayout;
        use super::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
        use super::schema_hybrid_heap::HybridHeap;
        use super::schema_meta_heap::FreshAnchoredHalfHeap;
        use std::hint::black_box;
        use std::mem::size_of;

        let pattern = super::schema_formats::HOSTNAME_SOURCE;
        let proof = FreshAnchoredHalfHeap::from_pattern(pattern, 255).unwrap();
        // Initialize shared programs outside callback-local allocation windows.
        assert!(super::schema_formats::initialize_reference_formats());
        let mut observed = 0_usize;
        let mut guard_refusals = 0_usize;
        for (hostname, valid, guarded) in [
            ("a.example".to_owned(), true, false),
            ([63_usize; 4].map(|n| "a".repeat(n)).join("."), true, false),
            (format!("{}!", "a".repeat(254)), false, false),
            ("\u{00e9}.example".to_owned(), false, false),
            ("a".repeat(256), false, true),
        ] {
            for _repeat in 0_usize..2 {
                let (actual, memory) = super::schema_meter::measure(|| {
                    black_box(super::schema_formats::reference_hostname_format(black_box(
                        &hostname,
                    )))
                });
                assert_eq!(actual, valid);
                observed = observed.checked_add(1).unwrap();
                let report = memory.json();
                assert_eq!(report["retained_bytes"], 0_i64);
                if guarded {
                    guard_refusals = guard_refusals.checked_add(1).unwrap();
                    assert_eq!(report["allocated_bytes"], 0_u64);
                    assert_eq!(report["peak_live_bytes"], 0_i64);
                    assert_eq!(report["allocation_calls"], 0_u64);
                } else {
                    assert!(report["allocated_bytes"].as_u64().unwrap() > 0);
                    let peak = report["peak_live_bytes"].as_u64().unwrap();
                    assert!(peak > 0);
                    assert!(peak <= u64::try_from(proof.peak).unwrap());
                }
            }
        }
        assert_eq!(observed, 10);
        assert_eq!(guard_refusals, 2);
        let layout = HeapLayout::new(size_of::<usize>(), 16).unwrap();
        let mut constructors = 0_usize;
        for (nfa, direction) in [
            (forward_nfa(pattern).unwrap(), HybridDirection::Forward),
            (reverse_nfa(pattern).unwrap(), HybridDirection::Reverse),
        ] {
            let dfa = candidate_hybrid(&nfa, direction).unwrap();
            let fresh = HybridHeap::from_fresh_dfa(&dfa, layout).unwrap();
            let (cache, construction) = super::schema_meter::measure(|| dfa.create_cache());
            let ((), release) = super::schema_meter::measure(|| drop(cache));
            let construction_report = construction.json();
            let release_report = release.json();
            let peak = construction_report["peak_live_bytes"].as_u64().unwrap();
            let retained = construction_report["retained_bytes"].as_i64().unwrap();
            assert!(peak > 0);
            assert!(retained > 0);
            assert!(peak <= u64::try_from(fresh.peak).unwrap());
            assert_eq!(
                retained.checked_add(release_report["retained_bytes"].as_i64().unwrap()),
                Some(0)
            );
            constructors = constructors.checked_add(1).unwrap();
        }
        assert_eq!(constructors, 2);
    }

    #[test]
    fn native_matcher_records_remain_complete_exploratory_observations() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut memberships = Vec::new();
        for (filename, expected_sha256) in [
            (
                "native-pilot-20261027.json",
                "2a2a44329243aa1bd792542415929be82cb3335f8c8b32395194e284f62c918b",
            ),
            (
                "native-refined-20261027.json",
                "199723884baf3e11b0195f59ebeea2ca56c0a49b2dc2fe1582bd76483e36cdce",
            ),
        ] {
            let bytes = fs::read(
                root.join("crates/auctioneer-bidder/tests/data/schema-matchers")
                    .join(filename),
            )
            .unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), expected_sha256);
            let record: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                record["metadata"]["scope"],
                "native matcher primitives; no production engine selection or resource qualification"
            );
            assert_eq!(record["metadata"]["profile"], "release");
            assert_eq!(record["corpus"]["schema_count"], 261_u64);
            assert_eq!(record["corpus"]["capture_cases"], 300_u64);
            let patterns = record["patterns"].as_array().unwrap();
            assert_eq!(patterns.len(), 24);
            let mut cases = 0_usize;
            let mut keys = Vec::new();
            for pattern in patterns {
                keys.push(pattern["key"].as_str().unwrap().to_owned());
                for engine in ["meta", "pikevm"] {
                    for phase in [
                        "program_lifetime",
                        "cache_history_lifetime",
                        "program_and_cache_lifetime",
                    ] {
                        assert_eq!(pattern["memory"][engine][phase]["retained_bytes"], 0_i64);
                    }
                }
                for case in pattern["searches"].as_array().unwrap() {
                    cases = cases.checked_add(1).unwrap();
                    for engine in ["meta", "pikevm"] {
                        for phase in ["cache_construction_ns", "first_search_ns", "warm_batch_ns"] {
                            let samples = &case["timing"][engine][phase];
                            if case["guard_skipped"] == true {
                                assert!(samples.is_null());
                            } else {
                                assert_eq!(samples.as_array().unwrap().len(), 7);
                            }
                        }
                    }
                }
            }
            assert_eq!(cases, 596);
            memberships.push(keys);
        }
        assert_eq!(memberships.len(), 2);
        assert_eq!(memberships.first(), memberships.last());
    }

    #[test]
    fn source_target_observes_the_independently_pinned_production_population() {
        let inputs = super::schema_profile::inputs();
        assert_eq!(inputs.snapshot.schemas.len(), 261);
        assert_eq!(inputs.snapshot.schema_bytes, 164_518);

        let inventory_bytes =
            include_bytes!("conformance/fixtures/reference/catalog/source-inventory.toml");
        assert_eq!(
            format!("{:x}", Sha256::digest(inventory_bytes)),
            "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6"
        );
        let frozen_inventory: toml::Value =
            toml::from_str(str::from_utf8(inventory_bytes).unwrap()).unwrap();
        let mut frozen_names: Vec<_> = frozen_inventory["schema"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect();
        frozen_names.sort_unstable();
        assert_eq!(
            inputs
                .snapshot
                .schemas
                .iter()
                .map(|row| row.witness.name.as_str())
                .collect::<Vec<_>>(),
            frozen_names
        );

        let capture = super::schema_reference::captured().unwrap();
        assert_eq!(capture.cases.len(), 300);
        assert_eq!(capture.sources.len(), 14);
        let capture_bytes =
            include_bytes!("conformance/fixtures/reference/schema-reference/parameters.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(capture_bytes)),
            "6efc0eaea62063f86d391df46265e1df2fb06789cfe2e746f53c3c9e4c41863e"
        );
        let frozen_capture: serde_json::Value = serde_json::from_slice(capture_bytes).unwrap();
        assert_eq!(serde_json::to_value(capture).unwrap(), frozen_capture);
        assert!(hint::black_box(super::SCHEMA_SOURCE_CONFORMANCE));
    }
}

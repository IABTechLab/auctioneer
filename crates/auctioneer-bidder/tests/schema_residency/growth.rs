//! Same-build data-only source reconstruction relative to d26fcf7.
//! No historical build, serving activation, physical fit or common C/I claim.

#![cfg(all(test, not(target_arch = "wasm32")))]

use auctioneer_config::types::BidderMetadataDeclaration;
use crate::schema_formats::HOSTNAME_SOURCE;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const LABEL: &str = "same-build data-only source reconstruction relative to d26fcf7";
const INCOMPLETE: &str = "catalog growth reconstruction incomplete";
// Same existing per-input staging cap; this is only a test-fixture interning fence.
const INPUT_BYTES: usize = 1 << 20;
const NATIVE_STAGING: &[u8] = include_bytes!("../../src/schema/staging.rs");

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
struct Delta {
    added: BTreeSet<String>,
    removed: BTreeSet<String>,
    changed: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct View {
    source: Value,
    measurement: Value,
    membership: Value,
    owners: BTreeMap<String, String>,
    programs: Vec<(String, String)>,
    seed_sha256: String,
    expanded_sha256: String,
    generated_source_bytes: u64,
}

fn reference_path(_value: Option<OsString>) -> Result<PathBuf, String> {
    Err(INCOMPLETE.into())
}

fn intern_rows(
    _rows: &[(&str, &str, Option<&str>)],
) -> Result<Vec<BidderMetadataDeclaration>, String> {
    Err(INCOMPLETE.into())
}

fn owner_delta(_reference: &BTreeMap<String, String>, _candidate: &BTreeMap<String, String>) -> Delta {
    Delta::default()
}

fn match_views(_reference: &View, _candidate: &View, _declared: &Delta) -> Result<Value, String> {
    Err(INCOMPLETE.into())
}

fn verify_frozen(_token: &Value, _reference: &View, _candidate: &View) -> Result<(), String> {
    Err(INCOMPLETE.into())
}

fn schedule(_graph: &Value, _owners: &BTreeMap<String, String>) -> Result<Vec<(String, String)>, String> {
    Err(INCOMPLETE.into())
}

fn membership(_seed: &Value, _independent: &Value) -> Result<(), String> {
    Err(INCOMPLETE.into())
}

fn observe_matched(
    _reference: &View,
    _candidate: &View,
    _declared: &Delta,
    _observe: impl FnMut(&[(String, String)]) -> Result<Value, String>,
) -> Result<Value, String> {
    Err(INCOMPLETE.into())
}

fn run_real() -> Result<Value, String> {
    let _ = (LABEL, INPUT_BYTES, NATIVE_STAGING);
    Err(INCOMPLETE.into())
}

fn native_format_first(_source: &[u8]) -> Result<(), String> {
    Err(INCOMPLETE.into())
}

fn source_files(
    _workspace: &Path,
    _paths: &BTreeSet<PathBuf>,
    _snapshot_paths: &BTreeSet<PathBuf>,
) -> Result<Value, String> {
    Err(INCOMPLETE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_config::schema_staging_build::catalog_reference::{REFERENCE_COMMIT, REFERENCE_TREE};
    use serde_json::json;
    use std::cell::Cell;
    use std::{fs, str};
    use quote::ToTokens as _;

    fn view(reference: bool) -> View {
        View {
            source: if reference {
                json!({"kind":"pinned-git-object-data","commit":REFERENCE_COMMIT,"tree":REFERENCE_TREE,
                    "lock_sha256":"historical-lock","files":{"Cargo.lock":"historical-lock"}})
            } else {
                json!({"kind":"current-source-content-snapshot","lock_sha256":"candidate-lock",
                    "files":{"Cargo.lock":"candidate-lock"}})
            },
            measurement: json!({
                "build":{"target":"fixture-target","profile":"fixture-profile",
                    "compiler_verbose":"fixture compiler","encoded_rustflags":"",
                    "enabled_features":["fixture-feature"]},
                "environment":{"TARGET":"fixture-target","OPT_LEVEL":"0","DEBUG":"true"},
                "abi":{"arch":"fixture-arch","pointer_width":"64","endian":"little"},
                "measurement_lock_sha256":"measurement-lock",
                "measurement_sources":{"fake/current-generator.rs":"generator-digest"},
                "regex_source":"fixture-reviewed-source","regex_features":["meta","syntax"],
                "generator_options":"fixture-options","workload":"fixture-workload",
                "non_catalog":"fixture-unchanged-non-catalog-scope"
            }),
            membership: json!({"roots":["fake_a"],"effective_schema_owners":["fake_a"]}),
            owners: BTreeMap::from([("fake_a".into(), "schema-a".into())]),
            programs: vec![("shared::hostname".into(), HOSTNAME_SOURCE.into()),
                ("fake_a::StagedSchema0::Regex0".into(), "^x$".into())],
            seed_sha256: if reference { "reference-seed" } else { "candidate-seed" }.into(),
            expanded_sha256: if reference { "reference-expanded" } else { "candidate-expanded" }.into(),
            generated_source_bytes: 19,
        }
    }

    fn seed_membership() -> (Value, Value) {
        let seed = json!({
            "implemented_roots_all_features":["fake_a","fake_alias","fake_local"],
            "inventory_dispositions":[{"name":"fake_a","implementation":"fake_impl","disposition":"implemented"}],
            "callable_bindings":[["fake_a","fake_impl"],["fake_alias","fake_impl"],["fake_local","fake_local"]],
            "selection":{"schema_owners":["fake_a"],"members":[
                {"name":"fake_a","implementation":"fake_impl","is_root":true,"ancestry":[],"schema_owner":"fake_a"},
                {"name":"fake_alias","implementation":"fake_impl","is_root":true,"ancestry":["fake_a"],"schema_owner":"fake_a"},
                {"name":"fake_local","implementation":"fake_local","is_root":true,"ancestry":[],"schema_owner":null}]}
        });
        let independent = json!({
            "implemented_roots_all_features":["fake_a","fake_alias","fake_local"],
            "inventory_dispositions":[{"name":"fake_a","implementation":"fake_impl","disposition":"implemented"}],
            "callable_bindings":[["fake_a","fake_impl"],["fake_alias","fake_impl"],["fake_local","fake_local"]],
            "roots":["fake_a","fake_alias","fake_local"],"effective_schema_owners":["fake_a"],
            "members":[
                {"name":"fake_a","implementation":"fake_impl","is_root":true,"implemented":true,"ancestry":[],"effective_schema_owner":"fake_a"},
                {"name":"fake_alias","implementation":"fake_impl","is_root":true,"implemented":true,"ancestry":["fake_a"],"effective_schema_owner":"fake_a"},
                {"name":"fake_local","implementation":"fake_local","is_root":true,"implemented":true,"ancestry":[],"effective_schema_owner":null}]
        });
        (seed, independent)
    }

    fn graph() -> Value {
        json!({
            "auxiliary":{"dispatch_edges":[
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"},
                {"from":"initialize_native","to":"StagedSchema1","role":"initialize-regexes","ownership":"callee-owned"}
            ]},
            "declarations":[
                {"owner":"fake_a","marker":"StagedSchema0","callbacks":[],
                    "initialization":["SECOND_REGEX","FIRST_REGEX"],
                    "programs":[
                        {"identity":"fake_a::StagedSchema0::FIRST_REGEX","symbol":"FIRST_REGEX","pattern":"^x$"},
                        {"identity":"fake_a::StagedSchema0::SECOND_REGEX","symbol":"SECOND_REGEX","pattern":"^y$"}
                    ]},
                {"owner":"fake_b","marker":"StagedSchema1","callbacks":[],
                    "initialization":["FIRST_REGEX"],
                    "programs":[
                        {"identity":"fake_b::StagedSchema1::FIRST_REGEX","symbol":"FIRST_REGEX","pattern":"^x$"}
                    ]}
            ]
        })
    }

    #[test]
    fn runtime_reference_input_is_required_without_global_environment_mutation() {
        assert_eq!(reference_path(None).unwrap_err(), "catalog reference directory is required");
        assert_eq!(reference_path(Some(OsString::new())).unwrap_err(),
            "catalog reference directory is empty");
        assert_eq!(reference_path(Some("/fixture/reference".into())).unwrap(),
            PathBuf::from("/fixture/reference"));
    }

    #[test]
    fn raw_metadata_rows_preserve_alias_implementation_and_disabled_feature_declarations() {
        let rows = intern_rows(&[
            ("fake_a", "fake_impl", None), ("fake_alias", "fake_impl", None),
            ("fake_dev", "fake_dev", Some("insecure-bidders")),
        ]).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].bidder, "fake_a");
        assert_eq!(rows[1].bidder, "fake_alias");
        assert_eq!(rows[1].implementation, "fake_impl");
        assert_eq!(rows[2].required_feature, Some("insecure-bidders"));
        let oversized = "x".repeat(INPUT_BYTES + 1);
        assert_eq!(intern_rows(&[(&oversized, "fake_impl", None)]).unwrap_err(),
            "metadata interning bound");
        assert_eq!(intern_rows(&[("fake_a","fake_impl",None),("fake_a","fake_other",None)]).unwrap_err(),
            "duplicate metadata declaration");
    }

    #[test]
    fn seed_membership_matches_the_independent_alias_walk_exactly() {
        let (seed, independent) = seed_membership();
        membership(&seed, &independent).unwrap();
        for pointer in [
            "/implemented_roots_all_features", "/inventory_dispositions",
            "/callable_bindings", "/selection/schema_owners", "/selection/members",
        ] {
            let mut changed = seed.clone();
            *changed.pointer_mut(pointer).unwrap() = json!([]);
            assert_eq!(membership(&changed, &independent).unwrap_err(),
                "independent membership mismatch", "{pointer}");
        }
        let mut changed = seed;
        changed["selection"]["members"][1]["ancestry"] = json!([]);
        assert_eq!(membership(&changed, &independent).unwrap_err(), "independent membership mismatch");
    }

    #[test]
    fn actual_initialization_edges_keep_hostname_first_and_preserve_helper_order() {
        let owners = BTreeMap::from([("fake_a".into(),"a".into()),("fake_b".into(),"b".into())]);
        let expected = vec![
            ("shared::hostname".into(), HOSTNAME_SOURCE.into()),
            ("fake_a::StagedSchema0::SECOND_REGEX".into(), "^y$".into()),
            ("fake_a::StagedSchema0::FIRST_REGEX".into(), "^x$".into()),
            ("fake_b::StagedSchema1::FIRST_REGEX".into(), "^x$".into()),
        ];
        assert_eq!(schedule(&graph(), &owners).unwrap(), expected);
        let mut reordered = graph();
        reordered["declarations"].as_array_mut().unwrap().reverse();
        assert_eq!(schedule(&reordered, &owners).unwrap(), expected);
        let mut changed = graph();
        changed["declarations"][1]["owner"] = json!("fake_a");
        assert_eq!(schedule(&changed,&owners).unwrap_err(),"expanded schedule mismatch");
        let mut changed = graph();
        changed["declarations"][0]["marker"] = json!("StagedSchema99");
        assert_eq!(schedule(&changed,&owners).unwrap_err(),"expanded schedule mismatch");
        let mut changed = graph();
        let duplicate=changed["declarations"][0]["programs"][0].clone();
        changed["declarations"][0]["programs"].as_array_mut().unwrap().push(duplicate);
        assert_eq!(schedule(&changed,&owners).unwrap_err(),"expanded schedule mismatch");
        for initialization in [
            json!(["FIRST_REGEX"]),
            json!(["SECOND_REGEX","FIRST_REGEX","FIRST_REGEX"]),
            json!(["SECOND_REGEX","FIRST_REGEX","UNKNOWN_REGEX"]),
        ] {
            let mut changed=graph();
            changed["declarations"][0]["initialization"]=initialization;
            assert_eq!(schedule(&changed,&owners).unwrap_err(),"expanded schedule mismatch");
        }
        for edges in [
            json!([]),
            json!([
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"},
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"}
            ]),
            json!([
                {"from":"initialize_native","to":"StagedSchema0","role":"initialize-regexes","ownership":"callee-owned"},
                {"from":"initialize_native","to":"StagedSchema99","role":"initialize-regexes","ownership":"callee-owned"}
            ]),
        ] {
            let mut changed=graph();
            changed["auxiliary"]["dispatch_edges"]=edges;
            assert_eq!(schedule(&changed,&owners).unwrap_err(),"expanded schedule mismatch");
        }
    }

    #[test]
    fn native_formats_initialize_unconditionally_before_generated_initializers() {
        native_format_first(NATIVE_STAGING).unwrap();
        let mut syntax: syn::File = syn::parse_file(str::from_utf8(NATIVE_STAGING).unwrap()).unwrap();
        let function=syntax.items.iter_mut().find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident=="initialize_native" => Some(function),
            _ => None,
        }).unwrap();
        function.block=Box::new(syn::parse_quote!({
            generated::retain_for_inspection();
            generated::initialize_native() && formats::initialize()
        }));
        assert_eq!(native_format_first(syntax.to_token_stream().to_string().as_bytes()).unwrap_err(),
            "native format initialization order mismatch");
    }

    #[test]
    fn source_capture_only_excludes_explicit_generated_snapshot_paths() {
        let directory=tempfile::tempdir().unwrap();
        let root=directory.path().canonicalize().unwrap();
        let workspace=root.join("workspace");
        fs::create_dir(&workspace).unwrap();
        let source=workspace.join("source.rs");
        fs::write(&source,b"source data").unwrap();
        let snapshot=root.join("known-generated-schema.json");
        let paths=BTreeSet::from([source.clone(),snapshot.clone()]);
        let captured=source_files(&workspace,&paths,&BTreeSet::from([snapshot])).unwrap();
        assert_eq!(captured.as_object().unwrap().len(),1);
        assert_eq!(captured["source.rs"]["bytes"],11);
        let unexpected=root.join("unexpected.rs");
        fs::write(&unexpected,b"unaccounted source").unwrap();
        assert_eq!(source_files(&workspace,&BTreeSet::from([source,unexpected]),&BTreeSet::new())
            .unwrap_err(),"unexpected source path outside workspace");
    }

    #[test]
    fn identical_owner_sets_allow_zero_growth_without_zero_absolute_observations() {
        let reference = view(true);
        let candidate = view(false);
        let calls = Cell::new(0);
        let report = observe_matched(&reference, &candidate, &Delta::default(), |programs| {
            calls.set(calls.get() + 1);
            assert_eq!(programs, &[("shared::hostname".into(),HOSTNAME_SOURCE.into()),
                ("fake_a::StagedSchema0::Regex0".into(),"^x$".into())]);
            Ok(json!({"programs":[{"identity":"shared::hostname",
                "cold":{"retained_bytes":2,"peak_live_bytes":3}},
                {"identity":"fake_a::StagedSchema0::Regex0",
                "cold":{"retained_bytes":13,"peak_live_bytes":17}}],
                "repeat":{"allocated_bytes":0},
                "late_size_limit_failure":{"retained_bytes":0},
                "physical_or_generated_singleton_qualification":false}))
        }).unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(report["label"], LABEL);
        assert_eq!(report["owner_delta"], json!({"added":[],"removed":[],"changed":[]}));
        assert_eq!(report["reference"]["constructor_observation"]["programs"][1]["cold"]["retained_bytes"], 13);
        assert_eq!(report["candidate"]["constructor_observation"]["programs"][1]["cold"]["peak_live_bytes"], 17);
        assert_eq!(report["component_deltas"]["generated_source_bytes"], 0);
        assert_eq!(report["common_C_I_qualified"], false);
        assert_eq!(report["task8_complete"], false);
    }

    #[test]
    fn declared_added_owner_is_exact_and_ordinal_changes_do_not_hide_substitution() {
        let reference = view(true);
        let mut candidate = view(false);
        candidate.owners.insert("fake_0".into(), "schema-new".into());
        candidate.programs = vec![
            ("shared::hostname".into(), HOSTNAME_SOURCE.into()),
            ("fake_0::StagedSchema0::Regex0".into(), "^new$".into()),
            ("fake_a::StagedSchema1::Regex0".into(), "^x$".into()),
        ];
        let declared = Delta { added:BTreeSet::from(["fake_0".into()]), ..Delta::default() };
        assert_eq!(owner_delta(&reference.owners, &candidate.owners), declared);
        match_views(&reference, &candidate, &declared).unwrap();
        assert_eq!(match_views(&reference, &candidate, &Delta::default()).unwrap_err(),
            "undeclared owner delta");
        candidate.programs[2].1="^substituted$".into();
        assert_eq!(match_views(&reference, &candidate, &declared).unwrap_err(),
            "undeclared program substitution");
    }

    #[test]
    fn changed_and_removed_schema_owners_cannot_be_hidden_by_an_added_owner() {
        let reference = view(true);
        let mut candidate = view(false);
        candidate.owners.insert("fake_a".into(), "changed-schema".into());
        assert_eq!(owner_delta(&reference.owners,&candidate.owners),
            Delta { changed:BTreeSet::from(["fake_a".into()]), ..Delta::default() });
        assert_eq!(match_views(&reference,&candidate,&Delta::default()).unwrap_err(),
            "undeclared owner delta");
        candidate.owners = BTreeMap::from([("fake_b".into(),"schema-b".into())]);
        candidate.programs = vec![("shared::hostname".into(),HOSTNAME_SOURCE.into()),
            ("fake_b::StagedSchema0::Regex0".into(),"^x$".into())];
        assert_eq!(owner_delta(&reference.owners,&candidate.owners),
            Delta { added:BTreeSet::from(["fake_b".into()]),
                removed:BTreeSet::from(["fake_a".into()]), ..Delta::default() });
    }

    #[test]
    fn reviewed_real_catalog_delta_is_frozen_zero_before_any_observation() {
        let reference=view(true);
        let mut candidate=view(false);
        candidate.owners.insert("fake_b".into(),"unexpected schema".into());
        candidate.programs.push(("fake_b::StagedSchema1::Regex0".into(),"^new$".into()));
        let calls=Cell::new(0);
        let reviewed_delta=Delta::default();
        assert_eq!(observe_matched(&reference,&candidate,&reviewed_delta, |_| {
            calls.set(calls.get()+1);
            Ok(json!({}))
        }).unwrap_err(),"undeclared owner delta");
        assert_eq!(calls.get(),0);
    }

    #[test]
    fn each_measurement_context_change_refuses_before_observing() {
        let reference = view(true);
        for pointer in [
            "/build/target", "/build/profile", "/build/compiler_verbose", "/build/encoded_rustflags",
            "/build/enabled_features", "/environment/OPT_LEVEL", "/abi/pointer_width",
            "/measurement_lock_sha256", "/measurement_sources/fake~1current-generator.rs",
            "/regex_source", "/regex_features", "/generator_options", "/workload", "/non_catalog",
        ] {
            let mut candidate = view(false);
            *candidate.measurement.pointer_mut(pointer).unwrap() = json!("independent change");
            let calls = Cell::new(0);
            assert_eq!(observe_matched(&reference,&candidate,&Delta::default(), |_| {
                calls.set(calls.get()+1);
                Ok(json!({}))
            }).unwrap_err(), "measurement context mismatch", "{pointer}");
            assert_eq!(calls.get(),0);
        }
    }

    #[test]
    fn reference_pin_and_post_freeze_source_substitution_refuse() {
        for key in ["commit","tree"] {
            let mut reference = view(true);
            reference.source[key] = json!("wrong");
            assert_eq!(match_views(&reference,&view(false),&Delta::default()).unwrap_err(),
                "reference identity mismatch");
        }
        let reference = view(true);
        let candidate = view(false);
        let token = match_views(&reference,&candidate,&Delta::default()).unwrap();
        let mut changed = reference.clone();
        changed.source["files"]["Cargo.lock"] = json!("substituted bytes");
        assert_eq!(verify_frozen(&token,&changed,&candidate).unwrap_err(), "frozen inputs changed");
        verify_frozen(&token,&reference,&candidate).unwrap();
    }

    #[test]
    #[cfg(not(feature = "insecure-bidders"))]
    fn matched_catalog_growth_reconstructs_actual_reference_and_candidate() {
        let report = run_real().unwrap();
        assert_eq!(report["reference"]["schema_owner_count"],16);
        assert_eq!(report["reference"]["public_definition_count"],18);
        assert_eq!(report["physical_or_serving_qualification"],false);
        eprintln!("{}", serde_json::to_string(&report).unwrap());
    }

    #[test]
    #[cfg(feature = "insecure-bidders")]
    fn development_metadata_matched_catalog_growth_reconstructs_actual_reference_and_candidate() {
        let report = run_real().unwrap();
        assert_eq!(report["reference"]["schema_owner_count"],16);
        assert_eq!(report["reference"]["public_definition_count"],18);
        assert_eq!(report["common_C_I_qualified"],false);
        eprintln!("{}", serde_json::to_string(&report).unwrap());
    }
}

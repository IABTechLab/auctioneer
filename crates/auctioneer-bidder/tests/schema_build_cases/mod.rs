#[cfg(test)]
mod tests {
    use crate::schema_build::{
        self, RawVerdict, manifest, run, verify_compiler_metadata, verify_lock,
    };
    use crate::schema_heap_layout::HeapLayout;
    use crate::{schema_profile, schema_qualification, schema_reference};
    use flate2::read::GzDecoder;
    use serde_json::{Value as Json, json};
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::env;
    use std::fs;
    use std::io::{Read as _, Write as _, stderr};
    use std::iter;
    use std::path::{Component, Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;
    use toml::Value;

    const ROOT: &str = include_str!("../../../../Cargo.toml");
    const LOCK: &str = include_str!("../../../../Cargo.lock");
    const ARCHIVE_SHA256: &str = "bf3ebe067f2c25a981a7d1b2f696fc213e10dbf80ba5fe997f7dc2e4fe261f26";
    const TYPE_CHECK_SHA256: &str =
        "c5fe64170d5154c5967280d28c2fcb269c24d49ef756322d269ac8eb8998d912";
    const INITIALIZATION_SHA256: &str =
        "0fe3dfb41dd37dab3fa8fe7613fb20084b91550b100798da2c21adbcd21a6730";
    const EXPLICIT_CACHE_PATCH: &str = "jsonschema-macros-core-explicit-cache.patch";
    const EXPLICIT_CACHE_PATCH_SHA256: &str =
        "32d16bb697447d2158b0b40da829fc1e93d16527b2b7c1b42345ae5689a18993";

    // Source tables fix accepts/refusals; lexical inputs distinguish JSON
    // refusals from schema refusals independently of the generated validator.
    fn reference_case_counts() -> BTreeMap<&'static str, [usize; 3]> {
        schema_reference::verdict_counts()
    }

    #[test]
    fn captured_source_families_match_the_parameter_table_inventory() {
        let corpus = schema_reference::captured().unwrap();
        let inputs = schema_profile::inputs();
        let schemas: BTreeSet<_> = inputs
            .snapshot
            .schemas
            .iter()
            .map(|row| row.witness.name.as_str())
            .collect();
        for case in &corpus.cases {
            assert!(
                schemas.contains(case.schema.as_str()),
                "captured schema must exist in the linked inventory: {}",
                case.schema
            );
        }
        assert_eq!(
            corpus
                .cases
                .iter()
                .map(|case| case.schema.as_str())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>(),
            reference_case_counts().into_keys().collect::<Vec<_>>()
        );
        assert_eq!(
            schema_reference::source_objects()
                .map(|(schema, _)| schema)
                .collect::<Vec<_>>(),
            reference_case_counts().into_keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn compiled_explicit_cache_layouts_have_complete_typed_owner_membership() {
        use regex_automata::meta::Cache;
        use std::mem::{align_of, size_of};

        let catalog = schema_build::linked_explicit_catalog().unwrap();
        let layouts = catalog.native_cache_layouts().unwrap();
        assert_eq!(layouts.schema_version, 2);
        assert_eq!(layouts.word_bytes, size_of::<usize>());
        assert_eq!(layouts.cache_bytes, size_of::<Cache>());
        assert_eq!(layouts.cache_alignment, align_of::<Cache>());
        let inputs = schema_profile::inputs();
        assert_eq!(layouts.rows.len(), schema_profile::schema_count());
        assert_eq!(
            layouts
                .rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            inputs
                .snapshot
                .schemas
                .iter()
                .map(|row| row.witness.name.as_str())
                .collect::<Vec<_>>()
        );
        let mut helpers = 0_usize;
        let mut positive = 0_usize;
        let mut empty = 0_usize;
        for (layout, schema) in layouts.rows.iter().zip(&inputs.snapshot.schemas) {
            let patterns = schema
                .audit
                .patterns
                .iter()
                .map(|pattern| pattern.value.as_str())
                .collect::<BTreeSet<_>>()
                .len();
            assert_eq!(
                layout.inline_bytes,
                patterns.checked_mul(layouts.cache_bytes).unwrap()
            );
            assert_eq!(
                layout.alignment,
                if patterns == 0 {
                    1
                } else {
                    layouts.cache_alignment
                }
            );
            helpers = helpers.checked_add(patterns).unwrap();
            positive = positive.checked_add(usize::from(patterns > 0)).unwrap();
            empty = empty.checked_add(usize::from(patterns == 0)).unwrap();
        }
        assert_eq!(helpers, schema_profile::helper_count());
        assert!(positive > 0 && empty > 0);
        assert_eq!(
            positive.checked_add(empty),
            Some(schema_profile::schema_count())
        );
    }

    #[test]
    fn standalone_harness_preserves_profiles_and_exact_candidate_features() {
        let root: Value = toml::from_str(ROOT).unwrap();
        let generated: Value = toml::from_str(&manifest(&root).unwrap()).unwrap();
        assert_eq!(generated["profile"], root["profile"]);
        assert_eq!(
            generated["workspace"]["resolver"],
            root["workspace"]["resolver"]
        );
        let dependencies = generated["dependencies"].as_table().unwrap();
        assert_eq!(
            dependencies.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "jsonschema",
                "regex",
                "regex-automata",
                "serde_json",
                "uuid-simd"
            ]
        );
        for name in dependencies.keys() {
            assert_eq!(dependencies[name], root["workspace"]["dependencies"][name]);
        }
    }

    #[test]
    fn standalone_harness_uses_exactly_both_reviewed_source_patches() {
        let root: Value = toml::from_str(ROOT).unwrap();
        let generated: Value = toml::from_str(&manifest(&root).unwrap()).unwrap();
        let patches = generated["patch"]["crates-io"].as_table().unwrap();
        assert_eq!(patches.len(), 2);
        for name in ["jsonschema-macros-core", "regex-automata"] {
            let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../vendor")
                .join(name)
                .canonicalize()
                .unwrap();
            let declaration = patches[name].as_table().unwrap();
            assert_eq!(declaration.len(), 1);
            assert_eq!(declaration["path"].as_str(), expected.to_str());
        }
    }

    #[test]
    fn standalone_harness_rejects_missing_or_changed_patch_declarations() {
        let original: Value = toml::from_str(ROOT).unwrap();
        let mut missing = original.clone();
        assert!(missing.as_table_mut().unwrap().remove("patch").is_some());
        assert_eq!(manifest(&missing), Err("missing compiler patch".into()));
        for (field, value) in [("path", "unreviewed"), ("version", "0.58.6")] {
            let mut changed = original.clone();
            changed["patch"]["crates-io"]["jsonschema-macros-core"]
                .as_table_mut()
                .unwrap()
                .insert(field.into(), Value::String(value.into()));
            assert_eq!(
                manifest(&changed),
                Err("unreviewed compiler patch".into()),
                "{field}"
            );
        }
        let mut extra = original;
        extra["patch"]["crates-io"]
            .as_table_mut()
            .unwrap()
            .insert("regex".into(), Value::Table(toml::Table::new()));
        assert_eq!(manifest(&extra), Err("unreviewed compiler patch".into()));
    }

    #[test]
    fn standalone_harness_refuses_each_unreviewed_engine_declaration() {
        let original: Value = toml::from_str(ROOT).unwrap();
        for replacement in [
            None,
            Some(Value::String("vendor/regex-automata".into())),
            Some(toml::from_str::<Value>("path = 'unreviewed'").unwrap()),
            Some(
                toml::from_str::<Value>("path = 'vendor/regex-automata'\nversion = '0.4.18'")
                    .unwrap(),
            ),
            Some(
                toml::from_str::<Value>(
                    "path = 'vendor/regex-automata'\npackage = 'regex-automata'",
                )
                .unwrap(),
            ),
            Some(
                toml::from_str::<Value>("path = 'vendor/regex-automata'\noptional = true").unwrap(),
            ),
        ] {
            let mut changed = original.clone();
            let patches = changed["patch"]["crates-io"].as_table_mut().unwrap();
            assert!(patches.remove("regex-automata").is_some());
            if let Some(value) = &replacement {
                patches.insert("regex-automata".into(), value.clone());
            }
            assert_eq!(
                manifest(&changed),
                Err("unreviewed engine patch".into()),
                "{replacement:?}"
            );
        }
    }

    #[test]
    fn compiler_metadata_requires_the_exact_local_package_and_manifest() {
        let vendor = Path::new("/reviewed/compiler");
        let original = json!({"packages": [{
            "name": "jsonschema-macros-core", "version": "0.58.6",
            "source": null, "manifest_path": "/reviewed/compiler/Cargo.toml"
        }, {"name": "unrelated"}]});
        assert_eq!(
            verify_compiler_metadata(&serde_json::to_vec(&original).unwrap(), vendor),
            Ok(())
        );
        for (field, value) in [
            ("version", json!("0.58.5")),
            (
                "source",
                json!("registry+https://github.com/rust-lang/crates.io-index"),
            ),
            ("manifest_path", json!("/unreviewed/compiler/Cargo.toml")),
            ("manifest_path", json!("/reviewed/compiler/other.toml")),
        ] {
            let mut changed = original.clone();
            changed["packages"][0][field] = value;
            assert_eq!(
                verify_compiler_metadata(&serde_json::to_vec(&changed).unwrap(), vendor),
                Err("compiler metadata does not resolve the reviewed vendor".into()),
                "{field}"
            );
        }
        for field in ["version", "source", "manifest_path"] {
            let mut changed = original.clone();
            assert!(
                changed["packages"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove(field)
                    .is_some()
            );
            assert_eq!(
                verify_compiler_metadata(&serde_json::to_vec(&changed).unwrap(), vendor),
                Err("compiler metadata does not resolve the reviewed vendor".into()),
                "missing {field}"
            );
        }
        let mut duplicate = original.clone();
        duplicate["packages"]
            .as_array_mut()
            .unwrap()
            .push(original["packages"][0].clone());
        assert_eq!(
            verify_compiler_metadata(&serde_json::to_vec(&duplicate).unwrap(), vendor),
            Err("compiler metadata does not resolve the reviewed vendor".into())
        );
        assert_eq!(
            verify_compiler_metadata(
                &serde_json::to_vec(&json!({"packages": []})).unwrap(),
                vendor
            ),
            Err("missing patched compiler".into())
        );
    }

    fn published_compiler_files() -> BTreeMap<PathBuf, Vec<u8>> {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let raw = fs::read(workspace.join("vendor/jsonschema-macros-core-0.58.6.crate")).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&raw)), ARCHIVE_SHA256);
        let mut archive = tar::Archive::new(GzDecoder::new(raw.as_slice()));
        let mut files = BTreeMap::new();
        for entry_result in archive.entries().unwrap() {
            let mut entry = entry_result.unwrap();
            assert!(entry.header().entry_type().is_file());
            let relative = entry
                .path()
                .unwrap()
                .strip_prefix("jsonschema-macros-core-0.58.6")
                .unwrap()
                .to_owned();
            assert!(!relative.as_os_str().is_empty());
            assert!(
                relative
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
            );
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            assert!(files.insert(relative, bytes).is_none());
        }
        assert_eq!(files.len(), 280);
        files
    }

    fn compiler_tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        let mut tree = BTreeMap::new();
        let mut pending = vec![PathBuf::new()];
        while let Some(parent) = pending.pop() {
            for entry_result in fs::read_dir(root.join(&parent)).unwrap() {
                let entry = entry_result.unwrap();
                let relative = parent.join(entry.file_name());
                let metadata = fs::symlink_metadata(entry.path()).unwrap();
                if metadata.is_dir() {
                    assert!(tree.insert(relative.clone(), None).is_none());
                    pending.push(relative);
                } else {
                    assert!(metadata.is_file(), "{}", relative.display());
                    assert!(
                        tree.insert(relative, Some(fs::read(entry.path()).unwrap()))
                            .is_none()
                    );
                }
            }
        }
        tree
    }

    fn apply_portable_compiler_patch(directory: &Path, patch_name: &str) {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let patch = workspace.join("vendor/patches").join(patch_name);
        let checksum = match patch_name {
            "jsonschema-macros-core-type-check.patch" => {
                "1d1ed4cdfdd21abbd2c469bbee6f7aa3ea8079c736dd22b9abe5d08e76265358"
            }
            "jsonschema-macros-core-regex-initialization.patch" => {
                "a3a0e791aeedd859835cb97e8a36d9c5c6cdb1db523c6348a21f41d1876a65f2"
            }
            EXPLICIT_CACHE_PATCH => EXPLICIT_CACHE_PATCH_SHA256,
            _ => panic!("unreviewed portable patch: {patch_name}"),
        };
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(&patch).unwrap())),
            checksum
        );
        let original = compiler_tree(directory);
        for arguments in [&["apply", "--check"][..], &["apply"][..]] {
            let output = run(
                Command::new("git")
                    .env_remove("GIT_DIR")
                    .env_remove("GIT_WORK_TREE")
                    .env_remove("GIT_INDEX_FILE")
                    .current_dir(directory)
                    .args(arguments)
                    .arg(&patch),
                Duration::from_secs(10),
            )
            .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            if arguments.len() == 2 {
                assert!(
                    compiler_tree(directory) == original,
                    "git apply --check changed the compiler tree"
                );
            }
        }
    }

    fn assert_portable_patch_recreates_historical_file(
        file_name: &str,
        patch_name: &str,
        expected_sha256: &str,
    ) {
        let published = published_compiler_files();
        let original = &published[Path::new(file_name)];
        let temporary = tempfile::tempdir().unwrap();
        let file = temporary.path().join(file_name);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, original).unwrap();
        apply_portable_compiler_patch(temporary.path(), patch_name);
        assert_eq!(
            format!("{:x}", Sha256::digest(fs::read(file).unwrap())),
            expected_sha256
        );
    }

    #[test]
    fn portable_type_check_patch_recreates_the_reviewed_object_schema_file() {
        assert_portable_patch_recreates_historical_file(
            "src/codegen/object_schema.rs",
            "jsonschema-macros-core-type-check.patch",
            TYPE_CHECK_SHA256,
        );
    }

    #[test]
    fn portable_regex_initialization_patch_recreates_the_reviewed_emit_root_file() {
        assert_portable_patch_recreates_historical_file(
            "src/codegen/emit_root.rs",
            "jsonschema-macros-core-regex-initialization.patch",
            INITIALIZATION_SHA256,
        );
    }

    #[test]
    fn three_portable_patches_recreate_the_complete_reviewed_compiler_tree() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root: Value = toml::from_str(ROOT).unwrap();
        manifest(&root).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        for (relative, bytes) in published_compiler_files() {
            let path = temporary.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        // The separately pinned license is not part of any compiler patch.
        let license = fs::read(workspace.join("vendor/jsonschema-macros-core/LICENSE")).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&license)),
            "117829c3ca21efb132d81a44b55363d395ab8eea18526873bc828da4c0e5f038"
        );
        fs::write(temporary.path().join("LICENSE"), license).unwrap();
        for patch in [
            "jsonschema-macros-core-type-check.patch",
            "jsonschema-macros-core-regex-initialization.patch",
        ] {
            apply_portable_compiler_patch(temporary.path(), patch);
        }
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(
                    fs::read(temporary.path().join("src/codegen/emit_root.rs")).unwrap()
                )
            ),
            INITIALIZATION_SHA256
        );
        let two_patch_base = compiler_tree(temporary.path());
        apply_portable_compiler_patch(temporary.path(), EXPLICIT_CACHE_PATCH);
        let replayed = compiler_tree(temporary.path());
        assert!(
            replayed != two_patch_base,
            "third patch must change its base"
        );
        let reviewed = compiler_tree(&workspace.join("vendor/jsonschema-macros-core"));
        assert_eq!(
            replayed.keys().collect::<Vec<_>>(),
            reviewed.keys().collect::<Vec<_>>()
        );
        for (name, bytes) in replayed {
            assert!(
                reviewed.get(&name) == Some(&bytes),
                "replayed compiler entry differs: {}",
                name.display()
            );
        }
    }

    fn lock_with(packages: Vec<Value>) -> String {
        let mut root = toml::Table::new();
        root.insert("version".into(), Value::Integer(4));
        root.insert("package".into(), Value::Array(packages));
        toml::to_string(&root).unwrap()
    }

    fn harness_package() -> Value {
        let mut package = toml::Table::new();
        package.insert(
            "name".into(),
            Value::String("auctioneer-schema-link-probe".into()),
        );
        package.insert("version".into(), Value::String("0.0.0".into()));
        Value::Table(package)
    }

    fn compiler_package() -> Value {
        let workspace: Value = toml::from_str(LOCK).unwrap();
        workspace["package"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"].as_str() == Some("jsonschema-macros-core"))
            .unwrap()
            .clone()
    }

    fn engine_package() -> Value {
        toml::from_str("name = 'regex-automata'\nversion = '0.4.18'").unwrap()
    }

    #[test]
    fn child_lock_requires_the_engine_independently_of_the_compiler() {
        let compiler = compiler_package();
        let engine = engine_package();
        assert_eq!(
            verify_lock(
                &lock_with(vec![harness_package(), compiler.clone(), engine.clone()]),
                LOCK
            ),
            Ok(())
        );
        assert_eq!(
            verify_lock(&lock_with(vec![harness_package(), compiler.clone()]), LOCK),
            Err("missing patched engine".into())
        );
        for (field, value) in [
            ("version", "0.4.17"),
            (
                "source",
                "registry+https://github.com/rust-lang/crates.io-index",
            ),
            (
                "checksum",
                "ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2",
            ),
        ] {
            let mut changed = engine.clone();
            changed
                .as_table_mut()
                .unwrap()
                .insert(field.into(), Value::String(value.into()));
            assert_eq!(
                verify_lock(
                    &lock_with(vec![harness_package(), compiler.clone(), changed]),
                    LOCK
                ),
                Err("unreviewed patched engine".into()),
                "{field}"
            );
        }
        assert_eq!(
            verify_lock(
                &lock_with(vec![
                    harness_package(),
                    compiler.clone(),
                    engine.clone(),
                    engine.clone()
                ]),
                LOCK
            ),
            Err("duplicate child package".into())
        );
        let mut registry = engine.clone();
        registry.as_table_mut().unwrap().insert(
            "source".into(),
            Value::String("registry+https://github.com/rust-lang/crates.io-index".into()),
        );
        assert_eq!(
            verify_lock(
                &lock_with(vec![harness_package(), compiler, engine, registry]),
                LOCK
            ),
            Err("unreviewed patched engine".into())
        );
    }

    #[test]
    fn child_lock_accepts_only_exact_reviewed_registry_packages() {
        let workspace: Value = toml::from_str(LOCK).unwrap();
        let registry = workspace["package"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"].as_str() == Some("jsonschema"))
            .unwrap()
            .clone();
        assert_eq!(
            verify_lock(
                &lock_with(vec![
                    harness_package(),
                    compiler_package(),
                    engine_package(),
                    registry.clone()
                ]),
                LOCK
            ),
            Ok(())
        );
        for (field, replacement) in [
            ("version", "0.58.5"),
            (
                "checksum",
                "0000000000000000000000000000000000000000000000000000000000000000",
            ),
            ("source", "registry+https://unreviewed.example/index"),
        ] {
            let mut changed = registry.clone();
            changed[field] = Value::String(replacement.into());
            assert!(
                verify_lock(
                    &lock_with(vec![
                        harness_package(),
                        compiler_package(),
                        engine_package(),
                        changed
                    ]),
                    LOCK
                )
                .is_err(),
                "{field}"
            );
        }
        for packages in [
            vec![registry.clone()],
            vec![harness_package(), registry.clone(), registry.clone()],
            vec![harness_package(), harness_package(), registry],
        ] {
            assert!(verify_lock(&lock_with(packages), LOCK).is_err());
        }
    }

    #[test]
    fn child_lock_requires_exactly_the_reviewed_patched_compiler() {
        let compiler = compiler_package();
        let positive = vec![harness_package(), compiler.clone(), engine_package()];
        assert_eq!(verify_lock(&lock_with(positive), LOCK), Ok(()));
        for (field, value) in [
            ("version", "0.58.5"),
            ("name", "another-path-package"),
            (
                "source",
                "registry+https://github.com/rust-lang/crates.io-index",
            ),
            ("checksum", "unreviewed"),
        ] {
            let mut changed = compiler.clone();
            changed
                .as_table_mut()
                .unwrap()
                .insert(field.into(), Value::String(value.into()));
            assert!(
                verify_lock(
                    &lock_with(vec![harness_package(), changed, engine_package()]),
                    LOCK
                )
                .is_err(),
                "{field}"
            );
        }
        assert!(verify_lock(&lock_with(vec![harness_package(), engine_package()]), LOCK).is_err());
        assert!(
            verify_lock(
                &lock_with(vec![
                    harness_package(),
                    compiler.clone(),
                    compiler,
                    engine_package()
                ]),
                LOCK
            )
            .is_err()
        );
    }

    fn metric(row: &serde_json::Value, key: &str) -> u64 {
        row[key]
            .as_u64()
            .unwrap_or_else(|| panic!("missing {key}: {row}"))
    }

    fn assert_allocator_controls(control: &Json) {
        assert_eq!(
            control["positive"],
            json!({
                "allocated_bytes": 180_u64, "peak_live_bytes": 120_u64,
                "retained_bytes": 0_i64, "allocation_calls": 4_u64
            })
        );
        assert_eq!(
            control["retained"],
            json!({
                "allocated_bytes":113_u64,"peak_live_bytes":113_u64,"retained_bytes":113_i64,"allocation_calls":1_u64
            })
        );
        assert_eq!(
            control["release"],
            json!({
                "allocated_bytes":0_u64,"peak_live_bytes":0_u64,"retained_bytes":-113_i64,"allocation_calls":0_u64
            })
        );
        for name in ["outside", "other_thread"] {
            assert_eq!(
                control[name],
                json!({
                    "allocated_bytes": 0_u64, "peak_live_bytes": 0_u64,
                    "retained_bytes": 0_i64, "allocation_calls": 0_u64
                }),
                "{name}"
            );
        }
        assert_collection_layout_controls(&control["collection_layouts"]);
        assert_fresh_core_controls(&control["fresh_core"]);
        assert_fallback_controls(&control["lazy_fallbacks"]);
    }

    fn assert_fallback_controls(report: &Json) {
        use crate::schema_cache_bounds::forward_nfa;
        use crate::schema_fallback_heap::FallbackHeap;
        use std::mem::size_of;
        let rows = report.as_array().unwrap();
        assert_eq!(rows.len(), 3);
        for (row, (id, pattern)) in rows.iter().zip([
            ("alternates", "^(a|b)+z$"),
            ("captures", "^(a+)(b*)z$"),
            ("unicode", r"^\p{Greek}+z$"),
        ]) {
            assert_eq!(row["id"], id);
            assert_eq!(row["pattern"], pattern);
            let nfa = forward_nfa(pattern).unwrap();
            let heap =
                FallbackHeap::from_nfa(&nfa, HeapLayout::new(size_of::<usize>(), 16).unwrap(), 128)
                    .unwrap();
            for (name, retained_bound) in [
                (
                    "pike",
                    heap.pike_sets
                        .checked_add(heap.pike_slots)
                        .unwrap()
                        .checked_add(heap.pike_stack)
                        .unwrap(),
                ),
                (
                    "backtrack",
                    heap.backtrack_bitmap
                        .checked_add(heap.backtrack_stack)
                        .unwrap(),
                ),
            ] {
                let engine = &row[name];
                let searches = engine["searches"].as_array().unwrap();
                assert_eq!(searches.len(), 4);
                for (search, (phase, valid, input_bytes)) in searches.iter().zip([
                    ("empty", false, 0_usize),
                    ("short-match", true, 3_usize),
                    ("long-miss", false, 128_usize),
                    ("short-after-long", true, 3_usize),
                ]) {
                    assert_eq!(search["phase"], phase);
                    assert_eq!(search["valid"], valid);
                    assert_eq!(search["input_bytes"], input_bytes);
                }
                let live = assert_fallback_metrics(
                    engine,
                    u64::try_from(retained_bound).unwrap(),
                    u64::try_from(heap.peak).unwrap(),
                );
                assert_eq!(
                    searches[3]["metrics"]["allocated_bytes"], 0_u64,
                    "short search must reuse prior retained capacity"
                );
                assert!(searches[3]["reported_bytes"].as_u64().unwrap() < live);
            }
        }
    }

    fn assert_fallback_metrics(engine: &Json, retained_bound: u64, peak_bound: u64) -> u64 {
        let searches = engine["searches"].as_array().unwrap();
        let phases = iter::once(&engine["construction"])
            .chain(searches.iter().map(|search| &search["metrics"]))
            .chain(iter::once(&engine["release"]));
        let (mut allocated, mut calls, mut live, mut peak) = (0_u64, 0_u64, 0_i64, 0_i64);
        for phase in phases {
            let requested = metric(phase, "allocated_bytes");
            let events = metric(phase, "allocation_calls");
            let delta = phase["retained_bytes"].as_i64().unwrap();
            let local_peak = i64::try_from(metric(phase, "peak_live_bytes")).unwrap();
            let absolute_peak = live.checked_add(local_peak).unwrap();
            assert!(
                u64::try_from(absolute_peak).unwrap() <= peak_bound,
                "fallback phase peak exceeds envelope"
            );
            assert_eq!(
                requested == 0,
                events == 0,
                "fallback allocation calls disagree with requests"
            );
            assert!(
                u64::try_from(local_peak).unwrap() <= requested,
                "fallback phase peak exceeds requests"
            );
            assert!(
                local_peak >= delta.max(0),
                "fallback phase peak below retained growth"
            );
            allocated = allocated.checked_add(requested).unwrap();
            calls = calls.checked_add(events).unwrap();
            peak = peak.max(absolute_peak);
            live = live.checked_add(delta).unwrap();
            assert!(
                live >= 0 && u64::try_from(live).unwrap() <= retained_bound,
                "fallback retained storage exceeds envelope"
            );
        }
        assert!(
            allocated > 0 && calls > 0 && peak > 0,
            "fallback allocation work must be positive"
        );
        assert_eq!(live, 0_i64, "fallback cache must fully release");
        assert_eq!(
            engine["total"],
            json!({"allocated_bytes": allocated, "allocation_calls": calls,
                "retained_bytes": live, "peak_live_bytes": peak}),
            "fallback totals must reconcile"
        );
        u64::try_from(
            engine["release"]["retained_bytes"]
                .as_i64()
                .unwrap()
                .checked_neg()
                .unwrap(),
        )
        .unwrap()
    }

    fn fallback_history_fixture() -> Json {
        let phase = |allocated: u64, peak: i64, retained: i64, calls: u64| {
            json!({"allocated_bytes": allocated, "peak_live_bytes": peak,
                "retained_bytes": retained, "allocation_calls": calls})
        };
        json!({
            "construction": phase(10, 10, 10, 1),
            "searches": [
                {"metrics": phase(0, 0, 0, 0)},
                {"metrics": phase(4, 4, 4, 1)},
                {"metrics": phase(20, 20, 6, 2)},
                {"metrics": phase(0, 0, 0, 0)}
            ],
            "release": phase(0, 0, -20, 0),
            "total": phase(34, 34, 0, 4)
        })
    }

    #[test]
    fn fallback_metric_guard_accepts_a_reconciled_positive_history() {
        assert_eq!(
            assert_fallback_metrics(&fallback_history_fixture(), 24, 40),
            20
        );
    }

    #[test]
    #[should_panic(expected = "fallback phase peak exceeds envelope")]
    fn fallback_metric_guard_rejects_an_excessive_phase_hidden_by_summary() {
        let mut report = fallback_history_fixture();
        report["searches"][2]["metrics"]["peak_live_bytes"] = json!(41_i64);
        assert_fallback_metrics(&report, 24, 40);
    }

    #[test]
    #[should_panic(expected = "fallback allocation calls disagree with requests")]
    fn fallback_metric_guard_rejects_zero_phase_calls_hidden_by_summary() {
        let mut report = fallback_history_fixture();
        report["searches"][2]["metrics"]["allocation_calls"] = json!(0_u64);
        assert_fallback_metrics(&report, 24, 40);
    }

    #[test]
    #[should_panic(expected = "fallback totals must reconcile")]
    fn fallback_metric_guard_rejects_an_inconsistent_summary() {
        let mut report = fallback_history_fixture();
        report["total"]["allocation_calls"] = json!(5_u64);
        assert_fallback_metrics(&report, 24, 40);
    }

    #[test]
    #[should_panic(expected = "fallback phase peak exceeds requests")]
    fn fallback_metric_guard_rejects_erased_phase_requests_and_calls() {
        let mut report = fallback_history_fixture();
        report["searches"][2]["metrics"]["allocated_bytes"] = json!(0_u64);
        report["searches"][2]["metrics"]["allocation_calls"] = json!(0_u64);
        assert_fallback_metrics(&report, 24, 40);
    }

    #[test]
    #[should_panic(expected = "fallback allocation work must be positive")]
    fn fallback_metric_guard_rejects_empty_measurements_with_positive_summary() {
        let mut report = fallback_history_fixture();
        let zero = json!({"allocated_bytes": 0_u64, "peak_live_bytes": 0_i64,
            "retained_bytes": 0_i64, "allocation_calls": 0_u64});
        report["construction"] = zero.clone();
        report["release"] = zero.clone();
        for search in report["searches"].as_array_mut().unwrap() {
            search["metrics"] = zero.clone();
        }
        assert_fallback_metrics(&report, 24, 40);
    }

    #[test]
    #[should_panic(expected = "fallback retained storage exceeds envelope")]
    fn fallback_metric_guard_keeps_retained_and_peak_limits_independent() {
        assert_fallback_metrics(&fallback_history_fixture(), 19, 40);
    }

    fn assert_fresh_core_controls(report: &Json) {
        use std::mem::size_of;
        let word = size_of::<usize>();
        assert_eq!(report["word_bytes"], word);
        let rows = report["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 7);
        for (row, (id, slots, patterns, enabled, bytes32, bytes64, report_slots, calls)) in
            rows.iter().zip([
                (
                    "no-captures",
                    2_usize,
                    1_usize,
                    true,
                    8_i64,
                    16_i64,
                    0_usize,
                    1_u64,
                ),
                ("one-capture", 4, 1, true, 32, 64, 2, 2),
                ("two-captures", 6, 1, true, 40, 80, 4, 2),
                ("onepass-disabled", 6, 1, false, 24, 48, 0, 1),
                ("sixteen-captures", 34, 1, true, 264, 528, 32, 2),
                ("seventeen-captures", 36, 1, true, 144, 288, 0, 1),
                ("two-patterns", 10, 2, true, 64, 128, 6, 2),
            ])
        {
            assert_eq!(row["id"], id);
            assert_eq!(row["slots"], slots);
            assert_eq!(row["patterns"], patterns);
            assert_eq!(row["onepass_enabled"], enabled);
            assert_eq!(
                row["reported_bytes"],
                report_slots.checked_mul(word).unwrap()
            );
            let bytes = if word == 8 { bytes64 } else { bytes32 };
            assert_eq!(
                row["construction"],
                json!({"allocated_bytes": bytes, "peak_live_bytes": bytes,
                    "retained_bytes": bytes, "allocation_calls": calls}),
                "fresh Core construction: {id}"
            );
            assert_eq!(
                row["release"],
                json!({"allocated_bytes": 0_u64, "peak_live_bytes": 0_i64,
                    "retained_bytes": bytes.checked_neg().unwrap(), "allocation_calls": 0_u64}),
                "fresh Core release: {id}"
            );
        }
        let pike = &report["standalone_pike"];
        assert!(metric(&pike["construction"], "allocation_calls") > 0);
        assert!(metric(&pike["construction"], "retained_bytes") > 0);
        assert!(metric(pike, "reported_bytes") > 0);
        assert_eq!(
            pike["release"]["retained_bytes"].as_i64(),
            pike["construction"]["retained_bytes"]
                .as_i64()
                .unwrap()
                .checked_neg()
        );
        for phase in ["construction", "release"] {
            assert_eq!(
                report["standalone_backtrack"][phase],
                json!({"allocated_bytes": 0_u64, "peak_live_bytes": 0_i64,
                    "retained_bytes": 0_i64, "allocation_calls": 0_u64})
            );
        }
    }

    fn assert_vector_activity(observation: &Json, count: usize) {
        assert_eq!(observation["length"], count, "vector length");
        if count > 0 {
            assert!(metric(observation, "capacity") >= u64::try_from(count).unwrap());
            assert!(
                metric(&observation["construction"], "allocation_calls") > 0
                    && metric(&observation["construction"], "allocated_bytes") > 0,
                "positive vector allocation"
            );
        }
    }

    fn assert_map_repopulation(observation: &Json, count: usize) {
        assert_eq!(
            observation["length_after_repeat"], count,
            "map repopulation length"
        );
    }

    fn assert_arc_ownership(observation: &Json) {
        assert_eq!(
            observation["strong_after_clone"], 3_usize,
            "Arc clone ownership"
        );
        assert_eq!(
            observation["strong_after_drop"], 1_usize,
            "Arc drop ownership"
        );
    }

    #[test]
    #[should_panic(expected = "map repopulation length")]
    fn native_collection_guard_rejects_missing_map_repopulation() {
        assert_map_repopulation(&json!({"length_after_repeat": 0_usize}), 3);
    }

    #[test]
    #[should_panic(expected = "Arc clone ownership")]
    fn native_collection_guard_rejects_missing_arc_clones() {
        assert_arc_ownership(&json!({"strong_after_clone": 1_usize, "strong_after_drop": 1_usize}));
    }

    #[test]
    #[should_panic(expected = "Arc drop ownership")]
    fn native_collection_guard_rejects_missing_arc_drops() {
        assert_arc_ownership(&json!({"strong_after_clone": 3_usize, "strong_after_drop": 3_usize}));
    }

    #[test]
    fn native_collection_guard_accepts_repopulation_and_shared_ownership() {
        assert_map_repopulation(&json!({"length_after_repeat": 3_usize}), 3);
        assert_map_repopulation(&json!({"length_after_repeat": 0_usize}), 0);
        assert_arc_ownership(&json!({"strong_after_clone": 3_usize, "strong_after_drop": 1_usize}));
    }

    #[test]
    #[should_panic(expected = "vector length")]
    fn native_vector_guard_rejects_reported_work_without_insertions() {
        assert_vector_activity(
            &json!({"length": 0_usize, "capacity": 0_usize,
            "construction": {"allocation_calls": 0_u64, "allocated_bytes": 0_u64}}),
            1,
        );
    }

    #[test]
    #[should_panic(expected = "positive vector allocation")]
    fn native_vector_guard_rejects_missing_positive_allocation_calls() {
        assert_vector_activity(
            &json!({"length": 1_usize, "capacity": 8_usize,
            "construction": {"allocation_calls": 0_u64, "allocated_bytes": 8_u64}}),
            1,
        );
    }

    #[test]
    #[should_panic(expected = "positive vector allocation")]
    fn native_vector_guard_rejects_missing_positive_requested_bytes() {
        assert_vector_activity(
            &json!({"length": 1_usize, "capacity": 8_usize,
            "construction": {"allocation_calls": 1_u64, "allocated_bytes": 0_u64}}),
            1,
        );
    }

    #[test]
    fn native_vector_guard_accepts_nonvacuous_positive_and_empty_controls() {
        assert_vector_activity(
            &json!({"length": 1_usize, "capacity": 8_usize,
            "construction": {"allocation_calls": 1_u64, "allocated_bytes": 8_u64}}),
            1,
        );
        assert_vector_activity(
            &json!({"length": 0_usize, "capacity": 0_usize,
            "construction": {"allocation_calls": 0_u64, "allocated_bytes": 0_u64}}),
            0,
        );
    }

    fn assert_collection_layout_controls(report: &Json) {
        use std::mem::size_of;

        let word = size_of::<usize>();
        let layout = HeapLayout::new(word, 16).unwrap();
        assert_eq!(report["word_bytes"], word);
        assert_eq!(
            report["pair_bytes"],
            if word == 8 { 24_usize } else { 12_usize }
        );
        assert_eq!(report["pair_align"], word);
        assert_vector_layouts(&report["vectors"], layout);
        assert_map_layouts(&report["state_maps"], layout);
        assert_arc_layouts(&report["arc_slices"], layout);
    }

    fn assert_vector_layouts(rows: &Json, layout: HeapLayout) {
        let word = layout.word_bytes();
        let vectors = rows.as_array().unwrap();
        assert_eq!(vectors.len(), 30);
        let expected = [
            ("byte", 1_usize),
            ("id", 4),
            ("handle", word.checked_mul(2).unwrap()),
        ]
        .into_iter()
        .flat_map(|(kind, element)| {
            [0_usize, 1, 3, 4, 5, 8, 9, 16, 17, 65]
                .into_iter()
                .map(move |count| (kind, element, count))
        });
        for (observation, (kind, element, count)) in vectors.iter().zip(expected) {
            assert_eq!(observation["kind"], kind);
            assert_eq!(observation["high_water"], count);
            assert_vector_activity(observation, count);
            assert_eq!(observation["element_bytes"], element);
            let envelope = layout.append_vector(element, count).unwrap();
            let retained = metric(&observation["construction"], "retained_bytes");
            assert_eq!(
                retained,
                metric(observation, "capacity")
                    .checked_mul(u64::try_from(element).unwrap())
                    .unwrap()
            );
            assert!(retained <= u64::try_from(envelope.retained_bytes).unwrap());
            assert!(
                metric(&observation["construction"], "peak_live_bytes")
                    <= u64::try_from(
                        envelope
                            .retained_bytes
                            .checked_add(envelope.old_growth_bytes)
                            .unwrap()
                    )
                    .unwrap()
            );
            assert_eq!(
                observation["release"]["retained_bytes"].as_i64(),
                Some(i64::try_from(retained).unwrap().checked_neg().unwrap())
            );
        }
    }

    fn assert_map_layouts(rows: &Json, layout: HeapLayout) {
        let maps = rows.as_array().unwrap();
        assert_eq!(maps.len(), 11);
        let mut grown = BTreeSet::new();
        for (observation, count) in maps
            .iter()
            .zip([0_usize, 1, 3, 7, 14, 15, 28, 29, 56, 57, 128])
        {
            assert_eq!(observation["high_water"], count);
            assert_eq!(observation["length_after_replace"], count);
            assert_eq!(observation["length_after_clear"], 0_usize);
            assert_map_repopulation(observation, count);
            assert_eq!(
                observation["capacity_after_clear"],
                observation["capacity_after_replace"]
            );
            let envelope = layout.state_map(count).unwrap();
            let before = metric(&observation["construction"], "retained_bytes");
            let replacement = &observation["replacement"];
            let retained = before
                .checked_add(metric(replacement, "retained_bytes"))
                .unwrap();
            if metric(replacement, "allocation_calls") > 0 {
                assert!(grown.insert(count));
            }
            assert!(retained <= u64::try_from(envelope.retained_bytes).unwrap());
            let peak = metric(&observation["construction"], "peak_live_bytes").max(
                before
                    .checked_add(metric(replacement, "peak_live_bytes"))
                    .unwrap(),
            );
            assert!(
                peak <= u64::try_from(
                    envelope
                        .retained_bytes
                        .checked_add(envelope.old_growth_bytes)
                        .unwrap()
                )
                .unwrap()
            );
            for phase in ["clear", "repeat"] {
                assert_eq!(
                    observation[phase],
                    json!({"allocated_bytes":0_u64, "peak_live_bytes":0_u64, "retained_bytes":0_i64, "allocation_calls":0_u64})
                );
            }
            assert_eq!(
                observation["release"]["retained_bytes"].as_i64(),
                Some(i64::try_from(retained).unwrap().checked_neg().unwrap())
            );
        }
        assert_eq!(grown, BTreeSet::from([3, 7, 14, 28, 56]));
    }

    fn assert_arc_layouts(rows: &Json, layout: HeapLayout) {
        let arcs = rows.as_array().unwrap();
        assert_eq!(arcs.len(), 13);
        for (observation, count) in arcs
            .iter()
            .zip([0_usize, 1, 4, 7, 8, 9, 15, 16, 17, 31, 32, 33, 255])
        {
            assert_eq!(observation["payload_bytes"], count);
            assert_arc_ownership(observation);
            let expected_bytes = layout.arc_slice(count).unwrap();
            let expected_i64 = i64::try_from(expected_bytes).unwrap();
            assert_eq!(
                observation["construction"],
                json!({
                    "allocated_bytes": expected_bytes, "peak_live_bytes": expected_bytes,
                    "retained_bytes": expected_bytes, "allocation_calls": 1_u64,
                })
            );
            for phase in ["clone", "drop_clones"] {
                assert_eq!(
                    observation[phase],
                    json!({"allocated_bytes":0_u64, "peak_live_bytes":0_u64, "retained_bytes":0_i64, "allocation_calls":0_u64})
                );
            }
            assert_eq!(
                observation["release"],
                json!({"allocated_bytes":0_u64, "peak_live_bytes":0_u64, "retained_bytes":expected_i64.checked_neg().unwrap(), "allocation_calls":0_u64})
            );
        }
    }

    fn assert_dispatch_observations(catalog: &schema_build::Catalog) {
        let dispatch = catalog.native_allocations("dispatch").unwrap();
        assert_eq!(
            dispatch["scope"],
            "empty-object dispatch only; not cache warming"
        );
        let rows = dispatch["rows"].as_array().unwrap();
        assert_eq!(rows.len(), schema_profile::schema_count());
        let native = catalog
            .reports()
            .iter()
            .find(|row| row.target == "native")
            .unwrap();
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row["name"], native.names[index]);
            assert_eq!(row["cold"]["valid"], native.results[index]);
            assert_eq!(row["cold"]["valid"], row["repeat"]["valid"]);
            for pass in ["cold", "repeat"] {
                let metrics = &row[pass]["metrics"];
                let _allocated = metric(metrics, "allocated_bytes");
                let _calls = metric(metrics, "allocation_calls");
                assert!(metrics["retained_bytes"].as_i64().is_some());
                assert!(metric(metrics, "peak_live_bytes") <= 512 * 1024);
            }
        }
    }

    fn assert_sampled_catalog_observations(catalog: &schema_build::Catalog) {
        let sampled = catalog.native_allocations("sampled-catalog").unwrap();
        assert_eq!(
            sampled["dispatch_callbacks"],
            schema_profile::schema_count()
        );
        assert_eq!(sampled["callback_inputs"], schema_profile::sampled_count());
        assert_eq!(sampled["all_results_match"], true);
        // Gross first-pass allocation is a conservative upper bound on the
        // sampled initialization scratch, not a disjoint catalog derivation.
        assert!(
            metric(&sampled["first_pass"], "allocated_bytes") <= 8 * 1024 * 1024,
            "{sampled}"
        );
        assert!(
            metric(&sampled["first_pass"], "retained_bytes") <= 12 * 1024 * 1024,
            "{sampled}"
        );
        assert!(
            metric(&sampled["repeat"], "peak_live_bytes") <= 512 * 1024,
            "{sampled}"
        );
        writeln!(stderr(), "native sampled catalog: {sampled}").unwrap();
    }

    fn assert_workload_bounds(name: &str, cases: &[Json]) {
        assert!(cases.len() >= 4, "{name}");
        assert!(cases.iter().any(|row| row["expected"] == false), "{name}");
        assert!(cases.iter().any(|row| row["expected"] == true), "{name}");
        assert!(
            cases
                .iter()
                .any(|row| metric(row, "input_bytes") == 0x0001_0000)
                || schema_profile::workloads()[name]["maximum_string"] != true,
            "{name}"
        );
        if schema_profile::workloads()[name]["maximum_collections"] == true {
            for id in ["max-items", "max-properties", "max-properties-invalid-tail"] {
                let row = cases.iter().find(|row| row["id"] == id).unwrap();
                assert_eq!(metric(row, "input_nodes"), 4096, "{name}: {id}");
                assert_eq!(row["expected"], id != "max-properties-invalid-tail");
            }
        }
        if schema_profile::workloads()[name]["string_refusals"] == true {
            for (id, valid) in [
                ("max-string", true),
                ("max-string-invalid-tail", false),
                ("unicode", false),
            ] {
                let row = cases.iter().find(|row| row["id"] == id).unwrap();
                assert_eq!(row["expected"], valid);
                if id != "unicode" {
                    assert_eq!(metric(row, "input_bytes"), 0x0001_0000);
                }
            }
        }
    }

    fn assert_callback_observations(name: &str, cases: &[Json], report: &Json) {
        let mut cold_bytes = 0_u64;
        let mut repeat_bytes = 0_u64;
        let mut cold_calls = 0_u64;
        let mut repeat_calls = 0_u64;
        for row in cases {
            assert!(metric(row, "input_bytes") <= 0x0001_0000, "{row}");
            assert!(metric(row, "input_nodes") <= 4096, "{row}");
            assert!(metric(row, "input_depth") <= 32, "{row}");
            assert_eq!(row["repeat"]["valid"], row["expected"], "{name}: {row}");
            assert_eq!(row["first_pass"]["valid"], row["expected"], "{name}: {row}");
            let cold = &row["first_pass"]["metrics"];
            let repeat = &row["repeat"]["metrics"];
            // Requested live heap, not System allocator internals or physical RSS.
            assert!(
                metric(repeat, "peak_live_bytes") <= 512 * 1024,
                "{name}: {row}"
            );
            assert_eq!(repeat["retained_bytes"], 0_i64, "repeat grew cache: {row}");
            if row != &cases[0] {
                assert!(
                    metric(cold, "peak_live_bytes") <= 512 * 1024,
                    "changed-input warm scratch: {name}: {row}"
                );
            }
            cold_bytes = cold_bytes
                .checked_add(metric(cold, "allocated_bytes"))
                .unwrap();
            repeat_bytes = repeat_bytes
                .checked_add(metric(repeat, "allocated_bytes"))
                .unwrap();
            cold_calls = cold_calls
                .checked_add(metric(cold, "allocation_calls"))
                .unwrap();
            repeat_calls = repeat_calls
                .checked_add(metric(repeat, "allocation_calls"))
                .unwrap();
        }
        assert!(
            cases
                .iter()
                .skip(1)
                .any(|row| row["input_bytes"] != cases[0]["input_bytes"])
        );
        if schema_profile::workloads()[name]["pattern_initialization"] == true {
            assert!(
                cold_bytes > repeat_bytes && cold_calls > repeat_calls,
                "pattern initialization control was vacuous: {report}"
            );
            assert!(
                metric(&report["cold_first_use"], "allocated_bytes")
                    > metric(&cases[0]["repeat"]["metrics"], "allocated_bytes")
            );
        }
        if schema_profile::workloads()[name]["fractional_allocation"] == true {
            let fractional = cases.iter().find(|row| row["id"] == "fractional").unwrap();
            let integer = cases.iter().find(|row| row["id"] == "integer").unwrap();
            assert!(metric(&fractional["repeat"]["metrics"], "allocated_bytes") > 0);
            assert!(metric(&fractional["repeat"]["metrics"], "allocation_calls") > 0);
            assert_eq!(metric(&integer["repeat"]["metrics"], "allocated_bytes"), 0);
        }
    }

    fn constructor_total(rows: &[Json], formats: &Json, pass: &str) -> Json {
        let (mut allocated, mut calls, mut live, mut peak) = (0_u64, 0_u64, 0_i64, 0_i64);
        for owner in iter::once(formats).chain(rows) {
            let values = &owner[pass]["metrics"];
            peak = peak.max(
                live.checked_add(values["peak_live_bytes"].as_i64().unwrap())
                    .unwrap(),
            );
            live = live
                .checked_add(values["retained_bytes"].as_i64().unwrap())
                .unwrap();
            allocated = allocated
                .checked_add(metric(values, "allocated_bytes"))
                .unwrap();
            calls = calls
                .checked_add(metric(values, "allocation_calls"))
                .unwrap();
        }
        json!({"allocated_bytes":allocated,"allocation_calls":calls,"peak_live_bytes":peak,"retained_bytes":live})
    }

    fn assert_initialized_catalog_observations(catalog: &schema_build::Catalog) -> Json {
        let initialized = catalog.native_allocations("initialized-catalog").unwrap();
        assert_eq!(
            initialized["scope"],
            "explicit constructors then sampled validation; not complete cache bounds"
        );
        let rows = initialized["constructors"]["schemas"].as_array().unwrap();
        let native = catalog
            .reports()
            .iter()
            .find(|row| row.target == "native")
            .unwrap();
        assert_eq!(rows.len(), schema_profile::schema_count());
        for (row, name) in rows.iter().zip(&native.names) {
            assert_eq!(row["name"], *name);
            assert_eq!(row["first"]["initialized"], true);
            assert_eq!(row["repeat"]["initialized"], true);
            assert_eq!(
                row["repeat"]["metrics"],
                json!({
                    "allocated_bytes": 0_u64, "peak_live_bytes": 0_i64,
                    "retained_bytes": 0_i64, "allocation_calls": 0_u64
                })
            );
        }
        let format = &initialized["constructors"]["formats"];
        assert_eq!(format["first"]["initialized"], true);
        assert_eq!(format["repeat"]["initialized"], true);
        assert!(metric(&format["first"]["metrics"], "retained_bytes") > 0);
        assert_eq!(
            format["repeat"]["metrics"],
            json!({
                "allocated_bytes": 0_u64, "peak_live_bytes": 0_i64,
                "retained_bytes": 0_i64, "allocation_calls": 0_u64
            })
        );
        for (pass, total) in [("first", "first_pass"), ("repeat", "repeat")] {
            assert_eq!(
                initialized["constructors"][total],
                constructor_total(rows, format, pass)
            );
        }
        for (name, _) in schema_profile::workloads()
            .iter()
            .filter(|(_, data)| data["retained_constructor"] == true)
        {
            let row = rows
                .iter()
                .find(|row| row["name"] == name.as_str())
                .unwrap();
            assert!(
                metric(&row["first"]["metrics"], "retained_bytes") > 0,
                "{name}: {row}"
            );
        }
        assert_eq!(
            initialized["callback_inputs"],
            schema_profile::sampled_count()
        );
        let outcomes = initialized["outcomes"].as_array().unwrap();
        assert_eq!(outcomes.len(), schema_profile::sampled_count());
        for (index, name) in native.names.iter().enumerate() {
            assert_eq!(
                outcomes[index],
                json!({
                    "name": name, "id": "empty",
                    "first_valid": native.results[index], "repeat_valid": native.results[index]
                })
            );
        }
        assert_eq!(initialized["all_results_match"], true);
        assert!(metric(&initialized["constructors"]["first_pass"], "retained_bytes") > 0);
        assert!(
            metric(&initialized["constructors"]["first_pass"], "retained_bytes")
                <= 12 * 1024 * 1024
        );
        // Necessary joint observation only: do not derive disjoint scratch by
        // subtracting the final retained value from an earlier peak.
        assert!(
            metric(
                &initialized["constructors"]["first_pass"],
                "peak_live_bytes"
            ) <= 20 * 1024 * 1024
        );
        assert!(metric(&initialized["repeat"], "allocated_bytes") > 0);
        writeln!(
            stderr(),
            "explicit initialization: {}",
            initialized["constructors"]["first_pass"]
        )
        .unwrap();
        writeln!(
            stderr(),
            "validation after initialization: first={} repeat={}",
            initialized["first_pass"],
            initialized["repeat"]
        )
        .unwrap();
        initialized
    }

    #[test]
    fn native_generated_callbacks_have_isolated_cold_and_repeat_allocation_reports() {
        let catalog = schema_build::linked_catalog().unwrap();
        let controls = catalog.native_allocation_output("controls").unwrap();
        assert_eq!(
            String::from_utf8(controls.stderr).unwrap(),
            concat!(
                "allocation phase: entered\n",
                "allocation phase: positive-complete\n",
                "allocation phase: retained-complete\n",
                "allocation phase: release-complete\n",
                "allocation phase: outside-complete\n",
                "allocation phase: thread-spawned\n",
                "allocation phase: thread-complete\n",
                "allocation phase: complete\n"
            )
        );
        assert_allocator_controls(&serde_json::from_slice(&controls.stdout).unwrap());
        let initialized = assert_initialized_catalog_observations(&catalog);
        assert_dispatch_observations(&catalog);
        assert_sampled_catalog_observations(&catalog);
        for name in schema_profile::workloads().keys() {
            // Each invocation starts a new process, including each hostname user.
            let report = catalog.native_allocations(name).unwrap();
            assert_eq!(
                report["scope"],
                "named workload first use and same-input repeat"
            );
            assert_eq!(report["name"].as_str(), Some(name.as_str()));
            let cases = report["rows"].as_array().unwrap();
            assert_workload_bounds(name, cases);
            assert_callback_observations(name, cases, &report);
            let outcomes: Vec<_> = initialized["outcomes"].as_array().unwrap()
                [schema_profile::schema_count()..]
                .iter()
                .filter(|row| row["name"].as_str() == Some(name.as_str()))
                .collect();
            assert_eq!(outcomes.len(), cases.len(), "{name}");
            for (outcome, case) in outcomes.iter().zip(cases) {
                assert_eq!(
                    **outcome,
                    json!({
                        "name": name, "id": case["id"],
                        "first_valid": case["first_pass"]["valid"],
                        "repeat_valid": case["repeat"]["valid"]
                    })
                );
            }
            writeln!(
                stderr(),
                "native allocations {name}: cold={} changed-inputs={} repeat={}",
                report["cold_first_use"],
                report["changed_inputs"],
                report["repeat"]
            )
            .unwrap();
        }
    }

    #[test]
    fn explicit_cache_measurements_separate_construction_growth_reuse_and_release() {
        let catalog = schema_build::linked_explicit_catalog().unwrap();
        assert_allocator_controls(&catalog.native_allocations("controls").unwrap());
        let report = catalog.native_allocations("explicit-caches").unwrap();
        assert_eq!(
            report["scope"],
            "native requested heap; explicit probe owners, not production bounds"
        );
        let native = catalog
            .reports()
            .iter()
            .find(|row| row.target == "native")
            .unwrap();
        let schemas = report["schemas"].as_array().unwrap();
        assert_eq!(native.names.len(), schema_profile::schema_count());
        assert_eq!(native.results.len(), schema_profile::schema_count());
        assert_eq!(schemas.len(), schema_profile::schema_count());
        assert!(metric(&report, "owner_inline_bytes") > 0);
        let constructors = &report["constructors"];
        let constructor_rows = assert_explicit_constructors(constructors);
        let (mut sampled, mut searches, mut growth) = (0_usize, 0_usize, 0_u64);
        for ((row, constructor), (name, empty_valid)) in schemas
            .iter()
            .zip(constructor_rows)
            .zip(native.names.iter().zip(&native.results))
        {
            assert_eq!(row["name"], *name);
            assert_eq!(constructor["name"], *name);
            assert_eq!(constructor["first"]["initialized"], true);
            assert_eq!(constructor["repeat"]["initialized"], true);
            let populations = row["populations"].as_array().unwrap();
            assert_eq!(populations.len(), 3);
            for (population, owners) in populations.iter().zip([1, 2, 4]) {
                let (checked, allocated) =
                    assert_explicit_population(name, *empty_valid, population, owners);
                searches = searches.checked_add(checked).unwrap();
                growth = growth.checked_add(allocated).unwrap();
            }
            sampled = sampled
                .checked_add(usize::from(
                    populations[0]["searches"].as_array().unwrap().len() > 1,
                ))
                .unwrap();
        }
        assert_eq!(
            sampled,
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                11
            } else {
                2
            }
        );
        assert_eq!(searches, schema_profile::sampled_count() * 14);
        assert!(growth > 0, "first-search allocation positive control");
        assert_hostname_observations(&report);
        writeln!(stderr(), "explicit allocations: constructors={} inline-owner-bytes={} schemas={} populations=1,2,4 searches={searches} search-allocated-bytes={growth} hostname={}",
            constructors["first_pass"], report["owner_inline_bytes"], schema_profile::schema_count(), report["hostname"]
        ).unwrap();
    }

    fn assert_explicit_constructors(constructors: &Json) -> &[Json] {
        let rows = constructors["schemas"].as_array().unwrap();
        assert_eq!(rows.len(), schema_profile::schema_count());
        for (pass, total) in [("first", "first_pass"), ("repeat", "repeat")] {
            assert_eq!(
                constructors[total],
                constructor_total(rows, &constructors["formats"], pass)
            );
        }
        assert_eq!(metric(&constructors["repeat"], "allocated_bytes"), 0);
        assert_eq!(constructors["repeat"]["retained_bytes"], 0_i64);
        assert!(metric(&constructors["first_pass"], "allocated_bytes") > 0);
        assert_eq!(constructors["formats"]["first"]["initialized"], true);
        assert_eq!(constructors["formats"]["repeat"]["initialized"], true);
        rows
    }

    fn assert_hostname_observations(report: &Json) {
        let hostname = report["hostname"]["searches"].as_array().unwrap();
        assert_eq!(hostname.len(), 5);
        assert_eq!(hostname[1]["input_bytes"], 255_u64);
        assert_eq!(hostname[4]["input_bytes"], 256_u64);
        for (row, (id, expected)) in hostname.iter().zip([
            ("short", true),
            ("max-host", true),
            ("invalid-tail", false),
            ("overlong-label", false),
            ("overlong-host", false),
        ]) {
            assert_eq!(row["id"], id);
            for phase in ["cold", "warm"] {
                assert_eq!(row[phase]["valid"], expected);
                let metrics = &row[phase]["metrics"];
                assert_eq!(metrics["retained_bytes"], 0_i64);
                if id == "overlong-host" {
                    assert_eq!(metrics["allocated_bytes"], 0_u64);
                    assert_eq!(metrics["allocation_calls"], 0_u64);
                    assert_eq!(metrics["peak_live_bytes"], 0_i64);
                } else {
                    assert!(metric(metrics, "allocated_bytes") > 0);
                    assert!(metric(metrics, "allocation_calls") > 0);
                    assert!(metrics["peak_live_bytes"].as_i64().unwrap() > 0);
                }
            }
        }
    }

    fn explicit_input_count(name: &str) -> usize {
        1_usize
            .checked_add(
                schema_profile::workloads()
                    .get(name)
                    .map_or(0, |data| data["rows"].as_array().unwrap().len()),
            )
            .unwrap()
    }

    fn assert_explicit_population(
        name: &str,
        empty_valid: bool,
        population: &Json,
        owners: usize,
    ) -> (usize, u64) {
        assert_eq!(population["owners"], owners);
        let construction = population["construction"].as_array().unwrap();
        let release = population["release"].as_array().unwrap();
        assert_eq!(construction.len(), owners);
        assert_eq!(release.len(), owners);
        let mut retained = 0_i64;
        for row in construction {
            assert_eq!(
                row, &construction[0],
                "independent factory population: {name}"
            );
            retained = retained
                .checked_add(row["retained_bytes"].as_i64().unwrap())
                .unwrap();
        }
        let rows = population["searches"].as_array().unwrap();
        assert_eq!(rows.len(), explicit_input_count(name), "{name}");
        if rows.len() > 1 {
            assert_workload_bounds(name, &rows[1..]);
        }
        let (mut checked, mut allocated) = (0_usize, 0_u64);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row["phase"],
                if index == 0 {
                    "first-search"
                } else {
                    "changed-input"
                }
            );
            if index == 0 {
                assert_eq!(row["id"], "empty");
                assert_eq!(row["input_bytes"], 2_u64);
                assert_eq!(row["input_nodes"], 1_u64);
                assert!(row["expected"].is_null());
            }
            let expected = if index == 0 {
                empty_valid
            } else {
                row["expected"].as_bool().unwrap()
            };
            for phase in ["cold", "warm"] {
                let evaluations = row[phase].as_array().unwrap();
                assert_eq!(evaluations.len(), owners);
                for evaluation in evaluations {
                    assert_eq!(
                        evaluation["valid"], expected,
                        "{name}: {}: {phase}",
                        row["id"]
                    );
                    retained = retained
                        .checked_add(evaluation["metrics"]["retained_bytes"].as_i64().unwrap())
                        .unwrap();
                    allocated = allocated
                        .checked_add(metric(&evaluation["metrics"], "allocated_bytes"))
                        .unwrap();
                    checked = checked.checked_add(1).unwrap();
                }
            }
        }
        for metrics in release {
            assert_eq!(metric(metrics, "allocated_bytes"), 0);
            assert_eq!(metric(metrics, "allocation_calls"), 0);
            retained = retained
                .checked_add(metrics["retained_bytes"].as_i64().unwrap())
                .unwrap();
        }
        assert_eq!(
            retained, 0,
            "bundle lifetime did not balance: {name}/{owners}"
        );
        (checked, allocated)
    }

    #[test]
    fn all_schema_dispatch_links_on_native_and_three_wasm_targets() {
        let catalog = schema_build::linked_catalog().unwrap();
        let by_target: BTreeMap<_, _> = catalog
            .reports()
            .iter()
            .map(|row| (row.target.as_str(), row))
            .collect();
        assert_eq!(
            by_target.keys().copied().collect::<Vec<_>>(),
            [
                "native",
                "wasm32-unknown-unknown",
                "wasm32-wasip1",
                "wasm32-wasip2"
            ]
        );
        for row in catalog.reports() {
            assert_eq!(row.validators, schema_profile::schema_count());
            assert!(row.artifact_bytes > 0);
            assert_eq!(row.sha256.len(), 64);
            assert_guest_observations(row);
            writeln!(
                stderr(),
                "{}: validators={} bytes={} sha256={}",
                row.target,
                row.validators,
                row.artifact_bytes,
                row.sha256
            )
            .unwrap();
        }
        let native = &by_target["native"];
        assert_eq!(native.names.len(), schema_profile::schema_count());
        assert_eq!(native.results.len(), schema_profile::schema_count());
        assert_eq!(
            catalog.initialize_native().unwrap(),
            json!({
                "names": native.names,
                "first": vec![true; schema_profile::schema_count()],
                "repeated": vec![true; schema_profile::schema_count()]
            })
        );
        assert_raw_schema_controls(&catalog);
        assert_source_parameter_cases(&catalog);
        assert_literal_schema_controls(&catalog);
        catalog.check_reference_refusals().unwrap();
    }

    fn assert_literal_schema_controls(catalog: &schema_build::Catalog) {
        let inputs = schema_profile::inputs();
        let literals = schema_profile::declarations(&inputs.snapshot, "literal-controls.toml");
        assert_eq!(
            literals.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                9
            } else {
                3
            }
        );
        assert_eq!(
            literals
                .values()
                .map(|data| data["cases"].as_array().unwrap().len())
                .sum::<usize>(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                63
            } else {
                8
            }
        );
        let mut format_comparisons = 0_usize;
        let mut draft_comparisons = 0_usize;
        let mut byte_witnesses = 0_usize;
        for (name, data) in literals {
            for case in data["cases"].as_array().unwrap() {
                let input: Json =
                    serde_json::from_str(case["input_json"].as_str().unwrap()).unwrap();
                if let Some(witness) = case.get("string_bytes") {
                    assert_eq!(
                        input
                            .pointer(witness["pointer"].as_str().unwrap())
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .len(),
                        usize::try_from(witness["bytes"].as_u64().unwrap()).unwrap(),
                        "{name}: string byte witness"
                    );
                    byte_witnesses = byte_witnesses.checked_add(1).unwrap();
                }
                assert_eq!(
                    catalog.check_native(&name, &input).unwrap(),
                    case["valid"].as_bool().unwrap(),
                    "{name}: {input}"
                );
                if let Some(expected) = case.get("formats_disabled_valid") {
                    assert_eq!(
                        catalog
                            .check_native_formats_disabled(&name, &input)
                            .unwrap(),
                        expected.as_bool().unwrap(),
                        "formats disabled: {name}: {input}"
                    );
                    format_comparisons = format_comparisons.checked_add(1).unwrap();
                }
                if let Some(expected) = case.get("draft4_valid") {
                    assert_eq!(
                        catalog.check_native_draft4(&name, &input).unwrap(),
                        expected.as_bool().unwrap(),
                        "Draft4 contrast: {name}: {input}"
                    );
                    draft_comparisons = draft_comparisons.checked_add(1).unwrap();
                }
            }
        }
        assert_eq!(
            format_comparisons,
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                1
            } else {
                3
            }
        );
        assert_eq!(
            draft_comparisons,
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                2
            } else {
                3
            }
        );
        assert_eq!(
            byte_witnesses,
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                5
            } else {
                0
            }
        );
        assert_numeric_type_controls(catalog, &inputs.snapshot);
    }

    fn assert_numeric_type_controls(
        catalog: &schema_build::Catalog,
        snapshot: &schema_qualification::Inventory,
    ) {
        let numeric = schema_profile::declarations(snapshot, "numeric-type.toml");
        assert_eq!(
            numeric.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                2
            } else {
                0
            }
        );
        for (name, data) in numeric {
            assert_eq!(data["valid"], false);
            let witness = snapshot
                .schemas
                .iter()
                .find(|row| row.witness.name == name)
                .unwrap();
            let schema: Json = serde_json::from_slice(
                &fs::read(snapshot.root.join(&witness.witness.path)).unwrap(),
            )
            .unwrap();
            let dynamic = jsonschema::options()
                .with_draft(jsonschema::Draft::Draft4)
                .build(&schema)
                .unwrap();
            for (field, valid) in [("positive_json", true), ("input_json", false)] {
                let input: Json = serde_json::from_str(data[field].as_str().unwrap()).unwrap();
                assert_eq!(
                    dynamic.is_valid(&input),
                    valid,
                    "runtime numeric type: {name}"
                );
                assert_eq!(
                    catalog.check_native(&name, &input).unwrap(),
                    valid,
                    "generated numeric type: {name}"
                );
            }
        }
    }

    #[test]
    fn all_schema_explicit_dispatch_links_on_native_and_three_wasm_targets() {
        let catalog = schema_build::linked_explicit_catalog().unwrap();
        let by_target: BTreeMap<_, _> = catalog
            .reports()
            .iter()
            .map(|row| (row.target.as_str(), row))
            .collect();
        assert_eq!(catalog.reports().len(), 4);
        assert_eq!(
            by_target.keys().copied().collect::<Vec<_>>(),
            [
                "native",
                "wasm32-unknown-unknown",
                "wasm32-wasip1",
                "wasm32-wasip2"
            ]
        );
        for row in catalog.reports() {
            assert_eq!(row.validators, schema_profile::schema_count());
            assert!(row.artifact_bytes > 0);
            assert_eq!(row.sha256.len(), 64);
            assert_guest_observations(row);
            writeln!(
                stderr(),
                "mode=explicit {}: validators={} bytes={} sha256={}",
                row.target,
                row.validators,
                row.artifact_bytes,
                row.sha256
            )
            .unwrap();
        }
        let inputs = schema_profile::inputs();
        let snapshot = &inputs.snapshot;
        let expected_names: Vec<_> = snapshot
            .schemas
            .iter()
            .map(|row| row.witness.name.clone())
            .collect();
        let native = &by_target["native"];
        assert_eq!(expected_names.len(), schema_profile::schema_count());
        assert_eq!(native.names, expected_names);
        assert_eq!(native.results.len(), schema_profile::schema_count());
        assert_eq!(
            catalog.initialize_native().unwrap(),
            json!({
                "names": native.names,
                "first": vec![true; schema_profile::schema_count()],
                "repeated": vec![true; schema_profile::schema_count()]
            })
        );
        assert_raw_schema_controls(&catalog);
        assert_source_parameter_cases(&catalog);
        assert_literal_schema_controls(&catalog);
        writeln!(
            stderr(),
            "mode=explicit source parameter cases: total={}",
            schema_reference::captured().unwrap().cases.len()
        )
        .unwrap();
        assert_source_parameter_sequences(&catalog);
        catalog.check_explicit_refusals().unwrap();
        catalog.check_reference_refusals().unwrap();
    }

    fn assert_guest_observations(row: &schema_build::LinkReport) {
        if row.target == "native" {
            assert!(row.wasm.is_none());
            return;
        }
        let measured = row.wasm.as_ref().unwrap();
        assert_eq!(measured.artifact_bytes, row.artifact_bytes);
        assert!(measured.core_modules > 0);
        assert!(measured.defined_functions > 0);
        assert!(measured.code_body_bytes > 0);
        assert!(measured.data_segments > 0);
        assert!(measured.active_data_bytes > 0);
        assert!(measured.defined_memories > 0);
        assert!(measured.initial_memory_bytes > 0);
        writeln!(
            stderr(),
            "{}: wasm={}",
            row.target,
            serde_json::to_string(measured).unwrap()
        )
        .unwrap();
    }

    fn assert_raw_schema_controls(catalog: &schema_build::Catalog) {
        let inputs = schema_profile::inputs();
        let declarations = schema_profile::declarations(&inputs.snapshot, "raw-controls.toml");
        assert_eq!(declarations.len(), 1);
        let (name, data) = declarations.first_key_value().unwrap();
        let valid = data["valid_json"].as_str().unwrap();
        let invalid = data["invalid_json"].as_str().unwrap();
        let null = serde_json::to_string(&Json::Null).unwrap();
        for (input, expected) in [
            ("", RawVerdict::InvalidJson),
            ("[", RawVerdict::InvalidJson),
            (valid, RawVerdict::Schema { valid: true }),
            (invalid, RawVerdict::Schema { valid: false }),
            (null.as_str(), RawVerdict::Schema { valid: false }),
        ] {
            assert_eq!(catalog.check_native_raw(name, input).unwrap(), expected);
        }
        let padding = 0x0001_0000_usize.checked_sub(valid.len()).unwrap();
        let at_limit = format!("{valid}{}", " ".repeat(padding));
        assert_eq!(at_limit.len(), 0x0001_0000);
        assert_eq!(
            catalog.check_native_raw(name, &at_limit).unwrap(),
            RawVerdict::Schema { valid: true }
        );
        let above_limit = format!("{at_limit} ");
        assert!(
            catalog
                .check_native_raw(name, &above_limit)
                .unwrap_err()
                .contains("invalid probe input")
        );
        assert!(
            catalog
                .check_native_raw("unknown-schema", "")
                .unwrap_err()
                .contains("unknown schema")
        );
    }

    fn assert_source_parameter_cases(catalog: &schema_build::Catalog) {
        let corpus = schema_reference::captured().unwrap();
        let source_blobs = assert_source_blob_witnesses(&corpus);
        let mut counts = [0_usize; 3];
        let mut by_schema = BTreeMap::<&str, [usize; 3]>::new();
        for case in &corpus.cases {
            let expected = if serde_json::from_str::<Json>(&case.input).is_ok() {
                RawVerdict::Schema { valid: case.valid }
            } else {
                assert!(!case.valid, "source-accepted input must be JSON");
                RawVerdict::InvalidJson
            };
            let actual = catalog.check_native_raw(&case.schema, &case.input).unwrap();
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
            counts[index] = counts[index].checked_add(1).unwrap();
            let schema_counts = by_schema.entry(&case.schema).or_default();
            schema_counts[index] = schema_counts[index].checked_add(1).unwrap();
        }
        assert_eq!(by_schema, reference_case_counts());
        assert_eq!(counts, schema_reference::totals());
        writeln!(stderr(), "source parameter cases: source_blobs={source_blobs}, accepted={}, schema_rejected={}, json_rejected={}, total={}", counts[0], counts[1], counts[2], corpus.cases.len()).unwrap();
    }

    fn assert_source_parameter_sequences(catalog: &schema_build::Catalog) {
        let corpus = schema_reference::captured().unwrap();
        let mut by_schema = BTreeMap::<&str, Vec<(Json, bool)>>::new();
        let mut json_refusals = 0_usize;
        for case in &corpus.cases {
            if let Ok(value) = serde_json::from_str::<Json>(&case.input) {
                by_schema
                    .entry(&case.schema)
                    .or_default()
                    .push((value, case.valid));
            } else {
                assert!(!case.valid, "source-accepted input must be JSON");
                json_refusals = json_refusals.checked_add(1).unwrap();
            }
        }
        assert_eq!(json_refusals, schema_reference::totals()[2]);
        assert_eq!(
            by_schema.keys().copied().collect::<Vec<_>>(),
            reference_case_counts().into_keys().collect::<Vec<_>>()
        );
        let mut source_cases = 0_usize;
        let mut evaluations = 0_usize;
        for (name, cases) in by_schema {
            source_cases = source_cases.checked_add(cases.len()).unwrap();
            let ordered = cases.iter().chain(cases.iter().rev()).chain(cases.iter());
            let (inputs, expected): (Vec<_>, Vec<_>) = ordered
                .map(|(input, valid)| (input.clone(), *valid))
                .unzip();
            assert_eq!(
                catalog.check_native_sequence(name, &inputs).unwrap(),
                expected,
                "mode=explicit persistent source sequence: {name}"
            );
            evaluations = evaluations.checked_add(inputs.len()).unwrap();
        }
        assert_eq!(
            source_cases,
            schema_reference::totals()[0]
                .checked_add(schema_reference::totals()[1])
                .unwrap()
        );
        assert_eq!(evaluations, source_cases.checked_mul(3).unwrap());
        writeln!(stderr(), "mode=explicit persistent source sequences: schemas={} cases={source_cases} evaluations={evaluations}", schema_reference::pins().len()).unwrap();
    }

    fn assert_source_blob_witnesses(corpus: &schema_reference::Corpus) -> usize {
        if !crate::SCHEMA_SOURCE_CONFORMANCE {
            schema_reference::verify_source_blob(
                corpus,
                "fake_base",
                include_bytes!("../support/fake_bidder/source-tables.txt"),
            )
            .unwrap();
            return 1;
        }
        let mut source_blobs = 0_usize;
        if let Some(repository) = env::var_os("AUCTIONEER__BIDDER__SOURCE_DIR") {
            let objects = schema_reference::source_objects().collect::<Vec<_>>();
            assert_eq!(
                objects
                    .iter()
                    .map(|(schema, _)| *schema)
                    .collect::<Vec<_>>(),
                reference_case_counts().into_keys().collect::<Vec<_>>()
            );
            for (schema, object) in objects {
                let output = run(
                    Command::new("git")
                        .arg("--no-replace-objects")
                        .arg("-C")
                        .arg(&repository)
                        .args(["cat-file", "blob", &object]),
                    Duration::from_secs(10),
                )
                .unwrap();
                assert!(
                    output.status.success(),
                    "{schema}: pinned source blob is required"
                );
                schema_reference::verify_source_blob(corpus, schema, &output.stdout).unwrap();
                source_blobs = source_blobs.checked_add(1).unwrap();
                let mut changed = output.stdout.clone();
                changed[0] ^= 1;
                assert_eq!(
                    schema_reference::verify_source_blob(corpus, schema, &changed),
                    Err(schema_reference::CaptureError::Witness)
                );
                let mut changed_corpus = corpus.clone();
                let row = changed_corpus
                    .cases
                    .iter_mut()
                    .find(|row| row.schema == schema && row.input.contains('"'))
                    .unwrap();
                assert!(
                    row.input.contains('"'),
                    "the witness mutation must change its input"
                );
                row.input = row.input.replace('"', "#");
                assert_eq!(
                    schema_reference::verify_source_blob(&changed_corpus, schema, &output.stdout),
                    Err(schema_reference::CaptureError::Witness)
                );
            }
            assert_eq!(source_blobs, schema_reference::pins().len());
        }
        source_blobs
    }
}

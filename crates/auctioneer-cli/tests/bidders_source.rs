//! Exercises the offline inventory command against immutable source objects.

#[cfg(test)]
mod tests {
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeSet;
    use std::env::var_os;
    use std::fs;
    use std::panic::catch_unwind;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::str::from_utf8;
    use tempfile::{TempDir, tempdir};
    use toml::Value;

    const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
    const SOURCE_REPOSITORY: &str = "https://github.com/prebid/prebid-server.git";

    fn source() -> PathBuf {
        var_os("AUCTIONEER__BIDDER__SOURCE_DIR")
            .map(PathBuf::from)
            .expect(
                "set AUCTIONEER__BIDDER__SOURCE_DIR to the separately pinned reference checkout",
            )
    }

    fn pinned_source_repository(manifest: &Value, reference: &Value) -> Option<PathBuf> {
        if main_implementation(manifest, reference).is_some() {
            return Some(source());
        }
        // Select the checkout from an independent pin, never the witness being verified.
        (supplemental_schema(manifest)["repository"].as_str() == Some(SOURCE_REPOSITORY))
            .then(|| {
                var_os("AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR")
                    .map(PathBuf::from)
                    .expect(
                        "set AUCTIONEER__BIDDER__SUPPLEMENTAL_SOURCE_DIR to the pinned supplemental reference checkout",
                    )
            })
    }

    fn inventory(source: &Path, output: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["bidders", "inventory", "--source"])
            .arg(source)
            .arg("--output")
            .arg(output)
            .output()
            .unwrap()
    }

    fn check(path: &Path, complete: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"));
        command
            .args(["bidders", "check", "--source"])
            .arg(source())
            .arg("--inventory")
            .arg(path);
        if complete {
            command.arg("--require-complete");
        }
        command.output().unwrap()
    }

    fn committed_inventory() -> &'static str {
        include_str!(
            "../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml"
        )
    }

    fn source_object(repository: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new("git")
            .args(["--no-replace-objects", "-C"])
            .arg(repository)
            .args(args)
            .output()
            .expect("Git is required to inspect the pinned source objects");
        assert!(
            output.status.success(),
            "cannot inspect immutable source object {args:?} in {}: {}",
            repository.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    fn pinned_inventory() -> Value {
        assert_eq!(
            format!("{:x}", Sha256::digest(committed_inventory().as_bytes())),
            "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6"
        );
        let reference: Value = toml::from_str(committed_inventory()).unwrap();
        assert_eq!(reference["revision"].as_str(), Some(REVISION));
        reference
    }

    fn supplemental_schema(manifest: &Value) -> Value {
        let capture = include_str!(
            "../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/supplemental-schemas.toml"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(capture.as_bytes())),
            "2022d18b8e5580824111a2a3781039b701c7c6df85a0456206ce2b370775ebbb",
            "frozen supplemental source pins"
        );
        let reference: Value = toml::from_str(capture).unwrap();
        reference["schema"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"] == manifest["key"])
            .expect("a non-catalog reference factory needs an independent supplemental source pin")
            .clone()
    }

    fn main_implementation<'a>(manifest: &Value, reference: &'a Value) -> Option<&'a str> {
        let Some(implementations) = reference["implementations"].as_array() else {
            panic!("pinned reference implementations must be an array");
        };
        implementations
            .iter()
            .find(|row| row["internal_key"] == manifest["key"])
            .map(|row| {
                let Some(directory) = row["source_directory"].as_str() else {
                    panic!("pinned reference implementation source directory must be a string");
                };
                directory
            })
    }

    fn verify_manifest_snapshot(bytes: &[u8], fixture: &serde_json::Value) {
        assert_eq!(fixture["schema_version"].as_u64(), Some(1));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            fixture["manifest_sha256"].as_str().unwrap(),
            "bidder-owned frozen manifest"
        );
    }

    fn source_factories(root: &Path, reference: &Value) -> Vec<(PathBuf, Value)> {
        let mut factories = Vec::new();
        let mut keys = BTreeSet::new();
        for entry in fs::read_dir(root).unwrap() {
            let folder = entry.unwrap().path();
            let path = folder.join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let bytes = fs::read(path).unwrap();
            let manifest: Value = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
            if manifest.get("factory").and_then(Value::as_bool) != Some(true) {
                continue;
            }
            let key = manifest["key"].as_str().unwrap();
            assert_eq!(folder.file_name().unwrap().to_str(), Some(key));
            assert!(keys.insert(key.to_owned()), "duplicate factory {key}");
            let fixture =
                serde_json::from_slice(&fs::read(folder.join("fixtures/factory.json")).unwrap())
                    .unwrap();
            verify_manifest_snapshot(&bytes, &fixture);
            if manifest["provenance"]["origin"].as_str() == Some("local") {
                assert!(main_implementation(&manifest, reference).is_none());
                assert!(manifest["provenance"].get("sources").is_none());
                continue;
            }
            verify_factory_ownership(&manifest, reference);
            factories.push((folder, manifest));
        }
        assert!(!factories.is_empty(), "no source-backed factories");
        factories
    }

    fn verify_source_identity(source: &Value, main: bool, supplemental: Option<&Value>) {
        if main {
            assert_eq!(
                source["revision"].as_str(),
                Some(REVISION),
                "main source revision"
            );
            assert_eq!(
                source["repository"].as_str(),
                Some(SOURCE_REPOSITORY),
                "main source repository"
            );
            return;
        }
        if let Some(pin) = supplemental {
            assert_eq!(
                source["revision"], pin["revision"],
                "supplemental source revision"
            );
            assert_eq!(
                source["repository"], pin["repository"],
                "supplemental source repository"
            );
        }
    }

    fn verify_factory_ownership(manifest: &Value, reference: &Value) {
        assert_eq!(manifest["factory"].as_bool(), Some(true));
        let provenance = &manifest["provenance"];
        assert_eq!(provenance["origin"].as_str(), Some("reference"));
        let main = main_implementation(manifest, reference);
        let supplemental = main.is_none().then(|| supplemental_schema(manifest));
        let sources = provenance["sources"].as_table().unwrap();
        assert!(!sources.is_empty(), "missing immutable sources");
        for (name, source) in sources {
            let revision = source["revision"].as_str().unwrap();
            assert!(
                revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "immutable revision for {name}"
            );
            assert!(!source["repository"].as_str().unwrap().is_empty());
            verify_source_identity(
                source,
                name == "upstream" || main.is_some(),
                supplemental.as_ref(),
            );
        }
        let directory = main.map_or_else(
            || format!("adapters/{}", manifest["key"].as_str().unwrap()),
            str::to_owned,
        );
        let trees = provenance["trees"].as_array().unwrap();
        let implementations: Vec<_> = trees
            .iter()
            .filter(|row| row["category"].as_str() == Some("implementation"))
            .collect();
        assert_eq!(implementations.len(), 1, "implementation source ownership");
        let implementation = implementations[0];
        assert_eq!(
            implementation["source_path"].as_str(),
            Some(directory.as_str())
        );
        if main.is_some() {
            assert_eq!(implementation["source"].as_str(), Some("upstream"));
        }
        let artifacts = provenance["artifacts"].as_array().unwrap();
        // Presence and ownership are independent of the manifest's own row counts.
        // Full row-omission coverage belongs to the bidder-owned frozen manifest.
        let canonical: Vec<_> = artifacts
            .iter()
            .filter(|row| {
                row["category"].as_str() == Some("schema")
                    && row
                        .get("retained")
                        .and_then(|retained| retained.get("source_path"))
                        .and_then(Value::as_str)
                        == Some("parameters.json")
            })
            .collect();
        assert_eq!(canonical.len(), 1, "canonical schema source ownership");
        let schema = canonical[0];
        let schema_path = format!(
            "static/bidder-params/{}.json",
            directory.strip_prefix("adapters/").unwrap()
        );
        assert_eq!(schema["source_path"].as_str(), Some(schema_path.as_str()));
        assert_eq!(schema["source"], implementation["source"]);
        if let Some(pin) = &supplemental {
            for field in [
                "source_path",
                "source_blob",
                "source_bytes",
                "source_sha256",
                "retained",
            ] {
                assert_eq!(
                    schema[field], pin[field],
                    "pinned supplemental schema {field}"
                );
            }
        }
        for row in trees.iter().chain(artifacts) {
            assert!(
                sources.contains_key(row["source"].as_str().unwrap()),
                "missing witness source"
            );
        }
        for schema_witness in artifacts
            .iter()
            .filter(|row| row["category"].as_str() == Some("schema"))
        {
            verify_schema_witness(schema_witness, reference);
        }
    }

    fn verify_schema_witness(schema: &Value, reference: &Value) {
        let retained = &schema["retained"];
        assert!(
            schema.get("transformation").is_none(),
            "verbatim schema witness"
        );
        assert_eq!(schema["source_bytes"], retained["source_bytes"]);
        assert_eq!(schema["source_sha256"], retained["source_sha256"]);
        if schema["source"].as_str() == Some("upstream") {
            let pin = reference["schema"]
                .as_array()
                .unwrap()
                .iter()
                .find(|pin| pin["path"] == schema["source_path"])
                .unwrap();
            assert_eq!(schema["source_blob"], pin["blob"], "pinned schema blob");
            assert_eq!(schema["source_bytes"], pin["bytes"], "pinned schema length");
            assert_eq!(
                schema["source_sha256"], pin["sha256"],
                "pinned schema digest"
            );
        }
    }

    fn verify_retained_artifact(artifact: &Value, assets: &Path, source_bytes: Option<&[u8]>) {
        let object = artifact["source_path"].as_str().unwrap();
        if artifact["category"].as_str() == Some("metadata") {
            assert!(
                artifact.get("retained").is_none(),
                "metadata source witness {object}"
            );
        }
        if let Some(retained) = artifact.get("retained") {
            let retained_bytes =
                fs::read(assets.join(retained["source_path"].as_str().unwrap())).unwrap();
            assert_eq!(
                u64::try_from(retained_bytes.len()).unwrap(),
                u64::try_from(retained["source_bytes"].as_integer().unwrap()).unwrap(),
                "retained length {object}"
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&retained_bytes)),
                retained["source_sha256"].as_str().unwrap(),
                "retained digest {object}"
            );
            if let Some(bytes) =
                source_bytes.filter(|_bytes| artifact.get("transformation").is_none())
            {
                assert_eq!(retained_bytes, bytes, "verbatim source capture {object}");
            }
        }
    }

    fn verify_source_factory(
        manifest: &Value,
        assets: &Path,
        repository: &Path,
        reference: &Value,
    ) {
        verify_factory_ownership(manifest, reference);
        let provenance = &manifest["provenance"];
        let sources = provenance["sources"].as_table().unwrap();
        for source in sources.values() {
            assert_eq!(
                source["repository"].as_str(),
                Some(SOURCE_REPOSITORY),
                "pinned public source repository"
            );
            let revision = source["revision"].as_str().unwrap();
            assert_eq!(
                source_object(repository, &["cat-file", "-t", revision]),
                b"commit\n",
                "source revision must identify a commit {revision}"
            );
        }
        for tree in provenance["trees"].as_array().unwrap() {
            let revision = sources[tree["source"].as_str().unwrap()]["revision"]
                .as_str()
                .unwrap();
            let object = format!("{revision}:{}", tree["source_path"].as_str().unwrap());
            assert_eq!(
                source_object(repository, &["cat-file", "-t", &object]),
                b"tree\n",
                "source tree type {object}"
            );
            let tree_id = source_object(repository, &["rev-parse", &object]);
            assert_eq!(
                String::from_utf8(tree_id).unwrap().trim(),
                tree["tree"].as_str().unwrap(),
                "source tree {object}"
            );
        }
        for artifact in provenance["artifacts"].as_array().unwrap() {
            let revision = sources[artifact["source"].as_str().unwrap()]["revision"]
                .as_str()
                .unwrap();
            let object = format!("{revision}:{}", artifact["source_path"].as_str().unwrap());
            let blob_id = source_object(repository, &["rev-parse", &object]);
            assert_eq!(
                String::from_utf8(blob_id).unwrap().trim(),
                artifact["source_blob"].as_str().unwrap(),
                "source blob {object}"
            );
            let bytes = source_object(repository, &["cat-file", "blob", &object]);
            assert_eq!(
                u64::try_from(bytes.len()).unwrap(),
                u64::try_from(artifact["source_bytes"].as_integer().unwrap()).unwrap(),
                "source length {object}"
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                artifact["source_sha256"].as_str().unwrap(),
                "source digest {object}"
            );
            verify_retained_artifact(artifact, assets, Some(&bytes));
        }
    }

    #[test]
    fn pinned_source_factory_provenance_binds_real_objects_and_retained_assets() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-bidder/src/bidders");
        let reference = pinned_inventory();
        let mut checked_main = false;
        let mut checked_supplemental = false;
        for (folder, manifest) in source_factories(&root, &reference) {
            if let Some(repository) = pinned_source_repository(&manifest, &reference) {
                verify_source_factory(&manifest, &folder, &repository, &reference);
                if main_implementation(&manifest, &reference).is_some() {
                    checked_main = true;
                } else {
                    checked_supplemental = true;
                }
            } else {
                // Other repositories retain their separate source contract, not these public pins.
                for artifact in manifest["provenance"]["artifacts"].as_array().unwrap() {
                    verify_retained_artifact(artifact, &folder, None);
                }
            }
        }
        assert!(checked_main, "no main-catalog source factories");
        assert!(
            checked_supplemental,
            "no supplemental public source factories"
        );
    }

    #[test]
    fn pinned_source_factories_reject_an_incorrect_blob_digest() {
        reject_source_witness(
            "source_sha256",
            &Value::String("0".repeat(64)),
            "source digest",
        );
    }

    #[test]
    fn pinned_source_factories_reject_an_incorrect_source_length() {
        reject_source_witness("source_bytes", &Value::Integer(0), "source length");
    }

    #[test]
    fn pinned_source_factories_reject_an_incorrect_tree_id() {
        reject_source_witness("tree", &Value::String("0".repeat(40)), "source tree");
    }

    fn reject_source_witness(field: &str, value: &Value, expected: &str) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-bidder/src/bidders");
        let reference = pinned_inventory();
        let mut checked = false;
        for (folder, mut manifest) in source_factories(&root, &reference) {
            let Some(repository) = pinned_source_repository(&manifest, &reference) else {
                continue;
            };
            // Keep controls small without relying on the manifest's snapshot or row count.
            let artifacts = manifest["provenance"]["artifacts"].as_array_mut().unwrap();
            let raw = artifacts
                .iter()
                .find(|row| {
                    row["category"].as_str() != Some("schema") && row.get("retained").is_none()
                })
                .expect("a source witness independent of retained assets and schema inventory")
                .clone();
            artifacts.retain(|row| {
                row["category"].as_str() == Some("schema")
                    && row["retained"]["source_path"].as_str() == Some("parameters.json")
            });
            artifacts.push(raw);
            manifest["provenance"]["trees"]
                .as_array_mut()
                .unwrap()
                .retain(|row| row["category"].as_str() == Some("implementation"));
            // Establish valid Git prerequisites before accepting any negative control.
            verify_source_factory(&manifest, &folder, &repository, &reference);
            let witness = if field == "tree" {
                &mut manifest["provenance"]["trees"][0]
            } else {
                manifest["provenance"]["artifacts"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|row| {
                        row["category"].as_str() != Some("schema") && row.get("retained").is_none()
                    })
                    .expect("a source witness independent of retained assets and schema inventory")
            };
            witness[field] = value.clone();
            // Deliberately bypass the frozen-manifest hash: the Git oracle must reject this.
            verify_factory_ownership(&manifest, &reference);
            let panic = catch_unwind(|| {
                verify_source_factory(&manifest, &folder, &repository, &reference);
            })
            .expect_err("incorrect source witness must be rejected independently of the snapshot");
            let message = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap();
            assert!(
                message.contains(expected),
                "{}: wrong rejection for {field}: {message}",
                manifest["key"].as_str().unwrap()
            );
            checked = true;
        }
        assert!(
            checked,
            "no pinned public source factories for {field} control"
        );
    }

    #[test]
    fn supplemental_public_sources_reject_changed_repository_or_revision() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-bidder/src/bidders");
        let reference = pinned_inventory();
        let mut checked = false;
        for (_folder, original) in source_factories(&root, &reference) {
            if main_implementation(&original, &reference).is_some() {
                continue;
            }
            for (name, source) in original["provenance"]["sources"].as_table().unwrap() {
                if source["repository"].as_str() != Some(SOURCE_REPOSITORY) {
                    continue;
                }
                for (field, value) in [
                    ("revision", REVISION),
                    ("repository", "https://example.invalid/unrelated.git"),
                ] {
                    let mut invalid = original.clone();
                    invalid["provenance"]["sources"][name.as_str()][field] =
                        Value::String(value.to_owned());
                    assert!(
                        catch_unwind(|| verify_factory_ownership(&invalid, &reference)).is_err(),
                        "incorrect supplemental source {field}"
                    );
                    assert!(
                        pinned_source_repository(&invalid, &reference).is_some(),
                        "incorrect witness must not bypass pinned source verification"
                    );
                }
                checked = true;
            }
        }
        assert!(checked, "no supplemental public source revision controls");
    }

    fn source_factory_fixture() -> (PathBuf, Value) {
        source_factory_fixtures().into_iter().next().unwrap()
    }

    fn source_factory_fixtures() -> Vec<(PathBuf, Value)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-bidder/src/bidders");
        let reference = pinned_inventory();
        let mut rows: Vec<_> = source_factories(&root, &reference)
            .into_iter()
            .filter(|(_root, manifest)| {
                main_implementation(manifest, &reference).is_some()
                    && manifest["provenance"]["artifacts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|row| row["category"].as_str() == Some("helper"))
                    && manifest["provenance"]["artifacts"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|row| {
                            row["category"].as_str() == Some("schema")
                                && row.get("retained").is_some()
                        })
            })
            .collect();
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        assert!(rows.len() >= 2, "independent source-backed fixture owners");
        rows
    }

    #[test]
    fn frozen_manifest_rejects_row_omission_and_missing_or_wrong_snapshot_hash() {
        let (_root, mut manifest) = source_factory_fixture();
        let bytes = toml::to_string(&manifest).unwrap().into_bytes();
        let fixture = serde_json::json!({
            "schema_version": 1_u8,
            "manifest_sha256": format!("{:x}", Sha256::digest(&bytes)),
        });
        verify_manifest_snapshot(&bytes, &fixture);
        for hash in [
            serde_json::Value::Null,
            serde_json::Value::String("0".repeat(64)),
        ] {
            let mut invalid = fixture.clone();
            invalid["manifest_sha256"] = hash;
            assert!(
                catch_unwind(|| verify_manifest_snapshot(&bytes, &invalid)).is_err(),
                "missing or wrong frozen hash"
            );
        }
        assert!(
            manifest["provenance"]["artifacts"]
                .as_array_mut()
                .unwrap()
                .pop()
                .is_some()
        );
        // Ownership alone deliberately does not assert a universal source inventory.
        verify_factory_ownership(&manifest, &pinned_inventory());
        let omitted = toml::to_string(&manifest).unwrap().into_bytes();
        assert!(
            catch_unwind(|| verify_manifest_snapshot(&omitted, &fixture)).is_err(),
            "row omission must invalidate the bidder-owned snapshot"
        );
    }

    #[test]
    fn source_factory_ownership_rejects_missing_witness_groups() {
        let (_root, original) = source_factory_fixture();
        let reference = pinned_inventory();
        verify_factory_ownership(&original, &reference);
        for missing in [
            "sources",
            "trees",
            "artifacts",
            "empty_sources",
            "empty_trees",
            "empty_artifacts",
            "implementation",
            "schema",
            "retained",
        ] {
            let mut manifest = original.clone();
            match missing {
                "sources" | "trees" | "artifacts" => {
                    manifest["provenance"]
                        .as_table_mut()
                        .unwrap()
                        .remove(missing);
                }
                "empty_sources" => {
                    manifest["provenance"]["sources"]
                        .as_table_mut()
                        .unwrap()
                        .clear();
                }
                "empty_trees" | "empty_artifacts" => {
                    let group = missing.strip_prefix("empty_").unwrap();
                    manifest["provenance"][group]
                        .as_array_mut()
                        .unwrap()
                        .clear();
                }
                "implementation" => {
                    manifest["provenance"]["trees"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|row| row["category"].as_str() != Some("implementation"));
                }
                "schema" => {
                    manifest["provenance"]["artifacts"]
                        .as_array_mut()
                        .unwrap()
                        .retain(|row| row["category"].as_str() != Some("schema"));
                }
                _ => {
                    for row in manifest["provenance"]["artifacts"].as_array_mut().unwrap() {
                        row.as_table_mut().unwrap().remove("retained");
                    }
                }
            }
            assert!(
                catch_unwind(|| verify_factory_ownership(&manifest, &reference)).is_err(),
                "missing {missing} witness group"
            );
        }
    }

    #[test]
    fn main_checkout_cannot_certify_a_helpers_different_repository_or_revision() {
        let (_root, mut manifest) = source_factory_fixture();
        let reference = pinned_inventory();
        let source = manifest["provenance"]["sources"]["upstream"].clone();
        manifest["provenance"]["sources"]
            .as_table_mut()
            .unwrap()
            .insert("helper_reference".to_owned(), source);
        let helper = manifest["provenance"]["artifacts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["category"].as_str() == Some("helper"))
            .unwrap();
        helper["source"] = Value::String("helper_reference".to_owned());
        verify_factory_ownership(&manifest, &reference);
        for (field, value) in [
            (
                "repository",
                "https://example.invalid/unrelated.git".to_owned(),
            ),
            ("revision", "a".repeat(40)),
        ] {
            let mut invalid = manifest.clone();
            invalid["provenance"]["sources"]["helper_reference"][field] = Value::String(value);
            assert!(
                catch_unwind(|| verify_factory_ownership(&invalid, &reference)).is_err(),
                "main checkout must not certify helper {field}"
            );
        }
    }

    #[test]
    fn source_factory_ownership_rejects_wrong_revision_repository_and_schema() {
        let (_root, original) = source_factory_fixture();
        let reference = pinned_inventory();
        verify_factory_ownership(&original, &reference);
        for (field, value) in [
            ("revision", "a".repeat(40)),
            (
                "repository",
                "https://example.invalid/source.git".to_owned(),
            ),
        ] {
            let mut manifest = original.clone();
            manifest["provenance"]["sources"]["upstream"][field] = Value::String(value);
            assert!(
                catch_unwind(|| verify_factory_ownership(&manifest, &reference)).is_err(),
                "wrong source {field}"
            );
        }
        let (_other_root, other) = source_factory_fixtures()
            .into_iter()
            .find(|(_path, manifest)| manifest["key"] != original["key"])
            .unwrap();
        let mut manifest = original.clone();
        let schema = manifest["provenance"]["artifacts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["category"].as_str() == Some("schema"))
            .unwrap();
        *schema = other["provenance"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["category"].as_str() == Some("schema"))
            .unwrap()
            .clone();
        assert!(
            catch_unwind(|| verify_factory_ownership(&manifest, &reference)).is_err(),
            "valid immutable schema owned by another factory"
        );
    }

    #[test]
    fn source_factory_ownership_rejects_repinned_schema_and_dangling_sources() {
        let (_root, original) = source_factory_fixture();
        let reference = pinned_inventory();
        for mutation in [
            "schema",
            "tree_source",
            "artifact_source",
            "tree_path",
            "renamed_upstream",
        ] {
            let mut manifest = original.clone();
            match mutation {
                "schema" => {
                    let schema = manifest["provenance"]["artifacts"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|row| row["category"].as_str() == Some("schema"))
                        .unwrap();
                    schema["source_sha256"] = Value::String("0".repeat(64));
                    schema["retained"]["source_sha256"] = Value::String("0".repeat(64));
                }
                "tree_source" => {
                    manifest["provenance"]["trees"][0]["source"] =
                        Value::String("missing".to_owned());
                }
                "tree_path" => {
                    let original_path = main_implementation(&original, &reference).unwrap();
                    let other_path = reference["implementations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|row| row["source_directory"].as_str())
                        .find(|path| *path != original_path)
                        .unwrap();
                    manifest["provenance"]["trees"][0]["source_path"] =
                        Value::String(other_path.to_owned());
                }
                "renamed_upstream" => {
                    let sources = manifest["provenance"]["sources"].as_table_mut().unwrap();
                    let upstream = sources.remove("upstream").unwrap();
                    sources.insert("fork".to_owned(), upstream);
                    for group in ["trees", "artifacts"] {
                        for row in manifest["provenance"][group].as_array_mut().unwrap() {
                            row["source"] = Value::String("fork".to_owned());
                        }
                    }
                }
                _ => {
                    manifest["provenance"]["artifacts"][0]["source"] =
                        Value::String("missing".to_owned());
                }
            }
            assert!(
                catch_unwind(|| verify_factory_ownership(&manifest, &reference)).is_err(),
                "invalid {mutation} ownership"
            );
        }
    }

    #[test]
    fn pinned_source_factories_reject_an_incorrect_blob_id() {
        reject_source_witness("source_blob", &Value::String("0".repeat(40)), "source blob");
    }

    #[test]
    fn retained_artifact_rejects_wrong_length_digest_and_corrupt_bytes() {
        let (root, manifest) = source_factory_fixture();
        let schema = manifest["provenance"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["category"].as_str() == Some("schema"))
            .unwrap();
        verify_retained_artifact(schema, &root, None);
        for (field, value) in [
            ("source_bytes", Value::Integer(0)),
            ("source_sha256", Value::String("0".repeat(64))),
        ] {
            let mut artifact = schema.clone();
            artifact["retained"][field] = value;
            assert!(
                catch_unwind(|| verify_retained_artifact(&artifact, &root, None)).is_err(),
                "wrong retained {field}"
            );
        }
        let directory = tempdir().unwrap();
        let bytes = fs::read(root.join("parameters.json")).unwrap();
        let mut corrupt = bytes.clone();
        corrupt[0] ^= 1;
        fs::write(directory.path().join("parameters.json"), &corrupt).unwrap();
        assert!(
            catch_unwind(|| verify_retained_artifact(schema, directory.path(), None)).is_err(),
            "corrupt retained bytes"
        );
        let mut repinned = schema.clone();
        repinned["retained"]["source_sha256"] =
            Value::String(format!("{:x}", Sha256::digest(&corrupt)));
        assert!(
            catch_unwind(|| verify_retained_artifact(&repinned, directory.path(), Some(&bytes)))
                .is_err(),
            "repinned capture must still equal the verbatim source"
        );
    }

    #[test]
    #[should_panic(expected = "no source-backed factories")]
    fn source_factory_discovery_rejects_an_empty_scan() {
        let directory = tempdir().unwrap();
        source_factories(directory.path(), &pinned_inventory());
    }

    #[test]
    fn reference_check_defaults_to_shared_catalog_inventory() {
        let result = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .args(["bidders", "check", "--source"])
            .arg(source())
            .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["status"], "reference_valid");
        assert_eq!(report["scope"], "reference_inventory_only");
        assert_eq!(report["identities"], 358_u64);
        assert_eq!(report["migrated"], 0_u64);
    }

    #[test]
    fn reference_check_accepts_unfinished_inventory_but_never_claims_migration() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("inventory.toml");
        fs::write(&input, committed_inventory()).unwrap();
        let normal = check(&input, false);
        assert!(normal.status.success(), "{normal:?}");
        let report: serde_json::Value = serde_json::from_slice(&normal.stdout).unwrap();
        assert_eq!(report["status"], "reference_valid");
        assert_eq!(report["scope"], "reference_inventory_only");
        assert_eq!(report["identities"], 358_u64);
        assert_eq!(report["migrated"], 0_u64);
        let complete = check(&input, true);
        assert_eq!(complete.status.code(), Some(1_i32));
        let incomplete_report: serde_json::Value =
            serde_json::from_slice(&complete.stdout).unwrap();
        assert_eq!(incomplete_report["status"], "migration_incomplete");
        assert_eq!(incomplete_report["migrated"], 0_u64);
    }

    #[test]
    fn reference_check_rejects_a_wrong_revision_even_with_repinned_digests() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("inventory.toml");
        let mut data: Value = toml::from_str(committed_inventory()).unwrap();
        data["revision"] = Value::String("a".repeat(40));
        for key in ["identity", "schema"] {
            for row in data[key].as_array_mut().unwrap() {
                row["blob"] = Value::String("b".repeat(40));
                row["sha256"] = Value::String("c".repeat(64));
            }
        }
        fs::write(&input, toml::to_string(&data).unwrap()).unwrap();
        let result = check(&input, false);
        assert_eq!(result.status.code(), Some(1_i32));
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(report["status"], "reference_drift");
    }

    #[test]
    fn reference_check_refuses_forged_completion_missing_rows_and_unknown_fields() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("inventory.toml");
        let original: Value = toml::from_str(committed_inventory()).unwrap();
        for mutation in ["missing", "duplicate", "unknown", "migrated", "alias"] {
            let mut data = original.clone();
            match mutation {
                "missing" => {
                    data["identity"].as_array_mut().unwrap().pop();
                }
                "duplicate" => {
                    let row = data["identity"].as_array().unwrap()[0].clone();
                    data["identity"].as_array_mut().unwrap().push(row);
                }
                "unknown" => {
                    data.as_table_mut()
                        .unwrap()
                        .insert("unreviewed".to_owned(), Value::Boolean(true));
                }
                "migrated" => {
                    data["identity"].as_array_mut().unwrap()[0]
                        .as_table_mut()
                        .unwrap()
                        .insert(
                            "migration_state".to_owned(),
                            Value::String("migrated".to_owned()),
                        );
                }
                _ => {
                    data["identity"].as_array_mut().unwrap()[0]
                        .as_table_mut()
                        .unwrap()
                        .insert(
                            "alias_of".to_owned(),
                            Value::String("missing_base".to_owned()),
                        );
                }
            }
            fs::write(&input, toml::to_string(&data).unwrap()).unwrap();
            let result = check(&input, false);
            assert_eq!(result.status.code(), Some(1_i32), "{mutation}: {result:?}");
            let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(report["status"], "reference_drift", "{mutation}");
        }
    }

    #[test]
    fn reference_check_distinguishes_invalid_input_from_drift_without_echoing_bytes() {
        let directory = tempdir().unwrap();
        let input = directory.path().join("inventory.toml");
        let oversized = format!("#{}\n", "x".repeat(0x0010_0000));
        for bytes in [
            "SENSITIVE_CANARY invalid TOML",
            "revision = 'one'\nrevision = 'two'",
            &oversized,
        ] {
            fs::write(&input, bytes).unwrap();
            let result = check(&input, false);
            assert_eq!(result.status.code(), Some(2_i32));
            assert!(result.stdout.is_empty());
            let diagnostic = String::from_utf8(result.stderr).unwrap();
            assert!(!diagnostic.contains("SENSITIVE_CANARY"));
            assert!(
                diagnostic.contains("bidder_inventory_input"),
                "{diagnostic}"
            );
        }
    }

    fn git(source: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(["--no-replace-objects", "-C"])
            .arg(source)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    fn isolated_source() -> TempDir {
        let clone = tempdir().unwrap();
        let output = Command::new("git")
            .args(["clone", "--shared", "--no-checkout", "--"])
            .arg(source())
            .arg(clone.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        clone
    }

    #[test]
    fn pinned_inventory_reports_exact_source_and_http_populations() {
        let directory = tempdir().unwrap();
        let output = directory.path().join("inventory.toml");
        let result = inventory(&source(), &output);
        assert!(result.status.success(), "{result:?}");
        let bytes = fs::read(&output).unwrap();
        assert!(
            bytes
                == include_bytes!(
                    "../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml"
                ),
            "committed reference inventory differs from pinned Git objects"
        );
        let data: Value = toml::from_str(&fs::read_to_string(output).unwrap()).unwrap();
        assert_eq!(data["revision"].as_str(), Some(REVISION));
        for (name, count) in [
            ("identities", 358_i64),
            ("bases", 261),
            ("aliases", 97),
            ("implementations", 259),
            ("schemas", 261),
            ("schema_bytes", 164_518),
            ("largest_schema_bytes", 3_778),
            ("http_bases", 78),
            ("enabled_http_bases", 77),
            ("disabled_http_bases", 1),
            ("sync_identities", 196),
            ("sync_families", 187),
            ("without_sync", 162),
            ("declared_sync_identities", 188),
            ("inherited_sync_identities", 8),
            ("supports_only_identities", 38),
        ] {
            assert_eq!(data["counts"][name].as_integer(), Some(count), "{name}");
        }
        let rows = data["identity"].as_array().unwrap();
        assert_eq!(rows.len(), 358);
        let reference = pinned_inventory();
        assert_eq!(data["identity"], reference["identity"]);
        for row in rows {
            assert!(row.get("callable_endpoint").is_none());
        }
        let implementations = data["implementations"].as_array().unwrap();
        assert_eq!(implementations.len(), 259);
        assert_eq!(data["implementations"], reference["implementations"]);
        assert_eq!(data["schema"].as_array().unwrap().len(), 261);
        assert_eq!(data["schema"], reference["schema"]);
    }

    #[test]
    fn sync_inventory_separates_family_inheritance_from_vendor_and_endpoint_ownership() {
        let directory = tempdir().unwrap();
        let output = directory.path().join("inventory.toml");
        assert!(inventory(&source(), &output).status.success());
        let data: Value = toml::from_str(&fs::read_to_string(output).unwrap()).unwrap();
        let rows = data["identity"].as_array().unwrap();
        let reference = pinned_inventory();
        let expected = reference["identity"].as_array().unwrap();
        assert_eq!(rows.len(), expected.len());
        let mut origins = BTreeSet::new();
        let mut independent_vendors = 0_usize;
        let mut undeclared_vendors = 0_usize;
        for pin in expected {
            let name = pin["name"].as_str().unwrap();
            let row = rows
                .iter()
                .find(|row| row["name"].as_str() == Some(name))
                .unwrap();
            assert_eq!(row["sync"], pin["sync"], "{name}");
            assert_eq!(row["source_vendor_id"], pin["source_vendor_id"], "{name}");
            origins.insert(row["sync"]["origin"].as_str().unwrap());
            assert_eq!(
                row["sync"]["defined"].as_bool(),
                Some(row["sync"].get("family").is_some()),
                "{name}"
            );
            if row["source_vendor_id"].as_integer() == Some(0) {
                undeclared_vendors += 1;
            }
            if let Some(base_name) = row.get("alias_of").and_then(Value::as_str) {
                let base = rows
                    .iter()
                    .find(|base| base["name"].as_str() == Some(base_name))
                    .unwrap();
                if row["sync"]["origin"].as_str() == Some("inherited_key") {
                    assert_eq!(row["sync"]["family"], base["sync"]["family"]);
                    assert!(row["sync"]["endpoint_types"].as_array().unwrap().is_empty());
                    if row["source_vendor_id"] != base["source_vendor_id"] {
                        independent_vendors += 1;
                    }
                }
            }
        }
        assert_eq!(
            origins,
            BTreeSet::from(["inherited_key", "declared", "supports_only", "absent"])
        );
        assert!(
            independent_vendors > 0,
            "inherited sync must not replace vendor ownership"
        );
        assert!(
            undeclared_vendors > 0,
            "missing vendor declarations stay zero"
        );
        let families = data["sync_family"].as_array().unwrap();
        assert_eq!(families.len(), 187);
        assert_eq!(data["sync_family"], reference["sync_family"]);
        let shared: Vec<_> = families
            .iter()
            .filter(|row| row["members"].as_array().unwrap().len() > 1)
            .collect();
        assert_eq!(shared.len(), 6);
        for family in shared {
            let key = family["key"].as_str().unwrap();
            assert!(
                !family["endpoint_providers"].as_array().unwrap().is_empty(),
                "{key}"
            );
        }
    }

    #[test]
    fn pending_factory_capture_reconciles_every_base_and_implementation_directory() {
        let bytes = include_bytes!(
            "../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "d1a0f18d99f53cbd4f62fe74c5d1e5580f8688fce95a29b1b05ee934dbe9ff0c"
        );
        let capture: Value = toml::from_str(from_utf8(bytes).unwrap()).unwrap();
        let facts = pinned_inventory();
        assert_eq!(capture["schema_version"].as_integer(), Some(1));
        assert_eq!(capture["revision"].as_str(), Some(REVISION));
        assert_eq!(
            capture["source_path"].as_str(),
            Some("exchange/adapter_builders.go")
        );
        assert_eq!(
            capture["source_blob"].as_str(),
            Some("630fbe348b94d3196cb2a330a7f8ff0790eb5e03")
        );
        assert_eq!(
            capture["identity_source_path"].as_str(),
            Some("openrtb_ext/bidders.go")
        );
        assert_eq!(
            capture["identity_source_blob"].as_str(),
            Some("1b61ff724e249fa6cfaa5bcd728c857cddb4a7bf")
        );
        assert_eq!(capture["captured_on"].as_str(), Some("2026-10-16"));
        assert_eq!(capture["review_status"].as_str(), Some("pending"));
        assert_eq!(
            capture
                .as_table()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "schema_version",
                "revision",
                "source_path",
                "source_blob",
                "identity_source_path",
                "identity_source_blob",
                "captured_on",
                "review_status",
                "bindings",
            ])
        );
        let bindings = capture["bindings"].as_table().unwrap();
        let bases: BTreeSet<_> = facts["identity"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row.get("alias_of").is_none())
            .map(|row| row["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            bindings.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            bases
        );
        assert_eq!(bindings.len(), 261);
        let directories: BTreeSet<_> = facts["implementations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["source_directory"].as_str().unwrap())
            .collect();
        assert_eq!(
            bindings
                .values()
                .map(|value| value.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            directories
        );
        assert_eq!(directories.len(), 259);
        for (path, blob) in [
            ("source_path", "source_blob"),
            ("identity_source_path", "identity_source_blob"),
        ] {
            let object = format!("{REVISION}:{}", capture[path].as_str().unwrap());
            let actual = source_object(&source(), &["rev-parse", &object]);
            assert_eq!(
                from_utf8(&actual).unwrap().trim(),
                capture[blob].as_str().unwrap()
            );
        }
    }

    #[test]
    fn inventory_is_byte_identical_when_working_files_and_replacement_refs_change() {
        let clone = isolated_source();
        let directory = tempdir().unwrap();
        let before = directory.path().join("before.toml");
        let after = directory.path().join("after.toml");
        assert!(inventory(clone.path(), &before).status.success());
        fs::create_dir_all(clone.path().join("static/bidder-info")).unwrap();
        let reference = pinned_inventory();
        let path = reference["identity"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row.get("alias_of").is_none())
            .unwrap()["path"]
            .as_str()
            .unwrap();
        fs::write(
            clone.path().join(path),
            "endpoint: https://attacker.invalid\n",
        )
        .unwrap();
        git(
            clone.path(),
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--allow-empty",
                "-m",
                "empty replacement tree",
            ],
        );
        git(clone.path(), &["replace", REVISION, "HEAD"]);
        let replaced = Command::new("git")
            .arg("-C")
            .arg(clone.path())
            .args([
                "rev-parse",
                "--verify",
                &format!("{REVISION}:static/bidder-info"),
            ])
            .output()
            .unwrap();
        assert!(
            !replaced.status.success(),
            "replacement must actually hide the source tree"
        );
        let result = inventory(clone.path(), &after);
        assert!(result.status.success(), "{result:?}");
        assert_eq!(fs::read(before).unwrap(), fs::read(after).unwrap());
    }

    #[test]
    fn unrelated_repository_refuses_without_publishing_inventory() {
        let repository = tempdir().unwrap();
        git(repository.path(), &["init"]);
        let output = repository.path().join("inventory.toml");
        let result = inventory(repository.path(), &output);
        assert_eq!(result.status.code(), Some(2_i32));
        assert!(!output.exists());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("bidder_source")
        );
    }

    #[test]
    fn inventory_never_overwrites_an_existing_output() {
        let directory = tempdir().unwrap();
        let output = directory.path().join("inventory.toml");
        fs::write(&output, "previous artifact\n").unwrap();
        let result = inventory(&source(), &output);
        assert_eq!(result.status.code(), Some(2_i32));
        assert_eq!(fs::read_to_string(output).unwrap(), "previous artifact\n");
    }
}

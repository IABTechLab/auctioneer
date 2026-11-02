//! Offline staging is reference data, never permission to register or route a bidder.

#[cfg(test)]
mod tests {
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::env::{temp_dir, var_os};
    use std::fs;
    use std::io::Result as IoResult;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::str::from_utf8;
    use tempfile::{Builder, TempDir};

    const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
    const HTTPS_DEFAULT_NOTE: &str = "Source HTTP template uses the HTTPS default; effective metadata binding and route tests remain pending; no new runtime definition is generated.";

    fn tempdir() -> IoResult<TempDir> {
        Builder::new().tempdir_in(temp_dir().canonicalize()?)
    }

    fn source() -> PathBuf {
        var_os("AUCTIONEER__BIDDER__SOURCE_DIR")
            .map(PathBuf::from)
            .expect("set AUCTIONEER__BIDDER__SOURCE_DIR to the pinned reference checkout")
    }

    fn source_git(repository: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new("git")
            .args(["--no-replace-objects", "-C"])
            .arg(repository)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}: {output:?}");
        output.stdout
    }

    fn source_bytes(repository: &Path, source_path: &str) -> Vec<u8> {
        source_git(repository, &["show", &format!("{REVISION}:{source_path}")])
    }

    fn pinned_inventory() -> toml::Value {
        let bytes = include_bytes!(
            "../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6"
        );
        toml::from_str(from_utf8(bytes).unwrap()).unwrap()
    }

    fn pinned_bindings() -> toml::Value {
        let bytes = include_bytes!(
            "../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "d1a0f18d99f53cbd4f62fe74c5d1e5580f8688fce95a29b1b05ee934dbe9ff0c"
        );
        toml::from_str(from_utf8(bytes).unwrap()).unwrap()
    }

    fn import(staging: &Path, check: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"));
        command
            .args(["bidders", "import", "--source"])
            .arg(source())
            .arg("--staging")
            .arg(staging);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    }

    fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn visit(root: &Path, path: &Path, rows: &mut Vec<(PathBuf, Vec<u8>)>) {
            for item in fs::read_dir(path).unwrap() {
                let entry = item.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    visit(root, &entry.path(), rows);
                } else {
                    rows.push((
                        entry.path().strip_prefix(root).unwrap().to_owned(),
                        fs::read(entry.path()).unwrap(),
                    ));
                }
            }
        }
        let mut rows = Vec::new();
        visit(root, root, &mut rows);
        rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        rows
    }

    fn assert_inactive_reference(manifest: &toml::Value) {
        assert_eq!(manifest["factory"].as_bool(), Some(false));
        assert!(manifest.get("endpoint").is_none());
        assert!(
            manifest
                .get("definitions")
                .is_none_or(|rows| rows.as_table().unwrap().is_empty())
        );
        let upstream = &manifest["provenance"]["sources"]["upstream"];
        assert_eq!(upstream["revision"].as_str(), Some(REVISION));
        assert_eq!(
            upstream["repository"].as_str(),
            Some("https://github.com/prebid/prebid-server.git")
        );
    }

    fn assert_source_witness(repository: &Path, artifact: &toml::Value) -> Vec<u8> {
        assert_eq!(artifact["source"].as_str(), Some("upstream"));
        let source_path = artifact["source_path"].as_str().unwrap();
        let object = format!("{REVISION}:{source_path}");
        let blob = String::from_utf8(source_git(repository, &["rev-parse", &object])).unwrap();
        assert_eq!(artifact["source_blob"].as_str(), Some(blob.trim()));
        let original = source_bytes(repository, source_path);
        assert_eq!(
            artifact["source_bytes"].as_integer().unwrap(),
            i64::try_from(original.len()).unwrap()
        );
        assert_eq!(
            artifact["source_sha256"].as_str().unwrap(),
            format!("{:x}", Sha256::digest(&original))
        );
        original
    }

    fn assert_retained_schema(owner: &Path, artifact: &toml::Value, original: &[u8]) {
        let retained = &artifact["retained"];
        let path = owner.join(retained["source_path"].as_str().unwrap());
        assert_eq!(fs::read(path).unwrap(), original);
        assert_eq!(
            retained["source_bytes"].as_integer().unwrap(),
            i64::try_from(original.len()).unwrap()
        );
        assert_eq!(
            retained["source_sha256"].as_str().unwrap(),
            format!("{:x}", Sha256::digest(original))
        );
    }

    fn source_endpoint<'a>(
        document: &'a serde_yaml_ng::Value,
        documents: &'a BTreeMap<String, serde_yaml_ng::Value>,
    ) -> Option<(&'a str, Option<&'a str>)> {
        if let Some(endpoint) = document["endpoint"]
            .as_str()
            .filter(|value| !value.is_empty())
        {
            return Some((endpoint, None));
        }
        let parent = document["aliasOf"]
            .as_str()
            .filter(|value| !value.is_empty())?;
        let endpoint = documents[parent]["endpoint"]
            .as_str()
            .filter(|value| !value.is_empty())?;
        Some((endpoint, Some(parent)))
    }

    fn expected_https_default(
        document: &serde_yaml_ng::Value,
        documents: &BTreeMap<String, serde_yaml_ng::Value>,
    ) -> Option<toml::Value> {
        if document["whiteLabelOnly"].as_bool() == Some(true) {
            return None;
        }
        let (source_url, inherited_from) = source_endpoint(document, documents)?;
        let remainder = source_url.strip_prefix("http://")?;
        let mut record = [
            ("source_url".to_owned(), source_url.into()),
            (
                "selected_url".to_owned(),
                format!("https://{remainder}").into(),
            ),
        ]
        .into_iter()
        .collect::<toml::Table>();
        if let Some(parent) = inherited_from {
            record.insert("inherited_from".to_owned(), parent.into());
        }
        Some(toml::Value::Table(record))
    }

    fn assert_import_artifacts(
        repository: &Path,
        owner: &Path,
        manifest: &toml::Value,
        metadata: &mut BTreeSet<String>,
        documents: &BTreeMap<String, serde_yaml_ng::Value>,
        https_defaults: &mut BTreeMap<String, toml::Value>,
    ) -> usize {
        assert_inactive_reference(manifest);
        let mut schemas = 0_usize;
        for artifact in manifest["provenance"]["artifacts"].as_array().unwrap() {
            let category = artifact["category"].as_str().unwrap();
            if !matches!(category, "metadata" | "schema") {
                assert!(artifact.get("https_default").is_none());
                continue;
            }
            let original = assert_source_witness(repository, artifact);
            if category == "metadata" {
                let source_path = artifact["source_path"].as_str().unwrap();
                assert!(artifact.get("retained").is_none(), "{source_path}");
                assert!(metadata.insert(source_path.to_owned()), "{source_path}");
                let document: serde_yaml_ng::Value = serde_yaml_ng::from_slice(&original).unwrap();
                let expected = expected_https_default(&document, documents);
                assert_eq!(
                    artifact.get("https_default"),
                    expected.as_ref(),
                    "{source_path}"
                );
                let note = if let Some(record) = artifact.get("https_default") {
                    assert!(
                        https_defaults
                            .insert(source_path.to_owned(), record.clone())
                            .is_none(),
                        "{source_path}"
                    );
                    HTTPS_DEFAULT_NOTE
                } else if document["whiteLabelOnly"].as_bool() == Some(true) {
                    "Source identity is white-label-only; it supplies no independently callable base."
                } else if source_endpoint(&document, documents).is_none() {
                    "Source routing unavailable: missing endpoint; no runtime definition is generated."
                } else {
                    "Source endpoint policy is unreviewed; no runtime definition is generated."
                };
                let notes = artifact["port_differences"].as_array().unwrap();
                assert!(
                    notes.iter().any(|row| row.as_str() == Some(note)),
                    "{source_path}"
                );
                assert!(
                    notes.iter().all(|row| !row
                        .as_str()
                        .unwrap()
                        .contains("HTTP endpoint requires independent HTTPS review")),
                    "{source_path}"
                );
                if let Some(record) = artifact.get("https_default") {
                    let source_url = record["source_url"].as_str().unwrap();
                    let selected_url = record["selected_url"].as_str().unwrap();
                    assert_eq!(
                        source_url.strip_prefix("http://").unwrap().as_bytes(),
                        selected_url.strip_prefix("https://").unwrap().as_bytes(),
                        "{source_path}"
                    );
                }
                assert!(artifact.get("callable_endpoint").is_none());
            } else {
                assert!(artifact.get("https_default").is_none());
                assert_retained_schema(owner, artifact, &original);
                schemas = schemas.checked_add(1).unwrap();
            }
        }
        schemas
    }

    fn protected_bidder_paths(repository: &Path) -> Vec<String> {
        let owned = "crates/auctioneer-bidder/src/bidders";
        let bidder_paths: Vec<_> = tree(&repository.join(owned))
            .into_iter()
            .map(|(path, _bytes)| format!("{owned}/{}", path.to_str().unwrap()))
            .filter(|path| {
                path.ends_with("/parameters.json")
                    || path.ends_with(".schema.json")
                    || path.ends_with("/bidder.toml")
                    || path.contains("/fixtures/")
                    || path.ends_with("/LICENSE.reference")
            })
            .collect();
        assert!(
            bidder_paths
                .iter()
                .any(|path| path.ends_with(".schema.json"))
        );
        assert!(
            bidder_paths
                .iter()
                .any(|path| path.contains("/fixtures/protocol/"))
        );
        bidder_paths
    }

    #[test]
    fn checkout_filters_preserve_byte_pinned_catalog_and_resource_fixtures() {
        let catalog = "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog";
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let isolated = tempdir().unwrap();
        let database = isolated.path().join("objects.git");
        let worktree = isolated.path().join("worktree");
        let attributes = isolated.path().join("empty-attributes");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(&attributes, b"").unwrap();
        fs::copy(
            repository.join(".gitattributes"),
            worktree.join(".gitattributes"),
        )
        .unwrap();
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("--git-dir")
                .arg(&database)
                .arg("--work-tree")
                .arg(&worktree)
                .args(["-c", "core.autocrlf=true", "-c"])
                .arg(format!("core.attributesFile={}", attributes.display()))
                .env_remove("GIT_COMMON_DIR")
                .env_remove("GIT_OBJECT_DIRECTORY")
                .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
                .env_remove("GIT_INDEX_FILE")
                .env("GIT_ATTR_NOSYSTEM", "1")
                .args(args)
                .current_dir(&worktree)
                .output()
                .unwrap();
            assert!(output.status.success(), "{args:?}: {output:?}");
            output.stdout
        };
        drop(git(&["init", "--quiet", "--template="]));
        let reference_paths = ["source-inventory.toml", "source-bindings.toml"]
            .map(|relative| format!("{catalog}/{relative}"));
        let mut checked = 0_usize;
        let mut conversion_controls = 0_usize;
        let bidder_paths = protected_bidder_paths(&repository);
        let expected = reference_paths.len() + bidder_paths.len() + 4;
        for relative in reference_paths.into_iter().chain(bidder_paths).chain([
            "crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml".to_owned(),
            "crates/auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline-patched-lock.toml".to_owned(),
            "crates/auctioneer-bidder/tests/conformance/fixtures/reference/schema-reference/parameters.json".to_owned(),
            "crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml".to_owned(),
        ]) {
            let fixture = repository.join(&relative);
            let raw = fs::read(&fixture).unwrap();
            let blob_bytes = git(&[
                "hash-object",
                "-w",
                "--no-filters",
                "--",
                fixture.to_str().unwrap(),
            ]);
            let blob = String::from_utf8(blob_bytes).unwrap();
            if conversion_controls == 0 && relative.ends_with("/parameters.json") {
                let converted = git(&[
                    "cat-file",
                    "--filters",
                    "--path=unprotected/parameters.json",
                    blob.trim(),
                ]);
                assert!(
                    converted != raw,
                    "unprotected LF schema must undergo CRLF conversion"
                );
                conversion_controls += 1;
            }
            let filtered = git(&[
                "cat-file",
                "--filters",
                &format!("--path={relative}"),
                blob.trim(),
            ]);
            assert!(filtered == raw, "checkout changed {relative}");
            checked += 1;
        }
        assert_eq!(checked, expected);
        assert_eq!(conversion_controls, 1);
    }

    #[test]
    fn import_is_deterministic_complete_and_never_approves_dispatch() {
        let root = tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let result = import(&first, false);
        assert!(result.status.success(), "{result:?}");
        assert!(import(&second, false).status.success());
        assert_eq!(tree(&first), tree(&second));
        let owned = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../auctioneer-bidder/src/bidders")
            .canonicalize()
            .unwrap();
        let checked = import(&owned, true);
        assert!(checked.status.success(), "{checked:?}");
        for (path, bytes) in tree(&first) {
            if path.file_name().and_then(|name| name.to_str()) == Some("bidder.toml") {
                let manifest = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
                assert_inactive_reference(&manifest);
            }
        }
        assert_eq!(tree(&first).len(), 521);
        assert!(
            fs::read_to_string(first.join("LICENSE.reference"))
                .unwrap()
                .contains("Copyright 2017 PREBID.ORG, INC")
        );
    }

    fn assert_pinned_import_artifact(
        staging: &Path,
        manifest: &toml::Value,
        artifact: &toml::Value,
        reference: &toml::Value,
        bindings: &toml::Value,
    ) -> String {
        let category = artifact["category"].as_str().unwrap();
        let pins = reference[if category == "metadata" {
            "identity"
        } else {
            "schema"
        }]
        .as_array()
        .unwrap();
        let pin = pins
            .iter()
            .find(|pin| pin["path"] == artifact["source_path"])
            .unwrap();
        let name = pin["name"].as_str().unwrap();
        for (field, pinned) in [
            ("source_blob", "blob"),
            ("source_bytes", "bytes"),
            ("source_sha256", "sha256"),
        ] {
            assert_eq!(artifact[field], pin[pinned], "{name}: {field}");
        }
        let base = pin
            .get("alias_of")
            .and_then(toml::Value::as_str)
            .unwrap_or(name);
        let implementation = reference["implementations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source_directory"] == bindings["bindings"][base])
            .unwrap();
        assert_eq!(manifest["key"], implementation["internal_key"], "{name}");
        if category == "schema" {
            let key = manifest["key"].as_str().unwrap();
            let owned: toml::Value = toml::from_str(
                &fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../auctioneer-bidder/src/bidders")
                        .join(key)
                        .join("bidder.toml"),
                )
                .unwrap(),
            )
            .unwrap();
            let retained = owned["provenance"]["artifacts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| {
                    row["category"].as_str() == Some("schema") && row["source_path"] == pin["path"]
                })
                .unwrap();
            assert_eq!(
                artifact["retained"], retained["retained"],
                "{name}: bidder-owned schema layout"
            );
            assert_retained_schema(
                &staging.join(key),
                artifact,
                &source_bytes(&source(), pin["path"].as_str().unwrap()),
            );
            if name != key {
                assert!(
                    !staging.join(name).exists(),
                    "schema must remain with its source factory: {name}"
                );
            }
        }
        name.to_owned()
    }

    #[test]
    fn fresh_import_owns_every_schema_and_identity_without_creating_factories() {
        let root = tempdir().unwrap();
        let staging = root.path().join("owned");
        let result = import(&staging, false);
        assert!(result.status.success(), "{result:?}");
        let reference = pinned_inventory();
        let capture = pinned_bindings();
        let implementations = reference["implementations"].as_array().unwrap();
        let identities = reference["identity"].as_array().unwrap();
        let schema_pins = reference["schema"].as_array().unwrap();
        let mut seen_metadata = BTreeSet::new();
        let mut seen_schemas = BTreeSet::new();
        let mut seen_packages = BTreeSet::new();
        let mut packages = 0_usize;
        let mut metadata = 0_usize;
        let mut schemas = 0_usize;
        for entry_result in fs::read_dir(&staging).unwrap() {
            let entry = entry_result.unwrap();
            if !entry.file_type().unwrap().is_dir() {
                continue;
            }
            let owner = entry.path();
            let manifest: toml::Value =
                toml::from_str(&fs::read_to_string(owner.join("bidder.toml")).unwrap()).unwrap();
            let key = manifest["key"].as_str().unwrap();
            assert_eq!(owner.file_name().unwrap().to_str(), Some(key));
            assert!(seen_packages.insert(key.to_owned()));
            assert_eq!(manifest["factory"].as_bool(), Some(false));
            assert!(
                manifest
                    .get("definitions")
                    .is_none_or(|rows| rows.as_table().unwrap().is_empty())
            );
            assert!(!owner.join("mod.rs").exists());
            assert!(!owner.join("LICENSE.reference").exists());
            for artifact in manifest["provenance"]["artifacts"].as_array().unwrap() {
                match artifact["category"].as_str().unwrap() {
                    category @ ("metadata" | "schema") => {
                        let seen = if category == "metadata" {
                            metadata += 1;
                            &mut seen_metadata
                        } else {
                            schemas += 1;
                            &mut seen_schemas
                        };
                        assert!(seen.insert(assert_pinned_import_artifact(
                            &staging, &manifest, artifact, &reference, &capture
                        )));
                    }
                    "license" => {}
                    other => panic!("unexpected importer-owned artifact category {other}"),
                }
            }
            packages += 1;
        }
        assert_eq!(
            seen_packages,
            implementations
                .iter()
                .map(|row| row["internal_key"].as_str().unwrap().to_owned())
                .collect()
        );
        assert_eq!(
            seen_metadata,
            identities
                .iter()
                .map(|row| row["name"].as_str().unwrap().to_owned())
                .collect()
        );
        assert_eq!(
            seen_schemas,
            schema_pins
                .iter()
                .map(|row| row["name"].as_str().unwrap().to_owned())
                .collect()
        );
        assert_eq!((packages, metadata, schemas), (259, 358, 261));
        assert_eq!(tree(&staging).len(), 521);
        assert!(staging.join("LICENSE.reference").is_file());
        assert!(!staging.join("manifest.json").exists());
    }

    fn source_metadata_paths(repository: &Path) -> BTreeSet<String> {
        let expected_metadata: BTreeSet<_> = String::from_utf8(source_git(
            repository,
            &[
                "ls-tree",
                "-r",
                "--name-only",
                REVISION,
                "--",
                "static/bidder-info",
            ],
        ))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
        assert_eq!(expected_metadata.len(), 358);
        expected_metadata
    }

    fn source_metadata_documents(
        repository: &Path,
        paths: &BTreeSet<String>,
    ) -> BTreeMap<String, serde_yaml_ng::Value> {
        paths
            .iter()
            .map(|path| {
                let name = path
                    .strip_prefix("static/bidder-info/")
                    .unwrap()
                    .strip_suffix(".yaml")
                    .unwrap();
                (
                    name.to_owned(),
                    serde_yaml_ng::from_slice(&source_bytes(repository, path)).unwrap(),
                )
            })
            .collect()
    }

    fn declared_http_base_paths(
        documents: &BTreeMap<String, serde_yaml_ng::Value>,
    ) -> BTreeSet<String> {
        let declared_http_bases: BTreeSet<_> = documents
            .iter()
            .filter(|(_, document)| {
                document["aliasOf"].as_str().is_none_or(str::is_empty)
                    && document["endpoint"]
                        .as_str()
                        .is_some_and(|endpoint| endpoint.starts_with("http://"))
            })
            .map(|(name, _)| format!("static/bidder-info/{name}.yaml"))
            .collect();
        assert_eq!(declared_http_bases.len(), 78);
        let enabled_http_bases = documents
            .values()
            .filter(|document| {
                document["aliasOf"].as_str().is_none_or(str::is_empty)
                    && document["endpoint"]
                        .as_str()
                        .is_some_and(|endpoint| endpoint.starts_with("http://"))
                    && document["disabled"].as_bool() != Some(true)
            })
            .count();
        assert_eq!(enabled_http_bases, 77);
        assert_eq!(
            declared_http_bases.len().checked_sub(enabled_http_bases),
            Some(1)
        );
        declared_http_bases
    }

    fn assert_https_default_population(
        documents: &BTreeMap<String, serde_yaml_ng::Value>,
        https_defaults: &BTreeMap<String, toml::Value>,
    ) {
        let expected_https: BTreeMap<_, _> = documents
            .iter()
            .filter_map(|(name, document)| {
                expected_https_default(document, documents)
                    .map(|record| (format!("static/bidder-info/{name}.yaml"), record))
            })
            .collect();
        let declared_http_bases = declared_http_base_paths(documents);
        let inherited_http: BTreeSet<_> = documents
            .iter()
            .filter(|(_, document)| {
                document["whiteLabelOnly"].as_bool() != Some(true)
                    && source_endpoint(document, documents).is_some_and(|(endpoint, inherited)| {
                        inherited.is_some() && endpoint.starts_with("http://")
                    })
            })
            .map(|(name, _)| format!("static/bidder-info/{name}.yaml"))
            .collect();
        let own_http_aliases: BTreeSet<_> = documents
            .iter()
            .filter(|(_, document)| {
                document["whiteLabelOnly"].as_bool() != Some(true)
                    && document["aliasOf"]
                        .as_str()
                        .is_some_and(|name| !name.is_empty())
                    && document["endpoint"]
                        .as_str()
                        .is_some_and(|endpoint| endpoint.starts_with("http://"))
            })
            .map(|(name, _)| format!("static/bidder-info/{name}.yaml"))
            .collect();
        assert!(!inherited_http.is_empty());
        assert_eq!(inherited_http.len(), 17);
        assert_eq!(own_http_aliases.len(), 27);
        assert_eq!(expected_https.len(), 122);
        assert_eq!(
            expected_https.keys().cloned().collect::<BTreeSet<_>>(),
            declared_http_bases
                .iter()
                .chain(&inherited_http)
                .chain(&own_http_aliases)
                .cloned()
                .collect()
        );
        assert_eq!(https_defaults, &expected_https);
        assert_eq!(
            https_defaults
                .iter()
                .filter(|(_, record)| record.get("inherited_from").is_some())
                .map(|(path, _)| path.clone())
                .collect::<BTreeSet<_>>(),
            inherited_http
        );
        assert_eq!(
            https_defaults.len(),
            declared_http_bases
                .len()
                .checked_add(inherited_http.len())
                .and_then(|count| count.checked_add(own_http_aliases.len()))
                .unwrap()
        );
    }

    #[test]
    fn fresh_import_records_metadata_provenance_without_copying_source_yaml() {
        let repository = source();
        let root = tempdir().unwrap();
        let staging = root.path().join("staging");
        let result = import(&staging, false);
        assert!(result.status.success(), "{result:?}");
        let files = tree(&staging);
        let yaml: Vec<_> = files
            .iter()
            .filter(|(path, _)| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("yaml" | "yml")
                )
            })
            .map(|(path, _)| path)
            .collect();
        assert!(yaml.is_empty(), "fresh import copied source YAML: {yaml:?}");
        let expected_metadata = source_metadata_paths(&repository);
        let documents = source_metadata_documents(&repository, &expected_metadata);
        let mut metadata = BTreeSet::new();
        let mut https_defaults = BTreeMap::new();
        let mut schemas = 0_usize;
        for (path, bytes) in &files {
            if path.file_name().and_then(|name| name.to_str()) != Some("bidder.toml") {
                continue;
            }
            let manifest: toml::Value = toml::from_str(from_utf8(bytes).unwrap()).unwrap();
            let owner = staging.join(path.parent().unwrap());
            schemas += assert_import_artifacts(
                &repository,
                &owner,
                &manifest,
                &mut metadata,
                &documents,
                &mut https_defaults,
            );
        }
        assert_eq!(metadata, expected_metadata);
        assert_https_default_population(&documents, &https_defaults);
        assert_eq!(schemas, 261);
        assert_eq!(files.len(), 521);
    }

    #[test]
    fn check_never_creates_or_repairs_changed_added_or_missing_files() {
        let root = tempdir().unwrap();
        let staging = root.path().join("staging");
        assert_eq!(import(&staging, true).status.code(), Some(1_i32));
        assert!(!staging.exists());
        assert!(import(&staging, false).status.success());
        let before = tree(&staging);
        assert!(import(&staging, true).status.success());
        assert_eq!(tree(&staging), before);
        let parameter_path = before
            .iter()
            .map(|(path, _bytes)| path)
            .find(|path| path.file_name().and_then(|name| name.to_str()) == Some("parameters.json"))
            .unwrap();
        let path = staging.join(parameter_path);
        let original = fs::read(&path).unwrap();
        let mut same_length = original.clone();
        *same_length.first_mut().unwrap() = b'[';
        fs::write(&path, &same_length).unwrap();
        assert_eq!(import(&staging, true).status.code(), Some(1_i32));
        assert_eq!(fs::read(&path).unwrap(), same_length);
        fs::write(&path, b"{}\n").unwrap();
        assert_eq!(import(&staging, true).status.code(), Some(1_i32));
        assert_eq!(fs::read(&path).unwrap(), b"{}\n");
        fs::write(&path, &original).unwrap();
        fs::write(staging.join("extra.json"), b"{}\n").unwrap();
        assert_eq!(import(&staging, true).status.code(), Some(1_i32));
        fs::remove_file(staging.join("extra.json")).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(import(&staging, true).status.code(), Some(1_i32));
        assert!(!path.exists());
    }

    #[derive(Debug)]
    enum HttpsDefaultMutation {
        Field(&'static str, &'static str),
        MissingInheritance,
        MissingRecord,
        NonMetadataRecord,
    }

    #[test]
    fn check_rejects_https_default_record_tampering_without_repair() {
        let root = tempdir().unwrap();
        let staging = root.path().join("staging");
        let result = import(&staging, false);
        assert!(result.status.success(), "{result:?}");
        assert!(import(&staging, true).status.success());
        let files = tree(&staging);
        let (relative, original, manifest, index) = files
            .iter()
            .filter(|(path, _)| {
                path.file_name().and_then(|name| name.to_str()) == Some("bidder.toml")
            })
            .find_map(|(path, bytes)| {
                let manifest: toml::Value = toml::from_str(from_utf8(bytes).unwrap()).unwrap();
                let index = manifest["provenance"]["artifacts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .position(|artifact| {
                        artifact
                            .get("https_default")
                            .is_some_and(|record| record.get("inherited_from").is_some())
                    })?;
                Some((path, bytes, manifest, index))
            })
            .expect("source-derived inherited HTTP endpoint must have HTTPS-default provenance");
        let path = staging.join(relative);
        for mutation in [
            HttpsDefaultMutation::Field(
                "source_url",
                "http://fake_changed.invalid/{{.AccountID}}?q=%2F#fragment",
            ),
            HttpsDefaultMutation::Field(
                "selected_url",
                "https://fake_changed.invalid/{{.AccountID}}?q=%2F#fragment",
            ),
            HttpsDefaultMutation::Field("inherited_from", "fake_unrelated_parent"),
            HttpsDefaultMutation::MissingInheritance,
            HttpsDefaultMutation::MissingRecord,
            HttpsDefaultMutation::NonMetadataRecord,
        ] {
            let mut changed = manifest.clone();
            let artifacts = changed["provenance"]["artifacts"].as_array_mut().unwrap();
            match mutation {
                HttpsDefaultMutation::MissingRecord => {
                    artifacts[index]
                        .as_table_mut()
                        .unwrap()
                        .remove("https_default");
                }
                HttpsDefaultMutation::MissingInheritance => {
                    artifacts[index]["https_default"]
                        .as_table_mut()
                        .unwrap()
                        .remove("inherited_from");
                }
                HttpsDefaultMutation::NonMetadataRecord => {
                    let record = artifacts[index]["https_default"].clone();
                    artifacts
                        .iter_mut()
                        .find(|artifact| artifact["category"].as_str() != Some("metadata"))
                        .unwrap()
                        .as_table_mut()
                        .unwrap()
                        .insert("https_default".to_owned(), record);
                }
                HttpsDefaultMutation::Field(field, value) => {
                    artifacts[index]["https_default"]
                        .as_table_mut()
                        .unwrap()
                        .insert(field.to_owned(), value.into());
                }
            }
            fs::write(&path, toml::to_string_pretty(&changed).unwrap()).unwrap();
            let tampered = tree(&staging);
            let checked = import(&staging, true);
            assert_eq!(
                checked.status.code(),
                Some(1_i32),
                "{mutation:?}: {checked:?}"
            );
            assert_eq!(tree(&staging), tampered, "{mutation:?}");
            fs::write(&path, original).unwrap();
        }
        assert_eq!(tree(&staging), files);
        assert!(import(&staging, true).status.success());
    }

    #[test]
    fn publication_refuses_existing_destinations_and_leaves_unrelated_data_unchanged() {
        let root = tempdir().unwrap();
        let staging = root.path().join("staging");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("operator-data"), b"retain me").unwrap();
        let result = import(&staging, false);
        assert_eq!(result.status.code(), Some(2_i32));
        assert_eq!(result.stderr, b"bidder_import_output\n");
        assert_eq!(
            tree(&staging),
            [(PathBuf::from("operator-data"), b"retain me".to_vec())]
        );
    }

    #[cfg(unix)]
    #[test]
    fn staging_check_refuses_symbolic_links_without_reading_their_targets() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let staging = root.path().join("staging");
        let missing = root.path().join("must-not-create");
        symlink(&missing, &staging).unwrap();
        let result = import(&staging, true);
        assert_eq!(result.status.code(), Some(2_i32));
        assert_eq!(result.stderr, b"bidder_import_input\n");
        assert!(!missing.exists());
    }
}

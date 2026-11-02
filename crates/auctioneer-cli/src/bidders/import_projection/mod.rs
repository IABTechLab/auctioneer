//! Check importer-owned source associations without treating a review as re-import.

use super::bidder_manifest::{Category, Manifest, Provenance, Source};
use super::import::Files;
use super::{Error, import_packages, source};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::str::from_utf8;

mod schema_evidence;

fn original(source: &Source) -> bool {
    source.repository == import_packages::REPOSITORY && source.revision == source::REVISION
}

fn source_projection(manifest: &Manifest) -> Result<BTreeMap<String, Value>, Error> {
    let mut projection = BTreeMap::new();
    let Provenance::Reference {
        sources,
        trees,
        artifacts,
    } = &manifest.provenance
    else {
        return Ok(projection);
    };
    for tree in trees {
        let source = sources.get(&tree.source).ok_or(Error::ImportInput)?;
        if original(source) && tree.category == Category::Implementation {
            let value = json!({"category": tree.category, "source": source, "path": tree.source_path, "tree": tree.tree});
            if projection
                .insert(format!("tree:{}", tree.source_path), value)
                .is_some()
            {
                return Err(Error::ImportInput);
            }
        }
    }
    for artifact in artifacts {
        if artifact.https_default.is_some() && artifact.category != Category::Metadata {
            return Err(Error::ImportInput);
        }
        let source = sources.get(&artifact.source).ok_or(Error::ImportInput)?;
        if original(source)
            && matches!(
                artifact.category,
                Category::Schema | Category::Metadata | Category::License
            )
        {
            let value = json!({"category": artifact.category, "source": source, "path": artifact.source_path,
                "blob": artifact.source_blob, "bytes": artifact.source_bytes, "sha256": artifact.source_sha256,
                "retained": artifact.retained, "https_default": artifact.https_default});
            if projection
                .insert(format!("blob:{}", artifact.source_path), value)
                .is_some()
            {
                return Err(Error::ImportInput);
            }
        }
    }
    Ok(projection)
}

fn manifests(files: &Files) -> Result<BTreeMap<PathBuf, Manifest>, Error> {
    files
        .iter()
        .filter(|(path, _)| path.file_name().is_some_and(|name| name == "bidder.toml"))
        .map(|(path, bytes)| {
            let manifest: Manifest =
                toml::from_str(from_utf8(bytes).map_err(|_error| Error::ImportInput)?)
                    .map_err(|_error| Error::ImportInput)?;
            if path.components().count() != 2 || path.parent() != Some(Path::new(&manifest.key)) {
                return Err(Error::ImportInput);
            }
            Ok((path.clone(), manifest))
        })
        .collect()
}

fn retained_inputs(
    actual: &Files,
    manifests: &BTreeMap<PathBuf, Manifest>,
) -> Result<BTreeSet<PathBuf>, Error> {
    let mut paths = BTreeSet::new();
    for manifest in manifests.values() {
        let Provenance::Reference { artifacts, .. } = &manifest.provenance else {
            continue;
        };
        for artifact in artifacts {
            let Some(retained) = &artifact.retained else {
                continue;
            };
            let relative = Path::new(&retained.path);
            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
            {
                return Err(Error::ImportInput);
            }
            let path = Path::new(&manifest.key).join(relative);
            let bytes = actual.get(&path).ok_or(Error::ImportInput)?;
            if u64::try_from(bytes.len()).ok() != Some(retained.bytes)
                || format!("{:x}", Sha256::digest(bytes)) != retained.sha256
            {
                return Err(Error::ImportInput);
            }
            paths.insert(path);
        }
    }
    Ok(paths)
}

fn manifest_matches(actual: &Manifest, expected: Option<&Manifest>) -> Result<bool, Error> {
    let Ok(projection) = source_projection(actual) else {
        return Ok(false);
    };
    if let Some(expected_manifest) = expected
        && projection != source_projection(expected_manifest)?
    {
        return Ok(false);
    }
    if !actual.factory && (!actual.definitions.is_empty() || actual.required_feature.is_some()) {
        return Ok(false);
    }
    Ok(expected.is_some()
        || (actual.factory
            && match &actual.provenance {
                Provenance::Local {} => true,
                Provenance::Reference { sources, .. } => !sources.values().any(original),
            }))
}

pub(super) fn matches(actual: &Files, expected: &Files) -> Result<bool, Error> {
    let expected_manifests = manifests(expected)?;
    // Non-catalog callers still use the same confined exact-tree checker.
    if expected_manifests.is_empty() {
        return Ok(actual == expected);
    }
    let Ok(actual_manifests) = manifests(actual) else {
        return Ok(false);
    };
    if expected_manifests
        .keys()
        .any(|path| !actual_manifests.contains_key(path))
    {
        return Ok(false);
    }
    for (path, manifest) in &actual_manifests {
        if !manifest_matches(manifest, expected_manifests.get(path))? {
            return Ok(false);
        }
    }
    let Ok(retained) = retained_inputs(actual, &actual_manifests) else {
        return Ok(false);
    };
    for (path, bytes) in expected {
        if !expected_manifests.contains_key(path) && actual.get(path) != Some(bytes) {
            return Ok(false);
        }
    }
    for path in actual.keys().filter(|path| !expected.contains_key(*path)) {
        if actual_manifests.contains_key(path)
            || retained.contains(path)
            || path == Path::new("README.md")
            || (path.components().count() == 1
                && path.extension().is_some_and(|extension| extension == "rs"))
        {
            continue;
        }
        let Some(owner) = path
            .components()
            .next()
            .and_then(|part| match part {
                Component::Normal(name) => Some(Path::new(name).join("bidder.toml")),
                Component::Prefix(_)
                | Component::RootDir
                | Component::CurDir
                | Component::ParentDir => None,
            })
            .and_then(|owner_path| actual_manifests.get(&owner_path))
        else {
            return Ok(false);
        };
        // A sync UID policy is independently discovered; it does not imply
        // that this owner has a reviewed executable bidding factory.
        if path.strip_prefix(&owner.key) == Ok(Path::new("uid_policy.rs")) {
            continue;
        }
        let schema_names = match &owner.provenance {
            Provenance::Reference { artifacts, .. } => artifacts
                .iter()
                .filter(|artifact| artifact.category == Category::Schema)
                .filter_map(|artifact| {
                    artifact
                        .source_path
                        .strip_prefix("static/bidder-params/")
                        .and_then(|name| name.strip_suffix(".json"))
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>(),
            Provenance::Local {} => Vec::new(),
        };
        if path
            .strip_prefix(&owner.key)
            .is_ok_and(|relative| schema_evidence::owned(relative, &actual[path], &schema_names))
        {
            continue;
        }
        if !owner.factory
            || !(path.extension().is_some_and(|extension| extension == "rs")
                || path.file_name().is_some_and(|name| name == "README.md")
                || path.strip_prefix(&owner.key).is_ok_and(|relative| {
                    ["fixtures/factory.json", "fixtures/privacy.json"]
                        .iter()
                        .any(|fixture| relative == Path::new(fixture))
                }))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::super::import::Files;
    use serde_json::json;
    use std::path::PathBuf;
    use std::str::from_utf8;

    fn fixture() -> Files {
        Files::from([
            (
                PathBuf::from("fake_example/bidder.toml"),
                br#"
key = "fake_example"
factory = false
[provenance]
origin = "reference"
[provenance.sources.original]
repository = "https://github.com/prebid/prebid-server.git"
revision = "d7f8515b86258688304b0d9b6668c6a0e258bc9e"
[[provenance.artifacts]]
category = "schema"
source = "original"
source_path = "static/bidder-params/fake_example.json"
source_blob = "0000000000000000000000000000000000000000"
source_bytes = 3
source_sha256 = "ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356"
[provenance.artifacts.retained]
source_path = "parameters.json"
source_bytes = 3
source_sha256 = "ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356"
"#
                .to_vec(),
            ),
            (
                PathBuf::from("fake_example/parameters.json"),
                b"{}\n".to_vec(),
            ),
        ])
    }

    #[test]
    fn https_default_source_projection_pins_each_selection_field_and_its_presence() {
        let path = PathBuf::from("fake_example/bidder.toml");
        let mut expected = fixture();
        let mut manifest: toml::Value =
            toml::from_str(from_utf8(&expected[&path]).unwrap()).unwrap();
        manifest["provenance"]["artifacts"]
            .as_array_mut()
            .unwrap()
            .push(
                toml::Value::try_from(json!({
                    "category": "metadata", "source": "original",
                    "source_path": "static/bidder-info/fake_example.yaml",
                    "source_blob": "1".repeat(40), "source_bytes": 19_u64,
                    "source_sha256": "2".repeat(64),
                    "https_default": {
                        "source_url": "http://bid.example.test:80/rtb?account={{.AccountID}}",
                        "selected_url": "https://bid.example.test:80/rtb?account={{.AccountID}}",
                        "inherited_from": "fake_parent"
                    }
                }))
                .unwrap(),
            );
        expected.insert(
            path.clone(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        );
        assert_eq!(super::matches(&expected, &expected), Ok(true));
        for (field, value) in [
            ("source_url", "http://other.example.test/rtb"),
            ("selected_url", "https://bid.example.test/other"),
            ("inherited_from", "fake_other"),
        ] {
            let mut changed = manifest.clone();
            changed["provenance"]["artifacts"][1]["https_default"][field] = value.into();
            let mut actual = expected.clone();
            actual.insert(
                path.clone(),
                toml::to_string(&changed).unwrap().into_bytes(),
            );
            assert_eq!(super::matches(&actual, &expected), Ok(false), "{field}");
        }
        for field in ["source_url", "selected_url", "inherited_from"] {
            let mut changed = manifest.clone();
            changed["provenance"]["artifacts"][1]["https_default"]
                .as_table_mut()
                .unwrap()
                .remove(field)
                .unwrap();
            let mut actual = expected.clone();
            actual.insert(
                path.clone(),
                toml::to_string(&changed).unwrap().into_bytes(),
            );
            assert_eq!(
                super::matches(&actual, &expected),
                Ok(false),
                "missing {field}"
            );
        }
        let mut absent = manifest.clone();
        absent["provenance"]["artifacts"][1]
            .as_table_mut()
            .unwrap()
            .remove("https_default")
            .unwrap();
        let mut actual = expected.clone();
        actual.insert(path.clone(), toml::to_string(&absent).unwrap().into_bytes());
        assert_eq!(super::matches(&actual, &expected), Ok(false));

        manifest["provenance"]["artifacts"][1]
            .as_table_mut()
            .unwrap()
            .insert(
                "port_differences".to_owned(),
                toml::Value::Array(vec!["Independent protocol review note".into()]),
            );
        let mut reviewed = expected.clone();
        reviewed.insert(path, toml::to_string(&manifest).unwrap().into_bytes());
        assert_eq!(super::matches(&reviewed, &expected), Ok(true));
    }

    #[test]
    fn https_default_cannot_be_hidden_on_an_independently_reviewed_protocol_witness() {
        let path = PathBuf::from("fake_example/bidder.toml");
        let expected = fixture();
        let mut manifest: toml::Value =
            toml::from_str(from_utf8(&expected[&path]).unwrap()).unwrap();
        let artifacts = manifest["provenance"]["artifacts"].as_array_mut().unwrap();
        let mut protocol = artifacts[0].as_table().unwrap().clone();
        protocol.insert("category".to_owned(), "protocol".into());
        protocol.insert(
            "source_path".to_owned(),
            "adapters/fake_example/fake_test.json".into(),
        );
        protocol.remove("retained").unwrap();
        protocol.insert(
            "https_default".to_owned(),
            toml::Value::try_from(json!({
                "source_url": "http://bid.example.test/rtb",
                "selected_url": "https://bid.example.test/rtb"
            }))
            .unwrap(),
        );
        artifacts.push(toml::Value::Table(protocol));
        let mut actual = expected.clone();
        actual.insert(
            path.clone(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        );
        assert_eq!(super::matches(&actual, &expected), Ok(false));
        manifest["provenance"]["artifacts"][1]
            .as_table_mut()
            .unwrap()
            .remove("https_default")
            .unwrap();
        actual.insert(path, toml::to_string(&manifest).unwrap().into_bytes());
        assert_eq!(super::matches(&actual, &expected), Ok(true));
    }

    #[test]
    fn https_default_scope_is_checked_for_supplemental_only_manifests() {
        let expected = fixture();
        let path = PathBuf::from("fake_supplemental/bidder.toml");
        let mut manifest = toml::Value::try_from(json!({
            "key": "fake_supplemental", "factory": true,
            "provenance": {
                "origin": "reference",
                "sources": {"supplemental": {
                    "repository": "https://example.test/fake_supplemental.git",
                    "revision": "9".repeat(40)
                }},
                "artifacts": [{
                    "category": "protocol", "source": "supplemental",
                    "source_path": "adapters/fake_supplemental/fake_test.json",
                    "source_blob": "3".repeat(40), "source_bytes": 17_u64,
                    "source_sha256": "4".repeat(64),
                    "https_default": {
                        "source_url": "http://bid.example.test/rtb",
                        "selected_url": "https://bid.example.test/rtb"
                    }
                }]
            }
        }))
        .unwrap();
        let mut actual = expected.clone();
        actual.insert(
            path.clone(),
            toml::to_string(&manifest).unwrap().into_bytes(),
        );
        assert_eq!(super::matches(&actual, &expected), Ok(false));
        manifest["provenance"]["artifacts"][0]
            .as_table_mut()
            .unwrap()
            .remove("https_default")
            .unwrap();
        actual.insert(path, toml::to_string(&manifest).unwrap().into_bytes());
        assert_eq!(super::matches(&actual, &expected), Ok(true));
    }

    #[test]
    fn checking_preserves_reviewed_factory_definitions_notes_and_code() {
        let expected = fixture();
        let mut actual = expected.clone();
        let path = PathBuf::from("fake_example/bidder.toml");
        let text = String::from_utf8(actual[&path].clone())
            .unwrap()
            .replace("factory = false", "factory = true");
        actual.insert(
            path,
            format!("{text}\n[definitions.fake_example]\nendpoint = 'https://reviewed.invalid'\n")
                .into_bytes(),
        );
        actual.insert(
            PathBuf::from("fake_example/mod.rs"),
            b"// reviewed code\n".to_vec(),
        );
        actual.insert(PathBuf::from("mod.rs"), b"// discovered modules\n".to_vec());
        assert_eq!(super::matches(&actual, &expected), Ok(true));
        assert_eq!(super::matches(&expected, &expected), Ok(true));
    }

    #[test]
    fn checking_preserves_only_the_factories_exact_local_contract_path() {
        let expected = fixture();
        let manifest_path = PathBuf::from("fake_example/bidder.toml");
        let original = String::from_utf8(expected[&manifest_path].clone()).unwrap();
        for (factory, path, accepted) in [
            (true, "fake_example/fixtures/factory.json", true),
            (false, "fake_example/fixtures/factory.json", false),
            (true, "fake_example/fixtures/privacy.json", true),
            (false, "fake_example/fixtures/privacy.json", false),
            (true, "fake_example/nested/fixtures/privacy.json", false),
            (true, "fake_unknown/fixtures/privacy.json", false),
            (true, "fake_example/fixtures/unowned.json", false),
            (true, "fake_example/nested/fixtures/factory.json", false),
            (true, "fake_unknown/fixtures/factory.json", false),
        ] {
            let mut actual = expected.clone();
            if factory {
                actual.insert(
                    manifest_path.clone(),
                    original
                        .replace("factory = false", "factory = true")
                        .into_bytes(),
                );
            }
            actual.insert(PathBuf::from(path), b"{}\n".to_vec());
            assert_eq!(
                super::matches(&actual, &expected),
                Ok(accepted),
                "{path}, factory={factory}"
            );
        }
    }

    #[test]
    fn checking_preserves_only_the_exact_owner_uid_policy_without_factory_promotion() {
        let expected = fixture();
        let manifest = PathBuf::from("fake_example/bidder.toml");
        let original = String::from_utf8(expected[&manifest].clone()).unwrap();
        for factory in [false, true] {
            for (path, accepted) in [
                ("fake_example/uid_policy.rs", true),
                ("fake_example/nested/uid_policy.rs", factory),
                ("fake_example/mod.rs", factory),
                ("fake_unknown/uid_policy.rs", false),
                ("fake_example/uid_policy.json", false),
            ] {
                let mut actual = expected.clone();
                if factory {
                    actual.insert(
                        manifest.clone(),
                        original
                            .replace("factory = false", "factory = true")
                            .into_bytes(),
                    );
                }
                actual.insert(
                    PathBuf::from(path),
                    b"// independently reviewed policy\n".to_vec(),
                );
                assert_eq!(
                    super::matches(&actual, &expected),
                    Ok(accepted),
                    "{path}, factory={factory}"
                );
                assert_eq!(
                    actual[&PathBuf::from("fake_example/parameters.json")],
                    expected[&PathBuf::from("fake_example/parameters.json")]
                );
            }
        }
    }

    #[test]
    fn schema_conformance_is_typed_and_bound_to_a_retained_schema_owner() {
        let expected = fixture();
        for (path, bytes, accepted) in [
            (
                "fake_example/tests/schema/helpers.toml",
                "schema='fake_example'\nhelpers=[{index=0,prefixes=2,onepass=true,alternative=false}]\n",
                true,
            ),
            (
                "fake_example/tests/schema/literal-controls.toml",
                "schema='fake_example'\ncases=[{input_json='{}',valid=false,draft4_valid=true,formats_disabled_valid=false,string_bytes={pointer='/value',bytes=1}}]\n",
                true,
            ),
            (
                "fake_example/tests/schema/literal-controls.toml",
                "schema='fake_example'\ncases=[{input_json='{}',valid=false,draft4_valid='true'}]\n",
                false,
            ),
            (
                "fake_example/tests/schema/helpers.toml",
                "schema='fake_other'\nhelpers=[]\n",
                false,
            ),
            (
                "fake_example/tests/schema/helpers.toml",
                "schema='fake_example'\nhelpers=[]\nunowned=true\n",
                false,
            ),
            (
                "fake_example/tests/schema/helpers.toml",
                "schema='fake_example'\nhelpers=[{index=0,unowned=true}]\n",
                false,
            ),
            (
                "fake_example/tests/schema/unknown.toml",
                "schema='fake_example'\nhelpers=[]\n",
                false,
            ),
            (
                "fake_example/nested/tests/schema/helpers.toml",
                "schema='fake_example'\nhelpers=[]\n",
                false,
            ),
            ("fake_example/tests/schema/helpers.json", "{}\n", false),
            (
                "fake_unknown/tests/schema/helpers.toml",
                "schema='fake_example'\nhelpers=[]\n",
                false,
            ),
        ] {
            let mut actual = expected.clone();
            actual.insert(PathBuf::from(path), bytes.as_bytes().to_vec());
            assert_eq!(
                super::matches(&actual, &expected),
                Ok(accepted),
                "{path}: {bytes}"
            );
        }
    }

    #[test]
    fn checked_source_and_manifest_drift_cannot_be_hidden_by_local_review() {
        let expected = fixture();
        for (from, to) in [
            (
                "d7f8515b86258688304b0d9b6668c6a0e258bc9e",
                "0000000000000000000000000000000000000000",
            ),
            ("prebid/prebid-server.git", "other/reference.git"),
            (
                "static/bidder-params/fake_example.json",
                "static/bidder-params/fake_other.json",
            ),
            ("source_bytes = 3", "source_bytes = 4"),
            ("category = \"schema\"", "category = \"protocol\""),
            ("path = \"parameters.json\"", "path = \"other.json\""),
        ] {
            let mut actual = expected.clone();
            let path = PathBuf::from("fake_example/bidder.toml");
            let text = String::from_utf8(actual[&path].clone()).unwrap();
            assert!(text.contains(from));
            actual.insert(path, text.replace(from, to).into_bytes());
            assert_eq!(super::matches(&actual, &expected), Ok(false), "{from}");
        }
        for path in ["fake_example/parameters.json", "fake_example/bidder.toml"] {
            let mut actual = expected.clone();
            actual.remove(&PathBuf::from(path));
            assert_eq!(super::matches(&actual, &expected), Ok(false));
        }
        for path in [
            "extra.json",
            "fake_example/fixtures/unowned.json",
            "fake_example/mod.rs",
        ] {
            let mut actual = expected.clone();
            actual.insert(PathBuf::from(path), b"extra".to_vec());
            assert_eq!(super::matches(&actual, &expected), Ok(false), "{path}");
        }
        let mut actual = expected.clone();
        actual.insert(
            PathBuf::from("fake_example/parameters.json"),
            b"[]\n".to_vec(),
        );
        assert_eq!(super::matches(&actual, &expected), Ok(false));
    }

    #[test]
    fn checking_preserves_declared_supplemental_captures_but_rejects_changed_bytes() {
        let expected = fixture();
        let mut actual = expected.clone();
        let original =
            String::from_utf8(actual[&PathBuf::from("fake_example/bidder.toml")].clone()).unwrap();
        let supplemental = original
            .replace("fake_example", "fake_supplemental")
            .replace("factory = false", "factory = true")
            .replace(
                "d7f8515b86258688304b0d9b6668c6a0e258bc9e",
                "1111111111111111111111111111111111111111",
            );
        actual.insert(
            PathBuf::from("fake_supplemental/bidder.toml"),
            supplemental.into_bytes(),
        );
        actual.insert(
            PathBuf::from("fake_supplemental/mod.rs"),
            b"// separate port\n".to_vec(),
        );
        actual.insert(
            PathBuf::from("fake_supplemental/parameters.json"),
            b"{}\n".to_vec(),
        );
        assert_eq!(super::matches(&actual, &expected), Ok(true));
        actual.insert(
            PathBuf::from("fake_supplemental/parameters.json"),
            b"[]\n".to_vec(),
        );
        assert_eq!(super::matches(&actual, &expected), Ok(false));
    }

    #[test]
    fn checking_requires_each_exact_unique_implementation_tree_witness() {
        let mut expected = fixture();
        let path = PathBuf::from("fake_example/bidder.toml");
        expected.get_mut(&path).unwrap().extend_from_slice(
            br#"
[[provenance.trees]]
category = "implementation"
source = "original"
source_path = "adapters/fake_example"
tree = "1111111111111111111111111111111111111111"
"#,
        );
        assert_eq!(super::matches(&expected, &expected), Ok(true));
        let original: toml::Value = toml::from_str(from_utf8(&expected[&path]).unwrap()).unwrap();
        for (field, value) in [
            ("category", "protocol"),
            ("source", "missing"),
            ("source_path", "adapters/fake_other"),
            ("tree", "2222222222222222222222222222222222222222"),
        ] {
            let mut document = original.clone();
            document["provenance"]["trees"][0][field] = value.into();
            let mut actual = expected.clone();
            actual.insert(
                path.clone(),
                toml::to_string(&document).unwrap().into_bytes(),
            );
            assert_eq!(super::matches(&actual, &expected), Ok(false), "{field}");
        }
        for duplicate in [false, true] {
            let mut document = original.clone();
            let trees = document["provenance"]["trees"].as_array_mut().unwrap();
            if duplicate {
                trees.push(trees[0].clone());
            } else {
                trees.clear();
            }
            let mut actual = expected.clone();
            actual.insert(
                path.clone(),
                toml::to_string(&document).unwrap().into_bytes(),
            );
            assert_eq!(
                super::matches(&actual, &expected),
                Ok(false),
                "duplicate={duplicate}"
            );
        }
        let mut reviewed = original;
        reviewed["provenance"]["trees"][0]
            .as_table_mut()
            .unwrap()
            .insert(
                "port_differences".into(),
                toml::Value::Array(vec!["Reviewed local implementation differences".into()]),
            );
        let mut actual = expected.clone();
        actual.insert(path, toml::to_string(&reviewed).unwrap().into_bytes());
        assert_eq!(super::matches(&actual, &expected), Ok(true));
    }
}

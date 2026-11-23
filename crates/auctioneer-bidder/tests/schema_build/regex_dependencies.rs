//! Resolved source context from the existing offline, locked metadata owner.

use super::{cargo, root, schema_regex_vendor, successful};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use toml::Value as Toml;

const REFUSAL: &str = "metadata does not resolve the engine's locked regex-syntax";
const SYNTAX_VERSION: &str = "0.8.12";
const SYNTAX_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";
const SYNTAX_CHECKSUM: &str = "d902046e6a28a1e4c3cab97258a0ce11e994903f01c2d97a9447a0b3df85f4f6";

#[derive(Debug, Eq, PartialEq)]
pub struct ResolvedRegexDependencies {
    pub engine_features: Vec<String>,
    pub syntax_root: PathBuf,
}

pub fn resolved_regex_dependencies(
    source: &schema_regex_vendor::VerifiedSource,
) -> Result<ResolvedRegexDependencies, String> {
    let workspace = root();
    let lock = fs::read_to_string(workspace.join("Cargo.lock"))
        .map_err(|error| format!("resolved regex-syntax lockfile: {error}"))?;
    let metadata = successful(
        cargo(&workspace).args(["metadata", "--offline", "--locked", "--format-version", "1"]),
        Duration::from_secs(60_u64),
    )?;
    let dependencies = parse(&metadata.stdout, &source.canonical_root, &lock)?;
    let canonical = fs::canonicalize(&dependencies.syntax_root)
        .map_err(|_error| "resolved regex-syntax source missing")?;
    if canonical != dependencies.syntax_root {
        return Err("resolved regex-syntax source is not canonical".into());
    }
    Ok(dependencies)
}

fn verify_syntax_lock(lock: &str) -> Result<(), String> {
    let Ok(document) = toml::from_str::<Toml>(lock) else {
        return Err(REFUSAL.into());
    };
    let packages = document
        .get("package")
        .and_then(Toml::as_array)
        .ok_or(REFUSAL)?;
    let matching = packages
        .iter()
        .filter(|package| package.get("name").and_then(Toml::as_str) == Some("regex-syntax"))
        .collect::<Vec<_>>();
    let [syntax] = matching.as_slice() else {
        return Err(REFUSAL.into());
    };
    for (field, expected) in [
        ("version", SYNTAX_VERSION),
        ("source", SYNTAX_SOURCE),
        ("checksum", SYNTAX_CHECKSUM),
    ] {
        if syntax.get(field).and_then(Toml::as_str) != Some(expected) {
            return Err(REFUSAL.into());
        }
    }
    Ok(())
}

fn parse(bytes: &[u8], vendor: &Path, lock: &str) -> Result<ResolvedRegexDependencies, String> {
    let engine_features = schema_regex_vendor::verify_metadata(bytes, vendor)?;
    verify_syntax_lock(lock)?;
    let refused = REFUSAL;
    let document: Value = serde_json::from_slice(bytes).map_err(|_error| refused)?;
    let packages = document["packages"].as_array().ok_or(refused)?;
    let candidates = packages
        .iter()
        .filter(|package| package["name"] == "regex-syntax")
        .collect::<Vec<_>>();
    let [syntax] = candidates.as_slice() else {
        return Err(refused.into());
    };
    if syntax["version"] != SYNTAX_VERSION || syntax["source"] != SYNTAX_SOURCE {
        return Err(refused.into());
    }
    let manifest = Path::new(syntax["manifest_path"].as_str().ok_or(refused)?);
    if !manifest.is_absolute()
        || manifest.file_name().is_none_or(|name| name != "Cargo.toml")
        || manifest
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(refused.into());
    }
    let syntax_root = manifest.parent().ok_or(refused)?.to_owned();
    if syntax_root.join("Cargo.toml").as_os_str() != manifest.as_os_str() {
        return Err(refused.into());
    }
    let syntax_id = syntax["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or(refused)?;
    let engine = packages
        .iter()
        .find(|package| package["name"] == "regex-automata")
        .ok_or(refused)?;
    let nodes = document["resolve"]["nodes"].as_array().ok_or(refused)?;
    if nodes.iter().filter(|node| node["id"] == syntax_id).count() != 1_usize {
        return Err(refused.into());
    }
    let node = nodes
        .iter()
        .find(|node| node["id"] == engine["id"])
        .ok_or(refused)?;
    let edges = node["deps"]
        .as_array()
        .ok_or(refused)?
        .iter()
        .filter(|edge| edge["name"] == "regex_syntax" || edge["pkg"] == syntax_id)
        .collect::<Vec<_>>();
    let [edge] = edges.as_slice() else {
        return Err(refused.into());
    };
    if edge["name"] != "regex_syntax"
        || edge["pkg"] != syntax_id
        || !edge["dep_kinds"].as_array().is_some_and(|kinds| {
            kinds.iter().any(|kind| {
                kind.get("kind").is_some_and(Value::is_null)
                    && kind.get("target").is_some_and(Value::is_null)
            })
        })
    {
        return Err(refused.into());
    }
    Ok(ResolvedRegexDependencies {
        engine_features,
        syntax_root,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({"packages":[
            {"name":"regex-automata","version":"0.4.19","source":null,
                "manifest_path":"/fake/vendor/regex-automata/Cargo.toml","id":"engine"},
            {"name":"regex-syntax","version":"0.8.12",
                "source":"registry+https://github.com/rust-lang/crates.io-index",
                "manifest_path":"/fake/registry/regex-syntax-0.8.12/Cargo.toml","id":"syntax"}],
            "resolve":{"nodes":[
                {"id":"engine","features":schema_regex_vendor::FEATURES,
                    "deps":[{"name":"regex_syntax","pkg":"syntax","dep_kinds":[{"kind":null,"target":null}]}]},
                {"id":"syntax","features":["std","unicode"],"deps":[]}]}})
    }

    fn lock_fixture() -> Value {
        json!({"version":4_u32,"package":[{
            "name":"regex-syntax","version":"0.8.12",
            "source":"registry+https://github.com/rust-lang/crates.io-index",
            "checksum":"d902046e6a28a1e4c3cab97258a0ce11e994903f01c2d97a9447a0b3df85f4f6"
        }]})
    }

    fn inspect_with_lock(value: &Value, lock: &str) -> Result<ResolvedRegexDependencies, String> {
        parse(
            &serde_json::to_vec(value).map_err(|error| error.to_string())?,
            Path::new("/fake/vendor/regex-automata"),
            lock,
        )
    }

    fn inspect(value: &Value) -> Result<ResolvedRegexDependencies, String> {
        let lock = toml::to_string(&lock_fixture()).map_err(|error| error.to_string())?;
        inspect_with_lock(value, &lock)
    }

    #[test]
    fn actual_metadata_selects_features_and_manifest_parent_together() {
        let dependencies = inspect(&fixture()).unwrap();
        assert_eq!(
            dependencies.syntax_root,
            Path::new("/fake/registry/regex-syntax-0.8.12")
        );
        assert_eq!(dependencies.engine_features.len(), 25_usize);
        assert!(dependencies.engine_features.contains(&"syntax".to_owned()));
    }

    #[test]
    fn wrong_duplicate_and_missing_syntax_packages_refuse_without_filesystem_or_cargo() {
        for index in 0_usize..7_usize {
            let mut value = fixture();
            match index {
                0_usize => {
                    value["packages"].as_array_mut().unwrap().pop().unwrap();
                }
                1_usize => {
                    let duplicate = value["packages"][1_usize].clone();
                    value["packages"].as_array_mut().unwrap().push(duplicate);
                }
                2_usize => value["packages"][1_usize]["version"] = json!("0.8.10"),
                3_usize => value["packages"][1_usize]["source"] = Value::Null,
                4_usize => {
                    value["packages"][1_usize]["manifest_path"] = json!("relative/Cargo.toml");
                }
                5_usize => {
                    value["packages"][1_usize]["manifest_path"] =
                        json!("/fake/../other/Cargo.toml");
                }
                _ => value["packages"][1_usize]["manifest_path"] = json!("/fake/not-a-manifest"),
            }
            assert_eq!(
                inspect(&value).unwrap_err(),
                "metadata does not resolve the engine's locked regex-syntax"
            );
        }
    }

    #[test]
    fn previous_syntax_release_cannot_match_the_current_locked_engine() {
        let mut value = fixture();
        value["packages"][1_usize]["version"] = json!("0.8.11");
        value["packages"][1_usize]["manifest_path"] =
            json!("/fake/registry/regex-syntax-0.8.11/Cargo.toml");
        assert_eq!(
            inspect(&value).unwrap_err(),
            "metadata does not resolve the engine's locked regex-syntax"
        );
    }

    #[test]
    fn missing_duplicate_stale_or_substituted_syntax_lock_entries_refuse() {
        for index in 0_usize..8_usize {
            let mut value = lock_fixture();
            match index {
                0_usize => value["package"] = json!([]),
                1_usize => {
                    let duplicate = value["package"][0_usize].clone();
                    value["package"].as_array_mut().unwrap().push(duplicate);
                }
                2_usize => value["package"][0_usize]["version"] = json!("0.8.11"),
                3_usize => {
                    value["package"][0_usize]["source"] =
                        json!("registry+https://example.invalid/index");
                }
                4_usize => {
                    value["package"][0_usize]["checksum"] =
                        json!("d6f6ff9a378485b298a5286656da665ba74413d36db0979633275d2e708145d4");
                }
                5_usize => {
                    assert!(
                        value["package"][0_usize]
                            .as_object_mut()
                            .unwrap()
                            .remove("checksum")
                            .is_some()
                    );
                }
                6_usize => value["package"][0_usize]["name"] = json!("unrelated"),
                _ => value["package"] = json!({}),
            }
            let lock = toml::to_string(&value).unwrap();
            assert_eq!(
                inspect_with_lock(&fixture(), &lock).unwrap_err(),
                "metadata does not resolve the engine's locked regex-syntax",
                "lock axis {index}"
            );
        }
        for malformed in ["", "[[package", "package = []"] {
            assert_eq!(
                inspect_with_lock(&fixture(), malformed).unwrap_err(),
                "metadata does not resolve the engine's locked regex-syntax"
            );
        }
    }

    #[test]
    fn missing_duplicate_substituted_and_build_only_dependency_edges_refuse() {
        for index in 0_usize..5_usize {
            let mut value = fixture();
            match index {
                0_usize => value["resolve"]["nodes"][0_usize]["deps"] = json!([]),
                1_usize => {
                    let edge = value["resolve"]["nodes"][0_usize]["deps"][0_usize].clone();
                    value["resolve"]["nodes"][0_usize]["deps"]
                        .as_array_mut()
                        .unwrap()
                        .push(edge);
                }
                2_usize => {
                    value["resolve"]["nodes"][0_usize]["deps"][0_usize]["pkg"] = json!("other");
                }
                3_usize => {
                    value["resolve"]["nodes"][0_usize]["deps"][0_usize]["dep_kinds"][0_usize]["kind"] =
                        json!("build");
                }
                _ => {
                    value["resolve"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .pop()
                        .unwrap();
                }
            }
            assert_eq!(
                inspect(&value).unwrap_err(),
                "metadata does not resolve the engine's locked regex-syntax"
            );
        }
    }
}

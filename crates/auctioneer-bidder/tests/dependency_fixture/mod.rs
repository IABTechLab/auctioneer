//! Reviewed patches and lock identities for standalone Cargo test roots.

#![cfg(test)]

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn command(directory: &Path, action: &str) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .args([action, "--offline", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_BUILD_JOBS", "2")
        .env("CARGO_BUILD_WARNINGS", "deny")
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .current_dir(directory);
    command
}

pub fn prepare(directory: &Path, root: &Path, manifest: &mut toml::Value) -> String {
    let workspace: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    // Path dependencies do not inherit their workspace's root patch table.
    let mut patches = workspace["patch"].clone();
    for (_, registry) in patches.as_table_mut().unwrap().iter_mut() {
        for (_, declaration) in registry.as_table_mut().unwrap().iter_mut() {
            if let Some(path) = declaration.get_mut("path") {
                let absolute = root.join(path.as_str().unwrap()).canonicalize().unwrap();
                *path = absolute.to_str().unwrap().into();
            }
        }
    }
    manifest
        .as_table_mut()
        .unwrap()
        .insert("patch".into(), patches);
    fs::write(
        directory.join("Cargo.toml"),
        toml::to_string(manifest).unwrap(),
    )
    .unwrap();
    let reviewed = fs::read_to_string(root.join("Cargo.lock")).unwrap();
    fs::write(directory.join("Cargo.lock"), &reviewed).unwrap();
    reviewed
}

#[derive(Deserialize, Eq, PartialEq, Ord, PartialOrd)]
struct LockIdentity {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

#[derive(Deserialize)]
struct Lockfile {
    package: Vec<LockIdentity>,
}

pub fn verify_lock(child: &str, reviewed: &str, own_name: &str) -> Result<(), &'static str> {
    let approved: Lockfile = toml::from_str(reviewed).map_err(|_error| "invalid reviewed lock")?;
    let actual: Lockfile = toml::from_str(child).map_err(|_error| "invalid fixture lock")?;
    let own = LockIdentity {
        name: own_name.to_owned(),
        version: "0.0.0".to_owned(),
        source: None,
        checksum: None,
    };
    if actual.package.iter().filter(|row| **row == own).count() != 1 {
        return Err("fixture package identity mismatch");
    }
    let mut accepted: BTreeSet<_> = approved.package.into_iter().collect();
    accepted.insert(own);
    if actual.package.iter().any(|row| !accepted.contains(row)) {
        return Err("unreviewed fixture dependency");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{command, verify_lock};
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn standalone_builds_explicitly_deny_warnings() {
        let build = command(Path::new("."), "build");
        assert_eq!(
            build.get_envs().find_map(|(key, value)| {
                (key == OsStr::new("CARGO_BUILD_WARNINGS")).then_some(value)
            }),
            Some(Some(OsStr::new("deny")))
        );
    }

    fn fixture() -> (toml::Value, toml::Value) {
        let reviewed = toml::Value::Table(toml::toml! {
            version = 4_i64
            [[package]]
            name = "fake_dependency"
            version = "1.2.3"
            source = "registry+https://github.com/rust-lang/crates.io-index"
            checksum = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        });
        let mut child = reviewed.clone();
        child["package"]
            .as_array_mut()
            .unwrap()
            .push(toml::Value::Table(toml::toml! {
                name = "fake_fixture"
                version = "0.0.0"
            }));
        (reviewed, child)
    }

    #[test]
    fn fixture_lock_accepts_reviewed_dependencies_and_its_own_new_root() {
        let (reviewed, child) = fixture();
        assert_eq!(
            verify_lock(
                &toml::to_string(&child).unwrap(),
                &toml::to_string(&reviewed).unwrap(),
                "fake_fixture"
            ),
            Ok(())
        );
    }

    #[test]
    fn fixture_lock_rejects_dependency_name_version_source_and_checksum_drift() {
        let (reviewed, child) = fixture();
        let mut added = child.clone();
        let mut extra = added["package"][0].clone();
        extra["name"] = "fake_extra_dependency".into();
        added["package"].as_array_mut().unwrap().push(extra);
        assert_eq!(
            verify_lock(
                &toml::to_string(&added).unwrap(),
                &toml::to_string(&reviewed).unwrap(),
                "fake_fixture"
            ),
            Err("unreviewed fixture dependency")
        );
        for (field, changed) in [
            ("name", "fake_unreviewed"),
            ("version", "1.2.4"),
            (
                "source",
                "git+https://example.test/fake_dependency#fake_revision",
            ),
            ("checksum", "fake_changed_checksum"),
        ] {
            let mut tampered = child.clone();
            assert!(tampered["package"][0].get(field).is_some());
            tampered["package"][0][field] = changed.into();
            assert_eq!(
                verify_lock(
                    &toml::to_string(&tampered).unwrap(),
                    &toml::to_string(&reviewed).unwrap(),
                    "fake_fixture"
                ),
                Err("unreviewed fixture dependency"),
                "{field}"
            );
        }
    }

    #[test]
    fn fixture_lock_rejects_missing_duplicate_and_mistyped_fixture_roots() {
        let (reviewed, child) = fixture();
        assert_eq!(
            verify_lock(
                &toml::to_string(&reviewed).unwrap(),
                &toml::to_string(&reviewed).unwrap(),
                "fake_fixture"
            ),
            Err("fixture package identity mismatch")
        );
        let mut duplicate = child.clone();
        let own = duplicate["package"].as_array().unwrap()[1].clone();
        duplicate["package"].as_array_mut().unwrap().push(own);
        assert_eq!(
            verify_lock(
                &toml::to_string(&duplicate).unwrap(),
                &toml::to_string(&reviewed).unwrap(),
                "fake_fixture"
            ),
            Err("fixture package identity mismatch")
        );
        let mut changed = child;
        changed["package"][1]["version"] = "0.0.1".into();
        assert_eq!(
            verify_lock(
                &toml::to_string(&changed).unwrap(),
                &toml::to_string(&reviewed).unwrap(),
                "fake_fixture"
            ),
            Err("fixture package identity mismatch")
        );
    }
}

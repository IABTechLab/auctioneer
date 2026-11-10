//! Compiler and runtime controls for the generic public API across feature selections.
//!
//! The public discovery API remains available with or without insecure bidders;
//! discovered factory membership, not concrete symbol visibility, follows the
//! feature. Distinct child packages and artifact checks establish compilation,
//! and executing each child verifies its selected dependency's registry.

mod dependency_fixture;

#[cfg(test)]
mod tests {
    use super::dependency_fixture;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{self, Command};
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Per-invocation nonce, so the default-features and
    /// `--features insecure-bidders` runs of this binary cannot collide on
    /// a fixture directory.
    static FIXTURE_NONCE: AtomicU64 = AtomicU64::new(0);

    fn crate_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn workspace_root() -> PathBuf {
        crate_dir()
            .parent()
            .and_then(Path::parent)
            .expect("crate two parents up = workspace root")
            .to_owned()
    }

    /// A fixture crate that depends on `auctioneer-bidder` by path and does
    /// one thing in `main`.
    ///
    /// `[workspace]` is empty on purpose: without it cargo walks up, finds
    /// the real workspace manifest, and refuses the fixture as a member it
    /// does not list.
    ///
    /// Each fixture gets its own PACKAGE NAME, not just its own directory.
    /// Cargo fingerprints a build unit by package name and feature set, not
    /// by manifest path, so two fixtures sharing the name
    /// `auctioneer-gate-probe` in one `--target-dir` could reuse another case's
    /// binary rather than compile the intended source. The name carries the
    /// label so each feature selection has its own checked artifact.
    struct Fixture {
        dir: PathBuf,
        package: String,
        reviewed_lock: String,
    }

    impl Fixture {
        /// A fixture whose dependency takes `auctioneer-bidder`'s DEFAULT
        /// features.
        fn new(label: &str, body: &str) -> Self {
            Self::build_fixture(label, body, false)
        }

        /// A fixture whose dependency asks for `insecure-bidders`.
        ///
        /// The feature is requested in the FIXTURE's manifest, not
        /// inherited from the outer run: both selections are exercised
        /// regardless of the parent test binary's features.
        fn with_feature(label: &str, body: &str) -> Self {
            Self::build_fixture(label, body, true)
        }

        fn build_fixture(label: &str, body: &str, insecure: bool) -> Self {
            let nonce = FIXTURE_NONCE.fetch_add(1, Ordering::Relaxed);
            // The package name carries the case label so the cases cannot
            // share a cargo build unit, and the pid so a default-features
            // run and an insecure-bidders run of this binary cannot either.
            let package = format!("auctioneer-gate-probe-{label}-{}", process::id());
            let dir = env::temp_dir().join(format!("{package}.{nonce}"));
            let src = dir.join("src");
            fs::create_dir_all(&src).expect("create the fixture source dir");
            let bidder = crate_dir();
            let root = workspace_root();
            let mut manifest = toml::Value::Table(toml::toml! {
                [package]
                name = ""
                edition = "2024"
                version = "0.0.0"
                publish = false
                [workspace]
                [dependencies.auctioneer-bidder]
                path = ""
            });
            manifest["package"]["name"] = package.clone().into();
            manifest["dependencies"]["auctioneer-bidder"]["path"] = bidder.to_str().unwrap().into();
            if insecure {
                manifest["dependencies"]["auctioneer-bidder"]
                    .as_table_mut()
                    .unwrap()
                    .insert("features".into(), vec!["insecure-bidders"].into());
            }
            let reviewed_lock = dependency_fixture::prepare(&dir, &root, &mut manifest);
            fs::write(src.join("main.rs"), body).expect("write the fixture main");
            Self {
                dir,
                package,
                reviewed_lock,
            }
        }

        fn binary_path(&self) -> PathBuf {
            workspace_root()
                .join("target/insecure-gate-probe/debug")
                .join(format!("{}{}", self.package, env::consts::EXE_SUFFIX))
        }

        /// Build the fixture and return (success, combined output).
        ///
        /// The `--target-dir` is shared across cases and persists between
        /// runs: it is what turns the second and later invocations into
        /// cache hits, since every case shares `auctioneer-bidder`'s dependency
        /// closure. It sits under the workspace's own `target/`, so
        /// `cargo clean` reaches it.
        ///
        /// A reported success is checked against the artifact on disk. A
        /// `cargo build` that exits 0 having produced no binary is not
        /// evidence that the public API compiled.
        fn build(&self) -> (bool, String) {
            let target_dir = workspace_root().join("target").join("insecure-gate-probe");
            let binary = self.binary_path();
            drop(fs::remove_file(&binary));
            // Materialize only the standalone root; dependency identities must
            // remain in the reviewed closure before a locked build can start.
            let resolution = dependency_fixture::command(&self.dir, "update")
                .arg("--workspace")
                .output()
                .expect("materialize the offline fixture lock");
            assert!(
                resolution.status.success(),
                "{}",
                String::from_utf8_lossy(&resolution.stderr)
            );
            let lock = fs::read_to_string(self.dir.join("Cargo.lock")).unwrap();
            assert_eq!(
                dependency_fixture::verify_lock(&lock, &self.reviewed_lock, &self.package),
                Ok(())
            );
            let output = dependency_fixture::command(&self.dir, "build")
                .arg("--locked")
                .arg("--target-dir")
                .arg(&target_dir)
                .output()
                .expect("spawn the child cargo build");
            let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
            combined.push_str(&String::from_utf8_lossy(&output.stderr));
            // Not folded into the verdict: a missing binary after a
            // successful build is a BROKEN HARNESS, and reporting it as
            // a build failure would hide a broken child-build control.
            assert!(
                !output.status.success() || binary.is_file(),
                "the child cargo build exited 0 but left no binary at {}; the harness \
                 cannot verify the public API with a build that did nothing.\n{combined}",
                binary.display(),
            );
            (output.status.success(), combined)
        }

        fn build_and_run(&self, insecure: bool) {
            let (ok, build_log) = self.build();
            assert!(ok, "the generic public API child must build.\n{build_log}");
            let output = Command::new(self.binary_path())
                .arg(insecure.to_string())
                .current_dir(&self.dir)
                .output()
                .expect("run the generic public API child");
            assert!(
                output.status.success(),
                "the public API child rejected insecure-bidders={insecure}.\n{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Best effort: a leaked tempdir is noise, a panic in Drop
            // during an already-failing test hides the real verdict.
            drop(fs::remove_dir_all(&self.dir));
        }
    }

    /// A minimal public reference built through the same child-build path.
    const CONTROL_BODY: &str =
        "fn main() { let _: Option<auctioneer_bidder::BidderName> = None; }\n";

    const API_BODY: &str = r#"
fn main() {
    let insecure: bool = std::env::args().nth(1).unwrap().parse().unwrap();
    let declarations: &[auctioneer_bidder::BidderDeclaration] = auctioneer_bidder::bidder_declarations();
    let factories: &[auctioneer_bidder::BidderRegistration] = auctioneer_bidder::compiled_bidders();
    assert!(
        !declarations.is_empty(),
        "public declarations must not be empty"
    );
    let mut settings_owners = 0_usize;
    for declaration in declarations {
        let enabled = match declaration.required_feature {
            None => true,
            Some("insecure-bidders") => {
                insecure
            }
            Some(_) => panic!("unexpected factory feature"),
        };
        settings_owners += usize::from(enabled && declaration.has_settings);
        let matching: Vec<_> = factories
            .iter()
            .filter(|row| row.key == declaration.key)
            .collect();
        assert_eq!(
            matching.len(),
            usize::from(enabled),
            "factory membership: {}",
            declaration.key
        );
        if let Some(registration) = matching.first() {
            let _: Box<dyn auctioneer_bidder::Bidder> = (registration.factory)();
        }
    }
    assert!(settings_owners > 0, "typed settings owners must be available in both feature lanes");
    for registration in factories {
        assert!(
            declarations.iter().any(|row| row.key == registration.key),
            "undeclared factory: {}",
            registration.key
        );
    }
}
"#;

    #[test]
    fn standalone_feature_fixture_inherits_all_workspace_patch_paths() {
        let fixture = Fixture::new("patch-contract", CONTROL_BODY);
        let actual: toml::Value =
            toml::from_str(&fs::read_to_string(fixture.dir.join("Cargo.toml")).unwrap()).unwrap();
        let root = workspace_root();
        let reviewed: toml::Value =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        let expected = reviewed["patch"]["crates-io"].as_table().unwrap();
        assert!(!expected.is_empty(), "before-state patched dependencies");
        let patches = actual
            .get("patch")
            .expect("standalone root must retain patches");
        let actual_patches = patches["crates-io"].as_table().unwrap();
        assert_eq!(actual_patches.len(), expected.len());
        for (name, declaration) in expected {
            let relative = declaration["path"].as_str().unwrap();
            assert_eq!(
                actual_patches[name]["path"].as_str().unwrap(),
                root.join(relative)
                    .canonicalize()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "patch owner {name}"
            );
        }
    }

    #[test]
    fn standalone_feature_fixture_seeds_the_reviewed_workspace_lock() {
        let fixture = Fixture::new("lock-contract", CONTROL_BODY);
        assert_eq!(
            fs::read(fixture.dir.join("Cargo.lock")).expect("standalone lock must be seeded"),
            fs::read(workspace_root().join("Cargo.lock")).unwrap()
        );
    }

    #[test]
    fn the_harness_builds_an_ungated_reference() {
        let control = Fixture::new("control", CONTROL_BODY);
        let (ok, output) = control.build();
        assert!(
            ok,
            "the ungated control must build or the gate verdict is invalid.\n{output}"
        );
    }

    #[test]
    fn default_features_keep_the_public_api_and_exclude_gated_factories() {
        Fixture::new("api-default", API_BODY).build_and_run(false);
    }

    #[test]
    fn insecure_feature_keeps_the_public_api_and_includes_gated_factories() {
        Fixture::with_feature("api-insecure", API_BODY).build_and_run(true);
    }
}

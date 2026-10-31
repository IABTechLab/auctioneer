//! Maintenance must remain buildable when the application cannot bake its archive.

#[cfg(test)]
mod tests {
    use fs2::FileExt as _;
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs::{self, OpenOptions};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::str::from_utf8;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn maintenance_dependency_graph_excludes_the_application_and_embedded_archive() {
        let output = Command::new(env!("CARGO"))
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--no-default-features",
            ])
            .current_dir(root())
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
        let cli_manifest = metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["name"] == "auctioneer-cli")
            .unwrap();
        assert_eq!(cli_manifest["features"]["default"], json!(["application"]));
        // Metadata can list weak optional edges resolved elsewhere in the
        // workspace. Ask Cargo for this package's selected build graph instead.
        let selected = Command::new(env!("CARGO"))
            .args([
                "tree",
                "--locked",
                "--offline",
                "-p",
                "auctioneer-cli",
                "--no-default-features",
                "--edges",
                "normal,build",
                "--prefix",
                "none",
                "--format",
                "{p}",
            ])
            .current_dir(root())
            .output()
            .unwrap();
        assert!(selected.status.success(), "{selected:?}");
        let tree = from_utf8(&selected.stdout).unwrap();
        let names: BTreeSet<_> = tree
            .lines()
            .map(|line| line.split_whitespace().next().expect("Cargo package row"))
            .collect();
        assert!(names.contains("auctioneer-cli"));
        for required in ["auctioneer-gvl-schema", "auctioneer-openrtb-catalogue"] {
            assert!(
                names.contains(required),
                "missing maintenance owner {required}"
            );
        }
        for forbidden in [
            "auctioneer-config",
            "auctioneer-core",
            "auctioneer-privacy",
            "auctioneer-exchange",
        ] {
            assert!(
                !names.contains(forbidden),
                "maintenance reaches {forbidden}"
            );
        }
    }

    fn copied_workspace(destination: &Path) {
        let source_root = root();
        let listing = Command::new("git")
            .args([
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ])
            .current_dir(&source_root)
            .output()
            .unwrap();
        assert!(listing.status.success(), "{listing:?}");
        for relative_bytes in listing
            .stdout
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
        {
            let relative = Path::new(from_utf8(relative_bytes).unwrap());
            let source = source_root.join(relative);
            if !source.exists() {
                continue;
            }
            let output = destination.join(relative);
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            fs::copy(source, output).unwrap();
        }
    }

    fn child_cargo(workspace: &Path, target: &Path, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO"))
            .args(arguments)
            .args(["--locked", "--offline", "-j", "2", "--target-dir"])
            .arg(target)
            .env_remove("CARGO_BUILD_TARGET")
            .env_remove("AUCTIONEER__GVL__ARCHIVE__DIR")
            .current_dir(workspace)
            .output()
            .unwrap()
    }

    #[test]
    fn corrupt_embedded_archive_cannot_prevent_building_the_repair_command() {
        let original_root = root();
        let relative = "crates/auctioneer-config/data/gvl_archive/gcl.json";
        let original = fs::read(original_root.join(relative)).unwrap();
        // Build scripts embed CARGO_MANIFEST_DIR; a reusable cache needs a stable source path.
        let boundary = original_root.join("target/cli-maintenance-build");
        fs::create_dir_all(&boundary).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(boundary.join("workspace.lock"))
            .unwrap();
        lock.lock_exclusive().unwrap();
        let copy = boundary.join("workspace");
        if copy.exists() {
            fs::remove_dir_all(&copy).unwrap();
        }
        copied_workspace(&copy);
        fs::write(copy.join(relative), b"corrupted GCL fixture").unwrap();
        let target = boundary.join("build");
        let broken = child_cargo(&copy, &target, &["check", "-p", "auctioneer-config"]);
        assert!(
            !broken.status.success(),
            "a corrupt archive unexpectedly baked"
        );
        let diagnostic = String::from_utf8(broken.stderr).unwrap();
        assert!(diagnostic.contains("auctioneer-config"), "{diagnostic}");
        assert!(
            diagnostic.contains("SHA mismatch for gcl.json")
                || diagnostic.contains("manifest validation:"),
            "control failed outside archive validation: {diagnostic}"
        );
        let built = child_cargo(
            &copy,
            &target,
            &[
                "build",
                "-p",
                "auctioneer-cli",
                "--no-default-features",
                "--message-format=json",
            ],
        );
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let binary = String::from_utf8(built.stdout)
            .unwrap()
            .lines()
            .filter_map(|line| {
                let record: Value = serde_json::from_str(line).unwrap();
                (record["reason"] == "compiler-artifact"
                    && record["target"]["name"] == "auctioneer-cli")
                    .then(|| record["executable"].as_str().map(PathBuf::from))
                    .flatten()
            })
            .next_back()
            .unwrap();
        let help = Command::new(&binary)
            .args(["gvl", "refresh", "--help"])
            .output()
            .unwrap();
        assert!(help.status.success(), "{help:?}");
        assert!(
            String::from_utf8(help.stdout)
                .unwrap()
                .contains("--max-age-days")
        );
        let refusal = Command::new(binary)
            .args(["gvl", "refresh", "--max-age-days", "181"])
            // A second pre-network refusal prevents accidental network use if the first guard regresses.
            .env("AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB", "25")
            .output()
            .unwrap();
        assert_eq!(refusal.status.code(), Some(1_i32));
        assert!(
            String::from_utf8(refusal.stderr)
                .unwrap()
                .contains("exceeds 180")
        );
        assert_eq!(fs::read(original_root.join(relative)).unwrap(), original);
        assert_eq!(
            fs::read(copy.join(relative)).unwrap(),
            b"corrupted GCL fixture"
        );
    }
}

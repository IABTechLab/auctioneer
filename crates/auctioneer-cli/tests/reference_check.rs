#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process::{self, Command, Output},
    };

    use serde_json::Value;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn cli(args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
            .arg("reference")
            .args(args)
            .output()
            .unwrap()
    }

    fn paths() -> Vec<String> {
        let manifest: Value =
            serde_json::from_str(include_str!("../src/maintenance/reference/captures.json"))
                .unwrap();
        manifest["captures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|capture| capture["path"].as_str().unwrap().to_owned())
            .collect()
    }

    fn snapshot(root: &Path) -> Vec<Vec<u8>> {
        paths()
            .iter()
            .map(|path| fs::read(root.join(path)).unwrap())
            .collect()
    }

    #[test]
    fn real_cli_checks_every_staged_capture_read_only_and_does_not_bless() {
        let root = root();
        let staged = env::temp_dir().join(format!("auctioneer-reference-cli-{}", process::id()));
        fs::create_dir_all(&staged).unwrap();
        for path in paths() {
            let destination = staged.join(&path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(root.join(path), destination).unwrap();
        }
        let root_arg = root.to_str().unwrap();
        let staged_arg = staged.to_str().unwrap();
        let original = snapshot(&root);
        let valid = cli(&["check", "--root", root_arg]);
        assert!(
            valid.status.success(),
            "{}",
            String::from_utf8_lossy(&valid.stderr)
        );
        assert_eq!(
            valid.stdout,
            b"verified 10 frozen captures; no files written\n"
        );
        let args = ["check", "--root", staged_arg];
        let valid_staged = cli(&args);
        assert!(
            valid_staged.status.success(),
            "{}",
            String::from_utf8_lossy(&valid_staged.stderr)
        );
        assert_eq!(snapshot(&staged), original);
        for path in paths() {
            let destination = staged.join(&path);
            let original_bytes = fs::read(&destination).unwrap();
            let mut changed = original_bytes.clone();
            changed[0] ^= 1;
            fs::write(&destination, &changed).unwrap();
            let changed_result = cli(&args);
            assert_eq!(
                changed_result.status.code(),
                Some(1_i32),
                "accepted changed {path}"
            );
            assert!(changed_result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&changed_result.stderr).contains(&path));
            assert_eq!(fs::read(&destination).unwrap(), changed);
            fs::remove_file(&destination).unwrap();
            let missing_result = cli(&args);
            assert_eq!(
                missing_result.status.code(),
                Some(1_i32),
                "accepted missing {path}"
            );
            assert!(missing_result.stdout.is_empty());
            assert!(String::from_utf8_lossy(&missing_result.stderr).contains(&path));
            assert!(!destination.exists());
            fs::write(destination, original_bytes).unwrap();
        }
        assert_eq!(snapshot(&root), original);
        assert_eq!(snapshot(&staged), original);
        let fake_pins =
            serde_json::to_vec(&serde_json::json!({"schema_version": 0_u32, "captures": []}))
                .unwrap();
        for path in [
            "crates/auctioneer-reference-fixtures/captures.json",
            "crates/auctioneer-cli/src/maintenance/reference/captures.json",
        ] {
            let staged_pins = staged.join(path);
            fs::create_dir_all(staged_pins.parent().unwrap()).unwrap();
            fs::write(&staged_pins, &fake_pins).unwrap();
            assert!(
                cli(&args).status.success(),
                "staged pins cannot replace compiled-in pins"
            );
            assert_eq!(fs::read(&staged_pins).unwrap(), fake_pins);
        }
        assert!(!cli(&["refresh", "--root", root_arg]).status.success());
        fs::remove_dir_all(staged).unwrap();
    }

    #[test]
    fn redundant_staging_command_is_rejected_without_a_compatibility_alias() {
        let root = root();
        let root_arg = root.to_str().unwrap();
        for command in ["validate", "refresh", "check-capture"] {
            let output = cli(&[command, "--root", root_arg, "--capture-root", root_arg]);
            assert_eq!(output.status.code(), Some(2_i32));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
        }
    }
}

//! Compile the real owner in a scratch workspace, never mutate the active tree.

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::{Command, Output};

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for result in fs::read_dir(source).unwrap() {
            let entry = result.unwrap();
            let output = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &output);
            } else {
                fs::copy(entry.path(), output).unwrap();
            }
        }
    }

    fn check(root: &Path, target: &Path) -> Output {
        Command::new(env!("CARGO"))
            .args(["check", "--offline", "--lib", "--message-format=json"])
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(target)
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .output()
            .unwrap()
    }

    #[test]
    fn adding_an_isolate_term_requires_the_real_fold_and_export_to_classify_it() {
        let owner = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace = owner.parent().unwrap().parent().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let root = scratch.path();
        let mut manifest: toml::Value =
            toml::from_str(&fs::read_to_string(workspace.join("Cargo.toml")).unwrap()).unwrap();
        manifest["workspace"]["members"] = toml::Value::Array(vec![toml::Value::String(
            "crates/auctioneer-runtime".into(),
        )]);
        // Keep local patches bound to the real source tree, not an absent
        // directory in this deliberately runtime-only scratch workspace.
        for (_, registry) in manifest["patch"].as_table_mut().unwrap() {
            for (_, package) in registry.as_table_mut().unwrap() {
                if let Some(path) = package.get_mut("path") {
                    let source = workspace.join(path.as_str().unwrap());
                    *path = toml::Value::String(source.to_str().unwrap().to_owned());
                }
            }
        }
        fs::write(root.join("Cargo.toml"), toml::to_string(&manifest).unwrap()).unwrap();
        fs::copy(workspace.join("Cargo.lock"), root.join("Cargo.lock")).unwrap();
        let copied = root.join("crates/auctioneer-runtime");
        copy_tree(&owner.join("src"), &copied.join("src"));
        fs::copy(owner.join("Cargo.toml"), copied.join("Cargo.toml")).unwrap();
        let target = root.join("target");
        let baseline = check(root, &target);
        assert!(
            baseline.status.success(),
            "scratch baseline must compile: {}",
            String::from_utf8_lossy(&baseline.stderr)
        );

        let path = copied.join("src/budget.rs");
        let original = fs::read_to_string(&path).unwrap();
        let marker = "pub struct IsolateTerms {";
        assert_eq!(original.matches(marker).count(), 1);
        let changed = original.replacen(
            marker,
            "pub struct IsolateTerms {\n    pub unclassified_bytes: u64,",
            1,
        );
        fs::write(&path, &changed).unwrap();
        let mutated = check(root, &target);
        assert!(!mutated.status.success(), "unclassified field compiled");
        let diagnostics: Vec<serde_json::Value> = mutated
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice(line).ok())
            .filter(|line: &serde_json::Value| {
                line["reason"] == "compiler-message" && line["message"]["code"]["code"] == "E0027"
            })
            .collect();
        assert_eq!(
            diagnostics.len(),
            2,
            "both owner mappings must reject the new field"
        );
        let fold_start = changed.find("pub fn checked_total(").unwrap();
        let export_start = changed.find("pub const fn entries(").unwrap();
        let export_end = export_start + changed.get(export_start..).unwrap().find("\n}\n").unwrap();
        let mut locations = Vec::new();
        for diagnostic in diagnostics {
            assert!(
                diagnostic["message"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("unclassified_bytes")
            );
            let primary = diagnostic["message"]["spans"]
                .as_array()
                .unwrap()
                .iter()
                .find(|span| span["is_primary"] == true)
                .unwrap();
            assert!(
                primary["file_name"]
                    .as_str()
                    .unwrap()
                    .ends_with("src/budget.rs")
            );
            locations.push(usize::try_from(primary["byte_start"].as_u64().unwrap()).unwrap());
        }
        locations.sort_unstable();
        let [fold, export] = locations.as_slice() else {
            panic!("expected both owner diagnostics");
        };
        assert!((fold_start..export_start).contains(fold));
        assert!((export_start..export_end).contains(export));
    }
}

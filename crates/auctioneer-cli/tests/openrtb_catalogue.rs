//! Executable boundary: nonzero refusal and deterministic offline generation.

#[cfg(test)]
mod tests {
    use auctioneer_openrtb_catalogue::{Catalogue, SourceManifest};
    use sha2::{Digest as _, Sha256};
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{self, Command, Output};
    use std::sync::atomic::{AtomicU64, Ordering};

    const SOURCE: &str = include_str!("../../auctioneer-config-data/data/openrtb-2.6-source.md");
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Inputs(PathBuf);

    impl Inputs {
        fn new() -> Self {
            let directory = env::temp_dir().join(format!(
                "auctioneer-catalogue-command-{}-{}",
                process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&directory).unwrap();
            let manifest = SourceManifest {
                url: "https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/1debba8001f40eb14597102dec3205afddf811d4/2.6.md".to_owned(),
                release: "2.6-202606".to_owned(),
                commit: "1debba8001f40eb14597102dec3205afddf811d4".to_owned(),
                sha256: hex::encode(Sha256::digest(SOURCE.as_bytes())),
                reviewer: "Subprocess fixture only; not human inventory approval".to_owned(),
                review_date: "2026-10-09".to_owned(),
            };
            fs::write(
                directory.join("manifest.toml"),
                toml::to_string(&manifest).unwrap(),
            )
            .unwrap();
            fs::write(directory.join("source.md"), SOURCE).unwrap();
            Self(directory)
        }

        fn run(&self, mode: &str, epoch: &str, output: &Path) -> Output {
            Command::new(env!("CARGO_BIN_EXE_auctioneer-cli"))
                .args(["openrtb", "catalogue"])
                .arg(mode)
                .arg("--manifest")
                .arg(self.0.join("manifest.toml"))
                .arg("--source")
                .arg(self.0.join("source.md"))
                .arg(if mode == "generate" {
                    "--output"
                } else {
                    "--catalogue"
                })
                .arg(output)
                .env("SOURCE_DATE_EPOCH", epoch)
                .env("TZ", "UTC")
                .output()
                .unwrap()
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Inputs {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn success(output: &Output) {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn executable_reproduces_membership_without_rewriting_human_metadata() {
        let inputs = Inputs::new();
        let manifest = fs::read(inputs.0.join("manifest.toml")).unwrap();
        let first = inputs.0.join("first.toml");
        let second = inputs.0.join("second.toml");
        success(&inputs.run("generate", "0", &first));
        success(&inputs.run("generate", "4102444800", &second));
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        success(&inputs.run("check", "0", &first));
        let parsed: Catalogue = toml::from_str(&fs::read_to_string(&first).unwrap()).unwrap();
        assert_eq!(parsed.fields.len(), 79);
        assert_eq!(fs::read(inputs.0.join("manifest.toml")).unwrap(), manifest);
        assert_eq!(
            fs::read_to_string(inputs.0.join("source.md")).unwrap(),
            SOURCE
        );
    }

    #[test]
    fn executable_refuses_source_and_catalogue_mutations_with_diagnostics() {
        let inputs = Inputs::new();
        let catalogue = inputs.0.join("catalogue.toml");
        success(&inputs.run("generate", "0", &catalogue));
        let original = fs::read(&catalogue).unwrap();
        fs::write(inputs.0.join("source.md"), format!("{SOURCE}\n")).unwrap();
        for mode in ["check", "generate"] {
            let refused = inputs.run(mode, "0", &catalogue);
            assert_eq!(refused.status.code(), Some(1_i32));
            assert!(String::from_utf8_lossy(&refused.stderr).contains("digest"));
            assert_eq!(fs::read(&catalogue).unwrap(), original);
        }
        fs::write(inputs.0.join("source.md"), SOURCE).unwrap();
        fs::write(&catalogue, "fields = []\n").unwrap();
        let refused = inputs.run("check", "0", &catalogue);
        assert_eq!(refused.status.code(), Some(1_i32));
        assert!(String::from_utf8_lossy(&refused.stderr).contains("differs"));
        assert_eq!(fs::read_to_string(&catalogue).unwrap(), "fields = []\n");
    }

    #[test]
    fn executable_refuses_output_aliases_without_modifying_input() {
        let inputs = Inputs::new();
        for name in ["manifest.toml", "source.md"] {
            let path = inputs.0.join(name);
            let before = fs::read(&path).unwrap();
            let refused = inputs.run("generate", "0", &path);
            assert_eq!(refused.status.code(), Some(1_i32));
            assert!(String::from_utf8_lossy(&refused.stderr).contains("aliases"));
            assert_eq!(fs::read(&path).unwrap(), before);
        }
    }
}

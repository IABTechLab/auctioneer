//! Compiler controls over feature-gated exports of the shared functional fake.

#![cfg(all(unix, not(target_arch = "wasm32")))]

#[cfg(test)]
pub mod build_workspace;

#[cfg(test)]
mod tests {
    use super::build_workspace::{BuildWorkspace, logs, require_success};
    use std::fs;
    use std::io::ErrorKind;
    use std::process::Output;

    const PROBE: &str = "synthetic-feature-export-probe";
    const CONTROL_BODY: &str = "fn main() { let bidder = auctioneer_bidder::fake_synthetic::create_bidder(); assert_eq!(bidder.debug_body_schema(), auctioneer_bidder::DebugBodySchema::OpenRtb); }\n";
    const OPTIONAL_BODY: &str = "fn main() { let bidder = auctioneer_bidder::create_optional_fake(); assert_eq!(bidder.debug_body_schema(), auctioneer_bidder::DebugBodySchema::OpenRtb); }\n";

    fn fixture() -> BuildWorkspace {
        let workspace = BuildWorkspace::new_synthetic(PROBE, CONTROL_BODY);
        let library_path = workspace.bidder().join("src/lib.rs");
        let library = fs::read_to_string(&library_path).unwrap();
        fs::write(
            library_path,
            format!("{library}\n#[cfg(feature = \"insecure-bidders\")]\npub use fake_synthetic::create_bidder as create_optional_fake;\n"),
        ).unwrap();
        workspace
    }

    fn build(workspace: &BuildWorkspace, body: &str, feature: bool) -> Output {
        fs::write(workspace.bidder().join("src/bin/build_probe.rs"), body).unwrap();
        let binary = workspace.target().join("debug").join(PROBE);
        match fs::remove_file(&binary) {
            Ok(()) => (),
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => panic!("cannot remove stale probe artifact: {error}"),
        }
        let mut arguments = vec!["--quiet", "--bin", PROBE];
        if feature {
            arguments.extend(["--features", "insecure-bidders"]);
        }
        let output = workspace.cargo("build", &arguments, true);
        assert!(
            !output.status.success() || binary.is_file(),
            "successful compiler control produced no artifact: {}",
            logs(&output),
        );
        output
    }

    #[test]
    fn the_harness_builds_an_ungated_fake_factory() {
        let workspace = fixture();
        require_success(
            &build(&workspace, CONTROL_BODY, false),
            "ungated fake factory control",
        );
        workspace.report();
    }

    #[test]
    fn default_features_make_the_optional_fake_factory_unreferenceable() {
        let workspace = fixture();
        require_success(
            &build(&workspace, CONTROL_BODY, false),
            "same-workspace ungated control",
        );
        let output = build(&workspace, OPTIONAL_BODY, false);
        assert!(
            !output.status.success(),
            "default features exposed the optional factory: {}",
            logs(&output)
        );
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("E0425"),
            "wrong optional factory refusal: {}",
            logs(&output)
        );
        assert!(
            diagnostic.contains("create_optional_fake"),
            "resolution refusal named another symbol: {}",
            logs(&output)
        );
        workspace.report();
    }

    #[test]
    fn the_feature_makes_the_same_fake_factory_probe_compile() {
        let workspace = fixture();
        require_success(
            &build(&workspace, OPTIONAL_BODY, true),
            "feature-enabled identical fake factory probe",
        );
        workspace.report();
    }
}

//! Isolated feature boundaries, without changing the serving feature closure.

use super::*;

fn source(prefilter: bool, meta: bool) -> String {
    let prefilter_source = prefilter.then(|| {
        quote! {
            fn prefilter(value: &regex_automata::util::prefilter::Prefilter)
                -> regex_automata::util::prefilter::PrefilterResourceLayout {
                let report = value.resource_layout();
                let _: core::alloc::Layout = report.value();
                let _: core::alloc::Layout = report.matcher_payload();
                report
            }
        }
    });
    let meta_source = meta.then(|| {
        quote! {
            fn regex(value: &regex_automata::meta::Regex)
                -> regex_automata::meta::RegexResourceLayout {
                let report = value.resource_layout();
                let _: core::alloc::Layout = report.value();
                let _: core::alloc::Layout = report.regex_payload();
                let _: core::alloc::Layout = report.metadata_payload();
                let _: core::alloc::Layout = report.strategy_payload();
                report
            }
        }
    });
    let prefilter_reference = prefilter.then(|| quote! { let _ = prefilter; });
    let meta_reference = meta.then(|| quote! { let _ = regex; });
    quote! {
        #prefilter_source
        #meta_source
        fn main() {
            #prefilter_reference
            #meta_reference
            let _ = core::mem::size_of::<regex_automata::MatchKind>();
        }
    }
    .to_string()
}

fn check(directory: &Path) -> Output {
    run(
        cargo(directory).env("CARGO_BUILD_WARNINGS", "deny").args([
            "check",
            "--offline",
            "--locked",
            "-j2",
        ]),
        BUILD_TIMEOUT,
    )
    .unwrap()
}

fn assert_source_and_features(directory: &Path, vendor: &Path, expected: &[&str]) {
    let metadata_output = successful(
        cargo(directory).args(["metadata", "--offline", "--format-version", "1"]),
        Duration::from_secs(60),
    )
    .unwrap();
    let metadata: Json = serde_json::from_slice(&metadata_output.stdout).unwrap();
    let engines: Vec<_> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|package| package["name"] == "regex-automata")
        .collect();
    assert_eq!(engines.len(), 1);
    let engine = engines[0];
    assert_eq!(engine["version"], "0.4.18");
    assert_eq!(engine["source"], Json::Null);
    assert_eq!(
        engine["manifest_path"],
        vendor.join("Cargo.toml").to_str().unwrap()
    );
    let nodes: Vec<_> = metadata["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|node| node["id"] == engine["id"])
        .collect();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["features"], serde_json::json!(expected));

    let reviewed: BTreeSet<_> = packages(&fs::read_to_string(root().join("Cargo.lock")).unwrap())
        .unwrap()
        .into_iter()
        .collect();
    let actual = packages(&fs::read_to_string(directory.join("Cargo.lock")).unwrap()).unwrap();
    let mut seen = BTreeSet::new();
    for identity in actual {
        assert!(seen.insert(identity.clone()), "duplicate probe package");
        if identity
            == (
                "auctioneer-layout-feature-probe".into(),
                "0.0.0".into(),
                None,
                None,
            )
        {
            continue;
        }
        assert!(
            reviewed.contains(&identity),
            "unreviewed probe package: {identity:?}"
        );
        if identity.0 == "regex-automata" {
            assert_eq!(
                identity,
                ("regex-automata".into(), "0.4.18".into(), None, None)
            );
        } else {
            assert_eq!(
                identity.2.as_deref(),
                Some("registry+https://github.com/rust-lang/crates.io-index")
            );
        }
    }
}

#[test]
fn layout_api_preserves_no_alloc_alloc_only_and_minimal_meta_boundaries() {
    let verified = schema_regex_vendor::verify(&root()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join("src")).unwrap();
    for (features, expected, prefilter, meta) in [
        (vec![], vec![], false, false),
        (vec!["alloc"], vec!["alloc"], true, false),
        (
            vec!["meta"],
            vec!["alloc", "meta", "nfa-pikevm", "nfa-thompson", "syntax"],
            true,
            true,
        ),
    ] {
        let manifest_json = serde_json::json!({
            "package": {"name": "auctioneer-layout-feature-probe", "version": "0.0.0", "edition": "2021"},
            "workspace": {},
            "dependencies": {"regex-automata": {
                "path": verified.canonical_root, "default-features": false, "features": features
            }}
        });
        let manifest: Value = serde_json::from_value(manifest_json).unwrap();
        fs::write(
            directory.path().join("Cargo.toml"),
            toml::to_string(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(
            directory.path().join("Cargo.lock"),
            fs::read(root().join("Cargo.lock")).unwrap(),
        )
        .unwrap();
        fs::write(
            directory.path().join("src/main.rs"),
            source(prefilter, meta),
        )
        .unwrap();
        assert_source_and_features(directory.path(), &verified.canonical_root, &expected);
        let output = check(directory.path());
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        if features.is_empty() {
            for (negative, diagnostics) in [
                (
                    quote! {
                        fn main() { let _: Option<regex_automata::util::prefilter::PrefilterResourceLayout> = None; }
                    },
                    vec!["error[E0425]", "PrefilterResourceLayout"],
                ),
                (
                    quote! {
                        fn probe(value: &regex_automata::util::prefilter::Prefilter) { let _ = value.resource_layout(); }
                        fn main() {}
                    },
                    vec!["error[E0599]", "resource_layout"],
                ),
            ] {
                fs::write(directory.path().join("src/main.rs"), negative.to_string()).unwrap();
                let refusal = check(directory.path());
                assert!(!refusal.status.success());
                let diagnostic = String::from_utf8_lossy(&refusal.stderr);
                for required in diagnostics {
                    assert!(diagnostic.contains(required), "{diagnostic}");
                }
            }
        }
    }
}

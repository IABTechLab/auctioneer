//! Incremental executable discovery evidence for a noninstalled validated registry plan.

#![cfg(all(unix, not(target_arch = "wasm32")))]

#[cfg(test)]
mod build_workspace;
#[cfg(test)]
mod registry_workspace;

#[cfg(test)]
mod tests {
    use super::registry_workspace::{Execution, Expected, Harness};
    use std::fs;
    use toml::Value;

    const ALPHA: Execution<'_> = Execution {
        name: "fake_discovery_alpha",
        key: "fake_discovery_alpha",
        marker: "alpha-v1",
        endpoint: "https://alpha.discovery.invalid/openrtb2",
        ordinal: 1,
    };

    const BETA: Execution<'_> = Execution {
        name: "fake_discovery_beta",
        key: "fake_discovery_beta",
        marker: "beta-v1",
        endpoint: "https://beta.discovery.invalid/openrtb2",
        ordinal: 1,
    };

    #[test]
    fn incremental_discovery_executes_changed_artifacts() {
        let harness = Harness::new();
        let initial = initial_setup(&harness);
        let both = second_impl(&harness, &initial);
        edit_rename_remove(&harness, &initial, &both);
        let mirror = metadata_only(&harness, &initial);
        optional_feature(&harness, &mirror);
        harness.report();
    }

    fn initial_setup(harness: &Harness) -> Expected<'static> {
        harness.add_source("fake_discovery_alpha", "alpha-v1", None);
        harness.add_metadata(
            "fake_discovery_alpha",
            "fake_discovery_alpha",
            "https://alpha.discovery.invalid/openrtb2",
            false,
        );
        let initial = Expected {
            factories: &["fake_discovery_alpha"],
            definitions: &["fake_discovery_alpha"],
            bindings: &[("fake_discovery_alpha", "fake_discovery_alpha")],
            declarations: &[("fake_discovery_alpha", None)],
            metadata: &[("fake_discovery_alpha", "fake_discovery_alpha", None)],
            executions: &[ALPHA],
            execution_count: 1,
            feature: false,
        };
        harness.run(&initial, "initial unknown implementation");
        initial
    }

    fn second_impl(harness: &Harness, initial: &Expected<'static>) -> Expected<'static> {
        // A new factory candidate has no effective public definitions yet.
        harness.add_source("fake_discovery_beta", "beta-v1", None);
        let source_added = Expected {
            factories: &["fake_discovery_alpha", "fake_discovery_beta"],
            declarations: &[
                ("fake_discovery_alpha", None),
                ("fake_discovery_beta", None),
            ],
            ..*initial
        };
        harness.run(
            &source_added,
            "source/declaration addition without metadata",
        );
        harness.add_metadata(
            "fake_discovery_beta",
            "fake_discovery_beta",
            "https://beta.discovery.invalid/openrtb2",
            false,
        );
        let both = Expected {
            definitions: &["fake_discovery_alpha", "fake_discovery_beta"],
            bindings: &[
                ("fake_discovery_alpha", "fake_discovery_alpha"),
                ("fake_discovery_beta", "fake_discovery_beta"),
            ],
            metadata: &[
                ("fake_discovery_alpha", "fake_discovery_alpha", None),
                ("fake_discovery_beta", "fake_discovery_beta", None),
            ],
            executions: &[ALPHA, BETA],
            execution_count: 2,
            ..source_added
        };
        harness.run(&both, "second implementation bound and executed");

        harness.refuse_missing("fake_discovery_beta", "bidder.toml");
        harness.run(&both, "declaration restored without cleaning");
        harness.refuse_missing("fake_discovery_beta", "mod.rs");
        harness.run(&both, "source restored without cleaning");
        both
    }

    fn edit_rename_remove(
        harness: &Harness,
        initial: &Expected<'static>,
        both: &Expected<'static>,
    ) {
        harness.edit_marker("fake_discovery_beta", "beta-v2");
        let edited = Expected {
            executions: &[
                ALPHA,
                Execution {
                    marker: "beta-v2",
                    ..BETA
                },
            ],
            ..*both
        };
        harness.run(&edited, "nested helper edit without metadata changes");

        fs::rename(
            harness.implementation("fake_discovery_beta"),
            harness.implementation("fake_discovery_fake_gamma"),
        )
        .unwrap();
        let declaration_path = harness.manifest_path("fake_discovery_fake_gamma");
        let mut declaration: Value =
            toml::from_str(&fs::read_to_string(&declaration_path).unwrap()).unwrap();
        declaration["key"] = "fake_discovery_fake_gamma".into();
        assert!(
            declaration["definitions"]
                .as_table_mut()
                .unwrap()
                .remove("fake_discovery_beta")
                .is_some()
        );
        fs::write(declaration_path, toml::to_string(&declaration).unwrap()).unwrap();
        harness.add_metadata(
            "fake_discovery_fake_gamma",
            "fake_discovery_fake_gamma",
            "https://fake-third.discovery.invalid/openrtb2",
            false,
        );
        let renamed = Expected {
            factories: &["fake_discovery_alpha", "fake_discovery_fake_gamma"],
            definitions: &["fake_discovery_alpha", "fake_discovery_fake_gamma"],
            bindings: &[
                ("fake_discovery_alpha", "fake_discovery_alpha"),
                ("fake_discovery_fake_gamma", "fake_discovery_fake_gamma"),
            ],
            declarations: &[
                ("fake_discovery_alpha", None),
                ("fake_discovery_fake_gamma", None),
            ],
            metadata: &[
                ("fake_discovery_alpha", "fake_discovery_alpha", None),
                (
                    "fake_discovery_fake_gamma",
                    "fake_discovery_fake_gamma",
                    None,
                ),
            ],
            executions: &[
                ALPHA,
                Execution {
                    name: "fake_discovery_fake_gamma",
                    key: "fake_discovery_fake_gamma",
                    marker: "beta-v2",
                    endpoint: "https://fake-third.discovery.invalid/openrtb2",
                    ordinal: 1,
                },
            ],
            execution_count: 2,
            feature: false,
        };
        harness.run(&renamed, "source/declaration/metadata renamed together");

        harness.remove_metadata("fake_discovery_fake_gamma", "fake_discovery_fake_gamma");
        let metadata_removed = Expected {
            factories: &["fake_discovery_alpha", "fake_discovery_fake_gamma"],
            declarations: &[
                ("fake_discovery_alpha", None),
                ("fake_discovery_fake_gamma", None),
            ],
            ..*initial
        };
        harness.run(
            &metadata_removed,
            "metadata removed while unused factory remains",
        );
        fs::remove_dir_all(harness.implementation("fake_discovery_fake_gamma")).unwrap();
        harness.run(
            initial,
            "source/declaration removed without metadata changes",
        );
    }

    fn metadata_only(harness: &Harness, initial: &Expected<'static>) -> Expected<'static> {
        harness.add_metadata(
            "fake_discovery_mirror",
            "fake_discovery_alpha",
            "https://mirror.discovery.invalid/different",
            false,
        );
        let mirror = Expected {
            definitions: &["fake_discovery_alpha", "fake_discovery_mirror"],
            bindings: &[
                ("fake_discovery_alpha", "fake_discovery_alpha"),
                ("fake_discovery_mirror", "fake_discovery_alpha"),
            ],
            metadata: &[
                ("fake_discovery_alpha", "fake_discovery_alpha", None),
                ("fake_discovery_mirror", "fake_discovery_alpha", None),
            ],
            executions: &[
                ALPHA,
                Execution {
                    name: "fake_discovery_mirror",
                    key: "fake_discovery_alpha",
                    marker: "alpha-v1",
                    endpoint: "https://mirror.discovery.invalid/different",
                    ordinal: 2,
                },
            ],
            execution_count: 2,
            ..*initial
        };
        harness.run(
            &mirror,
            "metadata-only public identity shares the existing factory",
        );
        mirror
    }

    fn optional_feature(harness: &Harness, mirror: &Expected<'static>) {
        harness.add_source(
            "fake_discovery_optional",
            "optional-v1",
            Some("insecure-bidders"),
        );
        harness.add_metadata(
            "fake_discovery_optional",
            "fake_discovery_optional",
            "https://optional.discovery.invalid/openrtb2",
            true,
        );
        let feature_off = Expected {
            declarations: &[
                ("fake_discovery_alpha", None),
                ("fake_discovery_optional", Some("insecure-bidders")),
            ],
            metadata: &[
                ("fake_discovery_alpha", "fake_discovery_alpha", None),
                ("fake_discovery_mirror", "fake_discovery_alpha", None),
                (
                    "fake_discovery_optional",
                    "fake_discovery_optional",
                    Some("insecure-bidders"),
                ),
            ],
            ..*mirror
        };
        harness.run(
            &feature_off,
            "feature off with inactive declarations retained",
        );
        let feature_on = Expected {
            factories: &["fake_discovery_alpha", "fake_discovery_optional"],
            definitions: &[
                "fake_discovery_alpha",
                "fake_discovery_mirror",
                "fake_discovery_optional",
            ],
            bindings: &[
                ("fake_discovery_alpha", "fake_discovery_alpha"),
                ("fake_discovery_mirror", "fake_discovery_alpha"),
                ("fake_discovery_optional", "fake_discovery_optional"),
            ],
            executions: &[
                ALPHA,
                Execution {
                    name: "fake_discovery_mirror",
                    key: "fake_discovery_alpha",
                    marker: "alpha-v1",
                    endpoint: "https://mirror.discovery.invalid/different",
                    ordinal: 2,
                },
                Execution {
                    name: "fake_discovery_optional",
                    key: "fake_discovery_optional",
                    marker: "optional-v1",
                    endpoint: "https://optional.discovery.invalid/openrtb2",
                    ordinal: 1,
                },
            ],
            execution_count: 3,
            feature: true,
            ..feature_off
        };
        harness.run(
            &feature_on,
            "feature on executes the optional module and factory",
        );
        harness.run(
            &feature_off,
            "feature off again removes executable and public identity",
        );
    }
}

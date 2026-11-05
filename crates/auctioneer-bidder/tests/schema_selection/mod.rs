//! Shared build-time identity selection; no serving or qualification claim.

use auctioneer_config::schema_staging::select as select_bindings;
pub use auctioneer_config::schema_staging::{Declaration as Identity, Member, Schema, Selection};

/// Exercise the build owner's selector with the original test-only root view.
pub fn select<'a>(
    definitions: &[Identity<'a>],
    roots: &[&str],
    enabled_features: &[&str],
) -> Result<Selection<'a>, String> {
    select_bindings(definitions, roots, &[], enabled_features)
}

#[cfg(test)]
mod tests {
    use super::{Identity, Member, Schema, Selection, select};

    fn base<'a>(name: &'a str, owners: &'a [&'a str]) -> Identity<'a> {
        Identity {
            name,
            parent: None,
            implementation: "fake_factory",
            required_feature: None,
            schema: Schema::Owners(owners),
        }
    }

    fn alias<'a>(name: &'a str, parent: &'a str) -> Identity<'a> {
        Identity {
            parent: Some(parent),
            schema: Schema::Inherit,
            ..base(name, &[])
        }
    }

    fn refuses(definitions: &[Identity<'_>], roots: &[&str], features: &[&str], reason: &str) {
        let error = select(definitions, roots, features).expect_err("invalid declaration");
        assert!(error.contains(reason), "expected {reason:?}, got {error:?}");
    }

    #[test]
    fn follows_nested_aliases_and_preserves_binding_ancestry() {
        let definitions = [
            alias("fake_leaf", "fake_middle"),
            base("fake_base", &["fake_base_schema"]),
            alias("fake_middle", "fake_base"),
        ];
        assert_eq!(
            select(&definitions, &["fake_leaf"], &[]).expect("valid aliases"),
            Selection {
                members: vec![
                    Member {
                        name: "fake_base",
                        implementation: "fake_factory",
                        is_root: false,
                        ancestry: vec![],
                        schema_owner: Some("fake_base_schema"),
                    },
                    Member {
                        name: "fake_leaf",
                        implementation: "fake_factory",
                        is_root: true,
                        ancestry: vec!["fake_middle", "fake_base"],
                        schema_owner: Some("fake_base_schema"),
                    },
                    Member {
                        name: "fake_middle",
                        implementation: "fake_factory",
                        is_root: false,
                        ancestry: vec!["fake_base"],
                        schema_owner: Some("fake_base_schema"),
                    },
                ],
                schema_owners: vec!["fake_base_schema"],
            }
        );
    }

    #[test]
    fn nearest_override_wins_without_selecting_shadowed_ancestor_owners() {
        let definitions = [
            base("fake_base", &["fake_original_schema"]),
            Identity {
                schema: Schema::Owners(&["fake_middle_schema"]),
                ..alias("fake_middle", "fake_base")
            },
            alias("fake_leaf", "fake_middle"),
            Identity {
                schema: Schema::Owners(&["fake_tip_schema"]),
                ..alias("fake_tip", "fake_leaf")
            },
        ];
        let selected =
            select(&definitions, &["fake_leaf", "fake_tip"], &[]).expect("valid schema overrides");
        assert_eq!(
            selected.schema_owners,
            ["fake_middle_schema", "fake_tip_schema"]
        );
        let leaf = selected.members.iter().find(|row| row.name == "fake_leaf");
        assert_eq!(
            leaf.map(|row| row.schema_owner),
            Some(Some("fake_middle_schema"))
        );
        let tip = selected.members.iter().find(|row| row.name == "fake_tip");
        assert_eq!(
            tip.map(|row| row.ancestry.as_slice()),
            Some(["fake_leaf", "fake_middle", "fake_base"].as_slice())
        );
        let with_base = select(&definitions, &["fake_base", "fake_leaf", "fake_tip"], &[])
            .expect("ancestor can also be a root");
        assert_eq!(
            with_base.schema_owners,
            [
                "fake_middle_schema",
                "fake_original_schema",
                "fake_tip_schema"
            ]
        );
        assert_eq!(
            with_base.members.iter().filter(|row| row.is_root).count(),
            3
        );
    }

    #[test]
    fn sharing_a_factory_does_not_select_other_public_identities_or_schemas() {
        let definitions = [
            base("fake_selected", &["fake_selected_schema"]),
            base("fake_unselected", &["fake_unselected_schema"]),
        ];
        let selected = select(&definitions, &["fake_selected"], &[]).expect("shared factory");
        assert_eq!(selected.schema_owners, ["fake_selected_schema"]);
        assert_eq!(selected.members.len(), 1);
        assert_eq!(selected.members[0].name, "fake_selected");
    }

    #[test]
    fn owners_deduplicate_by_explicit_identity_not_factory_or_content() {
        let definitions = [
            base("fake_a", &["fake_shared_schema"]),
            Identity {
                implementation: "fake_other_factory",
                ..base("fake_b", &["fake_shared_schema"])
            },
            base("fake_c", &["fake_distinct_schema"]),
        ];
        let selected = select(&definitions, &["fake_c", "fake_b", "fake_a"], &[])
            .expect("explicit owner identities");
        assert_eq!(
            selected.schema_owners,
            ["fake_distinct_schema", "fake_shared_schema"]
        );
        assert_eq!(selected.members.len(), 3);
    }

    #[test]
    fn runtime_off_cannot_exclude_a_shipped_root_by_construction() {
        // Runtime activation is deliberately absent from Identity and select.
        let runtime_disabled_binding = base("fake_runtime_off", &["fake_runtime_off_schema"]);
        let selected = select(&[runtime_disabled_binding], &["fake_runtime_off"], &[])
            .expect("shipped root regardless of runtime activation");
        assert_eq!(selected.schema_owners, ["fake_runtime_off_schema"]);
        assert!(selected.members[0].is_root);
    }

    #[test]
    fn feature_off_on_off_selects_only_the_compiled_lane() {
        let definitions = [
            base("fake_secure", &["fake_secure_schema"]),
            Identity {
                implementation: "fake_local_factory",
                required_feature: Some("fake_insecure"),
                schema: Schema::LocalSchemaFree,
                ..base("fake_local", &[])
            },
        ];
        let roots = ["fake_local", "fake_secure"];
        let off = select(&definitions, &roots, &[]).expect("feature-off lane");
        assert_eq!(off.members.len(), 1);
        assert_eq!(off.members[0].name, "fake_secure");
        let on = select(&definitions, &roots, &["fake_insecure"]).expect("feature-on lane");
        assert_eq!(on.members.len(), 2);
        assert_eq!(on.members[0].name, "fake_local");
        assert_eq!(on.members[0].schema_owner, None);
        assert_eq!(on.schema_owners, ["fake_secure_schema"]);
        assert_eq!(
            select(&definitions, &roots, &[]).expect("feature off again"),
            off
        );
    }

    #[test]
    fn gated_schema_and_alias_appear_together_only_in_the_enabled_lane() {
        let definitions = [
            Identity {
                required_feature: Some("fake_optional"),
                ..base("fake_base", &["fake_schema"])
            },
            Identity {
                required_feature: Some("fake_optional"),
                ..alias("fake_alias", "fake_base")
            },
        ];
        assert_eq!(
            select(&definitions, &["fake_alias"], &[]).expect("feature off"),
            Selection {
                members: vec![],
                schema_owners: vec![]
            }
        );
        let on = select(&definitions, &["fake_alias"], &["fake_optional"]).expect("feature on");
        assert_eq!(on.members.len(), 2);
        assert_eq!(on.schema_owners, ["fake_schema"]);
    }

    #[test]
    fn definition_and_root_order_and_repeated_roots_do_not_change_output() {
        let definitions = [
            base("fake_Zeta", &["fake_zeta_schema"]),
            base("fake_alpha", &["fake_alpha_schema"]),
            alias("fake_middle", "FAKE_ZETA"),
        ];
        let expected = select(&definitions, &["fake_middle", "fake_alpha"], &[])
            .expect("canonical root selection");
        assert_eq!(
            expected
                .members
                .iter()
                .map(|row| row.name)
                .collect::<Vec<_>>(),
            ["fake_alpha", "fake_middle", "fake_Zeta"]
        );
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let reordered = order.map(|index| definitions[index]);
            assert_eq!(
                select(
                    &reordered,
                    &["FAKE_ALPHA", "fake_middle", "fake_alpha"],
                    &[]
                )
                .expect("order-independent selection"),
                expected
            );
        }
    }

    #[test]
    fn rejects_unknown_roots_instead_of_filtering_them_away() {
        refuses(&[], &["fake_unknown"], &[], "unknown root");
        let definitions = [Identity {
            required_feature: Some("fake_optional"),
            ..base("fake_gated", &["fake_schema"])
        }];
        refuses(
            &definitions,
            &["fake_gated", "fake_typo"],
            &[],
            "unknown root",
        );
    }

    #[test]
    fn rejects_missing_parent_even_when_the_alias_overrides_the_schema() {
        let definitions = [Identity {
            schema: Schema::Owners(&["fake_override"]),
            ..alias("fake_alias", "fake_missing")
        }];
        refuses(&definitions, &["fake_alias"], &[], "missing parent");
    }

    #[test]
    fn rejects_schema_absence_unless_local_schema_free_is_explicit() {
        let definitions = [Identity {
            schema: Schema::Inherit,
            ..base("fake_base", &[])
        }];
        refuses(&definitions, &["fake_base"], &[], "missing schema owner");
        let local_definitions = [
            Identity {
                schema: Schema::LocalSchemaFree,
                ..base("fake_local", &[])
            },
            alias("fake_alias", "fake_local"),
        ];
        refuses(
            &local_definitions,
            &["fake_alias"],
            &[],
            "missing schema owner",
        );
    }

    #[test]
    fn rejects_empty_or_ambiguous_explicit_owners() {
        for owners in [&[][..], &[""][..], &[" \t"][..]] {
            refuses(
                &[base("fake_base", owners)],
                &["fake_base"],
                &[],
                "empty schema owner",
            );
        }
        for owners in [
            ["fake_schema_a", "fake_schema_b"],
            ["fake_schema_a", "fake_schema_a"],
        ] {
            refuses(
                &[base("fake_base", &owners)],
                &["fake_base"],
                &[],
                "ambiguous schema owner",
            );
        }
    }

    #[test]
    fn local_schema_free_cannot_hide_alias_ancestry() {
        let definitions = [
            base("fake_base", &["fake_schema"]),
            Identity {
                schema: Schema::LocalSchemaFree,
                ..alias("fake_alias", "fake_base")
            },
        ];
        refuses(&definitions, &["fake_alias"], &[], "schema-free alias");
    }

    #[test]
    fn rejects_self_and_longer_cycles_despite_explicit_overrides() {
        let definitions = [Identity {
            schema: Schema::Owners(&["fake_schema"]),
            ..alias("fake_self", "FAKE_SELF")
        }];
        refuses(&definitions, &["fake_self"], &[], "cycle");
        let cyclic_definitions = [
            Identity {
                schema: Schema::Owners(&["fake_schema"]),
                ..alias("fake_a", "fake_b")
            },
            alias("fake_b", "fake_c"),
            alias("fake_c", "fake_a"),
        ];
        refuses(&cyclic_definitions, &["fake_a"], &[], "cycle");
    }

    #[test]
    fn rejects_duplicate_case_insensitive_public_names_without_roots() {
        let definitions = [
            base("fake_Public", &["fake_one"]),
            base("FAKE_public", &["fake_two"]),
        ];
        refuses(&definitions, &[], &[], "duplicate public name");
        refuses(
            &[definitions[0], definitions[0]],
            &[],
            &[],
            "duplicate public name",
        );
    }

    #[test]
    fn rejects_shared_factory_feature_mismatch() {
        let definitions = [
            base("fake_a", &["fake_schema_a"]),
            Identity {
                required_feature: Some("fake_optional"),
                ..base("fake_b", &["fake_schema_b"])
            },
        ];
        refuses(&definitions, &["fake_a"], &[], "feature mismatch");
        refuses(
            &definitions,
            &["fake_a"],
            &["fake_optional"],
            "feature mismatch",
        );
    }

    #[test]
    fn alias_binding_cannot_switch_factory() {
        let definitions = [
            base("fake_base", &["fake_schema"]),
            Identity {
                implementation: "fake_other_factory",
                ..alias("fake_alias", "fake_base")
            },
        ];
        refuses(
            &definitions,
            &["fake_alias"],
            &[],
            "implementation mismatch",
        );
    }

    #[test]
    fn validates_unselected_gated_declarations_before_feature_filtering() {
        let gated = Identity {
            required_feature: Some("fake_optional"),
            ..base("fake_gated", &["fake_schema"])
        };
        let cases = [
            (
                vec![Identity {
                    parent: Some("fake_missing"),
                    ..gated
                }],
                "missing parent",
            ),
            (
                vec![Identity {
                    parent: Some("fake_gated"),
                    ..gated
                }],
                "cycle",
            ),
            (
                vec![Identity {
                    schema: Schema::Owners(&[]),
                    ..gated
                }],
                "empty schema owner",
            ),
            (
                vec![Identity {
                    schema: Schema::Owners(&["fake_one", "fake_two"]),
                    ..gated
                }],
                "ambiguous schema owner",
            ),
            (
                vec![Identity {
                    schema: Schema::Inherit,
                    ..gated
                }],
                "missing schema owner",
            ),
            (
                vec![
                    gated,
                    Identity {
                        name: "FAKE_GATED",
                        ..gated
                    },
                ],
                "duplicate public name",
            ),
            (
                vec![
                    gated,
                    Identity {
                        name: "fake_other",
                        required_feature: None,
                        ..gated
                    },
                ],
                "feature mismatch",
            ),
        ];
        for (bad, reason) in cases {
            let mut definitions = vec![Identity {
                implementation: "fake_secure_factory",
                ..base("fake_secure", &["fake_secure_schema"])
            }];
            definitions.extend(bad);
            refuses(&definitions, &["fake_secure"], &[], reason);
            refuses(&definitions, &["fake_secure"], &["fake_optional"], reason);
        }
    }

    #[test]
    fn rejects_empty_declaration_identities_and_feature_predicates() {
        for name in ["", " "] {
            refuses(
                &[base(name, &["fake_schema"])],
                &[],
                &[],
                "empty public name",
            );
        }
        refuses(
            &[Identity {
                implementation: "",
                ..base("fake_base", &["fake_schema"])
            }],
            &[],
            &[],
            "empty implementation",
        );
        refuses(
            &[Identity {
                required_feature: Some(""),
                ..base("fake_base", &["fake_schema"])
            }],
            &[],
            &[],
            "empty feature",
        );
    }

    #[test]
    fn invalid_input_refusal_is_order_independent() {
        let definitions = [
            alias("fake_zeta", "fake_missing_zeta"),
            alias("fake_alpha", "fake_missing_alpha"),
        ];
        let first = select(&definitions, &[], &[]).expect_err("invalid parents");
        assert!(first.contains("fake_alpha"));
        assert_eq!(
            select(&[definitions[1], definitions[0]], &[], &[])
                .expect_err("reordered invalid parents"),
            first
        );
    }

    #[test]
    fn ancestry_has_no_fixed_depth_cutoff() {
        let names: Vec<_> = (0_usize..128)
            .map(|index| format!("fake_chain_{index:03}"))
            .collect();
        let mut definitions = vec![base(&names[0], &["fake_schema"])];
        definitions.extend(names.windows(2).map(|pair| alias(&pair[1], &pair[0])));
        let selected = select(&definitions, &[&names[127]], &[]).expect("deep alias ancestry");
        assert_eq!(selected.members.len(), 128);
        assert_eq!(selected.schema_owners, ["fake_schema"]);
        let leaf = selected.members.last().expect("last canonical member");
        assert!(leaf.is_root);
        assert_eq!(
            leaf.ancestry,
            names[..127]
                .iter()
                .rev()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn schema_owner_identity_is_exact_not_public_name_case_folding() {
        let definitions = [
            base("fake_a", &["fake_schema"]),
            base("fake_b", &["fake_SCHEMA"]),
        ];
        let selected = select(&definitions, &["fake_a", "fake_b"], &[]).expect("distinct owners");
        assert_eq!(selected.schema_owners, ["fake_SCHEMA", "fake_schema"]);
    }

    #[test]
    fn empty_roots_produce_no_members_or_owners_after_validation() {
        let empty = Selection {
            members: vec![],
            schema_owners: vec![],
        };
        assert_eq!(select(&[], &[], &[]).expect("empty inventory"), empty);
        assert_eq!(
            select(&[base("fake_base", &["fake_schema"])], &[], &[]).expect("no roots"),
            empty
        );
    }
}

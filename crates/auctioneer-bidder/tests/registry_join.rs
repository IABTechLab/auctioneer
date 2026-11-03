//! Registry behavior over synthetic declarations and the functional fake bidder.

#![cfg(all(test, not(target_arch = "wasm32")))]

#[cfg(test)]
mod registry_inventory;

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod fake_registry;

#[cfg(test)]
mod tests {
    use super::{
        fake_registry,
        registry_inventory::{
            Manifest, Membership, SchemaError, SourceInventory, check_canonical_schema,
        },
    };
    use auctioneer_bidder::{
        BidderHttpResponse, BidderName, BidderRequestInfo, CallBudget, DebugBodySchema,
        EndpointRejectReason, EndpointValidator, EntryPoint, OutboundRequestMetaBuilder,
        OutboundRequestSink, RegistryAlias, RegistryBuildError, RegistryInputs, SinkLimits,
        ValidatedRegistryPlan,
    };
    use auctioneer_config::types::{BidderImplementationBinding, BidderMetadataDeclaration};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::ptr;

    fn schema_fixture() -> (Manifest, Vec<u8>, SourceInventory) {
        let manifest = toml::from_str(fake_registry::MANIFEST).unwrap();
        let bytes = fake_registry::SCHEMA.to_vec();
        let inventory = toml::from_str(fake_registry::SOURCE_INVENTORY).unwrap();
        assert_eq!(
            check_canonical_schema(&manifest, &bytes, &inventory),
            Ok(())
        );
        (manifest, bytes, inventory)
    }

    #[test]
    fn canonical_schema_expectations_reject_missing_ownership() {
        let (mut manifest, bytes, inventory) = schema_fixture();
        let artifacts = manifest.provenance["artifacts"].as_array_mut().unwrap();
        let before = artifacts.len();
        artifacts.retain(|row| {
            row["source_path"].as_str() != Some("static/bidder-params/fake_registry_fake.json")
        });
        assert_eq!(artifacts.len(), before - 1);
        assert_eq!(
            check_canonical_schema(&manifest, &bytes, &inventory),
            Err(SchemaError::Ownership)
        );
    }

    #[test]
    fn canonical_schema_expectations_reject_an_alias_as_the_canonical_owner() {
        let (mut manifest, _, inventory) = schema_fixture();
        let artifacts = manifest.provenance["artifacts"].as_array_mut().unwrap();
        let canonical = artifacts
            .iter_mut()
            .find(|row| {
                row["source_path"].as_str() == Some("static/bidder-params/fake_registry_fake.json")
            })
            .unwrap();
        canonical
            .as_table_mut()
            .unwrap()
            .remove("retained")
            .unwrap();
        let alias = artifacts
            .iter_mut()
            .find(|row| {
                row["source_path"].as_str() == Some("static/bidder-params/fake_registry_alias.json")
            })
            .unwrap();
        alias["retained"]["source_path"] = "parameters.json".into();
        let bytes = fake_registry::ALIAS_SCHEMA;
        assert_eq!(
            check_canonical_schema(&manifest, bytes, &inventory),
            Err(SchemaError::Ownership)
        );
    }

    #[test]
    fn canonical_schema_expectations_reject_a_different_implementation_owner() {
        let (mut manifest, bytes, inventory) = schema_fixture();
        let tree = manifest.provenance["trees"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|tree| tree["category"].as_str() == Some("implementation"))
            .unwrap();
        tree["source_path"] = "adapters/fake_registry_unrelated".into();
        assert_eq!(
            check_canonical_schema(&manifest, &bytes, &inventory),
            Err(SchemaError::Ownership)
        );
    }

    #[test]
    fn canonical_schema_expectations_reject_an_unpinned_source_blob() {
        let (mut manifest, bytes, inventory) = schema_fixture();
        let schema = manifest.provenance["artifacts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| {
                row["source_path"].as_str() == Some("static/bidder-params/fake_registry_fake.json")
            })
            .unwrap();
        schema["source_blob"] = "0000000000000000000000000000000000000000".into();
        assert_eq!(
            check_canonical_schema(&manifest, &bytes, &inventory),
            Err(SchemaError::Witness)
        );
    }

    #[test]
    fn canonical_schema_expectations_reject_self_consistent_but_unpinned_bytes() {
        use sha2::{Digest as _, Sha256};

        let (mut manifest, mut bytes, inventory) = schema_fixture();
        bytes.push(b'\n');
        let length = i64::try_from(bytes.len()).unwrap();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let schema = manifest.provenance["artifacts"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| {
                row["source_path"].as_str() == Some("static/bidder-params/fake_registry_fake.json")
            })
            .unwrap();
        schema["source_bytes"] = length.into();
        schema["source_sha256"] = digest.clone().into();
        schema["retained"]["source_bytes"] = length.into();
        schema["retained"]["source_sha256"] = digest.into();
        assert_eq!(
            check_canonical_schema(&manifest, &bytes, &inventory),
            Err(SchemaError::Witness)
        );
    }

    #[test]
    fn canonical_schema_expectations_reject_corrupt_retained_witnesses() {
        let (manifest, bytes, inventory) = schema_fixture();
        for field in ["source_bytes", "source_sha256"] {
            let mut wrong = manifest.clone();
            let schema = wrong.provenance["artifacts"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|row| {
                    row["source_path"].as_str()
                        == Some("static/bidder-params/fake_registry_fake.json")
                })
                .unwrap();
            schema["retained"][field] = if field == "source_bytes" {
                0_i64.into()
            } else {
                "0000000000000000000000000000000000000000000000000000000000000000".into()
            };
            assert_eq!(
                check_canonical_schema(&wrong, &bytes, &inventory),
                Err(SchemaError::Retained),
                "{field}"
            );
        }
    }

    #[test]
    fn canonical_schema_expectations_reject_wrong_source_repositories_and_revisions() {
        let (manifest, bytes, inventory) = schema_fixture();
        for (field, value) in [
            (
                "repository",
                "https://unrelated.registry.invalid/source.git",
            ),
            ("revision", "5555555555555555555555555555555555555555"),
        ] {
            let mut wrong = manifest.clone();
            wrong.provenance["sources"]["fixture"][field] = value.into();
            assert_eq!(
                check_canonical_schema(&wrong, &bytes, &inventory),
                Err(SchemaError::Source),
                "{field}",
            );
        }
    }

    struct AcceptEndpoints;

    impl EndpointValidator for AcceptEndpoints {
        fn validate(
            &self,
            _: &url::Url,
            _: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    fn request() -> BidRequest {
        serde_json::from_value(json!({
            "id": "registry-request",
            "imp": [{
                "id": "i",
                "banner": {},
                "ext": {"bidder": {"route": "synthetic"}},
            }],
            "ext": {
                "registry_test": {"preserve": true},
            },
        }))
        .unwrap()
    }

    fn synthetic_manifests() -> Vec<Manifest> {
        [fake_registry::MANIFEST, fake_registry::OPTIONAL_MANIFEST]
            .map(|source| toml::from_str(source).unwrap())
            .into()
    }

    fn synthetic_membership() -> Membership {
        let mut membership = Membership {
            factory_declarations: vec![
                ("fake_registry_fake".into(), None),
                (
                    "fake_registry_optional".into(),
                    Some("insecure-bidders".into()),
                ),
            ],
            factories: vec!["fake_registry_fake".into()],
            metadata_declarations: vec![
                (
                    "fake_registry_alias".into(),
                    "fake_registry_fake".into(),
                    None,
                ),
                (
                    "fake_registry_base".into(),
                    "fake_registry_fake".into(),
                    None,
                ),
                (
                    "fake_registry_optional".into(),
                    "fake_registry_optional".into(),
                    Some("insecure-bidders".into()),
                ),
                (
                    "fake_registry_shared".into(),
                    "fake_registry_fake".into(),
                    None,
                ),
            ],
            public_names: vec![
                "fake_registry_alias".into(),
                "fake_registry_base".into(),
                "fake_registry_shared".into(),
            ],
            bindings: vec![
                ("fake_registry_alias".into(), "fake_registry_fake".into()),
                ("fake_registry_base".into(), "fake_registry_fake".into()),
                ("fake_registry_shared".into(), "fake_registry_fake".into()),
            ],
        };
        if cfg!(feature = "insecure-bidders") {
            membership.factories.push("fake_registry_optional".into());
            membership
                .public_names
                .push("fake_registry_optional".into());
            membership.public_names.sort();
            membership.bindings.push((
                "fake_registry_optional".into(),
                "fake_registry_optional".into(),
            ));
            membership.bindings.sort();
        }
        membership
    }

    #[test]
    fn synthetic_declarations_have_exact_feature_membership_in_every_view() {
        let expected = Membership::from_manifests(&synthetic_manifests());
        let actual = synthetic_membership();
        assert_eq!(
            expected.check(&actual),
            Ok(()),
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn validated_fake_join_borrows_metadata_without_rewriting_public_identity() {
        let inputs = fake_registry::inputs();
        let plan = ValidatedRegistryPlan::validate(inputs).unwrap();
        assert_eq!(inputs.infos.len(), 3);
        assert_eq!(plan.definitions().len(), inputs.infos.len());
        for info in inputs.infos {
            let definition = plan.definition(&info.name).unwrap();
            let binding = inputs
                .bindings
                .iter()
                .find(|row| row.bidder == info.name)
                .unwrap();
            assert!(ptr::eq(definition.info(), info));
            assert_eq!(definition.info().name, binding.bidder);
            assert_eq!(definition.implementation(), binding.implementation);
            assert_eq!(definition.canonical_base(), binding.bidder);
            assert_eq!(definition.source_parent(), None);
        }
        for unknown in [
            "fake_unknown_bidder",
            "fake_Registry_Base",
            " fake_registry_base ",
        ] {
            assert!(plan.definition(unknown).is_none());
        }
    }

    #[test]
    fn validated_fake_views_are_sorted_complete_and_repeatable_without_aliases() {
        let expected_names = [
            "fake_registry_alias",
            "fake_registry_base",
            "fake_registry_shared",
        ];
        for _ in 0_usize..3 {
            let plan = ValidatedRegistryPlan::validate(fake_registry::inputs()).unwrap();
            assert_eq!(plan.definitions().len(), 3);
            assert_eq!(
                plan.definitions()
                    .map(|row| row.info().name.as_str())
                    .collect::<Vec<_>>(),
                expected_names
            );
            assert_eq!(plan.base_names().len(), 3);
            assert_eq!(plan.base_names().collect::<Vec<_>>(), expected_names);
            assert_eq!(plan.factory_keys().len(), 1);
            assert_eq!(
                plan.factory_keys().collect::<Vec<_>>(),
                ["fake_registry_fake"]
            );
            assert!(
                plan.definition("fake_registry_fake").is_none(),
                "a factory must not implicitly promote metadata"
            );
            assert_eq!(plan.aliases().len(), 0);
            assert!(plan.aliases().next().is_none());
        }
    }

    #[test]
    fn independent_discovery_keeps_public_names_distinct_from_factory_owners() {
        let mut manifests = synthetic_manifests();
        let owner = manifests
            .iter_mut()
            .find(|manifest| manifest.key == "fake_registry_fake")
            .unwrap();
        let definition = owner.definitions.remove("fake_registry_base").unwrap();
        assert!(
            owner
                .definitions
                .insert("fake_registry_public_name".into(), definition)
                .is_none()
        );
        let expected = Membership::from_manifests(&manifests);
        assert!(
            expected
                .factories
                .contains(&"fake_registry_fake".to_owned())
        );
        assert!(
            !expected
                .public_names
                .contains(&"fake_registry_base".to_owned())
        );
        assert!(
            expected
                .public_names
                .contains(&"fake_registry_public_name".to_owned())
        );
        assert!(expected.bindings.contains(&(
            "fake_registry_public_name".into(),
            "fake_registry_fake".into()
        )));
        assert!(expected.metadata_declarations.contains(&(
            "fake_registry_public_name".into(),
            "fake_registry_fake".into(),
            None
        )));
    }

    type MembershipMutation = (&'static str, fn(&mut Membership));

    #[test]
    fn independent_discovery_expectations_reject_dropped_membership_in_every_view() {
        let expected = Membership::from_manifests(&synthetic_manifests());
        let actual = synthetic_membership();
        assert_eq!(expected.check(&actual), Ok(()));
        let mutations: [MembershipMutation; 5] = [
            ("factory declarations", |rows| {
                rows.factory_declarations.pop().unwrap();
            }),
            ("active factories", |rows| {
                rows.factories.pop().unwrap();
            }),
            ("metadata declarations", |rows| {
                rows.metadata_declarations.pop().unwrap();
            }),
            ("active public names", |rows| {
                rows.public_names.pop().unwrap();
            }),
            ("active bindings", |rows| {
                rows.bindings.pop().unwrap();
            }),
        ];
        for (view, mutate) in mutations {
            let mut dropped = actual.clone();
            mutate(&mut dropped);
            assert_eq!(expected.check(&dropped), Err(view), "dropped {view}");
        }
    }

    #[test]
    fn independent_discovery_expectations_reject_extra_membership_in_every_view() {
        let expected = Membership::from_manifests(&synthetic_manifests());
        let actual = synthetic_membership();
        assert_eq!(expected.check(&actual), Ok(()));
        let mutations: [MembershipMutation; 5] = [
            ("factory declarations", |rows| {
                rows.factory_declarations
                    .push(("zz_undiscovered_factory".into(), None));
            }),
            ("active factories", |rows| {
                rows.factories.push("zz_undiscovered_factory".into());
            }),
            ("metadata declarations", |rows| {
                rows.metadata_declarations.push((
                    "fake_zz_undiscovered_bidder".into(),
                    "fake_registry_fake".into(),
                    None,
                ));
            }),
            ("active public names", |rows| {
                rows.public_names.push("fake_zz_undiscovered_bidder".into());
            }),
            ("active bindings", |rows| {
                rows.bindings.push((
                    "fake_zz_undiscovered_bidder".into(),
                    "fake_registry_fake".into(),
                ));
            }),
        ];
        for (view, mutate) in mutations {
            let mut extra = actual.clone();
            mutate(&mut extra);
            assert_eq!(expected.check(&extra), Err(view), "extra {view}");
        }
    }

    #[test]
    fn independent_discovery_expectations_reject_feature_wrong_membership() {
        let manifests = synthetic_manifests();
        let expected = Membership::from_manifests(&manifests);
        let actual = synthetic_membership();
        assert_eq!(expected.check(&actual), Ok(()));
        let gated = manifests
            .iter()
            .find(|manifest| {
                manifest.factory
                    && manifest.required_feature.is_some()
                    && !manifest.definitions.is_empty()
            })
            .unwrap();
        let mut wrong_factories = actual.clone();
        if cfg!(feature = "insecure-bidders") {
            wrong_factories.factories.retain(|key| key != &gated.key);
        } else {
            wrong_factories.factories.push(gated.key.clone());
            wrong_factories.factories.sort();
        }
        assert_eq!(expected.check(&wrong_factories), Err("active factories"));

        let name = gated.definitions.keys().next().unwrap();
        let mut wrong_public_names = actual.clone();
        if cfg!(feature = "insecure-bidders") {
            wrong_public_names
                .public_names
                .retain(|public| public != name);
        } else {
            wrong_public_names.public_names.push(name.clone());
            wrong_public_names.public_names.sort();
        }
        assert_eq!(
            expected.check(&wrong_public_names),
            Err("active public names")
        );

        let mut wrong_bindings = actual.clone();
        if cfg!(feature = "insecure-bidders") {
            wrong_bindings.bindings.retain(|(public, _)| public != name);
        } else {
            wrong_bindings
                .bindings
                .push((name.clone(), gated.key.clone()));
            wrong_bindings.bindings.sort();
        }
        assert_eq!(expected.check(&wrong_bindings), Err("active bindings"));

        let mut wrong_factory_declarations = actual.clone();
        wrong_factory_declarations
            .factory_declarations
            .iter_mut()
            .find(|(key, _)| key == &gated.key)
            .unwrap()
            .1 = None;
        assert_eq!(
            expected.check(&wrong_factory_declarations),
            Err("factory declarations")
        );
        let mut wrong_metadata_declarations = actual.clone();
        wrong_metadata_declarations
            .metadata_declarations
            .iter_mut()
            .find(|(public, _, _)| public == name)
            .unwrap()
            .2 = None;
        assert_eq!(
            expected.check(&wrong_metadata_declarations),
            Err("metadata declarations")
        );
    }

    #[test]
    fn independent_discovery_expectations_reject_wrong_implementation_bindings() {
        let expected = Membership::from_manifests(&synthetic_manifests());
        let actual = synthetic_membership();
        assert_eq!(expected.check(&actual), Ok(()));
        let mut wrong_bindings = actual.clone();
        wrong_bindings.bindings.first_mut().unwrap().1 = "missing_factory".into();
        assert_eq!(expected.check(&wrong_bindings), Err("active bindings"));
        let mut wrong_metadata = actual;
        wrong_metadata.metadata_declarations.first_mut().unwrap().1 = "missing_factory".into();
        assert_eq!(
            expected.check(&wrong_metadata),
            Err("metadata declarations")
        );
    }

    #[test]
    #[should_panic(expected = "unknown required feature: unrecognized-bidder-feature")]
    fn independent_discovery_expectations_fail_on_unknown_feature_requirements() {
        let mut manifests = synthetic_manifests();
        manifests.first_mut().unwrap().required_feature =
            Some("unrecognized-bidder-feature".into());
        Membership::from_manifests(&manifests);
    }

    #[test]
    fn validated_factories_construct_the_fake_through_the_real_sink() {
        let plan = ValidatedRegistryPlan::validate(fake_registry::inputs()).unwrap();
        let request = request();
        let original = serde_json::to_value(&request).unwrap();
        let mut exercised_names = Vec::new();
        for definition in plan.definitions() {
            let info = definition.info();
            let bidder = definition.construct();
            assert_eq!(bidder.debug_body_schema(), DebugBodySchema::OpenRtb);
            let request_info = BidderRequestInfo::new(
                BidderName::new(info.name.as_str()),
                BidderName::new(definition.canonical_base()),
                "registry-account",
                None,
                &info.endpoint,
                &[],
                None,
                None,
                EntryPoint::Auction,
                &[],
            );
            let budget = CallBudget::new(1);
            let mut sink =
                OutboundRequestSink::new(SinkLimits::new(256 * 1024, 1), &budget, &AcceptEndpoints);
            bidder
                .make_requests(&request, &request_info, &mut sink)
                .unwrap();
            let outcome = sink.into_outcome();
            assert!(outcome.errors.is_empty(), "factory {}", info.name);
            assert_eq!(outcome.suppressed_error_count, 0);
            assert_eq!(outcome.rejected_call_count, 0);
            assert_eq!(outcome.accepted.len(), 1, "factory {}", info.name);
            let outbound = &outcome.accepted[0];
            assert_eq!(outbound.method(), "POST");
            assert_eq!(outbound.url(), info.endpoint);
            assert_eq!(outbound.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
            for expected in [
                ("Content-Type", "application/json"),
                ("Accept", "application/json"),
            ] {
                assert!(outbound.headers().any(|header| header == expected));
            }
            let actual: Value = serde_json::from_slice(outbound.body()).unwrap();
            assert_eq!(actual, original, "wrong body from factory {}", info.name);
            exercised_names.push(info.name.as_str());
        }
        assert_eq!(
            exercised_names,
            [
                "fake_registry_alias",
                "fake_registry_base",
                "fake_registry_shared"
            ]
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), original);
    }

    #[test]
    fn validated_fake_factory_decodes_success_and_empty_status_through_the_sealed_parser() {
        let plan = ValidatedRegistryPlan::validate(fake_registry::inputs()).unwrap();
        let request = request();
        let mut pool = Vec::new();
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
        builder.set_method("POST").unwrap();
        builder.set_url("https://registry.invalid/openrtb").unwrap();
        builder.imp_id("i").unwrap();
        let sent = builder.finish();
        let body = serde_json::to_vec(&json!({
            "id": "response",
            "seatbid": [{
                "seat": "fake_synthetic",
                "bid": [{"id": "bid", "impid": "i", "price": 1.25_f64, "mtype": 1_i32}],
            }],
            "cur": "USD",
        }))
        .unwrap();
        let bidder = plan.definition("fake_registry_base").unwrap().construct();
        for (status, expected_bids) in [(204, 0), (200, 1)] {
            let response =
                BidderHttpResponse::new(status, vec![], body.clone()).within_node_budget(1_000);
            let decoded = bidder.make_bids(&request, &sent, &response).unwrap();
            assert_eq!(decoded.bids.len(), expected_bids, "status {status}");
            assert_eq!(decoded.currency, "USD");
            assert!(decoded.errors.is_empty());
            if expected_bids != 0 {
                assert_eq!(decoded.bids[0].bid.id, "bid");
                assert_eq!(decoded.bids[0].bid.impid, "i");
            }
        }
    }

    fn distinct_factory() -> Box<dyn auctioneer_bidder::Bidder> {
        Box::new(super::fake_bidder::FakeBidder {
            method: "PATCH",
            headers: &[("X-Registry-Factory", "selected")],
            ..super::fake_bidder::FakeBidder::default()
        })
    }

    fn shared_factory_plan(aliases: &[RegistryAlias]) -> ValidatedRegistryPlan {
        // Both factories implement the same capable fake contract, but their outbound
        // signatures differ. Selecting any other registered factory must fail this test.
        let declarations = [
            auctioneer_bidder::BidderDeclaration {
                key: "fake_registry_fake",
                required_feature: None,
                has_settings: false,
            },
            auctioneer_bidder::BidderDeclaration {
                key: "fake_registry_other",
                required_feature: None,
                has_settings: false,
            },
        ];
        let factories = [
            auctioneer_bidder::BidderRegistration {
                key: "fake_registry_fake",
                factory: distinct_factory,
                config_validator: None,
            },
            auctioneer_bidder::BidderRegistration {
                key: "fake_registry_other",
                factory: super::fake_bidder::create_bidder,
                config_validator: None,
            },
        ];
        ValidatedRegistryPlan::validate(RegistryInputs {
            aliases,
            factory_declarations: &declarations,
            factories: &factories,
            ..fake_registry::inputs()
        })
        .unwrap()
    }

    #[test]
    fn shared_bases_and_alias_execute_the_exact_factory_with_public_attribution() {
        let inputs = fake_registry::inputs();
        let aliases = [RegistryAlias {
            name: "fake_registry_alias",
            source_parent: "fake_registry_base",
            canonical_base: "fake_registry_base",
        }];
        let plan = shared_factory_plan(&aliases);
        assert_eq!(
            plan.base_names().collect::<Vec<_>>(),
            ["fake_registry_base", "fake_registry_shared"]
        );
        assert_eq!(plan.aliases().copied().collect::<Vec<_>>(), aliases);
        let request = request();
        let original = serde_json::to_value(&request).unwrap();
        let mut executed = 0_usize;
        for info in inputs.infos {
            let definition = plan.definition(&info.name).unwrap();
            assert!(ptr::eq(definition.info(), info));
            assert_eq!(definition.implementation(), "fake_registry_fake");
            let is_alias = info.name == "fake_registry_alias";
            assert_eq!(
                definition.source_parent(),
                is_alias.then_some("fake_registry_base")
            );
            assert_eq!(
                definition.canonical_base(),
                if is_alias {
                    "fake_registry_base"
                } else {
                    &info.name
                }
            );
            let bidder = definition.construct();
            let request_info = BidderRequestInfo::new(
                BidderName::new(info.name.as_str()),
                BidderName::new(definition.canonical_base()),
                "registry-account",
                None,
                &info.endpoint,
                &[],
                None,
                None,
                EntryPoint::Auction,
                &[],
            );
            let budget = CallBudget::new(1);
            let mut sink =
                OutboundRequestSink::new(SinkLimits::new(256 * 1024, 1), &budget, &AcceptEndpoints);
            bidder
                .make_requests(&request, &request_info, &mut sink)
                .unwrap();
            let outcome = sink.into_outcome();
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.rejected_call_count, 0);
            assert_eq!(outcome.suppressed_error_count, 0);
            assert_eq!(outcome.accepted.len(), 1);
            let outbound = &outcome.accepted[0];
            assert_eq!(outbound.url(), info.endpoint);
            assert_eq!(outbound.method(), "PATCH");
            assert!(
                outbound
                    .headers()
                    .any(|header| header == ("X-Registry-Factory", "selected"))
            );
            assert_eq!(outbound.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
            assert_eq!(
                serde_json::from_slice::<Value>(outbound.body()).unwrap(),
                original
            );
            let body = serde_json::to_vec(&json!({
                "id": "response", "cur": "USD",
                "seatbid": [{"seat": "fake_synthetic", "bid": [{"id": "bid", "impid": "i", "price": 1.25_f64, "mtype": 1_i32}]}],
            })).unwrap();
            let response = BidderHttpResponse::new(200, vec![], body).within_node_budget(1_000);
            let decoded = bidder
                .make_bids(&request, outbound.meta(), &response)
                .unwrap();
            assert!(decoded.errors.is_empty());
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "bid");
            assert_eq!(decoded.bids[0].bid.impid, "i");
            assert_eq!(decoded.currency, "USD");
            executed = executed.checked_add(1).unwrap();
        }
        assert_eq!(executed, 3);
        assert_eq!(serde_json::to_value(&request).unwrap(), original);
    }

    #[test]
    fn unknown_implementation_binding_refuses_instead_of_using_another_factory() {
        let bindings = [BidderImplementationBinding {
            bidder: "fake_registry_base",
            implementation: "fake_missing_factory",
        }];
        let metadata = [BidderMetadataDeclaration {
            bidder: "fake_registry_base",
            implementation: "fake_missing_factory",
            required_feature: None,
        }];
        assert_eq!(
            ValidatedRegistryPlan::validate(RegistryInputs {
                infos: &fake_registry::infos()[1..2],
                bindings: &bindings,
                metadata_declarations: &metadata,
                ..fake_registry::inputs()
            })
            .unwrap_err(),
            RegistryBuildError::MissingFactoryDeclaration {
                implementation: "fake_missing_factory"
            }
        );
    }
}

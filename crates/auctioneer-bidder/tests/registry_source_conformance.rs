//! Whole-catalog source and generated-artifact conformance; no serving registry is installed.

#![cfg(all(unix, not(target_arch = "wasm32")))]

#[cfg(test)]
pub mod build_workspace;
#[cfg(test)]
mod catalog;
#[cfg(test)]
pub mod registry_inventory;

#[cfg(test)]
mod tests {
    use super::build_workspace::{copy_source_tree, prepare_synthetic_catalog, prepare_workspace};
    use super::catalog::{
        bidder_root, check_metadata_source_inventory, manifests, source_inventory,
    };
    use super::registry_inventory::{Membership, check_canonical_schema};
    use auctioneer_bidder::{
        RegistryInputs, ValidatedRegistryPlan, bidder_declarations, compiled_bidders,
    };
    use auctioneer_config::{
        bidder_implementation_bindings, bidder_metadata_declarations, bidders,
    };
    use serde_json::Value;
    use std::ptr;
    use std::{fs, path::Path};
    use toml::Value as Toml;

    fn generated_inputs() -> RegistryInputs<'static> {
        RegistryInputs {
            infos: bidders(),
            bindings: bidder_implementation_bindings(),
            metadata_declarations: bidder_metadata_declarations(),
            factory_declarations: bidder_declarations(),
            factories: compiled_bidders(),
            aliases: &[],
        }
    }

    fn generated_plan() -> ValidatedRegistryPlan {
        ValidatedRegistryPlan::validate(generated_inputs()).unwrap()
    }

    fn generated_membership() -> Membership {
        Membership {
            public_names: bidders().iter().map(|info| info.name.clone()).collect(),
            bindings: bidder_implementation_bindings()
                .iter()
                .map(|binding| (binding.bidder.to_owned(), binding.implementation.to_owned()))
                .collect(),
            factories: compiled_bidders()
                .iter()
                .map(|factory| factory.key.to_owned())
                .collect(),
            metadata_declarations: bidder_metadata_declarations()
                .iter()
                .map(|declaration| {
                    (
                        declaration.bidder.to_owned(),
                        declaration.implementation.to_owned(),
                        declaration.required_feature.map(str::to_owned),
                    )
                })
                .collect(),
            factory_declarations: bidder_declarations()
                .iter()
                .map(|declaration| {
                    (
                        declaration.key.to_owned(),
                        declaration.required_feature.map(str::to_owned),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn bidder_manifest_prose_wraps_without_splitting_urls_or_paths() {
        use std::{fs, path::Path};

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders");
        let mut checked = 0_usize;
        for entry in fs::read_dir(root).unwrap() {
            let manifest = entry.unwrap().path().join("bidder.toml");
            if !manifest.is_file() {
                continue;
            }
            let source = fs::read_to_string(&manifest).unwrap();
            for (line, text) in source.lines().enumerate() {
                if [
                    "url = ",
                    "source_url = ",
                    "selected_url = ",
                    "endpoint = ",
                    "repository = ",
                    "source_path = ",
                    "path = ",
                ]
                .iter()
                .any(|prefix| text.starts_with(*prefix))
                {
                    assert!(
                        !text.contains("\"\"\"") && !text.contains("'''"),
                        "{}:{} must keep its URL or path on one line",
                        manifest.display(),
                        line + 1,
                    );
                    continue;
                }
                assert!(
                    text.chars().count() <= 100,
                    "{}:{} exceeds 100 columns",
                    manifest.display(),
                    line + 1,
                );
            }
            checked += 1;
        }
        assert!(checked > 0);
    }

    #[test]
    fn bidder_metadata_uses_source_witnesses_without_yaml_copies() {
        use std::{fs, path::Path};

        fn assert_no_yaml(directory: &Path) {
            for result in fs::read_dir(directory).unwrap() {
                let entry = result.unwrap();
                let child = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    assert_no_yaml(&child);
                } else {
                    assert!(
                        !matches!(
                            child.extension().and_then(|ext| ext.to_str()),
                            Some("yaml" | "yml")
                        ),
                        "duplicate metadata file: {}",
                        child.display()
                    );
                }
            }
        }

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders");
        assert_no_yaml(&root);
        let mut checked = 0_usize;
        for result in fs::read_dir(root).unwrap() {
            let manifest = result.unwrap().path().join("bidder.toml");
            if !manifest.is_file() {
                continue;
            }
            let definition: toml::Value =
                toml::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
            let Some(artifacts) = definition["provenance"]
                .get("artifacts")
                .and_then(toml::Value::as_array)
            else {
                continue;
            };
            for artifact in artifacts
                .iter()
                .filter(|row| row["category"].as_str() == Some("metadata"))
            {
                assert!(artifact.get("retained").is_none());
                checked += 1;
            }
        }
        assert!(checked > 0);
        check_metadata_source_inventory(&manifests(), &source_inventory());
    }

    #[test]
    fn every_reference_factory_owns_its_original_canonical_parameter_schema() {
        use std::fs;

        let root = bidder_root();
        let inventory = source_inventory();
        let manifests = manifests();
        let owners: Vec<_> = manifests
            .iter()
            .filter(|manifest| {
                manifest.factory && manifest.provenance["origin"].as_str() == Some("reference")
            })
            .collect();
        assert!(!owners.is_empty());
        for manifest in owners {
            let directory = root.join(&manifest.key);
            let bytes = fs::read(directory.join("parameters.json")).unwrap();
            assert_eq!(
                check_canonical_schema(manifest, &bytes, &inventory),
                Ok(()),
                "{}",
                manifest.key
            );
            assert!(!directory.join("params.schema.json").exists());
            assert!(!directory.join("LICENSE").exists());
        }
        for manifest in manifests.iter().filter(|manifest| manifest.factory) {
            let owns_schema = manifest
                .provenance
                .get("artifacts")
                .and_then(toml::Value::as_array)
                .is_some_and(|artifacts| {
                    artifacts.iter().any(|artifact| {
                        artifact
                            .get("retained")
                            .and_then(|row| row.get("source_path"))
                            .and_then(toml::Value::as_str)
                            == Some("parameters.json")
                    })
                });
            assert_eq!(
                root.join(&manifest.key).join("parameters.json").exists(),
                owns_schema,
                "{}",
                manifest.key
            );
        }
    }

    #[test]
    fn bidder_owned_manifests_preserve_the_exact_effective_catalog() {
        use sha2::{Digest as _, Sha256};
        use std::fs;

        let root = bidder_root();
        let factories: Vec<_> = manifests().into_iter().filter(|row| row.factory).collect();
        assert!(!factories.is_empty());
        for manifest in factories {
            let directory = root.join(&manifest.key);
            let fixture: Value =
                serde_json::from_slice(&fs::read(directory.join("fixtures/factory.json")).unwrap())
                    .unwrap();
            assert_eq!(
                fixture["implementation"].as_str(),
                Some(manifest.key.as_str())
            );
            assert!(!fixture["cases"].as_array().unwrap().is_empty());
            // Frozen bidder-owned expectations pin all effective metadata and provenance,
            // including supplemental sources, inactive factories and schema-less owners.
            let digest = format!(
                "{:x}",
                Sha256::digest(fs::read(directory.join("bidder.toml")).unwrap())
            );
            assert_eq!(
                fixture["manifest_sha256"].as_str(),
                Some(digest.as_str()),
                "{}",
                manifest.key
            );
        }
    }

    fn require_owner_modules(implementation: &syn::File) -> Result<(), String> {
        for name in ["request", "response"] {
            let modules: Vec<_> = implementation
                .items
                .iter()
                .filter_map(|item| {
                    if let syn::Item::Mod(module) = item
                        && module.ident == name
                    {
                        Some(module)
                    } else {
                        None
                    }
                })
                .collect();
            if modules.len() != 1 || modules[0].content.is_some() {
                return Err(format!("{name} must have one ordinary owner module"));
            }
        }
        for item in &implementation.items {
            if let syn::Item::Fn(function) = item
                && function.sig.ident != "create_bidder"
            {
                return Err(format!(
                    "protocol helper {} belongs in its owner module",
                    function.sig.ident
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn owner_layout_rejects_missing_inline_duplicate_modules_and_root_helpers() {
        let valid = "mod request; mod response; fn create_bidder() {}";
        assert_eq!(
            require_owner_modules(&syn::parse_file(valid).unwrap()),
            Ok(())
        );
        for (source, expected) in [
            (
                "mod response;",
                "request must have one ordinary owner module",
            ),
            (
                "mod request;",
                "response must have one ordinary owner module",
            ),
            (
                "mod request {} mod response;",
                "request must have one ordinary owner module",
            ),
            (
                "mod request; mod response {}",
                "response must have one ordinary owner module",
            ),
            (
                "mod request; mod request; mod response;",
                "request must have one ordinary owner module",
            ),
            (
                "mod request; mod response; mod response;",
                "response must have one ordinary owner module",
            ),
            (
                "mod request; mod response; fn decode() {}",
                "protocol helper decode belongs in its owner module",
            ),
        ] {
            assert_eq!(
                require_owner_modules(&syn::parse_file(source).unwrap()),
                Err(expected.into()),
                "{source}"
            );
        }
    }

    #[test]
    fn every_implemented_bidder_owns_request_and_response_modules() {
        let root = bidder_root();
        let declarations: Vec<_> = manifests()
            .into_iter()
            .filter(|manifest| manifest.factory)
            .collect();
        assert!(!declarations.is_empty());
        for declaration in declarations {
            let directory = root.join(&declaration.key);
            let source = fs::read_to_string(directory.join("mod.rs")).unwrap();
            let implementation = syn::parse_file(&source).unwrap();
            assert_eq!(
                require_owner_modules(&implementation),
                Ok(()),
                "{}",
                declaration.key
            );
            for name in ["request", "response"] {
                assert!(
                    directory.join(format!("{name}.rs")).is_file(),
                    "{}: {name}",
                    declaration.key
                );
            }
        }
    }

    #[test]
    fn shared_fake_bidder_owns_the_standard_request_and_response_modules() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/test_support/fake_bidder");
        let source = fs::read_to_string(directory.join("mod.rs")).unwrap();
        let implementation = syn::parse_file(&source).unwrap();
        assert_eq!(require_owner_modules(&implementation), Ok(()));
        for name in ["request", "response"] {
            assert!(directory.join(format!("{name}.rs")).is_file(), "{name}");
        }
    }

    #[test]
    fn every_declared_bidder_keeps_its_unit_tests_inline_with_its_implementation() {
        use std::{fs, path::Path};

        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders");
        let declarations: Vec<_> = manifests()
            .into_iter()
            .filter(|manifest| manifest.factory)
            .collect();
        assert!(!declarations.is_empty());
        for declaration in declarations {
            let directory = root.join(&declaration.key);
            let implementation =
                syn::parse_file(&fs::read_to_string(directory.join("mod.rs")).unwrap()).unwrap();
            let modules = implementation
                .items
                .iter()
                .filter_map(|item| {
                    if let syn::Item::Mod(module) = item
                        && module.ident == "tests"
                    {
                        Some(module)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(modules.len(), 1, "{}", declaration.key);
            let module = modules[0];
            assert!(
                module
                    .content
                    .as_ref()
                    .is_some_and(|(_, tests)| !tests.is_empty()),
                "unit tests must stay inline: {}",
                declaration.key
            );
            assert!(module.attrs.iter().any(|attr| {
                matches!(&attr.meta, syn::Meta::List(list)
                    if list.path.is_ident("cfg") && list.tokens.to_string() == "test")
            }));
            assert!(!directory.join("tests.rs").exists(), "{}", declaration.key);
        }
    }

    #[test]
    fn feature_predicate_controls_all_declared_factories_and_metadata() {
        let declarations: Vec<_> = auctioneer_bidder::bidder_declarations()
            .iter()
            .filter(|row| row.required_feature == Some("insecure-bidders"))
            .collect();
        assert!(!declarations.is_empty());
        for declaration in declarations {
            assert_eq!(
                auctioneer_bidder::compiled_bidders()
                    .iter()
                    .any(|row| row.key == declaration.key),
                cfg!(feature = "insecure-bidders"),
                "{}",
                declaration.key,
            );
            let metadata: Vec<_> = auctioneer_config::bidder_metadata_declarations()
                .iter()
                .filter(|row| row.implementation == declaration.key)
                .collect();
            assert!(
                !metadata.is_empty(),
                "feature-gated metadata control must not be empty"
            );
            for row in metadata {
                assert_eq!(row.required_feature, declaration.required_feature);
                assert_eq!(
                    auctioneer_config::bidder(row.bidder).is_some(),
                    cfg!(feature = "insecure-bidders")
                );
            }
        }
    }

    #[test]
    fn generated_views_have_exact_feature_membership_and_all_declarations() {
        let expected = Membership::from_manifests(&manifests());
        let actual = generated_membership();
        assert_eq!(
            expected.check(&actual),
            Ok(()),
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn validated_join_preserves_generated_metadata_identity_and_public_names() {
        let plan = generated_plan();
        let expected = Membership::from_manifests(&manifests());
        assert_eq!(bidders().len(), expected.public_names.len());
        assert_eq!(plan.definitions().len(), bidders().len());
        for info in bidders() {
            let definition = plan.definition(&info.name).unwrap();
            let binding = bidder_implementation_bindings()
                .iter()
                .find(|binding| binding.bidder == info.name)
                .unwrap();
            assert!(ptr::eq(definition.info(), info));
            assert_eq!(definition.info().name, binding.bidder);
            assert_eq!(definition.implementation(), binding.implementation);
            assert_eq!(definition.canonical_base(), binding.bidder);
            assert_eq!(definition.source_parent(), None);
        }
        assert!(plan.definition("unknown-bidder").is_none());
    }

    #[test]
    fn validated_generated_views_are_sorted_complete_and_repeatable_without_aliases() {
        let expected = Membership::from_manifests(&manifests());
        for _ in 0_usize..3 {
            let plan = generated_plan();
            assert_eq!(plan.definitions().len(), expected.public_names.len());
            assert_eq!(
                plan.definitions()
                    .map(|definition| definition.info().name.as_str())
                    .collect::<Vec<_>>(),
                expected.public_names
            );
            assert_eq!(plan.base_names().len(), expected.public_names.len());
            assert_eq!(plan.base_names().collect::<Vec<_>>(), expected.public_names);
            assert_eq!(plan.factory_keys().len(), compiled_bidders().len());
            assert_eq!(plan.factory_keys().collect::<Vec<_>>(), expected.factories);
            for key in &expected.factories {
                assert_eq!(
                    plan.definition(key).is_some(),
                    expected.public_names.contains(key),
                    "factory {key} must not implicitly promote metadata"
                );
            }
            assert_eq!(plan.aliases().len(), 0);
            assert!(plan.aliases().next().is_none());
        }
    }

    #[test]
    fn owned_workspace_preserves_the_shared_catalog_reader_sibling_and_manifests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let original: Toml =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let (bidder, config) = prepare_workspace(&root, temporary.path(), &original, &[]);
        let sibling = bidder
            .parent()
            .unwrap()
            .join("auctioneer-config-schema/src");
        assert_eq!(
            sibling,
            config
                .parent()
                .unwrap()
                .join("auctioneer-config-schema/src")
        );
        for filename in ["build_catalog.rs", "bidder_manifest.rs"] {
            assert_eq!(
                fs::read(sibling.join(filename)).unwrap(),
                fs::read(
                    root.join("crates/auctioneer-config-schema/src")
                        .join(filename)
                )
                .unwrap(),
            );
        }
        let declarations = auctioneer_bidder::bidder_declarations();
        assert!(!declarations.is_empty());
        for declaration in declarations {
            for filename in ["bidder.toml", "mod.rs"] {
                let relative = Path::new("src/bidders")
                    .join(declaration.key)
                    .join(filename);
                assert_eq!(
                    fs::read(bidder.join(&relative)).unwrap(),
                    fs::read(root.join("crates/auctioneer-bidder").join(&relative)).unwrap()
                );
            }
        }
    }

    #[test]
    fn synthetic_catalog_prunes_only_the_scratch_catalog_and_preserves_framework_inputs() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let temporary = tempfile::tempdir().unwrap();
        let bidder = temporary.path();
        let mut copied_bytes = 0;
        copy_source_tree(&root.join("src"), &bidder.join("src"), &mut copied_bytes);
        let original_library = fs::read(root.join("src/lib.rs")).unwrap();
        let original_catalog = fs::read(root.join("src/bidders/mod.rs")).unwrap();
        let original_framework = fs::read(bidder.join("src/bidder.rs")).unwrap();
        prepare_synthetic_catalog(bidder);
        assert_eq!(fs::read(root.join("src/lib.rs")).unwrap(), original_library);
        assert_eq!(
            fs::read(root.join("src/bidders/mod.rs")).unwrap(),
            original_catalog
        );
        assert_eq!(
            fs::read(bidder.join("src/bidder.rs")).unwrap(),
            original_framework
        );
        let mut removed = 0_usize;
        for result in fs::read_dir(root.join("src/bidders")).unwrap() {
            let path = result.unwrap().path();
            if path.join("bidder.toml").is_file() {
                assert!(
                    !bidder
                        .join("src/bidders")
                        .join(path.file_name().unwrap())
                        .exists()
                );
                removed = removed.checked_add(1).unwrap();
            }
        }
        assert!(removed > 0);
        let catalog =
            syn::parse_file(&fs::read_to_string(bidder.join("src/bidders/mod.rs")).unwrap())
                .unwrap();
        assert!(
            catalog
                .items
                .iter()
                .all(|item| !matches!(item, syn::Item::Mod(module) if module.content.is_none()))
        );
        assert!(catalog.items.iter().any(
            |item| matches!(item, syn::Item::Macro(invocation) if invocation.mac.path.is_ident("include"))
        ));
    }
}

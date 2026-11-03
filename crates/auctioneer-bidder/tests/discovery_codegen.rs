//! Native integration tests of the real shared discovery and reconciliation API.

#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use auctioneer_config::{bidder_discovery, bidder_reconcile, types::BidderMetadataDeclaration};
    use quote::ToTokens as _;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn fixture() -> TempDir {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().canonicalize().unwrap();
        fs::write(
            directory.join("Cargo.toml"),
            "[features]\nexperimental = ['auctioneer-config-data/experimental']\n",
        )
        .unwrap();
        for (key, factory, feature) in [
            ("fake_alpha", true, ""),
            ("fake_gated", true, "required_feature = 'experimental'\n"),
            ("fake_reference", false, ""),
        ] {
            let owner = directory.join("src/bidders").join(key);
            fs::create_dir_all(owner.join("tests")).unwrap();
            fs::write(
                owner.join("bidder.toml"),
                format!(
                    "key = '{key}'\nfactory = {factory}\n{feature}[provenance]\norigin = 'local'\n"
                ),
            )
            .unwrap();
            if factory {
                fs::write(owner.join("mod.rs"), "").unwrap();
                fs::write(
                    owner.join("tests/allocation_conformance.rs"),
                    "#[test] fn allocation() {}\n",
                )
                .unwrap();
                fs::write(
                    owner.join("tests/response_conformance.rs"),
                    "pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 1;\n",
                )
                .unwrap();
            } else {
                fs::write(owner.join("uid_policy.rs"), "pub const FAMILY: &str = \"reference\";\npub fn accepts_uid(_: &str) -> bool { true }\n").unwrap();
            }
        }
        temporary
    }

    #[test]
    fn committed_catalog_reconciles_with_the_actual_generated_metadata() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"));
        let discovery = bidder_discovery::scan(directory).unwrap();
        let metadata = auctioneer_config_data::bidder_metadata_declarations();
        assert!(!metadata.is_empty());
        assert_eq!(discovery.validate_metadata(metadata), Ok(()));
        let registry = syn::parse_file(&discovery.render_registry(metadata).unwrap()).unwrap();
        assert!(registry.items.iter().any(|item| matches!(item, syn::Item::Fn(function) if function.sig.ident == "configured_registry")));
        assert!(
            discovery
                .rerun_paths()
                .contains(&directory.join("Cargo.toml"))
        );
        assert!(
            discovery
                .rerun_paths()
                .contains(&directory.join("src/bidders"))
        );
    }

    #[test]
    fn shared_builder_preserves_factory_gates_and_reference_only_uid_ownership() {
        let temporary = fixture();
        let directory = temporary.path().canonicalize().unwrap();
        let discovery = bidder_discovery::scan(&directory).unwrap();
        let parsed = syn::parse_file(&discovery.render().unwrap()).unwrap();
        let modules: Vec<_> = parsed
            .items
            .iter()
            .filter_map(|item| {
                if let syn::Item::Mod(module) = item
                    && module.content.is_none()
                {
                    Some(module)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            modules
                .iter()
                .map(|module| module.ident.to_string())
                .collect::<Vec<_>>(),
            ["fake_alpha", "fake_gated"]
        );
        let path = directory.join("src/bidders/fake_gated/mod.rs");
        let path_text = path.to_str().unwrap();
        let expected: syn::ItemMod = syn::parse_quote! {
            #[cfg(feature = "experimental")]
            #[path = #path_text]
            mod fake_gated;
        };
        assert_eq!(
            modules[1].to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
        for (source, expected_name) in [
            (
                discovery.render_allocation_conformance().unwrap(),
                "bidder_allocation_conformance",
            ),
            (
                discovery.render_response_conformance().unwrap(),
                "bidder_response_conformance",
            ),
        ] {
            let tree = syn::parse_file(&source).unwrap();
            let [syn::Item::Mod(module)] = tree.items.as_slice() else {
                panic!("missing conformance runner");
            };
            assert_eq!(module.ident, expected_name);
            let items = &module.content.as_ref().unwrap().1;
            assert_eq!(
                items
                    .iter()
                    .filter(|item| matches!(item, syn::Item::Mod(_)))
                    .count(),
                2
            );
            assert_eq!(
                items
                    .iter()
                    .filter(|item| matches!(item, syn::Item::Fn(_)))
                    .count(),
                1
            );
        }
        let uid = syn::parse_file(&discovery.render_uid_policies().unwrap()).unwrap();
        let syn::Item::Mod(module) = &uid.items[0] else {
            panic!("missing UID policy module");
        };
        let owners: Vec<_> = module
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .filter_map(|item| {
                if let syn::Item::Mod(owner) = item {
                    Some(owner.ident.to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(owners, ["fake_reference"]);
        assert!(
            discovery
                .rerun_paths()
                .contains(&directory.join("src/bidders/fake_reference/uid_policy.rs"))
        );
    }

    #[test]
    fn public_reconciliation_rejects_unbound_metadata_and_missing_forwarding() {
        let temporary = fixture();
        let directory = temporary.path().canonicalize().unwrap();
        let discovery = bidder_discovery::scan(&directory).unwrap();
        let metadata = [BidderMetadataDeclaration {
            bidder: "fake_Public",
            implementation: "fake_missing",
            required_feature: None,
        }];
        let error = "metadata implementation has no factory: fake_Public".to_owned();
        assert_eq!(discovery.validate_metadata(&metadata), Err(error.clone()));
        assert_eq!(discovery.render_registry(&metadata), Err(error));
        let bound_metadata = [BidderMetadataDeclaration {
            implementation: "fake_gated",
            required_feature: Some("experimental"),
            ..metadata[0]
        }];
        assert_eq!(discovery.validate_metadata(&bound_metadata), Ok(()));
        assert_eq!(
            bidder_reconcile::validate(
                &bound_metadata,
                [("fake_gated", Some("experimental"))],
                &BTreeMap::from([("experimental".to_owned(), Vec::new())]),
            ),
            Err("missing config feature forwarding: experimental".to_owned())
        );
    }
}

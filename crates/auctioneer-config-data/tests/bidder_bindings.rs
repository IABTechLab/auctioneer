//! Pure compiler tests and checks against the actual build-generated static views.

#[cfg(test)]
mod tests {
    use auctioneer_config_data::types::{BidderImplementationBinding, BidderMetadataDeclaration};
    use auctioneer_config_data::{
        bidder_implementation_bindings, bidder_metadata_declarations, bidders,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::ptr;

    #[test]
    fn generated_binding_views_return_pointer_stable_static_slices() {
        let active: &'static [BidderImplementationBinding] = bidder_implementation_bindings();
        let all: &'static [BidderMetadataDeclaration] = bidder_metadata_declarations();
        assert!(!active.is_empty());
        assert!(!all.is_empty());
        for _ in 0..4_usize {
            assert!(ptr::eq(active, bidder_implementation_bindings()));
            assert!(ptr::eq(all, bidder_metadata_declarations()));
        }
    }

    #[test]
    fn generated_active_bindings_equal_the_actual_metadata_view() {
        let bindings = bidder_implementation_bindings();
        let metadata = bidders();
        assert_eq!(
            bindings.iter().map(|row| row.bidder).collect::<Vec<_>>(),
            metadata
                .iter()
                .map(|info| info.name.as_str())
                .collect::<Vec<_>>()
        );
        let all = bidder_metadata_declarations();
        for binding in bindings {
            assert!(all.iter().any(|declaration| {
                declaration.bidder == binding.bidder
                    && declaration.implementation == binding.implementation
            }));
        }
    }

    #[test]
    fn generated_all_declarations_preserve_unconditional_and_target_gated_rows() {
        // Read source declarations independently of the catalog/compiler under test.
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .with_file_name("auctioneer-bidder")
            .join("src/bidders");
        let mut expected = BTreeMap::new();
        for directory in fs::read_dir(root).unwrap() {
            let path = directory.unwrap().path().join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let manifest: toml::Table = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            let implementation = manifest["key"].as_str().unwrap();
            let feature = manifest
                .get("required_feature")
                .map(|value| value.as_str().unwrap());
            let native_only = manifest
                .get("native_only")
                .is_some_and(|value| value.as_bool().unwrap());
            for name in manifest
                .get("definitions")
                .and_then(toml::Value::as_table)
                .into_iter()
                .flat_map(|table| table.keys())
            {
                assert!(
                    expected
                        .insert(
                            name.clone(),
                            (
                                implementation.to_owned(),
                                feature.map(str::to_owned),
                                native_only
                            )
                        )
                        .is_none()
                );
            }
        }
        assert!(!expected.is_empty());
        // Product publications cover both target gates. Optional Cargo feature
        // gates have independent synthetic discovery/compiler controls.
        assert!(
            expected
                .values()
                .any(|(_, feature, native_only)| feature.is_none() && *native_only)
        );
        assert!(
            expected
                .values()
                .any(|(_, feature, native_only)| feature.is_none() && !*native_only)
        );
        let declarations = bidder_metadata_declarations();
        let actual: Vec<_> = declarations
            .iter()
            .map(|row| (row.bidder, row.implementation, row.required_feature))
            .collect();
        assert_eq!(
            actual,
            expected
                .iter()
                .map(|(name, (implementation, feature, _))| (
                    name.as_str(),
                    implementation.as_str(),
                    feature.as_deref()
                ))
                .collect::<Vec<_>>()
        );
        let expected_active: Vec<_> = expected
            .iter()
            .filter(|(_, (_, feature, native_only))| {
                let feature_active = match feature.as_deref() {
                    None => true,
                    Some("insecure-bidders") => cfg!(feature = "insecure-bidders"),
                    Some(unknown) => panic!("add explicit test feature selection for {unknown}"),
                };
                feature_active && (!*native_only || !cfg!(target_arch = "wasm32"))
            })
            .map(|(name, (implementation, _, _))| (name.as_str(), implementation.as_str()))
            .collect();
        let active: Vec<_> = bidder_implementation_bindings()
            .iter()
            .map(|row| (row.bidder, row.implementation))
            .collect();
        assert_eq!(active, expected_active);
    }
}

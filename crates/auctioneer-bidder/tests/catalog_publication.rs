//! Execute source-to-publication conformance natively and inside WASI.
//! Guest execution preopens the workspace containing `CARGO_MANIFEST_DIR`.

#[cfg(test)]
mod tests {
    use auctioneer_bidder::{bidder_declarations, compiled_bidders, configured_registry};
    use auctioneer_config::{
        bidder_implementation_bindings, bidder_metadata_declarations, bidders,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::env::consts::ARCH;
    use std::fs;
    use std::path::Path;

    type Declaration = (String, Option<String>);

    struct SourceCatalog {
        declarations: BTreeMap<String, Declaration>,
        selected: BTreeMap<String, String>,
        factories: BTreeMap<String, Option<String>>,
        selected_factories: BTreeSet<String>,
    }

    fn feature_enabled(feature: Option<&str>) -> bool {
        match feature {
            None => true,
            Some("insecure-bidders") => cfg!(feature = "insecure-bidders"),
            Some(unknown) => panic!("unreviewed publication feature: {unknown}"),
        }
    }

    fn source_catalog() -> SourceCatalog {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders");
        let mut catalog = SourceCatalog {
            declarations: BTreeMap::new(),
            selected: BTreeMap::new(),
            factories: BTreeMap::new(),
            selected_factories: BTreeSet::new(),
        };
        let mut native_publications = false;
        let mut portable_publications = false;
        let mut feature_publications = false;
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path().join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let source: toml::Value = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            let owner = source["key"].as_str().unwrap();
            let feature = source
                .get("required_feature")
                .map(|row| row.as_str().unwrap());
            let enabled = feature_enabled(feature);
            let native = source
                .get("native_only")
                .is_some_and(|row| row.as_bool().unwrap());
            if source["factory"].as_bool().unwrap() {
                assert!(
                    catalog
                        .factories
                        .insert(owner.to_owned(), feature.map(str::to_owned))
                        .is_none()
                );
                if enabled {
                    assert!(catalog.selected_factories.insert(owner.to_owned()));
                }
            }
            let definitions = source.get("definitions").and_then(toml::Value::as_table);
            for name in definitions.into_iter().flat_map(|rows| rows.keys()) {
                assert!(
                    catalog
                        .declarations
                        .insert(name.clone(), (owner.to_owned(), feature.map(str::to_owned)))
                        .is_none()
                );
                native_publications |= native;
                portable_publications |= !native;
                feature_publications |= feature.is_some();
                let target_permits = !matches!((native, ARCH), (true, "wasm32"));
                if enabled && target_permits {
                    assert!(
                        catalog
                            .selected
                            .insert(name.clone(), owner.to_owned())
                            .is_none()
                    );
                }
            }
        }
        assert!(
            native_publications,
            "native-only source controls must exist"
        );
        assert!(
            portable_publications,
            "portable positive controls must exist"
        );
        assert!(feature_publications, "feature selection must be exercised");
        assert!(
            !catalog.selected.is_empty(),
            "publication cannot pass vacuously"
        );
        catalog
    }

    #[test]
    fn generated_metadata_and_bindings_obey_source_target_and_feature_gates() {
        let source = source_catalog();
        let metadata = bidders();
        let actual_names: BTreeSet<_> = metadata.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(
            actual_names.len(),
            metadata.len(),
            "metadata has duplicate names"
        );
        let expected_names: BTreeSet<_> = source.selected.keys().map(String::as_str).collect();
        assert_eq!(actual_names, expected_names);

        let bindings = bidder_implementation_bindings();
        let actual_bindings: BTreeMap<_, _> = bindings
            .iter()
            .map(|row| (row.bidder.to_owned(), row.implementation.to_owned()))
            .collect();
        assert_eq!(
            actual_bindings.len(),
            bindings.len(),
            "bindings have duplicate names"
        );
        assert_eq!(actual_bindings, source.selected);

        let declarations = bidder_metadata_declarations();
        let actual_declarations: BTreeMap<_, _> = declarations
            .iter()
            .map(|row| {
                (
                    row.bidder.to_owned(),
                    (
                        row.implementation.to_owned(),
                        row.required_feature.map(str::to_owned),
                    ),
                )
            })
            .collect();
        assert_eq!(
            actual_declarations.len(),
            declarations.len(),
            "declarations have duplicate names"
        );
        assert_eq!(actual_declarations, source.declarations);
    }

    #[test]
    fn serving_registry_obeys_publication_gates_without_losing_factory_declarations() {
        let source = source_catalog();
        let registry = configured_registry().unwrap();
        let actual_names: BTreeSet<_> = registry
            .base_names()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(actual_names, source.selected.keys().cloned().collect());

        let declarations = bidder_declarations();
        let actual_declarations: BTreeMap<_, _> = declarations
            .iter()
            .map(|row| (row.key.to_owned(), row.required_feature.map(str::to_owned)))
            .collect();
        assert_eq!(
            actual_declarations.len(),
            declarations.len(),
            "factory declarations have duplicates"
        );
        assert_eq!(actual_declarations, source.factories);

        let factories = compiled_bidders();
        let actual_factories: BTreeSet<_> =
            factories.iter().map(|row| row.key.to_owned()).collect();
        assert_eq!(
            actual_factories.len(),
            factories.len(),
            "compiled factories have duplicates"
        );
        assert_eq!(actual_factories, source.selected_factories);
    }
}

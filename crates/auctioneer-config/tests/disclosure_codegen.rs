//! Integration coverage of the public compiler and actual generated descriptors.

#[cfg(test)]
mod tests {
    use auctioneer_config::disclosure::{self, Domains};
    use auctioneer_config_schema::build_disclosure::compile;
    use std::path::Path;
    use time::OffsetDateTime;

    #[test]
    fn public_compiler_matches_the_build_generated_disclosure_tables() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let compiled = compile(&data, OffsetDateTime::now_utc().date()).unwrap();
        assert_eq!(
            compiled.rust_source,
            include_str!(concat!(env!("OUT_DIR"), "/disclosure.rs"))
        );
    }

    #[test]
    fn runtime_tables_have_sorted_fields_and_bounded_identifier_lookup() {
        assert_eq!(disclosure::SCHEMAS.len(), 11);
        assert_eq!(
            disclosure::SCHEMAS
                .iter()
                .map(|schema| schema.fields.len())
                .sum::<usize>(),
            86
        );
        let names: Vec<&str> = disclosure::SCHEMAS
            .iter()
            .map(|schema| schema.name)
            .collect();
        assert_eq!(
            names,
            [
                "BrandVersion",
                "Device",
                "DeviceExt",
                "DeviceExtPrebid",
                "DeviceInterstitial",
                "EID",
                "Geo",
                "UID",
                "User",
                "UserAgent",
                "UserExt"
            ]
        );
        for (index, schema) in disclosure::SCHEMAS.iter().enumerate() {
            assert_eq!(schema.id.index(), index);
            assert_eq!(schema.id.descriptor(), schema);
            assert!(schema.domains.intersects(Domains::ID));
            assert!(schema.domains.intersects(Domains::GEO));
            for pair in schema.fields.windows(2) {
                assert!(matches!(pair, [left, right] if left.name < right.name));
            }
            for field in schema.fields {
                assert_eq!(schema.field(field.name), Some(field));
                assert_eq!(field.id.descriptor(), field);
            }
            assert_eq!(schema.field("not-a-reviewed-field"), None);
        }
        for (global_index, field) in disclosure::SCHEMAS
            .iter()
            .flat_map(|schema| schema.fields)
            .enumerate()
        {
            assert_eq!(field.id.index(), global_index);
        }
    }
}

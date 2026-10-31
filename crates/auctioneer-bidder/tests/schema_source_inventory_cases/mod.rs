#[cfg(test)]
mod tests {
    use crate::schema_qualification::Draft;
    use crate::{schema_profile, schema_qualification};
    use std::collections::BTreeSet;
    use std::path::Path;

    #[test]
    fn committed_schema_snapshot_has_exact_source_witnesses() {
        let inputs = schema_profile::inputs();
        let result = &inputs.snapshot;
        assert_eq!(result.schemas.len(), schema_profile::schema_count());
        if crate::SCHEMA_SOURCE_CONFORMANCE {
            assert_eq!(result.revision, "d7f8515b86258688304b0d9b6668c6a0e258bc9e");
            assert_eq!(result.schema_bytes, 164_518);
            assert_eq!(result.largest_schema.bytes, 3_778);
            assert_eq!(
                result.largest_schema.source_blob,
                "37909b12066ad9239b488b9751d070212e90d61a"
            );
        } else {
            assert_eq!(result.revision, schema_qualification::fixture::REVISION);
            assert_eq!(
                result.schema_bytes,
                schema_qualification::fixture::schema_bytes()
            );
            assert_eq!(result.largest_schema.name, "fake_base");
            assert_eq!(result.largest_schema.path, "fake_base/parameters.json");
            assert_eq!(
                result.largest_schema.bytes,
                schema_qualification::fixture::largest_bytes()
            );
        }
        assert_eq!(
            result
                .schemas
                .iter()
                .filter(|row| row.audit.draft == Draft::Draft4)
                .count(),
            schema_profile::schema_count() - 1
        );
        let modern = result
            .schemas
            .iter()
            .find(|row| row.audit.draft == Draft::Draft202012)
            .expect("the non-Draft4 source fixture is required");
        assert_eq!(modern.audit.draft, Draft::Draft202012);
        assert!(
            modern
                .audit
                .validation_keywords
                .iter()
                .any(|item| item.pointer
                    == if crate::SCHEMA_SOURCE_CONFORMANCE {
                        "/allOf/0/if/required"
                    } else {
                        "/if/type"
                    })
        );
        let formats: BTreeSet<_> = result
            .schemas
            .iter()
            .flat_map(|row| &row.audit.formats)
            .map(|item| item.value.as_str())
            .collect();
        assert_eq!(formats, BTreeSet::from(["hostname", "uuid"]));
        assert!(
            result
                .schemas
                .iter()
                .all(|row| row.audit.unknown_keywords.is_empty())
        );
        assert!(
            result
                .schemas
                .iter()
                .all(|row| row.audit.local_references.is_empty()
                    && row.audit.external_references.is_empty())
        );
        let encoded = serde_json::to_vec(&result).expect("inventory serializes");
        let repeated = if crate::SCHEMA_SOURCE_CONFORMANCE {
            schema_qualification::inventory(
                &result.root,
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/conformance/fixtures/reference/catalog/source-inventory.toml"),
            )
            .unwrap()
        } else {
            schema_qualification::fixture::inventory(
                &result.root,
                &inputs
                    .snapshot
                    .root
                    .parent()
                    .unwrap()
                    .join("source-inventory.toml"),
            )
            .unwrap()
        };
        assert_eq!(
            encoded,
            serde_json::to_vec(&repeated).expect("repeat serialization")
        );
    }
}

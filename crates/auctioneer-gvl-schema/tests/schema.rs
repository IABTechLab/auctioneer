//! Direct-call tests over the shared validators — `validate_gvl_json`,
//! `validate_manifest`, `validate_gcl_json`, `check_gcl_consistency`,
//! `validate_bundle_limits`, and `select_retained_versions`. No
//! cargo-build spawn: the build-failure harness proves the wiring, this
//! suite proves the rules.
#![expect(
    clippy::default_numeric_fallback,
    reason = "json! fixture literals read as wire JSON; per-literal \
              suffixes would obscure the fixtures under test"
)]

#[cfg(test)]
mod tests {
    use auctioneer_gvl_schema::{BundleLimitError, WASM_BUNDLE_MAX_RAW_BYTES};
    use auctioneer_gvl_schema::{
        GCL_MAX_RAW_BYTES, GclSchemaError, GvlSchemaError, ManifestError, RetentionError,
        ValidatedGvl, VersionMeta, check_gcl_consistency, select_retained_versions,
        validate_bundle_limits, validate_gcl_json, validate_gvl_json, validate_manifest,
    };
    use serde_json::{Value, json};
    use time::OffsetDateTime;
    use time::macros::datetime;

    // ─────────────────────── GVL fixture builders ───────────────────────

    /// Minimal valid spec-3 (v2.4-era policy 5) GVL.
    fn base_gvl() -> Value {
        json!({
            "gvlSpecificationVersion": 3,
            "tcfPolicyVersion": 5,
            "vendorListVersion": 42,
            "lastUpdated": "2026-07-01T00:00:00Z",
            "purposes": {
                "1": {"id": 1, "name": "Store and/or access information on a device"},
                "2": {"id": 2, "name": "Use limited data to select advertising"},
                "3": {"id": 3, "name": "Create profiles for personalised advertising"},
                "4": {"id": 4, "name": "Use profiles to select personalised advertising"},
                "7": {"id": 7, "name": "Measure advertising performance"},
                "11": {"id": 11, "name": "Use limited data to select content"}
            },
            "specialPurposes": {
                "1": {"id": 1, "name": "Ensure security, prevent and detect fraud"},
                "2": {"id": 2, "name": "Deliver and present advertising and content"}
            },
            "features": {
                "1": {"id": 1, "name": "Match and combine data from other data sources"}
            },
            "specialFeatures": {
                "1": {"id": 1, "name": "Use precise geolocation data"}
            },
            "vendors": {
                "8": {"id": 8, "name": "Vendor Eight", "purposes": [1, 2],
                       "legIntPurposes": [7], "flexiblePurposes": [2],
                       "specialPurposes": [1], "features": [1], "specialFeatures": [1]}
            }
        })
    }

    /// Minimal valid spec-2 (policy 2) GVL, frozen-list flavored.
    fn spec2_gvl() -> Value {
        let mut root = base_gvl();
        merge_top(&mut root, "gvlSpecificationVersion", json!(2));
        merge_top(&mut root, "tcfPolicyVersion", json!(2));
        merge_top(&mut root, "vendorListVersion", json!(224));
        merge_top(&mut root, "lastUpdated", json!("2023-11-16T16:05:29Z"));
        root
    }

    /// Replace or insert a top-level key.
    fn merge_top(root: &mut Value, key: &str, value: Value) {
        if let Some(obj) = root.as_object_mut() {
            obj.insert(key.to_owned(), value);
        }
    }

    /// Serialize the blob minus one top-level key.
    fn without_top(root: &Value, key: &str) -> String {
        let mut obj = root.as_object().cloned().unwrap_or_default();
        obj.remove(key);
        Value::Object(obj).to_string()
    }

    /// Insert one entry into a top-level table (`vendors`, `purposes`, …).
    fn with_table_entry(root: &Value, table: &str, key: &str, entry: Value) -> Value {
        let mut obj = root.as_object().cloned().unwrap_or_default();
        let mut entries = obj
            .get(table)
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        entries.insert(key.to_owned(), entry);
        obj.insert(table.to_owned(), Value::Object(entries));
        Value::Object(obj)
    }

    fn validate(root: &Value, spec: Option<u32>) -> Result<ValidatedGvl, GvlSchemaError> {
        validate_gvl_json(&root.to_string(), spec)
    }

    // ───────────────────────── Base block (12) ─────────────────────────

    #[test]
    fn valid_minimal_gvl_returns_populated_validated_gvl() {
        let validated = validate(&base_gvl(), Some(3)).expect("minimal GVL validates");
        assert_eq!(validated.gvl_specification_version, 3);
        assert_eq!(validated.tcf_policy_version, 5);
        assert_eq!(validated.vendor_list_version, 42);
        assert_eq!(validated.last_updated, datetime!(2026-07-01 00:00:00 UTC));
        assert!(validated.purposes.contains_key(&1));
        let vendor = validated.vendors.get(&8).expect("vendor 8 lowered");
        assert_eq!(vendor.purposes, vec![1, 2]);
        assert_eq!(vendor.leg_int_purposes, vec![7]);
        assert!(vendor.deleted_date.is_none());
        // Debug round-trip and equality against a re-validation of the
        // same bytes.
        let again = validate(&base_gvl(), Some(3)).expect("deterministic");
        assert_eq!(format!("{validated:?}"), format!("{again:?}"));
        assert_eq!(validated, again);
    }

    #[test]
    fn gvl_missing_each_required_top_level_field_rejected() {
        let required = [
            "gvlSpecificationVersion",
            "tcfPolicyVersion",
            "vendorListVersion",
            "lastUpdated",
            "purposes",
            "specialPurposes",
            "features",
            "specialFeatures",
            "vendors",
        ];
        for field in required {
            let err = validate_gvl_json(&without_top(&base_gvl(), field), Some(3))
                .expect_err("absent required field must reject");
            assert_eq!(
                err,
                GvlSchemaError::MissingRequiredField(field),
                "wrong variant for absent {field}"
            );
        }
    }

    #[test]
    fn gvl_with_malformed_vendor_entry_returns_malformed_entry() {
        // Well-formed map key, entry body missing `id` — the typed pass
        // reports the dotted path.
        let root = with_table_entry(&base_gvl(), "vendors", "5", json!({"name": "No Id"}));
        let err = validate(&root, Some(3)).expect_err("id-less vendor must reject");
        let GvlSchemaError::MalformedEntry {
            table,
            path,
            reason,
        } = err
        else {
            panic!("expected MalformedEntry, got {err:?}");
        };
        assert_eq!(table, "vendors");
        assert_eq!(path, "vendors.5");
        assert!(reason.contains("missing field `id`"), "reason: {reason}");
    }

    #[test]
    fn unparseable_json_returns_invalid_json() {
        let err = validate_gvl_json("{not json", Some(3)).expect_err("must reject");
        assert!(matches!(err, GvlSchemaError::InvalidJson(_)));
    }

    #[test]
    fn gvl_spec_version_2_against_spec_expected_3_returns_unsupported_spec_version() {
        let err = validate(&spec2_gvl(), Some(3)).expect_err("cross-spec must reject");
        assert_eq!(
            err,
            GvlSchemaError::UnsupportedSpecVersion {
                got: 2,
                expected: 3
            }
        );
    }

    #[test]
    fn vendors_map_key_id_mismatch_returns_idkey_mismatch() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "5",
            json!({"id": 7, "name": "Shifted"}),
        );
        let err = validate(&root, Some(3)).expect_err("key/id mismatch must reject");
        assert_eq!(
            err,
            GvlSchemaError::IdKeyMismatch {
                table: "vendors",
                map_key: "5".to_owned(),
                embedded_id: 7
            }
        );
    }

    #[test]
    fn gvl_with_purpose_11_is_accepted() {
        let validated = validate(&base_gvl(), Some(3)).expect("purpose 11 is real IAB policy");
        assert!(validated.purposes.contains_key(&11));
    }

    #[test]
    fn gvl_purpose_id_25_returns_out_of_range() {
        let root = with_table_entry(
            &base_gvl(),
            "purposes",
            "25",
            json!({"id": 25, "name": "Past the 24-bit wire field"}),
        );
        let err = validate(&root, Some(3)).expect_err("purpose 25 must reject");
        assert_eq!(err, GvlSchemaError::PurposeIdOutOfRange { purpose_id: 25 });
    }

    #[test]
    fn gvl_special_feature_id_13_returns_out_of_range() {
        let root = with_table_entry(
            &base_gvl(),
            "specialFeatures",
            "13",
            json!({"id": 13, "name": "Past the 12-bit wire field"}),
        );
        let err = validate(&root, Some(3)).expect_err("special feature 13 must reject");
        assert_eq!(
            err,
            GvlSchemaError::SpecialFeatureIdOutOfRange {
                special_feature_id: 13
            }
        );
    }

    #[test]
    fn gvl_with_unknown_special_purpose_reference_returns_unknown_purpose_reference() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Dangling", "specialPurposes": [9]}),
        );
        let err = validate(&root, Some(3)).expect_err("dangling reference must reject");
        assert_eq!(
            err,
            GvlSchemaError::UnknownPurposeReference {
                vendor_id: 9,
                purpose_id: 9,
                field: "specialPurposes"
            }
        );
    }

    #[test]
    fn gvl_vendor_with_deleted_date_deserialises_field() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Gone", "purposes": [1],
                   "deletedDate": "2026-01-15T00:00:00Z"}),
        );
        let validated = validate(&root, Some(3)).expect("deleted vendor still validates");
        let vendor = validated.vendors.get(&9).expect("record retained");
        assert_eq!(
            vendor.deleted_date,
            Some(datetime!(2026-01-15 00:00:00 UTC))
        );
    }

    #[test]
    fn deleted_vendor_688_flex_without_default_tolerated() {
        // The official v2 list ships deleted records with legacy flex
        // inconsistencies (vendor 688: flexiblePurposes [7], 7 in
        // NEITHER default list). Tombstones (deleted_date <=
        // lastUpdated) skip the flex invariants; the same shape on an
        // ACTIVE vendor is still rejected.
        let tombstone = with_table_entry(
            &spec2_gvl(),
            "vendors",
            "688",
            json!({"id": 688, "name": "Legacy Flex", "flexiblePurposes": [7],
                   "deletedDate": "2022-06-01T00:00:00Z"}),
        );
        let validated = validate(&tombstone, Some(2)).expect("official-v2 legacy flex tolerated");
        assert!(
            validated
                .vendors
                .get(&688)
                .and_then(|vendor| vendor.deleted_date)
                .is_some()
        );
        let active = with_table_entry(
            &spec2_gvl(),
            "vendors",
            "688",
            json!({"id": 688, "name": "Legacy Flex", "flexiblePurposes": [7]}),
        );
        let err = validate(&active, Some(2)).expect_err("active vendor gets the full 5b block");
        assert_eq!(
            err,
            GvlSchemaError::FlexPurposeWithoutDefault {
                vendor_id: 688,
                purpose_id: 7
            }
        );
    }

    // ────────────────────── GVL named additions ──────────────────────

    #[test]
    fn validate_gvl_json_accepts_special_purpose_only_vendor() {
        // No "special-purpose-only requires an LI declaration" rule:
        // that removed workaround was a TC-string concern, never a
        // GVL-schema rule.
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Security Only", "specialPurposes": [1]}),
        );
        let validated = validate(&root, Some(3)).expect("special-purpose-only vendor validates");
        let vendor = validated.vendors.get(&9).expect("vendor lowered");
        assert!(vendor.purposes.is_empty());
        assert!(vendor.leg_int_purposes.is_empty());
        assert_eq!(vendor.special_purposes, vec![1]);
    }

    #[test]
    fn flex_basis_overlap_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Dual Default", "purposes": [3], "legIntPurposes": [3]}),
        );
        let err = validate(&root, Some(3)).expect_err("dual default basis must reject");
        assert_eq!(
            err,
            GvlSchemaError::FlexBasisOverlap {
                vendor_id: 9,
                purpose_id: 3
            }
        );
    }

    #[test]
    fn flex_purpose_without_default_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "No Default", "purposes": [1], "flexiblePurposes": [4]}),
        );
        let err = validate(&root, Some(3)).expect_err("flex without default must reject");
        assert_eq!(
            err,
            GvlSchemaError::FlexPurposeWithoutDefault {
                vendor_id: 9,
                purpose_id: 4
            }
        );
    }

    #[test]
    fn flexible_purpose_one_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Flexible Storage", "purposes": [1], "flexiblePurposes": [1]}),
        );
        let err = validate(&root, Some(3)).expect_err("purpose 1 is consent-only");
        assert_eq!(
            err,
            GvlSchemaError::FlexiblePurposeOneForbidden { vendor_id: 9 }
        );
    }

    #[test]
    fn vendor_deleted_date_malformed_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Bad Instant", "purposes": [1], "deletedDate": "yesterday"}),
        );
        let err = validate(&root, Some(3)).expect_err("malformed deletedDate must reject");
        assert_eq!(
            err,
            GvlSchemaError::BadTimestamp {
                path: "vendors.9.deletedDate".to_owned(),
                raw: "yesterday".to_owned()
            }
        );
    }

    #[test]
    fn future_deleted_vendor_with_malformed_flex_rejected() {
        // deletedDate one day AFTER the list's lastUpdated: the vendor
        // is still authorizable, so the presence-based exemption would
        // be fail-open — the full flex block applies.
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Future Tombstone", "flexiblePurposes": [7],
                   "deletedDate": "2026-07-02T00:00:00Z"}),
        );
        let err = validate(&root, Some(3)).expect_err("future-dated vendor gets 5b");
        assert_eq!(
            err,
            GvlSchemaError::FlexPurposeWithoutDefault {
                vendor_id: 9,
                purpose_id: 7
            }
        );
    }

    #[test]
    fn gvl_last_updated_malformed_rejected() {
        let mut root = base_gvl();
        merge_top(&mut root, "lastUpdated", json!("16/11/2023"));
        let err = validate(&root, Some(3)).expect_err("malformed lastUpdated must reject");
        assert_eq!(
            err,
            GvlSchemaError::BadTimestamp {
                path: "lastUpdated".to_owned(),
                raw: "16/11/2023".to_owned()
            }
        );
    }

    #[test]
    fn purposes_entry_missing_subfield_returns_malformed_entry() {
        let root = with_table_entry(&base_gvl(), "purposes", "3", json!({"id": 3}));
        let err = validate(&root, Some(3)).expect_err("name-less purpose must reject");
        assert!(matches!(err, GvlSchemaError::MalformedEntry { table, .. } if table == "purposes"));
    }

    #[test]
    fn special_purposes_entry_missing_subfield_returns_malformed_entry() {
        let root = with_table_entry(&base_gvl(), "specialPurposes", "2", json!({"id": 2}));
        let err = validate(&root, Some(3)).expect_err("name-less special purpose must reject");
        assert!(matches!(
            err,
            GvlSchemaError::MalformedEntry { table, .. } if table == "special_purposes"
        ));
    }

    #[test]
    fn features_entry_missing_subfield_returns_malformed_entry() {
        let root = with_table_entry(&base_gvl(), "features", "1", json!({"id": 1}));
        let err = validate(&root, Some(3)).expect_err("name-less feature must reject");
        assert!(matches!(err, GvlSchemaError::MalformedEntry { table, .. } if table == "features"));
    }

    #[test]
    fn special_features_entry_missing_subfield_returns_malformed_entry() {
        let root = with_table_entry(&base_gvl(), "specialFeatures", "1", json!({"id": 1}));
        let err = validate(&root, Some(3)).expect_err("name-less special feature must reject");
        assert!(matches!(
            err,
            GvlSchemaError::MalformedEntry { table, .. } if table == "special_features"
        ));
    }

    #[test]
    fn gvl_spec_version_3_against_spec_expected_2_returns_unsupported_spec_version() {
        let err = validate(&base_gvl(), Some(2)).expect_err("cross-spec must reject");
        assert_eq!(
            err,
            GvlSchemaError::UnsupportedSpecVersion {
                got: 3,
                expected: 2
            }
        );
    }

    #[test]
    fn gvl_spec_version_2_against_spec_expected_2_returns_ok() {
        let validated = validate(&spec2_gvl(), Some(2)).expect("spec-2 slot accepts spec-2 blob");
        assert_eq!(validated.gvl_specification_version, 2);
    }

    #[test]
    fn spec2_blob_under_none_expected_ok() {
        let validated = validate(&spec2_gvl(), None).expect("self-describing spec-2 load");
        assert_eq!(validated.gvl_specification_version, 2);
    }

    #[test]
    fn spec3_blob_under_none_expected_ok() {
        let validated = validate(&base_gvl(), None).expect("self-describing spec-3 load");
        assert_eq!(validated.gvl_specification_version, 3);
    }

    #[test]
    fn invariants_still_run_under_none_expected() {
        // `None` skips ONLY the spec cross-check; every other invariant
        // still runs.
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "5",
            json!({"id": 7, "name": "Shifted"}),
        );
        let err = validate(&root, None).expect_err("IdKeyMismatch fires under None");
        assert_eq!(
            err,
            GvlSchemaError::IdKeyMismatch {
                table: "vendors",
                map_key: "5".to_owned(),
                embedded_id: 7
            }
        );
    }

    #[test]
    fn purposes_map_key_id_mismatch_returns_idkey_mismatch() {
        let root = with_table_entry(
            &base_gvl(),
            "purposes",
            "5",
            json!({"id": 7, "name": "Shifted"}),
        );
        let err = validate(&root, Some(3)).expect_err("must reject");
        assert!(matches!(err, GvlSchemaError::IdKeyMismatch { table, .. } if table == "purposes"));
    }

    #[test]
    fn special_purposes_map_key_id_mismatch_returns_idkey_mismatch() {
        let root = with_table_entry(
            &base_gvl(),
            "specialPurposes",
            "5",
            json!({"id": 7, "name": "Shifted"}),
        );
        let err = validate(&root, Some(3)).expect_err("must reject");
        assert!(matches!(
            err,
            GvlSchemaError::IdKeyMismatch { table, .. } if table == "special_purposes"
        ));
    }

    #[test]
    fn features_map_key_id_mismatch_returns_idkey_mismatch() {
        let root = with_table_entry(
            &base_gvl(),
            "features",
            "5",
            json!({"id": 7, "name": "Shifted"}),
        );
        let err = validate(&root, Some(3)).expect_err("must reject");
        assert!(matches!(err, GvlSchemaError::IdKeyMismatch { table, .. } if table == "features"));
    }

    #[test]
    fn special_features_map_key_id_mismatch_returns_idkey_mismatch() {
        let root = with_table_entry(
            &base_gvl(),
            "specialFeatures",
            "5",
            json!({"id": 7, "name": "Shifted"}),
        );
        let err = validate(&root, Some(3)).expect_err("must reject");
        assert!(matches!(
            err,
            GvlSchemaError::IdKeyMismatch { table, .. } if table == "special_features"
        ));
    }

    #[test]
    fn vendors_zero_id_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "0",
            json!({"id": 0, "name": "Zero"}),
        );
        let err = validate(&root, Some(3)).expect_err("zero id must reject");
        assert_eq!(
            err,
            GvlSchemaError::ZeroId {
                table: "vendors",
                map_key: "0".to_owned()
            }
        );
    }

    #[test]
    fn purposes_zero_id_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "purposes",
            "0",
            json!({"id": 0, "name": "Zero"}),
        );
        let err = validate(&root, Some(3)).expect_err("zero id must reject");
        assert!(matches!(err, GvlSchemaError::ZeroId { table, .. } if table == "purposes"));
    }

    #[test]
    fn special_purposes_zero_id_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "specialPurposes",
            "0",
            json!({"id": 0, "name": "Zero"}),
        );
        let err = validate(&root, Some(3)).expect_err("zero id must reject");
        assert!(matches!(err, GvlSchemaError::ZeroId { table, .. } if table == "special_purposes"));
    }

    #[test]
    fn features_zero_id_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "features",
            "0",
            json!({"id": 0, "name": "Zero"}),
        );
        let err = validate(&root, Some(3)).expect_err("zero id must reject");
        assert!(matches!(err, GvlSchemaError::ZeroId { table, .. } if table == "features"));
    }

    #[test]
    fn special_features_zero_id_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "specialFeatures",
            "0",
            json!({"id": 0, "name": "Zero"}),
        );
        let err = validate(&root, Some(3)).expect_err("zero id must reject");
        assert!(matches!(err, GvlSchemaError::ZeroId { table, .. } if table == "special_features"));
    }

    #[test]
    fn vendor_unknown_normal_purpose_reference_rejected() {
        let root = with_table_entry(
            &base_gvl(),
            "vendors",
            "9",
            json!({"id": 9, "name": "Dangling", "purposes": [20]}),
        );
        let err = validate(&root, Some(3)).expect_err("purpose 20 is undeclared");
        assert_eq!(
            err,
            GvlSchemaError::UnknownPurposeReference {
                vendor_id: 9,
                purpose_id: 20,
                field: "purposes"
            }
        );
    }

    #[test]
    fn validate_gvl_json_accepts_v24_spec_version() {
        // The v2.4-era shape: gvlSpecificationVersion 3, tcfPolicyVersion
        // 5, plus the new standardTexts field (tolerated passthrough).
        let mut root = base_gvl();
        merge_top(
            &mut root,
            "standardTexts",
            json!({"purposes": {"1": {"id": 1, "legal": "text"}}}),
        );
        let validated = validate(&root, Some(3)).expect("v2.4 blob validates");
        assert_eq!(validated.tcf_policy_version, 5);
    }

    // ─────────────────────── Manifest validator ───────────────────────

    /// The pinned validation instant for manifest tests: inside the
    /// standard GCL record's lifetime.
    const NOW: OffsetDateTime = datetime!(2026-07-14 12:00:00 UTC);

    const STD_GCL: &str = r#""gcl": { "path": "gcl.json",
        "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        "last_updated": "2026-07-10T12:00:00Z",
        "fetched_at": "2026-07-14T00:00:00Z",
        "cache_max_age_secs": 604800 }"#;

    // Raw-string `const` manifest entry, not `serde_json::json!`: it is
    // spliced into the `format!` templates below as JSON text, and
    // callers rewrite its literal spelling with `replacen`. See the
    // note on `manifest_with_spec3`.
    const STD_SPEC2_ENTRY: &str = r#"{ "spec": 2, "vendor_list_version": 224,
        "tcf_policy_version": 2, "last_updated": "2023-11-16T16:05:29Z",
        "path": "spec2/vendor-list-v224.json",
        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }"#;

    /// A valid manifest around the caller's spec-3 entries (the frozen
    /// spec-2 entry supplies coverage).
    ///
    /// Raw `format!`, not `serde_json::json!`, and the same for
    /// `STD_SPEC2_ENTRY` and `spec3_entry` below. Two reasons: the
    /// pieces spliced in are strings of JSON text rather than `Value`s
    /// (`STD_GCL` is a bare `"key": value` fragment, and `extra` is a
    /// trailing `, "field": value`), and several callers mutate the
    /// rendered output with `replacen`/`replace` on its literal
    /// spelling — `"cache_max_age_secs": 604800`, the sha runs, the
    /// `"spec": 3` key. `json!` renders its own spacing, so those
    /// replacements would stop matching and the rows would assert
    /// nothing.
    fn manifest_with_spec3(entries: &str) -> String {
        format!(
            r#"{{ "manifest_version": 1, "generated_at": "2026-07-14T00:00:00Z",
                 "retention": {{ "max_age_days": 180 }},
                 {STD_GCL},
                 "entries": [ {STD_SPEC2_ENTRY}, {entries} ] }}"#
        )
    }

    fn spec3_entry(version: u32, last_updated: &str, extra: &str) -> String {
        format!(
            r#"{{ "spec": 3, "vendor_list_version": {version}, "tcf_policy_version": 5,
                 "last_updated": "{last_updated}",
                 "path": "spec3/vendor-list-v{version}.json",
                 "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"{extra} }}"#
        )
    }

    #[test]
    fn documented_manifest_example_passes_validate_manifest() {
        // The plan's documented example (sha fields substituted with
        // valid 64-hex), checked in as a fixture so the documentation
        // cannot drift from the validator.
        let raw = include_str!("fixtures/documented_manifest_example.json");
        let manifest = validate_manifest(raw, datetime!(2026-07-14 00:00:00 UTC))
            .expect("documented example validates");
        assert_eq!(manifest.entries.len(), 3);
        assert!(!manifest.oversize_bundle_ack);
    }

    #[test]
    fn refresh_tool_manifest_output_passes_build_validator() {
        // A frozen real refresh-tool output run through the same
        // validator build.rs uses — the shared-validator invariant:
        // whatever the tool writes, the build accepts.
        let raw = include_str!("fixtures/refresh_tool_output_manifest.json");
        let manifest = validate_manifest(raw, datetime!(2026-08-29 01:00:00 UTC))
            .expect("tool output validates");
        assert!(manifest.entries.iter().any(|entry| entry.spec == 2));
        assert!(manifest.entries.iter().any(|entry| entry.spec == 3));
    }

    #[test]
    fn manifest_rejects_manifest_version_2() {
        let raw = manifest_with_spec3(&spec3_entry(348, "2026-07-13T00:00:00Z", "")).replacen(
            "\"manifest_version\": 1",
            "\"manifest_version\": 2",
            1,
        );
        let err = validate_manifest(&raw, NOW).expect_err("version 2 must reject");
        assert_eq!(err, ManifestError::UnsupportedVersion { got: 2 });
    }

    #[test]
    fn manifest_rejects_malformed_rfc3339() {
        let raw = manifest_with_spec3(&spec3_entry(348, "2026-07-13T00:00:00Z", "")).replacen(
            "2026-07-14T00:00:00Z",
            "not-a-date",
            1,
        );
        let err = validate_manifest(&raw, NOW).expect_err("malformed timestamp must reject");
        assert!(matches!(
            err,
            ManifestError::BadTimestamp { field, .. } if field == "generated_at"
        ));
    }

    #[test]
    fn manifest_rejects_future_generated_at() {
        let raw = manifest_with_spec3(&spec3_entry(348, "2026-07-13T00:00:00Z", "")).replacen(
            "2026-07-14T00:00:00Z",
            "2026-07-15T00:00:00Z",
            1,
        );
        let err = validate_manifest(&raw, NOW).expect_err("future generated_at must reject");
        assert!(matches!(err, ManifestError::FutureGeneratedAt { .. }));
    }

    #[test]
    fn manifest_rejects_bad_sha256_length() {
        let entry = spec3_entry(348, "2026-07-13T00:00:00Z", "").replace(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "abc",
        );
        let err = validate_manifest(&manifest_with_spec3(&entry), NOW)
            .expect_err("short sha must reject");
        assert!(matches!(err, ManifestError::Sha256Format { .. }));
    }

    #[test]
    fn manifest_rejects_vendor_list_v_with_no_digits() {
        // Regression for the vacuous-`.all()` bug: `\d+` requires at
        // least one digit.
        let entry = spec3_entry(348, "2026-07-13T00:00:00Z", "")
            .replace("vendor-list-v348.json", "vendor-list-v.json");
        let err = validate_manifest(&manifest_with_spec3(&entry), NOW)
            .expect_err("digit-less path must reject");
        assert!(matches!(err, ManifestError::BadPath { .. }));
    }

    #[test]
    fn manifest_rejects_path_version_disagreeing_with_entry() {
        let entry = spec3_entry(348, "2026-07-13T00:00:00Z", "")
            .replace("vendor-list-v348.json", "vendor-list-v347.json");
        let err = validate_manifest(&manifest_with_spec3(&entry), NOW)
            .expect_err("path/version disagreement must reject");
        assert_eq!(
            err,
            ManifestError::PathVersionMismatch {
                path: "spec3/vendor-list-v347.json".to_owned(),
                declared: 348
            }
        );
    }

    #[test]
    fn manifest_accepts_absent_oversize_bundle_ack_as_false() {
        // The builder writes no oversize_bundle_ack key at all.
        let manifest = validate_manifest(
            &manifest_with_spec3(&spec3_entry(348, "2026-07-13T00:00:00Z", "")),
            NOW,
        )
        .expect("ack-less manifest parses");
        assert!(!manifest.oversize_bundle_ack);
    }

    #[test]
    fn frozen_v2_list_exempt_from_max_age_days() {
        // The frozen spec-2 entry (2023-11-16) is ~970 days old under a
        // 180-day window and still passes; the SAME age on a spec-3
        // entry is RetentionExceeded.
        let ok = validate_manifest(
            &manifest_with_spec3(&spec3_entry(348, "2026-07-13T00:00:00Z", "")),
            NOW,
        );
        assert!(ok.is_ok(), "frozen spec-2 must be age-exempt: {ok:?}");
        let stale_spec3 = validate_manifest(
            &manifest_with_spec3(&spec3_entry(348, "2023-11-16T16:05:29Z", "")),
            NOW,
        )
        .expect_err("a spec-3 entry that old must reject");
        assert!(matches!(
            stale_spec3,
            ManifestError::RetentionExceeded { .. }
        ));
    }

    #[test]
    fn manifest_missing_or_stale_gcl_rejected() {
        // MissingGcl arm: no gcl object at all. Raw `format!`, not
        // `serde_json::json!`, because it splices the same JSON-text
        // fragments as `manifest_with_spec3` — see the note there.
        let missing = format!(
            r#"{{ "manifest_version": 1, "generated_at": "2026-07-14T00:00:00Z",
                 "retention": {{ "max_age_days": 180 }},
                 "entries": [ {STD_SPEC2_ENTRY},
                              {} ] }}"#,
            spec3_entry(348, "2026-07-13T00:00:00Z", "")
        );
        assert_eq!(
            validate_manifest(&missing, NOW).expect_err("gcl is required"),
            ManifestError::MissingGcl
        );
        // GclStale arm: expired lifetime (now >= fetched_at + max_age).
        let stale = manifest_with_spec3(&spec3_entry(348, "2026-07-13T00:00:00Z", "")).replacen(
            "\"cache_max_age_secs\": 604800",
            "\"cache_max_age_secs\": 3600",
            1,
        );
        assert!(matches!(
            validate_manifest(&stale, NOW).expect_err("stale gcl must reject"),
            ManifestError::GclStale { .. }
        ));
    }

    #[test]
    fn entry_cache_metadata_pair_required_together() {
        let only_fetched = spec3_entry(
            348,
            "2026-07-13T00:00:00Z",
            r#", "fetched_at": "2026-07-14T00:00:00Z""#,
        );
        assert!(matches!(
            validate_manifest(&manifest_with_spec3(&only_fetched), NOW)
                .expect_err("fetched_at without lifetime must reject"),
            ManifestError::BadTimestamp { field, .. }
                if field == "entry.fetched_at/cache_max_age_secs"
        ));
        let only_secs = spec3_entry(348, "2026-07-13T00:00:00Z", r#", "cache_max_age_secs": 60"#);
        assert!(matches!(
            validate_manifest(&manifest_with_spec3(&only_secs), NOW)
                .expect_err("lifetime without fetched_at must reject"),
            ManifestError::BadTimestamp { field, .. }
                if field == "entry.fetched_at/cache_max_age_secs"
        ));
    }

    #[test]
    fn entry_fetched_at_future_rejected() {
        let entry = spec3_entry(
            348,
            "2026-07-13T00:00:00Z",
            r#", "fetched_at": "2026-07-15T00:00:00Z", "cache_max_age_secs": 60"#,
        );
        assert!(matches!(
            validate_manifest(&manifest_with_spec3(&entry), NOW)
                .expect_err("future fetched_at must reject"),
            ManifestError::FutureTimestamp { field, .. } if field == "entry.fetched_at"
        ));
    }

    #[test]
    fn entry_cache_expiry_overflow_rejected() {
        let entry = spec3_entry(
            348,
            "2026-07-13T00:00:00Z",
            r#", "fetched_at": "2026-07-14T00:00:00Z",
                "cache_max_age_secs": 18446744073709551615"#,
        );
        assert_eq!(
            validate_manifest(&manifest_with_spec3(&entry), NOW)
                .expect_err("u64::MAX lifetime must reject"),
            ManifestError::EntryExpiryOverflow {
                path: "spec3/vendor-list-v348.json".to_owned()
            }
        );
    }

    #[test]
    fn gcl_last_updated_mismatch_rejected() {
        let manifest = validate_manifest(
            &manifest_with_spec3(&spec3_entry(348, "2026-07-13T00:00:00Z", "")),
            NOW,
        )
        .expect("manifest validates");
        // The blob disagrees with the manifest's recorded instant.
        let gcl = validate_gcl_json(
            &serde_json::json!({"lastUpdated": "2026-07-11T12:00:00Z", "cmps": {"2": {"id": 2}}})
                .to_string(),
        )
        .expect("blob validates");
        assert!(matches!(
            check_gcl_consistency(&manifest, &gcl)
                .expect_err("manifest/blob disagreement must reject"),
            ManifestError::GclLastUpdatedMismatch { .. }
        ));
    }

    // ───────────────────────── GCL validator ─────────────────────────

    #[test]
    fn validate_gcl_json_accepts_official_shape() {
        // Excerpt of the real Global CMP List: active and deleted
        // entries, unknown fields (name/isCommercial/environments)
        // tolerated, fractional-second deletedDate parsed.
        let raw = include_str!("fixtures/gcl_official_shape.json");
        let validated = validate_gcl_json(raw).expect("official shape validates");
        assert_eq!(validated.last_updated, datetime!(2026-08-27 16:00:21 UTC));
        assert!(
            validated
                .cmps
                .get(&2)
                .is_some_and(|cmp| cmp.deleted_date.is_none())
        );
        assert!(
            validated
                .cmps
                .get(&3)
                .is_some_and(|cmp| cmp.deleted_date.is_some())
        );
        assert!(validated.cmps.contains_key(&515));
    }

    #[test]
    fn gcl_over_raw_cap_rejected() {
        // Raw string, not `serde_json::json!`: this row pads the
        // fixture past `GCL_MAX_RAW_BYTES`, so the assertion is about a
        // raw byte length and the rendered text is part of it.
        let mut raw = String::from(r#"{"lastUpdated": "2026-08-27T16:00:21Z", "cmps": {}}"#);
        raw.push_str(&" ".repeat(GCL_MAX_RAW_BYTES));
        let err = validate_gcl_json(&raw).expect_err("over-cap GCL must reject");
        assert!(matches!(
            err,
            GclSchemaError::InvalidJson(msg) if msg.contains("GCL_MAX_RAW_BYTES")
        ));
    }

    #[test]
    fn gcl_missing_last_updated_rejected() {
        let err = validate_gcl_json(&serde_json::json!({"cmps": {"2": {"id": 2}}}).to_string())
            .expect_err("lastUpdated is required");
        assert_eq!(err, GclSchemaError::MissingLastUpdated);
    }

    #[test]
    fn gcl_bad_timestamp_rejected() {
        // Root arm.
        let root_err = validate_gcl_json(
            &serde_json::json!({"lastUpdated": "not-a-date", "cmps": {}}).to_string(),
        )
        .expect_err("malformed root lastUpdated must reject");
        assert_eq!(
            root_err,
            GclSchemaError::BadTimestamp {
                path: "lastUpdated".to_owned(),
                raw: "not-a-date".to_owned()
            }
        );
        // deletedDate arm.
        let deleted_err = validate_gcl_json(
            &serde_json::json!({
                "lastUpdated": "2026-08-27T16:00:21Z",
                "cmps": {"3": {"id": 3, "deletedDate": "soon"}}
            })
            .to_string(),
        )
        .expect_err("malformed deletedDate must reject");
        assert_eq!(
            deleted_err,
            GclSchemaError::BadTimestamp {
                path: "cmps.3.deletedDate".to_owned(),
                raw: "soon".to_owned()
            }
        );
    }

    #[test]
    fn gcl_nonnumeric_map_key_rejected() {
        let err = validate_gcl_json(
            &serde_json::json!({
                "lastUpdated": "2026-08-27T16:00:21Z", "cmps": {"abc": {"id": 5}}
            })
            .to_string(),
        )
        .expect_err("non-numeric key must reject");
        assert_eq!(
            err,
            GclSchemaError::NonNumericMapKey {
                key: "abc".to_owned()
            }
        );
    }

    #[test]
    fn gcl_id_key_mismatch_rejected() {
        let err = validate_gcl_json(
            &serde_json::json!({
                "lastUpdated": "2026-08-27T16:00:21Z", "cmps": {"5": {"id": 7}}
            })
            .to_string(),
        )
        .expect_err("key/id mismatch must reject");
        assert_eq!(
            err,
            GclSchemaError::IdKeyMismatch {
                map_key: "5".to_owned(),
                embedded_id: 7
            }
        );
    }

    #[test]
    fn gcl_cmp_id_out_of_range_rejected() {
        let zero = validate_gcl_json(
            &serde_json::json!({
                "lastUpdated": "2026-08-27T16:00:21Z", "cmps": {"0": {"id": 0}}
            })
            .to_string(),
        )
        .expect_err("CmpId is nonzero");
        assert_eq!(zero, GclSchemaError::CmpIdOutOfRange { id: 0 });
        let high = validate_gcl_json(
            &serde_json::json!({
                "lastUpdated": "2026-08-27T16:00:21Z", "cmps": {"4096": {"id": 4096}}
            })
            .to_string(),
        )
        .expect_err("CmpId is 12-bit");
        assert_eq!(high, GclSchemaError::CmpIdOutOfRange { id: 4096 });
    }

    // ──────────────────── Bundle-limit / retention ────────────────────

    #[test]
    fn bundle_over_cap_rejected_even_when_ack_true() {
        // The ack is not an input — the cap is unconditional.
        let cap: u64 = 1_000;
        assert_eq!(
            validate_bundle_limits("x86_64", cap, cap.saturating_add(1))
                .expect_err("over-cap bundle must reject"),
            BundleLimitError::BundleOverCap {
                bundle_bytes: 1_001,
                configured_cap: 1_000
            }
        );
    }

    #[test]
    fn wasm_cap_above_24_mib_hard_max_rejected() {
        let above = WASM_BUNDLE_MAX_RAW_BYTES.saturating_add(1);
        assert_eq!(
            validate_bundle_limits("wasm32", above, 0)
                .expect_err("wasm cap above the hard max must reject"),
            BundleLimitError::WasmCapAboveHardMax {
                configured_cap: above
            }
        );
        // The same cap on a native target is accepted.
        validate_bundle_limits("x86_64", above, 0).expect("native targets may raise the cap");
    }

    fn meta(spec: u32, version: u32, last_updated: OffsetDateTime, raw_bytes: u64) -> VersionMeta {
        VersionMeta {
            spec,
            version,
            last_updated,
            raw_bytes,
        }
    }

    #[test]
    fn growth_margin_above_raw_cap_is_budget_underflow() {
        let err = select_retained_versions(&[], &[], 1_000, 2_000, NOW)
            .expect_err("margin above cap must reject");
        assert_eq!(
            err,
            RetentionError::BudgetUnderflow {
                raw_cap: 1_000,
                growth_margin: 2_000
            }
        );
    }

    #[test]
    fn reserved_pair_over_budget_is_error() {
        // A mandatory version is never silently dropped to fit.
        let reserved = [
            meta(2, 224, datetime!(2023-11-16 16:05:29 UTC), 2_000),
            meta(3, 100, datetime!(2026-07-13 00:00:00 UTC), 1_000),
        ];
        let err = select_retained_versions(&[], &reserved, 2_500, 100, NOW)
            .expect_err("reserved pair over budget must reject");
        assert_eq!(
            err,
            RetentionError::ReservedPairOverBudget {
                reserved_bytes: 3_000,
                budget: 2_400
            }
        );
    }

    #[test]
    fn fills_spec3_newest_first_until_size_budget() {
        let reserved = [
            meta(2, 224, datetime!(2023-11-16 16:05:29 UTC), 400),
            meta(3, 100, datetime!(2026-07-13 00:00:00 UTC), 500),
        ];
        let candidates = [
            meta(3, 99, datetime!(2026-06-01 00:00:00 UTC), 600),
            meta(3, 98, datetime!(2026-05-01 00:00:00 UTC), 600),
            meta(3, 97, datetime!(2026-04-01 00:00:00 UTC), 600),
            meta(3, 96, datetime!(2026-03-01 00:00:00 UTC), 600),
            meta(3, 95, datetime!(2026-02-01 00:00:00 UTC), 600),
        ];
        // budget = 3000 - 100 = 2900; reserved 900; the fill takes
        // v99 (1500), v98 (2100), v97 (2700) and stops at v96 (3300).
        let keep = select_retained_versions(
            &candidates,
            &reserved,
            3_000,
            100,
            datetime!(2026-01-01 00:00:00 UTC),
        )
        .expect("selection succeeds");
        let versions: Vec<(u32, u32)> = keep.iter().map(|kept| (kept.spec, kept.version)).collect();
        assert_eq!(
            versions,
            vec![(2, 224), (3, 100), (3, 99), (3, 98), (3, 97)]
        );
    }
}

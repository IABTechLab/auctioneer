//! GVL / GCL / manifest schema validation shared by `auctioneer-config-data`'s
//! build script, the `auctioneer-privacy` runtime lowering, and
//! `auctioneer-cli`'s archive-independent `gvl refresh` maintenance command.
//!
//! One validator, one parse path: every consumer funnels the same raw
//! bytes through [`validate_gvl_json`] / [`validate_gcl_json`] /
//! [`validate_manifest`], so schema and hostile-input invariants hold
//! identically at build time and at runtime. The crate deliberately has
//! no build script and no `auctioneer-config-data` dependency — it must build
//! even when the checked-in archive is broken, or the refresh tool
//! could never repair it — and it must keep cross-compiling to wasm32
//! (so no `fs2`; [`recover_gvl_archive`] is pure `std::fs` and the
//! caller supplies the held advisory lock).

use serde_json::Value;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

/// Raw-size bound on the Global CMP List blob (the official list is a
/// few hundred KB; anything near this cap is upstream breakage or an
/// inflation attack). The refresh tool enforces it on the decoded
/// network stream; `validate_gcl_json` re-enforces it on every parse so
/// the build gate and runtime share the bound.
pub const GCL_MAX_RAW_BYTES: usize = 0x0010_0000;

/// Hard platform maximum for the RAW GVL bundle on wasm targets: the
/// 128 MB isolate budget reserves at most 24 MiB of
/// raw GVL. The single committed archive feeds all four adapters, so
/// every archive must fit this bound regardless of a raised native cap.
pub const WASM_BUNDLE_MAX_RAW_BYTES: u64 = 0x0180_0000;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GvlSchemaError {
    #[error("GVL is not valid JSON: {0}")]
    InvalidJson(String),
    #[error("GVL missing required top-level field {0:?}")]
    MissingRequiredField(&'static str),
    /// Table-generic malformed-entry variant. Applies to any of the
    /// five indexed tables (vendors, purposes, special purposes,
    /// features, special features). `path` is the dotted field path
    /// produced by `serde_path_to_error::deserialize` — precise enough
    /// that operators can grep the offending row in the source JSON.
    #[error("GVL table {table:?} entry at {path:?} is malformed: {reason}")]
    MalformedEntry {
        table: &'static str,
        path: String,
        reason: String,
    },
    /// A timestamp field that fails RFC 3339 parsing during validation —
    /// the vendor `deletedDate` (`path` = the dotted vendor path) and
    /// the root `lastUpdated` (`path` == "lastUpdated" exactly).
    /// Distinct from `MalformedEntry` so operators can tell a shape
    /// error from a bad instant.
    #[error("GVL timestamp at {path:?} is not RFC 3339: {raw:?}")]
    BadTimestamp { path: String, raw: String },
    #[error("GVL declares gvlSpecificationVersion {got}, expected {expected}")]
    UnsupportedSpecVersion { got: u32, expected: u32 },
    /// One variant covers vendors, purposes, special purposes,
    /// features, and special features. `table` names the GVL table
    /// (`"vendors"`, `"purposes"`, …).
    #[error("GVL table {table:?} map key {map_key:?} embeds id {embedded_id}")]
    IdKeyMismatch {
        table: &'static str,
        map_key: String,
        embedded_id: u64,
    },
    /// TCF IDs are positive integers per the technical spec; parsing a
    /// `u32` accepts `0` on the serde side, so validation rejects it
    /// explicitly.
    #[error("GVL table {table:?} map key {map_key:?} is zero (TCF ids are positive)")]
    ZeroId {
        table: &'static str,
        map_key: String,
    },
    #[error("GVL vendor id {vendor_id} exceeds the u16 vendor-id space")]
    VendorIdOutOfRange { vendor_id: u64 },
    #[error("GVL purpose id {purpose_id} exceeds the 24-position wire cap")]
    PurposeIdOutOfRange { purpose_id: u64 },
    #[error("GVL special-feature id {special_feature_id} exceeds the 12-position wire cap")]
    SpecialFeatureIdOutOfRange { special_feature_id: u64 },
    #[error("GVL vendor {vendor_id} references unknown id {purpose_id} in {field:?}")]
    UnknownPurposeReference {
        vendor_id: u32,
        purpose_id: u32,
        field: &'static str,
    },
    /// A purpose id appears in BOTH `purposes` and `legIntPurposes` (a
    /// purpose has exactly one default basis).
    #[error(
        "GVL vendor {vendor_id} declares purpose {purpose_id} under both consent and LI defaults"
    )]
    FlexBasisOverlap { vendor_id: u32, purpose_id: u8 },
    /// A purpose id in `flexiblePurposes` has no default basis in
    /// either `purposes` or `legIntPurposes`.
    #[error("GVL vendor {vendor_id} flexible purpose {purpose_id} has no default basis")]
    FlexPurposeWithoutDefault { vendor_id: u32, purpose_id: u8 },
    /// Purpose 1 (storage/access) is consent-only and cannot be listed
    /// in `flexiblePurposes`.
    #[error("GVL vendor {vendor_id} lists consent-only purpose 1 as flexible")]
    FlexiblePurposeOneForbidden { vendor_id: u32 },
}

/// Deserialised (not yet validated) GVL blob. Raw parse uses `u32`
/// widths; domain lowering happens inside `validate_gvl_json` — `u8`
/// map keys directly on this DTO would make serde reject out-of-range
/// ids before validation could run, producing a generic serde error
/// instead of the precise `PurposeIdOutOfRange` /
/// `SpecialFeatureIdOutOfRange` diagnostics the invariant list
/// promises.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawGvl {
    pub gvl_specification_version: u32,
    pub tcf_policy_version: u32,
    pub vendor_list_version: u32,
    pub last_updated: String,
    pub purposes: BTreeMap<u32, RawPurpose>,
    pub special_purposes: BTreeMap<u32, RawSpecialPurpose>,
    pub features: BTreeMap<u32, RawFeature>,
    pub special_features: BTreeMap<u32, RawSpecialFeature>,
    // Plain deserialize — the Value-walk in `validate_gvl_json` already
    // enforces map-key == embedded `id` for EVERY table, so `vendors`
    // needs no special deserializer.
    pub vendors: BTreeMap<u32, RawVendor>,
}

/// Post-validation shape. Only the wire-capped tables get lowered
/// widths (purposes ≤ 24, special features ≤ 12 → `u8`); special
/// purposes and features stay `u32` because they have no wire cap and
/// can hold arbitrary positive integers from the GVL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGvl {
    pub gvl_specification_version: u32,
    pub tcf_policy_version: u32,
    pub vendor_list_version: u32,
    /// Parsed at validation (the raw DTO keeps the `String`) —
    /// malformed → `GvlSchemaError::BadTimestamp { path: "lastUpdated" }`.
    /// The flex-invariant tombstone exemption compares vendor
    /// `deleted_date` against THIS instant.
    pub last_updated: OffsetDateTime,
    pub purposes: BTreeMap<u8, RawPurpose>,
    pub special_purposes: BTreeMap<u32, RawSpecialPurpose>,
    pub features: BTreeMap<u32, RawFeature>,
    pub special_features: BTreeMap<u8, RawSpecialFeature>,
    pub vendors: BTreeMap<u16, ValidatedVendor>,
}

/// `RawVendor` after validation — the only field that changes shape is
/// `deleted_date` (parsed RFC 3339; a malformed value fails the
/// validator, build-gate and runtime alike). The runtime lowering
/// consumes this type, so the runtime `Vendor` needs no second parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedVendor {
    pub id: u32,
    pub name: String,
    pub purposes: Vec<u32>,
    pub leg_int_purposes: Vec<u32>,
    pub flexible_purposes: Vec<u32>,
    pub special_purposes: Vec<u32>,
    pub features: Vec<u32>,
    pub special_features: Vec<u32>,
    pub deleted_date: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPurpose {
    pub id: u32,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSpecialPurpose {
    pub id: u32,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawFeature {
    pub id: u32,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSpecialFeature {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawVendor {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub purposes: Vec<u32>,
    #[serde(default)]
    pub leg_int_purposes: Vec<u32>,
    #[serde(default)]
    pub flexible_purposes: Vec<u32>,
    #[serde(default)]
    pub special_purposes: Vec<u32>,
    #[serde(default)]
    pub features: Vec<u32>,
    #[serde(default)]
    pub special_features: Vec<u32>,
    /// Present when the vendor was removed from the current GVL
    /// revision. A deleted vendor is still looked up by id (the GVL map
    /// retains the record) but is excluded from authorization once the
    /// deletion instant has passed. Validation parses this raw string
    /// (RFC 3339; malformed → `BadTimestamp`) onto
    /// `ValidatedVendor::deleted_date`, and the flex invariants below
    /// tolerate legacy inconsistencies on TOMBSTONE rows only
    /// (`deleted_date <= lastUpdated`) — the official v2 list ships
    /// deleted records the current schema forbids.
    #[serde(default)]
    pub deleted_date: Option<String>,
}

/// The nine required top-level GVL fields, checked before typed
/// deserialization so each absence maps to the exact
/// `MissingRequiredField` name rather than an opaque serde error.
const REQUIRED_TOP_LEVEL_FIELDS: [&str; 9] = [
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

/// The five indexed tables: (JSON key, crate-side table name, per-key
/// inclusive cap where the wire format imposes one).
const INDEXED_TABLES: [(&str, &str, Option<u64>); 5] = [
    ("purposes", "purposes", Some(24)),
    ("specialPurposes", "special_purposes", None),
    ("features", "features", None),
    ("specialFeatures", "special_features", Some(12)),
    ("vendors", "vendors", Some(0xFFFF)),
];

fn table_for_path(path: &str) -> &'static str {
    let root = path.split('.').next().unwrap_or("");
    match root {
        "purposes" => "purposes",
        "specialPurposes" => "special_purposes",
        "features" => "features",
        "specialFeatures" => "special_features",
        "vendors" => "vendors",
        _ => "gvl",
    }
}

/// Walk one indexed table at the `Value` level, enforcing key/id
/// equality, positivity, and the wire-position caps BEFORE the derived
/// deserializer runs — so each rejection carries its typed variant
/// instead of a generic serde message. Entries whose key or embedded
/// `id` cannot be read numerically are left for the typed pass, which
/// reports them as `MalformedEntry` with the exact dotted path.
fn walk_table(
    root: &serde_json::Map<String, Value>,
    json_key: &str,
    table: &'static str,
    key_cap: Option<u64>,
) -> Result<(), GvlSchemaError> {
    let Some(entries) = root.get(json_key).and_then(Value::as_object) else {
        return Ok(());
    };
    for (map_key, entry) in entries {
        let Ok(key_num) = map_key.parse::<u64>() else {
            continue;
        };
        let Some(embedded_id) = entry.get("id").and_then(Value::as_u64) else {
            continue;
        };
        if embedded_id != key_num {
            return Err(GvlSchemaError::IdKeyMismatch {
                table,
                map_key: map_key.clone(),
                embedded_id,
            });
        }
        if key_num == 0 {
            return Err(GvlSchemaError::ZeroId {
                table,
                map_key: map_key.clone(),
            });
        }
        if let Some(cap) = key_cap
            && key_num > cap
        {
            return Err(match table {
                "purposes" => GvlSchemaError::PurposeIdOutOfRange {
                    purpose_id: key_num,
                },
                "special_features" => GvlSchemaError::SpecialFeatureIdOutOfRange {
                    special_feature_id: key_num,
                },
                _ => GvlSchemaError::VendorIdOutOfRange { vendor_id: key_num },
            });
        }
    }
    Ok(())
}

/// Cross-reference every id a vendor declares against the table that
/// must define it.
fn check_vendor_references(vendor: &RawVendor, raw: &RawGvl) -> Result<(), GvlSchemaError> {
    let normal_purpose_lists: [(&'static str, &Vec<u32>); 3] = [
        ("purposes", &vendor.purposes),
        ("legIntPurposes", &vendor.leg_int_purposes),
        ("flexiblePurposes", &vendor.flexible_purposes),
    ];
    for (field, list) in normal_purpose_lists {
        for &purpose_id in list {
            if !raw.purposes.contains_key(&purpose_id) {
                return Err(GvlSchemaError::UnknownPurposeReference {
                    vendor_id: vendor.id,
                    purpose_id,
                    field,
                });
            }
        }
    }
    for &special_purpose_id in &vendor.special_purposes {
        if !raw.special_purposes.contains_key(&special_purpose_id) {
            return Err(GvlSchemaError::UnknownPurposeReference {
                vendor_id: vendor.id,
                purpose_id: special_purpose_id,
                field: "specialPurposes",
            });
        }
    }
    for &feature_id in &vendor.features {
        if !raw.features.contains_key(&feature_id) {
            return Err(GvlSchemaError::UnknownPurposeReference {
                vendor_id: vendor.id,
                purpose_id: feature_id,
                field: "features",
            });
        }
    }
    for &special_feature_id in &vendor.special_features {
        if !raw.special_features.contains_key(&special_feature_id) {
            return Err(GvlSchemaError::UnknownPurposeReference {
                vendor_id: vendor.id,
                purpose_id: special_feature_id,
                field: "specialFeatures",
            });
        }
    }
    Ok(())
}

/// Flex-basis invariants per the IAB vendor-list contract — required so
/// the "default basis" the enforcement matrix relies on is well-defined.
/// Applies to ACTIVE vendors only; the caller has already decided
/// tombstone status from the parsed timestamps.
fn check_vendor_flex_invariants(vendor: &RawVendor) -> Result<(), GvlSchemaError> {
    for &purpose_id in &vendor.purposes {
        if vendor.leg_int_purposes.contains(&purpose_id) {
            return Err(GvlSchemaError::FlexBasisOverlap {
                vendor_id: vendor.id,
                // Cross-reference validation already ran, so every id
                // here exists in the ≤ 24-key purposes table.
                purpose_id: u8::try_from(purpose_id).unwrap_or(u8::MAX),
            });
        }
    }
    for &purpose_id in &vendor.flexible_purposes {
        if !vendor.purposes.contains(&purpose_id) && !vendor.leg_int_purposes.contains(&purpose_id)
        {
            return Err(GvlSchemaError::FlexPurposeWithoutDefault {
                vendor_id: vendor.id,
                purpose_id: u8::try_from(purpose_id).unwrap_or(u8::MAX),
            });
        }
    }
    if vendor.flexible_purposes.contains(&1) {
        return Err(GvlSchemaError::FlexiblePurposeOneForbidden {
            vendor_id: vendor.id,
        });
    }
    Ok(())
}

/// Validate a GVL blob and lower it into the shared [`ValidatedGvl`]
/// DTO.
///
/// `spec_expected`:
/// - `Some(n)` — spec-checked gate (build AND runtime): the caller
///   knows which manifest slot the blob was fetched into, and a blob
///   whose `gvlSpecificationVersion != n` is rejected with
///   `UnsupportedSpecVersion`.
/// - `None` — self-describing load, test fixtures only: skips ONLY the
///   spec cross-check; every other schema / hostile-input invariant
///   still runs.
///
/// Two-stage parse: a direct typed deserialize would give up on the
/// first error with no field-path context, so the validator parses into
/// `serde_json::Value` first (invalid JSON → `InvalidJson`), checks the
/// required top-level fields and per-table key/id invariants on the
/// `Value`, and only then runs the typed deserializer through
/// `serde_path_to_error` so residual shape errors become
/// `MalformedEntry { table, path, reason }`.
///
/// # Errors
///
/// Returns the [`GvlSchemaError`] variant matching the first violated
/// invariant (see the variant docs for the full taxonomy).
pub fn validate_gvl_json(
    raw: &str,
    spec_expected: Option<u32>,
) -> Result<ValidatedGvl, GvlSchemaError> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|parse_err| GvlSchemaError::InvalidJson(parse_err.to_string()))?;
    let root = value
        .as_object()
        .ok_or_else(|| GvlSchemaError::InvalidJson("top level is not a JSON object".to_owned()))?;

    for field in REQUIRED_TOP_LEVEL_FIELDS {
        if !root.contains_key(field) {
            return Err(GvlSchemaError::MissingRequiredField(field));
        }
    }

    if let Some(expected) = spec_expected
        && let Some(got) = root.get("gvlSpecificationVersion").and_then(Value::as_u64)
    {
        let got_u32 = u32::try_from(got).unwrap_or(u32::MAX);
        if got_u32 != expected {
            return Err(GvlSchemaError::UnsupportedSpecVersion {
                got: got_u32,
                expected,
            });
        }
    }

    for (json_key, table, key_cap) in INDEXED_TABLES {
        walk_table(root, json_key, table, key_cap)?;
    }

    let raw_gvl: RawGvl = serde_path_to_error::deserialize(&value).map_err(|typed_err| {
        let path = typed_err.path().to_string();
        GvlSchemaError::MalformedEntry {
            table: table_for_path(&path),
            path,
            reason: typed_err.inner().to_string(),
        }
    })?;

    let last_updated =
        OffsetDateTime::parse(&raw_gvl.last_updated, &Rfc3339).map_err(|_parse_err| {
            GvlSchemaError::BadTimestamp {
                path: "lastUpdated".to_owned(),
                raw: raw_gvl.last_updated.clone(),
            }
        })?;

    let mut vendors: BTreeMap<u16, ValidatedVendor> = BTreeMap::new();
    for vendor in raw_gvl.vendors.values() {
        check_vendor_references(vendor, &raw_gvl)?;
        let deleted_date = match vendor.deleted_date.as_ref() {
            None => None,
            Some(raw_ts) => Some(OffsetDateTime::parse(raw_ts, &Rfc3339).map_err(
                |_parse_err| GvlSchemaError::BadTimestamp {
                    path: format!("vendors.{}.deletedDate", vendor.id),
                    raw: raw_ts.clone(),
                },
            )?),
        };
        // "Active" is timestamp-defined, not presence-defined: a vendor
        // is a TOMBSTONE for flex-invariant purposes iff its deletion
        // instant is at or before the list's own `lastUpdated` (the
        // validator has no request clock; the list's instant is the
        // deterministic, content-derived reference). Only tombstones
        // skip the flex invariants — they are excluded from
        // authorization anyway, so tolerating their legacy flex shape
        // changes no grant decision. A future-dated vendor can still be
        // authorized, so its flex defaults must be well-defined.
        let is_tombstone = deleted_date.is_some_and(|instant| instant <= last_updated);
        if !is_tombstone {
            check_vendor_flex_invariants(vendor)?;
        }
        // The walk capped vendor keys at 65_535 and key == embedded id.
        let Ok(vendor_key) = u16::try_from(vendor.id) else {
            continue;
        };
        vendors.insert(
            vendor_key,
            ValidatedVendor {
                id: vendor.id,
                name: vendor.name.clone(),
                purposes: vendor.purposes.clone(),
                leg_int_purposes: vendor.leg_int_purposes.clone(),
                flexible_purposes: vendor.flexible_purposes.clone(),
                special_purposes: vendor.special_purposes.clone(),
                features: vendor.features.clone(),
                special_features: vendor.special_features.clone(),
                deleted_date,
            },
        );
    }

    // Infallible narrowing: the walk enforced the wire caps, so ids
    // outside them cannot reach the lowering.
    let purposes: BTreeMap<u8, RawPurpose> = raw_gvl
        .purposes
        .into_iter()
        .filter_map(|(id, purpose)| u8::try_from(id).ok().map(|narrow| (narrow, purpose)))
        .collect();
    let special_features: BTreeMap<u8, RawSpecialFeature> = raw_gvl
        .special_features
        .into_iter()
        .filter_map(|(id, feature)| u8::try_from(id).ok().map(|narrow| (narrow, feature)))
        .collect();

    Ok(ValidatedGvl {
        gvl_specification_version: raw_gvl.gvl_specification_version,
        tcf_policy_version: raw_gvl.tcf_policy_version,
        vendor_list_version: raw_gvl.vendor_list_version,
        last_updated,
        purposes,
        special_purposes: raw_gvl.special_purposes,
        features: raw_gvl.features,
        special_features,
        vendors,
    })
}

// ───────────────────────── Global CMP List ─────────────────────────

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum GclSchemaError {
    #[error("GCL is not valid JSON or violates a raw bound: {0}")]
    InvalidJson(String),
    #[error("GCL entry at {path:?} is malformed")]
    MalformedEntry { path: String },
    #[error("GCL root lastUpdated is required")]
    MissingLastUpdated,
    #[error("GCL timestamp at {path:?} is not RFC 3339: {raw:?}")]
    BadTimestamp { path: String, raw: String },
    #[error("GCL cmps map key {key:?} is not numeric")]
    NonNumericMapKey { key: String },
    #[error("GCL cmps map key {map_key:?} embeds id {embedded_id}")]
    IdKeyMismatch { map_key: String, embedded_id: u32 },
    /// `CmpId` is a 12-bit NONZERO domain: 1..=4095 (matches the TC
    /// string's 12-bit `CmpId` field).
    #[error("GCL cmp id {id} is outside the 12-bit nonzero domain 1..=4095")]
    CmpIdOutOfRange { id: u32 },
}

/// Serde side of the GCL. Root `lastUpdated` is `Option` so its
/// absence maps to the typed `MissingLastUpdated` (a required `String`
/// would surface as an opaque serde error instead). Unknown fields are
/// tolerated everywhere — no `deny_unknown_fields`.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGcl {
    #[serde(default)]
    last_updated: Option<String>,
    cmps: BTreeMap<String, RawCmp>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCmp {
    id: u32,
    #[serde(default)]
    deleted_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGcl {
    pub last_updated: OffsetDateTime,
    pub cmps: BTreeMap<u16, ValidatedCmp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCmp {
    pub id: u16,
    pub deleted_date: Option<OffsetDateTime>,
}

/// Validate the Global CMP List blob. Invariants: the raw size is
/// bounded by [`GCL_MAX_RAW_BYTES`]; root `lastUpdated` is required and
/// RFC 3339; every `cmps` map key parses numerically AND equals its
/// entry's embedded `id`; ids ∈ 1..=4095; `deletedDate` parsed when
/// present; unknown fields tolerated.
///
/// # Errors
///
/// Returns the [`GclSchemaError`] variant matching the first violated
/// invariant.
pub fn validate_gcl_json(raw: &str) -> Result<ValidatedGcl, GclSchemaError> {
    if raw.len() > GCL_MAX_RAW_BYTES {
        return Err(GclSchemaError::InvalidJson(format!(
            "GCL raw size {} exceeds GCL_MAX_RAW_BYTES ({GCL_MAX_RAW_BYTES})",
            raw.len()
        )));
    }
    let value: Value = serde_json::from_str(raw)
        .map_err(|parse_err| GclSchemaError::InvalidJson(parse_err.to_string()))?;
    let raw_gcl: RawGcl = serde_path_to_error::deserialize(&value).map_err(|typed_err| {
        GclSchemaError::MalformedEntry {
            path: typed_err.path().to_string(),
        }
    })?;
    let raw_last_updated = raw_gcl
        .last_updated
        .ok_or(GclSchemaError::MissingLastUpdated)?;
    let last_updated =
        OffsetDateTime::parse(&raw_last_updated, &Rfc3339).map_err(|_parse_err| {
            GclSchemaError::BadTimestamp {
                path: "lastUpdated".to_owned(),
                raw: raw_last_updated.clone(),
            }
        })?;
    let mut cmps: BTreeMap<u16, ValidatedCmp> = BTreeMap::new();
    for (map_key, cmp) in &raw_gcl.cmps {
        let Ok(key_num) = map_key.parse::<u64>() else {
            return Err(GclSchemaError::NonNumericMapKey {
                key: map_key.clone(),
            });
        };
        if key_num != u64::from(cmp.id) {
            return Err(GclSchemaError::IdKeyMismatch {
                map_key: map_key.clone(),
                embedded_id: cmp.id,
            });
        }
        if !(1..=4095).contains(&cmp.id) {
            return Err(GclSchemaError::CmpIdOutOfRange { id: cmp.id });
        }
        let deleted_date = match cmp.deleted_date.as_ref() {
            None => None,
            Some(raw_ts) => Some(OffsetDateTime::parse(raw_ts, &Rfc3339).map_err(
                |_parse_err| GclSchemaError::BadTimestamp {
                    path: format!("cmps.{map_key}.deletedDate"),
                    raw: raw_ts.clone(),
                },
            )?),
        };
        // Infallible: 1..=4095 fits u16.
        let Ok(cmp_id) = u16::try_from(cmp.id) else {
            continue;
        };
        cmps.insert(
            cmp_id,
            ValidatedCmp {
                id: cmp_id,
                deleted_date,
            },
        );
    }
    Ok(ValidatedGcl { last_updated, cmps })
}

// ───────────────────────── Archive manifest ─────────────────────────

/// The VALIDATED manifest — built by hand inside [`validate_manifest`],
/// never deserialised directly (the internal `Raw*` structs own the
/// serde derives).
#[derive(Clone, Debug)]
pub struct Manifest {
    pub manifest_version: u32,
    pub generated_at_rfc3339: String,
    pub generated_at: OffsetDateTime,
    pub retention: Retention,
    /// Records that the operator accepted spec-3 entries older than the
    /// default retention window. Relaxes the refresh tool's AGE filter
    /// only — never the raw-bundle cap.
    pub oversize_bundle_ack: bool,
    /// The REQUIRED Global CMP List record — absent →
    /// `ManifestError::MissingGcl`; freshness `now >= expires_at()` →
    /// `ManifestError::GclStale` (the GCL's own RFC 9111 lifetime,
    /// never the GVL manifest-age window, and never overrideable).
    pub gcl: GclRecord,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Clone, Debug)]
pub struct GclRecord {
    /// Constrained to exactly `gcl.json`.
    pub path: String,
    pub sha256: String,
    /// Upstream `lastUpdated`.
    pub last_updated: OffsetDateTime,
    /// Tool fetch instant.
    pub fetched_at: OffsetDateTime,
    /// RFC 9111-derived lifetime the refresh tool recorded
    /// (Age/Date-corrected; no floor applied).
    pub cache_max_age_secs: u64,
}

impl GclRecord {
    /// Checked addition; overflow → `GclExpiryOverflow`.
    ///
    /// # Errors
    ///
    /// [`ManifestError::GclExpiryOverflow`] when
    /// `fetched_at + cache_max_age_secs` leaves the representable
    /// timestamp range.
    pub fn expires_at(&self) -> Result<OffsetDateTime, ManifestError> {
        i64::try_from(self.cache_max_age_secs)
            .ok()
            .and_then(|secs| self.fetched_at.checked_add(Duration::seconds(secs)))
            .ok_or(ManifestError::GclExpiryOverflow)
    }
}

#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct Retention {
    pub max_age_days: u32,
}

#[derive(Clone, Debug)]
pub struct ManifestEntry {
    pub spec: u32,
    pub vendor_list_version: u32,
    pub tcf_policy_version: u32,
    pub last_updated_rfc3339: String,
    pub last_updated: OffsetDateTime,
    pub path: String,
    pub sha256: String,
    /// Recorded on the CURRENT-list entry so refresh runs can reuse it
    /// within its advertised max-age; `None` on archived entries.
    pub fetched_at: Option<OffsetDateTime>,
    pub cache_max_age_secs: Option<u64>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest is not valid JSON: {0}")]
    Json(String),
    #[error("unsupported manifest_version {got} (expected 1)")]
    UnsupportedVersion { got: u32 },
    #[error("manifest field {field} is not RFC 3339: {raw:?}")]
    BadTimestamp { field: &'static str, raw: String },
    #[error("manifest generated_at {generated_at} is in the future (now {now})")]
    FutureGeneratedAt { generated_at: String, now: String },
    #[error("manifest field {field} is in the future: {raw:?}")]
    FutureTimestamp { field: &'static str, raw: String },
    #[error("manifest contains duplicate (spec {spec}, list {list}) entries")]
    DuplicatePair { spec: u32, list: u32 },
    #[error("manifest entry spec {spec} is not 2 or 3")]
    OutOfRangeSpec { spec: u32 },
    #[error("manifest entry vendor_list_version must be >= 1")]
    ZeroListVersion,
    #[error("manifest entry path {path:?} rejected: {reason}")]
    BadPath { path: String, reason: &'static str },
    #[error("manifest path {path:?} version digits disagree with declared version {declared}")]
    PathVersionMismatch { path: String, declared: u32 },
    #[error("{path}: sha256 must be 64 lowercase hex chars, got {got:?}")]
    Sha256Format { path: String, got: String },
    #[error("manifest entry {path:?} is {age_days} days old (retention cap {retention_days})")]
    RetentionExceeded {
        path: String,
        age_days: i64,
        retention_days: u32,
    },
    #[error("manifest entries are not in (spec asc, vendor_list_version desc) order")]
    WrongEntryOrder,
    #[error(
        "manifest must cover spec 2 and spec 3 (has_spec_2: {has_spec_2}, has_spec_3: {has_spec_3})"
    )]
    MissingSpecCoverage { has_spec_2: bool, has_spec_3: bool },
    #[error("manifest is missing the required gcl record")]
    MissingGcl,
    #[error(
        "baked GCL is stale: expired {expires_at} (now {now}); re-run: cargo run --locked -p auctioneer-cli --no-default-features -- gvl refresh"
    )]
    GclStale { expires_at: String, now: String },
    #[error("gcl fetched_at + cache_max_age_secs overflows the timestamp range")]
    GclExpiryOverflow,
    #[error("manifest gcl.last_updated {manifest} disagrees with the blob's lastUpdated {blob}")]
    GclLastUpdatedMismatch { manifest: String, blob: String },
    /// Per-ENTRY cache-metadata overflow — a GVL-entry defect, not a
    /// GCL one.
    #[error("manifest entry {path:?} cache metadata overflows the timestamp range")]
    EntryExpiryOverflow { path: String },
}

/// Shared RFC 3339 parse used by both the manifest validator and the
/// GCL cross-checks — one definition.
fn parse_rfc3339(field: &'static str, raw: &str) -> Result<OffsetDateTime, ManifestError> {
    OffsetDateTime::parse(raw, &Rfc3339).map_err(|_parse_err| ManifestError::BadTimestamp {
        field,
        raw: raw.to_owned(),
    })
}

fn check_sha256_format(candidate: String) -> Result<String, ManifestError> {
    let well_formed = candidate.len() == 64
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());
    if well_formed {
        Ok(candidate)
    } else {
        Err(ManifestError::Sha256Format {
            path: String::new(),
            got: candidate,
        })
    }
}

/// The manifest/blob cross-check. Called by `build.rs` AND the refresh
/// tool's pre-swap validation after both `validate_manifest` and
/// `validate_gcl_json` succeed — the manifest may not disagree with the
/// blob it points at.
///
/// # Errors
///
/// [`ManifestError::GclLastUpdatedMismatch`] when the manifest's
/// recorded `gcl.last_updated` disagrees with the validated blob.
pub fn check_gcl_consistency(manifest: &Manifest, gcl: &ValidatedGcl) -> Result<(), ManifestError> {
    if manifest.gcl.last_updated == gcl.last_updated {
        Ok(())
    } else {
        Err(ManifestError::GclLastUpdatedMismatch {
            manifest: manifest.gcl.last_updated.to_string(),
            blob: gcl.last_updated.to_string(),
        })
    }
}

/// Whole days elapsed from `then` to `now` (negative when `then` is in
/// the future). Unix-timestamp subtraction is checked; the difference
/// of two in-range instants cannot overflow i64 seconds, so the
/// fallback is unreachable and conservative.
fn days_between(now: OffsetDateTime, then: OffsetDateTime) -> i64 {
    now.unix_timestamp()
        .checked_sub(then.unix_timestamp())
        .map_or(i64::MAX, |secs| Duration::seconds(secs).whole_days())
}

/// Validate an archive `manifest.json` against the full invariant set:
/// version, RFC 3339 timestamps (none in the future), entry dedupe and
/// ordering, path safety, sha256 format, spec coverage, retention, the
/// optional per-entry cache metadata, and the required-and-fresh GCL
/// record.
///
/// # Errors
///
/// Returns the [`ManifestError`] variant matching the first violated
/// invariant.
#[expect(
    clippy::too_many_lines,
    reason = "one linear pass over the manifest invariants; splitting it would \
              scatter the validation order this module exists to pin down"
)]
pub fn validate_manifest(raw: &str, now: OffsetDateTime) -> Result<Manifest, ManifestError> {
    #[derive(serde::Deserialize)]
    struct Raw {
        manifest_version: u32,
        generated_at: String,
        retention: Retention,
        /// `Option` + `default` so manifests written before the ack
        /// field existed still parse (absent → false).
        #[serde(default)]
        oversize_bundle_ack: Option<bool>,
        /// `Option` so the ABSENT case maps to the typed
        /// `ManifestError::MissingGcl`, not a serde error.
        #[serde(default)]
        gcl: Option<RawGclRecord>,
        entries: Vec<RawEntry>,
    }
    #[derive(serde::Deserialize)]
    struct RawGclRecord {
        path: String,
        sha256: String,
        last_updated: String,
        fetched_at: String,
        cache_max_age_secs: u64,
    }
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    struct RawEntry {
        spec: u32,
        vendor_list_version: u32,
        tcf_policy_version: u32,
        last_updated: String,
        path: String,
        sha256: String,
        #[serde(default)]
        fetched_at: Option<String>,
        #[serde(default)]
        cache_max_age_secs: Option<u64>,
    }
    let manifest_raw: Raw = serde_json::from_str(raw)
        .map_err(|parse_err| ManifestError::Json(parse_err.to_string()))?;
    if manifest_raw.manifest_version != 1 {
        return Err(ManifestError::UnsupportedVersion {
            got: manifest_raw.manifest_version,
        });
    }
    let generated_at = parse_rfc3339("generated_at", &manifest_raw.generated_at)?;
    // Reject future dates (clock skew / manual manifest edit).
    if generated_at > now {
        return Err(ManifestError::FutureGeneratedAt {
            generated_at: manifest_raw.generated_at.clone(),
            now: now.format(&Rfc3339).unwrap_or_default(),
        });
    }
    let mut seen = BTreeSet::new();
    let mut entries = Vec::with_capacity(manifest_raw.entries.len());
    for entry in manifest_raw.entries {
        if !(entry.spec == 2 || entry.spec == 3) {
            return Err(ManifestError::OutOfRangeSpec { spec: entry.spec });
        }
        if entry.vendor_list_version < 1 {
            return Err(ManifestError::ZeroListVersion);
        }
        if !seen.insert((entry.spec, entry.vendor_list_version)) {
            return Err(ManifestError::DuplicatePair {
                spec: entry.spec,
                list: entry.vendor_list_version,
            });
        }
        let expected_prefix = format!("spec{}/", entry.spec);
        if !entry.path.starts_with(&expected_prefix) {
            return Err(ManifestError::BadPath {
                path: entry.path,
                reason: "missing spec{2,3}/ prefix",
            });
        }
        if entry.path.contains("..") || entry.path.starts_with('/') {
            return Err(ManifestError::BadPath {
                path: entry.path,
                reason: "contains .. or starts with /",
            });
        }
        // Enforce `^spec[23]/vendor-list-v\d+\.json$` — `\d+` requires
        // at least one digit (a vacuous `.all()` on an empty digit run
        // would accept `vendor-list-v.json`), and the digit run is
        // cross-checked against the declared version so a renamed file
        // cannot silently shadow another version.
        let stem = entry.path.strip_prefix(&expected_prefix).unwrap_or("");
        let digits = stem
            .strip_prefix("vendor-list-v")
            .and_then(|rest| rest.strip_suffix(".json"))
            .filter(|run| !run.is_empty() && run.chars().all(|ch| ch.is_ascii_digit()));
        let Some(version_digits) = digits else {
            return Err(ManifestError::BadPath {
                path: entry.path,
                reason: "does not match spec[23]/vendor-list-v<N>.json",
            });
        };
        if version_digits.parse::<u32>().ok() != Some(entry.vendor_list_version) {
            return Err(ManifestError::PathVersionMismatch {
                path: entry.path.clone(),
                declared: entry.vendor_list_version,
            });
        }
        // SHA-256 must be exactly 64 lowercase hex chars — a length
        // check alone would accept 64 `g`s.
        if entry.sha256.len() != 64
            || !entry
                .sha256
                .chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
        {
            return Err(ManifestError::Sha256Format {
                path: entry.path.clone(),
                got: entry.sha256.clone(),
            });
        }
        let last_updated = parse_rfc3339("last_updated", &entry.last_updated)?;
        if last_updated > now {
            return Err(ManifestError::FutureTimestamp {
                field: "last_updated",
                raw: entry.last_updated.clone(),
            });
        }
        // Retention enforcement is executable, not just documented. The
        // frozen spec-2 list (final TCF v2.2 GVL, lastUpdated
        // 2023-11-16) is AGE-EXEMPT: it is older than any retention
        // window by design, yet mandatory spec-2 coverage requires it,
        // so the age gate applies to spec-3 entries only.
        let age = days_between(now, last_updated);
        if entry.spec != 2 && age > i64::from(manifest_raw.retention.max_age_days) {
            return Err(ManifestError::RetentionExceeded {
                path: entry.path.clone(),
                age_days: age,
                retention_days: manifest_raw.retention.max_age_days,
            });
        }
        // The optional cache metadata is validated AND initialized —
        // the fields come in pairs, fetched_at parses RFC 3339 and may
        // not be in the future, and the pair's checked expiry addition
        // is exercised so overflow fails here, not at reuse time.
        let fetched_at = match (&entry.fetched_at, entry.cache_max_age_secs) {
            (None, None) => None,
            (Some(raw_ts), Some(secs)) => {
                let parsed_ts = parse_rfc3339("entry.fetched_at", raw_ts)?;
                if parsed_ts > now {
                    return Err(ManifestError::FutureTimestamp {
                        field: "entry.fetched_at",
                        raw: raw_ts.clone(),
                    });
                }
                i64::try_from(secs)
                    .ok()
                    .and_then(|secs_i64| parsed_ts.checked_add(Duration::seconds(secs_i64)))
                    .ok_or(ManifestError::EntryExpiryOverflow {
                        path: entry.path.clone(),
                    })?;
                Some(parsed_ts)
            }
            (None, Some(_)) | (Some(_), None) => {
                return Err(ManifestError::BadTimestamp {
                    field: "entry.fetched_at/cache_max_age_secs",
                    raw: "fields must be present together".into(),
                });
            }
        };
        entries.push(ManifestEntry {
            spec: entry.spec,
            vendor_list_version: entry.vendor_list_version,
            tcf_policy_version: entry.tcf_policy_version,
            last_updated_rfc3339: entry.last_updated,
            last_updated,
            path: entry.path,
            sha256: entry.sha256,
            fetched_at,
            cache_max_age_secs: entry.cache_max_age_secs,
        });
    }
    // Manifest ordering must ALREADY be (spec asc, vendor_list_version
    // desc) — the refresh tool writes it that way. Silently sorting
    // would hide deployer edits that reordered entries: validate order,
    // error on mismatch, never sort in place.
    let expected_order = {
        let mut sorted = entries.clone();
        sorted.sort_by(|left, right| {
            left.spec
                .cmp(&right.spec)
                .then_with(|| right.vendor_list_version.cmp(&left.vendor_list_version))
        });
        sorted
    };
    if entries
        .iter()
        .zip(&expected_order)
        .any(|(actual, expected)| {
            (actual.spec, actual.vendor_list_version)
                != (expected.spec, expected.vendor_list_version)
        })
    {
        return Err(ManifestError::WrongEntryOrder);
    }
    // Spec-version coverage: production traffic mixes both spec-2 and
    // spec-3 TC strings, so a single-spec archive is a deployment bug.
    let has_spec_2 = entries.iter().any(|entry| entry.spec == 2);
    let has_spec_3 = entries.iter().any(|entry| entry.spec == 3);
    if !(has_spec_2 && has_spec_3) {
        return Err(ManifestError::MissingSpecCoverage {
            has_spec_2,
            has_spec_3,
        });
    }
    // GCL — required, path-constrained, timestamp-parsed,
    // freshness-enforced with the `>=` comparator used at both build
    // and runtime.
    let raw_gcl = manifest_raw.gcl.ok_or(ManifestError::MissingGcl)?;
    if raw_gcl.path != "gcl.json" {
        return Err(ManifestError::BadPath {
            path: raw_gcl.path,
            reason: "gcl path must be gcl.json",
        });
    }
    let gcl = GclRecord {
        last_updated: parse_rfc3339("gcl.last_updated", &raw_gcl.last_updated)?,
        fetched_at: parse_rfc3339("gcl.fetched_at", &raw_gcl.fetched_at)?,
        path: raw_gcl.path,
        sha256: check_sha256_format(raw_gcl.sha256)?,
        cache_max_age_secs: raw_gcl.cache_max_age_secs,
    };
    if gcl.fetched_at > now {
        return Err(ManifestError::FutureTimestamp {
            field: "gcl.fetched_at",
            raw: gcl.fetched_at.to_string(),
        });
    }
    let gcl_expires = gcl.expires_at()?;
    if now >= gcl_expires {
        return Err(ManifestError::GclStale {
            expires_at: gcl_expires.to_string(),
            now: now.to_string(),
        });
    }
    Ok(Manifest {
        manifest_version: manifest_raw.manifest_version,
        generated_at_rfc3339: manifest_raw.generated_at,
        generated_at,
        retention: manifest_raw.retention,
        oversize_bundle_ack: manifest_raw.oversize_bundle_ack.unwrap_or(false),
        gcl,
        entries,
    })
}

// ─────────────────────── Bundle-size decision ───────────────────────

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum BundleLimitError {
    #[error(
        "GVL raw-bundle cap {configured_cap} bytes exceeds the wasm hard maximum of \
         {WASM_BUNDLE_MAX_RAW_BYTES} bytes (24 MiB). The single committed archive feeds \
         all four adapters and must fit the wasm isolate budget; a raised native cap \
         only matters for a future per-target-artifact deployment. Lower \
         AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB."
    )]
    WasmCapAboveHardMax { configured_cap: u64 },
    #[error(
        "GVL raw bundle is {bundle_bytes} bytes, over the {configured_cap}-byte cap \
         (AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB). Re-run the refresh tool with a \
         smaller --max-age-days, or raise the cap where the platform allows."
    )]
    BundleOverCap {
        bundle_bytes: u64,
        configured_cap: u64,
    },
}

/// The PURE raw-cap decision shared by `build.rs` (which cannot host
/// unit tests) and this crate's own tests. Always enforced — the
/// oversize-bundle ack never skips it. No I/O, no env reads: the caller
/// supplies all three inputs, so every branch is unit-testable.
///
/// # Errors
///
/// [`BundleLimitError::WasmCapAboveHardMax`] for a wasm cap above the
/// 24 MiB platform maximum; [`BundleLimitError::BundleOverCap`] when
/// the bundle exceeds the configured cap.
pub fn validate_bundle_limits(
    target_arch: &str,
    configured_cap: u64,
    bundle_bytes: u64,
) -> Result<(), BundleLimitError> {
    if target_arch == "wasm32" && configured_cap > WASM_BUNDLE_MAX_RAW_BYTES {
        return Err(BundleLimitError::WasmCapAboveHardMax { configured_cap });
    }
    if bundle_bytes > configured_cap {
        return Err(BundleLimitError::BundleOverCap {
            bundle_bytes,
            configured_cap,
        });
    }
    Ok(())
}

// ───────────────────────── Retention decision ─────────────────────────

/// Metadata for one fetched GVL version, as the refresh tool's
/// candidate walk observes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionMeta {
    pub spec: u32,
    pub version: u32,
    pub last_updated: OffsetDateTime,
    pub raw_bytes: u64,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RetentionError {
    #[error(
        "retention budget underflow: growth margin {growth_margin} bytes exceeds the raw \
         cap {raw_cap} bytes (AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB)"
    )]
    BudgetUnderflow { raw_cap: u64, growth_margin: u64 },
    #[error(
        "the mandatory reserved pair alone is {reserved_bytes} bytes, over the \
         {budget}-byte size budget (raw cap minus growth margin; see \
         AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB). A mandatory version is never \
         silently dropped to fit."
    )]
    ReservedPairOverBudget { reserved_bytes: u64, budget: u64 },
}

/// The PURE retention decision behind the refresh tool's reserve-then-
/// fill: the reserved set is kept unconditionally, then age-filtered
/// candidates fill newest-first while the running raw total stays
/// within `raw_cap - growth_margin`. The subtraction happens HERE
/// (checked) so `BudgetUnderflow` is reachable and unit-testable; the
/// tool passes cap and margin through uncombined. Candidates older than
/// `cutoff` end the fill (versions are contiguous by recency), as does
/// the first candidate that would cross the budget — no
/// skip-and-continue.
///
/// # Errors
///
/// [`RetentionError::BudgetUnderflow`] when `growth_margin > raw_cap`;
/// [`RetentionError::ReservedPairOverBudget`] when the reserved set
/// alone exceeds the budget — a mandatory version is never silently
/// dropped to fit.
pub fn select_retained_versions(
    candidates: &[VersionMeta],
    reserved: &[VersionMeta],
    raw_cap: u64,
    growth_margin: u64,
    cutoff: OffsetDateTime,
) -> Result<Vec<VersionMeta>, RetentionError> {
    let budget = raw_cap
        .checked_sub(growth_margin)
        .ok_or(RetentionError::BudgetUnderflow {
            raw_cap,
            growth_margin,
        })?;
    let mut reserved_bytes: u64 = 0;
    for meta in reserved {
        // Saturating: an overflowing reserved total is over any budget.
        reserved_bytes = reserved_bytes.saturating_add(meta.raw_bytes);
    }
    if reserved_bytes > budget {
        return Err(RetentionError::ReservedPairOverBudget {
            reserved_bytes,
            budget,
        });
    }
    let mut newest_first: Vec<&VersionMeta> = candidates.iter().collect();
    newest_first.sort_by_key(|meta| Reverse(meta.last_updated));
    let mut keep: Vec<VersionMeta> = reserved.to_vec();
    let mut running_total = reserved_bytes;
    for meta in newest_first {
        if meta.last_updated < cutoff {
            break;
        }
        let Some(next_total) = running_total.checked_add(meta.raw_bytes) else {
            break;
        };
        if next_total > budget {
            break;
        }
        running_total = next_total;
        keep.push(meta.clone());
    }
    Ok(keep)
}

// ───────────────────────── Archive recovery ─────────────────────────

/// Heal a crashed or interrupted GVL-archive swap. The caller MUST
/// already hold the EXCLUSIVE `fs2` advisory lock on the archive lock
/// sentinel; this helper runs only the pure `std::fs` four-case
/// recovery decision, so it adds no `fs2` dependency and stays
/// wasm-clean. `build.rs` (its missing-`live` read branch) and the
/// refresh tool (pre-swap) both call this under the held exclusive
/// lock, then read/swap while STILL holding it — no unlocked window.
/// `archive_dir` is the live archive dir; the single fixed aside is its
/// `.old` sibling. Keyed only on which of the two directories exists:
/// - live present + `.old` present → crash AFTER the 2nd rename; the
///   new archive is already installed, so DELETE the stale `.old`.
/// - live ABSENT + `.old` present → crash BETWEEN the renames;
///   `rename(.old → live)`.
/// - live present + no `.old` → nothing to do.
/// - live ABSENT + no `.old` → first install / clean state; nothing to
///   recover (the caller's swap installs `tx → live` directly).
///
/// # Errors
///
/// Propagates the underlying `std::fs` error when a rename, removal,
/// or parent-directory fsync fails.
pub fn recover_gvl_archive(archive_dir: &Path) -> io::Result<()> {
    let mut old_name = archive_dir
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_default();
    old_name.push(".old");
    let old_dir = archive_dir.with_file_name(old_name);
    match (archive_dir.exists(), old_dir.exists()) {
        // Crash after the 2nd rename: new `live` already complete.
        (true, true) => fs::remove_dir_all(&old_dir),
        // Crash between the renames: restore the aside, then fsync the
        // parent so the restored dir entry is durable.
        (false, true) => {
            fs::rename(&old_dir, archive_dir)?;
            if let Some(parent) = archive_dir.parent() {
                fs::File::open(parent)?.sync_all()?;
            }
            Ok(())
        }
        // Steady state, or first-install/clean: nothing to do.
        (true | false, false) => Ok(()),
    }
}

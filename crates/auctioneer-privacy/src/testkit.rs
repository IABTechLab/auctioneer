//! Structural builders for the decision tests: a `TcfString` with
//! exactly the bits under test, and a GVL holding exactly the vendors
//! under test.
//!
//! Building the TC string as a struct rather than encoding one keeps a
//! legal-basis test readable — the assertion is about which bits are
//! set, and the base64 round trip has its own coverage in
//! [`crate::tcf`]. Tests that must exercise decoding read the committed
//! fixtures in `tests/data/tcf_v2` instead.

use time::OffsetDateTime;
use time::macros::datetime;

use crate::gvl::Gvl;
use crate::tcf::{PublisherRestriction, TcfString, VendorBitSet};

/// The GVL revision every builder here emits, and the TC policy version
/// that resolves to it.
pub const SPEC: u32 = 3;
pub const POLICY_VERSION: u8 = 5;
/// A CMP id the test GCLs register.
pub const CMP_ID: u16 = 300;
/// The authorization instant every test context reads.
///
/// A real instant, not the epoch: time-dependent authorization
/// (`Vendor::is_deleted_at`) compares against this field, and against
/// 1970 no vendor deletion has happened yet — every deletion assertion
/// would pass without the check being wired at all.
pub const AUTHORIZATION_NOW: OffsetDateTime = datetime!(2026-08-29 12:00:00 UTC);

pub struct TcfSpec {
    pub tcf_policy_version: u8,
    pub vendor_list_version: u16,
    pub cmp_id: u16,
    pub purposes_consent: Vec<u8>,
    pub purposes_li_transparency: Vec<u8>,
    pub vendor_consent: Vec<u16>,
    pub vendor_li: Vec<u16>,
    pub special_feature_optins: Vec<u8>,
    pub purpose_one_treatment: bool,
    pub publisher_restrictions: Vec<PublisherRestriction>,
}

impl Default for TcfSpec {
    fn default() -> Self {
        Self {
            tcf_policy_version: POLICY_VERSION,
            vendor_list_version: 42,
            cmp_id: CMP_ID,
            purposes_consent: Vec::new(),
            purposes_li_transparency: Vec::new(),
            vendor_consent: Vec::new(),
            vendor_li: Vec::new(),
            special_feature_optins: Vec::new(),
            purpose_one_treatment: false,
            publisher_restrictions: Vec::new(),
        }
    }
}

impl TcfSpec {
    #[must_use]
    pub fn build(self) -> TcfString {
        let mut vendor_consent = VendorBitSet::new();
        for id in self.vendor_consent {
            vendor_consent.set(id);
        }
        let mut vendor_li = VendorBitSet::new();
        for id in self.vendor_li {
            vendor_li.set(id);
        }
        TcfString {
            tcf_policy_version: self.tcf_policy_version,
            vendor_list_version: self.vendor_list_version,
            cmp_id: self.cmp_id,
            cmp_version: 2,
            created: datetime!(2026-02-01 00:00:00 UTC),
            last_updated: datetime!(2026-02-01 00:00:00 UTC),
            purposes_consent: self.purposes_consent.into_iter().collect(),
            purposes_li_transparency: self.purposes_li_transparency.into_iter().collect(),
            vendor_consent,
            vendor_li,
            disclosed_vendors: VendorBitSet::new(),
            special_feature_optins: self.special_feature_optins.into_iter().collect(),
            purpose_one_treatment: self.purpose_one_treatment,
            publisher_cc: *b"DE",
            publisher_restrictions: self.publisher_restrictions,
        }
    }
}

/// One vendor's declaration. `deleted_date` is the RFC 3339 literal the
/// GVL carries, so a test can place the deletion instant either side of
/// the authorization clock.
#[derive(Default)]
pub struct VendorSpec {
    pub id: u16,
    pub purposes: Vec<u8>,
    pub leg_int_purposes: Vec<u8>,
    pub flex_purposes: Vec<u8>,
    pub special_features: Vec<u8>,
    pub deleted_date: Option<&'static str>,
}

impl VendorSpec {
    /// A vendor declaring the given purposes under consent and nothing
    /// else, the shape most legal-basis tests want.
    #[must_use]
    pub fn consenting(id: u16, purposes: &[u8]) -> Self {
        Self {
            id,
            purposes: purposes.to_vec(),
            ..Self::default()
        }
    }

    fn render(&self) -> String {
        let deleted = self.deleted_date.map_or_else(String::new, |instant| {
            format!(r#","deletedDate":"{instant}""#)
        });
        format!(
            r#""{id}":{{"id":{id},"name":"Vendor {id}","purposes":{purposes},
               "legIntPurposes":{leg_int},"flexiblePurposes":{flex},
               "specialFeatures":{special_features}{deleted}}}"#,
            id = self.id,
            purposes = id_list(&self.purposes),
            leg_int = id_list(&self.leg_int_purposes),
            flex = id_list(&self.flex_purposes),
            special_features = id_list(&self.special_features),
        )
    }
}

fn id_list(ids: &[u8]) -> String {
    let joined = ids.iter().map(u8::to_string).collect::<Vec<_>>().join(",");
    format!("[{joined}]")
}

/// A GVL at `vendor_list_version` declaring purposes 1-10, special
/// purpose 1, feature 1 and special features 1-2, so any vendor
/// declaration in that range cross-references.
#[must_use]
pub fn gvl(vendor_list_version: u32, vendors: &[VendorSpec]) -> Gvl {
    let purposes = (1_u8..=10)
        .map(|id| format!(r#""{id}":{{"id":{id},"name":"Purpose {id}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    let rendered = vendors
        .iter()
        .map(VendorSpec::render)
        .collect::<Vec<_>>()
        .join(",");
    // Raw `format!`, not `serde_json::json!`: `auctioneer-privacy` has no
    // `serde_json` dependency on purpose — the GVL/GCL parse path is
    // delegated to `auctioneer-gvl-schema` as the one parse path, and
    // `Cargo.toml` says so. Converting would add that dependency back
    // for a fixture-formatting change.
    let json = format!(
        r#"{{"gvlSpecificationVersion":{SPEC},"tcfPolicyVersion":{POLICY_VERSION},
             "vendorListVersion":{vendor_list_version},
             "lastUpdated":"2026-07-01T00:00:00Z",
             "purposes":{{{purposes}}},
             "specialPurposes":{{"1":{{"id":1,"name":"Security"}}}},
             "features":{{"1":{{"id":1,"name":"Matching"}}}},
             "specialFeatures":{{"1":{{"id":1,"name":"Geolocation"}},
                                 "2":{{"id":2,"name":"Scanning"}}}},
             "vendors":{{{rendered}}}}}"#
    );
    Gvl::load_from_json(&json, None).expect("test GVL is valid")
}

/// Purposes 1-10, the range a vendor must declare to be authorizable on
/// every gate the permission walk consults.
#[must_use]
pub fn all_purposes() -> Vec<u8> {
    (1_u8..=10).collect()
}

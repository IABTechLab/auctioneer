//! Per-purpose legal basis: the full-enforcement and basic-enforcement
//! decision tables.
//!
//! Both tables are FIRST-MATCH: rows are evaluated top to bottom and the
//! first row whose conditions all hold decides the purpose; rows below it
//! never fire. The row order mirrors the GDPR truth tables verbatim, so
//! each `if` below is one table row and reordering them changes the legal
//! outcome.
//!
//! Full enforcement consults the GVL declaration set (which legal basis
//! the vendor declared for the purpose) and the publisher restrictions
//! carried in the TC string. Basic enforcement ignores GVL declarations
//! and publisher basis switching, but honors publisher denials and conflicting
//! bases before its configuration/TCF walk. Its LI-transparency override
//! requires a nonzero vendor id; vendor consent remains its final row.
//!
//! Policy lists (`vendor_exceptions`, `basic_enforcement_vendors`) are
//! keyed by the CORE bidder name. The alias name is never consulted, so
//! an alias whose name happens to collide with a policy entry cannot
//! inherit an unrelated grant.

use auctioneer_config_data::types::{EnforceAlgo, TriState};

use crate::auction_context::PrivacyContext;
use crate::gvl::Vendor;
use crate::tcf::{PubRestrictionType, TcfString};

/// The effective per-purpose enforcement bits, resolved from the
/// account-over-host tri-states before dispatch. The identity path
/// forces both on regardless of configuration, so the same pair of
/// functions serves both callers.
#[derive(Clone, Copy, Debug)]
pub struct EnforceFlags {
    pub enforce_purpose: bool,
    pub enforce_vendors: bool,
}

/// Full-enforcement legal basis for one (purpose, vendor).
#[must_use]
pub fn purpose_allowed(
    ctx: &PrivacyContext,
    purpose_id: u8,
    core_name: &str,
    vendor_id: u16,
    flags: EnforceFlags,
) -> bool {
    let consent = ctx.tcf.as_deref();

    let restriction = effective_publisher_restriction(consent, purpose_id, vendor_id);
    // Publisher denial and ambiguous bases precede every policy bypass.
    if restriction == Some(PubRestrictionType::NotAllowed) {
        return false;
    }

    if !flags.enforce_purpose && !flags.enforce_vendors {
        return true;
    }

    if is_vendor_exception(ctx, purpose_id, core_name) {
        return true;
    }

    // Every remaining row establishes a legal basis, and none of them can
    // succeed without a TC string, a GVL declaration, and a real vendor
    // id: id 0 means the bidder has no GVL id at all.
    if vendor_id == 0 {
        return false;
    }
    let Some(tcf) = consent else {
        return false;
    };
    let Some(vendor) = ctx.gvl.vendor(vendor_id) else {
        return false;
    };

    // A restriction switches which basis applies, so the two rows below
    // evaluate exactly one leg and the suppressed leg cannot rescue it.
    if restriction == Some(PubRestrictionType::RequireConsent) {
        return consent_established(
            tcf,
            vendor,
            purpose_id,
            vendor_id,
            flags,
            Some(PubRestrictionType::RequireConsent),
        );
    }
    if restriction == Some(PubRestrictionType::RequireLegitInterest) {
        return li_established(
            tcf,
            vendor,
            purpose_id,
            vendor_id,
            flags,
            Some(PubRestrictionType::RequireLegitInterest),
        );
    }

    // Restriction-first: both legs receive the (purpose, vendor)
    // restriction so a flexible purpose is authorized on its DEFAULT
    // basis only. The three rows above have already excluded every
    // restriction type, so the resolved restriction is `None` here; the legs take it
    // as an argument rather than assuming that. Neither leg establishing
    // a basis is the closing deny of the table.
    consent_established(tcf, vendor, purpose_id, vendor_id, flags, restriction)
        || li_established(tcf, vendor, purpose_id, vendor_id, flags, restriction)
}

/// Basic-enforcement legal basis for one (purpose, vendor).
///
/// `allow_li_transparency` is an invocation override, not configuration:
/// callers pass `purpose_id == 2` so only basic Purpose 2 can be
/// satisfied by an LI-transparency bit. Purposes 3-10 can never be
/// granted by that bit without vendor consent. The override cannot grant
/// an unknown vendor (id 0). Publisher denials precede all bypass rows.
#[must_use]
pub fn purpose_allowed_basic(
    ctx: &PrivacyContext,
    purpose_id: u8,
    core_name: &str,
    vendor_id: u16,
    flags: EnforceFlags,
    allow_li_transparency: bool,
) -> bool {
    let consent = ctx.tcf.as_deref();
    if effective_publisher_restriction(consent, purpose_id, vendor_id)
        == Some(PubRestrictionType::NotAllowed)
    {
        return false;
    }
    let purpose_consented = consent.is_some_and(|t| t.purposes_consent.contains(&purpose_id));
    let purpose_li_transparent =
        consent.is_some_and(|t| t.purposes_li_transparency.contains(&purpose_id));
    let basic_vendor = ctx.basic_enforcement_vendors.contains(core_name);

    if !flags.enforce_purpose && !flags.enforce_vendors {
        return true;
    }
    if is_vendor_exception(ctx, purpose_id, core_name) {
        return true;
    }
    // Reaching here means vendor enforcement is on, so a basic bidder
    // bypasses the vendor-consent bit whenever the purpose itself is
    // unenforced, or is enforced and permitted.
    if !flags.enforce_purpose && basic_vendor {
        return true;
    }
    if flags.enforce_purpose && purpose_consented && basic_vendor {
        return true;
    }
    if flags.enforce_purpose && purpose_li_transparent && allow_li_transparency && vendor_id != 0 {
        return true;
    }
    if flags.enforce_purpose && !purpose_consented {
        return false;
    }
    if !flags.enforce_vendors {
        return true;
    }
    // Basic enforcement never consults the GVL declaration set, but it
    // does read the vendor-consent bit as its final row. A bidder with no
    // GVL id arrives with id 0, which is never in the set, so it denies
    // here. The LI override also rejects id 0; explicit exception/basic-vendor
    // bypasses retain their own policy rather than becoming a blanket id-0 deny.
    consent.is_some_and(|t| t.vendor_consent.contains(vendor_id))
}

/// Which enforcement table applies to a (purpose, core bidder) pair.
///
/// A bidder the account lists for basic enforcement is downgraded for
/// every purpose; otherwise the resolved per-purpose configuration
/// decides. An unconfigured purpose keeps the configuration default,
/// under which nothing is enforced anyway.
#[must_use]
pub fn purpose_effective_algo(
    ctx: &PrivacyContext,
    purpose_id: u8,
    core_name: &str,
) -> EnforceAlgo {
    if ctx.basic_enforcement_vendors.contains(core_name) {
        return EnforceAlgo::Basic;
    }
    ctx.per_purpose_effective
        .get(&purpose_id)
        .map_or(EnforceAlgo::Full, |cfg| cfg.enforce_algo)
}

/// The configured enforcement flags for one purpose, read from the
/// account-over-host table the resolver cached.
///
/// A purpose absent from the table is one neither layer configured, and
/// an unconfigured purpose enforces nothing. Indexing the map instead
/// would panic on any deployment that enumerates only the purposes it
/// cares about.
#[must_use]
pub fn effective_flags_for_purpose(ctx: &PrivacyContext, purpose_id: u8) -> EnforceFlags {
    let resolved = ctx.per_purpose_effective.get(&purpose_id);
    EnforceFlags {
        enforce_purpose: resolved.is_some_and(|cfg| cfg.enforce_purpose == TriState::Enabled),
        enforce_vendors: resolved.is_some_and(|cfg| cfg.enforce_vendors == TriState::Enabled),
    }
}

/// Collapse conflicting publisher bases to an effective denial without
/// altering the decoded records. Duplicate same-type matches remain valid.
pub(crate) fn effective_publisher_restriction(
    consent: Option<&TcfString>,
    purpose_id: u8,
    vendor_id: u16,
) -> Option<PubRestrictionType> {
    let tcf = consent?;
    if tcf.check_pub_restriction(purpose_id, PubRestrictionType::NotAllowed, vendor_id) {
        return Some(PubRestrictionType::NotAllowed);
    }
    let require_consent =
        tcf.check_pub_restriction(purpose_id, PubRestrictionType::RequireConsent, vendor_id);
    let require_li = tcf.check_pub_restriction(
        purpose_id,
        PubRestrictionType::RequireLegitInterest,
        vendor_id,
    );
    match (require_consent, require_li) {
        (true, true) => Some(PubRestrictionType::NotAllowed),
        (true, false) => Some(PubRestrictionType::RequireConsent),
        (false, true) => Some(PubRestrictionType::RequireLegitInterest),
        (false, false) => None,
    }
}

/// Is the CORE bidder listed in this purpose's `vendor_exceptions`?
fn is_vendor_exception(ctx: &PrivacyContext, purpose_id: u8, core_name: &str) -> bool {
    ctx.per_purpose_effective
        .get(&purpose_id)
        .is_some_and(|cfg| {
            cfg.vendor_exceptions
                .iter()
                .any(|listed| listed.as_str() == core_name)
        })
}

/// The consent leg: the vendor's declaration under the resolved
/// restriction, then the purpose and vendor bits each gated by its own
/// enforcement flag.
fn consent_established(
    tcf: &TcfString,
    vendor: &Vendor,
    purpose_id: u8,
    vendor_id: u16,
    flags: EnforceFlags,
    restriction: Option<PubRestrictionType>,
) -> bool {
    vendor.declares_purpose_for_consent(purpose_id, restriction)
        && (!flags.enforce_purpose || tcf.purposes_consent.contains(&purpose_id))
        && (!flags.enforce_vendors || tcf.vendor_consent.contains(vendor_id))
}

/// The legitimate-interest leg, structurally identical to the consent
/// leg over the LI declaration set and the LI bits.
fn li_established(
    tcf: &TcfString,
    vendor: &Vendor,
    purpose_id: u8,
    vendor_id: u16,
    flags: EnforceFlags,
    restriction: Option<PubRestrictionType>,
) -> bool {
    vendor.declares_purpose_for_li(purpose_id, restriction)
        && (!flags.enforce_purpose || tcf.purposes_li_transparency.contains(&purpose_id))
        && (!flags.enforce_vendors || tcf.vendor_li.contains(vendor_id))
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use auctioneer_config_data::types::{EnforceAlgo, PurposeEnforcement, TriState};
    use time::macros::datetime;

    use super::{EnforceFlags, purpose_allowed, purpose_allowed_basic, purpose_effective_algo};
    use crate::auction_context::PrivacyContext;
    use crate::gvl::Gvl;
    use crate::tcf::{
        PubRestrictionType, PublisherRestriction, TcfString, VendorBitSet, parse_tcf_v2,
    };

    const VENDOR: u16 = 8;
    const CORE: &str = "fake_corebidder";
    const ALIAS: &str = "fake_aliasbidder";

    const FULL: EnforceFlags = EnforceFlags {
        enforce_purpose: true,
        enforce_vendors: true,
    };
    const NOTHING: EnforceFlags = EnforceFlags {
        enforce_purpose: false,
        enforce_vendors: false,
    };
    const VENDORS_ONLY: EnforceFlags = EnforceFlags {
        enforce_purpose: false,
        enforce_vendors: true,
    };
    const PURPOSE_ONLY: EnforceFlags = EnforceFlags {
        enforce_purpose: true,
        enforce_vendors: false,
    };

    /// The TC string fields the decision tables read; everything else
    /// takes a fixed valid value.
    #[derive(Default)]
    struct TcfSpec {
        purposes_consent: Vec<u8>,
        purposes_li_transparency: Vec<u8>,
        vendor_consent: Vec<u16>,
        vendor_li: Vec<u16>,
        publisher_restrictions: Vec<PublisherRestriction>,
    }

    impl TcfSpec {
        fn build(self) -> TcfString {
            let mut vendor_consent = VendorBitSet::new();
            for id in self.vendor_consent {
                vendor_consent.set(id);
            }
            let mut vendor_li = VendorBitSet::new();
            for id in self.vendor_li {
                vendor_li.set(id);
            }
            TcfString {
                tcf_policy_version: 4,
                vendor_list_version: 42,
                cmp_id: 300,
                cmp_version: 2,
                created: datetime!(2026-02-01 00:00:00 UTC),
                last_updated: datetime!(2026-02-01 00:00:00 UTC),
                purposes_consent: self.purposes_consent.into_iter().collect(),
                purposes_li_transparency: self.purposes_li_transparency.into_iter().collect(),
                vendor_consent,
                vendor_li,
                disclosed_vendors: VendorBitSet::new(),
                special_feature_optins: BTreeSet::new(),
                purpose_one_treatment: false,
                publisher_cc: *b"DE",
                publisher_restrictions: self.publisher_restrictions,
            }
        }
    }

    fn restriction(restriction_type: PubRestrictionType, purpose_id: u8) -> PublisherRestriction {
        PublisherRestriction {
            purpose_id,
            restriction_type,
            vendor_range: VENDOR..=VENDOR,
        }
    }

    fn id_list(ids: &[u8]) -> String {
        let joined = ids.iter().map(u8::to_string).collect::<Vec<_>>().join(",");
        format!("[{joined}]")
    }

    /// A one-vendor GVL where vendor [`VENDOR`] declares the given
    /// purposes under consent, under legitimate interest, and as
    /// flexible. Purposes 1-10 exist at the top level so any
    /// combination cross-references.
    fn gvl_with(purposes: &[u8], leg_int: &[u8], flex: &[u8]) -> Gvl {
        let table = (1_u8..=10)
            .map(|id| format!(r#""{id}":{{"id":{id},"name":"Purpose {id}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        // Raw `format!`, not `serde_json::json!`: `auctioneer-privacy` has
        // no `serde_json` dependency on purpose — see the note in
        // `testkit::gvl`.
        let json = format!(
            r#"{{"gvlSpecificationVersion":3,"tcfPolicyVersion":5,
                 "vendorListVersion":42,"lastUpdated":"2026-07-01T00:00:00Z",
                 "purposes":{{{table}}},
                 "specialPurposes":{{"1":{{"id":1,"name":"Security"}}}},
                 "features":{{"1":{{"id":1,"name":"Matching"}}}},
                 "specialFeatures":{{"1":{{"id":1,"name":"Geolocation"}}}},
                 "vendors":{{"{vendor}":{{"id":{vendor},"name":"Vendor Eight",
                   "purposes":{consent},"legIntPurposes":{li},
                   "flexiblePurposes":{flexible}}}}}}}"#,
            vendor = VENDOR,
            consent = id_list(purposes),
            li = id_list(leg_int),
            flexible = id_list(flex),
        );
        Gvl::load_from_json(&json, None).expect("test GVL is valid")
    }

    fn purpose_config(
        purpose_id: u8,
        enforce_algo: EnforceAlgo,
        vendor_exceptions: &[&str],
    ) -> BTreeMap<u8, PurposeEnforcement> {
        BTreeMap::from([(
            purpose_id,
            PurposeEnforcement {
                enforce_algo,
                enforce_purpose: TriState::Enabled,
                enforce_vendors: TriState::Enabled,
                vendor_exceptions: vendor_exceptions
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
            },
        )])
    }

    fn no_config() -> BTreeMap<u8, PurposeEnforcement> {
        BTreeMap::new()
    }

    fn names(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/data/tcf_v2/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("fixture {path} unreadable: {err}"))
            .trim_end()
            .to_owned()
    }

    // ─────────────── full enforcement, consent path (6) ───────────────

    #[test]
    fn full_consent_purpose_declared_in_vendor_purposes_consent_and_tcf_allows() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[2], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    #[test]
    fn flex_consent_default_no_restriction_allows_via_consent_only() {
        // Purpose 2 declared under consent AND as flexible. Absent a
        // publisher restriction only the consent default applies.
        let consent_bits = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(consent_bits),
            gvl_with(&[2], &[], &[2]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));

        let li_bits = TcfSpec {
            purposes_li_transparency: vec![2],
            vendor_li: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let li_only = PrivacyContext::for_test(
            Some(li_bits),
            gvl_with(&[2], &[], &[2]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&li_only, 2, CORE, VENDOR, FULL));
    }

    #[test]
    fn full_consent_purpose_declared_in_vendor_flex_purposes_with_require_consent_restriction_allows()
     {
        let restricted = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            publisher_restrictions: vec![restriction(PubRestrictionType::RequireConsent, 2)],
            ..TcfSpec::default()
        }
        .build();
        // Default basis is legitimate interest; the restriction switches
        // the flexible purpose to consent.
        let ctx = PrivacyContext::for_test(
            Some(restricted),
            gvl_with(&[], &[2], &[2]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));

        let unrestricted = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let without_restriction = PrivacyContext::for_test(
            Some(unrestricted),
            gvl_with(&[], &[2], &[2]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(
            &without_restriction,
            2,
            CORE,
            VENDOR,
            FULL
        ));
    }

    #[test]
    fn full_consent_purpose_absent_from_vendor_gvl_denies_even_with_tcf_consent() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    #[test]
    fn full_consent_vendor_missing_purpose_consent_bit_denies() {
        let tcf = TcfSpec {
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[2], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    #[test]
    fn full_consent_vendor_missing_from_tcf_vendor_consent_denies() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[2], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    // ───────────────── full enforcement, LI path (5) ─────────────────

    #[test]
    fn full_li_purpose_declared_in_vendor_leg_int_purposes_allows() {
        let tcf = TcfSpec {
            purposes_li_transparency: vec![7],
            vendor_li: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[7], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(purpose_allowed(&ctx, 7, CORE, VENDOR, FULL));
    }

    #[test]
    fn full_li_purpose_declared_in_flex_allows() {
        let tcf = TcfSpec {
            purposes_li_transparency: vec![7],
            vendor_li: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[7], &[7]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(purpose_allowed(&ctx, 7, CORE, VENDOR, FULL));

        // Same LI bits, but the flexible purpose's default is consent —
        // absent a restriction the LI leg is not open to it.
        let consent_default = TcfSpec {
            purposes_li_transparency: vec![7],
            vendor_li: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let consent_default_ctx = PrivacyContext::for_test(
            Some(consent_default),
            gvl_with(&[7], &[], &[7]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(
            &consent_default_ctx,
            7,
            CORE,
            VENDOR,
            FULL
        ));
    }

    #[test]
    fn full_li_purpose_absent_from_gvl_denies_even_with_tcf_li_transparency() {
        let tcf = TcfSpec {
            purposes_li_transparency: vec![7],
            vendor_li: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 7, CORE, VENDOR, FULL));
    }

    #[test]
    fn full_li_vendor_missing_from_tcf_vendor_li_denies() {
        let tcf = TcfSpec {
            purposes_li_transparency: vec![7],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[7], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 7, CORE, VENDOR, FULL));
    }

    #[test]
    fn disclosed_vendor_with_unset_vendor_li_bit_denied_no_v24_li_workaround() {
        let raw = fixture("core_vendor_li_bit_unset.txt");
        let tcf = parse_tcf_v2(&raw).expect("fixture is a valid TC string");
        // The fixture's trailing Disclosed Vendors segment names the
        // vendor and its membership is retained, but it cannot establish
        // an LI basis without the core vendor legitimate-interest bit.
        assert!(tcf.disclosed_vendors.contains(VENDOR));
        assert!(tcf.purposes_li_transparency.contains(&7));
        assert!(!tcf.vendor_li.contains(VENDOR));

        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[7], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 7, CORE, VENDOR, FULL));
    }

    // ──────────────────── publisher restrictions ────────────────────

    #[test]
    fn conflicting_publisher_bases_deny_in_either_order_before_policy_bypasses() {
        for order in [
            [
                PubRestrictionType::RequireConsent,
                PubRestrictionType::RequireLegitInterest,
            ],
            [
                PubRestrictionType::RequireLegitInterest,
                PubRestrictionType::RequireConsent,
            ],
        ] {
            for consent in [false, true] {
                for li in [false, true] {
                    for exceptions in [vec![], vec![CORE]] {
                        let ctx = PrivacyContext::for_test(
                            Some(
                                TcfSpec {
                                    purposes_consent: if consent { vec![2] } else { vec![] },
                                    vendor_consent: if consent { vec![VENDOR] } else { vec![] },
                                    purposes_li_transparency: if li { vec![2] } else { vec![] },
                                    vendor_li: if li { vec![VENDOR] } else { vec![] },
                                    publisher_restrictions: order
                                        .map(|kind| restriction(kind, 2))
                                        .to_vec(),
                                }
                                .build(),
                            ),
                            gvl_with(&[], &[2], &[2]),
                            purpose_config(2, EnforceAlgo::Full, &exceptions),
                            BTreeSet::new(),
                        );
                        for flags in [FULL, NOTHING, VENDORS_ONLY, PURPOSE_ONLY] {
                            assert!(
                                !purpose_allowed(&ctx, 2, CORE, VENDOR, flags),
                                "order={order:?} consent={consent} li={li} exceptions={exceptions:?} flags={flags:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn publisher_basis_conflicts_are_scoped_to_the_matching_purpose_and_vendor() {
        for unrelated in [
            restriction(PubRestrictionType::RequireLegitInterest, 3),
            PublisherRestriction {
                purpose_id: 2,
                restriction_type: PubRestrictionType::RequireLegitInterest,
                vendor_range: (VENDOR + 1)..=(VENDOR + 2),
            },
            restriction(PubRestrictionType::RequireConsent, 2),
        ] {
            let ctx = PrivacyContext::for_test(
                Some(
                    TcfSpec {
                        purposes_consent: vec![2],
                        vendor_consent: vec![VENDOR],
                        publisher_restrictions: vec![
                            PublisherRestriction {
                                purpose_id: 2,
                                restriction_type: PubRestrictionType::RequireConsent,
                                vendor_range: (VENDOR - 1)..=(VENDOR + 1),
                            },
                            unrelated,
                        ],
                        ..TcfSpec::default()
                    }
                    .build(),
                ),
                gvl_with(&[], &[2], &[2]),
                no_config(),
                BTreeSet::new(),
            );
            assert!(purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
        }
    }

    #[test]
    fn pub_restriction_not_allowed_denies_regardless_of_consent() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            publisher_restrictions: vec![restriction(PubRestrictionType::NotAllowed, 2)],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf.clone()),
            gvl_with(&[2], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));

        // Not even a vendor exception or a fully unenforced purpose
        // overrides it.
        let excepted = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[2], &[], &[]),
            purpose_config(2, EnforceAlgo::Full, &[CORE]),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&excepted, 2, CORE, VENDOR, FULL));
        assert!(!purpose_allowed(&excepted, 2, CORE, VENDOR, NOTHING));
    }

    #[test]
    fn pub_restriction_require_consent_suppresses_li_path() {
        let tcf = TcfSpec {
            purposes_li_transparency: vec![2],
            vendor_li: vec![VENDOR],
            publisher_restrictions: vec![restriction(PubRestrictionType::RequireConsent, 2)],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[2], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    #[test]
    fn pub_restriction_require_legit_interest_suppresses_consent_path() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            publisher_restrictions: vec![restriction(PubRestrictionType::RequireLegitInterest, 2)],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[2], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    // ─────────────── vendor exceptions + special cases (4) ───────────────

    #[test]
    fn vendor_exceptions_bypass_gvl_declaration_check() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[], &[], &[]),
            purpose_config(2, EnforceAlgo::Full, &[CORE]),
            BTreeSet::new(),
        );
        assert!(purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    #[test]
    fn vendor_exceptions_bypass_tcf_vendor_consent() {
        let ctx = PrivacyContext::for_test(
            Some(TcfSpec::default().build()),
            gvl_with(&[2], &[], &[]),
            purpose_config(2, EnforceAlgo::Full, &[CORE]),
            BTreeSet::new(),
        );
        assert!(purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    #[test]
    fn vendor_id_zero_denies_under_full_enforcement() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(
            Some(tcf),
            gvl_with(&[2], &[], &[]),
            no_config(),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, 0, FULL));
    }

    #[test]
    fn vendor_exceptions_ignore_alias_name_collision() {
        // The exception names the ALIAS; the core bidder is absent, so
        // the walk falls through to the legal-basis rows and denies.
        let ctx = PrivacyContext::for_test(
            Some(TcfSpec::default().build()),
            gvl_with(&[2], &[], &[]),
            purpose_config(2, EnforceAlgo::Full, &[ALIAS]),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed(&ctx, 2, CORE, VENDOR, FULL));
    }

    // ──────────────── basic enforcement, the ordered walk (12) ────────────────

    #[test]
    fn basic_publisher_denials_precede_disabled_flags_and_policy_bypasses() {
        for restrictions in [
            vec![PubRestrictionType::NotAllowed],
            vec![
                PubRestrictionType::RequireConsent,
                PubRestrictionType::RequireLegitInterest,
            ],
            vec![
                PubRestrictionType::RequireLegitInterest,
                PubRestrictionType::RequireConsent,
            ],
        ] {
            for excepted in [false, true] {
                for basic_vendor in [false, true] {
                    let ctx = PrivacyContext::for_test(
                        Some(
                            TcfSpec {
                                purposes_consent: vec![2],
                                purposes_li_transparency: vec![2],
                                vendor_consent: vec![VENDOR],
                                publisher_restrictions: restrictions
                                    .iter()
                                    .map(|kind| restriction(*kind, 2))
                                    .collect(),
                                ..TcfSpec::default()
                            }
                            .build(),
                        ),
                        Gvl::empty(),
                        purpose_config(2, EnforceAlgo::Basic, if excepted { &[CORE] } else { &[] }),
                        if basic_vendor {
                            names(&[CORE])
                        } else {
                            BTreeSet::new()
                        },
                    );
                    for flags in [FULL, NOTHING, VENDORS_ONLY, PURPOSE_ONLY] {
                        assert!(
                            !purpose_allowed_basic(&ctx, 2, CORE, VENDOR, flags, true),
                            "restrictions={restrictions:?} excepted={excepted} basic_vendor={basic_vendor} flags={flags:?}",
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn basic_publisher_denials_only_apply_to_the_matching_purpose_and_vendor() {
        for unrelated in [
            restriction(PubRestrictionType::NotAllowed, 3),
            PublisherRestriction {
                purpose_id: 2,
                restriction_type: PubRestrictionType::NotAllowed,
                vendor_range: (VENDOR + 1)..=(VENDOR + 2),
            },
            restriction(PubRestrictionType::RequireLegitInterest, 3),
            PublisherRestriction {
                purpose_id: 2,
                restriction_type: PubRestrictionType::RequireLegitInterest,
                vendor_range: (VENDOR + 1)..=(VENDOR + 2),
            },
            restriction(PubRestrictionType::RequireConsent, 2),
        ] {
            let ctx = PrivacyContext::for_test(
                Some(
                    TcfSpec {
                        purposes_consent: vec![2],
                        vendor_consent: vec![VENDOR],
                        publisher_restrictions: vec![
                            restriction(PubRestrictionType::RequireConsent, 2),
                            unrelated,
                        ],
                        ..TcfSpec::default()
                    }
                    .build(),
                ),
                Gvl::empty(),
                no_config(),
                BTreeSet::new(),
            );
            assert!(purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, true));
        }
    }

    #[test]
    fn basic_li_transparency_override_requires_a_nonzero_vendor_id() {
        let ctx = PrivacyContext::for_test(
            Some(
                TcfSpec {
                    purposes_li_transparency: vec![2],
                    ..TcfSpec::default()
                }
                .build(),
            ),
            Gvl::empty(),
            no_config(),
            BTreeSet::new(),
        );
        for vendor in [0, VENDOR] {
            for override_enabled in [false, true] {
                assert_eq!(
                    purpose_allowed_basic(&ctx, 2, CORE, vendor, FULL, override_enabled),
                    vendor != 0 && override_enabled,
                    "vendor={vendor} override={override_enabled}",
                );
            }
        }
    }

    #[test]
    fn both_flags_disabled_allows_regardless_of_tcf() {
        let ctx = PrivacyContext::for_test(
            Some(TcfSpec::default().build()),
            Gvl::empty(),
            no_config(),
            BTreeSet::new(),
        );
        assert!(purpose_allowed_basic(&ctx, 2, CORE, VENDOR, NOTHING, false));
    }

    #[test]
    fn vendor_exceptions_short_circuit_allows() {
        // The core bidder is ALSO a basic-enforcement vendor with no
        // purpose-consent bit, so rows 3 and 4 cannot fire and row 6
        // would deny: only the exception row can produce this allow.
        let ctx = PrivacyContext::for_test(
            Some(TcfSpec::default().build()),
            Gvl::empty(),
            purpose_config(2, EnforceAlgo::Basic, &[CORE]),
            names(&[CORE]),
        );
        assert!(purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, false));
    }

    #[test]
    fn basic_vendor_purpose_unenforced_bypasses_vendor_consent() {
        let ctx = PrivacyContext::for_test(
            Some(TcfSpec::default().build()),
            Gvl::empty(),
            no_config(),
            names(&[CORE]),
        );
        assert!(purpose_allowed_basic(
            &ctx,
            2,
            CORE,
            VENDOR,
            VENDORS_ONLY,
            false
        ));
    }

    #[test]
    fn basic_vendor_purpose_enforced_and_permitted_skips_vendor_bit() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), names(&[CORE]));
        assert!(purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, false));
    }

    #[test]
    fn basic_p2_li_transparency_override_allows_without_vendor_lookup() {
        let tcf = TcfSpec {
            purposes_li_transparency: vec![2],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), BTreeSet::new());
        assert!(purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, true));
    }

    #[test]
    fn basic_p3_li_transparency_denied_without_override() {
        let tcf = TcfSpec {
            purposes_li_transparency: vec![3],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), BTreeSet::new());
        assert!(!purpose_allowed_basic(&ctx, 3, CORE, VENDOR, FULL, false));
    }

    #[test]
    fn purpose_enforced_but_bit_absent_denies() {
        // Vendor consent is present, so only the earlier deny row can
        // produce this result.
        let tcf = TcfSpec {
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), BTreeSet::new());
        assert!(!purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, false));
    }

    #[test]
    fn enforce_vendors_disabled_falls_through_to_purpose_only() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), BTreeSet::new());
        assert!(purpose_allowed_basic(
            &ctx,
            2,
            CORE,
            VENDOR,
            PURPOSE_ONLY,
            false
        ));
    }

    #[test]
    fn default_consults_tcf_vendor_consent_bit() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), BTreeSet::new());
        assert!(purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, false));
    }

    #[test]
    fn no_row_matches_denies_by_default() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            vendor_consent: vec![VENDOR],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), BTreeSet::new());
        // A bidder with no GVL id arrives as vendor 0, which the wire
        // vendor-consent set can never contain.
        assert!(!purpose_allowed_basic(&ctx, 2, CORE, 0, FULL, false));
        // A vendor outside the consent set denies the same way.
        assert!(!purpose_allowed_basic(&ctx, 2, CORE, 9, FULL, false));
    }

    #[test]
    fn basic_vendor_exceptions_ignore_alias_name_collision() {
        let ctx = PrivacyContext::for_test(
            Some(TcfSpec::default().build()),
            Gvl::empty(),
            purpose_config(2, EnforceAlgo::Basic, &[ALIAS]),
            BTreeSet::new(),
        );
        assert!(!purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, false));
    }

    #[test]
    fn basic_enforcement_vendors_ignore_alias_name_collision() {
        let tcf = TcfSpec {
            purposes_consent: vec![2],
            ..TcfSpec::default()
        }
        .build();
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), names(&[ALIAS]));
        // Row 3: the purpose is unenforced but the core bidder is not
        // listed, so vendor consent is still required.
        assert!(!purpose_allowed_basic(
            &ctx,
            2,
            CORE,
            VENDOR,
            VENDORS_ONLY,
            false
        ));
        // Row 4: the purpose bit is set but the core bidder is not
        // listed, so the walk still reaches the vendor-consent row.
        assert!(!purpose_allowed_basic(&ctx, 2, CORE, VENDOR, FULL, false));
    }

    // ──────────── effective algorithm, keyed by the core name (2) ────────────

    #[test]
    fn basic_enforcement_vendors_matches_core_name_downgrades() {
        let downgraded = PrivacyContext::for_test(
            None,
            Gvl::empty(),
            purpose_config(2, EnforceAlgo::Full, &[]),
            names(&[CORE]),
        );
        assert_eq!(
            purpose_effective_algo(&downgraded, 2, CORE),
            EnforceAlgo::Basic
        );

        // The same configuration without the listing keeps the
        // configured algorithm, so the listing is what flipped it.
        let listed_nowhere = PrivacyContext::for_test(
            None,
            Gvl::empty(),
            purpose_config(2, EnforceAlgo::Full, &[]),
            BTreeSet::new(),
        );
        assert_eq!(
            purpose_effective_algo(&listed_nowhere, 2, CORE),
            EnforceAlgo::Full
        );

        let configured_basic = PrivacyContext::for_test(
            None,
            Gvl::empty(),
            purpose_config(2, EnforceAlgo::Basic, &[]),
            BTreeSet::new(),
        );
        assert_eq!(
            purpose_effective_algo(&configured_basic, 2, CORE),
            EnforceAlgo::Basic
        );
    }

    #[test]
    fn basic_enforcement_vendors_ignore_alias_name_collision_returns_full() {
        let ctx = PrivacyContext::for_test(
            None,
            Gvl::empty(),
            purpose_config(2, EnforceAlgo::Full, &[]),
            names(&[ALIAS]),
        );
        assert_eq!(purpose_effective_algo(&ctx, 2, CORE), EnforceAlgo::Full);
    }

    // ─────────────────── no-GVL-id regression (1) ───────────────────

    #[test]
    fn no_gvl_bidder_denied_against_zero_bit() {
        // A vendor-consent set whose bit 0 is set by construction: the
        // storage layer must refuse to report id 0 as present, so a
        // bidder with no GVL id cannot be granted by it.
        let mut words = [0_u64; 1024];
        words[0] = 1;
        let tcf = TcfString {
            vendor_consent: VendorBitSet::from_raw_words(&words),
            ..TcfSpec {
                purposes_consent: vec![2],
                ..TcfSpec::default()
            }
            .build()
        };
        let ctx = PrivacyContext::for_test(Some(tcf), Gvl::empty(), no_config(), BTreeSet::new());
        assert!(!purpose_allowed_basic(&ctx, 2, CORE, 0, FULL, false));
    }
}

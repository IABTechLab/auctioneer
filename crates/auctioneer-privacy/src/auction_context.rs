//! Auction-path privacy context.
//!
//! `PrivacyContext` is the ONE decision context: the GDPR resolution
//! path populates its owned fields once per request, and the per-purpose
//! enforcement algorithms in [`crate::purposes`] read them. It carries no
//! lifetime — the TCF string, the GVL handle, and the resolved
//! enforcement tables are all owned, so it outlives the borrowed
//! `BidRequest` it was resolved from.
//!
//! GDPR scope does not bypass independent activity, CCPA, LMT, or COPPA
//! decisions. The exchange applies the resulting bidder decision to its
//! bidder-owned request; this crate never mutates the auction request.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::mem;

use auctioneer_config::activity::ActivityConfigs;
use auctioneer_config::types::{EnforceAlgo, PurposeEnforcement, SpecialFeatureEnforcement};
use time::OffsetDateTime;

#[cfg(test)]
use crate::testkit::AUTHORIZATION_NOW;
use crate::{
    activity::{Activity, Component},
    auction_permissions::AuctionPermissions,
    decision::IndependentPolicies,
    gvl::Gvl,
    purposes::{
        EnforceFlags, effective_flags_for_purpose, purpose_allowed, purpose_allowed_basic,
        purpose_effective_algo,
    },
    resolve::SPECIAL_FEATURE_GEO,
    tcf::TcfString,
};

/// Identity passes only under full enforcement of both legs, whatever
/// the operator configured per purpose: an identifier is a stronger
/// grant than "may this bidder bid".
const FORCED: EnforceFlags = EnforceFlags {
    enforce_purpose: true,
    enforce_vendors: true,
};

/// Auction-path privacy decisions.
///
/// No `Default`: the `gvl` handle has none, and a defaultable
/// "permissive empty" context is exactly the fail-open shape the GDPR
/// path must not have. Fields are `pub(crate)` because the decision
/// algorithms and the resolver live in sibling modules and Rust field
/// privacy is module-scoped; nothing outside this crate can reach them.
#[expect(
    clippy::field_scoped_visibility_modifiers,
    reason = "the resolver writes and the decision tables read these fields from sibling \
              modules; a private field plus a getter/setter pair per field would be pure \
              ceremony, and crate visibility already keeps them off the public API"
)]
pub struct PrivacyContext {
    /// Immutable account rules shared without per-auction rule copies.
    pub(crate) activities: ActivityConfigs,
    pub(crate) independent: IndependentPolicies,
    /// Non-fatal warnings accumulated during resolution, drained once
    /// per auction by [`PrivacyContext::take_warnings`].
    pub(crate) warnings: Vec<auctioneer_openrtb::NonFatalWarning>,
    /// Effective per-purpose enforcement, account overrides already
    /// merged over the host defaults. Sparse: a purpose with no entry
    /// is unconfigured, which resolves default-permissive.
    pub(crate) per_purpose_effective: BTreeMap<u8, PurposeEnforcement>,
    /// `false` for traffic GDPR does not reach: the host kill switch,
    /// `Signal::No`, a channel the operator did not enable, or a geo
    /// ladder that resolves out of scope. `permissions()` checks this
    /// FIRST and allows everything, because establishment and legal
    /// basis are both moot where the regulation does not apply. It is
    /// the only state in which `consent_usable` says nothing.
    pub(crate) enforcement_active: bool,
    /// `false` when consent is UNUSABLE (absent/empty, malformed TCF, or
    /// the CMP-declared GVL revision unavailable). `permissions()` checks
    /// this SECOND and returns the deterministic default row, so the
    /// GVL- and TCF-independent grants (`vendor_exceptions`, basic
    /// enforcement) can never leak `PassID = true` on unusable consent.
    pub(crate) consent_usable: bool,
    /// The authorization instant: one clock reading per request, shared
    /// by resolve-time checks (CMP deletion, GCL staleness) and
    /// permission-time checks (vendor deletion), so a single request can
    /// never straddle two instants.
    pub(crate) authorization_now: OffsetDateTime,
    /// Resolved Purpose-2 `enforce_purpose`, cached for the
    /// unusable-consent default row (`AllowBidRequest = !p2_enforced`).
    pub(crate) p2_enforced: bool,
    /// Resolved special-feature-1 enforcement, cached for the
    /// unusable-consent default row (`PassGeo = !sf1_enforced`).
    pub(crate) sf1_enforced: bool,
    /// Parsed TCF string when the request supplied a usable one.
    ///
    /// Boxed: `TcfString` carries three fixed 8 KiB vendor bitsets, and
    /// `run_auction` holds a `PrivacyContext` across await points, so
    /// storing it inline would put 24 KiB of mostly-zero bits into every
    /// in-flight auction future.
    pub(crate) tcf: Option<Box<TcfString>>,
    /// The GVL revision the TC string declared. `Gvl` is a cheap-clone
    /// `Arc` wrapper, so holding it is a refcount bump, not a copy of
    /// the vendor map.
    pub(crate) gvl: Gvl,
    /// Request-declared `ext.prebid.aliasgvlids`, keyed by alias name.
    /// Owned so the context does not borrow the request.
    pub(crate) alias_gvl_ids: BTreeMap<Box<str>, u32>,
    /// Account-configured basic-enforcement bidders, keyed by CORE
    /// bidder name. Account-only: there is no host fallthrough.
    pub(crate) basic_enforcement_vendors: BTreeSet<String>,
    /// Effective special-feature-1 enforcement. `None` when SF1 is
    /// unconfigured on both layers, which permits geo by default.
    pub(crate) sf1_effective: Option<SpecialFeatureEnforcement>,
}

impl fmt::Debug for PrivacyContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PrivacyContext")
            .field("enforcement_active", &self.enforcement_active)
            .field("consent_usable", &self.consent_usable)
            .field("has_tcf", &self.tcf.is_some())
            .field("warning_count", &self.warnings.len())
            .field("alias_count", &self.alias_gvl_ids.len())
            .finish_non_exhaustive()
    }
}

impl PrivacyContext {
    /// Explicitly permissive context: no policies, consent string, or GVL.
    /// Auction callers must use `resolve`, including when GDPR is disabled,
    /// so independent policies and input validation still run.
    ///
    /// `consent_usable` is `true` here on purpose. It gates the
    /// fail-closed default row, and out-of-scope traffic must reach
    /// `AllowAll` rather than that row; with an empty enforcement table
    /// every purpose takes the everything-unenforced path anyway.
    #[must_use]
    pub fn allow_all_no_scrub() -> Self {
        Self::with_activities(ActivityConfigs::default())
    }

    pub(crate) fn with_activities(activities: ActivityConfigs) -> Self {
        Self {
            activities,
            independent: IndependentPolicies::default(),
            warnings: Vec::new(),
            per_purpose_effective: BTreeMap::new(),
            enforcement_active: false,
            consent_usable: true,
            authorization_now: OffsetDateTime::UNIX_EPOCH,
            p2_enforced: false,
            sf1_enforced: false,
            tcf: None,
            gvl: Gvl::empty(),
            alias_gvl_ids: BTreeMap::new(),
            basic_enforcement_vendors: BTreeSet::new(),
            sf1_effective: None,
        }
    }

    /// Field builder for in-crate tests: an enforcing context with the
    /// four fields the enforcement algorithms read supplied directly, so
    /// a decision test needs no resolution path.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn for_test(
        tcf: Option<TcfString>,
        gvl: Gvl,
        per_purpose_effective: BTreeMap<u8, PurposeEnforcement>,
        basic_enforcement_vendors: BTreeSet<String>,
    ) -> Self {
        Self {
            per_purpose_effective,
            enforcement_active: true,
            // A real instant, not the permissive base's epoch: vendor
            // deletion is evaluated against this field, and nothing has
            // been deleted as of 1970.
            authorization_now: AUTHORIZATION_NOW,
            tcf: tcf.map(Box::new),
            gvl,
            basic_enforcement_vendors,
            ..Self::allow_all_no_scrub()
        }
    }

    /// Ordered activity rules apply independently of GDPR enforcement.
    #[must_use]
    pub fn activity_allow(&self, activity: Activity, component: Component<'_>) -> bool {
        crate::activity_allowed(&self.activities, activity, component)
    }

    /// The per-bidder permission summary, keyed by the CORE bidder name
    /// and the participating bidder name (equal unless `bidder` is an
    /// alias of `core_name`).
    ///
    /// First-match, in the order the GDPR permission-assembly table
    /// gives: traffic the regulation does not reach allows everything;
    /// enforced traffic whose consent is unusable takes the
    /// deterministic default row without consulting any policy list;
    /// a vendor the GVL has deleted is denied on every gate; only then
    /// does the per-purpose legal-basis walk decide.
    ///
    /// Policy lists are keyed by `core_name` throughout, so an alias
    /// whose name collides with a `vendor_exceptions` or
    /// `basic_enforcement_vendors` entry cannot inherit that grant.
    #[must_use]
    pub fn permissions(&self, core_name: &str, bidder: &str) -> AuctionPermissions {
        if !self.enforcement_active {
            return AuctionPermissions::AllowAll;
        }
        if !self.consent_usable {
            return self.default_permissions();
        }

        let vendor_id = self.vendor_id(core_name, bidder);
        if self
            .gvl
            .vendor(vendor_id)
            .is_some_and(|vendor| vendor.is_deleted_at(self.authorization_now))
        {
            // A vendor the list has deleted may no longer be established
            // to users, so deletion outranks even the grants that skip
            // the GVL and the TC string. The record still looks up; only
            // authorization is refused.
            return AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                pass_geo: false,
            };
        }

        let allow_bid_request = self.allow_bid_request(core_name, vendor_id);
        let pass_id = self.pass_id(core_name, vendor_id);
        let pass_geo = self.pass_geo(core_name, vendor_id);
        if allow_bid_request && pass_id && pass_geo {
            AuctionPermissions::AllowAll
        } else {
            AuctionPermissions::Partial {
                allow_bid_request,
                pass_id,
                pass_geo,
            }
        }
    }

    /// The row for enforced traffic whose consent is unusable.
    ///
    /// Per-request, not per-bidder: no vendor exception, no
    /// basic-enforcement downgrade, no GVL and no TC string are read, so
    /// none of the grants that bypass consent can leak an identifier
    /// through an unusable string. Bid and geo still flow to the extent
    /// the operator does not enforce Purpose 2 / special feature 1.
    fn default_permissions(&self) -> AuctionPermissions {
        AuctionPermissions::Partial {
            allow_bid_request: !self.p2_enforced,
            pass_id: false,
            pass_geo: !self.sf1_enforced,
        }
    }

    /// The GVL vendor id to authorize against.
    ///
    /// An alias's own `ext.prebid.aliasgvlids` entry wins; otherwise the
    /// core bidder's configured id applies, which is how an alias with
    /// no entry inherits its base's identity. An id outside the GVL's
    /// `u16` namespace is a configuration bug or a hostile request, so
    /// it normalises to the unknown-vendor id 0 — truncating would
    /// authorize against whichever vendor the low 16 bits happen to
    /// name.
    fn vendor_id(&self, core_name: &str, bidder: &str) -> u16 {
        let declared = if bidder == core_name {
            None
        } else {
            self.alias_gvl_ids.get(bidder).copied()
        };
        let resolved = match declared {
            Some(id) => Some(id),
            None => auctioneer_config::bidder(core_name).and_then(|info| info.gvl_vendor_id),
        };
        resolved.map_or(0, |id| u16::try_from(id).unwrap_or(0))
    }

    /// Purpose 2 under the operator's CONFIGURED flags: this gate asks
    /// whether the bidder may bid at all, which the operator is entitled
    /// to leave unenforced.
    fn allow_bid_request(&self, core_name: &str, vendor_id: u16) -> bool {
        let flags = effective_flags_for_purpose(self, 2);
        match purpose_effective_algo(self, 2, core_name) {
            EnforceAlgo::Basic => purpose_allowed_basic(self, 2, core_name, vendor_id, flags, true),
            EnforceAlgo::Full => purpose_allowed(self, 2, core_name, vendor_id, flags),
        }
    }

    /// One satisfied purpose in 2..=10 under forced enforcement passes
    /// the identifier. Legitimate-interest transparency alone satisfies
    /// basic Purpose 2 only for a nonzero vendor id, so an LI bit on some later
    /// purpose cannot stand in for vendor consent.
    fn pass_id(&self, core_name: &str, vendor_id: u16) -> bool {
        (2_u8..=10).any(
            |purpose_id| match purpose_effective_algo(self, purpose_id, core_name) {
                EnforceAlgo::Basic => purpose_allowed_basic(
                    self,
                    purpose_id,
                    core_name,
                    vendor_id,
                    FORCED,
                    purpose_id == 2,
                ),
                EnforceAlgo::Full => {
                    purpose_allowed(self, purpose_id, core_name, vendor_id, FORCED)
                }
            },
        )
    }

    /// Geo passes on the special-feature-1 opt-in AND the vendor having
    /// declared that feature, unless the operator does not enforce SF1
    /// or excepts the core bidder. A basic-enforcement bidder is never
    /// measured against the GVL, so its declaration is not required.
    fn pass_geo(&self, core_name: &str, vendor_id: u16) -> bool {
        let excepted = self.sf1_effective.as_ref().is_some_and(|sf1| {
            sf1.vendor_exceptions
                .iter()
                .any(|listed| listed.as_str() == core_name)
        });
        if !self.sf1_enforced || excepted {
            return true;
        }
        let opted_in = self
            .tcf
            .as_deref()
            .is_some_and(|tcf| tcf.special_feature_optins.contains(&SPECIAL_FEATURE_GEO));
        let declared = self
            .gvl
            .vendor(vendor_id)
            .is_some_and(|vendor| vendor.special_features.contains(&SPECIAL_FEATURE_GEO))
            || self.basic_enforcement_vendors.contains(core_name);
        opted_in && declared
    }

    /// Whether CCPA applies before per-bidder no-sale exemptions.
    /// Use `bidder_decision` to include those exemptions.
    #[must_use]
    pub fn ccpa_should_enforce(&self) -> bool {
        self.independent.ccpa
    }

    /// Drains the accumulated non-fatal warnings. Called by
    /// `run_auction` once per auction, so a second call is empty.
    #[must_use]
    pub fn take_warnings(&mut self) -> Vec<auctioneer_openrtb::NonFatalWarning> {
        mem::take(&mut self.warnings)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use auctioneer_config::types::{
        AccountConfig, EnforceAlgo, HostConfig, PurposeEnforcement, SpecialFeatureEnforcement,
        TriState,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::inventory::Site;
    use auctioneer_openrtb::types::regs::Regs;
    use auctioneer_openrtb::types::user::User;
    use time::OffsetDateTime;
    use time::macros::datetime;

    use super::{
        Activity, ActivityConfigs, AuctionPermissions, Component, PrivacyContext, TcfString,
    };
    use crate::decision::IndependentPolicies;
    use crate::gcl::{CmpRecord, Gcl};
    use crate::gvl::{Gvl, GvlStore};
    use crate::testkit::{
        AUTHORIZATION_NOW as NOW, CMP_ID, SPEC, TcfSpec, VendorSpec, all_purposes, gvl,
    };

    /// Synthetic permission inputs; vendor 32 also matches the committed
    /// TC strings used by the resolution-path tests below.
    const CORE: &str = "fake_core";
    const CORE_ALIAS: &str = "fake_bidder";
    const CORE_VENDOR: u16 = 32;
    const OTHER_CORE: &str = "fake_other";
    const NO_GVL_BIDDER: &str = "fake_no_gvl";
    const ALIAS: &str = "fake_myalias";
    const ALIAS_VENDOR: u16 = 42;
    /// A vendor the live GVL carries as a tombstone.
    const DELETED_VENDOR: u16 = 688;
    /// The revision the committed TC-string fixtures declare.
    const LIST: u32 = 348;

    /// An enforcing context assembled field by field, so each permission
    /// test states exactly the consent, configuration and vendor list its
    /// outcome depends on. `p2_enforced` and `sf1_enforced` are derived
    /// the way the resolver derives them, never set independently.
    struct Ctx {
        tcf: Option<TcfString>,
        gvl: Gvl,
        purposes: BTreeMap<u8, PurposeEnforcement>,
        basic_enforcement_vendors: BTreeSet<String>,
        alias_gvl_ids: BTreeMap<Box<str>, u32>,
        sf1: Option<SpecialFeatureEnforcement>,
        consent_usable: bool,
        now: OffsetDateTime,
    }

    impl Default for Ctx {
        fn default() -> Self {
            Self {
                tcf: None,
                gvl: gvl(LIST, &[]),
                purposes: BTreeMap::new(),
                basic_enforcement_vendors: BTreeSet::new(),
                alias_gvl_ids: alias_ids(&[(CORE_ALIAS, u32::from(CORE_VENDOR))]),
                sf1: None,
                consent_usable: true,
                now: NOW,
            }
        }
    }

    impl Ctx {
        fn build(self) -> PrivacyContext {
            let p2_enforced = self
                .purposes
                .get(&2)
                .is_some_and(|cfg| cfg.enforce_purpose == TriState::Enabled);
            let sf1_enforced = self
                .sf1
                .as_ref()
                .is_some_and(|sf1| sf1.enforce == TriState::Enabled);
            PrivacyContext {
                activities: ActivityConfigs::default(),
                independent: IndependentPolicies::default(),
                warnings: Vec::new(),
                per_purpose_effective: self.purposes,
                enforcement_active: true,
                consent_usable: self.consent_usable,
                authorization_now: self.now,
                p2_enforced,
                sf1_enforced,
                tcf: self.tcf.map(Box::new),
                gvl: self.gvl,
                alias_gvl_ids: self.alias_gvl_ids,
                basic_enforcement_vendors: self.basic_enforcement_vendors,
                sf1_effective: self.sf1,
            }
        }
    }

    /// The three gates, whichever variant carries them.
    fn gates(permissions: &AuctionPermissions) -> (bool, bool, bool) {
        match *permissions {
            AuctionPermissions::AllowAll => (true, true, true),
            AuctionPermissions::DenyBidRequest => (false, false, false),
            AuctionPermissions::Partial {
                allow_bid_request,
                pass_id,
                pass_geo,
            } => (allow_bid_request, pass_id, pass_geo),
        }
    }

    fn allow_bid_request(ctx: &PrivacyContext, core: &str, bidder: &str) -> bool {
        gates(&ctx.permissions(core, bidder)).0
    }

    fn pass_id(ctx: &PrivacyContext, core: &str, bidder: &str) -> bool {
        gates(&ctx.permissions(core, bidder)).1
    }

    fn pass_geo(ctx: &PrivacyContext, core: &str, bidder: &str) -> bool {
        gates(&ctx.permissions(core, bidder)).2
    }

    fn purpose(
        enforce_algo: EnforceAlgo,
        enforce_purpose: TriState,
        enforce_vendors: TriState,
        vendor_exceptions: &[&str],
    ) -> PurposeEnforcement {
        PurposeEnforcement {
            enforce_algo,
            enforce_purpose,
            enforce_vendors,
            vendor_exceptions: vendor_exceptions
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        }
    }

    /// Purposes 1-10 under full enforcement of both legs, the shape most
    /// permission assertions want.
    fn enforced_purposes() -> BTreeMap<u8, PurposeEnforcement> {
        (1_u8..=10)
            .map(|id| {
                (
                    id,
                    purpose(EnforceAlgo::Full, TriState::Enabled, TriState::Enabled, &[]),
                )
            })
            .collect()
    }

    fn one_purpose(purpose_id: u8, cfg: PurposeEnforcement) -> BTreeMap<u8, PurposeEnforcement> {
        BTreeMap::from([(purpose_id, cfg)])
    }

    fn sf1_config(enforce: TriState, vendor_exceptions: &[&str]) -> SpecialFeatureEnforcement {
        SpecialFeatureEnforcement {
            enforce,
            vendor_exceptions: vendor_exceptions
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        }
    }

    fn names(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    fn alias_ids(entries: &[(&str, u32)]) -> BTreeMap<Box<str>, u32> {
        entries
            .iter()
            .map(|(name, id)| ((*name).into(), *id))
            .collect()
    }

    #[test]
    fn synthetic_alias_id_resolves_without_a_baked_core() {
        let ctx = Ctx::default().build();
        assert!(auctioneer_config::bidder(CORE).is_none());
        assert!(auctioneer_config::bidder(NO_GVL_BIDDER).is_none());
        assert_eq!(ctx.vendor_id(CORE, CORE_ALIAS), CORE_VENDOR);
        assert_eq!(ctx.vendor_id(CORE, CORE), 0);
    }

    #[test]
    fn context_debug_reports_structure_without_consent_or_request_values() {
        let mut ctx = Ctx {
            tcf: Some(TcfSpec::default().build()),
            alias_gvl_ids: alias_ids(&[("fake_private_alias_value", u32::from(ALIAS_VENDOR))]),
            ..Ctx::default()
        }
        .build();
        ctx.warnings.push(auctioneer_openrtb::NonFatalWarning {
            code: auctioneer_openrtb::WarningCode::InvalidPrivacyConsent,
            message: "private-warning-value".to_owned(),
            scope: None,
        });
        let rendered = format!("{ctx:?}");
        for private in [
            "fake_private_alias_value",
            "private-warning-value",
            "purposes_consent",
            "vendor_consent",
            "publisher_restrictions",
            "TcfString",
            "cmp_id",
        ] {
            assert!(!rendered.contains(private), "exposed {private} in Debug");
        }
        assert!(rendered.contains("PrivacyContext"));
        assert!(rendered.contains("has_tcf: true"));
        assert!(rendered.contains("warning_count: 1"));
        assert!(rendered.contains("alias_count: 1"));
    }

    /// A vendor declaring purposes 1-10 under consent plus special
    /// feature 1, so it can satisfy every gate.
    fn full_vendor(id: u16) -> VendorSpec {
        VendorSpec {
            id,
            purposes: all_purposes(),
            special_features: vec![1],
            ..VendorSpec::default()
        }
    }

    /// A TC string granting purposes 1-10, special feature 1, and vendor
    /// consent to the given ids.
    fn full_consent(vendors: &[u16]) -> TcfString {
        TcfSpec {
            vendor_list_version: 348,
            purposes_consent: all_purposes(),
            vendor_consent: vendors.to_vec(),
            special_feature_optins: vec![1],
            ..TcfSpec::default()
        }
        .build()
    }

    // ─────────────── the resolution path, for the tests whose ───────────────
    // ─────────────── subject is a context resolve produced   ───────────────

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/data/tcf_v2/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("fixture {path} unreadable: {err}"))
            .trim_end()
            .to_owned()
    }

    /// Host with GDPR on, TCF2 on, purposes 1-10 enforced, and a
    /// `default_value` of `0` so an ambiguous wire signal stays out of
    /// scope unless the request says otherwise.
    fn host_config() -> HostConfig {
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.default_value = "0".to_owned();
        host.gdpr.tcf2.enabled = true;
        host.gdpr.tcf2.purposes = enforced_purposes();
        host
    }

    fn store_with_pair() -> GvlStore {
        GvlStore::for_test(vec![(SPEC, LIST, gvl(LIST, &[full_vendor(CORE_VENDOR)]))])
    }

    fn resolve_with(req: &BidRequest, account: &AccountConfig, store: &GvlStore) -> PrivacyContext {
        let gcl = Gcl::for_test(
            vec![CmpRecord {
                id: CMP_ID,
                deleted_date: None,
            }],
            datetime!(2027-01-01 0:00:00 UTC),
        );
        PrivacyContext::resolve(
            req,
            account,
            &host_config(),
            store,
            &gcl,
            NOW,
            crate::ResolveOptions::default(),
        )
        .expect("legal regs.gdpr")
        .context
    }

    /// A site request carrying `regs.gdpr = 1`, so the wire signal is
    /// `Yes` and enforcement is active.
    fn applies_request(consent: Option<&str>) -> BidRequest {
        BidRequest {
            id: "req-1".to_owned(),
            site: Some(Site::default()),
            regs: Some(Regs {
                gdpr: Some(1_i32),
                ..Regs::default()
            }),
            user: consent.map(|raw| User {
                consent: Some(raw.to_owned()),
                ..User::default()
            }),
            ..BidRequest::default()
        }
    }

    fn account_with_exception(purpose_id: u8, bidder: &str) -> AccountConfig {
        let mut account = AccountConfig::default();
        account.gdpr.purposes = one_purpose(
            purpose_id,
            purpose(
                EnforceAlgo::Full,
                TriState::Enabled,
                TriState::Enabled,
                &[bidder],
            ),
        );
        account
    }

    #[test]
    fn empty_rules_allow_every_activity() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        assert!(ctx.activity_allow(Activity::FetchBids, Component::Bidder("fake_alias")));
        assert!(ctx.activity_allow(Activity::SyncUser, Component::General("host")));
        assert!(ctx.activity_allow(Activity::ReportAnalytics, Component::Analytics("analytics")));
    }

    #[test]
    fn explicit_permissive_context_does_not_enforce_ccpa() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        assert!(!ctx.ccpa_should_enforce());
    }

    #[test]
    fn explicit_permissive_context_has_no_warnings() {
        let mut ctx = PrivacyContext::allow_all_no_scrub();
        assert!(ctx.take_warnings().is_empty());
        assert!(ctx.take_warnings().is_empty()); // drain idempotent
    }

    // ───────── unusable consent beats every grant that skips consent ─────────

    #[test]
    fn unusable_consent_denies_pass_id_even_for_vendor_exception() {
        // The bidder is excepted for purpose 2, which normally bypasses
        // both the GVL declaration check and the consent bits. Each of
        // the three ways consent becomes unusable must still deny.
        let account = account_with_exception(2, CORE);
        let empty_store = GvlStore::for_test(Vec::new());

        let gvl_missing = resolve_with(
            &applies_request(Some(&fixture("valid_full_consent.txt"))),
            &account,
            &empty_store,
        );
        let malformed = resolve_with(
            &applies_request(Some("~~~not-a-tc-string~~~")),
            &account,
            &store_with_pair(),
        );
        let empty_consent = resolve_with(&applies_request(Some("")), &account, &store_with_pair());

        for ctx in [&gvl_missing, &malformed, &empty_consent] {
            assert!(!ctx.consent_usable);
            assert!(!pass_id(ctx, CORE, CORE));
        }
    }

    #[test]
    fn unusable_consent_denies_pass_id_for_basic_enforcement_vendor() {
        // The outcome, not the mechanism: with no TC string the basic
        // table denies on its own, so what this pins is that a
        // basic-enforcement bidder lands on the default row's
        // `pass_id: false`. That the short-circuit is what gets it
        // there is pinned by `unusable_consent_row_is_p2_and_sf1_only`,
        // whose context carries a TC string the walk would otherwise
        // read.
        let mut account = AccountConfig::default();
        account.gdpr.basic_enforcement_vendors = vec![CORE.to_owned()];
        let ctx = resolve_with(
            &applies_request(Some(&fixture("valid_full_consent.txt"))),
            &account,
            &GvlStore::for_test(Vec::new()),
        );
        assert!(!ctx.consent_usable);
        assert!(!pass_id(&ctx, CORE, CORE));
    }

    #[test]
    fn stale_policy_version_gvl_denies_pass_id() {
        // The archive carries the right list version under the wrong
        // spec, so the revision the string declared is unreachable. The
        // load-bearing assertion is `!consent_usable`; the denied
        // identifier follows from the default row.
        let elsewhere = GvlStore::for_test(vec![(2, LIST, gvl(LIST, &[full_vendor(CORE_VENDOR)]))]);
        let ctx = resolve_with(
            &applies_request(Some(&fixture("valid_full_consent.txt"))),
            &AccountConfig::default(),
            &elsewhere,
        );
        assert!(!ctx.consent_usable);
        assert!(!pass_id(&ctx, CORE, CORE));
    }

    #[test]
    fn pass_id_li_transparency_on_p4_does_not_grant_without_vendor_consent() {
        // Legitimate-interest transparency satisfies basic purpose 2 and
        // nothing else, so an LI bit on a later purpose cannot stand in
        // for vendor consent.
        for purpose_id in [4_u8, 7] {
            let ctx = Ctx {
                tcf: Some(
                    TcfSpec {
                        purposes_li_transparency: vec![purpose_id],
                        ..TcfSpec::default()
                    }
                    .build(),
                ),
                purposes: enforced_purposes(),
                basic_enforcement_vendors: names(&[CORE]),
                ..Ctx::default()
            }
            .build();
            assert!(!pass_id(&ctx, CORE, CORE_ALIAS), "purpose {purpose_id}");
        }
    }

    #[test]
    fn pass_id_basic_p2_li_transparency_grants() {
        let ctx = Ctx {
            tcf: Some(
                TcfSpec {
                    purposes_li_transparency: vec![2],
                    ..TcfSpec::default()
                }
                .build(),
            ),
            purposes: enforced_purposes(),
            basic_enforcement_vendors: names(&[CORE]),
            ..Ctx::default()
        }
        .build();
        assert!(pass_id(&ctx, CORE, CORE_ALIAS));
        // The bid gate is basic Purpose 2 too, so the same LI bit
        // satisfies it without any vendor consent.
        assert!(allow_bid_request(&ctx, CORE, CORE_ALIAS));
    }

    #[test]
    fn pass_geo_ignores_alias_name_in_sf1_vendor_exceptions() {
        // The exception list is keyed by the core bidder, so an alias
        // whose name collides with an entry inherits nothing.
        let build = |exception: &str| {
            Ctx {
                tcf: Some(TcfSpec::default().build()),
                gvl: gvl(
                    LIST,
                    &[VendorSpec::consenting(CORE_VENDOR, &all_purposes())],
                ),
                purposes: enforced_purposes(),
                sf1: Some(sf1_config(TriState::Enabled, &[exception])),
                alias_gvl_ids: alias_ids(&[(ALIAS, u32::from(CORE_VENDOR))]),
                ..Ctx::default()
            }
            .build()
        };
        assert_eq!(build(ALIAS).vendor_id(CORE, ALIAS), CORE_VENDOR);
        assert_eq!(build(CORE).vendor_id(CORE, ALIAS), CORE_VENDOR);
        assert!(!pass_geo(&build(ALIAS), CORE, ALIAS));
        assert!(pass_geo(&build(CORE), CORE, ALIAS));
    }

    #[test]
    fn alias_name_collision_does_not_inherit_core_vendor_exceptions() {
        let build = |exception: &str| {
            Ctx {
                tcf: Some(
                    TcfSpec {
                        purposes_consent: all_purposes(),
                        ..TcfSpec::default()
                    }
                    .build(),
                ),
                gvl: gvl(
                    LIST,
                    &[VendorSpec::consenting(CORE_VENDOR, &all_purposes())],
                ),
                purposes: one_purpose(
                    2,
                    purpose(
                        EnforceAlgo::Full,
                        TriState::Enabled,
                        TriState::Enabled,
                        &[exception],
                    ),
                ),
                alias_gvl_ids: alias_ids(&[(ALIAS, u32::from(CORE_VENDOR))]),
                ..Ctx::default()
            }
            .build()
        };
        assert_eq!(build(ALIAS).vendor_id(CORE, ALIAS), CORE_VENDOR);
        assert_eq!(build(CORE).vendor_id(CORE, ALIAS), CORE_VENDOR);
        assert!(!allow_bid_request(&build(ALIAS), CORE, ALIAS));
        assert!(allow_bid_request(&build(CORE), CORE, ALIAS));
    }

    #[test]
    fn unusable_consent_row_is_p2_and_sf1_only() {
        for (p2, sf1) in [
            (TriState::Enabled, TriState::Enabled),
            (TriState::Enabled, TriState::Disabled),
            (TriState::Disabled, TriState::Enabled),
            (TriState::Disabled, TriState::Disabled),
        ] {
            let ctx = Ctx {
                consent_usable: false,
                purposes: one_purpose(
                    2,
                    purpose(EnforceAlgo::Full, p2, TriState::Enabled, &[CORE]),
                ),
                basic_enforcement_vendors: names(&[CORE]),
                sf1: Some(sf1_config(sf1, &[])),
                ..Ctx::default()
            }
            .build();
            let expected = AuctionPermissions::Partial {
                allow_bid_request: p2 != TriState::Enabled,
                pass_id: false,
                pass_geo: sf1 != TriState::Enabled,
            };
            // The excepted, basic-enforced bidder and an unrelated one
            // get the identical row: this is a per-request answer.
            assert_eq!(ctx.permissions(CORE, CORE), expected);
            assert_eq!(ctx.permissions(OTHER_CORE, OTHER_CORE), expected);
        }
    }

    #[test]
    fn unbaked_pair_fails_closed_to_enforced_but_empty() {
        let mut account = AccountConfig::default();
        account.gdpr.special_features =
            BTreeMap::from([(1_u8, sf1_config(TriState::Disabled, &[]))]);
        let ctx = resolve_with(
            &applies_request(Some(&fixture("valid_full_consent.txt"))),
            &account,
            &GvlStore::for_test(Vec::new()),
        );

        // Purposes are enforced by the host, SF1 is configured off.
        assert!(ctx.p2_enforced);
        assert!(!ctx.sf1_enforced);
        assert_eq!(
            ctx.permissions(CORE, CORE),
            AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                pass_geo: true,
            }
        );
        let warning = ctx.warnings.first().expect("one unavailable-GVL warning");
        assert_eq!(
            warning.code,
            auctioneer_openrtb::WarningCode::GvlVersionUnavailable
        );
    }

    #[test]
    fn unbaked_pair_does_not_allow_all_when_enforcing() {
        let mut account = AccountConfig::default();
        account.gdpr.special_features =
            BTreeMap::from([(1_u8, sf1_config(TriState::Enabled, &[]))]);
        let ctx = resolve_with(
            &applies_request(Some(&fixture("valid_full_consent.txt"))),
            &account,
            &GvlStore::for_test(Vec::new()),
        );
        assert!(ctx.p2_enforced);
        assert!(ctx.sf1_enforced);
        assert_eq!(
            ctx.permissions(CORE, CORE),
            AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                pass_geo: false,
            }
        );
    }

    // ───────── the per-bidder walk ─────────

    #[test]
    fn alias_gvl_id_override_with_vendor_consent_yields_allow_all() {
        let ctx = Ctx {
            tcf: Some(full_consent(&[ALIAS_VENDOR])),
            gvl: gvl(LIST, &[full_vendor(ALIAS_VENDOR)]),
            purposes: enforced_purposes(),
            alias_gvl_ids: alias_ids(&[(ALIAS, u32::from(ALIAS_VENDOR))]),
            sf1: Some(sf1_config(TriState::Enabled, &[])),
            ..Ctx::default()
        }
        .build();
        assert_eq!(ctx.permissions(CORE, ALIAS), AuctionPermissions::AllowAll);
    }

    #[test]
    fn alias_gvl_id_above_u16_max_normalises_to_the_unknown_vendor() {
        // The exchange rejects this input with a 400 and the resolver
        // clamps it, but the permission path is reached directly by a
        // caller that skips alias resolution, so its own guard has to
        // hold. Truncating 0x1_0020 would authorize against vendor 32.
        let ctx = Ctx {
            tcf: Some(full_consent(&[CORE_VENDOR])),
            gvl: gvl(LIST, &[full_vendor(CORE_VENDOR)]),
            purposes: enforced_purposes(),
            alias_gvl_ids: alias_ids(&[(ALIAS, 0x0001_0020)]),
            ..Ctx::default()
        }
        .build();
        assert!(!allow_bid_request(&ctx, CORE, ALIAS));
        assert!(!pass_id(&ctx, CORE, ALIAS));
    }

    #[test]
    fn vendor_id_zero_under_full_enforcement_denied() {
        let ctx = Ctx {
            tcf: Some(full_consent(&[])),
            gvl: gvl(LIST, &[]),
            purposes: enforced_purposes(),
            ..Ctx::default()
        }
        .build();
        assert!(!allow_bid_request(&ctx, NO_GVL_BIDDER, NO_GVL_BIDDER));
    }

    #[test]
    fn vendor_id_zero_basic_with_purpose_two_vendor_exception_allowed() {
        // The exception row fires before basic enforcement reaches its
        // closing vendor-consent row, which id 0 could never satisfy.
        let ctx = Ctx {
            tcf: Some(full_consent(&[])),
            purposes: one_purpose(
                2,
                purpose(
                    EnforceAlgo::Basic,
                    TriState::Enabled,
                    TriState::Enabled,
                    &[NO_GVL_BIDDER],
                ),
            ),
            ..Ctx::default()
        }
        .build();
        assert!(allow_bid_request(&ctx, NO_GVL_BIDDER, NO_GVL_BIDDER));
    }

    #[test]
    fn vendor_id_zero_basic_without_exception_or_li_denied() {
        let ctx = Ctx {
            tcf: Some(
                TcfSpec {
                    purposes_consent: vec![2],
                    ..TcfSpec::default()
                }
                .build(),
            ),
            purposes: one_purpose(
                2,
                purpose(
                    EnforceAlgo::Basic,
                    TriState::Enabled,
                    TriState::Enabled,
                    &[],
                ),
            ),
            ..Ctx::default()
        }
        .build();
        assert!(!allow_bid_request(&ctx, NO_GVL_BIDDER, NO_GVL_BIDDER));
    }

    #[test]
    fn vendor_id_zero_basic_with_vendors_unenforced_allowed() {
        // Vendor enforcement off skips the closing row entirely.
        let ctx = Ctx {
            tcf: Some(
                TcfSpec {
                    purposes_consent: vec![2],
                    ..TcfSpec::default()
                }
                .build(),
            ),
            purposes: one_purpose(
                2,
                purpose(
                    EnforceAlgo::Basic,
                    TriState::Enabled,
                    TriState::Disabled,
                    &[],
                ),
            ),
            ..Ctx::default()
        }
        .build();
        assert!(allow_bid_request(&ctx, NO_GVL_BIDDER, NO_GVL_BIDDER));
    }

    #[test]
    fn basic_enforcement_vendor_downgrades_to_basic_algorithm() {
        // The vendor is absent from the GVL, so full enforcement denies
        // and only the downgrade can allow.
        let build = |basic: &[&str]| {
            Ctx {
                tcf: Some(
                    TcfSpec {
                        purposes_consent: vec![2],
                        ..TcfSpec::default()
                    }
                    .build(),
                ),
                gvl: gvl(LIST, &[]),
                purposes: enforced_purposes(),
                basic_enforcement_vendors: names(basic),
                ..Ctx::default()
            }
            .build()
        };
        assert!(!allow_bid_request(&build(&[]), CORE, CORE_ALIAS));
        assert!(allow_bid_request(&build(&[CORE]), CORE, CORE_ALIAS));
    }

    #[test]
    fn purpose_two_enforced_without_consent_denies_bid_request() {
        let ctx = Ctx {
            tcf: Some(
                TcfSpec {
                    vendor_consent: vec![CORE_VENDOR],
                    ..TcfSpec::default()
                }
                .build(),
            ),
            gvl: gvl(LIST, &[full_vendor(CORE_VENDOR)]),
            purposes: enforced_purposes(),
            ..Ctx::default()
        }
        .build();
        assert!(!allow_bid_request(&ctx, CORE, CORE_ALIAS));
    }

    #[test]
    fn purpose_two_unenforced_allows_bid_request() {
        // Neither leg enforced: no consent, no vendor id, still allowed.
        let ctx = Ctx {
            purposes: one_purpose(
                2,
                purpose(
                    EnforceAlgo::Full,
                    TriState::Disabled,
                    TriState::Disabled,
                    &[],
                ),
            ),
            ..Ctx::default()
        }
        .build();
        assert!(allow_bid_request(&ctx, NO_GVL_BIDDER, NO_GVL_BIDDER));
        assert!(allow_bid_request(&ctx, CORE, CORE_ALIAS));
    }

    #[test]
    fn special_feature_one_enforced_without_optin_denies_pass_geo() {
        let build = |optins: Vec<u8>, declares_sf1: bool, basic: &[&str]| {
            let vendor = if declares_sf1 {
                full_vendor(CORE_VENDOR)
            } else {
                VendorSpec::consenting(CORE_VENDOR, &all_purposes())
            };
            Ctx {
                tcf: Some(
                    TcfSpec {
                        special_feature_optins: optins,
                        ..TcfSpec::default()
                    }
                    .build(),
                ),
                gvl: gvl(LIST, &[vendor]),
                purposes: enforced_purposes(),
                basic_enforcement_vendors: names(basic),
                sf1: Some(sf1_config(TriState::Enabled, &[])),
                ..Ctx::default()
            }
            .build()
        };
        assert!(!pass_geo(&build(Vec::new(), true, &[]), CORE, CORE_ALIAS));
        assert!(pass_geo(&build(vec![1], true, &[]), CORE, CORE_ALIAS));

        // The opt-in alone is not enough: geo also needs the vendor to
        // have declared the feature, unless basic enforcement exempts it
        // from the GVL entirely.
        assert!(!pass_geo(&build(vec![1], false, &[]), CORE, CORE_ALIAS));
        assert!(pass_geo(&build(vec![1], false, &[CORE]), CORE, CORE_ALIAS));
        assert!(!pass_geo(
            &build(Vec::new(), false, &[CORE]),
            CORE,
            CORE_ALIAS
        ));
    }

    #[test]
    fn purpose_two_vendor_exception_treats_consent_as_given() {
        let ctx = Ctx {
            tcf: Some(full_consent(&[])),
            gvl: gvl(LIST, &[full_vendor(CORE_VENDOR)]),
            purposes: one_purpose(
                2,
                purpose(
                    EnforceAlgo::Full,
                    TriState::Enabled,
                    TriState::Enabled,
                    &[CORE],
                ),
            ),
            ..Ctx::default()
        }
        .build();
        assert!(allow_bid_request(&ctx, CORE, CORE_ALIAS));
    }

    #[test]
    fn allow_all_no_scrub_yields_allow_all_for_every_bidder() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        for bidder in [
            CORE,
            OTHER_CORE,
            NO_GVL_BIDDER,
            "fake_extra",
            "fake_unknown",
        ] {
            assert_eq!(
                ctx.permissions(bidder, bidder),
                AuctionPermissions::AllowAll
            );
            assert_eq!(ctx.permissions(bidder, ALIAS), AuctionPermissions::AllowAll);
        }
    }

    #[test]
    fn signal_no_context_yields_allow_all_for_every_bidder() {
        let out_of_scope = BidRequest {
            regs: Some(Regs {
                gdpr: Some(0_i32),
                ..Regs::default()
            }),
            ..applies_request(Some(&fixture("valid_full_consent.txt")))
        };
        let ctx = resolve_with(&out_of_scope, &AccountConfig::default(), &store_with_pair());
        assert!(!ctx.enforcement_active);
        for bidder in [CORE, OTHER_CORE, NO_GVL_BIDDER, "fake_unknown"] {
            assert_eq!(
                ctx.permissions(bidder, bidder),
                AuctionPermissions::AllowAll
            );
        }
    }

    #[test]
    fn out_of_scope_allows_all_where_unusable_consent_takes_the_default_row() {
        // The two states share `consent_usable == true`/`tcf == None`
        // shapes closely enough that keying on either would conflate
        // them. Enforcement is what separates allow-all from the
        // fail-closed row, and it is enforced traffic that must not
        // benefit from an absent TC string.
        let host_off = {
            let mut host = host_config();
            host.gdpr.enabled = false;
            let gcl = Gcl::for_test(
                vec![CmpRecord {
                    id: CMP_ID,
                    deleted_date: None,
                }],
                datetime!(2027-01-01 0:00:00 UTC),
            );
            PrivacyContext::resolve(
                &applies_request(None),
                &AccountConfig::default(),
                &host,
                &store_with_pair(),
                &gcl,
                NOW,
                crate::ResolveOptions::default(),
            )
            .expect("legal regs.gdpr")
            .context
        };
        let enforced = resolve_with(
            &applies_request(None),
            &AccountConfig::default(),
            &store_with_pair(),
        );

        assert!(!host_off.enforcement_active);
        assert!(host_off.tcf.is_none());
        assert_eq!(
            host_off.permissions(CORE, CORE),
            AuctionPermissions::AllowAll
        );

        assert!(enforced.enforcement_active);
        assert!(!enforced.consent_usable);
        assert!(enforced.tcf.is_none());
        assert_eq!(
            enforced.permissions(CORE, CORE),
            AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                pass_geo: true,
            }
        );
    }

    #[test]
    fn pass_id_enforces_even_when_purposes_are_unenforced_by_config() {
        let unenforced: BTreeMap<u8, PurposeEnforcement> = (2_u8..=10)
            .map(|id| {
                (
                    id,
                    purpose(
                        EnforceAlgo::Full,
                        TriState::Disabled,
                        TriState::Disabled,
                        &[],
                    ),
                )
            })
            .collect();
        let build = |consented: Vec<u8>| {
            Ctx {
                tcf: Some(
                    TcfSpec {
                        purposes_consent: consented,
                        vendor_consent: vec![CORE_VENDOR],
                        ..TcfSpec::default()
                    }
                    .build(),
                ),
                gvl: gvl(LIST, &[full_vendor(CORE_VENDOR)]),
                purposes: unenforced.clone(),
                ..Ctx::default()
            }
            .build()
        };
        assert!(pass_id(&build(vec![4]), CORE, CORE_ALIAS));
        assert!(!pass_id(&build(Vec::new()), CORE, CORE_ALIAS));
    }

    #[test]
    fn pass_id_respects_vendor_exceptions() {
        let ctx = Ctx {
            tcf: Some(full_consent(&[])),
            gvl: gvl(LIST, &[full_vendor(CORE_VENDOR)]),
            purposes: one_purpose(
                4,
                purpose(
                    EnforceAlgo::Full,
                    TriState::Enabled,
                    TriState::Enabled,
                    &[CORE],
                ),
            ),
            ..Ctx::default()
        }
        .build();
        assert!(pass_id(&ctx, CORE, CORE_ALIAS));
    }

    #[test]
    fn pass_id_denies_when_vendor_id_is_zero() {
        // Purpose 2 is unenforced, so the bid gate opens; identity still
        // needs a vendor to name.
        let ctx = Ctx {
            tcf: Some(full_consent(&[])),
            purposes: (2_u8..=10)
                .map(|id| {
                    (
                        id,
                        purpose(
                            EnforceAlgo::Full,
                            TriState::Disabled,
                            TriState::Disabled,
                            &[],
                        ),
                    )
                })
                .collect(),
            ..Ctx::default()
        }
        .build();
        assert!(allow_bid_request(&ctx, NO_GVL_BIDDER, NO_GVL_BIDDER));
        assert!(!pass_id(&ctx, NO_GVL_BIDDER, NO_GVL_BIDDER));
    }

    // ───────── the deletion boundary ─────────

    #[test]
    fn future_deleted_vendor_not_denied_until_date() {
        // One vendor's deletion lands tomorrow, the other's has passed.
        let ctx = Ctx {
            tcf: Some(full_consent(&[CORE_VENDOR, DELETED_VENDOR])),
            gvl: gvl(
                LIST,
                &[
                    VendorSpec {
                        deleted_date: Some("2026-08-30T12:00:00Z"),
                        ..full_vendor(CORE_VENDOR)
                    },
                    VendorSpec {
                        deleted_date: Some("2026-06-01T00:00:00Z"),
                        ..full_vendor(DELETED_VENDOR)
                    },
                ],
            ),
            purposes: enforced_purposes(),
            alias_gvl_ids: alias_ids(&[
                (CORE_ALIAS, u32::from(CORE_VENDOR)),
                (ALIAS, u32::from(DELETED_VENDOR)),
            ]),
            ..Ctx::default()
        }
        .build();
        assert_eq!(
            ctx.permissions(CORE, CORE_ALIAS),
            AuctionPermissions::AllowAll
        );
        assert_eq!(
            ctx.permissions(CORE, ALIAS),
            AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                pass_geo: false,
            }
        );
    }

    #[test]
    fn vendor_deleted_at_equal_now_denied() {
        // The comparator denies at equality, matching the CMP rule.
        let ctx = Ctx {
            tcf: Some(full_consent(&[CORE_VENDOR])),
            gvl: gvl(
                LIST,
                &[VendorSpec {
                    deleted_date: Some("2026-08-29T12:00:00Z"),
                    ..full_vendor(CORE_VENDOR)
                }],
            ),
            purposes: enforced_purposes(),
            ..Ctx::default()
        }
        .build();
        assert_eq!(ctx.authorization_now, NOW);
        assert_eq!(ctx.vendor_id(CORE, CORE_ALIAS), CORE_VENDOR);
        assert_eq!(
            ctx.permissions(CORE, CORE_ALIAS),
            AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                pass_geo: false,
            }
        );
    }

    /// Exercise real baked core-name lookup with metadata-selected inputs.
    /// Consent grants and synthetic GVL declarations remain independent expectations.
    mod baked_lookup_conformance {
        use super::*;

        fn baked_vendor_excluding(excluded: &[u16]) -> Option<(&'static str, u16)> {
            auctioneer_config::bidders().iter().find_map(|info| {
                let id = u16::try_from(info.gvl_vendor_id?).ok()?;
                if [0, ALIAS_VENDOR, DELETED_VENDOR].contains(&id) || excluded.contains(&id) {
                    None
                } else {
                    Some((info.name.as_str(), id))
                }
            })
        }

        #[test]
        fn core_vendor_id_and_missing_id_match_shipped_metadata() {
            let ctx = Ctx::default().build();
            let metadata = auctioneer_config::bidders();
            assert!(metadata.iter().any(|info| info.gvl_vendor_id.is_some()));
            assert!(metadata.iter().any(|info| info.gvl_vendor_id.is_none()));
            for info in metadata {
                let expected = match info.gvl_vendor_id {
                    Some(id) => u16::try_from(id).unwrap(),
                    None => 0,
                };
                assert_eq!(ctx.vendor_id(&info.name, &info.name), expected);
            }
            assert_eq!(ctx.vendor_id(NO_GVL_BIDDER, NO_GVL_BIDDER), 0);
        }

        #[test]
        fn base_bidder_with_vendor_consent_yields_allow_all() {
            let (core, vendor_id) = baked_vendor_excluding(&[]).unwrap();
            let ctx = Ctx {
                tcf: Some(full_consent(&[vendor_id])),
                gvl: gvl(LIST, &[full_vendor(vendor_id)]),
                purposes: enforced_purposes(),
                sf1: Some(sf1_config(TriState::Enabled, &[])),
                ..Ctx::default()
            }
            .build();
            assert_eq!(ctx.permissions(core, core), AuctionPermissions::AllowAll);
        }

        #[test]
        fn base_bidder_without_vendor_consent_denies_bid_request() {
            let (core, vendor_id) = baked_vendor_excluding(&[]).unwrap();
            let ctx = Ctx {
                tcf: Some(full_consent(&[])),
                gvl: gvl(LIST, &[full_vendor(vendor_id)]),
                purposes: enforced_purposes(),
                ..Ctx::default()
            }
            .build();
            assert!(matches!(
                ctx.permissions(core, core),
                AuctionPermissions::Partial {
                    allow_bid_request: false,
                    ..
                }
            ));
        }

        #[test]
        fn alias_without_gvl_id_entry_inherits_core_vendor_id() {
            let (core, vendor_id) = baked_vendor_excluding(&[]).unwrap();
            let build = |consented: u16| {
                Ctx {
                    tcf: Some(full_consent(&[consented])),
                    gvl: gvl(LIST, &[full_vendor(vendor_id), full_vendor(ALIAS_VENDOR)]),
                    purposes: enforced_purposes(),
                    sf1: Some(sf1_config(TriState::Enabled, &[])),
                    ..Ctx::default()
                }
                .build()
            };
            assert_eq!(
                build(vendor_id).permissions(core, ALIAS),
                AuctionPermissions::AllowAll
            );
            // Consent for some other vendor does not reach the alias,
            // which is authorized as its base.
            assert!(!allow_bid_request(&build(ALIAS_VENDOR), core, ALIAS));
        }

        #[test]
        fn alias_name_matching_a_core_bidder_uses_the_alias_vendor_id() {
            let (core, vendor_id) = baked_vendor_excluding(&[]).unwrap();
            let (other_core, other_vendor_id) = baked_vendor_excluding(&[vendor_id]).unwrap();
            assert_ne!(core, other_core);
            // The other name is itself a baked core, but as an alias
            // its own aliasgvlids entry governs.
            let build = |consented: u16| {
                Ctx {
                    tcf: Some(full_consent(&[consented])),
                    gvl: gvl(
                        LIST,
                        &[
                            full_vendor(vendor_id),
                            full_vendor(ALIAS_VENDOR),
                            full_vendor(other_vendor_id),
                        ],
                    ),
                    purposes: enforced_purposes(),
                    alias_gvl_ids: alias_ids(&[(other_core, u32::from(ALIAS_VENDOR))]),
                    ..Ctx::default()
                }
                .build()
            };
            assert!(allow_bid_request(&build(ALIAS_VENDOR), core, other_core));
            assert!(!allow_bid_request(
                &build(other_vendor_id),
                core,
                other_core
            ));
            assert!(!allow_bid_request(&build(vendor_id), core, other_core));
        }

        #[test]
        fn deleted_vendor_denied_through_baked_core_and_alias_even_with_full_consent() {
            let (core, vendor_id) = baked_vendor_excluding(&[]).unwrap();
            // Both lookup shapes: the core's baked id and an explicit
            // alias id pointing at the tombstoned vendor.
            let build = |deleted: Option<&'static str>| {
                let deleted_spec = |id: u16| VendorSpec {
                    deleted_date: deleted,
                    ..full_vendor(id)
                };
                Ctx {
                    tcf: Some(full_consent(&[vendor_id, DELETED_VENDOR])),
                    gvl: gvl(
                        LIST,
                        &[deleted_spec(vendor_id), deleted_spec(DELETED_VENDOR)],
                    ),
                    purposes: enforced_purposes(),
                    alias_gvl_ids: alias_ids(&[(ALIAS, u32::from(DELETED_VENDOR))]),
                    ..Ctx::default()
                }
                .build()
            };

            let denied = AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                pass_geo: false,
            };
            let tombstoned = build(Some("2026-06-01T00:00:00Z"));
            assert_eq!(tombstoned.permissions(core, core), denied);
            assert_eq!(tombstoned.permissions(core, ALIAS), denied);

            // Clearing deletion proves the denial is not a missing grant.
            let active = build(None);
            assert_eq!(active.permissions(core, core), AuctionPermissions::AllowAll);
            assert_eq!(
                active.permissions(core, ALIAS),
                AuctionPermissions::AllowAll
            );
        }
    }
}

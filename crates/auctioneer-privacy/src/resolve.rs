//! GDPR resolution: one `BidRequest` plus host and account configuration
//! become one [`PrivacyContext`].
//!
//! The resolver runs the §7.1 ladder in a fixed order, and the order is
//! load-bearing. The wire signal is computed FIRST so its conflict
//! warning survives every later short-circuit, including the host master
//! switch. Configuration-derived enforcement is resolved BEFORE the
//! consent split, because it does not depend on consent and both the
//! usable and unusable branches need it.
//!
//! Every branch that cannot establish consent converges on
//! `enforced_but_empty`, so an unusable TC string, an unauthentic CMP, a
//! stale CMP list, and an unavailable GVL revision all produce the same
//! fail-closed shape rather than four near-identical ones.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use auctioneer_config::activity::ActivityConfigs;
use auctioneer_config::types::{
    AccountConfig, AccountGdprConfig, ChannelEnabled, HostConfig, PurposeEnforcement,
    SpecialFeatureEnforcement, Tcf2Config, TriState,
};
use auctioneer_openrtb::NonFatalWarning;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::types::device::Geo;
use time::OffsetDateTime;

use crate::auction_context::PrivacyContext;
use crate::ccpa::NoSale;
use crate::decision::IndependentPolicies;
use crate::error::PrivacyError;
use crate::gcl::Gcl;
use crate::gpp::Limits;
use crate::gvl::{Gvl, GvlStore, gvl_spec_for_policy};
use crate::signal::{AuctionGeoInput, GeoView, Signal, auction_geo_signal};
use crate::sources::{self, ConsentSource, LegacyProjection, NormalizedSources, SelectedConsent};
use crate::tcf::{TcfString, parse_tcf_v2};
use crate::warnings::{gcl_stale, gvl_version_unavailable, invalid_privacy_consent};

/// Exchange-owned identity validation and narrow GDPR exemption.
/// The no-sale value must have been checked against the full resolved namespace.
#[derive(Debug, Default)]
pub struct ResolveOptions {
    pub gdpr_exempt: bool,
    pub no_sale: NoSale,
}

/// Owned decisions plus borrowed raw fields for later bidder-local projection.
#[derive(Debug)]
pub struct ResolvedPrivacy<'a> {
    pub context: PrivacyContext,
    pub projection: LegacyProjection<'a>,
}

enum ConsentDiagnostic {
    Malformed,
    UnauthenticCmp,
    StaleGcl,
    UnavailableGvl { spec: u32, list: u32 },
}

impl ConsentDiagnostic {
    fn warning(&self, selected: SelectedConsent<'_>) -> NonFatalWarning {
        match self {
            Self::Malformed => selected.malformed_warning(),
            Self::StaleGcl => gcl_stale(),
            Self::UnauthenticCmp => match selected.source() {
                ConsentSource::Legacy => invalid_privacy_consent(),
                ConsentSource::Gpp => NonFatalWarning {
                    code: auctioneer_openrtb::WarningCode::InvalidPrivacyConsent,
                    scope: Some("regs.gpp".into()),
                    message:
                        "regs.gpp section 2 names an absent or deleted CMP; treating as no consent"
                            .into(),
                },
            },
            Self::UnavailableGvl { spec, list } => match selected.source() {
                ConsentSource::Legacy => gvl_version_unavailable(*spec, *list),
                ConsentSource::Gpp => NonFatalWarning {
                    code: auctioneer_openrtb::WarningCode::GvlVersionUnavailable,
                    scope: Some("regs.gpp".into()),
                    message: format!(
                        "regs.gpp section 2 requires unavailable GVL (spec {spec}, list {list}); applying enforced-but-empty defaults (identifiers denied)"
                    ),
                },
            },
        }
    }
}

/// Special feature 1 is geolocation, the only special feature the
/// enforcement path resolves.
pub(crate) const SPECIAL_FEATURE_GEO: u8 = 1;

/// The policy key for the channel gate.
///
/// Derived from the inventory objects on the request, never from
/// `ext.prebid.channel.name`: that extension is request-controlled, so
/// reading it would let an app request claim `web` and select a
/// disabled-web path to dodge enforcement. `amp` and `video` are
/// route-derived and unreachable on `/openrtb2/auction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    App,
    Dooh,
    Web,
}

/// What the resolver derives from configuration alone.
///
/// Computed once before the consent split and consumed by whichever
/// branch wins, so the usable and unusable contexts can never disagree
/// about what the operator configured.
struct ResolvedEnforcement {
    activities: ActivityConfigs,
    per_purpose_effective: BTreeMap<u8, PurposeEnforcement>,
    sf1_effective: Option<SpecialFeatureEnforcement>,
    p2_enforced: bool,
    sf1_enforced: bool,
    alias_gvl_ids: BTreeMap<Box<str>, u32>,
    basic_enforcement_vendors: BTreeSet<String>,
}

#[expect(
    clippy::multiple_inherent_impl,
    reason = "the resolution ladder is its own module; hosting the method beside \
              `allow_all_no_scrub` would put 200 lines of §7.1 sequencing in the file that \
              defines the type, and splitting the method from the helpers it drives would \
              cost more than the second impl block does"
)]
impl PrivacyContext {
    /// Normalize inputs once and resolve the independent policies for one auction.
    ///
    /// `now` is a value the handler read from its clock, not a clock
    /// dependency: resolve-time checks (CMP deletion, list freshness)
    /// and permission-time checks share the single instant stored on the
    /// context, so one request can never straddle two.
    ///
    /// # Errors
    /// Returns [`PrivacyError::InvalidRegsGdpr`] when `regs.gdpr` carries
    /// a value other than 0 or 1 and `regs.gpp_sid` is absent or empty.
    /// Returns [`PrivacyError::GppLimitExceeded`] for terminal envelope limits,
    /// including when every configurable policy is disabled. The exchange maps
    /// either error to 400 and validates no-sale before calling this method.
    pub fn resolve<'a>(
        req: &'a BidRequest,
        account: &AccountConfig,
        host: &HostConfig,
        gvl_store: &GvlStore,
        gcl: &Gcl,
        now: OffsetDateTime,
        options: ResolveOptions,
    ) -> Result<ResolvedPrivacy<'a>, PrivacyError> {
        let sources = sources::normalize(req, Limits::default())?;
        let projection = sources.projection();
        let (mut context, diagnostic) = Self::resolve_gdpr(
            req,
            account,
            host,
            gvl_store,
            gcl,
            now,
            &sources,
            options.gdpr_exempt,
        );
        context.independent = IndependentPolicies {
            ccpa: ccpa_enabled(account, host, channel_of(req))
                && (sources.usp_is_malformed()
                    || sources.usp().is_some_and(|usp| usp.sale_opt_out())),
            lmt: host.lmt.enforce
                && req
                    .device
                    .as_ref()
                    .is_some_and(|device| device.lmt == Some(1_i32)),
            coppa: req
                .regs
                .as_ref()
                .is_some_and(|regs| regs.coppa == Some(1_i32)),
            no_sale: options.no_sale,
        };
        let (syntax, authenticity) = match diagnostic {
            Some(ConsentDiagnostic::Malformed) => (Some(sources.tcf().malformed_warning()), None),
            Some(reason) => (None, Some(reason.warning(sources.tcf()))),
            None => (None, None),
        };
        context.warnings = sources.into_warnings(syntax, authenticity);
        Ok(ResolvedPrivacy {
            context,
            projection,
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "private GDPR ladder shares the public resolver inputs plus normalized sources and the narrow exemption"
    )]
    fn resolve_gdpr(
        req: &BidRequest,
        account: &AccountConfig,
        host: &HostConfig,
        gvl_store: &GvlStore,
        gcl: &Gcl,
        now: OffsetDateTime,
        sources: &NormalizedSources<'_>,
        gdpr_exempt: bool,
    ) -> (PrivacyContext, Option<ConsentDiagnostic>) {
        if gdpr_exempt || !host.gdpr.enabled {
            return (permissive(account, now), None);
        }

        let geo_signal = auction_geo_signal(&AuctionGeoInput {
            user_geo: geo_view(req.user.as_ref().and_then(|user| user.geo.as_ref())),
            device_geo: geo_view(req.device.as_ref().and_then(|device| device.geo.as_ref())),
            host_default_value: &host.gdpr.default_value,
            eea_countries: auctioneer_config::eea_set(),
        });
        let channel_enabled = channel_enabled(&account.gdpr, host, channel_of(req));

        if !enforcement_active(sources.wire_signal(), geo_signal, channel_enabled) {
            return (permissive(account, now), None);
        }

        let base = resolved_enforcement(req, account, host);

        // An absent or empty string is the "no opt-in collected yet"
        // convention, not a defect, so only a non-empty string that
        // fails to decode warns.
        let parsed = match sources.tcf().raw() {
            None | Some("") => None,
            Some(raw) => match parse_tcf_v2(raw) {
                Ok(tcf) => Some(tcf),
                Err(_) => {
                    return (
                        enforced_but_empty(base, now),
                        Some(ConsentDiagnostic::Malformed),
                    );
                }
            },
        };

        let Some(tcf) = parsed else {
            return (enforced_but_empty(base, now), None);
        };

        // A list past its freshness window cannot authenticate any CMP,
        // so every TCF request fails closed until the deployment is
        // rebuilt.
        if now >= gcl.expires_at {
            return (
                enforced_but_empty(base, now),
                Some(ConsentDiagnostic::StaleGcl),
            );
        }
        let Some(cmp) = gcl.cmps.get(&tcf.cmp_id) else {
            return (
                enforced_but_empty(base, now),
                Some(ConsentDiagnostic::UnauthenticCmp),
            );
        };
        // A deletion scheduled for the future leaves the CMP valid until
        // it lands; `tcf.created` plays no part, because the current
        // spec invalidates a string once the deletion date passes
        // regardless of when the string was written.
        if cmp.deleted_date.is_some_and(|deleted| deleted <= now) {
            return (
                enforced_but_empty(base, now),
                Some(ConsentDiagnostic::UnauthenticCmp),
            );
        }

        let policy = u32::from(tcf.tcf_policy_version);
        let spec = gvl_spec_for_policy(policy);
        let list = u32::from(tcf.vendor_list_version);
        let Some(gvl) = gvl_store.get(spec, list) else {
            return (
                enforced_but_empty(base, now),
                Some(ConsentDiagnostic::UnavailableGvl { spec, list }),
            );
        };
        // The store is keyed by `(spec, list)` alone, and
        // `gvl_spec_for_policy` maps every policy from 4 upward onto
        // spec 3, so the key alone does not prove the blob implements
        // the policy the string was written under. A string claiming a
        // policy the list PREDATES cannot be evaluated against it, so
        // that direction fails closed. The reverse is the ordinary
        // forward-compatible case the TCF expects — policy-4 strings
        // are authorized against the current policy-5 list — and
        // rejecting it would deny most live consent.
        if policy > gvl.tcf_policy_version() {
            return (
                enforced_but_empty(base, now),
                Some(ConsentDiagnostic::UnavailableGvl { spec, list }),
            );
        }

        (with_consent(base, now, tcf, gvl), None)
    }
}

/// The out-of-scope shape: nothing enforced, diagnostics preserved.
fn permissive(account: &AccountConfig, now: OffsetDateTime) -> PrivacyContext {
    PrivacyContext {
        authorization_now: now,
        ..PrivacyContext::with_activities(Arc::clone(&account.privacy.allowactivities))
    }
}

/// The fail-closed shape: enforcement configured, consent unusable.
///
/// Both this and [`with_consent`] spell out every field rather than
/// spreading a base, so a field added to `PrivacyContext` fails to
/// compile here instead of silently defaulting on one of the two paths.
/// [`permissive`] does spread, deliberately: its whole contract is to be
/// the allow-all shape, so inheriting a new field's allow-all default is
/// the right answer there.
fn enforced_but_empty(base: ResolvedEnforcement, now: OffsetDateTime) -> PrivacyContext {
    PrivacyContext {
        activities: base.activities,
        independent: IndependentPolicies::default(),
        warnings: Vec::new(),
        per_purpose_effective: base.per_purpose_effective,
        enforcement_active: true,
        consent_usable: false,
        authorization_now: now,
        p2_enforced: base.p2_enforced,
        sf1_enforced: base.sf1_enforced,
        tcf: None,
        gvl: Gvl::empty(),
        alias_gvl_ids: base.alias_gvl_ids,
        basic_enforcement_vendors: base.basic_enforcement_vendors,
        sf1_effective: base.sf1_effective,
    }
}

/// The full shape: a TC string decoded, its CMP authenticated, and the
/// GVL revision it declared available.
fn with_consent(
    base: ResolvedEnforcement,
    now: OffsetDateTime,
    tcf: TcfString,
    gvl: Gvl,
) -> PrivacyContext {
    PrivacyContext {
        activities: base.activities,
        independent: IndependentPolicies::default(),
        warnings: Vec::new(),
        per_purpose_effective: base.per_purpose_effective,
        enforcement_active: true,
        consent_usable: true,
        authorization_now: now,
        p2_enforced: base.p2_enforced,
        sf1_enforced: base.sf1_enforced,
        tcf: Some(Box::new(tcf)),
        gvl,
        alias_gvl_ids: base.alias_gvl_ids,
        basic_enforcement_vendors: base.basic_enforcement_vendors,
        sf1_effective: base.sf1_effective,
    }
}

fn resolved_enforcement(
    req: &BidRequest,
    account: &AccountConfig,
    host: &HostConfig,
) -> ResolvedEnforcement {
    let per_purpose_effective = per_purpose_table(&host.gdpr.tcf2, &account.gdpr);
    let sf1_effective = effective_sf1(&host.gdpr.tcf2, &account.gdpr);
    ResolvedEnforcement {
        activities: Arc::clone(&account.privacy.allowactivities),
        p2_enforced: per_purpose_effective
            .get(&2)
            .is_some_and(|purpose| purpose.enforce_purpose == TriState::Enabled),
        sf1_enforced: sf1_effective
            .as_ref()
            .is_some_and(|sf1| sf1.enforce == TriState::Enabled),
        per_purpose_effective,
        sf1_effective,
        alias_gvl_ids: alias_gvl_ids(req),
        basic_enforcement_vendors: account
            .gdpr
            .basic_enforcement_vendors
            .iter()
            .cloned()
            .collect(),
    }
}

/// `Ambiguous` alone is not a green light: it enforces only when the geo
/// ladder agrees, so a request whose wire is silent but whose geo says
/// No stays out of scope.
fn enforcement_active(wire: Signal, geo: Signal, channel_enabled: bool) -> bool {
    match wire {
        Signal::Yes => channel_enabled,
        Signal::No => false,
        Signal::Ambiguous => channel_enabled && geo == Signal::Yes,
    }
}

/// A present geo OBJECT with an absent country is distinct from an
/// absent object, and the ladder reads the difference.
fn geo_view(geo: Option<&Geo>) -> Option<GeoView<'_>> {
    geo.map(|object| GeoView {
        country: object.country.as_deref(),
    })
}

fn channel_of(req: &BidRequest) -> Channel {
    if req.dooh.is_some() {
        Channel::Dooh
    } else if req.app.is_some() {
        Channel::App
    } else {
        Channel::Web
    }
}

fn channel_tri(channels: &ChannelEnabled, channel: Channel) -> TriState {
    match channel {
        Channel::App => channels.app,
        Channel::Dooh => channels.dooh,
        Channel::Web => channels.web,
    }
}

/// The §7.1 ladder: the channel's own tri-state, then the account-wide
/// tri-state, then the host default.
fn channel_enabled(account: &AccountGdprConfig, host: &HostConfig, channel: Channel) -> bool {
    match tri_or(
        channel_tri(&account.channel_enabled, channel),
        account.enabled,
    ) {
        TriState::Enabled => true,
        TriState::Disabled => false,
        TriState::Unset => host.gdpr.tcf2.enabled,
    }
}

fn ccpa_enabled(account: &AccountConfig, host: &HostConfig, channel: Channel) -> bool {
    match tri_or(
        channel_tri(&account.ccpa.channel_enabled, channel),
        account.ccpa.enabled,
    ) {
        TriState::Enabled => true,
        TriState::Disabled => false,
        TriState::Unset => host.ccpa.enforce,
    }
}

fn tri_or(primary: TriState, fallback: TriState) -> TriState {
    if primary == TriState::Unset {
        fallback
    } else {
        primary
    }
}

/// The union of host-declared and account-declared purposes.
///
/// Whole-map replacement is explicitly not the semantics: an account
/// that overrides purpose 3 leaves every other purpose at its host
/// value.
fn per_purpose_table(
    host: &Tcf2Config,
    account: &AccountGdprConfig,
) -> BTreeMap<u8, PurposeEnforcement> {
    let mut table: BTreeMap<u8, PurposeEnforcement> = host
        .purposes
        .iter()
        .map(|(id, purpose)| (*id, effective_purpose(purpose, account.purposes.get(id))))
        .collect();
    for (id, purpose) in &account.purposes {
        table.entry(*id).or_insert_with(|| purpose.clone());
    }
    table
}

/// Only `enforce_purpose` and `enforce_vendors` fall through to the
/// host: §9 makes `enforce_algo` and `vendor_exceptions` account-only,
/// so an account entry replaces those outright.
fn effective_purpose(
    host: &PurposeEnforcement,
    account: Option<&PurposeEnforcement>,
) -> PurposeEnforcement {
    let Some(overlay) = account else {
        return host.clone();
    };
    PurposeEnforcement {
        enforce_algo: overlay.enforce_algo,
        vendor_exceptions: overlay.vendor_exceptions.clone(),
        enforce_purpose: tri_or(overlay.enforce_purpose, host.enforce_purpose),
        enforce_vendors: tri_or(overlay.enforce_vendors, host.enforce_vendors),
    }
}

/// SF1 resolves exactly as a purpose does: `enforce` is a tri-state that
/// falls through to the host when the account has no opinion, while
/// `vendor_exceptions` is account-only and replaces the host's list
/// outright.
///
/// Reading the account entry whole would let an account that declares
/// only exceptions — leaving `enforce` at `Unset` — discard a host
/// `Enabled` and silently stop enforcing geolocation for every bidder.
fn effective_sf1(
    host: &Tcf2Config,
    account: &AccountGdprConfig,
) -> Option<SpecialFeatureEnforcement> {
    let host_sf1 = host.special_features.get(&SPECIAL_FEATURE_GEO);
    let Some(overlay) = account.special_features.get(&SPECIAL_FEATURE_GEO) else {
        return host_sf1.cloned();
    };
    Some(SpecialFeatureEnforcement {
        enforce: tri_or(
            overlay.enforce,
            host_sf1.map_or(TriState::Unset, |sf1| sf1.enforce),
        ),
        vendor_exceptions: overlay.vendor_exceptions.clone(),
    })
}

/// The request's alias vendor-id declarations, cached so the context
/// does not borrow the request.
///
/// An id above the GVL's `u16` namespace is normalised to 0 — the
/// unknown vendor, which denies under full enforcement. The exchange
/// rejects the same input with a 400 before any permission call, so this
/// only covers a caller that runs the auction without alias resolution;
/// it exists so the resolver is total rather than trusting that caller.
fn alias_gvl_ids(req: &BidRequest) -> BTreeMap<Box<str>, u32> {
    req.ext
        .as_ref()
        .and_then(|ext| ext.prebid.as_ref())
        .map(|prebid| {
            prebid
                .aliasgvlids
                .iter()
                .map(|(alias, id)| {
                    let bounded = if *id > u32::from(u16::MAX) { 0 } else { *id };
                    (alias.as_str().into(), bounded)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::Arc;

    use auctioneer_config::types::{
        AccountConfig, EnforceAlgo, PurposeEnforcement, SpecialFeatureEnforcement, TriState,
    };
    use auctioneer_openrtb::WarningCode;
    use auctioneer_openrtb::ext::request_prebid::{Channel as ExtChannel, ExtRequestPrebid};
    use auctioneer_openrtb::types::bid_request::{BidRequest, BidRequestExt};
    use auctioneer_openrtb::types::device::{Device, Geo};
    use auctioneer_openrtb::types::inventory::{App, Dooh, Site};
    use auctioneer_openrtb::types::regs::Regs;
    use auctioneer_openrtb::types::user::User;
    use time::OffsetDateTime;
    use time::macros::datetime;

    use super::{Channel, HostConfig, channel_of};
    use crate::auction_context::PrivacyContext;
    use crate::auction_permissions::AuctionPermissions;
    use crate::gcl::{CmpRecord, Gcl};
    use crate::gvl::{Gvl, GvlStore};
    use crate::purposes::{
        effective_flags_for_purpose, purpose_allowed, purpose_allowed_basic, purpose_effective_algo,
    };

    const CORE: &str = "fake_corebidder";
    const VENDOR: u16 = 8;
    /// The `cmp_id`, `vendorListVersion`, and derived GVL spec every TCF
    /// fixture in `tests/data/tcf_v2` encodes.
    const CMP_ID: u16 = 300;
    const LIST: u32 = 348;
    const SPEC: u32 = 3;
    const NOW: OffsetDateTime = datetime!(2026-08-29 12:00:00 UTC);
    /// The row every unusable-consent branch must produce under
    /// [`host_config`]: purposes 1-10 enforced, special feature 1 not.
    const DEFAULT_ROW: AuctionPermissions = AuctionPermissions::Partial {
        allow_bid_request: false,
        pass_id: false,
        pass_geo: true,
    };
    /// The same row once special feature 1 is enforced too.
    const DEFAULT_ROW_SF1_ENFORCED: AuctionPermissions = AuctionPermissions::Partial {
        allow_bid_request: false,
        pass_id: false,
        pass_geo: false,
    };

    fn fixture(name: &str) -> String {
        let path = format!("{}/tests/data/tcf_v2/{name}", env!("CARGO_MANIFEST_DIR"));
        fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("fixture {path} unreadable: {err}"))
            .trim_end()
            .to_owned()
    }

    /// A GVL at the revision the fixtures declare, where [`VENDOR`]
    /// declares purposes 1-10 under consent and nothing under legitimate
    /// interest.
    fn gvl_with_vendor() -> Gvl {
        gvl_json(&format!(
            r#""{VENDOR}":{{"id":{VENDOR},"name":"Vendor Eight",
                 "purposes":[1,2,3,4,5,6,7,8,9,10],
                 "legIntPurposes":[],"flexiblePurposes":[]}}"#
        ))
    }

    /// The same revision with no vendors at all, so any decision that
    /// consults the GVL denies and one that does not still grants.
    fn gvl_without_vendors() -> Gvl {
        gvl_json("")
    }

    /// The same revision declaring a different TCF policy version, for
    /// the policy-comparison branch.
    fn gvl_at_policy(policy: u32) -> Gvl {
        gvl_json_at_policy("", policy)
    }

    fn gvl_json(vendors: &str) -> Gvl {
        gvl_json_at_policy(vendors, 5)
    }

    fn gvl_json_at_policy(vendors: &str, policy: u32) -> Gvl {
        let purposes = (1_u8..=10)
            .map(|id| format!(r#""{id}":{{"id":{id},"name":"Purpose {id}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        // Raw `format!`, not `serde_json::json!`: `auctioneer-privacy` has
        // no `serde_json` dependency on purpose — see the note in
        // `testkit::gvl`.
        let json = format!(
            r#"{{"gvlSpecificationVersion":3,"tcfPolicyVersion":{policy},
                 "vendorListVersion":{LIST},"lastUpdated":"2026-07-01T00:00:00Z",
                 "purposes":{{{purposes}}},
                 "specialPurposes":{{"1":{{"id":1,"name":"Security"}}}},
                 "features":{{"1":{{"id":1,"name":"Matching"}}}},
                 "specialFeatures":{{"1":{{"id":1,"name":"Geolocation"}}}},
                 "vendors":{{{vendors}}}}}"#
        );
        Gvl::load_from_json(&json, None).expect("test GVL is valid")
    }

    fn store() -> GvlStore {
        GvlStore::for_test(vec![(SPEC, LIST, gvl_with_vendor())])
    }

    fn fresh_gcl() -> Gcl {
        Gcl::for_test(
            vec![CmpRecord {
                id: CMP_ID,
                deleted_date: None,
            }],
            datetime!(2027-01-01 0:00:00 UTC),
        )
    }

    fn purpose(enforce_purpose: TriState, enforce_vendors: TriState) -> PurposeEnforcement {
        PurposeEnforcement {
            enforce_algo: EnforceAlgo::Full,
            enforce_purpose,
            enforce_vendors,
            vendor_exceptions: Vec::new(),
        }
    }

    /// Purposes 1-10 with both flags on, the shape most enforcement
    /// assertions want.
    fn enforced_purposes() -> BTreeMap<u8, PurposeEnforcement> {
        (1_u8..=10)
            .map(|id| (id, purpose(TriState::Enabled, TriState::Enabled)))
            .collect()
    }

    /// Host with GDPR on, TCF2 on, and a `default_value` of `0` so an
    /// ambiguous wire signal resolves to "does not apply" unless a test
    /// supplies geo.
    fn host_config() -> HostConfig {
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.default_value = "0".to_owned();
        host.gdpr.tcf2.enabled = true;
        host.gdpr.tcf2.purposes = enforced_purposes();
        host
    }

    fn account_config() -> AccountConfig {
        AccountConfig::default()
    }

    fn site_request() -> BidRequest {
        BidRequest {
            id: "req-1".to_owned(),
            site: Some(Site::default()),
            ..BidRequest::default()
        }
    }

    /// A site request that already carries `regs.gdpr = 1`, so the wire
    /// signal is `Yes` and the enforcement path is reached.
    fn applies_request() -> BidRequest {
        BidRequest {
            regs: Some(Regs {
                gdpr: Some(1_i32),
                ..Regs::default()
            }),
            ..site_request()
        }
    }

    fn with_consent(req: BidRequest, consent: &str) -> BidRequest {
        BidRequest {
            user: Some(User {
                consent: Some(consent.to_owned()),
                ..User::default()
            }),
            ..req
        }
    }

    fn ext_channel(name: &str) -> BidRequestExt {
        BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                channel: Some(ExtChannel {
                    name: Some(name.to_owned()),
                    version: None,
                }),
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        }
    }

    fn resolve_with(
        req: &BidRequest,
        account: &AccountConfig,
        host: &HostConfig,
        gvl_store: &GvlStore,
        gcl: &Gcl,
    ) -> PrivacyContext {
        PrivacyContext::resolve(
            req,
            account,
            host,
            gvl_store,
            gcl,
            NOW,
            super::ResolveOptions::default(),
        )
        .expect("legal regs.gdpr")
        .context
    }

    fn resolve(req: &BidRequest, account: &AccountConfig, host: &HostConfig) -> PrivacyContext {
        resolve_with(req, account, host, &store(), &fresh_gcl())
    }

    /// The enforcement path is what fills the per-purpose table; the
    /// out-of-scope path returns the permissive context, whose table is
    /// empty. With purposes configured, the two are distinguishable.
    fn enforcing(ctx: &PrivacyContext) -> bool {
        !ctx.per_purpose_effective.is_empty()
    }

    fn permits(ctx: &PrivacyContext, purpose_id: u8) -> bool {
        purpose_allowed(
            ctx,
            purpose_id,
            CORE,
            VENDOR,
            effective_flags_for_purpose(ctx, purpose_id),
        )
    }

    #[test]
    fn empty_request_permissive_defaults() {
        // Neither regs nor user: the wire signal is Ambiguous and the geo
        // ladder falls to the host default of "does not apply", so
        // enforcement never starts and nothing warns.
        let ctx = resolve(&site_request(), &account_config(), &host_config());
        assert!(!enforcing(&ctx));
        assert!(ctx.warnings.is_empty());
        assert!(ctx.tcf.is_none());
        for purpose_id in 1_u8..=10 {
            assert!(permits(&ctx, purpose_id), "purpose {purpose_id}");
        }

        // The geo answer is what decided it, not the ambiguity: the same
        // request with an EEA device geo does enforce.
        let in_eea = BidRequest {
            device: Some(Device {
                geo: Some(Geo {
                    country: Some("DEU".to_owned()),
                    ..Geo::default()
                }),
                ..Device::default()
            }),
            ..site_request()
        };
        assert!(enforcing(&resolve(
            &in_eea,
            &account_config(),
            &host_config()
        )));

        // A present user.geo whose country is absent still blocks the
        // device fallback, so the host default decides rather than the
        // device's EEA country.
        let user_object_only = BidRequest {
            user: Some(User {
                geo: Some(Geo::default()),
                ..User::default()
            }),
            ..in_eea
        };
        assert!(!enforcing(&resolve(
            &user_object_only,
            &account_config(),
            &host_config()
        )));
    }

    #[test]
    fn signal_no_permits_all() {
        let req = BidRequest {
            regs: Some(Regs {
                gdpr: Some(0_i32),
                ..Regs::default()
            }),
            ..site_request()
        };
        let ctx = resolve(&req, &account_config(), &host_config());
        assert!(!enforcing(&ctx));
        for purpose_id in 1_u8..=10 {
            assert!(permits(&ctx, purpose_id), "purpose {purpose_id}");
        }
    }

    #[test]
    fn signal_yes_full_consent_permits_listed() {
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve(&req, &account_config(), &host_config());
        assert!(ctx.consent_usable);
        assert!(enforcing(&ctx));
        for purpose_id in 1_u8..=10 {
            assert!(permits(&ctx, purpose_id), "purpose {purpose_id}");
        }
    }

    #[test]
    fn signal_yes_purpose1_only_others_denied() {
        // Vendor enforcement off isolates the purpose bits: the fixture
        // consents to purpose 1 and to no vendor at all.
        let mut host = host_config();
        host.gdpr.tcf2.purposes = (1_u8..=10)
            .map(|id| (id, purpose(TriState::Enabled, TriState::Disabled)))
            .collect();
        let req = with_consent(applies_request(), &fixture("valid_purpose_1_only.txt"));
        let ctx = resolve(&req, &account_config(), &host);
        assert!(ctx.consent_usable);
        assert!(permits(&ctx, 1));
        for purpose_id in 2_u8..=10 {
            assert!(!permits(&ctx, purpose_id), "purpose {purpose_id}");
        }
    }

    #[test]
    fn signal_yes_missing_tcf_default_row_no_warning() {
        // Empty consent is the "no opt-in collected yet" convention, not
        // a request defect, so the enforced-but-empty row carries no
        // warning.
        let ctx = resolve(&applies_request(), &account_config(), &host_config());
        assert!(enforcing(&ctx));
        assert!(!ctx.consent_usable);
        assert!(ctx.tcf.is_none());
        assert!(ctx.warnings.is_empty());
        assert_eq!(ctx.permissions(CORE, CORE), DEFAULT_ROW);

        let empty = with_consent(applies_request(), "");
        let empty_ctx = resolve(&empty, &account_config(), &host_config());
        assert!(!empty_ctx.consent_usable);
        assert!(empty_ctx.warnings.is_empty());
        assert_eq!(empty_ctx.permissions(CORE, CORE), DEFAULT_ROW);
    }

    #[test]
    fn malformed_tcf_string_unusable_with_warning() {
        let req = with_consent(applies_request(), "~~~not-a-tc-string~~~");
        let ctx = resolve(&req, &account_config(), &host_config());
        assert!(!ctx.consent_usable);
        assert!(ctx.tcf.is_none());
        let warning = ctx.warnings.first().expect("one malformed-consent warning");
        assert_eq!(warning.code, WarningCode::InvalidPrivacyConsent);
        assert_eq!(warning.scope.as_deref(), Some("user.consent"));
    }

    #[test]
    fn gvl_pair_unavailable_fails_closed_default_row() {
        // A store holding some other revision: the TC string parses, but
        // the revision it declares cannot be loaded.
        let elsewhere = GvlStore::for_test(vec![(SPEC, 999_u32, gvl_with_vendor())]);
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve_with(
            &req,
            &account_config(),
            &host_config(),
            &elsewhere,
            &fresh_gcl(),
        );
        assert!(!ctx.consent_usable);
        assert!(ctx.tcf.is_none());
        assert_eq!(ctx.gvl.vendor_list_version(), 0_u32);
        assert_eq!(ctx.permissions(CORE, CORE), DEFAULT_ROW);
        // The `/_auctioneer/health/gvl` counter watches for this code, so
        // the branch has to emit it or the alarm is permanently silent.
        let warning = ctx.warnings.first().expect("one unavailable-GVL warning");
        assert_eq!(warning.code, WarningCode::GvlVersionUnavailable);
        assert_eq!(warning.code.as_u32(), 10018_u32);
        assert!(warning.message.contains("spec 3, list 348"));
    }

    #[test]
    fn tc_string_policy_newer_than_gvl_fails_closed_with_10018() {
        // A string written under a policy the bundled list predates
        // cannot be evaluated against it. The reverse direction — the
        // fixtures' policy 4 against the list's policy 5 — is the
        // ordinary case and stays usable.
        let ahead = GvlStore::for_test(vec![(SPEC, LIST, gvl_at_policy(3))]);
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve_with(
            &req,
            &account_config(),
            &host_config(),
            &ahead,
            &fresh_gcl(),
        );
        assert!(!ctx.consent_usable);
        assert_eq!(ctx.permissions(CORE, CORE), DEFAULT_ROW);
        assert_eq!(
            ctx.warnings.first().map(|w| w.code),
            Some(WarningCode::GvlVersionUnavailable)
        );

        let behind = resolve_with(
            &req,
            &account_config(),
            &host_config(),
            &store(),
            &fresh_gcl(),
        );
        assert!(behind.consent_usable);
    }

    #[test]
    fn enforced_but_empty_context_populates_all_fields() {
        let mut account = account_config();
        account.gdpr.basic_enforcement_vendors = vec![CORE.to_owned()];
        account.gdpr.special_features = BTreeMap::from([(
            1_u8,
            SpecialFeatureEnforcement {
                enforce: TriState::Enabled,
                vendor_exceptions: Vec::new(),
            },
        )]);
        let req = BidRequest {
            // A present-but-empty gpp_sid conflicts with regs.gdpr, so
            // the warnings buffer is populated too.
            regs: Some(Regs {
                gdpr: Some(1_i32),
                gpp_sid: Some(Vec::new()),
                ..Regs::default()
            }),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliasgvlids: BTreeMap::from([("fake_myalias".to_owned(), 32_u32)]),
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..site_request()
        };
        let ctx = resolve(&req, &account, &host_config());

        assert_eq!(ctx.warnings.len(), 1);
        assert_eq!(ctx.per_purpose_effective.len(), 10);
        assert!(!ctx.consent_usable);
        assert_eq!(ctx.authorization_now, NOW);
        assert!(ctx.p2_enforced);
        assert!(ctx.sf1_enforced);
        assert!(ctx.tcf.is_none());
        assert_eq!(ctx.gvl.vendor_list_version(), 0_u32);
        assert_eq!(ctx.alias_gvl_ids.get("fake_myalias"), Some(&32_u32));
        assert!(ctx.basic_enforcement_vendors.contains(CORE));
        assert!(ctx.sf1_effective.is_some());
    }

    #[test]
    fn host_gdpr_disabled_short_circuits_permissive() {
        let mut host = host_config();
        host.gdpr.enabled = false;
        let req = with_consent(
            BidRequest {
                regs: Some(Regs {
                    gdpr: Some(1_i32),
                    gpp_sid: Some(Vec::new()),
                    ..Regs::default()
                }),
                ..site_request()
            },
            &fixture("valid_full_consent.txt"),
        );
        let ctx = resolve(&req, &account_config(), &host);
        assert!(!enforcing(&ctx));
        assert!(ctx.tcf.is_none());
        // The kill switch stops enforcement, not diagnostics: the wire
        // signal runs before it, so its conflict warning still lands.
        assert_eq!(
            ctx.warnings.first().and_then(|w| w.scope.as_deref()),
            Some("regs.gpp_sid")
        );
    }

    #[test]
    fn resolve_with_present_empty_gpp_sid_conflict_appends_regs_gpp_sid_scope() {
        let req = BidRequest {
            regs: Some(Regs {
                gdpr: Some(1_i32),
                gpp_sid: Some(Vec::new()),
                ..Regs::default()
            }),
            ..site_request()
        };
        let ctx = resolve(&req, &account_config(), &host_config());
        let warning = ctx.warnings.first().expect("one conflict warning");
        assert_eq!(warning.code, WarningCode::InvalidPrivacyConsent);
        assert_eq!(warning.scope.as_deref(), Some("regs.gpp_sid"));
    }

    #[test]
    fn gpp_sid_section2_without_envelope_uses_legacy_consent() {
        // M4 can decode GPP. SID still selects scope, but a missing
        // envelope permits the documented legacy fallback.
        let req = with_consent(
            BidRequest {
                regs: Some(Regs {
                    gdpr: Some(1_i32),
                    gpp_sid: Some(vec![2_i32]),
                    ..Regs::default()
                }),
                ..site_request()
            },
            &fixture("valid_full_consent.txt"),
        );
        let ctx = resolve(&req, &account_config(), &host_config());
        assert!(enforcing(&ctx));
        assert!(ctx.consent_usable);
        assert!(ctx.tcf.is_some());
    }

    #[test]
    fn activity_rules_remain_shared_across_all_gdpr_resolution_shapes() {
        let mut account = account_config();
        account.privacy.allowactivities = serde_json::from_value(serde_json::json!({
            "fetchBids":{"default":false},"transmitTid":{"rules":[{"allow":false}]}
        }))
        .unwrap();
        for req in [
            site_request(),
            applies_request(),
            with_consent(applies_request(), "invalid"),
            with_consent(applies_request(), &fixture("valid_full_consent.txt")),
        ] {
            let ctx = resolve(&req, &account, &host_config());
            assert!(Arc::ptr_eq(
                &ctx.activities,
                &account.privacy.allowactivities
            ));
            assert!(!ctx.activity_allow(
                crate::Activity::FetchBids,
                crate::Component::Bidder("fake_ax")
            ));
            assert!(!ctx.activity_allow(
                crate::Activity::TransmitTid,
                crate::Component::Bidder("fake_ax")
            ));
        }
    }

    #[test]
    fn gpp_sid_without_section2_reads_user_consent() {
        let consent = fixture("valid_full_consent.txt");
        for gpp_sid in [None, Some(Vec::new())] {
            let req = with_consent(
                BidRequest {
                    regs: Some(Regs {
                        gdpr: Some(1_i32),
                        gpp_sid,
                        ..Regs::default()
                    }),
                    ..site_request()
                },
                &consent,
            );
            let ctx = resolve(&req, &account_config(), &host_config());
            assert!(ctx.consent_usable);
            assert!(ctx.tcf.is_some());
        }
    }

    #[test]
    fn vendor_exceptions_honored_in_resolve() {
        // Purpose 2 is not consented by this fixture, so only the
        // account's exception can grant it.
        let req = with_consent(applies_request(), &fixture("valid_purpose_1_only.txt"));
        let denied = resolve(&req, &account_config(), &host_config());
        assert!(!permits(&denied, 2));

        let mut account = account_config();
        account.gdpr.purposes = BTreeMap::from([(
            2_u8,
            PurposeEnforcement {
                vendor_exceptions: vec![CORE.to_owned()],
                ..purpose(TriState::Enabled, TriState::Enabled)
            },
        )]);
        let excepted = resolve(&req, &account, &host_config());
        assert!(
            excepted.per_purpose_effective[&2]
                .vendor_exceptions
                .contains(&CORE.to_owned())
        );
        assert!(permits(&excepted, 2));
    }

    #[test]
    fn purpose_one_treatment_short_circuits_purpose1() {
        // Nothing reads this flag yet. The permission gates are
        // Purpose 2, Purposes 2-10, and special feature 1, so purpose 1
        // is never evaluated on the auction path; purpose-1 treatment
        // governs device storage and access, which is the sync path's
        // question. What resolution owes it today is the decoded flag.
        let req = with_consent(
            applies_request(),
            &fixture("valid_purpose_one_treatment.txt"),
        );
        let ctx = resolve(&req, &account_config(), &host_config());
        assert!(
            ctx.tcf
                .as_deref()
                .is_some_and(|tcf| tcf.purpose_one_treatment)
        );

        let baseline = with_consent(applies_request(), &fixture("valid_purpose_1_only.txt"));
        let untreated = resolve(&baseline, &account_config(), &host_config());
        assert!(
            untreated
                .tcf
                .as_deref()
                .is_some_and(|tcf| !tcf.purpose_one_treatment)
        );
    }

    #[test]
    fn basic_enforcement_bidder_skips_gvl_lookup() {
        // The GVL at this revision declares no vendors, so any decision
        // that consults it denies. Basic enforcement never does.
        let vendorless = GvlStore::for_test(vec![(SPEC, LIST, gvl_without_vendors())]);
        let mut account = account_config();
        account.gdpr.basic_enforcement_vendors = vec![CORE.to_owned()];
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve_with(&req, &account, &host_config(), &vendorless, &fresh_gcl());

        assert_eq!(purpose_effective_algo(&ctx, 2, CORE), EnforceAlgo::Basic);
        assert!(purpose_allowed_basic(
            &ctx,
            2,
            CORE,
            VENDOR,
            effective_flags_for_purpose(&ctx, 2),
            false
        ));
        assert!(
            !permits(&ctx, 2),
            "the full table needs the GVL declaration"
        );
    }

    #[test]
    fn account_basic_algorithm_controls_live_permissions_without_a_bidder_list_downgrade() {
        let vendorless = GvlStore::for_test(vec![(SPEC, LIST, gvl_without_vendors())]);
        let mut req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        req.ext = Some(BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                aliasgvlids: BTreeMap::from([("fake_alias".to_owned(), u32::from(VENDOR))]),
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        });
        let mut account = account_config();
        let mut host = host_config();
        let full = resolve_with(&req, &account, &host, &vendorless, &fresh_gcl());
        assert_eq!(full.permissions(CORE, "fake_alias"), DEFAULT_ROW);

        account.gdpr.purposes.insert(
            2,
            PurposeEnforcement {
                enforce_algo: EnforceAlgo::Basic,
                enforce_purpose: TriState::Enabled,
                enforce_vendors: TriState::Enabled,
                vendor_exceptions: Vec::new(),
            },
        );
        assert!(account.gdpr.basic_enforcement_vendors.is_empty());
        let basic = resolve_with(&req, &account, &host, &vendorless, &fresh_gcl());
        assert_eq!(purpose_effective_algo(&basic, 2, CORE), EnforceAlgo::Basic);
        assert_eq!(
            basic.permissions(CORE, "fake_alias"),
            AuctionPermissions::AllowAll
        );

        // An explicit account Full must replace a host Basic as well.
        host.gdpr.tcf2.purposes.get_mut(&2).unwrap().enforce_algo = EnforceAlgo::Basic;
        account.gdpr.purposes.get_mut(&2).unwrap().enforce_algo = EnforceAlgo::Full;
        let restored = resolve_with(&req, &account, &host, &vendorless, &fresh_gcl());
        assert_eq!(
            purpose_effective_algo(&restored, 2, CORE),
            EnforceAlgo::Full
        );
        assert_eq!(restored.permissions(CORE, "fake_alias"), DEFAULT_ROW);
    }

    #[test]
    fn per_purpose_enforcement_account_overrides_host() {
        let mut host = host_config();
        host.gdpr.tcf2.purposes = BTreeMap::from([
            (2_u8, purpose(TriState::Disabled, TriState::Disabled)),
            (3_u8, purpose(TriState::Enabled, TriState::Enabled)),
        ]);
        let mut account = account_config();
        account.gdpr.purposes = BTreeMap::from([
            (
                2_u8,
                PurposeEnforcement {
                    enforce_algo: EnforceAlgo::Basic,
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Unset,
                    vendor_exceptions: vec![CORE.to_owned()],
                },
            ),
            (4_u8, purpose(TriState::Enabled, TriState::Enabled)),
        ]);
        let ctx = resolve(&applies_request(), &account, &host);

        let two = &ctx.per_purpose_effective[&2];
        assert_eq!(two.enforce_purpose, TriState::Enabled, "account overrides");
        assert_eq!(
            two.enforce_vendors,
            TriState::Disabled,
            "an Unset account field falls through to the host"
        );
        assert_eq!(two.enforce_algo, EnforceAlgo::Basic, "account-only field");
        assert_eq!(two.vendor_exceptions, vec![CORE.to_owned()]);
        assert_eq!(
            ctx.per_purpose_effective[&3].enforce_purpose,
            TriState::Enabled,
            "a host-only purpose survives an account overlay elsewhere"
        );
        assert!(
            ctx.per_purpose_effective.contains_key(&4),
            "an account-only purpose is active"
        );

        // The SF1 slot runs the same account-over-host resolution.
        let mut host_sf1 = host;
        host_sf1.gdpr.tcf2.special_features = BTreeMap::from([(
            1_u8,
            SpecialFeatureEnforcement {
                enforce: TriState::Disabled,
                vendor_exceptions: Vec::new(),
            },
        )]);
        let mut account_sf1 = account.clone();
        account_sf1.gdpr.special_features = BTreeMap::from([(
            1_u8,
            SpecialFeatureEnforcement {
                enforce: TriState::Enabled,
                vendor_exceptions: Vec::new(),
            },
        )]);
        let overridden = resolve(&applies_request(), &account_sf1, &host_sf1);
        assert!(overridden.sf1_enforced, "the account entry wins outright");

        let inherited = resolve(&applies_request(), &account, &host_sf1);
        assert!(
            inherited.sf1_effective.is_some(),
            "a host-only SF1 entry still resolves"
        );
        assert!(!inherited.sf1_enforced, "and keeps the host's own value");

        // An account entry that declares only exceptions leaves
        // `enforce` unset, which must inherit the host's value rather
        // than replace it — replacing would disable geo enforcement for
        // every bidder without the operator asking for it.
        let mut enforcing_host = host_sf1;
        enforcing_host.gdpr.tcf2.special_features = BTreeMap::from([(
            1_u8,
            SpecialFeatureEnforcement {
                enforce: TriState::Enabled,
                vendor_exceptions: Vec::new(),
            },
        )]);
        let mut exceptions_only = account;
        exceptions_only.gdpr.special_features = BTreeMap::from([(
            1_u8,
            SpecialFeatureEnforcement {
                enforce: TriState::Unset,
                vendor_exceptions: vec!["fake_someotherbidder".to_owned()],
            },
        )]);
        let merged = resolve(&applies_request(), &exceptions_only, &enforcing_host);
        assert!(merged.sf1_enforced, "the host's enforce survives");
        assert_eq!(
            merged.permissions(CORE, CORE),
            DEFAULT_ROW_SF1_ENFORCED,
            "and geo stays denied without an opt-in"
        );
        assert_eq!(
            merged
                .sf1_effective
                .as_ref()
                .map(|sf1| sf1.vendor_exceptions.len()),
            Some(1),
            "while the account's exception list replaces the host's"
        );
    }

    #[test]
    fn unknown_cmp_id_fails_closed_default_permissions() {
        let other_cmp = Gcl::for_test(
            vec![CmpRecord {
                id: 7_u16,
                deleted_date: None,
            }],
            datetime!(2027-01-01 0:00:00 UTC),
        );
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve_with(
            &req,
            &account_config(),
            &host_config(),
            &store(),
            &other_cmp,
        );
        assert!(!ctx.consent_usable);
        assert!(ctx.tcf.is_none());
        let warning = ctx.warnings.first().expect("one authenticity warning");
        assert_eq!(warning.code, WarningCode::InvalidPrivacyConsent);
        assert_eq!(warning.scope.as_deref(), Some("user.consent"));
    }

    #[test]
    fn cmp_deleted_date_in_past_fails_closed_regardless_of_created() {
        // The fixture was created 2024-02-01, before the deletion, and
        // the deletion is before `now`. Creation order does not rescue
        // it.
        let deleted = Gcl::for_test(
            vec![CmpRecord {
                id: CMP_ID,
                deleted_date: Some(datetime!(2024-06-01 0:00:00 UTC)),
            }],
            datetime!(2027-01-01 0:00:00 UTC),
        );
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve_with(&req, &account_config(), &host_config(), &store(), &deleted);
        assert!(!ctx.consent_usable);
        assert_eq!(
            ctx.warnings.first().map(|warning| warning.code),
            Some(WarningCode::InvalidPrivacyConsent)
        );
    }

    #[test]
    fn cmp_deleted_date_in_future_check_passes() {
        let scheduled = Gcl::for_test(
            vec![CmpRecord {
                id: CMP_ID,
                deleted_date: Some(datetime!(2026-12-01 0:00:00 UTC)),
            }],
            datetime!(2027-01-01 0:00:00 UTC),
        );
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve_with(
            &req,
            &account_config(),
            &host_config(),
            &store(),
            &scheduled,
        );
        assert!(ctx.consent_usable);
        assert!(ctx.warnings.is_empty());
    }

    #[test]
    fn gcl_stale_consent_unusable_emits_10019() {
        let stale = Gcl::for_test(
            vec![CmpRecord {
                id: CMP_ID,
                deleted_date: None,
            }],
            datetime!(2026-01-01 0:00:00 UTC),
        );
        let req = with_consent(applies_request(), &fixture("valid_full_consent.txt"));
        let ctx = resolve_with(&req, &account_config(), &host_config(), &store(), &stale);
        assert!(!ctx.consent_usable);
        assert!(ctx.tcf.is_none());
        let warning = ctx.warnings.first().expect("one staleness warning");
        assert_eq!(warning.code, WarningCode::GclStale);
        assert_eq!(warning.code.as_u32(), 10019_u32);
    }

    #[test]
    fn core_only_string_consent_unusable() {
        // The disclosed-vendors segment is mandatory, so a core-only
        // string takes the malformed path before CMP authenticity is
        // ever consulted.
        let full = fixture("valid_full_consent.txt");
        let core_only = full.split('.').next().expect("a core segment").to_owned();
        let req = with_consent(applies_request(), &core_only);
        let ctx = resolve(&req, &account_config(), &host_config());
        assert!(!ctx.consent_usable);
        assert!(ctx.tcf.is_none());
        assert_eq!(
            ctx.warnings.first().and_then(|w| w.scope.as_deref()),
            Some("user.consent")
        );
    }

    #[test]
    fn dv_segment_present_string_usable() {
        let full = fixture("valid_full_consent.txt");
        assert!(full.contains('.'), "the fixture carries a trailing segment");
        let req = with_consent(applies_request(), &full);
        let ctx = resolve(&req, &account_config(), &host_config());
        assert!(ctx.consent_usable);
        assert!(ctx.warnings.is_empty());
    }

    #[test]
    fn channel_disabled_on_account_overrides_account_enabled_true() {
        let mut account = account_config();
        account.gdpr.enabled = TriState::Enabled;
        account.gdpr.channel_enabled.web = TriState::Disabled;
        let ctx = resolve(&applies_request(), &account, &host_config());
        assert!(!enforcing(&ctx));
    }

    #[test]
    fn channel_enabled_on_account_overrides_account_disabled() {
        let mut account = account_config();
        account.gdpr.enabled = TriState::Disabled;
        account.gdpr.channel_enabled.web = TriState::Enabled;
        let ctx = resolve(&applies_request(), &account, &host_config());
        assert!(enforcing(&ctx));
    }

    #[test]
    fn channel_unset_falls_through_to_account_enabled_enabled() {
        // The host default is off, so only the account rung can enable.
        let mut host = host_config();
        host.gdpr.tcf2.enabled = false;
        let mut account = account_config();
        account.gdpr.enabled = TriState::Enabled;
        let ctx = resolve(&applies_request(), &account, &host);
        assert!(enforcing(&ctx));
    }

    #[test]
    fn channel_unset_falls_through_to_account_enabled_disabled() {
        let mut account = account_config();
        account.gdpr.enabled = TriState::Disabled;
        let ctx = resolve(&applies_request(), &account, &host_config());
        assert!(!enforcing(&ctx), "the host default is never reached");
    }

    #[test]
    fn channel_and_account_both_unset_fall_through_to_host_tcf2_true() {
        let ctx = resolve(&applies_request(), &account_config(), &host_config());
        assert!(enforcing(&ctx));
    }

    #[test]
    fn channel_and_account_both_unset_fall_through_to_host_tcf2_false() {
        let mut host = host_config();
        host.gdpr.tcf2.enabled = false;
        let ctx = resolve(&applies_request(), &account_config(), &host);
        assert!(!enforcing(&ctx));
    }

    #[test]
    fn channel_derivation_matches_wire_shape_app_vs_site_vs_dooh() {
        let app = BidRequest {
            app: Some(App::default()),
            ..BidRequest::default()
        };
        assert_eq!(channel_of(&app), Channel::App);
        assert_eq!(channel_of(&site_request()), Channel::Web);
        let dooh = BidRequest {
            dooh: Some(Dooh::default()),
            ..BidRequest::default()
        };
        assert_eq!(channel_of(&dooh), Channel::Dooh);
    }

    #[test]
    fn ext_channel_web_does_not_override_app_inventory_privacy_gate() {
        // The GDPR-bypass regression: an app request claiming `web`
        // must not reach the disabled-web path.
        let req = BidRequest {
            site: None,
            app: Some(App::default()),
            ext: Some(ext_channel("web")),
            ..applies_request()
        };
        assert_eq!(channel_of(&req), Channel::App);

        let mut account = account_config();
        account.gdpr.channel_enabled.app = TriState::Enabled;
        account.gdpr.channel_enabled.web = TriState::Disabled;
        let ctx = resolve(&req, &account, &host_config());
        assert!(enforcing(&ctx));
    }

    #[test]
    fn ext_channel_name_ignored_when_absent_matches_inventory() {
        let req = site_request();
        assert!(req.ext.is_none());
        assert_eq!(channel_of(&req), Channel::Web);
    }

    #[test]
    fn dooh_inventory_selects_dooh_regardless_of_ext_channel() {
        let req = BidRequest {
            site: None,
            dooh: Some(Dooh::default()),
            ext: Some(ext_channel("app")),
            ..applies_request()
        };
        assert_eq!(channel_of(&req), Channel::Dooh);

        let mut account = account_config();
        account.gdpr.channel_enabled.dooh = TriState::Disabled;
        account.gdpr.channel_enabled.app = TriState::Enabled;
        let ctx = resolve(&req, &account, &host_config());
        assert!(!enforcing(&ctx), "the dooh key decides, not the extension");
    }

    #[test]
    fn amp_video_unreachable_on_auction_route() {
        // `amp` and `video` are route-derived and no such route exists;
        // an extension claiming one still classifies by inventory.
        for name in ["amp", "video"] {
            let req = BidRequest {
                ext: Some(ext_channel(name)),
                ..site_request()
            };
            assert_eq!(channel_of(&req), Channel::Web, "{name}");
        }
    }
}

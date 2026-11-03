//! Host and bidder Purpose 1 permission over one cached TCF/GVL selection.

use std::{borrow::Cow, mem};

use auctioneer_config_data::types::{
    EnforceAlgo, PurposeEnforcement, PurposeOneTreatment, TriState,
};

use super::{SyncDecision, SyncPrivacyContext, SyncTcfError, SyncerLookup};
use crate::{
    activity::{Activity, Component, activity_allowed},
    auction_context::PrivacyContext,
    gvl::{Gvl, gvl_spec_for_policy},
    purposes::{
        effective_flags_for_purpose, effective_publisher_restriction, purpose_allowed,
        purpose_allowed_basic, purpose_effective_algo,
    },
    signal::Signal,
    tcf::{PubRestrictionType, TcfParseError, TcfString, parse_tcf_v2},
};

pub(super) enum ConsentState {
    Unparsed,
    Empty,
    Malformed,
    ResourceLimit,
    Parsed(Box<TcfString>),
    Unauthenticated(Box<TcfString>),
    Authorized(Box<PrivacyContext>),
}

impl SyncPrivacyContext<'_> {
    /// Parse the selected TCF once; inactive permission paths never call this.
    ///
    /// # Errors
    /// A nonempty invalid TCF value produces a closed, payload-free error;
    /// parser ceiling exhaustion remains distinct from malformed consent.
    pub fn parse_selected_tcf(&mut self) -> Result<Option<&TcfString>, SyncTcfError> {
        if matches!(self.consent_state, ConsentState::Unparsed) {
            self.consent_state = match self.selected_consent.filter(|raw| !raw.is_empty()) {
                None => ConsentState::Empty,
                Some(raw) => match parse_tcf_v2(raw) {
                    Ok(tcf) => ConsentState::Parsed(Box::new(tcf)),
                    Err(error) if tcf_resource_limit(&error) => ConsentState::ResourceLimit,
                    Err(_error) => ConsentState::Malformed,
                },
            };
        }
        match &self.consent_state {
            ConsentState::Empty => Ok(None),
            ConsentState::Malformed | ConsentState::Unparsed => Err(SyncTcfError::Malformed),
            ConsentState::ResourceLimit => Err(SyncTcfError::ResourceLimit),
            ConsentState::Parsed(tcf) | ConsentState::Unauthenticated(tcf) => Ok(Some(tcf)),
            ConsentState::Authorized(ctx) => Ok(ctx.tcf.as_deref()),
        }
    }

    fn authenticate(&mut self) -> Result<&PrivacyContext, SyncDecision> {
        match self.parse_selected_tcf() {
            Err(SyncTcfError::Malformed) => return Err(SyncDecision::MalformedTcf),
            Err(SyncTcfError::ResourceLimit) => return Err(SyncDecision::ResourceLimit),
            Ok(None) => return Err(SyncDecision::BlockedByPrivacy),
            Ok(Some(_)) => {}
        }
        if let ConsentState::Parsed(tcf) = &self.consent_state {
            let valid_cmp =
                self.now < self.gcl.expires_at
                    && self.gcl.cmps.get(&tcf.cmp_id).is_some_and(|cmp| {
                        cmp.deleted_date.is_none_or(|deleted| deleted > self.now)
                    });
            let policy = u32::from(tcf.tcf_policy_version);
            let selected_gvl = if valid_cmp {
                self.gvl
                    .get(
                        gvl_spec_for_policy(policy),
                        u32::from(tcf.vendor_list_version),
                    )
                    .filter(|gvl| policy <= gvl.tcf_policy_version())
            } else {
                None
            };
            let ConsentState::Parsed(parsed_tcf) =
                mem::replace(&mut self.consent_state, ConsentState::Unparsed)
            else {
                return Err(SyncDecision::BlockedByPrivacy);
            };
            self.consent_state = match selected_gvl {
                Some(gvl) => {
                    ConsentState::Authorized(Box::new(self.policy_context(parsed_tcf, gvl)))
                }
                None => ConsentState::Unauthenticated(parsed_tcf),
            };
        }
        match &self.consent_state {
            ConsentState::Authorized(ctx) => Ok(ctx),
            ConsentState::Unauthenticated(_) | ConsentState::Empty => {
                Err(SyncDecision::BlockedByPrivacy)
            }
            ConsentState::Malformed => Err(SyncDecision::MalformedTcf),
            ConsentState::ResourceLimit => Err(SyncDecision::ResourceLimit),
            ConsentState::Parsed(_) | ConsentState::Unparsed => Err(SyncDecision::BlockedByPrivacy),
        }
    }

    fn policy_context(&self, tcf: Box<TcfString>, gvl: Gvl) -> PrivacyContext {
        // Purpose helpers read this state directly; no auction request or PassID is synthesized.
        let mut ctx = PrivacyContext::allow_all_no_scrub();
        ctx.authorization_now = self.now;
        ctx.enforcement_active = true;
        ctx.consent_usable = true;
        ctx.per_purpose_effective
            .insert(1, effective_purpose_one(self));
        ctx.basic_enforcement_vendors = self
            .account
            .gdpr
            .basic_enforcement_vendors
            .iter()
            .cloned()
            .collect();
        ctx.tcf = Some(tcf);
        ctx.gvl = gvl;
        ctx
    }

    fn vendor_permission(&mut self, raw_vendor_id: u32, base_name: Option<&str>) -> SyncDecision {
        let treatment = self.effective_treatment();
        let now = self.now;
        let ctx = match self.authenticate() {
            Ok(ctx) => ctx,
            Err(decision) => return decision,
        };
        let Ok(vendor_id) = u16::try_from(raw_vendor_id) else {
            return SyncDecision::BlockedByPrivacy;
        };
        if vendor_id == 0 {
            return SyncDecision::BlockedByPrivacy;
        }
        let Some(vendor) = ctx.gvl.vendor(vendor_id) else {
            return SyncDecision::BlockedByPrivacy;
        };
        if vendor.is_deleted_at(now) {
            return SyncDecision::BlockedByPrivacy;
        }
        let Some(tcf) = ctx.tcf.as_deref() else {
            return SyncDecision::BlockedByPrivacy;
        };
        if effective_publisher_restriction(Some(tcf), 1, vendor_id)
            == Some(PubRestrictionType::NotAllowed)
        {
            return SyncDecision::BlockedByPrivacy;
        }
        let flags = effective_flags_for_purpose(ctx, 1);
        if !flags.enforce_purpose {
            return SyncDecision::Allowed;
        }
        if tcf.purpose_one_treatment && treatment.enabled {
            return if treatment.access_allowed {
                SyncDecision::Allowed
            } else {
                SyncDecision::BlockedByPrivacy
            };
        }
        // Purpose helpers take a bidder name; the host must not match either
        // configured bidder-only exception list.
        let core_name: Cow<'_, str> = match base_name {
            Some(name) => Cow::Borrowed(name),
            None => Cow::Owned(host_policy_name(ctx)),
        };
        let allowed = match purpose_effective_algo(ctx, 1, core_name.as_ref()) {
            EnforceAlgo::Full => purpose_allowed(ctx, 1, core_name.as_ref(), vendor_id, flags),
            EnforceAlgo::Basic => {
                purpose_allowed_basic(ctx, 1, core_name.as_ref(), vendor_id, flags, false)
            }
        };
        if allowed {
            SyncDecision::Allowed
        } else {
            SyncDecision::BlockedByPrivacy
        }
    }

    fn effective_treatment(&self) -> PurposeOneTreatment {
        let host = self.host.gdpr.tcf2.purpose_one_treatment;
        let account = self.account.gdpr.purpose_one_treatment;
        PurposeOneTreatment {
            enabled: account.enabled.unwrap_or(host.enabled),
            access_allowed: account.access_allowed.unwrap_or(host.access_allowed),
        }
    }

    #[must_use]
    pub fn cookie_sync_host_permission(&mut self) -> SyncDecision {
        if !self.gdpr_permissions_enabled || self.normalized_request_signal != Signal::Yes {
            return SyncDecision::Allowed;
        }
        let host_vendor = self.host.gdpr.host_vendor_id.unwrap_or(0);
        if host_vendor == 0 {
            return SyncDecision::Allowed;
        }
        self.vendor_permission(host_vendor, None)
    }

    #[must_use]
    pub fn cookie_sync_candidate_permission(&mut self, lookup: &SyncerLookup<'_>) -> SyncDecision {
        if !activity_allowed(
            &self.account.privacy.allowactivities,
            Activity::SyncUser,
            Component::Bidder(lookup.normalized_name),
        ) {
            return SyncDecision::BlockedByActivity;
        }
        let host_permission = self.cookie_sync_host_permission();
        if host_permission != SyncDecision::Allowed {
            return if host_permission == SyncDecision::ResourceLimit {
                host_permission
            } else {
                SyncDecision::BlockedByPrivacy
            };
        }
        if self.gdpr_permissions_enabled && self.normalized_request_signal == Signal::Yes {
            let bidder_vendor = lookup.bidder.gvl_vendor_id.unwrap_or(0);
            let decision = self.vendor_permission(bidder_vendor, Some(lookup.base_name));
            if decision == SyncDecision::ResourceLimit {
                return decision;
            }
            if decision != SyncDecision::Allowed {
                return SyncDecision::BlockedByPrivacy;
            }
        }
        if !lookup.syncer.enabled {
            return SyncDecision::BlockedByDisabledUsersync;
        }
        if (lookup.syncer.skip_when.gdpr && self.parsed_request_signal == Signal::Yes)
            || self.raw_gpp_sid.is_some_and(|sid| {
                lookup
                    .syncer
                    .skip_when
                    .gpp_sid
                    .iter()
                    .any(|configured| configured == sid)
            })
        {
            return SyncDecision::BlockedByRegulationScope;
        }
        SyncDecision::Allowed
    }

    #[must_use]
    pub fn setuid_permission(&mut self, lookup: &SyncerLookup<'_>) -> SyncDecision {
        if !activity_allowed(
            &self.account.privacy.allowactivities,
            Activity::SyncUser,
            Component::Bidder(lookup.normalized_name),
        ) {
            return SyncDecision::BlockedByActivity;
        }
        let host = self.cookie_sync_host_permission();
        if host != SyncDecision::Allowed {
            return host;
        }
        if !lookup.syncer.enabled {
            return SyncDecision::BlockedByDisabledUsersync;
        }
        SyncDecision::Allowed
    }
}

fn tcf_resource_limit(error: &TcfParseError) -> bool {
    matches!(
        error,
        TcfParseError::InputTooLong(_)
            | TcfParseError::DecodedTooLong(_)
            | TcfParseError::TooManyTrailingSegments { .. }
            | TcfParseError::TooManyRangeEntries { .. }
            | TcfParseError::TooManyPubRestrictions { .. }
            | TcfParseError::TooManyPublisherRestrictions
            | TcfParseError::OpBudgetExceeded
    )
}

fn host_policy_name(ctx: &PrivacyContext) -> String {
    let mut name = String::new();
    while ctx.basic_enforcement_vendors.contains(name.as_str())
        || ctx.per_purpose_effective.get(&1).is_some_and(|purpose| {
            purpose
                .vendor_exceptions
                .iter()
                .any(|exception| exception == &name)
        })
    {
        name.push('\0');
    }
    name
}

fn effective_purpose_one(input: &SyncPrivacyContext<'_>) -> PurposeEnforcement {
    let host = input
        .host
        .gdpr
        .tcf2
        .purposes
        .get(&1)
        .cloned()
        .unwrap_or_default();
    let Some(account) = input.account.gdpr.purposes.get(&1) else {
        return host;
    };
    PurposeEnforcement {
        enforce_algo: account.enforce_algo,
        enforce_purpose: tri_or(account.enforce_purpose, host.enforce_purpose),
        enforce_vendors: tri_or(account.enforce_vendors, host.enforce_vendors),
        vendor_exceptions: account.vendor_exceptions.clone(),
    }
}

fn tri_or(value: TriState, fallback: TriState) -> TriState {
    if value == TriState::Unset {
        fallback
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::{eq, from_ref};

    use auctioneer_config_data::types::{AccountConfig, BidderInfo, EnforceAlgo, HostConfig};
    use serde_json::json;
    use time::{OffsetDateTime, macros::datetime};

    use super::{ConsentState, PurposeEnforcement, SyncDecision, TriState};
    use crate::{
        GdprSignalIn, SyncEndpoint, SyncPrivacyContext, SyncPrivacyInput, SyncerLookup,
        auction_context::PrivacyContext,
        gcl::{CmpRecord, Gcl},
        gvl::{GVL_PARSE_CACHE_MAX_ENTRIES, Gvl, GvlStore},
        resolve_sync_privacy,
        test_support::tc_encode,
    };

    const NOW: OffsetDateTime = datetime!(2026-09-01 00:00 UTC);
    const HOST_VENDOR: u16 = 1;
    const CANDIDATE_VENDOR: u16 = 2;

    #[test]
    fn host_policy_name_excludes_each_bidder_list_independently() {
        let lists: [&[&str]; 4] = [&[], &[""], &["", "\0"], &["\0"]];
        for basic in lists {
            for exceptions in lists {
                let mut ctx = PrivacyContext::allow_all_no_scrub();
                ctx.basic_enforcement_vendors =
                    basic.iter().map(|name| (*name).to_owned()).collect();
                ctx.per_purpose_effective.insert(
                    1,
                    PurposeEnforcement {
                        vendor_exceptions: exceptions
                            .iter()
                            .map(|name| (*name).to_owned())
                            .collect(),
                        ..Default::default()
                    },
                );
                let expected = if basic.contains(&"") || exceptions.contains(&"") {
                    if basic.contains(&"\0") || exceptions.contains(&"\0") {
                        "\0\0"
                    } else {
                        "\0"
                    }
                } else {
                    ""
                };
                assert_eq!(
                    super::host_policy_name(&ctx),
                    expected,
                    "basic={basic:?} exceptions={exceptions:?}"
                );
            }
        }
    }

    fn configured_host() -> HostConfig {
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.host_vendor_id = Some(u32::from(HOST_VENDOR));
        host.gdpr.tcf2.purposes.insert(
            1,
            PurposeEnforcement {
                enforce_purpose: TriState::Enabled,
                enforce_vendors: TriState::Enabled,
                ..Default::default()
            },
        );
        host
    }

    fn encoded_consent(list_version: u32) -> String {
        let mut spec = tc_encode::valid_core_spec();
        spec.vendor_list_version = u16::try_from(list_version).expect("TCF list version fits");
        spec.vendor_consent = tc_encode::VendorSectionSpec::Bitfield {
            max_vendor_id: CANDIDATE_VENDOR,
            set: vec![HOST_VENDOR, CANDIDATE_VENDOR],
        };
        tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec))
    }

    fn authorized_gvl<'a>(context: &'a SyncPrivacyContext<'_>) -> &'a Gvl {
        let ConsentState::Authorized(authorized) = &context.consent_state else {
            panic!("permission must authenticate and retain the selected GVL");
        };
        &authorized.gvl
    }

    fn assert_cookie_sync_allowed(context: &mut SyncPrivacyContext<'_>, lookup: &SyncerLookup<'_>) {
        assert_eq!(context.cookie_sync_host_permission(), SyncDecision::Allowed);
        assert_eq!(
            context.cookie_sync_candidate_permission(lookup),
            SyncDecision::Allowed
        );
    }

    fn permission_store(deleted_vendor: Option<u16>) -> GvlStore {
        let deleted_date =
            |vendor| (deleted_vendor == Some(vendor)).then_some("2026-09-01T00:00:00Z");
        let gvl = Gvl::load_from_json(
            &json!({
                "gvlSpecificationVersion": 3_i32, "tcfPolicyVersion": 5_i32,
                "vendorListVersion": 348_i32, "lastUpdated": "2026-07-01T00:00:00Z",
                "purposes": {"1": {"id": 1_i32, "name": "Storage"}},
                "specialPurposes": {}, "features": {}, "specialFeatures": {},
                "vendors": {
                    "1": {"id": 1_i32, "name": "Host", "purposes": [1_i32],
                        "legIntPurposes": [], "flexiblePurposes": [], "specialFeatures": [],
                        "deletedDate": deleted_date(HOST_VENDOR)},
                    "2": {"id": 2_i32, "name": "Candidate", "purposes": [1_i32],
                        "legIntPurposes": [], "flexiblePurposes": [], "specialFeatures": [],
                        "deletedDate": deleted_date(CANDIDATE_VENDOR)}
                }
            })
            .to_string(),
            None,
        )
        .expect("valid permission GVL");
        GvlStore::for_test(vec![(3, 348, gvl)])
    }

    fn permission_gcl() -> Gcl {
        Gcl::for_test(
            vec![CmpRecord {
                id: tc_encode::valid_core_spec().cmp_id,
                deleted_date: None,
            }],
            datetime!(2027-01-01 00:00 UTC),
        )
    }

    fn permission_context<'a>(
        account: &'a AccountConfig,
        host: &'a HostConfig,
        store: &'a GvlStore,
        gcl: &'a Gcl,
        consent: &'a str,
    ) -> SyncPrivacyContext<'a> {
        resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(1),
            consent: Some(consent),
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account,
            host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: store,
            gcl,
            now: NOW,
        })
        .expect("valid permission extraction")
    }

    fn permission_spec() -> tc_encode::CoreSpec {
        let mut spec = tc_encode::valid_core_spec();
        spec.purposes_consent.clear();
        spec.vendor_consent = tc_encode::VendorSectionSpec::Bitfield {
            max_vendor_id: CANDIDATE_VENDOR,
            set: vec![],
        };
        spec
    }

    fn restrict_purpose_one(spec: &mut tc_encode::CoreSpec, vendor: u16, types: &[u8]) {
        spec.pub_restrictions = types
            .iter()
            .map(|restriction_type| tc_encode::PubRestrictionSpec {
                purpose_id: 1,
                restriction_type: *restriction_type,
                entries: vec![tc_encode::RangeEntrySpec::Single(vendor)],
            })
            .collect();
    }

    fn host_for_permission(vendor: u16, enforcement: PurposeEnforcement) -> HostConfig {
        let mut host = configured_host();
        if vendor == CANDIDATE_VENDOR {
            host.gdpr.host_vendor_id = Some(0);
        }
        host.gdpr.tcf2.purposes.insert(1, enforcement);
        host
    }

    fn permission_lookup(bidder: &BidderInfo) -> SyncerLookup<'_> {
        SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        }
    }

    fn assert_vendor_permission(
        context: &mut SyncPrivacyContext<'_>,
        vendor: u16,
        expected: SyncDecision,
    ) {
        let bidder = BidderInfo {
            gvl_vendor_id: Some(u32::from(CANDIDATE_VENDOR)),
            ..Default::default()
        };
        let lookup = permission_lookup(&bidder);
        if vendor == HOST_VENDOR {
            assert_eq!(context.cookie_sync_host_permission(), expected, "host");
            assert_eq!(context.setuid_permission(&lookup), expected, "setuid host");
        } else {
            assert_eq!(context.cookie_sync_host_permission(), SyncDecision::Allowed);
            assert_eq!(
                context.cookie_sync_candidate_permission(&lookup),
                expected,
                "candidate"
            );
        }
    }

    #[test]
    fn publisher_basis_conflicts_deny_before_disabled_purpose_grants() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for enforce_algo in [EnforceAlgo::Full, EnforceAlgo::Basic] {
                for enforce_vendors in [TriState::Disabled, TriState::Enabled] {
                    let host = host_for_permission(
                        vendor,
                        PurposeEnforcement {
                            enforce_algo,
                            enforce_purpose: TriState::Disabled,
                            enforce_vendors,
                            ..Default::default()
                        },
                    );
                    for types in [[1, 2], [2, 1]] {
                        let mut spec = permission_spec();
                        restrict_purpose_one(&mut spec, vendor, &types);
                        let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
                        let mut context =
                            permission_context(&account, &host, &store, &gcl, &consent);
                        assert_vendor_permission(
                            &mut context,
                            vendor,
                            SyncDecision::BlockedByPrivacy,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn publisher_basis_conflicts_deny_before_treatment_grants() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for enforce_algo in [EnforceAlgo::Full, EnforceAlgo::Basic] {
                let host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_algo,
                        enforce_purpose: TriState::Enabled,
                        enforce_vendors: TriState::Enabled,
                        ..Default::default()
                    },
                );
                for types in [[1, 2], [2, 1]] {
                    let mut spec = permission_spec();
                    spec.purpose_one_treatment = true;
                    restrict_purpose_one(&mut spec, vendor, &types);
                    let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
                    let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                    assert_vendor_permission(&mut context, vendor, SyncDecision::BlockedByPrivacy);
                }
            }
        }
    }

    #[test]
    fn publisher_not_allowed_denies_before_purpose_bypasses() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for (enforce_purpose, enforce_vendors, treatment) in [
                (TriState::Disabled, TriState::Disabled, false),
                (TriState::Disabled, TriState::Enabled, false),
                (TriState::Enabled, TriState::Enabled, true),
            ] {
                let host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_purpose,
                        enforce_vendors,
                        ..Default::default()
                    },
                );
                let mut spec = permission_spec();
                spec.purpose_one_treatment = treatment;
                restrict_purpose_one(&mut spec, vendor, &[0]);
                let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
                let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                assert_vendor_permission(&mut context, vendor, SyncDecision::BlockedByPrivacy);
            }
        }
    }

    #[test]
    fn duplicate_same_basis_restrictions_preserve_purpose_bypasses() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for (enforce_purpose, enforce_vendors, treatment) in [
                (TriState::Disabled, TriState::Disabled, false),
                (TriState::Disabled, TriState::Enabled, false),
                (TriState::Enabled, TriState::Enabled, true),
            ] {
                let host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_purpose,
                        enforce_vendors,
                        ..Default::default()
                    },
                );
                for restriction_type in [1, 2] {
                    let mut spec = permission_spec();
                    spec.purpose_one_treatment = treatment;
                    restrict_purpose_one(&mut spec, vendor, &[restriction_type, restriction_type]);
                    let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
                    let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                    assert_vendor_permission(&mut context, vendor, SyncDecision::Allowed);
                }
            }
        }
    }

    #[test]
    fn disabled_purpose_grants_with_vendor_enforcement_enabled() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&permission_spec()));
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for enforce_algo in [EnforceAlgo::Full, EnforceAlgo::Basic] {
                let host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_algo,
                        enforce_purpose: TriState::Disabled,
                        enforce_vendors: TriState::Enabled,
                        ..Default::default()
                    },
                );
                let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                assert_vendor_permission(&mut context, vendor, SyncDecision::Allowed);
            }
        }
    }

    #[test]
    fn treatment_denial_does_not_reverse_disabled_purpose_grant() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        let mut spec = permission_spec();
        spec.purpose_one_treatment = true;
        let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for enforce_vendors in [TriState::Disabled, TriState::Enabled] {
                let mut host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_purpose: TriState::Disabled,
                        enforce_vendors,
                        ..Default::default()
                    },
                );
                host.gdpr.tcf2.purpose_one_treatment.access_allowed = false;
                let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                assert_vendor_permission(&mut context, vendor, SyncDecision::Allowed);
            }
        }
    }

    #[test]
    fn disabled_vendor_enforcement_still_requires_enabled_purpose_consent() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for enforce_algo in [EnforceAlgo::Full, EnforceAlgo::Basic] {
                let host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_algo,
                        enforce_purpose: TriState::Enabled,
                        enforce_vendors: TriState::Disabled,
                        ..Default::default()
                    },
                );
                for consented in [false, true] {
                    let mut spec = permission_spec();
                    if consented {
                        spec.purposes_consent = vec![1];
                    }
                    let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
                    let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                    let expected = if consented {
                        SyncDecision::Allowed
                    } else {
                        SyncDecision::BlockedByPrivacy
                    };
                    assert_vendor_permission(&mut context, vendor, expected);
                }
            }
        }
    }

    #[test]
    fn purpose_flags_resolve_account_over_host_per_field() {
        let store = permission_store(None);
        let gcl = permission_gcl();
        let enabled = TriState::Enabled;
        let disabled = TriState::Disabled;
        let unset = TriState::Unset;
        let allowed = SyncDecision::Allowed;
        let blocked = SyncDecision::BlockedByPrivacy;
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for (
                [host_purpose, host_vendors],
                [account_purpose, account_vendors],
                [purpose_bit, vendor_bit],
                expected,
            ) in [
                (
                    [enabled, enabled],
                    [disabled, unset],
                    [false, false],
                    allowed,
                ),
                (
                    [disabled, enabled],
                    [enabled, unset],
                    [false, true],
                    blocked,
                ),
                (
                    [enabled, enabled],
                    [unset, disabled],
                    [true, false],
                    allowed,
                ),
                (
                    [enabled, disabled],
                    [unset, enabled],
                    [true, false],
                    blocked,
                ),
                (
                    [disabled, disabled],
                    [unset, enabled],
                    [false, false],
                    allowed,
                ),
                ([enabled, enabled], [unset, unset], [false, false], blocked),
                ([disabled, enabled], [unset, unset], [false, false], allowed),
            ] {
                let host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_purpose: host_purpose,
                        enforce_vendors: host_vendors,
                        ..Default::default()
                    },
                );
                let mut account = AccountConfig::default();
                account.gdpr.purposes.insert(
                    1,
                    PurposeEnforcement {
                        enforce_purpose: account_purpose,
                        enforce_vendors: account_vendors,
                        ..Default::default()
                    },
                );
                let mut spec = permission_spec();
                if purpose_bit {
                    spec.purposes_consent = vec![1];
                }
                if vendor_bit {
                    spec.vendor_consent = tc_encode::VendorSectionSpec::Bitfield {
                        max_vendor_id: CANDIDATE_VENDOR,
                        set: vec![HOST_VENDOR, CANDIDATE_VENDOR],
                    };
                }
                let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
                let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                assert_vendor_permission(&mut context, vendor, expected);
            }
        }
    }

    #[test]
    fn treatment_fields_resolve_account_over_host_independently() {
        let store = permission_store(None);
        let gcl = permission_gcl();
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for host_enabled in [false, true] {
                for host_access in [false, true] {
                    let mut host = configured_host();
                    if vendor == CANDIDATE_VENDOR {
                        host.gdpr.host_vendor_id = Some(0);
                    }
                    host.gdpr.tcf2.purpose_one_treatment.enabled = host_enabled;
                    host.gdpr.tcf2.purpose_one_treatment.access_allowed = host_access;
                    for account_enabled in [None, Some(false), Some(true)] {
                        for account_access in [None, Some(false), Some(true)] {
                            let mut account = AccountConfig::default();
                            account.gdpr.purpose_one_treatment.enabled = account_enabled;
                            account.gdpr.purpose_one_treatment.access_allowed = account_access;
                            for treatment in [false, true] {
                                let mut spec = permission_spec();
                                spec.purpose_one_treatment = treatment;
                                let consent =
                                    tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
                                let mut context =
                                    permission_context(&account, &host, &store, &gcl, &consent);
                                let expected = if treatment
                                    && account_enabled.unwrap_or(host_enabled)
                                    && account_access.unwrap_or(host_access)
                                {
                                    SyncDecision::Allowed
                                } else {
                                    SyncDecision::BlockedByPrivacy
                                };
                                assert_vendor_permission(&mut context, vendor, expected);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn purpose_bypasses_do_not_skip_consent_authentication_or_vendor_deletion() {
        let account = AccountConfig::default();
        for vendor in [HOST_VENDOR, CANDIDATE_VENDOR] {
            for (enforce_purpose, enforce_vendors, treatment) in [
                (TriState::Disabled, TriState::Disabled, false),
                (TriState::Disabled, TriState::Enabled, false),
                (TriState::Enabled, TriState::Enabled, true),
            ] {
                let host = host_for_permission(
                    vendor,
                    PurposeEnforcement {
                        enforce_purpose,
                        enforce_vendors,
                        vendor_exceptions: vec!["fake_base".into()],
                        ..Default::default()
                    },
                );
                for case in [
                    "malformed",
                    "stale-gcl",
                    "missing-cmp",
                    "deleted-cmp",
                    "missing-gvl",
                    "old-policy",
                    "deleted-vendor",
                ] {
                    let store = if case == "missing-gvl" {
                        GvlStore::for_test(vec![])
                    } else {
                        permission_store((case == "deleted-vendor").then_some(vendor))
                    };
                    let gcl = match case {
                        "stale-gcl" => Gcl::for_test(
                            vec![CmpRecord {
                                id: 300,
                                deleted_date: None,
                            }],
                            NOW,
                        ),
                        "missing-cmp" => Gcl::for_test(vec![], datetime!(2027-01-01 00:00 UTC)),
                        "deleted-cmp" => Gcl::for_test(
                            vec![CmpRecord {
                                id: 300,
                                deleted_date: Some(NOW),
                            }],
                            datetime!(2027-01-01 00:00 UTC),
                        ),
                        _ => permission_gcl(),
                    };
                    let mut spec = permission_spec();
                    spec.purpose_one_treatment = treatment;
                    if case == "old-policy" {
                        spec.tcf_policy_version = 6;
                    }
                    let consent = if case == "malformed" {
                        "malformed-consent".into()
                    } else {
                        tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec))
                    };
                    let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                    let expected = if case == "malformed" && vendor == HOST_VENDOR {
                        SyncDecision::MalformedTcf
                    } else {
                        SyncDecision::BlockedByPrivacy
                    };
                    assert_vendor_permission(&mut context, vendor, expected);
                }
            }
        }
    }

    #[test]
    fn host_vendor_zero_does_not_grant_unknown_candidate_vendor() {
        let account = AccountConfig::default();
        let store = permission_store(None);
        let gcl = permission_gcl();
        let mut spec = permission_spec();
        spec.purpose_one_treatment = true;
        let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
        for enforce_purpose in [TriState::Disabled, TriState::Enabled] {
            let host = host_for_permission(
                CANDIDATE_VENDOR,
                PurposeEnforcement {
                    enforce_purpose,
                    enforce_vendors: TriState::Enabled,
                    vendor_exceptions: vec!["fake_base".into()],
                    ..Default::default()
                },
            );
            for vendor_id in [None, Some(0), Some(3), Some(0x0001_0000), Some(u32::MAX)] {
                let bidder = BidderInfo {
                    gvl_vendor_id: vendor_id,
                    ..Default::default()
                };
                let lookup = permission_lookup(&bidder);
                let mut context = permission_context(&account, &host, &store, &gcl, &consent);
                assert_eq!(context.cookie_sync_host_permission(), SyncDecision::Allowed);
                assert_eq!(context.setuid_permission(&lookup), SyncDecision::Allowed);
                assert_eq!(
                    context.cookie_sync_candidate_permission(&lookup),
                    SyncDecision::BlockedByPrivacy,
                    "vendor={vendor_id:?}"
                );
            }
        }
    }

    #[test]
    fn authenticated_sync_context_retains_selected_gvl_after_lru_eviction() {
        let selected = auctioneer_config_data::gvl_archive()
            .iter()
            .filter(|entry| entry.spec == 3)
            .min_by_key(|entry| entry.vendor_list_version)
            .expect("archive has a spec-3 list");
        let store = GvlStore::fresh_baked_for_test();
        let account = AccountConfig::default();
        let host = configured_host();
        let bidder = BidderInfo {
            gvl_vendor_id: Some(u32::from(CANDIDATE_VENDOR)),
            ..BidderInfo::default()
        };
        let lookup = SyncerLookup {
            requested_name: "fake_alias",
            normalized_name: "fake_alias",
            base_name: "fake_base",
            bidder: &bidder,
            syncer: &bidder.user_sync,
            syncer_key: "family",
        };
        let gcl = Gcl::for_test(
            vec![CmpRecord {
                id: tc_encode::valid_core_spec().cmp_id,
                deleted_date: None,
            }],
            datetime!(2027-01-01 00:00 UTC),
        );
        let consent = encoded_consent(selected.vendor_list_version);
        let mut context = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(1),
            consent: Some(&consent),
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &store,
            gcl: &gcl,
            now: NOW,
        })
        .expect("valid sync privacy input");
        assert_cookie_sync_allowed(&mut context, &lookup);
        let original_vendor = from_ref(authorized_gvl(&context).vendor(HOST_VENDOR).unwrap());
        let cached = store
            .get(selected.spec, selected.vendor_list_version)
            .expect("selected list is cached");
        assert_eq!(
            from_ref(cached.vendor(HOST_VENDOR).unwrap()),
            original_vendor
        );
        drop(cached);

        // Only the context holds the selected request GVL while the real LRU rotates.
        let mut fetched = 0_usize;
        for entry in auctioneer_config_data::gvl_archive()
            .iter()
            .filter(|entry| {
                entry.spec == selected.spec
                    && entry.vendor_list_version != selected.vendor_list_version
            })
            .take(GVL_PARSE_CACHE_MAX_ENTRIES)
        {
            let other = store
                .get(entry.spec, entry.vendor_list_version)
                .expect("another archive list parses");
            assert_eq!(other.vendor_list_version(), entry.vendor_list_version);
            fetched = fetched.saturating_add(1);
        }
        assert_eq!(fetched, GVL_PARSE_CACHE_MAX_ENTRIES);

        assert_cookie_sync_allowed(&mut context, &lookup);
        assert_eq!(
            from_ref(authorized_gvl(&context).vendor(HOST_VENDOR).unwrap()),
            original_vendor
        );
        let reparsed = store
            .get(selected.spec, selected.vendor_list_version)
            .expect("evicted archive list reparses");
        assert_ne!(
            from_ref(reparsed.vendor(HOST_VENDOR).unwrap()),
            original_vendor,
            "cache miss must parse a new object while the context pins the original"
        );
        assert_cookie_sync_allowed(&mut context, &lookup);
        let retained = authorized_gvl(&context);
        assert_eq!(
            from_ref(retained.vendor(HOST_VENDOR).unwrap()),
            original_vendor
        );
        assert_eq!(retained.gvl_specification_version(), selected.spec);
        assert_eq!(retained.vendor_list_version(), selected.vendor_list_version);
        assert_eq!(retained.vendor(HOST_VENDOR), reparsed.vendor(HOST_VENDOR));
        let cached_again = store
            .get(selected.spec, selected.vendor_list_version)
            .expect("reparsed list is now cached");
        assert!(eq(
            reparsed.vendor(HOST_VENDOR).unwrap(),
            cached_again.vendor(HOST_VENDOR).unwrap()
        ));
    }
}

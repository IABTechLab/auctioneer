//! Deterministic, bounded syncer selection over a request-scoped catalog.

use std::collections::BTreeSet;

use auctioneer_config_data::types::{BidderInfo, SyncType};
use auctioneer_privacy::{SyncDecision, SyncPrivacyContext, SyncerLookup};
use auctioneer_runtime::{
    Rng,
    aux_work::{
        MAX_SYNC_DEBUG_ROWS, MAX_SYNC_FILTER_NAMES_PER_LIST, MAX_SYNC_PRIORITY_ENTRIES,
        MAX_SYNC_PRIORITY_GROUPS, MAX_SYNC_REQUESTED_NAMES,
    },
    shuffle,
};
use serde_json::Value;
use time::OffsetDateTime;

use super::{
    catalog::Catalog,
    cookie::UidCookie,
    wire::{FilterBiddersWire, FilterRuleWire, FilterSettingsWire},
};

const MAX_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterError {
    InvalidShape,
    InvalidName,
    TooManyNames,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChooseError {
    InvalidName,
    TooManyNames,
    TooManyGroups,
    PrivacyResourceLimit,
}

#[derive(Clone, Debug)]
enum NameSet {
    All,
    Names(BTreeSet<String>),
}

impl NameSet {
    fn matches(&self, name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Names(names) => names.contains(name),
        }
    }
}

#[derive(Clone, Debug)]
struct TypeFilter {
    include: bool,
    bidders: NameSet,
}

impl TypeFilter {
    fn allows(&self, name: &str) -> bool {
        self.bidders.matches(name) == self.include
    }

    fn from_wire(rule: &FilterRuleWire) -> Result<Self, FilterError> {
        let include = match rule.filter.as_deref() {
            Some("include") => true,
            Some("exclude") => false,
            _ => return Err(FilterError::InvalidShape),
        };
        let bidders = match rule.bidders.as_ref() {
            Some(FilterBiddersWire::Literal(all)) if all == "*" => NameSet::All,
            Some(FilterBiddersWire::Names(values)) => {
                if u32::try_from(values.len())
                    .ok()
                    .is_none_or(|count| count > MAX_SYNC_FILTER_NAMES_PER_LIST)
                {
                    return Err(FilterError::TooManyNames);
                }
                let mut names = BTreeSet::new();
                for value in values {
                    if !valid_name(value) {
                        return Err(FilterError::InvalidName);
                    }
                    names.insert(value.to_ascii_lowercase());
                }
                NameSet::Names(names)
            }
            _ => return Err(FilterError::InvalidShape),
        };
        Ok(Self { include, bidders })
    }
}

#[derive(Clone, Debug, Default)]
pub struct FilterSettings {
    iframe: Option<TypeFilter>,
    image: Option<TypeFilter>,
}

impl FilterSettings {
    /// # Errors
    /// Rejects invalid filter modes, bidder names, or excessive name lists.
    pub fn from_wire(value: &FilterSettingsWire) -> Result<Self, FilterError> {
        Ok(Self {
            iframe: value
                .iframe
                .as_ref()
                .map(TypeFilter::from_wire)
                .transpose()?,
            image: value
                .image
                .as_ref()
                .map(TypeFilter::from_wire)
                .transpose()?,
        })
    }

    /// Parse the wire `filterSettings` object. Redirect syncs use `image`.
    ///
    /// # Errors
    /// Rejects unknown fields, invalid rules, or excessive/invalid bidder names.
    pub fn parse(value: Option<&Value>) -> Result<Self, FilterError> {
        let Some(raw) = value else {
            return Ok(Self::default());
        };
        let object = raw.as_object().ok_or(FilterError::InvalidShape)?;
        if object.len() > 2 || object.keys().any(|key| key != "iframe" && key != "image") {
            return Err(FilterError::InvalidShape);
        }
        for rule in object.values() {
            let fields = rule.as_object().ok_or(FilterError::InvalidShape)?;
            if fields.len() != 2
                || !fields.contains_key("filter")
                || !fields.contains_key("bidders")
            {
                return Err(FilterError::InvalidShape);
            }
        }
        let wire: FilterSettingsWire =
            serde_json::from_value(raw.clone()).map_err(|_error| FilterError::InvalidShape)?;
        Self::from_wire(&wire)
    }

    fn allows(&self, sync_type: SyncType, normalized_name: &str) -> bool {
        let filter = match sync_type {
            SyncType::Iframe => &self.iframe,
            SyncType::Redirect => &self.image,
        };
        filter
            .as_ref()
            .is_none_or(|rule| rule.allows(normalized_name))
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name.bytes().all(|byte| byte >= 0x21 && byte != 0x7f)
}

/// Raw ordering without catalog eligibility policy, used by reference controls.
/// Group order is fixed; each group is shuffled separately. Duplicates are
/// deliberately retained here so the selection ladder owns deduplication.
#[must_use]
pub fn candidate_order<'a>(
    requested: &'a [String],
    cooperative: bool,
    priority_groups: &'a [Vec<String>],
    available: &[&'a str],
    rng: &dyn Rng,
) -> Vec<&'a str> {
    candidate_order_filtered(
        requested,
        cooperative,
        priority_groups,
        available,
        rng,
        |_| true,
    )
}

fn candidate_order_filtered<'a>(
    requested: &'a [String],
    cooperative: bool,
    priority_groups: &'a [Vec<String>],
    available: &[&'a str],
    rng: &dyn Rng,
    automatic_candidate: impl Fn(&str) -> bool,
) -> Vec<&'a str> {
    let mut names = Vec::with_capacity(
        requested
            .len()
            .saturating_add(if cooperative {
                priority_groups
                    .iter()
                    .map(Vec::len)
                    .fold(0_usize, usize::saturating_add)
            } else {
                0
            })
            .saturating_add(if requested.is_empty() || cooperative {
                available.len()
            } else {
                0
            }),
    );
    if !requested.is_empty() {
        let mut group = requested.iter().map(String::as_str).collect::<Vec<_>>();
        shuffle(&mut group, rng);
        names.extend(group);
    }
    if cooperative {
        for priority in priority_groups {
            // Preserve the existing capacity owner even when filtering lowers
            // the iterator's size hint and would otherwise permit Vec growth.
            let mut group = Vec::with_capacity(priority.len());
            group.extend(
                priority
                    .iter()
                    .map(String::as_str)
                    .filter(|name| automatic_candidate(name)),
            );
            shuffle(&mut group, rng);
            names.extend(group);
        }
    }
    if requested.is_empty() || cooperative {
        let mut group = Vec::with_capacity(available.len());
        group.extend(
            available
                .iter()
                .copied()
                .filter(|name| automatic_candidate(name)),
        );
        group.sort_unstable();
        shuffle(&mut group, rng);
        names.extend(group);
    }
    names
}

#[derive(Clone, Copy)]
pub struct ChooseInput<'a> {
    pub catalog: &'a Catalog<'a>,
    pub requested: &'a [String],
    pub cooperative: bool,
    pub priority_groups: &'a [Vec<String>],
    pub filters: &'a FilterSettings,
    pub cookie: &'a UidCookie,
    pub now: OffsetDateTime,
    pub limit: Option<i64>,
    pub debug: bool,
}

#[derive(Clone, Debug)]
pub struct ChosenSync {
    pub name: String,
    pub normalized_name: String,
    pub base_name: String,
    pub family: String,
    pub sync_type: SyncType,
    pub bidder: &'static BidderInfo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rejection {
    Selected,
    UnknownBidder,
    Unconfigured,
    SharedFamily,
    Filtered,
    AlreadySynced,
    ActivityDenied,
    PrivacyDenied,
    Disabled,
    Skipped,
}

fn permission_rejection(
    decision: SyncDecision,
    runtime_enabled: bool,
) -> Result<Rejection, ChooseError> {
    Ok(match decision {
        SyncDecision::ResourceLimit => return Err(ChooseError::PrivacyResourceLimit),
        SyncDecision::BlockedByActivity => Rejection::ActivityDenied,
        SyncDecision::BlockedByPrivacy | SyncDecision::MalformedTcf => Rejection::PrivacyDenied,
        SyncDecision::BlockedByDisabledUsersync => Rejection::Disabled,
        SyncDecision::BlockedByRegulationScope | SyncDecision::Allowed if !runtime_enabled => {
            Rejection::Disabled
        }
        SyncDecision::BlockedByRegulationScope => Rejection::Skipped,
        SyncDecision::Allowed => Rejection::Selected,
    })
}

#[derive(Clone, Debug)]
pub struct CandidateDebug {
    pub name: String,
    pub reason: Rejection,
}

#[derive(Clone, Debug, Default)]
pub struct ChooseResult {
    pub selected: Vec<ChosenSync>,
    pub debug: Vec<CandidateDebug>,
}

impl ChooseResult {
    fn push_debug(&mut self, name: &str, reason: Rejection) -> Result<(), ChooseError> {
        if u32::try_from(self.debug.len())
            .ok()
            .is_none_or(|count| count >= MAX_SYNC_DEBUG_ROWS)
        {
            return Err(ChooseError::TooManyNames);
        }
        self.debug.push(CandidateDebug {
            name: name.to_owned(),
            reason,
        });
        Ok(())
    }
}

fn validate_input(input: &ChooseInput<'_>) -> Result<(), ChooseError> {
    if u32::try_from(input.requested.len())
        .ok()
        .is_none_or(|count| count > MAX_SYNC_REQUESTED_NAMES)
    {
        return Err(ChooseError::TooManyNames);
    }
    if input.requested.iter().any(|name| !valid_name(name)) {
        return Err(ChooseError::InvalidName);
    }
    if u32::try_from(input.priority_groups.len())
        .ok()
        .is_none_or(|count| count > MAX_SYNC_PRIORITY_GROUPS)
        || input
            .priority_groups
            .iter()
            .try_fold(0_usize, |count, group| count.checked_add(group.len()))
            .and_then(|count| u32::try_from(count).ok())
            .is_none_or(|count| count > MAX_SYNC_PRIORITY_ENTRIES)
    {
        return Err(ChooseError::TooManyGroups);
    }
    Ok(())
}

/// The route is responsible for host permission before this call. Candidate
/// activity/GDPR/baked-syncer/skip policy is delegated to auctioneer-privacy.
///
/// # Errors
/// Rejects excessive requested names/priority groups or invalid requested names.
pub fn choose(
    input: ChooseInput<'_>,
    privacy: &mut SyncPrivacyContext<'_>,
    rng: &dyn Rng,
) -> Result<ChooseResult, ChooseError> {
    validate_input(&input)?;
    let available = input.catalog.available_names();
    let names = candidate_order_filtered(
        input.requested,
        input.cooperative,
        input.priority_groups,
        &available,
        rng,
        // Explicit requests retain the normal diagnostics/family-attempt
        // ladder. Only automatic additions skip known disabled identities.
        |name| input.catalog.sync_enabled(name) != Some(false),
    );
    let mut seen_names = BTreeSet::new();
    let mut seen_unknown_names = BTreeSet::new();
    let mut seen_families = BTreeSet::new();
    let mut result = ChooseResult::default();
    let limit = input
        .limit
        .filter(|limit| *limit > 0)
        .and_then(|limit| usize::try_from(limit).ok());
    for name in names {
        if limit.is_some_and(|maximum| result.selected.len() >= maximum) {
            break;
        }
        let catalog_match = input.catalog.lookup(name);
        let fresh = match &catalog_match {
            Some(candidate) => seen_names.insert(candidate.catalog_name),
            None => seen_unknown_names.insert(name.to_ascii_lowercase()),
        };
        if !fresh {
            continue;
        }
        let is_requested = input
            .requested
            .iter()
            .any(|requested| requested.eq_ignore_ascii_case(name));
        let reason = match catalog_match {
            None => Rejection::UnknownBidder,
            Some(candidate) if !candidate.configured => Rejection::Unconfigured,
            Some(candidate) if !seen_families.insert(candidate.family) => Rejection::SharedFamily,
            Some(candidate) => {
                let syncer = &candidate.bidder.user_sync;
                let supports =
                    |sync_type| syncer.supports.is_empty() || syncer.supports.contains(&sync_type);
                let preferred_type = if syncer.iframe.is_some()
                    && supports(SyncType::Iframe)
                    && input
                        .filters
                        .allows(SyncType::Iframe, &candidate.normalized_name)
                {
                    Some(SyncType::Iframe)
                } else if syncer.redirect.is_some()
                    && supports(SyncType::Redirect)
                    && input
                        .filters
                        .allows(SyncType::Redirect, &candidate.normalized_name)
                {
                    Some(SyncType::Redirect)
                } else {
                    None
                };
                match preferred_type {
                    None => Rejection::Filtered,
                    Some(_) if input.cookie.is_live(candidate.family, input.now) => {
                        Rejection::AlreadySynced
                    }
                    Some(sync_type) => {
                        let lookup = SyncerLookup {
                            requested_name: &candidate.requested_name,
                            normalized_name: &candidate.normalized_name,
                            base_name: candidate.base_name,
                            bidder: candidate.bidder,
                            syncer: &candidate.bidder.user_sync,
                            syncer_key: candidate.family,
                        };
                        let decision = privacy.cookie_sync_candidate_permission(&lookup);
                        let reason = permission_rejection(decision, candidate.enabled)?;
                        if reason == Rejection::Selected {
                            result.selected.push(ChosenSync {
                                name: if is_requested {
                                    candidate.requested_name
                                } else {
                                    candidate.catalog_name.to_owned()
                                },
                                normalized_name: candidate.normalized_name,
                                base_name: candidate.base_name.to_owned(),
                                family: candidate.family.to_owned(),
                                sync_type,
                                bidder: candidate.bidder,
                            });
                        }
                        reason
                    }
                }
            }
        };
        if input.debug && is_requested {
            result.push_debug(name, reason)?;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{
        Catalog, ChooseError, ChooseInput, ChooseResult, FilterError, FilterSettings, Rejection,
        SyncDecision, choose, permission_rejection,
    };
    use crate::bidder_fixture::{self, fake_bidder::FakeBidder};
    use crate::{config::AuctioneerAppConfig, usersync::cookie::UidCookie};
    use auctioneer_bidder::BidderRegistry;
    use auctioneer_config_data::types::{
        AccountConfig, BidderInfo, HostConfig, SkipWhen, SyncType, SyncerConfig, SyncerEndpoint,
    };
    use auctioneer_privacy::{
        GdprSignalIn, Signal, SyncEndpoint, SyncPrivacyInput, gcl::Gcl, gvl::GvlStore,
        resolve_sync_privacy,
    };
    use auctioneer_runtime::{Rng, SeededRng};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use time::OffsetDateTime;

    fn choose_with_skip(
        gdpr: GdprSignalIn<'_>,
        gpp_sid: Option<&str>,
        skip_when: SkipWhen,
    ) -> (Signal, Signal, Rejection) {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut host = HostConfig::default();
        host.gdpr.default_value = "1".to_owned();
        host.gdpr.enabled = false;
        let account = AccountConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], now);
        let bidder = BidderInfo {
            name: "fake_aaa".to_owned(),
            user_sync: SyncerConfig {
                redirect: Some(SyncerEndpoint {
                    url: "https://sync.example.test".to_owned(),
                    user_macro: None,
                }),
                skip_when,
                ..SyncerConfig::default()
            },
            ..BidderInfo::default()
        };
        let catalog = Catalog::for_test_bidder("fake_aaa", bidder);
        let mut privacy = resolve_sync_privacy(SyncPrivacyInput {
            gdpr,
            consent: Some("opaque-nonempty-consent"),
            us_privacy: None,
            gpp: None,
            gpp_sid,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now,
        })
        .unwrap();
        let parsed = privacy.parsed_request_signal;
        let normalized = privacy.normalized_request_signal;
        let requested = vec!["fake_aaa".to_owned()];
        let result = choose(
            ChooseInput {
                catalog: &catalog,
                requested: &requested,
                cooperative: false,
                priority_groups: &[],
                filters: &FilterSettings::default(),
                cookie: &UidCookie::default(),
                now,
                limit: None,
                debug: true,
            },
            &mut privacy,
            &SeededRng::new(1),
        )
        .unwrap();
        assert_eq!(result.debug.len(), 1);
        (parsed, normalized, result.debug[0].reason)
    }

    fn choose_with_syncer(syncer: SyncerConfig) -> ChooseResult {
        choose_named_syncer(syncer, &["fake_aaa".to_owned()], &[], true)
    }

    fn choose_named_syncer(
        syncer: SyncerConfig,
        requested: &[String],
        priority_groups: &[Vec<String>],
        debug: bool,
    ) -> ChooseResult {
        let bidder = BidderInfo {
            name: "fake_aaa".to_owned(),
            user_sync: syncer,
            ..BidderInfo::default()
        };
        let catalog = Catalog::for_test_bidder("fake_aaa", bidder);
        choose_in_catalog(&catalog, requested, priority_groups, Some(1), debug).unwrap()
    }

    fn choose_in_catalog(
        catalog: &Catalog<'_>,
        requested: &[String],
        priority_groups: &[Vec<String>],
        limit: Option<i64>,
        debug: bool,
    ) -> Result<ChooseResult, ChooseError> {
        choose_with_privacy(
            ChooseInput {
                catalog,
                requested,
                cooperative: !priority_groups.is_empty(),
                priority_groups,
                filters: &FilterSettings::default(),
                cookie: &UidCookie::default(),
                now: OffsetDateTime::UNIX_EPOCH,
                limit,
                debug,
            },
            &SeededRng::new(1),
        )
    }

    fn choose_with_privacy(
        input: ChooseInput<'_>,
        rng: &dyn Rng,
    ) -> Result<ChooseResult, ChooseError> {
        let now = input.now;
        let host = HostConfig::default();
        let account = AccountConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], now);
        let mut privacy = resolve_sync_privacy(SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(0),
            consent: None,
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now,
        })
        .map_err(|_error| ChooseError::PrivacyResourceLimit)?;
        choose(input, &mut privacy, rng)
    }

    #[derive(Default)]
    struct CountingIdentityRng(AtomicUsize);

    impl Rng for CountingIdentityRng {
        fn next_u64(&self) -> u64 {
            self.0.fetch_add(1, Ordering::Relaxed);
            u64::MAX
        }
    }

    fn shared_family_catalog<'a>(host: &'a HostConfig, app: &AuctioneerAppConfig) -> Catalog<'a> {
        let mut registry = BidderRegistry::new();
        for (name, family) in [
            ("fake_disabled", "shared"),
            ("fake_enabled", "shared"),
            ("fake_independent", "fake_independent"),
        ] {
            let info = Box::leak(Box::new(BidderInfo {
                name: name.to_owned(),
                user_sync: SyncerConfig {
                    key: Some(family.to_owned()),
                    redirect: Some(SyncerEndpoint {
                        url: "https://sync.example.test".to_owned(),
                        user_macro: None,
                    }),
                    ..SyncerConfig::default()
                },
                ..BidderInfo::default()
            }));
            registry
                .register(info, Box::<FakeBidder>::default())
                .expect("unique test bidder");
        }
        Catalog::from_registry(host, app, &registry).unwrap()
    }

    #[test]
    fn disabled_automatic_candidates_do_not_reserve_a_shared_sync_family() {
        for flag in ["enabled", "usersync_enabled"] {
            let host = HostConfig::default();
            let app: AuctioneerAppConfig =
                toml::from_str(&format!("[bidders.fake_disabled]\n{flag} = false\n")).unwrap();
            let catalog = shared_family_catalog(&host, &app);
            for priority in [
                vec![],
                vec![vec!["fake_DISABLED".into(), "fake_enabled".into()]],
                vec![vec!["fake_enabled".into(), "fake_DISABLED".into()]],
            ] {
                for requested in [vec![], vec!["fake_unknown".to_owned()]] {
                    for debug in [false, true] {
                        let result = choose_with_privacy(
                            ChooseInput {
                                catalog: &catalog,
                                requested: &requested,
                                cooperative: !requested.is_empty() || !priority.is_empty(),
                                priority_groups: &priority,
                                filters: &FilterSettings::default(),
                                cookie: &UidCookie::default(),
                                now: OffsetDateTime::UNIX_EPOCH,
                                limit: None,
                                debug,
                            },
                            &CountingIdentityRng::default(),
                        )
                        .unwrap();
                        assert_eq!(
                            result
                                .selected
                                .iter()
                                .map(|sync| sync.name.as_str())
                                .collect::<Vec<_>>(),
                            ["fake_enabled", "fake_independent"],
                            "{flag}: priority={priority:?}, requested={requested:?}, debug={debug}"
                        );
                        assert_eq!(
                            result.debug.len(),
                            usize::from(debug && !requested.is_empty())
                        );
                        if let Some(row) = result.debug.first() {
                            assert_eq!(row.name, "fake_unknown");
                            assert_eq!(row.reason, Rejection::UnknownBidder);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn disabled_automatic_candidates_are_removed_before_each_group_shuffle() {
        let host = HostConfig::default();
        let app: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_disabled]\nusersync_enabled = false\n").unwrap();
        let catalog = shared_family_catalog(&host, &app);
        for priority in [
            vec![],
            vec![vec![
                "fake_disabled".into(),
                "fake_enabled".into(),
                "fake_independent".into(),
            ]],
            vec![vec![
                "fake_independent".into(),
                "fake_enabled".into(),
                "fake_DISABLED".into(),
            ]],
        ] {
            let rng = CountingIdentityRng::default();
            let result = choose_with_privacy(
                ChooseInput {
                    catalog: &catalog,
                    requested: &[],
                    cooperative: true,
                    priority_groups: &priority,
                    filters: &FilterSettings::default(),
                    cookie: &UidCookie::default(),
                    now: OffsetDateTime::UNIX_EPOCH,
                    limit: None,
                    debug: false,
                },
                &rng,
            )
            .unwrap();
            assert_eq!(
                rng.0.load(Ordering::Relaxed),
                if priority.is_empty() { 1 } else { 2 }
            );
            assert_eq!(result.selected.len(), 2);
        }
    }

    #[test]
    fn explicitly_requested_disabled_candidate_still_reserves_its_sync_family() {
        let host = HostConfig::default();
        let app: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_disabled]\nusersync_enabled = false\n").unwrap();
        let catalog = shared_family_catalog(&host, &app);
        for debug in [false, true] {
            let result = choose_with_privacy(
                ChooseInput {
                    catalog: &catalog,
                    requested: &["fake_DISABLED".into(), "fake_enabled".into()],
                    cooperative: true,
                    priority_groups: &[vec!["fake_enabled".into()]],
                    filters: &FilterSettings::default(),
                    cookie: &UidCookie::default(),
                    now: OffsetDateTime::UNIX_EPOCH,
                    limit: None,
                    debug,
                },
                &CountingIdentityRng::default(),
            )
            .unwrap();
            assert_eq!(result.selected.len(), 1);
            assert_eq!(result.selected[0].name, "fake_independent");
            assert_eq!(result.debug.len(), if debug { 2 } else { 0 });
            if debug {
                assert_eq!(result.debug[0].name, "fake_DISABLED");
                assert_eq!(result.debug[0].reason, Rejection::Disabled);
                assert_eq!(result.debug[1].name, "fake_enabled");
                assert_eq!(result.debug[1].reason, Rejection::SharedFamily);
            }
        }
    }

    #[test]
    fn enabled_explicit_candidate_keeps_precedence_over_a_disabled_family_sibling() {
        let host = HostConfig::default();
        let app: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_disabled]\nusersync_enabled = false\n").unwrap();
        let catalog = shared_family_catalog(&host, &app);
        let result = choose_with_privacy(
            ChooseInput {
                catalog: &catalog,
                requested: &["fake_enabled".into(), "fake_DISABLED".into()],
                cooperative: true,
                priority_groups: &[vec!["fake_disabled".into()]],
                filters: &FilterSettings::default(),
                cookie: &UidCookie::default(),
                now: OffsetDateTime::UNIX_EPOCH,
                limit: None,
                debug: true,
            },
            &CountingIdentityRng::default(),
        )
        .unwrap();
        assert_eq!(
            result
                .selected
                .iter()
                .map(|sync| sync.name.as_str())
                .collect::<Vec<_>>(),
            ["fake_enabled", "fake_independent"]
        );
        assert_eq!(result.debug.len(), 2);
        assert_eq!(result.debug[0].reason, Rejection::Selected);
        assert_eq!(result.debug[1].reason, Rejection::SharedFamily);
    }

    #[test]
    fn automatic_selection_honors_each_baked_base_veto_for_aliases_before_shuffle() {
        let host = HostConfig {
            aliases: [
                ("fake_alias_one".into(), "fake_base".into()),
                ("fake_alias_two".into(), "fake_base".into()),
            ]
            .into(),
            ..HostConfig::default()
        };
        for field in ["disabled", "sync_disabled"] {
            for denied in [false, true] {
                let info = Box::leak(Box::new(BidderInfo {
                    name: "fake_base".into(),
                    disabled: field == "disabled" && denied,
                    user_sync: SyncerConfig {
                        enabled: field != "sync_disabled" || !denied,
                        redirect: Some(SyncerEndpoint {
                            url: "https://sync.example.test".into(),
                            user_macro: None,
                        }),
                        ..SyncerConfig::default()
                    },
                    ..BidderInfo::default()
                }));
                let mut registry = BidderRegistry::new();
                registry
                    .register(info, Box::<FakeBidder>::default())
                    .expect("unique test bidder");
                let app: AuctioneerAppConfig = toml::from_str(
                    "[bidders.fake_alias_one]\nenabled = true\nusersync_enabled = true\n",
                )
                .unwrap();
                let catalog = Catalog::from_registry(&host, &app, &registry).unwrap();
                let rng = CountingIdentityRng::default();
                let result = choose_with_privacy(
                    ChooseInput {
                        catalog: &catalog,
                        requested: &[],
                        cooperative: true,
                        priority_groups: &[vec![
                            "fake_base".into(),
                            "fake_ALIAS_ONE".into(),
                            "fake_alias_two".into(),
                        ]],
                        filters: &FilterSettings::default(),
                        cookie: &UidCookie::default(),
                        now: OffsetDateTime::UNIX_EPOCH,
                        limit: None,
                        debug: true,
                    },
                    &rng,
                )
                .unwrap();
                assert_eq!(
                    rng.0.load(Ordering::Relaxed),
                    if denied { 0 } else { 4 },
                    "{field}={denied}"
                );
                assert_eq!(
                    result.selected.len(),
                    usize::from(!denied),
                    "{field}={denied}"
                );
                assert!(result.debug.is_empty());
                assert_eq!(catalog.available_names().len(), 3);
            }
        }
    }

    #[test]
    fn full_512_catalog_is_accepted_with_output_limit_one() {
        let registry = bidder_fixture::registry();
        let bidders = registry.infos().collect::<Vec<_>>();
        assert!(
            bidders.len() <= 512,
            "synthetic catalog exceeds the 512-identity owner"
        );
        let base = &bidders
            .iter()
            .find(|bidder| bidder.user_sync.iframe.is_some() || bidder.user_sync.redirect.is_some())
            .unwrap()
            .name;
        let mut host = HostConfig::default();
        for index in bidders.len()..512 {
            host.aliases
                .insert(format!("fake_catalog_boundary_{index:03}"), base.clone());
        }
        let catalog =
            Catalog::from_registry(&host, &AuctioneerAppConfig::default(), registry).unwrap();
        let names = bidders
            .iter()
            .map(|bidder| &bidder.name)
            .chain(host.aliases.keys());
        assert_eq!(names.clone().count(), 512);
        for name in names {
            assert!(catalog.lookup(name).is_some(), "missing identity {name}");
        }
        let result = choose_in_catalog(&catalog, &[], &[], Some(1), true).unwrap();
        assert_eq!(result.selected.len(), 1);
        assert!(result.debug.is_empty());
    }

    #[test]
    fn requested_names_accept_255_and_256_but_refuse_257_before_dedup() {
        let catalog = Catalog::for_test_bidder("fake_aaa", BidderInfo::default());
        for count in [255, 256, 257] {
            let requested = vec!["fake_unknown".to_owned(); count];
            let result = choose_in_catalog(&catalog, &requested, &[], None, true);
            if count <= 256 {
                let chosen = result.unwrap();
                assert!(chosen.selected.is_empty());
                assert_eq!(chosen.debug.len(), 1);
                assert_eq!(chosen.debug[0].reason, Rejection::UnknownBidder);
            } else {
                assert!(matches!(result, Err(ChooseError::TooManyNames)));
            }
        }
    }

    #[test]
    fn priority_entries_accept_255_and_256_but_refuse_257_across_groups() {
        let catalog = Catalog::for_test_bidder("fake_aaa", BidderInfo::default());
        for count in [255_usize, 256, 257] {
            let first = count.checked_div(2).unwrap();
            let groups = vec![
                vec!["fake_unknown".to_owned(); first],
                vec!["fake_unknown".to_owned(); count.checked_sub(first).unwrap()],
            ];
            let result = choose_in_catalog(&catalog, &[], &groups, None, true);
            if count <= 256 {
                assert!(result.unwrap().debug.is_empty());
            } else {
                assert!(matches!(result, Err(ChooseError::TooManyGroups)));
            }
        }
    }

    #[test]
    fn priority_groups_accept_32_but_refuse_33_independently_of_entries() {
        let catalog = Catalog::for_test_bidder("fake_aaa", BidderInfo::default());
        for count in [31, 32, 33] {
            let groups = vec![vec![]; count];
            let result = choose_in_catalog(&catalog, &[], &groups, None, false);
            if count <= 32 {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(ChooseError::TooManyGroups)));
            }
        }
    }

    #[test]
    fn debug_insertion_accepts_256_rows_but_refuses_257_without_requested_name_validation() {
        let mut result = ChooseResult::default();
        for index in 0_usize..256 {
            result
                .push_debug(
                    &format!("fake_unknown_{index:03}"),
                    Rejection::UnknownBidder,
                )
                .unwrap();
        }
        assert_eq!(result.debug.len(), 256);
        assert_eq!(
            result.push_debug("fake_overflow", Rejection::UnknownBidder),
            Err(ChooseError::TooManyNames)
        );
        assert_eq!(result.debug.len(), 256);
        assert_eq!(result.debug[255].name, "fake_unknown_255");
    }

    #[test]
    fn each_filter_list_accepts_255_and_256_but_refuses_257_independently() {
        for field in ["iframe", "image"] {
            for count in [255, 256, 257] {
                let mut value = serde_json::json!({
                    "iframe": {"filter": "exclude", "bidders": vec!["fake_unknown"; 256]},
                    "image": {"filter": "exclude", "bidders": vec!["fake_unknown"; 256]},
                });
                value[field]["bidders"] = serde_json::json!(vec!["fake_unknown"; count]);
                let wire = serde_json::from_value(value.clone()).unwrap();
                for result in [
                    FilterSettings::parse(Some(&value)),
                    FilterSettings::from_wire(&wire),
                ] {
                    if count <= 256 {
                        assert!(result.is_ok(), "{field}: {count} names");
                    } else {
                        assert!(
                            matches!(result, Err(FilterError::TooManyNames)),
                            "{field}: {count} names"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cooperative_additions_use_catalog_spelling_independently_of_debug() {
        for debug in [false, true] {
            for requested in [vec![], vec!["fake_AAA".to_owned()]] {
                let result = choose_named_syncer(
                    SyncerConfig {
                        redirect: Some(SyncerEndpoint {
                            url: "https://sync.example.test".to_owned(),
                            user_macro: None,
                        }),
                        ..SyncerConfig::default()
                    },
                    &requested,
                    &[vec!["fake_AAA".to_owned()]],
                    debug,
                );
                assert_eq!(result.selected.len(), 1);
                assert_eq!(
                    result.selected[0].name,
                    if requested.is_empty() {
                        "fake_aaa"
                    } else {
                        "fake_AAA"
                    },
                    "requested={requested:?}, debug={debug}"
                );
                assert_eq!(
                    result.debug.len(),
                    usize::from(debug && !requested.is_empty())
                );
            }
        }
    }

    #[test]
    fn redirect_only_supports_overrides_iframe_endpoint_preference() {
        let endpoint = SyncerEndpoint {
            url: "https://sync.example.test".to_owned(),
            user_macro: None,
        };
        let result = choose_with_syncer(SyncerConfig {
            iframe: Some(endpoint.clone()),
            redirect: Some(endpoint),
            supports: vec![SyncType::Redirect],
            ..SyncerConfig::default()
        });
        assert_eq!(result.selected.len(), 1);
        assert_eq!(result.selected[0].sync_type, SyncType::Redirect);
        assert_eq!(result.debug[0].reason, Rejection::Selected);
    }

    #[test]
    fn unsupported_endpoint_is_filtered_when_no_supported_type_is_configured() {
        let result = choose_with_syncer(SyncerConfig {
            iframe: Some(SyncerEndpoint {
                url: "https://sync.example.test".to_owned(),
                user_macro: None,
            }),
            supports: vec![SyncType::Redirect],
            ..SyncerConfig::default()
        });
        assert!(result.selected.is_empty());
        assert_eq!(result.debug[0].reason, Rejection::Filtered);
    }

    #[test]
    fn runtime_disablement_precedes_scope_skip_without_hiding_earlier_denials() {
        assert_eq!(
            permission_rejection(SyncDecision::BlockedByRegulationScope, false).unwrap(),
            Rejection::Disabled
        );
        assert_eq!(
            permission_rejection(SyncDecision::BlockedByRegulationScope, true).unwrap(),
            Rejection::Skipped
        );
        assert_eq!(
            permission_rejection(SyncDecision::BlockedByActivity, false).unwrap(),
            Rejection::ActivityDenied
        );
        assert_eq!(
            permission_rejection(SyncDecision::BlockedByPrivacy, false).unwrap(),
            Rejection::PrivacyDenied
        );
        assert_eq!(
            permission_rejection(SyncDecision::Allowed, false).unwrap(),
            Rejection::Disabled
        );
        assert_eq!(
            permission_rejection(SyncDecision::ResourceLimit, true),
            Err(ChooseError::PrivacyResourceLimit)
        );
    }

    #[test]
    fn gdpr_skip_uses_parsed_wire_yes_not_host_defaulted_ambiguous() {
        let skip = SkipWhen {
            gdpr: true,
            gpp_sid: vec![],
        };
        assert_eq!(
            choose_with_skip(GdprSignalIn::CookieNumber(1), None, skip.clone()),
            (Signal::Yes, Signal::Yes, Rejection::Skipped)
        );
        assert_eq!(
            choose_with_skip(GdprSignalIn::CookieAbsent, None, skip),
            (Signal::Ambiguous, Signal::Yes, Rejection::Selected)
        );
    }

    #[test]
    fn sid_skip_uses_exact_decoded_wire_spelling_in_chooser() {
        for (sid, expected) in [
            ("2,6", Rejection::Skipped),
            ("6,2", Rejection::Selected),
            ("02,6", Rejection::Selected),
            ("-1", Rejection::Skipped),
        ] {
            let skip = SkipWhen {
                gdpr: false,
                gpp_sid: vec!["2,6".to_owned(), "-1".to_owned()],
            };
            let (_, _, reason) = choose_with_skip(GdprSignalIn::CookieNumber(0), Some(sid), skip);
            assert_eq!(reason, expected, "{sid}");
        }
    }
}

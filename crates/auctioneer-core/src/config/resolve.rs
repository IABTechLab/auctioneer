//! Overlay-over-baked resolution.
//!
//! Each resolver takes the pushed `AuctioneerAppConfig` (from the config
//! store) and the build-time TOML default, then returns an owned
//! `HostConfig` / `AccountConfig` / status body with the overlay
//! applied. `None` fields fall through; `Some(value)` overrides.
//!
//! Handlers call these once per request and pass the resolved config
//! into `run_auction` — the exchange never learns about
//! `AuctioneerAppConfig` directly, preserving the foundations dependency-purity
//! contract.

use crate::config::policy_account::AccountGdprFields;
use crate::config::{
    AccountOverride, AccountPrivacyOverride, AuctionConfig, AuctioneerAppConfig,
    ChannelEnabledOverride, HostGdprOverride,
};
use crate::host_primitives::gates::boot_limits;
use auctioneer_bidder::BidderName;
use auctioneer_config_data::activity::merge_activities;
use auctioneer_config_data::types::{
    AccountConfig, AccountGdprConfig, BidderInfo, ChannelEnabled, CooperativeConfig, HostConfig,
    PurposeEnforcement, ResourceLimits, SpecialFeatureEnforcement, UserSyncConfig,
};
use auctioneer_exchange::{AccountSource, AuctionError, ExchangeFeatures, UNRESOLVED_ACCOUNT_ID};
use auctioneer_runtime::aux_work::MAX_CATALOG_IDENTITIES;
use auctioneer_runtime::{BoundedJsonError, ResolvedIsolateLimits, count_json_nodes};
use serde::{
    Deserialize as _, Deserializer,
    de::{DeserializeSeed, Error as DeError, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt};

/// The ordered publisher-id lookup paths, most specific first. The first
/// non-empty string value wins.
const ACCOUNT_ID_PATHS: [&str; 6] = [
    "/app/publisher/ext/prebid/parentAccount",
    "/app/publisher/id",
    "/site/publisher/ext/prebid/parentAccount",
    "/site/publisher/id",
    "/dooh/publisher/ext/prebid/parentAccount",
    "/dooh/publisher/id",
];

/// Owned pair the handler holds on the stack for the lifetime of the
/// request. The exchange-facing `ResolvedAccount<'a>` (which borrows via
/// `config: &'a AccountConfig`) is constructed by the handler from a
/// reference to `owned.config` after [`resolve_account`] returns.
#[derive(Clone, Debug)]
pub struct ResolvedAccountOwned {
    pub id: String,
    pub selection: AccountSelection,
    pub source: AccountSource,
    pub config: AccountConfig,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccountSelection {
    Default,
    Named(String),
}

impl AccountSelection {
    #[must_use]
    pub fn from_requested(id: Option<&str>) -> Self {
        match id.filter(|requested| !requested.is_empty()) {
            Some(requested) => Self::Named(requested.to_owned()),
            None => Self::Default,
        }
    }
}

impl ResolvedAccountOwned {
    #[must_use]
    pub fn effective_priority_groups<'a>(&'a self, host: &'a HostConfig) -> &'a [Vec<String>] {
        self.config
            .cookie_sync
            .priority_groups
            .as_deref()
            .unwrap_or_else(|| {
                host.user_sync
                    .as_ref()
                    .map_or(&[], |sync| sync.priority_groups.as_slice())
            })
    }
}

/// Resolve one request-population cap against what the boot envelope
/// funds: the effective value is the smaller of the two.
///
/// `configured` is what somebody ASKED for — the baked `auctioneer.toml`
/// row, or a runtime override pushed over it. `funded` is what the
/// installed envelope BUDGETED memory for. They come from different
/// places and the shipped rows disagree: the baked default permits 100
/// outbound calls while the wasm envelopes fund 32. Taking the
/// configured value directly would authorise allocation and dispatch
/// the envelope never reserved memory for, and a pushed override could
/// widen that gap without a redeploy.
///
/// A configured value BELOW the ceiling is an operator's own decision
/// and passes through untouched — the ceiling is a bound, not a target.
///
/// With no envelope installed the configured value stands. That is not
/// a hole: the gate accessors answer `None` only before boot completes,
/// and a request arriving then is refused at admission rather than
/// reaching an auction. Clamping to zero here would instead turn an
/// in-process test, which installs no envelope, into a deployment that
/// calls no bidders.
///
/// Generic over the cap's width because the knobs it resolves are not
/// all one type: the two populations are `usize`, the response and
/// outbound byte caps are `u64` and the node budget is `u32`. Each caller
/// widens the envelope's own figure into the config field's type before calling,
/// so the comparison here is between two values of one type and no
/// conversion can lose a bit inside the rule itself.
fn funded_cap<T: Ord>(configured: T, funded: Option<T>) -> T {
    match funded {
        Some(ceiling) => configured.min(ceiling),
        None => configured,
    }
}

/// Widen an envelope population count into the `usize` the two
/// population knobs are typed as.
///
/// Saturating rather than fallible: a ceiling wider than this
/// platform's `usize` cannot be exceeded on it, so `usize::MAX` is the
/// tightest honest bound and not a silent truncation. Only a 16-bit
/// target could reach that arm.
fn widen_u32(funded: u32) -> usize {
    usize::try_from(funded).unwrap_or(usize::MAX)
}

/// Build the effective `HostConfig` = baked TOML default with `auctioneer.toml`
/// overrides applied, with the request-population caps resolved against
/// the boot envelope.
///
/// Auction resource limits, host aliases, GDPR/CCPA/LMT flags, and
/// the status body all funnel through here.
///
/// The seven caps the boot envelope also declares are resolved against
/// it LAST, after the overlay, so one rule covers the baked default and
/// every override: see [`funded_cap`]. Those are the two that decide
/// how many outbound calls an auction issues and how many bidder bodies
/// it retains, the two outbound request caps (raw and encoded), the two
/// per-response byte caps, and the inbound node budget. The remaining
/// knobs are not envelope rows:
/// `default_tmax_ms` and `max_tmax_ms` bound a deadline, and
/// `max_bidder_response_bytes` is a per-call cap the wave scheduler
/// narrows the resolved decoded cap BY rather than one the envelope
/// funds separately — so they take the overlay value as given.
#[must_use]
pub fn resolved_host(app: &AuctioneerAppConfig, baked: &HostConfig) -> HostConfig {
    let mut host = baked.clone();
    apply_resource_overrides(&mut host.resource_limits, &app.auction);
    if let Some(v) = app.status.banner.as_ref() {
        host.status_response.clone_from(v);
    }
    if let Some(value) = app.ccpa.as_ref().and_then(|ccpa| ccpa.enforce) {
        host.ccpa.enforce = value;
    }
    if let Some(value) = app.lmt.as_ref().and_then(|lmt| lmt.enforce) {
        host.lmt.enforce = value;
    }
    // Each knob resolves against its OWN funded ceiling. One shared
    // minimum would collapse a row whose bidder and call ceilings
    // differ onto whichever is smaller.
    let funded = boot_limits();
    host.resource_limits.max_bidders_per_auction = funded_cap(
        host.resource_limits.max_bidders_per_auction,
        funded.map(|row| widen_u32(row.max_bidders_per_auction)),
    );
    host.resource_limits.max_outbound_requests = funded_cap(
        host.resource_limits.max_outbound_requests,
        funded.map(|row| widen_u32(row.max_outbound_requests)),
    );
    host.resource_limits.max_outbound_raw_bytes = funded_cap(
        host.resource_limits.max_outbound_raw_bytes,
        funded.map(|row| row.max_outbound_raw_bytes),
    );
    host.resource_limits.max_outbound_request_bytes = funded_cap(
        host.resource_limits.max_outbound_request_bytes,
        funded.map(|row| row.max_outbound_request_bytes),
    );
    host.resource_limits.max_transport_bytes = funded_cap(
        host.resource_limits.max_transport_bytes,
        funded.map(|row| row.max_transport_bytes),
    );
    host.resource_limits.max_decoded_bytes = funded_cap(
        host.resource_limits.max_decoded_bytes,
        funded.map(|row| row.max_decoded_bytes),
    );
    host.resource_limits.max_parsed_nodes = funded_cap(
        host.resource_limits.max_parsed_nodes,
        funded.map(|row| row.max_parsed_nodes),
    );
    host
}

/// Narrow an envelope row by the resolved config's own figures for the
/// five caps config can now lower.
///
/// The envelope row is what the request path has always read its caps
/// from, and it stays the authority: each cap is clamped to the supplied
/// row even if the caller has not applied [`resolved_host`]. This is a
/// narrowing, never a widening. Applied rather than replaced, because
/// every OTHER field on the row (the gate capacities, the derived
/// response ceiling, the parse factors) has no config surface and must
/// survive untouched.
///
/// The two outbound caps are narrowed here as well, which is what makes
/// a lowered `max_outbound_request_bytes` reach enforcement:
/// `drive_fanout` takes its encoded ceiling from this row, and
/// `outbound::build` hands the same field to the host's
/// `max_request_body_bytes`. Both read the narrowed row rather than the
/// installed one.
///
/// Returns an owned copy: `ResolvedIsolateLimits` is `Copy`, the
/// installed row lives behind a process-global `OnceLock` that nothing
/// may mutate, and the narrowed row is per-request state.
#[must_use]
pub fn limits_for_request(
    envelope: &ResolvedIsolateLimits,
    host: &HostConfig,
) -> ResolvedIsolateLimits {
    ResolvedIsolateLimits {
        max_outbound_raw_bytes: host
            .resource_limits
            .max_outbound_raw_bytes
            .min(envelope.max_outbound_raw_bytes),
        max_outbound_request_bytes: host
            .resource_limits
            .max_outbound_request_bytes
            .min(envelope.max_outbound_request_bytes),
        max_transport_bytes: host
            .resource_limits
            .max_transport_bytes
            .min(envelope.max_transport_bytes),
        max_decoded_bytes: host
            .resource_limits
            .max_decoded_bytes
            .min(envelope.max_decoded_bytes),
        max_parsed_nodes: host
            .resource_limits
            .max_parsed_nodes
            .min(envelope.max_parsed_nodes),
        ..*envelope
    }
}

/// Build the fully-resolved `HostConfig`: the existing deployment-wide
/// overlay, then cookie, sync, GDPR, and alias overlays layered on top.
///
/// It EXTENDS [`resolved_host`], never replaces it — the base call folds
/// in the `[auction]` and `[status]` overlays, and every baked field the
/// added sections do not name (resource limits, status body, and any GDPR
/// sub-entry absent from the override) passes through untouched. Nested enforcement maps merge
/// PER ENTRY, so a keyed override touches only that id and its sibling
/// baked entries survive.
///
/// `channel_enabled` is account-only, not a field on `HostConfig`, so it
/// is not a concern here.
#[must_use]
pub fn resolve_host(app: &AuctioneerAppConfig, baked: &HostConfig) -> HostConfig {
    // Base = the existing deployment-wide overlay. Never bypassed.
    let mut host = resolved_host(app, baked);

    if let Some(cookie) = &app.host_cookie {
        if let Some(value) = &cookie.cookie_name {
            host.host_cookie.cookie_name.clone_from(value);
        }
        if let Some(value) = &cookie.family {
            host.host_cookie.family.clone_from(value);
        }
        if let Some(value) = &cookie.domain {
            host.host_cookie.domain = Some(value.clone());
        }
        if let Some(value) = cookie.ttl_days {
            host.host_cookie.ttl_days = value;
        }
        if let Some(value) = cookie.max_cookie_size_bytes {
            host.host_cookie.max_cookie_size_bytes = value;
        }
        if let Some(optout) = &cookie.optout_cookie {
            let target = host
                .host_cookie
                .optout_cookie
                .get_or_insert_with(Default::default);
            if let Some(value) = &optout.name {
                target.name.clone_from(value);
            }
            if let Some(value) = &optout.value {
                target.value.clone_from(value);
            }
        }
    }
    if let Some(sync) = &app.user_sync {
        if host.user_sync.is_none() {
            host.user_sync = sync
                .external_url
                .as_ref()
                .map(|external_url| UserSyncConfig {
                    external_url: external_url.clone(),
                    redirect_url: None,
                    cooperative: CooperativeConfig::default(),
                    priority_groups: Vec::new(),
                    allowed_origins: Vec::new(),
                });
        }
        if let Some(target) = &mut host.user_sync {
            if let Some(value) = &sync.external_url {
                target.external_url.clone_from(value);
            }
            if let Some(value) = &sync.redirect_url {
                target.redirect_url = Some(value.clone());
            }
            if let Some(value) = sync.cooperative.as_ref().and_then(|v| v.enabled_by_default) {
                target.cooperative.enabled_by_default = value;
            }
            if let Some(value) = &sync.priority_groups {
                target.priority_groups.clone_from(value);
            }
            if let Some(value) = &sync.allowed_origins {
                target.allowed_origins.clone_from(value);
            }
        }
    }

    // Host `[gdpr]` overlay. Absent → the baked gdpr passes through whole.
    if let Some(gdpr) = app.gdpr.as_ref() {
        apply_host_gdpr_overlay(&mut host, gdpr);
    }

    // Host `[aliases]` overlay: per-KEY object merge (RFC 7396) — baked
    // aliases the override does not name survive; a named key replaces
    // its base bidder.
    if let Some(overrides) = app.aliases.as_ref() {
        for (alias, base_bidder) in overrides {
            host.aliases.insert(alias.clone(), base_bidder.clone());
        }
    }

    host
}

pub(super) fn apply_host_gdpr_overlay(host: &mut HostConfig, gdpr: &HostGdprOverride) {
    if let Some(value) = gdpr.enabled {
        host.gdpr.enabled = value;
    }
    if let Some(value) = gdpr.host_vendor_id {
        host.gdpr.host_vendor_id = Some(value);
    }
    if let Some(value) = gdpr.default_value.as_ref() {
        host.gdpr.default_value.clone_from(value);
    }
    // Whole-array replace (RFC 7396 array semantics).
    if let Some(value) = gdpr.non_standard_publishers.as_ref() {
        host.gdpr.non_standard_publishers.clone_from(value);
    }
    if let Some(tcf2) = gdpr.tcf2.as_ref() {
        if let Some(value) = tcf2.enabled {
            host.gdpr.tcf2.enabled = value;
        }
        if let Some(treatment) = tcf2.purpose_one_treatment {
            if let Some(value) = treatment.enabled {
                host.gdpr.tcf2.purpose_one_treatment.enabled = value;
            }
            if let Some(value) = treatment.access_allowed {
                host.gdpr.tcf2.purpose_one_treatment.access_allowed = value;
            }
        }
        if let Some(overrides) = tcf2.purposes.as_ref() {
            for (&id, purpose) in overrides {
                let base = host.gdpr.tcf2.purposes.entry(id).or_default();
                if let Some(value) = purpose.enforce_algo {
                    base.enforce_algo = value;
                }
                if let Some(value) = purpose.enforce_purpose {
                    base.enforce_purpose = value;
                }
                if let Some(value) = purpose.enforce_vendors {
                    base.enforce_vendors = value;
                }
                if let Some(value) = purpose.vendor_exceptions.as_ref() {
                    base.vendor_exceptions.clone_from(value);
                }
            }
        }
        if let Some(overrides) = tcf2.special_features.as_ref() {
            for (&id, feature) in overrides {
                let base = host.gdpr.tcf2.special_features.entry(id).or_default();
                if let Some(value) = feature.enforce {
                    base.enforce = value;
                }
                if let Some(value) = feature.vendor_exceptions.as_ref() {
                    base.vendor_exceptions.clone_from(value);
                }
            }
        }
    }
}

pub(super) fn apply_resource_overrides(base: &mut ResourceLimits, requested: &AuctionConfig) {
    if let Some(value) = requested.default_tmax_ms {
        base.default_tmax_ms = value;
    }
    if let Some(value) = requested.max_tmax_ms {
        base.max_tmax_ms = value;
    }
    if let Some(value) = requested.max_bidders_per_auction {
        base.max_bidders_per_auction = value;
    }
    if let Some(value) = requested.max_outbound_requests {
        base.max_outbound_requests = value;
    }
    if let Some(value) = requested.max_bidder_response_bytes {
        base.max_bidder_response_bytes = value;
    }
    if let Some(value) = requested.max_outbound_raw_bytes {
        base.max_outbound_raw_bytes = value;
    }
    if let Some(value) = requested.max_outbound_request_bytes {
        base.max_outbound_request_bytes = value;
    }
    if let Some(value) = requested.max_transport_bytes {
        base.max_transport_bytes = value;
    }
    if let Some(value) = requested.max_decoded_bytes {
        base.max_decoded_bytes = value;
    }
    if let Some(value) = requested.max_parsed_nodes {
        base.max_parsed_nodes = value;
    }
}

/// Runtime overlays can narrow construction budgets, never widen boot's envelope.
pub(crate) fn clamp_host_to_boot(
    host: &mut HostConfig,
    boot: &auctioneer_runtime::ResolvedIsolateLimits,
) {
    let limits = &mut host.resource_limits;
    limits.max_bidders_per_auction = limits
        .max_bidders_per_auction
        .min(usize::try_from(boot.max_bidders_per_auction).unwrap_or(usize::MAX));
    limits.max_outbound_requests = limits
        .max_outbound_requests
        .min(usize::try_from(boot.max_outbound_requests).unwrap_or(usize::MAX));
    limits.max_bidder_response_bytes = limits
        .max_bidder_response_bytes
        .min(usize::try_from(boot.max_decoded_bytes).unwrap_or(usize::MAX));
    limits.max_outbound_raw_bytes = limits
        .max_outbound_raw_bytes
        .min(boot.max_outbound_raw_bytes);
    limits.max_outbound_request_bytes = limits
        .max_outbound_request_bytes
        .min(boot.max_outbound_request_bytes);
    limits.max_transport_bytes = limits.max_transport_bytes.min(boot.max_transport_bytes);
    limits.max_decoded_bytes = limits.max_decoded_bytes.min(boot.max_decoded_bytes);
    limits.max_parsed_nodes = limits.max_parsed_nodes.min(boot.max_parsed_nodes);
}

/// Extract the account id from an ALREADY-PARSED request body, before
/// `prepare()` has run, by walking the ordered publisher-id path list.
///
/// Callers holding a bounded tree can reuse it. The raw-byte resolver instead
/// discovers identity selectively, preserving unrelated opaque numeric tokens.
fn account_id_from_value(value: &serde_json::Value) -> Option<String> {
    ACCOUNT_ID_PATHS
        .iter()
        .filter_map(|path| value.pointer(path))
        .filter_map(serde_json::Value::as_str)
        .find(|id| !id.is_empty())
        .map(str::to_owned)
}

/// Retain only a publisher-path string; skipped subtrees never convert numbers.
struct AccountPath<'a>(&'a [&'a str]);

/// Match `serde_json`'s default container-depth ceiling even in skipped values.
struct DiscoveryDepth(u8);

impl<'de> DeserializeSeed<'de> for DiscoveryDepth {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        let raw = <&RawValue>::deserialize(deserializer)?;
        let kind = raw.get().as_bytes().first();
        if matches!(kind, Some(b'{' | b'[')) {
            let remaining = self
                .0
                .checked_sub(1)
                .ok_or_else(|| DeError::custom("account discovery depth exceeded"))?;
            let mut parser = serde_json::Deserializer::from_str(raw.get());
            let result = if kind == Some(&b'{') {
                Deserializer::deserialize_map(&mut parser, DiscoveryDepth(remaining))
            } else {
                Deserializer::deserialize_seq(&mut parser, DiscoveryDepth(remaining))
            };
            result.map_err(DeError::custom)?;
        }
        Ok(())
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "this visitor is only dispatched for objects and arrays"
)]
impl<'de> Visitor<'de> for DiscoveryDepth {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a depth-bounded JSON container")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        while map.next_key::<IgnoredAny>()?.is_some() {
            map.next_value_seed(DiscoveryDepth(self.0))?;
        }
        Ok(())
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<(), S::Error> {
        while seq.next_element_seed(DiscoveryDepth(self.0))?.is_some() {}
        Ok(())
    }
}

impl<'de> DeserializeSeed<'de> for AccountPath<'_> {
    type Value = Option<String>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        let raw = <&RawValue>::deserialize(deserializer)?;
        match raw.get().as_bytes().first() {
            Some(b'{') if !self.0.is_empty() => {
                let mut parser = serde_json::Deserializer::from_str(raw.get());
                parser.deserialize_map(self).map_err(DeError::custom)
            }
            Some(b'"') if self.0.is_empty() => serde_json::from_str(raw.get())
                .map(Some)
                .map_err(DeError::custom),
            _ => Ok(None),
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "raw-token dispatch invokes this visitor only for publisher-path objects"
)]
impl<'de> Visitor<'de> for AccountPath<'_> {
    type Value = Option<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a publisher path in valid JSON")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut found = None;
        while let Some(key) = map.next_key::<String>()? {
            if let Some((field, rest)) = self.0.split_first()
                && key == *field
            {
                // Match Value's last-key-wins behavior, including an absent leaf.
                found = map.next_value_seed(AccountPath(rest))?;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(found)
    }
}

/// Resolve the per-request account BEFORE `prepare()` runs.
///
/// Three layers, in order: the baked `AccountConfig` default, the
/// deployment-wide overlay [`resolved_account`] already applies to every
/// request, then the `[accounts.<id>]` entry when the request names a
/// publisher the deployment has onboarded.
///
/// An unknown publisher — no id on the request, or an id with no
/// matching `[accounts.<id>]` entry — is NOT an error. It falls back to
/// [`AccountSource::DefaultFallback`], and `prepare()` then surfaces any
/// structural request error as normal. That mirrors prebid's
/// `PublisherUnknown` fall-through: a publisher nobody has onboarded yet
/// still gets 400s from structural errors, not from account resolution.
///
/// A request that names no publisher at all resolves to
/// [`UNRESOLVED_ACCOUNT_ID`] and deliberately SKIPS the `accounts` map
/// lookup, so an `[accounts.unknown]` entry cannot capture every
/// publisher-less request.
///
/// # Errors
/// Returns [`AuctionError::BadInput`] for invalid JSON, discovery failure or an
/// exceeded syntactic node budget, before account-state checks. Returns [`AuctionError::AccountDisabled`]
/// when the resolved account carries `disabled = true`. Overlay failure
/// is compile-time impossible
/// (both sides are typed structs), so this resolver never emits
/// `MalformedAcct`; that variant stays in the taxonomy for a future
/// remote-account fetch path.
pub fn resolve_account(
    raw_body: &[u8],
    app: &AuctioneerAppConfig,
    default: &AccountConfig,
    max_nodes: u32,
) -> Result<ResolvedAccountOwned, AuctionError> {
    // Bound and validate all bytes before selective discovery, including skipped
    // subtrees. A failed parse is never evidence that identity was absent.
    count_json_nodes(raw_body, max_nodes).map_err(|error| match error {
        BoundedJsonError::NodeBudget(_) => AuctionError::BadInput("too many JSON nodes".to_owned()),
        BoundedJsonError::Syntax(_) | BoundedJsonError::RawValue(_) => {
            AuctionError::BadInput("invalid account discovery JSON".to_owned())
        }
    })?;
    DiscoveryDepth(127)
        .deserialize(&mut serde_json::Deserializer::from_slice(raw_body))
        .map_err(|_error| AuctionError::BadInput("account discovery depth exceeded".to_owned()))?;
    let mut candidate = None;
    for path in ACCOUNT_ID_PATHS {
        let fields: Vec<_> = path.split('/').skip(1).collect();
        let mut parser = serde_json::Deserializer::from_slice(raw_body);
        let discovered = AccountPath(&fields)
            .deserialize(&mut parser)
            .map_err(|_error| {
                AuctionError::BadInput("account identity discovery failed".to_owned())
            })?;
        if let Some(id) = discovered.filter(|value| !value.is_empty()) {
            candidate = Some(id);
            break;
        }
    }
    let resolved = resolve_selected_account(
        AccountSelection::from_requested(candidate.as_deref()),
        app,
        default,
    );
    if resolved.config.disabled {
        return Err(AuctionError::AccountDisabled(resolved.id));
    }
    Ok(resolved)
}

/// [`resolve_account`] over a body the caller has already parsed within
/// its syntactic node budget; this entry point does not parse again.
///
/// `None` means the body was not parseable as JSON, which resolves to
/// the fallback account for the reason given above.
///
/// # Errors
/// Returns [`AuctionError::AccountDisabled`] when the resolved account
/// is disabled. Node-budget checks belong to the caller's bounded parse.
pub fn resolve_account_from_value(
    body: Option<&serde_json::Value>,
    app: &AuctioneerAppConfig,
    default: &AccountConfig,
) -> Result<ResolvedAccountOwned, AuctionError> {
    let candidate_id = body.and_then(account_id_from_value);
    let resolved = resolve_selected_account(
        AccountSelection::from_requested(candidate_id.as_deref()),
        app,
        default,
    );
    if resolved.config.disabled {
        return Err(AuctionError::AccountDisabled(resolved.id));
    }
    Ok(resolved)
}

/// Resolve account policy without interpreting `disabled`; sync reports
/// disabled accounts with its own 400, while auctions retain code 7/503.
#[must_use]
pub fn resolve_selected_account(
    selection: AccountSelection,
    app: &AuctioneerAppConfig,
    default: &AccountConfig,
) -> ResolvedAccountOwned {
    let effective_selection = match selection {
        AccountSelection::Named(id) if id.is_empty() => AccountSelection::Default,
        AccountSelection::Named(id) => AccountSelection::Named(id),
        AccountSelection::Default => AccountSelection::Default,
    };
    let base = resolved_account(app, default);
    let (id, source, config) = match &effective_selection {
        AccountSelection::Default => (
            UNRESOLVED_ACCOUNT_ID.to_owned(),
            AccountSource::DefaultFallback,
            base,
        ),
        AccountSelection::Named(id) => match app.accounts.get(id) {
            Some(overlay) => (
                id.clone(),
                AccountSource::Configured,
                merge_account(base, overlay),
            ),
            None => (id.clone(), AccountSource::DefaultFallback, base),
        },
    };
    ResolvedAccountOwned {
        id,
        selection: effective_selection,
        source,
        config,
    }
}

/// Layer 3: apply one `[accounts.<id>]` entry on top of the
/// deployment-wide result. Present fields write; absent fields leave the
/// deployment-wide value in place.
fn merge_account(mut base: AccountConfig, overlay: &AccountOverride) -> AccountConfig {
    if let Some(value) = overlay.disabled {
        base.disabled = value;
    }
    if let Some(value) = overlay.debug_allow {
        base.debug_allow = value;
    }
    if let Some(sync) = &overlay.cookie_sync {
        if let Some(value) = sync.default_limit {
            base.cookie_sync.default_limit = Some(value);
        }
        if let Some(value) = sync.max_limit {
            base.cookie_sync.max_limit = Some(value);
        }
        if let Some(value) = sync.default_coop_sync {
            base.cookie_sync.default_coop_sync = Some(value);
        }
        if let Some(value) = &sync.priority_groups {
            base.cookie_sync.priority_groups = Some(value.clone());
        }
    }
    if let Some(value) = overlay.bid_rounding.as_ref() {
        base.bid_rounding.clone_from(value);
    }
    if let Some(value) = overlay.targeting_prefix.as_ref() {
        base.targeting_prefix.clone_from(value);
    }
    if let Some(value) = overlay.truncate_target_attribute {
        base.truncate_target_attribute = Some(value);
    }
    if let Some(gdpr) = overlay.gdpr.as_ref() {
        merge_account_gdpr(&mut base.gdpr, gdpr);
    }
    if let Some(ccpa) = overlay.ccpa.as_ref() {
        if let Some(value) = ccpa.enabled {
            base.ccpa.enabled = value;
        }
        if let Some(channels) = ccpa.channel_enabled.as_ref() {
            merge_channels(&mut base.ccpa.channel_enabled, channels);
        }
    }
    if let Some(privacy) = overlay.privacy.as_ref() {
        merge_account_privacy(&mut base, privacy);
    }
    base
}

/// Per-entry recursive merge of the account GDPR block. A keyed purpose
/// or special-feature override touches ONLY that id; sibling baked
/// entries survive. `TriState::Unset` on a resolved entry stays legal
/// and meaningful — it tells the runtime merge the account has no
/// opinion for that field and host should decide. `auctioneer-privacy::sync`
/// owns the effective web gate and Purpose 1 treatment permission merge.
pub(super) fn merge_account_gdpr<'a>(
    base: &mut AccountGdprConfig,
    source: impl Into<AccountGdprFields<'a>>,
) {
    let overlay = source.into();
    if let Some(value) = overlay.enabled {
        base.enabled = value;
    }
    if let Some(treatment) = overlay.purpose_one_treatment {
        if let Some(value) = treatment.enabled {
            base.purpose_one_treatment.enabled = Some(value);
        }
        if let Some(value) = treatment.access_allowed {
            base.purpose_one_treatment.access_allowed = Some(value);
        }
    }
    if let Some(channels) = overlay.channel_enabled {
        merge_channels(&mut base.channel_enabled, channels);
    }
    if let Some(overrides) = overlay.purposes {
        for (&id, purpose) in overrides {
            let entry: &mut PurposeEnforcement = base.purposes.entry(id).or_default();
            if let Some(value) = purpose.enforce_algo {
                entry.enforce_algo = value;
            }
            if let Some(value) = purpose.enforce_purpose {
                entry.enforce_purpose = value;
            }
            if let Some(value) = purpose.enforce_vendors {
                entry.enforce_vendors = value;
            }
            if let Some(value) = purpose.vendor_exceptions.as_ref() {
                entry.vendor_exceptions.clone_from(value);
            }
        }
    }
    if let Some(overrides) = overlay.special_features {
        for (&id, feature) in overrides {
            let entry: &mut SpecialFeatureEnforcement =
                base.special_features.entry(id).or_default();
            if let Some(value) = feature.enforce {
                entry.enforce = value;
            }
            if let Some(value) = feature.vendor_exceptions.as_ref() {
                entry.vendor_exceptions.clone_from(value);
            }
        }
    }
    // Whole-array replace (RFC 7396 array semantics); account-only, so
    // there is no host-side merge for this field.
    if let Some(value) = overlay.basic_enforcement_vendors {
        base.basic_enforcement_vendors.clone_from(value);
    }
}

pub(super) fn merge_channels(base: &mut ChannelEnabled, overlay: &ChannelEnabledOverride) {
    if let Some(value) = overlay.web {
        base.web = value;
    }
    if let Some(value) = overlay.app {
        base.app = value;
    }
    if let Some(value) = overlay.video {
        base.video = value;
    }
    if let Some(value) = overlay.amp {
        base.amp = value;
    }
    if let Some(value) = overlay.dooh {
        base.dooh = value;
    }
}

/// IP-anon is account-scoped with no runtime host fall-through, so the
/// resolved value here is what `ip_keep_bits()` reads directly. A family
/// the overlay leaves absent keeps its baked default (24 / 56).
pub(super) fn merge_account_privacy(base: &mut AccountConfig, overlay: &AccountPrivacyOverride) {
    if let Some(bits) = overlay.ipv4.as_ref().and_then(|v4| v4.anon_keep_bits) {
        base.privacy.ipv4.anon_keep_bits = bits;
    }
    if let Some(bits) = overlay.ipv6.as_ref().and_then(|v6| v6.anon_keep_bits) {
        base.privacy.ipv6.anon_keep_bits = bits;
    }
    if let Some(activities) = overlay.allowactivities.as_ref() {
        merge_activities(&mut base.privacy.allowactivities, activities);
    }
}

/// Build the effective `AccountConfig` = baked TOML default with
/// `auctioneer.toml` overrides applied.
#[must_use]
pub fn resolved_account(app: &AuctioneerAppConfig, baked: &AccountConfig) -> AccountConfig {
    let mut account = baked.clone();
    if let Some(v) = app.targeting.prefix.as_ref() {
        account.targeting_prefix.clone_from(v);
    }
    if let Some(v) = app.targeting.truncate_target_attribute {
        account.truncate_target_attribute = Some(v);
    }
    account
}

/// Return `true` iff the bidder is enabled after the overlay.
///
/// `[bidders.<name>] enabled = false` in `auctioneer.toml` is load-bearing:
/// the auction handler folds every disabled name into the
/// request-scoped disabled set on `AuctionPlan`, and the splitter
/// drops the bidder before fan-out, emitting a code-6
/// `BidderTemporarilyDisabled` warning under the bidder's observable
/// name (`ext.warnings.<name>`) instead of removing it silently. This
/// helper reports the overlay's own opinion only — it does not
/// consult the baked `BidderInfo.disabled` bit, which is enforced
/// independently and unconditionally at split time regardless of what
/// the overlay says.
#[must_use]
pub fn bidder_enabled(app: &AuctioneerAppConfig, bidder: &BidderName) -> bool {
    app.bidders
        .get(bidder.as_str())
        .and_then(|o| o.enabled)
        .unwrap_or(true)
}

/// Resolve compiled activation against one snapshot and an existing indexed catalog.
/// Missing ancestry and cycles fail closed; no registry is constructed here.
#[must_use]
pub fn effective_bidder_enabled<'a, F>(
    app: &AuctioneerAppConfig,
    bidder: &'a BidderInfo,
    lookup: F,
) -> bool
where
    F: Fn(&str) -> Option<&'a BidderInfo>,
{
    compiled_ancestry_enabled(bidder, lookup, |info| {
        !info.disabled
            && app
                .bidders
                .get(&info.name)
                .and_then(|overlay| overlay.enabled)
                .or(info.default_enabled)
                .unwrap_or(true)
    })
}

fn compiled_ancestry_enabled<'a, F, P>(mut bidder: &'a BidderInfo, lookup: F, eligible: P) -> bool
where
    F: Fn(&str) -> Option<&'a BidderInfo>,
    P: Fn(&BidderInfo) -> bool,
{
    for _ in 0..MAX_CATALOG_IDENTITIES {
        if !eligible(bidder) {
            return false;
        }
        let Some(parent_name) = bidder.alias_of.as_deref() else {
            return true;
        };
        let Some(parent) = lookup(parent_name) else {
            return false;
        };
        bidder = parent;
    }
    false
}

/// Sync eligibility without an ancestry catalog. Compiled aliases fail closed.
#[must_use]
pub fn sync_bidder_enabled(app: &AuctioneerAppConfig, requested: &str, base: &BidderInfo) -> bool {
    sync_bidder_enabled_in_catalog(app, requested, base, |_| None)
}

/// Sync-specific eligibility retains every compiled ancestor's activation and sync vetoes.
#[must_use]
pub fn sync_bidder_enabled_in_catalog<'a, F>(
    app: &AuctioneerAppConfig,
    requested: &str,
    base: &'a BidderInfo,
    lookup: F,
) -> bool
where
    F: Fn(&str) -> Option<&'a BidderInfo>,
{
    effective_bidder_enabled(app, base, &lookup)
        && compiled_ancestry_enabled(base, lookup, |info| {
            info.user_sync.enabled
                && app
                    .bidders
                    .get(&info.name)
                    .is_none_or(|overlay| overlay.usersync_enabled.unwrap_or(true))
        })
        && app.bidders.get(requested).is_none_or(|overlay| {
            overlay.enabled.unwrap_or(true) && overlay.usersync_enabled.unwrap_or(true)
        })
}

#[must_use]
pub fn resolve_cooperative(
    request: Option<bool>,
    account: &AccountConfig,
    host: &HostConfig,
) -> bool {
    request
        .or(account.cookie_sync.default_coop_sync)
        .unwrap_or_else(|| {
            host.user_sync
                .as_ref()
                .is_some_and(|sync| sync.cooperative.enabled_by_default)
        })
}

/// `None` means unlimited. A positive account maximum still caps an
/// unlimited request/default; no signed-to-unsigned cast occurs before the
/// positive check.
#[must_use]
pub fn resolve_limit(request: Option<i64>, account: &AccountConfig) -> Option<i64> {
    let selected = request
        .or(account.cookie_sync.default_limit)
        .filter(|value| *value > 0);
    let maximum = account.cookie_sync.max_limit.filter(|value| *value > 0);
    match (selected, maximum) {
        (Some(value), Some(max)) => Some(value.min(max)),
        (None, Some(max)) => Some(max),
        (value, None) => value,
    }
}

/// Per-bidder timeout override (milliseconds). `None` → the
/// build-time `BidderInfo.bidder_timeout_ms` applies; if that is
/// also `None`, the exchange falls back to `bidder_visible_tmax_ms`.
#[must_use]
pub fn bidder_timeout_ms(app: &AuctioneerAppConfig, bidder: &BidderName) -> Option<u32> {
    app.bidders.get(bidder.as_str()).and_then(|o| o.timeout_ms)
}

/// Collect every `[bidders.<name>].timeout_ms` override into the
/// `BTreeMap<BidderName, u32>` shape `AuctionContext` expects. Empty
/// map when no bidder has an override — the exchange's built-in
/// fallback chain (baked `BidderInfo.bidder_timeout_ms` →
/// `bidder_visible_tmax_ms`) then applies unchanged.
#[must_use]
pub fn resolved_bidder_timeouts(app: &AuctioneerAppConfig) -> BTreeMap<BidderName, u32> {
    let mut out: BTreeMap<BidderName, u32> = BTreeMap::new();
    for (name, override_cfg) in &app.bidders {
        if let Some(timeout) = override_cfg.timeout_ms {
            out.insert(BidderName::new(name.as_str()), timeout);
        }
    }
    out
}

/// Build the effective `ExchangeFeatures` = `auctioneer.toml` values
/// where set, `ExchangeFeatures::default()` fill-in otherwise.
#[must_use]
pub fn resolved_features(app: &AuctioneerAppConfig) -> ExchangeFeatures {
    let baseline = ExchangeFeatures::default();
    ExchangeFeatures {
        gzip_outbound: app.features.gzip_outbound.unwrap_or(baseline.gzip_outbound),
        returnallbidstatus_default: app
            .features
            .returnallbidstatus_default
            .unwrap_or(baseline.returnallbidstatus_default),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuctioneerAppConfig, bidder_enabled, resolve_cooperative, resolve_limit, resolved_account,
        resolved_host, sync_bidder_enabled,
    };
    use crate::config::validate_effective;
    use crate::outbound::wave::tests::test_limits;
    use crate::test_support::config_store::configured_sync_host;
    use auctioneer_bidder::BidderName;
    use auctioneer_config_data::activity::Activity;
    use auctioneer_config_data::types::{
        AccountConfig, BidderInfo, HostConfig, OptoutCookie, ResourceLimits, TriState,
    };
    use auctioneer_runtime::ResolvedIsolateLimits;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use std::sync::Arc;
    use validator::Validate as _;

    #[test]
    fn disabled_publisher_survives_unrelated_opaque_numeric_overflow() {
        let app: AuctioneerAppConfig =
            toml::from_str("[accounts.pub]\ndisabled=true\n").expect("config");
        // Literal exemption: json! cannot preserve the out-of-range 1e400 token.
        let body =
            br#"{"site":{"publisher":{"id":"pub"}},"imp":[{"banner":{"ext":{"opaque":1e400}}}]}"#;
        let nodes = auctioneer_runtime::count_json_nodes(body, 100).expect("valid syntax");
        assert!(
            matches!(super::resolve_account(body, &app, &baked_account(), nodes), Err(auctioneer_exchange::AuctionError::AccountDisabled(id)) if id == "pub")
        );
        assert!(matches!(
            super::resolve_account(body, &app, &baked_account(), nodes - 1),
            Err(auctioneer_exchange::AuctionError::BadInput(_))
        ));
    }

    #[test]
    fn nonstring_publisher_candidates_do_not_mask_lower_priority_identity() {
        let app: AuctioneerAppConfig =
            toml::from_str("[accounts.pub]\ndisabled=true\n").expect("config");
        for wire in [
            // Literal exemption: preserve the nonstring app's 1e400 token.
            br#"{"app":1e400,"site":{"publisher":{"id":"pub"}}}"#.as_slice(),
            // Literal exemption: preserve the nonstring publisher id's 1e400 token.
            br#"{"app":{"publisher":{"id":1e400}},"site":{"publisher":{"id":"pub"}}}"#.as_slice(),
            // Literal exemption: preserve the nonstring parentAccount's 1e400 token.
            br#"{"site":{"publisher":{"ext":{"prebid":{"parentAccount":1e400}},"id":"pub"}}}"#
                .as_slice(),
        ] {
            assert!(
                matches!(super::resolve_account(wire, &app, &baked_account(), 100), Err(auctioneer_exchange::AuctionError::AccountDisabled(id)) if id == "pub")
            );
        }
    }

    #[test]
    fn account_discovery_preserves_order_escaped_keys_and_last_duplicate_values() {
        let app = AuctioneerAppConfig::default();
        let locations = [
            serde_json::json!({"app":{"publisher":{"ext":{"prebid":{"parentAccount":"app-parent"}},"id":"app-id"}}}),
            serde_json::json!({"app":{"publisher":{"id":"app-id"}}}),
            serde_json::json!({"site":{"publisher":{"ext":{"prebid":{"parentAccount":"site-parent"}},"id":"site-id"}}}),
            serde_json::json!({"site":{"publisher":{"id":"site-id"}}}),
            serde_json::json!({"dooh":{"publisher":{"ext":{"prebid":{"parentAccount":"dooh-parent"}},"id":"dooh-id"}}}),
            serde_json::json!({"dooh":{"publisher":{"id":"dooh-id"}}}),
        ];
        for (index, expected) in [
            "app-parent",
            "app-id",
            "site-parent",
            "site-id",
            "dooh-parent",
            "dooh-id",
        ]
        .iter()
        .enumerate()
        {
            let mut body = serde_json::json!({});
            for location in locations[index..].iter().rev() {
                for (key, value) in location.as_object().expect("object") {
                    body[key] = value.clone();
                }
            }
            let wire = serde_json::to_vec(&body).expect("wire");
            assert_eq!(
                super::resolve_account(&wire, &app, &baked_account(), 100)
                    .expect("account")
                    .id,
                *expected
            );
        }
        for wire in [
            // Literal exemption: json! would collapse the duplicate publisher id keys.
            br#"{"site":{"publisher":{"id":"first","id":"last"}},"opaque":1e400}"#.as_slice(),
            // Literal exemption: the original escaped key spelling must reach discovery.
            br#"{"s\u0069te":{"publisher":{"id":"last"}},"opaque":1e400}"#.as_slice(),
            // Literal exemption: preserve 1e400 while empty/nonstring candidates fall through.
            br#"{"app":{"publisher":{"id":false,"ext":{"prebid":{"parentAccount":""}}}},"site":{"publisher":{"id":"last"}},"opaque":1e400}"#.as_slice(),
        ] {
            assert_eq!(super::resolve_account(wire, &app, &baked_account(), 100).expect("account").id, "last");
        }
    }

    #[test]
    fn discovery_failure_is_not_absent_identity_and_depth_remains_bounded() {
        let app = AuctioneerAppConfig::default();
        // Literal exemption: incomplete JSON must reach the resolver unchanged.
        assert!(matches!(
            super::resolve_account(
                br#"{"site":{"publisher":{"id":"pub"}}"#,
                &app,
                &baked_account(),
                100
            ),
            Err(auctioneer_exchange::AuctionError::BadInput(_))
        ));
        for depth in [126, 127, 128, 129] {
            let nested = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
            let was_valid = auctioneer_runtime::value_within(nested.as_bytes(), 1000).is_ok();
            assert_eq!(
                super::resolve_account(nested.as_bytes(), &app, &baked_account(), 1000).is_ok(),
                was_valid,
                "depth {depth}"
            );
        }
        let body = format!(
            "{{\"site\":{{\"publisher\":{{\"id\":\"pub\"}}}},\"opaque\":{}0{}}}",
            "[".repeat(130),
            "]".repeat(130)
        );
        assert!(matches!(
            super::resolve_account(body.as_bytes(), &app, &baked_account(), 1000),
            Err(auctioneer_exchange::AuctionError::BadInput(_))
        ));
    }

    #[test]
    fn absent_null_and_empty_deep_overlays_inherit_independent_nondefault_siblings() {
        let mut host = baked_host();
        host.gdpr.tcf2.enabled = false;
        host.gdpr.tcf2.purpose_one_treatment.access_allowed = true;
        host.host_cookie.domain = Some("publisher.example".into());
        host.user_sync
            .as_mut()
            .unwrap()
            .cooperative
            .enabled_by_default = true;
        host.user_sync.as_mut().unwrap().priority_groups = vec![vec!["fake_pp".into()]];
        let mut account = baked_account();
        account.gdpr.channel_enabled.web = TriState::Disabled;
        account.gdpr.basic_enforcement_vendors = vec!["fake_pp".into()];
        account.privacy.ipv4.anon_keep_bits = 16;
        account.privacy.ipv6.anon_keep_bits = 48;
        account.cookie_sync.default_limit = Some(7);
        for overlay in [
            serde_json::json!({}),
            serde_json::json!({"gdpr":null,"host_cookie":null,"user_sync":null,"accounts":{"pub":{"gdpr":null,"privacy":null,"cookie_sync":null}}}),
            serde_json::json!({"gdpr":{},"host_cookie":{},"user_sync":{},"accounts":{"pub":{"gdpr":{},"privacy":{},"cookie_sync":{}}}}),
            serde_json::json!({"gdpr":{"tcf2":null},"host_cookie":{"domain":null},"user_sync":{"cooperative":null,"priority_groups":null},"accounts":{"pub":{"gdpr":{"channel_enabled":null,"basic_enforcement_vendors":null},"privacy":{"ipv4":null,"ipv6":null},"cookie_sync":{"default_limit":null}}}}),
            serde_json::json!({"gdpr":{"tcf2":{"enabled":null,"purpose_one_treatment":{}}},"user_sync":{"cooperative":{"enabled_by_default":null}},"accounts":{"pub":{"gdpr":{"channel_enabled":{"web":null}},"privacy":{"ipv4":{"anon_keep_bits":null},"ipv6":{}}}}}),
        ] {
            let app: AuctioneerAppConfig = serde_json::from_value(overlay).expect("overlay");
            let resolved = super::resolve_host(&app, &host);
            let selected = super::resolve_selected_account(
                super::AccountSelection::Named("pub".into()),
                &app,
                &account,
            );
            assert_eq!(
                serde_json::to_value(resolved).expect("host"),
                serde_json::to_value(&host).expect("baked host")
            );
            assert_eq!(
                serde_json::to_value(selected.config).expect("account"),
                serde_json::to_value(&account).expect("baked account")
            );
        }
    }

    #[test]
    fn explicit_deep_values_replace_arrays_without_resetting_sibling_fields() {
        let mut host = baked_host();
        host.host_cookie.domain = Some("publisher.example".into());
        host.host_cookie.max_cookie_size_bytes = 1024;
        host.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "optout".into(),
            value: "yes".into(),
        });
        host.user_sync
            .as_mut()
            .unwrap()
            .cooperative
            .enabled_by_default = true;
        host.user_sync.as_mut().unwrap().priority_groups = vec![vec!["fake_aaa".into()]];
        host.user_sync.as_mut().unwrap().allowed_origins = vec!["https://publisher.example".into()];
        let mut account = baked_account();
        account.debug_allow = true;
        account.cookie_sync.default_limit = Some(7);
        account.cookie_sync.max_limit = Some(9);
        account.privacy.ipv4.anon_keep_bits = 16;
        account.privacy.ipv6.anon_keep_bits = 48;
        account.gdpr.basic_enforcement_vendors = vec!["fake_aaa".into()];
        for replacement in [Vec::<String>::new(), vec!["fake_pp".to_owned()]] {
            let app: AuctioneerAppConfig = serde_json::from_value(serde_json::json!({
                "host_cookie":{"optout_cookie":{"name":"","value":""},"max_cookie_size_bytes":0_i32},
                "user_sync":{"cooperative":{"enabled_by_default":false},"priority_groups":[],"allowed_origins":[]},
                "accounts":{"pub":{"debug_allow":false,"cookie_sync":{"default_limit":0_i32,"priority_groups":[]},"privacy":{"ipv4":{"anon_keep_bits":0_i32}},"gdpr":{"basic_enforcement_vendors":replacement}}}
            })).expect("explicit overlay");
            validate_effective(&app, &host, &account).expect("valid explicit values");
            let resolved = super::resolve_host(&app, &host);
            assert_eq!(resolved.host_cookie.domain, host.host_cookie.domain);
            let optout = resolved
                .host_cookie
                .optout_cookie
                .expect("explicit empty marker");
            assert_eq!(optout.name, "");
            assert_eq!(optout.value, "");
            assert_eq!(resolved.host_cookie.max_cookie_size_bytes, 0);
            assert_eq!(
                resolved.host_cookie.cookie_name,
                host.host_cookie.cookie_name
            );
            assert!(
                !resolved
                    .user_sync
                    .as_ref()
                    .unwrap()
                    .cooperative
                    .enabled_by_default
            );
            assert!(
                resolved
                    .user_sync
                    .as_ref()
                    .unwrap()
                    .priority_groups
                    .is_empty()
            );
            assert!(
                resolved
                    .user_sync
                    .as_ref()
                    .unwrap()
                    .allowed_origins
                    .is_empty()
            );
            let selected = super::resolve_selected_account(
                super::AccountSelection::Named("pub".into()),
                &app,
                &account,
            );
            assert!(!selected.config.debug_allow);
            assert_eq!(selected.config.cookie_sync.default_limit, Some(0));
            assert_eq!(selected.config.cookie_sync.max_limit, Some(9));
            assert_eq!(
                selected.config.cookie_sync.priority_groups,
                Some(Vec::new())
            );
            assert_eq!(selected.config.privacy.ipv4.anon_keep_bits, 0);
            assert_eq!(selected.config.privacy.ipv6.anon_keep_bits, 48);
            assert_eq!(selected.config.gdpr.basic_enforcement_vendors, replacement);
        }
    }

    #[test]
    fn effective_account_tristate_not_raw_overlay_absence_controls_host_fallthrough() {
        use auctioneer_privacy::{
            GdprSignalIn, SyncEndpoint, SyncPrivacyInput, gcl::Gcl, gvl::GvlStore,
            resolve_sync_privacy,
        };
        let mut host = baked_host();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        let mut account = baked_account();
        account.gdpr.channel_enabled.web = TriState::Disabled;
        for (overlay, expected) in [
            (serde_json::json!({}), false),
            (
                serde_json::json!({"gdpr":{"channel_enabled":{"web":null}}}),
                false,
            ),
            (
                serde_json::json!({"gdpr":{"channel_enabled":{"web":"unset"}}}),
                true,
            ),
            (
                serde_json::json!({"gdpr":{"channel_enabled":{"web":"enabled"}}}),
                true,
            ),
        ] {
            let app: AuctioneerAppConfig =
                serde_json::from_value(serde_json::json!({"accounts":{"pub":overlay}}))
                    .expect("overlay");
            let selected = super::resolve_selected_account(
                super::AccountSelection::Named("pub".into()),
                &app,
                &account,
            );
            let privacy = resolve_sync_privacy(SyncPrivacyInput {
                gdpr: GdprSignalIn::CookieNumber(0),
                consent: None,
                us_privacy: None,
                gpp: None,
                gpp_sid: None,
                account: &selected.config,
                host: &host,
                endpoint: SyncEndpoint::CookieSync,
                gvl: GvlStore::baked(),
                gcl: Gcl::baked(),
                now: time::OffsetDateTime::UNIX_EPOCH,
            })
            .expect("privacy");
            assert_eq!(privacy.gdpr_permissions_enabled, expected);
        }
    }

    #[test]
    fn cooperative_and_limit_precedence_preserves_nonpositive_unlimited() {
        let mut host = configured_sync_host();
        host.user_sync
            .as_mut()
            .unwrap()
            .cooperative
            .enabled_by_default = true;
        let mut account = AccountConfig::default();
        assert!(resolve_cooperative(None, &account, &host));
        account.cookie_sync.default_coop_sync = Some(false);
        assert!(!resolve_cooperative(None, &account, &host));
        assert!(resolve_cooperative(Some(true), &account, &host));
        assert_eq!(resolve_limit(None, &account), None);
        account.cookie_sync.default_limit = Some(9);
        assert_eq!(resolve_limit(None, &account), Some(9));
        assert_eq!(resolve_limit(Some(2), &account), Some(2));
        assert_eq!(resolve_limit(Some(0), &account), None);
        account.cookie_sync.max_limit = Some(4);
        assert_eq!(resolve_limit(Some(0), &account), Some(4));
        assert_eq!(resolve_limit(Some(i64::MAX), &account), Some(4));
        account.cookie_sync.max_limit = Some(-1);
        assert_eq!(resolve_limit(Some(0), &account), None);
    }

    #[test]
    fn alias_cannot_reenable_a_disabled_base_or_baked_syncer() {
        let disabled_base_cfg: AuctioneerAppConfig = toml::from_str(
            "[bidders.fake_pp]\nusersync_enabled = false\n[bidders.fake_alias]\nusersync_enabled = true\n",
        )
        .expect("runtime TOML parses");
        let mut baked = BidderInfo {
            name: "fake_pp".into(),
            ..BidderInfo::default()
        };
        baked.user_sync.enabled = true;
        assert!(!sync_bidder_enabled(
            &disabled_base_cfg,
            "fake_alias",
            &baked
        ));
        let enabled_alias_cfg: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_alias]\nusersync_enabled = true\n")
                .expect("runtime TOML parses");
        baked.disabled = true;
        assert!(!sync_bidder_enabled(
            &enabled_alias_cfg,
            "fake_alias",
            &baked
        ));
        baked.disabled = false;
        baked.user_sync.enabled = false;
        assert!(!sync_bidder_enabled(
            &enabled_alias_cfg,
            "fake_alias",
            &baked
        ));
        baked.user_sync.enabled = true;
        assert!(sync_bidder_enabled(
            &enabled_alias_cfg,
            "fake_alias",
            &baked
        ));
    }

    #[test]
    fn boot_clamps_all_construction_limits_without_widening_smaller_overlays() {
        let boot = test_limits();
        for smaller in [false, true] {
            let n = if smaller { 1 } else { usize::MAX };
            let bytes = if smaller { 1024 } else { u64::MAX };
            let mut host = baked_host();
            host.resource_limits.max_bidders_per_auction = n;
            host.resource_limits.max_outbound_requests = n;
            host.resource_limits.max_bidder_response_bytes = n;
            host.resource_limits.max_outbound_raw_bytes = bytes;
            host.resource_limits.max_outbound_request_bytes = bytes;
            host.resource_limits.max_transport_bytes = bytes;
            host.resource_limits.max_decoded_bytes = bytes;
            host.resource_limits.max_parsed_nodes = if smaller { 1 } else { u32::MAX };
            super::clamp_host_to_boot(&mut host, &boot);
            let actual = &host.resource_limits;
            assert_eq!(
                actual.max_bidders_per_auction,
                n.min(usize::try_from(boot.max_bidders_per_auction).unwrap())
            );
            assert_eq!(
                actual.max_outbound_requests,
                n.min(usize::try_from(boot.max_outbound_requests).unwrap())
            );
            assert_eq!(
                actual.max_bidder_response_bytes,
                n.min(usize::try_from(boot.max_decoded_bytes).unwrap())
            );
            assert_eq!(
                actual.max_outbound_raw_bytes,
                bytes.min(boot.max_outbound_raw_bytes)
            );
            assert_eq!(
                actual.max_outbound_request_bytes,
                bytes.min(boot.max_outbound_request_bytes)
            );
            assert_eq!(
                actual.max_transport_bytes,
                bytes.min(boot.max_transport_bytes)
            );
            assert_eq!(actual.max_decoded_bytes, bytes.min(boot.max_decoded_bytes));
            assert_eq!(
                actual.max_parsed_nodes,
                if smaller { 1 } else { boot.max_parsed_nodes }
            );
        }
    }

    fn baked_host() -> HostConfig {
        HostConfig {
            status_response: "OK".into(),
            resource_limits: ResourceLimits {
                default_tmax_ms: 1000,
                max_tmax_ms: 2000,
                max_bidder_response_bytes: 0x0004_0000,
                max_bidders_per_auction: 50,
                max_outbound_requests: 100,
                max_outbound_raw_bytes: 0x0004_0000,
                max_outbound_request_bytes: 0x0004_0000,
                max_transport_bytes: 0x0004_0000,
                max_decoded_bytes: 0x0004_0000,
                max_parsed_nodes: 0x0001_0000,
            },
            ..configured_sync_host()
        }
    }

    /// An envelope row for the narrowing test.
    ///
    /// Only the caps the narrowing touches carry meaningful figures —
    /// the shipped 256 KiB byte caps and the 65536 node budget. Every
    /// other field is a distinct non-zero placeholder, which is what makes
    /// "nothing outside the five moved" an assertion with teeth: a
    /// narrowing that reset a field to its `Default` would be caught.
    /// This row is never installed and never validated, so it does not
    /// have to balance as a real envelope.
    fn envelope_row() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 1_600_000_000,
            baseline_bytes: 96_000_000,
            inbound_request_bytes: 0x0004_0000,
            inbound_parse_factor: 5,
            max_outbound_request_bytes: 0x0004_0000,
            max_outbound_raw_bytes: 0x0004_0000,
            max_transport_bytes: 0x0004_0000,
            max_decoded_bytes: 0x0004_0000,
            response_parse_factor: 7,
            response_node_peak_bytes: 9,
            final_response_bytes: 0x0040_0000,
            egress_permits: 3,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 11,
            aux_route_permits: 5,
            // The derived floor: the runtime-config extraction an aux
            // route performs plus the manifest it renders. The validator
            // refuses anything under it.
            aux_route_peak_bytes: 31_318_016,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: 0x0080_0000,
            max_bidders_per_auction: 50,
            max_outbound_requests: 64,
            max_concurrent_auctions: 2,
            max_active_body_streams: 7,
            max_runtime_config_bytes: 0x0010_0000,
            max_runtime_config_blob_bytes: 0x0010_0000,
            max_parsed_nodes: 0x0001_0000,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 0x0000_4000,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 68,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        }
    }

    fn baked_account() -> AccountConfig {
        AccountConfig {
            targeting_prefix: "hb".into(),
            bid_rounding: "down".into(),
            ..AccountConfig::default()
        }
    }

    #[test]
    fn empty_overlay_returns_baked_host() {
        let app = AuctioneerAppConfig::default();
        let resolved = resolved_host(&app, &baked_host());
        assert_eq!(resolved.resource_limits.default_tmax_ms, 1000);
        assert_eq!(resolved.status_response, "OK");
    }

    #[test]
    fn overlay_overrides_tmax_and_status() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[auction]\ndefault_tmax_ms = 750\nmax_bidders_per_auction = 30\n\n\
             [status]\nbanner = \"auctioneer dev\"\n",
        )
        .unwrap();
        let resolved = resolved_host(&app, &baked_host());
        assert_eq!(resolved.resource_limits.default_tmax_ms, 750);
        assert_eq!(resolved.resource_limits.max_bidders_per_auction, 30);
        assert_eq!(resolved.status_response, "auctioneer dev");
        // Fields not overridden stay at baked values.
        assert_eq!(resolved.resource_limits.max_tmax_ms, 2000);
    }

    #[test]
    fn overlay_overrides_targeting_prefix() {
        let app: AuctioneerAppConfig = toml::from_str("[targeting]\nprefix = \"pb\"\n").unwrap();
        let resolved = resolved_account(&app, &baked_account());
        assert_eq!(resolved.targeting_prefix, "pb");
        assert_eq!(resolved.bid_rounding, "down"); // untouched
    }

    #[test]
    fn per_bidder_enabled_flag_honored() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[bidders.fake_aaa]\nenabled = false\n\n\
             [bidders.fake_bbb]\nenabled = true\n",
        )
        .unwrap();
        assert!(!bidder_enabled(&app, &BidderName::new("fake_aaa")));
        assert!(bidder_enabled(&app, &BidderName::new("fake_bbb")));
        // Unlisted bidder defaults to enabled.
        assert!(bidder_enabled(&app, &BidderName::new("fake_pp")));
    }

    #[test]
    fn catalog_activation_is_snapshot_local_and_preserves_compiled_ancestry_vetoes() {
        let base = BidderInfo {
            name: "fake_base".into(),
            default_enabled: Some(false),
            ..BidderInfo::default()
        };
        let alias = BidderInfo {
            name: "fake_alias".into(),
            alias_of: Some(base.name.clone()),
            default_enabled: Some(false),
            ..BidderInfo::default()
        };
        for (overlay, base_enabled, alias_enabled) in [
            ("", false, false),
            ("[bidders.fake_alias]\nenabled = true\n", false, false),
            (
                "[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
                true,
                true,
            ),
            (
                "[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = false\n",
                true,
                false,
            ),
            (
                "[bidders.fake_base]\nenabled = false\n[bidders.fake_alias]\nenabled = true\n",
                false,
                false,
            ),
            (
                "[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
                true,
                true,
            ),
        ] {
            let app: AuctioneerAppConfig = toml::from_str(overlay).unwrap();
            let lookup = |name: &str| (name == base.name).then_some(&base);
            assert_eq!(
                super::effective_bidder_enabled(&app, &base, lookup),
                base_enabled
            );
            assert_eq!(
                super::effective_bidder_enabled(&app, &alias, lookup),
                alias_enabled
            );
            assert!(!super::effective_bidder_enabled(&app, &alias, |_| None));
        }
        let app: AuctioneerAppConfig = toml::from_str(
            "[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
        )
        .unwrap();
        let vetoed = BidderInfo {
            disabled: true,
            ..base.clone()
        };
        assert!(!super::effective_bidder_enabled(&app, &alias, |_| Some(
            &vetoed
        )));
        let cycle = BidderInfo {
            alias_of: Some(alias.name.clone()),
            ..base.clone()
        };
        assert!(!super::effective_bidder_enabled(&app, &alias, |name| {
            if name == base.name {
                Some(&cycle)
            } else {
                Some(&alias)
            }
        }));
    }

    #[test]
    fn compiled_sync_aliases_retain_base_activation_and_sync_vetoes() {
        let mut base = BidderInfo {
            name: "fake_base".into(),
            default_enabled: Some(false),
            ..BidderInfo::default()
        };
        base.user_sync.enabled = true;
        let mut alias = BidderInfo {
            name: "fake_alias".into(),
            alias_of: Some(base.name.clone()),
            default_enabled: Some(false),
            ..base.clone()
        };
        alias.user_sync.enabled = true;
        for (overlay, expected) in [
            (
                "[bidders.fake_alias]\nenabled = true\nusersync_enabled = true\n",
                false,
            ),
            (
                "[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
                true,
            ),
            (
                "[bidders.fake_base]\nenabled = true\nusersync_enabled = false\n[bidders.fake_alias]\nenabled = true\nusersync_enabled = true\n",
                false,
            ),
        ] {
            let app: AuctioneerAppConfig = toml::from_str(overlay).unwrap();
            assert_eq!(
                super::sync_bidder_enabled_in_catalog(&app, "fake_host_alias", &alias, |name| {
                    (name == base.name).then_some(&base)
                }),
                expected
            );
        }
        base.user_sync.enabled = false;
        let app: AuctioneerAppConfig = toml::from_str("[bidders.fake_base]\nenabled = true\nusersync_enabled = true\n[bidders.fake_alias]\nenabled = true\nusersync_enabled = true\n").unwrap();
        assert!(!super::sync_bidder_enabled_in_catalog(
            &app,
            "fake_host_alias",
            &alias,
            |_| Some(&base)
        ));
    }

    #[test]
    fn resolved_features_uses_baseline_when_absent() {
        let app = AuctioneerAppConfig::default();
        let features = super::resolved_features(&app);
        assert!(features.gzip_outbound);
        assert!(!features.returnallbidstatus_default);
    }

    #[test]
    fn resolved_features_applies_overlay() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[features]\ngzip_outbound = false\nreturnallbidstatus_default = true\n",
        )
        .unwrap();
        let features = super::resolved_features(&app);
        assert!(!features.gzip_outbound);
        assert!(features.returnallbidstatus_default);
    }

    /// Every shipped provider's funded call count against the shipped
    /// baked default, plus an override on each side of the ceiling.
    ///
    /// The clamp itself is one shared rule, so what varies per provider
    /// is the FUNDED number — and those differ: the three wasm rows fund
    /// 32 calls while axum funds 64, against a baked default of 100. A
    /// table here rather than four installed envelopes because
    /// `boot_limits()` is one process-global `OnceLock`: a process can
    /// hold exactly one row, and `host_limits_clamped_to_envelope.rs`
    /// drives that installed path end to end. This covers the arithmetic
    /// for every row the fleet actually ships.
    #[test]
    fn every_shipped_provider_ceiling_bounds_the_baked_default() {
        // (provider, funded calls, funded bidders) — as each adapter's
        // `envelope.rs` declares them. Held as `u32`, the width
        // `ResolvedIsolateLimits` uses, and widened through `usize::from`
        // / `try_from` rather than `as`, which the workspace's
        // restriction lints deny for hiding a lossy conversion.
        let rows: [(&str, u32, u32); 4] = [
            ("cloudflare", 32, 50),
            ("fastly", 32, 50),
            ("spin", 32, 50),
            ("axum", 64, 50),
        ];
        for (provider, calls, bidders) in rows {
            let funded_calls = usize::try_from(calls).expect("a funded call count fits a usize");
            let funded_bidders =
                usize::try_from(bidders).expect("a funded bidder count fits a usize");
            assert!(
                BAKED_CALLS > funded_calls,
                "{provider}: the fixture only proves something while the baked default \
                 ({BAKED_CALLS}) is above the funded ceiling ({funded_calls})",
            );
            assert_eq!(
                super::funded_cap(BAKED_CALLS, Some(funded_calls)),
                funded_calls,
                "{provider}: the baked default must resolve down to the funded call count",
            );
            assert_eq!(
                super::funded_cap(50, Some(funded_bidders)),
                50,
                "{provider}: a baked bidder count equal to the funded one is unchanged",
            );
            // An override on each side of the ceiling.
            assert_eq!(
                super::funded_cap(funded_calls + 1, Some(funded_calls)),
                funded_calls,
                "{provider}: one call over the ceiling is clamped",
            );
            assert_eq!(
                super::funded_cap(funded_calls - 1, Some(funded_calls)),
                funded_calls - 1,
                "{provider}: one call under the ceiling is the operator's own number",
            );
        }
    }

    /// The baked `auctioneer.toml` call count, restated so the table above
    /// states what it is comparing against.
    const BAKED_CALLS: usize = 100;

    /// The three envelope-funded response/parse caps, at every width
    /// they are declared in: two `u64` byte caps and a `u32` node
    /// budget.
    ///
    /// Every shipped adapter funds the same three figures — 256 KiB,
    /// 256 KiB and 65536 — and the baked `auctioneer.toml` row declares
    /// exactly those, so at the DEFAULT the clamp is a no-op by design.
    /// That makes a fixture built only from the shipped numbers useless:
    /// `min(x, x) == x` holds with the clamp deleted. So the overrides
    /// here straddle the funded figure instead, which is the property
    /// the knob actually needs.
    #[test]
    fn each_response_cap_resolves_against_its_own_funded_figure() {
        const FUNDED_TRANSPORT: u64 = 0x0004_0000;
        const FUNDED_DECODED: u64 = 0x0004_0000;
        const FUNDED_NODES: u32 = 0x0001_0000;

        // The shipped baked value equals the funded one, so it passes
        // through. Stated rather than skipped: it is the shipped
        // situation, and "the surface changed no value" is what it
        // proves.
        assert_eq!(
            super::funded_cap(FUNDED_TRANSPORT, Some(FUNDED_TRANSPORT)),
            FUNDED_TRANSPORT,
        );
        assert_eq!(
            super::funded_cap(FUNDED_DECODED, Some(FUNDED_DECODED)),
            FUNDED_DECODED,
        );
        assert_eq!(
            super::funded_cap(FUNDED_NODES, Some(FUNDED_NODES)),
            FUNDED_NODES
        );

        // ABOVE the funded figure: clamped. A value above it would
        // falsify the boot-time proof that the envelope fits.
        assert_eq!(
            super::funded_cap(FUNDED_TRANSPORT + 1, Some(FUNDED_TRANSPORT)),
            FUNDED_TRANSPORT,
        );
        assert_eq!(
            super::funded_cap(u64::MAX, Some(FUNDED_DECODED)),
            FUNDED_DECODED,
        );
        assert_eq!(
            super::funded_cap(u32::MAX, Some(FUNDED_NODES)),
            FUNDED_NODES,
        );

        // BELOW it: the operator's own number, untouched. This is the
        // direction the surface exists for.
        assert_eq!(
            super::funded_cap(FUNDED_TRANSPORT - 1, Some(FUNDED_TRANSPORT)),
            FUNDED_TRANSPORT - 1,
        );
        assert_eq!(
            super::funded_cap(0x0000_8000_u64, Some(FUNDED_DECODED)),
            0x0000_8000,
        );
        assert_eq!(
            super::funded_cap(0x0000_1000_u32, Some(FUNDED_NODES)),
            0x1000
        );

        // With no envelope installed the configured value stands, at
        // each of the two widths.
        assert_eq!(super::funded_cap(u64::MAX, None), u64::MAX);
        assert_eq!(super::funded_cap(7_u32, None), 7);
    }

    /// The whole overlay-then-clamp path for the five settable byte /
    /// node caps, with no envelope installed so the overlay is
    /// observable on its own.
    ///
    /// `boot_limits()` is a process-global `OnceLock` and this crate's
    /// unit tests install nothing, so `funded` is `None` here and the
    /// clamp is the identity. That isolates the OTHER half of the
    /// surface: that a pushed `[auction]` value reaches the field at
    /// all. `host_limits_clamped_to_envelope.rs` installs a real row
    /// and covers the clamp end to end.
    ///
    /// Every cap gets a DISTINCT value, so a resolver that wrote one
    /// overlay field into the wrong `ResourceLimits` row is a failure
    /// here rather than a coincidence that passes.
    #[test]
    fn overlay_reaches_the_five_settable_byte_caps() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_outbound_raw_bytes = 16384\n\
             max_outbound_request_bytes = 8192\n\
             max_transport_bytes = 65536\nmax_decoded_bytes = 32768\n\
             max_parsed_nodes = 4096\n",
        )
        .expect("the override parses");
        let resolved = resolved_host(&app, &baked_host());
        assert_eq!(resolved.resource_limits.max_outbound_raw_bytes, 0x0000_4000);
        assert_eq!(
            resolved.resource_limits.max_outbound_request_bytes,
            0x0000_2000,
        );
        assert_eq!(resolved.resource_limits.max_transport_bytes, 0x0001_0000);
        assert_eq!(resolved.resource_limits.max_decoded_bytes, 0x0000_8000);
        assert_eq!(resolved.resource_limits.max_parsed_nodes, 0x0000_1000);
        // An unnamed cap keeps its baked value rather than falling to a
        // default.
        assert_eq!(
            resolved.resource_limits.max_bidder_response_bytes,
            0x0004_0000,
        );
    }

    /// `limits_for_request` narrows the five settable caps and nothing
    /// else.
    ///
    /// The row it narrows is the only path by which a resolved cap
    /// reaches the wave scheduler's response policy AND the outbound
    /// builder's request-body ceiling, so a version of it that returned
    /// the envelope row unchanged would leave the baked value in force
    /// at the read site while every resolver test still passed.
    #[test]
    fn limits_for_request_narrows_only_the_five_settable_caps() {
        let mut host = baked_host();
        host.resource_limits.max_outbound_raw_bytes = 0x0000_4000;
        host.resource_limits.max_outbound_request_bytes = 0x0000_2000;
        host.resource_limits.max_transport_bytes = 0x0001_0000;
        host.resource_limits.max_decoded_bytes = 0x0000_8000;
        host.resource_limits.max_parsed_nodes = 0x0000_1000;
        let envelope = envelope_row();
        let narrowed = super::limits_for_request(&envelope, &host);
        assert_eq!(narrowed.max_outbound_raw_bytes, 0x0000_4000);
        assert_eq!(narrowed.max_outbound_request_bytes, 0x0000_2000);
        assert_eq!(narrowed.max_transport_bytes, 0x0001_0000);
        assert_eq!(narrowed.max_decoded_bytes, 0x0000_8000);
        assert_eq!(narrowed.max_parsed_nodes, 0x0000_1000);
        assert_eq!(
            ResolvedIsolateLimits {
                max_outbound_raw_bytes: envelope.max_outbound_raw_bytes,
                max_outbound_request_bytes: envelope.max_outbound_request_bytes,
                max_transport_bytes: envelope.max_transport_bytes,
                max_decoded_bytes: envelope.max_decoded_bytes,
                max_parsed_nodes: envelope.max_parsed_nodes,
                ..narrowed
            },
            envelope,
            "no field outside the five settable caps may move",
        );
    }

    #[test]
    fn limits_for_request_cannot_widen_the_supplied_envelope() {
        let mut host = baked_host();
        host.resource_limits.max_outbound_raw_bytes = u64::MAX;
        host.resource_limits.max_outbound_request_bytes = u64::MAX;
        host.resource_limits.max_transport_bytes = u64::MAX;
        host.resource_limits.max_decoded_bytes = u64::MAX;
        host.resource_limits.max_parsed_nodes = u32::MAX;
        let envelope = envelope_row();
        assert_eq!(super::limits_for_request(&envelope, &host), envelope);
    }

    #[test]
    fn parsed_account_resolution_matches_bounded_bytes_and_shares_activities() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[accounts.pub.ccpa]\nenabled = 'enabled'\n\n\
             [accounts.pub.cookie_sync]\ndefault_limit = 9\nmax_limit = 4\n\
             default_coop_sync = false\npriority_groups = [['fake_pp']]\n\n\
             [accounts.pub.privacy.allowactivities.fetchBids]\n\
             default = false\nrules = [{ allow = true }]\n",
        )
        .expect("privacy and sync account overlay");
        let baked = baked_account();
        let body = serde_json::to_vec(&serde_json::json!({"site":{"publisher":{"id":"pub"}}}))
            .expect("request body");
        let parsed = auctioneer_runtime::value_within(&body, 100).expect("bounded tree");
        let from_bytes = super::resolve_account(&body, &app, &baked, 100).expect("bounded bytes");
        let from_tree = super::resolve_account_from_value(Some(&parsed), &app, &baked)
            .expect("already parsed tree");
        assert_eq!(from_bytes.id, from_tree.id);
        assert_eq!(from_bytes.selection, from_tree.selection);
        assert_eq!(
            from_tree.selection,
            super::AccountSelection::Named("pub".into())
        );
        assert_eq!(from_bytes.source, from_tree.source);
        assert_eq!(
            serde_json::to_value(&from_bytes.config).expect("bytes config"),
            serde_json::to_value(&from_tree.config).expect("tree config")
        );
        assert_eq!(resolve_limit(None, &from_tree.config), Some(4));
        assert!(!resolve_cooperative(None, &from_tree.config, &baked_host()));
        assert_eq!(
            from_tree.effective_priority_groups(&baked_host()),
            &[vec!["fake_pp".to_owned()]]
        );
        let activity = Activity::FetchBids;
        assert!(Arc::ptr_eq(
            &from_bytes.config.privacy.allowactivities[&activity].rules,
            &from_tree.config.privacy.allowactivities[&activity].rules
        ));
        let fallback =
            super::resolve_account_from_value(None, &app, &baked).expect("fallback account");
        assert_eq!(fallback.selection, super::AccountSelection::Default);
        assert!(Arc::ptr_eq(
            &fallback.config.privacy.allowactivities,
            &baked.privacy.allowactivities
        ));
    }

    #[test]
    fn bounded_auction_lookup_keeps_sync_selection_but_rejects_disabled_accounts() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[accounts.pub]\ndisabled = true\n\n\
             [accounts.pub.cookie_sync]\nmax_limit = 4\n\n\
             [accounts.unknown]\ndisabled = true\n",
        )
        .expect("disabled sync account overlay");
        let baked = baked_account();
        let selection = super::AccountSelection::Named("pub".into());
        let sync = super::resolve_selected_account(selection.clone(), &app, &baked);
        assert_eq!(sync.selection, selection);
        assert!(sync.config.disabled);
        assert_eq!(sync.config.cookie_sync.max_limit, Some(4));

        let body = serde_json::to_vec(&serde_json::json!({"site":{"publisher":{"id":"pub"}}}))
            .expect("request body");
        let parsed = auctioneer_runtime::value_within(&body, 100).expect("bounded tree");
        for result in [
            super::resolve_account(&body, &app, &baked, 100),
            super::resolve_account_from_value(Some(&parsed), &app, &baked),
        ] {
            assert!(matches!(
                result,
                Err(auctioneer_exchange::AuctionError::AccountDisabled(id)) if id == "pub"
            ));
        }
        assert!(matches!(
            super::resolve_account(&body, &app, &baked, 0),
            Err(auctioneer_exchange::AuctionError::BadInput(_))
        ));
        let fallback = super::resolve_account_from_value(None, &app, &baked)
            .expect("default skips accounts map");
        assert_eq!(fallback.selection, super::AccountSelection::Default);
        assert!(!fallback.config.disabled);
    }

    /// With no envelope installed the configured value stands. Clamping
    /// to zero would make an in-process test — which installs no
    /// envelope — call no bidders at all.
    #[test]
    fn no_installed_envelope_leaves_the_configured_value_alone() {
        assert_eq!(super::funded_cap(100_usize, None), 100);
        assert_eq!(super::funded_cap(0_usize, None), 0);
    }

    #[test]
    fn resolved_bidder_timeouts_collects_overrides() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[bidders.fake_aaa]\ntimeout_ms = 250\n\n\
             [bidders.fake_bbb]\nenabled = true\n",
        )
        .unwrap();
        let timeouts = super::resolved_bidder_timeouts(&app);
        assert_eq!(
            timeouts.get(&BidderName::new("fake_aaa")).copied(),
            Some(250_u32),
        );
        assert!(!timeouts.contains_key(&BidderName::new("fake_bbb")));
    }

    #[test]
    fn outbound_overlay_round_trips_and_preserves_smaller_caps() {
        let app: AuctioneerAppConfig = toml::from_str(
            "[auction]\nmax_outbound_raw_bytes = 1024\nmax_outbound_request_bytes = 2048\n",
        )
        .unwrap();
        app.validate().unwrap();
        let host = resolved_host(&app, auctioneer_config_data::host_default());
        assert_eq!(host.resource_limits.max_outbound_raw_bytes, 1024);
        assert_eq!(host.resource_limits.max_outbound_request_bytes, 2048);
        let round_trip: AuctioneerAppConfig =
            toml::from_str(&toml::to_string(&app).unwrap()).unwrap();
        let round_trip_host = resolved_host(&round_trip, auctioneer_config_data::host_default());
        assert_eq!(round_trip_host.resource_limits.max_outbound_raw_bytes, 1024);
        assert_eq!(
            round_trip_host.resource_limits.max_outbound_request_bytes,
            2048
        );
    }

    mod account_resolution {
        use crate::config::AuctioneerAppConfig;
        use crate::config::resolve::{ResolvedAccountOwned, resolve_account, resolve_host};
        use crate::test_support::config_store::configured_sync_host;
        use auctioneer_config_data::types::{
            AccountConfig, AccountGdprConfig, ChannelEnabled, EnforceAlgo, GdprConfig, HostConfig,
            PurposeEnforcement, PurposeOneTreatment, PurposeOneTreatmentOverride, ResourceLimits,
            SpecialFeatureEnforcement, Tcf2Config, TriState,
        };
        use auctioneer_exchange::{AccountSource, AuctionError};
        use std::collections::BTreeMap;

        fn app(toml_src: &str) -> AuctioneerAppConfig {
            toml::from_str(toml_src).expect("fixture TOML parses")
        }

        fn body(value: &serde_json::Value) -> Vec<u8> {
            serde_json::to_vec(value).expect("fixture body serialises")
        }

        /// A baked account carrying entries the overlay tests can leave
        /// untouched: purposes 2 and 7, special features 1 and 2, and two
        /// channel tri-states that are already decided.
        fn baked_account() -> AccountConfig {
            let mut purposes: BTreeMap<u8, PurposeEnforcement> = BTreeMap::new();
            purposes.insert(
                2_u8,
                PurposeEnforcement {
                    enforce_algo: EnforceAlgo::Full,
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Enabled,
                    vendor_exceptions: vec!["fake_baked_p2_vendor".to_owned()],
                },
            );
            purposes.insert(
                7_u8,
                PurposeEnforcement {
                    enforce_algo: EnforceAlgo::Basic,
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Disabled,
                    vendor_exceptions: vec!["fake_baked_p7_vendor".to_owned()],
                },
            );
            let mut special_features: BTreeMap<u8, SpecialFeatureEnforcement> = BTreeMap::new();
            special_features.insert(
                1_u8,
                SpecialFeatureEnforcement {
                    enforce: TriState::Enabled,
                    vendor_exceptions: vec!["fake_baked_sf1_vendor".to_owned()],
                },
            );
            special_features.insert(
                2_u8,
                SpecialFeatureEnforcement {
                    enforce: TriState::Disabled,
                    vendor_exceptions: vec!["fake_baked_sf2_vendor".to_owned()],
                },
            );
            AccountConfig {
                bid_rounding: "down".to_owned(),
                targeting_prefix: "hb".to_owned(),
                gdpr: AccountGdprConfig {
                    enabled: TriState::Unset,
                    channel_enabled: ChannelEnabled {
                        web: TriState::Enabled,
                        app: TriState::Disabled,
                        ..ChannelEnabled::default()
                    },
                    purposes,
                    special_features,
                    basic_enforcement_vendors: vec!["fake_baked_basic".to_owned()],
                    purpose_one_treatment: PurposeOneTreatmentOverride::default(),
                },
                ..AccountConfig::default()
            }
        }

        /// A baked host with populated enforcement maps and one alias, so a
        /// per-entry overlay has siblings that can be observed surviving.
        fn baked_host() -> HostConfig {
            let mut purposes: BTreeMap<u8, PurposeEnforcement> = BTreeMap::new();
            purposes.insert(
                1_u8,
                PurposeEnforcement {
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Enabled,
                    ..PurposeEnforcement::default()
                },
            );
            purposes.insert(
                7_u8,
                PurposeEnforcement {
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Disabled,
                    vendor_exceptions: vec!["fake_host_p7_vendor".to_owned()],
                    ..PurposeEnforcement::default()
                },
            );
            let mut special_features: BTreeMap<u8, SpecialFeatureEnforcement> = BTreeMap::new();
            special_features.insert(
                1_u8,
                SpecialFeatureEnforcement {
                    enforce: TriState::Enabled,
                    vendor_exceptions: vec![],
                },
            );
            special_features.insert(
                2_u8,
                SpecialFeatureEnforcement {
                    enforce: TriState::Disabled,
                    vendor_exceptions: vec!["fake_host_sf2_vendor".to_owned()],
                },
            );
            let mut aliases: BTreeMap<String, String> = BTreeMap::new();
            aliases.insert("fake_aaa_alias".to_owned(), "fake_pp".to_owned());
            aliases.insert("fake_bbb_alias".to_owned(), "fake_bbb".to_owned());
            HostConfig {
                gdpr: GdprConfig {
                    enabled: true,
                    default_value: "0".to_owned(),
                    host_vendor_id: None,
                    tcf2: Tcf2Config {
                        enabled: false,
                        purpose_one_treatment: PurposeOneTreatment::default(),
                        purposes,
                        special_features,
                    },
                    non_standard_publishers: vec![],
                },
                status_response: "OK".to_owned(),
                resource_limits: ResourceLimits {
                    default_tmax_ms: 1000,
                    max_tmax_ms: 2000,
                    max_bidder_response_bytes: 0x0004_0000,
                    max_bidders_per_auction: 50,
                    max_outbound_requests: 100,
                    max_outbound_raw_bytes: 0x0004_0000,
                    max_outbound_request_bytes: 0x0004_0000,
                    max_transport_bytes: 0x0004_0000,
                    max_decoded_bytes: 0x0004_0000,
                    max_parsed_nodes: 0x0001_0000,
                },
                aliases,
                ..configured_sync_host()
            }
        }

        fn resolved(
            app_cfg: &AuctioneerAppConfig,
            request: &serde_json::Value,
        ) -> ResolvedAccountOwned {
            resolve_account(&body(request), app_cfg, &baked_account(), u32::MAX)
                .expect("account resolves")
        }

        fn purpose(config: &AccountConfig, id: u8) -> &PurposeEnforcement {
            config
                .gdpr
                .purposes
                .get(&id)
                .expect("the resolved account carries this purpose")
        }

        #[test]
        fn account_id_falls_back_when_no_publisher_id_present_at_all() {
            let owned = resolved(
                &AuctioneerAppConfig::default(),
                &serde_json::json!({ "id": "auc-1", "imp": [] }),
            );
            assert_eq!(owned.id, "unknown");
            assert_eq!(owned.source, AccountSource::DefaultFallback);
        }

        #[test]
        fn account_lookup_counts_unknown_nodes_before_disabled_state() {
            let request = serde_json::json!({
                "site": {"publisher": {"id": "acct-42"}},
                "extra": {"deep": [{}, null]}
            });
            let app_cfg = app("[accounts.acct-42]\ndisabled = true\n");
            let encoded = body(&request);
            // Seven nodes along the account path, six in the extra subtree.
            let at_limit = resolve_account(&encoded, &app_cfg, &baked_account(), 13);
            assert!(matches!(at_limit, Err(AuctionError::AccountDisabled(_))));
            let over_limit = resolve_account(&encoded, &app_cfg, &baked_account(), 12);
            assert!(matches!(over_limit, Err(AuctionError::BadInput(message))
                if message == "too many JSON nodes"));
        }

        #[test]
        fn account_lookup_rejects_syntax_errors_and_node_exhaustion() {
            let app_cfg = AuctioneerAppConfig::default();
            // Direct resolver calls fail closed; the public auction handler already
            // validates JSON syntax before invoking this owner API.
            let malformed = resolve_account(b"[", &app_cfg, &baked_account(), 100);
            assert!(matches!(malformed, Err(AuctionError::BadInput(message))
                if message == "invalid account discovery JSON"));
            let over_limit = resolve_account(b"{}", &app_cfg, &baked_account(), 0);
            assert!(matches!(over_limit, Err(AuctionError::BadInput(message))
                if message == "too many JSON nodes"));
        }

        #[test]
        fn trailing_json_is_rejected_before_disabled_default_at_node_boundary() {
            let mut baked = baked_account();
            baked.disabled = true;
            let result = resolve_account(b"{} []", &AuctioneerAppConfig::default(), &baked, 1);
            assert!(matches!(result, Err(AuctionError::BadInput(message))
                if message == "invalid account discovery JSON"));
        }

        #[test]
        fn publisher_id_present_but_no_accounts_entry_returns_default_fallback() {
            // The "onboarded publisher" case: the request names an account
            // the deployment has no `[accounts.<id>]` section for. Not an
            // error — `prepare()` runs next and reports structural problems.
            let owned = resolved(
                &AuctioneerAppConfig::default(),
                &serde_json::json!({
                    "id": "auc-1",
                    "site": { "publisher": { "id": "acct-42" } },
                }),
            );
            assert_eq!(owned.id, "acct-42");
            assert_eq!(owned.source, AccountSource::DefaultFallback);
            assert_eq!(
                serde_json::to_value(&owned.config).expect("config serialises"),
                serde_json::to_value(baked_account()).expect("baked serialises"),
                "an unmatched publisher inherits the baked account untouched",
            );
        }

        #[test]
        fn account_id_resolves_from_site_publisher_id_when_app_absent() {
            let owned = resolved(
                &AuctioneerAppConfig::default(),
                &serde_json::json!({
                    "id": "auc-1",
                    "site": { "publisher": { "id": "site-acct" } },
                }),
            );
            assert_eq!(owned.id, "site-acct");
        }

        #[test]
        fn parent_account_takes_precedence_over_publisher_id() {
            let owned = resolved(
                &AuctioneerAppConfig::default(),
                &serde_json::json!({
                    "id": "auc-1",
                    "site": {
                        "publisher": {
                            "id": "child-9",
                            "ext": { "prebid": { "parentAccount": "parent-1" } },
                        },
                    },
                }),
            );
            assert_eq!(
                owned.id, "parent-1",
                "`parentAccount` outranks the sibling publisher id",
            );
        }

        #[test]
        fn disabled_account_returns_account_disabled_code_7() {
            let app_cfg = app("[accounts.acct-42]\ndisabled = true\n");
            let err = resolve_account(
                &body(&serde_json::json!({
                    "id": "auc-1",
                    "site": { "publisher": { "id": "acct-42" } },
                })),
                &app_cfg,
                &baked_account(),
                u32::MAX,
            )
            .expect_err("a disabled account is refused");
            assert_eq!(err.code(), 7_u32);
            assert!(
                matches!(&err, AuctionError::AccountDisabled(id) if id.as_str() == "acct-42"),
                "expected AccountDisabled(acct-42), got {err:?}",
            );
        }

        #[test]
        fn configured_source_reported_when_accounts_entry_matches() {
            let app_cfg =
                app("[accounts.acct-42]\ntargeting_prefix = \"pb\"\ndebug_allow = true\n");
            let owned = resolved(
                &app_cfg,
                &serde_json::json!({
                    "id": "auc-1",
                    "site": { "publisher": { "id": "acct-42" } },
                }),
            );
            assert_eq!(owned.source, AccountSource::Configured);
            assert_eq!(owned.config.targeting_prefix, "pb");
            assert!(owned.config.debug_allow);
            // Untouched baked fields survive the overlay.
            assert_eq!(owned.config.bid_rounding, "down");
        }

        #[test]
        fn account_overlay_of_one_purpose_entry_preserves_sibling_purposes() {
            let app_cfg = app("[accounts.acct-42.gdpr.purposes.2]\n\
                 enforce_purpose = \"disabled\"\n\n\
                 [accounts.acct-42.gdpr.special_features.1]\n\
                 enforce = \"disabled\"\n\n\
                 [accounts.acct-42.gdpr.channel_enabled]\n\
                 web = \"disabled\"\n");
            let owned = resolved(
                &app_cfg,
                &serde_json::json!({
                    "id": "auc-1",
                    "site": { "publisher": { "id": "acct-42" } },
                }),
            );
            let config = &owned.config;

            // The named purpose took the override on the field it names…
            assert_eq!(purpose(config, 2_u8).enforce_purpose, TriState::Disabled);
            // …and kept every field the override did NOT name.
            assert_eq!(purpose(config, 2_u8).enforce_vendors, TriState::Enabled);
            assert_eq!(purpose(config, 2_u8).enforce_algo, EnforceAlgo::Full);
            assert_eq!(
                purpose(config, 2_u8).vendor_exceptions,
                vec!["fake_baked_p2_vendor".to_owned()],
            );
            // The sibling entry the override never mentions is untouched —
            // a whole-map replacement would have deleted it.
            assert_eq!(purpose(config, 7_u8).enforce_purpose, TriState::Enabled);
            assert_eq!(purpose(config, 7_u8).enforce_vendors, TriState::Disabled);
            assert_eq!(purpose(config, 7_u8).enforce_algo, EnforceAlgo::Basic);
            assert_eq!(
                purpose(config, 7_u8).vendor_exceptions,
                vec!["fake_baked_p7_vendor".to_owned()],
            );

            // Same shape for special features…
            let sf = &config.gdpr.special_features;
            assert_eq!(
                sf.get(&1_u8).map(|entry| entry.enforce),
                Some(TriState::Disabled),
            );
            assert_eq!(
                sf.get(&1_u8).map(|entry| entry.vendor_exceptions.clone()),
                Some(vec!["fake_baked_sf1_vendor".to_owned()]),
                "an `enforce`-only override leaves the baked exception list alone",
            );
            assert_eq!(
                sf.get(&2_u8).map(|entry| entry.enforce),
                Some(TriState::Disabled),
            );
            assert_eq!(
                sf.get(&2_u8).map(|entry| entry.vendor_exceptions.clone()),
                Some(vec!["fake_baked_sf2_vendor".to_owned()]),
                "the unnamed sibling special feature survives",
            );

            // …and for the channel tri-states.
            assert_eq!(config.gdpr.channel_enabled.web, TriState::Disabled);
            assert_eq!(
                config.gdpr.channel_enabled.app,
                TriState::Disabled,
                "the baked `app` tri-state is untouched by a `web`-only override",
            );
            assert_eq!(
                config.gdpr.basic_enforcement_vendors,
                vec!["fake_baked_basic".to_owned()],
                "an unlisted account-only array keeps its baked value",
            );
        }

        #[test]
        fn account_privacy_overlay_sets_ipv4_only() {
            let app_cfg = app("[accounts.acme.privacy.ipv4]\nanon_keep_bits = 16\n");
            let owned = resolved(
                &app_cfg,
                &serde_json::json!({
                    "id": "auc-1",
                    "site": { "publisher": { "id": "acme" } },
                }),
            );
            let keep = owned.config.ip_keep_bits();
            assert_eq!(keep.v4, 16_u8, "the named family takes the override");
            assert_eq!(keep.v6, 56_u8, "the unnamed family keeps its baked default");
        }

        /// The fallback identity is synthesized, not request-supplied, so it
        /// must not be able to select a deployer's `[accounts.unknown]`
        /// section — that would let one TOML stanza reconfigure (or 503)
        /// every publisher-less request at once.
        #[test]
        fn unresolved_fallback_id_skips_the_accounts_map_lookup() {
            let app_cfg = app("[accounts.unknown]\ndisabled = true\ntargeting_prefix = \"zz\"\n");
            let owned = resolved(&app_cfg, &serde_json::json!({ "id": "auc-1", "imp": [] }));
            assert_eq!(owned.id, "unknown");
            assert_eq!(owned.source, AccountSource::DefaultFallback);
            assert_eq!(owned.config.targeting_prefix, "hb");
            assert!(!owned.config.disabled);
        }

        #[test]
        fn host_gdpr_overlay_enabled_true_flips_baked_default_false() {
            let app_cfg = app("[gdpr.tcf2]\nenabled = true\n");
            let host = resolve_host(&app_cfg, &baked_host());
            assert!(
                host.gdpr.tcf2.enabled,
                "without this overlay enforcement can never be turned on at runtime",
            );
        }

        #[test]
        fn resolve_host_preserves_auction_runtime_override_alongside_gdpr() {
            let app_cfg = app("[auction]\ndefault_tmax_ms = 900\n\n[gdpr.tcf2]\nenabled = true\n");
            let host = resolve_host(&app_cfg, &baked_host());
            assert_eq!(
                host.resource_limits.default_tmax_ms, 900,
                "the GDPR overlay must layer ON the existing overlay, not replace it",
            );
            assert!(host.gdpr.tcf2.enabled);
        }

        #[test]
        fn resolve_host_preserves_status_runtime_override_alongside_gdpr() {
            let app_cfg = app("[status]\nbanner = \"up\"\n\n[gdpr.tcf2]\nenabled = true\n");
            let host = resolve_host(&app_cfg, &baked_host());
            assert_eq!(host.status_response, "up");
            assert!(host.gdpr.tcf2.enabled);
        }

        #[test]
        fn resolve_host_overlay_of_one_purpose_entry_preserves_sibling_purposes() {
            let app_cfg = app("[gdpr.tcf2.purposes.1]\nenforce_purpose = \"disabled\"\n");
            let host = resolve_host(&app_cfg, &baked_host());
            let purposes = &host.gdpr.tcf2.purposes;
            assert_eq!(
                purposes.get(&1_u8).map(|entry| entry.enforce_purpose),
                Some(TriState::Disabled),
            );
            assert_eq!(
                purposes.get(&1_u8).map(|entry| entry.enforce_vendors),
                Some(TriState::Enabled),
                "the field the override did not name keeps its baked value",
            );
            assert_eq!(
                purposes.get(&7_u8).map(|entry| entry.enforce_purpose),
                Some(TriState::Enabled),
                "the sibling purpose survives a per-entry overlay",
            );
            assert_eq!(
                purposes
                    .get(&7_u8)
                    .map(|entry| entry.vendor_exceptions.clone()),
                Some(vec!["fake_host_p7_vendor".to_owned()]),
            );
        }

        #[test]
        fn resolve_host_overlay_of_one_special_feature_entry_preserves_siblings() {
            let app_cfg = app("[gdpr.tcf2.special_features.1]\nenforce = \"disabled\"\n");
            let host = resolve_host(&app_cfg, &baked_host());
            let features = &host.gdpr.tcf2.special_features;
            assert_eq!(
                features.get(&1_u8).map(|entry| entry.enforce),
                Some(TriState::Disabled),
            );
            assert_eq!(
                features.get(&2_u8).map(|entry| entry.enforce),
                Some(TriState::Disabled),
                "the sibling special feature survives",
            );
            assert_eq!(
                features
                    .get(&2_u8)
                    .map(|entry| entry.vendor_exceptions.clone()),
                Some(vec!["fake_host_sf2_vendor".to_owned()]),
            );
        }

        #[test]
        fn resolve_host_alias_overlay_preserves_baked_aliases() {
            let app_cfg = app(
                "[aliases]\nfake_cc_alias = \"fake_pp\"\nfake_aaa_alias = \"fake_d\"\n\n\
                 [gdpr.tcf2]\nenabled = true\n",
            );
            let host = resolve_host(&app_cfg, &baked_host());
            assert_eq!(
                host.aliases.get("fake_aaa_alias").map(String::as_str),
                Some("fake_d"),
                "a named key replaces its base bidder",
            );
            assert_eq!(
                host.aliases.get("fake_cc_alias").map(String::as_str),
                Some("fake_pp"),
                "a new key is added",
            );
            assert_eq!(
                host.aliases.get("fake_bbb_alias").map(String::as_str),
                Some("fake_bbb"),
                "a baked alias the override never names survives",
            );
            assert_eq!(
                host.aliases.len(),
                3,
                "per-key merge, not whole-map replacement: {:?}",
                host.aliases,
            );
        }
    }
}

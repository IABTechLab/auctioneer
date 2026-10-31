//! Typed application config for `auctioneer`, loaded from `auctioneer.toml`
//! via `edgezero_core::app_config::load_app_config::<AuctioneerAppConfig>`
//! at build time and pushed into `[stores.config]` at deploy time.
//!
//! Overlay-over-baked semantics: every field is `Option<_>`. `None`
//! falls through to the build-time TOML defaults emitted by
//! `auctioneer-config` from `auctioneer.toml`'s `[baked.*]` namespaces
//! (`host_default()` / `account_default()`);
//! `Some(value)` overrides at request time. The `AUCTIONEER__<SECTION>__…`
//! env-var overlay is applied on top by
//! `edgezero_core::app_config::load_app_config`.
//!
//! Runtime read path: handlers pull the typed config through the single
//! owner `config::runtime::resolve_runtime_config`, then merge it over
//! the baked defaults via the `resolve_host()` / `resolve_account()`
//! helpers in `auctioneer_core::config::resolve`. The deploy ships ONE blob:
//! the same TOML that carries auction and bidder overrides also carries
//! the host `[gdpr]` overlay and the per-publisher `[accounts.<id>]`
//! sections, so account resolution is a `BTreeMap` lookup rather than a
//! per-request store round trip.

use auctioneer_bidder::BidderName;
use auctioneer_config::FloorCurrencyConfig;
use auctioneer_config::activity::{ActivityOverrides, validate_activity_overrides};
use auctioneer_config::bidder_settings::ensure_limits as ensure_settings_limits;
use auctioneer_config::limits::MAX_STATUS_BANNER_BYTES;
use auctioneer_config::types::{
    AccountConfig, AccountCookieSyncConfig, EnforceAlgo, HostConfig, MAX_BIDDER_NAME_BYTES,
    PurposeOneTreatmentOverride, TriState,
};
use auctioneer_config::usersync::{
    validate_account_cookie_sync, validate_catalog_size, validate_host_cookie, validate_user_sync,
};
use core::num::ParseIntError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::RangeInclusive;
use validator::{Validate, ValidationErrors};

pub mod bidder_settings;
pub mod budgets;
mod floor_currency;
pub mod resolve;
pub mod runtime;

/// Structural registry failures carried by [`EffectiveConfigError::Registry`].
pub use auctioneer_bidder::RegistryBuildError;

/// Invalid runtime data is distinct from an immutable registry build failure.
#[derive(Debug, thiserror::Error)]
pub enum EffectiveConfigError {
    #[error("{0}")]
    Malformed(String),
    #[error(transparent)]
    Registry(#[from] RegistryBuildError),
}

use crate::bidder_registry::live_name_slice;

/// TCF enforcement-config key ranges. Purposes are the 10 standard
/// consent purposes Prebid gates; special features are the 12-slot wire
/// bitfield (GVL special-feature ids `1..=12`). Key `0` is never valid.
/// These MUST match the build-time constants in `auctioneer_config_schema::codegen`.
const PURPOSE_ID_RANGE: RangeInclusive<u8> = 1..=10;
const SPECIAL_FEATURE_ID_RANGE: RangeInclusive<u8> = 1..=12;

#[derive(Debug, Default, Deserialize, Serialize, edgezero_core::AppConfig)]
#[serde(deny_unknown_fields)]
pub struct AuctioneerAppConfig {
    /// Auction-tuning overrides. Absent fields fall through to
    /// `[baked.host]` in `auctioneer.toml` at build time.
    #[serde(default)]
    pub auction: AuctionConfig,

    /// Targeting-key overrides. Absent fields fall through to
    /// `[baked.account]` in `auctioneer.toml`.
    #[serde(default)]
    pub targeting: TargetingConfig,

    /// `/status` handler overrides.
    #[serde(default)]
    pub status: StatusConfig,

    /// Feature kill-switches. Absent → conservative defaults.
    #[serde(default)]
    pub features: FeatureFlags,

    /// Per-observable-bidder tunables, including request and host aliases.
    /// Alias fields fall back independently to their base bidder.
    /// Validated manually so rejected maps are not copied into
    /// validation-error parameters by the custom-field derive.
    #[serde(default)]
    pub bidders: BTreeMap<String, BidderOverrides>,

    /// Operator-owned floor-only rate snapshot, borrowed by the auction caller.
    /// The existing extracted-config byte/node ownership and caps still apply.
    /// Absence permits USD identity only; freshness uses the handler's instant.
    #[serde(default)]
    pub floor_currency: Option<FloorCurrencyConfig>,

    /// Per-account overrides keyed by publisher id (or `parentAccount`).
    /// Absent → every request resolves to the baked default account.
    /// Values are the same overlay-over-baked shape as the top-level
    /// `TargetingConfig` / `AuctionConfig`: every field is `Option<_>`
    /// and only present fields override the baked default.
    #[serde(default)]
    pub accounts: BTreeMap<String, AccountOverride>,

    /// Host GDPR overlay. Absent → the baked `HostConfig.gdpr` applies
    /// unchanged. Present → per-field merge onto the baked default per
    /// the same `Option::Some`/`None` overlay pattern the account map
    /// uses. Deployers who want to turn on TCF2 enforcement, add
    /// non-standard publishers, or change per-purpose enforcement config
    /// edit this section.
    #[serde(default)]
    pub gdpr: Option<HostGdprOverride>,

    #[serde(default)]
    pub ccpa: Option<HostEnforcementOverride>,

    #[serde(default)]
    pub lmt: Option<HostEnforcementOverride>,

    /// Host-declared aliases. Root-level, NOT under `gdpr` — aliases are
    /// routing/config, not privacy. Overlays onto the baked
    /// `HostConfig.aliases` map. Alias values are base bidder names.
    ///
    /// Bounded in entry count by [`MAX_ALIASES`] and in alias-name
    /// length by [`MAX_ALIAS_NAME_BYTES`]; see
    /// [`validate_host_alias_overlay`] for why both bounds exist and
    /// why crossing either is a refusal rather than a truncation.
    #[serde(default)]
    pub aliases: Option<BTreeMap<String, String>>,

    #[serde(default)]
    pub host_cookie: Option<HostCookieOverride>,

    #[serde(default)]
    pub user_sync: Option<UserSyncOverride>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct HostCookieOverride {
    #[serde(default)]
    pub cookie_name: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub ttl_days: Option<i64>,
    #[serde(default)]
    pub max_cookie_size_bytes: Option<usize>,
    #[serde(default)]
    pub optout_cookie: Option<OptoutCookieOverride>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct OptoutCookieOverride {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct UserSyncOverride {
    #[serde(default)]
    pub external_url: Option<String>,
    #[serde(default)]
    pub redirect_url: Option<String>,
    #[serde(default)]
    pub cooperative: Option<CooperativeOverride>,
    #[serde(default)]
    pub priority_groups: Option<Vec<Vec<String>>>,
    #[serde(default)]
    pub allowed_origins: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct CooperativeOverride {
    #[serde(default)]
    pub enabled_by_default: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AccountCookieSyncOverride {
    #[serde(default)]
    pub default_limit: Option<i64>,
    #[serde(default)]
    pub max_limit: Option<i64>,
    #[serde(default)]
    pub default_coop_sync: Option<bool>,
    #[serde(default)]
    pub priority_groups: Option<Vec<Vec<String>>>,
}

impl AuctioneerAppConfig {
    /// Shared push/load validation against this build's baked defaults.
    ///
    /// # Errors
    /// Returns a malformed-config error for unsafe effective configuration,
    /// or a distinct fatal error if this build's registry cannot initialize.
    pub fn validate_effective_defaults(&self) -> Result<(), EffectiveConfigError> {
        self.validate_effective(
            auctioneer_config::host_default(),
            auctioneer_config::account_default(),
        )
    }

    /// Validate the merged values used by both push and runtime extraction.
    /// Account sync settings have no cross-field host dependency, so each
    /// account validates its own replacement list without cloning inherited
    /// account privacy maps or host priority groups.
    ///
    /// # Errors
    /// Returns a malformed-config error for an invalid overlay, host, account,
    /// or namespace; registry initialization failures remain distinct and fatal.
    pub fn validate_effective(
        &self,
        baked_host: &HostConfig,
        baked_account: &AccountConfig,
    ) -> Result<(), EffectiveConfigError> {
        self.validate_effective_with_catalog(baked_host, baked_account, live_name_slice()?)
            .map_err(EffectiveConfigError::Malformed)
    }

    /// Validate against the complete immutable catalog supplied by an embedder.
    /// The standard push/load path always supplies this build's live catalog.
    /// Names are borrowed; validation does not copy them or
    /// install a registry in a process-global cell.
    ///
    /// # Errors
    /// Returns the same effective-config and namespace errors as
    /// [`Self::validate_effective`], using the supplied catalog for each check.
    pub fn validate_effective_with_catalog(
        &self,
        baked_host: &HostConfig,
        baked_account: &AccountConfig,
        compiled_names: &[BidderName],
    ) -> Result<(), String> {
        self.validate().map_err(|err| err.to_string())?;
        let host = resolve::resolve_host(self, baked_host);
        validate_host_cookie(&host.host_cookie)?;
        validate_user_sync(&host.user_sync, cfg!(feature = "insecure-bidders"))?;
        validate_catalog_size(compiled_names.len(), host.aliases.len())?;
        validate_account_cookie_sync(&baked_account.cookie_sync)?;
        for (id, account) in &self.accounts {
            if let Some(sync) = &account.cookie_sync {
                let replacement = AccountCookieSyncConfig {
                    default_limit: sync.default_limit,
                    max_limit: sync.max_limit,
                    default_coop_sync: sync.default_coop_sync,
                    priority_groups: sync.priority_groups.clone(),
                };
                validate_account_cookie_sync(&replacement)
                    .map_err(|err| format!("accounts.{id}.cookie_sync: {err}"))?;
            }
        }
        let mut folded_aliases = BTreeSet::new();
        for (alias, base) in &host.aliases {
            if alias.is_empty() {
                return Err("host alias name must be nonempty".to_owned());
            }
            if alias.len() > 128 {
                return Err("host alias name exceeds 128 UTF-8 bytes".to_owned());
            }
            if !folded_aliases.insert(alias.to_ascii_lowercase()) {
                return Err(format!("alias namespace collides at {alias:?}"));
            }
            if !compiled_names.iter().any(|name| name.as_str() == base) {
                return Err(format!(
                    "host alias {alias:?} references unknown base {base:?}"
                ));
            }
            if compiled_names
                .iter()
                .any(|name| name.as_str().eq_ignore_ascii_case(alias) && name.as_str() != alias)
            {
                return Err(format!(
                    "alias {alias:?} shadows a differently cased base bidder"
                ));
            }
        }
        let mut folded_bidders = BTreeSet::new();
        for name in self.bidders.keys() {
            let folded = name.to_ascii_lowercase();
            if !folded_bidders.insert(folded.clone()) {
                return Err(format!("bidder override namespace collides at {name:?}"));
            }
            if folded_aliases.contains(&folded) && !host.aliases.contains_key(name) {
                return Err(format!(
                    "bidder override {name:?} shadows a differently cased alias"
                ));
            }
            if compiled_names
                .iter()
                .any(|base| base.as_str().eq_ignore_ascii_case(name) && base.as_str() != name)
            {
                return Err(format!(
                    "bidder override {name:?} shadows a differently cased base bidder"
                ));
            }
        }
        Ok(())
    }
}

// The custom-field derive serializes the whole rejected field into `params.value`.
// Add root map errors directly, preserving field keys and nested validation shapes.
impl Validate for AuctioneerAppConfig {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();
        errors.merge_self("auction", self.auction.validate());
        errors.merge_self("targeting", self.targeting.validate());
        errors.merge_self("status", self.status.validate());

        if let Err(err) = validate_bidder_map(&self.bidders) {
            errors.add("bidders", err);
        }
        if let Some(snapshot) = &self.floor_currency
            && let Err(err) = floor_currency::validate(snapshot)
        {
            errors.add("floor_currency", err);
        }
        if let Err(err) = validate_account_map(&self.accounts) {
            errors.add("accounts", err);
        }
        if let Some(gdpr) = &self.gdpr {
            errors.merge_self("gdpr", gdpr.validate());
        }
        if let Some(host_cookie) = &self.host_cookie {
            errors.merge_self("host_cookie", host_cookie.validate());
        }
        if let Some(user_sync) = &self.user_sync {
            errors.merge_self("user_sync", user_sync.validate());
        }
        if let Some(aliases) = &self.aliases
            && let Err(err) = validate_host_alias_overlay(aliases)
        {
            errors.add("aliases", err);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl validator::ValidateArgs<'_> for AuctioneerAppConfig {
    type Args = ();

    fn validate_with_args(&self, (): Self::Args) -> Result<(), ValidationErrors> {
        self.validate()
    }
}

/// Entries the host `[aliases]` overlay may declare.
///
/// `/info/bidders/all` aggregates every registry bidder AND every
/// runtime alias, and the boot envelope reserves that route as
/// `(registry_bidder_count + max_aliases) x per_bidder_detail_bytes`.
/// The first factor is compile-time known; this bounds the second, so
/// the product is a number the envelope can fund rather than a
/// function of whatever the config store last accepted.
pub const MAX_ALIASES: usize = 64;

/// Bytes one alias NAME may occupy.
///
/// The same aux-route reservation needs `per_bidder_detail_bytes` to be
/// a constant, and an alias's own name is part of the detail the route
/// renders for it. Unbounded names would make that term config-derived
/// too, so bounding the entry count alone would not bound the route.
///
/// BYTES, not characters: alias names are arbitrary UTF-8 and the
/// reservation funds bytes. A 33-character name of four-byte characters
/// is 132 bytes, so a character count would admit more than twice the
/// funded length.
pub const MAX_ALIAS_NAME_BYTES: usize = MAX_BIDDER_NAME_BYTES;

/// Reject a host `[aliases]` overlay past either bound.
///
/// FAIL-LOUD, not truncating: the overlay is the operator's own pushed
/// map, and serving a silently shortened one would mean an auction
/// routes on an alias set nobody wrote. A refusal here surfaces as the
/// malformed-config class every other invalid host shape produces.
///
/// This entry-count limit is for the HOST overlay only. Request-level
/// `ext.prebid.aliases` is a separate surface, but shares the byte limit
/// for names and bases with the exchange resolver.
///
/// Complements rather than replaces the registry check (`build.rs`'s
/// `validate_host_aliases_against_registry` for the baked map,
/// `config::runtime::validate_alias_bases` for a pushed overlay):
/// that one asks whether each entry's BASE resolves, which says nothing
/// about how many entries there are or how long their names run.
///
/// Length is compared as `str::len()`, which is the byte length. No
/// byte-index slicing of the name, so a bound landing mid-character
/// cannot panic.
fn validate_host_alias_overlay(
    aliases: &BTreeMap<String, String>,
) -> Result<(), validator::ValidationError> {
    if aliases.len() > MAX_ALIASES {
        let mut ve = validator::ValidationError::new("host_alias_count_over_limit");
        ve.message = Some(
            format!(
                "[aliases]: {count} entries exceeds the {MAX_ALIASES}-entry limit \
                 (max_aliases)",
                count = aliases.len(),
            )
            .into(),
        );
        return Err(ve);
    }
    for (name, base) in aliases {
        if name.len() > MAX_ALIAS_NAME_BYTES {
            let mut ve = validator::ValidationError::new("host_alias_name_too_long");
            ve.message = Some(
                format!(
                    "[aliases]: an alias name is {len} bytes, over the \
                     {MAX_ALIAS_NAME_BYTES}-byte limit (max_alias_name_bytes)",
                    len = name.len(),
                )
                .into(),
            );
            return Err(ve);
        }
        if base.len() > MAX_BIDDER_NAME_BYTES {
            let mut ve = validator::ValidationError::new("host_alias_base_too_long");
            ve.message = Some(
                format!(
                    "[aliases]: a base name is {len} bytes, over the \
                     {MAX_BIDDER_NAME_BYTES}-byte bidder identity limit",
                    len = base.len(),
                )
                .into(),
            );
            return Err(ve);
        }
    }
    Ok(())
}

fn validate_account_map(
    map: &BTreeMap<String, AccountOverride>,
) -> Result<(), validator::ValidationError> {
    for (id, entry) in map {
        // Baked rules are build-validated; replacement arrays are validated
        // below through AccountPrivacyOverride, without rescanning inherited rules.
        if let Err(err) = entry.validate() {
            let mut ve = validator::ValidationError::new("account_override_invalid");
            ve.message = Some(format!("[accounts.{id}]: {err}").into());
            return Err(ve);
        }
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostEnforcementOverride {
    #[serde(default)]
    pub enforce: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AccountCcpaOverride {
    #[serde(default)]
    pub enabled: Option<TriState>,
    #[serde(default)]
    pub channel_enabled: Option<ChannelEnabledOverride>,
}

fn validate_activity_map(map: &ActivityOverrides) -> Result<(), validator::ValidationError> {
    validate_activity_overrides(map).map_err(validator::ValidationError::new)
}

/// Deserialize a `BTreeMap<u8, V>` from STRING keys.
///
/// The deploy format is TOML, whose table keys are always strings, and
/// `toml` — unlike `serde_json` — hands the key to serde as a `str`
/// rather than re-parsing it for an integer key type. Without this the
/// canonical `[gdpr.tcf2.purposes.2]` block fails with "invalid type:
/// string \"2\", expected u8" before any validator runs. Accepting the
/// string form also keeps the JSON blob path working, since JSON object
/// keys are strings too.
///
/// A key outside `u8` is rejected here; a key inside `u8` but outside the
/// legal id range is left to the range validators, so the operator gets
/// the specific "purpose id out of range" message.
fn deserialize_u8_keyed_map<'de, D, V>(deserializer: D) -> Result<Option<BTreeMap<u8, V>>, D::Error>
where
    D: serde::Deserializer<'de>,
    V: Deserialize<'de>,
{
    use serde::de::Error as _;

    let Some(raw) = Option::<BTreeMap<String, V>>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let mut parsed: BTreeMap<u8, V> = BTreeMap::new();
    for (key, value) in raw {
        let id: u8 = key.parse().map_err(|err: ParseIntError| {
            D::Error::custom(format!("enforcement map key `{key}` is not an id: {err}"))
        })?;
        parsed.insert(id, value);
    }
    Ok(Some(parsed))
}

fn validate_default_value(value: &str) -> Result<(), validator::ValidationError> {
    match value {
        "0" | "1" => Ok(()),
        _ => Err(validator::ValidationError::new(
            "gdpr_default_value_must_be_0_or_1",
        )),
    }
}

// Generic over the map VALUE type so one function serves both the host
// (`HostPurposeEnforcementOverride`) and account
// (`PurposeEnforcementOverride`) maps — the KEYS are `u8` in both. The
// `validator` crate does not inspect `BTreeMap` keys, so a custom
// validator does it (skipped when the field is `None`).
fn validate_purpose_id_keys<V>(map: &BTreeMap<u8, V>) -> Result<(), validator::ValidationError> {
    if map.keys().any(|id| !PURPOSE_ID_RANGE.contains(id)) {
        return Err(validator::ValidationError::new("purpose_id_out_of_range"));
    }
    Ok(())
}

fn validate_special_feature_id_keys<V>(
    map: &BTreeMap<u8, V>,
) -> Result<(), validator::ValidationError> {
    if map.keys().any(|id| !SPECIAL_FEATURE_ID_RANGE.contains(id)) {
        return Err(validator::ValidationError::new(
            "special_feature_id_out_of_range",
        ));
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct HostGdprOverride {
    /// GDPR master switch. Overlays onto the baked
    /// `HostConfig.gdpr.enabled`.
    #[serde(default)]
    pub enabled: Option<bool>,

    /// GVL vendor id assigned to the host as itself a TCF vendor. `0` is
    /// valid — it is the "no host vendor id declared" sentinel and the
    /// default; `65535` is the `u16` max the GVL keys use.
    #[serde(default)]
    #[validate(range(min = 0_u32, max = 0xFFFF_u32))]
    pub host_vendor_id: Option<u32>,

    #[serde(default)]
    pub non_standard_publishers: Option<Vec<String>>,

    /// The string value the resolver uses when the geo signal is
    /// ambiguous and neither `user.geo` nor `device.geo` resolves. Must
    /// be exactly `"0"` or `"1"` (the `OpenRTB` `regs.gdpr` wire values).
    #[serde(default)]
    #[validate(custom(function = "validate_default_value"))]
    pub default_value: Option<String>,

    #[serde(default)]
    #[validate(nested)]
    pub tcf2: Option<HostTcf2Override>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct HostTcf2Override {
    #[serde(default)]
    pub enabled: Option<bool>,

    #[serde(default)]
    pub purpose_one_treatment: Option<PurposeOneTreatmentOverride>,

    /// Per-purpose enforcement overrides, keys `1..=10`.
    #[serde(default, deserialize_with = "deserialize_u8_keyed_map")]
    #[validate(custom(function = "validate_purpose_id_keys"))]
    pub purposes: Option<BTreeMap<u8, HostPurposeEnforcementOverride>>,

    /// Per-special-feature enforcement overrides, keys `1..=12`.
    #[serde(default, deserialize_with = "deserialize_u8_keyed_map")]
    #[validate(custom(function = "validate_special_feature_id_keys"))]
    pub special_features: Option<BTreeMap<u8, SpecialFeatureEnforcementOverride>>,
}

/// Host-side per-purpose override. Field-identical to the account
/// `PurposeEnforcementOverride`; the distinct type is retained for
/// structural stability and may be collapsed in a later cleanup.
#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct HostPurposeEnforcementOverride {
    #[serde(default)]
    pub enforce_algo: Option<EnforceAlgo>,
    #[serde(default)]
    pub enforce_purpose: Option<TriState>,
    #[serde(default)]
    pub enforce_vendors: Option<TriState>,
    #[serde(default)]
    pub vendor_exceptions: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AccountOverride {
    /// Account-scoped on-wire response cap; overrides the host default.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0640_0000_u64))]
    pub max_transport_bytes: Option<u64>,

    /// Account-scoped post-decode response cap, independent of transport.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0640_0000_u64))]
    pub max_decoded_bytes: Option<u64>,

    #[serde(default)]
    pub disabled: Option<bool>,

    #[serde(default)]
    pub debug_allow: Option<bool>,

    #[serde(default)]
    #[validate(nested)]
    pub cookie_sync: Option<AccountCookieSyncOverride>,

    #[serde(default)]
    #[validate(length(min = 1_u64, max = 32_u64))]
    pub bid_rounding: Option<String>,

    #[serde(default)]
    #[validate(length(min = 1_u64, max = 8_u64))]
    pub targeting_prefix: Option<String>,

    /// Mirrors the top-level `TargetingConfig` range so the same
    /// validator rejects the same values regardless of which section a
    /// deployer sets. Bounds are structural: shorter than 12 breaks the
    /// `hb_<attr>_<bidder>` key-length invariant even for short bidder
    /// names; longer than 20 is the `OpenRTB` targeting-macro cap.
    #[serde(default)]
    #[validate(range(min = 12_usize, max = 20_usize))]
    pub truncate_target_attribute: Option<usize>,

    #[serde(default)]
    #[validate(nested)]
    pub gdpr: Option<AccountGdprOverride>,

    #[serde(default)]
    #[validate(nested)]
    pub ccpa: Option<AccountCcpaOverride>,

    /// IP-anonymisation override. Each keep-bits is `Option` so an
    /// overlay can set one family without disturbing the other; the
    /// merge applies present values over the baked
    /// `AccountPrivacyConfig`.
    #[serde(default)]
    #[validate(nested)]
    pub privacy: Option<AccountPrivacyOverride>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AccountPrivacyOverride {
    // v4 and v6 use DISTINCT override types — a shared 32-capped struct
    // would reject a legitimate v6 keep-bits in 33..=128.
    #[serde(default)]
    #[validate(nested)]
    pub ipv4: Option<IpAnonOverrideV4>,
    #[serde(default)]
    #[validate(nested)]
    pub ipv6: Option<IpAnonOverrideV6>,
    #[serde(default)]
    #[validate(custom(function = "validate_activity_map"))]
    pub allowactivities: Option<ActivityOverrides>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct IpAnonOverrideV4 {
    // Range guarded here: the OVERLAY crate is `auctioneer-core`, which DOES
    // depend on `validator` — unlike `auctioneer-config`.
    #[serde(default)]
    #[validate(range(min = 0_u8, max = 32_u8))]
    pub anon_keep_bits: Option<u8>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct IpAnonOverrideV6 {
    #[serde(default)]
    #[validate(range(min = 0_u8, max = 128_u8))]
    pub anon_keep_bits: Option<u8>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AccountGdprOverride {
    /// `TriState::Unset` / `Enabled` / `Disabled` — the account-side
    /// master switch. `Unset` falls through to `host.tcf2.enabled`.
    #[serde(default)]
    pub enabled: Option<TriState>,

    #[serde(default)]
    pub purpose_one_treatment: Option<PurposeOneTreatmentOverride>,

    /// Per-channel enablement fallback. `Unset` entries fall through to
    /// the account-level `enabled`, then to `host.tcf2.enabled`.
    #[serde(default)]
    pub channel_enabled: Option<ChannelEnabledOverride>,

    /// Per-purpose enforcement overrides. Keys `1..=10`. `None` =
    /// "no per-purpose overrides declared in this account TOML — inherit
    /// the baked account default map"; `Some(empty map)` = "declared,
    /// but no purposes overridden"; a present key overlays ONLY the
    /// fields set on `PurposeEnforcementOverride` onto the baked account
    /// entry (or `PurposeEnforcement::default()` when the baked account
    /// has no entry for that id). Runtime host fall-through is a
    /// SEPARATE step in the privacy resolver, not a boot-time overlay.
    #[serde(default, deserialize_with = "deserialize_u8_keyed_map")]
    #[validate(custom(function = "validate_purpose_id_keys"))]
    pub purposes: Option<BTreeMap<u8, PurposeEnforcementOverride>>,

    /// Per-special-feature enforcement overrides. Same two-layer
    /// semantics as `purposes`.
    #[serde(default, deserialize_with = "deserialize_u8_keyed_map")]
    #[validate(custom(function = "validate_special_feature_id_keys"))]
    pub special_features: Option<BTreeMap<u8, SpecialFeatureEnforcementOverride>>,

    /// Vendors that use A.3 basic enforcement instead of A.2 full
    /// enforcement. Bidder / core names (not GVL ids). Account-only —
    /// there is no host-level `basic_enforcement_vendors`. `None` =
    /// inherit the baked account default; `Some(list)` = replace the
    /// baked account list wholesale (whole-array replace matches
    /// RFC 7396). There is no host-side merge for this field.
    #[serde(default)]
    pub basic_enforcement_vendors: Option<Vec<String>>,
}

/// Per-field boot-time overlay for a single purpose. Each `None` field
/// leaves the corresponding baked account default field untouched
/// (usually `TriState::Unset` for tri-state fields, meaning "no account
/// opinion, fall through to host at runtime"); each `Some(_)` field
/// replaces it. `vendor_exceptions` is `Option<Vec<String>>` because a
/// whole-list replace matches RFC 7396's whole-array semantics; there is
/// no per-entry exception merge. Runtime host fall-through happens
/// separately in the privacy resolver, not here.
#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct PurposeEnforcementOverride {
    #[serde(default)]
    pub enforce_algo: Option<EnforceAlgo>,
    #[serde(default)]
    pub enforce_purpose: Option<TriState>,
    #[serde(default)]
    pub enforce_vendors: Option<TriState>,
    #[serde(default)]
    pub vendor_exceptions: Option<Vec<String>>,
}

/// Per-field override for a single special feature. Same shape pattern
/// as `PurposeEnforcementOverride`.
#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct SpecialFeatureEnforcementOverride {
    #[serde(default)]
    pub enforce: Option<TriState>,
    #[serde(default)]
    pub vendor_exceptions: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct ChannelEnabledOverride {
    #[serde(default)]
    pub web: Option<TriState>,
    #[serde(default)]
    pub app: Option<TriState>,
    #[serde(default)]
    pub video: Option<TriState>,
    #[serde(default)]
    pub amp: Option<TriState>,
    #[serde(default)]
    pub dooh: Option<TriState>,
}

fn validate_bidder_map(
    map: &BTreeMap<String, BidderOverrides>,
) -> Result<(), validator::ValidationError> {
    // Bound every key before value validation can include any name in a diagnostic.
    if map.keys().any(|name| name.len() > MAX_BIDDER_NAME_BYTES) {
        let mut ve = validator::ValidationError::new("bidder_name_too_long");
        ve.message = Some(
            format!("[bidders]: a bidder name exceeds the {MAX_BIDDER_NAME_BYTES}-byte limit")
                .into(),
        );
        return Err(ve);
    }
    for (name, entry) in map {
        if let Some(settings) = &entry.settings {
            ensure_settings_limits(settings)
                .map_err(|_reason| validator::ValidationError::new("bidder_settings_limits"))?;
        }
        if let Err(err) = entry.validate() {
            let mut ve = validator::ValidationError::new("bidder_overrides_invalid");
            ve.message = Some(format!("[bidders.{name}]: {err}").into());
            return Err(ve);
        }
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AuctionConfig {
    /// Runtime override for `HostConfig.resource_limits.default_tmax_ms`.
    /// Deployers tune the auction budget without a rebuild. Range
    /// enforces the `> 0` rule.
    #[serde(default)]
    #[validate(range(min = 1_i64, max = 30_000_i64))]
    pub default_tmax_ms: Option<i64>,

    /// Runtime override for `HostConfig.resource_limits.max_tmax_ms`.
    #[serde(default)]
    #[validate(range(min = 1_i64, max = 30_000_i64))]
    pub max_tmax_ms: Option<i64>,

    /// Runtime override for `HostConfig.resource_limits.max_bidders_per_auction`.
    #[serde(default)]
    #[validate(range(min = 1_usize, max = 500_usize))]
    pub max_bidders_per_auction: Option<usize>,

    /// Runtime override for `HostConfig.resource_limits.max_outbound_requests`.
    #[serde(default)]
    #[validate(range(min = 1_usize, max = 1_000_usize))]
    pub max_outbound_requests: Option<usize>,

    /// Runtime override for `HostConfig.resource_limits.max_bidder_response_bytes`.
    /// Deployers cap bidder response size below the build default when
    /// running under a tight memory budget.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_usize, max = 0x0100_0000_usize))]
    pub max_bidder_response_bytes: Option<usize>,

    /// Runtime override for `HostConfig.resource_limits.max_outbound_raw_bytes`.
    /// Clamped to the envelope's funded figure the same way the other
    /// envelope-funded byte caps are: this lowers the per-call charge,
    /// it cannot raise it.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0100_0000_u64))]
    pub max_outbound_raw_bytes: Option<u64>,

    /// Runtime override for
    /// `HostConfig.resource_limits.max_outbound_request_bytes`.
    /// Clamped to the envelope's funded figure the same way.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0100_0000_u64))]
    pub max_outbound_request_bytes: Option<u64>,

    /// Runtime override for `HostConfig.resource_limits.max_transport_bytes`.
    /// The resolver clamps it to what the boot envelope funds, so this
    /// range only bounds what is syntactically a cap at all — the
    /// envelope, not this attribute, is what stops a value from
    /// authorising memory nothing budgeted for.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0640_0000_u64))]
    pub max_transport_bytes: Option<u64>,

    /// Runtime override for `HostConfig.resource_limits.max_decoded_bytes`.
    /// Clamped to the envelope's funded figure the same way.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0640_0000_u64))]
    pub max_decoded_bytes: Option<u64>,

    /// Runtime override for `HostConfig.resource_limits.max_parsed_nodes`.
    /// Clamped to the envelope's funded figure the same way.
    #[serde(default)]
    #[validate(range(min = 1_u32, max = 0x0010_0000_u32))]
    pub max_parsed_nodes: Option<u32>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct TargetingConfig {
    /// Runtime override for `AccountConfig.targeting_prefix`.
    /// Length capped so the resolved key length invariant
    /// still leaves room for the `_pb_<bidder>` suffix on legit bidders.
    #[serde(default)]
    #[validate(length(min = 1_u64, max = 8_u64))]
    pub prefix: Option<String>,

    /// Runtime override for `AccountConfig.truncate_target_attribute`.
    /// `None` → default (`MaxKeyLength = 20`).
    #[serde(default)]
    #[validate(range(min = 12_usize, max = 20_usize))]
    pub truncate_target_attribute: Option<usize>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct StatusConfig {
    /// Overrides `HostConfig.status_response`. Empty string → 204 No
    /// Content. Deployers set an env-name / banner here for prod
    /// probes.
    #[serde(default)]
    #[validate(custom(function = "validate_status_banner"))]
    pub banner: Option<String>,
}

fn validate_status_banner(banner: &str) -> Result<(), validator::ValidationError> {
    if banner.len() > MAX_STATUS_BANNER_BYTES {
        return Err(validator::ValidationError::new("status_banner_bytes"));
    }
    Ok(())
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureFlags {
    /// Kill-switch for outbound `Content-Encoding: gzip` when a
    /// bidder declares `endpoint_compression: gzip`. `None` /
    /// `Some(true)` → gzip; `Some(false)` → send plaintext (useful
    /// when a downstream proxy strips gzip incorrectly).
    #[serde(default)]
    pub gzip_outbound: Option<bool>,

    /// Default `returnallbidstatus` when the request omits
    /// `ext.prebid.returnallbidstatus`. Defaults to `false`;
    /// deployers can flip it on for auditability.
    #[serde(default)]
    pub returnallbidstatus_default: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Serialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct BidderOverrides {
    /// Implementation-tagged, bidder-owned host protocol settings. This does
    /// not activate an implementation or authorize an endpoint.
    #[serde(default)]
    pub settings: Option<auctioneer_config::BidderSettings>,

    /// Observable-name response cap. An alias falls back per field to
    /// its base bidder, then the selected account and host defaults.
    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0640_0000_u64))]
    pub max_transport_bytes: Option<u64>,

    #[serde(default)]
    #[validate(range(min = 0x0000_0400_u64, max = 0x0640_0000_u64))]
    pub max_decoded_bytes: Option<u64>,

    /// Kill-switch for this bidder. `None` / `Some(true)` → bidder
    /// participates.
    ///
    /// `Some(false)` drops the bidder before fan-out: the auction
    /// handler folds every disabled name into the request-scoped
    /// disabled set on `AuctionPlan`, and the splitter — plus the
    /// alias resolver, for a request-declared alias whose base or own
    /// name is disabled — removes the bidder and emits a code-6
    /// `BidderTemporarilyDisabled` warning under its observable name
    /// rather than silently omitting it.
    #[serde(default)]
    pub enabled: Option<bool>,

    #[serde(default)]
    pub usersync_enabled: Option<bool>,

    /// Runtime override for `BidderInfo.bidder_timeout_ms`. Absent →
    /// falls back to build-time TOML value; if that is also absent,
    /// the exchange uses the auction-wide `bidder_visible_tmax_ms`.
    #[serde(default)]
    #[validate(range(min = 1_u32, max = 30_000_u32))]
    pub timeout_ms: Option<u32>,

    /// 0.0..=1.0 traffic sample rate. `None` / `Some(1.0)` → include
    /// the bidder on every eligible imp; `Some(0.0)` → drop the
    /// bidder silently before dispatch (no `ext.warnings` entry, no
    /// `seatnonbid` row; the sentinel bundle keeps the "no live
    /// bidders" branch reachable so `nbr = NoBidInvalidRequest`
    /// still fires per Appendix E.5). To hard-disable a bidder
    /// noisily, use `enabled = false` instead.
    #[serde(default)]
    #[validate(custom(function = "validate_sample_rate"))]
    pub sample_rate: Option<f64>,
}

/// `validator` 0.20's built-in `range` check accepts `NaN` — both
/// `NaN < min` and `NaN > max` are false — and TOML's `nan` / `inf`
/// literals can reach this field directly from `auctioneer.toml`. A custom
/// validator that requires finiteness first closes the hole `range`
/// leaves open.
fn validate_sample_rate(v: f64) -> Result<(), validator::ValidationError> {
    if v.is_finite() && (0.0_f64..=1.0_f64).contains(&v) {
        Ok(())
    } else {
        Err(validator::ValidationError::new(
            "sample_rate_must_be_finite_and_in_0_1",
        ))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn response_caps_accept_boundaries_in_every_runtime_scope() {
        for section in [
            "auction",
            "accounts.publisher",
            "bidders.fake_aaa",
            "bidders.fake_alias",
        ] {
            for cap in [1_024_u64, 80 * 1024 * 1024, 100 * 1024 * 1024] {
                let cfg: AuctioneerAppConfig = toml::from_str(&format!(
                    "[{section}]\nmax_transport_bytes = {cap}\nmax_decoded_bytes = {cap}\n"
                ))
                .expect("each scope must accept independent response caps");
                cfg.validate()
                    .expect("the inclusive cap boundaries are valid");
            }
        }
    }

    #[test]
    fn response_caps_reject_values_outside_the_interval_in_every_runtime_scope() {
        for section in ["auction", "accounts.publisher", "bidders.fake_aaa"] {
            for field in ["max_transport_bytes", "max_decoded_bytes"] {
                for cap in [0_u64, 1_023, 100 * 1024 * 1024 + 1] {
                    let cfg: AuctioneerAppConfig =
                        toml::from_str(&format!("[{section}]\n{field} = {cap}\n"))
                            .expect("validation, not unknown-field rejection, must reject the cap");
                    assert!(cfg.validate().is_err(), "{section}.{field} accepted {cap}");
                }
            }
        }
    }

    #[test]
    fn host_alias_bases_are_bounded_before_registry_diagnostics() {
        let cfg = AuctioneerAppConfig {
            aliases: Some(BTreeMap::from([(
                "fake_short_alias".to_owned(),
                "x".repeat(0x0001_0000),
            )])),
            ..AuctioneerAppConfig::default()
        };
        let error = cfg.validate().expect_err("an oversized base is invalid");
        let diagnostic = error.to_string();
        assert!(diagnostic.len() < 512);
        assert!(!diagnostic.contains(&"x".repeat(128)));
    }

    #[test]
    fn status_banner_is_limited_by_utf8_bytes_not_characters() {
        let mut cfg = super::AuctioneerAppConfig::default();
        cfg.status.banner = Some("\u{1f600}".repeat(2_049));
        assert!(
            cfg.validate().is_err(),
            "8,196 bytes must not pass an 8,192-byte limit"
        );
        cfg.status.banner = Some("\u{1f600}".repeat(2_048));
        cfg.validate().expect("the exact byte boundary is valid");
    }

    use super::{AuctioneerAppConfig, BidderOverrides, MAX_BIDDER_NAME_BYTES};
    use auctioneer_bidder::BidderName;
    use auctioneer_config::types::{AccountConfig, HostConfig};
    use std::collections::BTreeMap;
    use validator::Validate as _;

    fn config_with_bidder_key(name: &str, fields: &str) -> AuctioneerAppConfig {
        toml::from_str(&format!("[bidders.{name:?}]\n{fields}\n"))
            .expect("quoted bidder keys parse from TOML")
    }

    fn assert_bidder_key_error(cfg: &AuctioneerAppConfig, name: &str) {
        let error = cfg
            .validate()
            .expect_err("oversized bidder keys must be rejected");
        let fields = error.field_errors();
        assert_eq!(fields["bidders"][0].code, "bidder_name_too_long");
        let diagnostic = error.to_string();
        assert!(diagnostic.len() < 256, "key errors must remain bounded");
        assert!(
            !diagnostic.contains(name),
            "key errors must not echo the supplied name"
        );
        assert!(
            !diagnostic.contains("max_transport_bytes"),
            "key errors must precede value errors"
        );
    }

    #[test]
    fn bidder_override_ascii_key_at_64_bytes_is_valid_even_when_disabled() {
        assert_eq!(MAX_BIDDER_NAME_BYTES, 64);
        let name = format!("fake_{}", "a".repeat(MAX_BIDDER_NAME_BYTES - 5));
        for fields in ["enabled = true", "enabled = false"] {
            config_with_bidder_key(&name, fields)
                .validate()
                .expect("64-byte bidder keys are valid");
        }
    }

    #[test]
    fn bidder_override_ascii_key_at_65_bytes_is_rejected_even_when_disabled() {
        let name = "a".repeat(MAX_BIDDER_NAME_BYTES + 1);
        for fields in ["enabled = true", "enabled = false"] {
            assert_bidder_key_error(&config_with_bidder_key(&name, fields), &name);
        }
    }

    #[test]
    fn bidder_override_utf8_key_limit_uses_bytes_at_the_character_boundary() {
        let boundary = format!("fake_a{}", "\u{e9}".repeat(29));
        assert_eq!(boundary.len(), 64);
        config_with_bidder_key(&boundary, "enabled = false")
            .validate()
            .expect("a 64-byte multibyte name is valid");

        let over = format!("{}\u{e9}", "a".repeat(63));
        assert_eq!(over.len(), 65);
        assert!(!over.is_char_boundary(MAX_BIDDER_NAME_BYTES));
        assert_bidder_key_error(&config_with_bidder_key(&over, "enabled = false"), &over);
    }

    #[test]
    fn bidder_override_oversized_key_error_precedes_its_invalid_value_without_echoing_key() {
        for length in [MAX_BIDDER_NAME_BYTES + 1, 0x0001_0000] {
            let name = "z".repeat(length);
            let cfg = config_with_bidder_key(&name, "enabled = false\nmax_transport_bytes = 0");
            assert_bidder_key_error(&cfg, &name);
        }
    }

    #[test]
    fn bidder_override_key_validation_precedes_invalid_values_in_other_entries() {
        let name = "z".repeat(MAX_BIDDER_NAME_BYTES + 1);
        let mut cfg = config_with_bidder_key(&name, "enabled = false");
        cfg.bidders.insert(
            "fake_aaa".to_owned(),
            BidderOverrides {
                max_transport_bytes: Some(0),
                ..BidderOverrides::default()
            },
        );
        assert_bidder_key_error(&cfg, &name);
    }

    #[test]
    fn bidder_override_bounded_key_still_reports_invalid_value() {
        let name = format!("fake_{}", "a".repeat(MAX_BIDDER_NAME_BYTES - 5));
        let error = config_with_bidder_key(&name, "max_transport_bytes = 0")
            .validate()
            .expect_err("bounded keys must still validate their values");
        let fields = error.field_errors();
        assert_eq!(fields["bidders"][0].code, "bidder_overrides_invalid");
        assert!(error.to_string().contains("max_transport_bytes"));
    }

    #[test]
    fn bidder_override_oversized_key_errors_retain_no_parameters() {
        for length in [MAX_BIDDER_NAME_BYTES + 1, 0x0001_0000] {
            for fields in [
                "enabled = false",
                "enabled = false\nmax_transport_bytes = 0",
            ] {
                let name = "z".repeat(length);
                let error = config_with_bidder_key(&name, fields)
                    .validate()
                    .expect_err("oversized bidder keys must be rejected");
                let field_errors = error.field_errors();
                assert_eq!(field_errors["bidders"][0].code, "bidder_name_too_long");
                assert!(
                    field_errors["bidders"][0].params.is_empty(),
                    "the derive must not retain the bidder map in validation-error parameters"
                );
            }
        }
    }

    #[test]
    fn bidder_override_oversized_key_serialized_errors_do_not_echo_or_copy_the_map() {
        for length in [MAX_BIDDER_NAME_BYTES + 1, 0x0001_0000] {
            for fields in [
                "enabled = false",
                "enabled = false\nmax_transport_bytes = 0",
            ] {
                let name = "z".repeat(length);
                let error = config_with_bidder_key(&name, fields)
                    .validate()
                    .expect_err("oversized bidder keys must be rejected");
                let value = serde_json::to_value(&error).expect("validation errors serialize");
                let serialized = value.to_string();
                assert!(
                    !serialized.contains(&name),
                    "serialized errors must not include the oversized bidder key"
                );
                assert_eq!(value["bidders"][0]["params"], serde_json::json!({}));
                assert!(
                    serialized.len() < 256,
                    "serialized key errors must stay bounded"
                );
            }
        }
    }

    #[test]
    fn bidder_override_bounded_invalid_map_errors_do_not_capture_valid_sibling_entries() {
        let mut cfg = config_with_bidder_key("fake_aaa", "max_transport_bytes = 0");
        for index in 0..64_usize {
            cfg.bidders.insert(
                format!("fake_unrelated_valid_bidder-{index}"),
                BidderOverrides {
                    enabled: Some(true),
                    timeout_ms: Some(100_u32),
                    ..BidderOverrides::default()
                },
            );
        }
        let error = cfg
            .validate()
            .expect_err("the invalid override is rejected");
        let field_errors = error.field_errors();
        assert_eq!(field_errors["bidders"][0].code, "bidder_overrides_invalid");
        assert!(
            field_errors["bidders"][0].params.is_empty(),
            "bounded invalid bidder maps must not be copied into error parameters either"
        );
        let value = serde_json::to_value(&error).expect("validation errors serialize");
        assert_eq!(value["bidders"][0]["params"], serde_json::json!({}));
        let serialized = value.to_string();
        assert!(!serialized.contains("fake_unrelated_valid_bidder"));
        assert!(
            serialized.len() < 512,
            "one invalid value cannot retain the entire map"
        );
    }

    #[test]
    fn root_account_map_errors_retain_no_map_parameters() {
        let cfg: AuctioneerAppConfig = toml::from_str(
            "[accounts.bad-account]\nmax_transport_bytes = 0\n\
             [accounts.untouched-publisher]\nmax_transport_bytes = 1024\n",
        )
        .expect("account maps deserialize before validation");
        let error = cfg.validate().expect_err("the invalid account is rejected");
        let field_errors = error.field_errors();
        assert_eq!(field_errors["accounts"][0].code, "account_override_invalid");
        assert!(
            field_errors["accounts"][0].params.is_empty(),
            "account validation errors must not retain the account map"
        );
        let value = serde_json::to_value(&error).expect("validation errors serialize");
        assert_eq!(value["accounts"][0]["params"], serde_json::json!({}));
        let serialized = value.to_string();
        assert!(!serialized.contains("untouched-publisher"));
        assert!(serialized.len() < 512);
    }

    #[test]
    fn root_alias_map_errors_retain_no_map_parameters() {
        for length in [super::MAX_ALIAS_NAME_BYTES + 1, 0x0001_0000] {
            let oversized = "z".repeat(length);
            for (name, base, code) in [
                (
                    oversized.clone(),
                    "fake_aaa".to_owned(),
                    "host_alias_name_too_long",
                ),
                (
                    "fake_short_alias".to_owned(),
                    oversized.clone(),
                    "host_alias_base_too_long",
                ),
            ] {
                let cfg = AuctioneerAppConfig {
                    aliases: Some(BTreeMap::from([
                        (name, base),
                        ("fake_untouched_alias".to_owned(), "fake_aaa".to_owned()),
                    ])),
                    ..AuctioneerAppConfig::default()
                };
                let error = cfg
                    .validate()
                    .expect_err("oversized alias identities are rejected");
                let field_errors = error.field_errors();
                assert_eq!(field_errors["aliases"][0].code, code);
                assert!(
                    field_errors["aliases"][0].params.is_empty(),
                    "alias validation errors must not retain the alias map"
                );
                let value = serde_json::to_value(&error).expect("validation errors serialize");
                assert_eq!(value["aliases"][0]["params"], serde_json::json!({}));
                let serialized = value.to_string();
                assert!(!serialized.contains(&oversized));
                assert!(!serialized.contains("fake_untouched_alias"));
                assert!(serialized.len() < 512);
            }
        }
    }

    #[test]
    fn root_validation_preserves_existing_error_fields_and_nested_shapes() {
        let mut cfg: AuctioneerAppConfig = toml::from_str(
            "[auction]\ndefault_tmax_ms = 0\n\
             [targeting]\nprefix = \"toolongprefix\"\n\
             [gdpr]\nhost_vendor_id = 65536\n\
             [bidders.fake_aaa]\nmax_transport_bytes = 0\n\
             [accounts.publisher]\nmax_transport_bytes = 0\n",
        )
        .expect("invalid values deserialize before validation");
        cfg.status.banner = Some("x".repeat(super::MAX_STATUS_BANNER_BYTES + 1));
        cfg.aliases = Some(BTreeMap::from([("a".repeat(65), "fake_aaa".to_owned())]));
        let error = cfg
            .validate()
            .expect_err("all invalid fields are collected");
        let value = serde_json::to_value(&error).expect("validation errors serialize");
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "accounts",
                "aliases",
                "auction",
                "bidders",
                "gdpr",
                "status",
                "targeting"
            ]
        );
        for (field, nested_error) in [
            ("auction", cfg.auction.validate().unwrap_err()),
            ("targeting", cfg.targeting.validate().unwrap_err()),
            ("status", cfg.status.validate().unwrap_err()),
            ("gdpr", cfg.gdpr.as_ref().unwrap().validate().unwrap_err()),
        ] {
            assert_eq!(value[field], serde_json::to_value(nested_error).unwrap());
        }
        let field_errors = error.field_errors();
        assert_eq!(field_errors["accounts"][0].code, "account_override_invalid");
        assert_eq!(field_errors["aliases"][0].code, "host_alias_name_too_long");
        assert_eq!(field_errors["bidders"][0].code, "bidder_overrides_invalid");
        let args_error = validator::ValidateArgs::validate_with_args(&cfg, ()).unwrap_err();
        assert_eq!(args_error.errors().len(), error.errors().len());
        for field in [
            "accounts",
            "aliases",
            "auction",
            "bidders",
            "gdpr",
            "status",
            "targeting",
        ] {
            assert!(args_error.errors().contains_key(field));
        }
        let args_field_errors = args_error.field_errors();
        for field in ["accounts", "aliases", "bidders"] {
            assert_eq!(
                args_field_errors[field][0].code,
                field_errors[field][0].code
            );
        }
    }

    #[test]
    fn empty_toml_deserializes_to_defaults() {
        let cfg: AuctioneerAppConfig = toml::from_str("").unwrap();
        assert!(cfg.auction.default_tmax_ms.is_none());
        assert!(cfg.targeting.prefix.is_none());
        assert!(cfg.status.banner.is_none());
        assert!(cfg.features.gzip_outbound.is_none());
        assert!(cfg.bidders.is_empty());
    }

    #[test]
    fn range_bounds_reject_zero_tmax() {
        let cfg: AuctioneerAppConfig = toml::from_str("[auction]\ndefault_tmax_ms = 0\n").unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn prefix_length_capped_to_eight() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[targeting]\nprefix = \"toolongprefix\"\n").unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn truncate_below_min_key_length_rejected() {
        // MIN_KEY_LENGTH = 12 in build_keys; anything below that is
        // guaranteed to collapse every key to the prefix fallback.
        let cfg: AuctioneerAppConfig =
            toml::from_str("[targeting]\ntruncate_target_attribute = 8\n").unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn max_bidder_response_bytes_1kb_min() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[auction]\nmax_bidder_response_bytes = 512\n").unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn per_bidder_overrides_parse() {
        let cfg: AuctioneerAppConfig = toml::from_str(
            "[bidders.fake_aaa]\nenabled = false\ntimeout_ms = 250\n\n\
             [bidders.fake_bbb]\nenabled = true\nsample_rate = 0.5\n",
        )
        .unwrap();
        assert_eq!(cfg.bidders["fake_aaa"].enabled, Some(false));
        assert_eq!(cfg.bidders["fake_aaa"].timeout_ms, Some(250_u32));
        assert_eq!(cfg.bidders["fake_bbb"].enabled, Some(true));
        assert!((cfg.bidders["fake_bbb"].sample_rate.unwrap() - 0.5).abs() < f64::EPSILON);
        // Both entries individually valid → outer validate() passes.
        cfg.validate().expect("both entries individually valid");
    }

    #[test]
    fn per_bidder_sample_rate_out_of_range_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_aaa]\nsample_rate = 42.0\n").unwrap();
        assert!(
            cfg.validate().is_err(),
            "sample_rate = 42.0 must fail the 0..=1 range validator",
        );
    }

    /// `validator`'s built-in `range` accepts `NaN` (both bound
    /// comparisons are false), and TOML parses the bare `nan` literal —
    /// so the custom finiteness check is the only thing standing between
    /// a deploy typo and a sampler that draws against a `NaN` rate.
    #[test]
    fn sample_rate_nan_is_rejected_at_config_load() {
        let overrides = BidderOverrides {
            sample_rate: Some(f64::NAN),
            ..BidderOverrides::default()
        };
        assert!(
            overrides.validate().is_err(),
            "a NaN sample_rate must be rejected at config load",
        );
    }

    /// The reach path an operator actually takes: TOML spells `nan` as
    /// a bare float literal, so the value arrives through
    /// deserialization rather than being constructed in Rust. The three
    /// sibling tests build `BidderOverrides` directly and so never
    /// prove the literal parses to something the validator rejects.
    #[test]
    fn sample_rate_nan_from_a_toml_literal_is_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_aaa]\nsample_rate = nan\n").expect("nan parses as f64");
        assert!(
            cfg.bidders["fake_aaa"].sample_rate.is_some_and(f64::is_nan),
            "the TOML literal must deserialize to NaN, or this guards nothing",
        );
        assert!(
            cfg.validate().is_err(),
            "a NaN sample_rate reaching config load via TOML must be rejected",
        );
    }

    #[test]
    fn sample_rate_positive_infinity_is_rejected_at_config_load() {
        let overrides = BidderOverrides {
            sample_rate: Some(f64::INFINITY),
            ..BidderOverrides::default()
        };
        assert!(
            overrides.validate().is_err(),
            "a positive-infinity sample_rate must be rejected at config load",
        );
    }

    #[test]
    fn sample_rate_negative_infinity_is_rejected_at_config_load() {
        let overrides = BidderOverrides {
            sample_rate: Some(f64::NEG_INFINITY),
            ..BidderOverrides::default()
        };
        assert!(
            overrides.validate().is_err(),
            "a negative-infinity sample_rate must be rejected at config load",
        );
    }

    #[test]
    fn per_bidder_timeout_ms_zero_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[bidders.fake_aaa]\ntimeout_ms = 0\n").unwrap();
        assert!(
            cfg.validate().is_err(),
            "timeout_ms = 0 must fail the min=1 range validator",
        );
    }

    /// TOML table names are strings, so the enforcement maps have to
    /// accept `[gdpr.tcf2.purposes.2]` — the canonical spelling an
    /// operator writes — and not just a JSON integer key.
    #[test]
    fn enforcement_map_keys_parse_from_toml_table_names() {
        let cfg: AuctioneerAppConfig = toml::from_str(
            "[gdpr.tcf2.purposes.2]\nenforce_purpose = \"disabled\"\n\n\
             [gdpr.tcf2.special_features.1]\nenforce = \"enabled\"\n\n\
             [accounts.acct-42.gdpr.purposes.3]\nenforce_vendors = \"enabled\"\n",
        )
        .expect("keyed enforcement tables parse");
        let tcf2 = cfg
            .gdpr
            .as_ref()
            .and_then(|gdpr| gdpr.tcf2.as_ref())
            .expect("the host tcf2 block is present");
        assert!(
            tcf2.purposes
                .as_ref()
                .is_some_and(|map| map.contains_key(&2_u8)),
        );
        assert!(
            tcf2.special_features
                .as_ref()
                .is_some_and(|map| map.contains_key(&1_u8)),
        );
        assert!(
            cfg.accounts["acct-42"]
                .gdpr
                .as_ref()
                .and_then(|gdpr| gdpr.purposes.as_ref())
                .is_some_and(|map| map.contains_key(&3_u8)),
        );
        cfg.validate().expect("in-range ids validate");
    }

    #[test]
    fn purpose_id_outside_one_to_ten_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[gdpr.tcf2.purposes.0]\nenforce_purpose = \"enabled\"\n")
                .expect("the map parses; the range check is a validator");
        assert!(cfg.validate().is_err(), "purpose id 0 is never valid");
    }

    /// The account map's custom validator has to recurse, or a bad
    /// `[accounts.<id>]` entry would sail through `validate()`.
    #[test]
    fn account_special_feature_id_out_of_range_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[accounts.acct-42.gdpr.special_features.13]\nenforce = \"enabled\"\n")
                .expect("the map parses; the range check is a validator");
        assert!(
            cfg.validate().is_err(),
            "special feature ids stop at the 12-slot wire bitfield",
        );
    }

    /// Key `0` is the lower edge outside `PURPOSE_ID_RANGE`: the map
    /// deserializes fine (TOML table keys are strings) but the custom
    /// validator must fire through the `gdpr` -> `tcf2` nested chain.
    #[test]
    fn host_purpose_key_zero_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[gdpr.tcf2.purposes.0]\nenforce_purpose = \"enabled\"\n")
                .expect("the map parses; the range check is a validator");
        assert!(cfg.validate().is_err(), "host purpose id 0 is never valid");
    }

    #[test]
    fn host_special_feature_key_overflow_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[gdpr.tcf2.special_features.13]\nenforce = \"enabled\"\n")
                .expect("the map parses; the range check is a validator");
        assert!(
            cfg.validate().is_err(),
            "host special feature ids stop at the 12-slot wire bitfield",
        );
    }

    /// Proves `validate_account_map` recurses into each entry's own
    /// `AccountOverride::validate()` — an out-of-range key nested three
    /// levels down (`accounts.<id>.gdpr.purposes.<id>`) must still fail
    /// the top-level `AuctioneerAppConfig::validate()`.
    #[test]
    fn account_purpose_key_overflow_rejected() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[accounts.pub1.gdpr.purposes.11]\nenforce_purpose = \"enabled\"\n")
                .expect("the map parses; the range check is a validator");
        assert!(
            cfg.validate().is_err(),
            "account purpose ids stop at the 10-purpose roster",
        );
    }

    /// The inclusive upper (and lower) edges must NOT be rejected —
    /// pins the boundary the overflow/zero tests approach from outside.
    #[test]
    fn in_range_enforcement_keys_accepted() {
        let cfg: AuctioneerAppConfig = toml::from_str(
            "[gdpr.tcf2.purposes.10]\nenforce_purpose = \"enabled\"\n\n\
             [gdpr.tcf2.special_features.12]\nenforce = \"enabled\"\n\n\
             [accounts.pub1.gdpr.purposes.1]\nenforce_vendors = \"enabled\"\n",
        )
        .expect("the map parses; the range check is a validator");
        cfg.validate()
            .expect("the inclusive upper/lower edges (10, 12, 1) must not be rejected");
    }

    #[test]
    fn gdpr_default_value_must_be_zero_or_one() {
        let cfg: AuctioneerAppConfig =
            toml::from_str("[gdpr]\ndefault_value = \"yes\"\n").expect("the string parses");
        assert!(cfg.validate().is_err());
        let ok: AuctioneerAppConfig =
            toml::from_str("[gdpr]\ndefault_value = \"1\"\n").expect("the string parses");
        ok.validate().expect("`1` is a legal wire value");
    }

    #[test]
    fn feature_flag_toggles_parse() {
        let cfg: AuctioneerAppConfig = toml::from_str(
            "[features]\ngzip_outbound = false\nreturnallbidstatus_default = true\n",
        )
        .unwrap();
        assert_eq!(cfg.features.gzip_outbound, Some(false));
        assert_eq!(cfg.features.returnallbidstatus_default, Some(true));
    }

    #[test]
    fn empty_runtime_alias_name_is_rejected_by_effective_config() {
        let app = AuctioneerAppConfig {
            aliases: Some(BTreeMap::from([(String::new(), "fake_aaa".to_owned())])),
            ..AuctioneerAppConfig::default()
        };
        let error = app
            .validate_effective_with_catalog(
                &HostConfig::default(),
                &AccountConfig::default(),
                &[BidderName::new("fake_aaa")],
            )
            .expect_err("empty alias must fail before callback construction");
        assert_eq!(error, "host alias name must be nonempty");
    }

    #[test]
    fn empty_inherited_alias_name_is_rejected_by_effective_config() {
        let host = HostConfig {
            aliases: BTreeMap::from([(String::new(), "fake_aaa".to_owned())]),
            ..HostConfig::default()
        };
        let error = AuctioneerAppConfig::default()
            .validate_effective_with_catalog(
                &host,
                &AccountConfig::default(),
                &[BidderName::new("fake_aaa")],
            )
            .expect_err("inherited empty alias must fail before callback construction");
        assert_eq!(error, "host alias name must be nonempty");
    }

    #[test]
    fn runtime_alias_over_64_bytes_is_rejected_without_priority_groups() {
        for alias in ["a".repeat(65), format!("{}a", "\u{e9}".repeat(32))] {
            let app = AuctioneerAppConfig {
                aliases: Some(BTreeMap::from([(alias, "fake_aaa".to_owned())])),
                ..AuctioneerAppConfig::default()
            };
            let error = app
                .validate_effective_with_catalog(
                    &HostConfig::default(),
                    &AccountConfig::default(),
                    &[BidderName::new("fake_aaa")],
                )
                .expect_err("oversized alias must fail config validation, not a later callback");
            assert!(error.contains("alias"), "{error}");
            assert!(error.contains("64"), "{error}");
        }
    }

    #[test]
    fn inherited_alias_over_128_bytes_is_rejected_without_priority_groups() {
        let host = HostConfig {
            aliases: BTreeMap::from([("a".repeat(129), "fake_aaa".to_owned())]),
            ..HostConfig::default()
        };
        let error = AuctioneerAppConfig::default()
            .validate_effective_with_catalog(
                &host,
                &AccountConfig::default(),
                &[BidderName::new("fake_aaa")],
            )
            .expect_err("inherited aliases obey the same name limit");
        assert!(error.contains("alias"), "{error}");
        assert!(error.contains("128"), "{error}");
    }

    #[test]
    fn runtime_aliases_at_64_utf8_bytes_are_accepted() {
        for alias in [
            format!("fake_{}", "a".repeat(59)),
            format!("fake_a{}", "\u{e9}".repeat(29)),
        ] {
            let app = AuctioneerAppConfig {
                aliases: Some(BTreeMap::from([(alias, "fake_aaa".to_owned())])),
                ..AuctioneerAppConfig::default()
            };
            app.validate_effective_with_catalog(
                &HostConfig::default(),
                &AccountConfig::default(),
                &[BidderName::new("fake_aaa")],
            )
            .expect("runtime alias at the limit fits");
        }
    }

    #[test]
    fn inherited_aliases_at_128_utf8_bytes_are_accepted() {
        for alias in [
            format!("fake_{}", "a".repeat(123)),
            format!("fake_a{}", "\u{e9}".repeat(61)),
        ] {
            let host = HostConfig {
                aliases: BTreeMap::from([(alias, "fake_aaa".to_owned())]),
                ..HostConfig::default()
            };
            AuctioneerAppConfig::default()
                .validate_effective_with_catalog(
                    &host,
                    &AccountConfig::default(),
                    &[BidderName::new("fake_aaa")],
                )
                .expect("inherited alias at the limit fits");
        }
    }

    #[test]
    fn outbound_overlay_rejects_out_of_range_values() {
        for field in ["max_outbound_raw_bytes", "max_outbound_request_bytes"] {
            for (value, valid) in [
                (0_u64, false),
                (1023, false),
                (1024, true),
                (0x0100_0000, true),
                (0x0100_0001, false),
            ] {
                let app: AuctioneerAppConfig =
                    toml::from_str(&format!("[auction]\n{field} = {value}\n")).unwrap();
                assert_eq!(app.validate().is_ok(), valid, "{field}={value}");
            }
        }
    }
}

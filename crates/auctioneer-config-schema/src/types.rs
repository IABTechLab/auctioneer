//! Runtime config types.
//!
//! Typed host, account and bidder configuration shared by build-time defaults
//! and runtime resolution. These declarations include auction, privacy,
//! activity, user-sync and resource settings without depending on the engine
//! or provider handlers that consume them.

use super::activity::ActivityConfigs;
pub use super::usersync::{
    AccountCookieSyncConfig, CooperativeConfig, HostCookieConfig, OptoutCookie,
    PurposeOneTreatment, PurposeOneTreatmentOverride, SkipWhen, SyncResponseFormat, SyncType,
    SyncerConfig, SyncerEndpoint, UserSyncConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use super::limits::MAX_BIDDER_NAME_BYTES;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostConfig {
    pub gdpr: GdprConfig,
    pub ccpa: CcpaConfig,
    pub lmt: LmtConfig,
    pub host_cookie: HostCookieConfig,
    pub user_sync: UserSyncConfig,
    /// Body returned by `GET /status` when no upstream probe is configured.
    pub status_response: String,
    pub resource_limits: ResourceLimits,
    /// Host-defined bidder aliases (`alias_name` → `core_bidder`).
    /// Surfaced by `/info/bidders` and available to the auction
    /// alias-resolution path. Empty by default.
    pub aliases: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GdprConfig {
    pub enabled: bool,
    pub default_value: String,
    pub host_vendor_id: Option<u32>,
    pub tcf2: Tcf2Config,
    /// Account ids the host flags as non-standard publishers; each id
    /// in the list toggles the account-side non-standard-publisher
    /// path. Host-owned, not account-owned.
    pub non_standard_publishers: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tcf2Config {
    pub enabled: bool,
    pub purpose_one_treatment: PurposeOneTreatment,
    pub purposes: BTreeMap<u8, PurposeEnforcement>,
    /// Mirrors the per-purpose enforcement map but keyed by
    /// special-feature id (1 = geo).
    pub special_features: BTreeMap<u8, SpecialFeatureEnforcement>,
}

/// The two SF-relevant fields (no `enforce_algo` since SF1 does not
/// have an A.2/A.3 split, and no `allow_li_transparency` since LI does
/// not apply to special features).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpecialFeatureEnforcement {
    pub enforce: TriState,
    pub vendor_exceptions: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PurposeEnforcement {
    pub enforce_algo: EnforceAlgo,
    pub enforce_purpose: TriState,
    pub enforce_vendors: TriState,
    pub vendor_exceptions: Vec<String>,
    // No `allow_li_transparency` field — the basic-A.3 LI-transparency
    // allowance is a per-call invocation override, not account config.
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnforceAlgo {
    #[default]
    Full,
    Basic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriState {
    #[default]
    Unset,
    Enabled,
    Disabled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChannelEnabled {
    pub web: TriState,
    pub app: TriState,
    pub amp: TriState,
    pub video: TriState,
    pub dooh: TriState,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountGdprConfig {
    /// Account-level tri-state the channel resolver checks BEFORE
    /// falling through to `host.gdpr.tcf2.enabled`. No
    /// `non_standard_publisher` field on the account; the flag is
    /// host-owned + account-id keyed.
    pub enabled: TriState,
    pub channel_enabled: ChannelEnabled,
    pub purposes: BTreeMap<u8, PurposeEnforcement>,
    pub special_features: BTreeMap<u8, SpecialFeatureEnforcement>,
    pub basic_enforcement_vendors: Vec<String>,
    pub purpose_one_treatment: PurposeOneTreatmentOverride,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccountCcpaConfig {
    pub enabled: TriState,
    #[serde(deserialize_with = "deserialize_ccpa_channels")]
    pub channel_enabled: ChannelEnabled,
}

// Keep CCPA's partial-channel defaults independent of the existing GDPR DTO.
fn deserialize_ccpa_channels<'de, D>(deserializer: D) -> Result<ChannelEnabled, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Default, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct Channels {
        web: TriState,
        app: TriState,
        amp: TriState,
        video: TriState,
        dooh: TriState,
    }
    let value = Channels::deserialize(deserializer)?;
    Ok(ChannelEnabled {
        web: value.web,
        app: value.app,
        amp: value.amp,
        video: value.video,
        dooh: value.dooh,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IpKeepBits {
    pub v4: u8,
    pub v6: u8,
}

// NO `Default` in the derive (a manual impl follows); per-field serde
// defaults because v4/v6 differ.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountPrivacyConfig {
    #[serde(default = "default_ipv4_anon")]
    pub ipv4: IpAnonConfig,
    #[serde(default = "default_ipv6_anon")]
    pub ipv6: IpAnonConfig,
    #[serde(default)]
    pub allowactivities: ActivityConfigs,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct IpAnonConfig {
    pub anon_keep_bits: u8,
}

fn default_ipv4_anon() -> IpAnonConfig {
    IpAnonConfig { anon_keep_bits: 24 }
}

fn default_ipv6_anon() -> IpAnonConfig {
    IpAnonConfig { anon_keep_bits: 56 }
}

impl Default for AccountPrivacyConfig {
    fn default() -> Self {
        Self {
            ipv4: default_ipv4_anon(),
            ipv6: default_ipv6_anon(),
            allowactivities: ActivityConfigs::default(),
        }
    }
}

impl AccountConfig {
    #[must_use]
    pub fn ip_keep_bits(&self) -> IpKeepBits {
        IpKeepBits {
            v4: self.privacy.ipv4.anon_keep_bits,
            v6: self.privacy.ipv6.anon_keep_bits,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CcpaConfig {
    pub enforce: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LmtConfig {
    pub enforce: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Default `tmax` ceiling enforced by the auction handler when the
    /// request omits one.
    pub default_tmax_ms: i64,
    /// Hard ceiling on the resolved auction budget.
    pub max_tmax_ms: i64,
    /// Per-bidder response size cap before the bid is dropped with
    /// code 4 `BadServerResponse`.
    pub max_bidder_response_bytes: usize,
    /// Maximum bidders selected per auction (registry order).
    pub max_bidders_per_auction: usize,
    /// Maximum total outbound requests across all bidders.
    pub max_outbound_requests: usize,
    /// Pre-encoding cap on one adapter-built outbound body, enforced
    /// DURING construction by the per-bidder `OutboundRequestSink`'s
    /// `CappedWriter`. Charged by the memory envelope's per-call term
    /// alongside the encoded cap below, so a configured value is
    /// resolved DOWN to what the envelope funds and never up.
    #[serde(default = "default_outbound_bytes")]
    pub max_outbound_raw_bytes: u64,
    /// Cap on the FINAL ENCODED outbound body, post-serialization and
    /// post-compression, with gzip expansion counted. Resolved against
    /// the envelope the same way.
    #[serde(default = "default_outbound_bytes")]
    pub max_outbound_request_bytes: u64,
    /// Compressed on-the-wire cap for one bidder response. Charged by
    /// the memory envelope's per-stream term, so a configured value is
    /// resolved DOWN to what the envelope funds and never up.
    pub max_transport_bytes: u64,
    /// Post-decode cap for one bidder response. The single largest term
    /// in the per-auction charge, so this is the knob a deployment
    /// under a tight isolate budget lowers first. Resolved against the
    /// envelope the same way.
    pub max_decoded_bytes: u64,
    /// Syntactic JSON node budget for the inbound request parse. A byte
    /// cap does not bound what a parse RETAINS, which is why the
    /// envelope charges nodes rather than bytes for it. Resolved
    /// against the envelope the same way.
    pub max_parsed_nodes: u32,
}

const fn default_outbound_bytes() -> u64 {
    256 * 1024
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            default_tmax_ms: 0,
            max_tmax_ms: 0,
            max_bidder_response_bytes: 0,
            max_bidders_per_auction: 0,
            max_outbound_requests: 0,
            max_outbound_raw_bytes: default_outbound_bytes(),
            max_outbound_request_bytes: default_outbound_bytes(),
            max_transport_bytes: 0,
            max_decoded_bytes: 0,
            max_parsed_nodes: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountConfig {
    pub disabled: bool,
    pub debug_allow: bool,
    pub bid_rounding: String,
    /// Account-level `hb_*` prefix override. Empty string means
    /// "no account override" — the request-level prefix (or the
    /// default `"hb"`) is used.
    pub targeting_prefix: String,
    /// Account-level cap on the emitted `hb_*` key length. `None`
    /// falls back to the default (`20`).
    pub truncate_target_attribute: Option<usize>,
    pub gdpr: AccountGdprConfig,
    pub cookie_sync: AccountCookieSyncConfig,
    #[serde(default)]
    pub ccpa: AccountCcpaConfig,
    // Defaults (`/24` v4, `/56` v6) come from
    // `AccountPrivacyConfig::default()`; the 32/128 range checks run
    // at build time in the codegen renderer.
    #[serde(default)]
    pub privacy: AccountPrivacyConfig,
}

/// Build-time public identity to implementation-key association.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BidderImplementationBinding {
    pub bidder: &'static str,
    pub implementation: &'static str,
}

/// All metadata declarations, including feature-disabled identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BidderMetadataDeclaration {
    pub bidder: &'static str,
    pub implementation: &'static str,
    pub required_feature: Option<&'static str>,
}

/// Target-independent allocation census of generated metadata expressions.
/// Counts requested payloads, not allocator overhead or executable residency.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MetadataHeapShape {
    pub string_bytes: u64,
    pub nonempty_strings: u64,
    pub string_elements: u64,
    pub media_type_elements: u64,
    pub sync_type_elements: u64,
    pub nonempty_vectors: u64,
}

/// Build-generated metadata census, available before lazy materialization.
/// The digest binds constructor and binding source, not a qualification record.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MetadataStorageDescriptor {
    pub active_bidders: u64,
    pub active_bindings: u64,
    pub declarations: u64,
    pub heap: MetadataHeapShape,
    pub source_sha256: [u8; 32],
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BidderInfo {
    pub name: String,
    /// Immutable compiled ancestry, independent of runtime host/request aliases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias_of: Option<String>,
    /// `None` preserves legacy activation; explicit runtime enablement overrides this default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_enabled: Option<bool>,
    pub endpoint: String,
    pub uses_https: bool,
    pub disabled: bool,
    pub debug_allow: bool,
    pub endpoint_compression: Option<Compression>,
    pub gvl_vendor_id: Option<u32>,
    pub host_allowlist: Option<Vec<String>>,
    /// Per-bidder timeout in milliseconds, sourced from the TOML
    /// `bidderTimeoutMs` field. `None` means the runtime falls back
    /// to the auction-wide `bidder_visible_tmax_ms`. Read by
    /// `run_auction` when it builds the per-bidder call list.
    pub bidder_timeout_ms: Option<u32>,
    /// The set of `{{...}}` macro names that appear in this bidder's
    /// `endpoint` template, extracted by codegen from the TOML at
    /// build time. Stored without surrounding braces or leading `.`
    /// (e.g. `["AccountID", "PublisherID"]`) so auction core's
    /// `BidderRequestInfo.endpoint_macros` and `EndpointBuilder` can
    /// consume it as a flat allow-list. Empty for fixed-host endpoints
    /// with no path/query macros; the special `{{.Host}}` authority
    /// macro is NOT included (it is handled via `host_allowlist`).
    pub endpoint_macros: Vec<String>,
    pub maintainer_email: Option<String>,
    /// Per-inventory media-type support from `capabilities.*.mediaTypes`
    /// in the bidder TOML. Consumed by the auction core exchange to filter bidders
    /// against the incoming `site` / `app` / `dooh` shape.
    pub capabilities: Capabilities,
    /// `userSync` block from the TOML. Empty when the bidder ships no
    /// syncer. Consumed by `/cookie_sync` (user sync).
    pub user_sync: SyncerConfig,
    /// OpenRTB-level capabilities (version, GPP support, etc.) from the
    /// `openrtb` block. Consumed by the auction core exchange when forwarding the
    /// request shape.
    pub openrtb: OpenRtbCapabilities,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Compression {
    Gzip,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// Media types supported on `site` inventory requests.
    pub site: Vec<MediaType>,
    /// Media types supported on `app` inventory requests.
    pub app: Vec<MediaType>,
    /// Media types supported on `dooh` inventory requests.
    pub dooh: Vec<MediaType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Banner,
    Video,
    Native,
    Audio,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OpenRtbCapabilities {
    /// `openrtb.version` — declared `OpenRTB` version (e.g. "2.6").
    pub version: Option<String>,
    /// `openrtb.gpp-supported` — bidder accepts GPP / `gpp_sid` forwarding.
    pub gpp_supported: bool,
}

/// The deploy-immutable runtime bootstrap policy, baked from
/// `data/runtime-bootstrap.toml` at build time as
/// `auctioneer_config::RUNTIME_BOOTSTRAP`. Nothing at runtime — no environment
/// variable, no config-store value — can change it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuntimeBootstrap {
    /// Whether a bound config store with no blob pushed is an
    /// intentionally unconfigured deploy (degrade with a WARN) or a
    /// misconfigured one (fail closed).
    pub allow_missing_config: bool,
    /// The bound on ONE typed-config extraction, in milliseconds,
    /// installed as `ConfigExtractionLimits.timeout` so the host's own
    /// extraction budget enforces it. Baked rather than pushed because
    /// it bounds the very read that resolves the pushed config.
    pub max_config_fetch_ms: u32,
}

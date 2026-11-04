//! Pure-logic half of the `auctioneer-config-data` codegen: TOML types +
//! `render_*` helpers. Build scripts and native tests consume
//! this library module; serving builds do not enable `build-support`.
//!
//! Everything in this module is deterministic and file-system free.
//! `build.rs` retains file I/O, validation-that-emits-BuildError, and
//! the `emit_*` coordinators that stitch imports + module boilerplate
//! around the rendered struct literals.

pub use crate::url_safety as build_support;
pub use crate::usersync;
use crate::{
    activity,
    bindings::{BindingInput, compile_bindings},
    limits, types,
};

pub use types::AccountCcpaConfig;

pub mod storage;

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens as _, quote};
use serde::Deserialize;
use serde::de::Error as _;

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::ops::RangeInclusive;
use std::str;
use validator::Validate;

pub type BuildResult<T = ()> = Result<T, Box<dyn Error>>;

/// Typed validation and syntax errors from the build renderers.
#[derive(Debug)]
pub enum RenderError {
    Activity(&'static str),
    UserSync(String),
    StatusResponseTooLarge {
        bytes: usize,
    },
    InvalidTristate(String),
    InvalidGdprDefaultValue(String),
    IpKeepBitsOutOfRange {
        family: &'static str,
        value: u8,
        max: u8,
    },
    // Baked-config enforcement-map key out of range.
    PurposeIdOutOfRange {
        id: u8,
    },
    SpecialFeatureIdOutOfRange {
        id: u8,
    },
    // `enforce_algo` domain is "full" | "basic", NOT a tri-state — a
    // separate error from `InvalidTristate`.
    InvalidEnforceAlgo(String),
    // `host_vendor_id` must be a u16 (0..=65535); reject rather than
    // clamp.
    HostVendorIdOutOfRange {
        value: u32,
    },
    // Same u16 namespace for a bidder's own `gvlVendorID`: a larger
    // value could only reach the GVL by truncating onto an unrelated
    // vendor, so the build fails instead.
    BidderGvlVendorIdOutOfRange {
        bidder: String,
        value: u32,
    },
}

#[expect(
    clippy::use_debug,
    reason = "diagnostics quote rejected scalar values unambiguously"
)]
impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Activity(message) => f.write_str(message),
            RenderError::StatusResponseTooLarge { bytes } => {
                write!(
                    f,
                    "status_response is {bytes} bytes (maximum {})",
                    limits::MAX_STATUS_BANNER_BYTES
                )
            }
            RenderError::InvalidTristate(v) => write!(f, "invalid tri-state {v:?}"),
            RenderError::InvalidGdprDefaultValue(v) => {
                write!(f, "invalid gdpr default_value {v:?}")
            }
            RenderError::IpKeepBitsOutOfRange { family, value, max } => {
                write!(f, "{family}.anon_keep_bits {value} exceeds max {max}")
            }
            RenderError::UserSync(message) => write!(f, "{message}"),
            RenderError::PurposeIdOutOfRange { id } => {
                write!(f, "purpose id {id} out of range (must be 1..=10)")
            }
            RenderError::SpecialFeatureIdOutOfRange { id } => {
                write!(f, "special feature id {id} out of range (must be 1..=12)")
            }
            RenderError::InvalidEnforceAlgo(v) => {
                write!(
                    f,
                    "invalid enforce_algo {v:?} (must be \"full\" or \"basic\")"
                )
            }
            RenderError::HostVendorIdOutOfRange { value } => {
                write!(f, "host_vendor_id {value} out of range (must be 0..=65535)")
            }
            RenderError::BidderGvlVendorIdOutOfRange { bidder, value } => {
                write!(
                    f,
                    "gvlVendorID out of range for bidder {bidder}: {value} (must be 0..=65535)"
                )
            }
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "leaf error with no source; the defaulted `Error` methods are all correct"
)]
impl Error for RenderError {}

fn option_tokens<T: quote::ToTokens>(value: Option<T>) -> TokenStream {
    value.map_or_else(|| quote!(None), |inner| quote!(Some(#inner)))
}

fn owned_string(value: &str) -> TokenStream {
    quote!(#value.to_owned())
}

fn string_vec_tokens(items: &[String]) -> TokenStream {
    if items.is_empty() {
        quote!(Vec::new())
    } else {
        let literals = items.iter().map(|item| owned_string(item));
        quote!(vec![#(#literals),*])
    }
}

// The spelling is built only from canonical numeric digits and a fixed suffix.
fn typed_integer(digits: &str, suffix: &str) -> syn::LitInt {
    syn::LitInt::new(&[digits, "_", suffix].concat(), Span::call_site())
}

// The `Raw` suffix on every DTO in this file means "what the build
// script deserialises the config files INTO, before validation and
// before rendering". They carry the `#[serde(deny_unknown_fields)]`
// and `#[validate(range(...))]` rules, are consumed only by the
// `render_*` functions that emit generated Rust, and never exist at
// runtime — the runtime types live in `src/types.rs` under the
// unsuffixed names. The suffix names that build-time role and nothing
// about the on-disk format, which is TOML.

/// `#[derive(Validate)]` covers the purely-numeric / shape constraints
/// (e.g. `bidder_timeout_ms` must be > 0). The SSRF / URL gate
/// (`validate_endpoint_url`, `validate_syncer_template`) stays as
/// explicit calls in `build.rs` because it needs `allow_insecure`,
/// which is determined per source-directory, not per-field.
#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct BidderInfoRaw {
    pub implementation: String,
    /// Build-only contract supplied by the owning manifest, not the definition.
    #[serde(skip)]
    pub supported_endpoint_macros: Vec<String>,
    #[serde(skip)]
    pub native_only: bool,
    #[serde(default)]
    pub required_feature: Option<String>,
    #[serde(default, rename = "aliasOf")]
    pub alias_of: Option<String>,
    #[serde(default, rename = "defaultEnabled")]
    pub default_enabled: Option<bool>,
    pub endpoint: String,
    #[serde(default)]
    pub maintainer: Option<MaintainerRaw>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default, rename = "endpointCompression")]
    pub endpoint_compression: Option<String>,
    #[serde(default, rename = "gvlVendorID")]
    pub gvl_vendor_id: Option<u32>,
    /// Host-varying bidders (those whose endpoint URL contains
    /// `{{.Host}}`) MUST declare the set of hosts they may resolve
    /// `{{.Host}}` to. Reads the TOML `hostAllowlist` field directly
    /// into the codegen output so the runtime sees the real list
    /// instead of an always-None placeholder.
    #[serde(default, rename = "hostAllowlist")]
    pub host_allowlist: Option<Vec<String>>,
    /// Per-bidder timeout in milliseconds. Optional -- when unset the
    /// runtime falls back to the auction-wide
    /// `bidder_visible_tmax_ms`. Zero is a config bug (the bidder
    /// would be unreachable), enforced via
    /// `#[validate(range(min = 1))]`.
    #[serde(default, rename = "bidderTimeoutMs")]
    #[validate(range(
        min = 1_u32,
        message = "bidderTimeoutMs must be > 0; omit the field to fall back to bidder_visible_tmax_ms"
    ))]
    pub bidder_timeout_ms: Option<u32>,
    #[serde(default)]
    pub debug: Option<DebugRaw>,
    #[serde(default)]
    pub capabilities: CapabilitiesRaw,
    #[serde(default, rename = "userSync")]
    pub user_sync: UserSyncRaw,
    #[serde(default)]
    pub openrtb: OpenRtbRaw,
}

#[derive(Debug, Deserialize)]
pub struct MaintainerRaw {
    #[serde(default)]
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DebugRaw {
    #[serde(default)]
    pub allow: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CapabilitiesRaw {
    #[serde(default)]
    pub site: Option<CapabilityShapeRaw>,
    #[serde(default)]
    pub app: Option<CapabilityShapeRaw>,
    #[serde(default)]
    pub dooh: Option<CapabilityShapeRaw>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CapabilityShapeRaw {
    #[serde(default, rename = "mediaTypes")]
    pub media_types: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct UserSyncRaw {
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub iframe: Option<SyncerEndpointRaw>,
    #[serde(default)]
    pub redirect: Option<SyncerEndpointRaw>,
    #[serde(default)]
    pub supports: Vec<usersync::SyncType>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default, rename = "skipWhen")]
    pub skip_when: usersync::SkipWhen,
    #[serde(default, rename = "defaultResponse")]
    pub default_response: Option<usersync::SyncResponseFormat>,
}

#[derive(Debug, Default, Deserialize)]
pub struct SyncerEndpointRaw {
    #[serde(default)]
    pub url: String,
    #[serde(default, rename = "userMacro")]
    pub user_macro: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct OpenRtbRaw {
    #[serde(default)]
    pub version: Option<toml::Value>,
    #[serde(default, rename = "gpp-supported")]
    pub gpp_supported: Option<bool>,
}

// `#[serde(deny_unknown_fields)]` on the ENTIRE host/account raw
// privacy graph — a typo in a privacy-critical product-data key
// (`enfroce_purpose`, `defualt_value`, …) must FAIL the build, not
// silently select a default. That includes the PARENT wrappers
// `HostDefaultRaw` and `AccountRaw` themselves, not just the nested
// privacy DTOs: without the attribute on the two top-level structs, a
// ROOT-level typo (e.g. `[gdrp]` or a misspelled top-level key under
// product data's `[host]` / `[account]`) is silently ignored — exactly
// the privacy-config fail-open this rule exists to close.
//
// It also covers `CcpaRaw`, `LmtRaw`, `HostCookieRaw` and
// `ResourceLimitsRaw`, reviewed product-data tables whose fields carry
// a `#[serde(default)]` or a natural zero.
// Without the guard, a misspelling INSIDE one of them leaves the
// default in place silently — `ccpa.enfroce = true` would read as
// "CCPA enforcement on" and ship with it off.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostDefaultRaw {
    pub gdpr: GdprRaw,
    pub ccpa: CcpaRaw,
    pub lmt: LmtRaw,
    pub host_cookie: HostCookieRaw,
    #[serde(default)]
    pub user_sync: Option<usersync::UserSyncConfig>,
    pub status_response: String,
    pub resource_limits: ResourceLimitsRaw,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GdprRaw {
    pub enabled: bool,
    pub default_value: String,
    // Plain `u32`, default 0 (= "no host vendor id declared", spec
    // §7.9 valid). NOT `Option` / a sentinel type / clamped-into-`u16`
    // — the render path validates `0..=65535` and rejects `65536` with
    // `HostVendorIdOutOfRange` rather than truncating silently.
    #[serde(default)]
    pub host_vendor_id: u32,
    pub tcf2: Tcf2ConfigRaw,
    #[serde(default)]
    pub non_standard_publishers: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tcf2ConfigRaw {
    pub enabled: bool,
    #[serde(default)]
    pub purpose_one_treatment: usersync::PurposeOneTreatment,
    #[serde(default, deserialize_with = "u8_keyed_map")]
    pub purposes: BTreeMap<u8, PurposeEnforcementRaw>,
    #[serde(default, deserialize_with = "u8_keyed_map")]
    pub special_features: BTreeMap<u8, SpecialFeatureEnforcementRaw>,
}

// ── shared purpose / SF / channel DTOs ──
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurposeEnforcementRaw {
    pub enforce_algo: String,    // "full" | "basic"
    pub enforce_purpose: String, // tri-state: unset|enabled|disabled
    pub enforce_vendors: String, // tri-state
    // No `allow_li_transparency` field on either side.
    #[serde(default)]
    pub vendor_exceptions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialFeatureEnforcementRaw {
    pub enforce: String,
    #[serde(default)]
    pub vendor_exceptions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelEnabledRaw {
    pub web: String,
    pub app: String,
    pub amp: String,
    pub video: String,
    pub dooh: String,
}

/// TOML table keys are always strings, and serde's stock
/// `BTreeMap<u8, _>` impl refuses them (`invalid type: string "1",
/// expected u8`). Parse each key through `u8::from_str` so
/// `[gdpr.tcf2.purposes.1]` lands on the numeric key the range
/// validators check; a non-numeric or overflowing key fails
/// deserialization.
fn u8_keyed_map<'de, D, V>(deserializer: D) -> Result<BTreeMap<u8, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    V: Deserialize<'de>,
{
    let raw = BTreeMap::<String, V>::deserialize(deserializer)?;
    let mut out = BTreeMap::new();
    for (key, value) in raw {
        let id = key.parse::<u8>().map_err(|_int_err| {
            D::Error::custom(format!("enforcement-map key {key:?} is not a u8 id"))
        })?;
        out.insert(id, value);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CcpaRaw {
    pub enforce: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LmtRaw {
    pub enforce: bool,
}

pub type HostCookieRaw = usersync::HostCookieConfig;

/// `#[validate(range(min = 1))]` enforces the positivity rule for
/// every cap (a zero value means the cap is effectively disabled,
/// which is a config bug rather than a usable default).
/// `#[serde(default)]` on each field lets a missing value parse as 0
/// so the validator's error message names the offending field
/// instead of serde's generic "missing field" complaint.
#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct ResourceLimitsRaw {
    #[validate(range(min = 1_i64, message = "resource_limits.default_tmax_ms must be > 0"))]
    pub default_tmax_ms: i64,
    #[serde(default)]
    #[validate(range(min = 1_i64, message = "resource_limits.max_tmax_ms must be > 0"))]
    pub max_tmax_ms: i64,
    #[serde(default)]
    #[validate(range(
        min = 1_usize,
        message = "resource_limits.max_bidder_response_bytes must be > 0"
    ))]
    pub max_bidder_response_bytes: usize,
    #[serde(default)]
    #[validate(range(
        min = 1_usize,
        message = "resource_limits.max_bidders_per_auction must be > 0"
    ))]
    pub max_bidders_per_auction: usize,
    #[serde(default)]
    #[validate(range(
        min = 1_usize,
        message = "resource_limits.max_outbound_requests must be > 0"
    ))]
    pub max_outbound_requests: usize,
    #[serde(default = "default_outbound_bytes")]
    #[validate(range(min = 1024_u64, max = 0x0100_0000_u64))]
    pub max_outbound_raw_bytes: u64,
    #[serde(default = "default_outbound_bytes")]
    #[validate(range(min = 1024_u64, max = 0x0100_0000_u64))]
    pub max_outbound_request_bytes: u64,
    #[serde(default)]
    #[validate(range(
        min = 1_u64,
        message = "resource_limits.max_transport_bytes must be > 0"
    ))]
    pub max_transport_bytes: u64,
    #[serde(default)]
    #[validate(range(min = 1_u64, message = "resource_limits.max_decoded_bytes must be > 0"))]
    pub max_decoded_bytes: u64,
    #[serde(default)]
    #[validate(range(min = 1_u32, message = "resource_limits.max_parsed_nodes must be > 0"))]
    pub max_parsed_nodes: u32,
}

const fn default_outbound_bytes() -> u64 {
    256 * 1024
}

// The gdpr/privacy asymmetry below is intentional and fail-closed:
// - `gdpr` is REQUIRED (no `#[serde(default)]`). `AccountGdprConfigRaw`
//   deliberately does NOT derive/impl `Default` (there is no safe
//   default for `enabled` / `channel_enabled` — a blank tri-state or
//   empty channel set would silently disable enforcement), so
//   `#[serde(default)]` on it would not even compile (E0277). The
//   product data's `[account]` always ships a complete `[gdpr]`
//   block; a build that removes it must FAIL, not silently default
//   privacy config.
// - `privacy` keeps `#[serde(default)]` because
//   `AccountPrivacyConfigRaw` has a SAFE manual `Default` — an absent
//   block masks MORE (`/24` v4, `/56` v6), which is the conservative
//   direction.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountRaw {
    pub disabled: bool,
    pub debug_allow: bool,
    pub bid_rounding: String,
    #[serde(default)]
    pub targeting_prefix: String,
    #[serde(default)]
    pub truncate_target_attribute: Option<usize>,
    pub gdpr: AccountGdprConfigRaw,
    #[serde(default)]
    pub cookie_sync: usersync::AccountCookieSyncConfig,
    #[serde(default)]
    pub ccpa: AccountCcpaConfig,
    #[serde(default)]
    pub privacy: AccountPrivacyConfigRaw,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountGdprConfigRaw {
    pub enabled: String,
    pub channel_enabled: ChannelEnabledRaw,
    #[serde(default, deserialize_with = "u8_keyed_map")]
    pub purposes: BTreeMap<u8, PurposeEnforcementRaw>,
    #[serde(default, deserialize_with = "u8_keyed_map")]
    pub special_features: BTreeMap<u8, SpecialFeatureEnforcementRaw>,
    #[serde(default)]
    pub basic_enforcement_vendors: Vec<String>,
    #[serde(default)]
    pub purpose_one_treatment: usersync::PurposeOneTreatmentOverride,
}

// ── privacy (IP-anon) — v6 defaults to 56, not 24 ──
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountPrivacyConfigRaw {
    #[serde(default = "default_ipv4_raw")]
    pub ipv4: IpAnonConfigRaw,
    #[serde(default = "default_ipv6_raw")]
    pub ipv6: IpAnonConfigRaw,
    #[serde(default)]
    pub allowactivities: BTreeMap<activity::Activity, activity::ActivityConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IpAnonConfigRaw {
    pub anon_keep_bits: u8,
}

fn default_ipv4_raw() -> IpAnonConfigRaw {
    IpAnonConfigRaw { anon_keep_bits: 24 }
}

fn default_ipv6_raw() -> IpAnonConfigRaw {
    IpAnonConfigRaw { anon_keep_bits: 56 }
}

// Manual `Default` (NOT derived — a derive would use
// `IpAnonConfigRaw`'s own `Default` for BOTH families and print
// v6=24). `IpAnonConfigRaw` has NO `Default`, so the only way to build
// the pair is via the two family functions above.
impl Default for AccountPrivacyConfigRaw {
    fn default() -> Self {
        Self {
            ipv4: default_ipv4_raw(),
            ipv6: default_ipv6_raw(),
            allowactivities: BTreeMap::new(),
        }
    }
}

/// The same publication predicate is used for metadata, bindings and serving dispatch.
#[must_use]
pub fn publication_gate(native_only: bool, required_feature: Option<&str>) -> TokenStream {
    let native = native_only.then(|| quote!(#[cfg(not(target_arch = "wasm32"))]));
    let feature = required_feature.map(|feature| quote!(#[cfg(feature = #feature)]));
    quote!(#native #feature)
}

/// Validate public bindings and render target-gated bindings plus unfiltered declarations.
///
/// # Errors
/// Rejects malformed names, implementation keys, or feature predicates.
pub fn render_bidder_bindings(
    bidders: &[(String, BidderInfoRaw)],
    known_features: &BTreeSet<String>,
) -> BuildResult<String> {
    let rows: Vec<_> = bidders
        .iter()
        .map(|(name, info)| BindingInput {
            bidder: name,
            implementation: &info.implementation,
            required_feature: info.required_feature.as_deref(),
        })
        .collect();
    compile_bindings(&rows, known_features)
        .map_err(|error| -> Box<dyn Error> { error.to_string().into() })?;
    let mut sorted: Vec<_> = bidders.iter().collect();
    sorted.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    let bindings = sorted.iter().map(|(name, info)| {
        let implementation = &info.implementation;
        let gate = publication_gate(info.native_only, info.required_feature.as_deref());
        quote!(#gate BidderImplementationBinding { bidder: #name, implementation: #implementation },)
    });
    let declarations = sorted.iter().map(|(name, info)| {
        let implementation = &info.implementation;
        let feature = option_tokens(info.required_feature.as_deref().map(|feature| quote!(#feature)));
        quote!(BidderMetadataDeclaration { bidder: #name, implementation: #implementation, required_feature: #feature },)
    });
    Ok(quote! {
        use crate::types::{BidderImplementationBinding, BidderMetadataDeclaration};
        #[must_use]
        pub const fn bidder_implementation_bindings() -> &'static [BidderImplementationBinding] {
            static BINDINGS: &[BidderImplementationBinding] = &[#(#bindings)*];
            BINDINGS
        }
        #[must_use]
        pub fn bidder_metadata_declarations() -> &'static [BidderMetadataDeclaration] {
            static DECLARATIONS: &[BidderMetadataDeclaration] = &[#(#declarations)*];
            DECLARATIONS
        }
    }
    .to_string())
}

/// Render one validated bidder metadata constructor.
///
/// # Errors
/// Rejects invalid bidder names, raw values, or endpoint macro declarations.
pub fn render_bidder_info(name: &str, info: &BidderInfoRaw) -> BuildResult<String> {
    if name.len() > limits::MAX_BIDDER_NAME_BYTES {
        return Err(format!(
            "bidder name is {} bytes (maximum {})",
            name.len(),
            limits::MAX_BIDDER_NAME_BYTES,
        )
        .into());
    }
    // `#[validate(range(min = 1))]` on bidder_timeout_ms enforces the
    // positivity rule. Hoist any field-level violation into a clean
    // build error naming the bidder before the render pulls the value.
    info.validate().map_err(|errs| -> Box<dyn Error> {
        format!("bidder {name:?} TOML failed validation: {errs}").into()
    })?;

    let compression = if info.endpoint_compression.as_deref() == Some("gzip") {
        quote!(Some(Compression::Gzip))
    } else {
        quote!(None)
    };
    let uses_https = info.endpoint.starts_with("https://");
    let disabled = info.disabled.unwrap_or(false);
    let alias_of = render_option_string(info.alias_of.as_ref());
    let default_enabled = render_option_bool(info.default_enabled);
    let debug_allow = info
        .debug
        .as_ref()
        .and_then(|debug| debug.allow)
        .unwrap_or(true);
    let gvl = match info.gvl_vendor_id {
        None => quote!(None),
        Some(id) if id > u32::from(u16::MAX) => {
            return Err(RenderError::BidderGvlVendorIdOutOfRange {
                bidder: name.to_owned(),
                value: id,
            }
            .into());
        }
        Some(id) => {
            let literal = proc_macro2::Literal::u32_unsuffixed(id);
            quote!(Some(#literal))
        }
    };
    let maintainer_email = option_tokens(
        info.maintainer
            .as_ref()
            .and_then(|m| m.email.as_deref())
            .filter(|addr| !addr.is_empty())
            .map(owned_string),
    );
    let bidder_timeout_ms = option_tokens(
        info.bidder_timeout_ms
            .map(|ms| typed_integer(&ms.to_string(), "u32")),
    );
    let endpoint = &info.endpoint;

    let host_allowlist = host_allowlist_tokens(info.host_allowlist.as_deref());
    let endpoint_macros =
        endpoint_macros_tokens(name, &info.endpoint, &info.supported_endpoint_macros)?;
    let capabilities = capabilities_tokens(&info.capabilities);
    let user_sync = user_sync_tokens(&info.user_sync);
    let openrtb = openrtb_tokens(&info.openrtb);
    Ok(quote!(BidderInfo {
        name: #name.to_owned(), endpoint: #endpoint.to_owned(), uses_https: #uses_https,
        alias_of: #alias_of, default_enabled: #default_enabled,
        disabled: #disabled, debug_allow: #debug_allow, endpoint_compression: #compression,
        gvl_vendor_id: #gvl, host_allowlist: #host_allowlist, bidder_timeout_ms: #bidder_timeout_ms,
        endpoint_macros: #endpoint_macros, maintainer_email: #maintainer_email,
        capabilities: #capabilities, user_sync: #user_sync, openrtb: #openrtb,
    })
    .to_string())
}

/// Render the `endpoint_macros` allow-list -- the path/query template
/// macros extracted from the endpoint template -- into the codegen
/// output. Empty list -> `Vec::new()`; otherwise a `vec![..]` of
/// owned `String`s. Names are stored without surrounding braces or
/// leading `.` (e.g. `"AccountID"`).
///
/// Rejects unclosed `{{` fragments and `{{.Host}}` outside the
/// authority up front, then checks the owning implementation's declaration.
/// Render the admitted endpoint macro vocabulary for one bidder.
///
/// # Errors
/// Rejects unclosed, misplaced, or undeclared endpoint macros.
pub fn render_endpoint_macros(
    bidder_name: &str,
    endpoint: &str,
    declared: &[String],
) -> BuildResult<String> {
    Ok(endpoint_macros_tokens(bidder_name, endpoint, declared)?.to_string())
}

fn endpoint_macros_tokens(
    bidder_name: &str,
    endpoint: &str,
    declared: &[String],
) -> BuildResult<TokenStream> {
    if build_support::endpoint_unclosed_macro_prefix(endpoint).is_some() {
        return Err(format!(
            "bidder {bidder_name:?} endpoint contains an unclosed template macro \
             (no matching `}}}}` before end-of-input). Close the macro \
             or remove the stray `{{{{`."
        )
        .into());
    }
    // `{{.Host}}` is strictly an authority-substitution token; it must
    // never appear in the path or query. Silently allowing it there
    // would let `endpoint = "https://fixed.example/rtb/{{.Host}}"` +
    // a declared `hostAllowlist` misclassify a fixed-host bidder as
    // host-varying (the runtime `select_host` path uses a substring
    // scan for `{{.Host}}`, not authority classification).
    if build_support::endpoint_has_host_macro_outside_authority(endpoint) {
        return Err(format!(
            "bidder {bidder_name:?} endpoint contains `{{{{.Host}}}}` in the \
             path or query. `{{{{.Host}}}}` is the authority-substitution token and may \
             only appear as the endpoint's host -- set the authority to exactly \
             `{{{{.Host}}}}` and declare a `hostAllowlist`."
        )
        .into());
    }
    let macros = build_support::extract_endpoint_macros(endpoint);
    build_support::validate_endpoint_macros(bidder_name, &macros, declared).map_err(
        |_error| -> Box<dyn Error> {
            format!(
                "bidder {bidder_name:?} endpoint contains an unsupported macro or invalid declaration"
            )
            .into()
        },
    )?;
    Ok(string_vec_tokens(&macros))
}

fn render_option_string(value: Option<&String>) -> TokenStream {
    option_tokens(value.map(|inner| owned_string(inner)))
}

fn render_option_bool(value: Option<bool>) -> TokenStream {
    option_tokens(value)
}

fn render_option_i64(value: Option<i64>) -> TokenStream {
    option_tokens(value.map(|inner| {
        let literal = typed_integer(&inner.unsigned_abs().to_string(), "i64");
        if inner < 0 {
            quote!(-#literal)
        } else {
            quote!(#literal)
        }
    }))
}

fn render_priority_groups(groups: &[Vec<String>]) -> TokenStream {
    if groups.is_empty() {
        quote!(Vec::new())
    } else {
        let literals = groups.iter().map(|group| string_vec_tokens(group));
        quote!(vec![#(#literals),*])
    }
}

fn render_treatment(value: usersync::PurposeOneTreatment) -> TokenStream {
    let enabled = value.enabled;
    let access_allowed = value.access_allowed;
    quote!(PurposeOneTreatment { enabled: #enabled, access_allowed: #access_allowed })
}

fn render_treatment_override(value: usersync::PurposeOneTreatmentOverride) -> TokenStream {
    let enabled = render_option_bool(value.enabled);
    let access_allowed = render_option_bool(value.access_allowed);
    quote!(PurposeOneTreatmentOverride { enabled: #enabled, access_allowed: #access_allowed })
}

fn render_host_cookie(cookie: &HostCookieRaw) -> TokenStream {
    let name = &cookie.cookie_name;
    let family = &cookie.family;
    let domain = render_option_string(cookie.domain.as_ref());
    let optout = option_tokens(cookie.optout_cookie.as_ref().map(|optout| {
        let optout_name = &optout.name;
        let optout_value = &optout.value;
        quote!(OptoutCookie { name: #optout_name.to_owned(), value: #optout_value.to_owned() })
    }));
    let ttl_magnitude = typed_integer(&cookie.ttl_days.unsigned_abs().to_string(), "i64");
    let ttl = if cookie.ttl_days < 0 {
        quote!(-#ttl_magnitude)
    } else {
        quote!(#ttl_magnitude)
    };
    let cap = typed_integer(&cookie.max_cookie_size_bytes.to_string(), "usize");
    quote!(HostCookieConfig { cookie_name: #name.to_owned(), family: #family.to_owned(),
        domain: #domain, ttl_days: #ttl, max_cookie_size_bytes: #cap, optout_cookie: #optout })
}

fn render_host_user_sync(sync: &usersync::UserSyncConfig) -> TokenStream {
    let external = &sync.external_url;
    let redirect = render_option_string(sync.redirect_url.as_ref());
    let priority = render_priority_groups(&sync.priority_groups);
    let origins = string_vec_tokens(&sync.allowed_origins);
    let coop = sync.cooperative.enabled_by_default;
    quote!(UserSyncConfig { external_url: #external.to_owned(), redirect_url: #redirect,
        cooperative: CooperativeConfig { enabled_by_default: #coop },
        priority_groups: #priority, allowed_origins: #origins })
}

fn render_account_cookie_sync(sync: &usersync::AccountCookieSyncConfig) -> TokenStream {
    let priority = option_tokens(
        sync.priority_groups
            .as_ref()
            .map(|groups| render_priority_groups(groups)),
    );
    let default = render_option_i64(sync.default_limit);
    let max = render_option_i64(sync.max_limit);
    let coop = render_option_bool(sync.default_coop_sync);
    quote!(AccountCookieSyncConfig { default_limit: #default, max_limit: #max,
        default_coop_sync: #coop, priority_groups: #priority })
}

/// Render the whole `HostConfig { ... }` struct literal that
/// `host_default()` returns.
/// Render the baked host configuration after validating its bounded policies.
///
/// # Errors
/// Rejects invalid cookie, sync, privacy, status-banner, or resource-limit values.
pub fn render_host_config(parsed: &HostDefaultRaw) -> Result<String, RenderError> {
    Ok(host_config_tokens(parsed)?.to_string())
}

fn host_config_tokens(parsed: &HostDefaultRaw) -> Result<TokenStream, RenderError> {
    usersync::validate_host_cookie(&parsed.host_cookie).map_err(RenderError::UserSync)?;
    if let Some(sync) = &parsed.user_sync {
        usersync::validate_user_sync(sync, cfg!(feature = "insecure-bidders"))
            .map_err(RenderError::UserSync)?;
    }
    if parsed.status_response.len() > limits::MAX_STATUS_BANNER_BYTES {
        return Err(RenderError::StatusResponseTooLarge {
            bytes: parsed.status_response.len(),
        });
    }
    let enabled = parsed.gdpr.enabled;
    let value = if gdpr_default_signal(&parsed.gdpr.default_value)? {
        "1"
    } else {
        "0"
    };
    let host_vendor_id = host_vendor_id_tokens(parsed.gdpr.host_vendor_id)?;
    let tcf2 = tcf2_tokens(&parsed.gdpr.tcf2)?;
    let publishers = string_vec_tokens(&parsed.gdpr.non_standard_publishers);
    let gdpr = quote!(GdprConfig { enabled: #enabled, default_value: #value.to_owned(),
        host_vendor_id: #host_vendor_id, tcf2: #tcf2, non_standard_publishers: #publishers });
    let ccpa_enforce = parsed.ccpa.enforce;
    let lmt_enforce = parsed.lmt.enforce;
    let ccpa = quote!(CcpaConfig { enforce: #ccpa_enforce });
    let lmt = quote!(LmtConfig { enforce: #lmt_enforce });
    let host_cookie = render_host_cookie(&parsed.host_cookie);
    let status_render = if parsed.status_response.is_empty() {
        // Empty status_response -> /status returns 204 No Content.
        quote!(String::new())
    } else {
        let response = &parsed.status_response;
        owned_string(response)
    };

    let user_sync = parsed.user_sync.as_ref().map_or_else(
        || quote!(None),
        |sync| {
            let service = render_host_user_sync(sync);
            quote!(Some(#service))
        },
    );
    let resource_limits = resource_limits_tokens(&parsed.resource_limits);
    let aliases = string_map_tokens(&parsed.aliases);
    Ok(
        quote!(HostConfig { gdpr: #gdpr, ccpa: #ccpa, lmt: #lmt, host_cookie: #host_cookie,
            user_sync: #user_sync, status_response: #status_render, resource_limits: #resource_limits,
            aliases: #aliases,
        }),
    )
}

/// Render the `ResourceLimits { ... }` sub-struct.
/// Render the baked resource limits.
///
/// # Errors
/// No errors are produced by this token renderer.
pub fn render_resource_limits(limits: &ResourceLimitsRaw) -> Result<String, RenderError> {
    Ok(resource_limits_tokens(limits).to_string())
}

fn resource_limits_tokens(limits: &ResourceLimitsRaw) -> TokenStream {
    let default_tmax = int_i64_tokens(limits.default_tmax_ms);
    let max_tmax = int_i64_tokens(limits.max_tmax_ms);
    let response = int_usize_literal(limits.max_bidder_response_bytes);
    let bidders = int_usize_literal(limits.max_bidders_per_auction);
    let requests = int_usize_literal(limits.max_outbound_requests);
    let raw = int_u64_literal(limits.max_outbound_raw_bytes);
    let request = int_u64_literal(limits.max_outbound_request_bytes);
    let transport = int_u64_literal(limits.max_transport_bytes);
    let decoded = int_u64_literal(limits.max_decoded_bytes);
    let nodes = int_u64_literal(u64::from(limits.max_parsed_nodes));
    quote!(ResourceLimits {
        default_tmax_ms: #default_tmax, max_tmax_ms: #max_tmax,
        max_bidder_response_bytes: #response, max_bidders_per_auction: #bidders,
        max_outbound_requests: #requests, max_outbound_raw_bytes: #raw,
        max_outbound_request_bytes: #request, max_transport_bytes: #transport,
        max_decoded_bytes: #decoded, max_parsed_nodes: #nodes,
    })
}

/// Render the `AccountConfig { ... }` struct literal that
/// `account_default()` returns.
/// Render the baked account configuration.
///
/// # Errors
/// Rejects invalid cookie-sync, privacy, or enforcement configuration.
pub fn render_account_config(parsed: &AccountRaw) -> Result<String, RenderError> {
    usersync::validate_account_cookie_sync(&parsed.cookie_sync).map_err(RenderError::UserSync)?;
    let truncate = option_tokens(
        parsed
            .truncate_target_attribute
            .map(|n| typed_integer(&n.to_string(), "usize")),
    );
    let rounding = &parsed.bid_rounding;
    let prefix = &parsed.targeting_prefix;

    let disabled = parsed.disabled;
    let debug = parsed.debug_allow;
    let gdpr = account_gdpr_tokens(&parsed.gdpr)?;
    let cookie_sync = render_account_cookie_sync(&parsed.cookie_sync);
    let ccpa = render_account_ccpa(&parsed.ccpa);
    let privacy = account_privacy_tokens(&parsed.privacy)?;
    Ok(quote!(AccountConfig {
        disabled: #disabled, debug_allow: #debug, bid_rounding: #rounding.to_owned(),
        targeting_prefix: #prefix.to_owned(), truncate_target_attribute: #truncate,
        gdpr: #gdpr, cookie_sync: #cookie_sync, ccpa: #ccpa, privacy: #privacy,
    })
    .to_string())
}

fn tristate_tokens(value: types::TriState) -> TokenStream {
    match value {
        types::TriState::Unset => quote!(TriState::Unset),
        types::TriState::Enabled => quote!(TriState::Enabled),
        types::TriState::Disabled => quote!(TriState::Disabled),
    }
}

fn render_account_ccpa(cfg: &AccountCcpaConfig) -> TokenStream {
    let channels = &cfg.channel_enabled;
    let enabled = tristate_tokens(cfg.enabled);
    let web = tristate_tokens(channels.web);
    let app = tristate_tokens(channels.app);
    let amp = tristate_tokens(channels.amp);
    let video = tristate_tokens(channels.video);
    let dooh = tristate_tokens(channels.dooh);
    quote!(AccountCcpaConfig { enabled: #enabled, channel_enabled: ChannelEnabled {
        web: #web, app: #app, amp: #amp, video: #video, dooh: #dooh
    }})
}

/// Enforcement-map key ranges. MUST match the runtime
/// `PURPOSE_ID_RANGE` / `SPECIAL_FEATURE_ID_RANGE` in
/// `auctioneer-config-data/src`. Purposes 1..=10, special features 1..=12.
const PURPOSE_ID_RANGE: RangeInclusive<u8> = 1..=10;
const SPECIAL_FEATURE_ID_RANGE: RangeInclusive<u8> = 1..=12;

/// Render the `Tcf2Config { ... }` struct literal for the host's
/// `gdpr.tcf2` block.
/// Render the host TCF policy.
///
/// # Errors
/// Rejects invalid purpose or special-feature identifiers and enforcement values.
pub fn render_tcf2(cfg: &Tcf2ConfigRaw) -> Result<String, RenderError> {
    Ok(tcf2_tokens(cfg)?.to_string())
}

fn tcf2_tokens(cfg: &Tcf2ConfigRaw) -> Result<TokenStream, RenderError> {
    // Reject out-of-range keys (0 or overflow) BEFORE emitting the
    // baked maps, so a mistyped `[gdpr.tcf2.purposes.0]` or `.99`
    // fails the build instead of baking a dead entry.
    for &id in cfg.purposes.keys() {
        if !PURPOSE_ID_RANGE.contains(&id) {
            return Err(RenderError::PurposeIdOutOfRange { id });
        }
    }
    for &id in cfg.special_features.keys() {
        if !SPECIAL_FEATURE_ID_RANGE.contains(&id) {
            return Err(RenderError::SpecialFeatureIdOutOfRange { id });
        }
    }
    let enabled = cfg.enabled;
    let purposes = purpose_map_tokens(&cfg.purposes)?;
    let special_features = special_feature_map_tokens(&cfg.special_features)?;
    let treatment = render_treatment(cfg.purpose_one_treatment);
    Ok(quote!(Tcf2Config { enabled: #enabled, purposes: #purposes,
        special_features: #special_features, purpose_one_treatment: #treatment }))
}

/// Render the `AccountGdprConfig { ... }` struct literal. Runs the
/// SAME two key-range loops as `render_tcf2` before emitting the
/// account maps — the range constants above are shared. Host and
/// account purposes render identically (no `is_host` distinction)
/// since `allow_li_transparency` is not a config field.
/// Render the account GDPR policy.
///
/// # Errors
/// Rejects invalid enforcement-map identifiers and values.
pub fn render_account_gdpr(cfg: &AccountGdprConfigRaw) -> Result<String, RenderError> {
    Ok(account_gdpr_tokens(cfg)?.to_string())
}

fn account_gdpr_tokens(cfg: &AccountGdprConfigRaw) -> Result<TokenStream, RenderError> {
    for &id in cfg.purposes.keys() {
        if !PURPOSE_ID_RANGE.contains(&id) {
            return Err(RenderError::PurposeIdOutOfRange { id });
        }
    }
    for &id in cfg.special_features.keys() {
        if !SPECIAL_FEATURE_ID_RANGE.contains(&id) {
            return Err(RenderError::SpecialFeatureIdOutOfRange { id });
        }
    }
    let enabled = tristate_tokens(parse_tristate(&cfg.enabled)?);
    let channel_enabled = channel_enabled_tokens(&cfg.channel_enabled)?;
    let purposes = purpose_map_tokens(&cfg.purposes)?;
    let special_features = special_feature_map_tokens(&cfg.special_features)?;
    let vendors = string_vec_tokens(&cfg.basic_enforcement_vendors);
    let treatment = render_treatment_override(cfg.purpose_one_treatment);
    Ok(
        quote!(AccountGdprConfig { enabled: #enabled, channel_enabled: #channel_enabled,
        purposes: #purposes, special_features: #special_features,
        basic_enforcement_vendors: #vendors, purpose_one_treatment: #treatment }),
    )
}

/// The privacy renderer — validates the 32/128 range HERE (runtime
/// types can't derive `Validate`; the codegen renderer is the single
/// owner of the bound) and emits the struct literal. Absent block →
/// family defaults 24/56.
/// Render account privacy and activity policies.
///
/// # Errors
/// Rejects invalid IP keep-bit counts or activity rules.
pub fn render_account_privacy(cfg: &AccountPrivacyConfigRaw) -> Result<String, RenderError> {
    Ok(account_privacy_tokens(cfg)?.to_string())
}

fn account_privacy_tokens(cfg: &AccountPrivacyConfigRaw) -> Result<TokenStream, RenderError> {
    for (family, max, bits) in [
        ("ipv4", 32_u8, cfg.ipv4.anon_keep_bits),
        ("ipv6", 128_u8, cfg.ipv6.anon_keep_bits),
    ] {
        if bits > max {
            return Err(RenderError::IpKeepBitsOutOfRange {
                family,
                value: bits,
                max,
            });
        }
    }
    let v4 = proc_macro2::Literal::u8_unsuffixed(cfg.ipv4.anon_keep_bits);
    let v6 = proc_macro2::Literal::u8_unsuffixed(cfg.ipv6.anon_keep_bits);
    let activities = render_activities(&cfg.allowactivities)?;
    Ok(
        quote!(AccountPrivacyConfig { ipv4: IpAnonConfig { anon_keep_bits: #v4 },
        ipv6: IpAnonConfig { anon_keep_bits: #v6 }, allowactivities: #activities }),
    )
}

fn render_activities(
    configs: &BTreeMap<activity::Activity, activity::ActivityConfig>,
) -> Result<TokenStream, RenderError> {
    activity::validate_activities(configs).map_err(RenderError::Activity)?;
    if configs.is_empty() {
        return Ok(quote!({
            use std::sync::Arc;
            Arc::default()
        }));
    }
    let mut entries = Vec::new();
    let mut imports = vec![quote!(Activity), quote!(ActivityConfig)];
    for (key, config) in configs {
        let mut rules = Vec::new();
        for rule in config.rules.iter() {
            imports.push(quote!(Rule));
            let condition = rule.condition.as_ref().map_or_else(
                || quote!(None),
                |condition| {
                    imports.push(quote!(Condition));
                    if !condition.component_type.is_empty() {
                        imports.push(quote!(ComponentType));
                    }
                    let kinds = condition
                        .component_type
                        .iter()
                        .map(|kind| match kind {
                            activity::ComponentType::Bidder => quote!(ComponentType::Bidder),
                            activity::ComponentType::Analytics => quote!(ComponentType::Analytics),
                            activity::ComponentType::Rtd => quote!(ComponentType::Rtd),
                            activity::ComponentType::General => quote!(ComponentType::General),
                        });
                    let names = string_vec_tokens(&condition.component_name);
                    quote!(Some(Condition { component_name: #names, component_type: vec![#(#kinds),*] }))
                },
            );
            let allow = rule.allow;
            rules.push(quote!(Rule { allow: #allow, condition: #condition }));
        }
        let key_literal = match key {
            activity::Activity::FetchBids => quote!(Activity::FetchBids),
            activity::Activity::TransmitUfpd => quote!(Activity::TransmitUfpd),
            activity::Activity::TransmitPreciseGeo => quote!(Activity::TransmitPreciseGeo),
            activity::Activity::TransmitTid => quote!(Activity::TransmitTid),
            activity::Activity::SyncUser => quote!(Activity::SyncUser),
            activity::Activity::ReportAnalytics => quote!(Activity::ReportAnalytics),
            activity::Activity::EnrichUfpd => quote!(Activity::EnrichUfpd),
            activity::Activity::TransmitUniqueRequestIds => {
                quote!(Activity::TransmitUniqueRequestIds)
            }
        };
        let default = option_tokens(config.default);
        entries.push(quote!((#key_literal, ActivityConfig { default: #default, rules: Arc::from(vec![#(#rules),*]) })));
    }
    imports.sort_unstable_by_key(ToString::to_string);
    imports.dedup_by(|left, right| left.to_string() == right.to_string());
    Ok(
        quote!({ use std::sync::Arc; use crate::activity::{#(#imports),*};
        Arc::new(BTreeMap::from([#(#entries),*])) }),
    )
}

/// `render_purpose_enforcement` takes NO `is_host` flag and no
/// `purpose_id` — `allow_li_transparency` is not a config field on
/// either side, so there is nothing host-specific to reject and
/// host/account purposes render identically.
/// Render one purpose's enforcement policy.
///
/// # Errors
/// Rejects unknown enforcement algorithms or tri-state values.
pub fn render_purpose_enforcement(p: &PurposeEnforcementRaw) -> Result<String, RenderError> {
    Ok(purpose_enforcement_tokens(p)?.to_string())
}

fn purpose_enforcement_tokens(p: &PurposeEnforcementRaw) -> Result<TokenStream, RenderError> {
    // `enforce_algo` is `"full" | "basic"` (an `EnforceAlgo`, NOT a
    // tri-state) — route it through `render_enforce_algo`, NOT
    // `render_tristate`. Only `enforce_purpose` / `enforce_vendors`
    // are tri-states.
    let algo = match parse_enforce_algo(&p.enforce_algo)? {
        types::EnforceAlgo::Full => quote!(EnforceAlgo::Full),
        types::EnforceAlgo::Basic => quote!(EnforceAlgo::Basic),
    };
    let purpose = tristate_tokens(parse_tristate(&p.enforce_purpose)?);
    let vendors = tristate_tokens(parse_tristate(&p.enforce_vendors)?);
    let exceptions = p.vendor_exceptions.iter().map(|value| owned_string(value));
    Ok(
        quote!(PurposeEnforcement { enforce_algo: #algo, enforce_purpose: #purpose,
        enforce_vendors: #vendors, vendor_exceptions: vec![#(#exceptions),*] }),
    )
}

/// Render one `SpecialFeatureEnforcement { ... }` literal — the
/// tri-state `enforce` plus `vendor_exceptions` (no `enforce_algo`
/// slot; SF1 has no A.2/A.3 split).
/// Render one special feature's enforcement policy.
///
/// # Errors
/// Rejects unknown tri-state values.
pub fn render_special_feature_enforcement(
    sf: &SpecialFeatureEnforcementRaw,
) -> Result<String, RenderError> {
    Ok(special_feature_enforcement_tokens(sf)?.to_string())
}

fn special_feature_enforcement_tokens(
    sf: &SpecialFeatureEnforcementRaw,
) -> Result<TokenStream, RenderError> {
    let enforce = tristate_tokens(parse_tristate(&sf.enforce)?);
    let exceptions = sf.vendor_exceptions.iter().map(|value| owned_string(value));
    Ok(quote!(SpecialFeatureEnforcement { enforce: #enforce,
        vendor_exceptions: vec![#(#exceptions),*] }))
}

/// Render the five-channel `ChannelEnabled { ... }` literal; every
/// field is a tri-state.
/// Render channel-specific enforcement switches.
///
/// # Errors
/// Rejects unknown tri-state values.
pub fn render_channel_enabled(channels: &ChannelEnabledRaw) -> Result<String, RenderError> {
    Ok(channel_enabled_tokens(channels)?.to_string())
}

fn channel_enabled_tokens(channels: &ChannelEnabledRaw) -> Result<TokenStream, RenderError> {
    let web = tristate_tokens(parse_tristate(&channels.web)?);
    let app = tristate_tokens(parse_tristate(&channels.app)?);
    let amp = tristate_tokens(parse_tristate(&channels.amp)?);
    let video = tristate_tokens(parse_tristate(&channels.video)?);
    let dooh = tristate_tokens(parse_tristate(&channels.dooh)?);
    Ok(quote!(ChannelEnabled { web: #web, app: #app, amp: #amp, video: #video, dooh: #dooh }))
}

/// Render a `BTreeMap<u8, PurposeEnforcement>` as an inline
/// `{ let mut m = BTreeMap::new(); ... m }` block (the shape
/// `render_string_string_btreemap` emits). Empty maps collapse to
/// `BTreeMap::new()`.
/// Render a purpose enforcement map.
///
/// # Errors
/// Rejects out-of-range purpose identifiers and invalid enforcement values.
pub fn render_purpose_map(
    map: &BTreeMap<u8, PurposeEnforcementRaw>,
) -> Result<String, RenderError> {
    Ok(purpose_map_tokens(map)?.to_string())
}

fn purpose_map_tokens(
    map: &BTreeMap<u8, PurposeEnforcementRaw>,
) -> Result<TokenStream, RenderError> {
    if map.is_empty() {
        return Ok(quote!(BTreeMap::new()));
    }
    let entries = map
        .iter()
        .map(|(id, purpose)| {
            let id_literal = typed_integer(&id.to_string(), "u8");
            let literal = purpose_enforcement_tokens(purpose)?;
            Ok::<_, RenderError>(quote!(m.insert(#id_literal, #literal);))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote!({ let mut m = BTreeMap::new(); #(#entries)* m }))
}

/// `render_purpose_map`'s sibling for the special-features map.
/// Render a special-feature enforcement map.
///
/// # Errors
/// Rejects out-of-range special-feature identifiers and invalid enforcement values.
pub fn render_special_feature_map(
    map: &BTreeMap<u8, SpecialFeatureEnforcementRaw>,
) -> Result<String, RenderError> {
    Ok(special_feature_map_tokens(map)?.to_string())
}

fn special_feature_map_tokens(
    map: &BTreeMap<u8, SpecialFeatureEnforcementRaw>,
) -> Result<TokenStream, RenderError> {
    if map.is_empty() {
        return Ok(quote!(BTreeMap::new()));
    }
    let entries = map
        .iter()
        .map(|(id, feature)| {
            let id_literal = typed_integer(&id.to_string(), "u8");
            let literal = special_feature_enforcement_tokens(feature)?;
            Ok::<_, RenderError>(quote!(m.insert(#id_literal, #literal);))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote!({ let mut m = BTreeMap::new(); #(#entries)* m }))
}

/// Dedicated renderer for the two-value `enforce_algo` domain.
/// `render_tristate` would reject BOTH valid values ("full" / "basic"
/// are not "unset"/"enabled"/"disabled").
///
/// # Errors
/// Rejects values other than `full` and `basic`.
pub fn render_enforce_algo(s: &str) -> Result<&'static str, RenderError> {
    Ok(match parse_enforce_algo(s)? {
        types::EnforceAlgo::Full => "EnforceAlgo::Full",
        types::EnforceAlgo::Basic => "EnforceAlgo::Basic",
    })
}

fn parse_enforce_algo(s: &str) -> Result<types::EnforceAlgo, RenderError> {
    match s {
        "full" => Ok(types::EnforceAlgo::Full),
        "basic" => Ok(types::EnforceAlgo::Basic),
        other => Err(RenderError::InvalidEnforceAlgo(other.to_owned())),
    }
}

/// `host_vendor_id` renders into the baked
/// `GdprConfig.host_vendor_id: Option<u32>`. The raw DTO is `u32`
/// because TOML has no `u16` and 0 is a valid "skip host-company GDPR
/// check" sentinel (spec §7.9). The renderer REJECTS anything above
/// `u16::MAX` — it does NOT clamp into `u16` (`u32 as u16` would
/// silently wrap `65536 → 0`). Called from `render_host_config`; the
/// baked default is always a concrete value, so it emits `Some(..)`,
/// never `None`.
///
/// # Errors
/// Rejects identifiers greater than `u16::MAX`.
pub fn render_host_vendor_id(value: u32) -> Result<String, RenderError> {
    Ok(host_vendor_id_tokens(value)?.to_string())
}

fn host_vendor_id_tokens(value: u32) -> Result<TokenStream, RenderError> {
    if value > u32::from(u16::MAX) {
        return Err(RenderError::HostVendorIdOutOfRange { value });
    }
    let literal = typed_integer(&value.to_string(), "u32");
    Ok(quote!(Some(#literal)))
}

/// Fail-closed tri-state renderer: rejects unknown values instead of
/// mapping them to `TriState::Unset`, so a typo in a baked TOML fails
/// the build instead of silently disabling enforcement.
///
/// # Errors
/// Rejects values other than `unset`, `enabled`, and `disabled`.
pub fn render_tristate(s: &str) -> Result<&'static str, RenderError> {
    Ok(match parse_tristate(s)? {
        types::TriState::Unset => "TriState::Unset",
        types::TriState::Enabled => "TriState::Enabled",
        types::TriState::Disabled => "TriState::Disabled",
    })
}

fn parse_tristate(s: &str) -> Result<types::TriState, RenderError> {
    match s {
        "unset" => Ok(types::TriState::Unset),
        "enabled" => Ok(types::TriState::Enabled),
        "disabled" => Ok(types::TriState::Disabled),
        other => Err(RenderError::InvalidTristate(other.to_owned())),
    }
}

/// `gdpr.default_value` is the fallback consent-scope signal; only
/// literal `"0"` / `"1"` are legal.
///
/// # Errors
/// Rejects values other than `0` and `1`.
pub fn render_gdpr_default_value(s: &str) -> Result<&'static str, RenderError> {
    Ok(if gdpr_default_signal(s)? {
        "\"1\""
    } else {
        "\"0\""
    })
}

fn gdpr_default_signal(s: &str) -> Result<bool, RenderError> {
    match s {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(RenderError::InvalidGdprDefaultValue(other.to_owned())),
    }
}

/// Render a `Vec<String>` field: empty → `Vec::new()`, otherwise a
/// `vec![..]` of owned `String`s.
#[must_use]
pub fn render_string_vec(items: &[String]) -> String {
    string_vec_tokens(items).to_string()
}

/// Render a `BTreeMap<String, String>` as an inline `{ let mut m =
/// BTreeMap::new(); m.insert(...); m }` block. Empty maps collapse
/// to `BTreeMap::new()`.
///
/// # Errors
/// No errors are produced by this token renderer.
pub fn render_string_string_btreemap(
    map: &BTreeMap<String, String>,
) -> Result<String, RenderError> {
    Ok(string_map_tokens(map).to_string())
}

fn string_map_tokens(map: &BTreeMap<String, String>) -> TokenStream {
    if map.is_empty() {
        return quote!(BTreeMap::new());
    }
    let entries = map
        .iter()
        .map(|(key, value)| quote!(m.insert(#key.to_owned(), #value.to_owned());));
    quote!({ let mut m = BTreeMap::new(); #(#entries)* m })
}

/// Render an integer with `_` thousands-separators so the generated
/// code clears clippy's `unreadable_literal` restriction (any literal
/// `>= 100_000` without separators trips the lint at the call site).
#[must_use]
pub fn render_int_i64(value: i64) -> String {
    int_i64_tokens(value).to_string()
}

fn int_i64_tokens(value: i64) -> TokenStream {
    let abs_str = value.unsigned_abs().to_string();
    let chunks: Vec<&str> = abs_str
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| str::from_utf8(chunk).unwrap_or(""))
        .collect();
    let body = chunks.join("_");
    let literal = syn::LitInt::new(&body, Span::call_site());
    if value < 0 {
        quote!(-#literal)
    } else {
        quote!(#literal)
    }
}

#[must_use]
pub fn render_int_usize(value: usize) -> String {
    int_usize_literal(value).into_token_stream().to_string()
}

fn int_usize_literal(value: usize) -> syn::LitInt {
    // Power-of-two literals (e.g. 262_144) trip clippy's
    // `decimal_literal_representation` restriction at the codegen
    // call site. Render those as hex; everything else gets the
    // thousands-separator decimal form.
    if value >= 1024 && value.is_power_of_two() {
        return syn::LitInt::new(&format!("{value:#x}"), Span::call_site());
    }
    let abs_str = value.to_string();
    let chunks: Vec<&str> = abs_str
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| str::from_utf8(chunk).unwrap_or(""))
        .collect();
    syn::LitInt::new(&chunks.join("_"), Span::call_site())
}

/// The target-width-independent form of [`render_int_usize`], with
/// grouped hexadecimal digits for the larger byte caps.
#[must_use]
pub fn render_int_u64(value: u64) -> String {
    int_u64_literal(value).into_token_stream().to_string()
}

fn int_u64_literal(value: u64) -> syn::LitInt {
    syn::LitInt::new(&u64_digits(value), Span::call_site())
}

fn u64_digits(value: u64) -> String {
    let (digits, prefix, group_size) = if value >= 1024 && value.is_power_of_two() {
        (format!("{value:x}"), "0x", 4_usize)
    } else {
        (value.to_string(), "", 3_usize)
    };
    let chunks: Vec<&str> = digits
        .as_bytes()
        .rchunks(group_size)
        .rev()
        .map(|chunk| str::from_utf8(chunk).unwrap_or(""))
        .collect();
    let body = chunks.join("_");
    [prefix, &body].concat()
}

/// Render the `hostAllowlist` TOML field into the codegen output.
/// Empty / missing -> `None`; non-empty -> `Some(vec![...])` of owned
/// `String`s. Entries themselves are validated in `build.rs` before
/// this runs, so by the time we render we know each value is a
/// non-empty hostname.
#[must_use]
pub fn render_host_allowlist(allowlist: Option<&[String]>) -> String {
    host_allowlist_tokens(allowlist).to_string()
}

fn host_allowlist_tokens(allowlist: Option<&[String]>) -> TokenStream {
    let Some(entries) = allowlist else {
        return quote!(None);
    };
    if entries.is_empty() {
        return quote!(None);
    }
    let items = entries.iter().map(|entry| owned_string(entry));
    quote!(Some(vec![#(#items),*]))
}

/// Render the `use crate::types::{...};` line at the top of the
/// bidder section, listing only the types the loaded fixtures
/// actually reference. Downstream builds run under `-D warnings`, so
/// an unused import would break the build for minimal harness
/// fixtures.
///
/// `BidderInfo` / `Capabilities` / `SyncerConfig` / `OpenRtbCapabilities`
/// are emitted per bidder unconditionally (their render functions
/// produce the struct literal even when the underlying TOML is
/// empty). `Compression`, `MediaType`, and `SyncerEndpoint` appear
/// only when a bidder actually supplies matching content.
#[must_use]
pub fn render_bidder_type_imports(bidders: &[(String, BidderInfoRaw)]) -> String {
    let infos: Vec<_> = bidders.iter().map(|(_, info)| info).collect();
    let all = bidder_type_imports(&infos);
    let portable = bidder_type_imports(
        &infos
            .into_iter()
            .filter(|info| !info.native_only)
            .collect::<Vec<_>>(),
    );
    let identifiers = |names: Vec<&str>| {
        names
            .into_iter()
            .map(|name| syn::Ident::new(name, proc_macro2::Span::call_site()))
            .collect::<Vec<_>>()
    };
    let native = identifiers(
        all.iter()
            .copied()
            .filter(|name| !portable.contains(name))
            .collect(),
    );
    let portable_identifiers = identifiers(portable);
    let native_imports = (!native.is_empty()).then(|| {
        let gate = publication_gate(true, None);
        quote!(#gate use crate::types::{#(#native),*};)
    });
    quote!(use crate::types::{#(#portable_identifiers),*}; #native_imports).to_string()
}

fn bidder_type_imports(bidders: &[&BidderInfoRaw]) -> Vec<&'static str> {
    let uses_compression = bidders
        .iter()
        .any(|info| info.endpoint_compression.as_deref().is_some());
    let uses_media_type = bidders.iter().any(|info| {
        [
            info.capabilities.site.as_ref(),
            info.capabilities.app.as_ref(),
            info.capabilities.dooh.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|shape| {
            shape
                .media_types
                .iter()
                .any(|name| matches!(name.as_str(), "banner" | "video" | "native" | "audio"))
        })
    });
    let uses_syncer_endpoint = bidders
        .iter()
        .any(|info| info.user_sync.iframe.is_some() || info.user_sync.redirect.is_some());
    let mut imports = vec!["BidderInfo"];
    if !bidders.is_empty() {
        imports.extend([
            "Capabilities",
            "OpenRtbCapabilities",
            "SkipWhen",
            "SyncerConfig",
        ]);
    }
    if uses_compression {
        imports.push("Compression");
    }
    if uses_media_type {
        imports.push("MediaType");
    }
    if uses_syncer_endpoint {
        imports.push("SyncerEndpoint");
    }
    if bidders
        .iter()
        .any(|info| !info.user_sync.supports.is_empty())
    {
        imports.push("SyncType");
    }
    if bidders
        .iter()
        .any(|info| info.user_sync.default_response.is_some())
    {
        imports.push("SyncResponseFormat");
    }
    imports.sort_unstable();
    imports
}

/// Conditional `use crate::types::{...}` list for the emitted
/// `host_default()` section. `PurposeEnforcement` / `EnforceAlgo` /
/// `TriState` / `SpecialFeatureEnforcement` appear in the literal only
/// when the corresponding baked map is non-empty; importing them
/// unconditionally would trip `unused_imports` under `-D warnings`
/// for the default (empty-map) TOML, the same failure mode
/// `render_bidder_type_imports` guards against.
#[must_use]
pub fn host_default_type_imports(parsed: &HostDefaultRaw) -> Vec<&'static str> {
    let mut imports = vec![
        "HostConfig",
        "GdprConfig",
        "CcpaConfig",
        "LmtConfig",
        "HostCookieConfig",
        "PurposeOneTreatment",
        "ResourceLimits",
        "Tcf2Config",
    ];
    if parsed.user_sync.is_some() {
        imports.extend(["UserSyncConfig", "CooperativeConfig"]);
    }
    if parsed.host_cookie.optout_cookie.is_some() {
        imports.push("OptoutCookie");
    }
    if !parsed.gdpr.tcf2.purposes.is_empty() {
        imports.extend(["PurposeEnforcement", "EnforceAlgo", "TriState"]);
    }
    if !parsed.gdpr.tcf2.special_features.is_empty() {
        imports.extend(["SpecialFeatureEnforcement", "TriState"]);
    }
    imports.sort_unstable();
    imports.dedup();
    imports
}

/// Conditional import list for the emitted `account_default()`
/// section. The generated file is ONE module, so any type the host
/// section already imported must be skipped here — a second
/// `use crate::types::TriState;` would be an E0252 duplicate-import
/// error in the generated code.
#[must_use]
pub fn account_default_type_imports(
    parsed: &AccountRaw,
    already_imported: &[&'static str],
) -> Vec<&'static str> {
    let mut imports = vec![
        "AccountConfig",
        "AccountGdprConfig",
        "AccountCcpaConfig",
        "ChannelEnabled",
        "TriState",
        "AccountPrivacyConfig",
        "IpAnonConfig",
        "AccountCookieSyncConfig",
        "PurposeOneTreatmentOverride",
    ];
    if !parsed.gdpr.purposes.is_empty() {
        imports.extend(["PurposeEnforcement", "EnforceAlgo"]);
    }
    if !parsed.gdpr.special_features.is_empty() {
        imports.push("SpecialFeatureEnforcement");
    }
    imports.sort_unstable();
    imports.dedup();
    imports.retain(|ty| !already_imported.contains(ty));
    imports
}

#[must_use]
pub fn render_capabilities(cap: &CapabilitiesRaw) -> String {
    capabilities_tokens(cap).to_string()
}

fn capabilities_tokens(cap: &CapabilitiesRaw) -> TokenStream {
    let site = media_types_tokens(cap.site.as_ref());
    let app = media_types_tokens(cap.app.as_ref());
    let dooh = media_types_tokens(cap.dooh.as_ref());
    quote!(Capabilities { site: #site, app: #app, dooh: #dooh })
}

#[must_use]
pub fn render_media_types(shape_opt: Option<&CapabilityShapeRaw>) -> String {
    media_types_tokens(shape_opt).to_string()
}

fn media_types_tokens(shape_opt: Option<&CapabilityShapeRaw>) -> TokenStream {
    let Some(shape) = shape_opt else {
        return quote!(Vec::new());
    };
    let items: Vec<_> = shape
        .media_types
        .iter()
        .filter_map(|name| match name.as_str() {
            "banner" => Some(quote!(MediaType::Banner)),
            "video" => Some(quote!(MediaType::Video)),
            "native" => Some(quote!(MediaType::Native)),
            "audio" => Some(quote!(MediaType::Audio)),
            _ => None,
        })
        .collect();
    if items.is_empty() {
        quote!(Vec::new())
    } else {
        quote!(vec![#(#items),*])
    }
}

#[must_use]
pub fn render_user_sync(sync: &UserSyncRaw) -> String {
    user_sync_tokens(sync).to_string()
}

fn user_sync_tokens(sync: &UserSyncRaw) -> TokenStream {
    let key = render_option_string(sync.key.as_ref());
    let iframe = syncer_endpoint_tokens(sync.iframe.as_ref());
    let redirect = syncer_endpoint_tokens(sync.redirect.as_ref());
    let supports = if sync.supports.is_empty() {
        quote!(Vec::new())
    } else {
        let items = sync.supports.iter().map(|entry| match entry {
            usersync::SyncType::Iframe => quote!(SyncType::Iframe),
            usersync::SyncType::Redirect => quote!(SyncType::Redirect),
        });
        quote!(vec![#(#items),*])
    };
    let enabled = sync.enabled.unwrap_or(true);
    let gdpr = sync.skip_when.gdpr;
    let gpp_sid = string_vec_tokens(&sync.skip_when.gpp_sid);
    let default_response = option_tokens(sync.default_response.map(|response| match response {
        usersync::SyncResponseFormat::Image => quote!(SyncResponseFormat::Image),
        usersync::SyncResponseFormat::Blank => quote!(SyncResponseFormat::Blank),
    }));
    quote!(SyncerConfig { key: #key, iframe: #iframe, redirect: #redirect,
        supports: #supports, enabled: #enabled,
        skip_when: SkipWhen { gdpr: #gdpr, gpp_sid: #gpp_sid }, default_response: #default_response })
}

#[must_use]
pub fn render_syncer_endpoint(endpoint_opt: Option<&SyncerEndpointRaw>) -> String {
    syncer_endpoint_tokens(endpoint_opt).to_string()
}

fn syncer_endpoint_tokens(endpoint_opt: Option<&SyncerEndpointRaw>) -> TokenStream {
    let Some(endpoint) = endpoint_opt else {
        return quote!(None);
    };
    let user_macro = render_option_string(
        endpoint
            .user_macro
            .as_ref()
            .filter(|value| !value.is_empty()),
    );
    let url = &endpoint.url;
    quote!(Some(SyncerEndpoint { url: #url.to_owned(), user_macro: #user_macro }))
}

#[must_use]
pub fn render_openrtb(open_rtb: &OpenRtbRaw) -> String {
    openrtb_tokens(open_rtb).to_string()
}

fn openrtb_tokens(open_rtb: &OpenRtbRaw) -> TokenStream {
    let raw_version = match &open_rtb.version {
        Some(toml::Value::String(name)) => Some(owned_string(name)),
        Some(toml::Value::Integer(number)) => {
            let rendered = number.to_string();
            Some(owned_string(&rendered))
        }
        Some(toml::Value::Float(number)) => {
            let rendered = number.to_string();
            Some(owned_string(&rendered))
        }
        Some(_) | None => None,
    };
    let version = option_tokens(raw_version);
    let gpp = open_rtb.gpp_supported.unwrap_or(false);
    quote!(OpenRtbCapabilities { version: #version, gpp_supported: #gpp })
}

#[cfg(test)]
mod binding_input_tests {
    use super::BidderInfoRaw;
    use crate::types::BidderInfo;
    use std::collections::BTreeSet;

    #[test]
    fn compiled_activation_fields_are_optional_and_rendered_without_changing_safety() {
        let mut raw: BidderInfoRaw =
            toml::from_str("implementation = 'fake_owner'\nendpoint = 'https://example.test'\n")
                .unwrap();
        assert_eq!(raw.alias_of, None);
        assert_eq!(raw.default_enabled, None);
        raw.alias_of = Some("fake_base".to_owned());
        raw.default_enabled = Some(false);
        let generated = super::render_bidder_info("fake_alias", &raw).unwrap();
        let syntax: syn::Expr = syn::parse_str(&generated).unwrap();
        let rendered = quote::quote!(#syntax).to_string();
        assert!(
            rendered.contains(&quote::quote!(alias_of: Some("fake_base".to_owned())).to_string())
        );
        assert!(rendered.contains(&quote::quote!(default_enabled: Some(false)).to_string()));
        assert!(rendered.contains(&quote::quote!(disabled: false).to_string()));
        let legacy = BidderInfo::default();
        let json = serde_json::to_value(&legacy).unwrap();
        assert!(json.get("alias_of").is_none());
        assert!(json.get("default_enabled").is_none());
        let round_tripped: BidderInfo = serde_json::from_value(json).unwrap();
        assert_eq!(round_tripped.alias_of, None);
        assert_eq!(round_tripped.default_enabled, None);
        let parsed_raw: BidderInfoRaw = toml::from_str("implementation = 'fake_owner'\nendpoint = 'https://example.test'\naliasOf = 'fake_base'\ndefaultEnabled = false\n").unwrap();
        assert_eq!(parsed_raw.alias_of.as_deref(), Some("fake_base"));
        assert_eq!(parsed_raw.default_enabled, Some(false));
    }

    #[test]
    fn public_binding_target_and_feature_gates_leave_all_declarations_intact() {
        let mut native: BidderInfoRaw = toml::from_str("implementation = 'fake_owner'\nrequired_feature = 'experimental'\nendpoint = 'https://example.test'\n").unwrap();
        native.native_only = true;
        let generated = super::render_bidder_bindings(
            &[("fake_native".to_owned(), native)],
            &BTreeSet::from(["experimental".to_owned()]),
        )
        .unwrap();
        let parsed = syn::parse_file(&generated).unwrap();
        let source = quote::quote!(#parsed).to_string();
        let (active, declarations) = source
            .split_once("pub fn bidder_metadata_declarations")
            .unwrap();
        assert!(active.contains(&super::publication_gate(true, Some("experimental")).to_string()));
        assert!(!declarations.contains("cfg"));
        assert!(declarations.contains("fake_native"));
    }

    #[test]
    fn raw_metadata_preserves_explicit_implementation_and_optional_predicate() {
        for (feature, expected) in [
            ("", None),
            (
                "required_feature = 'insecure-bidders'\n",
                Some("insecure-bidders"),
            ),
        ] {
            let source = format!(
                "implementation = 'fake_fixture_ads'\n{feature}endpoint = 'https://fixture.example/rtb'\n"
            );
            let parsed: BidderInfoRaw = toml::from_str(&source).unwrap();
            assert_eq!(parsed.implementation, "fake_fixture_ads");
            assert_eq!(parsed.required_feature.as_deref(), expected);
        }
    }

    #[test]
    fn raw_metadata_rejects_missing_bindings_and_unknown_root_fields() {
        for (source, reason) in [
            (
                "endpoint = 'https://fixture.example/rtb'",
                "missing field `implementation`",
            ),
            (
                "implementation = 'fake_fixture_adapter'\nendpoint = 'https://fixture.example/rtb'\nlegacy_binding = true",
                "unknown field `legacy_binding`",
            ),
        ] {
            let error = toml::from_str::<BidderInfoRaw>(source).unwrap_err();
            assert!(error.message().contains(reason), "{error}");
        }
    }
}

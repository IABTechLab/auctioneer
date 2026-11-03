//! URL safety primitives shared by codegen, runtime user-sync validation,
//! and `tests/url_safety.rs`.
//!
//! Kept dependency-light: only `ipnetwork` + `url` + `std`. Both
//! build-dependencies and dev-dependencies pin the same versions.

use ipnetwork::IpNetwork;
use std::net::IpAddr;
use std::sync::OnceLock;

#[cfg(feature = "build-support")]
use crate::endpoint_contract;

/// IANA non-public IPv4 ranges that `is_private` / `is_link_local` /
/// `is_broadcast` don't cover (notably RFC 6598 CGNAT).
static V4_PRIVATE: OnceLock<Vec<IpNetwork>> = OnceLock::new();
/// IANA non-public IPv6 ranges checked by `endpoint_has_private_ip_authority`.
static V6_PRIVATE: OnceLock<Vec<IpNetwork>> = OnceLock::new();

/// Outcome of running the codegen validator. Each variant carries the
/// human-readable reason so build.rs can panic with the exact message
/// the reviewer pinned, and tests can assert on the variant.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(feature = "build-support")]
pub enum ValidationResult {
    Ok,
    /// `url::Url::parse` failed or the URL has no host. Catches `https://`
    /// (bare scheme), `https://%zz` (bad percent-encoding), `https://[broken`
    /// (mismatched IPv6 brackets), and anything else that doesn't make it
    /// to a resolved (scheme, host, ...) tuple. Prefix-only scheme
    /// matching would let these pass; we require a real parse +
    /// non-empty host before any other check runs.
    MalformedUrl,
    UnsupportedScheme,
    HttpInDefaultBuild,
    LoopbackInDefaultBuild,
    NonLocalhostHttpUnderInsecure,
    /// Under `insecure-bidders`, the feature-on allow-list is exactly
    /// `127.0.0.1`, `[::1]`, `localhost`; every other host inside
    /// `127.0.0.0/8` stays rejected even under the feature, and the
    /// rule applies to both schemes. Without this guard
    /// `https://127.0.0.2/...` slips past the loopback check because
    /// `is_loopback_range` would only be consulted when
    /// `!allow_insecure`.
    NonLiteralLoopbackUnderInsecure,
    /// Hosts that always resolve to loopback via DNS-trick services
    /// (`127.0.0.1.nip.io`, `*.localhost`, `.xip.io`, `.lvh.me`,
    /// `.localtest.me`, `.vcap.me`) get the same treatment as
    /// `localhost` itself — rejected in default builds, and not on
    /// the exact-literal allow-list under insecure either.
    LocalhostAlias,
    PrivateIpAuthority,
    AuthorityMacro,
    TemplateTooLong,
    /// Bidder sync destinations must not carry credentials or a fragment.
    SyncerUserinfoOrFragment,
    /// A syncer URL contains a `{{...}}` macro that is NOT on the
    /// bidder-syncer allow-list. Allowed macros are `{{.RedirectURL}}`
    /// plus the privacy passthrough set (`{{.GDPR}}`, `{{.GDPRConsent}}`,
    /// `{{.USPrivacy}}`, `{{.GPP}}`, `{{.GPPSID}}`). Notably
    /// `{{.ExternalURL}}`, `{{.SyncerKey}}`, `{{.BidderName}}`,
    /// `{{.SyncType}}`, `{{.UserMacro}}` are NOT allowed in this
    /// template — they belong in the `redirect_url`. The variant
    /// carries the offending macro string so the build-time error
    /// message can name it verbatim.
    UnknownMacro(String),
    /// An unclosed `{{...` macro (no matching `}}`) — e.g. a syncer
    /// template with `{{.RedirectURL` — would otherwise ship a broken
    /// template into the codegen output. Carries the unclosed prefix
    /// so the build-time error names where the macro starts.
    UnclosedMacro(String),
}

/// Codegen-time endpoint validation.
///
/// **Default build (`allow_insecure == false`)**:
/// - Endpoint must be `https://` — any `http://` rejected.
/// - Any loopback authority rejected (full `127.0.0.0/8` + `::1` +
///   `localhost`) regardless of scheme.
/// - Any RFC 1918 / 6598 / 4193 / 3927 / 3879 / 4291 link-local
///   private IP literal rejected.
///
/// **Insecure build (`allow_insecure == true`)**:
/// - `https://` continues to be accepted everywhere.
/// - `http://` accepted ONLY when the authority is one of the three
///   exact literals `127.0.0.1`, `::1`, or `localhost`. `127.0.0.2`,
///   `127.1.2.3`, or any other 127/8 host is **rejected** — the
///   relaxation narrows to exact literals, not the whole range.
/// - Private-IP literals still rejected.
#[must_use]
#[cfg(feature = "build-support")]
pub fn validate_endpoint_url(url: &str, allow_insecure: bool) -> ValidationResult {
    // Scheme check first — only `http://` and `https://` are valid input
    // shapes. Anything else is reported as UnsupportedScheme even if the
    // rest of the URL doesn't parse; the scheme is the most actionable
    // error message ("must be http:// or https://").
    let lower = url.to_lowercase();
    let is_https = lower.starts_with("https://");
    let is_http = lower.starts_with("http://");
    if !is_https && !is_http {
        return ValidationResult::UnsupportedScheme;
    }

    // Real URL parse + non-empty host requirement. Catches `https://`
    // (bare scheme), `https://%zz` (bad percent-encoding), `https://[broken`
    // (mismatched bracket), etc. Without this gate the loopback +
    // private-IP checks below silently return false on `endpoint_host`'s
    // None branch and a malformed URL would slip through as Ok.
    if endpoint_host(url).is_none() {
        return ValidationResult::MalformedUrl;
    }

    if is_http && !allow_insecure {
        return ValidationResult::HttpInDefaultBuild;
    }

    // DNS-trick services that always resolve to loopback
    // (`127.0.0.1.nip.io`, `*.localhost`, etc.) get the same
    // treatment as `localhost` itself — rejected in default builds,
    // AND under insecure (the feature-on allow-list is exact-literal
    // only, not aliases).
    if is_localhost_alias_host(url) {
        return ValidationResult::LocalhostAlias;
    }

    // Loopback-range handling. Default builds reject the full
    // `127.0.0.0/8` + `::1` + `localhost`. Insecure narrows the
    // rejection to "anything outside the exact-literal allow-list
    // (`127.0.0.1`, `[::1]`, `localhost`)". Without this branch,
    // `is_loopback_range` would only be consulted when
    // `!allow_insecure`, letting `https://127.0.0.2/...` slip past
    // under the feature.
    let is_loopback = is_loopback_range(url);
    if is_loopback {
        if !allow_insecure {
            return ValidationResult::LoopbackInDefaultBuild;
        }
        if !is_exact_loopback_literal(url) {
            return ValidationResult::NonLiteralLoopbackUnderInsecure;
        }
    }

    // Non-loopback HTTP under insecure stays rejected: the feature
    // only relaxes the scheme for the three exact loopback literals.
    if is_http && allow_insecure && !is_loopback {
        return ValidationResult::NonLocalhostHttpUnderInsecure;
    }

    if endpoint_has_private_ip_authority(url) {
        return ValidationResult::PrivateIpAuthority;
    }

    ValidationResult::Ok
}

/// Require an anchored HTTP(S) scheme and a nonempty authority spelling.
///
/// # Errors
/// Rejects missing/empty authorities, whitespace, and backslash repairs.
pub fn validate_url_spelling(raw: &str, parsed: &url::Url) -> Result<(), String> {
    let after_scheme = raw
        .split_once(':')
        .filter(|(scheme, _)| {
            matches!(parsed.scheme(), "http" | "https")
                && scheme.eq_ignore_ascii_case(parsed.scheme())
        })
        .and_then(|(_, rest)| rest.strip_prefix("//"))
        .ok_or_else(|| "sync URL must have an explicit scheme and authority".to_owned())?;
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if authority.is_empty() || raw.chars().any(char::is_whitespace) || raw.contains('\\') {
        return Err(
            "sync URL must have a literal authority and no whitespace or backslash".to_owned(),
        );
    }
    Ok(())
}

/// Validate path/query placeholders against their owning implementation.
/// `Host` remains an authority token, not a declared value binding.
///
/// # Errors
/// Refuses invalid/reserved declarations or any undeclared template placeholder.
/// Diagnostics never contain template or declaration values.
#[cfg(feature = "build-support")]
pub fn validate_endpoint_macros(
    bidder_name: &str,
    macros: &[String],
    declared: &[String],
) -> Result<(), String> {
    if !endpoint_contract::valid_placeholders(declared.iter().map(String::as_str))
        || macros.iter().any(|name| !declared.contains(name))
    {
        return Err(format!(
            "bidder {bidder_name:?}: endpoint contains an unsupported macro or invalid declaration"
        ));
    }
    Ok(())
}

/// Allow-list of `{{...}}` macros that may appear in a bidder syncer URL
/// (path or query portion): `{{.RedirectURL}}` plus the privacy
/// passthrough set. Any other macro is a build-time error.
#[cfg(feature = "build-support")]
pub const BIDDER_SYNCER_ALLOWED_MACROS: &[&str] = &[
    "{{.RedirectURL}}",
    "{{.GDPR}}",
    "{{.GDPRConsent}}",
    "{{.USPrivacy}}",
    "{{.GPP}}",
    "{{.GPPSID}}",
];

/// Syncer-URL template validation. Applies `validate_endpoint_url`,
/// rejects authority-level template macros (the host portion must be a
/// literal), AND enforces the bidder-syncer macro allow-list on
/// path/query — `{{.ExternalURL}}`, `{{.SyncerKey}}`, `{{.BidderName}}`,
/// `{{.SyncType}}`, `{{.UserMacro}}` and any other unknown macro is
/// rejected at build time.
#[must_use]
#[cfg(feature = "build-support")]
pub fn validate_syncer_template(url: &str, allow_insecure: bool) -> ValidationResult {
    if url.len() > 8_192 {
        return ValidationResult::TemplateTooLong;
    }
    if url.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return ValidationResult::MalformedUrl;
    }
    let endpoint = validate_endpoint_url(url, allow_insecure);
    if endpoint != ValidationResult::Ok {
        return endpoint;
    }
    let authority = strip_to_authority(url);
    if authority.contains("{{") {
        return ValidationResult::AuthorityMacro;
    }
    let Ok(parsed) = url::Url::parse(url) else {
        return ValidationResult::MalformedUrl;
    };
    if validate_url_spelling(url, &parsed).is_err() {
        return ValidationResult::MalformedUrl;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.fragment().is_some() {
        return ValidationResult::SyncerUserinfoOrFragment;
    }
    match scan_macros(url) {
        MacroScan::Ok => ValidationResult::Ok,
        MacroScan::Unknown(bad) => ValidationResult::UnknownMacro(bad),
        MacroScan::Unclosed(prefix) => ValidationResult::UnclosedMacro(prefix),
    }
}

/// Internal result of `scan_macros`. Lets the syncer-template
/// validator distinguish a known-but-unknown macro from a malformed/
/// unclosed one — both are build-time errors, but the messages
/// differ.
#[cfg(feature = "build-support")]
enum MacroScan {
    Ok,
    Unknown(String),
    Unclosed(String),
}

/// Scan a URL for `{{...}}` macros and report the first failure:
///   - `Unknown(m)` — `{{m}}` is well-formed but not on
///     `BIDDER_SYNCER_ALLOWED_MACROS`.
///   - `Unclosed(prefix)` — a `{{` opener has no matching `}}` before
///     end-of-input. Without this branch, a broken syncer template
///     would ship into the codegen output.
///
/// Implementation note: uses `split_once("{{")` + `split_once("}}")`
/// to avoid byte indexing into the input — both `clippy::string_slice`
/// and `clippy::arithmetic_side_effects` are denied in this crate,
/// so the position arithmetic the more obvious implementation would
/// need is unavailable.
#[cfg(feature = "build-support")]
fn scan_macros(url: &str) -> MacroScan {
    let mut remaining = url;
    loop {
        let Some((before, after_open)) = remaining.split_once("{{") else {
            return if remaining.contains("}}") {
                MacroScan::Unclosed("}}".to_owned())
            } else {
                MacroScan::Ok
            };
        };
        if before.contains("}}") {
            return MacroScan::Unclosed("}}".to_owned());
        }
        let Some((body, after_close)) = after_open.split_once("}}") else {
            // Unclosed: report the `{{` plus a short window of what
            // followed so the build-time error pinpoints the start of
            // the broken macro.
            let body_snippet: String = after_open.chars().take(20).collect();
            let mut prefix = String::with_capacity(body_snippet.len().saturating_add(2));
            prefix.push_str("{{");
            prefix.push_str(&body_snippet);
            return MacroScan::Unclosed(prefix);
        };
        let mut macro_str = String::with_capacity(body.len().saturating_add(4));
        macro_str.push_str("{{");
        macro_str.push_str(body);
        macro_str.push_str("}}");
        if !BIDDER_SYNCER_ALLOWED_MACROS.contains(&macro_str.as_str()) {
            return MacroScan::Unknown(macro_str);
        }
        remaining = after_close;
    }
}

/// Classification of a bidder endpoint URL's authority. Scheme + host
/// are taken verbatim from the endpoint (or, when the authority is
/// exactly `{{.Host}}`, from a member of `hostAllowlist`); no other
/// macros may appear in scheme or host. A naive
/// `endpoint.contains("{{.Host}}")` substring check would let
/// `https://{{.RegionHost}}/...` look like a fixed-host endpoint and
/// `https://{{.Host}}{{.Port}}/...` could slip a second authority
/// macro alongside the allow-listed one.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg(feature = "build-support")]
pub enum EndpointAuthorityKind {
    /// Authority contains no `{{...}}` macros — a literal host (with
    /// optional port). Runs through the standard endpoint validator.
    Fixed,
    /// Authority is exactly the token `{{.Host}}`. The runtime
    /// substitutes a member of `hostAllowlist` at request time; the
    /// allow-list is validated per-entry by `validate_host_allowlist`.
    HostVarying,
    /// Authority contains a different / additional macro. This is a
    /// build-time error. The string carried by the variant is the
    /// offending authority verbatim so the error can point at it.
    UnsupportedMacro(String),
}

/// Classify a bidder endpoint URL by inspecting only its authority
/// (everything between `://` and the first `/`, `?`, or `#`). The
/// classifier intentionally does NOT delegate to `url::Url::parse`
/// for the host-varying case — `url::Url` percent-encodes `{` and
/// `}`, so `https://{{.Host}}/...` would parse with a garbage host
/// rather than failing.
#[must_use]
#[cfg(feature = "build-support")]
pub fn classify_endpoint_authority(endpoint: &str) -> EndpointAuthorityKind {
    let authority = strip_to_authority(endpoint);
    if !authority.contains("{{") {
        return EndpointAuthorityKind::Fixed;
    }
    if authority == "{{.Host}}" {
        return EndpointAuthorityKind::HostVarying;
    }
    EndpointAuthorityKind::UnsupportedMacro(authority.to_owned())
}

/// Extract the set of macro names that appear in an endpoint
/// template's path/query portion, deduplicated and sorted for
/// stable codegen output. Each name is returned
/// WITHOUT the surrounding `{{` / `}}` and WITHOUT the leading `.`
/// (e.g. `"AccountID"`, not `"{{.AccountID}}"`), matching the form
/// the auction core plan pins for `BidderRequestInfo.endpoint_macros` and
/// `EndpointBuilder` consumption.
///
/// The special `{{.Host}}` authority macro is EXCLUDED — it is the
/// authority-substitution token handled separately via
/// `host_allowlist`, not a path/query fill that the adapter looks up.
/// Authority-level macros other than `{{.Host}}` are rejected up
/// front by `classify_endpoint_authority`; this function operates on
/// already-validated endpoints, so any `{{...}}` token it sees is
/// in the path or query.
#[must_use]
#[cfg(feature = "build-support")]
pub fn extract_endpoint_macros(endpoint: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    let mut remaining = endpoint;
    while let Some((_before, after_open)) = remaining.split_once("{{") {
        let Some((body, after_close)) = after_open.split_once("}}") else {
            // Unclosed `{{` — caller must gate on
            // `endpoint_unclosed_macro_prefix()` before this scan so
            // the malformed template surfaces at build time. Bail
            // early to avoid infinite loops on a stray `{{`.
            return seen;
        };
        // Strip an optional leading '.' (Go template syntax for top-
        // level fields).
        let name = body.strip_prefix('.').unwrap_or(body);
        if name != "Host" && !seen.iter().any(|existing| existing == name) {
            seen.push(name.to_owned());
        }
        remaining = after_close;
    }
    seen.sort();
    seen
}

/// `Host` is an authority-substitution token, never a path/query
/// fill. Returns true when `{{.Host}}` appears anywhere in `endpoint`
/// outside the authority — i.e. after the first `/`, `?`, or `#` that
/// terminates the authority (or, for endpoints without a scheme
/// separator, anywhere past the start).
///
/// Build-time callers reject any endpoint where this returns true;
/// otherwise a template like `https://fixed.example/rtb/{{.Host}}`
/// would be misclassified — `classify_endpoint_authority` would return
/// `Fixed`, but a substring scan for `{{.Host}}` (used by the runtime
/// `select_host` path and by `validate_host_allowlist`) would find it
/// in the path and treat the bidder as host-varying.
#[must_use]
#[cfg(feature = "build-support")]
pub fn endpoint_has_host_macro_outside_authority(endpoint: &str) -> bool {
    let authority = strip_to_authority(endpoint);
    let after_scheme = endpoint.split_once("://").map_or(endpoint, |split| split.1);
    let path_and_query = after_scheme.get(authority.len()..).unwrap_or("");
    path_and_query.contains("{{.Host}}")
}

/// Returns the first unclosed `{{` prefix in `endpoint` (a `{{` with
/// no following `}}`), or `None` if every `{{` is properly closed.
///
/// The prefix carries up to 32 bytes of context starting at the
/// unclosed `{{` so the build-time error message can point the
/// operator at the offending fragment. Extraction stops at
/// whitespace / newline to keep the message single-line.
///
/// Endpoint macro validation is a two-step gate:
/// `render_endpoint_macros` calls this first, then invokes
/// [`extract_endpoint_macros`] + [`validate_endpoint_macros`]. The
/// syncer path has an equivalent gate inside `MacroScan::Unclosed`
/// so the two shapes stay symmetric.
#[must_use]
#[cfg(feature = "build-support")]
pub fn endpoint_unclosed_macro_prefix(endpoint: &str) -> Option<String> {
    let mut remaining = endpoint;
    while let Some((_before, after_open)) = remaining.split_once("{{") {
        if let Some((_body, after_close)) = after_open.split_once("}}") {
            remaining = after_close;
            continue;
        }
        // No matching `}}` — capture up to 32 bytes of context
        // stopping at whitespace.
        let ctx: String = after_open
            .chars()
            .take_while(|c| !c.is_whitespace())
            .take(32)
            .collect();
        return Some(format!("{{{{{ctx}"));
    }
    None
}

/// Authority extraction for the syncer-template macro check. Uses
/// string splitting because the authority portion may contain raw
/// `{{...}}` macros that `url::Url::parse` rejects.
#[must_use]
#[cfg(feature = "build-support")]
pub fn strip_to_authority(url: &str) -> &str {
    let after_scheme = url.split_once("://").map_or("", |split| split.1);
    after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme)
}

/// True when the host is anywhere in IANA loopback space: full
/// `127.0.0.0/8`, IPv6 `::1`, or the case-insensitive `localhost`
/// hostname, including DNS root dots. Used to gate the default-build rejection.
#[must_use]
pub fn is_loopback_range(endpoint: &str) -> bool {
    let Some(host) = endpoint_host(endpoint) else {
        return false;
    };
    // Normalize DNS root dots only for denial, never for the exact allow-list.
    if host.trim_end_matches('.').eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|addr| addr.is_loopback())
}

/// Known DNS-trick services whose hostnames always resolve to a
/// loopback / caller-supplied IP literal. `127.0.0.1.nip.io` is the
/// canonical example; the rest are equally well-known in the same
/// family (xip.io is nip.io's predecessor, lvh.me / localtest.me /
/// vcap.me are aliases shipped by Pivotal's CF ecosystem and adopted
/// broadly). Exact-match the apex OR `*.<apex>` suffix.
const LOCALHOST_ALIAS_APEXES: &[&str] = &["nip.io", "xip.io", "lvh.me", "localtest.me", "vcap.me"];

/// True for hostnames that always resolve to loopback via a DNS
/// trick:
///   - `*.localhost` (RFC 6761 reserves the `.localhost.` TLD for
///     loopback). Note this is the `.localhost`-SUFFIX form; exact
///     `localhost` is checked by `is_loopback_range` and allowed on
///     the insecure-bidders allow-list.
///   - `*.<apex>` for the apex hosts in `LOCALHOST_ALIAS_APEXES`
///     (`nip.io`, `xip.io`, `lvh.me`, `localtest.me`, `vcap.me`).
///   - The bare apex itself (e.g. `nip.io` with no subdomain).
///
/// Default builds reject these outright; insecure builds also reject
/// them, because the feature-on allow-list is `127.0.0.1` / `[::1]` /
/// `localhost` exact-literal only — aliases are not on it.
#[must_use]
pub fn is_localhost_alias_host(endpoint: &str) -> bool {
    let Some(host) = endpoint_host(endpoint) else {
        return false;
    };
    let lower = host.trim_end_matches('.').to_ascii_lowercase();
    if lower.ends_with(".localhost") {
        return true;
    }
    for apex in LOCALHOST_ALIAS_APEXES {
        if lower == *apex {
            return true;
        }
        // Match `.<apex>` as a suffix by stripping the apex tail, then
        // confirming the remaining prefix ends with `.`. Avoids byte
        // indexing arithmetic (denied by clippy in this crate).
        if let Some(prefix) = lower.strip_suffix(apex)
            && prefix.ends_with('.')
        {
            return true;
        }
    }
    false
}

/// True ONLY for the three exact literals pinned for the
/// `insecure-bidders` relaxation: `127.0.0.1`, `::1`, and `localhost`
/// (case-insensitive). Anything else in `127.0.0.0/8` (e.g.
/// `127.0.0.2`) is rejected.
#[must_use]
pub fn is_exact_loopback_literal(endpoint: &str) -> bool {
    let Some(host) = endpoint_host(endpoint) else {
        return false;
    };
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
}

/// True when the host is a hard-coded non-public IP literal. Mirrors
/// the stronger `is_public_ip` classifier used in
/// `auctioneer_openrtb::prepare::implicit_fill` so the build-time SSRF
/// boundary matches the runtime header-IP classifier:
///
///   - **IPv4**: RFC 1918 private (`10/8`, `172.16/12`, `192.168/16`),
///     RFC 6598 CGNAT (`100.64/10`), RFC 3927 link-local (`169.254/16`),
///     RFC 919 broadcast (`255.255.255.255`), RFC 1122 "this network"
///     (`0.0.0.0/8`), RFC 7335 IETF protocol assignments (`192.0.0.0/24`),
///     RFC 5737 documentation (`192.0.2.0/24`, `198.51.100.0/24`,
///     `203.0.113.0/24`), RFC 2544 benchmarking (`198.18.0.0/15`),
///     RFC 1112 reserved future use (`240.0.0.0/4`), plus the IPv4
///     unspecified (`0.0.0.0`) and multicast space (`224.0.0.0/4`).
///   - **IPv6**: RFC 4193 unique-local (`fc00::/7`), RFC 4291
///     link-local (`fe80::/10`), RFC 3879 deprecated site-local
///     (`fec0::/10`), RFC 3849 documentation (`2001:db8::/32`),
///     RFC 6666 discard prefix (`100::/64`), RFC 6052 / RFC 8215
///     NAT64 (`64:ff9b::/96` + `64:ff9b:1::/48`), RFC 2928 IETF
///     protocol assignments (`2001::/23`), RFC 5180 benchmarking
///     (`2001:2::/48`), RFC 9637 reserved documentation (`3fff::/20`),
///     RFC 4291 IPv4-mapped (`::ffff:0:0/96`), plus the IPv6
///     unspecified (`::`) and multicast space (`ff00::/8`).
///
/// Loopback is checked separately by [`is_loopback_range`] so the
/// caller can run the right rule per dir; loopback addresses
/// short-circuit at the top so the per-build loopback policy
/// (default-reject vs insecure-allow-exact-literal) decides the
/// outcome instead of this function.
///
/// # Panics
/// Panics if a hardcoded reserved-address CIDR is invalid.
#[must_use]
pub fn endpoint_has_private_ip_authority(endpoint: &str) -> bool {
    let Some(host) = endpoint_host(endpoint) else {
        return false;
    };
    let Ok(addr) = host.parse::<IpAddr>() else {
        return false;
    };
    if addr.is_loopback() {
        return false;
    }
    // Unspecified and multicast (i.e. `0.0.0.0`, `::`, and
    // `224.0.0.0/4`) are not public addresses; reject them here so
    // they don't short-circuit to `false` and slip past codegen.
    if addr.is_unspecified() || addr.is_multicast() {
        return true;
    }
    let v4 = V4_PRIVATE.get_or_init(|| {
        [
            "0.0.0.0/8",       // RFC 1122 "this network" / source-only
            "100.64.0.0/10",   // RFC 6598 CGNAT / shared address space
            "192.0.0.0/24",    // RFC 7335 IETF protocol assignments
            "192.0.2.0/24",    // RFC 5737 documentation (TEST-NET-1)
            "198.18.0.0/15",   // RFC 2544 benchmarking
            "198.51.100.0/24", // RFC 5737 documentation (TEST-NET-2)
            "203.0.113.0/24",  // RFC 5737 documentation (TEST-NET-3)
            "240.0.0.0/4",     // RFC 1112 reserved future use
        ]
        .iter()
        .map(|cidr| {
            #[expect(
                clippy::expect_used,
                reason = "hardcoded IANA-reserved CIDRs are statically valid"
            )]
            cidr.parse().expect("hardcoded CIDR")
        })
        .collect()
    });
    let v6 = V6_PRIVATE.get_or_init(|| {
        [
            "fc00::/7",       // RFC 4193 unique-local
            "fe80::/10",      // RFC 4291 link-local
            "fec0::/10",      // RFC 3879 deprecated site-local
            "2001:db8::/32",  // RFC 3849 documentation
            "100::/64",       // RFC 6666 discard prefix
            "64:ff9b::/96",   // RFC 6052 well-known NAT64
            "64:ff9b:1::/48", // RFC 8215 NAT64
            "2001::/23",      // RFC 2928 IETF protocol assignments
            "2001:2::/48",    // RFC 5180 benchmarking
            "3fff::/20",      // RFC 9637 reserved documentation
            "::ffff:0:0/96",  // RFC 4291 IPv4-mapped (also covered in v4 form)
        ]
        .iter()
        .map(|cidr| {
            #[expect(
                clippy::expect_used,
                reason = "hardcoded IANA-reserved IPv6 CIDRs are statically valid"
            )]
            cidr.parse().expect("hardcoded CIDR")
        })
        .collect()
    });
    match addr {
        IpAddr::V4(addr_v4) => {
            addr_v4.is_private()
                || addr_v4.is_link_local()
                || addr_v4.is_broadcast()
                || v4.iter().any(|net| net.contains(addr))
        }
        IpAddr::V6(_) => v6.iter().any(|net| net.contains(addr)),
    }
}

/// Extract the host portion via `url::Url`. Port stripping is handled
/// by the parser. IPv6 literals come back from `host_str` with their
/// `[...]` brackets so the caller can compare against `[::1]` etc.;
/// we strip them here so `IpAddr::parse` succeeds when the caller
/// runs it.
#[must_use]
pub fn endpoint_host(endpoint: &str) -> Option<String> {
    let parsed = url::Url::parse(endpoint).ok()?;
    let host = parsed.host_str()?;
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(host);
    Some(unbracketed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{is_exact_loopback_literal, is_localhost_alias_host, is_loopback_range};

    #[test]
    fn loopback_dns_classification_ignores_root_dots() {
        for host in ["localhost", "LOCALHOST"] {
            for suffix in ["", ".", ".."] {
                let endpoint = format!("https://{host}{suffix}:8443/sync");
                assert!(is_loopback_range(&endpoint), "{endpoint}");
                assert_eq!(
                    is_exact_loopback_literal(&endpoint),
                    suffix.is_empty(),
                    "{endpoint}"
                );
            }
        }
    }

    #[test]
    fn denied_alias_classification_ignores_root_dots() {
        for host in [
            "sub.localhost",
            "nip.io",
            "xip.io",
            "lvh.me",
            "localtest.me",
            "vcap.me",
            "127.0.0.1.nip.io",
            "app.xip.io",
            "app.lvh.me",
            "app.localtest.me",
            "app.vcap.me",
        ] {
            for suffix in ["", ".", ".."] {
                let endpoint = format!("https://{}{suffix}:8443/sync", host.to_ascii_uppercase());
                assert!(is_localhost_alias_host(&endpoint), "{endpoint}");
                assert!(!is_exact_loopback_literal(&endpoint), "{endpoint}");
            }
        }
    }

    #[test]
    fn root_dot_classification_keeps_dns_label_boundaries() {
        for host in [
            "sync.example.com",
            "fakelocalhost.example",
            "snip.io",
            "notlvh.me",
            "nip.io.example",
        ] {
            for suffix in ["", "."] {
                let endpoint = format!("https://{host}{suffix}/sync");
                assert!(!is_loopback_range(&endpoint), "{endpoint}");
                assert!(!is_localhost_alias_host(&endpoint), "{endpoint}");
            }
        }
    }
}

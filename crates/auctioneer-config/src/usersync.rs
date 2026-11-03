//! Shared user-sync configuration and validation.
//!
//! This file is compiled by the library and by the build script, so baked
//! values and runtime overlays have one set of safety checks.

use auctioneer_runtime::aux_work::{
    MAX_CATALOG_IDENTITIES, MAX_SYNC_PRIORITY_ENTRIES, MAX_SYNC_PRIORITY_GROUPS,
};
use serde::{Deserialize, Serialize};
use url::{Url, form_urlencoded};

use crate::url_safety;
pub use crate::url_safety::validate_url_spelling;

pub const MAX_SYNC_QUERY_PARAMS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyncQueryEncodingError {
    InvalidPercentEncoding,
    InvalidUtf8,
}

/// Validate without the form parser's permissive percent/UTF-8 repair.
///
/// # Errors
/// Rejects incomplete/non-hex escapes and non-UTF-8 decoded query bytes.
pub fn validate_sync_query_encoding(query: &str) -> Result<(), SyncQueryEncodingError> {
    let mut bytes = query.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%'
            && (!bytes.next().is_some_and(|digit| digit.is_ascii_hexdigit())
                || !bytes.next().is_some_and(|digit| digit.is_ascii_hexdigit()))
        {
            return Err(SyncQueryEncodingError::InvalidPercentEncoding);
        }
    }
    percent_encoding::percent_decode_str(query)
        .decode_utf8()
        .map(|_| ())
        .map_err(|_error| SyncQueryEncodingError::InvalidUtf8)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostCookieConfig {
    pub cookie_name: String,
    pub family: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default = "default_ttl_days")]
    pub ttl_days: i64,
    #[serde(default)]
    pub max_cookie_size_bytes: usize,
    #[serde(default)]
    pub optout_cookie: Option<OptoutCookie>,
}

const fn default_ttl_days() -> i64 {
    90
}

impl Default for HostCookieConfig {
    fn default() -> Self {
        Self {
            cookie_name: "auctioneer_uid".to_owned(),
            family: "auctioneer".to_owned(),
            domain: None,
            ttl_days: default_ttl_days(),
            max_cookie_size_bytes: 0,
            optout_cookie: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptoutCookie {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserSyncConfig {
    pub external_url: String,
    #[serde(default)]
    pub redirect_url: Option<String>,
    #[serde(default)]
    pub cooperative: CooperativeConfig,
    #[serde(default)]
    pub priority_groups: Vec<Vec<String>>,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

impl Default for UserSyncConfig {
    fn default() -> Self {
        Self {
            external_url: "https://sync.example.test".to_owned(),
            redirect_url: None,
            cooperative: CooperativeConfig::default(),
            priority_groups: Vec::new(),
            allowed_origins: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CooperativeConfig {
    pub enabled_by_default: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AccountCookieSyncConfig {
    pub default_limit: Option<i64>,
    pub max_limit: Option<i64>,
    pub default_coop_sync: Option<bool>,
    pub priority_groups: Option<Vec<Vec<String>>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PurposeOneTreatment {
    pub enabled: bool,
    pub access_allowed: bool,
}

impl Default for PurposeOneTreatment {
    fn default() -> Self {
        Self {
            enabled: true,
            access_allowed: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PurposeOneTreatmentOverride {
    pub enabled: Option<bool>,
    pub access_allowed: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncType {
    Iframe,
    Redirect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncResponseFormat {
    Image,
    Blank,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SyncerConfig {
    pub key: Option<String>,
    pub iframe: Option<SyncerEndpoint>,
    pub redirect: Option<SyncerEndpoint>,
    pub supports: Vec<SyncType>,
    pub enabled: bool,
    pub skip_when: SkipWhen,
    pub default_response: Option<SyncResponseFormat>,
}

impl Default for SyncerConfig {
    fn default() -> Self {
        Self {
            key: None,
            iframe: None,
            redirect: None,
            supports: Vec::new(),
            enabled: true,
            skip_when: SkipWhen::default(),
            default_response: None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkipWhen {
    pub gdpr: bool,
    #[serde(alias = "gppSid", alias = "GPPSID")]
    pub gpp_sid: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SyncerEndpoint {
    pub url: String,
    pub user_macro: Option<String>,
}

fn cookie_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn cookie_value(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b'"' | b';' | b'\\'))
}

/// # Errors
/// Rejects an unsafe cookie identity, attribute, domain, TTL, or size cap.
pub fn validate_host_cookie(cookie: &HostCookieConfig) -> Result<(), String> {
    if cookie.cookie_name.len() > 128
        || !cookie_token(&cookie.cookie_name)
        || cookie.cookie_name == "uids"
    {
        return Err("host_cookie.cookie_name must be a non-reserved cookie token".to_owned());
    }
    if cookie.family.len() > 128 || !cookie_token(&cookie.family) {
        return Err("host_cookie.family must be a cookie token".to_owned());
    }
    if cookie.ttl_days <= 0 || cookie.ttl_days.checked_mul(86_400).is_none() {
        return Err("host_cookie.ttl_days must be positive and finite".to_owned());
    }
    if cookie.max_cookie_size_bytes > 0 && cookie.max_cookie_size_bytes < 500 {
        return Err("host_cookie.max_cookie_size_bytes must be zero or at least 500".to_owned());
    }
    if let Some(domain) = &cookie.domain
        && (domain.is_empty()
            || domain.len() > 253
            || domain.starts_with('.')
            || domain.ends_with('.')
            || domain.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            }))
    {
        return Err("host_cookie.domain is not a valid DNS name".to_owned());
    }
    if let Some(optout) = &cookie.optout_cookie {
        if optout.name.is_empty() && optout.value.is_empty() {
            return Ok(());
        }
        if optout.name.len() > 128
            || optout.value.len() > 128
            || !cookie_token(&optout.name)
            || optout.value.is_empty()
            || !cookie_value(&optout.value)
        {
            return Err("host_cookie.optout_cookie requires a safe name and value".to_owned());
        }
    }
    Ok(())
}

fn validate_base_url(raw: &str, allow_insecure: bool) -> Result<Url, String> {
    if raw.len() > 2_048 || raw.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err("sync URL is too long or contains a control character".to_owned());
    }
    let parsed = Url::parse(raw).map_err(|_error| "sync URL must be absolute".to_owned())?;
    validate_url_spelling(raw, &parsed)?;
    let loopback = host_is_loopback(&parsed);
    if parsed.scheme() != "https" && !(allow_insecure && parsed.scheme() == "http" && loopback) {
        return Err("sync URL must use HTTPS".to_owned());
    }
    if host_is_private(&parsed) && !(allow_insecure && loopback) {
        return Err("sync URL cannot use a private literal host".to_owned());
    }
    // HTTPS parsing and strict spelling already require a host; keep the
    // explicit authority invariant alongside the independent userinfo checks.
    if parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(
            "sync URL must have a literal host and no credentials, query, or fragment".to_owned(),
        );
    }
    Ok(parsed)
}

/// # Errors
/// Rejects an unsafe callback origin, redirect template, priority list, or CORS origin.
pub fn validate_user_sync(sync: &UserSyncConfig, allow_insecure: bool) -> Result<(), String> {
    validate_base_url(&sync.external_url, allow_insecure)?;
    if let Some(redirect) = &sync.redirect_url {
        validate_redirect_template(redirect, &sync.external_url, allow_insecure)?;
    }
    validate_priority_groups(&sync.priority_groups)?;
    if sync.allowed_origins.len() > 256 {
        return Err("user_sync.allowed_origins exceeds 256 origins".to_owned());
    }
    for origin in &sync.allowed_origins {
        let parsed = validate_base_url(origin, allow_insecure)?;
        if parsed.path() != "/" || origin != &parsed.origin().ascii_serialization() {
            return Err("user_sync.allowed_origins must contain exact origins".to_owned());
        }
    }
    Ok(())
}

fn validate_redirect_template(
    template: &str,
    external_url: &str,
    allow_insecure: bool,
) -> Result<(), String> {
    if template.is_empty() || template.len() > 8_192 {
        return Err("user_sync.redirect_url must be a bounded nonempty template".to_owned());
    }
    if template.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err("user_sync.redirect_url contains a control character".to_owned());
    }
    let query = template.split_once('?').map_or("", |(_, query)| query);
    validate_sync_query_encoding(query)
        .map_err(|_error| "user_sync.redirect_url has invalid query encoding".to_owned())?;
    if query.split('&').count() >= MAX_SYNC_QUERY_PARAMS {
        return Err(
            "user_sync.redirect_url must reserve one query parameter for account".to_owned(),
        );
    }
    if query.split('&').any(|field| {
        let key = field.split_once('=').map_or(field, |(key, _)| key);
        key.contains("{{") || key.contains("}}")
    }) {
        return Err("user_sync.redirect_url cannot template query keys".to_owned());
    }

    let external_token = "{{.ExternalURL}}";
    if let Some(rest) = template.strip_prefix(external_token) {
        if !rest.starts_with('/') {
            return Err("user_sync.redirect_url has a templated authority".to_owned());
        }
    } else {
        let after_scheme = template
            .split_once(':')
            .and_then(|(_, rest)| rest.strip_prefix("//"))
            .ok_or_else(|| "user_sync.redirect_url must resolve to an absolute URL".to_owned())?;
        let authority = after_scheme
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default();
        if authority.contains("{{") || authority.contains("}}") {
            return Err("user_sync.redirect_url cannot template its authority".to_owned());
        }
    }

    let mut rendered = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((prefix, after_open)) = rest.split_once("{{") {
        if prefix.contains("}}") {
            return Err("user_sync.redirect_url has an unmatched token close".to_owned());
        }
        rendered.push_str(prefix);
        let (token, after_close) = after_open
            .split_once("}}")
            .ok_or_else(|| "user_sync.redirect_url has an unclosed token".to_owned())?;
        let dummy = match token {
            ".ExternalURL" => external_url.trim_end_matches('/'),
            ".SyncerKey" | ".BidderName" | ".SyncType" | ".UserMacro" | ".GDPR"
            | ".GDPRConsent" | ".USPrivacy" | ".GPP" | ".GPPSID" => "safe",
            _ => return Err("user_sync.redirect_url has an unknown token".to_owned()),
        };
        rendered.push_str(dummy);
        rest = after_close;
    }
    if rest.contains("}}") {
        return Err("user_sync.redirect_url has an unmatched token close".to_owned());
    }
    rendered.push_str(rest);
    let parsed = Url::parse(&rendered)
        .map_err(|_error| "user_sync.redirect_url does not resolve to a URL".to_owned())?;
    validate_url_spelling(&rendered, &parsed)?;
    let loopback = host_is_loopback(&parsed);
    if parsed.scheme() != "https" && !(allow_insecure && parsed.scheme() == "http" && loopback) {
        return Err("user_sync.redirect_url must resolve to HTTPS".to_owned());
    }
    if host_is_private(&parsed) && !(allow_insecure && loopback) {
        return Err("user_sync.redirect_url cannot use a private literal host".to_owned());
    }
    if parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err("user_sync.redirect_url has an unsafe authority or fragment".to_owned());
    }
    if parsed.query_pairs().any(|(key, _)| key == "account") {
        return Err("user_sync.redirect_url must not hard-code account".to_owned());
    }
    if !(template.ends_with("?uid={{.UserMacro}}") || template.ends_with("&uid={{.UserMacro}}"))
        || template.matches("{{.UserMacro}}").count() != 1
        || parsed.query_pairs().filter(|(key, _)| key == "uid").count() != 1
    {
        return Err("user_sync.redirect_url must end in one uid macro field".to_owned());
    }
    let mut bidder_fields = query.split('&').filter_map(|field| {
        let (key, value) = field.split_once('=').unwrap_or((field, ""));
        let decoded_key = form_urlencoded::parse(key.as_bytes()).next()?.0;
        (decoded_key == "bidder").then_some(value)
    });
    if !matches!(
        bidder_fields.next(),
        Some("{{.SyncerKey}}" | "{{.BidderName}}")
    ) || bidder_fields.next().is_some()
    {
        return Err("user_sync.redirect_url must contain one bound bidder macro field".to_owned());
    }
    Ok(())
}

fn host_is_loopback(url: &Url) -> bool {
    url_safety::is_exact_loopback_literal(url.as_str())
}

fn host_is_private(url: &Url) -> bool {
    url_safety::is_loopback_range(url.as_str())
        || url_safety::is_localhost_alias_host(url.as_str())
        || url_safety::endpoint_has_private_ip_authority(url.as_str())
}

fn validate_priority_groups(groups: &[Vec<String>]) -> Result<(), String> {
    if u32::try_from(groups.len())
        .ok()
        .is_none_or(|count| count > MAX_SYNC_PRIORITY_GROUPS)
        || groups
            .iter()
            .try_fold(0_usize, |count, group| count.checked_add(group.len()))
            .and_then(|count| u32::try_from(count).ok())
            .is_none_or(|count| count > MAX_SYNC_PRIORITY_ENTRIES)
    {
        return Err("user-sync priority groups exceed bounds".to_owned());
    }
    for name in groups.iter().flatten() {
        if name.is_empty()
            || name.len() > 128
            || !name.bytes().all(|byte| byte >= 0x21 && byte != 0x7f)
        {
            return Err("user-sync priority bidder name is invalid".to_owned());
        }
    }
    Ok(())
}

/// # Errors
/// Rejects an invalid account priority list.
pub fn validate_account_cookie_sync(sync: &AccountCookieSyncConfig) -> Result<(), String> {
    if let Some(groups) = &sync.priority_groups {
        validate_priority_groups(groups)?;
    }
    Ok(())
}

/// # Errors
/// Rejects a catalog larger than the configured work ceiling.
pub fn validate_catalog_size(bidders: usize, aliases: usize) -> Result<(), String> {
    if bidders
        .checked_add(aliases)
        .and_then(|total| u32::try_from(total).ok())
        .is_none_or(|total| total > MAX_CATALOG_IDENTITIES)
    {
        return Err(format!(
            "user-sync catalog exceeds {MAX_CATALOG_IDENTITIES} names"
        ));
    }
    Ok(())
}

/// # Errors
/// Rejects oversized or unsafe raw GPP section identifiers.
pub fn validate_skip_when(policy: &SkipWhen) -> Result<(), String> {
    if policy.gpp_sid.len() > 256
        || policy.gpp_sid.iter().any(|value| {
            value.is_empty()
                || value.len() > 128
                || value.bytes().any(|byte| byte < 0x21 || byte == 0x7f)
        })
    {
        return Err("userSync.skipWhen exceeds its bounds or contains unsafe values".to_owned());
    }
    Ok(())
}

/// Validate the bidder-owned literal token that its sync endpoint replaces.
/// An absent or empty token means append-only callback mode.
///
/// # Errors
/// Rejects a token that is oversized or can alter a callback query field.
pub fn validate_user_macro(maybe_macro: Option<&str>) -> Result<(), String> {
    let Some(user_macro) = maybe_macro else {
        return Ok(());
    };
    if user_macro.len() > 256
        || user_macro
            .bytes()
            .any(|byte| byte < 0x21 || byte == 0x7f || b"&=?%".contains(&byte))
        || user_macro.contains("{{")
    {
        return Err("userSync userMacro is unsafe or exceeds 256 bytes".to_owned());
    }
    Ok(())
}

/// # Errors
/// Rejects a bidder cookie-family key that cannot be represented safely.
pub fn validate_syncer_key(maybe_key: Option<&str>) -> Result<(), String> {
    if let Some(key) = maybe_key
        && (key.len() > 128 || !cookie_token(key))
    {
        return Err("userSync key must be a bounded cookie token".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AccountCookieSyncConfig, HostCookieConfig, OptoutCookie, PurposeOneTreatment, SkipWhen,
        UserSyncConfig, validate_account_cookie_sync, validate_catalog_size, validate_host_cookie,
        validate_skip_when, validate_syncer_key, validate_user_macro, validate_user_sync,
    };

    #[test]
    fn callback_query_keys_cannot_contain_request_macros() {
        for key in [
            "{{.GDPRConsent}}",
            "{{.GDPR}}account",
            "prefix{{.GPP}}",
            "{{.USPrivacy}}suffix",
            "{{.GPPSID}}",
        ] {
            let sync = UserSyncConfig {
                redirect_url: Some(format!(
                    "{{{{.ExternalURL}}}}/setuid?{key}=other&bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            assert!(validate_user_sync(&sync, false).is_err(), "key {key}");
        }
        let sync = UserSyncConfig {
            redirect_url: Some(
                "{{.ExternalURL}}/setuid?consent={{.GDPRConsent}}&bidder={{.SyncerKey}}&uid={{.UserMacro}}".to_owned(),
            ),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&sync, false).expect("request macros remain valid query values");
    }

    #[test]
    fn callback_reserves_one_receiver_parameter_for_account() {
        for extra in [29_usize, 30] {
            let fields = "x=1&".repeat(extra);
            let sync = UserSyncConfig {
                redirect_url: Some(format!(
                    "{{{{.ExternalURL}}}}/setuid?{fields}bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            assert_eq!(validate_user_sync(&sync, false).is_ok(), extra == 29);
        }
    }

    #[test]
    fn callback_literals_must_obey_receiver_form_encoding() {
        for field in ["x=%zz", "%FF=1", "x=%C3%28", "x=%", "x=%0"] {
            let sync = UserSyncConfig {
                redirect_url: Some(format!(
                    "{{{{.ExternalURL}}}}/setuid?{field}&bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            assert!(validate_user_sync(&sync, false).is_err(), "{field}");
        }
        let sync = UserSyncConfig {
            redirect_url: Some(
                "{{.ExternalURL}}/setuid?x=%EF%BF%BD&%62idder={{.SyncerKey}}&uid={{.UserMacro}}"
                    .to_owned(),
            ),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&sync, false).expect("valid encoded Unicode and literal keys");
    }

    #[test]
    fn priorities_accept_bounded_catalog_alias_names() {
        for name in ["fake_partner.alias", "fake_partner+alias", "fake_\u{e9}"] {
            let groups = vec![vec![name.to_owned()]];
            let sync = UserSyncConfig {
                priority_groups: groups.clone(),
                ..UserSyncConfig::default()
            };
            validate_user_sync(&sync, false).expect("host priority alias");
            let account = AccountCookieSyncConfig {
                priority_groups: Some(groups),
                ..AccountCookieSyncConfig::default()
            };
            validate_account_cookie_sync(&account).expect("account priority alias");
        }
    }

    #[test]
    fn redirect_bidder_identity_requires_one_exact_macro_binding() {
        let invalid = [
            "uid={{.UserMacro}}",
            "bidder=fake_fixture_base&uid={{.UserMacro}}",
            "bidder=fake_fixture_base&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "bidder={{.SyncerKey}}&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "%62idder=fake_fixture_base&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "bidder={{.SyncerKey}}&bidd%65r=fake_fixture_base&uid={{.UserMacro}}",
            "bidder=prefix{{.SyncerKey}}&uid={{.UserMacro}}",
            "bidder={{.BidderName}}suffix&uid={{.UserMacro}}",
            "bidder={{.GDPRConsent}}&uid={{.UserMacro}}",
            "bidder=%7B%7B.SyncerKey%7D%7D&uid={{.UserMacro}}",
        ];
        let accepted: Vec<_> = invalid
            .into_iter()
            .filter(|query| {
                let sync = UserSyncConfig {
                    redirect_url: Some(format!("{{{{.ExternalURL}}}}/setuid?{query}")),
                    ..UserSyncConfig::default()
                };
                validate_user_sync(&sync, false).is_ok()
            })
            .collect();
        assert!(
            accepted.is_empty(),
            "unsafe bidder bindings accepted: {accepted:?}"
        );

        for binding in ["{{.SyncerKey}}", "{{.BidderName}}"] {
            let sync = UserSyncConfig {
                redirect_url: Some(format!(
                    "{{{{.ExternalURL}}}}/setuid?bidder={binding}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            validate_user_sync(&sync, false).expect("exact bidder macro binding");
        }
    }

    #[test]
    fn callback_base_rejects_parser_repaired_scheme_authority_and_whitespace() {
        let invalid = [
            "https:sync.example.com",
            "https:/sync.example.com",
            "https:///sync.example.com",
            "https:////sync.example.com",
            "https://sync.example.com ",
            " https://sync.example.com",
            "https://sync.example.com/base path",
            "https://sync.example.com/\u{a0}",
            "https://sync.example.com\\base",
        ];
        let accepted: Vec<_> = invalid
            .into_iter()
            .filter(|external| {
                let sync = UserSyncConfig {
                    external_url: (*external).to_owned(),
                    ..UserSyncConfig::default()
                };
                validate_user_sync(&sync, false).is_ok()
            })
            .collect();
        assert!(
            accepted.is_empty(),
            "repaired callback bases accepted: {accepted:?}"
        );
    }

    #[test]
    fn callback_base_rejects_embedded_scheme_delimiter() {
        let invalid = [
            ("https:sync.example.test://unused", false),
            ("https:/sync.example.test/path://unused", false),
            ("HTTPS:sync.example.test://unused", false),
            ("http:localhost://unused", true),
            ("http:/127.0.0.1/path://unused", true),
        ];
        let accepted: Vec<_> = invalid
            .into_iter()
            .filter(|(external_url, allow_insecure)| {
                let sync = UserSyncConfig {
                    external_url: (*external_url).to_owned(),
                    ..UserSyncConfig::default()
                };
                validate_user_sync(&sync, *allow_insecure).is_ok()
            })
            .collect();
        assert!(
            accepted.is_empty(),
            "unanchored callback bases accepted: {accepted:?}"
        );
    }

    #[test]
    fn literal_redirect_rejects_embedded_scheme_delimiter() {
        let invalid = [
            ("https:sync.example.test://unused", false),
            ("https:/sync.example.test/path://unused", false),
            ("HTTPS:sync.example.test://unused", false),
            ("http:localhost://unused", true),
        ];
        let accepted: Vec<_> = invalid
            .into_iter()
            .filter(|(base, allow_insecure)| {
                let sync = UserSyncConfig {
                    redirect_url: Some(format!(
                        "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                    )),
                    ..UserSyncConfig::default()
                };
                validate_user_sync(&sync, *allow_insecure).is_ok()
            })
            .collect();
        assert!(
            accepted.is_empty(),
            "unanchored literal callbacks accepted: {accepted:?}"
        );
    }

    #[test]
    fn anchored_callbacks_preserve_scheme_case_and_path_delimiters() {
        for (base, allow_insecure) in [
            ("https://sync.example.test/path://unused", false),
            ("HTTPS://sync.example.test/path://unused", false),
            ("http://localhost/path://unused", true),
            ("HTTP://127.0.0.1/path://unused", true),
        ] {
            let mut sync = UserSyncConfig {
                external_url: base.to_owned(),
                redirect_url: Some(VALID_CALLBACK.to_owned()),
                ..UserSyncConfig::default()
            };
            validate_user_sync(&sync, allow_insecure).expect("anchored base callback");
            sync.redirect_url = Some(format!(
                "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
            ));
            validate_user_sync(&sync, allow_insecure).expect("anchored literal callback");
        }
    }

    #[test]
    fn literal_redirect_rejects_parser_repaired_authority_and_whitespace() {
        for base in [
            "https:///sync.example.com",
            "https:////sync.example.com",
            "https://sync.example.com/base path",
            "https://sync.example.com\\base",
        ] {
            let sync = UserSyncConfig {
                redirect_url: Some(format!(
                    "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            assert!(validate_user_sync(&sync, false).is_err(), "{base}");
        }
    }

    #[test]
    fn callback_hosts_reject_all_shared_nonpublic_ip_ranges() {
        for host in [
            "0.1.2.3",
            "[fec0::1]",
            "[64:ff9b::a00:1]",
            "[64:ff9b:1::1]",
            "[100::1]",
            "[3fff::1]",
            "[2001::1]",
            "[::ffff:8.8.8.8]",
        ] {
            for allow_insecure in [false, true] {
                let sync = UserSyncConfig {
                    external_url: format!("https://{host}"),
                    ..UserSyncConfig::default()
                };
                assert!(
                    validate_user_sync(&sync, allow_insecure).is_err(),
                    "{host}, insecure={allow_insecure}"
                );
            }
        }
    }

    #[test]
    fn literal_redirect_hosts_use_shared_nonpublic_ip_classifier() {
        for host in [
            "0.1.2.3",
            "[fec0::1]",
            "[64:ff9b::a00:1]",
            "[100::1]",
            "[3fff::1]",
        ] {
            let sync = UserSyncConfig {
                redirect_url: Some(format!(
                    "https://{host}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            assert!(validate_user_sync(&sync, false).is_err(), "{host}");
        }
    }

    #[test]
    fn callback_host_policy_keeps_exact_insecure_loopback_exceptions() {
        for host in ["127.0.0.1", "[::1]", "localhost"] {
            for scheme in ["http", "https"] {
                let sync = UserSyncConfig {
                    external_url: format!("{scheme}://{host}:9999/base/"),
                    redirect_url: Some(VALID_CALLBACK.to_owned()),
                    allowed_origins: vec![format!("{scheme}://{host}:9999")],
                    ..UserSyncConfig::default()
                };
                validate_user_sync(&sync, true).expect("exact insecure loopback exception");
                validate_user_sync(&sync, false).expect_err("secure builds reject loopback");
            }
        }
        for host in [
            "127.0.0.2",
            "[::ffff:127.0.0.1]",
            "sub.localhost",
            "127.0.0.1.nip.io",
        ] {
            let sync = UserSyncConfig {
                external_url: format!("https://{host}/base"),
                ..UserSyncConfig::default()
            };
            validate_user_sync(&sync, true).expect_err(host);
            let http = UserSyncConfig {
                external_url: format!("http://{host}"),
                ..sync
            };
            validate_user_sync(&http, true).expect_err(host);
        }
    }

    #[test]
    fn root_dotted_loopback_names_are_refused_on_each_callback_config_surface() {
        for host in [
            "localhost",
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
            let base = format!("https://{host}.:8443");
            for allow_insecure in [false, true] {
                let external = UserSyncConfig {
                    external_url: format!("{base}/base/"),
                    ..UserSyncConfig::default()
                };
                let redirect = UserSyncConfig {
                    redirect_url: Some(format!(
                        "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                    )),
                    ..UserSyncConfig::default()
                };
                let origin = UserSyncConfig {
                    allowed_origins: vec![base.clone()],
                    ..UserSyncConfig::default()
                };
                for sync in [external, redirect, origin] {
                    assert!(
                        validate_user_sync(&sync, allow_insecure).is_err(),
                        "{host}, insecure={allow_insecure}"
                    );
                }
            }
        }
    }

    #[test]
    fn callback_host_policy_checks_literal_ranges_without_dns_resolution() {
        for host in [
            "8.8.8.8",
            "[2606:4700:4700::1111]",
            "sync.example.com",
            "sync.internal",
        ] {
            let sync = UserSyncConfig {
                external_url: format!("https://{host}/base%20path/"),
                redirect_url: Some(VALID_CALLBACK.to_owned()),
                ..UserSyncConfig::default()
            };
            validate_user_sync(&sync, false).expect("public literals or DNS names");
        }
        for host in [
            "0.1.2.3",
            "[fec0::1]",
            "[64:ff9b::a00:1]",
            "[100::1]",
            "[3fff::1]",
        ] {
            let sync = UserSyncConfig {
                allowed_origins: vec![format!("https://{host}")],
                ..UserSyncConfig::default()
            };
            validate_user_sync(&sync, false).expect_err(host);
        }
    }

    #[test]
    fn cookie_names_and_optout_values_match_request_reader_grammar() {
        for byte in u8::MIN..=u8::MAX {
            let text = char::from(byte).to_string();
            let allowed_name = byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte);
            let host = HostCookieConfig {
                cookie_name: text.clone(),
                ..HostCookieConfig::default()
            };
            assert_eq!(
                validate_host_cookie(&host).is_ok(),
                allowed_name,
                "name byte {byte}"
            );
            let optout = HostCookieConfig {
                optout_cookie: Some(OptoutCookie {
                    name: text,
                    value: "1".to_owned(),
                }),
                ..HostCookieConfig::default()
            };
            assert_eq!(
                validate_host_cookie(&optout).is_ok(),
                allowed_name,
                "optout name byte {byte}"
            );

            let allowed_value =
                (b' '..=b'~').contains(&byte) && !matches!(byte, b'"' | b';' | b'\\');
            let value = HostCookieConfig {
                optout_cookie: Some(OptoutCookie {
                    name: "stop".to_owned(),
                    value: char::from(byte).to_string(),
                }),
                ..HostCookieConfig::default()
            };
            assert_eq!(
                validate_host_cookie(&value).is_ok(),
                allowed_value,
                "optout value byte {byte}"
            );
        }
    }

    #[test]
    fn callback_validation_rejects_unmatched_closes_in_every_literal_segment() {
        for callback in [
            "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&x=}}&uid={{.UserMacro}}",
            "{{.ExternalURL}}/setuid?x=}}&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "https://sync.example.test/setuid?x=}}&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
        ] {
            let sync = UserSyncConfig {
                redirect_url: Some(callback.to_owned()),
                ..UserSyncConfig::default()
            };
            assert_eq!(
                validate_user_sync(&sync, false).unwrap_err(),
                "user_sync.redirect_url has an unmatched token close"
            );
        }
    }

    const VALID_CALLBACK: &str =
        "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&gdpr={{.GDPR}}&uid={{.UserMacro}}";

    #[test]
    fn external_url_token_cannot_extend_the_callback_authority() {
        for suffix in [
            ".evil.example/setuid",
            ":8443/setuid",
            "@evil.example/setuid",
            "?x=1",
        ] {
            let sync = UserSyncConfig {
                redirect_url: Some(format!(
                    "{{{{.ExternalURL}}}}{suffix}?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            assert_eq!(
                validate_user_sync(&sync, false).unwrap_err(),
                "user_sync.redirect_url has a templated authority",
                "{suffix}"
            );
        }
        validate_user_sync(
            &UserSyncConfig {
                redirect_url: Some(VALID_CALLBACK.to_owned()),
                ..UserSyncConfig::default()
            },
            false,
        )
        .unwrap();
    }

    #[test]
    fn callback_rejects_username_password_and_fragment_independently() {
        for callback in [
            "https://name@sync.example.test/setuid?bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "https://:password@sync.example.test/setuid?bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "https://sync.example.test/setuid?bidder={{.SyncerKey}}&uid={{.UserMacro}}#fragment",
        ] {
            assert_eq!(
                validate_user_sync(
                    &UserSyncConfig {
                        redirect_url: Some(callback.to_owned()),
                        ..UserSyncConfig::default()
                    },
                    false
                )
                .unwrap_err(),
                "user_sync.redirect_url has an unsafe authority or fragment",
                "{callback}"
            );
        }
        let valid = UserSyncConfig {
            redirect_url: Some(
                "https://sync.example.test/setuid?bidder={{.SyncerKey}}&uid={{.UserMacro}}"
                    .to_owned(),
            ),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&valid, false).unwrap();
    }

    #[test]
    fn user_macro_bounds_and_reserved_syntax_are_independent() {
        let error = "userSync userMacro is unsafe or exceeds 256 bytes";
        for value in [None, Some(""), Some("$UID"), Some("{UID}"), Some("a#b")] {
            assert_eq!(validate_user_macro(value), Ok(()));
        }
        for len in [255, 256] {
            assert_eq!(validate_user_macro(Some(&"a".repeat(len))), Ok(()));
        }
        assert_eq!(
            validate_user_macro(Some(&"a".repeat(257))).unwrap_err(),
            error
        );
        for value in ["&", "=", "?", "%", "{{UID}}"] {
            assert_eq!(
                validate_user_macro(Some(value)).unwrap_err(),
                error,
                "{value}"
            );
        }
        for byte in 0_u8..=0x20 {
            let value = char::from(byte).to_string();
            assert_eq!(
                validate_user_macro(Some(&value)).unwrap_err(),
                error,
                "byte {byte}"
            );
        }
        assert_eq!(validate_user_macro(Some("\u{7f}")).unwrap_err(), error);
    }

    #[test]
    fn syncer_key_bounds_and_cookie_token_grammar_are_independent() {
        let error = "userSync key must be a bounded cookie token";
        assert_eq!(validate_syncer_key(None), Ok(()));
        assert_eq!(validate_syncer_key(Some("")).unwrap_err(), error);
        for len in [127, 128] {
            assert_eq!(validate_syncer_key(Some(&"a".repeat(len))), Ok(()));
        }
        assert_eq!(
            validate_syncer_key(Some(&"a".repeat(129))).unwrap_err(),
            error
        );
        for byte in u8::MIN..=u8::MAX {
            let value = char::from(byte).to_string();
            let allowed = byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte);
            let expected = if allowed {
                Ok(())
            } else {
                Err(error.to_owned())
            };
            assert_eq!(validate_syncer_key(Some(&value)), expected, "byte {byte}");
        }
    }

    #[test]
    fn host_cookie_defaults_and_rejects_reserved_identity() {
        let cookie = HostCookieConfig::default();
        assert_eq!(cookie.ttl_days, 90);
        validate_host_cookie(&cookie).unwrap();

        let invalid = HostCookieConfig {
            cookie_name: "uids".to_owned(),
            ..cookie
        };
        validate_host_cookie(&invalid).unwrap_err();
    }

    #[test]
    fn purpose_one_treatment_defaults_allow_both_flags() {
        let treatment = PurposeOneTreatment::default();
        assert!(treatment.enabled);
        assert!(treatment.access_allowed);
    }

    #[test]
    fn callback_origin_is_explicit_https_without_query_or_credentials() {
        let mut sync = UserSyncConfig {
            external_url: "https://sync.example.test/base".to_owned(),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&sync, false).unwrap();

        for invalid in [
            "",
            "http://sync.example.test/base",
            "https://name:password@sync.example.test/base",
            "https://sync.example.test/base?x=1",
            "https://sync.example.test/base#fragment",
        ] {
            sync.external_url = invalid.to_owned();
            validate_user_sync(&sync, false).expect_err(invalid);
        }
    }

    #[test]
    fn base_urls_reject_credentials_query_and_fragment_independently() {
        for url in [
            "https://name@sync.example.test/base",
            "https://:password@sync.example.test/base",
            "https://sync.example.test/base?query=1",
            "https://sync.example.test/base#fragment",
        ] {
            for origin in [false, true] {
                let sync = if origin {
                    UserSyncConfig {
                        allowed_origins: vec![url.to_owned()],
                        ..UserSyncConfig::default()
                    }
                } else {
                    UserSyncConfig {
                        external_url: url.to_owned(),
                        ..UserSyncConfig::default()
                    }
                };
                assert_eq!(
                    validate_user_sync(&sync, false).unwrap_err(),
                    "sync URL must have a literal host and no credentials, query, or fragment",
                    "{url}, origin={origin}"
                );
            }
        }
        validate_user_sync(&UserSyncConfig::default(), false).unwrap();
    }

    #[test]
    fn cookie_attributes_and_optout_are_validated() {
        let mut cookie = HostCookieConfig {
            domain: Some("example.test".to_owned()),
            optout_cookie: Some(OptoutCookie {
                name: "optout".to_owned(),
                value: "1".to_owned(),
            }),
            ..HostCookieConfig::default()
        };
        validate_host_cookie(&cookie).unwrap();

        cookie.domain = Some("example.test\r\nSet-Cookie: evil=1".to_owned());
        validate_host_cookie(&cookie).unwrap_err();
        cookie.domain = None;
        cookie.optout_cookie.as_mut().expect("optout").value.clear();
        validate_host_cookie(&cookie).unwrap_err();
    }

    #[test]
    fn cookie_identity_and_optout_name_limits_are_inclusive() {
        let mut cookie = HostCookieConfig {
            cookie_name: "a".repeat(128),
            family: "b".repeat(128),
            optout_cookie: Some(OptoutCookie {
                name: "c".repeat(128),
                value: "1".to_owned(),
            }),
            ..HostCookieConfig::default()
        };
        validate_host_cookie(&cookie).unwrap();

        cookie.cookie_name.push('a');
        validate_host_cookie(&cookie).unwrap_err();
        cookie.cookie_name.pop();
        cookie.family.push('b');
        validate_host_cookie(&cookie).unwrap_err();
        cookie.family.pop();
        cookie.optout_cookie.as_mut().unwrap().name.push('c');
        validate_host_cookie(&cookie).unwrap_err();
    }

    #[test]
    fn account_limits_and_empty_priority_override_are_distinct() {
        let mut account = AccountCookieSyncConfig {
            priority_groups: Some(Vec::new()),
            default_coop_sync: Some(false),
            ..AccountCookieSyncConfig::default()
        };
        assert!(account.priority_groups.is_some());
        assert_eq!(account.default_coop_sync, Some(false));
        validate_account_cookie_sync(&account).unwrap();
        account.max_limit = Some(-1);
        validate_account_cookie_sync(&account).unwrap();
        account.priority_groups = Some(vec![Vec::new(); 33]);
        validate_account_cookie_sync(&account).unwrap_err();
    }

    #[test]
    fn redirect_callback_template_is_closed_and_resolves_to_configured_origin() {
        let mut sync = UserSyncConfig {
            external_url: "https://sync.example.test/base".to_owned(),
            redirect_url: Some(
                "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&uid={{.UserMacro}}".to_owned(),
            ),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&sync, false).unwrap();
        sync.redirect_url = Some("{{.Host}}/setuid".to_owned());
        validate_user_sync(&sync, false).unwrap_err();
        sync.redirect_url = Some("{{.ExternalURL}}/setuid?account=stale".to_owned());
        validate_user_sync(&sync, false).unwrap_err();
    }

    #[test]
    fn production_callback_origin_rejects_private_and_loopback_literals() {
        let mut sync = UserSyncConfig::default();
        for origin in [
            "https://127.0.0.1",
            "https://10.1.2.3",
            "https://100.64.0.1",
            "https://192.0.2.1",
            "https://[::1]",
            "https://localhost",
        ] {
            sync.external_url = origin.to_owned();
            validate_user_sync(&sync, false).expect_err(origin);
        }
        sync.external_url = "http://127.0.0.1:9999".to_owned();
        validate_user_sync(&sync, true).unwrap();
        for origin in [
            "http://127.0.0.2:9999",
            "https://127.0.0.2:9999",
            "http://sub.localhost:9999",
            "https://127.0.0.1.nip.io",
        ] {
            sync.external_url = origin.to_owned();
            validate_user_sync(&sync, true).expect_err(origin);
        }
    }

    #[test]
    fn catalog_ceiling_counts_bidders_and_aliases_together() {
        for (bidders, aliases) in [(511, 0), (512, 0), (0, 511), (0, 512)] {
            assert_eq!(
                validate_catalog_size(bidders, aliases),
                Ok(()),
                "{bidders} bidders plus {aliases} aliases"
            );
        }
    }

    #[test]
    fn catalog_ceiling_accepts_447_and_448_compiled_names_plus_64_aliases() {
        for bidders in [447, 448] {
            assert_eq!(validate_catalog_size(bidders, 64), Ok(()), "{bidders}");
        }
    }

    #[test]
    fn catalog_ceiling_rejects_513_names_with_exact_diagnostic() {
        for (bidders, aliases) in [(513, 0), (0, 513), (449, 64)] {
            assert_eq!(
                validate_catalog_size(bidders, aliases),
                Err("user-sync catalog exceeds 512 names".to_owned()),
                "{bidders} bidders plus {aliases} aliases"
            );
        }
    }

    #[test]
    fn catalog_ceiling_rejects_checked_add_overflow_with_exact_diagnostic() {
        for (bidders, aliases) in [(usize::MAX, 1), (1, usize::MAX)] {
            assert_eq!(
                validate_catalog_size(bidders, aliases),
                Err("user-sync catalog exceeds 512 names".to_owned()),
                "{bidders} bidders plus {aliases} aliases"
            );
        }
    }

    #[test]
    fn skip_when_is_bounded_and_keeps_exact_strings() {
        validate_skip_when(&SkipWhen {
            gdpr: false,
            gpp_sid: vec!["2".to_owned(); 256],
        })
        .unwrap();
        validate_skip_when(&SkipWhen {
            gdpr: true,
            gpp_sid: vec!["02,6".to_owned(), "-1".to_owned()],
        })
        .unwrap();
        validate_skip_when(&SkipWhen {
            gdpr: false,
            gpp_sid: vec!["2".to_owned(); 257],
        })
        .unwrap_err();
        validate_skip_when(&SkipWhen {
            gdpr: false,
            gpp_sid: vec!["x".repeat(129)],
        })
        .unwrap_err();
    }

    #[test]
    fn host_and_account_priority_collection_ceiling_is_inclusive() {
        for entries in [255, 256, 257] {
            let groups = vec![vec!["fake_fixture_base".to_owned(); entries]];
            let host = UserSyncConfig {
                priority_groups: groups.clone(),
                ..UserSyncConfig::default()
            };
            let account = AccountCookieSyncConfig {
                priority_groups: Some(groups),
                ..AccountCookieSyncConfig::default()
            };
            let expected = if entries <= 256 {
                Ok(())
            } else {
                Err("user-sync priority groups exceed bounds".to_owned())
            };
            assert_eq!(validate_user_sync(&host, false), expected, "{entries}");
            assert_eq!(
                validate_account_cookie_sync(&account),
                expected,
                "{entries}"
            );
        }
    }

    #[test]
    fn host_and_account_priority_group_ceiling_is_independent_of_entry_ceiling() {
        let within = vec![vec!["fake_fixture_base".to_owned()]; 32];
        let mut host = UserSyncConfig {
            priority_groups: within.clone(),
            ..UserSyncConfig::default()
        };
        let mut account = AccountCookieSyncConfig {
            priority_groups: Some(within),
            ..AccountCookieSyncConfig::default()
        };
        validate_user_sync(&host, false).unwrap();
        validate_account_cookie_sync(&account).unwrap();

        let invalid = vec![Vec::new(); 33];
        host.priority_groups = invalid.clone();
        account.priority_groups = Some(invalid);
        let error = Err("user-sync priority groups exceed bounds".to_owned());
        assert_eq!(validate_user_sync(&host, false), error);
        assert_eq!(validate_account_cookie_sync(&account), error);
    }

    #[test]
    fn allowed_origins_collection_ceiling_is_inclusive() {
        let mut sync = UserSyncConfig {
            allowed_origins: (0_usize..256_usize)
                .map(|index| format!("https://sync{index}.example.test"))
                .collect(),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&sync, false).unwrap();
        sync.allowed_origins
            .push("https://overflow.example.test".to_owned());
        validate_user_sync(&sync, false).unwrap_err();
    }

    #[test]
    fn callback_origin_and_redirect_template_string_ceilings_are_inclusive() {
        let origin_prefix = "https://sync.example.test/";
        let mut sync = UserSyncConfig {
            external_url: format!("{origin_prefix}{}", "x".repeat(2_048 - origin_prefix.len())),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&sync, false).unwrap();
        sync.external_url.push('x');
        validate_user_sync(&sync, false).unwrap_err();

        sync.external_url = origin_prefix.to_owned();
        let uid_suffix = "?bidder={{.SyncerKey}}&uid={{.UserMacro}}";
        sync.redirect_url = Some(format!(
            "{origin_prefix}{}{uid_suffix}",
            "x".repeat(8_192 - origin_prefix.len() - uid_suffix.len())
        ));
        validate_user_sync(&sync, false).unwrap();
        sync.redirect_url.as_mut().unwrap().push('x');
        validate_user_sync(&sync, false).unwrap_err();
    }

    #[test]
    fn secure_callback_origins_reject_mapped_ipv4_hosts() {
        let mut sync = UserSyncConfig::default();
        for origin in [
            "https://[::ffff:127.0.0.1]",
            "https://[::ffff:10.1.2.3]",
            "https://[::ffff:192.168.1.1]",
        ] {
            sync.external_url = origin.to_owned();
            assert_eq!(
                validate_user_sync(&sync, false).unwrap_err(),
                "sync URL cannot use a private literal host",
                "{origin}"
            );
        }

        sync.external_url = "https://sync.example.test".to_owned();
        sync.redirect_url = Some("https://[::ffff:127.0.0.1]/setuid?uid={{.UserMacro}}".to_owned());
        assert_eq!(
            validate_user_sync(&sync, false).unwrap_err(),
            "user_sync.redirect_url cannot use a private literal host"
        );

        sync.redirect_url = None;
        sync.allowed_origins = vec!["https://[::ffff:10.1.2.3]".to_owned()];
        assert_eq!(
            validate_user_sync(&sync, false).unwrap_err(),
            "sync URL cannot use a private literal host"
        );

        sync.allowed_origins.clear();
        sync.external_url = "https://[::ffff:8.8.8.8]".to_owned();
        validate_user_sync(&sync, false).expect_err("shared classifier rejects all mapped IPv4");
    }

    #[test]
    fn callback_uid_structure_is_required_and_unique_during_config_validation() {
        let mut sync = UserSyncConfig {
            redirect_url: Some(VALID_CALLBACK.to_owned()),
            ..UserSyncConfig::default()
        };
        validate_user_sync(&sync, false).expect("terminal UID macro is valid");

        for invalid in [
            "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}",
            "{{.ExternalURL}}/setuid?uid=static",
            "{{.ExternalURL}}/setuid?uid={{.UserMacro}}&uid={{.UserMacro}}",
            "{{.ExternalURL}}/setuid?uid={{.UserMacro}}&f=i",
            "{{.ExternalURL}}/setuid?%75id={{.UserMacro}}",
            "{{.ExternalURL}}/setuid?uid={{.UserMacro}}&%75id=other",
            "{{.ExternalURL}}/setuid?uid={{.UserMacro}}&other={{.UserMacro}}",
            "{{.ExternalURL}}/setuid?uid={{.UserMacro}}/tail",
        ] {
            sync.redirect_url = Some(invalid.to_owned());
            assert_eq!(
                validate_user_sync(&sync, false).unwrap_err(),
                "user_sync.redirect_url must end in one uid macro field",
                "{invalid}"
            );
        }

        sync.redirect_url = None;
        validate_user_sync(&sync, false).expect("omitted template uses the valid default");
    }
}

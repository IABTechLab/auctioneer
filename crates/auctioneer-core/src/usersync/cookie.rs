//! Bounded, reference-compatible UID cookie codec and request read model.

use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Write as _},
    str,
};

use auctioneer_bidder::{CappedWriter, accepts_sync_uid};
use base64::{
    Engine as _, alphabet,
    engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig, URL_SAFE},
};
use cookie::{Cookie, SameSite};
use http::{HeaderMap, HeaderName, HeaderValue, header::COOKIE};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use super::limits::{CookieLimitError, CookieLimits};

const ENTRY_TTL_DAYS: i64 = 14;

#[derive(Clone, Eq, PartialEq)]
pub struct UidEntry {
    uid: String,
    expires: OffsetDateTime,
    expires_wire: String,
}

impl fmt::Debug for UidEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UidEntry")
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

impl UidEntry {
    #[must_use]
    pub fn uid(&self) -> &str {
        &self.uid
    }
    #[must_use]
    pub fn expires(&self) -> OffsetDateTime {
        self.expires
    }
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct UidCookie {
    entries: BTreeMap<String, UidEntry>,
    optout: bool,
}

impl fmt::Debug for UidCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UidCookie")
            .field("entry_count", &self.entries.len())
            .field("optout", &self.optout)
            .finish()
    }
}

impl UidCookie {
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<String, UidEntry> {
        &self.entries
    }
    #[must_use]
    pub fn allow_syncs(&self) -> bool {
        !self.optout
    }

    #[must_use]
    pub fn opted_out() -> Self {
        Self {
            entries: BTreeMap::new(),
            optout: true,
        }
    }

    #[must_use]
    pub fn uid(&self, family: &str) -> Option<&str> {
        let entry = self.entries.get(family)?;
        accepts_sync_uid(family, &entry.uid).then_some(entry.uid.as_str())
    }

    #[must_use]
    pub fn uids(&self) -> BTreeMap<&str, &str> {
        self.entries
            .keys()
            .filter_map(|family| self.uid(family).map(|uid| (family.as_str(), uid)))
            .collect()
    }

    #[must_use]
    pub fn is_live(&self, family: &str, now: OffsetDateTime) -> bool {
        self.uid(family).is_some()
            && self
                .entries
                .get(family)
                .is_some_and(|entry| now < entry.expires)
    }

    #[must_use]
    pub fn has_live_syncs(&self, now: OffsetDateTime) -> bool {
        self.entries.keys().any(|family| self.is_live(family, now))
    }

    /// # Errors
    /// Returns `CookieLimitError` if inserting a missing host UID exceeds the
    /// retained byte/node limits or its expiry cannot be represented.
    pub fn fill_host(
        &mut self,
        family: &str,
        uid: &str,
        now: OffsetDateTime,
        limits: &CookieLimits,
    ) -> Result<(), CookieLimitError> {
        if self.optout
            || family.is_empty()
            || uid.is_empty()
            || self
                .entries
                .get(family)
                .is_some_and(|entry| !entry.uid.is_empty())
        {
            return Ok(());
        }
        self.set_uid(family, uid, now, limits)
    }

    /// # Errors
    /// Returns `CookieLimitError` on retained byte/node overflow or an
    /// unrepresentable expiry, before inserting the new entry.
    pub fn set_uid(
        &mut self,
        family: &str,
        uid: &str,
        now: OffsetDateTime,
        limits: &CookieLimits,
    ) -> Result<(), CookieLimitError> {
        if self.optout {
            return Ok(());
        }
        if uid.is_empty() || !accepts_sync_uid(family, uid) {
            self.entries.remove(family);
            return Ok(());
        }
        if family.len() > limits.max_decoded_bytes || uid.len() > limits.max_decoded_bytes {
            return Err(CookieLimitError);
        }
        let expires = now
            .checked_add(Duration::days(ENTRY_TTL_DAYS))
            .ok_or(CookieLimitError)?;
        let expires_wire = expires
            .format(&Rfc3339)
            .map_err(|_error| CookieLimitError)?;
        let other_bytes = self
            .entries
            .iter()
            .filter(|(key, _)| key.as_str() != family)
            .try_fold(0_usize, |total, (key, entry)| {
                total
                    .checked_add(key.len())?
                    .checked_add(entry.uid.len())?
                    .checked_add(entry.expires_wire.len())
            })
            .ok_or(CookieLimitError)?;
        let total_bytes = other_bytes
            .checked_add(family.len())
            .and_then(|n| n.checked_add(uid.len()))
            .and_then(|n| n.checked_add(expires_wire.len()))
            .ok_or(CookieLimitError)?;
        let count = self
            .entries
            .len()
            .checked_add(usize::from(!self.entries.contains_key(family)))
            .ok_or(CookieLimitError)?;
        if total_bytes > limits.max_decoded_bytes
            || count > limits.max_json_nodes.saturating_sub(3).div_euclid(6)
        {
            return Err(CookieLimitError);
        }
        self.entries.insert(
            family.to_owned(),
            UidEntry {
                uid: uid.to_owned(),
                expires,
                expires_wire,
            },
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeOutcome {
    pub cookie: UidCookie,
    pub corrupt: bool,
}

impl DecodeOutcome {
    fn valid(cookie: UidCookie) -> Self {
        Self {
            cookie,
            corrupt: false,
        }
    }
    fn corrupt() -> Self {
        Self {
            cookie: UidCookie::default(),
            corrupt: true,
        }
    }
}

/// # Errors
/// Returns `CookieLimitError` when encoded/decoded bytes or JSON nodes exceed
/// their limits. Corrupt cookie data instead yields a fresh, corrupt outcome.
pub fn decode_cookie(
    encoded: &str,
    limits: &CookieLimits,
) -> Result<DecodeOutcome, CookieLimitError> {
    if encoded.len() > limits.max_encoded_bytes {
        return Err(CookieLimitError);
    }
    let compact: Vec<u8> = encoded
        .bytes()
        .filter(|b| *b != b'\r' && *b != b'\n')
        .collect();
    let estimated = compact
        .len()
        .checked_div(4)
        .and_then(|n| n.checked_mul(3))
        .ok_or(CookieLimitError)?;
    if estimated > limits.max_decoded_bytes.saturating_add(2) {
        return Err(CookieLimitError);
    }
    let engine = GeneralPurpose::new(
        &alphabet::URL_SAFE,
        GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
    );
    let Ok(decoded) = engine.decode(&compact) else {
        return Ok(DecodeOutcome::corrupt());
    };
    if decoded.len() > limits.max_decoded_bytes {
        return Err(CookieLimitError);
    }

    let mut remaining = limits.max_json_nodes;
    let mut exceeded = false;
    let mut preflight = serde_json::Deserializer::from_slice(&decoded);
    let counted = (Counted {
        remaining: &mut remaining,
        exceeded: &mut exceeded,
    })
    .deserialize(&mut preflight);
    if exceeded {
        return Err(CookieLimitError);
    }
    if counted.is_err() || preflight.end().is_err() {
        return Ok(DecodeOutcome::corrupt());
    }
    let raw: RawCookie = match serde_json::from_slice(&decoded) {
        Ok(value) => value,
        Err(_) => return Ok(DecodeOutcome::corrupt()),
    };
    let mut cookie = UidCookie {
        optout: raw.optout,
        ..UidCookie::default()
    };
    for (family, raw_entry) in raw.entries {
        let uid = raw_entry.uid;
        let expires = raw_entry
            .expires
            .unwrap_or(time::macros::datetime!(0001-01-01 0:00 UTC));
        if cookie.optout || !accepts_sync_uid(&family, &uid) {
            continue;
        }
        let expires_wire = expires
            .format(&Rfc3339)
            .map_err(|_error| CookieLimitError)?;
        cookie.entries.insert(
            family,
            UidEntry {
                uid,
                expires,
                expires_wire,
            },
        );
    }
    Ok(DecodeOutcome::valid(cookie))
}

/// # Errors
/// Returns `CookieLimitError` if serialization, HTML escaping, or base64 output
/// exceeds the configured decoded/encoded byte limits or cannot be serialized.
pub fn encode_cookie(
    cookie: &UidCookie,
    limits: &CookieLimits,
) -> Result<String, CookieLimitError> {
    #[derive(Serialize)]
    struct WireEntry<'a> {
        uid: &'a str,
        expires: &'a str,
    }
    #[derive(Serialize)]
    struct WireCookie<'a> {
        #[serde(rename = "tempUIDs", skip_serializing_if = "BTreeMap::is_empty")]
        entries: BTreeMap<&'a str, WireEntry<'a>>,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        optout: bool,
    }
    let entries = if cookie.optout {
        BTreeMap::new()
    } else {
        cookie
            .entries
            .iter()
            .map(|(family, entry)| {
                (
                    family.as_str(),
                    WireEntry {
                        uid: &entry.uid,
                        expires: &entry.expires_wire,
                    },
                )
            })
            .collect()
    };
    let wire = WireCookie {
        entries,
        optout: cookie.optout,
    };
    let mut raw = BoundedWriter::new(limits.max_decoded_bytes);
    serde_json::to_writer(&mut raw, &wire).map_err(|_error| CookieLimitError)?;
    let text = str::from_utf8(&raw.bytes).map_err(|_error| CookieLimitError)?;
    let mut escaped = BoundedWriter::new(limits.max_decoded_bytes);
    for ch in text.chars() {
        let replacement = match ch {
            '<' => Some("\\u003c"),
            '>' => Some("\\u003e"),
            '&' => Some("\\u0026"),
            '\u{2028}' => Some("\\u2028"),
            '\u{2029}' => Some("\\u2029"),
            _ => None,
        };
        if let Some(value) = replacement {
            escaped
                .write_all(value.as_bytes())
                .map_err(|_error| CookieLimitError)?;
        } else {
            let mut buffer = [0; 4];
            escaped
                .write_all(ch.encode_utf8(&mut buffer).as_bytes())
                .map_err(|_error| CookieLimitError)?;
        }
    }
    let encoded_len = escaped
        .bytes
        .len()
        .div_ceil(3)
        .checked_mul(4)
        .ok_or(CookieLimitError)?;
    if encoded_len > limits.max_encoded_bytes {
        return Err(CookieLimitError);
    }
    Ok(URL_SAFE.encode(&escaped.bytes))
}

struct BoundedWriter {
    bytes: Vec<u8>,
    max: usize,
}

impl BoundedWriter {
    fn new(max: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max,
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "default Write helpers delegate to the bounded write method; flush has no buffered side effects"
)]
impl io::Write for BoundedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.max.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("cookie output limit"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub const MAX_SET_COOKIE_FIELD_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy)]
pub struct CookieWriteOptions<'a> {
    pub name: &'a str,
    pub domain: &'a str,
    pub ttl_days: i64,
    pub user_agent: Option<&'a str>,
    pub hard_cap_bytes: usize,
}

impl Default for CookieWriteOptions<'_> {
    fn default() -> Self {
        Self {
            name: "uids",
            domain: "",
            ttl_days: 90,
            user_agent: None,
            hard_cap_bytes: MAX_SET_COOKIE_FIELD_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CookieWriteError {
    #[error("cookie field exceeds hard limit")]
    HardLimit,
    #[error("invalid cookie attributes")]
    InvalidAttributes,
    #[error("cookie expiry out of range")]
    ExpiryOverflow,
    #[error("cookie codec resource limit exceeded")]
    CodecLimit,
    #[error("cookie field allocation failed")]
    Allocation,
    #[error("cookie write forbidden by opt-out")]
    OptedOut,
    #[error("invalid cookie priority configuration")]
    InvalidPriorities,
}

pub struct PreparedCookie {
    field_value: HeaderValue,
}

impl fmt::Debug for PreparedCookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedCookie")
            .field("field_bytes", &self.field_value.as_bytes().len())
            .finish()
    }
}

impl PreparedCookie {
    pub fn field_value(&self) -> &HeaderValue {
        &self.field_value
    }

    pub fn into_header_value(self) -> HeaderValue {
        self.field_value
    }
}

/// # Errors
/// Rejects invalid cookie attributes, an unrepresentable expiry, a complete
/// serialized field exceeding the hard byte limit, or a field allocation failure.
pub fn prepare_set_cookie_value(
    encoded: &str,
    options: &CookieWriteOptions<'_>,
    now: OffsetDateTime,
) -> Result<PreparedCookie, CookieWriteError> {
    if options.name.is_empty() || HeaderName::from_bytes(options.name.as_bytes()).is_err() {
        return Err(CookieWriteError::InvalidAttributes);
    }
    if !options.domain.is_empty() && !valid_cookie_domain(options.domain) {
        return Err(CookieWriteError::InvalidAttributes);
    }
    if !encoded
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'=' | b'.'))
    {
        return Err(CookieWriteError::InvalidAttributes);
    }
    let hard_cap = options.hard_cap_bytes.min(MAX_SET_COOKIE_FIELD_BYTES);
    // The final field check is authoritative; this preflight avoids formatting
    // an input already known to exceed the allocation/output allowance. The
    // allocation suite checks this path with both small and large rejected
    // values, separately from the final complete-field boundary tests.
    let minimum = options
        .name
        .len()
        .checked_add(options.domain.len())
        .and_then(|len| len.checked_add(encoded.len()))
        .ok_or(CookieWriteError::HardLimit)?;
    if minimum > hard_cap {
        return Err(CookieWriteError::HardLimit);
    }
    let seconds = options
        .ttl_days
        .checked_mul(86_400)
        .ok_or(CookieWriteError::ExpiryOverflow)?;
    let expires = now
        .checked_add(Duration::seconds(seconds))
        .ok_or(CookieWriteError::ExpiryOverflow)?;
    let mut builder = Cookie::build((options.name, encoded))
        .path("/")
        .expires(expires);
    if encoded.is_empty() {
        builder = builder
            .max_age(Duration::ZERO)
            .expires(OffsetDateTime::UNIX_EPOCH);
    }
    if !options.domain.is_empty() {
        builder = builder.domain(options.domain);
    }
    if chrome_same_site_none(options.user_agent) {
        builder = builder.secure(true).same_site(SameSite::None);
    }
    let cookie = builder.build();
    let mut field = Vec::new();
    field
        .try_reserve_exact(hard_cap)
        .map_err(|_allocation| CookieWriteError::Allocation)?;
    let mut writer = CappedWriter::new(&mut field, hard_cap)
        .map_err(|_capacity| CookieWriteError::Allocation)?;
    // Keep the library's complete cookie grammar, but never grow its field buffer.
    write!(&mut writer, "{cookie}").map_err(|error| {
        if error.kind() == io::ErrorKind::WriteZero {
            CookieWriteError::HardLimit
        } else {
            CookieWriteError::InvalidAttributes
        }
    })?;
    let field_value =
        HeaderValue::from_bytes(&field).map_err(|_error| CookieWriteError::InvalidAttributes)?;
    Ok(PreparedCookie { field_value })
}

fn valid_cookie_domain(domain: &str) -> bool {
    let labels = domain.strip_prefix('.').unwrap_or(domain);
    !labels.is_empty()
        && labels.split('.').all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn chrome_same_site_none(user_agent: Option<&str>) -> bool {
    let Some(agent) = user_agent else {
        return false;
    };
    for marker in ["Chrome/", "CriOS/"] {
        if let Some((_, rest)) = agent.split_once(marker) {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            let next = rest.as_bytes().get(digits).copied();
            if digits == 0 || !matches!(next, None | Some(b'.' | b' ')) {
                return false;
            }
            return rest
                .get(..digits)
                .is_some_and(|number| number.parse::<u32>().is_ok_and(|major| major >= 67));
        }
    }
    false
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HostCookie<'a> {
    pub cookie_name: &'a str,
    pub family: &'a str,
    pub optout_name: &'a str,
    pub optout_value: &'a str,
}

/// # Errors
/// Returns `CookieLimitError` on excessive input headers, cookie bytes/nodes,
/// or an over-limit host UID merge.
/// This is payload-codec qualification, not cookie authentication. HTTP UID
/// readers must use `CookieKeyring::read_cookie`; an empty name checks only opt-out.
pub fn read_cookie(
    headers: &HeaderMap,
    cookie_name: &str,
    host: HostCookie<'_>,
    now: OffsetDateTime,
    limits: &CookieLimits,
) -> Result<DecodeOutcome, CookieLimitError> {
    read_cookie_with_decoder(headers, cookie_name, host, now, limits, decode_cookie)
}

pub(super) fn read_cookie_with_decoder<E: From<CookieLimitError>>(
    headers: &HeaderMap,
    cookie_name: &str,
    host: HostCookie<'_>,
    now: OffsetDateTime,
    limits: &CookieLimits,
    decode: impl FnOnce(&str, &CookieLimits) -> Result<DecodeOutcome, E>,
) -> Result<DecodeOutcome, E> {
    let mut count = 0_usize;
    let mut bytes = 0_usize;
    for header in headers.get_all(COOKIE) {
        count = count.checked_add(1).ok_or(CookieLimitError)?;
        bytes = bytes
            .checked_add(header.as_bytes().len())
            .ok_or(CookieLimitError)?;
        if count > limits.max_header_count || bytes > limits.max_header_bytes {
            return Err(CookieLimitError.into());
        }
    }
    if !host.optout_name.is_empty()
        && first_value(headers, host.optout_name, limits.max_decoded_bytes)?.as_deref()
            == Some(host.optout_value)
    {
        return Ok(DecodeOutcome::valid(UidCookie::opted_out()));
    }
    let mut result = match first_value(headers, cookie_name, limits.max_encoded_bytes)? {
        Some(value) => decode(&value, limits)?,
        None => DecodeOutcome::valid(UidCookie::default()),
    };
    if result.cookie.allow_syncs()
        && !host.cookie_name.is_empty()
        && let Some(value) = first_value(headers, host.cookie_name, limits.max_decoded_bytes)?
    {
        result.cookie.fill_host(host.family, &value, now, limits)?;
    }
    Ok(result)
}

fn first_value(
    headers: &HeaderMap,
    name: &str,
    max_value_bytes: usize,
) -> Result<Option<String>, CookieLimitError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
    {
        return Ok(None);
    }
    for header in headers.get_all(COOKIE) {
        for raw_part in header.as_bytes().split(|byte| *byte == b';') {
            let Ok(text) = str::from_utf8(raw_part) else {
                continue;
            };
            let part = text.trim_matches([' ', '\t']);
            let (candidate, raw_value) = part.split_once('=').unwrap_or((part, ""));
            if candidate.trim_matches([' ', '\t']) != name {
                continue;
            }
            // Go's request grammar preserves leading value spaces, permits
            // spaces/commas, and strips only one matching pair of quotes.
            let value = raw_value
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
                .unwrap_or(raw_value);
            if !value
                .bytes()
                .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b'"' | b';' | b'\\'))
            {
                continue;
            }
            if value.len() > max_value_bytes {
                return Err(CookieLimitError);
            }
            return Ok(Some(value.to_owned()));
        }
    }
    Ok(None)
}

#[derive(Default)]
struct RawEntry {
    uid: String,
    expires: Option<OffsetDateTime>,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Serde's default deserialize_in_place uses the same validated replacement semantics"
)]
impl<'de> Deserialize<'de> for RawEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntryVisitor;
        #[expect(
            clippy::missing_trait_methods,
            reason = "only object and null entries are accepted; Serde defaults reject other value types"
        )]
        impl<'de> Visitor<'de> for EntryVisitor {
            type Value = RawEntry;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a UID entry object or null")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(RawEntry::default())
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut entry = RawEntry::default();
                while let Some(key) = map.next_key::<String>()? {
                    if key.eq_ignore_ascii_case("uid") {
                        entry.uid = map.next_value::<Option<String>>()?.unwrap_or_default();
                    } else if key.eq_ignore_ascii_case("expires") {
                        // Parse before any duplicate entry, map overwrite, or opt-out can discard it.
                        if let Some(expires) = map.next_value::<Option<ParsedTimestamp>>()? {
                            entry.expires = Some(expires.0);
                        }
                    } else {
                        map.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(entry)
            }
        }
        deserializer.deserialize_any(EntryVisitor)
    }
}

struct ParsedTimestamp(OffsetDateTime);

#[expect(
    clippy::missing_trait_methods,
    reason = "Serde's default deserialize_in_place uses the same timestamp validation"
)]
impl<'de> Deserialize<'de> for ParsedTimestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TimestampVisitor;
        #[expect(
            clippy::missing_trait_methods,
            reason = "only strings are accepted; Serde defaults forward string variants and reject other types"
        )]
        impl Visitor<'_> for TimestampVisitor {
            type Value = ParsedTimestamp;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an RFC3339 timestamp")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                OffsetDateTime::parse(v, &Rfc3339)
                    .map(ParsedTimestamp)
                    .map_err(|_error| E::custom("invalid RFC3339 timestamp"))
            }
        }
        deserializer.deserialize_str(TimestampVisitor)
    }
}

struct RawEntries<'a>(&'a mut BTreeMap<String, RawEntry>);

impl<'de> DeserializeSeed<'de> for RawEntries<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "only object and null maps are accepted; Serde defaults reject other value types"
)]
impl<'de> Visitor<'de> for RawEntries<'_> {
    type Value = ();
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a UID family map or null")
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        self.0.clear();
        Ok(())
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
        while let Some((family, entry)) = map.next_entry::<String, RawEntry>()? {
            self.0.insert(family, entry);
        }
        Ok(())
    }
}

#[derive(Default)]
struct RawCookie {
    entries: BTreeMap<String, RawEntry>,
    optout: bool,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Serde's default deserialize_in_place preserves the same validated replacement semantics"
)]
impl<'de> Deserialize<'de> for RawCookie {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RawVisitor;
        #[expect(
            clippy::missing_trait_methods,
            reason = "only object and null cookies are accepted; Serde defaults reject other value types"
        )]
        impl<'de> Visitor<'de> for RawVisitor {
            type Value = RawCookie;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a UID cookie object or null")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(RawCookie::default())
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
                let mut raw = RawCookie::default();
                while let Some(key) = map.next_key::<String>()? {
                    if key.eq_ignore_ascii_case("tempUIDs") {
                        map.next_value_seed(RawEntries(&mut raw.entries))?;
                    } else if key.eq_ignore_ascii_case("optout") {
                        if let Some(optout) = map.next_value::<Option<bool>>()? {
                            raw.optout = optout;
                        }
                    } else {
                        map.next_value::<IgnoredAny>()?;
                    }
                }
                Ok(raw)
            }
        }
        deserializer.deserialize_any(RawVisitor)
    }
}

struct Counted<'a> {
    remaining: &'a mut usize,
    exceeded: &'a mut bool,
}

impl<'de> DeserializeSeed<'de> for Counted<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        struct CountVisitor<'a> {
            remaining: &'a mut usize,
            exceeded: &'a mut bool,
        }
        #[expect(
            clippy::missing_trait_methods,
            reason = "Serde defaults forward narrower scalar types to these JSON visitor methods and reject unsupported types"
        )]
        impl<'de> Visitor<'de> for CountVisitor<'_> {
            type Value = ();
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("JSON value")
            }
            fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
                Ok(())
            }
            fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
                Ok(())
            }
            fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
                Ok(())
            }
            fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
                Ok(())
            }
            fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
                Ok(())
            }
            fn visit_string<E: de::Error>(self, _: String) -> Result<(), E> {
                Ok(())
            }
            fn visit_unit<E: de::Error>(self) -> Result<(), E> {
                Ok(())
            }
            fn visit_none<E: de::Error>(self) -> Result<(), E> {
                Ok(())
            }
            fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
                Counted {
                    remaining: self.remaining,
                    exceeded: self.exceeded,
                }
                .deserialize(deserializer)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
                while seq
                    .next_element_seed(Counted {
                        remaining: self.remaining,
                        exceeded: self.exceeded,
                    })?
                    .is_some()
                {}
                Ok(())
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
                while map.next_key::<IgnoredAny>()?.is_some() {
                    if *self.remaining == 0 {
                        *self.exceeded = true;
                        return Err(de::Error::custom("node limit"));
                    }
                    *self.remaining = self.remaining.saturating_sub(1);
                    map.next_value_seed(Counted {
                        remaining: self.remaining,
                        exceeded: self.exceeded,
                    })?;
                }
                Ok(())
            }
        }
        if *self.remaining == 0 {
            *self.exceeded = true;
            return Err(de::Error::custom("node limit"));
        }
        *self.remaining = self.remaining.saturating_sub(1);
        deserializer.deserialize_any(CountVisitor {
            remaining: self.remaining,
            exceeded: self.exceeded,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-09-30T12:00:00Z", &Rfc3339).unwrap()
    }

    fn decode_json(raw: &str) -> DecodeOutcome {
        decode_cookie(&URL_SAFE.encode(raw), &CookieLimits::default()).unwrap()
    }

    // Raw JSON below preserves duplicate/case-aliased keys and null update order;
    // json! would collapse these fields before the codec can visit them.
    #[test]
    fn cookie_field_names_are_case_insensitive() {
        for key in ["OPTOUT", "OptOut", "optOUT"] {
            let decoded = decode_json(&json!({key: true}).to_string());
            assert!(!decoded.corrupt, "{key}");
            assert!(!decoded.cookie.allow_syncs(), "{key}");
        }
        let decoded = decode_json(
            &json!({
                "TEMPuids": {"FaM_A": {"UID": "one", "EXPIRES": "2026-10-14T12:00:00Z"}}
            })
            .to_string(),
        );
        assert!(!decoded.corrupt);
        assert_eq!(decoded.cookie.uid("FaM_A"), Some("one"));
        assert_eq!(decoded.cookie.uid("fam_a"), None);
        assert!(decoded.cookie.is_live("FaM_A", now()));
    }

    #[test]
    fn optout_null_preserves_previous_boolean() {
        for raw in [
            r#"{"optout":true,"optout":null}"#,
            r#"{"optout":true,"OPTOUT":null}"#,
            r#"{"optout":null,"optout":true,"optout":null}"#,
        ] {
            let decoded = decode_json(raw);
            assert!(!decoded.corrupt, "{raw}");
            assert!(!decoded.cookie.allow_syncs(), "{raw}");
        }
        let decoded = decode_json(r#"{"optout":true,"optout":false,"optout":null}"#);
        assert!(!decoded.corrupt);
        assert!(decoded.cookie.allow_syncs());
    }

    #[test]
    fn duplicate_entry_fields_do_not_reset_valid_optout() {
        for entry in [
            r#"{"uid":"one","uid":"two"}"#,
            r#"{"expires":"2026-10-14T12:00:00Z","expires":"2026-10-15T12:00:00Z"}"#,
            r#"{"uid":"one","uid":null,"expires":"2026-10-14T12:00:00Z","expires":null}"#,
        ] {
            let raw = format!(r#"{{"optout":true,"tempUIDs":{{"a":{entry}}}}}"#);
            let decoded = decode_json(&raw);
            assert!(!decoded.corrupt, "{raw}");
            assert!(!decoded.cookie.allow_syncs(), "{raw}");
            assert!(decoded.cookie.entries().is_empty(), "{raw}");
        }
    }

    #[test]
    fn duplicate_entry_fields_apply_reference_null_semantics() {
        for (entry, uid, expiry) in [
            (
                r#"{"uid":"one","uid":"two","expires":"2026-10-14T12:00:00Z","expires":"2026-10-15T12:00:00Z"}"#,
                "two",
                "2026-10-15T12:00:00Z",
            ),
            (
                r#"{"uid":"one","uid":null,"expires":"2026-10-14T12:00:00Z","expires":null}"#,
                "",
                "2026-10-14T12:00:00Z",
            ),
            (
                r#"{"UID":"one","uid":"two","EXPIRES":null}"#,
                "two",
                "0001-01-01T00:00:00Z",
            ),
        ] {
            let raw = format!(r#"{{"tempUIDs":{{"a":{entry}}}}}"#);
            let decoded = decode_json(&raw);
            assert!(!decoded.corrupt, "{raw}");
            assert_eq!(decoded.cookie.uid("a"), Some(uid), "{raw}");
            assert_eq!(
                decoded.cookie.entries()["a"].expires(),
                OffsetDateTime::parse(expiry, &Rfc3339).unwrap(),
                "{raw}"
            );
        }
    }

    #[test]
    fn discarded_malformed_timestamps_invalidate_entire_cookie() {
        let policy_independent = json!({
            "tempUIDs": {"fake_bidder": {"uid": "0", "expires": "bad"}}
        })
        .to_string();
        for raw in [
            r#"{"tempUIDs":{"a":{"expires":"bad"},"a":{"expires":"2026-10-14T12:00:00Z"}}}"#,
            r#"{"tempUIDs":{"a":{"expires":"bad"},"a":null}}"#,
            r#"{"tempUIDs":{"a":{"expires":"bad"}},"tempUIDs":{"a":{"expires":"2026-10-14T12:00:00Z"}}}"#,
            r#"{"tempUIDs":{"a":{"expires":"bad"}},"tempUIDs":null}"#,
            r#"{"tempUIDs":{"a":{"expires":"bad","expires":"2026-10-14T12:00:00Z"}}}"#,
            r#"{"tempUIDs":{"a":{"expires":"bad","expires":null}}}"#,
            r#"{"optout":true,"tempUIDs":{"a":{"expires":"bad"},"a":{}}}"#,
            policy_independent.as_str(),
        ] {
            let decoded = decode_json(raw);
            assert!(decoded.corrupt, "{raw}");
            assert_eq!(decoded.cookie, UidCookie::default(), "{raw}");
        }
    }

    #[test]
    fn repeated_uid_maps_merge_and_null_resets() {
        for (raw, expected) in [
            (
                r#"{"tempUIDs":{"a":{"uid":"one"}},"tempUIDs":{"b":{"uid":"two"}}}"#,
                vec![("a", "one"), ("b", "two")],
            ),
            (
                r#"{"tempUIDs":{"a":{"uid":"one"}},"tempUIDs":null}"#,
                vec![],
            ),
            (
                r#"{"tempUIDs":{"a":{"uid":"one"}},"tempUIDs":null,"tempUIDs":{"b":{"uid":"two"}}}"#,
                vec![("b", "two")],
            ),
            (
                r#"{"tempUIDs":{"a":{"uid":"one"}},"tempUIDs":{},"tempUIDs":{"a":null}}"#,
                vec![("a", "")],
            ),
        ] {
            let decoded = decode_json(raw);
            assert!(!decoded.corrupt, "{raw}");
            assert_eq!(
                decoded.cookie.uids(),
                expected.into_iter().collect(),
                "{raw}"
            );
        }
    }

    #[test]
    fn duplicate_family_replaces_whole_entry_instead_of_merging_fields() {
        let decoded = decode_json(
            r#"{"tempUIDs":{"a":{"uid":"one","expires":"2026-10-14T12:00:00Z"}},"tempUIDs":{"a":{"uid":"two"}}}"#,
        );
        assert!(!decoded.corrupt);
        assert_eq!(decoded.cookie.uid("a"), Some("two"));
        assert_eq!(
            decoded.cookie.entries()["a"].expires(),
            OffsetDateTime::parse("0001-01-01T00:00:00Z", &Rfc3339).unwrap()
        );
    }

    #[test]
    fn timestamp_encoding_canonicalizes_without_losing_precision_or_offset() {
        for (input, expected) in [
            ("2026-10-14T12:00:00.000000000000Z", "2026-10-14T12:00:00Z"),
            ("2026-10-14T12:00:00+00:00", "2026-10-14T12:00:00Z"),
            (
                "2026-10-14T12:00:00.123400000000Z",
                "2026-10-14T12:00:00.1234Z",
            ),
            (
                "2026-10-14T12:00:00.123456789999-07:30",
                "2026-10-14T12:00:00.123456789-07:30",
            ),
            (
                "2026-10-14T12:00:00.120000000+05:45",
                "2026-10-14T12:00:00.12+05:45",
            ),
        ] {
            let raw = json!({"tempUIDs": {"a": {"uid": "one", "expires": input}}}).to_string();
            let decoded = decode_json(&raw);
            assert!(!decoded.corrupt, "{input}");
            let encoded = encode_cookie(&decoded.cookie, &CookieLimits::default()).unwrap();
            let wire: Value = serde_json::from_slice(&URL_SAFE.decode(encoded).unwrap()).unwrap();
            assert_eq!(wire["tempUIDs"]["a"]["expires"], expected, "{input}");
            assert_eq!(
                decoded.cookie.entries()["a"].expires().offset(),
                OffsetDateTime::parse(expected, &Rfc3339).unwrap().offset(),
                "{input}"
            );
        }
    }

    #[test]
    fn discarded_json_nodes_and_bytes_still_consume_exact_budgets() {
        for (raw, nodes) in [
            (r#"{"optout":true,"optout":null}"#, 5),
            (
                r#"{"tempUIDs":{"a":{"uid":"one","uid":null,"expires":null,"expires":null}},"tempUIDs":null}"#,
                15,
            ),
            (
                r#"{"tempUIDs":{"a":{"expires":"bad"},"a":{}},"unknown":[0,1]}"#,
                13,
            ),
        ] {
            let encoded = URL_SAFE.encode(raw);
            let exact = CookieLimits {
                max_encoded_bytes: encoded.len(),
                max_decoded_bytes: raw.len(),
                max_json_nodes: nodes,
                ..CookieLimits::default()
            };
            decode_cookie(&encoded, &exact).expect("exact input budgets accept syntax");
            for over in [
                CookieLimits {
                    max_json_nodes: nodes - 1,
                    ..exact
                },
                CookieLimits {
                    max_decoded_bytes: raw.len() - 1,
                    ..exact
                },
                CookieLimits {
                    max_encoded_bytes: encoded.len() - 1,
                    ..exact
                },
            ] {
                assert_eq!(
                    decode_cookie(&encoded, &over),
                    Err(CookieLimitError),
                    "{raw}"
                );
            }
        }
    }

    #[test]
    fn timestamp_parse_errors_are_fixed_and_payload_free() {
        let secret = "private-timestamp-value-4829";
        let raw = json!({"tempUIDs": {"a": {"uid": secret, "expires": secret}}}).to_string();
        let error = serde_json::from_str::<RawCookie>(&raw)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("invalid RFC3339 timestamp"), "{error}");
        assert!(!error.contains(secret));
    }

    #[test]
    fn captured_nonempty_uid_rejection_applies_to_reads_mutation_and_decode() {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../../auctioneer-bidder/tests/conformance/fixtures/usersync/cookies.json"
        ))
        .unwrap();
        let cases = corpus["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 48);
        let matches: Vec<_> = cases
            .iter()
            .filter_map(|row| {
                if row["operation"] != "decode_cookie"
                    || row["expected"]["allow_syncs"] != true
                    || row["expected"]["uids"] != json!({})
                {
                    return None;
                }
                let input: Value =
                    serde_json::from_str(row["input"]["decoded_json_utf8"].as_str()?).ok()?;
                let entries = input["tempUIDs"].as_object()?;
                if entries.len() != 1 {
                    return None;
                }
                let (family, entry) = entries.iter().next()?;
                let uid = entry["uid"].as_str()?;
                let expires = OffsetDateTime::parse(entry["expires"].as_str()?, &Rfc3339).ok()?;
                (uid == "0" && expires > now()).then(|| (row, family.clone(), uid.to_owned()))
            })
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "one live nonempty UID excluded by capture policy"
        );
        let (row, family, uid) = &matches[0];
        assert_eq!(row["expected"]["has_live_syncs"], false);
        assert_eq!(row["expected"]["json_utf8"], "{}");
        let limits = CookieLimits::default();
        let mut cookie = UidCookie::default();
        cookie
            .set_uid(family, "accepted-user", now(), &limits)
            .unwrap();
        cookie.entries.get_mut(family).unwrap().uid.clone_from(uid);
        assert_eq!(cookie.uid(family), None);
        assert!(cookie.uids().is_empty());
        assert!(!cookie.is_live(family, now()));
        assert!(!cookie.has_live_syncs(now()));
        cookie.set_uid(family, uid, now(), &limits).unwrap();
        assert!(cookie.entries().is_empty());
        assert!(!cookie.has_live_syncs(now()));
        let decoded = decode_cookie(row["input"]["encoded"].as_str().unwrap(), &limits).unwrap();
        assert!(!decoded.corrupt);
        assert_eq!(decoded.cookie, cookie);
        assert_eq!(
            encode_cookie(&decoded.cookie, &limits).unwrap(),
            row["expected"]["encoded"]
        );
        cookie.set_uid("fake_bidder", uid, now(), &limits).unwrap();
        assert_eq!(cookie.uid("fake_bidder"), Some(uid.as_str()));
        assert!(cookie.is_live("fake_bidder", now()));
    }

    #[test]
    fn mutation_refusal_preserves_existing_entries() {
        let limits = CookieLimits::default();
        let mut cookie = UidCookie::default();
        cookie.set_uid("fam_a", "old", now(), &limits).unwrap();
        let before = cookie.clone();
        let small = CookieLimits {
            max_decoded_bytes: 8,
            ..limits
        };
        assert_eq!(
            cookie.set_uid("fam_a", "too-long-to-fit", now(), &small),
            Err(CookieLimitError)
        );
        assert_eq!(cookie, before);
    }

    #[test]
    fn expiry_overflow_does_not_mutate_cookie() {
        let mut cookie = UidCookie::default();
        let latest = OffsetDateTime::parse("9999-12-31T23:59:59Z", &Rfc3339).unwrap();
        assert_eq!(
            cookie.set_uid("fam_a", "new", latest, &CookieLimits::default()),
            Err(CookieLimitError)
        );
        assert!(cookie.entries().is_empty());
    }

    #[test]
    fn decoded_empty_uid_with_future_expiry_matches_pinned_live_semantics() {
        let json = serde_json::to_vec(&json!({
            "tempUIDs": {"fam_a": {"uid": "", "expires": "2026-10-14T12:00:00Z"}}
        }))
        .unwrap();
        let encoded = URL_SAFE.encode(json);
        let decoded = decode_cookie(&encoded, &CookieLimits::default()).unwrap();
        assert_eq!(decoded.cookie.uid("fam_a"), Some(""));
        assert!(decoded.cookie.is_live("fam_a", now()));
        assert!(decoded.cookie.has_live_syncs(now()));
    }

    #[test]
    fn debug_output_never_contains_uid_values() {
        let secret = "private-uid-value-4829";
        let mut cookie = UidCookie::default();
        cookie
            .set_uid("fam_a", secret, now(), &CookieLimits::default())
            .unwrap();
        assert!(!format!("{cookie:?}").contains(secret));
        assert!(!format!("{:?}", cookie.entries()["fam_a"]).contains(secret));
        assert!(!format!("{:?}", DecodeOutcome::valid(cookie)).contains(secret));
    }

    #[test]
    fn prepared_set_cookie_matches_complete_reference_fields() {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../../auctioneer-bidder/tests/conformance/fixtures/usersync/cookies.json"
        ))
        .unwrap();
        let cases = corpus["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 48);
        let mut checked = 0_usize;
        for row in cases {
            if row["operation"] != "write_cookie" {
                continue;
            }
            checked = checked.checked_add(1).unwrap();
            let input = &row["input"];
            let expected = row["expected"]["set_cookie"].as_str().unwrap();
            let options = CookieWriteOptions {
                name: "uids",
                domain: input["domain"].as_str().unwrap(),
                ttl_days: input["ttl_days"].as_i64().unwrap(),
                user_agent: (input["secure_same_site_none"] == true).then_some("Chrome/67.0"),
                hard_cap_bytes: MAX_SET_COOKIE_FIELD_BYTES,
            };
            let prepared =
                prepare_set_cookie_value(input["encoded"].as_str().unwrap(), &options, now())
                    .unwrap();
            let actual = prepared.field_value().to_str().unwrap();
            if input["encoded"] == "" {
                // The frozen reference used a future-expiring empty value.
                // The signed transport deliberately deletes it instead.
                assert_eq!(row["id"], "set_cookie_clear");
                assert_eq!(
                    expected,
                    "uids=; Path=/; Expires=Tue, 29 Dec 2026 12:00:00 GMT"
                );
                assert_eq!(
                    actual,
                    "uids=; Path=/; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
                );
                continue;
            }
            let mut actual_parts: Vec<_> = actual.split("; ").collect();
            let mut expected_parts: Vec<_> = expected.split("; ").collect();
            actual_parts.sort_unstable();
            expected_parts.sort_unstable();
            assert_eq!(actual_parts, expected_parts, "{}", row["id"]);
            assert_eq!(actual.len(), expected.len(), "{}", row["id"]);
        }
        assert_eq!(checked, 5);
    }

    #[test]
    fn chrome_and_crios_major_67_add_secure_same_site_none() {
        for (agent, expected) in [
            (None, false),
            (Some("Chrome/66.0"), false),
            (Some("Chrome/67.0"), true),
            (Some("CriOS/67.0"), true),
            (Some("Firefox/130.0"), false),
            (Some("Chrome/nope"), false),
            (Some("Chrome/67x"), false),
        ] {
            let options = CookieWriteOptions {
                user_agent: agent,
                ..CookieWriteOptions::default()
            };
            let value = prepare_set_cookie_value("e30=", &options, now()).unwrap();
            let parsed = Cookie::parse(value.field_value().to_str().unwrap()).unwrap();
            assert_eq!(parsed.secure().unwrap_or(false), expected, "{agent:?}");
            assert_eq!(
                parsed.same_site(),
                expected.then_some(cookie::SameSite::None),
                "{agent:?}"
            );
            assert_eq!(parsed.http_only(), None, "{agent:?}");
        }
    }

    #[test]
    fn hard_field_limit_is_independent_and_inclusive() {
        let options = CookieWriteOptions::default();
        let prepared = prepare_set_cookie_value("e30=", &options, now()).unwrap();
        let length = prepared.field_value().as_bytes().len();
        let exact = CookieWriteOptions {
            hard_cap_bytes: length,
            ..options
        };
        prepare_set_cookie_value("e30=", &exact, now()).expect("exact field cap accepts cookie");
        let short = CookieWriteOptions {
            hard_cap_bytes: length - 1,
            ..options
        };
        assert_eq!(
            prepare_set_cookie_value("e30=", &short, now()).unwrap_err(),
            CookieWriteError::HardLimit
        );
        let over_ceiling = "a".repeat(MAX_SET_COOKIE_FIELD_BYTES);
        let large = CookieWriteOptions {
            hard_cap_bytes: usize::MAX,
            ..options
        };
        assert_eq!(
            prepare_set_cookie_value(&over_ceiling, &large, now()).unwrap_err(),
            CookieWriteError::HardLimit
        );
    }

    #[test]
    fn complete_field_boundaries_clamp_requested_caps_to_4096_bytes() {
        for options in [
            CookieWriteOptions::default(),
            CookieWriteOptions {
                domain: "sync.example.com",
                user_agent: Some("Chrome/130.0"),
                ..CookieWriteOptions::default()
            },
        ] {
            let reference = prepare_set_cookie_value("a", &options, now()).unwrap();
            let attribute_bytes = reference.field_value().as_bytes().len() - 1;
            for (requested_cap, effective_cap) in [
                (4_095, 4_095),
                (4_096, 4_096),
                (4_097, 4_096),
                (usize::MAX, 4_096),
            ] {
                let bounded = CookieWriteOptions {
                    hard_cap_bytes: requested_cap,
                    ..options
                };
                for complete_bytes in [effective_cap - 1, effective_cap, effective_cap + 1] {
                    let encoded = "a".repeat(complete_bytes - attribute_bytes);
                    let prepared = prepare_set_cookie_value(&encoded, &bounded, now());
                    if complete_bytes <= effective_cap {
                        assert_eq!(
                            prepared.unwrap().field_value().as_bytes().len(),
                            complete_bytes,
                            "requested cap {requested_cap}"
                        );
                    } else {
                        // The name/domain/value minimum fits: only the complete
                        // formatted field, including attributes, exceeds the cap.
                        assert!(
                            bounded.name.len() + bounded.domain.len() + encoded.len()
                                <= effective_cap
                        );
                        assert_eq!(
                            prepared.unwrap_err(),
                            CookieWriteError::HardLimit,
                            "requested cap {requested_cap}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cap_measures_domain_secure_and_same_site_attributes() {
        let options = CookieWriteOptions {
            domain: "sync.example.com",
            user_agent: Some("CriOS/67.0"),
            ..CookieWriteOptions::default()
        };
        let complete = prepare_set_cookie_value("e30=", &options, now()).unwrap();
        let complete_len = complete.field_value().as_bytes().len();
        let bare = prepare_set_cookie_value("e30=", &CookieWriteOptions::default(), now()).unwrap();
        assert!(complete_len > bare.field_value().as_bytes().len());
        let exact = CookieWriteOptions {
            hard_cap_bytes: complete_len,
            ..options
        };
        prepare_set_cookie_value("e30=", &exact, now()).expect("exact field cap accepts cookie");
        let short = CookieWriteOptions {
            hard_cap_bytes: complete_len - 1,
            ..options
        };
        assert_eq!(
            prepare_set_cookie_value("e30=", &short, now()).unwrap_err(),
            CookieWriteError::HardLimit
        );
    }

    #[test]
    fn invalid_attributes_and_expiry_overflow_never_prepare_a_header() {
        let invalid_domain = CookieWriteOptions {
            domain: "example.com; Secure",
            ..CookieWriteOptions::default()
        };
        assert_eq!(
            prepare_set_cookie_value("e30=", &invalid_domain, now()).unwrap_err(),
            CookieWriteError::InvalidAttributes
        );
        let huge_ttl = CookieWriteOptions {
            ttl_days: i64::MAX,
            ..CookieWriteOptions::default()
        };
        assert_eq!(
            prepare_set_cookie_value("e30=", &huge_ttl, now()).unwrap_err(),
            CookieWriteError::ExpiryOverflow
        );
    }
}

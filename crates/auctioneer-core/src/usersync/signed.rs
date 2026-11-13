//! Integrity for issued UID state. Identifiers inside it remain unverified hints.

use std::{borrow::Cow, collections::BTreeMap, fmt};

use auctioneer_runtime::aux_work::MAX_SYNC_KEYRING_BYTES;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use edgezero_core::http::HeaderMap;
use hmac::{Hmac, KeyInit as _, Mac as _};
use serde::Deserialize;
use sha2::Sha256;
use time::OffsetDateTime;
use zeroize::Zeroizing;

use super::{
    cookie::{DecodeOutcome, HostCookie, UidCookie, read_cookie_with_decoder},
    limits::{CookieLimitError, CookieLimits},
    wire::ObjectOnly,
};

pub const MAX_KEYRING_BYTES: usize = MAX_SYNC_KEYRING_BYTES;
pub const MAX_KEYS: usize = 8;
const MAX_KEY_ID_BYTES: usize = 32;
const MAC_DOMAIN: &[u8] = b"auctioneer.uids.v1\0";
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid UID cookie keyring")]
pub struct InvalidKeyring;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SignedCookieError {
    #[error("invalid UID cookie")]
    InvalidCookie,
    #[error("cookie resource limit exceeded")]
    ResourceLimit,
}

impl From<CookieLimitError> for SignedCookieError {
    fn from(_: CookieLimitError) -> Self {
        Self::ResourceLimit
    }
}

impl SignedCookieError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidCookie => "invalid UID cookie",
            Self::ResourceLimit => "cookie resource limit exceeded",
        }
    }
}

pub struct CookieKeyring {
    active: String,
    keys: BTreeMap<String, Zeroizing<[u8; 32]>>,
}

impl fmt::Debug for CookieKeyring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieKeyring")
            .field("key_count", &self.keys.len())
            .finish_non_exhaustive()
    }
}

impl CookieKeyring {
    /// Parse an atomic, operator-provisioned keyring; never read keys from config.
    ///
    /// # Errors
    /// Rejects invalid, ambiguous or oversized keyring data with a fixed error.
    pub fn parse(bytes: &[u8]) -> Result<Self, InvalidKeyring> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireKey<'a> {
            #[serde(borrow)]
            id: Cow<'a, str>,
            #[serde(borrow)]
            key: Cow<'a, str>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireKeyring<'a> {
            #[serde(borrow)]
            active: Cow<'a, str>,
            #[serde(borrow)]
            keys: Vec<ObjectOnly<WireKey<'a>>>,
        }
        if bytes.len() > MAX_KEYRING_BYTES {
            return Err(InvalidKeyring);
        }
        let wire = serde_json::from_slice::<ObjectOnly<WireKeyring<'_>>>(bytes)
            .map_err(|_error| InvalidKeyring)?
            .into_inner();
        if !valid_key_id(&wire.active) || wire.keys.is_empty() || wire.keys.len() > MAX_KEYS {
            return Err(InvalidKeyring);
        }
        let mut keys = BTreeMap::new();
        for wrapped_entry in wire.keys {
            let entry = wrapped_entry.into_inner();
            if !valid_key_id(&entry.id)
                || entry.key.len() != 43
                || keys.contains_key(entry.id.as_ref())
            {
                return Err(InvalidKeyring);
            }
            let mut key = Zeroizing::new([0_u8; 32]);
            let size = URL_SAFE_NO_PAD
                .decode_slice(entry.key.as_bytes(), key.as_mut())
                .map_err(|_error| InvalidKeyring)?;
            if size != 32 {
                return Err(InvalidKeyring);
            }
            keys.insert(entry.id.into_owned(), key);
        }
        if !keys.contains_key(wire.active.as_ref()) {
            return Err(InvalidKeyring);
        }
        Ok(Self {
            active: wire.active.into_owned(),
            keys,
        })
    }

    /// # Errors
    /// Refuses a cookie payload or complete envelope exceeding the codec limits.
    pub fn encode_cookie(
        &self,
        cookie: &UidCookie,
        limits: &CookieLimits,
    ) -> Result<String, SignedCookieError> {
        let payload = super::cookie::encode_cookie(cookie, limits)?;
        self.sign_payload(&payload, limits)
    }

    /// # Errors
    /// Refuses unsigned/tampered state before decoding its payload, or input limits.
    pub fn decode_cookie(
        &self,
        encoded: &str,
        limits: &CookieLimits,
    ) -> Result<DecodeOutcome, SignedCookieError> {
        if encoded.len() > limits.max_encoded_bytes {
            return Err(SignedCookieError::ResourceLimit);
        }
        let mut fields = encoded.split('.');
        let (Some(version), Some(id), Some(payload), Some(tag)) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(SignedCookieError::InvalidCookie);
        };
        if fields.next().is_some()
            || version != "v1"
            || !valid_key_id(id)
            || payload.is_empty()
            || tag.len() != 43
        {
            return Err(SignedCookieError::InvalidCookie);
        }
        let key = self.keys.get(id).ok_or(SignedCookieError::InvalidCookie)?;
        let mut supplied = [0_u8; 32];
        if URL_SAFE_NO_PAD.decode_slice(tag, &mut supplied).ok() != Some(supplied.len()) {
            return Err(SignedCookieError::InvalidCookie);
        }
        let (prefix, _) = encoded
            .rsplit_once('.')
            .ok_or(SignedCookieError::InvalidCookie)?;
        let mut mac = HmacSha256::new_from_slice(key.as_ref())
            .map_err(|_error| SignedCookieError::InvalidCookie)?;
        mac.update(MAC_DOMAIN);
        mac.update(prefix.as_bytes());
        mac.verify_slice(&supplied)
            .map_err(|_error| SignedCookieError::InvalidCookie)?;
        let outcome = super::cookie::decode_cookie(payload, limits)?;
        if outcome.corrupt {
            return Err(SignedCookieError::InvalidCookie);
        }
        Ok(outcome)
    }

    /// Read header grammar/host opt-out normally, but authenticate UID payloads first.
    ///
    /// # Errors
    /// Rejects unauthenticated state or any cookie resource limit.
    pub fn read_cookie(
        &self,
        headers: &HeaderMap,
        cookie_name: &str,
        host: HostCookie<'_>,
        now: OffsetDateTime,
        limits: &CookieLimits,
    ) -> Result<DecodeOutcome, SignedCookieError> {
        read_cookie_with_decoder(
            headers,
            cookie_name,
            host,
            now,
            limits,
            |encoded, bounds| self.decode_cookie(encoded, bounds),
        )
    }

    fn sign_payload(
        &self,
        payload: &str,
        limits: &CookieLimits,
    ) -> Result<String, SignedCookieError> {
        let size = payload
            .len()
            .checked_add(self.active.len())
            .and_then(|size| size.checked_add(48))
            .filter(|size| *size <= limits.max_encoded_bytes)
            .ok_or(SignedCookieError::ResourceLimit)?;
        let mut envelope = String::with_capacity(size);
        envelope.push_str("v1.");
        envelope.push_str(&self.active);
        envelope.push('.');
        envelope.push_str(payload);
        let key = self
            .keys
            .get(&self.active)
            .ok_or(SignedCookieError::InvalidCookie)?;
        let mut mac = HmacSha256::new_from_slice(key.as_ref())
            .map_err(|_error| SignedCookieError::InvalidCookie)?;
        mac.update(MAC_DOMAIN);
        mac.update(envelope.as_bytes());
        envelope.push('.');
        URL_SAFE_NO_PAD.encode_string(mac.finalize().into_bytes(), &mut envelope);
        Ok(envelope)
    }
}

fn valid_key_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_KEY_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use time::{OffsetDateTime, macros::datetime};

    const NOW: OffsetDateTime = datetime!(2026-10-13 12:00 UTC);

    fn keyring(active: &str, entries: &[(&str, u8)]) -> CookieKeyring {
        let rows = entries
            .iter()
            .map(|(id, byte)| {
                json!({
                    "id": id, "key": URL_SAFE_NO_PAD.encode([*byte; 32])
                })
            })
            .collect::<Vec<_>>();
        CookieKeyring::parse(&serde_json::to_vec(&json!({"active": active, "keys": rows})).unwrap())
            .unwrap()
    }

    fn cookie() -> UidCookie {
        let mut cookie = UidCookie::default();
        cookie
            .set_uid("fam_a", "synthetic-user", NOW, &CookieLimits::default())
            .unwrap();
        cookie
    }

    #[test]
    fn empty_payload_has_an_independent_hmac_known_vector() {
        // OpenSSL HMAC-SHA256, key=32*0x01, input="auctioneer.uids.v1\0v1.test.e30=".
        let encoded = keyring("test", &[("test", 1)])
            .encode_cookie(&UidCookie::default(), &CookieLimits::default())
            .unwrap();
        assert_eq!(
            encoded,
            "v1.test.e30=.SGSYr9zgS45SX6pFUJEQXggl3ELkCg552LjH-AybNjk"
        );
    }

    #[test]
    fn authentic_state_round_trips_without_changing_payload() {
        let keys = keyring("current", &[("old", 1), ("current", 2)]);
        let original = cookie();
        let encoded = keys
            .encode_cookie(&original, &CookieLimits::default())
            .unwrap();
        assert!(encoded.starts_with("v1.current."));
        assert_eq!(
            keys.decode_cookie(&encoded, &CookieLimits::default())
                .unwrap()
                .cookie,
            original
        );
    }

    #[test]
    fn unsigned_state_and_each_authenticated_field_are_rejected() {
        let keys = keyring("test", &[("test", 1), ("other", 1)]);
        let payload =
            super::super::cookie::encode_cookie(&cookie(), &CookieLimits::default()).unwrap();
        assert_eq!(
            keys.decode_cookie(&payload, &CookieLimits::default()),
            Err(SignedCookieError::InvalidCookie)
        );
        let encoded = keys
            .encode_cookie(&cookie(), &CookieLimits::default())
            .unwrap();
        let fields = encoded.split('.').collect::<Vec<_>>();
        for (index, replacement) in [
            (0, "v2"),
            (1, "other"),
            (2, "e30="),
            (3, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
        ] {
            let mut altered = fields.clone();
            altered[index] = replacement;
            assert_eq!(
                keys.decode_cookie(&altered.join("."), &CookieLimits::default()),
                Err(SignedCookieError::InvalidCookie),
                "field {index}"
            );
        }
    }

    #[test]
    fn unsupported_version_is_rejected_even_with_an_authentic_mac() {
        let keys = keyring("test", &[("test", 1)]);
        let limits = CookieLimits::default();
        let payload = super::super::cookie::encode_cookie(&cookie(), &limits).unwrap();
        for version in ["v1", "v2"] {
            let prefix = format!("{version}.test.{payload}");
            let mut mac = Hmac::<Sha256>::new_from_slice(&[1_u8; 32]).unwrap();
            mac.update(b"auctioneer.uids.v1\0");
            mac.update(prefix.as_bytes());
            let envelope = format!(
                "{prefix}.{}",
                URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
            );
            let decoded = keys.decode_cookie(&envelope, &limits);
            if version == "v1" {
                assert_eq!(decoded.unwrap().cookie, cookie());
            } else {
                assert_eq!(decoded, Err(SignedCookieError::InvalidCookie));
            }
        }
    }

    #[test]
    fn keyring_root_requires_an_object_not_a_positional_array() {
        let key = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        let positional = json!(["test", [{"id":"test", "key":key}]]);
        assert_eq!(
            CookieKeyring::parse(&serde_json::to_vec(&positional).unwrap()).unwrap_err(),
            InvalidKeyring
        );
    }

    #[test]
    fn keyring_records_require_objects_not_positional_arrays() {
        let key = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        let positional = json!({"active":"test", "keys":[["test", key]]});
        assert_eq!(
            CookieKeyring::parse(&serde_json::to_vec(&positional).unwrap()).unwrap_err(),
            InvalidKeyring
        );
    }

    #[test]
    fn keyring_object_fields_reject_duplicates_and_unknown_names() {
        let key = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        let entry = json!({"id":"test", "key":key}).to_string();
        // Literal exemption: json! would discard the duplicate fields under test.
        for raw in [
            format!(r#"{{"active":"test","keys":[{entry}],"keys":[{entry}]}}"#),
            format!(r#"{{"active":"test","keys":[{{"id":"test","id":"test","key":"{key}"}}]}}"#),
            format!(r#"{{"active":"test","keys":[{{"id":"test","key":"{key}","key":"{key}"}}]}}"#),
            json!({"active":"test", "keys":[{"id":"test", "key":key,
                "unknown":"private-key-canary"}]})
            .to_string(),
        ] {
            let error = CookieKeyring::parse(raw.as_bytes()).unwrap_err();
            assert_eq!(error, InvalidKeyring);
            assert_eq!(error.to_string(), "invalid UID cookie keyring");
        }
    }

    #[test]
    fn rotation_accepts_retained_keys_and_rejects_retired_keys() {
        // Ids are ordered so the active key is NOT the lexically first entry:
        // signing must select by `active`, not by map order.
        let before = keyring("k1", &[("k1", 1)]);
        let during = keyring("k2", &[("k1", 1), ("k2", 2)]);
        let after = keyring("k2", &[("k2", 2)]);
        let old_cookie = before
            .encode_cookie(&cookie(), &CookieLimits::default())
            .unwrap();
        during
            .decode_cookie(&old_cookie, &CookieLimits::default())
            .unwrap();
        assert_eq!(
            after.decode_cookie(&old_cookie, &CookieLimits::default()),
            Err(SignedCookieError::InvalidCookie)
        );
        let new_cookie = during
            .encode_cookie(&cookie(), &CookieLimits::default())
            .unwrap();
        assert!(new_cookie.starts_with("v1.k2."));
        after
            .decode_cookie(&new_cookie, &CookieLimits::default())
            .unwrap();
    }

    #[test]
    fn invalid_keyrings_are_payload_free_errors() {
        let key = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        for value in [
            json!({"active":"missing", "keys":[{"id":"test", "key":key}]}),
            json!({"active":"test", "keys":[]}),
            json!({"active":"test", "keys":[{"id":"test", "key":"private-key-canary"}]}),
            // Decodable base64 that is too short: reaches the key-length guards
            // instead of failing in the decoder, so a padded weak key is refused.
            json!({"active":"test", "keys":[{"id":"test", "key":"AAAA"}]}),
            json!({"active":"test", "keys":[{"id":"test", "key":key}, {"id":"test", "key":key}]}),
            json!({"active":"bad.id", "keys":[{"id":"bad.id", "key":key}]}),
            json!({"active":"test", "keys":[{"id":"test", "key":key}], "unknown":"private-key-canary"}),
        ] {
            let error = CookieKeyring::parse(&serde_json::to_vec(&value).unwrap()).unwrap_err();
            assert_eq!(error, InvalidKeyring);
            assert!(!format!("{error:?} {error}").contains("private-key-canary"));
        }
        let raw = format!(
            "{{\"active\":\"test\",\"active\":\"test\",\"keys\":[{{\"id\":\"test\",\"key\":\"{key}\"}}]}}"
        );
        assert_eq!(
            CookieKeyring::parse(raw.as_bytes()).unwrap_err(),
            InvalidKeyring
        );
        assert_eq!(
            CookieKeyring::parse(&vec![b' '; MAX_KEYRING_BYTES + 1]).unwrap_err(),
            InvalidKeyring
        );
    }

    #[test]
    fn keyring_debug_does_not_reveal_keys_or_ids() {
        let keys = keyring("private-id-canary", &[("private-id-canary", 1)]);
        let debug = format!("{keys:?}");
        assert!(!debug.contains("private-id-canary"));
        assert!(!debug.contains(&URL_SAFE_NO_PAD.encode([1_u8; 32])));
    }

    #[test]
    fn authentic_mac_does_not_turn_malformed_payload_into_fresh_state() {
        let keys = keyring("test", &[("test", 1)]);
        let malformed = keys
            .sign_payload("bm90LWpzb24=", &CookieLimits::default())
            .unwrap();
        assert_eq!(
            keys.decode_cookie(&malformed, &CookieLimits::default()),
            Err(SignedCookieError::InvalidCookie)
        );
    }

    #[test]
    fn envelope_limits_include_version_key_id_and_mac() {
        let keys = keyring("test", &[("test", 1)]);
        let encoded = keys
            .encode_cookie(&cookie(), &CookieLimits::default())
            .unwrap();
        let exact = CookieLimits {
            max_encoded_bytes: encoded.len(),
            ..CookieLimits::default()
        };
        assert_eq!(keys.encode_cookie(&cookie(), &exact).unwrap(), encoded);
        keys.decode_cookie(&encoded, &exact).unwrap();
        let short = CookieLimits {
            max_encoded_bytes: encoded.len() - 1,
            ..exact
        };
        assert_eq!(
            keys.encode_cookie(&cookie(), &short),
            Err(SignedCookieError::ResourceLimit)
        );
        assert_eq!(
            keys.decode_cookie(&encoded, &short),
            Err(SignedCookieError::ResourceLimit)
        );
    }

    #[test]
    fn every_mac_byte_is_authenticated_independently() {
        let keys = keyring("test", &[("test", 1)]);
        let encoded = keys
            .encode_cookie(&cookie(), &CookieLimits::default())
            .unwrap();
        let (prefix, tag) = encoded.rsplit_once('.').unwrap();
        let original = URL_SAFE_NO_PAD.decode(tag).unwrap();
        assert_eq!(original.len(), 32);
        for index in 0..32 {
            let mut altered = original.clone();
            altered[index] ^= 1;
            let tampered = format!("{prefix}.{}", URL_SAFE_NO_PAD.encode(altered));
            assert_eq!(
                keys.decode_cookie(&tampered, &CookieLimits::default()),
                Err(SignedCookieError::InvalidCookie),
                "MAC byte {index}"
            );
        }
    }

    #[test]
    fn authenticated_payload_resource_failures_never_become_fresh_state() {
        let keys = keyring("test", &[("test", 1)]);
        let encoded = keys
            .encode_cookie(&cookie(), &CookieLimits::default())
            .unwrap();
        let decoded = keys
            .decode_cookie(&encoded, &CookieLimits::default())
            .unwrap();
        assert_eq!(decoded.cookie.uid("fam_a"), Some("synthetic-user"));
        for limits in [
            CookieLimits {
                max_decoded_bytes: 1,
                ..CookieLimits::default()
            },
            CookieLimits {
                max_json_nodes: 1,
                ..CookieLimits::default()
            },
        ] {
            assert!(encoded.len() <= limits.max_encoded_bytes);
            assert_eq!(
                keys.decode_cookie(&encoded, &limits),
                Err(SignedCookieError::ResourceLimit)
            );
        }
    }

    #[test]
    fn keyring_json_escapes_preserve_decoded_key_identity() {
        // Literal exemption: json! canonicalizes away the escape spelling under test.
        let raw = br#"{"active":"te\u0073t","keys":[{"id":"test","key":"\u0041QEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"}]}"#;
        let keys = CookieKeyring::parse(raw).unwrap();
        assert_eq!(
            keys.encode_cookie(&UidCookie::default(), &CookieLimits::default())
                .unwrap(),
            "v1.test.e30=.SGSYr9zgS45SX6pFUJEQXggl3ELkCg552LjH-AybNjk"
        );
    }

    #[test]
    fn valid_keyring_at_byte_ceiling_rejects_one_extra_byte() {
        let mut raw = serde_json::to_vec(&json!({"active":"test", "keys":[{
            "id":"test", "key":URL_SAFE_NO_PAD.encode([1_u8; 32])
        }]}))
        .unwrap();
        raw.resize(MAX_KEYRING_BYTES, b' ');
        CookieKeyring::parse(&raw).unwrap();
        raw.push(b' ');
        assert_eq!(CookieKeyring::parse(&raw).unwrap_err(), InvalidKeyring);
    }

    #[test]
    fn retained_key_count_and_id_length_are_independent_limits() {
        let entries = (0..MAX_KEYS)
            .map(|n| {
                json!({
                    "id": format!("key{n}"), "key": URL_SAFE_NO_PAD.encode([1_u8; 32])
                })
            })
            .collect::<Vec<_>>();
        let mut value = json!({"active":"key0", "keys":entries});
        CookieKeyring::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
        value["keys"].as_array_mut().unwrap().push(json!({
            "id":"extra", "key":URL_SAFE_NO_PAD.encode([1_u8; 32])
        }));
        assert_eq!(
            CookieKeyring::parse(&serde_json::to_vec(&value).unwrap()).unwrap_err(),
            InvalidKeyring
        );
        for id in [
            String::new(),
            "k".repeat(33),
            "private\0id".to_owned(),
            "bad.id".to_owned(),
        ] {
            let invalid =
                json!({"active":id, "keys":[{"id":id,"key":URL_SAFE_NO_PAD.encode([1_u8; 32])}]});
            assert_eq!(
                CookieKeyring::parse(&serde_json::to_vec(&invalid).unwrap()).unwrap_err(),
                InvalidKeyring
            );
        }
        let id = "k".repeat(32);
        CookieKeyring::parse(
            &serde_json::to_vec(&json!({
                "active":id,"keys":[{"id":id,"key":URL_SAFE_NO_PAD.encode([1_u8; 32])}]
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

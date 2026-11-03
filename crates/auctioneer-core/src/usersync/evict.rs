//! Priority-aware, request-local cookie write preparation.

use std::collections::BTreeMap;

use auctioneer_runtime::aux_work::{
    MAX_CATALOG_IDENTITIES, MAX_SYNC_PRIORITY_ENTRIES, MAX_SYNC_PRIORITY_GROUPS,
};
use time::OffsetDateTime;

use super::{
    catalog::Catalog,
    cookie::{
        CookieWriteError, CookieWriteOptions, PreparedCookie, UidCookie, prepare_set_cookie_value,
    },
    limits::CookieLimits,
    signed::CookieKeyring,
};

const MAX_NAME_BYTES: usize = 128;

fn validate_groups(groups: &[Vec<String>]) -> Result<(), CookieWriteError> {
    if u32::try_from(groups.len())
        .ok()
        .is_none_or(|count| count > MAX_SYNC_PRIORITY_GROUPS)
    {
        return Err(CookieWriteError::InvalidPriorities);
    }
    let mut entries = 0_usize;
    for group in groups {
        entries = entries
            .checked_add(group.len())
            .ok_or(CookieWriteError::InvalidPriorities)?;
        if u32::try_from(entries)
            .ok()
            .is_none_or(|count| count > MAX_SYNC_PRIORITY_ENTRIES)
            || group.iter().any(|name| name.len() > MAX_NAME_BYTES)
        {
            return Err(CookieWriteError::InvalidPriorities);
        }
    }
    Ok(())
}

pub struct FamilyPriorities {
    ranks: BTreeMap<String, usize>,
}

impl FamilyPriorities {
    /// Resolve priority names through the catalog's existing folded index.
    /// No complete name/family map is materialized for cookie preparation.
    ///
    /// # Errors
    /// Returns `InvalidPriorities` when group, entry or name bounds are exceeded.
    pub fn from_catalog(
        groups: &[Vec<String>],
        catalog: &Catalog<'_>,
    ) -> Result<Self, CookieWriteError> {
        validate_groups(groups)?;
        if catalog
            .configured_families()
            .any(|(name, family)| name.len() > MAX_NAME_BYTES || family.len() > MAX_NAME_BYTES)
        {
            return Err(CookieWriteError::InvalidPriorities);
        }
        let mut ranks = BTreeMap::new();
        for (rank, group) in groups.iter().enumerate() {
            for bidder in group {
                if let Some(family) = catalog.configured_family(bidder) {
                    ranks.entry(family.to_owned()).or_insert(rank);
                }
            }
        }
        Ok(Self { ranks })
    }

    /// Project a caller-owned standalone family map. Serving paths reuse the
    /// existing index through [`Self::from_catalog`] instead of building this map.
    ///
    /// # Errors
    /// Returns `InvalidPriorities` when group, catalog, or name limits are exceeded.
    pub fn project(
        groups: &[Vec<String>],
        bidder_families: &BTreeMap<String, String>,
    ) -> Result<Self, CookieWriteError> {
        validate_groups(groups)?;
        if u32::try_from(bidder_families.len())
            .ok()
            .is_none_or(|count| count > MAX_CATALOG_IDENTITIES)
        {
            return Err(CookieWriteError::InvalidPriorities);
        }
        if bidder_families
            .iter()
            .any(|(bidder, family)| bidder.len() > MAX_NAME_BYTES || family.len() > MAX_NAME_BYTES)
        {
            return Err(CookieWriteError::InvalidPriorities);
        }
        let folded_families: BTreeMap<_, _> = bidder_families
            .iter()
            .map(|(bidder, family)| (bidder.to_ascii_lowercase(), family))
            .collect();
        let mut ranks = BTreeMap::new();
        for (rank, group) in groups.iter().enumerate() {
            for bidder in group {
                if let Some(&family) = folded_families.get(&bidder.to_ascii_lowercase()) {
                    ranks.entry(family.clone()).or_insert(rank);
                }
            }
        }
        Ok(Self { ranks })
    }

    #[must_use]
    pub fn rank(&self, family: &str) -> Option<usize> {
        self.ranks.get(family).copied()
    }
}

#[derive(Debug)]
pub enum CookieWriteDecision {
    Write(PreparedCookie),
    WarnNoWrite,
    RejectNoWrite,
}

/// # Errors
/// Returns an error for opt-out, codec limits, invalid attributes, or a field
/// exceeding the hard output limit. Logical eviction refusal is a decision.
#[expect(
    clippy::too_many_arguments,
    reason = "transaction keeps independent state, policy, bounds, clock and signing inputs explicit"
)]
pub fn prepare_cookie_transaction(
    cookie: &UidCookie,
    incoming_family: &str,
    priorities: &FamilyPriorities,
    options: &CookieWriteOptions<'_>,
    now: OffsetDateTime,
    codec_limits: &CookieLimits,
    logical_cap_bytes: usize,
    keys: &CookieKeyring,
) -> Result<CookieWriteDecision, CookieWriteError> {
    if !cookie.allow_syncs() {
        return Err(CookieWriteError::OptedOut);
    }
    let mut working = cookie.clone();
    loop {
        let encoded = if working.entries().is_empty() {
            String::new()
        } else {
            keys.encode_cookie(&working, codec_limits)
                .map_err(|_error| CookieWriteError::CodecLimit)?
        };
        let prepared = prepare_set_cookie_value(&encoded, options, now)?;
        if logical_cap_bytes == 0 || prepared.field_value().as_bytes().len() <= logical_cap_bytes {
            return Ok(CookieWriteDecision::Write(prepared));
        }
        if working.entries().len() <= 1 {
            return Ok(CookieWriteDecision::RejectNoWrite);
        }
        let nonpriority_count = working
            .entries()
            .keys()
            .filter(|family| priorities.rank(family).is_none())
            .count();
        let resident_priority = nonpriority_count < working.entries().len();
        if resident_priority && nonpriority_count == 1 && priorities.rank(incoming_family).is_none()
        {
            return Ok(CookieWriteDecision::WarnNoWrite);
        }
        let Some(key) = oldest_evictable_family(&working, priorities) else {
            return Ok(CookieWriteDecision::RejectNoWrite);
        };
        working
            .set_uid(&key, "", now, codec_limits)
            .map_err(|_error| CookieWriteError::CodecLimit)?;
    }
}

fn oldest_evictable_family(cookie: &UidCookie, priorities: &FamilyPriorities) -> Option<String> {
    let nonpriority = cookie
        .entries()
        .iter()
        .filter(|(family, _)| priorities.rank(family).is_none());
    if let Some((family, _)) = nonpriority.min_by(|(left_name, left), (right_name, right)| {
        left.expires()
            .cmp(&right.expires())
            .then_with(|| left_name.cmp(right_name))
    }) {
        return Some(family.clone());
    }
    let lowest_rank = cookie
        .entries()
        .keys()
        .filter_map(|family| priorities.rank(family))
        .max()?;
    cookie
        .entries()
        .iter()
        .filter(|(family, _)| priorities.rank(family) == Some(lowest_rank))
        .min_by(|(left_name, left), (right_name, right)| {
            left.expires()
                .cmp(&right.expires())
                .then_with(|| left_name.cmp(right_name))
        })
        .map(|(family, _)| family.clone())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use auctioneer_config_data::types::{BidderInfo, SyncerConfig, SyncerEndpoint};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use http::{HeaderMap, header::SET_COOKIE};
    use serde_json::json;
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

    use super::*;
    use crate::usersync::{
        cookie::{
            CookieWriteError, CookieWriteOptions, MAX_SET_COOKIE_FIELD_BYTES, PreparedCookie,
            UidCookie, encode_cookie, prepare_set_cookie_value,
        },
        evict::FamilyPriorities as PublicFamilyPriorities,
        limits::CookieLimits,
        signed::CookieKeyring,
    };

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-09-30T12:00:00Z", &Rfc3339).unwrap()
    }

    fn one_uid() -> UidCookie {
        let mut cookie = UidCookie::default();
        cookie
            .set_uid("fam_a", "user-one", now(), &CookieLimits::default())
            .unwrap();
        cookie
    }

    fn priorities() -> FamilyPriorities {
        FamilyPriorities::project(&[], &BTreeMap::new()).unwrap()
    }

    fn priority_catalog(
        name: &'static str,
        family: &str,
        configured: bool,
        disabled: bool,
    ) -> Catalog<'static> {
        Catalog::for_test_bidder(
            name,
            BidderInfo {
                name: "fake_base".to_owned(),
                disabled,
                user_sync: SyncerConfig {
                    key: Some(family.to_owned()),
                    redirect: configured.then(|| SyncerEndpoint {
                        url: "https://sync.example.test".to_owned(),
                        user_macro: None,
                    }),
                    ..SyncerConfig::default()
                },
                ..BidderInfo::default()
            },
        )
    }

    #[test]
    fn indexed_priority_projection_preserves_first_rank_and_disabled_family_membership() {
        let groups = vec![
            vec!["fake_unknown".to_owned()],
            vec!["fake_alias".to_owned(), "fake_AlIaS".to_owned()],
            vec!["fake_ALIAS".to_owned()],
        ];
        let families = BTreeMap::from([("fake_ALIAS".to_owned(), "shared".to_owned())]);
        let reference = FamilyPriorities::project(&groups, &families).unwrap();
        assert_eq!(reference.rank("shared"), Some(1));
        for disabled in [false, true] {
            let catalog = priority_catalog("fake_ALIAS", "shared", true, disabled);
            let projected = FamilyPriorities::from_catalog(&groups, &catalog).unwrap();
            assert_eq!(projected.ranks, reference.ranks);
            assert_eq!(projected.rank("fake_unknown"), None);
        }
    }

    #[test]
    fn indexed_priority_projection_refuses_each_independent_input_bound() {
        let catalog = priority_catalog("fake_ALIAS", "shared", true, false);
        for count in [255_usize, 256, 257] {
            let groups = vec![vec!["fake_alias".to_owned(); count]];
            let result = FamilyPriorities::from_catalog(&groups, &catalog);
            if count <= 256 {
                assert_eq!(result.unwrap().rank("shared"), Some(0));
            } else {
                assert!(matches!(result, Err(CookieWriteError::InvalidPriorities)));
            }
        }
        for groups in [vec![Vec::new(); 33], vec![vec!["x".repeat(129)]]] {
            assert!(matches!(
                FamilyPriorities::from_catalog(&groups, &catalog),
                Err(CookieWriteError::InvalidPriorities)
            ));
        }
    }

    #[test]
    fn indexed_priority_projection_checks_only_configured_name_and_family_lengths() {
        let long_name: &'static str = Box::leak("a".repeat(129).into_boxed_str());
        let long_family = "f".repeat(129);
        for (name, family) in [(long_name, "shared"), ("fake_ALIAS", &long_family)] {
            let configured = priority_catalog(name, family, true, false);
            assert!(matches!(
                FamilyPriorities::from_catalog(&[], &configured),
                Err(CookieWriteError::InvalidPriorities)
            ));
            let unconfigured = priority_catalog(name, family, false, false);
            let ignored = FamilyPriorities::from_catalog(&[], &unconfigured).unwrap();
            assert!(ignored.ranks.is_empty());
        }
    }

    fn keyring(active: &str) -> CookieKeyring {
        CookieKeyring::parse(
            &serde_json::to_vec(&json!({
                "active": active,
                "keys": [{"id": active, "key": URL_SAFE_NO_PAD.encode([1_u8; 32])}],
            }))
            .unwrap(),
        )
        .unwrap()
    }

    fn test_keys() -> CookieKeyring {
        keyring("test")
    }

    fn signed_field_len(cookie: &UidCookie, keys: &CookieKeyring) -> usize {
        let encoded = keys
            .encode_cookie(cookie, &CookieLimits::default())
            .unwrap();
        prepare_set_cookie_value(&encoded, &CookieWriteOptions::default(), now())
            .unwrap()
            .field_value()
            .as_bytes()
            .len()
    }

    #[test]
    fn signed_mac_bytes_count_toward_the_logical_cap_for_a_sole_uid() {
        let cookie = one_uid();
        let original = cookie.clone();
        let keys = test_keys();
        let signed = keys
            .encode_cookie(&cookie, &CookieLimits::default())
            .unwrap();
        let fields: Vec<_> = signed.split('.').collect();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0], "v1");
        assert_eq!(fields[1], "test");
        assert_eq!(
            fields[2],
            encode_cookie(&cookie, &CookieLimits::default()).unwrap()
        );
        assert_eq!(fields[3].len(), 43);
        let cap_without_mac = signed_field_len(&cookie, &keys)
            .checked_sub(fields[3].len())
            .unwrap();
        assert!(unsigned_field_len(&cookie) <= cap_without_mac);
        let decision = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            cap_without_mac,
            &keys,
        )
        .unwrap();
        assert!(matches!(decision, CookieWriteDecision::RejectNoWrite));
        assert_eq!(cookie, original);
    }

    #[test]
    fn signed_active_key_id_length_changes_fit_independently_of_payload_and_mac_length() {
        let cookie = one_uid();
        let original = cookie.clone();
        let short_keys = keyring("s");
        let long_id = "l".repeat(32);
        let long_keys = keyring(&long_id);
        let short = short_keys
            .encode_cookie(&cookie, &CookieLimits::default())
            .unwrap();
        let long = long_keys
            .encode_cookie(&cookie, &CookieLimits::default())
            .unwrap();
        let short_fields: Vec<_> = short.split('.').collect();
        let long_fields: Vec<_> = long.split('.').collect();
        assert_eq!(short_fields[2], long_fields[2]);
        assert_eq!(short_fields[3].len(), 43);
        assert_eq!(long_fields[3].len(), 43);
        let cap = signed_field_len(&cookie, &short_keys);
        assert_eq!(
            signed_field_len(&cookie, &long_keys),
            cap.checked_add(31).unwrap()
        );
        let short_decision = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            cap,
            &short_keys,
        )
        .unwrap();
        assert!(matches!(short_decision, CookieWriteDecision::Write(_)));
        let long_decision = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            cap,
            &long_keys,
        )
        .unwrap();
        assert!(matches!(long_decision, CookieWriteDecision::RejectNoWrite));
        let CookieWriteDecision::Write(prepared) = short_decision else {
            panic!("short key ID must fit");
        };
        assert_eq!(prepared.field_value().as_bytes().len(), cap);
        let parsed = cookie::Cookie::parse(prepared.field_value().to_str().unwrap()).unwrap();
        assert_eq!(
            short_keys
                .decode_cookie(parsed.value(), &CookieLimits::default())
                .unwrap()
                .cookie,
            cookie
        );
        assert_eq!(cookie, original);
    }

    #[test]
    fn signed_codec_limit_refusal_discards_the_whole_transaction() {
        let cookie = cookie_with_entries(&[("a", "one", -1), ("b", "two", 0)]);
        let original = cookie.clone();
        let limits = CookieLimits {
            max_encoded_bytes: encode_cookie(&cookie, &CookieLimits::default())
                .unwrap()
                .len(),
            ..CookieLimits::default()
        };
        assert_eq!(
            prepare_cookie_transaction(
                &cookie,
                "b",
                &priorities(),
                &CookieWriteOptions::default(),
                now(),
                &limits,
                0,
                &test_keys(),
            )
            .unwrap_err(),
            CookieWriteError::CodecLimit
        );
        assert_eq!(cookie, original);
    }

    #[test]
    fn signed_overhead_triggers_oldest_nonpriority_eviction_without_mutating_source() {
        let cookie =
            cookie_with_entries(&[("a", &"a".repeat(100), -1), ("b", &"b".repeat(100), 0)]);
        let original = cookie.clone();
        let cap = unsigned_field_len(&cookie);
        let keys = test_keys();
        assert!(signed_field_len(&cookie, &keys) > cap);
        let mut expected = cookie.clone();
        expected
            .set_uid("a", "", now(), &CookieLimits::default())
            .unwrap();
        assert!(signed_field_len(&expected, &keys) <= cap);
        let written = written_cookie(
            prepare_cookie_transaction(
                &cookie,
                "b",
                &priorities(),
                &CookieWriteOptions::default(),
                now(),
                &CookieLimits::default(),
                cap,
                &keys,
            )
            .unwrap(),
        );
        assert_eq!(written, expected);
        assert_eq!(cookie, original);
    }

    #[test]
    fn signed_complete_field_fits_four_kibibytes_but_one_extra_field_byte_refuses() {
        assert_eq!(MAX_SET_COOKIE_FIELD_BYTES, 4096);
        let keys = test_keys();
        let baseline = cookie_with_entries(&[("fam_a", "x", 0)]);
        let room = MAX_SET_COOKIE_FIELD_BYTES
            .checked_sub(signed_field_len(&baseline, &keys))
            .unwrap();
        let uid_len = room
            .checked_div(4)
            .unwrap()
            .checked_mul(3)
            .unwrap()
            .checked_add(1)
            .unwrap();
        let cookie = cookie_with_entries(&[("fam_a", &"x".repeat(uid_len), 0)]);
        let original = cookie.clone();
        let name = format!("uids{}", "x".repeat(room.rem_euclid(4)));
        let options = CookieWriteOptions {
            name: &name,
            ..CookieWriteOptions::default()
        };
        let signed = keys
            .encode_cookie(&cookie, &CookieLimits::default())
            .unwrap();
        assert!(signed.len() < MAX_SET_COOKIE_FIELD_BYTES);
        let exact = prepare_set_cookie_value(&signed, &options, now()).unwrap();
        assert_eq!(
            exact.field_value().as_bytes().len(),
            MAX_SET_COOKIE_FIELD_BYTES
        );
        let CookieWriteDecision::Write(prepared) = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &options,
            now(),
            &CookieLimits::default(),
            0,
            &keys,
        )
        .unwrap() else {
            panic!("complete signed field at the hard cap must fit");
        };
        assert_eq!(prepared.field_value(), exact.field_value());
        let overflow_name = format!("{name}x");
        let overflow_options = CookieWriteOptions {
            name: &overflow_name,
            hard_cap_bytes: usize::MAX,
            ..options
        };
        assert_eq!(
            prepare_cookie_transaction(
                &cookie,
                "fam_a",
                &priorities(),
                &overflow_options,
                now(),
                &CookieLimits::default(),
                0,
                &keys,
            )
            .unwrap_err(),
            CookieWriteError::HardLimit
        );
        assert_eq!(cookie, original);
    }

    #[test]
    fn eviction_is_exported_at_usersync_root() {
        let projection = PublicFamilyPriorities::project(&[], &BTreeMap::new()).unwrap();
        assert_eq!(projection.rank("fam_a"), None);
    }

    fn cookie_with_entries(entries: &[(&str, &str, i64)]) -> UidCookie {
        let mut cookie = UidCookie::default();
        for (family, uid, age_days) in entries {
            cookie
                .set_uid(
                    family,
                    uid,
                    now()
                        .checked_add(Duration::days(*age_days))
                        .expect("test date stays in range"),
                    &CookieLimits::default(),
                )
                .unwrap();
        }
        cookie
    }

    fn field_len(cookie: &UidCookie) -> usize {
        let encoded = if cookie.entries().is_empty() {
            String::new()
        } else {
            test_keys()
                .encode_cookie(cookie, &CookieLimits::default())
                .unwrap()
        };
        prepare_set_cookie_value(&encoded, &CookieWriteOptions::default(), now())
            .unwrap()
            .field_value()
            .as_bytes()
            .len()
    }

    fn unsigned_field_len(cookie: &UidCookie) -> usize {
        let payload = encode_cookie(cookie, &CookieLimits::default()).unwrap();
        prepare_set_cookie_value(&payload, &CookieWriteOptions::default(), now())
            .unwrap()
            .field_value()
            .as_bytes()
            .len()
    }

    fn written_cookie(decision: CookieWriteDecision) -> UidCookie {
        let CookieWriteDecision::Write(prepared) = decision else {
            panic!("expected write");
        };
        let parsed = cookie::Cookie::parse(prepared.field_value().to_str().unwrap()).unwrap();
        test_keys()
            .decode_cookie(parsed.value(), &CookieLimits::default())
            .unwrap()
            .cookie
    }

    #[test]
    fn logical_zero_means_unlimited_but_the_hard_ceiling_still_applies() {
        let cookie = one_uid();
        let options = CookieWriteOptions::default();
        let decision = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &options,
            now(),
            &CookieLimits::default(),
            0,
            &test_keys(),
        )
        .unwrap();
        let CookieWriteDecision::Write(prepared) = decision else {
            panic!("expected write");
        };
        let length = prepared.field_value().as_bytes().len();
        assert!(length > 0);
        let reduced = CookieWriteOptions {
            hard_cap_bytes: length - 1,
            ..options
        };
        assert_eq!(
            prepare_cookie_transaction(
                &cookie,
                "fam_a",
                &priorities(),
                &reduced,
                now(),
                &CookieLimits::default(),
                0,
                &test_keys(),
            )
            .unwrap_err(),
            CookieWriteError::HardLimit
        );
    }

    #[test]
    fn complete_field_cap_is_inclusive_and_one_byte_overflow_rejects_sole_uid() {
        let cookie = one_uid();
        let options = CookieWriteOptions::default();
        let encoded = test_keys()
            .encode_cookie(&cookie, &CookieLimits::default())
            .unwrap();
        let length = prepare_set_cookie_value(&encoded, &options, now())
            .unwrap()
            .field_value()
            .as_bytes()
            .len();
        assert!(matches!(
            prepare_cookie_transaction(
                &cookie,
                "fam_a",
                &priorities(),
                &options,
                now(),
                &CookieLimits::default(),
                length,
                &test_keys(),
            )
            .unwrap(),
            CookieWriteDecision::Write(_)
        ));
        assert!(matches!(
            prepare_cookie_transaction(
                &cookie,
                "fam_a",
                &priorities(),
                &options,
                now(),
                &CookieLimits::default(),
                length - 1,
                &test_keys(),
            )
            .unwrap(),
            CookieWriteDecision::RejectNoWrite
        ));
        assert_eq!(cookie.entries().len(), 1);
    }

    #[test]
    fn empty_cookie_clearing_still_obeys_attribute_overhead() {
        let cookie = UidCookie::default();
        let options = CookieWriteOptions::default();
        let cleared = prepare_set_cookie_value("", &options, now()).unwrap();
        let length = cleared.field_value().as_bytes().len();
        assert!(matches!(
            prepare_cookie_transaction(
                &cookie,
                "fam_a",
                &priorities(),
                &options,
                now(),
                &CookieLimits::default(),
                length - 1,
                &test_keys(),
            )
            .unwrap(),
            CookieWriteDecision::RejectNoWrite
        ));
        let result = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &options,
            now(),
            &CookieLimits::default(),
            length,
            &test_keys(),
        )
        .unwrap();
        let CookieWriteDecision::Write(prepared) = result else {
            panic!("expected clearing write");
        };
        assert!(
            prepared
                .field_value()
                .to_str()
                .unwrap()
                .starts_with("uids=;")
        );
    }

    #[test]
    fn priority_projection_uses_highest_shared_family_and_discards_unknown_names() {
        let groups = vec![
            vec!["fake_first".to_owned(), "fake_unknown".to_owned()],
            vec!["fake_second".to_owned()],
            vec!["fake_third".to_owned()],
        ];
        let families = BTreeMap::from([
            ("fake_first".to_owned(), "shared".to_owned()),
            ("fake_second".to_owned(), "shared".to_owned()),
            ("fake_third".to_owned(), "other".to_owned()),
        ]);
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        assert_eq!(projection.rank("shared"), Some(0));
        assert_eq!(projection.rank("other"), Some(2));
        assert_eq!(projection.rank("fake_unknown"), None);
        assert_eq!(groups[0][0], "fake_first");
    }

    #[test]
    fn case_insensitive_projection_preserves_family_keys_and_highest_rank() {
        let groups = vec![
            vec!["fake_FAM_ALIAS_".to_owned(), "fake_unknown".to_owned()],
            vec!["fake_AAA".to_owned(), "fake_secondaryname".to_owned()],
        ];
        let families = BTreeMap::from([
            ("fake_aaa".to_owned(), "fam_a".to_owned()),
            ("fake_Fam_Alias_".to_owned(), "fam_a".to_owned()),
            (
                "fake_SecondaryName".to_owned(),
                "familySecondary".to_owned(),
            ),
        ]);
        let original_groups = groups.clone();
        let original_families = families.clone();
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        assert_eq!(projection.rank("fam_a"), Some(0));
        assert_eq!(projection.rank("familySecondary"), Some(1));
        assert_eq!(projection.rank("familysecondary"), None);
        assert_eq!(projection.rank("fake_unknown"), None);
        assert_eq!(groups, original_groups);
        assert_eq!(families, original_families);
    }

    #[test]
    fn case_insensitive_priority_keeps_nonpriority_warning() {
        let cookie = cookie_with_entries(&[
            ("fam_a", &"a".repeat(200), -1),
            ("fake_bbb", &"p".repeat(200), 0),
        ]);
        let original = cookie.clone();
        let groups = vec![vec!["fake_AAA".to_owned()]];
        let families = BTreeMap::from([
            ("fake_aaa".to_owned(), "fam_a".to_owned()),
            ("fake_bbb".to_owned(), "fake_bbb".to_owned()),
        ]);
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        let mut incoming_only = cookie.clone();
        incoming_only
            .set_uid("fam_a", "", now(), &CookieLimits::default())
            .unwrap();
        let cap = 500;
        assert!(field_len(&cookie) > cap);
        assert!(field_len(&incoming_only) <= cap);
        let decision = prepare_cookie_transaction(
            &cookie,
            "fake_bbb",
            &projection,
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            cap,
            &test_keys(),
        )
        .unwrap();
        assert!(matches!(decision, CookieWriteDecision::WarnNoWrite));
        assert_eq!(cookie, original);
    }

    #[test]
    fn empty_account_priority_override_leaves_all_families_nonpriority() {
        let families = BTreeMap::from([("fake_bidder".to_owned(), "shared".to_owned())]);
        let projection = FamilyPriorities::project(&[], &families).unwrap();
        assert_eq!(projection.rank("shared"), None);
    }

    #[test]
    fn family_projection_accepts_511_and_512_catalog_identities_but_refuses_513() {
        for count in [511_usize, 512, 513] {
            let families = (0..count)
                .map(|index| {
                    (
                        format!("fake_bidder_{index:03}"),
                        format!("family-{index:03}"),
                    )
                })
                .collect();
            let groups = vec![vec![
                "fake_bidder_000".to_owned(),
                "fake_bidder_510".to_owned(),
            ]];
            let result = FamilyPriorities::project(&groups, &families);
            if count <= 512 {
                let projection = result.unwrap();
                assert_eq!(projection.rank("family-000"), Some(0));
                assert_eq!(projection.rank("family-510"), Some(0));
            } else {
                assert!(matches!(result, Err(CookieWriteError::InvalidPriorities)));
            }
        }
    }

    #[test]
    fn family_projection_priority_entries_have_an_independent_256_entry_limit() {
        let families = BTreeMap::from([("fake_bidder".to_owned(), "family".to_owned())]);
        for count in [255_usize, 256, 257] {
            let first = count.checked_div(2).unwrap();
            let groups = vec![
                vec!["fake_bidder".to_owned(); first],
                vec!["fake_bidder".to_owned(); count.checked_sub(first).unwrap()],
            ];
            let result = FamilyPriorities::project(&groups, &families);
            if count <= 256 {
                assert_eq!(result.unwrap().rank("family"), Some(0));
            } else {
                assert!(matches!(result, Err(CookieWriteError::InvalidPriorities)));
            }
        }
    }

    #[test]
    fn family_projection_priority_groups_keep_their_32_group_limit() {
        for count in [31, 32, 33] {
            let groups = vec![vec![]; count];
            let result = FamilyPriorities::project(&groups, &BTreeMap::new());
            if count <= 32 {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(CookieWriteError::InvalidPriorities)));
            }
        }
    }

    #[test]
    fn oversized_priority_configuration_is_rejected_before_projection() {
        let too_many_groups = vec![Vec::<String>::new(); 33];
        assert!(matches!(
            FamilyPriorities::project(&too_many_groups, &BTreeMap::new()),
            Err(CookieWriteError::InvalidPriorities)
        ));
        let too_many_entries = vec![vec!["fake_x".to_owned(); 257]];
        assert!(matches!(
            FamilyPriorities::project(&too_many_entries, &BTreeMap::new()),
            Err(CookieWriteError::InvalidPriorities)
        ));
        let long_name = vec![vec!["x".repeat(129)]];
        assert!(matches!(
            FamilyPriorities::project(&long_name, &BTreeMap::new()),
            Err(CookieWriteError::InvalidPriorities)
        ));
        let too_many_catalog = (0_usize..513_usize)
            .map(|number| (format!("fake_{number}"), "family".to_owned()))
            .collect();
        assert!(matches!(
            FamilyPriorities::project(&[], &too_many_catalog),
            Err(CookieWriteError::InvalidPriorities)
        ));
    }

    #[test]
    fn prepared_transaction_is_not_attached_before_response_succeeds() {
        let cookie = one_uid();
        let finish = |prepared: PreparedCookie, later: Result<(), &str>| {
            let mut headers = HeaderMap::new();
            if later.is_ok() {
                headers.append(SET_COOKIE, prepared.into_header_value());
            }
            headers
        };
        for failure in ["permission", "serialization", "header"] {
            let result = prepare_cookie_transaction(
                &cookie,
                "fam_a",
                &priorities(),
                &CookieWriteOptions::default(),
                now(),
                &CookieLimits::default(),
                0,
                &test_keys(),
            )
            .unwrap();
            let CookieWriteDecision::Write(prepared) = result else {
                panic!("expected write");
            };
            assert!(!format!("{prepared:?}").contains("user-one"));
            let headers = finish(prepared, Err(failure));
            assert!(headers.get(SET_COOKIE).is_none(), "{failure}");
        }
        let result = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            0,
            &test_keys(),
        )
        .unwrap();
        let CookieWriteDecision::Write(prepared) = result else {
            panic!("expected write");
        };
        let headers = finish(prepared, Ok(()));
        assert!(headers.get(SET_COOKIE).is_some());
    }

    #[test]
    fn nonpriority_oldest_uses_lexical_family_tiebreak() {
        let cookie =
            cookie_with_entries(&[("a", "old-a", -5), ("b", "old-b", -5), ("p", "priority", 0)]);
        let original = cookie.clone();
        let groups = vec![vec!["fake_bidder_p".to_owned()]];
        let families = BTreeMap::from([("fake_bidder_p".to_owned(), "p".to_owned())]);
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        let mut expected = cookie.clone();
        expected
            .set_uid("a", "", now(), &CookieLimits::default())
            .unwrap();
        let decision = prepare_cookie_transaction(
            &cookie,
            "p",
            &projection,
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            field_len(&expected),
            &test_keys(),
        )
        .unwrap();
        let written = written_cookie(decision);
        assert!(!written.entries().contains_key("a"));
        assert!(written.entries().contains_key("b"));
        assert!(written.entries().contains_key("p"));
        assert_eq!(cookie, original);
        assert_eq!(groups[0], ["fake_bidder_p"]);
    }

    #[test]
    fn lowest_populated_priority_group_evicts_oldest_and_skips_empty_groups() {
        let cookie =
            cookie_with_entries(&[("high", "h", 0), ("low-a", "a", -3), ("low-b", "b", -3)]);
        let groups = vec![
            vec!["fake_h".to_owned()],
            vec!["fake_a".to_owned(), "fake_b".to_owned()],
            vec!["fake_ghost".to_owned()],
        ];
        let families = BTreeMap::from([
            ("fake_h".to_owned(), "high".to_owned()),
            ("fake_a".to_owned(), "low-a".to_owned()),
            ("fake_b".to_owned(), "low-b".to_owned()),
        ]);
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        let mut expected = cookie.clone();
        expected
            .set_uid("low-a", "", now(), &CookieLimits::default())
            .unwrap();
        let written = written_cookie(
            prepare_cookie_transaction(
                &cookie,
                "high",
                &projection,
                &CookieWriteOptions::default(),
                now(),
                &CookieLimits::default(),
                field_len(&expected),
                &test_keys(),
            )
            .unwrap(),
        );
        assert_eq!(
            written
                .entries()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["high", "low-b"]
        );
        assert_eq!(groups[2], ["fake_ghost"]);
    }

    #[test]
    fn single_nonpriority_warning_precedes_eviction_but_sole_oversized_uid_rejects() {
        let cookie = cookie_with_entries(&[("n", "normal", 0), ("p", "priority", 0)]);
        let groups = vec![vec!["fake_priority_bidder".to_owned()]];
        let families = BTreeMap::from([("fake_priority_bidder".to_owned(), "p".to_owned())]);
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        let mut priority_only = cookie.clone();
        priority_only
            .set_uid("n", "", now(), &CookieLimits::default())
            .unwrap();
        assert!(matches!(
            prepare_cookie_transaction(
                &cookie,
                "n",
                &projection,
                &CookieWriteOptions::default(),
                now(),
                &CookieLimits::default(),
                field_len(&priority_only),
                &test_keys(),
            )
            .unwrap(),
            CookieWriteDecision::WarnNoWrite
        ));
        assert_eq!(cookie.entries().len(), 2);

        let only_nonpriority = cookie_with_entries(&[("n", "normal", 0)]);
        assert!(matches!(
            prepare_cookie_transaction(
                &only_nonpriority,
                "n",
                &projection,
                &CookieWriteOptions::default(),
                now(),
                &CookieLimits::default(),
                1,
                &test_keys(),
            )
            .unwrap(),
            CookieWriteDecision::RejectNoWrite
        ));
    }

    #[test]
    fn warning_after_prior_eviction_still_discards_the_whole_transaction() {
        let cookie = cookie_with_entries(&[
            ("n-old", "old", -5),
            ("n-new", "new", 0),
            ("p", "priority", 0),
        ]);
        let groups = vec![vec!["fake_priority_bidder".to_owned()]];
        let families = BTreeMap::from([("fake_priority_bidder".to_owned(), "p".to_owned())]);
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        let mut priority_only = cookie.clone();
        priority_only
            .set_uid("n-old", "", now(), &CookieLimits::default())
            .unwrap();
        priority_only
            .set_uid("n-new", "", now(), &CookieLimits::default())
            .unwrap();
        let decision = prepare_cookie_transaction(
            &cookie,
            "n-new",
            &projection,
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            field_len(&priority_only),
            &test_keys(),
        )
        .unwrap();
        assert!(matches!(decision, CookieWriteDecision::WarnNoWrite));
        assert_eq!(cookie.entries().len(), 3);
    }

    #[test]
    fn repeated_evictions_make_strict_progress_without_mutating_source() {
        let cookie = cookie_with_entries(&[
            ("high", "h", 0),
            ("low", "l", 0),
            ("n1", "one", -2),
            ("n2", "two", -1),
        ]);
        let original = cookie.clone();
        let groups = vec![vec!["fake_h".to_owned()], vec!["fake_l".to_owned()]];
        let families = BTreeMap::from([
            ("fake_h".to_owned(), "high".to_owned()),
            ("fake_l".to_owned(), "low".to_owned()),
        ]);
        let projection = FamilyPriorities::project(&groups, &families).unwrap();
        let only_high = cookie_with_entries(&[("high", "h", 0)]);
        let written = written_cookie(
            prepare_cookie_transaction(
                &cookie,
                "high",
                &projection,
                &CookieWriteOptions::default(),
                now(),
                &CookieLimits::default(),
                field_len(&only_high),
                &test_keys(),
            )
            .unwrap(),
        );
        assert_eq!(
            written
                .entries()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["high"]
        );
        assert_eq!(cookie, original);
    }

    #[test]
    fn reduced_hard_limit_rejects_multi_entry_cookie_without_partial_eviction() {
        let cookie = cookie_with_entries(&[("a", "one", 0), ("b", "two", 0)]);
        let original = cookie.clone();
        let only_a = cookie_with_entries(&[("a", "one", 0)]);
        let options = CookieWriteOptions {
            hard_cap_bytes: field_len(&only_a),
            ..CookieWriteOptions::default()
        };
        assert_eq!(
            prepare_cookie_transaction(
                &cookie,
                "a",
                &priorities(),
                &options,
                now(),
                &CookieLimits::default(),
                0,
                &test_keys(),
            )
            .unwrap_err(),
            CookieWriteError::HardLimit
        );
        assert_eq!(cookie, original);
    }

    #[test]
    fn deletion_prepares_an_empty_value_without_retaining_old_uid() {
        let mut cookie = one_uid();
        cookie
            .set_uid("fam_a", "", now(), &CookieLimits::default())
            .unwrap();
        let decision = prepare_cookie_transaction(
            &cookie,
            "fam_a",
            &priorities(),
            &CookieWriteOptions::default(),
            now(),
            &CookieLimits::default(),
            0,
            &test_keys(),
        )
        .unwrap();
        let CookieWriteDecision::Write(prepared) = decision else {
            panic!("expected clearing write");
        };
        assert!(
            prepared
                .field_value()
                .to_str()
                .unwrap()
                .starts_with("uids=;")
        );
        assert!(
            !prepared
                .field_value()
                .to_str()
                .unwrap()
                .contains("user-one")
        );
        let parsed = cookie::Cookie::parse(prepared.field_value().to_str().unwrap()).unwrap();
        assert!(parsed.value().is_empty());
        assert_eq!(parsed.max_age(), Some(Duration::ZERO));
        assert_eq!(parsed.expires_datetime(), Some(OffsetDateTime::UNIX_EPOCH));
    }
}

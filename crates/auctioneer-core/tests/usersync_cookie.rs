//! Reference rows qualify the unchanged unsigned inner payload and header grammar.
//! Eviction writes use the signed transport envelope, not reference wire goldens.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use auctioneer_core::usersync::{
        cookie::{
            CookieWriteError, CookieWriteOptions, HostCookie, UidCookie, decode_cookie,
            encode_cookie, prepare_set_cookie_value, read_cookie,
        },
        evict::{CookieWriteDecision, FamilyPriorities, prepare_cookie_transaction},
        limits::{CookieLimitError, CookieLimits},
        signed::CookieKeyring,
    };
    use base64::{
        Engine as _,
        engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    };
    use http::{HeaderMap, HeaderValue, header::COOKIE};
    use serde_json::{Value, json};
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

    const ALPHABET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=/+ %\r\n";

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-09-30T12:00:00Z", &Rfc3339).unwrap()
    }

    fn test_keys() -> CookieKeyring {
        CookieKeyring::parse(
            &serde_json::to_vec(&json!({
                "active": "test",
                "keys": [{"id": "test", "key": URL_SAFE_NO_PAD.encode([1_u8; 32])}],
            }))
            .unwrap(),
        )
        .unwrap()
    }

    fn cases() -> Vec<Value> {
        serde_json::from_str::<Value>(include_str!(
            "../../auctioneer-bidder/tests/conformance/fixtures/usersync/cookies.json"
        ))
        .unwrap()["cases"]
            .as_array()
            .unwrap()
            .clone()
    }

    #[test]
    fn reference_optout_variants_block_host_import_and_cookie_transactions() {
        let limits = CookieLimits::default();
        let priorities = FamilyPriorities::project(&[], &BTreeMap::new()).unwrap();
        let keys = test_keys();
        let uppercase_optout = json!({"OPTOUT": true}).to_string();
        // Preserve duplicate keys and ordered null updates that json! would collapse.
        for raw in [
            uppercase_optout.as_str(),
            r#"{"optout":true,"optout":null}"#,
            r#"{"optout":true,"tempUIDs":{"a":{"uid":"one","uid":null,"expires":"2026-10-14T12:00:00Z","expires":null}}}"#,
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                COOKIE,
                HeaderValue::from_str(&format!(
                    "uids={}; host=host-identity",
                    URL_SAFE.encode(raw)
                ))
                .unwrap(),
            );
            let read = read_cookie(
                &headers,
                "uids",
                HostCookie {
                    cookie_name: "host",
                    family: "fam_a",
                    ..HostCookie::default()
                },
                now(),
                &limits,
            )
            .unwrap();
            assert!(!read.corrupt, "{raw}");
            assert!(!read.cookie.allow_syncs(), "{raw}");
            assert!(read.cookie.entries().is_empty(), "{raw}");
            assert_eq!(
                prepare_cookie_transaction(
                    &read.cookie,
                    "fam_a",
                    &priorities,
                    &CookieWriteOptions::default(),
                    now(),
                    &limits,
                    0,
                    &keys,
                )
                .unwrap_err(),
                CookieWriteError::OptedOut,
                "{raw}"
            );
        }
    }

    #[test]
    fn timestamp_padding_cannot_trigger_cookie_cap_eviction() {
        let limits = CookieLimits::default();
        let options = CookieWriteOptions::default();
        let keys = test_keys();
        let priorities = FamilyPriorities::project(&[], &BTreeMap::new()).unwrap();
        let canonical = URL_SAFE.encode(
            json!({"tempUIDs": {
                "a": {"uid": "one", "expires": "2026-10-14T12:00:00Z"},
                "b": {"uid": "two", "expires": "2026-10-15T12:00:00Z"},
            }})
            .to_string(),
        );
        let canonical_cookie = decode_cookie(&canonical, &limits).unwrap().cookie;
        let cap = prepare_set_cookie_value(
            &keys.encode_cookie(&canonical_cookie, &limits).unwrap(),
            &options,
            now(),
        )
        .unwrap()
        .field_value()
        .as_bytes()
        .len();
        let padded = URL_SAFE.encode(json!({"tempUIDs": {
            "a": {"uid": "one", "expires": format!("2026-10-14T12:00:00.{}+00:00", "0".repeat(300))},
            "b": {"uid": "two", "expires": "2026-10-15T12:00:00Z"},
        }}).to_string());
        let decoded = decode_cookie(&padded, &limits).unwrap();
        assert!(!decoded.corrupt);
        let CookieWriteDecision::Write(prepared) = prepare_cookie_transaction(
            &decoded.cookie,
            "b",
            &priorities,
            &options,
            now(),
            &limits,
            cap,
            &keys,
        )
        .unwrap() else {
            panic!("canonical cookie fits the logical cap");
        };
        assert_eq!(prepared.field_value().as_bytes().len(), cap);
        let written = cookie::Cookie::parse(prepared.field_value().to_str().unwrap()).unwrap();
        let retained = keys.decode_cookie(written.value(), &limits).unwrap().cookie;
        assert_eq!(retained.uid("a"), Some("one"));
        assert_eq!(retained.uid("b"), Some("two"));
        assert_eq!(decoded.cookie.entries().len(), 2);
    }

    #[test]
    fn decodes_and_reencodes_reference_cookie_cases() {
        let limits = CookieLimits::default();
        let mut checked = 0_usize;
        for case in cases()
            .into_iter()
            .filter(|case| case["operation"] == "decode_cookie")
        {
            let id = case["id"].as_str().unwrap();
            let encoded = case["input"]["encoded"].as_str().unwrap();
            let decoded = decode_cookie(encoded, &limits).unwrap();
            assert_eq!(
                decoded.cookie.allow_syncs(),
                case["expected"]["allow_syncs"],
                "{id}"
            );
            assert_eq!(
                encode_cookie(&decoded.cookie, &limits).unwrap(),
                case["expected"]["encoded"],
                "{id}"
            );
            assert_eq!(
                decoded.cookie.has_live_syncs(now()),
                case["expected"]["has_live_syncs"],
                "{id}"
            );
            assert_eq!(
                serde_json::to_value(decoded.cookie.uids()).unwrap(),
                case["expected"]["uids"],
                "{id}"
            );
            checked += 1;
        }
        assert_eq!(checked, 36);
    }

    #[test]
    fn reads_first_matching_request_cookie_and_host_optout_from_reference() {
        let limits = CookieLimits::default();
        let mut checked = 0_usize;
        for case in cases()
            .into_iter()
            .filter(|case| case["operation"] == "read_cookie")
        {
            let id = case["id"].as_str().unwrap();
            let mut headers = HeaderMap::new();
            for value in case["input"]["cookie_headers"].as_array().unwrap() {
                headers.append(
                    COOKIE,
                    HeaderValue::from_str(value.as_str().unwrap()).unwrap(),
                );
            }
            let host = &case["input"]["host_cookie"];
            let read = read_cookie(
                &headers,
                "uids",
                HostCookie {
                    cookie_name: host["cookie_name"].as_str().unwrap(),
                    family: host["family"].as_str().unwrap(),
                    optout_name: host["optout_name"].as_str().unwrap(),
                    optout_value: host["optout_value"].as_str().unwrap(),
                },
                now(),
                &limits,
            )
            .unwrap();
            assert_eq!(
                encode_cookie(&read.cookie, &limits).unwrap(),
                case["expected"]["encoded"],
                "{id}"
            );
            assert_eq!(
                serde_json::to_value(read.cookie.uids()).unwrap(),
                case["expected"]["uids"],
                "{id}"
            );
            checked += 1;
        }
        assert_eq!(checked, 7);
    }

    #[test]
    fn invalid_request_cookie_values_do_not_mask_later_optout() {
        for invalid in [
            r"bad\value",
            r#""bad\value""#,
            r#"bad"value"#,
            r#""unclosed"#,
            r#"unopened""#,
            "bad\tvalue",
            " \"yes\"",
        ] {
            for separate_headers in [false, true] {
                let mut headers = HeaderMap::new();
                if separate_headers {
                    headers.append(
                        COOKIE,
                        HeaderValue::from_str(&format!("stop={invalid}")).unwrap(),
                    );
                    headers.append(COOKIE, HeaderValue::from_static("stop=yes"));
                } else {
                    headers.append(
                        COOKIE,
                        HeaderValue::from_str(&format!("stop={invalid}; stop=yes")).unwrap(),
                    );
                }
                let read = read_cookie(
                    &headers,
                    "uids",
                    HostCookie {
                        optout_name: "stop",
                        optout_value: "yes",
                        ..HostCookie::default()
                    },
                    now(),
                    &CookieLimits::default(),
                )
                .unwrap();
                assert!(
                    !read.cookie.allow_syncs(),
                    "{invalid:?}, separate_headers={separate_headers}"
                );
            }
        }
    }

    #[test]
    fn first_valid_request_cookie_preserves_value_and_duplicate_order() {
        for (first, expected) in [
            ("host=first", Some("first")),
            (r#"host="first""#, Some("first")),
            ("host= first", Some(" first")),
            ("host=one, two", Some("one, two")),
            (r#"host=" first ""#, Some(" first ")),
            ("host=one=two", Some("one=two")),
            ("host=%5C", Some("%5C")),
            ("host", None),
            ("host=", None),
            (r#"host="""#, None),
        ] {
            let mut headers = HeaderMap::new();
            headers.append(
                COOKIE,
                HeaderValue::from_str(&format!("{first}; host=later")).unwrap(),
            );
            let read = read_cookie(
                &headers,
                "uids",
                HostCookie {
                    cookie_name: "host",
                    family: "fam_a",
                    ..HostCookie::default()
                },
                now(),
                &CookieLimits::default(),
            )
            .unwrap();
            assert_eq!(read.cookie.uid("fam_a"), expected, "{first:?}");
        }
    }

    #[test]
    fn request_cookie_grammar_applies_before_uid_decode_and_host_import() {
        let limits = CookieLimits::default();
        let mut live = UidCookie::default();
        live.set_uid("fam_a", "live", now(), &limits).unwrap();
        let encoded = encode_cookie(&live, &limits).unwrap();
        for (first, expected_uid, corrupt) in
            [(r"bad\value", Some("live"), false), ("%%%", None, true)]
        {
            let mut headers = HeaderMap::new();
            headers.append(
                COOKIE,
                HeaderValue::from_str(&format!("uids={first}; uids={encoded}")).unwrap(),
            );
            let read =
                read_cookie(&headers, "uids", HostCookie::default(), now(), &limits).unwrap();
            assert_eq!(read.cookie.uid("fam_a"), expected_uid, "{first:?}");
            assert_eq!(read.corrupt, corrupt, "{first:?}");
        }

        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_static(r"host=bad\value; host=valid"),
        );
        let read = read_cookie(
            &headers,
            "uids",
            HostCookie {
                cookie_name: "host",
                family: "fam_a",
                ..HostCookie::default()
            },
            now(),
            &limits,
        )
        .unwrap();
        assert_eq!(read.cookie.uid("fam_a"), Some("valid"));
    }

    #[test]
    fn request_cookie_names_must_be_http_tokens() {
        for name in [
            "bad name",
            "bad(name)",
            "bad/name",
            "bad[name]",
            "bad\\name",
            "bad\"name",
        ] {
            let mut headers = HeaderMap::new();
            headers.append(
                COOKIE,
                HeaderValue::from_str(&format!("{name}=yes")).unwrap(),
            );
            let read = read_cookie(
                &headers,
                "uids",
                HostCookie {
                    optout_name: name,
                    optout_value: "yes",
                    ..HostCookie::default()
                },
                now(),
                &CookieLimits::default(),
            )
            .unwrap();
            assert!(read.cookie.allow_syncs(), "{name:?}");
        }
    }

    #[test]
    fn invalid_request_cookie_bytes_only_discard_their_own_pair() {
        for invalid in [b'\t', 0x80, 0xff] {
            let mut raw = b"stop=bad".to_vec();
            raw.push(invalid);
            raw.extend_from_slice(b"value; stop=yes");
            let mut headers = HeaderMap::new();
            headers.append(COOKIE, HeaderValue::from_bytes(&raw).unwrap());
            let read = read_cookie(
                &headers,
                "uids",
                HostCookie {
                    optout_name: "stop",
                    optout_value: "yes",
                    ..HostCookie::default()
                },
                now(),
                &CookieLimits::default(),
            )
            .unwrap();
            assert!(!read.cookie.allow_syncs(), "{invalid:#x}");
        }
    }

    #[test]
    fn request_cookie_validation_retains_header_and_selected_value_bounds() {
        let host = HostCookie {
            optout_name: "stop",
            optout_value: "yes",
            ..HostCookie::default()
        };
        let limits = CookieLimits {
            max_decoded_bytes: 3,
            ..CookieLimits::default()
        };
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("stop={}; stop=yes", "\\".repeat(10_000))).unwrap(),
        );
        assert!(
            !read_cookie(&headers, "uids", host, now(), &limits)
                .unwrap()
                .cookie
                .allow_syncs()
        );
        let header_bytes = headers[COOKIE].as_bytes().len();
        let exact_limit = CookieLimits {
            max_header_bytes: header_bytes,
            ..limits
        };
        read_cookie(&headers, "uids", host, now(), &exact_limit).unwrap();
        let over_limit = CookieLimits {
            max_header_bytes: header_bytes - 1,
            ..limits
        };
        assert_eq!(
            read_cookie(&headers, "uids", host, now(), &over_limit),
            Err(CookieLimitError)
        );
        headers.append(COOKIE, HeaderValue::from_static("ignored=value"));
        let over_count = CookieLimits {
            max_header_count: 1,
            ..limits
        };
        assert_eq!(
            read_cookie(&headers, "uids", host, now(), &over_count),
            Err(CookieLimitError)
        );
        headers.insert(COOKIE, HeaderValue::from_static("stop=long; stop=yes"));
        assert_eq!(
            read_cookie(&headers, "uids", host, now(), &limits),
            Err(CookieLimitError)
        );
    }

    #[test]
    fn host_fill_keeps_nonempty_expired_uid_but_replaces_empty_uid() {
        let limits = CookieLimits::default();
        let expired = cases()
            .into_iter()
            .find(|case| case["id"] == "expired")
            .unwrap();
        let input: Value =
            serde_json::from_str(expired["input"]["decoded_json_utf8"].as_str().unwrap()).unwrap();
        let entries = input["tempUIDs"].as_object().unwrap();
        assert_eq!(entries.len(), 1);
        let family = entries.keys().next().unwrap();
        assert_eq!(entries[family]["uid"], "old");
        let mut expired_cookie =
            decode_cookie(expired["input"]["encoded"].as_str().unwrap(), &limits)
                .unwrap()
                .cookie;
        expired_cookie
            .fill_host(family, "host", now(), &limits)
            .unwrap();
        assert_eq!(expired_cookie.uid(family), Some("old"));
        assert!(!expired_cookie.is_live(family, now()));

        let empty = cases()
            .into_iter()
            .find(|case| case["id"] == "missing_entry_fields")
            .unwrap();
        let empty_input: Value =
            serde_json::from_str(empty["input"]["decoded_json_utf8"].as_str().unwrap()).unwrap();
        assert_eq!(empty_input["tempUIDs"], json!({(family): {}}));
        let mut empty_cookie = decode_cookie(empty["input"]["encoded"].as_str().unwrap(), &limits)
            .unwrap()
            .cookie;
        empty_cookie
            .fill_host(family, "host", now(), &limits)
            .unwrap();
        assert_eq!(empty_cookie.uid(family), Some("host"));
        assert!(empty_cookie.is_live(family, now()));
        assert_eq!(
            empty_cookie.entries()[family].expires(),
            now() + Duration::days(14)
        );
        empty_cookie.set_uid(family, "", now(), &limits).unwrap();
        assert!(!empty_cookie.entries().contains_key(family));
    }

    #[test]
    fn malformed_cookie_resets_but_budget_exhaustion_is_terminal() {
        let limits = CookieLimits {
            max_json_nodes: 1,
            ..CookieLimits::default()
        };
        // Keep literal trailing garbage: valid serialization would remove the
        // syntax corruption this reset-versus-budget precedence test exercises.
        let malformed = URL_SAFE.encode("{} trailing");
        let read = decode_cookie(&malformed, &limits).unwrap();
        assert!(read.corrupt);
        assert_eq!(read.cookie, UidCookie::default());
        let over_nodes = URL_SAFE.encode(serde_json::to_vec(&json!({"a": 0_i32})).unwrap());
        assert_eq!(decode_cookie(&over_nodes, &limits), Err(CookieLimitError));

        let byte_limits = CookieLimits {
            max_decoded_bytes: 1,
            ..CookieLimits::default()
        };
        assert_eq!(decode_cookie("e30=", &byte_limits), Err(CookieLimitError));
    }

    #[test]
    fn header_limits_apply_even_when_host_optout_short_circuits_decoding() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, HeaderValue::from_static("stop=yes; uids=%%%"));
        let limits = CookieLimits {
            max_header_bytes: 8,
            ..CookieLimits::default()
        };
        assert_eq!(
            read_cookie(
                &headers,
                "uids",
                HostCookie {
                    optout_name: "stop",
                    optout_value: "yes",
                    ..HostCookie::default()
                },
                now(),
                &limits
            ),
            Err(CookieLimitError)
        );
    }

    #[test]
    fn malformed_timestamp_is_corrupt_even_when_cookie_requests_optout() {
        let encoded = URL_SAFE.encode(
            serde_json::to_vec(&json!({
                "optout": true,
                "tempUIDs": {"fam_a": {"uid": "x", "expires": "yesterday"}}
            }))
            .unwrap(),
        );
        let read = decode_cookie(&encoded, &CookieLimits::default()).unwrap();
        assert!(read.corrupt);
        assert!(read.cookie.allow_syncs());
    }

    #[test]
    fn oversized_host_identity_is_refused_before_copy() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, HeaderValue::from_static("host=long-host-identity"));
        let limits = CookieLimits {
            max_decoded_bytes: 8,
            ..CookieLimits::default()
        };
        assert_eq!(
            read_cookie(
                &headers,
                "uids",
                HostCookie {
                    cookie_name: "host",
                    family: "fam_a",
                    ..HostCookie::default()
                },
                now(),
                &limits
            ),
            Err(CookieLimitError)
        );
    }

    #[test]
    fn cookie_optout_skips_host_identity_import() {
        let optout = URL_SAFE.encode(serde_json::to_vec(&json!({"optout": true})).unwrap());
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!("uids={optout}; host={}", "x".repeat(64))).unwrap(),
        );
        let limits = CookieLimits {
            max_decoded_bytes: 32,
            ..CookieLimits::default()
        };
        let read = read_cookie(
            &headers,
            "uids",
            HostCookie {
                cookie_name: "host",
                family: "fam_a",
                ..HostCookie::default()
            },
            now(),
            &limits,
        )
        .unwrap();
        assert!(!read.cookie.allow_syncs());
        assert!(read.cookie.entries().is_empty());
    }

    #[test]
    fn new_uid_mutation_rejects_strings_exceeding_the_budget() {
        let mut cookie = UidCookie::default();
        let limits = CookieLimits {
            max_decoded_bytes: 8,
            ..CookieLimits::default()
        };
        assert_eq!(
            cookie.set_uid("fam_a", "long-host-identity", now(), &limits),
            Err(CookieLimitError)
        );
        assert!(cookie.entries().is_empty());
    }

    #[test]
    fn bare_cookie_name_is_the_first_empty_value() {
        let mut headers = HeaderMap::new();
        let live = cases()
            .into_iter()
            .find(|case| case["id"] == "live")
            .unwrap();
        headers.append(
            COOKIE,
            HeaderValue::from_str(&format!(
                "uids; uids={}",
                live["input"]["encoded"].as_str().unwrap()
            ))
            .unwrap(),
        );
        let read = read_cookie(
            &headers,
            "uids",
            HostCookie::default(),
            now(),
            &CookieLimits::default(),
        )
        .unwrap();
        assert!(read.cookie.entries().is_empty());
    }

    #[test]
    fn discarded_duplicate_json_values_still_consume_node_budget() {
        let limits = CookieLimits {
            max_json_nodes: 20,
            ..CookieLimits::default()
        };
        let last = serde_json::to_vec(&json!({
            "tempUIDs": {"fam_a": {"uid": "last", "expires": "2026-10-14T12:00:00Z"}}
        }))
        .unwrap();
        let last_encoded = URL_SAFE.encode(last);
        assert_eq!(
            decode_cookie(&last_encoded, &limits)
                .unwrap()
                .cookie
                .uid("fam_a"),
            Some("last")
        );

        // Literal exemption: json! collapses duplicate family keys and would
        // discard the first entry's nodes before the decoder could charge them.
        let duplicated = r#"{"tempUIDs":{"fam_a":{"uid":"first","expires":"2026-10-14T12:00:00Z","ignored":[1,2,3,4,5,6,7,8]},"fam_a":{"uid":"last","expires":"2026-10-14T12:00:00Z"}}}"#;
        let duplicated_encoded = URL_SAFE.encode(duplicated);
        assert_eq!(
            decode_cookie(&duplicated_encoded, &limits),
            Err(CookieLimitError)
        );
    }

    #[test]
    fn bounded_arbitrary_cookie_inputs_never_panic() {
        let limits = CookieLimits {
            max_encoded_bytes: 256,
            max_decoded_bytes: 192,
            max_json_nodes: 64,
            ..CookieLimits::default()
        };
        for seed in 0..512_usize {
            let len = seed.rem_euclid(129);
            let input: String = (0..len)
                .map(|index| {
                    char::from(
                        ALPHABET[seed
                            .wrapping_mul(17)
                            .wrapping_add(index.wrapping_mul(31))
                            .rem_euclid(ALPHABET.len())],
                    )
                })
                .collect();
            drop(decode_cookie(&input, &limits));
        }
    }
}

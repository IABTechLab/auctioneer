//! Reference-corpus integrity and library qualification, not endpoint acceptance.
//! Production cookie/chooser/handler tests must also consume these fixtures.

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE};
    use cookie::{Cookie, SameSite};
    use http::{HeaderMap, HeaderValue, Uri, header};
    use serde::Deserialize;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{collections::BTreeSet, fs, io::Cursor, path::Path};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339, macros::datetime};
    use url::form_urlencoded;

    const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Corpus {
        schema_version: u32,
        reference: Reference,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Reference {
        revision: String,
        license: String,
        generator_version: u32,
        go: String,
        now: String,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        id: String,
        operation: String,
        provenance: Provenance,
        input: Value,
        expected: Value,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Provenance {
        kind: String,
        paths: Vec<String>,
        notes: String,
    }

    fn corpus(name: &str) -> Corpus {
        checked_corpus(&fixture(name), name)
    }

    fn checked_corpus(bytes: &[u8], name: &str) -> Corpus {
        let corpus: Corpus = serde_json::from_slice(bytes).unwrap();
        assert_eq!(corpus.schema_version, 1, "{name}");
        assert_eq!(corpus.reference.revision, REVISION, "{name}");
        assert_eq!(corpus.reference.license, "Apache-2.0", "{name}");
        assert_eq!(corpus.reference.generator_version, 1, "{name}");
        assert_eq!(corpus.reference.go, "go1.23.12", "{name}");
        assert_eq!(corpus.reference.now, "2026-09-30T12:00:00Z", "{name}");
        assert!(!corpus.cases.is_empty(), "{name}");
        let mut ids = BTreeSet::new();
        for case in &corpus.cases {
            assert!(
                ids.insert(case.id.as_str()),
                "duplicate {} in {name}",
                case.id
            );
            assert!(!case.id.is_empty());
            assert!(!case.operation.is_empty());
            assert!(!case.provenance.kind.is_empty());
            assert!(!case.provenance.paths.is_empty());
            assert!(!case.provenance.notes.is_empty());
            assert!(case.input.is_object(), "{}", case.id);
            assert!(case.expected.is_object(), "{}", case.id);
        }
        corpus
    }

    fn privacy_corpus() -> Corpus {
        checked_corpus(
            include_bytes!("../../auctioneer-privacy/tests/data/sync/reference.json"),
            "privacy/reference.json",
        )
    }

    fn case<'a>(corpus: &'a Corpus, id: &str) -> &'a Case {
        corpus
            .cases
            .iter()
            .find(|case| case.id == id)
            .unwrap_or_else(|| panic!("missing case {id}"))
    }

    fn successful_setuid<'a>(
        corpus: &'a Corpus,
        format: Option<&str>,
        content_type: &str,
    ) -> &'a Case {
        let matches: Vec<_> = corpus
            .cases
            .iter()
            .filter(|row| {
                let target: Uri = row.input["target"].as_str().unwrap().parse().unwrap();
                let query: Vec<_> =
                    form_urlencoded::parse(target.query().unwrap_or_default().as_bytes()).collect();
                row.input["method"] == "GET"
                    && target.path() == "/setuid"
                    && row.input["body_utf8"] == ""
                    && row.input["cookie_headers"] == json!([])
                    && row.input["user_agent"] == ""
                    && row.input["host_permission_stub"] == ""
                    && query.len() == if format.is_some() { 3 } else { 2 }
                    && query
                        .iter()
                        .any(|(key, value)| key == "bidder" && !value.is_empty())
                    && query
                        .iter()
                        .any(|(key, value)| key == "uid" && value == "123")
                    && query
                        .iter()
                        .find(|(key, _)| key == "f")
                        .map(|(_, value)| value.as_ref())
                        == format
                    && row.expected["headers"]["Content-Type"] == json!([content_type])
            })
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "one {format:?}/{content_type} success capture"
        );
        let row = matches[0];
        assert_eq!(row.expected["status"], 200_i32);
        assert_eq!(row.expected["set_cookie_present"], true);
        assert_eq!(
            row.expected["headers"]["Set-Cookie"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        row
    }

    fn fixture(name: &str) -> Vec<u8> {
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../auctioneer-bidder/tests/conformance/fixtures/usersync")
                .join(name),
        )
        .unwrap_or_else(|error| panic!("read fixture {name}: {error}"))
    }

    fn decode_pixel(bytes: &[u8]) -> Result<(png::OutputInfo, Vec<u8>), png::DecodingError> {
        let mut options = png::DecodeOptions::default();
        options.set_ignore_checksums(false);
        options.set_skip_ancillary_crc_failures(false);
        let decoder = png::Decoder::new_with_options(Cursor::new(bytes), options);
        let mut reader = decoder.read_info()?;
        let output_bytes = reader
            .output_buffer_size()
            .ok_or(png::DecodingError::LimitsExceeded)?;
        let mut rgba = vec![0; output_bytes];
        let info = reader.next_frame(&mut rgba)?;
        reader.finish()?;
        Ok((info, rgba))
    }

    #[test]
    fn original_tracking_pixel_is_a_transparent_rgba_image() {
        let bytes = fixture("pixel.png");
        assert_eq!(bytes.len(), 86);
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            "c2ecff291918a3caf0b7e470323e89f2a1f05b92e12a10649e598cacebe62acf"
        );
        let (info, rgba) = decode_pixel(&bytes).unwrap();
        assert_eq!((info.width, info.height), (1, 1));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        assert_eq!(rgba, [0, 0, 0, 0]);
    }

    #[test]
    fn decoder_checks_every_pixel_chunk_crc() {
        let bytes = fixture("pixel.png");
        // The original image contains IHDR, sBIT, IDAT, and IEND chunks.
        for crc_offset in [29, 45, 70, 82] {
            let mut corrupt = bytes.clone();
            corrupt[crc_offset] ^= 1;
            assert!(
                decode_pixel(&corrupt).is_err(),
                "accepted corrupt CRC at {crc_offset}"
            );
        }
    }

    #[test]
    fn cookie_corpus_covers_defaults_duplicates_optout_and_expiry() {
        let corpus = corpus("cookies.json");
        assert_eq!(corpus.cases.len(), 48);
        for id in [
            "empty_object",
            "json_null",
            "missing_uids",
            "null_uids",
            "null_optout",
            "missing_entry_fields",
            "null_entry",
            "null_uid",
            "null_expiry",
            "duplicate_family",
            "duplicate_optout",
            "duplicate_top_level",
            "unknown_family",
            "escaping",
            "bad_timestamp",
            "invalid_uid_type",
            "invalid_optout_type",
            "invalid_map_type",
            "array_root",
            "trailing_json",
            "broken_json",
            "base64_crlf",
            "padding_nonzero_bits",
            "duplicate_name_first_wins",
            "duplicate_header_first_wins",
            "invalid_first_cookie_wins",
            "quoted_cookie_value",
            "host_optout_wins",
            "host_optout_first_name_wins",
        ] {
            case(&corpus, id);
        }
        let mut checked = [0_usize; 3];
        for row in &corpus.cases {
            match row.operation.as_str() {
                "decode_cookie" => checked[0] += 1,
                "read_cookie" => checked[1] += 1,
                "write_cookie" => {
                    checked[2] += 1;
                    continue;
                }
                operation => panic!("unexpected cookie operation {operation}: {}", row.id),
            }
            let encoded = row.expected["encoded"].as_str().unwrap();
            let json_bytes = URL_SAFE.decode(encoded).unwrap();
            assert_eq!(
                json_bytes,
                row.expected["json_utf8"].as_str().unwrap().as_bytes(),
                "{}",
                row.id
            );
            let decoded: Value = serde_json::from_slice(&json_bytes).unwrap();
            assert!(decoded.is_object());
            if let Some(entries) = decoded["tempUIDs"].as_object() {
                for entry in entries.values() {
                    OffsetDateTime::parse(entry["expires"].as_str().unwrap(), &Rfc3339).unwrap();
                }
            }
            if row.expected["allow_syncs"] == false {
                assert!(row.expected["uids"].as_object().unwrap().is_empty());
                assert_eq!(decoded, json!({"optout": true}));
            }
        }
        assert_eq!(checked, [36, 7, 5]);
        for id in ["expired", "expiry_equality"] {
            assert_eq!(case(&corpus, id).expected["has_live_syncs"], false);
            assert!(
                !case(&corpus, id).expected["uids"]
                    .as_object()
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(
            case(&corpus, "expiry_one_ns_later").expected["has_live_syncs"],
            true
        );
        let duplicate = case(&corpus, "duplicate_family");
        let input: Value =
            serde_json::from_str(duplicate.input["decoded_json_utf8"].as_str().unwrap()).unwrap();
        let families = input["tempUIDs"].as_object().unwrap();
        assert_eq!(families.len(), 1);
        let family = families.keys().next().unwrap();
        assert_eq!(duplicate.expected["uids"], json!({(family): "last"}));
    }

    #[test]
    fn codec_edge_probes_distinguish_acceptance_from_empty_fallback() {
        use base64::engine::general_purpose::STANDARD;
        let corpus = corpus("cookies.json");
        for id in ["base64_crlf", "padding_nonzero_bits"] {
            let row = case(&corpus, id);
            assert_eq!(
                row.expected["allow_syncs"], false,
                "{id} must retain observable state"
            );
            assert_eq!(row.expected["json_utf8"], "{\"optout\":true}");
        }
        for id in [
            "missing_padding",
            "extra_padding",
            "base64_space",
            "invalid_alphabet",
            "trailing_json",
        ] {
            let row = case(&corpus, id);
            assert_eq!(
                row.expected["allow_syncs"], true,
                "{id} must fall back to fresh"
            );
            assert_eq!(row.expected["json_utf8"], "{}");
        }
        for (id, padding) in [("missing_padding", true), ("extra_padding", false)] {
            let encoded = case(&corpus, id).input["encoded"].as_str().unwrap();
            let mut canonical = encoded.trim_end_matches('=').to_owned();
            assert_eq!(!encoded.contains('='), padding);
            while !canonical.len().is_multiple_of(4) {
                canonical.push('=');
            }
            let decoded = URL_SAFE.decode(canonical).unwrap();
            let state: Value = serde_json::from_slice(&decoded).unwrap();
            assert_eq!(
                state["optout"], true,
                "{id} would otherwise be indistinguishable from fallback"
            );
        }
        // These repairs model wrong permissive decoders only. Production
        // decoding must reject whitespace other than Go's CR/LF rule and
        // must not guess the standard base64 alphabet.
        let spaced = case(&corpus, "base64_space").input["encoded"]
            .as_str()
            .unwrap();
        let repaired = URL_SAFE.decode(spaced.replace(' ', "")).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&repaired).unwrap()["optout"],
            true
        );
        let alternate = case(&corpus, "invalid_alphabet").input["encoded"]
            .as_str()
            .unwrap();
        let decoded_alternate = STANDARD.decode(alternate).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&decoded_alternate).unwrap()["optout"],
            true
        );
        let trailing = case(&corpus, "trailing_json").input["decoded_json_utf8"]
            .as_str()
            .unwrap();
        let first: Value = serde_json::Deserializer::from_str(trailing)
            .into_iter()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            first["optout"], true,
            "ignoring trailing JSON must change the result"
        );
    }

    #[test]
    fn formatter_matches_reference_attributes_and_total_field_value_size() {
        let corpus = corpus("cookies.json");
        let mut seen = 0_usize;
        for row in corpus
            .cases
            .iter()
            .filter(|row| row.operation == "write_cookie")
        {
            let go = row.expected["set_cookie"].as_str().unwrap();
            let parsed = Cookie::parse(go).unwrap();
            let mut builder = Cookie::build(("uids", row.input["encoded"].as_str().unwrap()))
                .path("/")
                .expires(datetime!(2026-12-29 12:00 UTC));
            let domain = row.input["domain"].as_str().unwrap();
            if !domain.is_empty() {
                builder = builder.domain(domain);
            }
            if row.input["secure_same_site_none"] == true {
                builder = builder.secure(true).same_site(SameSite::None);
            }
            let rust = builder.build();
            assert_eq!(rust.name(), parsed.name());
            assert_eq!(rust.value(), parsed.value());
            assert_eq!(rust.domain(), parsed.domain());
            assert_eq!(rust.path(), parsed.path());
            assert_eq!(rust.expires_datetime(), parsed.expires_datetime());
            assert_eq!(rust.secure(), parsed.secure());
            assert_eq!(rust.same_site(), parsed.same_site());
            assert_eq!(rust.http_only(), None);
            assert_eq!(rust.to_string().len(), go.len(), "{}", row.id);
            let formatted = rust.to_string();
            let mut rust_attributes: Vec<_> = formatted.split("; ").collect();
            let mut go_attributes: Vec<_> = go.split("; ").collect();
            rust_attributes.sort_unstable();
            go_attributes.sort_unstable();
            assert_eq!(rust_attributes, go_attributes, "{}", row.id);
            seen += 1;
        }
        assert_eq!(seen, 5);
    }

    #[test]
    fn wire_corpus_pins_raw_response_bytes_including_pixel_and_newlines() {
        use base64::engine::general_purpose::STANDARD;
        let corpus = corpus("wire.json");
        assert_eq!(corpus.cases.len(), 42);
        for id in [
            "getuids_empty",
            "getuids_expired_retained",
            "getuids_optout",
            "getuids_invalid",
            "setuid_duplicate_query_first_wins",
            "setuid_clear",
            "setuid_missing_bidder",
            "setuid_unknown_bidder",
            "setuid_optout",
            "setuid_host_denial",
            "setuid_disabled_account",
            "cookie_sync_empty",
            "cookie_sync_selected",
            "cookie_sync_live",
            "cookie_sync_expired",
            "cookie_sync_optout",
            "cookie_sync_malformed",
            "cookie_sync_required_consent",
        ] {
            case(&corpus, id);
        }
        for (format, content_type) in [
            (None, "image/png"),
            (None, "text/html"),
            (Some("i"), "image/png"),
        ] {
            successful_setuid(&corpus, format, content_type);
        }
        let pixel = fixture("pixel.png");
        let mut pixel_cases = 0_usize;
        for row in &corpus.cases {
            assert_eq!(row.operation, "http_response");
            let body = STANDARD
                .decode(row.expected["body_base64"].as_str().unwrap())
                .unwrap();
            if let Some(text) = row.expected["body_utf8"].as_str() {
                assert_eq!(body, text.as_bytes(), "{}", row.id);
            } else {
                assert_eq!(body, pixel, "{}", row.id);
                pixel_cases += 1;
            }
            assert!((200..600).contains(&row.expected["status"].as_u64().unwrap()));
            let headers = row.expected["headers"].as_object().unwrap();
            let has_cookie = headers
                .get("Set-Cookie")
                .is_some_and(|value| !value.as_array().unwrap().is_empty());
            assert_eq!(row.expected["set_cookie_present"], has_cookie, "{}", row.id);
            if row.expected["status"].as_u64().unwrap() >= 400 {
                assert!(!has_cookie, "{} commits a cookie on failure", row.id);
            }
        }
        assert!(pixel_cases >= 2);
    }

    fn unique_input_field(row: &Case, matches: impl Fn(&str, &Value) -> bool) -> &str {
        let fields: Vec<_> = row
            .input
            .as_object()
            .unwrap()
            .iter()
            .filter(|(name, value)| matches(name, value))
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(fields.len(), 1, "one semantic control in {}", row.id);
        fields[0]
    }

    fn chooser_input_fields(corpus: &Corpus) -> BTreeSet<&str> {
        let common = BTreeSet::from([
            "bidders",
            "cookie_json_utf8",
            "cooperative",
            "filter",
            "gpp_sid",
            "known_bidders",
            "limit",
            "privacy",
        ]);
        let allowed = case(corpus, "allowed");
        let disabled = case(corpus, "disabled_before_skip");
        let shared = case(corpus, "shared_family_first_denied");
        assert_eq!(allowed.expected["evaluated"][0]["status"], "ok");
        assert_eq!(disabled.expected["evaluated"][0]["status"], "disabled");
        assert_eq!(
            shared.expected["evaluated"][1]["status"],
            "duplicate_family"
        );
        let enabled = unique_input_field(allowed, |key, value| {
            !common.contains(key) && value == true && disabled.input[key] == false
        });
        let shared_syncer = unique_input_field(allowed, |key, value| {
            !common.contains(key) && value == false && shared.input[key] == true
        });
        let skip_gdpr = unique_input_field(allowed, |key, value| {
            !common.contains(key) && key != enabled && value == true
        });
        let skip_sid = unique_input_field(allowed, |key, value| {
            !common.contains(key) && value == &json!(["2,6", "-1"])
        });
        let mut counts = [0_usize; 2];
        for row in corpus.cases.iter().filter(|row| row.operation == "choose") {
            assert!(row.input[enabled].is_boolean(), "{}", row.id);
            assert!(row.input[shared_syncer].is_boolean(), "{}", row.id);
            counts[0] = counts[0]
                .checked_add(usize::from(row.input[enabled] == false))
                .unwrap();
            counts[1] = counts[1]
                .checked_add(usize::from(row.input[shared_syncer] == true))
                .unwrap();
            assert_eq!(row.input[skip_gdpr], true, "{}", row.id);
            assert_eq!(row.input[skip_sid], json!(["2,6", "-1"]), "{}", row.id);
        }
        assert_eq!(counts, [2, 2]);
        let mut fields = common;
        fields.extend([enabled, shared_syncer, skip_gdpr, skip_sid]);
        assert_eq!(fields.len(), 12);
        fields
    }

    fn assert_template_metadata(row: &Case) -> (bool, &str, bool, bool) {
        let bidder = row.input["bidder"].as_str().unwrap();
        let metadata = &row.input["metadata"];
        let family = metadata["declared_family"].as_str().unwrap();
        let mode = row.expected["type"].as_str().unwrap();
        let token = row.expected["user_macro"].as_str().unwrap();
        assert_eq!(row.input["allowed_types"], serde_json::json!([mode]));
        assert_eq!(
            row.expected["family"],
            if family.is_empty() { bidder } else { family }
        );
        assert_eq!(metadata["user_macro"], token);
        let outer = url::Url::parse(row.expected["url"].as_str().unwrap()).unwrap();
        assert_eq!(outer.scheme(), "https");
        assert!(
            outer.fragment().is_none(),
            "{} loses macro to fragment",
            row.id
        );
        let inner = row.expected["callback_utf8"].as_str().unwrap();
        let template = metadata["endpoint_url"].as_str().unwrap();
        let (prefix, suffix) = template.split_once("{{.RedirectURL}}").unwrap();
        assert!(!suffix.contains("{{.RedirectURL}}"), "one callback slot");
        let shape = url::Url::parse(&format!("{prefix}callback-slot{suffix}")).unwrap();
        let bare_callback = shape.query() == Some("callback-slot");
        if bare_callback {
            let encoded_callback = outer.query().unwrap();
            let decoded_callback =
                form_urlencoded::parse(format!("callback={encoded_callback}").as_bytes())
                    .next()
                    .unwrap()
                    .1
                    .into_owned();
            assert_eq!(decoded_callback, inner);
        } else {
            let parameters: Vec<_> = shape
                .query_pairs()
                .filter(|(_key, value)| value == "callback-slot")
                .collect();
            assert_eq!(parameters.len(), 1, "one named callback slot");
            let name = &parameters[0].0;
            let callbacks: Vec<_> = outer
                .query_pairs()
                .filter(|(key, _value)| key == name)
                .collect();
            assert_eq!(callbacks.len(), 1);
            assert_eq!(callbacks[0].1, inner);
        }
        if token.is_empty() {
            assert_eq!(mode, "iframe");
            assert!(inner.ends_with("&uid="));
        } else {
            let encoded = form_urlencoded::Serializer::new(String::new())
                .append_pair("", token)
                .finish();
            assert!(outer.as_str().contains(encoded.strip_prefix('=').unwrap()));
        }
        (family.is_empty(), mode, token.is_empty(), bare_callback)
    }

    fn assert_callback_round_trip(row: &Case) {
        let bidder = row.input["bidder"].as_str().unwrap();
        let callback = url::Url::parse(
            row.expected["round_trip"]["callback_utf8"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert!(callback.fragment().is_none());
        let query: Vec<_> = callback.query_pairs().collect();
        let uids: Vec<_> = query.iter().filter(|(name, _)| name == "uid").collect();
        assert_eq!(uids.len(), 1);
        assert_eq!(uids[0].1, row.input["simulated_uid"].as_str().unwrap());
        assert_eq!(
            query.iter().find(|(key, _)| key == "bidder").unwrap().1,
            bidder
        );
        assert_eq!(
            query.iter().find(|(key, _)| key == "gdpr").unwrap().1,
            row.input["privacy"]["GDPR"].as_str().unwrap()
        );
        let accounts: Vec<_> = query.iter().filter(|(key, _)| key == "account").collect();
        let account = row.input["account"].as_str().unwrap();
        assert_eq!(accounts.len(), usize::from(!account.is_empty()));
        if let Some(pair) = accounts.first() {
            assert_eq!(pair.1, account);
        }
    }

    #[test]
    fn captured_metadata_preserves_family_callback_identity_and_uid_modes() {
        let corpus = corpus("templates.json");
        assert_eq!(corpus.cases.len(), 12);
        let mut shapes = BTreeSet::new();
        for row in &corpus.cases {
            shapes.insert(assert_template_metadata(row));
            assert_callback_round_trip(row);
        }
        assert_eq!(
            shapes,
            BTreeSet::from([
                (false, "redirect", false, true),
                (true, "iframe", true, false),
                (true, "redirect", false, false),
            ])
        );
    }
    #[test]
    fn chooser_corpus_records_order_and_shared_family_denial() {
        let corpus = corpus("chooser.json");
        assert_eq!(corpus.cases.len(), 25);
        let choose_input_fields = chooser_input_fields(&corpus);
        let mut checked = [0_usize; 2];
        for row in &corpus.cases {
            let (input_fields, expected_fields): (BTreeSet<&str>, &[&str]) =
                match row.operation.as_str() {
                    "bidder_order" => {
                        checked[0] += 1;
                        (
                            BTreeSet::from([
                                "available",
                                "cooperative",
                                "priority_groups",
                                "requested",
                            ]),
                            &["bidders"],
                        )
                    }
                    "choose" => {
                        checked[1] += 1;
                        for field in ["chosen", "evaluated", "privacy_calls"] {
                            assert!(row.expected[field].is_array(), "{}: {field}", row.id);
                        }
                        assert!(row.expected["status"].is_string(), "{}", row.id);
                        assert!(row.expected["status_code"].as_i64().is_some(), "{}", row.id);
                        (
                            choose_input_fields.clone(),
                            &[
                                "chosen",
                                "evaluated",
                                "privacy_calls",
                                "status",
                                "status_code",
                            ],
                        )
                    }
                    operation => panic!("unexpected chooser operation {operation}: {}", row.id),
                };
            for (name, value, fields) in [
                ("input", &row.input, input_fields),
                (
                    "expected",
                    &row.expected,
                    expected_fields.iter().copied().collect(),
                ),
            ] {
                assert_eq!(
                    value
                        .as_object()
                        .unwrap()
                        .keys()
                        .map(String::as_str)
                        .collect::<BTreeSet<_>>(),
                    fields,
                    "{}: {name} fields",
                    row.id,
                );
            }
        }
        assert_eq!(checked, [4, 21]);
        for requested in ["true", "false"] {
            for cooperative in ["true", "false"] {
                case(
                    &corpus,
                    &format!("order_requested_{requested}_cooperative_{cooperative}"),
                );
            }
        }
        let opted_out = case(&corpus, "optout_before_host");
        assert_eq!(opted_out.expected["status"], "user_optout");
        assert_eq!(opted_out.expected["privacy_calls"], json!([]));
        let denied = case(&corpus, "shared_family_first_denied");
        assert_eq!(denied.expected["evaluated"][0]["status"], "privacy_denied");
        assert_eq!(
            denied.expected["evaluated"][1]["status"],
            "duplicate_family"
        );
        assert_eq!(denied.expected["chosen"], json!([]));
        assert_eq!(
            case(&corpus, "failed_candidates_do_not_consume_limit").expected["chosen"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn privacy_reference_keeps_upstream_results_separate_from_auctioneer_policy() {
        let corpus = privacy_corpus();
        assert_eq!(corpus.cases.len(), 71);
        assert_eq!(
            case(&corpus, "cookie_absent_default_0").expected["signal"],
            -1_i64
        );
        assert_eq!(
            case(
                &corpus,
                "cookie_sentinel_overrides_invalid_number_default_1"
            )
            .expected["signal"],
            0_i64
        );
        let max_sid = corpus
            .cases
            .iter()
            .find(|row| row.operation == "parse_sid" && row.input["raw"] == "65535")
            .unwrap();
        assert_eq!(max_sid.expected["valid"], false);
    }

    #[test]
    fn every_reference_case_has_hashed_source_provenance() {
        let manifest: Value = serde_json::from_slice(&fixture("sources.json")).unwrap();
        assert_eq!(manifest["revision"], REVISION);
        let hashes = manifest["source_sha256"].as_object().unwrap();
        for corpus in [
            corpus("cookies.json"),
            corpus("wire.json"),
            corpus("templates.json"),
            corpus("chooser.json"),
            privacy_corpus(),
        ] {
            for row in corpus.cases {
                assert_eq!(row.provenance.kind, "source_call");
                for path in row.provenance.paths {
                    let hash = hashes
                        .get(&path)
                        .unwrap_or_else(|| panic!("{} missing source hash for {path}", row.id))
                        .as_str()
                        .unwrap();
                    assert_eq!(hash.len(), 64, "{path}");
                    assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{path}");
                }
            }
        }
        assert_eq!(
            hex::encode(Sha256::digest(fixture("LICENSE.prebid"))),
            hashes["LICENSE"]
        );
    }

    #[test]
    fn intentional_differences_are_separate_and_link_to_existing_cases() {
        let core = corpus("divergences.json");
        let privacy = checked_corpus(
            include_bytes!("../../auctioneer-privacy/tests/data/sync/divergences.json"),
            "privacy/divergences.json",
        );
        for (group, count) in [(&core, 8), (&privacy, 7)] {
            assert_eq!(group.cases.len(), count);
            for row in &group.cases {
                assert_eq!(row.operation, "auctioneer_divergence");
                assert_eq!(row.provenance.kind, "auctioneer_design");
                for path in &row.provenance.paths {
                    assert!(path.starts_with("docs/superpowers/specs/"));
                    assert!(!path.contains(".."));
                    assert!(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../..")
                            .join(path)
                            .is_file()
                    );
                }
                if let Some(id) = row.input["reference_case"].as_str() {
                    let name = row.input["reference_file"].as_str().unwrap();
                    let reference = if name == "reference.json" {
                        privacy_corpus()
                    } else {
                        assert!(matches!(
                            name,
                            "wire.json" | "cookies.json" | "chooser.json" | "templates.json"
                        ));
                        corpus(name)
                    };
                    case(&reference, id);
                }
            }
        }
        for id in [
            "safe_diagnostics",
            "lexical_eviction_tie",
            "credentialed_cors",
            "empty_explicit_format",
            "disabled_automatic_sync_candidates",
        ] {
            case(&core, id);
        }
        for id in [
            "wide_positive_sid",
            "inactive_section_two",
            "invalid_legacy_json_overridden_by_sid",
        ] {
            case(&privacy, id);
        }
    }

    #[test]
    fn url_safe_codec_uses_required_padding_and_rejects_other_alphabets() {
        assert_eq!(URL_SAFE.encode(b"{}"), "e30=");
        assert_eq!(URL_SAFE.decode("e30=").unwrap(), b"{}");
        assert_eq!(URL_SAFE.encode([251, 255]), "-_8=");
        for invalid in ["e30", "e30==", "e3=0", "+/8=", "e30= ", "e30=\n"] {
            assert!(URL_SAFE.decode(invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn cookie_parser_preserves_header_order_without_percent_decoding() {
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("uids=first; uids=second"),
        );
        headers.append(
            header::COOKIE,
            HeaderValue::from_static("uids=third; host=uid%2Fvalue"),
        );
        let cookies: Vec<_> = headers
            .get_all(header::COOKIE)
            .iter()
            .flat_map(|field| Cookie::split_parse(field.to_str().unwrap()))
            .filter_map(Result::ok)
            .collect();
        assert_eq!(
            cookies.iter().find(|c| c.name() == "uids").unwrap().value(),
            "first"
        );
        assert_eq!(
            cookies.iter().find(|c| c.name() == "host").unwrap().value(),
            "uid%2Fvalue"
        );
        assert_eq!(
            Cookie::parse("uids=\"e30=\"").unwrap().value_trimmed(),
            "e30="
        );
    }

    #[test]
    fn cookie_library_needs_request_grammar_validation() {
        // This library parses Set-Cookie syntax too. Success is not proof that
        // an incoming request pair satisfies Go net/http's cookie grammar.
        for pair in ["bad name=value", "uids=bad\\value", "uids=bad\"value"] {
            Cookie::parse(pair).unwrap();
        }
        Cookie::parse("uids").unwrap_err();
        assert_eq!(Cookie::parse("uids= value").unwrap().value(), "value");
    }

    #[test]
    fn go_base64_padding_bits_require_explicit_decoder_configuration() {
        use base64::{
            alphabet,
            engine::{GeneralPurpose, GeneralPurposeConfig},
        };
        URL_SAFE.decode("e31=").unwrap_err();
        let go_padded = GeneralPurpose::new(
            &alphabet::URL_SAFE,
            GeneralPurposeConfig::new().with_decode_allow_trailing_bits(true),
        );
        assert_eq!(go_padded.decode("e31=").unwrap(), b"{}");
        go_padded.decode("e30").unwrap_err();
        // Go additionally discards CR/LF. Rust deliberately does not; the
        // bounded cookie compatibility layer must handle only those bytes.
        go_padded.decode("e3\r\n0=").unwrap_err();
    }

    #[test]
    fn cookie_formatter_emits_complete_attributes_without_httponly() {
        let cookie = Cookie::build(("uids", "e30="))
            .path("/")
            .domain("sync.example")
            .expires(datetime!(2026-12-29 12:00 UTC))
            .secure(true)
            .same_site(SameSite::None)
            .build();
        assert_eq!(
            cookie.to_string(),
            "uids=e30=; SameSite=None; Secure; Path=/; Domain=sync.example; Expires=Tue, 29 Dec 2026 12:00:00 GMT"
        );
        assert_eq!(cookie.http_only(), None);
        let legacy = Cookie::build(("uids", "e30="))
            .path("/")
            .expires(datetime!(2026-12-29 12:00 UTC))
            .build();
        assert_eq!(
            legacy.to_string(),
            "uids=e30=; Path=/; Expires=Tue, 29 Dec 2026 12:00:00 GMT"
        );
    }

    #[test]
    fn structured_json_retains_utf8_and_escapes_only_json_syntax() {
        let value = json!({"uid": "\"\\\n<&>\u{2028}\u{2029}\u{00e9}"});
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(
            encoded,
            "{\"uid\":\"\\\"\\\\\\n<&>\u{2028}\u{2029}\u{00e9}\"}"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
            value
        );
    }

    #[test]
    fn timestamp_codec_preserves_fraction_and_explicit_offset() {
        for wire in [
            "0001-01-01T00:00:00Z",
            "2026-10-14T12:00:00Z",
            "2026-10-14T12:00:00.123456789Z",
            "2026-10-14T13:00:00+01:00",
        ] {
            let time = OffsetDateTime::parse(wire, &Rfc3339).unwrap();
            assert_eq!(time.format(&Rfc3339).unwrap(), wire);
        }
        for invalid in [
            "",
            "yesterday",
            "2026-02-30T12:00:00Z",
            "2026-10-14",
            "10000-01-01T00:00:00Z",
        ] {
            assert!(
                OffsetDateTime::parse(invalid, &Rfc3339).is_err(),
                "accepted {invalid:?}"
            );
        }
    }
}

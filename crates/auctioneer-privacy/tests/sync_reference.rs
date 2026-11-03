//! Behavioral extraction checks against immutable pinned-source observations.

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use auctioneer_config::types::{AccountConfig, HostConfig};
    use auctioneer_privacy::{
        GdprSignalIn, Signal, SyncEndpoint, SyncPrivacyContext, SyncPrivacyInput, gcl::Gcl,
        gvl::GvlStore, resolve_sync_privacy,
    };
    use serde_json::{Value, json};
    use time::OffsetDateTime;

    fn signal(value: Signal) -> i32 {
        match value {
            Signal::Ambiguous => -1,
            Signal::No => 0,
            Signal::Yes => 1,
        }
    }

    fn sid_projection(context: &SyncPrivacyContext<'_>) -> Value {
        // Exhaust the numeric domain, rather than merely checking expected members.
        let ids: Vec<_> = (0..=u16::MAX)
            .filter(|id| context.sid().contains(*id))
            .collect();
        json!({"valid": true, "ids": ids, "sentinel": context.sid().is_no_applicable_section()})
    }

    fn reference_sid(expected: &Value) -> Value {
        if expected["valid"] == false {
            return json!({"valid": false});
        }
        let values: Vec<_> = expected["values"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|value| value.as_i64().unwrap())
            .collect();
        let ids: BTreeSet<_> = values.iter().copied().filter(|value| *value >= 0).collect();
        json!({"valid": true, "ids": ids, "sentinel": values.contains(&-1)})
    }

    fn reference_projection(row: &Value) -> Value {
        let expected = &row["expected"];
        match row["operation"].as_str().unwrap() {
            "parse_sid" => reference_sid(expected),
            operation => {
                if expected["error_kind"] == "error" {
                    let error = if expected["stage"] == "json" {
                        "MalformedGdpr"
                    } else {
                        match expected["error"].as_str().unwrap() {
                            "GDPR signal should be integer 0 or 1"
                            | "the gdpr query param must be either 0 or 1. You gave 2" => {
                                "MalformedGdpr"
                            }
                            "gdpr_consent is required if gdpr=1"
                            | "gdpr_consent is required. gdpr is not specified and is assumed to be 1 by the server. set gdpr=0 to exempt this request"
                            | "GDPR consent is required when gdpr signal equals 1" => {
                                "MissingRequiredConsent"
                            }
                            "error parsing GPP header, header must have type=3" => "MalformedGpp",
                            "request.us_privacy consent does not match uspv1" => "UspConflict",
                            other => panic!("unaccounted reference error: {other}"),
                        }
                    };
                    return json!({"error": error});
                }
                if operation == "setuid_privacy" {
                    json!({"signal": expected["signal"], "consent": expected["consent"]})
                } else {
                    let macros = &expected["macros"];
                    json!({"signal": expected["signal"], "consent": macros["GDPRConsent"],
                        "usp": macros["USPrivacy"], "gpp": macros["GPP"], "sid": macros["GPPSID"]})
                }
            }
        }
    }

    fn reviewed_divergences<'a>(
        corpus: &Value,
        artifact: &'a Value,
    ) -> BTreeMap<&'a str, &'a Value> {
        assert_eq!(artifact["schema_version"], 1_u8);
        assert_eq!(artifact["reference"], corpus["reference"]);
        let reference_rows = corpus["cases"].as_array().unwrap();
        let references: BTreeMap<_, _> = reference_rows
            .iter()
            .map(|row| (row["id"].as_str().unwrap(), row))
            .collect();
        assert_eq!(
            references.len(),
            reference_rows.len(),
            "duplicate reference ID"
        );
        let approved_designs = BTreeSet::from([
            "docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md",
            "docs/superpowers/specs/2026-09-30-usersync-design.md",
            "docs/superpowers/specs/2026-09-29-gpp-envelope-module-design.md",
        ]);
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut cited_designs = BTreeSet::new();
        let mut divergences = BTreeMap::new();
        for row in artifact["cases"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            assert!(
                divergences.insert(id, row).is_none(),
                "duplicate divergence: {id}"
            );
            let reference = references
                .get(id)
                .expect("divergence must name a captured case");
            assert_eq!(row["input"]["reference_file"], "reference.json", "{id}");
            assert_eq!(row["input"]["reference_case"], id, "{id}");
            assert_eq!(row["operation"], reference["operation"], "{id}");
            let reason = row["reason"].as_str().unwrap();
            assert!(
                !reason.is_empty()
                    && reason
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'_'),
                "missing or unnamed reason: {id}"
            );
            assert_eq!(row["provenance"]["kind"], "auctioneer_design", "{id}");
            assert!(
                !row["provenance"]["notes"]
                    .as_str()
                    .unwrap()
                    .trim()
                    .is_empty(),
                "missing rationale: {id}"
            );
            let paths = row["provenance"]["paths"].as_array().unwrap();
            assert!(!paths.is_empty(), "missing design reference: {id}");
            let mut seen_paths = BTreeSet::new();
            for design_path in paths {
                let path = design_path.as_str().unwrap();
                assert!(seen_paths.insert(path), "duplicate design reference: {id}");
                assert!(
                    approved_designs.contains(path) && root.join(path).is_file(),
                    "not an existing approved design reference: {id}: {path}"
                );
                cited_designs.insert(path);
            }
            assert!(
                !row["expected"].as_object().unwrap().is_empty(),
                "missing expectation: {id}"
            );
            assert_ne!(
                reference_projection(reference),
                row["expected"],
                "divergence no longer differs: {id}"
            );
        }
        assert_eq!(divergences.len(), 22);
        assert_eq!(cited_designs, approved_designs);
        divergences
    }

    fn cookie_gdpr(body: &Value) -> GdprSignalIn<'_> {
        match body.get("gdpr") {
            None => GdprSignalIn::CookieAbsent,
            Some(Value::Null) => GdprSignalIn::CookieNull,
            Some(value) => value
                .as_i64()
                .map_or(GdprSignalIn::CookieOther, GdprSignalIn::CookieNumber),
        }
    }

    fn run_row(row: &Value) -> Value {
        let operation = row["operation"].as_str().unwrap();
        let body: Value = if operation == "cookie_privacy" {
            serde_json::from_str(row["input"]["body_utf8"].as_str().unwrap()).unwrap()
        } else {
            json!({})
        };
        let mut query = BTreeMap::new();
        if operation == "setuid_privacy" {
            let raw = row["input"]["query_utf8"].as_str().unwrap();
            // This pinned corpus contains only literal ASCII query values. Query
            // decoding/limits belong to the real HTTP integration tests, not this carrier.
            assert!(!raw.contains(['%', '+']));
            assert!(raw.is_ascii());
            for pair in raw.split('&').filter(|pair| !pair.is_empty()) {
                let (key, value) = pair.split_once('=').unwrap();
                query.entry(key).or_insert(value);
            }
        }
        let account = AccountConfig::default();
        let mut host = HostConfig::default();
        host.gdpr.default_value = row["input"]["host_default_gdpr"]
            .as_str()
            .unwrap_or("0")
            .into();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        let mut input = SyncPrivacyInput {
            gdpr: GdprSignalIn::CookieNumber(0),
            consent: Some("opaque-sid-test"),
            us_privacy: None,
            gpp: None,
            gpp_sid: None,
            account: &account,
            host: &host,
            endpoint: SyncEndpoint::CookieSync,
            gvl: &gvl,
            gcl: &gcl,
            now: OffsetDateTime::UNIX_EPOCH,
        };
        match operation {
            "cookie_privacy" => {
                input.gdpr = cookie_gdpr(&body);
                input.consent = body["gdpr_consent"].as_str();
                input.us_privacy = body["us_privacy"].as_str();
                input.gpp = body["gpp"].as_str();
                input.gpp_sid = body["gpp_sid"].as_str();
            }
            "setuid_privacy" => {
                input.endpoint = SyncEndpoint::SetUid;
                input.gdpr = GdprSignalIn::SetUidQuery(query.get("gdpr").copied());
                input.consent = query.get("gdpr_consent").copied();
                input.gpp = query.get("gpp").copied();
                input.gpp_sid = query.get("gpp_sid").copied();
            }
            "parse_sid" => input.gpp_sid = row["input"]["raw"].as_str(),
            _ => panic!("unknown reference operation: {operation}"),
        }
        match resolve_sync_privacy(input) {
            Err(error) if operation == "parse_sid" => {
                assert_eq!(error, auctioneer_privacy::SyncPrivacyError::MalformedGppSid);
                json!({"valid": false})
            }
            Err(error) => json!({"error": format!("{error:?}")}),
            Ok(context) => {
                assert_eq!(context.raw_gpp_sid(), input.gpp_sid);
                assert_eq!(
                    context.validated_gpp(),
                    input.gpp.filter(|raw| !raw.is_empty())
                );
                let normalized = match context.parsed_request_signal {
                    Signal::Ambiguous if host.gdpr.default_value == "0" => Signal::No,
                    Signal::Ambiguous => Signal::Yes,
                    definite @ (Signal::Yes | Signal::No) => definite,
                };
                assert_eq!(context.normalized_request_signal, normalized);
                match operation {
                    "parse_sid" => sid_projection(&context),
                    "setuid_privacy" => json!({"signal": signal(context.parsed_request_signal),
                        "consent": context.selected_consent().unwrap_or("")}),
                    _ => json!({"signal": signal(context.parsed_request_signal),
                        "consent": context.selected_consent().unwrap_or(""),
                        "usp": context.selected_usp().unwrap_or(""),
                        "gpp": context.validated_gpp().unwrap_or(""),
                        "sid": context.raw_gpp_sid().unwrap_or("")}),
                }
            }
        }
    }

    fn expected_ids() -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        for name in [
            "absent",
            "null",
            "zero",
            "one_empty",
            "one_consent",
            "invalid_number",
            "invalid_string",
            "invalid_empty_string",
            "invalid_boolean",
            "sentinel_overrides_invalid_number",
            "sentinel_cannot_override_json_string",
            "inactive_tcf_in_envelope",
            "envelope_without_sid",
            "active_tcf",
            "active_missing_tcf_falls_back",
            "bad_envelope",
            "usp_valid",
            "usp_invalid",
            "usp_conflict",
            "usp_selected",
        ] {
            for default in [0_u8, 1] {
                ids.insert(format!("cookie_{name}_default_{default}"));
            }
        }
        for name in [
            "absent",
            "empty",
            "zero",
            "one_empty",
            "invalid",
            "one_consent",
            "sentinel_invalid_legacy",
            "sid_missing_tcf_no_fallback",
            "inactive_tcf_in_envelope",
            "envelope_without_sid",
            "bad_envelope",
            "duplicate_first_wins",
            "mixed_sentinel",
        ] {
            ids.insert(format!("setuid_{name}"));
        }
        for index in 0_u8..18 {
            ids.insert(format!("sid_{index:02}"));
        }
        ids
    }

    #[test]
    #[expect(
        clippy::print_stdout,
        reason = "nocapture exposes exact per-row behavioral coverage in verification logs"
    )]
    fn every_reference_row_drives_real_sync_extraction() {
        let corpus: Value = serde_json::from_str(include_str!("data/sync/reference.json")).unwrap();
        assert_eq!(corpus["schema_version"], 1_u8);
        assert_eq!(
            corpus["reference"]["revision"],
            "d7f8515b86258688304b0d9b6668c6a0e258bc9e"
        );
        assert_eq!(corpus["reference"]["license"], "Apache-2.0");
        let artifact: Value =
            serde_json::from_str(include_str!("data/sync/extraction-divergences.json")).unwrap();
        let divergences = reviewed_divergences(&corpus, &artifact);
        let mut seen = BTreeSet::new();
        let mut seen_divergences = BTreeSet::new();
        let mut counts: BTreeMap<(&str, bool), usize> = BTreeMap::new();
        let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
        for row in corpus["cases"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            assert!(seen.insert(id.to_owned()), "duplicate row: {id}");
            assert_eq!(row["provenance"]["kind"], "source_call", "{id}");
            assert!(
                !row["provenance"]["paths"].as_array().unwrap().is_empty(),
                "{id}"
            );
            let reference = reference_projection(row);
            let actual = run_row(row);
            let (classification, expected) = if let Some(divergence) = divergences.get(id) {
                let reason = divergence["reason"].as_str().unwrap();
                let expected = divergence["expected"].clone();
                assert!(
                    seen_divergences.insert(id),
                    "duplicate exercised divergence: {id}"
                );
                assert_ne!(actual, reference, "actual extraction must differ: {id}");
                *reasons.entry(reason).or_insert(0) += 1;
                (reason, expected)
            } else {
                ("reference_agreement", reference)
            };
            assert_eq!(actual, expected, "{id}: {classification}");
            *counts
                .entry((
                    row["operation"].as_str().unwrap(),
                    classification == "reference_agreement",
                ))
                .or_insert(0) += 1;
            println!("covered {id}: {classification}");
        }
        assert_eq!(seen_divergences, divergences.keys().copied().collect());
        assert_eq!(seen, expected_ids());
        assert_eq!(
            counts,
            BTreeMap::from([
                (("cookie_privacy", true), 30),
                (("cookie_privacy", false), 10),
                (("setuid_privacy", true), 9),
                (("setuid_privacy", false), 4),
                (("parse_sid", true), 10),
                (("parse_sid", false), 8),
            ])
        );
        assert_eq!(
            reasons,
            BTreeMap::from([
                ("invalid_legacy_json_overridden_by_sid", 2),
                ("inactive_section_two", 6),
                ("invalid_usp_cleared", 2),
                ("active_gpp_usp_wins_conflict", 2),
                ("setuid_missing_section_fallback", 1),
                ("mixed_sentinel_rejected", 2),
                ("nonpositive_sid_rejected", 5),
                ("wide_positive_sid", 2),
            ])
        );
        println!("coverage: 71/71 rows; 49 reference agreements; 22 deliberate divergences");
    }
}

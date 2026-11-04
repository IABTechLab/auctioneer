//! Per-route handlers for auctions, auxiliary probes, and user sync.

pub mod auction;
pub mod aux_egress;
pub(crate) mod bidder_credentials;
pub mod cookie_sync;
pub(crate) mod debug_signals;
pub mod egress_deadline;
pub mod getuids;
pub mod health_gvl;
pub mod info_bidders;
pub mod probe_config;
pub mod request_disclosure_audit;
pub mod setuid;
pub mod status;
pub mod sync_http;
pub mod usersync_keys;

pub use auction::{TestFanoutHandle, auction};
pub use aux_egress::with_aux_egress;
pub use cookie_sync::{cookie_sync, cookie_sync_preflight};
pub use egress_deadline::{EgressDeadline, EgressRefusal, OVERLOADED_BODY};
pub use getuids::getuids;
pub use health_gvl::health_gvl;
pub use info_bidders::{info_bidder, info_bidders};
pub use setuid::setuid;
pub use status::status;

#[cfg(test)]
mod tests {
    use crate::test_support::config_store::configured_sync_host;
    pub(super) mod bidder_registry {
        use std::{collections::BTreeMap, sync::OnceLock};

        use auctioneer_bidder::BidderRegistry;
        use auctioneer_config_data::{
            types::BidderInfo,
            usersync::{SyncType, SyncerEndpoint},
        };
        use serde::Deserialize;

        use auctioneer_bidder::test_support::fake_bidder::FakeBidder;

        #[derive(Deserialize)]
        struct Templates {
            cases: Vec<TemplateCase>,
        }

        #[derive(Deserialize)]
        struct TemplateCase {
            input: TemplateInput,
        }

        #[derive(Deserialize)]
        struct TemplateInput {
            bidder: String,
            allowed_types: Vec<SyncType>,
            metadata: Metadata,
        }

        #[derive(Deserialize)]
        struct Metadata {
            declared_family: String,
            endpoint_url: String,
            user_macro: String,
        }

        pub(crate) fn registry_handle() -> &'static BidderRegistry {
            static REGISTRY: OnceLock<BidderRegistry> = OnceLock::new();
            REGISTRY.get_or_init(|| {
                let templates: Templates = serde_json::from_str(include_str!(
                    "../../../auctioneer-bidder/tests/conformance/fixtures/usersync/templates.json"
                ))
                .expect("frozen template inputs");
                assert_eq!(templates.cases.len(), 12);
                let mut infos = BTreeMap::<String, BidderInfo>::new();
                for case in templates.cases {
                    let input = case.input;
                    let info = infos
                        .entry(input.bidder.clone())
                        .or_insert_with(|| BidderInfo {
                            name: input.bidder,
                            ..BidderInfo::default()
                        });
                    let family = (!input.metadata.declared_family.is_empty())
                        .then_some(input.metadata.declared_family);
                    if info.user_sync.iframe.is_some() || info.user_sync.redirect.is_some() {
                        assert_eq!(
                            info.user_sync.key, family,
                            "capture family must be consistent"
                        );
                    } else {
                        info.user_sync.key = family;
                    }
                    assert_eq!(
                        input.allowed_types.len(),
                        1,
                        "each capture pins one endpoint type"
                    );
                    let endpoint = SyncerEndpoint {
                        url: input.metadata.endpoint_url,
                        user_macro: Some(input.metadata.user_macro),
                    };
                    let slot = match input.allowed_types[0] {
                        SyncType::Iframe => &mut info.user_sync.iframe,
                        SyncType::Redirect => &mut info.user_sync.redirect,
                    };
                    if let Some(previous) = slot {
                        assert_eq!(
                            previous.url, endpoint.url,
                            "capture endpoint must be consistent"
                        );
                        assert_eq!(previous.user_macro, endpoint.user_macro);
                    } else {
                        *slot = Some(endpoint);
                    }
                }
                let mut registry = BidderRegistry::new();
                for info in infos.into_values() {
                    registry
                        .register(Box::leak(Box::new(info)), Box::<FakeBidder>::default())
                        .expect("unique test bidder");
                }
                registry
            })
        }
    }

    use std::collections::BTreeMap;
    use std::future::ready;
    use std::panic::catch_unwind;

    use crate::test_support::usersync_keys;
    use crate::usersync::wire::parse_cookie_sync_body;
    use crate::usersync::{cookie as payload_cookie, limits::CookieLimits};
    use crate::{config::AuctioneerAppConfig, handlers::sync_http};
    use auctioneer_config_data::usersync::UserSyncConfig;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use cookie::Cookie;
    use edgezero_core::{body::Body, http::Response};
    use futures::executor::block_on;
    use http::{HeaderMap, HeaderValue, StatusCode, Uri};
    use serde::Deserialize;
    use serde_json::Value;
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};
    use url::form_urlencoded;

    // Same trusted configuration as the archived reference capture harness,
    // not an expectation extracted from the response under test.
    const CALLBACK: &str = "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&gdpr={{.GDPR}}&gdpr_consent={{.GDPRConsent}}&gpp={{.GPP}}&gpp_sid={{.GPPSID}}&f={{.SyncType}}&uid={{.UserMacro}}";

    #[derive(Deserialize)]
    struct Corpus {
        reference: Reference,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Reference {
        now: String,
    }

    #[derive(Deserialize)]
    struct Case {
        id: String,
        input: Input,
        expected: Expected,
    }

    #[derive(Deserialize)]
    struct Input {
        method: String,
        target: String,
        body_utf8: String,
        cookie_headers: Vec<String>,
        user_agent: String,
        host_cookie: HostCookie,
        host_default_gdpr: String,
        host_permission_stub: String,
    }

    #[derive(Deserialize)]
    struct HostCookie {
        ttl_days: i64,
    }

    #[derive(Deserialize)]
    struct Expected {
        status: u16,
        headers: BTreeMap<String, Vec<String>>,
        body_base64: String,
        set_cookie_present: bool,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct WireOutput {
        status: u16,
        headers: BTreeMap<String, Vec<String>>,
        body: Vec<u8>,
        set_cookie_present: bool,
    }

    fn corpus() -> Corpus {
        serde_json::from_str(include_str!(
            "../../../auctioneer-bidder/tests/conformance/fixtures/usersync/wire.json"
        ))
        .unwrap()
    }

    fn case<'a>(corpus: &'a Corpus, id: &str) -> &'a Case {
        corpus.cases.iter().find(|case| case.id == id).expect(id)
    }

    fn successful_setuid<'a>(
        corpus: &'a Corpus,
        format: Option<&str>,
        content_type: &str,
    ) -> &'a Case {
        let matches: Vec<_> = corpus
            .cases
            .iter()
            .filter(|case| {
                let input = &case.input;
                let target: Uri = input.target.parse().unwrap();
                let query: Vec<_> =
                    form_urlencoded::parse(target.query().unwrap_or_default().as_bytes()).collect();
                input.method == "GET"
                    && target.path() == "/setuid"
                    && input.body_utf8.is_empty()
                    && input.cookie_headers.is_empty()
                    && input.user_agent.is_empty()
                    && input.host_permission_stub.is_empty()
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
                    && case
                        .expected
                        .headers
                        .get("Content-Type")
                        .is_some_and(|values| values.iter().map(String::as_str).eq([content_type]))
            })
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "one {format:?}/{content_type} success capture"
        );
        let row = matches[0];
        assert_eq!(row.expected.status, 200);
        assert!(row.expected.set_cookie_present);
        assert_eq!(row.expected.headers["Set-Cookie"].len(), 1);
        let body = STANDARD.decode(&row.expected.body_base64).unwrap();
        match content_type {
            "image/png" => assert_eq!(
                body,
                include_bytes!(
                    "../../../auctioneer-bidder/tests/conformance/fixtures/usersync/pixel.png"
                )
            ),
            "text/html" => assert!(body.is_empty()),
            _ => panic!("unexpected success media type {content_type}"),
        }
        row
    }

    fn divergence(id: &str) -> Value {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../../auctioneer-bidder/tests/conformance/fixtures/usersync/divergences.json"
        ))
        .unwrap();
        corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == id)
            .expect(id)
            .clone()
    }

    fn production_response(corpus: &Corpus, case: &Case) -> Response {
        let translated: Vec<_> = case
            .input
            .cookie_headers
            .iter()
            .map(|header| {
                Cookie::split_parse(header)
                    .map(|parsed| {
                        let mut cookie = parsed.unwrap().into_owned();
                        if cookie.name() == "uids" {
                            let decoded = payload_cookie::decode_cookie(
                                cookie.value(),
                                &CookieLimits::default(),
                            )
                            .unwrap();
                            if decoded.corrupt {
                                assert_eq!(
                                    case.id, "getuids_invalid",
                                    "only this fixture carries malformed payload"
                                );
                            } else {
                                cookie.set_value(
                                    usersync_keys::encode_cookie(
                                        &decoded.cookie,
                                        &CookieLimits::default(),
                                    )
                                    .unwrap(),
                                );
                            }
                        }
                        cookie.to_string()
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .collect();
        production_response_with_cookies(corpus, case, &translated)
    }

    fn production_response_with_cookies(
        corpus: &Corpus,
        case: &Case,
        cookies: &[String],
    ) -> Response {
        let input = &case.input;
        assert!(
            input.host_permission_stub.is_empty() || input.host_permission_stub == "malformed",
            "{} requires a reference permission stub, not replayable authorization",
            case.id
        );
        let now = OffsetDateTime::parse(&corpus.reference.now, &Rfc3339).unwrap();
        let mut host = configured_sync_host();
        host.host_cookie.ttl_days = input.host_cookie.ttl_days;
        host.gdpr.default_value.clone_from(&input.host_default_gdpr);
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.host_vendor_id = Some(32);
        host.user_sync.as_mut().unwrap().external_url = "https://sync.example.com".to_owned();
        host.user_sync.as_mut().unwrap().redirect_url = Some(CALLBACK.to_owned());
        let app: AuctioneerAppConfig = toml::from_str(
            "schema_version = 1\n[accounts.disabled]\ndisabled = true\n\
             [accounts.activity-deny.privacy.allowactivities.syncUser]\ndefault = false\n",
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        for cookie in cookies {
            headers.append("cookie", HeaderValue::from_str(cookie).unwrap());
        }
        if !input.user_agent.is_empty() {
            headers.insert(
                "user-agent",
                HeaderValue::from_str(&input.user_agent).unwrap(),
            );
        }
        let uri: Uri = input.target.parse().unwrap();
        match (input.method.as_str(), uri.path()) {
            ("GET", "/getuids") => block_on(super::getuids::respond(
                &headers,
                &host,
                now,
                ready(Ok(usersync_keys::keys())),
            )),
            ("GET", "/setuid") => block_on(super::setuid::tests::wire_response(
                uri.query(),
                &headers,
                &app,
                &host,
                now,
                ready(Ok(usersync_keys::keys())),
            )),
            ("POST", "/cookie_sync") => {
                let body = Body::from_bytes(input.body_utf8.clone().into_bytes());
                let raw = block_on(sync_http::read_cookie_sync_body(&headers, body)).unwrap();
                match parse_cookie_sync_body(&raw, 4096) {
                    Ok(request) => block_on(super::cookie_sync::tests::wire_response(
                        &request,
                        &headers,
                        &app,
                        &host,
                        now,
                        &auctioneer_runtime::SeededRng::new(1),
                        ready(Ok(usersync_keys::keys())),
                    )),
                    Err(error) => sync_http::cookie_sync_wire_error(error),
                }
            }
            _ => panic!("unsupported wire operation {}", case.id),
        }
    }

    fn reference_output(case: &Case) -> WireOutput {
        WireOutput {
            status: case.expected.status,
            headers: case
                .expected
                .headers
                .iter()
                .map(|(name, values)| (name.to_ascii_lowercase(), values.clone()))
                .collect(),
            body: STANDARD.decode(&case.expected.body_base64).unwrap(),
            set_cookie_present: case.expected.set_cookie_present,
        }
    }

    fn qualified_reference_output(case: &Case) -> WireOutput {
        let mut expected = reference_output(case);
        let no_store = divergence("no_store");
        assert!(no_store["expected"]["reference_cache_control"].is_null());
        assert!(!expected.headers.contains_key("cache-control"));
        expected.headers.insert(
            "cache-control".to_owned(),
            vec![
                no_store["expected"]["auctioneer_cache_control"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            ],
        );
        let target: Uri = case.input.target.parse().unwrap();
        if case.input.method == "GET"
            && target.path() == "/setuid"
            && expected.status == 200
            && expected.body.is_empty()
        {
            assert_eq!(expected.headers["content-type"], ["text/html"]);
            assert_eq!(expected.headers.remove("content-length").unwrap(), ["0"]);
        }
        if case.id == "getuids_invalid" {
            // The source accepted corrupt-as-fresh state. Signed transport
            // deliberately refuses it before interpreting any UID payload.
            assert_eq!(expected.status, 200);
            assert_eq!(expected.body, b"{}\n");
            expected.status = 400;
            expected.body = b"invalid UID cookie".to_vec();
            expected.headers.remove("content-type");
        }
        if matches!(case.id.as_str(), "setuid_clear" | "setuid_gdpr_empty") {
            let fields = expected.headers.get_mut("set-cookie").unwrap();
            assert_eq!(fields.len(), 1);
            let mut deletion = Cookie::parse(fields[0].clone()).unwrap();
            assert!(deletion.value().is_empty());
            assert!(deletion.max_age().is_none());
            assert!(deletion.expires_datetime().unwrap() > OffsetDateTime::UNIX_EPOCH);
            deletion.set_max_age(Duration::ZERO);
            deletion.set_expires(OffsetDateTime::UNIX_EPOCH);
            fields[0] = deletion.to_string();
        }
        expected
    }

    fn qualified_payload_field(raw: &str) -> Result<String, String> {
        if raw.len() > 4096 {
            return Err("signed Set-Cookie exceeds 4096 bytes".into());
        }
        let cookie =
            Cookie::parse(raw).map_err(|_error| "invalid Set-Cookie grammar".to_owned())?;
        if cookie.name() != "uids" {
            return Err("unexpected cookie name".into());
        }
        if cookie.value().is_empty() {
            return Ok(raw.to_owned());
        }
        let decoded = usersync_keys::decode_cookie(cookie.value(), &CookieLimits::default())
            .map_err(|_error| "invalid signed output cookie".to_owned())?;
        if decoded.corrupt {
            return Err("invalid authenticated output payload".into());
        }
        let payload = payload_cookie::encode_cookie(&decoded.cookie, &CookieLimits::default())
            .map_err(|_error| "invalid output payload".to_owned())?;
        let (_, attributes) = raw
            .split_once(';')
            .ok_or_else(|| "missing cookie attributes".to_owned())?;
        // Only the authenticated envelope is projected away; attribute values,
        // payload UID and entry expiry still compare with the source.
        Ok(format!("uids={payload};{attributes}"))
    }

    fn canonical_cookie_attributes(raw: &str) -> String {
        let Some((pair, attributes)) = raw.split_once(';') else {
            return raw.to_owned();
        };
        // Retain all tokens: Cookie's formatter would lose unknown attributes.
        let mut tokens: Vec<_> = attributes.split(';').map(str::trim).collect();
        tokens.sort_unstable();
        format!("{pair}; {}", tokens.join("; "))
    }

    fn compare(expected: &WireOutput, response: &Response) -> Result<(), String> {
        let mut actual = WireOutput {
            status: response.status().as_u16(),
            headers: response
                .headers()
                .keys()
                .map(|name| {
                    let values = response
                        .headers()
                        .get_all(name)
                        .iter()
                        .map(|value| value.to_str().unwrap().to_owned())
                        .collect();
                    (name.as_str().to_owned(), values)
                })
                .collect(),
            body: response
                .body()
                .as_bytes()
                .expect("buffered production response")
                .to_vec(),
            set_cookie_present: response.headers().contains_key("set-cookie"),
        };
        if let Some(fields) = actual.headers.get_mut("set-cookie") {
            for field in fields {
                *field = canonical_cookie_attributes(&qualified_payload_field(field)?);
            }
        }
        let mut qualified_expected = expected.clone();
        if let Some(fields) = qualified_expected.headers.get_mut("set-cookie") {
            for field in fields {
                *field = canonical_cookie_attributes(field);
            }
        }
        if actual != qualified_expected {
            return Err(format!("expected {expected:?}, got {actual:?}"));
        }
        Ok(())
    }

    fn assert_case(corpus: &Corpus, id: &str) {
        let case = case(corpus, id);
        assert_response(corpus, case);
    }

    fn assert_response(corpus: &Corpus, case: &Case) {
        let response = production_response(corpus, case);
        let expected = qualified_reference_output(case);
        assert_eq!(compare(&expected, &response), Ok(()), "{}", case.id);
    }

    #[test]
    fn semantic_success_selection_ignores_ids_and_rejects_missing_or_ambiguous_rows() {
        let mut renamed = corpus();
        assert_eq!(renamed.cases.len(), 42);
        for (index, row) in renamed.cases.iter_mut().enumerate() {
            row.id = format!("capture-{index}");
        }
        let mut missing = corpus();
        missing.cases.clear();
        let mut duplicated = corpus();
        duplicated.cases.extend(corpus().cases);
        for (format, content_type) in [
            (None, "image/png"),
            (None, "text/html"),
            (Some("i"), "image/png"),
        ] {
            assert_response(&renamed, successful_setuid(&renamed, format, content_type));
            assert!(
                catch_unwind(|| { successful_setuid(&missing, format, content_type) }).is_err()
            );
            assert!(
                catch_unwind(|| { successful_setuid(&duplicated, format, content_type) }).is_err()
            );
        }
    }

    #[test]
    fn getuids_payload_parity_with_signed_transport_and_corrupt_cookie_rejection() {
        let corpus = corpus();
        for id in [
            "getuids_empty",
            "getuids_live",
            "getuids_expired_retained",
            "getuids_optout",
            "getuids_invalid",
            "getuids_html_escaping",
        ] {
            assert_case(&corpus, id);
        }
    }

    #[test]
    fn setuid_default_image_payload_parity_with_signed_transport() {
        let corpus = corpus();
        assert_response(&corpus, successful_setuid(&corpus, None, "image/png"));
    }

    #[test]
    fn setuid_image_error_and_deletion_payload_parity_with_signed_transport() {
        let corpus = corpus();
        assert_response(&corpus, successful_setuid(&corpus, Some("i"), "image/png"));
        assert_response(&corpus, successful_setuid(&corpus, None, "text/html"));
        for id in [
            "setuid_uppercase_format",
            "setuid_duplicate_query_first_wins",
            "setuid_clear",
            "setuid_gdpr_empty",
            "setuid_chrome_66",
            "setuid_chrome_67",
            "setuid_crios_67",
            "setuid_chrome_invalid",
            "setuid_missing_bidder",
            "setuid_unknown_bidder",
            "setuid_invalid_format",
            "setuid_optout",
            "setuid_gdpr_missing_consent",
            "setuid_activity_denial",
            "setuid_disabled_account",
            "setuid_invalid_sid",
        ] {
            assert_case(&corpus, id);
        }
    }

    #[test]
    fn unsigned_reference_wire_inputs_are_rejected_without_fixture_translation() {
        let corpus = corpus();
        for (id, body) in [
            ("getuids_live", b"invalid UID cookie".as_slice()),
            ("cookie_sync_live", b"invalid UID cookie\n".as_slice()),
            ("setuid_clear", b"invalid UID cookie".as_slice()),
        ] {
            let case = case(&corpus, id);
            let response =
                production_response_with_cookies(&corpus, case, &case.input.cookie_headers);
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{id}");
            assert_eq!(response.body().as_bytes().unwrap(), body, "{id}");
            assert!(!response.headers().contains_key("set-cookie"));
            assert_eq!(response.headers()["cache-control"], "no-store");
        }
    }

    #[test]
    fn cookie_sync_payload_parity_with_signed_fixture_inputs() {
        let corpus = corpus();
        for id in [
            "cookie_sync_empty",
            "cookie_sync_selected",
            "cookie_sync_live",
            "cookie_sync_expired",
            "cookie_sync_optout",
            "cookie_sync_required_consent",
            "cookie_sync_default_requires_consent",
            "cookie_sync_disabled_account",
        ] {
            assert_case(&corpus, id);
        }
    }

    #[test]
    fn safe_diagnostics_are_explicit_divergences_not_reference_parity() {
        let corpus = corpus();
        let safe = divergence("safe_diagnostics");
        assert_eq!(safe["input"]["reference_case"], "setuid_malformed_consent");
        assert_eq!(safe["expected"]["auctioneer_echoes_consent"], false);
        // Design sections 9/13 require fixed, non-echoing diagnostics. Do not
        // compare only status or normalize either side into a matching prefix.
        for (id, body) in [
            (
                "cookie_sync_malformed",
                "JSON parsing failed: invalid JSON\n",
            ),
            (
                "cookie_sync_invalid_filter",
                "error parsing filtersettings.image: invalid filter value. must be either 'include' or 'exclude'\n",
            ),
            (
                "setuid_gdpr_invalid",
                "the gdpr query param must be either 0 or 1",
            ),
            (
                "setuid_malformed_consent",
                "gdpr_consent was invalid. malformed TCF consent",
            ),
        ] {
            let case = case(&corpus, id);
            let mut expected = qualified_reference_output(case);
            assert_ne!(
                expected.body,
                body.as_bytes(),
                "{id} must be a real divergence"
            );
            assert_eq!(expected.status, 400);
            assert!(!expected.set_cookie_present);
            expected.body = body.as_bytes().to_vec();
            let response = production_response(&corpus, case);
            assert_eq!(compare(&expected, &response), Ok(()), "{id}");
        }
    }

    #[test]
    fn empty_explicit_format_is_a_documented_rejection_not_reference_success() {
        let corpus = corpus();
        let empty = divergence("empty_explicit_format");
        let case = case(&corpus, empty["input"]["reference_case"].as_str().unwrap());
        assert_eq!(case.expected.status, 200);
        assert!(case.expected.set_cookie_present);
        let mut expected = qualified_reference_output(case);
        expected.status =
            u16::try_from(empty["expected"]["auctioneer_status"].as_u64().unwrap()).unwrap();
        expected.set_cookie_present = empty["expected"]["auctioneer_set_cookie_present"]
            .as_bool()
            .unwrap();
        for name in ["content-type", "content-length", "set-cookie"] {
            expected.headers.remove(name);
        }
        expected.body = b"\"f\" query param is invalid. must be \"b\" or \"i\"".to_vec();
        let response = production_response(&corpus, case);
        assert_eq!(compare(&expected, &response), Ok(()));
    }

    #[test]
    fn credentialed_cors_is_a_documented_addition_to_the_reference_handler() {
        let corpus = corpus();
        let case = case(&corpus, "getuids_live");
        let cors = divergence("credentialed_cors");
        let config = UserSyncConfig {
            allowed_origins: serde_json::from_value(cors["input"]["allowed_origins"].clone())
                .unwrap(),
            ..UserSyncConfig::default()
        };
        let mut headers = HeaderMap::new();
        headers.insert(
            "origin",
            HeaderValue::from_str(cors["input"]["origin"].as_str().unwrap()).unwrap(),
        );
        let response = sync_http::SyncBrowserPolicy::new(&config)
            .decorate(production_response(&corpus, case), &headers, 16 * 1024)
            .unwrap();
        let mut expected = qualified_reference_output(case);
        let additions: BTreeMap<String, Vec<String>> =
            serde_json::from_value(cors["expected"]["auctioneer_headers"].clone()).unwrap();
        for (name, values) in additions {
            let previous = expected.headers.insert(name.to_ascii_lowercase(), values);
            assert!(
                previous.is_none(),
                "reference handler must not supply {name}"
            );
        }
        assert_eq!(compare(&expected, &response), Ok(()));
    }

    #[test]
    fn comparator_rejects_authenticated_uid_and_entry_expiry_changes() {
        let corpus = corpus();
        let case = successful_setuid(&corpus, Some("i"), "image/png");
        let expected = qualified_reference_output(case);
        let target: Uri = case.input.target.parse().unwrap();
        let requested: Vec<_> = form_urlencoded::parse(target.query().unwrap().as_bytes())
            .filter(|(name, _value)| name == "bidder")
            .map(|(_name, value)| value.into_owned())
            .collect();
        assert_eq!(requested.len(), 1);
        let now = OffsetDateTime::parse(&corpus.reference.now, &Rfc3339).unwrap();
        for change_expiry in [false, true] {
            let mut response = production_response(&corpus, case);
            assert_eq!(compare(&expected, &response), Ok(()));
            let mut field = Cookie::parse(response.headers()["set-cookie"].to_str().unwrap())
                .unwrap()
                .into_owned();
            let mut decoded =
                usersync_keys::decode_cookie(field.value(), &CookieLimits::default()).unwrap();
            decoded
                .cookie
                .set_uid(
                    &requested[0],
                    if change_expiry {
                        "123"
                    } else {
                        "different-user"
                    },
                    if change_expiry {
                        now.checked_add(Duration::seconds(1)).unwrap()
                    } else {
                        now
                    },
                    &CookieLimits::default(),
                )
                .unwrap();
            let signed =
                usersync_keys::encode_cookie(&decoded.cookie, &CookieLimits::default()).unwrap();
            assert_eq!(
                usersync_keys::decode_cookie(&signed, &CookieLimits::default())
                    .unwrap()
                    .cookie,
                decoded.cookie
            );
            field.set_value(signed);
            response.headers_mut().insert(
                "set-cookie",
                HeaderValue::from_str(&field.to_string()).unwrap(),
            );
            assert!(
                compare(&expected, &response).is_err(),
                "entry expiry changed={change_expiry}"
            );
        }
    }

    #[test]
    fn comparator_rejects_incorrect_deletion_expiry_age_and_scope() {
        let corpus = corpus();
        let case = case(&corpus, "setuid_clear");
        let expected = qualified_reference_output(case);
        let now = OffsetDateTime::parse(&corpus.reference.now, &Rfc3339).unwrap();
        for mutation in [
            "future-expires",
            "missing-max-age",
            "nonzero-max-age",
            "path",
            "domain",
        ] {
            let mut response = production_response(&corpus, case);
            assert_eq!(compare(&expected, &response), Ok(()));
            let mut field = Cookie::parse(response.headers()["set-cookie"].to_str().unwrap())
                .unwrap()
                .into_owned();
            match mutation {
                "future-expires" => field.set_expires(now.checked_add(Duration::days(90)).unwrap()),
                "missing-max-age" => {
                    field = Cookie::build(("uids", ""))
                        .path("/")
                        .expires(OffsetDateTime::UNIX_EPOCH)
                        .build();
                }
                "nonzero-max-age" => field.set_max_age(Duration::seconds(1)),
                "path" => field.set_path("/different"),
                "domain" => field.set_domain("different.example"),
                _ => panic!("unknown deletion mutation {mutation}"),
            }
            response.headers_mut().insert(
                "set-cookie",
                HeaderValue::from_str(&field.to_string()).unwrap(),
            );
            assert!(
                compare(&expected, &response).is_err(),
                "missed deletion {mutation}"
            );
        }
    }

    #[test]
    fn comparator_rejects_mutated_status_headers_cookie_and_body() {
        let corpus = corpus();
        let case = case(&corpus, "getuids_live");
        let expected = qualified_reference_output(case);
        for mutation in [
            "status",
            "content-type",
            "set-cookie",
            "extra-header",
            "no-store",
            "newline",
            "body",
        ] {
            let mut response = production_response(&corpus, case);
            match mutation {
                "status" => *response.status_mut() = StatusCode::BAD_REQUEST,
                "content-type" => {
                    response.headers_mut().remove("content-type");
                }
                "set-cookie" => {
                    response
                        .headers_mut()
                        .insert("set-cookie", HeaderValue::from_static("uids=bad"));
                }
                "extra-header" => {
                    response
                        .headers_mut()
                        .insert("x-unexpected", HeaderValue::from_static("true"));
                }
                "no-store" => {
                    response.headers_mut().remove("cache-control");
                }
                "newline" => {
                    let mut body = response.body().as_bytes().unwrap().to_vec();
                    assert_eq!(body.pop(), Some(b'\n'));
                    *response.body_mut() = Body::from_bytes(body);
                }
                "body" => *response.body_mut() = Body::from_bytes(b"{}".to_vec()),
                _ => panic!("unknown mutation {mutation}"),
            }
            assert!(
                compare(&expected, &response).is_err(),
                "missed {mutation} mutation"
            );
        }
    }

    #[test]
    fn comparator_rejects_cookie_field_length_and_png_byte_mutations() {
        let corpus = corpus();
        let case = successful_setuid(&corpus, Some("i"), "image/png");
        let expected = qualified_reference_output(case);
        for mutation in [
            "cookie-value",
            "missing-cookie",
            "content-length",
            "png-byte",
        ] {
            let mut response = production_response(&corpus, case);
            match mutation {
                "cookie-value" => {
                    response
                        .headers_mut()
                        .insert("set-cookie", HeaderValue::from_static("uids=bad"));
                }
                "missing-cookie" => {
                    response.headers_mut().remove("set-cookie");
                }
                "content-length" => {
                    response
                        .headers_mut()
                        .insert("content-length", HeaderValue::from_static("85"));
                }
                "png-byte" => {
                    let mut bytes = response.body().as_bytes().unwrap().to_vec();
                    *bytes.last_mut().unwrap() ^= 1;
                    *response.body_mut() = Body::from_bytes(bytes);
                }
                _ => panic!("unknown mutation {mutation}"),
            }
            assert!(
                compare(&expected, &response).is_err(),
                "missed {mutation} mutation"
            );
        }
    }
}

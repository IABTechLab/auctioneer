//! Native-only conformance for discovered factories using adjacent, independent cases.

use crate::BidderSettings;
use crate::bidders::harness;
use crate::{
    BidderError, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse, EntryPoint,
    compiled_bidders,
};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::bidder_credentials::BidderCredentials;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: u8,
    implementation: String,
    manifest_sha256: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    request: BidRequest,
    context: Context,
    outbound: Vec<Outbound>,
    responses: Vec<Response>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Context {
    account_id: String,
    endpoint_template: String,
    endpoint_macros: Vec<String>,
    host_allowlist: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_recorded_settings")]
    settings: Option<BidderSettings>,
    #[serde(default)]
    credentials: Option<Value>,
}

/// Normalize the historical `values` envelope without changing capture bytes.
fn deserialize_recorded_settings<'de, D>(
    deserializer: D,
) -> Result<Option<BidderSettings>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error as _;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RecordedSettings {
        implementation: String,
        values: Map<String, Value>,
    }

    let Some(RecordedSettings {
        implementation,
        values,
    }) = Option::<RecordedSettings>::deserialize(deserializer)?
    else {
        return Ok(None);
    };
    let document = Map::from_iter([
        ("implementation".to_owned(), Value::String(implementation)),
        ("settings".to_owned(), Value::Object(values)),
    ]);
    serde_json::from_value(Value::Object(document))
        .map(Some)
        .map_err(|_error| D::Error::custom("invalid recorded settings"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outbound {
    method: String,
    url: String,
    required_headers: Vec<[String; 2]>,
    absent_headers: Vec<String>,
    imp_ids: Vec<String>,
    body: Value,
    #[serde(default)]
    package_version_paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    name: String,
    call: usize,
    status: u16,
    body: Value,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Expected {
    Ok {
        currency: String,
        bids: Vec<ExpectedBid>,
        errors: Vec<u32>,
        #[serde(default)]
        auction_configs: Vec<ExpectedBrowserConfig>,
    },
    Error {
        errors: Vec<u32>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedBid {
    id: String,
    impid: String,
    media_type: MediaType,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedBrowserConfig {
    impid: String,
    config: Value,
}

pub(crate) fn assert_factory_contract(key: &str, source: &str) {
    let fixture: Fixture = serde_json::from_str(source).unwrap();
    assert_eq!(fixture.schema_version, 1, "factory fixture version");
    assert_eq!(fixture.implementation, key, "factory fixture owner");
    assert!(!fixture.cases.is_empty(), "factory fixture has no cases");
    let registration = compiled_bidders()
        .iter()
        .find(|entry| entry.key == key)
        .unwrap();
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/bidders")
        .join(key)
        .join("bidder.toml");
    assert_eq!(
        format!("{:x}", Sha256::digest(fs::read(manifest).unwrap())),
        fixture.manifest_sha256,
        "factory manifest digest"
    );
    let mut names = BTreeSet::new();
    for case in &fixture.cases {
        assert!(
            !case.name.is_empty() && names.insert(&case.name),
            "factory case names must be nonempty and unique"
        );
        if let Some(settings) = &case.context.settings {
            assert_eq!(settings.implementation_key(), key, "factory settings owner");
        }
        let bidder = (registration.factory)();
        let before = serde_json::to_value(&case.request).unwrap();
        let macros: Vec<_> = case
            .context
            .endpoint_macros
            .iter()
            .map(String::as_str)
            .collect();
        let hosts = case
            .context
            .host_allowlist
            .as_ref()
            .map(|values| values.iter().map(String::as_str).collect::<Vec<_>>());
        let mut info = BidderRequestInfo::new(
            BidderName::new(key),
            BidderName::new(key),
            &case.context.account_id,
            None,
            &case.context.endpoint_template,
            &macros,
            hosts.as_deref(),
            None,
            EntryPoint::Auction,
            &[],
        );
        info.set_settings(Some(key), case.context.settings.as_ref(), None)
            .unwrap();
        let credentials =
            case.context.credentials.as_ref().map(|value| {
                BidderCredentials::parse(&serde_json::to_vec(value).unwrap()).unwrap()
            });
        info.credentials = credentials.as_ref();
        assert!(
            !case.outbound.is_empty(),
            "factory case must exercise construction"
        );
        let calls = harness::drive_ok(bidder.as_ref(), &case.request, &info);
        assert_eq!(
            serde_json::to_value(&case.request).unwrap(),
            before,
            "factory mutated its caller"
        );
        assert_eq!(
            calls.len(),
            case.outbound.len(),
            "factory call count: {key}/{}",
            case.name
        );
        for (call, expected) in calls.iter().zip(&case.outbound) {
            assert_outbound(call, expected);
        }
        assert!(
            !case.responses.is_empty(),
            "factory case must exercise decoding"
        );
        let mut response_names = BTreeSet::new();
        for response in &case.responses {
            assert!(
                !response.name.is_empty() && response_names.insert(&response.name),
                "factory response names must be nonempty and unique"
            );
            let call = &calls[response.call];
            let decoded = bidder.make_bids(
                &case.request,
                call.meta(),
                &harness::budgeted_response(
                    response.status,
                    serde_json::to_vec(&response.body).unwrap(),
                ),
            );
            assert_response(decoded, &response.expected);
        }
    }
}

fn assert_outbound(call: &BidderHttpRequest, expected: &Outbound) {
    assert_eq!(call.method(), expected.method, "factory outbound method");
    assert_eq!(call.url(), expected.url, "factory outbound URI");
    assert_eq!(
        call.covered_imp_ids().collect::<Vec<_>>(),
        expected.imp_ids,
        "factory outbound coverage"
    );
    for [name, value] in &expected.required_headers {
        assert!(
            call.headers().any(|(actual_name, actual_value)| actual_name
                .eq_ignore_ascii_case(name)
                && actual_value == value),
            "missing factory header {name}"
        );
    }
    for name in &expected.absent_headers {
        assert!(
            !call
                .headers()
                .any(|(actual, _value)| actual.eq_ignore_ascii_case(name)),
            "unexpected factory header {name}"
        );
    }
    assert_eq!(
        serde_json::from_slice::<Value>(call.body()).unwrap(),
        expected_body(expected, env!("CARGO_PKG_VERSION")),
        "factory outbound body"
    );
}

fn expected_body(outbound: &Outbound, package_version: &str) -> Value {
    let mut body = outbound.body.clone();
    for path in &outbound.package_version_paths {
        let field = body
            .pointer_mut(path)
            .unwrap_or_else(|| panic!("factory package-version pointer does not exist: {path}"));
        assert_eq!(
            field.as_str(),
            Some("$CARGO_PKG_VERSION"),
            "factory package-version pointer must replace an explicit marker"
        );
        *field = Value::String(package_version.to_owned());
    }
    body
}

fn assert_response(result: Result<BidderResponse, Vec<BidderError>>, expected: &Expected) {
    match expected {
        Expected::Ok {
            currency,
            bids,
            errors,
            auction_configs,
        } => {
            let actual = result.unwrap();
            assert_eq!(&actual.currency, currency, "factory response currency");
            assert_eq!(actual.bids.len(), bids.len(), "factory response bid count");
            for (bid, expected_bid) in actual.bids.iter().zip(bids) {
                assert_eq!(bid.bid.id, expected_bid.id, "factory response bid ID");
                assert_eq!(
                    bid.bid.impid, expected_bid.impid,
                    "factory response impression ID"
                );
                assert_eq!(
                    bid.media_type, expected_bid.media_type,
                    "factory response media type"
                );
            }
            assert_eq!(
                actual.auction_configs.len(),
                auction_configs.len(),
                "factory response browser configuration count"
            );
            for (config, expected_config) in actual.auction_configs.iter().zip(auction_configs) {
                assert_eq!(
                    config.imp_id, expected_config.impid,
                    "factory configuration impression ID"
                );
                assert_eq!(
                    config.config, expected_config.config,
                    "factory configuration payload"
                );
            }
            assert_eq!(
                actual
                    .errors
                    .iter()
                    .map(|error| error.code)
                    .collect::<Vec<_>>(),
                *errors,
                "factory response partial errors"
            );
        }
        Expected::Error { errors } => assert_eq!(
            result
                .unwrap_err()
                .iter()
                .map(|error| error.code)
                .collect::<Vec<_>>(),
            *errors,
            "factory response fatal errors"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{Expected, Outbound, assert_response, expected_body};
    use crate::{BidderResponse, BrowserAuctionConfig};
    use serde_json::Value;
    use std::panic::catch_unwind;

    fn outbound_fixture() -> Value {
        serde_json::json!({
            "method":"POST", "url":"https://fake-bidder.example/auction",
            "required_headers":[], "absent_headers":[], "imp_ids":["i"],
            "body":{"id":"fake_request"}, "package_version_paths":[]
        })
    }

    #[test]
    fn factory_contract_checks_optional_configuration_identity_and_payload() {
        let expected: Expected = serde_json::from_value(serde_json::json!({
            "kind":"ok", "currency":"USD", "bids":[], "errors":[],
            "auction_configs":[{"impid":"i", "config":{"payload":"expected"}}]
        }))
        .unwrap();
        for (imp_id, payload) in [("wrong", "expected"), ("i", "wrong")] {
            let result = BidderResponse {
                bids: vec![],
                currency: "USD".into(),
                errors: vec![],
                auction_configs: vec![BrowserAuctionConfig {
                    imp_id: imp_id.into(),
                    config: serde_json::json!({"payload":payload}),
                }],
            };
            assert!(catch_unwind(|| assert_response(Ok(result), &expected)).is_err());
        }
    }

    #[test]
    fn expected_build_version_changes_only_the_declared_field() {
        let mut row = outbound_fixture();
        row["body"] =
            serde_json::json!({"version":"$CARGO_PKG_VERSION", "literal":"$CARGO_PKG_VERSION"});
        row["package_version_paths"] = serde_json::json!(["/version"]);
        let outbound: Outbound = serde_json::from_value(row).unwrap();
        assert_eq!(
            expected_body(&outbound, "9.8.7"),
            serde_json::json!({"version":"9.8.7", "literal":"$CARGO_PKG_VERSION"})
        );
    }

    #[test]
    #[should_panic(expected = "factory package-version pointer does not exist")]
    fn expected_build_version_cannot_ignore_a_missing_field() {
        let mut row = outbound_fixture();
        row["package_version_paths"] = serde_json::json!(["/missing"]);
        let outbound: Outbound = serde_json::from_value(row).unwrap();
        expected_body(&outbound, "9.8.7");
    }
}

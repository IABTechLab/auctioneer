//! APS protocol transformation through the ordinary bounded bidder contract.
//!
//! Source: adapters/aps at f1e9d0d2164e76a8bcb5f10021ebb56e00dbfe9a.
//! Ordered source witnesses and the shared license are declared in bidder.toml.

mod request;
mod response;

use crate::bidder::{Bidder, BidderError};
use crate::debug_projection::DebugBodySchema;
use crate::meta::OutboundRequestMeta;
use crate::request_info::BidderRequestInfo;
use crate::sink::OutboundRequestSink;
use crate::types::{BidderHttpResponse, BidderResponse};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct ApsBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(ApsBidder)
}

impl Bidder for ApsBidder {
    fn debug_body_schema(&self) -> DebugBodySchema {
        // The APS envelope adds account/SDK fields outside the reviewed
        // OpenRTB projection. Do not disclose the body through that schema.
        DebugBodySchema::Unsupported
    }

    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>> {
        request::make_requests(request, info, sink)
    }

    fn make_bids(
        &self,
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        response::make_bids(request, sent, response)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    mod conformance;

    use super::ApsBidder;
    use crate::bidder::{Bidder as _, BidderError, EntryPoint};
    use crate::bidders::harness::drive_with_limits;
    use crate::debug_projection::DebugBodySchema;
    #[cfg(not(target_arch = "wasm32"))]
    use crate::diagnostics::ConstructionDiagnostic;
    use crate::endpoint_validator::{EndpointRejectReason, EndpointValidator};
    use crate::request_info::BidderRequestInfo;
    use crate::sink::{CallBudget, OutboundRequestSink, SinkLimits, SinkOutcome};
    #[cfg(not(target_arch = "wasm32"))]
    use crate::types::BidderHttpRequest;
    use crate::types::{BidderHttpResponse, BidderName};
    #[cfg(not(target_arch = "wasm32"))]
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::bounded_json::{JsonFieldShape, count_json_nodes_with_field};
    use serde::Serialize;
    use serde_json::{Value, json};

    const ENDPOINT: &str = "https://{{.Host}}/e/pb/bid";
    pub(super) const HOSTS: &[&str] = &[
        "s2s.prebid.bid-na.ads.aps.amazon-adsystem.com",
        "s2s.prebid.bid-eu.ads.aps.amazon-adsystem.com",
        "s2s.prebid.bid-fe.ads.aps.amazon-adsystem.com",
    ];
    pub(super) const NA: &str = "https://s2s.prebid.bid-na.ads.aps.amazon-adsystem.com/e/pb/bid";
    pub(super) const EU: &str = "https://s2s.prebid.bid-eu.ads.aps.amazon-adsystem.com/e/pb/bid";
    pub(super) const FE: &str = "https://s2s.prebid.bid-fe.ads.aps.amazon-adsystem.com/e/pb/bid";
    pub(super) const LIMITS: SinkLimits = SinkLimits::new(64 * 1024, 16);
    const NODE_LIMIT: u32 = 8_192;

    // The exchange's standard validator is an integration control, not a bidder
    // dependency. This stricter unit-test policy admits only the reviewed origins.
    struct ReviewedEndpoints;

    impl EndpointValidator for ReviewedEndpoints {
        fn validate(
            &self,
            url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if url.scheme() != "https" {
                return Err(EndpointRejectReason::SchemeNotAllowed);
            }
            if !url.username().is_empty() || url.password().is_some() {
                return Err(EndpointRejectReason::UserinfoRejected);
            }
            if url.port().is_some() {
                return Err(EndpointRejectReason::PortNotAllowed);
            }
            if !matches!(
                url.host_str(),
                Some(
                    "s2s.prebid.bid-na.ads.aps.amazon-adsystem.com"
                        | "s2s.prebid.bid-eu.ads.aps.amazon-adsystem.com"
                        | "s2s.prebid.bid-fe.ads.aps.amazon-adsystem.com"
                )
            ) || url.path() != "/e/pb/bid"
            {
                return Err(EndpointRejectReason::AuthorityMismatch);
            }
            Ok(())
        }
    }

    pub(super) fn info(shape: JsonFieldShape) -> BidderRequestInfo<'static> {
        let mut info = BidderRequestInfo::new(
            BidderName::new("aps"),
            BidderName::new("aps"),
            "host-account-not-the-aps-account",
            None,
            ENDPOINT,
            &[],
            Some(HOSTS),
            None,
            EntryPoint::Auction,
            &[],
        );
        info.original_request_ext_shape = shape;
        info
    }

    pub(super) fn shape(wire: &[u8]) -> JsonFieldShape {
        count_json_nodes_with_field(wire, NODE_LIMIT, "ext")
            .expect("bounded fixture JSON")
            .1
    }

    pub(super) fn request(imps: Value) -> BidRequest {
        BidRequest {
            id: "r".into(),
            imp: serde_json::from_value(imps).unwrap(),
            ..BidRequest::default()
        }
    }

    pub(super) fn imp(id: &str, params: Value) -> Value {
        let mut imp = json!({"id": id, "banner": {}, "ext": {}});
        imp["ext"]["bidder"] = params;
        imp
    }

    pub(super) fn minimal() -> BidRequest {
        request(json!([imp("i", json!({"accountID": "account"}))]))
    }

    pub(super) fn value<T: Serialize>(input: &T) -> Value {
        serde_json::from_slice(&serde_json::to_vec(input).unwrap()).unwrap()
    }

    pub(super) fn drive_with(
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        limits: SinkLimits,
        max_calls: u32,
    ) -> (Result<(), Vec<BidderError>>, SinkOutcome, u32) {
        // Do not discard on Err here: the bidder itself must push zero calls on
        // conflicting accounts, rather than relying on the exchange's rollback.
        drive_with_limits(
            &ApsBidder,
            request,
            info,
            limits,
            max_calls,
            &ReviewedEndpoints,
        )
    }

    pub(super) fn drive(request: &BidRequest) -> SinkOutcome {
        let wire = serde_json::to_vec(request).unwrap();
        let (semantic, outcome, remaining) = drive_with(request, &info(shape(&wire)), LIMITS, 1);
        semantic.unwrap();
        assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
        assert_eq!(
            remaining,
            1_u32.saturating_sub(u32::try_from(outcome.accepted.len()).unwrap())
        );
        outcome
    }

    pub(super) fn body(outcome: &SinkOutcome) -> Value {
        assert_eq!(outcome.accepted.len(), 1);
        serde_json::from_slice(outcome.accepted[0].body()).unwrap()
    }

    pub(super) fn response(status: u16, body: Vec<u8>) -> BidderHttpResponse {
        BidderHttpResponse::new(
            status,
            vec![("Content-Type".into(), "application/json".into())],
            body,
        )
        .within_node_budget(NODE_LIMIT)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn run_fixture(name: &str, source: &str) {
        let fixture: Value = serde_json::from_str(source).unwrap();
        let wire = serde_json::to_vec(&fixture["mockBidRequest"]).unwrap();
        let request: BidRequest = serde_json::from_slice(&wire).unwrap();
        let before = serde_json::to_vec(&request).unwrap();
        let (semantic, outcome, _) = drive_with(&request, &info(shape(&wire)), LIMITS, 1);
        let expected_calls = fixture["httpCalls"].as_array().unwrap();
        assert_eq!(outcome.accepted.len(), expected_calls.len(), "{name}");
        assert!(outcome.errors.is_empty(), "{name}: {:?}", outcome.errors);
        assert_eq!(serde_json::to_vec(&request).unwrap(), before, "{name}");

        if expected_calls.is_empty() {
            assert_eq!(
                fixture["expectedMakeRequestsErrors"]
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            if name == "rejects_mismatched_account" {
                let errors = semantic.unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, 2);
                assert!(outcome.diagnostics.is_empty());
            } else {
                semantic.unwrap();
                // Source request-text errors become fixed, funded diagnostics.
                assert!(
                    outcome
                        .diagnostics
                        .iter()
                        .eq([ConstructionDiagnostic::InvalidImpressionParameters].iter())
                );
            }
            return;
        }
        semantic.unwrap();
        assert!(outcome.diagnostics.is_empty());
        for (actual, call) in outcome.accepted.iter().zip(expected_calls) {
            let expected = &call["expectedRequest"];
            assert_eq!(actual.method(), "POST", "{name}");
            assert_eq!(actual.url(), expected["uri"].as_str().unwrap(), "{name}");
            assert_eq!(
                actual.headers().collect::<Vec<_>>(),
                [
                    ("Content-Type", "application/json"),
                    ("Accept", "application/json")
                ],
                "{name}"
            );
            assert_eq!(
                actual.covered_imp_ids().collect::<Vec<_>>(),
                expected["impIDs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap())
                    .collect::<Vec<_>>(),
                "{name}"
            );
            let mut expected_body = expected["body"].clone();
            // Approved provider-visible identity difference only. Captured files
            // and every other expected field remain unchanged and independent.
            assert_eq!(expected_body["ext"]["sdk"]["source"], "prebid-server");
            assert_eq!(expected_body["ext"]["sdk"]["version"], "1.0.0");
            expected_body["ext"]["sdk"]["source"] = json!("auctioneer");
            expected_body["ext"]["sdk"]["version"] = json!(env!("CARGO_PKG_VERSION"));
            let actual_body: Value = serde_json::from_slice(actual.body()).unwrap();
            assert_eq!(actual_body, expected_body, "{name}");

            check_fixture_response(name, &request, actual, call, &fixture);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn check_fixture_response(
        name: &str,
        request: &BidRequest,
        actual: &BidderHttpRequest,
        call: &Value,
        fixture: &Value,
    ) {
        let mock = &call["mockResponse"];
        let status = u16::try_from(mock["status"].as_u64().unwrap()).unwrap();
        let bytes = mock
            .get("body")
            .map_or_else(Vec::new, |body| serde_json::to_vec(body).unwrap());
        let result = ApsBidder.make_bids(request, actual.meta(), &response(status, bytes));
        if let Some(errors) = fixture.get("expectedMakeBidsErrors") {
            let actual_errors = result.unwrap_err();
            let expected_errors = errors.as_array().unwrap();
            assert_eq!(actual_errors.len(), expected_errors.len(), "{name}");
            for (error, expected_error) in actual_errors.iter().zip(expected_errors) {
                assert_eq!(error.code, if status == 400 { 2 } else { 4 });
                assert_eq!(error.message, expected_error["value"].as_str().unwrap());
            }
        } else {
            let decoded = result.unwrap();
            assert!(decoded.errors.is_empty(), "{name}");
            let expected_responses = fixture["expectedBidResponses"].as_array().unwrap();
            if expected_responses.is_empty() {
                assert!(decoded.bids.is_empty(), "{name}");
            } else {
                assert_eq!(expected_responses.len(), 1);
                let expected_response = &expected_responses[0];
                assert_eq!(
                    decoded.currency,
                    expected_response["currency"].as_str().unwrap()
                );
                let bids = expected_response["bids"].as_array().unwrap();
                assert_eq!(decoded.bids.len(), bids.len());
                for (actual_bid, expected) in decoded.bids.iter().zip(bids) {
                    assert_eq!(value(&actual_bid.bid), expected["bid"], "{name}");
                    assert_eq!(
                        actual_bid.media_type,
                        match expected["type"].as_str().unwrap() {
                            "banner" => MediaType::Banner,
                            "video" => MediaType::Video,
                            other => panic!("unexpected fixture type: {other}"),
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn discovery_constructs_the_standard_bidder_with_unsupported_debug_schema() {
        let registration = crate::compiled_bidders()
            .iter()
            .find(|row| row.key == "aps")
            .unwrap();
        assert_eq!(
            (registration.factory)().debug_body_schema(),
            DebugBodySchema::Unsupported
        );
        assert_eq!(ApsBidder.debug_body_schema(), DebugBodySchema::Unsupported);
        let declaration = crate::bidder_declarations()
            .iter()
            .find(|row| row.key == "aps")
            .unwrap();
        assert_eq!(declaration.required_feature, None);
        let bidder = (registration.factory)();
        let budget = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(LIMITS, &budget, &ReviewedEndpoints);
        bidder
            .make_requests(&minimal(), &info(JsonFieldShape::Absent), &mut sink)
            .unwrap();
        let outcome = sink.into_outcome();
        assert!(outcome.errors.is_empty());
        assert_eq!(
            body(&outcome)["ext"],
            json!({
                "account": "account", "sdk": {"source": "auctioneer", "version": env!("CARGO_PKG_VERSION")}
            })
        );
    }
}

//! Mocktioneer bidder — snapshot E2E backend.
//!
//! **Feature-gated.** Mocktioneer points at `http://127.0.0.1` which fails
//! `validate_endpoint`'s `https://`-only + localhost-reject rule in default
//! builds. The relaxation lives behind `#[cfg(feature =
//! "insecure-bidders")]`; in default builds this module does not exist.

mod config;
mod request;
mod response;

use crate::debug_projection::DebugBodySchema;
use crate::{
    BidderSettings, ConfiguredEndpoint, OutboundRequestMeta, OutboundRequestSink,
    bidder::{Bidder, BidderError},
    request_info::BidderRequestInfo,
    types::{BidderHttpResponse, BidderResponse},
};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub struct MocktioneerBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(MocktioneerBidder)
}

pub(super) fn validate_config(
    input: crate::BidderConfigInput<'_>,
) -> Result<(), crate::BidderConfigError> {
    use validator::Validate as _;
    config::Config::from(input)
        .validate()
        .map_err(config::error)
}

impl Bidder for MocktioneerBidder {
    fn configured_endpoint<'a>(
        &'a self,
        requested: Option<&'a BidderSettings>,
        base: Option<&'a BidderSettings>,
    ) -> Result<Option<ConfiguredEndpoint<'a>>, BidderError> {
        config::endpoint(
            requested.and_then(BidderSettings::as_mocktioneer),
            base.and_then(BidderSettings::as_mocktioneer),
        )
        .map_err(|()| BidderError::failed_to_request_bids("invalid configured bidder endpoint"))?
        .map(ConfiguredEndpoint::new)
        .transpose()
    }

    fn debug_body_schema(&self) -> DebugBodySchema {
        DebugBodySchema::OpenRtb
    }

    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        sink: &mut OutboundRequestSink<'_>,
    ) -> Result<(), Vec<BidderError>> {
        request::make_requests(request, info, sink);
        Ok(())
    }

    fn make_bids(
        &self,
        request: &BidRequest,
        _sent: &OutboundRequestMeta,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        response::make_bids(request, response)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_endpoint_settings_accept_an_optional_string() {
        let settings = serde_json::from_value::<crate::BidderSettings>(serde_json::json!({
            "implementation": "mocktioneer",
            "settings": {"endpoint": "https://mock-bidder.example.test:8443/auction?route=one"}
        }));
        assert!(
            settings.is_ok(),
            "Mocktioneer must declare endpoint settings"
        );
    }

    #[test]
    fn endpoint_declaration_preserves_development_gate_and_compiled_loopback() {
        let manifest: toml::Value = toml::from_str(include_str!("bidder.toml")).unwrap();
        assert_eq!(
            manifest["settings"]["fields"]["endpoint"].as_str(),
            Some("optional_string")
        );
        assert_eq!(
            manifest["required_feature"].as_str(),
            Some("insecure-bidders")
        );
        assert_eq!(
            manifest["definitions"]["mocktioneer"]["endpoint"].as_str(),
            Some("http://127.0.0.1:8787/openrtb2/auction")
        );
        for values in [
            serde_json::json!({"unknown_endpoint":"https://example.test"}),
            serde_json::json!({"endpoint":1_i32}),
            serde_json::json!({"endpoint":false}),
            serde_json::json!({"endpoint":[]}),
            serde_json::json!({"endpoint":{}}),
        ] {
            let error = serde_json::from_value::<crate::BidderSettings>(
                serde_json::json!({"implementation":"mocktioneer", "settings":values}),
            )
            .unwrap_err();
            assert!(error.is_data());
        }
    }

    #[test]
    fn declares_the_reviewed_debug_body_schema() {
        use super::{Bidder as _, MocktioneerBidder};
        use crate::debug_projection::DebugBodySchema;

        assert_eq!(
            MocktioneerBidder.debug_body_schema(),
            DebugBodySchema::OpenRtb
        );
    }
}

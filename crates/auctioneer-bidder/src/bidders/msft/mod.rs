//! MSFT protocol candidate; public activation and selected-flow acceptance remain separate.

mod parameters;
mod request;
mod response;

use crate::{
    Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
    OutboundRequestMeta, OutboundRequestSink, debug_projection::DebugBodySchema,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

#[cfg(all(test, not(target_arch = "wasm32")))]
const CAPTURE_MANIFEST_SHA256: &str =
    "880e956ea92c9a69c915bd34d646230aacabf4f07a5ce68a12efcbf5ecf2d30a";

pub struct MsftBidder;

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(MsftBidder)
}

impl Bidder for MsftBidder {
    fn debug_body_schema(&self) -> DebugBodySchema {
        DebugBodySchema::OpenRtb
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

    mod generated_settings_conformance;

    #[cfg(not(target_arch = "wasm32"))]
    use super::CAPTURE_MANIFEST_SHA256;
    use super::MsftBidder;
    use crate::bidders::harness;
    use crate::{
        BidderName, BidderRequestInfo, EndpointRejectReason, EndpointValidator, EntryPoint,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::bounded_json::JsonFieldShape;
    #[cfg(not(target_arch = "wasm32"))]
    use serde::Deserialize;
    use serde_json::{Value, json};
    use url::Url;

    pub(super) fn info(entry: EntryPoint) -> BidderRequestInfo<'static> {
        let mut info = BidderRequestInfo::new(
            BidderName::new("msft-alias"),
            BidderName::new("msft"),
            "account",
            None,
            "https://bidder.example.test/openrtb2",
            &[],
            None,
            None,
            entry,
            &[],
        );
        info.original_request_ext_shape = JsonFieldShape::Absent;
        info
    }

    pub(super) fn request(imps: Value) -> BidRequest {
        let mut value = json!({"id":"request", "tmax":81_i32});
        value["imp"] = imps;
        serde_json::from_value(value).unwrap()
    }

    pub(super) fn banner(id: &str, params: Value) -> Value {
        let mut value = json!({"id":id,"banner":{"format":[{"w":300_i32,"h":250_i32}]},"ext":{}});
        value["ext"]["bidder"] = params;
        value
    }

    pub(super) fn body(request: &BidRequest, info: &BidderRequestInfo<'_>) -> Value {
        let calls = harness::drive_ok(&MsftBidder, request, info);
        assert_eq!(calls.len(), 1);
        serde_json::from_slice(calls[0].body()).unwrap()
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Retained {
        #[serde(rename = "source_path")]
        path: String,
        #[serde(rename = "source_bytes")]
        bytes: u64,
        #[serde(rename = "source_sha256")]
        sha256: String,
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn retained_fixture_witnesses_reject_legacy_and_mixed_keys() {
        let manifest: toml::Value = toml::from_str(include_str!("bidder.toml")).unwrap();
        let mut checked = 0_usize;
        for artifact in manifest["provenance"]["artifacts"].as_array().unwrap() {
            let Some(retained) = artifact.get("retained") else {
                continue;
            };
            let witness: Retained = retained.clone().try_into().unwrap();
            assert!(!witness.path.is_empty());
            assert!(witness.bytes > 0);
            assert_eq!(witness.sha256.len(), 64);
            for (canonical, legacy) in [
                ("source_path", "path"),
                ("source_bytes", "bytes"),
                ("source_sha256", "sha256"),
            ] {
                let mut changed = retained.as_table().unwrap().clone();
                let value = changed.remove(canonical).unwrap();
                changed.insert(legacy.into(), value.clone());
                toml::Value::Table(changed.clone())
                    .try_into::<Retained>()
                    .unwrap_err();
                changed.insert(canonical.into(), value);
                toml::Value::Table(changed)
                    .try_into::<Retained>()
                    .unwrap_err();
            }
            checked = checked.saturating_add(1);
        }
        assert_eq!(checked, 34);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn captured_fixture(path: &str, source: &str) -> Value {
        use sha2::{Digest as _, Sha256};
        let manifest_source = include_str!("bidder.toml");
        assert_eq!(
            format!("{:x}", Sha256::digest(manifest_source.as_bytes())),
            CAPTURE_MANIFEST_SHA256
        );
        let manifest: toml::Value = toml::from_str(manifest_source).unwrap();
        let entries = manifest["provenance"]["artifacts"].as_array().unwrap();
        let matching: Vec<_> = entries
            .iter()
            .filter(|artifact| {
                artifact
                    .get("retained")
                    .and_then(|retained| retained.get("source_path"))
                    .and_then(toml::Value::as_str)
                    == Some(path)
            })
            .collect();
        assert_eq!(matching.len(), 1);
        let retained: Retained = matching[0]["retained"].clone().try_into().unwrap();
        assert_eq!(retained.path, path);
        assert_eq!(retained.bytes, u64::try_from(source.len()).unwrap());
        assert_eq!(
            retained.sha256,
            format!("{:x}", Sha256::digest(source.as_bytes()))
        );
        serde_json::from_str(source).unwrap()
    }

    pub(super) struct AllowEndpoints;
    impl EndpointValidator for AllowEndpoints {
        fn validate(
            &self,
            _url: &Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }
}

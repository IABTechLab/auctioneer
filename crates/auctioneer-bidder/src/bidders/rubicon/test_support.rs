use super::{RubiconBidder, tests as conformance};
use crate::bidders::harness;
use crate::correlation::CorrelationMedia;
use crate::{
    Bidder as _, BidderHttpRequest, BidderHttpResponse, BidderName, BidderRequestInfo,
    EndpointRejectReason, EndpointValidator, EntryPoint, FloorConversion, META_BYTES_CAPACITY,
    OutboundRequestMeta, OutboundRequestMetaBuilder,
};
use auctioneer_config::BidderSettings;
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::{Value, json};
use std::cell::Cell;

pub(super) const ENDPOINT: &str = "https://bidder.example.test/a/api/exchange";

pub(super) struct EndpointPolicy(pub bool);

impl EndpointValidator for EndpointPolicy {
    fn validate(
        &self,
        _url: &url::Url,
        _info: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        if self.0 {
            Ok(())
        } else {
            Err(EndpointRejectReason::HostNotAllowed)
        }
    }
}

pub(super) struct Transactions(pub Cell<usize>);

impl EndpointValidator for Transactions {
    fn validate(
        &self,
        _: &url::Url,
        _: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        self.0.set(self.0.get().checked_add(1).unwrap());
        Ok(())
    }
}

pub(super) fn parameters() -> Value {
    json!({"accountId":1001_i64, "siteId":113_932_i64, "zoneId":535_510_i64})
}

pub(super) fn banner(id: &str) -> Value {
    json!({"id":id, "banner":{"format":[{"w":300_i32,"h":250_i32}]},
        "ext":{"bidder":parameters()}})
}

pub(super) fn mixed(id: &str) -> Value {
    let mut imp = banner(id);
    imp["video"] = json!({"mimes":["video/mp4"], "protocols":[2_i32],
        "maxduration":30_i32, "linearity":1_i32, "w":640_i32, "h":480_i32});
    imp["native"] = json!({"ver":"1.2", "request":json!({
        "eventtrackers":[], "plcmttype":1_i32, "context":1_i32, "assets":[]
    }).to_string()});
    imp["audio"] = json!({"mimes":["audio/mp4"]});
    imp
}

pub(super) fn request(imps: &Value) -> BidRequest {
    serde_json::from_value(
        json!({"id":"r", "site":{"page":"https://publisher.example/page"},
        "imp":imps}),
    )
    .unwrap()
}

pub(super) fn context() -> BidderRequestInfo<'static> {
    BidderRequestInfo::new(
        BidderName::new("fake_alias"),
        BidderName::new("rubicon"),
        "fake_account",
        None,
        ENDPOINT,
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    )
}

pub(super) fn settings(values: &Value) -> BidderSettings {
    serde_json::from_value(json!({"implementation":"rubicon","values":values})).unwrap()
}

pub(super) fn outbound(request: &BidRequest) -> Vec<BidderHttpRequest> {
    conformance::with_source_context(|info| harness::drive_ok(&RubiconBidder, request, info))
}

pub(super) fn body(call: &BidderHttpRequest) -> Value {
    serde_json::from_slice(call.body()).unwrap()
}

pub(super) fn sent(media: Option<CorrelationMedia>, ids: &[&str]) -> OutboundRequestMeta {
    let mut bytes = Vec::with_capacity(META_BYTES_CAPACITY);
    let mut builder =
        OutboundRequestMetaBuilder::new(&mut bytes, u16::try_from(ids.len()).unwrap());
    builder.set_method("POST").unwrap();
    builder.set_url(ENDPOINT).unwrap();
    for (index, id) in ids.iter().enumerate() {
        builder.imp_id(id).unwrap();
        if let Some(selected) = media {
            builder
                .correlate_imp(u16::try_from(index).unwrap(), selected, 0)
                .unwrap();
        }
    }
    builder.finish()
}

pub(super) fn response(status: u16, value: &Value) -> BidderHttpResponse {
    harness::budgeted_response(status, serde_json::to_vec(value).unwrap())
}

pub(super) fn bid(fields: Value) -> Value {
    let Value::Object(attributes) = fields else {
        panic!("object bid fields")
    };
    let mut bid = json!({"id":"bid", "impid":"i", "price":1.25_f64, "adm":"creative"});
    bid.as_object_mut().unwrap().extend(attributes);
    bid
}

pub(super) fn envelope(bids: &Value) -> Value {
    json!({"id":"r", "cur":"USD", "seatbid":[{"bid":bids}]})
}

pub(super) fn decode(
    value: &Value,
    media: CorrelationMedia,
) -> Result<crate::BidderResponse, Vec<crate::BidderError>> {
    RubiconBidder.make_bids(
        &request(&json!([mixed("i")])),
        &sent(Some(media), &["i"]),
        &response(200, value),
    )
}

pub(super) fn assert_credential_eligibility(
    request: &BidRequest,
    conversion: FloorConversion<'_>,
    eligible: bool,
) {
    let settings = settings(&json!({
        "credentials_secret":"fake_credential_bundle","region":"us-east"
    }));
    let mut info = context();
    info.set_settings(Some("rubicon"), Some(&settings), None)
        .unwrap();
    info.floor_currency = conversion;
    assert!(info.credentials.is_none());
    let before = serde_json::to_value(request).unwrap();
    let result = RubiconBidder.credential_key(request, &info);
    if eligible {
        let key = result.unwrap().unwrap();
        let expected = settings
            .as_rubicon()
            .unwrap()
            .credentials_secret
            .as_deref()
            .unwrap();
        assert_eq!(key, "fake_credential_bundle");
        assert_eq!(key.as_ptr(), expected.as_ptr());
    } else {
        let Err(error) = result else {
            panic!("expected no eligible impression for {before}");
        };
        assert_eq!(error.code, 2);
        assert_eq!(error.message, "rubicon no eligible impression");
    }
    assert_eq!(serde_json::to_value(request).unwrap(), before);
    assert!(info.credentials.is_none());
}

pub(super) fn native_impression(version: &str, payload: &Value) -> Value {
    json!({"id":"native","native":{"ver":version,"request":payload.to_string()},
        "ext":{"bidder":parameters()}})
}

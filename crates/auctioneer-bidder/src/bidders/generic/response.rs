use crate::bidders::media_type::{classify_bid, classify_response, legacy_media_type};
use crate::{BidderError, BidderHttpResponse, BidderResponse};
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};

pub(super) fn make_bids(
    request: &BidRequest,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if response.status() >= 400 {
        return Err(vec![BidderError::bad_server_response(format!(
            "status={}",
            response.status()
        ))]);
    }
    // Some bidders return 204 No Content for "no bids" — emit empty.
    if response.status() == 204 || response.body_is_empty() {
        return Ok(BidderResponse {
            bids: vec![],
            currency: "USD".into(),
            errors: vec![],
            auction_configs: Vec::new(),
        });
    }
    // The response body is sealed: this is the only accessor that
    // reads it, and it parses within the node budget the exchange
    // attached. There is no `&[u8]` to hand to `serde_json` here.
    let parsed: BidResponse = response.parse_bid_response()?;
    classify_response(parsed, |bid| {
        classify_bid(request, bid, legacy_media_type(bid)?)
    })
}

#[cfg(test)]
mod tests {
    use super::super::GenericBidder;
    use crate::Bidder as _;
    use crate::bidders::harness;
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;

    #[test]
    fn make_bids_honors_standard_media_type() {
        let response = harness::decode_bid(
            &GenericBidder,
            serde_json::json!([{"id": "i", "banner": {}, "video": {}}]),
            serde_json::json!({"mtype": 2_u32}),
        )
        .unwrap();
        assert_eq!(response.bids[0].media_type, MediaType::Video);
    }

    #[test]
    fn make_bids_classifies_and_partitions_media_across_seats() {
        let response = harness::decode_bids(
            &GenericBidder,
            serde_json::json!([{"id": "i", "banner": {}, "video": {}, "audio": {}, "native": {}}]),
            serde_json::json!([
                {"seat": "first", "bid": [{"id": "bad", "mtype": 0_u32}, {"id": "video", "mtype": 2_u32}]},
                {"seat": "second", "bid": [{"id": "audio", "mtype": 3_u32}, {"id": "bad-conflict", "mtype": 1_u32, "ext": {"video": {}}}]},
            ]),
        )
        .unwrap();
        assert_eq!(response.bids.len(), 2);
        assert_eq!(response.bids[0].bid.id, "video");
        assert_eq!(response.bids[0].media_type, MediaType::Video);
        assert_eq!(response.bids[1].bid.id, "audio");
        assert_eq!(response.bids[1].media_type, MediaType::Audio);
        harness::assert_bad_server_response_errors(&response.errors, 2);
    }

    #[test]
    fn make_bids_accepts_legacy_media_hints() {
        for fields in [
            serde_json::json!({"ext": {"prebid": {"mediatype": "video"}}}),
            serde_json::json!({"ext": {"video": {}}}),
        ] {
            let response = harness::decode_bid(
                &GenericBidder,
                serde_json::json!([{"id": "i", "banner": {}, "video": {}}]),
                fields,
            )
            .unwrap();
            assert_eq!(response.bids[0].media_type, MediaType::Video);
            assert!(response.errors.is_empty());
        }
    }

    #[test]
    fn make_bids_rejects_invalid_or_conflicting_legacy_media_hints() {
        for fields in [
            serde_json::json!({"ext": {"prebid": {"mediatype": "invalid"}}}),
            serde_json::json!({"ext": {"prebid": {"mediatype": 2_u32}}}),
            serde_json::json!({"ext": {"video": {}, "native": {}}}),
            serde_json::json!({"ext": {"prebid": {"type": "banner", "mediatype": "video"}}}),
        ] {
            let errors = harness::decode_bid(
                &GenericBidder,
                serde_json::json!([{"id": "i", "banner": {}, "video": {}, "native": {}}]),
                fields,
            )
            .unwrap_err();
            harness::assert_bad_server_response_errors(&errors, 1);
        }
    }

    #[test]
    fn all_rejected_bids_return_every_error_without_silent_no_bid() {
        let errors = harness::decode_bids(
            &GenericBidder,
            serde_json::json!([{"id": "i", "banner": {}}]),
            serde_json::json!([
                {"seat": "first", "bid": [{"mtype": 0_u32}, {"mtype": 2_u32}]},
                {"seat": "second", "bid": [{"mtype": 3_u32}]},
            ]),
        )
        .unwrap_err();
        harness::assert_bad_server_response_errors(&errors, 3);
    }

    #[test]
    fn make_bids_returns_empty_on_204() {
        let req = BidRequest::default();
        let sent = harness::sent_meta("https://x");
        let resp = harness::budgeted_response(204, vec![]);
        let out = GenericBidder.make_bids(&req, &sent, &resp).unwrap();
        assert!(out.bids.is_empty());
    }

    #[test]
    fn make_bids_status_500_emits_bad_server_response() {
        let req = BidRequest::default();
        let sent = harness::sent_meta("https://x");
        let resp = harness::budgeted_response(500, b"oops".to_vec());
        let errs = GenericBidder.make_bids(&req, &sent, &resp).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, 4);
    }

    #[test]
    fn make_bids_parses_seatbid_into_typed_bids() {
        let req: BidRequest = serde_json::from_value(serde_json::json!({
            "id": "request", "imp": [{"id": "i-1", "banner": {}}],
        }))
        .unwrap();
        let sent = harness::sent_meta("https://x");
        // Byte-string fixture: a `&[u8]` wire body. `serde_json::json!`
        // yields a `Value`, so converting would mean `.to_string().into_bytes()`
        // at the call site for no gain.
        let json = br#"{"id":"r-1","seatbid":[{"seat":"ax","bid":[{"id":"b-1","impid":"i-1","price":1.5}]}],"cur":"USD"}"#;
        let resp = harness::budgeted_response(200, json.to_vec());
        let out = GenericBidder.make_bids(&req, &sent, &resp).unwrap();
        assert_eq!(out.bids.len(), 1);
        assert_eq!(out.bids[0].bid.impid, "i-1");
        assert_eq!(out.currency, "USD");
    }
}

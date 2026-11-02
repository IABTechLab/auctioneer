use crate::bidder::BidderError;
use crate::bidders::{
    media_type::{classify_bid, classify_response},
    status::standard_status,
};
use crate::meta::OutboundRequestMeta;
use crate::types::{BidderHttpResponse, BidderResponse};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};

pub(super) fn make_bids(
    request: &BidRequest,
    _sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        if status == 400 {
            BidderError::bad_input("Unexpected status code: 400. Bad request from publisher.")
        } else {
            BidderError::bad_server_response(format!("Unexpected status code: {status}."))
        }
    })? {
        return Ok(no_bid);
    }
    let parsed: BidResponse = response.parse_bid_response()?;
    classify_response(parsed, |bid| {
        // Enforce APS's explicit restriction BEFORE the shared classifier
        // can infer media from a single-format impression or legacy hint.
        let media = match bid.other.get("mtype").and_then(serde_json::Value::as_u64) {
            Some(1) => MediaType::Banner,
            Some(2) => MediaType::Video,
            _ => {
                return Err(BidderError::bad_server_response(
                    "aps: bid.mtype must explicitly be 1 or 2",
                ));
            }
        };
        classify_bid(request, bid, Some(media))
    })
}

#[cfg(test)]
mod tests {
    use super::super::ApsBidder;
    use super::super::tests::{NA, minimal, response};
    use crate::bidder::{Bidder as _, BidderErrorKind};
    use crate::meta::{META_BYTES_CAPACITY, OutboundRequestMeta, OutboundRequestMetaBuilder};
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::Video;
    use serde_json::json;

    fn sent() -> OutboundRequestMeta {
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 16);
        meta.set_method("POST").unwrap();
        meta.set_url(NA).unwrap();
        meta.imp_id("i").unwrap();
        meta.finish()
    }

    #[test]
    fn response_status_policy_is_exact_and_204_does_not_parse_a_body() {
        let out = ApsBidder
            .make_bids(&minimal(), &sent(), &response(204, b"not json".to_vec()))
            .unwrap();
        assert!(out.bids.is_empty());
        assert!(out.errors.is_empty());
        assert_eq!(out.currency, "USD");
        for status in [201, 202, 301, 400, 403, 429, 500, 503] {
            let errors = ApsBidder
                .make_bids(&minimal(), &sent(), &response(status, Vec::new()))
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
        }
    }

    #[test]
    fn successful_responses_still_require_valid_sealed_json_and_node_budget() {
        let mut trailing = serde_json::to_vec(&json!({"id":"r"})).unwrap();
        trailing.extend_from_slice(b" trailing");
        for bytes in [
            b"".as_slice(),
            b"not json",
            b"null",
            b"[]",
            trailing.as_slice(),
        ] {
            let errors = ApsBidder
                .make_bids(&minimal(), &sent(), &response(200, bytes.to_vec()))
                .unwrap_err();
            assert!(
                errors
                    .iter()
                    .all(|error| error.kind == BidderErrorKind::ResponseParse)
            );
        }
        let response = response(
            200,
            serde_json::to_vec(&json!({"id":"r","cur":"EUR","seatbid":[]})).unwrap(),
        )
        .within_node_budget(1);
        let errors = ApsBidder
            .make_bids(&minimal(), &sent(), &response)
            .unwrap_err();
        assert!(
            errors
                .iter()
                .all(|error| error.kind == BidderErrorKind::ResponseParse)
        );
        let out = ApsBidder
            .make_bids(
                &minimal(),
                &sent(),
                &self::response(200, serde_json::to_vec(&json!({"id":"r"})).unwrap()),
            )
            .unwrap();
        assert_eq!(out.currency, "USD");
        assert!(out.bids.is_empty());
    }

    #[test]
    fn missing_zero_or_unsupported_mtype_never_uses_shared_inference() {
        for fields in [
            json!({}),
            json!({"mtype": 0_i32}),
            json!({"mtype": 3_i32}),
            json!({"mtype": 4_i32}),
            json!({"mtype": 5_i32}),
            json!({"mtype": -1_i32}),
            json!({"mtype": "1"}),
            json!({"mtype": 1.0_f64}),
            json!({"mtype": null}),
            json!({"mtype": false}),
            json!({"ext": {"prebid": {"type": "banner"}}}),
            json!({"ext": {"video": {}}}),
        ] {
            let mut bid = json!({"id": "b", "impid": "i", "price": 1.5_f64});
            bid.as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            let bytes =
                serde_json::to_vec(&json!({"id": "r", "seatbid": [{"seat": "aps", "bid": [bid]}]}))
                    .unwrap();
            let errors = ApsBidder
                .make_bids(&minimal(), &sent(), &response(200, bytes))
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
    }

    #[test]
    fn multiseat_classification_preserves_valid_siblings_order_and_currency() {
        let mut request = minimal();
        request.imp[0].video = Some(Video::default());
        let bytes = serde_json::to_vec(&json!({"id": "r", "cur": "EUR", "seatbid": [
            {"seat": "one", "bid": [
                {"id": "bad-missing", "impid": "i", "price": 1.5_f64},
                {"id": "video", "impid": "i", "price": 1.5_f64, "mtype": 2_i32},
                {"id": "bad-zero", "impid": "i", "price": 1.5_f64, "mtype": 0_i32}]},
            {"seat": "two", "bid": [
                {"id": "banner", "impid": "i", "price": 1.5_f64, "mtype": 1_i32},
                {"id": "bad-audio", "impid": "i", "price": 1.5_f64, "mtype": 3_i32},
                {"id": "bad-unknown-imp", "impid": "unknown", "price": 1.5_f64, "mtype": 1_i32},
                {"id": "bad-conflict", "impid": "i", "price": 1.5_f64, "mtype": 1_i32, "ext": {"prebid": {"type": "video"}}}]}]
        })).unwrap();
        let out = ApsBidder
            .make_bids(&request, &sent(), &response(200, bytes))
            .unwrap();
        assert_eq!(out.currency, "EUR");
        assert_eq!(
            out.bids
                .iter()
                .map(|bid| (bid.bid.id.as_str(), bid.media_type))
                .collect::<Vec<_>>(),
            [("video", MediaType::Video), ("banner", MediaType::Banner)]
        );
        assert_eq!(out.errors.len(), 5);
        assert!(out.errors.iter().all(|error| error.code == 4));
    }

    #[test]
    fn explicit_media_type_must_also_be_offered_by_the_matching_impression() {
        let bytes = serde_json::to_vec(&json!({"id":"r","cur":"USD","seatbid":[{"seat":"aps","bid":[{"id":"b","impid":"i","price":1.5_f64,"mtype":2_u32}]}]})).unwrap();
        let errors = ApsBidder
            .make_bids(&minimal(), &sent(), &response(200, bytes))
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
    }
}

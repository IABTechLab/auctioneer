use crate::bidders::media_type::{classify_bid, classify_response};
use crate::{BidderError, BidderHttpResponse, BidderResponse};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::{
    bid_request::BidRequest,
    bid_response::{Bid, BidResponse},
};

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
    if response.body_is_empty() {
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
        let hint = appnexus_media_type(bid)?;
        let media_type = classify_bid(request, bid, hint)?;
        if media_type == MediaType::Audio {
            return Err(BidderError::bad_server_response(
                "appnexus does not support audio bids",
            ));
        }
        Ok(media_type)
    })
}

fn appnexus_media_type(bid: &Bid) -> Result<Option<MediaType>, BidderError> {
    let Some(namespace) = bid.ext.as_ref().and_then(|ext| ext.other.get("appnexus")) else {
        return Ok(None);
    };
    let fields = namespace
        .as_object()
        .ok_or_else(|| BidderError::bad_server_response("bid.ext.appnexus must be an object"))?;
    let Some(ad_type) = fields.get("bid_ad_type") else {
        return Ok(None);
    };
    // Pinned prebid-server d7f8515, appnexus.go::getMediaTypeForBid.
    // These are not OpenRTB mtype codes: 1 means video, not banner.
    let media_type = match ad_type.as_u64() {
        Some(0) => MediaType::Banner,
        Some(1) => MediaType::Video,
        Some(3) => MediaType::Native,
        _ => {
            return Err(BidderError::bad_server_response(
                "bid.ext.appnexus.bid_ad_type must be 0, 1, or 3",
            ));
        }
    };
    Ok(Some(media_type))
}

#[cfg(test)]
mod tests {
    use super::super::AppnexusBidder;
    use crate::bidders::harness;
    use auctioneer_config_data::types::MediaType;

    #[test]
    fn all_rejected_bids_return_every_error_without_silent_no_bid() {
        let errors = harness::decode_bids(
            &AppnexusBidder,
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
    fn make_bids_preserves_valid_siblings_across_seats() {
        for rejected_first in [false, true] {
            let good = serde_json::json!({"id": "good", "price": 9.5_f64, "mtype": 1_u32});
            let bad = serde_json::json!({"id": "bad", "mtype": 0_u32});
            let bids = if rejected_first {
                vec![bad, good]
            } else {
                vec![good, bad]
            };
            let response = harness::decode_bids(
                &AppnexusBidder,
                serde_json::json!([{"id": "i", "banner": {}, "video": {}}]),
                serde_json::json!([
                    {"seat": "first", "bid": bids},
                    {"seat": "second", "bid": [{"id": "second-good", "mtype": 2_u32}, {"id": "second-bad", "mtype": 3_u32}]},
                ]),
            )
            .expect("one rejected bid must not destroy valid siblings");
            assert_eq!(response.bids.len(), 2);
            assert_eq!(response.bids[0].bid.id, "good");
            assert_eq!(response.bids[0].bid.price.to_bits(), 9.5_f64.to_bits());
            assert_eq!(response.bids[1].bid.id, "second-good");
            assert_eq!(response.currency, "USD");
            assert_eq!(response.errors.len(), 2);
            assert!(response.errors.iter().all(|error| error.code == 4));
        }
    }

    #[test]
    fn make_bids_classifies_openrtb_mtype_in_multiformat_impressions() {
        for (mtype, expected) in [
            (1_u32, MediaType::Banner),
            (2_u32, MediaType::Video),
            (4_u32, MediaType::Native),
        ] {
            let response = harness::decode_bid(
                &AppnexusBidder,
                serde_json::json!([{"id": "i", "banner": {}, "video": {}, "audio": {}, "native": {}}]),
                serde_json::json!({"mtype": mtype, "unknown_bid": {"keep": [true, null]}}),
            )
            .unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].media_type, expected);
            assert_eq!(response.bids[0].bid.other["mtype"], mtype);
            assert_eq!(
                response.bids[0].bid.other["unknown_bid"],
                serde_json::json!({"keep": [true, null]}),
            );
            assert_eq!(response.currency, "USD");
        }
    }

    #[test]
    fn make_bids_rejects_audio_even_if_the_original_impression_offers_it() {
        for fields in [
            serde_json::json!({"mtype": 3_u32}),
            serde_json::json!({"ext": {"prebid": {"type": "audio"}}}),
            serde_json::json!({}),
        ] {
            let errors = harness::decode_bid(
                &AppnexusBidder,
                serde_json::json!([{"id": "i", "audio": {}}]),
                fields,
            )
            .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
    }

    #[test]
    fn make_bids_uses_appnexus_bid_ad_type_codes() {
        for (ad_type, expected) in [
            (0_u32, MediaType::Banner),
            (1_u32, MediaType::Video),
            (3_u32, MediaType::Native),
        ] {
            let response = harness::decode_bid(
                &AppnexusBidder,
                serde_json::json!([{"id": "i", "banner": {}, "video": {}, "native": {}}]),
                serde_json::json!({"ext": {"appnexus": {"bid_ad_type": ad_type, "keep": {"nested": true}}}}),
            )
            .unwrap();
            assert_eq!(response.bids[0].media_type, expected);
            assert_eq!(
                response.bids[0].bid.ext.as_ref().unwrap().other["appnexus"]["keep"],
                serde_json::json!({"nested": true}),
            );
        }
    }

    #[test]
    fn make_bids_infers_only_the_matching_single_format_impression() {
        for (impression, expected) in [
            (
                serde_json::json!({"id": "i", "banner": {}}),
                MediaType::Banner,
            ),
            (
                serde_json::json!({"id": "i", "video": {}}),
                MediaType::Video,
            ),
            (
                serde_json::json!({"id": "i", "native": {}}),
                MediaType::Native,
            ),
        ] {
            let response = harness::decode_bid(
                &AppnexusBidder,
                serde_json::json!([{"id": "other", "audio": {}}, impression]),
                serde_json::json!({}),
            )
            .unwrap();
            assert_eq!(response.bids[0].media_type, expected);
        }
    }

    #[test]
    fn make_bids_honors_prebid_type_for_multiformat_impressions() {
        for (name, expected) in [("video", MediaType::Video), ("native", MediaType::Native)] {
            let response = harness::decode_bid(
                &AppnexusBidder,
                serde_json::json!([{"id": "i", "banner": {}, "video": {}, "native": {}}]),
                serde_json::json!({"ext": {"prebid": {"type": name}}}),
            )
            .unwrap();
            assert_eq!(response.bids[0].media_type, expected);
        }
    }

    #[test]
    fn make_bids_accepts_consistent_media_hints() {
        let response = harness::decode_bid(
            &AppnexusBidder,
            serde_json::json!([{"id": "i", "banner": {}, "video": {}}]),
            serde_json::json!({"mtype": 2_u32, "ext": {"prebid": {"type": "video"}, "appnexus": {"bid_ad_type": 1_u32}}}),
        )
        .unwrap();
        assert_eq!(response.bids[0].media_type, MediaType::Video);
    }

    #[test]
    fn make_bids_does_not_treat_impression_mtype_as_a_response_hint() {
        let response = harness::decode_bid(
            &AppnexusBidder,
            serde_json::json!([{"id": "i", "video": {}, "mtype": 1_u32}]),
            serde_json::json!({}),
        )
        .unwrap();
        assert_eq!(response.bids[0].media_type, MediaType::Video);
        let errors = harness::decode_bid(
            &AppnexusBidder,
            serde_json::json!([{"id": "i", "banner": {}, "video": {}, "mtype": 2_u32}]),
            serde_json::json!({}),
        )
        .unwrap_err();
        assert_eq!(errors[0].code, 4);
    }

    #[test]
    fn make_bids_rejects_ambiguous_or_unrequested_media() {
        for (impressions, fields) in [
            (
                serde_json::json!([{"id": "i", "banner": {}, "video": {}}]),
                serde_json::json!({}),
            ),
            (
                serde_json::json!([{"id": "i", "video": {}}]),
                serde_json::json!({"mtype": 1_u32}),
            ),
            (
                serde_json::json!([{"id": "i", "native": {}}]),
                serde_json::json!({"ext": {"appnexus": {"bid_ad_type": 1_u32}}}),
            ),
            (
                serde_json::json!([{"id": "i", "banner": {}}]),
                serde_json::json!({"impid": "unknown", "mtype": 1_u32}),
            ),
            (serde_json::json!([{"id": "i"}]), serde_json::json!({})),
        ] {
            let errors = harness::decode_bid(&AppnexusBidder, impressions, fields).unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
    }

    #[test]
    fn make_bids_rejects_invalid_or_conflicting_media_hints() {
        for fields in [
            serde_json::json!({"mtype": 0_u32}),
            serde_json::json!({"mtype": 5_u32}),
            serde_json::json!({"mtype": -1_i32}),
            serde_json::json!({"mtype": "2"}),
            serde_json::json!({"mtype": 2.5_f64}),
            serde_json::json!({"mtype": null}),
            serde_json::json!({"ext": {"appnexus": {"bid_ad_type": 2_u32}}}),
            serde_json::json!({"ext": {"appnexus": {"bid_ad_type": "1"}}}),
            serde_json::json!({"ext": {"appnexus": {"bid_ad_type": 1.5_f64}}}),
            serde_json::json!({"ext": {"appnexus": "invalid"}}),
            serde_json::json!({"ext": {"prebid": {"type": "unknown"}}}),
            serde_json::json!({"mtype": 2_u32, "ext": {"appnexus": {"bid_ad_type": 0_u32}}}),
            serde_json::json!({"mtype": 2_u32, "ext": {"prebid": {"type": "native"}}}),
        ] {
            let errors = harness::decode_bid(
                &AppnexusBidder,
                serde_json::json!([{"id": "i", "banner": {}, "video": {}, "audio": {}, "native": {}}]),
                fields,
            )
            .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
    }
}

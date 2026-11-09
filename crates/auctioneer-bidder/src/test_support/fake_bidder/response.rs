//! Sealed response decoding and fault-injection coverage for the shared fake.

use super::{FakeBidder, ResponseCoverage};
use crate::{
    BidderError, BidderHttpResponse, BidderResponse, BrowserAuctionConfig, OutboundRequestMeta,
    TypedBid,
};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
use serde_json::Value;
use std::sync::Arc;

pub(super) fn make_bids(
    bidder: &FakeBidder<'_>,
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if response.status() >= 400 {
        return Err(vec![BidderError::bad_server_response("fake HTTP refusal")]);
    }
    if response.status() == 204 || response.body_is_empty() {
        return Ok(BidderResponse {
            bids: Vec::new(),
            currency: "USD".into(),
            errors: Vec::new(),
            auction_configs: Vec::new(),
        });
    }
    let mut parsed = response.parse_bid_response()?;
    let mut errors = Vec::new();
    let mut auction_configs = Vec::new();
    if let Some(Value::Object(ext)) = &mut parsed.ext
        && let Some(source_configs) = ext.remove("auctionconfigs")
    {
        if let Value::Array(entries) = source_configs {
            for entry in entries {
                match browser_config(entry) {
                    Ok(config) => auction_configs.push(config),
                    Err(error) => errors.push(error),
                }
            }
        } else {
            errors.push(BidderError::bad_server_response(
                "fake invalid configurations",
            ));
        }
    }
    let mut bids = Vec::new();
    for (bid, source_seat) in parsed.seatbid.into_iter().flat_map(|seat| {
        let source_seat = (!seat.seat.is_empty()).then(|| Arc::<str>::from(seat.seat));
        seat.bid
            .into_iter()
            .map(move |bid| (bid, source_seat.clone()))
    }) {
        match classify(request, sent, &bid, bidder.response_coverage) {
            Ok(media_type) => bids.push(TypedBid {
                bid,
                media_type,
                source_seat,
            }),
            Err(error) => errors.push(error),
        }
    }
    if bids.is_empty() && auction_configs.is_empty() && !errors.is_empty() {
        return Err(errors);
    }
    Ok(BidderResponse {
        bids,
        currency: if parsed.cur.is_empty() {
            "USD".into()
        } else {
            parsed.cur
        },
        errors,
        auction_configs,
    })
}

fn browser_config(entry: Value) -> Result<BrowserAuctionConfig, BidderError> {
    let invalid = || BidderError::bad_server_response("fake invalid configuration");
    let Value::Object(mut fields) = entry else {
        return Err(invalid());
    };
    let Some(Value::String(imp_id)) = fields.remove("impid") else {
        return Err(invalid());
    };
    let config = fields.remove("config").ok_or_else(invalid)?;
    Ok(BrowserAuctionConfig { imp_id, config })
}

fn classify(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    bid: &Bid,
    coverage: ResponseCoverage,
) -> Result<MediaType, BidderError> {
    let invalid = || BidderError::bad_server_response("fake invalid bid media or coverage");
    let imp = request
        .imp
        .iter()
        .find(|imp| imp.id == bid.impid)
        .ok_or_else(invalid)?;
    // An empty sent fixture is useful for parser-only tests; nonempty metadata
    // must identify the exact call. The exchange separately rejects empty coverage.
    if coverage == ResponseCoverage::SentCall
        && sent.imp_id_count() > 0
        && !sent.covered_imp_ids().any(|id| id == bid.impid)
    {
        return Err(invalid());
    }
    let offered = [
        (1_u64, MediaType::Banner, imp.banner.is_some()),
        (2, MediaType::Video, imp.video.is_some()),
        (3, MediaType::Audio, imp.audio.is_some()),
        (4, MediaType::Native, imp.native.is_some()),
    ];
    if let Some(value) = bid.other.get("mtype") {
        offered
            .into_iter()
            .find_map(|(code, media, present)| {
                (present && value.as_u64() == Some(code)).then_some(media)
            })
            .ok_or_else(invalid)
    } else {
        let mut candidates = offered
            .into_iter()
            .filter_map(|(_, media, present)| present.then_some(media));
        match (candidates.next(), candidates.next()) {
            (Some(media), None) => Ok(media),
            _ => Err(invalid()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BidderError, browser_config};
    use serde_json::json;

    #[test]
    fn browser_config_preserves_opaque_values_without_validating_the_exchange_owned_shape() {
        for expected in [
            json!(null),
            json!({"seller": "https://seller.example.test"}),
            json!([1_i32]),
        ] {
            let config = browser_config(json!({
                "impid": "fake_imp", "config": expected.clone(), "unrelated": "ignored"
            }))
            .unwrap();
            assert_eq!(config.imp_id, "fake_imp");
            assert_eq!(config.config, expected);
        }
    }

    #[test]
    fn malformed_browser_config_envelopes_have_one_fixed_error() {
        for input in [
            json!(null),
            json!(false),
            json!(0_i32),
            json!("value"),
            json!([]),
            json!({}),
            json!({"config": {}}),
            json!({"impid": 1_i32, "config": {}}),
            json!({"impid": "fake_imp"}),
        ] {
            assert_eq!(
                browser_config(input).unwrap_err(),
                BidderError::bad_server_response("fake invalid configuration")
            );
        }
    }
}

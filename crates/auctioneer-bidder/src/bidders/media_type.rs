//! Response media types must identify one format offered by the matching impression.

use crate::types::{BidderResponse, TypedBid};
use crate::{BidderError, OutboundRequestMeta};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::{
    bid_request::BidRequest,
    bid_response::{Bid, BidResponse, SeatBid},
};
use std::sync::Arc;

/// Capture each seat's provenance before flattening, without per-bid text copies.
pub(super) fn source_seat_bids(seat: SeatBid) -> impl Iterator<Item = (Bid, Option<Arc<str>>)> {
    let source_seat = (!seat.seat.is_empty()).then(|| Arc::<str>::from(seat.seat));
    seat.bid
        .into_iter()
        .map(move |bid| (bid, source_seat.clone()))
}

/// Resolve only media explicitly recorded for the impression in this exact call.
pub(super) fn sent_media_type(
    sent: &OutboundRequestMeta,
    imp_id: &str,
) -> Result<MediaType, BidderError> {
    let ordinal = sent
        .covered_imp_ids()
        .position(|covered| covered == imp_id)
        .and_then(|index| u16::try_from(index).ok())
        .ok_or_else(|| {
            BidderError::bad_server_response("bid response references an unsent impression")
        })?;
    sent.imp_correlation(ordinal)
        .map(|row| row.media().into())
        .ok_or_else(|| BidderError::bad_server_response("bid response lacks call-local media"))
}

/// Move each parsed bid once; a bid-local failure cannot discard valid siblings.
pub(super) fn classify_response<F>(
    response: BidResponse,
    classify: F,
) -> Result<BidderResponse, Vec<BidderError>>
where
    F: FnMut(&Bid) -> Result<MediaType, BidderError>,
{
    classify_response_with_errors(response, Vec::new(), classify)
}

/// Candidate parse errors participate in the same all-invalid verdict as
/// semantic errors; successful siblings must retain both classes of failure.
pub(super) fn classify_response_with_errors<F>(
    response: BidResponse,
    mut errors: Vec<BidderError>,
    mut classify: F,
) -> Result<BidderResponse, Vec<BidderError>>
where
    F: FnMut(&Bid) -> Result<MediaType, BidderError>,
{
    // Exact capacity avoids transient reallocations while the parsed bid
    // arrays are still live. Their total is already bounded by the node gate.
    let bid_count = response.seatbid.iter().map(|seat| seat.bid.len()).sum();
    let mut bids = Vec::with_capacity(bid_count);
    for (bid, source_seat) in response.seatbid.into_iter().flat_map(source_seat_bids) {
        match classify(&bid) {
            Ok(media_type) => bids.push(TypedBid {
                bid,
                media_type,
                source_seat,
            }),
            Err(error) => errors.push(error),
        }
    }
    if bids.is_empty() && !errors.is_empty() {
        return Err(errors);
    }
    let currency = if response.cur.is_empty() {
        "USD".into()
    } else {
        response.cur
    };
    Ok(BidderResponse {
        bids,
        currency,
        errors,
        auction_configs: Vec::new(),
    })
}

/// Resolve reviewed legacy response media type hints.
pub(super) fn legacy_media_type(bid: &Bid) -> Result<Option<MediaType>, BidderError> {
    let Some(ext) = bid.ext.as_ref() else {
        return Ok(None);
    };
    let mut declared = ext
        .prebid
        .as_ref()
        .and_then(|prebid| prebid.other.get("mediatype"))
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| {
                    BidderError::bad_server_response("bid.ext.prebid.mediatype must be a string")
                })
                .and_then(prebid_media_type)
        })
        .transpose()?;
    for (key, media_type) in [
        ("video", MediaType::Video),
        ("native", MediaType::Native),
        ("audio", MediaType::Audio),
    ] {
        if ext.other.contains_key(key) {
            if declared.is_some_and(|hint| hint != media_type) {
                return Err(BidderError::bad_server_response(
                    "bid response contains conflicting legacy media type hints",
                ));
            }
            declared = Some(media_type);
        }
    }
    Ok(declared)
}

pub(super) fn classify_bid(
    request: &BidRequest,
    bid: &Bid,
    bidder_hint: Option<MediaType>,
) -> Result<MediaType, BidderError> {
    let standard_hint = bid.other.get("mtype").map(openrtb_media_type).transpose()?;
    let prebid_hint = bid
        .ext
        .as_ref()
        .and_then(|ext| ext.prebid.as_ref())
        .and_then(|prebid| prebid.bid_type.as_deref())
        .map(prebid_media_type)
        .transpose()?;
    let mut hints = [standard_hint, prebid_hint, bidder_hint]
        .into_iter()
        .flatten();
    let declared = hints.next();
    if hints.any(|hint| Some(hint) != declared) {
        return Err(BidderError::bad_server_response(
            "bid response contains conflicting media type hints",
        ));
    }
    let imp = request
        .imp
        .iter()
        .find(|imp| imp.id == bid.impid)
        .ok_or_else(|| {
            BidderError::bad_server_response("bid response references an unknown impression")
        })?;
    let mut offered = [
        (MediaType::Banner, imp.banner.is_some()),
        (MediaType::Video, imp.video.is_some()),
        (MediaType::Audio, imp.audio.is_some()),
        (MediaType::Native, imp.native.is_some()),
    ]
    .into_iter()
    .filter_map(|(media_type, present)| present.then_some(media_type));
    match declared {
        Some(media_type) if offered.any(|candidate| candidate == media_type) => Ok(media_type),
        Some(_) => Err(BidderError::bad_server_response(
            "bid response media type was not offered by its impression",
        )),
        // Only a single-format impression makes a missing response hint unambiguous.
        None => match (offered.next(), offered.next()) {
            (Some(media_type), None) => Ok(media_type),
            _ => Err(BidderError::bad_server_response(
                "bid response lacks an unambiguous media type",
            )),
        },
    }
}

fn openrtb_media_type(value: &serde_json::Value) -> Result<MediaType, BidderError> {
    match value.as_u64() {
        Some(1) => Ok(MediaType::Banner),
        Some(2) => Ok(MediaType::Video),
        Some(3) => Ok(MediaType::Audio),
        Some(4) => Ok(MediaType::Native),
        _ => Err(BidderError::bad_server_response(
            "bid.mtype must be an integer from 1 through 4",
        )),
    }
}

fn prebid_media_type(name: &str) -> Result<MediaType, BidderError> {
    match name {
        "banner" => Ok(MediaType::Banner),
        "video" => Ok(MediaType::Video),
        "audio" => Ok(MediaType::Audio),
        "native" => Ok(MediaType::Native),
        _ => Err(BidderError::bad_server_response(
            "bid.ext.prebid.type is not a supported media type",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidders::harness;
    use crate::correlation::CorrelationMedia;
    use crate::{META_BYTES_CAPACITY, OutboundRequestMetaBuilder};
    use auctioneer_openrtb::types::bid_response::SeatBid;
    use serde_json::json;
    use std::sync::Arc;

    fn source_seat_response(seats: &serde_json::Value) -> BidderResponse {
        let request = request(json!([{"id":"i", "banner":{}}]));
        let parsed = serde_json::from_value(json!({"id":"r", "seatbid":seats})).unwrap();
        classify_response(parsed, |bid| classify_bid(&request, bid, None)).unwrap()
    }

    #[test]
    fn source_seat_is_absent_for_missing_or_empty_seats() {
        let decoded = source_seat_response(&json!([
            {"bid":[{"id":"missing", "impid":"i", "price":1_i32}]},
            {"seat":"", "bid":[{"id":"empty", "impid":"i", "price":1_i32}]}
        ]));
        assert_eq!(decoded.bids.len(), 2);
        assert!(decoded.bids.iter().all(|bid| bid.source_seat.is_none()));
    }

    #[test]
    fn source_seat_keeps_two_upstream_seats_distinct_from_requested_alias() {
        let requested = crate::BidderName::new("fake_alias");
        let seats = json!([
            {"seat":"upstream_a", "bid":[{"id":"a", "impid":"i", "price":1_i32}]},
            {"seat":"upstream_b", "bid":[{"id":"b", "impid":"i", "price":1_i32}]}
        ]);
        for seat in seats.as_array().unwrap() {
            assert_ne!(seat["seat"].as_str().unwrap(), requested.as_str());
        }
        let decoded = source_seat_response(&seats);
        assert_eq!(decoded.bids.len(), 2);
        assert_eq!(decoded.bids[0].source_seat.as_deref(), Some("upstream_a"));
        assert_eq!(decoded.bids[1].source_seat.as_deref(), Some("upstream_b"));
        assert!(!Arc::ptr_eq(
            decoded.bids[0].source_seat.as_ref().unwrap(),
            decoded.bids[1].source_seat.as_ref().unwrap()
        ));
    }

    #[test]
    fn source_seat_shares_one_owner_per_decoded_seat_not_per_value() {
        let decoded = source_seat_response(&json!([
            {"seat":"upstream", "bid":[
                {"id":"a", "impid":"i", "price":1_i32},
                {"id":"b", "impid":"i", "price":1_i32}
            ]},
            {"seat":"upstream", "bid":[{"id":"c", "impid":"i", "price":1_i32}]}
        ]));
        assert_eq!(decoded.bids.len(), 3);
        let owner = decoded.bids[0].source_seat.as_ref().unwrap();
        assert_eq!(&**owner, "upstream");
        assert!(Arc::ptr_eq(
            owner,
            decoded.bids[1].source_seat.as_ref().unwrap()
        ));
        assert_eq!(Arc::strong_count(owner), 2);
        assert!(!Arc::ptr_eq(
            owner,
            decoded.bids[2].source_seat.as_ref().unwrap()
        ));
    }

    #[test]
    fn source_seat_preserves_valid_siblings_around_a_bad_bid() {
        let decoded = source_seat_response(&json!([{"seat":"upstream", "bid":[
            {"id":"before", "impid":"i", "price":1_i32},
            {"id":"bad", "impid":"i", "price":1_i32, "mtype":0_i32},
            {"id":"after", "impid":"i", "price":1_i32}
        ]}]));
        assert_eq!(decoded.errors.len(), 1);
        assert_eq!(decoded.bids.len(), 2);
        assert_eq!(decoded.bids[0].bid.id, "before");
        assert_eq!(decoded.bids[1].bid.id, "after");
        let owner = decoded.bids[0].source_seat.as_ref().unwrap();
        assert_eq!(&**owner, "upstream");
        assert!(Arc::ptr_eq(
            owner,
            decoded.bids[1].source_seat.as_ref().unwrap()
        ));
        assert_eq!(Arc::strong_count(owner), 2);
    }

    #[test]
    fn source_seat_repeated_response_calls_do_not_share_owners() {
        let seats = json!([{"seat":"upstream", "bid":[
            {"id":"a", "impid":"i", "price":1_i32}
        ]}]);
        let first = source_seat_response(&seats);
        let second = source_seat_response(&seats);
        let owner = first.bids[0].source_seat.as_ref().unwrap();
        let other = second.bids[0].source_seat.as_ref().unwrap();
        assert_eq!(owner, other);
        assert!(!Arc::ptr_eq(owner, other));
        let retained = Arc::clone(owner);
        drop(first);
        drop(second);
        assert_eq!(&*retained, "upstream");
        assert_eq!(Arc::strong_count(&retained), 1);
    }

    fn sent(media: CorrelationMedia) -> OutboundRequestMeta {
        let mut bytes = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, 3);
        builder.set_method("POST").unwrap();
        builder.set_url("https://bidder.example.test/").unwrap();
        builder.imp_id("uncorrelated").unwrap();
        builder.imp_id("shared").unwrap();
        builder.correlate_imp(1, media, 0).unwrap();
        builder.imp_id("other").unwrap();
        builder
            .correlate_imp(2, CorrelationMedia::Native, 0)
            .unwrap();
        builder.finish()
    }

    #[test]
    fn split_calls_keep_distinct_media_for_the_same_impression_id() {
        let banner = sent(CorrelationMedia::Banner);
        let video = sent(CorrelationMedia::Video);
        for (call, expected) in [(&video, MediaType::Video), (&banner, MediaType::Banner)] {
            assert_eq!(sent_media_type(call, "shared").unwrap(), expected);
            assert_eq!(sent_media_type(call, "other").unwrap(), MediaType::Native);
        }
    }

    #[test]
    fn sent_media_never_guesses_for_unsent_or_uncorrelated_impressions() {
        let call = sent(CorrelationMedia::Video);
        for (id, message) in [
            ("unsent", "bid response references an unsent impression"),
            ("uncorrelated", "bid response lacks call-local media"),
            ("", "bid response references an unsent impression"),
        ] {
            let error = sent_media_type(&call, id).unwrap_err();
            assert_eq!(error.code, 4);
            assert_eq!(error.message, message);
        }
        let empty = harness::sent_meta("https://bidder.example.test/");
        assert_eq!(empty.imp_id_count(), 0);
        let error = sent_media_type(&empty, "shared").unwrap_err();
        assert_eq!(error.code, 4);
        assert_eq!(
            error.message,
            "bid response references an unsent impression"
        );
    }

    fn bid(fields: serde_json::Value) -> Bid {
        let serde_json::Value::Object(extra_fields) = fields else {
            panic!("bid fields must be an object");
        };
        let mut value = json!({"id": "bid", "impid": "i", "price": 1.0_f64});
        value.as_object_mut().unwrap().extend(extra_fields);
        serde_json::from_value(value).unwrap()
    }

    fn request(impressions: serde_json::Value) -> BidRequest {
        BidRequest {
            id: "request".into(),
            imp: serde_json::from_value(impressions).unwrap(),
            ..BidRequest::default()
        }
    }

    #[test]
    fn standard_and_named_types_identify_all_four_offered_formats() {
        let request =
            request(json!([{"id": "i", "banner": {}, "video": {}, "audio": {}, "native": {}}]));
        for (code, name, expected) in [
            (1_u32, "banner", MediaType::Banner),
            (2_u32, "video", MediaType::Video),
            (3_u32, "audio", MediaType::Audio),
            (4_u32, "native", MediaType::Native),
        ] {
            for fields in [
                json!({"mtype": code}),
                json!({"ext": {"prebid": {"type": name}}}),
                json!({"mtype": code, "ext": {"prebid": {"type": name}}}),
            ] {
                assert_eq!(
                    classify_bid(&request, &bid(fields), Some(expected)).unwrap(),
                    expected
                );
            }
        }
    }

    #[test]
    fn missing_hint_requires_exactly_one_format_on_the_matching_impression() {
        for (format, expected) in [
            ("banner", MediaType::Banner),
            ("video", MediaType::Video),
            ("audio", MediaType::Audio),
            ("native", MediaType::Native),
        ] {
            let request =
                request(json!([{"id": "unrelated", "banner": {}}, {"id": "i", format: {}}]));
            assert_eq!(
                classify_bid(&request, &bid(json!({})), None).unwrap(),
                expected
            );
        }
        for impressions in [
            json!([]),
            json!([{"id": "unknown", "banner": {}}]),
            json!([{"id": "i"}]),
            json!([{"id": "i", "banner": {}, "video": {}}]),
        ] {
            assert_eq!(
                classify_bid(&request(impressions), &bid(json!({})), None)
                    .unwrap_err()
                    .code,
                4
            );
        }
    }

    #[test]
    fn invalid_conflicting_and_unoffered_hints_are_bid_local_failures() {
        let request = request(json!([{"id": "i", "banner": {}, "video": {}}]));
        for fields in [
            json!({"mtype": 0_u32}),
            json!({"mtype": 5_u32}),
            json!({"mtype": -1_i32}),
            json!({"mtype": null}),
            json!({"mtype": "2"}),
            json!({"mtype": 2.5_f64}),
            json!({"mtype": 3_u32}),
            json!({"mtype": 4_u32}),
            json!({"mtype": 1_u32, "ext": {"prebid": {"type": "video"}}}),
            json!({"ext": {"prebid": {"type": "unknown"}}}),
        ] {
            assert_eq!(
                classify_bid(&request, &bid(fields), None).unwrap_err().code,
                4
            );
        }
        assert_eq!(
            classify_bid(
                &request,
                &bid(json!({"mtype": 1_u32})),
                Some(MediaType::Video)
            )
            .unwrap_err()
            .code,
            4
        );
    }

    #[test]
    fn classify_response_partitions_valid_bids_and_errors_across_seats() {
        let request =
            request(json!([{"id": "i", "banner": {}, "video": {}, "audio": {}, "native": {}}]));
        for currency in ["", "EUR"] {
            let parsed = BidResponse {
                seatbid: vec![
                    SeatBid {
                        bid: vec![
                            bid(json!({"id": "bad", "mtype": 0_u32})),
                            bid(json!({"id": "video", "mtype": 2_u32, "price": 9.5_f64})),
                        ],
                        ..SeatBid::default()
                    },
                    SeatBid {
                        bid: vec![
                            bid(json!({"id": "audio", "mtype": 3_u32})),
                            bid(
                                json!({"id": "bad-conflict", "mtype": 1_u32, "ext": {"video": {}}}),
                            ),
                        ],
                        ..SeatBid::default()
                    },
                ],
                cur: currency.into(),
                ..BidResponse::default()
            };
            let mut visited = Vec::new();
            let response = classify_response(parsed, |bid| {
                visited.push(bid.id.clone());
                classify_bid(&request, bid, legacy_media_type(bid)?)
            })
            .unwrap();
            assert_eq!(visited, ["bad", "video", "audio", "bad-conflict"]);
            assert_eq!(response.bids.len(), 2);
            assert_eq!(response.bids[0].bid.id, "video");
            assert_eq!(response.bids[0].bid.price.to_bits(), 9.5_f64.to_bits());
            assert_eq!(response.bids[0].media_type, MediaType::Video);
            assert_eq!(response.bids[1].bid.id, "audio");
            assert_eq!(response.bids[1].media_type, MediaType::Audio);
            assert_eq!(
                response.currency,
                if currency.is_empty() { "USD" } else { currency }
            );
            assert!(response.auction_configs.is_empty());
            assert_eq!(
                response.errors,
                [
                    BidderError::bad_server_response(
                        "bid.mtype must be an integer from 1 through 4"
                    ),
                    BidderError::bad_server_response(
                        "bid response contains conflicting media type hints"
                    ),
                ]
            );
        }
    }

    #[test]
    fn classify_response_returns_every_error_when_all_bids_are_invalid() {
        let request = request(json!([{"id": "i", "banner": {}}]));
        let parsed = BidResponse {
            seatbid: vec![
                SeatBid {
                    bid: vec![
                        bid(json!({"id": "invalid", "mtype": 0_u32})),
                        bid(json!({"id": "video", "mtype": 2_u32})),
                    ],
                    ..SeatBid::default()
                },
                SeatBid {
                    bid: vec![bid(json!({"id": "audio", "mtype": 3_u32}))],
                    ..SeatBid::default()
                },
            ],
            ..BidResponse::default()
        };
        let mut visited = Vec::new();
        let errors = classify_response(parsed, |bid| {
            visited.push(bid.id.clone());
            classify_bid(&request, bid, None)
        })
        .unwrap_err();
        assert_eq!(visited, ["invalid", "video", "audio"]);
        assert_eq!(
            errors,
            [
                BidderError::bad_server_response("bid.mtype must be an integer from 1 through 4"),
                BidderError::bad_server_response(
                    "bid response media type was not offered by its impression"
                ),
                BidderError::bad_server_response(
                    "bid response media type was not offered by its impression"
                ),
            ]
        );
    }

    #[test]
    fn classify_response_keeps_empty_responses_distinct_from_all_invalid() {
        for seatbid in [vec![], vec![SeatBid::default()]] {
            let parsed = BidResponse {
                seatbid,
                ..BidResponse::default()
            };
            let response = classify_response(parsed, |_bid| {
                panic!("empty response has no bids to classify")
            })
            .unwrap();
            assert!(response.bids.is_empty());
            assert!(response.errors.is_empty());
            assert_eq!(response.currency, "USD");
            assert!(response.auction_configs.is_empty());
        }
    }

    #[test]
    fn candidate_parse_failures_coexist_with_semantic_errors_and_successes() {
        let request = request(json!([{"id":"i", "banner":{}}]));
        let response = harness::budgeted_response(
            200,
            serde_json::to_vec(&json!({"id":"r",
            "seatbid":[{"bid":[
                {"id":"bad_ext", "impid":"i", "price":1_i32, "ext":false},
                {"id":"bad_media", "impid":"i", "price":1_i32, "mtype":0_i32},
                {"id":"good", "impid":"i", "price":1_i32, "mtype":1_i32}
            ]}]}))
            .unwrap(),
        );
        let (parsed, errors) = response.parse_bid_candidates().unwrap();
        let decoded =
            classify_response_with_errors(parsed, errors, |bid| classify_bid(&request, bid, None))
                .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "good");
        assert_eq!(decoded.errors.len(), 2);
        assert_eq!(decoded.errors[0].code, 14);
        assert_eq!(
            decoded.errors[0].kind,
            crate::BidderErrorKind::ResponseParse
        );
        assert_eq!(decoded.errors[1].code, 4);
        assert_eq!(decoded.errors[1].kind, crate::BidderErrorKind::Other);
    }

    #[test]
    fn all_candidate_parse_failures_are_not_mistaken_for_an_empty_no_bid() {
        let response = harness::budgeted_response(
            200,
            serde_json::to_vec(&json!({"id":"r",
                "seatbid":[{"bid":[{"id":"bad", "impid":"i", "price":1_i32, "ext":false}]}]
            }))
            .unwrap(),
        );
        let (parsed, errors) = response.parse_bid_candidates().unwrap();
        let classification_errors = classify_response_with_errors(parsed, errors, |_bid| {
            panic!("refused candidate cannot reach classification")
        })
        .unwrap_err();
        assert_eq!(classification_errors.len(), 1);
        assert_eq!(classification_errors[0].code, 14);
        assert_eq!(
            classification_errors[0].kind,
            crate::BidderErrorKind::ResponseParse
        );
    }
}

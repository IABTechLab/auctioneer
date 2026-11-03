use super::parameters::{known, parameters, text};
use crate::bidders::media_type::{classify_bid, sent_media_type};
use crate::bidders::status::{bad_input_or_server, standard_status};
use crate::{
    BidderError, BidderErrorKind, BidderHttpResponse, BidderResponse, OutboundRequestMeta, TypedBid,
};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
use serde_json::{Map, Value};

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        bad_input_or_server(status, "unexpected rubicon response status")
    })? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    let root_id = take_known(&mut parsed.other, "bidid")
        .and_then(owned_text)
        .map_err(|()| vec![invalid_dto()])?;
    for seat in &parsed.seatbid {
        text(known(&seat.other, "buyer").map_err(|()| vec![invalid_dto()])?)
            .map_err(|()| vec![invalid_dto()])?;
    }
    let count = parsed.seatbid.iter().map(|seat| seat.bid.len()).sum();
    let mut decoded = BidderResponse {
        bids: Vec::with_capacity(count),
        currency: if parsed.cur.is_empty() {
            "USD".into()
        } else {
            parsed.cur
        },
        errors: parse_errors,
        auction_configs: Vec::new(),
    };
    // Repeated root IDs, seats and native markup share one derived-text bound.
    let mut text_remaining = response.body_len();
    let root_override = request_override(request);
    for seat in parsed.seatbid {
        let buyer = text(known(&seat.other, "buyer").map_err(|()| vec![invalid_dto()])?)
            .map_err(|()| vec![invalid_dto()])?
            .parse::<i64>()
            .unwrap_or_default();
        for mut bid in seat.bid {
            let override_price = request
                .imp
                .iter()
                .rev()
                .filter(|imp| imp.id == bid.impid)
                .find_map(|imp| parameters(imp).ok().map(|params| params.price_override))
                .filter(|price| *price != 0.0_f64)
                .unwrap_or(root_override);
            if override_price > 0.0_f64 {
                bid.price = override_price;
            }
            if bid.price == 0.0_f64 {
                continue;
            }
            match finish_bid(
                request,
                sent,
                response,
                &mut bid,
                BidContext {
                    buyer,
                    seat: &seat.seat,
                    root_id: &root_id,
                },
                &mut text_remaining,
            ) {
                Ok(media_type) => decoded.bids.push(TypedBid { bid, media_type }),
                Err(error) => decoded.errors.push(error),
            }
        }
    }
    if decoded.bids.is_empty() && !decoded.errors.is_empty() {
        Err(decoded.errors)
    } else {
        Ok(decoded)
    }
}

fn take_known(fields: &mut Map<String, Value>, name: &str) -> Result<Option<Value>, ()> {
    let mut selected = None;
    let mut ambiguous = false;
    fields.retain(|key, value| {
        if !key.eq_ignore_ascii_case(name) {
            return true;
        }
        if selected.is_some() {
            ambiguous = true;
        } else {
            selected = Some(value.take());
        }
        false
    });
    if ambiguous { Err(()) } else { Ok(selected) }
}

fn owned_text(value: Option<Value>) -> Result<String, ()> {
    match value {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(string)) => Ok(string),
        Some(_) => Err(()),
    }
}

fn invalid_dto() -> BidderError {
    let mut error = BidderError::failed_to_unmarshal("invalid rubicon response fields");
    error.kind = BidderErrorKind::ResponseParse;
    error
}
fn invalid_bid() -> BidderError {
    BidderError::bad_server_response("invalid rubicon bid fields")
}
fn expansion_error() -> BidderError {
    BidderError::bad_server_response("rubicon derived response exceeded limits")
}
fn spend(remaining: &mut usize, bytes: usize) -> Result<(), BidderError> {
    *remaining = remaining.checked_sub(bytes).ok_or_else(expansion_error)?;
    Ok(())
}
fn request_override(request: &BidRequest) -> f64 {
    request
        .ext
        .as_ref()
        .and_then(|ext| ext.prebid.as_ref())
        .and_then(|prebid| known(&prebid.other, "bidders").ok().flatten())
        .and_then(Value::as_object)
        .and_then(|bidders| known(bidders, "rubicon").ok().flatten())
        .and_then(Value::as_object)
        .and_then(|rubicon| known(rubicon, "debug").ok().flatten())
        .and_then(Value::as_object)
        .and_then(|debug| known(debug, "cpmoverride").ok().flatten())
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .unwrap_or_default()
}
#[derive(Clone, Copy)]
struct BidContext<'a> {
    buyer: i64,
    seat: &'a str,
    root_id: &'a str,
}
fn finish_bid(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
    bid: &mut Bid,
    context: BidContext<'_>,
    remaining: &mut usize,
) -> Result<MediaType, BidderError> {
    if let Some(value) = take_known(&mut bid.other, "mtype").map_err(|()| invalid_bid())? {
        if !matches!(value.as_i64(), Some(1 | 2 | 4)) {
            return Err(invalid_bid());
        }
        bid.other.insert("mtype".into(), value);
    }
    let actual = sent_media_type(sent, &bid.impid)?;
    if !matches!(
        actual,
        MediaType::Banner | MediaType::Video | MediaType::Native
    ) {
        return Err(invalid_bid());
    }
    let media_type = classify_bid(request, bid, Some(actual))?;
    let checkpoint = *remaining;
    let result = (|| {
        if bid.id == "0" {
            spend(remaining, context.root_id.len())?;
            context.root_id.clone_into(&mut bid.id);
        }
        let native = take_known(&mut bid.other, "adm_native").map_err(|()| invalid_bid())?;
        if bid.adm.as_ref().is_none_or(String::is_empty) {
            let projected = response.project_adm_value(native.as_ref().unwrap_or(&Value::Null))?;
            spend(remaining, projected.len())?;
            bid.adm = Some(projected);
        }
        if context.buyer > 0 || !context.seat.is_empty() {
            let ext = bid.ext.get_or_insert_with(Default::default);
            let prebid = ext.prebid.get_or_insert_with(Default::default);
            if prebid
                .meta
                .as_ref()
                .is_some_and(|value| !value.is_null() && !value.is_object())
            {
                return Ok(media_type);
            }
            spend(remaining, context.seat.len())?;
            let mut meta = match prebid.meta.take() {
                Some(Value::Object(meta)) => meta,
                _ => Map::new(),
            };
            take_known(&mut meta, "networkId").map_err(|()| invalid_bid())?;
            take_known(&mut meta, "seat").map_err(|()| invalid_bid())?;
            if context.buyer != 0 {
                meta.insert("networkId".into(), Value::from(context.buyer));
            }
            if !context.seat.is_empty() {
                meta.insert("seat".into(), Value::String(context.seat.to_owned()));
            }
            prebid.meta = Some(Value::Object(meta));
        }
        Ok(media_type)
    })();
    if result.is_err() {
        *remaining = checkpoint;
    }
    result
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::bidders::rubicon::RubiconBidder;
    use crate::bidders::rubicon::test_support::{
        banner, bid, decode, envelope, mixed, request, response, sent,
    };
    use crate::correlation::CorrelationMedia;
    use crate::{Bidder as _, BidderErrorKind, BidderHttpResponse};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::json;

    #[test]
    fn status_handling_distinguishes_no_content_bad_input_and_provider_failure() {
        for (status, code) in [
            (204_u16, None),
            (400, Some(2_u32)),
            (500, Some(4)),
            (201, Some(4)),
        ] {
            let result = RubiconBidder.make_bids(
                &request(&json!([banner("i")])),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &response(status, &json!({})),
            );
            if let Some(expected_code) = code {
                let errors = result.unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, expected_code);
            } else {
                let decoded = result.unwrap();
                assert!(decoded.bids.is_empty());
                assert!(decoded.errors.is_empty());
                assert!(decoded.auction_configs.is_empty());
            }
        }
    }

    #[test]
    fn zero_prices_are_skipped_and_positive_impression_overrides_precede_root_overrides() {
        let mut imp = mixed("i");
        imp["ext"]["bidder"]["debug"] = json!({"cpmoverride":20_i32});
        let request: BidRequest = serde_json::from_value(json!({"id":"r","site":{},"imp":[imp],
            "ext":{"prebid":{"bidders":{"rubicon":{"debug":{"cpmoverride":10_i32}}}}}}))
        .unwrap();
        let decoded = RubiconBidder
            .make_bids(
                &request,
                &sent(Some(CorrelationMedia::Video), &["i"]),
                &response(200, &envelope(&json!([bid(json!({"price":0_i32}))]))),
            )
            .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.price.to_bits(), 20.0_f64.to_bits());
        let skipped_zero = decode(
            &envelope(&json!([
                bid(json!({"id":"zero","price":0_i32})),
                bid(json!({"id":"positive","price":1.25_f64}))
            ])),
            CorrelationMedia::Banner,
        )
        .unwrap();
        assert_eq!(skipped_zero.bids.len(), 1);
        assert_eq!(skipped_zero.bids[0].bid.id, "positive");
        assert!(skipped_zero.errors.is_empty());
    }

    #[test]
    fn root_price_override_is_used_for_absent_or_zero_impression_override_not_a_negative_one() {
        for (override_value, expected) in [
            (None, 10.0_f64),
            (Some(0_i32), 10.0_f64),
            (Some(-1_i32), 1.25_f64),
        ] {
            let mut imp = mixed("i");
            if let Some(value) = override_value {
                imp["ext"]["bidder"]["debug"] = json!({"cpmoverride":value});
            }
            let request: BidRequest =
                serde_json::from_value(json!({"id":"r","site":{},"imp":[imp],
                "ext":{"prebid":{"bidders":{"rubicon":{"debug":{"cpmoverride":10_i32}}}}}}))
                .unwrap();
            let decoded = RubiconBidder
                .make_bids(
                    &request,
                    &sent(Some(CorrelationMedia::Video), &["i"]),
                    &response(200, &envelope(&json!([bid(json!({}))]))),
                )
                .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.price.to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn zero_bid_id_uses_root_bidid_but_nonzero_bid_ids_are_unchanged() {
        let mut value = envelope(&json!([bid(json!({"id":"0"})), bid(json!({"id":"keep"}))]));
        value["bidid"] = json!("root-bid-id");
        let decoded = decode(&value, CorrelationMedia::Banner).unwrap();
        assert_eq!(decoded.bids.len(), 2);
        assert_eq!(decoded.bids[0].bid.id, "root-bid-id");
        assert_eq!(decoded.bids[1].bid.id, "keep");
    }

    #[test]
    fn proprietary_native_adm_is_projected_only_when_standard_adm_is_empty() {
        for (adm, expected) in [
            (Some("standard"), "standard"),
            (Some(""), "{\"property1\":\"value1\"}"),
            (None, "{\"property1\":\"value1\"}"),
        ] {
            let mut candidate = bid(json!({"adm_native":{"property1":"value1"}}));
            if let Some(markup) = adm {
                candidate["adm"] = json!(markup);
            } else {
                candidate.as_object_mut().unwrap().remove("adm");
            }
            let decoded = decode(&envelope(&json!([candidate])), CorrelationMedia::Native).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.adm.as_deref(), Some(expected));
            assert!(!decoded.bids[0].bid.other.contains_key("adm_native"));
        }
    }

    #[test]
    fn missing_and_null_proprietary_native_adm_follow_source_null_projection() {
        for fields in [json!({"adm":""}), json!({"adm":"","adm_native":null})] {
            let decoded =
                decode(&envelope(&json!([bid(fields)])), CorrelationMedia::Native).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.adm.as_deref(), Some("null"));
        }
    }

    #[test]
    fn seat_buyer_and_seat_metadata_replace_only_their_source_owned_fields() {
        let mut value = envelope(&json!([bid(json!({"ext":{"prebid":{"meta":{
            "networkId":77_i32,"seat":"old","advertiserName":"keep"}},"bidder":{"keep":true}}}))]));
        value["seatbid"][0]["buyer"] = json!("123");
        value["seatbid"][0]["seat"] = json!("rubicon");
        let decoded = decode(&value, CorrelationMedia::Banner).unwrap();
        assert_eq!(decoded.bids.len(), 1);
        let wire = serde_json::to_value(&decoded.bids[0].bid).unwrap();
        assert_eq!(
            wire["ext"]["prebid"]["meta"],
            json!({
                "networkId":123_i32,"seat":"rubicon","advertiserName":"keep"
            })
        );
        assert_eq!(wire["ext"]["bidder"], json!({"keep":true}));
    }

    #[test]
    fn nonnumeric_buyer_still_sets_seat_without_leaking_its_original_text() {
        let mut value = envelope(&json!([bid(json!({}))]));
        value["seatbid"][0]["buyer"] = json!("not-a-number");
        value["seatbid"][0]["seat"] = json!("rubicon");
        let decoded = decode(&value, CorrelationMedia::Banner).unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(
            serde_json::to_value(&decoded.bids[0].bid).unwrap()["ext"]["prebid"]["meta"],
            json!({"seat":"rubicon"})
        );
    }

    #[test]
    fn numeric_seat_buyer_preserves_response_parse_provenance() {
        let mut valid = envelope(&json!([bid(json!({}))]));
        valid["seatbid"][0]["buyer"] = json!("123");
        assert_eq!(
            decode(&valid, CorrelationMedia::Banner).unwrap().bids.len(),
            1
        );

        let mut malformed = envelope(&json!([bid(json!({}))]));
        malformed["seatbid"][0]["buyer"] = json!(123_i64);
        let errors = decode(&malformed, CorrelationMedia::Banner).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 14_u32);
        assert_eq!(
            errors[0].kind,
            BidderErrorKind::ResponseParse,
            "exchange debug classification must retain response_parse provenance"
        );
    }

    #[test]
    fn absent_standard_media_uses_actual_call_media_and_supported_mismatch_is_bid_local() {
        for (media, expected) in [
            (CorrelationMedia::Banner, MediaType::Banner),
            (CorrelationMedia::Video, MediaType::Video),
            (CorrelationMedia::Native, MediaType::Native),
        ] {
            let decoded = decode(&envelope(&json!([bid(json!({}))])), media).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, expected);
        }
        let decoded = decode(
            &envelope(&json!([
                bid(json!({"id":"wrong","mtype":2_i32})),
                bid(json!({"id":"valid","mtype":1_i32}))
            ])),
            CorrelationMedia::Banner,
        )
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "valid");
        assert_eq!(decoded.errors.len(), 1);
        assert_eq!(decoded.errors[0].code, 4);
    }

    #[test]
    fn invalid_standard_media_rejects_before_fallback_and_keeps_valid_siblings() {
        for invalid in [
            json!(null),
            json!(0_i32),
            json!(3_i32),
            json!(5_i32),
            json!(-1_i32),
            json!("1"),
            json!(true),
            json!(1.0_f64),
            json!([]),
        ] {
            let decoded = decode(
                &envelope(&json!([
                    bid(json!({"id":"invalid","mtype":invalid})),
                    bid(json!({"id":"valid","mtype":1_i32}))
                ])),
                CorrelationMedia::Banner,
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "valid");
            assert_eq!(decoded.errors.len(), 1);
            assert_eq!(decoded.errors[0].code, 4);
        }
    }

    #[test]
    fn singleton_case_spellings_are_accepted_and_competing_media_spellings_are_bid_local_errors() {
        let singleton = decode(
            &envelope(&json!([bid(json!({"Mtype":1_i32}))])),
            CorrelationMedia::Banner,
        )
        .unwrap();
        assert_eq!(singleton.bids.len(), 1);
        let decoded = decode(
            &envelope(&json!([
                bid(json!({"id":"ambiguous","mtype":1_i32,"Mtype":1_i32})),
                bid(json!({"id":"valid"}))
            ])),
            CorrelationMedia::Banner,
        )
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "valid");
        assert_eq!(decoded.errors.len(), 1);
    }

    #[test]
    fn missing_correlation_and_unknown_or_unsent_ids_cannot_use_request_inventory_as_proof() {
        let request = request(&json!([mixed("i"), mixed("unsent")]));
        for meta in [
            sent(None, &["i"]),
            sent(Some(CorrelationMedia::Banner), &["other"]),
        ] {
            let errors = RubiconBidder
                .make_bids(
                    &request,
                    &meta,
                    &response(200, &envelope(&json!([bid(json!({}))]))),
                )
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 4);
        }
        let decoded = RubiconBidder
            .make_bids(
                &request,
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &response(
                    200,
                    &envelope(&json!([
                        bid(json!({"id":"unsent","impid":"unsent"})),
                        bid(json!({"id":"unknown","impid":"unknown"})),
                        bid(json!({"id":"valid"}))
                    ])),
                ),
            )
            .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "valid");
        assert_eq!(decoded.errors.len(), 2);
    }

    #[test]
    fn response_currency_preserves_source_values_with_usd_only_for_empty_or_missing() {
        for (currency, expected) in [
            (None, "USD"),
            (Some(""), "USD"),
            (Some("EUR"), "EUR"),
            (Some("USD"), "USD"),
        ] {
            let mut value = envelope(&json!([bid(json!({}))]));
            if let Some(code) = currency {
                value["cur"] = json!(code);
            } else {
                value.as_object_mut().unwrap().remove("cur");
            }
            let decoded = decode(&value, CorrelationMedia::Banner).unwrap();
            assert_eq!(decoded.currency, expected);
        }
    }

    #[test]
    fn zero_node_budget_refuses_parsing_and_a_within_budget_control_produces_a_bid() {
        let value = envelope(&json!([bid(json!({}))]));
        let raw = serde_json::to_vec(&value).unwrap();
        let error = RubiconBidder
            .make_bids(
                &request(&json!([mixed("i")])),
                &sent(Some(CorrelationMedia::Banner), &["i"]),
                &BidderHttpResponse::new(200, vec![], raw),
            )
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, 4);
        let decoded = decode(&value, CorrelationMedia::Banner).unwrap();
        assert_eq!(decoded.bids.len(), 1);
    }
}

use super::request::known;
use crate::bidders::{
    media_type::{classify_bid, sent_media_type},
    status::{bad_input_or_server, standard_status},
    wire_fields::{canonical_integer, signed_integer},
};
use crate::{
    BidderError, BidderHttpResponse, BidderResponse, BrowserAuctionConfig, OutboundRequestMeta,
    TypedBid,
};
use auctioneer_config::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
use serde_json::{Map, Value};

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        bad_input_or_server(status, "unexpected openx response status")
    })? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    // Duration/category DTO failures remain response-fatal. Markup types follow
    // the OpenRTB-first policy at bid-local scope instead of Go's DTO defaults.
    for bid in parsed.seatbid.iter().flat_map(|seat| &seat.bid) {
        signed_integer(&bid.other, "dur").map_err(|_error| vec![invalid_dto()])?;
        validate_categories(&bid.other).map_err(|()| vec![invalid_dto()])?;
    }
    let auction_configs = extract_configs(parsed.ext.take()).unwrap_or_default();
    let bid_count = parsed.seatbid.iter().map(|seat| seat.bid.len()).sum();
    let mut decoded = BidderResponse {
        bids: Vec::with_capacity(bid_count),
        currency: if parsed.cur.is_empty() {
            "USD".into()
        } else {
            parsed.cur
        },
        errors: parse_errors,
        auction_configs,
    };
    for mut bid in parsed.seatbid.into_iter().flat_map(|seat| seat.bid) {
        let duration =
            canonical_integer(&mut bid.other, "dur").map_err(|_error| vec![invalid_dto()])?;
        canonical_categories(&mut bid.other).map_err(|()| vec![invalid_dto()])?;
        match classify_openx_bid(request, sent, &mut bid) {
            Ok(media_type) => {
                enrich_bid(&mut bid, duration);
                decoded.bids.push(TypedBid { bid, media_type });
            }
            Err(error) => decoded.errors.push(error),
        }
    }
    if decoded.bids.is_empty() && decoded.auction_configs.is_empty() && !decoded.errors.is_empty() {
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

fn invalid_dto() -> BidderError {
    BidderError::failed_to_unmarshal("invalid openx response fields")
}

fn invalid_mtype() -> BidderError {
    BidderError::bad_server_response("invalid or unsupported openx mtype")
}

fn validate_categories(fields: &Map<String, Value>) -> Result<(), ()> {
    match known(fields, "cat")? {
        None | Some(Value::Null) => Ok(()),
        Some(Value::Array(values))
            if values
                .iter()
                .all(|value| value.is_string() || value.is_null()) =>
        {
            Ok(())
        }
        Some(
            Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Array(_)
            | Value::Object(_),
        ) => Err(()),
    }
}

fn canonical_categories(fields: &mut Map<String, Value>) -> Result<(), ()> {
    match take_known(fields, "cat")? {
        Some(Value::Array(mut values)) if !values.is_empty() => {
            for value in &mut values {
                if value.is_null() {
                    *value = Value::String(String::new());
                }
            }
            fields.insert("cat".into(), Value::Array(values));
        }
        None | Some(Value::Null | Value::Array(_)) => {}
        Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_)) => {
            return Err(());
        }
    }
    Ok(())
}

fn classify_openx_bid(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    bid: &mut Bid,
) -> Result<MediaType, BidderError> {
    if !sent
        .covered_imp_ids()
        .any(|id| !id.is_empty() && id == bid.impid)
    {
        return Err(BidderError::bad_server_response(
            "bid response references an unsent impression",
        ));
    }
    let declared = known(&bid.other, "mtype").map_err(|()| invalid_mtype())?;
    let media = match declared {
        None => sent_media_type(sent, &bid.impid)?,
        Some(markup) => match markup.as_u64() {
            Some(1) => MediaType::Banner,
            Some(2) => MediaType::Video,
            Some(4) => MediaType::Native,
            None | Some(_) => return Err(invalid_mtype()),
        },
    };
    if declared.is_some() {
        canonical_integer(&mut bid.other, "mtype").map_err(|_error| invalid_mtype())?;
    }
    classify_bid(request, bid, Some(media))
}

struct MetadataIds {
    network: i64,
    brand: i64,
    advertiser: i64,
}

fn quoted_integer(encoded: Option<&Value>) -> Result<i64, ()> {
    match encoded {
        None | Some(Value::Null) => Ok(0),
        Some(Value::String(value)) => value.parse().map_err(|_error| ()),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => Err(()),
    }
}

fn metadata_ids(bid: &Bid) -> Option<MetadataIds> {
    let fields = &bid.ext.as_ref()?.other;
    let network = quoted_integer(known(fields, "dsp_id").ok()?).ok()?;
    let brand = quoted_integer(known(fields, "brand_id").ok()?).ok()?;
    let advertiser = match known(fields, "buyer_id").ok()? {
        None | Some(Value::Null) => 0,
        Some(Value::String(value)) => value.parse::<i64>().unwrap_or_default(),
        Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
            return None;
        }
    };
    (network > 0 || brand > 0 || advertiser > 0).then_some(MetadataIds {
        network,
        brand,
        advertiser,
    })
}

fn enrich_bid(bid: &mut Bid, duration: i64) {
    let candidate_ids = metadata_ids(bid);
    let category = bid
        .other
        .get("cat")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let ext = bid.ext.get_or_insert_with(Default::default);
    let prebid = ext.prebid.get_or_insert_with(Default::default);
    let mut video = Map::new();
    video.insert("duration".into(), Value::from(duration));
    video.insert("primary_category".into(), Value::String(category));
    prebid.other.insert("video".into(), Value::Object(video));
    if let Some(ids) = candidate_ids {
        let mut fields = Map::new();
        for (key, value) in [
            ("networkId", ids.network),
            ("brandId", ids.brand),
            ("advertiserId", ids.advertiser),
        ] {
            if value != 0 {
                fields.insert(key.into(), Value::from(value));
            }
        }
        prebid.meta = Some(Value::Object(fields));
    }
}

fn extract_configs(ext: Option<Value>) -> Result<Vec<BrowserAuctionConfig>, ()> {
    let Some(Value::Object(mut fields)) = ext else {
        return Ok(Vec::new());
    };
    let config_map = match take_known(&mut fields, "fledge_auction_configs")? {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Object(configs)) => configs,
        Some(Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_)) => {
            return Err(());
        }
    };
    let mut configs: Vec<_> = config_map
        .into_iter()
        .map(|(imp_id, config)| BrowserAuctionConfig { imp_id, config })
        .collect();
    configs.sort_unstable_by(|left, right| left.imp_id.cmp(&right.imp_id));
    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::super::OpenxBidder;
    use super::super::tests::{ENDPOINT, imp, request};
    use crate::bidders::harness;
    use crate::correlation::CorrelationMedia;
    use crate::{
        Bidder as _, BidderError, BidderErrorKind, BidderHttpResponse, BidderResponse,
        META_BYTES_CAPACITY, OutboundRequestMeta, OutboundRequestMetaBuilder,
    };
    use auctioneer_config::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    fn sent(rows: &[(&str, CorrelationMedia)]) -> OutboundRequestMeta {
        let mut bytes = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut bytes, 100);
        meta.set_method("POST").unwrap();
        meta.set_url(ENDPOINT).unwrap();
        for (index, (id, media)) in rows.iter().enumerate() {
            meta.imp_id(id).unwrap();
            meta.correlate_imp(u16::try_from(index).unwrap(), *media, 0)
                .unwrap();
        }
        meta.finish()
    }

    fn response(document: &Value) -> BidderHttpResponse {
        harness::budgeted_response(200, serde_json::to_vec(document).unwrap())
    }

    fn bid(id: &str, impid: &str) -> Value {
        json!({"id":id, "impid":impid, "price":1.25_f64})
    }

    fn decode(
        request: &BidRequest,
        sent: &OutboundRequestMeta,
        bids: Value,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        let mut document = json!({"id":"fake_response", "seatbid":[{"bid":[]}]});
        document["seatbid"][0]["bid"] = bids;
        OpenxBidder.make_bids(request, sent, &response(&document))
    }

    #[test]
    fn status_policy_is_204_empty_400_bad_input_and_other_non200_bad_server_response() {
        let request = request(json!([]));
        let sent = sent(&[]);
        for (status, code) in [(400, 2), (201, 4), (429, 4), (500, 4)] {
            let errors = OpenxBidder
                .make_bids(&request, &sent, &harness::budgeted_response(status, vec![]))
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, code);
        }
        let decoded = OpenxBidder
            .make_bids(&request, &sent, &harness::budgeted_response(204, vec![]))
            .unwrap();
        assert!(decoded.bids.is_empty());
        assert!(decoded.auction_configs.is_empty());
        assert!(decoded.errors.is_empty());
    }

    #[test]
    fn response_currency_defaults_to_usd_but_preserves_nonempty_source_currency() {
        for (currency_field, expected) in [(None, "USD"), (Some(""), "USD"), (Some("EUR"), "EUR")] {
            let mut value = json!({"id":"fake_response"});
            if let Some(currency) = currency_field {
                value["cur"] = json!(currency);
            }
            let decoded = OpenxBidder
                .make_bids(&request(json!([])), &sent(&[]), &response(&value))
                .unwrap();
            assert_eq!(decoded.currency, expected);
        }
    }

    #[test]
    fn missing_mtype_uses_exact_call_correlation_for_each_supported_media() {
        for (name, media, expected) in [
            ("banner", CorrelationMedia::Banner, MediaType::Banner),
            ("video", CorrelationMedia::Video, MediaType::Video),
            ("native", CorrelationMedia::Native, MediaType::Native),
        ] {
            let request = request(json!([imp("fake_imp", name)]));
            let decoded = decode(
                &request,
                &sent(&[("fake_imp", media)]),
                json!([bid("fake_bid", "fake_imp")]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, expected);
        }
    }

    #[test]
    fn explicit_supported_mtype_overrides_fallback_if_offered_by_original_impression() {
        let mut impression = imp("fake_imp", "banner");
        impression["video"] = json!({});
        impression["native"] = json!({});
        let request = request(json!([impression]));
        for (mtype, expected) in [
            (1_i32, MediaType::Banner),
            (2_i32, MediaType::Video),
            (4_i32, MediaType::Native),
        ] {
            let mut response_bid = bid("fake_bid", "fake_imp");
            response_bid["mtype"] = json!(mtype);
            let decoded = decode(
                &request,
                &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                json!([response_bid]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, expected);
        }
    }

    #[test]
    fn unsupported_or_invalid_mtypes_refuse_only_the_affected_bid() {
        let mut impression = imp("fake_imp", "banner");
        impression["video"] = json!({});
        impression["native"] = json!({});
        impression["audio"] = json!({});
        let request = request(json!([impression]));
        for (key, value) in [
            ("mtype", json!(0_i32)),
            ("mtype", json!(3_i32)),
            ("mtype", json!(5_i32)),
            ("mtype", json!(-1_i32)),
            ("MTYPE", json!(3_i32)),
            ("mtype", Value::Null),
            ("mtype", json!(128_i32)),
            ("mtype", json!(u64::MAX)),
        ] {
            let mut value_bid = bid("fake_bid", "fake_imp");
            value_bid[key] = value;
            let decoded = decode(
                &request,
                &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                json!([value_bid, bid("fake_good", "fake_imp")]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "fake_good");
            assert_eq!(decoded.bids[0].media_type, MediaType::Banner);
            assert_eq!(decoded.errors.len(), 1);
            assert_eq!(decoded.errors[0].code, 4);
        }
    }

    #[test]
    fn malformed_mtype_preserves_valid_bid_siblings() {
        let request = request(json!([imp("fake_imp", "banner")]));
        for value in [json!(2.5_f64), json!("2"), json!(true), json!({})] {
            let mut bad = bid("fake_bad", "fake_imp");
            bad["mtype"] = value;
            let decoded = decode(
                &request,
                &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                json!([bid("fake_good", "fake_imp"), bad]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "fake_good");
            assert_eq!(decoded.errors.len(), 1);
            assert_eq!(decoded.errors[0].code, 4);
        }
    }

    #[test]
    fn one_folded_mtype_is_valid_and_competing_spellings_are_bid_local() {
        let mut impression = imp("fake_imp", "banner");
        impression["video"] = json!({});
        let request = request(json!([impression]));
        for key in ["MTYPE", "Mtype"] {
            let mut response_bid = bid("fake_video", "fake_imp");
            response_bid[key] = json!(2_i32);
            let decoded = decode(
                &request,
                &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                json!([response_bid]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, MediaType::Video);
            assert!(decoded.errors.is_empty());
        }
        for (first, second) in [
            (json!(1_i32), json!(1_i32)),
            (json!(1_i32), Value::Null),
            (Value::Null, Value::Null),
        ] {
            let mut ambiguous = bid("fake_bad", "fake_imp");
            ambiguous["mtype"] = first;
            ambiguous["MTYPE"] = second;
            let decoded = decode(
                &request,
                &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                json!([ambiguous, bid("fake_good", "fake_imp")]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "fake_good");
            assert_eq!(decoded.errors.len(), 1);
            assert_eq!(decoded.errors[0].code, 4);
        }
    }

    #[test]
    fn invalid_media_codes_do_not_erase_independent_browser_configs() {
        let request = request(json!([imp("fake_imp", "banner")]));
        for marker in [
            json!(0_i32),
            json!(3_i32),
            json!(5_i32),
            Value::Null,
            json!("2"),
        ] {
            let mut bad = bid("fake_bad", "fake_imp");
            bad["mtype"] = marker;
            let decoded = OpenxBidder
                .make_bids(
                    &request,
                    &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                    &response(&json!({
                        "id":"fake_response", "seatbid":[{"bid":[bad]}],
                        "ext":{"fledge_auction_configs":{"fake_imp":{"fake_keep":true}}}
                    })),
                )
                .unwrap();
            assert!(decoded.bids.is_empty());
            assert_eq!(decoded.errors.len(), 1);
            assert_eq!(decoded.errors[0].code, 4);
            assert_eq!(decoded.auction_configs.len(), 1);
            assert_eq!(decoded.auction_configs[0].imp_id, "fake_imp");
            assert_eq!(decoded.auction_configs[0].config, json!({"fake_keep":true}));
        }
    }

    #[test]
    fn unsent_and_unoffered_bids_are_local_refusals_preserving_valid_siblings() {
        let request = request(json!([
            imp("fake_sent", "banner"),
            imp("fake_unsent", "banner")
        ]));
        let mut wrong = bid("fake_wrong", "fake_sent");
        wrong["mtype"] = json!(2_i32);
        let decoded = decode(
            &request,
            &sent(&[("fake_sent", CorrelationMedia::Banner)]),
            json!([
                wrong,
                bid("fake_unsent_bid", "fake_unsent"),
                bid("fake_good", "fake_sent")
            ]),
        )
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "fake_good");
        assert_eq!(decoded.errors.len(), 2);
        assert!(decoded.errors.iter().all(|error| error.code == 4));
    }

    #[test]
    fn metadata_ids_are_string_encoded_and_invalid_brand_suppresses_only_metadata() {
        let request = request(json!([imp("fake_imp", "banner")]));
        for (extension, expected) in [
            (
                json!({"dsp_id":"456", "brand_id":"789", "buyer_id":"123"}),
                Some(json!({"networkId":456_i32, "brandId":789_i32, "advertiserId":123_i32})),
            ),
            (
                json!({"DSP_ID":"456", "BRAND_ID":"789", "BUYER_ID":"123"}),
                Some(json!({"networkId":456_i32, "brandId":789_i32, "advertiserId":123_i32})),
            ),
            (
                json!({"dsp_id":"456", "buyer_id":"not-an-id"}),
                Some(json!({"networkId":456_i32})),
            ),
            (
                json!({"buyer_id":"123"}),
                Some(json!({"advertiserId":123_i32})),
            ),
            (json!({"brand_id":"789"}), Some(json!({"brandId":789_i32}))),
            (json!({"dsp_id":"456"}), Some(json!({"networkId":456_i32}))),
            (
                json!({"buyer_id":"123", "brand_id":"789"}),
                Some(json!({"advertiserId":123_i32, "brandId":789_i32})),
            ),
            (
                json!({"dsp_id":"456", "brand_id":"789"}),
                Some(json!({"networkId":456_i32, "brandId":789_i32})),
            ),
            (json!({"buyer_id":"0", "dsp_id":"0", "brand_id":"0"}), None),
            (
                json!({"buyer_id":"-123", "dsp_id":"-456", "brand_id":"-789"}),
                None,
            ),
            (
                json!({"buyer_id":"123", "dsp_id":"-456", "brand_id":"-789"}),
                Some(json!({"advertiserId":123_i32, "networkId":-456_i32, "brandId":-789_i32})),
            ),
            (
                json!({"dsp_id":"456", "brand_id":"invalid", "buyer_id":"123"}),
                None,
            ),
            (json!({"dsp_id":456_i32}), None),
            (json!({"dsp_id":"456", "DSP_ID":"456"}), None),
            (json!({}), None),
            (Value::Null, None),
        ] {
            let mut value = bid("fake_bid", "fake_imp");
            value["ext"] = extension;
            let decoded = decode(
                &request,
                &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                json!([value]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert!(decoded.errors.is_empty());
            let wire = serde_json::to_value(&decoded.bids[0].bid).unwrap();
            assert_eq!(wire.pointer("/ext/prebid/meta"), expected.as_ref());
        }
    }

    #[test]
    fn source_video_metadata_uses_first_category_and_duration_in_existing_wire_channel() {
        let request = request(json!([imp("fake_imp", "video")]));
        let mut value = bid("fake_bid", "fake_imp");
        value["dur"] = json!(30_i32);
        value["cat"] = json!(["IAB20", "IAB1"]);
        value["ext"] = json!({"fake_keep":true});
        let decoded = decode(
            &request,
            &sent(&[("fake_imp", CorrelationMedia::Video)]),
            json!([value]),
        )
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        let wire = serde_json::to_value(&decoded.bids[0].bid).unwrap();
        assert_eq!(
            wire["ext"]["prebid"]["video"],
            json!({"duration":30_i32, "primary_category":"IAB20"})
        );
        assert_eq!(wire["ext"]["fake_keep"], true);
    }

    #[test]
    fn fresh_source_metadata_and_video_replace_stale_reserved_values() {
        let request = request(json!([imp("fake_imp", "banner")]));
        let mut value = bid("fake_bid", "fake_imp");
        value["ext"] = json!({"brand_id":"123", "fake_keep":true, "prebid":{
            "meta":{"networkId":999_i32, "advertiserId":888_i32, "brandId":777_i32, "fake_old":true},
            "video":{"duration":999_i32, "primary_category":"fake_old", "fake_old":true}
        }});
        let decoded = decode(
            &request,
            &sent(&[("fake_imp", CorrelationMedia::Banner)]),
            json!([value]),
        )
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        let wire = serde_json::to_value(&decoded.bids[0].bid).unwrap();
        assert_eq!(wire["ext"]["prebid"]["meta"], json!({"brandId":123_i32}));
        assert_eq!(
            wire["ext"]["prebid"]["video"],
            json!({"duration":0_i32, "primary_category":""})
        );
        assert_eq!(wire["ext"]["fake_keep"], true);
    }

    #[test]
    fn absent_source_metadata_preserves_existing_meta_but_still_replaces_video() {
        let request = request(json!([imp("fake_imp", "banner")]));
        let mut value = bid("fake_bid", "fake_imp");
        value["ext"] = json!({"prebid":{
            "meta":{"networkId":999_i32, "advertiserId":888_i32},
            "video":{"duration":999_i32, "primary_category":"fake_old", "fake_old":true}
        }});
        let decoded = decode(
            &request,
            &sent(&[("fake_imp", CorrelationMedia::Banner)]),
            json!([value]),
        )
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        let wire = serde_json::to_value(&decoded.bids[0].bid).unwrap();
        assert_eq!(
            wire["ext"]["prebid"]["meta"],
            json!({"networkId":999_i32, "advertiserId":888_i32})
        );
        assert_eq!(
            wire["ext"]["prebid"]["video"],
            json!({"duration":0_i32, "primary_category":""})
        );
    }

    #[test]
    fn valid_sent_browser_configs_use_shared_channel_without_currency_or_identity_fabrication() {
        let request = request(json!([imp("fake_imp", "banner")]));
        let decoded = OpenxBidder.make_bids(&request, &sent(&[("fake_imp", CorrelationMedia::Banner)]), &response(&json!({
            "id":"fake_response", "cur":"EUR", "ext":{"fledge_auction_configs":{
                "fake_imp":{"seller":"fake_seller", "bidder":"fake_forged", "adapter":"fake_forged"}
            }}
        }))).unwrap();
        assert!(decoded.bids.is_empty());
        assert_eq!(decoded.currency, "EUR");
        assert_eq!(decoded.auction_configs.len(), 1);
        assert_eq!(decoded.auction_configs[0].imp_id, "fake_imp");
        assert_eq!(
            decoded.auction_configs[0].config,
            json!({"seller":"fake_seller", "bidder":"fake_forged", "adapter":"fake_forged"})
        );
    }

    #[test]
    fn browser_config_map_has_stable_key_order_and_preserves_null_payload() {
        let request = request(json!([imp("fake_a", "banner"), imp("fake_z", "banner")]));
        let decoded = OpenxBidder.make_bids(&request, &sent(&[("fake_a", CorrelationMedia::Banner), ("fake_z", CorrelationMedia::Banner)]), &response(&json!({
            "id":"fake_response", "ext":{"FLEDGE_AUCTION_CONFIGS":{"fake_z":{}, "fake_a":null}}
        }))).unwrap();
        assert_eq!(decoded.auction_configs.len(), 2);
        assert_eq!(decoded.auction_configs[0].imp_id, "fake_a");
        assert!(decoded.auction_configs[0].config.is_null());
        assert_eq!(decoded.auction_configs[1].imp_id, "fake_z");
    }

    #[test]
    fn valid_browser_configs_survive_all_bad_bid_siblings() {
        let request = request(json!([imp("fake_imp", "banner")]));
        let mut unsupported = bid("fake_bad_media", "fake_imp");
        unsupported["mtype"] = json!(2_i32);
        let decoded = OpenxBidder
            .make_bids(
                &request,
                &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                &response(
                    &json!({"id":"fake_response", "cur":"EUR", "seatbid":[{"bid":[
                unsupported, bid("fake_unknown", "fake_unsent")
            ]}], "ext":{"fledge_auction_configs":{"fake_imp":{"fake_keep":true}}}}),
                ),
            )
            .unwrap();
        assert!(decoded.bids.is_empty());
        assert_eq!(decoded.errors.len(), 2);
        assert!(decoded.errors.iter().all(|error| error.code == 4));
        assert_eq!(decoded.currency, "EUR");
        assert_eq!(decoded.auction_configs.len(), 1);
        assert_eq!(decoded.auction_configs[0].imp_id, "fake_imp");
        assert_eq!(decoded.auction_configs[0].config, json!({"fake_keep":true}));
    }

    #[test]
    fn repeated_mixed_status_calls_skip_204_parsing_and_never_retain_prior_configs() {
        let request = request(json!([imp("fake_imp", "banner")]));
        let sent = sent(&[("fake_imp", CorrelationMedia::Banner)]);
        for status in [200, 204, 400, 500, 204, 200] {
            let http = if status == 200 {
                response(
                    &json!({"id":"fake_response", "ext":{"fledge_auction_configs":{
                        "fake_imp":{"fake_keep":true}
                    }}}),
                )
            } else {
                // Invalid lexical bytes and a tiny node limit must never be parsed for 204.
                harness::budgeted_response(status, vec![b'{']).within_node_budget(1)
            };
            let result = OpenxBidder.make_bids(&request, &sent, &http);
            match status {
                200 => {
                    let decoded = result.unwrap();
                    assert!(decoded.bids.is_empty());
                    assert_eq!(decoded.auction_configs.len(), 1);
                }
                204 => {
                    let decoded = result.unwrap();
                    assert!(decoded.bids.is_empty());
                    assert!(decoded.errors.is_empty());
                    assert!(decoded.auction_configs.is_empty());
                }
                _ => {
                    let errors = result.unwrap_err();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
                    assert_eq!(errors[0].kind, BidderErrorKind::Other);
                }
            }
        }
    }

    #[test]
    fn malformed_or_ambiguous_optional_browser_map_does_not_remove_ordinary_bids() {
        for extension in [
            json!({"fledge_auction_configs":[]}),
            json!({"fledge_auction_configs":true}),
            json!({"fledge_auction_configs":{}, "FLEDGE_AUCTION_CONFIGS":{}}),
        ] {
            let request = request(json!([imp("fake_imp", "banner")]));
            let decoded = OpenxBidder.make_bids(&request, &sent(&[("fake_imp", CorrelationMedia::Banner)]), &response(&json!({
                "id":"fake_response", "seatbid":[{"bid":[bid("fake_bid", "fake_imp")]}], "ext":extension
            }))).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert!(decoded.auction_configs.is_empty());
        }
    }

    #[test]
    fn sealed_malformed_input_and_node_budget_failure_keep_shared_parse_provenance() {
        // Lexical malformed control, not an ordinary JSON fixture.
        let malformed = vec![b'{'];
        let request = request(json!([]));
        for (response, code) in [
            (harness::budgeted_response(200, malformed), 14),
            (
                response(&
                    json!({"id":"fake_response", "ext":{"fledge_auction_configs":{"fake_imp":{}}}}),
                )
                .within_node_budget(1),
                4,
            ),
        ] {
            let errors = OpenxBidder
                .make_bids(&request, &sent(&[]), &response)
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, code);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        }
    }

    #[test]
    fn inherited_decoder_required_null_and_nonobject_ext_boundaries_remain_explicit() {
        for value in [
            json!({}),
            json!({"id":null}),
            json!({"id":"fake_response", "cur":null}),
            json!({"id":"fake_response", "seatbid":null}),
            json!({"id":"fake_response", "seatbid":[{}]}),
            json!({"id":"fake_response", "seatbid":[{"bid":[{"id":"fake_bid", "impid":"fake_imp", "price":1_i32, "ext":[]}]}]}),
        ] {
            let errors = response(&value).parse_bid_response().unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        }
    }

    #[test]
    fn inherited_response_ext_null_presence_loss_remains_explicit() {
        let parsed = response(&json!({"id":"fake_response", "seatbid":[{"bid":[{
            "id":"fake_bid", "impid":"fake_imp", "price":1_i32,
            "ext":null, "EXT":null
        }]}]}))
        .parse_bid_response()
        .unwrap();
        assert!(parsed.seatbid[0].bid[0].ext.is_none());
        assert!(!parsed.seatbid[0].bid[0].other.contains_key("ext"));
        assert_eq!(
            parsed.seatbid[0].bid[0].other.get("EXT"),
            Some(&Value::Null)
        );
    }

    #[test]
    fn typed_bid_ext_failure_keeps_valid_siblings_and_configs() {
        let request = request(json!([imp("fake_imp", "banner")]));
        for extension in [
            json!({"origbidcpm":"invalid"}),
            json!({"origbidcur":{}}),
            json!({"prebid":{"type":{}}}),
            json!({"prebid":{"targeting":{"fake_key":7_i32}}}),
            json!({"prebid":{"dealpriority":false}}),
        ] {
            let mut bad = bid("fake_bad", "fake_imp");
            bad["ext"] = extension;
            let decoded = OpenxBidder
                .make_bids(
                    &request,
                    &sent(&[("fake_imp", CorrelationMedia::Banner)]),
                    &response(&json!({"id":"fake_response", "seatbid":[{"bid":[
                    bid("fake_good", "fake_imp"), bad
                ]}], "ext":{"fledge_auction_configs":{"fake_imp":{}}}})),
                )
                .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "fake_good");
            assert_eq!(decoded.auction_configs.len(), 1);
            let errors = decoded.errors;
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        }
    }
}

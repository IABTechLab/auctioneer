use super::parameters::{boolean, object, text, unique_field};
use crate::bidders::{
    media_type::{classify_bid, source_seat_bids},
    status::{bad_input_or_server, standard_status},
    wire_fields::{canonical_integer, signed_integer},
};
use crate::{
    BidderError, BidderHttpResponse, BidderResponse, BrowserAuctionConfig, OutboundRequestMeta,
    TypedBid,
};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
use serde_json::{Value, json};

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        bad_input_or_server(status, format!("status={status}"))
    })? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    let auction_configs = browser_configs(parsed.ext.take()).unwrap_or_default();
    let count = parsed.seatbid.iter().map(|seat| seat.bid.len()).sum();
    let mut decoded = BidderResponse {
        bids: Vec::with_capacity(count),
        currency: if parsed.cur.is_empty() {
            "USD".into()
        } else {
            parsed.cur
        },
        errors: parse_errors,
        auction_configs,
    };
    for (mut bid, source_seat) in parsed.seatbid.into_iter().flat_map(source_seat_bids) {
        match prepare_bid(request, sent, response, &mut bid) {
            Ok(media_type) => decoded.bids.push(TypedBid {
                bid,
                media_type,
                source_seat,
            }),
            Err(error) => decoded.errors.push(error),
        }
    }
    if decoded.bids.is_empty() && decoded.auction_configs.is_empty() && !decoded.errors.is_empty() {
        Err(decoded.errors)
    } else {
        Ok(decoded)
    }
}

fn invalid_bid() -> BidderError {
    BidderError::bad_server_response("invalid pubmatic bid")
}

fn prepare_bid(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
    bid: &mut Bid,
) -> Result<MediaType, BidderError> {
    if !sent.covered_imp_ids().any(|id| id == bid.impid) {
        return Err(BidderError::bad_server_response(
            "bid response references an unsent impression",
        ));
    }
    let markup = canonical_integer(&mut bid.other, "mtype").map_err(|_error| invalid_bid())?;
    if !matches!(markup, 1 | 2 | 4) {
        return Err(invalid_bid());
    }
    let media_type = classify_bid(request, bid, None)?;
    if let Some(categories) = bid.other.get_mut("cat") {
        match categories {
            Value::Array(values) => {
                if !values.iter().all(Value::is_string) {
                    return Err(invalid_bid());
                }
                values.truncate(1);
            }
            Value::Null => {}
            Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Object(_) => {
                return Err(invalid_bid());
            }
        }
    }
    let metadata = response_metadata(bid).map_err(|()| invalid_bid())?;
    if media_type == MediaType::Native {
        response.unwrap_native_adm(bid.adm.as_mut().ok_or_else(invalid_bid)?)?;
    }
    let ext = bid.ext.get_or_insert_default();
    let prebid = ext.prebid.get_or_insert_default();
    prebid.other.insert(
        "video".into(),
        json!({"duration":metadata.duration,"primary_category":""}),
    );
    if metadata.present {
        prebid.meta =
            Some(json!({"mediaType":if metadata.ibv { MediaType::Video } else { media_type }}));
    }
    prebid.dealpriority = (metadata.priority > 0).then_some(metadata.priority);
    Ok(media_type)
}

#[derive(Default)]
struct ResponseMetadata {
    present: bool,
    duration: i64,
    priority: i64,
    ibv: bool,
}

fn response_metadata(bid: &Bid) -> Result<ResponseMetadata, ()> {
    let Some(ext) = &bid.ext else {
        return Ok(ResponseMetadata::default());
    };
    let _marketplace = text(unique_field(&ext.other, "marketplace")?)?;
    let video = object(unique_field(&ext.other, "video")?)?;
    let duration = video.map_or(Ok(0), |fields| {
        signed_integer(fields, "duration").map_err(|_error| ())
    })?;
    Ok(ResponseMetadata {
        present: true,
        duration,
        priority: signed_integer(&ext.other, "prebiddealpriority").map_err(|_error| ())?,
        ibv: boolean(unique_field(&ext.other, "ibv")?)?,
    })
}

fn browser_configs(ext: Option<Value>) -> Result<Vec<BrowserAuctionConfig>, ()> {
    let Some(Value::Object(fields)) = ext else {
        return Ok(Vec::new());
    };
    let mut selected = fields
        .into_iter()
        .filter(|(key, _value)| key.eq_ignore_ascii_case("fledge_auction_configs"));
    let Some((_key, value)) = selected.next() else {
        return Ok(Vec::new());
    };
    if selected.next().is_some() {
        return Err(());
    }
    match value {
        Value::Object(configs) => {
            let mut output: Vec<_> = configs
                .into_iter()
                .map(|(imp_id, config)| BrowserAuctionConfig { imp_id, config })
                .collect();
            output.sort_unstable_by(|left, right| left.imp_id.cmp(&right.imp_id));
            Ok(output)
        }
        Value::Null => Ok(Vec::new()),
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => Err(()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::PubmaticBidder;
    use crate::bidders::harness;
    use crate::{Bidder as _, BidderError, BidderErrorKind, BidderHttpResponse, BidderResponse};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::sync::Arc;

    fn request() -> BidRequest {
        serde_json::from_value(json!({
            "id":"r", "imp":[{"id":"i", "banner":{}, "video":{}, "native":{}, "audio":{}}],
        }))
        .unwrap()
    }

    fn response(body: &Value) -> BidderHttpResponse {
        harness::budgeted_response(200, serde_json::to_vec(body).unwrap())
    }

    fn decode(body: &Value) -> Result<BidderResponse, Vec<BidderError>> {
        PubmaticBidder.make_bids(
            &request(),
            &harness::sent_meta_with_ids("https://bidder.example.test/translator", ["i"]),
            &response(body),
        )
    }

    fn body(bids: Value) -> Value {
        let mut result = json!({"id":"r", "seatbid":[{"bid":[]}]});
        result["seatbid"][0]["bid"] = bids;
        result
    }

    fn bid(fields: Value) -> Value {
        let Value::Object(extra_fields) = fields else {
            panic!("bid fields must be an object");
        };
        let mut value = json!({"id":"b", "impid":"i", "price":1.25_f64, "mtype":1_i32});
        value.as_object_mut().unwrap().extend(extra_fields);
        value
    }

    #[test]
    fn source_seat_retains_protocol_provenance_and_shares_only_within_each_seat() {
        let value = json!({"id":"r", "seatbid":[
            {"bid":[bid(json!({"id":"missing"}))]},
            {"seat":"", "bid":[bid(json!({"id":"empty"}))]},
            {"seat":"upstream_a", "bid":[bid(json!({"id":"before"})),
                bid(json!({"id":"bad", "mtype":0_i32})), bid(json!({"id":"after"}))]},
            {"seat":"upstream_b", "bid":[bid(json!({"id":"other"}))]}
        ]});
        assert_ne!(value["seatbid"][2]["seat"], json!("fake_alias"));
        let first = decode(&value).unwrap();
        let second = decode(&value).unwrap();
        assert_eq!(first.errors.len(), 1);
        assert_eq!(first.bids.len(), 5);
        assert_eq!(
            first
                .bids
                .iter()
                .map(|bid| bid.bid.id.as_str())
                .collect::<Vec<_>>(),
            ["missing", "empty", "before", "after", "other"]
        );
        assert_eq!(
            first
                .bids
                .iter()
                .map(|bid| bid.source_seat.as_deref())
                .collect::<Vec<_>>(),
            [
                None,
                None,
                Some("upstream_a"),
                Some("upstream_a"),
                Some("upstream_b")
            ]
        );
        let owner = first.bids[2].source_seat.as_ref().unwrap();
        assert!(Arc::ptr_eq(
            owner,
            first.bids[3].source_seat.as_ref().unwrap()
        ));
        assert_eq!(Arc::strong_count(owner), 2);
        assert!(!Arc::ptr_eq(
            owner,
            first.bids[4].source_seat.as_ref().unwrap()
        ));
        assert!(!Arc::ptr_eq(
            owner,
            second.bids[2].source_seat.as_ref().unwrap()
        ));
    }

    #[test]
    fn standard_media_partitions_invalid_bids_across_seats() {
        let response = decode(&json!({"id":"r", "seatbid":[
            {"bid":[bid(json!({"id":"bad","mtype":0_i32})),bid(json!({"id":"video","mtype":2_i32}))]},
            {"bid":[
                bid(json!({"id":"audio","mtype":3_i32})),
                bid(json!({"id":"native","mtype":4_i32,"adm":json!({"assets":[]}).to_string()})),
                bid(json!({"id":"conflict","mtype":1_i32,"ext":{"prebid":{"type":"video"}}})),
            ]},
        ]})).unwrap();
        assert_eq!(response.bids.len(), 2);
        assert_eq!(response.bids[0].bid.id, "video");
        assert_eq!(response.bids[0].media_type, MediaType::Video);
        assert_eq!(response.bids[1].bid.id, "native");
        assert_eq!(response.bids[1].media_type, MediaType::Native);
        harness::assert_bad_server_response_errors(&response.errors, 3);
    }

    #[test]
    fn missing_standard_media_never_falls_back_to_legacy_hints_or_inventory() {
        for fields in [
            json!({}),
            json!({"ext":{"video":{"duration":15_i32}}}),
            json!({"ext":{"prebid":{"mediatype":"native"}}}),
            json!({"ext":{"prebid":{"type":"video","mediatype":"video"}}}),
        ] {
            let mut candidate = bid(fields);
            candidate.as_object_mut().unwrap().remove("mtype").unwrap();
            let errors = decode(&body(json!([candidate]))).unwrap_err();
            harness::assert_bad_server_response_errors(&errors, 1);
        }
    }

    #[test]
    fn invalid_and_ambiguous_standard_media_are_bid_local() {
        for fields in [
            json!({"mtype":null}),
            json!({"mtype":0_i32}),
            json!({"mtype":5_i32}),
            json!({"mtype":-1_i32}),
            json!({"mtype":"1"}),
            json!({"mtype":1.0_f64}),
            json!({"mtype":1_i32,"MTYPE":1_i32}),
            json!({"mtype":1_i32,"MTYPE":null}),
        ] {
            let response = decode(&body(json!([bid(fields), bid(json!({"id":"good"})),]))).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].bid.id, "good");
            harness::assert_bad_server_response_errors(&response.errors, 1);
        }
        for key in ["mtype", "Mtype", "MTYPE"] {
            let mut candidate = bid(json!({}));
            candidate.as_object_mut().unwrap().remove("mtype").unwrap();
            candidate[key] = json!(2_i32);
            assert_eq!(
                decode(&body(json!([candidate]))).unwrap().bids[0].media_type,
                MediaType::Video
            );
        }
    }

    #[test]
    fn response_status_precedes_any_body_decode() {
        let request = BidRequest::default();
        let sent = harness::sent_meta("https://bidder.example.test/translator");
        for status in [204_u16, 400, 201, 302, 500] {
            for bytes in [Vec::new(), b"not JSON".to_vec()] {
                let result = PubmaticBidder.make_bids(
                    &request,
                    &sent,
                    &harness::budgeted_response(status, bytes),
                );
                if status == 204 {
                    let decoded = result.unwrap();
                    assert!(decoded.bids.is_empty());
                    assert!(decoded.errors.is_empty());
                    assert!(decoded.auction_configs.is_empty());
                    assert_eq!(decoded.currency, "USD");
                } else {
                    let errors = result.unwrap_err();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
                    assert_ne!(errors[0].kind, BidderErrorKind::ResponseParse);
                }
            }
        }
        let errors = PubmaticBidder
            .make_bids(
                &request,
                &sent,
                &harness::budgeted_response(200, Vec::new()),
            )
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 14);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
    }

    #[test]
    fn metadata_maps_duration_priority_and_ibv_without_changing_bid_format() {
        for (priority, expected_priority) in [
            (None, None),
            (Some(json!(null)), None),
            (Some(json!(0_i64)), None),
            (Some(json!(-1_i64)), None),
            (Some(json!(7_i64)), Some(7_i64)),
        ] {
            let mut metadata = json!({
                "marketplace":"fake_provider", "video":{"duration":25_i64}, "ibv":true,
                "unknown":{"keep":true}, "prebid":{"dealpriority":99_i64},
            });
            if let Some(value) = priority {
                metadata["prebiddealpriority"] = value;
            }
            let response = decode(&body(json!([bid(json!({
                "cat":["IAB1","IAB2"], "ext":metadata,
            }))])))
            .unwrap();
            assert!(response.errors.is_empty());
            let actual = &response.bids[0];
            assert_eq!(actual.media_type, MediaType::Banner);
            assert_eq!(actual.bid.other["cat"], json!(["IAB1"]));
            let ext = actual.bid.ext.as_ref().unwrap();
            assert_eq!(ext.other["marketplace"], json!("fake_provider"));
            assert_eq!(ext.other["unknown"], json!({"keep":true}));
            let prebid = ext.prebid.as_ref().unwrap();
            assert_eq!(prebid.meta, Some(json!({"mediaType":"video"})));
            assert_eq!(
                prebid.other["video"],
                json!({"duration":25_i64,"primary_category":""})
            );
            assert_eq!(prebid.dealpriority, expected_priority);
            assert!(prebid.targetbiddercode.is_none());
        }
    }

    #[test]
    fn absent_metadata_has_default_video_without_fabricated_media_metadata() {
        for extension in [None, Some(json!({})), Some(json!({"ibv":false}))] {
            let mut candidate = bid(json!({}));
            if let Some(value) = extension.clone() {
                candidate["ext"] = value;
            }
            let response = decode(&body(json!([candidate]))).unwrap();
            let prebid = response.bids[0]
                .bid
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap();
            assert_eq!(
                prebid.other["video"],
                json!({"duration":0_i32,"primary_category":""})
            );
            assert_eq!(
                prebid.meta,
                extension.map(|_value| json!({"mediaType":"banner"}))
            );
            assert!(prebid.dealpriority.is_none());
        }
    }

    #[test]
    fn malformed_native_creatives_drop_only_the_affected_bid() {
        for adm in [
            None,
            Some("{".to_owned()),
            Some(json!({"native":null}).to_string()),
        ] {
            let mut candidate = bid(json!({"id":"bad","mtype":4_i32}));
            if let Some(value) = adm {
                candidate["adm"] = json!(value);
            }
            let response = decode(&body(json!([
                candidate,
                bid(json!({"id":"good","mtype":4_i32,"adm":json!({"native":{"assets":[]}}).to_string()})),
            ]))).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].bid.id, "good");
            assert_eq!(
                response.bids[0].bid.adm,
                Some(json!({"assets":[]}).to_string())
            );
            harness::assert_bad_server_response_errors(&response.errors, 1);
        }
    }

    #[test]
    fn browser_configs_preserve_values_without_becoming_bidder_identities() {
        let payload =
            json!({"seller":"https://fake_provider.test","nested":[true,null,{"keep":1_i32}]});
        let decoded = decode(&json!({
            "id":"r", "cur":"EUR", "ext":{"fledge_auction_configs":{"i":payload}},
        }))
        .unwrap();
        assert!(decoded.bids.is_empty());
        assert!(decoded.errors.is_empty());
        assert_eq!(decoded.currency, "EUR");
        assert_eq!(decoded.auction_configs.len(), 1);
        assert_eq!(decoded.auction_configs[0].imp_id, "i");
        assert_eq!(decoded.auction_configs[0].config, payload);
    }

    #[test]
    fn all_bad_bids_keep_browser_configs_but_never_become_silent_no_bid() {
        let mut input = body(json!([
            bid(json!({"mtype":0_i32})),
            bid(json!({"mtype":5_i32}))
        ]));
        let errors = decode(&input).unwrap_err();
        harness::assert_bad_server_response_errors(&errors, 2);
        input["ext"] = json!({"fledge_auction_configs":{"i":{"keep":true}}});
        let decoded = decode(&input).unwrap();
        assert!(decoded.bids.is_empty());
        harness::assert_bad_server_response_errors(&decoded.errors, 2);
        assert_eq!(decoded.auction_configs.len(), 1);
    }

    #[test]
    fn invalid_browser_config_shapes_do_not_discard_valid_sibling_bids() {
        for extension in [
            json!(null),
            json!(false),
            json!([]),
            json!({"fledge_auction_configs":null}),
            json!({"fledge_auction_configs":[]}),
            json!({"fledge_auction_configs":true}),
            json!({"fledge_auction_configs":{"i":{}},"FLEDGE_AUCTION_CONFIGS":{"i":{}}}),
        ] {
            let mut input = body(json!([bid(json!({"id":"good"}))]));
            input["ext"] = extension;
            let decoded = decode(&input).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "good");
            assert!(decoded.errors.is_empty());
            assert!(decoded.auction_configs.is_empty());
        }
        for key in ["fledge_auction_configs", "FLEDGE_AUCTION_CONFIGS"] {
            let decoded = decode(&json!({"id":"r","ext":{key:{"i":{"keep":true}}}})).unwrap();
            assert_eq!(decoded.auction_configs.len(), 1);
            assert_eq!(decoded.auction_configs[0].imp_id, "i");
            assert_eq!(decoded.auction_configs[0].config, json!({"keep":true}));
        }
    }

    #[test]
    fn malformed_recognized_extensions_are_bid_local_without_echoing_values() {
        for extension in [
            json!({"marketplace":[]}),
            json!({"prebiddealpriority":"secret"}),
            json!({"prebiddealpriority":1_i32,"PREBIDDEALPRIORITY":1_i32}),
            json!({"video":true}),
            json!({"video":{"duration":1_i32,"DURATION":1_i32}}),
            json!({"video":{"duration":"secret"}}),
            json!({"ibv":"secret"}),
            json!({"ibv":true,"IBV":false}),
        ] {
            let decoded = decode(&body(json!([
                bid(json!({"ext":extension})),
                bid(json!({"id":"good"})),
            ])))
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "good");
            harness::assert_bad_server_response_errors(&decoded.errors, 1);
            assert!(!decoded.errors[0].message.contains("secret"));
        }
    }
}

use crate::bidders::{
    media_type::classify_bid,
    status::{bad_input_or_server, standard_status},
};
use crate::{
    BidderError, BidderHttpResponse, BidderResponse, BrowserAuctionConfig, OutboundRequestMeta,
    TypedBid,
};
use auctioneer_openrtb::{
    ext::bid::ExtBidPrebid,
    types::{bid_request::BidRequest, bid_response::Bid},
};
use serde_json::{Map, Value};

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), |status| {
        bad_input_or_server(
            status,
            format!("Unexpected status code: {status}. Run with request.debug = 1 for more info."),
        )
    })? {
        return Ok(no_bid);
    }

    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    let auction_configs = extract_configs(parsed.ext.take()).unwrap_or_default();
    let bid_count = parsed.seatbid.iter().map(|seat| seat.bid.len()).sum();
    let mut decoded = BidderResponse {
        bids: Vec::with_capacity(bid_count),
        currency: parsed.cur,
        errors: parse_errors,
        auction_configs,
    };
    for mut bid in parsed.seatbid.into_iter().flat_map(|seat| seat.bid) {
        let classified = prepare_bid(&mut bid).and_then(|()| {
            if !sent
                .covered_imp_ids()
                .any(|id| !id.is_empty() && id == bid.impid)
            {
                return Err(BidderError::bad_server_response(
                    "bid response references an unsent impression",
                ));
            }
            classify_bid(request, &bid, None)
        });
        match classified {
            Ok(media_type) => decoded.bids.push(TypedBid { bid, media_type }),
            Err(error) => decoded.errors.push(error),
        }
    }
    if decoded.bids.is_empty() && decoded.auction_configs.is_empty() && !decoded.errors.is_empty() {
        return Err(decoded.errors);
    }
    Ok(decoded)
}

fn known<'a>(fields: &'a Map<String, Value>, name: &str) -> Result<Option<&'a Value>, ()> {
    let mut matching = fields
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let selected = matching.next().map(|(_, value)| value);
    if matching.next().is_some() {
        return Err(());
    }
    Ok(selected)
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

fn invalid_extension() -> BidderError {
    BidderError::bad_server_response("invalid criteo bid extension")
}

fn prepare_bid(bid: &mut Bid) -> Result<(), BidderError> {
    let ext = bid.ext.as_mut().ok_or_else(invalid_extension)?;
    if let Some(raw) = take_known(&mut ext.other, "prebid").map_err(|()| invalid_extension())? {
        if ext.prebid.is_some() {
            return Err(invalid_extension());
        }
        let Value::Object(fields) = raw else {
            return Err(invalid_extension());
        };
        let _: Option<&Value> = known(&fields, "type").map_err(|()| invalid_extension())?;
        let _: Option<&Value> = known(&fields, "networkName").map_err(|()| invalid_extension())?;
        // Convert the already budgeted owned tree, never the sealed response bytes.
        ext.prebid = Some(
            serde_json::from_value::<ExtBidPrebid>(Value::Object(fields))
                .map_err(|_error| invalid_extension())?,
        );
    }
    let prebid = ext.prebid.as_mut().ok_or_else(invalid_extension)?;
    if let Some(value) = take_known(&mut prebid.other, "type").map_err(|()| invalid_extension())? {
        if prebid.bid_type.is_some() {
            return Err(invalid_extension());
        }
        prebid.bid_type = match value {
            Value::String(name) => Some(name),
            Value::Null => None,
            Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => {
                return Err(invalid_extension());
            }
        };
    }
    if prebid.bid_type.is_none() {
        return Err(invalid_extension());
    }
    let network = known(&prebid.other, "networkName").map_err(|()| invalid_extension())?;
    let name = match network {
        None | Some(Value::Null) => return Ok(()),
        Some(Value::String(name)) if name.is_empty() => return Ok(()),
        Some(Value::String(name)) => name.to_owned(),
        Some(_) => return Err(invalid_extension()),
    };
    let meta = prebid.meta.get_or_insert_with(|| Value::Object(Map::new()));
    let fields = meta.as_object_mut().ok_or_else(invalid_extension)?;
    fields.insert("networkName".into(), Value::String(name));
    Ok(())
}

fn extract_configs(ext: Option<Value>) -> Result<Vec<BrowserAuctionConfig>, ()> {
    let Some(Value::Object(mut root)) = ext else {
        return Ok(Vec::new());
    };
    let Some(igi) = take_known(&mut root, "igi")? else {
        return Ok(Vec::new());
    };
    let Value::Array(groups) = igi else {
        return Err(());
    };
    let mut configs = Vec::with_capacity(groups.len());
    for group_value in groups {
        let Value::Object(mut group) = group_value else {
            return Err(());
        };
        let imp_id = match take_known(&mut group, "impid")? {
            Some(Value::String(id)) => id,
            None | Some(Value::Null) => String::new(),
            Some(_) => return Err(()),
        };
        let igs = match take_known(&mut group, "igs")? {
            None | Some(Value::Null) => continue,
            Some(Value::Array(igs)) => igs,
            Some(_) => return Err(()),
        };
        let mut first = None;
        // Validate every group member before publishing any optional result.
        for (index, ig_value) in igs.into_iter().enumerate() {
            let Value::Object(mut ig) = ig_value else {
                return Err(());
            };
            let config = take_known(&mut ig, "config")?;
            if index == 0 {
                first = config;
            }
        }
        if let Some(config) = first {
            configs.push(BrowserAuctionConfig { imp_id, config });
        }
    }
    Ok(configs)
}

#[cfg(test)]
mod tests {
    use super::super::CriteoBidder;
    use crate::bidders::harness;
    use crate::{Bidder as _, BidderError, BidderErrorKind, BidderHttpResponse, BidderResponse};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    const ENDPOINT: &str = "https://bidder.example.test/openrtb?profile=230";

    fn request() -> BidRequest {
        serde_json::from_value(json!({
            "id":"request", "imp":[
                {"id":"i", "banner":{"w":300_i32,"h":250_i32},
                 "ext":{"bidder":{"zoneId":123_i32},"preserve":"imp"}},
                {"id":"v", "video":{"mimes":["video/mp4"]}},
            ],
            "ext":{"preserve":{"privacy_scrubbed":true}},
        }))
        .unwrap()
    }

    fn bid(id: &str, impid: &str, media: &str) -> Value {
        json!({"id":id,"impid":impid,"price":1.25_f64,
               "ext":{"prebid":{"type":media}}})
    }

    fn body(bids: Vec<Value>, ext: Value) -> Value {
        let mut response = json!({"id":"response","cur":"USD","seatbid":[{}]});
        response["seatbid"][0]["bid"] = Value::Array(bids);
        response["ext"] = ext;
        response
    }

    fn config() -> Value {
        json!({"igi":[{"impid":"i","igs":[{"config":{"seller":"seller.test"}}]}]})
    }

    fn response(value: &Value) -> BidderHttpResponse {
        harness::budgeted_response(200, serde_json::to_vec(value).unwrap())
    }

    fn decode(value: &Value) -> Result<BidderResponse, Vec<BidderError>> {
        CriteoBidder.make_bids(
            &request(),
            &harness::sent_meta_with_ids(ENDPOINT, ["i", "v"]),
            &response(value),
        )
    }

    fn codes(errors: &[BidderError]) -> Vec<u32> {
        errors.iter().map(|error| error.code).collect()
    }

    #[test]
    fn no_content_ignores_body_and_returns_no_bid_or_config() {
        let response = CriteoBidder
            .make_bids(
                &request(),
                &harness::sent_meta_with_ids(ENDPOINT, ["i"]),
                &harness::budgeted_response(204, b"not JSON".to_vec()),
            )
            .unwrap();
        assert!(response.bids.is_empty());
        assert!(response.errors.is_empty());
        assert!(response.auction_configs.is_empty());
    }

    #[test]
    fn status_errors_have_source_codes_and_fixed_redacted_messages() {
        for status in [400, 401, 403, 500, 503] {
            let errors = CriteoBidder
                .make_bids(
                    &request(),
                    &harness::sent_meta_with_ids(ENDPOINT, ["i"]),
                    &harness::budgeted_response(status, b"PROVIDER_SECRET".to_vec()),
                )
                .unwrap_err();
            assert_eq!(codes(&errors), [if status == 400 { 2 } else { 4 }]);
            assert_eq!(
                errors[0].message,
                format!(
                    "Unexpected status code: {status}. Run with request.debug = 1 for more info."
                )
            );
            assert!(!format!("{errors:?}").contains("PROVIDER_SECRET"));
        }
    }

    #[test]
    fn currency_is_preserved_verbatim_including_missing_and_empty() {
        for currency in [None, Some(""), Some("EUR"), Some("usd")] {
            let mut value = body(vec![bid("b", "i", "banner")], config());
            if let Some(wire_currency) = currency {
                value["cur"] = json!(wire_currency);
            } else {
                value.as_object_mut().unwrap().remove("cur");
            }
            let response = decode(&value).unwrap();
            assert_eq!(response.currency, currency.unwrap_or_default());
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.auction_configs.len(), 1);
        }
    }

    #[test]
    fn explicit_media_selects_the_offered_format() {
        let mut request = request();
        request.imp[0].video = request.imp[1].video.take();
        for (name, expected) in [("banner", MediaType::Banner), ("video", MediaType::Video)] {
            let response = CriteoBidder
                .make_bids(
                    &request,
                    &harness::sent_meta_with_ids(ENDPOINT, ["i"]),
                    &harness::budgeted_response(
                        200,
                        serde_json::to_vec(&body(vec![bid("b", "i", name)], json!({}))).unwrap(),
                    ),
                )
                .unwrap();
            assert_eq!(response.bids[0].media_type, expected);
        }
    }

    #[test]
    fn network_name_merges_into_existing_prebid_meta_without_losing_fields() {
        let mut value = bid("b", "i", "banner");
        value["ext"]["prebid"]["networkName"] = json!("Criteo");
        value["ext"]["prebid"]["meta"] = json!({"advertiserDomains":["advertiser.test"]});
        let response = decode(&body(vec![value], json!({}))).unwrap();
        let output = serde_json::to_value(&response.bids[0].bid).unwrap();
        assert_eq!(
            output["ext"]["prebid"]["meta"],
            json!({
                "networkName":"Criteo", "advertiserDomains":["advertiser.test"],
            })
        );
    }

    #[test]
    fn absent_or_empty_network_name_does_not_fabricate_meta() {
        for name in [None, Some("")] {
            let mut value = bid("b", "i", "banner");
            if let Some(network_name) = name {
                value["ext"]["prebid"]["networkName"] = json!(network_name);
            }
            let response = decode(&body(vec![value], json!({}))).unwrap();
            let output = serde_json::to_value(&response.bids[0].bid).unwrap();
            assert!(output["ext"]["prebid"].get("meta").is_none());
        }
    }

    #[test]
    fn bad_media_and_unsent_bids_preserve_valid_sibling_and_browser_config() {
        let response = decode(&body(
            vec![
                bid("bad-type", "i", "unsupported"),
                bid("unoffered", "v", "banner"),
                bid("unsent", "forged", "banner"),
                bid("valid", "i", "banner"),
            ],
            config(),
        ))
        .unwrap();
        assert_eq!(response.bids.len(), 1);
        assert_eq!(response.bids[0].bid.id, "valid");
        assert_eq!(codes(&response.errors), [4, 4, 4]);
        assert_eq!(response.auction_configs.len(), 1);
    }

    #[test]
    fn configs_survive_even_when_every_ordinary_bid_is_invalid() {
        let response = decode(&body(vec![bid("bad", "i", "unsupported")], config())).unwrap();
        assert!(response.bids.is_empty());
        assert_eq!(codes(&response.errors), [4]);
        assert_eq!(response.auction_configs.len(), 1);
    }

    #[test]
    fn request_membership_does_not_authorize_an_unsent_bid() {
        let errors = CriteoBidder
            .make_bids(
                &request(),
                &harness::sent_meta_with_ids(ENDPOINT, ["i"]),
                &harness::budgeted_response(
                    200,
                    serde_json::to_vec(&body(vec![bid("b", "v", "video")], json!({}))).unwrap(),
                ),
            )
            .unwrap_err();
        assert_eq!(codes(&errors), [4]);
    }

    #[test]
    fn sent_membership_does_not_authorize_an_unknown_bid() {
        let errors = CriteoBidder
            .make_bids(
                &request(),
                &harness::sent_meta_with_ids(ENDPOINT, ["forged"]),
                &harness::budgeted_response(
                    200,
                    serde_json::to_vec(&body(vec![bid("b", "forged", "banner")], json!({})))
                        .unwrap(),
                ),
            )
            .unwrap_err();
        assert_eq!(codes(&errors), [4]);
    }

    #[test]
    fn igi_config_only_output_keeps_group_order_and_first_igs_only() {
        let response = decode(&body(
            vec![],
            json!({"igi":[
                {"impid":"v","igs":[{"config":{"first":1_i32}},{"config":{"ignored":2_i32}}]},
                {"impid":"i","igs":[{"config":[1_i32,true,"opaque"]}]},
            ]}),
        ))
        .unwrap();
        assert!(response.bids.is_empty());
        assert!(response.errors.is_empty());
        assert_eq!(response.auction_configs.len(), 2);
        assert_eq!(response.auction_configs[0].imp_id, "v");
        assert_eq!(response.auction_configs[0].config, json!({"first":1_i32}));
        assert_eq!(response.auction_configs[1].imp_id, "i");
        assert_eq!(
            response.auction_configs[1].config,
            json!([1_i32, true, "opaque"])
        );
    }

    #[test]
    fn present_config_accepts_every_json_kind_including_explicit_null() {
        for config in [
            json!(null),
            json!(false),
            json!(7_i32),
            json!("opaque"),
            json!([]),
            json!({}),
        ] {
            let response = decode(&body(
                vec![],
                json!({"igi":[
                    {"impid":"i","igs":[{"config":config}]},
                ]}),
            ))
            .unwrap();
            assert_eq!(response.auction_configs.len(), 1);
            assert_eq!(response.auction_configs[0].config, config);
        }
    }

    #[test]
    fn missing_first_config_does_not_fall_back_to_later_igs() {
        for group in [
            json!({"impid":"i"}),
            json!({"impid":"i","igs":[]}),
            json!({"impid":"i","igs":[{}, {"config":{"later":true}}]}),
        ] {
            let response = decode(&body(vec![], json!({"igi":[group]}))).unwrap();
            assert!(response.auction_configs.is_empty());
        }
    }

    #[test]
    fn malformed_optional_shapes_ignore_all_configs_but_keep_ordinary_bids() {
        for ext in [
            json!(null),
            json!(3_i32),
            json!([]),
            json!({"igi":null}),
            json!({"igi":{}}),
            json!({"igi":[null]}),
            json!({"igi":[{"impid":3_i32,"igs":[]}]}),
            json!({"igi":[{"impid":"i","igs":{}}]}),
            json!({"igi":[{"impid":"i","igs":[null]}]}),
            json!({"igi":[{"impid":"i","igs":[{"config":1_i32}]},null]}),
        ] {
            let response = decode(&body(vec![bid("b", "i", "banner")], ext)).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert!(response.errors.is_empty());
            assert!(response.auction_configs.is_empty());
        }
    }

    #[test]
    fn equivalent_known_key_spellings_work_without_canonical_spelling() {
        let value = json!({"id":"b","impid":"i","price":1.25_f64,"ext":{
            "PREBID":{"TYPE":"banner","NETWORKNAME":"Criteo"},
        }});
        let response = decode(&body(
            vec![value],
            json!({"IGI":[
                {"IMPID":"i","IGS":[{"CONFIG":{"opaque":true}}]},
            ]}),
        ))
        .unwrap();
        assert_eq!(response.bids[0].media_type, MediaType::Banner);
        let output = serde_json::to_value(&response.bids[0].bid).unwrap();
        assert_eq!(output["ext"]["prebid"]["meta"]["networkName"], "Criteo");
        assert_eq!(response.auction_configs.len(), 1);
    }

    #[test]
    fn ambiguous_optional_keys_suppress_only_browser_configs() {
        for ext in [
            json!({"igi":[],"IGI":[]}),
            json!({"igi":null,"IGI":[]}),
            json!({"igi":[{"impid":"i","IMPID":"i","igs":[{"config":1_i32}]}]}),
            json!({"igi":[{"impid":"i","igs":[],"IGS":[]}]}),
            json!({"igi":[{"impid":"i","igs":[{"config":null,"CONFIG":null}]}]}),
        ] {
            let response = decode(&body(vec![bid("b", "i", "banner")], ext)).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert!(response.auction_configs.is_empty());
        }
    }

    #[test]
    fn ambiguous_bid_local_keys_preserve_valid_siblings_and_configs() {
        for ext in [
            json!({"prebid":{"type":"banner"},"PREBID":{"type":"banner"}}),
            json!({"prebid":{"type":"banner","TYPE":"banner"}}),
            json!({"prebid":{"type":"banner","networkName":"x","NETWORKNAME":"x"}}),
        ] {
            let mut bad = bid("bad", "i", "banner");
            bad["ext"] = ext;
            let response = decode(&body(vec![bad, bid("good", "i", "banner")], config())).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(response.bids[0].bid.id, "good");
            assert_eq!(codes(&response.errors), [4]);
            assert_eq!(response.auction_configs.len(), 1);
        }
    }

    #[test]
    fn null_or_missing_media_is_bid_local_and_does_not_guess_banner() {
        for ext in [
            json!(null),
            json!({}),
            json!({"prebid":null}),
            json!({"prebid":{}}),
            json!({"prebid":{"type":null}}),
        ] {
            let mut bad = bid("bad", "i", "banner");
            bad["ext"] = ext;
            let response = decode(&body(vec![bad, bid("good", "i", "banner")], config())).unwrap();
            assert_eq!(response.bids.len(), 1);
            assert_eq!(codes(&response.errors), [4]);
            assert_eq!(response.auction_configs.len(), 1);
        }
    }

    #[test]
    fn bad_network_name_is_bid_local_and_diagnostics_do_not_echo_values() {
        let mut bad = bid("IMPRESSION_SECRET", "i", "banner");
        bad["ext"]["prebid"]["networkName"] = json!({"secret":"NETWORK_SECRET"});
        let response = decode(&body(vec![bad, bid("good", "i", "banner")], config())).unwrap();
        assert_eq!(response.bids.len(), 1);
        assert_eq!(codes(&response.errors), [4]);
        let diagnostic = format!("{:?}", response.errors);
        assert!(!diagnostic.contains("IMPRESSION_SECRET"));
        assert!(!diagnostic.contains("NETWORK_SECRET"));
        assert_eq!(response.auction_configs.len(), 1);
    }

    #[test]
    fn malformed_envelope_retains_sealed_parse_provenance_and_redaction() {
        // Lexical malformed-byte control, not an ordinary JSON fixture.
        let errors = CriteoBidder
            .make_bids(
                &request(),
                &harness::sent_meta_with_ids(ENDPOINT, ["i"]),
                &harness::budgeted_response(200, b"PROVIDER_SECRET".to_vec()),
            )
            .unwrap_err();
        assert_eq!(codes(&errors), [14]);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(errors[0].message, "invalid bidder response");
    }

    #[test]
    fn malformed_canonical_typed_media_preserves_valid_siblings_and_configs() {
        let mut bad = bid("bad", "i", "banner");
        bad["ext"]["prebid"]["type"] = json!({"secret":"TYPE_SECRET"});
        let decoded = decode(&body(vec![bad, bid("good", "i", "banner")], config())).unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "good");
        assert_eq!(decoded.auction_configs.len(), 1);
        let errors = decoded.errors;
        assert_eq!(codes(&errors), [14]);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(errors[0].message, "invalid bidder response");
    }

    #[test]
    fn optional_config_nodes_still_obey_the_fatal_sealed_budget() {
        let bytes = serde_json::to_vec(&body(
            vec![bid("good", "i", "banner")],
            json!({"igi":[{"impid":"i","igs":[{"config":vec![0_i32; 128]}]}]}),
        ))
        .unwrap();
        let response = BidderHttpResponse::new(200, vec![], bytes).within_node_budget(32);
        let errors = CriteoBidder
            .make_bids(
                &request(),
                &harness::sent_meta_with_ids(ENDPOINT, ["i"]),
                &response,
            )
            .unwrap_err();
        assert_eq!(codes(&errors), [4]);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(
            errors[0].message,
            "bidder response exceeded the decode budget"
        );
    }

    #[test]
    fn opaque_provider_identity_is_not_promoted_to_the_result_record() {
        let payload = json!({"bidder":"FORGED_BIDDER","adapter":"FORGED_ADAPTER"});
        let response = decode(&body(
            vec![],
            json!({"igi":[{
                "impid":"i","bidder":"FORGED_GROUP","adapter":"FORGED_GROUP",
                "igs":[{"config":payload}],
            }]}),
        ))
        .unwrap();
        assert_eq!(response.auction_configs.len(), 1);
        assert_eq!(response.auction_configs[0].imp_id, "i");
        assert_eq!(response.auction_configs[0].config, payload);
        let diagnostic = format!("{:?}", response.auction_configs);
        assert!(!diagnostic.contains("FORGED"));
    }
}

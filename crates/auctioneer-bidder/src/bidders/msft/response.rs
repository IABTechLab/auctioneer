use super::parameters::{integer, object, unique};
use crate::bidders::{
    media_type::{classify_bid, classify_response_with_errors},
    status::{debug_status_error, standard_status},
    wire_fields::{canonical_integer, signed_byte},
};
use crate::response_decode::CandidatePolicy;
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::Bid};
use serde_json::Value;

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), status_error)? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates_with_prefilter(
        CandidatePolicy::RequiredExtensionNullDefault,
        response_dto_shape,
    )?;
    for bid in parsed.seatbid.iter_mut().flat_map(|seat| &mut seat.bid) {
        canonical_integer(&mut bid.other, "mtype")
            .map_err(|_invalid_mtype| vec![response_dto_error()])?;
    }
    let mut decoded = classify_response_with_errors(parsed, parse_errors, |bid| {
        let extension = response_extension(bid)?;
        let media = match extension.ad_type {
            0 => MediaType::Banner,
            1 => MediaType::Video,
            3 => MediaType::Native,
            _ => return Err(unsupported_bid_type()),
        };
        if !sent.covered_imp_ids().any(|id| id == bid.impid) {
            return Err(BidderError::bad_server_response(
                "bid response references an unsent impression",
            ));
        }
        classify_bid(request, bid, Some(media))
    })?;
    for accepted in &mut decoded.bids {
        let extension = response_extension(&accepted.bid).map_err(|error| vec![error])?;
        if let Some(category) = category(extension.category) {
            accepted
                .bid
                .other
                .insert("cat".into(), serde_json::json!([category]));
        } else if accepted
            .bid
            .other
            .get("cat")
            .and_then(Value::as_array)
            .is_none_or(|items| items.is_empty() || items.len() > 1)
        {
            accepted.bid.other.remove("cat");
        } else {
            // Retain the existing singleton category.
        }
        let ext = accepted.bid.ext.get_or_insert_with(Default::default);
        let prebid = ext.prebid.get_or_insert_with(Default::default);
        prebid.dealpriority = (extension.deal_priority != 0).then_some(extension.deal_priority);
        prebid.other.insert(
            "video".into(),
            serde_json::json!({
            "duration":extension.duration,"primary_category":""}),
        );
    }
    Ok(decoded)
}

fn bid_extension_error() -> BidderError {
    BidderError::failed_to_unmarshal("invalid msft bid extension")
}
pub(super) fn unsupported_bid_type() -> BidderError {
    BidderError::unknown("unrecognized msft bid_ad_type")
}
fn response_dto_error() -> BidderError {
    let mut error = BidderError::failed_to_unmarshal("invalid bidder response");
    error.kind = crate::BidderErrorKind::ResponseParse;
    error
}

fn response_dto_shape(bid: &Bid) -> bool {
    signed_byte(&bid.other, "mtype").is_ok() && category_shape(bid.other.get("cat"))
}
pub(super) fn status_error(status: u16) -> BidderError {
    debug_status_error(status)
}

#[derive(Clone, Copy)]
struct ResponseExtension {
    ad_type: i64,
    category: i64,
    duration: i64,
    deal_priority: i64,
}
fn response_extension(bid: &Bid) -> Result<ResponseExtension, BidderError> {
    let parse = || -> Result<ResponseExtension, ()> {
        // The closed decoder refuses missing ext and maps present-null to the
        // source's empty extension DTO before this owner-specific validation.
        let ext = bid.ext.as_ref().ok_or(())?;
        let appnexus = object(unique(&ext.other, "appnexus")?)?;
        let get = |name| {
            appnexus
                .map(|fields| unique(fields, name))
                .transpose()
                .map(Option::flatten)
        };
        let _brand_id = integer(get("brand_id")?)?;
        let creative = object(get("creative_info")?)?;
        let video = object(
            creative
                .map(|fields| unique(fields, "video"))
                .transpose()?
                .flatten(),
        )?;
        let duration = integer(
            video
                .map(|fields| unique(fields, "duration"))
                .transpose()?
                .flatten(),
        )?;
        Ok(ResponseExtension {
            ad_type: integer(get("bid_ad_type")?)?,
            category: integer(get("brand_category_id")?)?,
            duration,
            deal_priority: integer(get("deal_priority")?)?,
        })
    };
    parse().map_err(|()| bid_extension_error())
}
fn category_shape(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::Array(items)) => items.iter().all(Value::is_string),
        _ => false,
    }
}
fn category(id: i64) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .find(|(source, _)| *source == id)
        .map(|(_, iab)| *iab)
}

const CATEGORIES: &[(i64, &str)] = &[
    (1, "IAB20-3"),
    (2, "IAB18-5"),
    (3, "IAB10-1"),
    (4, "IAB2-3"),
    (5, "IAB19-8"),
    (6, "IAB22-1"),
    (7, "IAB18-1"),
    (8, "IAB12-3"),
    (9, "IAB5-1"),
    (10, "IAB4-5"),
    (11, "IAB13-4"),
    (12, "IAB8-7"),
    (13, "IAB9-7"),
    (14, "IAB7-1"),
    (15, "IAB20-18"),
    (16, "IAB10-7"),
    (17, "IAB19-18"),
    (18, "IAB13-6"),
    (19, "IAB18-4"),
    (20, "IAB1-5"),
    (21, "IAB1-6"),
    (22, "IAB3-4"),
    (23, "IAB19-13"),
    (24, "IAB22-2"),
    (25, "IAB3-9"),
    (26, "IAB17-18"),
    (27, "IAB19-6"),
    (28, "IAB1-7"),
    (29, "IAB9-30"),
    (30, "IAB20-7"),
    (31, "IAB20-17"),
    (32, "IAB7-32"),
    (33, "IAB16-5"),
    (34, "IAB19-34"),
    (35, "IAB11-5"),
    (36, "IAB12-3"),
    (37, "IAB11-4"),
    (38, "IAB12-3"),
    (39, "IAB9-30"),
    (41, "IAB7-44"),
    (42, "IAB7-1"),
    (43, "IAB7-30"),
    (50, "IAB19-30"),
    (51, "IAB17-12"),
    (52, "IAB19-30"),
    (53, "IAB3-1"),
    (55, "IAB13-2"),
    (56, "IAB19-30"),
    (57, "IAB19-30"),
    (58, "IAB7-39"),
    (59, "IAB22-1"),
    (60, "IAB7-39"),
    (61, "IAB21-3"),
    (62, "IAB5-1"),
    (63, "IAB12-3"),
    (64, "IAB20-18"),
    (65, "IAB11-2"),
    (66, "IAB17-18"),
    (67, "IAB9-9"),
    (68, "IAB9-5"),
    (69, "IAB7-44"),
    (71, "IAB22-3"),
    (73, "IAB19-30"),
    (74, "IAB8-5"),
    (78, "IAB22-1"),
    (85, "IAB12-2"),
    (86, "IAB22-3"),
    (87, "IAB11-3"),
    (112, "IAB7-32"),
    (113, "IAB7-32"),
    (114, "IAB7-32"),
    (115, "IAB7-32"),
    (118, "IAB9-5"),
    (119, "IAB9-5"),
    (120, "IAB9-5"),
    (121, "IAB9-5"),
    (122, "IAB9-5"),
    (123, "IAB9-5"),
    (124, "IAB9-5"),
    (125, "IAB9-5"),
    (126, "IAB9-5"),
    (127, "IAB22-1"),
    (132, "IAB1-2"),
    (133, "IAB19-30"),
    (137, "IAB3-9"),
    (138, "IAB19-3"),
    (140, "IAB2-3"),
    (141, "IAB2-1"),
    (142, "IAB2-3"),
    (143, "IAB17-13"),
    (166, "IAB11-4"),
    (175, "IAB3-1"),
    (176, "IAB13-4"),
    (182, "IAB8-9"),
    (183, "IAB3-5"),
];

#[cfg(test)]
mod tests {
    use super::super::MsftBidder;
    #[cfg(not(target_arch = "wasm32"))]
    use super::super::tests::captured_fixture;
    use super::super::tests::request;
    #[cfg(not(target_arch = "wasm32"))]
    use super::CATEGORIES;
    use super::{bid_extension_error, response_dto_error, status_error, unsupported_bid_type};
    use crate::bidders::harness;
    use crate::{Bidder as _, BidderError, BidderErrorKind, BidderResponse};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    #[test]
    fn category_arrays_cannot_emit_null_or_other_non_string_elements() {
        for invalid in [Value::Null, json!(1_i64), json!(true), json!({}), json!([])] {
            let mut bid = wire_bid(
                "bad-category",
                json!({"appnexus":{"brand_category_id":1000_i64}}),
            );
            bid["cat"] = json!([invalid]);
            let result = decode(&json!({"id":"r", "seatbid":[{"bid":[bid]}]}));
            assert_eq!(result.unwrap_err(), [response_dto_error()]);
        }
    }

    fn response_request() -> BidRequest {
        request(json!([{"id":"i","banner":{},"video":{},"native":{}}]))
    }

    fn decode(value: &Value) -> Result<BidderResponse, Vec<BidderError>> {
        let request = response_request();
        let response_body = serde_json::to_vec(value).map_err(|_serialization_error| {
            vec![BidderError::bad_server_response(
                "invalid msft test response",
            )]
        })?;
        MsftBidder.make_bids(
            &request,
            &harness::sent_meta_with_ids("https://bidder.example.test/", ["i"]),
            &harness::budgeted_response(200, response_body),
        )
    }

    fn wire_bid(id: &str, extension: Value) -> Value {
        let mut value = json!({"id":id,"impid":"i","price":1.25_f64});
        value["ext"] = extension;
        value
    }

    #[test]
    fn bid_local_extension_errors_preserve_good_bids_across_seats() {
        let decoded = decode(&json!({"id":"r","cur":"EUR","seatbid":[
            {"bid":[wire_bid("bad",json!({"appnexus":{"brand_id":"bad"}}))]},
            {"bid":[wire_bid("video",json!({"appnexus":{"bid_ad_type":1_i32}})),
                wire_bid("unknown",json!({"appnexus":{"bid_ad_type":2_i32}}))]}]}))
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "video");
        assert_eq!(decoded.bids[0].media_type, MediaType::Video);
        assert_eq!(decoded.currency, "EUR");
        assert_eq!(
            decoded.errors,
            [bid_extension_error(), unsupported_bid_type()]
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn all_category_entries_and_unmapped_category_lists_have_literal_expectations() {
        assert_eq!(CATEGORIES.len(), 95);
        let captured = captured_fixture(
            "fixtures/iab_categories.json",
            include_str!("fixtures/iab_categories.json"),
        );
        let expected_categories: Vec<(i64, String)> = serde_json::from_value(captured).unwrap();
        assert_eq!(expected_categories.len(), 95);
        for (category, expected) in expected_categories {
            let decoded = decode(&json!({"id":"r","seatbid":[{"bid":[wire_bid("b",
                json!({"appnexus":{"brand_category_id":category}}))]}]}))
            .unwrap();
            assert_eq!(decoded.bids[0].bid.other["cat"], json!([expected]));
        }
        for (cat, expected) in [
            (json!(["one"]), json!(["one"])),
            (json!(["one", "two"]), Value::Null),
            (json!([]), Value::Null),
        ] {
            let mut bid = wire_bid("b", json!({"appnexus":{"brand_category_id":1_000_i32}}));
            bid["cat"] = cat;
            let decoded = decode(&json!({"id":"r","seatbid":[{"bid":[bid]}]})).unwrap();
            assert_eq!(
                decoded.bids[0].bid.other.get("cat").unwrap_or(&Value::Null),
                &expected
            );
        }
    }

    #[test]
    fn duration_is_unconditional_and_deal_priority_preserves_nonzero_signed_values() {
        for ad_type in [0_i32, 1_i32, 3_i32] {
            for number in [-7, 0, 30] {
                let decoded = decode(&json!({"id":"r","seatbid":[{"bid":[wire_bid("b",
                    json!({"appnexus":{"bid_ad_type":ad_type,"deal_priority":number,
                        "creative_info":{"video":{"duration":number}}}}))]}]}))
                .unwrap();
                let wire = serde_json::to_value(&decoded.bids[0].bid).unwrap();
                assert_eq!(
                    wire["ext"]["prebid"]["video"],
                    json!({"duration":number,"primary_category":""})
                );
                assert_eq!(
                    wire["ext"]["prebid"]["dealpriority"].as_i64().unwrap_or(0),
                    number
                );
                assert_eq!(
                    wire["ext"]["prebid"].get("dealpriority").is_none(),
                    number == 0
                );
            }
        }
    }

    #[test]
    fn deal_priority_preserves_the_full_signed_integer_domain_and_good_siblings() {
        for priority in [
            i64::MIN,
            -0x8000_0001_i64,
            -0x8000_0000_i64,
            -1_i64,
            0_i64,
            0x7FFF_FFFF_i64,
            0x8000_0000_i64,
            i64::MAX,
        ] {
            let result = decode(&json!({"id":"r","seatbid":[{"bid":[
                wire_bid("edge",json!({"appnexus":{"deal_priority":priority}})),
                wire_bid("good",json!({"appnexus":{"deal_priority":-7_i32}}))]}]}))
            .unwrap();
            assert_eq!(result.bids.len(), 2);
            assert_eq!(result.bids[0].bid.id, "edge");
            assert_eq!(result.bids[1].bid.id, "good");
            assert!(result.errors.is_empty());
            assert_eq!(
                result.bids[0]
                    .bid
                    .ext
                    .as_ref()
                    .unwrap()
                    .prebid
                    .as_ref()
                    .unwrap()
                    .dealpriority,
                (priority != 0).then_some(priority)
            );
        }
    }

    #[test]
    fn malformed_or_ambiguous_deal_priority_is_bid_local_and_preserves_good_siblings() {
        for fields in [
            json!({"deal_priority":"7"}),
            json!({"deal_priority":7.0_f64}),
            json!({"deal_priority":true}),
            json!({"deal_priority":[]}),
            json!({"deal_priority":{}}),
            json!({"deal_priority":u64::MAX}),
            json!({"deal_priority":7_i32,"DEAL_PRIORITY":null}),
        ] {
            let result = decode(&json!({"id":"r","seatbid":[
                {"bid":[wire_bid("bad",json!({"appnexus":fields}))]},
                {"bid":[wire_bid("good",json!({"appnexus":{"deal_priority":i64::MAX}}))]}]}))
            .unwrap();
            assert_eq!(result.bids.len(), 1);
            assert_eq!(result.bids[0].bid.id, "good");
            assert_eq!(result.errors, [bid_extension_error()]);
            assert_eq!(
                result.bids[0]
                    .bid
                    .ext
                    .as_ref()
                    .unwrap()
                    .prebid
                    .as_ref()
                    .unwrap()
                    .dealpriority,
                Some(i64::MAX)
            );
        }
    }

    #[test]
    fn deal_priority_defaults_and_single_ascii_equivalent_spelling_are_preserved() {
        for (fields, expected) in [
            (json!({}), None),
            (json!({"deal_priority":null}), None),
            (json!({"DEAL_PRIORITY":i64::MIN}), Some(i64::MIN)),
        ] {
            let result = decode(&json!({"id":"r","seatbid":[{"bid":[
                wire_bid("b",json!({"appnexus":fields}))]}]}))
            .unwrap();
            assert_eq!(result.bids.len(), 1);
            assert!(result.errors.is_empty());
            assert_eq!(
                result.bids[0]
                    .bid
                    .ext
                    .as_ref()
                    .unwrap()
                    .prebid
                    .as_ref()
                    .unwrap()
                    .dealpriority,
                expected
            );
        }
    }

    #[test]
    fn sealed_parse_status_currency_and_exact_call_coverage_are_independent() {
        let request = response_request();
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/", ["i"]);
        for status in [204, 400, 401, 500] {
            let result = MsftBidder.make_bids(
                &request,
                &sent,
                &harness::budgeted_response(status, b"bad".to_vec()),
            );
            if status == 204 {
                assert!(result.unwrap().bids.is_empty());
            } else {
                let error = result.unwrap_err();
                assert_eq!(error, [status_error(status)]);
            }
        }
        let error = MsftBidder
            .make_bids(
                &request,
                &sent,
                &harness::budgeted_response(200, Vec::new()),
            )
            .unwrap_err();
        assert_eq!(error[0].kind, BidderErrorKind::ResponseParse);
        for cur in [None, Some(""), Some("EUR")] {
            let mut value = json!({"id":"r","seatbid":[]});
            if let Some(currency) = cur {
                value["cur"] = json!(currency);
            }
            assert_eq!(
                decode(&value).unwrap().currency,
                cur.filter(|s| !s.is_empty()).unwrap_or("USD")
            );
        }
        let value = json!({"id":"r","seatbid":[{"bid":[wire_bid("b",json!({"appnexus":{}}))]}]});
        let coverage_error = MsftBidder
            .make_bids(
                &request,
                &harness::sent_meta("https://bidder.example.test/"),
                &harness::budgeted_response(200, serde_json::to_vec(&value).unwrap()),
            )
            .unwrap_err();
        assert_eq!(
            coverage_error,
            [BidderError::bad_server_response(
                "bid response references an unsent impression"
            )]
        );
    }

    #[test]
    fn source_extension_defaults_and_malformed_dto_fields_have_different_failure_scopes() {
        for extension in [
            json!({}),
            json!({"appnexus":null}),
            json!({"APPNEXUS":{"BID_AD_TYPE":0_i32}}),
        ] {
            let result =
                decode(&json!({"id":"r","seatbid":[{"bid":[wire_bid("b",extension)]}]})).unwrap();
            assert_eq!(result.bids.len(), 1);
            assert_eq!(result.bids[0].media_type, MediaType::Banner);
        }
        let bad_namespace = wire_bid("bad", json!({"appnexus":null,"APPNEXUS":null}));
        let result = decode(&json!({"id":"r","seatbid":[{"bid":[bad_namespace,
            wire_bid("good",json!({"appnexus":{}}))]}]}))
        .unwrap();
        assert_eq!(result.bids[0].bid.id, "good");
        assert_eq!(result.errors, [bid_extension_error()]);
        let mut malformed = wire_bid("bad", json!({"appnexus":{}}));
        malformed["mtype"] = json!("1");
        let error = decode(&json!({"id":"r","seatbid":[{"bid":[malformed,
            wire_bid("good",json!({"appnexus":{}}))]}]}))
        .unwrap_err();
        assert_eq!(error, [response_dto_error()]);
        assert_eq!(error[0].kind, BidderErrorKind::ResponseParse);
        let mut conflicting = wire_bid("conflict", json!({"appnexus":{"bid_ad_type":1_i32}}));
        conflicting["mtype"] = json!(1_i32);
        let conflict_error =
            decode(&json!({"id":"r","seatbid":[{"bid":[conflicting]}]})).unwrap_err();
        assert_eq!(
            conflict_error,
            [BidderError::bad_server_response(
                "bid response contains conflicting media type hints"
            )]
        );
    }

    #[test]
    fn missing_extension_is_local_but_present_null_uses_source_defaults() {
        for extension in [None, Some(Value::Null)] {
            let present = extension.is_some();
            let mut bid = wire_bid("unavailable", Value::Null);
            if let Some(extension_value) = extension {
                bid["ext"] = extension_value;
            } else {
                bid.as_object_mut().unwrap().remove("ext");
            }
            let result = decode(&json!({"id":"r","seatbid":[{"bid":[bid,
                wire_bid("good",json!({"appnexus":{}}))]}]}))
            .unwrap();
            assert_eq!(result.bids.len(), if present { 2 } else { 1 });
            assert_eq!(result.bids.last().unwrap().bid.id, "good");
            if present {
                assert!(result.errors.is_empty());
                assert_eq!(result.bids[0].bid.id, "unavailable");
                assert_eq!(result.bids[0].media_type, MediaType::Banner);
            } else {
                assert_eq!(result.errors.len(), 1);
                assert_eq!(result.errors[0].code, 14);
                assert_eq!(result.errors[0].kind, BidderErrorKind::ResponseParse);
                assert_eq!(result.errors[0].message, "invalid bidder response");
            }
        }
    }

    #[test]
    fn malformed_standard_fields_are_envelope_fatal_before_extension_filtering_in_both_orders() {
        let request = response_request();
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/", ["i"]);
        for standard in [r#""mtype":"1""#, r#""cat":[7]"#] {
            for extension in [
                r#""ext":false"#,
                r#""ext":{"prebid":{"targeting":{"bad":false}}}"#,
                "",
            ] {
                for extension_first in [false, true] {
                    let fields = if extension.is_empty() {
                        standard.to_owned()
                    } else if extension_first {
                        format!("{extension},{standard}")
                    } else {
                        format!("{standard},{extension}")
                    };
                    // Literal bytes preserve the two field orders under review.
                    let body = format!(
                        r#"{{"id":"r","seatbid":[{{"bid":[{{"id":"first","impid":"i","price":1,"ext":{{}}}},{{"id":"bad","impid":"i","price":1,{fields}}},{{"id":"last","impid":"i","price":1,"ext":{{}}}}]}}]}}"#
                    );
                    let errors = MsftBidder
                        .make_bids(
                            &request,
                            &sent,
                            &harness::budgeted_response(200, body.into_bytes()),
                        )
                        .unwrap_err();
                    assert_eq!(errors, [response_dto_error()], "fields={fields}");
                }
            }
        }
    }

    #[test]
    fn valid_standard_fields_keep_extension_refusals_bid_local_in_both_orders() {
        let request = response_request();
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/", ["i"]);
        for fields in [
            r#""ext":false,"mtype":1,"cat":["IAB1"]"#,
            r#""cat":["IAB1"],"mtype":1,"ext":false"#,
            r#""mtype":1,"cat":["IAB1"]"#,
        ] {
            let body = format!(
                r#"{{"id":"r","seatbid":[{{"bid":[{{"id":"first","impid":"i","price":1,"ext":{{}}}},{{"id":"bad","impid":"i","price":1,{fields}}},{{"id":"last","impid":"i","price":1,"ext":{{}}}}]}}]}}"#
            );
            let decoded = MsftBidder
                .make_bids(
                    &request,
                    &sent,
                    &harness::budgeted_response(200, body.into_bytes()),
                )
                .unwrap();
            assert_eq!(
                decoded
                    .bids
                    .iter()
                    .map(|bid| bid.bid.id.as_str())
                    .collect::<Vec<_>>(),
                ["first", "last"]
            );
            assert_eq!(decoded.errors.len(), 1);
            assert_eq!(decoded.errors[0].code, 14);
            assert_eq!(decoded.errors[0].kind, BidderErrorKind::ResponseParse);
        }
    }

    #[test]
    fn nonobject_and_reserved_typed_bid_extensions_preserve_valid_siblings() {
        for extension in [
            json!(true),
            json!(1_i32),
            json!("not an object"),
            json!([]),
            json!({"prebid":{"dealpriority":"7"}}),
            json!({"prebid":{"dealpriority":7.0_f64}}),
            json!({"prebid":{"dealpriority":true}}),
            json!({"prebid":{"dealpriority":[]}}),
            json!({"prebid":{"dealpriority":{}}}),
            json!({"prebid":{"dealpriority":u64::MAX}}),
        ] {
            let result = decode(&json!({"id":"r","seatbid":[{"bid":[
                wire_bid("bad",extension),wire_bid("good",json!({"appnexus":{}}))]}]}))
            .unwrap();
            assert_eq!(result.bids.len(), 1);
            assert_eq!(result.bids[0].bid.id, "good");
            let error = result.errors;
            assert_eq!(error.len(), 1);
            assert_eq!(error[0].code, 14);
            assert_eq!(error[0].kind, BidderErrorKind::ResponseParse);
        }
    }

    #[test]
    fn wide_reserved_deal_priority_decodes_before_the_provider_overwrite_projection() {
        for priority in [i64::MIN, -0x8000_0001_i64, 0x8000_0000_i64, i64::MAX] {
            for provider_priority in [None, Some(-7_i64), Some(i64::MAX)] {
                let mut extension = json!({"prebid":{"dealpriority":priority,"opaque":true},
                    "appnexus":{}});
                if let Some(priority_value) = provider_priority {
                    extension["appnexus"]["deal_priority"] = json!(priority_value);
                }
                let result = decode(&json!({"id":"r","seatbid":[{"bid":[
                    wire_bid("wide",extension),wire_bid("good",json!({"appnexus":{}}))]}]}))
                .unwrap();
                assert_eq!(result.bids.len(), 2);
                assert_eq!(result.bids[0].bid.id, "wide");
                assert_eq!(result.bids[1].bid.id, "good");
                assert!(result.errors.is_empty());
                let prebid = result.bids[0]
                    .bid
                    .ext
                    .as_ref()
                    .unwrap()
                    .prebid
                    .as_ref()
                    .unwrap();
                assert_eq!(prebid.dealpriority, provider_priority);
                assert_eq!(prebid.other["opaque"], json!(true));
                assert!(!prebid.other.contains_key("dealpriority"));
            }
        }
    }
}

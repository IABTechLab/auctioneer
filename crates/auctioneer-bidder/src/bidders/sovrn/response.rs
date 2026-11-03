use crate::bidders::{
    media_type::classify_response_with_errors,
    status::{debug_status_error, standard_status},
    wire_fields::{IntegerFieldError, signed_byte},
};
use crate::{BidderError, BidderHttpResponse, BidderResponse, OutboundRequestMeta};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::mem::take;

pub(super) fn make_bids(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    response: &BidderHttpResponse,
) -> Result<BidderResponse, Vec<BidderError>> {
    if let Some(no_bid) = standard_status(response.status(), debug_status_error)? {
        return Ok(no_bid);
    }
    let (mut parsed, parse_errors) = response.parse_bid_candidates()?;
    // Source decodes the entire signed-byte field before considering ADM or bids.
    for bid in parsed.seatbid.iter().flat_map(|seat| &seat.bid) {
        signed_byte(&bid.other, "mtype").map_err(|error| {
            vec![BidderError::bad_server_response(match error {
                IntegerFieldError::Invalid => "invalid sovrn mtype",
                IntegerFieldError::Ambiguous => "sovrn response contains ambiguous mtype keys",
            })]
        })?;
    }
    parsed.cur.clear();
    let mut decode_errors = Vec::new();
    for seat in &mut parsed.seatbid {
        seat.bid
            .retain_mut(|bid| match decode_markup(&mut bid.adm) {
                Ok(keep) => keep,
                Err(error) => {
                    decode_errors.push(error);
                    false
                }
            });
    }
    let result = classify_response_with_errors(parsed, parse_errors, |bid| {
        let imp = request
            .imp
            .iter()
            .find(|imp| imp.id == bid.impid)
            .ok_or_else(|| BidderError::bad_input("sovrn bid references an unknown impression"))?;
        if !sent.covered_imp_ids().any(|id| id == bid.impid) {
            return Err(BidderError::bad_server_response(
                "sovrn bid references an unsent impression",
            ));
        }
        // Sovrn ignores response hints and defaults every matched non-video imp to banner.
        Ok(if imp.video.is_some() {
            MediaType::Video
        } else {
            MediaType::Banner
        })
    });
    match result {
        Ok(mut decoded) => {
            decoded.errors.extend(decode_errors);
            if decoded.bids.is_empty() && !decoded.errors.is_empty() {
                Err(decoded.errors)
            } else {
                Ok(decoded)
            }
        }
        Err(mut errors) => {
            errors.extend(decode_errors);
            Err(errors)
        }
    }
}

fn decode_markup(markup: &mut Option<String>) -> Result<bool, BidderError> {
    let Some(text) = markup else {
        return Ok(true);
    };
    if !text.bytes().any(|byte| matches!(byte, b'%' | b'+')) {
        return Ok(true);
    }
    // Query unescaping cannot expand its input. Reuse the sealed parser's allocation.
    let mut bytes = take(text).into_bytes();
    let mut read = 0_usize;
    let mut written = 0_usize;
    while let Some(byte) = bytes.get(read).copied() {
        let decoded = match byte {
            b'%' => {
                let Some((high, low)) = bytes
                    .get(read.saturating_add(1))
                    .and_then(|hex_byte| char::from(*hex_byte).to_digit(16))
                    .zip(
                        bytes
                            .get(read.saturating_add(2))
                            .and_then(|hex_byte| char::from(*hex_byte).to_digit(16)),
                    )
                else {
                    return Ok(false);
                };
                read = read.saturating_add(2);
                u8::try_from(high.saturating_mul(16).saturating_add(low)).unwrap_or_default()
            }
            b'+' => b' ',
            _ => byte,
        };
        if let Some(slot) = bytes.get_mut(written) {
            *slot = decoded;
        }
        written = written.saturating_add(1);
        read = read.saturating_add(1);
    }
    bytes.truncate(written);
    *text = String::from_utf8(bytes).map_err(|_error| {
        BidderError::bad_server_response("sovrn ADM contains unsupported decoded bytes")
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::super::{SovrnBidder, tests::request};
    use crate::bidders::harness;
    use crate::{Bidder as _, BidderErrorKind, BidderHttpResponse};
    use auctioneer_config_data::types::MediaType;
    use serde_json::{Value, json};

    #[test]
    fn statuses_are_204_no_bid_400_bad_input_and_other_non200_bad_server() {
        let request = request(json!([{"id":"i","banner":{}}]));
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/", ["i"]);
        // Invalid bytes prove status handling happens before any JSON decode.
        for status in [204_u16, 400, 201, 302, 404, 500] {
            let response = harness::budgeted_response(status, vec![b'{']);
            let result = SovrnBidder.make_bids(&request, &sent, &response);
            if status == 204 {
                let decoded = result.unwrap();
                assert!(decoded.bids.is_empty());
                assert!(decoded.errors.is_empty());
                assert_eq!(decoded.currency, "USD");
            } else {
                let errors = result.unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, if status == 400 { 2 } else { 4 });
                assert_eq!(errors[0].kind, BidderErrorKind::Other);
                assert_eq!(
                    errors[0].message,
                    format!(
                        "Unexpected status code: {status}. Run with request.debug = 1 for more info"
                    )
                );
            }
        }
    }

    #[test]
    fn response_currency_is_always_usd_including_valid_non_usd_and_empty_seats() {
        let request = request(json!([{"id":"i","banner":{}}]));
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/", ["i"]);
        for currency in [None, Some(""), Some("USD"), Some("EUR")] {
            for seats in [
                json!([]),
                json!([{"seat":"s","bid":[]}]),
                json!([{"seat":"s","bid":[{"id":"b","impid":"i","price":1.25_f64}]}]),
            ] {
                let mut value = json!({"id":"r","seatbid":seats});
                if let Some(wire_currency) = currency {
                    value["cur"] = json!(wire_currency);
                }
                let response = harness::budgeted_response(200, serde_json::to_vec(&value).unwrap());
                let decoded = SovrnBidder.make_bids(&request, &sent, &response).unwrap();
                assert_eq!(decoded.currency, "USD");
                assert!(decoded.errors.is_empty());
                assert_eq!(
                    decoded.bids.len(),
                    seats[0]["bid"].as_array().map_or(0, Vec::len)
                );
            }
        }
    }

    #[test]
    fn adm_query_decoding_is_once_only_plus_to_space_and_preserves_other_fields() {
        for (encoded, expected) in [
            ("a+b%2Bc%20d", "a b+c d"),
            ("%3Cdiv%3Eok%3C%2Fdiv%3E", "<div>ok</div>"),
            ("%2520", "%20"),
            ("%E2%82%AC", "\u{20ac}"),
            ("", ""),
            ("a=b&c=d", "a=b&c=d"),
        ] {
            let decoded = harness::decode_bid(&SovrnBidder, json!([{"id":"i","banner":{}}]), json!({"adm":encoded,"nurl":"https://notice.test/?x=a+b%20c","burl":"https://bill.test/%2F","ext":{"opaque":true}})).unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.adm.as_deref(), Some(expected));
            assert_eq!(
                decoded.bids[0].bid.other["nurl"],
                json!("https://notice.test/?x=a+b%20c")
            );
            assert_eq!(
                decoded.bids[0].bid.other["burl"],
                json!("https://bill.test/%2F")
            );
            assert!(decoded.errors.is_empty());
        }
    }

    #[test]
    fn invalid_adm_escapes_drop_only_that_bid_without_a_diagnostic() {
        for malformed in ["%", "%0", "%gg", "%2x", "ok%20bad%"] {
            let decoded = harness::decode_bids(
                &SovrnBidder,
                json!([{"id":"i","banner":{}}]),
                json!([
                    {"seat":"first","bid":[{"id":"bad","adm":malformed}]},
                    {"seat":"second","bid":[{"id":"good","adm":"good+ad"}]},
                ]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "good");
            assert_eq!(decoded.bids[0].bid.adm.as_deref(), Some("good ad"));
            assert!(decoded.errors.is_empty());
            let empty = harness::decode_bid(
                &SovrnBidder,
                json!([{"id":"i","banner":{}}]),
                json!({"impid":"unknown","adm":malformed}),
            )
            .unwrap();
            assert!(empty.bids.is_empty());
            assert!(empty.errors.is_empty());
            assert_eq!(empty.currency, "USD");
        }
    }

    #[test]
    fn matched_video_precedes_banner_and_response_media_hints_do_not_override_source() {
        for fields in [
            json!({}),
            json!({"mtype":1_u32}),
            json!({"mtype":3_i32}),
            json!({"mtype":-128_i32}),
            json!({"mtype":127_i32}),
            json!({"mtype":null}),
            json!({"ext":{"prebid":{"type":"banner"}}}),
            json!({"ext":{"native":{},"video":{}}}),
        ] {
            let decoded = harness::decode_bid(
                &SovrnBidder,
                json!([{"id":"i","banner":{},"video":{},"native":{}}]),
                fields,
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].media_type, MediaType::Video);
            assert!(decoded.errors.is_empty());
        }
        let decoded = harness::decode_bid(
            &SovrnBidder,
            json!([{"id":"other","video":{}},{"id":"i","banner":{}}]),
            json!({"ext":{"video":{}}}),
        )
        .unwrap();
        assert_eq!(decoded.bids[0].media_type, MediaType::Banner);
    }

    #[test]
    fn unknown_impression_is_redacted_bid_local_bad_input_across_seats() {
        let decoded = harness::decode_bids(
            &SovrnBidder,
            json!([{"id":"i","banner":{}}]),
            json!([
                {"seat":"first","bid":[{"id":"bad","impid":"sensitive-unknown"}]},
                {"seat":"second","bid":[{"id":"good","impid":"i"}]},
            ]),
        )
        .unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "good");
        assert_eq!(decoded.errors.len(), 1);
        assert_eq!(decoded.errors[0].code, 2);
        assert_eq!(decoded.errors[0].kind, BidderErrorKind::Other);
        assert!(!decoded.errors[0].message.contains("sensitive-unknown"));
        let errors = harness::decode_bid(
            &SovrnBidder,
            json!([{"id":"i","banner":{}}]),
            json!({"impid":"unknown"}),
        )
        .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 2);
    }

    #[test]
    fn sent_call_coverage_excludes_filtered_impressions_without_losing_siblings() {
        let request = request(json!([{"id":"not-sent","banner":{}},{"id":"sent","video":{}}]));
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/", ["sent"]);
        let response = harness::budgeted_response(
            200,
            serde_json::to_vec(&json!({
                "id":"r", "seatbid":[{"seat":"s","bid":[
                    {"id":"bad","impid":"not-sent","price":1.0_f64},
                    {"id":"good","impid":"sent","price":2.0_f64},
                ]}],
            }))
            .unwrap(),
        );
        let decoded = SovrnBidder.make_bids(&request, &sent, &response).unwrap();
        assert_eq!(decoded.bids.len(), 1);
        assert_eq!(decoded.bids[0].bid.id, "good");
        assert_eq!(decoded.bids[0].media_type, MediaType::Video);
        assert_eq!(decoded.errors.len(), 1);
        assert_eq!(decoded.errors[0].code, 4);
    }

    #[test]
    fn malformed_envelopes_and_node_budget_use_the_sealed_decoder() {
        let request = request(json!([{"id":"i","banner":{}}]));
        let sent = harness::sent_meta_with_ids("https://bidder.example.test/", ["i"]);
        // Empty and truncated bytes are lexical controls, not ordinary JSON fixtures.
        let mut malformed = vec![Vec::new(), vec![b'{']];
        for value in [
            json!(false),
            json!([]),
            json!({"id":"r","seatbid":false}),
            json!({"id":"r","cur":1_i32}),
        ] {
            malformed.push(serde_json::to_vec(&value).unwrap());
        }
        for bytes in malformed {
            let errors = SovrnBidder
                .make_bids(&request, &sent, &harness::budgeted_response(200, bytes))
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, 14);
            assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
            assert_eq!(errors[0].message, "invalid bidder response");
        }
        let response = BidderHttpResponse::new(
            200,
            vec![],
            serde_json::to_vec(&json!({"id":"r","seatbid":[]})).unwrap(),
        );
        let errors = SovrnBidder
            .make_bids(&request, &sent, &response)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(errors[0].code, 4);
    }

    #[test]
    fn unrepresentable_decoded_markup_is_explicitly_refused_without_losing_a_sibling() {
        for markup in ["%FF", "%C0%AF", "%E2%82"] {
            let decoded = harness::decode_bids(
                &SovrnBidder,
                json!([{"id":"i","banner":{}}]),
                json!([
                    {"seat":"first","bid":[{"id":"unsupported","adm":markup}]},
                    {"seat":"second","bid":[{"id":"good","adm":"valid+markup"}]},
                ]),
            )
            .unwrap();
            assert_eq!(decoded.bids.len(), 1);
            assert_eq!(decoded.bids[0].bid.id, "good");
            assert_eq!(decoded.bids[0].bid.adm.as_deref(), Some("valid markup"));
            assert_eq!(decoded.errors.len(), 1);
            assert_eq!(decoded.errors[0].code, 4);
            assert_eq!(decoded.errors[0].kind, BidderErrorKind::Other);
            assert_eq!(
                decoded.errors[0].message,
                "sovrn ADM contains unsupported decoded bytes"
            );
        }
    }

    #[test]
    fn malformed_mtype_is_response_wide_bad_server_response() {
        for key in ["mtype", "Mtype", "MTYPE"] {
            for value in [
                json!("private-marker"),
                json!(false),
                json!(1.0_f64),
                json!(1.5_f64),
                json!([]),
                json!({}),
                json!(128_i32),
                json!(-129_i32),
                json!(u64::MAX),
            ] {
                for markup in ["valid", "%"] {
                    let errors = harness::decode_bids(
                        &SovrnBidder,
                        json!([{"id":"i","banner":{}}]),
                        json!([
                            {"seat":"bad","bid":[{"id":"bad",key:value,"adm":markup}]},
                            {"seat":"good","bid":[{"id":"good","mtype":1_i32}]},
                        ]),
                    )
                    .unwrap_err();
                    assert_eq!(errors.len(), 1);
                    // Sovrn wraps source JSON decoding failures as BadServerResponse.
                    assert_eq!(errors[0].code, 4);
                    assert!(!errors[0].message.contains("private-marker"));
                }
            }
        }
    }

    #[test]
    fn competing_mtype_spellings_reject_the_whole_response() {
        for (left, right) in [("mtype", "Mtype"), ("Mtype", "MTYPE")] {
            for (first, second) in [
                (json!(1_i32), json!(2_i32)),
                (json!(1_i32), json!(1_i32)),
                (json!(1_i32), Value::Null),
                (Value::Null, Value::Null),
            ] {
                let errors = harness::decode_bids(
                    &SovrnBidder,
                    json!([{"id":"i","banner":{}}]),
                    json!([
                        {"seat":"bad","bid":[{"id":"bad",left:first,right:second}]},
                        {"seat":"good","bid":[{"id":"good","mtype":1_i32}]},
                    ]),
                )
                .unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].code, 4);
            }
        }
    }
}

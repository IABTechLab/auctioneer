//! Literal price-replacement oracles and original response-only allocation windows.

use crate::alloc_track::response as alloc_track;
use crate::response_allocation::{
    SHIPPED_DECODED_CAP, SHIPPED_RESPONSE_NODE_CAP, response_charge, syntactic_nodes,
};
use auctioneer_bidder::{
    BidderHttpResponse, BidderResponse, META_BYTES_CAPACITY, OutboundRequestMetaBuilder,
};
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::budget::MIN_RESPONSE_PARSE_FACTOR;
use serde_json::json;
use std::{str::from_utf8, thread};

const TTD_ALLOCATION_PRICE_MACRO: &str = "${AUCTION_PRICE}";

fn ttd_allocation_price_cases() -> [(f64, String); 3] {
    // Independent literal oracles for Rust's fixed decimal Display form:
    // MAX is 17 significant digits plus 292 zeros; 5e-324 has 323
    // fractional zeros before its final 5. These are not measured lengths.
    [
        (1.5_f64, "1.5".to_owned()),
        (f64::MAX, format!("17976931348623157{}", "0".repeat(292))),
        (f64::from_bits(1), format!("0.{}5", "0".repeat(323))),
    ]
}

fn ttd_allocation_macro_body(count: usize, repeats: usize, price: f64) -> (Vec<u8>, String) {
    let channel = format!(
        "head|{}${{auction_price}}|${{Auction_Price}}|tail",
        format!("{TTD_ALLOCATION_PRICE_MACRO}|").repeat(repeats),
    );
    let bytes = serde_json::to_vec(&json!({
        "id":"r", "cur":"USD", "seatbid":[{"bid":
            (0..count).map(|index| json!({
                "id":format!("b{index}"), "impid":"i", "price":price, "mtype":1_i32,
                "adm":channel, "nurl":channel, "burl":channel, "lurl":channel,
            })).collect::<Vec<_>>()
        }],
    }))
    .unwrap();
    (bytes, channel)
}

#[test]
fn thetradedesk_price_rewriting_fits_the_funded_response_charge() {
    thread::spawn(|| {
        let bidder = (auctioneer_bidder::compiled_bidders()
            .iter()
            .find(|entry| entry.key == "thetradedesk")
            .unwrap()
            .factory)();
        let request: BidRequest = serde_json::from_value(json!({
            "id":"r", "imp":[{"id":"i","banner":{}}],
        }))
        .unwrap();
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
        builder.set_method("POST").unwrap();
        builder
            .set_url("https://bidder.example.test/bid/s")
            .unwrap();
        builder.imp_id("i").unwrap();
        let sent = builder.finish();
        assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["i"]);

        for (price, literal) in ttd_allocation_price_cases() {
            assert_eq!(price.to_string(), literal, "literal price oracle drifted");
            let mut measured_cases = 0_usize;
            for count in [1_usize, 32, 256] {
                for repeats in [0_usize, 1, 32] {
                    let (bytes, original) = ttd_allocation_macro_body(count, repeats, price);
                    let wire_bytes = u64::try_from(bytes.len()).unwrap();
                    let nodes = syntactic_nodes(from_utf8(&bytes).unwrap());
                    assert!(nodes <= u64::from(SHIPPED_RESPONSE_NODE_CAP));
                    if count == 256 && repeats == 32 {
                        // Three rewriting channels alone carry 393,216 macro
                        // bytes. Do not widen the declared 256 KiB cap to
                        // measure a body production could never admit.
                        let macro_bytes = count
                            .checked_mul(3)
                            .and_then(|n| n.checked_mul(repeats))
                            .and_then(|n| n.checked_mul(TTD_ALLOCATION_PRICE_MACRO.len()))
                            .unwrap();
                        assert_eq!(macro_bytes, 393_216);
                        assert!(u64::try_from(macro_bytes).unwrap() > SHIPPED_DECODED_CAP);
                        assert!(wire_bytes > SHIPPED_DECODED_CAP);
                        continue;
                    }
                    assert!(wire_bytes <= SHIPPED_DECODED_CAP);
                    // The expected text is built from literal digits, not
                    // from adapter output or an allocator observation.
                    let expected = format!(
                        "head|{}${{auction_price}}|${{Auction_Price}}|tail",
                        format!("{literal}|").repeat(repeats),
                    );
                    let response = BidderHttpResponse::new(200, Vec::new(), bytes)
                        .within_node_budget(SHIPPED_RESPONSE_NODE_CAP);
                    assert_eq!(u64::try_from(response.body_len()).unwrap(), wire_bytes);
                    assert_eq!(response.max_parsed_nodes(), SHIPPED_RESPONSE_NODE_CAP);

                    // All fixtures, expected strings, factory and correlation
                    // metadata exist before this response-only window.
                    let baseline = alloc_track::reset_peak();
                    let result = bidder.make_bids(&request, &sent, &response);
                    let peak = alloc_track::peak_since(baseline);

                    let decoded = result.unwrap();
                    assert!(peak > 0);
                    assert!(
                        peak <= response_charge(wire_bytes),
                        "count={count} repeats={repeats} price={price:e} \
                         wire={wire_bytes} nodes={nodes} peak={peak} \
                         funded={}",
                        response_charge(wire_bytes),
                    );
                    assert_ttd_substituted_bids(&decoded, count, price, &expected, &original);
                    measured_cases += 1;
                }
            }
            assert_eq!(
                measured_cases, 8,
                "only the explicit over-byte-cap corner is excluded"
            );
        }
    })
    .join()
    .unwrap();
}

fn assert_ttd_substituted_bids(
    decoded: &BidderResponse,
    count: usize,
    price: f64,
    expected: &str,
    original: &str,
) {
    assert_eq!(
        decoded.bids.len(),
        count,
        "classification must not mask allocation"
    );
    assert!(decoded.errors.is_empty());
    assert_eq!(decoded.currency, "USD");
    for (index, typed) in decoded.bids.iter().enumerate() {
        assert_eq!(typed.media_type, MediaType::Banner);
        let bid = &typed.bid;
        assert_eq!(bid.id, format!("b{index}"));
        assert_eq!(bid.impid, "i");
        assert_eq!(
            bid.price.to_bits(),
            price.to_bits(),
            "price must survive sealed parsing"
        );
        assert_eq!(bid.adm.as_deref(), Some(expected));
        assert_eq!(bid.other["nurl"].as_str(), Some(expected));
        assert_eq!(bid.other["burl"].as_str(), Some(expected));
        assert_eq!(bid.other["lurl"].as_str(), Some(original));
        // Exact full-string equality also preserves both
        // non-uppercase controls and replaces every repeat.
        let encoded = serde_json::to_value(bid).unwrap();
        for name in ["adm", "nurl", "burl"] {
            assert_eq!(encoded[name].as_str(), Some(expected));
        }
        assert_eq!(encoded["lurl"].as_str(), Some(original));
    }
}

#[test]
fn thetradedesk_price_expansion_has_a_literal_bound_within_response_funding() {
    // This is string-content reasoning, not a substitute for the measured
    // parser/classifier/replacement peak in the other test. There are no
    // allocator-derived coefficients or fixture-sized node reservations.
    let cases = ttd_allocation_price_cases();
    let longest_literal = cases
        .iter()
        .map(|(_, literal)| literal.len())
        .max()
        .unwrap();
    assert_eq!(TTD_ALLOCATION_PRICE_MACRO.len(), 16);
    assert_eq!(longest_literal, 326);
    let literal_multiplier = longest_literal.div_ceil(TTD_ALLOCATION_PRICE_MACRO.len());
    assert_eq!(literal_multiplier, 21);
    assert!(u64::try_from(literal_multiplier).unwrap() <= MIN_RESPONSE_PARSE_FACTOR);

    for (price, literal) in cases {
        assert_eq!(literal.parse::<f64>().unwrap().to_bits(), price.to_bits());
        assert_eq!(price.to_string(), literal);
        for count in [1_usize, 32, 256] {
            for repeats in [0_usize, 1, 32] {
                let (bytes, original) = ttd_allocation_macro_body(count, repeats, price);
                assert_eq!(
                    original.matches(TTD_ALLOCATION_PRICE_MACRO).count(),
                    repeats
                );
                let expected = format!(
                    "head|{}${{auction_price}}|${{Auction_Price}}|tail",
                    format!("{literal}|").repeat(repeats),
                );
                let non_macro_bytes = original
                    .len()
                    .checked_sub(
                        repeats
                            .checked_mul(TTD_ALLOCATION_PRICE_MACRO.len())
                            .unwrap(),
                    )
                    .unwrap();
                assert_eq!(
                    expected.len(),
                    non_macro_bytes
                        .checked_add(repeats.checked_mul(literal.len()).unwrap())
                        .unwrap(),
                );
                assert!(!expected.contains(TTD_ALLOCATION_PRICE_MACRO));
                assert_eq!(expected.matches("${auction_price}").count(), 1);
                assert_eq!(expected.matches("${Auction_Price}").count(), 1);

                let input_contents = count
                    .checked_mul(3)
                    .unwrap()
                    .checked_mul(original.len())
                    .unwrap();
                let output_contents = count
                    .checked_mul(3)
                    .unwrap()
                    .checked_mul(expected.len())
                    .unwrap();
                let wire_bytes = u64::try_from(bytes.len()).unwrap();
                assert!(output_contents <= input_contents.checked_mul(literal_multiplier).unwrap());
                // Even conservatively keeping ALL three old strings beside
                // ALL replacements plus one formatted price fits the byte
                // floor. Heap handles/capacities and parsed nodes are covered
                // by the real peak assertion, not claimed by this inequality.
                let simultaneous_contents = input_contents
                    .checked_add(output_contents)
                    .and_then(|n| n.checked_add(literal.len()))
                    .unwrap();
                assert!(
                    u64::try_from(simultaneous_contents).unwrap()
                        <= wire_bytes.checked_mul(MIN_RESPONSE_PARSE_FACTOR).unwrap()
                );
                assert!(
                    u64::try_from(simultaneous_contents).unwrap() <= response_charge(wire_bytes)
                );
                if count == 256 && repeats == 32 {
                    assert!(wire_bytes > SHIPPED_DECODED_CAP);
                } else {
                    assert!(wire_bytes <= SHIPPED_DECODED_CAP);
                }
            }
        }
    }
}

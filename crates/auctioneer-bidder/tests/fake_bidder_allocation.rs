//! Shared construction bounds use synthetic capabilities, independently of a provider protocol.

#![cfg(not(target_arch = "wasm32"))]

pub mod allocation;
pub use allocation as alloc_track;
pub use auctioneer_bidder::test_support::fake_bidder;

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod floor_currency_tests {
    use super::alloc_track;
    use auctioneer_bidder::{FloorConversion, FloorConversionError};
    use auctioneer_config_data::FloorCurrencyConfig;
    use std::{collections::BTreeMap, hint::black_box};

    fn snapshot(count: usize) -> FloorCurrencyConfig {
        FloorCurrencyConfig {
            expires_at: 2_000,
            usd_per_unit: (0..count)
                .map(|index| {
                    let code = String::from_utf8(vec![
                        b'Q',
                        b'A'.checked_add(u8::try_from(index.checked_div(26).unwrap()).unwrap())
                            .unwrap(),
                        b'A'.checked_add(u8::try_from(index.checked_rem(26).unwrap()).unwrap())
                            .unwrap(),
                    ])
                    .unwrap();
                    (code, 2.5_f64)
                })
                .collect::<BTreeMap<_, _>>(),
        }
    }

    #[test]
    fn floor_views_and_repeated_lookups_allocate_no_rate_table_or_code_storage() {
        for count in [1_usize, 8_usize, 256_usize] {
            let snapshot = snapshot(count);
            for repetitions in [1_i32, 100_i32] {
                alloc_track::start();
                let conversion = FloorConversion::new(Some(black_box(&snapshot)), 1_999);
                for _ in 0_i32..repetitions {
                    assert_eq!(
                        conversion.convert_floor_to_usd(black_box(4.0_f64), "QAA"),
                        Ok(10.0_f64)
                    );
                    assert_eq!(
                        conversion.convert_floor_to_usd(black_box(4.0_f64), "qAa"),
                        Ok(10.0_f64)
                    );
                    assert_eq!(conversion.convert_floor_to_usd(4.0, "USD"), Ok(4.0_f64));
                    assert_eq!(
                        conversion.convert_floor_to_usd(4.0, "QZZ"),
                        Err(FloorConversionError::MissingRate)
                    );
                    assert_eq!(
                        conversion.convert_floor_to_usd(f64::MAX, "QAA"),
                        Err(FloorConversionError::Overflow)
                    );
                }
                let measured = alloc_track::finish();
                assert_eq!(
                    measured.allocated(),
                    0,
                    "count={count}, repetitions={repetitions}: {measured:?}"
                );
                assert_eq!(measured.peak(), 0);
                assert_eq!(measured.largest(), 0);
            }

            alloc_track::start();
            drop(black_box(snapshot.usd_per_unit.clone()));
            let copied = alloc_track::finish();
            assert!(
                copied.allocated() > 0,
                "positive control must observe an owned rate table"
            );
            assert!(copied.peak() > 0);
        }
    }
}

#[cfg(test)]
mod native_creative_tests {
    use super::alloc_track;
    use auctioneer_bidder::BidderHttpResponse;
    use serde_json::{Value, json};
    use std::{hint::black_box, thread};

    #[test]
    fn native_value_projection_allocates_only_its_exact_encoded_output() {
        for nodes in [1_usize, 1024] {
            for bytes in [0_usize, 64 * 1024] {
                let value = json!({"assets": vec![json!({"id": 1_i32}); nodes],
                    "text": "\u{00e9}\n".repeat(bytes)});
                let expected = value.to_string();
                let response = BidderHttpResponse::new(200, Vec::new(), vec![b' '; expected.len()])
                    .within_node_budget(1 << 18);
                alloc_track::start();
                let result = response.project_adm_value(black_box(&value));
                let measured = alloc_track::finish();
                let output = result.unwrap();
                assert_eq!(output, expected);
                assert_eq!(output.capacity(), expected.len());
                assert_eq!(measured.peak(), expected.len());
                assert_eq!(measured.allocated(), expected.len());
                assert_eq!(measured.largest(), expected.len());
            }
        }
    }

    #[test]
    fn native_value_projection_refuses_before_allocating_output_or_a_tree() {
        let value = json!({"assets": vec![json!({"id": 1_i32}); 1024],
            "text": "x".repeat(64 * 1024)});
        let expected = value.to_string();
        for (bytes, nodes) in [(1_usize, 1_u32 << 18_u32), (expected.len(), 1_u32)] {
            let response = BidderHttpResponse::new(200, Vec::new(), vec![b' '; bytes])
                .within_node_budget(nodes);
            alloc_track::start();
            let result = response.project_adm_value(black_box(&value));
            let measured = alloc_track::finish();
            let error = result.unwrap_err();
            assert_eq!(error.code, 4);
            assert_eq!(error.message, "native creative exceeded parsing limits");
            assert!(measured.allocated() <= 64, "{measured:?}");
            assert!(measured.peak() <= 64);
            assert!(measured.largest() <= 64);
        }
        alloc_track::start();
        drop(black_box(value.clone()));
        let copied = alloc_track::finish();
        assert!(copied.allocated() > 64 * 1024);
        assert!(copied.peak() > 64 * 1024);
    }

    #[test]
    fn unescaped_native_validation_does_not_retain_a_payload_or_node_tree() {
        for nodes in [1_usize, 128, 4096] {
            for bytes in [0_usize, 64 * 1024] {
                thread::spawn(move || {
                    let creative = json!({
                        "assets": vec![json!({"id": 1_i32}); nodes],
                        "link": {"url": "x".repeat(bytes)},
                    });
                    let expected = creative.to_string();
                    let mut adm = json!({"native": creative, "ignored": [null, true]}).to_string();
                    let response = BidderHttpResponse::new(
                        200,
                        Vec::new(),
                        serde_json::to_vec(&json!({"adm": adm})).unwrap(),
                    )
                    .within_node_budget(1 << 16);
                    let pointer = adm.as_ptr();
                    let capacity = adm.capacity();
                    alloc_track::start();
                    let result = response.unwrap_native_adm(black_box(&mut adm));
                    let measured = alloc_track::finish();
                    result.unwrap();
                    assert_eq!(adm, expected);
                    assert_eq!(adm.as_ptr(), pointer);
                    assert_eq!(adm.capacity(), capacity);
                    // Small parser scratch is allowed; one owned object tree is not.
                    assert!(
                        measured.peak() <= 64,
                        "nodes={nodes}, bytes={bytes}: {measured:?}"
                    );
                    assert!(measured.allocated() <= 64);
                    assert!(measured.largest() <= 64);

                    // The same allocator window must observe the forbidden tree copy.
                    alloc_track::start();
                    let owned: Value = serde_json::from_str(black_box(&adm)).unwrap();
                    black_box(&owned);
                    let copied = alloc_track::finish();
                    assert!(copied.peak() > 0);
                    assert!(copied.allocated() > 0);
                    assert!(copied.largest() > 0);
                })
                .join()
                .unwrap();
            }
        }
    }

    #[test]
    fn escaped_native_parser_scratch_stays_inside_response_byte_funding() {
        for nodes in [1_usize, 4096] {
            for bytes in [128_usize, 64 * 1024] {
                for escaped_key in [false, true] {
                    thread::spawn(move || {
                        let key = if escaped_key {
                            "\\".repeat(bytes)
                        } else {
                            "link".into()
                        };
                        let creative = json!({
                            key: "\\".repeat(bytes),
                            "assets": vec![json!({"id": 1_i32}); nodes],
                        });
                        let expected = creative.to_string();
                        let mut adm = json!({"native": creative}).to_string();
                        let response = BidderHttpResponse::new(
                            200,
                            Vec::new(),
                            serde_json::to_vec(&json!({"adm": adm})).unwrap(),
                        )
                        .within_node_budget(1 << 16);
                        let original_len = adm.len();
                        let pointer = adm.as_ptr();
                        alloc_track::start();
                        let result = response.unwrap_native_adm(black_box(&mut adm));
                        let measured = alloc_track::finish();
                        result.unwrap();
                        assert_eq!(adm, expected);
                        assert_eq!(adm.as_ptr(), pointer);
                        assert!(adm.len() < original_len);
                        // Two decoder scratch buffers may grow geometrically, but no
                        // input-dependent tree may remain. This is stricter than the
                        // existing whole-response parse charge.
                        assert!(measured.peak() <= original_len * 2, "{measured:?}");
                        assert!(measured.allocated() <= original_len * 4, "{measured:?}");
                        assert!(measured.largest() <= original_len * 2, "{measured:?}");
                    })
                    .join()
                    .unwrap();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::alloc_track::{self, AllowAllEndpoints, Measurement};
    use super::fake_bidder::{FakeBidder, RequestMode};
    use auctioneer_bidder::{
        Bidder as _, BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundRequestSink,
        SinkLimits, outbound_meta_bytes,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::json;
    use std::{hint::black_box, num::NonZeroUsize, thread};

    fn request(width: usize, count: usize) -> BidRequest {
        let tree = (0..width)
            .map(|index| {
                (
                    format!("field{index}"),
                    json!({"keep":["value",1_i32,true]}),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        serde_json::from_value(json!({
            "id":"r", "root_unknown":tree,
            "imp":(0..count).map(|index| json!({"id":format!("i{index}"),"banner":{},"unknown":tree})).collect::<Vec<_>>(),
        })).unwrap()
    }

    fn context() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("fake_public"),
            BidderName::new("fake_bidder"),
            "a",
            None,
            "https://fake.example.test/bid",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn probe(width: usize, count: usize, mode: RequestMode, cap: usize, slots: u32) -> Measurement {
        thread::spawn(move || {
            let request = request(width, count);
            let context = context();
            let bidder = FakeBidder { request_mode: mode, ..FakeBidder::default() };
            let budget = CallBudget::new(slots);
            let limits = SinkLimits::new(cap, u16::try_from(count).unwrap());
            let mut sink = OutboundRequestSink::new(limits, &budget, &AllowAllEndpoints);
            alloc_track::start();
            let result = bidder.make_requests(black_box(&request), black_box(&context), &mut sink);
            let outcome = sink.into_outcome();
            let measured = alloc_track::finish();
            assert_eq!(result, Ok(()));
            let attempts = match mode {
                RequestMode::Aggregate => 1,
                RequestMode::Split => count,
                RequestMode::Chunked(size) => count.div_ceil(size.get()),
            };
            if slots == 0 {
                assert!(outcome.accepted.is_empty());
                assert_eq!(bidder.written_bodies(), 0);
                assert_eq!(outcome.rejected_call_count, u32::try_from(attempts).unwrap());
            } else {
                assert!(bidder.written_bodies() > 0);
                if cap == 128 {
                    assert!(outcome.accepted.is_empty());
                    assert_eq!(outcome.rejected_call_count, 0);
                    assert_eq!(bidder.written_bodies(), attempts);
                    assert_eq!(outcome.errors, vec![auctioneer_bidder::OutboundBuildError::OversizeRaw { limit: 128 }; attempts]);
                } else {
                    assert_eq!(outcome.accepted.len(), attempts.min(usize::try_from(slots).unwrap()));
                    for call in &outcome.accepted {
                        let body: serde_json::Value = serde_json::from_slice(call.body()).unwrap();
                        assert_eq!(body["root_unknown"].as_object().unwrap().len(), width);
                        assert_eq!(body["imp"].as_array().unwrap().len(), call.covered_imp_ids().count());
                    }
                }
            }
            let per_attempt = cap.checked_add(usize::try_from(outbound_meta_bytes(u16::try_from(count).unwrap())).unwrap()).unwrap()
                .checked_add(alloc_track::CONSTRUCTION_CONTROL_BYTES).unwrap();
            let funded = per_attempt.checked_mul(attempts).unwrap();
            assert!(measured.peak() <= funded && measured.allocated() <= funded && measured.largest() <= funded, "width={width} count={count} mode={mode:?} cap={cap} slots={slots}: {measured:?}; bound={funded}");
            measured
        }).join().unwrap()
    }

    #[test]
    fn fake_request_modes_are_bounded_across_independent_payload_caps_and_slots() {
        for count in [1_usize, 8] {
            for mode in [
                RequestMode::Aggregate,
                RequestMode::Split,
                RequestMode::Chunked(NonZeroUsize::new(3).unwrap()),
            ] {
                for cap in [128_usize, 2 * 1024 * 1024] {
                    for slots in [0_u32, 1, 8] {
                        let small = probe(1, count, mode, cap, slots);
                        let large = probe(512, count, mode, cap, slots);
                        assert!(
                            alloc_track::size_growth_is_bounded(&small, &large),
                            "count={count} mode={mode:?} cap={cap} slots={slots}: small={small:?} large={large:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn synthetic_allocation_controls_detect_actual_caller_tree_clones() {
        for count in [1_usize, 8] {
            let clone_probe = |width| {
                thread::spawn(move || {
                    let request = request(width, count);
                    alloc_track::start();
                    let copied = black_box(request.clone());
                    let measured = alloc_track::finish();
                    assert_eq!(copied.imp.len(), count);
                    measured
                })
                .join()
                .unwrap()
            };
            let small = clone_probe(1);
            let large = clone_probe(512);
            assert!(
                !alloc_track::size_growth_is_bounded(&small, &large),
                "the positive control must distinguish copying from borrowing"
            );
            assert!(large.allocated() > small.allocated());
            assert!(large.peak() > small.peak());
            assert!(large.largest() > 0);
            alloc_track::assert_bounded(&small, 64 * 1024);
        }
    }
}

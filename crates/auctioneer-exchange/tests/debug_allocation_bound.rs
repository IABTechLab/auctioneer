//! Allocation and privacy guards over the sealed projector, collector and engine.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;
#[cfg(test)]
mod debug_allocation;
mod support;

#[global_allocator]
static ALLOCATOR: debug_allocation::Allocator = debug_allocation::Allocator;

#[cfg(test)]
mod tests {
    use super::{alloc_track, debug_allocation};
    use auctioneer_bidder::{
        BidderError, BidderErrorKind, BidderHttpResponse, DebugBodyEligibility, DebugBodySchema,
        DebugBodyStatus, project_request_body,
    };
    use auctioneer_exchange::debug::capture::Capture;
    use auctioneer_exchange::{BidderCallResult, FanoutError};
    use auctioneer_runtime::bounded_json::NODE_RETAINED_BYTES;
    use auctioneer_runtime::debug_budget::{
        DEBUG_BODY_INPUT_FACTOR, DEBUG_CONVERSION_RETAINED_FACTOR, DEBUG_DEPTH_FRAME_BYTES,
        DEBUG_GROWING_BUFFER_FACTOR, DEBUG_JSON_ESCAPE_FACTOR, DEBUG_OUTER_VALUE_NODES,
        DEBUG_ROW_VALUE_NODES, MAX_DEBUG_BODY_DEPTH, MAX_DEBUG_RETAINED_BYTES,
        MAX_DEBUG_SANITIZED_BODY_BYTES, MAX_DEBUG_SERIALIZED_BYTES, debug_peak_bytes,
    };
    use auctioneer_runtime::{Clock, FakeClock, MonotonicInstant, count_json_nodes};
    use debug_allocation::{
        CallStorageProbe, Engine, REQUEST_MIME_BYTES, RefuseNext, deadline, outbound,
    };
    use serde_json::{Value, json};
    use std::io::{self, Write as _};
    use std::iter;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    fn with_margin(peak: u64) -> u64 {
        peak.checked_mul(6).unwrap().div_ceil(5)
    }

    fn report(name: &str, stage: &str, peak: u64, funded: u64) {
        let margin = with_margin(peak);
        writeln!(
            io::stderr(),
            "{name}/{stage}: peak={peak} margin={margin} funded={funded}"
        )
        .unwrap();
        assert!(margin <= funded, "{name}/{stage}: {margin} > {funded}");
    }

    fn response(value: &Value) -> BidderCallResult {
        BidderCallResult {
            result: Ok(BidderHttpResponse::from_transport(
                200,
                vec![
                    (
                        "Content-Type".into(),
                        "application/json; token=RESPONSE_HEADER_CANARY".into(),
                    ),
                    ("Content-Encoding".into(), "gzip".into()),
                    ("Sec-GPC".into(), "1".into()),
                    ("Set-Cookie".into(), "SET_COOKIE_CANARY".into()),
                ],
                serde_json::to_vec(value).unwrap(),
                DebugBodyEligibility::SingleTextValue,
            )
            .within_node_budget(4096)),
            elapsed: Duration::from_micros(17_345),
        }
    }

    fn projected(part: &Value) -> Value {
        assert_eq!(part["body_status"], "captured");
        serde_json::from_str(part["body"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn sealed_projector_peak_fits_streaming_components_at_node_depth_and_escape_bounds() {
        let mut forest = vec![json!({"": []}); 1363];
        forest.extend([Value::Null, Value::Null]);
        let deep = (1_u32..64).fold(Value::Null, |value, _| json!([value]));
        let mut full_input = json!({"test": 1_i32, "unknown": ""});
        let overhead = serde_json::to_vec(&full_input).unwrap().len();
        full_input["unknown"] = json!("x".repeat(64 * 1024 - overhead));
        let shapes = [
            ("tiny", json!({"test": 1_i32})),
            (
                "max-nodes",
                json!({"test": 1_i32, "unknown": vec![Value::Null; 4091]}),
            ),
            ("unknown-forest", json!({"test": 1_i32, "unknown": forest})),
            ("depth-64", json!({"test": 1_i32, "unknown": deep})),
            ("input-cap", full_input),
            (
                "unicode-controls",
                json!({"test": 1_i32, "\u{96ea}\u{1f642}\0\n\"\\".repeat(1700): "\u{96ea}\u{1f642}\0\n\"\\".repeat(1700)}),
            ),
        ];
        for (name, shape) in shapes {
            thread::spawn(move || {
                let raw = serde_json::to_vec(&shape).unwrap();
                assert!(raw.len() <= 64 * 1024);
                if name == "input-cap" {
                    assert_eq!(raw.len(), 64 * 1024);
                }
                if matches!(name, "max-nodes" | "unknown-forest") {
                    assert_eq!(count_json_nodes(&raw, 4096).unwrap(), 4096);
                }
                let mut response_shape = shape;
                response_shape.as_object_mut().unwrap().remove("test");
                response_shape["cur"] = json!("USD");
                if name == "input-cap" {
                    let excess = serde_json::to_vec(&response_shape).unwrap().len() - 64 * 1024;
                    let padding = response_shape["unknown"].as_str().unwrap().len() - excess;
                    response_shape["unknown"] = json!("x".repeat(padding));
                }
                let response_raw = serde_json::to_vec(&response_shape).unwrap();
                assert!(response_raw.len() <= 64 * 1024);
                let input_bytes = raw.len().max(response_raw.len());
                let sealed = BidderHttpResponse::from_transport(
                    200,
                    vec![("Content-Type".into(), "application/json".into())],
                    response_raw,
                    DebugBodyEligibility::SingleTextValue,
                )
                .within_node_budget(4096);
                // Streaming scratch, deliberately excluding the provisional owned-node-tree term.
                let components = u64::try_from(input_bytes).unwrap() * DEBUG_BODY_INPUT_FACTOR
                    + u64::from(MAX_DEBUG_BODY_DEPTH) * DEBUG_DEPTH_FRAME_BYTES
                    + MAX_DEBUG_SANITIZED_BODY_BYTES * DEBUG_GROWING_BUFFER_FACTOR;
                let before = alloc_track::reset_peak();
                let request = project_request_body(
                    DebugBodySchema::OpenRtb,
                    &raw,
                    [("Content-Type", "application/json")],
                    4096,
                );
                let request_peak = alloc_track::peak_since(before);
                assert_eq!(request.body_status, DebugBodyStatus::Captured);
                assert_eq!(
                    serde_json::from_str::<Value>(request.body.as_ref().unwrap()).unwrap(),
                    json!({"test": 1_i32})
                );
                assert!(request_peak >= MAX_DEBUG_SANITIZED_BODY_BYTES);
                report(name, "request-projector", request_peak, components);
                drop(request);
                let response_start = alloc_track::reset_peak();
                let response = sealed.project_debug_body(DebugBodySchema::OpenRtb);
                let response_peak = alloc_track::peak_since(response_start);
                assert_eq!(response.body_status, DebugBodyStatus::Captured);
                assert_eq!(
                    serde_json::from_str::<Value>(response.body.as_ref().unwrap()).unwrap(),
                    json!({"cur": "USD"})
                );
                report(name, "response-projector", response_peak, components);
                assert!(
                    with_margin(request_peak) > MAX_DEBUG_SANITIZED_BODY_BYTES,
                    "a body-length-only allowance must fail even for a tiny projection"
                );
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn collector_peak_includes_retained_metadata_dto_value_and_serialized_output() {
        for (name, calls, ceiling, headers, complete) in [
            ("tiny", 1_usize, 1_u32, false, true),
            ("request-only", 64, 64, false, false),
            ("empty-204", 64, 64, false, true),
            ("headers", 64, 64, true, true),
            ("funded-width", 65, 65, false, true),
        ] {
            thread::spawn(move || {
                let request = outbound(&json!({"test": 1_i32}), headers);
                let empty = BidderCallResult {
                    result: Ok(
                        BidderHttpResponse::new(204, vec![], vec![]).within_node_budget(4096)
                    ),
                    elapsed: Duration::ZERO,
                };
                let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
                let deadline = deadline(&clock);
                let alias =
                    "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
                let before = alloc_track::reset_peak();
                let mut capture = Capture::new(ceiling);
                for index in 0..calls {
                    let tentative = capture.prepare_request(
                        &request,
                        DebugBodySchema::OpenRtb,
                        4096,
                        &clock,
                        deadline,
                    );
                    let slot = capture.commit_request(
                        index,
                        &alias,
                        tentative,
                        request
                            .headers()
                            .chain([("Content-Encoding", "gzip"), ("Sec-GPC", "1")])
                            .filter(|_| headers),
                        &clock,
                        deadline,
                    );
                    if complete {
                        capture.complete(slot, &empty, DebugBodySchema::OpenRtb, &clock, deadline);
                    }
                }
                let retained = u64::try_from(alloc_track::live() - before).unwrap();
                let capture_peak = alloc_track::peak_since(before);
                let conversion_start = alloc_track::reset_peak();
                let value = capture.finish(&clock, deadline).unwrap();
                let wire = serde_json::to_vec(&value).unwrap();
                let conversion_peak = alloc_track::peak_since(conversion_start);
                let peak = capture_peak.max(retained + conversion_peak);
                assert!(retained <= MAX_DEBUG_RETAINED_BYTES);
                assert!(u64::try_from(wire.len()).unwrap() <= MAX_DEBUG_SERIALIZED_BYTES);
                let rows = value["httpcalls"][&alias].as_array().unwrap();
                assert!(!rows.is_empty());
                let kept = rows.len();
                assert_eq!(value["dropped_calls"], calls - kept);
                if !headers {
                    assert_eq!(kept, calls.min(64));
                }
                for (index, row) in rows.iter().enumerate() {
                    assert_eq!(row["call_index"], index);
                    assert_eq!(projected(&row["request"]), json!({"test": 1_i32}));
                    if complete {
                        assert_eq!(row["response"]["status"], 204_i32);
                        assert_eq!(row["response"]["body_status"], "empty");
                    } else {
                        assert!(row.get("response").is_none());
                    }
                }
                let conversion_funded = MAX_DEBUG_RETAINED_BYTES * DEBUG_CONVERSION_RETAINED_FACTOR
                    + (u64::try_from(kept).unwrap() * DEBUG_ROW_VALUE_NODES
                        + DEBUG_OUTER_VALUE_NODES)
                        * NODE_RETAINED_BYTES
                    + (MAX_DEBUG_RETAINED_BYTES * DEBUG_JSON_ESCAPE_FACTOR
                        + MAX_DEBUG_SERIALIZED_BYTES)
                        * DEBUG_GROWING_BUFFER_FACTOR;
                report(
                    name,
                    "retained-plus-conversion",
                    retained + conversion_peak,
                    conversion_funded,
                );
                report(
                    name,
                    "capture-to-wire",
                    peak,
                    debug_peak_bytes(ceiling).unwrap(),
                );
                assert!(peak > u64::try_from(wire.len()).unwrap());
                writeln!(
                    io::stderr(),
                    "{name}: rows={kept} retained={retained} wire={}",
                    wire.len()
                )
                .unwrap();
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn dense_projected_bodies_retain_near_the_capacity_limit_before_value_conversion() {
        thread::spawn(|| {
            let source = json!({"imp": vec![json!({"banner": {"w": 1_i32, "h": 1_i32}}); 500]});
            let expected_response = json!({"cur": "USD", "seatbid": [{"bid": vec![json!({"price": 1_i32, "w": 1_i32, "h": 1_i32}); 500]}]});
            let request = outbound(&source, false);
            let result = response(&expected_response);
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let deadline = deadline(&clock);
            let mut slots = Vec::with_capacity(4);
            let baseline = alloc_track::reset_peak();
            let mut capture = Capture::new(4);
            for index in 0..4_usize {
                let pending = capture.prepare_request(&request, DebugBodySchema::OpenRtb, 4096, &clock, deadline);
                slots.push(capture.commit_request(index, "fake_", pending, iter::empty(), &clock, deadline));
            }
            for slot in &slots {
                capture.complete(*slot, &result, DebugBodySchema::OpenRtb, &clock, deadline);
            }
            let retained = u64::try_from(alloc_track::live() - baseline).unwrap();
            let capture_peak = alloc_track::peak_since(baseline);
            let conversion_start = alloc_track::reset_peak();
            let value = capture.finish(&clock, deadline).unwrap();
            let wire = serde_json::to_vec(&value).unwrap();
            let peak = capture_peak.max(retained + alloc_track::peak_since(conversion_start));
            assert!(retained > 48 * 1024, "dense fixture must exercise retained capacity, not just metadata");
            assert!(retained <= MAX_DEBUG_RETAINED_BYTES);
            let rows = value["httpcalls"]["fake_"].as_array().unwrap();
            assert_eq!(projected(&rows[0]["request"]), source);
            assert_eq!(projected(&rows[0]["response"]), expected_response);
            assert_eq!(value["dropped_calls"], 4_usize - rows.len());
            assert_eq!(value["truncated"], true);
            assert!(wire.len() <= 64 * 1024);
            report("dense-bodies", "capture-to-wire", peak, debug_peak_bytes(4).unwrap());
            writeln!(io::stderr(), "dense-bodies: rows={} retained={retained} wire={}", rows.len(), wire.len()).unwrap();
        }).join().unwrap();
    }

    #[test]
    fn real_projected_output_cap_omits_atomically_without_unbounded_growth() {
        thread::spawn(|| {
            let raw = serde_json::to_vec(&json!({"imp": vec![json!({"id": "CANARY_dense_id", "banner": {"w": 1_i32, "h": 1_i32}}); 400]})).unwrap();
            assert!(raw.len() < 64 * 1024);
            assert!(count_json_nodes(&raw, 4096).unwrap() <= 4096);
            let baseline = alloc_track::reset_peak();
            let projection = project_request_body(DebugBodySchema::OpenRtb, &raw, [("Content-Type", "application/json")], 4096);
            let peak = alloc_track::peak_since(baseline);
            assert_eq!(projection.body_status, DebugBodyStatus::OmittedSize);
            assert!(projection.body.is_none());
            assert_eq!(alloc_track::live(), baseline);
            report("output-cap", "projector", peak, MAX_DEBUG_SANITIZED_BODY_BYTES * DEBUG_GROWING_BUFFER_FACTOR);
        }).join().unwrap();
    }

    #[test]
    fn real_projector_fallible_writer_reservation_reports_unavailable_once() {
        let raw = serde_json::to_vec(&json!({"test": 1_i32})).unwrap();
        let before = alloc_track::live();
        let refusal = RefuseNext::arm();
        let projection = project_request_body(
            DebugBodySchema::OpenRtb,
            &raw,
            [("Content-Type", "application/json")],
            4096,
        );
        drop(refusal);
        assert_eq!(projection.body_status, DebugBodyStatus::Unavailable);
        assert!(projection.body.is_none());
        assert_eq!(alloc_track::live(), before);
        let positive = project_request_body(
            DebugBodySchema::OpenRtb,
            &raw,
            [("Content-Type", "application/json")],
            4096,
        );
        assert_eq!(positive.body_status, DebugBodyStatus::Captured);
    }

    fn request_canaries() -> (Value, Value) {
        let mut source = json!({
            "at": 1_i32, "test": 1_i32, "tmax": 100_i32, "allimps": 0_i32, "cur": ["USD"],
            "imp": [{"id": "CANARY_imp_id", "tagid": "CANARY_tagid", "ext": {"tid": "CANARY_imp_tid"},
                "banner": {"w": 300_i32, "h": 250_i32, "pos": 1_i32, "format": [{"w": 320_i32, "h": 50_i32}],
                    "ext": {"CANARY_banner_key": "CANARY_banner_value"}}}],
            "device": {"dnt": 1_i32, "lmt": 0_i32, "CANARY_flat_device_key": "CANARY_flat_device_value"},
            "regs": {"gdpr": 1_i32, "coppa": 0_i32, "gpp": "CANARY_modern_gpp", "us_privacy": "CANARY_modern_usp",
                "ext": {"gdpr": 1_i32, "gpp": "CANARY_legacy_gpp", "us_privacy": "CANARY_legacy_usp", "consent": "CANARY_legacy_consent",
                    "gpc": "CANARY_gpc", "dsa": "CANARY_dsa", "CANARY_regs_key": "CANARY_regs_value"}},
            "CANARY_flat_root_key": "CANARY_flat_root_value"
        });
        let mut expected = json!({
            "at": 1_i32, "test": 1_i32, "tmax": 100_i32, "allimps": 0_i32, "cur": ["USD"],
            "imp": [{"id": "[REDACTED]", "tagid": "[REDACTED]", "ext": "[REDACTED]",
                "banner": {"w": 300_i32, "h": 250_i32, "pos": 1_i32, "format": [{"w": 320_i32, "h": 50_i32}]}}],
            "device": {"dnt": 1_i32, "lmt": 0_i32},
            "regs": {"gdpr": 1_i32, "coppa": 0_i32, "gpp": "[REDACTED]", "us_privacy": "[REDACTED]",
                "ext": {"gdpr": 1_i32, "gpp": "[REDACTED]", "us_privacy": "[REDACTED]", "consent": "[REDACTED]"}}
        });
        for key in ["id", "site", "app", "dooh", "source", "ext"] {
            source[key] = json!({format!("CANARY_{key}_key"): format!("CANARY_{key}_value")});
            expected[key] = json!("[REDACTED]");
        }
        source["user"] = json!({
            "id": "CANARY_user_id", "buyeruid": "CANARY_buyer_id", "consent": "CANARY_modern_consent",
            "eids": [{"source": "CANARY_eid_source", "uids": [{"id": "CANARY_eid_id"}]}],
            "geo": {"lat": 12.345_678_f64, "lon": 87.654_321_f64},
            "ext": {"consent": "CANARY_user_legacy_consent", "eids": [{"id": "CANARY_legacy_eid"}]},
            "CANARY_flat_user_key": "CANARY_flat_user_value"
        });
        expected["user"] = json!("[REDACTED]");
        for key in [
            "ua", "sua", "ip", "ipv6", "ifa", "geo", "dpidsha1", "dpidmd5", "didsha1", "didmd5",
            "macsha1", "macmd5", "ext",
        ] {
            source["device"][key] =
                json!({format!("CANARY_device_{key}_key"): format!("CANARY_device_{key}_value")});
            expected["device"][key] = json!("[REDACTED]");
        }
        (source, expected)
    }

    fn response_canaries() -> (Value, Value) {
        let mut source = json!({
            "id": "CANARY_response_id", "cur": "USD", "ext": {"CANARY_response_ext_key": "CANARY_response_ext_value"},
            "seatbid": [{"seat": "fake_CANARY", "group": 1_i32, "bid": [{"price": 1.25_f64, "w": 300_i32, "h": 250_i32,
                "CANARY_bid_key": "CANARY_bid_value"}]}],
            "CANARY_response_key": "CANARY_response_value"
        });
        let mut expected = json!({
            "id": "[REDACTED]", "cur": "USD", "ext": "[REDACTED]",
            "seatbid": [{"seat": "[REDACTED]", "group": 1_i32, "bid": [{"price": 1.25_f64, "w": 300_i32, "h": 250_i32}]}]
        });
        for key in [
            "id", "impid", "crid", "adid", "dealid", "adm", "nurl", "burl", "lurl", "ext",
        ] {
            source["seatbid"][0]["bid"][0][key] = json!(format!("CANARY_creative_tracking_{key}"));
            expected["seatbid"][0]["bid"][0][key] = json!("[REDACTED]");
        }
        (source, expected)
    }

    fn assert_no_canaries(value: &Value) {
        for output in [serde_json::to_string(value).unwrap(), format!("{value:?}")] {
            assert!(!output.contains("CANARY"));
            assert!(!output.contains("12.345678"));
            assert!(!output.contains("87.654321"));
        }
        testing_logger::validate(|logs| {
            for entry in logs {
                assert!(!entry.body.contains("CANARY"));
                assert!(!entry.body.contains("12.345678"));
                assert!(!entry.body.contains("87.654321"));
            }
        });
    }

    #[test]
    fn complete_capture_and_logs_keep_exact_safe_leaves_and_mask_every_canary() {
        testing_logger::setup();
        let (raw_request, expected_request) = request_canaries();
        let (raw_response, expected_response) = response_canaries();
        let request = outbound(&raw_request, true);
        let result = response(&raw_response);
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let deadline = deadline(&clock);
        let mut capture = Capture::new(1);
        let pending =
            capture.prepare_request(&request, DebugBodySchema::OpenRtb, 4096, &clock, deadline);
        let slot = capture.commit_request(0, "fake_", pending, request.headers(), &clock, deadline);
        capture.complete(slot, &result, DebugBodySchema::OpenRtb, &clock, deadline);
        capture.adapter_errors(
            slot,
            &[BidderError {
                code: 999,
                message: "CANARY_parser_error".into(),
                kind: BidderErrorKind::ResponseParse,
            }],
        );
        let value = capture.finish(&clock, deadline).unwrap();
        let row = &value["httpcalls"]["fake_"][0];
        assert_eq!(projected(&row["request"]), expected_request);
        assert_eq!(projected(&row["response"]), expected_response);
        assert_eq!(row["request"]["endpoint_origin"], "https://bidder.example");
        assert_eq!(
            row["request"]["headers"],
            json!({"content-type": ["application/json"]})
        );
        assert_eq!(
            row["response"]["headers"],
            json!({"content-type": ["application/json"], "content-encoding": ["gzip"], "sec-gpc": ["1"]})
        );
        assert_eq!(row["error"], "response_parse");
        assert_eq!(row["elapsed_ms"], 17_i32);
        assert_eq!(value["truncated"], false);
        assert_eq!(value["dropped_calls"], 0_i32);
        assert_no_canaries(&value);
    }

    #[test]
    fn opaque_compressed_and_transport_failures_keep_safe_captured_request_not_raw_diagnostics() {
        testing_logger::setup();
        let (source, expected) = request_canaries();
        let request = outbound(&source, true);
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let deadline = deadline(&clock);
        let cases = [
            (
                Ok(BidderHttpResponse::from_transport(
                    200,
                    vec![("Content-Type".into(), "application/json".into())],
                    b"\x1f\x8bCANARY_compressed".to_vec(),
                    DebugBodyEligibility::SingleTextValue,
                )
                .within_node_budget(4096)),
                Some("omitted_type"),
                None,
            ),
            (
                Ok(BidderHttpResponse::from_transport(
                    200,
                    vec![(
                        "Content-Type".into(),
                        "text/html; token=CANARY_opaque_header".into(),
                    )],
                    b"CANARY_opaque".to_vec(),
                    DebugBodyEligibility::SingleTextValue,
                )
                .within_node_budget(4096)),
                Some("omitted_type"),
                None,
            ),
            (
                Err(FanoutError::Other("CANARY_transport_error".into())),
                None,
                Some("transport"),
            ),
        ];
        for (result, status, error) in cases {
            let mut capture = Capture::new(1);
            let pending =
                capture.prepare_request(&request, DebugBodySchema::OpenRtb, 4096, &clock, deadline);
            let slot =
                capture.commit_request(0, "fake_", pending, request.headers(), &clock, deadline);
            capture.complete(
                slot,
                &BidderCallResult {
                    result,
                    elapsed: Duration::ZERO,
                },
                DebugBodySchema::OpenRtb,
                &clock,
                deadline,
            );
            let value = capture.finish(&clock, deadline).unwrap();
            let row = &value["httpcalls"]["fake_"][0];
            assert_eq!(projected(&row["request"]), expected);
            if let Some(expected_status) = status {
                assert_eq!(row["response"]["body_status"], expected_status);
                assert!(row["response"].get("body").is_none());
                assert_eq!(value["truncated"], true);
            } else {
                assert!(row.get("response").is_none());
                assert_eq!(row["error"], error.unwrap());
                assert_eq!(value["truncated"], false);
            }
            assert_no_canaries(&value);
        }
    }

    fn engine_results(count: usize, mime: &str) -> Vec<BidderCallResult> {
        iter::repeat_with(|| BidderCallResult {
            result: Ok(BidderHttpResponse::from_transport(
                200,
                vec![("Content-Type".into(), mime.into())],
                serde_json::to_vec(&json!({"id": "CANARY_engine_response", "cur": "USD"})).unwrap(),
                DebugBodyEligibility::SingleTextValue,
            )),
            elapsed: Duration::ZERO,
        })
        .take(count)
        .collect()
    }

    #[test]
    fn actual_engine_denied_gates_skip_body_mime_and_call_capture_with_positive_controls() {
        for gate in ["unrequested", "account-denied", "base-denied", "allowed"] {
            thread::spawn(move || {
                let prefix = "application/json; a=";
                let mime = format!("{prefix}{}", "x".repeat(64 * 1024 - prefix.len()));
                let mut fixture =
                    Engine::new(1, 1, gate != "base-denied", engine_results(1, &mime));
                fixture.set_permissions(gate != "unrequested", gate != "account-denied");
                let observed = fixture.run();
                assert_eq!(observed.dispatched, 1);
                let response = observed.response.unwrap();
                let debug = &response["ext"]["debug"];
                if gate == "allowed" {
                    assert_eq!(observed.schema_reads, 1);
                    assert!(observed.response_probes.writers > 0);
                    assert!(observed.response_probes.mime_sources > 0);
                    let alias = "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_owned();
                    assert_eq!(debug["httpcalls"][&alias].as_array().unwrap().len(), 1);
                    assert_eq!(
                        projected(&debug["httpcalls"][&alias][0]["response"]),
                        json!({"id": "[REDACTED]", "cur": "USD"})
                    );
                } else {
                    assert_eq!(observed.schema_reads, 0);
                    assert_eq!(observed.response_probes.writers, 0);
                    assert_eq!(observed.response_probes.mime_sources, 0);
                    if gate == "base-denied" {
                        assert_eq!(
                            debug,
                            &json!({"httpcalls": {}, "truncated": false, "dropped_calls": 0_i32})
                        );
                    } else {
                        assert!(debug.is_null());
                    }
                }
                let warning_key = if gate == "account-denied" {
                    "prebid".to_owned()
                } else {
                    "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()
                };
                let warnings = response["ext"]["warnings"][&warning_key].as_array();
                if matches!(gate, "account-denied" | "base-denied") {
                    let denied = warnings.unwrap();
                    assert_eq!(denied.len(), 1);
                    assert_eq!(
                        denied[0]["code"],
                        if gate == "account-denied" {
                            10002_i32
                        } else {
                            10003_i32
                        }
                    );
                } else {
                    assert!(warnings.is_none());
                }
                // This includes ordinary engine work and required denial warnings.
                report(
                    gate,
                    "whole-engine",
                    observed.peak,
                    debug_peak_bytes(1).unwrap(),
                );
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn actual_engine_request_phase_suppresses_body_mime_and_call_storage_for_each_denied_gate() {
        for gate in ["unrequested", "account-denied", "base-denied", "allowed"] {
            thread::spawn(move || {
                let prefix = "application/json; token=";
                let mime = format!("{prefix}{}", "x".repeat(REQUEST_MIME_BYTES - prefix.len()));
                let mut fixture = Engine::with_request_mime(
                    4,
                    37,
                    gate != "base-denied",
                    engine_results(4, "application/json"),
                    Some(mime),
                );
                fixture.set_permissions(gate != "unrequested", gate != "account-denied");
                let observed = fixture.run();
                assert_eq!(observed.dispatched, 4);
                let request = observed.request_probes;
                if gate == "allowed" {
                    assert_eq!(request.writers, 4, "one real request projection per call");
                    assert_eq!(request.request_mime_sources, 8, "body eligibility and final header projection each parse MIME per call");
                    assert_eq!(
                        request.call_storage, 1,
                        "one prepaid row vector, not one allocation per response"
                    );
                    let output = observed.response.unwrap();
                    let alias = "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
                    let rows = output["ext"]["debug"]["httpcalls"][&alias]
                        .as_array()
                        .unwrap();
                    assert_eq!(rows.len(), 4);
                    for row in rows {
                        let body = projected(&row["request"]);
                        assert_eq!(body["id"], "[REDACTED]");
                        assert_eq!(
                            body["imp"][0]["banner"],
                            json!({"w": 300_i32, "h": 250_i32})
                        );
                    }
                } else {
                    assert_eq!(
                        request.writers, 0,
                        "{gate}: request projector was invoked even if its result was discarded"
                    );
                    assert_eq!(
                        request.request_mime_sources, 0,
                        "{gate}: request MIME parser was invoked"
                    );
                    assert_eq!(
                        request.call_storage, 0,
                        "{gate}: per-call capture storage was allocated"
                    );
                }
                writeln!(io::stderr(), "{gate}/request: writers={} mime_sources={} call_storage={} ordinary_64k_buffers={}",
                    request.writers, request.request_mime_sources, request.call_storage, request.mime_sources).unwrap();
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn call_storage_probe_observes_real_collector_row_reservation() {
        let request = outbound(&json!({"test": 1_i32}), false);
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let deadline = deadline(&clock);
        let mut capture = Capture::new(37);
        let pending =
            capture.prepare_request(&request, DebugBodySchema::OpenRtb, 4096, &clock, deadline);
        let _storage_probe = CallStorageProbe::new(37);
        let before = debug_allocation::probes();
        assert_eq!(
            capture.commit_request(0, "fake_", pending, iter::empty(), &clock, deadline),
            Some(0)
        );
        assert_eq!(
            debug_allocation::probes().call_storage - before.call_storage,
            1
        );
        assert_eq!(
            capture.finish(&clock, deadline).unwrap()["httpcalls"]["fake_"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn actual_fake_b_parser_error_keeps_captured_safe_body_without_payload_in_output_or_logs() {
        testing_logger::setup();
        // An absent seat is valid. The typed nbr field, not an opaque media
        // hint, must independently force the sealed parser's refusal.
        let source = json!({"id": "CANARY_parser_response_id", "cur": "USD",
            "nbr": "CANARY_invalid_nbr", "seatbid": [{"bid": [{
            "id": "CANARY_parser_bid_id", "impid": "imp-0", "price": 1_i32,
            "mtype": 1_i32
        }]}]});
        let observed = Engine::new(1, 1, true, vec![response(&source)]).run();
        let output = observed.response.unwrap();
        let alias = "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
        let row = &output["ext"]["debug"]["httpcalls"][&alias][0];
        let request = projected(&row["request"]);
        assert_eq!(request["id"], "[REDACTED]");
        assert_eq!(
            request["imp"][0]["banner"],
            json!({"w": 300_i32, "h": 250_i32})
        );
        assert_eq!(
            projected(&row["response"]),
            json!({"id": "[REDACTED]", "cur": "USD", "seatbid": [{"bid": [{
                "id": "[REDACTED]", "impid": "[REDACTED]", "price": 1_i32
            }]}]})
        );
        assert_eq!(row["error"], "response_parse");
        assert_eq!(
            output["ext"]["errors"][&alias],
            json!([{"code": 14_i32, "message": "invalid bidder response"}])
        );
        assert!(output["seatbid"].as_array().is_none_or(Vec::is_empty));
        assert_eq!(output["id"], "ENGINE_REQUEST_CANARY");
        assert_no_canaries(&output["ext"]);
    }

    #[test]
    fn actual_engine_assembly_fits_prepaid_debug_at_funded_call_widths() {
        for calls in [1_usize, 63, 64, 65] {
            thread::spawn(move || {
                let ceiling = u32::try_from(calls).unwrap();
                let fixture = Engine::new(
                    calls,
                    ceiling,
                    true,
                    engine_results(calls, "application/json"),
                );
                let observed = fixture.run();
                assert_eq!(observed.dispatched, calls);
                assert_eq!(observed.schema_reads, 1);
                let response = observed.response.unwrap();
                let debug = &response["ext"]["debug"];
                let alias =
                    "fake_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned();
                let rows = debug["httpcalls"][&alias].as_array().unwrap();
                assert!(!rows.is_empty());
                assert_eq!(debug["dropped_calls"], calls - rows.len());
                assert!(rows.len() <= 64);
                for (index, row) in rows.iter().enumerate() {
                    assert_eq!(row["call_index"], index);
                    let request = projected(&row["request"]);
                    assert_eq!(request["id"], "[REDACTED]");
                    assert_eq!(
                        request["imp"][0]["banner"],
                        json!({"w": 300_i32, "h": 250_i32})
                    );
                    assert_eq!(
                        projected(&row["response"]),
                        json!({"id": "[REDACTED]", "cur": "USD"})
                    );
                }
                assert!(serde_json::to_vec(debug).unwrap().len() <= 64 * 1024);
                let name = format!("engine-{calls}");
                report(
                    &name,
                    "whole-engine-including-assembly",
                    observed.peak,
                    debug_peak_bytes(ceiling).unwrap(),
                );
                writeln!(io::stderr(), "{name}: retained_rows={}", rows.len()).unwrap();
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn cancelling_real_engine_suspended_with_capture_releases_owned_storage() {
        thread::spawn(|| {
            let mut fixture = Engine::new(64, 64, true, vec![]);
            fixture.suspend();
            let observed = fixture.run();
            assert_eq!(observed.dispatched, 64);
            assert_eq!(observed.schema_reads, 1);
            assert!(observed.response.is_none());
            assert!(
                observed.released,
                "cancellation retains no engine/capture heap storage"
            );
            report(
                "cancelled-64",
                "suspended-engine",
                observed.peak,
                debug_peak_bytes(64).unwrap(),
            );
        })
        .join()
        .unwrap();
    }

    #[test]
    fn known_empty_request_and_sealed_204_skip_all_projection_scratch() {
        thread::spawn(|| {
            let sealed = BidderHttpResponse::new(
                204,
                vec![("Content-Type".into(), "CANARY_invalid_mime".into())],
                vec![],
            )
            .within_node_budget(0);
            let baseline = alloc_track::reset_peak();
            let request = project_request_body(
                DebugBodySchema::Unsupported,
                &[],
                [("Content-Type", "CANARY_invalid_mime")],
                0,
            );
            let response = sealed.project_debug_body(DebugBodySchema::Unsupported);
            let peak = alloc_track::peak_since(baseline);
            assert_eq!(request.body_status, DebugBodyStatus::Empty);
            assert_eq!(response.body_status, DebugBodyStatus::Empty);
            assert!(request.body.is_none());
            assert!(response.body.is_none());
            assert_eq!(peak, 0);
        })
        .join()
        .unwrap();
    }

    struct ProjectionClock {
        before: MonotonicInstant,
        deadline: MonotonicInstant,
        reads: AtomicUsize,
    }

    impl Clock for ProjectionClock {
        fn now(&self) -> time::OffsetDateTime {
            time::OffsetDateTime::UNIX_EPOCH
        }

        fn now_monotonic(&self) -> MonotonicInstant {
            if self.reads.fetch_add(1, Ordering::Relaxed) >= 2 {
                self.deadline
            } else {
                self.before
            }
        }
    }

    #[test]
    fn expiry_observed_after_real_request_and_response_projection_releases_capture() {
        for on_response in [false, true] {
            thread::spawn(move || {
                let request = outbound(&json!({"test": 1_i32}), false);
                let result = response(&json!({"cur": "USD"}));
                let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
                let deadline = deadline(&clock);
                let expiring = ProjectionClock {
                    before: clock.origin(),
                    deadline,
                    reads: AtomicUsize::new(0),
                };
                let baseline = alloc_track::live();
                let mut capture = Capture::new(1);
                let mut slot = None;
                if on_response {
                    let pending = capture.prepare_request(
                        &request,
                        DebugBodySchema::OpenRtb,
                        4096,
                        &clock,
                        deadline,
                    );
                    slot = capture.commit_request(
                        0,
                        "fake_",
                        pending,
                        iter::empty(),
                        &clock,
                        deadline,
                    );
                    assert_eq!(slot, Some(0));
                }
                let before = debug_allocation::probes();
                if on_response {
                    capture.complete(slot, &result, DebugBodySchema::OpenRtb, &expiring, deadline);
                } else {
                    let pending = capture.prepare_request(
                        &request,
                        DebugBodySchema::OpenRtb,
                        4096,
                        &expiring,
                        deadline,
                    );
                    assert!(pending.is_none());
                }
                assert_eq!(
                    debug_allocation::probes().writers - before.writers,
                    1,
                    "the real writer ran before the post-projection deadline check"
                );
                assert_eq!(expiring.reads.load(Ordering::Relaxed), 3);
                assert!(
                    capture.finish(&clock, deadline).is_none(),
                    "no partial or false empty success after interrupted capture"
                );
                assert_eq!(alloc_track::live(), baseline);
            })
            .join()
            .unwrap();
        }
    }

    #[test]
    fn collector_reservation_refusal_drops_one_call_and_releases_tentative_storage() {
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let request = outbound(&json!({"test": 1_i32}), false);
        let deadline = deadline(&clock);
        let baseline = alloc_track::live();
        let mut capture = Capture::new(64);
        let pending =
            capture.prepare_request(&request, DebugBodySchema::OpenRtb, 4096, &clock, deadline);
        assert!(pending.is_some());
        let refusal = RefuseNext::arm();
        let slot = capture.commit_request(0, "fake_", pending, iter::empty(), &clock, deadline);
        drop(refusal);
        assert_eq!(slot, None);
        let value = capture.finish(&clock, deadline).unwrap();
        assert_eq!(
            value,
            json!({"httpcalls": {}, "truncated": true, "dropped_calls": 1_i32})
        );
        drop(value);
        assert_eq!(alloc_track::live(), baseline);
    }
}

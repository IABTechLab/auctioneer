//! Sparse extension trees must fit grouping, survivor projection and adapter
//! lifetimes, not just their small wire encoding.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

use auctioneer_exchange::test_support as support;

mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "test-only counting allocator delegates to System"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
        static PEAK: Cell<i64> = const { Cell::new(0) };
    }

    pub fn live() -> i64 {
        LIVE.try_with(Cell::get).unwrap_or(0)
    }

    pub fn reset_peak() {
        let _result = PEAK.try_with(|peak| peak.set(live()));
    }

    pub fn peak_since(before: i64) -> u64 {
        PEAK.try_with(|peak| u64::try_from(peak.get().saturating_sub(before)).unwrap_or(0))
            .unwrap_or(0)
    }

    fn add(bytes: usize) {
        let _result = LIVE.try_with(|live| {
            live.set(
                live.get()
                    .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
            let _result = PEAK.try_with(|peak| peak.set(peak.get().max(live.get())));
        });
    }

    fn sub(bytes: usize) {
        let _result = LIVE.try_with(|live| {
            live.set(
                live.get()
                    .saturating_sub(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    pub struct CountingAllocator;

    // SAFETY: all pointer/layout operations delegate unchanged to System.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller supplied the allocation layout.
            let ptr = unsafe { System.alloc(layout) };
            if !ptr.is_null() {
                add(layout.size());
            }
            ptr
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: the caller supplied the allocation layout.
            let ptr = unsafe { System.alloc_zeroed(layout) };
            if !ptr.is_null() {
                add(layout.size());
            }
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            // SAFETY: the pointer and layout came from a matching allocation.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: the caller supplied the original layout and new size.
            let next = unsafe { System.realloc(ptr, layout, new_size) };
            if !next.is_null() {
                sub(layout.size());
                add(new_size);
            }
            next
        }
    }
}

#[global_allocator]
static COUNTING_ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    // Owner captures are filesystem-backed; synthetic allocation controls below
    // remain compiled for the deployment targets.
    #[cfg(not(target_arch = "wasm32"))]
    mod provider_allocation_conformance;
    use super::alloc_track;
    use crate::fake_bidder::FakeBidder;
    use crate::support::response_budgets;
    use auctioneer_bidder::{
        Bidder as _, BidderName, BidderRegistry, BidderRequestInfo, CallBudget, EntryPoint,
        OutboundRequestSink, SinkLimits, outbound_meta_bytes,
    };
    use auctioneer_config::types::BidderInfo;
    use auctioneer_exchange::scrub::{DisclosureAudit, construction_costs};
    use auctioneer_exchange::split::{GroupedBidders, group_by_bidder, materialize_per_bidder};
    use auctioneer_exchange::validate_endpoint::ExchangeEndpointValidator;
    use auctioneer_exchange::{AliasMap, AuctionError, PerBidderRequest};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::budget::{
        derive_aux_work_for_profile, min_aux_route_peak_bytes, validate_isolate_limits,
    };
    use auctioneer_runtime::{
        MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES,
        MIN_RESPONSE_PARSE_FACTOR, NODE_RETAINED_BYTES, ResolvedIsolateLimits,
        TargetPlatformConstants, bounded_json,
    };
    use std::collections::BTreeSet;
    use std::hint::black_box;
    use std::sync::LazyLock;
    use std::thread;

    const BODY_CAP: u64 = 0x0004_0000;
    const NODE_CAP: u32 = 0x0001_0000;
    static SYNTHETIC_BIDDER: LazyLock<BidderInfo> = LazyLock::new(|| BidderInfo {
        name: "fake_aaa".into(),
        endpoint: "https://bidder.example.test/openrtb".into(),
        uses_https: true,
        ..BidderInfo::default()
    });

    fn sparse_body(impressions: u16, supplement: bool) -> String {
        sparse_body_with_chains(impressions, supplement, 270)
    }

    fn sparse_body_with_chains(impressions: u16, supplement: bool, chain_count: usize) -> String {
        let chain = format!("{}0{}", "{\"\":".repeat(120), "}".repeat(120));
        let chains = vec![chain; chain_count].join(",");
        let imps = (0..impressions)
            .map(|i| format!(
                "{{\"id\":\"i{i}\",\"banner\":{{\"w\":300,\"h\":250}},\"ext\":{{\"prebid\":{{\"bidder\":{{\"fake_aaa\":{{\"fakeParamId\":1}}}}}}}}}}"
            ))
            .collect::<Vec<_>>()
            .join(",");
        let ext = if supplement {
            format!("{{\"prebid\":{{\"bidderparams\":{{\"fake_aaa\":{{\"x\":[{chains}]}}}}}}}}")
        } else {
            format!("{{\"x\":[{chains}]}}")
        };
        format!("{{\"id\":\"r\",\"site\":{{\"id\":\"s\"}},\"imp\":[{imps}],\"ext\":{ext}}}")
    }

    fn count_nodes(value: &serde_json::Value) -> u64 {
        match value {
            serde_json::Value::Array(items) => {
                items.iter().map(count_nodes).fold(1, u64::saturating_add)
            }
            serde_json::Value::Object(entries) => entries.values().fold(1_u64, |total, child| {
                total.saturating_add(1).saturating_add(count_nodes(child))
            }),
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => 1,
        }
    }

    fn sparse_sweep_body(
        impressions: u16,
        supplement: bool,
        projection_capacity: u64,
    ) -> (String, u64) {
        let empty_body = sparse_body_with_chains(impressions, supplement, 0);
        let empty = bounded_json::value_within(empty_body.as_bytes(), NODE_CAP).unwrap();
        let base_nodes = count_nodes(&empty);
        // Each chain costs 120 objects, 120 empty keys and one scalar.
        let chain_nodes = 241_u64;
        let input_chains = u64::from(NODE_CAP)
            .checked_sub(base_nodes)
            .unwrap()
            .checked_div(chain_nodes)
            .unwrap();
        // The supplement's object, x key and array cost three more nodes
        // PER impression. Keep accepted scopes within the unchanged cap;
        // the separate rejection guards retain the full 270-chain fixture.
        let scoped_chains = if supplement {
            u64::from(NODE_CAP)
                .checked_div(u64::from(impressions))
                .unwrap()
                .checked_sub(3)
                .unwrap()
                .checked_div(chain_nodes)
                .unwrap()
        } else {
            input_chains
        };
        // privacy enforcement retains each survivor's full request, including the original
        // supplement and its per-impression copies. Size this context against
        // the existing projection receipt, not bounded fanout's sequential reconstruction.
        let projection_charge = |chain_count| {
            let body = sparse_body_with_chains(impressions, supplement, chain_count);
            let request: BidRequest =
                bounded_json::from_slice_within(body.as_bytes(), NODE_CAP).unwrap();
            let normalized = auctioneer_openrtb::prepare(
                request,
                &auctioneer_openrtb::RequestHeaders::default(),
                &auctioneer_openrtb::PrepareLimits {
                    max_impressions: u32::from(impressions),
                    ..Default::default()
                },
            )
            .unwrap();
            let registry = registry();
            let grouped = group_by_bidder(
                &normalized,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
            grouped
                .projection_charge(&[BidderName::new("fake_aaa")])
                .unwrap()
        };
        // Start at one chain so every later increment includes its comma.
        let first_charge = projection_charge(1);
        let chain_charge = projection_charge(2).checked_sub(first_charge).unwrap();
        let context_chains = projection_capacity
            .checked_sub(first_charge)
            .unwrap()
            .checked_div(chain_charge)
            .unwrap()
            .checked_add(1)
            .unwrap();
        let chains = input_chains.min(scoped_chains).min(context_chains).min(270);
        assert!(chains > 0, "every sweep case must carry sparse trees");
        let sparse_nodes = chains.checked_mul(chain_nodes).unwrap();
        let input_nodes = base_nodes.checked_add(sparse_nodes).unwrap();
        if supplement {
            assert!(
                u64::from(impressions)
                    .checked_mul(sparse_nodes.checked_add(3).unwrap())
                    .unwrap()
                    <= u64::from(NODE_CAP)
            );
        }
        let body =
            sparse_body_with_chains(impressions, supplement, usize::try_from(chains).unwrap());
        let tree = bounded_json::value_within(body.as_bytes(), NODE_CAP).unwrap();
        assert_eq!(count_nodes(&tree), input_nodes);
        (body, sparse_nodes)
    }

    fn row(impressions: u16) -> ResolvedIsolateLimits {
        let mut limits = ResolvedIsolateLimits {
            isolate_total_bytes: u64::MAX,
            baseline_bytes: 27_328_512,
            inbound_request_bytes: BODY_CAP,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: BODY_CAP,
            max_outbound_raw_bytes: BODY_CAP,
            max_transport_bytes: 1_024,
            max_decoded_bytes: 1_024,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 3_072,
            egress_permits: 1,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 1,
            aux_route_peak_bytes: min_aux_route_peak_bytes(1_024, 1_024, 8_192),
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 1,
            max_outbound_requests: 1,
            max_concurrent_auctions: 1,
            max_active_body_streams: 1,
            max_runtime_config_bytes: 1_024,
            max_runtime_config_blob_bytes: 1_024,
            max_parsed_nodes: NODE_CAP,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 0x4000,
            max_admitted_header_count: 128,
            max_impressions_per_auction: impressions,
            max_config_json_nodes: 8_192,
            terminal_response_permits: 384,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        };
        let auxiliary = derive_aux_work_for_profile(&limits, &target(), 0).unwrap();
        limits.aux_route_peak_bytes = limits.aux_route_peak_bytes.max(auxiliary.peak_bytes());
        limits
    }

    fn target() -> TargetPlatformConstants {
        TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        }
    }

    fn registry() -> BidderRegistry {
        let mut registry = BidderRegistry::new();
        registry
            .register(&SYNTHETIC_BIDDER, Box::new(FakeBidder::default()))
            .expect("unique test bidder");
        registry
    }

    fn scoped_fake_aaa(
        grouped: &GroupedBidders,
        registry: &BidderRegistry,
        max_nodes: u32,
        max_bytes: usize,
    ) -> Result<PerBidderRequest, AuctionError> {
        let survivors = [BidderName::new("fake_aaa")];
        grouped.ensure_supplement_capacity(&survivors, max_nodes, max_bytes)?;
        Ok(materialize_per_bidder(
            grouped,
            &survivors,
            &AliasMap::default(),
            registry,
            &BTreeSet::new(),
        )?
        .pop()
        .unwrap())
    }

    fn discarded_competitor_projection(
        slot: &'static str,
        payload_bytes: usize,
    ) -> (u64, u64, Vec<u8>) {
        let registry = registry();
        let mut aliases = AliasMap::default();
        aliases.bases.insert(
            BidderName::new("fake_competitor/~"),
            BidderName::new("fake_aaa"),
        );
        let dropped = BTreeSet::from([BidderName::new("fake_disabled/~")]);
        let payload = serde_json::json!({"payload": "x".repeat(payload_bytes)});
        let mut ext = serde_json::json!({"bidder": {"fake_aaa": {"fakeParamId": 1_i64}}});
        match slot {
            "hoisted" => ext["bidder"]["fake_competitor/~"] = payload,
            "typed" => {
                ext["prebid"] = serde_json::json!({"bidder": {"fake_competitor/~": payload}});
            }
            "fpd" => ext["prebid"] = serde_json::json!({"imp": {"fake_competitor/~": payload}}),
            "legacy-live" => ext["fake_competitor/~"] = payload,
            "legacy-dropped" => ext["fake_disabled/~"] = payload,
            "passthrough" => ext["vendor_hint"] = payload,
            _ => panic!("unknown competitor surface"),
        }
        let request: BidRequest = serde_json::from_value(serde_json::json!({
            "id": "request", "site": {"id": "site"},
            "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32}, "ext": ext}]
        }))
        .unwrap();
        let original = serde_json::to_vec(&request).unwrap();
        let source: serde_json::Value = serde_json::from_slice(&original).unwrap();
        let present = match slot {
            "hoisted" => &source["imp"][0]["ext"]["bidder"]["fake_competitor/~"],
            "typed" => &source["imp"][0]["ext"]["prebid"]["bidder"]["fake_competitor/~"],
            "fpd" => &source["imp"][0]["ext"]["prebid"]["imp"]["fake_competitor/~"],
            "legacy-live" => &source["imp"][0]["ext"]["fake_competitor/~"],
            "legacy-dropped" => &source["imp"][0]["ext"]["fake_disabled/~"],
            "passthrough" => &source["imp"][0]["ext"]["vendor_hint"],
            _ => panic!("unknown competitor projection slot: {slot}"),
        };
        assert_eq!(present["payload"].as_str().unwrap().len(), payload_bytes);
        assert!(original.len() <= usize::try_from(BODY_CAP).unwrap());
        let grouped =
            group_by_bidder(&request, &aliases, &registry, &BTreeSet::new(), &dropped).unwrap();
        let survivors = [BidderName::new("fake_aaa")];
        let before = alloc_track::live();
        alloc_track::reset_peak();
        let scoped =
            materialize_per_bidder(&grouped, &survivors, &aliases, &registry, &dropped).unwrap();
        black_box(&scoped);
        let peak = alloc_track::peak_since(before);
        let retained = u64::try_from(alloc_track::live().checked_sub(before).unwrap()).unwrap();
        assert_eq!(scoped.len(), 1);
        let wire = serde_json::to_vec(&scoped[0].request).unwrap();
        assert_eq!(serde_json::to_vec(&request).unwrap(), original);
        let output: serde_json::Value = serde_json::from_slice(&wire).unwrap();
        let output_ext = &output["imp"][0]["ext"];
        assert_eq!(
            output_ext["bidder"],
            serde_json::json!({"fakeParamId": 1_i64})
        );
        assert!(output_ext.get("fake_competitor/~").is_none());
        assert!(output_ext.get("fake_disabled/~").is_none());
        assert!(output_ext["prebid"].get("bidder").is_none());
        assert!(output_ext["prebid"].get("imp").is_none());
        if slot == "passthrough" {
            assert_eq!(
                output_ext["vendor_hint"]["payload"].as_str().unwrap().len(),
                payload_bytes
            );
        } else {
            assert!(output_ext.get("vendor_hint").is_none());
        }
        (peak, retained, wire)
    }

    #[test]
    #[expect(
        clippy::print_stderr,
        reason = "parent-controlled red/green runs record cross-size allocation evidence"
    )]
    fn discarded_competitor_payload_does_not_scale_survivor_projection() {
        let mut scaled_slots = Vec::new();
        for slot in ["hoisted", "typed", "fpd", "legacy-live", "legacy-dropped"] {
            let scales = thread::spawn(move || {
                let baseline = discarded_competitor_projection(slot, 32);
                let mut scales = false;
                for bytes in [32 * 1024, 128 * 1024] {
                    let measured = discarded_competitor_projection(slot, bytes);
                    eprintln!(
                        "discarded competitor: slot={slot} payload={bytes} baseline_peak={} peak={} baseline_retained={} retained={}",
                        baseline.0, measured.0, baseline.1, measured.1
                    );
                    assert_eq!(
                        measured.2, baseline.2,
                        "{slot}: discarded bytes changed the wire output"
                    );
                    assert_eq!(
                        measured.1, baseline.1,
                        "{slot}: discarded bytes changed retained output"
                    );
                    scales |= measured.0 != baseline.0;
                }
                scales
            })
            .join()
            .expect("isolated competitor projection finishes");
            if scales {
                scaled_slots.push(slot);
            }
        }
        assert!(
            scaled_slots.is_empty(),
            "survivor projection duplicated discarded payload bytes: {scaled_slots:?}"
        );
    }

    #[test]
    #[expect(
        clippy::print_stderr,
        reason = "retained-payload control validates the cross-size allocation measurement"
    )]
    fn retained_passthrough_payload_scales_survivor_projection() {
        thread::spawn(|| {
            let baseline = discarded_competitor_projection("passthrough", 32);
            for bytes in [32 * 1024, 128 * 1024] {
                let measured = discarded_competitor_projection("passthrough", bytes);
                eprintln!(
                    "retained passthrough: payload={bytes} baseline_peak={} peak={} baseline_retained={} retained={}",
                    baseline.0, measured.0, baseline.1, measured.1
                );
                assert!(
                    measured.0 > baseline.0,
                    "retained payload must increase measured peak"
                );
                assert!(
                    measured.1 > baseline.1,
                    "retained payload must increase retained output"
                );
                assert!(
                    measured.2.len() > baseline.2.len(),
                    "retained payload must increase wire bytes"
                );
            }
        })
        .join()
        .expect("isolated retained-payload control finishes");
    }

    fn explicit_identity_rejection_peak(slot: &str) -> u64 {
        let owned_slot = slot.to_owned();
        thread::spawn(move || {
            let name = "a".repeat(65);
            let params = serde_json::json!({
                "fakeParamId": 1_i64,
                "payload": "x".repeat(64 * 1024)
            });
            let mut bidders = serde_json::Map::new();
            bidders.insert(name, params.clone());
            let rejected_ext = match owned_slot.as_str() {
                "bidder" => serde_json::json!({"bidder": bidders}),
                "prebid.bidder" => serde_json::json!({"prebid": {"bidder": bidders}}),
                _ => panic!("unknown explicit bidder slot"),
            };
            let request: BidRequest = serde_json::from_value(serde_json::json!({
                "id": "request",
                "site": {"id": "site"},
                "imp": [
                    {
                        "id": "valid-first",
                        "banner": {"w": 300_i32, "h": 250_i32},
                        "ext": {"bidder": {"fake_aaa": params}}
                    },
                    {
                        "id": "invalid-second",
                        "banner": {"w": 300_i32, "h": 250_i32},
                        "ext": rejected_ext
                    }
                ]
            }))
            .unwrap();
            assert!(
                serde_json::to_vec(&request).unwrap().len() <= usize::try_from(BODY_CAP).unwrap()
            );
            let registry = registry();
            let before = alloc_track::live();
            alloc_track::reset_peak();
            let result = group_by_bidder(
                &request,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            );
            let peak = alloc_track::peak_since(before);
            assert!(matches!(
                result,
                Err(auctioneer_exchange::AuctionError::BadInput(_))
            ));
            peak
        })
        .join()
        .expect("identity preflight measurement finishes")
    }

    #[test]
    fn explicit_bidder_identity_is_rejected_before_request_parameter_copies() {
        let peak = explicit_identity_rejection_peak("bidder");
        assert!(
            peak <= 4096,
            "identity rejection copied parameter trees: {peak}"
        );
    }

    #[test]
    fn explicit_prebid_bidder_identity_is_rejected_before_request_parameter_copies() {
        let peak = explicit_identity_rejection_peak("prebid.bidder");
        assert!(
            peak <= 4096,
            "identity rejection copied parameter trees: {peak}"
        );
    }

    fn assert_reused_survivor_scopes(
        grouped: &GroupedBidders,
        registry: &BidderRegistry,
        funded: &ResolvedIsolateLimits,
        impressions: u16,
    ) {
        let retained_grouping = alloc_track::live();
        // Repeated one-survivor projections reuse the same grouping.
        for _bidder in 0_u8..8_u8 {
            let scoped = scoped_fake_aaa(
                grouped,
                registry,
                funded.max_parsed_nodes,
                usize::try_from(funded.max_outbound_raw_bytes).unwrap(),
            )
            .unwrap();
            assert_eq!(scoped.request.imp.len(), usize::from(impressions));
            black_box(&scoped);
            drop(scoped);
            assert_eq!(alloc_track::live(), retained_grouping);
        }
    }

    #[test]
    #[expect(
        clippy::print_stderr,
        reason = "nocapture records the funded bounds; measured auction lifetimes stay in one isolated thread"
    )]
    fn sparse_grouping_and_survivor_scopes_fit_the_request_charge() {
        for (impressions, supplement) in [1_u16, 8, 100]
            .into_iter()
            .flat_map(|impressions| [false, true].map(move |supplement| (impressions, supplement)))
        {
            let funded = row(impressions);
            let meta_bytes = outbound_meta_bytes(funded.max_impressions_per_auction);
            let charged = validate_isolate_limits(
                &funded,
                &target(),
                meta_bytes,
                construction_costs(),
                auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap();
            let (body, sparse_nodes) =
                sparse_sweep_body(impressions, supplement, charged.auction_charges.projection);
            let body_bytes = u64::try_from(body.len()).unwrap();
            assert!(body_bytes <= BODY_CAP);
            assert_eq!(funded.max_parsed_nodes, NODE_CAP);
            let mut narrowed = funded;
            narrowed.inbound_request_bytes = body_bytes;
            let narrower_charge = validate_isolate_limits(
                &narrowed,
                &target(),
                meta_bytes,
                construction_costs(),
                auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            )
            .unwrap();
            let (measured, retained, projection_charge) = thread::spawn(move || {
                let registry = registry();
                let before = alloc_track::live();
                alloc_track::reset_peak();
                let request: BidRequest =
                    bounded_json::from_slice_within(body.as_bytes(), funded.max_parsed_nodes)
                        .expect("sparse input satisfies the parser's depth and node caps");
                let retained = u64::try_from(alloc_track::live() - before).unwrap();
                assert!(
                    retained
                        > sparse_nodes
                            .checked_mul(NODE_RETAINED_BYTES)
                            .unwrap()
                            .div_ceil(2),
                    "fixture must retain sparse container heap, not just wire contents"
                );
                let normalized = auctioneer_openrtb::prepare(
                    request,
                    &auctioneer_openrtb::RequestHeaders::default(),
                    &auctioneer_openrtb::PrepareLimits {
                        max_impressions: u32::from(funded.max_impressions_per_auction),
                        ..Default::default()
                    },
                )
                .unwrap();
                let underlying: &BidRequest = &normalized;
                let grouped = group_by_bidder(
                    underlying,
                    &AliasMap::default(),
                    &registry,
                    &BTreeSet::new(),
                    &BTreeSet::new(),
                )
                .unwrap();
                let survivors = [BidderName::new("fake_aaa")];
                assert_eq!(grouped.bidder_imps().len(), 1);
                assert_eq!(
                    grouped.bidder_imps().get(&survivors[0]).unwrap().len(),
                    usize::from(impressions)
                );
                grouped
                    .ensure_projection_capacity(&survivors, charged.auction_charges.projection)
                    .expect("funded projection admits every sweep row");
                let projection_charge = grouped.projection_charge(&survivors).unwrap();
                assert_reused_survivor_scopes(&grouped, &registry, &funded, impressions);
                black_box((&normalized, underlying, &grouped));
                (alloc_track::peak_since(before), retained, projection_charge)
            })
            .join()
            .unwrap();
            eprintln!(
                "sparse request: impressions={impressions} supplement={supplement} \
                 wire={body_bytes} sparse_nodes={sparse_nodes} node_cap={} \
                 meta_bytes={meta_bytes} retained={retained} peak={measured} \
                 projection_charge={projection_charge} projection_capacity={} \
                 request_side_term={} narrowed_request_side_term={}",
                funded.max_parsed_nodes,
                charged.auction_charges.projection,
                charged.request_side_term,
                narrower_charge.request_side_term,
            );
            assert!(
                measured <= charged.request_side_term,
                "sparse grouping/scoped peak {measured} exceeds request charge {}",
                charged.request_side_term
            );
            assert!(
                measured <= narrower_charge.request_side_term,
                "narrowing wire bytes must not remove funding for retained sparse nodes"
            );
        }
    }

    #[test]
    fn sparse_supplement_materialization_accepts_exact_node_and_wire_caps() {
        let body = sparse_body(1, true);
        thread::spawn(move || {
            let request: BidRequest =
                bounded_json::from_slice_within(body.as_bytes(), NODE_CAP).unwrap();
            let registry = registry();
            let grouped = group_by_bidder(
                &request,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
            let supplement = &request
                .ext
                .as_ref()
                .unwrap()
                .prebid
                .as_ref()
                .unwrap()
                .bidderparams["fake_aaa"];
            let exact_wire_bytes = serde_json::to_vec(supplement).unwrap().len();
            // Object + key + array, then 270 chains of 120 objects,
            // 120 empty keys and one scalar. Wire bytes include all JSON syntax.
            let exact_nodes = 3 + 270 * 241;
            let scoped =
                scoped_fake_aaa(&grouped, &registry, exact_nodes, exact_wire_bytes).unwrap();
            assert!(
                scoped.request.imp[0]
                    .ext
                    .as_ref()
                    .unwrap()
                    .bidder
                    .as_ref()
                    .unwrap()
                    .get("x")
                    .is_some()
            );
            drop(scoped);
            for (nodes, bytes) in [
                (exact_nodes - 1, exact_wire_bytes),
                (exact_nodes, exact_wire_bytes - 1),
            ] {
                let before = alloc_track::live();
                alloc_track::reset_peak();
                let error = scoped_fake_aaa(&grouped, &registry, nodes, bytes).unwrap_err();
                assert_eq!(error.code(), 2);
                assert!(
                    alloc_track::peak_since(before) < 0x4000,
                    "rejection must precede scoped allocation"
                );
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn copied_advisory_headers_on_borrowed_fake_path_fit_the_funded_charge() {
        fn peak_with_headers(copy_cap: u32) -> u64 {
            let body = sparse_body(1, false);
            thread::spawn(move || {
                let ua = "u".repeat(0x0001_0000);
                let referer = format!("https://example.test/{}", "p".repeat(0x0001_0000));
                let registry = registry();
                let before = alloc_track::live();
                alloc_track::reset_peak();
                let request: BidRequest =
                    bounded_json::from_slice_within(body.as_bytes(), NODE_CAP).unwrap();
                let normalized = auctioneer_openrtb::prepare(
                    request,
                    &auctioneer_openrtb::RequestHeaders {
                        user_agent: Some(&ua),
                        referer: Some(&referer),
                        ..Default::default()
                    },
                    &auctioneer_openrtb::PrepareLimits {
                        max_copied_header_bytes: copy_cap,
                        ..Default::default()
                    },
                )
                .unwrap();
                let underlying: &BidRequest = &normalized;
                let grouped = group_by_bidder(
                    underlying,
                    &AliasMap::default(),
                    &registry,
                    &BTreeSet::new(),
                    &BTreeSet::new(),
                )
                .unwrap();
                let scoped = scoped_fake_aaa(
                    &grouped,
                    &registry,
                    NODE_CAP,
                    usize::try_from(BODY_CAP).unwrap(),
                )
                .unwrap();
                let info = BidderRequestInfo::new(
                    BidderName::new("fake_aaa"),
                    BidderName::new("fake_aaa"),
                    "acct",
                    None,
                    &SYNTHETIC_BIDDER.endpoint,
                    &[],
                    None,
                    None,
                    EntryPoint::Auction,
                    &[],
                );
                let budget = CallBudget::new(1);
                let validator = ExchangeEndpointValidator::new(&SYNTHETIC_BIDDER);
                let mut sink = OutboundRequestSink::new(
                    SinkLimits::new(usize::try_from(BODY_CAP).unwrap(), 100),
                    &budget,
                    &validator,
                );
                FakeBidder::default()
                    .make_requests(&scoped.request, &info, &mut sink)
                    .unwrap();
                let outcome = sink.into_outcome();
                assert_eq!(outcome.accepted.len(), 1);
                black_box((&normalized, underlying, &grouped, &scoped, &outcome));
                alloc_track::peak_since(before)
            })
            .join()
            .unwrap()
        }
        let copied = 0x0001_0000;
        let baseline = peak_with_headers(0);
        let with_headers = peak_with_headers(copied);
        let extra = with_headers
            .checked_sub(baseline)
            .expect("copied headers must not reduce the request peak");
        // Normalized, grouped Value and scoped request retain the headers;
        // underlying is only a reference. The fake borrows the scope; the sink's fixed-capacity body
        // buffer is allocated in both runs, so it adds no header-dependent copy.
        let three_copies = 3 * u64::from(copied);
        assert!(
            (three_copies..=three_copies + 4_096).contains(&extra),
            "borrowed fake path must retain three header copies plus field/container \
             overhead: baseline {baseline}, with headers {with_headers}, extra {extra}"
        );
        // Production still funds five copies; this optimization does not lower it.
        assert!(
            extra <= 5 * u64::from(copied) + 4_096,
            "advisory peak {extra} must fit the funded five-copy bound plus overhead"
        );
    }

    #[test]
    fn oversized_sparse_supplement_is_refused_before_scope_allocation() {
        let body = sparse_body(3, true);
        thread::spawn(move || {
            let request: BidRequest =
                bounded_json::from_slice_within(body.as_bytes(), NODE_CAP).unwrap();
            let registry = registry();
            let grouped = group_by_bidder(
                &request,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
            let before = alloc_track::live();
            alloc_track::reset_peak();
            let result = scoped_fake_aaa(
                &grouped,
                &registry,
                NODE_CAP,
                usize::try_from(BODY_CAP).unwrap(),
            );
            assert!(
                result.is_err(),
                "within-cap input must not amplify unbounded node heap in the scope"
            );
            assert!(
                alloc_track::peak_since(before) < 0x4000,
                "a rejected supplement must not allocate its scoped tree first"
            );
        })
        .join()
        .unwrap();
    }

    #[test]
    fn fake_serialization_keeps_the_sparse_extension_within_the_funded_bound() {
        let body = sparse_body(1, false);
        thread::spawn(move || {
            let request: BidRequest = bounded_json::from_slice_within(body.as_bytes(), NODE_CAP).unwrap();
            let registry = registry();
            let grouped = group_by_bidder(
                &request, &AliasMap::default(), &registry, &BTreeSet::new(), &BTreeSet::new(),
            ).unwrap();
            let scoped = scoped_fake_aaa(&grouped, &registry, NODE_CAP, usize::try_from(BODY_CAP).unwrap()).unwrap();
            let info = BidderRequestInfo::new(
                BidderName::new("fake_aaa"), BidderName::new("fake_aaa"), "acct", None,
                &SYNTHETIC_BIDDER.endpoint, &[], None, None, EntryPoint::Auction, &[],
            );
            let budget = CallBudget::new(1);
            let validator = ExchangeEndpointValidator::new(&SYNTHETIC_BIDDER);
            let before = alloc_track::live();
            alloc_track::reset_peak();
            let mut sink = OutboundRequestSink::new(SinkLimits::new(usize::try_from(BODY_CAP).unwrap(), 100), &budget, &validator);
            FakeBidder::default().make_requests(&scoped.request, &info, &mut sink).unwrap();
            let outcome = sink.into_outcome();
            let measured = alloc_track::peak_since(before);
            let one_tree_and_sink = u64::from(NODE_CAP) * NODE_RETAINED_BYTES + 4 * BODY_CAP + 0x4000;
            assert_eq!(outcome.accepted.len(), 1);
            assert!(measured <= one_tree_and_sink,
                "fake serialization peak {measured} exceeds one sparse tree plus sink {one_tree_and_sink}");
            assert!(!scoped.request.ext.as_ref().unwrap().other.contains_key("fake_aaa"));
        }).join().unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "allocation setup and measured auction lifetimes must stay in the same isolated thread"
    )]
    fn denied_alias_status_rows_are_bounded_before_impression_ids_are_copied() {
        use async_trait::async_trait;
        use auctioneer_config::types::{AccountConfig, HostConfig, PurposeEnforcement, TriState};
        use auctioneer_exchange::{
            AccountSource, AuctionContext, AuctionDeadline, AuctionPlan, BidderCall,
            BidderCallResult, BidderFanout, ExchangeFeatures, FanoutInternalError, ResolvedAccount,
            resolve_aliases, run_auction,
        };
        use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
        use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
        use futures::executor::block_on;
        use std::collections::BTreeMap;

        struct NoDispatch;
        #[async_trait(?Send)]
        impl BidderFanout for NoDispatch {
            async fn send_all(
                &self,
                _calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                panic!("privacy-denied aliases must not dispatch");
            }
        }

        for (alias_count, imp_bytes, return_status, refused) in [
            (1_000_u16, 0x8000, true, true),
            (61, 4_096, true, false),
            (61, 4_096, false, false),
        ] {
            thread::spawn(move || {
            let aliases = (0_u16..alias_count)
                .map(|i| (format!("fake_a{i}"), serde_json::json!("fake_aaa")))
                .collect::<serde_json::Map<_, _>>();
            let params = aliases
                .keys()
                .map(|name| (name.clone(), serde_json::json!({})))
                .collect::<serde_json::Map<_, _>>();
            let body = serde_json::to_vec(&serde_json::json!({
                "id": "denied-status",
                "regs": {"gdpr": 1_i32},
                "imp": [{"id": "i".repeat(imp_bytes), "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"prebid": {"bidder": params}}}],
                "ext": {"prebid": {"aliases": aliases, "returnallbidstatus": return_status}}
            }))
            .unwrap();
            assert!(u64::try_from(body.len()).unwrap() < BODY_CAP);
            let request: BidRequest = bounded_json::from_slice_within(&body, 8_192).unwrap();
            let normalized = auctioneer_openrtb::prepare(
                request,
                &auctioneer_openrtb::RequestHeaders::default(),
                &auctioneer_openrtb::PrepareLimits::default(),
            )
            .unwrap();
            let registry = registry();
            let mut host = HostConfig::default();
            host.resource_limits.default_tmax_ms = 1_000;
            host.resource_limits.max_tmax_ms = 5_000;
            host.resource_limits.max_bidders_per_auction = 1;
            host.resource_limits.max_outbound_requests = 1;
            host.resource_limits.max_bidder_response_bytes = 1_024;
            host.resource_limits.max_decoded_bytes = 1_024;
            host.resource_limits.max_transport_bytes = 1_024;
            host.resource_limits.max_parsed_nodes = 8_192;
            host.resource_limits.max_outbound_raw_bytes = BODY_CAP;
            host.gdpr.enabled = true;
            host.gdpr.tcf2.enabled = true;
            host.gdpr.tcf2.purposes.insert(
                2,
                PurposeEnforcement {
                    enforce_purpose: TriState::Enabled,
                    ..PurposeEnforcement::default()
                },
            );
            let account = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "acct".into(),
                source: AccountSource::DefaultFallback,
                config: &account,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng = SeededRng::new(42);
            let timeouts = BTreeMap::new();
            let ctx = AuctionContext::with_request_start_ext(
                clock.now_monotonic(),
                &host,
                &resolved,
                &registry,
                &NoDispatch,
                &clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                clock.now(),
                false,
                ExchangeFeatures::default(),
                &timeouts,
            );
            let resolved_aliases =
                resolve_aliases(&normalized, &registry, &BTreeSet::new(), &BTreeMap::new())
                    .unwrap();
            let plan = AuctionPlan::new(
                resolved_aliases,
                BTreeSet::new(),
                response_budgets(&registry, &host),
                BTreeMap::new(),
            );
            let before = alloc_track::live();
            alloc_track::reset_peak();
            let mut audit = DisclosureAudit::default();
            let result = block_on(run_auction(&normalized, &ctx, &plan, &mut audit));
            let peak = alloc_track::peak_since(before);
            if refused {
                assert!(result.is_err(), "status bookkeeping must be refused before amplification");
                assert_eq!(result.err().unwrap().code(), 2);
                assert!(peak < 4 * BODY_CAP, "refusal allocated {peak} bytes before deciding");
            } else {
                let response = result.unwrap();
                let ext = response.ext.as_ref().unwrap();
                let prebid = &ext["prebid"];
                assert_eq!(ext["warnings"].as_object().unwrap().len(), usize::from(alias_count));
                if return_status {
                    assert_eq!(prebid["seatnonbid"].as_array().unwrap().len(), usize::from(alias_count));
                } else {
                    assert!(prebid.get("seatnonbid").is_none());
                }
                let bookkeeping_charge = 4 * (BODY_CAP + 8_192 * NODE_RETAINED_BYTES);
                assert!(peak <= bookkeeping_charge,
                    "accepted status peak {peak} exceeds its separately funded charge {bookkeeping_charge}");
            }
        })
        .join()
        .unwrap();
        }
    }
}

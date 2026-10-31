//! Native allocation evidence for the consuming projection boundary and a
//! production classified in-place mutator with caller-owned audit counters.
//! Grouping overlaps the retained scoped vector and its current Value/decode;
//! after grouping drops, all scopes overlap mutation and retained capped bodies.
//! Uses the public budget equation with actual exchange construction costs and
//! synthetic native rows. Installed adapter rows and core audit ownership are
//! qualified elsewhere; this does not qualify WASM or transport/finalization.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[cfg(test)]
#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track;
    use crate::fake_bidder::{self, FakeBidder};
    use auctioneer_bidder::{
        Bidder as _, BidderName, BidderRegistry, BidderRequestInfo, CallBudget, EntryPoint,
        OutboundRequestSink, SinkLimits, outbound_meta_bytes,
    };
    use auctioneer_config::types::{BidderInfo, IpKeepBits};
    use auctioneer_exchange::{
        context::RequestConstructionLimits,
        resolve_aliases,
        scrub::{DisclosureAudit, ScrubDecisions, apply_classified, construction_costs},
        split::{group_by_bidder, materialize_per_bidder},
        validate_endpoint::ExchangeEndpointValidator,
    };
    use auctioneer_openrtb::{PrepareLimits, RequestHeaders, types::bid_request::BidRequest};
    use auctioneer_runtime::budget::derive_aux_work_for_profile;
    use auctioneer_runtime::{
        AuctionBudget, ChargeKind, Derived, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NORMALIZATION_IMPLICIT_NODES,
        ResolvedIsolateLimits, ResourceError, TargetPlatformConstants, count_json_nodes,
        derive_isolate_budget_with_baked_config, min_aux_route_peak_bytes,
        normalization_wire_allowance, serialized_json_bytes,
    };
    use serde_json::{Map, Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::hint::black_box;
    use std::mem::size_of;
    use std::thread;

    const BODY_CAP: u64 = 0x0004_0000;
    const NODE_CAP: u32 = 0x0001_0000;
    // All four current adapter envelopes admit up to 50 scoped bidders.
    const MAX_SURVIVORS: u32 = 50;

    fn registry() -> (BidderRegistry, &'static BidderInfo) {
        let info = fake_bidder::info();
        let mut registry = BidderRegistry::new();
        registry
            .register(info, Box::new(FakeBidder::default()))
            .expect("unique test bidder");
        (registry, info)
    }

    fn fixture(width: usize, brands: usize) -> Value {
        let aliases: Map<_, _> = (0..MAX_SURVIVORS)
            .map(|n| (format!("fake_a{n}"), json!("fake_bb")))
            .collect();
        let params: Map<_, _> = aliases
            .keys()
            .map(|name| (name.clone(), json!({})))
            .collect();
        let unknown: Map<_, _> = (0..width)
            .map(|n| (format!("unknown{n}"), json!({"uid2": "secret"})))
            .collect();
        json!({
            "id": "request", "at": 2_i32,
            "site": {"id": "site", "ext": {"amp": 0_i32}},
            "imp": [{"id": "imp", "banner": {"w": 300_i32, "h": 250_i32,
                "ext": {"unrelated": "b".repeat(width)}},
                "ext": {"bidder": params}}],
            "ext": {"prebid": {"aliases": aliases}},
            "opaque": vec![json!({"context": []}); width],
            "user": {"id": "private-user", "kwarray": ["news"], "unknown": unknown,
                "geo": {"country": "GBR", "city": "London", "metro": "market",
                    "lat": 51.5074_f64, "lon": -0.1278_f64, "accuracy": 1_i32,
                    "zip": "precise", "ext": {"denied": "g".repeat(width)}}},
            "device": {"ifa": "private-device", "dnt": 1_i32, "lmt": 0_i32,
                "h": null, "pxratio": 2_i32,
                "sua": {"browsers": vec![json!({}); brands], "platform": {},
                    "ext": {"uid2": "secret"}, "unknown": {"uid2": "secret"}},
                "ext": {"atts": 2_i32, "prebid": {"interstitial": {
                    "minwidthperc": 70_i32, "minheightperc": 60_i32,
                    "unknown": {"uid2": "secret"}}}}}
        })
    }

    fn row(survivors: u32) -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
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
            final_response_bytes: u64::from(survivors).checked_mul(3_072).unwrap(),
            egress_permits: 1,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 1,
            aux_route_peak_bytes: min_aux_route_peak_bytes(1_024, 1_024, 8_192),
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: survivors,
            max_outbound_requests: survivors,
            max_concurrent_auctions: 1,
            max_active_body_streams: 1,
            max_runtime_config_bytes: 1_024,
            max_runtime_config_blob_bytes: 1_024,
            max_parsed_nodes: NODE_CAP,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 0x4000,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 1,
            max_config_json_nodes: 8_192,
            terminal_response_permits: 384,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        }
    }

    fn derive(limits: &ResolvedIsolateLimits) -> Derived {
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: 0x0010_0000,
            egress_handoff_factor: 2,
        };
        // The public equation used by install_boot_envelope, including actual
        // baked configuration sizing; this binary does not install core globals.
        // Parser transients match Axum's 1 MiB, but this is still a synthetic
        // descriptor; adapter envelope APIs are not dependencies here.
        let baked = serialized_json_bytes(&(
            auctioneer_config::host_default(),
            auctioneer_config::account_default(),
        ))
        .unwrap();
        // This synthetic row proves auction phases, but must also fund M5's
        // independent auxiliary privacy/config peak before the row can derive.
        let mut funded_limits = *limits;
        let auxiliary = derive_aux_work_for_profile(&funded_limits, &target, baked).unwrap();
        funded_limits.aux_route_peak_bytes = funded_limits
            .aux_route_peak_bytes
            .max(auxiliary.peak_bytes());
        derive_isolate_budget_with_baked_config(
            &funded_limits,
            &target,
            outbound_meta_bytes(limits.max_impressions_per_auction),
            construction_costs(),
            baked,
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap()
    }

    fn retained_since(before: i64) -> u64 {
        u64::try_from(alloc_track::live().checked_sub(before).unwrap()).unwrap()
    }

    fn assert_sparse(request: &BidRequest, brands: usize) {
        let sua = request.device.as_ref().unwrap().sua.as_ref().unwrap();
        let browsers = sua.browsers.as_ref().unwrap();
        assert_eq!(browsers.len(), brands);
        assert!(browsers.iter().all(|brand| brand.fields().is_none()));
        assert!(sua.platform.as_ref().unwrap().fields().is_none());
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the measured source, scoped vector and body lifetimes must stay together"
    )]
    #[expect(
        clippy::cognitive_complexity,
        reason = "keep measured lifetimes and subsequent wire assertions in one scope"
    )]
    fn measure(
        width: usize,
        count: u32,
        brands: usize,
        pass_id: bool,
        pass_geo: bool,
        clone_before_mutation: bool,
    ) -> u64 {
        let wire = serde_json::to_vec(&fixture(width, brands)).unwrap();
        let limits = row(count);
        assert!(u64::try_from(wire.len()).unwrap() <= limits.inbound_request_bytes);
        let nodes = count_json_nodes(&wire, limits.max_parsed_nodes).unwrap();
        let funded = derive(&limits);
        assert_eq!(funded.construction_costs, construction_costs());
        assert_eq!(
            funded.auction_charges.total().unwrap(),
            funded.per_auction_peak
        );
        let mut narrow = limits;
        narrow.inbound_request_bytes = u64::try_from(wire.len()).unwrap();
        narrow.max_parsed_nodes = nodes;
        let narrowed = derive(&narrow);

        thread::spawn(move || {
            let (registry, bidder_info) = registry();
            let disabled = BTreeSet::new();
            let budget = AuctionBudget::new(funded.per_auction_peak);
            let receipts = funded.auction_charges.prepay(&budget).unwrap();
            let projection = receipts
                .iter()
                .find(|r| r.kind() == ChargeKind::Projection)
                .unwrap();
            // Audit ownership starts after all phase charges have been prepaid.
            let mut audit = DisclosureAudit::default();

            let before = alloc_track::reset_peak();
            // The outer wire is fixture ownership, outside the auction window.
            // Allocate the actual retained inbound body inside the window.
            let input = wire.clone();
            count_json_nodes(&input, limits.max_parsed_nodes).unwrap();
            // Match ingress's node preflight followed by one typed wire decode:
            // from_slice_within cannot admit these opaque RawValue extensions.
            let request: BidRequest = serde_json::from_slice(&input).unwrap();
            let normalized = auctioneer_openrtb::prepare(
                request,
                &RequestHeaders::default(),
                &PrepareLimits::default(),
            )
            .unwrap();
            let source: &BidRequest = &normalized;
            RequestConstructionLimits {
                max_nodes: u64::from(limits.max_parsed_nodes)
                    .checked_add(NORMALIZATION_IMPLICIT_NODES)
                    .unwrap(),
                max_serialized_bytes: normalization_wire_allowance(
                    limits.inbound_request_bytes,
                    u64::from(limits.max_copied_header_bytes),
                    u64::from(limits.max_parsed_nodes),
                )
                .unwrap(),
                reusable_scratch_bytes: funded.construction_costs.reusable_scratch_bytes,
                retained_per_scope_bytes: funded.construction_costs.retained_per_scope_bytes,
            }
            .ensure(source)
            .unwrap();
            let aliases = resolve_aliases(source, &registry, &disabled, &BTreeMap::new()).unwrap();
            let grouped = group_by_bidder(
                source,
                &aliases.map,
                &registry,
                &disabled,
                &aliases.dropped_alias_names,
            )
            .unwrap();
            let survivors: Vec<_> = grouped
                .bidder_imps()
                .keys()
                .take(usize::try_from(count).unwrap())
                .cloned()
                .collect();
            assert_eq!(survivors.len(), usize::try_from(count).unwrap());
            let source_grouped_peak = alloc_track::peak_since(before);
            assert!(source_grouped_peak <= narrowed.auction_charges.ingress);

            grouped
                .ensure_supplement_capacity(
                    &survivors,
                    limits.max_parsed_nodes,
                    funded.max_outbound_raw_bytes_usize,
                )
                .unwrap();
            let charge = grouped.projection_charge(&survivors).unwrap();
            let retained_growth = u64::try_from(survivors.len())
                .unwrap()
                .checked_mul(funded.construction_costs.retained_per_scope_bytes)
                .unwrap();
            projection
                .ensure_capacity(
                    ChargeKind::Projection,
                    charge
                        .checked_add(retained_growth)
                        .unwrap()
                        .checked_add(funded.construction_costs.reusable_scratch_bytes)
                        .unwrap(),
                )
                .unwrap();
            grouped
                .ensure_projection_capacity(&survivors, funded.auction_charges.projection)
                .unwrap();
            let scope_before = alloc_track::reset_peak();
            let mut scoped = materialize_per_bidder(
                &grouped,
                &survivors,
                &aliases.map,
                &registry,
                &aliases.dropped_alias_names,
            )
            .unwrap();
            let scoped_peak = alloc_track::peak_since(scope_before);
            let scoped_retained = retained_since(scope_before);
            assert_eq!(scoped.len(), survivors.len());
            assert!(scoped_retained > 0);
            assert!(scoped_peak >= scoped_retained);
            assert!(scoped_peak.checked_mul(6).unwrap().div_ceil(5) <= charge);
            projection
                .ensure_capacity(ChargeKind::Projection, scoped_peak)
                .unwrap();
            let mut request_peak = source_grouped_peak.max(alloc_track::peak_since(before));
            // Same ordering as run_auction: complete the vector's consuming
            // decode/validation before dropping grouping or mutating any scope.
            black_box((&normalized, &grouped, &scoped));
            drop(survivors);
            drop(grouped);

            let mut mutation_peak = 0;
            for scope in &mut scoped {
                assert_eq!(
                    scope.request.user.as_ref().unwrap().id.as_deref(),
                    Some("private-user"),
                    "previously mutated siblings must not change this scope"
                );
                assert_sparse(&scope.request, brands);
                let mutation_before = alloc_track::reset_peak();
                // Positive control models the forbidden extra request copy,
                // held through cleanup instead of being optimized away.
                let copy = clone_before_mutation.then(|| scope.request.clone());
                apply_classified(
                    &mut scope.request,
                    ScrubDecisions {
                        pass_id,
                        pass_geo,
                        transmit_ufpd: true,
                        transmit_precise_geo: true,
                        transmit_tid: true,
                        ccpa: false,
                        lmt: false,
                        coppa: false,
                    },
                    IpKeepBits { v4: 24, v6: 56 },
                    &mut audit,
                )
                .unwrap();
                black_box((&copy, &scope.request, &audit));
                mutation_peak = mutation_peak.max(alloc_track::peak_since(mutation_before));
                request_peak = request_peak.max(alloc_track::peak_since(before));
                drop(copy);
                assert_sparse(&scope.request, brands);
            }
            assert!(request_peak <= narrowed.request_side_term);

            let calls = CallBudget::new(limits.max_outbound_requests);
            let adapter_before = alloc_track::reset_peak();
            let mut bodies = Vec::with_capacity(scoped.len());
            for scope in &scoped {
                let info = BidderRequestInfo::new(
                    scope.bidder.clone(),
                    scope.base.clone(),
                    "acct",
                    None,
                    &bidder_info.endpoint,
                    &[],
                    None,
                    None,
                    EntryPoint::Auction,
                    &[],
                );
                let validator = ExchangeEndpointValidator::new(bidder_info);
                let mut sink = OutboundRequestSink::new(
                    SinkLimits::new(
                        funded.max_outbound_raw_bytes_usize,
                        limits.max_impressions_per_auction,
                    ),
                    &calls,
                    &validator,
                );
                FakeBidder::default()
                    .make_requests(&scope.request, &info, &mut sink)
                    .unwrap();
                let mut outcome = sink.into_outcome();
                assert!(outcome.errors.is_empty());
                assert_eq!(outcome.accepted.len(), 1);
                assert_eq!(outcome.rejected_call_count, 0);
                bodies.push(outcome.accepted.pop().unwrap());
            }
            assert_eq!(calls.remaining(), 0);
            black_box((
                &input,
                &normalized,
                &aliases,
                &scoped,
                &bodies,
                &audit,
                &receipts,
            ));
            let adapter_peak = alloc_track::peak_since(adapter_before);
            let full_peak = request_peak.max(alloc_track::peak_since(before));
            assert!(adapter_peak <= funded.auction_charges.outbound);
            assert!(
                full_peak
                    <= narrowed
                        .request_side_term
                        .checked_add(funded.auction_charges.outbound)
                        .unwrap()
            );
            assert_eq!(budget.in_use(), funded.per_auction_peak);
            assert_eq!(audit.attempted_scopes, u64::from(count));
            assert_eq!(audit.failed_scopes, 0);
            assert_eq!(audit.counts() == [0; 8], pass_id && pass_geo);

            // Semantic inspections allocate only after the lifetime readings.
            for (scope, body) in scoped.iter().zip(&bodies) {
                let value: Value = serde_json::from_slice(body.body()).unwrap();
                assert_eq!(value["opaque"].as_array().unwrap().len(), width);
                assert_eq!(
                    value["imp"][0]["banner"]["ext"]["unrelated"]
                        .as_str()
                        .unwrap()
                        .len(),
                    width
                );
                assert_eq!(value["device"].get("h"), Some(&Value::Null));
                assert_eq!(value["device"]["pxratio"], json!(2_i32));
                assert_eq!(value["user"]["kwarray"], json!(["news"]));
                if !pass_id || !pass_geo {
                    assert!(value["user"].get("unknown").is_none());
                    assert!(value["user"]["geo"].get("ext").is_none());
                    assert!(value["device"]["sua"].get("ext").is_none());
                    assert!(
                        value["device"]["ext"]["prebid"]["interstitial"]
                            .get("unknown")
                            .is_none()
                    );
                    assert_eq!(value["user"]["geo"]["city"], "London");
                    assert_eq!(value["user"]["geo"]["metro"], "market");
                }
                if !pass_id {
                    assert!(value["user"].get("id").is_none());
                    assert!(value["device"].get("ifa").is_none());
                }
                if !pass_geo {
                    assert_eq!(value["user"]["geo"]["lat"], json!(51.51_f64));
                    assert_eq!(value["user"]["geo"]["lon"], json!(-0.13_f64));
                    assert!(value["user"]["geo"].get("accuracy").is_none());
                    assert!(value["user"]["geo"].get("zip").is_none());
                }
                if pass_id && pass_geo {
                    assert_eq!(body.body(), serde_json::to_vec(&scope.request).unwrap());
                    assert!(
                        scope
                            .request
                            .user
                            .as_ref()
                            .unwrap()
                            .other
                            .contains_key("unknown")
                    );
                }
            }
            assert_eq!(
                source.user.as_ref().unwrap().id.as_deref(),
                Some("private-user")
            );
            assert!(source.user.as_ref().unwrap().other.contains_key("unknown"));
            assert!(
                source
                    .user
                    .as_ref()
                    .unwrap()
                    .geo
                    .as_ref()
                    .unwrap()
                    .ext
                    .is_some()
            );
            drop(bodies);
            drop(scoped);
            drop(aliases);
            drop(normalized);
            drop(input);
            black_box(&audit);
            drop(receipts);
            assert_eq!(budget.in_use(), 0);
            mutation_peak
        })
        .join()
        .unwrap()
    }

    #[test]
    fn exchange_construction_layouts_follow_auction_and_scope_ownership() {
        use auctioneer_runtime::budget::{
            PRIVACY_FIXED_BYTES, PRIVACY_REQUEST_FACTOR, TYPED_CLONE_FACTOR,
        };

        let costs = construction_costs();
        assert_eq!(
            costs.fixed_per_auction_bytes,
            u64::try_from(size_of::<DisclosureAudit>()).unwrap()
        );
        assert!(costs.reusable_scratch_bytes > 0);
        assert_eq!(costs.retained_per_scope_bytes, 2 * 39);
        for count in [1, 2, MAX_SURVIVORS] {
            let limits = row(count);
            let funded = derive(&limits);
            let normalized = normalization_wire_allowance(
                limits.inbound_request_bytes,
                u64::from(limits.max_copied_header_bytes),
                u64::from(limits.max_parsed_nodes),
            )
            .unwrap();
            let retained_projections = normalized
                .checked_mul(TYPED_CLONE_FACTOR)
                .unwrap()
                .checked_add(2 * 39)
                .unwrap()
                .checked_mul(u64::from(count))
                .unwrap();
            assert_eq!(
                funded.auction_charges.projection,
                retained_projections
                    .checked_add(costs.reusable_scratch_bytes)
                    .unwrap()
            );
            assert_eq!(
                funded.auction_charges.privacy,
                PRIVACY_FIXED_BYTES
                    .checked_add(
                        limits
                            .inbound_request_bytes
                            .checked_mul(PRIVACY_REQUEST_FACTOR)
                            .unwrap()
                    )
                    .unwrap()
                    .checked_add(costs.fixed_per_auction_bytes)
                    .unwrap()
            );
            assert_eq!(
                funded.auction_charges.total().unwrap(),
                funded.per_auction_peak
            );
            let budget = AuctionBudget::new(funded.per_auction_peak);
            let receipts = funded.auction_charges.prepay(&budget).unwrap();
            assert_eq!(budget.in_use(), funded.per_auction_peak);
            drop(receipts);
            assert_eq!(budget.in_use(), 0);
        }
    }

    #[test]
    fn retained_scopes_mutation_and_capped_bodies_fit_derived_phase_charges() {
        for width in [32, 256, 2048] {
            for count in [1, 2, MAX_SURVIVORS] {
                for (pass_id, pass_geo) in
                    [(false, true), (true, false), (false, false), (true, true)]
                {
                    assert!(
                        measure(width, count, 64, pass_id, pass_geo, false)
                            <= construction_costs().reusable_scratch_bytes
                    );
                }
            }
        }
    }

    #[test]
    fn sparse_brand_boundaries_keep_empty_backing_fields_allocation_free() {
        for brands in [64, 512, 513] {
            for count in [1, 2, MAX_SURVIVORS] {
                assert_eq!(measure(256, count, brands, false, false, false), 0);
            }
        }
    }

    fn fits_in_place_growth(small: u64, large: u64) -> bool {
        // Measure above the already-retained vector, before body construction:
        // a fixed capped body must not hide an extra input-scaled request copy.
        large <= small
    }

    #[test]
    fn cross_size_mutation_bound_rejects_an_additional_whole_request_copy() {
        for count in [1, 2, MAX_SURVIVORS] {
            for (pass_id, pass_geo) in [(false, true), (true, false), (false, false), (true, true)]
            {
                let actual = [32, 256, 2048]
                    .map(|width| measure(width, count, 64, pass_id, pass_geo, false));
                let copied =
                    [32, 256, 2048].map(|width| measure(width, count, 64, pass_id, pass_geo, true));
                for pair in [0, 1] {
                    assert!(
                        fits_in_place_growth(actual[pair], actual[pair + 1]),
                        "count={count}, gates=({pass_id},{pass_geo}): scratch {actual:?}"
                    );
                    assert!(
                        !fits_in_place_growth(copied[pair], copied[pair + 1]),
                        "count={count}, gates=({pass_id},{pass_geo}): retained clone {copied:?}"
                    );
                    assert!(copied[pair + 1].checked_sub(copied[pair]).unwrap() > 0);
                }
            }
        }
    }

    #[test]
    fn mutation_scratch_does_not_scale_with_retained_survivor_count() {
        for width in [32, 256, 2048] {
            for (pass_id, pass_geo) in [(false, true), (true, false), (false, false), (true, true)]
            {
                let actual = [1, 2, MAX_SURVIVORS]
                    .map(|count| measure(width, count, 64, pass_id, pass_geo, false));
                assert!(fits_in_place_growth(actual[0], actual[1]));
                assert!(fits_in_place_growth(actual[1], actual[2]));
            }
        }
    }

    #[test]
    fn independent_supplement_caps_refuse_before_projection_and_keep_input_immutable() {
        let supplement = json!({"payload": [{}, null, "x"]});
        let exact_nodes =
            count_json_nodes(&serde_json::to_vec(&supplement).unwrap(), NODE_CAP).unwrap();
        let exact_bytes = usize::try_from(serialized_json_bytes(&supplement).unwrap()).unwrap();
        let mut value = fixture(256, 64);
        value["ext"]["prebid"]["bidderparams"] = json!({"fake_a0": supplement});
        thread::spawn(move || {
            let source: BidRequest = serde_json::from_value(value).unwrap();
            let original = serde_json::to_vec(&source).unwrap();
            let (registry, _) = registry();
            let disabled = BTreeSet::new();
            let aliases = resolve_aliases(&source, &registry, &disabled, &BTreeMap::new()).unwrap();
            let grouped = group_by_bidder(
                &source,
                &aliases.map,
                &registry,
                &disabled,
                &aliases.dropped_alias_names,
            )
            .unwrap();
            let selected = [BidderName::new("fake_a0")];
            let charge = grouped.projection_charge(&selected).unwrap();
            let budget = AuctionBudget::new(charge);
            let receipt = budget.try_reserve(ChargeKind::Projection, charge).unwrap();
            let refusal_before = alloc_track::reset_peak();
            assert_eq!(
                grouped.ensure_projection_capacity(&selected, charge.checked_sub(1).unwrap()),
                Err(ResourceError::Exhausted {
                    kind: ChargeKind::Projection
                })
            );
            assert_eq!(alloc_track::peak_since(refusal_before), 0);
            for (nodes, bytes) in [
                (exact_nodes - 1, exact_bytes),
                (exact_nodes, exact_bytes - 1),
            ] {
                let supplement_before = alloc_track::reset_peak();
                let error = grouped
                    .ensure_supplement_capacity(&selected, nodes, bytes)
                    .unwrap_err();
                assert_eq!(error.code(), 2);
                drop(error);
                assert_eq!(alloc_track::live(), supplement_before);
            }
            grouped
                .ensure_supplement_capacity(&selected, exact_nodes, exact_bytes)
                .unwrap();
            grouped
                .ensure_projection_capacity(&selected, charge)
                .unwrap();
            let projection_before = alloc_track::reset_peak();
            let scoped = materialize_per_bidder(
                &grouped,
                &selected,
                &aliases.map,
                &registry,
                &aliases.dropped_alias_names,
            )
            .unwrap();
            receipt
                .ensure_capacity(
                    ChargeKind::Projection,
                    alloc_track::peak_since(projection_before),
                )
                .unwrap();
            assert_eq!(
                scoped[0].request.imp[0]
                    .ext
                    .as_ref()
                    .unwrap()
                    .bidder
                    .as_ref()
                    .unwrap()["payload"],
                json!([{}, null, "x"])
            );
            assert_eq!(serde_json::to_vec(&source).unwrap(), original);
            drop(scoped);
            drop(grouped);
            drop(source);
            drop(receipt);
            assert_eq!(budget.in_use(), 0);
        })
        .join()
        .unwrap();
    }

    #[test]
    fn invalid_later_scope_releases_retained_scopes_without_mutating_input() {
        for patch in [
            json!({"banner": null}),
            json!({"banner": {"w": "malformed"}}),
        ] {
            let mut value = fixture(2048, 513);
            value["imp"][0]["ext"]["prebid"] = json!({"imp": {"fake_a1": patch}});
            thread::spawn(move || {
                let source: BidRequest = serde_json::from_value(value).unwrap();
                source.validate().unwrap();
                let original = serde_json::to_vec(&source).unwrap();
                let (registry, _) = registry();
                let disabled = BTreeSet::new();
                let aliases =
                    resolve_aliases(&source, &registry, &disabled, &BTreeMap::new()).unwrap();
                let grouped = group_by_bidder(
                    &source,
                    &aliases.map,
                    &registry,
                    &disabled,
                    &aliases.dropped_alias_names,
                )
                .unwrap();
                let selected = [BidderName::new("fake_a0"), BidderName::new("fake_a1")];
                let funded = derive(&row(2));
                let budget = AuctionBudget::new(funded.per_auction_peak);
                let receipts = funded.auction_charges.prepay(&budget).unwrap();
                let projection = receipts
                    .iter()
                    .find(|r| r.kind() == ChargeKind::Projection)
                    .unwrap();
                let charge = grouped.projection_charge(&selected).unwrap();
                projection
                    .ensure_capacity(ChargeKind::Projection, charge)
                    .unwrap();
                let before = alloc_track::reset_peak();
                let error = materialize_per_bidder(
                    &grouped,
                    &selected,
                    &aliases.map,
                    &registry,
                    &aliases.dropped_alias_names,
                )
                .unwrap_err();
                let peak = alloc_track::peak_since(before);
                assert_eq!(error.code(), 15);
                projection
                    .ensure_capacity(ChargeKind::Projection, peak)
                    .unwrap();
                assert!(peak <= charge);
                drop(error);
                assert_eq!(
                    alloc_track::live(),
                    before,
                    "the partially retained vector must unwind"
                );
                assert_eq!(serde_json::to_vec(&source).unwrap(), original);
                // The malformed candidate was not consumed from the shared tree;
                // choosing only the valid survivor still succeeds afterward.
                let valid = materialize_per_bidder(
                    &grouped,
                    &selected[..1],
                    &aliases.map,
                    &registry,
                    &aliases.dropped_alias_names,
                )
                .unwrap();
                assert_eq!(valid.len(), 1);
                assert!(valid[0].request.imp[0].banner.is_some());
                drop(valid);
                drop(grouped);
                drop(source);
                drop(receipts);
                assert_eq!(budget.in_use(), 0);
            })
            .join()
            .unwrap();
        }
    }
}

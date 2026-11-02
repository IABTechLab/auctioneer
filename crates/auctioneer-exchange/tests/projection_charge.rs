//! Peak allocation of survivor materialization, including its transient Value
//! and typed-deserialization overlap. The shared grouped tree is charged
//! separately and exists before each measurement window.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track;
    use crate::fake_bidder::{self, FakeBidder};
    use auctioneer_bidder::BidderRegistry;
    use auctioneer_exchange::{
        resolve_aliases,
        split::{group_by_bidder, materialize_per_bidder},
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_runtime::bounded_json::count_json_nodes;
    use auctioneer_runtime::budget::TYPED_CLONE_FACTOR;
    use serde_json::{Map, Value, json};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::{self, Write as _};
    use std::thread;

    fn base(impressions: usize) -> Value {
        let aliases: BTreeMap<_, _> = (0_usize..8)
            .map(|n| (format!("fake_a{n}"), "fake_bb"))
            .collect();
        let bidders: Map<_, _> = aliases
            .keys()
            .map(|name| (name.clone(), json!({})))
            .collect();
        let imps: Vec<_> = (0..impressions)
            .map(|n| {
                json!({
                    "id":n.to_string(), "banner":{}, "ext":{"bidder":bidders}
                })
            })
            .collect();
        json!({"id":"r", "site":{"id":"s"}, "imp":imps, "ext":{"prebid":{"aliases":aliases}}})
    }

    fn fixtures() -> Vec<(&'static str, Value)> {
        let mut unknown = base(1);
        let fields: Map<_, _> = (0_usize..10_000)
            .map(|n| (n.to_string(), json!(0_i32)))
            .collect();
        unknown["unknown"] = json!(fields);

        let mut nested = base(1);
        nested["unknown"] = json!(vec![json!([[[], []], [[], []]]); 5000]);

        let mut strings = base(100);
        for imp in strings["imp"].as_array_mut().unwrap() {
            imp["tagid"] = json!("t");
            imp["banner"]["format"] = json!(vec![json!({"w":1_i32,"h":1_i32}); 10]);
            imp["ext"]["context"] = json!({"a":"", "b":"", "c":""});
        }

        let mut fpd = base(1);
        fpd["imp"][0]["ext"]["context"] = json!(vec![json!({"":[]}); 2000]);
        let patches: Map<_, _> = (0_usize..8)
            .map(|n| (format!("fake_a{n}"), json!({"tagid":"t"})))
            .collect();
        fpd["imp"][0]["ext"]["prebid"] = json!({"imp":patches});

        let mut full_fpd = fpd.clone();
        full_fpd["imp"][0]["ext"]["context"] = json!(vec![json!({"":[]}); 20_000]);
        full_fpd["padding"] = json!("");
        let used = serde_json::to_vec(&full_fpd).unwrap().len();
        full_fpd["padding"] = json!("p".repeat((256 * 1024_usize).checked_sub(used).unwrap()));
        assert_eq!(serde_json::to_vec(&full_fpd).unwrap().len(), 256 * 1024);

        let mut patch_array = base(1);
        patch_array["imp"][0]["ext"]["prebid"] = json!({"imp":{"fake_a0":{"ext":{
            "context":vec![json!({"":[]});2000]
        }}}});
        let mut patch_map = base(1);
        patch_map["imp"][0]["ext"]["prebid"] = json!({"imp":{"fake_a0":{"ext":{
            "context":{"nested":vec![json!({"":[]});2000]}
        }}}});

        let mut supplemented = base(20);
        supplemented["ext"]["prebid"]["bidderparams"] =
            json!({"fake_a0":{"p":vec![json!({"":[]});200]}});

        let dense_params = json!({"p":vec![json!({"":[]});20_000]});
        let mut direct_params = base(1);
        direct_params["imp"][0]["ext"]["bidder"]["fake_a0"] = dense_params.clone();
        let mut prebid_params = base(1);
        prebid_params["imp"][0]["ext"]["bidder"]
            .as_object_mut()
            .unwrap()
            .remove("fake_a0");
        prebid_params["imp"][0]["ext"]["prebid"] = json!({"bidder":{"fake_a0":dense_params}});
        let mut legacy_params = base(1);
        legacy_params["imp"][0]["ext"]["bidder"]
            .as_object_mut()
            .unwrap()
            .remove("fake_a0");
        legacy_params["imp"][0]["ext"]["fake_a0"] = dense_params;

        let mut aliases = base(1);
        let names: Map<_, _> = (0_usize..10_000)
            .map(|n| (format!("fake_a{n}"), json!("fake_bb")))
            .collect();
        aliases["ext"]["prebid"]["aliases"] = json!(names);

        vec![
            ("minimal", base(1)),
            ("100-impressions", base(100)),
            ("1000-impressions", base(1000)),
            ("dense-unknown-map", unknown),
            ("nested-forest", nested),
            ("short-typed-strings", strings),
            ("fpd-projection", fpd),
            ("full-ingress-fpd-projection", full_fpd),
            ("heavy-fpd-array-patch", patch_array),
            ("heavy-fpd-map-patch", patch_map),
            ("bidder-params-amplification", supplemented),
            ("dense-direct-params", direct_params),
            ("dense-prebid-params", prebid_params),
            ("dense-legacy-params", legacy_params),
            ("dense-alias-retention", aliases),
        ]
    }

    #[test]
    fn materialization_peak_with_margin_fits_the_per_survivor_charge() {
        let mut violations = Vec::new();
        for (name, value) in fixtures() {
            let raw = serde_json::to_vec(&value).unwrap();
            assert!(
                raw.len() <= 256 * 1024,
                "{name} must be within the ingress cap"
            );
            let bytes = u64::try_from(raw.len()).unwrap();
            let costs = thread::spawn(move || {
                count_json_nodes(&raw, 0x0001_0000).unwrap();
                let req: BidRequest = serde_json::from_slice(&raw).unwrap();
                req.validate().unwrap();
                let mut registry = BidderRegistry::new();
                let info = fake_bidder::info();
                registry
                    .register(info, Box::new(FakeBidder::default()))
                    .expect("unique test bidder");
                let disabled = BTreeSet::new();
                let aliases =
                    resolve_aliases(&req, &registry, &disabled, &BTreeMap::new()).unwrap();
                let grouped = group_by_bidder(
                    &req,
                    &aliases.map,
                    &registry,
                    &disabled,
                    &aliases.dropped_alias_names,
                )
                .unwrap();
                [1_usize, 8].map(|count| {
                    let survivors: Vec<_> =
                        grouped.bidder_imps().keys().take(count).cloned().collect();
                    assert_eq!(survivors.len(), count);
                    let funded = grouped.projection_charge(&survivors).unwrap();
                    let before = alloc_track::reset_peak();
                    let projected = materialize_per_bidder(
                        &grouped,
                        &survivors,
                        &aliases.map,
                        &registry,
                        &aliases.dropped_alias_names,
                    )
                    .unwrap();
                    let peak = alloc_track::peak_since(before);
                    let retained =
                        u64::try_from(alloc_track::live().checked_sub(before).unwrap()).unwrap();
                    assert_eq!(projected.len(), count);
                    drop(projected);
                    (count, peak, retained, funded)
                })
            })
            .join()
            .unwrap();
            for (count, peak, retained, funded) in costs {
                let with_margin = peak.checked_mul(6).unwrap().div_ceil(5);
                writeln!(io::stderr(), "{name}: raw={bytes} survivors={count} peak={peak} retained={retained} margin={with_margin} funded={funded}").unwrap();
                if with_margin > funded {
                    violations.push(format!("{name}/{count}: {with_margin} > {funded}"));
                }
            }
        }
        assert!(
            violations.is_empty(),
            "underfunded projections: {violations:?}"
        );
    }

    #[test]
    fn amplified_supplement_is_refused_before_projection_allocations() {
        let mut value = base(100);
        value["ext"]["prebid"]["bidderparams"] =
            json!({"fake_a0":{"p":vec![json!({"":[]});20_000]}});
        let raw = serde_json::to_vec(&value).unwrap();
        assert!(raw.len() <= 256 * 1024);
        count_json_nodes(&raw, 0x0001_0000).unwrap();
        let req: BidRequest = serde_json::from_slice(&raw).unwrap();
        let mut registry = BidderRegistry::new();
        let info = fake_bidder::info();
        registry
            .register(info, Box::new(FakeBidder::default()))
            .expect("unique test bidder");
        let disabled = BTreeSet::new();
        let aliases = resolve_aliases(&req, &registry, &disabled, &BTreeMap::new()).unwrap();
        let grouped = group_by_bidder(
            &req,
            &aliases.map,
            &registry,
            &disabled,
            &aliases.dropped_alias_names,
        )
        .unwrap();
        let selected = vec![auctioneer_bidder::BidderName::new("fake_a0")];
        let capacity = 256 * 1024 * TYPED_CLONE_FACTOR;
        let before = alloc_track::reset_peak();
        assert_eq!(
            grouped.ensure_projection_capacity(&selected, capacity),
            Err(auctioneer_runtime::ResourceError::Exhausted {
                kind: auctioneer_runtime::ChargeKind::Projection
            })
        );
        assert_eq!(
            alloc_track::peak_since(before),
            0,
            "refusal must precede every projection allocation"
        );
    }
}

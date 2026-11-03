//! Native retained IPv6 growth evidence, independent of projection admission.
//! Source, scope vector, and fixed audit ownership precede the mutation snapshot.
//! This does not qualify a hosting target or its allocator.

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[cfg(test)]
#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track;
    use auctioneer_config_data::disclosure::SchemaId;
    use auctioneer_config_data::types::IpKeepBits;
    use auctioneer_exchange::scrub::{
        DisclosureAudit, ScrubDecisions, apply_classified, construction_costs,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::device::Device;
    use serde_json::Value;
    use std::hint::black_box;

    const INPUT: &str = "2001:4860::3:4:5:6:7";
    const NETWORK: &str = "2001:4860:0:3:4:5:6:0";
    // A whole-request clone must exceed even the 50-scope growth/scratch bound.
    const CONTEXT_BYTES: usize = 0x0001_0000;
    const KEEP: IpKeepBits = IpKeepBits { v4: 24, v6: 112 };
    const DECISIONS: ScrubDecisions = ScrubDecisions {
        pass_id: true,
        pass_geo: false,
        transmit_ufpd: true,
        transmit_precise_geo: true,
        transmit_tid: true,
        ccpa: false,
        lmt: false,
        coppa: false,
    };

    #[derive(Clone, Copy)]
    enum Slot {
        Typed,
        Shadow,
    }

    fn fixture(slot: Slot) -> BidRequest {
        let mut device = Device {
            ua: Some("x".repeat(CONTEXT_BYTES)),
            ..Device::default()
        };
        let tight = Box::<str>::from(INPUT).into_string();
        match slot {
            Slot::Typed => device.ipv6 = Some(tight),
            Slot::Shadow => {
                let previous = device.other.insert("ipv6".into(), Value::String(tight));
                assert!(previous.is_none());
            }
        }
        BidRequest {
            id: "fixed-source".into(),
            device: Some(device),
            ..BidRequest::default()
        }
    }

    fn address(request: &BidRequest, slot: Slot) -> &String {
        let device = request.device.as_ref().unwrap();
        match slot {
            Slot::Typed => {
                assert!(device.other.is_empty());
                device.ipv6.as_ref().unwrap()
            }
            Slot::Shadow => {
                assert!(device.ipv6.is_none());
                assert_eq!(device.other.len(), 1);
                let Some(Value::String(value)) = device.other.get("ipv6") else {
                    panic!("shadow IPv6 must remain a string");
                };
                value
            }
        }
    }

    fn assert_growth_audit(audit: &DisclosureAudit, count: u64) {
        assert_eq!(audit.attempted_scopes, count);
        assert_eq!(audit.failed_scopes, 0);
        assert!(!audit.saturated);
        assert_eq!(audit.counts(), [0, 0, count, 0, 0, 0, 0, 0]);
        assert_eq!(audit.schema_counts(SchemaId::Device), audit.counts());
    }

    fn measure_retained_growth(source: &BidRequest, slot: Slot, scope_count: usize) {
        let costs = construction_costs();
        assert_eq!(costs.retained_per_scope_bytes, 78);
        let count = u64::try_from(scope_count).unwrap();
        let growth_budget = costs.retained_per_scope_bytes.checked_mul(count).unwrap();
        let peak_budget = growth_budget
            .checked_add(costs.reusable_scratch_bytes)
            .unwrap();
        assert!(u64::try_from(CONTEXT_BYTES).unwrap() > peak_budget);
        let expected_growth = u64::try_from(NETWORK.len().checked_sub(INPUT.len()).unwrap())
            .unwrap()
            .checked_mul(count)
            .unwrap();
        let mut audit = DisclosureAudit::default();
        let before_scopes = alloc_track::live();
        let mut scopes = Vec::with_capacity(scope_count);
        for _index in 0..scope_count {
            let scope = source.clone();
            let input = address(&scope, slot);
            assert_eq!(input.as_str(), INPUT);
            assert_eq!(input.capacity(), INPUT.len(), "fixture must force growth");
            scopes.push(scope);
        }

        // All scope/vector ownership and privacy counters are already funded.
        let baseline = alloc_track::reset_peak();
        for scope in &mut scopes {
            apply_classified(scope, DECISIONS, KEEP, &mut audit).unwrap();
        }
        let peak = alloc_track::peak_since(baseline);
        let retained = alloc_track::live();
        let growth = u64::try_from(retained.checked_sub(baseline).unwrap()).unwrap();

        assert_eq!(scopes.len(), scope_count);
        assert!(growth > 0, "every tight destination must actually grow");
        assert_eq!(growth, expected_growth);
        assert!(growth <= growth_budget);
        assert!(peak >= growth);
        assert!(peak <= peak_budget, "peak {peak} exceeds {peak_budget}");
        assert_growth_audit(&audit, count);
        for scope in &scopes {
            assert_eq!(address(scope, slot).as_str(), NETWORK);
            assert_eq!(
                scope.device.as_ref().unwrap().ua.as_deref(),
                source.device.as_ref().unwrap().ua.as_deref()
            );
        }
        assert_eq!(address(source, slot).as_str(), INPUT);

        // Separate positive control: the guard rejects a transient whole copy.
        let clone_baseline = alloc_track::reset_peak();
        let extra = black_box(scopes.first().unwrap().clone());
        let clone_peak = alloc_track::peak_since(clone_baseline);
        drop(extra);
        assert!(clone_peak > peak_budget);
        assert_eq!(alloc_track::live(), retained);

        drop(scopes);
        assert_eq!(alloc_track::live(), before_scopes);
        assert_eq!(address(source, slot).as_str(), INPUT);
    }

    #[test]
    fn typed_ipv6_growth_is_funded_while_1_2_50_scopes_remain_retained() {
        let source = fixture(Slot::Typed);
        for count in [1, 2, 50] {
            measure_retained_growth(&source, Slot::Typed, count);
        }
    }

    #[test]
    fn shadow_ipv6_growth_is_funded_while_1_2_50_scopes_remain_retained() {
        let source = fixture(Slot::Shadow);
        for count in [1, 2, 50] {
            measure_retained_growth(&source, Slot::Shadow, count);
        }
    }
}

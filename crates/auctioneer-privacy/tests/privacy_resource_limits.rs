//! Allocation evidence for fixed privacy scratch and cold GVL parsing.

#[cfg(test)]
use auctioneer_privacy::test_support::tc_encode;
#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::{alloc_track, tc_encode};
    use auctioneer_config_data::types::{HostConfig, PurposeEnforcement, TriState};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_privacy::gpp::Envelope;
    use auctioneer_privacy::gvl::Gvl;
    use auctioneer_privacy::tcf::{TcfParser, TcfString, parse_tcf_v2};
    use auctioneer_privacy::{
        AuctionPermissions, PrivacyContext, ResolveOptions,
        ccpa::NoSale,
        gcl::{CmpRecord, Gcl},
        gvl::GvlStore,
    };
    use auctioneer_runtime::budget::{PRIVACY_FIXED_BYTES, PRIVACY_REQUEST_FACTOR};
    use serde_json::{Map, Value, json};
    use std::io::{self, Write as _};
    use std::mem::size_of;
    use std::thread;
    use time::{OffsetDateTime, macros::datetime};

    fn peak<F, T>(build: F) -> u64
    where
        F: FnOnce() -> T + Send + 'static,
    {
        thread::spawn(move || {
            let before = alloc_track::reset_peak();
            let held = build();
            let cost = alloc_track::peak_since(before);
            drop(held);
            cost
        })
        .join()
        .unwrap()
    }

    #[test]
    fn cold_gvl_parse_peak_is_charged_separately_from_resident_storage() {
        let mut maximum = 0;
        for entry in auctioneer_config_data::gvl_archive() {
            let raw = entry.raw;
            let spec = entry.spec;
            let cost = peak(move || Gvl::load_from_json_bytes(raw, Some(spec)).unwrap());
            writeln!(
                io::stderr(),
                "GVL {spec}/{}: cold_peak={cost}",
                entry.vendor_list_version
            )
            .unwrap();
            maximum = maximum.max(cost);
        }
        let combined = maximum.checked_add(hostile_tcf_peak()).unwrap();
        let margined = combined.checked_mul(6).unwrap().div_ceil(5);
        assert!(
            margined <= PRIVACY_FIXED_BYTES,
            "cold GVL plus retained TCF requires {margined} bytes including margin"
        );
    }

    #[test]
    fn tcf_peak_includes_dv_parser_result_and_decoded_scratch() {
        let charged = hostile_tcf_peak().checked_mul(6).unwrap().div_ceil(5);
        assert!(charged <= PRIVACY_FIXED_BYTES);
    }

    fn hostile_tcf_peak() -> u64 {
        let mut spec = tc_encode::valid_core_spec();
        spec.vendor_consent = tc_encode::VendorSectionSpec::Range {
            max_vendor_id: u16::MAX,
            entries: (1..=4095)
                .map(|id| {
                    if id == 1 {
                        tc_encode::RangeEntrySpec::Span(1, 1)
                    } else {
                        tc_encode::RangeEntrySpec::Single(id)
                    }
                })
                .collect(),
        };
        spec.vendor_li = tc_encode::VendorSectionSpec::Bitfield {
            max_vendor_id: u16::MAX,
            set: vec![1, u16::MAX],
        };
        spec.pub_restrictions = vec![tc_encode::PubRestrictionSpec {
            purpose_id: 1,
            restriction_type: 0,
            entries: (1..=2048).map(tc_encode::RangeEntrySpec::Single).collect(),
        }];
        let encoded = format!(
            "{}.{}",
            tc_encode::encode_core(&spec),
            tc_encode::dv_bitfield_segment(u16::MAX, &[1, u16::MAX])
        );
        let measured = peak(move || {
            let parsed = parse_tcf_v2(&encoded).unwrap();
            assert!(parsed.disclosed_vendors.contains(u16::MAX));
            assert_eq!(parsed.publisher_restrictions.len(), 2048);
            Box::new(parsed)
        });
        // Inline parser/result state is invisible to GlobalAlloc. Include
        // both plus the bounded GPP slot array rather than omitting it.
        let inline = size_of::<TcfParser>()
            .checked_add(size_of::<TcfString>())
            .unwrap()
            .checked_add(size_of::<Envelope<'_>>())
            .unwrap();
        let charged = measured
            .checked_add(u64::try_from(inline).unwrap())
            .unwrap();
        writeln!(
            io::stderr(),
            "TCF: heap_peak={measured} inline={inline} total={charged}"
        )
        .unwrap();
        charged
    }

    #[test]
    fn smallest_tcf_also_fits_the_fixed_charge() {
        let encoded =
            tc_encode::with_minimal_dv(&tc_encode::encode_core(&tc_encode::valid_core_spec()));
        let measured = peak(move || Box::new(parse_tcf_v2(&encoded).unwrap()));
        assert!(measured <= PRIVACY_FIXED_BYTES);
    }

    fn granting_baked_consent(
        now: OffsetDateTime,
    ) -> (&'static str, u16, tc_encode::CoreSpec, String) {
        let entry = auctioneer_config_data::gvl_archive()
            .iter()
            .filter(|entry| entry.spec == 3)
            .max_by_key(|entry| entry.vendor_list_version)
            .unwrap();
        // Cold decoding has its own fixed-charge guard above. Warm exactly
        // the revision used here so name growth cannot be hidden by a cold
        // parse or funded twice through the request-dependent term.
        let warmed = GvlStore::baked()
            .get(entry.spec, entry.vendor_list_version)
            .unwrap();
        let (core_name, vendor_id) = auctioneer_config_data::bidders()
            .iter()
            .find_map(|info| {
                let id = u16::try_from(info.gvl_vendor_id?).ok()?;
                let vendor = warmed.vendor(id)?;
                (id != 0
                    && !vendor.is_deleted_at(now)
                    && (vendor.purposes.contains(&2) || vendor.leg_int_purposes.contains(&2)))
                .then_some((info.name.as_str(), id))
            })
            .expect("baked metadata must retain an active purpose-2 vendor in the newest GVL");
        let vendor = warmed.vendor(vendor_id).unwrap();
        assert!(!vendor.is_deleted_at(now));
        assert!(vendor.purposes.contains(&2) || vendor.leg_int_purposes.contains(&2));
        let mut spec = tc_encode::valid_core_spec();
        spec.vendor_list_version = u16::try_from(entry.vendor_list_version).unwrap();
        assert!(
            spec.vendor_list_version <= 0x0fff,
            "TCF list version must fit its 12-bit wire field"
        );
        spec.purposes_consent = vec![2];
        spec.purposes_li_transparency = vec![2];
        spec.vendor_consent = tc_encode::VendorSectionSpec::Range {
            max_vendor_id: vendor_id,
            entries: vec![tc_encode::RangeEntrySpec::Single(vendor_id)],
        };
        spec.vendor_li = tc_encode::VendorSectionSpec::Range {
            max_vendor_id: vendor_id,
            entries: vec![tc_encode::RangeEntrySpec::Single(vendor_id)],
        };
        let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
        let parsed = parse_tcf_v2(&consent).unwrap();
        assert_eq!(parsed.vendor_list_version, spec.vendor_list_version);
        assert!(parsed.vendor_consent.contains(vendor_id));
        (core_name, vendor_id, spec, consent)
    }

    fn enforcing_host() -> HostConfig {
        let mut host = auctioneer_config_data::host_default().clone();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        for purpose in 1..=10 {
            host.gdpr.tcf2.purposes.insert(
                purpose,
                PurposeEnforcement {
                    enforce_purpose: TriState::Enabled,
                    enforce_vendors: TriState::Enabled,
                    ..PurposeEnforcement::default()
                },
            );
        }
        host
    }

    fn assert_baked_permission_presence(context: &PrivacyContext, core_name: &str) {
        assert_eq!(
            context.permissions(core_name, core_name),
            AuctionPermissions::AllowAll
        );
        assert_eq!(
            context.permissions(core_name, "fake_budget_probe"),
            AuctionPermissions::AllowAll
        );
        assert!(
            matches!(
                context.permissions("fake_privacy_budget_unknown_core", "fake_budget_probe"),
                AuctionPermissions::Partial {
                    allow_bid_request: false,
                    ..
                }
            ),
            "a missing baked vendor must be denied under active enforcement"
        );
    }

    #[test]
    fn baked_gvl_request_names_and_selected_signals_fit_the_request_dependent_charge() {
        // Allocation conformance for metadata-selected input and its baked GVL,
        // including the core-id fallback for the unlisted budget probe.
        let now = datetime!(2026-10-12 00:00 UTC);
        let gvl = GvlStore::baked();
        let (core_name, vendor_id, spec, consent) = granting_baked_consent(now);
        assert!(auctioneer_config_data::bidder("fake_privacy_budget_unknown_core").is_none());
        let mut baseline = None;
        for count in [0_usize, 1, 100, 5000] {
            let names: Vec<String> = (0..count).map(|n| format!("fake_{n}")).collect();
            let ids: Map<String, Value> = names
                .iter()
                .map(|name| (name.clone(), json!(vendor_id)))
                .collect();
            let raw = serde_json::to_vec(&json!({
                "id":"privacy-budget", "imp":[],
                "regs":{"gpp":format!("DBABM~{consent}"),"gpp_sid":[2_u16],"us_privacy":"1YYN"},
                "user":{"consent":consent},
                "ext":{"prebid":{"aliasgvlids":ids,"nosale":names}}
            }))
            .unwrap();
            assert!(raw.len() <= 0x0004_0000);
            let request: BidRequest =
                auctioneer_runtime::from_slice_within(&raw, 0x0001_0000).unwrap();
            let bytes = u64::try_from(raw.len()).unwrap();
            let host = enforcing_host();
            let account = auctioneer_config_data::account_default();
            let gcl = Gcl::for_test(
                vec![CmpRecord {
                    id: spec.cmp_id,
                    deleted_date: None,
                }],
                datetime!(2027-01-01 00:00 UTC),
            );
            let cost = peak(move || {
                let no_sale = NoSale::validate(&names, |name| name.starts_with("fake_")).unwrap();
                let resolved = PrivacyContext::resolve(
                    &request,
                    account,
                    &host,
                    gvl,
                    &gcl,
                    now,
                    ResolveOptions {
                        gdpr_exempt: false,
                        no_sale,
                    },
                )
                .unwrap();
                assert_baked_permission_presence(&resolved.context, core_name);
                for name in &names {
                    assert_eq!(
                        resolved.context.permissions(core_name, name),
                        AuctionPermissions::AllowAll,
                        "fixture must reach usable consent and a GVL declaration, not the empty-consent default"
                    );
                    let _decision = resolved.context.bidder_decision(core_name, name);
                }
                resolved.context
            });
            let margined = cost.checked_mul(6).unwrap().div_ceil(5);
            let request_charge = bytes.checked_mul(PRIVACY_REQUEST_FACTOR).unwrap();
            let charge = PRIVACY_FIXED_BYTES.checked_add(request_charge).unwrap();
            writeln!(
                io::stderr(),
                "privacy names={count} raw={bytes} peak={cost} margin={margined} charge={charge}"
            )
            .unwrap();
            assert!(margined <= charge);
            if let Some((base_bytes, base_cost)) = baseline {
                let growth = cost
                    .checked_sub(base_cost)
                    .expect("names must grow the measured peak");
                assert!(growth > 0);
                let margined_growth = growth.checked_mul(6).unwrap().div_ceil(5);
                let charged_growth = bytes
                    .checked_sub(base_bytes)
                    .unwrap()
                    .checked_mul(PRIVACY_REQUEST_FACTOR)
                    .unwrap();
                writeln!(io::stderr(), "privacy names={count} growth={growth} margin={margined_growth} request_growth_charge={charged_growth}").unwrap();
                assert!(margined_growth <= charged_growth);
            } else {
                assert_eq!(count, 0);
                assert!(margined <= PRIVACY_FIXED_BYTES);
                baseline = Some((bytes, cost));
            }
        }
    }
}

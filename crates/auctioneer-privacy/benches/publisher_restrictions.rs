//! Per-identity GDPR decision cost as the TC string's publisher-restriction
//! list grows to the decoder's limit. An auction makes this decision once per
//! bidder identity, before the bidder cap applies. Run with
//! `cargo bench -p auctioneer-privacy --bench publisher_restrictions`.
#![expect(
    clippy::print_stdout,
    reason = "benchmark harness reports its measurements on stdout"
)]

use auctioneer_config_data::types::{HostConfig, PurposeEnforcement, TriState};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_privacy::ccpa::NoSale;
use auctioneer_privacy::gcl::{CmpRecord, Gcl};
use auctioneer_privacy::gvl::GvlStore;
use auctioneer_privacy::test_support::tc_encode;
use auctioneer_privacy::{AuctionPermissions, PrivacyContext, ResolveOptions};
use serde_json::json;
use std::hint::black_box;
use std::time::Instant;
use time::OffsetDateTime;
use time::macros::datetime;

const RESTRICTION_COUNTS: [usize; 3] = [0, 64, 2_048];
const CALLS_PER_SAMPLE: u32 = 200;
const SAMPLES: usize = 21;

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

/// Resolves one enforcing context with `restriction_count` non-adjacent
/// single-vendor restrictions on purpose 1. The auction decision never queries
/// purpose 1, so no lookup matches and a linear lookup reads every record.
fn context(
    vendor_list_version: u32,
    restriction_count: usize,
    grant: Option<u16>,
    now: OffsetDateTime,
) -> Result<PrivacyContext, &'static str> {
    let mut spec = tc_encode::valid_core_spec();
    spec.vendor_list_version = u16::try_from(vendor_list_version)
        .map_err(|_overflow| "TCF list version exceeds 12 bits")?;
    if let Some(vendor_id) = grant {
        let vendor_only = || tc_encode::VendorSectionSpec::Range {
            max_vendor_id: vendor_id,
            entries: vec![tc_encode::RangeEntrySpec::Single(vendor_id)],
        };
        spec.purposes_consent = vec![2];
        spec.purposes_li_transparency = vec![2];
        spec.vendor_consent = vendor_only();
        spec.vendor_li = vendor_only();
    }
    if restriction_count != 0 {
        spec.pub_restrictions = vec![tc_encode::PubRestrictionSpec {
            purpose_id: 1,
            restriction_type: 0,
            entries: (1_u16..)
                .step_by(2)
                .take(restriction_count)
                .map(tc_encode::RangeEntrySpec::Single)
                .collect(),
        }];
    }
    let cmp_id = spec.cmp_id;
    let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
    let request: BidRequest = serde_json::from_value(json!({
        "id": "publisher-restriction-bench",
        "imp": [],
        "regs": {"gdpr": 1_u8},
        "user": {"consent": consent},
    }))
    .map_err(|_error| "bench request does not deserialize")?;
    let gcl = Gcl::for_test(
        vec![CmpRecord {
            id: cmp_id,
            deleted_date: None,
        }],
        datetime!(2027-01-01 00:00 UTC),
    );
    let no_sale = NoSale::validate(&[], |_name| false).map_err(|_error| "empty no-sale list")?;
    let resolved = PrivacyContext::resolve(
        &request,
        auctioneer_config_data::account_default(),
        &enforcing_host(),
        GvlStore::baked(),
        &gcl,
        now,
        ResolveOptions {
            gdpr_exempt: false,
            no_sale,
        },
    )
    .map_err(|_error| "enforcing context does not resolve")?;
    Ok(resolved.context)
}

fn measure(label: &str, restriction_count: usize, ctx: &PrivacyContext, core_name: &str) {
    let mut samples_ns: Vec<u128> = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        for _ in 0..CALLS_PER_SAMPLE {
            black_box(ctx.bidder_decision(black_box(core_name), black_box(core_name)));
        }
        let per_call = started
            .elapsed()
            .as_nanos()
            .checked_div(u128::from(CALLS_PER_SAMPLE))
            .unwrap_or(0);
        samples_ns.push(per_call);
    }
    samples_ns.sort_unstable();
    let p50 = samples_ns
        .get(samples_ns.len().wrapping_div(2))
        .copied()
        .unwrap_or(0);
    println!(
        "publisher_restrictions {label}: entries={restriction_count} per_call_ns min={} p50={p50} max={}",
        samples_ns.first().copied().unwrap_or(0),
        samples_ns.last().copied().unwrap_or(0),
    );
}

fn run() -> Result<(), &'static str> {
    let now = datetime!(2026-10-12 00:00 UTC);
    let entry = auctioneer_config_data::gvl_archive()
        .iter()
        .filter(|entry| entry.spec == 3)
        .max_by_key(|entry| entry.vendor_list_version)
        .ok_or("no baked spec-3 vendor list")?;
    let list = GvlStore::baked()
        .get(entry.spec, entry.vendor_list_version)
        .ok_or("baked vendor list does not parse")?;
    let (core_name, vendor_id) = auctioneer_config_data::bidders()
        .iter()
        .find_map(|info| {
            let id = u16::try_from(info.gvl_vendor_id?).ok()?;
            let vendor = list.vendor(id)?;
            let declares_purpose_2 =
                vendor.purposes.contains(&2) || vendor.leg_int_purposes.contains(&2);
            (id != 0 && !vendor.is_deleted_at(now) && declares_purpose_2)
                .then_some((info.name.as_str(), id))
        })
        .ok_or("no active purpose-2 vendor in the newest baked list")?;

    for restriction_count in RESTRICTION_COUNTS {
        let granting = context(
            entry.vendor_list_version,
            restriction_count,
            Some(vendor_id),
            now,
        )?;
        let granted = match granting.permissions(core_name, core_name) {
            AuctionPermissions::AllowAll => true,
            AuctionPermissions::Partial {
                allow_bid_request, ..
            } => allow_bid_request,
            AuctionPermissions::DenyBidRequest => false,
        };
        if !granted {
            return Err("the granting string must authenticate and allow the bid request");
        }
        let denying = context(entry.vendor_list_version, restriction_count, None, now)?;
        if !matches!(
            denying.permissions(core_name, core_name),
            AuctionPermissions::Partial {
                allow_bid_request: false,
                pass_id: false,
                ..
            }
        ) {
            return Err("without grants the identity must lose the bid request and IDs");
        }

        measure("grant", restriction_count, &granting, core_name);
        measure("deny", restriction_count, &denying, core_name);
    }
    Ok(())
}

fn main() {
    if let Err(reason) = run() {
        println!("publisher_restrictions: {reason}");
    }
}

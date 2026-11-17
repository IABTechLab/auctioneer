//! GDPR cost as the TC string's publisher-restriction list grows to the
//! decoder's limit. `resolve` is paid once per request and includes building
//! the restriction index; `grant` and `deny` are the decision an auction makes
//! once per bidder identity, before the bidder cap applies. Run with
//! `cargo bench -p auctioneer-privacy --bench publisher_restrictions`.
//! Figures from a debug build, such as a `cargo test --all-targets` run, are
//! not meaningful.
//! A failed setup or control check exits nonzero, so a run that measured
//! nothing never passes as a result.
#![expect(
    clippy::print_stdout,
    reason = "benchmark harness reports its measurements on stdout"
)]

use auctioneer_config_data::types::{HostConfig, PurposeEnforcement, TriState};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_privacy::gcl::{CmpRecord, Gcl};
use auctioneer_privacy::gvl::GvlStore;
use auctioneer_privacy::test_support::tc_encode;
use auctioneer_privacy::{AuctionPermissions, PrivacyContext, ResolveOptions};
use serde_json::json;
use std::hint::black_box;
use std::process::ExitCode;
use std::time::Instant;
use time::OffsetDateTime;
use time::macros::datetime;

const RESTRICTION_COUNTS: [usize; 3] = [0, 64, 2_048];
const DECISIONS_PER_SAMPLE: u32 = 200;
const RESOLUTIONS_PER_SAMPLE: u32 = 20;
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

/// One enforcing request whose purpose 1 carries `restriction_count`
/// non-adjacent single-vendor restrictions. The auction decision never queries
/// purpose 1, so no lookup matches and a linear lookup reads every record.
struct Scenario {
    request: BidRequest,
    gcl: Gcl,
}

fn scenario(
    vendor_list_version: u32,
    restriction_count: usize,
    grant: Option<u16>,
) -> Result<Scenario, &'static str> {
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
    let consent = tc_encode::with_minimal_dv(&tc_encode::encode_core(&spec));
    let request = serde_json::from_value(json!({
        "id": "publisher-restriction-bench",
        "imp": [],
        "regs": {"gdpr": 1_u8},
        "user": {"consent": consent},
    }))
    .map_err(|_error| "bench request does not deserialize")?;
    let gcl = Gcl::for_test(
        vec![CmpRecord {
            id: spec.cmp_id,
            deleted_date: None,
        }],
        datetime!(2027-01-01 00:00 UTC),
    );
    Ok(Scenario { request, gcl })
}

fn resolve(
    scenario: &Scenario,
    host: &HostConfig,
    now: OffsetDateTime,
) -> Result<PrivacyContext, &'static str> {
    PrivacyContext::resolve(
        &scenario.request,
        auctioneer_config_data::account_default(),
        host,
        GvlStore::baked(),
        &scenario.gcl,
        now,
        ResolveOptions::default(),
    )
    .map(|resolved| resolved.context)
    .map_err(|_error| "enforcing context does not resolve")
}

/// Sorted per-call nanoseconds, one entry per sample.
fn per_call_samples(calls: u32, mut operation: impl FnMut()) -> Vec<u128> {
    let mut samples_ns = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        for _ in 0..calls {
            operation();
        }
        let per_call = started
            .elapsed()
            .as_nanos()
            .checked_div(u128::from(calls))
            .unwrap_or(0);
        samples_ns.push(per_call);
    }
    samples_ns.sort_unstable();
    samples_ns
}

fn report(label: &str, restriction_count: usize, samples_ns: &[u128]) {
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

fn decision_samples(ctx: &PrivacyContext, core_name: &str) -> Vec<u128> {
    per_call_samples(DECISIONS_PER_SAMPLE, || {
        black_box(ctx.bidder_decision(black_box(core_name), black_box(core_name)));
    })
}

fn run() -> Result<(), &'static str> {
    let now = datetime!(2026-10-12 00:00 UTC);
    let host = enforcing_host();
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
        let granting_scenario = scenario(
            entry.vendor_list_version,
            restriction_count,
            Some(vendor_id),
        )?;
        let granting = resolve(&granting_scenario, &host, now)?;
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
        let denying = resolve(
            &scenario(entry.vendor_list_version, restriction_count, None)?,
            &host,
            now,
        )?;
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

        let resolutions = per_call_samples(RESOLUTIONS_PER_SAMPLE, || {
            drop(black_box(resolve(
                black_box(&granting_scenario),
                &host,
                now,
            )));
        });
        report("resolve", restriction_count, &resolutions);
        report(
            "grant",
            restriction_count,
            &decision_samples(&granting, core_name),
        );
        report(
            "deny",
            restriction_count,
            &decision_samples(&denying, core_name),
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            println!("publisher_restrictions: {reason}");
            ExitCode::FAILURE
        }
    }
}

//! Source-bound arithmetic reports. Native substitutions are not WASM ABI proof.

mod catalog_allowances;
mod catalog_comparison;
mod records;
pub(super) mod review;

use super::Error;
use super::resources_platform::{Input, Validation, evaluate};
use auctioneer_adapter_axum::{build_features, envelope as axum};
use auctioneer_adapter_cloudflare::envelope as cloudflare;
use auctioneer_adapter_fastly::envelope as fastly;
use auctioneer_adapter_spin::envelope as spin;
use auctioneer_core::host_primitives::{ResourceProfile, resolve_budget_inputs};
use auctioneer_privacy::gvl::GVL_PARSE_CACHE_MAX_ENTRIES;
use auctioneer_runtime::{
    AuctionCharges, CatalogResourceCosts, Derived, HeadroomClass, IsolateTerms,
    RequestConstructionCosts, ResolvedIsolateLimits, TargetPlatformConstants,
    derive_isolate_budget_with_baked_config,
};
use edgezero_core::{MemoryCeilingScope, PlatformFact, PlatformMetadata};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env::consts::{ARCH, OS};
use std::path::Path;
use std::slice::from_ref;

use auctioneer_cli as resource_identity;
use resource_identity::{BuildIdentity, snapshot};

const MAX_BYTES: usize = 1 << 20_usize;
const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AdapterId {
    Axum,
    Cloudflare,
    Fastly,
    Spin,
}

impl AdapterId {
    pub(crate) const ALL: [Self; 4] = [Self::Axum, Self::Cloudflare, Self::Fastly, Self::Spin];

    fn profile(self) -> ResourceProfile {
        match self {
            Self::Axum => axum::resource_profile(),
            Self::Cloudflare => cloudflare::resource_profile(),
            Self::Fastly => fastly::resource_profile(),
            Self::Spin => spin::resource_profile(),
        }
    }

    fn class(self) -> HeadroomClass {
        self.profile().headroom_class
    }

    fn scope(self) -> MemoryCeilingScope {
        self.profile().memory_scope
    }

    fn platform(self) -> PlatformMetadata {
        self.profile().platform
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Selection {
    Adapter { adapter: AdapterId },
    AllAdapters,
}

impl Selection {
    fn adapters(&self) -> &[AdapterId] {
        match self {
            Self::Adapter { adapter } => from_ref(adapter),
            Self::AllAdapters => &AdapterId::ALL,
        }
    }
}

const fn layout_evidence(profile: &ResourceProfile) -> LayoutEvidence {
    match profile.headroom_class {
        HeadroomClass::Native => LayoutEvidence::NativeControl,
        HeadroomClass::Wasm => LayoutEvidence::HostNativeSubstitute,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LayoutEvidence {
    NativeControl,
    HostNativeSubstitute,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Identity {
    pub code_base_commit: String,
    pub dirty: bool,
    pub source_tree_matches_commit: bool,
    pub sdk_revision: String,
    pub lock_sha256: String,
    pub declared_toolchain: String,
    pub features: Vec<String>,
    pub source_sha256: String,
    pub source_file_count: usize,
    pub compiler_verbose: String,
    pub compiled_target: String,
    pub reporter_abi: String,
    pub target_abi: String,
    pub layout_evidence: LayoutEvidence,
    pub artifact_proof: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Fact<T> {
    Known { value: T, source: String },
    Pending { reason: String },
}

fn fact<T: Copy, U>(input: PlatformFact<T>, map: impl FnOnce(T) -> U) -> Fact<U> {
    match input.value() {
        Some(value) => Fact::Known {
            value: map(value),
            source: format!("{:?}", input.source()),
        },
        None => Fact::Pending {
            reason: format!("{:?}", input.unknown_reason()),
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ceiling {
    pub primary_bytes: u64,
    pub scope: String,
    pub separate_stack_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlatformFacts {
    pub ceiling: Fact<Ceiling>,
    pub inbound_population: Fact<u32>,
    pub host_ingress_accounting: Fact<String>,
}

impl PlatformFacts {
    fn new(platform: PlatformMetadata) -> Self {
        Self {
            ceiling: fact(platform.memory_ceiling(), |value| Ceiling {
                primary_bytes: value.total_bytes(),
                scope: format!("{:?}", value.scope()),
                separate_stack_bytes: value.stack_bytes(),
            }),
            inbound_population: fact(platform.inbound_request_population_bound(), |value| {
                value.max_live_requests().get()
            }),
            host_ingress_accounting: fact(platform.host_ingress_memory_accounting(), |value| {
                format!("{value:?}")
            }),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    pub schema_version: u16,
    pub accounting_complete: bool,
    pub selection: Selection,
    pub adapters: Vec<AdapterRow>,
}

impl Report {
    pub(crate) fn exit_code(&self, require_fit: bool) -> i32 {
        i32::from(
            !self.accounting_complete
                || (require_fit && self.adapters.iter().any(|row| !row.required_fit)),
        )
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1
            || !self.accounting_complete
            || self
                .adapters
                .iter()
                .map(|row| row.adapter)
                .collect::<Vec<_>>()
                != self.selection.adapters()
        {
            return Err(Error::ResourceInput);
        }
        let identity = source_identity()?;
        let (baked_bytes, baked_digest) = baked_identity()?;
        for row in &self.adapters {
            let expected_row = source_row(row.adapter, &identity, &baked_digest)?;
            let rederived = derive_isolate_budget_with_baked_config(
                &row.effective_limits,
                &row.target_constants,
                row.outbound_meta_bytes,
                row.construction_costs,
                row.baked_config_bytes,
                row.catalog_costs,
            )
            .map_err(|_error| Error::ResourceInput)?;
            check_named_terms(row.derived.isolate_terms, &row.terms)?;
            if row.validation.canonical_sdk_result() != row.validation.sdk_result
                || row.baked_config_bytes != baked_bytes
                || row.baked_config_sha256 != baked_digest
                || rederived != row.derived
                || row.effective_profile_sha256 != expected_row.effective_profile_sha256
                || profile_digest(row)? != row.effective_profile_sha256
                || row
                    .derived
                    .isolate_terms
                    .checked_total()
                    .map_err(|_error| Error::ResourceInput)?
                    != row.derived.total_bytes
                || row
                    .derived
                    .auction_charges
                    .total()
                    .map_err(|_error| Error::ResourceInput)?
                    != row.derived.per_auction_peak
                || resolve_budget_inputs(&row.raw_limits)?.limits != row.effective_limits
            {
                return Err(Error::ResourceInput);
            }
        }
        Ok(())
    }
}

fn check_named_terms(terms: IsolateTerms, named: &BTreeMap<String, u64>) -> Result<(), Error> {
    let expected: BTreeMap<_, _> = terms
        .entries()
        .into_iter()
        .map(|(term, bytes)| (term.name().to_owned(), bytes))
        .collect();
    if &expected != named {
        return Err(Error::ResourceInput);
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AdapterRow {
    pub adapter: AdapterId,
    pub effective_profile_sha256: String,
    pub identity: Identity,
    pub platform_profile: String,
    pub dispatch_profile: String,
    pub memory_scope: String,
    pub platform_facts: PlatformFacts,
    pub dispatch_facts: PlatformFacts,
    #[serde(with = "LimitsDto")]
    pub raw_limits: ResolvedIsolateLimits,
    #[serde(with = "LimitsDto")]
    pub effective_limits: ResolvedIsolateLimits,
    #[serde(with = "TargetConstantsDto")]
    pub target_constants: TargetPlatformConstants,
    #[serde(with = "construction")]
    pub construction_costs: RequestConstructionCosts,
    #[serde(with = "catalog")]
    pub catalog_costs: CatalogResourceCosts,
    pub outbound_meta_bytes: u32,
    pub baked_config_bytes: u64,
    pub baked_config_sha256: String,
    pub actual_gvl_cache_entries: usize,
    pub terms: BTreeMap<String, u64>,
    #[serde(with = "ReceiptsDto")]
    pub auction_receipts: AuctionCharges,
    #[serde(with = "DerivedDto")]
    pub derived: Derived,
    pub validation: Validation,
    pub required_fit: bool,
}

fn row(
    adapter: AdapterId,
    profile: &ResourceProfile,
    identity: &Identity,
    baked_digest: &str,
) -> Result<AdapterRow, Error> {
    let boot = profile.boot_envelope();
    catalog_allowances::check_allowances(boot.catalog_costs)?;
    let inputs = resolve_budget_inputs(boot.limits)?;
    let derived = derive_isolate_budget_with_baked_config(
        &inputs.limits,
        boot.target,
        boot.outbound_meta_bytes,
        boot.construction_costs,
        inputs.baked_config_bytes,
        boot.catalog_costs,
    )
    .map_err(|_error| Error::ResourceInput)?;
    let mut validation = evaluate(
        profile.platform,
        Input {
            total: derived.total_bytes,
            configured: inputs.limits.isolate_total_bytes,
            class: profile.headroom_class,
            scope: Some(profile.memory_scope),
            population: profile.target.platform_concurrent_ceiling,
            headroom: None,
            stack: None,
        },
    )?;
    validation.profile_matches_dispatch = profile.dispatch_matches_platform();
    if !validation.profile_matches_dispatch {
        validation
            .external_blockers
            .push("platform_dispatch_mismatch".to_owned());
        validation.required_fit = false;
    }
    let mut row_identity = identity.clone();
    profile.target_abi.clone_into(&mut row_identity.target_abi);
    row_identity.layout_evidence = layout_evidence(profile);
    // A host layout cannot establish the guest artifact's construction bound.
    let required_fit =
        validation.required_fit && row_identity.layout_evidence == LayoutEvidence::NativeControl;
    let mut result = AdapterRow {
        adapter,
        effective_profile_sha256: String::new(),
        identity: row_identity,
        platform_profile: profile.name.to_owned(),
        dispatch_profile: profile.dispatch_name.to_owned(),
        memory_scope: format!("{:?}", profile.memory_scope),
        platform_facts: PlatformFacts::new(profile.platform),
        dispatch_facts: PlatformFacts::new(profile.dispatch_platform),
        raw_limits: profile.limits,
        effective_limits: inputs.limits,
        target_constants: profile.target,
        construction_costs: boot.construction_costs,
        catalog_costs: boot.catalog_costs,
        outbound_meta_bytes: boot.outbound_meta_bytes,
        baked_config_bytes: inputs.baked_config_bytes,
        baked_config_sha256: baked_digest.to_owned(),
        actual_gvl_cache_entries: GVL_PARSE_CACHE_MAX_ENTRIES,
        terms: derived
            .isolate_terms
            .entries()
            .into_iter()
            .map(|(term, bytes)| (term.name().to_owned(), bytes))
            .collect(),
        auction_receipts: derived.auction_charges,
        derived,
        validation,
        required_fit,
    };
    if usize::try_from(inputs.limits.gvl_cache_entries).map_err(|_error| Error::ResourceInput)?
        < result.actual_gvl_cache_entries
    {
        return Err(Error::ResourceInput);
    }
    result.effective_profile_sha256 = profile_digest(&result)?;
    Ok(result)
}

fn profile_digest(row: &AdapterRow) -> Result<String, Error> {
    let mut value = serde_json::to_value(row).map_err(|_error| Error::ResourceInput)?;
    value
        .as_object_mut()
        .ok_or(Error::ResourceInput)?
        .remove("effective_profile_sha256");
    Ok(hex::encode(Sha256::digest(
        serde_json::to_vec(&value).map_err(|_error| Error::ResourceInput)?,
    )))
}

fn baked_identity() -> Result<(u64, String), Error> {
    let baked = serde_json::to_vec(&(
        auctioneer_config::host_default(),
        auctioneer_config::account_default(),
    ))
    .map_err(|_error| Error::ResourceInput)?;
    let baked_digest = hex::encode(Sha256::digest(&baked));
    Ok((
        u64::try_from(baked.len()).map_err(|_error| Error::ResourceInput)?,
        baked_digest,
    ))
}

fn source_row(
    adapter: AdapterId,
    identity: &Identity,
    baked_digest: &str,
) -> Result<AdapterRow, Error> {
    row(adapter, &adapter.profile(), identity, baked_digest)
}

pub(crate) fn build_report() -> Result<Report, Error> {
    build_report_for(Selection::AllAdapters)
}

pub(crate) fn build_report_for(selection: Selection) -> Result<Report, Error> {
    let identity = source_identity()?;
    let (_bytes, baked_digest) = baked_identity()?;
    let adapters = selection
        .adapters()
        .iter()
        .map(|adapter| source_row(*adapter, &identity, &baked_digest))
        .collect::<Result<Vec<_>, _>>()?;
    let report = Report {
        schema_version: 1,
        accounting_complete: true,
        selection,
        adapters,
    };
    report.validate()?;
    Ok(report)
}

pub(crate) fn serialize(report: &Report) -> Result<Vec<u8>, Error> {
    report.validate()?;
    let mut bytes = serde_json::to_vec(report).map_err(|_error| Error::Output)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_BYTES {
        return Err(Error::Output);
    }
    Ok(bytes)
}

#[derive(Serialize)]
struct CatalogBaselineIdentity {
    code_base_commit: String,
    sha256: String,
    continuity: Option<records::CatalogContinuity>,
}

#[derive(Serialize)]
struct CatalogComparisonReport {
    schema_version: u16,
    scope: &'static str,
    catalog_qualification: &'static str,
    baseline: CatalogBaselineIdentity,
    candidate: Report,
    comparisons: Vec<catalog_comparison::ComparisonRow>,
}

pub(super) fn compare_catalog_baseline(
    baseline_bytes: &[u8],
    require_fit: bool,
) -> Result<(Vec<u8>, i32), Error> {
    let baseline = records::catalog_baseline(baseline_bytes)?;
    let candidate = build_report()?;
    let report = catalog_comparison_report(baseline, candidate)?;
    let exit_code = report.candidate.exit_code(require_fit);
    let mut bytes = serde_json::to_vec(&report).map_err(|_error| Error::Output)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_BYTES {
        return Err(Error::Output);
    }
    Ok((bytes, exit_code))
}

fn catalog_comparison_report(
    baseline: records::CatalogBaseline,
    candidate: Report,
) -> Result<CatalogComparisonReport, Error> {
    candidate.validate()?;
    if candidate.selection != Selection::AllAdapters
        || baseline
            .adapters
            .iter()
            .map(|row| row.adapter)
            .collect::<Vec<_>>()
            != AdapterId::ALL
    {
        return Err(Error::ResourceInput);
    }
    let comparisons = candidate
        .adapters
        .iter()
        .map(|row| {
            let previous = baseline
                .adapters
                .iter()
                .find(|previous| previous.adapter == row.adapter)
                .ok_or(Error::ResourceInput)?;
            catalog_comparison::compare(previous, row)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CatalogComparisonReport {
        schema_version: 1,
        scope: "matched_catalog_arithmetic_only",
        catalog_qualification: "not_evaluated",
        baseline: CatalogBaselineIdentity {
            code_base_commit: baseline.code_base_commit,
            sha256: baseline.sha256,
            continuity: baseline.continuity,
        },
        candidate,
        comparisons,
    })
}

#[cfg(test)]
mod catalog_report_tests {
    use super::{
        AdapterId, AdapterRow, Error, Selection, build_report, build_report_for,
        catalog_comparison_report, records,
    };
    use serde_json::Value;

    fn synthetic_baseline(rows: &[AdapterRow]) -> records::CatalogBaseline {
        records::CatalogBaseline {
            code_base_commit: "1".repeat(40),
            sha256: "2".repeat(64),
            adapters: serde_json::from_value(serde_json::to_value(rows).unwrap()).unwrap(),
            continuity: None,
        }
    }

    #[test]
    fn synthetic_matched_report_carries_all_rows_without_catalog_qualification() {
        let candidate = build_report().unwrap();
        let baseline = synthetic_baseline(&candidate.adapters);
        let assembled = catalog_comparison_report(baseline, candidate).unwrap();
        assert_eq!(assembled.candidate.exit_code(false), 0_i32);
        assert_eq!(assembled.candidate.exit_code(true), 1_i32);
        let value = serde_json::to_value(assembled).unwrap();
        assert_eq!(value["schema_version"], 1_u16);
        assert_eq!(value["scope"], "matched_catalog_arithmetic_only");
        assert_eq!(value["catalog_qualification"], "not_evaluated");
        assert_eq!(value["baseline"]["code_base_commit"], "1".repeat(40));
        assert_eq!(value["baseline"]["sha256"], "2".repeat(64));
        assert!(value["baseline"]["continuity"].is_null());
        assert_eq!(value["candidate"]["adapters"].as_array().unwrap().len(), 4);
        let comparisons = value["comparisons"].as_array().unwrap();
        assert_eq!(comparisons.len(), 4);
        for (row, (adapter, allowance)) in comparisons.iter().zip([
            ("axum", 0x0600_0000_u64),
            ("cloudflare", 0x0800_0000),
            ("fastly", 0x0400_0000),
            ("spin", 0x0400_0000),
        ]) {
            assert_eq!(row["adapter"], adapter);
            assert_eq!(row["total_delta_bytes"], 0_u64);
            assert_eq!(row["total_delta_allowance_bytes"], allowance);
            assert_eq!(row["absolute_required_fit"], false);
            assert!(row.get("qualified").is_none());
            assert_eq!(row["growth"].as_object().unwrap().len(), 7);
            assert!(
                row["growth"]
                    .as_object()
                    .unwrap()
                    .values()
                    .all(|amount| amount == &Value::from(0_u64))
            );
        }
    }

    #[test]
    fn incomplete_duplicate_or_misordered_baseline_rows_refuse_assembly() {
        for operation in ["missing", "duplicate", "reverse"] {
            let candidate = build_report().unwrap();
            let mut baseline = synthetic_baseline(&candidate.adapters);
            match operation {
                "missing" => {
                    baseline.adapters.pop();
                }
                "duplicate" => {
                    let duplicate = serde_json::from_value(
                        serde_json::to_value(&baseline.adapters[0]).unwrap(),
                    )
                    .unwrap();
                    baseline.adapters.push(duplicate);
                }
                "reverse" => baseline.adapters.reverse(),
                _ => panic!("unknown control"),
            }
            assert!(matches!(
                catalog_comparison_report(baseline, candidate),
                Err(Error::ResourceInput)
            ));
        }
        let candidate = build_report_for(Selection::Adapter {
            adapter: AdapterId::Axum,
        })
        .unwrap();
        let baseline = synthetic_baseline(&candidate.adapters);
        assert!(matches!(
            catalog_comparison_report(baseline, candidate),
            Err(Error::ResourceInput)
        ));
    }
}

#[derive(Serialize)]
struct CheckedRecords<'a> {
    scope: &'static str,
    code_base_commit: &'a str,
    audit_sha256: &'a str,
    files: &'a BTreeSet<String>,
}

pub(super) fn capture_audit() -> Result<Vec<u8>, Error> {
    records::capture_audit(build_report()?)
}

pub(super) fn summarize_audit(audit: &[u8]) -> Result<Vec<u8>, Error> {
    let report = build_report()?;
    let profiles = report
        .adapters
        .into_iter()
        .map(|row| (row.adapter, row.effective_profile_sha256))
        .collect();
    records::summarize_audit(audit, &profiles)
}

pub(super) fn check_current_records(audit: &[u8], plan: &[u8]) -> Result<Vec<u8>, Error> {
    let report = build_report()?;
    let records = records::validate_pair(audit, plan)?;
    let profiles: BTreeMap<_, _> = report
        .adapters
        .iter()
        .map(|row| (row.adapter, row.effective_profile_sha256.clone()))
        .collect();
    if profiles != records.profiles {
        return Err(Error::ResourceInput);
    }
    let mut output = serde_json::to_vec(&CheckedRecords {
        scope: "current_source_audit_only_not_human_approval_or_fit",
        code_base_commit: &records.code_base_commit,
        audit_sha256: &records.audit_sha256,
        files: &records.files,
    })
    .map_err(|_error| Error::Output)?;
    output.push(b'\n');
    if output.len() > MAX_BYTES {
        return Err(Error::Output);
    }
    Ok(output)
}

#[cfg(test)]
mod current_records_tests {
    use super::*;
    use crate::bidders::{Command as BidderCommand, run as run_command};
    use std::fs;

    #[test]
    fn source_rows_refuse_each_overdeclared_catalog_component() {
        let identity = source_identity().unwrap();
        let (_, digest) = baked_identity().unwrap();
        let accepted = CatalogResourceCosts {
            resident_bytes: 101,
            initialization_bytes: 211,
            schema_validation_per_auction_bytes: 307,
            context_per_auction_bytes: 401,
        };
        let mut profile = axum::resource_profile();
        profile.catalog_costs = accepted;
        let control = row(AdapterId::Axum, &profile, &identity, &digest).unwrap();
        assert_eq!(control.catalog_costs, accepted);
        assert_eq!(control.derived.catalog_resident_term, 101);
        assert_eq!(control.derived.catalog_initialization_term, 211);

        for costs in [
            CatalogResourceCosts {
                resident_bytes: 0x0100_0001,
                ..accepted
            },
            CatalogResourceCosts {
                initialization_bytes: 0x0080_0001,
                ..accepted
            },
            CatalogResourceCosts {
                schema_validation_per_auction_bytes: 0x0008_0001,
                ..accepted
            },
        ] {
            profile.catalog_costs = costs;
            assert_eq!(
                row(AdapterId::Axum, &profile, &identity, &digest).map(|_| ()),
                Err(Error::ResourceInput),
                "overdeclared component: {costs:?}",
            );
        }
    }

    #[test]
    fn accepted_catalog_declarations_cannot_promote_target_readiness() {
        let identity = source_identity().unwrap();
        let (_, digest) = baked_identity().unwrap();
        for adapter in AdapterId::ALL {
            let mut profile = adapter.profile();
            profile.catalog_costs = CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            };
            let report = row(adapter, &profile, &identity, &digest).unwrap();
            assert_eq!(report.derived.catalog_resident_term, 101);
            assert_eq!(report.derived.catalog_initialization_term, 211);
            assert!(!report.required_fit);
            assert!(!report.validation.external_blockers.is_empty());
        }
    }

    #[test]
    fn resource_rows_consume_adapter_profiles_not_target_specific_platform_rules() {
        let identity = source_identity().unwrap();
        let (_, digest) = baked_identity().unwrap();
        let fastly = row(
            AdapterId::Axum,
            &fastly::resource_profile(),
            &identity,
            &digest,
        )
        .unwrap();
        assert_eq!(fastly.platform_profile, "fastly");
        assert_eq!(fastly.dispatch_profile, "fastly");
        assert_eq!(fastly.memory_scope, "PerExecution");
        assert_eq!(fastly.identity.target_abi, "wasm32-wasip1");
        assert_eq!(
            fastly.identity.layout_evidence,
            LayoutEvidence::HostNativeSubstitute
        );
        assert_eq!(fastly.validation.minimum_headroom_bytes, Some(8_000_000));
        assert_eq!(
            fastly.validation.separate_stack_allowance_bytes,
            Some(1_000_000)
        );
        assert!(fastly.validation.profile_matches_dispatch);

        let mut profile = spin::resource_profile();
        profile.dispatch_platform = edgezero_adapter_spin::SPIN_PLATFORM;
        profile.dispatch_name = "spin_generic";
        let mismatched = row(AdapterId::Fastly, &profile, &identity, &digest).unwrap();
        assert!(!mismatched.validation.profile_matches_dispatch);
        assert!(
            mismatched
                .validation
                .external_blockers
                .iter()
                .any(|reason| reason == "platform_dispatch_mismatch")
        );
        let hosted_profile = spin::resource_profile();
        let aligned = row(AdapterId::Spin, &hosted_profile, &identity, &digest).unwrap();
        assert!(aligned.validation.profile_matches_dispatch);
        assert!(
            !aligned
                .validation
                .external_blockers
                .iter()
                .any(|reason| reason == "platform_dispatch_mismatch")
        );
        assert!(
            !aligned.required_fit,
            "matching metadata cannot prove guest memory or clear other blockers"
        );
    }

    #[test]
    fn selected_report_requires_exactly_the_named_adapter_and_declares_its_scope() {
        for adapter in AdapterId::ALL {
            let mut report = build_report_for(Selection::Adapter { adapter }).unwrap();
            assert_eq!(report.selection, Selection::Adapter { adapter });
            assert_eq!(report.adapters.len(), 1);
            assert_eq!(report.adapters[0].adapter, adapter);
            serialize(&report).unwrap();
            report.selection = Selection::AllAdapters;
            assert_eq!(serialize(&report), Err(Error::ResourceInput));
            report.selection = Selection::Adapter { adapter };
            let next = AdapterId::ALL
                .into_iter()
                .find(|other| *other != adapter)
                .unwrap();
            report.adapters[0].adapter = next;
            assert_eq!(serialize(&report), Err(Error::ResourceInput));
            report.adapters.clear();
            assert_eq!(serialize(&report), Err(Error::ResourceInput));
        }
    }

    #[test]
    fn current_record_entrypoint_requires_the_actual_clean_source_profiles() {
        let report = build_report().unwrap();
        let identity = &report.adapters.first().unwrap().identity;
        let (audit, plan) = records::test_pair(&identity.code_base_commit);
        let result = check_current_records(&audit, &plan);
        if identity.dirty || !identity.source_tree_matches_commit {
            assert_eq!(result, Err(Error::ResourceInput));
        } else {
            let parsed: serde_json::Value = serde_json::from_slice(&result.unwrap()).unwrap();
            assert_eq!(
                parsed["scope"],
                "current_source_audit_only_not_human_approval_or_fit"
            );
            assert_eq!(parsed["code_base_commit"], identity.code_base_commit);
            let directory = tempfile::tempdir().unwrap();
            let audit_path = directory.path().join("audit.toml");
            let plan_path = directory.path().join("plan.toml");
            fs::write(&audit_path, audit).unwrap();
            fs::write(&plan_path, plan).unwrap();
            assert_eq!(
                run_command(BidderCommand::Resources {
                    adapter: None,
                    all_adapters: true,
                    capture_audit: false,
                    check: true,
                    require_fit: false,
                    catalog_baseline: None,
                    audit: Some(audit_path),
                    plan: Some(plan_path),
                    review_commit: None,
                    staged: false,
                    head: None,
                }),
                0_i32
            );
        }
    }
}

fn source_identity() -> Result<Identity, Error> {
    source_identity_at(Path::new(ROOT))
}

fn source_identity_at(root: &Path) -> Result<Identity, Error> {
    let built: BuildIdentity = serde_json::from_slice(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/resource-identity.json"
    )))
    .map_err(|_error| Error::ResourceInput)?;
    if snapshot(root).map_err(|_error| Error::ResourceInput)? != built.snapshot {
        return Err(Error::ResourceInput);
    }
    let manifest: toml::Value = toml::from_str(include_str!("../../../../../Cargo.toml"))
        .map_err(|_error| Error::ResourceInput)?;
    let sdk_revision = manifest
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("edgezero-core"))
        .and_then(|value| value.get("rev"))
        .and_then(toml::Value::as_str)
        .ok_or(Error::ResourceInput)?
        .to_owned();
    let lock_sha256 = built
        .snapshot
        .files
        .get("Cargo.lock")
        .and_then(Option::as_ref)
        .ok_or(Error::ResourceInput)?
        .clone();
    let mut features = vec!["application"];
    features.extend(build_features());
    Ok(Identity {
        code_base_commit: built.code_base_commit,
        dirty: !built.source_tree_matches_commit,
        source_tree_matches_commit: built.source_tree_matches_commit,
        sdk_revision,
        lock_sha256,
        declared_toolchain: include_str!("../../../../../.tool-versions").to_owned(),
        features: features.into_iter().map(str::to_owned).collect(),
        source_sha256: built.snapshot.sha256,
        source_file_count: built.snapshot.files.len(),
        compiler_verbose: built.compiler_verbose,
        compiled_target: built.compiled_target,
        reporter_abi: format!("{ARCH}-{OS}-{}bit", usize::BITS),
        target_abi: "pending".to_owned(),
        layout_evidence: LayoutEvidence::HostNativeSubstitute,
        artifact_proof: "pending_not_a_deployment_observation".to_owned(),
    })
}

macro_rules! remote_dto {
    ($name:ident, $model:literal { $($(#[$attribute:meta])* $field:ident: $kind:ty),+ $(,)? }) => {
        #[derive(Serialize, Deserialize)]
        #[serde(remote = $model, deny_unknown_fields)]
        struct $name {
            $($(#[$attribute])* $field: $kind),+
        }
    };
}

macro_rules! mapped_dto {
    ($module:ident, $model:ident { $($(#[$attribute:meta])* $wire:ident => $field:ident),+ $(,)? }) => {
        mod $module {
            use super::$model;
            use serde::{Deserialize, Serialize};

            #[derive(Serialize, Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Wire { $($(#[$attribute])* $wire: u64),+ }

            pub(super) fn serialize<S: serde::Serializer>(input: &$model, serializer: S) -> Result<S::Ok, S::Error> {
                let $model { $($field),+ } = *input;
                Wire { $($wire: $field),+ }.serialize(serializer)
            }

            pub(super) fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<$model, D::Error> {
                let Wire { $($wire),+ } = Wire::deserialize(deserializer)?;
                Ok($model { $($field: $wire),+ })
            }
        }
    };
}

mapped_dto!(construction, RequestConstructionCosts {
    #[serde(rename = "fixed_per_auction_bytes")]
    fixed => fixed_per_auction_bytes,
    #[serde(rename = "reusable_scratch_bytes")]
    scratch => reusable_scratch_bytes,
    #[serde(rename = "retained_per_scope_bytes")]
    retained => retained_per_scope_bytes,
});

mapped_dto!(catalog, CatalogResourceCosts {
    #[serde(rename = "resident_bytes")]
    resident => resident_bytes,
    #[serde(rename = "initialization_bytes")]
    initialization => initialization_bytes,
    #[serde(rename = "schema_validation_per_auction_bytes")]
    validation => schema_validation_per_auction_bytes,
    #[serde(rename = "context_per_auction_bytes")]
    context => context_per_auction_bytes,
});

remote_dto!(ReceiptsDto, "AuctionCharges" {
    config: u64,
    ingress: u64,
    privacy: u64,
    projection: u64,
    outbound: u64,
    response: u64,
});

remote_dto!(LimitsDto, "ResolvedIsolateLimits" {
    isolate_total_bytes: u64,
    baseline_bytes: u64,
    inbound_request_bytes: u64,
    inbound_parse_factor: u64,
    max_outbound_request_bytes: u64,
    max_outbound_raw_bytes: u64,
    max_transport_bytes: u64,
    max_decoded_bytes: u64,
    response_parse_factor: u64,
    response_node_peak_bytes: u64,
    final_response_bytes: u64,
    egress_permits: u32,
    response_write_deadline_ms: u64,
    guest_decoder_bound_bytes: u64,
    aux_route_permits: u32,
    aux_route_peak_bytes: u64,
    max_brotli_wbits: u8,
    max_brotli_layers: u8,
    gvl_cache_entries: u32,
    gvl_parsed_bytes: u64,
    max_bidders_per_auction: u32,
    max_outbound_requests: u32,
    max_concurrent_auctions: u32,
    max_active_body_streams: u32,
    max_runtime_config_bytes: u64,
    max_runtime_config_blob_bytes: u64,
    max_parsed_nodes: u32,
    max_copied_header_bytes: u32,
    max_admitted_header_bytes: u32,
    max_admitted_header_count: u16,
    max_impressions_per_auction: u16,
    max_config_json_nodes: u32,
    terminal_response_permits: u32,
    refusal_body_bytes: u32,
    transmit_state_bytes: u32,
});

remote_dto!(TargetConstantsDto, "TargetPlatformConstants" {
    platform_concurrent_ceiling: u32,
    header_entry_overhead_bytes: u32,
    parser_header_ceiling_bytes: u32,
    parser_header_knob_bytes: u32,
    max_request_target_bytes: u32,
    h1_max_buf_size_bytes: u32,
    h1_max_headers: u32,
    parser_max_header_entries: u32,
    pseudoheader_allowance_bytes: u32,
    parser_transient_bytes: u32,
    egress_handoff_factor: u32,
});

mapped_dto!(isolate_terms, IsolateTerms {
    baseline => baseline_bytes,
    gvl => gvl_bytes,
    auctions => auctions_bytes,
    auction_egress => auction_egress_bytes,
    auxiliary => auxiliary_bytes,
    terminal_egress => terminal_egress_bytes,
    probe_output => probe_output_bytes,
    pre_admission_parser => pre_admission_parser_bytes,
    body_streams => body_streams_bytes,
    catalog_resident => catalog_resident_bytes,
    catalog_initialization => catalog_initialization_bytes,
});

remote_dto!(DerivedDto, "Derived" {
    #[serde(with = "isolate_terms")]
    isolate_terms: auctioneer_runtime::IsolateTerms,
    #[serde(with = "ReceiptsDto")]
    auction_charges: AuctionCharges,
    #[serde(with = "construction")]
    construction_costs: RequestConstructionCosts,
    catalog_resident_term: u64,
    catalog_initialization_term: u64,
    schema_validation_term: u64,
    catalog_context_term: u64,
    runtime_config_term: u64,
    request_side_term: u64,
    call_side_term: u64,
    bookkeeping_term: u64,
    debug_term: u64,
    assembly_scratch_bytes: u64,
    per_auction_peak: u64,
    per_stream_peak: u64,
    decoder_chain_bytes: u64,
    gvl_terms: u64,
    egress_term: u64,
    aux_term: u64,
    terminal_term: u64,
    probe_output_term: u64,
    h2_slot_peak: u64,
    h1_slot_peak: u64,
    pre_admission_parser_term: u64,
    total_bytes: u64,
    margin_bytes: u64,
    max_outbound_raw_bytes_usize: usize,
});

#[cfg(test)]
mod tests {
    #[test]
    fn unqualified_resource_refusal_keeps_its_explicit_identity() {
        use super::super::Error;
        use auctioneer_runtime::BudgetError;
        assert_eq!(
            Error::from(BudgetError::UnqualifiedSchemaArtifact).code(),
            "unqualified_schema_artifact"
        );
        assert_eq!(
            Error::from(BudgetError::BudgetArithmeticOverflow).code(),
            "bidder_resource_input"
        );
    }

    use super::{AdapterId, ROOT, build_report, check_named_terms, serialize, source_identity_at};
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::str::from_utf8;
    use tempfile::TempDir;

    fn copied_sources() -> TempDir {
        let root = tempfile::tempdir().unwrap();
        let cloned = Command::new("git")
            .args(["clone", "--shared", "--no-checkout", "--quiet", ROOT])
            .arg(root.path())
            .output()
            .unwrap();
        assert!(cloned.status.success(), "{cloned:?}");
        let indexed = Command::new("git")
            .current_dir(root.path())
            .args(["read-tree", "HEAD"])
            .output()
            .unwrap();
        assert!(indexed.status.success(), "{indexed:?}");
        let listing = Command::new("git")
            .current_dir(ROOT)
            .args([
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ])
            .output()
            .unwrap();
        assert!(listing.status.success(), "{listing:?}");
        for bytes in listing
            .stdout
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
        {
            let path = Path::new(from_utf8(bytes).unwrap());
            let source = Path::new(ROOT).join(path);
            if source.is_file() {
                let destination = root.path().join(path);
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(source, destination).unwrap();
            }
        }
        root
    }

    #[test]
    fn compiled_source_identity_detects_baked_layout_added_and_deleted_inputs() {
        let root = copied_sources();
        source_identity_at(root.path()).unwrap();
        let mut rejected = Vec::new();
        for relative in [
            "auctioneer.toml",
            "crates/auctioneer-config/build.rs",
            "crates/auctioneer-config/data/gvl_archive/gcl.json",
            "crates/auctioneer-exchange/src/scrub/value.rs",
            "crates/auctioneer-core/src/handlers/request_disclosure_audit.rs",
        ] {
            let path = root.path().join(relative);
            let original = fs::read(&path).unwrap();
            let mut changed = original.clone();
            changed.push(b'\n');
            fs::write(&path, changed).unwrap();
            rejected.push((relative, source_identity_at(root.path()).is_err()));
            fs::write(&path, &original).unwrap();
            fs::remove_file(&path).unwrap();
            rejected.push((relative, source_identity_at(root.path()).is_err()));
            fs::write(&path, original).unwrap();
        }
        let added = root
            .path()
            .join("crates/auctioneer-runtime/src/resource_identity_added.rs");
        fs::write(&added, b"// isolated source addition\n").unwrap();
        rejected.push(("added", source_identity_at(root.path()).is_err()));
        assert!(
            rejected.iter().all(|(_path, refused)| *refused),
            "{rejected:?}"
        );
    }

    #[test]
    fn evidence_records_do_not_change_code_identity_but_rust_in_their_trees_does() {
        let root = copied_sources();
        let before = source_identity_at(root.path()).unwrap().source_sha256;
        for relative in [
            "docs/testing/resource-identity-generated.json",
            "crates/auctioneer-core/tests/fixtures/resource_envelopes/audit.toml",
            "crates/auctioneer-core/tests/fixtures/resource_envelopes/report.json",
        ] {
            let path = root.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"generated evidence, not compiled source\n").unwrap();
        }
        assert_eq!(
            source_identity_at(root.path()).unwrap().source_sha256,
            before
        );
        for relative in [
            "docs/testing/resource_identity_owner.rs",
            "crates/auctioneer-core/tests/fixtures/resource_envelopes/resource_identity_owner.rs",
        ] {
            let path = root.path().join(relative);
            fs::write(&path, b"// isolated Rust owner\n").unwrap();
            assert_eq!(
                source_identity_at(root.path()).unwrap_err(),
                super::Error::ResourceInput
            );
            fs::remove_file(&path).unwrap();
        }
        let original = root
            .path()
            .join("crates/auctioneer-exchange/src/scrub/value.rs");
        let moved = root
            .path()
            .join("docs/testing/moved_resource_identity_owner.rs");
        fs::rename(original, moved).unwrap();
        assert_eq!(
            source_identity_at(root.path()).unwrap_err(),
            super::Error::ResourceInput
        );
    }

    #[test]
    fn report_records_actual_compiler_and_target_without_repeating_source_maps() {
        let report = build_report().unwrap();
        let value = serde_json::to_value(report).unwrap();
        for row in value["adapters"].as_array().unwrap() {
            let identity = &row["identity"];
            assert!(
                identity["compiler_verbose"]
                    .as_str()
                    .unwrap()
                    .contains("commit-hash:")
            );
            assert!(!identity["compiled_target"].as_str().unwrap().is_empty());
            assert!(identity["source_file_count"].as_u64().unwrap() > 1_000);
            assert!(identity.get("source_files").is_none());
        }
    }

    #[test]
    fn strict_exit_requires_every_synthetic_row_to_fit() {
        let mut report = build_report().unwrap();
        for mask in 0_u8..16 {
            for (index, row) in report.adapters.iter_mut().enumerate() {
                row.required_fit = mask & (1_u8 << index) != 0;
            }
            assert_eq!(report.exit_code(false), 0_i32);
            assert_eq!(report.exit_code(true), i32::from(mask != 15));
            report.accounting_complete = false;
            assert_eq!(report.exit_code(false), 1_i32);
            assert_eq!(report.exit_code(true), 1_i32);
            report.accounting_complete = true;
        }
    }

    #[test]
    fn report_uses_normalized_baked_inputs_and_all_runtime_owned_terms() {
        let report = build_report().unwrap();
        assert_eq!(
            report
                .adapters
                .iter()
                .map(|row| row.adapter)
                .collect::<Vec<_>>(),
            AdapterId::ALL
        );
        let baked = serde_json::to_vec(&(
            auctioneer_config::host_default(),
            auctioneer_config::account_default(),
        ))
        .unwrap();
        for row in &report.adapters {
            assert_eq!(row.baked_config_bytes, u64::try_from(baked.len()).unwrap());
            assert_eq!(row.terms.values().sum::<u64>(), row.derived.total_bytes);
            assert_eq!(
                row.auction_receipts.total().unwrap(),
                row.derived.per_auction_peak
            );
        }
        assert!(serialize(&report).unwrap().len() <= 1 << 20_usize);
    }

    #[test]
    fn every_named_term_rejects_missing_wrong_bytes_and_unknown_extras() {
        let mut report = build_report().unwrap();
        let names = [
            "baseline",
            "gvl",
            "auctions",
            "auction_egress",
            "auxiliary",
            "terminal_egress",
            "probe_output",
            "pre_admission_parser",
            "body_streams",
            "catalog_resident",
            "catalog_initialization",
        ];
        let expected_catalog_terms = [
            (AdapterId::Axum, 0x0100_0000, 0x0080_0000),
            (AdapterId::Cloudflare, 0, 0),
            (AdapterId::Fastly, 0, 0),
            (AdapterId::Spin, 0, 0),
        ];
        assert_eq!(report.adapters.len(), expected_catalog_terms.len());
        for (index, (adapter, resident, initialization)) in
            expected_catalog_terms.into_iter().enumerate()
        {
            assert_eq!(report.adapters[index].adapter, adapter);
            assert_eq!(report.adapters[index].terms.len(), names.len());
            assert_eq!(report.adapters[index].terms["catalog_resident"], resident);
            assert_eq!(
                report.adapters[index].terms["catalog_initialization"],
                initialization
            );
            for name in names {
                let bytes = report.adapters[index].terms.remove(name).unwrap();
                assert!(
                    check_named_terms(
                        report.adapters[index].derived.isolate_terms,
                        &report.adapters[index].terms
                    )
                    .is_err(),
                    "missing {name} in row {index}"
                );
                report.adapters[index]
                    .terms
                    .insert(name.to_owned(), bytes + 1);
                assert!(
                    check_named_terms(
                        report.adapters[index].derived.isolate_terms,
                        &report.adapters[index].terms
                    )
                    .is_err(),
                    "wrong {name} in row {index}"
                );
                report.adapters[index].terms.insert(name.to_owned(), bytes);
            }
            report.adapters[index]
                .terms
                .insert("unknown_extra".to_owned(), 0);
            assert!(
                check_named_terms(
                    report.adapters[index].derived.isolate_terms,
                    &report.adapters[index].terms
                )
                .is_err(),
                "extra term in row {index}"
            );
            report.adapters[index].terms.remove("unknown_extra");
            check_named_terms(
                report.adapters[index].derived.isolate_terms,
                &report.adapters[index].terms,
            )
            .unwrap();
        }
        report.validate().unwrap();
    }

    #[test]
    fn source_report_refuses_rehashed_missing_and_wrong_catalog_terms() {
        let mut report = build_report().unwrap();
        let expected_catalog_terms = [
            (AdapterId::Axum, 0x0100_0000, 0x0080_0000),
            (AdapterId::Cloudflare, 0, 0),
            (AdapterId::Fastly, 0, 0),
            (AdapterId::Spin, 0, 0),
        ];
        assert_eq!(report.adapters.len(), expected_catalog_terms.len());
        for (index, (adapter, resident, initialization)) in
            expected_catalog_terms.into_iter().enumerate()
        {
            assert_eq!(report.adapters[index].adapter, adapter);
            let digest = report.adapters[index].effective_profile_sha256.clone();
            for (name, expected) in [
                ("catalog_resident", resident),
                ("catalog_initialization", initialization),
            ] {
                assert_eq!(report.adapters[index].terms[name], expected);
                let wrong = u64::from(expected == 0);
                for replacement in [None, Some(wrong)] {
                    if let Some(bytes) = replacement {
                        report.adapters[index].terms.insert(name.to_owned(), bytes);
                    } else {
                        report.adapters[index].terms.remove(name);
                    }
                    report.adapters[index].effective_profile_sha256 =
                        super::profile_digest(&report.adapters[index]).unwrap();
                    assert!(
                        report.validate().is_err(),
                        "rehashed {name} in row {index}: {replacement:?}"
                    );
                    report.adapters[index]
                        .terms
                        .insert(name.to_owned(), expected);
                    report.adapters[index].effective_profile_sha256 = digest.clone();
                }
            }
        }
        report.validate().unwrap();
    }

    #[test]
    fn source_report_refuses_independent_row_and_validation_fit_mutations() {
        let mut report = build_report().unwrap();
        for index in 0..report.adapters.len() {
            let digest = report.adapters[index].effective_profile_sha256.clone();
            let expected = report.adapters[index].required_fit;
            report.adapters[index].required_fit = !expected;
            assert!(report.validate().is_err(), "row required_fit {index}");
            // Repair the self-digest too: the independently constructed source
            // row must still refuse promotion, not just an unrehashed edit.
            report.adapters[index].effective_profile_sha256 =
                super::profile_digest(&report.adapters[index]).unwrap();
            assert!(
                report.validate().is_err(),
                "rehashed row required_fit {index}"
            );
            report.adapters[index].required_fit = expected;
            report.adapters[index].effective_profile_sha256 = digest.clone();
            let expected_validation_fit = report.adapters[index].validation.required_fit;
            report.adapters[index].validation.required_fit = !expected_validation_fit;
            assert!(
                report.validate().is_err(),
                "validation required_fit {index}"
            );
            report.adapters[index].effective_profile_sha256 =
                super::profile_digest(&report.adapters[index]).unwrap();
            assert!(
                report.validate().is_err(),
                "rehashed validation required_fit {index}"
            );
            report.adapters[index].validation.required_fit = expected_validation_fit;
            report.adapters[index].effective_profile_sha256 = digest;
        }
        report.validate().unwrap();
    }

    #[test]
    fn missing_rows_zero_baked_and_raw_shortcuts_refuse() {
        let mut report = build_report().unwrap();
        let saved = report.adapters.pop().unwrap();
        assert!(report.validate().is_err());
        report.adapters.push(saved);
        report.validate().unwrap();
        let mut zero_baked = build_report().unwrap();
        zero_baked.adapters.first_mut().unwrap().baked_config_bytes = 0;
        assert!(zero_baked.validate().is_err());
        let mut raw_shortcut = build_report().unwrap();
        raw_shortcut
            .adapters
            .first_mut()
            .unwrap()
            .effective_limits
            .max_runtime_config_bytes = 0;
        assert!(raw_shortcut.validate().is_err());
    }

    #[test]
    fn positive_but_wrong_baked_bytes_and_stale_identity_refuse() {
        let mut report = build_report().unwrap();
        report.adapters.first_mut().unwrap().baked_config_bytes += 1;
        assert!(report.validate().is_err());
        let mut stale = build_report().unwrap();
        stale.adapters.first_mut().unwrap().identity.source_sha256 = "0".repeat(64);
        assert!(stale.validate().is_err());
    }
}

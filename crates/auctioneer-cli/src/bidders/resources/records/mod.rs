//! Strict, bounded records for the resource review checkpoint.

mod capture;
mod catalog_continuity;

use super::super::Error;
use super::super::resources_platform::SdkResult;
use super::{AdapterId, AdapterRow};
use auctioneer_core::host_primitives::resolve_budget_inputs;
use auctioneer_privacy::gvl::GVL_PARSE_CACHE_MAX_ENTRIES;
use auctioneer_runtime::aux_work::AuxWorkCharges;
use auctioneer_runtime::budget::derive_aux_work_for_profile;
use auctioneer_runtime::{
    AuctionCharges, IsolateTerm, IsolateTerms, PROBE_CONFIG_MIN_PERMITS, PROBE_RESPONSE_PERMITS,
    derive_isolate_budget_with_baked_config,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::str::from_utf8;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Audit {
    schema_version: u16,
    code_base_commit: String,
    sdk_revision: String,
    lock_sha256: String,
    toolchain: String,
    features: Vec<String>,
    runtime_facts: RuntimeFacts,
    adapters: Vec<AdapterAudit>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeFacts {
    probe_config_min_permits: u32,
    probe_response_permits: u32,
    gvl_cache_max_entries: usize,
}

impl RuntimeFacts {
    const fn current() -> Self {
        Self {
            probe_config_min_permits: PROBE_CONFIG_MIN_PERMITS,
            probe_response_permits: PROBE_RESPONSE_PERMITS,
            gvl_cache_max_entries: GVL_PARSE_CACHE_MAX_ENTRIES,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterAudit {
    report: AdapterRow,
    #[serde(with = "AuxReceiptsDto")]
    auxiliary_receipts: AuxWorkCharges,
    component_receipts: Vec<ComponentReceipt>,
    owners: Vec<Ownership>,
    decisions: Vec<Decision>,
    artifact: Evidence<Artifact>,
    abi: Evidence<AbiBound>,
    pending_facts: Vec<PendingFact>,
    gap_bytes: Evidence<u64>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "AuxWorkCharges", deny_unknown_fields)]
struct AuxReceiptsDto {
    config: u64,
    body: u64,
    query: u64,
    input_headers: u64,
    cookie: u64,
    signing: u64,
    nodes: u64,
    privacy: u64,
    chooser: u64,
    urls: u64,
    retained_urls: u64,
    output: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum Evidence<T> {
    Known {
        value: T,
        source: String,
    },
    Pending {
        reason: String,
        owner: String,
        next_action: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    sha256: String,
    source_sha256: String,
    target_abi: String,
    toolchain: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AbiBound {
    upper_bound_bytes: u64,
    artifact_sha256: String,
    abi: String,
    toolchain: String,
    covered_paths: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingFact {
    fact: String,
    reason: String,
    owner: String,
    next_action: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership {
    term_id: TermId,
    allocation_owner: String,
    source_path: String,
    acquire: String,
    release: String,
    population: Evidence<u64>,
    scope: Evidence<String>,
    enforcement: Evidence<String>,
    overlapping_copies: String,
    abi_bound: Evidence<AbiBound>,
    disposition: ChangeClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
struct TermId(IsolateTerm);

impl From<TermId> for String {
    fn from(term: TermId) -> Self {
        term.0.name().to_owned()
    }
}

impl TryFrom<String> for TermId {
    type Error = &'static str;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        IsolateTerm::ALL
            .iter()
            .find(|term| term.name() == name)
            .copied()
            .map(Self)
            .ok_or("unknown runtime term")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ChangeClass {
    Unchanged,
    ImplementationImprovement,
    AccountingCorrection,
    WorkloadProfileChange,
    ExternalPrerequisite,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Subject {
    Term { term_id: TermId },
    Component { component: Component },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Component {
    AuctionConfig,
    AuctionIngress,
    AuctionPrivacy,
    AuctionProjection,
    AuctionOutbound,
    AuctionResponse,
    RuntimeConfig,
    RequestSide,
    CallSide,
    Bookkeeping,
    Debug,
    Assembly,
    AuxConfig,
    AuxBody,
    AuxQuery,
    AuxInputHeaders,
    AuxCookie,
    AuxSigning,
    AuxNodes,
    AuxPrivacy,
    AuxChooser,
    AuxUrls,
    AuxRetainedUrls,
    AuxOutput,
    AuxDeclaredWork,
    AuxHandoff,
    AuxTransmit,
    AuxRoutes,
    ProbeConfig,
}

impl Component {
    const ALL: &'static [Self] = &[
        Self::AuctionConfig,
        Self::AuctionIngress,
        Self::AuctionPrivacy,
        Self::AuctionProjection,
        Self::AuctionOutbound,
        Self::AuctionResponse,
        Self::RuntimeConfig,
        Self::RequestSide,
        Self::CallSide,
        Self::Bookkeeping,
        Self::Debug,
        Self::Assembly,
        Self::AuxConfig,
        Self::AuxBody,
        Self::AuxQuery,
        Self::AuxInputHeaders,
        Self::AuxCookie,
        Self::AuxSigning,
        Self::AuxNodes,
        Self::AuxPrivacy,
        Self::AuxChooser,
        Self::AuxUrls,
        Self::AuxRetainedUrls,
        Self::AuxOutput,
        Self::AuxDeclaredWork,
        Self::AuxHandoff,
        Self::AuxTransmit,
        Self::AuxRoutes,
        Self::ProbeConfig,
    ];
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComponentReceipt {
    component: Component,
    bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    subject: Subject,
    class: ChangeClass,
    owner: String,
    rationale: String,
    next_action: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema_version: u16,
    code_base_commit: String,
    audit_sha256: String,
    packages: Vec<Package>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    id: String,
    adapter: AdapterId,
    changes: Vec<Change>,
    projected_total_bytes: u64,
    savings_bytes: u64,
    residual_gap_bytes: Evidence<u64>,
    files: Vec<String>,
    test_commands: Vec<String>,
    disposition: Disposition,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    id: String,
    class: ChangeClass,
    owner: String,
    functions: Vec<String>,
    old_lifetime: String,
    new_lifetime: String,
    evidence: String,
    #[serde(with = "super::isolate_terms")]
    before: IsolateTerms,
    #[serde(with = "super::isolate_terms")]
    after: IsolateTerms,
    savings_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    workload: Option<WorkloadChange>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkloadChange {
    #[serde(with = "super::LimitsDto")]
    old_limits: auctioneer_runtime::ResolvedIsolateLimits,
    #[serde(with = "super::LimitsDto")]
    new_limits: auctioneer_runtime::ResolvedIsolateLimits,
    product_impact: String,
    deployment_scope: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Disposition {
    Full(#[serde(with = "super::super::empty_object")] ()),
    Partial {
        unresolved_terms: Vec<TermId>,
        owner: String,
        next_action: String,
    },
}

#[derive(Debug)]
pub(super) struct ValidatedRecords {
    pub code_base_commit: String,
    pub files: BTreeSet<String>,
    pub audit_sha256: String,
    pub lock_sha256: String,
    pub sdk_revision: String,
    pub toolchain: String,
    pub profiles: BTreeMap<AdapterId, String>,
}

#[derive(Clone, Copy)]
enum Mode {
    Current,
    Frozen,
}

pub(super) struct CatalogBaseline {
    pub code_base_commit: String,
    pub sha256: String,
    pub adapters: Vec<AdapterRow>,
    pub continuity: Option<CatalogContinuity>,
}

#[derive(Debug, Serialize)]
pub(super) struct CatalogContinuity {
    original_code_base_commit: String,
    original_sha256: String,
    adapters: Vec<PreservedChooserGrowth>,
}

#[derive(Debug, Serialize)]
struct PreservedChooserGrowth {
    adapter: AdapterId,
    original_total_bytes: u64,
    rebased_total_bytes: u64,
    auxiliary_chooser_per_permit_delta_bytes: u64,
    auxiliary_chooser_component_delta_bytes: u64,
}

pub(super) fn catalog_baseline(bytes: &[u8]) -> Result<CatalogBaseline, Error> {
    require(bytes.len() <= super::MAX_BYTES)?;
    let sha256 = hex::encode(Sha256::digest(bytes));
    match sha256.as_str() {
        "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39" => {
            catalog_baseline_projection(validate_audit_mode(bytes, Mode::Frozen)?, sha256)
        }
        "1d20dfa95f09a32a45f106f5503094efd71a718d794814fac353f83f357219d0" => {
            let original_bytes = include_bytes!(
                "../../../../../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml"
            );
            require(
                hex::encode(Sha256::digest(original_bytes))
                    == "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39",
            )?;
            let original = validate_audit_mode(original_bytes, Mode::Frozen)?;
            let audit = validate_audit(bytes)?;
            let continuity = catalog_continuity::verify(original, &audit)?;
            let mut baseline = catalog_baseline_projection(audit, sha256)?;
            baseline.continuity = Some(continuity);
            Ok(baseline)
        }
        _ => Err(Error::ResourceInput),
    }
}

fn catalog_baseline_projection(audit: Audit, sha256: String) -> Result<CatalogBaseline, Error> {
    require(audit.runtime_facts == RuntimeFacts::current())?;
    Ok(CatalogBaseline {
        code_base_commit: audit.code_base_commit,
        sha256,
        adapters: audit
            .adapters
            .into_iter()
            .map(|target| target.report)
            .collect(),
        continuity: None,
    })
}

pub(super) fn validate_pair(
    audit_bytes: &[u8],
    plan_bytes: &[u8],
) -> Result<ValidatedRecords, Error> {
    validate_pair_mode(audit_bytes, plan_bytes, Mode::Current)
}

pub(super) fn validate_frozen_pair(
    audit_bytes: &[u8],
    plan_bytes: &[u8],
) -> Result<ValidatedRecords, Error> {
    validate_pair_mode(audit_bytes, plan_bytes, Mode::Frozen)
}

fn validate_pair_mode(
    audit_bytes: &[u8],
    plan_bytes: &[u8],
    mode: Mode,
) -> Result<ValidatedRecords, Error> {
    let audit = match mode {
        Mode::Current => validate_audit(audit_bytes)?,
        Mode::Frozen => validate_audit_mode(audit_bytes, mode)?,
    };
    let plan: Plan = parse(plan_bytes)?;
    let audit_sha256 = hex::encode(Sha256::digest(audit_bytes));
    require(
        plan.schema_version == 1
            && plan.code_base_commit == audit.code_base_commit
            && plan.audit_sha256 == audit_sha256
            && !plan.packages.is_empty(),
    )?;
    let mut ids = BTreeSet::new();
    let mut files = BTreeSet::new();
    let mut states: Vec<_> = audit
        .adapters
        .iter()
        .map(|target| {
            (
                target.report.adapter,
                target.report.derived.isolate_terms,
                target.report.raw_limits,
            )
        })
        .collect();
    for package in &plan.packages {
        require(concrete(&package.id) && ids.insert(&package.id))?;
        let target = audit
            .adapters
            .iter()
            .find(|row| row.report.adapter == package.adapter)
            .ok_or(Error::ResourceInput)?;
        let (_, state, limits) = states
            .iter_mut()
            .find(|(identity, _state, _limits)| *identity == package.adapter)
            .ok_or(Error::ResourceInput)?;
        validate_package(package, target, state, limits)?;
        files.extend(package.files.iter().cloned());
    }
    let profiles = audit
        .adapters
        .iter()
        .map(|target| {
            (
                target.report.adapter,
                target.report.effective_profile_sha256.clone(),
            )
        })
        .collect();
    Ok(ValidatedRecords {
        code_base_commit: audit.code_base_commit,
        files,
        audit_sha256,
        lock_sha256: audit.lock_sha256,
        sdk_revision: audit.sdk_revision,
        toolchain: audit.toolchain,
        profiles,
    })
}

pub(super) fn validate_audit(bytes: &[u8]) -> Result<Audit, Error> {
    validate_audit_mode(bytes, Mode::Current)
}

pub(super) fn capture_audit(report: super::Report) -> Result<Vec<u8>, Error> {
    let audit = capture::capture(report)?;
    let bytes = toml::to_string_pretty(&audit)
        .map_err(|_error| Error::Output)?
        .into_bytes();
    require(bytes.len() <= super::MAX_BYTES)?;
    Ok(bytes)
}

pub(super) fn summarize_audit(
    bytes: &[u8],
    current_profiles: &BTreeMap<AdapterId, String>,
) -> Result<Vec<u8>, Error> {
    let audit = validate_audit(bytes)?;
    let profiles: BTreeMap<_, _> = audit
        .adapters
        .iter()
        .map(|target| {
            (
                target.report.adapter,
                target.report.effective_profile_sha256.clone(),
            )
        })
        .collect();
    require(profiles == *current_profiles)?;
    Ok(capture::render(&audit)?.into_bytes())
}

fn validate_audit_mode(bytes: &[u8], mode: Mode) -> Result<Audit, Error> {
    let audit: Audit = parse(bytes)?;
    require(
        audit.runtime_facts.probe_config_min_permits > 0
            && audit.runtime_facts.probe_response_permits > 0
            && audit.runtime_facts.gvl_cache_max_entries > 0,
    )?;
    if matches!(mode, Mode::Current) {
        require(audit.runtime_facts == RuntimeFacts::current())?;
    }
    require(
        audit.schema_version == 1
            && hex_id(&audit.code_base_commit, 40)
            && hex_id(&audit.sdk_revision, 40)
            && hex_id(&audit.lock_sha256, 64)
            && concrete(&audit.toolchain)
            && unique_text(&audit.features),
    )?;
    require(audit.adapters.len() == AdapterId::ALL.len())?;
    let mut adapters = BTreeSet::new();
    let first = &audit
        .adapters
        .first()
        .ok_or(Error::ResourceInput)?
        .report
        .identity;
    for target in &audit.adapters {
        let row = &target.report;
        let identity = &row.identity;
        require(
            adapters.insert(row.adapter)
                && identity.code_base_commit == audit.code_base_commit
                && identity.sdk_revision == audit.sdk_revision
                && identity.lock_sha256 == audit.lock_sha256
                && identity.compiler_verbose == audit.toolchain
                && identity.features == audit.features
                && !identity.dirty
                && identity.source_tree_matches_commit
                && hex_id(&identity.source_sha256, 64)
                && identity.source_file_count != 0
                && identity.source_sha256 == first.source_sha256
                && identity.source_file_count == first.source_file_count
                && identity.declared_toolchain == first.declared_toolchain
                && identity.compiled_target == first.compiled_target
                && identity.reporter_abi == first.reporter_abi
                && concrete(&identity.declared_toolchain)
                && concrete(&identity.compiled_target)
                && concrete(&identity.reporter_abi),
        )?;
        validate_snapshot(target, &audit.runtime_facts, mode)?;
        require(row.actual_gvl_cache_entries == audit.runtime_facts.gvl_cache_max_entries)?;
        validate_target(target, &audit.runtime_facts)?;
    }
    require(
        AdapterId::ALL
            .iter()
            .all(|target| adapters.contains(target)),
    )?;
    Ok(audit)
}

fn require(valid: bool) -> Result<(), Error> {
    if valid {
        Ok(())
    } else {
        Err(Error::ResourceInput)
    }
}

fn concrete(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.contains('\0')
        && !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "pending" | "unknown" | "todo" | "tbd" | "n/a"
        )
}

fn hex_id(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unique_text(values: &[String]) -> bool {
    !values.is_empty()
        && values.iter().all(|value| concrete(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn same(left: &impl Serialize, right: &impl Serialize) -> Result<bool, Error> {
    Ok(
        toml::to_string(left).map_err(|_error| Error::ResourceInput)?
            == toml::to_string(right).map_err(|_error| Error::ResourceInput)?,
    )
}

fn validate_snapshot(target: &AdapterAudit, facts: &RuntimeFacts, mode: Mode) -> Result<(), Error> {
    let row = &target.report;
    let derived = &row.derived;
    let terms: BTreeMap<_, _> = derived
        .isolate_terms
        .entries()
        .into_iter()
        .map(|(term, bytes)| (term.name().to_owned(), bytes))
        .collect();
    require(
        derived.auction_charges == row.auction_receipts
            && derived.construction_costs == row.construction_costs
            && terms == row.terms
            && derived
                .isolate_terms
                .checked_total()
                .map_err(|_error| Error::ResourceInput)?
                == derived.total_bytes
            && row
                .auction_receipts
                .total()
                .map_err(|_error| Error::ResourceInput)?
                == derived.per_auction_peak
            && derived.catalog_resident_term == derived.isolate_terms.catalog_resident_bytes
            && derived.catalog_initialization_term
                == derived.isolate_terms.catalog_initialization_bytes
            && derived.gvl_terms == derived.isolate_terms.gvl_bytes
            && derived.egress_term == derived.isolate_terms.auction_egress_bytes
            && derived.aux_term == derived.isolate_terms.auxiliary_bytes
            && derived.terminal_term == derived.isolate_terms.terminal_egress_bytes
            && derived.probe_output_term == derived.isolate_terms.probe_output_bytes
            && derived.pre_admission_parser_term
                == derived.isolate_terms.pre_admission_parser_bytes
            && row.baked_config_bytes > 0
            && hex_id(&row.baked_config_sha256, 64)
            && row.actual_gvl_cache_entries > 0
            && u64::try_from(row.actual_gvl_cache_entries)
                .map_err(|_error| Error::ResourceInput)?
                <= u64::from(row.effective_limits.gvl_cache_entries)
            && super::profile_digest(row)? == row.effective_profile_sha256,
    )?;
    validate_components(target, facts)?;
    validate_stored_validation(row)?;
    match mode {
        Mode::Current => validate_current_snapshot(target),
        Mode::Frozen => Ok(()),
    }
}

fn validate_components(target: &AdapterAudit, facts: &RuntimeFacts) -> Result<(), Error> {
    require(target.component_receipts.len() == Component::ALL.len())?;
    let mut components = Vec::new();
    for receipt in &target.component_receipts {
        require(!components.contains(&receipt.component))?;
        components.push(receipt.component);
    }
    require(
        Component::ALL
            .iter()
            .all(|component| components.contains(component)),
    )?;
    let row = &target.report;
    let expected = component_values(row, target.auxiliary_receipts, facts)?;
    require(
        target
            .component_receipts
            .iter()
            .all(|receipt| expected.contains(&(receipt.component, receipt.bytes))),
    )?;
    require(
        target
            .auxiliary_receipts
            .total()
            .map_err(|_error| Error::ResourceInput)?
            <= row.effective_limits.aux_route_peak_bytes
            && row.derived.isolate_terms.auctions_bytes
                == row
                    .derived
                    .per_auction_peak
                    .checked_mul(u64::from(row.effective_limits.max_concurrent_auctions))
                    .ok_or(Error::ResourceInput)?
            && row.derived.isolate_terms.body_streams_bytes
                == row
                    .derived
                    .per_stream_peak
                    .checked_mul(u64::from(row.effective_limits.max_active_body_streams))
                    .ok_or(Error::ResourceInput)?,
    )
}

fn validate_current_snapshot(target: &AdapterAudit) -> Result<(), Error> {
    let row = &target.report;
    let inputs = resolve_budget_inputs(&row.raw_limits)?;
    let derived = derive_isolate_budget_with_baked_config(
        &row.effective_limits,
        &row.target_constants,
        row.outbound_meta_bytes,
        row.construction_costs,
        row.baked_config_bytes,
        row.catalog_costs,
    )
    .map_err(|_error| Error::ResourceInput)?;
    require(
        inputs.limits == row.effective_limits
            && derived == row.derived
            && inputs.baked_config_bytes == row.baked_config_bytes
            && row.actual_gvl_cache_entries == GVL_PARSE_CACHE_MAX_ENTRIES,
    )?;
    let (_, digest) = super::baked_identity()?;
    require(digest == row.baked_config_sha256)?;
    let profile = row.adapter.profile();
    require(
        row.platform_profile == profile.name
            && row.dispatch_profile == profile.dispatch_name
            && row.memory_scope == format!("{:?}", profile.memory_scope)
            && row.identity.target_abi == profile.target_abi
            && row.identity.layout_evidence == super::layout_evidence(&profile)
            && same(
                &row.platform_facts,
                &super::PlatformFacts::new(row.adapter.platform()),
            )?
            && same(
                &row.dispatch_facts,
                &super::PlatformFacts::new(profile.dispatch_platform),
            )?,
    )?;
    let aux = derive_aux_work_for_profile(
        &row.effective_limits,
        &row.target_constants,
        row.baked_config_bytes,
    )
    .map_err(|_error| Error::ResourceInput)?;
    require(aux.charges() == target.auxiliary_receipts)?;
    if row.adapter.platform().memory_ceiling().value().is_none() {
        require(row.validation.selected_headroom_bytes.is_none())?;
    }
    let mut validation = super::evaluate(
        row.adapter.platform(),
        super::Input {
            total: derived.total_bytes,
            configured: row.effective_limits.isolate_total_bytes,
            class: row.adapter.class(),
            scope: Some(row.adapter.scope()),
            population: row.target_constants.platform_concurrent_ceiling,
            headroom: row.validation.selected_headroom_bytes,
            stack: row.validation.separate_stack_requirement.clone(),
        },
    )?;
    validation.profile_matches_dispatch = profile.dispatch_matches_platform();
    if !validation.profile_matches_dispatch {
        validation
            .external_blockers
            .push("platform_dispatch_mismatch".to_owned());
        validation.required_fit = false;
    }
    require(
        row.required_fit
            == (validation.required_fit
                && row.identity.layout_evidence == super::LayoutEvidence::NativeControl)
            && same(&row.validation, &validation)?,
    )
}

fn validate_stored_validation(row: &AdapterRow) -> Result<(), Error> {
    let validation = &row.validation;
    let required = row
        .derived
        .total_bytes
        .checked_add(validation.selected_headroom_bytes.unwrap_or(0))
        .ok_or(Error::ResourceInput)?;
    require(
        concrete(&validation.sdk_result)
            && matches!(row.memory_scope.as_str(), "PerInstance" | "PerExecution")
            && validation.configured_cap_exceeded
                == (required > row.effective_limits.isolate_total_bytes)
            && validation.gap_bytes == projected_gap(row.derived.total_bytes, row)?,
    )?;
    match &row.platform_facts.ceiling {
        super::Fact::Known { value, source } => {
            require(
                concrete(source)
                    && value.primary_bytes != 0
                    && value.scope == row.memory_scope
                    && validation.effective_ceiling_bytes
                        == Some(
                            value
                                .primary_bytes
                                .min(row.effective_limits.isolate_total_bytes),
                        )
                    && validation
                        .minimum_headroom_bytes
                        .is_some_and(|bytes| bytes > 0)
                    && validation
                        .selected_headroom_bytes
                        .zip(validation.minimum_headroom_bytes)
                        .is_some_and(|(selected, floor)| selected >= floor)
                    && validation.platform_cap_exceeded == (required > value.primary_bytes)
                    && validation.separate_stack_allowance_bytes == value.separate_stack_bytes,
            )?;
        }
        super::Fact::Pending { reason } => require(
            concrete(reason)
                && validation.effective_ceiling_bytes.is_none()
                && validation.minimum_headroom_bytes.is_none()
                && validation.selected_headroom_bytes.is_none()
                && validation.gap_bytes.is_none()
                && !validation.platform_cap_exceeded
                && !validation.required_fit,
        )?,
    }
    require(!row.required_fit || validation.required_fit)?;
    require(
        !validation.required_fit
            || (matches!(validation.sdk_verdict, SdkResult::Fits { .. })
                && validation.gap_bytes == Some(0)
                && !validation.configured_cap_exceeded
                && !validation.platform_cap_exceeded
                && validation.external_blockers.is_empty()
                && validation.population_covered
                && validation.profile_matches_dispatch),
    )
}

impl<T> Evidence<T> {
    fn validate(&self, known: impl FnOnce(&T) -> bool) -> Result<(), Error> {
        require(match self {
            Self::Known { value, source } => concrete(source) && known(value),
            Self::Pending {
                reason,
                owner,
                next_action,
            } => concrete(reason) && concrete(owner) && concrete(next_action),
        })
    }

    const fn is_known(&self) -> bool {
        matches!(self, Self::Known { .. })
    }
}

fn validate_target(target: &AdapterAudit, facts: &RuntimeFacts) -> Result<(), Error> {
    let row = &target.report;
    target.artifact.validate(|artifact| {
        hex_id(&artifact.sha256, 64)
            && artifact.source_sha256 == row.identity.source_sha256
            && artifact.target_abi == row.identity.target_abi
            && artifact.toolchain == row.identity.compiler_verbose
            && row.identity.artifact_proof == artifact.sha256
    })?;
    target
        .abi
        .validate(|bound| abi_bound(bound, target, row.derived.total_bytes))?;
    gap_evidence(&target.gap_bytes, row.derived.total_bytes, row)?;
    let mut seen_facts = BTreeSet::new();
    require(target.pending_facts.len() == row.validation.external_blockers.len())?;
    for pending in &target.pending_facts {
        require(
            row.validation.external_blockers.contains(&pending.fact)
                && seen_facts.insert(&pending.fact)
                && concrete(&pending.reason)
                && concrete(&pending.owner)
                && concrete(&pending.next_action),
        )?;
    }
    require(target.owners.len() == IsolateTerm::ALL.len())?;
    let mut owners = BTreeSet::new();
    for owner in &target.owners {
        let term = owner.term_id.0;
        require(
            owners.insert(term.name())
                && concrete(&owner.allocation_owner)
                && exact_path(&owner.source_path)
                && concrete(&owner.acquire)
                && concrete(&owner.release)
                && concrete(&owner.overlapping_copies),
        )?;
        let population = term_population(term, row, facts)?;
        owner.population.validate(|value| *value == population)?;
        owner.scope.validate(|value| *value == row.memory_scope)?;
        owner.enforcement.validate(|value| concrete(value))?;
        let bytes = *row.terms.get(term.name()).ok_or(Error::ResourceInput)?;
        owner
            .abi_bound
            .validate(|bound| abi_bound(bound, target, bytes))?;
    }
    require(
        IsolateTerm::ALL
            .iter()
            .all(|term| owners.contains(term.name())),
    )?;
    validate_decisions(target)
}

fn abi_bound(bound: &AbiBound, target: &AdapterAudit, bytes: u64) -> bool {
    let row = &target.report;
    let artifact_matches = match &target.artifact {
        Evidence::Known { value, .. } => value.sha256 == bound.artifact_sha256,
        Evidence::Pending { .. } => false,
    };
    artifact_matches
        && bound.upper_bound_bytes <= bytes
        && bound.abi == row.identity.target_abi
        && bound.toolchain == row.identity.compiler_verbose
        && unique_text(&bound.covered_paths)
        && bound.covered_paths.iter().all(|path| exact_path(path))
        && matches!(
            (row.adapter, row.identity.layout_evidence),
            (AdapterId::Axum, super::LayoutEvidence::NativeControl)
        )
}

fn term_population(
    term: IsolateTerm,
    row: &AdapterRow,
    facts: &RuntimeFacts,
) -> Result<u64, Error> {
    let limits = &row.effective_limits;
    Ok(match term {
        IsolateTerm::Baseline
        | IsolateTerm::CatalogResident
        | IsolateTerm::CatalogInitialization => 1,
        IsolateTerm::Gvl => u64::from(limits.gvl_cache_entries)
            .checked_add(u64::from(limits.max_concurrent_auctions))
            .and_then(|value| value.checked_add(u64::from(limits.aux_route_permits)))
            .ok_or(Error::ResourceInput)?,
        IsolateTerm::Auctions => u64::from(limits.max_concurrent_auctions),
        IsolateTerm::AuctionEgress => u64::from(limits.egress_permits),
        IsolateTerm::Auxiliary => u64::from(limits.aux_route_permits)
            .checked_add(u64::from(
                limits.aux_route_permits.max(facts.probe_config_min_permits),
            ))
            .ok_or(Error::ResourceInput)?,
        IsolateTerm::TerminalEgress => u64::from(limits.terminal_response_permits),
        IsolateTerm::ProbeOutput => u64::from(facts.probe_response_permits)
            .checked_add(1)
            .ok_or(Error::ResourceInput)?,
        IsolateTerm::PreAdmissionParser => {
            u64::from(row.target_constants.platform_concurrent_ceiling)
        }
        IsolateTerm::BodyStreams => u64::from(limits.max_active_body_streams),
    })
}

fn aux_population(row: &AdapterRow, facts: &RuntimeFacts) -> Result<u64, Error> {
    u64::from(row.effective_limits.aux_route_permits)
        .checked_add(u64::from(
            row.effective_limits
                .aux_route_permits
                .max(facts.probe_config_min_permits),
        ))
        .ok_or(Error::ResourceInput)
}

fn significant(bytes: u64, total: u64) -> bool {
    bytes > total.div_euclid(20)
}

fn validate_decisions(target: &AdapterAudit) -> Result<(), Error> {
    let row = &target.report;
    let mut subjects: Vec<_> = row
        .derived
        .isolate_terms
        .entries()
        .into_iter()
        .map(|(term, bytes)| {
            (
                Subject::Term {
                    term_id: TermId(term),
                },
                bytes,
            )
        })
        .collect();
    subjects.extend(target.component_receipts.iter().map(|receipt| {
        (
            Subject::Component {
                component: receipt.component,
            },
            receipt.bytes,
        )
    }));
    require(target.decisions.len() <= subjects.len())?;
    let mut seen = Vec::new();
    for decision in &target.decisions {
        require(
            !seen.contains(&&decision.subject)
                && subjects
                    .iter()
                    .any(|(subject, _bytes)| *subject == decision.subject)
                && concrete(&decision.owner)
                && concrete(&decision.rationale)
                && concrete(&decision.next_action),
        )?;
        if let Subject::Term { term_id } = decision.subject {
            require(
                target
                    .owners
                    .iter()
                    .any(|owner| owner.term_id == term_id && owner.disposition == decision.class),
            )?;
        }
        seen.push(&decision.subject);
    }
    require(
        subjects.iter().all(|(subject, bytes)| {
            !significant(*bytes, row.derived.total_bytes) || seen.contains(&subject)
        }) && target.owners.iter().all(|owner| {
            owner.disposition == ChangeClass::Unchanged
                || seen.contains(&&Subject::Term {
                    term_id: owner.term_id,
                })
        }),
    )
}

fn auction_component_values(row: &AdapterRow) -> Result<Vec<(Component, u64)>, Error> {
    use Component::{
        Assembly, AuctionConfig, AuctionIngress, AuctionOutbound, AuctionPrivacy,
        AuctionProjection, AuctionResponse, Bookkeeping, CallSide, Debug, RequestSide,
        RuntimeConfig,
    };
    let AuctionCharges {
        config,
        ingress,
        privacy,
        projection,
        outbound,
        response,
    } = row.auction_receipts;
    let auctions = u64::from(row.effective_limits.max_concurrent_auctions);
    let mut values = Vec::new();
    for (component, value) in [
        (AuctionConfig, config),
        (AuctionIngress, ingress),
        (AuctionPrivacy, privacy),
        (AuctionProjection, projection),
        (AuctionOutbound, outbound),
        (AuctionResponse, response),
        (RuntimeConfig, row.derived.runtime_config_term),
        (RequestSide, row.derived.request_side_term),
        (CallSide, row.derived.call_side_term),
        (Bookkeeping, row.derived.bookkeeping_term),
        (Debug, row.derived.debug_term),
        (Assembly, row.derived.assembly_scratch_bytes),
    ] {
        values.push((
            component,
            value.checked_mul(auctions).ok_or(Error::ResourceInput)?,
        ));
    }
    Ok(values)
}

fn component_values(
    row: &AdapterRow,
    charges: AuxWorkCharges,
    facts: &RuntimeFacts,
) -> Result<Vec<(Component, u64)>, Error> {
    use Component::{
        AuxBody, AuxChooser, AuxConfig, AuxCookie, AuxDeclaredWork, AuxHandoff, AuxInputHeaders,
        AuxNodes, AuxOutput, AuxPrivacy, AuxQuery, AuxRetainedUrls, AuxRoutes, AuxSigning,
        AuxTransmit, AuxUrls, ProbeConfig,
    };
    let mut values = auction_component_values(row)?;
    let AuxWorkCharges {
        config,
        body,
        query,
        input_headers,
        cookie,
        signing,
        nodes,
        privacy,
        chooser,
        urls,
        retained_urls,
        output,
    } = charges;
    let population = aux_population(row, facts)?;
    for (component, value) in [
        (AuxConfig, config),
        (AuxBody, body),
        (AuxQuery, query),
        (AuxInputHeaders, input_headers),
        (AuxCookie, cookie),
        (AuxSigning, signing),
        (AuxNodes, nodes),
        (AuxPrivacy, privacy),
        (AuxChooser, chooser),
        (AuxUrls, urls),
        (AuxRetainedUrls, retained_urls),
        (AuxOutput, output),
        (AuxDeclaredWork, row.effective_limits.aux_route_peak_bytes),
        (
            AuxTransmit,
            u64::from(row.effective_limits.transmit_state_bytes),
        ),
    ] {
        values.push((
            component,
            value.checked_mul(population).ok_or(Error::ResourceInput)?,
        ));
    }
    let work = row.effective_limits.aux_route_peak_bytes;
    let handoff = u64::from(row.target_constants.egress_handoff_factor);
    let per_slot = work
        .checked_mul(handoff)
        .and_then(|value| value.checked_add(u64::from(row.effective_limits.transmit_state_bytes)))
        .ok_or(Error::ResourceInput)?;
    values.push((
        AuxHandoff,
        work.checked_mul(handoff.checked_sub(1).ok_or(Error::ResourceInput)?)
            .and_then(|value| value.checked_mul(population))
            .ok_or(Error::ResourceInput)?,
    ));
    values.push((
        AuxRoutes,
        per_slot
            .checked_mul(u64::from(row.effective_limits.aux_route_permits))
            .ok_or(Error::ResourceInput)?,
    ));
    values.push((
        ProbeConfig,
        per_slot
            .checked_mul(u64::from(
                row.effective_limits
                    .aux_route_permits
                    .max(facts.probe_config_min_permits),
            ))
            .ok_or(Error::ResourceInput)?,
    ));
    Ok(values)
}

fn projected_gap(total: u64, row: &AdapterRow) -> Result<Option<u64>, Error> {
    match (
        row.validation.selected_headroom_bytes,
        row.validation.effective_ceiling_bytes,
    ) {
        (Some(headroom), Some(ceiling)) => Ok(Some(
            total
                .checked_add(headroom)
                .ok_or(Error::ResourceInput)?
                .saturating_sub(ceiling),
        )),
        (None, None) => Ok(None),
        (Some(_) | None, Some(_) | None) => Err(Error::ResourceInput),
    }
}

fn gap_evidence(evidence: &Evidence<u64>, total: u64, row: &AdapterRow) -> Result<(), Error> {
    let gap = projected_gap(total, row)?;
    evidence.validate(|value| gap == Some(*value))?;
    require(evidence.is_known() == gap.is_some())
}

fn validate_package(
    package: &Package,
    target: &AdapterAudit,
    state: &mut IsolateTerms,
    limits: &mut auctioneer_runtime::ResolvedIsolateLimits,
) -> Result<(), Error> {
    require(
        !package.changes.is_empty()
            && !package.files.is_empty()
            && package.files.len() <= 128
            && unique_text(&package.files)
            && package.files.iter().all(|file| exact_path(file))
            && unique_text(&package.test_commands),
    )?;
    let previous_total = state
        .checked_total()
        .map_err(|_error| Error::ResourceInput)?;
    let mut ids = BTreeSet::new();
    for change in &package.changes {
        require(
            concrete(&change.id)
                && ids.insert(&change.id)
                && change.before == *state
                && change.after != change.before
                && !matches!(
                    change.class,
                    ChangeClass::Unchanged | ChangeClass::ExternalPrerequisite
                )
                && concrete(&change.owner)
                && unique_text(&change.functions)
                && concrete(&change.old_lifetime)
                && concrete(&change.new_lifetime)
                && concrete(&change.evidence),
        )?;
        let before = change
            .before
            .checked_total()
            .map_err(|_error| Error::ResourceInput)?;
        let after = change
            .after
            .checked_total()
            .map_err(|_error| Error::ResourceInput)?;
        require(before.checked_sub(after) == Some(change.savings_bytes))?;
        validate_workload(change, target, limits)?;
        *state = change.after;
    }
    let total = state
        .checked_total()
        .map_err(|_error| Error::ResourceInput)?;
    require(
        package.projected_total_bytes == total
            && previous_total.checked_sub(total) == Some(package.savings_bytes),
    )?;
    gap_evidence(&package.residual_gap_bytes, total, &target.report)?;
    match &package.disposition {
        Disposition::Full(()) => {
            // Full is numeric closure, never platform or deployment qualification.
            require(projected_gap(total, &target.report)? == Some(0))?;
        }
        Disposition::Partial {
            unresolved_terms,
            owner,
            next_action,
        } => {
            let mut terms = BTreeSet::new();
            require(
                concrete(owner)
                    && concrete(next_action)
                    && !unresolved_terms.is_empty()
                    && unresolved_terms
                        .iter()
                        .all(|term| terms.insert(term.0.name()))
                    && state.entries().iter().all(|(term, bytes)| {
                        !significant(*bytes, total) || terms.contains(term.name())
                    }),
            )?;
        }
    }
    Ok(())
}

fn validate_workload(
    change: &Change,
    target: &AdapterAudit,
    limits: &mut auctioneer_runtime::ResolvedIsolateLimits,
) -> Result<(), Error> {
    match (&change.workload, change.class) {
        (Some(workload), ChangeClass::WorkloadProfileChange) => {
            require(
                workload.old_limits == *limits
                    && workload.old_limits != workload.new_limits
                    && concrete(&workload.product_impact)
                    && workload.deployment_scope == target.report.platform_profile
                    && workload.new_limits.isolate_total_bytes
                        == workload.old_limits.isolate_total_bytes,
            )?;
            *limits = workload.new_limits;
            Ok(())
        }
        (None, ChangeClass::WorkloadProfileChange) | (Some(_), _) => Err(Error::ResourceInput),
        (None, _) => Ok(()),
    }
}

fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    if bytes.len() > super::MAX_BYTES {
        return Err(Error::ResourceInput);
    }
    toml::from_str(from_utf8(bytes).map_err(|_error| Error::ResourceInput)?)
        .map_err(|_error| Error::ResourceInput)
}

pub(super) fn exact_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path
            .chars()
            .any(|value| value.is_control() || "\\*?[]{}:".contains(value))
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

#[cfg(test)]
pub(super) fn test_pair(code_base_commit: &str) -> (Vec<u8>, Vec<u8>) {
    let mut audit = tests::fixture();
    audit.code_base_commit = code_base_commit.to_owned();
    for target in &mut audit.adapters {
        target.report.identity.code_base_commit = code_base_commit.to_owned();
        target.report.effective_profile_sha256 = super::profile_digest(&target.report).unwrap();
    }
    let plan = tests::plan(&audit);
    (tests::encode(&audit), tests::encode(&plan))
}

#[cfg(test)]
mod tests {
    use super::{
        AbiBound, AdapterAudit, Audit, Change, ChangeClass, Component, Decision, Disposition,
        Evidence, Ownership, Package, PendingFact, Plan, Subject, TermId, exact_path,
        validate_audit, validate_pair,
    };
    use auctioneer_core::host_primitives::resolve_budget_inputs;
    use auctioneer_runtime::{
        CatalogResourceCosts, IsolateTerm, IsolateTerms, derive_isolate_budget_with_baked_config,
    };
    use sha2::{Digest as _, Sha256};
    use std::env::consts::{ARCH, OS};
    use std::fs;
    use std::path::Path;

    #[test]
    fn pre_catalog_snapshot_pins_source_terms_and_unqualified_targets() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml");
        let bytes = fs::read(path).unwrap();
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39",
        );
        // Validate the recorded arithmetic without reinterpreting its ABI or build mode.
        let audit = super::validate_audit_mode(&bytes, super::Mode::Frozen).unwrap();
        assert_eq!(
            audit.code_base_commit,
            "e71e40aba611215bb9305f82e2449e13fb0f41ed"
        );
        assert_eq!(
            audit.sdk_revision,
            "bc300ace5ca1ffe7060594a7bdb01c1da80de305"
        );
        assert_eq!(audit.features, ["application"]);
        let expected = [
            (super::AdapterId::Axum, 2_533_631_672_u64),
            (super::AdapterId::Cloudflare, 138_048_496_272),
            (super::AdapterId::Fastly, 23_589_646_360),
            (super::AdapterId::Spin, 23_589_646_360),
        ];
        assert_eq!(audit.adapters.len(), expected.len());
        for (target, (adapter, total)) in audit.adapters.iter().zip(expected) {
            let row = &target.report;
            assert_eq!(row.adapter, adapter);
            assert_eq!(row.derived.total_bytes, total);
            assert_eq!(row.catalog_costs, CatalogResourceCosts::UNCHANGED);
            assert_eq!(row.baked_config_bytes, 1584);
            assert!(!row.identity.dirty);
            assert!(row.identity.source_tree_matches_commit);
            assert!(!row.required_fit);
            assert!(!row.validation.external_blockers.is_empty());
        }
    }

    #[test]
    fn catalog_comparison_loads_only_the_frozen_reviewed_baseline_bytes() {
        let bytes = include_bytes!(
            "../../../../../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml"
        );
        let baseline = super::catalog_baseline(bytes).unwrap();
        assert!(baseline.continuity.is_none());
        assert_eq!(
            baseline.code_base_commit,
            "e71e40aba611215bb9305f82e2449e13fb0f41ed"
        );
        assert_eq!(
            baseline.sha256,
            "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39"
        );
        assert_eq!(
            baseline
                .adapters
                .iter()
                .map(|row| row.adapter)
                .collect::<Vec<_>>(),
            [
                super::AdapterId::Axum,
                super::AdapterId::Cloudflare,
                super::AdapterId::Fastly,
                super::AdapterId::Spin
            ]
        );
        assert!(
            baseline
                .adapters
                .iter()
                .all(|row| row.catalog_costs == CatalogResourceCosts::UNCHANGED)
        );

        // Valid TOML with identical semantic content must still refuse a raw
        // identity change, not merely survive an earlier parsing failure.
        let mut changed = bytes.to_vec();
        changed.extend_from_slice(b"\n# changed baseline bytes\n");
        assert!(matches!(
            super::catalog_baseline(&changed),
            Err(super::Error::ResourceInput)
        ));
    }

    #[test]
    fn patched_lock_catalog_baseline_retains_origin_and_chooser_growth() {
        let bytes = include_bytes!(
            "../../../../../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline-patched-lock.toml"
        );
        assert_eq!(
            hex::encode(Sha256::digest(bytes)),
            "1d20dfa95f09a32a45f106f5503094efd71a718d794814fac353f83f357219d0"
        );
        let baseline = super::catalog_baseline(bytes).unwrap();
        assert_eq!(
            baseline.code_base_commit,
            "7286bedfd1f1999edae9a5e3e5665b1f95117541"
        );
        assert_eq!(
            baseline.sha256,
            "1d20dfa95f09a32a45f106f5503094efd71a718d794814fac353f83f357219d0"
        );
        let continuity = serde_json::to_value(baseline.continuity.unwrap()).unwrap();
        assert_eq!(
            continuity["original_code_base_commit"],
            "e71e40aba611215bb9305f82e2449e13fb0f41ed"
        );
        assert_eq!(
            continuity["original_sha256"],
            "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39"
        );
        let rows = continuity["adapters"].as_array().unwrap();
        assert_eq!(rows.len(), 4);
        for (row, (name, total, scaled)) in rows.iter().zip([
            ("axum", 2_533_631_672_u64, 0x0002_0000_u64),
            ("cloudflare", 138_048_496_272, 0x0008_0000),
            ("fastly", 23_589_646_360, 0x0002_0000),
            ("spin", 23_589_646_360, 0x0002_0000),
        ]) {
            assert_eq!(row["adapter"], name);
            assert_eq!(row["original_total_bytes"], total);
            assert_eq!(row["rebased_total_bytes"], total);
            assert_eq!(
                row["auxiliary_chooser_per_permit_delta_bytes"],
                0x0001_0000_u64
            );
            assert_eq!(row["auxiliary_chooser_component_delta_bytes"], scaled);
        }
        let mut changed = bytes.to_vec();
        changed.extend_from_slice(b"\n# same semantics, different raw identity\n");
        assert!(matches!(
            super::catalog_baseline(&changed),
            Err(super::Error::ResourceInput)
        ));
    }

    #[test]
    fn catalog_baseline_projection_refuses_each_changed_runtime_fact() {
        let bytes = include_bytes!(
            "../../../../../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml"
        );
        for field in [
            "probe_config_min_permits",
            "probe_response_permits",
            "gvl_cache_max_entries",
        ] {
            let mut value: toml::Value = toml::from_str(super::from_utf8(bytes).unwrap()).unwrap();
            let selected = &mut value["runtime_facts"][field];
            let original = selected.as_integer().unwrap();
            *selected = toml::Value::Integer(original.checked_add(1).unwrap());
            let audit = value.try_into().unwrap();
            assert!(matches!(
                super::catalog_baseline_projection(audit, "synthetic-input".to_owned()),
                Err(super::Error::ResourceInput)
            ));
        }
    }

    pub(super) fn encode(value: &impl serde::Serialize) -> Vec<u8> {
        toml::to_string(value).unwrap().into_bytes()
    }

    fn pending<T>() -> Evidence<T> {
        Evidence::Pending {
            reason: "Synthetic evidence not observed".to_owned(),
            owner: "Fixture maintainer".to_owned(),
            next_action: "Obtain independent scope-bound evidence".to_owned(),
        }
    }

    fn fixture_identity() -> super::super::Identity {
        let built: super::super::resource_identity::BuildIdentity = serde_json::from_slice(
            include_bytes!(concat!(env!("OUT_DIR"), "/resource-identity.json")),
        )
        .unwrap();
        let manifest: toml::Value =
            toml::from_str(include_str!("../../../../../../Cargo.toml")).unwrap();
        let mut features = vec!["application".to_owned()];
        features.extend(
            auctioneer_adapter_axum::build_features()
                .into_iter()
                .map(str::to_owned),
        );
        super::super::Identity {
            code_base_commit: built.code_base_commit,
            dirty: false,
            source_tree_matches_commit: true,
            sdk_revision: manifest
                .get("workspace")
                .unwrap()
                .get("dependencies")
                .unwrap()
                .get("edgezero-core")
                .unwrap()
                .get("rev")
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned(),
            lock_sha256: built.snapshot.files["Cargo.lock"].as_ref().unwrap().clone(),
            declared_toolchain: include_str!("../../../../../../.tool-versions").to_owned(),
            features,
            source_sha256: built.snapshot.sha256,
            source_file_count: built.snapshot.files.len(),
            compiler_verbose: built.compiler_verbose,
            compiled_target: built.compiled_target,
            reporter_abi: format!("{}-{}-{}bit", ARCH, OS, usize::BITS),
            target_abi: "pending".to_owned(),
            layout_evidence: super::super::LayoutEvidence::HostNativeSubstitute,
            artifact_proof: "pending_not_a_deployment_observation".to_owned(),
        }
    }

    fn fixture_target(report: super::AdapterRow) -> AdapterAudit {
        let auxiliary_receipts = super::derive_aux_work_for_profile(
            &report.effective_limits,
            &report.target_constants,
            report.baked_config_bytes,
        )
        .unwrap()
        .charges();
        let mut decisions: Vec<_> = IsolateTerm::ALL
            .iter()
            .map(|term| Decision {
                subject: Subject::Term {
                    term_id: TermId(*term),
                },
                class: ChangeClass::Unchanged,
                owner: "Fixture maintainer".to_owned(),
                rationale: "Retain the existing charged bound".to_owned(),
                next_action: "Review independent allocation evidence".to_owned(),
            })
            .collect();
        for component in Component::ALL.iter().copied() {
            decisions.push(Decision {
                subject: Subject::Component { component },
                class: ChangeClass::ExternalPrerequisite,
                owner: "Fixture maintainer".to_owned(),
                rationale: "Retain charge until independently bounded".to_owned(),
                next_action: "Inspect the owning lifetime and population".to_owned(),
            });
        }
        AdapterAudit {
            auxiliary_receipts,
            component_receipts: super::component_values(
                &report,
                auxiliary_receipts,
                &super::RuntimeFacts::current(),
            )
            .unwrap()
            .into_iter()
            .map(|(component, bytes)| super::ComponentReceipt { component, bytes })
            .collect(),
            owners: IsolateTerm::ALL
                .iter()
                .map(|term| Ownership {
                    term_id: TermId(*term),
                    allocation_owner: "Fixture allocation owner".to_owned(),
                    source_path: "crates/auctioneer-runtime/src/budget.rs".to_owned(),
                    acquire: "Fixture acquisition before work".to_owned(),
                    release: "Fixture completion and cancellation".to_owned(),
                    population: pending(),
                    scope: Evidence::Known {
                        value: report.memory_scope.clone(),
                        source: "Selected SDK memory scope".to_owned(),
                    },
                    enforcement: pending(),
                    overlapping_copies: "Every overlapping copy remains funded".to_owned(),
                    abi_bound: pending::<AbiBound>(),
                    disposition: ChangeClass::Unchanged,
                })
                .collect(),
            pending_facts: report
                .validation
                .external_blockers
                .iter()
                .map(|fact| PendingFact {
                    fact: fact.clone(),
                    reason: "Published SDK fact is unresolved".to_owned(),
                    owner: "Fixture platform maintainer".to_owned(),
                    next_action: "Publish an enforced scope-bound fact".to_owned(),
                })
                .collect(),
            gap_bytes: report
                .validation
                .gap_bytes
                .map_or_else(pending, |value| Evidence::Known {
                    value,
                    source: "Shared resource validation".to_owned(),
                }),
            report,
            decisions,
            artifact: pending(),
            abi: pending(),
        }
    }

    pub(super) fn fixture() -> Audit {
        let identity = fixture_identity();
        let (_, digest) = super::super::baked_identity().unwrap();
        let adapters = super::AdapterId::ALL
            .into_iter()
            .map(|target| {
                fixture_target(super::super::source_row(target, &identity, &digest).unwrap())
            })
            .collect();
        Audit {
            schema_version: 1,
            code_base_commit: identity.code_base_commit,
            sdk_revision: identity.sdk_revision,
            lock_sha256: identity.lock_sha256,
            toolchain: identity.compiler_verbose,
            features: identity.features,
            runtime_facts: super::RuntimeFacts::current(),
            adapters,
        }
    }

    pub(super) fn plan(audit: &Audit) -> Plan {
        let row = &audit.adapters[1].report;
        let before = row.derived.isolate_terms;
        let mut after = before;
        after.baseline_bytes = after.baseline_bytes.checked_sub(1).unwrap();
        let total = after.checked_total().unwrap();
        let residual = total
            .checked_add(row.validation.selected_headroom_bytes.unwrap())
            .unwrap()
            .saturating_sub(row.validation.effective_ceiling_bytes.unwrap());
        Plan {
            schema_version: 1,
            code_base_commit: audit.code_base_commit.clone(),
            audit_sha256: hex::encode(Sha256::digest(encode(audit))),
            packages: vec![Package {
                id: "synthetic-only".to_owned(),
                adapter: row.adapter,
                changes: vec![Change {
                    id: "synthetic-change".to_owned(),
                    class: ChangeClass::ImplementationImprovement,
                    owner: "Fixture maintainer".to_owned(),
                    functions: vec!["fixture::release".to_owned()],
                    old_lifetime: "Synthetic overlapping lifetime".to_owned(),
                    new_lifetime: "Synthetic earlier release".to_owned(),
                    evidence: "Synthetic arithmetic control, not a proposal".to_owned(),
                    before,
                    after,
                    savings_bytes: 1,
                    workload: None,
                }],
                projected_total_bytes: total,
                savings_bytes: 1,
                residual_gap_bytes: Evidence::Known {
                    value: residual,
                    source: "Checked projected arithmetic".to_owned(),
                },
                files: vec!["crates/auctioneer-runtime/src/budget.rs".to_owned()],
                test_commands: vec!["cargo test --locked -p auctioneer-runtime --lib".to_owned()],
                disposition: Disposition::Partial {
                    unresolved_terms: IsolateTerm::ALL.iter().copied().map(TermId).collect(),
                    owner: "Fixture maintainer".to_owned(),
                    next_action: "Obtain independent proof and product decision".to_owned(),
                },
            }],
        }
    }

    fn rejects_audit(change: impl FnOnce(&mut Audit)) {
        let mut audit = fixture();
        change(&mut audit);
        validate_audit(&encode(&audit)).unwrap_err();
    }

    fn rejects_plan(change: impl FnOnce(&mut Plan)) {
        let audit = fixture();
        let mut plan = plan(&audit);
        change(&mut plan);
        validate_pair(&encode(&audit), &encode(&plan)).unwrap_err();
    }

    #[test]
    fn complete_pending_audit_and_exact_partial_projection_are_accepted() {
        let audit = fixture();
        let bytes = encode(&audit);
        assert!(bytes.len() < 1 << 20_usize);
        validate_audit(&bytes).unwrap();
        let records = validate_pair(&bytes, &encode(&plan(&audit))).unwrap();
        assert_eq!(records.code_base_commit, audit.code_base_commit);
        assert_eq!(records.audit_sha256, hex::encode(Sha256::digest(&bytes)));
        assert_eq!(records.lock_sha256, audit.lock_sha256);
        assert_eq!(records.sdk_revision, audit.sdk_revision);
        assert_eq!(records.toolchain, audit.toolchain);
        assert_eq!(
            records.files.into_iter().collect::<Vec<_>>(),
            ["crates/auctioneer-runtime/src/budget.rs"]
        );
    }

    #[test]
    fn targets_terms_and_owners_include_zero_rows_exactly_once() {
        rejects_audit(|audit| {
            audit.adapters.pop();
        });
        rejects_audit(|audit| {
            audit.adapters[1].report.adapter = super::AdapterId::Axum;
        });
        rejects_audit(|audit| {
            audit.adapters.first_mut().unwrap().owners.pop();
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .owners
                .first_mut()
                .unwrap()
                .term_id = TermId(IsolateTerm::Gvl);
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .terms
                .remove("catalog_resident");
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .terms
                .insert("unknown".to_owned(), 0);
        });
    }

    #[test]
    fn numeric_and_platform_snapshot_fields_cannot_be_forged() {
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .raw_limits
                .baseline_bytes += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .effective_limits
                .baseline_bytes += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .construction_costs
                .reusable_scratch_bytes += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .catalog_costs
                .resident_bytes += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .baked_config_bytes += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .auction_receipts
                .response += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .actual_gvl_cache_entries += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .outbound_meta_bytes += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .derived
                .total_bytes += 1;
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .validation
                .required_fit = true;
        });
        rejects_audit(|audit| {
            audit.adapters[2]
                .report
                .validation
                .separate_stack_allowance_bytes = Some(999_999);
        });
        rejects_audit(|audit| {
            audit.adapters[1].report.dispatch_profile = "different".to_owned();
        });
    }

    #[test]
    fn identities_are_bound_across_header_and_targets() {
        rejects_audit(|audit| audit.code_base_commit = "b".repeat(40));
        rejects_audit(|audit| audit.sdk_revision = "b".repeat(40));
        rejects_audit(|audit| audit.lock_sha256 = "b".repeat(64));
        rejects_audit(|audit| audit.features.push("extra".to_owned()));
        rejects_audit(|audit| audit.toolchain.push_str("stale"));
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .report
                .identity
                .source_sha256 = "bad".to_owned();
        });
        rejects_audit(|audit| audit.adapters.first_mut().unwrap().report.identity.dirty = true);
    }

    #[test]
    fn evidence_requires_concrete_owner_lifetimes_and_pending_actions() {
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .owners
                .first_mut()
                .unwrap()
                .acquire
                .clear();
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .owners
                .first_mut()
                .unwrap()
                .source_path = "crates/**".to_owned();
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .owners
                .first_mut()
                .unwrap()
                .abi_bound = Evidence::Pending {
                reason: "Unobserved".to_owned(),
                owner: String::new(),
                next_action: "Measure guest layout".to_owned(),
            };
        });
        rejects_audit(|audit| {
            audit
                .adapters
                .first_mut()
                .unwrap()
                .owners
                .first_mut()
                .unwrap()
                .population = Evidence::Known {
                value: 999,
                source: "Invented population".to_owned(),
            };
        });
        rejects_audit(|audit| {
            audit.adapters.first_mut().unwrap().pending_facts.pop();
        });
        rejects_audit(|audit| {
            audit.adapters[1].gap_bytes = Evidence::Known {
                value: 0,
                source: "Invented closed gap".to_owned(),
            };
        });
        rejects_audit(|audit| {
            audit.adapters.first_mut().unwrap().gap_bytes = Evidence::Known {
                value: 0,
                source: "Unknown is not zero".to_owned(),
            };
        });
    }

    #[test]
    fn dominant_terms_and_subcomponents_require_unique_decisions() {
        rejects_audit(|audit| {
            audit.adapters[1].decisions.retain(|decision| {
                decision.subject
                    != Subject::Term {
                        term_id: TermId(IsolateTerm::Auctions),
                    }
            });
        });
        rejects_audit(|audit| {
            audit.adapters[1].decisions.retain(|decision| {
                decision.subject
                    != Subject::Component {
                        component: Component::AuctionProjection,
                    }
            });
        });
        rejects_audit(|audit| {
            audit.adapters[1].decisions[1].subject = Subject::Term {
                term_id: TermId(IsolateTerm::Baseline),
            };
        });
    }

    #[test]
    fn strict_toml_rejects_unknown_duplicate_missing_oversized_and_non_utf8() {
        let audit = encode(&fixture());
        let text = String::from_utf8(audit.clone()).unwrap();
        for candidate in [
            format!("unknown = 1\n{text}"),
            format!("schema_version = 1\n{text}"),
            text.replacen("schema_version = 1\n", "", 1),
            text.replacen("config = ", "unknown_receipt = ", 1),
            text.replacen("catalog_initialization = 0\n", "", 1),
        ] {
            validate_audit(candidate.as_bytes()).unwrap_err();
        }
        validate_audit(&vec![b' '; (1 << 20_usize) + 1]).unwrap_err();
        validate_audit(&[255]).unwrap_err();
        validate_pair(&audit, &vec![b' '; (1 << 20_usize) + 1]).unwrap_err();
    }

    #[test]
    fn plans_bind_digest_identity_and_nonempty_unique_exact_scope() {
        rejects_plan(|plan| plan.schema_version = 2);
        rejects_plan(|plan| plan.audit_sha256 = "f".repeat(64));
        rejects_plan(|plan| plan.code_base_commit = "f".repeat(40));
        rejects_plan(|plan| plan.packages.clear());
        rejects_plan(|plan| plan.packages.first_mut().unwrap().files.clear());
        rejects_plan(|plan| {
            plan.packages.first_mut().unwrap().files = vec!["Cargo.toml".to_owned(); 2];
        });
        rejects_plan(|plan| {
            plan.packages.first_mut().unwrap().files = (0_u32..129)
                .map(|index| format!("file{index}.rs"))
                .collect();
        });
        rejects_plan(|plan| plan.packages.first_mut().unwrap().files = vec!["crates/*".to_owned()]);
        rejects_plan(|plan| plan.packages.first_mut().unwrap().test_commands.clear());
    }

    #[test]
    fn ordered_changes_cannot_reuse_original_savings_or_forge_totals() {
        rejects_plan(|plan| {
            let package = plan.packages.first_mut().unwrap();
            let copy: Change = toml::from_str(
                &String::from_utf8(encode(package.changes.first().unwrap())).unwrap(),
            )
            .unwrap();
            package.changes.push(copy);
            package.changes.last_mut().unwrap().id = "overlap".to_owned();
        });
        rejects_plan(|plan| plan.packages.first_mut().unwrap().projected_total_bytes -= 1);
        rejects_plan(|plan| plan.packages.first_mut().unwrap().savings_bytes += 1);
        rejects_plan(|plan| {
            plan.packages
                .first_mut()
                .unwrap()
                .changes
                .first_mut()
                .unwrap()
                .savings_bytes += 1;
        });
        rejects_plan(|plan| {
            plan.packages
                .first_mut()
                .unwrap()
                .changes
                .first_mut()
                .unwrap()
                .before
                .auctions_bytes -= 1;
        });
        rejects_plan(|plan| {
            plan.packages.first_mut().unwrap().residual_gap_bytes = Evidence::Known {
                value: 0,
                source: "Unjustified residual".to_owned(),
            };
        });
    }

    #[test]
    fn full_packages_cannot_hide_gap_and_partial_requires_residual_owner() {
        rejects_plan(|plan| plan.packages.first_mut().unwrap().disposition = Disposition::Full(()));
        rejects_plan(|plan| {
            plan.packages.first_mut().unwrap().disposition = Disposition::Partial {
                unresolved_terms: Vec::new(),
                owner: "Fixture owner".to_owned(),
                next_action: "Review residual".to_owned(),
            };
        });
        rejects_plan(|plan| {
            plan.packages.first_mut().unwrap().disposition = Disposition::Partial {
                unresolved_terms: IsolateTerm::ALL.iter().copied().map(TermId).collect(),
                owner: String::new(),
                next_action: "Review residual".to_owned(),
            };
        });
    }

    #[test]
    fn full_numeric_closure_preserves_external_pending_facts() {
        let audit = fixture();
        let mut plan = plan(&audit);
        let package = plan.packages.first_mut().unwrap();
        package.changes.first_mut().unwrap().after = IsolateTerms::default();
        package.changes.first_mut().unwrap().savings_bytes = package
            .changes
            .first()
            .unwrap()
            .before
            .checked_total()
            .unwrap();
        package.savings_bytes = package.changes.first().unwrap().savings_bytes;
        package.projected_total_bytes = 0;
        package.residual_gap_bytes = Evidence::Known {
            value: 0,
            source: "Synthetic numeric closure, no platform proof".to_owned(),
        };
        package.disposition = Disposition::Full(());
        validate_pair(&encode(&audit), &encode(&plan)).unwrap();
        assert!(!audit.adapters[1].report.required_fit);
        assert!(!audit.adapters[1].pending_facts.is_empty());
    }

    #[test]
    fn independent_allocation_bound_must_fit_funded_allowance() {
        let mut audit = fixture();
        let target = audit.adapters.first_mut().unwrap();
        target.artifact = Evidence::Known {
            value: super::Artifact {
                sha256: "a".repeat(64),
                source_sha256: target.report.identity.source_sha256.clone(),
                target_abi: target.report.identity.target_abi.clone(),
                toolchain: target.report.identity.compiler_verbose.clone(),
            },
            source: "Synthetic independent artifact".to_owned(),
        };
        let bound = AbiBound {
            upper_bound_bytes: 100,
            artifact_sha256: "a".repeat(64),
            abi: target.report.identity.target_abi.clone(),
            toolchain: target.report.identity.compiler_verbose.clone(),
            covered_paths: vec!["crates/auctioneer-runtime/src/budget.rs".to_owned()],
        };
        assert!(super::abi_bound(&bound, target, 100));
        assert!(super::abi_bound(&bound, target, 101));
        assert!(!super::abi_bound(&bound, target, 99));
    }

    #[test]
    fn workload_changes_require_explicit_caps_and_product_scope() {
        rejects_plan(|plan| {
            plan.packages
                .first_mut()
                .unwrap()
                .changes
                .first_mut()
                .unwrap()
                .class = ChangeClass::WorkloadProfileChange;
        });
    }

    #[test]
    fn workload_changes_record_numeric_caps_and_keep_impact_and_scope() {
        let audit = fixture();
        let row = &audit.adapters[1].report;
        let mut plan = plan(&audit);
        let package = plan.packages.first_mut().unwrap();
        let change = package.changes.first_mut().unwrap();
        let old_limits = row.raw_limits;
        let mut new_limits = old_limits;
        new_limits.max_concurrent_auctions -= 1;
        let effective = resolve_budget_inputs(&new_limits).unwrap();
        let derived = derive_isolate_budget_with_baked_config(
            &effective.limits,
            &row.target_constants,
            row.outbound_meta_bytes,
            row.construction_costs,
            row.baked_config_bytes,
            row.catalog_costs,
        )
        .unwrap();
        change.class = ChangeClass::WorkloadProfileChange;
        change.after = derived.isolate_terms;
        change.savings_bytes = row.derived.total_bytes - derived.total_bytes;
        change.workload = Some(super::WorkloadChange {
            old_limits,
            new_limits,
            product_impact: "Synthetic lower auction concurrency rejects excess parallel requests"
                .to_owned(),
            deployment_scope: row.platform_profile.clone(),
        });
        package.projected_total_bytes = derived.total_bytes;
        package.savings_bytes = row.derived.total_bytes - derived.total_bytes;
        package.residual_gap_bytes = Evidence::Known {
            value: super::projected_gap(derived.total_bytes, row)
                .unwrap()
                .unwrap(),
            source: "Checked synthetic cap projection".to_owned(),
        };
        validate_pair(&encode(&audit), &encode(&plan)).unwrap();
        super::validate_frozen_pair(&encode(&audit), &encode(&plan)).unwrap();
        plan.packages
            .first_mut()
            .unwrap()
            .changes
            .first_mut()
            .unwrap()
            .workload
            .as_mut()
            .unwrap()
            .product_impact
            .clear();
        validate_pair(&encode(&audit), &encode(&plan)).unwrap_err();
    }

    #[test]
    fn ordered_changes_and_packages_replay_the_previous_state_once() {
        let audit = fixture();
        let mut plan = plan(&audit);
        let package = plan.packages.first_mut().unwrap();
        let mut second: Change =
            toml::from_str(&String::from_utf8(encode(package.changes.first().unwrap())).unwrap())
                .unwrap();
        second.id = "second-change".to_owned();
        second.before = second.after;
        second.after.baseline_bytes = second.after.baseline_bytes.checked_sub(1).unwrap();
        package.changes.push(second);
        package.projected_total_bytes -= 1;
        package.savings_bytes += 1;
        if let Evidence::Known { value, .. } = &mut package.residual_gap_bytes {
            *value -= 1;
        }
        validate_pair(&encode(&audit), &encode(&plan)).unwrap();
        let mut next: Package =
            toml::from_str(&String::from_utf8(encode(plan.packages.first().unwrap())).unwrap())
                .unwrap();
        next.id = "second-package".to_owned();
        validate_pair(&encode(&audit), &encode(&plan)).unwrap();
        plan.packages.push(next);
        validate_pair(&encode(&audit), &encode(&plan)).unwrap_err();
    }

    #[test]
    fn each_small_term_is_explicitly_unchanged_or_has_matching_decision() {
        let mut audit = fixture();
        let target = &mut audit.adapters[1];
        target.decisions.retain(|decision| {
            decision.subject
                != Subject::Term {
                    term_id: TermId(IsolateTerm::CatalogResident),
                }
        });
        validate_audit(&encode(&audit)).unwrap();
        audit.adapters[1]
            .owners
            .iter_mut()
            .find(|owner| owner.term_id == TermId(IsolateTerm::CatalogResident))
            .unwrap()
            .disposition = ChangeClass::AccountingCorrection;
        validate_audit(&encode(&audit)).unwrap_err();
        rejects_audit(|other| {
            other.adapters[1].decisions.retain(|decision| {
                decision.subject
                    != Subject::Component {
                        component: Component::AuxHandoff,
                    }
            });
        });
    }

    #[test]
    fn exact_scope_accepts_128_paths_but_not_duplicate_packages() {
        let audit = fixture();
        let mut plan = plan(&audit);
        plan.packages.first_mut().unwrap().files = (0_u32..128)
            .map(|index| format!("files/file{index}.rs"))
            .collect();
        assert_eq!(
            validate_pair(&encode(&audit), &encode(&plan))
                .unwrap()
                .files
                .len(),
            128
        );
        let copy: Package =
            toml::from_str(&String::from_utf8(encode(plan.packages.first().unwrap())).unwrap())
                .unwrap();
        plan.packages.push(copy);
        validate_pair(&encode(&audit), &encode(&plan)).unwrap_err();
    }

    #[test]
    fn exact_five_percent_threshold_uses_isolate_total_without_rounding() {
        assert!(!super::significant(10, 200));
        assert!(super::significant(11, 200));
        assert!(!super::significant(10, 219));
        assert!(super::significant(11, 219));
        assert!(!super::significant(922_337_203_685_477_580, u64::MAX));
        assert!(super::significant(922_337_203_685_477_581, u64::MAX));
        assert!(!super::significant(0, 0));
        assert!(super::significant(1, 0));
    }

    #[test]
    fn frozen_review_does_not_rederive_old_coefficients_with_current_code() {
        let mut audit = fixture();
        let target = &mut audit.adapters[1];
        let population = u64::from(target.report.effective_limits.max_concurrent_auctions);
        target.report.auction_receipts.projection += 1;
        target.report.derived.auction_charges.projection += 1;
        target.report.derived.per_auction_peak += 1;
        target.report.derived.request_side_term += 1;
        target.report.derived.isolate_terms.auctions_bytes += population;
        target.report.derived.total_bytes += population;
        target.report.terms.insert(
            "auctions".to_owned(),
            target.report.derived.isolate_terms.auctions_bytes,
        );
        for receipt in &mut target.component_receipts {
            if matches!(
                receipt.component,
                Component::AuctionProjection | Component::RequestSide
            ) {
                receipt.bytes += population;
            }
        }
        target.report.validation.gap_bytes = target
            .report
            .validation
            .gap_bytes
            .map(|value| value + population);
        if let super::super::super::resources_platform::SdkResult::Exceeds {
            required_bytes, ..
        } = &mut target.report.validation.sdk_verdict
        {
            *required_bytes += population;
        }
        // The frozen SDK text is evidence from B; it is not recreated by H.
        target.report.validation.sdk_result = "Synthetic captured B validation".to_owned();
        target.gap_bytes = Evidence::Known {
            value: target.report.validation.gap_bytes.unwrap(),
            source: "Frozen B arithmetic".to_owned(),
        };
        target.report.effective_profile_sha256 =
            super::super::profile_digest(&target.report).unwrap();
        let bytes = encode(&audit);
        validate_audit(&bytes).unwrap_err();
        super::validate_frozen_pair(&bytes, &encode(&plan(&audit))).unwrap();
    }

    #[test]
    fn checked_projection_refuses_overflow() {
        let audit = fixture();
        let row = &audit.adapters[1].report;
        super::projected_gap(u64::MAX, row).unwrap_err();
    }

    #[test]
    fn byte_limits_are_exact_for_otherwise_valid_records() {
        let audit = fixture();
        let mut audit_bytes = encode(&audit);
        audit_bytes.resize(0x0010_0000, b' ');
        let mut plan = plan(&audit);
        plan.audit_sha256 = hex::encode(Sha256::digest(&audit_bytes));
        let mut plan_bytes = encode(&plan);
        plan_bytes.resize(0x0010_0000, b' ');
        validate_pair(&audit_bytes, &plan_bytes).unwrap();
        plan_bytes.push(b' ');
        validate_pair(&audit_bytes, &plan_bytes).unwrap_err();
        audit_bytes.push(b' ');
        plan.audit_sha256 = hex::encode(Sha256::digest(&audit_bytes));
        validate_pair(&audit_bytes, &encode(&plan)).unwrap_err();
    }

    #[test]
    fn frozen_components_cannot_hide_a_dominant_projection_receipt() {
        let mut audit = fixture();
        let target = &mut audit.adapters[1];
        target
            .component_receipts
            .iter_mut()
            .find(|receipt| receipt.component == Component::AuctionProjection)
            .unwrap()
            .bytes = 0;
        target.decisions.retain(|decision| {
            decision.subject
                != Subject::Component {
                    component: Component::AuctionProjection,
                }
        });
        super::validate_frozen_pair(&encode(&audit), &encode(&plan(&audit))).unwrap_err();
    }

    #[test]
    fn frozen_fit_cannot_contradict_gap_blockers_and_sdk_verdict() {
        let mut audit = fixture();
        let target = &mut audit.adapters[1];
        target.report.required_fit = true;
        target.report.validation.required_fit = true;
        target.report.effective_profile_sha256 =
            super::super::profile_digest(&target.report).unwrap();
        super::validate_frozen_pair(&encode(&audit), &encode(&plan(&audit))).unwrap_err();
    }

    #[test]
    fn mixed_class_packages_replay_caps_without_using_old_projection_coefficients() {
        let audit = fixture();
        let row = &audit.adapters[1].report;
        let mut plan = plan(&audit);
        let mut next: Package =
            toml::from_str(&String::from_utf8(encode(plan.packages.first().unwrap())).unwrap())
                .unwrap();
        next.id = "mixed-next".to_owned();
        let change = next.changes.first_mut().unwrap();
        change.id = "new-cap-after-ownership-change".to_owned();
        change.class = ChangeClass::WorkloadProfileChange;
        change.before = change.after;
        change.after.auctions_bytes -= 1;
        let mut new_limits = row.raw_limits;
        new_limits.max_concurrent_auctions -= 1;
        change.workload = Some(super::WorkloadChange {
            old_limits: row.raw_limits,
            new_limits,
            product_impact: "Synthetic lower concurrency after an ownership improvement".to_owned(),
            deployment_scope: row.platform_profile.clone(),
        });
        next.projected_total_bytes -= 1;
        if let Evidence::Known { value, .. } = &mut next.residual_gap_bytes {
            *value -= 1;
        }
        plan.packages.push(next);
        validate_pair(&encode(&audit), &encode(&plan)).unwrap();
        super::validate_frozen_pair(&encode(&audit), &encode(&plan)).unwrap();
    }

    #[test]
    fn frozen_owner_population_uses_frozen_probe_facts() {
        let mut audit = fixture();
        audit.runtime_facts.probe_response_permits += 1;
        let old_population = u64::from(audit.runtime_facts.probe_response_permits) + 1;
        audit.adapters[1]
            .owners
            .iter_mut()
            .find(|owner| owner.term_id == TermId(IsolateTerm::ProbeOutput))
            .unwrap()
            .population = Evidence::Known {
            value: old_population,
            source: "Synthetic independently frozen probe population".to_owned(),
        };
        validate_audit(&encode(&audit)).unwrap_err();
        super::validate_frozen_pair(&encode(&audit), &encode(&plan(&audit))).unwrap();
        audit.adapters[1]
            .owners
            .iter_mut()
            .find(|owner| owner.term_id == TermId(IsolateTerm::ProbeOutput))
            .unwrap()
            .population = Evidence::Known {
            value: old_population + 1,
            source: "Contradictory frozen owner population".to_owned(),
        };
        super::validate_frozen_pair(&encode(&audit), &encode(&plan(&audit))).unwrap_err();
    }

    #[test]
    fn exact_paths_accept_files_and_reject_ambiguous_scope() {
        for path in [
            "Cargo.toml",
            "crates/auctioneer-runtime/src/budget.rs",
            ".github/workflows/test.yml",
        ] {
            assert!(exact_path(path), "{path}");
        }
        for path in [
            "", "/tmp/a", "a/../b", "./a", "a//b", "a/", "a\\b", "a\nb", "a*", "a?", "a[0]",
            "a{b}", ".", "..",
        ] {
            assert!(!exact_path(path), "{path:?}");
        }
    }

    #[test]
    fn unit_dispositions_and_sdk_states_reject_unknown_fields() {
        use super::super::super::resources_platform::SdkResult;
        toml::from_str::<Disposition>("kind = 'full'\n").unwrap();
        toml::from_str::<Disposition>("kind = 'full'\ntypo = 1\n").unwrap_err();
        toml::from_str::<SdkResult>("status = 'pending'\n").unwrap();
        toml::from_str::<SdkResult>("status = 'pending'\ntypo = 1\n").unwrap_err();
    }
}

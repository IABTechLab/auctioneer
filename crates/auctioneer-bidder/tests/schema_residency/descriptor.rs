//! Sealed actual-source C/I join, separate from conditional target evaluation.
//! No conversion to installed catalog costs, physical fit or serving permission.

use super::{expanded, initialization, program_construction, retained, runtime_sources};
use crate::schema_build::application::CapturedSeed;
use crate::schema_heap_layout::NativeField;
use crate::schema_nfa_heap::target;
use crate::schema_program_heap::{
    NativeProgramHeap, ProgramHeapError, ProgramRecipe, SourceRecipe,
};
use crate::schema_vendor;
use auctioneer_bidder::catalog::CatalogOwner;
use regex_automata::meta::Config;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::slice;

pub(crate) use super::program_construction::Sources;

#[cfg(test)]
pub(crate) use tests::assert_hypothetical_evaluation;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    Compiler(String),
    Graph(String),
    GraphMismatch,
    Coverage,
    UnsupportedState(String),
    RuntimeSource(&'static str),
    Program(String, ProgramHeapError),
    Construction(String, program_construction::Refusal),
    TargetIncomplete(Vec<String>),
    TargetBinding,
    TargetLayout(String),
    Arithmetic,
}

#[derive(Debug)]
struct Program {
    id: String,
    pattern: String,
    group: String,
    retained: SourceRecipe,
    constructor: program_construction::SourceRecipe,
}

#[derive(Debug)]
struct LinkedGroup {
    id: String,
    source_sha256: String,
    roots: BTreeSet<String>,
    inline: BTreeSet<String>,
}

/// Fields are private. Only describe can join an authenticated capture to models.
#[derive(Debug)]
pub(crate) struct ActualSourceDescriptor {
    binding: Value,
    recipe_identity: String,
    graph: Value,
    selection: Value,
    runtime: runtime_sources::Witness,
    programs: Vec<Program>,
    groups: Vec<LinkedGroup>,
    validation: Vec<ValidationAssignment>,
}

/// Conditional premises, supplied by the target evidence owner, never defaults.
pub(crate) struct RequestedFacts<'a> {
    pub binding: &'a Value,
    pub abi: &'a str,
    pub programs: &'a BTreeMap<String, ProgramRecipe<'a>>,
    pub constructor_layouts: &'a BTreeMap<String, BTreeMap<String, NativeField>>,
    pub linked_inline: &'a BTreeMap<String, LinkedInlineFact>,
}

/// One disjoint linked/inline group, including its statics, not a second cell bill.
pub(crate) struct LinkedInlineFact {
    pub source_sha256: String,
    pub budget_owner: CatalogOwner,
    pub attributed_bytes: u64,
    pub inline_layouts: BTreeMap<String, NativeField>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ValidationAssignment {
    pub child: String,
    pub source_owner: String,
    pub role: String,
}

/// Requested heap and supplied linked/inline attribution remain separate units.
/// This data has no physical qualification or production-cost conversion API.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RequestedStorage {
    pub abi: String,
    pub retained_requested_bytes: u64,
    pub construction_only_requested_bytes: u64,
    pub additive_requested_envelope: u64,
    pub linked_inline_attributed_bytes: u64,
    pub trace_peaks: Vec<u64>,
    pub constructor_source_visits: u64,
}

pub(super) fn model_sources() -> Value {
    macro_rules! source {
        ($relative:literal) => {
            (
                $relative,
                include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../", $relative))
                    .as_slice(),
            )
        };
    }
    // These are data witnesses, not source imports. Bind the component helpers
    // as well as the join: a changed child model must invalidate its recipe.
    let sources: &[(&str, &[u8])] = &[
        source!("crates/auctioneer-bidder/tests/schema_residency/descriptor.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/runtime_sources.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/expanded.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/auxiliary.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/options.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/storage.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/retained.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/initialization.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/program_construction.rs"),
        source!("crates/auctioneer-bidder/tests/schema_residency/literal_construction.rs"),
        source!("crates/auctioneer-bidder/tests/schema_program_heap/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_metadata_heap/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_nfa_heap/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_nfa_heap/target.rs"),
        source!("crates/auctioneer-bidder/tests/schema_onepass_bounds/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_literal_heap/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_pool_heap/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_heap_layout/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_cache_model/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_hybrid_prefix/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_syntax_construction/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_nfa_construction/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_onepass_construction/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_build/application.rs"),
        source!("crates/auctioneer-bidder/tests/schema_build/regex_dependencies.rs"),
        source!("crates/auctioneer-bidder/tests/schema_build/schema_regex_vendor/mod.rs"),
        source!("crates/auctioneer-bidder/tests/schema_vendor/mod.rs"),
    ];
    json!(
        sources
            .iter()
            .map(|(path, bytes)| {
                (
                    *path,
                    json!({"bytes":bytes.len(),"sha256":expanded::digest(bytes)}),
                )
            })
            .collect::<BTreeMap<_, _>>()
    )
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Refusal> {
    value[key]
        .as_str()
        .filter(|candidate| !candidate.is_empty())
        .ok_or(Refusal::Coverage)
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], Refusal> {
    value[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or(Refusal::Coverage)
}

fn source_group(row: &Value) -> Result<String, Refusal> {
    Ok(format!(
        "linked::declaration::{}::{}",
        text(row, "owner")?,
        text(row, "marker")?
    ))
}

fn row_programs(row: &Value) -> Result<Vec<(&Value, &str)>, Refusal> {
    let state = array(row, "schema_state")?;
    if !state.is_empty() {
        return Err(Refusal::UnsupportedState(text(row, "owner")?.to_owned()));
    }
    let mut programs = BTreeMap::new();
    for program in array(row, "programs")? {
        let symbol = text(program, "symbol")?;
        if programs.insert(symbol, program).is_some() {
            return Err(Refusal::Coverage);
        }
    }
    let mut result = Vec::new();
    for value in array(row, "initialization")? {
        let symbol = value.as_str().ok_or(Refusal::Coverage)?;
        let program = programs.remove(symbol).ok_or(Refusal::Coverage)?;
        result.push((program, text(program, "pattern")?));
    }
    if !programs.is_empty() {
        return Err(Refusal::Coverage);
    }
    Ok(result)
}

/// Auxiliary row ordinals are numeric, not declaration or lexical iteration order.
fn ordered_declarations(graph: &Value) -> Result<Vec<&Value>, Refusal> {
    let auxiliary = &graph["auxiliary"];
    if auxiliary["status"] != "Complete" {
        return Err(Refusal::Coverage);
    }
    let mut declarations = BTreeMap::new();
    let mut owners = BTreeSet::new();
    for row in array(graph, "declarations")? {
        if !owners.insert(text(row, "owner")?)
            || declarations.insert(text(row, "marker")?, row).is_some()
        {
            return Err(Refusal::Coverage);
        }
        row_programs(row)?;
    }
    let mut rows = BTreeMap::new();
    for root in array(auxiliary, "roots")? {
        if root["kind"] != "inline-retention-row" {
            continue;
        }
        let details = &root["details"];
        let entry = text(details, "entry")?;
        let ordinal = entry
            .strip_prefix("STAGED_VALIDATOR_")
            .ok_or(Refusal::Coverage)?
            .parse::<usize>()
            .map_err(|_error| Refusal::Coverage)?;
        let marker = text(details, "marker")?;
        let row = *declarations.get(marker).ok_or(Refusal::Coverage)?;
        if entry != format!("STAGED_VALIDATOR_{ordinal}")
            || details["owner"] != row["owner"]
            || rows.insert(ordinal, row).is_some()
        {
            return Err(Refusal::Coverage);
        }
    }
    let ordered = rows
        .into_iter()
        .enumerate()
        .map(|(expected, (ordinal, row))| {
            if ordinal == expected {
                Ok(row)
            } else {
                Err(Refusal::Coverage)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let markers = ordered
        .iter()
        .map(|row| text(row, "marker"))
        .collect::<Result<Vec<_>, _>>()?;
    let calls = array(auxiliary, "dispatch_edges")?
        .iter()
        .filter(|edge| edge["from"] == "initialize_native")
        .map(|edge| {
            if edge["role"] != "initialize-regexes" || edge["ownership"] != "callee-owned" {
                return Err(Refusal::Coverage);
            }
            text(edge, "to")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if ordered.len() != declarations.len() || calls != markers {
        return Err(Refusal::Coverage);
    }
    Ok(ordered)
}

fn program(
    id: &str,
    pattern: &str,
    group: &str,
    config: &Config,
    sources: &Sources<'_>,
) -> Result<Program, Refusal> {
    let retained = SourceRecipe::from_pattern(pattern, config)
        .map_err(|error| Refusal::Program(id.to_owned(), error))?;
    let constructor = program_construction::SourceRecipe::from_source(
        id,
        pattern,
        config,
        Sources {
            workspace: sources.workspace,
            syntax: sources.syntax,
            rust_release: sources.rust_release,
            features: sources.features,
        },
    )
    .map_err(|error| Refusal::Construction(id.to_owned(), error))?;
    if constructor.owner() != id
        || constructor.pattern() != pattern
        || constructor.required_facts().is_empty()
        || constructor
            .phase_models()
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != 13_usize
        || constructor
            .work_models()
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != 15_usize
    {
        return Err(Refusal::Coverage);
    }
    Ok(Program {
        id: id.to_owned(),
        pattern: pattern.to_owned(),
        group: group.to_owned(),
        retained,
        constructor,
    })
}

fn declaration_group(row: &Value) -> Result<LinkedGroup, Refusal> {
    let id = source_group(row)?;
    // This root denotes the ENTIRE authenticated expansion, including all
    // functions, types, literals and non-regex code, not just the pattern list.
    let mut roots = BTreeSet::from([format!(
        "expanded::{}::{}",
        text(row, "owner")?,
        text(row, "marker")?
    )]);
    let mut inline = BTreeSet::new();
    for row_program in array(row, "programs")? {
        let program_id = text(row_program, "identity")?.to_owned();
        if !roots.insert(format!("static::{program_id}")) || !inline.insert(program_id) {
            return Err(Refusal::Coverage);
        }
    }
    for readonly in array(row, "readonly")? {
        if !roots.insert(text(readonly, "identity")?.to_owned()) {
            return Err(Refusal::Coverage);
        }
    }
    Ok(LinkedGroup {
        id,
        roots,
        inline,
        source_sha256: text(row, "expansion_sha256")?.to_owned(),
    })
}

fn linked_groups(
    graph: &Value,
    runtime: &runtime_sources::Witness,
) -> Result<Vec<LinkedGroup>, Refusal> {
    let auxiliary = &graph["auxiliary"];
    let mut roots = BTreeSet::new();
    for root in array(auxiliary, "roots")? {
        let ordinal = root["ordinal"].as_u64().ok_or(Refusal::Coverage)?;
        if !roots.insert(format!("auxiliary::{ordinal}")) {
            return Err(Refusal::Coverage);
        }
    }
    if roots.is_empty() {
        return Err(Refusal::Coverage);
    }
    let mut groups = vec![
        LinkedGroup {
            id: "linked::captured-header".into(),
            roots,
            inline: BTreeSet::new(),
            source_sha256: expanded::digest(
                &serde_json::to_vec(auxiliary).map_err(|_error| Refusal::Coverage)?,
            ),
        },
        LinkedGroup {
            id: "linked::runtime-staging".into(),
            source_sha256: runtime.staging_sha256().to_owned(),
            roots: BTreeSet::from(["schema::staging::entire-owner".into()]),
            inline: BTreeSet::from([runtime_sources::HOSTNAME.into()]),
        },
        LinkedGroup {
            id: "linked::runtime-native".into(),
            source_sha256: runtime.native_sha256().to_owned(),
            roots: BTreeSet::from(["schema::native::entire-owner".into()]),
            inline: BTreeSet::from([runtime_sources::COORDINATOR.into()]),
        },
    ];
    for row in array(graph, "declarations")? {
        groups.push(declaration_group(row)?);
    }
    let mut assigned = BTreeSet::new();
    let mut group_ids = BTreeSet::new();
    for group in &groups {
        if !group_ids.insert(&group.id) || group.roots.iter().any(|root| !assigned.insert(root)) {
            return Err(Refusal::Coverage);
        }
    }
    Ok(groups)
}

fn validation_assignments(
    programs: &[&str],
    graph: &Value,
) -> Result<Vec<ValidationAssignment>, Refusal> {
    let mut rows = Vec::new();
    for id in programs {
        for role in ["cache", "mutable-scratch", "search-fallback"] {
            let child = if *id == runtime_sources::HOSTNAME && role == "cache" {
                format!("{id}::per-call-cache")
            } else {
                format!("{id}::validation::{role}")
            };
            rows.push(ValidationAssignment {
                child,
                source_owner: (*id).to_owned(),
                role: role.into(),
            });
        }
    }
    let mut formats = BTreeSet::new();
    for row in array(graph, "declarations")? {
        let owner = format!("{}::{}", text(row, "owner")?, text(row, "marker")?);
        for role in ["declaration-stack", "declaration-work"] {
            rows.push(ValidationAssignment {
                child: format!("{owner}::{role}"),
                source_owner: owner.clone(),
                role: role.into(),
            });
        }
        for callback in array(row, "callbacks")? {
            formats.insert(callback.as_str().ok_or(Refusal::Coverage)?);
        }
    }
    for format in formats {
        if !matches!(format, "uuid" | "hostname") {
            return Err(Refusal::Coverage);
        }
        let owner = format!("schema::staging::formats::{format}");
        for role in ["callback-stack", "callback-work"] {
            rows.push(ValidationAssignment {
                child: format!("{owner}::{role}"),
                source_owner: owner.clone(),
                role: role.into(),
            });
        }
    }
    exact_assignments(&rows, &rows)?;
    Ok(rows)
}

fn exact_assignments(
    expected: &[ValidationAssignment],
    actual: &[ValidationAssignment],
) -> Result<(), Refusal> {
    let unique = |rows: &[ValidationAssignment]| {
        rows.iter()
            .map(|row| row.child.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            == rows.len()
    };
    if expected.is_empty()
        || !unique(expected)
        || !unique(actual)
        || expected.iter().collect::<BTreeSet<_>>() != actual.iter().collect::<BTreeSet<_>>()
    {
        return Err(Refusal::Coverage);
    }
    Ok(())
}

pub(crate) fn describe(
    seed: &CapturedSeed,
    accepted_graph: &Value,
    sources: &Sources<'_>,
) -> Result<ActualSourceDescriptor, Refusal> {
    let compiler = schema_vendor::verify(sources.workspace).map_err(Refusal::Compiler)?;
    let graph = expanded::expand(seed).map_err(Refusal::Graph)?;
    if graph != *accepted_graph {
        return Err(Refusal::GraphMismatch);
    }
    let manifest: Value =
        serde_json::from_slice(&seed.manifest_bytes).map_err(|_error| Refusal::Coverage)?;
    let runtime = runtime_sources::capture(sources.workspace, &manifest)?;
    let ordered = ordered_declarations(&graph)?;
    let groups = linked_groups(&graph, &runtime)?;
    let mut programs = vec![program(
        runtime_sources::HOSTNAME,
        runtime.hostname(),
        "linked::runtime-staging",
        runtime.config(),
        sources,
    )?];
    for row in ordered {
        let group = source_group(row)?;
        for (child, pattern) in row_programs(row)? {
            programs.push(program(
                text(child, "identity")?,
                pattern,
                &group,
                runtime.config(),
                sources,
            )?);
        }
    }
    let mut ids = BTreeSet::new();
    if programs.iter().any(|row| !ids.insert(row.id.as_str())) {
        return Err(Refusal::Coverage);
    }
    let program_ids = programs
        .iter()
        .map(|program| program.id.as_str())
        .collect::<Vec<_>>();
    let validation = validation_assignments(&program_ids, &graph)?;
    let syntax_source = sources
        .syntax
        .to_str()
        .ok_or(Refusal::RuntimeSource("syntax source UTF-8"))?;
    let compiler_source = compiler
        .to_str()
        .ok_or(Refusal::RuntimeSource("compiler source UTF-8"))?;
    let binding = json!({"identity":seed.identity_sha256,
        "manifest_sha256":expanded::digest(&seed.manifest_bytes),
        "expanded_graph_sha256":expanded::digest(&serde_json::to_vec(&graph)
            .map_err(|_error| Refusal::Coverage)?),
        "runtime":runtime.report(),"regex_features":sources.features,
        "rust_release":sources.rust_release,"syntax_source":syntax_source,
        "schema_compiler":{"canonical_root":compiler_source,
            "verified_by":"schema_vendor::verify",
            "verifier_source":"crates/auctioneer-bidder/tests/schema_vendor/mod.rs"},
        "syntax_rank_source":{"path":"src/rank.rs",
            "sha256":"ff3d58b0cc5ffa69e2e8c56fc7d9ef41dd399d59a639a253a51551b858cb5bbd",
            "verified_by":"program_construction::SourceRecipe::from_source"},
        "model_sources":model_sources()});
    let recipe_identity =
        expanded::digest(&serde_json::to_vec(&binding).map_err(|_error| Refusal::Coverage)?);
    runtime.recheck()?;
    if schema_vendor::verify(sources.workspace).map_err(Refusal::Compiler)? != compiler {
        return Err(Refusal::Compiler("verified compiler source changed".into()));
    }
    Ok(ActualSourceDescriptor {
        binding,
        recipe_identity,
        graph,
        selection: manifest.get("selection").cloned().unwrap_or(Value::Null),
        runtime,
        programs,
        groups,
        validation,
    })
}

impl ActualSourceDescriptor {
    /// Stable data projection for the existing pure consumer; not a receipt.
    pub(crate) fn projection(&self) -> Value {
        let programs = self
            .programs
            .iter()
            .map(|program| {
                json!({
            "identity":program.id,"retained_layout_fields":["metadata","arc_header",
                "strategy_payload","nfa_facts","onepass_word","memchr_payload","pool"],
            "nfa_fact_keys":target::fact_keys(),
            "constructor_layout_keys":program.constructor.required_facts(),
            "inline_in_group":program.group})
            })
            .collect::<Vec<_>>();
        json!({"schema_version":1_u8,"kind":"schema_source_recipe_projection",
            "source_recipe_identity":self.recipe_identity,
            "common_status":"SourceComplete","target_status":"TargetIncomplete",
            "required_slots":{"programs":programs,"linked_inline_groups":
                self.groups.iter().map(|group| json!({"identity":group.id,
                    "source_sha256":group.source_sha256,"inline_children":group.inline,
                    "budget_owner":"CatalogOwner::Validators"})).collect::<Vec<_>>()},
            "formulas":{"C":"retained::compose","I":"initialization::compose",
                "linked_inline":"separate attribution, contains lazy cells; not added twice"},
            "numeric_costs":Value::Null,"authenticated_qualification_receipt":false,
            "physical_cost_conversion":"refused; requires existing #72-#75 acceptance",
            "physical_fit":false,"serving_permission":false})
    }

    pub(crate) fn report(&self) -> Value {
        let programs = self
            .programs
            .iter()
            .map(|program| {
                json!({
            "identity":program.id,"pattern":program.pattern,"linked_inline_group":program.group,
            "retained_formula":"schema_program_heap::SourceRecipe::evaluate",
            "constructor_formula":"program_construction::SourceRecipe::evaluate",
            "constructor_layout_keys":program.constructor.required_facts(),
            "retained_layouts":["metadata","arc_header","strategy_payload","nfa_facts",
                "onepass_word","memchr_payload","pool"],
            "retained_children":["metadata","strategy_arc","directional_nfas",
                "onepass_tables","automatic_prefilter","unused_pool"],
            "static_inline":"already included in linked_inline_group; reconcile, never add twice",
            "constructor_phases":program.constructor.phase_models().iter()
                .map(|stage| format!("{stage:?}")).collect::<Vec<_>>(),
            "source_work":program.constructor.work_models().iter()
                .map(|work| format!("{work:?}")).collect::<Vec<_>>() })
            })
            .collect::<Vec<_>>();
        let groups = self.groups.iter().map(|group| json!({
            "identity":group.id,"source_sha256":group.source_sha256,"roots":group.roots,
            "inline_children":group.inline,"budget_owner":"CatalogOwner::Validators",
            "units":"target-supplied disjoint linked/inline attribution bytes",
            "scope":"entire attributed source roots, including code, functions, literals, readonly data and schema registry metadata"
        })).collect::<Vec<_>>();
        json!({"schema_version":1_u8,"status":"SourceComplete",
            "kind":"actual_source_requested_storage_recipe","binding":self.binding,
            "projection":self.projection(),
            "membership":self.selection,"expanded_graph":self.graph,
            "programs_in_initialization_order":programs,"linked_inline_groups":groups,
            "validation_handoff":self.validation_handoff(),
            "constructor_work":{
                "units":"source collection-record/byte visits",
                "bound_kind":"checked upper bound, not an exact dynamic execution count",
                "aggregation":"sum of constructor source record/byte-visit upper bounds",
                "includes":["buffer initialization","buffer relocation","cleanup"],
                "excludes":["scalar moves","atomics","allocator internals",
                    "machine operations","elapsed time"],
                "scope":"construction and failure-prefix cleanup; not validation/search work",
                "latency_bound":false,"cpu_instruction_bound":false,
                "runtime_validation_token_bound":false},
            "retained_formula":"retained::compose; exact disjoint program heaps",
            "initialization_formula":"initialization::compose; serial phase scratch maxima with final C covering each live retained prefix",
            "linked_inline_formula":"separate supplied group attribution; includes lazy cells and native coordinator",
            "target_status":"TargetIncomplete","requested_storage":Value::Null,
            "target_owner":"#72-#75: matched ABI, layouts, linked artifact and allocator reconciliation",
            "catalog_boundary":"schema-specific registry metadata is in captured-header; generic catalog Metadata/Registry heaps retain their existing owners, not a new BTree proof",
            "qualification":"Unqualified","resource_qualification":false,
            "physical_fit":false,"serving_permission":false})
    }

    pub(crate) fn validation_handoff(&self) -> Value {
        json!({"consumer":"#47","binding":self.binding,
            "units":"source-identified obligations, not numeric V/work/stack bounds",
            "lifetime":"validation-call; never C/I",
            "children":self.validation.iter().map(|row| json!({"child":row.child,
                "source_owner":row.source_owner,"role":row.role})).collect::<Vec<_>>()})
    }

    pub(crate) fn check_validation_handoff(
        &self,
        rows: &[ValidationAssignment],
    ) -> Result<(), Refusal> {
        exact_assignments(&self.validation, rows)
    }

    pub(crate) fn evaluate(&self, facts: &RequestedFacts<'_>) -> Result<RequestedStorage, Refusal> {
        self.check_facts(facts)?;
        self.runtime.recheck()?;
        let mut heaps = Vec::new();
        let mut constructors = Vec::new();
        for program in &self.programs {
            let root = facts.programs.get(&program.id).ok_or(Refusal::Coverage)?;
            let heap = program
                .retained
                .evaluate(root)
                .map_err(|error| match error {
                    ProgramHeapError::Arithmetic => Refusal::Arithmetic,
                    ProgramHeapError::Configuration
                    | ProgramHeapError::Compilation
                    | ProgramHeapError::Metadata
                    | ProgramHeapError::RootLayout
                    | ProgramHeapError::Layout
                    | ProgramHeapError::Nfa
                    | ProgramHeapError::OnePass
                    | ProgramHeapError::AutomaticPrefilter(_)
                    | ProgramHeapError::Pool
                    | ProgramHeapError::Shape => Refusal::TargetLayout(program.id.clone()),
                })?;
            let layouts = facts
                .constructor_layouts
                .get(&program.id)
                .ok_or(Refusal::Coverage)?;
            let constructor =
                program
                    .constructor
                    .evaluate(layouts, root)
                    .map_err(|error| match error {
                        program_construction::Refusal::Missing(key) => Refusal::TargetIncomplete(
                            vec![format!("constructor::{}::{key}", program.id)],
                        ),
                        program_construction::Refusal::Layout => {
                            Refusal::TargetLayout(program.id.clone())
                        }
                        program_construction::Refusal::Arithmetic => Refusal::Arithmetic,
                        program_construction::Refusal::Configuration
                        | program_construction::Refusal::PatternScope
                        | program_construction::Refusal::Features
                        | program_construction::Refusal::Source(_)
                        | program_construction::Refusal::RetainedCoverage
                        | program_construction::Refusal::UnboundWork(_) => {
                            Refusal::Construction(program.id.clone(), error)
                        }
                    })?;
            if constructor.owner != program.id || constructor.retained != heap {
                return Err(Refusal::Coverage);
            }
            heaps.push(heap);
            constructors.push(constructor);
        }
        let linked_inline_attributed_bytes = self.reconcile_groups(facts, &heaps)?;
        let totals = compose(&self.programs, &heaps, &constructors)?;
        let constructor_source_visits = constructors.iter().try_fold(0_u64, |sum, row| {
            let visits = row
                .complete_work()
                .map_err(|error| Refusal::Construction(row.owner.clone(), error))?;
            sum.checked_add(as_u64(visits)?).ok_or(Refusal::Arithmetic)
        })?;
        self.runtime.recheck()?;
        Ok(RequestedStorage {
            abi: facts.abi.to_owned(),
            retained_requested_bytes: totals.retained,
            construction_only_requested_bytes: totals.construction_only,
            additive_requested_envelope: totals.additive_envelope,
            linked_inline_attributed_bytes,
            trace_peaks: totals.trace_peaks,
            constructor_source_visits,
        })
    }

    fn check_facts(&self, facts: &RequestedFacts<'_>) -> Result<(), Refusal> {
        if facts.binding != &self.binding || facts.abi.trim().is_empty() {
            return Err(Refusal::TargetBinding);
        }
        let expected = self
            .programs
            .iter()
            .map(|row| row.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut missing = Vec::new();
        exact_keys(
            &expected,
            facts.programs.keys().map(String::as_str),
            "program",
            &mut missing,
        )?;
        exact_keys(
            &expected,
            facts.constructor_layouts.keys().map(String::as_str),
            "constructor",
            &mut missing,
        )?;
        let groups = self
            .groups
            .iter()
            .map(|row| row.id.as_str())
            .collect::<BTreeSet<_>>();
        exact_keys(
            &groups,
            facts.linked_inline.keys().map(String::as_str),
            "linked-inline",
            &mut missing,
        )?;
        for program in &self.programs {
            program_fact_keys(
                &program.id,
                program.constructor.required_facts(),
                facts
                    .programs
                    .get(&program.id)
                    .map(|root| root.nfa_facts.keys().map(String::as_str)),
                facts
                    .constructor_layouts
                    .get(&program.id)
                    .map(|layouts| layouts.keys().map(String::as_str)),
                &mut missing,
            )?;
        }
        for group in &self.groups {
            if let Some(fact) = facts.linked_inline.get(&group.id) {
                let inline_keys = group
                    .inline
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                exact_keys(
                    &inline_keys,
                    fact.inline_layouts.keys().map(String::as_str),
                    &format!("inline::{}", group.id),
                    &mut missing,
                )?;
            }
        }
        if !missing.is_empty() {
            return Err(Refusal::TargetIncomplete(missing));
        }
        common_target_layouts(facts.programs, facts.constructor_layouts)
    }

    fn reconcile_groups(
        &self,
        facts: &RequestedFacts<'_>,
        heaps: &[NativeProgramHeap],
    ) -> Result<u64, Refusal> {
        let mut total = 0_u64;
        for group in &self.groups {
            let fact = facts
                .linked_inline
                .get(&group.id)
                .ok_or(Refusal::Coverage)?;
            if fact.source_sha256 != group.source_sha256
                || fact.budget_owner != CatalogOwner::Validators
            {
                return Err(Refusal::TargetBinding);
            }
            let inline = fact.inline_layouts.keys().cloned().collect::<BTreeSet<_>>();
            if inline != group.inline {
                return Err(Refusal::TargetLayout(group.id.clone()));
            }
            let mut contained = 0_u64;
            for (id, field) in &fact.inline_layouts {
                field
                    .validate()
                    .map_err(|_error| Refusal::TargetLayout(id.clone()))?;
                contained = contained
                    .checked_add(as_u64(field.bytes)?)
                    .ok_or(Refusal::Arithmetic)?;
                match self
                    .programs
                    .iter()
                    .enumerate()
                    .find(|(_, program)| &program.id == id)
                {
                    Some((index, program)) => {
                        let root = facts.programs.get(id).ok_or(Refusal::Coverage)?;
                        let heap = heaps.get(index).ok_or(Refusal::Coverage)?;
                        if program.group != group.id
                            || heap.static_inline_bytes != field.bytes
                            || root.metadata.static_inline != *field
                        {
                            return Err(Refusal::TargetLayout(id.clone()));
                        }
                    }
                    None if id == runtime_sources::COORDINATOR => (),
                    None => return Err(Refusal::TargetLayout(id.clone())),
                }
            }
            if contained > fact.attributed_bytes {
                return Err(Refusal::TargetLayout(group.id.clone()));
            }
            total = total
                .checked_add(fact.attributed_bytes)
                .ok_or(Refusal::Arithmetic)?;
        }
        Ok(total)
    }
}

// Coverage is checked first. One ABI cannot assign different layouts to the
// same named type or to the fixed Regex/metadata/Arc/pool field types.
fn common_target_layouts(
    programs: &BTreeMap<String, ProgramRecipe<'_>>,
    constructors: &BTreeMap<String, BTreeMap<String, NativeField>>,
) -> Result<(), Refusal> {
    let (first_id, first) = programs.first_key_value().ok_or(Refusal::TargetBinding)?;
    let common = constructors.get(first_id).ok_or(Refusal::TargetBinding)?;
    for (id, program) in programs {
        let constructor = constructors.get(id).ok_or(Refusal::TargetBinding)?;
        if constructor != common
            || program.metadata != first.metadata
            || program.arc_header != first.arc_header
            || program.onepass_word != first.onepass_word
            || program.memchr_payload != first.memchr_payload
            || program.pool != first.pool
            || program
                .nfa_facts
                .iter()
                .any(|(name, layout)| constructor.get(name) != Some(layout))
        {
            return Err(Refusal::TargetBinding);
        }
    }
    // Concrete strategy payloads can differ with the selected pattern/strategy.
    // Agreement does not validate individual layouts or authenticate an ABI.
    Ok(())
}

fn program_fact_keys<'a>(
    owner: &str,
    required_constructor: &BTreeSet<String>,
    nfa: Option<impl Iterator<Item = &'a str>>,
    constructor: Option<impl Iterator<Item = &'a str>>,
    missing: &mut Vec<String>,
) -> Result<(), Refusal> {
    if let Some(keys) = nfa {
        let required = target::fact_keys().into_iter().collect::<BTreeSet<_>>();
        exact_keys(&required, keys, &format!("program::{owner}::nfa"), missing)?;
    }
    if let Some(keys) = constructor {
        let required = required_constructor
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        exact_keys(&required, keys, &format!("constructor::{owner}"), missing)?;
    }
    Ok(())
}

fn exact_keys<'a>(
    expected: &BTreeSet<&str>,
    actual: impl Iterator<Item = &'a str>,
    role: &str,
    missing: &mut Vec<String>,
) -> Result<(), Refusal> {
    let actual_keys = actual.collect::<BTreeSet<_>>();
    if !actual_keys.is_subset(expected) {
        return Err(Refusal::TargetBinding);
    }
    missing.extend(
        expected
            .difference(&actual_keys)
            .map(|id| format!("{role}::{id}")),
    );
    Ok(())
}

fn as_u64(value: usize) -> Result<u64, Refusal> {
    u64::try_from(value).map_err(|_error| Refusal::Arithmetic)
}

fn retained_total(
    ids: &[&str],
    final_owners: &[initialization::Allocation<'_>],
) -> Result<u64, Refusal> {
    if ids.len() != final_owners.len() {
        return Err(Refusal::Coverage);
    }
    let expected = ids.iter().map(|id| (*id, *id)).collect::<Vec<_>>();
    let charges = ids
        .iter()
        .zip(final_owners)
        .map(|(id, row)| retained::Charge {
            owner: row.owner,
            covered: slice::from_ref(id),
            bytes: row.bytes,
        })
        .collect::<Vec<_>>();
    retained::compose(&expected, &charges, &[]).map_err(|error| match error {
        retained::Refusal::Arithmetic => Refusal::Arithmetic,
        retained::Refusal::Coverage | retained::Refusal::Sharing => Refusal::Coverage,
    })
}

fn compose(
    programs: &[Program],
    heaps: &[NativeProgramHeap],
    constructors: &[program_construction::Certificate],
) -> Result<initialization::Totals, Refusal> {
    if programs.is_empty() || programs.len() != heaps.len() || programs.len() != constructors.len()
    {
        return Err(Refusal::Coverage);
    }
    let ids = programs
        .iter()
        .map(|row| row.id.as_str())
        .collect::<Vec<_>>();
    let final_owners = ids
        .iter()
        .zip(heaps)
        .map(|(id, heap)| {
            Ok(initialization::Allocation {
                owner: id,
                bytes: as_u64(heap.total)?,
            })
        })
        .collect::<Result<Vec<_>, Refusal>>()?;
    let c = retained_total(&ids, &final_owners)?;
    let mut labels = Vec::new();
    let mut scratch_ids = Vec::new();
    let mut trace_rows = Vec::new();
    for (index, (program, constructor)) in programs.iter().zip(constructors).enumerate() {
        let expected_phases = program
            .constructor
            .phase_models()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if constructor
            .phase_scratch
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != expected_phases
            || constructor.phase_scratch.values().copied().max()
                != Some(constructor.construction_only_bytes)
        {
            return Err(Refusal::Coverage);
        }
        for (phase, scratch) in &constructor.phase_scratch {
            labels.push(format!("{}::{phase:?}", program.id));
            scratch_ids.push(format!("construction-only::{}::{phase:?}", program.id));
            trace_rows.push((index, as_u64(*scratch)?));
        }
    }
    let scratch = scratch_ids
        .iter()
        .zip(&trace_rows)
        .map(|(id, (_, bytes))| {
            if *bytes == 0_u64 {
                Vec::new()
            } else {
                vec![initialization::Allocation {
                    owner: id,
                    bytes: *bytes,
                }]
            }
        })
        .collect::<Vec<_>>();
    // Every phase, including failure cleanup, is bounded by this retained
    // prefix's final capacity. Scratch is disjoint, never observed_peak - C.
    let traces = trace_rows
        .iter()
        .enumerate()
        .map(|(trace, (program, _))| {
            Ok(initialization::Trace {
                label: labels.get(trace).ok_or(Refusal::Coverage)?,
                retained: final_owners.get(..=*program).ok_or(Refusal::Coverage)?,
                scratch: scratch.get(trace).ok_or(Refusal::Coverage)?,
            })
        })
        .collect::<Result<Vec<_>, Refusal>>()?;
    let required = labels.iter().map(String::as_str).collect::<Vec<_>>();
    let totals =
        initialization::compose(&final_owners, &required, &traces).map_err(
            |error| match error {
                initialization::Refusal::Arithmetic => Refusal::Arithmetic,
                initialization::Refusal::Owners
                | initialization::Refusal::Traces
                | initialization::Refusal::RetainedCoverage => Refusal::Coverage,
            },
        )?;
    if totals.retained != c {
        return Err(Refusal::Coverage);
    }
    Ok(totals)
}

#[cfg(test)]
mod tests {
    use super::program_construction::Stage;
    use super::*;
    use crate::schema_nfa_heap::target::{NFA_INNER, fact_keys as nfa_fact_keys};
    use std::fs::{self, DirBuilder};
    use tempfile::tempdir;

    #[test]
    fn describe_refuses_compiler_source_before_invalid_seed() {
        for invalid_archive in [false, true] {
            let temporary = tempdir().unwrap();
            let workspace = temporary.path().canonicalize().unwrap();
            let vendor = workspace.join("vendor");
            let reason = if invalid_archive {
                DirBuilder::new().create(&vendor).unwrap();
                fs::write(
                    vendor.join("jsonschema-macros-core-0.62.0.crate"),
                    b"invalid compiler archive",
                )
                .unwrap();
                "archive SHA-256 differs from approved crates.io archive".to_owned()
            } else {
                fs::write(&vendor, b"not a compiler directory").unwrap();
                format!("not a directory: {}", vendor.display())
            };
            let seed = CapturedSeed {
                identity_sha256: String::new(),
                manifest_bytes: Vec::new(),
                rust: Vec::new(),
                build_root: workspace.clone(),
                out_dir: workspace.clone(),
                snapshots: Vec::new(),
            };
            assert_eq!(
                expanded::expand(&seed),
                Err("invalid captured staging manifest".to_owned())
            );
            let features = BTreeSet::new();
            let sources = Sources {
                workspace: &workspace,
                syntax: &workspace,
                rust_release: "invalid compiler-first control",
                features: &features,
            };
            assert_eq!(
                describe(&seed, &json!({}), &sources).err(),
                Some(Refusal::Compiler(reason))
            );
        }
    }

    type HypotheticalRoots<'a> = BTreeMap<String, ProgramRecipe<'a>>;
    type HypotheticalConstructors = BTreeMap<String, BTreeMap<String, NativeField>>;
    type HypotheticalLinked = BTreeMap<String, LinkedInlineFact>;

    /// Invoke once for the current capture in each existing real-growth lane,
    /// after describe succeeds, never once per application-artifact lane.
    /// Every supplied layout and attribution is hypothetical, never an
    /// observed native/guest layout, physical cost or qualification receipt.
    pub(crate) fn assert_hypothetical_evaluation(
        descriptor: &ActualSourceDescriptor,
    ) -> RequestedStorage {
        assert!(descriptor.programs.len() >= 2_usize);
        assert!(descriptor.groups.len() >= 2_usize);
        let layouts = program_construction::hypothetical_layouts();
        // Own the exact retained-NFA family separately from the full
        // constructor family. Neither map represents observed target facts.
        let nfa_layouts = nfa_fact_keys()
            .into_iter()
            .map(|key| {
                (
                    key.to_owned(),
                    *layouts.get(key).expect("required NFA layout"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(nfa_layouts.len(), 11_usize);
        let roots = descriptor
            .programs
            .iter()
            .map(|program| {
                let mut root =
                    program_construction::hypothetical_program(&program.pattern, &layouts);
                root.nfa_facts = &nfa_layouts;
                (program.id.clone(), root)
            })
            .collect::<HypotheticalRoots<'_>>();
        let constructors = descriptor
            .programs
            .iter()
            .map(|program| (program.id.clone(), layouts.clone()))
            .collect::<HypotheticalConstructors>();
        let linked = hypothetical_linked(descriptor, &roots);
        assert_eq!(roots.len(), descriptor.programs.len());
        assert_eq!(constructors.len(), descriptor.programs.len());
        assert_eq!(linked.len(), descriptor.groups.len());
        let facts = hypothetical_facts(&descriptor.binding, &roots, &constructors, &linked);
        let heaps = descriptor
            .programs
            .iter()
            .map(|program| program.retained.evaluate(&roots[&program.id]).unwrap())
            .collect::<Vec<_>>();
        let certificates = descriptor
            .programs
            .iter()
            .map(|program| {
                program
                    .constructor
                    .evaluate(&constructors[&program.id], &roots[&program.id])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let expected = hypothetical_oracle(&heaps, &certificates, &linked);
        assert!(expected.retained_requested_bytes > 0_u64);
        assert!(expected.construction_only_requested_bytes > 0_u64);
        assert!(expected.constructor_source_visits > 0_u64);
        let evaluated = descriptor.evaluate(&facts).unwrap();
        assert_eq!(evaluated, expected);
        assert_constructor_work_semantics(&descriptor.report()["constructor_work"]);
        assert_top_level_slots(descriptor, &facts);
        assert_required_layout_slots(descriptor, &facts);
        assert_extra_layout_slots(descriptor, &facts);
        assert_binding_axes(descriptor, &facts, &heaps);
        assert_inline_axes(descriptor, &facts, &heaps);
        assert_root_axes(descriptor, &facts);
        assert_overflow_axes(descriptor, &facts, &heaps, certificates);
        evaluated
    }

    fn assert_constructor_work_semantics(actual: &Value) {
        assert_eq!(
            actual,
            &json!({
                "units": "source collection-record/byte visits",
                "bound_kind": "checked upper bound, not an exact dynamic execution count",
                "aggregation": "sum of constructor source record/byte-visit upper bounds",
                "includes": ["buffer initialization", "buffer relocation", "cleanup"],
                "excludes": [
                    "scalar moves", "atomics", "allocator internals",
                    "machine operations", "elapsed time"
                ],
                "scope": "construction and failure-prefix cleanup; not validation/search work",
                "latency_bound": false,
                "cpu_instruction_bound": false,
                "runtime_validation_token_bound": false
            })
        );
    }

    fn hypothetical_facts<'a>(
        binding: &'a Value,
        programs: &'a HypotheticalRoots<'a>,
        constructors: &'a HypotheticalConstructors,
        linked: &'a HypotheticalLinked,
    ) -> RequestedFacts<'a> {
        RequestedFacts {
            binding,
            abi: "hypothetical-only-not-observed-or-physical",
            programs,
            constructor_layouts: constructors,
            linked_inline: linked,
        }
    }

    fn hypothetical_linked(
        descriptor: &ActualSourceDescriptor,
        roots: &HypotheticalRoots<'_>,
    ) -> HypotheticalLinked {
        descriptor
            .groups
            .iter()
            .map(|group| {
                let inline_layouts = group
                    .inline
                    .iter()
                    .map(|id| {
                        let field = if let Some(root) = roots.get(id) {
                            root.metadata.static_inline
                        } else {
                            assert_eq!(id, runtime_sources::COORDINATOR);
                            NativeField {
                                bytes: 8_usize,
                                alignment: 8_usize,
                            }
                        };
                        (id.clone(), field)
                    })
                    .collect::<BTreeMap<_, _>>();
                let contained = inline_layouts
                    .values()
                    .try_fold(0_u64, |total, field| {
                        total.checked_add(u64::try_from(field.bytes).unwrap())
                    })
                    .unwrap();
                (
                    group.id.clone(),
                    LinkedInlineFact {
                        source_sha256: group.source_sha256.clone(),
                        budget_owner: CatalogOwner::Validators,
                        attributed_bytes: contained.checked_add(257_u64).unwrap(),
                        inline_layouts,
                    },
                )
            })
            .collect()
    }

    fn hypothetical_oracle(
        heaps: &[NativeProgramHeap],
        certificates: &[program_construction::Certificate],
        linked: &HypotheticalLinked,
    ) -> RequestedStorage {
        let mut retained = 0_u64;
        let mut scratch = 0_u64;
        let mut visits = 0_u64;
        let mut peaks = Vec::new();
        assert_eq!(heaps.len(), certificates.len());
        for (heap, certificate) in heaps.iter().zip(certificates) {
            retained = retained
                .checked_add(u64::try_from(heap.total).unwrap())
                .unwrap();
            for stage in [
                Stage::Syntax,
                Stage::Literal,
                Stage::ForwardNfa,
                Stage::PikeVm,
                Stage::Backtrack,
                Stage::OnePass,
                Stage::ReverseNfa,
                Stage::HybridForward,
                Stage::HybridReverse,
                Stage::HybridPair,
                Stage::Publication,
                Stage::Cleanup,
                Stage::FailureCleanup,
            ] {
                let bytes = u64::try_from(certificate.phase_scratch[&stage]).unwrap();
                scratch = scratch.max(bytes);
                peaks.push(retained.checked_add(bytes).unwrap());
            }
            visits = visits
                .checked_add(u64::try_from(certificate.complete_work().unwrap()).unwrap())
                .unwrap();
        }
        let linked_bytes = linked
            .values()
            .try_fold(0_u64, |total, fact| {
                total.checked_add(fact.attributed_bytes)
            })
            .unwrap();
        RequestedStorage {
            abi: "hypothetical-only-not-observed-or-physical".to_owned(),
            retained_requested_bytes: retained,
            construction_only_requested_bytes: scratch,
            additive_requested_envelope: retained.checked_add(scratch).unwrap(),
            linked_inline_attributed_bytes: linked_bytes,
            trace_peaks: peaks,
            constructor_source_visits: visits,
        }
    }

    fn assert_top_level_slots(descriptor: &ActualSourceDescriptor, facts: &RequestedFacts<'_>) {
        for id in facts.programs.keys() {
            let mut roots: HypotheticalRoots<'_> = facts.programs.clone();
            assert!(roots.remove(id).is_some());
            let missing_root = hypothetical_facts(
                facts.binding,
                &roots,
                facts.constructor_layouts,
                facts.linked_inline,
            );
            assert_eq!(
                descriptor.evaluate(&missing_root),
                Err(Refusal::TargetIncomplete(vec![format!("program::{id}")]))
            );
            let mut constructors = facts.constructor_layouts.clone();
            assert!(constructors.remove(id).is_some());
            let missing_constructor = hypothetical_facts(
                facts.binding,
                facts.programs,
                &constructors,
                facts.linked_inline,
            );
            assert_eq!(
                descriptor.evaluate(&missing_constructor),
                Err(Refusal::TargetIncomplete(vec![format!(
                    "constructor::{id}"
                )]))
            );
        }
        for id in facts.linked_inline.keys() {
            let mut linked = hypothetical_linked(descriptor, facts.programs);
            assert!(linked.remove(id).is_some());
            let missing = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &linked,
            );
            assert_eq!(
                descriptor.evaluate(&missing),
                Err(Refusal::TargetIncomplete(vec![format!(
                    "linked-inline::{id}"
                )]))
            );
        }
        assert_unexpected_top_level_slots(descriptor, facts);
    }

    fn assert_unexpected_top_level_slots(
        descriptor: &ActualSourceDescriptor,
        facts: &RequestedFacts<'_>,
    ) {
        let unknown = "hypothetical::unexpected-source-slot";
        let mut roots: HypotheticalRoots<'_> = facts.programs.clone();
        assert!(
            roots
                .insert(unknown.to_owned(), *facts.programs.values().next().unwrap())
                .is_none()
        );
        assert_eq!(
            descriptor.evaluate(&hypothetical_facts(
                facts.binding,
                &roots,
                facts.constructor_layouts,
                facts.linked_inline
            )),
            Err(Refusal::TargetBinding)
        );
        let mut constructors = facts.constructor_layouts.clone();
        assert!(
            constructors
                .insert(
                    unknown.to_owned(),
                    facts.constructor_layouts.values().next().unwrap().clone()
                )
                .is_none()
        );
        assert_eq!(
            descriptor.evaluate(&hypothetical_facts(
                facts.binding,
                facts.programs,
                &constructors,
                facts.linked_inline
            )),
            Err(Refusal::TargetBinding)
        );
        let mut linked = hypothetical_linked(descriptor, facts.programs);
        assert!(
            linked
                .insert(
                    unknown.to_owned(),
                    LinkedInlineFact {
                        source_sha256: "hypothetical-unbound-source".to_owned(),
                        budget_owner: CatalogOwner::Validators,
                        attributed_bytes: 1_u64,
                        inline_layouts: BTreeMap::new(),
                    }
                )
                .is_none()
        );
        assert_eq!(
            descriptor.evaluate(&hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &linked
            )),
            Err(Refusal::TargetBinding)
        );
    }

    fn assert_required_layout_slots(
        descriptor: &ActualSourceDescriptor,
        facts: &RequestedFacts<'_>,
    ) {
        for program in &descriptor.programs {
            for key in nfa_fact_keys() {
                let mut missing = facts.programs[&program.id].nfa_facts.clone();
                assert!(missing.remove(key).is_some());
                let mut roots: HypotheticalRoots<'_> = facts.programs.clone();
                roots.get_mut(&program.id).unwrap().nfa_facts = &missing;
                let input = hypothetical_facts(
                    facts.binding,
                    &roots,
                    facts.constructor_layouts,
                    facts.linked_inline,
                );
                assert_eq!(
                    descriptor.evaluate(&input),
                    Err(Refusal::TargetIncomplete(vec![format!(
                        "program::{}::nfa::{key}",
                        program.id
                    )]))
                );
            }
            for key in program.constructor.required_facts() {
                let mut constructors = facts.constructor_layouts.clone();
                assert!(
                    constructors
                        .get_mut(&program.id)
                        .unwrap()
                        .remove(key)
                        .is_some()
                );
                let input = hypothetical_facts(
                    facts.binding,
                    facts.programs,
                    &constructors,
                    facts.linked_inline,
                );
                assert_eq!(
                    descriptor.evaluate(&input),
                    Err(Refusal::TargetIncomplete(vec![format!(
                        "constructor::{}::{key}",
                        program.id
                    )]))
                );
            }
        }
    }

    fn assert_extra_layout_slots(descriptor: &ActualSourceDescriptor, facts: &RequestedFacts<'_>) {
        for program in &descriptor.programs {
            let extra_key = "hypothetical::unexpected-inner-layout";
            let valid_field = facts.programs[&program.id].arc_header;
            let mut nfa = facts.programs[&program.id].nfa_facts.clone();
            assert_eq!(nfa.len(), 11_usize);
            assert!(nfa.insert(extra_key.to_owned(), valid_field).is_none());
            let mut roots: HypotheticalRoots<'_> = facts.programs.clone();
            roots.get_mut(&program.id).unwrap().nfa_facts = &nfa;
            let nfa_extra = hypothetical_facts(
                facts.binding,
                &roots,
                facts.constructor_layouts,
                facts.linked_inline,
            );
            assert_eq!(descriptor.evaluate(&nfa_extra), Err(Refusal::TargetBinding));
            let mut constructors = facts.constructor_layouts.clone();
            assert!(
                constructors
                    .get_mut(&program.id)
                    .unwrap()
                    .insert(extra_key.to_owned(), valid_field)
                    .is_none()
            );
            let constructor_extra = hypothetical_facts(
                facts.binding,
                facts.programs,
                &constructors,
                facts.linked_inline,
            );
            assert_eq!(
                descriptor.evaluate(&constructor_extra),
                Err(Refusal::TargetBinding)
            );
        }
    }

    fn assert_binding_axes(
        descriptor: &ActualSourceDescriptor,
        facts: &RequestedFacts<'_>,
        heaps: &[NativeProgramHeap],
    ) {
        let binding = json!({"hypothetical":"not-the-described-source"});
        assert_ne!(facts.binding, &binding);
        let wrong = hypothetical_facts(
            &binding,
            facts.programs,
            facts.constructor_layouts,
            facts.linked_inline,
        );
        assert_eq!(descriptor.evaluate(&wrong), Err(Refusal::TargetBinding));
        let mut blank = hypothetical_facts(
            facts.binding,
            facts.programs,
            facts.constructor_layouts,
            facts.linked_inline,
        );
        blank.abi = " ";
        assert_eq!(descriptor.evaluate(&blank), Err(Refusal::TargetBinding));
        for id in facts.linked_inline.keys() {
            let mut changed_source = hypothetical_linked(descriptor, facts.programs);
            changed_source
                .get_mut(id)
                .unwrap()
                .source_sha256
                .push_str("-hypothetical-mismatch");
            let source = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &changed_source,
            );
            assert_eq!(
                descriptor.reconcile_groups(&source, heaps),
                Err(Refusal::TargetBinding)
            );
            let mut changed_owner = hypothetical_linked(descriptor, facts.programs);
            changed_owner.get_mut(id).unwrap().budget_owner = CatalogOwner::Metadata;
            let owner = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &changed_owner,
            );
            assert_eq!(
                descriptor.reconcile_groups(&owner, heaps),
                Err(Refusal::TargetBinding)
            );
        }
    }

    fn assert_inline_layout_axes(
        descriptor: &ActualSourceDescriptor,
        facts: &RequestedFacts<'_>,
        heaps: &[NativeProgramHeap],
        group: &str,
        layouts: &BTreeMap<String, NativeField>,
    ) {
        for (id, field) in layouts {
            let mut removed = hypothetical_linked(descriptor, facts.programs);
            assert!(
                removed
                    .get_mut(group)
                    .unwrap()
                    .inline_layouts
                    .remove(id)
                    .is_some()
            );
            let missing = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &removed,
            );
            assert_eq!(
                descriptor.evaluate(&missing),
                Err(Refusal::TargetIncomplete(vec![format!(
                    "inline::{group}::{id}"
                )]))
            );
            let mut invalid = hypothetical_linked(descriptor, facts.programs);
            invalid
                .get_mut(group)
                .unwrap()
                .inline_layouts
                .get_mut(id)
                .unwrap()
                .bytes = 0_usize;
            let malformed = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &invalid,
            );
            assert_eq!(
                descriptor.reconcile_groups(&malformed, heaps),
                Err(Refusal::TargetLayout(id.clone()))
            );
            if facts.programs.contains_key(id) {
                for replacement in [
                    NativeField {
                        bytes: field.bytes.checked_add(field.alignment).unwrap(),
                        ..*field
                    },
                    NativeField {
                        alignment: field.alignment.checked_mul(2_usize).unwrap(),
                        ..*field
                    },
                ] {
                    let mut mismatched = hypothetical_linked(descriptor, facts.programs);
                    *mismatched
                        .get_mut(group)
                        .unwrap()
                        .inline_layouts
                        .get_mut(id)
                        .unwrap() = replacement;
                    let unequal = hypothetical_facts(
                        facts.binding,
                        facts.programs,
                        facts.constructor_layouts,
                        &mismatched,
                    );
                    assert_eq!(
                        descriptor.reconcile_groups(&unequal, heaps),
                        Err(Refusal::TargetLayout(id.clone()))
                    );
                }
            }
        }
    }

    fn assert_inline_axes(
        descriptor: &ActualSourceDescriptor,
        facts: &RequestedFacts<'_>,
        heaps: &[NativeProgramHeap],
    ) {
        let baseline = descriptor.reconcile_groups(facts, heaps).unwrap();
        for (group, fact) in facts.linked_inline {
            assert_inline_layout_axes(descriptor, facts, heaps, group, &fact.inline_layouts);
            let mut extra = hypothetical_linked(descriptor, facts.programs);
            assert!(
                extra
                    .get_mut(group)
                    .unwrap()
                    .inline_layouts
                    .insert(
                        "hypothetical::unexpected-inline-child".to_owned(),
                        NativeField {
                            bytes: 8_usize,
                            alignment: 8_usize
                        },
                    )
                    .is_none()
            );
            let unexpected = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &extra,
            );
            assert_eq!(
                descriptor.evaluate(&unexpected),
                Err(Refusal::TargetBinding)
            );
            let contained = fact
                .inline_layouts
                .values()
                .try_fold(0_u64, |total, field| {
                    total.checked_add(u64::try_from(field.bytes).unwrap())
                })
                .unwrap();
            let mut boundary = hypothetical_linked(descriptor, facts.programs);
            boundary.get_mut(group).unwrap().attributed_bytes = contained;
            let exact = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &boundary,
            );
            assert_eq!(
                descriptor.reconcile_groups(&exact, heaps),
                Ok(baseline.checked_sub(257_u64).unwrap())
            );
            if contained > 0_u64 {
                boundary.get_mut(group).unwrap().attributed_bytes = contained
                    .checked_sub(1_u64)
                    .expect("positive linked attribution");
                let short = hypothetical_facts(
                    facts.binding,
                    facts.programs,
                    facts.constructor_layouts,
                    &boundary,
                );
                assert_eq!(
                    descriptor.reconcile_groups(&short, heaps),
                    Err(Refusal::TargetLayout(group.clone()))
                );
            }
        }
    }

    fn assert_root_axes(descriptor: &ActualSourceDescriptor, facts: &RequestedFacts<'_>) {
        for program in &descriptor.programs {
            let mut pattern: HypotheticalRoots<'_> = facts.programs.clone();
            pattern.get_mut(&program.id).unwrap().pattern = "^hypothetical-wrong-binding$";
            let substituted = hypothetical_facts(
                facts.binding,
                &pattern,
                facts.constructor_layouts,
                facts.linked_inline,
            );
            assert_eq!(
                descriptor.evaluate(&substituted),
                Err(Refusal::TargetLayout(program.id.clone()))
            );
            let mut roots: HypotheticalRoots<'_> = facts.programs.clone();
            let payload = &mut roots.get_mut(&program.id).unwrap().metadata.props_payload;
            payload.bytes = payload.bytes.checked_add(payload.alignment).unwrap();
            let mismatched = hypothetical_facts(
                facts.binding,
                &roots,
                facts.constructor_layouts,
                facts.linked_inline,
            );
            assert_eq!(
                descriptor.evaluate(&mismatched),
                Err(Refusal::TargetBinding)
            );
            let mut nfa = facts.programs[&program.id].nfa_facts.clone();
            let inner = nfa.get_mut(NFA_INNER).unwrap();
            inner.bytes = inner.bytes.checked_add(inner.alignment).unwrap();
            let mut root_map: HypotheticalRoots<'_> = facts.programs.clone();
            root_map.get_mut(&program.id).unwrap().nfa_facts = &nfa;
            let different_nfa = hypothetical_facts(
                facts.binding,
                &root_map,
                facts.constructor_layouts,
                facts.linked_inline,
            );
            assert_eq!(
                descriptor.evaluate(&different_nfa),
                Err(Refusal::TargetBinding)
            );
        }
    }

    fn assert_overflow_axes(
        descriptor: &ActualSourceDescriptor,
        facts: &RequestedFacts<'_>,
        heaps: &[NativeProgramHeap],
        mut certificates: Vec<program_construction::Certificate>,
    ) {
        for id in facts.linked_inline.keys() {
            let mut linked = hypothetical_linked(descriptor, facts.programs);
            linked.get_mut(id).unwrap().attributed_bytes = u64::MAX;
            let overflow = hypothetical_facts(
                facts.binding,
                facts.programs,
                facts.constructor_layouts,
                &linked,
            );
            assert_eq!(
                descriptor.reconcile_groups(&overflow, heaps),
                Err(Refusal::Arithmetic)
            );
        }
        let mut roots: HypotheticalRoots<'_> = facts.programs.clone();
        // Preserve 14's common-layout agreement. Numeric request validation
        // precedes linked reconciliation, whose hypothetical facts stay fixed.
        for root in roots.values_mut() {
            root.metadata.static_inline = NativeField {
                bytes: usize::MAX,
                alignment: 1_usize,
            };
        }
        let request = hypothetical_facts(
            facts.binding,
            &roots,
            facts.constructor_layouts,
            facts.linked_inline,
        );
        assert_eq!(descriptor.evaluate(&request), Err(Refusal::Arithmetic));
        let mut retained_overflow = heaps.to_vec();
        retained_overflow.first_mut().unwrap().total = usize::MAX;
        assert_eq!(
            compose(&descriptor.programs, &retained_overflow, &certificates),
            Err(Refusal::Arithmetic)
        );
        let first = certificates.first_mut().unwrap();
        assert!(
            first
                .phase_scratch
                .insert(Stage::Syntax, usize::MAX)
                .is_some()
        );
        first.construction_only_bytes = usize::MAX;
        assert_eq!(
            compose(&descriptor.programs, heaps, &certificates),
            Err(Refusal::Arithmetic)
        );
        // Keep every trace peak representable, but overflow final C + max I.
        // There are at least two positive retained owners, so final C > C0.
        let without_first = usize::MAX
            .checked_sub(heaps.first().unwrap().total)
            .unwrap();
        let additive = certificates.first_mut().unwrap();
        assert!(
            additive
                .phase_scratch
                .insert(Stage::Syntax, without_first)
                .is_some()
        );
        additive.construction_only_bytes = without_first;
        assert_eq!(
            compose(&descriptor.programs, heaps, &certificates),
            Err(Refusal::Arithmetic)
        );
    }

    #[test]
    fn recipe_binds_each_consumed_component_and_source_join() {
        let expected = [
            "schema_residency/descriptor.rs",
            "schema_residency/runtime_sources.rs",
            "schema_residency/expanded.rs",
            "schema_residency/auxiliary.rs",
            "schema_residency/options.rs",
            "schema_residency/storage.rs",
            "schema_residency/retained.rs",
            "schema_residency/initialization.rs",
            "schema_residency/program_construction.rs",
            "schema_residency/literal_construction.rs",
            "schema_program_heap/mod.rs",
            "schema_metadata_heap/mod.rs",
            "schema_nfa_heap/mod.rs",
            "schema_nfa_heap/target.rs",
            "schema_onepass_bounds/mod.rs",
            "schema_literal_heap/mod.rs",
            "schema_pool_heap/mod.rs",
            "schema_heap_layout/mod.rs",
            "schema_cache_model/mod.rs",
            "schema_hybrid_prefix/mod.rs",
            "schema_syntax_construction/mod.rs",
            "schema_nfa_construction/mod.rs",
            "schema_onepass_construction/mod.rs",
            "schema_build/application.rs",
            "schema_build/regex_dependencies.rs",
            "schema_build/schema_regex_vendor/mod.rs",
            "schema_vendor/mod.rs",
        ]
        .map(|path| format!("crates/auctioneer-bidder/tests/{path}"))
        .into_iter()
        .collect::<BTreeSet<_>>();
        let source_witnesses = model_sources();
        let witnesses = source_witnesses.as_object().unwrap();
        assert_eq!(witnesses.keys().cloned().collect::<BTreeSet<_>>(), expected);
        for witness in witnesses.values() {
            assert_eq!(
                witness
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from(["bytes", "sha256"])
            );
            assert!(witness["bytes"].as_u64().unwrap() > 0);
            let digest = witness["sha256"].as_str().unwrap();
            assert_eq!(digest.len(), 64);
            assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
    }

    fn graph() -> Value {
        json!({"declarations":[
            {"owner":"fake_b","marker":"MarkerB","schema_state":[],"callbacks":[],"programs":[
                {"identity":"fake_b::MarkerB::FIRST_REGEX","symbol":"FIRST_REGEX","pattern":"^[0-9]+$"}],
                "initialization":["FIRST_REGEX"]},
            {"owner":"fake_a","marker":"MarkerA","schema_state":[],"callbacks":["hostname","uuid"],"programs":[
                {"identity":"fake_a::MarkerA::FIRST_REGEX","symbol":"FIRST_REGEX","pattern":"^[0-9]+$"},
                {"identity":"fake_a::MarkerA::SECOND_REGEX","symbol":"SECOND_REGEX","pattern":"^y$"}],
                "initialization":["SECOND_REGEX","FIRST_REGEX"]}],
            "auxiliary":{"status":"Complete","roots":[
                {"kind":"inline-retention-row","details":{"entry":"STAGED_VALIDATOR_1","marker":"MarkerB","owner":"fake_b"}},
                {"kind":"inline-retention-row","details":{"entry":"STAGED_VALIDATOR_0","marker":"MarkerA","owner":"fake_a"}}],
                "dispatch_edges":[
                    {"from":"initialize_native","to":"MarkerA","role":"initialize-regexes","ownership":"callee-owned"},
                    {"from":"initialize_native","to":"MarkerB","role":"initialize-regexes","ownership":"callee-owned"}]}})
    }

    #[test]
    fn auxiliary_numeric_schedule_and_per_row_order_keep_equal_patterns_distinct() {
        let graph = graph();
        let rows = ordered_declarations(&graph).unwrap();
        let ids = rows
            .into_iter()
            .flat_map(|row| row_programs(row).unwrap())
            .map(|(row, _)| text(row, "identity").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "fake_a::MarkerA::SECOND_REGEX",
                "fake_a::MarkerA::FIRST_REGEX",
                "fake_b::MarkerB::FIRST_REGEX"
            ]
        );
    }

    #[test]
    fn every_missing_duplicate_or_extra_program_and_initializer_refuses() {
        for initialization in [
            json!([]),
            json!(["FIRST_REGEX", "FIRST_REGEX"]),
            json!(["FIRST_REGEX", "UNKNOWN_REGEX"]),
        ] {
            let mut graph = graph();
            graph["declarations"][0]["initialization"] = initialization;
            assert_eq!(ordered_declarations(&graph).err(), Some(Refusal::Coverage));
        }
        for duplicate in [false, true] {
            let mut graph = graph();
            let programs = graph["declarations"][0]["programs"].as_array_mut().unwrap();
            assert_eq!(programs.len(), 1_usize);
            if duplicate {
                programs.push(programs[0].clone());
            } else {
                programs.clear();
            }
            assert_eq!(ordered_declarations(&graph).err(), Some(Refusal::Coverage));
        }
    }

    #[test]
    fn schedule_requires_exact_header_and_refuses_nonempty_state_instead_of_zero() {
        for pointer in ["/auxiliary/roots", "/auxiliary/dispatch_edges"] {
            for duplicate in [false, true] {
                let mut graph = graph();
                let rows = graph.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
                assert_eq!(rows.len(), 2_usize);
                if duplicate {
                    rows.push(rows[0].clone());
                } else {
                    rows.pop().unwrap();
                }
                assert_eq!(ordered_declarations(&graph).err(), Some(Refusal::Coverage));
            }
        }
        let mut graph = graph();
        graph["declarations"][0]["schema_state"] = json!([{"kind":"lazy_json_array"}]);
        assert_eq!(
            ordered_declarations(&graph).err(),
            Some(Refusal::UnsupportedState("fake_b".into()))
        );
    }

    #[test]
    fn validation_handoff_rejects_each_missing_duplicate_and_misowned_child() {
        let expected = [
            ValidationAssignment {
                child: "fake::cache".into(),
                source_owner: "fake".into(),
                role: "cache".into(),
            },
            ValidationAssignment {
                child: "hostname::per-call-cache".into(),
                source_owner: "hostname".into(),
                role: "cache".into(),
            },
            ValidationAssignment {
                child: "fake::scratch".into(),
                source_owner: "fake".into(),
                role: "mutable-scratch".into(),
            },
        ];
        assert_eq!(exact_assignments(&expected, &expected), Ok(()));
        for index in 0_usize..expected.len() {
            let mut missing = expected.to_vec();
            missing.remove(index);
            assert_eq!(
                exact_assignments(&expected, &missing),
                Err(Refusal::Coverage)
            );
            let mut duplicate = expected.to_vec();
            duplicate.push(expected[index].clone());
            assert_eq!(
                exact_assignments(&expected, &duplicate),
                Err(Refusal::Coverage)
            );
            let mut wrong = expected.to_vec();
            wrong[index].source_owner = "other".into();
            assert_eq!(exact_assignments(&expected, &wrong), Err(Refusal::Coverage));
        }
    }

    #[test]
    fn validation_handoff_assigns_actual_program_declaration_and_format_children_once() {
        let graph = graph();
        let ids = [runtime_sources::HOSTNAME, "fake_a::MarkerA::FIRST_REGEX"];
        let assignments = validation_assignments(&ids, &graph).unwrap();
        let expected = [
            "schema::staging::formats::HOSTNAME_PATTERN::per-call-cache",
            "schema::staging::formats::HOSTNAME_PATTERN::validation::mutable-scratch",
            "schema::staging::formats::HOSTNAME_PATTERN::validation::search-fallback",
            "fake_a::MarkerA::FIRST_REGEX::validation::cache",
            "fake_a::MarkerA::FIRST_REGEX::validation::mutable-scratch",
            "fake_a::MarkerA::FIRST_REGEX::validation::search-fallback",
            "fake_b::MarkerB::declaration-stack",
            "fake_b::MarkerB::declaration-work",
            "fake_a::MarkerA::declaration-stack",
            "fake_a::MarkerA::declaration-work",
            "schema::staging::formats::hostname::callback-stack",
            "schema::staging::formats::hostname::callback-work",
            "schema::staging::formats::uuid::callback-stack",
            "schema::staging::formats::uuid::callback-work",
        ];
        assert_eq!(
            assignments
                .iter()
                .map(|row| row.child.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        for index in 0_usize..assignments.len() {
            let mut missing = assignments.clone();
            missing.remove(index);
            assert_eq!(
                exact_assignments(&assignments, &missing),
                Err(Refusal::Coverage)
            );
            let mut duplicate = assignments.clone();
            duplicate.push(assignments[index].clone());
            assert_eq!(
                exact_assignments(&assignments, &duplicate),
                Err(Refusal::Coverage)
            );
        }
    }

    #[test]
    fn missing_target_rows_are_target_incomplete_and_extras_cannot_substitute() {
        let expected = BTreeSet::from(["fake_a", "fake_b"]);
        let mut missing = Vec::new();
        exact_keys(&expected, ["fake_b"].into_iter(), "program", &mut missing).unwrap();
        assert_eq!(missing, ["program::fake_a"]);
        assert_eq!(
            exact_keys(
                &expected,
                ["fake_b", "other"].into_iter(),
                "program",
                &mut missing
            ),
            Err(Refusal::TargetBinding)
        );
    }

    fn constructor_fact_names() -> BTreeSet<String> {
        BTreeSet::from(["fake_ctor::Frame".to_owned(), "fake_ctor::Queue".to_owned()])
    }

    fn nfa_fact_names() -> BTreeSet<String> {
        target::fact_keys().into_iter().map(str::to_owned).collect()
    }

    fn inspect_fact_names(
        nfa: &BTreeSet<String>,
        constructor: &BTreeSet<String>,
    ) -> Result<Vec<String>, Refusal> {
        let mut missing = Vec::new();
        program_fact_keys(
            "fake_owner",
            &constructor_fact_names(),
            Some(nfa.iter().map(String::as_str)),
            Some(constructor.iter().map(String::as_str)),
            &mut missing,
        )?;
        Ok(missing)
    }

    #[test]
    fn every_inner_nfa_key_is_required_and_any_extra_refuses() {
        let nfa = nfa_fact_names();
        let constructor = constructor_fact_names();
        assert_eq!(nfa.len(), 11_usize);
        assert_eq!(inspect_fact_names(&nfa, &constructor), Ok(Vec::new()));
        for key in &nfa {
            let mut missing = nfa.clone();
            assert!(missing.remove(key));
            assert_eq!(
                inspect_fact_names(&missing, &constructor),
                Ok(vec![format!("program::fake_owner::nfa::{key}")])
            );
        }
        let mut extra = nfa.clone();
        assert!(extra.insert("unexpected::NfaFact".to_owned()));
        assert_eq!(
            inspect_fact_names(&extra, &constructor),
            Err(Refusal::TargetBinding)
        );
    }

    #[test]
    fn every_inner_constructor_key_is_required_and_any_extra_refuses() {
        let nfa = nfa_fact_names();
        let constructor = constructor_fact_names();
        assert_eq!(constructor.len(), 2_usize);
        assert_eq!(inspect_fact_names(&nfa, &constructor), Ok(Vec::new()));
        for key in &constructor {
            let mut missing = constructor.clone();
            assert!(missing.remove(key));
            assert_eq!(
                inspect_fact_names(&nfa, &missing),
                Ok(vec![format!("constructor::fake_owner::{key}")])
            );
        }
        let mut extra = constructor.clone();
        assert!(extra.insert("unexpected::ConstructorFact".to_owned()));
        assert_eq!(
            inspect_fact_names(&nfa, &extra),
            Err(Refusal::TargetBinding)
        );
    }

    #[test]
    fn equal_cardinality_substitutions_and_cross_family_holes_cannot_hide_extras() {
        let nfa = nfa_fact_names();
        let constructor = constructor_fact_names();
        for key in &nfa {
            let mut substituted = nfa.clone();
            assert!(substituted.remove(key));
            assert!(substituted.insert("unexpected::NfaFact".to_owned()));
            assert_eq!(substituted.len(), 11_usize);
            assert_eq!(
                inspect_fact_names(&substituted, &constructor),
                Err(Refusal::TargetBinding)
            );
        }
        for key in &constructor {
            let mut substituted = constructor.clone();
            assert!(substituted.remove(key));
            assert!(substituted.insert("unexpected::ConstructorFact".to_owned()));
            assert_eq!(substituted.len(), 2_usize);
            assert_eq!(
                inspect_fact_names(&nfa, &substituted),
                Err(Refusal::TargetBinding)
            );
        }
        let mut nfa_extra = nfa.clone();
        assert!(nfa_extra.insert("unexpected::NfaFact".to_owned()));
        let mut constructor_hole = constructor.clone();
        assert!(constructor_hole.remove("fake_ctor::Frame"));
        assert_eq!(
            inspect_fact_names(&nfa_extra, &constructor_hole),
            Err(Refusal::TargetBinding)
        );

        let mut nfa_hole = nfa.clone();
        assert!(nfa_hole.remove("compiler_abi::Pointer"));
        let mut constructor_extra = constructor;
        assert!(constructor_extra.insert("unexpected::ConstructorFact".to_owned()));
        assert_eq!(
            inspect_fact_names(&nfa_hole, &constructor_extra),
            Err(Refusal::TargetBinding)
        );
    }

    // These synthetic premises exercise agreement only. No numeric constructor
    // or target/physical certificate is evaluated in these controls.
    fn common_constructor_facts() -> BTreeMap<String, NativeField> {
        let mut facts = program_construction::required_fact_keys()
            .into_iter()
            .map(|name| {
                (
                    name,
                    NativeField {
                        bytes: 32_usize,
                        alignment: 8_usize,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        facts.insert(
            target::ABI_POINTER.to_owned(),
            NativeField {
                bytes: 8_usize,
                alignment: 8_usize,
            },
        );
        facts
    }

    fn common_nfa_facts(
        constructor: &BTreeMap<String, NativeField>,
    ) -> BTreeMap<String, NativeField> {
        target::fact_keys()
            .into_iter()
            .map(|name| {
                (
                    name.to_owned(),
                    *constructor.get(name).expect("complete constructor keys"),
                )
            })
            .collect()
    }

    fn common_program<'a>(
        pattern: &'a str,
        nfa: &'a BTreeMap<String, NativeField>,
    ) -> ProgramRecipe<'a> {
        use crate::schema_metadata_heap::MetadataLayout;
        use crate::schema_pool_heap::PoolLayout;

        let field = NativeField {
            bytes: 32_usize,
            alignment: 8_usize,
        };
        ProgramRecipe {
            pattern,
            metadata: MetadataLayout {
                regex_fields: [field; 2_usize],
                info_fields: [field; 3_usize],
                props_payload: field,
                static_inline: field,
            },
            arc_header: NativeField {
                bytes: 16_usize,
                alignment: 8_usize,
            },
            strategy_payload: field,
            nfa_facts: nfa,
            onepass_word: NativeField {
                bytes: 8_usize,
                alignment: 8_usize,
            },
            memchr_payload: NativeField {
                bytes: 1_usize,
                alignment: 1_usize,
            },
            pool: PoolLayout {
                fields: [field; 4_usize],
                shard: field,
                closure: field,
            },
        }
    }

    fn check_two_root_layouts(
        first: &ProgramRecipe<'_>,
        second: &ProgramRecipe<'_>,
        first_constructor: &BTreeMap<String, NativeField>,
        second_constructor: &BTreeMap<String, NativeField>,
    ) -> Result<(), Refusal> {
        let programs = BTreeMap::from([
            ("fake_a".to_owned(), *first),
            ("fake_b".to_owned(), *second),
        ]);
        let constructors = BTreeMap::from([
            ("fake_a".to_owned(), first_constructor.clone()),
            ("fake_b".to_owned(), second_constructor.clone()),
        ]);
        common_target_layouts(&programs, &constructors)
    }

    fn changed_layout(field: NativeField, change_bytes: bool) -> NativeField {
        if change_bytes {
            NativeField {
                bytes: field
                    .bytes
                    .checked_mul(2_usize)
                    .expect("small fixture bytes"),
                ..field
            }
        } else {
            NativeField {
                alignment: field
                    .alignment
                    .checked_mul(2_usize)
                    .expect("small fixture alignment"),
                ..field
            }
        }
    }

    fn fixed_program_fields<'a>(program: &'a mut ProgramRecipe<'_>) -> Vec<&'a mut NativeField> {
        program
            .metadata
            .regex_fields
            .iter_mut()
            .chain(program.metadata.info_fields.iter_mut())
            .chain([
                &mut program.metadata.props_payload,
                &mut program.metadata.static_inline,
                &mut program.arc_header,
                &mut program.onepass_word,
                &mut program.memchr_payload,
            ])
            .chain(program.pool.fields.iter_mut())
            .chain([&mut program.pool.shard, &mut program.pool.closure])
            .collect()
    }

    fn change_fixed_program_field(
        program: &mut ProgramRecipe<'_>,
        index: usize,
        change_bytes: bool,
    ) {
        let mut fields = fixed_program_fields(program);
        let field = fields.get_mut(index).expect("fixed field present");
        let before = **field;
        **field = changed_layout(before, change_bytes);
        assert_ne!(**field, before);
    }

    #[test]
    fn constructor_type_layouts_agree_across_roots_in_bytes_and_alignment() {
        let constructor = common_constructor_facts();
        let nfa = common_nfa_facts(&constructor);
        let first = common_program("^[0-9]+$", &nfa);
        assert_eq!(
            check_two_root_layouts(&first, &first, &constructor, &constructor),
            Ok(())
        );
        assert!(!constructor.is_empty());
        for (name, field) in &constructor {
            for change_bytes in [true, false] {
                let mut changed = constructor.clone();
                let previous = changed.insert(name.clone(), changed_layout(*field, change_bytes));
                assert_eq!(previous, Some(*field));
                assert_ne!(changed.get(name), Some(field));
                assert_eq!(changed.len(), constructor.len());
                let second_nfa = common_nfa_facts(&changed);
                let second = common_program("^[0-9]+$", &second_nfa);
                assert_eq!(
                    check_two_root_layouts(&first, &second, &constructor, &changed),
                    Err(Refusal::TargetBinding),
                    "{name}: bytes={change_bytes}"
                );
                assert_eq!(
                    check_two_root_layouts(&second, &first, &changed, &constructor),
                    Err(Refusal::TargetBinding),
                    "reverse {name}: bytes={change_bytes}"
                );
            }
        }
    }

    #[test]
    fn nfa_facts_reconcile_with_common_constructor_types_for_each_root() {
        let constructor = common_constructor_facts();
        let nfa = common_nfa_facts(&constructor);
        let first = common_program("^[0-9]+$", &nfa);
        assert_eq!(nfa.len(), 11_usize);
        assert_eq!(
            check_two_root_layouts(&first, &first, &constructor, &constructor),
            Ok(())
        );
        for (name, field) in &nfa {
            for change_bytes in [true, false] {
                let mut changed = nfa.clone();
                assert_eq!(
                    changed.insert(name.clone(), changed_layout(*field, change_bytes)),
                    Some(*field)
                );
                let second = common_program("^[0-9]+$", &changed);
                assert_eq!(
                    check_two_root_layouts(&first, &second, &constructor, &constructor),
                    Err(Refusal::TargetBinding),
                    "{name}: bytes={change_bytes}"
                );
                assert_eq!(
                    check_two_root_layouts(&second, &first, &constructor, &constructor),
                    Err(Refusal::TargetBinding),
                    "reverse {name}: bytes={change_bytes}"
                );
            }
        }
    }

    #[test]
    fn every_fixed_program_layout_agrees_without_freezing_pattern_strategy_payloads() {
        let constructor = common_constructor_facts();
        let nfa = common_nfa_facts(&constructor);
        let first = common_program("^[0-9]+$", &nfa);
        let mut probe = first;
        assert_eq!(fixed_program_fields(&mut probe).len(), 16_usize);
        assert_eq!(
            check_two_root_layouts(&first, &first, &constructor, &constructor),
            Ok(())
        );
        for index in 0_usize..16_usize {
            for change_bytes in [true, false] {
                let mut second = first;
                change_fixed_program_field(&mut second, index, change_bytes);
                assert_eq!(
                    check_two_root_layouts(&first, &second, &constructor, &constructor),
                    Err(Refusal::TargetBinding),
                    "field {index}: bytes={change_bytes}"
                );
                assert_eq!(
                    check_two_root_layouts(&second, &first, &constructor, &constructor),
                    Err(Refusal::TargetBinding),
                    "reverse field {index}: bytes={change_bytes}"
                );
            }
        }
        let mut selected = common_program(r"\.[a-zA-Z]{2,3}$", &nfa);
        selected.strategy_payload = NativeField {
            bytes: 64_usize,
            alignment: 16_usize,
        };
        assert_ne!(selected.pattern, first.pattern);
        assert_ne!(selected.strategy_payload, first.strategy_payload);
        assert_eq!(
            check_two_root_layouts(&first, &selected, &constructor, &constructor),
            Ok(())
        );
    }

    #[test]
    fn one_abi_cannot_combine_a_four_byte_and_eight_byte_program_word() {
        let wide_constructor = common_constructor_facts();
        let wide_nfa = common_nfa_facts(&wide_constructor);
        let wide = common_program("^[0-9]+$", &wide_nfa);
        let mut narrow_constructor = wide_constructor.clone();
        let narrow_word = NativeField {
            bytes: 4_usize,
            alignment: 4_usize,
        };
        assert_eq!(
            narrow_constructor.insert(target::ABI_POINTER.to_owned(), narrow_word),
            Some(NativeField {
                bytes: 8_usize,
                alignment: 8_usize
            })
        );
        let narrow_nfa = common_nfa_facts(&narrow_constructor);
        let mut narrow = common_program("^[0-9]+$", &narrow_nfa);
        narrow.onepass_word = narrow_word;
        narrow.arc_header = NativeField {
            bytes: 8_usize,
            alignment: 4_usize,
        };
        assert_eq!(
            check_two_root_layouts(&wide, &narrow, &wide_constructor, &narrow_constructor),
            Err(Refusal::TargetBinding)
        );
    }
}

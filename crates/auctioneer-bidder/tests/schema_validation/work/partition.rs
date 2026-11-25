//! Traversal/affine caller-bound DATA, not source authority.
//!
//! Coefficients count existing helper entries, iterator callbacks and top-level
//! regex invocations. They are not new work units or a closed `WorkLedger`. Actual
//! use must obtain this same canonical `Recipe` from the sealed body handoff and
//! bind the node cap to the existing caller context. Fixture caps are not
//! installed limits. An origin spelling alone is never a partition witness.
//! Ordinary primitive, cleanup, byte, frame and target obligations stay open.
//! The old coarse counter is neither changed nor exposed as a fallback here.

use crate::schema_validation::{
    operations::{
        self, Primitive,
        model::{
            Action, Arm, Atom, Binding, Comparison, Function, IterationBound, Kind, Op, Origin,
            Pattern, Recipe, Site, StopRule, Ty,
        },
    },
    storage::{OwnerSchedule, owner_schedule_data},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::iter::once;
use std::slice::from_ref;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NodeCapData {
    nodes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceBoundCoeff {
    constant: u64,
    nodes: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SourceRequirement {
    CanonicalRecipe,
    BorrowedInputTree,
    FormalActualBindings,
    IteratorMemberProtocol,
    CallerNodeCap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallBound {
    site: Site,
    target: usize,
    formal_ordinal: usize,
    formal: Binding,
    actual: Binding,
    per_attempt: SourceBoundCoeff,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceBounds {
    roots: Vec<usize>,
    cap: NodeCapData,
    attempts: u64,
    helper_calls: Vec<CallBound>,
    iterator_callbacks_per_attempt: SourceBoundCoeff,
    required_sources: BTreeSet<SourceRequirement>,
}

// Supplied invocation ceilings, not an actual caller-population witness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct CoupledPopulation {
    pub factories: u64,
    pub validations: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum CoupledDuty {
    CallerPopulations,
    SourceBinding,
    ValidationTraversal,
    SearchMultiplicity,
    PrimitiveSourceCalls,
    OrdinaryWork,
    OrdinaryCleanup,
    PartialConstructionCleanup,
}

#[derive(Debug, Eq, PartialEq)]
struct ConstantHelperBound {
    site: Site,
    target: usize,
    upper: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct OptionalCallbackBound {
    site: Site,
    minimum: u64,
    maximum: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct ProgramFactoryBound {
    field: String,
    program: String,
    initializer_probe: Site,
    constructor: Site,
    completed_siblings: Vec<String>,
    initializer_checks_upper: u64,
    constructor_calls_upper: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct ConstantFactoryBounds {
    root: usize,
    entries_upper: u64,
    helpers: Vec<ConstantHelperBound>,
    wrapper_map: OptionalCallbackBound,
    // These are initializer probes, not all Lazy/Option helper work and not
    // immutable regex reconstructions. Field as_ref setup remains a duty.
    initializer_checks_upper: u64,
    programs: Vec<ProgramFactoryBound>,
    completed_prefixes: Vec<Vec<String>>,
}

/// Narrow factory/entry DATA only, never success for the whole input Recipe.
/// Both original roots and initialization requirements must remain present.
/// A missing input projection is not a zero traversal, search or helper cost.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct CoupledFactoryBounds {
    roots: Vec<usize>,
    initialization: BTreeMap<String, Vec<String>>,
    population: CoupledPopulation,
    node_cap: u64,
    root_entries_upper: u64,
    validation_root: usize,
    factory: ConstantFactoryBounds,
    input_partition: Option<SourceBounds>,
    required: BTreeSet<CoupledDuty>,
}

#[derive(Debug, Eq, PartialEq)]
struct SearchSiteBound {
    site: Site,
    per_validation: SourceBoundCoeff,
    per_validation_upper: u64,
    invocations_upper: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct ProgramSearchBound {
    program: String,
    field: String,
    sites: Vec<SearchSiteBound>,
    per_validation_upper: u64,
    invocations_upper: u64,
}

// Invocation DATA only: haystack bytes and engine work remain separate duties.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct CoupledSearchBounds {
    input: CoupledFactoryBounds,
    programs: Vec<ProgramSearchBound>,
    per_validation_upper: u64,
    invocations_upper: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum CoupledRefusal {
    RootBinding,
    OwnerBinding,
    Initialization,
    Initializer,
    SourceCalls(Site),
    LocalBinding(Site),
    CallBinding(Site),
    IterationBinding(Site),
    SourceRefusal {
        function: Option<String>,
        site: Site,
        cause: operations::Refusal,
    },
    Overflow,
}

// The storage owner checks factory/transfer structure. This consumer projects
// its source sites and separately matches the initializer's executable probes.
// Neither supplied populations nor this narrow result closes input/work duties.
pub(in crate::schema_validation) fn coupled_factory_bounds(
    recipe: &Recipe,
    graph: &Value,
    owner: &str,
    marker: &str,
    node_cap: u64,
    population: CoupledPopulation,
) -> Result<CoupledFactoryBounds, CoupledRefusal> {
    let (factory_root, validation_root) = coupled_roots(recipe, marker)?;
    let (initializer, programs) = coupled_initialization(recipe, marker)?;
    coupled_source_calls(recipe)?;
    let schedule = owner_schedule_data(recipe, graph, owner, marker)
        .map_err(|_error| CoupledRefusal::OwnerBinding)?;
    if schedule.roots() != (factory_root, validation_root) {
        return Err(CoupledRefusal::OwnerBinding);
    }
    let projection = factory_projection(recipe, factory_root, initializer)?;
    let probes = initializer_probes(recipe, initializer, programs)?;
    let program_sites = factory_program_sites(&schedule, &probes)?;

    // All source checks precede even zero-population arithmetic. F and V are
    // independent supplied ceilings; no caller-lifetime relation is proved.
    let root_entries_upper = population
        .factories
        .checked_add(population.validations)
        .ok_or(CoupledRefusal::Overflow)?;
    let probe_count = u64::try_from(probes.len()).map_err(|_error| CoupledRefusal::Overflow)?;
    let initializer_checks_upper = population
        .factories
        .checked_mul(probe_count)
        .ok_or(CoupledRefusal::Overflow)?;
    let completed_prefixes = program_sites
        .iter()
        .map(|row| row.completed_siblings.clone())
        .chain(once(
            program_sites.iter().map(|row| row.field.clone()).collect(),
        ))
        .collect();
    let bounded_programs = program_sites
        .into_iter()
        .map(|row| ProgramFactoryBound {
            field: row.field,
            program: row.program,
            initializer_probe: row.initializer_probe,
            constructor: row.constructor,
            completed_siblings: row.completed_siblings,
            initializer_checks_upper: population.factories,
            constructor_calls_upper: population.factories,
        })
        .collect();
    Ok(CoupledFactoryBounds {
        roots: recipe.roots.clone(),
        initialization: recipe.initialization.clone(),
        population,
        node_cap,
        root_entries_upper,
        validation_root,
        factory: ConstantFactoryBounds {
            root: factory_root,
            entries_upper: population.factories,
            helpers: projection
                .helpers
                .into_iter()
                .map(|(site, target)| ConstantHelperBound {
                    site,
                    target,
                    upper: population.factories,
                })
                .collect(),
            wrapper_map: OptionalCallbackBound {
                site: projection.wrapper_map,
                minimum: 0,
                maximum: population.factories,
            },
            initializer_checks_upper,
            programs: bounded_programs,
            completed_prefixes,
        },
        input_partition: None,
        required: BTreeSet::from([
            CoupledDuty::CallerPopulations,
            CoupledDuty::SourceBinding,
            CoupledDuty::ValidationTraversal,
            CoupledDuty::SearchMultiplicity,
            CoupledDuty::PrimitiveSourceCalls,
            CoupledDuty::OrdinaryWork,
            CoupledDuty::OrdinaryCleanup,
            CoupledDuty::PartialConstructionCleanup,
        ]),
    })
}

// The factory owner establishes the selected validation root in this same
// Recipe. Only invocation DATA is projected; both original roots, all program
// requirements and the unresolved ordinary work/cleanup duties remain intact.
pub(in crate::schema_validation) fn coupled_input_bounds(
    recipe: &Recipe,
    graph: &Value,
    owner: &str,
    marker: &str,
    node_cap: u64,
    population: CoupledPopulation,
) -> Result<CoupledFactoryBounds, CoupledRefusal> {
    coupled_partition(recipe, graph, owner, marker, node_cap, population)
        .map(|(input, _searches)| input)
}

// Component projections share this one analysis of the original Recipe. The
// input-only result does not assert representability of search aggregates.
fn coupled_partition(
    recipe: &Recipe,
    graph: &Value,
    owner: &str,
    marker: &str,
    node_cap: u64,
    population: CoupledPopulation,
) -> Result<(CoupledFactoryBounds, Vec<SearchOccurrences>), CoupledRefusal> {
    let mut bounded = coupled_factory_bounds(recipe, graph, owner, marker, node_cap, population)?;
    let analysis = partition_analysis(
        recipe,
        from_ref(&bounded.validation_root),
        NodeCapData { nodes: node_cap },
        population.validations,
        &bounded.factory.programs,
    )
    .map_err(|refusal| coupled_input_refusal(recipe, refusal, bounded.validation_root))?;
    bounded.input_partition = Some(analysis.input);
    Ok((bounded, analysis.searches))
}

pub(in crate::schema_validation) fn coupled_search_bounds(
    recipe: &Recipe,
    graph: &Value,
    owner: &str,
    marker: &str,
    node_cap: u64,
    population: CoupledPopulation,
) -> Result<CoupledSearchBounds, CoupledRefusal> {
    let (input, searches) = coupled_partition(recipe, graph, owner, marker, node_cap, population)?;
    search_projection(recipe, input, searches)
}

fn search_projection(
    recipe: &Recipe,
    input: CoupledFactoryBounds,
    searches: Vec<SearchOccurrences>,
) -> Result<CoupledSearchBounds, CoupledRefusal> {
    let cap = NodeCapData {
        nodes: input.node_cap,
    };
    let validations = input.population.validations;
    let mut programs: Vec<_> = input
        .factory
        .programs
        .iter()
        .map(|program| ProgramSearchBound {
            program: program.program.clone(),
            field: program.field.clone(),
            sites: Vec::new(),
            per_validation_upper: 0,
            invocations_upper: 0,
        })
        .collect();
    for search in searches {
        let row = programs
            .iter_mut()
            .find(|row| row.program == search.program && row.field == search.field)
            .ok_or_else(|| CoupledRefusal::CallBinding(search.site.clone()))?;
        let coefficient = search
            .occurrences
            .coefficient()
            .map_err(|refusal| coupled_input_refusal(recipe, refusal, input.validation_root))?;
        let per_validation_upper = checked_bound(coefficient, cap)
            .map_err(|refusal| coupled_input_refusal(recipe, refusal, input.validation_root))?;
        let invocations_upper = per_validation_upper
            .checked_mul(validations)
            .ok_or(CoupledRefusal::Overflow)?;
        row.per_validation_upper = row
            .per_validation_upper
            .checked_add(per_validation_upper)
            .ok_or(CoupledRefusal::Overflow)?;
        row.sites.push(SearchSiteBound {
            site: search.site,
            per_validation: coefficient,
            per_validation_upper,
            invocations_upper,
        });
    }
    let mut per_validation_upper = 0_u64;
    for program in &mut programs {
        program.invocations_upper = program
            .per_validation_upper
            .checked_mul(validations)
            .ok_or(CoupledRefusal::Overflow)?;
        per_validation_upper = per_validation_upper
            .checked_add(program.per_validation_upper)
            .ok_or(CoupledRefusal::Overflow)?;
    }
    // Even V=0 must retain per-program and whole-projection overflow checks.
    let invocations_upper = per_validation_upper
        .checked_mul(validations)
        .ok_or(CoupledRefusal::Overflow)?;
    Ok(CoupledSearchBounds {
        input,
        programs,
        per_validation_upper,
        invocations_upper,
    })
}

fn coupled_input_refusal(recipe: &Recipe, refusal: Refusal, root: usize) -> CoupledRefusal {
    match refusal {
        Refusal::LocalBinding(site) => CoupledRefusal::LocalBinding(site),
        Refusal::CallBinding(site) => CoupledRefusal::CallBinding(site),
        Refusal::IterationBinding(site) => CoupledRefusal::IterationBinding(site),
        Refusal::AtSource { site, cause } => CoupledRefusal::SourceRefusal {
            function: recipe
                .functions
                .get(&site.function)
                .map(|function| function.identity.clone()),
            site,
            cause,
        },
        Refusal::Source(cause) => CoupledRefusal::SourceRefusal {
            function: recipe
                .functions
                .get(&root)
                .map(|function| function.identity.clone()),
            site: Site {
                function: root,
                lexical: Vec::new(),
            },
            cause,
        },
        Refusal::Overflow => CoupledRefusal::Overflow,
    }
}

fn coupled_function(recipe: &Recipe, identity: &str) -> Option<usize> {
    let mut found = recipe
        .functions
        .iter()
        .filter(|(_, function)| function.identity == identity);
    let (id, _) = found.next()?;
    found.next().is_none().then_some(*id)
}

fn coupled_roots(recipe: &Recipe, marker: &str) -> Result<(usize, usize), CoupledRefusal> {
    let factory = coupled_function(recipe, &format!("{marker}::new_search_cache"))
        .ok_or(CoupledRefusal::RootBinding)?;
    let validation = coupled_function(recipe, &format!("{marker}::is_valid"))
        .ok_or(CoupledRefusal::RootBinding)?;
    if recipe.roots.len() != 2
        || recipe.roots.iter().copied().collect::<BTreeSet<_>>()
            != BTreeSet::from([factory, validation])
    {
        return Err(CoupledRefusal::RootBinding);
    }
    Ok((factory, validation))
}

fn coupled_initialization<'a>(
    recipe: &'a Recipe,
    marker: &str,
) -> Result<(usize, &'a [String]), CoupledRefusal> {
    let identity = format!("__{}_impl::initialize_regexes", marker.to_lowercase());
    let initializer = coupled_function(recipe, &identity).ok_or(CoupledRefusal::Initialization)?;
    let programs = recipe
        .initialization
        .get(&identity)
        .ok_or(CoupledRefusal::Initialization)?;
    if recipe.initialization.len() != 1
        || programs.iter().any(String::is_empty)
        || programs.iter().collect::<BTreeSet<_>>().len() != programs.len()
    {
        return Err(CoupledRefusal::Initialization);
    }
    Ok((initializer, programs))
}

fn coupled_source_calls(recipe: &Recipe) -> Result<(), CoupledRefusal> {
    for function in recipe.functions.values() {
        let mut first = None;
        operations::model::walk(&function.body, &mut |op| {
            if first.is_none()
                && let Kind::Primitive { source_calls, .. } = &op.kind
                && !source_calls.is_empty()
            {
                first = Some(op.site.clone());
            }
            Ok(())
        })
        .map_err(|_error| CoupledRefusal::OwnerBinding)?;
        if let Some(site) = first {
            return Err(CoupledRefusal::SourceCalls(site));
        }
    }
    Ok(())
}

struct FactoryProjection {
    helpers: Vec<(Site, usize)>,
    wrapper_map: Site,
}

fn coupled_calls(recipe: &Recipe, id: usize) -> Result<Vec<(Site, usize)>, CoupledRefusal> {
    let function = recipe
        .functions
        .get(&id)
        .ok_or(CoupledRefusal::OwnerBinding)?;
    let mut calls = Vec::new();
    operations::model::walk(&function.body, &mut |op| {
        if let Kind::Call { target, .. } = &op.kind {
            if op.site.function != id || !recipe.functions.contains_key(target) {
                return Err(operations::Refusal::UnclassifiedCall);
            }
            calls.push((op.site.clone(), *target));
        }
        Ok(())
    })
    .map_err(|_error| CoupledRefusal::OwnerBinding)?;
    Ok(calls)
}

// Metadata projection only: wrapper/guard/constructor interpretation remains
// in owner_schedule_data. The accepted wrapper and inner factory each call once.
fn factory_projection(
    recipe: &Recipe,
    root: usize,
    initializer: usize,
) -> Result<FactoryProjection, CoupledRefusal> {
    let outer = coupled_calls(recipe, root)?;
    let [(outer_site, inner)] = outer.as_slice() else {
        return Err(CoupledRefusal::OwnerBinding);
    };
    let calls = coupled_calls(recipe, *inner)?;
    let [(initializer_site, target)] = calls.as_slice() else {
        return Err(CoupledRefusal::OwnerBinding);
    };
    if *target != initializer {
        return Err(CoupledRefusal::OwnerBinding);
    }
    let function = recipe
        .functions
        .get(&root)
        .ok_or(CoupledRefusal::OwnerBinding)?;
    let mut maps = Vec::new();
    operations::model::walk(&function.body, &mut |op| {
        if let Kind::Callback {
            protocol: Primitive::ValueAccess,
            callee,
            ..
        } = &op.kind
            && callee == "Option::map"
        {
            maps.push(op.site.clone());
        }
        Ok(())
    })
    .map_err(|_error| CoupledRefusal::OwnerBinding)?;
    let [wrapper_map] = maps.as_slice() else {
        return Err(CoupledRefusal::OwnerBinding);
    };
    Ok(FactoryProjection {
        helpers: vec![
            (outer_site.clone(), *inner),
            (initializer_site.clone(), *target),
        ],
        wrapper_map: wrapper_map.clone(),
    })
}

fn coupled_child(site: &Site, ordinal: usize) -> Site {
    let mut child = site.clone();
    child.lexical.push(ordinal);
    child
}

fn initializer_parts(function: &Function) -> Result<(&Binding, &[Op]), CoupledRefusal> {
    let Kind::Scope {
        label: None,
        steps,
        locals,
    } = &function.body.kind
    else {
        return Err(CoupledRefusal::Initializer);
    };
    let Some((bind, tail)) = steps.split_first() else {
        return Err(CoupledRefusal::Initializer);
    };
    let Some((returned, checks)) = tail.split_last() else {
        return Err(CoupledRefusal::Initializer);
    };
    let Kind::Bind {
        pattern: Pattern::Binding(name),
        bindings,
        value,
    } = &bind.kind
    else {
        return Err(CoupledRefusal::Initializer);
    };
    let [ready] = bindings.as_slice() else {
        return Err(CoupledRefusal::Initializer);
    };
    let mut expected = value.value.clone();
    expected.place.clone_from(name);
    expected.declaration = Some(bind.site.clone());
    if !function.parameters.is_empty()
        || function.result != Ty::Bool
        || locals != bindings
        || ready != &expected
        || ready.ty != Ty::Bool
        || ready.borrowed
        || bind.value.ty != Ty::Unit
        || !matches!(&value.kind, Kind::Value(Atom::Bool(true)))
        || value.value.origin != Origin::Temporary(value.site.clone())
        || value.value.declaration.is_some()
        || bind.site != coupled_child(&function.body.site, 0)
        || value.site != coupled_child(&bind.site, 0)
        || returned.value != *ready
        || !matches!(&returned.kind, Kind::Value(Atom::Place))
        || returned.site
            != coupled_child(
                &function.body.site,
                steps
                    .len()
                    .checked_sub(1)
                    .ok_or(CoupledRefusal::Initializer)?,
            )
    {
        return Err(CoupledRefusal::Initializer);
    }
    Ok((ready, checks))
}

fn readiness_probe(result: &Op) -> Result<(String, Site), CoupledRefusal> {
    let Kind::Primitive {
        lemma: Primitive::ValueAccess,
        callee,
        receiver: Some(optional),
        arguments,
        source_calls,
    } = &result.kind
    else {
        return Err(CoupledRefusal::Initializer);
    };
    let [probe] = arguments.as_slice() else {
        return Err(CoupledRefusal::Initializer);
    };
    let Kind::Primitive {
        lemma: Primitive::Readiness,
        callee: probe_callee,
        receiver: Some(lazy),
        arguments: inputs,
        source_calls: probe_calls,
    } = &probe.kind
    else {
        return Err(CoupledRefusal::Initializer);
    };
    let [place] = inputs.as_slice() else {
        return Err(CoupledRefusal::Initializer);
    };
    let Ty::LazyRegex(program) = &lazy.ty else {
        return Err(CoupledRefusal::Initializer);
    };
    let mut expected_probe = lazy.clone();
    expected_probe.ty = Ty::Option(Box::new(Ty::Regex));
    let mut expected_result = expected_probe.clone();
    expected_result.ty = Ty::Bool;
    expected_result.borrowed = false;
    if callee != "Option(Regex)::is_some"
        || probe_callee != &format!("{:?}::as_ref", lazy.ty)
        || !source_calls.is_empty()
        || !probe_calls.is_empty()
        || !lazy.borrowed
        || lazy.declaration.is_some()
        || lazy.place != *program
        || lazy.origin != Origin::Static(program.clone())
        || place.value != *lazy
        || !matches!(&place.kind, Kind::Value(Atom::Place))
        || optional != &expected_probe
        || probe.value != expected_probe
        || result.value != expected_result
        || probe.site != coupled_child(&result.site, 0)
        || place.site != coupled_child(&probe.site, 0)
    {
        return Err(CoupledRefusal::Initializer);
    }
    Ok((program.clone(), probe.site.clone()))
}

fn initializer_probes(
    recipe: &Recipe,
    id: usize,
    programs: &[String],
) -> Result<BTreeMap<String, Site>, CoupledRefusal> {
    let function = recipe
        .functions
        .get(&id)
        .ok_or(CoupledRefusal::Initializer)?;
    if function.body.site.function != id {
        return Err(CoupledRefusal::Initializer);
    }
    let (ready, checks) = initializer_parts(function)?;
    if checks.len() != programs.len() {
        return Err(CoupledRefusal::Initializer);
    }
    let mut probes = BTreeMap::new();
    for (ordinal, check) in checks.iter().enumerate() {
        let Kind::Evaluate(Action::AndAssign(target), operands) = &check.kind else {
            return Err(CoupledRefusal::Initializer);
        };
        let [left, right] = operands.as_slice() else {
            return Err(CoupledRefusal::Initializer);
        };
        let index = ordinal.checked_add(1).ok_or(CoupledRefusal::Initializer)?;
        if target != ready
            || left.value != *ready
            || check.value.ty != Ty::Unit
            || !matches!(&left.kind, Kind::Value(Atom::Place))
            || check.site != coupled_child(&function.body.site, index)
            || left.site != coupled_child(&check.site, 0)
            || right.site != coupled_child(&check.site, 1)
        {
            return Err(CoupledRefusal::Initializer);
        }
        let (program, site) = readiness_probe(right)?;
        if !programs.contains(&program) || probes.insert(program, site).is_some() {
            return Err(CoupledRefusal::Initializer);
        }
    }
    // A bijection, not execution in the metadata map's sorted order. Each
    // recorded lexical site preserves its actual eager-probe position.
    Ok(probes)
}

struct FactoryProgramSites {
    field: String,
    program: String,
    initializer_probe: Site,
    constructor: Site,
    completed_siblings: Vec<String>,
}

fn factory_program_sites(
    schedule: &OwnerSchedule,
    probes: &BTreeMap<String, Site>,
) -> Result<Vec<FactoryProgramSites>, CoupledRefusal> {
    schedule
        .initializers()
        .map(|(field, program, constructor, prior)| {
            Ok(FactoryProgramSites {
                field: field.to_owned(),
                program: program.to_owned(),
                initializer_probe: probes
                    .get(program)
                    .ok_or(CoupledRefusal::Initializer)?
                    .clone(),
                constructor: constructor.clone(),
                completed_siblings: prior.to_vec(),
            })
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Source(operations::Refusal),
    AtSource {
        site: Site,
        cause: operations::Refusal,
    },
    LocalBinding(Site),
    IterationBinding(Site),
    CallBinding(Site),
    Overflow,
}

impl From<operations::Refusal> for Refusal {
    fn from(refusal: operations::Refusal) -> Self {
        Self::Source(refusal)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct InputNode {
    root: usize,
    // Each entry is one authenticated child iteration, not an origin label.
    members: Vec<usize>,
}

#[derive(Clone, Eq, PartialEq)]
enum Provenance {
    Input(InputNode),
    Cache(Origin),
    Boolean,
    // Payloads and keys are authenticated borrows, never Value children.
    Scalar,
    Regex(String),
}

type ProvenanceBindings = [(Binding, Provenance)];

struct MemberAxis {
    identity: usize,
    node: InputNode,
    cap: u64,
}

#[derive(Default)]
struct Occurrences {
    constant: u64,
    by_depth: BTreeMap<(usize, usize), u64>,
}

impl Occurrences {
    fn combine(&mut self, other: Self) -> Result<(), Refusal> {
        self.constant = self
            .constant
            .checked_add(other.constant)
            .ok_or(Refusal::Overflow)?;
        for (key, count) in other.by_depth {
            let prior = self.by_depth.entry(key).or_default();
            *prior = prior.checked_add(count).ok_or(Refusal::Overflow)?;
        }
        Ok(())
    }

    fn coefficient(&self) -> Result<SourceBoundCoeff, Refusal> {
        // Distinct depths partition one borrowed tree. Separate source passes
        // at the same depth add; they are never declared disjoint by label.
        let mut roots = BTreeMap::<usize, u64>::new();
        for (&(root, _depth), &count) in &self.by_depth {
            let maximum = roots.entry(root).or_default();
            *maximum = (*maximum).max(count);
        }
        let nodes = roots.values().try_fold(0_u64, |sum, count| {
            sum.checked_add(*count).ok_or(Refusal::Overflow)
        })?;
        Ok(SourceBoundCoeff {
            constant: self.constant,
            nodes,
        })
    }
}

struct CallOccurrences {
    binding: CallBound,
    occurrences: Occurrences,
}

struct SearchOccurrences {
    site: Site,
    program: String,
    field: String,
    occurrences: Occurrences,
}

struct PartitionAnalysis {
    input: SourceBounds,
    searches: Vec<SearchOccurrences>,
}

struct Deriver<'a> {
    recipe: &'a Recipe,
    programs: &'a [ProgramFactoryBound],
    child_cap: u64,
    active_functions: BTreeSet<usize>,
    active_members: Vec<MemberAxis>,
    next_member: usize,
    calls: Vec<CallOccurrences>,
    searches: Vec<SearchOccurrences>,
    callbacks: Occurrences,
    labels: Vec<String>,
}

fn parameters<'a>(
    function: &'a Function,
    id: usize,
    site: &Site,
) -> Result<&'a [Binding], Refusal> {
    let supported = match (function.parameters.as_slice(), &function.result) {
        ([input], Ty::Bool) => input.ty == Ty::Value,
        ([input, cache], Ty::Bool) => {
            matches!(input.ty, Ty::Value | Ty::String) && matches!(&cache.ty, Ty::Cache(_))
        }
        ([cache], Ty::Unit) => matches!(&cache.ty, Ty::Cache(_)),
        _ => false,
    };
    if !supported
        || function.body.site
            != (Site {
                function: id,
                lexical: vec![0],
            })
    {
        return Err(Refusal::CallBinding(site.clone()));
    }
    for (ordinal, parameter) in function.parameters.iter().enumerate() {
        if !parameter.borrowed
            || parameter.declaration.is_some()
            || parameter.origin
                != (Origin::Parameter {
                    function: id,
                    ordinal,
                })
        {
            return Err(Refusal::CallBinding(site.clone()));
        }
    }
    Ok(&function.parameters)
}

fn bound_value(bindings: &ProvenanceBindings, binding: &Binding) -> Option<Provenance> {
    bindings
        .iter()
        .rev()
        .find(|(known, _)| known.place == binding.place)
        .and_then(|(known, value)| (known == binding).then(|| value.clone()))
}

fn input_node(bindings: &ProvenanceBindings, binding: &Binding) -> Option<InputNode> {
    match bound_value(bindings, binding)? {
        Provenance::Input(node) => Some(node),
        Provenance::Cache(_) | Provenance::Boolean | Provenance::Scalar | Provenance::Regex(_) => {
            None
        }
    }
}

// Field types come from the canonical lowerer's declaration lookup. Here the
// entire evaluated place must agree with the caller's already-bound lineage.
// Cache fields and borrows never introduce an input-tree member.
fn argument_value(op: &Op, bindings: &ProvenanceBindings) -> Option<Provenance> {
    match &op.kind {
        Kind::Value(Atom::Place) => bound_value(bindings, &op.value),
        Kind::Evaluate(Action::Borrow { mutable: true }, values) => {
            let [value] = values.as_slice() else {
                return None;
            };
            let mut expected = value.value.clone();
            expected.borrowed = true;
            if op.value != expected || !matches!(&op.value.ty, Ty::Cache(_)) {
                return None;
            }
            match argument_value(value, bindings)? {
                Provenance::Cache(origin) => Some(Provenance::Cache(origin)),
                Provenance::Input(_)
                | Provenance::Boolean
                | Provenance::Scalar
                | Provenance::Regex(_) => None,
            }
        }
        Kind::Evaluate(Action::Field(member), values) => {
            let [base] = values.as_slice() else {
                return None;
            };
            if !matches!(&base.value.ty, Ty::Cache(_)) || !matches!(&op.value.ty, Ty::Cache(_)) {
                return None;
            }
            let expected = Binding {
                ty: op.value.ty.clone(),
                place: format!("{}.{member}", base.value.place),
                borrowed: base.value.borrowed,
                origin: Origin::Projection {
                    base: Box::new(base.value.origin.clone()),
                    member: member.clone(),
                },
                declaration: None,
            };
            if op.value != expected {
                return None;
            }
            let Provenance::Cache(origin) = argument_value(base, bindings)? else {
                return None;
            };
            Some(Provenance::Cache(Origin::Projection {
                base: Box::new(origin),
                member: member.clone(),
            }))
        }
        Kind::Primitive {
            lemma,
            callee,
            receiver: Some(receiver),
            arguments,
            source_calls,
        } if *lemma == Primitive::StringAccess
            && callee == "String::as_str"
            && source_calls.is_empty() =>
        {
            let [argument] = arguments.as_slice() else {
                return None;
            };
            let mut expected = receiver.clone();
            expected.declaration = None;
            (argument.value == *receiver
                && op.value == expected
                && argument_value(argument, bindings) == Some(Provenance::Scalar))
            .then_some(Provenance::Scalar)
        }
        Kind::Value(_)
        | Kind::Scope { .. }
        | Kind::Bind { .. }
        | Kind::Evaluate(_, _)
        | Kind::ScalarTest(_, _)
        | Kind::Choice { .. }
        | Kind::ShortCircuit(_, _)
        | Kind::Call { .. }
        | Kind::Service(_)
        | Kind::Iterate { .. }
        | Kind::Callback { .. }
        | Kind::Construct { .. }
        | Kind::Exit(_, _)
        | Kind::Primitive { .. } => None,
    }
}

fn evaluated_arguments(
    site: &Site,
    parameters: &[Binding],
    arguments: &[Binding],
    evaluation: &[Op],
    bindings: &ProvenanceBindings,
) -> Option<Vec<Provenance>> {
    if arguments.len() != parameters.len() || evaluation.len() != parameters.len() {
        return None;
    }
    let mut values = Vec::new();
    for (ordinal, ((formal, argument), expression)) in
        parameters.iter().zip(arguments).zip(evaluation).enumerate()
    {
        if !expression_site(&expression.site, &coupled_child(site, ordinal))
            || argument != &expression.value
            || argument.ty != formal.ty
            || !argument.borrowed
        {
            return None;
        }
        let value = argument_value(expression, bindings)?;
        if !matches!(
            (&formal.ty, &value),
            (Ty::Value, Provenance::Input(_))
                | (Ty::Cache(_), Provenance::Cache(_))
                | (Ty::String, Provenance::Scalar)
        ) {
            return None;
        }
        values.push(value);
    }
    Some(values)
}

fn root_values(recipe: &Recipe, root: usize, site: &Site) -> Result<Vec<Provenance>, Refusal> {
    let function = recipe
        .functions
        .get(&root)
        .ok_or_else(|| Refusal::AtSource {
            site: site.clone(),
            cause: operations::Refusal::UnclassifiedCall,
        })?;
    parameters(function, root, site)?
        .iter()
        .map(|parameter| match &parameter.ty {
            Ty::Value => Ok(Provenance::Input(InputNode {
                root,
                members: Vec::new(),
            })),
            Ty::Cache(_) => Ok(Provenance::Cache(parameter.origin.clone())),
            Ty::String
            | Ty::Number
            | Ty::Bool
            | Ty::Scalar
            | Ty::Map
            | Ty::Array
            | Ty::Regex
            | Ty::Input
            | Ty::LazyRegex(_)
            | Ty::Option(_)
            | Ty::Iterator(_)
            | Ty::Tuple(_)
            | Ty::Callback(_)
            | Ty::Named(_)
            | Ty::Unit => Err(Refusal::CallBinding(site.clone())),
        })
        .collect()
}

struct Traversal<'a> {
    parameters: &'a [Binding],
    evaluation: &'a Op,
    body: &'a Op,
    parent: InputNode,
    collection: &'a Binding,
}

fn iterator_matches(evaluation: &Op, bound: &IterationBound, receiver: &Binding) -> bool {
    let (expected, element) = match (&bound.collection.ty, &bound.element) {
        (Ty::Array, Ty::Value) => ("Array::iter", Ty::Value),
        (Ty::Map, Ty::Value) => ("Map::values", Ty::Value),
        (Ty::Map, Ty::String) => ("Map::keys", Ty::String),
        (Ty::Map, Ty::Tuple(types)) if types == &[Ty::String, Ty::Value] => {
            ("Map::iter", Ty::Tuple(vec![Ty::String, Ty::Value]))
        }
        _ => return false,
    };
    let collection = &bound.collection;
    let Kind::Primitive {
        lemma,
        callee,
        receiver: Some(actual),
        arguments,
        source_calls,
    } = &evaluation.kind
    else {
        return false;
    };
    let [argument] = arguments.as_slice() else {
        return false;
    };
    *lemma == Primitive::IteratorProtocol
        && callee == expected
        && bound.element == element
        && source_calls.is_empty()
        && actual == collection
        && argument.value == *collection
        && matches!(&argument.kind, Kind::Value(Atom::Place))
        && receiver == &evaluation.value
        && receiver.ty == Ty::Iterator(Box::new(element))
        && receiver.borrowed
        && receiver.origin == collection.origin
        && receiver.place == collection.place
        && receiver.declaration.is_none()
}

fn member_matches(parameter: &Binding, ty: &Ty, origin: &Origin, declaration: &Site) -> bool {
    parameter.ty == *ty
        && parameter.borrowed
        && parameter.declaration.as_ref() == Some(declaration)
        && parameter.origin == *origin
}

fn value_parameter<'a>(
    parameters: &'a [Binding],
    declaration: &Site,
    collection: &Binding,
) -> Option<&'a Binding> {
    let member = Origin::Projection {
        base: Box::new(collection.origin.clone()),
        member: "member".to_owned(),
    };
    match (&collection.ty, parameters) {
        (Ty::Array | Ty::Map, [value])
            if member_matches(value, &Ty::Value, &member, declaration) =>
        {
            Some(value)
        }
        (Ty::Map, [key]) if member_matches(key, &Ty::String, &member, declaration) => Some(key),
        (Ty::Map, [key, value]) => {
            let mut key_site = declaration.clone();
            key_site.lexical.push(0);
            let mut value_site = declaration.clone();
            value_site.lexical.push(1);
            let key_origin = Origin::Projection {
                base: Box::new(member.clone()),
                member: "0".to_owned(),
            };
            let value_origin = Origin::Projection {
                base: Box::new(member),
                member: "1".to_owned(),
            };
            // The key is checked, but only the Value slot acquires tree provenance.
            (member_matches(key, &Ty::String, &key_origin, &key_site)
                && member_matches(value, &Ty::Value, &value_origin, &value_site))
            .then_some(value)
        }
        _ => None,
    }
}

fn traversal<'a>(op: &'a Op, bindings: &ProvenanceBindings) -> Result<Traversal<'a>, Refusal> {
    let Kind::Iterate {
        protocol,
        callee,
        stop,
        none_on_exhaustion,
        bound,
        receiver,
        parameters,
        body,
        evaluation,
    } = &op.kind
    else {
        return Err(unsupported(op));
    };
    let refusal = || Refusal::IterationBinding(op.site.clone());
    let mut body_site = op.site.clone();
    body_site.lexical.push(1);
    if *protocol != Primitive::IteratorProtocol
        || !matches!(
            (callee.as_str(), stop, &op.value.ty),
            ("Iterator::all", StopRule::FirstFalseOrExhaustion, Ty::Bool)
                | ("Iterator::next/for", StopRule::BreakOrExhaustion, Ty::Unit)
        )
        || !*none_on_exhaustion
        || !bound.collection.borrowed
        || !iterator_matches(evaluation, bound, receiver)
        || !expression_site(&body.site, &body_site)
    {
        return Err(refusal());
    }
    value_parameter(parameters, &body_site, &bound.collection).ok_or_else(refusal)?;
    let parent = input_node(bindings, &bound.collection).ok_or_else(refusal)?;
    Ok(Traversal {
        parameters,
        evaluation,
        body,
        parent,
        collection: &bound.collection,
    })
}

impl Deriver<'_> {
    fn occurrences(&self) -> Result<Occurrences, Refusal> {
        let Some(deepest) = self
            .active_members
            .iter()
            .max_by_key(|axis| axis.node.members.len())
        else {
            return Ok(Occurrences {
                constant: 1,
                ..Occurrences::default()
            });
        };
        // The selected node fixes the indices on its path. Only other active
        // axes rescan: each retains its own array or key-funded map ceiling.
        let mut multiplicity = 1_u64;
        for axis in &self.active_members {
            if !deepest.node.members.contains(&axis.identity) {
                multiplicity = multiplicity
                    .checked_mul(axis.cap)
                    .ok_or(Refusal::Overflow)?;
            }
        }
        Ok(Occurrences {
            constant: 0,
            by_depth: BTreeMap::from([(
                (deepest.node.root, deepest.node.members.len()),
                multiplicity,
            )]),
        })
    }

    fn enter(&mut self, id: usize, values: Vec<Provenance>, site: &Site) -> Result<(), Refusal> {
        let recipe = self.recipe;
        let function = recipe.functions.get(&id).ok_or_else(|| Refusal::AtSource {
            site: site.clone(),
            cause: operations::Refusal::UnclassifiedCall,
        })?;
        let parameters = parameters(function, id, site)?;
        if parameters.len() != values.len() {
            return Err(Refusal::CallBinding(site.clone()));
        }
        let bindings: Vec<_> = parameters.iter().cloned().zip(values).collect();
        if !self.active_functions.insert(id) {
            return Err(Refusal::AtSource {
                site: site.clone(),
                cause: operations::Refusal::MissingDecreasingMeasure,
            });
        }
        let result = self.visit(&function.body, &bindings);
        self.active_functions.remove(&id);
        result
    }

    fn call(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Kind::Call {
            target,
            arguments,
            evaluation,
        } = &op.kind
        else {
            return Err(unsupported(op));
        };
        let recipe = self.recipe;
        let function = recipe
            .functions
            .get(target)
            .ok_or_else(|| Refusal::AtSource {
                site: op.site.clone(),
                cause: operations::Refusal::UnclassifiedCall,
            })?;
        let parameters = parameters(function, *target, &op.site)?;
        let values = evaluated_arguments(&op.site, parameters, arguments, evaluation, bindings)
            .ok_or_else(|| Refusal::CallBinding(op.site.clone()))?;
        for expression in evaluation {
            self.visit(expression, bindings)?;
        }
        let parameter = parameters
            .first()
            .ok_or_else(|| Refusal::CallBinding(op.site.clone()))?;
        let argument = arguments
            .first()
            .ok_or_else(|| Refusal::CallBinding(op.site.clone()))?;
        // One row counts this call site; every formal ordinal was checked above.
        let binding = CallBound {
            site: op.site.clone(),
            target: *target,
            formal_ordinal: 0,
            formal: parameter.clone(),
            actual: argument.clone(),
            per_attempt: SourceBoundCoeff {
                constant: 0,
                nodes: 0,
            },
        };
        let occurrences = self.occurrences()?;
        if let Some(prior) = self
            .calls
            .iter_mut()
            .find(|prior| prior.binding.site == op.site)
        {
            if prior.binding != binding {
                return Err(Refusal::CallBinding(op.site.clone()));
            }
            prior.occurrences.combine(occurrences)?;
        } else {
            self.calls.push(CallOccurrences {
                binding,
                occurrences,
            });
        }
        self.enter(*target, values, &op.site)
    }

    fn choice(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Kind::Choice { selector, arms } = &op.kind else {
            return Err(unsupported(op));
        };
        self.visit(selector, bindings)?;
        for (ordinal, arm) in arms.iter().enumerate() {
            let mut branch = bindings.to_vec();
            match &arm.pattern {
                Pattern::Variant(name, fields)
                    if payload_type(name).is_some()
                        && matches!(fields.as_slice(), [Pattern::Binding(_)]) =>
                {
                    branch.push(container_binding(
                        op, selector, arm, ordinal, bindings, name, fields,
                    )?);
                }
                pattern if nonbinding_pattern(pattern, selector) => {}
                Pattern::Wild
                | Pattern::Literal(_)
                | Pattern::Binding(_)
                | Pattern::Variant(_, _)
                | Pattern::Tuple(_)
                | Pattern::Or(_) => return Err(unsupported(op)),
            }
            if let Some(guard) = &arm.guard {
                self.visit(guard, &branch)?;
            }
            self.visit(&arm.body, &branch)?;
        }
        Ok(())
    }

    fn iteration(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Traversal {
            parameters,
            evaluation,
            body,
            parent: mut node,
            collection,
        } = traversal(op, bindings)?;
        self.visit(evaluation, bindings)?;
        let member = self.next_member;
        self.next_member = member.checked_add(1).ok_or(Refusal::Overflow)?;
        node.members.push(member);
        // Object entries fund both a key and a value in the existing J count.
        let cap = match &collection.ty {
            Ty::Array => self.child_cap,
            Ty::Map => self.child_cap.checked_div(2).ok_or(Refusal::Overflow)?,
            Ty::Value
            | Ty::String
            | Ty::Number
            | Ty::Bool
            | Ty::Scalar
            | Ty::Regex
            | Ty::Input
            | Ty::Cache(_)
            | Ty::LazyRegex(_)
            | Ty::Option(_)
            | Ty::Iterator(_)
            | Ty::Tuple(_)
            | Ty::Callback(_)
            | Ty::Named(_)
            | Ty::Unit => return Err(Refusal::IterationBinding(op.site.clone())),
        };
        self.active_members.push(MemberAxis {
            identity: member,
            node: node.clone(),
            cap,
        });
        let occurrences = self.occurrences()?;
        self.callbacks.combine(occurrences)?;
        let mut nested = bindings.to_vec();
        for parameter in parameters {
            let provenance = if parameter.ty == Ty::Value {
                Provenance::Input(node.clone())
            } else {
                Provenance::Scalar
            };
            nested.push((parameter.clone(), provenance));
        }
        let result = self.visit(body, &nested);
        self.active_members.pop();
        result
    }

    fn sequence(&mut self, values: &[Op], bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        for value in values {
            self.visit(value, bindings)?;
        }
        Ok(())
    }

    // This is a 0/1 callback under the existing occurrence, not a JSON child
    // iterator. Its protocol/search work is still an outstanding duty.
    fn regex_callback(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Kind::Callback {
            protocol: Primitive::ValueAccess,
            callee,
            receiver,
            parameters,
            body,
            evaluation,
        } = &op.kind
        else {
            return Err(unsupported(op));
        };
        let Kind::Primitive {
            lemma: Primitive::Readiness,
            callee: readiness,
            receiver: Some(storage),
            arguments,
            source_calls,
        } = &evaluation.kind
        else {
            return Err(unsupported(op));
        };
        let (Ty::LazyRegex(program), [parameter], [argument]) =
            (&storage.ty, parameters.as_slice(), arguments.as_slice())
        else {
            return Err(unsupported(op));
        };
        let expected_storage = Binding {
            ty: Ty::LazyRegex(program.clone()),
            place: program.clone(),
            borrowed: true,
            origin: Origin::Static(program.clone()),
            declaration: None,
        };
        let expected_receiver = Binding {
            ty: Ty::Option(Box::new(Ty::Regex)),
            ..expected_storage.clone()
        };
        let expected_parameter = Binding {
            ty: Ty::Regex,
            place: parameter.place.clone(),
            borrowed: true,
            origin: Origin::Projection {
                base: Box::new(expected_storage.origin.clone()),
                member: "Some".to_owned(),
            },
            declaration: Some(coupled_child(&op.site, 1)),
        };
        if callee != "Option::is_some_and"
            || readiness != &format!("{:?}::as_ref", storage.ty)
            || !source_calls.is_empty()
            || !self.programs.iter().any(|row| row.program == *program)
            || *storage != expected_storage
            || argument.value != expected_storage
            || !matches!(&argument.kind, Kind::Value(Atom::Place))
            || !expression_site(&argument.site, &coupled_child(&evaluation.site, 0))
            || *receiver != expected_receiver
            || evaluation.value != expected_receiver
            || *parameter != expected_parameter
            || parameter.place.is_empty()
            || !expression_site(&evaluation.site, &coupled_child(&op.site, 0))
            || !expression_site(&body.site, &coupled_child(&op.site, 1))
        {
            return Err(Refusal::CallBinding(op.site.clone()));
        }
        self.visit(evaluation, bindings)?;
        let mut nested = bindings.to_vec();
        nested.push((parameter.clone(), Provenance::Regex(program.clone())));
        self.visit(body, &nested)
    }

    fn regex_search(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Kind::Primitive {
            receiver: Some(receiver),
            arguments,
            ..
        } = &op.kind
        else {
            return Err(unsupported(op));
        };
        let Some(Provenance::Regex(program)) = bound_value(bindings, receiver) else {
            return Err(unsupported(op));
        };
        let [regex, cache, input] = arguments.as_slice() else {
            return Err(unsupported(op));
        };
        let Some(row) = self.programs.iter().find(|row| row.program == program) else {
            return Err(unsupported(op));
        };
        let Some(Provenance::Cache(Origin::Projection { member, .. })) =
            argument_value(cache, bindings)
        else {
            return Err(Refusal::CallBinding(op.site.clone()));
        };
        if regex.value != *receiver
            || !expression_site(&regex.site, &coupled_child(&op.site, 0))
            || !expression_site(&cache.site, &coupled_child(&op.site, 1))
            || !expression_site(&input.site, &coupled_child(&op.site, 2))
            || member != row.field
            || !borrowed_search_input(input, bindings)
        {
            return Err(Refusal::CallBinding(op.site.clone()));
        }
        let field = row.field.clone();
        let occurrences = self.occurrences()?;
        if let Some(prior) = self.searches.iter_mut().find(|prior| prior.site == op.site) {
            if prior.program != program || prior.field != field {
                return Err(Refusal::CallBinding(op.site.clone()));
            }
            prior.occurrences.combine(occurrences)?;
        } else {
            self.searches.push(SearchOccurrences {
                site: op.site.clone(),
                program,
                field,
                occurrences,
            });
        }
        Ok(())
    }

    fn scope(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Kind::Scope {
            label: scope_label,
            steps,
            locals,
        } = &op.kind
        else {
            return Err(unsupported(op));
        };
        if let Some(active_label) = scope_label
            && (active_label.is_empty()
                || self.labels.contains(active_label)
                || op.value.ty != Ty::Bool)
        {
            return Err(unsupported(op));
        }
        let declared: Vec<_> = steps
            .iter()
            .filter_map(|step| {
                if let Kind::Bind {
                    bindings: declared_bindings,
                    ..
                } = &step.kind
                {
                    Some(declared_bindings.as_slice())
                } else {
                    None
                }
            })
            .flatten()
            .collect();
        if !declared.iter().copied().eq(locals) || !scope_sites(op, steps) {
            return Err(Refusal::LocalBinding(op.site.clone()));
        }
        if op.value.ty == Ty::Bool && steps.last().is_none_or(|tail| tail.value.ty != Ty::Bool) {
            return Err(Refusal::LocalBinding(op.site.clone()));
        }
        let mut nested = bindings.to_vec();
        if let Some(active_label) = scope_label {
            self.labels.push(active_label.clone());
        }
        for step in steps {
            if matches!(&step.kind, Kind::Bind { .. }) {
                self.bind_local(step, &mut nested)?;
            } else {
                self.visit(step, &nested)?;
            }
        }
        if scope_label.is_some() {
            self.labels.pop();
        }
        Ok(())
    }

    fn bind_local(
        &mut self,
        op: &Op,
        active: &mut Vec<(Binding, Provenance)>,
    ) -> Result<(), Refusal> {
        let Kind::Bind {
            pattern,
            bindings,
            value,
        } = &op.kind
        else {
            return Err(unsupported(op));
        };
        let (Pattern::Binding(name), [binding]) = (pattern, bindings.as_slice()) else {
            return Err(unsupported(op));
        };
        if binding.ty != value.value.ty || !matches!(binding.ty, Ty::Bool | Ty::String) {
            return Err(unsupported(op));
        }
        let mut expected = value.value.clone();
        expected.place.clone_from(name);
        expected.declaration = Some(op.site.clone());
        if binding != &expected
            || (binding.ty == Ty::Bool && binding.borrowed)
            || name.is_empty()
            || !expression_site(&value.site, &coupled_child(&op.site, 0))
            || !temporary(op, &Ty::Unit)
        {
            return Err(Refusal::LocalBinding(op.site.clone()));
        }
        self.visit(value, active)?;
        let provenance = if binding.ty == Ty::Bool {
            Provenance::Boolean
        } else {
            argument_value(value, active).ok_or_else(|| Refusal::LocalBinding(op.site.clone()))?
        };
        active.push((binding.clone(), provenance));
        Ok(())
    }

    fn evaluate(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Kind::Evaluate(action, values) = &op.kind else {
            return Err(unsupported(op));
        };
        match action {
            Action::CastScalar => self.scalar_cast(op, values, bindings),
            Action::Not => {
                let [value] = values.as_slice() else {
                    return Err(Refusal::LocalBinding(op.site.clone()));
                };
                if !temporary(op, &Ty::Bool)
                    || value.value.ty != Ty::Bool
                    || !expression_site(&value.site, &coupled_child(&op.site, 0))
                {
                    return Err(Refusal::LocalBinding(op.site.clone()));
                }
                self.visit(value, bindings)
            }
            Action::Assign(target) => {
                let [left, right] = values.as_slice() else {
                    return Err(Refusal::LocalBinding(op.site.clone()));
                };
                if !bound_boolean(bindings, target)
                    || left.value != *target
                    || !matches!(&left.kind, Kind::Value(Atom::Place))
                    || right.value.ty != Ty::Bool
                    || left.site != coupled_child(&op.site, 0)
                    || !expression_site(&right.site, &coupled_child(&op.site, 1))
                    || !temporary(op, &Ty::Unit)
                {
                    return Err(Refusal::LocalBinding(op.site.clone()));
                }
                self.sequence(values, bindings)
            }
            Action::Borrow { mutable: true } | Action::Field(_)
                if matches!(argument_value(op, bindings), Some(Provenance::Cache(_))) =>
            {
                self.sequence(values, bindings)
            }
            Action::Dereference
                if values.len() == 1
                    && temporary(op, &Ty::Bool)
                    && values.first().is_some_and(|value| {
                        value.value.ty == Ty::Bool
                            && value.value.borrowed
                            && bound_boolean(bindings, &value.value)
                    }) =>
            {
                self.sequence(values, bindings)
            }
            Action::Borrow { mutable: false } if borrowed_search_input(op, bindings) => {
                self.sequence(values, bindings)
            }
            Action::Borrow { .. }
            | Action::Field(_)
            | Action::Negate
            | Action::Dereference
            | Action::AndAssign(_) => Err(unsupported(op)),
        }
    }

    // A scalar cast changes no helper/iterator population. This visits its
    // operand once; scalar semantics, work and width remain separate duties.
    fn scalar_cast(
        &mut self,
        op: &Op,
        values: &[Op],
        bindings: &ProvenanceBindings,
    ) -> Result<(), Refusal> {
        let [value] = values else {
            return Err(unsupported(op));
        };
        if value.value.ty != Ty::Scalar
            || !temporary(op, &Ty::Scalar)
            || !expression_site(&value.site, &coupled_child(&op.site, 0))
        {
            return Err(unsupported(op));
        }
        self.visit(value, bindings)
    }

    fn visit(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        if op.value.ty == Ty::Bool && !boolean_result(op, bindings) {
            return Err(Refusal::LocalBinding(op.site.clone()));
        }
        match &op.kind {
            Kind::Value(_) => Ok(()),
            Kind::Scope { .. } => self.scope(op, bindings),
            Kind::ScalarTest(_, values) | Kind::ShortCircuit(_, values) => {
                self.sequence(values, bindings)
            }
            Kind::Evaluate(_, _) => self.evaluate(op, bindings),
            Kind::Choice { .. } => self.choice(op, bindings),
            Kind::Call { .. } => self.call(op, bindings),
            Kind::Iterate { .. } => self.iteration(op, bindings),
            Kind::Callback { .. } => self.regex_callback(op, bindings),
            Kind::Primitive {
                lemma,
                callee,
                arguments,
                source_calls,
                ..
            } if source_calls.is_empty() => {
                if *lemma == Primitive::RegexSearch && callee == "Regex::search_half_with" {
                    self.regex_search(op, bindings)?;
                }
                self.sequence(arguments, bindings)
            }
            Kind::Exit(exit, result) => {
                if let operations::model::Exit::Break(label) = exit
                    && (label.is_empty() || !self.labels.contains(label))
                {
                    return Err(unsupported(op));
                }
                if let Some(value) = result {
                    self.visit(value, bindings)?;
                }
                Ok(())
            }
            Kind::Bind { .. }
            | Kind::Primitive { .. }
            | Kind::Service(_)
            | Kind::Construct { .. } => Err(unsupported(op)),
        }
    }
}

fn unsupported(op: &Op) -> Refusal {
    Refusal::AtSource {
        site: op.site.clone(),
        cause: operations::Refusal::UnsupportedSource,
    }
}

fn borrowed_search_input(op: &Op, bindings: &ProvenanceBindings) -> bool {
    let Kind::Evaluate(Action::Borrow { mutable: false }, values) = &op.kind else {
        return false;
    };
    let [earliest] = values.as_slice() else {
        return false;
    };
    let Kind::Primitive {
        lemma: Primitive::RegexSearch,
        callee,
        receiver: Some(receiver),
        arguments,
        source_calls,
    } = &earliest.kind
    else {
        return false;
    };
    let [new, flag] = arguments.as_slice() else {
        return false;
    };
    let mut borrowed = earliest.value.clone();
    borrowed.borrowed = true;
    if callee != "Input::earliest"
        || !source_calls.is_empty()
        || op.value != borrowed
        || !expression_site(&earliest.site, &coupled_child(&op.site, 0))
        || !expression_site(&new.site, &coupled_child(&earliest.site, 0))
        || !expression_site(&flag.site, &coupled_child(&earliest.site, 1))
        || !matches!(&flag.kind, Kind::Value(Atom::Bool(true)))
        || flag.value.ty != Ty::Bool
        || *receiver != new.value
        || earliest.value != new.value
    {
        return false;
    }
    let Kind::Primitive {
        lemma: Primitive::RegexSearch,
        callee: constructor_callee,
        receiver: None,
        arguments: constructor_arguments,
        source_calls: constructor_source_calls,
    } = &new.kind
    else {
        return false;
    };
    let [subject] = constructor_arguments.as_slice() else {
        return false;
    };
    constructor_callee == "regex_automata::Input::new"
        && constructor_source_calls.is_empty()
        && temporary(new, &Ty::Input)
        && expression_site(&subject.site, &coupled_child(&new.site, 0))
        && subject.value.ty == Ty::String
        && subject.value.borrowed
        && argument_value(subject, bindings) == Some(Provenance::Scalar)
}

fn temporary(op: &Op, ty: &Ty) -> bool {
    op.value.ty == *ty
        && op.value.place.is_empty()
        && !op.value.borrowed
        && op.value.declaration.is_none()
        && op.value.origin == Origin::Temporary(op.site.clone())
}

fn bound_boolean(bindings: &ProvenanceBindings, binding: &Binding) -> bool {
    bindings
        .iter()
        .rev()
        .find(|(known, _)| known.place == binding.place)
        .is_some_and(|(known, provenance)| {
            known == binding && matches!(provenance, Provenance::Boolean)
        })
}

fn boolean_result(op: &Op, bindings: &ProvenanceBindings) -> bool {
    match &op.kind {
        Kind::Value(Atom::Place) => bound_boolean(bindings, &op.value),
        Kind::Value(Atom::Scalar(_) | Atom::String(_) | Atom::None | Atom::Unit) => false,
        Kind::Primitive { .. } => boolean_primitive(op),
        Kind::Value(Atom::Bool(_))
        | Kind::Scope { .. }
        | Kind::Bind { .. }
        | Kind::Evaluate(_, _)
        | Kind::ScalarTest(_, _)
        | Kind::Choice { .. }
        | Kind::ShortCircuit(_, _)
        | Kind::Call { .. }
        | Kind::Service(_)
        | Kind::Iterate { .. }
        | Kind::Callback { .. }
        | Kind::Construct { .. }
        | Kind::Exit(_, _) => temporary(op, &Ty::Bool),
    }
}

// Primitive method results retain the evaluated receiver's place/origin in
// the canonical lowerer. Its synthetic string-pattern selector is a temporary.
// This correspondence does not discharge the primitive's source/work lemma.
fn boolean_primitive(op: &Op) -> bool {
    let Kind::Primitive {
        lemma,
        callee,
        receiver: receiver_option,
        arguments,
        ..
    } = &op.kind
    else {
        return false;
    };
    let Some(receiver) = receiver_option else {
        return temporary(op, &Ty::Bool);
    };
    let Some(argument) = arguments.first() else {
        return false;
    };
    if argument.value != *receiver {
        return false;
    }
    if *lemma == Primitive::StringAccess && callee == "str::pattern_decision" {
        return expression_site(&argument.site, &op.site) && temporary(op, &Ty::Bool);
    }
    expression_site(&argument.site, &coupled_child(&op.site, 0))
        && op.value
            == (Binding {
                ty: Ty::Bool,
                place: receiver.place.clone(),
                borrowed: false,
                origin: receiver.origin.clone(),
                declaration: None,
            })
}

// The canonical lowerer erases Paren/Group while descending through child 0.
// Only expression slots admit those zero suffixes; declarations stay exact.
// This is conditional on CanonicalRecipe, not source or work authority.
fn expression_site(actual: &Site, expected: &Site) -> bool {
    actual.function == expected.function
        && actual
            .lexical
            .strip_prefix(expected.lexical.as_slice())
            .is_some_and(|wrappers| wrappers.iter().all(|ordinal| *ordinal == 0))
}

fn scope_sites(op: &Op, steps: &[Op]) -> bool {
    let mut previous = None;
    for step in steps {
        let Some(relative) = step.site.lexical.strip_prefix(op.site.lexical.as_slice()) else {
            return false;
        };
        let Some((ordinal, wrappers)) = relative.split_first() else {
            return false;
        };
        if step.site.function != op.site.function
            || wrappers.iter().any(|component| *component != 0)
            || (matches!(&step.kind, Kind::Bind { .. }) && !wrappers.is_empty())
            || previous.is_some_and(|prior| prior >= *ordinal)
        {
            return false;
        }
        previous = Some(*ordinal);
    }
    true
}

fn container_type(name: &str) -> Option<Ty> {
    match name {
        "serde_json::Value::Array" | "jsonschema::__private::serde_json::Value::Array" => {
            Some(Ty::Array)
        }
        "serde_json::Value::Object" | "jsonschema::__private::serde_json::Value::Object" => {
            Some(Ty::Map)
        }
        _ => None,
    }
}

fn payload_type(name: &str) -> Option<Ty> {
    container_type(name).or(match name {
        "serde_json::Value::String" | "jsonschema::__private::serde_json::Value::String" => {
            Some(Ty::String)
        }
        "serde_json::Value::Number" | "jsonschema::__private::serde_json::Value::Number" => {
            Some(Ty::Number)
        }
        "serde_json::Value::Bool" | "jsonschema::__private::serde_json::Value::Bool" => {
            Some(Ty::Bool)
        }
        _ => None,
    })
}

fn selected_type(selector: &Op) -> Option<&Ty> {
    if let Kind::ScalarTest(Comparison::TypeDiscriminant, values) = &selector.kind {
        return values.first().map(|value| &value.value.ty);
    }
    if let Kind::Primitive {
        lemma: Primitive::StringAccess,
        callee,
        arguments,
        source_calls,
        ..
    } = &selector.kind
        && callee == "str::pattern_decision"
        && source_calls.is_empty()
    {
        return arguments.first().map(|value| &value.value.ty);
    }
    (selector.value.ty == Ty::Bool).then_some(&selector.value.ty)
}

fn nonbinding_pattern(pattern: &Pattern, selector: &Op) -> bool {
    match pattern {
        Pattern::Wild => true,
        Pattern::Literal(tokens) => matches!(
            (selected_type(selector), syn::parse_str::<syn::Lit>(tokens)),
            (Some(Ty::String), Ok(syn::Lit::Str(_)))
                | (Some(Ty::Bool), Ok(syn::Lit::Bool(_)))
                | (Some(Ty::Scalar), Ok(syn::Lit::Int(_)))
        ),
        Pattern::Or(patterns) => {
            !patterns.is_empty() && patterns.iter().all(|p| nonbinding_pattern(p, selector))
        }
        Pattern::Variant(name, fields) if selected_type(selector) == Some(&Ty::Value) => {
            (payload_type(name).is_some() && fields == &[Pattern::Wild])
                || (name == "Value::Null" && fields.is_empty())
        }
        Pattern::Binding(_) | Pattern::Variant(_, _) | Pattern::Tuple(_) => false,
    }
}

// Payload provenance comes from the actual typed match arm and selector, then
// the iterator checks its exact receiver and binder sites. The spelling of a
// Projection alone cannot introduce either a container or a member.
fn container_binding(
    op: &Op,
    selector: &Op,
    arm: &Arm,
    ordinal: usize,
    bindings: &ProvenanceBindings,
    name: &str,
    fields: &[Pattern],
) -> Result<(Binding, Provenance), Refusal> {
    let refusal = || Refusal::IterationBinding(op.site.clone());
    let Kind::ScalarTest(Comparison::TypeDiscriminant, values) = &selector.kind else {
        return Err(refusal());
    };
    let ([value], [Pattern::Binding(place)]) = (values.as_slice(), fields) else {
        return Err(refusal());
    };
    if value.value.ty != Ty::Value
        || !value.value.borrowed
        || !matches!(&value.kind, Kind::Value(Atom::Place))
    {
        return Err(refusal());
    }
    let node = input_node(bindings, &value.value).ok_or_else(refusal)?;
    let mut anchor = op.site.clone();
    anchor
        .lexical
        .push(ordinal.checked_add(1).ok_or(Refusal::Overflow)?);
    let mut body_site = anchor.clone();
    body_site.lexical.push(1);
    if !expression_site(&arm.body.site, &body_site) {
        return Err(refusal());
    }
    anchor.lexical.push(0);
    Ok((
        Binding {
            ty: payload_type(name).ok_or_else(refusal)?,
            place: place.clone(),
            borrowed: true,
            origin: Origin::Projection {
                base: Box::new(value.value.origin.clone()),
                member: name.to_owned(),
            },
            declaration: Some(anchor),
        },
        match payload_type(name).ok_or_else(refusal)? {
            Ty::Array | Ty::Map => Provenance::Input(node),
            Ty::Bool => Provenance::Boolean,
            Ty::String | Ty::Number => Provenance::Scalar,
            Ty::Value
            | Ty::Scalar
            | Ty::Regex
            | Ty::Input
            | Ty::Cache(_)
            | Ty::LazyRegex(_)
            | Ty::Option(_)
            | Ty::Iterator(_)
            | Ty::Tuple(_)
            | Ty::Callback(_)
            | Ty::Named(_)
            | Ty::Unit => return Err(refusal()),
        },
    ))
}

fn checked_bound(coefficient: SourceBoundCoeff, cap: NodeCapData) -> Result<u64, Refusal> {
    coefficient
        .nodes
        .checked_mul(cap.nodes)
        .and_then(|value| value.checked_add(coefficient.constant))
        .ok_or(Refusal::Overflow)
}

// Supported signatures carry a borrowed Value, optionally its ancillary cache,
// or a cache-only Unit helper. Array/map members use the canonical all protocol.
// Other grammar refuses; no coarse counter is a fallback.
// All source checks and per-attempt duties survive zero execution attempts.
fn derive_source_bounds(
    recipe: &Recipe,
    cap: NodeCapData,
    attempts: u64,
) -> Result<SourceBounds, Refusal> {
    if recipe.roots.is_empty()
        || recipe.roots.iter().collect::<BTreeSet<_>>().len() != recipe.roots.len()
        || !recipe.initialization.is_empty()
    {
        return Err(operations::Refusal::UnsupportedSource.into());
    }
    partition_roots(recipe, &recipe.roots, cap, attempts, &[])
}

// All projections share one provenance/counting kernel and original Recipe.
// Root selection is private: the coupled caller supplies its just-checked root,
// while the standalone entry above retains its empty-initialization guard.
fn partition_roots(
    recipe: &Recipe,
    roots: &[usize],
    cap: NodeCapData,
    attempts: u64,
    programs: &[ProgramFactoryBound],
) -> Result<SourceBounds, Refusal> {
    partition_analysis(recipe, roots, cap, attempts, programs).map(|analysis| analysis.input)
}

fn partition_analysis(
    recipe: &Recipe,
    roots: &[usize],
    cap: NodeCapData,
    attempts: u64,
    programs: &[ProgramFactoryBound],
) -> Result<PartitionAnalysis, Refusal> {
    let mut deriver = Deriver {
        recipe,
        programs,
        child_cap: cap.nodes.checked_sub(1).ok_or(Refusal::Overflow)?,
        active_functions: BTreeSet::new(),
        active_members: Vec::new(),
        next_member: 0,
        calls: Vec::new(),
        searches: Vec::new(),
        callbacks: Occurrences::default(),
        labels: Vec::new(),
    };
    for &root in roots {
        let site = Site {
            function: root,
            lexical: Vec::new(),
        };
        deriver.enter(root, root_values(recipe, root, &site)?, &site)?;
    }
    let callbacks = deriver.callbacks.coefficient()?;
    let mut maximum = checked_bound(callbacks, cap)?;
    let mut helper_calls = Vec::new();
    for CallOccurrences {
        mut binding,
        occurrences,
    } in deriver.calls
    {
        binding.per_attempt = occurrences.coefficient()?;
        maximum = maximum
            .checked_add(checked_bound(binding.per_attempt, cap)?)
            .ok_or(Refusal::Overflow)?;
        helper_calls.push(binding);
    }
    maximum.checked_mul(attempts).ok_or(Refusal::Overflow)?;
    let input = SourceBounds {
        roots: roots.to_vec(),
        cap,
        attempts,
        helper_calls,
        iterator_callbacks_per_attempt: callbacks,
        required_sources: BTreeSet::from([
            SourceRequirement::CanonicalRecipe,
            SourceRequirement::BorrowedInputTree,
            SourceRequirement::FormalActualBindings,
            SourceRequirement::IteratorMemberProtocol,
            SourceRequirement::CallerNodeCap,
        ]),
    };
    Ok(PartitionAnalysis {
        input,
        searches: deriver.searches,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CallBound, NodeCapData, Refusal, SourceBoundCoeff, SourceBounds, SourceRequirement,
        derive_source_bounds,
    };
    use crate::schema_validation::operations::{
        self,
        model::{self, Binding, Kind, Op, Origin, Recipe, Site, StopRule, Ty},
    };
    use auctioneer_bidder::schema::{SchemaInputLimits, measure_schema_input};
    use serde_json::{Value, json};
    use std::collections::BTreeSet;

    fn requirements() -> BTreeSet<SourceRequirement> {
        BTreeSet::from([
            SourceRequirement::CanonicalRecipe,
            SourceRequirement::BorrowedInputTree,
            SourceRequirement::FormalActualBindings,
            SourceRequirement::IteratorMemberProtocol,
            SourceRequirement::CallerNodeCap,
        ])
    }

    // The sole source difference is whether the helper receives the member or
    // its ancestor. Neither source fixture is a selected-program witness.
    fn fixture(descendant: bool) -> Recipe {
        let argument: syn::Expr = if descendant {
            syn::parse_quote!(item)
        } else {
            syn::parse_quote!(instance)
        };
        let source: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::Array(items) => items.iter().all(|item| middle(#argument)),
                    _ => false,
                }
            }
            fn middle(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::Array(items) => items.iter().all(|item| leaf(item)),
                    _ => false,
                }
            }
            fn leaf(instance: &serde_json::Value) -> bool { instance.is_boolean() }
        };
        operations::classify_source_body(&source, &[], &[])
            .expect("both source variants pass the existing canonical classifier")
            .recipe()
            .clone()
    }

    fn function(recipe: &Recipe, name: &str) -> usize {
        let mut matching = recipe
            .functions
            .iter()
            .filter(|(_, function)| function.identity == name);
        let (id, _) = matching.next().expect("fixture function");
        assert!(matching.next().is_none());
        *id
    }

    fn call_bound(
        recipe: &Recipe,
        source_function: &str,
        target_function: &str,
        nodes: u64,
    ) -> CallBound {
        let caller_id = function(recipe, source_function);
        let target = function(recipe, target_function);
        let formal = &recipe.functions[&target].parameters[0];
        assert_eq!(recipe.functions[&target].parameters.len(), 1);
        assert_eq!(formal.ty, Ty::Value);
        assert!(formal.borrowed);
        assert_eq!(
            formal.origin,
            Origin::Parameter {
                function: target,
                ordinal: 0,
            }
        );
        let mut found = Vec::new();
        model::walk(&recipe.functions[&caller_id].body, &mut |op| {
            if let Kind::Call {
                target: actual_target,
                arguments,
                evaluation,
            } = &op.kind
                && *actual_target == target
            {
                assert_eq!(op.site.function, caller_id);
                assert_eq!(arguments.len(), 1);
                assert_eq!(evaluation.len(), 1);
                assert_eq!(arguments[0], evaluation[0].value);
                assert_eq!(arguments[0].ty, formal.ty);
                assert!(arguments[0].borrowed);
                found.push(CallBound {
                    site: op.site.clone(),
                    target,
                    formal_ordinal: 0,
                    formal: formal.clone(),
                    actual: arguments[0].clone(),
                    per_attempt: SourceBoundCoeff { constant: 0, nodes },
                });
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(found.len(), 1);
        found.pop().unwrap()
    }

    fn iteration(recipe: &Recipe, name: &str) -> (Site, Binding, Binding) {
        let id = function(recipe, name);
        let mut found = Vec::new();
        model::walk(&recipe.functions[&id].body, &mut |op| {
            if let Kind::Iterate {
                callee,
                stop,
                none_on_exhaustion,
                bound,
                parameters,
                body,
                ..
            } = &op.kind
            {
                assert_eq!(callee, "Iterator::all");
                assert_eq!(*stop, StopRule::FirstFalseOrExhaustion);
                assert!(*none_on_exhaustion);
                assert_eq!(bound.collection.ty, Ty::Array);
                assert!(bound.collection.borrowed);
                assert_eq!(bound.element, Ty::Value);
                assert_eq!(parameters.len(), 1);
                assert_eq!(parameters[0].ty, Ty::Value);
                assert!(parameters[0].borrowed);
                assert_eq!(parameters[0].declaration.as_ref(), Some(&body.site));
                assert_eq!(
                    parameters[0].origin,
                    Origin::Projection {
                        base: Box::new(bound.collection.origin.clone()),
                        member: "member".to_owned(),
                    }
                );
                found.push((
                    op.site.clone(),
                    bound.collection.clone(),
                    parameters[0].clone(),
                ));
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(found.len(), 1);
        found.pop().unwrap()
    }

    fn assert_bindings(recipe: &Recipe, descendant: bool) {
        let root = function(recipe, "is_valid");
        let middle = function(recipe, "middle");
        assert_eq!(recipe.roots, vec![root]);
        let root_input = &recipe.functions[&root].parameters[0];
        let middle_input = &recipe.functions[&middle].parameters[0];
        let (_, outer_collection, outer_item) = iteration(recipe, "is_valid");
        let (_, inner_collection, inner_item) = iteration(recipe, "middle");
        for (collection, input) in [
            (&outer_collection, root_input),
            (&inner_collection, middle_input),
        ] {
            assert_eq!(
                collection.origin,
                Origin::Projection {
                    base: Box::new(input.origin.clone()),
                    member: "serde_json::Value::Array".to_owned(),
                }
            );
        }
        let outer_call = call_bound(recipe, "is_valid", "middle", 1);
        assert_eq!(
            outer_call.actual.origin,
            if descendant {
                outer_item.origin
            } else {
                root_input.origin.clone()
            }
        );
        let inner_call = call_bound(recipe, "middle", "leaf", 1);
        assert_eq!(inner_call.actual.origin, inner_item.origin);
    }

    // Literal coefficients are supplied by each test, never calculated from
    // the deriver or inferred from the observed JSON's node count.
    fn expected(
        recipe: &Recipe,
        cap: NodeCapData,
        attempts: u64,
        leaf_nodes: u64,
        callback_nodes: u64,
    ) -> SourceBounds {
        SourceBounds {
            roots: recipe.roots.clone(),
            cap,
            attempts,
            helper_calls: vec![
                call_bound(recipe, "is_valid", "middle", 1),
                call_bound(recipe, "middle", "leaf", leaf_nodes),
            ],
            iterator_callbacks_per_attempt: SourceBoundCoeff {
                constant: 0,
                nodes: callback_nodes,
            },
            required_sources: requirements(),
        }
    }

    fn measured_nodes(value: &Value, cap: NodeCapData) -> u64 {
        // Diagnostic fixture limits only. Actual use must borrow installed
        // limits from the existing checked caller context, not these literals.
        measure_schema_input(
            value,
            SchemaInputLimits {
                serialized_bytes: 0x0001_0000,
                nodes: cap.nodes,
                depth: 32,
            },
        )
        .expect("literal JSON witness is admitted within its diagnostic cap")
        .nodes
    }

    fn at_nodes(coefficient: SourceBoundCoeff, nodes: u64) -> u64 {
        coefficient
            .nodes
            .checked_mul(nodes)
            .and_then(|value| value.checked_add(coefficient.constant))
            .expect("small literal coefficient projection")
    }

    #[test]
    fn disjoint_children_across_helpers_have_linear_coefficients() {
        let recipe = fixture(true);
        assert_bindings(&recipe, true);
        let cap = NodeCapData { nodes: 7 };
        let witness = json!([[true, true], [true]]);
        let nodes = measured_nodes(&witness, cap);
        assert_eq!(nodes, 6);
        let rows = witness.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].as_array().unwrap().len(), 2);
        assert_eq!(rows[1].as_array().unwrap().len(), 1);
        // Both all() calls finish: three true boolean leaves, five callbacks.
        assert_eq!(
            rows[0]
                .as_array()
                .unwrap()
                .len()
                .checked_add(rows[1].as_array().unwrap().len()),
            Some(3)
        );
        assert_eq!(rows.len().checked_add(3), Some(5));
        let literal = expected(&recipe, cap, 1, 1, 1);
        assert_eq!(at_nodes(literal.helper_calls[1].per_attempt, nodes), 6);
        assert_eq!(at_nodes(literal.iterator_callbacks_per_attempt, nodes), 6);
        // Six is an upper bound, not an assertion of six executed leaf calls.
        assert_eq!(derive_source_bounds(&recipe, cap, 1), Ok(literal));
        assert_eq!(
            derive_source_bounds(&recipe, NodeCapData { nodes: 11 }, 1),
            Ok(expected(&recipe, NodeCapData { nodes: 11 }, 1, 1, 1))
        );
        let zero = expected(&recipe, cap, 0, 1, 1);
        assert_eq!(zero.required_sources, requirements());
        assert_eq!(zero.helper_calls[1].per_attempt.nodes, 1);
        assert_eq!(derive_source_bounds(&recipe, cap, 0), Ok(zero));
    }

    #[test]
    fn ancestor_rescans_require_source_bound_cap_lifting() {
        let recipe = fixture(false);
        assert_bindings(&recipe, false);
        let cap = NodeCapData { nodes: 7 };
        let witness = json!([true, true, true]);
        let nodes = measured_nodes(&witness, cap);
        assert_eq!((cap.nodes, nodes), (7, 4));
        let members = u64::try_from(witness.as_array().unwrap().len()).unwrap();
        assert_eq!(members, 3);
        assert_eq!(members.checked_mul(members), Some(9));
        assert_eq!(members.checked_add(9), Some(12));
        assert!(9 > nodes, "one J cannot cover the ancestor rescan");
        // n <= J-1 <= 6: n*n <= 6*J; n+n*n <= 7*J.
        // These are cap-lifted ceilings, not the exact 9/12 witness counts.
        let literal = expected(&recipe, cap, 1, 6, 7);
        assert_eq!(at_nodes(literal.helper_calls[1].per_attempt, nodes), 24);
        assert_eq!(at_nodes(literal.iterator_callbacks_per_attempt, nodes), 28);
        assert_eq!(derive_source_bounds(&recipe, cap, 1), Ok(literal));
        // Vary only the cap; observed J stays four. Fitting J would give three
        // instead of either required cap-dependent leaf coefficient.
        let larger = NodeCapData { nodes: 11 };
        assert_eq!(measured_nodes(&witness, larger), 4);
        assert_eq!(
            derive_source_bounds(&recipe, larger, 1),
            Ok(expected(&recipe, larger, 1, 10, 11))
        );
        assert_eq!(
            derive_source_bounds(&recipe, cap, 0),
            Ok(expected(&recipe, cap, 0, 6, 7))
        );
        assert_eq!(
            derive_source_bounds(&recipe, NodeCapData { nodes: u64::MAX }, 1),
            Err(Refusal::Overflow)
        );
    }

    // Fixture-shaped projection only, not another source parser or validator.
    fn iteration_mut(recipe: &mut Recipe, owner: usize) -> &mut Op {
        let function = recipe.functions.get_mut(&owner).unwrap();
        let Kind::Scope { steps, .. } = &mut function.body.kind else {
            panic!("fixture function scope");
        };
        assert_eq!(steps.len(), 1);
        let Kind::Choice { arms, .. } = &mut steps[0].kind else {
            panic!("fixture array match");
        };
        assert_eq!(arms.len(), 2);
        let body = &mut arms[0].body;
        assert!(matches!(&body.kind, Kind::Iterate { .. }));
        body
    }

    #[test]
    fn changed_iteration_and_call_bindings_are_refused() {
        let recipe = fixture(true);
        assert_bindings(&recipe, true);
        let cap = NodeCapData { nodes: 7 };
        assert_eq!(
            derive_source_bounds(&recipe, cap, 1),
            Ok(expected(&recipe, cap, 1, 1, 1))
        );
        let root = function(&recipe, "is_valid");
        let middle = function(&recipe, "middle");
        let (loop_site, _, _) = iteration(&recipe, "is_valid");
        let call_site = call_bound(&recipe, "is_valid", "middle", 1).site;
        let mut unborrowed = recipe.clone();
        let Kind::Iterate { bound, .. } = &mut iteration_mut(&mut unborrowed, root).kind else {
            panic!("fixture iterator");
        };
        assert!(bound.collection.borrowed);
        bound.collection.borrowed = false;
        let mut invented_origin = recipe.clone();
        let Kind::Iterate { parameters, .. } = &mut iteration_mut(&mut invented_origin, root).kind
        else {
            panic!("fixture iterator");
        };
        // Keep the attractive 'member' label and type while removing its
        // source-proven borrowed-container/binder correspondence.
        parameters[0].origin = Origin::Projection {
            base: Box::new(Origin::Temporary(loop_site.clone())),
            member: "member".to_owned(),
        };
        let mut wrong_site = recipe.clone();
        let Kind::Iterate {
            parameters: wrong_parameters,
            ..
        } = &mut iteration_mut(&mut wrong_site, root).kind
        else {
            panic!("fixture iterator");
        };
        assert_eq!(wrong_parameters[0].declaration.as_ref(), Some(&call_site));
        wrong_parameters[0].declaration = Some(Site {
            function: root,
            lexical: vec![999],
        });
        for changed in [&unborrowed, &invented_origin, &wrong_site] {
            for attempts in [1, 0] {
                assert_eq!(
                    derive_source_bounds(changed, cap, attempts),
                    Err(Refusal::IterationBinding(loop_site.clone()))
                );
            }
        }
        let mut changed_actual = recipe.clone();
        let root_input = changed_actual.functions[&root].parameters[0].clone();
        let Kind::Iterate { body, .. } = &mut iteration_mut(&mut changed_actual, root).kind else {
            panic!("fixture iterator");
        };
        let Kind::Call {
            arguments,
            evaluation,
            ..
        } = &mut body.kind
        else {
            panic!("fixture helper call");
        };
        assert_eq!(arguments[0], evaluation[0].value);
        arguments[0] = root_input;
        assert_ne!(arguments[0], evaluation[0].value);
        let mut changed_formal = recipe.clone();
        let parameter = &mut changed_formal
            .functions
            .get_mut(&middle)
            .unwrap()
            .parameters[0];
        assert_eq!(
            parameter.origin,
            Origin::Parameter {
                function: middle,
                ordinal: 0,
            }
        );
        parameter.origin = Origin::Temporary(call_site.clone());
        for changed in [&changed_actual, &changed_formal] {
            for attempts in [1, 0] {
                assert_eq!(
                    derive_source_bounds(changed, cap, attempts),
                    Err(Refusal::CallBinding(call_site.clone()))
                );
            }
        }
    }

    // Generated-ABI-shaped DATA fixtures only: no factory root, source window,
    // selected-owner receipt or installed cap is manufactured by these tests.
    #[derive(Clone, Copy)]
    enum ParameterFixture {
        Boolean,
        ObjectChildren,
        ObjectAncestor,
    }

    fn parameterized_fixture(shape: ParameterFixture) -> Recipe {
        let objects = !matches!(shape, ParameterFixture::Boolean);
        let argument: syn::Expr = if matches!(shape, ParameterFixture::ObjectAncestor) {
            syn::parse_quote!(instance)
        } else {
            syn::parse_quote!(member)
        };
        let entry: syn::Block = if objects {
            syn::parse_quote!({
                __cache.reset();
                match instance {
                    serde_json::Value::Object(obj) =>
                        obj.iter().all(|(key, member)| middle(#argument, __cache)),
                    _ => false,
                }
            })
        } else {
            syn::parse_quote!({
                __cache.reset();
                leaf(instance, __cache)
            })
        };
        let middle: Option<syn::ItemFn> = objects.then(|| {
            syn::parse_quote! {
                fn middle(instance: &serde_json::Value, __cache: &mut InnerCache) -> bool {
                    match instance {
                        serde_json::Value::Object(obj) =>
                            obj.iter().all(|(key, member)| leaf(member, __cache)),
                        _ => false,
                    }
                }
            }
        });
        let syntax: syn::File = syn::parse_quote! {
            struct InnerCache {}
            struct OuterCache { __inner: InnerCache }
            impl InnerCache { fn reset(&mut self) {} }
            fn is_valid(instance: &serde_json::Value, __cache: &mut OuterCache) -> bool {
                entry_is_valid(instance, &mut __cache.__inner)
            }
            fn entry_is_valid(instance: &serde_json::Value, __cache: &mut InnerCache) -> bool #entry
            #middle
            fn leaf(instance: &serde_json::Value, __cache: &mut InnerCache) -> bool {
                instance.is_boolean()
            }
        };
        let recipe = operations::classify_source_body(&syntax, &[], &[])
            .expect("ABI-shaped fixture passes the existing classifier")
            .recipe()
            .clone();
        assert_eq!(recipe.roots, vec![function(&recipe, "is_valid")]);
        assert!(recipe.initialization.is_empty());
        assert_eq!(recipe.functions.len(), if objects { 5 } else { 4 });
        recipe
    }

    struct FixtureCall {
        bound: CallBound,
        arguments: Vec<Binding>,
    }

    fn parameter_call(
        recipe: &Recipe,
        source_function: &str,
        target_function: &str,
        per_attempt: SourceBoundCoeff,
    ) -> FixtureCall {
        let caller_id = function(recipe, source_function);
        let target = function(recipe, target_function);
        let parameters = &recipe.functions[&target].parameters;
        let mut found = Vec::new();
        model::walk(&recipe.functions[&caller_id].body, &mut |op| {
            if let Kind::Call {
                target: actual,
                arguments,
                evaluation,
            } = &op.kind
                && *actual == target
            {
                assert_eq!(op.site.function, caller_id);
                assert_eq!(arguments.len(), parameters.len());
                assert_eq!(evaluation.len(), parameters.len());
                for (ordinal, ((argument, expression), formal)) in
                    arguments.iter().zip(evaluation).zip(parameters).enumerate()
                {
                    assert_eq!(argument, &expression.value);
                    assert_eq!(argument.ty, formal.ty);
                    assert!(argument.borrowed && formal.borrowed);
                    assert_eq!(
                        formal.origin,
                        Origin::Parameter {
                            function: target,
                            ordinal
                        }
                    );
                    assert!(formal.declaration.is_none());
                }
                found.push(FixtureCall {
                    bound: CallBound {
                        site: op.site.clone(),
                        target,
                        formal_ordinal: 0,
                        formal: parameters[0].clone(),
                        actual: arguments[0].clone(),
                        per_attempt,
                    },
                    arguments: arguments.clone(),
                });
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(found.len(), 1);
        found.pop().unwrap()
    }

    fn assert_parameter_signature(recipe: &Recipe, name: &str, cache: &str) {
        let id = function(recipe, name);
        let function = &recipe.functions[&id];
        assert_eq!(function.result, Ty::Bool);
        assert_eq!(function.parameters.len(), 2);
        for (ordinal, ty) in [Ty::Value, Ty::Cache(cache.to_owned())]
            .into_iter()
            .enumerate()
        {
            let parameter = &function.parameters[ordinal];
            assert_eq!(parameter.ty, ty);
            assert!(parameter.borrowed);
            assert!(parameter.declaration.is_none());
            assert_eq!(
                parameter.origin,
                Origin::Parameter {
                    function: id,
                    ordinal
                }
            );
        }
    }

    fn assert_wrapper_borrow(recipe: &Recipe) {
        let root = function(recipe, "is_valid");
        let entry = function(recipe, "entry_is_valid");
        let cache = &recipe.functions[&root].parameters[1];
        let mut visits = 0_u64;
        model::walk(&recipe.functions[&root].body, &mut |op| {
            if let Kind::Call {
                target, evaluation, ..
            } = &op.kind
                && *target == entry
            {
                visits = visits.checked_add(1).unwrap();
                let Kind::Evaluate(model::Action::Borrow { mutable: true }, borrowed) =
                    &evaluation[1].kind
                else {
                    panic!("mutable wrapper cache borrow");
                };
                assert_eq!(borrowed.len(), 1);
                let Kind::Evaluate(model::Action::Field(field), base) = &borrowed[0].kind else {
                    panic!("wrapper cache field projection");
                };
                assert_eq!(field, "__inner");
                assert_eq!(base.len(), 1);
                assert_eq!(&base[0].value, cache);
                assert!(matches!(&base[0].kind, Kind::Value(model::Atom::Place)));
                assert_eq!(evaluation[1].value, borrowed[0].value);
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(visits, 1);
    }

    struct ObjectIteration {
        site: Site,
        collection: Binding,
        parameters: Vec<Binding>,
    }

    fn object_iteration(recipe: &Recipe, owner: &str) -> ObjectIteration {
        let id = function(recipe, owner);
        let mut found = Vec::new();
        model::walk(&recipe.functions[&id].body, &mut |op| {
            if let Kind::Iterate {
                protocol,
                callee,
                stop,
                none_on_exhaustion,
                bound,
                receiver,
                parameters,
                body,
                evaluation,
            } = &op.kind
            {
                assert_eq!(*protocol, operations::Primitive::IteratorProtocol);
                assert_eq!(callee, "Iterator::all");
                assert_eq!(*stop, StopRule::FirstFalseOrExhaustion);
                assert!(*none_on_exhaustion);
                assert_eq!(bound.collection.ty, Ty::Map);
                assert!(bound.collection.borrowed);
                assert_eq!(bound.element, Ty::Tuple(vec![Ty::String, Ty::Value]));
                assert_eq!(receiver, &evaluation.value);
                let Kind::Primitive {
                    callee: iterator,
                    receiver: Some(collection),
                    arguments,
                    source_calls,
                    ..
                } = &evaluation.kind
                else {
                    panic!("map iterator receiver");
                };
                assert_eq!(iterator, "Map::iter");
                assert_eq!(collection, &bound.collection);
                assert_eq!(arguments.len(), 1);
                assert_eq!(arguments[0].value, *collection);
                assert!(source_calls.is_empty());
                assert_eq!(parameters.len(), 2);
                for (ordinal, (parameter, ty)) in
                    parameters.iter().zip([Ty::String, Ty::Value]).enumerate()
                {
                    assert_eq!(parameter.ty, ty);
                    assert!(parameter.borrowed);
                    let mut declaration = body.site.clone();
                    declaration.lexical.push(ordinal);
                    assert_eq!(parameter.declaration.as_ref(), Some(&declaration));
                    assert_eq!(
                        parameter.origin,
                        Origin::Projection {
                            base: Box::new(Origin::Projection {
                                base: Box::new(collection.origin.clone()),
                                member: "member".to_owned(),
                            }),
                            member: ordinal.to_string(),
                        }
                    );
                }
                found.push(ObjectIteration {
                    site: op.site.clone(),
                    collection: collection.clone(),
                    parameters: parameters.clone(),
                });
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(found.len(), 1);
        found.pop().unwrap()
    }

    fn assert_parameter_bindings(recipe: &Recipe, shape: ParameterFixture) {
        let objects = !matches!(shape, ParameterFixture::Boolean);
        for name in ["entry_is_valid", "leaf"] {
            assert_parameter_signature(recipe, name, "InnerCache");
        }
        assert_parameter_signature(recipe, "is_valid", "OuterCache");
        assert_wrapper_borrow(recipe);
        let root = function(recipe, "is_valid");
        let entry = function(recipe, "entry_is_valid");
        let root_parameters = &recipe.functions[&root].parameters;
        let entry_parameters = &recipe.functions[&entry].parameters;
        let constant = SourceBoundCoeff {
            constant: 1,
            nodes: 0,
        };
        let wrapper = parameter_call(recipe, "is_valid", "entry_is_valid", constant);
        assert_eq!(
            wrapper.arguments,
            vec![
                root_parameters[0].clone(),
                Binding {
                    ty: Ty::Cache("InnerCache".to_owned()),
                    place: "__cache.__inner".to_owned(),
                    borrowed: true,
                    origin: Origin::Projection {
                        base: Box::new(root_parameters[1].origin.clone()),
                        member: "__inner".to_owned()
                    },
                    declaration: None,
                }
            ]
        );
        let reset = parameter_call(recipe, "entry_is_valid", "InnerCache::reset", constant);
        assert_eq!(reset.arguments, vec![entry_parameters[1].clone()]);
        assert_eq!(reset.bound.formal.ty, Ty::Cache("InnerCache".to_owned()));
        assert_eq!(recipe.functions[&reset.bound.target].result, Ty::Unit);
        if objects {
            assert_object_bindings(recipe, shape);
        } else {
            assert_eq!(
                parameter_call(recipe, "entry_is_valid", "leaf", constant).arguments,
                vec![entry_parameters[0].clone(), entry_parameters[1].clone()]
            );
        }
    }

    fn assert_object_bindings(recipe: &Recipe, shape: ParameterFixture) {
        assert_parameter_signature(recipe, "middle", "InnerCache");
        let upper = object_iteration(recipe, "entry_is_valid");
        let lower = object_iteration(recipe, "middle");
        for (owner, iteration) in [("entry_is_valid", &upper), ("middle", &lower)] {
            let id = function(recipe, owner);
            assert_eq!(
                iteration.collection.origin,
                Origin::Projection {
                    base: Box::new(recipe.functions[&id].parameters[0].origin.clone()),
                    member: "serde_json::Value::Object".to_owned(),
                }
            );
        }
        let entry = function(recipe, "entry_is_valid");
        let middle = function(recipe, "middle");
        let entry_parameters = &recipe.functions[&entry].parameters;
        let middle_parameters = &recipe.functions[&middle].parameters;
        let selected = if matches!(shape, ParameterFixture::ObjectAncestor) {
            entry_parameters[0].clone()
        } else {
            upper.parameters[1].clone()
        };
        let linear = SourceBoundCoeff {
            constant: 0,
            nodes: 1,
        };
        assert_eq!(
            parameter_call(recipe, "entry_is_valid", "middle", linear).arguments,
            vec![selected, entry_parameters[1].clone()]
        );
        assert_eq!(
            parameter_call(recipe, "middle", "leaf", linear).arguments,
            vec![lower.parameters[1].clone(), middle_parameters[1].clone()]
        );
        assert_ne!(upper.parameters[0].ty, upper.parameters[1].ty);
    }

    // Expected coefficients are literal inputs. Only identities/bindings are
    // projected from Recipe; no expected coefficient calls the deriver.
    fn parameterized_expected(
        recipe: &Recipe,
        cap: NodeCapData,
        attempts: u64,
        object_coefficients: Option<(u64, u64)>,
    ) -> SourceBounds {
        let constant = SourceBoundCoeff {
            constant: 1,
            nodes: 0,
        };
        let mut calls = vec![
            parameter_call(recipe, "is_valid", "entry_is_valid", constant).bound,
            parameter_call(recipe, "entry_is_valid", "InnerCache::reset", constant).bound,
        ];
        let callbacks = if let Some((leaf_nodes, callback_nodes)) = object_coefficients {
            calls.push(
                parameter_call(
                    recipe,
                    "entry_is_valid",
                    "middle",
                    SourceBoundCoeff {
                        constant: 0,
                        nodes: 1,
                    },
                )
                .bound,
            );
            calls.push(
                parameter_call(
                    recipe,
                    "middle",
                    "leaf",
                    SourceBoundCoeff {
                        constant: 0,
                        nodes: leaf_nodes,
                    },
                )
                .bound,
            );
            SourceBoundCoeff {
                constant: 0,
                nodes: callback_nodes,
            }
        } else {
            calls.push(parameter_call(recipe, "entry_is_valid", "leaf", constant).bound);
            SourceBoundCoeff {
                constant: 0,
                nodes: 0,
            }
        };
        SourceBounds {
            roots: recipe.roots.clone(),
            cap,
            attempts,
            helper_calls: calls,
            iterator_callbacks_per_attempt: callbacks,
            required_sources: requirements(),
        }
    }

    #[test]
    fn parameterized_calls_preserve_instance_and_cache_bindings() {
        let recipe = parameterized_fixture(ParameterFixture::Boolean);
        assert_parameter_bindings(&recipe, ParameterFixture::Boolean);
        let cap = NodeCapData { nodes: 7 };
        assert_eq!(measured_nodes(&json!(true), cap), 1);
        let literal = parameterized_expected(&recipe, cap, 1, None);
        assert_eq!(literal.helper_calls.len(), 3);
        assert_eq!(
            literal
                .helper_calls
                .iter()
                .map(|call| call.per_attempt)
                .collect::<Vec<_>>(),
            vec![
                SourceBoundCoeff {
                    constant: 1,
                    nodes: 0
                };
                3
            ]
        );
        assert_eq!(
            literal.iterator_callbacks_per_attempt,
            SourceBoundCoeff {
                constant: 0,
                nodes: 0
            }
        );
        assert_eq!(derive_source_bounds(&recipe, cap, 1), Ok(literal));
        assert_eq!(
            derive_source_bounds(&recipe, cap, 0),
            Ok(parameterized_expected(&recipe, cap, 0, None))
        );
    }

    #[test]
    fn object_entry_values_partition_across_parameterized_helpers() {
        let recipe = parameterized_fixture(ParameterFixture::ObjectChildren);
        assert_parameter_bindings(&recipe, ParameterFixture::ObjectChildren);
        let cap = NodeCapData { nodes: 17 };
        let witness = json!({"a": {"x": true, "y": true}, "b": {"z": true}});
        let nodes = measured_nodes(&witness, cap);
        assert_eq!(nodes, 11);
        let rows = witness.as_object().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows["a"].as_object().unwrap().len(), 2);
        assert_eq!(rows["b"].as_object().unwrap().len(), 1);
        assert_eq!(
            rows["a"]
                .as_object()
                .unwrap()
                .len()
                .checked_add(rows["b"].as_object().unwrap().len()),
            Some(3)
        );
        assert_eq!(rows.len().checked_add(3), Some(5));
        let literal = parameterized_expected(&recipe, cap, 1, Some((1, 1)));
        assert_eq!(literal.helper_calls.len(), 4);
        assert_eq!(at_nodes(literal.helper_calls[2].per_attempt, nodes), 11);
        assert_eq!(at_nodes(literal.helper_calls[3].per_attempt, nodes), 11);
        assert_eq!(at_nodes(literal.iterator_callbacks_per_attempt, nodes), 11);
        // Actual 2/3 helper entries and five callbacks are below these ceilings;
        // the two wrapper/reset entries remain separate constant occurrences.
        assert_eq!(derive_source_bounds(&recipe, cap, 1), Ok(literal));
        assert_eq!(
            derive_source_bounds(&recipe, cap, 0),
            Ok(parameterized_expected(&recipe, cap, 0, Some((1, 1))))
        );
    }

    #[test]
    fn object_ancestor_rescans_require_key_funded_cap_lifting() {
        let recipe = parameterized_fixture(ParameterFixture::ObjectAncestor);
        assert_parameter_bindings(&recipe, ParameterFixture::ObjectAncestor);
        let cap = NodeCapData { nodes: 13 };
        let witness = json!({"a": true, "b": true, "c": true});
        let nodes = measured_nodes(&witness, cap);
        assert_eq!((cap.nodes, nodes), (13, 7));
        let members = u64::try_from(witness.as_object().unwrap().len()).unwrap();
        assert_eq!(members, 3);
        assert_eq!(members.checked_mul(members), Some(9));
        assert_eq!(members.checked_add(9), Some(12));
        assert!(9 > nodes);
        // Each map member funds a key and a value. M(13)=6, M(19)=9.
        // n*n <= M(C)*J; n+n*n <= (1+M(C))*J, not exact witness counts.
        let literal = parameterized_expected(&recipe, cap, 1, Some((6, 7)));
        assert_eq!(at_nodes(literal.helper_calls[3].per_attempt, nodes), 42);
        assert_eq!(at_nodes(literal.iterator_callbacks_per_attempt, nodes), 49);
        assert_eq!(derive_source_bounds(&recipe, cap, 1), Ok(literal));
        let larger = NodeCapData { nodes: 19 };
        assert_eq!(measured_nodes(&witness, larger), 7);
        let larger_literal = parameterized_expected(&recipe, larger, 1, Some((9, 10)));
        assert_eq!(
            at_nodes(larger_literal.helper_calls[3].per_attempt, nodes),
            63
        );
        assert_eq!(
            at_nodes(larger_literal.iterator_callbacks_per_attempt, nodes),
            70
        );
        assert_eq!(derive_source_bounds(&recipe, larger, 1), Ok(larger_literal));
        assert_eq!(
            derive_source_bounds(&recipe, cap, 0),
            Ok(parameterized_expected(&recipe, cap, 0, Some((6, 7))))
        );
        for attempts in [1, 0] {
            assert_eq!(
                derive_source_bounds(&recipe, NodeCapData { nodes: u64::MAX }, attempts),
                Err(Refusal::Overflow)
            );
        }
    }

    // These are fixture-shape accessors, never a second source recognizer.
    fn wrapper_call_mut(recipe: &mut Recipe) -> &mut Op {
        let id = function(recipe, "is_valid");
        let Kind::Scope { steps, .. } = &mut recipe.functions.get_mut(&id).unwrap().body.kind
        else {
            panic!("fixture wrapper scope");
        };
        assert_eq!(steps.len(), 1);
        assert!(matches!(&steps[0].kind, Kind::Call { .. }));
        &mut steps[0]
    }

    fn object_iteration_mut(recipe: &mut Recipe) -> &mut Op {
        let id = function(recipe, "entry_is_valid");
        let Kind::Scope { steps, .. } = &mut recipe.functions.get_mut(&id).unwrap().body.kind
        else {
            panic!("fixture entry scope");
        };
        assert_eq!(steps.len(), 2);
        let Kind::Choice { arms, .. } = &mut steps[1].kind else {
            panic!("fixture object match");
        };
        assert_eq!(arms.len(), 2);
        assert!(matches!(&arms[0].body.kind, Kind::Iterate { .. }));
        &mut arms[0].body
    }

    #[derive(Clone, Copy)]
    enum CacheDrift {
        ArgumentOnly,
        InventedOrigin,
        FormalOrigin,
    }

    fn cache_mutant(recipe: &Recipe, drift: CacheDrift, site: &Site) -> Recipe {
        let mut changed = recipe.clone();
        if matches!(drift, CacheDrift::FormalOrigin) {
            let id = function(&changed, "entry_is_valid");
            let formal = &mut changed.functions.get_mut(&id).unwrap().parameters[1];
            assert_eq!(
                formal.origin,
                Origin::Parameter {
                    function: id,
                    ordinal: 1
                }
            );
            formal.origin = Origin::Temporary(site.clone());
        } else {
            let Kind::Call {
                arguments,
                evaluation,
                ..
            } = &mut wrapper_call_mut(&mut changed).kind
            else {
                panic!("fixture wrapper call");
            };
            assert_eq!(arguments.len(), 2);
            assert_eq!(arguments[1], evaluation[1].value);
            assert!(
                matches!(&arguments[1].origin, Origin::Projection { member, .. } if member == "__inner")
            );
            arguments[1].origin = Origin::Temporary(site.clone());
            if matches!(drift, CacheDrift::InventedOrigin) {
                evaluation[1].value = arguments[1].clone();
                assert_eq!(arguments[1], evaluation[1].value);
            } else {
                assert_ne!(arguments[1], evaluation[1].value);
            }
        }
        changed
    }

    #[derive(Clone, Copy)]
    enum ObjectDrift {
        KeySlot,
        ValueSlot,
        Declaration,
        Unborrowed,
        Protocol,
    }

    fn object_mutant(recipe: &Recipe, drift: ObjectDrift) -> Recipe {
        let mut changed = recipe.clone();
        let Kind::Iterate {
            bound,
            parameters,
            callee,
            body,
            ..
        } = &mut object_iteration_mut(&mut changed).kind
        else {
            panic!("fixture object iterator");
        };
        assert_eq!(parameters.len(), 2);
        match drift {
            ObjectDrift::KeySlot | ObjectDrift::ValueSlot => {
                let (ordinal, before, after) = if matches!(drift, ObjectDrift::KeySlot) {
                    (0, "0", "1")
                } else {
                    (1, "1", "0")
                };
                let Kind::Call { .. } = &body.kind else {
                    panic!("fixture member call");
                };
                let Origin::Projection { member, .. } = &mut parameters[ordinal].origin else {
                    panic!("fixture tuple member origin");
                };
                assert_eq!(member.as_str(), before);
                *member = after.to_owned();
                assert_eq!(parameters[0].ty, Ty::String);
                assert_eq!(parameters[1].ty, Ty::Value);
            }
            ObjectDrift::Declaration => {
                let mut before = body.site.clone();
                before.lexical.push(1);
                assert_eq!(parameters[1].declaration.as_ref(), Some(&before));
                parameters[1].declaration = Some(Site {
                    function: before.function,
                    lexical: vec![999],
                });
            }
            ObjectDrift::Unborrowed => {
                assert!(bound.collection.borrowed);
                bound.collection.borrowed = false;
            }
            ObjectDrift::Protocol => {
                assert_eq!(callee.as_str(), "Iterator::all");
                *callee = "Iterator::any".to_owned();
            }
        }
        changed
    }

    #[test]
    fn cache_and_object_binder_drift_are_independently_refused() {
        let recipe = parameterized_fixture(ParameterFixture::ObjectChildren);
        assert_parameter_bindings(&recipe, ParameterFixture::ObjectChildren);
        let cap = NodeCapData { nodes: 17 };
        assert_eq!(
            derive_source_bounds(&recipe, cap, 1),
            Ok(parameterized_expected(&recipe, cap, 1, Some((1, 1))))
        );
        let wrapper = parameter_call(
            &recipe,
            "is_valid",
            "entry_is_valid",
            SourceBoundCoeff {
                constant: 1,
                nodes: 0,
            },
        )
        .bound
        .site;
        for drift in [
            CacheDrift::ArgumentOnly,
            CacheDrift::InventedOrigin,
            CacheDrift::FormalOrigin,
        ] {
            let changed = cache_mutant(&recipe, drift, &wrapper);
            for attempts in [1, 0] {
                assert_eq!(
                    derive_source_bounds(&changed, cap, attempts),
                    Err(Refusal::CallBinding(wrapper.clone()))
                );
            }
        }
        let site = object_iteration(&recipe, "entry_is_valid").site;
        for drift in [
            ObjectDrift::KeySlot,
            ObjectDrift::ValueSlot,
            ObjectDrift::Declaration,
            ObjectDrift::Unborrowed,
            ObjectDrift::Protocol,
        ] {
            let changed = object_mutant(&recipe, drift);
            for attempts in [1, 0] {
                assert_eq!(
                    derive_source_bounds(&changed, cap, attempts),
                    Err(Refusal::IterationBinding(site.clone()))
                );
            }
        }
    }
}

#[cfg(test)]
mod coupled_tests {
    use super::{
        ConstantFactoryBounds, ConstantHelperBound, CoupledDuty, CoupledFactoryBounds,
        CoupledPopulation, CoupledRefusal, Kind, NodeCapData, Op, OptionalCallbackBound, Origin,
        Primitive, ProgramFactoryBound, Recipe, Refusal, Site, coupled_factory_bounds,
        derive_source_bounds,
    };
    use crate::schema_validation::{operations, storage};
    use auctioneer_bidder::schema::{SchemaInputLimits, measure_schema_input};
    use quote::{ToTokens as _, quote};
    use serde_json::{Value, json};
    use std::collections::{BTreeMap, BTreeSet};

    const OWNER: &str = "fake_coupled_owner";
    const MARKER: &str = "Coupled";
    const INITIALIZER: &str = "__coupled_impl::initialize_regexes";
    const FACTORY: &str = "__coupled_impl::new_search_cache";
    const PUBLIC_FACTORY: &str = "Coupled::new_search_cache";
    const PUBLIC_VALIDATION: &str = "Coupled::is_valid";

    struct Fixture {
        syntax: syn::File,
        recipe: Recipe,
        graph: Value,
    }

    fn programs(two: bool) -> Vec<String> {
        if two {
            vec![
                "__coupled_impl::PATTERN_0_REGEX".to_owned(),
                "__coupled_impl::PATTERN_1_REGEX".to_owned(),
            ]
        } else {
            vec![]
        }
    }

    fn program_tokens(two: bool) -> proc_macro2::TokenStream {
        if !two {
            return quote! {};
        }
        quote! {
            static PATTERN_0_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| {
                __ra::meta::Regex::builder()
                    .configure(__ra::meta::Regex::config()
                        .match_kind(__ra::MatchKind::LeftmostFirst).utf8_empty(true)
                        .nfa_size_limit(Some(65_536)).hybrid_cache_capacity(65_536).pool_capacity(1))
                    .syntax(__ra::util::syntax::Config::default().utf8(true)).build("a").ok()
            });
            static PATTERN_1_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| {
                __ra::meta::Regex::builder()
                    .configure(__ra::meta::Regex::config()
                        .match_kind(__ra::MatchKind::LeftmostFirst).utf8_empty(true)
                        .nfa_size_limit(Some(65_536)).hybrid_cache_capacity(65_536).pool_capacity(1))
                    .syntax(__ra::util::syntax::Config::default().utf8(true)).build("b").ok()
            });
        }
    }

    fn coupled_syntax(two: bool) -> syn::File {
        let statics = program_tokens(two);
        let (fields, initializers, readiness) = if two {
            (
                quote! { pattern_0_cache: __ra::meta::Cache, pattern_1_cache: __ra::meta::Cache, },
                quote! {
                    pattern_0_cache: PATTERN_0_REGEX.as_ref()?.create_cache(),
                    pattern_1_cache: PATTERN_1_REGEX.as_ref()?.create_cache(),
                },
                quote! {
                    ready &= PATTERN_0_REGEX.as_ref().is_some();
                    ready &= PATTERN_1_REGEX.as_ref().is_some();
                },
            )
        } else {
            (quote! {}, quote! {}, quote! {})
        };
        syn::parse2(quote! {
            struct Coupled;
            struct CoupledSearchCache { __inner: __coupled_impl::__SearchCache }
            impl Coupled {
                #[doc = " Initializes every generated pattern regex, including after a"]
                #[doc = " failed constructor. Returns false if any cached construction"]
                #[doc = " failed. Repeated calls do not reconstruct or retry regexes."]
                #[doc = " This does not initialize custom formats or bound search caches."]
                pub fn initialize_regexes() -> bool { __coupled_impl::initialize_regexes() }
                pub fn new_search_cache() -> Option<CoupledSearchCache> {
                    __coupled_impl::new_search_cache().map(|__inner| CoupledSearchCache { __inner })
                }
                pub fn is_valid(instance: &serde_json::Value, __cache: &mut CoupledSearchCache) -> bool {
                    { __coupled_impl::entry_is_valid(instance, &mut __cache.__inner) }
                }
            }
            #[doc(hidden)]
            #[allow(non_snake_case, dead_code, unused_imports, unused_variables, unreachable_code, clippy::all)]
            mod __coupled_impl {
                use regex_automata as __ra;
                use std::sync::LazyLock as __Lazy;
                #statics
                pub(super) struct __SearchCache { #fields }
                impl __SearchCache { fn reset(&mut self) {} }
                pub(super) fn initialize_regexes() -> bool {
                    let mut ready = true;
                    #readiness
                    ready
                }
                pub(super) fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache { #initializers })
                }
                pub(super) fn entry_is_valid(instance: &serde_json::Value, __cache: &mut __SearchCache) -> bool {
                    __cache.reset();
                    match instance {
                        serde_json::Value::Array(arr) => arr.iter().all(|instance| leaf(instance, __cache)),
                        _ => instance.is_boolean(),
                    }
                }
                fn leaf(instance: &serde_json::Value, __cache: &mut __SearchCache) -> bool {
                    instance.is_boolean()
                }
            }
        }).unwrap()
    }

    fn graph() -> Value {
        // Auxiliary DATA only, never a descriptor or source-authority constructor.
        json!({"auxiliary":{
            "dispatch_edges":[
                {"from":"validate_native","to":"Coupled","role":"validation-call-cache-factory"},
                {"from":"validate_native","to":"Coupled","role":"is-valid-with-call-cache"}
            ],
            "roots":[{"kind":"inline-retention-row",
                "details":{"owner":OWNER,"marker":MARKER,"cache":"CoupledSearchCache"}}]
        }})
    }

    fn fixture(two: bool) -> Fixture {
        let syntax = coupled_syntax(two);
        let classified = operations::classify_source_body(&syntax, &[], &[]);
        assert!(
            classified.is_ok(),
            "canonical fixture classification: {classified:?}"
        );
        let recipe = classified.unwrap().recipe().clone();
        let graph = graph();
        let schedule_result = storage::owner_schedule_data(&recipe, &graph, OWNER, MARKER);
        assert!(
            schedule_result.is_ok(),
            "existing factory owner setup: {schedule_result:?}"
        );
        let schedule = schedule_result.unwrap();
        assert_eq!(
            schedule.roots(),
            (
                function(&recipe, PUBLIC_FACTORY),
                function(&recipe, PUBLIC_VALIDATION)
            )
        );
        assert_eq!(
            recipe.roots,
            vec![
                function(&recipe, PUBLIC_VALIDATION),
                function(&recipe, PUBLIC_FACTORY)
            ]
        );
        assert_eq!(
            recipe.initialization,
            BTreeMap::from([(INITIALIZER.to_owned(), programs(two))])
        );
        let fields: Vec<_> = schedule
            .initializers()
            .map(|(field, program, _, prior)| {
                (field.to_owned(), program.to_owned(), prior.to_vec())
            })
            .collect();
        let expected = if two {
            vec![
                (
                    "pattern_0_cache".to_owned(),
                    programs(true)[0].clone(),
                    vec![],
                ),
                (
                    "pattern_1_cache".to_owned(),
                    programs(true)[1].clone(),
                    vec!["pattern_0_cache".to_owned()],
                ),
            ]
        } else {
            vec![]
        };
        assert_eq!(fields, expected);
        assert!(schedule.callbacks().next().is_none());
        Fixture {
            syntax,
            recipe,
            graph,
        }
    }

    fn function(recipe: &Recipe, name: &str) -> usize {
        let found: Vec<_> = recipe
            .functions
            .iter()
            .filter(|(_, function)| function.identity == name)
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(found.len(), 1, "one fixture function: {name}");
        found[0]
    }

    fn site(recipe: &Recipe, owner: &str, mut predicate: impl FnMut(&Op) -> bool) -> Site {
        let mut found = Vec::new();
        operations::model::walk(
            &recipe.functions[&function(recipe, owner)].body,
            &mut |op| {
                if predicate(op) {
                    found.push(op.site.clone());
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(found.len(), 1, "one independent source site in {owner}");
        found.pop().unwrap()
    }

    fn helper(
        recipe: &Recipe,
        source_function: &str,
        target_function: &str,
        upper: u64,
    ) -> ConstantHelperBound {
        let target = function(recipe, target_function);
        ConstantHelperBound {
            site: site(
                recipe,
                source_function,
                |op| matches!(&op.kind, Kind::Call { target: id, .. } if *id == target),
            ),
            target,
            upper,
        }
    }

    fn program_bounds(recipe: &Recipe, two: bool, upper: u64) -> Vec<ProgramFactoryBound> {
        programs(two)
            .into_iter()
            .enumerate()
            .map(|(ordinal, program)| {
                let from_program = |op: &Op, lemma, callee: &str| {
                    matches!(&op.kind,
                Kind::Primitive { lemma: actual, callee: name, receiver: Some(receiver), .. }
                if *actual == lemma && name.ends_with(callee)
                    && receiver.origin == Origin::Static(program.clone()))
                };
                ProgramFactoryBound {
                    field: format!("pattern_{ordinal}_cache"),
                    initializer_probe: site(recipe, INITIALIZER, |op| {
                        from_program(op, Primitive::Readiness, "::as_ref")
                    }),
                    constructor: site(recipe, FACTORY, |op| {
                        from_program(op, Primitive::CacheConstruction, "Regex::create_cache")
                    }),
                    completed_siblings: if ordinal == 0 {
                        vec![]
                    } else {
                        vec!["pattern_0_cache".to_owned()]
                    },
                    program,
                    initializer_checks_upper: upper,
                    constructor_calls_upper: upper,
                }
            })
            .collect()
    }

    fn duties() -> BTreeSet<CoupledDuty> {
        BTreeSet::from([
            CoupledDuty::CallerPopulations,
            CoupledDuty::SourceBinding,
            CoupledDuty::ValidationTraversal,
            CoupledDuty::SearchMultiplicity,
            CoupledDuty::PrimitiveSourceCalls,
            CoupledDuty::OrdinaryWork,
            CoupledDuty::OrdinaryCleanup,
            CoupledDuty::PartialConstructionCleanup,
        ])
    }

    fn expected(
        fixture: &Fixture,
        two: bool,
        cap: u64,
        population: CoupledPopulation,
        root_total: u64,
        readiness_total: u64,
    ) -> CoupledFactoryBounds {
        let recipe = &fixture.recipe;
        let factory = population.factories;
        CoupledFactoryBounds {
            roots: recipe.roots.clone(),
            initialization: recipe.initialization.clone(),
            population,
            node_cap: cap,
            root_entries_upper: root_total,
            validation_root: function(recipe, PUBLIC_VALIDATION),
            factory: ConstantFactoryBounds {
                root: function(recipe, PUBLIC_FACTORY),
                entries_upper: factory,
                helpers: vec![
                    helper(recipe, PUBLIC_FACTORY, FACTORY, factory),
                    helper(recipe, FACTORY, INITIALIZER, factory),
                ],
                wrapper_map: OptionalCallbackBound {
                    site: site(recipe, PUBLIC_FACTORY, |op| {
                        matches!(&op.kind,
                        Kind::Callback { callee, .. } if callee == "Option::map")
                    }),
                    minimum: 0,
                    maximum: factory,
                },
                initializer_checks_upper: readiness_total,
                programs: program_bounds(recipe, two, factory),
                completed_prefixes: if two {
                    vec![
                        vec![],
                        vec!["pattern_0_cache".to_owned()],
                        vec!["pattern_0_cache".to_owned(), "pattern_1_cache".to_owned()],
                    ]
                } else {
                    vec![vec![]]
                },
            },
            input_partition: None,
            required: duties(),
        }
    }

    fn derive(
        fixture: &Fixture,
        cap: u64,
        population: CoupledPopulation,
    ) -> Result<CoupledFactoryBounds, CoupledRefusal> {
        coupled_factory_bounds(
            &fixture.recipe,
            &fixture.graph,
            OWNER,
            MARKER,
            cap,
            population,
        )
    }

    #[test]
    fn coupled_factories_keep_constant_work_separate_from_input_traversals() {
        for (two, readiness) in [(false, 0), (true, 6)] {
            let fixture = fixture(two);
            let population = CoupledPopulation {
                factories: 3,
                validations: 2,
            };
            assert_eq!(
                derive(&fixture, 7, population),
                Ok(expected(&fixture, two, 7, population, 5, readiness))
            );
            let seven = derive(&fixture, 7, population).unwrap();
            let eleven = derive(&fixture, 11, population).unwrap();
            assert_eq!(seven.factory, eleven.factory);
            assert_eq!((seven.node_cap, eleven.node_cap), (7, 11));
            assert_eq!(seven.factory.entries_upper, 3);
            assert_eq!(
                seven
                    .factory
                    .helpers
                    .iter()
                    .map(|row| row.upper)
                    .collect::<Vec<_>>(),
                [3, 3]
            );
            assert_eq!(
                (
                    seven.factory.wrapper_map.minimum,
                    seven.factory.wrapper_map.maximum
                ),
                (0, 3)
            );
            for row in &seven.factory.programs {
                assert_eq!(
                    (row.initializer_checks_upper, row.constructor_calls_upper),
                    (3, 3)
                );
            }
            let value = json!([true, true, true]);
            let measured = measure_schema_input(
                &value,
                SchemaInputLimits {
                    serialized_bytes: 0x0001_0000,
                    nodes: 7,
                    depth: 32,
                },
            )
            .unwrap();
            assert_eq!(measured.nodes, 4);
            assert_eq!(value.as_array().unwrap().len(), 3);
            assert_eq!(
                (seven.input_partition, eleven.input_partition),
                (None, None)
            );
            // The whole coupled input grammar is still unsupported, not a
            // pruned validation-only success or a fallback to the old counter.
            assert_eq!(
                derive_source_bounds(&fixture.recipe, NodeCapData { nodes: 7 }, 2),
                Err(Refusal::Source(operations::Refusal::UnsupportedSource))
            );
        }
    }

    #[test]
    fn factory_readiness_prefixes_and_source_duties_survive_zero_entries() {
        let fixture = fixture(true);
        let population = CoupledPopulation {
            factories: 3,
            validations: 2,
        };
        assert_eq!(
            derive(&fixture, 7, population),
            Ok(expected(&fixture, true, 7, population, 5, 6))
        );
        let zero = CoupledPopulation {
            factories: 0,
            validations: 0,
        };
        assert_eq!(
            derive(&fixture, 7, zero),
            Ok(expected(&fixture, true, 7, zero, 0, 0))
        );
        let bounds = derive(&fixture, 7, zero).unwrap();
        assert_eq!(bounds.factory.programs.len(), 2);
        assert_eq!(
            bounds.factory.completed_prefixes,
            [
                vec![],
                vec!["pattern_0_cache".to_owned()],
                vec!["pattern_0_cache".to_owned(), "pattern_1_cache".to_owned()]
            ]
        );
        assert_eq!(bounds.required, duties());
        assert_eq!(bounds.initialization, fixture.recipe.initialization);
        assert_eq!(bounds.roots, fixture.recipe.roots);
        assert_eq!(bounds.input_partition, None);
        let mut unknown = fixture.recipe.clone();
        let initializer = function(&unknown, INITIALIZER);
        let Kind::Scope { steps, .. } =
            &mut unknown.functions.get_mut(&initializer).unwrap().body.kind
        else {
            panic!("initializer block");
        };
        let Kind::Evaluate(_, operands) = &mut steps[1].kind else {
            panic!("eager readiness assignment");
        };
        let Kind::Primitive { arguments, .. } = &mut operands[1].kind else {
            panic!("is_some");
        };
        let probe = &mut arguments[0];
        let anchor = probe.site.clone();
        let mut unresolved = probe.clone();
        unresolved.kind = Kind::Call {
            target: usize::MAX,
            arguments: vec![],
            evaluation: vec![],
        };
        let Kind::Primitive { source_calls, .. } = &mut probe.kind else {
            panic!("readiness source probe");
        };
        assert!(source_calls.is_empty());
        source_calls.push(unresolved);
        for entries in [zero, population] {
            assert_eq!(
                coupled_factory_bounds(&unknown, &fixture.graph, OWNER, MARKER, 7, entries),
                Err(CoupledRefusal::SourceCalls(anchor.clone()))
            );
        }
    }

    fn short_circuit_initializer(fixture: &Fixture) -> Recipe {
        let mut syntax = fixture.syntax.clone();
        let module = syntax
            .items
            .iter_mut()
            .find_map(|item| {
                if let syn::Item::Mod(module) = item
                    && module.ident == "__coupled_impl"
                {
                    Some(module)
                } else {
                    None
                }
            })
            .unwrap();
        let function = module
            .content
            .as_mut()
            .unwrap()
            .1
            .iter_mut()
            .find_map(|item| {
                if let syn::Item::Fn(function) = item
                    && function.sig.ident == "initialize_regexes"
                {
                    Some(function)
                } else {
                    None
                }
            })
            .unwrap();
        let before: syn::Block = syn::parse_quote!({
            let mut ready = true;
            ready &= PATTERN_0_REGEX.as_ref().is_some();
            ready &= PATTERN_1_REGEX.as_ref().is_some();
            ready
        });
        assert_eq!(
            function.block.to_token_stream().to_string(),
            before.to_token_stream().to_string()
        );
        *function.block = syn::parse_quote!({
            PATTERN_0_REGEX.as_ref().is_some() && PATTERN_1_REGEX.as_ref().is_some()
        });
        let classified = operations::classify_source_body(&syntax, &[], &[]).unwrap();
        classified.recipe().clone()
    }

    fn assert_binding_mutants(fixture: &Fixture, population: CoupledPopulation) {
        let mut missing = fixture.recipe.clone();
        let factory = function(&missing, PUBLIC_FACTORY);
        let position = missing.roots.iter().position(|id| *id == factory).unwrap();
        assert_eq!(missing.roots.remove(position), factory);
        assert_eq!(
            coupled_factory_bounds(&missing, &fixture.graph, OWNER, MARKER, 7, population),
            Err(CoupledRefusal::RootBinding)
        );
        let mut duplicate = fixture.recipe.clone();
        duplicate.roots.push(factory);
        assert_eq!(
            coupled_factory_bounds(&duplicate, &fixture.graph, OWNER, MARKER, 7, population),
            Err(CoupledRefusal::RootBinding)
        );
        let mut initialization = fixture.recipe.clone();
        assert_eq!(
            initialization.initialization.remove(INITIALIZER),
            Some(programs(true))
        );
        assert_eq!(
            coupled_factory_bounds(
                &initialization,
                &fixture.graph,
                OWNER,
                MARKER,
                7,
                population
            ),
            Err(CoupledRefusal::Initialization)
        );
        let mut duplicate_program = fixture.recipe.clone();
        let programs = duplicate_program
            .initialization
            .get_mut(INITIALIZER)
            .unwrap();
        assert_eq!(programs.len(), 2);
        programs.push(programs[0].clone());
        assert_eq!(
            coupled_factory_bounds(
                &duplicate_program,
                &fixture.graph,
                OWNER,
                MARKER,
                7,
                population
            ),
            Err(CoupledRefusal::Initialization)
        );
        let mut graph = fixture.graph.clone();
        assert_eq!(
            graph["auxiliary"]["roots"][0]["details"]["cache"],
            "CoupledSearchCache"
        );
        graph["auxiliary"]["roots"][0]["details"]["cache"] = json!("OtherSearchCache");
        assert_eq!(
            coupled_factory_bounds(&fixture.recipe, &graph, OWNER, MARKER, 7, population),
            Err(CoupledRefusal::OwnerBinding)
        );
        let changed = short_circuit_initializer(fixture);
        storage::owner_schedule_data(&changed, &fixture.graph, OWNER, MARKER).unwrap();
        assert_eq!(
            coupled_factory_bounds(&changed, &fixture.graph, OWNER, MARKER, 7, population),
            Err(CoupledRefusal::Initializer)
        );
    }

    #[test]
    fn coupled_factory_bindings_and_overflow_refuse_independently() {
        let fixture = fixture(true);
        let population = CoupledPopulation {
            factories: 3,
            validations: 2,
        };
        assert_eq!(
            derive(&fixture, 7, population),
            Ok(expected(&fixture, true, 7, population, 5, 6))
        );
        for entries in [
            population,
            CoupledPopulation {
                factories: 0,
                validations: 0,
            },
        ] {
            assert_binding_mutants(&fixture, entries);
        }
        assert_eq!(u64::MAX.checked_mul(2), None);
        assert_eq!(
            derive(
                &fixture,
                7,
                CoupledPopulation {
                    factories: u64::MAX,
                    validations: 0
                }
            ),
            Err(CoupledRefusal::Overflow)
        );
        // Moving only validation entries must not change factory bounds.
        let no_validation = CoupledPopulation {
            factories: 3,
            validations: 0,
        };
        assert_eq!(
            derive(&fixture, 7, no_validation),
            Ok(expected(&fixture, true, 7, no_validation, 3, 6))
        );
    }

    mod input_tests {
        use super::super::{
            Action, CallBound, Kind, NodeCapData, Op, Origin, SourceBoundCoeff, SourceBounds,
            SourceRequirement, coupled_factory_bounds, coupled_input_bounds,
        };
        use super::{
            CoupledFactoryBounds, CoupledPopulation, CoupledRefusal, FACTORY, Fixture, INITIALIZER,
            MARKER, OWNER, PUBLIC_VALIDATION, Recipe, expected, fixture as factory_fixture,
            function, programs,
        };
        use crate::schema_validation::operations;
        use auctioneer_bidder::schema::{SchemaInputLimits, measure_schema_input};
        use serde_json::json;
        use std::collections::BTreeSet;

        const STRING_LEAF: &str = "__coupled_impl::string_leaf";
        const SENTINEL: &str = "__coupled_impl::sentinel";
        const KEY_LEAF: &str = "__coupled_impl::key_leaf";
        const ENTRY: &str = "__coupled_impl::entry_is_valid";
        const RESET: &str = "__coupled_impl::__SearchCache::reset";
        const LEAF: &str = "__coupled_impl::leaf";

        #[test]
        fn erased_wrappers_change_only_expression_addresses() {
            let expected = super::super::Site {
                function: 7,
                lexical: vec![0, 1, 2],
            };
            for lexical in [vec![0, 1, 2], vec![0, 1, 2, 0], vec![0, 1, 2, 0, 0]] {
                let actual = super::super::Site {
                    function: 7,
                    lexical,
                };
                assert!(super::super::expression_site(&actual, &expected));
            }
            for actual in [
                super::super::Site {
                    function: 8,
                    lexical: vec![0, 1, 2],
                },
                super::super::Site {
                    function: 7,
                    lexical: vec![0, 1],
                },
                super::super::Site {
                    function: 7,
                    lexical: vec![0, 1, 3],
                },
                super::super::Site {
                    function: 7,
                    lexical: vec![0, 1, 2, 0, 1],
                },
            ] {
                assert!(!super::super::expression_site(&actual, &expected));
            }
            let mut fixture = local_fixture();
            let scope = object_scope(&mut fixture.recipe);
            let Kind::Scope {
                steps: original_steps,
                ..
            } = &scope.kind
            else {
                panic!("object local scope");
            };
            assert!(super::super::scope_sites(scope, original_steps));
            let mut changed = scope.clone();
            let Kind::Scope {
                steps: changed_steps,
                ..
            } = &mut changed.kind
            else {
                panic!("cloned object scope");
            };
            assert!(matches!(&changed_steps[0].kind, Kind::Bind { .. }));
            changed_steps[0].site.lexical.push(0);
            let Kind::Scope {
                steps: final_steps, ..
            } = &changed.kind
            else {
                panic!("changed object scope");
            };
            assert!(!super::super::scope_sites(&changed, final_steps));
        }

        fn input(
            fixture: &Fixture,
            cap: u64,
            population: CoupledPopulation,
        ) -> Result<CoupledFactoryBounds, CoupledRefusal> {
            coupled_input_bounds(
                &fixture.recipe,
                &fixture.graph,
                OWNER,
                MARKER,
                cap,
                population,
            )
        }

        fn input_call(
            recipe: &Recipe,
            source_function: &str,
            target_function: &str,
            ordinal: usize,
            count: usize,
            coefficient: SourceBoundCoeff,
        ) -> CallBound {
            let target = function(recipe, target_function);
            let mut calls = Vec::new();
            operations::model::walk(
                &recipe.functions[&function(recipe, source_function)].body,
                &mut |op| {
                    if let Kind::Call {
                        target: id,
                        arguments,
                        evaluation,
                    } = &op.kind
                        && *id == target
                    {
                        assert_eq!(arguments.len(), evaluation.len());
                        for (argument, expression) in arguments.iter().zip(evaluation) {
                            assert_eq!(argument, &expression.value);
                        }
                        calls.push(CallBound {
                            site: op.site.clone(),
                            target,
                            formal_ordinal: 0,
                            formal: recipe.functions[&target].parameters[0].clone(),
                            actual: arguments[0].clone(),
                            per_attempt: coefficient,
                        });
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(calls.len(), count, "independent lexical call census");
            calls.remove(ordinal)
        }

        fn expected_input(
            fixture: &Fixture,
            two: bool,
            locals: bool,
            cap: u64,
            population: CoupledPopulation,
            totals: (u64, u64),
        ) -> CoupledFactoryBounds {
            let recipe = &fixture.recipe;
            let constant = SourceBoundCoeff {
                constant: 1,
                nodes: 0,
            };
            let member = SourceBoundCoeff {
                constant: 0,
                nodes: 1,
            };
            let mut calls = vec![
                input_call(recipe, PUBLIC_VALIDATION, ENTRY, 0, 1, constant),
                input_call(recipe, ENTRY, RESET, 0, 1, constant),
                input_call(recipe, ENTRY, LEAF, 0, if locals { 2 } else { 1 }, member),
            ];
            if locals {
                calls.push(input_call(recipe, ENTRY, LEAF, 1, 2, member));
            }
            let mut result = expected(fixture, two, cap, population, totals.0, totals.1);
            result.input_partition = Some(SourceBounds {
                roots: vec![function(recipe, PUBLIC_VALIDATION)],
                cap: NodeCapData { nodes: cap },
                attempts: population.validations,
                helper_calls: calls,
                iterator_callbacks_per_attempt: member,
                required_sources: BTreeSet::from([
                    SourceRequirement::CanonicalRecipe,
                    SourceRequirement::BorrowedInputTree,
                    SourceRequirement::FormalActualBindings,
                    SourceRequirement::IteratorMemberProtocol,
                    SourceRequirement::CallerNodeCap,
                ]),
            });
            result
        }

        fn local_fixture() -> Fixture {
            let mut fixture = factory_fixture(true);
            let items = fixture
                .syntax
                .items
                .iter_mut()
                .find_map(|item| {
                    if let syn::Item::Mod(module) = item
                        && module.ident == "__coupled_impl"
                    {
                        Some(&mut module.content.as_mut().unwrap().1)
                    } else {
                        None
                    }
                })
                .unwrap();
            let entry = items
                .iter_mut()
                .find_map(|item| {
                    if let syn::Item::Fn(function) = item
                        && function.sig.ident == "entry_is_valid"
                    {
                        Some(function)
                    } else {
                        None
                    }
                })
                .unwrap();
            *entry.block = syn::parse_quote!({
                __cache.reset();
                match instance {
                    serde_json::Value::Object(obj) => {
                        let mut __req_0 = false;
                        let __object_pass = obj.iter().all(|(key, instance)| {
                            let mut covered = false;
                            match key.as_str() {
                                "a" => {
                                    if !leaf(instance, __cache) {
                                        return false;
                                    }
                                    __req_0 = true;
                                    covered = true;
                                }
                                _ => {}
                            }
                            if !covered {
                                if !leaf(instance, __cache) {
                                    return false;
                                }
                            }
                            true
                        });
                        __object_pass && __req_0
                    }
                    _ => instance.is_boolean(),
                }
            });
            let classified = operations::classify_source_body(&fixture.syntax, &[], &[]);
            assert!(
                classified.is_ok(),
                "local-flow fixture classification: {classified:?}"
            );
            fixture.recipe = classified.unwrap().recipe().clone();
            assert_eq!(
                fixture.recipe.initialization.get(INITIALIZER),
                Some(&programs(true))
            );
            let population = CoupledPopulation {
                factories: 3,
                validations: 2,
            };
            assert_eq!(
                coupled_factory_bounds(
                    &fixture.recipe,
                    &fixture.graph,
                    OWNER,
                    MARKER,
                    7,
                    population
                ),
                Ok(expected(&fixture, true, 7, population, 5, 6))
            );
            fixture
        }

        fn protocol_fixture(rescan: bool) -> Fixture {
            let mut fixture = factory_fixture(true);
            let items = fixture
                .syntax
                .items
                .iter_mut()
                .find_map(|item| {
                    if let syn::Item::Mod(module) = item
                        && module.ident == "__coupled_impl"
                    {
                        Some(&mut module.content.as_mut().unwrap().1)
                    } else {
                        None
                    }
                })
                .unwrap();
            let entry = items
                .iter_mut()
                .find_map(|item| {
                    if let syn::Item::Fn(function) = item
                        && function.sig.ident == "entry_is_valid"
                    {
                        Some(function)
                    } else {
                        None
                    }
                })
                .unwrap();
            let values = quote::quote! {
                obj.values().all(|instance| match instance {
                    serde_json::Value::String(s) => string_leaf(s, __cache),
                    serde_json::Value::Number(n) => jsonschema::__private::types::is_integer_draft4(n),
                    serde_json::Value::Bool(b) => *b,
                    _ => true,
                })
            };
            let object = if rescan {
                quote::quote! { 'pass: {
                    if (obj.len() as u64) < 1 { return false; }
                    for (key, instance) in obj.iter() {
                        let key_str = key.as_str();
                        if !key_leaf(key_str, __cache) { break 'pass false; }
                        if !(#values) { break 'pass false; }
                    }
                    true
                } }
            } else {
                quote::quote! {{
                    if (obj.len() as u64) < 1 { return false; }
                    #values
                }}
            };
            *entry.block = syn::parse2(quote::quote!({
                __cache.reset();
                match instance {
                    serde_json::Value::Object(obj) => #object,
                    _ => true,
                }
            }))
            .unwrap();
            items.push(syn::parse_quote! {
                fn string_leaf(subject: &str, __cache: &mut __SearchCache) -> bool {
                    PATTERN_0_REGEX.as_ref().is_some_and(|regex| {
                        sentinel(subject, __cache) && regex.search_half_with(
                            &mut __cache.pattern_0_cache,
                            &__ra::Input::new(subject).earliest(true),
                        ).is_some()
                    })
                }
            });
            items.push(syn::parse_quote! {
                fn sentinel(subject: &str, __cache: &mut __SearchCache) -> bool {
                    matches!(subject, "x" | "y")
                }
            });
            items.push(syn::parse_quote! {
                fn key_leaf(subject: &str, __cache: &mut __SearchCache) -> bool {
                    matches!(subject, "s" | "n")
                }
            });
            fixture.recipe = operations::classify_source_body(&fixture.syntax, &[], &[])
                .unwrap()
                .recipe()
                .clone();
            coupled_factory_bounds(
                &fixture.recipe,
                &fixture.graph,
                OWNER,
                MARKER,
                7,
                CoupledPopulation {
                    factories: 3,
                    validations: 2,
                },
            )
            .unwrap();
            fixture
        }

        fn helper_coefficients(
            bounds: &CoupledFactoryBounds,
            recipe: &Recipe,
            name: &str,
        ) -> Vec<(u64, u64)> {
            // The canonical recipe contains only reachable functions. The
            // positive rows below still require every expected helper once.
            let targets: Vec<_> = recipe
                .functions
                .iter()
                .filter(|(_, function)| function.identity == name)
                .map(|(id, _)| *id)
                .collect();
            assert!(targets.len() <= 1);
            let Some(target) = targets.first() else {
                return vec![];
            };
            bounds
                .input_partition
                .as_ref()
                .unwrap()
                .helper_calls
                .iter()
                .filter(|row| row.target == *target)
                .map(|row| (row.per_attempt.constant, row.per_attempt.nodes))
                .collect()
        }

        fn assert_key_only_protocol() {
            let mut fixture = protocol_fixture(false);
            let entry = fixture
                .syntax
                .items
                .iter_mut()
                .find_map(|item| {
                    let syn::Item::Mod(module) = item else {
                        return None;
                    };
                    module.content.as_mut()?.1.iter_mut().find_map(|member| {
                        if let syn::Item::Fn(function) = member
                            && function.sig.ident == "entry_is_valid"
                        {
                            Some(function)
                        } else {
                            None
                        }
                    })
                })
                .unwrap();
            *entry.block = syn::parse_quote!({
                __cache.reset();
                match instance {
                    serde_json::Value::Object(obj) => {
                        obj.keys().all(|key| key_leaf(key.as_str(), __cache))
                    }
                    _ => true,
                }
            });
            fixture.recipe = operations::classify_source_body(&fixture.syntax, &[], &[])
                .unwrap()
                .recipe()
                .clone();
            for cap in [7, 11] {
                let bounds = input(
                    &fixture,
                    cap,
                    CoupledPopulation {
                        factories: 3,
                        validations: 2,
                    },
                )
                .unwrap();
                assert_eq!(
                    helper_coefficients(&bounds, &fixture.recipe, KEY_LEAF),
                    vec![(0, 1)]
                );
                assert!(helper_coefficients(&bounds, &fixture.recipe, STRING_LEAF).is_empty());
                let projection = bounds.input_partition.unwrap();
                assert_eq!(projection.helper_calls.len(), 3);
                assert_eq!(
                    projection.iterator_callbacks_per_attempt,
                    SourceBoundCoeff {
                        constant: 0,
                        nodes: 1
                    }
                );
            }
        }

        #[test]
        fn emitted_protocols_keep_scalar_and_optional_callback_coefficients() {
            let witness = json!({"s":"x", "n":1_u64});
            for (rescan, cap, string_nodes, callback_nodes) in [
                (false, 7, 1, 1),
                (false, 11, 1, 1),
                (true, 7, 3, 4),
                (true, 11, 5, 6),
            ] {
                let fixture = protocol_fixture(rescan);
                let limits = SchemaInputLimits {
                    serialized_bytes: 128,
                    nodes: cap,
                    depth: 4,
                };
                assert_eq!(measure_schema_input(&witness, limits).unwrap().nodes, 5);
                let bounds = input(
                    &fixture,
                    cap,
                    CoupledPopulation {
                        factories: 3,
                        validations: 2,
                    },
                )
                .unwrap();
                assert_eq!(bounds.root_entries_upper, 5);
                assert_eq!(bounds.factory.initializer_checks_upper, 6);
                assert_eq!(bounds.required, super::duties());
                assert_eq!(
                    helper_coefficients(&bounds, &fixture.recipe, STRING_LEAF),
                    vec![(0, string_nodes)]
                );
                assert_eq!(
                    helper_coefficients(&bounds, &fixture.recipe, SENTINEL),
                    vec![(0, string_nodes)]
                );
                assert_eq!(
                    helper_coefficients(&bounds, &fixture.recipe, ENTRY),
                    vec![(1, 0)]
                );
                assert_eq!(
                    helper_coefficients(&bounds, &fixture.recipe, RESET),
                    vec![(1, 0)]
                );
                assert_eq!(
                    helper_coefficients(&bounds, &fixture.recipe, KEY_LEAF),
                    if rescan { vec![(0, 1)] } else { vec![] }
                );
                let projection = bounds.input_partition.unwrap();
                assert_eq!(projection.helper_calls.len(), if rescan { 5 } else { 4 });
                assert_eq!(
                    projection.iterator_callbacks_per_attempt,
                    SourceBoundCoeff {
                        constant: 0,
                        nodes: callback_nodes
                    }
                );
            }
            assert_key_only_protocol();
        }

        mod search_tests {
            use super::super::super::{
                CoupledSearchBounds, ProgramSearchBound, coupled_search_bounds,
            };
            use super::{
                CoupledPopulation, CoupledRefusal, Fixture, MARKER, OWNER, Recipe, STRING_LEAF,
                factory_fixture, function, input, operations, protocol_fixture, protocol_search,
            };
            use crate::schema_validation::operations::{
                Primitive,
                model::{Kind, Site},
            };
            use serde_json::json;

            const FIRST: &str = "__coupled_impl::PATTERN_0_REGEX";
            const SECOND: &str = "__coupled_impl::PATTERN_1_REGEX";
            const OTHER: &str = "__coupled_impl::other_leaf";

            fn searches(
                fixture: &Fixture,
                cap: u64,
                population: CoupledPopulation,
            ) -> Result<CoupledSearchBounds, CoupledRefusal> {
                coupled_search_bounds(
                    &fixture.recipe,
                    &fixture.graph,
                    OWNER,
                    MARKER,
                    cap,
                    population,
                )
            }

            fn source_sites(recipe: &Recipe, owner: &str, expected: usize) -> Vec<Site> {
                let mut sites = Vec::new();
                operations::model::walk(
                    &recipe.functions[&function(recipe, owner)].body,
                    &mut |op| {
                        if let Kind::Primitive {
                            lemma: Primitive::RegexSearch,
                            callee,
                            ..
                        } = &op.kind
                            && callee == "Regex::search_half_with"
                        {
                            sites.push(op.site.clone());
                        }
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(sites.len(), expected, "independent lexical search census");
                sites
            }

            fn assert_program(
                row: &ProgramSearchBound,
                program: &str,
                field: &str,
                sites: &[Site],
                coefficient: (u64, u64),
                maxima: (u64, u64),
            ) {
                assert_eq!(row.program, program);
                assert_eq!(row.field, field);
                assert_eq!(
                    row.sites.iter().map(|site| &site.site).collect::<Vec<_>>(),
                    sites.iter().collect::<Vec<_>>()
                );
                for site in &row.sites {
                    assert_eq!(
                        (site.per_validation.constant, site.per_validation.nodes),
                        coefficient
                    );
                    assert_eq!((site.per_validation_upper, site.invocations_upper), maxima);
                }
            }

            struct SingleSearchExpectation {
                nodes: u64,
                per_call: u64,
                all_calls: u64,
                witness_calls: u64,
                roots: u64,
                probes: u64,
            }

            fn assert_single_result(
                bounds: &CoupledSearchBounds,
                fixture: &Fixture,
                population: CoupledPopulation,
                wanted: &SingleSearchExpectation,
            ) {
                assert_eq!(bounds.input.root_entries_upper, wanted.roots);
                assert_eq!(bounds.input.factory.initializer_checks_upper, wanted.probes);
                assert_eq!(bounds.input.required, super::super::duties());
                assert_eq!(bounds.input.roots, fixture.recipe.roots);
                assert_eq!(bounds.input.initialization, fixture.recipe.initialization);
                assert_eq!(bounds.programs.len(), 2);
                assert_program(
                    &bounds.programs[0],
                    FIRST,
                    "pattern_0_cache",
                    &source_sites(&fixture.recipe, STRING_LEAF, 1),
                    (0, wanted.nodes),
                    (wanted.per_call, wanted.all_calls),
                );
                assert_program(
                    &bounds.programs[1],
                    SECOND,
                    "pattern_1_cache",
                    &[],
                    (0, 0),
                    (0, 0),
                );
                // The admitted witness is distinct from the declared cap.
                let coefficient = bounds.programs[0].sites[0].per_validation;
                assert_eq!(
                    coefficient
                        .nodes
                        .checked_mul(5)
                        .unwrap()
                        .checked_add(coefficient.constant)
                        .unwrap()
                        .checked_mul(population.validations)
                        .unwrap(),
                    wanted.witness_calls
                );
                assert_eq!(
                    (
                        bounds.programs[0].per_validation_upper,
                        bounds.programs[0].invocations_upper
                    ),
                    (wanted.per_call, wanted.all_calls)
                );
                assert_eq!(
                    (
                        bounds.programs[1].per_validation_upper,
                        bounds.programs[1].invocations_upper
                    ),
                    (0, 0)
                );
                assert_eq!(
                    (bounds.per_validation_upper, bounds.invocations_upper),
                    (wanted.per_call, wanted.all_calls)
                );
            }

            #[test]
            fn search_sites_bind_programs_and_independent_populations() {
                let witness = json!({"s": "x", "n": 1_u64});
                for (
                    rescan,
                    cap,
                    factories,
                    validations,
                    nodes,
                    per_call,
                    all_calls,
                    witness_calls,
                    roots,
                    probes,
                ) in [
                    (false, 7, 3, 2, 1, 7, 14, 10, 5, 6),
                    (false, 11, 3, 2, 1, 11, 22, 10, 5, 6),
                    (true, 7, 3, 2, 3, 21, 42, 30, 5, 6),
                    (true, 11, 3, 2, 5, 55, 110, 50, 5, 6),
                    (false, 7, 9, 2, 1, 7, 14, 10, 11, 18),
                    (true, 11, 3, 0, 5, 55, 0, 0, 3, 6),
                ] {
                    let fixture = protocol_fixture(rescan);
                    let limits = super::SchemaInputLimits {
                        serialized_bytes: 128,
                        nodes: cap,
                        depth: 4,
                    };
                    assert_eq!(
                        super::measure_schema_input(&witness, limits).unwrap().nodes,
                        5
                    );
                    let population = CoupledPopulation {
                        factories,
                        validations,
                    };
                    input(&fixture, cap, population).unwrap();
                    let result = searches(&fixture, cap, population);
                    assert!(result.is_ok(), "search invocation derivation: {result:?}");
                    let bounds = result.unwrap();
                    assert_single_result(
                        &bounds,
                        &fixture,
                        population,
                        &SingleSearchExpectation {
                            nodes,
                            per_call,
                            all_calls,
                            witness_calls,
                            roots,
                            probes,
                        },
                    );
                }
                let no_regex = factory_fixture(false);
                let bounds = searches(
                    &no_regex,
                    7,
                    CoupledPopulation {
                        factories: 3,
                        validations: 2,
                    },
                )
                .unwrap();
                assert!(bounds.programs.is_empty());
                assert_eq!(
                    (bounds.per_validation_upper, bounds.invocations_upper),
                    (0, 0)
                );
                assert_eq!(bounds.input.required, super::super::duties());
                assert_constant_searches();
            }

            fn items(fixture: &mut Fixture) -> &mut Vec<syn::Item> {
                fixture
                    .syntax
                    .items
                    .iter_mut()
                    .find_map(|item| {
                        if let syn::Item::Mod(module) = item
                            && module.ident == "__coupled_impl"
                        {
                            Some(&mut module.content.as_mut().unwrap().1)
                        } else {
                            None
                        }
                    })
                    .unwrap()
            }

            fn function_mut<'a>(fixture: &'a mut Fixture, name: &str) -> &'a mut syn::ItemFn {
                items(fixture)
                    .iter_mut()
                    .find_map(|item| {
                        if let syn::Item::Fn(declaration) = item
                            && declaration.sig.ident == name
                        {
                            Some(declaration)
                        } else {
                            None
                        }
                    })
                    .unwrap()
            }

            fn rebuild(fixture: &mut Fixture) {
                fixture.recipe = operations::classify_source_body(&fixture.syntax, &[], &[])
                    .unwrap()
                    .recipe()
                    .clone();
                input(
                    fixture,
                    7,
                    CoupledPopulation {
                        factories: 3,
                        validations: 2,
                    },
                )
                .unwrap();
            }

            fn assert_constant_searches() {
                let mut fixture = protocol_fixture(false);
                *function_mut(&mut fixture, "entry_is_valid").block = syn::parse_quote!({
                    __cache.reset();
                    match instance {
                        serde_json::Value::String(subject) => string_leaf(subject, __cache),
                        _ => true,
                    }
                });
                rebuild(&mut fixture);
                for (cap, validations, total) in [(7, 2, 2), (11, 0, 0), (11, 5, 5)] {
                    let bounds = searches(
                        &fixture,
                        cap,
                        CoupledPopulation {
                            factories: 3,
                            validations,
                        },
                    )
                    .unwrap();
                    assert_eq!(bounds.programs.len(), 2);
                    assert_program(
                        &bounds.programs[0],
                        FIRST,
                        "pattern_0_cache",
                        &source_sites(&fixture.recipe, STRING_LEAF, 1),
                        (1, 0),
                        (1, total),
                    );
                    assert_program(
                        &bounds.programs[1],
                        SECOND,
                        "pattern_1_cache",
                        &[],
                        (0, 0),
                        (0, 0),
                    );
                    assert_eq!(
                        (
                            bounds.programs[0].per_validation_upper,
                            bounds.programs[0].invocations_upper
                        ),
                        (1, total)
                    );
                    assert_eq!(
                        (bounds.per_validation_upper, bounds.invocations_upper),
                        (1, total)
                    );
                    assert_eq!(bounds.input.required, super::super::duties());
                }
            }

            fn eight_searches(program: &syn::Ident, field: &syn::Ident) -> syn::Block {
                let mut expression: syn::Expr = syn::parse_quote!(sentinel(subject, __cache));
                for _ in 0..8_u8 {
                    expression = syn::parse_quote!(#expression && regex.search_half_with(
                        &mut __cache.#field,
                        &__ra::Input::new(subject).earliest(true),
                    ).is_some());
                }
                syn::parse_quote!({ #program.as_ref().is_some_and(|regex| { #expression }) })
            }

            fn eight_site_fixture() -> Fixture {
                let mut fixture = protocol_fixture(false);
                *function_mut(&mut fixture, "string_leaf").block = eight_searches(
                    &syn::parse_quote!(PATTERN_0_REGEX),
                    &syn::parse_quote!(pattern_0_cache),
                );
                rebuild(&mut fixture);
                fixture
            }

            fn repeated_helper_fixture() -> Fixture {
                let mut fixture = protocol_fixture(false);
                *function_mut(&mut fixture, "entry_is_valid").block = syn::parse_quote!({
                    __cache.reset();
                    match instance {
                        serde_json::Value::Object(obj) => {
                            obj.values().all(|instance| match instance {
                                serde_json::Value::String(subject) => {
                                    string_leaf(subject, __cache)
                                        && string_leaf(subject, __cache)
                                        && other_leaf(subject, __cache)
                                }
                                _ => true,
                            })
                        }
                        _ => true,
                    }
                });
                items(&mut fixture).push(syn::parse_quote! {
                    fn other_leaf(subject: &str, __cache: &mut __SearchCache) -> bool {
                        PATTERN_1_REGEX.as_ref().is_some_and(|regex| regex.search_half_with(
                            &mut __cache.pattern_1_cache,
                            &__ra::Input::new(subject).earliest(true),
                        ).is_some())
                    }
                });
                rebuild(&mut fixture);
                fixture
            }

            #[test]
            fn repeated_helpers_and_distinct_sites_preserve_search_multiplicity() {
                let population = CoupledPopulation {
                    factories: 3,
                    validations: 2,
                };
                let fixture = repeated_helper_fixture();
                let result = searches(&fixture, 7, population);
                assert!(
                    result.is_ok(),
                    "repeated-helper search derivation: {result:?}"
                );
                let bounds = result.unwrap();
                assert_eq!(bounds.programs.len(), 2);
                assert_program(
                    &bounds.programs[0],
                    FIRST,
                    "pattern_0_cache",
                    &source_sites(&fixture.recipe, STRING_LEAF, 1),
                    (0, 2),
                    (14, 28),
                );
                assert_program(
                    &bounds.programs[1],
                    SECOND,
                    "pattern_1_cache",
                    &source_sites(&fixture.recipe, OTHER, 1),
                    (0, 1),
                    (7, 14),
                );
                assert_eq!(
                    (
                        bounds.programs[0].per_validation_upper,
                        bounds.programs[0].invocations_upper
                    ),
                    (14, 28)
                );
                assert_eq!(
                    (
                        bounds.programs[1].per_validation_upper,
                        bounds.programs[1].invocations_upper
                    ),
                    (7, 14)
                );
                assert_eq!(
                    (bounds.per_validation_upper, bounds.invocations_upper),
                    (21, 42)
                );
                let eight = eight_site_fixture();
                let eight_bounds = searches(&eight, 7, population).unwrap();
                assert_eq!(eight_bounds.programs.len(), 2);
                assert_program(
                    &eight_bounds.programs[0],
                    FIRST,
                    "pattern_0_cache",
                    &source_sites(&eight.recipe, STRING_LEAF, 8),
                    (0, 1),
                    (7, 14),
                );
                assert_eq!(
                    (
                        eight_bounds.programs[0].per_validation_upper,
                        eight_bounds.programs[0].invocations_upper
                    ),
                    (56, 112)
                );
                assert_program(
                    &eight_bounds.programs[1],
                    SECOND,
                    "pattern_1_cache",
                    &[],
                    (0, 0),
                    (0, 0),
                );
                assert_eq!(
                    (
                        eight_bounds.per_validation_upper,
                        eight_bounds.invocations_upper
                    ),
                    (56, 112)
                );
            }

            fn assert_cross_program_overflow() {
                let mut fixture = repeated_helper_fixture();
                *function_mut(&mut fixture, "string_leaf").block = eight_searches(
                    &syn::parse_quote!(PATTERN_0_REGEX),
                    &syn::parse_quote!(pattern_0_cache),
                );
                *function_mut(&mut fixture, "other_leaf").block = eight_searches(
                    &syn::parse_quote!(PATTERN_1_REGEX),
                    &syn::parse_quote!(pattern_1_cache),
                );
                rebuild(&mut fixture);
                let bounds = searches(
                    &fixture,
                    7,
                    CoupledPopulation {
                        factories: 3,
                        validations: 2,
                    },
                )
                .unwrap();
                assert_eq!(bounds.programs.len(), 2);
                assert_program(
                    &bounds.programs[0],
                    FIRST,
                    "pattern_0_cache",
                    &source_sites(&fixture.recipe, STRING_LEAF, 8),
                    (0, 2),
                    (14, 28),
                );
                assert_program(
                    &bounds.programs[1],
                    SECOND,
                    "pattern_1_cache",
                    &source_sites(&fixture.recipe, OTHER, 8),
                    (0, 1),
                    (7, 14),
                );
                assert_eq!(
                    (
                        bounds.programs[0].per_validation_upper,
                        bounds.programs[0].invocations_upper
                    ),
                    (112, 224)
                );
                assert_eq!(
                    (
                        bounds.programs[1].per_validation_upper,
                        bounds.programs[1].invocations_upper
                    ),
                    (56, 112)
                );
                assert_eq!(
                    (bounds.per_validation_upper, bounds.invocations_upper),
                    (168, 336)
                );
                // Each program fits independently; only the whole sum fails.
                for (cap, validations) in [(0x0C00_0000_0000_0000, 0), (7, 0x0200_0000_0000_0000)] {
                    let population = CoupledPopulation {
                        factories: 0,
                        validations,
                    };
                    input(&fixture, cap, population).unwrap();
                    assert_eq!(
                        searches(&fixture, cap, population),
                        Err(CoupledRefusal::Overflow)
                    );
                }
            }

            #[test]
            fn search_refusals_and_overflow_survive_zero_workload() {
                let fixture = protocol_fixture(false);
                for validations in [0, 1] {
                    let population = CoupledPopulation {
                        factories: 3,
                        validations,
                    };
                    let result = searches(&fixture, 7, population);
                    assert!(result.is_ok(), "positive source setup: {result:?}");
                    for change_lemma in [false, true] {
                        let mut recipe = fixture.recipe.clone();
                        let search = protocol_search(&mut recipe);
                        let Kind::Primitive { lemma, callee, .. } = &mut search.kind else {
                            panic!("search primitive");
                        };
                        assert_eq!(*lemma, Primitive::RegexSearch);
                        assert_eq!(callee.as_str(), "Regex::search_half_with");
                        if change_lemma {
                            *lemma = Primitive::ValueAccess;
                        } else {
                            *callee = "Regex::search_with".to_owned();
                        }
                        assert_eq!(
                            coupled_search_bounds(
                                &recipe,
                                &fixture.graph,
                                OWNER,
                                MARKER,
                                7,
                                population
                            ),
                            // The unchanged ownership foundation rejects the
                            // malformed engine entry before numeric projection.
                            Err(CoupledRefusal::OwnerBinding)
                        );
                    }
                }
                let aggregate = eight_site_fixture();
                let zero = CoupledPopulation {
                    factories: 0,
                    validations: 0,
                };
                input(&aggregate, 0x4000_0000_0000_0000, zero).unwrap();
                assert_eq!(
                    searches(&aggregate, 0x4000_0000_0000_0000, zero),
                    Err(CoupledRefusal::Overflow)
                );
                let many = CoupledPopulation {
                    factories: 0,
                    validations: 0x0800_0000_0000_0000,
                };
                input(&aggregate, 7, many).unwrap();
                assert_eq!(searches(&aggregate, 7, many), Err(CoupledRefusal::Overflow));
                assert_cross_program_overflow();
            }
        }

        fn protocol_callback(recipe: &mut Recipe) -> &mut Op {
            let id = function(recipe, STRING_LEAF);
            let Kind::Scope { steps, .. } = &mut recipe.functions.get_mut(&id).unwrap().body.kind
            else {
                panic!("string helper scope");
            };
            assert_eq!(steps.len(), 1);
            assert!(matches!(&steps[0].kind, Kind::Callback { .. }));
            &mut steps[0]
        }

        fn protocol_search(recipe: &mut Recipe) -> &mut Op {
            let Kind::Callback { body, .. } = &mut protocol_callback(recipe).kind else {
                panic!("optional regex callback");
            };
            let Kind::Scope { steps, .. } = &mut body.kind else {
                panic!("regex callback scope");
            };
            let Kind::ShortCircuit(_, values) = &mut steps[0].kind else {
                panic!("sentinel/search conjunction");
            };
            let Kind::Primitive { arguments, .. } = &mut values[1].kind else {
                panic!("search is_some");
            };
            assert_eq!(arguments.len(), 1);
            &mut arguments[0]
        }

        fn assert_search_binding_mutants(fixture: &Fixture, population: CoupledPopulation) {
            for change_input in [false, true] {
                let mut recipe = fixture.recipe.clone();
                let search = protocol_search(&mut recipe);
                let anchor = search.site.clone();
                let Kind::Primitive { arguments, .. } = &mut search.kind else {
                    panic!("regex search primitive");
                };
                assert_eq!(arguments.len(), 3);
                let index = if change_input { 2 } else { 1 };
                let Kind::Evaluate(Action::Borrow { .. }, values) = &mut arguments[index].kind
                else {
                    panic!("borrowed search operand");
                };
                if change_input {
                    let Kind::Primitive {
                        arguments: input_arguments,
                        ..
                    } = &mut values[0].kind
                    else {
                        panic!("earliest input");
                    };
                    assert!(matches!(
                        &input_arguments[1].kind,
                        Kind::Value(super::super::Atom::Bool(true))
                    ));
                    input_arguments[1].kind = Kind::Value(super::super::Atom::Bool(false));
                } else {
                    let Kind::Evaluate(Action::Field(field), _) = &mut values[0].kind else {
                        panic!("search cache field");
                    };
                    assert_eq!(field.as_str(), "pattern_0_cache");
                    *field = "pattern_1_cache".to_owned();
                }
                assert_eq!(
                    coupled_input_bounds(&recipe, &fixture.graph, OWNER, MARKER, 11, population),
                    Err(CoupledRefusal::CallBinding(anchor))
                );
            }
        }

        #[test]
        fn emitted_protocol_bindings_refuse_at_zero_validations() {
            let fixture = protocol_fixture(true);
            for validations in [0, 1] {
                let population = CoupledPopulation {
                    factories: 3,
                    validations,
                };
                input(&fixture, 11, population).unwrap();
                assert_scalar_cast_binding(&fixture, population);
                assert_search_binding_mutants(&fixture, population);
                let mut formal = fixture.recipe.clone();
                let target = function(&formal, STRING_LEAF);
                let anchor = super::site(
                    &formal,
                    ENTRY,
                    |op| matches!(&op.kind, Kind::Call { target: id, .. } if *id == target),
                );
                let parameter = &mut formal.functions.get_mut(&target).unwrap().parameters[0];
                assert!(matches!(
                    &parameter.origin,
                    Origin::Parameter { ordinal: 0, .. }
                ));
                parameter.origin = Origin::Parameter {
                    function: target,
                    ordinal: 7,
                };
                assert_eq!(
                    coupled_input_bounds(&formal, &fixture.graph, OWNER, MARKER, 11, population),
                    Err(CoupledRefusal::CallBinding(anchor))
                );
                let mut callback = fixture.recipe.clone();
                let op = protocol_callback(&mut callback);
                let callback_site = op.site.clone();
                let Kind::Callback { parameters, .. } = &mut op.kind else {
                    panic!("optional regex callback parameters");
                };
                assert!(parameters[0].declaration.is_some());
                parameters[0].declaration = None;
                assert_eq!(
                    coupled_input_bounds(&callback, &fixture.graph, OWNER, MARKER, 11, population),
                    Err(CoupledRefusal::CallBinding(callback_site))
                );
                let mut labelled = fixture.recipe.clone();
                let mut anchors = Vec::new();
                operations::model::walk(
                    &labelled.functions[&function(&labelled, ENTRY)].body,
                    &mut |candidate| {
                        if matches!(
                            &candidate.kind,
                            Kind::Exit(operations::model::Exit::Break(_), _)
                        ) {
                            anchors.push(candidate.site.clone());
                        }
                        Ok(())
                    },
                )
                .unwrap();
                assert_eq!(anchors.len(), 2);
                let Kind::Scope { label, .. } = &mut object_scope(&mut labelled).kind else {
                    panic!("labelled pass scope");
                };
                assert_eq!(label.as_deref(), Some("pass"));
                *label = Some("wrong".to_owned());
                assert_eq!(
                    coupled_input_bounds(&labelled, &fixture.graph, OWNER, MARKER, 11, population),
                    Err(CoupledRefusal::SourceRefusal {
                        function: Some(ENTRY.to_owned()),
                        site: anchors[0].clone(),
                        cause: operations::Refusal::UnsupportedSource,
                    })
                );
            }
        }

        fn assert_scalar_cast_binding(fixture: &Fixture, population: CoupledPopulation) {
            let mut recipe = fixture.recipe.clone();
            let Kind::Scope { steps, .. } = &mut object_scope(&mut recipe).kind else {
                panic!("count-check scope");
            };
            let Kind::Choice { selector, .. } = &mut steps[0].kind else {
                panic!("count comparison guard");
            };
            let Kind::ScalarTest(_, operands) = &mut selector.kind else {
                panic!("count comparison");
            };
            let cast = &mut operands[0];
            let anchor = cast.site.clone();
            let Kind::Evaluate(Action::CastScalar, cast_operands) = &mut cast.kind else {
                panic!("count cast");
            };
            assert_eq!(cast_operands.len(), 1);
            assert_eq!(
                cast_operands[0].site,
                super::super::coupled_child(&anchor, 0)
            );
            cast_operands[0].site.lexical.push(99);
            assert_eq!(
                coupled_input_bounds(&recipe, &fixture.graph, OWNER, MARKER, 11, population),
                Err(CoupledRefusal::SourceRefusal {
                    function: Some(ENTRY.to_owned()),
                    site: anchor,
                    cause: operations::Refusal::UnsupportedSource,
                })
            );
        }

        fn object_scope(recipe: &mut Recipe) -> &mut Op {
            let entry = function(recipe, ENTRY);
            let Kind::Scope { steps, .. } =
                &mut recipe.functions.get_mut(&entry).unwrap().body.kind
            else {
                panic!("entry scope");
            };
            let Kind::Choice { arms, .. } = &mut steps[1].kind else {
                panic!("input Object match");
            };
            &mut arms[0].body
        }

        fn property_branch(recipe: &mut Recipe) -> &mut Op {
            let Kind::Scope {
                steps: object_steps,
                ..
            } = &mut object_scope(recipe).kind
            else {
                panic!("object scope");
            };
            let Kind::Bind { value, .. } = &mut object_steps[1].kind else {
                panic!("object pass binding");
            };
            let Kind::Iterate { body, .. } = &mut value.kind else {
                panic!("object entry all");
            };
            let Kind::Scope {
                steps: callback_steps,
                ..
            } = &mut body.kind
            else {
                panic!("member callback scope");
            };
            let Kind::Choice { arms, .. } = &mut callback_steps[1].kind else {
                panic!("literal property match");
            };
            &mut arms[0].body
        }

        fn first_leaf_call(recipe: &mut Recipe) -> &mut Op {
            let Kind::Scope { steps, .. } = &mut property_branch(recipe).kind else {
                panic!("property branch scope");
            };
            let Kind::Choice { selector, .. } = &mut steps[0].kind else {
                panic!("leaf result guard");
            };
            let Kind::Evaluate(Action::Not, values) = &mut selector.kind else {
                panic!("negated leaf result");
            };
            &mut values[0]
        }

        #[derive(Clone, Copy)]
        enum LocalDrift {
            Declaration,
            Assignment,
            Actual,
            Formal,
        }

        fn local_mutant(fixture: &Fixture, drift: LocalDrift) -> (Recipe, CoupledRefusal) {
            let mut recipe = fixture.recipe.clone();
            let refusal = match drift {
                LocalDrift::Declaration => {
                    let scope = object_scope(&mut recipe);
                    let anchor = scope.site.clone();
                    let Kind::Scope { steps, locals, .. } = &mut scope.kind else {
                        panic!("object locals");
                    };
                    let Kind::Bind { bindings, .. } = &steps[0].kind else {
                        panic!("required flag declaration");
                    };
                    assert_eq!(locals[0], bindings[0]);
                    assert_eq!(locals[0].declaration.as_ref(), Some(&steps[0].site));
                    locals[0].declaration = None;
                    CoupledRefusal::LocalBinding(anchor)
                }
                LocalDrift::Assignment => {
                    let Kind::Scope { steps, .. } = &mut property_branch(&mut recipe).kind else {
                        panic!("property assignment scope");
                    };
                    let assignment = &mut steps[1];
                    let Kind::Evaluate(Action::Assign(target), values) = &mut assignment.kind
                    else {
                        panic!("required flag assignment");
                    };
                    assert_eq!(target.place, "__req_0");
                    assert_eq!(*target, values[0].value);
                    target.place = "covered".to_owned();
                    CoupledRefusal::LocalBinding(assignment.site.clone())
                }
                LocalDrift::Actual => {
                    let call = first_leaf_call(&mut recipe);
                    let Kind::Call {
                        arguments,
                        evaluation,
                        ..
                    } = &mut call.kind
                    else {
                        panic!("leaf call");
                    };
                    assert_eq!(arguments[0], evaluation[0].value);
                    assert!(arguments[0].declaration.is_some());
                    arguments[0].declaration = None;
                    CoupledRefusal::CallBinding(call.site.clone())
                }
                LocalDrift::Formal => {
                    let site = first_leaf_call(&mut recipe).site.clone();
                    let leaf = function(&recipe, LEAF);
                    let formal = &mut recipe.functions.get_mut(&leaf).unwrap().parameters[1];
                    assert_eq!(
                        formal.origin,
                        Origin::Parameter {
                            function: leaf,
                            ordinal: 1
                        }
                    );
                    formal.origin = Origin::Parameter {
                        function: leaf,
                        ordinal: 0,
                    };
                    CoupledRefusal::CallBinding(site)
                }
            };
            (recipe, refusal)
        }

        #[test]
        fn coupled_input_keeps_factory_and_validation_populations_separate() {
            let population = CoupledPopulation {
                factories: 3,
                validations: 2,
            };
            for (two, readiness) in [(false, 0), (true, 6)] {
                let fixture = factory_fixture(two);
                assert_eq!(
                    input(&fixture, 7, population),
                    Ok(expected_input(
                        &fixture,
                        two,
                        false,
                        7,
                        population,
                        (5, readiness)
                    ))
                );
                let seven = input(&fixture, 7, population).unwrap();
                let eleven = input(&fixture, 11, population).unwrap();
                assert_eq!(seven.factory, eleven.factory);
                assert_eq!(seven.roots, fixture.recipe.roots);
                assert_eq!(seven.initialization, fixture.recipe.initialization);
                let projection = seven.input_partition.as_ref().unwrap();
                assert_eq!(
                    projection.roots,
                    [function(&fixture.recipe, PUBLIC_VALIDATION)]
                );
                assert_eq!(projection.attempts, 2);
                assert_eq!(
                    projection
                        .helper_calls
                        .iter()
                        .map(|row| row.per_attempt)
                        .collect::<Vec<_>>(),
                    [
                        SourceBoundCoeff {
                            constant: 1,
                            nodes: 0
                        },
                        SourceBoundCoeff {
                            constant: 1,
                            nodes: 0
                        },
                        SourceBoundCoeff {
                            constant: 0,
                            nodes: 1
                        },
                    ]
                );
                let measured = measure_schema_input(
                    &json!([true, true, true]),
                    SchemaInputLimits {
                        serialized_bytes: 0x0001_0000,
                        nodes: 7,
                        depth: 32,
                    },
                )
                .unwrap();
                assert_eq!(measured.nodes, 4);
                // Three callback/leaf executions witness, but do not define, the J upper.
                assert_eq!(
                    (
                        projection.cap.nodes,
                        eleven.input_partition.unwrap().cap.nodes
                    ),
                    (7, 11)
                );
                let no_validation = CoupledPopulation {
                    factories: 3,
                    validations: 0,
                };
                let zero = input(&fixture, 7, no_validation).unwrap();
                assert_eq!(zero.factory, seven.factory);
                assert_eq!(zero.root_entries_upper, 3);
                assert_eq!(zero.input_partition.unwrap().attempts, 0);
                let no_factory = CoupledPopulation {
                    factories: 0,
                    validations: 2,
                };
                let validation_only = input(&fixture, 7, no_factory).unwrap();
                assert_eq!(validation_only.factory.entries_upper, 0);
                assert_eq!(
                    validation_only.input_partition.as_ref(),
                    seven.input_partition.as_ref()
                );
                assert_eq!(validation_only.required, seven.required);
            }
        }

        #[test]
        fn object_pass_locals_require_exact_source_bindings() {
            let fixture = local_fixture();
            let population = CoupledPopulation {
                factories: 3,
                validations: 2,
            };
            assert_eq!(
                input(&fixture, 7, population),
                Ok(expected_input(&fixture, true, true, 7, population, (5, 6)))
            );
            let value = json!({"a": true, "b": true, "c": true});
            let measured = measure_schema_input(
                &value,
                SchemaInputLimits {
                    serialized_bytes: 0x0001_0000,
                    nodes: 11,
                    depth: 32,
                },
            )
            .unwrap();
            assert_eq!((measured.nodes, value.as_object().unwrap().len()), (7, 3));
            // Full-success witness: one named-property leaf, two fallback leaves,
            // three callbacks. Separate lexical sites retain separate J ceilings.
            assert_eq!(
                value
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|key| key.as_str() == "a")
                    .count(),
                1
            );
            assert_eq!(
                value
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|key| key.as_str() != "a")
                    .count(),
                2
            );
            for validations in [0, 1] {
                let entries = CoupledPopulation {
                    factories: 3,
                    validations,
                };
                assert_eq!(
                    input(&fixture, 11, entries),
                    Ok(expected_input(
                        &fixture,
                        true,
                        true,
                        11,
                        entries,
                        (3_u64.checked_add(validations).unwrap(), 6)
                    ))
                );
                for drift in [
                    LocalDrift::Declaration,
                    LocalDrift::Assignment,
                    LocalDrift::Actual,
                    LocalDrift::Formal,
                ] {
                    let (changed, refusal) = local_mutant(&fixture, drift);
                    coupled_factory_bounds(&changed, &fixture.graph, OWNER, MARKER, 11, entries)
                        .unwrap();
                    assert_eq!(
                        coupled_input_bounds(&changed, &fixture.graph, OWNER, MARKER, 11, entries),
                        Err(refusal)
                    );
                }
            }
        }

        fn assert_input_source_mutants(fixture: &Fixture, population: CoupledPopulation) {
            let mut missing = fixture.recipe.clone();
            let validation = function(&missing, PUBLIC_VALIDATION);
            let position = missing
                .roots
                .iter()
                .position(|root| *root == validation)
                .unwrap();
            assert_eq!(missing.roots.remove(position), validation);
            assert_eq!(
                coupled_input_bounds(&missing, &fixture.graph, OWNER, MARKER, 7, population),
                Err(CoupledRefusal::RootBinding)
            );
            let mut initialization = fixture.recipe.clone();
            assert_eq!(
                initialization.initialization.remove(INITIALIZER),
                Some(programs(true))
            );
            assert_eq!(
                coupled_input_bounds(
                    &initialization,
                    &fixture.graph,
                    OWNER,
                    MARKER,
                    7,
                    population
                ),
                Err(CoupledRefusal::Initialization)
            );
            let mut unknown = fixture.recipe.clone();
            let leaf = function(&unknown, LEAF);
            let Kind::Scope { steps, .. } =
                &mut unknown.functions.get_mut(&leaf).unwrap().body.kind
            else {
                panic!("leaf scope");
            };
            let probe = &mut steps[0];
            let anchor = probe.site.clone();
            let mut unresolved = probe.clone();
            unresolved.kind = Kind::Call {
                target: usize::MAX,
                arguments: vec![],
                evaluation: vec![],
            };
            let Kind::Primitive { source_calls, .. } = &mut probe.kind else {
                panic!("leaf value access");
            };
            assert!(source_calls.is_empty());
            source_calls.push(unresolved);
            assert_eq!(
                coupled_input_bounds(&unknown, &fixture.graph, OWNER, MARKER, 7, population),
                Err(CoupledRefusal::SourceCalls(anchor))
            );
            let mut graph = fixture.graph.clone();
            assert_eq!(
                graph["auxiliary"]["roots"][0]["details"]["cache"],
                "CoupledSearchCache"
            );
            graph["auxiliary"]["roots"][0]["details"]["cache"] = json!("OtherSearchCache");
            assert_eq!(
                coupled_input_bounds(&fixture.recipe, &graph, OWNER, MARKER, 7, population),
                Err(CoupledRefusal::OwnerBinding)
            );
        }

        #[test]
        fn coupled_input_source_checks_and_overflow_survive_zero_validations() {
            let fixture = factory_fixture(true);
            let zero = CoupledPopulation {
                factories: 0,
                validations: 0,
            };
            assert_eq!(
                input(&fixture, 7, zero),
                Ok(expected_input(&fixture, true, false, 7, zero, (0, 0)))
            );
            let bounds = input(&fixture, 7, zero).unwrap();
            assert_eq!(bounds.factory.programs.len(), 2);
            assert_eq!(
                bounds.input_partition.as_ref().unwrap().helper_calls.len(),
                3
            );
            assert_eq!(
                bounds
                    .input_partition
                    .as_ref()
                    .unwrap()
                    .required_sources
                    .len(),
                5
            );
            for validations in [0, 1] {
                assert_input_source_mutants(
                    &fixture,
                    CoupledPopulation {
                        factories: 0,
                        validations,
                    },
                );
            }
            // Per-invocation affine arithmetic is checked before multiplying
            // by zero. Factory arithmetic alone accepts this independent cap.
            coupled_factory_bounds(
                &fixture.recipe,
                &fixture.graph,
                OWNER,
                MARKER,
                u64::MAX,
                zero,
            )
            .unwrap();
            assert_eq!(u64::MAX.checked_add(1), None);
            assert_eq!(
                input(&fixture, u64::MAX, zero),
                Err(CoupledRefusal::Overflow)
            );
            assert_eq!(input(&fixture, 0, zero), Err(CoupledRefusal::Overflow));
            assert_eq!(
                input(
                    &fixture,
                    7,
                    CoupledPopulation {
                        factories: u64::MAX,
                        validations: 0
                    }
                ),
                Err(CoupledRefusal::Overflow)
            );
            // No required source or ordinary-work/cleanup duty disappears at V=0.
            assert_eq!(bounds.required, super::duties());
            assert_eq!(bounds.initialization, fixture.recipe.initialization);
            assert_eq!(bounds.roots, fixture.recipe.roots);
            assert_eq!(
                function(&fixture.recipe, FACTORY),
                bounds.factory.helpers[0].target
            );
        }
    }
}

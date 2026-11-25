//! Conditional source-event counts; primitive work derivations remain pending.
//!
//! Binding/provenance below is DATA, not an artifact capability. The parent
//! joins these bytes to the sealed descriptor and its compiled-source census,
//! checking current regular-file bytes before/after the same capture lifetime.
//! This module neither opens that filesystem boundary nor invents checksums.
//! A supported source recipe, not a matching digest, must justify every count.
//!
//! Native clock calls = native entries + budget entries + post-evaluate checks:
//! early refusal 1, budget refusal 2, ordinary return 3, validator panic 2.
//! Missing parameters invoke native zero times but still visit retain/error/drop
//! paths. P includes those outer paths and every reached preflight, before debit.
//! V includes fresh cache construction, validation and release. Clock and seed
//! bodies remain separate `N_clock*H` and `N_seed*E` requirements; no warm TLS premise.
//! Seed multiplicity requires the matched construction closure, not search count.
//! No result here is `CommonSourceComplete`, a numeric work cost or qualification.
//! P begins at the schema caller after successful core admission. Whole-request
//! cleanup still needs the core prepare/services/operation, policy resolution,
//! runtime preflight/bounded-JSON, `OpenRTB` type and scrub/split-resource proofs.
//! Contexts omitting those caller caps are not covered by an actual-use claim.
//! Allocation success is a separate premise: fatal OOM diagnostics are excluded,
//! never ordinary returned-error, partial-construction or unwind cleanup.

mod caller;
mod context;
mod entropy;
mod epsilon;
pub(super) mod partition;
mod prefix;
use epsilon::primitives;
#[cfg(test)]
pub(super) use primitives::conditional_fixture::{
    assert_cost_projections, assert_demand_setup, conditional_controls, conditional_demands,
};
pub(super) use primitives::{
    ConditionalHelperCostProjection, ConditionalHelperDemand, HelperSourceJoinRefusal,
    Refusal as HelperInterpretationRefusal, SourceUnit as HelperSourceUnit,
};

use super::handoff::SourceInputs;
use super::operations::{self, Primitive, model::Recipe};
use auctioneer_bidder::schema::{
    SchemaEvaluationError, SchemaInputError, SchemaInputLimits, SchemaInputStats,
    measure_schema_input,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::str::from_utf8;

const NATIVE: &[u8] = include_bytes!("../../../src/schema/native.rs");
const BUDGET: &[u8] = include_bytes!("../../../src/schema/mod.rs");
const INPUT: &[u8] = include_bytes!("../../../src/schema/input.rs");
const JSON_SIZE: &[u8] = include_bytes!("../../../../auctioneer-runtime/src/json_size.rs");
const EXCHANGE_SCHEMA: &[u8] = include_bytes!("../../../../auctioneer-exchange/src/run/schema.rs");
const EXCHANGE_RUN: &[u8] = include_bytes!("../../../../auctioneer-exchange/src/run/mod.rs");
const EXCHANGE_SPLIT: &[u8] = include_bytes!("../../../../auctioneer-exchange/src/split/mod.rs");
const EXCHANGE_LIMITS: &[u8] = include_bytes!("../../../../auctioneer-exchange/src/limits.rs");
const DISPATCH_GENERATOR: &[u8] =
    include_bytes!("../../../../auctioneer-config/src/schema_staging_build.rs");
const FORMATS: &[u8] = include_bytes!("../../../src/schema/staging.rs");

// Borrow existing parent provenance; none of these values creates authority.
#[derive(Debug)]
pub(super) struct ContextBinding<'a> {
    descriptor_binding: &'a Value,
    capture_binding: &'a Value,
    context_roots: &'a Value,
    compiled_sources: &'a Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ContextSources<'a> {
    native: &'a [u8],
    budget: &'a [u8],
    input: &'a [u8],
    json_size: &'a [u8],
    exchange_schema: &'a [u8],
    exchange_run: &'a [u8],
    exchange_split: &'a [u8],
    exchange_limits: &'a [u8],
    dispatch_generator: &'a [u8],
    formats: &'a [u8],
}

#[derive(Debug)]
pub(super) struct AttemptContext<'a> {
    binding: &'a ContextBinding<'a>,
    sources: &'a ContextSources<'a>,
    native_input_limits: SchemaInputLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceRole {
    Native,
    Budget,
    Input,
    JsonSize,
    ExchangeSchema,
    ExchangeRun,
    ExchangeSplit,
    ExchangeLimits,
    DispatchGenerator,
    Formats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CallerFact {
    OriginalImpressions,
    BidderCap,
    PanicUnwind,
    ScopeGate,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    ScalarSource(prefix::ScalarRefusal),
    UnsupportedContext,
    SourceShape(SourceRole),
    BindingMismatch,
    MissingCallerFact(CallerFact),
    UnsupportedNativePath,
    NoSourceLeafDuty(Primitive),
    MissingRuntimeLemma(Primitive),
    InputRejected(SchemaInputError),
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePath {
    EntryExpired,
    Unavailable,
    InputRejected(SchemaInputError),
    BudgetRejected(SchemaEvaluationError),
    Returned { valid: bool },
    ValidatorPanicked,
}

#[derive(Clone, Copy, Debug)]
struct CallerFacts {
    original_impressions: Option<u64>,
    configured_bidder_cap: Option<u64>,
    panic_unwinds: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
struct Population {
    scopes_entered: u64,
    retain_visits: u64,
    missing_parameters: u64,
    native_attempts: u64,
    native_path: Option<NativePath>,
}

#[derive(Debug, Eq, PartialEq)]
struct InputCounts {
    stats: SchemaInputStats,
    value_visits: u64,
    key_visits: u64,
    container_entries: u64,
    scalar_serializations: u64,
    unresolved_calls: Vec<&'static str>,
}

#[derive(Debug, Eq, PartialEq)]
struct PrefixCounts {
    outer_scopes: u64,
    retain_visits: u64,
    missing_parameter_errors: u64,
    error_rows: u64,
    removed_impressions: u64,
    native_entries: u64,
    preflight_entries: u64,
    budget_entries: u64,
    debit_events: u64,
    post_evaluate_checks: u64,
    native_clock_calls: u64,
    fresh_cache_calls: u64,
    cache_drop_entries: u64,
    seed_invocations: Option<u64>,
    unresolved: Vec<Primitive>,
    // All-attempt source-prefix ceilings, not observed counts or helper costs.
    // None remains unproved; zero calls do not discharge helper-source duties.
    input_node_attempts_upper: Option<u64>,
    input_charged_nodes_upper: Option<u64>,
    input_container_entries_upper: Option<u64>,
    input_scalar_serializations_upper: Option<u64>,
    input_string_prechecks_upper: Option<u64>,
    input_byte_charge_calls_upper: Option<u64>,
    input_iterator_some_upper: Option<u64>,
    input_iterator_none_upper: Option<u64>,
    // Sum raw UTF-8 length + two quotes only for strings/keys passing the
    // precheck and reaching serialization, including a final serialization
    // whose byte charge fails. Exclude rejected precheck operands.
    input_raw_utf8_with_quotes_upper: Option<u64>,
    input_visit_nesting_upper: Option<u64>,
    input_iterator_nesting_upper: Option<u64>,
    input_counter_writes_per_nonempty_request_upper: Option<u64>,
    input_counter_writes_per_empty_request_upper: Option<u64>,
    input_counter_interrupted_retries_upper: Option<u64>,
    // Native B/J/D do not bound the caller-owned map's unvisited population.
    input_tree_navigation_upper: Option<u64>,
    input_unresolved_calls: Vec<&'static str>,
}

#[derive(Debug, Eq, PartialEq)]
struct RecipeCounts {
    root_entries: u64,
    helpers: BTreeMap<String, u64>,
    iterator_callbacks: u64,
    searches: u64,
    fresh_cache_calls: u64,
    cache_drop_entries: u64,
    seed_invocations: Option<u64>,
    unresolved: Vec<Primitive>,
}

// Conformance-only seam: matching helper bytes is not receiver/invariant,
// invocation, source-completeness or numeric work authority.
pub(super) fn check_helper_source_setup(
    inputs: &SourceInputs<'_>,
) -> Result<(), HelperSourceJoinRefusal> {
    primitives::check_consumed_source_setup(inputs)
}

pub(super) fn join_helper_costs(
    inputs: &SourceInputs<'_>,
    demand: &ConditionalHelperDemand,
) -> Result<ConditionalHelperCostProjection, HelperSourceJoinRefusal> {
    primitives::join_consumed_helper_costs(inputs, demand)
}

// This validates conditional source data, not a sealed descriptor capability.
pub(super) fn bind_attempt_context<'a>(
    binding: &'a ContextBinding<'a>,
    sources: &'a ContextSources<'a>,
) -> Result<AttemptContext<'a>, Refusal> {
    let native_input_limits = context::check(binding, sources)?;
    Ok(AttemptContext {
        binding,
        sources,
        native_input_limits,
    })
}

fn input_counts(
    context: &AttemptContext<'_>,
    value: &Value,
    limits: SchemaInputLimits,
) -> Result<InputCounts, Refusal> {
    if limits != context.native_input_limits {
        return Err(Refusal::UnsupportedContext);
    }
    let stats = measure_schema_input(value, limits).map_err(Refusal::InputRejected)?;
    let mut counts = InputCounts {
        stats,
        value_visits: 0,
        key_visits: 0,
        container_entries: 0,
        scalar_serializations: 0,
        unresolved_calls: Vec::new(),
    };
    // Project the checked input recipe's events, not a second byte-admission
    // implementation. The production owner above supplies exact B/J/D.
    let mut pending = vec![value];
    while let Some(current) = pending.pop() {
        checked_add(&mut counts.value_visits, 1)?;
        match current {
            Value::Array(items) => {
                checked_add(&mut counts.container_entries, 1)?;
                pending.extend(items);
            }
            Value::Object(items) => {
                let keys = u64::try_from(items.len()).map_err(|_error| Refusal::Overflow)?;
                checked_add(&mut counts.container_entries, 1)?;
                checked_add(&mut counts.key_visits, keys)?;
                checked_add(&mut counts.scalar_serializations, keys)?;
                pending.extend(items.values());
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                checked_add(&mut counts.scalar_serializations, 1)?;
            }
        }
    }
    if counts.scalar_serializations != 0 {
        counts.unresolved_calls.push("serde_json::to_writer");
    }
    if counts.container_entries != 0 {
        counts.unresolved_calls.push("IteratorProtocol");
    }
    Ok(counts)
}

fn prefix_counts(
    context: &AttemptContext<'_>,
    scalar: &prefix::ScalarData,
    facts: CallerFacts,
    population: Population,
) -> Result<PrefixCounts, Refusal> {
    let original = facts
        .original_impressions
        .ok_or(Refusal::MissingCallerFact(CallerFact::OriginalImpressions))?;
    let cap = facts
        .configured_bidder_cap
        .ok_or(Refusal::MissingCallerFact(CallerFact::BidderCap))?;
    let maximum_visits = original
        .checked_mul(population.scopes_entered)
        .ok_or(Refusal::Overflow)?;
    let classified_visits = population
        .missing_parameters
        .checked_add(population.native_attempts)
        .ok_or(Refusal::Overflow)?;
    if population.scopes_entered > cap
        || population.retain_visits > maximum_visits
        || classified_visits != population.retain_visits
        || (population.native_attempts == 0) != population.native_path.is_none()
    {
        return Err(Refusal::UnsupportedContext);
    }
    // This API describes reached retain predicates. A skipped/unfunded scope
    // needs its own caller-gate fact, not a fabricated zero-work scope.
    if population.scopes_entered != 0 && population.retain_visits == 0 {
        return Err(Refusal::MissingCallerFact(CallerFact::ScopeGate));
    }
    let (preflight, budget, charged, post, failed) = native_prefix_path(facts, population)?;
    let native = population.native_attempts;
    let preflight_entries = if preflight { native } else { 0 };
    let budget_entries = if budget { native } else { 0 };
    let debit_events = if charged { native } else { 0 };
    let post_evaluate_checks = if post { native } else { 0 };
    let native_clock_calls = native
        .checked_add(budget_entries)
        .and_then(|count| count.checked_add(post_evaluate_checks))
        .ok_or(Refusal::Overflow)?;
    let error_rows = population
        .missing_parameters
        .checked_add(if failed { native } else { 0 })
        .ok_or(Refusal::Overflow)?;
    let mut counts = PrefixCounts {
        outer_scopes: population.scopes_entered,
        retain_visits: population.retain_visits,
        missing_parameter_errors: population.missing_parameters,
        error_rows,
        removed_impressions: error_rows,
        native_entries: native,
        preflight_entries,
        budget_entries,
        debit_events,
        post_evaluate_checks,
        native_clock_calls,
        fresh_cache_calls: debit_events,
        cache_drop_entries: debit_events,
        seed_invocations: None,
        unresolved: vec![Primitive::CacheConstruction, Primitive::Cleanup],
        input_node_attempts_upper: None,
        input_charged_nodes_upper: None,
        input_container_entries_upper: None,
        input_scalar_serializations_upper: None,
        input_string_prechecks_upper: None,
        input_byte_charge_calls_upper: None,
        input_iterator_some_upper: None,
        input_iterator_none_upper: None,
        input_raw_utf8_with_quotes_upper: None,
        input_visit_nesting_upper: None,
        input_iterator_nesting_upper: None,
        input_counter_writes_per_nonempty_request_upper: None,
        input_counter_writes_per_empty_request_upper: None,
        input_counter_interrupted_retries_upper: None,
        input_tree_navigation_upper: None,
        input_unresolved_calls: vec![
            "serde_json::to_writer",
            "IteratorProtocol",
            "caller-owned BTreeMap height/navigation",
            "ordinary preflight error/unwind cleanup",
        ],
    };
    prefix::attach_bounds(context, scalar, &mut counts)?;
    Ok(counts)
}

fn native_prefix_path(
    facts: CallerFacts,
    population: Population,
) -> Result<(bool, bool, bool, bool, bool), Refusal> {
    let shape = match population.native_path {
        None => (false, false, false, false, false),
        Some(NativePath::EntryExpired | NativePath::Unavailable) => {
            (false, false, false, false, true)
        }
        Some(NativePath::InputRejected(SchemaInputError::InvalidDepthLimit)) => {
            return Err(Refusal::UnsupportedNativePath);
        }
        Some(NativePath::InputRejected(
            SchemaInputError::Bytes
            | SchemaInputError::Nodes
            | SchemaInputError::Depth
            | SchemaInputError::Representation,
        )) => (true, false, false, false, true),
        Some(NativePath::BudgetRejected(
            SchemaEvaluationError::Deadline | SchemaEvaluationError::AggregateWork,
        )) => (true, true, false, false, true),
        Some(NativePath::BudgetRejected(
            SchemaEvaluationError::CostOverflow
            | SchemaEvaluationError::ZeroWork
            | SchemaEvaluationError::SingleWork
            | SchemaEvaluationError::Scratch,
        )) => return Err(Refusal::UnsupportedNativePath),
        Some(NativePath::Returned { valid }) => (true, true, true, true, !valid),
        Some(NativePath::ValidatorPanicked) => {
            match facts.panic_unwinds {
                None => return Err(Refusal::MissingCallerFact(CallerFact::PanicUnwind)),
                Some(false) => return Err(Refusal::UnsupportedNativePath),
                Some(true) => {}
            }
            if population.native_attempts != 1 {
                return Err(Refusal::UnsupportedNativePath);
            }
            (true, true, true, false, false)
        }
    };
    Ok(shape)
}

fn recipe_counts(
    context: &AttemptContext<'_>,
    recipe: &Recipe,
    dimensions: SchemaInputStats,
    attempts: u64,
) -> Result<RecipeCounts, Refusal> {
    if recipe.roots.is_empty()
        || dimensions.nodes == 0
        || dimensions.serialized_bytes > context.native_input_limits.serialized_bytes
        || dimensions.nodes > context.native_input_limits.nodes
        || dimensions.max_depth > context.native_input_limits.depth
    {
        return Err(Refusal::UnsupportedContext);
    }
    let mut counts = empty_recipe_counts();
    counts.root_entries = attempts
        .checked_mul(u64::try_from(recipe.roots.len()).map_err(|_error| Refusal::Overflow)?)
        .ok_or(Refusal::Overflow)?;
    let mut active = BTreeSet::new();
    for root in &recipe.roots {
        let function = recipe
            .functions
            .get(root)
            .ok_or(Refusal::UnsupportedContext)?;
        if !active.insert(*root) {
            return Err(Refusal::UnsupportedContext);
        }
        let root_counts = count_operation(
            recipe,
            &function.body,
            dimensions.nodes,
            attempts,
            &mut active,
        )?;
        active.remove(root);
        add_recipe_counts(&mut counts, root_counts)?;
    }
    // Source requirements survive zero invocation. A synthetic zero count is
    // not an absence proof for the ordinary primitive closure or hash seeding.
    counts.unresolved = source_duties(recipe)?.into_iter().collect();
    // These are individual engine-cache constructor/drop bounds, unlike the
    // native prefix's validation-wrapper episode count. Fields remain distinct.
    counts.cache_drop_entries = counts.fresh_cache_calls;
    Ok(counts)
}

fn empty_recipe_counts() -> RecipeCounts {
    RecipeCounts {
        root_entries: 0,
        helpers: BTreeMap::new(),
        iterator_callbacks: 0,
        searches: 0,
        fresh_cache_calls: 0,
        cache_drop_entries: 0,
        seed_invocations: None,
        unresolved: Vec::new(),
    }
}

fn add_recipe_counts(total: &mut RecipeCounts, next: RecipeCounts) -> Result<(), Refusal> {
    checked_add(&mut total.root_entries, next.root_entries)?;
    checked_add(&mut total.iterator_callbacks, next.iterator_callbacks)?;
    checked_add(&mut total.searches, next.searches)?;
    checked_add(&mut total.fresh_cache_calls, next.fresh_cache_calls)?;
    checked_add(&mut total.cache_drop_entries, next.cache_drop_entries)?;
    for (helper, visits) in next.helpers {
        checked_add(total.helpers.entry(helper).or_default(), visits)?;
    }
    Ok(())
}

fn maximum_recipe_counts(total: &mut RecipeCounts, next: RecipeCounts) {
    total.root_entries = total.root_entries.max(next.root_entries);
    total.iterator_callbacks = total.iterator_callbacks.max(next.iterator_callbacks);
    total.searches = total.searches.max(next.searches);
    total.fresh_cache_calls = total.fresh_cache_calls.max(next.fresh_cache_calls);
    total.cache_drop_entries = total.cache_drop_entries.max(next.cache_drop_entries);
    for (helper, visits) in next.helpers {
        let prior = total.helpers.entry(helper).or_default();
        *prior = (*prior).max(visits);
    }
}

fn count_sequence(
    recipe: &Recipe,
    steps: &[operations::model::Op],
    nodes: u64,
    visits: u64,
    active: &mut BTreeSet<usize>,
) -> Result<RecipeCounts, Refusal> {
    let mut total = empty_recipe_counts();
    for step in steps {
        add_recipe_counts(
            &mut total,
            count_operation(recipe, step, nodes, visits, active)?,
        )?;
        if matches!(
            &step.kind,
            operations::model::Kind::Exit(
                operations::model::Exit::Return(_) | operations::model::Exit::Break(_),
                _
            )
        ) {
            break;
        }
    }
    Ok(total)
}

fn count_choice(
    recipe: &Recipe,
    selector: &operations::model::Op,
    arms: &[operations::model::Arm],
    nodes: u64,
    visits: u64,
    active: &mut BTreeSet<usize>,
) -> Result<RecipeCounts, Refusal> {
    let mut total = count_operation(recipe, selector, nodes, visits, active)?;
    let mut alternatives = empty_recipe_counts();
    // A failed guard can precede a later selected body. Guards sum;
    // only bodies of the same source match/if are exclusive.
    for arm in arms {
        if let Some(guard) = &arm.guard {
            // Nested or-pattern alternatives can retest a failed guard.
            // A once-per-arm count is not a bound for that protocol.
            if pattern_has_alternatives(&arm.pattern) {
                return Err(Refusal::UnsupportedContext);
            }
            add_recipe_counts(
                &mut total,
                count_operation(recipe, guard, nodes, visits, active)?,
            )?;
        }
        maximum_recipe_counts(
            &mut alternatives,
            count_operation(recipe, &arm.body, nodes, visits, active)?,
        );
    }
    add_recipe_counts(&mut total, alternatives)?;
    Ok(total)
}

fn count_called(
    recipe: &Recipe,
    target: usize,
    evaluation: &[operations::model::Op],
    nodes: u64,
    visits: u64,
    active: &mut BTreeSet<usize>,
) -> Result<RecipeCounts, Refusal> {
    let mut total = count_sequence(recipe, evaluation, nodes, visits, active)?;
    let function = recipe
        .functions
        .get(&target)
        .ok_or(Refusal::UnsupportedContext)?;
    if !active.insert(target) {
        return Err(Refusal::UnsupportedContext);
    }
    checked_add(
        total.helpers.entry(function.identity.clone()).or_default(),
        visits,
    )?;
    let called = count_operation(recipe, &function.body, nodes, visits, active)?;
    active.remove(&target);
    add_recipe_counts(&mut total, called)?;
    Ok(total)
}

fn count_iteration(
    recipe: &Recipe,
    bound: &operations::model::IterationBound,
    body: &operations::model::Op,
    evaluation: &operations::model::Op,
    nodes: u64,
    visits: u64,
    active: &mut BTreeSet<usize>,
) -> Result<RecipeCounts, Refusal> {
    use operations::model::Ty;

    let mut total = count_operation(recipe, evaluation, nodes, visits, active)?;
    let remaining = nodes.checked_sub(1).ok_or(Refusal::UnsupportedContext)?;
    let members = match &bound.collection.ty {
        Ty::Array => remaining,
        Ty::Map => remaining.checked_div(2).ok_or(Refusal::Overflow)?,
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
        | Ty::Unit => return Err(Refusal::UnsupportedContext),
    };
    let callbacks = visits.checked_mul(members).ok_or(Refusal::Overflow)?;
    checked_add(&mut total.iterator_callbacks, callbacks)?;
    // This deliberately retains a global subtree ceiling per nested
    // pass. No disjoint-child optimization is inferred from an origin
    // label; a stronger aggregate formula needs its own source proof.
    add_recipe_counts(
        &mut total,
        count_operation(recipe, body, nodes, callbacks, active)?,
    )?;
    Ok(total)
}

fn count_operation(
    recipe: &Recipe,
    op: &operations::model::Op,
    nodes: u64,
    visits: u64,
    active: &mut BTreeSet<usize>,
) -> Result<RecipeCounts, Refusal> {
    use operations::model::Kind;
    let mut total = empty_recipe_counts();
    match &op.kind {
        Kind::Value(_) => {}
        Kind::Scope { steps, .. }
        | Kind::Evaluate(_, steps)
        | Kind::ScalarTest(_, steps)
        | Kind::ShortCircuit(_, steps) => {
            // Both short-circuit operands may execute. Keeping all occurrences
            // is conservative; it does not claim every input takes that path.
            total = count_sequence(recipe, steps, nodes, visits, active)?;
        }
        Kind::Bind { value, .. } => total = count_operation(recipe, value, nodes, visits, active)?,
        Kind::Choice { selector, arms } => {
            total = count_choice(recipe, selector, arms, nodes, visits, active)?;
        }
        Kind::Call {
            target, evaluation, ..
        } => {
            total = count_called(recipe, *target, evaluation, nodes, visits, active)?;
        }
        Kind::Primitive {
            lemma,
            arguments,
            source_calls,
            ..
        } => {
            total = count_sequence(recipe, arguments, nodes, visits, active)?;
            add_recipe_counts(
                &mut total,
                count_sequence(recipe, source_calls, nodes, visits, active)?,
            )?;
            match lemma {
                Primitive::RegexSearch => checked_add(&mut total.searches, visits)?,
                Primitive::CacheConstruction => checked_add(&mut total.fresh_cache_calls, visits)?,
                Primitive::UuidFormat
                | Primitive::StateMapConstruction
                | Primitive::ValueAccess
                | Primitive::StringAccess
                | Primitive::MapAccess
                | Primitive::IteratorProtocol
                | Primitive::Numeric
                | Primitive::Readiness
                | Primitive::HostnameFormat
                | Primitive::Equality
                | Primitive::Uniqueness
                | Primitive::Cleanup => {}
            }
        }
        Kind::Service(_) => return Err(Refusal::UnsupportedContext),
        Kind::Iterate {
            bound,
            body,
            evaluation,
            ..
        } => {
            total = count_iteration(recipe, bound, body, evaluation, nodes, visits, active)?;
        }
        Kind::Callback {
            body, evaluation, ..
        } => {
            total = count_operation(recipe, evaluation, nodes, visits, active)?;
            add_recipe_counts(
                &mut total,
                count_operation(recipe, body, nodes, visits, active)?,
            )?;
        }
        Kind::Construct { fields, .. } => {
            for (_, field) in fields {
                add_recipe_counts(
                    &mut total,
                    count_operation(recipe, field, nodes, visits, active)?,
                )?;
            }
        }
        Kind::Exit(_, value) => {
            if let Some(exit_value) = value {
                total = count_operation(recipe, exit_value, nodes, visits, active)?;
            }
        }
    }
    Ok(total)
}

fn pattern_has_alternatives(pattern: &operations::model::Pattern) -> bool {
    use operations::model::Pattern;
    match pattern {
        Pattern::Or(_) => true,
        Pattern::Variant(_, fields) | Pattern::Tuple(fields) => {
            fields.iter().any(pattern_has_alternatives)
        }
        Pattern::Wild | Pattern::Binding(_) | Pattern::Literal(_) => false,
    }
}

fn require_source_leaf_cost(
    _context: &AttemptContext<'_>,
    recipe: &Recipe,
    duty: Primitive,
) -> Result<(), Refusal> {
    if source_duties(recipe)?.contains(&duty) {
        Err(Refusal::MissingRuntimeLemma(duty))
    } else {
        Err(Refusal::NoSourceLeafDuty(duty))
    }
}

fn checked_add(total: &mut u64, value: u64) -> Result<(), Refusal> {
    *total = total.checked_add(value).ok_or(Refusal::Overflow)?;
    Ok(())
}

fn source_duties(recipe: &Recipe) -> Result<BTreeSet<Primitive>, Refusal> {
    use operations::model::{Kind, walk};
    let mut pending = recipe.roots.clone();
    let mut visited = BTreeSet::new();
    let mut duties = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let function = recipe
            .functions
            .get(&id)
            .ok_or(Refusal::UnsupportedContext)?;
        walk(&function.body, &mut |op| {
            match &op.kind {
                Kind::Call { target, .. } => pending.push(*target),
                Kind::Primitive { lemma, .. }
                | Kind::Construct {
                    cleanup: Some(lemma),
                    ..
                } => {
                    duties.insert(*lemma);
                }
                Kind::Iterate {
                    protocol,
                    parameters,
                    ..
                }
                | Kind::Callback {
                    protocol,
                    parameters,
                    ..
                } => {
                    duties.insert(*protocol);
                    if parameters.iter().any(operations::model::needs_cleanup) {
                        duties.insert(Primitive::Cleanup);
                    }
                }
                Kind::Scope { locals, .. }
                    if locals.iter().any(operations::model::needs_cleanup) =>
                {
                    duties.insert(Primitive::Cleanup);
                }
                Kind::Value(_)
                | Kind::Scope { .. }
                | Kind::Bind { .. }
                | Kind::Evaluate(_, _)
                | Kind::ScalarTest(_, _)
                | Kind::Choice { .. }
                | Kind::ShortCircuit(_, _)
                | Kind::Service(_)
                | Kind::Construct { cleanup: None, .. }
                | Kind::Exit(_, _) => {}
            }
            Ok(())
        })
        .map_err(|_error| Refusal::UnsupportedContext)?;
    }
    Ok(duties)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens as _;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::fmt::Debug;
    use syn::visit_mut::{self, VisitMut};

    fn sources() -> ContextSources<'static> {
        ContextSources {
            native: NATIVE,
            budget: BUDGET,
            input: INPUT,
            json_size: JSON_SIZE,
            exchange_schema: EXCHANGE_SCHEMA,
            exchange_run: EXCHANGE_RUN,
            exchange_split: EXCHANGE_SPLIT,
            exchange_limits: EXCHANGE_LIMITS,
            dispatch_generator: DISPATCH_GENERATOR,
            formats: FORMATS,
        }
    }

    fn successful<T: Debug>(result: Result<T, Refusal>) -> T {
        assert_eq!(result.as_ref().map(|_| ()), Ok(()));
        result.unwrap()
    }

    fn context<'a>(
        binding: &'a ContextBinding<'a>,
        sources: &'a ContextSources<'a>,
    ) -> AttemptContext<'a> {
        // These independent field checks prevent a role from silently borrowing
        // a different source. They are data checks, not artifact certification.
        for (role, actual, expected) in [
            (SourceRole::Native, sources.native, NATIVE),
            (SourceRole::Budget, sources.budget, BUDGET),
            (SourceRole::Input, sources.input, INPUT),
            (SourceRole::JsonSize, sources.json_size, JSON_SIZE),
            (
                SourceRole::ExchangeSchema,
                sources.exchange_schema,
                EXCHANGE_SCHEMA,
            ),
            (SourceRole::ExchangeRun, sources.exchange_run, EXCHANGE_RUN),
            (
                SourceRole::ExchangeSplit,
                sources.exchange_split,
                EXCHANGE_SPLIT,
            ),
            (
                SourceRole::ExchangeLimits,
                sources.exchange_limits,
                EXCHANGE_LIMITS,
            ),
            (
                SourceRole::DispatchGenerator,
                sources.dispatch_generator,
                DISPATCH_GENERATOR,
            ),
            (SourceRole::Formats, sources.formats, FORMATS),
        ] {
            assert_eq!(actual, expected, "source role: {role:?}");
        }
        let checked = successful(bind_attempt_context(binding, sources));
        assert_eq!(
            checked.binding.descriptor_binding,
            binding.descriptor_binding
        );
        assert_eq!(checked.binding.capture_binding, binding.capture_binding);
        assert_eq!(checked.binding.context_roots, binding.context_roots);
        assert_eq!(checked.binding.compiled_sources, binding.compiled_sources);
        assert_eq!(checked.sources, sources);
        assert_eq!(
            checked.native_input_limits,
            SchemaInputLimits {
                serialized_bytes: 0x0001_0000,
                nodes: 4_096,
                depth: 32,
            }
        );
        checked
    }

    fn facts() -> CallerFacts {
        CallerFacts {
            original_impressions: Some(3),
            configured_bidder_cap: Some(2),
            panic_unwinds: Some(true),
        }
    }

    fn population(path: NativePath) -> Population {
        Population {
            scopes_entered: 1,
            retain_visits: 1,
            missing_parameters: 0,
            native_attempts: 1,
            native_path: Some(path),
        }
    }

    fn assert_native_prefix(
        context: &AttemptContext<'_>,
        scalar: &prefix::ScalarData,
        path: NativePath,
        expected: [u64; 9],
    ) {
        let counts = successful(prefix_counts(context, scalar, facts(), population(path)));
        assert_eq!(counts.outer_scopes, 1);
        assert_eq!(counts.retain_visits, 1);
        assert_eq!(counts.missing_parameter_errors, 0);
        assert_eq!(counts.native_entries, 1);
        assert_eq!(
            [
                counts.preflight_entries,
                counts.budget_entries,
                counts.debit_events,
                counts.post_evaluate_checks,
                counts.native_clock_calls,
                counts.fresh_cache_calls,
                counts.cache_drop_entries,
                counts.error_rows,
                counts.removed_impressions,
            ],
            expected,
            "native path: {path:?}"
        );
    }

    fn classified(syntax: &syn::File) -> operations::BodyOperations {
        let result = operations::classify_source_body(syntax, &[], &[]);
        assert_eq!(result.as_ref().map(|_| ()), Ok(()));
        result.unwrap()
    }

    // Presence only, not an execution-count oracle. Follow source call edges
    // from roots so an unused helper cannot supply an obligation.
    fn reachable_duties(recipe: &Recipe) -> BTreeSet<Primitive> {
        use operations::model::{Kind, walk};

        let mut pending = recipe.roots.clone();
        let mut visited = BTreeSet::new();
        let mut duties = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let function = recipe
                .functions
                .get(&id)
                .expect("classified reachable function");
            walk(&function.body, &mut |op| {
                match &op.kind {
                    Kind::Call { target, .. } => pending.push(*target),
                    Kind::Primitive { lemma, .. }
                    | Kind::Construct {
                        cleanup: Some(lemma),
                        ..
                    } => {
                        duties.insert(*lemma);
                    }
                    Kind::Iterate { protocol, .. } | Kind::Callback { protocol, .. } => {
                        duties.insert(*protocol);
                    }
                    Kind::Value(_)
                    | Kind::Scope { .. }
                    | Kind::Bind { .. }
                    | Kind::Evaluate(_, _)
                    | Kind::ScalarTest(_, _)
                    | Kind::Choice { .. }
                    | Kind::ShortCircuit(_, _)
                    | Kind::Service(_)
                    | Kind::Construct { cleanup: None, .. }
                    | Kind::Exit(_, _) => {}
                }
                Ok(())
            })
            .unwrap();
        }
        duties
    }

    fn callback_body() -> syn::File {
        syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                is_branch_valid_0(instance) && is_branch_valid_0(instance)
            }
            fn is_branch_valid_0(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::Array(items) => items.iter().all(|item| is_branch_valid_1(item)),
                    _ => false,
                }
            }
            fn is_branch_valid_1(instance: &serde_json::Value) -> bool {
                match instance { serde_json::Value::Number(_) => true, _ => false }
            }
        }
    }

    // Mutations inspect handwritten caller source only. Generated bodies are
    // classified once by operations, then borrowed as its unique Recipe.
    fn edited(source: &[u8], name: &str, change: &mut dyn FnMut(&mut syn::Block)) -> Vec<u8> {
        struct Edit<'a> {
            name: &'a str,
            change: &'a mut dyn FnMut(&mut syn::Block),
            hits: usize,
        }
        impl VisitMut for Edit<'_> {
            fn visit_item_fn_mut(&mut self, i: &mut syn::ItemFn) {
                if i.sig.ident == self.name {
                    (self.change)(&mut i.block);
                    self.hits = self.hits.checked_add(1).expect("bounded caller root count");
                } else {
                    visit_mut::visit_item_fn_mut(self, i);
                }
            }
            fn visit_impl_item_fn_mut(&mut self, i: &mut syn::ImplItemFn) {
                if i.sig.ident == self.name {
                    (self.change)(&mut i.block);
                    self.hits = self.hits.checked_add(1).expect("bounded caller root count");
                } else {
                    visit_mut::visit_impl_item_fn_mut(self, i);
                }
            }
        }
        let mut syntax = syn::parse_file(from_utf8(source).unwrap()).unwrap();
        let mut edit = Edit {
            name,
            change,
            hits: 0,
        };
        edit.visit_file_mut(&mut syntax);
        assert_eq!(edit.hits, 1, "one actual caller root: {name}");
        syntax.to_token_stream().to_string().into_bytes()
    }

    #[test]
    fn dimensions_vary_independently() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            capture_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        // B varies with J/D fixed; J varies with B/D fixed; D varies with B/J fixed.
        for (value, bytes, nodes, depth, values, keys, containers, serializations) in [
            (json!("a"), 3, 1, 0, 1, 0, 0, 1),
            (json!("abcd"), 6, 1, 0, 1, 0, 0, 1),
            (json!([1_i32, 2_i32]), 5, 3, 1, 3, 0, 1, 2),
            (json!([123_i32]), 5, 2, 1, 2, 0, 1, 1),
            (json!([[0_i32]]), 5, 3, 2, 3, 0, 2, 1),
            (json!([]), 2, 1, 1, 1, 0, 1, 0),
            (json!({}), 2, 1, 1, 1, 0, 1, 0),
            (json!(false), 5, 1, 0, 1, 0, 0, 1),
            (
                json!({"\u{00e9}\"\n":"\u{1f642}\\\t\u{0000}"}),
                27,
                3,
                1,
                2,
                1,
                1,
                2,
            ),
        ] {
            let expected = SchemaInputStats {
                serialized_bytes: bytes,
                nodes,
                max_depth: depth,
            };
            assert_eq!(
                measure_schema_input(&value, context.native_input_limits),
                Ok(expected)
            );
            let counts = successful(input_counts(&context, &value, context.native_input_limits));
            assert_eq!(counts.stats, expected);
            assert_eq!(counts.value_visits, values);
            assert_eq!(counts.key_visits, keys);
            assert_eq!(counts.container_entries, containers);
            assert_eq!(counts.scalar_serializations, serializations);
            if serializations != 0 {
                assert!(counts.unresolved_calls.contains(&"serde_json::to_writer"));
            }
        }
        let changed = edited(INPUT, "measure_schema_input", &mut |body| {
            assert_eq!(body.stmts.len(), 4);
            let guard = body.stmts.remove(0);
            body.stmts.insert(2, guard); // Traversal before the invalid-depth refusal.
        });
        let changed_sources = ContextSources {
            input: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::Input))
        );
        let json_changed = edited(JSON_SIZE, "serialized_json_bytes", &mut |body| {
            *body = syn::parse_quote!({
                let bytes = serde_json::to_vec(value)?;
                Ok(u64::try_from(bytes.len()).unwrap())
            });
        });
        let json_sources = ContextSources {
            json_size: &json_changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &json_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::JsonSize))
        );
    }

    #[test]
    fn expiry_prefixes_bound_real_call_sites() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            capture_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let (scalar, scalar_limits) = prefix::scalar_fixture::fixture();
        assert_eq!(scalar_limits, context.native_input_limits);
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::EntryExpired,
            [0, 0, 0, 0, 1, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::Unavailable,
            [0, 0, 0, 0, 1, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::InputRejected(SchemaInputError::Bytes),
            [1, 0, 0, 0, 1, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::BudgetRejected(SchemaEvaluationError::Deadline),
            [1, 1, 0, 0, 2, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::BudgetRejected(SchemaEvaluationError::AggregateWork),
            [1, 1, 0, 0, 2, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::Returned { valid: true },
            [1, 1, 1, 1, 3, 1, 1, 0, 0],
        );
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::Returned { valid: false },
            [1, 1, 1, 1, 3, 1, 1, 1, 1],
        );
        assert_native_prefix(
            &context,
            &scalar,
            NativePath::ValidatorPanicked,
            [1, 1, 1, 0, 2, 1, 1, 0, 0],
        );
        let changed = edited(NATIVE, "validate_native_parameters", &mut |body| {
            body.stmts.swap(0, 1);
        });
        let changed_sources = ContextSources {
            native: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::Native))
        );
        let budget_changed = edited(BUDGET, "evaluate", &mut |body| {
            let tail = body.stmts.pop().unwrap();
            let debit = body.stmts.pop().unwrap();
            assert!(matches!(
                &debit,
                syn::Stmt::Expr(syn::Expr::Assign(_), Some(_))
            ));
            assert!(matches!(&tail, syn::Stmt::Expr(syn::Expr::Call(_), None)));
            body.stmts
                .push(syn::parse_quote!(let result = Ok(validate());));
            body.stmts.push(debit);
            body.stmts
                .push(syn::Stmt::Expr(syn::parse_quote!(result), None));
        });
        let budget_sources = ContextSources {
            budget: &budget_changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &budget_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::Budget))
        );
    }

    fn assert_preflight_refusals(context: &AttemptContext<'_>, scalar: &prefix::ScalarData) {
        for reason in [
            SchemaInputError::Bytes,
            SchemaInputError::Nodes,
            SchemaInputError::Depth,
            SchemaInputError::Representation,
        ] {
            let attempts = Population {
                native_attempts: 3,
                retain_visits: 3,
                ..population(NativePath::InputRejected(reason))
            };
            let counts = successful(prefix_counts(context, scalar, facts(), attempts));
            assert_eq!(
                (
                    counts.native_entries,
                    counts.preflight_entries,
                    counts.native_clock_calls
                ),
                (3, 3, 3)
            );
            assert_eq!(
                (
                    counts.budget_entries,
                    counts.debit_events,
                    counts.fresh_cache_calls
                ),
                (0, 0, 0)
            );
        }
    }

    fn assert_caller_source_binding(binding: &ContextBinding<'_>, sources: ContextSources<'_>) {
        let changed = edited(EXCHANGE_SCHEMA, "retain_valid", &mut |body| {
            *body = syn::parse_quote!({
                request.imp.retain(|imp| {
                    let value = imp
                        .ext
                        .as_ref()
                        .and_then(|ext| ext.bidder.as_ref())
                        .unwrap_or(&Value::Null);
                    match validate(value) {
                        Ok(()) => true,
                        Err(reason) => {
                            errors.push(error(reason));
                            false
                        }
                    }
                });
            });
        });
        let changed_sources = ContextSources {
            exchange_schema: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::ExchangeSchema))
        );
        let other = json!({"identity":"fixture_capture_b"});
        let other_binding = ContextBinding {
            descriptor_binding: &other,
            ..*binding
        };
        assert_eq!(
            bind_attempt_context(&other_binding, &sources).map(|_| ()),
            Err(Refusal::BindingMismatch)
        );
    }

    #[test]
    fn attempts_before_admission_are_charged() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            capture_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let (scalar, scalar_limits) = prefix::scalar_fixture::fixture();
        assert_eq!(scalar_limits, context.native_input_limits);
        let mut outer_only = population(NativePath::Returned { valid: true });
        let before = successful(prefix_counts(&context, &scalar, facts(), outer_only));
        assert_eq!((before.native_entries, before.debit_events), (1, 1));
        assert_eq!(outer_only.scopes_entered, 1);
        assert_eq!(outer_only.retain_visits, 1);
        outer_only.retain_visits = 3;
        assert_eq!(outer_only.missing_parameters, 0);
        outer_only.missing_parameters = 3;
        assert_eq!(outer_only.native_attempts, 1);
        outer_only.native_attempts = 0;
        assert_eq!(
            outer_only.native_path.take(),
            Some(NativePath::Returned { valid: true })
        );
        let counts = successful(prefix_counts(&context, &scalar, facts(), outer_only));
        assert_eq!(
            (
                counts.outer_scopes,
                counts.retain_visits,
                counts.error_rows,
                counts.missing_parameter_errors,
                counts.removed_impressions
            ),
            (1, 3, 3, 3, 3)
        );
        assert_eq!(
            (
                counts.native_entries,
                counts.preflight_entries,
                counts.budget_entries,
                counts.debit_events,
                counts.native_clock_calls,
                counts.fresh_cache_calls
            ),
            (0, 0, 0, 0, 0, 0)
        );
        assert!(counts.unresolved.contains(&Primitive::Cleanup));
        assert_preflight_refusals(&context, &scalar);
        // These generic API errors cannot arise from native's fixed, positive,
        // bounded projection. Do not manufacture additional native paths.
        for path in [
            NativePath::InputRejected(SchemaInputError::InvalidDepthLimit),
            NativePath::BudgetRejected(SchemaEvaluationError::ZeroWork),
            NativePath::BudgetRejected(SchemaEvaluationError::CostOverflow),
        ] {
            assert_eq!(
                prefix_counts(&context, &scalar, facts(), population(path)),
                Err(Refusal::UnsupportedNativePath)
            );
        }
        let mut missing = facts();
        assert_eq!(missing.original_impressions.take(), Some(3));
        assert_eq!(
            prefix_counts(&context, &scalar, missing, outer_only),
            Err(Refusal::MissingCallerFact(CallerFact::OriginalImpressions))
        );
        let mut missing_cap = facts();
        assert_eq!(missing_cap.configured_bidder_cap.take(), Some(2));
        assert_eq!(
            prefix_counts(&context, &scalar, missing_cap, outer_only),
            Err(Refusal::MissingCallerFact(CallerFact::BidderCap))
        );
        assert_caller_source_binding(&binding, sources);
    }

    fn assert_guarded_alternatives(recipe: &Recipe) {
        use operations::model::{Arm, Atom, Kind, Pattern};
        let mut value = recipe.functions[&recipe.roots[0]].body.clone();
        value.kind = Kind::Value(Atom::Place);
        let alternatives = Pattern::Or(vec![Pattern::Wild, Pattern::Wild]);
        for pattern in [
            alternatives.clone(),
            Pattern::Tuple(vec![alternatives.clone()]),
            Pattern::Variant("fixture".to_owned(), vec![alternatives]),
        ] {
            let mut decision = value.clone();
            decision.kind = Kind::Choice {
                selector: Box::new(value.clone()),
                arms: vec![Arm {
                    pattern,
                    guard: Some(value.clone()),
                    body: value.clone(),
                }],
            };
            assert_eq!(
                count_operation(recipe, &decision, 4, 1, &mut BTreeSet::new()),
                Err(Refusal::UnsupportedContext)
            );
        }
        let mut decision = value.clone();
        decision.kind = Kind::Choice {
            selector: Box::new(value.clone()),
            arms: vec![Arm {
                pattern: Pattern::Wild,
                guard: Some(value.clone()),
                body: value,
            }],
        };
        assert_eq!(
            count_operation(recipe, &decision, 4, 1, &mut BTreeSet::new()),
            Ok(empty_recipe_counts())
        );
    }

    fn search_body() -> syn::File {
        syn::parse_quote! {
            struct Fixture;
            struct FixtureSearchCache {
                __inner: __fixture_impl::__SearchCache,
            }
            impl Fixture {
                /// Initializes every generated pattern regex, including after a
                /// failed constructor. Returns false if any cached construction
                /// failed. Repeated calls do not reconstruct or retry regexes.
                /// This does not initialize custom formats or bound search caches.
                pub fn initialize_regexes() -> bool {
                    __fixture_impl::initialize_regexes()
                }
                pub fn new_search_cache() -> Option<FixtureSearchCache> {
                    __fixture_impl::new_search_cache()
                        .map(|__inner| FixtureSearchCache { __inner })
                }
                pub fn is_valid(
                    input: &regex_automata::Input<'_>,
                    __cache: &mut FixtureSearchCache,
                ) -> bool {
                    { __fixture_impl::entry_is_valid(input, &mut __cache.__inner) }
                }
            }
            mod __fixture_impl {
                static PATTERN_REGEX: std::sync::LazyLock<Option<regex_automata::meta::Regex>> =
                    std::sync::LazyLock::new(|| regex_automata::meta::Regex::new("a").ok());

                pub(super) struct __SearchCache {
                    pattern_cache: regex_automata::meta::Cache,
                }
                impl __SearchCache {
                    fn reset(&mut self) {}
                }
                pub(super) fn initialize_regexes() -> bool {
                    PATTERN_REGEX.as_ref().is_some()
                }
                pub(super) fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache {
                        pattern_cache: PATTERN_REGEX.as_ref()?.create_cache(),
                    })
                }
                pub(super) fn entry_is_valid(
                    input: &regex_automata::Input<'_>,
                    cache: &mut __SearchCache,
                ) -> bool {
                    cache.reset();
                    PATTERN_REGEX.as_ref().is_some_and(|pattern| {
                        pattern.search_half_with(&mut cache.pattern_cache, input).is_some()
                            && pattern.search_half_with(&mut cache.pattern_cache, input).is_some()
                    })
                }
            }
        }
    }

    fn three_searches(syntax: &syn::File) -> syn::File {
        let changed_bytes = edited(
            syntax.to_token_stream().to_string().as_bytes(),
            "entry_is_valid",
            &mut |body| {
                let before: syn::Block = syn::parse_quote!({
                    cache.reset();
                    PATTERN_REGEX.as_ref().is_some_and(|pattern| {
                        pattern
                            .search_half_with(&mut cache.pattern_cache, input)
                            .is_some()
                            && pattern
                                .search_half_with(&mut cache.pattern_cache, input)
                                .is_some()
                    })
                });
                assert_eq!(
                    body.to_token_stream().to_string(),
                    before.to_token_stream().to_string()
                );
                *body = syn::parse_quote!({
                    cache.reset();
                    PATTERN_REGEX.as_ref().is_some_and(|pattern| {
                        pattern
                            .search_half_with(&mut cache.pattern_cache, input)
                            .is_some()
                            && pattern
                                .search_half_with(&mut cache.pattern_cache, input)
                                .is_some()
                            && pattern
                                .search_half_with(&mut cache.pattern_cache, input)
                                .is_some()
                    })
                });
            },
        );
        syn::parse_file(from_utf8(&changed_bytes).unwrap()).unwrap()
    }

    fn additional_cache(syntax: &syn::File) -> syn::File {
        let mut two_caches = syntax.clone();
        let module = two_caches
            .items
            .iter_mut()
            .find_map(|item| {
                let syn::Item::Mod(module) = item else {
                    return None;
                };
                (module.ident == "__fixture_impl").then_some(module)
            })
            .unwrap();
        let (_, items) = module.content.as_mut().unwrap();
        items.push(syn::parse_quote! {
            static OTHER_REGEX: std::sync::LazyLock<Option<regex_automata::meta::Regex>> =
                std::sync::LazyLock::new(|| regex_automata::meta::Regex::new("b").ok());
        });
        let extra: syn::ItemStruct = syn::parse_quote! {
            struct ExtraCacheField { other_cache: regex_automata::meta::Cache }
        };
        let mut changed_fields = 0_usize;
        let mut changed_initializers = 0_usize;
        let mut changed_constructors = 0_usize;
        for item in items {
            if let syn::Item::Struct(cache) = item {
                if cache.ident == "__SearchCache" {
                    let syn::Fields::Named(fields) = &mut cache.fields else {
                        panic!("the cache fixture has named fields");
                    };
                    assert_eq!(fields.named.len(), 1);
                    fields.named.extend(extra.fields.iter().cloned());
                    changed_fields = changed_fields
                        .checked_add(1)
                        .expect("bounded fixture edit count");
                }
                continue;
            }
            let syn::Item::Fn(function) = item else {
                continue;
            };
            if function.sig.ident == "initialize_regexes" {
                *function.block = syn::parse_quote!({
                    PATTERN_REGEX.as_ref().is_some() && OTHER_REGEX.as_ref().is_some()
                });
                changed_initializers = changed_initializers
                    .checked_add(1)
                    .expect("bounded fixture edit count");
                continue;
            }
            if function.sig.ident == "new_search_cache" {
                *function.block = syn::parse_quote!({
                    if !initialize_regexes() {
                        return None;
                    }
                    Some(__SearchCache {
                        pattern_cache: PATTERN_REGEX.as_ref()?.create_cache(),
                        other_cache: OTHER_REGEX.as_ref()?.create_cache(),
                    })
                });
                changed_constructors = changed_constructors
                    .checked_add(1)
                    .expect("bounded fixture edit count");
            }
        }
        assert_eq!(
            (changed_fields, changed_initializers, changed_constructors),
            (1, 1, 1)
        );
        two_caches
    }

    fn assert_search_multiplicity(context: &AttemptContext<'_>, dimensions: SchemaInputStats) {
        let search_syntax = search_body();
        let search = classified(&search_syntax);
        assert_eq!(
            search
                .recipe()
                .roots
                .iter()
                .map(|id| search.recipe().functions[id].identity.as_str())
                .collect::<Vec<_>>(),
            vec!["Fixture::is_valid", "Fixture::new_search_cache"]
        );
        assert_eq!(
            search
                .recipe()
                .initialization
                .get("__fixture_impl::initialize_regexes"),
            Some(&vec!["__fixture_impl::PATTERN_REGEX".to_owned()])
        );
        let search_counts = successful(recipe_counts(context, search.recipe(), dimensions, 5));
        // Five episodes enter both coupled roots: ten function-root entries.
        // Constructor attempts remain a distinct count, not additional searches.
        assert_eq!(search_counts.root_entries, 10);
        assert_eq!(search_counts.fresh_cache_calls, 5);
        assert_eq!(search_counts.searches, 10); // Not J=4, callbacks=6, or a flat site census of 2.
        assert!(search_counts.unresolved.contains(&Primitive::RegexSearch));
        assert!(
            search_counts
                .unresolved
                .contains(&Primitive::CacheConstruction)
        );
        assert!(search_counts.unresolved.contains(&Primitive::Cleanup));
        assert_eq!(search_counts.seed_invocations, None);
        let changed_syntax = three_searches(&search_syntax);
        let changed = classified(&changed_syntax);
        assert_eq!(changed.recipe().roots, search.recipe().roots);
        assert_eq!(
            changed.recipe().initialization,
            search.recipe().initialization
        );
        let changed_counts = successful(recipe_counts(context, changed.recipe(), dimensions, 5));
        assert_eq!(changed_counts.root_entries, 10);
        assert_eq!(changed_counts.fresh_cache_calls, 5);
        assert_eq!(changed_counts.searches, 15);

        let two_caches = additional_cache(&changed_syntax);
        let extra_cache_operations = classified(&two_caches);
        let extra_cache_counts = successful(recipe_counts(
            context,
            extra_cache_operations.recipe(),
            dimensions,
            5,
        ));
        // The additional field is eager but never searched. Root/search
        // populations are unchanged; individual cache constructions double.
        assert_eq!(extra_cache_counts.root_entries, 10);
        assert_eq!(extra_cache_counts.searches, 15);
        assert_eq!(extra_cache_counts.fresh_cache_calls, 10);
        assert_eq!(extra_cache_counts.cache_drop_entries, 10);
    }

    #[test]
    fn branches_and_callbacks_have_source_derived_multiplicity() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            capture_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let syntax = callback_body();
        let operations = classified(&syntax);
        let dimensions = SchemaInputStats {
            serialized_bytes: 7,
            nodes: 4,
            max_depth: 1,
        };
        assert_eq!(
            measure_schema_input(&json!([1_i32, 2_i32, 3_i32]), context.native_input_limits),
            Ok(dimensions)
        );
        let counts = successful(recipe_counts(&context, operations.recipe(), dimensions, 1));
        assert_eq!(counts.root_entries, 1);
        assert_eq!(counts.helpers.get("is_branch_valid_0"), Some(&2));
        assert_eq!(counts.helpers.get("is_branch_valid_1"), Some(&6));
        assert_eq!(counts.iterator_callbacks, 6); // One callback site executes repeatedly.
        assert_eq!(
            (
                counts.searches,
                counts.fresh_cache_calls,
                counts.cache_drop_entries
            ),
            (0, 0, 0)
        );
        assert_eq!(counts.seed_invocations, None); // Opaque primitive closure is not seed-free proof.
        assert!(counts.unresolved.contains(&Primitive::IteratorProtocol));
        assert_guarded_alternatives(operations.recipe());
        assert_search_multiplicity(&context, dimensions);
    }

    #[test]
    fn late_failure_includes_cleanup() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            capture_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let (scalar, scalar_limits) = prefix::scalar_fixture::fixture();
        assert_eq!(scalar_limits, context.native_input_limits);
        for path in [
            NativePath::Returned { valid: false },
            NativePath::ValidatorPanicked,
        ] {
            let counts = successful(prefix_counts(&context, &scalar, facts(), population(path)));
            assert_eq!(counts.debit_events, 1);
            assert_eq!(counts.fresh_cache_calls, 1);
            assert_eq!(counts.cache_drop_entries, 1);
            assert_eq!(counts.seed_invocations, None); // Not the cache count, nor a free seed body.
            assert!(counts.unresolved.contains(&Primitive::CacheConstruction));
            assert!(counts.unresolved.contains(&Primitive::Cleanup));
        }
        let mut missing = facts();
        assert_eq!(missing.panic_unwinds.take(), Some(true));
        assert_eq!(
            prefix_counts(
                &context,
                &scalar,
                missing,
                population(NativePath::ValidatorPanicked)
            ),
            Err(Refusal::MissingCallerFact(CallerFact::PanicUnwind))
        );
        let operations = classified(&callback_body());
        assert!(reachable_duties(operations.recipe()).contains(&Primitive::IteratorProtocol));
        assert_eq!(
            require_source_leaf_cost(&context, operations.recipe(), Primitive::IteratorProtocol),
            Err(Refusal::MissingRuntimeLemma(Primitive::IteratorProtocol))
        );
        let primitive_free: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                match instance { serde_json::Value::String(_) => true, _ => false }
            }
        };
        let primitive_free_operations = classified(&primitive_free);
        assert!(
            !reachable_duties(primitive_free_operations.recipe())
                .contains(&Primitive::IteratorProtocol)
        );
        assert_eq!(
            require_source_leaf_cost(
                &context,
                primitive_free_operations.recipe(),
                Primitive::IteratorProtocol
            ),
            Err(Refusal::NoSourceLeafDuty(Primitive::IteratorProtocol))
        );
        let changed = edited(DISPATCH_GENERATOR, "native_dispatch", &mut |body| {
            *body = syn::parse_quote!({
                quote! {
                    fn validate_native(owner: &str, value: &serde_json::Value) -> Option<bool> { None }
                }
            });
        });
        let changed_sources = ContextSources {
            dispatch_generator: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::DispatchGenerator))
        );
    }

    #[test]
    fn overflow_refuses() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            capture_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let operations = classified(&callback_body());
        let dimensions = SchemaInputStats {
            serialized_bytes: 7,
            nodes: 4,
            max_depth: 1,
        };
        assert_eq!(
            recipe_counts(&context, operations.recipe(), dimensions, u64::MAX),
            Err(Refusal::Overflow)
        );
        let (scalar, scalar_limits) = prefix::scalar_fixture::fixture();
        assert_eq!(scalar_limits, context.native_input_limits);
        let mut caller = facts();
        assert_eq!(caller.original_impressions.replace(u64::MAX), Some(3));
        let many = Population {
            retain_visits: u64::MAX,
            native_attempts: u64::MAX,
            ..population(NativePath::Returned { valid: true })
        };
        assert_eq!(
            prefix_counts(&context, &scalar, caller, many),
            Err(Refusal::Overflow)
        );
    }
}

//! Hypothetical correspondence calculations, not generated-source proof.
//!
//! For each source component, the intended premise is V <= a + b*B + c*J.
//! Q = max(ceil(a/1024), ceil(b/16), ceil(c/128)) then bounds admitted work
//! by Q*sum(T), using a maximum across declarations of that same component.
//! Attempts P and external `N_clock*H` are disjoint from the admitted closure.
//! Q need not be one: it is not a CPU weight or an intrinsic token unit, and
//! establishes neither numerical policy adequacy nor latency qualification.
//! These interfaces neither implement a budget nor issue serving authority.
//! The parent owns module registration, hosted RED and the actual source join;
//! no fixture witness below is authority for an actual generated source.
//! Only the declared clock fixture is evaluated here. Other external services
//! remain unevaluated, not implicitly absent or included in that clock term.
//! Policy extraction recognizes the named work local only. Cost-use and actual
//! coordinator-graph correspondence still require the parent's source join.

use quote::ToTokens as _;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Component {
    ControlVisits,
    BytesScanned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CoefficientDimension {
    Constant,
    Bytes,
    Nodes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Obligation {
    Operations,
    Ownership,
    Shape,
    Lifetimes,
    Storage,
    StructuralStack,
    Attempts,
    ExternalClock,
    DisjointStages,
}

const OBLIGATIONS: [Obligation; 9] = [
    Obligation::Operations,
    Obligation::Ownership,
    Obligation::Shape,
    Obligation::Lifetimes,
    Obligation::Storage,
    Obligation::StructuralStack,
    Obligation::Attempts,
    Obligation::ExternalClock,
    Obligation::DisjointStages,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetFact {
    ClockWork,
    RequestedStorage,
    RetainedState,
    StackFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    MissingCommonProof(Obligation),
    MissingCoefficient(CoefficientDimension),
    UnwitnessedZero,
    PolicyTuple,
    ComponentMismatch,
    Overflow,
    MissingTargetFact(TargetFact),
    TargetBinding,
    HypotheticalFacts,
    NumericalPolicyNotEvaluated,
    RetiredCandidate,
    NonCanonicalField,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Coefficient {
    Derived { count: u64, witness: &'static str },
    ProvenAbsent { witness: &'static str },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AffineCoefficients {
    component: Component,
    a: Option<Coefficient>,
    b: Option<Coefficient>,
    c: Option<Coefficient>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PolicyTuple {
    constant: u64,
    bytes: u64,
    nodes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ComponentRatio {
    component: Component,
    source_units_per_debit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeclarationFixture {
    owner: Option<&'static str>,
    coefficients: AffineCoefficients,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommonFixture {
    premises: [Option<&'static str>; 9],
    declarations: Vec<DeclarationFixture>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumericalPolicyAdequacy {
    NotEvaluated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttemptFunding {
    NotEstablished,
}

// This is a conditional fixture result, not a common-source completion token.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ConditionalShape {
    component_maxima: Vec<ComponentRatio>,
    numeric_policy_adequacy: NumericalPolicyAdequacy,
    attempted_work_funding: AttemptFunding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceWork(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RequestedBytes(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameBytes(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct AttemptPopulation {
    attempts: u64,
    per_attempt: SourceWork,
    admitted_work: Vec<SourceWork>,
    admitted_debits: Vec<u64>,
    clock_invocations: u64,
    clock_binding: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalEnvelope {
    component: Component,
    attempts: SourceWork,
    admitted: SourceWork,
    local: SourceWork,
    admitted_policy_bound: SourceWork,
    local_policy_bound: SourceWork,
    attempted_work_funding: AttemptFunding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SuppliedFact<T> {
    Value(T),
    ProvenAbsent { witness: &'static str },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HypotheticalTargetFacts {
    clock_binding: &'static str,
    clock_work: Option<SuppliedFact<SourceWork>>,
    requested_storage: Option<SuppliedFact<RequestedBytes>>,
    retained_state: Option<SuppliedFact<RequestedBytes>>,
    stack_frame: Option<SuppliedFact<FrameBytes>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HypotheticalEvaluation {
    local: LocalEnvelope,
    external_work: SourceWork,
    total_work: SourceWork,
    total_policy_bound: SourceWork,
    requested_storage: RequestedBytes,
    retained_state: RequestedBytes,
    stack_frame: FrameBytes,
    numeric_policy_adequacy: NumericalPolicyAdequacy,
}

fn captured_policy_tuple() -> Result<PolicyTuple, Refusal> {
    let policy = policy_tuple_from_source(include_str!("../../src/schema/native.rs"))?;
    check_policy_tuple(policy)?;
    Ok(policy)
}

fn native_policy_block(function: &syn::ItemFn) -> Result<&syn::Block, Refusal> {
    let mut blocks = function.block.stmts.iter().filter_map(|statement| {
        let syn::Stmt::Expr(syn::Expr::Block(block), None) = statement else {
            return None;
        };
        let [attribute] = block.attrs.as_slice() else {
            return None;
        };
        if block.label.is_some() {
            return None;
        }
        let syn::Meta::List(cfg) = &attribute.meta else {
            return None;
        };
        if !cfg.path.is_ident("cfg") {
            return None;
        }
        let syn::Meta::List(not) = syn::parse2::<syn::Meta>(cfg.tokens.clone()).ok()? else {
            return None;
        };
        if !not.path.is_ident("not") {
            return None;
        }
        let syn::Meta::NameValue(target) = syn::parse2::<syn::Meta>(not.tokens).ok()? else {
            return None;
        };
        let syn::Expr::Lit(value) = target.value else {
            return None;
        };
        let syn::Lit::Str(architecture) = value.lit else {
            return None;
        };
        (target.path.is_ident("target_arch") && architecture.value() == "wasm32")
            .then_some(&block.block)
    });
    let block = blocks.next().ok_or(Refusal::Unsupported)?;
    if blocks.next().is_some() {
        return Err(Refusal::Unsupported);
    }
    Ok(block)
}

fn policy_work_local(block: &syn::Block) -> Result<&syn::Local, Refusal> {
    let mut locals = block.stmts.iter().filter_map(|statement| {
        let syn::Stmt::Local(local) = statement else {
            return None;
        };
        let pattern = if let syn::Pat::Type(typed) = &local.pat {
            typed.pat.as_ref()
        } else {
            &local.pat
        };
        matches!(pattern, syn::Pat::Ident(binding) if binding.ident == "work").then_some(local)
    });
    let local = locals.next().ok_or(Refusal::Unsupported)?;
    if locals.next().is_some() {
        return Err(Refusal::Unsupported);
    }
    Ok(local)
}

fn method_parts<'a>(
    expression: &'a syn::Expr,
    method: &str,
) -> Result<(&'a syn::Expr, &'a syn::Expr), Refusal> {
    let syn::Expr::MethodCall(call) = expression else {
        return Err(Refusal::Unsupported);
    };
    if call.method != method
        || call.turbofish.is_some()
        || !call.attrs.is_empty()
        || call.args.len() != 1
    {
        return Err(Refusal::Unsupported);
    }
    Ok((
        call.receiver.as_ref(),
        call.args.first().ok_or(Refusal::Unsupported)?,
    ))
}

fn closure_body(expression: &syn::Expr) -> Result<&syn::Expr, Refusal> {
    let syn::Expr::Closure(closure) = expression else {
        return Err(Refusal::Unsupported);
    };
    Ok(closure.body.as_ref())
}

fn policy_literal(expression: &syn::Expr) -> Result<&syn::LitInt, Refusal> {
    let syn::Expr::Lit(literal) = expression else {
        return Err(Refusal::Unsupported);
    };
    let syn::Lit::Int(integer) = &literal.lit else {
        return Err(Refusal::Unsupported);
    };
    if !literal.attrs.is_empty() || !integer.suffix().is_empty() {
        return Err(Refusal::Unsupported);
    }
    Ok(integer)
}

// Extract literals from the named function's native work local, then require
// the complete supported AST shape. Token printing compares parsed ASTs only;
// it never searches raw source text or substitutes another budget calculation.
fn policy_tuple_from_source(source: &str) -> Result<PolicyTuple, Refusal> {
    let syntax = syn::parse_file(source).map_err(|_error| Refusal::Unsupported)?;
    let mut functions = syntax.items.iter().filter_map(|item| {
        let syn::Item::Fn(function) = item else {
            return None;
        };
        (function.sig.ident == "validate_native_parameters").then_some(function)
    });
    let function = functions.next().ok_or(Refusal::Unsupported)?;
    if functions.next().is_some() {
        return Err(Refusal::Unsupported);
    }
    let work = policy_work_local(native_policy_block(function)?)?;
    let initializer = work.init.as_ref().ok_or(Refusal::Unsupported)?;
    let (byte_stage, constant_closure) = method_parts(&initializer.expr, "and_then")?;
    let (_, constant_expression) = method_parts(closure_body(constant_closure)?, "checked_add")?;
    let constant = policy_literal(constant_expression)?;
    let (byte_multiply, node_closure) = method_parts(byte_stage, "and_then")?;
    let syn::Expr::Block(node_block) = closure_body(node_closure)? else {
        return Err(Refusal::Unsupported);
    };
    let [syn::Stmt::Local(node_local), syn::Stmt::Expr(_, None)] =
        node_block.block.stmts.as_slice()
    else {
        return Err(Refusal::Unsupported);
    };
    let nodes_initializer = node_local.init.as_ref().ok_or(Refusal::Unsupported)?;
    let syn::Expr::Try(nodes_try) = nodes_initializer.expr.as_ref() else {
        return Err(Refusal::Unsupported);
    };
    let (_, node_expression) = method_parts(&nodes_try.expr, "checked_mul")?;
    let nodes = policy_literal(node_expression)?;
    let (_, byte_expression) = method_parts(byte_multiply, "checked_mul")?;
    let bytes = policy_literal(byte_expression)?;
    let expected: syn::Stmt = syn::parse_quote! {
        let work = stats
            .serialized_bytes
            .checked_mul(#bytes)
            .and_then(|bytes| {
                let nodes = stats.nodes.checked_mul(#nodes)?;
                bytes.checked_add(nodes)
            })
            .and_then(|units| units.checked_add(#constant));
    };
    if work.to_token_stream().to_string() != expected.to_token_stream().to_string() {
        return Err(Refusal::Unsupported);
    }
    Ok(PolicyTuple {
        constant: constant
            .base10_parse()
            .map_err(|_error| Refusal::Overflow)?,
        bytes: bytes.base10_parse().map_err(|_error| Refusal::Overflow)?,
        nodes: nodes.base10_parse().map_err(|_error| Refusal::Overflow)?,
    })
}

fn check_policy_tuple(actual: PolicyTuple) -> Result<(), Refusal> {
    if actual
        != (PolicyTuple {
            constant: 1_024,
            bytes: 16,
            nodes: 128,
        })
    {
        return Err(Refusal::PolicyTuple);
    }
    Ok(())
}

fn coefficient_value(
    coefficient: Option<Coefficient>,
    dimension: CoefficientDimension,
) -> Result<u64, Refusal> {
    match coefficient.ok_or(Refusal::MissingCoefficient(dimension))? {
        Coefficient::Derived { count: 0, .. } => Err(Refusal::UnwitnessedZero),
        Coefficient::Derived { count, witness } => {
            if witness.trim().is_empty() {
                return Err(Refusal::MissingCommonProof(Obligation::Shape));
            }
            Ok(count)
        }
        Coefficient::ProvenAbsent { witness } => {
            if witness.trim().is_empty() {
                return Err(Refusal::UnwitnessedZero);
            }
            Ok(0)
        }
    }
}

fn checked_ceiling(numerator: u64, denominator: u64) -> Result<u64, Refusal> {
    let quotient = numerator
        .checked_div(denominator)
        .ok_or(Refusal::PolicyTuple)?;
    let remainder = numerator
        .checked_rem(denominator)
        .ok_or(Refusal::PolicyTuple)?;
    quotient
        .checked_add(u64::from(remainder != 0))
        .ok_or(Refusal::Overflow)
}

fn component_ratio(
    coefficients: &AffineCoefficients,
    policy: PolicyTuple,
) -> Result<ComponentRatio, Refusal> {
    check_policy_tuple(policy)?;
    let a = coefficient_value(coefficients.a, CoefficientDimension::Constant)?;
    let b = coefficient_value(coefficients.b, CoefficientDimension::Bytes)?;
    let c = coefficient_value(coefficients.c, CoefficientDimension::Nodes)?;
    Ok(ComponentRatio {
        component: coefficients.component,
        source_units_per_debit: checked_ceiling(a, policy.constant)?
            .max(checked_ceiling(b, policy.bytes)?)
            .max(checked_ceiling(c, policy.nodes)?),
    })
}

fn declaration_maximum(ratios: &[ComponentRatio]) -> Result<ComponentRatio, Refusal> {
    let (first, rest) = ratios
        .split_first()
        .ok_or(Refusal::MissingCommonProof(Obligation::Ownership))?;
    let mut maximum = *first;
    for ratio in rest {
        if ratio.component != maximum.component {
            return Err(Refusal::ComponentMismatch);
        }
        maximum.source_units_per_debit = maximum
            .source_units_per_debit
            .max(ratio.source_units_per_debit);
    }
    Ok(maximum)
}

// Fixture premise presence is not source authority. This function has no
// actual-source input and can return only a hypothetical, non-qualifying shape.
fn check_common_fixture(fixture: &CommonFixture) -> Result<ConditionalShape, Refusal> {
    for (premise, obligation) in fixture.premises.iter().zip(OBLIGATIONS) {
        if premise.is_none_or(|witness| witness.trim().is_empty()) {
            return Err(Refusal::MissingCommonProof(obligation));
        }
    }
    if fixture.declarations.is_empty() {
        return Err(Refusal::MissingCommonProof(Obligation::Ownership));
    }
    let policy = captured_policy_tuple()?;
    let mut component_maxima: Vec<ComponentRatio> = Vec::new();
    for declaration in &fixture.declarations {
        if declaration
            .owner
            .is_none_or(|owner| owner.trim().is_empty())
        {
            return Err(Refusal::MissingCommonProof(Obligation::Ownership));
        }
        let ratio = component_ratio(&declaration.coefficients, policy)?;
        if let Some(maximum) = component_maxima
            .iter_mut()
            .find(|maximum| maximum.component == ratio.component)
        {
            *maximum = declaration_maximum(&[*maximum, ratio])?;
        } else {
            component_maxima.push(ratio);
        }
    }
    Ok(ConditionalShape {
        component_maxima,
        numeric_policy_adequacy: NumericalPolicyAdequacy::NotEvaluated,
        attempted_work_funding: AttemptFunding::NotEstablished,
    })
}

fn affine_bound(
    coefficients: &AffineCoefficients,
    bytes: u64,
    nodes: u64,
) -> Result<SourceWork, Refusal> {
    let a = coefficient_value(coefficients.a, CoefficientDimension::Constant)?;
    let b = coefficient_value(coefficients.b, CoefficientDimension::Bytes)?;
    let c = coefficient_value(coefficients.c, CoefficientDimension::Nodes)?;
    let byte_work = b.checked_mul(bytes).ok_or(Refusal::Overflow)?;
    let node_work = c.checked_mul(nodes).ok_or(Refusal::Overflow)?;
    a.checked_add(byte_work)
        .and_then(|work| work.checked_add(node_work))
        .map(SourceWork)
        .ok_or(Refusal::Overflow)
}

fn local_envelope(
    ratio: ComponentRatio,
    population: &AttemptPopulation,
) -> Result<LocalEnvelope, Refusal> {
    let admitted_count =
        u64::try_from(population.admitted_work.len()).map_err(|_error| Refusal::Overflow)?;
    if population.admitted_work.len() != population.admitted_debits.len()
        || admitted_count > population.attempts
    {
        return Err(Refusal::Unsupported);
    }
    let attempts = population
        .attempts
        .checked_mul(population.per_attempt.0)
        .ok_or(Refusal::Overflow)?;
    let admitted = population
        .admitted_work
        .iter()
        .try_fold(0_u64, |sum, work| {
            sum.checked_add(work.0).ok_or(Refusal::Overflow)
        })?;
    let debit_sum = population
        .admitted_debits
        .iter()
        .try_fold(0_u64, |sum, debit| {
            sum.checked_add(*debit).ok_or(Refusal::Overflow)
        })?;
    let admitted_policy_bound = ratio
        .source_units_per_debit
        .checked_mul(debit_sum)
        .ok_or(Refusal::Overflow)?;
    let local = attempts.checked_add(admitted).ok_or(Refusal::Overflow)?;
    let local_policy_bound = attempts
        .checked_add(admitted_policy_bound)
        .ok_or(Refusal::Overflow)?;
    if admitted > admitted_policy_bound {
        return Err(Refusal::Unsupported);
    }
    Ok(LocalEnvelope {
        component: ratio.component,
        attempts: SourceWork(attempts),
        admitted: SourceWork(admitted),
        local: SourceWork(local),
        admitted_policy_bound: SourceWork(admitted_policy_bound),
        local_policy_bound: SourceWork(local_policy_bound),
        attempted_work_funding: AttemptFunding::NotEstablished,
    })
}

fn supplied_fact<T: Copy + Eq>(
    fact: Option<SuppliedFact<T>>,
    required: TargetFact,
    zero: T,
) -> Result<T, Refusal> {
    match fact.ok_or(Refusal::MissingTargetFact(required))? {
        SuppliedFact::Value(value) if value != zero => Ok(value),
        SuppliedFact::ProvenAbsent { witness } if !witness.trim().is_empty() => Ok(zero),
        SuppliedFact::Value(_) | SuppliedFact::ProvenAbsent { .. } => Err(Refusal::UnwitnessedZero),
    }
}

fn evaluate_hypothetical(
    ratio: ComponentRatio,
    population: &AttemptPopulation,
    facts: &HypotheticalTargetFacts,
) -> Result<HypotheticalEvaluation, Refusal> {
    if facts.clock_binding.trim().is_empty() || facts.clock_binding != population.clock_binding {
        return Err(Refusal::TargetBinding);
    }
    // All facts remain required even for zero calls. An explicit absence is
    // limited to this supplied clock fixture; it says nothing about other hosts.
    let clock_work = supplied_fact(facts.clock_work, TargetFact::ClockWork, SourceWork(0))?;
    let requested_storage = supplied_fact(
        facts.requested_storage,
        TargetFact::RequestedStorage,
        RequestedBytes(0),
    )?;
    let retained_state = supplied_fact(
        facts.retained_state,
        TargetFact::RetainedState,
        RequestedBytes(0),
    )?;
    let stack_frame = supplied_fact(facts.stack_frame, TargetFact::StackFrame, FrameBytes(0))?;
    let local = local_envelope(ratio, population)?;
    let external_work = population
        .clock_invocations
        .checked_mul(clock_work.0)
        .ok_or(Refusal::Overflow)?;
    let total_work = local
        .local
        .0
        .checked_add(external_work)
        .ok_or(Refusal::Overflow)?;
    let total_policy_bound = local
        .local_policy_bound
        .0
        .checked_add(external_work)
        .ok_or(Refusal::Overflow)?;
    Ok(HypotheticalEvaluation {
        local,
        external_work: SourceWork(external_work),
        total_work: SourceWork(total_work),
        total_policy_bound: SourceWork(total_policy_bound),
        requested_storage,
        retained_state,
        stack_frame,
        numeric_policy_adequacy: NumericalPolicyAdequacy::NotEvaluated,
    })
}

// Only refusals are representable here: no qualified receipt or serving result.
fn qualification_refusal(
    _shape: &ConditionalShape,
    evaluation: Option<&HypotheticalEvaluation>,
) -> Refusal {
    match evaluation {
        Some(_) => Refusal::HypotheticalFacts,
        None => Refusal::NumericalPolicyNotEvaluated,
    }
}

// In-memory output field check only; this cannot read or return active costs.
fn check_current_report_fields(report: &Value) -> Result<(), Refusal> {
    let fields = report.as_object().ok_or(Refusal::NonCanonicalField)?;
    if fields.contains_key("native_cache_heap_candidate")
        || fields.get("kind").and_then(Value::as_str)
            == Some("conditional_native_schema_cache_heap_candidate")
    {
        return Err(Refusal::RetiredCandidate);
    }
    if fields.len() != 3
        || fields.get("kind").and_then(Value::as_str) != Some("conditional_source_shape")
        || fields
            .get("numeric_policy_adequacy")
            .and_then(Value::as_str)
            != Some("not_evaluated")
        || fields.get("attempted_work_funding").and_then(Value::as_str) != Some("not_established")
    {
        return Err(Refusal::NonCanonicalField);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use syn::visit_mut::{self, VisitMut};

    const ZERO: Option<Coefficient> = Some(Coefficient::ProvenAbsent {
        witness: "fake_explicit_source_absence",
    });

    fn derived(count: u64) -> Coefficient {
        Coefficient::Derived {
            count,
            witness: "fake_source_derivation",
        }
    }

    fn coefficients(
        a: Option<Coefficient>,
        b: Option<Coefficient>,
        c: Option<Coefficient>,
    ) -> AffineCoefficients {
        AffineCoefficients {
            component: Component::ControlVisits,
            a,
            b,
            c,
        }
    }

    fn policy() -> PolicyTuple {
        PolicyTuple {
            constant: 1_024,
            bytes: 16,
            nodes: 128,
        }
    }

    fn ratio(source_units_per_debit: u64) -> ComponentRatio {
        ComponentRatio {
            component: Component::ControlVisits,
            source_units_per_debit,
        }
    }

    fn common_fixture() -> CommonFixture {
        CommonFixture {
            premises: [
                Some("fake_operations"),
                Some("fake_ownership"),
                Some("fake_affine_shape"),
                Some("fake_lifetimes"),
                Some("fake_storage"),
                Some("fake_structural_stack"),
                Some("fake_attempt_envelope"),
                Some("fake_external_clock_boundary"),
                Some("fake_disjoint_attempt_admitted_external_stages"),
            ],
            declarations: vec![DeclarationFixture {
                owner: Some("fake_declaration"),
                coefficients: coefficients(Some(derived(3)), Some(derived(5)), Some(derived(7))),
            }],
        }
    }

    fn shape_fixture() -> ConditionalShape {
        ConditionalShape {
            component_maxima: vec![ratio(1)],
            numeric_policy_adequacy: NumericalPolicyAdequacy::NotEvaluated,
            attempted_work_funding: AttemptFunding::NotEstablished,
        }
    }

    fn population() -> AttemptPopulation {
        AttemptPopulation {
            attempts: 3,
            per_attempt: SourceWork(5),
            admitted_work: vec![SourceWork(11)],
            admitted_debits: vec![1_024],
            clock_invocations: 3,
            clock_binding: "fake_clock_source_and_target_identity",
        }
    }

    fn facts() -> HypotheticalTargetFacts {
        HypotheticalTargetFacts {
            clock_binding: "fake_clock_source_and_target_identity",
            clock_work: Some(SuppliedFact::Value(SourceWork(7))),
            requested_storage: Some(SuppliedFact::Value(RequestedBytes(17))),
            retained_state: Some(SuppliedFact::Value(RequestedBytes(19))),
            stack_frame: Some(SuppliedFact::Value(FrameBytes(23))),
        }
    }

    fn local_fixture() -> LocalEnvelope {
        LocalEnvelope {
            component: Component::ControlVisits,
            attempts: SourceWork(15),
            admitted: SourceWork(11),
            local: SourceWork(26),
            admitted_policy_bound: SourceWork(1_024),
            local_policy_bound: SourceWork(1_039),
            attempted_work_funding: AttemptFunding::NotEstablished,
        }
    }

    fn evaluation_fixture() -> HypotheticalEvaluation {
        HypotheticalEvaluation {
            local: local_fixture(),
            external_work: SourceWork(21),
            total_work: SourceWork(47),
            total_policy_bound: SourceWork(1_060),
            requested_storage: RequestedBytes(17),
            retained_state: RequestedBytes(19),
            stack_frame: FrameBytes(23),
            numeric_policy_adequacy: NumericalPolicyAdequacy::NotEvaluated,
        }
    }

    fn fixture_policy_function(source: &mut syn::File) -> &mut syn::ItemFn {
        source
            .items
            .iter_mut()
            .find_map(|item| {
                let syn::Item::Fn(function) = item else {
                    return None;
                };
                (function.sig.ident == "validate_native_parameters").then_some(function)
            })
            .expect("actual native entrypoint")
    }

    fn fixture_policy_block(source: &mut syn::File) -> &mut syn::ExprBlock {
        fixture_policy_function(source)
            .block
            .stmts
            .iter_mut()
            .find_map(|statement| {
                let syn::Stmt::Expr(syn::Expr::Block(block), None) = statement else {
                    return None;
                };
                Some(block)
            })
            .expect("first entrypoint block is the native fixture")
    }

    fn fixture_policy_work(source: &mut syn::File) -> &mut syn::Local {
        fixture_policy_block(source)
            .block
            .stmts
            .iter_mut()
            .find_map(|statement| {
                let syn::Stmt::Local(local) = statement else {
                    return None;
                };
                matches!(&local.pat, syn::Pat::Ident(binding) if binding.ident == "work")
                    .then_some(local)
            })
            .expect("actual named work local")
    }

    enum FormulaEdit {
        Literal(CoefficientDimension, syn::LitInt),
        Saturating(CoefficientDimension),
        SwapReceivers,
    }

    struct FormulaEditor {
        edit: FormulaEdit,
        changes: usize,
    }

    fn fixture_method_dimension(call: &syn::ExprMethodCall) -> Option<CoefficientDimension> {
        if call.method == "checked_add"
            && matches!(call.args.first(), Some(syn::Expr::Lit(value)) if matches!(value.lit, syn::Lit::Int(_)))
        {
            return Some(CoefficientDimension::Constant);
        }
        if call.method != "checked_mul" {
            return None;
        }
        let syn::Expr::Field(field) = call.receiver.as_ref() else {
            return None;
        };
        let syn::Member::Named(name) = &field.member else {
            return None;
        };
        if name == "serialized_bytes" {
            Some(CoefficientDimension::Bytes)
        } else if name == "nodes" {
            Some(CoefficientDimension::Nodes)
        } else {
            None
        }
    }

    impl VisitMut for FormulaEditor {
        fn visit_expr_method_call_mut(&mut self, i: &mut syn::ExprMethodCall) {
            match (&self.edit, fixture_method_dimension(i)) {
                (FormulaEdit::Literal(selected, replacement), Some(found))
                    if *selected == found =>
                {
                    *i.args.first_mut().expect("original literal argument") =
                        syn::parse_quote!(#replacement);
                    self.changes = self.changes.checked_add(1).expect("fixture edit count");
                }
                (FormulaEdit::Saturating(selected), Some(found)) if *selected == found => {
                    i.method = match found {
                        CoefficientDimension::Constant => syn::parse_quote!(saturating_add),
                        CoefficientDimension::Bytes | CoefficientDimension::Nodes => {
                            syn::parse_quote!(saturating_mul)
                        }
                    };
                    self.changes = self.changes.checked_add(1).expect("fixture edit count");
                }
                (FormulaEdit::SwapReceivers, Some(found)) => {
                    if let syn::Expr::Field(field) = i.receiver.as_mut() {
                        field.member = match found {
                            CoefficientDimension::Bytes => {
                                syn::Member::Named(syn::parse_quote!(nodes))
                            }
                            CoefficientDimension::Nodes => {
                                syn::Member::Named(syn::parse_quote!(serialized_bytes))
                            }
                            CoefficientDimension::Constant => return,
                        };
                        self.changes = self.changes.checked_add(1).expect("fixture edit count");
                    }
                }
                _ => {}
            }
            visit_mut::visit_expr_method_call_mut(self, i);
        }
    }

    fn policy_coefficient_source_controls() {
        let original = syn::parse_file(include_str!("../../src/schema/native.rs")).unwrap();
        assert_eq!(
            policy_tuple_from_source(&original.to_token_stream().to_string()),
            Ok(PolicyTuple {
                constant: 1_024,
                bytes: 16,
                nodes: 128
            })
        );
        for (dimension, replacement, expected) in [
            (
                CoefficientDimension::Constant,
                syn::parse_quote!(1_025),
                PolicyTuple {
                    constant: 1_025,
                    bytes: 16,
                    nodes: 128,
                },
            ),
            (
                CoefficientDimension::Bytes,
                syn::parse_quote!(17),
                PolicyTuple {
                    constant: 1_024,
                    bytes: 17,
                    nodes: 128,
                },
            ),
            (
                CoefficientDimension::Nodes,
                syn::parse_quote!(129),
                PolicyTuple {
                    constant: 1_024,
                    bytes: 16,
                    nodes: 129,
                },
            ),
        ] {
            let mut changed = original.clone();
            let mut editor = FormulaEditor {
                edit: FormulaEdit::Literal(dimension, replacement),
                changes: 0,
            };
            editor.visit_local_mut(fixture_policy_work(&mut changed));
            assert_eq!(editor.changes, 1);
            let extracted = policy_tuple_from_source(&changed.to_token_stream().to_string())
                .expect("changed coefficient must be extracted, not replaced by a fixed tuple");
            assert_eq!(extracted, expected);
            assert_eq!(check_policy_tuple(extracted), Err(Refusal::PolicyTuple));
        }
    }

    fn assert_unsupported_policy(source: &syn::File) {
        assert_eq!(
            policy_tuple_from_source(&source.to_token_stream().to_string()),
            Err(Refusal::Unsupported)
        );
    }

    fn policy_shape_source_controls() {
        let original = syn::parse_file(include_str!("../../src/schema/native.rs")).unwrap();
        assert_eq!(
            policy_tuple_from_source(&original.to_token_stream().to_string()),
            Ok(policy())
        );
        for (edit, changes) in [
            (FormulaEdit::Saturating(CoefficientDimension::Constant), 1),
            (FormulaEdit::Saturating(CoefficientDimension::Bytes), 1),
            (FormulaEdit::Saturating(CoefficientDimension::Nodes), 1),
            (FormulaEdit::SwapReceivers, 2),
        ] {
            let mut changed = original.clone();
            let mut editor = FormulaEditor { edit, changes: 0 };
            editor.visit_local_mut(fixture_policy_work(&mut changed));
            assert_eq!(editor.changes, changes);
            assert_unsupported_policy(&changed);
        }
        let mut addition = original.clone();
        let initializer = fixture_policy_work(&mut addition).init.as_mut().unwrap();
        let previous = &initializer.expr;
        *initializer.expr = syn::parse_quote!(
            #previous.and_then(|units| units.checked_add(1))
        );
        assert_unsupported_policy(&addition);

        let mut duplicate = original.clone();
        let statements = &mut fixture_policy_block(&mut duplicate).block.stmts;
        let index = statements
            .iter()
            .position(|statement| {
                matches!(statement, syn::Stmt::Local(local)
                if matches!(&local.pat, syn::Pat::Ident(binding) if binding.ident == "work"))
            })
            .unwrap();
        let work = statements[index].clone();
        statements.insert(index, work);
        assert_unsupported_policy(&duplicate);

        let mut cfg = original.clone();
        let native = fixture_policy_block(&mut cfg);
        assert_eq!(native.attrs.len(), 1);
        native.attrs[0] = syn::parse_quote!(#[cfg(target_arch = "wasm32")]);
        assert_unsupported_policy(&cfg);

        let mut name = original.clone();
        fixture_policy_function(&mut name).sig.ident = syn::parse_quote!(fake_policy_helper);
        assert_unsupported_policy(&name);

        let mut decoy = original;
        let mut helper = fixture_policy_function(&mut decoy).clone();
        helper.sig.ident = syn::parse_quote!(fake_formula_decoy);
        *fixture_policy_work(&mut decoy).init.as_mut().unwrap().expr = syn::parse_quote!(None);
        decoy.items.push(syn::Item::Fn(helper));
        assert_unsupported_policy(&decoy);
    }

    #[test]
    fn missing_common_proof_cannot_complete() {
        let original = common_fixture();
        assert_eq!(check_common_fixture(&original), Ok(shape_fixture()));
        for (index, obligation) in OBLIGATIONS.into_iter().enumerate() {
            let mut missing = original.clone();
            assert!(missing.premises[index].take().is_some());
            assert_eq!(
                check_common_fixture(&missing),
                Err(Refusal::MissingCommonProof(obligation))
            );
        }
        let mut ownerless = original.clone();
        assert!(ownerless.declarations[0].owner.take().is_some());
        assert_eq!(
            check_common_fixture(&ownerless),
            Err(Refusal::MissingCommonProof(Obligation::Ownership))
        );
        let mut empty = original.clone();
        assert_eq!(empty.declarations.len(), 1);
        empty.declarations.clear();
        assert_eq!(
            check_common_fixture(&empty),
            Err(Refusal::MissingCommonProof(Obligation::Ownership))
        );
        for axis in [
            CoefficientDimension::Constant,
            CoefficientDimension::Bytes,
            CoefficientDimension::Nodes,
        ] {
            let mut missing = original.clone();
            let work = &mut missing.declarations[0].coefficients;
            let coefficient = match axis {
                CoefficientDimension::Constant => &mut work.a,
                CoefficientDimension::Bytes => &mut work.b,
                CoefficientDimension::Nodes => &mut work.c,
            };
            assert!(coefficient.take().is_some());
            assert_eq!(
                check_common_fixture(&missing),
                Err(Refusal::MissingCoefficient(axis))
            );
        }
    }

    #[test]
    fn hypothetical_facts_cannot_qualify() {
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &facts()),
            Ok(evaluation_fixture())
        );
        assert_eq!(
            qualification_refusal(&shape_fixture(), Some(&evaluation_fixture())),
            Refusal::HypotheticalFacts
        );
    }

    #[test]
    fn component_ratios_are_not_cpu_weights() {
        for (work, expected) in [
            (coefficients(Some(derived(2_049)), ZERO, ZERO), 3),
            (coefficients(ZERO, Some(derived(33)), ZERO), 3),
            (coefficients(ZERO, ZERO, Some(derived(129))), 2),
        ] {
            assert_eq!(component_ratio(&work, policy()), Ok(ratio(expected)));
        }
        let mut byte_work = coefficients(ZERO, Some(derived(33)), ZERO);
        byte_work.component = Component::BytesScanned;
        assert_eq!(
            component_ratio(&byte_work, policy()),
            Ok(ComponentRatio {
                component: Component::BytesScanned,
                source_units_per_debit: 3,
            })
        );
        assert_eq!(
            affine_bound(
                &coefficients(Some(derived(3)), Some(derived(5)), Some(derived(7))),
                2,
                3,
            ),
            Ok(SourceWork(34))
        );
        assert_eq!(
            shape_fixture().numeric_policy_adequacy,
            NumericalPolicyAdequacy::NotEvaluated
        );
    }

    #[test]
    fn coefficient_boundaries_vary_independently() {
        for (work, expected) in [
            (coefficients(Some(derived(1_023)), ZERO, ZERO), 1),
            (coefficients(Some(derived(1_024)), ZERO, ZERO), 1),
            (coefficients(Some(derived(1_025)), ZERO, ZERO), 2),
            (coefficients(ZERO, Some(derived(15)), ZERO), 1),
            (coefficients(ZERO, Some(derived(16)), ZERO), 1),
            (coefficients(ZERO, Some(derived(17)), ZERO), 2),
            (coefficients(ZERO, ZERO, Some(derived(127))), 1),
            (coefficients(ZERO, ZERO, Some(derived(128))), 1),
            (coefficients(ZERO, ZERO, Some(derived(129))), 2),
            (
                coefficients(Some(derived(1_025)), Some(derived(33)), Some(derived(129))),
                3,
            ),
            (coefficients(ZERO, ZERO, ZERO), 0),
        ] {
            assert_eq!(component_ratio(&work, policy()), Ok(ratio(expected)));
        }
        for axis in [
            CoefficientDimension::Constant,
            CoefficientDimension::Bytes,
            CoefficientDimension::Nodes,
        ] {
            let mut missing = coefficients(ZERO, ZERO, ZERO);
            let coefficient = match axis {
                CoefficientDimension::Constant => &mut missing.a,
                CoefficientDimension::Bytes => &mut missing.b,
                CoefficientDimension::Nodes => &mut missing.c,
            };
            assert_eq!(coefficient.take(), ZERO);
            assert_eq!(
                component_ratio(&missing, policy()),
                Err(Refusal::MissingCoefficient(axis))
            );
            *match axis {
                CoefficientDimension::Constant => &mut missing.a,
                CoefficientDimension::Bytes => &mut missing.b,
                CoefficientDimension::Nodes => &mut missing.c,
            } = Some(derived(0));
            assert_eq!(
                component_ratio(&missing, policy()),
                Err(Refusal::UnwitnessedZero)
            );
        }
    }

    #[test]
    fn uncharged_attempts_survive_zero_debit() {
        let admitted = population();
        assert_eq!(local_envelope(ratio(1), &admitted), Ok(local_fixture()));
        let mut refused = admitted.clone();
        assert_eq!(refused.admitted_work, vec![SourceWork(11)]);
        assert_eq!(refused.admitted_debits, vec![1_024]);
        refused.admitted_work.clear();
        refused.admitted_debits.clear();
        for (attempts, expected) in [(0, 0), (1, 5), (3, 15)] {
            refused.attempts = attempts;
            assert_eq!(
                local_envelope(ratio(1), &refused),
                Ok(LocalEnvelope {
                    component: Component::ControlVisits,
                    attempts: SourceWork(expected),
                    admitted: SourceWork(0),
                    local: SourceWork(expected),
                    admitted_policy_bound: SourceWork(0),
                    local_policy_bound: SourceWork(expected),
                    attempted_work_funding: AttemptFunding::NotEstablished,
                })
            );
        }
        assert_eq!(
            evaluate_hypothetical(ratio(1), &admitted, &facts()),
            Ok(evaluation_fixture())
        );
        let mut no_clock_calls = admitted;
        assert_eq!(no_clock_calls.clock_invocations, 3);
        no_clock_calls.clock_invocations = 0;
        let mut expected = evaluation_fixture();
        expected.external_work = SourceWork(0);
        expected.total_work = SourceWork(26);
        expected.total_policy_bound = SourceWork(1_039);
        assert_eq!(
            evaluate_hypothetical(ratio(1), &no_clock_calls, &facts()),
            Ok(expected)
        );
    }

    #[test]
    fn shape_complete_cannot_qualify_policy() {
        assert_eq!(check_common_fixture(&common_fixture()), Ok(shape_fixture()));
        let shape = shape_fixture();
        assert_eq!(shape.component_maxima, vec![ratio(1)]);
        assert_eq!(
            shape.numeric_policy_adequacy,
            NumericalPolicyAdequacy::NotEvaluated
        );
        assert_eq!(shape.attempted_work_funding, AttemptFunding::NotEstablished);
        assert_eq!(
            qualification_refusal(&shape, None),
            Refusal::NumericalPolicyNotEvaluated
        );
    }

    #[test]
    fn missing_target_facts_cannot_be_zero() {
        let original = facts();
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &original),
            Ok(evaluation_fixture())
        );
        for fact in [
            TargetFact::ClockWork,
            TargetFact::RequestedStorage,
            TargetFact::RetainedState,
            TargetFact::StackFrame,
        ] {
            let mut missing = original.clone();
            match fact {
                TargetFact::ClockWork => assert!(missing.clock_work.take().is_some()),
                TargetFact::RequestedStorage => {
                    assert!(missing.requested_storage.take().is_some());
                }
                TargetFact::RetainedState => assert!(missing.retained_state.take().is_some()),
                TargetFact::StackFrame => assert!(missing.stack_frame.take().is_some()),
            }
            for calls in [0, 3] {
                let mut attempted = population();
                attempted.clock_invocations = calls;
                assert_eq!(
                    evaluate_hypothetical(ratio(1), &attempted, &missing),
                    Err(Refusal::MissingTargetFact(fact))
                );
            }
        }
        let mut stale = original.clone();
        assert_eq!(stale.clock_binding, population().clock_binding);
        stale.clock_binding = "fake_other_clock_identity";
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &stale),
            Err(Refusal::TargetBinding)
        );
        let zero = HypotheticalTargetFacts {
            clock_binding: original.clock_binding,
            clock_work: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_clock_work",
            }),
            requested_storage: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_storage",
            }),
            retained_state: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_retention",
            }),
            stack_frame: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_frame",
            }),
        };
        let mut expected = evaluation_fixture();
        expected.external_work = SourceWork(0);
        expected.total_work = SourceWork(26);
        expected.total_policy_bound = SourceWork(1_039);
        expected.requested_storage = RequestedBytes(0);
        expected.retained_state = RequestedBytes(0);
        expected.stack_frame = FrameBytes(0);
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &zero),
            Ok(expected)
        );
    }

    #[test]
    fn retired_candidate_cannot_supply_costs() {
        let current = json!({
            "kind": "conditional_source_shape",
            "numeric_policy_adequacy": "not_evaluated",
            "attempted_work_funding": "not_established"
        });
        assert_eq!(check_current_report_fields(&current), Ok(()));
        let retired = json!({
            "kind": "conditional_native_schema_cache_heap_candidate",
            "native_cache_heap_candidate": {"requested_bytes": 7_u64}
        });
        assert_eq!(
            check_current_report_fields(&retired),
            Err(Refusal::RetiredCandidate)
        );
        let mut hybrid = current.clone();
        assert!(hybrid.get("native_cache_heap_candidate").is_none());
        hybrid["native_cache_heap_candidate"] = json!({"requested_bytes": 7_u64});
        assert_eq!(
            check_current_report_fields(&hybrid),
            Err(Refusal::RetiredCandidate)
        );
        let mut fallback = current;
        assert!(fallback.get("heuristic_fallback").is_none());
        fallback["heuristic_fallback"] = json!({"work_units": 1_024_u64});
        assert_eq!(
            check_current_report_fields(&fallback),
            Err(Refusal::NonCanonicalField)
        );
    }

    #[test]
    fn existing_policy_tuple_is_checked_independently() {
        assert_eq!(
            captured_policy_tuple(),
            Ok(PolicyTuple {
                constant: 1_024,
                bytes: 16,
                nodes: 128,
            })
        );
        assert_eq!(check_policy_tuple(policy()), Ok(()));
        for changed in [
            PolicyTuple {
                constant: 1_023,
                ..policy()
            },
            PolicyTuple {
                constant: 1_025,
                ..policy()
            },
            PolicyTuple {
                bytes: 15,
                ..policy()
            },
            PolicyTuple {
                bytes: 17,
                ..policy()
            },
            PolicyTuple {
                nodes: 127,
                ..policy()
            },
            PolicyTuple {
                nodes: 129,
                ..policy()
            },
            PolicyTuple {
                constant: 0,
                ..policy()
            },
            PolicyTuple {
                bytes: 0,
                ..policy()
            },
            PolicyTuple {
                nodes: 0,
                ..policy()
            },
        ] {
            assert_ne!(changed, policy());
            assert_eq!(check_policy_tuple(changed), Err(Refusal::PolicyTuple));
            assert_eq!(
                component_ratio(
                    &coefficients(Some(derived(3)), Some(derived(5)), Some(derived(7))),
                    changed,
                ),
                Err(Refusal::PolicyTuple)
            );
        }
        policy_coefficient_source_controls();
        policy_shape_source_controls();
    }

    #[test]
    fn declaration_maxima_preserve_components() {
        assert_eq!(
            declaration_maximum(&[ratio(2), ratio(5), ratio(3)]),
            Ok(ratio(5))
        );
        assert_eq!(declaration_maximum(&[ratio(0), ratio(0)]), Ok(ratio(0)));
        assert_eq!(
            declaration_maximum(&[]),
            Err(Refusal::MissingCommonProof(Obligation::Ownership))
        );
        assert_eq!(
            declaration_maximum(&[
                ratio(3),
                ComponentRatio {
                    component: Component::BytesScanned,
                    source_units_per_debit: 7,
                },
            ]),
            Err(Refusal::ComponentMismatch)
        );
        let mut attempted = population();
        attempted.admitted_work = vec![SourceWork(11), SourceWork(13)];
        attempted.admitted_debits = vec![1_024, 2_048];
        assert_eq!(
            local_envelope(ratio(3), &attempted),
            Ok(LocalEnvelope {
                component: Component::ControlVisits,
                attempts: SourceWork(15),
                admitted: SourceWork(24),
                local: SourceWork(39),
                admitted_policy_bound: SourceWork(9_216),
                local_policy_bound: SourceWork(9_231),
                attempted_work_funding: AttemptFunding::NotEstablished,
            })
        );
    }

    #[test]
    fn checked_arithmetic_refuses_overflow() {
        // Ceiling division must not spuriously overflow at numerator + divisor - 1.
        assert_eq!(
            component_ratio(&coefficients(Some(derived(u64::MAX)), ZERO, ZERO), policy()),
            Ok(ratio(0x0040_0000_0000_0000))
        );
        for (work, bytes, nodes) in [
            (coefficients(ZERO, Some(derived(u64::MAX)), ZERO), 2, 0),
            (coefficients(ZERO, ZERO, Some(derived(u64::MAX))), 0, 2),
            (
                coefficients(Some(derived(u64::MAX)), Some(derived(1)), ZERO),
                1,
                0,
            ),
        ] {
            assert_eq!(affine_bound(&work, bytes, nodes), Err(Refusal::Overflow));
        }
        let mut attempts_overflow = population();
        attempts_overflow.attempts = u64::MAX;
        attempts_overflow.per_attempt = SourceWork(2);
        assert_eq!(
            local_envelope(ratio(1), &attempts_overflow),
            Err(Refusal::Overflow)
        );
        assert_eq!(
            local_envelope(ratio(u64::MAX), &population()),
            Err(Refusal::Overflow)
        );
        let mut debit_sum_overflow = population();
        debit_sum_overflow.admitted_debits = vec![u64::MAX, 1];
        debit_sum_overflow.admitted_work = vec![SourceWork(1), SourceWork(1)];
        assert_eq!(
            local_envelope(ratio(1), &debit_sum_overflow),
            Err(Refusal::Overflow)
        );
        let mut local_sum_overflow = population();
        local_sum_overflow.attempts = 1;
        local_sum_overflow.per_attempt = SourceWork(u64::MAX);
        assert_eq!(
            local_envelope(ratio(1), &local_sum_overflow),
            Err(Refusal::Overflow)
        );
        let mut clock_overflow = population();
        clock_overflow.clock_invocations = u64::MAX;
        assert_eq!(
            evaluate_hypothetical(ratio(1), &clock_overflow, &facts()),
            Err(Refusal::Overflow)
        );
    }
}

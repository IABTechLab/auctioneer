//! Owned source recipe shared by future work and stack consumers.
//! No numeric library lemma or common-completeness constructor lives here.
//! Entering a function body charges one source-entry event; entering a nested
//! lexical block charges one block event. Calls charge their call-site event
//! and revisit the callee's entry/body on every execution, not once per symbol.

use super::{BodyOperations, Primitive, Refusal, ServiceBinding, ServiceRequirement, WorkLedger};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Ty {
    Value,
    String,
    Number,
    Bool,
    Scalar,
    Map,
    Array,
    Regex,
    Input,
    Cache(String),
    LazyRegex(String),
    Option(Box<Self>),
    Iterator(Box<Self>),
    Tuple(Vec<Self>),
    Callback(String),
    Named(String),
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct Binding {
    pub ty: Ty,
    // A source place, not a parameter value: root + borrowed field/member path.
    pub place: String,
    pub borrowed: bool,
    pub origin: Origin,
    pub declaration: Option<Site>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Origin {
    Parameter { function: usize, ordinal: usize },
    Static(String),
    Temporary(Site),
    Projection { base: Box<Self>, member: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct Site {
    // A captured lexical anchor. Unresolved primitive source_calls reuse their
    // call-site anchor; they do not pretend their callee bodies were captured.
    pub function: usize,
    pub lexical: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct Function {
    pub identity: String,
    pub parameters: Vec<Binding>,
    pub result: Ty,
    pub body: Op,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct Recipe {
    pub roots: Vec<usize>,
    pub functions: BTreeMap<usize, Function>,
    // Requirements only: success must be joined to these same program owners.
    pub initialization: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Junction {
    And,
    Or,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Exit {
    Return(Site),
    Break(String),
    TryNone(Site),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct Op {
    pub site: Site,
    pub value: Binding,
    pub kind: Kind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Pattern {
    Wild,
    Binding(String),
    Literal(String),
    Variant(String, Vec<Self>),
    Tuple(Vec<Self>),
    Or(Vec<Self>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct Arm {
    pub pattern: Pattern,
    pub guard: Option<Op>,
    pub body: Op,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Atom {
    Place,
    Bool(bool),
    Scalar(String),
    String(String),
    None,
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Action {
    Borrow { mutable: bool },
    Field(String),
    Not,
    Negate,
    Dereference,
    Assign(Binding),
    AndAssign(Binding),
    CastScalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Comparison {
    TypeDiscriminant,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum StopRule {
    FirstFalseOrExhaustion,
    BreakOrExhaustion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct IterationBound {
    // One body invocation per visited member of this exact borrowed container;
    // absolute cardinality/byte caps are supplied by the admitted-input proof.
    pub collection: Binding,
    pub element: Ty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Kind {
    Value(Atom),
    Scope {
        label: Option<String>,
        steps: Vec<Op>,
        // Declaration order + source exits preserve cleanup/transfer premises.
        locals: Vec<Binding>,
    },
    Bind {
        pattern: Pattern,
        bindings: Vec<Binding>,
        value: Box<Op>,
    },
    Evaluate(Action, Vec<Op>),
    ScalarTest(Comparison, Vec<Op>),
    Choice {
        selector: Box<Op>,
        // Each arm includes its tested pattern/guard. These alternatives are
        // exclusive because they came from one actual match/if, not a label.
        arms: Vec<Arm>,
    },
    ShortCircuit(Junction, Vec<Op>),
    Call {
        target: usize,
        arguments: Vec<Binding>,
        evaluation: Vec<Op>,
    },
    Primitive {
        lemma: Primitive,
        callee: String,
        receiver: Option<Binding>,
        arguments: Vec<Op>,
        // Conditional source-chain obligations of a primitive, not argument
        // evaluations and not a claim its missing source lemma is discharged.
        source_calls: Vec<Op>,
    },
    Service(ServiceBinding),
    Iterate {
        protocol: Primitive,
        callee: String,
        stop: StopRule,
        // Exhaustion advances once to None; a short-circuit/break need not.
        none_on_exhaustion: bool,
        bound: IterationBound,
        receiver: Binding,
        // At most one callback is active, but its body may execute len times.
        // The library protocol's active call chain remains an unresolved lemma.
        parameters: Vec<Binding>,
        body: Box<Op>,
        evaluation: Box<Op>,
    },
    Callback {
        protocol: Primitive,
        callee: String,
        receiver: Binding,
        parameters: Vec<Binding>,
        body: Box<Op>,
        evaluation: Box<Op>,
    },
    Construct {
        fields: Vec<(String, Op)>,
        // Includes partial-prefix cleanup on a later initializer's TryNone.
        cleanup: Option<Primitive>,
    },
    Exit(Exit, Option<Box<Op>>),
}

impl Recipe {
    pub(super) fn project(self) -> Result<BodyOperations, Refusal> {
        let mut active = BTreeSet::new();
        for root in &self.roots {
            self.check_calls(*root, &mut active)?;
        }
        let mut calls = BTreeMap::new();
        let mut traversals = BTreeSet::new();
        let mut required_lemmas = BTreeSet::new();
        let mut services: Vec<ServiceRequirement> = Vec::new();
        for initializer in self.initialization.keys() {
            if !self
                .functions
                .values()
                .any(|function| function.identity == *initializer)
            {
                return Err(Refusal::UnclassifiedCall);
            }
        }
        for function in self.functions.values() {
            walk(&function.body, &mut |op| {
                match &op.kind {
                    Kind::Call { target, .. } => {
                        let callee = &self
                            .functions
                            .get(target)
                            .ok_or(Refusal::UnclassifiedCall)?
                            .identity;
                        let count = calls.entry(callee.clone()).or_insert(0_u64);
                        *count = count.checked_add(1).ok_or(Refusal::Overflow)?;
                    }
                    Kind::Iterate {
                        receiver, protocol, ..
                    } => {
                        required_lemmas.insert(*protocol);
                        traversals.insert(format!("{}::{}", function.identity, receiver.place));
                    }
                    Kind::Callback {
                        protocol,
                        parameters,
                        ..
                    } => {
                        required_lemmas.insert(*protocol);
                        if parameters.iter().any(needs_cleanup) {
                            required_lemmas.insert(Primitive::Cleanup);
                        }
                    }
                    Kind::Primitive { lemma, .. }
                    | Kind::Construct {
                        cleanup: Some(lemma),
                        ..
                    } => {
                        required_lemmas.insert(*lemma);
                    }
                    Kind::Scope { locals, .. } if locals.iter().any(needs_cleanup) => {
                        required_lemmas.insert(Primitive::Cleanup);
                    }
                    Kind::Service(binding) => {
                        if let Some(prior) =
                            services.iter_mut().find(|prior| prior.binding == *binding)
                        {
                            prior.invocations =
                                prior.invocations.checked_add(1).ok_or(Refusal::Overflow)?;
                        } else {
                            services.push(ServiceRequirement {
                                binding: binding.clone(),
                                invocations: 1,
                            });
                        }
                    }
                    Kind::Value(_)
                    | Kind::Scope { .. }
                    | Kind::Bind { .. }
                    | Kind::Evaluate(..)
                    | Kind::ScalarTest(..)
                    | Kind::Choice { .. }
                    | Kind::ShortCircuit(..)
                    | Kind::Construct { .. }
                    | Kind::Exit(..) => {}
                }
                Ok(())
            })?;
        }
        // Only primitive-free, traversal-free source fragments receive constant
        // event bounds here. Unknown library work never becomes a zero default.
        let ledger = self.root_events()?;
        Ok(BodyOperations {
            recipe: self,
            ledger,
            calls,
            traversals,
            required_lemmas,
            services,
        })
    }

    fn root_events(&self) -> Result<WorkLedger, Refusal> {
        let mut ledger = zero();
        for root in &self.roots {
            ledger = add(
                &ledger,
                &self.events(
                    &self
                        .functions
                        .get(root)
                        .ok_or(Refusal::UnclassifiedCall)?
                        .body,
                )?,
            )?;
        }
        Ok(ledger)
    }

    fn check_calls(&self, id: usize, active: &mut BTreeSet<usize>) -> Result<(), Refusal> {
        if !active.insert(id) {
            return Err(Refusal::MissingDecreasingMeasure);
        }
        let function = self.functions.get(&id).ok_or(Refusal::UnclassifiedCall)?;
        walk(&function.body, &mut |op| {
            if op.site.function != id || op.site.lexical.is_empty() {
                return Err(Refusal::UnsupportedSource);
            }
            self.check_origin(&op.value.origin)?;
            if op.value.declaration.as_ref().is_some_and(|declaration| {
                declaration.function != id || declaration.lexical.is_empty()
            }) {
                return Err(Refusal::UnsupportedSource);
            }
            if let Kind::Call {
                target,
                arguments,
                evaluation,
            } = &op.kind
            {
                let callee = self
                    .functions
                    .get(target)
                    .ok_or(Refusal::UnclassifiedCall)?;
                if arguments.len() != callee.parameters.len()
                    || arguments.len() != evaluation.len()
                    || arguments
                        .iter()
                        .zip(evaluation)
                        .any(|(binding, expression)| *binding != expression.value)
                    || arguments
                        .iter()
                        .zip(&callee.parameters)
                        .any(|(actual, expected)| actual.ty != expected.ty)
                    || op.value.ty != callee.result
                {
                    return Err(Refusal::ReceiverMismatch);
                }
                self.check_calls(*target, active)?;
            }
            check_shape(op)?;
            Ok(())
        })?;
        active.remove(&id);
        Ok(())
    }

    fn check_origin(&self, origin: &Origin) -> Result<(), Refusal> {
        match origin {
            Origin::Parameter { function, ordinal } => {
                if self
                    .functions
                    .get(function)
                    .is_none_or(|callee| *ordinal >= callee.parameters.len())
                {
                    return Err(Refusal::ReceiverMismatch);
                }
            }
            Origin::Static(owner) => {
                if !self
                    .initialization
                    .values()
                    .any(|programs| programs.contains(owner))
                {
                    return Err(Refusal::ReceiverMismatch);
                }
            }
            Origin::Temporary(site) => {
                if !self.functions.contains_key(&site.function) || site.lexical.is_empty() {
                    return Err(Refusal::UnsupportedSource);
                }
            }
            Origin::Projection { base, member } => {
                if member.is_empty() {
                    return Err(Refusal::ReceiverMismatch);
                }
                self.check_origin(base)?;
            }
        }
        Ok(())
    }

    fn events(&self, op: &Op) -> Result<WorkLedger, Refusal> {
        let mut own = zero();
        own.source_visits = Some(1);
        let children = match &op.kind {
            Kind::Primitive { .. }
            | Kind::Service(_)
            | Kind::Iterate { .. }
            | Kind::Callback { .. }
            | Kind::Construct { .. } => return Ok(WorkLedger::default()),
            Kind::Value(_) => zero(),
            Kind::Scope { locals, .. } if locals.iter().any(needs_cleanup) => {
                return Ok(WorkLedger::default());
            }
            Kind::Scope { steps, .. } | Kind::Evaluate(_, steps) => self.sequence(steps)?,
            Kind::ScalarTest(_, steps) => {
                own.scalar_comparisons = Some(1);
                self.sequence(steps)?
            }
            Kind::Bind { value, .. } => self.events(value)?,
            Kind::Choice { selector, arms } => {
                // Decision protocols with guards or several variant tests
                // await their source lemma. No max-of-guards approximation.
                if arms.iter().any(|arm| arm.guard.is_some())
                    || arms.iter().any(|arm| match &arm.pattern {
                        Pattern::Or(_) => true,
                        Pattern::Literal(value) => value != "true" && value != "false",
                        Pattern::Variant(_, fields) | Pattern::Tuple(fields) => {
                            !fields.iter().all(irrefutable_payload)
                        }
                        Pattern::Wild | Pattern::Binding(_) => false,
                    })
                    || arms
                        .iter()
                        .filter(|arm| {
                            matches!(arm.pattern, Pattern::Variant(_, _) | Pattern::Or(_))
                        })
                        .count()
                        > 1
                {
                    return Ok(WorkLedger::default());
                }
                own.control_visits = Some(1);
                let mut alternatives = zero();
                let mut decisions = zero();
                for arm in arms {
                    let guard = arm
                        .guard
                        .as_ref()
                        .map_or(Ok(zero()), |guard| self.events(guard))?;
                    // Failed guards precede the chosen arm. Only bodies are
                    // exclusive. Guarded or-pattern protocol is unresolved.
                    if arm.guard.is_some() && matches!(arm.pattern, Pattern::Or(_)) {
                        return Ok(WorkLedger::default());
                    }
                    decisions = add(&decisions, &guard)?;
                    alternatives = maximum(&alternatives, &self.events(&arm.body)?);
                }
                add(&add(&self.events(selector)?, &decisions)?, &alternatives)?
            }
            Kind::ShortCircuit(_, operands) => {
                own.control_visits =
                    Some(u64::try_from(operands.len()).map_err(|_error| Refusal::Overflow)?);
                self.sequence(operands)?
            }
            Kind::Call {
                target, evaluation, ..
            } => {
                let function = self
                    .functions
                    .get(target)
                    .ok_or(Refusal::UnclassifiedCall)?;
                add(&self.sequence(evaluation)?, &self.events(&function.body)?)?
            }
            Kind::Exit(_, value) => value
                .as_ref()
                .map_or(Ok(zero()), |expression| self.events(expression))?,
        };
        add(&own, &children)
    }

    fn sequence(&self, steps: &[Op]) -> Result<WorkLedger, Refusal> {
        let mut total = zero();
        for step in steps {
            total = add(&total, &self.events(step)?)?;
            if matches!(step.kind, Kind::Exit(Exit::Return(_) | Exit::Break(_), _)) {
                break;
            }
        }
        Ok(total)
    }
}

fn check_shape(op: &Op) -> Result<(), Refusal> {
    match &op.kind {
        Kind::Value(atom) => check_atom(atom, &op.value)?,
        Kind::Scope { label, locals, .. } => {
            if label.as_ref().is_some_and(String::is_empty)
                || locals.iter().any(|local| local.place.is_empty())
            {
                return Err(Refusal::UnsupportedSource);
            }
        }
        Kind::Primitive {
            callee,
            receiver,
            arguments,
            ..
        } => {
            if callee.is_empty()
                || receiver.as_ref().is_some_and(|binding| {
                    arguments
                        .first()
                        .is_none_or(|argument| argument.value != *binding)
                })
            {
                return Err(Refusal::ReceiverMismatch);
            }
        }
        Kind::Iterate { .. } => check_iteration(op)?,
        Kind::Callback { .. } => check_callback(op)?,
        Kind::Evaluate(action, operands) => check_action(action, operands, &op.value)?,
        Kind::ScalarTest(comparison, operands) => {
            let arity = if *comparison == Comparison::TypeDiscriminant {
                1
            } else {
                2
            };
            if operands.len() != arity || op.value.ty != Ty::Bool {
                return Err(Refusal::ReceiverMismatch);
            }
        }
        Kind::Choice { arms, .. } => {
            if arms.is_empty() {
                return Err(Refusal::UnsupportedSource);
            }
            for arm in arms {
                check_pattern(&arm.pattern)?;
            }
        }
        Kind::ShortCircuit(junction, operands) => {
            if !matches!(junction, Junction::And | Junction::Or)
                || operands.len() < 2
                || operands.iter().any(|operand| operand.value.ty != Ty::Bool)
            {
                return Err(Refusal::ReceiverMismatch);
            }
        }
        Kind::Construct { fields, .. } => {
            let mut names = BTreeSet::new();
            for (name, _) in fields {
                if !names.insert(name) {
                    return Err(Refusal::UnsupportedSource);
                }
            }
        }
        Kind::Exit(Exit::Break(label), _) if label.contains("::") => {
            return Err(Refusal::UnsupportedSource);
        }
        Kind::Exit(Exit::Return(target) | Exit::TryNone(target), _)
            if target.function != op.site.function || target.lexical.is_empty() =>
        {
            return Err(Refusal::UnsupportedSource);
        }
        Kind::Bind {
            pattern, bindings, ..
        } => {
            check_pattern(pattern)?;
            if bindings.iter().any(|binding| binding.declaration.is_none()) {
                return Err(Refusal::UnsupportedSource);
            }
        }
        Kind::Call { .. } | Kind::Service(_) | Kind::Exit(_, _) => {}
    }
    Ok(())
}

fn check_atom(atom: &Atom, binding: &Binding) -> Result<(), Refusal> {
    match atom {
        Atom::Bool(_) if binding.ty == Ty::Bool => {}
        Atom::Scalar(value) if !value.is_empty() && binding.ty == Ty::Scalar => {}
        Atom::String(_) if binding.ty == Ty::String && binding.borrowed => {}
        Atom::None if matches!(binding.ty, Ty::Option(_)) => {}
        Atom::Unit if binding.ty == Ty::Unit => {}
        Atom::Place if !binding.place.is_empty() => {}
        Atom::Place
        | Atom::Bool(_)
        | Atom::Scalar(_)
        | Atom::String(_)
        | Atom::None
        | Atom::Unit => return Err(Refusal::ReceiverMismatch),
    }
    Ok(())
}

fn check_action(action: &Action, operands: &[Op], result: &Binding) -> Result<(), Refusal> {
    let valid = match action {
        Action::Borrow { .. } => operands.len() == 1 && result.borrowed,
        Action::Field(member) => !member.is_empty() && operands.len() == 1,
        Action::Not => operands.len() == 1 && result.ty == Ty::Bool,
        Action::Negate | Action::CastScalar => operands.len() == 1 && result.ty == Ty::Scalar,
        Action::Dereference => operands.first().is_some_and(|value| value.value.borrowed),
        Action::Assign(target) | Action::AndAssign(target) => {
            operands.len() == 2 && operands.first().is_some_and(|value| value.value == *target)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(Refusal::ReceiverMismatch)
    }
}

fn check_iteration(op: &Op) -> Result<(), Refusal> {
    let Kind::Iterate {
        protocol,
        callee,
        stop,
        none_on_exhaustion,
        bound,
        receiver,
        parameters,
        evaluation,
        body,
    } = &op.kind
    else {
        return Err(Refusal::ReceiverMismatch);
    };
    let expected = match stop {
        StopRule::FirstFalseOrExhaustion => "Iterator::all",
        StopRule::BreakOrExhaustion => "Iterator::next/for",
    };
    if *protocol != Primitive::IteratorProtocol
        || callee != expected
        || !none_on_exhaustion
        || receiver != &evaluation.value
        || !matches!(receiver.ty, Ty::Iterator(_))
        || !matches!(bound.collection.ty, Ty::Array | Ty::Map)
        || !bound.collection.borrowed
        || receiver.ty != Ty::Iterator(Box::new(bound.element.clone()))
        || parameters
            .iter()
            .any(|parameter| parameter.place.is_empty())
        || (*stop == StopRule::FirstFalseOrExhaustion && body.value.ty != Ty::Bool)
    {
        return Err(Refusal::ReceiverMismatch);
    }
    Ok(())
}

fn check_callback(op: &Op) -> Result<(), Refusal> {
    let Kind::Callback {
        protocol,
        callee,
        receiver,
        parameters,
        evaluation,
        body,
    } = &op.kind
    else {
        return Err(Refusal::ReceiverMismatch);
    };
    if *protocol != Primitive::ValueAccess
        || !matches!(callee.as_str(), "Option::map" | "Option::is_some_and")
        || receiver != &evaluation.value
        || !matches!(receiver.ty, Ty::Option(_))
        || parameters
            .iter()
            .any(|parameter| parameter.place.is_empty())
        || (callee == "Option::is_some_and" && body.value.ty != Ty::Bool)
    {
        return Err(Refusal::ReceiverMismatch);
    }
    Ok(())
}

// Only bindings/wildcards and their tuple products add no payload decision.
// Nested variants, literals and alternatives require a separate decision bound.
fn irrefutable_payload(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wild | Pattern::Binding(_) => true,
        Pattern::Tuple(fields) => fields.iter().all(irrefutable_payload),
        Pattern::Literal(_) | Pattern::Variant(_, _) | Pattern::Or(_) => false,
    }
}

fn check_pattern(pattern: &Pattern) -> Result<(), Refusal> {
    match pattern {
        Pattern::Binding(name) | Pattern::Literal(name) if name.is_empty() => {
            return Err(Refusal::UnsupportedSource);
        }
        Pattern::Variant(name, fields) => {
            if name.is_empty() {
                return Err(Refusal::UnsupportedSource);
            }
            for field in fields {
                check_pattern(field)?;
            }
        }
        Pattern::Tuple(fields) | Pattern::Or(fields) => {
            for field in fields {
                check_pattern(field)?;
            }
        }
        Pattern::Wild | Pattern::Binding(_) | Pattern::Literal(_) => {}
    }
    Ok(())
}

pub(in crate::schema_validation) fn walk(
    op: &Op,
    visit: &mut impl FnMut(&Op) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    visit(op)?;
    match &op.kind {
        Kind::Scope { steps, .. }
        | Kind::Evaluate(_, steps)
        | Kind::ScalarTest(_, steps)
        | Kind::ShortCircuit(_, steps) => {
            for step in steps {
                walk(step, visit)?;
            }
        }
        Kind::Bind { value, .. } | Kind::Exit(_, Some(value)) => walk(value, visit)?,
        Kind::Choice { selector, arms } => {
            walk(selector, visit)?;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    walk(guard, visit)?;
                }
                walk(&arm.body, visit)?;
            }
        }
        Kind::Call { evaluation, .. } => {
            for arg in evaluation {
                walk(arg, visit)?;
            }
        }
        Kind::Primitive {
            arguments,
            source_calls,
            ..
        } => {
            for arg in arguments.iter().chain(source_calls) {
                walk(arg, visit)?;
            }
        }
        Kind::Iterate {
            body, evaluation, ..
        }
        | Kind::Callback {
            body, evaluation, ..
        } => {
            walk(evaluation, visit)?;
            walk(body, visit)?;
        }
        Kind::Construct { fields, .. } => {
            for (_, value) in fields {
                walk(value, visit)?;
            }
        }
        Kind::Value(_) | Kind::Service(_) | Kind::Exit(_, None) => {}
    }
    Ok(())
}

pub(in crate::schema_validation) fn needs_cleanup(binding: &Binding) -> bool {
    !binding.borrowed && !matches!(binding.ty, Ty::Bool | Ty::Scalar | Ty::Unit)
}

fn zero() -> WorkLedger {
    WorkLedger {
        source_visits: Some(0),
        control_visits: Some(0),
        iterator_visits: Some(0),
        map_visits: Some(0),
        property_visits: Some(0),
        scalar_comparisons: Some(0),
        scanned_bytes: Some(0),
        copied_bytes: Some(0),
        equality_visits: Some(0),
        uniqueness_visits: Some(0),
        construction_visits: Some(0),
        relocated_bytes: Some(0),
        cleanup_visits: Some(0),
        search_transitions: Some(0),
    }
}

// Fieldwise arithmetic preserves distinct units and propagates missing lemmas.
macro_rules! fields {
    ($left:ident, $right:ident, $combine:expr) => {
        WorkLedger {
            source_visits: $combine($left.source_visits, $right.source_visits),
            control_visits: $combine($left.control_visits, $right.control_visits),
            iterator_visits: $combine($left.iterator_visits, $right.iterator_visits),
            map_visits: $combine($left.map_visits, $right.map_visits),
            property_visits: $combine($left.property_visits, $right.property_visits),
            scalar_comparisons: $combine($left.scalar_comparisons, $right.scalar_comparisons),
            scanned_bytes: $combine($left.scanned_bytes, $right.scanned_bytes),
            copied_bytes: $combine($left.copied_bytes, $right.copied_bytes),
            equality_visits: $combine($left.equality_visits, $right.equality_visits),
            uniqueness_visits: $combine($left.uniqueness_visits, $right.uniqueness_visits),
            construction_visits: $combine($left.construction_visits, $right.construction_visits),
            relocated_bytes: $combine($left.relocated_bytes, $right.relocated_bytes),
            cleanup_visits: $combine($left.cleanup_visits, $right.cleanup_visits),
            search_transitions: $combine($left.search_transitions, $right.search_transitions),
        }
    };
}

fn add(left: &WorkLedger, right: &WorkLedger) -> Result<WorkLedger, Refusal> {
    let mut overflow = false;
    let mut sum =
        |left_count: Option<u64>, right_count: Option<u64>| match (left_count, right_count) {
            (Some(left_value), Some(right_value)) => {
                let result = left_value.checked_add(right_value);
                overflow |= result.is_none();
                result
            }
            _ => None,
        };
    let result = fields!(left, right, sum);
    if overflow {
        Err(Refusal::Overflow)
    } else {
        Ok(result)
    }
}

fn maximum(left: &WorkLedger, right: &WorkLedger) -> WorkLedger {
    let max = |left_count: Option<u64>, right_count: Option<u64>| {
        left_count
            .zip(right_count)
            .map(|(left_value, right_value)| left_value.max(right_value))
    };
    fields!(left, right, max)
}

//! Actual cache source-structure contract, not a storage total or cleanup proof.
//! Field programs and the enclosing wrapper retain the existing `MetaCache` and
//! `BundleHeap` cost owners. No engine heap model or serving authority lives here.

use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::ValidationView;
use crate::schema_validation::{
    handoff::{self, CheckedBodies},
    operations::{
        self, BodyOperations, Primitive,
        model::{
            self, Action, Atom, Binding, Exit, Function, Kind, Op, Origin, Pattern, Recipe, Site,
            Ty,
        },
    },
};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct FieldInitializer {
    field: String,
    program: String,
    site: Site,
    completed_siblings: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CallbackOverlap {
    site: Site,
    helper: Primitive,
    wrapper: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct OwnerSchedule {
    wrapper: String,
    factory_root: usize,
    validation_root: usize,
    initializers: Vec<FieldInitializer>,
    callbacks: Vec<CallbackOverlap>,
}

impl OwnerSchedule {
    pub(in crate::schema_validation) fn roots(&self) -> (usize, usize) {
        (self.factory_root, self.validation_root)
    }

    pub(in crate::schema_validation) fn initializers(
        &self,
    ) -> impl Iterator<Item = (&str, &str, &Site, &[String])> + '_ {
        self.initializers.iter().map(|row| {
            (
                row.field.as_str(),
                row.program.as_str(),
                &row.site,
                row.completed_siblings.as_slice(),
            )
        })
    }

    pub(in crate::schema_validation) fn callbacks(
        &self,
    ) -> impl Iterator<Item = (&Site, Primitive, &str)> + '_ {
        self.callbacks
            .iter()
            .map(|row| (&row.site, row.helper, row.wrapper.as_str()))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct OwnerAnalysis {
    owner: String,
    marker: String,
    operations: BodyOperations,
    schedule: OwnerSchedule,
}

// Projections of the already classified C/I auxiliary rows, not capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeRoots {
    factory: Option<String>,
    validation: Option<String>,
    wrapper: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Refusal {
    NotDerived,
    Operations(operations::Refusal),
    InitializerOrder,
    Constructor,
    CacheEscape,
    CacheReset,
    CacheClone,
    CacheRebind,
    MissingRoot,
    CallbackOverlap,
}

fn native_roots(graph: &Value, owner: &str, marker: &str) -> NativeRoots {
    let auxiliary = graph.get("auxiliary");
    let method = |role: &str, name: &str| {
        let mut matching = auxiliary
            .and_then(|value| value.get("dispatch_edges"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|row| {
                row.get("from").and_then(Value::as_str) == Some("validate_native")
                    && row.get("to").and_then(Value::as_str) == Some(marker)
                    && row.get("role").and_then(Value::as_str) == Some(role)
            });
        let first = matching.next();
        (first.is_some() && matching.next().is_none()).then(|| format!("{marker}::{name}"))
    };
    let mut wrappers = auxiliary
        .and_then(|value| value.get("roots"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|row| {
            let details = row.get("details");
            row.get("kind").and_then(Value::as_str) == Some("inline-retention-row")
                && details
                    .and_then(|value| value.get("owner"))
                    .and_then(Value::as_str)
                    == Some(owner)
                && details
                    .and_then(|value| value.get("marker"))
                    .and_then(Value::as_str)
                    == Some(marker)
        });
    let first = wrappers.next();
    let wrapper = if wrappers.next().is_none() {
        first
            .and_then(|row| row.get("details"))
            .and_then(|details| details.get("cache"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    } else {
        None
    };
    NativeRoots {
        factory: method("validation-call-cache-factory", "new_search_cache"),
        validation: method("is-valid-with-call-cache", "is_valid"),
        wrapper,
    }
}

fn root_id(recipe: &Recipe, identity: Option<&str>) -> Result<usize, Refusal> {
    let name = identity.ok_or(Refusal::MissingRoot)?;
    let mut matching = recipe
        .functions
        .iter()
        .filter(|(_, function)| function.identity == name);
    let (id, _) = matching.next().ok_or(Refusal::MissingRoot)?;
    if matching.next().is_some() || recipe.roots.iter().filter(|root| *root == id).count() != 1 {
        return Err(Refusal::MissingRoot);
    }
    Ok(*id)
}

// Keep the shared traversal, adapting only its cache-specific diagnostic channel.
fn inspect(op: &Op, mut visit: impl FnMut(&Op) -> Result<(), Refusal>) -> Result<(), Refusal> {
    let mut result = Ok(());
    model::walk(op, &mut |node| {
        if result.is_ok() {
            result = visit(node);
        }
        Ok(())
    })
    .map_err(Refusal::Operations)?;
    result
}

fn cache_type(ty: &Ty) -> bool {
    match ty {
        Ty::Cache(_) => true,
        Ty::Option(inner) | Ty::Iterator(inner) => cache_type(inner),
        Ty::Tuple(fields) => fields.iter().any(cache_type),
        Ty::Value
        | Ty::String
        | Ty::Number
        | Ty::Bool
        | Ty::Scalar
        | Ty::Map
        | Ty::Array
        | Ty::Regex
        | Ty::Input
        | Ty::LazyRegex(_)
        | Ty::Callback(_)
        | Ty::Named(_)
        | Ty::Unit => false,
    }
}

fn parameter_origin(origin: &Origin) -> Option<(usize, usize)> {
    match origin {
        Origin::Parameter { function, ordinal } => Some((*function, *ordinal)),
        Origin::Projection { base, .. } => parameter_origin(base),
        Origin::Static(_) | Origin::Temporary(_) => None,
    }
}

fn cache_parameter(binding: &Binding, id: usize, function: &Function) -> bool {
    parameter_origin(&binding.origin).is_some_and(|(owner, ordinal)| {
        owner == id
            && function
                .parameters
                .get(ordinal)
                .is_some_and(|parameter| cache_type(&parameter.ty))
    })
}

fn single_expression(mut op: &Op) -> Result<&Op, Refusal> {
    while let Kind::Scope {
        label,
        steps,
        locals,
    } = &op.kind
    {
        let [expression] = steps.as_slice() else {
            return Err(Refusal::NotDerived);
        };
        if label.is_some() || !locals.is_empty() {
            return Err(Refusal::NotDerived);
        }
        op = expression;
    }
    Ok(op)
}

fn constructed_fields(op: &Op) -> Result<&[(String, Op)], Refusal> {
    let Kind::Construct {
        fields,
        cleanup: Some(Primitive::Cleanup),
    } = &op.kind
    else {
        return Err(Refusal::Constructor);
    };
    Ok(fields)
}

fn factory_link<'a>(
    recipe: &'a Recipe,
    id: usize,
    wrapper: &str,
) -> Result<(usize, &'a str), Refusal> {
    let factory = recipe.functions.get(&id).ok_or(Refusal::MissingRoot)?;
    if !factory.parameters.is_empty()
        || factory.result != Ty::Option(Box::new(Ty::Cache(wrapper.to_owned())))
    {
        return Err(Refusal::Constructor);
    }
    let mapped = single_expression(&factory.body)?;
    let Kind::Callback {
        protocol: Primitive::ValueAccess,
        callee,
        receiver,
        parameters,
        body,
        evaluation,
    } = &mapped.kind
    else {
        return Err(Refusal::Constructor);
    };
    let [parameter] = parameters.as_slice() else {
        return Err(Refusal::Constructor);
    };
    let Ty::Cache(inner) = &parameter.ty else {
        return Err(Refusal::Constructor);
    };
    let Kind::Call {
        target,
        arguments,
        evaluation: inputs,
    } = &evaluation.kind
    else {
        return Err(Refusal::Constructor);
    };
    let target_function = recipe.functions.get(target).ok_or(Refusal::Constructor)?;
    let (module, name) = target_function
        .identity
        .rsplit_once("::")
        .ok_or(Refusal::Constructor)?;
    if callee != "Option::map"
        || *receiver != evaluation.value
        || parameter.borrowed
        || !arguments.is_empty()
        || !inputs.is_empty()
        || !target_function.parameters.is_empty()
        || name != "new_search_cache"
        || *inner != format!("{module}::__SearchCache")
        || target_function.result != Ty::Option(Box::new(parameter.ty.clone()))
        || receiver.ty != target_function.result
        || mapped.value.ty != factory.result
        || parameter.origin
            != (Origin::Projection {
                base: Box::new(receiver.origin.clone()),
                member: "Some".to_owned(),
            })
    {
        return Err(Refusal::Constructor);
    }
    let transfer = single_expression(body)?;
    let [(field, value)] = constructed_fields(transfer)? else {
        return Err(Refusal::Constructor);
    };
    if transfer.value.ty != Ty::Cache(wrapper.to_owned())
        || field != "__inner"
        || value.value != *parameter
        || !matches!(&value.kind, Kind::Value(Atom::Place))
    {
        return Err(Refusal::Constructor);
    }
    Ok((*target, inner))
}

fn readiness_guard(
    recipe: &Recipe,
    factory: &Function,
    guard: &Op,
    identity: &str,
) -> Result<(), Refusal> {
    let Kind::Choice { selector, arms } = &guard.kind else {
        return Err(Refusal::Constructor);
    };
    let [yes, no] = arms.as_slice() else {
        return Err(Refusal::Constructor);
    };
    let Kind::Evaluate(Action::Not, operands) = &selector.kind else {
        return Err(Refusal::Constructor);
    };
    let [ready] = operands.as_slice() else {
        return Err(Refusal::Constructor);
    };
    let Kind::Call {
        target,
        arguments,
        evaluation,
    } = &ready.kind
    else {
        return Err(Refusal::Constructor);
    };
    let initializer = recipe.functions.get(target).ok_or(Refusal::Constructor)?;
    let failure = single_expression(&yes.body)?;
    let Kind::Exit(Exit::Return(exit), Some(value)) = &failure.kind else {
        return Err(Refusal::Constructor);
    };
    if initializer.identity != identity
        || initializer.result != Ty::Bool
        || !initializer.parameters.is_empty()
        || !arguments.is_empty()
        || !evaluation.is_empty()
        || yes.pattern != Pattern::Literal("true".to_owned())
        || no.pattern != Pattern::Literal("false".to_owned())
        || yes.guard.is_some()
        || no.guard.is_some()
        || *exit != factory.body.site
        || !matches!(&value.kind, Kind::Value(Atom::None))
        || !matches!(&no.body.kind, Kind::Value(Atom::Unit))
    {
        return Err(Refusal::Constructor);
    }
    Ok(())
}

fn initializer_program<'a>(value: &'a Op, exit: &Site) -> Result<&'a str, Refusal> {
    let Origin::Static(program) = &value.value.origin else {
        return Err(Refusal::Constructor);
    };
    let Kind::Primitive {
        lemma: Primitive::CacheConstruction,
        callee,
        receiver: Some(regex),
        arguments,
        source_calls,
    } = &value.kind
    else {
        return Err(Refusal::Constructor);
    };
    let [ready] = arguments.as_slice() else {
        return Err(Refusal::Constructor);
    };
    let Kind::Exit(Exit::TryNone(target), Some(optional)) = &ready.kind else {
        return Err(Refusal::Constructor);
    };
    let Kind::Primitive {
        lemma: Primitive::Readiness,
        callee: accessor,
        receiver: Some(lazy),
        arguments: inputs,
        source_calls: indirect,
    } = &optional.kind
    else {
        return Err(Refusal::Constructor);
    };
    let [source] = inputs.as_slice() else {
        return Err(Refusal::Constructor);
    };
    if value.value.ty != Ty::Cache("regex_automata::meta::Cache".to_owned())
        || value.value.borrowed
        || callee != "Regex::create_cache"
        || !source_calls.is_empty()
        || regex.ty != Ty::Regex
        || !regex.borrowed
        || *regex != ready.value
        || regex.origin != value.value.origin
        || *target != *exit
        || optional.value.ty != Ty::Option(Box::new(Ty::Regex))
        || optional.value.origin != value.value.origin
        || !optional.value.borrowed
        || lazy.ty != Ty::LazyRegex(program.clone())
        || lazy.origin != value.value.origin
        || !lazy.borrowed
        || accessor.as_str() != format!("{:?}::as_ref", lazy.ty)
        || !indirect.is_empty()
        || source.value != *lazy
        || !matches!(&source.kind, Kind::Value(Atom::Place))
    {
        return Err(Refusal::Constructor);
    }
    Ok(program)
}

fn field_schedule(
    recipe: &Recipe,
    id: usize,
    inner: &str,
) -> Result<Vec<FieldInitializer>, Refusal> {
    let factory = recipe.functions.get(&id).ok_or(Refusal::Constructor)?;
    let (module, _) = factory
        .identity
        .rsplit_once("::")
        .ok_or(Refusal::Constructor)?;
    let initializer = format!("{module}::initialize_regexes");
    let programs = recipe
        .initialization
        .get(&initializer)
        .ok_or(Refusal::Constructor)?;
    if recipe.initialization.len() != 1 {
        return Err(Refusal::Constructor);
    }
    let expected: BTreeSet<_> = programs.iter().map(String::as_str).collect();
    if expected.len() != programs.len() {
        return Err(Refusal::Constructor);
    }
    let Kind::Scope {
        label: None,
        steps,
        locals,
    } = &factory.body.kind
    else {
        return Err(Refusal::Constructor);
    };
    let [guard, success] = steps.as_slice() else {
        return Err(Refusal::Constructor);
    };
    if !locals.is_empty() {
        return Err(Refusal::Constructor);
    }
    readiness_guard(recipe, factory, guard, &initializer)?;
    let [(tag, cache)] = constructed_fields(success)? else {
        return Err(Refusal::Constructor);
    };
    if tag != "Some"
        || success.value.ty != factory.result
        || cache.value.ty != Ty::Cache(inner.to_owned())
    {
        return Err(Refusal::Constructor);
    }
    let fields = constructed_fields(cache)?;
    let mut seen_fields = BTreeSet::new();
    let mut seen_programs = BTreeSet::new();
    let mut completed = Vec::new();
    let mut initializers = Vec::new();
    for (ordinal, (field, value)) in fields.iter().enumerate() {
        let program = initializer_program(value, &factory.body.site)?;
        if !seen_fields.insert(field)
            || !seen_programs.insert(program)
            || !expected.contains(program)
        {
            return Err(Refusal::Constructor);
        }
        let mut lexical = cache.site.lexical.clone();
        lexical.push(ordinal);
        if value.site.function != id || value.site.lexical != lexical {
            return Err(Refusal::InitializerOrder);
        }
        initializers.push(FieldInitializer {
            field: field.clone(),
            program: program.to_owned(),
            site: value.site.clone(),
            completed_siblings: completed.clone(),
        });
        completed.push(field.clone());
    }
    if seen_programs != expected {
        return Err(Refusal::Constructor);
    }
    Ok(initializers)
}

fn validation_entry(
    recipe: &Recipe,
    id: usize,
    wrapper: &str,
    factory: usize,
    inner: &str,
) -> Result<(), Refusal> {
    let function = recipe.functions.get(&id).ok_or(Refusal::MissingRoot)?;
    let [instance, cache] = function.parameters.as_slice() else {
        return Err(Refusal::CallbackOverlap);
    };
    if function.result != Ty::Bool
        || instance.ty != Ty::Value
        || !instance.borrowed
        || cache.ty != Ty::Cache(wrapper.to_owned())
        || !cache.borrowed
    {
        return Err(Refusal::CallbackOverlap);
    }
    let entry = single_expression(&function.body)?;
    let Kind::Call {
        target,
        arguments,
        evaluation,
    } = &entry.kind
    else {
        return Err(Refusal::CallbackOverlap);
    };
    let [input, context] = arguments.as_slice() else {
        return Err(Refusal::CallbackOverlap);
    };
    let target_function = recipe
        .functions
        .get(target)
        .ok_or(Refusal::CallbackOverlap)?;
    let factory_function = recipe.functions.get(&factory).ok_or(Refusal::Constructor)?;
    let (module, _) = factory_function
        .identity
        .rsplit_once("::")
        .ok_or(Refusal::Constructor)?;
    if target_function.identity != format!("{module}::entry_is_valid")
        || target_function.result != Ty::Bool
        || input != instance
        || !context.borrowed
        || context.ty != Ty::Cache(inner.to_owned())
        || context.origin
            != (Origin::Projection {
                base: Box::new(cache.origin.clone()),
                member: "__inner".to_owned(),
            })
        || evaluation.len() != arguments.len()
        || evaluation
            .iter()
            .zip(arguments)
            .any(|(expression, binding)| expression.value != *binding)
    {
        return Err(Refusal::CallbackOverlap);
    }
    Ok(())
}

fn empty_reset(function: &Function) -> bool {
    let [receiver] = function.parameters.as_slice() else {
        return false;
    };
    let Ty::Cache(owner) = &receiver.ty else {
        return false;
    };
    matches!(&function.body.kind, Kind::Scope { label: None, steps, locals } if steps.is_empty() && locals.is_empty())
        && function.identity == format!("{owner}::reset")
        && function.result == Ty::Unit
        && receiver.borrowed
}

fn cache_call(
    recipe: &Recipe,
    id: usize,
    function: &Function,
    target: usize,
    arguments: &[Binding],
) -> Result<(), Refusal> {
    let called = recipe.functions.get(&target).ok_or(Refusal::NotDerived)?;
    if arguments.len() != called.parameters.len() {
        return Err(Refusal::CacheEscape);
    }
    if cache_type(&called.result) {
        return Err(Refusal::CacheRebind);
    }
    let has_cache_argument = arguments.iter().any(|binding| cache_type(&binding.ty));
    let method = called.identity.rsplit("::").next();
    if has_cache_argument && method == Some("clone") {
        return Err(Refusal::CacheClone);
    }
    // The selected emitter calls a source-empty reset. Accept its matched body,
    // not a reset name, and do not claim its entry/frame/work costs are zero.
    if has_cache_argument && matches!(method, Some("reset" | "clear")) && !empty_reset(called) {
        return Err(Refusal::CacheReset);
    }
    for (actual, parameter) in arguments.iter().zip(&called.parameters) {
        if (cache_type(&actual.ty) || cache_type(&parameter.ty))
            && (actual.ty != parameter.ty
                || !actual.borrowed
                || !parameter.borrowed
                || !cache_parameter(actual, id, function))
        {
            return Err(Refusal::CacheEscape);
        }
    }
    Ok(())
}

fn cache_effect(recipe: &Recipe, id: usize, function: &Function, op: &Op) -> Result<(), Refusal> {
    if let Kind::Exit(_, Some(value)) = &op.kind
        && cache_type(&value.value.ty)
        && !matches!(&value.kind, Kind::Value(Atom::None))
    {
        return Err(Refusal::CacheEscape);
    }
    if let Kind::Evaluate(Action::Assign(target) | Action::AndAssign(target), _) = &op.kind
        && (cache_type(&target.ty) || cache_parameter(target, id, function))
    {
        return Err(Refusal::CacheRebind);
    }
    if let Kind::Bind { bindings, .. } = &op.kind
        && bindings.iter().any(|binding| cache_type(&binding.ty))
    {
        return Err(Refusal::CacheRebind);
    }
    if let Kind::Scope { locals, .. } = &op.kind
        && locals
            .iter()
            .any(|binding| cache_type(&binding.ty) && !binding.borrowed)
    {
        return Err(Refusal::CacheRebind);
    }
    if let Kind::Construct { fields, .. } = &op.kind
        && fields.iter().any(|(_, value)| cache_type(&value.value.ty))
    {
        return Err(Refusal::CacheEscape);
    }
    if let Kind::Iterate {
        receiver,
        parameters,
        ..
    }
    | Kind::Callback {
        receiver,
        parameters,
        ..
    } = &op.kind
        && (cache_type(&receiver.ty)
            || parameters.iter().any(|parameter| cache_type(&parameter.ty)))
    {
        return Err(Refusal::NotDerived);
    }
    primitive_cache_effect(id, function, op)?;
    if let Kind::Call {
        target, arguments, ..
    } = &op.kind
    {
        cache_call(recipe, id, function, *target, arguments)?;
    }
    if cache_type(&op.value.ty)
        && !matches!(&op.kind, Kind::Value(Atom::None))
        && (!op.value.borrowed || !cache_parameter(&op.value, id, function))
    {
        return Err(Refusal::CacheEscape);
    }
    Ok(())
}

fn primitive_cache_effect(id: usize, function: &Function, op: &Op) -> Result<(), Refusal> {
    let Kind::Primitive {
        lemma,
        callee,
        receiver,
        arguments,
        ..
    } = &op.kind
    else {
        return Ok(());
    };
    let cache_input = receiver
        .as_ref()
        .is_some_and(|binding| cache_type(&binding.ty))
        || arguments
            .iter()
            .any(|argument| cache_type(&argument.value.ty));
    if cache_input {
        match callee.rsplit("::").next() {
            Some("reset" | "clear") => return Err(Refusal::CacheReset),
            Some("clone") => return Err(Refusal::CacheClone),
            Some(_) | None => {}
        }
        if *lemma != Primitive::RegexSearch
            || callee != "Regex::search_half_with"
            || arguments.iter().any(|argument| {
                cache_type(&argument.value.ty)
                    && (!argument.value.borrowed || !cache_parameter(&argument.value, id, function))
            })
        {
            return Err(Refusal::NotDerived);
        }
    }
    if *lemma == Primitive::CacheConstruction {
        return Err(Refusal::CacheRebind);
    }
    Ok(())
}

fn validation_functions(
    recipe: &Recipe,
    root: usize,
    wrapper: &str,
    factory: usize,
    inner: &str,
) -> Result<BTreeSet<usize>, Refusal> {
    validation_entry(recipe, root, wrapper, factory, inner)?;
    // A source-coverage set only: neither visits nor invocation counts are work
    // bounds. The classifier already owns call typing and cycle refusal.
    let mut visited = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let function = recipe.functions.get(&id).ok_or(Refusal::NotDerived)?;
        for parameter in &function.parameters {
            if cache_type(&parameter.ty)
                && (!parameter.borrowed
                    || !matches!(&parameter.ty, Ty::Cache(owner) if owner == wrapper || owner == inner))
            {
                return Err(Refusal::CacheEscape);
            }
        }
        inspect(&function.body, |op| {
            cache_effect(recipe, id, function, op)?;
            if let Kind::Call { target, .. } = &op.kind {
                pending.push(*target);
            }
            Ok(())
        })?;
    }
    Ok(visited)
}

fn callback_schedule(
    recipe: &Recipe,
    validation: &BTreeSet<usize>,
    wrapper: &str,
) -> Result<Vec<CallbackOverlap>, Refusal> {
    let mut callbacks = Vec::new();
    for (id, function) in &recipe.functions {
        inspect(&function.body, |op| {
            if let Kind::Primitive { lemma, .. } = &op.kind
                && matches!(lemma, Primitive::HostnameFormat | Primitive::UuidFormat)
            {
                if !validation.contains(id) || op.site.function != *id {
                    return Err(Refusal::CallbackOverlap);
                }
                callbacks.push(CallbackOverlap {
                    site: op.site.clone(),
                    helper: *lemma,
                    wrapper: wrapper.to_owned(),
                });
            }
            Ok(())
        })?;
    }
    Ok(callbacks)
}

// Source schedules retain cleanup obligations; none of these matches prices or
// discharges normal release, partial-prefix cleanup, library work or target frames.
fn derive_owner(recipe: &Recipe, roots: &NativeRoots) -> Result<OwnerSchedule, Refusal> {
    let factory_root = root_id(recipe, roots.factory.as_deref())?;
    let validation_root = root_id(recipe, roots.validation.as_deref())?;
    let wrapper = roots.wrapper.as_deref().ok_or(Refusal::CallbackOverlap)?;
    let (factory, inner) = factory_link(recipe, factory_root, wrapper)?;
    let initializers = field_schedule(recipe, factory, inner)?;
    let validation = validation_functions(recipe, validation_root, wrapper, factory, inner)?;
    let callbacks = callback_schedule(recipe, &validation, wrapper)?;
    Ok(OwnerSchedule {
        wrapper: wrapper.to_owned(),
        factory_root,
        validation_root,
        initializers,
        callbacks,
    })
}

// Reuses the owner recognizer on supplied DATA. This entry does not capture a
// source body, seal an owner, price cleanup or establish source completeness.
pub(in crate::schema_validation) fn owner_schedule_data(
    recipe: &Recipe,
    graph: &Value,
    owner: &str,
    marker: &str,
) -> Result<OwnerSchedule, Refusal> {
    derive_owner(recipe, &native_roots(graph, owner, marker))
}

pub(super) fn check_sources<'a>(
    seed: &'a CapturedSeed,
    view: ValidationView<'a>,
) -> Result<CheckedBodies<'a, Result<OwnerAnalysis, Refusal>>, handoff::Refusal> {
    handoff::checked_bodies(seed, view, |body| {
        let analysis = operations::classify_source_body(body.syntax, &[], &[])
            .map_err(Refusal::Operations)
            .and_then(|operations| {
                let roots = native_roots(view.graph(), body.owner, body.marker);
                let schedule = derive_owner(operations.recipe(), &roots)?;
                Ok(OwnerAnalysis {
                    owner: body.owner.to_owned(),
                    marker: body.marker.to_owned(),
                    operations,
                    schedule,
                })
            });
        Ok(analysis)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CallbackOverlap, FieldInitializer, NativeRoots, OwnerAnalysis, OwnerSchedule, Refusal,
        check_sources, derive_owner, native_roots,
    };
    use crate::schema_build::application::CapturedSeed;
    use crate::schema_residency::with_current_descriptor;
    use crate::schema_validation::{
        handoff,
        operations::{
            self, BodyOperations, Primitive,
            model::{self, Action, Atom, Binding, Exit, Kind, Op, Origin, Recipe, Site, Ty},
        },
    };
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn row_string<'a>(row: &'a Value, key: &str) -> &'a str {
        row.get(key)
            .and_then(Value::as_str)
            .expect("required descriptor string")
    }

    fn callbacks(recipe: &Recipe) -> Vec<(Site, Primitive)> {
        let mut result = Vec::new();
        for function in recipe.functions.values() {
            model::walk(&function.body, &mut |op| {
                if let Kind::Primitive { lemma, .. } = &op.kind
                    && matches!(lemma, Primitive::HostnameFormat | Primitive::UuidFormat)
                {
                    result.push((op.site.clone(), *lemma));
                }
                Ok(())
            })
            .expect("shared recipe traversal");
        }
        result
    }

    fn expected_initializers(
        recipe: &Recipe,
        factory_root: usize,
        declaration: &Value,
    ) -> Vec<FieldInitializer> {
        let mut factories = Vec::new();
        let public_factory = recipe.functions.get(&factory_root).expect("public factory");
        model::walk(&public_factory.body, &mut |op| {
            if let Kind::Call { target, .. } = &op.kind
                && recipe
                    .functions
                    .get(target)
                    .expect("classified call target")
                    .identity
                    .ends_with("::new_search_cache")
            {
                factories.push(*target);
            }
            Ok(())
        })
        .expect("public factory recipe");
        assert_eq!(factories.len(), 1);
        let factory_id = *factories.first().expect("single inner factory");
        let factory = recipe.functions.get(&factory_id).expect("inner factory");
        let Kind::Scope { steps, .. } = &factory.body.kind else {
            panic!("inner factory scope");
        };
        let Kind::Construct {
            fields: some_fields,
            ..
        } = &steps.last().unwrap().kind
        else {
            panic!("inner factory Some constructor");
        };
        assert_eq!(some_fields.len(), 1);
        let (tag, cache) = some_fields.first().expect("single Some field");
        assert_eq!(tag.as_str(), "Some");
        let Kind::Construct { fields, .. } = &cache.kind else {
            panic!("inner cache field constructor");
        };
        let expected: Vec<FieldInitializer> = fields
            .iter()
            .enumerate()
            .map(|(ordinal, (field, value))| {
                assert_eq!(
                    value.value.ty,
                    Ty::Cache("regex_automata::meta::Cache".into())
                );
                assert_eq!(value.site.function, factory_id);
                let Origin::Static(program) = &value.value.origin else {
                    panic!("initializer's complete static program identity");
                };
                FieldInitializer {
                    field: field.clone(),
                    program: program.clone(),
                    site: value.site.clone(),
                    completed_siblings: fields
                        .get(..ordinal)
                        .expect("preceding field prefix")
                        .iter()
                        .map(|row| row.0.clone())
                        .collect(),
                }
            })
            .collect();
        assert_initializer_programs(recipe, &factory.identity, declaration, &expected);
        expected
    }

    fn assert_initializer_programs(
        recipe: &Recipe,
        factory_identity: &str,
        declaration: &Value,
        initializers: &[FieldInitializer],
    ) {
        let programs: BTreeSet<_> = initializers.iter().map(|row| row.program.clone()).collect();
        assert_eq!(programs.len(), initializers.len());
        let module = factory_identity.rsplit_once("::").unwrap().0;
        let declared = declaration
            .get("programs")
            .and_then(Value::as_array)
            .expect("descriptor programs");
        let declared_programs: BTreeSet<_> = declared
            .iter()
            .map(|row| format!("{module}::{}", row_string(row, "symbol")))
            .collect();
        assert_eq!(declared_programs.len(), declared.len());
        assert_eq!(programs, declared_programs);
        let initialized: Vec<_> = recipe.initialization.values().flatten().cloned().collect();
        let initialized_programs: BTreeSet<_> = initialized.iter().cloned().collect();
        assert_eq!(initialized_programs.len(), initialized.len());
        assert_eq!(programs, initialized_programs);
    }

    fn assert_owner(analysis: &OwnerAnalysis, declaration: &Value, roots: &NativeRoots) {
        assert_eq!(analysis.owner, row_string(declaration, "owner"));
        assert_eq!(analysis.marker, row_string(declaration, "marker"));
        let recipe = analysis.operations.recipe();
        let schedule = &analysis.schedule;
        let wrapper = roots.wrapper.as_ref().expect("checked native wrapper");
        assert_eq!(&schedule.wrapper, wrapper);
        let root_id = |candidate: &Option<String>| {
            let name = candidate.as_deref().expect("checked native root");
            let mut matching = recipe
                .functions
                .iter()
                .filter(|(_, function)| function.identity == name);
            let (id, _) = matching.next().expect("exact recipe root");
            assert!(matching.next().is_none());
            assert!(recipe.roots.contains(id));
            *id
        };
        let factory_root = root_id(&roots.factory);
        let validation_root = root_id(&roots.validation);
        assert_eq!(schedule.factory_root, factory_root);
        assert_eq!(schedule.validation_root, validation_root);
        let initializers = expected_initializers(recipe, factory_root, declaration);
        assert_eq!(schedule.initializers.len(), initializers.len());
        for (actual, expected) in schedule.initializers.iter().zip(&initializers) {
            assert_eq!(actual.field, expected.field);
            assert_eq!(actual.program, expected.program);
            assert_eq!(actual.site, expected.site);
            assert_eq!(actual.completed_siblings, expected.completed_siblings);
        }
        let callbacks = callbacks(recipe)
            .into_iter()
            .map(|(site, helper)| CallbackOverlap {
                site,
                helper,
                wrapper: wrapper.clone(),
            })
            .collect::<Vec<_>>();
        assert_eq!(schedule.callbacks.len(), callbacks.len());
        for (actual, expected) in schedule.callbacks.iter().zip(&callbacks) {
            assert_eq!(actual.site, expected.site);
            assert_eq!(actual.helper, expected.helper);
        }
        for callback in &schedule.callbacks {
            assert_eq!(&callback.wrapper, wrapper);
        }
        assert_eq!(
            schedule,
            &OwnerSchedule {
                wrapper: wrapper.clone(),
                factory_root,
                validation_root,
                initializers,
                callbacks,
            }
        );
    }

    #[test]
    fn actual_cache_lifetimes_cover_discovered_owners() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let declarations = view.graph()["declarations"].as_array().unwrap();
            assert!(!declarations.is_empty());
            assert!(
                declarations
                    .iter()
                    .any(|row| !row["programs"].as_array().unwrap().is_empty())
            );
            assert!(
                declarations
                    .iter()
                    .any(|row| row["programs"].as_array().unwrap().is_empty())
            );
            for row in declarations {
                let roots = native_roots(
                    view.graph(),
                    row["owner"].as_str().unwrap(),
                    row["marker"].as_str().unwrap(),
                );
                assert!(roots.factory.is_some());
                assert!(roots.validation.is_some());
                assert!(roots.wrapper.is_some());
            }
            let check_result = check_sources(seed, view);
            assert!(
                check_result.is_ok(),
                "actual source must first pass the sealed handoff"
            );
            let checked = check_result.unwrap();
            assert_eq!(checked.recheck(view), Ok(()));
            assert_eq!(checked.analyses().len(), declarations.len());
            for result in checked.analyses() {
                if let Err(Refusal::Operations(reason)) = result {
                    panic!(
                        "actual body classification must precede lifetime derivation: {reason:?}"
                    );
                }
            }
            assert!(
                checked.analyses().iter().all(Result::is_ok),
                "actual cache source schedules must be derived"
            );
            let actual = checked
                .analyses()
                .iter()
                .map(|row| row.as_ref().unwrap().owner.as_str())
                .collect::<BTreeSet<_>>();
            let expected = declarations
                .iter()
                .map(|row| row["owner"].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected);
            for result in checked.analyses() {
                let analysis = result.as_ref().unwrap();
                let declaration = declarations
                    .iter()
                    .find(|row| row["owner"] == analysis.owner)
                    .unwrap();
                let roots = native_roots(view.graph(), &analysis.owner, &analysis.marker);
                assert_owner(analysis, declaration, &roots);
            }
            let mut changed = CapturedSeed {
                identity_sha256: seed.identity_sha256.clone(),
                manifest_bytes: seed.manifest_bytes.clone(),
                rust: seed.rust.clone(),
                build_root: seed.build_root.clone(),
                out_dir: seed.out_dir.clone(),
                snapshots: seed.snapshots.clone(),
            };
            changed.rust.push(b' ');
            assert_ne!(changed.rust, seed.rust);
            assert_eq!(
                check_sources(&changed, view).map(|_checked| ()),
                Err(handoff::Refusal::Binding)
            );
            assert_eq!(checked.recheck(view), Ok(()));
            Ok(())
        })
        .unwrap();
    }

    // Private emitter-shaped input only; it never enters the actual-source join.
    fn fixture(with_fields: bool) -> BodyOperations {
        let fields = if with_fields {
            quote::quote! {
                pattern_0_cache: __ra::meta::Cache,
                pattern_1_cache: __ra::meta::Cache,
            }
        } else {
            quote::quote! {}
        };
        let initializers = if with_fields {
            quote::quote! {
                pattern_0_cache: PATTERN_0_REGEX.as_ref()?.create_cache(),
                pattern_1_cache: PATTERN_1_REGEX.as_ref()?.create_cache(),
            }
        } else {
            quote::quote! {}
        };
        let programs = if with_fields {
            quote::quote! {
                static PATTERN_0_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| None);
                static PATTERN_1_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| None);
            }
        } else {
            quote::quote! {}
        };
        let syntax = syn::parse2(quote::quote! {
            struct FakeMarker;
            struct FakeMarkerSearchCache { __inner: __fakemarker_impl::__SearchCache }
            impl FakeMarker {
                fn is_valid(instance: &serde_json::Value, cache: &mut FakeMarkerSearchCache) -> bool {
                    { __fakemarker_impl::entry_is_valid(instance, &mut cache.__inner) }
                }
                fn new_search_cache() -> Option<FakeMarkerSearchCache> {
                    __fakemarker_impl::new_search_cache().map(|inner| FakeMarkerSearchCache { __inner: inner })
                }
                fn initialize_regexes() -> bool { __fakemarker_impl::initialize_regexes() }
            }
            mod __fakemarker_impl {
                use regex_automata as __ra;
                use std::sync::LazyLock as __Lazy;
                #programs
                struct __SearchCache { #fields }
                fn initialize_regexes() -> bool { true }
                fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache { #initializers })
                }
                fn entry_is_valid(instance: &serde_json::Value, cache: &mut __SearchCache) -> bool {
                    match instance {
                        serde_json::Value::String(value) => crate::schema::staging::formats::hostname(value),
                        _ => true,
                    }
                }
            }
        }).expect("private typed source input");
        let result = operations::classify_source_body(&syntax, &[], &[]);
        assert!(
            result.is_ok(),
            "private input must pass the shared classifier: {result:?}"
        );
        result.unwrap()
    }

    fn roots() -> NativeRoots {
        NativeRoots {
            factory: Some("FakeMarker::new_search_cache".into()),
            validation: Some("FakeMarker::is_valid".into()),
            wrapper: Some("FakeMarkerSearchCache".into()),
        }
    }

    fn fields_mut(recipe: &mut Recipe) -> &mut Vec<(String, Op)> {
        let factory = recipe
            .functions
            .values_mut()
            .find(|function| function.identity == "__fakemarker_impl::new_search_cache")
            .unwrap();
        let Kind::Scope { steps, .. } = &mut factory.body.kind else {
            panic!("private factory scope");
        };
        let Kind::Construct {
            fields: some_fields,
            ..
        } = &mut steps.last_mut().unwrap().kind
        else {
            panic!("private Some constructor");
        };
        assert_eq!(some_fields.len(), 1);
        let (tag, cache) = some_fields.first_mut().expect("single Some field");
        assert_eq!(tag.as_str(), "Some");
        let Kind::Construct { fields, .. } = &mut cache.kind else {
            panic!("private inner cache constructor");
        };
        fields
    }

    // A newly added fixture initializer gets fresh lexical anchors. Only the
    // reorder control deliberately keeps anchors inconsistent with field order.
    fn reanchor_initializer(op: &mut Op, slot: usize, ordinal: usize) {
        assert!(matches!(&op.value.origin, Origin::Static(_)));
        assert!(op.value.declaration.is_none());
        *op.site
            .lexical
            .get_mut(slot)
            .expect("initializer lexical slot") = ordinal;
        if let Kind::Primitive {
            arguments,
            source_calls,
            ..
        } = &mut op.kind
        {
            for child in arguments.iter_mut().chain(source_calls) {
                reanchor_initializer(child, slot, ordinal);
            }
        } else if let Kind::Exit(_, Some(value)) = &mut op.kind {
            reanchor_initializer(value, slot, ordinal);
        } else {
            assert!(
                matches!(&op.kind, Kind::Value(_)),
                "private cache initializer chain"
            );
        }
    }

    #[test]
    fn initializer_order_preserves_complete_sibling_prefixes() {
        let operations = fixture(true);
        let original = operations.recipe();
        let mut before = original.clone();
        let original_fields = fields_mut(&mut before);
        assert_eq!(
            original_fields
                .iter()
                .map(|row| row.0.as_str())
                .collect::<Vec<_>>(),
            ["pattern_0_cache", "pattern_1_cache"]
        );
        let anchors: Vec<Site> = original_fields
            .iter()
            .map(|row| row.1.site.clone())
            .collect();
        assert_ne!(anchors[0], anchors[1]);
        assert_eq!(
            derive_owner(original, &roots()).map(|schedule| schedule.initializers),
            Ok(vec![
                FieldInitializer {
                    field: "pattern_0_cache".into(),
                    program: "__fakemarker_impl::PATTERN_0_REGEX".into(),
                    site: anchors[0].clone(),
                    completed_siblings: vec![],
                },
                FieldInitializer {
                    field: "pattern_1_cache".into(),
                    program: "__fakemarker_impl::PATTERN_1_REGEX".into(),
                    site: anchors[1].clone(),
                    completed_siblings: vec!["pattern_0_cache".into()],
                },
            ])
        );
        let mut reordered = original.clone();
        fields_mut(&mut reordered).swap(0, 1);
        assert_eq!(fields_mut(&mut reordered)[0].1.site, anchors[1]);
        assert_eq!(
            derive_owner(&reordered, &roots()),
            Err(Refusal::InitializerOrder)
        );
        let mut extra = original.clone();
        let fields = fields_mut(&mut extra);
        let mut additional = fields[0].1.clone();
        let slot = additional.site.lexical.len().checked_sub(1).unwrap();
        reanchor_initializer(&mut additional, slot, fields.len());
        fields.push(("extra_cache".into(), additional));
        assert_eq!(fields.len(), 3);
        assert_eq!(derive_owner(&extra, &roots()), Err(Refusal::Constructor));
    }

    fn append_effect(recipe: &mut Recipe, effect: impl FnOnce(Op) -> Kind) {
        let (id, function) = recipe
            .functions
            .iter_mut()
            .find(|(_, function)| function.identity == "__fakemarker_impl::entry_is_valid")
            .unwrap();
        let parameter = function.parameters[1].clone();
        assert!(parameter.borrowed);
        let cache_type = parameter.ty.clone();
        let Kind::Scope { steps, .. } = &mut function.body.kind else {
            panic!("private validation scope");
        };
        let site = Site {
            function: *id,
            lexical: vec![0, steps.len()],
        };
        let place = Op {
            site: Site {
                function: *id,
                lexical: vec![0, steps.len(), 0],
            },
            value: parameter,
            kind: Kind::Value(Atom::Place),
        };
        let kind = effect(place);
        let ty = if matches!(&kind, Kind::Primitive { callee, .. } if callee == "clone") {
            cache_type
        } else {
            Ty::Unit
        };
        steps.push(Op {
            site: site.clone(),
            value: Binding {
                ty,
                place: String::new(),
                borrowed: false,
                origin: Origin::Temporary(site),
                declaration: None,
            },
            kind,
        });
    }

    #[test]
    fn cache_escape_reset_clone_and_rebind_are_refused() {
        let operations = fixture(true);
        let original = operations.recipe();
        assert!(
            derive_owner(original, &roots()).is_ok(),
            "private positive schedule"
        );
        let mut escape = original.clone();
        append_effect(&mut escape, |place| {
            Kind::Exit(
                Exit::Return(Site {
                    function: place.site.function,
                    lexical: vec![0],
                }),
                Some(Box::new(place)),
            )
        });
        assert_eq!(derive_owner(&escape, &roots()), Err(Refusal::CacheEscape));
        let mut reset = original.clone();
        append_effect(&mut reset, |place| Kind::Primitive {
            lemma: Primitive::CacheConstruction,
            callee: "reset".into(),
            receiver: Some(place.value.clone()),
            arguments: vec![place],
            source_calls: Vec::new(),
        });
        assert_eq!(derive_owner(&reset, &roots()), Err(Refusal::CacheReset));
        let mut cloned = original.clone();
        append_effect(&mut cloned, |place| Kind::Primitive {
            lemma: Primitive::CacheConstruction,
            callee: "clone".into(),
            receiver: Some(place.value.clone()),
            arguments: vec![place],
            source_calls: Vec::new(),
        });
        assert_eq!(derive_owner(&cloned, &roots()), Err(Refusal::CacheClone));
        let mut rebind = original.clone();
        append_effect(&mut rebind, |place| {
            Kind::Evaluate(
                Action::Assign(place.value.clone()),
                vec![place.clone(), place],
            )
        });
        assert_eq!(derive_owner(&rebind, &roots()), Err(Refusal::CacheRebind));
    }

    #[test]
    fn zero_cache_owner_still_requires_wrapper_and_callback_obligations() {
        let operations = fixture(false);
        let original = operations.recipe();
        let mut before = original.clone();
        assert!(fields_mut(&mut before).is_empty());
        let callback_sites = callbacks(original);
        assert_eq!(callback_sites.len(), 1);
        assert_eq!(callback_sites[0].1, Primitive::HostnameFormat);
        let root_id = |name: &str| {
            let (id, _) = original
                .functions
                .iter()
                .find(|(_, function)| function.identity == name)
                .unwrap();
            assert!(original.roots.contains(id));
            *id
        };
        assert_eq!(
            derive_owner(original, &roots()),
            Ok(OwnerSchedule {
                wrapper: "FakeMarkerSearchCache".into(),
                factory_root: root_id("FakeMarker::new_search_cache"),
                validation_root: root_id("FakeMarker::is_valid"),
                initializers: vec![],
                callbacks: vec![CallbackOverlap {
                    site: callback_sites[0].0.clone(),
                    helper: Primitive::HostnameFormat,
                    wrapper: "FakeMarkerSearchCache".into(),
                }],
            })
        );
        let mut missing_factory = roots();
        assert!(missing_factory.factory.take().is_some());
        assert_eq!(
            derive_owner(original, &missing_factory),
            Err(Refusal::MissingRoot)
        );
        let mut missing_validation = roots();
        assert!(missing_validation.validation.take().is_some());
        assert_eq!(
            derive_owner(original, &missing_validation),
            Err(Refusal::MissingRoot)
        );
        let mut missing_overlap = roots();
        assert!(missing_overlap.wrapper.take().is_some());
        assert_eq!(
            derive_owner(original, &missing_overlap),
            Err(Refusal::CallbackOverlap)
        );
    }
}

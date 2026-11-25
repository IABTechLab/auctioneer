//! Captured logical stack DATA only, not common completeness or target frames.
//! Helper/protocol/ordinary cleanup duties stay open, including partial failure
//! and unwind. Allocation-success does not erase those returning paths.

use super::FrameBytes;
use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::ValidationView;
use crate::schema_validation::{
    handoff::{self, CheckedBodies},
    operations::{
        self, BodyOperations, Primitive, ServiceBinding,
        model::{self, Kind, Op, Recipe, Site},
    },
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
enum FrameKey {
    Function(usize),
    Callback(Site),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrameSlot {
    key: FrameKey,
    bytes: Option<FrameBytes>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallEdge {
    site: Site,
    target: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DutyKind {
    Primitive(Primitive, String),
    Service(ServiceBinding),
    Cleanup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StackDuty {
    site: Site,
    kind: DutyKind,
}

// Every possible function/callback entry and opaque call is a separate path.
// Calls are the entry sites between adjacent frames; opaque duties retain
// active protocol boundaries but do not invent their missing library frames.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ActivePath {
    frames: Vec<FrameKey>,
    calls: Vec<Site>,
    duties: Vec<StackDuty>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StackShape {
    roots: Vec<usize>,
    frames: Vec<FrameSlot>,
    calls: Vec<CallEdge>,
    paths: Vec<ActivePath>,
    duties: Vec<StackDuty>,
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    MissingCallee(usize),
    Cycle(usize),
}

struct OwnerAnalysis {
    owner: String,
    marker: String,
    operations: BodyOperations,
    shape: Result<StackShape, Refusal>,
}

// The sole input is the checked body's canonical Recipe. These projections
// retain unresolved helper/unwind duties and cannot complete a source proof.
fn derive_shape(recipe: &Recipe) -> Result<StackShape, Refusal> {
    for root in &recipe.roots {
        if !recipe.functions.contains_key(root) {
            return Err(Refusal::MissingCallee(*root));
        }
    }
    let mut shape = StackShape {
        roots: recipe.roots.clone(),
        frames: recipe
            .functions
            .keys()
            .map(|id| FrameSlot {
                key: FrameKey::Function(*id),
                bytes: None,
            })
            .collect(),
        calls: Vec::new(),
        paths: Vec::new(),
        duties: Vec::new(),
    };
    let graph = inventory(recipe, &mut shape)?;
    check_cycles(recipe, &graph)?;
    shape.paths = activation_paths(recipe)?;
    Ok(shape)
}

fn children(op: &Op) -> Vec<&Op> {
    match &op.kind {
        Kind::Scope { steps, .. }
        | Kind::Evaluate(_, steps)
        | Kind::ScalarTest(_, steps)
        | Kind::ShortCircuit(_, steps) => steps.iter().collect(),
        Kind::Bind { value, .. } | Kind::Exit(_, Some(value)) => vec![value.as_ref()],
        Kind::Choice { selector, arms } => {
            let mut nodes = vec![selector.as_ref()];
            for arm in arms {
                nodes.extend(arm.guard.iter());
                nodes.push(&arm.body);
            }
            nodes
        }
        Kind::Call { evaluation, .. } => evaluation.iter().collect(),
        Kind::Primitive {
            arguments,
            source_calls,
            ..
        } => arguments.iter().chain(source_calls).collect(),
        Kind::Iterate {
            evaluation, body, ..
        }
        | Kind::Callback {
            evaluation, body, ..
        } => vec![evaluation.as_ref(), body.as_ref()],
        Kind::Construct { fields, .. } => fields.iter().map(|(_, value)| value).collect(),
        Kind::Value(_) | Kind::Service(_) | Kind::Exit(_, None) => Vec::new(),
    }
}

fn duty(op: &Op, kind: DutyKind) -> StackDuty {
    StackDuty {
        site: op.site.clone(),
        kind,
    }
}

fn local_duties(op: &Op) -> Vec<StackDuty> {
    let kinds = match &op.kind {
        Kind::Primitive { lemma, callee, .. } => vec![DutyKind::Primitive(*lemma, callee.clone())],
        Kind::Service(binding) => vec![DutyKind::Service(binding.clone())],
        Kind::Iterate {
            protocol,
            callee,
            parameters,
            ..
        }
        | Kind::Callback {
            protocol,
            callee,
            parameters,
            ..
        } => {
            let mut result = vec![DutyKind::Primitive(*protocol, callee.clone())];
            if parameters.iter().any(model::needs_cleanup) {
                result.push(DutyKind::Cleanup);
            }
            result
        }
        Kind::Scope { locals, .. } if locals.iter().any(model::needs_cleanup) => {
            vec![DutyKind::Cleanup]
        }
        Kind::Construct {
            cleanup: Some(_), ..
        } => vec![DutyKind::Cleanup],
        Kind::Value(_)
        | Kind::Scope { .. }
        | Kind::Bind { .. }
        | Kind::Evaluate(_, _)
        | Kind::ScalarTest(_, _)
        | Kind::Choice { .. }
        | Kind::ShortCircuit(_, _)
        | Kind::Call { .. }
        | Kind::Construct { cleanup: None, .. }
        | Kind::Exit(_, _) => Vec::new(),
    };
    kinds.into_iter().map(|kind| duty(op, kind)).collect()
}

fn inventory(
    recipe: &Recipe,
    shape: &mut StackShape,
) -> Result<BTreeMap<usize, Vec<usize>>, Refusal> {
    let mut graph = BTreeMap::new();
    for (id, function) in &recipe.functions {
        let mut targets = Vec::new();
        let mut pending = vec![&function.body];
        while let Some(op) = pending.pop() {
            if let Kind::Call { target, .. } = &op.kind {
                if !recipe.functions.contains_key(target) {
                    return Err(Refusal::MissingCallee(*target));
                }
                targets.push(*target);
                shape.calls.push(CallEdge {
                    site: op.site.clone(),
                    target: *target,
                });
            }
            if matches!(&op.kind, Kind::Callback { .. })
                || matches!(&op.kind, Kind::Iterate { callee, .. } if callee == "Iterator::all")
            {
                shape.frames.push(FrameSlot {
                    key: FrameKey::Callback(op.site.clone()),
                    bytes: None,
                });
            }
            shape.duties.extend(local_duties(op));
            pending.extend(children(op).into_iter().rev());
        }
        graph.insert(*id, targets);
    }
    Ok(graph)
}

enum GraphVisit {
    Open(usize),
    Close(usize),
}

fn check_cycles(recipe: &Recipe, graph: &BTreeMap<usize, Vec<usize>>) -> Result<(), Refusal> {
    let mut active = BTreeSet::new();
    let mut finished = BTreeSet::new();
    for root in recipe.roots.iter().chain(recipe.functions.keys()) {
        let mut pending = vec![GraphVisit::Open(*root)];
        while let Some(visit) = pending.pop() {
            match visit {
                GraphVisit::Open(id) => {
                    if finished.contains(&id) {
                        continue;
                    }
                    if !active.insert(id) {
                        return Err(Refusal::Cycle(id));
                    }
                    let targets = graph.get(&id).ok_or(Refusal::MissingCallee(id))?;
                    pending.push(GraphVisit::Close(id));
                    pending.extend(targets.iter().rev().map(|target| GraphVisit::Open(*target)));
                }
                GraphVisit::Close(id) => {
                    active.remove(&id);
                    finished.insert(id);
                }
            }
        }
    }
    Ok(())
}

enum Activation<'a> {
    Function(usize, ActivePath),
    Operation(&'a Op, ActivePath),
    Observe(ActivePath),
}

fn operations_at<'a>(
    nodes: impl IntoIterator<Item = &'a Op>,
    path: &ActivePath,
) -> Vec<Activation<'a>> {
    nodes
        .into_iter()
        .map(|op| Activation::Operation(op, path.clone()))
        .collect()
}

fn retaining(path: &ActivePath, boundary: StackDuty) -> ActivePath {
    let mut extended = path.clone();
    extended.duties.push(boundary);
    extended
}

fn protocol_activations<'a>(op: &'a Op, path: &ActivePath) -> Option<Vec<Activation<'a>>> {
    let (Kind::Iterate {
        protocol,
        callee,
        parameters,
        body,
        evaluation,
        ..
    }
    | Kind::Callback {
        protocol,
        callee,
        parameters,
        body,
        evaluation,
        ..
    }) = &op.kind
    else {
        return None;
    };
    let boundary = duty(op, DutyKind::Primitive(*protocol, callee.clone()));
    let inside = retaining(path, boundary);
    let mut scheduled = vec![
        Activation::Operation(evaluation, path.clone()),
        Activation::Observe(inside.clone()),
    ];
    let mut callback_path = path.clone();
    if matches!(&op.kind, Kind::Callback { .. }) || callee == "Iterator::all" {
        callback_path = inside;
        callback_path
            .frames
            .push(FrameKey::Callback(op.site.clone()));
        callback_path.calls.push(op.site.clone());
        scheduled.push(Activation::Observe(callback_path.clone()));
    }
    // A for/next call has returned before its inline body. Callback protocols
    // instead remain outstanding while the callback and its cleanup execute.
    scheduled.push(Activation::Operation(body, callback_path.clone()));
    if parameters.iter().any(model::needs_cleanup) {
        scheduled.push(Activation::Observe(retaining(
            &callback_path,
            duty(op, DutyKind::Cleanup),
        )));
    }
    Some(scheduled)
}

fn schedule<'a>(op: &'a Op, path: &ActivePath) -> Vec<Activation<'a>> {
    if let Some(protocol) = protocol_activations(op, path) {
        return protocol;
    }
    if let Kind::Call {
        target, evaluation, ..
    } = &op.kind
    {
        let mut scheduled = operations_at(evaluation, path);
        let mut callee_path = path.clone();
        callee_path.frames.push(FrameKey::Function(*target));
        callee_path.calls.push(op.site.clone());
        scheduled.push(Activation::Function(*target, callee_path));
        return scheduled;
    }
    if let Kind::Primitive {
        lemma,
        callee,
        arguments,
        source_calls,
        ..
    } = &op.kind
    {
        let mut scheduled = operations_at(arguments, path);
        let inside = retaining(path, duty(op, DutyKind::Primitive(*lemma, callee.clone())));
        scheduled.push(Activation::Observe(inside.clone()));
        scheduled.extend(operations_at(source_calls, &inside));
        return scheduled;
    }
    let mut scheduled = operations_at(children(op), path);
    scheduled.extend(
        local_duties(op)
            .into_iter()
            .map(|boundary| Activation::Observe(retaining(path, boundary))),
    );
    scheduled
}

fn activation_paths(recipe: &Recipe) -> Result<Vec<ActivePath>, Refusal> {
    let mut pending: Vec<_> = recipe
        .roots
        .iter()
        .rev()
        .map(|id| {
            Activation::Function(
                *id,
                ActivePath {
                    frames: vec![FrameKey::Function(*id)],
                    calls: Vec::new(),
                    duties: Vec::new(),
                },
            )
        })
        .collect();
    let mut paths = Vec::new();
    while let Some(activation) = pending.pop() {
        match activation {
            Activation::Function(id, path) => {
                let function = recipe
                    .functions
                    .get(&id)
                    .ok_or(Refusal::MissingCallee(id))?;
                paths.push(path.clone());
                pending.push(Activation::Operation(&function.body, path));
            }
            Activation::Operation(op, path) => {
                pending.extend(schedule(op, &path).into_iter().rev())
            }
            Activation::Observe(path) => paths.push(path),
        }
    }
    Ok(paths)
}

fn check_sources<'a>(
    seed: &'a CapturedSeed,
    view: ValidationView<'a>,
) -> Result<CheckedBodies<'a, OwnerAnalysis>, handoff::Refusal> {
    handoff::checked_bodies(seed, view, |body| {
        let operations = operations::classify_source_body(body.syntax, &[], &[])
            .map_err(|_error| handoff::Refusal::BodiesUnavailable)?;
        let shape = derive_shape(operations.recipe());
        Ok(OwnerAnalysis {
            owner: body.owner.to_owned(),
            marker: body.marker.to_owned(),
            operations,
            shape,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ActivePath, CallEdge, DutyKind, FrameKey, FrameSlot, Refusal, StackDuty, StackShape,
        check_sources, derive_shape,
    };
    use crate::schema_build::application::CapturedSeed;
    use crate::schema_residency::with_current_descriptor;
    use crate::schema_validation::{
        handoff,
        operations::{
            self, BodyOperations, ExternalService, Primitive,
            model::{self, Kind, Op, Recipe, Site},
        },
        stack::{
            CallPath, DepthInputs, LogicalFormula, Refusal as StackRefusal, SourceHelper,
            TargetIdentity, evaluate_hypothetical,
        },
    };
    use serde_json::Value;
    use std::{collections::BTreeSet, slice};

    fn classified(syntax: &syn::File) -> BodyOperations {
        operations::classify_source_body(syntax, &[], &[]).unwrap()
    }

    fn function(recipe: &Recipe, name: &str) -> usize {
        let mut matching = recipe
            .functions
            .iter()
            .filter(|(_, row)| row.identity == name);
        let (id, _) = matching.next().expect("exact fixture function");
        assert!(matching.next().is_none());
        *id
    }

    fn expected_calls(recipe: &Recipe) -> Vec<CallEdge> {
        let mut result = Vec::new();
        for (id, row) in &recipe.functions {
            model::walk(&row.body, &mut |op| {
                assert_eq!(op.site.function, *id);
                if let Kind::Call { target, .. } = &op.kind {
                    assert!(recipe.functions.contains_key(target));
                    result.push(CallEdge {
                        site: op.site.clone(),
                        target: *target,
                    });
                }
                Ok(())
            })
            .unwrap();
        }
        result
    }

    fn expected_frames(recipe: &Recipe) -> Vec<FrameKey> {
        let mut result: Vec<_> = recipe
            .functions
            .keys()
            .copied()
            .map(FrameKey::Function)
            .collect();
        for row in recipe.functions.values() {
            model::walk(&row.body, &mut |op| {
                if matches!(&op.kind, Kind::Callback { .. })
                    || matches!(&op.kind, Kind::Iterate { callee, .. } if callee == "Iterator::all")
                {
                    result.push(FrameKey::Callback(op.site.clone()));
                }
                Ok(())
            })
            .unwrap();
        }
        result
    }

    fn record_duties(op: &Op, result: &mut Vec<StackDuty>) {
        let mut push = |kind| {
            result.push(StackDuty {
                site: op.site.clone(),
                kind,
            })
        };
        if let Kind::Primitive { lemma, callee, .. } = &op.kind {
            push(DutyKind::Primitive(*lemma, callee.clone()));
        }
        if let Kind::Service(binding) = &op.kind {
            push(DutyKind::Service(binding.clone()));
        }
        if let Kind::Iterate {
            protocol,
            callee,
            parameters,
            ..
        }
        | Kind::Callback {
            protocol,
            callee,
            parameters,
            ..
        } = &op.kind
        {
            push(DutyKind::Primitive(*protocol, callee.clone()));
            if parameters.iter().any(model::needs_cleanup) {
                push(DutyKind::Cleanup);
            }
        }
        if matches!(
            &op.kind,
            Kind::Construct {
                cleanup: Some(_),
                ..
            }
        ) || matches!(&op.kind, Kind::Scope { locals, .. } if locals.iter().any(model::needs_cleanup))
        {
            push(DutyKind::Cleanup);
        }
    }

    fn expected_duties(recipe: &Recipe) -> Vec<StackDuty> {
        let mut result = Vec::new();
        for row in recipe.functions.values() {
            model::walk(&row.body, &mut |op| {
                record_duties(op, &mut result);
                Ok(())
            })
            .unwrap();
        }
        result
    }

    fn assert_duties(recipe: &Recipe, duties: &[StackDuty]) {
        let expected = expected_duties(recipe);
        assert_eq!(duties, expected.as_slice());
        for duty in duties {
            assert!(recipe.functions.contains_key(&duty.site.function));
            assert!(!duty.site.lexical.is_empty());
            match &duty.kind {
                DutyKind::Primitive(lemma, callee) => {
                    assert!(!callee.is_empty());
                    assert!(expected_duties(recipe).contains(&StackDuty {
                        site: duty.site.clone(),
                        kind: DutyKind::Primitive(*lemma, callee.clone()),
                    }));
                }
                DutyKind::Service(binding) => {
                    let mut found = false;
                    for row in recipe.functions.values() {
                        model::walk(&row.body, &mut |op| {
                            if let Kind::Service(expected) = &op.kind {
                                found |= op.site == duty.site && expected == binding;
                            }
                            Ok(())
                        })
                        .unwrap();
                    }
                    assert!(found);
                }
                DutyKind::Cleanup => assert!(expected_duties(recipe).contains(duty)),
            }
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    enum CoverageGap {
        MissingPath,
        UnexpectedPath,
        MissingDuty,
    }

    enum SourceTask<'a> {
        Enter(usize, ActivePath),
        Visit(&'a Op, ActivePath),
    }

    fn serial_tasks<'a>(pending: &mut Vec<SourceTask<'a>>, steps: &'a [Op], path: &ActivePath) {
        for step in steps.iter().rev() {
            pending.push(SourceTask::Visit(step, path.clone()));
        }
    }

    fn with_duty(path: &ActivePath, duty: StackDuty) -> ActivePath {
        let mut result = path.clone();
        result.duties.push(duty);
        result
    }

    fn callback_tasks<'a>(
        pending: &mut Vec<SourceTask<'a>>,
        op: &'a Op,
        path: &ActivePath,
        required: &mut Vec<ActivePath>,
    ) {
        let (Kind::Iterate {
            protocol,
            callee,
            parameters,
            body,
            evaluation,
            ..
        }
        | Kind::Callback {
            protocol,
            callee,
            parameters,
            body,
            evaluation,
            ..
        }) = &op.kind
        else {
            panic!("callback oracle receives a callback/loop node");
        };
        let protocol_path = with_duty(
            path,
            StackDuty {
                site: op.site.clone(),
                kind: DutyKind::Primitive(*protocol, callee.clone()),
            },
        );
        required.push(protocol_path.clone());
        let mut body_path = path.clone();
        if matches!(&op.kind, Kind::Callback { .. }) || callee == "Iterator::all" {
            body_path = protocol_path;
            body_path.frames.push(FrameKey::Callback(op.site.clone()));
            body_path.calls.push(op.site.clone());
            required.push(body_path.clone());
        }
        if parameters.iter().any(model::needs_cleanup) {
            required.push(with_duty(
                &body_path,
                StackDuty {
                    site: op.site.clone(),
                    kind: DutyKind::Cleanup,
                },
            ));
        }
        pending.push(SourceTask::Visit(body, body_path));
        pending.push(SourceTask::Visit(evaluation, path.clone()));
    }

    fn operation_tasks<'a>(
        pending: &mut Vec<SourceTask<'a>>,
        op: &'a Op,
        path: &ActivePath,
        required: &mut Vec<ActivePath>,
    ) {
        if matches!(&op.kind, Kind::Iterate { .. } | Kind::Callback { .. }) {
            callback_tasks(pending, op, path, required);
            return;
        }
        let mut local_duties = Vec::new();
        record_duties(op, &mut local_duties);
        for duty in &local_duties {
            required.push(with_duty(path, duty.clone()));
        }
        if let Kind::Call {
            target, evaluation, ..
        } = &op.kind
        {
            assert!(!path.frames.contains(&FrameKey::Function(*target)));
            let mut entered = path.clone();
            entered.frames.push(FrameKey::Function(*target));
            entered.calls.push(op.site.clone());
            pending.push(SourceTask::Enter(*target, entered));
            serial_tasks(pending, evaluation, path);
        } else if let Kind::Primitive {
            arguments,
            source_calls,
            ..
        } = &op.kind
        {
            assert_eq!(local_duties.len(), 1);
            let inside = with_duty(path, local_duties.remove(0));
            serial_tasks(pending, source_calls, &inside);
            serial_tasks(pending, arguments, path);
        } else if let Kind::Scope { steps, .. }
        | Kind::Evaluate(_, steps)
        | Kind::ScalarTest(_, steps)
        | Kind::ShortCircuit(_, steps) = &op.kind
        {
            serial_tasks(pending, steps, path);
        } else if let Kind::Bind { value, .. } | Kind::Exit(_, Some(value)) = &op.kind {
            pending.push(SourceTask::Visit(value, path.clone()));
        } else if let Kind::Choice { selector, arms } = &op.kind {
            for arm in arms.iter().rev() {
                pending.push(SourceTask::Visit(&arm.body, path.clone()));
                if let Some(guard) = &arm.guard {
                    pending.push(SourceTask::Visit(guard, path.clone()));
                }
            }
            pending.push(SourceTask::Visit(selector, path.clone()));
        } else if let Kind::Construct { fields, .. } = &op.kind {
            for (_, value) in fields.iter().rev() {
                pending.push(SourceTask::Visit(value, path.clone()));
            }
        } else {
            assert!(matches!(
                &op.kind,
                Kind::Value(_) | Kind::Service(_) | Kind::Exit(_, None)
            ));
        }
    }

    // Independent test oracle over the authenticated Recipe, never the stub
    // deriver and never another AST parser/runtime-cost model. Preserve every
    // entry/duty context, including identical callees at distinct source sites.
    fn required_paths(recipe: &Recipe) -> Vec<ActivePath> {
        let mut pending = Vec::new();
        for root in recipe.roots.iter().rev() {
            pending.push(SourceTask::Enter(
                *root,
                ActivePath {
                    frames: vec![FrameKey::Function(*root)],
                    calls: vec![],
                    duties: vec![],
                },
            ));
        }
        let mut required = Vec::new();
        while let Some(task) = pending.pop() {
            match task {
                SourceTask::Enter(id, path) => {
                    let row = recipe.functions.get(&id).expect("source oracle callee");
                    required.push(path.clone());
                    pending.push(SourceTask::Visit(&row.body, path));
                }
                SourceTask::Visit(op, path) => {
                    operation_tasks(&mut pending, op, &path, &mut required)
                }
            }
        }
        required
    }

    fn check_coverage(recipe: &Recipe, shape: &StackShape) -> Result<(), CoverageGap> {
        if expected_duties(recipe)
            .iter()
            .any(|duty| !shape.duties.contains(duty))
        {
            return Err(CoverageGap::MissingDuty);
        }
        let mut remaining = required_paths(recipe);
        for path in &shape.paths {
            let Some(index) = remaining.iter().position(|expected| expected == path) else {
                return Err(CoverageGap::UnexpectedPath);
            };
            remaining.remove(index);
        }
        if !remaining.is_empty() {
            return Err(CoverageGap::MissingPath);
        }
        Ok(())
    }

    fn assert_inventory(recipe: &Recipe, shape: &StackShape) {
        assert_eq!(shape.roots, recipe.roots);
        assert_eq!(shape.calls, expected_calls(recipe));
        assert_duties(recipe, &shape.duties);
        let keys: Vec<_> = shape.frames.iter().map(|slot| slot.key.clone()).collect();
        assert_eq!(keys, expected_frames(recipe));
        assert!(shape.frames.iter().all(|slot| slot.bytes.is_none()));
        assert_eq!(check_coverage(recipe, shape), Ok(()));
        for path in &shape.paths {
            assert!(!path.frames.is_empty());
            assert_eq!(path.frames.len().checked_sub(1), Some(path.calls.len()));
            assert!(path.frames.iter().all(|key| keys.contains(key)));
            assert!(path.duties.iter().all(|duty| shape.duties.contains(duty)));
            for (key, site) in path.frames.iter().skip(1).zip(&path.calls) {
                match key {
                    FrameKey::Function(target) => assert!(shape.calls.contains(&CallEdge {
                        site: site.clone(),
                        target: *target,
                    })),
                    FrameKey::Callback(callback) => assert_eq!(callback, site),
                }
            }
        }
        for root in &recipe.roots {
            assert!(shape.paths.contains(&ActivePath {
                frames: vec![FrameKey::Function(*root)],
                calls: Vec::new(),
                duties: Vec::new(),
            }));
        }
    }

    fn assert_actual_roots(graph: &Value, marker: &str, recipe: &Recipe) {
        let edges = graph
            .get("auxiliary")
            .and_then(|row| row.get("dispatch_edges"))
            .and_then(Value::as_array)
            .unwrap();
        let mut expected = Vec::new();
        for (role, method) in [
            ("is-valid-with-call-cache", "is_valid"),
            ("validation-call-cache-factory", "new_search_cache"),
        ] {
            assert_eq!(
                edges
                    .iter()
                    .filter(|row| {
                        row.get("from").and_then(Value::as_str) == Some("validate_native")
                            && row.get("to").and_then(Value::as_str) == Some(marker)
                            && row.get("role").and_then(Value::as_str) == Some(role)
                    })
                    .count(),
                1
            );
            expected.push(function(recipe, &format!("{marker}::{method}")));
        }
        assert_eq!(recipe.roots, expected);
    }

    fn changed_seed(seed: &CapturedSeed) -> CapturedSeed {
        CapturedSeed {
            identity_sha256: "fake_other_stack_capture".to_owned(),
            manifest_bytes: seed.manifest_bytes.clone(),
            rust: seed.rust.clone(),
            build_root: seed.build_root.clone(),
            out_dir: seed.out_dir.clone(),
            snapshots: seed.snapshots.clone(),
        }
    }

    #[test]
    fn actual_stack_roots_and_call_sites_match_the_capture() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            assert_eq!(view.check_handoff(view.assignments()), Ok(()));
            let declarations = view
                .graph()
                .get("declarations")
                .and_then(Value::as_array)
                .unwrap();
            assert!(!declarations.is_empty());
            let checked_result = check_sources(seed, view);
            assert_eq!(
                checked_result.as_ref().map(|data| data.analyses().len()),
                Ok(declarations.len())
            );
            let checked = checked_result.unwrap();
            assert_eq!(checked.recheck(view), Ok(()));
            let mut owners = BTreeSet::new();
            let mut no_cache = 0_usize;
            for analysis in checked.analyses() {
                assert!(owners.insert(analysis.owner.as_str()));
                let declaration = declarations
                    .iter()
                    .find(|row| {
                        row.get("owner").and_then(Value::as_str) == Some(analysis.owner.as_str())
                    })
                    .unwrap();
                assert_eq!(
                    declaration.get("marker").and_then(Value::as_str),
                    Some(analysis.marker.as_str())
                );
                let recipe = analysis.operations.recipe();
                assert!(!recipe.functions.is_empty());
                assert_actual_roots(view.graph(), &analysis.marker, recipe);
                if declaration
                    .get("programs")
                    .and_then(Value::as_array)
                    .unwrap()
                    .is_empty()
                {
                    no_cache = no_cache.checked_add(1).unwrap();
                    assert!(recipe.initialization.values().all(Vec::is_empty));
                }
            }
            assert_eq!(owners.len(), declarations.len());
            assert!(no_cache > 0);
            assert_eq!(
                check_sources(&changed_seed(seed), view).map(|data| data.analyses().len()),
                Err(handoff::Refusal::Binding)
            );
            assert_eq!(checked.recheck(view), Ok(()));
            for analysis in checked.analyses() {
                assert_eq!(
                    analysis.shape.as_ref().map(|shape| &shape.roots),
                    Ok(&analysis.operations.recipe().roots)
                );
                assert_inventory(
                    analysis.operations.recipe(),
                    analysis.shape.as_ref().unwrap(),
                );
            }
            assert_eq!(checked.recheck(view), Ok(()));
            Ok(())
        })
        .unwrap();
    }

    fn call_fixture() -> BodyOperations {
        classified(&syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value, condition: bool) -> bool {
                leaf(true);
                if condition { outer(leaf(false)) } else { leaf(true) }
            }
            fn outer(value: bool) -> bool { leaf(value) }
            fn leaf(value: bool) -> bool { value }
        })
    }

    fn sites(recipe: &Recipe, caller: usize, callee: usize) -> Vec<Site> {
        expected_calls(recipe)
            .into_iter()
            .filter(|edge| edge.site.function == caller && edge.target == callee)
            .map(|edge| edge.site)
            .collect()
    }

    fn expected_shape(recipe: &Recipe, paths: Vec<ActivePath>) -> StackShape {
        StackShape {
            roots: recipe.roots.clone(),
            frames: expected_frames(recipe)
                .into_iter()
                .map(|key| FrameSlot { key, bytes: None })
                .collect(),
            calls: expected_calls(recipe),
            paths,
            duties: expected_duties(recipe),
        }
    }

    fn assert_path_guard_controls(recipe: &Recipe, original: &StackShape) {
        assert_eq!(check_coverage(recipe, original), Ok(()));
        let root = function(recipe, "is_valid");
        let outer = function(recipe, "outer");
        let leaf = function(recipe, "leaf");
        let index = original
            .paths
            .iter()
            .position(|path| {
                path.frames
                    == vec![
                        FrameKey::Function(root),
                        FrameKey::Function(outer),
                        FrameKey::Function(leaf),
                    ]
            })
            .unwrap();
        let required = original.paths[index].clone();
        assert_eq!(required.calls.len(), 2);
        let mut missing = original.clone();
        assert_eq!(missing.paths.remove(index), required);
        assert_eq!(
            check_coverage(recipe, &missing),
            Err(CoverageGap::MissingPath)
        );
        let mut wrong_caller = original.clone();
        let mutated = &mut wrong_caller.paths[index];
        assert_eq!(mutated.frames[1], FrameKey::Function(outer));
        mutated.frames[1] = FrameKey::Function(leaf);
        mutated.calls[0] = sites(recipe, root, leaf).first().unwrap().clone();
        // Both edges exist globally, but the second edge's actual caller is
        // outer, not the leaf now preceding it in the claimed active chain.
        assert_eq!(mutated.calls[1].function, outer);
        for (key, site) in mutated.frames.iter().skip(1).zip(&mutated.calls) {
            let FrameKey::Function(target) = key else {
                panic!("function-only fixture");
            };
            assert!(original.calls.contains(&CallEdge {
                site: site.clone(),
                target: *target
            }));
        }
        assert_eq!(
            check_coverage(recipe, &wrong_caller),
            Err(CoverageGap::UnexpectedPath)
        );
    }

    fn assert_duty_guard_controls(recipe: &Recipe, original: &StackShape, kind: &DutyKind) {
        assert_eq!(check_coverage(recipe, original), Ok(()));
        let index = original
            .duties
            .iter()
            .position(|duty| &duty.kind == kind)
            .unwrap();
        let required = original.duties[index].clone();
        let mut missing = original.clone();
        assert_eq!(missing.duties.remove(index), required);
        assert_eq!(
            check_coverage(recipe, &missing),
            Err(CoverageGap::MissingDuty)
        );
        let path_index = original
            .paths
            .iter()
            .position(|path| path.duties.last() == Some(&required))
            .unwrap();
        let mut unattached = original.clone();
        assert_eq!(
            unattached.paths.remove(path_index),
            original.paths[path_index]
        );
        assert_eq!(unattached.duties, original.duties);
        assert_eq!(
            check_coverage(recipe, &unattached),
            Err(CoverageGap::MissingPath)
        );
    }

    #[test]
    fn stack_paths_separate_calls_arguments_and_alternatives() {
        let operations = call_fixture();
        let recipe = operations.recipe();
        let root = function(recipe, "is_valid");
        let outer = function(recipe, "outer");
        let leaf = function(recipe, "leaf");
        let direct = sites(recipe, root, leaf);
        assert_eq!(direct.len(), 3);
        assert_ne!(direct[0], direct[1]);
        assert_ne!(direct[1], direct[2]);
        let outer_site = sites(recipe, root, outer);
        let nested_site = sites(recipe, outer, leaf);
        assert_eq!(outer_site.len(), 1);
        assert_eq!(nested_site.len(), 1);
        let path = |frames: &[usize], calls: Vec<Site>| ActivePath {
            frames: frames.iter().copied().map(FrameKey::Function).collect(),
            calls,
            duties: Vec::new(),
        };
        let expected = vec![
            path(&[root], vec![]),
            path(&[root, leaf], vec![direct[0].clone()]),
            path(&[root, leaf], vec![direct[1].clone()]),
            path(&[root, outer], vec![outer_site[0].clone()]),
            path(
                &[root, outer, leaf],
                vec![outer_site[0].clone(), nested_site[0].clone()],
            ),
            path(&[root, leaf], vec![direct[2].clone()]),
        ];
        let baseline = expected_shape(recipe, expected);
        assert_path_guard_controls(recipe, &baseline);
        let shape_result = derive_shape(recipe);
        assert_eq!(shape_result, Ok(baseline));
        let shape = shape_result.unwrap();
        assert_inventory(recipe, &shape);
    }

    fn traversal_fixture(callback: bool) -> BodyOperations {
        let body: syn::Expr = if callback {
            syn::parse_quote!(items.iter().all(|item| leaf(item)))
        } else {
            syn::parse_quote!({
                for item in items.iter() {
                    leaf(item);
                }
                true
            })
        };
        classified(&syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                match instance { serde_json::Value::Array(items) => #body, _ => false }
            }
            fn leaf(instance: &serde_json::Value) -> bool { instance.is_null() }
        })
    }

    fn traversal_site(recipe: &Recipe) -> (Site, String) {
        let mut result = Vec::new();
        for row in recipe.functions.values() {
            model::walk(&row.body, &mut |op| {
                if let Kind::Iterate { callee, .. } = &op.kind {
                    result.push((op.site.clone(), callee.clone()));
                }
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(result.len(), 1);
        result.remove(0)
    }

    fn assert_traversal_overlap(operations: &BodyOperations, callback: bool) {
        let recipe = operations.recipe();
        let root = function(recipe, "is_valid");
        let leaf = function(recipe, "leaf");
        let (site, callee) = traversal_site(recipe);
        let protocol = StackDuty {
            site: site.clone(),
            kind: DutyKind::Primitive(Primitive::IteratorProtocol, callee),
        };
        assert!(expected_duties(recipe).contains(&protocol));
        let call = sites(recipe, root, leaf);
        assert_eq!(call.len(), 1);
        let mut frames = vec![FrameKey::Function(root)];
        let mut calls = Vec::new();
        if callback {
            frames.push(FrameKey::Callback(site.clone()));
            calls.push(site);
        }
        frames.push(FrameKey::Function(leaf));
        calls.push(call[0].clone());
        let expected = ActivePath {
            frames,
            calls,
            duties: if callback {
                vec![protocol.clone()]
            } else {
                vec![]
            },
        };
        let shape_result = derive_shape(recipe);
        assert_eq!(
            shape_result
                .as_ref()
                .map(|shape| shape.paths.contains(&expected)),
            Ok(true)
        );
        let shape = shape_result.unwrap();
        assert_inventory(recipe, &shape);
        if !callback {
            assert!(
                !shape
                    .paths
                    .iter()
                    .any(|path| path.frames.contains(&FrameKey::Function(leaf))
                        && path.duties.contains(&protocol))
            );
        }
    }

    fn assert_option_and_service_duties() {
        let operations = classified(&syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                let value = Some(instance);
                value.map(|item| leaf(item)).is_some()
            }
            fn leaf(instance: &serde_json::Value) -> bool { instance.is_null() }
        });
        let recipe = operations.recipe();
        let duties = expected_duties(recipe);
        assert!(duties.iter().any(|duty| duty.kind == DutyKind::Cleanup));
        let protocol = duties.iter().find(|duty| {
            matches!(&duty.kind, DutyKind::Primitive(Primitive::ValueAccess, callee) if callee == "Option::map")
        }).unwrap();
        let oracle = expected_shape(recipe, required_paths(recipe));
        assert_duty_guard_controls(recipe, &oracle, &DutyKind::Cleanup);
        let shape_result = derive_shape(recipe);
        assert_eq!(
            shape_result.as_ref().map(|shape| &shape.duties),
            Ok(&duties)
        );
        let shape = shape_result.unwrap();
        assert_inventory(recipe, &shape);
        assert!(shape.paths.iter().any(|path| {
            path.frames
                .contains(&FrameKey::Callback(protocol.site.clone()))
                && path.duties.contains(protocol)
        }));
        // Missing owner-supported fixture DATA must refuse. The positively
        // bound service and exact retained path are checked separately below.
        let clock: syn::File = syn::parse_quote! {
            fn is_valid(expired: &mut dyn FnMut() -> bool) -> bool { !expired() }
        };
        assert_eq!(
            operations::classify_source_body(&clock, &[], &[]),
            Err(operations::Refusal::MissingServiceBinding(
                ExternalService::Clock
            ))
        );
    }

    #[test]
    fn callbacks_and_opaque_stack_duties_are_not_inline_loops() {
        let inline = traversal_fixture(false);
        let callback = traversal_fixture(true);
        assert_eq!(traversal_site(inline.recipe()).1, "Iterator::next/for");
        assert_eq!(traversal_site(callback.recipe()).1, "Iterator::all");
        assert_traversal_overlap(&inline, false);
        assert_traversal_overlap(&callback, true);
        assert_option_and_service_duties();
        assert_clock_duty();
    }

    fn assert_clock_duty() {
        let syntax: syn::File = syn::parse_quote! {
            fn is_valid(expired: &mut dyn FnMut() -> bool) -> bool { !expired() }
        };
        let binding = operations::clock_binding_fixture();
        let classification =
            operations::classify_source_body(&syntax, &[], slice::from_ref(&binding));
        assert_eq!(
            classification
                .as_ref()
                .map(|body| body.recipe().roots.len()),
            Ok(1)
        );
        let operations = classification.unwrap();
        let recipe = operations.recipe();
        let root = function(recipe, "is_valid");
        let duties = expected_duties(recipe);
        assert_eq!(duties.len(), 1);
        let duty = duties.first().unwrap();
        assert_eq!(duty.site.function, root);
        assert!(!duty.site.lexical.is_empty());
        assert_eq!(duty.kind, DutyKind::Service(binding.clone()));
        let expected = expected_shape(
            recipe,
            vec![
                ActivePath {
                    frames: vec![FrameKey::Function(root)],
                    calls: vec![],
                    duties: vec![],
                },
                ActivePath {
                    frames: vec![FrameKey::Function(root)],
                    calls: vec![],
                    duties: vec![duty.clone()],
                },
            ],
        );
        assert_eq!(expected.duties, duties);
        assert_duty_guard_controls(recipe, &expected, &DutyKind::Service(binding));
        let shape_result = derive_shape(recipe);
        assert_eq!(shape_result, Ok(expected));
        let shape = shape_result.unwrap();
        assert_inventory(recipe, &shape);
    }

    fn retarget(op: &mut Op, from: usize, to: usize) -> bool {
        if let Kind::Call { target, .. } = &mut op.kind {
            assert_eq!(*target, from);
            *target = to;
            true
        } else if let Kind::Scope { steps, .. } = &mut op.kind {
            steps.iter_mut().any(|step| retarget(step, from, to))
        } else {
            false
        }
    }

    fn assert_hypothetical_target_is_separate() {
        // Existing target evaluator, explicitly hypothetical. No conversion
        // from captured StackShape into its fixture witnesses is provided.
        let owner = SourceHelper {
            source_owner: "private_stack_fixture",
            source_sha256: "fake_source",
            helper: "leaf",
        };
        let formula = LogicalFormula {
            path: CallPath::Frame(owner),
            required_frames: vec![owner],
        };
        let identity = TargetIdentity {
            artifact_sha256: "fake_artifact",
            compiler: "fake_compiler",
            target: "fake_target",
            profile: "fake_profile",
            features: "fake_features",
            abi: "fake_abi",
            panic_mode: "fake_unwind",
        };
        assert_eq!(
            evaluate_hypothetical(
                &formula,
                DepthInputs {
                    schema_levels: 1,
                    instance_containers: 1
                },
                identity,
                &[]
            ),
            Err(StackRefusal::MissingTargetFact(owner))
        );
    }

    #[test]
    fn source_cycles_and_missing_callees_are_not_target_frame_gaps() {
        let operations = classified(&syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool { leaf(instance) }
            fn leaf(instance: &serde_json::Value) -> bool { true }
        });
        let recipe = operations.recipe();
        let root = function(recipe, "is_valid");
        let leaf = function(recipe, "leaf");
        assert_eq!(recipe.roots, vec![root]);
        assert_eq!(sites(recipe, root, leaf).len(), 1);
        assert_eq!(
            derive_shape(recipe).as_ref().map(|shape| &shape.roots),
            Ok(&recipe.roots)
        );
        let mut missing = recipe.clone();
        assert_eq!(missing.functions.remove(&leaf).unwrap().identity, "leaf");
        assert_eq!(derive_shape(&missing), Err(Refusal::MissingCallee(leaf)));
        let mut cycle = recipe.clone();
        assert!(retarget(
            &mut cycle.functions.get_mut(&root).unwrap().body,
            leaf,
            root
        ));
        assert_eq!(sites(&cycle, root, root).len(), 1);
        assert_eq!(derive_shape(&cycle), Err(Refusal::Cycle(root)));
        assert_hypothetical_target_is_separate();
    }
}

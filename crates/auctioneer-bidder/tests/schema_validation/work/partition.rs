//! Traversal/affine caller-bound DATA, not source authority.
//!
//! Coefficients count existing helper entries and iterator callbacks per input
//! and per attempt. They are not new work units or a closed `WorkLedger`. Actual
//! use must obtain this same canonical `Recipe` from the sealed body handoff and
//! bind the node cap to the existing caller context. Fixture caps are not
//! installed limits. An origin spelling alone is never a partition witness.
//! Ordinary primitive, cleanup, byte, frame and target obligations stay open.
//! The old coarse counter is neither changed nor exposed as a fallback here.

use crate::schema_validation::operations::{
    self, Primitive,
    model::{
        Arm, Atom, Binding, Comparison, Function, Kind, Op, Origin, Pattern, Recipe, Site,
        StopRule, Ty,
    },
};
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Source(operations::Refusal),
    IterationBinding(Site),
    CallBinding(Site),
    Overflow,
}

impl From<operations::Refusal> for Refusal {
    fn from(refusal: operations::Refusal) -> Self {
        Self::Source(refusal)
    }
}

#[derive(Clone)]
struct InputNode {
    root: usize,
    // Each entry is one authenticated child iteration, not an origin label.
    members: Vec<usize>,
}

type ProvenanceBindings = [(Binding, InputNode)];

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

struct Deriver<'a> {
    recipe: &'a Recipe,
    child_cap: u64,
    active_functions: BTreeSet<usize>,
    active_members: Vec<InputNode>,
    next_member: usize,
    calls: Vec<CallOccurrences>,
    callbacks: Occurrences,
}

fn formal<'a>(function: &'a Function, id: usize, site: &Site) -> Result<&'a Binding, Refusal> {
    let [parameter] = function.parameters.as_slice() else {
        return Err(Refusal::CallBinding(site.clone()));
    };
    if parameter.ty != Ty::Value
        || !parameter.borrowed
        || parameter.declaration.is_some()
        || parameter.origin
            != (Origin::Parameter {
                function: id,
                ordinal: 0,
            })
        || function.result != Ty::Bool
        || function.body.site.function != id
    {
        return Err(Refusal::CallBinding(site.clone()));
    }
    Ok(parameter)
}

fn input_node(bindings: &ProvenanceBindings, binding: &Binding) -> Option<InputNode> {
    bindings
        .iter()
        .rev()
        .find_map(|(known, node)| (known == binding).then(|| node.clone()))
}

struct Traversal<'a> {
    parameter: &'a Binding,
    evaluation: &'a Op,
    body: &'a Op,
    parent: InputNode,
}

fn iterator_matches(evaluation: &Op, collection: &Binding, receiver: &Binding) -> bool {
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
        && callee == "Array::iter"
        && source_calls.is_empty()
        && actual == collection
        && argument.value == *collection
        && matches!(&argument.kind, Kind::Value(Atom::Place))
        && receiver == &evaluation.value
        && receiver.ty == Ty::Iterator(Box::new(Ty::Value))
        && receiver.borrowed
        && receiver.origin == collection.origin
        && receiver.place == collection.place
        && receiver.declaration.is_none()
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
        return Err(operations::Refusal::UnsupportedSource.into());
    };
    let refusal = || Refusal::IterationBinding(op.site.clone());
    let [parameter] = parameters.as_slice() else {
        return Err(refusal());
    };
    let mut body_site = op.site.clone();
    body_site.lexical.push(1);
    if *protocol != Primitive::IteratorProtocol
        || callee != "Iterator::all"
        || *stop != StopRule::FirstFalseOrExhaustion
        || !*none_on_exhaustion
        || bound.collection.ty != Ty::Array
        || !bound.collection.borrowed
        || bound.element != Ty::Value
        || !iterator_matches(evaluation, &bound.collection, receiver)
        || body.site != body_site
        || parameter.ty != Ty::Value
        || !parameter.borrowed
        || parameter.declaration.as_ref() != Some(&body.site)
        || parameter.origin
            != (Origin::Projection {
                base: Box::new(bound.collection.origin.clone()),
                member: "member".to_owned(),
            })
    {
        return Err(refusal());
    }
    let parent = input_node(bindings, &bound.collection).ok_or_else(refusal)?;
    Ok(Traversal {
        parameter,
        evaluation,
        body,
        parent,
    })
}

impl Deriver<'_> {
    fn occurrences(&self) -> Result<Occurrences, Refusal> {
        let Some(deepest) = self
            .active_members
            .iter()
            .max_by_key(|node| node.members.len())
        else {
            return Ok(Occurrences {
                constant: 1,
                ..Occurrences::default()
            });
        };
        // The selected node determines every ancestor member index in its
        // path. Each remaining active rescan index has at most cap-1 choices.
        let extra = self
            .active_members
            .len()
            .checked_sub(deepest.members.len())
            .ok_or(Refusal::Overflow)?;
        let mut multiplicity = 1_u64;
        for _ in 0..extra {
            multiplicity = multiplicity
                .checked_mul(self.child_cap)
                .ok_or(Refusal::Overflow)?;
        }
        Ok(Occurrences {
            constant: 0,
            by_depth: BTreeMap::from([((deepest.root, deepest.members.len()), multiplicity)]),
        })
    }

    fn enter(&mut self, id: usize, node: InputNode, site: &Site) -> Result<(), Refusal> {
        let recipe = self.recipe;
        let function = recipe
            .functions
            .get(&id)
            .ok_or(operations::Refusal::UnclassifiedCall)?;
        let parameter = formal(function, id, site)?;
        if !self.active_functions.insert(id) {
            return Err(operations::Refusal::MissingDecreasingMeasure.into());
        }
        let result = self.visit(&function.body, &[(parameter.clone(), node)]);
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
            return Err(operations::Refusal::UnsupportedSource.into());
        };
        let recipe = self.recipe;
        let function = recipe
            .functions
            .get(target)
            .ok_or(operations::Refusal::UnclassifiedCall)?;
        let parameter = formal(function, *target, &op.site)?;
        let ([argument], [evaluated]) = (arguments.as_slice(), evaluation.as_slice()) else {
            return Err(Refusal::CallBinding(op.site.clone()));
        };
        if argument != &evaluated.value
            || argument.ty != parameter.ty
            || !argument.borrowed
            || !matches!(&evaluated.kind, Kind::Value(Atom::Place))
        {
            return Err(Refusal::CallBinding(op.site.clone()));
        }
        let node =
            input_node(bindings, argument).ok_or_else(|| Refusal::CallBinding(op.site.clone()))?;
        self.visit(evaluated, bindings)?;
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
        self.enter(*target, node, &op.site)
    }

    fn choice(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        let Kind::Choice { selector, arms } = &op.kind else {
            return Err(operations::Refusal::UnsupportedSource.into());
        };
        self.visit(selector, bindings)?;
        for (ordinal, arm) in arms.iter().enumerate() {
            let mut branch = bindings.to_vec();
            match &arm.pattern {
                Pattern::Variant(name, fields)
                    if matches!(
                        name.as_str(),
                        "serde_json::Value::Array"
                            | "jsonschema::__private::serde_json::Value::Array"
                    ) =>
                {
                    branch.push(array_binding(
                        op, selector, arm, ordinal, bindings, name, fields,
                    )?);
                }
                Pattern::Wild | Pattern::Literal(_) => {}
                _ => return Err(operations::Refusal::UnsupportedSource.into()),
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
            parameter,
            evaluation,
            body,
            parent: mut node,
        } = traversal(op, bindings)?;
        self.visit(evaluation, bindings)?;
        let member = self.next_member;
        self.next_member = member.checked_add(1).ok_or(Refusal::Overflow)?;
        node.members.push(member);
        self.active_members.push(node.clone());
        let occurrences = self.occurrences()?;
        self.callbacks.combine(occurrences)?;
        let mut nested = bindings.to_vec();
        nested.push((parameter.clone(), node));
        let result = self.visit(body, &nested);
        self.active_members.pop();
        result
    }

    fn visit(&mut self, op: &Op, bindings: &ProvenanceBindings) -> Result<(), Refusal> {
        match &op.kind {
            Kind::Value(_) => Ok(()),
            Kind::Scope { steps, locals, .. } if locals.is_empty() => {
                for step in steps {
                    self.visit(step, bindings)?;
                }
                Ok(())
            }
            Kind::ScalarTest(_, values) | Kind::ShortCircuit(_, values) => {
                for value in values {
                    self.visit(value, bindings)?;
                }
                Ok(())
            }
            Kind::Choice { .. } => self.choice(op, bindings),
            Kind::Call { .. } => self.call(op, bindings),
            Kind::Iterate { .. } => self.iteration(op, bindings),
            Kind::Primitive {
                arguments,
                source_calls,
                ..
            } if source_calls.is_empty() => {
                for argument in arguments {
                    self.visit(argument, bindings)?;
                }
                Ok(())
            }
            Kind::Exit(_, value) => {
                if let Some(value) = value {
                    self.visit(value, bindings)?;
                }
                Ok(())
            }
            _ => Err(operations::Refusal::UnsupportedSource.into()),
        }
    }
}

// Payload provenance comes from the actual typed match arm and selector, then
// the iterator checks its exact receiver and binder sites. The spelling of a
// Projection alone cannot introduce either a container or a member.
fn array_binding(
    op: &Op,
    selector: &Op,
    arm: &Arm,
    ordinal: usize,
    bindings: &ProvenanceBindings,
    name: &str,
    fields: &[Pattern],
) -> Result<(Binding, InputNode), Refusal> {
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
    if arm.body.site != body_site {
        return Err(refusal());
    }
    anchor.lexical.push(0);
    Ok((
        Binding {
            ty: Ty::Array,
            place: place.clone(),
            borrowed: true,
            origin: Origin::Projection {
                base: Box::new(value.value.origin.clone()),
                member: name.to_owned(),
            },
            declaration: Some(anchor),
        },
        node,
    ))
}

fn checked_bound(coefficient: SourceBoundCoeff, cap: NodeCapData) -> Result<u64, Refusal> {
    coefficient
        .nodes
        .checked_mul(cap.nodes)
        .and_then(|value| value.checked_add(coefficient.constant))
        .ok_or(Refusal::Overflow)
}

// Only borrowed unary Value functions and the canonical array/all protocol are
// supported here. Other grammar refuses; no coarse counter is a fallback.
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
    let mut deriver = Deriver {
        recipe,
        child_cap: cap.nodes.checked_sub(1).ok_or(Refusal::Overflow)?,
        active_functions: BTreeSet::new(),
        active_members: Vec::new(),
        next_member: 0,
        calls: Vec::new(),
        callbacks: Occurrences::default(),
    };
    for &root in &recipe.roots {
        deriver.enter(
            root,
            InputNode {
                root,
                members: Vec::new(),
            },
            &Site {
                function: root,
                lexical: Vec::new(),
            },
        )?;
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
    Ok(SourceBounds {
        roots: recipe.roots.clone(),
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

    fn call_bound(recipe: &Recipe, caller: &str, callee: &str, nodes: u64) -> CallBound {
        let caller_id = function(recipe, caller);
        let target = function(recipe, callee);
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
                serialized_bytes: 65_536,
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
        let Kind::Iterate { parameters, .. } = &mut iteration_mut(&mut wrong_site, root).kind
        else {
            panic!("fixture iterator");
        };
        assert_eq!(parameters[0].declaration.as_ref(), Some(&call_site));
        parameters[0].declaration = Some(Site {
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
        caller: &str,
        callee: &str,
        per_attempt: SourceBoundCoeff,
    ) -> FixtureCall {
        let caller_id = function(recipe, caller);
        let target = function(recipe, callee);
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

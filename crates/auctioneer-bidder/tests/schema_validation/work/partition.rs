//! Tests-first traversal/affine caller-bound DATA, not source authority.
//!
//! Coefficients count existing helper entries and iterator callbacks per input
//! and per attempt. They are not new work units or a closed `WorkLedger`. Actual
//! use must obtain this same canonical `Recipe` from the sealed body handoff and
//! bind the node cap to the existing caller context. Fixture caps are not
//! installed limits. An origin spelling alone is never a partition witness.
//! Ordinary primitive, cleanup, byte, frame and target obligations stay open.
//! The old coarse counter is neither changed nor exposed as a fallback here.

use crate::schema_validation::operations::model::{Binding, Recipe, Site};
use std::collections::BTreeSet;

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
    NotDerived,
    IterationBinding(Site),
    CallBinding(Site),
    Overflow,
}

// Hosted assertion RED precedes any provenance or coefficient implementation.
// Zero attempts must still check source bindings and retain per-attempt duties.
// Coefficients and their cap-domain/attempt arithmetic must be checked; an
// unrepresentable bound refuses rather than publishing wrapped coefficients.
// No compatibility path into the old J-per-nested-pass counter is provided.
fn derive_source_bounds(
    _recipe: &Recipe,
    _cap: NodeCapData,
    _attempts: u64,
) -> Result<SourceBounds, Refusal> {
    Err(Refusal::NotDerived)
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
}

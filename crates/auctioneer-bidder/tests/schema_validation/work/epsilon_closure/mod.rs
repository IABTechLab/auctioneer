//! Local epsilon-closure recipe and supplied-facts evaluation, not a complete
//! work certificate.
//!
//! Supplied dimensions are hypotheses, not selected-program/source authority.
//! Each epoch starts with an empty destination set; roots within it share that
//! visited set, and each root enters with an empty stack. IDs must be valid.
//! n must be positive; nonzero roots require an epoch. Empty epochs are permitted.
//! The parent must join these facts to their owners.
//! Site ceilings count entries/decisions, not the work inside called primitives.

pub(super) mod primitives;

use quote::ToTokens as _;
use std::collections::BTreeSet;
use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EpochFacts {
    nfa_states: u64,
    extra_branches: u64,
    root_calls: u64,
    epochs: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SiteTable {
    root_entries: u64,
    insert_attempts: u64,
    stack_pushes: u64,
    pop_decisions: u64,
    state_dispatches: u64,
    look_tests: u64,
    union_first_tests: u64,
    tail_elements: u64,
    tail_advances: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum RequiredLemma {
    NfaStateAccess,
    SparseSetInsert,
    // LookMatcher establishes the caller's facts; this body uses LookSet.
    LookMatcherAndSet,
    VectorReserveAndGrowth,
    StateIdClone,
    ReverseClonedSliceIterator,
    Drop,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct LocalEpsilon {
    sites: SiteTable,
    required_lemmas: BTreeSet<RequiredLemma>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum EpsilonError {
    UnsupportedBody,
    UnknownSourceCall(String),
    InvalidFacts,
    Overflow,
}

fn literal_doc(attribute: &syn::Attribute) -> bool {
    if !matches!(&attribute.style, syn::AttrStyle::Outer) || !attribute.path().is_ident("doc") {
        return false;
    }
    let syn::Meta::NameValue(meta) = &attribute.meta else {
        return false;
    };
    matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_)))
}

fn receiver_root(expression: &syn::Expr) -> Option<&syn::Ident> {
    if let syn::Expr::Path(path) = expression {
        return if path.qself.is_none() && path.path.segments.len() == 1 {
            path.path.segments.first().map(|segment| &segment.ident)
        } else {
            None
        };
    }
    if let syn::Expr::MethodCall(call) = expression {
        return receiver_root(&call.receiver);
    }
    if let syn::Expr::Index(index) = expression {
        return receiver_root(&index.expr);
    }
    if let syn::Expr::Paren(paren) = expression {
        return receiver_root(&paren.expr);
    }
    if let syn::Expr::Group(group) = expression {
        return receiver_root(&group.expr);
    }
    None
}

#[derive(Default)]
struct BodyGate {
    nested_attribute: bool,
    unsupported_macro: bool,
    unknown_call: Option<String>,
}

impl VisitMut for BodyGate {
    fn visit_attribute_mut(&mut self, _i: &mut syn::Attribute) {
        self.nested_attribute = true;
    }

    fn visit_expr_call_mut(&mut self, i: &mut syn::ExprCall) {
        visit_mut::visit_expr_call_mut(self, i);
        if self.unknown_call.is_none() {
            self.unknown_call = Some(i.func.to_token_stream().to_string());
        }
    }

    fn visit_expr_method_call_mut(&mut self, i: &mut syn::ExprMethodCall) {
        visit_mut::visit_expr_method_call_mut(self, i);
        let root = receiver_root(&i.receiver).map(ToString::to_string);
        let method = i.method.to_string();
        // This inventory classifies diagnostics only. Exact recipe equality
        // below, not a recognized method name, is the acceptance condition.
        let known = matches!(
            (root.as_deref(), method.as_str()),
            (Some("nfa"), "state" | "is_epsilon")
                | (Some("set"), "insert")
                | (Some("stack"), "is_empty" | "push" | "pop" | "extend")
                | (Some("look_have"), "contains")
                | (Some("alternates"), "get" | "iter" | "rev")
        );
        if !known && self.unknown_call.is_none() {
            self.unknown_call = Some(format!("{}::{}", i.receiver.to_token_stream(), i.method));
        }
    }

    fn visit_macro_mut(&mut self, i: &mut syn::Macro) {
        if !i.path.is_ident("assert") {
            self.unsupported_macro = true;
            return;
        }
        let parser = Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        match parser.parse2(i.tokens.clone()) {
            Ok(arguments) => {
                for mut argument in arguments {
                    self.visit_expr_mut(&mut argument);
                }
            }
            Err(_) => self.unsupported_macro = true,
        }
    }
}

// Independent supported AST recipe, never compiled as a traversal/proof function.
fn supported_epsilon_recipe() -> syn::ItemFn {
    syn::parse_quote! {
        pub(crate) fn epsilon_closure(
            nfa: &thompson::NFA,
            start_nfa_id: StateID,
            look_have: LookSet,
            stack: &mut Vec<StateID>,
            set: &mut SparseSet,
        ) {
            assert!(stack.is_empty());
            if !nfa.state(start_nfa_id).is_epsilon() {
                set.insert(start_nfa_id);
                return;
            }
            stack.push(start_nfa_id);
            while let Some(mut id) = stack.pop() {
                loop {
                    if !set.insert(id) {
                        break;
                    }
                    match *nfa.state(id) {
                        thompson::State::ByteRange { .. }
                        | thompson::State::Sparse { .. }
                        | thompson::State::Dense { .. }
                        | thompson::State::Fail
                        | thompson::State::Match { .. } => break,
                        thompson::State::Look { look, next } => {
                            if !look_have.contains(look) {
                                break;
                            }
                            id = next;
                        }
                        thompson::State::Union { ref alternates } => {
                            id = match alternates.get(0) {
                                None => break,
                                Some(&id) => id,
                            };
                            stack.extend(alternates[1..].iter().rev());
                        }
                        thompson::State::BinaryUnion { alt1, alt2 } => {
                            id = alt1;
                            stack.push(alt2);
                        }
                        thompson::State::Capture { next, .. } => {
                            id = next;
                        }
                    }
                }
            }
        }
    }
}

/// Match the supported signature/body, not just its source fingerprint.
/// Only literal-string doc function attributes are supported; other attributes
/// refuse even when signature/body tokens are unchanged. Nested attributes
/// must match the complete supported body recipe.
///
/// For n states, b extra branches, F roots and E epochs, the site table is:
/// root entries F; insert attempts A=F+(n+b)E; pushes P=F+bE;
/// pop decisions Q=P+F (including exhaustion); dispatches V=nE;
/// Look/Union-first decisions <=V; tail elements <=bE; advances <=(b+n)E.
/// State dispatch is visited-set bounded, not AST-size/graph-exploration work.
/// Arithmetic is checked. Unrecognized callees refuse rather than cost zero;
/// recognized but undischarged callees remain required lemmas on success.
pub(super) fn epsilon_sites(
    source: &syn::ItemFn,
    facts: EpochFacts,
) -> Result<LocalEpsilon, EpsilonError> {
    if !source.attrs.iter().all(literal_doc) {
        return Err(EpsilonError::UnsupportedBody);
    }
    let supported = supported_epsilon_recipe();
    if source.sig.to_token_stream().to_string() != supported.sig.to_token_stream().to_string() {
        return Err(EpsilonError::UnsupportedBody);
    }
    let mut gate = BodyGate::default();
    gate.visit_block_mut(&mut source.block.as_ref().clone());
    if gate.nested_attribute || gate.unsupported_macro {
        return Err(EpsilonError::UnsupportedBody);
    }
    if let Some(callee) = gate.unknown_call {
        return Err(EpsilonError::UnknownSourceCall(callee));
    }
    if source.block.to_token_stream().to_string() != supported.block.to_token_stream().to_string() {
        return Err(EpsilonError::UnsupportedBody);
    }
    if facts.nfa_states == 0 || (facts.epochs == 0 && facts.root_calls != 0) {
        return Err(EpsilonError::InvalidFacts);
    }
    let dispatches = facts
        .nfa_states
        .checked_mul(facts.epochs)
        .ok_or(EpsilonError::Overflow)?;
    let tails = facts
        .extra_branches
        .checked_mul(facts.epochs)
        .ok_or(EpsilonError::Overflow)?;
    let pushes = facts
        .root_calls
        .checked_add(tails)
        .ok_or(EpsilonError::Overflow)?;
    let attempts = pushes
        .checked_add(dispatches)
        .ok_or(EpsilonError::Overflow)?;
    let pops = pushes
        .checked_add(facts.root_calls)
        .ok_or(EpsilonError::Overflow)?;
    let advances = tails
        .checked_add(dispatches)
        .ok_or(EpsilonError::Overflow)?;
    Ok(LocalEpsilon {
        sites: SiteTable {
            root_entries: facts.root_calls,
            insert_attempts: attempts,
            stack_pushes: pushes,
            pop_decisions: pops,
            state_dispatches: dispatches,
            look_tests: dispatches,
            union_first_tests: dispatches,
            tail_elements: tails,
            tail_advances: advances,
        },
        required_lemmas: BTreeSet::from([
            RequiredLemma::NfaStateAccess,
            RequiredLemma::SparseSetInsert,
            RequiredLemma::LookMatcherAndSet,
            RequiredLemma::VectorReserveAndGrowth,
            RequiredLemma::StateIdClone,
            RequiredLemma::ReverseClonedSliceIterator,
            RequiredLemma::Drop,
        ]),
    })
}

/// Local reverse-tail protocol: t successful advances plus terminating None.
/// This does not discharge Vec reserve/growth, cloning, iterator bodies or drop.
pub(super) fn tail_advances(elements: u64) -> Result<u64, EpsilonError> {
    elements.checked_add(1).ok_or(EpsilonError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pristine_source() -> syn::ItemFn {
        let file = syn::parse_file(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/regex-automata/src/util/determinize/mod.rs"
        )))
        .expect("pristine determinization source parses as data");
        file.items
            .into_iter()
            .find_map(|item| {
                let syn::Item::Fn(function) = item else {
                    return None;
                };
                (function.sig.ident == "epsilon_closure").then_some(function)
            })
            .expect("pristine epsilon_closure body is present")
    }

    fn capture_facts() -> EpochFacts {
        EpochFacts {
            nfa_states: 1,
            extra_branches: 0,
            root_calls: 1,
            epochs: 1,
        }
    }

    fn obligations() -> BTreeSet<RequiredLemma> {
        BTreeSet::from([
            RequiredLemma::NfaStateAccess,
            RequiredLemma::SparseSetInsert,
            RequiredLemma::LookMatcherAndSet,
            RequiredLemma::VectorReserveAndGrowth,
            RequiredLemma::StateIdClone,
            RequiredLemma::ReverseClonedSliceIterator,
            RequiredLemma::Drop,
        ])
    }

    fn counts(table: &SiteTable) -> [u64; 9] {
        [
            table.root_entries,
            table.insert_attempts,
            table.stack_pushes,
            table.pop_decisions,
            table.state_dispatches,
            table.look_tests,
            table.union_first_tests,
            table.tail_elements,
            table.tail_advances,
        ]
    }

    fn assert_sites(facts: EpochFacts, expected: [u64; 9]) {
        let [
            root_entries,
            insert_attempts,
            stack_pushes,
            pop_decisions,
            state_dispatches,
            look_tests,
            union_first_tests,
            tail_elements,
            tail_advances,
        ] = expected;
        let expected_local = LocalEpsilon {
            sites: SiteTable {
                root_entries,
                insert_attempts,
                stack_pushes,
                pop_decisions,
                state_dispatches,
                look_tests,
                union_first_tests,
                tail_elements,
                tail_advances,
            },
            required_lemmas: obligations(),
        };
        assert_eq!(
            epsilon_sites(&pristine_source(), facts)
                .map(|local| (counts(&local.sites), local.required_lemmas)),
            Ok((
                counts(&expected_local.sites),
                expected_local.required_lemmas
            ))
        );
    }

    #[test]
    fn pristine_body_matches_the_independent_recipe() {
        let source = pristine_source();
        let recipe = supported_epsilon_recipe();
        assert_eq!(
            source.sig.to_token_stream().to_string(),
            recipe.sig.to_token_stream().to_string()
        );
        assert_eq!(
            source.block.to_token_stream().to_string(),
            recipe.block.to_token_stream().to_string()
        );
        assert_sites(capture_facts(), [1, 2, 1, 2, 1, 1, 1, 0, 1]);
    }

    #[test]
    fn capture_self_loop_has_literal_site_ceilings() {
        let facts = capture_facts();
        assert_eq!(
            (
                facts.nfa_states,
                facts.extra_branches,
                facts.root_calls,
                facts.epochs
            ),
            (1, 0, 1, 1)
        );
        // Hypothetical Capture(0)->0: insert succeeds, then the revisit fails.
        assert_sites(facts, [1, 2, 1, 2, 1, 1, 1, 0, 1]);
    }

    #[test]
    fn duplicate_union_alternatives_have_a_conservative_ceiling() {
        // Hypothetical BinaryUnion(1,1), terminal 1; root 0 is requested twice.
        let facts = EpochFacts {
            nfa_states: 2,
            extra_branches: 1,
            root_calls: 2,
            epochs: 1,
        };
        assert_sites(facts, [2, 5, 3, 5, 2, 2, 2, 1, 3]);
        // Toy trace: new root, new terminal, duplicate alternate, duplicate root.
        // Four actual insertions are not the independently expected upper five.
        let actual_toy_insertions = 4_u64;
        assert_eq!(
            epsilon_sites(&pristine_source(), facts).map(|local| (
                local.sites.insert_attempts,
                actual_toy_insertions <= local.sites.insert_attempts,
                actual_toy_insertions == local.sites.insert_attempts,
            )),
            Ok((5, true, false))
        );
    }

    #[test]
    fn roots_vary_without_multiplying_epoch_dispatches() {
        assert_sites(
            EpochFacts {
                root_calls: 3,
                ..capture_facts()
            },
            [3, 4, 3, 6, 1, 1, 1, 0, 1],
        );
    }

    #[test]
    fn epochs_vary_independently_of_root_calls() {
        // Two of these supplied epochs may have no roots.
        assert_sites(
            EpochFacts {
                epochs: 3,
                ..capture_facts()
            },
            [1, 4, 1, 2, 3, 3, 3, 0, 3],
        );
    }

    #[test]
    fn nfa_states_vary_independently_of_branches() {
        assert_sites(
            EpochFacts {
                nfa_states: 3,
                ..capture_facts()
            },
            [1, 4, 1, 2, 3, 3, 3, 0, 3],
        );
    }

    #[test]
    fn branches_vary_independently_of_nfa_states() {
        assert_sites(
            EpochFacts {
                extra_branches: 3,
                ..capture_facts()
            },
            [1, 5, 4, 5, 1, 1, 1, 3, 4],
        );
    }

    #[test]
    fn empty_local_execution_does_not_discharge_primitive_lemmas() {
        assert_sites(
            EpochFacts {
                root_calls: 0,
                epochs: 0,
                ..capture_facts()
            },
            [0, 0, 0, 0, 0, 0, 0, 0, 0],
        );
    }

    #[test]
    fn reverse_tail_counts_the_final_none() {
        assert_eq!(tail_advances(0), Ok(1));
        assert_eq!(tail_advances(1), Ok(2));
        assert_eq!(tail_advances(3), Ok(4));
    }

    #[test]
    fn changed_known_source_is_not_approved_by_its_identity() {
        let mut source = pristine_source();
        assert_eq!(
            epsilon_sites(&source, capture_facts()).map(|local| local.required_lemmas),
            Ok(obligations())
        );
        let mut nonliteral_doc = source.clone();
        nonliteral_doc
            .attrs
            .push(syn::parse_quote!(#[doc = stringify!(unreviewed)]));
        assert_eq!(
            epsilon_sites(&nonliteral_doc, capture_facts()),
            Err(EpsilonError::UnsupportedBody)
        );
        let mut nested = source.clone();
        nested.block.stmts[0] = syn::parse_quote! {
            #[unreviewed_runtime_attribute]
            assert!(stack.is_empty());
        };
        assert_eq!(
            epsilon_sites(&nested, capture_facts()),
            Err(EpsilonError::UnsupportedBody)
        );
        let mut attributed = source.clone();
        attributed
            .attrs
            .push(syn::parse_quote!(#[unreviewed_runtime_attribute]));
        assert_eq!(
            epsilon_sites(&attributed, capture_facts()),
            Err(EpsilonError::UnsupportedBody)
        );
        source.block.stmts[0] = syn::parse_quote! { assert!(!stack.is_empty()); };
        assert_eq!(
            epsilon_sites(&source, capture_facts()),
            Err(EpsilonError::UnsupportedBody)
        );
    }

    #[test]
    fn unknown_source_call_is_refused_not_assigned_zero_work() {
        let mut source = pristine_source();
        assert_eq!(
            epsilon_sites(&source, capture_facts()).map(|local| local.required_lemmas),
            Ok(obligations())
        );
        source
            .block
            .stmts
            .push(syn::parse_quote! { unreviewed(stack); });
        assert_eq!(
            epsilon_sites(&source, capture_facts()),
            Err(EpsilonError::UnknownSourceCall("unreviewed".to_owned()))
        );
    }

    #[test]
    fn malformed_epoch_hypotheses_are_refused() {
        for facts in [
            EpochFacts {
                nfa_states: 0,
                ..capture_facts()
            },
            EpochFacts {
                epochs: 0,
                ..capture_facts()
            },
        ] {
            assert_eq!(
                epsilon_sites(&pristine_source(), facts),
                Err(EpsilonError::InvalidFacts)
            );
        }
    }

    #[test]
    fn overflow_is_refused_for_each_independent_formula_axis() {
        for facts in [
            EpochFacts {
                root_calls: u64::MAX,
                ..capture_facts()
            },
            EpochFacts {
                epochs: u64::MAX,
                ..capture_facts()
            },
            EpochFacts {
                nfa_states: u64::MAX,
                epochs: 2,
                ..capture_facts()
            },
            EpochFacts {
                extra_branches: u64::MAX,
                epochs: 2,
                ..capture_facts()
            },
            EpochFacts {
                root_calls: u64::MAX.checked_div(2).unwrap().checked_add(1).unwrap(),
                ..capture_facts()
            },
        ] {
            assert_eq!(
                epsilon_sites(&pristine_source(), facts),
                Err(EpsilonError::Overflow)
            );
        }
        assert_eq!(tail_advances(u64::MAX), Err(EpsilonError::Overflow));
    }
}

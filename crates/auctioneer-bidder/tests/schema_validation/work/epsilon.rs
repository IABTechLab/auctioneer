//! Tests-first local epsilon-closure recipe, not a complete work certificate.
//!
//! Supplied dimensions are hypotheses, not selected-program/source authority.
//! Each epoch starts with an empty destination set; roots within it share that
//! visited set, and each root enters with an empty stack. IDs must be valid.
//! n must be positive; nonzero roots require an epoch. Empty epochs are permitted.
//! The parent must join these facts to their owners.
//! Site ceilings count entries/decisions, not the work inside called primitives.

use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EpochFacts {
    pub(super) nfa_states: u64,
    pub(super) extra_branches: u64,
    pub(super) root_calls: u64,
    pub(super) epochs: u64,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SiteTable {
    pub(super) root_entries: u64,
    pub(super) insert_attempts: u64,
    pub(super) stack_pushes: u64,
    pub(super) pop_decisions: u64,
    pub(super) state_dispatches: u64,
    pub(super) look_tests: u64,
    pub(super) union_first_tests: u64,
    pub(super) tail_elements: u64,
    pub(super) tail_advances: u64,
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
    pub(super) sites: SiteTable,
    pub(super) required_lemmas: BTreeSet<RequiredLemma>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum EpsilonError {
    MissingSourceProof,
    UnsupportedBody,
    UnknownSourceCall(String),
    InvalidFacts,
    Overflow,
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
    _source: &syn::ItemFn,
    _facts: EpochFacts,
) -> Result<LocalEpsilon, EpsilonError> {
    Err(EpsilonError::MissingSourceProof)
}

/// Local reverse-tail protocol: t successful advances plus terminating None.
/// This does not discharge Vec reserve/growth, cloning, iterator bodies or drop.
pub(super) fn tail_advances(_elements: u64) -> Result<u64, EpsilonError> {
    Err(EpsilonError::MissingSourceProof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::ToTokens;

    fn pristine_source() -> syn::ItemFn {
        let file = syn::parse_file(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vendor/regex-automata/src/util/determinize/mod.rs"
        )))
        .expect("pristine determinization source parses as data");
        file.items
            .into_iter()
            .find_map(|item| match item {
                syn::Item::Fn(function) if function.sig.ident == "epsilon_closure" => {
                    Some(function)
                }
                _ => None,
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
        let [root_entries, insert_attempts, stack_pushes, pop_decisions, state_dispatches, look_tests, union_first_tests, tail_elements, tail_advances] =
            expected;
        let expected = LocalEpsilon {
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
            Ok((counts(&expected.sites), expected.required_lemmas))
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
                root_calls: u64::MAX / 2 + 1,
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

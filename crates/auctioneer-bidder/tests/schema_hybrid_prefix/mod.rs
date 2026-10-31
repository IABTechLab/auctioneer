//! Structural optional-prefix superset; no claim about the selected strategy.

use crate::schema_cache_bounds::BoundsError;
use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
use crate::schema_hybrid_capacity::{CapacityError, ReportedCapacityProof};
use crate::schema_hybrid_graph::GraphError;
use crate::schema_hybrid_heap::{CompleteHeapError, HybridHeap};
use regex_automata::nfa::thompson::{self, NFA, WhichCaptures};
use regex_automata::util::{look::LookMatcher, syntax};
use regex_syntax::hir::{Hir, HirKind, Look};
use std::fmt;

#[derive(Clone, Copy)]
pub(crate) struct PrefixBudget {
    pub nodes: usize,
    pub depth: usize,
    pub prefixes: usize,
}

pub(crate) const QUALIFICATION_BUDGET: PrefixBudget = PrefixBudget {
    nodes: 4096,
    depth: 128,
    prefixes: 64,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PrefixError {
    Parse,
    TraversalLimit,
    PrefixLimit,
    Projection(BoundsError),
    Capacity(CapacityError),
    Graph(GraphError),
    RecoveryUnavailable,
}

impl fmt::Display for PrefixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse => f.write_str("invalid_syntax"),
            Self::TraversalLimit => f.write_str("hir_traversal_limit"),
            Self::PrefixLimit => f.write_str("prefix_limit"),
            Self::Projection(BoundsError::Arithmetic) => f.write_str("arithmetic_overflow"),
            Self::Projection(BoundsError::Compilation) => f.write_str("compilation_refused"),
            Self::Projection(BoundsError::Shape) => f.write_str("unexpected_shape"),
            Self::Capacity(reason) => write!(f, "capacity_{reason}"),
            Self::Graph(reason) => write!(f, "graph_{reason}"),
            Self::RecoveryUnavailable => f.write_str("recovery_unavailable"),
        }
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct PrefixEnvelope {
    pub candidates: usize,
    pub peak_bytes: usize,
    pub required_reported_bytes: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OptionalPrefixEnvelope {
    ExcludedAnchored { structural_candidates: usize },
    NoConcat,
    Alternatives(PrefixEnvelope),
}

pub(crate) fn for_meta_pattern(pattern: &str) -> Result<OptionalPrefixEnvelope, PrefixError> {
    let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true))
        .map_err(|_error| PrefixError::Parse)?;
    let prefixes = from_hir(&hir, QUALIFICATION_BUDGET)?;
    // For one pattern, meta's union properties are this exact HIR's properties.
    if hir.properties().look_set_prefix().contains(Look::Start) {
        return Ok(OptionalPrefixEnvelope::ExcludedAnchored {
            structural_candidates: prefixes.len(),
        });
    }
    if prefixes.is_empty() {
        return Ok(OptionalPrefixEnvelope::NoConcat);
    }
    PrefixEnvelope::from_candidates(prefixes).map(OptionalPrefixEnvelope::Alternatives)
}

impl PrefixEnvelope {
    pub(crate) fn from_pattern(pattern: &str) -> Result<Self, PrefixError> {
        Self::from_candidates(candidates(pattern)?)
    }

    fn from_candidates(prefixes: Vec<Hir>) -> Result<Self, PrefixError> {
        let mut envelope = Self::default();
        for prefix in prefixes {
            let nfa = reverse_prefix_nfa(&prefix).map_err(PrefixError::Projection)?;
            let dfa = candidate_hybrid(&nfa, HybridDirection::ReverseSharedStarts)
                .map_err(|_error| PrefixError::Projection(BoundsError::Compilation))?;
            // This constructor checks coarse heap/recovery before any graph.
            let capacity = ReportedCapacityProof::from_non_quitting_dfa(&dfa)
                .map_err(PrefixError::Capacity)?;
            let heap = HybridHeap::from_complete_dfa(&dfa)
                .map_err(|error| match error {
                    CompleteHeapError::Projection(reason) => PrefixError::Projection(reason),
                    CompleteHeapError::Exploration(reason) => PrefixError::Graph(reason),
                })?
                .ok_or(PrefixError::RecoveryUnavailable)?;
            envelope.include(heap.peak, capacity.required)?;
        }
        Ok(envelope)
    }

    fn include(&mut self, peak: usize, reported: usize) -> Result<(), PrefixError> {
        let count = self
            .candidates
            .checked_add(1)
            .ok_or(PrefixError::Projection(BoundsError::Arithmetic))?;
        self.peak_bytes = self.peak_bytes.max(peak);
        self.required_reported_bytes = self.required_reported_bytes.max(reported);
        self.candidates = count;
        Ok(())
    }
}

pub(crate) fn candidates(pattern: &str) -> Result<Vec<Hir>, PrefixError> {
    let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true))
        .map_err(|_error| PrefixError::Parse)?;
    from_hir(&hir, QUALIFICATION_BUDGET)
}

fn from_hir(mut hir: &Hir, budget: PrefixBudget) -> Result<Vec<Hir>, PrefixError> {
    let mut remaining = budget.nodes;
    inspect(hir, &mut remaining, 1, budget.depth)?;
    while let HirKind::Capture(capture) = hir.kind() {
        hir = &capture.sub;
    }
    let HirKind::Concat(children) = hir.kind() else {
        return Ok(Vec::new());
    };
    // Capture removal can merge adjacent literals and eliminate the concat.
    let normalized = Hir::concat(children.iter().map(without_captures).collect());
    let HirKind::Concat(normalized_children) = normalized.into_kind() else {
        return Ok(Vec::new());
    };
    if normalized_children.len().saturating_sub(1) > budget.prefixes {
        return Err(PrefixError::PrefixLimit);
    }
    let mut prefixes = Vec::new();
    for index in 1..normalized_children.len() {
        let prefix = normalized_children
            .get(..index)
            .ok_or(PrefixError::PrefixLimit)?;
        prefixes.push(Hir::concat(prefix.to_vec()));
    }
    Ok(prefixes)
}

fn inspect(
    hir: &Hir,
    remaining: &mut usize,
    depth: usize,
    max_depth: usize,
) -> Result<(), PrefixError> {
    if depth > max_depth {
        return Err(PrefixError::TraversalLimit);
    }
    *remaining = remaining
        .checked_sub(1)
        .ok_or(PrefixError::TraversalLimit)?;
    for child in hir.kind().subs() {
        inspect(
            child,
            remaining,
            depth.checked_add(1).ok_or(PrefixError::TraversalLimit)?,
            max_depth,
        )?;
    }
    Ok(())
}

fn without_captures(hir: &Hir) -> Hir {
    match hir.kind() {
        HirKind::Empty => Hir::empty(),
        HirKind::Literal(literal) => Hir::literal(literal.0.clone()),
        HirKind::Class(class) => Hir::class(class.clone()),
        HirKind::Look(look) => Hir::look(*look),
        HirKind::Repetition(repetition) => {
            Hir::repetition(repetition.with(without_captures(&repetition.sub)))
        }
        HirKind::Capture(capture) => without_captures(&capture.sub),
        HirKind::Concat(children) => Hir::concat(children.iter().map(without_captures).collect()),
        HirKind::Alternation(children) => {
            Hir::alternation(children.iter().map(without_captures).collect())
        }
    }
}

pub(crate) fn reverse_prefix_nfa(prefix: &Hir) -> Result<NFA, BoundsError> {
    let mut look = LookMatcher::new();
    look.set_line_terminator(b'\n');
    NFA::compiler()
        .configure(
            thompson::Config::new()
                .utf8(true)
                .reverse(true)
                .which_captures(WhichCaptures::None)
                .shrink(false)
                .nfa_size_limit(Some(0x0001_0000))
                .look_matcher(look),
        )
        .build_from_hir(prefix)
        .map_err(|_error| BoundsError::Compilation)
}

#[cfg(test)]
mod tests {
    use super::{
        OptionalPrefixEnvelope, PrefixBudget, PrefixEnvelope, PrefixError, candidates,
        for_meta_pattern, from_hir, reverse_prefix_nfa,
    };
    use crate::schema_cache_bounds::{BoundsError, NfaDimensions};
    use crate::schema_hybrid_bounds::QuitAdmissionError;
    use crate::schema_hybrid_capacity::CapacityError;
    use regex_automata::Anchored;
    use regex_automata::nfa::thompson::pikevm::PikeVM;
    use regex_automata::util::syntax;
    use regex_syntax::hir::{Capture, Hir};
    use toml::Value;

    fn budget(nodes: usize, depth: usize, prefixes: usize) -> PrefixBudget {
        PrefixBudget {
            nodes,
            depth,
            prefixes,
        }
    }

    #[test]
    fn structural_parser_is_exactly_pinned_and_native_test_only() {
        let workspace: Value = toml::from_str(include_str!("../../../../Cargo.toml")).unwrap();
        let dependency = &workspace["workspace"]["dependencies"]["regex-syntax"];
        assert_eq!(dependency["version"].as_str(), Some("=0.8.11"));
        assert_eq!(dependency["default-features"].as_bool(), Some(false));
        assert_eq!(
            dependency["features"].as_array().unwrap(),
            &[
                Value::String("std".to_owned()),
                Value::String("unicode".to_owned()),
            ]
        );
        let bidder: Value = toml::from_str(include_str!("../../Cargo.toml")).unwrap();
        assert!(bidder["dependencies"].get("regex-syntax").is_none());
        assert_eq!(bidder["target"]["cfg(not(target_arch = \"wasm32\"))"]["dev-dependencies"]["regex-syntax"]["workspace"].as_bool(), Some(true));
    }

    #[test]
    fn one_unproved_alternative_refuses_instead_of_returning_a_partial_maximum() {
        use crate::schema_hybrid_graph::GraphError;
        for patterns in [["a", "[01]{16}1[01]*"], ["[01]{16}1[01]*", "a"]] {
            assert_eq!(
                PrefixEnvelope::from_candidates(
                    patterns
                        .map(|pattern| syntax::parse(pattern).unwrap())
                        .to_vec()
                ),
                Err(PrefixError::Capacity(CapacityError::Exploration(
                    GraphError::Cleared
                )))
            );
        }
    }

    #[test]
    fn a_quitting_prefix_refuses_before_graph_exploration() {
        assert_eq!(
            PrefixEnvelope::from_pattern(r"\bword[0-9]+"),
            Err(PrefixError::Capacity(CapacityError::Quit(
                QuitAdmissionError::UnicodeWordAssertion
            )))
        );
    }

    #[test]
    fn optional_owner_maxima_keep_distinct_axes_and_do_not_sum_alternatives() {
        let mut envelope = PrefixEnvelope::default();
        for (peak, reported) in [(19, 5), (7, 11), (19, 8)] {
            envelope.include(peak, reported).unwrap();
        }
        assert_eq!(
            envelope,
            PrefixEnvelope {
                candidates: 3,
                peak_bytes: 19,
                required_reported_bytes: 11,
            }
        );
        let mut full = PrefixEnvelope {
            candidates: usize::MAX,
            peak_bytes: 19,
            required_reported_bytes: 11,
        };
        assert_eq!(
            full.include(20, 12),
            Err(PrefixError::Projection(BoundsError::Arithmetic))
        );
        assert_eq!(full.peak_bytes, 19);
        assert_eq!(full.required_reported_bytes, 11);
    }

    #[test]
    fn a_verified_nonconcat_root_has_no_optional_owner_but_bad_syntax_refuses() {
        assert_eq!(
            PrefixEnvelope::from_pattern("[a-z]+"),
            Ok(PrefixEnvelope::default())
        );
        assert_eq!(PrefixEnvelope::from_pattern("("), Err(PrefixError::Parse));
    }

    #[test]
    fn only_always_start_anchored_patterns_exclude_the_optional_strategy() {
        for pattern in ["^a[0-9]+z", "(^a|^b)", "(((^a)))", "^"] {
            assert_eq!(
                for_meta_pattern(pattern),
                Ok(OptionalPrefixEnvelope::ExcludedAnchored {
                    structural_candidates: candidates(pattern).unwrap().len(),
                })
            );
        }
        for pattern in ["^a|b", "(?m)^a", "\\^a", "a$", "a[0-9]+z"] {
            let envelope = PrefixEnvelope::from_pattern(pattern).unwrap();
            let expected = if envelope.candidates == 0 {
                OptionalPrefixEnvelope::NoConcat
            } else {
                OptionalPrefixEnvelope::Alternatives(envelope)
            };
            assert_eq!(for_meta_pattern(pattern), Ok(expected));
        }
        assert_eq!(for_meta_pattern("("), Err(PrefixError::Parse));
    }

    #[test]
    fn optional_envelope_binds_all_literal_prefixes_to_real_reverse_components() {
        use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
        use crate::schema_hybrid_capacity::ReportedCapacityProof;
        use crate::schema_hybrid_heap::HybridHeap;
        let mut expected_peaks = Vec::new();
        let mut expected_reports = Vec::new();
        // Independent explicit prefixes, not the implementation's HIR slicing.
        for pattern in ["a", "a[0-9]+"] {
            let nfa = reverse_prefix_nfa(&syntax::parse(pattern).unwrap()).unwrap();
            let dfa = candidate_hybrid(&nfa, HybridDirection::ReverseSharedStarts).unwrap();
            let capacity = ReportedCapacityProof::from_dfa(&dfa).unwrap();
            let heap = HybridHeap::from_complete_dfa(&dfa).unwrap().unwrap();
            expected_peaks.push(heap.peak);
            expected_reports.push(capacity.required);
        }
        assert_ne!(expected_peaks.first(), expected_peaks.get(1));
        assert_ne!(expected_reports.first(), expected_reports.get(1));
        assert_eq!(
            PrefixEnvelope::from_pattern("(a)[0-9]+z"),
            Ok(PrefixEnvelope {
                candidates: 2,
                peak_bytes: expected_peaks.into_iter().max().unwrap(),
                required_reported_bytes: expected_reports.into_iter().max().unwrap(),
            })
        );
    }

    #[test]
    fn capture_flattening_merges_literals_before_choosing_boundaries() {
        let expected = ["ab", "ab[0-9]+"]
            .map(|pattern| syntax::parse(pattern).unwrap())
            .to_vec();
        for pattern in ["(a)(b)[0-9]+z", "(((a)(b)[0-9]+z))"] {
            assert_eq!(candidates(pattern), Ok(expected.clone()));
        }
    }

    #[test]
    fn flattening_removes_nested_captures_under_repetition_and_alternation() {
        assert_eq!(
            candidates("(([0-9](ab|cd))+)Z"),
            Ok(vec![syntax::parse("(?:[0-9](?:ab|cd))+").unwrap()])
        );
    }

    #[test]
    fn nonconcat_roots_and_concat_collapsed_by_normalization_have_no_prefix() {
        for pattern in ["abc", "[ab]+", "a|b", "((abc))", "(ab)c", "(?:ab|cd)+"] {
            assert_eq!(candidates(pattern), Ok(Vec::new()));
        }
    }

    #[test]
    fn assertion_only_prefixes_are_not_mistaken_for_absent_work() {
        let expected = ["^", "^[0-9]+", "^[0-9]+-", "^[0-9]+-[a-z]+"]
            .map(|pattern| syntax::parse(pattern).unwrap())
            .to_vec();
        assert_eq!(candidates("^[0-9]+-[a-z]+$"), Ok(expected));
    }

    #[test]
    fn unicode_prefixes_use_hir_boundaries_not_source_byte_offsets() {
        assert_eq!(
            candidates("(\\x{E9})[0-9]+z"),
            Ok(["\\x{E9}", "\\x{E9}[0-9]+"]
                .map(|pattern| syntax::parse(pattern).unwrap())
                .to_vec())
        );
    }

    #[test]
    fn invalid_syntax_refuses_instead_of_reporting_zero_prefixes() {
        assert_eq!(candidates("("), Err(PrefixError::Parse));
    }

    #[test]
    fn traversal_node_depth_and_prefix_limits_are_independent_and_inclusive() {
        // Concat plus literal, repetition/class, literal: five nodes, depth 3.
        let hir = syntax::parse("a[0-9]+z").unwrap();
        let expected = ["a", "a[0-9]+"]
            .map(|pattern| syntax::parse(pattern).unwrap())
            .to_vec();
        for limits in [budget(5, 3, 2), budget(6, 4, 3)] {
            assert_eq!(from_hir(&hir, limits), Ok(expected.clone()));
        }
        for limits in [
            budget(4, 3, 2),
            budget(5, 2, 2),
            budget(0, 3, 2),
            budget(5, 0, 2),
        ] {
            assert_eq!(from_hir(&hir, limits), Err(PrefixError::TraversalLimit));
        }
        for prefixes in [0, 1] {
            assert_eq!(
                from_hir(&hir, budget(5, 3, prefixes)),
                Err(PrefixError::PrefixLimit)
            );
        }
    }

    #[test]
    fn outer_capture_chains_are_counted_before_removal() {
        let mut hir = syntax::parse("a[0-9]+z").unwrap();
        for index in 1..=4 {
            hir = Hir::capture(Capture {
                index,
                name: None,
                sub: Box::new(hir),
            });
        }
        assert_eq!(from_hir(&hir, budget(9, 7, 2)).unwrap().len(), 2);
        assert_eq!(
            from_hir(&hir, budget(8, 7, 2)),
            Err(PrefixError::TraversalLimit)
        );
        assert_eq!(
            from_hir(&hir, budget(9, 6, 2)),
            Err(PrefixError::TraversalLimit)
        );
    }

    #[test]
    fn reverse_prefix_compilation_keeps_end_anchor_without_capture_storage() {
        let hir = syntax::parse("(ab)[0-9]+$").unwrap();
        let nfa = reverse_prefix_nfa(&hir).unwrap();
        assert!(nfa.is_reverse());
        let dimensions = NfaDimensions::from_nfa(&nfa).unwrap();
        assert_eq!((dimensions.slots, dimensions.capture_states), (0, 0));
        let vm = PikeVM::new_from_nfa(nfa).unwrap();
        let mut cache = vm.create_cache();
        for (haystack, expected) in [("12ba", true), ("x12ba", false), ("12baX", true)] {
            let input = regex_automata::Input::new(haystack).anchored(Anchored::Yes);
            assert_eq!(vm.is_match(&mut cache, input), expected, "{haystack}");
        }
    }
}

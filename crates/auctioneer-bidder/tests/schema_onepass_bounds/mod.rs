//! Conditional one-pass scratch lengths and retained table heap, not whole-meta qualification.

use crate::schema_cache_bounds::{BoundsError, forward_nfa};
use crate::schema_heap_layout::{HeapLayout, LayoutError};
use regex_automata::{
    MatchKind,
    dfa::onepass::{self, DFA},
    nfa::thompson::NFA,
    util::primitives::StateID,
};
use std::mem::size_of;

pub(crate) fn candidate_onepass(nfa: &NFA) -> Result<DFA, Box<onepass::BuildError>> {
    DFA::builder()
        .configure(
            DFA::config()
                .match_kind(MatchKind::LeftmostFirst)
                .starts_for_each_pattern(true)
                .byte_classes(true)
                .size_limit(Some(0x0010_0000)),
        )
        .build_from_nfa(nfa.clone())
        .map_err(Box::new)
}

/// `Some` is a conditional length, not proof that meta constructs/selects the DFA.
pub(crate) fn explicit_scratch_slots(
    total_slots: usize,
    patterns: usize,
) -> Result<Option<usize>, BoundsError> {
    if !total_slots.is_multiple_of(2) {
        return Err(BoundsError::Shape);
    }
    let implicit = patterns.checked_mul(2).ok_or(BoundsError::Arithmetic)?;
    let explicit = total_slots
        .checked_sub(implicit)
        .ok_or(BoundsError::Shape)?;
    Ok((explicit <= 32).then_some(explicit))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OnePassRetainedTables {
    pub table_bytes: usize,
    pub starts_bytes: usize,
    pub total: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum OnePassTablesError {
    Configuration,
    Compilation,
    Shape,
    Arithmetic,
}

impl From<LayoutError> for OnePassTablesError {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Shape => Self::Shape,
            LayoutError::Arithmetic => Self::Arithmetic,
        }
    }
}

#[derive(Debug)]
pub(crate) enum OnePassTables {
    Candidate(OnePassRetainedTables),
    // Test-only refusal evidence, not ownership in an installed engine.
    Absent(Box<onepass::BuildError>),
}

pub(crate) fn retained_tables(
    states: usize,
    stride: usize,
    patterns: usize,
) -> Result<OnePassRetainedTables, OnePassTablesError> {
    if states < 2 || !stride.is_power_of_two() || !(2..=512).contains(&stride) {
        return Err(OnePassTablesError::Shape);
    }
    // Locked onepass.rs:725-726 shrinks both buffers before success; its
    // Transition(u64) and transparent u32 StateID occupy eight/four bytes.
    // Rust 1.98.1 Vec::shrink_to_fit -> RawVec::shrink_unchecked -> Global
    // requests exactly len * width on success (failure diverges). This is
    // not a generic Vec promise, constructor peak, inline DFA or shared NFA.
    let layout = HeapLayout::new(size_of::<usize>(), size_of::<usize>())?;
    let table_entries = states
        .checked_mul(stride)
        .ok_or(OnePassTablesError::Arithmetic)?;
    let table_bytes = layout.repeat_vector(size_of::<u64>(), table_entries)?;
    let start_entries = patterns
        .checked_add(1)
        .ok_or(OnePassTablesError::Arithmetic)?;
    let starts_bytes = layout.repeat_vector(size_of::<StateID>(), start_entries)?;
    let total = table_bytes
        .checked_add(starts_bytes)
        .ok_or(OnePassTablesError::Arithmetic)?;
    Ok(OnePassRetainedTables {
        table_bytes,
        starts_bytes,
        total,
    })
}

/// Structural envelope, with `stride = 1 << nfa.byte_classes().stride2()`.
pub(crate) fn nfa_table_upper_bound(
    nfa_states: usize,
    stride: usize,
    patterns: usize,
) -> Result<OnePassRetainedTables, OnePassTablesError> {
    // onepass.rs:604,838-858 adds DEAD plus at most one DFA state per NFA ID.
    let states = nfa_states
        .checked_add(1)
        .ok_or(OnePassTablesError::Arithmetic)?;
    retained_tables(states, stride, patterns)
}

impl OnePassRetainedTables {
    pub(crate) fn from_dfa(dfa: &DFA) -> Result<Self, OnePassTablesError> {
        let config = dfa.get_config();
        if config.get_match_kind() != MatchKind::LeftmostFirst
            || !config.get_starts_for_each_pattern()
            || !config.get_byte_classes()
            || config.get_size_limit() != Some(0x0010_0000)
        {
            return Err(OnePassTablesError::Configuration);
        }
        let nfa = dfa.get_nfa();
        let maximum_states = nfa
            .states()
            .len()
            .checked_add(1)
            .ok_or(OnePassTablesError::Arithmetic)?;
        // onepass.rs:524-558 keeps the NFA class stride, including the spare
        // match-information column. state_len already includes DEAD.
        if !(1..=9).contains(&dfa.stride2())
            || dfa.stride2() != nfa.byte_classes().stride2()
            || dfa.state_len() > maximum_states
        {
            return Err(OnePassTablesError::Shape);
        }
        retained_tables(dfa.state_len(), dfa.stride(), dfa.pattern_len())
    }
}

pub(crate) fn retained_candidate(pattern: &str) -> Result<OnePassTables, OnePassTablesError> {
    let nfa = forward_nfa(pattern).map_err(|_error| OnePassTablesError::Compilation)?;
    match candidate_onepass(&nfa) {
        Ok(dfa) => OnePassRetainedTables::from_dfa(&dfa).map(OnePassTables::Candidate),
        Err(error) => Ok(OnePassTables::Absent(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OnePassRetainedTables, OnePassTables, OnePassTablesError, candidate_onepass,
        explicit_scratch_slots, nfa_table_upper_bound, retained_candidate, retained_tables,
    };
    use crate::schema_cache_bounds::{BoundsError, forward_nfa};
    use regex_automata::{
        Anchored, Input, MatchError, MatchKind, meta::Regex, nfa::thompson::NFA,
        util::primitives::NonMaxUsize,
    };
    use std::mem::size_of;

    #[test]
    fn onepass_retained_literals_vary_each_owner_dimension() {
        for (states, stride, patterns, table_bytes, starts_bytes, total) in [
            (2, 4, 0, 64, 4, 68),
            (2, 4, 1, 64, 8, 72),
            (3, 4, 1, 96, 8, 104),
            (2, 8, 1, 128, 8, 136),
            (2, 4, 2, 64, 12, 76),
            (2, 2, 0, 32, 4, 36),
            (2, 512, 0, 8192, 4, 8196),
        ] {
            assert_eq!(
                retained_tables(states, stride, patterns),
                Ok(OnePassRetainedTables {
                    table_bytes,
                    starts_bytes,
                    total
                })
            );
        }
    }

    #[test]
    fn onepass_retained_refuses_missing_dead_or_start_state_and_invalid_stride() {
        for (states, stride) in [(0, 4), (1, 4), (2, 0), (2, 1), (2, 3), (2, 513), (2, 1024)] {
            assert_eq!(
                retained_tables(states, stride, 1),
                Err(OnePassTablesError::Shape)
            );
        }
    }

    #[test]
    fn onepass_retained_rejects_extreme_dimensions_without_saturation() {
        for (states, stride, patterns) in [
            (usize::MAX, 2, 0),
            (usize::MAX.checked_div(2).unwrap(), 2, 0),
            (2, 4, usize::MAX),
            (2, 4, usize::MAX.checked_div(4).unwrap()),
            (
                2,
                4,
                usize::MAX.checked_div(4).unwrap().checked_sub(1).unwrap(),
            ),
        ] {
            assert_eq!(
                retained_tables(states, stride, patterns),
                Err(OnePassTablesError::Arithmetic)
            );
        }
    }

    #[test]
    fn onepass_retained_refuses_individually_nonallocatable_native_buffers() {
        let native_max = usize::try_from(isize::MAX).unwrap();
        let actual = [
            retained_tables(
                native_max.checked_div(16).unwrap().checked_add(1).unwrap(),
                2,
                0,
            ),
            retained_tables(2, 4, native_max.checked_div(4).unwrap()),
        ];
        assert_eq!(
            actual,
            [
                Err(OnePassTablesError::Arithmetic),
                Err(OnePassTablesError::Arithmetic),
            ]
        );
    }

    #[test]
    fn onepass_retained_nfa_envelope_adds_dead_and_preserves_projection_terms() {
        assert_eq!(
            nfa_table_upper_bound(2, 4, 1),
            Ok(OnePassRetainedTables {
                table_bytes: 96,
                starts_bytes: 8,
                total: 104,
            })
        );
        assert_eq!(
            nfa_table_upper_bound(0, 4, 1),
            Err(OnePassTablesError::Shape)
        );
        assert_eq!(
            nfa_table_upper_bound(usize::MAX, 4, 1),
            Err(OnePassTablesError::Arithmetic)
        );
        assert_eq!(
            nfa_table_upper_bound(2, 3, 1),
            Err(OnePassTablesError::Shape)
        );
    }

    #[test]
    fn onepass_retained_from_dfa_refuses_every_wrong_constructor_setting() {
        use regex_automata::dfa::onepass::DFA;

        let nfa = forward_nfa("^a+$").unwrap();
        let fixed = candidate_onepass(&nfa).unwrap().get_config().clone();
        for config in [
            fixed.clone().match_kind(MatchKind::All),
            fixed.clone().starts_for_each_pattern(false),
            fixed.clone().byte_classes(false),
            fixed.clone().size_limit(Some(0x0001_0000)),
            fixed.size_limit(None),
        ] {
            let dfa = DFA::builder()
                .configure(config)
                .build_from_nfa(nfa.clone())
                .unwrap();
            assert_eq!(
                OnePassRetainedTables::from_dfa(&dfa),
                Err(OnePassTablesError::Configuration)
            );
        }
    }

    #[test]
    fn onepass_retained_raw_success_is_a_candidate_even_without_wrapper_incentive() {
        for pattern in ["^a+$", "^(a+)(b+)$"] {
            let nfa = forward_nfa(pattern).unwrap();
            let dfa = candidate_onepass(&nfa).unwrap();
            let OnePassTables::Candidate(tables) = retained_candidate(pattern).unwrap() else {
                panic!("raw constructor success must be funded: {pattern}");
            };
            assert_eq!(tables, OnePassRetainedTables::from_dfa(&dfa).unwrap());
            assert_eq!(tables.total, dfa.memory_usage());
            assert!(tables.table_bytes > 0);
            assert_eq!(tables.starts_bytes, 8);
        }
    }

    #[test]
    fn onepass_retained_constructor_absence_is_not_a_compilation_refusal() {
        let pattern = "^(a*a)$";
        let error = candidate_onepass(&forward_nfa(pattern).unwrap()).unwrap_err();
        let OnePassTables::Absent(actual) = retained_candidate(pattern).unwrap() else {
            panic!("ambiguous candidate must retain the constructor refusal");
        };
        assert_eq!(actual.to_string(), error.to_string());
        assert!(matches!(
            retained_candidate("["),
            Err(OnePassTablesError::Compilation)
        ));
    }

    #[test]
    fn onepass_retained_structural_envelope_covers_empty_single_and_multiple_patterns() {
        for nfa in [
            NFA::never_match(),
            forward_nfa("^a+$").unwrap(),
            NFA::new_many(&["^(a+)$", "^(b+)(c+)$"]).unwrap(),
        ] {
            let dfa = candidate_onepass(&nfa).unwrap();
            let stride = 1 << nfa.byte_classes().stride2();
            assert_eq!(dfa.stride(), stride);
            assert!(dfa.state_len() >= 2 && dfa.state_len() <= nfa.states().len() + 1);
            let tables = OnePassRetainedTables::from_dfa(&dfa).unwrap();
            let upper =
                nfa_table_upper_bound(nfa.states().len(), stride, nfa.pattern_len()).unwrap();
            assert_eq!(tables.total, dfa.memory_usage());
            assert_eq!(tables.starts_bytes, 4 * (nfa.pattern_len() + 1));
            assert!(tables.table_bytes <= upper.table_bytes);
            assert!(tables.total <= upper.total);
        }
    }

    #[test]
    fn onepass_candidate_reproduces_the_locked_meta_builder_configuration() {
        let nfa = forward_nfa("^(a+)(b+)$").unwrap();
        let engine = candidate_onepass(&nfa).unwrap();
        let config = engine.get_config();
        assert_eq!(config.get_match_kind(), MatchKind::LeftmostFirst);
        assert!(config.get_starts_for_each_pattern());
        assert!(config.get_byte_classes());
        assert_eq!(config.get_size_limit(), Some(0x0010_0000));
        let meta_defaults = Regex::config();
        assert!(meta_defaults.get_onepass());
        assert_eq!(meta_defaults.get_match_kind(), MatchKind::LeftmostFirst);
        assert!(meta_defaults.get_byte_classes());
        assert_eq!(meta_defaults.get_onepass_size_limit(), Some(0x0010_0000));
    }

    #[test]
    fn onepass_scratch_excludes_two_implicit_slots_per_pattern() {
        for (total, patterns, expected) in [
            (0, 0, Some(0)),
            (2, 1, Some(0)),
            (4, 1, Some(2)),
            (6, 1, Some(4)),
            (6, 2, Some(2)),
            (8, 2, Some(4)),
        ] {
            assert_eq!(explicit_scratch_slots(total, patterns), Ok(expected));
        }
    }

    #[test]
    fn onepass_capture_limit_applies_to_explicit_not_total_slots() {
        for (total, patterns, expected) in [
            (32, 1, Some(30)),
            (34, 1, Some(32)),
            (36, 1, None),
            (36, 2, Some(32)),
        ] {
            assert_eq!(explicit_scratch_slots(total, patterns), Ok(expected));
        }
    }

    #[test]
    fn onepass_slot_projection_refuses_underflow_and_pattern_overflow() {
        for (total, patterns) in [(0, 1), (2, 2), (3, 1), (5, 1)] {
            assert_eq!(
                explicit_scratch_slots(total, patterns),
                Err(BoundsError::Shape)
            );
        }
        assert_eq!(
            explicit_scratch_slots(usize::MAX - 1, usize::MAX),
            Err(BoundsError::Arithmetic)
        );
    }

    #[test]
    fn real_onepass_builder_enforces_the_explicit_capture_slot_boundary() {
        for groups in [16, 17] {
            let pattern = format!("^{}a{}$", "(".repeat(groups), ")".repeat(groups));
            let nfa = forward_nfa(&pattern).unwrap();
            assert_eq!(nfa.group_info().slot_len(), groups * 2 + 2);
            let projected =
                explicit_scratch_slots(nfa.group_info().slot_len(), nfa.pattern_len()).unwrap();
            if groups == 16 {
                let engine = candidate_onepass(&nfa).unwrap();
                assert_eq!(projected, Some(32));
                assert_eq!(
                    engine.create_cache().memory_usage(),
                    32 * size_of::<Option<NonMaxUsize>>()
                );
            } else {
                assert_eq!(projected, None);
                assert!(
                    candidate_onepass(&nfa)
                        .unwrap_err()
                        .to_string()
                        .contains("too many explicit capturing groups (max is 16)")
                );
            }
        }
    }

    #[test]
    fn onepass_scratch_is_eager_and_unchanged_by_caller_slots_or_input_lengths() {
        let nfa = forward_nfa("^(a*)(b*)$").unwrap();
        let engine = candidate_onepass(&nfa).unwrap();
        let mut cache = engine.create_cache();
        let bytes = 4 * size_of::<Option<NonMaxUsize>>();
        assert_eq!(cache.memory_usage(), bytes);
        let long_input = "a".repeat(257);
        for caller_slots in [0, 2, 6, 4, 6] {
            let mut slots = vec![None; caller_slots];
            for (input, expected) in [
                ("", Some(0)),
                ("ab", Some(0)),
                ("aaaabbbb", Some(0)),
                (long_input.as_str(), Some(0)),
                ("c", None),
            ] {
                let found = engine
                    .try_search_slots(&mut cache, &Input::new(input).earliest(true), &mut slots)
                    .unwrap();
                assert_eq!(found.map(|pattern| pattern.as_usize()), expected);
                assert_eq!(cache.memory_usage(), bytes);
            }
        }
    }

    #[test]
    fn real_multi_pattern_scratch_subtracts_each_patterns_implicit_pair() {
        let nfa = NFA::new_many(&["^(a+)$", "^(b+)(c+)$"]).unwrap();
        assert_eq!(nfa.pattern_len(), 2);
        assert_eq!(nfa.group_info().slot_len(), 10);
        assert_eq!(nfa.group_info().explicit_slot_len(), 6);
        assert_eq!(explicit_scratch_slots(10, 2), Ok(Some(6)));
        let engine = candidate_onepass(&nfa).unwrap();
        let mut cache = engine.create_cache();
        let mut captures = engine.create_captures();
        let bytes = 6 * size_of::<Option<NonMaxUsize>>();
        assert_eq!(cache.memory_usage(), bytes);
        for (input, expected) in [
            ("aaa", Some(0)),
            ("bbbccc", Some(1)),
            ("", None),
            ("x", None),
        ] {
            engine
                .try_search(
                    &mut cache,
                    &Input::new(input).anchored(Anchored::Yes).earliest(true),
                    &mut captures,
                )
                .unwrap();
            assert_eq!(
                captures.get_match().map(|found| found.pattern().as_usize()),
                expected
            );
            assert_eq!(cache.memory_usage(), bytes);
        }
    }

    #[test]
    fn onepass_without_explicit_captures_has_no_heap_scratch_slots() {
        let nfa = forward_nfa("^a+$").unwrap();
        assert_eq!(nfa.group_info().slot_len(), 2);
        let engine = candidate_onepass(&nfa).unwrap();
        let mut cache = engine.create_cache();
        let mut captures = engine.create_captures();
        assert_eq!(cache.memory_usage(), 0);
        engine
            .try_search(&mut cache, &Input::new("aaa").earliest(true), &mut captures)
            .unwrap();
        assert!(captures.is_match());
        assert_eq!(cache.memory_usage(), 0);
    }

    #[test]
    fn unanchored_onepass_rejection_does_not_remove_eager_scratch() {
        let nfa = forward_nfa("(a+)(b+)").unwrap();
        assert!(!nfa.is_always_start_anchored());
        let engine = candidate_onepass(&nfa).unwrap();
        let mut cache = engine.create_cache();
        let mut captures = engine.create_captures();
        let bytes = 4 * size_of::<Option<NonMaxUsize>>();
        assert_eq!(cache.memory_usage(), bytes);
        assert_eq!(
            engine.try_search(&mut cache, &Input::new("ab"), &mut captures),
            Err(MatchError::unsupported_anchored(Anchored::No))
        );
        assert_eq!(cache.memory_usage(), bytes);
        engine
            .try_search(
                &mut cache,
                &Input::new("ab").anchored(Anchored::Yes).earliest(true),
                &mut captures,
            )
            .unwrap();
        assert!(captures.is_match());
        assert_eq!(cache.memory_usage(), bytes);
    }
}

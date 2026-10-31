//! Directional fixed lengths and encoding limits, not a full hybrid-cache bound.

use crate::schema_cache_bounds::{BoundsError, NfaDimensions};
use regex_automata::{
    MatchKind,
    hybrid::{
        BuildError,
        dfa::{Config, DFA},
    },
    nfa::thompson::{NFA, State},
};
use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HybridDimensions {
    pub sparse_entries: usize,
    pub start_entries: usize,
    pub sentinel_transitions: usize,
    pub epsilon_stack: usize,
    pub state_payload_bytes: usize,
}

impl fmt::Display for HybridDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sparse-entries={} starts={} sentinel-transitions={} epsilon-stack={} payload-limit={}",
            self.sparse_entries,
            self.start_entries,
            self.sentinel_transitions,
            self.epsilon_stack,
            self.state_payload_bytes
        )
    }
}

impl HybridDimensions {
    pub(crate) fn from_dfa(dfa: &DFA) -> Result<Self, BoundsError> {
        let mut matches = BTreeSet::new();
        for state in dfa.get_nfa().states() {
            if let State::Match { pattern_id } = state
                && (pattern_id.as_usize() >= dfa.pattern_len() || !matches.insert(*pattern_id))
            {
                return Err(BoundsError::Shape);
            }
        }
        let shape = NfaDimensions::from_nfa(dfa.get_nfa())?;
        let shift = u32::try_from(dfa.byte_classes().stride2())
            .map_err(|_error| BoundsError::Arithmetic)?;
        let stride = 1_usize.checked_shl(shift).ok_or(BoundsError::Arithmetic)?;
        Self::project(
            shape,
            stride,
            dfa.get_config().get_starts_for_each_pattern(),
        )
    }

    pub(crate) fn project(
        shape: NfaDimensions,
        stride: usize,
        pattern_starts: bool,
    ) -> Result<Self, BoundsError> {
        if shape.states == 0 || !stride.is_power_of_two() || !(2..=512).contains(&stride) {
            return Err(BoundsError::Shape);
        }
        let sparse_entries = shape.states.checked_mul(4).ok_or(BoundsError::Arithmetic)?;
        let start_groups = if pattern_starts {
            shape
                .patterns
                .checked_add(2)
                .ok_or(BoundsError::Arithmetic)?
        } else {
            2
        };
        let start_entries = start_groups.checked_mul(6).ok_or(BoundsError::Arithmetic)?;
        let sentinel_transitions = stride.checked_mul(3).ok_or(BoundsError::Arithmetic)?;
        Ok(Self {
            sparse_entries,
            start_entries,
            sentinel_transitions,
            epsilon_stack: shape.branch_frames.max(1),
            state_payload_bytes: state_payload_limit(shape.states, shape.patterns)?,
        })
    }

    pub(crate) fn fresh_reported_bytes(&self, word_bytes: usize) -> Result<usize, BoundsError> {
        if !matches!(word_bytes, 4 | 8) {
            return Err(BoundsError::Shape);
        }
        // Three State handles, one map handle/ID, and three reported 9-byte payloads.
        let ids = self
            .sparse_entries
            .checked_add(self.start_entries)
            .and_then(|count| count.checked_add(self.sentinel_transitions))
            .and_then(|count| count.checked_add(1))
            .ok_or(BoundsError::Arithmetic)?;
        let handles = word_bytes.checked_mul(8).ok_or(BoundsError::Arithmetic)?;
        ids.checked_mul(4)
            .and_then(|bytes| bytes.checked_add(handles))
            .and_then(|bytes| bytes.checked_add(27))
            .ok_or(BoundsError::Arithmetic)
    }
}

fn state_payload_limit(states: usize, patterns: usize) -> Result<usize, BoundsError> {
    let nfa_bytes = states.checked_mul(5).ok_or(BoundsError::Arithmetic)?;
    patterns
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(13))
        .and_then(|bytes| bytes.checked_add(nfa_bytes))
        .ok_or(BoundsError::Arithmetic)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HybridDirection {
    Forward,
    Reverse,
    ReverseSharedStarts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum QuitAdmissionError {
    UnicodeWordAssertion,
    ExplicitQuitByte,
}

impl fmt::Display for QuitAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnicodeWordAssertion => "unicode_word_assertion",
            Self::ExplicitQuitByte => "explicit_quit_byte",
        })
    }
}

pub(crate) fn require_no_quit(dfa: &DFA) -> Result<(), QuitAdmissionError> {
    if dfa.get_nfa().look_set_any().contains_word_unicode() {
        return Err(QuitAdmissionError::UnicodeWordAssertion);
    }
    if (0..=u8::MAX).any(|byte| dfa.get_config().get_quit(byte)) {
        return Err(QuitAdmissionError::ExplicitQuitByte);
    }
    Ok(())
}

pub(crate) fn candidate_hybrid(
    nfa: &NFA,
    direction: HybridDirection,
) -> Result<DFA, Box<BuildError>> {
    let forward = direction == HybridDirection::Forward;
    DFA::builder()
        .configure(
            Config::new()
                .match_kind(if forward {
                    MatchKind::LeftmostFirst
                } else {
                    MatchKind::All
                })
                .prefilter(None)
                .starts_for_each_pattern(direction != HybridDirection::ReverseSharedStarts)
                .byte_classes(true)
                .unicode_word_boundary(true)
                .specialize_start_states(false)
                .cache_capacity(0x0001_0000)
                .skip_cache_capacity_check(false)
                .minimum_cache_clear_count(Some(3))
                .minimum_bytes_per_state(Some(10)),
        )
        .build_from_nfa(nfa.clone())
        .map_err(Box::new)
}

#[cfg(test)]
mod tests {
    use super::{
        HybridDimensions, HybridDirection, QuitAdmissionError, candidate_hybrid, require_no_quit,
        state_payload_limit,
    };
    use crate::schema_cache_bounds::{BoundsError, NfaDimensions, forward_nfa, reverse_nfa};
    use regex_automata::{
        Anchored, Input, MatchErrorKind, MatchKind, PatternID,
        hybrid::{dfa::DFA, regex::Regex as HybridRegex},
        nfa::thompson::{Builder, NFA},
    };
    use std::fmt::Write as _;
    use std::mem::size_of;

    #[test]
    fn unicode_word_assertions_refuse_in_both_directions_but_ascii_ones_do_not() {
        for direction in [HybridDirection::Forward, HybridDirection::Reverse] {
            for (pattern, expected) in [
                (r"\bword", Err(QuitAdmissionError::UnicodeWordAssertion)),
                (r"\Bword", Err(QuitAdmissionError::UnicodeWordAssertion)),
                (r"(?-u:\b)word", Ok(())),
                (r"[\x{E9}a-z]+", Ok(())),
            ] {
                let nfa = if direction == HybridDirection::Forward {
                    forward_nfa(pattern).unwrap()
                } else {
                    reverse_nfa(pattern).unwrap()
                };
                let dfa = candidate_hybrid(&nfa, direction).unwrap();
                assert_eq!(require_no_quit(&dfa), expected);
            }
        }
    }

    #[test]
    fn every_configured_quit_byte_refuses_even_without_word_assertions() {
        let nfa = forward_nfa("a+").unwrap();
        let mut checked = 0_usize;
        for byte in 0..=u8::MAX {
            let dfa = DFA::builder()
                .configure(DFA::config().quit(byte, true))
                .build_from_nfa(nfa.clone())
                .unwrap();
            assert_eq!(
                require_no_quit(&dfa),
                Err(QuitAdmissionError::ExplicitQuitByte)
            );
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 256);
    }

    #[test]
    fn removing_the_only_explicit_quit_byte_restores_admission() {
        let dfa = DFA::builder()
            .configure(DFA::config().quit(b'x', true).quit(b'x', false))
            .build("a+")
            .unwrap();
        assert_eq!(require_no_quit(&dfa), Ok(()));
    }

    #[test]
    fn real_search_distinguishes_unicode_and_explicit_quitting_from_capacity_clears() {
        let unicode =
            candidate_hybrid(&forward_nfa(r"\bword").unwrap(), HybridDirection::Forward).unwrap();
        let explicit = DFA::builder()
            .configure(DFA::config().quit(b'x', true))
            .build("a+")
            .unwrap();
        for (dfa, text, byte) in [(unicode, "\u{e9}word", 0xC3), (explicit, "x", b'x')] {
            let mut cache = dfa.create_cache();
            let error = dfa
                .try_search_fwd(&mut cache, &Input::new(text))
                .unwrap_err();
            assert_eq!(error.kind(), &MatchErrorKind::Quit { byte, offset: 0 });
            assert_eq!(cache.clear_count(), 0);
        }
    }

    const SHAPE: NfaDimensions = NfaDimensions {
        states: 5,
        slots: 2,
        patterns: 1,
        branch_frames: 3,
        capture_states: 2,
    };

    #[test]
    fn hybrid_fixed_buffers_vary_each_dimension_independently() {
        for (shape, stride, per_pattern, sparse, starts, transitions, stack, payload) in [
            (SHAPE, 8, true, 20, 18, 24, 3, 42),
            (
                NfaDimensions { states: 7, ..SHAPE },
                8,
                true,
                28,
                18,
                24,
                3,
                52,
            ),
            (
                NfaDimensions {
                    patterns: 2,
                    ..SHAPE
                },
                8,
                true,
                20,
                24,
                24,
                3,
                46,
            ),
            (SHAPE, 16, true, 20, 18, 48, 3, 42),
            (SHAPE, 8, false, 20, 12, 24, 3, 42),
            (
                NfaDimensions {
                    branch_frames: 0,
                    ..SHAPE
                },
                8,
                true,
                20,
                18,
                24,
                1,
                42,
            ),
            (
                NfaDimensions {
                    capture_states: 10,
                    ..SHAPE
                },
                8,
                true,
                20,
                18,
                24,
                3,
                42,
            ),
        ] {
            assert_eq!(
                HybridDimensions::project(shape, stride, per_pattern),
                Ok(HybridDimensions {
                    sparse_entries: sparse,
                    start_entries: starts,
                    sentinel_transitions: transitions,
                    epsilon_stack: stack,
                    state_payload_bytes: payload,
                })
            );
        }
    }

    #[test]
    fn fresh_hybrid_report_distinguishes_target_handle_width_and_start_mode() {
        let per_pattern = HybridDimensions::project(SHAPE, 8, true).unwrap();
        assert_eq!(per_pattern.fresh_reported_bytes(4), Ok(311));
        assert_eq!(per_pattern.fresh_reported_bytes(8), Ok(343));
        let shared = HybridDimensions::project(SHAPE, 8, false).unwrap();
        assert_eq!(shared.fresh_reported_bytes(4), Ok(287));
        assert_eq!(shared.fresh_reported_bytes(8), Ok(319));
        for width in [0, 2, 16] {
            assert_eq!(
                per_pattern.fresh_reported_bytes(width),
                Err(BoundsError::Shape)
            );
        }
    }

    #[test]
    fn encoded_state_payload_has_full_header_pattern_count_and_varint_allowance() {
        for (states, patterns, expected) in
            [(0, 0, 13), (0, 1, 17), (1, 0, 18), (5, 1, 42), (5, 2, 46)]
        {
            assert_eq!(state_payload_limit(states, patterns), Ok(expected));
        }
        for (states, patterns) in [
            (usize::MAX, 0),
            (0, usize::MAX),
            (usize::MAX.checked_div(5).unwrap(), 1),
        ] {
            assert_eq!(
                state_payload_limit(states, patterns),
                Err(BoundsError::Arithmetic)
            );
        }
    }

    #[test]
    fn hybrid_projection_refuses_invalid_layouts_and_checked_overflow() {
        for (shape, stride) in [
            (NfaDimensions { states: 0, ..SHAPE }, 8),
            (SHAPE, 0),
            (SHAPE, 1),
            (SHAPE, 3),
            (SHAPE, 1024),
        ] {
            assert_eq!(
                HybridDimensions::project(shape, stride, true),
                Err(BoundsError::Shape)
            );
        }
        for shape in [
            NfaDimensions {
                states: usize::MAX,
                ..SHAPE
            },
            NfaDimensions {
                patterns: usize::MAX,
                ..SHAPE
            },
            NfaDimensions {
                patterns: usize::MAX.checked_div(6).unwrap(),
                ..SHAPE
            },
        ] {
            assert_eq!(
                HybridDimensions::project(shape, 8, true),
                Err(BoundsError::Arithmetic)
            );
        }
        let mut projected = HybridDimensions::project(SHAPE, 8, true).unwrap();
        projected.sparse_entries = usize::MAX;
        assert_eq!(
            projected.fresh_reported_bytes(8),
            Err(BoundsError::Arithmetic)
        );
    }

    #[test]
    fn hybrid_direction_settings_pin_limits_semantics_and_start_modes() {
        for (direction, expected_kind, per_pattern) in [
            (HybridDirection::Forward, MatchKind::LeftmostFirst, true),
            (HybridDirection::Reverse, MatchKind::All, true),
            (HybridDirection::ReverseSharedStarts, MatchKind::All, false),
        ] {
            let nfa = if direction == HybridDirection::Forward {
                forward_nfa("a+b").unwrap()
            } else {
                reverse_nfa("a+b").unwrap()
            };
            let dfa = candidate_hybrid(&nfa, direction).unwrap();
            let config = dfa.get_config();
            assert_eq!(config.get_match_kind(), expected_kind);
            assert_eq!(config.get_starts_for_each_pattern(), per_pattern);
            assert!(config.get_byte_classes());
            assert!(config.get_unicode_word_boundary());
            assert!(!config.get_specialize_start_states());
            assert!(config.get_prefilter().is_none());
            assert_eq!(config.get_cache_capacity(), 0x0001_0000);
            assert!(!config.get_skip_cache_capacity_check());
            assert_eq!(config.get_minimum_cache_clear_count(), Some(3));
            assert_eq!(config.get_minimum_bytes_per_state(), Some(10));
        }
    }

    #[test]
    fn each_direction_starts_with_its_projected_reported_layout() {
        for pattern in ["a", "^(a|ab|abc)+$", "a{2,8}b?", "\\b[a-z]+\\b"] {
            for direction in [
                HybridDirection::Forward,
                HybridDirection::Reverse,
                HybridDirection::ReverseSharedStarts,
            ] {
                let nfa = if direction == HybridDirection::Forward {
                    forward_nfa(pattern).unwrap()
                } else {
                    reverse_nfa(pattern).unwrap()
                };
                let dfa = candidate_hybrid(&nfa, direction).unwrap();
                let projected = HybridDimensions::from_dfa(&dfa).unwrap();
                let cache = dfa.create_cache();
                assert_eq!(cache.clear_count(), 0);
                assert_eq!(
                    cache.memory_usage(),
                    projected.fresh_reported_bytes(size_of::<usize>()).unwrap()
                );
            }
        }
    }

    #[test]
    fn reverse_nfa_drops_captures_but_keeps_pattern_ids_and_reversed_semantics() {
        let nfa = reverse_nfa("(a+)(b+)").unwrap();
        assert!(nfa.is_reverse());
        assert!(nfa.is_utf8());
        assert_eq!(nfa.pattern_len(), 1);
        let shape = NfaDimensions::from_nfa(&nfa).unwrap();
        assert_eq!((shape.slots, shape.capture_states), (0, 0));
        let dfa = candidate_hybrid(&nfa, HybridDirection::Reverse).unwrap();
        let mut cache = dfa.create_cache();
        assert_eq!(
            dfa.try_search_rev(&mut cache, &Input::new("!aaabbb?").span(1..7))
                .unwrap()
                .unwrap()
                .offset(),
            1
        );
        assert_eq!(
            dfa.try_search_rev(&mut cache, &Input::new("bbbbaaaa"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn hybrid_builder_refuses_below_its_independent_admission_minimum() {
        let nfa = forward_nfa("^(a|ab|abc)+$").unwrap();
        let config = candidate_hybrid(&nfa, HybridDirection::Forward)
            .unwrap()
            .get_config()
            .clone();
        let minimum = config.get_minimum_cache_capacity(&nfa).unwrap();
        assert!(minimum > 1);
        let too_small = DFA::builder()
            .configure(config.clone().cache_capacity(minimum - 1))
            .build_from_nfa(nfa.clone());
        assert!(
            too_small
                .unwrap_err()
                .to_string()
                .contains("cache capacity")
        );
        let at_minimum = DFA::builder()
            .configure(config.cache_capacity(minimum))
            .build_from_nfa(nfa)
            .unwrap();
        assert_eq!(at_minimum.get_config().get_cache_capacity(), minimum);
    }

    #[test]
    fn powerset_payload_projection_refuses_duplicate_pattern_matches() {
        let mut builder = Builder::new();
        builder.start_pattern().unwrap();
        let first = builder.add_match().unwrap();
        let second = builder.add_match().unwrap();
        let start = builder.add_union(vec![first, second]).unwrap();
        builder.finish_pattern(start).unwrap();
        let nfa = builder.build(start, start).unwrap();
        assert_eq!(nfa.pattern_len(), 1);
        let dfa = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        assert_eq!(HybridDimensions::from_dfa(&dfa), Err(BoundsError::Shape));
    }

    #[test]
    fn real_hybrid_growth_clear_failure_and_reset_preserve_a_usable_owner() {
        let nfa = forward_nfa("[01]*1[01]{16}$").unwrap();
        let base = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        let mut full_cache = base.create_cache();
        let matched = base
            .try_search_fwd(
                &mut full_cache,
                &Input::new("11111111111111111").earliest(true),
            )
            .unwrap()
            .unwrap();
        assert_eq!(matched.offset(), 17);
        let minimum = base.get_config().get_minimum_cache_capacity(&nfa).unwrap();
        let dfa = DFA::builder()
            .configure(base.get_config().clone().cache_capacity(minimum))
            .build_from_nfa(nfa)
            .unwrap();
        let mut cache = dfa.create_cache();
        let initial = cache.memory_usage();
        let mut input = String::new();
        for value in 0_u16..256 {
            write!(input, "{value:08b}").unwrap();
        }
        let error = dfa
            .try_search_fwd(&mut cache, &Input::new(&input).earliest(true))
            .unwrap_err();
        assert!(matches!(error.kind(), MatchErrorKind::GaveUp { .. }));
        assert!(cache.clear_count() >= 3);
        assert!(cache.memory_usage() > initial);
        cache.reset(&dfa);
        assert_eq!(cache.clear_count(), 0);
        assert!(cache.memory_usage() > initial);
        assert_eq!(
            dfa.try_search_fwd(&mut cache, &Input::new("x").earliest(true))
                .unwrap(),
            None
        );
    }

    #[test]
    fn hybrid_pair_eagerly_owns_reverse_storage_before_a_forward_only_search() {
        let forward =
            candidate_hybrid(&forward_nfa("(a+)(b+)").unwrap(), HybridDirection::Forward).unwrap();
        let reverse =
            candidate_hybrid(&reverse_nfa("(a+)(b+)").unwrap(), HybridDirection::Reverse).unwrap();
        let pair = HybridRegex::builder().build_from_dfas(forward, reverse);
        let mut cache = pair.create_cache();
        let before_forward = cache.as_parts().0.memory_usage();
        let before_reverse = cache.as_parts().1.memory_usage();
        assert!(before_forward > 0 && before_reverse > 0);
        assert_eq!(cache.memory_usage(), before_forward + before_reverse);
        let input = Input::new("!aaabbb?");
        let found = pair
            .forward()
            .try_search_fwd(cache.as_parts_mut().0, &input.clone().earliest(true))
            .unwrap()
            .unwrap();
        assert_eq!(found.offset(), 5);
        assert!(cache.as_parts().0.memory_usage() > before_forward);
        assert_eq!(cache.as_parts().1.memory_usage(), before_reverse);
        let full = pair.try_search(&mut cache, &input).unwrap().unwrap();
        assert_eq!(full.range(), 1..7);
        assert!(cache.as_parts().1.memory_usage() > before_reverse);
        assert_eq!(
            cache.memory_usage(),
            cache.as_parts().0.memory_usage() + cache.as_parts().1.memory_usage()
        );
    }

    #[test]
    fn multi_pattern_hybrid_start_table_keeps_each_patterns_anchored_entry() {
        let nfa = NFA::new_many(&["a+", "b+"]).unwrap();
        let dfa = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        let projected = HybridDimensions::from_dfa(&dfa).unwrap();
        assert_eq!(projected.start_entries, 24);
        let mut cache = dfa.create_cache();
        assert_eq!(
            cache.memory_usage(),
            projected.fresh_reported_bytes(size_of::<usize>()).unwrap()
        );
        for (input, pattern, expected) in
            [("aaa", 0, Some(0)), ("bbb", 1, Some(1)), ("aaa", 1, None)]
        {
            let found = dfa
                .try_search_fwd(
                    &mut cache,
                    &Input::new(input).anchored(Anchored::Pattern(PatternID::must(pattern))),
                )
                .unwrap();
            assert_eq!(found.map(|matched| matched.pattern().as_usize()), expected);
        }
    }
}

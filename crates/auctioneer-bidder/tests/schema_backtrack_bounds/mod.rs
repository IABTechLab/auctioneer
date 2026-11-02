//! Logical bounded-backtracker lengths, not retained capacities or byte bounds.

use crate::schema_cache_bounds::{BoundsError, NfaDimensions};
use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BacktrackDimensions {
    pub visited_words: usize,
    pub stack_frames: usize,
}

impl fmt::Display for BacktrackDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "visited-words={} stack-frames={}",
            self.visited_words, self.stack_frames
        )
    }
}

impl BacktrackDimensions {
    /// `None` means ineligible, not that a reusable cache has no retained state.
    pub(crate) fn for_earliest(
        shape: NfaDimensions,
        haystack_bytes: usize,
        span_bytes: usize,
        word_bits: usize,
    ) -> Result<Option<BacktrackDimensions>, BoundsError> {
        if shape.states == 0 || span_bytes > haystack_bytes || !matches!(word_bits, 32 | 64) {
            return Err(BoundsError::Shape);
        }
        // meta::wrappers::BoundedBacktracker::get uses the full haystack here.
        if haystack_bytes > 128 {
            return Ok(None);
        }
        let offsets = span_bytes.checked_add(1).ok_or(BoundsError::Arithmetic)?;
        let visited_bits = shape
            .states
            .checked_mul(offsets)
            .ok_or(BoundsError::Arithmetic)?;
        // Default visited_capacity is 262,144 bytes; divisible by both word sizes.
        if visited_bits > 0x0020_0000 {
            return Ok(None);
        }
        let visited_words = visited_bits.div_ceil(word_bits);
        let stack_frames = shape
            .branch_frames
            .checked_add(shape.capture_states)
            .and_then(|frames| frames.checked_mul(offsets))
            .ok_or(BoundsError::Arithmetic)?
            .max(1);
        Ok(Some(BacktrackDimensions {
            visited_words,
            stack_frames,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::BacktrackDimensions;
    use crate::schema_cache_bounds::forward_nfa;
    use crate::schema_cache_bounds::{BoundsError, NfaDimensions};
    use regex_automata::{
        Input, MatchError,
        nfa::thompson::backtrack::{BoundedBacktracker, Config, min_visited_capacity},
    };
    use std::mem::size_of;

    const SHAPE: NfaDimensions = NfaDimensions {
        states: 5,
        slots: 7,
        patterns: 1,
        branch_frames: 3,
        capture_states: 2,
    };

    #[test]
    fn backtracker_bitmap_rounding_and_stack_depend_on_independent_dimensions() {
        for (shape, span, words32, words64, frames) in [
            (SHAPE, 0, 1, 1, 5),
            (SHAPE, 1, 1, 1, 10),
            (SHAPE, 12, 3, 2, 65),
            (SHAPE, 128, 21, 11, 645),
            (NfaDimensions { states: 7, ..SHAPE }, 12, 3, 2, 65),
            (NfaDimensions { states: 9, ..SHAPE }, 12, 4, 2, 65),
            (
                NfaDimensions {
                    states: 11,
                    ..SHAPE
                },
                12,
                5,
                3,
                65,
            ),
            (
                NfaDimensions {
                    branch_frames: 4,
                    ..SHAPE
                },
                12,
                3,
                2,
                78,
            ),
            (
                NfaDimensions {
                    capture_states: 4,
                    ..SHAPE
                },
                12,
                3,
                2,
                91,
            ),
            (
                NfaDimensions {
                    branch_frames: 0,
                    capture_states: 0,
                    ..SHAPE
                },
                12,
                3,
                2,
                1,
            ),
        ] {
            for (word_bits, visited_words) in [(32, words32), (64, words64)] {
                assert_eq!(
                    BacktrackDimensions::for_earliest(shape, span, span, word_bits),
                    Ok(Some(BacktrackDimensions {
                        visited_words,
                        stack_frames: frames
                    }))
                );
            }
        }
    }

    #[test]
    fn bitmap_words_round_up_on_each_target_word_boundary() {
        let shape = NfaDimensions { states: 1, ..SHAPE };
        for (span, words32, words64) in [
            (30, 1, 1),
            (31, 1, 1),
            (32, 2, 1),
            (62, 2, 1),
            (63, 2, 1),
            (64, 3, 2),
        ] {
            for (word_bits, expected) in [(32, words32), (64, words64)] {
                assert_eq!(
                    BacktrackDimensions::for_earliest(shape, span, span, word_bits)
                        .unwrap()
                        .unwrap()
                        .visited_words,
                    expected
                );
            }
        }
    }

    #[test]
    fn earliest_backtracker_gate_uses_the_full_haystack_not_just_the_span() {
        assert_eq!(
            BacktrackDimensions::for_earliest(SHAPE, 128, 12, 64),
            Ok(Some(BacktrackDimensions {
                visited_words: 2,
                stack_frames: 65
            }))
        );
        for span in [0, 12, 128, 129] {
            assert_eq!(
                BacktrackDimensions::for_earliest(SHAPE, 129, span, 64),
                Ok(None)
            );
        }
    }

    #[test]
    fn backtracker_visited_limit_is_independent_of_the_earliest_length_gate() {
        let shape = NfaDimensions {
            states: 0x4000,
            ..SHAPE
        };
        for (word_bits, visited_words) in [(32, 0x0001_0000), (64, 0x8000)] {
            assert_eq!(
                BacktrackDimensions::for_earliest(shape, 128, 127, word_bits),
                Ok(Some(BacktrackDimensions {
                    visited_words,
                    stack_frames: 640
                }))
            );
            assert_eq!(
                BacktrackDimensions::for_earliest(shape, 128, 128, word_bits),
                Ok(None)
            );
        }
    }

    #[test]
    fn backtracker_projection_refuses_invalid_shapes_and_checked_overflow() {
        for (shape, haystack, span, word_bits, expected) in [
            (SHAPE, 0, 1, 64, BoundsError::Shape),
            (SHAPE, 1, 1, 0, BoundsError::Shape),
            (SHAPE, 1, 1, 16, BoundsError::Shape),
            (
                NfaDimensions { states: 0, ..SHAPE },
                0,
                0,
                32,
                BoundsError::Shape,
            ),
            (
                NfaDimensions {
                    states: usize::MAX,
                    ..SHAPE
                },
                1,
                1,
                64,
                BoundsError::Arithmetic,
            ),
            (
                NfaDimensions {
                    branch_frames: usize::MAX,
                    ..SHAPE
                },
                0,
                0,
                64,
                BoundsError::Arithmetic,
            ),
            (
                NfaDimensions {
                    branch_frames: usize::MAX >> 1_usize,
                    capture_states: 0,
                    ..SHAPE
                },
                2,
                2,
                64,
                BoundsError::Arithmetic,
            ),
        ] {
            assert_eq!(
                BacktrackDimensions::for_earliest(shape, haystack, span, word_bits),
                Err(expected)
            );
        }
    }

    #[test]
    fn drained_real_backtracker_reports_bitmap_lengths_not_retained_capacity() {
        let native_word_bits = usize::try_from(usize::BITS).unwrap();
        let nfa = forward_nfa("^(a|ab|abc)+$").unwrap();
        let shape = NfaDimensions::from_nfa(&nfa).unwrap();
        let engine = BoundedBacktracker::builder().build_from_nfa(nfa).unwrap();
        assert_eq!(engine.get_config().get_visited_capacity(), 0x0004_0000);
        let mut cache = engine.create_cache();
        let mut captures = engine.create_captures();
        assert_eq!(cache.memory_usage(), 0);
        for length in [0, 1, 31, 32, 63, 64, 127, 128, 1] {
            let input = "x".repeat(length);
            let projected =
                BacktrackDimensions::for_earliest(shape, length, length, native_word_bits)
                    .unwrap()
                    .unwrap();
            engine
                .try_search(
                    &mut cache,
                    &Input::new(&input).earliest(true),
                    &mut captures,
                )
                .unwrap();
            assert!(!captures.is_match());
            assert_eq!(
                cache.memory_usage(),
                projected.visited_words * size_of::<usize>()
            );
        }
        // A successful search can leave restore/alternate frames; the next search clears them.
        engine
            .try_search(
                &mut cache,
                &Input::new("ababc").earliest(true),
                &mut captures,
            )
            .unwrap();
        assert!(captures.is_match());
        let successful = BacktrackDimensions::for_earliest(shape, 5, 5, native_word_bits)
            .unwrap()
            .unwrap();
        assert!(cache.memory_usage() > successful.visited_words * size_of::<usize>());
        engine
            .try_search(&mut cache, &Input::new("x").earliest(true), &mut captures)
            .unwrap();
        assert!(!captures.is_match());
        let restarted = BacktrackDimensions::for_earliest(shape, 1, 1, native_word_bits)
            .unwrap()
            .unwrap();
        assert_eq!(
            cache.memory_usage(),
            restarted.visited_words * size_of::<usize>()
        );
        let haystack = "x".repeat(128);
        for (start, end) in [(0, 0), (1, 64), (64, 65), (127, 128), (128, 128)] {
            let input = Input::new(&haystack).span(start..end).earliest(true);
            engine
                .try_search(&mut cache, &input, &mut captures)
                .unwrap();
            assert!(!captures.is_match());
            let projected =
                BacktrackDimensions::for_earliest(shape, 128, end - start, native_word_bits)
                    .unwrap()
                    .unwrap();
            assert_eq!(
                cache.memory_usage(),
                projected.visited_words * size_of::<usize>()
            );
        }
    }

    #[test]
    fn failed_unanchored_capture_loops_drain_the_real_backtracker_stack() {
        for pattern in ["(a|ab|abc)+z", "((a){3})+z"] {
            let nfa = forward_nfa(pattern).unwrap();
            let shape = NfaDimensions::from_nfa(&nfa).unwrap();
            let engine = BoundedBacktracker::builder().build_from_nfa(nfa).unwrap();
            let mut cache = engine.create_cache();
            let mut captures = engine.create_captures();
            let input = "a".repeat(128);
            engine
                .try_search(
                    &mut cache,
                    &Input::new(&input).earliest(true),
                    &mut captures,
                )
                .unwrap();
            assert!(!captures.is_match());
            let projected = BacktrackDimensions::for_earliest(
                shape,
                128,
                128,
                usize::try_from(usize::BITS).unwrap(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                cache.memory_usage(),
                projected.visited_words * size_of::<usize>()
            );
        }
    }

    #[test]
    fn real_backtracker_rejects_one_byte_below_its_required_visited_budget() {
        let nfa = forward_nfa("^(a|ab|abc)+$").unwrap();
        let haystack = "x".repeat(128);
        let input = Input::new(&haystack).earliest(true);
        let required = min_visited_capacity(&nfa, &input);
        assert!(required > 1);
        for (capacity, accepted) in [(required - 1, false), (required, true)] {
            let engine = BoundedBacktracker::builder()
                .configure(Config::new().visited_capacity(capacity))
                .build_from_nfa(nfa.clone())
                .unwrap();
            let mut cache = engine.create_cache();
            let mut captures = engine.create_captures();
            let result = engine.try_search(&mut cache, &input, &mut captures);
            if accepted {
                assert_eq!(result, Ok(()));
                assert!(!captures.is_match());
                assert!(cache.memory_usage() > 0);
            } else {
                assert_eq!(result, Err(MatchError::haystack_too_long(128)));
                assert_eq!(cache.memory_usage(), 0);
            }
        }
    }
}

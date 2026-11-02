//! Fresh Core capture/one-pass requested heap, not lazy fallback or whole-meta storage.

use crate::schema_cache_bounds::BoundsError;
use crate::schema_heap_layout::{HeapLayout, LayoutError};
use crate::schema_onepass_bounds::explicit_scratch_slots;
use regex_automata::nfa::thompson::NFA;
use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CoreEagerHeap {
    pub captures: usize,
    pub onepass: usize,
    pub total: usize,
}

impl fmt::Display for CoreEagerHeap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "capture-bytes={} onepass-bytes={} total-bytes={}",
            self.captures, self.onepass, self.total
        )
    }
}

impl CoreEagerHeap {
    pub(crate) fn from_nfa(nfa: &NFA, layout: HeapLayout) -> Result<Self, BoundsError> {
        Self::project(nfa.group_info().slot_len(), nfa.pattern_len(), layout)
    }

    fn project(
        total_slots: usize,
        patterns: usize,
        layout: HeapLayout,
    ) -> Result<Self, BoundsError> {
        let slots = explicit_scratch_slots(total_slots, patterns)?;
        let map_error = |error| match error {
            LayoutError::Shape => BoundsError::Shape,
            LayoutError::Arithmetic => BoundsError::Arithmetic,
        };
        let capture_bytes = layout
            .repeat_vector(layout.word_bytes(), total_slots)
            .map_err(map_error)?;
        // A fitting candidate may be absent. Funding it anyway does not
        // require observing meta's private construction/selection heuristic.
        let onepass_bytes = layout
            .fresh_resize_vector(layout.word_bytes(), slots.unwrap_or(0))
            .map_err(map_error)?;
        let total_bytes = capture_bytes
            .checked_add(onepass_bytes)
            .ok_or(BoundsError::Arithmetic)?;
        Ok(Self {
            captures: capture_bytes,
            onepass: onepass_bytes,
            total: total_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundsError, CoreEagerHeap, HeapLayout};
    use crate::schema_cache_bounds::forward_nfa;
    use regex_automata::nfa::thompson::NFA;

    #[test]
    fn fresh_core_auxiliary_owners_vary_slots_patterns_and_width_independently() {
        for (slots, patterns, word, captures, onepass, total) in [
            (0, 0, 8, 0, 0, 0),
            (2, 1, 8, 16, 0, 16),
            (4, 1, 8, 32, 32, 64),
            (6, 1, 8, 48, 32, 80),
            (6, 2, 8, 48, 32, 80),
            (6, 3, 8, 48, 0, 48),
            (4, 1, 4, 16, 16, 32),
            (10, 2, 8, 80, 48, 128),
            (10, 2, 4, 40, 24, 64),
            (12, 1, 8, 96, 80, 176),
            (12, 2, 8, 96, 64, 160),
            (12, 1, 4, 48, 40, 88),
            (34, 1, 8, 272, 256, 528),
            (36, 1, 8, 288, 0, 288),
            (36, 2, 8, 288, 256, 544),
        ] {
            assert_eq!(
                CoreEagerHeap::project(slots, patterns, HeapLayout::new(word, 16).unwrap()),
                Ok(CoreEagerHeap {
                    captures,
                    onepass,
                    total
                })
            );
        }
    }

    #[test]
    fn fresh_core_auxiliary_projection_refuses_malformed_and_overflowing_shapes() {
        let layout = HeapLayout::new(8, 16).unwrap();
        for (slots, patterns) in [(1, 0), (3, 1), (0, 1), (2, 2)] {
            assert_eq!(
                CoreEagerHeap::project(slots, patterns, layout),
                Err(BoundsError::Shape)
            );
        }
        assert_eq!(
            CoreEagerHeap::project(2, usize::MAX, layout),
            Err(BoundsError::Arithmetic)
        );
        assert_eq!(
            CoreEagerHeap::project(usize::MAX - 1, 1, layout),
            Err(BoundsError::Arithmetic)
        );
        assert_eq!(
            CoreEagerHeap::project(0x2000_0000, 1, HeapLayout::new(4, 16).unwrap()),
            Err(BoundsError::Arithmetic)
        );
        assert_eq!(
            CoreEagerHeap::project(0x1FFF_FFFE, 1, HeapLayout::new(4, 16).unwrap()),
            Ok(CoreEagerHeap {
                captures: 0x7FFF_FFF8,
                onepass: 0,
                total: 0x7FFF_FFF8
            })
        );
    }

    #[test]
    fn fresh_core_auxiliary_constructor_binds_both_counts_to_the_same_nfa() {
        for (pattern, captures, onepass, total) in [
            ("^a+$", 16, 0, 16),
            ("^(a+)$", 32, 32, 64),
            ("^(a+)(b+)$", 48, 32, 80),
        ] {
            let nfa = forward_nfa(pattern).unwrap();
            assert_eq!(
                CoreEagerHeap::from_nfa(&nfa, HeapLayout::new(8, 16).unwrap()),
                Ok(CoreEagerHeap {
                    captures,
                    onepass,
                    total
                })
            );
        }
        let nfa = NFA::new_many(&["^(a+)$", "^(b+)(c+)$"]).unwrap();
        assert_eq!(nfa.pattern_len(), 2);
        assert_eq!(nfa.group_info().slot_len(), 10);
        assert_eq!(
            CoreEagerHeap::from_nfa(&nfa, HeapLayout::new(8, 16).unwrap()),
            Ok(CoreEagerHeap {
                captures: 80,
                onepass: 48,
                total: 128,
            })
        );
    }
}

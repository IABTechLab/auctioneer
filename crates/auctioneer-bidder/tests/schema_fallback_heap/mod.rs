//! Retained lazy fallback requested heap; excludes eager/shared/hybrid owners.

use crate::schema_backtrack_bounds::BacktrackDimensions;
use crate::schema_cache_bounds::{BoundsError, NfaDimensions};
use crate::schema_heap_layout::{AllocationEnvelope, HeapLayout, LayoutError};
use regex_automata::nfa::thompson::NFA;
use std::fmt;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FallbackHeap {
    pub pike_sets: usize,
    pub pike_slots: usize,
    pub pike_stack: usize,
    pub backtrack_bitmap: usize,
    pub backtrack_stack: usize,
    pub backtrack_max_span: Option<usize>,
    pub live_envelope: usize,
    pub growth_overlap: usize,
    pub peak: usize,
}

impl fmt::Display for FallbackHeap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "pike-sets={} pike-slots={} pike-stack={} backtrack-bitmap={} backtrack-stack={} live={} overlap={} peak={}",
            self.pike_sets,
            self.pike_slots,
            self.pike_stack,
            self.backtrack_bitmap,
            self.backtrack_stack,
            self.live_envelope,
            self.growth_overlap,
            self.peak
        )
    }
}

impl FallbackHeap {
    /// Conditional layout bound for one fixed cache's entire search lifetime.
    /// `max_input_bytes` bounds every full haystack, not only the latest span.
    /// Only meta's `earliest(true)` path supplies the 128-byte admission gate.
    pub(crate) fn from_nfa(
        nfa: &NFA,
        layout: HeapLayout,
        max_input_bytes: usize,
    ) -> Result<Self, BoundsError> {
        Self::project(NfaDimensions::from_nfa(nfa)?, layout, max_input_bytes)
    }

    fn project(
        shape: NfaDimensions,
        layout: HeapLayout,
        max_input_bytes: usize,
    ) -> Result<Self, BoundsError> {
        if shape.states == 0 {
            return Err(BoundsError::Shape);
        }
        let pike = shape.pike_storage()?;
        let sets = layout
            .fresh_resize_vector(4, shape.states)
            .map_err(layout_error)?
            .checked_mul(4)
            .ok_or(BoundsError::Arithmetic)?;
        let tables = layout
            .fresh_resize_vector(
                layout.word_bytes(),
                pike.table_entries
                    .checked_div(2)
                    .ok_or(BoundsError::Arithmetic)?,
            )
            .map_err(layout_error)?
            .checked_mul(2)
            .ok_or(BoundsError::Arithmetic)?;
        // Both private frame enums fit a tag, u32 index and word-sized
        // offset. Three words conservatively cover padding on either width.
        let frame_bytes = layout
            .word_bytes()
            .checked_mul(3)
            .ok_or(BoundsError::Arithmetic)?;
        let pike_stack = layout
            .append_vector(frame_bytes, pike.stack_frames)
            .map_err(layout_error)?;
        // Default backtracker bitmap capacity is 262,144 bytes. A wrapper
        // rejects earliest searches over 128 full-haystack bytes before use.
        let max_offsets = 0x0020_0000_usize
            .checked_div(shape.states)
            .ok_or(BoundsError::Shape)?;
        let backtrack_max_span = max_offsets
            .checked_sub(1)
            .map(|span| span.min(128).min(max_input_bytes));
        let (bitmap, backtrack_stack) = match backtrack_max_span {
            None => (empty(), empty()),
            Some(span) => {
                let bits = layout
                    .word_bytes()
                    .checked_mul(8)
                    .ok_or(BoundsError::Arithmetic)?;
                let storage = BacktrackDimensions::for_earliest(shape, span, span, bits)?
                    .ok_or(BoundsError::Shape)?;
                (
                    layout
                        .append_vector(layout.word_bytes(), storage.visited_words)
                        .map_err(layout_error)?,
                    layout
                        .append_vector(frame_bytes, storage.stack_frames)
                        .map_err(layout_error)?,
                )
            }
        };
        let live_envelope = [
            sets,
            tables,
            pike_stack.retained_bytes,
            bitmap.retained_bytes,
            backtrack_stack.retained_bytes,
        ]
        .into_iter()
        .try_fold(0_usize, |total, bytes| {
            total.checked_add(bytes).ok_or(BoundsError::Arithmetic)
        })?;
        // Fallback caches can coexist after different searches. Only one
        // collection can be growing at a time on this serial cache history.
        let growth_overlap = [pike_stack, bitmap, backtrack_stack]
            .into_iter()
            .map(|owner| owner.old_growth_bytes)
            .fold(0, usize::max);
        let peak = live_envelope
            .checked_add(growth_overlap)
            .ok_or(BoundsError::Arithmetic)?;
        Ok(Self {
            pike_sets: sets,
            pike_slots: tables,
            pike_stack: pike_stack.retained_bytes,
            backtrack_bitmap: bitmap.retained_bytes,
            backtrack_stack: backtrack_stack.retained_bytes,
            backtrack_max_span,
            live_envelope,
            growth_overlap,
            peak,
        })
    }
}

const fn empty() -> AllocationEnvelope {
    AllocationEnvelope {
        retained_bytes: 0,
        old_growth_bytes: 0,
    }
}

const fn layout_error(error: LayoutError) -> BoundsError {
    match error {
        LayoutError::Shape => BoundsError::Shape,
        LayoutError::Arithmetic => BoundsError::Arithmetic,
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundsError, FallbackHeap, HeapLayout, NfaDimensions};
    use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};

    const SHAPE: NfaDimensions = NfaDimensions {
        states: 5,
        slots: 2,
        patterns: 1,
        branch_frames: 3,
        capture_states: 2,
    };

    #[test]
    fn fallback_heap_keeps_all_owners_and_only_one_serial_growth_overlap() {
        for (word, sets, slots, pike, bitmap, backtrack, live, overlap, peak) in [
            (4, 80, 96, 120, 168, 15480, 15944, 7728, 23672),
            (8, 80, 192, 240, 176, 30960, 31648, 15456, 47104),
        ] {
            assert_eq!(
                FallbackHeap::project(SHAPE, HeapLayout::new(word, 16).unwrap(), 0x0001_0000),
                Ok(FallbackHeap {
                    pike_sets: sets,
                    pike_slots: slots,
                    pike_stack: pike,
                    backtrack_bitmap: bitmap,
                    backtrack_stack: backtrack,
                    backtrack_max_span: Some(128),
                    live_envelope: live,
                    growth_overlap: overlap,
                    peak,
                })
            );
        }
    }

    #[test]
    fn backtracker_bound_honors_input_and_visited_limits_independently() {
        let layout = HeapLayout::new(8, 16).unwrap();
        for (input, span, bitmap, stack) in [
            (0, 0, 32, 240),
            (1, 1, 32, 480),
            (12, 12, 32, 3120),
            (127, 127, 160, 30720),
            (128, 128, 176, 30960),
            (129, 128, 176, 30960),
        ] {
            let heap = FallbackHeap::project(SHAPE, layout, input).unwrap();
            assert_eq!(heap.backtrack_max_span, Some(span));
            assert_eq!(heap.backtrack_bitmap, bitmap);
            assert_eq!(heap.backtrack_stack, stack);
        }
        for (states, span) in [(0x4000, 127), (0x0020_0000, 0)] {
            let heap =
                FallbackHeap::project(NfaDimensions { states, ..SHAPE }, layout, 128).unwrap();
            assert_eq!(heap.backtrack_max_span, Some(span));
            assert_eq!(heap.backtrack_bitmap, 0x0008_0000);
        }
        let heap = FallbackHeap::project(
            NfaDimensions {
                states: 0x0020_0001,
                ..SHAPE
            },
            layout,
            128,
        )
        .unwrap();
        assert_eq!(heap.backtrack_max_span, None);
        assert_eq!((heap.backtrack_bitmap, heap.backtrack_stack), (0, 0));
    }

    #[test]
    fn pike_resize_minima_apply_to_each_allocation_not_the_combined_length() {
        for (states, slots, patterns, sets, tables) in [
            (1, 0, 0, 64, 0),
            (1, 0, 1, 64, 64),
            (1, 2, 1, 64, 64),
            (2, 2, 1, 64, 96),
            (5, 2, 1, 80, 192),
            (5, 4, 1, 80, 384),
            (5, 2, 3, 80, 256),
        ] {
            let heap = FallbackHeap::project(
                NfaDimensions {
                    states,
                    slots,
                    patterns,
                    ..SHAPE
                },
                HeapLayout::new(8, 16).unwrap(),
                0,
            )
            .unwrap();
            assert_eq!((heap.pike_sets, heap.pike_slots), (sets, tables));
        }
    }

    #[test]
    fn branches_and_capture_frames_contribute_independently() {
        for (branches, captures, pike, backtrack) in [
            (0, 0, 96, 96),
            (3, 0, 144, 1872),
            (0, 2, 96, 1248),
            (3, 2, 240, 3120),
            (4, 2, 288, 3744),
            (3, 3, 288, 3744),
        ] {
            let heap = FallbackHeap::project(
                NfaDimensions {
                    branch_frames: branches,
                    capture_states: captures,
                    ..SHAPE
                },
                HeapLayout::new(8, 16).unwrap(),
                12,
            )
            .unwrap();
            assert_eq!((heap.pike_stack, heap.backtrack_stack), (pike, backtrack));
        }
    }

    #[test]
    fn fallback_projection_refuses_shape_and_arithmetic_failures() {
        let layout = HeapLayout::new(8, 16).unwrap();
        assert_eq!(
            FallbackHeap::project(NfaDimensions { states: 0, ..SHAPE }, layout, 128),
            Err(BoundsError::Shape)
        );
        for shape in [
            NfaDimensions {
                states: usize::MAX,
                ..SHAPE
            },
            NfaDimensions {
                slots: usize::MAX,
                ..SHAPE
            },
            NfaDimensions {
                patterns: usize::MAX,
                ..SHAPE
            },
            NfaDimensions {
                branch_frames: usize::MAX,
                ..SHAPE
            },
            NfaDimensions {
                capture_states: usize::MAX,
                ..SHAPE
            },
        ] {
            assert_eq!(
                FallbackHeap::project(shape, layout, 128),
                Err(BoundsError::Arithmetic)
            );
        }
        assert_eq!(
            FallbackHeap::project(SHAPE, layout, usize::MAX)
                .unwrap()
                .backtrack_max_span,
            Some(128)
        );
    }

    #[test]
    fn fallback_constructor_binds_dimensions_to_the_same_nfa() {
        for (nfa, slots) in [
            (forward_nfa("^(a+)(b+)$").unwrap(), 6),
            (reverse_nfa("^(a+)(b+)$").unwrap(), 0),
        ] {
            let shape = NfaDimensions::from_nfa(&nfa).unwrap();
            assert_eq!((shape.slots, shape.patterns), (slots, 1));
            for word in [4, 8] {
                let layout = HeapLayout::new(word, 16).unwrap();
                assert_eq!(
                    FallbackHeap::from_nfa(&nfa, layout, 128).unwrap(),
                    FallbackHeap::project(shape, layout, 128).unwrap()
                );
            }
        }
    }
}

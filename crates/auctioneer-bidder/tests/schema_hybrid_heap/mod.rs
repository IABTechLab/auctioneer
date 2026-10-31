//! Conditional requested-heap envelope for one fixed, serial hybrid cache.

use crate::schema_cache_bounds::BoundsError;
use crate::schema_heap_layout::{HeapLayout, LayoutError};
use crate::schema_hybrid_bounds::HybridDimensions;
use crate::schema_hybrid_graph::{GraphError, QUALIFICATION_BUDGET, certify};
use crate::schema_hybrid_population::HybridPopulation;
use regex_automata::hybrid::dfa::DFA;
use std::fmt;
use std::mem::size_of;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HybridHeap {
    pub live_envelope: usize,
    pub growth_overlap: usize,
    pub peak: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CompleteHeapError {
    Projection(BoundsError),
    Exploration(GraphError),
}

impl From<BoundsError> for CompleteHeapError {
    fn from(error: BoundsError) -> Self {
        Self::Projection(error)
    }
}

impl From<GraphError> for CompleteHeapError {
    fn from(error: GraphError) -> Self {
        Self::Exploration(error)
    }
}

impl fmt::Display for HybridHeap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "live-bytes={} growth-overlap-bytes={} peak-bytes={}",
            self.live_envelope, self.growth_overlap, self.peak
        )
    }
}

impl From<LayoutError> for BoundsError {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Shape => Self::Shape,
            LayoutError::Arithmetic => Self::Arithmetic,
        }
    }
}

impl HybridHeap {
    /// Fresh construction only, without search, reset, or clone history.
    pub(crate) fn from_fresh_dfa(dfa: &DFA, layout: HeapLayout) -> Result<Self, BoundsError> {
        Self::project_fresh(&HybridDimensions::from_dfa(dfa)?, layout)
    }

    fn project_fresh(storage: &HybridDimensions, layout: HeapLayout) -> Result<Self, BoundsError> {
        if storage.sparse_entries == 0
            || !storage.sparse_entries.is_multiple_of(4)
            || storage.start_entries < 12
            || !storage.start_entries.is_multiple_of(6)
            || !storage.sentinel_transitions.is_multiple_of(3)
        {
            return Err(BoundsError::Shape);
        }
        let stride = storage
            .sentinel_transitions
            .checked_div(3)
            .ok_or(BoundsError::Arithmetic)?;
        if !stride.is_power_of_two() || !(2..=512).contains(&stride) {
            return Err(BoundsError::Shape);
        }
        // Four independently resized sparse vectors: max(sparse_entries, 16) * 4.
        let sparse = layout
            .fresh_resize_vector(
                4,
                storage
                    .sparse_entries
                    .checked_div(4)
                    .ok_or(BoundsError::Arithmetic)?,
            )?
            .checked_mul(4)
            .ok_or(BoundsError::Arithmetic)?;
        let starts = layout.fresh_resize_vector(4, storage.start_entries)?;
        let transitions = layout.append_vector(4, storage.sentinel_transitions)?;
        let handles = layout.append_vector(
            layout
                .word_bytes()
                .checked_mul(2)
                .ok_or(BoundsError::Arithmetic)?,
            3,
        )?;
        let map = layout.state_map(1)?;
        // init_cache's three sentinels and map all share State::dead's backing.
        let dead = layout.arc_slice(9)?;
        let temporary = layout.fresh_resize_vector(1, 9)?;
        // Cache::new leaves search stack/builder empty. Keep the dead builder's
        // already freed temporary funded conservatively in the live envelope.
        let live_envelope = [
            sparse,
            starts,
            transitions.retained_bytes,
            handles.retained_bytes,
            map.retained_bytes,
            dead,
            temporary,
        ]
        .into_iter()
        .try_fold(0_usize, |total, bytes| {
            total.checked_add(bytes).ok_or(BoundsError::Arithmetic)
        })?;
        let growth_overlap = [
            transitions.old_growth_bytes,
            handles.old_growth_bytes,
            map.old_growth_bytes,
        ]
        .into_iter()
        .fold(0, usize::max);
        let peak = live_envelope
            .checked_add(growth_overlap)
            .ok_or(BoundsError::Arithmetic)?;
        Ok(Self {
            live_envelope,
            growth_overlap,
            peak,
        })
    }

    /// Internally binds the same fixed DFA's graph to its conditional heap model.
    pub(crate) fn from_complete_dfa(dfa: &DFA) -> Result<Option<Self>, CompleteHeapError> {
        let Some(coarse) = HybridPopulation::from_dfa(dfa)? else {
            return Ok(None);
        };
        let storage = HybridDimensions::from_dfa(dfa)?;
        let graph = certify(dfa, QUALIFICATION_BUDGET)?;
        let population = Self::closed_population(&storage, &coarse, graph.ordinary_states)?;
        let layout = HeapLayout::new(size_of::<usize>(), 16).map_err(BoundsError::from)?;
        Ok(Some(Self::project(&storage, &population, layout)?))
    }

    fn closed_population(
        storage: &HybridDimensions,
        coarse: &HybridPopulation,
        ordinary: usize,
    ) -> Result<HybridPopulation, BoundsError> {
        let closed_states = ordinary
            .checked_add(3)
            .ok_or(BoundsError::Arithmetic)?
            .max(5);
        let states = coarse.states.min(closed_states);
        let stride = storage
            .sentinel_transitions
            .checked_div(3)
            .ok_or(BoundsError::Arithmetic)?;
        let transition_entries = states.checked_mul(stride).ok_or(BoundsError::Arithmetic)?;
        let closed_payload = states
            .checked_add(2)
            .and_then(|count| count.checked_mul(storage.state_payload_bytes))
            .ok_or(BoundsError::Arithmetic)?;
        Ok(HybridPopulation {
            states,
            transition_entries,
            encoded_payload_envelope: coarse.encoded_payload_envelope.min(closed_payload),
        })
    }

    pub(crate) fn from_dfa(dfa: &DFA) -> Result<Option<Self>, BoundsError> {
        let Some(population) = HybridPopulation::from_dfa(dfa)? else {
            return Ok(None);
        };
        let storage = HybridDimensions::from_dfa(dfa)?;
        // Sixteen bounds every group implementation in the pinned std source;
        // it is not a claim about this host's selected SIMD implementation.
        let layout = HeapLayout::new(size_of::<usize>(), 16)?;
        Self::project(&storage, &population, layout).map(Some)
    }

    fn project(
        storage: &HybridDimensions,
        population: &HybridPopulation,
        layout: HeapLayout,
    ) -> Result<Self, BoundsError> {
        if storage.sparse_entries == 0
            || !storage.sparse_entries.is_multiple_of(4)
            || storage.start_entries < 12
            || !storage.start_entries.is_multiple_of(6)
            || !storage.sentinel_transitions.is_multiple_of(3)
            || storage.epsilon_stack == 0
            || storage.state_payload_bytes < 9
            || population.states < 5
        {
            return Err(BoundsError::Shape);
        }
        let stride = storage
            .sentinel_transitions
            .checked_div(3)
            .ok_or(BoundsError::Arithmetic)?;
        if !stride.is_power_of_two() || !(2..=512).contains(&stride) {
            return Err(BoundsError::Shape);
        }
        if population
            .states
            .checked_mul(stride)
            .ok_or(BoundsError::Arithmetic)?
            != population.transition_entries
        {
            return Err(BoundsError::Shape);
        }
        let minimum_payload = storage
            .state_payload_bytes
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(27))
            .ok_or(BoundsError::Arithmetic)?;
        if population.encoded_payload_envelope < minimum_payload {
            return Err(BoundsError::Shape);
        }
        let fixed = storage
            .sparse_entries
            .max(16)
            .checked_add(storage.start_entries)
            .and_then(|entries| entries.checked_mul(4))
            .ok_or(BoundsError::Arithmetic)?;
        let transitions = layout.append_vector(4, population.transition_entries)?;
        let handles = layout.append_vector(
            layout
                .word_bytes()
                .checked_mul(2)
                .ok_or(BoundsError::Arithmetic)?,
            population.states,
        )?;
        let stack = layout.append_vector(4, storage.epsilon_stack)?;
        let builder = layout.append_vector(1, storage.state_payload_bytes)?;
        let map = layout.state_map(population.states)?;
        // All clones share backing. Include pending and detached saved states;
        // each distinct backing has two header words and at most w-1 padding.
        let per_backing = layout
            .word_bytes()
            .checked_mul(3)
            .and_then(|bytes| bytes.checked_sub(1))
            .ok_or(BoundsError::Arithmetic)?;
        let arcs = population
            .states
            .checked_add(2)
            .and_then(|count| count.checked_mul(per_backing))
            .and_then(|headers| headers.checked_add(population.encoded_payload_envelope))
            .ok_or(BoundsError::Arithmetic)?;
        let live_envelope_bytes = [
            fixed,
            transitions.retained_bytes,
            handles.retained_bytes,
            stack.retained_bytes,
            builder.retained_bytes,
            map.retained_bytes,
            arcs,
            9, // State::dead's separate temporary builder.
        ]
        .into_iter()
        .try_fold(0_usize, |total, bytes| {
            total.checked_add(bytes).ok_or(BoundsError::Arithmetic)
        })?;
        // Each serial growth returns before another collection can grow.
        let growth_overlap_bytes = [
            transitions.old_growth_bytes,
            handles.old_growth_bytes,
            stack.old_growth_bytes,
            builder.old_growth_bytes,
            map.old_growth_bytes,
        ]
        .into_iter()
        .fold(0, usize::max);
        let peak_bytes = live_envelope_bytes
            .checked_add(growth_overlap_bytes)
            .ok_or(BoundsError::Arithmetic)?;
        Ok(Self {
            live_envelope: live_envelope_bytes,
            growth_overlap: growth_overlap_bytes,
            peak: peak_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoundsError, CompleteHeapError, GraphError, HeapLayout, HybridDimensions, HybridHeap,
        HybridPopulation,
    };
    use crate::schema_cache_bounds::{NfaDimensions, forward_nfa};
    use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
    use crate::schema_hybrid_population::recovery_ceiling;
    use regex_automata::hybrid::dfa::DFA;
    use regex_automata::nfa::thompson::{Builder, Transition};
    use regex_automata::util::look::Look;
    use std::mem::size_of;

    fn storage() -> HybridDimensions {
        HybridDimensions {
            sparse_entries: 12,
            start_entries: 18,
            sentinel_transitions: 12,
            epsilon_stack: 1,
            state_payload_bytes: 32,
        }
    }

    #[test]
    fn fresh_hybrid_heap_pins_literal_owners_across_widths_and_groups() {
        // 64 sparse + 72 starts + 96 transitions + handles + map + Arc + 9 temporary.
        for (word, group, live, overlap, peak) in [
            (4, 4, 365, 44, 409),
            (4, 8, 369, 44, 413),
            (4, 16, 377, 44, 421),
            (8, 8, 477, 44, 521),
            (8, 16, 485, 44, 529),
        ] {
            assert_eq!(
                HybridHeap::project_fresh(&storage(), HeapLayout::new(word, group).unwrap()),
                Ok(HybridHeap {
                    live_envelope: live,
                    growth_overlap: overlap,
                    peak,
                }),
                "word={word} group={group}"
            );
        }
    }

    #[test]
    fn fresh_hybrid_heap_varies_fixed_dimensions_and_stride_limits() {
        for (word, group, sparse, starts, sentinels, live, overlap, peak) in [
            (4, 4, 4, 12, 6, 293, 20, 313),
            (8, 8, 4, 12, 6, 405, 20, 425),
            (8, 16, 16, 18, 12, 485, 44, 529),
            (4, 8, 20, 24, 24, 505, 92, 597),
            (8, 16, 20, 24, 24, 621, 92, 713),
            (4, 16, 36, 30, 48, 793, 188, 981),
            (8, 8, 36, 30, 48, 893, 188, 1081),
            (4, 16, 4, 12, 1536, 12_545, 6140, 18_685),
            (8, 16, 4, 12, 1536, 12_653, 6140, 18_793),
        ] {
            let dimensions = HybridDimensions {
                sparse_entries: sparse,
                start_entries: starts,
                sentinel_transitions: sentinels,
                epsilon_stack: 0,
                state_payload_bytes: 0,
            };
            assert_eq!(
                HybridHeap::project_fresh(&dimensions, HeapLayout::new(word, group).unwrap()),
                Ok(HybridHeap {
                    live_envelope: live,
                    growth_overlap: overlap,
                    peak,
                }),
                "word={word} group={group} sparse={sparse} starts={starts} sentinels={sentinels}"
            );
        }
    }

    #[test]
    fn fresh_hybrid_heap_ignores_unallocated_search_scratch() {
        for (stack, payload) in [(0, 0), (0, 8), (1, 32), (usize::MAX, usize::MAX)] {
            let dimensions = HybridDimensions {
                epsilon_stack: stack,
                state_payload_bytes: payload,
                ..storage()
            };
            assert_eq!(
                HybridHeap::project_fresh(&dimensions, HeapLayout::new(8, 16).unwrap()),
                Ok(HybridHeap {
                    live_envelope: 485,
                    growth_overlap: 44,
                    peak: 529,
                })
            );
        }
    }

    #[test]
    fn fresh_hybrid_heap_refuses_malformed_fresh_dimensions() {
        for (sparse, starts, sentinels) in [
            (0, 18, 12),
            (13, 18, 12),
            (12, 0, 12),
            (12, 6, 12),
            (12, 11, 12),
            (12, 19, 12),
            (12, 18, 0),
            (12, 18, 3),
            (12, 18, 5),
            (12, 18, 9),
            (12, 18, 15),
            (12, 18, 1539),
            (12, 18, 3072),
        ] {
            let dimensions = HybridDimensions {
                sparse_entries: sparse,
                start_entries: starts,
                sentinel_transitions: sentinels,
                ..storage()
            };
            assert_eq!(
                HybridHeap::project_fresh(&dimensions, HeapLayout::new(8, 16).unwrap()),
                Err(BoundsError::Shape),
                "sparse={sparse} starts={starts} sentinels={sentinels}"
            );
        }
    }

    #[test]
    fn fresh_hybrid_heap_checks_allocation_and_component_overflow() {
        for (word, sparse, starts) in [
            (8, usize::MAX - 3, 18),
            (8, usize::MAX.checked_div(2).unwrap() - 3, 18),
            (8, 12, usize::MAX - usize::MAX.checked_rem(6).unwrap()),
            (4, 0x8000_0000, 18),
            (4, 12, 0x2000_0004),
        ] {
            let dimensions = HybridDimensions {
                sparse_entries: sparse,
                start_entries: starts,
                ..storage()
            };
            assert_eq!(
                HybridHeap::project_fresh(&dimensions, HeapLayout::new(word, 16).unwrap()),
                Err(BoundsError::Arithmetic),
                "word={word} sparse={sparse} starts={starts}"
            );
        }
        // The four separate sparse requests fit even when their sum exceeds i32::MAX.
        let dimensions = HybridDimensions {
            sparse_entries: 0x2000_0000,
            ..storage()
        };
        assert_eq!(
            HybridHeap::project_fresh(&dimensions, HeapLayout::new(4, 16).unwrap()),
            Ok(HybridHeap {
                live_envelope: 0x8000_0139,
                growth_overlap: 44,
                peak: 0x8000_0165,
            })
        );
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fresh_hybrid_heap_checks_composed_live_and_peak_overflow() {
        let layout = HeapLayout::new(8, 16).unwrap();
        // Besides sparse storage, the literal owners sum to 421; overlap is 44.
        for sparse in [0x3fff_ffff_ffff_fffc, 0x3fff_ffff_ffff_ff94] {
            let dimensions = HybridDimensions {
                sparse_entries: sparse,
                ..storage()
            };
            layout
                .fresh_resize_vector(4, sparse.checked_div(4).unwrap())
                .unwrap();
            assert!(sparse.checked_mul(4).is_some());
            assert_eq!(
                HybridHeap::project_fresh(&dimensions, layout),
                Err(BoundsError::Arithmetic)
            );
        }
        let dimensions = HybridDimensions {
            sparse_entries: 0x3fff_ffff_ffff_ff88,
            ..storage()
        };
        assert_eq!(
            HybridHeap::project_fresh(&dimensions, layout),
            Ok(HybridHeap {
                live_envelope: 0xffff_ffff_ffff_ffc5,
                growth_overlap: 44,
                peak: 0xffff_ffff_ffff_fff1,
            })
        );
    }

    #[test]
    fn fresh_hybrid_heap_constructor_binds_the_actual_never_match_dfa() {
        let dfa = DFA::never_match().unwrap();
        let dimensions = HybridDimensions::from_dfa(&dfa).unwrap();
        assert_eq!(dimensions.sparse_entries, 4);
        assert_eq!(dimensions.start_entries, 12);
        assert_eq!(dimensions.sentinel_transitions, 6);
        for (word, group, live, overlap, peak) in [
            (4, 4, 293, 20, 313),
            (4, 8, 297, 20, 317),
            (4, 16, 305, 20, 325),
            (8, 8, 405, 20, 425),
            (8, 16, 413, 20, 433),
        ] {
            assert_eq!(
                HybridHeap::from_fresh_dfa(&dfa, HeapLayout::new(word, group).unwrap()),
                Ok(HybridHeap {
                    live_envelope: live,
                    growth_overlap: overlap,
                    peak,
                }),
                "word={word} group={group}"
            );
        }
    }

    #[test]
    fn complete_population_minima_preserve_independent_state_and_payload_bounds() {
        for (coarse_states, coarse_payload, payload, ordinary, states, transitions, encoded) in [
            (24_usize, 1088, 32, 4, 7, 28, 288),
            (24, 1088, 32, 20, 23, 92, 800),
            (24, 1088, 32, 21, 24, 96, 832),
            (6, 1088, 32, 4, 6, 24, 256),
            (24, 200, 32, 4, 7, 28, 200),
            (24, 300, 64, 4, 7, 28, 300),
            (24, 1088, 32, 1, 5, 20, 224),
            (24, 832, 32, 21, 24, 96, 832),
        ] {
            let mut dimensions = storage();
            dimensions.state_payload_bytes = payload;
            let coarse = HybridPopulation {
                states: coarse_states,
                transition_entries: coarse_states.checked_mul(4).unwrap(),
                encoded_payload_envelope: coarse_payload,
            };
            assert_eq!(
                HybridHeap::closed_population(&dimensions, &coarse, ordinary),
                Ok(HybridPopulation {
                    states,
                    transition_entries: transitions,
                    encoded_payload_envelope: encoded
                })
            );
        }
    }

    #[test]
    fn complete_population_checks_arithmetic_before_minima_can_hide_overflow() {
        let coarse = HybridPopulation {
            states: 24,
            transition_entries: 96,
            encoded_payload_envelope: 1088,
        };
        assert_eq!(
            HybridHeap::closed_population(&storage(), &coarse, usize::MAX),
            Err(BoundsError::Arithmetic)
        );
        let mut dimensions = storage();
        dimensions.state_payload_bytes = usize::MAX;
        assert_eq!(
            HybridHeap::closed_population(&dimensions, &coarse, 4),
            Err(BoundsError::Arithmetic)
        );
    }

    #[test]
    fn complete_heap_tracks_its_own_distinct_nfa_shape_and_completed_graph() {
        let mut builder = Builder::new();
        builder.start_pattern().unwrap();
        let matched = builder.add_match().unwrap();
        let end = builder.add_look(matched, Look::End).unwrap();
        let byte = builder
            .add_range(Transition {
                start: b'a',
                end: b'a',
                next: end,
            })
            .unwrap();
        builder.finish_pattern(byte).unwrap();
        let nfa = builder.build(byte, byte).unwrap();
        let dfa = DFA::builder().build_from_nfa(nfa).unwrap();
        let (live_envelope, growth_overlap, peak) = if size_of::<usize>() == 8 {
            (1241, 116, 1357)
        } else {
            (953, 92, 1045)
        };
        assert_eq!(
            HybridHeap::from_complete_dfa(&dfa),
            Ok(Some(HybridHeap {
                live_envelope,
                growth_overlap,
                peak,
            }))
        );
    }

    #[test]
    fn complete_heap_has_a_literal_positive_envelope_for_the_same_never_match_dfa() {
        let dfa = DFA::never_match().unwrap();
        let coarse = HybridHeap::from_dfa(&dfa).unwrap().unwrap();
        let complete = HybridHeap::from_complete_dfa(&dfa).unwrap().unwrap();
        let (live_envelope, growth_overlap, peak) = if size_of::<usize>() == 8 {
            (916, 116, 1032)
        } else {
            (656, 68, 724)
        };
        assert_eq!(
            complete,
            HybridHeap {
                live_envelope,
                growth_overlap,
                peak
            }
        );
        assert!(complete.live_envelope < coarse.live_envelope);
        assert!(complete.peak < coarse.peak);
    }

    #[test]
    fn complete_heap_keeps_unavailable_recovery_distinct_from_an_accepted_zero() {
        let nfa = forward_nfa("[01]*1[01]{16}$").unwrap();
        let candidate = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        let required = recovery_ceiling(
            NfaDimensions::from_nfa(&nfa).unwrap(),
            &HybridDimensions::from_dfa(&candidate).unwrap(),
            size_of::<usize>(),
        )
        .unwrap();
        let dfa = DFA::builder()
            .configure(
                candidate
                    .get_config()
                    .clone()
                    .cache_capacity(required.checked_sub(1).unwrap()),
            )
            .build_from_nfa(nfa)
            .unwrap();
        assert_eq!(HybridHeap::from_complete_dfa(&dfa), Ok(None));
    }

    #[test]
    fn complete_heap_refuses_a_partial_graph_after_independent_recovery_admission() {
        let dfa = candidate_hybrid(
            &forward_nfa("[01]*1[01]{16}$").unwrap(),
            HybridDirection::Forward,
        )
        .unwrap();
        HybridHeap::from_dfa(&dfa).unwrap().unwrap();
        assert_eq!(
            HybridHeap::from_complete_dfa(&dfa),
            Err(CompleteHeapError::Exploration(GraphError::Cleared))
        );
    }

    #[test]
    fn heap_envelope_pins_disjoint_owners_and_one_largest_growth_overlap() {
        for (word, states, transitions, live, overlap, peak) in
            [(8, 24, 96, 4263, 416, 4679), (4, 31, 124, 4012, 492, 4504)]
        {
            let population = HybridPopulation {
                states,
                transition_entries: transitions,
                encoded_payload_envelope: 1088,
            };
            assert_eq!(
                HybridHeap::project(&storage(), &population, HeapLayout::new(word, 16).unwrap()),
                Ok(HybridHeap {
                    live_envelope: live,
                    growth_overlap: overlap,
                    peak
                })
            );
        }
    }

    #[test]
    fn heap_envelope_varies_fixed_scratch_and_payload_owners_independently() {
        let population = HybridPopulation {
            states: 24,
            transition_entries: 96,
            encoded_payload_envelope: 1088,
        };
        for (sparse, starts, stack, payload, live, overlap, peak) in [
            (20, 18, 1, 32, 4279, 416, 4695),
            (12, 24, 1, 32, 4287, 416, 4703),
            (12, 18, 120, 32, 5207, 476, 5683),
            (12, 18, 1, 240, 4679, 416, 5095),
        ] {
            let mut dimensions = storage();
            dimensions.sparse_entries = sparse;
            dimensions.start_entries = starts;
            dimensions.epsilon_stack = stack;
            dimensions.state_payload_bytes = payload;
            assert_eq!(
                HybridHeap::project(&dimensions, &population, HeapLayout::new(8, 16).unwrap()),
                Ok(HybridHeap {
                    live_envelope: live,
                    growth_overlap: overlap,
                    peak
                })
            );
        }
    }

    #[test]
    fn heap_projection_requires_real_recovery_admission_and_refuses_capacity_upgrades() {
        let nfa = forward_nfa("[01]*1[01]{16}$").unwrap();
        let ordinary = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        let dimensions = HybridDimensions::from_dfa(&ordinary).unwrap();
        let required = recovery_ceiling(
            NfaDimensions::from_nfa(&nfa).unwrap(),
            &dimensions,
            size_of::<usize>(),
        )
        .unwrap();
        assert!(
            required
                > ordinary
                    .get_config()
                    .get_minimum_cache_capacity(&nfa)
                    .unwrap()
        );
        for (capacity, accepted) in [(required - 1, false), (required, true)] {
            let dfa = DFA::builder()
                .configure(ordinary.get_config().clone().cache_capacity(capacity))
                .build_from_nfa(nfa.clone())
                .unwrap();
            assert_eq!(HybridHeap::from_dfa(&dfa).unwrap().is_some(), accepted);
        }
        let upgraded = DFA::builder()
            .configure(
                ordinary
                    .get_config()
                    .clone()
                    .cache_capacity(0)
                    .skip_cache_capacity_check(true),
            )
            .build_from_nfa(nfa)
            .unwrap();
        assert_eq!(HybridHeap::from_dfa(&upgraded), Err(BoundsError::Shape));
    }

    #[test]
    fn heap_projection_refuses_inconsistent_lengths_and_checked_component_overflow() {
        let valid = HybridPopulation {
            states: 24,
            transition_entries: 96,
            encoded_payload_envelope: 1088,
        };
        for population in [
            HybridPopulation { states: 0, ..valid },
            HybridPopulation {
                transition_entries: 95,
                ..valid
            },
        ] {
            assert_eq!(
                HybridHeap::project(&storage(), &population, HeapLayout::new(8, 16).unwrap()),
                Err(BoundsError::Shape)
            );
        }
        let overflowing = HybridPopulation {
            encoded_payload_envelope: usize::MAX,
            ..valid
        };
        assert_eq!(
            HybridHeap::project(&storage(), &overflowing, HeapLayout::new(8, 16).unwrap()),
            Err(BoundsError::Arithmetic)
        );
    }

    #[test]
    fn heap_projection_refuses_unreviewed_fixed_storage_shapes() {
        let population = HybridPopulation {
            states: 24,
            transition_entries: 96,
            encoded_payload_envelope: 1088,
        };
        for (sparse, starts, sentinels, stack, payload) in [
            (0, 18, 12, 1, 32),
            (13, 18, 12, 1, 32),
            (12, 6, 12, 1, 32),
            (12, 19, 12, 1, 32),
            (12, 18, 11, 1, 32),
            (12, 18, 0, 1, 32),
            (12, 18, 9, 1, 32),
            (12, 18, 3072, 1, 32),
            (12, 18, 12, 0, 32),
            (12, 18, 12, 1, 8),
        ] {
            let dimensions = HybridDimensions {
                sparse_entries: sparse,
                start_entries: starts,
                sentinel_transitions: sentinels,
                epsilon_stack: stack,
                state_payload_bytes: payload,
            };
            assert_eq!(
                HybridHeap::project(&dimensions, &population, HeapLayout::new(8, 16).unwrap()),
                Err(BoundsError::Shape)
            );
        }
        let too_small = HybridPopulation {
            encoded_payload_envelope: 154,
            ..population
        };
        assert_eq!(
            HybridHeap::project(&storage(), &too_small, HeapLayout::new(8, 16).unwrap()),
            Err(BoundsError::Shape)
        );
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn individually_admitted_allocations_cannot_overflow_composed_live_or_peak_bytes() {
        let dimensions = HybridDimensions {
            sparse_entries: 4,
            start_entries: 12,
            sentinel_transitions: 6,
            epsilon_stack: 1,
            state_payload_bytes: 32,
        };
        let layout = HeapLayout::new(8, 16).unwrap();
        // Arithmetic controls, not constructible source DFAs or admitted budgets.
        for (states, payload) in [
            ((1_usize << 57_u32) - 1, usize::MAX.checked_div(2).unwrap()),
            ((1_usize << 56_u32) - 1, 9_727_775_195_120_271_167),
        ] {
            let population = HybridPopulation {
                states,
                transition_entries: states * 2,
                encoded_payload_envelope: payload,
            };
            layout
                .append_vector(4, population.transition_entries)
                .unwrap();
            layout.append_vector(16, population.states).unwrap();
            layout.state_map(population.states).unwrap();
            assert!(payload.checked_add((states + 2) * 23).is_some());
            assert_eq!(
                HybridHeap::project(&dimensions, &population, layout),
                Err(BoundsError::Arithmetic)
            );
        }
        let peak_case_terms = [
            112_usize,
            0x0FFF_FFFF_FFFF_FFF0,
            2_305_843_009_213_693_920,
            16,
            64,
            3_602_879_701_896_396_816,
            11_385_099_857_992_613_718,
            9,
        ];
        assert_eq!(
            peak_case_terms
                .into_iter()
                .try_fold(0_usize, usize::checked_add),
            Some(usize::MAX)
        );
    }
}

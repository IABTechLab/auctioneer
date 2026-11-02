//! Conditional normal-recovery model, not capacities or a physical-memory bound.

use crate::schema_cache_bounds::{BoundsError, NfaDimensions};
use crate::schema_hybrid_bounds::HybridDimensions;
use regex_automata::hybrid::dfa::DFA;
use std::fmt;
use std::mem::size_of;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HybridPopulation {
    pub states: usize,
    pub transition_entries: usize,
    pub encoded_payload_envelope: usize,
}

impl HybridPopulation {
    /// None means recovery is unproved, never that this cache has no residency.
    pub(crate) fn from_dfa(dfa: &DFA) -> Result<Option<Self>, BoundsError> {
        if dfa.get_config().get_skip_cache_capacity_check() {
            return Err(BoundsError::Shape);
        }
        let shape = NfaDimensions::from_nfa(dfa.get_nfa())?;
        let storage = HybridDimensions::from_dfa(dfa)?;
        let width = size_of::<usize>();
        let capacity = dfa.get_config().get_cache_capacity();
        if capacity < recovery_ceiling(shape, &storage, width)? {
            return Ok(None);
        }
        Self::project(&storage, capacity, width).map(Some)
    }

    /// Assumes fixed-DFA ownership and nonrecursive five-state recovery.
    pub(crate) fn project(
        storage: &HybridDimensions,
        capacity: usize,
        word_bytes: usize,
    ) -> Result<Self, BoundsError> {
        let stride = checked_layout(storage, word_bytes)?;
        // Ignore map/scratch costs only to obtain a lower reported cost per state.
        let minimum_state_bytes = stride
            .checked_mul(4)
            .and_then(|bytes| bytes.checked_add(word_bytes.checked_mul(2)?))
            .and_then(|bytes| bytes.checked_add(9))
            .ok_or(BoundsError::Arithmetic)?;
        let states = capacity
            .checked_div(minimum_state_bytes)
            .ok_or(BoundsError::Arithmetic)?
            .max(5);
        let transition_entries = states.checked_mul(stride).ok_or(BoundsError::Arithmetic)?;
        // Clearing may retain one detached saved state alongside a pending state.
        let transient = storage
            .state_payload_bytes
            .checked_mul(2)
            .ok_or(BoundsError::Arithmetic)?;
        let reset_payload = transient.checked_add(27).ok_or(BoundsError::Arithmetic)?;
        let encoded_payload_envelope = capacity
            .max(reset_payload)
            .checked_add(transient)
            .ok_or(BoundsError::Arithmetic)?;
        Ok(Self {
            states,
            transition_entries,
            encoded_payload_envelope,
        })
    }
}

pub(crate) fn recovery_ceiling(
    nfa: NfaDimensions,
    storage: &HybridDimensions,
    word_bytes: usize,
) -> Result<usize, BoundsError> {
    let stride = checked_layout(storage, word_bytes)?;
    if nfa.states == 0 {
        return Err(BoundsError::Shape);
    }
    let sparse = nfa.states.checked_mul(16).ok_or(BoundsError::Arithmetic)?;
    let starts = storage
        .start_entries
        .checked_mul(4)
        .ok_or(BoundsError::Arithmetic)?;
    let stack = nfa
        .branch_frames
        .max(1)
        .checked_mul(2)
        .and_then(|entries| entries.max(4).checked_mul(4))
        .ok_or(BoundsError::Arithmetic)?;
    let builder = storage
        .state_payload_bytes
        .checked_mul(2)
        .ok_or(BoundsError::Arithmetic)?;
    // Covers the largest sentinel check and saved-state reinsertion, without L.
    let reinsertion = stride
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_add(word_bytes.checked_mul(12)?))
        .and_then(|bytes| bytes.checked_add(35))
        .and_then(|bytes| bytes.checked_add(storage.state_payload_bytes))
        .ok_or(BoundsError::Arithmetic)?;
    [sparse, starts, stack, builder, reinsertion]
        .into_iter()
        .try_fold(0_usize, |total, bytes| {
            total.checked_add(bytes).ok_or(BoundsError::Arithmetic)
        })
}

fn checked_layout(storage: &HybridDimensions, word_bytes: usize) -> Result<usize, BoundsError> {
    if !matches!(word_bytes, 4 | 8)
        || !storage.sentinel_transitions.is_multiple_of(3)
        || storage.state_payload_bytes < 9
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
    Ok(stride)
}

impl fmt::Display for HybridPopulation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "normal-recovery states<={} transition-entries<={} encoded-payload-envelope<={}",
            self.states, self.transition_entries, self.encoded_payload_envelope
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundsError, HybridDimensions, HybridPopulation, recovery_ceiling};
    use crate::schema_cache_bounds::{NfaDimensions, forward_nfa};
    use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
    use regex_automata::{Input, hybrid::dfa::DFA};
    use std::collections::HashSet;
    use std::fmt::Write as _;
    use std::io::{Write as _, stderr};
    use std::mem::size_of;

    const SHAPE: NfaDimensions = NfaDimensions {
        states: 5,
        slots: 2,
        patterns: 1,
        branch_frames: 0,
        capture_states: 0,
    };

    fn storage(states: usize, patterns: usize, stride: usize) -> HybridDimensions {
        HybridDimensions::project(
            NfaDimensions {
                states,
                slots: 2,
                patterns,
                branch_frames: 0,
                capture_states: 0,
            },
            stride,
            true,
        )
        .unwrap()
    }

    #[test]
    fn recovery_ceiling_varies_each_retained_component_without_population_feedback() {
        for (states, branches, starts, payload, stride, width, expected) in [
            (5, 0, 18, 42, 8, 8, 553),
            (7, 0, 18, 42, 8, 8, 585),
            (5, 3, 18, 42, 8, 8, 561),
            (5, 5, 18, 42, 8, 8, 577),
            (5, 0, 24, 42, 8, 8, 577),
            (5, 0, 18, 52, 8, 8, 583),
            (5, 0, 18, 42, 16, 8, 681),
            (5, 0, 18, 42, 8, 4, 505),
        ] {
            let shape = NfaDimensions {
                states,
                branch_frames: branches,
                ..SHAPE
            };
            let mut dimensions = storage(5, 1, stride);
            dimensions.start_entries = starts;
            dimensions.state_payload_bytes = payload;
            assert_eq!(recovery_ceiling(shape, &dimensions, width), Ok(expected));
        }
    }

    #[test]
    fn recovery_ceiling_refuses_invalid_layout_and_each_growth_overflow() {
        assert_eq!(
            recovery_ceiling(NfaDimensions { states: 0, ..SHAPE }, &storage(5, 1, 8), 8),
            Err(BoundsError::Shape)
        );
        for shape in [
            NfaDimensions {
                states: usize::MAX,
                ..SHAPE
            },
            NfaDimensions {
                branch_frames: usize::MAX,
                ..SHAPE
            },
        ] {
            assert_eq!(
                recovery_ceiling(shape, &storage(5, 1, 8), 8),
                Err(BoundsError::Arithmetic)
            );
        }
        for (starts, payload) in [(usize::MAX, 42), (18, usize::MAX)] {
            let mut dimensions = storage(5, 1, 8);
            dimensions.start_entries = starts;
            dimensions.state_payload_bytes = payload;
            assert_eq!(
                recovery_ceiling(SHAPE, &dimensions, 8),
                Err(BoundsError::Arithmetic)
            );
        }
        assert_eq!(
            recovery_ceiling(SHAPE, &storage(5, 1, 8), 2),
            Err(BoundsError::Shape)
        );
        // Each component fits; only the final sum overflows.
        let shape = NfaDimensions {
            states: usize::MAX.checked_div(32).unwrap(),
            ..SHAPE
        };
        let mut dimensions = storage(5, 1, 8);
        dimensions.start_entries = usize::MAX.checked_div(8).unwrap();
        assert!(shape.states.checked_mul(16).is_some());
        assert!(dimensions.start_entries.checked_mul(4).is_some());
        assert_eq!(
            recovery_ceiling(shape, &dimensions, 8),
            Err(BoundsError::Arithmetic)
        );
    }

    #[test]
    fn automatic_capacity_upgrades_cannot_claim_a_projection_for_the_smaller_setting() {
        let nfa = forward_nfa("a+b").unwrap();
        let ordinary = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        assert!(HybridPopulation::from_dfa(&ordinary).unwrap().is_some());
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
        assert_eq!(
            HybridPopulation::from_dfa(&upgraded),
            Err(BoundsError::Shape)
        );
    }

    #[test]
    fn recovery_admission_is_inclusive_after_the_engine_builder_succeeds() {
        let nfa = forward_nfa("[01]*1[01]{16}$").unwrap();
        let base = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        let dimensions = HybridDimensions::from_dfa(&base).unwrap();
        let required = recovery_ceiling(
            NfaDimensions::from_nfa(&nfa).unwrap(),
            &dimensions,
            size_of::<usize>(),
        )
        .unwrap();
        let engine_minimum = base.get_config().get_minimum_cache_capacity(&nfa).unwrap();
        assert!(required > engine_minimum);
        for (capacity, accepted) in [(required - 1, false), (required, true)] {
            let dfa = DFA::builder()
                .configure(base.get_config().clone().cache_capacity(capacity))
                .build_from_nfa(nfa.clone())
                .unwrap();
            assert_eq!(
                HybridPopulation::from_dfa(&dfa).unwrap().is_some(),
                accepted
            );
        }
    }

    #[test]
    fn hybrid_population_varies_capacity_stride_word_width_and_payload_independently() {
        for (nfa, patterns, stride, capacity, width, states, transitions, payload) in [
            (5, 1, 8, 1024, 8, 17, 136, 1108),
            (5, 1, 8, 2048, 8, 35, 280, 2132),
            (5, 1, 8, 1024, 4, 20, 160, 1108),
            (5, 1, 16, 1024, 8, 11, 176, 1108),
            (7, 1, 8, 1024, 8, 17, 136, 1128),
            (5, 2, 8, 1024, 8, 17, 136, 1116),
        ] {
            assert_eq!(
                HybridPopulation::project(&storage(nfa, patterns, stride), capacity, width),
                Ok(HybridPopulation {
                    states,
                    transition_entries: transitions,
                    encoded_payload_envelope: payload,
                })
            );
        }
    }

    #[test]
    fn hybrid_population_clearing_allowance_and_payload_threshold_are_independent() {
        // Small capacities exercise the arithmetic, not admission of a real DFA.
        for (capacity, states, payload) in [
            (0, 5, 195),
            (100, 5, 195),
            (111, 5, 195),
            (112, 5, 196),
            (342, 6, 426),
        ] {
            assert_eq!(
                HybridPopulation::project(&storage(5, 1, 8), capacity, 8),
                Ok(HybridPopulation {
                    states,
                    transition_entries: states * 8,
                    encoded_payload_envelope: payload,
                })
            );
        }
    }

    #[test]
    fn hybrid_population_refuses_invalid_layout_and_overflow() {
        for width in [0, 2, 16] {
            assert_eq!(
                HybridPopulation::project(&storage(5, 1, 8), 1024, width),
                Err(BoundsError::Shape)
            );
        }
        let mut invalid = storage(5, 1, 8);
        for transitions in [0, 3, 7, 9, 3072] {
            invalid.sentinel_transitions = transitions;
            assert_eq!(
                HybridPopulation::project(&invalid, 1024, 8),
                Err(BoundsError::Shape)
            );
        }
        invalid = storage(5, 1, 8);
        invalid.state_payload_bytes = 8;
        assert_eq!(
            HybridPopulation::project(&invalid, 1024, 8),
            Err(BoundsError::Shape)
        );
        for (payload, capacity) in [
            (usize::MAX, 1024),
            (usize::MAX.checked_div(2).unwrap(), 1024),
            (42, usize::MAX),
        ] {
            let mut overflow = storage(5, 1, 8);
            overflow.state_payload_bytes = payload;
            assert_eq!(
                HybridPopulation::project(&overflow, capacity, 8),
                Err(BoundsError::Arithmetic)
            );
        }
    }

    #[test]
    fn observed_hybrid_state_id_populations_stay_bounded_across_clearing_epochs() {
        let nfa = forward_nfa("[01]*1[01]{16}$").unwrap();
        let base = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        let minimum = base.get_config().get_minimum_cache_capacity(&nfa).unwrap();
        let mut haystack = String::new();
        for value in 0_u16..1024 {
            write!(haystack, "{value:010b}").unwrap();
        }
        let mut observations = [0; 2];
        let mut reported_over_capacity = false;
        for (slot, capacity) in [minimum, minimum * 8].into_iter().enumerate() {
            let dfa = DFA::builder()
                .configure(base.get_config().clone().cache_capacity(capacity))
                .build_from_nfa(nfa.clone())
                .unwrap();
            let storage = HybridDimensions::from_dfa(&dfa).unwrap();
            let bound = HybridPopulation::project(&storage, capacity, size_of::<usize>()).unwrap();
            let checked_population = HybridPopulation::from_dfa(&dfa).unwrap();
            if slot == 0 {
                assert_eq!(
                    checked_population, None,
                    "construction is not a recovery proof"
                );
            } else {
                assert_eq!(checked_population.as_ref(), Some(&bound));
            }
            let mut cache = dfa.create_cache();
            let mut id = dfa
                .start_state_forward(&mut cache, &Input::new(&haystack))
                .unwrap();
            let mut seen = HashSet::from([id]);
            let (mut epoch, mut maximum, mut consumed, mut failed) = (0, 1, 0, false);
            let mut max_reported = cache.memory_usage();
            cache.search_start(0);
            for (index, byte) in haystack.bytes().enumerate() {
                cache.search_update(index);
                match dfa.next_state(&mut cache, id, byte) {
                    Ok(next) => id = next,
                    Err(_cache_error) => {
                        failed = true;
                        break;
                    }
                }
                consumed = index + 1;
                if cache.clear_count() != epoch {
                    seen.clear();
                    epoch = cache.clear_count();
                }
                seen.insert(id);
                maximum = maximum.max(seen.len());
                assert!(seen.len() <= bound.states);
                max_reported = max_reported.max(cache.memory_usage());
            }
            cache.search_finish(consumed);
            writeln!(
                stderr(),
                "hybrid population observation: capacity={capacity} {bound} observed={maximum} consumed={consumed} clears={} max-reported={max_reported}",
                cache.clear_count()
            ).unwrap();
            assert!(failed, "fixture must reach cache-efficiency refusal");
            assert!(cache.clear_count() >= 3);
            assert!(maximum >= 2, "state-ID observation must not be vacuous");
            assert!(consumed > maximum);
            observations[slot] = maximum;
            reported_over_capacity |= max_reported > capacity;
        }
        assert!(observations[1] > observations[0]);
        assert!(observations[1] >= 8);
        assert!(
            reported_over_capacity,
            "restored scratch must exercise report overshoot"
        );
    }
}

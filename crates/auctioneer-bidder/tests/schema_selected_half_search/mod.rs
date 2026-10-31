//! Conditional native requested heap, not a constructor, work or serving bound.
//!
//! Contract: one fixed cache, serial whole UTF-8 haystacks, ordinary unanchored
//! `Input::new(...).earliest(true)`, no reset/clone/rebind, then drop. A finite
//! call budget is a caller premise, NOT an enforced generated-validator limit.
//!
//! Locked-source premises (verified by the selected-cohort source producer):
//! - meta/strategy.rs selects `Core` for mandatory `Start`, or `ReverseAnchored` first
//!   for mandatory `End` without `Start`. The latter excludes reverse prefix caches.
//! - meta/wrappers.rs installs a hybrid only after BOTH builders succeed; its
//!   half-search methods mutate only the selected primary direction's cache.
//! - hybrid/dfa.rs counters start at zero; hybrid/search.rs advances within the
//!   input span. Nonempty NFAs exclude the UTF-8 split-match retry loop.
//!
//! Complete non-quitting capacity/ID proofs exclude clears and fallback, not
//! construction of the inactive cache, captures, one-pass scratch or prefilters.

use crate::schema_cache_bounds::{BoundsError, forward_nfa, reverse_nfa};
use crate::schema_heap_layout::HeapLayout;
use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
use crate::schema_hybrid_capacity::ReportedCapacityProof;
use crate::schema_hybrid_heap::{CompleteHeapError, HybridHeap};
use crate::schema_literal_heap::AutomaticPrefilterHeap;
use crate::schema_meta_eager::CoreEagerHeap;
use crate::schema_meta_heap::{
    FreshAnchoredHalfHeap, FreshMetaHeapError, MetaHeapError, ProjectionStage,
};
use regex_automata::{
    MatchKind,
    hybrid::dfa::DFA,
    meta,
    nfa::thompson::{NFA, WhichCaptures},
    util::{prefilter::Prefilter, syntax},
};
use regex_syntax::hir::Look;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HalfDirection {
    CoreForward,
    ReverseAnchored,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum HalfSearchError {
    Configuration,
    MissingSearchBudget,
    InvalidSearchBudget,
    ProgressArithmetic,
    Syntax,
    Strategy,
    EmptyMatches,
    Meta(MetaHeapError),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CounterBound {
    pub calls: usize,
    pub searched_bytes: usize,
    pub endpoint: usize,
}

impl CounterBound {
    pub(crate) fn new(
        max_input_bytes: usize,
        max_calls: Option<usize>,
        counter_max: usize,
    ) -> Result<Self, HalfSearchError> {
        let calls = max_calls.ok_or(HalfSearchError::MissingSearchBudget)?;
        if calls == 0 {
            return Err(HalfSearchError::InvalidSearchBudget);
        }
        // search_start may finish an old progress record; search_total_len
        // adds the in-flight record. At most N scans of at most B bytes fund
        // both, even in reverse. No clear_count increment is admitted below.
        // This is byte-progress accounting, not a CPU-work coefficient.
        let searched_bytes = max_input_bytes
            .checked_mul(calls)
            .filter(|total| *total <= counter_max)
            .ok_or(HalfSearchError::ProgressArithmetic)?;
        let endpoint = max_input_bytes
            .checked_add(1)
            .filter(|endpoint| *endpoint <= counter_max)
            .ok_or(HalfSearchError::ProgressArithmetic)?;
        Ok(Self {
            calls,
            searched_bytes,
            endpoint,
        })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HalfSearchCertificate {
    pub direction: HalfDirection,
    pub eager_bytes: usize,
    pub active_peak: usize,
    pub inactive_fresh_peak: usize,
    pub peak: usize,
    pub counters: CounterBound,
    pub capacity: ReportedCapacityProof,
}

impl HalfSearchCertificate {
    pub(crate) fn from_pattern(
        pattern: &str,
        max_input_bytes: usize,
        max_calls: Option<usize>,
    ) -> Result<Self, HalfSearchError> {
        let counters = CounterBound::new(max_input_bytes, max_calls, usize::MAX)?;
        let config = pinned_configuration()?;
        let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true))
            .map_err(|_error| HalfSearchError::Syntax)?;
        let direction = if hir.properties().look_set_prefix().contains(Look::Start) {
            HalfDirection::CoreForward
        } else if hir.properties().look_set_suffix().contains(Look::End) {
            HalfDirection::ReverseAnchored
        } else {
            return Err(HalfSearchError::Strategy);
        };
        // This existing proof excludes literal-only short circuits, bounds the
        // actual scalar prefilter type and retains its shared immutable owner.
        // It refuses other prefilter/backend classes instead of guessing C.
        let prefilter = match AutomaticPrefilterHeap::from_pattern(pattern, &config)
            .map_err(|_error| HalfSearchError::Strategy)?
        {
            AutomaticPrefilterHeap::ExcludedStartAnchored => None,
            AutomaticPrefilterHeap::SingleByte { .. } => Some(
                Prefilter::from_hir_prefix(MatchKind::LeftmostFirst, &hir)
                    .ok_or(HalfSearchError::Strategy)?,
            ),
        };
        let forward_nfa = forward_nfa(pattern)
            .map_err(|reason| projection(ProjectionStage::ForwardNfa, reason))?;
        if forward_nfa.has_empty() {
            return Err(HalfSearchError::EmptyMatches);
        }
        let reverse_nfa = reverse_nfa(pattern)
            .map_err(|reason| projection(ProjectionStage::ReverseNfa, reason))?;
        if reverse_nfa.has_empty() {
            return Err(HalfSearchError::EmptyMatches);
        }
        let (forward, reverse) = primary_hybrids(&forward_nfa, &reverse_nfa, prefilter)?;
        let (active, active_direction) = match direction {
            HalfDirection::CoreForward => (&forward, HybridDirection::Forward),
            HalfDirection::ReverseAnchored => (&reverse, HybridDirection::Reverse),
        };
        let capacity = ReportedCapacityProof::from_non_quitting_dfa(active).map_err(|reason| {
            HalfSearchError::Meta(MetaHeapError::Capacity {
                direction: active_direction,
                reason,
            })
        })?;
        let (eager_bytes, active_peak, inactive_fresh_peak) = match direction {
            HalfDirection::CoreForward => {
                // Its forward complete-graph heap is already a history-wide
                // envelope. Only the new finite progress premise permits reuse;
                // the existing fresh-only constructor and broad model stay intact.
                let fresh = FreshAnchoredHalfHeap::from_pattern(pattern, max_input_bytes)?;
                (
                    fresh.eager_bytes,
                    fresh.forward_peak,
                    fresh.reverse_fresh_peak,
                )
            }
            HalfDirection::ReverseAnchored => reverse_heap_parts(&forward, &reverse)?,
        };
        let peak = eager_bytes
            .checked_add(active_peak)
            .and_then(|bytes| bytes.checked_add(inactive_fresh_peak))
            .ok_or_else(|| projection(ProjectionStage::Composition, BoundsError::Arithmetic))?;
        Ok(Self {
            direction,
            eager_bytes,
            active_peak,
            inactive_fresh_peak,
            peak,
            counters,
            capacity,
        })
    }
}

fn pinned_configuration() -> Result<meta::Config, HalfSearchError> {
    let config = meta::Regex::config()
        .match_kind(MatchKind::LeftmostFirst)
        .utf8_empty(true)
        .nfa_size_limit(Some(0x0001_0000))
        .hybrid_cache_capacity(0x0001_0000)
        .pool_capacity(1);
    if !config.get_hybrid()
        || config.get_dfa()
        || !config.get_auto_prefilter()
        || config.get_prefilter().is_some()
        || !matches!(config.get_which_captures(), WhichCaptures::All)
        || !config.get_byte_classes()
        || config.get_line_terminator() != b'\n'
    {
        return Err(HalfSearchError::Configuration);
    }
    Ok(config)
}

fn primary_hybrids(
    forward_nfa: &NFA,
    reverse_nfa: &NFA,
    prefilter: Option<Prefilter>,
) -> Result<(DFA, DFA), HalfSearchError> {
    let forward_candidate =
        candidate_hybrid(forward_nfa, HybridDirection::Forward).map_err(|_error| {
            HalfSearchError::Meta(MetaHeapError::HybridBuild {
                direction: HybridDirection::Forward,
            })
        })?;
    let forward = if let Some(prefilter_value) = prefilter {
        // HybridEngine::new forwards the automatic prefilter and specializes
        // forward start states. The reverse builder uses neither option.
        DFA::builder()
            .configure(
                forward_candidate
                    .get_config()
                    .clone()
                    .prefilter(Some(prefilter_value))
                    .specialize_start_states(true),
            )
            .build_from_nfa(forward_nfa.clone())
            .map_err(|_error| {
                HalfSearchError::Meta(MetaHeapError::HybridBuild {
                    direction: HybridDirection::Forward,
                })
            })?
    } else {
        forward_candidate
    };
    let reverse = candidate_hybrid(reverse_nfa, HybridDirection::Reverse).map_err(|_error| {
        HalfSearchError::Meta(MetaHeapError::HybridBuild {
            direction: HybridDirection::Reverse,
        })
    })?;
    Ok((forward, reverse))
}

fn reverse_heap_parts(
    forward: &DFA,
    reverse: &DFA,
) -> Result<(usize, usize, usize), HalfSearchError> {
    let layout = HeapLayout::new(size_of::<usize>(), 16)
        .map_err(|reason| projection(ProjectionStage::Layout, BoundsError::from(reason)))?;
    let eager = CoreEagerHeap::from_nfa(forward.get_nfa(), layout)
        .map_err(|reason| projection(ProjectionStage::Eager, reason))?;
    let active = HybridHeap::from_complete_dfa(reverse)
        .map_err(|reason| {
            HalfSearchError::Meta(MetaHeapError::Heap {
                direction: HybridDirection::Reverse,
                reason,
            })
        })?
        .ok_or(HalfSearchError::Meta(MetaHeapError::RecoveryUnavailable {
            direction: HybridDirection::Reverse,
        }))?;
    let inactive = HybridHeap::from_fresh_dfa(forward, layout).map_err(|reason| {
        HalfSearchError::Meta(MetaHeapError::Heap {
            direction: HybridDirection::Forward,
            reason: CompleteHeapError::Projection(reason),
        })
    })?;
    Ok((eager.total, active.peak, inactive.peak))
}

fn projection(stage: ProjectionStage, reason: BoundsError) -> HalfSearchError {
    HalfSearchError::Meta(MetaHeapError::Projection { stage, reason })
}

impl From<FreshMetaHeapError> for HalfSearchError {
    fn from(error: FreshMetaHeapError) -> Self {
        match error {
            FreshMetaHeapError::Configuration => Self::Configuration,
            FreshMetaHeapError::Syntax => Self::Syntax,
            FreshMetaHeapError::NotStartAnchored => Self::Strategy,
            FreshMetaHeapError::EmptyMatches => Self::EmptyMatches,
            FreshMetaHeapError::ProgressArithmetic => Self::ProgressArithmetic,
            FreshMetaHeapError::Meta(reason) => Self::Meta(reason),
        }
    }
}

#[cfg(test)]
pub(crate) fn assert_source_components(
    pattern: &str,
    max_input_bytes: usize,
    max_calls: usize,
    certificate: &HalfSearchCertificate,
) {
    use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};
    use crate::schema_heap_layout::HeapLayout;
    use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
    use crate::schema_hybrid_heap::HybridHeap;
    use crate::schema_meta_eager::CoreEagerHeap;
    use crate::schema_meta_heap::MetaCacheHeap;
    use std::mem::size_of;

    let forward_nfa = forward_nfa(pattern).unwrap();
    let reverse_nfa = reverse_nfa(pattern).unwrap();
    assert!(!forward_nfa.has_empty());
    assert!(!reverse_nfa.has_empty());
    let forward = candidate_hybrid(&forward_nfa, HybridDirection::Forward).unwrap();
    let reverse = candidate_hybrid(&reverse_nfa, HybridDirection::Reverse).unwrap();
    let layout = HeapLayout::new(size_of::<usize>(), 16).unwrap();
    let eager = CoreEagerHeap::from_nfa(&forward_nfa, layout).unwrap();
    let (active, inactive) = match certificate.direction {
        HalfDirection::CoreForward => (&forward, &reverse),
        HalfDirection::ReverseAnchored => (&reverse, &forward),
    };
    let capacity = ReportedCapacityProof::from_non_quitting_dfa(active).unwrap();
    let active_heap = HybridHeap::from_complete_dfa(active).unwrap().unwrap();
    let cold_heap = HybridHeap::from_fresh_dfa(inactive, layout).unwrap();
    assert_eq!(certificate.capacity, capacity);
    assert_eq!(capacity.configured, 0x0001_0000);
    assert!(capacity.required <= capacity.configured);
    assert_eq!(certificate.eager_bytes, eager.total);
    assert_eq!(certificate.active_peak, active_heap.peak);
    assert_eq!(certificate.inactive_fresh_peak, cold_heap.peak);
    assert!(eager.total > 0);
    assert!(cold_heap.peak > 0, "inactive does not mean unallocated");
    assert_eq!(
        certificate.peak,
        eager
            .total
            .checked_add(active_heap.peak)
            .unwrap()
            .checked_add(cold_heap.peak)
            .unwrap()
    );
    assert_eq!(certificate.counters.calls, max_calls);
    assert_eq!(
        certificate.counters.searched_bytes,
        max_input_bytes.checked_mul(max_calls).unwrap()
    );
    assert_eq!(
        certificate.counters.endpoint,
        max_input_bytes.checked_add(1).unwrap()
    );
    assert_strategy_components(pattern, max_input_bytes, certificate);
    let broad = MetaCacheHeap::from_pattern(pattern, max_input_bytes).unwrap();
    assert_eq!(certificate.eager_bytes, broad.eager_bytes);
    assert!(broad.fallback_bytes > 0);
    assert!(certificate.peak < broad.peak);
}

#[cfg(test)]
fn assert_strategy_components(
    pattern: &str,
    max_input_bytes: usize,
    certificate: &HalfSearchCertificate,
) {
    use regex_automata::meta::Regex;

    let config = Regex::config().match_kind(MatchKind::LeftmostFirst);
    assert!(config.get_hybrid());
    assert!(!config.get_dfa());
    let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true)).unwrap();
    let anchored_start = hir.properties().look_set_prefix().contains(Look::Start);
    let anchored_end = hir.properties().look_set_suffix().contains(Look::End);
    let prefilter = AutomaticPrefilterHeap::from_pattern(pattern, &config).unwrap();
    match certificate.direction {
        HalfDirection::CoreForward => {
            assert!(anchored_start);
            assert_eq!(prefilter, AutomaticPrefilterHeap::ExcludedStartAnchored);
            let fresh = FreshAnchoredHalfHeap::from_pattern(pattern, max_input_bytes).unwrap();
            assert_eq!(certificate.eager_bytes, fresh.eager_bytes);
            assert_eq!(certificate.active_peak, fresh.forward_peak);
            assert_eq!(certificate.inactive_fresh_peak, fresh.reverse_fresh_peak);
            assert_eq!(certificate.peak, fresh.peak);
        }
        HalfDirection::ReverseAnchored => {
            assert!(!anchored_start && anchored_end);
            assert!(matches!(
                prefilter,
                AutomaticPrefilterHeap::SingleByte { byte: b'.', .. }
            ));
            // The immutable scalar-prefilter Arc is still retained in C. Only
            // unreachable search scratch is excluded here, not that owner.
            assert!(prefilter.requested_bytes() > 0);
        }
    }
}

#[cfg(test)]
pub(crate) fn assert_directional_replay(
    pattern: &str,
    inputs: &[String],
    certificate: &HalfSearchCertificate,
) {
    use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};
    use crate::schema_heap_layout::HeapLayout;
    use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
    use crate::schema_hybrid_heap::HybridHeap;
    use regex_automata::util::{prefilter::Prefilter, syntax};
    use regex_automata::{Anchored, HalfMatch, Input, MatchKind, hybrid::dfa::DFA, meta::Regex};
    use std::mem::size_of;

    assert_eq!(inputs.len(), certificate.counters.calls);
    let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true)).unwrap();
    let prefilter = if certificate.direction == HalfDirection::ReverseAnchored {
        Prefilter::from_hir_prefix(MatchKind::LeftmostFirst, &hir)
    } else {
        None
    };
    let forward_nfa = forward_nfa(pattern).unwrap();
    let candidate = candidate_hybrid(&forward_nfa, HybridDirection::Forward).unwrap();
    // Reconstruct the actual forward builder, including the end-only owner's
    // retained scalar prefilter and specialized start states. It stays cold.
    let forward = DFA::builder()
        .configure(
            candidate
                .get_config()
                .clone()
                .specialize_start_states(prefilter.is_some())
                .prefilter(prefilter),
        )
        .build_from_nfa(forward_nfa)
        .unwrap();
    let reverse =
        candidate_hybrid(&reverse_nfa(pattern).unwrap(), HybridDirection::Reverse).unwrap();
    let inactive = match certificate.direction {
        HalfDirection::CoreForward => &reverse,
        HalfDirection::ReverseAnchored => &forward,
    };
    let layout = HeapLayout::new(size_of::<usize>(), 16).unwrap();
    assert_eq!(
        certificate.inactive_fresh_peak,
        HybridHeap::from_fresh_dfa(inactive, layout).unwrap().peak
    );
    let mut forward_cache = forward.create_cache();
    let mut reverse_cache = reverse.create_cache();
    let cold_report = match certificate.direction {
        HalfDirection::CoreForward => reverse_cache.memory_usage(),
        HalfDirection::ReverseAnchored => forward_cache.memory_usage(),
    };
    let regex = Regex::builder()
        .configure(
            Regex::config()
                .match_kind(MatchKind::LeftmostFirst)
                .utf8_empty(true)
                .nfa_size_limit(Some(0x0001_0000))
                .hybrid_cache_capacity(0x0001_0000)
                .pool_capacity(1),
        )
        .syntax(syntax::Config::default().utf8(true))
        .build(pattern)
        .unwrap();
    let mut meta_cache = regex.create_cache();
    for input_text in inputs {
        assert!(input_text.len() < certificate.counters.endpoint);
        let input = Input::new(input_text).earliest(true);
        assert_eq!(input.get_anchored(), Anchored::No);
        assert_eq!(input.start(), 0);
        assert_eq!(input.end(), input.haystack().len());
        let actual = regex.search_half_with(&mut meta_cache, &input);
        let directional = match certificate.direction {
            HalfDirection::CoreForward => {
                forward.try_search_fwd(&mut forward_cache, &input).unwrap()
            }
            HalfDirection::ReverseAnchored => reverse
                .try_search_rev(&mut reverse_cache, &input.clone().anchored(Anchored::Yes))
                .unwrap()
                .map(|found| HalfMatch::new(found.pattern(), input.end())),
        };
        assert_eq!(actual, directional);
        assert_eq!(forward_cache.clear_count(), 0);
        assert_eq!(reverse_cache.clear_count(), 0);
        let (active, cold) = match certificate.direction {
            HalfDirection::CoreForward => (&forward_cache, &reverse_cache),
            HalfDirection::ReverseAnchored => (&reverse_cache, &forward_cache),
        };
        assert!(active.search_total_len() <= certificate.counters.searched_bytes);
        assert_eq!(cold.search_total_len(), 0);
        assert_eq!(cold.memory_usage(), cold_report);
    }
    // Replay observations corroborate the source/capacity argument; they do not
    // derive an all-input heap, CPU-work or unbounded cache-lifetime bound.
}

#[cfg(test)]
mod tests {
    use super::{CounterBound, HalfSearchCertificate, HalfSearchError};

    #[test]
    fn selected_half_search_counter_accepts_exact_finite_histories() {
        for (bytes, calls) in [(0_usize, 1_usize), (255, 1), (0x0001_0000, 2)] {
            assert_eq!(
                CounterBound::new(bytes, Some(calls), usize::MAX),
                Ok(CounterBound {
                    calls,
                    searched_bytes: bytes.checked_mul(calls).unwrap(),
                    endpoint: bytes.checked_add(1).unwrap(),
                })
            );
        }
        // Counter arithmetic only, not a guest heap/layout certificate.
        let max32 = usize::try_from(u32::MAX).unwrap();
        assert_eq!(
            CounterBound::new(0x0001_0000, Some(0xFFFF), max32),
            Ok(CounterBound {
                calls: 0xFFFF,
                searched_bytes: 0xFFFF_0000,
                endpoint: 0x0001_0001,
            })
        );
    }

    #[test]
    fn selected_half_search_counter_refuses_missing_or_vacuous_lifetime() {
        assert_eq!(
            CounterBound::new(0x0001_0000, None, usize::MAX),
            Err(HalfSearchError::MissingSearchBudget)
        );
        assert_eq!(
            CounterBound::new(0x0001_0000, Some(0), usize::MAX),
            Err(HalfSearchError::InvalidSearchBudget)
        );
        assert_eq!(
            HalfSearchCertificate::from_pattern("^a+$", 0x0001_0000, None),
            Err(HalfSearchError::MissingSearchBudget)
        );
    }

    #[test]
    fn selected_half_search_counter_refuses_wrap_and_unrepresentable_endpoint() {
        let max32 = usize::try_from(u32::MAX).unwrap();
        for (bytes, calls, maximum) in [
            (0x0001_0000, 0x0001_0000, max32),
            (max32, 1, max32),
            (usize::MAX, 1, usize::MAX),
            (usize::MAX.checked_div(2).unwrap(), 3, usize::MAX),
        ] {
            assert_eq!(
                CounterBound::new(bytes, Some(calls), maximum),
                Err(HalfSearchError::ProgressArithmetic)
            );
        }
    }

    #[test]
    fn selected_half_search_rejects_unproved_strategy_or_empty_retry() {
        for (pattern, error) in [
            ("[", HalfSearchError::Syntax),
            ("a+", HalfSearchError::Strategy),
            ("(?m)^a+", HalfSearchError::Strategy),
            (r"\^a+", HalfSearchError::Strategy),
            ("^a+|b+", HalfSearchError::Strategy),
            ("^a*$", HalfSearchError::EmptyMatches),
            ("^$", HalfSearchError::EmptyMatches),
        ] {
            assert_eq!(
                HalfSearchCertificate::from_pattern(pattern, 255, Some(2)),
                Err(error)
            );
        }
    }

    #[test]
    fn selected_half_search_keeps_quit_and_incomplete_graph_refusals() {
        use crate::schema_hybrid_bounds::{HybridDirection, QuitAdmissionError};
        use crate::schema_hybrid_capacity::CapacityError;
        use crate::schema_hybrid_graph::GraphError;
        use crate::schema_meta_heap::MetaHeapError;
        for (pattern, direction, reason) in [
            (
                r"^\bword",
                HybridDirection::Forward,
                CapacityError::Quit(QuitAdmissionError::UnicodeWordAssertion),
            ),
            (
                r"\.[a-zA-Z]{2,3}\b$",
                HybridDirection::Reverse,
                CapacityError::Quit(QuitAdmissionError::UnicodeWordAssertion),
            ),
            (
                "^[ab]*a[ab]{12}$",
                HybridDirection::Forward,
                CapacityError::Exploration(GraphError::Cleared),
            ),
        ] {
            assert_eq!(
                HalfSearchCertificate::from_pattern(pattern, 255, Some(2)),
                Err(HalfSearchError::Meta(MetaHeapError::Capacity {
                    direction,
                    reason,
                }))
            );
        }
    }
}

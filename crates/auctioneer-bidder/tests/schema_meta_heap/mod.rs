//! Conditional requested heap of one serial meta cache; excludes shared programs.

use crate::schema_cache_bounds::{BoundsError, forward_nfa, reverse_nfa};
use crate::schema_fallback_heap::FallbackHeap;
use crate::schema_heap_layout::HeapLayout;
use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
use crate::schema_hybrid_capacity::{CapacityError, ReportedCapacityProof};
use crate::schema_hybrid_heap::{CompleteHeapError, HybridHeap};
use crate::schema_hybrid_prefix::{OptionalPrefixEnvelope, PrefixError, for_meta_pattern};
use crate::schema_meta_eager::CoreEagerHeap;
use regex_automata::{
    MatchKind, meta,
    nfa::thompson::{NFA, WhichCaptures},
    util::syntax,
};
use regex_syntax::hir::Look;
use std::fmt;
use std::mem::size_of;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FreshMetaHeapError {
    Configuration,
    Syntax,
    NotStartAnchored,
    EmptyMatches,
    ProgressArithmetic,
    Meta(MetaHeapError),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FreshAnchoredHalfHeap {
    pub eager_bytes: usize,
    pub forward_peak: usize,
    pub reverse_fresh_peak: usize,
    pub max_searched_bytes: usize,
    pub peak: usize,
}

impl FreshAnchoredHalfHeap {
    /// Conditional native requested heap for new/one whole-haystack earliest
    /// half-search/drop. Not a reused-cache, CPU, program or physical bound.
    pub(crate) fn from_pattern(
        pattern: &str,
        max_input_bytes: usize,
    ) -> Result<Self, FreshMetaHeapError> {
        let config = meta::Regex::config()
            .match_kind(MatchKind::LeftmostFirst)
            .utf8_empty(true)
            .nfa_size_limit(Some(0x0001_0000))
            .hybrid_cache_capacity(0x0001_0000)
            .pool_capacity(1);
        require_fresh_half_configuration(&config)?;
        let max_searched_bytes = single_search_progress(max_input_bytes)?;
        let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true))
            .map_err(|_error| FreshMetaHeapError::Syntax)?;
        if !hir.properties().look_set_prefix().contains(Look::Start) {
            return Err(FreshMetaHeapError::NotStartAnchored);
        }
        let projection =
            |stage, reason| FreshMetaHeapError::Meta(MetaHeapError::Projection { stage, reason });
        let forward_nfa = forward_nfa(pattern)
            .map_err(|reason| projection(ProjectionStage::ForwardNfa, reason))?;
        // Otherwise one public call can retry an empty UTF-8 split internally.
        if forward_nfa.has_empty() {
            return Err(FreshMetaHeapError::EmptyMatches);
        }
        let reverse_nfa = reverse_nfa(pattern)
            .map_err(|reason| projection(ProjectionStage::ReverseNfa, reason))?;
        // Core only installs its hybrid pair when BOTH builders succeed.
        let forward =
            candidate_hybrid(&forward_nfa, HybridDirection::Forward).map_err(|_error| {
                FreshMetaHeapError::Meta(MetaHeapError::HybridBuild {
                    direction: HybridDirection::Forward,
                })
            })?;
        let reverse =
            candidate_hybrid(&reverse_nfa, HybridDirection::Reverse).map_err(|_error| {
                FreshMetaHeapError::Meta(MetaHeapError::HybridBuild {
                    direction: HybridDirection::Reverse,
                })
            })?;
        ReportedCapacityProof::from_non_quitting_dfa(&forward).map_err(|reason| {
            FreshMetaHeapError::Meta(MetaHeapError::Capacity {
                direction: HybridDirection::Forward,
                reason,
            })
        })?;
        let forward_heap = HybridHeap::from_complete_dfa(&forward)
            .map_err(|reason| {
                FreshMetaHeapError::Meta(MetaHeapError::Heap {
                    direction: HybridDirection::Forward,
                    reason,
                })
            })?
            .ok_or(FreshMetaHeapError::Meta(
                MetaHeapError::RecoveryUnavailable {
                    direction: HybridDirection::Forward,
                },
            ))?;
        let layout = HeapLayout::new(size_of::<usize>(), 16)
            .map_err(|reason| projection(ProjectionStage::Layout, BoundsError::from(reason)))?;
        let eager = CoreEagerHeap::from_nfa(&forward_nfa, layout)
            .map_err(|reason| projection(ProjectionStage::Eager, reason))?;
        let reverse_heap = HybridHeap::from_fresh_dfa(&reverse, layout)
            .map_err(|reason| projection(ProjectionStage::ReverseFresh, reason))?;
        // Start anchoring forces Core, without prefilters or reverse strategies.
        // Its available non-quitting forward hybrid cannot enter fallback.
        Self::compose(
            eager.total,
            forward_heap.peak,
            reverse_heap.peak,
            max_searched_bytes,
        )
    }

    fn compose(
        eager_bytes: usize,
        forward_peak: usize,
        reverse_fresh_peak: usize,
        max_searched_bytes: usize,
    ) -> Result<Self, FreshMetaHeapError> {
        let peak = eager_bytes
            .checked_add(forward_peak)
            .and_then(|bytes| bytes.checked_add(reverse_fresh_peak))
            .ok_or(FreshMetaHeapError::Meta(MetaHeapError::Projection {
                stage: ProjectionStage::Composition,
                reason: BoundsError::Arithmetic,
            }))?;
        Ok(Self {
            eager_bytes,
            forward_peak,
            reverse_fresh_peak,
            max_searched_bytes,
            peak,
        })
    }
}

fn require_fresh_half_configuration(config: &meta::Config) -> Result<(), FreshMetaHeapError> {
    if config.get_match_kind() != MatchKind::LeftmostFirst
        || !config.get_utf8_empty()
        || !matches!(config.get_which_captures(), WhichCaptures::All)
        || config.get_nfa_size_limit() != Some(0x0001_0000)
        || config.get_hybrid_cache_capacity() != 0x0001_0000
        || !config.get_byte_classes()
        || config.get_line_terminator() != b'\n'
        || config.get_pool_capacity() != 1
        || config.get_prefilter().is_some()
    {
        return Err(FreshMetaHeapError::Configuration);
    }
    require_half_engine_flags(config.get_hybrid(), config.get_dfa())
}

fn require_half_engine_flags(hybrid: bool, full_dfa: bool) -> Result<(), FreshMetaHeapError> {
    if hybrid && !full_dfa {
        Ok(())
    } else {
        Err(FreshMetaHeapError::Configuration)
    }
}

fn single_search_progress(max_input_bytes: usize) -> Result<usize, FreshMetaHeapError> {
    // With no empty retry, no clears and a fresh counter, there is one scan.
    // Reserve representable endpoint arithmetic even for the declared maximum.
    max_input_bytes
        .checked_add(1)
        .ok_or(FreshMetaHeapError::ProgressArithmetic)?;
    Ok(max_input_bytes)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ProjectionStage {
    ForwardNfa,
    ReverseNfa,
    ReverseFresh,
    Layout,
    Eager,
    Fallback,
    Composition,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum MetaHeapError {
    Projection {
        stage: ProjectionStage,
        reason: BoundsError,
    },
    HybridBuild {
        direction: HybridDirection,
    },
    Capacity {
        direction: HybridDirection,
        reason: CapacityError,
    },
    Heap {
        direction: HybridDirection,
        reason: CompleteHeapError,
    },
    RecoveryUnavailable {
        direction: HybridDirection,
    },
    Prefix(PrefixError),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MetaCacheHeap {
    pub eager_bytes: usize,
    pub forward_bytes: usize,
    pub reverse_bytes: usize,
    pub prefix_peak_bytes: usize,
    pub fallback_bytes: usize,
    pub live_envelope: usize,
    pub growth_overlap: usize,
    pub peak: usize,
}

impl MetaCacheHeap {
    /// One fixed native cache: new, serial earliest searches, then drop.
    /// All full haystacks in its lifetime must fit `max_input_bytes`.
    /// No reset, clone, rebind, shared-program or physical-heap bound is supplied.
    pub(crate) fn from_pattern(
        pattern: &str,
        max_input_bytes: usize,
    ) -> Result<Self, MetaHeapError> {
        let nfa = forward_nfa(pattern).map_err(|reason| MetaHeapError::Projection {
            stage: ProjectionStage::ForwardNfa,
            reason,
        })?;
        let reverse_nfa = reverse_nfa(pattern).map_err(|reason| MetaHeapError::Projection {
            stage: ProjectionStage::ReverseNfa,
            reason,
        })?;
        let layout = HeapLayout::new(size_of::<usize>(), 16).map_err(|reason| {
            MetaHeapError::Projection {
                stage: ProjectionStage::Layout,
                reason: BoundsError::from(reason),
            }
        })?;
        let eager =
            CoreEagerHeap::from_nfa(&nfa, layout).map_err(|reason| MetaHeapError::Projection {
                stage: ProjectionStage::Eager,
                reason,
            })?;
        let forward = directional_heap(&nfa, HybridDirection::Forward)?;
        let reverse = directional_heap(&reverse_nfa, HybridDirection::Reverse)?;
        let prefix = for_meta_pattern(pattern).map_err(MetaHeapError::Prefix)?;
        let fallback = FallbackHeap::from_nfa(&nfa, layout, max_input_bytes).map_err(|reason| {
            MetaHeapError::Projection {
                stage: ProjectionStage::Fallback,
                reason,
            }
        })?;
        Self::compose(&eager, &forward, &reverse, &prefix, &fallback).map_err(|reason| {
            MetaHeapError::Projection {
                stage: ProjectionStage::Composition,
                reason,
            }
        })
    }

    fn compose(
        eager: &CoreEagerHeap,
        forward: &HybridHeap,
        reverse: &HybridHeap,
        prefix: &OptionalPrefixEnvelope,
        fallback: &FallbackHeap,
    ) -> Result<Self, BoundsError> {
        // Prefix alternatives are mutually exclusive. Keep their full peak
        // funded even while a different owner's collection is growing.
        let prefix_peak_bytes = match prefix {
            OptionalPrefixEnvelope::NoConcat | OptionalPrefixEnvelope::ExcludedAnchored { .. } => 0,
            OptionalPrefixEnvelope::Alternatives(bound) => bound.peak_bytes,
        };
        let live_envelope = [
            eager.total,
            forward.live_envelope,
            reverse.live_envelope,
            prefix_peak_bytes,
            fallback.live_envelope,
        ]
        .into_iter()
        .try_fold(0_usize, |total, bytes| {
            total.checked_add(bytes).ok_or(BoundsError::Arithmetic)
        })?;
        let growth_overlap = [
            forward.growth_overlap,
            reverse.growth_overlap,
            fallback.growth_overlap,
        ]
        .into_iter()
        .fold(0, usize::max);
        let peak = live_envelope
            .checked_add(growth_overlap)
            .ok_or(BoundsError::Arithmetic)?;
        Ok(Self {
            eager_bytes: eager.total,
            forward_bytes: forward.live_envelope,
            reverse_bytes: reverse.live_envelope,
            prefix_peak_bytes,
            fallback_bytes: fallback.live_envelope,
            live_envelope,
            growth_overlap,
            peak,
        })
    }
}

fn directional_heap(nfa: &NFA, direction: HybridDirection) -> Result<HybridHeap, MetaHeapError> {
    let dfa = candidate_hybrid(nfa, direction)
        .map_err(|_error| MetaHeapError::HybridBuild { direction })?;
    ReportedCapacityProof::from_non_quitting_dfa(&dfa)
        .map_err(|reason| MetaHeapError::Capacity { direction, reason })?;
    HybridHeap::from_complete_dfa(&dfa)
        .map_err(|reason| MetaHeapError::Heap { direction, reason })?
        .ok_or(MetaHeapError::RecoveryUnavailable { direction })
}

impl fmt::Display for MetaCacheHeap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "eager={} forward={} reverse={} prefix-peak={} fallback={} live={} overlap={} peak={}",
            self.eager_bytes,
            self.forward_bytes,
            self.reverse_bytes,
            self.prefix_peak_bytes,
            self.fallback_bytes,
            self.live_envelope,
            self.growth_overlap,
            self.peak
        )
    }
}

impl fmt::Display for MetaHeapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let direction_name = |direction: &HybridDirection| match direction {
            HybridDirection::Forward => "forward",
            HybridDirection::Reverse => "reverse",
            HybridDirection::ReverseSharedStarts => "reverse-shared-starts",
        };
        match self {
            Self::Projection { stage, reason } => {
                let stage_name = match stage {
                    ProjectionStage::ForwardNfa => "forward_nfa",
                    ProjectionStage::ReverseNfa => "reverse_nfa",
                    ProjectionStage::ReverseFresh => "reverse_fresh",
                    ProjectionStage::Layout => "layout",
                    ProjectionStage::Eager => "eager",
                    ProjectionStage::Fallback => "fallback",
                    ProjectionStage::Composition => "composition",
                };
                write!(f, "{stage_name}_{}", projection_name(reason))
            }
            Self::HybridBuild { direction } => {
                write!(f, "{}_hybrid_build", direction_name(direction))
            }
            Self::Capacity { direction, reason } => {
                write!(f, "{}_capacity_{reason}", direction_name(direction))
            }
            Self::Heap { direction, reason } => {
                write!(f, "{}_heap_", direction_name(direction))?;
                match reason {
                    CompleteHeapError::Projection(projection) => {
                        f.write_str(projection_name(projection))
                    }
                    CompleteHeapError::Exploration(graph) => write!(f, "graph_{graph}"),
                }
            }
            Self::RecoveryUnavailable { direction } => {
                write!(f, "{}_recovery_unavailable", direction_name(direction))
            }
            Self::Prefix(reason) => write!(f, "prefix_{reason}"),
        }
    }
}

fn projection_name(reason: &BoundsError) -> &'static str {
    match reason {
        BoundsError::Arithmetic => "arithmetic_overflow",
        BoundsError::Compilation => "compilation_refused",
        BoundsError::Shape => "unexpected_shape",
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundsError, CoreEagerHeap, FallbackHeap, HybridHeap, MetaCacheHeap};
    use super::{
        FreshAnchoredHalfHeap, FreshMetaHeapError, require_fresh_half_configuration,
        require_half_engine_flags, single_search_progress,
    };
    use crate::schema_hybrid_prefix::{OptionalPrefixEnvelope, PrefixEnvelope};
    use regex_automata::{
        MatchKind,
        meta::{Config, Regex},
        nfa::thompson::WhichCaptures,
    };

    fn fresh_half_config() -> Config {
        Regex::config()
            .match_kind(MatchKind::LeftmostFirst)
            .utf8_empty(true)
            .nfa_size_limit(Some(0x0001_0000))
            .hybrid_cache_capacity(0x0001_0000)
            .pool_capacity(1)
    }

    #[test]
    fn fresh_anchored_half_composition_keeps_each_owner_and_checks_both_sums() {
        for (eager, forward, reverse, peak) in [
            (1_usize, 2_usize, 3_usize, 6_usize),
            (9, 2, 3, 14),
            (1, 8, 3, 12),
            (1, 2, 7, 10),
        ] {
            assert_eq!(
                FreshAnchoredHalfHeap::compose(eager, forward, reverse, 255),
                Ok(FreshAnchoredHalfHeap {
                    eager_bytes: eager,
                    forward_peak: forward,
                    reverse_fresh_peak: reverse,
                    max_searched_bytes: 255,
                    peak,
                })
            );
        }
        for (eager, forward, reverse) in [(usize::MAX, 1, 0), (usize::MAX - 1, 1, 1)] {
            assert_eq!(
                FreshAnchoredHalfHeap::compose(eager, forward, reverse, 0),
                Err(FreshMetaHeapError::Meta(super::MetaHeapError::Projection {
                    stage: super::ProjectionStage::Composition,
                    reason: BoundsError::Arithmetic,
                }))
            );
        }
    }

    #[test]
    fn fresh_anchored_half_configuration_rejects_independent_correspondence_drift() {
        use regex_automata::util::prefilter::Prefilter;
        assert_eq!(
            require_fresh_half_configuration(&fresh_half_config()),
            Ok(())
        );
        for changed in [
            fresh_half_config().match_kind(MatchKind::All),
            fresh_half_config().utf8_empty(false),
            fresh_half_config().which_captures(WhichCaptures::None),
            fresh_half_config().nfa_size_limit(Some(1024)),
            fresh_half_config().hybrid_cache_capacity(0x8000),
            fresh_half_config().byte_classes(false),
            fresh_half_config().line_terminator(b'x'),
            fresh_half_config().pool_capacity(2),
            fresh_half_config().hybrid(false),
            fresh_half_config().prefilter(Prefilter::new(MatchKind::LeftmostFirst, &[b"a"])),
        ] {
            assert_eq!(
                require_fresh_half_configuration(&changed),
                Err(FreshMetaHeapError::Configuration)
            );
        }
        for (hybrid, full_dfa, accepted) in [
            (true, false, true),
            (false, false, false),
            (true, true, false),
            (false, true, false),
        ] {
            assert_eq!(
                require_half_engine_flags(hybrid, full_dfa),
                if accepted {
                    Ok(())
                } else {
                    Err(FreshMetaHeapError::Configuration)
                }
            );
        }
    }

    #[test]
    fn fresh_anchored_half_progress_has_no_cumulative_history_or_offset_overflow() {
        for bytes in [0_usize, 1, 255, 0x0001_0000, usize::MAX - 1] {
            assert_eq!(single_search_progress(bytes), Ok(bytes));
        }
        assert_eq!(
            single_search_progress(usize::MAX),
            Err(FreshMetaHeapError::ProgressArithmetic)
        );
    }

    #[test]
    fn fresh_anchored_half_requires_whole_input_start_and_nonempty_matching() {
        for (pattern, error) in [
            ("[", FreshMetaHeapError::Syntax),
            ("a+", FreshMetaHeapError::NotStartAnchored),
            ("(?m)^a+", FreshMetaHeapError::NotStartAnchored),
            (r"\^a+", FreshMetaHeapError::NotStartAnchored),
            ("^a+|b+", FreshMetaHeapError::NotStartAnchored),
            ("^a*$", FreshMetaHeapError::EmptyMatches),
            ("^$", FreshMetaHeapError::EmptyMatches),
        ] {
            assert_eq!(
                FreshAnchoredHalfHeap::from_pattern(pattern, 255),
                Err(error)
            );
        }
        for pattern in ["^a+$", r"\A[a-z]+$", "^(?:a+|b+)$"] {
            let fresh = FreshAnchoredHalfHeap::from_pattern(pattern, 255).unwrap();
            let history = MetaCacheHeap::from_pattern(pattern, 255).unwrap();
            assert!(fresh.eager_bytes > 0);
            assert!(fresh.forward_peak > 0);
            assert!(fresh.reverse_fresh_peak > 0);
            assert_eq!(fresh.max_searched_bytes, 255);
            assert!(fresh.peak < history.peak);
        }
    }

    #[test]
    fn fresh_anchored_half_preserves_quit_and_input_refusals() {
        use crate::schema_hybrid_bounds::{HybridDirection, QuitAdmissionError};
        use crate::schema_hybrid_capacity::CapacityError;
        assert_eq!(
            FreshAnchoredHalfHeap::from_pattern(r"^\bword", 255),
            Err(FreshMetaHeapError::Meta(super::MetaHeapError::Capacity {
                direction: HybridDirection::Forward,
                reason: CapacityError::Quit(QuitAdmissionError::UnicodeWordAssertion),
            }))
        );
        assert_eq!(
            FreshAnchoredHalfHeap::from_pattern("^a+$", usize::MAX),
            Err(FreshMetaHeapError::ProgressArithmetic)
        );
    }

    #[test]
    fn fresh_anchored_half_rejects_unproved_cache_graphs_instead_of_excluding_fallback() {
        use crate::schema_hybrid_bounds::HybridDirection;
        use crate::schema_hybrid_capacity::CapacityError;
        use crate::schema_hybrid_graph::GraphError;
        assert_eq!(
            FreshAnchoredHalfHeap::from_pattern("^[ab]*a[ab]{12}$", 255),
            Err(FreshMetaHeapError::Meta(super::MetaHeapError::Capacity {
                direction: HybridDirection::Forward,
                reason: CapacityError::Exploration(GraphError::Cleared),
            }))
        );
    }

    #[test]
    fn meta_heap_constructor_keeps_same_pattern_owners_and_full_lifetime_input_bound() {
        use crate::schema_cache_bounds::forward_nfa;
        use crate::schema_heap_layout::HeapLayout;
        use std::mem::size_of;
        let pattern = "^(a+)$";
        let full = MetaCacheHeap::from_pattern(pattern, 0x0001_0000).unwrap();
        let empty = MetaCacheHeap::from_pattern(pattern, 0).unwrap();
        let nfa = forward_nfa(pattern).unwrap();
        let layout = HeapLayout::new(size_of::<usize>(), 16).unwrap();
        assert_eq!(
            full.eager_bytes,
            CoreEagerHeap::from_nfa(&nfa, layout).unwrap().total
        );
        assert_eq!(
            full.fallback_bytes,
            FallbackHeap::from_nfa(&nfa, layout, 0x0001_0000)
                .unwrap()
                .live_envelope
        );
        assert!(full.forward_bytes > 0 && full.reverse_bytes > 0);
        assert_eq!(full.prefix_peak_bytes, 0);
        assert_eq!(full.eager_bytes, empty.eager_bytes);
        assert_eq!(full.forward_bytes, empty.forward_bytes);
        assert_eq!(full.reverse_bytes, empty.reverse_bytes);
        assert!(full.fallback_bytes > empty.fallback_bytes);
        assert!(full.peak > empty.peak);
        let unanchored = MetaCacheHeap::from_pattern("[a-z]+[0-9]+", 128).unwrap();
        assert!(unanchored.prefix_peak_bytes > 0);
    }

    #[test]
    fn meta_heap_constructor_preserves_compilation_quit_and_graph_refusals() {
        use super::{HybridDirection, MetaHeapError, ProjectionStage};
        use crate::schema_hybrid_bounds::QuitAdmissionError;
        use crate::schema_hybrid_capacity::CapacityError;
        use crate::schema_hybrid_graph::GraphError;
        assert_eq!(
            MetaCacheHeap::from_pattern("[", 128),
            Err(MetaHeapError::Projection {
                stage: ProjectionStage::ForwardNfa,
                reason: BoundsError::Compilation
            })
        );
        assert_eq!(
            MetaCacheHeap::from_pattern(r"\bword", 128),
            Err(MetaHeapError::Capacity {
                direction: HybridDirection::Forward,
                reason: CapacityError::Quit(QuitAdmissionError::UnicodeWordAssertion),
            })
        );
        assert_eq!(
            MetaCacheHeap::from_pattern("[01]*1[01]{16}$", 128),
            Err(MetaHeapError::Capacity {
                direction: HybridDirection::Forward,
                reason: CapacityError::Exploration(GraphError::Cleared),
            })
        );
    }

    #[test]
    fn projection_refusals_name_every_independent_component() {
        use super::{MetaHeapError, ProjectionStage};
        for (stage, reason, expected) in [
            (
                ProjectionStage::ForwardNfa,
                BoundsError::Compilation,
                "forward_nfa_compilation_refused",
            ),
            (
                ProjectionStage::ReverseNfa,
                BoundsError::Compilation,
                "reverse_nfa_compilation_refused",
            ),
            (
                ProjectionStage::Layout,
                BoundsError::Shape,
                "layout_unexpected_shape",
            ),
            (
                ProjectionStage::Eager,
                BoundsError::Arithmetic,
                "eager_arithmetic_overflow",
            ),
            (
                ProjectionStage::Fallback,
                BoundsError::Arithmetic,
                "fallback_arithmetic_overflow",
            ),
            (
                ProjectionStage::Composition,
                BoundsError::Arithmetic,
                "composition_arithmetic_overflow",
            ),
        ] {
            assert_eq!(
                MetaHeapError::Projection { stage, reason }.to_string(),
                expected
            );
        }
        let malformed = MetaCacheHeap::from_pattern("[", 128).unwrap_err();
        assert_eq!(malformed.to_string(), "forward_nfa_compilation_refused");
    }

    fn hybrid(live: usize, overlap: usize) -> HybridHeap {
        HybridHeap {
            live_envelope: live,
            growth_overlap: overlap,
            peak: live.checked_add(overlap).unwrap(),
        }
    }

    fn eager(total: usize) -> CoreEagerHeap {
        CoreEagerHeap {
            captures: total,
            onepass: 0,
            total,
        }
    }

    fn fallback(live: usize, overlap: usize) -> FallbackHeap {
        FallbackHeap {
            pike_sets: live,
            pike_slots: 0,
            pike_stack: 0,
            backtrack_bitmap: 0,
            backtrack_stack: 0,
            backtrack_max_span: Some(0),
            live_envelope: live,
            growth_overlap: overlap,
            peak: live.checked_add(overlap).unwrap(),
        }
    }

    fn prefix(peak: usize) -> OptionalPrefixEnvelope {
        OptionalPrefixEnvelope::Alternatives(PrefixEnvelope {
            candidates: 2,
            peak_bytes: peak,
            required_reported_bytes: 13,
        })
    }

    #[test]
    fn meta_heap_sums_coexisting_owners_and_one_largest_serial_growth_overlap() {
        assert_eq!(
            MetaCacheHeap::compose(
                &eager(11),
                &hybrid(17, 5),
                &hybrid(23, 9),
                &prefix(31),
                &fallback(41, 7),
            ),
            Ok(MetaCacheHeap {
                eager_bytes: 11,
                forward_bytes: 17,
                reverse_bytes: 23,
                prefix_peak_bytes: 31,
                fallback_bytes: 41,
                live_envelope: 123,
                growth_overlap: 9,
                peak: 132,
            })
        );
    }

    #[test]
    fn meta_heap_varies_every_live_owner_independently() {
        for (e, f, r, p, b, live, peak) in [
            (12, 17, 23, 31, 41, 124, 133),
            (11, 18, 23, 31, 41, 124, 133),
            (11, 17, 24, 31, 41, 124, 133),
            (11, 17, 23, 32, 41, 124, 133),
            (11, 17, 23, 31, 42, 124, 133),
            (7, 13, 19, 0, 29, 68, 77),
        ] {
            let bound = MetaCacheHeap::compose(
                &eager(e),
                &hybrid(f, 5),
                &hybrid(r, 9),
                &prefix(p),
                &fallback(b, 7),
            )
            .unwrap();
            assert_eq!(bound.live_envelope, live);
            assert_eq!(bound.peak, peak);
        }
    }

    #[test]
    fn each_serial_growth_owner_can_independently_determine_the_peak() {
        for (f, r, b, overlap, peak) in [
            (10, 9, 7, 10, 133),
            (5, 9, 7, 9, 132),
            (5, 4, 7, 7, 130),
            (5, 9, 12, 12, 135),
        ] {
            let bound = MetaCacheHeap::compose(
                &eager(11),
                &hybrid(17, f),
                &hybrid(23, r),
                &prefix(31),
                &fallback(41, b),
            )
            .unwrap();
            assert_eq!(bound.live_envelope, 123);
            assert_eq!(bound.growth_overlap, overlap);
            assert_eq!(bound.peak, peak);
        }
    }

    #[test]
    fn absent_prefix_owners_do_not_invent_residency() {
        for absent in [
            OptionalPrefixEnvelope::NoConcat,
            OptionalPrefixEnvelope::ExcludedAnchored {
                structural_candidates: 3,
            },
        ] {
            let bound = MetaCacheHeap::compose(
                &eager(11),
                &hybrid(17, 5),
                &hybrid(23, 9),
                &absent,
                &fallback(41, 7),
            )
            .unwrap();
            assert_eq!(bound.prefix_peak_bytes, 0);
            assert_eq!(bound.live_envelope, 92);
            assert_eq!(bound.peak, 101);
        }
    }

    #[test]
    fn meta_heap_refuses_sum_and_final_growth_overflow_separately() {
        for (e, f, r, p, b, overlap) in [
            (usize::MAX, 1, 0, 0, 0, 0),
            (0, usize::MAX, 1, 0, 0, 0),
            (0, 0, usize::MAX, 1, 0, 0),
            (0, 0, 0, usize::MAX, 1, 0),
            (usize::MAX - 1, 1, 0, 0, 0, 1),
        ] {
            assert_eq!(
                MetaCacheHeap::compose(
                    &eager(e),
                    &hybrid(f, overlap),
                    &hybrid(r, 0),
                    &prefix(p),
                    &fallback(b, 0),
                ),
                Err(BoundsError::Arithmetic)
            );
        }
    }
}

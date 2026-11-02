//! Source-derived retained literal owners, not constructor or whole-program peaks.

use crate::schema_heap_layout::{HeapLayout, LayoutError, native_arc_value_layout};
use regex_automata::{
    MatchKind as RegexMatchKind,
    meta::Config,
    util::{prefilter::Prefilter, syntax},
};
use regex_syntax::hir::{
    Look,
    literal::{ExtractKind, Extractor, Seq},
};
use std::alloc::Layout;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AutomaticPrefilterHeap {
    ExcludedStartAnchored,
    SingleByte { byte: u8, requested_bytes: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AutomaticPrefilterError {
    Configuration,
    Syntax,
    Scope,
    Layout,
}

impl AutomaticPrefilterHeap {
    /// Conditional native matcher ownership under the pinned meta constructor.
    /// A public same-type layout witness is not hidden allocation identity.
    pub(crate) fn from_pattern(
        pattern: &str,
        config: &Config,
    ) -> Result<Self, AutomaticPrefilterError> {
        if !config.get_auto_prefilter()
            || config.get_prefilter().is_some()
            || config.get_match_kind() != RegexMatchKind::LeftmostFirst
        {
            return Err(AutomaticPrefilterError::Configuration);
        }
        let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true))
            .map_err(|_error| AutomaticPrefilterError::Syntax)?;
        if hir.properties().look_set_prefix().contains(Look::Start) {
            return Ok(Self::ExcludedStartAnchored);
        }
        // End anchoring rules out both literal-only short circuits in the
        // pinned meta constructor. Other pattern classes remain unqualified.
        if !hir.properties().look_set_suffix().contains(Look::End) {
            return Err(AutomaticPrefilterError::Scope);
        }
        // Same single-HIR prefix extraction/optimization as the pinned library,
        // using its public structural APIs, not a second regex parser.
        let mut extractor = Extractor::new();
        extractor.kind(ExtractKind::Prefix);
        let mut prefixes = Seq::empty();
        prefixes.union(&mut extractor.extract(&hir));
        prefixes.optimize_for_prefix_by_preference();
        let literals = prefixes.literals().ok_or(AutomaticPrefilterError::Scope)?;
        let [literal] = literals else {
            return Err(AutomaticPrefilterError::Scope);
        };
        let [byte] = literal.as_bytes() else {
            return Err(AutomaticPrefilterError::Scope);
        };
        // Choice::new selects Memchr first for one one-byte needle. This
        // independent construction witnesses that TYPE's compiler layout;
        // source propagation clones one Arc rather than allocating per handle.
        let witness = Prefilter::from_hir_prefix(RegexMatchKind::LeftmostFirst, &hir)
            .ok_or(AutomaticPrefilterError::Layout)?;
        let payload = witness.resource_layout().matcher_payload();
        if payload != Layout::new::<u8>() || witness.memory_usage() != 0 || !witness.is_fast() {
            return Err(AutomaticPrefilterError::Layout);
        }
        let requested_bytes = native_arc_value_layout(payload)
            .map_err(|_error| AutomaticPrefilterError::Layout)?
            .size();
        Ok(Self::SingleByte {
            byte: *byte,
            requested_bytes,
        })
    }

    pub(crate) fn requested_bytes(self) -> usize {
        match self {
            Self::ExcludedStartAnchored => 0,
            Self::SingleByte {
                requested_bytes, ..
            } => requested_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiteralStart {
    Anchored,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LiteralHeapError {
    Empty,
    PatternCount,
    Arithmetic,
    Layout(LayoutError),
}

impl From<LayoutError> for LiteralHeapError {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Arithmetic => Self::Arithmetic,
            LayoutError::Shape => Self::Layout(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LiteralDfaHeap {
    pub max_states: usize,
    pub transitions: usize,
    pub match_headers: usize,
    pub match_ids: usize,
    pub pattern_lengths: usize,
    pub total: usize,
}

impl LiteralDfaHeap {
    pub(crate) fn from_lengths(
        lengths: &[usize],
        start: LiteralStart,
    ) -> Result<Self, LiteralHeapError> {
        if lengths.is_empty() || lengths.contains(&0) {
            return Err(LiteralHeapError::Empty);
        }
        // regex-automata's literal wrapper selects contiguous NFA above 500.
        // That separate owner cannot be represented by this DFA component.
        if lengths.len() > 500 {
            return Err(LiteralHeapError::PatternCount);
        }
        let bytes = lengths
            .iter()
            .try_fold(0_usize, |sum, next| sum.checked_add(*next))
            .ok_or(LiteralHeapError::Arithmetic)?;
        // aho-corasick 1.1.4 noncontiguous.rs:970-985,1098-1143:
        // four sentinels, then at most one trie state for each literal byte.
        let nfa_states = bytes.checked_add(4).ok_or(LiteralHeapError::Arithmetic)?;
        let max_states = match start {
            LiteralStart::Anchored => nfa_states,
            LiteralStart::Both => nfa_states
                .checked_mul(2)
                .and_then(|states| states.checked_sub(4))
                .ok_or(LiteralHeapError::Arithmetic)?,
        };
        let layout = HeapLayout::new(size_of::<usize>(), 16)?;
        // dfa.rs:431-534: no EOI alphabet column; both-start duplication is
        // already in max_states. Outer buffers are created at their final
        // lengths, filled without growth, and never inherit trie capacity.
        let transitions = layout.repeat_vector(
            size_of::<u32>(),
            max_states
                .checked_mul(256)
                .ok_or(LiteralHeapError::Arithmetic)?,
        )?;
        let match_headers =
            layout.repeat_vector(size_of::<Vec<aho_corasick::PatternID>>(), max_states)?;
        let pattern_lengths = layout.repeat_vector(size_of::<u32>(), lengths.len())?;
        // Under LeftmostFirst each terminal gets its ID once; nonterminals
        // copy one failure list once. An ID can recur in different rows.
        // dfa.rs:170-184 appends, never shrinks these inner vectors. Pay the
        // pinned amortized-capacity envelope, including its minimum allocation.
        let per_match = layout
            .append_vector(size_of::<aho_corasick::PatternID>(), lengths.len())?
            .retained_bytes;
        let match_ids = per_match
            .checked_mul(max_states)
            .ok_or(LiteralHeapError::Arithmetic)?;
        let total = [transitions, match_headers, match_ids, pattern_lengths]
            .into_iter()
            .try_fold(0_usize, usize::checked_add)
            .ok_or(LiteralHeapError::Arithmetic)?;
        Ok(Self {
            max_states,
            transitions,
            match_headers,
            match_ids,
            pattern_lengths,
            total,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_meter::measure;
    use aho_corasick::{MatchKind, PatternID, StartKind, dfa::DFA};
    use std::{hint, mem::size_of};

    #[test]
    fn fake_automatic_prefilter_excludes_anchored_and_funds_one_scalar_matcher() {
        let config = Config::new();
        for pattern in ["^fake[0-9]+$", "^$", "(?i)^fake"] {
            assert_eq!(
                AutomaticPrefilterHeap::from_pattern(pattern, &config),
                Ok(AutomaticPrefilterHeap::ExcludedStartAnchored)
            );
        }
        for (pattern, byte) in [("q[a-z]{2,3}$", b'q'), (r"\.[a-z]{2,3}$", b'.')] {
            let heap = AutomaticPrefilterHeap::from_pattern(pattern, &config).unwrap();
            let expected = if usize::BITS == 64 { 24 } else { 12 };
            assert_eq!(
                heap,
                AutomaticPrefilterHeap::SingleByte {
                    byte,
                    requested_bytes: expected
                }
            );
            assert_eq!(heap.requested_bytes(), expected);
        }
    }

    #[test]
    fn fake_automatic_prefilter_refuses_unproved_literal_shapes_and_syntax() {
        for pattern in ["qr[a-z]+$", "(q|x)[a-z]+$", "q[a-z]+", "q", "[a-z]+$"] {
            assert_eq!(
                AutomaticPrefilterHeap::from_pattern(pattern, &Config::new()),
                Err(AutomaticPrefilterError::Scope),
                "{pattern}"
            );
        }
        assert_eq!(
            AutomaticPrefilterHeap::from_pattern("[", &Config::new()),
            Err(AutomaticPrefilterError::Syntax)
        );
    }

    #[test]
    fn fake_automatic_prefilter_configuration_axes_refuse_independently() {
        use regex_automata::{MatchKind as RegexMatchKind, util::prefilter::Prefilter};
        for config in [
            Config::new().auto_prefilter(false),
            Config::new().match_kind(RegexMatchKind::All),
            Config::new().prefilter(Prefilter::new(RegexMatchKind::LeftmostFirst, &[b"q"])),
        ] {
            assert_eq!(
                AutomaticPrefilterHeap::from_pattern("q[a-z]+$", &config),
                Err(AutomaticPrefilterError::Configuration)
            );
            assert_eq!(
                AutomaticPrefilterHeap::from_pattern("^q[a-z]+$", &config),
                Err(AutomaticPrefilterError::Configuration)
            );
        }
    }

    #[test]
    fn fake_scalar_matcher_allocates_once_and_clones_share_until_last_drop() {
        use regex_automata::{MatchKind as RegexMatchKind, util::prefilter::Prefilter};
        let expected = if usize::BITS == 64 { 24_u64 } else { 12_u64 };
        let (prefilter, created) =
            measure(|| Prefilter::new(RegexMatchKind::LeftmostFirst, &[b"q"]).unwrap());
        assert_eq!(created.json()["allocation_calls"], 1_u64);
        assert_eq!(created.json()["allocated_bytes"], expected);
        assert_eq!(
            created.json()["retained_bytes"],
            i64::try_from(expected).unwrap()
        );
        assert_eq!(prefilter.memory_usage(), 0);
        let (clones, copied) =
            measure(|| (prefilter.clone(), prefilter.clone(), prefilter.clone()));
        assert_eq!(copied.json()["allocated_bytes"], 0_u64);
        assert_eq!(copied.json()["allocation_calls"], 0_u64);
        let ((), original_dropped) = measure(|| drop(prefilter));
        assert_eq!(original_dropped.json()["retained_bytes"], 0_i64);
        let ((), released) = measure(|| drop(clones));
        assert_eq!(
            released.json()["retained_bytes"],
            i64::try_from(expected).unwrap().checked_neg().unwrap()
        );
        let (independent, second_created) =
            measure(|| Prefilter::new(RegexMatchKind::LeftmostFirst, &[b"x"]).unwrap());
        assert_eq!(second_created.json()["allocation_calls"], 1_u64);
        assert_eq!(second_created.json()["allocated_bytes"], expected);
        drop(independent);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_literal_dfa_counts_bytes_and_start_population_independently() {
        assert_eq!(
            LiteralDfaHeap::from_lengths(&[3, 5], LiteralStart::Both),
            Ok(LiteralDfaHeap {
                max_states: 20,
                transitions: 20_480,
                match_headers: 480,
                match_ids: 320,
                pattern_lengths: 8,
                total: 21_288
            })
        );
        assert_eq!(
            LiteralDfaHeap::from_lengths(&[3, 5], LiteralStart::Anchored),
            Ok(LiteralDfaHeap {
                max_states: 12,
                transitions: 12_288,
                match_headers: 288,
                match_ids: 192,
                pattern_lengths: 8,
                total: 12_776
            })
        );
        assert_eq!(
            LiteralDfaHeap::from_lengths(&[4, 5], LiteralStart::Both)
                .unwrap()
                .total,
            23_416
        );
        assert_eq!(
            LiteralDfaHeap::from_lengths(&[8], LiteralStart::Both)
                .unwrap()
                .total,
            21_284
        );
        assert_eq!(
            LiteralDfaHeap::from_lengths(&[2, 3, 3], LiteralStart::Both)
                .unwrap()
                .total,
            21_452
        );
    }

    #[test]
    fn fake_empty_large_and_overflowing_literal_sets_refuse() {
        assert_eq!(
            LiteralDfaHeap::from_lengths(&[], LiteralStart::Both),
            Err(LiteralHeapError::Empty)
        );
        assert_eq!(
            LiteralDfaHeap::from_lengths(&[1, 0], LiteralStart::Both),
            Err(LiteralHeapError::Empty)
        );
        assert_eq!(
            LiteralDfaHeap::from_lengths(&vec![1; 501], LiteralStart::Both),
            Err(LiteralHeapError::PatternCount)
        );
        assert_eq!(
            LiteralDfaHeap::from_lengths(&vec![1; 500], LiteralStart::Both)
                .unwrap()
                .max_states,
            1004
        );
        for lengths in [
            &[usize::MAX][..],
            &[usize::MAX - 4][..],
            &[usize::MAX.checked_div(2).unwrap()][..],
        ] {
            assert_eq!(
                LiteralDfaHeap::from_lengths(lengths, LiteralStart::Both),
                Err(LiteralHeapError::Arithmetic)
            );
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_match_rows_vary_pattern_count_without_changing_trie_byte_budget() {
        for (lengths, pattern_lengths, match_ids, total) in [
            (vec![1, 2, 3, 4], 16, 768, 25_936),
            (vec![1, 1, 2, 3, 3], 20, 960, 26_132),
        ] {
            let bound = LiteralDfaHeap::from_lengths(&lengths, LiteralStart::Both).unwrap();
            assert_eq!(bound.max_states, 24);
            assert_eq!(bound.transitions, 24_576);
            assert_eq!(bound.match_headers, 576);
            assert_eq!(bound.pattern_lengths, pattern_lengths);
            assert_eq!(bound.match_ids, match_ids);
            assert_eq!(bound.total, total);
        }
    }

    #[test]
    fn fake_match_id_append_crosses_the_minimum_capacity_with_observed_release() {
        for (count, capacity, retained) in [(4, 4, 16_i64), (5, 8, 32)] {
            let (ids, metrics) = measure(|| {
                let mut ids = Vec::new();
                for index in 0..count {
                    ids.push(PatternID::must(index));
                }
                hint::black_box(ids)
            });
            assert_eq!(ids.capacity(), capacity);
            assert_eq!(metrics.json()["retained_bytes"], retained);
            let ((), released) = measure(|| drop(hint::black_box(ids)));
            assert_eq!(released.json()["retained_bytes"], -retained);
        }
    }

    #[test]
    fn fake_dfa_checks_downstream_product_request_and_sum_overflow() {
        let native_states = usize::try_from(isize::MAX)
            .unwrap()
            .checked_div(1024)
            .unwrap();
        for lengths in [
            vec![usize::MAX, 1],
            vec![usize::MAX.checked_div(256).unwrap()],
            vec![usize::MAX.checked_div(512).unwrap()],
            vec![native_states],
        ] {
            assert_eq!(
                LiteralDfaHeap::from_lengths(&lengths, LiteralStart::Anchored),
                Err(LiteralHeapError::Arithmetic)
            );
        }
        // Keep every transition allocation valid while separately overflowing
        // the aggregate of match-ID requests, then the final buffer sum.
        let bytes = native_states - 4;
        for count in [500, 128] {
            let mut lengths = vec![1; count];
            lengths[0] = bytes - (count - 1);
            assert_eq!(
                LiteralDfaHeap::from_lengths(&lengths, LiteralStart::Anchored),
                Err(LiteralHeapError::Arithmetic)
            );
        }
    }

    #[test]
    fn fake_dfa_pattern_ids_are_the_reviewed_transparent_u32_layout() {
        assert_eq!((size_of::<PatternID>(), align_of::<PatternID>()), (4, 4));
        assert_eq!(size_of::<Vec<PatternID>>(), size_of::<Vec<u32>>());
    }

    #[test]
    fn fake_fresh_dfa_retention_and_release_fit_both_start_envelopes() {
        for needles in [
            vec![b"f".as_slice()],
            vec![b"fake_alpha".as_slice(), b"fake_beta".as_slice()],
            vec![b"a".as_slice(), b"a".as_slice(), b"aa".as_slice()],
            vec![b"a".as_slice(); 5],
            vec![b"cabz".as_slice(), b"ab".as_slice()],
            vec![
                b"a".as_slice(),
                b"aa".as_slice(),
                b"aaa".as_slice(),
                b"aaaa".as_slice(),
            ],
        ] {
            let lengths: Vec<_> = needles.iter().map(|needle| needle.len()).collect();
            for (mode, start) in [
                (LiteralStart::Anchored, StartKind::Anchored),
                (LiteralStart::Both, StartKind::Both),
            ] {
                let bound = LiteralDfaHeap::from_lengths(&lengths, mode).unwrap();
                let (dfa, construction) = measure(|| {
                    DFA::builder()
                        .match_kind(MatchKind::LeftmostFirst)
                        .start_kind(start)
                        .byte_classes(false)
                        .prefilter(false)
                        .build(hint::black_box(&needles))
                        .unwrap()
                });
                let observed = construction.json();
                let retained = observed["retained_bytes"].as_i64().unwrap();
                assert!(retained > 0);
                assert!(observed["allocation_calls"].as_u64().unwrap() > 0);
                assert!(
                    usize::try_from(retained).unwrap() <= bound.total,
                    "{lengths:?} {mode:?}"
                );
                let ((), released) = measure(|| drop(hint::black_box(dfa)));
                assert_eq!(
                    released.json()["retained_bytes"],
                    retained.checked_neg().unwrap()
                );
            }
        }
    }
}

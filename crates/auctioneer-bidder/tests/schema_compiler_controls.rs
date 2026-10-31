//! Synthetic native compatibility controls, not catalog or full engine qualification.

#![cfg(not(target_arch = "wasm32"))]

mod schema_formats;
use schema_formats::{reference_hostname_format, reference_uuid_format};

mod schema_vendor;

mod schema_cache_model;
use schema_cache_model as schema_cache_bounds;

mod schema_backtrack_bounds;

mod schema_onepass_bounds;

mod schema_hybrid_bounds;

mod schema_hybrid_population;

mod schema_heap_layout;

mod schema_meta_eager;

mod schema_fallback_heap;

mod schema_meta_heap;

mod schema_hybrid_heap;

mod schema_hybrid_graph;

mod schema_hybrid_capacity;

mod schema_hybrid_prefix;

#[cfg(test)]
mod tests {
    use auctioneer_bidder::schema::{
        SchemaCostError, SchemaEvaluationCost, SchemaEvaluationError, SchemaInputError,
        SchemaInputLimits, SchemaInputStats, SchemaWorkBudget, SchemaWorkLimits,
        measure_schema_input,
    };
    use serde_json::{Value as Json, json};
    use std::cell::Cell;
    use std::io::{Write as _, stderr};
    use toml::Value;

    #[test]
    fn hostname_fresh_half_search_keeps_reverse_construction_without_unreachable_fallbacks() {
        use super::schema_meta_heap::FreshAnchoredHalfHeap;
        use std::mem::size_of;
        let fresh =
            FreshAnchoredHalfHeap::from_pattern(super::schema_formats::HOSTNAME_SOURCE, 255)
                .unwrap();
        let history = super::schema_meta_heap::MetaCacheHeap::from_pattern(
            super::schema_formats::HOSTNAME_SOURCE,
            255,
        )
        .unwrap();
        assert_eq!(fresh.max_searched_bytes, 255);
        assert!(fresh.reverse_fresh_peak > 0);
        assert!(fresh.peak < history.peak);
        if size_of::<usize>() == 8 {
            assert_eq!(fresh.eager_bytes, 112);
            assert_eq!(fresh.forward_peak, 151_821);
            // 4112 sparse + 72 starts + 384 transitions + 96 handles + 116 map
            // + 32 shared backing + 9 temporary + 188 serial overlap.
            assert_eq!(fresh.reverse_fresh_peak, 5009);
            assert_eq!(fresh.peak, 156_942);
        }
        writeln!(
            stderr(),
            "fresh anchored hostname half-search: eager={} forward-peak={} reverse-fresh-peak={} max-searched-bytes={} peak={}",
            fresh.eager_bytes,
            fresh.forward_peak,
            fresh.reverse_fresh_peak,
            fresh.max_searched_bytes,
            fresh.peak
        )
        .unwrap();
    }

    #[test]
    fn hostname_fixed_cache_composition_retains_both_hybrid_and_fallback_owners() {
        use super::schema_meta_heap::MetaCacheHeap;
        use std::mem::size_of;
        let full =
            MetaCacheHeap::from_pattern(super::schema_formats::HOSTNAME_SOURCE, 255).unwrap();
        let empty = MetaCacheHeap::from_pattern(super::schema_formats::HOSTNAME_SOURCE, 0).unwrap();
        let word64 = size_of::<usize>() == 8;
        assert_eq!(full.eager_bytes, if word64 { 112 } else { 56 });
        assert!(full.forward_bytes > 0);
        assert!(full.reverse_bytes > 0);
        assert_eq!(full.prefix_peak_bytes, 0);
        assert_eq!(full.fallback_bytes, if word64 { 876_768 } else { 444_776 });
        if word64 {
            // Directional literals predate this composition; their asymmetry
            // detects reusing the forward program as the reverse owner.
            assert_eq!(
                full,
                MetaCacheHeap {
                    eager_bytes: 112,
                    forward_bytes: 135_569,
                    reverse_bytes: 106_372,
                    prefix_peak_bytes: 0,
                    fallback_bytes: 876_768,
                    live_envelope: 1_118_821,
                    growth_overlap: 411_744,
                    peak: 1_530_565,
                }
            );
        }
        assert!(full.fallback_bytes > empty.fallback_bytes);
        assert!(full.peak > empty.peak);
        writeln!(stderr(), "fixed native hostname cache: {full}").unwrap();
    }

    #[test]
    fn hostname_fallback_bound_keeps_long_input_pike_and_short_input_backtracker() {
        use super::schema_cache_bounds::forward_nfa;
        use super::schema_fallback_heap::FallbackHeap;
        use super::schema_heap_layout::HeapLayout;
        let nfa = forward_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap();
        for word in [4, 8] {
            let layout = HeapLayout::new(word, 16).unwrap();
            let full = FallbackHeap::from_nfa(&nfa, layout, 255).unwrap();
            let short = FallbackHeap::from_nfa(&nfa, layout, 0).unwrap();
            assert_eq!(full.backtrack_max_span, Some(128));
            assert_eq!(short.backtrack_max_span, Some(0));
            assert_eq!(full.pike_sets, 4240);
            assert_eq!(full.pike_slots, if word == 4 { 17024 } else { 34048 });
            assert_eq!(full.pike_stack, if word == 4 { 3192 } else { 6384 });
            assert_eq!(full.backtrack_bitmap, if word == 4 { 8552 } else { 8560 });
            assert_eq!(
                full.backtrack_stack,
                if word == 4 { 411_768 } else { 823_536 }
            );
            assert_eq!(full.pike_sets, short.pike_sets);
            assert_eq!(full.pike_slots, short.pike_slots);
            assert_eq!(full.pike_stack, short.pike_stack);
            assert!(full.backtrack_bitmap > short.backtrack_bitmap);
            assert!(full.backtrack_stack > short.backtrack_stack);
        }
    }

    #[test]
    fn hostname_fresh_core_bound_funds_an_optional_onepass_despite_candidate_refusal() {
        use super::schema_cache_bounds::forward_nfa;
        use super::schema_heap_layout::HeapLayout;
        use super::schema_meta_eager::CoreEagerHeap;
        use super::schema_onepass_bounds::candidate_onepass;
        let nfa = forward_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap();
        let error = candidate_onepass(&nfa).unwrap_err();
        assert!(error.to_string().contains("pattern is not one-pass:"));
        for (word, captures, onepass, total) in [(4, 32, 24, 56), (8, 64, 48, 112)] {
            assert_eq!(
                CoreEagerHeap::from_nfa(&nfa, HeapLayout::new(word, 16).unwrap()),
                Ok(CoreEagerHeap {
                    captures,
                    onepass,
                    total
                })
            );
        }
    }

    #[test]
    fn hostname_optional_prefix_refusal_is_distinct_from_source_proven_exclusion() {
        use super::schema_hybrid_capacity::CapacityError;
        use super::schema_hybrid_graph::GraphError;
        use super::schema_hybrid_prefix::{OptionalPrefixEnvelope, PrefixEnvelope, PrefixError};
        assert_eq!(
            super::schema_hybrid_prefix::candidates(super::schema_formats::HOSTNAME_SOURCE)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            PrefixEnvelope::from_pattern(super::schema_formats::HOSTNAME_SOURCE),
            Err(PrefixError::Capacity(CapacityError::Exploration(
                GraphError::Cleared
            )))
        );
        assert_eq!(
            super::schema_hybrid_prefix::for_meta_pattern(super::schema_formats::HOSTNAME_SOURCE),
            Ok(OptionalPrefixEnvelope::ExcludedAnchored {
                structural_candidates: 3
            })
        );
    }

    #[test]
    fn hostname_format_forward_nfa_uses_all_real_capture_slots() {
        use super::schema_cache_bounds::{NfaDimensions, PikeDimensions, forward_nfa};
        let nfa = forward_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap();
        let shape = NfaDimensions::from_nfa(&nfa).unwrap();
        assert_eq!(
            shape,
            NfaDimensions {
                states: 265,
                slots: 8,
                patterns: 1,
                branch_frames: 125,
                capture_states: 8,
            }
        );
        let storage = shape.pike_storage().unwrap();
        assert_eq!(
            storage,
            PikeDimensions {
                set_entries: 1060,
                table_entries: 4256,
                meta_capture_entries: 8,
                stack_frames: 133,
            }
        );
        writeln!(
            stderr(),
            "hostname forward-nfa logical audit: {shape} {storage}"
        )
        .unwrap();
    }

    #[test]
    fn hostname_format_backtracker_uses_its_full_input_eligibility_boundary() {
        use super::schema_backtrack_bounds::BacktrackDimensions;
        use super::schema_cache_bounds::{NfaDimensions, forward_nfa};
        let nfa = forward_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap();
        let shape = NfaDimensions::from_nfa(&nfa).unwrap();
        for (word_bits, visited_words) in [(32, 1069), (64, 535)] {
            assert_eq!(
                BacktrackDimensions::for_earliest(shape, 128, 128, word_bits),
                Ok(Some(BacktrackDimensions {
                    visited_words,
                    stack_frames: 17_157
                }))
            );
            for length in [129, 255] {
                assert_eq!(
                    BacktrackDimensions::for_earliest(shape, length, length, word_bits),
                    Ok(None)
                );
            }
        }
    }

    #[test]
    fn hostname_format_onepass_candidate_refuses_despite_fitting_slot_count() {
        use super::schema_cache_bounds::{NfaDimensions, forward_nfa};
        use super::schema_onepass_bounds::{candidate_onepass, explicit_scratch_slots};
        let nfa = forward_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap();
        let shape = NfaDimensions::from_nfa(&nfa).unwrap();
        assert_eq!(
            explicit_scratch_slots(shape.slots, shape.patterns),
            Ok(Some(6))
        );
        let error = candidate_onepass(&nfa).unwrap_err();
        assert!(error.to_string().contains("pattern is not one-pass:"));
    }

    #[test]
    fn hostname_primary_hybrid_graphs_complete_without_cache_or_workload_changes() {
        use super::schema_cache_bounds::{forward_nfa, reverse_nfa};
        use super::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
        use super::schema_hybrid_capacity::ReportedCapacityProof;
        use super::schema_hybrid_graph::{QUALIFICATION_BUDGET, certify};
        use std::collections::{BTreeMap, BTreeSet};
        let mut completed = 0_usize;
        let mut capacity_proved = BTreeSet::new();
        let mut capacity_refused = BTreeMap::new();
        for (nfa, direction, label) in [
            (
                forward_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap(),
                HybridDirection::Forward,
                "forward",
            ),
            (
                reverse_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap(),
                HybridDirection::Reverse,
                "reverse",
            ),
        ] {
            let dfa = candidate_hybrid(&nfa, direction).unwrap();
            match ReportedCapacityProof::from_non_quitting_dfa(&dfa) {
                Ok(capacity) => {
                    writeln!(stderr(), "hostname no-capacity-clear {label}: {capacity}").unwrap();
                    assert!(capacity_proved.insert(label));
                }
                Err(reason) => {
                    writeln!(stderr(), "hostname unproved capacity {label}: {reason}").unwrap();
                    assert!(capacity_refused.insert(label, reason).is_none());
                    continue;
                }
            }
            let certificate = certify(&dfa, QUALIFICATION_BUDGET).unwrap();
            assert_eq!(certificate.starts, 771);
            assert!(certificate.ordinary_states > 0);
            writeln!(stderr(), "hostname complete graph {label}: {certificate}").unwrap();
            completed = completed.checked_add(1).unwrap();
        }
        assert_eq!(completed, 2);
        assert!(
            capacity_refused.is_empty(),
            "unproved hostname capacities: {capacity_refused:?}"
        );
        assert_eq!(capacity_proved, BTreeSet::from(["forward", "reverse"]));
    }

    #[test]
    fn hostname_hybrid_candidates_account_each_direction_separately() {
        use super::schema_cache_bounds::{forward_nfa, reverse_nfa};
        use super::schema_hybrid_bounds::{HybridDimensions, HybridDirection, candidate_hybrid};
        use super::schema_hybrid_heap::HybridHeap;
        use super::schema_hybrid_population::{HybridPopulation, recovery_ceiling};
        use std::mem::size_of;
        let forward = forward_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap();
        let reverse = reverse_nfa(super::schema_formats::HOSTNAME_SOURCE).unwrap();
        let mut reported_total = 0_usize;
        for (nfa, direction, expected, expected_recovery) in [
            (
                &forward,
                HybridDirection::Forward,
                HybridDimensions {
                    sparse_entries: 1060,
                    start_entries: 18,
                    sentinel_transitions: 48,
                    epsilon_stack: 125,
                    state_payload_bytes: 1342,
                },
                (9677, 9725),
            ),
            (
                &reverse,
                HybridDirection::Reverse,
                HybridDimensions {
                    sparse_entries: 1028,
                    start_entries: 18,
                    sentinel_transitions: 48,
                    epsilon_stack: 125,
                    state_payload_bytes: 1302,
                },
                (9429, 9477),
            ),
        ] {
            let dfa = candidate_hybrid(nfa, direction).unwrap();
            let projected = HybridDimensions::from_dfa(&dfa).unwrap();
            assert_eq!(projected, expected);
            let reported = projected.fresh_reported_bytes(size_of::<usize>()).unwrap();
            assert_eq!(dfa.create_cache().memory_usage(), reported);
            let population = HybridPopulation::from_dfa(&dfa).unwrap().unwrap();
            let heap = HybridHeap::from_dfa(&dfa).unwrap().unwrap();
            let closed = HybridHeap::from_complete_dfa(&dfa).unwrap().unwrap();
            assert!(closed.live_envelope < heap.live_envelope);
            assert!(closed.growth_overlap <= heap.growth_overlap);
            assert!(closed.peak > 0 && closed.peak < heap.peak);
            assert_eq!(
                recovery_ceiling(
                    super::schema_cache_bounds::NfaDimensions::from_nfa(nfa).unwrap(),
                    &projected,
                    size_of::<usize>()
                )
                .unwrap(),
                if size_of::<usize>() == 4 {
                    expected_recovery.0
                } else {
                    expected_recovery.1
                }
            );
            assert_eq!(
                population.states,
                if size_of::<usize>() == 4 { 809 } else { 736 }
            );
            assert_eq!(
                population.transition_entries,
                if size_of::<usize>() == 4 {
                    12944
                } else {
                    11776
                }
            );
            assert_eq!(
                population.encoded_payload_envelope,
                if direction == HybridDirection::Forward {
                    68220
                } else {
                    68140
                }
            );
            reported_total += reported;
            writeln!(
                stderr(),
                "hostname hybrid candidate: reverse={} {projected} {population} fresh-reported={reported} requested-heap=({heap}) complete-graph-heap=({closed})",
                nfa.is_reverse()
            )
            .unwrap();
        }
        assert_eq!(
            reported_total,
            if size_of::<usize>() == 4 { 9006 } else { 9070 }
        );
        writeln!(
            stderr(),
            "hostname hybrid candidate pair: fresh-reported-total={reported_total}"
        )
        .unwrap();
    }

    #[test]
    fn candidate_is_pinned_without_retrieval_and_is_only_a_native_test_dependency() {
        let root: Value = toml::from_str(include_str!("../../../Cargo.toml")).unwrap();
        let candidate = &root["workspace"]["dependencies"]["jsonschema"];
        assert_eq!(candidate["version"].as_str(), Some("=0.58.6"));
        assert_eq!(candidate["default-features"].as_bool(), Some(false));
        assert_eq!(
            candidate["features"].as_array().unwrap(),
            &[Value::String("macros".to_owned())]
        );
        let bidder: Value = toml::from_str(include_str!("../Cargo.toml")).unwrap();
        assert!(bidder["dependencies"].get("jsonschema").is_none());
        assert_eq!(
            bidder["target"]["cfg(not(target_arch = \"wasm32\"))"]["dev-dependencies"]
                ["jsonschema"]["workspace"]
                .as_bool(),
            Some(true)
        );
    }

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-choice.json",
        draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct SampleChoiceParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-counter.json",
        draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct CounterParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-overlap.json",
        draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct OverlappingParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-uuid.json",
        draft = Draft4,
        validate_formats = true,
        ignore_unknown_formats = false,
        formats = { "uuid" => crate::reference_uuid_format },
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct FormattedParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-uuid.json",
        draft = Draft4,
        validate_formats = false,
        formats = { "uuid" => crate::reference_uuid_format },
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct UnformattedParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-format-property.json",
        draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct NamedFormatParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-hostname.json",
        draft = Draft4,
        validate_formats = true,
        ignore_unknown_formats = false,
        formats = { "hostname" => crate::reference_hostname_format },
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct HostnameParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-conditional.json",
        draft = Draft202012,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct ConditionalParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-conditional.json",
        draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct WrongDraftParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-numeric-pattern.json",
        draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct NumericPatternParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-numeric-length.json",
        draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct NumericLengthParameters;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-integer-keywords.json",
        draft = Draft4
    )]
    struct IntegerWithForeignKeywords;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-union-keywords.json",
        draft = Draft4
    )]
    struct UnionWithForeignKeywords;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/compiler-counter.json",
        draft = Draft4,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct CachedCounterParameters;

    #[test]
    fn bounded_borrowed_parameters_precede_generated_validation() {
        let valid_input = json!({"sample_index":17_i32});
        let invalid_input = json!({"sample_index":null});
        let cases = [
            (&valid_input, 18, 3, 1, Err(SchemaInputError::Bytes)),
            (&valid_input, 19, 2, 1, Err(SchemaInputError::Nodes)),
            (&valid_input, 19, 3, 0, Err(SchemaInputError::Depth)),
            (
                &valid_input,
                19,
                3,
                1,
                Ok((
                    SchemaInputStats {
                        serialized_bytes: 19,
                        nodes: 3,
                        max_depth: 1,
                    },
                    true,
                )),
            ),
            (
                &invalid_input,
                21,
                3,
                1,
                Ok((
                    SchemaInputStats {
                        serialized_bytes: 21,
                        nodes: 3,
                        max_depth: 1,
                    },
                    false,
                )),
            ),
        ];
        for (input, serialized_bytes, nodes, depth, expected) in cases {
            let before = input.clone();
            let mut budget = SchemaWorkBudget::new(SchemaWorkLimits::new(7, 19, 21));
            let calls = Cell::new(0_usize);
            let measured = measure_schema_input(
                input,
                SchemaInputLimits {
                    serialized_bytes,
                    nodes,
                    depth,
                },
            );
            match (measured, expected) {
                (Err(actual), Err(expected_error)) => {
                    assert_eq!(actual, expected_error);
                    assert_eq!(budget.remaining(), 19);
                }
                (Ok(shape), Ok((expected_shape, valid))) => {
                    assert_eq!(shape, expected_shape);
                    // Synthetic projection exercises composition, not an engine bound.
                    let cost = SchemaEvaluationCost {
                        work_units: shape.nodes.checked_mul(2).unwrap().checked_add(1).unwrap(),
                        scratch_bytes: shape.serialized_bytes,
                    };
                    assert_eq!(
                        budget.evaluate(
                            Ok(cost),
                            || false,
                            || {
                                calls.set(calls.get().checked_add(1).unwrap());
                                SampleChoiceParameters::is_valid(input)
                            },
                        ),
                        Ok(valid),
                    );
                    assert_eq!(budget.remaining(), 12);
                }
                (actual, expected_shape) => {
                    panic!("input admission mismatch: {actual:?} / {expected_shape:?}");
                }
            }
            assert_eq!(calls.get(), usize::from(expected.is_ok()));
            assert_eq!(*input, before);
        }
    }

    #[test]
    fn generated_schema_calls_share_work_and_pay_for_invalid_instances() {
        let mut budget = SchemaWorkBudget::new(SchemaWorkLimits {
            single_evaluation_work_units: 7,
            auction_work_units: 19,
            evaluation_scratch_bytes: 11,
        });
        let calls = Cell::new(0_usize);
        let clock_checks = Cell::new(0_usize);
        // Synthetic charges exercise admission, not the schemas' unqualified costs.
        let sample_choice: fn(&Json) -> bool = SampleChoiceParameters::is_valid;
        let conditional: fn(&Json) -> bool = ConditionalParameters::is_valid;
        let cases = [
            (
                sample_choice,
                json!({"sample_index":null}),
                7,
                11,
                false,
                12,
            ),
            (
                conditional,
                json!({"begin":"first","finish":"last"}),
                5,
                4,
                true,
                7,
            ),
            (sample_choice, json!({"sample_index":17_i32}), 7, 9, true, 0),
        ];
        for (validate, input, work_units, scratch_bytes, valid, remaining) in cases {
            assert_eq!(
                budget.evaluate(
                    Ok(SchemaEvaluationCost {
                        work_units,
                        scratch_bytes
                    }),
                    || {
                        clock_checks.set(clock_checks.get().checked_add(1).unwrap());
                        false
                    },
                    || {
                        calls.set(calls.get().checked_add(1).unwrap());
                        validate(&input)
                    },
                ),
                Ok(valid),
            );
            assert_eq!(budget.remaining(), remaining);
        }
        assert_eq!(calls.get(), 3);
        assert_eq!(clock_checks.get(), 3);
        assert_eq!(
            budget.evaluate(
                Ok(SchemaEvaluationCost {
                    work_units: 1,
                    scratch_bytes: 0
                }),
                || {
                    clock_checks.set(clock_checks.get().checked_add(1).unwrap());
                    false
                },
                || {
                    calls.set(calls.get().checked_add(1).unwrap());
                    ConditionalParameters::is_valid(&json!({}))
                },
            ),
            Err(SchemaEvaluationError::AggregateWork),
        );
        assert_eq!(calls.get(), 3);
        assert_eq!(clock_checks.get(), 4);
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn schema_resource_refusals_never_enter_generated_code() {
        for (cost, expired, expected) in [
            (
                Err(SchemaCostError::Overflow),
                false,
                SchemaEvaluationError::CostOverflow,
            ),
            (
                Ok(SchemaEvaluationCost {
                    work_units: 0,
                    scratch_bytes: 1,
                }),
                false,
                SchemaEvaluationError::ZeroWork,
            ),
            (
                Ok(SchemaEvaluationCost {
                    work_units: 8,
                    scratch_bytes: 1,
                }),
                false,
                SchemaEvaluationError::SingleWork,
            ),
            (
                Ok(SchemaEvaluationCost {
                    work_units: 3,
                    scratch_bytes: 12,
                }),
                false,
                SchemaEvaluationError::Scratch,
            ),
            (
                Ok(SchemaEvaluationCost {
                    work_units: 3,
                    scratch_bytes: 1,
                }),
                true,
                SchemaEvaluationError::Deadline,
            ),
        ] {
            let mut budget = SchemaWorkBudget::new(SchemaWorkLimits {
                single_evaluation_work_units: 7,
                auction_work_units: 19,
                evaluation_scratch_bytes: 11,
            });
            let input = json!({"sample_index":17_i32});
            let calls = Cell::new(0_usize);
            assert_eq!(
                budget.evaluate(
                    cost,
                    || expired,
                    || {
                        calls.set(calls.get().checked_add(1).unwrap());
                        SampleChoiceParameters::is_valid(&input)
                    }
                ),
                Err(expected)
            );
            assert_eq!(calls.get(), 0);
            assert_eq!(budget.remaining(), 19);
            assert_eq!(
                budget.evaluate(
                    Ok(SchemaEvaluationCost {
                        work_units: 7,
                        scratch_bytes: 11
                    }),
                    || false,
                    || {
                        calls.set(calls.get().checked_add(1).unwrap());
                        SampleChoiceParameters::is_valid(&input)
                    },
                ),
                Ok(true)
            );
            assert_eq!(calls.get(), 1);
            assert_eq!(budget.remaining(), 12);
        }
    }

    #[test]
    fn inapplicable_keywords_neither_admit_forbidden_types_nor_reject_allowed_types() {
        for (value, integer_valid, union_valid) in [
            (json!(0_i32), true, true),
            (json!(-12_i32), true, true),
            (json!(1.5_f64), false, false),
            (json!(true), false, true),
            (json!(null), false, true),
            (json!("x"), false, false),
            (json!([null]), false, false),
            (json!({"sentinel": null}), false, false),
        ] {
            assert_eq!(
                IntegerWithForeignKeywords::is_valid(&value),
                integer_valid,
                "{value}"
            );
            assert_eq!(
                UnionWithForeignKeywords::is_valid(&value),
                union_valid,
                "{value}"
            );
            assert_eq!(
                IntegerWithForeignKeywords::validate(&value).is_ok(),
                integer_valid,
                "{value}"
            );
            assert_eq!(
                UnionWithForeignKeywords::validate(&value).is_ok(),
                union_valid,
                "{value}"
            );
            assert_eq!(
                IntegerWithForeignKeywords::iter_errors(&value)
                    .next()
                    .is_none(),
                integer_valid,
                "{value}"
            );
            assert_eq!(
                UnionWithForeignKeywords::iter_errors(&value)
                    .next()
                    .is_none(),
                union_valid,
                "{value}"
            );
            if !integer_valid {
                let error = IntegerWithForeignKeywords::validate(&value).unwrap_err();
                assert_eq!(error.schema_path().to_string(), "/type", "{value}");
            }
        }
    }

    #[test]
    fn generated_checks_enforce_declared_numeric_types_with_string_keywords() {
        let numeric_pattern: fn(&Json) -> bool = NumericPatternParameters::is_valid;
        let numeric_length: fn(&Json) -> bool = NumericLengthParameters::is_valid;
        for (name, schema_source, valid, invalid, generated) in [
            (
                "synthetic numeric pattern",
                include_str!("support/fake_bidder/schemas/compiler-numeric-pattern.json"),
                json!({"count": 1_i32, "ratio": 1_i32}),
                json!({"count": "x", "ratio": "x"}),
                numeric_pattern,
            ),
            (
                "synthetic numeric length",
                include_str!("support/fake_bidder/schemas/compiler-numeric-length.json"),
                json!({"sample": {"tick": 1_i32}}),
                json!({"sample": {"tick": "123"}}),
                numeric_length,
            ),
        ] {
            let schema: serde_json::Value = serde_json::from_str(schema_source).unwrap();
            let dynamic = jsonschema::options()
                .with_draft(jsonschema::Draft::Draft4)
                .build(&schema)
                .unwrap();
            assert!(dynamic.is_valid(&valid), "positive control: {name}");
            assert!(generated(&valid), "positive generated control: {name}");
            // Literal expected rejection follows the schemas' declared numeric
            // types; the independent runtime code path confirms the contrast.
            assert!(!dynamic.is_valid(&invalid), "numeric type: {name}");
            assert!(!generated(&invalid), "generated numeric type: {name}");
        }
    }

    #[test]
    fn generated_checks_preserve_required_fields_and_integer_string_alternatives() {
        for (label, value, expected) in [
            ("missing", json!({}), false),
            ("integer", json!({"sample_index": 17_i32}), true),
            ("string", json!({"sample_index": "17"}), true),
            ("null", json!({"sample_index": null}), false),
            ("boolean", json!({"sample_index": true}), false),
            ("fraction", json!({"sample_index": 17.5_f64}), false),
        ] {
            assert_eq!(
                SampleChoiceParameters::is_valid(&value),
                expected,
                "{label}"
            );
        }
    }

    #[test]
    fn generated_combinators_preserve_exclusive_and_alternative_branches() {
        for (label, value, expected) in [
            ("one selector", json!({"sample_index": 17_i32}), true),
            (
                "two sample selectors",
                json!({"sample_index": 17_i32, "sample_name": "first"}),
                false,
            ),
            (
                "window pair",
                json!({"window": {"start": 2_i32, "end": 5_i32}}),
                true,
            ),
            (
                "incomplete pair",
                json!({"window": {"start": 2_i32}}),
                false,
            ),
            (
                "two outer branches",
                json!({"sample_index": 17_i32, "window": {"start": 2_i32, "end": 5_i32}}),
                false,
            ),
            (
                "array branch",
                json!({"sample_index": 17_i32, "annotations": [{"label": "first"}]}),
                true,
            ),
            (
                "array required field",
                json!({"sample_index": 17_i32, "annotations": [{}]}),
                false,
            ),
            (
                "no string branch",
                json!({"sample_index": 17_i32, "annotations": ""}),
                false,
            ),
        ] {
            assert_eq!(
                SampleChoiceParameters::is_valid(&value),
                expected,
                "{label}"
            );
        }
        assert!(OverlappingParameters::is_valid(&json!({"serial": 5_i32})));
        assert!(OverlappingParameters::is_valid(
            &json!({"range": {"lower": 2_i32, "upper": 5_i32}})
        ));
        assert!(OverlappingParameters::is_valid(
            &json!({"serial": 5_i32, "range": {"lower": 2_i32, "upper": 5_i32}})
        ));
        assert!(!OverlappingParameters::is_valid(
            &json!({"range": {"lower": 2_i32}})
        ));
    }

    #[test]
    fn generated_regex_checks_retain_ascii_digits_and_unanchored_search() {
        for (value, expected) in [
            (json!({"counter": "123"}), true),
            (json!({"counter": 123_i32}), true),
            (json!({"counter": ""}), false),
            (json!({"counter": "a123"}), false),
            (json!({"counter": "\u{0661}"}), false),
            (json!({"counter": "\u{ff11}"}), false),
        ] {
            assert_eq!(CounterParameters::is_valid(&value), expected, "{value}");
        }
        assert!(OverlappingParameters::is_valid(
            &json!({"serial": 5_i32, "phase": "raw"})
        ));
        assert!(OverlappingParameters::is_valid(
            &json!({"serial": 5_i32, "phase": "prefixpackedsuffix"})
        ));
        assert!(!OverlappingParameters::is_valid(
            &json!({"serial": 5_i32, "phase": "other"})
        ));
    }

    #[test]
    fn synthetic_pattern_caches_preserve_verdicts_across_interleaved_owners() {
        let mut first = CachedCounterParameters::new_search_cache().unwrap();
        let mut second = CachedCounterParameters::new_search_cache().unwrap();
        for (input, expected) in [
            (json!({"counter": "0"}), true),
            (json!({"counter": "letters"}), false),
            (json!({"counter": null}), false),
            (json!({"counter": 14_i32}), true),
            (json!({}), false),
            (json!({"counter": "901"}), true),
            (json!({"counter": "prefix1"}), false),
            (json!({"counter": "42"}), true),
        ] {
            let before = input.clone();
            assert_eq!(
                CachedCounterParameters::is_valid(&input, &mut first),
                expected,
                "first: {input}"
            );
            assert!(!CachedCounterParameters::is_valid(
                &json!({"counter": "invalid"}),
                &mut second
            ));
            assert_eq!(
                CachedCounterParameters::is_valid(&input, &mut second),
                expected,
                "second: {input}"
            );
            assert_eq!(CounterParameters::is_valid(&input), expected, "{input}");
            assert_eq!(input, before);
        }
    }

    #[test]
    fn explicit_format_validation_is_observable_and_not_a_property_name() {
        let invalid_uuid = json!({"trace": "not-a-uuid"});
        assert!(!FormattedParameters::is_valid(&invalid_uuid));
        assert!(UnformattedParameters::is_valid(&invalid_uuid));
        assert!(FormattedParameters::is_valid(&json!({
            "trace": "123e4567-e89b-12d3-a456-426614174000"
        })));
        for invalid in [
            "123E4567-e89b-12d3-a456-426614174000",
            "123e4567e89b12d3a456426614174000",
            "{123e4567-e89b-12d3-a456-426614174000}",
            "urn:uuid:123e4567-e89b-12d3-a456-426614174000",
            "123e4567-e89b-12d3-a456-42661417400g",
            "123e4567_e89b_12d3_a456_426614174000",
        ] {
            assert!(
                !FormattedParameters::is_valid(&json!({"trace": invalid})),
                "{invalid}"
            );
        }
        assert!(FormattedParameters::is_valid(&json!({
            "trace": "00000000-0000-0000-0000-000000000000"
        })));
        assert!(NamedFormatParameters::is_valid(&json!({
            "note": "sample", "format": "not-a-format-name"
        })));
        assert!(!NamedFormatParameters::is_valid(&json!({
            "note": "sample"
        })));
    }

    #[test]
    fn hostname_format_preserves_reference_length_and_label_boundaries() {
        assert!(super::schema_formats::initialize_reference_formats());
        assert!(super::schema_formats::initialize_reference_formats());
        for (host, expected) in [
            ("example.com", true),
            ("1.example", true),
            ("example.com.", false),
            ("a..example", false),
            ("a_b.example", false),
            ("-a.example", false),
            ("a-.example", false),
            ("\u{e9}.example", false),
        ] {
            assert_eq!(
                HostnameParameters::is_valid(&json!({"hostname": host})),
                expected,
                "{host}"
            );
        }
        for (last_label_bytes, expected) in [(61_usize, true), (62, true), (63, true), (64, false)]
        {
            let host = [
                "a".repeat(63),
                "b".repeat(63),
                "c".repeat(63),
                "d".repeat(last_label_bytes),
            ]
            .join(".");
            assert_eq!(host.len(), 192_usize.checked_add(last_label_bytes).unwrap());
            assert_eq!(
                HostnameParameters::is_valid(&json!({"hostname": host})),
                expected,
                "last label {last_label_bytes}"
            );
        }
        assert!(!HostnameParameters::is_valid(
            &json!({"hostname": format!("{}.example", "a".repeat(64))})
        ));
        let overlong = [63_usize, 63, 63, 62, 1]
            .map(|length| "a".repeat(length))
            .join(".");
        assert_eq!(overlong.len(), 256);
        assert!(!HostnameParameters::is_valid(
            &json!({"hostname": overlong})
        ));
    }

    #[test]
    fn declared_newer_draft_preserves_conditional_requirements() {
        for (value, expected) in [
            (json!({}), true),
            (json!({"begin": "first", "finish": "last"}), true),
            (json!({"begin": "first"}), false),
            (json!({"finish": "last"}), false),
            (json!({"begin": "", "finish": "last"}), false),
        ] {
            assert_eq!(ConditionalParameters::is_valid(&value), expected, "{value}");
        }
        assert!(WrongDraftParameters::is_valid(&json!({"begin": "first"})));
        assert!(WrongDraftParameters::is_valid(&json!({"finish": "last"})));
    }
}

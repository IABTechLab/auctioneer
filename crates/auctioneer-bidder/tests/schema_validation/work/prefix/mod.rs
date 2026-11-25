//! Tests-first all-attempt preflight multiplicities, not serializer work.
//!
//! Native limits come only from the source-matched `AttemptContext`. Literal
//! prefix witnesses below are independent source oracles, not a second walk.
//! `input_raw_utf8_with_quotes_upper` sums raw UTF-8 length plus two quotes
//! only for strings/keys passing the precheck and reaching serialization,
//! including the final serialization whose byte charge fails. Operands of
//! rejected prechecks are excluded, not bounded by that sum.
//! Scalar output lengths are virtual `Counter` bytes, not copied/scanned work.
//! `Counter`'s request protocol does not prove serde's number of requests. Map
//! height/navigation and ordinary error/unwind cleanup remain named missing.

mod scalar;

pub(in crate::schema_validation) use scalar::Refusal as ScalarRefusal;
pub(super) use scalar::ScalarData;
#[cfg(test)]
pub(super) use scalar::fixture as scalar_fixture;

use super::{AttemptContext, PrefixCounts, Refusal};

// The parent supplies limits only after matching the native/input recipes.
// Each value and key first charges the same monotone node counter. At most J
// charges succeed, and the first failed charge ends that attempt immediately.
// Containers, scalar serializations and string prechecks require a paid node.
// Each Some reaches a child/key charge except a possible final comma failure;
// accounting for the already paid root still gives at most J Some results.
// Each entered loop contributes at most one final None. Container/scalar byte
// charges together, commas, and key colons each have ceiling J, giving 3J.
// Visit nesting is D+1; only D successful containers can retain iterators.
// These are source multiplicities, not transitive helper costs or frame bytes.
pub(super) fn attach_bounds(
    context: &AttemptContext<'_>,
    _scalar: &ScalarData,
    counts: &mut PrefixCounts,
) -> Result<(), Refusal> {
    // Tests-first bridge: the required DATA operand is not checked yet. The
    // four scalar slots stay unknown until the new guards reach hosted RED.
    let limits = context.native_input_limits;
    let preflight_entries = counts.preflight_entries;
    let node_attempts = if preflight_entries == 0 {
        0
    } else {
        limits
            .nodes
            .checked_add(1)
            .and_then(|per_attempt| preflight_entries.checked_mul(per_attempt))
            .ok_or(Refusal::Overflow)?
    };
    let charged_nodes = preflight_entries
        .checked_mul(limits.nodes)
        .ok_or(Refusal::Overflow)?;
    let byte_charges = charged_nodes.checked_mul(3).ok_or(Refusal::Overflow)?;
    let (visit_nesting, iterator_nesting) = if preflight_entries == 0 {
        (0, 0)
    } else {
        let containers = u64::from(limits.depth);
        (
            containers.checked_add(1).ok_or(Refusal::Overflow)?,
            containers,
        )
    };

    // Commit only after every supported arithmetic check succeeds.
    counts.input_node_attempts_upper = Some(node_attempts);
    counts.input_charged_nodes_upper = Some(charged_nodes);
    counts.input_container_entries_upper = Some(charged_nodes);
    counts.input_scalar_serializations_upper = Some(charged_nodes);
    counts.input_string_prechecks_upper = Some(charged_nodes);
    counts.input_byte_charge_calls_upper = Some(byte_charges);
    counts.input_iterator_some_upper = Some(charged_nodes);
    counts.input_iterator_none_upper = Some(charged_nodes);
    counts.input_visit_nesting_upper = Some(visit_nesting);
    counts.input_iterator_nesting_upper = Some(iterator_nesting);

    // These require the separately bound concrete serde/Write source lemmas.
    // A precheck alone does not prove the summed raw-string budget, and a
    // generic writer trait does not supply Counter's request protocol.
    counts.input_raw_utf8_with_quotes_upper = None;
    counts.input_counter_writes_per_nonempty_request_upper = None;
    counts.input_counter_writes_per_empty_request_upper = None;
    counts.input_counter_interrupted_retries_upper = None;
    counts.input_tree_navigation_upper = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{
        AttemptContext, BUDGET, CallerFacts, ContextBinding, ContextSources, DISPATCH_GENERATOR,
        EXCHANGE_LIMITS, EXCHANGE_RUN, EXCHANGE_SCHEMA, EXCHANGE_SPLIT, FORMATS, INPUT, JSON_SIZE,
        NATIVE, NativePath, Population, PrefixCounts, Refusal, SchemaInputError, SchemaInputStats,
        SourceRole, bind_attempt_context, measure_schema_input, prefix_counts,
    };
    use super::scalar::{
        Dispatch as ScalarDispatch, FeatureClause as ScalarFeatureClause,
        SourceUnit as ScalarSourceUnit,
    };
    use super::{ScalarData, ScalarRefusal, attach_bounds, scalar_fixture};
    use auctioneer_runtime::serialized_json_bytes;
    use quote::ToTokens as _;
    use serde_json::{Map, Value, json};
    use std::str::from_utf8;
    use syn::visit_mut::{self, VisitMut};

    fn with_context(run: impl FnOnce(&AttemptContext<'_>, &ScalarData)) {
        let descriptor = json!({"identity":"fake_prefix_capture"});
        let roots = json!({"fixture":"fake_caller_roots"});
        let census = json!({"fixture":"fake_compiled_source_census"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            capture_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = ContextSources {
            native: NATIVE,
            budget: BUDGET,
            input: INPUT,
            json_size: JSON_SIZE,
            exchange_schema: EXCHANGE_SCHEMA,
            exchange_run: EXCHANGE_RUN,
            exchange_split: EXCHANGE_SPLIT,
            exchange_limits: EXCHANGE_LIMITS,
            dispatch_generator: DISPATCH_GENERATOR,
            formats: FORMATS,
        };
        let checked = bind_attempt_context(&binding, &sources);
        assert_eq!(checked.as_ref().map(|_| ()), Ok(()));
        let context = checked.unwrap();
        assert_eq!(context.sources, &sources);
        assert_eq!(context.native_input_limits.serialized_bytes, 65_536);
        assert_eq!(context.native_input_limits.nodes, 4_096);
        assert_eq!(context.native_input_limits.depth, 32);
        let (scalar, scalar_limits) = scalar_fixture::fixture();
        assert_eq!(scalar_limits, context.native_input_limits);
        run(&context, &scalar);
    }

    // Conditional caller data, not an authentic ingress/caller-cap witness.
    fn prefix_result(
        context: &AttemptContext<'_>,
        scalar: &ScalarData,
        attempts: u64,
        path: NativePath,
    ) -> Result<PrefixCounts, Refusal> {
        prefix_counts(
            context,
            scalar,
            CallerFacts {
                original_impressions: Some(attempts),
                configured_bidder_cap: Some(1),
                panic_unwinds: Some(true),
            },
            Population {
                scopes_entered: u64::from(attempts != 0),
                retain_visits: attempts,
                missing_parameters: 0,
                native_attempts: attempts,
                native_path: (attempts != 0).then_some(path),
            },
        )
    }

    fn rejected(
        context: &AttemptContext<'_>,
        scalar: &ScalarData,
        attempts: u64,
        error: SchemaInputError,
    ) -> PrefixCounts {
        let checked = prefix_result(context, scalar, attempts, NativePath::InputRejected(error));
        assert_eq!(checked.as_ref().map(|_| ()), Ok(()));
        let counts = checked.unwrap();
        assert_eq!(counts.preflight_entries, attempts);
        assert_eq!(counts.native_clock_calls, attempts);
        assert_eq!(counts.budget_entries, 0);
        assert_eq!(counts.debit_events, 0);
        assert_eq!(counts.fresh_cache_calls, 0);
        counts
    }

    fn multiplicities(counts: &PrefixCounts) -> [Option<u64>; 11] {
        [
            counts.input_node_attempts_upper,
            counts.input_charged_nodes_upper,
            counts.input_container_entries_upper,
            counts.input_scalar_serializations_upper,
            counts.input_string_prechecks_upper,
            counts.input_byte_charge_calls_upper,
            counts.input_iterator_some_upper,
            counts.input_iterator_none_upper,
            counts.input_raw_utf8_with_quotes_upper,
            counts.input_visit_nesting_upper,
            counts.input_iterator_nesting_upper,
        ]
    }

    fn missing_costs(counts: &PrefixCounts) {
        assert_eq!(
            counts.input_unresolved_calls,
            [
                "serde_json::to_writer",
                "IteratorProtocol",
                "caller-owned BTreeMap height/navigation",
                "ordinary preflight error/unwind cleanup",
            ]
        );
        assert_eq!(counts.input_tree_navigation_upper, None);
    }

    #[test]
    fn rejected_preflight_prefixes_remain_charged() {
        with_context(|context, scalar| {
            let counts = rejected(context, scalar, 3, SchemaInputError::Nodes);
            // First normative assertion must fail on the missing bound, not setup.
            assert_eq!(counts.input_node_attempts_upper, Some(12_291));
            assert_eq!(
                multiplicities(&counts),
                [
                    Some(12_291),
                    Some(12_288),
                    Some(12_288),
                    Some(12_288),
                    Some(12_288),
                    Some(36_864),
                    Some(12_288),
                    Some(12_288),
                    Some(196_608),
                    Some(33),
                    Some(32),
                ]
            );
            let once = rejected(context, scalar, 1, SchemaInputError::Nodes);
            assert_eq!(
                multiplicities(&once),
                [
                    Some(4_097),
                    Some(4_096),
                    Some(4_096),
                    Some(4_096),
                    Some(4_096),
                    Some(12_288),
                    Some(4_096),
                    Some(4_096),
                    Some(65_536),
                    Some(33),
                    Some(32),
                ]
            );
            let empty = rejected(context, scalar, 0, SchemaInputError::Nodes);
            assert_eq!(multiplicities(&empty), [Some(0); 11]);
            missing_costs(&counts);
            assert_node_prefixes(context, &once);
            assert_node_charge_mutation(context);
        });
    }

    fn assert_node_prefixes(context: &AttemptContext<'_>, bounds: &PrefixCounts) {
        let child_count = usize::try_from(context.native_input_limits.nodes).unwrap();
        let array = json!(vec![false; child_count]);
        assert_eq!(array.as_array().unwrap().len(), 4_096);
        assert_eq!(serialized_json_bytes(&array).unwrap(), 24_577);
        assert_eq!(
            measure_schema_input(&array, context.native_input_limits),
            Err(SchemaInputError::Nodes)
        );
        // Root + 4095 successful children + the failing child. The final
        // Some occurs before charge_node fails, and no final None is reached.
        let (values, keys, serializations, some, none) =
            (4_097_u64, 0_u64, 4_095_u64, 4_096_u64, 0_u64);
        assert_eq!(values.checked_add(keys), bounds.input_node_attempts_upper);
        assert!(Some(serializations) <= bounds.input_scalar_serializations_upper);
        assert_eq!(Some(some), bounds.input_iterator_some_upper);
        assert!(Some(none) <= bounds.input_iterator_none_upper);
    }

    #[test]
    fn keys_and_string_prechecks_bound_partial_prefixes() {
        with_context(|context, scalar| {
            let counts = rejected(context, scalar, 1, SchemaInputError::Bytes);
            assert_eq!(counts.input_raw_utf8_with_quotes_upper, Some(65_536));
            assert_eq!(counts.input_string_prechecks_upper, Some(4_096));
            assert_eq!(counts.input_scalar_serializations_upper, Some(4_096));
            missing_costs(&counts);
            assert_key_prefix(context, &counts);
            assert_string_prefixes(context);
            assert_key_charge_mutation(context);
            assert_string_precheck_mutations(context);
        });
    }

    fn assert_key_prefix(context: &AttemptContext<'_>, bounds: &PrefixCounts) {
        let mut properties = Map::new();
        let keys = context.native_input_limits.nodes.checked_div(2).unwrap();
        for index in 0..keys {
            assert!(
                properties
                    .insert(format!("{index:04}"), json!(false))
                    .is_none()
            );
        }
        let object = Value::Object(properties);
        assert_eq!(object.as_object().unwrap().len(), 2_048);
        assert_eq!(serialized_json_bytes(&object).unwrap(), 26_625);
        assert_eq!(
            measure_schema_input(&object, context.native_input_limits),
            Err(SchemaInputError::Nodes)
        );
        // All 2048 keys are charged; the last value's node attempt fails.
        let (value_visits, key_visits, scalar_calls) = (2_049_u64, 2_048_u64, 4_095_u64);
        assert_eq!(value_visits.checked_add(key_visits), Some(4_097));
        assert_eq!(
            value_visits.checked_add(key_visits),
            bounds.input_node_attempts_upper
        );
        assert!(Some(value_visits) < bounds.input_node_attempts_upper);
        assert!(Some(scalar_calls) <= bounds.input_scalar_serializations_upper);
    }

    fn assert_string_prefixes(context: &AttemptContext<'_>) {
        let cap = usize::try_from(context.native_input_limits.serialized_bytes).unwrap();
        // Raw 65535 + two quotes = 65537: this rejected precheck operand
        // never reaches serialization and is excluded from the raw sum.
        let too_long = "a".repeat(cap.checked_sub(1).unwrap());
        assert_eq!(too_long.len(), 65_535);
        assert_eq!(
            measure_schema_input(&Value::String(too_long), context.native_input_limits),
            Err(SchemaInputError::Bytes)
        );
        let quoted_len = cap.checked_sub(2).unwrap();
        let admitted = "a".repeat(quoted_len);
        assert_eq!(admitted.len(), 65_534);
        assert_eq!(
            measure_schema_input(&Value::String(admitted), context.native_input_limits),
            Ok(SchemaInputStats {
                serialized_bytes: 65_536,
                nodes: 1,
                max_depth: 0
            })
        );
        let escaped = "\0".repeat(quoted_len);
        assert_eq!(escaped.len(), 65_534);
        // This precheck passes: raw 65534 + two quotes = 65536 belongs to
        // the sum even though charging its expanded serialization fails.
        // This is virtual emitted length only: no scanned/copied-byte lemma.
        assert_eq!(serialized_json_bytes(escaped.as_str()).unwrap(), 393_206);
        assert_eq!(
            measure_schema_input(&Value::String(escaped), context.native_input_limits),
            Err(SchemaInputError::Bytes)
        );
        let unicode = json!({"\u{00e9}\"\n":"\u{1f642}\\\t\u{0000}"});
        assert_eq!(
            measure_schema_input(&unicode, context.native_input_limits),
            Ok(SchemaInputStats {
                serialized_bytes: 27,
                nodes: 3,
                max_depth: 1
            })
        );
    }

    #[test]
    fn iterator_population_and_recursion_axes_remain_distinct() {
        with_context(|context, scalar| {
            let counts = rejected(context, scalar, 1, SchemaInputError::Depth);
            assert_eq!(counts.input_visit_nesting_upper, Some(33));
            assert_eq!(counts.input_iterator_nesting_upper, Some(32));
            assert_eq!(counts.input_iterator_some_upper, Some(4_096));
            assert_eq!(counts.input_iterator_none_upper, Some(4_096));
            let depth = context.native_input_limits.depth.checked_add(1).unwrap();
            let nested = (0..depth).fold(Value::Null, |inner, _| Value::Array(vec![inner]));
            assert_eq!(depth, 33);
            assert_eq!(
                measure_schema_input(&nested, context.native_input_limits),
                Err(SchemaInputError::Depth)
            );
            for empty in [json!([]), json!({})] {
                assert_eq!(
                    measure_schema_input(&empty, context.native_input_limits),
                    Ok(SchemaInputStats {
                        serialized_bytes: 2,
                        nodes: 1,
                        max_depth: 1
                    })
                );
            }
            let serial = rejected(context, scalar, 3, SchemaInputError::Depth);
            assert_eq!(serial.input_visit_nesting_upper, Some(33));
            assert_eq!(serial.input_iterator_nesting_upper, Some(32));
            assert_eq!(serial.input_iterator_some_upper, Some(12_288));
            assert_eq!(serial.input_iterator_none_upper, Some(12_288));
            missing_costs(&counts);
            missing_costs(&serial);
            assert_hypothetical_height_is_not_a_proof(&counts);
            assert_depth_mutation(context);
        });
    }

    fn assert_hypothetical_height_is_not_a_proof(counts: &PrefixCounts) {
        // Conditional arithmetic DATA only: actual map invariants/navigation
        // are not matched here. No supplied height grants source authority.
        let (objects, steps) = (2_u64, 5_u64);
        for (hypothetical_height, literal) in [(3_u64, 36_u64), (5_u64, 60_u64)] {
            let conditional = steps
                .checked_mul(2)
                .and_then(|some| objects.checked_add(some))
                .and_then(|coefficient| hypothetical_height.checked_mul(coefficient));
            assert_eq!(conditional, Some(literal));
            assert_ne!(counts.input_tree_navigation_upper, conditional);
        }
        assert_eq!(counts.input_tree_navigation_upper, None);
    }

    #[test]
    fn counter_protocol_source_drift_and_overflow_refuse() {
        with_context(|context, scalar| {
            let counts = rejected(context, scalar, 1, SchemaInputError::Bytes);
            assert_eq!(
                counts.input_counter_writes_per_nonempty_request_upper,
                Some(1)
            );
            assert_eq!(counts.input_counter_writes_per_empty_request_upper, Some(0));
            assert_eq!(counts.input_counter_interrupted_retries_upper, Some(0));
            missing_costs(&counts);
            assert_counter_mutations(context);
            assert_eq!(
                prefix_result(
                    context,
                    scalar,
                    u64::MAX,
                    NativePath::InputRejected(SchemaInputError::Nodes)
                ),
                Err(Refusal::Overflow)
            );
        });
    }

    fn assert_bridge_setup(context: &AttemptContext<'_>, scalar: &ScalarData) {
        scalar_fixture::assert_pristine(scalar, context.native_input_limits);
        for attempts in [0_u64, 1_u64] {
            let result = prefix_result(
                context,
                scalar,
                attempts,
                NativePath::InputRejected(SchemaInputError::Nodes),
            );
            assert_eq!(result.as_ref().map(|_| ()), Ok(()));
            let counts = result.unwrap();
            assert_eq!(counts.preflight_entries, attempts);
            assert_eq!(counts.native_entries, attempts);
            assert_eq!(
                counts.input_node_attempts_upper,
                Some(if attempts == 0 { 0 } else { 4_097 })
            );
            missing_costs(&counts);
        }
    }

    fn assert_bridge_refusal(
        context: &AttemptContext<'_>,
        scalar: &ScalarData,
        attempts: u64,
        expected: ScalarRefusal,
    ) {
        assert_eq!(
            prefix_result(
                context,
                scalar,
                attempts,
                NativePath::InputRejected(SchemaInputError::Nodes),
            ),
            Err(Refusal::ScalarSource(expected))
        );
    }

    fn assert_input_links(context: &AttemptContext<'_>, scalar: &ScalarData, attempts: u64) {
        for unit in [
            ScalarSourceUnit::Native,
            ScalarSourceUnit::Input,
            ScalarSourceUnit::JsonSize,
        ] {
            // Same parsed source semantics/path, different bytes in both
            // synthetic snapshots with a new digest: byte links must still fail.
            let changed = scalar_fixture::different_input_bytes(scalar, unit);
            assert_bridge_refusal(
                context,
                &changed,
                attempts,
                ScalarRefusal::ContextSourceMismatch(unit),
            );
        }
    }

    fn assert_limit_links(context: &AttemptContext<'_>, scalar: &ScalarData, attempts: u64) {
        let original = context.native_input_limits;
        let mut bytes = original;
        bytes.serialized_bytes = bytes.serialized_bytes.checked_sub(1).unwrap();
        let mut nodes = original;
        nodes.nodes = nodes.nodes.checked_sub(1).unwrap();
        let mut depth = original;
        depth.depth = depth.depth.checked_sub(1).unwrap();
        for changed in [bytes, nodes, depth] {
            assert_ne!(changed, original);
            // Explicit conditional context DATA mutation, not a successfully
            // rebound context or an actual descriptor/source-window witness.
            let wrong_limits = AttemptContext {
                binding: context.binding,
                sources: context.sources,
                native_input_limits: changed,
            };
            assert_bridge_refusal(
                &wrong_limits,
                scalar,
                attempts,
                ScalarRefusal::NativeLimitsMismatch,
            );
        }
    }

    #[test]
    fn scalar_input_links_are_required_at_zero_and_one_preflights() {
        with_context(|context, scalar| {
            assert_bridge_setup(context, scalar);
            for attempts in [0_u64, 1_u64] {
                assert_input_links(context, scalar, attempts);
                assert_limit_links(context, scalar, attempts);
            }
        });
    }

    fn assert_scalar_duties(context: &AttemptContext<'_>, scalar: &ScalarData) {
        let units = scalar_fixture::units();
        assert_eq!(units.len(), 20);
        for unit in units {
            let missing = scalar_fixture::without_source(scalar, unit);
            assert_bridge_refusal(context, &missing, 0, ScalarRefusal::MissingSource(unit));
            let duplicate = scalar_fixture::duplicate_source(scalar, unit);
            assert_bridge_refusal(context, &duplicate, 0, ScalarRefusal::DuplicateSource(unit));
        }
        let drift = scalar_fixture::changed_record(scalar, ScalarSourceUnit::IoWrite);
        assert_bridge_refusal(
            context,
            &drift,
            0,
            ScalarRefusal::SourceChanged(ScalarSourceUnit::IoWrite),
        );
        let retargeted = scalar_fixture::wrong_path(scalar, ScalarSourceUnit::SerdeStr);
        assert_bridge_refusal(
            context,
            &retargeted,
            0,
            ScalarRefusal::SourcePath(ScalarSourceUnit::SerdeStr),
        );
        let framing = scalar_fixture::missing_string_framing(scalar);
        assert_bridge_refusal(
            context,
            &framing,
            0,
            ScalarRefusal::SourceShape(ScalarSourceUnit::JsonSerializer),
        );
        let loop_skipped = scalar_fixture::skipped_write_loop(scalar);
        assert_bridge_refusal(
            context,
            &loop_skipped,
            0,
            ScalarRefusal::SourceShape(ScalarSourceUnit::IoWrite),
        );
    }

    fn assert_scalar_premises(context: &AttemptContext<'_>, scalar: &ScalarData) {
        for (clause, supported) in [
            (ScalarFeatureClause::SerdeStd, true),
            (ScalarFeatureClause::SerdeCoreStd, true),
            (ScalarFeatureClause::JsonStd, true),
            (ScalarFeatureClause::ArbitraryPrecision, false),
            (ScalarFeatureClause::Docsrs, false),
        ] {
            let absent = scalar_fixture::feature_value(scalar, clause, None);
            assert_bridge_refusal(context, &absent, 0, ScalarRefusal::MissingFeature(clause));
            let opposite = scalar_fixture::feature_value(scalar, clause, Some(!supported));
            assert_bridge_refusal(
                context,
                &opposite,
                0,
                ScalarRefusal::FeatureMismatch(clause),
            );
        }
        let absent_width = scalar_fixture::width_value(scalar, None);
        assert_bridge_refusal(context, &absent_width, 0, ScalarRefusal::MissingWidth);
        let wrong_width = scalar_fixture::width_value(scalar, Some(128));
        assert_bridge_refusal(
            context,
            &wrong_width,
            0,
            ScalarRefusal::UnsupportedWidth(128),
        );
        for dispatch in [
            ScalarDispatch::GenericSerialize,
            ScalarDispatch::OtherWriter,
            ScalarDispatch::PrettyFormatter,
        ] {
            let wrong_dispatch = scalar_fixture::dispatch_value(scalar, dispatch);
            assert_bridge_refusal(
                context,
                &wrong_dispatch,
                0,
                ScalarRefusal::UnsupportedDispatch,
            );
        }
    }

    #[test]
    fn scalar_source_refusals_survive_zero_preflights() {
        with_context(|context, scalar| {
            assert_bridge_setup(context, scalar);
            assert_scalar_duties(context, scalar);
            assert_scalar_premises(context, scalar);
            let counts = prefix_result(
                context,
                scalar,
                0,
                NativePath::InputRejected(SchemaInputError::Nodes),
            )
            .unwrap();
            assert_eq!(counts.input_raw_utf8_with_quotes_upper, Some(0));
            // These are conditional per-request ceilings, not zero invocations.
            assert_eq!(
                counts.input_counter_writes_per_nonempty_request_upper,
                Some(1)
            );
            assert_eq!(counts.input_counter_writes_per_empty_request_upper, Some(0));
            assert_eq!(counts.input_counter_interrupted_retries_upper, Some(0));
            missing_costs(&counts);
            let narrower = scalar_fixture::width_value(scalar, Some(32));
            let result = prefix_result(
                context,
                &narrower,
                0,
                NativePath::InputRejected(SchemaInputError::Nodes),
            );
            assert_eq!(result.as_ref().map(|_| ()), Ok(()));
        });
    }

    fn destination(
        context: &AttemptContext<'_>,
        scalar: &ScalarData,
        attempts: u64,
    ) -> PrefixCounts {
        let mut counts = prefix_result(
            context,
            scalar,
            1,
            NativePath::InputRejected(SchemaInputError::Nodes),
        )
        .unwrap();
        assert_eq!(counts.preflight_entries, 1);
        assert_eq!(counts.native_entries, 1);
        counts.preflight_entries = attempts;
        // Snapshot sentinels are DATA, never supplied bounds or certificates.
        // A intentionally differs from native_entries to guard its exact field.
        counts.input_node_attempts_upper = Some(17);
        counts.input_raw_utf8_with_quotes_upper = Some(19);
        counts.input_counter_writes_per_nonempty_request_upper = Some(23);
        counts.input_counter_writes_per_empty_request_upper = Some(29);
        counts.input_counter_interrupted_retries_upper = Some(31);
        counts
    }

    fn assert_source_failure_is_atomic(
        context: &AttemptContext<'_>,
        scalar: &ScalarData,
        attempts: u64,
    ) {
        let mut counts = destination(context, scalar, attempts);
        let before = destination(context, scalar, attempts);
        assert_eq!(counts, before);
        let missing = scalar_fixture::without_source(scalar, ScalarSourceUnit::IoWrite);
        assert_eq!(
            attach_bounds(context, &missing, &mut counts),
            Err(Refusal::ScalarSource(ScalarRefusal::MissingSource(
                ScalarSourceUnit::IoWrite
            )))
        );
        assert_eq!(counts, before);
    }

    #[test]
    fn scalar_prefix_failures_preserve_counts_transactionally() {
        with_context(|context, scalar| {
            assert_bridge_setup(context, scalar);
            let attempts = 281_474_976_710_656_u64;
            assert_eq!(attempts.checked_mul(4_096), Some(1_152_921_504_606_846_976));
            assert_eq!(
                attempts.checked_mul(12_288),
                Some(3_458_764_513_820_540_928)
            );
            assert_eq!(attempts.checked_mul(65_536), None);
            let mut counts = destination(context, scalar, attempts);
            let before = destination(context, scalar, attempts);
            assert_eq!(counts, before);
            // Old J/3J bounds fit: only the newly required A*B overflows.
            assert_eq!(
                attach_bounds(context, scalar, &mut counts),
                Err(Refusal::Overflow)
            );
            assert_eq!(counts, before);
            for source_attempts in [0_u64, 1_u64] {
                assert_source_failure_is_atomic(context, scalar, source_attempts);
            }
        });
    }

    // Mutations are AST data for the existing context owner, never executed
    // alternate implementations. Every edit asserts its exact before-state.
    fn edit_block(source: &[u8], name: &str, change: &mut dyn FnMut(&mut syn::Block)) -> Vec<u8> {
        struct Edit<'a> {
            name: &'a str,
            change: &'a mut dyn FnMut(&mut syn::Block),
            hits: usize,
        }
        impl VisitMut for Edit<'_> {
            fn visit_impl_item_fn_mut(&mut self, i: &mut syn::ImplItemFn) {
                if i.sig.ident == self.name {
                    (self.change)(&mut i.block);
                    self.hits = self.hits.checked_add(1).unwrap();
                } else {
                    visit_mut::visit_impl_item_fn_mut(self, i);
                }
            }
        }
        let mut file = syn::parse_file(from_utf8(source).unwrap()).unwrap();
        let before = file.to_token_stream().to_string();
        let mut edit = Edit {
            name,
            change,
            hits: 0,
        };
        edit.visit_file_mut(&mut file);
        assert_eq!(edit.hits, 1, "one actual source helper: {name}");
        let after = file.to_token_stream().to_string();
        assert_ne!(after, before);
        after.into_bytes()
    }

    fn refused(context: &AttemptContext<'_>, role: SourceRole, changed: &[u8]) {
        let mut sources = *context.sources;
        match role {
            SourceRole::Input => sources.input = changed,
            SourceRole::JsonSize => sources.json_size = changed,
            SourceRole::Native
            | SourceRole::Budget
            | SourceRole::ExchangeSchema
            | SourceRole::ExchangeRun
            | SourceRole::ExchangeSplit
            | SourceRole::ExchangeLimits
            | SourceRole::DispatchGenerator
            | SourceRole::Formats => panic!("unexpected mutation role"),
        }
        assert_eq!(
            bind_attempt_context(context.binding, &sources).map(|_| ()),
            Err(Refusal::SourceShape(role))
        );
    }

    fn assert_node_charge_mutation(context: &AttemptContext<'_>) {
        let changed = edit_block(INPUT, "visit", &mut |body| {
            let first = body.stmts.first().unwrap();
            let expected: syn::Stmt = syn::parse_quote!(self.charge_node()?;);
            assert_eq!(
                first.to_token_stream().to_string(),
                expected.to_token_stream().to_string()
            );
            body.stmts.remove(0);
        });
        refused(context, SourceRole::Input, &changed);
    }

    fn assert_key_charge_mutation(context: &AttemptContext<'_>) {
        let changed = edit_block(INPUT, "visit", &mut |body| {
            struct RemoveKeyCharge {
                hits: usize,
            }
            impl VisitMut for RemoveKeyCharge {
                fn visit_expr_for_loop_mut(&mut self, i: &mut syn::ExprForLoop) {
                    let expected: syn::Expr = syn::parse_quote!(properties);
                    if i.expr.to_token_stream().to_string()
                        == expected.to_token_stream().to_string()
                    {
                        let charge: syn::Stmt = syn::parse_quote!(self.charge_node()?;);
                        let matching: Vec<usize> = i
                            .body
                            .stmts
                            .iter()
                            .enumerate()
                            .filter_map(|(index, statement)| {
                                (statement.to_token_stream().to_string()
                                    == charge.to_token_stream().to_string())
                                .then_some(index)
                            })
                            .collect();
                        assert_eq!(matching.len(), 1);
                        i.body.stmts.remove(*matching.first().unwrap());
                        self.hits = self.hits.checked_add(1).unwrap();
                    }
                    visit_mut::visit_expr_for_loop_mut(self, i);
                }
            }
            let mut mutation = RemoveKeyCharge { hits: 0 };
            mutation.visit_block_mut(body);
            assert_eq!(mutation.hits, 1);
        });
        refused(context, SourceRole::Input, &changed);
    }

    fn assert_string_precheck_mutations(context: &AttemptContext<'_>) {
        let reordered = edit_block(INPUT, "charge_string", &mut |body| {
            assert_eq!(body.stmts.len(), 3);
            let check: syn::Stmt = syn::parse_quote!(self.check_string_len(text.len())?;);
            assert_eq!(
                body.stmts.first().unwrap().to_token_stream().to_string(),
                check.to_token_stream().to_string()
            );
            body.stmts.swap(0, 1);
        });
        refused(context, SourceRole::Input, &reordered);
        let unquoted = edit_block(INPUT, "check_string_len", &mut |body| {
            struct Quotes {
                hits: usize,
            }
            impl VisitMut for Quotes {
                fn visit_expr_method_call_mut(&mut self, i: &mut syn::ExprMethodCall) {
                    if i.method == "checked_add" {
                        assert_eq!(i.args.len(), 1);
                        let expected: syn::Expr = syn::parse_quote!(2);
                        assert_eq!(
                            i.args.first().unwrap().to_token_stream().to_string(),
                            expected.to_token_stream().to_string()
                        );
                        *i.args.first_mut().unwrap() = syn::parse_quote!(0);
                        self.hits = self.hits.checked_add(1).unwrap();
                    }
                    visit_mut::visit_expr_method_call_mut(self, i);
                }
            }
            let mut mutation = Quotes { hits: 0 };
            mutation.visit_block_mut(body);
            assert_eq!(mutation.hits, 1);
        });
        refused(context, SourceRole::Input, &unquoted);
    }

    fn assert_depth_mutation(context: &AttemptContext<'_>) {
        let changed = edit_block(INPUT, "enter_container", &mut |body| {
            assert_eq!(body.stmts.len(), 5);
            let expected: syn::Stmt = syn::parse_quote! {
                if depth > self.limits.depth { return Err(SchemaInputError::Depth); }
            };
            assert_eq!(
                body.stmts.get(1).unwrap().to_token_stream().to_string(),
                expected.to_token_stream().to_string()
            );
            body.stmts.remove(1);
        });
        refused(context, SourceRole::Input, &changed);
    }

    fn assert_counter_mutations(context: &AttemptContext<'_>) {
        let short_write = edit_block(JSON_SIZE, "write", &mut |body| {
            let expected = syn::Stmt::Expr(syn::parse_quote!(Ok(buf.len())), None);
            assert_eq!(
                body.stmts.last().unwrap().to_token_stream().to_string(),
                expected.to_token_stream().to_string()
            );
            *body.stmts.last_mut().unwrap() =
                syn::Stmt::Expr(syn::parse_quote!(Ok(buf.len().saturating_sub(1))), None);
        });
        refused(context, SourceRole::JsonSize, &short_write);
        let unchecked = edit_block(JSON_SIZE, "write", &mut |body| {
            struct Addition {
                hits: usize,
            }
            impl VisitMut for Addition {
                fn visit_expr_method_call_mut(&mut self, i: &mut syn::ExprMethodCall) {
                    if i.method == "checked_add" {
                        let expected: syn::Expr = syn::parse_quote!(self.0.checked_add(len));
                        assert_eq!(
                            i.to_token_stream().to_string(),
                            expected.to_token_stream().to_string()
                        );
                        i.method = syn::parse_quote!(wrapping_add);
                        self.hits = self.hits.checked_add(1).unwrap();
                    }
                    visit_mut::visit_expr_method_call_mut(self, i);
                }
            }
            let mut mutation = Addition { hits: 0 };
            mutation.visit_block_mut(body);
            assert_eq!(mutation.hits, 1);
        });
        refused(context, SourceRole::JsonSize, &unchecked);
    }
}

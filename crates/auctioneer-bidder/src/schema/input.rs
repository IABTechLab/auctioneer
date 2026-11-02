//! Borrowed JSON input bounds, not engine cost, funding, or provenance proof.
//!
//! The walk retains only scalar counters and depth-bounded borrowed iterators.
//! It neither clones/reparses the input nor retains serialized output. Input
//! shape does not qualify schema evaluation work or authorize engine integration.

use auctioneer_runtime::serialized_json_bytes;
use serde_json::Value;

/// Maximum supported container nesting, independent of the caller's limits.
pub const MAX_SCHEMA_INPUT_DEPTH: u32 = 32;

/// Exact caller-supplied bounds, without defaults or clamping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaInputLimits {
    /// Maximum compact JSON bytes.
    pub serialized_bytes: u64,
    /// Maximum values plus object keys.
    pub nodes: u64,
    /// Maximum nested containers; a root scalar has depth zero.
    pub depth: u32,
}

/// Measured compact JSON size and shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaInputStats {
    /// Exact compact JSON bytes.
    pub serialized_bytes: u64,
    /// Every value, including containers, plus every object key.
    pub nodes: u64,
    /// Maximum nested containers; a root scalar has depth zero.
    pub max_depth: u32,
}

/// Fixed refusals without instance content or serializer diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaInputError {
    /// The serialized byte limit or byte counter was exceeded.
    Bytes,
    /// The node limit or node counter was exceeded.
    Nodes,
    /// The observed container nesting exceeded the supplied depth.
    Depth,
    /// A scalar could not be represented by the JSON serializer.
    Representation,
    /// The declared depth exceeds [`MAX_SCHEMA_INPUT_DEPTH`].
    InvalidDepthLimit,
}

/// Measure a borrowed final parameter subtree without cloning or reparsing it.
///
/// Counts compact JSON bytes, every value and object key, and container nesting.
/// A scalar root has depth zero; an empty container has depth one. Zero bounds
/// are literal: depth zero admits funded scalars, but no containers.
///
/// Before formatting any string/key, its raw UTF-8 length plus two quotes must
/// fit the remaining byte allowance. Escaping may finish scanning that bounded
/// scalar before refusing its expanded size: scanning is bounded by the supplied
/// cap, not stopped at the exact first excess output byte. Scalar encodings use
/// [`auctioneer_runtime::serialized_json_bytes`]; containers never reach that helper.
///
/// # Errors
/// Declared depth above [`MAX_SCHEMA_INPUT_DEPTH`] returns
/// [`SchemaInputError::InvalidDepthLimit`] before traversal. Other refusals occur
/// deterministically during traversal, without a global error-priority promise.
/// Every counter uses checked arithmetic; overflow refuses with its fixed class.
pub fn measure_schema_input(
    value: &Value,
    limits: SchemaInputLimits,
) -> Result<SchemaInputStats, SchemaInputError> {
    if limits.depth > MAX_SCHEMA_INPUT_DEPTH {
        return Err(SchemaInputError::InvalidDepthLimit);
    }
    let mut measurement = InputMeasurement {
        limits,
        stats: SchemaInputStats {
            serialized_bytes: 0,
            nodes: 0,
            max_depth: 0,
        },
    };
    measurement.visit(value, 0)?;
    Ok(measurement.stats)
}

struct InputMeasurement {
    limits: SchemaInputLimits,
    stats: SchemaInputStats,
}

impl InputMeasurement {
    fn charge_bytes(&mut self, bytes: u64) -> Result<(), SchemaInputError> {
        let next = self
            .stats
            .serialized_bytes
            .checked_add(bytes)
            .ok_or(SchemaInputError::Bytes)?;
        if next > self.limits.serialized_bytes {
            return Err(SchemaInputError::Bytes);
        }
        self.stats.serialized_bytes = next;
        Ok(())
    }

    fn charge_node(&mut self) -> Result<(), SchemaInputError> {
        let next = self
            .stats
            .nodes
            .checked_add(1)
            .ok_or(SchemaInputError::Nodes)?;
        if next > self.limits.nodes {
            return Err(SchemaInputError::Nodes);
        }
        self.stats.nodes = next;
        Ok(())
    }

    fn check_string_len(&self, raw_len: usize) -> Result<(), SchemaInputError> {
        let minimum = u64::try_from(raw_len)
            .ok()
            .and_then(|length| length.checked_add(2))
            .ok_or(SchemaInputError::Bytes)?;
        let remaining = self
            .limits
            .serialized_bytes
            .checked_sub(self.stats.serialized_bytes)
            .ok_or(SchemaInputError::Bytes)?;
        if minimum > remaining {
            return Err(SchemaInputError::Bytes);
        }
        Ok(())
    }

    fn charge_string(&mut self, text: &str) -> Result<(), SchemaInputError> {
        self.check_string_len(text.len())?;
        let bytes = serialized_json_bytes(text)
            .map_err(|_serialization| SchemaInputError::Representation)?;
        self.charge_bytes(bytes)
    }

    fn enter_container(&mut self, parent_depth: u32) -> Result<u32, SchemaInputError> {
        let depth = parent_depth.checked_add(1).ok_or(SchemaInputError::Depth)?;
        if depth > self.limits.depth {
            return Err(SchemaInputError::Depth);
        }
        self.stats.max_depth = self.stats.max_depth.max(depth);
        self.charge_bytes(2)?;
        Ok(depth)
    }

    fn visit(&mut self, value: &Value, parent_depth: u32) -> Result<(), SchemaInputError> {
        self.charge_node()?;
        match value {
            Value::Array(items) => {
                let depth = self.enter_container(parent_depth)?;
                let mut first = true;
                for item in items {
                    if !first {
                        self.charge_bytes(1)?;
                    }
                    first = false;
                    self.visit(item, depth)?;
                }
                Ok(())
            }
            Value::Object(properties) => {
                let depth = self.enter_container(parent_depth)?;
                let mut first = true;
                for (key, item) in properties {
                    if !first {
                        self.charge_bytes(1)?;
                    }
                    first = false;
                    self.charge_node()?;
                    self.charge_string(key)?;
                    self.charge_bytes(1)?;
                    self.visit(item, depth)?;
                }
                Ok(())
            }
            Value::String(text) => self.charge_string(text),
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                let bytes = serialized_json_bytes(value)
                    .map_err(|_serialization| SchemaInputError::Representation)?;
                self.charge_bytes(bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        InputMeasurement, MAX_SCHEMA_INPUT_DEPTH, SchemaInputError, SchemaInputLimits,
        SchemaInputStats, measure_schema_input,
    };
    use serde_json::{Value, json};

    fn limits(serialized_bytes: u64, nodes: u64, depth: u32) -> SchemaInputLimits {
        SchemaInputLimits {
            serialized_bytes,
            nodes,
            depth,
        }
    }

    fn stats(serialized_bytes: u64, nodes: u64, max_depth: u32) -> SchemaInputStats {
        SchemaInputStats {
            serialized_bytes,
            nodes,
            max_depth,
        }
    }

    fn assert_exact(value: &Value, expected: SchemaInputStats) {
        let measured = measure_schema_input(
            value,
            limits(
                expected.serialized_bytes,
                expected.nodes,
                expected.max_depth,
            ),
        );
        assert_eq!(measured, Ok(expected));
        assert_eq!(
            u64::try_from(serde_json::to_vec(value).unwrap().len()).unwrap(),
            expected.serialized_bytes,
            "the independent literal size must also match compact serialization",
        );
    }

    fn nested_arrays(depth: u32) -> Value {
        (0..depth).fold(Value::Null, |inner, _| Value::Array(vec![inner]))
    }

    fn counter(serialized_bytes: u64, nodes: u64) -> InputMeasurement {
        InputMeasurement {
            limits: limits(u64::MAX, u64::MAX, 32),
            stats: stats(serialized_bytes, nodes, 0),
        }
    }

    #[test]
    fn measures_literal_compact_json() {
        assert_eq!(
            measure_schema_input(
                &json!({"a": [null, true, -7_i64, "x"], "b": {}}),
                SchemaInputLimits {
                    serialized_bytes: 31,
                    nodes: 9,
                    depth: 2,
                },
            ),
            Ok(SchemaInputStats {
                serialized_bytes: 31,
                nodes: 9,
                max_depth: 2,
            }),
        );
    }

    #[test]
    fn refuses_an_unfunded_node() {
        assert_eq!(
            measure_schema_input(
                &json!(false),
                SchemaInputLimits {
                    serialized_bytes: 5,
                    nodes: 0,
                    depth: 0,
                },
            ),
            Err(SchemaInputError::Nodes),
        );
    }

    #[test]
    fn root_scalars_have_one_node_and_zero_depth() {
        for (value, bytes) in [
            (json!(null), 4_u64),
            (json!(true), 4),
            (json!(false), 5),
            (json!(-7_i64), 2),
            (json!("x"), 3),
            (json!(""), 2),
        ] {
            assert_exact(&value, stats(bytes, 1, 0));
        }
    }

    #[test]
    fn empty_containers_still_cost_a_node_and_one_depth() {
        for value in [json!([]), json!({})] {
            assert_exact(&value, stats(2, 1, 1));
            assert_eq!(
                measure_schema_input(&value, limits(2, 1, 0)),
                Err(SchemaInputError::Depth),
            );
        }
    }

    #[test]
    fn escaped_keys_and_values_count_utf8_and_json_escaping() {
        assert_exact(
            &json!({"\u{00e9}\"\n": "\u{1f642}\\\t\u{0000}"}),
            stats(27, 3, 1),
        );
        assert_exact(&json!("\u{00e9}"), stats(4, 1, 0));
        assert_exact(&json!("\u{1f642}"), stats(6, 1, 0));
        assert_exact(&json!("\u{2028}\u{2029}"), stats(8, 1, 0));
        assert_exact(&json!("/\r\u{0008}\u{000c}"), stats(9, 1, 0));
    }

    #[test]
    fn number_encodings_include_signed_zero_and_fractional_values() {
        for (value, bytes) in [
            (json!(i64::MIN), 20_u64),
            (json!(u64::MAX), 20),
            (json!(0_u64), 1),
            (json!(0.0_f64), 3),
            (json!(-0.0_f64), 4),
            (json!(1.25_f64), 4),
            (json!(-0.125_f64), 6),
        ] {
            assert_exact(&value, stats(bytes, 1, 0));
        }
    }

    #[test]
    fn byte_limits_are_independent_at_under_and_over_exact_sizes() {
        for (value, expected) in [
            (json!(true), stats(4, 1, 0)),
            (json!([12_u64, "x"]), stats(8, 3, 1)),
        ] {
            assert_eq!(
                measure_schema_input(
                    &value,
                    limits(expected.serialized_bytes.checked_sub(1).unwrap(), 103, 9),
                ),
                Err(SchemaInputError::Bytes),
            );
            for cap in [
                expected.serialized_bytes,
                expected.serialized_bytes.checked_add(1).unwrap(),
            ] {
                assert_eq!(
                    measure_schema_input(&value, limits(cap, 103, 9)),
                    Ok(expected)
                );
            }
        }
    }

    #[test]
    fn node_limits_are_independent_at_under_and_over_exact_counts() {
        for (value, expected) in [
            (json!({"a": 0_u64}), stats(7, 3, 1)),
            (json!([null, {}, false, ""]), stats(18, 5, 2)),
        ] {
            assert_eq!(
                measure_schema_input(
                    &value,
                    limits(107, expected.nodes.checked_sub(1).unwrap(), 9)
                ),
                Err(SchemaInputError::Nodes),
            );
            for cap in [expected.nodes, expected.nodes.checked_add(1).unwrap()] {
                assert_eq!(
                    measure_schema_input(&value, limits(107, cap, 9)),
                    Ok(expected)
                );
            }
        }
    }

    #[test]
    fn depth_limits_are_independent_at_under_and_over_exact_nesting() {
        for (value, expected) in [
            (json!([0_u64]), stats(3, 2, 1)),
            (json!({"a": [{}]}), stats(10, 4, 3)),
        ] {
            assert_eq!(
                measure_schema_input(
                    &value,
                    limits(109, 23, expected.max_depth.checked_sub(1).unwrap())
                ),
                Err(SchemaInputError::Depth),
            );
            for cap in [
                expected.max_depth,
                expected.max_depth.checked_add(1).unwrap(),
            ] {
                assert_eq!(
                    measure_schema_input(&value, limits(109, 23, cap)),
                    Ok(expected)
                );
            }
        }
    }

    #[test]
    fn object_keys_are_nodes_but_not_extra_depth() {
        let value = json!({"a": {"b": []}, "c": true});
        assert_exact(&value, stats(23, 7, 3));
        assert_eq!(
            measure_schema_input(&value, limits(101, 6, 8)),
            Err(SchemaInputError::Nodes),
        );
    }

    #[test]
    fn sibling_containers_do_not_accumulate_depth() {
        assert_exact(&json!([[], {}, [], {}]), stats(13, 5, 2));
        assert_exact(&json!({"a": [[]], "b": {"c": []}}), stats(23, 8, 3));
        assert_exact(&json!({"a": {"c": []}, "b": [[]]}), stats(23, 8, 3));
    }

    #[test]
    fn nesting_31_and_32_pass_but_33_is_refused() {
        assert_eq!(MAX_SCHEMA_INPUT_DEPTH, 32);
        assert_exact(&nested_arrays(31), stats(66, 32, 31));
        assert_exact(&nested_arrays(32), stats(68, 33, 32));
        assert_eq!(
            measure_schema_input(&nested_arrays(33), limits(1001, 103, 32)),
            Err(SchemaInputError::Depth),
        );
    }

    #[test]
    fn unsupported_declared_depth_is_refused_before_input_checks() {
        for depth in [33_u32, u32::MAX] {
            for value in [json!(null), json!([])] {
                assert_eq!(
                    measure_schema_input(&value, limits(0, 0, depth)),
                    Err(SchemaInputError::InvalidDepthLimit),
                );
            }
        }
    }

    #[test]
    fn giant_strings_refuse_tiny_byte_caps() {
        for text in ["x".repeat(1_000_000), "\u{0000}".repeat(1_000_000)] {
            assert_eq!(
                measure_schema_input(&Value::String(text), limits(7, 19, 3)),
                Err(SchemaInputError::Bytes),
            );
        }
    }

    #[test]
    fn giant_keys_refuse_tiny_remaining_byte_caps() {
        for key in ["x".repeat(1_000_000), "\n".repeat(1_000_000)] {
            let mut object = serde_json::Map::new();
            object.insert(key, Value::Null);
            assert_eq!(
                measure_schema_input(&Value::Object(object), limits(11, 19, 3)),
                Err(SchemaInputError::Bytes),
            );
        }
    }

    #[test]
    fn zero_limits_never_mean_unlimited() {
        assert_eq!(
            measure_schema_input(&json!(null), limits(0, 1, 0)),
            Err(SchemaInputError::Bytes)
        );
        assert_eq!(
            measure_schema_input(&json!(null), limits(4, 0, 0)),
            Err(SchemaInputError::Nodes)
        );
        assert_eq!(
            measure_schema_input(&json!(null), limits(4, 1, 0)),
            Ok(stats(4, 1, 0))
        );
        assert_eq!(
            measure_schema_input(&json!({}), limits(2, 1, 0)),
            Err(SchemaInputError::Depth)
        );
    }

    #[test]
    fn byte_counter_checks_near_u64_max_without_wrapping() {
        let mut measurement = counter(u64::MAX.checked_sub(1).unwrap(), 0);
        assert_eq!(measurement.charge_bytes(1), Ok(()));
        assert_eq!(measurement.stats.serialized_bytes, u64::MAX);
        assert_eq!(measurement.charge_bytes(1), Err(SchemaInputError::Bytes));
        assert_eq!(measurement.stats.serialized_bytes, u64::MAX);
    }

    #[test]
    fn node_counter_checks_near_u64_max_without_wrapping() {
        let mut measurement = counter(0, u64::MAX.checked_sub(1).unwrap());
        assert_eq!(measurement.charge_node(), Ok(()));
        assert_eq!(measurement.stats.nodes, u64::MAX);
        assert_eq!(measurement.charge_node(), Err(SchemaInputError::Nodes));
        assert_eq!(measurement.stats.nodes, u64::MAX);
    }

    #[test]
    fn string_precheck_includes_quotes_and_checked_remaining_bytes() {
        let measurement = counter(u64::MAX.checked_sub(3).unwrap(), 0);
        assert_eq!(measurement.check_string_len(1), Ok(()));
        assert_eq!(
            measurement.check_string_len(2),
            Err(SchemaInputError::Bytes)
        );
        assert_eq!(
            measurement.check_string_len(usize::MAX),
            Err(SchemaInputError::Bytes)
        );
        let mut invalid_counter = counter(8, 0);
        invalid_counter.limits.serialized_bytes = 7;
        assert_eq!(
            invalid_counter.check_string_len(0),
            Err(SchemaInputError::Bytes)
        );
    }

    #[test]
    fn escaped_scalar_may_exceed_bytes_after_passing_raw_precheck() {
        assert_eq!(
            measure_schema_input(&json!("\u{0000}"), limits(3, 1, 0)),
            Err(SchemaInputError::Bytes)
        );
        assert_exact(&json!("\u{0000}"), stats(8, 1, 0));
        assert_eq!(
            measure_schema_input(&json!({"\u{0000}": 0_u64}), limits(8, 3, 1)),
            Err(SchemaInputError::Bytes)
        );
    }

    #[test]
    fn node_refusal_precedes_scalar_or_key_formatting() {
        assert_eq!(
            measure_schema_input(&json!(""), limits(0, 0, 0)),
            Err(SchemaInputError::Nodes)
        );
        assert_eq!(
            measure_schema_input(&json!({"": 0_u64}), limits(2, 1, 1)),
            Err(SchemaInputError::Nodes)
        );
    }

    #[test]
    fn hostile_depth_stops_with_borrowed_walk_and_iterative_teardown() {
        let mut value = nested_arrays(10_000);
        let result = measure_schema_input(&value, limits(u64::MAX, u64::MAX, 32));
        // Do not let Value's recursive destructor become the tested stack limit.
        while let Value::Array(mut children) = value {
            value = children.pop().unwrap();
        }
        assert_eq!(result, Err(SchemaInputError::Depth));
    }

    #[test]
    fn success_and_refusal_leave_the_callers_value_unchanged() {
        let value = json!({"a": [null, true, -7_i64, "x"], "b": {}});
        let before = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            measure_schema_input(&value, limits(31, 9, 2)),
            Ok(stats(31, 9, 2))
        );
        assert_eq!(
            measure_schema_input(&value, limits(30, 9, 2)),
            Err(SchemaInputError::Bytes)
        );
        assert_eq!(
            measure_schema_input(&value, limits(31, 8, 2)),
            Err(SchemaInputError::Nodes)
        );
        assert_eq!(
            measure_schema_input(&value, limits(31, 9, 1)),
            Err(SchemaInputError::Depth)
        );
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    }
}

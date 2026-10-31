//! Borrowed integer checks for source DTO fields retained by the shared JSON map.

use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IntegerFieldError {
    Invalid,
    Ambiguous,
}

/// Source `mtype` and `dur` use ASCII-equivalent spellings; both names
/// have no additional non-ASCII matches under the source lowercase decoder.
pub(super) fn signed_integer(
    fields: &Map<String, Value>,
    name: &str,
) -> Result<i64, IntegerFieldError> {
    let mut matching = fields
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let selected = matching.next().map(|(_, value)| value);
    if matching.next().is_some() {
        return Err(IntegerFieldError::Ambiguous);
    }
    match selected {
        None | Some(Value::Null) => Ok(0),
        Some(value) => value.as_i64().ok_or(IntegerFieldError::Invalid),
    }
}

pub(super) fn signed_byte(
    fields: &Map<String, Value>,
    name: &str,
) -> Result<i8, IntegerFieldError> {
    i8::try_from(signed_integer(fields, name)?).map_err(|_error| IntegerFieldError::Invalid)
}

pub(super) fn canonical_integer(
    fields: &mut Map<String, Value>,
    name: &'static str,
) -> Result<i64, IntegerFieldError> {
    let value = signed_integer(fields, name)?;
    if value == 0 {
        fields.retain(|key, _| !key.eq_ignore_ascii_case(name));
        return Ok(0);
    }
    if !fields.contains_key(name) {
        fields.retain(|key, _| !key.eq_ignore_ascii_case(name));
        fields.insert(name.into(), Value::from(value));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{IntegerFieldError, canonical_integer, signed_byte, signed_integer};
    use serde_json::{Value, json};

    fn fields(value: Value) -> serde_json::Map<String, Value> {
        let Value::Object(fields) = value else {
            panic!("object fixture")
        };
        fields
    }

    #[test]
    fn absent_null_and_single_ascii_spellings_decode_without_changing_the_map() {
        for key in ["dur", "DUR", "Dur"] {
            for number in [i64::MIN, -1, 0, 30, i64::MAX] {
                let input = fields(json!({key:number,"keep":{"nested":[true,null]}}));
                let before = input.clone();
                assert_eq!(signed_integer(&input, "dur"), Ok(number));
                assert_eq!(input, before);
            }
            assert_eq!(signed_integer(&fields(json!({key:null})), "dur"), Ok(0));
        }
        assert_eq!(
            signed_integer(&fields(json!({"unrelated":1_i32})), "dur"),
            Ok(0)
        );
    }

    #[test]
    fn distinct_spellings_are_ambiguous_even_with_equal_or_null_values() {
        for (first, second) in [
            (json!(1_i32), json!(1_i32)),
            (json!(1_i32), Value::Null),
            (Value::Null, Value::Null),
        ] {
            for (left, right) in [("dur", "DUR"), ("Dur", "dUr")] {
                assert_eq!(
                    signed_integer(&fields(json!({left:first,right:second})), "dur"),
                    Err(IntegerFieldError::Ambiguous),
                );
            }
        }
    }

    #[test]
    fn integer_types_and_both_signed_width_boundaries_are_independent() {
        for bad in [
            json!("30"),
            json!(30.0_f64),
            json!(1.5_f64),
            json!(true),
            json!([]),
            json!({}),
            json!(u64::MAX),
        ] {
            assert_eq!(
                signed_integer(&fields(json!({"dur":bad})), "dur"),
                Err(IntegerFieldError::Invalid)
            );
        }
        for number in [-129_i64, -128, -1, 0, 127, 128] {
            let input = fields(json!({"Mtype":number}));
            assert_eq!(signed_integer(&input, "mtype"), Ok(number));
            assert_eq!(
                signed_byte(&input, "mtype"),
                i8::try_from(number).map_err(|_error| IntegerFieldError::Invalid)
            );
        }
    }

    #[test]
    fn canonical_integer_moves_only_its_validated_field_and_omits_source_zero() {
        for key in ["dur", "DUR", "Dur", "dUr"] {
            for number in [-1_i64, 0, 30, i64::MAX] {
                let mut input = fields(json!({key:number,"opaque":{"keep":[true,null]}}));
                assert_eq!(canonical_integer(&mut input, "dur"), Ok(number));
                let mut expected = fields(json!({"opaque":{"keep":[true,null]}}));
                if number != 0 {
                    expected.insert("dur".into(), json!(number));
                }
                assert_eq!(input, expected);
            }
        }
        for mut input in [fields(json!({"DUR":null})), fields(json!({}))] {
            assert_eq!(canonical_integer(&mut input, "dur"), Ok(0));
            assert!(input.is_empty());
        }
    }

    #[test]
    fn canonical_integer_refusal_leaves_even_competing_null_values_untouched() {
        for (document, error) in [
            (json!({"dur":"30"}), IntegerFieldError::Invalid),
            (
                json!({"dur":30_i64,"DUR":null}),
                IntegerFieldError::Ambiguous,
            ),
            (json!({"Dur":null,"DUR":null}), IntegerFieldError::Ambiguous),
        ] {
            let mut input = fields(document);
            let before = input.clone();
            assert_eq!(canonical_integer(&mut input, "dur"), Err(error));
            assert_eq!(input, before);
        }
    }
}

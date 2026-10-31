//! Bounded in-place unwrapping of an already-owned native creative.

use crate::{BidderError, CappedWriter};
use auctioneer_openrtb::types::field::JsonField;
use auctioneer_runtime::bounded_json::{BoundedJsonError, from_slice_within};
use auctioneer_runtime::{
    json_preflight::{MAX_CONSTRUCTION_DEPTH, preflight},
    serialized_json_bytes,
};
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;
use serde_json::value::RawValue;

pub(crate) fn project_value(
    value: &Value,
    body_bytes: usize,
    max_nodes: u32,
) -> Result<String, BidderError> {
    preflight(value, u64::from(max_nodes), MAX_CONSTRUCTION_DEPTH)
        .map_err(|_error| limit_error())?;
    let bytes = serialized_json_bytes(value).map_err(|_error| limit_error())?;
    let size = usize::try_from(bytes).map_err(|_overflow| limit_error())?;
    if size > body_bytes {
        return Err(limit_error());
    }
    let mut output = Vec::with_capacity(size);
    CappedWriter::new(&mut output, size)
        .and_then(|mut writer| writer.write_json(value))
        .map_err(|_error| limit_error())?;
    String::from_utf8(output).map_err(|_utf8| invalid_error())
}

#[derive(Deserialize)]
struct NativeEnvelope<'a> {
    #[serde(default, borrow)]
    native: JsonField<&'a RawValue>,
}

pub(crate) fn unwrap_in_place(
    adm: &mut String,
    body_bytes: usize,
    max_nodes: u32,
) -> Result<(), BidderError> {
    if adm.len() > body_bytes {
        return Err(limit_error());
    }
    from_slice_within::<IgnoredAny>(adm.as_bytes(), max_nodes).map_err(|error| match error {
        BoundedJsonError::NodeBudget(_) => limit_error(),
        BoundedJsonError::Syntax(_) | BoundedJsonError::RawValue(_) => invalid_error(),
    })?;
    if adm.trim_start().as_bytes().first() != Some(&b'{') {
        return Err(invalid_error());
    }

    // Borrow only the payload span after complete syntax/node admission. Unknown
    // siblings are counted above, but neither they nor the payload become a tree.
    let envelope: NativeEnvelope<'_> =
        serde_json::from_str(adm).map_err(|_error| invalid_error())?;
    let payload = match envelope.native {
        JsonField::Missing => return Ok(()),
        JsonField::Null => return Err(invalid_error()),
        JsonField::Value(value) => value.get(),
    };
    if !payload.starts_with('{') {
        return Err(invalid_error());
    }
    let start = payload
        .as_ptr()
        .addr()
        .checked_sub(adm.as_ptr().addr())
        .ok_or_else(invalid_error)?;
    let end = start.checked_add(payload.len()).ok_or_else(invalid_error)?;
    if end > adm.len() || !adm.is_char_boundary(start) || !adm.is_char_boundary(end) {
        return Err(invalid_error());
    }
    adm.truncate(end);
    drop(adm.drain(..start));
    Ok(())
}

fn invalid_error() -> BidderError {
    BidderError::bad_server_response("invalid native creative")
}

fn limit_error() -> BidderError {
    BidderError::bad_server_response("native creative exceeded parsing limits")
}

#[cfg(test)]
mod tests {
    use crate::BidderHttpResponse;
    use serde_json::json;

    fn response(adm: &str, nodes: u32) -> BidderHttpResponse {
        let body = serde_json::to_vec(&json!({"id":"r", "seatbid":[{"bid":[{
            "id":"b", "impid":"i", "price":1_i32, "mtype":4_i32, "adm":adm,
        }]}]}))
        .unwrap();
        BidderHttpResponse::new(200, Vec::new(), body).within_node_budget(nodes)
    }

    #[test]
    fn project_adm_value_serializes_values_without_changing_their_owner() {
        for value in [
            json!(null),
            json!({"assets": [], "text": "\u{00e9}\n\""}),
            json!([1_i32, true]),
        ] {
            let before = value.clone();
            let encoded = value.to_string();
            let response = BidderHttpResponse::new(200, Vec::new(), vec![b' '; encoded.len()])
                .within_node_budget(100);
            assert_eq!(response.project_adm_value(&value).unwrap(), encoded);
            assert_eq!(value, before);
        }
    }

    #[test]
    fn project_adm_value_respects_node_and_byte_limits_independently() {
        let value = json!({"text": "\n"});
        let encoded = value.to_string();
        for (bytes, nodes, accepted) in [
            (encoded.len() - 1, 100, false),
            (encoded.len(), 2, false),
            (encoded.len(), 3, true),
            (encoded.len() + 1, 3, true),
        ] {
            let response = BidderHttpResponse::new(200, Vec::new(), vec![b' '; bytes])
                .within_node_budget(nodes);
            let result = response.project_adm_value(&value);
            assert_eq!(result.is_ok(), accepted, "bytes={bytes}, nodes={nodes}");
            if accepted {
                assert_eq!(result.unwrap(), encoded);
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.code, 4);
                assert_eq!(error.message, "native creative exceeded parsing limits");
            }
        }
    }

    #[test]
    fn wrapper_removal_preserves_the_exact_payload_and_string_allocation() {
        let payload = json!({
            "assets": [],
            "link": {"url": "https://fake_bidder.test/a/b"},
        });
        let expected = payload.to_string();
        let mut adm = json!({"before": true, "native": payload, "after": []}).to_string();
        let pointer = adm.as_ptr();
        let capacity = adm.capacity();
        response(&adm, 100).unwrap_native_adm(&mut adm).unwrap();
        assert_eq!(adm, expected);
        assert_eq!(adm.as_ptr(), pointer);
        assert_eq!(adm.capacity(), capacity);
    }

    #[test]
    fn already_unwrapped_objects_remain_byte_identical() {
        for value in [
            json!({}),
            json!({"assets": []}),
            json!({"Native": {}, "assets": []}),
        ] {
            let source = serde_json::to_string_pretty(&value).unwrap();
            let mut adm = source.clone();
            let pointer = adm.as_ptr();
            response(&source, 100).unwrap_native_adm(&mut adm).unwrap();
            assert_eq!(adm, source);
            assert_eq!(adm.as_ptr(), pointer);
        }
    }

    #[test]
    fn escaped_wrapper_keys_follow_json_field_semantics() {
        // Lexical fixture: json! emits the decoded key, not its escaped spelling.
        let mut adm = "{\"na\\u0074ive\":{\"assets\":[]}}".to_owned();
        response(&adm, 100).unwrap_native_adm(&mut adm).unwrap();
        assert_eq!(adm, json!({"assets": []}).to_string());
    }

    #[test]
    fn unicode_payload_spans_use_byte_offsets_without_reencoding() {
        let payload = json!({"text": "\u{1f680}"});
        let expected = payload.to_string();
        let mut adm = json!({"a": "\u{20ac}", "native": payload, "z": "\u{00e9}"}).to_string();
        let pointer = adm.as_ptr();
        response(&adm, 100).unwrap_native_adm(&mut adm).unwrap();
        assert_eq!(adm, expected);
        assert_eq!(adm.as_ptr(), pointer);
    }

    #[test]
    fn malformed_duplicate_and_unrepresentable_numbers_refuse_without_mutating() {
        // json! cannot express invalid syntax, duplicate keys or overflowing numbers.
        for source in [
            "",
            "{",
            "{} trailing",
            "{\"native\":{},\"ignored\":1e400}",
            "{\"native\":{},\"ignored\":-1e400}",
            "{\"native\":{},\"native\":{}}",
            "{\"native\":{},\"na\\u0074ive\":{}}",
        ] {
            let mut adm = source.to_owned();
            let error = response(source, 100)
                .unwrap_native_adm(&mut adm)
                .unwrap_err();
            assert_eq!(error.code, 4);
            assert_eq!(error.message, "invalid native creative");
            assert_eq!(adm, source);
        }
    }

    #[test]
    fn nonobject_roots_and_native_payloads_refuse_without_mutating() {
        for value in [
            json!(null),
            json!([]),
            json!(1_i32),
            json!(true),
            json!("creative"),
            json!({"native": null}),
            json!({"native": []}),
            json!({"native": 1_i32}),
            json!({"native": "creative"}),
        ] {
            let source = value.to_string();
            let mut adm = source.clone();
            let error = response(&source, 100)
                .unwrap_native_adm(&mut adm)
                .unwrap_err();
            assert_eq!(error.code, 4);
            assert_eq!(error.message, "invalid native creative");
            assert_eq!(adm, source);
        }
    }

    #[test]
    fn the_attached_node_cap_includes_ignored_wrapper_siblings_and_is_inclusive() {
        let source = json!({"native": {}, "ignored": [null, null]}).to_string();
        for (nodes, accepted) in [(0_u32, false), (6, false), (7, true), (8, true)] {
            let mut adm = source.clone();
            let actual = response(&source, nodes).unwrap_native_adm(&mut adm);
            assert_eq!(actual.is_ok(), accepted, "nodes={nodes}");
            assert_eq!(adm, if accepted { "{}" } else { &source });
            if let Err(error) = actual {
                assert_eq!(error.code, 4);
                assert_eq!(error.message, "native creative exceeded parsing limits");
            }
        }
    }

    #[test]
    fn a_large_node_allowance_cannot_override_the_delivered_byte_bound() {
        let source = json!({"native": {}}).to_string();
        for (bytes, accepted) in [
            (0_usize, false),
            (source.len() - 1, false),
            (source.len(), true),
            (source.len() + 1, true),
        ] {
            let response =
                BidderHttpResponse::new(200, Vec::new(), vec![b' '; bytes]).within_node_budget(100);
            let mut adm = source.clone();
            let result = response.unwrap_native_adm(&mut adm);
            assert_eq!(result.is_ok(), accepted, "bytes={bytes}");
            assert_eq!(adm, if accepted { "{}" } else { &source });
            if let Err(error) = result {
                assert_eq!(error.code, 4);
                assert_eq!(error.message, "native creative exceeded parsing limits");
            }
        }
    }

    #[test]
    fn excessive_nesting_is_refused_before_any_string_edit() {
        for (depth, accepted) in [(32_usize, true), (256, false)] {
            let mut nested = json!(null);
            for _ in 0..depth {
                nested = json!([nested]);
            }
            let payload = json!({"x": nested});
            let expected = payload.to_string();
            let source = json!({"native": payload}).to_string();
            let mut adm = source.clone();
            let result = response(&source, 10_000).unwrap_native_adm(&mut adm);
            assert_eq!(result.is_ok(), accepted, "depth={depth}");
            assert_eq!(adm, if accepted { expected } else { source });
        }
    }
}

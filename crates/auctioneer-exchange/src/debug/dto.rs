//! Fixed debug wire types; only sanitized projections belong in these fields.

use super::metadata::{DebugError, DebugMethod};
use auctioneer_bidder::DebugBodyStatus;
use serde::Serialize;
use std::collections::BTreeMap;

/// Keys and values come only from the fixed metadata policy; no input strings
/// are retained here. Duplicate accepted values keep their receipt order.
pub type DebugHeaders = BTreeMap<&'static str, Vec<&'static str>>;

#[derive(Debug, Default, Serialize)]
pub struct DebugInfo {
    pub httpcalls: BTreeMap<String, Vec<DebugHttpCall>>,
    pub truncated: bool,
    pub dropped_calls: u32,
}

#[derive(Debug, Serialize)]
pub struct DebugHttpCall {
    pub call_index: u32,
    pub request: DebugRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<DebugResponse>,
    pub elapsed_ms: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<DebugError>,
}

#[derive(Debug, Serialize)]
pub struct DebugRequest {
    pub method: DebugMethod,
    pub endpoint_origin: String,
    pub headers: DebugHeaders,
    pub body_status: DebugBodyStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DebugResponse {
    pub status: u16,
    pub headers: DebugHeaders,
    pub body_status: DebugBodyStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{DebugHeaders, DebugHttpCall, DebugInfo, DebugRequest, DebugResponse};
    use crate::debug::metadata::{DebugError, DebugMethod};
    use auctioneer_bidder::DebugBodyStatus;
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    fn request() -> DebugRequest {
        DebugRequest {
            method: DebugMethod::Post,
            endpoint_origin: "https://bidder.example".into(),
            headers: DebugHeaders::from([("content-type", vec!["application/json"])]),
            body_status: DebugBodyStatus::Captured,
            body: Some(serde_json::to_string(&json!({"id":"[REDACTED]"})).unwrap()),
        }
    }

    #[test]
    fn successful_empty_debug_has_exactly_the_three_required_fields() {
        assert_eq!(
            serde_json::to_value(DebugInfo::default()).unwrap(),
            json!({"httpcalls":{},"truncated":false,"dropped_calls":0_i32})
        );
    }

    #[test]
    fn wire_shape_uses_a_string_body_and_omits_absent_response_and_error() {
        let call = DebugHttpCall {
            call_index: 2,
            request: request(),
            response: None,
            elapsed_ms: 12,
            error: None,
        };
        assert_eq!(
            serde_json::to_value(&call).unwrap(),
            json!({
                "call_index":2_i32,
                "request":{
                    "method":"POST", "endpoint_origin":"https://bidder.example",
                    "headers":{"content-type":["application/json"]},
                    "body_status":"captured",
                    "body":serde_json::to_string(&json!({"id":"[REDACTED]"})).unwrap()
                },
                "elapsed_ms":12_i32
            })
        );
        let Value::String(body) = &serde_json::to_value(&call).unwrap()["request"]["body"] else {
            panic!("body must be a JSON string, not an embedded object");
        };
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap(),
            json!({"id":"[REDACTED]"})
        );
    }

    #[test]
    fn status_and_error_tokens_are_exact_and_optional_body_is_omitted() {
        let call = DebugHttpCall {
            call_index: 0,
            request: DebugRequest {
                method: DebugMethod::Other,
                endpoint_origin: "https://bidder.example".into(),
                headers: BTreeMap::new(),
                body_status: DebugBodyStatus::Empty,
                body: None,
            },
            response: Some(DebugResponse {
                status: 503,
                headers: BTreeMap::new(),
                body_status: DebugBodyStatus::OmittedType,
                body: None,
            }),
            elapsed_ms: u32::MAX,
            error: Some(DebugError::Transport),
        };
        assert_eq!(
            serde_json::to_value(call).unwrap(),
            json!({
                "call_index":0_i32,
                "request":{
                    "method":"OTHER", "endpoint_origin":"https://bidder.example",
                    "headers":{},"body_status":"empty"
                },
                "response":{"status":503_i32,"headers":{},"body_status":"omitted_type"},
                "elapsed_ms":u32::MAX,"error":"transport"
            })
        );
    }

    #[test]
    fn grouping_is_lexical_and_preserves_positional_gaps_within_each_group() {
        let make_call = |call_index| DebugHttpCall {
            call_index,
            request: request(),
            response: None,
            elapsed_ms: 1,
            error: None,
        };
        let debug = DebugInfo {
            httpcalls: BTreeMap::from([
                ("fake_z_alias".into(), vec![make_call(1)]),
                ("fake_a_alias".into(), vec![make_call(0), make_call(3)]),
            ]),
            truncated: true,
            dropped_calls: 1,
        };
        let wire = serde_json::to_string(&debug).unwrap();
        assert!(wire.find("fake_a_alias").unwrap() < wire.find("fake_z_alias").unwrap());
        let decoded: Value = serde_json::from_str(&wire).unwrap();
        assert_eq!(decoded["httpcalls"]["fake_a_alias"][0]["call_index"], 0_i32);
        assert_eq!(decoded["httpcalls"]["fake_a_alias"][1]["call_index"], 3_i32);
        assert_eq!(decoded["dropped_calls"], 1_i32);
    }
}

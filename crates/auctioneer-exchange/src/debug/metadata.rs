//! Fixed metadata policies; no raw diagnostic or unvalidated URL is reflected.

use crate::fanout::{
    FanoutError, OversizeOrigin, OversizeRequestStage, TimeoutCause, TransportErrorKind,
    UndecodableEncoding,
};
use auctioneer_bidder::{BidderError, BidderErrorKind, is_debug_json_content_type};
use auctioneer_runtime::debug_budget::MAX_DEBUG_INPUT_BYTES;
use serde::Serialize;
use std::time::Duration;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DebugMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
    Trace,
    Connect,
    Other,
}

impl DebugMethod {
    #[must_use]
    pub fn from_token(method: &str) -> Self {
        match method {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "PATCH" => Self::Patch,
            "DELETE" => Self::Delete,
            "HEAD" => Self::Head,
            "OPTIONS" => Self::Options,
            "TRACE" => Self::Trace,
            "CONNECT" => Self::Connect,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugError {
    Timeout,
    Preflight,
    OversizeRequest,
    OversizeResponse,
    Decode,
    Transport,
    ResponseParse,
}

impl DebugError {
    /// Deliberately exhaustive over the neutral taxonomy. No upstream message,
    /// unsupported coding token or HTTP/protocol guess participates in output.
    #[must_use]
    pub fn from_transport(error: &FanoutError) -> Self {
        match error {
            FanoutError::Timeout(TimeoutCause::Bidder | TimeoutCause::AuctionDeadline) => {
                Self::Timeout
            }
            FanoutError::Preflight => Self::Preflight,
            FanoutError::OversizeRequest {
                stage: OversizeRequestStage::Raw | OversizeRequestStage::Encoded,
            }
            | FanoutError::OversizeResponse(OversizeOrigin::RequestHeaders) => {
                Self::OversizeRequest
            }
            FanoutError::OversizeResponse(
                OversizeOrigin::EncodedBody
                | OversizeOrigin::DecodedBody
                | OversizeOrigin::BufferedBody
                | OversizeOrigin::DecoderMemory
                | OversizeOrigin::BrotliWindow
                | OversizeOrigin::HeaderBytes
                | OversizeOrigin::HeaderCount
                | OversizeOrigin::CallResponseCap
                | OversizeOrigin::Unclassified,
            ) => Self::OversizeResponse,
            FanoutError::UndecodableResponse(
                UndecodableEncoding::Malformed
                | UndecodableEncoding::Stacked
                | UndecodableEncoding::Unsupported(_)
                | UndecodableEncoding::Unclassified,
            ) => Self::Decode,
            FanoutError::Transport(transport) => match transport.kind {
                TransportErrorKind::Initial | TransportErrorKind::Unreachable => Self::Transport,
            },
            FanoutError::BadServerResponse | FanoutError::Unreachable | FanoutError::Other(_) => {
                Self::Transport
            }
        }
    }

    /// Apply to the error vector in either adapter result channel. Ordinary
    /// code/message lookalikes never become `response_parse`.
    #[must_use]
    pub fn from_adapter_errors(errors: &[BidderError]) -> Option<Self> {
        errors
            .iter()
            .any(|error| error.kind == BidderErrorKind::ResponseParse)
            .then_some(Self::ResponseParse)
    }
}

/// Forward one positional result's target-defined elapsed observation. This
/// helper does not read a clock, aggregate bidders or manufacture completion time.
#[must_use]
pub fn elapsed_ms(elapsed: Duration) -> u32 {
    u32::try_from(elapsed.as_millis()).unwrap_or(u32::MAX)
}

/// Parse only after capture permission passes. Refusal has no raw-string fallback.
#[must_use]
pub fn endpoint_origin(endpoint: &str) -> Option<String> {
    if u64::try_from(endpoint.len()).ok()? > MAX_DEBUG_INPUT_BYTES {
        return None;
    }
    let parsed = Url::parse(endpoint).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
        return None;
    }
    Some(parsed.origin().ascii_serialization())
}

/// Sanitize one field after permission passes. These display-only MIME tokens
/// cannot prove original body eligibility; the sealed projector owns that proof.
/// Every returned string is static, not a slice of a secret-bearing input.
#[must_use]
pub fn sanitized_header(name: &str, value: &str) -> Option<(&'static str, &'static str)> {
    if u64::try_from(value.len()).ok()? > MAX_DEBUG_INPUT_BYTES {
        return None;
    }
    if name.eq_ignore_ascii_case("content-type") {
        return is_debug_json_content_type(value).then_some(("content-type", "application/json"));
    }
    let trimmed = value.trim_matches([' ', '\t']);
    if name.eq_ignore_ascii_case("content-encoding") {
        for token in ["identity", "gzip", "br", "deflate"] {
            if trimmed.eq_ignore_ascii_case(token) {
                return Some(("content-encoding", token));
            }
        }
    }
    if name.eq_ignore_ascii_case("sec-gpc") && trimmed == "1" {
        return Some(("sec-gpc", "1"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{DebugError, DebugMethod, elapsed_ms, endpoint_origin, sanitized_header};
    use crate::fanout::{
        FanoutError, OversizeOrigin, OversizeRequestStage, TimeoutCause, TransportError,
        TransportErrorKind, UndecodableEncoding,
    };
    use auctioneer_bidder::{BidderError, BidderErrorKind};
    use auctioneer_runtime::debug_budget::MAX_DEBUG_INPUT_BYTES;
    use std::collections::BTreeMap;
    use std::time::Duration;

    #[test]
    fn methods_only_render_fixed_case_sensitive_standard_tokens() {
        for (method, expected) in [
            ("GET", "GET"),
            ("POST", "POST"),
            ("PUT", "PUT"),
            ("PATCH", "PATCH"),
            ("DELETE", "DELETE"),
            ("HEAD", "HEAD"),
            ("OPTIONS", "OPTIONS"),
            ("TRACE", "TRACE"),
            ("CONNECT", "CONNECT"),
            ("post", "OTHER"),
            ("", "OTHER"),
            (" POST", "OTHER"),
            ("GET\r\nsecret", "OTHER"),
            ("SECRET_EXTENSION", "OTHER"),
            ("M-SEARCH", "OTHER"),
        ] {
            let safe = DebugMethod::from_token(method);
            assert_eq!(serde_json::to_value(safe).unwrap(), expected);
            assert!(!format!("{safe:?}").contains("SECRET"));
        }
    }

    #[test]
    fn origins_drop_credentials_and_all_non_origin_components() {
        for (input, expected) in [
            (
                "https://secret:password@Bidder.Example:443/private/uid?token=secret#secret",
                "https://bidder.example",
            ),
            (
                "http://secret:password@bidder.example:80/private?secret#secret",
                "http://bidder.example",
            ),
            (
                "https://bidder.example:8443/private",
                "https://bidder.example:8443",
            ),
            ("http://bidder.example:8080/", "http://bidder.example:8080"),
            (
                "https://secret:password@[2001:db8::1]:443/private?secret#secret",
                "https://[2001:db8::1]",
            ),
            (
                "https://[2001:db8::1]:8443/private",
                "https://[2001:db8::1]:8443",
            ),
            ("HTTPS://bidder.example/", "https://bidder.example"),
        ] {
            assert_eq!(endpoint_origin(input).as_deref(), Some(expected));
        }
        for input in [
            "",
            "/relative/secret",
            "secret",
            "mailto:secret@bidder.example",
            "data:application/json,secret",
            "file:///secret",
            "ftp://bidder.example/secret",
            "https://",
            "https://[bad-ip]/secret",
            "https://bidder.example:secret/",
        ] {
            assert_eq!(
                endpoint_origin(input),
                None,
                "invalid origin must not reflect input"
            );
        }
        let oversized = format!(
            "https://bidder.example/{}",
            "x".repeat(usize::try_from(MAX_DEBUG_INPUT_BYTES).unwrap())
        );
        assert_eq!(endpoint_origin(&oversized), None);
    }

    #[test]
    fn header_values_use_only_fixed_tokens_and_http_optional_whitespace() {
        for (name, value, expected) in [
            (
                "Content-Type",
                "Application/JSON; charset=utf-8; token=SECRET",
                Some(("content-type", "application/json")),
            ),
            (
                "content-type",
                "application/json; note=\"SECRET\"",
                Some(("content-type", "application/json")),
            ),
            ("content-type", "text/html", None),
            ("content-type", "application/json, text/plain", None),
            ("content-type", "application/json; broken=\"", None),
            ("content-type", "application/json\r\nSECRET", None),
            (
                "content-encoding",
                "\t GZip \t",
                Some(("content-encoding", "gzip")),
            ),
            (
                "CONTENT-ENCODING",
                "identity",
                Some(("content-encoding", "identity")),
            ),
            ("content-encoding", "BR", Some(("content-encoding", "br"))),
            (
                "content-encoding",
                "DEFLATE",
                Some(("content-encoding", "deflate")),
            ),
            ("content-encoding", "gzip,br", None),
            ("content-encoding", "gzip; token=SECRET", None),
            ("content-encoding", "zstd", None),
            ("content-encoding", "gzip\n", None),
            ("content-encoding", "\u{a0}gzip", None),
            ("Sec-GPC", "\t1 \t", Some(("sec-gpc", "1"))),
            ("sec-gpc", "0", None),
            ("sec-gpc", "1,1", None),
            ("sec-gpc", "1\n", None),
            ("sec-gpc", "\u{a0}1", None),
            ("sec-gpc", "1; token=SECRET", None),
            ("Authorization", "Bearer SECRET", None),
            ("Proxy-Authorization", "Bearer SECRET", None),
            ("Cookie", "uid=SECRET", None),
            ("Set-Cookie", "uid=SECRET", None),
            ("X-API-Key", "SECRET", None),
            ("X-Forwarded-For", "SECRET", None),
            ("SECRET-HEADER", "SECRET", None),
            (" content-type", "application/json", None),
        ] {
            assert_eq!(sanitized_header(name, value), expected, "{name}");
        }
        let huge = format!(
            "application/json; a={}",
            "x".repeat(usize::try_from(MAX_DEBUG_INPUT_BYTES).unwrap())
        );
        assert_eq!(sanitized_header("content-type", &huge), None);
    }

    #[test]
    fn header_input_cap_content_type_is_inclusive() {
        let cap = usize::try_from(MAX_DEBUG_INPUT_BYTES).unwrap();
        let prefix = "application/json;x=";
        let mut value = format!("{prefix}{}", "a".repeat(cap - prefix.len()));
        assert_eq!(value.len(), cap);
        assert_eq!(
            sanitized_header("Content-Type", &value),
            Some(("content-type", "application/json"))
        );
        value.push('a');
        assert_eq!(sanitized_header("Content-Type", &value), None);
    }

    #[test]
    fn mime_suffixes_are_not_normalized_as_json_headers() {
        for content_type in ["application/json", "Application/JSON; p=\"json+secret\""] {
            assert_eq!(
                sanitized_header("Content-Type", content_type),
                Some(("content-type", "application/json"))
            );
        }
        for content_type in [
            "application/json+secret",
            "application/json+json",
            "Application/JSON+XML; charset=utf-8",
            "application/problem+json",
        ] {
            assert_eq!(
                sanitized_header("Content-Type", content_type),
                None,
                "{content_type}"
            );
        }
    }

    #[test]
    fn header_input_cap_content_encoding_is_inclusive() {
        let cap = usize::try_from(MAX_DEBUG_INPUT_BYTES).unwrap();
        for (input, token) in [
            ("IDENTITY", "identity"),
            ("GZip", "gzip"),
            ("BR", "br"),
            ("Deflate", "deflate"),
        ] {
            let padding = " ".repeat(cap - input.len());
            for mut value in [format!("{padding}{input}"), format!("{input}{padding}")] {
                assert_eq!(value.len(), cap);
                assert_eq!(
                    sanitized_header("Content-Encoding", &value),
                    Some(("content-encoding", token))
                );
                value.push('\t');
                assert_eq!(
                    sanitized_header("Content-Encoding", &value),
                    None,
                    "{token}"
                );
            }
        }
    }

    #[test]
    fn header_input_cap_sec_gpc_is_inclusive() {
        let cap = usize::try_from(MAX_DEBUG_INPUT_BYTES).unwrap();
        let padding = "\t".repeat(cap - 1);
        for mut value in [format!("{padding}1"), format!("1{padding}")] {
            assert_eq!(value.len(), cap);
            assert_eq!(sanitized_header("Sec-GPC", &value), Some(("sec-gpc", "1")));
            value.push(' ');
            assert_eq!(sanitized_header("Sec-GPC", &value), None);
        }
    }

    #[test]
    fn header_input_cap_does_not_admit_unknown_headers() {
        let cap = usize::try_from(MAX_DEBUG_INPUT_BYTES).unwrap();
        for token in ["application/json", "gzip", "1"] {
            let mut value = format!("{token}{}", " ".repeat(cap - token.len()));
            assert_eq!(value.len(), cap);
            assert_eq!(sanitized_header("X-Unknown", &value), None);
            value.push(' ');
            assert_eq!(sanitized_header("X-Unknown", &value), None);
        }
    }

    #[test]
    fn repeated_allowed_headers_keep_receipt_order_without_authorizing_a_body() {
        let mut safe = BTreeMap::<_, Vec<_>>::new();
        for (name, value) in [
            ("CONTENT-ENCODING", "GZIP"),
            ("content-encoding", "br"),
            ("content-type", "application/json; token=SECRET"),
            ("Content-Type", "text/plain"),
            ("Content-Type", "application/json"),
            ("SEC-GPC", "1"),
            ("Cookie", "SECRET"),
        ] {
            if let Some((safe_name, safe_value)) = sanitized_header(name, value) {
                safe.entry(safe_name).or_default().push(safe_value);
            }
        }
        assert_eq!(
            safe,
            BTreeMap::from([
                ("content-encoding", vec!["gzip", "br"]),
                ("content-type", vec!["application/json", "application/json"]),
                ("sec-gpc", vec!["1"]),
            ])
        );
    }

    #[test]
    fn every_transport_cause_maps_to_a_fixed_category_without_its_payload() {
        let mut cases = vec![
            (FanoutError::Timeout(TimeoutCause::Bidder), "timeout"),
            (
                FanoutError::Timeout(TimeoutCause::AuctionDeadline),
                "timeout",
            ),
            (FanoutError::Preflight, "preflight"),
            (FanoutError::Unreachable, "transport"),
            (FanoutError::BadServerResponse, "transport"),
            (
                FanoutError::Other("SECRET-response_parse".into()),
                "transport",
            ),
        ];
        for kind in [TransportErrorKind::Initial, TransportErrorKind::Unreachable] {
            cases.push((FanoutError::Transport(TransportError { kind }), "transport"));
        }
        for stage in [OversizeRequestStage::Raw, OversizeRequestStage::Encoded] {
            cases.push((FanoutError::OversizeRequest { stage }, "oversize_request"));
        }
        for reason in [
            UndecodableEncoding::Malformed,
            UndecodableEncoding::Stacked,
            UndecodableEncoding::Unsupported("SECRET-CODING".into()),
            UndecodableEncoding::Unclassified,
        ] {
            cases.push((FanoutError::UndecodableResponse(reason), "decode"));
        }
        for origin in [
            OversizeOrigin::EncodedBody,
            OversizeOrigin::DecodedBody,
            OversizeOrigin::BufferedBody,
            OversizeOrigin::DecoderMemory,
            OversizeOrigin::BrotliWindow,
            OversizeOrigin::HeaderBytes,
            OversizeOrigin::HeaderCount,
            OversizeOrigin::CallResponseCap,
            OversizeOrigin::Unclassified,
        ] {
            cases.push((FanoutError::OversizeResponse(origin), "oversize_response"));
        }
        cases.push((
            FanoutError::OversizeResponse(OversizeOrigin::RequestHeaders),
            "oversize_request",
        ));
        assert_eq!(cases.len(), 24);
        for (error, expected) in cases {
            let safe = DebugError::from_transport(&error);
            assert_eq!(serde_json::to_value(safe).unwrap(), expected);
            assert!(!format!("{safe:?}").contains("SECRET"));
        }
    }

    #[test]
    fn adapter_parse_category_comes_only_from_typed_provenance() {
        let ordinary = [
            BidderError::failed_to_unmarshal("SECRET syntax error"),
            BidderError::bad_server_response("SECRET node budget"),
            BidderError::bad_input("SECRET media type"),
        ];
        assert_eq!(DebugError::from_adapter_errors(&[]), None);
        assert_eq!(DebugError::from_adapter_errors(&ordinary), None);
        let parse = BidderError {
            code: 999,
            message: "SECRET arbitrary text".into(),
            kind: BidderErrorKind::ResponseParse,
        };
        for errors in [
            vec![parse.clone()],
            vec![ordinary[0].clone(), parse.clone()],
            vec![parse, ordinary[1].clone()],
        ] {
            assert_eq!(
                DebugError::from_adapter_errors(&errors),
                Some(DebugError::ResponseParse)
            );
        }
        assert_eq!(
            serde_json::to_value(DebugError::ResponseParse).unwrap(),
            "response_parse"
        );
    }

    #[test]
    fn positional_elapsed_is_floored_saturated_and_not_recomputed() {
        for (elapsed, expected) in [
            (Duration::ZERO, 0),
            (Duration::from_nanos(999_999), 0),
            (Duration::from_nanos(1_999_999), 1),
            (Duration::from_millis(u64::from(u32::MAX)), u32::MAX),
            (Duration::from_millis(u64::from(u32::MAX) + 1), u32::MAX),
            (Duration::MAX, u32::MAX),
        ] {
            assert_eq!(elapsed_ms(elapsed), expected);
        }
    }
}

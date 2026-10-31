//! auctioneer's bounded renderer for host-generated error responses.
//!
//! The host calls this for every error it renders on auctioneer's behalf:
//! admitted dispatch errors, and the DETACHED pre-admission responses
//! it emits for a request head that fails normalization. It does NOT
//! call it for responses auctioneer's own handlers build — those carry
//! their own bodies — nor for an explicit admission refusal, which
//! supplies a response directly.
//!
//! # Why auctioneer owns this at all
//!
//! The host's default renderer wraps the error's `message` in a JSON
//! envelope, and some messages embed request-derived text: a
//! `NotFound` carries the whole unmatched path. That makes the response
//! size a function of caller input, which the egress accounting cannot
//! charge against a fixed per-response byte reservation, and it echoes
//! caller bytes back onto the wire. A bounded renderer fixes both:
//! every body this module can produce is a compile-time-known
//! `&'static str`.
//!
//! # Two arms, and why it is not one
//!
//! A single `error.kind()` body for everything looks simpler and is
//! WRONG: two of these statuses have pinned wire contracts that
//! predate this renderer, and `kind()` does not spell either of them.
//! A 414 must be `uri too long\n`, not `uri_too_long`; a 431 must be
//! `too many header bytes\n`, not `request_header_fields_too_large`.
//! So:
//!
//! 1. **Prescribed** — the statuses whose exact bytes are pinned get
//!    those bytes verbatim, trailing newline included.
//! 2. **Fallback** — everything else gets `error.kind()` alone, which
//!    is a closed vocabulary of `&'static str`s bounded by its longest
//!    member.
//!
//! NEITHER ARM INTERPOLATES CALLER TEXT. The `message` field is read
//! by nothing here. Responses whose contract does embed request-derived
//! text are emitted by auctioneer's own handlers through the bounded
//! serializer, never through this renderer.
//!
//! # What the host does after this returns
//!
//! It overwrites the status from the error and extends the headers with
//! the error's mandatory protocol headers. This renderer therefore sets
//! the body, content type, and no-store policy and does not set a status:
//! anything it set there would be replaced.

use crate::handlers::egress_deadline::REFUSAL_WRITE_BUDGET;
use edgezero_core::body::Body;
use edgezero_core::error::EdgeError;
use edgezero_core::http::{
    HeaderValue, Response,
    header::{CACHE_CONTROL, CONTENT_TYPE},
};
use edgezero_core::response_egress::ResponseEgressDeadline;

/// The content type every body below is emitted as.
const TEXT_PLAIN: HeaderValue = HeaderValue::from_static("text/plain; charset=utf-8");

/// Pinned body for a request target over the admitted cap (414).
const URI_TOO_LONG_BODY: &str = "uri too long\n";

/// Pinned body for a header block over the admitted caps (431).
const HEADER_BYTES_BODY: &str = "too many header bytes\n";

/// Pinned body for an expired ingress-read deadline (408).
///
/// Shared rather than private: the auction handler answers the same
/// 408 when its own bounded body read expires, and two copies of these
/// bytes could drift apart on the wire.
pub const REQUEST_TIMEOUT_BODY: &str = "request timeout\n";

/// The ceiling one rendered error body may occupy.
///
/// This is the envelope's `refusal_body_bytes` as a compile-time
/// constant, asserted against every body this module can produce. It is
/// duplicated as a literal rather than read from the boot envelope on
/// purpose: the assertion has to hold at COMPILE time for a `&'static
/// str` table, and a runtime envelope read could only fail after the
/// oversized body was already on the wire.
pub const MAX_RENDERED_BODY_BYTES: usize = 256;

/// The bounded body for one error: its pinned bytes, or its kind.
///
/// Split out from [`render_error_response`] so the pinned-versus-kind
/// decision is testable as data, without building a `Response` to read
/// bytes back out of one.
#[must_use]
pub fn error_body(error: &EdgeError) -> &'static str {
    match *error {
        // ARM 1 — PRESCRIBED. Exact wire bytes, trailing newline
        // included. Matching by VARIANT rather than by status keeps
        // this honest: a second variant that happened to map to 414
        // would be a compile-visible addition here, not a silent
        // inheritance of another row's body.
        EdgeError::UriTooLong { .. } => URI_TOO_LONG_BODY,
        EdgeError::RequestHeaderFieldsTooLarge { .. } => HEADER_BYTES_BODY,
        EdgeError::RequestTimeout { .. } => REQUEST_TIMEOUT_BODY,
        // ARM 2 — FALLBACK. `kind()` is a closed vocabulary of
        // `&'static str`s, so this is bounded by its longest member and
        // is independent of caller input.
        //
        // The variants are enumerated rather than left to a bare `_`,
        // so ADDING a variant to the host's error type shows up here as
        // a reviewable line rather than silently inheriting the
        // fallback — which is how a variant with its own pinned wire
        // body would get the wrong one. The trailing `_` is still
        // required because the host's error type is `#[non_exhaustive]`,
        // and it means exactly what it says: an error category this
        // revision has never seen renders its kind.
        EdgeError::BadGateway { .. }
        | EdgeError::BadRequest { .. }
        | EdgeError::ConfigOutOfDate { .. }
        | EdgeError::GatewayTimeout { .. }
        | EdgeError::Internal { .. }
        | EdgeError::MethodNotAllowed { .. }
        | EdgeError::NotFound { .. }
        | EdgeError::NotImplemented { .. }
        | EdgeError::PayloadTooLarge { .. }
        | EdgeError::ResponseTooLarge { .. }
        | EdgeError::ServiceUnavailable { .. }
        | EdgeError::StoreExtraction { .. }
        | EdgeError::Validation { .. }
        | _ => error.kind(),
    }
}

/// Render one host-generated error as a bounded plain-text response.
///
/// Install with `App::set_error_response_renderer`. Every body it can
/// produce is a `&'static str` of at most
/// [`MAX_RENDERED_BODY_BYTES`], so the response size is a compile-time
/// fact rather than a function of the request.
///
/// # The write budget
///
/// Every response this renders carries the short refusal budget. An
/// error reaching here is a static-class answer by the same argument
/// the admission refusal makes: a small fixed body out of a small fixed
/// pool, whose capacity argument is that a held permit drains within
/// that budget.
///
/// Without it these responses are bounded by nothing auctioneer chose. The
/// host consults auctioneer's installed egress policy only for responses
/// auctioneer's handlers return, and an `Err` never becomes one of those —
/// it is rendered HERE instead, so the host falls through to its own
/// default, an order of magnitude wider than the pool is sized for.
/// Every aux route reaches this leg: a status probe, a bidder-info
/// lookup or a GVL health check that fails returns `Err`, and the
/// wrapper that attaches the deadline on the success path never runs.
///
/// Relative rather than absolute, like the admission refusal and for
/// the same reason: this callback is synchronous and holds no clock to
/// sample, and the host resolves the relative form against the true
/// transmission start — which is also the instant the permit begins
/// being held.
#[must_use]
pub fn render_error_response(error: &EdgeError) -> Response {
    let body = error_body(error);
    let mut response = Response::new(Body::from(body));
    response.headers_mut().insert(CONTENT_TYPE, TEXT_PLAIN);
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .extensions_mut()
        .insert(ResponseEgressDeadline::after(REFUSAL_WRITE_BUDGET));
    response
}

#[cfg(test)]
mod tests {
    use super::{
        HEADER_BYTES_BODY, MAX_RENDERED_BODY_BYTES, REQUEST_TIMEOUT_BODY, URI_TOO_LONG_BODY,
        error_body, render_error_response,
    };
    use edgezero_core::error::{EdgeError, RequestBodyOverflowOrigin};
    use edgezero_core::http::{
        HeaderValue, Method, StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    };
    use edgezero_core::response_egress::ResponseEgressDeadline;
    use std::io::Error as IoError;

    /// A long request-derived string, used to prove no arm echoes it.
    fn caller_text() -> String {
        "/caller/supplied/path/".repeat(64)
    }

    /// The whole reason this renderer is a two-arm match: the pinned
    /// rows must emit their EXACT bytes. Asserting the byte string, not
    /// its length — a length-only assertion passes while the wire
    /// format is wrong, which is exactly how a blanket `kind()`
    /// renderer got past review once.
    #[test]
    fn prescribed_arms_emit_their_exact_pinned_bytes() {
        let uri = EdgeError::uri_too_long(caller_text());
        assert_eq!(
            error_body(&uri),
            "uri too long\n",
            "a 414 body is the pinned wire string, not the error kind",
        );
        assert_ne!(
            error_body(&uri),
            uri.kind(),
            "the error kind `uri_too_long` is NOT the 414 wire contract",
        );

        let headers = EdgeError::request_header_fields_too_large(caller_text());
        assert_eq!(
            error_body(&headers),
            "too many header bytes\n",
            "a 431 body is the pinned wire string, not the error kind",
        );
        assert_ne!(
            error_body(&headers),
            headers.kind(),
            "the error kind `request_header_fields_too_large` is NOT the 431 wire contract",
        );

        let timeout = EdgeError::request_timeout(caller_text());
        assert_eq!(
            error_body(&timeout),
            "request timeout\n",
            "a 408 body is the pinned wire string, not the error kind",
        );
        assert_ne!(error_body(&timeout), timeout.kind());
    }

    /// Byte-level, through the built `Response`, so the assertion
    /// covers what actually reaches the wire rather than the table the
    /// body came from. The status is deliberately NOT asserted here:
    /// the host overwrites it from the error after this returns.
    #[test]
    fn rendered_response_carries_the_exact_body_bytes() {
        for (error, expected) in [
            (EdgeError::uri_too_long(caller_text()), URI_TOO_LONG_BODY),
            (
                EdgeError::request_header_fields_too_large(caller_text()),
                HEADER_BYTES_BODY,
            ),
            (
                EdgeError::request_timeout(caller_text()),
                REQUEST_TIMEOUT_BODY,
            ),
        ] {
            let response = render_error_response(&error);
            assert_eq!(
                response.headers().get(CONTENT_TYPE),
                Some(&HeaderValue::from_static("text/plain; charset=utf-8")),
                "every rendered error body is plain text",
            );
            assert_eq!(
                response.headers().get(CACHE_CONTROL),
                Some(&HeaderValue::from_static("no-store"))
            );
            assert!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .is_none()
            );
            let body = response
                .into_body()
                .into_bytes()
                .expect("a static error body is in-memory, never a stream");
            assert_eq!(
                body.as_ref(),
                expected.as_bytes(),
                "the rendered body must be the pinned bytes, byte for byte",
            );
        }
    }

    /// The fallback arm: a variant with no pinned wire body renders its
    /// kind and nothing else. `NotFound` is the one that mattered — it
    /// carries the whole unmatched path, so an interpolating renderer's
    /// size would be a function of caller input.
    #[test]
    fn fallback_arm_emits_the_kind_and_never_the_message() {
        let not_found = EdgeError::not_found(caller_text());
        assert_eq!(
            error_body(&not_found),
            "not_found",
            "an unpinned variant falls back to its kind",
        );
        assert!(
            !error_body(&not_found).contains("/caller/supplied/path/"),
            "no arm may echo caller-supplied text",
        );

        let bad_request = EdgeError::bad_request(caller_text());
        assert_eq!(error_body(&bad_request), "bad_request");
        assert!(!error_body(&bad_request).contains("/caller/supplied/path/"));

        let method = EdgeError::method_not_allowed(&Method::POST, &[Method::GET]);
        assert_eq!(error_body(&method), "method_not_allowed");

        let unavailable = EdgeError::service_unavailable(caller_text());
        assert_eq!(error_body(&unavailable), "service_unavailable");
    }

    #[test]
    fn payload_overflow_origins_use_the_bounded_kind_fallback() {
        for origin in [
            RequestBodyOverflowOrigin::Inbound,
            RequestBodyOverflowOrigin::OutboundUpload,
        ] {
            let error = EdgeError::payload_too_large(origin);
            assert_eq!(error.status(), StatusCode::PAYLOAD_TOO_LARGE);
            assert_eq!(error_body(&error), "payload_too_large");
            let rendered = render_error_response(&error);
            assert_eq!(
                rendered.body().as_bytes(),
                Some(b"payload_too_large".as_slice()),
            );
            assert_eq!(
                rendered.headers()[CONTENT_TYPE],
                "text/plain; charset=utf-8"
            );
            assert_eq!(rendered.headers()[CACHE_CONTROL], "no-store");
            assert!(
                rendered
                    .extensions()
                    .get::<ResponseEgressDeadline>()
                    .is_some()
            );
            assert!(error_body(&error).len() <= MAX_RENDERED_BODY_BYTES);
        }
    }

    /// The bound that makes the per-response byte reservation a true
    /// ceiling: no body either arm can produce exceeds it, whatever the
    /// caller sent. Driven across a spread of variants rather than
    /// asserted about one, because the fallback arm's ceiling is a
    /// property of the whole `kind()` vocabulary.
    #[test]
    fn every_rendered_body_fits_the_refusal_reservation() {
        let text = caller_text();
        let errors = [
            EdgeError::bad_gateway(text.clone()),
            EdgeError::bad_request(text.clone()),
            EdgeError::config_out_of_date(text.clone(), text.clone()),
            EdgeError::gateway_timeout(text.clone()),
            EdgeError::internal(IoError::other(text.clone())),
            EdgeError::method_not_allowed(&Method::POST, &[Method::GET, Method::HEAD]),
            EdgeError::not_found(text.clone()),
            EdgeError::not_implemented(text.clone()),
            EdgeError::payload_too_large(RequestBodyOverflowOrigin::Inbound),
            EdgeError::payload_too_large(RequestBodyOverflowOrigin::OutboundUpload),
            EdgeError::request_header_fields_too_large(text.clone()),
            EdgeError::request_timeout(text.clone()),
            EdgeError::response_too_large(text.clone()),
            EdgeError::service_unavailable(text.clone()),
            EdgeError::uri_too_long(text.clone()),
            EdgeError::validation(text),
        ];
        for error in errors {
            let kind = error.kind();
            let body = error_body(&error);
            assert!(
                body.len() <= MAX_RENDERED_BODY_BYTES,
                "{kind} renders {} bytes, over the {MAX_RENDERED_BODY_BYTES}-byte reservation",
                body.len(),
            );
        }
    }

    /// The statuses the pinned arms belong to, read off the errors
    /// themselves. This ties each pinned body to the wire row it
    /// implements: if a variant's status moved, the body would be
    /// pinned to the wrong response and this would catch it.
    #[test]
    fn pinned_bodies_sit_on_the_statuses_they_were_written_for() {
        assert_eq!(
            EdgeError::uri_too_long("").status(),
            StatusCode::URI_TOO_LONG,
        );
        assert_eq!(
            EdgeError::request_header_fields_too_large("").status(),
            StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
        );
        assert_eq!(
            EdgeError::request_timeout("").status(),
            StatusCode::REQUEST_TIMEOUT,
        );
    }
}

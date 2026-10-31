//! auctioneer's synchronous ingress admission policy.
//!
//! This is the callback `App::set_ingress_admission_policy` installs.
//! The host runs it once per request, BEFORE it takes ownership of the
//! native body, and the decision it returns is what binds a route-class
//! egress permit to the response for the rest of that request's life.
//!
//! # Why this file is the thing that makes the permit binding live
//!
//! [`ingress_lease`](crate::host_primitives::ingress_lease) builds the
//! lease and the completion, and the handlers read them back. But with
//! no policy installed the host uses its own default, which admits every
//! request with an EMPTY grant and an EMPTY completion. A handler then
//! finds no
//! [`AuctioneerIngressLease`](crate::host_primitives::ingress_lease::AuctioneerIngressLease),
//! acquires nothing, and whatever permit it does hold releases when its
//! future drops — at handler return, not at terminal egress. Installing
//! this policy is what moves that release to the response's terminal
//! egress for every route, including the responses no handler produces.
//!
//! # The three route classes, and why the fallbacks are one of them
//!
//! - **auction** — the one body-bearing route. Its egress permit is
//!   acquired by the handler AFTER the auction resolves, because an
//!   egress permit bounds TRANSMISSION and there is nothing to transmit
//!   until then. So admission installs an EMPTY late-bound slot
//!   ([`admit_pending_holding_terminal`]) and takes the auction
//!   ADMISSION slot, which bounds concurrent in-flight auctions, plus
//!   the TERMINAL-response slot its eventual answer will need.
//! - **aux** — bidder info and the GVL health probe. Cheap, capped
//!   work whose whole cost is bounded before the body is read, so the
//!   class permit is acquired here, at admission, and moved straight
//!   into the completion ([`admit`]).
//! - **probe** — `/status`. Separate from aux because it must answer
//!   while the process is degraded. Its independent output pool exists
//!   before boot and cannot be consumed by auction or aux saturation.
//!   A permit remains held through terminal egress; when that pool is
//!   full, admission aborts without allocating an unfunded response.
//! - **the fallback arms** — an unmatched path or a wrong method. These
//!   are answered by the HOST's canonical 404/405, not by a auctioneer
//!   handler, and they still put bytes on the wire, so they still need a
//!   permit held to terminal egress. They draw from the AUX class:
//!   spending auction capacity on a 404 is not a trade auctioneer makes.
//!
//! # Fail-closed before boot, and shed under load
//!
//! Auction and aux requests that arrive before the boot envelope is
//! installed find no gate. They fail closed rather than running ungated.
//! Probes are the exception: their fixed, separately funded output pool
//! exists before bootstrap, so liveness does not depend on boot success.
//!
//! A refusal is itself a response, so it holds a STATIC-class permit
//! like every other response auctioneer emits. When even that class is
//! exhausted there is no bounded buffer to render a refusal into, so the
//! decision is [`AdmissionDecision::Abort`] — the connection is closed
//! or reset at the transport, which is bounded and recoverable, rather
//! than emitting a response nothing accounts for.
//!
//! # What this policy does NOT do
//!
//! It does not classify framing, reject `Content-Encoding`, or apply a
//! per-route Content-Length pre-check on a MATCHED route.
//!
//! The framing half is not deferred, it is elsewhere and already done:
//! the host validates the normalized head before this callback runs
//! (`edgezero_core::ingress::validate_normalized_ingress_parts`, called
//! by each adapter) and fails ambiguous framing closed with a 400 —
//! `Content-Length` together with `Transfer-Encoding`, duplicate
//! `Content-Length` even when byte-identical, a malformed or
//! comma-bearing length, a repeated or non-`chunked` transfer coding. A
//! request that reaches this policy has already passed all of that, so
//! there is nothing here to re-derive and no ambiguity left to see.
//!
//! The per-route BYTE CAP on a matched route stays where it already is:
//! the handler's own bounded read. Moving it here would not add a layer,
//! it would move one — the admission callback is body-blind, so it can
//! only act on a DECLARED length, and a declared length is exactly the
//! case a handler's bounded read already refuses cheaply. What is not
//! yet here is the pre-read rejection of an oversize DECLARED length on
//! a matched route, which would save reading a body destined for a 400.
//! The FALLBACK arms do get their cap here, because their read has no
//! handler behind it to enforce one.
//!
//! `Content-Encoding` rejection likewise stays in matched body handlers,
//! which can apply their canonical-body contracts to requests they hold.

use crate::handlers::egress_deadline::{OVERLOADED_BODY, REFUSAL_WRITE_BUDGET};
use crate::host_primitives::gates::{
    admission_gate, aux_gate, boot_gates, probe_egress_gate, static_egress_gate,
};
use crate::host_primitives::ingress_lease::{
    RouteAdmissionMarker, RouteEgressPermit, admit, admit_pending_holding_terminal,
};
use auctioneer_runtime::StaticEgressPermit;
use auctioneer_runtime::body_caps::DEFAULT_CAP_BYTES;
use edgezero_core::body::Body;
use edgezero_core::http::{HeaderValue, Response, StatusCode, header::CACHE_CONTROL};
use edgezero_core::ingress::{
    AdmissionDecision, BufferedIngressResponse, DEFAULT_INBOUND_READ_BUDGET, IngressGrant,
    IngressHead,
};
use edgezero_core::response_egress::{ResponseEgressCompletion, ResponseEgressDeadline};
use edgezero_core::router::RouteResolution;

/// The manifest route class that selects the auction gates.
pub const AUCTION_CLASS: &str = "auction";

/// The manifest route class that selects the aux gate.
pub const AUX_CLASS: &str = "aux";

/// Liveness probes have reserved output capacity independent of other routes.
///
/// Config work has its own counted pool and verdict cache. It does not
/// bound responses retained by slow readers, so admission takes one
/// independently funded output permit through terminal egress. A full
/// probe pool aborts without rendering another body. Other route classes
/// cannot exhaust this pool, including before a boot envelope exists.
pub const PROBE_CLASS: &str = "probe";

/// The body cap a fallback drain enforces.
///
/// A 404 or a 405 needs its request body drained before the canonical
/// response can be written, and that drain is the only reason the body
/// is read at all — so it gets the small non-body-route cap rather than
/// any route's own. It is deliberately the same 4 KiB every non-body
/// route uses, read from the one place that number is defined.
const FALLBACK_DRAIN_CAP_BYTES: usize = DEFAULT_CAP_BYTES;

/// Wire body for a fallback drain that ran past its read deadline.
const DRAIN_TIMEOUT_BODY: &str = "request timeout\n";

/// Wire body for a fallback drain whose body crossed the cap.
const DRAIN_EXCEEDED_BODY: &str = "request size exceeded max size of 4096 bytes.\n";

/// Decide one request's admission.
///
/// Install with `App::set_ingress_admission_policy`. Runs
/// synchronously, before the host owns the request body, and returns
/// the decision that owns the response's route-class permit from this
/// instant to terminal egress.
///
/// The arms, in the order they are decided:
///
/// 1. **A matched route with a known class.** `auction` takes an
///    admission slot and installs an EMPTY egress slot; `aux` takes an
///    aux permit and installs it immediately. Either way the permit
///    lives in the completion, never in the grant.
/// 2. **A matched route with NO class, or an unrecognized one.** Treated
///    as aux. That is the conservative reading: an unclassified route
///    gets the cheap class's bound rather than the auction class's
///    capacity or no bound at all, and it is logged so the manifest gap
///    is visible.
/// 3. **An unmatched path or a wrong method.** Admitted under the aux
///    class with a bounded drain, so the host's canonical 404/405 is
///    written while a permit is still held.
/// 4. **No capacity, or no gates yet.** Refused with a bounded 503, or
///    aborted when even the static class cannot bound the refusal.
#[must_use]
pub fn decide(head: &IngressHead) -> AdmissionDecision {
    match head.route_resolution() {
        RouteResolution::Matched(route) => match route.class() {
            Some(AUCTION_CLASS) => admit_auction(head),
            Some(AUX_CLASS) => admit_aux(head, false),
            Some(PROBE_CLASS) => admit_probe(head),
            other => {
                // A route whose manifest entry carries no class, or one
                // this revision does not know, is bounded as aux rather
                // than left unbounded. Loud, because it means the
                // manifest and this policy disagree about a wired route.
                log::error!(
                    "route {} has no admission class this revision knows ({other:?}); bounding \
                     it as the aux class",
                    route.pattern(),
                );
                admit_aux(head, false)
            }
        },
        // The host answers both of these itself, with its canonical
        // 404/405 rendered through auctioneer's bounded renderer. They are
        // admitted so that answer is written while an aux permit is
        // held, and their body is drained under a cap first because a
        // response cannot be written on a connection whose request body
        // is still arriving.
        RouteResolution::MethodNotAllowed { .. } | RouteResolution::NotFound => {
            admit_aux(head, true)
        }
        // The host's resolution set is open. A resolution this revision
        // has never seen is bounded as aux, which is the same
        // conservative reading an unclassified matched route gets.
        _ => admit_aux(head, false),
    }
}

/// Admit an auction request: an admission slot now, the egress slot
/// left EMPTY for the handler to fill.
fn admit_auction(head: &IngressHead) -> AdmissionDecision {
    let Some(gate) = admission_gate() else {
        return refuse(head, "the auction admission gate is not installed yet");
    };
    let Some(permit) = gate.try_acquire() else {
        return refuse(head, "the auction admission gate is exhausted");
    };
    // Every admitted auction is going to answer SOMETHING, so its
    // terminal-response slot is reserved here rather than left to the
    // handler. The handler's egress permit cannot stand in for it: that
    // one is acquired only once an auction has resolved, so an early
    // config or validation error installs nothing and its response
    // would go out unaccounted for.
    //
    // Reserved AFTER the concurrency slot and refused when the class is
    // exhausted, because admitting work whose answer cannot be bounded
    // is the thing this reservation exists to prevent. `refuse` draws
    // its own permit from the same class, so a refusal here is still
    // bounded — and when that fails too it aborts.
    let Some(terminal) = static_egress_gate().and_then(|statics| statics.try_acquire()) else {
        return refuse(
            head,
            "no terminal-response slot is available to bound the auction's answer",
        );
    };
    // The admission permit rides the LEASE, and so the grant, rather
    // than the completion. That is correct for this one resource: it
    // bounds concurrent in-flight AUCTIONS, which is work that ends when
    // the handler ends. The EGRESS permit — the one that must outlive
    // the grant, because the router drops the grant before it builds a
    // fallback response — is the late-bound slot inside the same lease,
    // and the handler installs it after the auction resolves. The
    // TERMINAL permit rides the completion for that same reason.
    let (lease, completion) =
        admit_pending_holding_terminal(RouteAdmissionMarker::Auction, Some(permit), Some(terminal));
    AdmissionDecision::Admit {
        completion,
        grant: IngressGrant::new(lease),
        read_deadline: head.read_deadline_after(DEFAULT_INBOUND_READ_BUDGET),
    }
}

/// Reserve one bounded liveness output through terminal egress.
fn admit_probe(head: &IngressHead) -> AdmissionDecision {
    let Some(permit) = probe_egress_gate().try_acquire() else {
        return AdmissionDecision::Abort;
    };
    let (lease, completion) = admit(
        RouteAdmissionMarker::Probe,
        RouteEgressPermit::Probe(permit),
    );
    AdmissionDecision::Admit {
        completion,
        grant: IngressGrant::new(lease),
        read_deadline: head.read_deadline_after(DEFAULT_INBOUND_READ_BUDGET),
    }
}

/// Admit an aux-class request, or a fallback arm, with its aux permit
/// already moved into the completion.
///
/// `drain_before_fallback` selects the host's bounded-drain disposition:
/// true for the unmatched / wrong-method arms, where the host writes its
/// own canonical response after draining, and false for a real aux route
/// whose handler runs.
fn admit_aux(head: &IngressHead, drain_before_fallback: bool) -> AdmissionDecision {
    let Some(gate) = aux_gate() else {
        return refuse(head, "the aux-route class is not installed yet");
    };
    let Some(permit) = gate.try_acquire() else {
        return refuse(head, "the aux-route class is exhausted");
    };
    let Some(gates) = boot_gates() else {
        return refuse(head, "the aux work budget is not installed yet");
    };
    let Ok(work_receipt) = gates.aux_work_derived.reserve(&gates.aux_work_budget) else {
        return refuse(head, "the aux work budget is exhausted");
    };
    let (lease, route_completion) =
        admit(RouteAdmissionMarker::Aux, RouteEgressPermit::Aux(permit));
    let completion = route_completion.join(ResponseEgressCompletion::new(move |_report| {
        drop(work_receipt);
    }));
    let read_deadline = head.read_deadline_after(DEFAULT_INBOUND_READ_BUDGET);
    if drain_before_fallback {
        return AdmissionDecision::ReadBodyBeforeFallback {
            completion,
            grant: IngressGrant::new(lease),
            max_body_bytes: FALLBACK_DRAIN_CAP_BYTES,
            read_deadline,
            on_exceeded: buffered(StatusCode::BAD_REQUEST, DRAIN_EXCEEDED_BODY),
            on_timeout: buffered(StatusCode::REQUEST_TIMEOUT, DRAIN_TIMEOUT_BODY),
        };
    }
    AdmissionDecision::Admit {
        completion,
        grant: IngressGrant::new(lease),
        read_deadline,
    }
}

/// Refuse this request with a bounded 503, or abort when even the
/// refusal cannot be bounded.
///
/// The refusal is a response, so it draws a STATIC-class permit and
/// moves it into the completion: held until the refusal reaches terminal
/// egress, exactly like every other response. With the static class
/// exhausted there is no bounded buffer to render into, so the answer is
/// an abort — which is what makes "every response auctioneer puts on the
/// wire holds a permit" true without qualification.
fn refuse(head: &IngressHead, reason: &str) -> AdmissionDecision {
    let Some(gate) = static_egress_gate() else {
        log::warn!(
            "aborting {} {}: {reason}, and the static-egress class is not installed either",
            head.method(),
            head.target().path(),
        );
        return AdmissionDecision::Abort;
    };
    let Some(permit) = gate.try_acquire() else {
        log::warn!(
            "aborting {} {}: {reason}, and the static-egress class cannot bound the refusal",
            head.method(),
            head.target().path(),
        );
        return AdmissionDecision::Abort;
    };
    log::warn!(
        "refusing {} {}: {reason}",
        head.method(),
        head.target().path(),
    );
    AdmissionDecision::Refuse {
        completion: static_completion(permit),
        response: overloaded_response(),
    }
}

/// A completion that owns `permit` until terminal egress.
fn static_completion(permit: StaticEgressPermit) -> ResponseEgressCompletion {
    ResponseEgressCompletion::new(move |_report| drop(permit))
}

/// The 503 an overloaded isolate answers with.
///
/// A fixed `&'static str` body, so the refusal's size is a compile-time
/// fact rather than a function of the request — the same rule the
/// bounded error renderer follows, for the same reason.
/// Carries the exact headers the buffered fallback responses do — the
/// content type, exact `Content-Length`, and no-store policy — from that same
/// helper rather than restated, so the two shapes cannot drift. The
/// host's own conversion from a `BufferedIngressResponse` is crate-private,
/// which is why the parts are copied across here instead.
fn overloaded_response() -> Response {
    let shape = buffered(StatusCode::SERVICE_UNAVAILABLE, OVERLOADED_BODY);
    // The body is the `&'static str` constant, not a copy of the
    // helper's buffer: a static body cannot be a function of the
    // request, which is the whole reason the refusal is bounded.
    let mut response = Response::new(Body::from(OVERLOADED_BODY));
    *response.status_mut() = shape.status();
    *response.headers_mut() = shape.headers().clone();
    // The refusal's own write budget, carried on the response itself.
    //
    // A refusal goes out through DETACHED egress, which installs the
    // host's own default policy, so the policy auctioneer installs for its
    // own responses is never consulted for this one. The extension is
    // the refusal's only chance to be bounded at all, and without it a
    // slow refusal reader holds a static-class permit for the host's
    // full default — an order of magnitude longer than the drain time
    // the class is sized against.
    //
    // Relative rather than absolute: this callback is synchronous and
    // pre-admission, so it holds no clock to sample, and the host
    // resolves the relative form against the true transmission start.
    // That is also the instant the permit actually starts being held,
    // which is the thing being bounded.
    response
        .extensions_mut()
        .insert(ResponseEgressDeadline::after(REFUSAL_WRITE_BUDGET));
    response
}

/// One buffered terminal response with an exact content length.
fn buffered(status: StatusCode, body: &'static str) -> BufferedIngressResponse {
    let shape = BufferedIngressResponse::text(status, body);
    let mut headers = shape.headers().clone();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    BufferedIngressResponse::new(shape.status(), headers, shape.body().to_vec())
}

#[cfg(test)]
mod tests {
    use super::{
        AUCTION_CLASS, AUX_CLASS, DEFAULT_CAP_BYTES, DRAIN_EXCEEDED_BODY, FALLBACK_DRAIN_CAP_BYTES,
        OVERLOADED_BODY, overloaded_response,
    };
    use crate::handlers::egress_deadline::EgressRefusal;
    use crate::host_primitives::error_renderer::MAX_RENDERED_BODY_BYTES;
    use edgezero_core::http::{
        StatusCode,
        header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE},
    };

    /// The class names this policy matches on are the ones the manifest
    /// writes. Pinned as literals, because a typo on either side would
    /// silently route the auction route into the aux class.
    #[test]
    fn the_class_names_are_the_manifest_spellings() {
        assert_eq!(AUCTION_CLASS, "auction");
        assert_eq!(AUX_CLASS, "aux");
    }

    /// The fallback drain uses the shared non-body-route cap rather
    /// than a number invented here.
    #[test]
    fn the_fallback_drain_cap_is_the_shared_default() {
        assert_eq!(FALLBACK_DRAIN_CAP_BYTES, DEFAULT_CAP_BYTES);
        assert_eq!(FALLBACK_DRAIN_CAP_BYTES, 4_096);
        assert!(
            DRAIN_EXCEEDED_BODY.contains("4096"),
            "the oversize body names the cap it enforced",
        );
    }

    /// Admission overload answers the bytes Appendix F specifies, and
    /// the SAME bytes the egress shed answers.
    ///
    /// The literal is spelled out here rather than compared to the
    /// constant alone: a test that only checks `body ==
    /// OVERLOADED_BODY` passes for every value the constant could
    /// hold, which is exactly how this surface came to answer
    /// `service unavailable\n` against a spec that says `overloaded\n`.
    /// The second assertion is what makes the two surfaces one.
    #[test]
    fn admission_overload_is_the_appendix_f_overload_body() {
        assert_eq!(OVERLOADED_BODY, "overloaded\n");
        assert_eq!(
            OVERLOADED_BODY,
            EgressRefusal::GateFull.body(),
            "admission overload and egress shed are one wire shape",
        );
        let body = overloaded_response()
            .body()
            .as_bytes()
            .expect("a static refusal body is buffered, never a stream")
            .to_vec();
        assert_eq!(
            body, b"overloaded\n",
            "the admission refusal puts those bytes on the wire",
        );
    }

    /// The refusal is a 503 with a fixed body, so a saturated isolate's
    /// answer is a compile-time known size.
    #[test]
    fn the_refusal_is_a_bounded_503() {
        let response = overloaded_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = response
            .body()
            .as_bytes()
            .expect("a static refusal body is buffered, never a stream");
        assert_eq!(body, OVERLOADED_BODY.as_bytes());
        assert!(
            body.len() <= MAX_RENDERED_BODY_BYTES,
            "the refusal fits the bounded-response reservation",
        );
    }

    /// The refusal carries an EXACT `Content-Length` and a content type,
    /// which is what makes it a complete response rather than one the
    /// host has to guess framing for. Read off the same buffered helper
    /// the fallback responses use, so the two shapes cannot drift.
    #[test]
    fn the_refusal_declares_its_own_exact_length() {
        let response = overloaded_response();
        assert_eq!(
            response
                .headers()
                .get(CONTENT_LENGTH)
                .expect("the refusal declares its length"),
            OVERLOADED_BODY.len().to_string().as_str(),
        );
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .expect("the refusal declares its content type"),
            "text/plain; charset=utf-8",
        );
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
    }

    #[test]
    fn buffered_fallback_errors_are_not_cached() {
        let response = super::buffered(StatusCode::BAD_REQUEST, DRAIN_EXCEEDED_BODY);
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
    }
}

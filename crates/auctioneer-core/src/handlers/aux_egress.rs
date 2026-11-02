//! One wrapper every aux route runs inside.
//!
//! The aux routes - status, bidder info, the GVL health probe, and user sync -
//! share admission and egress handling: read the admission lease,
//! keep the route's permit bound to the response
//! through terminal egress, and attach the write deadline the host
//! enforces the transmission leg from. Duplicating that across handlers
//! would be three chances to drift, and the one that drifted would leak
//! a permit silently. So it is done once, here, and each handler is a
//! one-line wrap.
//!
//! Non-body handlers first drain and cache the request under the default cap.
//! Protocols with ordered input checks use the preserving entrypoint so their
//! own config, HTTP checks, and body cap remain authoritative.
//!
//! # What this wrapper does NOT do
//!
//! It **acquires nothing**. The route's permit was acquired ONCE, at
//! admission, by the gate that owns the route class, and moved into the
//! response completion there. A wrapper that acquired a second permit
//! would, at capacity one, block an already-admitted request on a slot
//! it is itself holding — an admitted request rejecting itself.
//!
//! It **transfers nothing** either. The permit is already inside the
//! completion; the lease carries only a zero-payload class marker. That
//! is what makes the guarantee cover framework-generated responses: the
//! host drops the grant before it builds a fallback response, so
//! anything releasable stored there would be freed while the response
//! was still unwritten.
//!
//! # Running without a lease
//!
//! A request that reaches a handler with no auctioneer lease in its grant
//! is not an error here. On a SERVED request it does not happen:
//! `configure_app` installs [`crate::ingress::decide`], which puts a
//! lease in every admitted request's grant. A handler called directly
//! with a low-level context has no admission grant. The wrapper still
//! applies its selected body policy and attaches the write deadline to
//! whatever response the handler returns.

use crate::handlers::egress_deadline::EgressDeadline;
use crate::host_primitives::ingress_lease::{AuctioneerIngressLease, RouteAdmissionMarker};
use auctioneer_runtime::body_caps::DEFAULT_CAP_BYTES;
use edgezero_core::context::RequestContext;
use edgezero_core::error::EdgeError;
use edgezero_core::http::Response;
use std::future::Future;

/// Run one aux handler with its response deadline attached.
///
/// `body` receives the original context after its grant is taken and
/// its inbound body is drained and cached under the default byte cap
/// and admitted read deadline. Both precede handler and config work.
///
/// # Errors
/// Propagates SDK body-read errors (413 on overflow, 408 on read timeout)
/// and wrapped-handler errors to the host's bounded error renderer.
/// The SDK changed overflow from generic 400 to typed `PayloadTooLarge`;
/// this wrapper forwards that 413 with only the `payload_too_large` category,
/// not the SDK message. Cookie-sync's protocol-specific 400 is handled separately.
/// A missing lease is not an error.
pub async fn with_aux_egress<Body, Fut>(
    ctx: RequestContext,
    body: Body,
) -> Result<Response, EdgeError>
where
    Body: FnOnce(RequestContext) -> Fut,
    Fut: Future<Output = Result<Response, EdgeError>>,
{
    with_aux_egress_inner(ctx, body, true).await
}

/// Run an aux protocol handler without reading its request body first.
///
/// The handler owns its config-first checks and dedicated body cap. Admission
/// markers and response write deadlines are handled exactly as for non-body routes.
///
/// # Errors
/// Propagates wrapped-handler errors to the host's bounded error renderer.
/// A missing lease is not an error.
pub async fn with_aux_egress_preserving_body<Body, Fut>(
    ctx: RequestContext,
    body: Body,
) -> Result<Response, EdgeError>
where
    Body: FnOnce(RequestContext) -> Fut,
    Fut: Future<Output = Result<Response, EdgeError>>,
{
    with_aux_egress_inner(ctx, body, false).await
}

async fn with_aux_egress_inner<Body, Fut>(
    ctx: RequestContext,
    body: Body,
    drain_inbound_body: bool,
) -> Result<Response, EdgeError>
where
    Body: FnOnce(RequestContext) -> Fut,
    Fut: Future<Output = Result<Response, EdgeError>>,
{
    // The monotonic source the budget is anchored against, read before
    // the body consumes the context. The ANCHOR itself is sampled after
    // the handler returns — see below.
    let monotonic = ctx.monotonic_clock();
    // Taking the grant is what proves admission. The permit is NOT in
    // here — only the class marker is — so taking it, and dropping it
    // at the end of this scope, releases nothing.
    let admitted_as = ctx
        .take_ingress_grant()
        .and_then(|grant| grant.downcast::<AuctioneerIngressLease>().ok())
        .map(|lease| lease.admission);
    if let Some(marker) = admitted_as
        && matches!(marker, RouteAdmissionMarker::Auction)
    {
        // An auction-class admission reaching an aux handler means the
        // route table and the admission policy disagree about this
        // path. The request is still served — refusing it would turn a
        // wiring mistake into an outage — but it is loud, because the
        // permit bounding this response came from the wrong pool.
        log::error!(
            "aux route served a request admitted by the {} class",
            marker.class()
        );
    }

    // The pinned SDK names its capped, deadline-aware collector `body_bytes`.
    // Leave errors as errors: auctioneer's installed renderer supplies the category
    // body and refusal write deadline without echoing request or source detail.
    if drain_inbound_body {
        ctx.body_bytes(DEFAULT_CAP_BYTES).await?;
    }
    let mut response = body(ctx).await?;
    // Anchored at EGRESS ENTRY — here, where the handler has produced a
    // response and the write leg is about to begin — rather than at the
    // request's own start. The budget bounds the WRITE, and charging it
    // the handler's own elapsed time as well means a request that spent
    // time loading config arrives with less write allowance than the
    // envelope declared, and a slow enough one with none: its answer is
    // shed for a condition the budget was never about.
    //
    // Attached LAST, on whatever response the handler produced, so the
    // error responses get the same bounded write leg the success
    // responses do.
    let deadline = EgressDeadline::from_boot_envelope(monotonic.now());
    response
        .extensions_mut()
        .insert(deadline.to_response_extension());
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::{with_aux_egress, with_aux_egress_preserving_body};
    use crate::handlers::egress_deadline::REFUSAL_WRITE_BUDGET;
    use crate::handlers::sync_http::read_cookie_sync_body;
    use crate::host_primitives::error_renderer::render_error_response;
    use crate::host_primitives::ingress_lease::{RouteAdmissionMarker, admit_pending};
    use auctioneer_runtime::body_caps::{COOKIESYNC_CAP_BYTES, DEFAULT_CAP_BYTES};
    use edgezero_core::app::App;
    use edgezero_core::body::{Body, BodyStream};
    use edgezero_core::context::{BodyKind, RequestContext};
    use edgezero_core::error::{EdgeError, RequestBodyOverflowOrigin};
    use edgezero_core::http::{Method, Response, StatusCode, request_builder};
    use edgezero_core::ingress::{
        AdmissionDecision, DEFAULT_INBOUND_READ_BUDGET, IngressDispatchOutcome, IngressFraming,
        IngressGrant, IngressHeadAccounting,
    };
    use edgezero_core::params::PathParams;
    use edgezero_core::response_egress::{DEFAULT_RESPONSE_WRITE_BUDGET, ResponseEgressDeadline};
    use edgezero_core::router::RouterService;
    use edgezero_core::time::MonotonicInstant;
    use futures::executor::block_on;
    use futures::stream::{self, Stream};
    use futures::task::noop_waker;
    use std::future::Future as _;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};

    /// A low-level context for a direct call, without admission.
    fn context() -> RequestContext {
        context_with_body(Body::empty())
    }

    fn context_with_body(body: Body) -> RequestContext {
        let request = request_builder()
            .uri("/status")
            .header("x-test-context", "preserved")
            .body(body)
            .expect("a static request builds");
        RequestContext::new(request, PathParams::default())
    }

    struct TrackedStream {
        inner: BodyStream,
        drops: Arc<AtomicUsize>,
        polls: Arc<AtomicUsize>,
    }

    impl Stream for TrackedStream {
        type Item = <BodyStream as Stream>::Item;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.polls.fetch_add(1, Ordering::Relaxed);
            self.inner.as_mut().poll_next(cx)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.inner.size_hint()
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for TrackedStream {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn tracked_body(body: Body, drops: &Arc<AtomicUsize>, polls: &Arc<AtomicUsize>) -> Body {
        Body::from_stream(TrackedStream {
            inner: body.into_stream().expect("the fixture is a stream"),
            drops: Arc::clone(drops),
            polls: Arc::clone(polls),
        })
    }

    fn chunk(size: usize) -> <BodyStream as Stream>::Item {
        Ok(Body::from_bytes(vec![b'x'; size])
            .into_bytes()
            .expect("a buffered chunk"))
    }

    #[test]
    fn cookie_sync_post_and_preflight_leave_body_reads_to_the_protocol_handler() {
        for method in [Method::POST, Method::OPTIONS] {
            let request = request_builder()
                .method(method)
                .uri("/cookie_sync")
                .body(Body::from_stream(stream::poll_fn(|_| {
                    panic!("cookie-sync config and HTTP checks must precede body polling");
                })))
                .unwrap();
            let ctx = RequestContext::new(request, PathParams::default());
            let response = block_on(with_aux_egress_preserving_body(ctx, |_ctx| async {
                Ok(Response::new(Body::empty()))
            }))
            .expect("the protocol handler runs without an early body read");
            assert_eq!(response.status(), StatusCode::OK);
            assert!(
                response
                    .extensions()
                    .get::<ResponseEgressDeadline>()
                    .is_some()
            );
        }
    }

    #[test]
    fn cookie_sync_keeps_its_32_kib_body_for_the_protocol_handler() {
        let request = request_builder()
            .method(Method::POST)
            .uri("/cookie_sync")
            .body(Body::from_bytes(vec![b'x'; COOKIESYNC_CAP_BYTES]))
            .unwrap();
        let ctx = RequestContext::new(request, PathParams::default());
        let response = block_on(with_aux_egress_preserving_body(
            ctx,
            |handler_ctx| async move {
                let preserved_request = handler_ctx
                    .into_request()
                    .expect("the wrapper preserves the request");
                let (parts, body) = preserved_request.into_parts();
                let bytes = read_cookie_sync_body(&parts.headers, body)
                    .await
                    .expect("the exact cookie-sync cap reaches its protocol reader");
                assert_eq!(bytes.len(), COOKIESYNC_CAP_BYTES);
                Ok(Response::new(Body::empty()))
            },
        ))
        .expect("the generic aux cap does not reject a valid cookie-sync body");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn exact_cap_is_drained_before_the_handler_with_context_fields_preserved() {
        for body in [
            Body::from_bytes(vec![b'x'; DEFAULT_CAP_BYTES]),
            Body::from_stream(stream::iter([chunk(2_048), chunk(2_048)])),
        ] {
            let runs = Arc::new(AtomicUsize::new(0));
            let counted = Arc::clone(&runs);
            let response = block_on(with_aux_egress(
                context_with_body(body),
                move |ctx| async move {
                    counted.fetch_add(1, Ordering::Relaxed);
                    assert_eq!(
                        ctx.body_kind(),
                        BodyKind::Cached {
                            len: DEFAULT_CAP_BYTES
                        }
                    );
                    assert_eq!(
                        ctx.body_bytes(DEFAULT_CAP_BYTES).await.unwrap().as_ref(),
                        vec![b'x'; DEFAULT_CAP_BYTES].as_slice()
                    );
                    assert_eq!(ctx.uri().path(), "/status");
                    assert_eq!(ctx.headers()["x-test-context"], "preserved");
                    assert!(ctx.take_ingress_grant().is_none());
                    Ok(Response::new(Body::empty()))
                },
            ))
            .expect("the exact cap reaches the handler");
            assert_eq!(runs.load(Ordering::Relaxed), 1);
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    #[test]
    fn oversize_body_never_calls_the_handler() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&runs);
        let error = block_on(with_aux_egress(
            context_with_body(Body::from_bytes(vec![b'x'; DEFAULT_CAP_BYTES + 1])),
            move |_ctx| async move {
                counted.fetch_add(1, Ordering::Relaxed);
                Ok(Response::new(Body::empty()))
            },
        ))
        .expect_err("the SDK rejects cap plus one");
        assert!(matches!(
            error,
            EdgeError::PayloadTooLarge {
                origin: RequestBodyOverflowOrigin::Inbound,
            }
        ));
        assert_eq!(error.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(error.kind(), "payload_too_large");
        assert_eq!(
            render_error_response(&error).body().as_bytes(),
            Some(b"payload_too_large".as_slice()),
        );
        assert_eq!(runs.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn stream_overflow_drops_the_source_without_polling_the_remainder() {
        let drops = Arc::new(AtomicUsize::new(0));
        let polls = Arc::new(AtomicUsize::new(0));
        let body = tracked_body(
            Body::from_stream(stream::iter([
                chunk(DEFAULT_CAP_BYTES),
                chunk(1),
                chunk(DEFAULT_CAP_BYTES),
            ])),
            &drops,
            &polls,
        );
        let error = block_on(with_aux_egress(context_with_body(body), |_ctx| async {
            panic!("an oversized stream must not call the handler");
        }))
        .expect_err("the stream exceeds the cap");
        assert!(matches!(
            error,
            EdgeError::PayloadTooLarge {
                origin: RequestBodyOverflowOrigin::Inbound,
            }
        ));
        assert_eq!(error.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(error.kind(), "payload_too_large");
        assert_eq!(
            render_error_response(&error).body().as_bytes(),
            Some(b"payload_too_large".as_slice()),
        );
        assert_eq!(polls.load(Ordering::Relaxed), 2);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn cancelling_a_pending_read_drops_the_source_and_never_calls_the_handler() {
        let drops = Arc::new(AtomicUsize::new(0));
        let polls = Arc::new(AtomicUsize::new(0));
        let body = tracked_body(Body::from_stream(stream::pending()), &drops, &polls);
        let mut read = Box::pin(with_aux_egress(context_with_body(body), |_ctx| async {
            panic!("a pending read must not call the handler");
        }));
        assert!(
            read.as_mut()
                .poll(&mut Context::from_waker(&noop_waker()))
                .is_pending()
        );
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        drop(read);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert_eq!(polls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_body_source_error_propagates_its_category_with_a_bounded_rendering() {
        let body = Body::from_stream(stream::iter([Err(EdgeError::service_unavailable(
            "private transport detail must not reach the response",
        ))]));
        let error = block_on(with_aux_egress(context_with_body(body), |_ctx| async {
            panic!("a failed body read must not call the handler");
        }))
        .expect_err("the SDK preserves a body source error");
        assert_eq!(error.kind(), "service_unavailable");
        let rendered = render_error_response(&error);
        assert_eq!(
            rendered.body().as_bytes(),
            Some(b"service_unavailable".as_slice())
        );
        assert_eq!(
            format!(
                "{:?}",
                rendered
                    .extensions()
                    .get::<ResponseEgressDeadline>()
                    .unwrap()
            ),
            format!("{:?}", ResponseEgressDeadline::after(REFUSAL_WRITE_BUDGET)),
        );
    }

    #[test]
    fn aux_and_probe_markers_are_valid_and_only_auction_is_incorrect_wiring() {
        for marker in [
            RouteAdmissionMarker::Aux,
            RouteAdmissionMarker::Probe,
            RouteAdmissionMarker::Auction,
        ] {
            testing_logger::setup();
            let router = RouterService::builder()
                .get("/status", |ctx: RequestContext| async {
                    with_aux_egress(ctx, |handler_ctx| async move {
                        assert!(
                            handler_ctx.take_ingress_grant().is_none(),
                            "the wrapper takes the grant first"
                        );
                        assert!(
                            handler_ctx.read_deadline().is_some(),
                            "admission survives on the original context"
                        );
                        Ok(Response::new(Body::empty()))
                    })
                    .await
                })
                .build();
            let mut app = App::new(router);
            app.set_ingress_admission_policy(move |head| {
                let (lease, completion) = admit_pending(marker);
                AdmissionDecision::Admit {
                    completion,
                    grant: IngressGrant::new(lease),
                    read_deadline: head.read_deadline_after(DEFAULT_INBOUND_READ_BUDGET),
                }
            });
            let request = request_builder()
                .uri("/status")
                .body(Body::empty())
                .unwrap();
            let outcome = block_on(app.dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
            .unwrap();
            let IngressDispatchOutcome::Response(mut envelope) = outcome else {
                panic!("all admitted markers still answer");
            };
            assert_eq!(envelope.response_mut().status(), StatusCode::OK);
            testing_logger::validate(|captured| {
                let wiring_errors = captured
                    .iter()
                    .filter(|entry| {
                        entry.level == log::Level::Error
                            && entry
                                .body
                                .contains("aux route served a request admitted by")
                    })
                    .count();
                assert_eq!(
                    wiring_errors,
                    usize::from(matches!(marker, RouteAdmissionMarker::Auction)),
                    "{marker:?}"
                );
            });
        }
    }

    /// The wrapper attaches the write deadline to whatever the handler
    /// returned. Without it the host has nothing to enforce the
    /// transmission leg from, so this is the whole contract.
    #[test]
    fn the_wrapper_attaches_the_response_deadline() {
        let response = block_on(with_aux_egress(context(), |_ctx| async {
            Ok(Response::new(Body::from("ok")))
        }))
        .expect("the handler succeeded");

        assert!(
            response
                .extensions()
                .get::<ResponseEgressDeadline>()
                .is_some(),
            "every aux response carries a write deadline for the host",
        );
    }

    /// An ERROR-status response gets the deadline too. A wrapper that
    /// attached only on the success path would leave every aux failure
    /// with an unbounded write leg, which is the opposite of what a
    /// degraded isolate needs.
    #[test]
    fn an_error_status_response_is_bounded_too() {
        let response = block_on(with_aux_egress(context(), |_ctx| async {
            let mut failed = Response::new(Body::from("nope"));
            *failed.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
            Ok(failed)
        }))
        .expect("a 503 is still a response");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            response
                .extensions()
                .get::<ResponseEgressDeadline>()
                .is_some(),
            "a failing aux response is bounded like a succeeding one",
        );
    }

    /// A handler that returns `Err` is propagated unchanged. The
    /// host renders that error through auctioneer's bounded renderer.
    ///
    /// The error kind alone is not the whole story: an `Err` leaves
    /// through the `?` above the attachment site, so it carries no
    /// deadline of its own and the budget it ends up with is whatever
    /// the renderer gives it. That is asserted here, on the response
    /// the renderer actually builds, because the kind reads identically
    /// whether the write leg is bounded by auctioneer's refusal budget or
    /// by the host's far wider default.
    #[test]
    fn a_handler_error_propagates_unchanged_and_is_rendered_bounded() {
        let outcome: Result<Response, EdgeError> =
            block_on(with_aux_egress(context(), |_ctx| async {
                Err(EdgeError::service_unavailable("handler failed"))
            }));

        let err = outcome.expect_err("the handler's error reaches the caller");
        assert_eq!(err.kind(), "service_unavailable");

        let rendered = render_error_response(&err);
        let attached = rendered
            .extensions()
            .get::<ResponseEgressDeadline>()
            .copied()
            .expect("an aux handler's error is bounded on the way out");
        assert_eq!(
            format!("{attached:?}"),
            format!("{:?}", ResponseEgressDeadline::after(REFUSAL_WRITE_BUDGET)),
            "an aux route's error leg gets the short refusal budget, not the host's \
             own default: the static class is sized against that drain time",
        );
        assert_ne!(
            format!("{attached:?}"),
            format!(
                "{:?}",
                ResponseEgressDeadline::after(DEFAULT_RESPONSE_WRITE_BUDGET)
            ),
            "and specifically not the host default, which is what an unattached \
             response falls through to",
        );
    }

    /// The wrapper runs the body exactly once and hands it the context
    /// with its original request fields, so wrapping a handler does not
    /// replace what the handler can inspect.
    #[test]
    fn the_body_runs_exactly_once_with_the_context() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&runs);
        let response = block_on(with_aux_egress(context(), move |ctx| async move {
            counted.fetch_add(1, Ordering::Relaxed);
            // Reading from the context proves it was not consumed or
            // replaced on the way through.
            assert_eq!(ctx.uri().path(), "/status");
            Ok(Response::new(Body::empty()))
        }))
        .expect("the handler succeeded");

        assert_eq!(runs.load(Ordering::Relaxed), 1, "the body ran once");
        assert_eq!(response.status(), StatusCode::OK);
    }

    /// No lease in the grant is not an error: low-level direct calls
    /// bypass admission and must still be served and bounded.
    #[test]
    fn a_request_without_a_lease_is_still_served_and_bounded() {
        let response = block_on(with_aux_egress(context(), |_ctx| async {
            Ok(Response::new(Body::from("no lease")))
        }))
        .expect("a leaseless request is served");

        assert!(
            response
                .extensions()
                .get::<ResponseEgressDeadline>()
                .is_some(),
            "the deadline does not depend on a lease being present",
        );
    }
}

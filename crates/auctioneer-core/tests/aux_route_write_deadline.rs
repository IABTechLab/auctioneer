//! Each aux route, called individually, carries the response write
//! deadline the host enforces the transmission leg from.
//!
//! # Why this is not covered by what already exists
//!
//! Two tests sit near this property and neither asserts it per route.
//!
//! `aux_egress.rs`'s own unit tests drive `with_aux_egress` with a
//! synthetic closure. They prove the WRAPPER attaches a deadline; they
//! say nothing about which handlers are wrapped, because no handler is
//! involved.
//!
//! `aux_routes_wrapped.rs` proves every registered handler NAMES the
//! wrapper — by scanning the handler sources for the call. That is a
//! source-level property, and it holds against a handler that calls the
//! wrapper and then returns a different response, or one whose
//! deadline-bearing path is never taken.
//!
//! This file closes the join: it invokes each aux handler as a served
//! request would reach it and reads the deadline off the response the
//! handler actually returned. A handler that dropped the wrapper, or
//! rebuilt its response after the wrapper ran, fails here and nowhere
//! else.
//!
//! # Which routes, and why the count is what it is
//!
//! Eight handlers run inside the aux-egress wrapper: `status`,
//! `info_bidders`, `info_bidder`, `health_gvl`, `cookie_sync`,
//! `cookie_sync_preflight`, `setuid`, and `getuids`. Seven are
//! `class = "aux"` in `edgezero.toml`; `/status` is `class = "probe"`,
//! which exempts it from the aux ADMISSION gate but not from the aux
//! EGRESS wrapper — a probe still needs a bounded write leg. So the
//! wrapped set is eight and the admission-class set is seven, and this
//! file covers the wrapped set, which is what the deadline is a
//! property of.
//!
//! # No installed state
//!
//! Every route is called with nothing installed: no boot envelope, no
//! config store, no health policy. That is deliberate. The wrapper's
//! contract is that the deadline is attached UNCONDITIONALLY — to the
//! degraded answer as much as the healthy one — and the degraded answer
//! is the one a bounded write leg matters most for. Whatever status
//! each handler settles on, the deadline must be on it.

#[cfg(test)]
mod tests {
    use auctioneer_core::handlers::{
        cookie_sync, cookie_sync_preflight, getuids, health_gvl, info_bidder, info_bidders, setuid,
        status,
    };
    use edgezero_core::body::Body;
    use edgezero_core::context::RequestContext;
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::Response;
    use edgezero_core::params::PathParams;
    use edgezero_core::response_egress::ResponseEgressDeadline;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::collections::HashMap;
    use std::future::Future;

    /// A request context for `path`, with `params` bound as the route's
    /// path parameters — which is how the router hands a `{name}` route
    /// to its handler.
    fn context_with_method(method: Method, path: &str, params: PathParams) -> RequestContext {
        let request = Request::builder()
            .method(method)
            .uri(path.parse::<Uri>().expect("a static path parses as a URI"))
            .body(Body::from_bytes(Vec::new()))
            .expect("a GET with an empty body builds");
        RequestContext::new(request, params)
    }

    fn context(path: &str, params: PathParams) -> RequestContext {
        context_with_method(Method::GET, path, params)
    }

    fn path_params(name: &str, value: &str) -> PathParams {
        PathParams::new(HashMap::from([(name.to_owned(), value.to_owned())]))
    }

    /// Call one handler and assert the response it returned carries the
    /// deadline. The status is NOT asserted: with nothing installed each
    /// route degrades differently, and which degraded status a route
    /// picks is that route's own test's business. What this asserts is
    /// that whatever came back is bounded.
    fn assert_route_is_bounded<Handler, Fut>(route: &str, ctx: RequestContext, handler: Handler)
    where
        Handler: FnOnce(RequestContext) -> Fut,
        Fut: Future<Output = Result<Response, EdgeError>>,
    {
        let response = block_on(handler(ctx))
            .unwrap_or_else(|err| panic!("`{route}` answers rather than erroring: {err:?}"));
        assert!(
            response
                .extensions()
                .get::<ResponseEgressDeadline>()
                .is_some(),
            "`{route}` returned a response with no write deadline, so the host has nothing \
             to bound its transmission leg from",
        );
    }

    #[test]
    fn status_carries_the_write_deadline() {
        assert_route_is_bounded("/status", context("/status", PathParams::default()), status);
    }

    #[test]
    fn info_bidders_carries_the_write_deadline() {
        assert_route_is_bounded(
            "/info/bidders",
            context("/info/bidders", PathParams::default()),
            info_bidders,
        );
    }

    #[test]
    fn info_bidder_carries_the_write_deadline() {
        assert_route_is_bounded(
            "/info/bidders/{name}",
            context(
                "/info/bidders/fake_fixture_bidder",
                path_params("name", "fake_fixture_bidder"),
            ),
            info_bidder,
        );
    }

    #[test]
    fn health_gvl_carries_the_write_deadline() {
        assert_route_is_bounded(
            "/_auctioneer/health/gvl",
            context("/_auctioneer/health/gvl", PathParams::default()),
            health_gvl,
        );
    }

    #[test]
    fn cookie_sync_carries_the_write_deadline() {
        assert_route_is_bounded(
            "/cookie_sync POST",
            context_with_method(Method::POST, "/cookie_sync", PathParams::default()),
            cookie_sync,
        );
    }

    #[test]
    fn cookie_sync_preflight_carries_the_write_deadline() {
        assert_route_is_bounded(
            "/cookie_sync OPTIONS",
            context_with_method(Method::OPTIONS, "/cookie_sync", PathParams::default()),
            cookie_sync_preflight,
        );
    }

    #[test]
    fn setuid_carries_the_write_deadline() {
        assert_route_is_bounded("/setuid", context("/setuid", PathParams::default()), setuid);
    }

    #[test]
    fn getuids_carries_the_write_deadline() {
        assert_route_is_bounded(
            "/getuids",
            context("/getuids", PathParams::default()),
            getuids,
        );
    }

    /// The set this file covers is the set of handlers that run inside
    /// the wrapper, read from the handler sources rather than from a
    /// list written here.
    ///
    /// Without this, adding another wrapped route would leave it
    /// unasserted while the rows above stayed green — the exact
    /// shape of gap this file exists to close, reopened one route
    /// later.
    #[test]
    fn every_wrapped_route_has_a_write_deadline_case() {
        use std::fs;
        use std::path::Path;

        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers");
        let mut wrapped = Vec::new();
        for entry in fs::read_dir(&dir).expect("the handlers directory is readable") {
            let path = entry.expect("a readable directory entry").path();
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let source = fs::read_to_string(&path).expect("a readable handler source");
            for line in source.lines() {
                // The wrapping shape every aux handler uses: the public
                // `#[action]` body is a single aux-egress wrapper call
                // naming the handler's own private body function.
                let Some(rest) = [
                    "with_aux_egress(ctx, ",
                    "with_aux_egress_preserving_body(ctx, ",
                ]
                .iter()
                .find_map(|prefix| line.trim().strip_prefix(prefix)) else {
                    continue;
                };
                let Some(body_fn) = rest.split(')').next() else {
                    continue;
                };
                if let Some(name) = body_fn.strip_suffix("_body") {
                    wrapped.push(name.to_owned());
                }
            }
        }
        wrapped.sort();

        assert_eq!(
            wrapped,
            [
                "cookie_sync",
                "cookie_sync_preflight",
                "getuids",
                "health_gvl",
                "info_bidder",
                "info_bidders",
                "setuid",
                "status",
            ],
            "the set of handlers running inside `with_aux_egress` changed; every one of them \
             needs a row in this file, because a wrapped route with no row here is a route \
             whose write deadline nothing checks behaviourally",
        );
    }
}

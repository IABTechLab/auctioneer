//! A DECLARED config store whose provider open failed must reach
//! `/status` as a named fault, not as an absent binding.
//!
//! The distinction is the whole point. `resolve_runtime_config` answers
//! "no config store binding" from the bootstrap's `allow_missing_config`
//! rule, because a deploy that declared no `[stores.config]` is the
//! local-dev shape. A deploy that DID declare one and could not open it
//! is a broken deploy, and it must never borrow that rule: the
//! permissive setting would hand it `AuctioneerAppConfig::default()`, which
//! leaves GDPR enforcement OFF.
//!
//! Upstream used to make the two indistinguishable. Cloudflare and
//! Fastly omitted a binding they failed to open, so the declared store
//! vanished from the registry and arrived here as absence; Spin opened
//! during dispatch setup, so the failure became a dispatch error before
//! any route — `/status` included — could report it. The pinned revision
//! instead registers a typed failing handle, and Spin defers its open
//! into the bounded read. Both halves are asserted below through
//! `ConfigStoreHandle::failed_open`, the real constructor the adapters
//! call, rather than through a hand-built error that would pass whatever
//! upstream did.
//!
//! This lives in its own binary on purpose. The `/status` verdict cache
//! is process-global. The separate binary isolates it from other suites,
//! and the per-scenario guard isolates the different failed-open cases
//! from each other's cached verdicts.

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use auctioneer_core::config::runtime::CONFIG_STATUS_HEADER;
    use auctioneer_core::handlers::probe_config::__probe_isolation_for_tests;
    use auctioneer_core::handlers::status;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{ConfigStoreHandle, ConfigStoreOpenFailure};
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::{Response, StatusCode};
    use edgezero_core::params::PathParams;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use futures::executor::block_on;
    use http::{Method, Request, Uri};

    const STORE_ID: &str = "auctioneer_runtime";
    const WIDE_CAP: usize = 1 << 20;

    /// A registry holding exactly one DECLARED id whose open failed.
    /// `from_parts` accepting it is itself part of the contract: the
    /// binding has to stay registered, or the route sees absence.
    fn failed_open_registry(failure: ConfigStoreOpenFailure) -> ConfigRegistry {
        StoreRegistry::single_id(
            STORE_ID.to_owned(),
            ConfigStoreBinding {
                default_key: STORE_ID.to_owned(),
                handle: ConfigStoreHandle::failed_open(failure),
            },
        )
    }

    fn status_ctx(failure: ConfigStoreOpenFailure) -> RequestContext {
        config_funding::install();
        let mut request = Request::builder()
            .method(Method::GET)
            .uri(Uri::from_static("/status"))
            .body(Body::empty())
            .expect("request builds");
        request
            .extensions_mut()
            .insert(failed_open_registry(failure));
        RequestContext::new(request, PathParams::default())
    }

    fn status_body_and_headers(response: Response) -> (StatusCode, String, Vec<(String, String)>) {
        block_on(async {
            let status = response.status();
            let (parts, body) = response.into_parts();
            let headers = parts
                .headers
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_owned(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            (
                status,
                String::from_utf8_lossy(&bytes).into_owned(),
                headers,
            )
        })
    }

    fn header_of(headers: &[(String, String)], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|(header, _value)| header == name)
            .map(|(_header, value)| value.clone())
    }

    /// Drive `/status` against a failing-open binding and assert the
    /// shape a degraded answer must have: 200 rather than 204 or 500,
    /// the machine channel marked `degraded`, and the body naming the
    /// code. A silent 204 here is the exact regression: it is the shape
    /// a HEALTHY deploy with an empty banner answers with, so it would
    /// show an operator green while enforcement ran off the default.
    fn assert_status_reports(failure: ConfigStoreOpenFailure, code: &str) {
        let _isolation = __probe_isolation_for_tests();
        let response =
            block_on(status(status_ctx(failure))).expect("the status handler answers at all");
        let (status_code, body, headers) = status_body_and_headers(response);
        assert_eq!(
            status_code,
            StatusCode::OK,
            "a declared store that failed to open is a fault to name, so the probe answers 200 \
             with a body rather than the empty healthy shape; body {body}",
        );
        assert_eq!(
            header_of(&headers, CONFIG_STATUS_HEADER).as_deref(),
            Some("degraded"),
            "the header is the channel a monitoring rule reads without parsing a body",
        );
        assert!(
            body.ends_with(&format!("config: degraded ({code})\n")),
            "the body has to name the fault so an operator knows which fix applies; body {body}",
        );
    }

    /// Cloudflare's arm. A binding the provider could not resolve is
    /// transient from auctioneer's side — a retry can succeed — so it
    /// reports the backend code rather than the malformed one.
    #[test]
    fn status_reports_an_unavailable_declared_open_failure() {
        assert_status_reports(ConfigStoreOpenFailure::Unavailable, "store_unavailable");
    }

    /// Fastly's arm, and the one that matters most for Spin: a label
    /// declared in neither `spin.toml` nor `runtime-config.toml` is
    /// operator config drift, which no retry fixes. Because Spin now
    /// opens inside the bounded read instead of during dispatch setup,
    /// this failure reaches a route at all — it used to abort dispatch
    /// before `/status` could report anything.
    #[test]
    fn status_reports_an_invalid_declared_open_failure() {
        assert_status_reports(
            ConfigStoreOpenFailure::InvalidConfiguration,
            "malformed_config",
        );
    }

    /// The binding must still be RESOLVABLE. This is the half of the
    /// contract that fails if an adapter returns to omitting a failed
    /// open: `config_store_binding` would answer `None`,
    /// `resolve_runtime_config` would take its absent-binding path, and
    /// a deploy with `allow_missing_config = true` would then serve the
    /// baked default with GDPR enforcement off instead of degrading.
    #[test]
    fn a_failed_declared_open_stays_registered_rather_than_absent() {
        for failure in [
            ConfigStoreOpenFailure::Unavailable,
            ConfigStoreOpenFailure::InvalidConfiguration,
        ] {
            let ctx = status_ctx(failure);
            assert!(
                ctx.config_store_binding(STORE_ID).is_some(),
                "a declared id that failed to open must remain present in the registry, or the \
                 resolver cannot tell a broken store from one that was never declared",
            );
        }
    }
}

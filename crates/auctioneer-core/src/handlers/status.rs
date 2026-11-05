//! `GET /status` handler.

use crate::bidder_registry::cached_registry_error;
use crate::config::resolve;
use crate::config::runtime::{CONFIG_STATUS_HEADER, RuntimeConfigError, resolve_runtime_config};
use crate::handlers::auction as auction_helpers;
use crate::handlers::aux_egress::with_aux_egress;
use crate::handlers::probe_config;
use auctioneer_config_data::limits::MAX_STATUS_BANNER_BYTES;
use edgezero_core::action;
use edgezero_core::body::Body;
use edgezero_core::context::RequestContext;
use edgezero_core::error::EdgeError;
use edgezero_core::http::{Response, StatusCode, response_builder};

const BODY_CAP: usize = MAX_STATUS_BANNER_BYTES;

/// Wrapped, like every aux route — see
/// [`with_aux_egress`](crate::handlers::aux_egress::with_aux_egress)
/// for what the wrapper does and why it is one shared call.
#[action]
pub async fn status(ctx: RequestContext) -> Result<Response, EdgeError> {
    with_aux_egress(ctx, status_body).await
}

async fn status_body(ctx: RequestContext) -> Result<Response, EdgeError> {
    match probe_config::claim_config_work(|verdict| render_response(verdict, Freshness::Cached))
        .await
    {
        probe_config::ConfigClaim::Work(permit) => {
            let fresh = match resolve_verdict(&ctx).await {
                Ok(verdict) => verdict,
                Err(error) => return error.into_response(),
            };
            let response = render_response(&fresh, Freshness::Fresh);
            // The fresh banner and rendering coexist under config accounting.
            // After publication only the single isolate cache owns the banner.
            probe_config::record_verdict(fresh);
            drop(permit);
            response
        }
        probe_config::ConfigClaim::Cached(response) => response,
    }
}

fn render_response(
    verdict: &probe_config::ProbeVerdict,
    freshness: Freshness,
) -> Result<Response, EdgeError> {
    if let Some(error) = cached_registry_error() {
        return RuntimeConfigError::from(error).into_response();
    }
    let (status_code, body) = status_inner_from_verdict(verdict);
    // The config verdict is attached HERE rather than inside the
    // renderer, because this is the handler's only response construction
    // site: every answer — the 204 shape included, and the
    // permissively-degraded answer this handler gives when the config
    // store cannot be read — carries it. A probe is most likely to be
    // looking during exactly that outage, and on the 204 shape the
    // header is the only channel a fault can travel.
    response_builder()
        .status(status_code)
        .header(CONFIG_STATUS_HEADER, freshness.header_value(verdict))
        .body(Body::from_bytes(body))
        .map_err(EdgeError::internal)
}

/// Whether this answer's config verdict was established by THIS
/// request.
///
/// It is reported on the wire because the three states call for
/// different operator reactions, and collapsing them would make the
/// bound on config work look like a config fault. `degraded` means the
/// store is broken; `ok-cached` means the store was not re-consulted on
/// this request because the isolate was already resolving as much config
/// as it is allowed to, which is a load signal about auctioneer rather than
/// a fault in the deployment's config.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Freshness {
    /// This request resolved the config itself.
    Fresh,
    /// The config slots were full, so this answer carries the verdict a
    /// previous request established.
    Cached,
}

impl Freshness {
    /// The `x-auctioneer-config-status` value for this answer.
    ///
    /// A FAULT outranks staleness: a monitoring rule alerting on
    /// `degraded` must still fire when the probe is reporting a cached
    /// fault, because the fault is the thing that needs acting on and it
    /// is still true. Staleness is only reported when there is no fault
    /// to report.
    fn header_value(self, verdict: &probe_config::ProbeVerdict) -> &'static str {
        if verdict.fault.is_some() {
            return "degraded";
        }
        match self {
            Self::Fresh => "ok",
            Self::Cached => "ok-cached",
        }
    }
}

/// Resolve the config and reduce it to the two things an answer needs.
///
/// The shared resolver, so the load policy stays defined in one place,
/// but explicitly on its PERMISSIVE disposition. `/status` is a
/// liveness probe and reaches no privacy decision, so the fail-closed
/// path every other route takes would only convert a config-store
/// outage into a fleet-wide restart loop that cannot fix the store.
///
/// The failure is KEPT rather than dropped, because a probe that
/// degrades silently is indistinguishable from a healthy one: an
/// operator would see green while GDPR enforcement ran off a baked
/// default nobody vouched for. Only the closed code crosses to the
/// response; the full `detail` stays in the log line
/// `degrade_to_default` writes, since this route is unauthenticated and
/// `detail` can carry store-controlled config values.
///
/// The owned `AuctioneerAppConfig` and the resolved `HostConfig` are both
/// dropped before this returns. Only the banner survives, which is why
/// the recorded verdict costs a short string per isolate rather than a
/// retained parsed config.
async fn resolve_verdict(
    ctx: &RequestContext,
) -> Result<probe_config::ProbeVerdict, RuntimeConfigError> {
    // Counted at the site that does the work, which is what lets the
    // saturation regression observe the bound rather than observe the
    // gate it is itself holding.
    probe_config::note_config_resolution();
    let (app_cfg, fault) = match resolve_runtime_config(ctx).await {
        Ok(cfg) => (cfg, None),
        Err(err) => {
            let code = err.status_code();
            (err.degrade_to_default()?, Some(code))
        }
    };
    let baked_host = auction_helpers::host_config_handle();
    let host = resolve::resolved_host(&app_cfg, baked_host);
    let mut banner = host.status_response;
    banner.truncate(utf8_prefix_len(&banner, BODY_CAP));
    Ok(probe_config::ProbeVerdict {
        banner: banner.into_boxed_str(),
        fault,
    })
}

/// Render one verdict as a status line and a body.
///
/// The body is a function of the VERDICT alone. Staleness is reported
/// on the header rather than in the body: a fault is equally true
/// whether this request observed it or a previous one did, and a body
/// that hedged about its own freshness would make the one line an
/// operator reads harder to act on.
fn status_inner_from_verdict(verdict: &probe_config::ProbeVerdict) -> (StatusCode, Vec<u8>) {
    let diagnostic = verdict.fault.map_or_else(String::new, |code| {
        format!("config: degraded ({})\n", code.as_str())
    });
    // Reserve the diagnosis before truncating the banner; a fault never
    // disappears to make room for deployer-controlled text.
    let banner_cap = BODY_CAP
        .saturating_sub(diagnostic.len())
        .saturating_sub(usize::from(!diagnostic.is_empty()));
    let banner = verdict
        .banner
        .get(..utf8_prefix_len(&verdict.banner, banner_cap))
        .unwrap_or_default();
    let separator = !diagnostic.is_empty() && !banner.is_empty() && !banner.ends_with('\n');
    let length = banner
        .len()
        .saturating_add(usize::from(separator))
        .saturating_add(diagnostic.len());
    if length == 0 {
        return (StatusCode::NO_CONTENT, Vec::new());
    }
    let mut body = Vec::with_capacity(length);
    body.extend_from_slice(banner.as_bytes());
    if separator {
        body.push(b'\n');
    }
    body.extend_from_slice(diagnostic.as_bytes());
    (StatusCode::OK, body)
}

fn utf8_prefix_len(text: &str, max_bytes: usize) -> usize {
    let mut length = text.len().min(max_bytes);
    while !text.is_char_boundary(length) {
        length = length.saturating_sub(1);
    }
    length
}

#[cfg(test)]
mod tests {
    #[test]
    fn baked_and_runtime_banner_cap_matches_the_funded_body_cap() {
        assert_eq!(
            u64::try_from(super::BODY_CAP).unwrap(),
            auctioneer_runtime::PROBE_RESPONSE_BODY_MAX_BYTES,
        );
    }
    #[test]
    fn degraded_body_is_byte_bounded_and_preserves_utf8_and_diagnosis() {
        let host = HostConfig {
            status_response: "\u{1f600}".repeat(2_048),
            ..HostConfig::default()
        };
        let (_, body) = status_inner(&host, Some(ConfigStatusCode::StoreUnavailable));
        assert!(
            body.len() <= 8_192,
            "the diagnostic also fits the funded body cap"
        );
        let text = str::from_utf8(&body).expect("truncation preserves UTF-8");
        assert!(text.ends_with("config: degraded (store_unavailable)\n"));
    }

    #[test]
    fn unvalidated_banner_cannot_grow_probe_output_past_the_byte_cap() {
        let host = HostConfig {
            status_response: "x".repeat(0x4000),
            ..HostConfig::default()
        };
        let (_, body) = status_inner(&host, None);
        assert_eq!(body.len(), 8_192);
        assert!(
            body.capacity() <= 8_192,
            "rendering must not overallocate the capped body"
        );
    }

    /// Drives the renderer through a `HostConfig`, the way
    /// [`resolve_verdict`](super::resolve_verdict) produces one, so the
    /// body-shape regressions below stay written against the config
    /// rather than against the verdict struct.
    fn status_inner(host: &HostConfig, fault: Option<ConfigStatusCode>) -> (StatusCode, Vec<u8>) {
        super::status_inner_from_verdict(&super::probe_config::ProbeVerdict {
            banner: host.status_response.clone().into_boxed_str(),
            fault,
        })
    }

    use super::{Body, CONFIG_STATUS_HEADER, RequestContext, StatusCode, status};
    use crate::config::runtime::ConfigStatusCode;
    use auctioneer_config_data::types::HostConfig;
    use edgezero_core::params::PathParams;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::str;

    #[test]
    fn status_carries_config_verdict_header() {
        let _isolation = super::probe_config::PROBE_ISOLATION.lock().unwrap();
        let request = Request::builder()
            .method(Method::GET)
            .uri(Uri::from_static("/status"))
            .body(Body::empty())
            .expect("request builds");
        let response = block_on(status(RequestContext::new(request, PathParams::default())))
            .expect("the status handler answers");
        // This context binds no config store, and the baked bootstrap
        // does not allow a missing config, so the probe degrades: the
        // route still answers, and still answers 200 rather than 500 or
        // the empty 204 shape, which is when an operator most needs it.
        assert_eq!(response.status(), StatusCode::OK);
        let verdict = response
            .headers()
            .get(CONFIG_STATUS_HEADER)
            .and_then(|value| value.to_str().ok());
        assert_eq!(
            verdict,
            Some("degraded"),
            "the verdict rides on every answer, the degraded one included",
        );
    }

    #[test]
    fn default_is_204_empty() {
        let host = HostConfig::default();
        assert_eq!(
            status_inner(&host, None),
            (StatusCode::NO_CONTENT, Vec::new())
        );
    }

    #[test]
    fn configured_is_200_exact_bytes() {
        let host = HostConfig {
            status_response: "OK".into(),
            ..HostConfig::default()
        };
        assert_eq!(status_inner(&host, None), (StatusCode::OK, b"OK".to_vec()));
    }

    /// A `status_response` that already ends in a newline gets no second
    /// one: the separator exists to keep the diagnostic on its own line,
    /// not to pad the body.
    #[test]
    fn degraded_adds_one_separator_at_most() {
        for configured in ["OK", "OK\n"] {
            let host = HostConfig {
                status_response: configured.into(),
                ..HostConfig::default()
            };
            assert_eq!(
                status_inner(&host, Some(ConfigStatusCode::MalformedConfig)),
                (
                    StatusCode::OK,
                    b"OK\nconfig: degraded (malformed_config)\n".to_vec()
                ),
            );
        }
    }

    /// An empty `status_response` has nothing to separate from, so the
    /// degraded body opens on the diagnostic line rather than a blank
    /// one — and it is a 200, because 204 cannot carry it.
    #[test]
    fn degraded_empty_response_is_200_with_no_leading_blank_line() {
        let host = HostConfig::default();
        assert_eq!(
            status_inner(&host, Some(ConfigStatusCode::StoreUnavailable)),
            (
                StatusCode::OK,
                b"config: degraded (store_unavailable)\n".to_vec()
            ),
        );
    }
}

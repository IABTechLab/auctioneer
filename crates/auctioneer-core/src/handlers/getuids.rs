//! `GET /getuids` handler.

use crate::{
    config::{resolve, runtime::resolve_runtime_config},
    handlers::{
        aux_egress::with_aux_egress_preserving_body,
        egress_deadline::EgressDeadline,
        sync_http,
        usersync_keys::{self, KeyringResult},
    },
    host_primitives::clock_handle,
    usersync::{
        cookie::{HostCookie, read_cookie},
        limits::CookieLimits,
        wire::parse_setuid_query,
    },
};
use auctioneer_config_data::types::HostConfig;
use auctioneer_runtime::body_caps::DEFAULT_CAP_BYTES;
use edgezero_core::{
    action,
    context::RequestContext,
    error::EdgeError,
    http::{HeaderMap, Response, StatusCode},
};
use std::future::Future;
use time::OffsetDateTime;

#[action]
pub async fn getuids(ctx: RequestContext) -> Result<Response, EdgeError> {
    with_aux_egress_preserving_body(ctx, getuids_body).await
}

async fn getuids_body(ctx: RequestContext) -> Result<Response, EdgeError> {
    let deadline = EgressDeadline::from_boot_envelope(ctx.request_start());
    let app = match resolve_runtime_config(&ctx).await {
        Ok(app) => app,
        Err(error) => return error.into_response(),
    };
    let host = resolve::resolve_host(&app, auctioneer_config_data::product_host_default());
    let Some(sync) = &host.user_sync else {
        return Ok(sync_http::unavailable());
    };
    let policy = sync_http::SyncBrowserPolicy::new(sync);
    if deadline.is_expired_at(ctx.monotonic_clock().now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            ctx.headers(),
            sync_http::response_header_cap(),
        ));
    }
    ctx.body_bytes(DEFAULT_CAP_BYTES).await?;
    if deadline.is_expired_at(ctx.monotonic_clock().now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            ctx.headers(),
            sync_http::response_header_cap(),
        ));
    }
    let response = match parse_setuid_query(ctx.uri().query()) {
        Ok(_) => {
            respond(
                ctx.headers(),
                &host,
                clock_handle().now(),
                usersync_keys::load(&ctx, deadline),
            )
            .await
        }
        Err(error) => sync_http::setuid_wire_error(error),
    };
    if deadline.is_expired_at(ctx.monotonic_clock().now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            ctx.headers(),
            sync_http::response_header_cap(),
        ));
    }
    Ok(policy
        .decorate(response, ctx.headers(), sync_http::response_header_cap())
        .unwrap_or_else(|_| {
            policy
                .decorate(
                    sync_http::setuid_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "response limit exceeded",
                    ),
                    ctx.headers(),
                    sync_http::response_header_cap(),
                )
                .unwrap_or_else(|_| {
                    sync_http::setuid_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "response limit exceeded",
                    )
                })
        }))
}

pub(crate) async fn respond(
    headers: &HeaderMap,
    host: &HostConfig,
    now: OffsetDateTime,
    key_read: impl Future<Output = KeyringResult>,
) -> Response {
    if host.user_sync.is_none() {
        return sync_http::unavailable();
    }
    let limits = CookieLimits::default();
    let optout = host.host_cookie.optout_cookie.as_ref();
    let optout_only = HostCookie {
        optout_name: optout.map_or("", |cookie| cookie.name.as_str()),
        optout_value: optout.map_or("", |cookie| cookie.value.as_str()),
        ..HostCookie::default()
    };
    let Ok(initial) = read_cookie(headers, "", optout_only, now, &limits) else {
        return sync_http::setuid_error(StatusCode::BAD_REQUEST, "cookie resource limit exceeded");
    };
    let cookie = if initial.cookie.allow_syncs() {
        let host_cookie = HostCookie {
            cookie_name: &host.host_cookie.cookie_name,
            family: &host.host_cookie.family,
            ..HostCookie::default()
        };
        let Ok(keys) = key_read.await else {
            return sync_http::setuid_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "user sync signing unavailable",
            );
        };
        match keys.read_cookie(headers, "uids", host_cookie, now, &limits) {
            Ok(read) => read.cookie,
            Err(error) => {
                return sync_http::setuid_error(StatusCode::BAD_REQUEST, error.message());
            }
        }
    } else {
        initial.cookie
    };
    sync_http::getuids_success(&cookie, sync_http::response_body_cap()).unwrap_or_else(|_| {
        sync_http::setuid_error(StatusCode::INTERNAL_SERVER_ERROR, "response limit exceeded")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::config_store::configured_sync_host;
    use crate::usersync::signed::CookieKeyring;
    use auctioneer_config_data::usersync::OptoutCookie;
    use edgezero_core::http::{HeaderValue, header::COOKIE};
    use futures::executor::block_on;
    use serde_json::json;
    use std::future::ready;

    fn respond(headers: &HeaderMap, host: &HostConfig, now: OffsetDateTime) -> Response {
        let keys = CookieKeyring::parse(&serde_json::to_vec(&json!({
            "active":"test", "keys":[{"id":"test","key":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"}]
        })).unwrap()).unwrap();
        block_on(super::respond(headers, host, now, ready(Ok(keys))))
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_780_000_000).unwrap()
    }

    fn body(response: &Response) -> &[u8] {
        response.body().as_bytes().unwrap()
    }

    #[test]
    fn unconfigured_service_refuses_before_signing_keys() {
        let host = HostConfig::default();
        assert!(host.user_sync.is_none());
        let response = block_on(super::respond(&HeaderMap::new(), &host, now(), async {
            panic!("unconfigured sync must not poll signing keys")
        }));
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body(&response), b"user sync is not configured");
        assert!(!response.headers().contains_key("set-cookie"));
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-credentials")
        );
    }

    #[test]
    fn getuids_host_optout_does_not_read_uids() {
        let mut host = configured_sync_host();
        host.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "optout".into(),
            value: "yes".into(),
        });
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("optout=yes; uids=invalid"));
        let response = respond(&headers, &host, now());
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body(&response), b"{}\n");
        assert!(!response.headers().contains_key("set-cookie"));
    }
}

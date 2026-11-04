//! `POST /cookie_sync` and preflight handlers.

use crate::{
    bidder_registry::registry_handle,
    config::{AuctioneerAppConfig, resolve, runtime::resolve_runtime_config},
    handlers::{
        aux_egress::with_aux_egress_preserving_body,
        egress_deadline::EgressDeadline,
        sync_http::{self, BidderSyncStatus, SyncDebugStatus, SyncUrl},
        usersync_keys::{self, KeyringResult},
    },
    host_primitives::{clock_handle, rng_handle},
    usersync::{
        catalog::Catalog,
        choose::{ChooseError, ChooseInput, ChooseResult, FilterSettings, Rejection, choose},
        cookie::{HostCookie, UidCookie, read_cookie},
        limits::CookieLimits,
        template::{SyncPrivacyMacros, SyncTemplateInput, TemplateError, build_sync_url},
        wire::{CookieSyncWire, parse_cookie_sync_body},
    },
};
use auctioneer_bidder::{BidderRegistry, RegistryBuildError};
use auctioneer_config_data::{types::HostConfig, usersync::SyncType};
use auctioneer_privacy::{
    GdprSignalIn, Signal, SyncDecision, SyncEndpoint, SyncPrivacyContext, SyncPrivacyError,
    SyncPrivacyInput, gcl::Gcl, gvl::GvlStore, resolve_sync_privacy,
};
use auctioneer_runtime::Rng;
use auctioneer_runtime::body_caps::DEFAULT_CAP_BYTES;
use edgezero_core::{
    action,
    body::Body,
    context::RequestContext,
    error::EdgeError,
    http::{HeaderMap, Response, StatusCode},
};
use std::future::Future;
use time::OffsetDateTime;

const JSON_NODE_CAP: u32 = 4_096;
const RETAINED_URL_CAP: usize = 256 * 1024;

type SelectedUrl = (String, String, SyncType);

#[action]
pub async fn cookie_sync(ctx: RequestContext) -> Result<Response, EdgeError> {
    with_aux_egress_preserving_body(ctx, cookie_sync_body).await
}

#[action]
pub async fn cookie_sync_preflight(ctx: RequestContext) -> Result<Response, EdgeError> {
    // Preflight must resolve config and HTTP policy before polling its body.
    // Accepted preflights still drain under the default cap inside the handler.
    with_aux_egress_preserving_body(ctx, cookie_sync_preflight_body).await
}

async fn cookie_sync_preflight_body(ctx: RequestContext) -> Result<Response, EdgeError> {
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
    let response = preflight(ctx.headers(), &host);
    if deadline.is_expired_at(ctx.monotonic_clock().now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            ctx.headers(),
            sync_http::response_header_cap(),
        ));
    }
    if response.status() != StatusCode::NO_CONTENT {
        return Ok(response);
    }
    ctx.body_bytes(DEFAULT_CAP_BYTES).await?;
    if deadline.is_expired_at(ctx.monotonic_clock().now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            ctx.headers(),
            sync_http::response_header_cap(),
        ));
    }
    Ok(response)
}

pub(crate) fn preflight(headers: &HeaderMap, host: &HostConfig) -> Response {
    let Some(sync) = &host.user_sync else {
        return sync_http::unavailable();
    };
    let policy = sync_http::SyncBrowserPolicy::new(sync);
    policy
        .preflight(headers, sync_http::response_header_cap())
        .unwrap_or_else(|_| {
            decorate_response(
                sync_http::cookie_sync_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "response limit exceeded",
                ),
                headers,
                host,
            )
        })
}

pub(crate) fn decorate_response(
    response: Response,
    headers: &HeaderMap,
    host: &HostConfig,
) -> Response {
    let Some(sync) = &host.user_sync else {
        return sync_http::unavailable();
    };
    let policy = sync_http::SyncBrowserPolicy::new(sync);
    policy
        .decorate(response, headers, sync_http::response_header_cap())
        .unwrap_or_else(|_| {
            policy
                .decorate(
                    sync_http::cookie_sync_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "response limit exceeded",
                    ),
                    headers,
                    sync_http::response_header_cap(),
                )
                .unwrap_or_else(|_| {
                    sync_http::cookie_sync_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "response limit exceeded",
                    )
                })
        })
}

async fn cookie_sync_body(ctx: RequestContext) -> Result<Response, EdgeError> {
    let deadline = EgressDeadline::from_boot_envelope(ctx.request_start());
    let monotonic = ctx.monotonic_clock();
    let app = match resolve_runtime_config(&ctx).await {
        Ok(app) => app,
        Err(error) => return error.into_response(),
    };
    let host = resolve::resolve_host(&app, auctioneer_config_data::product_host_default());
    let Some(sync) = &host.user_sync else {
        return Ok(sync_http::unavailable());
    };
    let policy = sync_http::SyncBrowserPolicy::new(sync);
    if deadline.is_expired_at(monotonic.now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            ctx.headers(),
            sync_http::response_header_cap(),
        ));
    }
    let now = clock_handle().now();
    let rng = rng_handle();
    let headers = ctx.headers().clone();
    let keys = usersync_keys::load(&ctx, deadline);
    let response = match ctx.into_request() {
        Ok(request) => {
            let (_, body) = request.into_parts();
            match sync_http::read_cookie_sync_body(&headers, body).await {
                Ok(body_bytes) => {
                    if deadline.is_expired_at(monotonic.now()) {
                        return Ok(sync_http::cpu_overloaded_with_policy(
                            &policy,
                            &headers,
                            sync_http::response_header_cap(),
                        ));
                    }
                    let parsed = parse_cookie_sync_body(&body_bytes, JSON_NODE_CAP);
                    if deadline.is_expired_at(monotonic.now()) {
                        return Ok(sync_http::cpu_overloaded_with_policy(
                            &policy,
                            &headers,
                            sync_http::response_header_cap(),
                        ));
                    }
                    match parsed {
                        Ok(sync_request) => {
                            respond(
                                &sync_request,
                                &headers,
                                &app,
                                &host,
                                now,
                                rng.as_ref(),
                                keys,
                            )
                            .await
                        }
                        Err(error) => sync_http::cookie_sync_wire_error(error),
                    }
                }
                // An ingress read timeout is not CPU overload, even after the route deadline.
                Err(sync_http::SyncReadError::RequestTimeout(error)) => return Err(error),
                Err(error) => error.response(),
            }
        }
        Err(error @ EdgeError::RequestTimeout { .. }) => return Err(error),
        Err(_) => sync_http::cookie_sync_error(
            StatusCode::BAD_REQUEST,
            "Invalid request: body could not be read",
        ),
    };
    if deadline.is_expired_at(monotonic.now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            &headers,
            sync_http::response_header_cap(),
        ));
    }
    Ok(decorate_response(response, &headers, &host))
}

struct ResponseContext<'a, Registry> {
    app: &'a AuctioneerAppConfig,
    host: &'a HostConfig,
    registry: Registry,
}

pub(crate) async fn respond(
    request: &CookieSyncWire,
    headers: &HeaderMap,
    app: &AuctioneerAppConfig,
    host: &HostConfig,
    now: OffsetDateTime,
    rng: &dyn Rng,
    key_read: impl Future<Output = KeyringResult>,
) -> Response {
    respond_with_registry(
        request,
        headers,
        ResponseContext {
            app,
            host,
            registry: registry_handle,
        },
        now,
        rng,
        key_read,
    )
    .await
}

#[expect(
    clippy::too_many_lines,
    reason = "the endpoint pipeline keeps its error precedence visible in one ordered function"
)]
async fn respond_with_registry(
    request: &CookieSyncWire,
    headers: &HeaderMap,
    context: ResponseContext<
        '_,
        impl FnOnce() -> Result<&'static BidderRegistry, RegistryBuildError>,
    >,
    now: OffsetDateTime,
    rng: &dyn Rng,
    key_read: impl Future<Output = KeyringResult>,
) -> Response {
    let ResponseContext {
        app,
        host,
        registry: load_registry,
    } = context;
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
        return bad("cookie resource limit exceeded");
    };
    if !initial.cookie.allow_syncs() {
        return sync_http::cookie_sync_error(StatusCode::UNAUTHORIZED, "User has opted out");
    }
    let account = resolve::resolve_selected_account(
        resolve::AccountSelection::from_requested(request.account.as_deref()),
        app,
        auctioneer_config_data::product_account_default(),
    );
    if account.config.disabled {
        return bad("account is disabled, please reach out to the prebid server host");
    }
    let host_cookie = HostCookie {
        cookie_name: &host.host_cookie.cookie_name,
        family: &host.host_cookie.family,
        ..HostCookie::default()
    };
    let Ok(keys) = key_read.await else {
        return sync_http::cookie_sync_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "user sync signing unavailable",
        );
    };
    let cookie = match keys.read_cookie(headers, "uids", host_cookie, now, &limits) {
        Ok(read) => read.cookie,
        Err(error) => {
            return sync_http::cookie_sync_error(StatusCode::BAD_REQUEST, error.message());
        }
    };
    if !cookie.allow_syncs() {
        return sync_http::cookie_sync_error(StatusCode::UNAUTHORIZED, "User has opted out");
    }
    let gdpr = gdpr_signal(request);
    let mut privacy = match resolve_sync_privacy(SyncPrivacyInput {
        gdpr,
        consent: request.gdpr_consent.as_deref(),
        us_privacy: request.us_privacy.as_deref(),
        gpp: request.gpp.as_deref(),
        gpp_sid: request.gpp_sid.as_deref(),
        account: &account.config,
        host,
        endpoint: SyncEndpoint::CookieSync,
        gvl: GvlStore::baked(),
        gcl: Gcl::baked(),
        now,
    }) {
        Ok(privacy) => privacy,
        Err(error) => return privacy_error(error, gdpr, request.gpp_sid.as_deref()),
    };
    match privacy.cookie_sync_host_permission() {
        SyncDecision::Allowed => {}
        SyncDecision::ResourceLimit => return bad("privacy input limit exceeded"),
        SyncDecision::MalformedTcf
        | SyncDecision::BlockedByActivity
        | SyncDecision::BlockedByDisabledUsersync
        | SyncDecision::BlockedByPrivacy
        | SyncDecision::BlockedByRegulationScope => {
            return sync_http::cookie_sync_success(
                &cookie,
                now,
                &[],
                None,
                sync_http::response_body_cap(),
            )
            .unwrap_or_else(|_| bad("response limit exceeded"));
        }
    }
    let Ok(filter_value) = request.filter_settings_value() else {
        let unchecked = request.filter_settings_unchecked_value().ok().flatten();
        return filter_error(unchecked.as_ref());
    };
    let Ok(filters) = FilterSettings::parse(filter_value.as_ref()) else {
        return filter_error(filter_value.as_ref());
    };
    let requested_limit = match request.limit.as_ref() {
        None => None,
        Some(raw_limit) => match raw_limit.as_i64() {
            Some(number) => Some(number),
            None => return bad("invalid limit"),
        },
    };
    let registry = match load_registry() {
        Ok(value) => value,
        Err(error) => {
            log::error!("{error}");
            return sync_http::cookie_sync_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Bidder registry initialization failed",
            );
        }
    };
    let Ok(catalog) = Catalog::from_registry(host, app, registry) else {
        return bad("invalid bidder catalog");
    };
    let cooperative = resolve::resolve_cooperative(request.coop_sync, &account.config, host);
    let limit = resolve::resolve_limit(requested_limit, &account.config);
    let chosen = match choose(
        ChooseInput {
            catalog: &catalog,
            requested: &request.bidders,
            cooperative,
            priority_groups: account.effective_priority_groups(host),
            filters: &filters,
            cookie: &cookie,
            now,
            limit,
            debug: request.debug,
        },
        &mut privacy,
        rng,
    ) {
        Ok(selection) => selection,
        Err(ChooseError::PrivacyResourceLimit) => {
            return bad("privacy input limit exceeded");
        }
        Err(_) => return bad("invalid bidder list"),
    };
    chosen_response(
        chosen,
        &cookie,
        now,
        request.debug,
        host,
        &account.selection,
        &privacy,
    )
}

fn gdpr_signal(request: &CookieSyncWire) -> GdprSignalIn<'_> {
    match request.gdpr.as_ref().map(|value| value.get()) {
        None => GdprSignalIn::CookieAbsent,
        Some("null") => GdprSignalIn::CookieNull,
        Some(raw) => match serde_json::from_str::<i64>(raw) {
            Ok(value) => GdprSignalIn::CookieNumber(value),
            Err(_) => GdprSignalIn::CookieOther,
        },
    }
}

fn chosen_response(
    chosen: ChooseResult,
    cookie: &UidCookie,
    now: OffsetDateTime,
    debug_enabled: bool,
    host: &HostConfig,
    account: &resolve::AccountSelection,
    privacy: &SyncPrivacyContext<'_>,
) -> Response {
    let Some(sync) = &host.user_sync else {
        return sync_http::unavailable();
    };
    let account_name = match account {
        resolve::AccountSelection::Default => None,
        resolve::AccountSelection::Named(name) => Some(name.as_str()),
    };
    let gdpr = match privacy.parsed_request_signal {
        Signal::Yes => "1",
        Signal::No => "0",
        Signal::Ambiguous => "",
    };
    let macros = SyncPrivacyMacros {
        gdpr,
        gdpr_consent: privacy.selected_consent().unwrap_or(""),
        us_privacy: privacy.selected_usp().unwrap_or(""),
        gpp: privacy.validated_gpp().unwrap_or(""),
        gpp_sid: privacy.raw_gpp_sid().unwrap_or(""),
    };
    let mut urls = Vec::new();
    let mut retained_url_bytes = 0_usize;
    for selected in chosen.selected {
        let configured_endpoint = match selected.sync_type {
            SyncType::Iframe => selected.bidder.user_sync.iframe.as_ref(),
            SyncType::Redirect => selected.bidder.user_sync.redirect.as_ref(),
        };
        let Some(endpoint) = configured_endpoint else {
            continue;
        };
        let built_result = build_sync_url(SyncTemplateInput {
            host: sync,
            endpoint,
            callback_bidder: &selected.normalized_name,
            bidder_name: &selected.normalized_name,
            sync_type: selected.sync_type,
            account: account_name,
            privacy: macros,
        });
        match built_result {
            Ok(built) => {
                if push_selected_url(
                    &mut urls,
                    &mut retained_url_bytes,
                    selected.name,
                    built.url,
                    selected.sync_type,
                )
                .is_err()
                {
                    return bad("sync response exceeds resource limit");
                }
            }
            Err(TemplateError::OutputTooLong) => return bad("sync URL exceeds resource limit"),
            Err(_) => {
                return sync_http::cookie_sync_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error",
                );
            }
        }
    }
    let statuses = urls
        .iter()
        .map(|(name, url, kind)| BidderSyncStatus {
            bidder: name,
            no_cookie: true,
            usersync: Some(SyncUrl { url, kind: *kind }),
        })
        .collect::<Vec<_>>();
    let debug = chosen
        .debug
        .iter()
        .filter_map(|item| {
            let error = match item.reason {
                Rejection::Selected => return None,
                Rejection::UnknownBidder => "unknown bidder",
                Rejection::Unconfigured => "unconfigured bidder",
                Rejection::SharedFamily => "duplicate sync family",
                Rejection::Filtered => "filtered",
                Rejection::AlreadySynced => "already synced",
                Rejection::ActivityDenied => "activity denied",
                Rejection::PrivacyDenied => "privacy denied",
                Rejection::Disabled => "syncer disabled",
                Rejection::Skipped => "regulation scope skipped",
            };
            Some(SyncDebugStatus {
                bidder: &item.name,
                error,
            })
        })
        .collect::<Vec<_>>();
    sync_http::cookie_sync_success(
        cookie,
        now,
        &statuses,
        debug_enabled.then_some(debug.as_slice()),
        sync_http::response_body_cap(),
    )
    .unwrap_or_else(|_| bad("response limit exceeded"))
}

pub(crate) fn push_selected_url(
    urls: &mut Vec<SelectedUrl>,
    retained_bytes: &mut usize,
    name: String,
    url: String,
    kind: SyncType,
) -> Result<(), ()> {
    let next = retained_bytes
        .checked_add(url.len())
        .filter(|bytes| *bytes <= RETAINED_URL_CAP)
        .ok_or(())?;
    *retained_bytes = next;
    urls.push((name, url, kind));
    Ok(())
}

fn bad(message: &'static str) -> Response {
    sync_http::cookie_sync_error(StatusCode::BAD_REQUEST, message)
}

fn bad_owned(mut message: String) -> Response {
    let mut response = bad("");
    message.push('\n');
    *response.body_mut() = Body::from_bytes(message.into_bytes());
    response
}

fn filter_error(value: Option<&serde_json::Value>) -> Response {
    if value
        .and_then(serde_json::Value::as_object)
        .is_some_and(|object| object.keys().any(|key| key != "iframe" && key != "image"))
    {
        return bad("error parsing filtersettings: invalid shape");
    }
    for (branch, raw_rule) in ["iframe", "image"]
        .into_iter()
        .filter_map(|name| value?.get(name).map(|rule| (name, rule)))
    {
        let Some(rule) = raw_rule.as_object() else {
            continue;
        };
        if rule.keys().any(|key| key != "filter" && key != "bidders") {
            return bad("error parsing filtersettings: invalid shape");
        }
        if let Some(bidders) = rule.get("bidders")
            && !matches!(bidders, serde_json::Value::String(all) if all == "*")
            && !matches!(bidders, serde_json::Value::Array(_))
        {
            return bad_owned(format!(
                "error parsing filtersettings.{branch}: invalid bidders type. must either be a string '*' or a string array"
            ));
        }
        if let Some(filter) = rule.get("filter").and_then(serde_json::Value::as_str)
            && !matches!(filter, "include" | "exclude")
        {
            return bad_owned(format!(
                "error parsing filtersettings.{branch}: invalid filter value. must be either 'include' or 'exclude'"
            ));
        }
    }
    bad("error parsing filtersettings: invalid shape")
}

fn privacy_error(
    error: SyncPrivacyError,
    gdpr: GdprSignalIn<'_>,
    raw_gpp_sid: Option<&str>,
) -> Response {
    bad(match error {
        SyncPrivacyError::MalformedGdpr => "GDPR signal should be integer 0 or 1",
        SyncPrivacyError::MalformedGppSid => {
            "invalid gpp_sid encoding, must be a csv list of integers"
        }
        SyncPrivacyError::MalformedGpp => "error parsing GPP header, header must have type=3",
        SyncPrivacyError::MissingRequiredConsent => {
            // This error follows SID validation; specified SID scope overrides legacy GDPR.
            let parsed_yes = raw_gpp_sid.filter(|sid| !sid.is_empty()).map_or(
                matches!(gdpr, GdprSignalIn::CookieNumber(1)),
                |sid| {
                    sid.split(',')
                        .any(|section| section.parse::<u16>() == Ok(2))
                },
            );
            if parsed_yes {
                "gdpr_consent is required if gdpr=1"
            } else {
                "gdpr_consent is required. gdpr is not specified and is assumed to be 1 by the server. set gdpr=0 to exempt this request"
            }
        }
        SyncPrivacyError::GppSidLimitExceeded
        | SyncPrivacyError::GppLimitExceeded
        | SyncPrivacyError::InputLimitExceeded => "privacy input limit exceeded",
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::bidder_fixture::fixtures::registry;
    use crate::handlers::tests::bidder_registry;
    use crate::test_support::config_store::configured_sync_host;
    use crate::usersync::signed::CookieKeyring;
    use crate::usersync::wire;
    use auctioneer_config_data::usersync::OptoutCookie;
    use auctioneer_runtime::SeededRng;
    use edgezero_core::http::{HeaderValue, header::COOKIE};
    use futures::executor::block_on;
    use serde_json::json;
    use std::cell::Cell;
    use std::future::ready;

    pub(in crate::handlers) async fn wire_response(
        request: &CookieSyncWire,
        headers: &HeaderMap,
        app: &AuctioneerAppConfig,
        host: &HostConfig,
        now: OffsetDateTime,
        rng: &dyn Rng,
        key_read: impl Future<Output = KeyringResult>,
    ) -> Response {
        super::respond_with_registry(
            request,
            headers,
            ResponseContext {
                app,
                host,
                registry: || Ok(bidder_registry::registry_handle()),
            },
            now,
            rng,
            key_read,
        )
        .await
    }

    fn keys() -> CookieKeyring {
        CookieKeyring::parse(&serde_json::to_vec(&json!({
            "active":"test", "keys":[{"id":"test","key":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"}]
        })).unwrap()).unwrap()
    }

    fn respond(
        request: &CookieSyncWire,
        headers: &HeaderMap,
        app: &AuctioneerAppConfig,
        host: &HostConfig,
        now: OffsetDateTime,
        rng: &dyn Rng,
    ) -> Response {
        block_on(super::respond_with_registry(
            request,
            headers,
            ResponseContext {
                app,
                host,
                registry: || Ok(registry()),
            },
            now,
            rng,
            ready(Ok(keys())),
        ))
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_780_000_000).unwrap()
    }

    fn body(response: &Response) -> &[u8] {
        response.body().as_bytes().unwrap()
    }

    #[test]
    fn unconfigured_service_refuses_before_registry_or_signing_keys() {
        let host = HostConfig::default();
        assert!(host.user_sync.is_none());
        let bytes = serde_json::to_vec(&json!({"gdpr": 0_i32})).unwrap();
        let request = wire::parse_cookie_sync_body(&bytes, 64).unwrap();
        let response = block_on(super::respond_with_registry(
            &request,
            &HeaderMap::new(),
            ResponseContext {
                app: &AuctioneerAppConfig::default(),
                host: &host,
                registry: || panic!("unconfigured sync must not load the registry"),
            },
            now(),
            &SeededRng::new(1),
            async { panic!("unconfigured sync must not poll signing keys") },
        ));
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
    fn unconfigured_preflight_and_decoration_do_not_advertise_browser_access() {
        let host = HostConfig::default();
        let mut headers = HeaderMap::new();
        headers.insert(
            "origin",
            HeaderValue::from_static("https://publisher.example.test"),
        );
        for response in [
            super::preflight(&headers, &host),
            super::decorate_response(sync_http::setuid_error(StatusCode::OK, ""), &headers, &host),
        ] {
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(body(&response), b"user sync is not configured");
            assert!(
                !response
                    .headers()
                    .contains_key("access-control-allow-origin")
            );
            assert!(
                !response
                    .headers()
                    .contains_key("access-control-allow-credentials")
            );
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }

    #[test]
    fn registry_failure_is_not_an_invalid_bidder_catalog() {
        let bytes = serde_json::to_vec(&json!({"gdpr": 0_i32})).unwrap();
        let request = wire::parse_cookie_sync_body(&bytes, 64).unwrap();
        let response = block_on(super::respond_with_registry(
            &request,
            &HeaderMap::new(),
            ResponseContext {
                app: &AuctioneerAppConfig::default(),
                host: &configured_sync_host(),
                registry: || {
                    Err(RegistryBuildError::MissingBinding {
                        bidder: "fake_missing",
                    })
                },
            },
            now(),
            &SeededRng::new(1),
            ready(Ok(keys())),
        ));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body(&response), b"Bidder registry initialization failed\n");
    }

    #[test]
    fn catalog_loading_follows_optout_account_privacy_filter_and_limit_checks() {
        let app: AuctioneerAppConfig =
            toml::from_str("schema_version = 1\n[accounts.disabled]\ndisabled = true\n").unwrap();
        let mut host = configured_sync_host();
        host.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "optout".into(),
            value: "yes".into(),
        });
        host.aliases
            .insert("fake_alias_a".into(), "fake_missing_base".into());
        for (input, opted_out, status, expected, catalog_expected) in [
            (
                json!({"gdpr": 0_i32, "account": "disabled"}),
                true,
                StatusCode::UNAUTHORIZED,
                "User has opted out\n",
                false,
            ),
            (
                json!({"gdpr": 0_i32, "account": "disabled"}),
                false,
                StatusCode::BAD_REQUEST,
                "account is disabled, please reach out to the prebid server host\n",
                false,
            ),
            (
                json!({"gdpr": 1_i32}),
                false,
                StatusCode::BAD_REQUEST,
                "gdpr_consent is required if gdpr=1\n",
                false,
            ),
            (
                json!({"gdpr": 0_i32, "filterSettings": {"extra": true}}),
                false,
                StatusCode::BAD_REQUEST,
                "error parsing filtersettings: invalid shape\n",
                false,
            ),
            (
                json!({"gdpr": 0_i32, "limit": 1.5_f64}),
                false,
                StatusCode::BAD_REQUEST,
                "invalid limit\n",
                false,
            ),
            (
                json!({"gdpr": 0_i32}),
                false,
                StatusCode::BAD_REQUEST,
                "invalid bidder catalog\n",
                true,
            ),
        ] {
            let request =
                wire::parse_cookie_sync_body(&serde_json::to_vec(&input).unwrap(), 64).unwrap();
            let mut headers = HeaderMap::new();
            if opted_out {
                headers.insert(COOKIE, HeaderValue::from_static("optout=yes"));
            }
            let catalog_loaded = Cell::new(false);
            let response = block_on(super::respond_with_registry(
                &request,
                &headers,
                ResponseContext {
                    app: &app,
                    host: &host,
                    registry: || {
                        catalog_loaded.set(true);
                        Ok(registry())
                    },
                },
                now(),
                &SeededRng::new(1),
                async {
                    assert!(!catalog_loaded.get());
                    Ok(keys())
                },
            ));
            assert_eq!(response.status(), status, "{input}");
            assert_eq!(body(&response), expected.as_bytes(), "{input}");
            assert_eq!(catalog_loaded.get(), catalog_expected, "{input}");
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }

    #[test]
    fn privacy_limit_diagnostics_are_distinct_from_malformed_signals() {
        for error in [
            SyncPrivacyError::GppSidLimitExceeded,
            SyncPrivacyError::GppLimitExceeded,
        ] {
            let response = privacy_error(error, GdprSignalIn::CookieNumber(0), None);
            assert_eq!(body(&response), b"privacy input limit exceeded\n");
        }
        assert_eq!(
            body(&privacy_error(
                SyncPrivacyError::MalformedGppSid,
                GdprSignalIn::CookieNumber(0),
                None
            )),
            b"invalid gpp_sid encoding, must be a csv list of integers\n"
        );
        assert_eq!(
            body(&privacy_error(
                SyncPrivacyError::MalformedGpp,
                GdprSignalIn::CookieNumber(0),
                None
            )),
            b"error parsing GPP header, header must have type=3\n"
        );
    }

    fn over_limit_tcf() -> &'static str {
        include_str!(
            "../../../auctioneer-privacy/tests/data/tcf_v2/pub_restrictions_exceeds_2048_rejected.txt"
        )
        .trim()
    }

    fn active_gdpr_host() -> HostConfig {
        let mut host = configured_sync_host();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.host_vendor_id = Some(32);
        host
    }

    #[test]
    fn selected_url_retention_cap_is_inclusive_and_rejects_before_push() {
        let mut urls = Vec::new();
        let mut retained = 0_usize;
        let cap = 256 * 1024;
        for index in 0_usize..8 {
            push_selected_url(
                &mut urls,
                &mut retained,
                format!("fake_bidder_{index}"),
                "x".repeat(32 * 1024),
                SyncType::Iframe,
            )
            .unwrap();
        }
        assert_eq!(retained, cap);
        assert_eq!(urls.len(), 8);
        assert!(
            push_selected_url(
                &mut urls,
                &mut retained,
                "fake_second".into(),
                "x".into(),
                SyncType::Redirect,
            )
            .is_err()
        );
        assert_eq!(retained, cap);
        assert_eq!(urls.len(), 8);
    }

    #[test]
    fn cookie_sync_tcf_resource_exhaustion_is_terminal_bad_request() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "bidders": ["fake_aaa"],
            "gdpr": 1_i32,
            "gdpr_consent": over_limit_tcf(),
        }))
        .unwrap();
        let request = wire::parse_cookie_sync_body(&raw, 4_096).unwrap();
        let response = respond(
            &request,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &active_gdpr_host(),
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), b"privacy input limit exceeded\n");
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn bidder_tcf_resource_exhaustion_is_terminal_without_host_vendor() {
        let raw = serde_json::to_vec(&serde_json::json!({
            "bidders": ["fake_bbb"],
            "gdpr": 1_i32,
            "gdpr_consent": over_limit_tcf(),
        }))
        .unwrap();
        let request = wire::parse_cookie_sync_body(&raw, 4_096).unwrap();
        let mut host = active_gdpr_host();
        host.gdpr.host_vendor_id = Some(0);
        let response = respond(
            &request,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &host,
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), b"privacy input limit exceeded\n");
    }

    #[test]
    fn excess_tcf_segments_below_byte_cap_are_terminal_for_host_and_candidate() {
        use auctioneer_privacy::tcf::{MAX_TOTAL_INPUT_LEN, TcfParseError, parse_tcf_v2};

        let valid =
            include_str!("../../../auctioneer-privacy/tests/data/tcf_v2/valid_full_consent.txt")
                .trim();
        parse_tcf_v2(valid).expect("valid consent before appending segments");
        let tc = format!("{valid}.IABA.IABA");
        assert!(tc.len() < 32 * 1024 && tc.len() < MAX_TOTAL_INPUT_LEN);
        assert!(matches!(
            parse_tcf_v2(&tc),
            Err(TcfParseError::TooManyTrailingSegments { actual: 3 })
        ));
        let raw = serde_json::to_vec(&json!({
            "bidders": ["fake_aaa"], "coopSync": false,
            "gdpr": 1_i32, "gdpr_consent": tc,
        }))
        .unwrap();
        let request = wire::parse_cookie_sync_body(&raw, 4_096).unwrap();
        for host_vendor in [32, 0] {
            let mut host = active_gdpr_host();
            host.gdpr.host_vendor_id = Some(host_vendor);
            let response = respond(
                &request,
                &HeaderMap::new(),
                &AuctioneerAppConfig::default(),
                &host,
                now(),
                &SeededRng::new(1),
            );
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{host_vendor}");
            assert_eq!(
                body(&response),
                b"privacy input limit exceeded\n",
                "{host_vendor}"
            );
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }

    #[test]
    fn cookie_sync_optout_precedes_account_and_uid_decode() {
        let mut host = configured_sync_host();
        host.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "optout".into(),
            value: "yes".into(),
        });
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("optout=yes; uids=invalid"));
        let raw = serde_json::to_vec(&json!({"account": "disabled"})).unwrap();
        let request = wire::parse_cookie_sync_body(&raw, 32).unwrap();
        let response = respond(
            &request,
            &headers,
            &AuctioneerAppConfig::default(),
            &host,
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(body(&response), b"User has opted out\n");
    }

    #[test]
    fn cookie_sync_request_false_overrides_account_cooperative_true() {
        let mut host = configured_sync_host();
        host.user_sync
            .as_mut()
            .unwrap()
            .cooperative
            .enabled_by_default = false;
        let app: AuctioneerAppConfig = toml::from_str(
            "schema_version = 1\n[accounts.publisher.cookie_sync]\ndefault_coop_sync = true\n",
        )
        .unwrap();
        let one = wire::parse_cookie_sync_body(
            &serde_json::to_vec(&json!({
                "account": "publisher",
                "bidders": ["fake_aaa"],
                "gdpr": 0_i32,
                "coopSync": false,
            }))
            .unwrap(),
            64,
        )
        .unwrap();
        let all = wire::parse_cookie_sync_body(
            &serde_json::to_vec(&json!({
                "account": "publisher", "bidders": ["fake_aaa"], "gdpr": 0_i32
            }))
            .unwrap(),
            64,
        )
        .unwrap();
        let rng = SeededRng::new(1);
        let direct_response = respond(&one, &HeaderMap::new(), &app, &host, now(), &rng);
        let inherited_response = respond(&all, &HeaderMap::new(), &app, &host, now(), &rng);
        let direct: serde_json::Value = serde_json::from_slice(body(&direct_response)).unwrap();
        let inherited: serde_json::Value =
            serde_json::from_slice(body(&inherited_response)).unwrap();
        assert_eq!(direct["bidder_status"].as_array().unwrap().len(), 1);
        assert_eq!(inherited["bidder_status"].as_array().unwrap().len(), 2);
        assert_eq!(direct["bidder_status"][0]["bidder"], "fake_aaa");
        assert_eq!(direct["bidder_status"][0]["usersync"]["type"], "redirect");
        let dual_mode = inherited["bidder_status"]
            .as_array()
            .unwrap()
            .iter()
            .find(|status| status["bidder"] == "fake_bbb")
            .unwrap();
        assert_eq!(dual_mode["usersync"]["type"], "iframe");
        assert_eq!(direct["status"], "no_cookie");
        assert!(
            direct["bidder_status"][0]["usersync"]["url"]
                .as_str()
                .unwrap()
                .starts_with("https://")
        );
    }

    #[test]
    fn cookie_sync_preflight_uses_allowlist_without_cookie_work() {
        let mut host = configured_sync_host();
        host.user_sync.as_mut().unwrap().allowed_origins = vec!["https://publisher.example".into()];
        let mut headers = HeaderMap::new();
        headers.insert(
            "origin",
            HeaderValue::from_static("https://publisher.example"),
        );
        headers.insert(
            "access-control-request-method",
            HeaderValue::from_static("POST"),
        );
        headers.insert(
            "access-control-request-headers",
            HeaderValue::from_static("content-type"),
        );
        headers.insert(COOKIE, HeaderValue::from_static("uids=invalid"));
        let response = preflight(&headers, &host);
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://publisher.example"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn cookie_sync_debug_reports_bounded_unknown_bidder_category() {
        let host = configured_sync_host();
        let request = wire::parse_cookie_sync_body(
            &serde_json::to_vec(&json!({
                "bidders": ["fake_not_a_bidder"], "gdpr": 0_i32, "debug": true
            }))
            .unwrap(),
            64,
        )
        .unwrap();
        let response = respond(
            &request,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &host,
            now(),
            &SeededRng::new(1),
        );
        let value: serde_json::Value = serde_json::from_slice(body(&response)).unwrap();
        assert_eq!(value["debug"][0]["bidder"], "fake_not_a_bidder");
        assert_eq!(value["debug"][0]["error"], "unknown bidder");
    }

    #[test]
    fn cookie_sync_url_expansion_refuses_whole_response_without_partial_candidates() {
        let raw = serde_json::to_vec(&json!({
            "bidders": ["fake_aaa", "fake_bbb"],
            "gdpr": 0_i32,
            "gdpr_consent": "<".repeat(10_000),
        }))
        .unwrap();
        let request = wire::parse_cookie_sync_body(&raw, 16).unwrap();
        let response = respond(
            &request,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &configured_sync_host(),
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), b"sync URL exceeds resource limit\n");
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn cookie_sync_failed_header_bound_redecorates_fixed_error_with_cors() {
        let mut host = configured_sync_host();
        host.user_sync.as_mut().unwrap().allowed_origins = vec!["https://publisher.example".into()];
        let mut headers = HeaderMap::new();
        headers.insert(
            "origin",
            HeaderValue::from_static("https://publisher.example"),
        );
        let mut oversized = Response::new(Body::empty());
        oversized.headers_mut().insert(
            "set-cookie",
            HeaderValue::from_str(&"x".repeat(0x4000)).unwrap(),
        );
        let response = decorate_response(oversized, &headers, &host);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://publisher.example"
        );
        assert_eq!(
            response.headers()["access-control-allow-credentials"],
            "true"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn cookie_sync_empty_consent_uses_parsed_or_defaulted_gdpr_message() {
        let app = AuctioneerAppConfig::default();
        let mut host = configured_sync_host();
        host.gdpr.default_value = "1".into();
        for (value, expected) in [
        (json!({"gdpr": 1_i32}), b"gdpr_consent is required if gdpr=1\n".as_slice()),
        (
            json!({}),
            b"gdpr_consent is required. gdpr is not specified and is assumed to be 1 by the server. set gdpr=0 to exempt this request\n".as_slice(),
        ),
    ] {
        let raw = serde_json::to_vec(&value).unwrap();
        let request = wire::parse_cookie_sync_body(&raw, 32).unwrap();
        let response = respond(
            &request,
            &HeaderMap::new(),
            &app,
            &host,
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), expected);
        assert_eq!(response.headers()["content-type"], "text/plain; charset=utf-8");
        assert!(!response.headers().contains_key("set-cookie"));
    }
    }

    #[test]
    fn cookie_sync_invalid_image_filter_uses_fixed_branch_diagnostic() {
        let request = wire::parse_cookie_sync_body(
            &serde_json::to_vec(&json!({
                "gdpr": 0_i32,
                "filterSettings": {"image": {"filter": "bad", "bidders": "*"}}
            }))
            .unwrap(),
            64,
        )
        .unwrap();
        let response = respond(
            &request,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &configured_sync_host(),
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
        body(&response),
        b"error parsing filtersettings.image: invalid filter value. must be either 'include' or 'exclude'\n"
    );

        let hostile = wire::parse_cookie_sync_body(
            &serde_json::to_vec(&json!({
                "gdpr": 0_i32,
                "filterSettings": {"image": {"filter": "secret-consent-token", "bidders": "*"}}
            }))
            .unwrap(),
            64,
        )
        .unwrap();
        let hostile_response = respond(
            &hostile,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &configured_sync_host(),
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(
        body(&hostile_response),
        b"error parsing filtersettings.image: invalid filter value. must be either 'include' or 'exclude'\n"
    );
        assert!(
            !body(&hostile_response)
                .windows(6)
                .any(|window| window == b"secret")
        );
    }

    #[test]
    fn cookie_sync_invalid_iframe_bidders_type_uses_pinned_branch_diagnostic() {
        let request = wire::parse_cookie_sync_body(
            &serde_json::to_vec(&json!({
                "gdpr": 0_i32,
                "filterSettings": {"iframe": {"filter": "include", "bidders": 42_i32}}
            }))
            .unwrap(),
            64,
        )
        .unwrap();
        let response = respond(
            &request,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &configured_sync_host(),
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
        body(&response),
        b"error parsing filtersettings.iframe: invalid bidders type. must either be a string '*' or a string array\n"
    );
    }

    #[test]
    fn cookie_sync_unknown_filter_key_is_still_rejected() {
        let request = wire::parse_cookie_sync_body(
            &serde_json::to_vec(&json!({
                "gdpr": 0_i32,
                "filterSettings": {"image": {"filter": "include", "bidders": "*", "extra": true}}
            }))
            .unwrap(),
            64,
        )
        .unwrap();
        let response = respond(
            &request,
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &configured_sync_host(),
            now(),
            &SeededRng::new(1),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(!response.headers().contains_key("set-cookie"));
    }
}

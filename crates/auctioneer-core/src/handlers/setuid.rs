//! `GET /setuid` handler.

use std::future::Future;

use crate::{
    bidder_registry::registry_handle,
    config::{AuctioneerAppConfig, resolve, runtime::resolve_runtime_config},
    handlers::{
        aux_egress::with_aux_egress_preserving_body,
        egress_deadline::EgressDeadline,
        sync_http,
        usersync_keys::{self, KeyringResult},
    },
    host_primitives::clock_handle,
    usersync::{
        catalog::{Candidate, Catalog},
        cookie::{CookieWriteOptions, HostCookie, MAX_SET_COOKIE_FIELD_BYTES, read_cookie},
        evict::{CookieWriteDecision, FamilyPriorities, prepare_cookie_transaction},
        limits::CookieLimits,
        wire::{ParsedQuery, parse_setuid_query},
    },
};
use auctioneer_bidder::{BidderRegistry, RegistryBuildError};
use auctioneer_config::{
    types::{AccountConfig, HostConfig},
    usersync::SyncResponseFormat,
};
use auctioneer_privacy::{
    Activity, Component, GdprSignalIn, SyncDecision, SyncEndpoint, SyncPrivacyError,
    SyncPrivacyInput, SyncerLookup, activity_allowed, gcl::Gcl, gvl::GvlStore,
    resolve_sync_privacy,
};
use auctioneer_runtime::body_caps::DEFAULT_CAP_BYTES;
use edgezero_core::{
    action,
    context::RequestContext,
    error::EdgeError,
    http::{HeaderMap, Response, StatusCode},
};
use time::OffsetDateTime;

#[action]
pub async fn setuid(ctx: RequestContext) -> Result<Response, EdgeError> {
    with_aux_egress_preserving_body(ctx, setuid_body).await
}

async fn setuid_body(ctx: RequestContext) -> Result<Response, EdgeError> {
    let deadline = EgressDeadline::from_boot_envelope(ctx.request_start());
    let app = match resolve_runtime_config(&ctx).await {
        Ok(app) => app,
        Err(error) => return error.into_response(),
    };
    let host = resolve::resolve_host(&app, auctioneer_config::host_default());
    let policy = sync_http::SyncBrowserPolicy::new(&host.user_sync);
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
    let now = clock_handle().now();
    let response = respond(
        ctx.uri().query(),
        ctx.headers(),
        &app,
        &host,
        now,
        usersync_keys::load(&ctx, deadline),
    )
    .await;
    if deadline.is_expired_at(ctx.monotonic_clock().now()) {
        return Ok(sync_http::cpu_overloaded_with_policy(
            &policy,
            ctx.headers(),
            sync_http::response_header_cap(),
        ));
    }
    Ok(policy
        .decorate_setuid(response, ctx.headers(), sync_http::response_header_cap())
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

struct ResponseContext<'a, Registry> {
    app: &'a AuctioneerAppConfig,
    host: &'a HostConfig,
    registry: Registry,
}

pub(crate) async fn respond(
    query: Option<&str>,
    headers: &HeaderMap,
    app: &AuctioneerAppConfig,
    host: &HostConfig,
    now: OffsetDateTime,
    key_read: impl Future<Output = KeyringResult>,
) -> Response {
    respond_with_registry(
        query,
        headers,
        ResponseContext {
            app,
            host,
            registry: registry_handle,
        },
        now,
        key_read,
    )
    .await
}

#[expect(
    clippy::too_many_lines,
    reason = "the ordered endpoint pipeline preserves error precedence"
)]
async fn respond_with_registry(
    query: Option<&str>,
    headers: &HeaderMap,
    context: ResponseContext<
        '_,
        impl FnOnce() -> Result<&'static BidderRegistry, RegistryBuildError>,
    >,
    now: OffsetDateTime,
    key_read: impl Future<Output = KeyringResult>,
) -> Response {
    let ResponseContext {
        app,
        host,
        registry: load_registry,
    } = context;
    let parsed = match parse_setuid_query(query) {
        Ok(parsed) => parsed,
        Err(error) => return sync_http::setuid_wire_error(error),
    };
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
        return sync_http::setuid_error(StatusCode::UNAUTHORIZED, "");
    }

    let Some(bidder) = parsed.first("bidder").filter(|name| !name.is_empty()) else {
        return bad("\"bidder\" query param is required");
    };
    let registry = match load_registry() {
        Ok(value) => value,
        Err(error) => {
            log::error!("{error}");
            return sync_http::setuid_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Bidder registry initialization failed",
            );
        }
    };
    let Ok(catalog) = Catalog::from_registry(host, app, registry) else {
        return bad("The bidder name provided is not supported by Prebid Server");
    };
    let Some(candidate) = catalog.lookup(bidder) else {
        return bad("The bidder name provided is not supported by Prebid Server");
    };
    let format = match parsed.first("f") {
        None => None,
        Some(raw) if raw.eq_ignore_ascii_case("i") => Some(SyncResponseFormat::Image),
        Some(raw) if raw.eq_ignore_ascii_case("b") => Some(SyncResponseFormat::Blank),
        Some(_) => return bad("\"f\" query param is invalid. must be \"b\" or \"i\""),
    };
    let account = resolve::resolve_selected_account(
        resolve::AccountSelection::from_requested(parsed.first("account")),
        app,
        auctioneer_config::account_default(),
    );
    if account.config.disabled {
        return bad("account is disabled, please reach out to the prebid server host");
    }
    if !candidate.configured {
        return bad("The bidder name provided is not supported by Prebid Server");
    }
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
    let mut cookie = match keys.read_cookie(headers, "uids", host_cookie, now, &limits) {
        Ok(read) => read.cookie,
        Err(error) => return sync_http::setuid_error(StatusCode::BAD_REQUEST, error.message()),
    };
    if !cookie.allow_syncs() {
        return sync_http::setuid_error(StatusCode::UNAUTHORIZED, "");
    }
    if let Some(refusal) = privacy_refusal(&parsed, &candidate, &account.config, host, now) {
        return refusal;
    }
    if cookie
        .set_uid(
            candidate.family,
            parsed.first("uid").unwrap_or(""),
            now,
            &limits,
        )
        .is_err()
    {
        return bad("cookie resource limit exceeded");
    }
    let groups = account.effective_priority_groups(host);
    let Ok(priorities) = FamilyPriorities::from_catalog(groups, &catalog) else {
        return bad("cookie resource limit exceeded");
    };
    let options = CookieWriteOptions {
        name: "uids",
        domain: host.host_cookie.domain.as_deref().unwrap_or(""),
        ttl_days: host.host_cookie.ttl_days,
        user_agent: headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok()),
        hard_cap_bytes: MAX_SET_COOKIE_FIELD_BYTES,
    };
    let Ok(write) = prepare_cookie_transaction(
        &cookie,
        candidate.family,
        &priorities,
        &options,
        now,
        &limits,
        host.host_cookie.max_cookie_size_bytes,
        &keys,
    ) else {
        return bad("cookie resource limit exceeded");
    };
    write_response(write, format, &candidate)
}

fn privacy_refusal(
    parsed: &ParsedQuery,
    candidate: &Candidate<'_>,
    account: &AccountConfig,
    host: &HostConfig,
    now: OffsetDateTime,
) -> Option<Response> {
    let mut privacy = match resolve_sync_privacy(SyncPrivacyInput {
        gdpr: GdprSignalIn::SetUidQuery(parsed.first("gdpr")),
        consent: parsed.first("gdpr_consent"),
        us_privacy: None,
        gpp: parsed.first("gpp"),
        gpp_sid: parsed.first("gpp_sid"),
        account,
        host,
        endpoint: SyncEndpoint::SetUid,
        gvl: GvlStore::baked(),
        gcl: Gcl::baked(),
        now,
    }) {
        Ok(privacy) => privacy,
        Err(error) => return Some(privacy_error(error)),
    };
    if !activity_allowed(
        &account.privacy.allowactivities,
        Activity::SyncUser,
        Component::Bidder(&candidate.normalized_name),
    ) {
        return Some(sync_http::setuid_error(
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            "",
        ));
    }
    let lookup = SyncerLookup {
        requested_name: &candidate.requested_name,
        normalized_name: &candidate.normalized_name,
        base_name: candidate.base_name,
        bidder: candidate.bidder,
        syncer: &candidate.bidder.user_sync,
        syncer_key: candidate.family,
    };
    match privacy.setuid_permission(&lookup) {
        SyncDecision::Allowed if candidate.enabled => None,
        SyncDecision::MalformedTcf => Some(bad("gdpr_consent was invalid. malformed TCF consent")),
        SyncDecision::ResourceLimit => Some(bad("privacy input limit exceeded")),
        SyncDecision::BlockedByActivity => Some(sync_http::setuid_error(
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            "",
        )),
        SyncDecision::BlockedByPrivacy => Some(sync_http::setuid_error(
            StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
            "The gdpr_consent string prevents cookies from being saved",
        )),
        SyncDecision::BlockedByDisabledUsersync
        | SyncDecision::Allowed
        | SyncDecision::BlockedByRegulationScope => Some(bad("The bidder syncer is disabled")),
    }
}

fn write_response(
    write: CookieWriteDecision,
    format: Option<SyncResponseFormat>,
    candidate: &Candidate<'_>,
) -> Response {
    match write {
        CookieWriteDecision::Write(prepared) => sync_http::setuid_success(
            format
                .or(candidate.bidder.user_sync.default_response)
                .or_else(|| {
                    candidate
                        .bidder
                        .user_sync
                        .iframe
                        .as_ref()
                        .map(|_| SyncResponseFormat::Blank)
                })
                .or_else(|| {
                    candidate
                        .bidder
                        .user_sync
                        .redirect
                        .as_ref()
                        .map(|_| SyncResponseFormat::Image)
                }),
            Some(prepared),
        )
        .unwrap_or_else(|_| bad("response limit exceeded")),
        CookieWriteDecision::WarnNoWrite => sync_http::setuid_warning(),
        CookieWriteDecision::RejectNoWrite => bad(""),
    }
}

fn bad(message: &'static str) -> Response {
    sync_http::setuid_error(StatusCode::BAD_REQUEST, message)
}

fn privacy_error(error: SyncPrivacyError) -> Response {
    bad(match error {
        SyncPrivacyError::MalformedGdpr => "the gdpr query param must be either 0 or 1",
        SyncPrivacyError::MalformedGppSid => {
            "invalid gpp_sid encoding, must be a csv list of integers"
        }
        SyncPrivacyError::MalformedGpp => "error parsing GPP header, header must have type=3",
        SyncPrivacyError::MissingRequiredConsent => {
            "GDPR consent is required when gdpr signal equals 1"
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
    use crate::usersync::signed::CookieKeyring;
    use auctioneer_config::usersync::OptoutCookie;
    use edgezero_core::http::{HeaderValue, header::COOKIE};
    use futures::executor::block_on;
    use serde_json::json;
    use std::cell::Cell;
    use std::future::ready;
    use url::form_urlencoded;

    pub(in crate::handlers) async fn wire_response(
        query: Option<&str>,
        headers: &HeaderMap,
        app: &AuctioneerAppConfig,
        host: &HostConfig,
        now: OffsetDateTime,
        key_read: impl Future<Output = KeyringResult>,
    ) -> Response {
        super::respond_with_registry(
            query,
            headers,
            ResponseContext {
                app,
                host,
                registry: || Ok(bidder_registry::registry_handle()),
            },
            now,
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
        query: Option<&str>,
        headers: &HeaderMap,
        app: &AuctioneerAppConfig,
        host: &HostConfig,
        now: OffsetDateTime,
    ) -> Response {
        block_on(super::respond_with_registry(
            query,
            headers,
            ResponseContext {
                app,
                host,
                registry: || Ok(registry()),
            },
            now,
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
    fn registry_failure_is_not_an_unsupported_bidder() {
        let keys_polled = Cell::new(false);
        let response = block_on(super::respond_with_registry(
            Some("bidder=fake_bidder"),
            &HeaderMap::new(),
            ResponseContext {
                app: &AuctioneerAppConfig::default(),
                host: &HostConfig::default(),
                registry: || {
                    Err(RegistryBuildError::MissingBinding {
                        bidder: "fake_missing",
                    })
                },
            },
            now(),
            async {
                keys_polled.set(true);
                Ok(keys())
            },
        ));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body(&response), b"Bidder registry initialization failed");
        assert!(!keys_polled.get());
    }

    #[test]
    fn catalog_loading_follows_query_optout_and_bidder_but_precedes_format_and_account() {
        let app: AuctioneerAppConfig =
            toml::from_str("[accounts.disabled]\ndisabled = true\n").unwrap();
        let mut host = HostConfig::default();
        host.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "optout".into(),
            value: "yes".into(),
        });
        host.aliases
            .insert("fake_alias_a".into(), "fake_missing_base".into());
        for (query, opted_out, status, expected, catalog_expected) in [
            (
                Some("uid=%"),
                true,
                StatusCode::BAD_REQUEST,
                "invalid query encoding",
                false,
            ),
            (None, true, StatusCode::UNAUTHORIZED, "", false),
            (
                None,
                false,
                StatusCode::BAD_REQUEST,
                "\"bidder\" query param is required",
                false,
            ),
            (
                Some("bidder=fake_aaa&f=bad&account=disabled"),
                false,
                StatusCode::BAD_REQUEST,
                "The bidder name provided is not supported by Prebid Server",
                true,
            ),
            (
                Some("bidder=fake_bbb&f=b&account=disabled"),
                false,
                StatusCode::BAD_REQUEST,
                "The bidder name provided is not supported by Prebid Server",
                true,
            ),
        ] {
            let mut headers = HeaderMap::new();
            if opted_out {
                headers.insert(COOKIE, HeaderValue::from_static("optout=yes"));
            }
            let catalog_loaded = Cell::new(false);
            let keys_polled = Cell::new(false);
            let response = block_on(super::respond_with_registry(
                query,
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
                async {
                    keys_polled.set(true);
                    Ok(keys())
                },
            ));
            assert_eq!(response.status(), status, "{query:?}");
            assert_eq!(body(&response), expected.as_bytes(), "{query:?}");
            assert_eq!(catalog_loaded.get(), catalog_expected, "{query:?}");
            assert!(!keys_polled.get(), "{query:?}");
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }

    #[test]
    fn synthetic_syncers_preserve_alias_families_and_distinct_default_formats() {
        let mut host = HostConfig::default();
        host.aliases
            .insert("fake_alias_a".into(), "fake_aaa".into());
        for (bidder, format, content_type, byte_count, family) in [
            ("fake_ALIAS_A", "", "image/png", 86, "fam_a"),
            ("fake_BbB", "", "text/html", 0, "fam_b"),
            ("fake_alias_a", "&f=b", "text/html", 0, "fam_a"),
        ] {
            let query = format!("bidder={bidder}&uid=synthetic-user&gdpr=0{format}");
            let response = respond(
                Some(&query),
                &HeaderMap::new(),
                &AuctioneerAppConfig::default(),
                &host,
                now(),
            );
            assert_eq!(response.status(), StatusCode::OK, "{bidder}");
            assert_eq!(response.headers()["content-type"], content_type);
            assert_eq!(body(&response).len(), byte_count);
            let set_cookie = response.headers()["set-cookie"].to_str().unwrap();
            let mut headers = HeaderMap::new();
            headers.insert(
                COOKIE,
                HeaderValue::from_str(set_cookie.split(';').next().unwrap()).unwrap(),
            );
            let read = keys()
                .read_cookie(
                    &headers,
                    "uids",
                    HostCookie::default(),
                    now(),
                    &CookieLimits::default(),
                )
                .unwrap();
            assert_eq!(read.cookie.uid(family), Some("synthetic-user"));
            assert_eq!(read.cookie.uids().len(), 1);
        }
    }

    #[test]
    fn privacy_limit_diagnostics_are_distinct_from_malformed_signals() {
        for error in [
            SyncPrivacyError::GppSidLimitExceeded,
            SyncPrivacyError::GppLimitExceeded,
        ] {
            let response = privacy_error(error);
            assert_eq!(body(&response), b"privacy input limit exceeded");
        }
        assert_eq!(
            body(&privacy_error(SyncPrivacyError::MalformedGppSid)),
            b"invalid gpp_sid encoding, must be a csv list of integers"
        );
        assert_eq!(
            body(&privacy_error(SyncPrivacyError::MalformedGpp)),
            b"error parsing GPP header, header must have type=3"
        );
    }

    fn over_limit_tcf() -> &'static str {
        include_str!(
            "../../../auctioneer-privacy/tests/data/tcf_v2/pub_restrictions_exceeds_2048_rejected.txt"
        )
        .trim()
    }

    fn active_gdpr_host() -> HostConfig {
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.host_vendor_id = Some(32);
        host
    }

    #[test]
    fn setuid_tcf_resource_exhaustion_is_terminal_bad_request() {
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("bidder", "fake_aaa")
            .append_pair("uid", "partner-123")
            .append_pair("gdpr", "1")
            .append_pair("gdpr_consent", over_limit_tcf())
            .finish();
        let response = respond(
            Some(&query),
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &active_gdpr_host(),
            now(),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), b"privacy input limit exceeded");
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn excess_tcf_segments_below_byte_cap_are_terminal_bad_request() {
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
        let query = form_urlencoded::Serializer::new(String::new())
            .append_pair("bidder", "fake_bbb")
            .append_pair("uid", "partner-123")
            .append_pair("gdpr", "1")
            .append_pair("gdpr_consent", &tc)
            .finish();
        let response = respond(
            Some(&query),
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &active_gdpr_host(),
            now(),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), b"privacy input limit exceeded");
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn setuid_optout_precedes_missing_bidder() {
        let mut host = HostConfig::default();
        host.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "optout".into(),
            value: "yes".into(),
        });
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("optout=yes"));
        let response = respond(
            None,
            &headers,
            &AuctioneerAppConfig::default(),
            &host,
            now(),
        );
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(body(&response).is_empty());
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn setuid_cookie_over_hard_cap_refuses_without_partial_write() {
        let query = format!("bidder=fake_aaa&uid={}&f=b&gdpr=0", "a".repeat(12_400));
        let response = respond(
            Some(&query),
            &HeaderMap::new(),
            &AuctioneerAppConfig::default(),
            &HostConfig::default(),
            now(),
        );

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&response), b"cookie resource limit exceeded");
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn setuid_query_error_precedes_host_optout_and_format_precedes_account() {
        let mut host = HostConfig::default();
        host.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "optout".into(),
            value: "yes".into(),
        });
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static("optout=yes"));
        let query_error = respond(
            Some("uid=%"),
            &headers,
            &AuctioneerAppConfig::default(),
            &host,
            now(),
        );
        assert_eq!(query_error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(&query_error), b"invalid query encoding");

        let app: AuctioneerAppConfig =
            toml::from_str("[accounts.publisher]\ndisabled = true\n").unwrap();
        let format_error = respond(
            Some("bidder=fake_bbb&f=bad&account=publisher"),
            &HeaderMap::new(),
            &app,
            &HostConfig::default(),
            now(),
        );
        assert_eq!(format_error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body(&format_error),
            b"\"f\" query param is invalid. must be \"b\" or \"i\""
        );
    }

    #[test]
    fn setuid_known_unsyncable_bidder_rejects_before_uid_decode_and_never_writes() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!("auctioneer_uid={}", "x".repeat(25_000))).unwrap(),
        );
        let response = respond(
            Some("bidder=fake_pp&uid=should-not-write&gdpr=0"),
            &headers,
            &AuctioneerAppConfig::default(),
            &HostConfig::default(),
            now(),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body(&response),
            b"The bidder name provided is not supported by Prebid Server"
        );
        assert!(!response.headers().contains_key("set-cookie"));
    }

    #[test]
    fn setuid_format_and_account_errors_precede_unsyncable_syncer_resolution() {
        let app: AuctioneerAppConfig =
            toml::from_str("[accounts.disabled]\ndisabled = true\n").unwrap();
        let format = respond(
            Some("bidder=fake_pp&f=invalid&account=disabled"),
            &HeaderMap::new(),
            &app,
            &HostConfig::default(),
            now(),
        );
        assert_eq!(
            body(&format),
            b"\"f\" query param is invalid. must be \"b\" or \"i\""
        );
        let account = respond(
            Some("bidder=fake_pp&f=b&account=disabled"),
            &HeaderMap::new(),
            &app,
            &HostConfig::default(),
            now(),
        );
        assert_eq!(
            body(&account),
            b"account is disabled, please reach out to the prebid server host"
        );
    }

    #[test]
    fn setuid_gdpr_error_redacts_input_when_sid_does_not_override() {
        let host = HostConfig::default();
        let app = AuctioneerAppConfig::default();
        let invalid = respond(
            Some("bidder=fake_aaa&uid=x&gdpr=2&f=b"),
            &HeaderMap::new(),
            &app,
            &host,
            now(),
        );
        assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body(&invalid),
            b"the gdpr query param must be either 0 or 1"
        );
        assert!(!invalid.headers().contains_key("content-type"));
        assert!(!invalid.headers().contains_key("set-cookie"));

        let hostile = respond(
            Some("bidder=fake_bbb&uid=x&gdpr=secret-consent-token&f=b"),
            &HeaderMap::new(),
            &app,
            &host,
            now(),
        );
        assert_eq!(hostile.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body(&hostile),
            b"the gdpr query param must be either 0 or 1"
        );

        let sid_error = respond(
            Some("bidder=fake_aaa&uid=x&gdpr=2&gpp_sid=bad&f=b"),
            &HeaderMap::new(),
            &app,
            &host,
            now(),
        );
        assert_eq!(
            body(&sid_error),
            b"invalid gpp_sid encoding, must be a csv list of integers"
        );
        let overridden = respond(
            Some("bidder=fake_bbb&uid=x&gdpr=2&gpp_sid=-1&f=b"),
            &HeaderMap::new(),
            &app,
            &host,
            now(),
        );
        assert_eq!(overridden.status(), StatusCode::OK);
        assert!(overridden.headers().contains_key("set-cookie"));
    }

    #[test]
    fn setuid_activity_denial_precedes_deferred_malformed_tcf_permission_error() {
        let mut host = HostConfig::default();
        host.gdpr.enabled = true;
        host.gdpr.tcf2.enabled = true;
        host.gdpr.host_vendor_id = Some(32);
        let app: AuctioneerAppConfig =
            toml::from_str("[accounts.denied.privacy.allowactivities.syncUser]\ndefault = false\n")
                .unwrap();
        let query = "bidder=fake_aaa&uid=x&gdpr=1&gdpr_consent=malformed-tcf&f=b";
        let denied = respond(
            Some(&format!("{query}&account=denied")),
            &HeaderMap::new(),
            &app,
            &host,
            now(),
        );
        assert_eq!(denied.status(), StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS);
        assert!(body(&denied).is_empty());
        assert!(!denied.headers().contains_key("set-cookie"));

        let malformed = respond(Some(query), &HeaderMap::new(), &app, &host, now());
        assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
        assert!(body(&malformed).starts_with(b"gdpr_consent was invalid. "));
        assert!(!malformed.headers().contains_key("set-cookie"));
    }
}

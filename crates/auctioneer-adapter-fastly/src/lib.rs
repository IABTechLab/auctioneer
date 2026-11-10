//! Fastly Compute@Edge adapter for auctioneer.
//!
//! The crate was binary-only, and a `[[bin]]` target exports nothing:
//! the timer, the decode cooperator and the outbound profile were
//! unreachable from any integration test. They live in this library
//! now, and `src/main.rs` is the thin entrypoint that calls [`run`].
//!
//! # What is host-visible and what is not
//!
//! [`timer`], [`cooperator`], [`delay`] and [`limits`] compile on every
//! target: the profile is plain data and the two primitives are logic
//! over a [`delay::DelaySource`], so `tests/fanout.rs` exercises all of
//! it on the host. The Fastly SDK dispatch handoff is wasm-gated. Process
//! tests in `tests/boot_logging.rs` exercise the real guest in Viceroy.

pub mod cooperator;
pub mod delay;
pub mod envelope;
pub mod limits;
pub mod timer;

#[cfg(test)]
mod config_validation_tests {
    #[test]
    fn product_defaults_validate_against_the_target_published_catalog() {
        use auctioneer_core::config::{AuctioneerAppConfig, validate_effective_defaults};

        validate_effective_defaults(&AuctioneerAppConfig::default())
            .expect("the compiled target publication join accepts product defaults");
    }
}

#[cfg(target_arch = "wasm32")]
use auctioneer_core::App as AuctioneerApp;
#[cfg(target_arch = "wasm32")]
use auctioneer_core::host_primitives::{
    DelegateToDevStrategy, DevRng, SystemClock, bootstrap_adapter_with_envelope,
    install_stderr_boot_logger_with_level,
};
#[cfg(target_arch = "wasm32")]
use edgezero_adapter_fastly::{FastlyLogging, init_logger, run_app};
#[cfg(all(test, target_arch = "wasm32"))]
use edgezero_core::app::App as EdgeApp;
#[cfg(target_arch = "wasm32")]
use edgezero_core::app::Hooks;
#[cfg(target_arch = "wasm32")]
use fastly::Error;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

/// Boots the adapter and hands the request lifecycle to `EdgeZero`.
///
/// App assembly uses the generated `app!` hooks and Fastly platform metadata.
/// `EdgeZero`'s manifest-aware dispatch helper owns request conversion,
/// registries, routing and response transmission. Logging is baked into
/// the generated hooks; SDK registries bind logical store aliases to
/// physical resources at deployment.
///
/// # Errors
/// Propagates invalid boot limits, conflicting health counter-scope installation,
/// or request/response dispatch errors. The SDK delivers application assembly
/// failures as typed HTTP error responses through its owned egress path.
/// Health policy and secret faults affect the shared health handler only.
#[cfg(target_arch = "wasm32")]
pub fn run() -> Result<(), Error> {
    run_with_hooks::<AuctioneerApp>()
}

#[cfg(target_arch = "wasm32")]
fn run_with_hooks<A: Hooks>() -> Result<(), Error> {
    // THE LOG SINK, FIRST. Before the envelope, because the envelope's
    // over-ceiling diagnostic is a warning and a warning with no sink is
    // silence; and before anything else, because `log` admits one sink
    // per process and whoever installs first keeps it.
    //
    // Auctioneer owns the backend. The SDK lifecycle helper below does not
    // install another logger or reconfigure its filters.
    install_boot_log::<A>();
    // Idempotent — Fastly instantiates a fresh isolate per request, so
    // this is effectively "install once per request" and the
    // `get_or_init` fast path keeps it cheap after the first call.
    //
    // The timer is real in the sense that it is the only one this
    // platform can offer: Compute gives the guest no awaitable sleep, so
    // `FastlyTimer` over `ImmediateDelaySource` resolves at once and the
    // wave gate's wait is declared `BestEffort` rather than pretending
    // otherwise. The cooperator is a genuine yield: the Compute clock is
    // not frozen across CPU work, so its job is to let sibling slots be
    // observed during a long decode.
    //
    // The envelope is validated FIRST, and this row is OVER ITS
    // CEILING: `install_boot_envelope` warns and installs rather than
    // refusing, so the guest serves. The warning names the required
    // total, the ceiling, the overshoot and the terms that dominate it,
    // which is the entire safety margin on that path — an operator who
    // reads only that line must still be able to act on it.
    //
    // Every OTHER validation failure is still terminal and installs no
    // gate and no host primitive. A cap below a measured floor above
    // all: that under-reserves memory the code needs on every request,
    // so no amount of traffic tuning makes booting past it safe.
    // Keep the limit values distinct from the adapter's `limits` module.
    let profile = envelope::resource_profile();
    bootstrap_adapter_with_envelope(
        profile.boot_envelope(),
        || Arc::new(SystemClock::new()),
        || Arc::new(DevRng::new()),
        || Arc::new(timer::FastlyTimer::new(delay::ImmediateDelaySource)),
        || Arc::new(cooperator::FastlyDecodeCooperator),
        || Arc::new(DelegateToDevStrategy),
    )
    .map_err(|e| Error::msg(format!("auctioneer boot envelope rejected: {e}")))?;
    install_health()?;
    // The SDK selects FASTLY_PLATFORM, matching this profile's dispatch
    // metadata, and retains assembly-error and terminal-delivery ownership.
    run_app::<A>()
}

/// Install this process's `log` sink from the baked Fastly hook settings.
///
/// # Which sink
///
/// `Hooks::logging_for("fastly")` decides. When it names a Fastly log
/// endpoint, that endpoint is the sink, installed through `EdgeZero`'s
/// own `init_logger` so the format and level handling are upstream's
/// rather than a second implementation of them. If no endpoint is baked
/// in, or the named endpoint is unavailable or rejected, the sink is
/// [`auctioneer_core::host_primitives::StderrBootLog`] over the stderr Compute
/// captures. Reserved names such as `stdout` cannot be Fastly endpoints.
///
/// `EdgeZero`'s backend separates its boot target from ordinary logging.
/// Both filters are established before validation; no temporary widening
/// or post-bootstrap restoration is needed.
///
/// # On a lost race
///
/// Best-effort, like every other install on this path. A sink already
/// present belongs to whoever got there first and is left alone: the
/// warning needs SOME sink, not a particular one, and failing boot over
/// a logger would trade a lost diagnostic for a dead service.
#[cfg(target_arch = "wasm32")]
fn install_boot_log<A: Hooks>() {
    let logging = FastlyLogging::from(A::logging_for("fastly"));
    let installed = match logging.endpoint.as_deref() {
        Some(endpoint) => init_logger(endpoint, logging.level, logging.echo_stdout).is_ok(),
        None => install_stderr_boot_logger_with_level(logging.level),
    };
    if !installed {
        // A sink was already installed, or the endpoint was rejected.
        // Fall back so the envelope warning still has somewhere to go;
        // this is a no-op when something else already owns the sink.
        let _fallback = install_stderr_boot_logger_with_level(logging.level);
    }
}

/// Install the request's immutable health observation scope.
#[cfg(target_arch = "wasm32")]
fn install_health() -> Result<(), Error> {
    auctioneer_core::host_primitives::install_health_counter_scope("request")
        .map_err(|err| Error::msg(format!("health counter scope install conflicted: {err}")))
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod health_setup_tests {
    //! Real shared health routes through SDK registry assembly and terminal egress.

    use super::{
        AuctioneerApp, cooperator::FastlyDecodeCooperator, delay::ImmediateDelaySource,
        timer::FastlyTimer,
    };
    use auctioneer_core::host_primitives::{
        DelegateToDevStrategy, DevRng, SystemClock, TestClockHandle,
        bootstrap_adapter_with_envelope, counter_scope_handle,
    };
    use auctioneer_runtime::FakeClock;
    use edgezero_core::http::StatusCode;
    use std::sync::Arc;
    use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

    const TOKEN: &str = "auctioneer-development-only-health-test-token";

    fn verify_health(expected: StatusCode, bearer: Option<&str>) {
        super::install_boot_log::<AuctioneerApp>();
        let profile = super::envelope::resource_profile();
        bootstrap_adapter_with_envelope(
            profile.boot_envelope(),
            || Arc::new(SystemClock::new()),
            || Arc::new(DevRng::new()),
            || Arc::new(FastlyTimer::new(ImmediateDelaySource)),
            || Arc::new(FastlyDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("production envelope boots");
        super::install_health().expect("scope needs no provider policy read");
        assert_eq!(counter_scope_handle(), "request");
        let generated =
            OffsetDateTime::parse(auctioneer_config_data::gvl_archive_generated_at(), &Rfc3339)
                .unwrap();
        let expires =
            OffsetDateTime::from_unix_timestamp(auctioneer_config_data::gcl_expires_at_unix_secs())
                .unwrap();
        let now = generated
            .saturating_add(Duration::days(1))
            .min(expires.saturating_sub(Duration::hours(1)));
        let result = edgezero_adapter_fastly::run_app_with_hooks::<AuctioneerApp, _, _, _>(
            |request, extensions| {
                request.set_url("https://auctioneer.example.test/_auctioneer/health/gvl");
                request.set_method("GET");
                if let Some(presented) = bearer {
                    request.set_header("authorization", format!("Bearer {presented}"));
                }
                extensions.insert(TestClockHandle(Arc::new(FakeClock::new(now))));
            },
            |response| {
                assert_eq!(response.status(), expected);
                if expected == StatusCode::UNAUTHORIZED {
                    assert_eq!(response.headers()["www-authenticate"], "Bearer");
                } else {
                    let payload: serde_json::Value = serde_json::from_slice(
                        response.body().as_bytes().expect("buffered health payload"),
                    )
                    .unwrap();
                    assert_eq!(payload["counter_scope"], "request");
                    assert_eq!(payload["status"], "ok");
                }
                true
            },
        )
        .expect("SDK health dispatch and terminal delivery complete");
        assert_eq!(result, Some(true), "the SDK finalize hook must execute");
    }

    #[test]
    #[ignore = "requires token policy and SDK-backed stores supplied by manifest_health_stores"]
    fn runtime_token_policy_uses_sdk_bindings() {
        verify_health(StatusCode::OK, Some(TOKEN));
    }

    #[test]
    #[ignore = "requires explicit anonymous policy supplied by manifest_health_stores"]
    fn runtime_anonymous_policy_is_explicit() {
        verify_health(StatusCode::OK, None);
    }

    #[test]
    #[ignore = "requires token policy supplied by manifest_health_stores"]
    fn runtime_token_policy_denies_wrong_bearer() {
        verify_health(StatusCode::UNAUTHORIZED, Some("wrong-development-token"));
    }

    #[test]
    #[ignore = "requires token policy supplied by manifest_health_stores"]
    fn runtime_token_policy_denies_absent_bearer() {
        verify_health(StatusCode::UNAUTHORIZED, None);
    }

    #[test]
    #[ignore = "requires absent/invalid config or secret fixtures supplied by manifest_health_stores"]
    fn runtime_faults_close_only_health() {
        verify_health(StatusCode::UNAUTHORIZED, Some(TOKEN));
    }
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod logging_tests {
    //! Test-only hooks select fixed levels; Viceroy fixtures cannot override baked logging.

    use super::{AuctioneerApp, EdgeApp, Hooks};
    use edgezero_core::app::StoresMetadata;
    use edgezero_core::error::EdgeError;
    use edgezero_core::manifest::{BakedManifest, LogLevel, ResolvedLoggingConfig};
    use edgezero_core::router::RouterService;

    struct LoggingHooks<const LEVEL: u8, const SINK: u8>;

    impl<const LEVEL: u8, const SINK: u8> Hooks for LoggingHooks<LEVEL, SINK> {
        fn configure(app: &mut EdgeApp) -> Result<(), EdgeError> {
            <AuctioneerApp as Hooks>::configure(app)
        }

        fn logging_for(adapter: &str) -> ResolvedLoggingConfig {
            assert_eq!(adapter, "fastly");
            let mut logging = <AuctioneerApp as Hooks>::logging_for(adapter);
            logging.level = match LEVEL {
                0 => LogLevel::Off,
                1 => LogLevel::Error,
                2 => LogLevel::Debug,
                _ => panic!("unknown test logging level"),
            };
            logging.endpoint = match SINK {
                0 => Some("auctioneer-test".to_owned()),
                1 => None,
                // A named but rejected endpoint must also fall back to stderr.
                2 => Some("stdout".to_owned()),
                _ => panic!("unknown test logging sink"),
            };
            logging.echo_stdout = Some(false);
            logging
        }

        fn manifest() -> BakedManifest {
            <AuctioneerApp as Hooks>::manifest()
        }

        fn manifest_json() -> Option<&'static str> {
            <AuctioneerApp as Hooks>::manifest_json()
        }

        fn name() -> &'static str {
            <AuctioneerApp as Hooks>::name()
        }

        fn owns_logging() -> bool {
            <AuctioneerApp as Hooks>::owns_logging()
        }

        fn routes() -> Result<RouterService, EdgeError> {
            <AuctioneerApp as Hooks>::routes()
        }

        fn stores() -> StoresMetadata {
            <AuctioneerApp as Hooks>::stores()
        }
    }

    fn boot_warning<A: Hooks>() {
        super::install_boot_log::<A>();
        let configured: log::LevelFilter = A::logging_for("fastly").level.into();
        assert_eq!(
            log::max_level(),
            configured.max(edgezero_core::BOOT_LOG_LEVEL)
        );
        assert!(log::log_enabled!(target: edgezero_core::BOOT_LOG_TARGET, log::Level::Warn));
        assert!(!log::log_enabled!(target: edgezero_core::BOOT_LOG_TARGET, log::Level::Info));
        log::warn!(target: edgezero_core::BOOT_LOG_TARGET, "mandatory boot warning reached endpoint");
    }

    fn dispatch_filter<A: Hooks>() {
        super::run_with_hooks::<A>().expect("the shared boot path completes SDK response dispatch");
        let configured: log::LevelFilter = A::logging_for("fastly").level.into();
        assert_eq!(
            log::max_level(),
            configured.max(edgezero_core::BOOT_LOG_LEVEL)
        );
        assert!(log::log_enabled!(target: edgezero_core::BOOT_LOG_TARGET, log::Level::Warn));
        assert!(!log::log_enabled!(target: edgezero_core::BOOT_LOG_TARGET, log::Level::Info));
        for level in [
            log::Level::Error,
            log::Level::Warn,
            log::Level::Info,
            log::Level::Debug,
            log::Level::Trace,
        ] {
            assert_eq!(
                log::log_enabled!(target: "auctioneer::logging_test", level),
                level <= configured
            );
            log::log!(target: "auctioneer::logging_test", level, "ordinary {level}");
        }
    }

    macro_rules! logging_cases {
        ($level:literal, $boot:ident, $dispatch:ident, $stderr:ident, $rejected:ident) => {
            #[test]
            #[ignore = "requires a fresh Viceroy process supplied by tests/boot_logging.rs"]
            fn $boot() {
                boot_warning::<LoggingHooks<$level, 0>>();
            }

            #[test]
            #[ignore = "requires a fresh Viceroy process supplied by tests/boot_logging.rs"]
            fn $dispatch() {
                dispatch_filter::<LoggingHooks<$level, 0>>();
            }

            #[test]
            #[ignore = "requires a fresh Viceroy process supplied by tests/boot_logging.rs"]
            fn $stderr() {
                dispatch_filter::<LoggingHooks<$level, 1>>();
            }

            #[test]
            #[ignore = "requires a fresh Viceroy process supplied by tests/boot_logging.rs"]
            fn $rejected() {
                dispatch_filter::<LoggingHooks<$level, 2>>();
            }
        };
    }

    logging_cases!(
        1,
        endpoint_boot_error,
        endpoint_dispatch_error,
        stderr_dispatch_error,
        rejected_endpoint_dispatch_error
    );
    logging_cases!(
        0,
        endpoint_boot_off,
        endpoint_dispatch_off,
        stderr_dispatch_off,
        rejected_endpoint_dispatch_off
    );
    logging_cases!(
        2,
        endpoint_boot_debug,
        endpoint_dispatch_debug,
        stderr_dispatch_debug,
        rejected_endpoint_dispatch_debug
    );

    #[test]
    #[ignore = "requires the no-runtime-env fixture supplied by tests/boot_logging.rs"]
    fn baked_logging_dispatches_without_runtime_environment() {
        super::run().expect("the production entrypoint needs no runtime environment store");
        let logging = <AuctioneerApp as Hooks>::logging_for("fastly");
        assert_eq!(logging.endpoint.as_deref(), Some("auctioneer_log"));
        assert_eq!(logging.level, LogLevel::Info);
        assert_eq!(log::max_level(), log::LevelFilter::Info);
        log::info!(target: "auctioneer::logging_test", "production baked logging reached sink");
    }

    #[test]
    #[ignore = "requires the conflicting legacy settings fixture supplied by tests/boot_logging.rs"]
    fn legacy_runtime_logging_settings_cannot_override_baked_hooks() {
        baked_logging_dispatches_without_runtime_environment();
    }
}

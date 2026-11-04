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
//! it on the host. The `fastly` SDK calls — the store reads in
//! `install_health` and the dispatch handoff are wasm-gated. The process
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
    DelegateToDevStrategy, DevRng, HealthAccessPolicy, SystemClock,
    bootstrap_adapter_with_envelope, install_stderr_boot_logger_with_level,
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
/// the generated hooks; health and dispatch open the same logical store
/// aliases, bound to physical resources at deployment.
///
/// # Errors
/// Propagates invalid boot limits, conflicting health-policy installation,
/// or request/response dispatch errors. The SDK delivers application assembly
/// failures as typed HTTP error responses through its owned egress path.
/// Missing health stores install a closed policy rather than stopping routing.
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

/// Install the health-endpoint access policy, falling back to
/// [`HealthAccessPolicy::Closed`] when the stores cannot be read.
///
/// A store fault must NOT stop routing. `/status` exists to report
/// that this deployment is unhealthy, and propagating the fault out of
/// boot took the router down with it — so the one endpoint that could
/// have named the problem never answered, and an operator saw a dead
/// service rather than a degraded one.
///
/// Health authorization stays CLOSED in that case rather than opening:
/// an unreadable store is not permission to serve the GVL endpoint
/// unauthenticated. `Closed` answers 401, which is attributable.
///
/// The fault is logged at ERROR so Compute stderr or the Fastly log
/// endpoint carries it, and `/status` reports the config verdict
/// through its own resolution path.
#[cfg(target_arch = "wasm32")]
fn install_health() -> Result<(), Error> {
    use auctioneer_core::host_primitives::install_health_primitives;

    match resolve_health_policy() {
        Ok(policy) => install_health_primitives(policy, "request")
            .map_err(|err| Error::msg(format!("health policy install conflicted: {err}"))),
        Err(err) => {
            log::error!(
                target: "auctioneer::health",
                "health policy unresolved ({err}); serving with health CLOSED so \
                 /status can report the fault"
            );
            install_health_primitives(HealthAccessPolicy::Closed, "request").map_err(|install| {
                Error::msg(format!("health policy install conflicted: {install}"))
            })
        }
    }
}

/// Read the policy out of the Fastly stores.
///
/// The bearer token is a credential → `SecretStore` (encrypted at
/// rest); the non-secret `allow_unauthenticated_health` flag stays in
/// the `ConfigStore`. EVERY Fastly call is the fallible variant —
/// the infallible store-open and plaintext APIs panic on failure —
/// so a missing or broken store returns an error here instead of
/// panicking, and the caller decides what to serve.
#[cfg(target_arch = "wasm32")]
fn resolve_health_policy() -> Result<HealthAccessPolicy, Error> {
    use auctioneer_core::host_primitives::{BearerToken, decide_health_policy, read_bool_flag};
    use fastly::config_store::ConfigStore;
    use fastly::secret_store::SecretStore;
    // The store NAMES come from the manifest's `[stores.*]`
    // declarations, which `app!` bakes into `Hooks::stores()` — not from
    // a literal here. A literal is how boot and provisioning came to
    // disagree: `edgezero.toml` declares config store `auctioneer_runtime`
    // and secret store `default`, so a hardcoded "auctioneer" opened stores
    // nothing creates, and health installation failed before routing
    // could answer even `/status`.
    //
    // Fail closed when a declaration is absent rather than falling back
    // to a guess: a missing `[stores.*]` entry means the deployment was
    // never told to provision one, and inventing a name would put the
    // same disagreement back with a different literal.
    let stores = <AuctioneerApp as Hooks>::stores();
    let config_id = stores
        .config
        .ok_or_else(|| Error::msg("edgezero.toml declares no [stores.config]"))?
        .default;
    let secrets_id = stores
        .secrets
        .ok_or_else(|| Error::msg("edgezero.toml declares no [stores.secrets]"))?
        .default;
    // Fastly resource links bind physical stores under these logical
    // aliases at deployment. The SDK registries open the same aliases;
    // runtime environment remapping would make health read a different store.
    let raw_flag = ConfigStore::try_open(config_id)?.try_get("allow_unauthenticated_health")?;
    let allow_unauth = read_bool_flag("allow_unauthenticated_health", raw_flag.as_deref())?;
    let secret = SecretStore::open(secrets_id)?.try_get("health_gvl_token")?;
    let token_bytes = match secret {
        Some(s) => Some(s.try_plaintext()?),
        None => None,
    };
    // An empty secret reads as "no token configured".
    let token = token_bytes.and_then(BearerToken::new);
    let policy = decide_health_policy(token, allow_unauth)
        .map_err(|err| Error::msg(format!("health policy rejected: {err}")))?;
    Ok(policy)
}

#[cfg(test)]
#[cfg(target_arch = "wasm32")]
mod health_setup_tests {
    //! Real SDK reads against the shipped local manifest, in fresh Viceroy guests.

    use super::{AuctioneerApp, HealthAccessPolicy, Hooks, resolve_health_policy};
    use auctioneer_core::host_primitives::{
        HealthAuthz, HealthBootError, counter_scope_handle, health_authorize, health_policy_handle,
    };

    const FLAG_KEY: &str = "allow_unauthenticated_health";
    const TOKEN: &str = "auctioneer-development-only-health-test-token";
    const FLAG_TOKEN: &str = "flagHeadF8v3.flagBodyJ7m2.flagSigQ5n9";

    #[test]
    #[ignore = "requires the local manifest and unset token env supplied by tests/manifest_health_stores.rs"]
    fn unset_local_token_resolves_closed() {
        let policy =
            resolve_health_policy().expect("the declared local aliases open without a token");
        assert!(matches!(policy, HealthAccessPolicy::Closed));
    }

    #[test]
    #[ignore = "requires the local manifest and development token env supplied by tests/manifest_health_stores.rs"]
    fn local_development_token_resolves_authenticated() {
        let policy = resolve_health_policy().expect("the local health token resolves");
        let HealthAccessPolicy::Token(auth) = policy else {
            panic!("the local development token must select bearer authentication");
        };
        assert!(auth.verify("auctioneer-development-only-health-test-token"));
        assert!(!auth.verify("wrong-development-token"));
    }

    #[test]
    #[ignore = "requires malformed flag fixtures supplied by tests/manifest_health_stores.rs"]
    fn misplaced_health_flag_is_diagnosed_and_installs_closed() {
        super::install_boot_log::<AuctioneerApp>();
        assert!(health_policy_handle().is_none());
        let Err(error) = resolve_health_policy() else {
            panic!("a credential in the health flag must be rejected");
        };
        assert!(matches!(
            error.downcast_ref::<HealthBootError>(),
            Some(HealthBootError::NonBooleanFlag { key }) if *key == FLAG_KEY
        ));
        for rendered in [error.to_string(), format!("{error:?}")] {
            assert!(rendered.contains(FLAG_KEY));
            assert!(rendered.contains("not a strict boolean"));
            for sensitive in [
                TOKEN,
                "auctioneer-development",
                "health-test-token",
                FLAG_TOKEN,
                "flagHeadF8v3",
                "flagBodyJ7m2",
                "flagSigQ5n9",
                "F8v3",
                "J7m2",
                "Q5n9",
            ] {
                assert!(
                    !rendered.contains(sensitive),
                    "neither Display nor Debug may expose credentials or their segments"
                );
            }
        }

        super::install_health().expect("a malformed flag only closes health");
        assert_eq!(counter_scope_handle(), "request");
        assert!(matches!(
            health_policy_handle(),
            Some(HealthAccessPolicy::Closed)
        ));
        for bearer in [None, Some(TOKEN), Some(FLAG_TOKEN)] {
            assert_eq!(health_authorize(bearer), HealthAuthz::Deny);
        }
    }

    #[test]
    #[ignore = "requires conflicting legacy store selectors supplied by tests/manifest_health_stores.rs"]
    fn health_and_sdk_reads_use_logical_aliases_despite_legacy_selectors() {
        use edgezero_adapter_fastly::config_store::FastlyConfigStore;
        use edgezero_adapter_fastly::secret_store::FastlySecretStore;
        use edgezero_core::config_store::ConfigStore as _;
        use edgezero_core::secret_store::SecretStore as _;
        use futures::executor::block_on;

        let stores = <AuctioneerApp as Hooks>::stores();
        let config_id = stores.config.expect("config alias").default;
        let secrets_id = stores.secrets.expect("secret alias").default;
        let sdk_config = FastlyConfigStore::try_open(config_id).expect("SDK opens config alias");
        assert_eq!(
            block_on(sdk_config.get(FLAG_KEY))
                .expect("SDK reads config alias")
                .as_deref(),
            Some("false")
        );
        let sdk_token = block_on(FastlySecretStore.get_bytes(secrets_id, "health_gvl_token"))
            .expect("SDK reads secret alias")
            .expect("logical alias token");
        assert!(
            sdk_token.as_ref() == TOKEN.as_bytes(),
            "SDK must read the logical token"
        );
        let HealthAccessPolicy::Token(auth) =
            resolve_health_policy().expect("health opens aliases")
        else {
            panic!(
                "logical stores select bearer authentication, not the physical store's open policy"
            );
        };
        assert!(auth.verify(TOKEN));
        assert!(!auth.verify("wrong-physical-store-token"));
        super::run().expect("legacy selectors cannot break SDK response dispatch");
        assert_eq!(health_authorize(Some(TOKEN)), HealthAuthz::Allow);
        assert_eq!(health_authorize(None), HealthAuthz::Deny);
    }

    #[test]
    #[ignore = "requires missing store fixtures supplied by tests/manifest_health_stores.rs"]
    fn a_missing_health_store_stays_closed_through_sdk_dispatch() {
        assert!(
            resolve_health_policy().is_err(),
            "the fixture must actually fail store resolution"
        );
        super::run().expect("a store fault must not stop the SDK lifecycle");
        assert_eq!(counter_scope_handle(), "request");
        assert!(matches!(
            health_policy_handle(),
            Some(HealthAccessPolicy::Closed)
        ));
        for bearer in [None, Some(TOKEN)] {
            assert_eq!(health_authorize(bearer), HealthAuthz::Deny);
        }
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

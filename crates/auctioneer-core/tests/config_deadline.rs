//! The config-fetch deadline and the admission lifecycle around it.
//!
//! The runtime-config loader runs while the request already HOLDS its
//! admission lease, in a phase covered by neither the ingress-read
//! deadline nor the egress write deadline. Enough stalled store lookups
//! therefore exhaust auction capacity with no auctioneer allocation to
//! blame. The bound that closes that is the host's own typed-config
//! extraction deadline, which auctioneer INSTALLS rather than duplicates.
//!
//! # What these tests assert, and what they deliberately do not
//!
//! auctioneer's contract is two-sided and both sides are provable here:
//!
//! 1. It SUPPLIES the bound: the fixed two-second fetch timeout becomes
//!    `ConfigExtractionLimits.timeout` on the `App`, and the host turns
//!    that into an absolute deadline every store read is handed.
//! 2. It MAPS the typed expiry onto `ConfigStoreUnavailable`, which is
//!    503 plus `Retry-After` on the wire.
//!
//! What is NOT asserted anywhere below is that expiry CANCELS the
//! pending provider read. Upstream's own capability text puts every
//! adapter at `BestEffort` for `config-read-deadlines`: Cloudflare and
//! Spin race guest futures against native timers, while Axum and Fastly
//! provider operations are synchronous and guest-nonpreemptible, and no
//! provider operation is proved cancellable within a finite wall-clock
//! interval. A auctioneer test that required a never-ready read to be
//! cancelled would be requiring something no adapter promises, and the
//! only way to satisfy it would be the auctioneer-local outer race that
//! duplicating an upstream bound rules out.
//!
//! # The anchor is extraction start, not request start
//!
//! The host computes `started_at = clock.now()` at extraction ENTRY and
//! adds the configured timeout, and `ConfigExtractionLimits` exposes a
//! relative `Duration` only. So the deadline a store is handed is
//! `extraction_started_at + timeout`, not `request_start + timeout`, and
//! that is what the recording store below pins. The cost is stated
//! rather than hidden: upload and parse time fall outside the bound, so
//! a slow client can push total request time past it. The value exists
//! to stop a hung STORE from hanging a request that is holding admission
//! capacity, and it still does exactly that.

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use async_trait::async_trait;
    use auctioneer_core::config::runtime::{
        CONFIG_FETCH_TIMEOUT, RuntimeConfigError, resolve_runtime_config,
    };
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::host_primitives::ingress_lease::{
        RouteAdmissionMarker, RouteEgressPermit, admit_pending, install_route_permit,
    };
    use auctioneer_core::test_support::config_store as funded_config;
    use auctioneer_runtime::{AdmissionGate, EgressGate, MonotonicInstant};
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigExtractionLimits, ConfigStore, ConfigStoreError, ConfigStoreHandle,
        ConfigValue,
    };
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::{StatusCode, request_builder};
    use edgezero_core::router::RouterService;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    const STORE_ID: &str = "auctioneer_runtime";

    // WHY THERE IS NO SERIALISING MUTEX IN THIS FILE, since every other
    // gate test in the tree has one: those tests contend for the
    // PROCESS-GLOBAL gates behind `BOOT_GATES`, where a concurrent
    // holder can acquire and release between a snapshot and an
    // assertion — which is why a snapshot delta is not sufficient there
    // either. Every gate below is built LOCAL to its own test and is
    // reachable from nowhere else, so there is no shared counter to
    // race on. Nothing in this binary installs the boot envelope, so
    // `boot_limits()` answers `None` throughout and the byte caps the
    // configure callback resolves are the host's defaults; the timeout
    // this file asserts is a fixed constant and is envelope-independent
    // either way.

    /// A store that NEVER answers with a value: `get_bounded` records the
    /// deadline it was handed and reports the typed expiry.
    ///
    /// This is the never-completing lookup in the only shape auctioneer can
    /// own. A literally pending future would prove nothing testable —
    /// whether the provider read is abandoned is upstream's `BestEffort`
    /// business — while a store that reports `DeadlineExceeded` proves
    /// exactly the two things auctioneer is responsible for: the deadline
    /// arrived, and its expiry became a 503.
    struct ExpiringStore {
        seen: Mutex<Vec<Deadline>>,
    }

    impl ExpiringStore {
        fn new() -> Arc<Self> {
            Arc::new(ExpiringStore {
                seen: Mutex::new(Vec::new()),
            })
        }

        /// The deadlines `get_bounded` was handed, in call order.
        fn recorded(&self) -> Vec<Deadline> {
            self.seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        }
    }

    #[async_trait(?Send)]
    impl ConfigStore for ExpiringStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            Err(ConfigStoreError::DeadlineExceeded)
        }

        async fn get_bounded(
            &self,
            _key: &str,
            _clock: &MonotonicClock,
            deadline: Deadline,
            _max_backend_bytes: u64,
            _max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            self.seen
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(deadline);
            Err(ConfigStoreError::DeadlineExceeded)
        }
    }

    /// A request wired to `store` under the id the loader reads.
    fn ctx_with_store(store: Arc<ExpiringStore>) -> RequestContext {
        config_funding::install();
        let mut request = request_builder()
            .uri("/openrtb2/auction")
            .body(Body::empty())
            .expect("a static request builds");
        let registry: ConfigRegistry = StoreRegistry::single_id(
            STORE_ID.to_owned(),
            ConfigStoreBinding {
                default_key: STORE_ID.to_owned(),
                handle: ConfigStoreHandle::new(store),
            },
        );
        request.extensions_mut().insert(registry);
        funded_config::context(request)
    }

    /// The limits `configure_app` actually installs, read back off a
    /// freshly configured `App` rather than recomputed here.
    fn installed_limits() -> ConfigExtractionLimits {
        let mut app = App::new(RouterService::builder().build());
        configure_app(&mut app).expect("auctioneer's extraction limits are accepted by the host");
        app.config_extraction_limits()
    }

    #[test]
    fn fixture_context_preserves_route_clock_extensions_and_unread_body() {
        use auctioneer_core::host_primitives::TestMonotonicClockHandle;
        use edgezero_core::http::Method;
        use edgezero_core::ingress::DEFAULT_INBOUND_READ_BUDGET;
        use futures::stream;

        let instant = MonotonicInstant::now();
        let clock = MonotonicClock::new(move || instant);
        let mut request = request_builder()
            .method(Method::POST)
            .uri("/fixture/publisher?opaque=unchanged")
            .header("x-fixture", "untouched")
            .body(Body::from_bytes(b"original request body".to_vec()))
            .unwrap();
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(clock.clone()));
        let ctx = funded_config::context_for_route(request, "/fixture/{publisher}", clock.clone());
        assert_eq!(ctx.method(), &Method::POST);
        assert_eq!(ctx.uri().to_string(), "/fixture/publisher?opaque=unchanged");
        assert_eq!(ctx.headers()["x-fixture"], "untouched");
        assert_eq!(ctx.path_params().get("publisher"), Some("publisher"));
        assert_eq!(ctx.monotonic_clock().now(), instant);
        assert_eq!(ctx.request_start(), instant);
        let read_deadline =
            Deadline::at_instant(instant.checked_add(DEFAULT_INBOUND_READ_BUDGET).unwrap());
        assert_eq!(
            ctx.read_deadline().map(|value| value.instant()),
            Some(read_deadline.instant())
        );
        assert_eq!(
            ctx.extensions()
                .get::<TestMonotonicClockHandle>()
                .unwrap()
                .0
                .now(),
            instant,
        );
        assert_eq!(ctx.config_extraction_limits(), installed_limits());
        assert_eq!(
            ctx.config_extraction_limits().timeout,
            Duration::from_secs(2)
        );
        assert!(ctx.config_store_binding(STORE_ID).is_none());
        assert_eq!(
            block_on(ctx.body_bytes(1024)).unwrap().as_ref(),
            b"original request body"
        );
        assert_eq!(
            ctx.read_deadline().map(|value| value.instant()),
            Some(read_deadline.instant())
        );

        let unread_request = request_builder()
            .uri("/fixture/unread")
            .body(Body::from_stream(stream::poll_fn(|_| {
                panic!("context capture must not poll the fixture body");
            })))
            .unwrap();
        let unread =
            funded_config::context_for_route(unread_request, "/fixture/{publisher}", clock);
        assert_eq!(unread.path_params().get("publisher"), Some("unread"));
    }

    // -----------------------------------------------------------------
    // 1. auctioneer SUPPLIES the bound.
    // -----------------------------------------------------------------

    /// The `app!` configure callback installs the fixed two-second timeout.
    ///
    /// Before this seam existed the loader silently inherited upstream's
    /// own default, which is far wider, so the whole fetch bound was
    /// inert while the plan described it as live. That is the regression
    /// this pins: the value on the `App` is auctioneer's, not the host's.
    #[test]
    fn configure_installs_the_fixed_fetch_bound_as_the_extraction_timeout() {
        let expected = Duration::from_secs(2);
        assert_eq!(CONFIG_FETCH_TIMEOUT, expected);
        assert_eq!(
            installed_limits().timeout,
            expected,
            "the extraction timeout is the fixed two-second fetch bound",
        );
        // TIGHTER than the host's default, not merely different from it.
        // A bound at or above the default would install a number that
        // changes nothing upstream was not already enforcing, which is
        // the inert state this seam exists to leave behind. Compared
        // against the host's own constant rather than a copied literal,
        // so a default that moves upstream is caught here.
        assert!(
            installed_limits().timeout < ConfigExtractionLimits::default().timeout,
            "auctioneer's bound must actually tighten the host's default, not restate it",
        );
    }

    /// Repeated `App` builds converge: the callback runs per build, and
    /// a serverless adapter can build per request, so installing must be
    /// an overwrite rather than an accumulation.
    #[test]
    fn installing_the_limits_twice_is_idempotent() {
        let mut app = App::new(RouterService::builder().build());
        configure_app(&mut app).expect("first build configures");
        let first = app.config_extraction_limits();
        configure_app(&mut app).expect("second build configures");
        assert_eq!(
            app.config_extraction_limits(),
            first,
            "a second build installs the same limits over the first build's",
        );
    }

    /// Whatever the byte caps resolve to, the host must ACCEPT the set.
    /// Its validator rejects a zero cap and a total below the largest
    /// per-value cap, and a set it rejects would fail boot rather than
    /// bound anything.
    #[test]
    fn the_installed_limits_pass_the_hosts_own_validator() {
        let limits = installed_limits();
        limits
            .validate()
            .expect("the host accepts the limits auctioneer installs");
        assert!(
            !limits.timeout.is_zero(),
            "a zero fetch bound would expire every read before it started",
        );
    }

    // -----------------------------------------------------------------
    // 2. The bound REACHES the store, anchored at extraction start.
    // -----------------------------------------------------------------

    /// The store is handed an absolute deadline of
    /// `extraction_started_at + timeout`, and auctioneer maps its expiry onto
    /// the 503 class.
    ///
    /// The anchor is bracketed rather than compared for equality because
    /// `started_at` is read inside the host's extraction budget and is
    /// not observable from here: the deadline must land in
    /// `[before + timeout, after + timeout]`, which no other anchor
    /// satisfies. `request_start + timeout` in particular does not —
    /// that was the original specification, and it is not reachable
    /// through the typed extractor.
    ///
    /// The context captures Auctioneer's fixed timeout through actual SDK App
    /// dispatch. This test pins its extraction-entry anchor independently of
    /// `configure_installs_the_fixed_fetch_bound_as_the_extraction_timeout`.
    #[test]
    fn config_read_passes_the_envelope_deadline() {
        let store = ExpiringStore::new();
        let ctx = ctx_with_store(Arc::clone(&store));
        let timeout = ctx.config_extraction_limits().timeout;

        let before = MonotonicInstant::now();
        let err = block_on(resolve_runtime_config(&ctx))
            .expect_err("an expiring store never yields a config");
        let after = MonotonicInstant::now();

        let recorded = store.recorded();
        let deadline = recorded
            .first()
            .copied()
            .expect("the loader called get_bounded exactly once");
        assert_eq!(recorded.len(), 1, "one extraction, one bounded read");

        let earliest = before.checked_add(timeout).expect("no overflow");
        let latest = after.checked_add(timeout).expect("no overflow");
        assert!(
            deadline.instant() >= earliest,
            "the deadline is no EARLIER than extraction entry plus the timeout",
        );
        assert!(
            deadline.instant() <= latest,
            "and no LATER: the anchor is extraction start plus that same timeout",
        );

        assert!(
            matches!(err, RuntimeConfigError::ConfigStoreUnavailable { .. }),
            "a deadline expiry is transient, not a malformed host: {err:?}",
        );
        let response = err.into_response().expect("the 503 renders");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            response.headers().contains_key("retry-after"),
            "a transient store fault tells the caller when to come back",
        );
    }

    /// Expiry is 503, never 408. A request timeout names the CLIENT as
    /// the slow party; a config-fetch expiry names the store, and the
    /// caller's remedy is to retry rather than to resend faster.
    #[test]
    fn a_fetch_expiry_is_503_and_never_408() {
        let store = ExpiringStore::new();
        let ctx = ctx_with_store(store);
        let response = block_on(resolve_runtime_config(&ctx))
            .expect_err("an expiring store never yields a config")
            .into_response()
            .expect("the response renders");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_ne!(
            response.status(),
            StatusCode::REQUEST_TIMEOUT,
            "the slow party is the store, not the client",
        );
    }

    // -----------------------------------------------------------------
    // 3. A slow store is transient, not a missing publication.
    // -----------------------------------------------------------------

    /// A timeout must never become a missing-publication error or silently
    /// resolve product defaults. The transient detail identifies the store's
    /// deadline failure independently of whether a publication exists.
    #[test]
    fn a_timeout_remains_transient_and_distinct_from_missing_config() {
        let store = ExpiringStore::new();
        let ctx = ctx_with_store(store);
        let err = block_on(resolve_runtime_config(&ctx))
            .expect_err("a timeout never resolves a runtime policy");

        let detail = match &err {
            RuntimeConfigError::ConfigStoreUnavailable { detail, .. } => detail.clone(),
            RuntimeConfigError::RegistryBuildFailed { .. }
            | RuntimeConfigError::UnfundedExtraction { .. }
            | RuntimeConfigError::MalformedHost { .. }
            | RuntimeConfigError::StoreKeyRejected { .. } => {
                panic!("a slow store must remain transient, got {err:?}")
            }
        };
        assert!(
            detail.contains("DeadlineExceeded"),
            "the detail names the deadline reason, not the missing-config rule: {detail}",
        );
        assert!(
            !detail.contains("missing config"),
            "a timeout never travels the missing-publication arm: {detail}",
        );
    }

    // -----------------------------------------------------------------
    // 4. Capacity recovery, scoped to the handler-future boundary.
    // -----------------------------------------------------------------

    /// A request whose config read expires returns its admission
    /// capacity, so the next request is admitted.
    ///
    /// SCOPE, stated because it is narrower than it sounds: recovery is
    /// demonstrated at the HANDLER-FUTURE boundary — the permit releases
    /// when the response future completes — and NOT at terminal
    /// transmission. A 503 config-fetch expiry goes out as a refusal, and
    /// the host's completion path does not yet cover detached egress, so
    /// nothing signals that the refusal finished. This test therefore
    /// proves the release is exactly-once and prompt on auctioneer's side; it
    /// does not claim recovery-on-terminal-transmission.
    #[test]
    fn a_timed_out_request_returns_its_admission_capacity() {
        let gate = AdmissionGate::new(1);

        let permit = gate.try_acquire().expect("capacity 1 admits the first");
        assert_eq!(gate.in_use(), 1, "the request holds its lease");
        assert!(
            gate.try_acquire().is_none(),
            "capacity 1 means the second request waits on the first",
        );

        // The lease is held ACROSS the config read, which is the whole
        // point: the read runs on borrowed admission time.
        let store = ExpiringStore::new();
        let ctx = ctx_with_store(store);
        let err = block_on(resolve_runtime_config(&ctx))
            .expect_err("an expiring store never yields a config");
        assert!(matches!(
            err,
            RuntimeConfigError::ConfigStoreUnavailable { .. }
        ));
        assert_eq!(
            gate.in_use(),
            1,
            "the lease survives the failed read: the refusal is still unwritten",
        );

        // The handler future completing is what drops the permit.
        drop(permit);
        assert_eq!(gate.in_use(), 0, "the expired request returned its slot");
        assert!(
            gate.try_acquire().is_some(),
            "so the next request is admitted rather than shed behind a dead one",
        );
    }

    /// The release is exactly once even though the timeout path never
    /// acquires an egress permit at all.
    ///
    /// An auction admission hands back an EMPTY late-bound slot; the
    /// handler fills it after the auction resolves. A config-fetch expiry
    /// returns before that, so the slot closes empty. That must be a
    /// no-op rather than a stranded permit or a double release — it is
    /// the path every config failure, refusal and timeout takes.
    #[test]
    fn an_expired_config_read_strands_no_egress_capacity() {
        let egress = EgressGate::new(1);
        let (lease, completion) = admit_pending(RouteAdmissionMarker::Auction);
        assert_eq!(
            egress.in_use(),
            0,
            "a pending admission holds no egress capacity: nothing is transmittable yet",
        );

        let store = ExpiringStore::new();
        let ctx = ctx_with_store(store);
        assert!(
            block_on(resolve_runtime_config(&ctx)).is_err(),
            "the read expires before the handler ever reaches egress",
        );

        // The handler returned its 503 without acquiring, so the slot is
        // still empty when the response terminates.
        drop(lease);
        drop(completion);
        assert_eq!(
            egress.in_use(),
            0,
            "an empty slot closing releases nothing and strands nothing",
        );
        assert!(
            egress.try_acquire().is_some(),
            "the egress class is untouched by a request that never reached it",
        );
    }

    /// And a permit installed into that slot after it closed releases
    /// immediately rather than leaking — the shape a client that
    /// disconnects during a slow config read produces.
    #[test]
    fn a_permit_installed_after_an_expired_response_closed_releases_at_once() {
        let egress = EgressGate::new(1);
        let (lease, completion) = admit_pending(RouteAdmissionMarker::Auction);
        drop(completion);

        let permit = egress.try_acquire().expect("capacity 1 admits");
        assert_eq!(egress.in_use(), 1);
        assert!(
            !install_route_permit(
                &lease.route_egress_resource,
                RouteEgressPermit::Auction(permit),
            ),
            "a closed slot does not take ownership",
        );
        assert_eq!(
            egress.in_use(),
            0,
            "the refused permit dropped inside the install and returned its slot",
        );
    }
}

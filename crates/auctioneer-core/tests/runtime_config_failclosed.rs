//! Runtime-config load failures that CAN be told apart from a fresh
//! deploy must fail closed rather than serve from the baked default.
//!
//! The baked default carries `[gdpr.tcf2] enabled = false`, so a
//! degraded request forwards `user.id` and `device.ifa` to every bidder.
//! Each row below posts the SAME request through the real auction
//! handler and varies only what the config store does, so the difference
//! in outcome can only come from the load-failure classification.
//!
//! The degrading rows deliberately assert the leak they permit — that is
//! what makes the fail-closed rows mean something.
//!
//! One of those classes is not about the blob at all: the extractor reads
//! the BINDING's `default_key`, which the deploy resolves from
//! `edgezero.toml`, so a backend that refuses that key leaves a deploy
//! where no push can ever land. That is a broken deploy rather than a
//! fresh one, and it fails closed too.
//!
//! Fail-closed is the default, not a universal rule, and the last group
//! of rows pins the exception. `/status` is a liveness probe that reaches
//! no privacy decision, so it takes the permissive disposition: a probe
//! that 500s through a config-store outage turns a degraded-but-serving
//! fleet into a restart loop that cannot fix the store. `/info/bidders`
//! keeps fail-closed, because its entire response is derived from the
//! config it could not verify. Both dispositions are asserted against the
//! SAME corrupt store, so the difference can only come from the routes'
//! deliberate choice rather than from what the store did.
//!
//! Those `/status` rows share one process-global verdict cache, because
//! the handler answers a probe it could not give a config slot from the
//! verdict the isolate last recorded. Every one of them therefore goes
//! through `probe_status`, which holds that state still for the length
//! of a call — see its own comment for what goes wrong without it.

#[cfg(test)]
mod support;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod bounded_store;

#[cfg(test)]
mod tests {
    use crate::bounded_store::bounded_over_get;
    use crate::config_funding;
    use crate::serving_catalog::passthrough_info;
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_core::config::runtime::{
        CONFIG_STATUS_HEADER, RuntimeConfigError, resolve_runtime_config,
    };
    use auctioneer_core::handlers::probe_config::__probe_isolation_for_tests;
    use auctioneer_core::handlers::{TestFanoutHandle, auction, info_bidders, status};
    use auctioneer_core::host_primitives::{TestClockHandle, TestRngHandle};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, TimeoutCause,
    };
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::blob_envelope::BlobEnvelope;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
    };
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::{Response, StatusCode};
    use edgezero_core::params::PathParams;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    const STORE_ID: &str = "auctioneer_runtime";

    #[test]
    fn pushed_alias_base_refusal_has_a_bounded_diagnostic() {
        config_funding::install();
        let source = toml::to_string(&serde_json::json!({
            "aliases": {"fake_short_alias": format!("fake_{}", "x".repeat(0x0001_0000 - 5))}
        }))
        .unwrap();
        let request = Request::builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let ctx = ctx_with_config(&source, request);
        let error = block_on(resolve_runtime_config(&ctx))
            .expect_err("the pushed alias base exceeds its byte cap");
        assert_eq!(error.status_code().as_str(), "malformed_config");
        let RuntimeConfigError::MalformedHost { detail: diagnostic } = error else {
            panic!("an oversized base must be malformed host configuration");
        };
        assert!(
            diagnostic.len() < 512,
            "diagnostic is {} bytes",
            diagnostic.len()
        );
        assert!(!diagnostic.contains(&"x".repeat(128)));
    }
    const WIDE_CAP: usize = 1 << 20;
    /// A sha of the right SHAPE but the wrong value, so the envelope
    /// parses and only its integrity check fails.
    const WRONG_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";
    /// Key-length ceiling for [`RejectingKeyStore`], modelled on the 512
    /// bytes `edgezero_core::key_value_store` documents as the platform
    /// limit. The exact number does not matter to the assertion — only
    /// that the backend has one and the bound key exceeds it.
    const BACKEND_MAX_KEY_LEN: usize = 512;

    // -----------------------------------------------------------------
    // Config stores, one per failure class
    // -----------------------------------------------------------------

    /// A blob whose recorded sha disagrees with its own `data` — one
    /// flipped byte in the store looks exactly like this. `EdgeZero`
    /// surfaces it as `EdgeError::Internal`.
    struct CorruptStore;

    #[async_trait(?Send)]
    impl ConfigStore for CorruptStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            let mut envelope = BlobEnvelope::new(
                serde_json::json!({ "gdpr": { "enabled": true } }),
                "1970-01-01T00:00:00Z".to_owned(),
            );
            envelope.sha256 = WRONG_SHA.to_owned();
            Ok(Some(
                serde_json::to_string(&envelope)
                    .expect("envelope serialises")
                    .into(),
            ))
        }
        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    /// The backend cannot serve right now. `EdgeZero` surfaces it as
    /// `EdgeError::ServiceUnavailable`.
    struct OutageStore;

    #[async_trait(?Send)]
    impl ConfigStore for OutageStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            Err(ConfigStoreError::Unavailable {
                message: "kv namespace unreachable".to_owned(),
            })
        }
        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    /// No blob pushed yet — the legitimate fresh-deploy state.
    /// `EdgeZero` surfaces it as `EdgeError::ConfigOutOfDate`, the one
    /// class that cannot be told apart from a malformed push.
    struct EmptyStore;

    #[async_trait(?Send)]
    impl ConfigStore for EmptyStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            Ok(None)
        }
        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    /// A backend that refuses the key it was handed. Real backends do
    /// this: the Fastly config store maps its SDK's `KeyInvalid` and
    /// `KeyTooLong` to `ConfigStoreError::InvalidKey`, and the
    /// `config_store_contract_tests!` suite every backend runs permits
    /// exactly this answer for a key it will not accept. `EdgeZero`
    /// surfaces it as `EdgeError::BadRequest`.
    struct RejectingKeyStore;

    #[async_trait(?Send)]
    impl ConfigStore for RejectingKeyStore {
        async fn get(&self, key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            if key.len() > BACKEND_MAX_KEY_LEN {
                return Err(ConfigStoreError::invalid_key("invalid config key"));
            }
            Ok(None)
        }
        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    /// A blob that passes every integrity and deserialization check and
    /// is then refused by `validate_alias_bases`, which interpolates the
    /// store-controlled alias and base names into
    /// `RuntimeConfigError::detail`. That is the one construction site
    /// whose `detail` carries values a config writer chose, so it is the
    /// store the public-channel leak guard has to drive.
    struct HostileAliasStore;

    /// Alias and base names a config writer picked. Neither may reach an
    /// unauthenticated response.
    const CANARY_ALIAS: &str = "fake_canary_alias_must_never_appear";
    const CANARY_BASE: &str = "fake_canary_base_must_never_appear";
    /// A `status_response` the healthy row reads back verbatim. It sits
    /// in the hostile blob too, so the leak guard also proves a REFUSED
    /// blob's bytes never reach the wire.
    const HOSTILE_BANNER: &str = "auctioneer up";

    #[async_trait(?Send)]
    impl ConfigStore for HostileAliasStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            let envelope = BlobEnvelope::new(
                serde_json::json!({
                    "status": { "banner": HOSTILE_BANNER },
                    "aliases": { CANARY_ALIAS: CANARY_BASE },
                }),
                "1970-01-01T00:00:00Z".to_owned(),
            );
            Ok(Some(
                serde_json::to_string(&envelope)
                    .expect("envelope serialises")
                    .into(),
            ))
        }
        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    /// A blob that loads cleanly, so the healthy rows assert against a
    /// real successful resolution rather than against a missing binding.
    struct HealthyStore;

    #[async_trait(?Send)]
    impl ConfigStore for HealthyStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            let envelope = BlobEnvelope::new(
                serde_json::json!({ "status": { "banner": HOSTILE_BANNER } }),
                "1970-01-01T00:00:00Z".to_owned(),
            );
            Ok(Some(
                serde_json::to_string(&envelope)
                    .expect("envelope serialises")
                    .into(),
            ))
        }
        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            bounded_over_get(
                self,
                key,
                clock,
                deadline,
                max_backend_bytes,
                max_value_bytes,
            )
            .await
        }
    }

    #[derive(Clone)]
    struct ConfigReads(Arc<AtomicUsize>);

    struct CountedStore {
        inner: Arc<dyn ConfigStore>,
        reads: ConfigReads,
    }

    #[async_trait(?Send)]
    impl ConfigStore for CountedStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("the resolver must use bounded reads")
        }

        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            self.reads.0.fetch_add(1, Ordering::SeqCst);
            self.inner
                .get_bounded(key, clock, deadline, max_backend_bytes, max_value_bytes)
                .await
        }
    }

    struct TrackedRegistry {
        registry: ConfigRegistry,
        reads: ConfigReads,
    }

    fn registry_of(store: Arc<dyn ConfigStore>) -> TrackedRegistry {
        registry_keyed(STORE_ID.to_owned(), store)
    }

    /// The store id and the store KEY are different things: the id
    /// selects the binding, and the binding's `default_key` is what the
    /// extractor actually asks the backend for. `resolve_runtime_config`
    /// passes no explicit key, so a deploy whose `edgezero.toml` resolves
    /// a bad `default_key` sends that bad key to the backend.
    fn registry_keyed(default_key: String, store: Arc<dyn ConfigStore>) -> TrackedRegistry {
        config_funding::install();
        let reads = ConfigReads(Arc::new(AtomicUsize::new(0)));
        let registry = StoreRegistry::single_id(
            STORE_ID.to_owned(),
            ConfigStoreBinding {
                default_key,
                handle: ConfigStoreHandle::new(Arc::new(CountedStore {
                    inner: store,
                    reads: reads.clone(),
                })),
            },
        );
        TrackedRegistry { registry, reads }
    }

    // -----------------------------------------------------------------
    // Fixture request + recording fanout
    // -----------------------------------------------------------------

    /// Carries the identifiers a degraded (enforcement-off) resolution
    /// would forward verbatim.
    fn fixture_body() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": "auc-failclosed",
            "tmax": 1000,
            "cur": ["USD"],
            "regs": { "gdpr": 1 },
            "site": {
                "publisher": { "id": "acct-fc" },
                "page": "https://pub.example.test/article",
            },
            "user": { "id": "user-123" },
            "device": { "ifa": "ifa-abc-123" },
            "imp": [{
                "id": "imp-1",
                "banner": { "w": 300, "h": 250 },
                "ext": { "bidder": { "fake_bidder": {} } },
            }],
            "ext": { "prebid": { "aliases": { "fake_bidder": passthrough_info().name } } },
        }))
        .expect("fixture body serialises")
    }

    #[derive(Default)]
    struct RecordingFanout {
        bodies: Mutex<Vec<Vec<u8>>>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut recorded = self.bodies.lock().unwrap_or_else(PoisonError::into_inner);
            for call in &calls {
                recorded.push(call.request.body().to_vec());
            }
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    struct Driven {
        status: StatusCode,
        retry_after: Option<String>,
        body: String,
        outbound: Vec<Vec<u8>>,
    }

    /// `registry = None` models a deploy with no `[stores.config]` at
    /// all; `Some(..)` binds the store under test.
    fn drive(registry: Option<TrackedRegistry>) -> Driven {
        let read_counter = registry.as_ref().map(|bound| bound.reads.clone());
        let fanout = Arc::new(RecordingFanout::default());
        let handler_fanout: Arc<dyn BidderFanout + Send + Sync> =
            Arc::<RecordingFanout>::clone(&fanout);
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));

        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(fixture_body()))
            .expect("request builds");
        if let Some(bound) = registry {
            request.extensions_mut().insert(bound.registry);
        }
        request
            .extensions_mut()
            .insert(TestFanoutHandle(handler_fanout));
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        let ctx = RequestContext::new(request, PathParams::default());

        let driven = block_on(async {
            let response = auction(ctx).await.expect("the handler answers");
            let status = response.status();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let (_parts, response_body) = response.into_parts();
            let bytes = response_body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            Driven {
                status,
                retry_after,
                body: String::from_utf8_lossy(&bytes).into_owned(),
                outbound: fanout
                    .bodies
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone(),
            }
        });
        if let Some(reads) = read_counter {
            assert_eq!(
                reads.0.load(Ordering::SeqCst),
                1,
                "the bound fault must reach the SDK read"
            );
        }
        driven
    }

    // -----------------------------------------------------------------
    // Fail-closed rows
    // -----------------------------------------------------------------

    #[test]
    fn corrupt_blob_fails_closed_with_500_and_never_reaches_fanout() {
        let driven = drive(Some(registry_of(Arc::new(CorruptStore))));
        assert_eq!(
            driven.status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "a blob that fails its integrity check must not be served from the baked default; \
             body {body}",
            body = driven.body,
        );
        assert_eq!(driven.body, "Malformed host configuration");
        assert!(
            driven.outbound.is_empty(),
            "no bidder may be called, so no identifier can leak; recorded {n} call(s)",
            n = driven.outbound.len(),
        );
    }

    #[test]
    fn store_outage_fails_closed_with_503_and_retry_after() {
        let driven = drive(Some(registry_of(Arc::new(OutageStore))));
        assert_eq!(
            driven.status,
            StatusCode::SERVICE_UNAVAILABLE,
            "a transient store outage is not a fresh deploy; body {body}",
            body = driven.body,
        );
        assert_eq!(
            driven.retry_after.as_deref(),
            Some("5"),
            "a 503 has to tell the caller when to come back",
        );
        assert_eq!(driven.body, "Config store unavailable, retry after 5s");
        assert!(
            driven.outbound.is_empty(),
            "no bidder may be called; recorded {n} call(s)",
            n = driven.outbound.len(),
        );
    }

    #[test]
    fn rejected_store_key_fails_closed_with_500_and_never_reaches_fanout() {
        // The bound `default_key` comes from the deploy's `edgezero.toml`,
        // not from the store id, so it can be a key the backend refuses.
        // Nothing can ever be pushed where this deploy looks, which makes
        // it a broken deploy rather than a fresh one — degrading here
        // would leave GDPR enforcement off for the life of the mistake.
        let driven = drive(Some(registry_keyed(
            rejected_key(),
            Arc::new(RejectingKeyStore),
        )));
        assert_eq!(
            driven.status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "a key the backend rejects is a misconfigured deploy, not a fresh one; body {body}",
            body = driven.body,
        );
        assert_eq!(driven.body, "Malformed host configuration");
        assert!(
            driven.outbound.is_empty(),
            "no bidder may be called, so no identifier can leak; recorded {n} call(s)",
            n = driven.outbound.len(),
        );
    }

    /// The same store with an ACCEPTABLE key still degrades, which is
    /// what proves the row above turns on the rejected key rather than on
    /// the store type.
    #[test]
    fn accepted_store_key_with_no_blob_fails_closed_under_default_bootstrap() {
        let driven = drive(Some(registry_keyed(
            STORE_ID.to_owned(),
            Arc::new(RejectingKeyStore),
        )));
        assert_eq!(
            driven.status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "a key the backend accepts but holds no blob is a misconfigured deploy under the \
             baked `allow_missing_config = false`; body {body}",
            body = driven.body,
        );
        assert_eq!(driven.body, "Malformed host configuration");
        assert!(driven.outbound.is_empty(), "no bidder may be called");
    }

    // -----------------------------------------------------------------
    // The missing-blob row is a DEPLOY CHOICE, baked at build time: the
    // checked-in `data/runtime-bootstrap.toml` sets
    // `allow_missing_config = false`, so a bound store with nothing
    // pushed is a lost production key until a deploy says otherwise.
    // The `true` branch is pinned by the loader's unit tests, which
    // exercise the rule directly rather than rebuilding the crate.
    // -----------------------------------------------------------------

    #[test]
    fn missing_blob_fails_closed_under_default_bootstrap() {
        let driven = drive(Some(registry_of(Arc::new(EmptyStore))));
        assert_eq!(
            driven.status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "`no blob pushed yet` is indistinguishable from a deleted production key, and the \
             baked bootstrap does not allow it; body {body}",
            body = driven.body,
        );
        assert_eq!(driven.body, "Malformed host configuration");
        assert!(
            driven.outbound.is_empty(),
            "no bidder may be called, so no identifier can leak; recorded {n} call(s)",
            n = driven.outbound.len(),
        );
    }

    // -----------------------------------------------------------------
    // No [stores.config] binding at all takes the SAME immutable rule as
    // a bound store with no blob: under the checked-in
    // `allow_missing_config = false` it is a broken deploy, not a
    // fresh one — a production manifest missing its store must not
    // silently run with GDPR enforcement off.
    // -----------------------------------------------------------------

    #[test]
    fn absent_store_binding_fails_closed_under_default_bootstrap() {
        let driven = drive(None);
        assert_eq!(
            driven.status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "a deploy that declared no [stores.config] is a misconfigured deploy under the \
             baked bootstrap; body {body}",
            body = driven.body,
        );
        assert_eq!(driven.body, "Malformed host configuration");
        assert!(
            driven.outbound.is_empty(),
            "no bidder may be called, so no identifier can leak; recorded {n} call(s)",
            n = driven.outbound.len(),
        );
    }

    // -----------------------------------------------------------------
    // Per-route disposition. Same corrupt store, opposite outcomes,
    // because the routes differ in what failing closed would buy.
    // -----------------------------------------------------------------

    /// A GET context carrying only the config-store binding — neither
    /// `/status` nor `/info/bidders` needs a fanout, clock or rng.
    fn get_ctx(uri: &'static str, store: Arc<dyn ConfigStore>) -> RequestContext {
        get_ctx_keyed(uri, STORE_ID.to_owned(), store)
    }

    fn get_ctx_keyed(
        uri: &'static str,
        default_key: String,
        store: Arc<dyn ConfigStore>,
    ) -> RequestContext {
        let mut request = Request::builder()
            .method(Method::GET)
            .uri(Uri::from_static(uri))
            .body(Body::empty())
            .expect("request builds");
        let tracked = registry_keyed(default_key, store);
        request.extensions_mut().insert(tracked.registry);
        request.extensions_mut().insert(tracked.reads);
        RequestContext::new(request, PathParams::default())
    }

    fn rejected_key() -> String {
        "k".repeat(BACKEND_MAX_KEY_LEN + 1)
    }

    fn status_and_body(response: Response) -> (StatusCode, String) {
        let (code, body, _headers) = status_body_and_headers(response);
        (code, body)
    }

    /// The degradation rows assert on headers as well as the body, and
    /// the leak guard has to read every header value, so the response is
    /// taken apart once here rather than in each row.
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

    /// Drive `/status` through the real handler and take the answer
    /// apart, holding this process's probe state still for the length of
    /// the call.
    ///
    /// Every `/status` row in this file goes through here, and that is
    /// the point. The handler answers a probe it could not give a config
    /// slot from the verdict the isolate last recorded — correct in
    /// production, where the neighbours are earlier requests to the same
    /// deploy, and a race here, where `cargo test` makes the neighbours
    /// sibling tests on parallel threads of one process. The
    /// `HealthyStore` row records a clean verdict and every other row
    /// records a degraded one, so a shed probe read whichever landed last
    /// and asserted against the wrong one.
    ///
    /// The guard answers both halves at once: it clears the recorded
    /// verdict, so nothing stale can be read, and it keeps every other
    /// row out until this one is done, so nothing fresh can be recorded
    /// underneath it and the config gate has its slots free. It is bound
    /// to a name because dropping it at the end of this statement would
    /// release both before the handler ran.
    fn probe_status(ctx: RequestContext) -> (StatusCode, String, Vec<(String, String)>) {
        let isolation = __probe_isolation_for_tests();
        let read_counter = ctx.extensions().get::<ConfigReads>().cloned();
        let response = block_on(status(ctx)).expect("the status handler answers");
        let answer = status_body_and_headers(response);
        if let Some(reads) = read_counter {
            assert_eq!(
                reads.0.load(Ordering::SeqCst),
                1,
                "a funded probe must consult its bound store"
            );
        }
        drop(isolation);
        answer
    }

    /// Drive `/status` against a store and assert the shape every
    /// degraded answer shares: 200 rather than 204 or 500, the machine
    /// channel marked `degraded`, and the body ending in the named code.
    /// Returns the body so a row can assert what precedes that line.
    fn assert_degraded_status(ctx: RequestContext, code: &str) -> String {
        let (status_code, body, headers) = probe_status(ctx);
        assert_ne!(
            status_code,
            StatusCode::INTERNAL_SERVER_ERROR,
            "a liveness probe must not 500 on a config it could not load; body {body}",
        );
        assert_eq!(
            status_code,
            StatusCode::OK,
            "a degraded answer has a fault to name, so it is never the empty 204 shape; body \
             {body}",
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
        body
    }

    #[test]
    fn corrupt_blob_keeps_status_answering() {
        // The route still answers and still does not 500 — that is the
        // property this row has always pinned. What it gained is the
        // fault's name: an operator fixes a corrupt blob differently from
        // a refused key, so the two cannot share one code.
        let body = assert_degraded_status(
            get_ctx("/status", Arc::new(CorruptStore)),
            "malformed_config",
        );
        // The baked default carries an empty `status_response`, so
        // nothing precedes the diagnostic line and no blank line opens
        // the body.
        assert_eq!(body, "config: degraded (malformed_config)\n");
    }

    #[test]
    fn store_outage_keeps_status_answering() {
        // The case that motivates the exception: 503ing the probe through
        // an outage restarts a fleet that was still serving, and the
        // restart cannot fix the store. The code tells the operator to
        // wait for the backend rather than go looking for a bad push.
        assert_degraded_status(
            get_ctx("/status", Arc::new(OutageStore)),
            "store_unavailable",
        );
    }

    #[test]
    fn rejected_store_key_keeps_status_answering() {
        // The fail-closed class must not change the probe's disposition:
        // `/status` reaches no privacy decision, so it still degrades
        // rather than joining the auction in refusing to serve. It is
        // also the row that proves the codes are distinct: a refused key
        // and a corrupt blob arrive through the same fail-closed policy
        // and still report differently.
        assert_degraded_status(
            get_ctx_keyed("/status", rejected_key(), Arc::new(RejectingKeyStore)),
            "store_key_rejected",
        );
    }

    /// The body carries the CODE, and the answer is not a 500 — the two
    /// halves of the contract in one row, driven through a real
    /// malformed blob rather than a constructed error.
    #[test]
    fn status_names_the_fault_when_config_is_malformed() {
        let body = assert_degraded_status(
            get_ctx("/status", Arc::new(HostileAliasStore)),
            "malformed_config",
        );
        assert!(
            !body.contains("500"),
            "the probe reports the fault, it does not become one; body {body}",
        );
    }

    /// The structural boundary, asserted at the route. `/status` is
    /// unauthenticated, and `validate_alias_bases` interpolates
    /// store-controlled alias and base names into
    /// `RuntimeConfigError::detail`. Publishing `detail` would hand any
    /// caller fragments of the deploy's config, chosen by whoever can
    /// write the store. This fails the moment someone routes `detail` to
    /// the public channel.
    #[test]
    fn status_never_leaks_config_values() {
        let (_code, body, headers) = probe_status(get_ctx("/status", Arc::new(HostileAliasStore)));
        for canary in [CANARY_ALIAS, CANARY_BASE, HOSTILE_BANNER] {
            assert!(
                !body.contains(canary),
                "a value out of the refused blob reached the public body: {body}",
            );
            for (name, value) in &headers {
                assert!(
                    !value.contains(canary),
                    "a value out of the refused blob reached header {name}: {value}",
                );
            }
        }
    }

    /// The header works on the shape that has no body to carry anything.
    /// A deploy whose `status_response` is empty would otherwise answer
    /// a degraded probe with bytes indistinguishable from a healthy one.
    #[test]
    fn status_marks_degraded_in_a_header_on_both_branches() {
        let (_empty_code, _empty_body, empty_headers) =
            probe_status(get_ctx("/status", Arc::new(CorruptStore)));
        assert_eq!(
            header_of(&empty_headers, CONFIG_STATUS_HEADER).as_deref(),
            Some("degraded"),
            "the 204-shaped deploy still reports the fault, on the only channel it has",
        );

        let (_code, _body, headers) = probe_status(get_ctx("/status", Arc::new(HostileAliasStore)));
        assert_eq!(
            header_of(&headers, CONFIG_STATUS_HEADER).as_deref(),
            Some("degraded"),
        );
    }

    /// No false positives: a deploy whose config loads cleanly is marked
    /// `ok` and its body is the configured bytes VERBATIM. A probe that
    /// cried degraded on a healthy fleet would be ignored within a week.
    #[test]
    fn status_is_ok_and_unmarked_on_a_healthy_deploy() {
        let (code, body, headers) = probe_status(get_ctx("/status", Arc::new(HealthyStore)));
        assert_eq!(code, StatusCode::OK, "body {body}");
        assert_eq!(
            header_of(&headers, CONFIG_STATUS_HEADER).as_deref(),
            Some("ok"),
        );
        assert_eq!(
            body, HOSTILE_BANNER,
            "a healthy answer is the configured bytes and nothing else",
        );
    }

    /// The leak guard's other half, also at the route: upstream error
    /// text is the second way a value can reach `detail`, and the store
    /// message here would name the backend if it were published.
    #[test]
    fn status_detail_never_echoes_upstream_text() {
        let body = assert_degraded_status(
            get_ctx("/status", Arc::new(OutageStore)),
            "store_unavailable",
        );
        assert!(
            !body.contains("kv namespace unreachable"),
            "the backend's own message is a log-channel value; body {body}",
        );
    }

    /// The exact bytes a monitoring rule matches against, so two
    /// implementers cannot disagree on them.
    ///
    /// A degraded answer's `status_response` is the BAKED one, which
    /// ships empty, so every degraded body here is the diagnostic line
    /// alone with no leading blank line. It cannot be the store's
    /// banner: the blob carrying it is the one that was refused, and
    /// echoing bytes out of a rejected blob would reopen the leak the
    /// closed code exists to close. The separator rule that applies to a
    /// deploy whose baked banner is non-empty is pinned on `status_inner`
    /// directly, where the baked value is an input rather than a
    /// rebuild.
    #[test]
    fn degraded_body_format_is_exact() {
        let outage = assert_degraded_status(
            get_ctx("/status", Arc::new(OutageStore)),
            "store_unavailable",
        );
        assert_eq!(outage, "config: degraded (store_unavailable)\n");

        let malformed = assert_degraded_status(
            get_ctx("/status", Arc::new(HostileAliasStore)),
            "malformed_config",
        );
        assert_eq!(malformed, "config: degraded (malformed_config)\n");
    }

    #[test]
    fn rejected_store_key_fails_closed_on_info_bidders() {
        let response = block_on(info_bidders(get_ctx_keyed(
            "/info/bidders",
            rejected_key(),
            Arc::new(RejectingKeyStore),
        )))
        .expect("the handler renders the fail-closed response");
        let (code, body) = status_and_body(response);
        assert_eq!(
            code,
            StatusCode::INTERNAL_SERVER_ERROR,
            "a bidder catalog derived from a config that could not be read must not be \
             published; body {body}",
        );
        assert_eq!(body, "Malformed host configuration");
    }

    /// The isolation hook `probe_status` uses must stay reachable only
    /// from tests.
    ///
    /// `#[cfg(test)]` cannot express that: this file is a separate
    /// crate, so the library it links is compiled without `cfg(test)`
    /// set and anything gated that way would be invisible here. The hook
    /// is therefore an ordinary `pub fn`, named `__`-prefixed and marked
    /// `#[doc(hidden)]` so it is off the documented surface — and this
    /// row is what turns that convention into a checked property, by
    /// reading the crate's own sources for a caller.
    ///
    /// A source scan rather than a behavioural assertion for the same
    /// reason `aux_routes_wrapped` uses one: the property is about the
    /// whole set of call sites, including the one nobody wrote a test
    /// for, and that is the only kind that could ship.
    #[test]
    fn probe_isolation_is_test_only() {
        use std::fs;
        use std::path::{Path, PathBuf};

        const HOOK: &str = "__probe_isolation_for_tests";

        /// Every `.rs` file under a directory.
        fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in fs::read_dir(dir).expect("the source directory is readable") {
                let path = entry.expect("the directory entry reads").path();
                if path.is_dir() {
                    sources(&path, found);
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    found.push(path);
                } else {
                    // A non-Rust file under `src/` — nothing the scan
                    // can contain a call site.
                }
            }
        }

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        sources(&src, &mut files);
        assert!(
            !files.is_empty(),
            "the scan found no sources, so it would pass vacuously",
        );

        // The declaration lives in `probe_config.rs` and names itself
        // there, so that file is scanned for a CALL rather than for the
        // name: `pub fn <HOOK>` is the definition, `<HOOK>(` anywhere
        // else is an invocation.
        let mut callers = Vec::new();
        for file in &files {
            let text = fs::read_to_string(file).expect("the source file reads");
            for (index, line) in text.lines().enumerate() {
                let trimmed = line.trim_start();
                if !trimmed.contains(&format!("{HOOK}(")) {
                    continue;
                }
                if trimmed.starts_with("pub fn") || trimmed.starts_with("///") {
                    continue;
                }
                callers.push(format!(
                    "{path}:{number}: {trimmed}",
                    path = file.display(),
                    number = index + 1,
                ));
            }
        }
        assert!(
            callers.is_empty(),
            "the test-only probe isolation is called from library code, where it would \
             serialise and clear a live isolate's verdict cache: {callers:?}",
        );
    }

    #[test]
    fn corrupt_blob_fails_closed_on_info_bidders() {
        // Same store as `corrupt_blob_keeps_status_answering`. `/info` is
        // not a liveness surface and its whole response is derived from
        // the config, so an unverifiable blob makes the advertised bidder
        // list untrustworthy.
        let response = block_on(info_bidders(get_ctx(
            "/info/bidders",
            Arc::new(CorruptStore),
        )))
        .expect("the handler renders the fail-closed response");
        let (code, body) = status_and_body(response);
        assert_eq!(
            code,
            StatusCode::INTERNAL_SERVER_ERROR,
            "a bidder catalog derived from an unverifiable config must not be published; \
             body {body}",
        );
        assert_eq!(body, "Malformed host configuration");
    }
}

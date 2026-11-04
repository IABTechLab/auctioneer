//! Runtime host-alias registry validation (Fanout-9).
//!
//! Neutral compiled product defaults contain no operator aliases. The direct
//! runtime policy's `[host.aliases]` arrives through the config store and must
//! be validated against the linked catalog. Without a registry check on that
//! path, an alias naming a base this
//! build does not carry would resolve into an auction plan that can
//! never bid — a config error surfaced as silent no-bids instead of the
//! fail-loud 500 every other malformed-host shape produces.
//!
//! The store below returns a VALID envelope (integrity check passes)
//! whose config is structurally well-formed TOML-shaped JSON; the only
//! defect is semantic — `[host.aliases]` references a base bidder the
//! registry has never heard of. That must classify exactly like a
//! malformed blob: `RuntimeConfigError::MalformedHost` from the helper,
//! and the shared response mapper's fail-closed 500 on the wire, with
//! no bidder called.

mod bounded_store;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::bounded_store::bounded_over_get;
    use crate::config_funding;
    use crate::serving_catalog::passthrough_info;
    use async_trait::async_trait;
    use auctioneer_core::config::runtime::{RuntimeConfigError, resolve_runtime_config};
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{TestClockHandle, TestRngHandle};
    use auctioneer_core::test_support::config_store as funded_config;
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
    use edgezero_core::http::StatusCode;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    const STORE_ID: &str = "auctioneer_runtime";
    const WIDE_CAP: usize = 1 << 20;

    /// A well-formed, integrity-valid blob whose `[host.aliases]` names a
    /// base bidder that is not in this build's registry.
    struct UnknownAliasBaseStore;

    #[async_trait(?Send)]
    impl ConfigStore for UnknownAliasBaseStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            let envelope = BlobEnvelope::new(
                serde_json::json!({
                    "schema_version": 1_u64,
                    "host": {
                        "aliases": { "fake_totally_real": "fake_nonexistent_bidder" },
                    },
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

    /// The positive control: the same shape, but the base exists.
    struct KnownAliasBaseStore;

    #[async_trait(?Send)]
    impl ConfigStore for KnownAliasBaseStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            let envelope = BlobEnvelope::new(
                serde_json::json!({
                    "schema_version": 1_u64,
                    "host": {
                        "aliases": { "fake_generic_alias": passthrough_info().name },
                    },
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

    fn registry_of(store: Arc<dyn ConfigStore>) -> ConfigRegistry {
        StoreRegistry::single_id(
            STORE_ID.to_owned(),
            ConfigStoreBinding {
                default_key: STORE_ID.to_owned(),
                handle: ConfigStoreHandle::new(store),
            },
        )
    }

    #[derive(Default)]
    struct RecordingFanout {
        calls: Mutex<usize>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut seen = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
            *seen = seen.saturating_add(calls.len());
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    fn fixture_body() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": "auc-alias-validation",
            "tmax": 1000,
            "cur": ["USD"],
            "imp": [{
                "id": "imp-1",
                "banner": { "w": 300, "h": 250 },
                "ext": { "bidder": { "fake_bidder": {} } },
            }],
            "ext": { "prebid": { "aliases": { "fake_bidder": passthrough_info().name } } },
        }))
        .expect("fixture body serialises")
    }

    fn context_with(store: Arc<dyn ConfigStore>, fanout: Arc<RecordingFanout>) -> RequestContext {
        config_funding::install();
        let handler_fanout: Arc<dyn BidderFanout + Send + Sync> = fanout;
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(fixture_body()))
            .expect("request builds");
        request.extensions_mut().insert(registry_of(store));
        request
            .extensions_mut()
            .insert(TestFanoutHandle(handler_fanout));
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        funded_config::context(request)
    }

    #[test]
    fn runtime_host_alias_unknown_base_returns_malformed_host() {
        // Helper-level: the SDK exposes a fixed validation category and scope,
        // never the store-controlled alias or base name.
        let ctx = context_with(
            Arc::new(UnknownAliasBaseStore),
            Arc::new(RecordingFanout::default()),
        );
        let resolved = block_on(resolve_runtime_config(&ctx));
        match resolved {
            Err(RuntimeConfigError::MalformedHost { detail }) => {
                assert_eq!(
                    detail,
                    "config store extraction failed: Validation at runtime"
                );
                assert!(detail.len() < 512, "the diagnostic must remain bounded");
                for canary in ["fake_totally_real", "fake_nonexistent_bidder"] {
                    assert!(
                        !detail.contains(canary),
                        "a raw config name reached the diagnostic"
                    );
                }
            }
            other => panic!(
                "an unknown runtime alias base must classify as MalformedHost, got {other:?}"
            ),
        }

        // Wire-level: the shared response mapper's fail-closed 500,
        // with no bidder called.
        let fanout = Arc::new(RecordingFanout::default());
        let wire_ctx = context_with(
            Arc::new(UnknownAliasBaseStore),
            Arc::<RecordingFanout>::clone(&fanout),
        );
        block_on(async {
            let response = auction(wire_ctx).await.expect("the handler answers");
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            for value in response.headers().values() {
                let text = value.to_str().expect("fixture response header is text");
                for canary in ["fake_totally_real", "fake_nonexistent_bidder"] {
                    assert!(
                        !text.contains(canary),
                        "a raw config name reached a response header"
                    );
                }
            }
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            assert_eq!(
                String::from_utf8_lossy(&bytes),
                "Malformed host configuration"
            );
        });
        assert_eq!(
            *fanout.calls.lock().unwrap_or_else(PoisonError::into_inner),
            0,
            "no bidder may be called under a malformed host config",
        );
    }

    #[test]
    fn runtime_host_alias_known_base_is_accepted() {
        let ctx = context_with(
            Arc::new(KnownAliasBaseStore),
            Arc::new(RecordingFanout::default()),
        );
        let resolved = block_on(resolve_runtime_config(&ctx));
        let cfg = resolved.expect("a registry-known base must resolve cleanly");
        assert_eq!(
            cfg.host
                .aliases
                .as_ref()
                .and_then(|aliases| aliases.get("fake_generic_alias"))
                .map(String::as_str),
            Some(passthrough_info().name.as_str()),
        );
    }

    mod policy_envelope {
        use super::{STORE_ID, registry_of};
        use async_trait::async_trait;
        use auctioneer_core::config::SdkAuctioneerAppConfig;
        use auctioneer_core::test_support::config_store::runtime_registry;
        use edgezero_core::blob_envelope::BlobEnvelope;
        use edgezero_core::body::Body;
        use edgezero_core::config_store::{
            BoundedStoreRead, ConfigExtractionLimits, ConfigStore, ConfigStoreError,
            ConfigStoreHandle, ConfigValue,
        };
        use edgezero_core::context::RequestContext;
        use edgezero_core::error::{EdgeError, StoreExtractionReason};
        use edgezero_core::extractor::AppConfig;
        use edgezero_core::http::StatusCode;
        use edgezero_core::params::PathParams;
        use edgezero_core::time::{Deadline, MonotonicClock};
        use futures::executor::block_on;
        use http::Request;
        use serde_json::json;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::Duration;
        use validator::Validate as _;

        struct FakeBoundedPolicyStore {
            inner: ConfigStoreHandle,
            bounded_reads: AtomicUsize,
        }

        #[test]
        fn runtime_refuses_fetch_timeout_overrides_independently_of_sdk_duration_bounds() {
            let cases = [
                (0_u32, false),
                (1, true),
                (604_800_000, true),
                (604_800_001, false),
                (u32::MAX, false),
            ];
            assert_eq!(cases.len(), 5);
            let control: SdkAuctioneerAppConfig = serde_json::from_value(json!({
                "schema_version": 1_u64,
                "host": {"resource_limits": {}}
            }))
            .expect("the runtime policy control is valid");
            control.validate().unwrap();
            for (milliseconds, accepted) in cases {
                let rejected = serde_json::from_value::<SdkAuctioneerAppConfig>(json!({
                    "schema_version": 1_u64,
                    "host": {"resource_limits": {"max_config_fetch_ms": milliseconds}}
                }))
                .expect_err("an operator cannot change the fixed extraction timeout");
                assert!(rejected.to_string().contains("max_config_fetch_ms"));
                let sdk = ConfigExtractionLimits {
                    timeout: Duration::from_millis(u64::from(milliseconds)),
                    ..ConfigExtractionLimits::default()
                };
                assert_eq!(sdk.validate().is_ok(), accepted, "SDK: {milliseconds}");
            }
        }

        #[async_trait(?Send)]
        impl ConfigStore for FakeBoundedPolicyStore {
            async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
                panic!("policy extraction must use the bounded store API")
            }

            async fn get_bounded(
                &self,
                key: &str,
                clock: &MonotonicClock,
                deadline: Deadline,
                max_backend_bytes: u64,
                max_value_bytes: u64,
            ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
                self.bounded_reads.fetch_add(1, Ordering::SeqCst);
                assert_eq!(key, STORE_ID);
                assert_eq!(max_backend_bytes, 0x0100_0000);
                assert_eq!(max_value_bytes, 0x0080_0000);
                let remaining = deadline
                    .remaining_at(clock.now())
                    .expect("the SDK supplies a live extraction deadline");
                assert!(remaining <= ConfigExtractionLimits::default().timeout);

                let read = self
                    .inner
                    .get_bounded(key, clock, deadline, max_backend_bytes, max_value_bytes)
                    .await?;
                let value = read.value.as_ref().expect("the fixture has a policy blob");
                let envelope: BlobEnvelope = serde_json::from_slice(value.as_bytes())
                    .expect("the fixture is a valid SDK envelope");
                assert_eq!(envelope.version, 1);
                envelope
                    .verify()
                    .expect("each application-version row has a recomputed valid digest");
                Ok(read)
            }
        }

        #[test]
        fn application_version_is_required_under_valid_envelope() {
            let cases = [
                ("valid1", Some(json!(1_u64)), Ok(())),
                ("missing", None, Err(".")),
                ("wrong", Some(json!(2_u64)), Err("<redacted>")),
                ("null", Some(json!(null)), Err("<redacted>")),
            ];
            assert_eq!(cases.len(), 4);
            let mut accepted = 0_usize;
            let mut rejected = 0_usize;
            for (case, version, expected) in cases {
                let mut document = json!({
                    "schema_version": 1_u64,
                    "host": {
                        "status_response": "fake_policy_ready",
                    },
                    "accounts": {"fake_account": {"disabled": true}},
                });
                if let Some(application_version) = version {
                    document["schema_version"] = application_version;
                } else {
                    assert_eq!(
                        document
                            .as_object_mut()
                            .expect("the policy fixture is an object")
                            .remove("schema_version"),
                        Some(json!(1_u64)),
                    );
                }
                // Build after changing the application field; SDK version and
                // integrity remain valid independently in all four rows.
                let fixture = runtime_registry(&document);
                let store = Arc::new(FakeBoundedPolicyStore {
                    inner: fixture
                        .named(STORE_ID)
                        .expect("the shared fixture binds the runtime store")
                        .handle
                        .clone(),
                    bounded_reads: AtomicUsize::new(0),
                });
                let mut request = Request::builder()
                    .body(Body::empty())
                    .expect("the extraction request builds");
                request
                    .extensions_mut()
                    .insert(registry_of(Arc::<FakeBoundedPolicyStore>::clone(&store)));
                let ctx = RequestContext::new(request, PathParams::default());
                let result = block_on(AppConfig::<SdkAuctioneerAppConfig>::from_store(
                    &ctx, STORE_ID, None,
                ));
                assert_eq!(store.bounded_reads.load(Ordering::SeqCst), 1, "{case}");

                match expected {
                    Ok(()) => {
                        let SdkAuctioneerAppConfig(policy) = result.expect(case);
                        assert_eq!(
                            serde_json::to_value(policy.schema_version).unwrap(),
                            json!(1_u64)
                        );
                        assert_eq!(
                            policy.host.status_response.as_deref(),
                            Some("fake_policy_ready")
                        );
                        assert_eq!(policy.accounts["fake_account"].disabled, Some(true));
                        accepted += 1;
                    }
                    Err(expected_path) => {
                        let error = result.expect_err(case);
                        assert_eq!(error.status(), StatusCode::SERVICE_UNAVAILABLE, "{case}");
                        let EdgeError::StoreExtraction {
                            reason,
                            message,
                            field_path,
                        } = error
                        else {
                            panic!("{case}: expected an SDK store-extraction error");
                        };
                        assert_eq!(reason, StoreExtractionReason::Deserialization, "{case}");
                        assert_eq!(
                            message,
                            "typed app-config is out of date (wrong type or invalid value; value redacted)",
                            "{case}",
                        );
                        assert_eq!(field_path.as_deref(), Some(expected_path), "{case}");
                        rejected += 1;
                    }
                }
            }
            assert_eq!((accepted, rejected), (1, 3));
        }
    }
}

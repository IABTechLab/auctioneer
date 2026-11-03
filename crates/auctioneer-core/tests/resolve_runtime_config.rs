//! Runtime host-alias registry validation (Fanout-9).
//!
//! GDPR fanout's Task 18 rejects an unknown alias base in the BAKED
//! `[baked.host]` in `auctioneer.toml` at `build.rs` time, but a runtime overlay
//! arrives through the config store and never sees `build.rs`. Without a
//! registry check on that path, a pushed `[aliases]` naming a base this
//! build does not carry would resolve into an auction plan that can
//! never bid — a config error surfaced as silent no-bids instead of the
//! fail-loud 500 every other malformed-host shape produces.
//!
//! The store below returns a VALID envelope (integrity check passes)
//! whose config is structurally well-formed TOML-shaped JSON; the only
//! defect is semantic — `[aliases]` references a base bidder the
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
    use edgezero_core::params::PathParams;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    const STORE_ID: &str = "auctioneer_runtime";
    const WIDE_CAP: usize = 1 << 20;

    /// A well-formed, integrity-valid blob whose `[aliases]` names a
    /// base bidder that is not in this build's registry.
    struct UnknownAliasBaseStore;

    #[async_trait(?Send)]
    impl ConfigStore for UnknownAliasBaseStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            let envelope = BlobEnvelope::new(
                serde_json::json!({
                    "aliases": { "fake_totally_real": "fake_nonexistent_bidder" }
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
                    "aliases": { "fake_generic_alias": passthrough_info().name }
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
        RequestContext::new(request, PathParams::default())
    }

    #[test]
    fn runtime_host_alias_unknown_base_returns_malformed_host() {
        // Helper-level: the exact error class, with both names in the
        // detail so an operator can find the offending overlay line.
        let ctx = context_with(
            Arc::new(UnknownAliasBaseStore),
            Arc::new(RecordingFanout::default()),
        );
        let resolved = block_on(resolve_runtime_config(&ctx));
        match resolved {
            Err(RuntimeConfigError::MalformedHost { detail }) => {
                assert!(
                    detail.contains("fake_totally_real")
                        && detail.contains("fake_nonexistent_bidder"),
                    "detail must name the alias and its unknown base, got {detail:?}",
                );
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
            cfg.aliases
                .as_ref()
                .and_then(|aliases| aliases.get("fake_generic_alias"))
                .map(String::as_str),
            Some(passthrough_info().name.as_str()),
        );
    }
}

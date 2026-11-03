//! Request-bound credential reads; bidder implementations never access stores.

use async_trait::async_trait;
use auctioneer_bidder::bidder_settings::MAX_CREDENTIAL_REFERENCE_BYTES;
use auctioneer_runtime::MonotonicInstant;
use auctioneer_runtime::bidder_credentials::{
    BidderCredentialSource, BidderCredentials, CredentialUnavailable, MAX_BIDDER_CREDENTIAL_BYTES,
};
use edgezero_core::{
    context::RequestContext,
    store_registry::{BoundSecretStore, SecretRegistry},
    time::{Deadline, MonotonicClock},
};

const MAX_CREDENTIAL_STORE_NAME_BYTES: usize = 128;

pub(crate) struct CredentialLoader {
    store: Option<BoundSecretStore>,
    clock: MonotonicClock,
}

impl CredentialLoader {
    pub(crate) fn capture(ctx: &RequestContext) -> Self {
        Self {
            store: ctx
                .extensions()
                .get::<SecretRegistry>()
                .and_then(SecretRegistry::default_ref)
                .filter(|store| store.store_name().len() <= MAX_CREDENTIAL_STORE_NAME_BYTES)
                .cloned(),
            clock: ctx.monotonic_clock(),
        }
    }
}

#[async_trait(?Send)]
impl BidderCredentialSource for CredentialLoader {
    async fn load(
        &self,
        reference: &str,
        deadline: MonotonicInstant,
    ) -> Result<BidderCredentials, CredentialUnavailable> {
        if self.clock.now() >= deadline
            || reference.is_empty()
            || reference.len() > MAX_CREDENTIAL_REFERENCE_BYTES
            || reference.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(CredentialUnavailable);
        }
        let store = self.store.as_ref().ok_or(CredentialUnavailable)?;
        let allowance = u64::try_from(MAX_BIDDER_CREDENTIAL_BYTES)
            .map_err(|_overflow| CredentialUnavailable)?;
        let result = store
            .get_bytes_bounded(
                reference,
                &self.clock,
                Deadline::at_instant(deadline),
                allowance,
                allowance,
            )
            .await
            .map_err(|_error| CredentialUnavailable)?;
        if self.clock.now() >= deadline || result.backend_bytes > allowance {
            return Err(CredentialUnavailable);
        }
        let bytes = result.value.ok_or(CredentialUnavailable)?;
        BidderCredentials::parse(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use edgezero_core::{
        config_store::BoundedStoreRead,
        secret_store::{SecretError, SecretHandle, SecretStore},
        time::Deadline,
    };
    use futures::{executor::block_on, task::noop_waker};
    use serde_json::json;
    use std::{
        future::pending,
        mem::{size_of, size_of_val},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };

    struct Provider {
        reference: String,
        polls: AtomicUsize,
        completed: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
        pending: bool,
        missing: bool,
        backend_bytes: u64,
    }

    struct DropGuard(Arc<AtomicBool>);

    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[async_trait(?Send)]
    impl SecretStore for Provider {
        async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
            panic!("credential reads must use the bounded provider operation");
        }

        async fn get_bytes_bounded(
            &self,
            store_name: &str,
            key: &str,
            _: &MonotonicClock,
            _: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<Bytes>, SecretError> {
            assert_eq!((store_name, key), ("default", self.reference.as_str()));
            assert_eq!((max_backend_bytes, max_value_bytes), (4096, 4096));
            self.polls.fetch_add(1, Ordering::SeqCst);
            let _guard = DropGuard(Arc::clone(&self.dropped));
            if self.pending {
                pending::<()>().await;
            }
            self.completed.store(true, Ordering::SeqCst);
            let value = (!self.missing).then(|| {
                Bytes::from(
                    serde_json::to_vec(&json!({
                        "schema_version": 1_i32,
                        "username": "fake_user",
                        "password": "FAKE_PASSWORD_CANARY",
                        "tracker": "fake_tracker"
                    }))
                    .unwrap(),
                )
            });
            Ok(BoundedStoreRead {
                backend_bytes: self.backend_bytes,
                value,
            })
        }
    }

    fn provider(pending: bool, missing: bool, backend_bytes: u64) -> Arc<Provider> {
        Arc::new(Provider {
            reference: "fake_credentials".to_owned(),
            polls: AtomicUsize::new(0),
            completed: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicBool::new(false)),
            pending,
            missing,
            backend_bytes,
        })
    }

    fn loader(provider: &Arc<Provider>, clock: MonotonicClock) -> CredentialLoader {
        let concrete = Arc::clone(provider);
        let shared: Arc<dyn SecretStore> = concrete;
        CredentialLoader {
            store: Some(BoundSecretStore::new(
                SecretHandle::new(shared),
                "default".to_owned(),
            )),
            clock,
        }
    }

    #[test]
    fn credentials_are_loaded_as_one_bounded_snapshot_only_when_polled() {
        let provider = provider(false, false, 200);
        let clock = MonotonicClock::default();
        let reader = loader(&provider, clock.clone());
        let deadline = clock.now().checked_add(Duration::from_secs(30)).unwrap();
        let future = reader.load("fake_credentials", deadline);
        assert!(size_of::<CredentialLoader>() <= 128);
        assert!(size_of_val(future.as_ref().get_ref()) <= 4096);
        assert_eq!(provider.polls.load(Ordering::SeqCst), 0);
        let credentials = block_on(future).unwrap();
        assert_eq!(provider.polls.load(Ordering::SeqCst), 1);
        assert_eq!(credentials.username(), "fake_user");
        assert_eq!(credentials.tracker(), "fake_tracker");
        assert_eq!(
            credentials.authorization(),
            "Basic ZmFrZV91c2VyOkZBS0VfUEFTU1dPUkRfQ0FOQVJZ"
        );
        assert!(provider.dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn expired_credential_read_never_polls_the_provider() {
        let provider = provider(false, false, 200);
        let clock = MonotonicClock::default();
        let reader = loader(&provider, clock.clone());
        assert!(matches!(
            block_on(reader.load("fake_credentials", clock.now())),
            Err(CredentialUnavailable)
        ));
        assert_eq!(provider.polls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn credential_expiry_backend_cap_and_missing_value_refuse_independently() {
        let anchor = MonotonicInstant::now();
        for (expire, backend_bytes, missing, accepted) in [
            (false, 200, false, true),
            (true, 200, false, false),
            (false, 4097, false, false),
            (false, 200, true, false),
        ] {
            let provider = provider(false, missing, backend_bytes);
            let completed = Arc::clone(&provider.completed);
            let clock = MonotonicClock::new(move || {
                if expire && completed.load(Ordering::SeqCst) {
                    anchor.checked_add(Duration::from_secs(2)).unwrap()
                } else {
                    anchor
                }
            });
            let reader = loader(&provider, clock);
            let result = block_on(reader.load(
                "fake_credentials",
                anchor.checked_add(Duration::from_secs(1)).unwrap(),
            ));
            assert_eq!(
                result.is_ok(),
                accepted,
                "expire={expire}, backend={backend_bytes}, missing={missing}"
            );
            assert_eq!(provider.polls.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn cancellation_drops_the_pending_credential_provider_future() {
        let provider = provider(true, false, 200);
        let clock = MonotonicClock::default();
        let reader = loader(&provider, clock.clone());
        let mut future = reader.load(
            "fake_credentials",
            clock.now().checked_add(Duration::from_secs(30)).unwrap(),
        );
        assert!(matches!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(&noop_waker())),
            Poll::Pending
        ));
        assert_eq!(provider.polls.load(Ordering::SeqCst), 1);
        assert!(!provider.dropped.load(Ordering::SeqCst));
        drop(future);
        assert!(provider.dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn missing_credential_store_returns_only_the_fixed_error() {
        let clock = MonotonicClock::default();
        let reader = CredentialLoader {
            store: None,
            clock: clock.clone(),
        };
        let result = block_on(reader.load(
            "FAKE_SECRET_REFERENCE_CANARY",
            clock.now().checked_add(Duration::from_secs(1)).unwrap(),
        ));
        let error = result.unwrap_err();
        assert_eq!(error.to_string(), "bidder credentials unavailable");
        assert!(!format!("{error:?}").contains("CANARY"));
    }

    #[test]
    fn credential_reference_limits_are_checked_before_any_provider_poll() {
        for (reference, accepted) in [
            ("f".repeat(127), true),
            ("f".repeat(128), true),
            ("f".repeat(129), false),
            (String::new(), false),
            ("fake\rreference".to_owned(), false),
            ("fake\0reference".to_owned(), false),
        ] {
            let mut provider = provider(false, false, 200);
            Arc::get_mut(&mut provider)
                .unwrap()
                .reference
                .clone_from(&reference);
            let clock = MonotonicClock::default();
            let reader = loader(&provider, clock.clone());
            let result = block_on(reader.load(
                &reference,
                clock.now().checked_add(Duration::from_secs(30)).unwrap(),
            ));
            assert_eq!(
                result.is_ok(),
                accepted,
                "reference length {}",
                reference.len()
            );
            assert_eq!(provider.polls.load(Ordering::SeqCst), usize::from(accepted));
        }
    }

    #[test]
    fn captured_credential_binding_is_bounded_without_polling_or_legacy_fallback() {
        use edgezero_core::{
            Body,
            http::request_builder,
            params::PathParams,
            store_registry::{SecretRegistry, StoreRegistry},
        };
        use std::collections::BTreeMap;

        for (name, accepted) in [
            ("s".repeat(127), true),
            ("s".repeat(128), true),
            ("s".repeat(129), false),
        ] {
            let provider = provider(false, false, 200);
            let shared: Arc<dyn SecretStore> = Arc::<Provider>::clone(&provider);
            let handle = SecretHandle::new(shared);
            let registry: SecretRegistry = StoreRegistry::new(
                BTreeMap::from([(
                    "default".into(),
                    BoundSecretStore::new(handle, name.clone()),
                )]),
                "default".into(),
            );
            let mut request = request_builder()
                .uri("/auction")
                .body(Body::empty())
                .unwrap();
            request.extensions_mut().insert(registry);
            let context = RequestContext::new(request, PathParams::default());
            let reader = CredentialLoader::capture(&context);
            assert_eq!(
                reader.store.is_some(),
                accepted,
                "binding bytes={}",
                name.len()
            );
            assert_eq!(provider.polls.load(Ordering::SeqCst), 0);
            if let Some(store) = reader.store {
                assert_eq!(store.store_name(), name);
            }
        }
        let provider = provider(false, false, 200);
        let shared: Arc<dyn SecretStore> = Arc::<Provider>::clone(&provider);
        let mut request = request_builder()
            .uri("/auction")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(SecretHandle::new(shared));
        let context = RequestContext::new(request, PathParams::default());
        assert!(CredentialLoader::capture(&context).store.is_none());
        assert_eq!(provider.polls.load(Ordering::SeqCst), 0);
    }
}

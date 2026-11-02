//! One bounded atomic secret read, polled only at a sync route's UID-read stage.

use edgezero_core::context::RequestContext;
use edgezero_core::{store_registry::BoundSecretStore, time::MonotonicClock};
use std::future::Future;

use crate::{
    handlers::egress_deadline::EgressDeadline,
    usersync::signed::{CookieKeyring, MAX_KEYRING_BYTES},
};

pub const KEYRING_SECRET_NAME: &str = "usersync_cookie_keyring";

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("user sync signing unavailable")]
pub struct SigningUnavailable;

pub type KeyringResult = Result<CookieKeyring, SigningUnavailable>;

/// Capture request-bound handles without polling the provider.
///
/// # Errors
/// The future returns a fixed unavailable error for any bounded-read/keyring fault.
pub fn load(
    ctx: &RequestContext,
    deadline: EgressDeadline,
) -> impl Future<Output = KeyringResult> + use<> {
    let clock = ctx.monotonic_clock();
    let store = ctx.secret_store_default();
    read_keyring(store, clock, deadline)
}

async fn read_keyring(
    binding: Option<BoundSecretStore>,
    clock: MonotonicClock,
    deadline: EgressDeadline,
) -> KeyringResult {
    if deadline.is_expired_at(clock.now()) {
        return Err(SigningUnavailable);
    }
    let store = binding.ok_or(SigningUnavailable)?;
    let allowance = u64::try_from(MAX_KEYRING_BYTES).map_err(|_error| SigningUnavailable)?;
    let result = store
        .get_bytes_bounded(
            KEYRING_SECRET_NAME,
            &clock,
            deadline.deadline(),
            allowance,
            allowance,
        )
        .await
        .map_err(|_error| SigningUnavailable)?;
    if deadline.is_expired_at(clock.now()) || result.backend_bytes > allowance {
        return Err(SigningUnavailable);
    }
    let bytes = result.value.ok_or(SigningUnavailable)?;
    CookieKeyring::parse(&bytes).map_err(|_error| SigningUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use bytes::Bytes;
    use edgezero_core::{
        config_store::BoundedStoreRead,
        secret_store::{SecretError, SecretHandle, SecretStore},
        time::{Deadline, MonotonicInstant},
    };
    use futures::{executor::block_on, task::noop_waker};
    use serde_json::json;
    use std::future::pending;
    use std::{
        pin::pin,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };

    struct Provider {
        calls: AtomicUsize,
        expired: Arc<AtomicBool>,
        dropped: Arc<AtomicBool>,
        pending: bool,
        backend_bytes: u64,
    }

    struct DropGuard(Arc<AtomicBool>);
    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for DropGuard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[async_trait(?Send)]
    impl SecretStore for Provider {
        async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
            panic!("loader must use the bounded provider operation");
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
            assert_eq!((store_name, key), ("default", KEYRING_SECRET_NAME));
            assert_eq!((max_backend_bytes, max_value_bytes), (4096, 4096));
            self.calls.fetch_add(1, Ordering::SeqCst);
            let _guard = DropGuard(Arc::clone(&self.dropped));
            if self.pending {
                pending::<()>().await;
            }
            self.expired.store(true, Ordering::SeqCst);
            Ok(BoundedStoreRead { backend_bytes: self.backend_bytes, value: Some(Bytes::from(serde_json::to_vec(&json!({
                "active":"test", "keys":[{"id":"test","key":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"}]
            })).unwrap())) })
        }
    }

    fn provider(pending: bool, backend_bytes: u64) -> Arc<Provider> {
        Arc::new(Provider {
            calls: AtomicUsize::new(0),
            expired: Arc::new(AtomicBool::new(false)),
            dropped: Arc::new(AtomicBool::new(false)),
            pending,
            backend_bytes,
        })
    }
    fn binding(provider: &Arc<Provider>) -> BoundSecretStore {
        let concrete = Arc::clone(provider);
        let shared: Arc<dyn SecretStore> = concrete;
        BoundSecretStore::new(SecretHandle::new(shared), "default".to_owned())
    }

    #[test]
    fn secret_read_is_lazy_and_cancellation_drops_the_pending_provider_future() {
        let provider = provider(true, 100);
        let clock = MonotonicClock::default();
        let deadline = EgressDeadline::new(clock.now(), Duration::from_secs(30));
        {
            let future = read_keyring(Some(binding(&provider)), clock, deadline);
            assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
            let mut pinned = pin!(future);
            assert!(matches!(
                pinned
                    .as_mut()
                    .poll(&mut Context::from_waker(&noop_waker())),
                Poll::Pending
            ));
            assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
            assert!(!provider.dropped.load(Ordering::SeqCst));
        };
        assert!(provider.dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn expired_read_never_polls_the_provider() {
        let provider = provider(false, 100);
        let clock = MonotonicClock::default();
        let deadline = EgressDeadline::new(clock.now(), Duration::ZERO);
        assert!(matches!(
            block_on(read_keyring(Some(binding(&provider)), clock, deadline)),
            Err(SigningUnavailable)
        ));
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn deadline_recheck_and_backend_allowance_fail_independently_after_read() {
        let anchor = MonotonicInstant::now();
        for (advance_clock, backend_bytes) in [(true, 100), (false, 4097), (false, 100)] {
            let provider = provider(false, backend_bytes);
            let expired = Arc::clone(&provider.expired);
            let clock = MonotonicClock::new(move || {
                if advance_clock && expired.load(Ordering::SeqCst) {
                    anchor.checked_add(Duration::from_secs(2)).unwrap()
                } else {
                    anchor
                }
            });
            let result = block_on(read_keyring(
                Some(binding(&provider)),
                clock,
                EgressDeadline::new(anchor, Duration::from_secs(1)),
            ));
            assert_eq!(result.is_ok(), !advance_clock && backend_bytes <= 4096);
            assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        }
    }
}

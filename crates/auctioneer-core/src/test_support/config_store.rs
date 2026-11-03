//! Fixed runtime-config fixtures; no boot state, policy or expected limits.

use auctioneer_config::types::{CooperativeConfig, HostConfig, UserSyncConfig};
use edgezero_core::blob_envelope::BlobEnvelope;
use edgezero_core::config_store::{
    BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
};
use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
use edgezero_core::time::{Deadline, MonotonicClock};
use serde::Serialize;
use std::sync::Arc;

/// Explicit synthetic service for tests whose subject needs configured sync.
/// Production host defaults deliberately have no external sync origin.
#[must_use]
pub fn configured_sync_host() -> HostConfig {
    HostConfig {
        user_sync: Some(UserSyncConfig {
            external_url: "https://sync.example.test".to_owned(),
            redirect_url: None,
            cooperative: CooperativeConfig::default(),
            priority_groups: Vec::new(),
            allowed_origins: Vec::new(),
        }),
        ..HostConfig::default()
    }
}

/// Build the real loader's binding with a deterministic integrity envelope.
/// The caller owns the config and installs the registry on its own request.
///
/// # Panics
/// Panics if the fixture cannot be serialized as JSON.
pub fn runtime_registry<C: Serialize>(config: &C) -> ConfigRegistry {
    let data = serde_json::to_value(config).expect("fixture config serialises");
    let envelope = BlobEnvelope::new(data, "1970-01-01T00:00:00Z".to_owned());
    let blob = serde_json::to_string(&envelope).expect("fixture envelope serialises");
    StoreRegistry::single_id(
        "auctioneer_runtime".to_owned(),
        ConfigStoreBinding {
            default_key: "auctioneer_runtime".to_owned(),
            handle: ConfigStoreHandle::new(Arc::new(FixedStore(blob.into()))),
        },
    )
}

struct FixedStore(ConfigValue);

#[async_trait::async_trait(?Send)]
impl ConfigStore for FixedStore {
    async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
        Ok(Some(self.0.clone()))
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

/// Test stores' fallback semantics: check the caller's clock before and after
/// `get`, then apply independent backend/value caps. This does not claim
/// native provider bounds or cancellation of an unbounded, pending `get`.
///
/// # Errors
/// Propagates store errors; returns `DeadlineExceeded` at expiry and
/// `ValueTooLarge` when either byte cap is exceeded.
pub async fn bounded_over_get<S>(
    store: &S,
    key: &str,
    clock: &MonotonicClock,
    deadline: Deadline,
    max_backend_bytes: u64,
    max_value_bytes: u64,
) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError>
where
    S: ConfigStore + ?Sized,
{
    if deadline.is_expired_at(clock.now()) {
        return Err(ConfigStoreError::DeadlineExceeded);
    }
    let value = store.get(key).await?;
    if deadline.is_expired_at(clock.now()) {
        return Err(ConfigStoreError::DeadlineExceeded);
    }
    let backend_bytes = value.as_ref().map_or(Ok(0_u64), |stored_value| {
        u64::try_from(stored_value.len()).map_err(|_length_error| ConfigStoreError::ValueTooLarge)
    })?;
    if backend_bytes > max_backend_bytes || backend_bytes > max_value_bytes {
        return Err(ConfigStoreError::ValueTooLarge);
    }
    Ok(BoundedStoreRead {
        backend_bytes,
        value,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use edgezero_core::time::MonotonicInstant;
    use futures::executor::block_on;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn runtime_binding_carries_the_callers_config_in_a_verified_envelope() {
        let config = json!({"accounts": {"fake_account": {"disabled": true}}});
        let registry = runtime_registry(&config);
        assert!(registry.named("not_the_runtime_store").is_none());
        let binding = registry.named("auctioneer_runtime").unwrap();
        assert_eq!(binding.default_key, "auctioneer_runtime");
        let stored = block_on(binding.handle.get("auctioneer_runtime"))
            .unwrap()
            .unwrap();
        let envelope: BlobEnvelope = serde_json::from_slice(stored.as_bytes()).unwrap();
        envelope.verify().unwrap();
        assert_eq!(envelope.data, config);
        assert_eq!(envelope.generated_at, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn fixed_store_enforces_each_byte_cap_at_and_over_the_boundary() {
        let now = MonotonicInstant::now();
        let clock = MonotonicClock::new(move || now);
        let deadline = clock.deadline_after(Duration::from_secs(1));
        let store = FixedStore("data".into());
        for (backend_cap, value_cap) in [(4, 4), (3, 4), (4, 3)] {
            let read = block_on(store.get_bounded("key", &clock, deadline, backend_cap, value_cap));
            if (backend_cap, value_cap) == (4, 4) {
                let bounded_read = read.unwrap();
                assert_eq!(bounded_read.backend_bytes, 4);
                assert_eq!(bounded_read.value.unwrap().as_bytes(), b"data");
            } else {
                assert!(matches!(read, Err(ConfigStoreError::ValueTooLarge)));
            }
        }
    }

    #[test]
    fn fixed_store_checks_expiry_before_and_after_the_read() {
        let now = MonotonicInstant::now();
        let expiry = now + Duration::from_secs(1);
        for expires_before_read in [true, false] {
            let calls = Arc::new(AtomicUsize::new(0));
            let observed = Arc::clone(&calls);
            let clock = MonotonicClock::new(move || {
                if observed.fetch_add(1, Ordering::Relaxed) == 0 && !expires_before_read {
                    now
                } else {
                    expiry
                }
            });
            let result = block_on(FixedStore("data".into()).get_bounded(
                "key",
                &clock,
                Deadline::at_instant(expiry),
                4,
                4,
            ));
            assert!(matches!(result, Err(ConfigStoreError::DeadlineExceeded)));
            assert_eq!(
                calls.load(Ordering::Relaxed),
                if expires_before_read { 1 } else { 2 }
            );
        }
    }
}

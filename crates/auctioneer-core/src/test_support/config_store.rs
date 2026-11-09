//! Fixed runtime-config fixtures and SDK-captured request contexts.

use crate::configure::configure_app;
use crate::ingress::AUCTION_CLASS;
use auctioneer_config_data::types::{CooperativeConfig, HostConfig, UserSyncConfig};
use edgezero_core::app::App;
use edgezero_core::blob_envelope::BlobEnvelope;
use edgezero_core::body::Body;
use edgezero_core::config_store::{
    BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
};
use edgezero_core::response_egress::ResponseEgressEnvelope;
use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
use edgezero_core::time::{Deadline, MonotonicClock};
use edgezero_core::{
    context::RequestContext,
    error::EdgeError,
    http::{Request, Response},
    ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting},
    router::RouterService,
};
use futures::{
    FutureExt as _,
    future::{Ready, ready},
};
use serde::Serialize;
use std::cell::RefCell;
use std::sync::Arc;

thread_local! {
    static CAPTURED: RefCell<Option<RequestContext>> = const { RefCell::new(None) };
}

/// Capture production extraction limits without installing Auctioneer admission.
/// Raw constructors remain available to deliberately unfunded test controls.
///
/// # Panics
/// Panics if configured SDK dispatch cannot capture the untouched request.
#[must_use]
pub fn context(request: Request) -> RequestContext {
    let route = request.uri().path().to_owned();
    context_for_route(request, &route, MonotonicClock::default())
}

/// Preserve explicit route parameters and clock injection through real SDK dispatch.
///
/// # Panics
/// Panics if the route does not match or capture needs body/provider work.
#[must_use]
pub fn context_for_route(request: Request, route: &str, clock: MonotonicClock) -> RequestContext {
    CAPTURED.with(|slot| assert!(slot.borrow().is_none(), "no nested context capture"));
    let mut configured = App::new(RouterService::builder().build());
    configure_app(&mut configured)
        .expect("production extraction limits must configure for capture");
    let limits = configured.config_extraction_limits();
    let router = RouterService::builder()
        .route(route, request.method().clone(), capture_context)
        .build();
    let mut app = App::new(router);
    app.set_config_extraction_limits(limits)
        .expect("configured extraction limits must be accepted by the capture app");
    app.set_monotonic_clock(clock);
    let (ctx, envelope) = capture_owner(&app, request);
    drop(envelope);
    ctx
}

/// Capture an admitted auction context with its original terminal-egress owner.
///
/// Consume the context through the handler, replace `*envelope.response_mut()`
/// with its response, and retain the envelope until terminal release. Admission
/// comes from the existing production gates, so callers must install boot first.
///
/// # Panics
/// Panics if admission refuses, the route does not match, or capture needs
/// body/provider work.
#[must_use]
pub fn auction_context_for_route(
    request: Request,
    route: &str,
    clock: MonotonicClock,
) -> (RequestContext, ResponseEgressEnvelope) {
    CAPTURED.with(|slot| assert!(slot.borrow().is_none(), "no nested context capture"));
    let router = RouterService::builder()
        .route_with_class(
            route,
            request.method().clone(),
            AUCTION_CLASS,
            capture_context,
        )
        .build();
    let mut app = App::new(router);
    configure_app(&mut app).expect("production admission and extraction limits must configure");
    app.set_monotonic_clock(clock);
    capture_owner(&app, request)
}

fn capture_context(ctx: RequestContext) -> Ready<Result<Response, EdgeError>> {
    CAPTURED.with(|slot| {
        assert!(
            slot.borrow_mut().replace(ctx).is_none(),
            "the capture route must run exactly once"
        );
    });
    ready(Ok(Response::new(Body::empty())))
}

fn capture_owner(app: &App, request: Request) -> (RequestContext, ResponseEgressEnvelope) {
    let limits = app.config_extraction_limits();
    // Capture is synchronous and never polls the body, so this also works inside
    // a fixture's existing executor without entering a second executor.
    let outcome = app
        .dispatch_ingress(
            request,
            app.monotonic_now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        )
        .now_or_never()
        .expect("fixture context capture must not wait for body or store work")
        .expect("the capture request must dispatch successfully");
    let mut envelope = match outcome {
        IngressDispatchOutcome::Response(envelope) => Ok(*envelope),
        IngressDispatchOutcome::Aborted => Err("fixture context capture was aborted"),
        _ => Err("fixture context capture returned an unknown disposition"),
    }
    .expect("fixture context capture must be admitted");
    assert_eq!(
        envelope.response_mut().status(),
        200,
        "the capture route must complete successfully"
    );
    let ctx = CAPTURED.with(|slot| {
        slot.borrow_mut()
            .take()
            .expect("the capture route must retain its routed request context")
    });
    assert_eq!(
        ctx.config_extraction_limits(),
        limits,
        "the captured context must retain the complete configured extraction tuple"
    );
    (ctx, envelope)
}

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
        let config = json!({
            "schema_version": 1_u64,
            "accounts": {"fake_account": {"disabled": true}},
        });
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

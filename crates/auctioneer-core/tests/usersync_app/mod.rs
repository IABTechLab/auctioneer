//! Real-App dispatch with a request-bound, replaceable runtime config store.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "fixture setup and unexpected dispatch outcomes must fail the test"
)]
#![expect(
    clippy::allow_attributes,
    reason = "shared fixture methods are unused in some integration targets; expect(dead_code) would be unfulfilled in others"
)]

use crate::bounded_store;
use crate::fanout_limits as envelope;

use std::{
    mem,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};

use auctioneer_core::{
    config::AuctioneerAppConfig,
    handlers::request_disclosure_audit::construction_costs as core_construction_costs,
    host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, bootstrap_adapter_with_envelope,
    },
};
use auctioneer_runtime::budget::{ASSEMBLY_FACTOR, derive_aux_work_for_profile};
use auctioneer_runtime::{
    FakeClock, ImmediateResolveTimer, NoopDecodeCooperator, TargetPlatformConstants,
};
use edgezero_core::{
    app::App,
    blob_envelope::BlobEnvelope,
    body::Body,
    config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
    },
    context::RequestContext,
    http::{HeaderName, HeaderValue, Method, Request, Response, request_builder},
    ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting},
    params::PathParams,
    response_egress::ResponseEgressEnvelope,
    secret_store::{SecretError, SecretHandle, SecretStore},
    store_registry::{
        BoundSecretStore, ConfigRegistry, ConfigStoreBinding, SecretRegistry, StoreRegistry,
    },
    time::{Deadline, MonotonicClock, MonotonicInstant},
};
use futures::executor::block_on;
use time::{OffsetDateTime, macros::datetime};

pub const TEST_NOW: OffsetDateTime = datetime!(2026-09-30 12:00 UTC);
pub const STORE_ERROR_CANARY: &str = "synthetic-backend-secret-m5";

enum StoreValue {
    Blob(Option<ConfigValue>),
    Unavailable,
}

struct StoreState {
    current: StoreValue,
    after_next_read: Option<StoreValue>,
}

pub struct TestStore {
    state: Mutex<StoreState>,
    reads: AtomicUsize,
}

impl TestStore {
    fn new(source: &str) -> Self {
        Self {
            state: Mutex::new(StoreState {
                current: StoreValue::Blob(Some(blob(source).into())),
                after_next_read: None,
            }),
            reads: AtomicUsize::new(0),
        }
    }

    pub fn set_config(&self, source: &str) {
        self.set_blob(Some(blob(source)));
    }

    pub fn set_blob(&self, raw: Option<String>) {
        let mut state = self.state.lock().expect("store lock");
        state.current = StoreValue::Blob(raw.map(ConfigValue::from));
        state.after_next_read = None;
    }

    pub fn replace_after_next_read(&self, source: &str) {
        self.state.lock().expect("store lock").after_next_read =
            Some(StoreValue::Blob(Some(blob(source).into())));
    }

    pub fn set_unavailable(&self) {
        self.state.lock().expect("store lock").current = StoreValue::Unavailable;
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait(?Send)]
impl ConfigStore for TestStore {
    async fn get(&self, key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
        assert_eq!(
            key, "auctioneer_runtime",
            "the configured binding selects the key"
        );
        self.reads.fetch_add(1, Ordering::SeqCst);
        let mut state = self.state.lock().expect("store lock");
        let result = match &state.current {
            StoreValue::Blob(value) => Ok(value.clone()),
            StoreValue::Unavailable => Err(ConfigStoreError::Unavailable {
                message: format!("test config store unavailable: token={STORE_ERROR_CANARY}"),
            }),
        };
        if let Some(next) = state.after_next_read.take() {
            state.current = next;
        }
        result
    }

    async fn get_bounded(
        &self,
        key: &str,
        clock: &MonotonicClock,
        deadline: Deadline,
        max_backend_bytes: u64,
        max_value_bytes: u64,
    ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
        bounded_store::bounded_over_get(
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

fn blob(source: &str) -> String {
    let config: AuctioneerAppConfig = toml::from_str(source).expect("fixture config parses");
    let data = serde_json::to_value(config).expect("fixture config serializes");
    serde_json::to_string(&BlobEnvelope::new(data, "1970-01-01T00:00:00Z".to_owned()))
        .expect("fixture envelope serializes")
}

fn bootstrap(final_response_cap: Option<u64>) {
    static BOOT: OnceLock<()> = OnceLock::new();
    BOOT.get_or_init(|| {
        let mut limits = envelope::limits();
        if let Some(cap) = final_response_cap {
            limits.final_response_bytes = cap;
            // This fixture emits no calls. Narrow decoded input too so the
            // installed response cap satisfies the ordinary assembly floor.
            limits.max_decoded_bytes = cap
                .div_euclid(u64::from(limits.max_outbound_requests))
                .div_euclid(ASSEMBLY_FACTOR);
        }
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 55_424,
            parser_header_knob_bytes: 55_425,
            max_request_target_bytes: 33 * 1_024,
            h1_max_buf_size_bytes: 0x0001_0000,
            h1_max_headers: 128,
            parser_max_header_entries: 1_732,
            pseudoheader_allowance_bytes: 34_816,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        let baked = auctioneer_runtime::serialized_json_bytes(&(
            auctioneer_config::host_default(),
            auctioneer_config::account_default(),
        ))
        .expect("baked config size");
        limits.aux_route_peak_bytes = derive_aux_work_for_profile(&limits, &target, baked)
            .expect("derived aux work")
            .peak_bytes();
        bootstrap_adapter_with_envelope(
            BootEnvelope {
                limits: &limits,
                target: &target,
                outbound_meta_bytes: auctioneer_bidder::outbound_meta_bytes(
                    limits.max_impressions_per_auction,
                ),
                construction_costs: core_construction_costs(),
                log_sink: &StderrBootLog,
                catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
            },
            || Arc::new(FakeClock::new(TEST_NOW)),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("boot envelope");
    });
}

pub struct TestApp {
    _serial: MutexGuard<'static, ()>,
    app: App,
    store: Arc<TestStore>,
    secrets: Arc<TestSecrets>,
}

pub struct TestSecrets {
    value: Mutex<Result<Option<bytes::Bytes>, ()>>,
    reads: AtomicUsize,
}

impl TestSecrets {
    fn new() -> Self {
        Self {
            value: Mutex::new(Ok(Some(bytes::Bytes::from(serde_json::to_vec(&serde_json::json!({
                "active":"test", "keys":[{"id":"test","key":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"}]
            })).expect("public synthetic secret"))))),
            reads: AtomicUsize::new(0),
        }
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub fn set(&self, value: Option<&[u8]>) {
        *self.value.lock().expect("secret lock") = Ok(value.map(bytes::Bytes::copy_from_slice));
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub fn unavailable(&self) {
        *self.value.lock().expect("secret lock") = Err(());
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait(?Send)]
impl SecretStore for TestSecrets {
    async fn get_bytes(
        &self,
        store_name: &str,
        key: &str,
    ) -> Result<Option<bytes::Bytes>, SecretError> {
        assert_eq!(
            store_name, "default",
            "handlers must use the default secret binding"
        );
        assert_eq!(
            key, "usersync_cookie_keyring",
            "handlers must use the atomic keyring"
        );
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.value
            .lock()
            .expect("secret lock")
            .clone()
            .map_err(|_error| SecretError::Unavailable)
    }

    async fn get_bytes_bounded(
        &self,
        store_name: &str,
        key: &str,
        clock: &MonotonicClock,
        deadline: Deadline,
        max_backend_bytes: u64,
        max_value_bytes: u64,
    ) -> Result<BoundedStoreRead<bytes::Bytes>, SecretError> {
        if deadline.is_expired_at(clock.now()) {
            return Err(SecretError::DeadlineExceeded);
        }
        let value = self.get_bytes(store_name, key).await?;
        let backend_bytes = value.as_ref().map_or(0, |bytes| {
            u64::try_from(bytes.len()).expect("fixture byte length fits u64")
        });
        if deadline.is_expired_at(clock.now()) {
            return Err(SecretError::DeadlineExceeded);
        }
        if backend_bytes > max_backend_bytes || backend_bytes > max_value_bytes {
            return Err(SecretError::ValueTooLarge);
        }
        Ok(BoundedStoreRead {
            backend_bytes,
            value,
        })
    }
}

impl TestApp {
    pub fn new(config: &str) -> Self {
        Self::new_inner(config, None)
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub fn with_final_response_cap(config: &str, cap: u64) -> Self {
        Self::new_inner(config, Some(cap))
    }

    fn new_inner(config: &str, final_response_cap: Option<u64>) -> Self {
        static SERIAL: Mutex<()> = Mutex::new(());
        let serial = SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        bootstrap(final_response_cap);
        Self {
            _serial: serial,
            app: App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("manifest builds"),
            store: Arc::new(TestStore::new(config)),
            secrets: Arc::new(TestSecrets::new()),
        }
    }

    #[must_use]
    pub fn store(&self) -> &TestStore {
        &self.store
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub fn secrets(&self) -> &TestSecrets {
        &self.secrets
    }

    pub fn dispatch(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Body,
    ) -> Response {
        self.dispatch_with(method, uri, headers, body, |_| {})
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub fn dispatch_with<F: FnOnce(&mut Request)>(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Body,
        configure: F,
    ) -> Response {
        block_on(self.dispatch_with_async(method, uri, headers, body, configure))
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub async fn dispatch_with_async<F: FnOnce(&mut Request)>(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Body,
        configure: F,
    ) -> Response {
        let mut envelope = self
            .dispatch_envelope_with(method, uri, headers, body, configure)
            .await;
        mem::replace(envelope.response_mut(), Response::new(Body::empty()))
    }

    #[allow(
        dead_code,
        reason = "shared fixture methods are used by distinct integration targets"
    )]
    pub async fn dispatch_envelope_with<F: FnOnce(&mut Request)>(
        &self,
        method: Method,
        uri: &str,
        headers: &[(&str, &str)],
        body: Body,
        configure: F,
    ) -> ResponseEgressEnvelope {
        let mut request: Request = request_builder()
            .method(method)
            .uri(uri)
            .body(body)
            .expect("fixture request builds");
        for (name, value) in headers {
            request.headers_mut().append(
                HeaderName::from_bytes(name.as_bytes()).expect("header name"),
                HeaderValue::from_str(value).expect("header value"),
            );
        }
        let concrete_store = Arc::clone(&self.store);
        let store: Arc<dyn ConfigStore> = concrete_store;
        let registry: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(store),
            },
        );
        request.extensions_mut().insert(registry);
        let concrete_secrets = Arc::clone(&self.secrets);
        let secrets: Arc<dyn SecretStore> = concrete_secrets;
        let secret_registry: SecretRegistry = StoreRegistry::single_id(
            "default".to_owned(),
            BoundSecretStore::new(SecretHandle::new(secrets), "default".to_owned()),
        );
        request.extensions_mut().insert(secret_registry);
        configure(&mut request);
        let dispatch_request = RequestContext::new(request, PathParams::default())
            .into_request()
            .expect("fixture context request");
        let outcome = self
            .app
            .dispatch_ingress(
                dispatch_request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            )
            .await
            .expect("App dispatches");
        let IngressDispatchOutcome::Response(envelope) = outcome else {
            panic!("request must receive a response");
        };
        *envelope
    }
}

pub fn body(response: &Response) -> &[u8] {
    response.body().as_bytes().expect("buffered response body")
}

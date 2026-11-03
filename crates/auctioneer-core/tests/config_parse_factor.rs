//! Peak live allocations through the production runtime-config resolver,
//! SDK raw-envelope extraction/integrity/typed conversion, and host/account
//! default clones. Fixtures and provider-resident payloads are constructed
//! before measurement. Request-time reads clone shared `ConfigValue` handles;
//! extraction and resolver allocations remain inside the measurement window.
//!
//! Funding is the production runtime-config term: raw envelope bytes,
//! envelope bytes times `CONFIG_ENVELOPE_VALUE_FACTOR`, blob bytes times
//! `CONFIG_PARSE_FACTOR`, and the independent `CONFIG_NODE_RETAINED_BYTES` term.
//! Cumulative allocations are diagnostic only. Direct TOML retained-byte
//! coverage remains a separate, narrower claim.
//! Asymmetric installed-cap cases check funding attribution and each byte
//! boundary independently; a combined peak is not a per-phase allocation proof.

/// Test-scoped allocator with per-thread live, peak and cumulative counters.
#[cfg(test)]
mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the config parse-factor bound"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::marker::PhantomData;
    use std::ptr::null_mut;
    use std::rc::Rc;

    thread_local! {
        static ALLOCATED: Cell<u64> = const { Cell::new(0) };
        static LIVE: Cell<i64> = const { Cell::new(0) };
        static PEAK: Cell<i64> = const { Cell::new(0) };
        static MEASURING: Cell<bool> = const { Cell::new(false) };
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Stats {
        pub peak: u64,
        pub retained: u64,
        pub cumulative: u64,
    }

    /// Kept on the measuring thread even while its local executor polls async work.
    pub struct Measurement {
        baseline: i64,
        allocated_before: u64,
        _thread_local: PhantomData<Rc<()>>,
    }

    impl Measurement {
        pub fn start() -> Self {
            MEASURING.with(|active| assert!(!active.replace(true), "no nested measurement"));
            let baseline = live_on_this_thread();
            PEAK.with(|peak| peak.set(baseline));
            Self {
                baseline,
                allocated_before: allocated_on_this_thread(),
                _thread_local: PhantomData,
            }
        }

        pub fn snapshot(&self) -> Stats {
            Stats {
                peak: PEAK.with(|peak| live_delta(peak.get(), self.baseline)),
                retained: live_delta(live_on_this_thread(), self.baseline),
                cumulative: allocated_on_this_thread()
                    .checked_sub(self.allocated_before)
                    .expect("cumulative allocation cannot decrease"),
            }
        }

        pub fn finish(self) -> Stats {
            self.snapshot()
        }
    }

    fn live_delta(current: i64, baseline: i64) -> u64 {
        let delta = current.checked_sub(baseline).expect("byte deltas fit i64");
        u64::try_from(delta).expect("measured work must not free pre-existing inputs")
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Measurement {
        fn drop(&mut self) {
            MEASURING.with(|active| active.set(false));
        }
    }

    pub fn allocated_on_this_thread() -> u64 {
        ALLOCATED.try_with(Cell::get).unwrap_or(0)
    }

    fn live_on_this_thread() -> i64 {
        LIVE.try_with(Cell::get).unwrap_or(0)
    }

    fn live_add(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
            let _outcome = MEASURING.try_with(|active| {
                if active.get() {
                    let _outcome = PEAK.try_with(|peak| peak.set(peak.get().max(cell.get())));
                }
            });
        });
    }

    fn live_sub(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_sub(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    fn count(bytes: usize) {
        // TLS teardown is outside the measurement window.
        let _outcome = ALLOCATED.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX)),
            );
        });
    }

    fn record_allocation(ptr: *mut u8, bytes: usize) {
        if !ptr.is_null() {
            count(bytes);
            live_add(bytes);
        }
    }

    fn record_reallocation(ptr: *mut u8, old_size: usize, new_size: usize) {
        if !ptr.is_null() {
            if new_size >= old_size {
                let growth = new_size.saturating_sub(old_size);
                count(growth);
                live_add(growth);
            } else {
                live_sub(old_size.saturating_sub(new_size));
            }
        }
    }

    pub fn failed_operation_stats() -> Stats {
        let measurement = Measurement::start();
        // Exercise failure accounting without asking System for an OOM-sized block.
        record_allocation(null_mut(), 1_024);
        record_reallocation(null_mut(), 1_024, 2_048);
        measurement.finish()
    }

    pub struct CountingAllocator;

    pub fn reallocation_stats() -> Stats {
        let measurement = Measurement::start();
        let layout = Layout::from_size_align(1_024, 8).expect("valid layout");
        // SAFETY: layout is valid and nonzero.
        let allocated = unsafe { CountingAllocator.alloc_zeroed(layout) };
        assert!(!allocated.is_null(), "small allocation succeeds");
        // SAFETY: allocated points to at least 1,024 initialized bytes.
        let first_byte = unsafe { *allocated };
        assert_eq!(first_byte, 0, "alloc_zeroed delegates zeroing to System");
        // SAFETY: allocated came from this allocator with exactly layout.
        let grown = unsafe { CountingAllocator.realloc(allocated, layout, 2_048) };
        assert!(!grown.is_null(), "small growth succeeds");
        let grown_layout = Layout::from_size_align(2_048, 8).expect("valid layout");
        // SAFETY: grown is live with grown_layout, and the new size is nonzero.
        let shrunk = unsafe { CountingAllocator.realloc(grown, grown_layout, 512) };
        assert!(!shrunk.is_null(), "small shrink succeeds");
        let shrunk_layout = Layout::from_size_align(512, 8).expect("valid layout");
        // SAFETY: shrunk is live with shrunk_layout and is freed exactly once.
        unsafe {
            CountingAllocator.dealloc(shrunk, shrunk_layout);
            measurement.finish()
        }
    }

    // SAFETY: every operation delegates verbatim to `System`; the
    // counting side effect touches only a thread-local counter and
    // never the returned memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            // SAFETY: same layout contract as the caller's.
            let ptr = unsafe { System.alloc(layout) };
            record_allocation(ptr, layout.size());
            ptr
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            live_sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            // SAFETY: same layout contract as the caller's.
            let ptr = unsafe { System.alloc_zeroed(layout) };
            record_allocation(ptr, layout.size());
            ptr
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            // SAFETY: ptr/layout come from a matching System alloc;
            // new_size is the caller's requested size.
            let resized = unsafe { System.realloc(ptr, layout, new_size) };
            record_reallocation(resized, layout.size(), new_size);
            resized
        }
    }
}

#[global_allocator]
#[cfg(test)]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::alloc_track::{Measurement, Stats, allocated_on_this_thread};
    use crate::config_funding;
    use std::fmt::Write as _;
    use std::hint::{black_box, spin_loop};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use auctioneer_config_data::activity::{Activity, ActivityOverride, Condition, Rule};
    use auctioneer_core::config::{AccountOverride, AccountPrivacyOverride};
    use std::collections::BTreeMap;
    use validator::Validate as _;

    #[test]
    fn activity_validation_does_not_materialize_effective_config_per_account() {
        let rules: Arc<[Rule]> = vec![
            Rule {
                allow: false,
                condition: Some(Condition {
                    component_name: vec!["fake_pp".to_owned(); 128],
                    component_type: vec![],
                }),
            };
            128
        ]
        .into();
        let accounts = (0_usize..4_000)
            .zip([0_u8, 1, 2, 3].into_iter().cycle())
            .map(|(index, variant)| {
                let privacy = (variant != 0).then(|| AccountPrivacyOverride {
                    allowactivities: Some(BTreeMap::from([(
                        Activity::FetchBids,
                        ActivityOverride {
                            default: Some(false),
                            rules: match variant {
                                2 => Some(Arc::from([])),
                                3 => Some(Arc::clone(&rules)),
                                _ => None,
                            },
                        },
                    )])),
                    ..AccountPrivacyOverride::default()
                });
                (
                    format!("account-{index}"),
                    AccountOverride {
                        privacy,
                        ..AccountOverride::default()
                    },
                )
            })
            .collect();
        let config = AuctioneerAppConfig {
            accounts,
            ..AuctioneerAppConfig::default()
        };
        // Exclude one-time initialization from the request-time measurement.
        let _baked = auctioneer_config_data::account_default();
        let root_defaults = AuctioneerAppConfig::default();
        root_defaults.validate().expect("valid root defaults");
        // The derive's nested validators allocate scratch maps even on success.
        // Whole-config validation may pay that cost, but not merge activity maps.
        let direct_before = allocated_on_this_thread();
        root_defaults.validate().expect("valid root defaults");
        for entry in config.accounts.values() {
            entry.validate().expect("valid supplied account rules");
        }
        let direct_validation_bytes = allocated_on_this_thread().saturating_sub(direct_before);
        let before = allocated_on_this_thread();
        let result = config.validate();
        let allocated = allocated_on_this_thread().saturating_sub(before);
        result.expect("empty, inherited, cleared, and dense replacement rules are valid");
        assert_eq!(
            allocated, direct_validation_bytes,
            "validation must inspect supplied rules without merging baked defaults per account"
        );
    }
    use auctioneer_config_data::types::{HostConfig, TriState};
    use auctioneer_core::config::resolve::{
        ResolvedAccountOwned, resolve_account_from_value, resolve_host,
    };
    use auctioneer_core::config::runtime::{RuntimeConfigError, resolve_runtime_config};
    use auctioneer_core::config::{AuctioneerAppConfig, MAX_ALIASES};
    use auctioneer_exchange::AccountSource;
    use auctioneer_runtime::budget::{
        AUX_MANIFEST_BYTES, CONFIG_ENVELOPE_VALUE_FACTOR, CONFIG_NODE_RETAINED_BYTES,
        CONFIG_PARSE_FACTOR, min_aux_route_peak_bytes,
    };
    use edgezero_adapter_axum::config_store::AxumConfigStore;
    use edgezero_adapter_axum::config_store_limits::ConfigStoreLimits;
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::canonical_form::canonical_data_sha256;
    use edgezero_core::config_store::{ConfigExtractionLimits, ConfigStoreHandle};
    use edgezero_core::context::RequestContext;
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{request_builder, response_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::router::RouterService;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use futures::executor::block_on;
    use serde_json::{Value, json};

    const STORE_ID: &str = "auctioneer_runtime";
    // Different from STORE_ID to exercise the production binding's default-key lookup.
    const STORE_KEY: &str = "raw-config-at-bound-key";
    const CANARY: &str = "loaded-from-raw-envelope";

    fn json_nodes(value: &Value) -> u32 {
        let children = match value {
            Value::Array(values) => values.iter().map(json_nodes).sum::<u32>(),
            Value::Object(values) => values.values().map(json_nodes).sum::<u32>(),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
        };
        children
            .checked_add(1)
            .expect("fixture node count fits u32")
    }

    fn json_depth(value: &Value) -> u32 {
        match value {
            Value::Array(values) => values
                .iter()
                .map(json_depth)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .expect("fixture depth fits u32"),
            Value::Object(values) => values
                .values()
                .map(json_depth)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .expect("fixture depth fits u32"),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
        }
    }

    fn envelope(data: &Value) -> Value {
        json!({
            "sha256": canonical_data_sha256(data),
            "data": data,
            "version": 1,
            "generated_at": "2026-10-06T00:00:00Z",
        })
    }

    #[derive(Debug)]
    struct Fixture {
        raw: String,
        payload_bytes: u64,
        limits: ConfigExtractionLimits,
    }

    impl Fixture {
        fn new(envelope: &Value) -> Self {
            let defaults = ConfigExtractionLimits::default();
            let nodes = json_nodes(envelope);
            assert!(
                nodes <= defaults.max_json_nodes,
                "fixture must be node bounded"
            );
            assert!(json_depth(envelope) <= defaults.max_json_depth);
            let payload_bytes = u64::try_from(
                serde_json::to_vec(envelope.get("data").expect("data field"))
                    .expect("JSON")
                    .len(),
            )
            .expect("payload length fits");
            let raw = serde_json::to_string(envelope).expect("raw envelope JSON");
            let raw_bytes = u64::try_from(raw.len()).expect("raw length fits");
            assert!(
                raw_bytes <= 0x0010_0000,
                "fixture fits the shipped 1 MiB config caps"
            );
            Self {
                raw,
                payload_bytes,
                // Tighten caps to the fixture, rather than borrowing unused 1 MiB
                // headroom. The SDK's blob cap applies to the entire returned
                // envelope, not just data; funding uses that same installed cap.
                limits: ConfigExtractionLimits {
                    max_backend_bytes: raw_bytes,
                    max_blob_bytes: raw_bytes,
                    max_total_bytes: raw_bytes,
                    max_secret_bytes: 1,
                    max_json_nodes: nodes,
                    timeout: Duration::from_millis(u64::from(
                        auctioneer_config_data::RUNTIME_BOOTSTRAP.max_config_fetch_ms,
                    )),
                    ..defaults
                },
            }
        }

        fn charged(&self) -> u64 {
            min_aux_route_peak_bytes(
                self.limits.max_backend_bytes,
                self.limits.max_blob_bytes,
                u64::from(self.limits.max_json_nodes),
            )
            .checked_sub(AUX_MANIFEST_BYTES)
            .expect("only the config charge remains")
        }

        fn with_byte_caps(mut self, backend_bytes: u64, blob_bytes: u64) -> Self {
            self.limits.max_backend_bytes = backend_bytes;
            self.limits.max_blob_bytes = blob_bytes;
            self.limits.max_total_bytes = blob_bytes;
            self.limits.validate().expect("independent caps are valid");
            self
        }
    }

    #[derive(Debug)]
    struct Resolved {
        app: AuctioneerAppConfig,
        host: HostConfig,
        account: ResolvedAccountOwned,
    }

    #[derive(Debug)]
    struct Probe {
        result: Result<Resolved, RuntimeConfigError>,
        extraction: Stats,
        total: Stats,
    }

    /// The local executor polls the SDK App and resolver on a fresh OS thread.
    /// No mock deserializes data or substitutes a predecoded config.
    fn production_probe(fixture: &Fixture) -> Probe {
        config_funding::install();
        let raw = fixture.raw.clone();
        let limits = fixture.limits;
        thread::spawn(move || {
            let store = AxumConfigStore::from_map(
                [(STORE_KEY.to_owned(), raw)],
                ConfigStoreLimits::default(),
            )
            .expect("fixture fits the config-store limits");
            let registry: ConfigRegistry = StoreRegistry::single_id(
                STORE_ID.to_owned(),
                ConfigStoreBinding {
                    default_key: STORE_KEY.to_owned(),
                    handle: ConfigStoreHandle::new(Arc::new(store)),
                },
            );
            // Initialize immutable baked defaults and preparse the request outside
            // measurement. The resolvers' owned default clones remain inside it.
            let baked_host = auctioneer_config_data::host_default();
            let baked_account = auctioneer_config_data::account_default();
            let publisher = Arc::new(json!({"site": {"publisher": {"id": "0"}}}));
            let router = RouterService::builder()
                .get("/resolve-config", move |ctx: RequestContext| {
                    let request_publisher = Arc::clone(&publisher);
                    async move {
                        assert_eq!(ctx.config_extraction_limits(), limits);
                        assert_eq!(
                            ctx.config_store_binding(STORE_ID)
                                .expect("bound store")
                                .default_key,
                            STORE_KEY,
                        );
                        let measurement = Measurement::start();
                        let loaded = resolve_runtime_config(&ctx).await;
                        let extraction = measurement.snapshot();
                        let result = loaded.map(|app| {
                            let host = resolve_host(&app, baked_host);
                            let account = resolve_account_from_value(
                                Some(&request_publisher),
                                &app,
                                baked_account,
                            )
                            .expect("valid fixtures leave the account enabled");
                            Resolved { app, host, account }
                        });
                        let total = measurement.finish();
                        let mut response =
                            response_builder().body(Body::empty()).expect("response");
                        response.extensions_mut().insert(Arc::new(Probe {
                            result,
                            extraction,
                            total,
                        }));
                        Ok::<_, EdgeError>(response)
                    }
                })
                .build();
            let mut app = App::new(router);
            app.set_config_extraction_limits(limits)
                .expect("SDK accepts fixture caps");
            let mut request = request_builder()
                .uri("/resolve-config")
                .body(Body::empty())
                .expect("request");
            request.extensions_mut().insert(registry);
            let outcome = block_on(app.dispatch_ingress(
                request,
                app.monotonic_now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
            .expect("in-process dispatch does not require sockets");
            let IngressDispatchOutcome::Response(mut egress) = outcome else {
                panic!("probe must be dispatched");
            };
            let probe = egress
                .response_mut()
                .extensions_mut()
                .remove::<Arc<Probe>>()
                .expect("real resolver handler ran");
            Arc::try_unwrap(probe).expect("probe has a single owner")
        })
        .join()
        .expect("measurement thread completes")
    }

    #[expect(
        clippy::print_stderr,
        reason = "nocapture emits exact peak/funding evidence for review"
    )]
    fn guard_production_peak(name: &str, fixture: &Fixture) -> Resolved {
        let probe = production_probe(fixture);
        let raw = fixture.limits.max_backend_bytes;
        let wire_bytes = u64::try_from(fixture.raw.len()).expect("raw length fits");
        let envelope_charge = raw
            .checked_mul(CONFIG_ENVELOPE_VALUE_FACTOR)
            .expect("charge fits");
        let parse_charge = fixture
            .limits
            .max_blob_bytes
            .checked_mul(CONFIG_PARSE_FACTOR)
            .expect("charge fits");
        let node_charge = u64::from(fixture.limits.max_json_nodes) * CONFIG_NODE_RETAINED_BYTES;
        let charged = fixture.charged();
        let term_sum = raw
            .checked_add(envelope_charge)
            .and_then(|sum| sum.checked_add(parse_charge))
            .and_then(|sum| sum.checked_add(node_charge))
            .expect("term sum fits");
        assert_eq!(charged, term_sum);
        eprintln!(
            "{name}: wire={wire_bytes} backend_cap={raw} blob_cap={} payload={} nodes={} extraction_peak={} \
             resolved_peak={} extraction_retained={} resolved_retained={} \
             cumulative={} funding={charged} \
             (raw={raw} envelope={envelope_charge} parse={parse_charge} nodes={node_charge})",
            fixture.limits.max_blob_bytes,
            fixture.payload_bytes,
            fixture.limits.max_json_nodes,
            probe.extraction.peak,
            probe.total.peak,
            probe.extraction.retained,
            probe.total.retained,
            probe.total.cumulative,
        );
        let resolved = probe
            .result
            .expect("valid raw envelope resolves through production");
        assert!(probe.total.peak >= probe.extraction.peak);
        assert!(probe.total.peak >= probe.total.retained);
        assert!(
            probe.total.retained > probe.extraction.retained,
            "host/account resolution must retain owned default clones alongside the typed config",
        );
        assert!(
            probe.extraction.peak > wire_bytes,
            "measured extraction peak must exceed the raw-envelope byte count"
        );
        assert!(
            probe.total.peak <= charged,
            "{name}: actual production PEAK {} B exceeds {charged} B of funding \
             (raw={raw}, envelope={envelope_charge}, parse={parse_charge}, nodes={node_charge}); \
             extraction peak={} B; cumulative={} B is not peak",
            probe.total.peak,
            probe.extraction.peak,
            probe.total.cumulative,
        );
        resolved
    }

    #[test]
    fn config_funding_changes_independently_with_backend_blob_and_node_caps() {
        let initial = Fixture::new(&envelope(&json!({"status": {"banner": CANARY}})));
        let wire = u64::try_from(initial.raw.len()).expect("raw length fits");
        let backend = wire + 17;
        let blob = wire + 101;
        let node_charge = u64::from(initial.limits.max_json_nodes) * CONFIG_NODE_RETAINED_BYTES;
        let fixture = initial.with_byte_caps(backend, blob);
        let charged = fixture.charged();
        assert_ne!(backend, blob);
        assert_ne!(1 + CONFIG_ENVELOPE_VALUE_FACTOR, CONFIG_PARSE_FACTOR);
        assert_eq!(
            charged,
            backend * (1 + CONFIG_ENVELOPE_VALUE_FACTOR) + blob * CONFIG_PARSE_FACTOR + node_charge,
            "unequal caps expose swapped factors and charging both terms against one cap",
        );

        // Only one installed limit changes at a time. Equal caps would hide
        // attributing either factor to the other cap in min_aux_route_peak_bytes.
        let backend_only = Fixture::new(&envelope(&json!({"status": {"banner": CANARY}})))
            .with_byte_caps(backend + 1, blob);
        assert_eq!(
            backend_only.charged() - charged,
            1 + CONFIG_ENVELOPE_VALUE_FACTOR,
        );
        let blob_only = Fixture::new(&envelope(&json!({"status": {"banner": CANARY}})))
            .with_byte_caps(backend, blob + 1);
        assert_eq!(blob_only.charged() - charged, CONFIG_PARSE_FACTOR);
        let mut node_only = fixture;
        node_only.limits.max_json_nodes += 1;
        assert_eq!(node_only.charged() - charged, CONFIG_NODE_RETAINED_BYTES);
    }

    #[test]
    fn production_resolution_enforces_backend_byte_cap_independently_of_blob_cap() {
        let initial = Fixture::new(&envelope(&json!({"status": {"banner": CANARY}})));
        let wire = u64::try_from(initial.raw.len()).expect("raw length fits");
        let fixture = initial.with_byte_caps(wire, wire + 128);
        let resolved = guard_production_peak("exact-backend-cap-larger-blob-cap", &fixture);
        assert_eq!(resolved.host.status_response, CANARY);
        assert_production_error(
            &fixture.with_byte_caps(wire - 1, wire + 128),
            "ValueTooLarge",
        );
    }

    #[test]
    fn production_resolution_enforces_blob_byte_cap_independently_of_backend_cap() {
        let initial = Fixture::new(&envelope(&json!({"status": {"banner": CANARY}})));
        let wire = u64::try_from(initial.raw.len()).expect("raw length fits");
        let fixture = initial.with_byte_caps(wire + 128, wire);
        let resolved = guard_production_peak("exact-blob-cap-larger-backend-cap", &fixture);
        assert_eq!(resolved.host.status_response, CANARY);
        assert_production_error(
            &fixture.with_byte_caps(wire + 128, wire - 1),
            "ValueTooLarge",
        );
    }

    #[test]
    fn production_resolution_peak_fits_funding_for_minimal_config() {
        let fixture = Fixture::new(&envelope(&json!({})));
        let resolved = guard_production_peak("minimal", &fixture);
        assert_eq!(
            resolved.host.status_response,
            auctioneer_config_data::host_default().status_response
        );
        assert_eq!(resolved.account.source, AccountSource::DefaultFallback);
    }

    #[test]
    fn production_resolution_peak_fits_funding_for_node_bounded_empty_accounts() {
        let mut data = json!({"accounts": {}, "status": {"banner": CANARY}});
        let available =
            ConfigExtractionLimits::default().max_json_nodes - json_nodes(&envelope(&data));
        let accounts = data
            .get_mut("accounts")
            .expect("accounts")
            .as_object_mut()
            .expect("map");
        for id in 0..available {
            accounts.insert(format!("{id:x}"), json!({}));
        }
        let fixture = Fixture::new(&envelope(&data));
        assert_eq!(
            fixture.limits.max_json_nodes,
            ConfigExtractionLimits::default().max_json_nodes
        );
        let resolved = guard_production_peak("empty-accounts-at-node-cap", &fixture);
        assert_eq!(
            resolved.app.accounts.len(),
            usize::try_from(available).expect("count fits")
        );
        assert_eq!(resolved.account.source, AccountSource::Configured);
        assert_eq!(resolved.host.status_response, CANARY);
    }

    #[test]
    fn production_resolution_peak_fits_funding_for_many_account_overlays_and_aliases() {
        let base = &auctioneer_config_data::bidders()
            .first()
            .expect("production alias validation requires a baked base")
            .name;
        let mut data = json!({"accounts": {}, "aliases": {}, "status": {"banner": CANARY}});
        let accounts = data
            .get_mut("accounts")
            .expect("accounts")
            .as_object_mut()
            .expect("map");
        for id in 0_usize..768_usize {
            accounts.insert(
                format!("{id:x}"),
                json!({"gdpr": {
                    "enabled": "enabled", "channel_enabled": {"web": "enabled"},
                    "basic_enforcement_vendors": ["fake_bbb"],
                }}),
            );
        }
        let aliases = data
            .get_mut("aliases")
            .expect("aliases")
            .as_object_mut()
            .expect("map");
        for id in 0..MAX_ALIASES {
            // This row exercises the production loader's compiled-base validation.
            aliases.insert(format!("fake_{id:x}"), json!(base));
        }
        let fixture = Fixture::new(&envelope(&data));
        let resolved = guard_production_peak("account-overlays-and-aliases", &fixture);
        assert_eq!(resolved.app.accounts.len(), 768);
        assert_eq!(resolved.host.aliases.len(), MAX_ALIASES);
        assert_eq!(resolved.account.config.gdpr.enabled, TriState::Enabled);
        assert_eq!(
            resolved.account.config.gdpr.channel_enabled.web,
            TriState::Enabled
        );
        assert_eq!(resolved.host.status_response, CANARY);
    }

    #[test]
    fn production_resolution_peak_fits_funding_for_dense_ignored_envelope_metadata() {
        let defaults = ConfigExtractionLimits::default();
        let mut raw_envelope = envelope(&json!({"status": {"banner": CANARY}}));
        raw_envelope
            .as_object_mut()
            .expect("envelope")
            .insert("metadata".to_owned(), json!([]));
        let available = defaults.max_json_nodes - json_nodes(&raw_envelope);
        let chain_nodes = defaults.max_json_depth - 2;
        let metadata = raw_envelope
            .get_mut("metadata")
            .expect("metadata")
            .as_array_mut()
            .expect("array");
        let chains = available.checked_div(chain_nodes).expect("chain has nodes");
        for _ in 0..chains {
            let mut chain = json!({});
            for _ in 1..chain_nodes {
                chain = json!({"": chain});
            }
            metadata.push(chain);
        }
        let remaining = available.checked_rem(chain_nodes).expect("chain has nodes");
        for _ in 0..remaining {
            metadata.push(json!({}));
        }
        let fixture = Fixture::new(&raw_envelope);
        assert_eq!(fixture.limits.max_json_nodes, defaults.max_json_nodes);
        let resolved = guard_production_peak("dense-envelope-metadata", &fixture);
        assert_eq!(resolved.host.status_response, CANARY);
        assert!(
            resolved.app.accounts.is_empty(),
            "ignored metadata is not typed account data"
        );
    }

    #[test]
    fn production_resolution_peak_fits_funding_for_escaped_publisher_strings() {
        let publishers = vec!["\0\\\"\n"; 7_680];
        let fixture = Fixture::new(&envelope(&json!({
            "status": {"banner": CANARY},
            "gdpr": {"enabled": true, "non_standard_publishers": publishers},
        })));
        assert!(
            fixture.raw.contains("\\u0000"),
            "raw input carries JSON escapes"
        );
        let resolved = guard_production_peak("escaped-publishers", &fixture);
        assert!(resolved.host.gdpr.enabled);
        assert_eq!(resolved.host.gdpr.non_standard_publishers.len(), 7_680);
        assert_eq!(
            resolved
                .host
                .gdpr
                .non_standard_publishers
                .first()
                .expect("publisher"),
            "\0\\\"\n"
        );
    }

    #[expect(
        clippy::print_stderr,
        reason = "nocapture emits funded rejection-path peaks for review"
    )]
    fn assert_production_error(fixture: &Fixture, reason: &str) {
        let probe = production_probe(fixture);
        let charged = fixture.charged();
        eprintln!(
            "rejected-{reason}: wire={} backend_cap={} blob_cap={} nodes={} peak={} funding={charged}",
            fixture.raw.len(),
            fixture.limits.max_backend_bytes,
            fixture.limits.max_blob_bytes,
            fixture.limits.max_json_nodes,
            probe.total.peak,
        );
        assert!(
            probe.total.peak <= charged,
            "{reason}: rejected-envelope peak {} B exceeds {charged} B of combined funding",
            probe.total.peak,
        );
        let err = probe
            .result
            .expect_err("a faulty raw envelope must not become baked defaults");
        let RuntimeConfigError::MalformedHost { detail } = err else {
            panic!("expected malformed config, got {err:?}");
        };
        assert!(detail.contains(reason), "expected {reason}, got {detail}");
    }

    #[test]
    fn production_resolution_checks_raw_envelope_integrity_and_version() {
        let mut tampered = envelope(&json!({"status": {"banner": CANARY}}));
        tampered
            .get_mut("data")
            .expect("data")
            .get_mut("status")
            .expect("status")
            .as_object_mut()
            .expect("map")
            .insert("banner".to_owned(), json!("tampered"));
        assert_production_error(&Fixture::new(&tampered), "IntegrityMismatch");
        let mut future_version = envelope(&json!({}));
        future_version
            .as_object_mut()
            .expect("envelope")
            .insert("version".to_owned(), json!(99_u32));
        assert_production_error(&Fixture::new(&future_version), "UnsupportedVersion");
    }

    #[test]
    fn production_resolution_performs_typed_conversion_and_validation() {
        for (data, reason) in [
            (json!({"accounts": []}), "Deserialization"),
            (json!({"targeting": {"prefix": ""}}), "Validation"),
            (
                json!({"aliases": {"fake_a": "fake_unregistered_base"}}),
                "unknown base",
            ),
        ] {
            assert_production_error(&Fixture::new(&envelope(&data)), reason);
        }
    }

    #[test]
    fn production_resolution_enforces_installed_raw_byte_and_node_caps() {
        let mut fixture = Fixture::new(&envelope(&json!({"status": {"banner": CANARY}})));
        fixture.limits.max_blob_bytes -= 1;
        assert_production_error(&fixture, "ValueTooLarge");
        fixture.limits.max_blob_bytes += 1;
        fixture.limits.max_json_nodes -= 1;
        assert_production_error(&fixture, "StructuralLimit");
    }

    #[test]
    fn production_resolution_funds_oversized_bidder_key_rejection() {
        use auctioneer_config_data::types::MAX_BIDDER_NAME_BYTES;
        for key_bytes in [MAX_BIDDER_NAME_BYTES + 1, 0x0001_0000] {
            let key = "b".repeat(key_bytes);
            let mut data = json!({"bidders": {}});
            data.get_mut("bidders")
                .expect("bidders")
                .as_object_mut()
                .expect("map")
                .insert(key, json!({"enabled": false}));
            assert_production_error(&Fixture::new(&envelope(&data)), "Validation");
        }
    }

    /// An adversarial config blob: many small entries across the maps
    /// the config carries, each one an owned key and an owned value.
    /// Entry COUNT, not blob size, is what drives this factor.
    fn adversarial_config_toml() -> String {
        let mut src = String::new();

        // Per-account overlays: each one an owned account key plus an
        // owned nested GDPR overlay with its own owned vendor list.
        for account in 0_usize..256_usize {
            let _written = write!(
                src,
                "[accounts.\"acct-{account}\".gdpr]\n\
                 enabled = \"enabled\"\n\
                 channel_enabled = {{ web = \"enabled\" }}\n\
                 basic_enforcement_vendors = [\"fake_bbb\"]\n\n"
            );
        }

        // Host-declared aliases at the envelope's default cap: the
        // densest owned-key-to-owned-value map the config carries.
        src.push_str("[aliases]\n");
        for alias in 0_usize..64_usize {
            let _written = writeln!(src, "\"fake_alias_{alias}\" = \"fake_bbb\"");
        }

        src
    }

    #[test]
    fn direct_toml_typed_config_retention_fits_parse_charge() {
        let src = adversarial_config_toml();
        let bytes = src.as_bytes();
        let blob_len = u64::try_from(bytes.len()).expect("fixture length fits u64");
        let charged = blob_len
            .checked_mul(CONFIG_PARSE_FACTOR)
            .expect("the charge does not overflow");

        // Measure on a THREAD OF ITS OWN. Both counters are
        // thread-local and a fresh thread starts at zero, so the
        // readings cover this parse and nothing else — no harness
        // bookkeeping and no neighbouring test can land inside the
        // window. Measuring on the harness's own thread made the
        // result depend on whatever ran before it, which is not a
        // property of the parse being measured.
        let measured_src = src.clone();
        let stats = thread::spawn(move || {
            let measurement = Measurement::start();
            let parsed: AuctioneerAppConfig =
                toml::from_str(&measured_src).expect("fixture TOML parses");
            let stats = measurement.finish();
            // Drop inside the thread, after both readings, so the
            // measurement covers what the config RETAINS.
            drop(parsed);
            stats
        })
        .join()
        .expect("the measuring thread does not panic");

        assert!(
            stats.retained <= charged,
            "the config parse RETAINED {} B for a {blob_len} B blob, over \
             the {charged} B the budget charges at CONFIG_PARSE_FACTOR = \
             {CONFIG_PARSE_FACTOR} (cumulative including transients: {} B)",
            stats.retained,
            stats.cumulative,
        );
    }

    #[test]
    fn reallocation_peak_counts_only_the_live_size_delta() {
        let stats = thread::spawn(super::alloc_track::reallocation_stats)
            .join()
            .expect("measurement thread completes");
        assert_eq!(
            stats.peak, 2_048,
            "the old and new realloc blocks do not overlap"
        );
        assert_eq!(
            stats.retained, 0,
            "shrinking and deallocation restore live bytes"
        );
        assert_eq!(stats.cumulative, 2_048, "only successful growth is charged");
    }

    #[test]
    fn failed_allocations_do_not_change_live_peak_or_cumulative_bytes() {
        let stats = thread::spawn(super::alloc_track::failed_operation_stats)
            .join()
            .expect("measurement thread completes");
        assert_eq!(stats.peak, 0);
        assert_eq!(stats.retained, 0);
        assert_eq!(stats.cumulative, 0);
    }

    #[test]
    fn peak_live_bytes_exclude_freed_transient_allocations() {
        let stats = thread::spawn(|| {
            let measurement = Measurement::start();
            for _ in 0_u32..32_u32 {
                let buffer = black_box(vec![0_u8; 1_024]);
                drop(buffer);
            }
            measurement.finish()
        })
        .join()
        .expect("measurement thread completes");
        assert_eq!(stats.peak, 1_024);
        assert_eq!(stats.retained, 0);
        assert_eq!(stats.cumulative, 32 * 1_024);
    }

    #[test]
    fn peak_tracking_ignores_allocations_on_other_threads() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let (local, remote) = thread::spawn(|| {
            let begin = Arc::new(AtomicBool::new(false));
            let done = Arc::new(AtomicBool::new(false));
            let remote_begin = Arc::clone(&begin);
            let remote_done = Arc::clone(&done);
            let worker = thread::spawn(move || {
                while !remote_begin.load(Ordering::Acquire) {
                    spin_loop();
                }
                let measurement = Measurement::start();
                let buffer = black_box(vec![0_u8; 0x8000]);
                let stats = measurement.finish();
                drop(buffer);
                remote_done.store(true, Ordering::Release);
                stats
            });
            let measurement = Measurement::start();
            begin.store(true, Ordering::Release);
            while !done.load(Ordering::Acquire) {
                spin_loop();
            }
            let local = measurement.finish();
            (local, worker.join().expect("other thread completes"))
        })
        .join()
        .expect("measurement thread completes");
        assert_eq!(local.peak, 0);
        assert_eq!(local.retained, 0);
        assert_eq!(local.cumulative, 0);
        assert_eq!(remote.peak, 0x8000);
        assert_eq!(remote.retained, 0x8000);
    }
}

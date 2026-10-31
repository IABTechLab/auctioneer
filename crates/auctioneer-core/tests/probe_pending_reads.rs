//! Probe bounds observed at the backend and through retained responses.

#[cfg(test)]
mod support;

#[cfg(test)]
pub mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_config::limits::MAX_STATUS_BANNER_BYTES;
    use auctioneer_core::config::runtime::CONFIG_STATUS_HEADER;
    use auctioneer_core::handlers::probe_config::{
        __probe_isolation_for_tests, config_work_capacity, config_work_in_flight,
    };
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::handlers::status::status;
    use auctioneer_core::host_primitives::{boot_gates, install_boot_envelope};
    use auctioneer_runtime::budget::{
        ASSEMBLY_FACTOR, derive_aux_work_for_profile, min_aux_route_peak_bytes,
    };
    use auctioneer_runtime::{
        MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES,
        MIN_RESPONSE_PARSE_FACTOR, ResolvedIsolateLimits, TargetPlatformConstants,
        serialized_json_bytes,
    };
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
        DEFAULT_CONFIG_JSON_NODES,
    };
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::{Response, StatusCode, request_builder};
    use edgezero_core::params::PathParams;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::channel::oneshot;
    use futures::executor::block_on;
    use futures::task::noop_waker;
    use futures::task::{ArcWake, waker};
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::str;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};

    fn install_native_envelope() {
        const CAP: u64 = 1 << 18;
        const CONFIG_CAP: u64 = 1 << 20;
        let nodes = DEFAULT_CONFIG_JSON_NODES;
        let mut limits = ResolvedIsolateLimits {
            isolate_total_bytes: 512_000_000,
            baseline_bytes: 96_000_000,
            inbound_request_bytes: CAP,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: CAP,
            max_outbound_raw_bytes: CAP,
            max_transport_bytes: CAP,
            max_decoded_bytes: CAP,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 64 * CAP * ASSEMBLY_FACTOR,
            egress_permits: 64,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 16,
            aux_route_peak_bytes: min_aux_route_peak_bytes(
                CONFIG_CAP,
                CONFIG_CAP,
                u64::from(nodes),
            ),
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 50,
            max_outbound_requests: 64,
            max_concurrent_auctions: 64,
            max_active_body_streams: 32,
            max_runtime_config_bytes: CONFIG_CAP,
            max_runtime_config_blob_bytes: CONFIG_CAP,
            max_parsed_nodes: 1 << 16,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 1 << 14,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            max_config_json_nodes: nodes,
            terminal_response_permits: 384,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        };
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: 1 << 14,
            egress_handoff_factor: 2,
        };
        // Match bootstrap's auxiliary work and independent baked policy charge.
        let baked_bytes = serialized_json_bytes(&(
            auctioneer_config::host_default(),
            auctioneer_config::account_default(),
        ))
        .unwrap();
        let aux_work = derive_aux_work_for_profile(&limits, &target, baked_bytes).unwrap();
        limits.aux_route_peak_bytes = limits.aux_route_peak_bytes.max(aux_work.peak_bytes());
        config_funding::fund_sdk_defaults(&mut limits, &target);
        install_boot_envelope(
            &limits,
            &target,
            auctioneer_bidder::outbound_meta_bytes(100),
            core_construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
    }

    #[derive(Default)]
    struct WakeCount(AtomicU32);

    impl ArcWake for WakeCount {
        fn wake(self: Arc<Self>) {
            Self::wake_by_ref(&self);
        }

        fn wake_by_ref(arc_self: &Arc<Self>) {
            arc_self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[derive(Default)]
    struct PendingStore {
        active: AtomicU32,
        reads: AtomicU32,
        completions: Mutex<VecDeque<oneshot::Sender<()>>>,
    }

    struct ReadLifetime<'a>(&'a AtomicU32);

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for ReadLifetime<'_> {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    impl PendingStore {
        fn complete_one(&self) {
            self.completions
                .lock()
                .unwrap()
                .pop_front()
                .expect("a backend read is pending")
                .send(())
                .expect("the read has not been cancelled");
        }
    }

    #[async_trait(?Send)]
    impl ConfigStore for PendingStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("the probe must use bounded extraction")
        }

        async fn get_bounded(
            &self,
            _key: &str,
            _clock: &MonotonicClock,
            _deadline: Deadline,
            _max_backend_bytes: u64,
            _max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            self.active.fetch_add(1, Ordering::SeqCst);
            self.reads.fetch_add(1, Ordering::SeqCst);
            let _lifetime = ReadLifetime(&self.active);
            let (tx, rx) = oneshot::channel();
            self.completions.lock().unwrap().push_back(tx);
            rx.await.expect("the test releases pending backend reads");
            Err(ConfigStoreError::Unavailable {
                message: "backend outage".to_owned(),
            })
        }
    }

    fn request(store: Arc<PendingStore>) -> RequestContext {
        let mut request = request_builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let registry: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(store),
            },
        );
        request.extensions_mut().insert(registry);
        RequestContext::new(request, PathParams::default())
    }

    fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(&noop_waker()))
    }

    fn poll_counted<F: Future>(future: Pin<&mut F>, count: &Arc<WakeCount>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(&waker(Arc::clone(count))))
    }

    fn degraded(response: &Response) {
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CONFIG_STATUS_HEADER], "degraded");
        let body = response.body().as_bytes().expect("a buffered probe body");
        assert!(body.len() <= 8_192);
        assert!(str::from_utf8(body).unwrap().contains("config: degraded"));
    }

    #[test]
    fn cold_probe_waits_for_actual_pending_reads_without_exceeding_capacity() {
        let _isolation = __probe_isolation_for_tests();
        install_native_envelope();
        let store = Arc::new(PendingStore::default());
        let mut reads = vec![Box::pin(status(request(Arc::clone(&store))))];
        assert!(poll(reads[0].as_mut()).is_pending());
        let capacity = config_work_capacity().expect("the first read initializes the gate");
        assert_eq!(capacity, 16);
        for _ in 1..capacity {
            reads.push(Box::pin(status(request(Arc::clone(&store)))));
            assert!(poll(reads.last_mut().unwrap().as_mut()).is_pending());
        }
        assert_eq!(store.active.load(Ordering::SeqCst), capacity);
        let mut waiting = Box::pin(status(request(Arc::clone(&store))));
        let wakes = Arc::new(WakeCount::default());
        assert!(poll_counted(waiting.as_mut(), &wakes).is_pending());
        assert_eq!(
            store.active.load(Ordering::SeqCst),
            capacity,
            "a cold waiter must not start a capacity-plus-one backend read",
        );
        store.complete_one();
        let Poll::Ready(Ok(response)) = poll(reads[0].as_mut()) else {
            panic!("the released backend read must produce its diagnosis");
        };
        degraded(&response);
        assert!(
            wakes.0.load(Ordering::SeqCst) > 0,
            "publishing a verdict wakes the cold probe"
        );
        let Poll::Ready(Ok(waited)) = poll(waiting.as_mut()) else {
            panic!("the cold waiter must share the completed diagnosis");
        };
        degraded(&waited);
        assert_eq!(store.reads.load(Ordering::SeqCst), capacity);
        drop(reads);
        assert_eq!(store.active.load(Ordering::SeqCst), 0);
        assert_eq!(config_work_in_flight(), 0);
        // Output remains alive after every config permit has been released.
        degraded(&response);
        degraded(&waited);
    }

    #[test]
    fn cancelling_all_cold_reads_allows_a_waiter_to_resolve() {
        let _isolation = __probe_isolation_for_tests();
        install_native_envelope();
        let store = Arc::new(PendingStore::default());
        let mut reads = vec![Box::pin(status(request(Arc::clone(&store))))];
        assert!(poll(reads[0].as_mut()).is_pending());
        let capacity = config_work_capacity().unwrap();
        for _ in 1..capacity {
            reads.push(Box::pin(status(request(Arc::clone(&store)))));
            assert!(poll(reads.last_mut().unwrap().as_mut()).is_pending());
        }
        let mut waiting = Box::pin(status(request(Arc::clone(&store))));
        let wakes = Arc::new(WakeCount::default());
        assert!(poll_counted(waiting.as_mut(), &wakes).is_pending());
        assert_eq!(store.active.load(Ordering::SeqCst), capacity);
        drop(reads);
        assert!(
            wakes.0.load(Ordering::SeqCst) > 0,
            "releasing a slot wakes the cold probe"
        );
        assert_eq!(store.active.load(Ordering::SeqCst), 0);
        store.completions.lock().unwrap().clear();
        assert!(poll(waiting.as_mut()).is_pending());
        assert_eq!(store.active.load(Ordering::SeqCst), 1);
        assert_eq!(config_work_in_flight(), 1);
        store.complete_one();
        let Poll::Ready(Ok(response)) = poll(waiting.as_mut()) else {
            panic!("cancellation must not strand the next resolver");
        };
        degraded(&response);
        assert_eq!(config_work_in_flight(), 0);
    }

    fn configured_probe(banner: &str) -> Response {
        let request = request_builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let source = format!("[status]\nbanner = {banner:?}\n");
        block_on(status(ctx_with_config(&source, request))).unwrap()
    }

    #[test]
    fn fresh_output_survives_config_completion_for_the_whole_platform_population() {
        let _isolation = __probe_isolation_for_tests();
        install_native_envelope();
        let population = boot_gates().unwrap().target.platform_concurrent_ceiling;
        let cap = MAX_STATUS_BANNER_BYTES;
        let mut responses = Vec::new();
        for _ in 0..population {
            let response = configured_probe(&"x".repeat(cap));
            assert_eq!(response.headers()[CONFIG_STATUS_HEADER], "ok");
            assert_eq!(response.body().as_bytes().unwrap().len(), cap);
            assert_eq!(config_work_in_flight(), 0);
            responses.push(response);
        }
        let retained: usize = responses
            .iter()
            .map(|r| r.body().as_bytes().unwrap().len())
            .sum();
        assert_eq!(retained, usize::try_from(population).unwrap() * cap);
        assert!(retained > usize::try_from(config_work_capacity().unwrap()).unwrap() * cap);
        // Replacing the cache cannot mutate or invalidate prior output.
        drop(responses.pop());
        drop(configured_probe("replacement"));
        assert!(
            responses
                .iter()
                .all(|r| r.body().as_bytes().unwrap() == vec![b'x'; cap])
        );
    }

    #[test]
    fn cached_flood_retains_bounded_output_without_further_backend_reads() {
        let _isolation = __probe_isolation_for_tests();
        install_native_envelope();
        let cap = MAX_STATUS_BANNER_BYTES;
        drop(configured_probe(&"x".repeat(cap)));
        let capacity = config_work_capacity().unwrap();
        let store = Arc::new(PendingStore::default());
        let mut reads = Vec::new();
        for _ in 0..capacity {
            reads.push(Box::pin(status(request(Arc::clone(&store)))));
            assert!(poll(reads.last_mut().unwrap().as_mut()).is_pending());
        }
        let population = boot_gates().unwrap().target.platform_concurrent_ceiling;
        let mut responses = Vec::new();
        for _ in capacity..population {
            let response = block_on(status(request(Arc::clone(&store)))).unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[CONFIG_STATUS_HEADER], "ok-cached");
            assert_eq!(response.body().as_bytes().unwrap().len(), cap);
            responses.push(response);
        }
        assert_eq!(store.reads.load(Ordering::SeqCst), capacity);
        assert_eq!(store.active.load(Ordering::SeqCst), capacity);
        drop(reads);
        assert_eq!(config_work_in_flight(), 0);
        assert!(
            responses
                .iter()
                .all(|r| r.body().as_bytes().unwrap() == vec![b'x'; cap])
        );
        store.completions.lock().unwrap().clear();
        let mut recovered = Box::pin(status(request(Arc::clone(&store))));
        assert!(poll(recovered.as_mut()).is_pending());
        assert_eq!(store.reads.load(Ordering::SeqCst), capacity + 1);
        store.complete_one();
        let Poll::Ready(Ok(fresh)) = poll(recovered.as_mut()) else {
            panic!("available capacity must trigger a fresh backend resolution");
        };
        degraded(&fresh);
        assert_eq!(config_work_in_flight(), 0);
    }
}

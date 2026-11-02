//! A cold pair stays live without consulting an unfunded config store.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_core::config::runtime::CONFIG_STATUS_HEADER;
    use auctioneer_core::handlers::probe_config::{config_work_capacity, config_work_in_flight};
    use auctioneer_core::handlers::status::status;
    use auctioneer_core::host_primitives::boot_gates;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
    };
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::{StatusCode, request_builder};
    use edgezero_core::params::PathParams;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use futures::future::join;
    use std::str;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct OneReadStore {
        reads: AtomicU32,
    }

    #[async_trait(?Send)]
    impl ConfigStore for OneReadStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("bounded extraction is required")
        }

        async fn get_bounded(
            &self,
            _key: &str,
            _clock: &MonotonicClock,
            _deadline: Deadline,
            _max_backend_bytes: u64,
            _max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Err(ConfigStoreError::Unavailable {
                message: "outage".to_owned(),
            })
        }
    }

    fn request(store: Arc<OneReadStore>) -> RequestContext {
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

    #[test]
    fn preboot_cold_pair_refuses_reads_and_both_keep_the_diagnosis() {
        assert!(boot_gates().is_none());
        assert!(config_work_capacity().is_none());
        let store = Arc::new(OneReadStore {
            reads: AtomicU32::new(0),
        });
        let first = status(request(Arc::clone(&store)));
        let second = status(request(Arc::clone(&store)));
        let (first_response, second_response) = block_on(join(first, second));
        for response in [first_response.unwrap(), second_response.unwrap()] {
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[CONFIG_STATUS_HEADER], "degraded");
            assert!(
                str::from_utf8(response.body().as_bytes().unwrap())
                    .unwrap()
                    .contains("config: degraded (unfunded_extraction)")
            );
        }
        assert_eq!(config_work_capacity(), Some(1));
        assert_eq!(store.reads.load(Ordering::SeqCst), 0);
        assert_eq!(config_work_in_flight(), 0);
    }
}

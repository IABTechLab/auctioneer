//! The process-global logger needs isolation from bootstrap logger tests.

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use async_trait::async_trait;
    use auctioneer_core::config::runtime::resolve_runtime_config;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
    };
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::request_builder;
    use edgezero_core::params::PathParams;
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use log::{Level, LevelFilter, Log, Metadata, Record};
    use std::sync::{Arc, Mutex};

    const BACKEND_SECRET: &str = "backend-credential-must-not-reach-warning";

    struct UnavailableStore;

    #[async_trait(?Send)]
    impl ConfigStore for UnavailableStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("production extraction must use the bounded read");
        }

        async fn get_bounded(
            &self,
            _key: &str,
            _clock: &MonotonicClock,
            _deadline: Deadline,
            _max_backend_bytes: u64,
            _max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            Err(ConfigStoreError::Unavailable {
                message: BACKEND_SECRET.to_owned(),
            })
        }
    }

    struct CapturingLog(Mutex<Vec<String>>);

    impl Log for CapturingLog {
        fn enabled(&self, metadata: &Metadata<'_>) -> bool {
            metadata.level() == Level::Warn
                && metadata.target() == "auctioneer_core::config::runtime"
        }

        fn log(&self, record: &Record<'_>) {
            if self.enabled(record.metadata()) {
                self.0.lock().unwrap().push(record.args().to_string());
            }
        }

        fn flush(&self) {}
    }

    static LOGGER: CapturingLog = CapturingLog(Mutex::new(Vec::new()));

    #[test]
    fn permissive_config_degradation_warns_without_backend_error_contents() {
        config_funding::install();
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(LevelFilter::Warn);
        let mut request = request_builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let registry: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(UnavailableStore)),
            },
        );
        request.extensions_mut().insert(registry);
        let context = RequestContext::new(request, PathParams::default());
        let error = block_on(resolve_runtime_config(&context)).unwrap_err();
        assert_eq!(error.status_code().as_str(), "store_unavailable");
        let _fallback = error.degrade_to_default().unwrap();

        let warnings = LOGGER.0.lock().unwrap();
        assert_eq!(warnings.len(), 1, "permissive fallback must not be silent");
        assert!(warnings[0].contains("BackendUnavailable"));
        assert!(warnings[0].contains("Continuing from AuctioneerAppConfig::default()"));
        assert!(!warnings[0].contains(BACKEND_SECRET));
    }
}

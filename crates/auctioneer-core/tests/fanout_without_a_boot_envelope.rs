//! A direct handler call with platform primitives but no envelope fails before reads.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_core::handlers::auction;
    use auctioneer_core::host_primitives::{
        DelegateToDevStrategy, DevRng, SystemClock, boot_gates, bootstrap_adapter,
    };
    use auctioneer_runtime::{ImmediateResolveTimer, NoopDecodeCooperator};
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
    use std::sync::Arc;

    struct UnreadStore;

    #[async_trait(?Send)]
    impl ConfigStore for UnreadStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            panic!("unfunded extraction cannot read config")
        }

        async fn get_bounded(
            &self,
            _key: &str,
            _clock: &MonotonicClock,
            _deadline: Deadline,
            _max_backend_bytes: u64,
            _max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            panic!("unfunded extraction cannot read bounded config")
        }
    }

    #[test]
    fn an_auction_refuses_config_with_no_boot_envelope_installed() {
        bootstrap_adapter(
            || Arc::new(SystemClock),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        );
        assert!(boot_gates().is_none());
        let mut request = request_builder()
            .method("POST")
            .uri("/openrtb2/auction")
            .body(Body::empty())
            .unwrap();
        let registry: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".into(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".into(),
                handle: ConfigStoreHandle::new(Arc::new(UnreadStore)),
            },
        );
        request.extensions_mut().insert(registry);
        let response =
            block_on(auction(RequestContext::new(request, PathParams::default()))).unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("retry-after"));
        assert_eq!(
            response.body().as_bytes().unwrap(),
            b"Unfunded configuration extraction"
        );
        assert!(boot_gates().is_none());
    }
}

//! Live JSON-envelope extraction and merged policy allocation evidence.

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;
mod bounded_store;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::{alloc_track, bounded_store::bounded_over_get};
    use crate::config_funding;
    use async_trait::async_trait;
    use auctioneer_core::config::{
        resolve::{resolve_account, resolve_host},
        runtime::{RuntimeConfigError, resolve_runtime_config},
    };
    use auctioneer_core::test_support::config_store as funded_config;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_privacy::{
        PrivacyContext, ResolveOptions, ccpa::NoSale, gcl::Gcl, gvl::GvlStore,
    };
    use auctioneer_runtime::budget::CONFIG_PARSE_FACTOR;
    use edgezero_core::{
        blob_envelope::BlobEnvelope,
        body::Body,
        config_store::{
            BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
        },
        store_registry::{ConfigStoreBinding, StoreRegistry},
        time::{Deadline, MonotonicClock},
    };
    use futures::executor::block_on;
    use serde_json::{Map, Value, json};
    use std::io::{self, Write as _};
    use std::sync::Arc;
    use std::thread;
    use time::OffsetDateTime;

    struct Store(ConfigValue);
    #[async_trait(?Send)]
    impl ConfigStore for Store {
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

    fn enforcement_fixture() -> Value {
        let exceptions = vec![""; 30_000];
        json!({
            "schema_version": 1_u64,
            "host": {
                "gdpr": {
                    "enabled": true,
                    "tcf2": {
                        "purposes": {
                            "1": {
                                "vendor_exceptions": exceptions,
                            },
                        },
                    },
                },
            },
            "accounts": {
                "a": {
                    "gdpr": {
                        "basic_enforcement_vendors": exceptions,
                        "purposes": {"2":{"vendor_exceptions":exceptions}},
                    },
                },
            },
        })
    }

    fn fixtures() -> Vec<(&'static str, Value, bool)> {
        let activity_names = [
            "fetchBids",
            "transmitUfpd",
            "transmitPreciseGeo",
            "transmitTid",
            "syncUser",
            "reportAnalytics",
            "enrichUfpd",
            "transmitUniqueRequestIds",
        ];
        let activities: Map<_, _> = activity_names.into_iter().map(|name| (name.into(), json!({
            "rules":vec![json!({"allow":false, "condition":{"componentName":vec!["";128]}});128]
        }))).collect();
        let heavy = json!({
            "schema_version": 1_u64,
            "accounts": {"a":{"privacy":{"allowactivities":activities}}},
        });
        let rule_limits: Map<_, _> = activity_names
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    json!({
                        "rules":vec![json!({"allow":false, "condition":{"componentName":[""]}});128]
                    }),
                )
            })
            .collect();
        let component_limits: Map<_, _> = activity_names
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    json!({
                        "rules":[{"allow":false, "condition":{"componentName":vec!["";128]}}]
                    }),
                )
            })
            .collect();
        let accounts: Map<_, _> = (0_u32..256).map(|n| (format!("a{n}"), json!({
            "gdpr":{"basic_enforcement_vendors":["fake_pp"]}, "ccpa":{"enabled":"enabled"}
        }))).collect();
        let enforcement = enforcement_fixture();
        let empty_accounts: Map<_, _> = (0_u32..4000).map(|n| (n.to_string(), json!({}))).collect();
        vec![
            (
                "minimal",
                json!({
                    "schema_version": 1_u64,
                }),
                false,
            ),
            (
                "many-accounts",
                json!({
                    "schema_version": 1_u64,
                    "accounts": accounts,
                }),
                false,
            ),
            ("rule-name-product-over-structural-limit", heavy, true),
            (
                "rule-count-limits",
                json!({
                    "schema_version": 1_u64,
                    "accounts": {"a":{"privacy":{"allowactivities":rule_limits}}},
                }),
                false,
            ),
            (
                "component-name-limits",
                json!({
                    "schema_version": 1_u64,
                    "accounts": {"a":{"privacy":{"allowactivities":component_limits}}},
                }),
                false,
            ),
            (
                "enforcement-clones-over-structural-limit",
                enforcement,
                true,
            ),
            (
                "empty-accounts",
                json!({
                    "schema_version": 1_u64,
                    "accounts": empty_accounts,
                }),
                false,
            ),
            (
                "unknown-field-forest",
                json!({
                    "schema_version": 1_u64,
                    "unknown": vec![json!({"":[]}); 20_000],
                }),
                true,
            ),
        ]
    }

    #[test]
    fn live_extraction_and_resolution_peak_fit_the_config_charge() {
        config_funding::install();
        let mut failures = Vec::new();
        for (name, data, structural_rejection) in fixtures() {
            let envelope =
                serde_json::to_string(&BlobEnvelope::new(data, "2026-09-29T00:00:00Z".into()))
                    .unwrap();
            assert!(envelope.len() <= 1024 * 1024);
            let bytes = u64::try_from(envelope.len()).unwrap();
            let (peak, baked) = thread::spawn(move || {
                let baked_host = auctioneer_config_data::host_default();
                let baked_account = auctioneer_config_data::account_default();
                let baked = serde_json::to_vec(&(baked_host, baked_account))
                    .unwrap()
                    .len();
                let store = StoreRegistry::single_id(
                    "auctioneer_runtime".into(),
                    ConfigStoreBinding {
                        default_key: "auctioneer_runtime".into(),
                        handle: ConfigStoreHandle::new(Arc::new(Store(envelope.into()))),
                    },
                );
                let mut request = http::Request::builder()
                    .uri("/openrtb2/auction")
                    .body(Body::empty())
                    .unwrap();
                request.extensions_mut().insert(store);
                let ctx = funded_config::context(request);
                let raw_request =
                    serde_json::to_vec(&json!({"id":"r","imp":[],"site":{"publisher":{"id":"a"}},
                    "regs":{"gdpr":1_i32},"user":{"consent":"malformed"}}))
                    .unwrap();
                let req: BidRequest = serde_json::from_slice(&raw_request).unwrap();
                let gvl = GvlStore::for_test(vec![]);
                let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
                let before = alloc_track::reset_peak();
                let resolved_app = block_on(resolve_runtime_config(&ctx));
                if structural_rejection {
                    assert!(
                        matches!(&resolved_app,
                        Err(RuntimeConfigError::MalformedHost { detail })
                            if detail.contains("StructuralLimit")),
                        "{name}: {resolved_app:?}"
                    );
                    return (
                        alloc_track::peak_since(before),
                        u64::try_from(baked).unwrap(),
                    );
                }
                let app = resolved_app.unwrap();
                let host = resolve_host(&app, baked_host);
                let account =
                    resolve_account(&raw_request, &app, baked_account, 0x0001_0000).unwrap();
                let privacy = PrivacyContext::resolve(
                    &req,
                    &account.config,
                    &host,
                    &gvl,
                    &gcl,
                    OffsetDateTime::UNIX_EPOCH,
                    ResolveOptions {
                        gdpr_exempt: false,
                        no_sale: NoSale::default(),
                    },
                )
                .unwrap();
                let cost = alloc_track::peak_since(before);
                drop((privacy, account, host, app));
                (cost, u64::try_from(baked).unwrap())
            })
            .join()
            .unwrap();
            let margined = peak.checked_mul(6).unwrap().div_ceil(5);
            let charge = bytes
                .checked_add(baked)
                .unwrap()
                .checked_mul(CONFIG_PARSE_FACTOR)
                .unwrap();
            writeln!(
                io::stderr(),
                "{name}: wire={bytes} baked={baked} peak={peak} margin={margined} charge={charge}"
            )
            .unwrap();
            if margined > charge {
                failures.push(format!("{name}: {margined} > {charge}"));
            }
        }
        assert!(failures.is_empty(), "underfunded config work: {failures:?}");
    }
}

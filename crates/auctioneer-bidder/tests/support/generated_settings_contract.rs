//! Compiled only in the owned, fake-only settings workspace.

use auctioneer_runtime::test_support::alloc_track;
mod bounded_store;
mod config_funding_support;
use config_funding_support as config_funding;
mod fanout_limits;
mod generated_exchange_settings;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

mod allocation_meter {
    use super::alloc_track;
    use std::hint::black_box;

    #[test]
    fn canonical_allocator_tracks_allocations_without_refunding_deallocations() {
        let before = alloc_track::allocated();
        let buffer = black_box(Vec::<u8>::with_capacity(4096));
        let allocated = alloc_track::allocated();
        assert!(allocated - before >= 4096);
        drop(buffer);
        assert_eq!(alloc_track::allocated(), allocated);
    }
}

mod request_binding {
    use auctioneer_bidder::{BidderName, BidderRequestInfo, EntryPoint};
    use auctioneer_config::{BidderSettings, bidder_settings::fake_integer_probe};
    use serde_json::json;
    use std::ptr;

    fn settings_info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("fake_alias"),
            BidderName::new("fake_base"),
            "account",
            None,
            "https://bidder.example/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn settings_binding_refuses_each_wrong_or_missing_implementation_before_projection() {
        let mut cases = 0;
        let integer: BidderSettings =
            serde_json::from_value(json!({"implementation":"fake_integer_probe","values":{}}))
                .unwrap();
        let string: BidderSettings =
            serde_json::from_value(json!({"implementation":"fake_string_probe","values":{}}))
                .unwrap();
        for (expected, requested, base) in [
            (Some("fake_integer_probe"), Some(&string), None),
            (Some("fake_integer_probe"), None, Some(&string)),
            (Some("fake_integer_probe"), Some(&integer), Some(&string)),
            (Some("fake_integer_probe"), Some(&string), Some(&integer)),
            (Some("fake_alias"), Some(&integer), None),
            (None, Some(&integer), None),
            (None, None, Some(&integer)),
        ] {
            let mut info = settings_info();
            let error = info.set_settings(expected, requested, base).unwrap_err();
            assert_eq!(
                error,
                auctioneer_bidder::BidderError::failed_to_request_bids(
                    "bidder settings implementation mismatch"
                )
            );
            assert!(info.settings().is_none());
            assert!(info.base_settings().is_none());
            cases += 1;
        }
        assert_eq!(cases, 7);
    }

    #[test]
    fn settings_binding_borrows_both_entries_and_preserves_fieldwise_zero_precedence() {
        let base: BidderSettings = serde_json::from_value(
            json!({"implementation":"fake_integer_probe","values":{"primary_number":41_i64,"secondary_number":42_i64}}),
        )
        .unwrap();
        let alias: BidderSettings = serde_json::from_value(
            json!({"implementation":"fake_integer_probe","values":{"primary_number":0_i64}}),
        )
        .unwrap();
        let mut info = settings_info();
        info.set_settings(Some("fake_integer_probe"), Some(&alias), Some(&base))
            .unwrap();
        assert!(ptr::eq(info.settings().unwrap(), &raw const alias));
        assert!(ptr::eq(info.base_settings().unwrap(), &raw const base));
        let resolved = fake_integer_probe::resolve(
            info.settings()
                .and_then(BidderSettings::as_fake_integer_probe),
            info.base_settings()
                .and_then(BidderSettings::as_fake_integer_probe),
        );
        assert_eq!(resolved.primary_number(), Some(0));
        assert_eq!(resolved.secondary_number(), Some(42));
        info.set_settings(None, None, None).unwrap();
        assert!(info.settings().is_none());
        assert!(info.base_settings().is_none());
    }

    #[test]
    fn settings_binding_enforces_limits_for_programmatic_callers_and_clears_previous_binding() {
        let mut cases = 0;
        let small: BidderSettings = serde_json::from_value(
            json!({"implementation":"fake_string_probe","values":{"token":"host"}}),
        )
        .unwrap();
        let large: BidderSettings = serde_json::from_value(json!({"implementation":"fake_string_probe","values":{"token":"private-marker".repeat(2048)}})).unwrap();
        for (requested, base) in [
            (Some(&large), None),
            (None, Some(&large)),
            (Some(&small), Some(&large)),
        ] {
            let mut info = settings_info();
            info.set_settings(Some("fake_string_probe"), Some(&small), None)
                .unwrap();
            let error = info
                .set_settings(Some("fake_string_probe"), requested, base)
                .unwrap_err();
            assert_eq!(
                error,
                auctioneer_bidder::BidderError::failed_to_request_bids("bidder settings exceed limits")
            );
            assert!(info.settings().is_none());
            assert!(info.base_settings().is_none());
            cases += 1;
        }
        assert_eq!(cases, 3);
    }
}

mod retention {
    use super::alloc_track;
    use auctioneer_core::config::AuctioneerAppConfig;
    use serde_json::json;

    #[test]
    fn moving_protocol_settings_out_of_config_does_not_rebuild_the_map_or_clone_values() {
        use auctioneer_bidder::request_info::BidderSettingsSource as _;
        use auctioneer_core::config::bidder_settings::RuntimeBidderSettings;
        let mut cases = 0;
        for count in [1_usize, 8] {
            for width in [1, 64, 4_096] {
                let bidders: serde_json::Map<_,_> = (0..count).map(|index| (format!("fake_bidder_{index}"),json!({
                    "settings":{"implementation":"fake_string_probe","values":{"token":"x".repeat(width)}}
                }))).collect();
                let config: AuctioneerAppConfig =
                    serde_json::from_value(json!({"bidders":bidders})).unwrap();
                let original = config.bidders["fake_bidder_0"]
                    .settings
                    .as_ref()
                    .unwrap()
                    .as_fake_string_probe()
                    .unwrap()
                    .token
                    .as_ref()
                    .unwrap()
                    .as_ptr();
                let before = alloc_track::reset_peak();
                let retained = RuntimeBidderSettings::from(config);
                let measured = alloc_track::peak_since(before);
                assert_eq!(measured, 0);
                let supply = retained
                    .settings("fake_bidder_0")
                    .unwrap()
                    .as_fake_string_probe()
                    .unwrap()
                    .token
                    .as_ref()
                    .unwrap();
                assert_eq!(supply.as_ptr(), original);
                for index in 0..count {
                    assert_eq!(
                        retained
                            .settings(&format!("fake_bidder_{index}"))
                            .unwrap()
                            .as_fake_string_probe()
                            .unwrap()
                            .token
                            .as_ref()
                            .unwrap()
                            .len(),
                        width
                    );
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 6);
    }
}

mod retained_owner {
    #[cfg(test)]
    mod tests {
        use auctioneer_bidder::request_info::BidderSettingsSource as _;
        use auctioneer_config::{BidderSettings, bidder_settings::fake_string_probe};
        use auctioneer_core::config::{AuctioneerAppConfig, bidder_settings::RuntimeBidderSettings};
        use serde_json::json;

        #[test]
        fn retaining_settings_moves_the_original_buffers_and_drops_unrelated_entries() {
            let config: AuctioneerAppConfig = serde_json::from_value(json!({
            "bidders": {
                "fake_alias": {"settings": {"implementation":"fake_string_probe", "values":{"token":"alias-host"}}},
                "fake_base": {"settings": {"implementation":"fake_string_probe", "values":{"token":"base-host"}}},
                "fake_unrelated": {"enabled": false}
            },
                "accounts": {"fake_discarded":{"targeting_prefix":"discarded"}}
        })).unwrap();
            let original = config.bidders["fake_alias"]
                .settings
                .as_ref()
                .unwrap()
                .as_fake_string_probe()
                .unwrap()
                .token
                .as_ref()
                .unwrap()
                .as_ptr();
            let retained = RuntimeBidderSettings::from(config);
            assert_eq!(
                ["fake_alias", "fake_base", "fake_unrelated"]
                    .into_iter()
                    .filter(|name| retained.settings(name).is_some())
                    .count(),
                2
            );
            assert!(retained.settings("fake_unrelated").is_none());
            assert!(retained.settings("fake_missing").is_none());
            let view = fake_string_probe::resolve(
                retained
                    .settings("fake_alias")
                    .and_then(BidderSettings::as_fake_string_probe),
                retained
                    .settings("fake_base")
                    .and_then(BidderSettings::as_fake_string_probe),
            );
            assert_eq!(view.token(), Some("alias-host"));
            assert_eq!(view.token().unwrap().as_ptr(), original);
        }
    }
}

mod config_limits {
    use auctioneer_config::BidderSettings;
    use serde_json::{Value, json};

    fn decode(value: Value) -> Result<BidderSettings, serde_json::Error> {
        serde_json::from_value(value)
    }

    #[test]
    fn typed_settings_byte_ceiling_includes_the_envelope_and_json_escaping() {
        use auctioneer_config::bidder_settings::ensure_limits;
        let empty =
            decode(json!({"implementation":"fake_string_probe","values":{"token":""}})).unwrap();
        let overhead = auctioneer_runtime::serialized_json_bytes(&empty).unwrap();
        let width = usize::try_from(0x4000_u64 - overhead).unwrap();
        let mut cases = 0;
        for (extra, expected) in [(0, Ok(())), (1, Err("bidder settings exceed byte limit"))] {
            let settings = decode(json!({"implementation":"fake_string_probe","values":{"token":"x".repeat(width + extra)}})).unwrap();
            assert_eq!(ensure_limits(&settings), expected);
            cases += 1;
        }
        assert_eq!(cases, 2);
        let escaped = decode(
            json!({"implementation":"fake_string_probe","values":{"token":"\n".repeat(width)}}),
        )
        .unwrap();
        assert_eq!(
            ensure_limits(&escaped),
            Err("bidder settings exceed byte limit")
        );
    }
}

mod runtime {
    use super::{bounded_store::bounded_over_get, config_funding};
    use async_trait::async_trait;
    use edgezero_core::{
        blob_envelope::BlobEnvelope,
        body::Body,
        config_store::{
            BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
        },
        context::RequestContext,
        http::StatusCode,
        params::PathParams,
        store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry},
        time::{Deadline, MonotonicClock},
    };
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use auctioneer_core::{
        config::runtime::{RuntimeConfigError, resolve_runtime_config},
        handlers::TestFanoutHandle,
        host_primitives::{TestClockHandle, TestRngHandle},
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, TimeoutCause,
    };
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use std::{
        sync::{Arc, Mutex, PoisonError},
        time::Duration,
    };
    use time::OffsetDateTime;

    const STORE_ID: &str = "auctioneer_runtime";
    const WIDE_CAP: usize = 1 << 20;

    struct SettingsStore(serde_json::Value);

    #[async_trait(?Send)]
    impl ConfigStore for SettingsStore {
        async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            Ok(Some(
                serde_json::to_string(&BlobEnvelope::new(
                    self.0.clone(),
                    "1970-01-01T00:00:00Z".into(),
                ))
                .unwrap()
                .into(),
            ))
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

    #[test]
    fn malformed_generated_settings_fail_closed_without_payload_text_in_diagnostics() {
        use serde_json::json;
        const CANARY: &str = "private-settings-canary";
        let mut cases = 0;
        for settings in [
            json!({"implementation":CANARY,"values":{}}),
            json!({"implementation":"fake_integer_probe","values":{"primary_number":CANARY}}),
            json!({"implementation":"fake_integer_probe","values":{(CANARY):true}}),
            json!({"implementation":"fake_string_probe","values":{"token":CANARY.repeat(2048)}}),
        ] {
            let fanout = Arc::new(RecordingFanout::default());
            let ctx = context_with(
                Arc::new(SettingsStore(
                    json!({"bidders":{"fake_bidder":{"settings":settings}}}),
                )),
                Arc::clone(&fanout),
            );
            let error = block_on(resolve_runtime_config(&ctx)).unwrap_err();
            assert!(matches!(error, RuntimeConfigError::MalformedHost { .. }));
            assert!(!format!("{error:?}").contains(CANARY));
            let response = error.into_response().unwrap();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let bytes = block_on(response.into_body().into_bytes_bounded(WIDE_CAP)).unwrap();
            assert_eq!(bytes.as_ref(), b"Malformed host configuration");
            assert_eq!(*fanout.calls.lock().unwrap(), 0);
            cases += 1;
        }
        assert_eq!(cases, 4);
    }

    fn registry_of(store: Arc<dyn ConfigStore>) -> ConfigRegistry {
        StoreRegistry::single_id(
            STORE_ID.to_owned(),
            ConfigStoreBinding {
                default_key: STORE_ID.to_owned(),
                handle: ConfigStoreHandle::new(store),
            },
        )
    }

    #[derive(Default)]
    struct RecordingFanout {
        calls: Mutex<usize>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut seen = self.calls.lock().unwrap_or_else(PoisonError::into_inner);
            *seen = seen.saturating_add(calls.len());
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    fn context_with(store: Arc<dyn ConfigStore>, fanout: Arc<RecordingFanout>) -> RequestContext {
        config_funding::install();
        let handler_fanout: Arc<dyn BidderFanout + Send + Sync> = fanout;
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(
                serde_json::to_vec(&serde_json::json!({"id":"fake_settings", "imp":[]})).unwrap(),
            ))
            .expect("request builds");
        request.extensions_mut().insert(registry_of(store));
        request
            .extensions_mut()
            .insert(TestFanoutHandle(handler_fanout));
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        RequestContext::new(request, PathParams::default())
    }
}

mod generated_endpoints {
    use auctioneer_bidder::{
        EndpointBuilder, META_BIDDER_REGION_BYTES, META_BYTES_CAPACITY, OutboundRequestMetaBuilder,
        SinkAbort,
    };
    use auctioneer_config::bidder_endpoint::fake_string_probe::Values;

    fn typed_info<'a>(
        endpoint: &'a str,
        macros: &'a [&'a str],
    ) -> auctioneer_bidder::BidderRequestInfo<'a> {
        auctioneer_bidder::BidderRequestInfo::new(
            auctioneer_bidder::BidderName::new("fake_requested_alias"),
            auctioneer_bidder::BidderName::new("fake_base"),
            "account",
            None,
            endpoint,
            macros,
            None,
            None,
            auctioneer_bidder::EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn generated_bindings_encode_values_through_the_existing_capped_builder() {
        let request_info = typed_info("https://bidder.example.test/{{.ZuluRoute}}", &["ZuluRoute"]);
        let values = Values {
            a_route: "a b/&?=#+%",
            z_token: "unused",
        };
        let pairs = values.as_pairs();
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
        meta.set_method("POST").unwrap();
        EndpointBuilder::from_request(&request_info, &pairs)
            .expand_into(&mut meta)
            .unwrap();
        assert_eq!(
            meta.finish().url(),
            "https://bidder.example.test/a%20b%2F%26%3F%3D%23%2B%25"
        );
    }

    #[test]
    fn generated_bindings_cannot_relax_template_membership_or_traversal_refusals() {
        let mut cases = 0;
        for (macros, template, value) in [
            (
                &[][..],
                "https://bidder.example.test/{{.ZuluRoute}}",
                "safe",
            ),
            (
                &["ZuluRoute"][..],
                "https://bidder.example.test/{{.Other}}",
                "safe",
            ),
            (
                &["ZuluRoute"][..],
                "https://bidder.example.test/{{.ZuluRoute}}",
                "../private",
            ),
            (
                &["ZuluRoute"][..],
                "https://bidder.example.test/{{.ZuluRoute}}",
                "%252e%252e%252fprivate",
            ),
        ] {
            let request_info = typed_info(template, macros);
            let pairs = Values {
                a_route: value,
                z_token: "unused",
            }
            .as_pairs();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            assert_eq!(
                EndpointBuilder::from_request(&request_info, &pairs).expand_into(&mut meta),
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap()
                })
            );
            assert_eq!(meta.written(), 4);
            meta.set_url("https://recovery.example.test/").unwrap();
            assert_eq!(meta.finish().url(), "https://recovery.example.test/");
            cases += 1;
        }
        assert_eq!(cases, 4);
    }

    #[test]
    fn query_set_validates_borrowed_macros_even_in_values_it_would_remove() {
        let request_info = typed_info(
            "https://bidder.example.test/path?member_id={{.ZuluRoute}}",
            &["ZuluRoute"],
        );
        let mut cases = 0;
        for value in ["../private", "%252e%252e%252fprivate"] {
            let pairs = Values {
                a_route: value,
                z_token: "unused",
            }
            .as_pairs();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            assert_eq!(
                EndpointBuilder::from_request(&request_info, &pairs).expand_with_query_set_into(
                    &mut meta,
                    "member_id",
                    "42"
                ),
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                })
            );
            assert_eq!(meta.written(), 4);
            cases += 1;
        }
        assert_eq!(cases, 2);
    }
}

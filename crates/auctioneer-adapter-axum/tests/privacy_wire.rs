//! Handler-to-socket disclosure checks using the shipped local bidder, boot
//! envelope, timer and outbound client. Only the runtime config store is a fixture.

#[cfg(test)]
mod tests {
    use auctioneer_adapter_axum::{envelope::resource_profile, timer::TokioTimer};
    use auctioneer_bidder::test_support::serving_catalog;
    use auctioneer_config::types::MediaType;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::host_primitives::{
        DevRng, SystemClock, axum_production_strategy, boot_gates, bootstrap_adapter_with_envelope,
        stream_gate,
    };
    use auctioneer_runtime::{Clock as _, NoopDecodeCooperator};
    use axum::Router;
    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::{HeaderMap, Method, Request, StatusCode};
    use axum::routing::post;
    use edgezero_adapter_axum::outbound::AxumOutboundClient;
    use edgezero_core::app::App;
    use edgezero_core::blob_envelope::BlobEnvelope;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{
        BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
    };
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::outbound::{HttpClient, OutboundTime};
    use edgezero_core::store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
    use serde_json::{Value, json};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::TcpListener;
    use tokio::sync::{mpsc, oneshot};
    use tokio::task::LocalSet;
    use tokio::time::{sleep, timeout};

    const WAIT: Duration = Duration::from_secs(10);
    const BIDDER_ALIAS: &str = "fake_privacy_wire";
    const ID_PATHS: &[&str] = &[
        "/user/id",
        "/user/buyeruid",
        "/user/yob",
        "/user/gender",
        "/user/eids",
        "/user/ext/eids",
        "/device/ifa",
    ];
    const FPD_PATHS: &[&str] = &[
        "/user/keywords",
        "/user/kwarray",
        "/user/data",
        "/user/ext/data",
    ];
    const OPAQUE_PATHS: &[&str] = &[
        "/user/sha256email",
        "/device/uid2",
        "/device/ext/context",
        "/device/geo/ext",
        "/user/geo/ext",
    ];
    const GEO_PATHS: &[&str] = &["/device/geo/zip", "/user/geo/zip"];
    const TID_PATHS: &[&str] = &["/source/tid", "/imp/0/ext/tid"];
    const CONTEXT_PATHS: &[&str] = &[
        "/id",
        "/imp/0/id",
        "/site/publisher/id",
        "/device/make",
        "/device/model",
        "/device/h",
        "/device/w",
        "/device/language",
        "/device/ext/prebid/interstitial",
        "/device/geo/country",
        "/device/geo/region",
        "/device/geo/city",
        "/user/geo/country",
        "/user/geo/city",
        "/regs/us_privacy",
    ];

    #[derive(Clone, Copy, Debug)]
    struct WirePolicy {
        enforce_ccpa: bool,
        transmit_ufpd: bool,
        transmit_precise_geo: bool,
        transmit_tid: bool,
    }

    struct FixedStore(ConfigValue);

    #[async_trait::async_trait(?Send)]
    impl ConfigStore for FixedStore {
        async fn get(&self, key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
            Ok((key == "auctioneer_runtime").then(|| self.0.clone()))
        }

        async fn get_bounded(
            &self,
            key: &str,
            clock: &MonotonicClock,
            deadline: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
            if deadline.is_expired_at(clock.now()) {
                return Err(ConfigStoreError::DeadlineExceeded);
            }
            let backend_bytes = u64::try_from(self.0.len())
                .map_err(|_length_error| ConfigStoreError::ValueTooLarge)?;
            if backend_bytes > max_backend_bytes || backend_bytes > max_value_bytes {
                return Err(ConfigStoreError::ValueTooLarge);
            }
            let value = self.get(key).await?;
            if deadline.is_expired_at(clock.now()) {
                return Err(ConfigStoreError::DeadlineExceeded);
            }
            Ok(BoundedStoreRead {
                backend_bytes,
                value,
            })
        }
    }

    struct CapturedRequest {
        headers: HeaderMap,
        body: Vec<u8>,
    }

    async fn capture(
        State(sender): State<mpsc::Sender<CapturedRequest>>,
        headers: HeaderMap,
        body: Bytes,
    ) -> StatusCode {
        sender
            .send(CapturedRequest {
                headers,
                body: body.to_vec(),
            })
            .await
            .expect("capture receiver stays alive until every request finishes");
        StatusCode::NO_CONTENT
    }

    fn fixture() -> Value {
        json!({
            "id": "privacy-wire", "tmax": 5_000_i64, "cur": ["USD"],
            "site": {"publisher": {"id": "wire-account"}},
            "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"bidder": {(BIDDER_ALIAS): {}}, "tid": "private-imp-tid"}}],
            "user": {"id": "private-user-id", "buyeruid": "private-buyer-id",
                "yob": 2000_i32, "gender": "F", "sha256email": "private-email-hash",
                "eids": [{"source": "ids.example", "uids": [{"id": "private-eid"}]}],
                "keywords": "private-keywords", "kwarray": ["private-keyword"],
                "data": [{"id": "private-user-data"}],
                "ext": {"eids": [{"source": "ids.example", "uids": [{"id": "private-ext-eid"}]}],
                    "data": {"segment": "private-ext-data"}},
                "geo": {"lat": 20.567_89_f64, "lon": -1.238_f64, "country": "USA",
                    "city": "Oakland", "zip": "private-user-zip",
                    "ext": {"location": "private-user-location"}}},
            "device": {"ip": "8.8.4.123", "ipv6": "2001:4860:4860::8844",
                "ifa": "private-ifa", "uid2": "private-device-id",
                "make": "Context maker", "model": "Context model",
                "h": 2556_i64, "w": 1179_i64, "language": "en",
                "ext": {"context": "private-device-context",
                    "prebid": {"interstitial": {"minwidthperc": 75_i32, "minheightperc": 80_i32}}},
                "geo": {"lat": 48.858_844_f64, "lon": -1.125_f64, "country": "USA",
                    "region": "CA", "city": "San Francisco", "zip": "private-device-zip",
                    "ext": {"location": "private-device-location"}}},
            "source": {"tid": "private-source-tid"},
            "regs": {"us_privacy": "1NYN"}
        })
    }

    fn request(
        fixture: &Value,
        policy: WirePolicy,
        client: &HttpClient,
        bidder: &str,
    ) -> Request<Body> {
        let config_json = json!({
            "aliases": {(BIDDER_ALIAS): bidder},
            "gdpr": {"enabled": false}, "ccpa": {"enforce": policy.enforce_ccpa},
            "lmt": {"enforce": false},
            "features": {"gzip_outbound": false},
            "accounts": {"wire-account": {"privacy": {
                "ipv4": {"anon_keep_bits": 24_i32},
                "ipv6": {"anon_keep_bits": 64_i32},
                "allowactivities": {
                    "transmitUfpd": {"default": policy.transmit_ufpd},
                    "transmitPreciseGeo": {"default": policy.transmit_precise_geo},
                    "transmitTid": {"default": policy.transmit_tid}
                }
            }}}
        });
        let config: AuctioneerAppConfig =
            serde_json::from_value(config_json).expect("fixture uses the real config DTO");
        let envelope = BlobEnvelope::new(
            serde_json::to_value(config).expect("config DTO serializes"),
            "1970-01-01T00:00:00Z".to_owned(),
        );
        let value = serde_json::to_string(&envelope).expect("config envelope serializes");
        let registry: ConfigRegistry = StoreRegistry::single_id(
            "auctioneer_runtime".to_owned(),
            ConfigStoreBinding {
                default_key: "auctioneer_runtime".to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(FixedStore(value.into()))),
            },
        );
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/openrtb2/auction")
            .header("content-type", "application/json")
            .header("authorization", "Bearer private-browser-token")
            .header("cookie", "browser=private-browser-cookie")
            .header("x-forwarded-for", "8.8.4.123")
            .header("x-real-ip", "8.8.4.123")
            .header("sec-gpc", "1")
            .body(Body::from_bytes(
                serde_json::to_vec(fixture).expect("request serializes"),
            ))
            .expect("request builds");
        request.extensions_mut().insert(registry);
        request.extensions_mut().insert(client.clone());
        request
    }

    fn assert_field_disclosure(body: &Value, original: &Value, paths: &[&str], deny: bool) {
        for path in paths {
            assert!(
                original.pointer(path).is_some(),
                "before-state lacks {path}"
            );
            if deny {
                assert!(body.pointer(path).is_none(), "wire leak at {path}");
            } else {
                assert_eq!(body.pointer(path), original.pointer(path), "allow {path}");
            }
        }
    }

    fn assert_wire_disclosure(captured: &CapturedRequest, original: &Value, policy: WirePolicy) {
        let body: Value = serde_json::from_slice(&captured.body).expect("wire body is JSON");
        for path in CONTEXT_PATHS {
            assert_eq!(body.pointer(path), original.pointer(path), "context {path}");
        }
        let deny_ids = policy.enforce_ccpa || !policy.transmit_ufpd;
        let deny_geo = policy.enforce_ccpa || !policy.transmit_precise_geo;
        assert_field_disclosure(&body, original, ID_PATHS, deny_ids);
        assert_field_disclosure(&body, original, FPD_PATHS, !policy.transmit_ufpd);
        assert_field_disclosure(&body, original, OPAQUE_PATHS, deny_ids || deny_geo);
        assert_field_disclosure(&body, original, GEO_PATHS, deny_geo);
        assert_field_disclosure(&body, original, TID_PATHS, !policy.transmit_tid);
        if !policy.transmit_ufpd && deny_geo && !policy.transmit_tid {
            assert!(
                !String::from_utf8_lossy(&captured.body).contains("private-"),
                "fully denied markers must not survive elsewhere on the wire"
            );
        }
        if !deny_ids && !deny_geo {
            assert_eq!(body["device"]["geo"], original["device"]["geo"]);
            assert_eq!(body["user"]["geo"], original["user"]["geo"]);
        }
        if deny_geo {
            assert_eq!(body["device"]["ip"], "8.8.4.0");
            assert_eq!(body["device"]["ipv6"], "2001:4860:4860::");
            assert_eq!(body["device"]["geo"]["lat"], json!(48.86_f64));
            assert_eq!(body["device"]["geo"]["lon"], json!(-1.12_f64));
            assert_eq!(body["user"]["geo"]["lat"], json!(20.57_f64));
            assert_eq!(body["user"]["geo"]["lon"], json!(-1.24_f64));
        } else {
            assert_eq!(body["device"]["ip"], original["device"]["ip"]);
            assert_eq!(body["device"]["ipv6"], original["device"]["ipv6"]);
            for path in [
                "/device/geo/lat",
                "/device/geo/lon",
                "/user/geo/lat",
                "/user/geo/lon",
            ] {
                assert_eq!(body.pointer(path), original.pointer(path), "allow {path}");
            }
        }
        for name in ["authorization", "cookie", "x-forwarded-for", "x-real-ip"] {
            assert!(
                !captured.headers.contains_key(name),
                "wire header leak: {name}"
            );
        }
        assert_eq!(captured.headers["content-type"], "application/json");
        assert_eq!(captured.headers["sec-gpc"], "1");
        assert!(!captured.headers.contains_key("content-encoding"));
    }

    fn boot_outbound_client() -> HttpClient {
        let clock = Arc::new(SystemClock::new());
        let profile = resource_profile();
        bootstrap_adapter_with_envelope(
            profile.boot_envelope(),
            || Arc::<SystemClock>::clone(&clock),
            || Arc::new(DevRng::new()),
            || Arc::new(TokioTimer),
            || Arc::new(NoopDecodeCooperator),
            axum_production_strategy,
        )
        .expect("the shipped adapter boot installs the real wave path");
        let time = OutboundTime::new(
            MonotonicClock::new(move || clock.now_monotonic()),
            |deadline, paired_clock| {
                Box::pin(async move {
                    sleep(
                        deadline
                            .remaining_at(paired_clock.now())
                            .unwrap_or_default(),
                    )
                    .await;
                    Ok(())
                })
            },
        );
        HttpClient::with_client(
            AxumOutboundClient::try_with_time(time).expect("native outbound client builds"),
        )
    }

    fn configured_app() -> App {
        let app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default()).unwrap();
        let funded = &boot_gates().unwrap().limits;
        assert_eq!(
            app.config_extraction_limits().max_backend_bytes,
            funded.max_runtime_config_bytes
        );
        assert_eq!(
            app.config_extraction_limits().max_blob_bytes,
            funded.max_runtime_config_blob_bytes
        );
        assert_eq!(
            app.config_extraction_limits().max_json_nodes,
            funded.max_config_json_nodes
        );
        app
    }

    async fn dispatch(app: &App, request: Request<Body>) -> Vec<u8> {
        let dispatched = timeout(
            WAIT,
            app.dispatch_ingress(
                request,
                MonotonicInstant::now(),
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        let IngressDispatchOutcome::Response(mut egress) = dispatched else {
            panic!("the configured App has output capacity");
        };
        let response = egress.response_mut();
        let response_bytes = response.body().as_bytes().unwrap().to_vec();
        assert!(response_bytes.len() <= 0x0010_0000);
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&response_bytes)
        );
        drop(egress);
        response_bytes
    }

    #[tokio::test]
    async fn handler_privacy_denials_and_allowed_context_reach_the_real_bidder_socket() {
        LocalSet::new()
            .run_until(Box::pin(privacy_wire_scenario()))
            .await;
    }

    fn assert_complete_disclosure_fixture(original: &Value) {
        for path in ID_PATHS
            .iter()
            .chain(FPD_PATHS)
            .chain(OPAQUE_PATHS)
            .chain(GEO_PATHS)
            .chain(TID_PATHS)
            .chain(CONTEXT_PATHS)
        {
            assert!(
                original.pointer(path).is_some(),
                "before-state lacks {path}"
            );
        }
    }

    async fn privacy_wire_scenario() {
        let local_bidders: Vec<_> = serving_catalog::default_active_publications()
            .into_iter()
            .filter(|row| {
                let info = row.info;
                !info.disabled
                    && !info.uses_https
                    && info.endpoint_macros.is_empty()
                    && info.endpoint_compression.is_none()
                    && info.capabilities.site.contains(&MediaType::Banner)
                    && row.owner_expected_request_body.as_ref() == Some(&row.owner_request)
                    && url::Url::parse(&info.endpoint).is_ok_and(|endpoint| {
                        endpoint.scheme() == "http"
                            && endpoint.host_str() == Some("127.0.0.1")
                            && endpoint.port().is_some()
                    })
            })
            .collect();
        assert_eq!(
            local_bidders.len(),
            1,
            "one default-active loopback passthrough is required"
        );
        let bidder = &local_bidders[0];
        let endpoint = url::Url::parse(&bidder.info.endpoint).unwrap();
        assert_eq!(endpoint.scheme(), "http");
        assert_eq!(endpoint.host_str(), Some("127.0.0.1"));
        let listener = TcpListener::bind((
            "127.0.0.1",
            endpoint.port().expect("the local bidder declares its port"),
        ))
        .await
        .expect("the shipped local bidder port must be free; never skip this test");
        let (sender, mut receiver) = mpsc::channel(1);
        let router = Router::new()
            .route(endpoint.path(), post(capture))
            .with_state(sender);
        let (shutdown, stopped) = oneshot::channel();
        let server = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _stopped = stopped.await;
                })
                .await
                .expect("fixture server runs");
        });

        let client = boot_outbound_client();
        let app = configured_app();
        let original = fixture();
        assert_complete_disclosure_fixture(&original);

        for mask in 0_u8..16 {
            let policy = WirePolicy {
                enforce_ccpa: mask & 1 != 0,
                transmit_ufpd: mask & 2 == 0,
                transmit_precise_geo: mask & 4 == 0,
                transmit_tid: mask & 8 == 0,
            };
            let response_bytes =
                dispatch(&app, request(&original, policy, &client, bidder.name())).await;
            let response_json: Value =
                serde_json::from_slice(&response_bytes).expect("auction JSON");
            assert!(
                response_json
                    .pointer("/ext/errors")
                    .is_none_or(|errors| errors.as_object().is_some_and(serde_json::Map::is_empty)),
                "successful transport must not be an empty auction masking an error: {response_json}"
            );
            let captured = timeout(WAIT, receiver.recv())
                .await
                .expect("a real socket request is required")
                .expect("capture channel stays open");
            assert_wire_disclosure(&captured, &original, policy);
            assert_eq!(
                stream_gate()
                    .expect("boot installed the stream gate")
                    .in_flight(),
                0,
                "the handler releases real-transport permits"
            );
        }
        assert!(
            receiver.try_recv().is_err(),
            "exactly sixteen requests must be sent"
        );
        shutdown.send(()).expect("fixture server is still running");
        timeout(WAIT, server)
            .await
            .expect("fixture server shuts down within the watchdog")
            .expect("fixture server does not panic");
    }
}

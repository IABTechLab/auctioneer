#[cfg(test)]
mod fanout_limits;
use fanout_limits as envelope;
#[cfg(test)]
mod support;

#[cfg(test)]
use auctioneer_core::test_support::usersync_keys;

#[cfg(test)]
mod tests {
    use super::{envelope, support, usersync_keys};
    use auctioneer_core::test_support::live_catalog;

    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::fmt::Write as _;
    use std::mem;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use auctioneer_core::auction::audit::construction_costs as core_construction_costs;
    use auctioneer_core::config::runtime::CONFIG_STATUS_HEADER;
    use auctioneer_core::config::{
        AuctioneerAppConfig, MAX_ALIAS_NAME_BYTES, MAX_ALIASES, validate_effective_defaults,
    };
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, boot_gates,
        bootstrap_adapter_with_envelope, clock_handle,
    };
    use auctioneer_runtime::aux_work::{
        MAX_CATALOG_IDENTITIES, MAX_SYNC_DEBUG_ROWS, MAX_SYNC_REQUESTED_NAMES,
    };
    use auctioneer_runtime::body_caps::{COOKIESYNC_CAP_BYTES, DEFAULT_CAP_BYTES};
    use auctioneer_runtime::budget::derive_aux_work_for_profile;
    use auctioneer_runtime::{
        FakeClock, ImmediateResolveTimer, NoopDecodeCooperator, TargetPlatformConstants,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::{BoundedStoreRead, ConfigExtractionLimits};
    use edgezero_core::http::{Method, Request, Response, StatusCode, request_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::response_egress::ResponseEgressEnvelope;
    use edgezero_core::secret_store::{SecretError, SecretHandle, SecretStore};
    use edgezero_core::store_registry::{BoundSecretStore, SecretRegistry, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use futures::stream;
    use serde_json::Value;
    use std::task::Poll;
    use time::OffsetDateTime;

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    const ORIGIN: &str = "https://publisher.example";
    const SYNC_CONFIG: &str =
        "schema_version = 1\n[host.user_sync]\nexternal_url = 'https://sync.example.test'\n";
    const BROWSER_CONFIG: &str = "schema_version = 1\n[host.user_sync]\nexternal_url = \"https://sync.example.test\"\nallowed_origins = ['https://publisher.example']\n";
    const COOKIE_NOW: OffsetDateTime = time::macros::datetime!(2026-09-30 12:00:00.100000001 UTC);

    struct SigningStore;

    #[async_trait::async_trait(?Send)]
    impl SecretStore for SigningStore {
        async fn get_bytes(
            &self,
            store_name: &str,
            key: &str,
        ) -> Result<Option<bytes::Bytes>, SecretError> {
            assert_eq!(store_name, "default");
            assert_eq!(key, "usersync_cookie_keyring");
            Ok(Some(bytes::Bytes::from_static(
                usersync_keys::SECRET_JSON.as_bytes(),
            )))
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
            let backend_bytes = u64::try_from(usersync_keys::SECRET_JSON.len()).unwrap();
            if backend_bytes > max_backend_bytes || backend_bytes > max_value_bytes {
                return Err(SecretError::ValueTooLarge);
            }
            if deadline.is_expired_at(clock.now()) {
                return Err(SecretError::DeadlineExceeded);
            }
            Ok(BoundedStoreRead {
                backend_bytes,
                value,
            })
        }
    }

    fn app() -> App {
        let mut limits = envelope::limits();
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
            auctioneer_config_data::host_default(),
            auctioneer_config_data::account_default(),
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
            || Arc::new(FakeClock::new(COOKIE_NOW)),
            || Arc::new(DevRng::new()),
            || Arc::new(ImmediateResolveTimer),
            || Arc::new(NoopDecodeCooperator),
            || Arc::new(DelegateToDevStrategy),
        )
        .expect("boot envelope");
        let gates = boot_gates().expect("booted gates");
        assert_eq!(
            gates.aux.in_use(),
            0,
            "routed setup found a held aux permit"
        );
        assert_eq!(
            gates.aux_work_budget.in_use(),
            0,
            "routed setup found a held aux reservation"
        );
        assert_eq!(
            gates.egress.in_use(),
            0,
            "routed setup found held auction egress"
        );
        assert_eq!(
            gates.static_egress.in_use(),
            0,
            "routed setup found held static egress"
        );
        App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
            .expect("the manifest builds")
    }

    struct DebugFixture {
        config: String,
        bidders: Vec<String>,
    }

    fn debug_fixture(compiled: &[String]) -> Result<DebugFixture, &'static str> {
        let catalog_cap = usize::try_from(MAX_CATALOG_IDENTITIES)
            .map_err(|_error| "catalog owner is not representable")?;
        let requested_cap = usize::try_from(MAX_SYNC_REQUESTED_NAMES)
            .map_err(|_error| "requested-name owner is not representable")?;
        let remaining_catalog = catalog_cap
            .checked_sub(compiled.len())
            .ok_or("compiled catalog exceeds the catalog-identity owner")?;
        let alias_count = MAX_ALIASES.min(remaining_catalog);
        let compiled_slots = requested_cap
            .checked_sub(alias_count)
            .ok_or("fixture aliases exceed the requested-name owner")?;
        let mut sorted = compiled.iter().collect::<Vec<_>>();
        sorted.sort_unstable();
        let mut bidders = sorted
            .into_iter()
            .take(compiled_slots.min(32))
            .cloned()
            .collect::<Vec<_>>();
        let mut config = format!("{BROWSER_CONFIG}[host.aliases]\n");
        for index in 0..alias_count {
            let name = format!("fake_diagnostic_{index:03}_{}", "x".repeat(44));
            if name.len() != MAX_ALIAS_NAME_BYTES {
                return Err("fixture alias has the wrong byte length");
            }
            writeln!(config, "{name} = '{}'", live_catalog::redirect().name())
                .map_err(|_error| "fixture configuration cannot be formatted")?;
            bidders.push(name);
        }
        // Keep room for long unknown-name diagnostics even with a full catalog.
        let unknown_count = requested_cap
            .checked_sub(bidders.len())
            .ok_or("fixture bidders exceed the requested-name owner")?;
        // Reserve wire punctuation space when compiled names are also 128 bytes.
        let long_unknowns = unknown_count
            .checked_sub(16)
            .ok_or("fixture lacks its short-name wire reserve")?;
        for index in 0..unknown_count {
            let (padding, expected_bytes) = if index < long_unknowns {
                (116, 128)
            } else {
                (0, 12)
            };
            let name = format!("fake_un_{index:03}_{}", "x".repeat(padding));
            if name.len() != expected_bytes {
                return Err("fixture unknown name has the wrong byte length");
            }
            bidders.push(name);
        }
        let debug_cap = usize::try_from(MAX_SYNC_DEBUG_ROWS)
            .map_err(|_error| "debug-row owner is not representable")?;
        if bidders.len() > debug_cap {
            return Err("fixture bidders exceed the debug-row owner");
        }
        Ok(DebugFixture { config, bidders })
    }

    fn diagnostic_payload(bidders: &[String]) -> Value {
        serde_json::json!({
            "bidders": bidders,
            "gdpr": 0_i32,
            "coopSync": false,
            "debug": true,
            "filterSettings": {
                "image": {"filter": "exclude", "bidders": "*"},
                "iframe": {"filter": "exclude", "bidders": "*"},
            },
        })
    }

    #[test]
    fn debug_fixture_keeps_256_requested_rows_for_small_193_and_full_512_catalogs() {
        assert_eq!(MAX_SYNC_REQUESTED_NAMES, 256);
        assert_eq!(MAX_SYNC_DEBUG_ROWS, 256);
        assert_eq!(MAX_CATALOG_IDENTITIES, 512);
        assert_eq!(MAX_ALIASES, 64);
        for (count, expected_aliases, name_bytes) in [
            (3_usize, 64_usize, 64_usize),
            (193, 64, 64),
            (512, 0, 64),
            (512, 0, 128),
        ] {
            let compiled = (0..count)
                .map(|index| {
                    let prefix = format!("fake_compiled_{index:03}_");
                    format!(
                        "{prefix}{}",
                        "x".repeat(name_bytes.checked_sub(prefix.len()).unwrap())
                    )
                })
                .collect::<Vec<_>>();
            let DebugFixture { config, bidders } = debug_fixture(&compiled).unwrap();
            let parsed: AuctioneerAppConfig = toml::from_str(&config).unwrap();
            let aliases = parsed.host.aliases.as_ref().unwrap();
            assert_eq!(aliases.len(), expected_aliases);
            assert_eq!(bidders.len(), 256);
            assert_eq!(bidders.iter().collect::<BTreeSet<_>>().len(), 256);
            assert!(compiled.len() + aliases.len() <= 512);
            assert_eq!(
                bidders
                    .iter()
                    .filter(|name| compiled.contains(name))
                    .count(),
                count.min(32)
            );
            assert!(bidders.iter().any(|name| name.starts_with("fake_un_")));
            let payload = diagnostic_payload(&bidders);
            assert!(serde_json::to_vec(&payload).unwrap().len() <= 32 * 1_024);
            let mut reversed = compiled;
            reversed.reverse();
            assert_eq!(debug_fixture(&reversed).unwrap().bidders, bidders);
        }
    }

    #[test]
    fn debug_fixture_reports_exceeded_catalog_owner_without_subtraction_panic() {
        let compiled = (0_usize..513)
            .map(|index| format!("fake_compiled_{index:03}"))
            .collect::<Vec<_>>();
        assert!(matches!(
            debug_fixture(&compiled),
            Err("compiled catalog exceeds the catalog-identity owner")
        ));
    }

    #[test]
    fn routed_cookie_fixture_uses_a_fixed_nanosecond_clock() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let _app = app();
        let clock = clock_handle();
        for _ in 0_usize..16 {
            assert_eq!(clock.now(), COOKIE_NOW);
        }
    }

    fn dispatch(app: &App, method: Method, path: &str, body: Body) -> Response {
        let request = request_builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .body(body)
            .expect("request builds");
        dispatch_request(app, request)
    }

    fn dispatch_request(app: &App, request: Request) -> Response {
        let mut envelope = dispatch_envelope(app, request);
        mem::replace(envelope.response_mut(), Response::new(Body::empty()))
    }

    fn dispatch_envelope(app: &App, request: Request) -> ResponseEgressEnvelope {
        dispatch_envelope_at(app, request, MonotonicInstant::now())
    }

    fn dispatch_envelope_at(
        app: &App,
        request: Request,
        request_start: MonotonicInstant,
    ) -> ResponseEgressEnvelope {
        dispatch_configured_envelope_at(app, request, request_start, SYNC_CONFIG)
    }

    fn dispatch_configured_envelope_at(
        app: &App,
        mut request: Request,
        request_start: MonotonicInstant,
        config: &str,
    ) -> ResponseEgressEnvelope {
        let signing: Arc<dyn SecretStore> = Arc::new(SigningStore);
        let registry: SecretRegistry = StoreRegistry::single_id(
            "default".to_owned(),
            BoundSecretStore::new(SecretHandle::new(signing), "default".to_owned()),
        );
        request.extensions_mut().insert(registry);
        let fixture_request = support::ctx_with_config(config, request)
            .into_request()
            .expect("fixture request");
        let outcome = block_on(app.dispatch_ingress(
            fixture_request,
            request_start,
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .expect("App dispatches");
        let IngressDispatchOutcome::Response(envelope) = outcome else {
            panic!("request must receive an application response");
        };
        *envelope
    }

    fn dispatch_configured(
        app: &App,
        method: Method,
        uri: &str,
        config: &str,
        cookie: Option<&str>,
        body: Body,
    ) -> Response {
        let mut builder = request_builder()
            .method(method)
            .uri(uri)
            .header("origin", ORIGIN)
            .header("content-type", "application/json");
        if let Some(cookie_header) = cookie {
            builder = builder.header("cookie", cookie_header);
        }
        let mut envelope = dispatch_configured_envelope_at(
            app,
            builder.body(body).expect("request builds"),
            MonotonicInstant::now(),
            config,
        );
        // These wire fixtures include URL/body/cookie construction refusals.
        // Handler return must preserve the original admission receipt on all
        // of them; extracting bytes here is not the production egress path.
        let gates = boot_gates().unwrap();
        let peak = gates.aux_work_derived.peak_bytes();
        assert!(peak > 0);
        assert_eq!(gates.aux.in_use(), 1);
        assert_eq!(gates.aux_work_budget.in_use(), peak);
        assert_eq!(gates.static_egress.in_use(), 0);
        let response = mem::replace(envelope.response_mut(), Response::new(Body::empty()));
        assert_eq!(gates.aux_work_budget.in_use(), peak);
        drop(envelope);
        assert_eq!(gates.aux.in_use(), 0);
        assert_eq!(gates.aux_work_budget.in_use(), 0);
        response
    }

    fn assert_browser_wire(
        response: &Response,
        status: StatusCode,
        expected: &[u8],
        content_type: Option<&str>,
    ) {
        assert_eq!(
            response.status(),
            status,
            "{}",
            String::from_utf8_lossy(response.body().as_bytes().unwrap()),
        );
        assert_eq!(response.body().as_bytes().unwrap(), expected);
        let headers = response.headers();
        assert_eq!(
            headers
                .get("content-type")
                .map(|value| value.to_str().unwrap()),
            content_type,
        );
        assert_eq!(headers["cache-control"], "no-store");
        assert_eq!(headers["access-control-allow-origin"], ORIGIN);
        assert_eq!(headers["access-control-allow-credentials"], "true");
        assert!(headers.get_all("vary").iter().any(|value| {
            value
                .to_str()
                .unwrap()
                .split(',')
                .any(|token| token.trim().eq_ignore_ascii_case("origin"))
        }));
        assert!(!headers.contains_key("set-cookie"));
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "keep the full maximum-diagnostics wire assertions and no-debug control together"
    )]
    fn cookie_sync_serializes_all_256_debug_rows_without_truncation() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        assert_eq!(MAX_SYNC_REQUESTED_NAMES, 256);
        assert_eq!(MAX_SYNC_DEBUG_ROWS, 256);
        let compiled: Vec<String> = auctioneer_config_data::bidders()
            .iter()
            .map(|bidder| bidder.name.clone())
            .collect();
        let DebugFixture { config, bidders } = debug_fixture(&compiled).unwrap();
        let unconfigured: BTreeSet<&str> = auctioneer_config_data::bidders()
            .iter()
            .filter(|bidder| {
                bidder.user_sync.iframe.is_none() && bidder.user_sync.redirect.is_none()
            })
            .map(|bidder| bidder.name.as_str())
            .collect();
        assert_eq!(
            bidders.len(),
            usize::try_from(MAX_SYNC_REQUESTED_NAMES).unwrap()
        );
        validate_effective_defaults(&toml::from_str::<AuctioneerAppConfig>(&config).unwrap())
            .unwrap();
        let mut payload = diagnostic_payload(&bidders);
        let bytes = serde_json::to_vec(&payload).unwrap();
        assert!(bytes.len() <= 32 * 1_024, "must reach the chooser");
        let response = dispatch_configured(
            &app,
            Method::POST,
            "/cookie_sync",
            &config,
            None,
            Body::from_bytes(bytes),
        );
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(response.body().as_bytes().unwrap()),
        );
        let wire = response.body().as_bytes().unwrap();
        let json: Value = serde_json::from_slice(wire).unwrap();
        assert_eq!(json["status"], "no_cookie");
        assert_eq!(json["bidder_status"], serde_json::json!([]));
        let rows = json["debug"].as_array().unwrap();
        assert_eq!(rows.len(), usize::try_from(MAX_SYNC_DEBUG_ROWS).unwrap());
        let expected_names: BTreeSet<&str> = bidders.iter().map(String::as_str).collect();
        let actual_names: BTreeSet<&str> = rows
            .iter()
            .map(|row| row["bidder"].as_str().unwrap())
            .collect();
        assert_eq!(actual_names, expected_names);
        let mut seen_families = BTreeSet::new();
        for row in rows {
            let name = row["bidder"].as_str().unwrap();
            let error = if name.starts_with("fake_un_") {
                "unknown bidder"
            } else if unconfigured.contains(name) {
                "unconfigured bidder"
            } else {
                let base = if name.starts_with("fake_diagnostic_") {
                    live_catalog::redirect().name()
                } else {
                    name
                };
                let bidder = auctioneer_config_data::bidders()
                    .iter()
                    .find(|bidder| bidder.name == base)
                    .unwrap();
                let family = bidder.user_sync.key.as_deref().unwrap_or(&bidder.name);
                if seen_families.insert(family) {
                    "filtered"
                } else {
                    "duplicate sync family"
                }
            };
            assert_eq!(row, &serde_json::json!({"bidder": name, "error": error}));
        }
        let expected = format!(
            "{{\"status\":\"no_cookie\",\"bidder_status\":[],\"debug\":{}}}\n",
            serde_json::to_string(&json["debug"]).unwrap(),
        );
        assert!(
            wire.len() > 32 * 1_024,
            "exercise a large serialized diagnostic list"
        );
        assert!(wire.len() <= 256 * 1_024);
        assert_browser_wire(
            &response,
            StatusCode::OK,
            expected.as_bytes(),
            Some("application/json; charset=utf-8"),
        );

        payload["debug"] = Value::Bool(false);
        let without_debug = dispatch_configured(
            &app,
            Method::POST,
            "/cookie_sync",
            &config,
            None,
            Body::from_bytes(serde_json::to_vec(&payload).unwrap()),
        );
        assert_browser_wire(
            &without_debug,
            StatusCode::OK,
            b"{\"status\":\"no_cookie\",\"bidder_status\":[]}\n",
            Some("application/json; charset=utf-8"),
        );
    }

    #[test]
    fn cookie_sync_rejects_257_requested_rows_before_dedup_without_partial_list() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let compiled = auctioneer_config_data::bidders()
            .iter()
            .map(|bidder| bidder.name.clone())
            .collect::<Vec<_>>();
        let DebugFixture {
            config,
            mut bidders,
        } = debug_fixture(&compiled).unwrap();
        bidders.push(bidders[0].clone());
        assert_eq!(bidders.len(), 257);
        let payload = serde_json::json!({
            "bidders": bidders,
            "gdpr": 0_i32,
            "coopSync": false,
            "debug": true,
        });
        let bytes = serde_json::to_vec(&payload).unwrap();
        assert!(
            bytes.len() <= 32 * 1_024,
            "must reach the requested-name owner"
        );
        let response = dispatch_configured(
            &app,
            Method::POST,
            "/cookie_sync",
            &config,
            None,
            Body::from_bytes(bytes),
        );
        assert_browser_wire(
            &response,
            StatusCode::BAD_REQUEST,
            b"invalid bidder list\n",
            Some("text/plain; charset=utf-8"),
        );
    }

    #[test]
    fn cookie_sync_url_hard_ceiling_discards_even_an_expandable_candidate() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let config = format!(
            "{BROWSER_CONFIG}redirect_url = '{{{{.ExternalURL}}}}/setuid?bidder={{{{.SyncerKey}}}}&gdpr={{{{.GDPR}}}}&uid={{{{.UserMacro}}}}'\n"
        );
        for both_bidders in [false, true] {
            let bidders = if both_bidders {
                vec![
                    live_catalog::redirect().name(),
                    live_catalog::iframe().name(),
                ]
            } else {
                vec![live_catalog::redirect().name()]
            };
            // Only the iframe role expands consent in its outer endpoint.
            let payload = serde_json::json!({
                "bidders": bidders,
                "gdpr": 0_i32,
                "gdpr_consent": "<".repeat(11_000),
                "coopSync": false,
                "debug": true,
            });
            let bytes = serde_json::to_vec(&payload).unwrap();
            assert!(bytes.len() < 32 * 1_024);
            let response = dispatch_configured(
                &app,
                Method::POST,
                "/cookie_sync",
                &config,
                None,
                Body::from_bytes(bytes),
            );
            if both_bidders {
                assert_browser_wire(
                    &response,
                    StatusCode::BAD_REQUEST,
                    b"sync URL exceeds resource limit\n",
                    Some("text/plain; charset=utf-8"),
                );
            } else {
                assert_eq!(response.status(), StatusCode::OK);
                let json: Value =
                    serde_json::from_slice(response.body().as_bytes().unwrap()).unwrap();
                assert_eq!(json["bidder_status"].as_array().unwrap().len(), 1);
                assert_eq!(
                    json["bidder_status"][0]["bidder"],
                    live_catalog::redirect().name()
                );
                let expected = format!(
                    "{{\"status\":\"no_cookie\",\"bidder_status\":[{{\"bidder\":{},\"no_cookie\":true,\"usersync\":{{\"url\":{},\"type\":\"redirect\"}}}}]}}\n",
                    serde_json::to_string(live_catalog::redirect().name()).unwrap(),
                    serde_json::to_string(&json["bidder_status"][0]["usersync"]["url"]).unwrap(),
                );
                assert_browser_wire(
                    &response,
                    StatusCode::OK,
                    expected.as_bytes(),
                    Some("application/json; charset=utf-8"),
                );
            }
        }
    }

    #[test]
    fn setuid_cookie_serialization_hard_ceiling_refuses_without_partial_cookie() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let config = format!("{BROWSER_CONFIG}[host.host_cookie]\nmax_cookie_size_bytes = 0\n");
        let first = dispatch_configured(
            &app,
            Method::GET,
            &live_catalog::iframe().setuid("uid=preserved-user&gdpr=0&f=b"),
            &config,
            None,
            Body::empty(),
        );
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.body().as_bytes().unwrap(), b"");
        assert_eq!(first.headers()["content-type"], "text/html");
        let stored =
            cookie::Cookie::parse(first.headers()["set-cookie"].to_str().unwrap()).unwrap();
        assert!(stored.value().starts_with("v1.test."));
        let carried = format!("{}={}", stored.name(), stored.value());

        // Discover the true signed-field boundary rather than retaining the
        // former 16-KiB fixture or using an arbitrary oversized identifier.
        // A fixed clock removes expiry precision as an uncontrolled input; it
        // does not establish the cause of the separately reported route flake.
        let (mut fits, mut exceeds) = (1_usize, 4096_usize);
        while exceeds - fits > 1 {
            let middle = fits.midpoint(exceeds);
            let probe = dispatch_configured(
                &app,
                Method::GET,
                &format!(
                    "/setuid?bidder={}&uid={}&gdpr=0&f=b",
                    live_catalog::redirect().name(),
                    "x".repeat(middle)
                ),
                &config,
                None,
                Body::empty(),
            );
            match probe.status() {
                StatusCode::OK => fits = middle,
                StatusCode::BAD_REQUEST => {
                    assert_eq!(
                        probe.body().as_bytes().unwrap(),
                        b"cookie resource limit exceeded"
                    );
                    assert!(!probe.headers().contains_key("set-cookie"));
                    exceeds = middle;
                }
                status => panic!("unexpected signed-field boundary status {status}"),
            }
        }
        let boundary = dispatch_configured(
            &app,
            Method::GET,
            &format!(
                "/setuid?bidder={}&uid={}&gdpr=0&f=b",
                live_catalog::redirect().name(),
                "x".repeat(fits)
            ),
            &config,
            None,
            Body::empty(),
        );
        assert_eq!(boundary.status(), StatusCode::OK);
        assert!((4093..=4096).contains(&boundary.headers()["set-cookie"].as_bytes().len()));

        let uri = format!(
            "/setuid?bidder={}&uid={}&gdpr=0&f=b",
            live_catalog::redirect().name(),
            "x".repeat(exceeds),
        );
        assert!(uri.len() < 32 * 1_024);
        let refused = dispatch_configured(
            &app,
            Method::GET,
            &uri,
            &config,
            Some(&carried),
            Body::empty(),
        );
        assert_browser_wire(
            &refused,
            StatusCode::BAD_REQUEST,
            b"cookie resource limit exceeded",
            None,
        );

        let readback = dispatch_configured(
            &app,
            Method::GET,
            "/getuids",
            &config,
            Some(&carried),
            Body::empty(),
        );
        assert_browser_wire(
            &readback,
            StatusCode::OK,
            format!(
                "{{\"buyeruids\":{{{}:\"preserved-user\"}}}}\n",
                serde_json::to_string(live_catalog::iframe().family()).unwrap()
            )
            .as_bytes(),
            Some("text/plain; charset=utf-8"),
        );
    }

    #[test]
    fn narrowed_app_config_byte_limits_refuse_before_cookie_sync_body_poll() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let mut app = app();
        let installed_limits = app.config_extraction_limits();
        let mut large_config = format!("{BROWSER_CONFIG}[host.aliases]\n");
        for index in 0_usize..64 {
            let name = format!("config-{index:03}-{}", "x".repeat(90));
            writeln!(
                large_config,
                "{name} = '{}'",
                live_catalog::redirect().name()
            )
            .unwrap();
        }
        for (max_blob_bytes, max_backend_bytes) in [(4_096, 64 * 1_024), (64 * 1_024, 4_096)] {
            app.set_config_extraction_limits(ConfigExtractionLimits {
                max_blob_bytes,
                max_backend_bytes,
                ..installed_limits
            })
            .expect("positive local extraction limits");
            for (config, refused) in [(BROWSER_CONFIG, false), (large_config.as_str(), true)] {
                let polls = Rc::new(Cell::new(0_u32));
                let observed = Rc::clone(&polls);
                let body = Body::from_stream(stream::poll_fn(move |_| {
                    observed.set(observed.get().saturating_add(1));
                    assert_eq!(observed.get(), 0, "config/encoding refusal polled the body");
                    Poll::Ready(None)
                }));
                let request = request_builder()
                    .method(Method::POST)
                    .uri("/cookie_sync")
                    .header("origin", ORIGIN)
                    .header("content-type", "application/json")
                    .header("content-encoding", "gzip")
                    .body(body)
                    .expect("request builds");
                let mut envelope =
                    dispatch_configured_envelope_at(&app, request, MonotonicInstant::now(), config);
                let response = envelope.response_mut();
                if refused {
                    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
                    assert_eq!(
                        response.body().as_bytes().unwrap(),
                        b"Malformed host configuration"
                    );
                    assert_eq!(
                        response.headers()["content-type"],
                        "text/plain; charset=utf-8"
                    );
                    assert_eq!(response.headers()["cache-control"], "no-store");
                    for name in [
                        "set-cookie",
                        "access-control-allow-origin",
                        "access-control-allow-credentials",
                        "access-control-allow-methods",
                        "access-control-allow-headers",
                    ] {
                        assert!(!response.headers().contains_key(name), "{name}");
                    }
                } else {
                    assert_browser_wire(
                        response,
                        StatusCode::BAD_REQUEST,
                        b"Invalid request: Content-Encoding of type gzip is not supported\n",
                        Some("text/plain; charset=utf-8"),
                    );
                }
                assert_eq!(polls.get(), 0_u32);
            }
        }
    }

    #[test]
    fn registered_sync_methods_dispatch_through_the_configured_app() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let probe = dispatch(&app, Method::GET, "/status", Body::empty());
        assert_eq!(probe.status(), StatusCode::NO_CONTENT);
        assert_eq!(probe.headers()[CONFIG_STATUS_HEADER], "ok");
        for (method, path, body, expected) in [
            (
                Method::POST,
                "/cookie_sync",
                Body::from_bytes(
                    serde_json::to_vec(&serde_json::json!({"bidders": [], "gdpr": 0_i32}))
                        .expect("cookie-sync body"),
                ),
                StatusCode::OK,
            ),
            (
                Method::OPTIONS,
                "/cookie_sync",
                Body::empty(),
                StatusCode::FORBIDDEN,
            ),
            (
                Method::GET,
                &live_catalog::redirect().setuid("uid=buyer-1&gdpr=0&f=i"),
                Body::empty(),
                StatusCode::OK,
            ),
            (Method::GET, "/getuids", Body::empty(), StatusCode::OK),
        ] {
            let response = dispatch(&app, method.clone(), path, body);
            assert_eq!(response.status(), expected, "{method} {path}");
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        }
    }

    #[test]
    fn app_accepts_cookie_sync_above_the_default_cap_through_32_kib() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let gates = boot_gates().expect("booted gates");
        for size in [DEFAULT_CAP_BYTES + 1, COOKIESYNC_CAP_BYTES] {
            let mut bytes = serde_json::to_vec(&serde_json::json!({
                "bidders": [],
                "coopSync": false,
                "gdpr": 0_i32,
                "filterSettings": {
                    "image": {"filter": "exclude", "bidders": "*"},
                    "iframe": {"filter": "exclude", "bidders": "*"},
                },
            }))
            .expect("cookie-sync body");
            bytes.resize(size, b' ');
            let chunks = bytes
                .chunks(DEFAULT_CAP_BYTES)
                .map(|chunk| Ok(Body::from_bytes(chunk.to_vec()).into_bytes().unwrap()))
                .collect::<Vec<_>>();
            for (body, declared) in [
                (Body::from_bytes(bytes), true),
                (Body::from_stream(stream::iter(chunks)), false),
            ] {
                let mut builder = request_builder()
                    .method(Method::POST)
                    .uri("/cookie_sync")
                    .header("origin", ORIGIN)
                    .header("content-type", "application/json");
                if declared {
                    builder = builder.header("content-length", size.to_string());
                }
                let mut envelope = dispatch_configured_envelope_at(
                    &app,
                    builder.body(body).expect("request builds"),
                    MonotonicInstant::now(),
                    BROWSER_CONFIG,
                );
                assert_browser_wire(
                    envelope.response_mut(),
                    StatusCode::OK,
                    b"{\"status\":\"no_cookie\",\"bidder_status\":[]}\n",
                    Some("application/json; charset=utf-8"),
                );
                assert_eq!(gates.aux.in_use(), 1);
                assert_eq!(
                    gates.aux_work_budget.in_use(),
                    gates.aux_work_derived.peak_bytes(),
                );
                drop(envelope);
                assert_eq!(gates.aux.in_use(), 0);
                assert_eq!(gates.aux_work_budget.in_use(), 0);
            }
        }
    }

    #[test]
    fn app_preflight_retains_the_default_body_cap_after_http_checks() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        for size in [DEFAULT_CAP_BYTES, DEFAULT_CAP_BYTES + 1] {
            let chunks = [DEFAULT_CAP_BYTES, size - DEFAULT_CAP_BYTES]
                .map(|len| Ok(Body::from_bytes(vec![b'x'; len]).into_bytes().unwrap()));
            for body in [
                Body::from_bytes(vec![b'x'; size]),
                Body::from_stream(stream::iter(chunks)),
            ] {
                let request = request_builder()
                    .method(Method::OPTIONS)
                    .uri("/cookie_sync")
                    .header("origin", ORIGIN)
                    .header("access-control-request-method", "POST")
                    .header("access-control-request-headers", "Content-Type")
                    .body(body)
                    .expect("request builds");
                let mut envelope = dispatch_configured_envelope_at(
                    &app,
                    request,
                    MonotonicInstant::now(),
                    BROWSER_CONFIG,
                );
                let response = envelope.response_mut();
                if size == DEFAULT_CAP_BYTES {
                    assert_browser_wire(response, StatusCode::NO_CONTENT, b"", None);
                    assert_eq!(response.headers()["access-control-allow-methods"], "POST");
                } else {
                    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
                    assert_eq!(response.body().as_bytes().unwrap(), b"payload_too_large");
                    assert!(
                        !response
                            .headers()
                            .contains_key("access-control-allow-origin")
                    );
                }
            }
        }
    }

    #[test]
    fn app_preflight_http_refusal_precedes_body_polling() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let request = request_builder()
            .method(Method::OPTIONS)
            .uri("/cookie_sync")
            .header("origin", "https://unapproved.example")
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "Content-Type")
            .body(Body::from_stream(stream::poll_fn(|_| {
                panic!("a denied preflight must not poll its body");
            })))
            .expect("request builds");
        let mut envelope =
            dispatch_configured_envelope_at(&app, request, MonotonicInstant::now(), BROWSER_CONFIG);
        let response = envelope.response_mut();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin")
        );
    }

    #[test]
    fn app_get_sync_routes_drain_under_the_default_body_cap_before_cookie_work() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        for uri in [
            "/getuids",
            &live_catalog::redirect().setuid("uid=test-user&gdpr=0&f=b"),
        ] {
            for size in [DEFAULT_CAP_BYTES, DEFAULT_CAP_BYTES + 1] {
                for body in [
                    Body::from_bytes(vec![b'x'; size]),
                    Body::from_stream(stream::iter([Ok(Body::from_bytes(vec![b'x'; size])
                        .into_bytes()
                        .unwrap())])),
                ] {
                    let request = request_builder()
                        .method(Method::GET)
                        .uri(uri)
                        .body(body)
                        .expect("request builds");
                    let mut envelope = dispatch_configured_envelope_at(
                        &app,
                        request,
                        MonotonicInstant::now(),
                        BROWSER_CONFIG,
                    );
                    let response = envelope.response_mut();
                    if size == DEFAULT_CAP_BYTES {
                        assert_eq!(response.status(), StatusCode::OK, "{uri}");
                    } else {
                        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE, "{uri}");
                        assert_eq!(response.body().as_bytes().unwrap(), b"payload_too_large");
                        assert!(!response.headers().contains_key("set-cookie"));
                    }
                }
            }
        }
    }

    #[test]
    fn app_rejects_encoded_or_oversized_cookie_sync_before_body_poll() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        for (encoding, expected) in [
            (
                Some("gzip"),
                b"Invalid request: Content-Encoding of type gzip is not supported\n".as_slice(),
            ),
            (
                None,
                b"cookie_sync request size exceeded max size of 32768 bytes.\n".as_slice(),
            ),
        ] {
            let polls = Rc::new(Cell::new(0_u32));
            let observed = Rc::clone(&polls);
            let body = Body::from_stream(stream::poll_fn(move |_| {
                observed.set(observed.get().saturating_add(1));
                Poll::Ready(None)
            }));
            let mut builder = request_builder()
                .method(Method::POST)
                .uri("/cookie_sync")
                .header("content-type", "application/json")
                .header("content-length", "32769");
            if let Some(value) = encoding {
                builder = builder.header("content-encoding", value);
            }
            let response = dispatch_request(&app, builder.body(body).expect("request builds"));
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(response.body().as_bytes().unwrap(), expected);
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(polls.get(), 0_u32, "pre-read refusal polled the body");
        }
    }

    #[test]
    fn app_streaming_cap_rejects_cookie_sync_without_declared_length() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let response = dispatch(
            &app,
            Method::POST,
            "/cookie_sync",
            Body::from_bytes(vec![b'x'; 32 * 1_024 + 1]),
        );
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.body().as_bytes().unwrap(),
            b"cookie_sync request size exceeded max size of 32768 bytes.\n"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
    }

    #[test]
    fn getuids_rejects_oversized_or_overpopulated_query() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let oversized = format!("/getuids?{}", "x".repeat(32 * 1_024 + 1));
        let response = dispatch(&app, Method::GET, &oversized, Body::empty());
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.body().as_bytes().unwrap(),
            b"query exceeds 32768 bytes"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");

        let overpopulated = format!("/getuids?{}", vec!["x=1"; 33].join("&"));
        let overpopulated_response = dispatch(&app, Method::GET, &overpopulated, Body::empty());
        assert_eq!(overpopulated_response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            overpopulated_response.body().as_bytes().unwrap(),
            b"too many query parameters"
        );
    }

    #[test]
    fn sync_aux_capacity_and_work_reservation_survive_until_response_completion() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let gates = boot_gates().expect("booted gates");
        let peak = gates.aux_work_derived.peak_bytes();
        let mut held = Vec::new();
        for _ in 0..gates.limits.aux_route_permits {
            let request = request_builder()
                .method(Method::GET)
                .uri("/getuids")
                .body(Body::empty())
                .expect("request builds");
            let mut envelope = dispatch_envelope(&app, request);
            assert_eq!(envelope.response_mut().status(), StatusCode::OK);
            held.push(envelope);
        }
        assert_eq!(
            gates.aux_work_budget.in_use(),
            peak * u64::from(gates.limits.aux_route_permits)
        );
        let refused = dispatch(&app, Method::GET, "/getuids", Body::empty());
        assert_eq!(refused.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(refused.headers()["cache-control"], "no-store");
        assert!(
            !refused
                .headers()
                .contains_key("access-control-allow-origin")
        );
        let probe = dispatch(&app, Method::GET, "/status", Body::empty());
        assert_eq!(probe.status(), StatusCode::NO_CONTENT);
        drop(held.pop());
        assert_eq!(
            gates.aux_work_budget.in_use(),
            peak * u64::from(gates.limits.aux_route_permits - 1)
        );
        assert_eq!(
            dispatch(&app, Method::GET, "/getuids", Body::empty()).status(),
            StatusCode::OK
        );
        drop(held);
        assert_eq!(gates.aux_work_budget.in_use(), 0);
    }

    #[test]
    fn expired_cookie_sync_cpu_budget_refuses_before_body_poll() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let request_start = MonotonicInstant::now()
            .checked_sub(Duration::from_secs(31))
            .expect("monotonic lookback");
        let polls = Rc::new(Cell::new(0_u32));
        let observed = Rc::clone(&polls);
        let body = Body::from_stream(stream::poll_fn(move |_| {
            observed.set(observed.get().saturating_add(1));
            Poll::Ready(None)
        }));
        let request = request_builder()
            .method(Method::POST)
            .uri("/cookie_sync")
            .header("content-type", "application/json")
            .body(body)
            .expect("request builds");
        let mut envelope = dispatch_envelope_at(&app, request, request_start);
        let response = envelope.response_mut();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.body().as_bytes().unwrap(), b"overloaded\n");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("set-cookie"));
        assert_eq!(polls.get(), 0_u32, "expired request polled its body");
    }

    #[test]
    fn expired_cookie_sync_post_retains_snapshot_cors() {
        assert_expired_cookie_sync_retains_snapshot_cors(&Method::POST);
    }

    #[test]
    fn expired_cookie_sync_preflight_retains_snapshot_cors() {
        assert_expired_cookie_sync_retains_snapshot_cors(&Method::OPTIONS);
    }

    fn assert_expired_cookie_sync_retains_snapshot_cors(method: &Method) {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        for (origin, allowed) in [
            ("https://publisher.example", true),
            ("https://unapproved.example", false),
        ] {
            let polls = Rc::new(Cell::new(0_u32));
            let observed = Rc::clone(&polls);
            let body = Body::from_stream(stream::poll_fn(move |_| {
                observed.set(observed.get().saturating_add(1));
                Poll::Ready(None)
            }));
            let request = request_builder()
                .method(method.clone())
                .uri("/cookie_sync")
                .header("content-type", "application/json")
                .header("origin", origin)
                .header("access-control-request-method", "POST")
                .header("access-control-request-headers", "Content-Type")
                .body(body)
                .expect("request builds");
            let fixture_request = support::ctx_with_config(
                "schema_version = 1\n[host.user_sync]\nexternal_url = \"https://sync.example.test\"\nallowed_origins = ['https://publisher.example']\n",
                request,
            )
            .into_request()
            .expect("fixture request");
            let request_start = MonotonicInstant::now()
                .checked_sub(Duration::from_secs(31))
                .expect("monotonic lookback");
            let outcome = block_on(app.dispatch_ingress(
                fixture_request,
                request_start,
                IngressHeadAccounting::HostManaged,
                IngressFraming::HostManaged,
            ))
            .expect("App dispatches");
            let IngressDispatchOutcome::Response(mut envelope) = outcome else {
                panic!("request must receive an application response");
            };
            let response = envelope.response_mut();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(response.body().as_bytes().unwrap(), b"overloaded\n");
            let headers = response.headers();
            assert_eq!(headers["cache-control"], "no-store");
            if allowed {
                assert_eq!(headers.get("access-control-allow-origin").unwrap(), origin);
                assert_eq!(headers["access-control-allow-credentials"], "true");
                assert!(headers.get_all("vary").iter().any(|value| {
                    value
                        .to_str()
                        .unwrap()
                        .split(',')
                        .any(|token| token.trim().eq_ignore_ascii_case("Origin"))
                }));
            } else {
                assert!(!headers.contains_key("access-control-allow-origin"));
                assert!(!headers.contains_key("access-control-allow-credentials"));
            }
            assert!(!headers.contains_key("access-control-allow-methods"));
            assert!(!headers.contains_key("access-control-allow-headers"));
            assert!(!headers.contains_key("set-cookie"));
            assert_eq!(
                polls.get(),
                0_u32,
                "expired {method} request polled its body"
            );
        }
    }

    #[test]
    fn expired_sync_cpu_budget_refuses_before_get_or_cookie_write() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let request_start = MonotonicInstant::now()
            .checked_sub(Duration::from_secs(31))
            .expect("monotonic lookback");
        for uri in [
            "/getuids",
            &live_catalog::redirect().setuid("uid=must-not-write&gdpr=0&f=b"),
        ] {
            let request = request_builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .expect("request builds");
            let mut envelope = dispatch_envelope_at(&app, request, request_start);
            let response = envelope.response_mut();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
            assert_eq!(response.body().as_bytes().unwrap(), b"overloaded\n");
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert!(!response.headers().contains_key("set-cookie"));
        }
    }

    #[test]
    fn expired_sync_cpu_budget_retains_snapshot_cors() {
        let _serial = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let app = app();
        let request = request_builder()
            .method(Method::GET)
            .uri("/getuids")
            .header("origin", "https://publisher.example")
            .body(Body::empty())
            .expect("request builds");
        let fixture_request = support::ctx_with_config(
            "schema_version = 1\n[host.user_sync]\nexternal_url = \"https://sync.example.test\"\nallowed_origins = ['https://publisher.example']\n",
            request,
        )
        .into_request()
        .expect("fixture request");
        let request_start = MonotonicInstant::now()
            .checked_sub(Duration::from_secs(31))
            .expect("monotonic lookback");
        let outcome = block_on(app.dispatch_ingress(
            fixture_request,
            request_start,
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .expect("App dispatches");
        let IngressDispatchOutcome::Response(mut envelope) = outcome else {
            panic!("request must receive an application response");
        };
        let response = envelope.response_mut();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://publisher.example"
        );
        assert_eq!(
            response.headers()["access-control-allow-credentials"],
            "true"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
}

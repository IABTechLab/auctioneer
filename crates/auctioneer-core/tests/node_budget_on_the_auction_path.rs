//! The configured JSON node budget, enforced on the REAL request path.
//!
//! `max_parsed_nodes` is an envelope row and a memory-budget term: the
//! inbound-parse charge is sized as a node count times a per-node
//! retained-byte figure. An input that fits the 256 KiB body cap can
//! still expand into far more retained nodes than that term paid for —
//! a flat array of small numbers is a few bytes per node on the wire
//! and hundreds of bytes per node in the heap — so the body cap alone
//! does not bound what a parse retains. Only a parse that counts nodes
//! does.
//!
//! These assertions drive `App::dispatch_ingress`, because that is the
//! only shape that can tell a bounded parser from an unbounded one:
//! `bounded_json`'s own tests prove the parser counts correctly, and
//! would stay green however the handler parsed.
//!
//! Three cases, at the exact boundary the envelope declares:
//!
//! 1. A request AT the node budget is served.
//! 2. A request ONE NODE over it is refused, with the pinned wire body.
//! 3. A request over it that ALSO names a disabled account is refused
//!    for the node budget, not for the account — the budget is the
//!    first thing the bytes are measured against, so it decides before
//!    account resolution can conclude.

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod tests {
    use crate::serving_catalog::{ServingBidder, default_active_publications};
    use async_trait::async_trait;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_config::types::MediaType;
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::configure::configure_app;
    use auctioneer_core::handlers::TestFanoutHandle;
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs as core_construction_costs;
    use auctioneer_core::host_primitives::{
        BootEnvelope, DelegateToDevStrategy, DevRng, StderrBootLog, SystemClock,
        bootstrap_adapter_with_envelope,
    };
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    };
    use auctioneer_core::test_support::config_store::runtime_registry;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::bounded_json;
    use auctioneer_runtime::budget::ASSEMBLY_FACTOR;
    use auctioneer_runtime::{
        Clock, FakeClock, ImmediateResolveTimer, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, NoopDecodeCooperator,
        ResolvedIsolateLimits, Rng, SeededRng, TargetPlatformConstants,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::http::Request;
    use edgezero_core::http::{Method, StatusCode, request_builder};
    use edgezero_core::ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting};
    use edgezero_core::response_egress::ResponseEgressEnvelope;
    use edgezero_core::store_registry::ConfigRegistry;
    use edgezero_core::time::{MonotonicClock, MonotonicInstant};
    use futures::executor::block_on;
    use std::sync::{Arc, OnceLock};
    use std::time::Duration;
    use time::OffsetDateTime;

    const CAP_256_KIB: u64 = 1 << 18;
    const BYTES_1_MIB: u64 = 1 << 20;
    /// The response cap, DERIVED from the assembly scratch the budget
    /// requires it to hold (`max_outbound_requests x max_decoded_bytes x
    /// ASSEMBLY_FACTOR`). A literal here went stale the moment the
    /// factor was corrected.
    const FINAL_RESPONSE_BYTES: u64 = 64 * CAP_256_KIB * ASSEMBLY_FACTOR;
    const BYTES_16_KIB: u32 = 1 << 14;
    const OUTBOUND_META_BYTES: u32 = auctioneer_bidder::outbound_meta_bytes(100);
    // Boot gates are process-global. Each of the five parallel tests
    // dispatches at most one request at a time, including its response.
    const PARALLEL_AUCTION_FIXTURES: u32 = 5;

    /// The node budget these tests measure against.
    ///
    /// Deliberately SMALL rather than the shipped `65_536`, so the
    /// boundary fixtures stay well inside the 256 KiB body cap. That is
    /// the point of the whole finding: a body the cap admits can exceed
    /// the node budget, and a fixture that had to breach the body cap to
    /// breach the node budget would be testing the cap instead.
    const NODE_BUDGET: u32 = 2_048;

    /// The account id the disabled-account fixture names, and the one
    /// the overlay disables.
    const DISABLED_ACCOUNT: &str = "acct-disabled";

    /// The pinned wire body for a node-budget refusal. Byte-exact,
    /// including the trailing newline: a length-only or prefix-only
    /// assertion passes while the wire format is wrong.
    const NODE_BUDGET_BODY: &str = "Invalid request: too many JSON nodes\n";

    fn boot() {
        static BOOTED: OnceLock<()> = OnceLock::new();
        BOOTED.get_or_init(|| {
            bootstrap_adapter_with_envelope(
                BootEnvelope {
                    limits: &limits(),
                    target: &target(),
                    outbound_meta_bytes: OUTBOUND_META_BYTES,
                    construction_costs: core_construction_costs(),
                    log_sink: &StderrBootLog,
                    catalog_costs: auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
                },
                || Arc::new(SystemClock),
                || Arc::new(DevRng::new()),
                || Arc::new(ImmediateResolveTimer),
                || Arc::new(NoopDecodeCooperator),
                || Arc::new(DelegateToDevStrategy),
            )
            .expect("the fixture envelope validates");
        });
    }

    fn limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 1_600_000_000,
            baseline_bytes: 96_000_000,
            inbound_request_bytes: CAP_256_KIB,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: CAP_256_KIB,
            max_outbound_raw_bytes: CAP_256_KIB,
            max_transport_bytes: CAP_256_KIB,
            max_decoded_bytes: CAP_256_KIB,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: FINAL_RESPONSE_BYTES,
            egress_permits: 3,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 5,
            // Funds config extraction and the checked sync/privacy workload.
            aux_route_peak_bytes: 768 * 1_024 * 1_024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 50,
            max_outbound_requests: 64,
            max_concurrent_auctions: PARALLEL_AUCTION_FIXTURES,
            max_active_body_streams: 7,
            max_runtime_config_bytes: BYTES_1_MIB,
            max_runtime_config_blob_bytes: BYTES_1_MIB,
            max_parsed_nodes: NODE_BUDGET,
            max_copied_header_bytes: 4_096_u32,
            max_admitted_header_bytes: BYTES_16_KIB,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 6,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096_u32,
        }
    }

    fn target() -> TargetPlatformConstants {
        TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576_u32,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1_024,
            parser_transient_bytes: BYTES_16_KIB,
            egress_handoff_factor: 2,
        }
    }

    /// The runtime config these tests serve.
    ///
    /// `disabled` names one account as disabled, which is what makes
    /// the precedence fixture a real precedence fixture: without it the
    /// third test could not distinguish "the node budget won" from
    /// "there was no competing error".
    fn config_registry() -> ConfigRegistry {
        let overlay = serde_json::json!({
            "accounts": {
                DISABLED_ACCOUNT: { "disabled": true }
            }
        });
        let cfg: AuctioneerAppConfig =
            serde_json::from_value(overlay).expect("the overlay matches the config shape");
        runtime_registry(&cfg)
    }

    fn configured_app() -> App {
        let mut app =
            App::build::<auctioneer_core::App>(edgezero_core::PlatformMetadata::default())
                .expect("the app builds");
        configure_app(&mut app).expect("configure is idempotent");
        app
    }

    struct OkFanout;

    #[async_trait(?Send)]
    impl BidderFanout for OkFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            Ok(calls
                .iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], Vec::new())),
                    elapsed: Duration::from_millis(1),
                })
                .collect())
        }
    }

    fn auction_bidder() -> &'static ServingBidder {
        static FIXTURE: OnceLock<ServingBidder> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            default_active_publications()
                .into_iter()
                .filter(|row| {
                    let info = row.info;
                    !info.disabled
                        && info.uses_https
                        && !info.endpoint.contains("{{")
                        && info.capabilities.site.contains(&MediaType::Banner)
                })
                .min_by_key(ServingBidder::name)
                .expect("node-boundary success requires an enabled fixed-endpoint banner bidder")
        })
    }

    /// Builds a valid auction request whose `imp[0].ext.prebid` carries
    /// a padding array, so the body's NODE COUNT is tunable while the
    /// request stays structurally valid and well inside the body cap.
    ///
    /// A flat numeric array is the right padding: it is the cheapest
    /// shape per wire byte that still costs one retained node each, and
    /// it is passthrough content the typed target discards — which is
    /// exactly the content the review's 70,000-element reproduction
    /// used and exactly what an unbounded parse retains for free.
    fn request_body(account: &str, padding: usize) -> String {
        let bidder = auction_bidder();
        let pad: Vec<u64> = (0..padding)
            .map(|index| u64::try_from(index).expect("a fixture index fits a u64"))
            .collect();
        let body = serde_json::json!({
            "id": "auc-node-budget",
            "tmax": 1000_i32,
            "cur": ["USD"],
            "site": {
                "page": "https://publisher.example/story",
                "publisher": { "id": account }
            },
            "imp": [
                {
                    "id": "imp-1",
                    "banner": { "w": 300_i32, "h": 250_i32 },
                    "ext": {
                        "prebid": { "bidder": { bidder.name(): &bidder.params } },
                        "padding": pad
                    }
                }
            ]
        });
        serde_json::to_string(&body).expect("the fixture serialises")
    }

    /// The exact node count of `bytes`, found by probing upward: the
    /// smallest budget that parses IS the node count.
    ///
    /// Measured rather than computed, because the count is a property
    /// of the parser's accounting and not of the fixture's shape, and a
    /// hand-computed figure would silently drift if that accounting
    /// were ever corrected again.
    fn node_count(bytes: &[u8]) -> u32 {
        let mut low = 1_u32;
        let mut high = NODE_BUDGET.saturating_mul(4);
        assert!(
            bounded_json::value_within(bytes, high).is_ok(),
            "the probe's upper bound must actually parse the fixture",
        );
        while low < high {
            // Midpoint without overflow and without a bare `/`: the
            // span is halved by a shift, which is the same value for
            // non-negative operands and carries no division semantics
            // to reason about.
            let mid = low.saturating_add(high.saturating_sub(low) >> 1_u32);
            if bounded_json::value_within(bytes, mid).is_ok() {
                high = mid;
            } else {
                low = mid.saturating_add(1);
            }
        }
        low
    }

    /// A body whose node count is EXACTLY `target`.
    ///
    /// The padding-to-node relation is one node per element, so the
    /// search is a single correction rather than a loop; it is asserted
    /// afterwards either way, because an off-by-one here would silently
    /// move both boundary fixtures to the same side of the boundary.
    fn body_with_nodes(account: &str, target: u32) -> String {
        let probe = request_body(account, 0);
        let base = node_count(probe.as_bytes());
        assert!(
            base < target,
            "the unpadded fixture must sit below the target node count",
        );
        let padding =
            usize::try_from(target.saturating_sub(base)).expect("the padding fits a usize");
        let body = request_body(account, padding);
        let got = node_count(body.as_bytes());
        assert_eq!(
            got, target,
            "each padding element must cost exactly one node, so the boundary fixtures \
             land where their names claim",
        );
        body
    }

    fn auction_request(body: &str) -> Request {
        let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let fanout: Arc<dyn BidderFanout + Send + Sync> = Arc::new(OkFanout);
        let mut request = request_builder()
            .method(Method::POST)
            .uri("/openrtb2/auction")
            .header("content-type", "application/json")
            .body(Body::from_bytes(body.as_bytes().to_vec()))
            .expect("the fixture request builds");
        request.extensions_mut().insert(TestFanoutHandle(fanout));
        request
            .extensions_mut()
            .insert(TestClockHandle(handler_clock));
        let monotonic_source = Arc::<FakeClock>::clone(&clock);
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                monotonic_source.now_monotonic()
            })));
        request.extensions_mut().insert(TestRngHandle(rng));
        request.extensions_mut().insert(config_registry());
        request
    }

    fn dispatch(app: &App, request: Request) -> ResponseEgressEnvelope {
        let outcome = block_on(app.dispatch_ingress(
            request,
            MonotonicInstant::now(),
            IngressHeadAccounting::HostManaged,
            IngressFraming::HostManaged,
        ))
        .expect("admission begins");
        match outcome {
            IngressDispatchOutcome::Response(envelope) => *envelope,
            IngressDispatchOutcome::Aborted => panic!("the fixture is answered, not aborted"),
            _ => panic!("unrecognized ingress dispatch outcome"),
        }
    }

    fn answer(app: &App, body: &str) -> (StatusCode, String) {
        let mut envelope = dispatch(app, auction_request(body));
        let status = envelope.response_mut().status();
        let bytes = envelope
            .response_mut()
            .body_mut()
            .as_bytes()
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        drop(envelope);
        (status, String::from_utf8_lossy(&bytes).into_owned())
    }

    /// A request AT the node budget is served.
    ///
    /// The lower half of the boundary, and the half that proves the
    /// bound is a bound rather than a blanket refusal. Both this and
    /// the test below stay far inside the 256 KiB body cap, so neither
    /// can pass by tripping the cap instead.
    #[test]
    fn a_request_at_the_node_budget_is_served() {
        boot();
        let app = configured_app();
        let body = body_with_nodes("acct-node-budget", NODE_BUDGET);
        assert!(
            body.len() < 0x0004_0000,
            "the boundary fixture must fit the body cap, or it would be testing the cap",
        );

        let (status, _body) = answer(&app, &body);
        assert_eq!(
            status,
            StatusCode::OK,
            "a request whose node count equals the budget is inside it: the budget is a \
             ceiling, and refusing AT it would reject exactly the traffic the envelope \
             charged for",
        );
    }

    #[test]
    fn an_oversized_alias_is_refused_before_auction_construction() {
        boot();
        let app = configured_app();
        let alias = "a".repeat(64 * 1024);
        let body = serde_json::to_string(&serde_json::json!({
            "id": "alias-bound",
            "site": {"publisher": {"id": "acct-node-budget"}},
            "imp": [{
                "id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"prebid": {"bidder": {&alias: {}}}}
            }],
            "ext": {"prebid": {"aliases": {&alias: "fixture-base"}}}
        }))
        .expect("request serializes");
        assert!(body.len() < usize::try_from(CAP_256_KIB).unwrap());
        assert!(node_count(body.as_bytes()) < NODE_BUDGET);
        let (status, response) = answer(&app, &body);
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(response.contains("alias name exceeds the 64-byte bidder identity limit"));
        assert!(
            response.len() < 256,
            "the rejected alias must not be echoed"
        );
    }

    #[test]
    fn repeated_supplement_bytes_are_refused_on_the_auction_path() {
        boot();
        let app = configured_app();
        let bidder = auction_bidder();
        let imps = (0_u32..3)
            .map(|index| {
                serde_json::json!({
                    "id": format!("imp-{index}"),
                    "banner": {"w": 300_i32, "h": 250_i32},
                    "ext": {"prebid": {"bidder": {bidder.name(): {}}}}
                })
            })
            .collect::<Vec<_>>();
        let body = serde_json::to_string(&serde_json::json!({
            "id": "supplement-bound",
            "site": {"publisher": {"id": "acct-node-budget"}},
            "imp": imps,
            "ext": {"prebid": {"bidderparams": {bidder.name(): {"payload": "x".repeat(100_000)}}}}
        }))
        .unwrap();
        assert!(body.len() < usize::try_from(CAP_256_KIB).unwrap());
        assert!(node_count(body.as_bytes()) < NODE_BUDGET);
        let (status, response) = answer(&app, &body);
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(response.contains("supplement exceeds the materialization budget"));
        assert!(response.len() < 256);
    }

    /// A request ONE NODE over the budget is refused, with the pinned
    /// wire body.
    ///
    /// Today's handler parses this with ordinary `serde_json`, which
    /// accepts it: the body is far inside the 256 KiB cap, every field
    /// is structurally valid, and nothing else on the path counts
    /// nodes. The response is a 200 with a real bid response.
    #[test]
    fn a_request_one_node_over_the_budget_is_refused() {
        boot();
        let app = configured_app();
        let body = body_with_nodes("acct-node-budget", NODE_BUDGET.saturating_add(1));
        assert!(
            body.len() < 0x0004_0000,
            "one node over the budget still fits the body cap: that gap is the finding",
        );

        let (status, wire) = answer(&app, &body);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "one node past the budget is bad client input, not an internal fault: the \
             envelope's inbound-parse term paid for this many nodes and no more",
        );
        assert_eq!(
            wire, NODE_BUDGET_BODY,
            "the refusal carries the pinned wire body byte for byte, newline included",
        );
    }

    /// Over the budget AND naming a disabled account: the node budget
    /// decides.
    ///
    /// This is the precedence assertion, and it is the one that fails
    /// against a handler which resolves the account first — that
    /// handler answers 503 `AccountDisabled` and never reaches a parse
    /// at all. The node budget is the first thing the bytes are
    /// measured against, so it concludes before account resolution can.
    #[test]
    fn the_node_budget_refusal_wins_over_a_disabled_account() {
        boot();
        let app = configured_app();

        // The control: the SAME disabled account, inside the budget,
        // really does produce the 503. Without this the test below
        // could pass against a config whose overlay never loaded.
        let inside = body_with_nodes(DISABLED_ACCOUNT, NODE_BUDGET);
        let (control_status, _control) = answer(&app, &inside);
        assert_eq!(
            control_status,
            StatusCode::SERVICE_UNAVAILABLE,
            "the fixture's account really is disabled, so the precedence assertion below \
             has a competing error to win against",
        );

        let over = body_with_nodes(DISABLED_ACCOUNT, NODE_BUDGET.saturating_add(1));
        let (status, wire) = answer(&app, &over);
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "the node budget is checked on the first traversal of the bytes, so it \
             concludes before account resolution does: a 503 here would mean the handler \
             walked an over-budget tree to look up an account id",
        );
        assert_eq!(
            wire, NODE_BUDGET_BODY,
            "and it is the node-budget refusal, not some other 400",
        );
    }
}

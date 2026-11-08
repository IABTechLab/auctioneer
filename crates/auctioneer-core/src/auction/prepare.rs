//! Shared admission and policy projection for both auction representations.

use super::operation::{PreparedAuction, PreparedState};
use super::{AuctionFailure, AuctionFailureReason, AuctionFunding, AuctionServices};
use crate::config::{
    AuctioneerAppConfig, bidder_settings::RuntimeBidderSettings, budgets::resolve_policy_budgets,
    policy_host::resolve_policy_host, resolve,
};
use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_config::currency::FloorCurrencyConfig;
use auctioneer_config_data::types::HostConfig;
use auctioneer_exchange::debug::DebugPermission;
use auctioneer_exchange::{AuctionPlan, BootEnvelope, participating_bidders, resolve_aliases};
use auctioneer_openrtb::{PrepareLimits, RequestHeaders, prepare, types::bid_request::BidRequest};
use auctioneer_runtime::{
    BoundedJsonError, bounded_json,
    json_preflight::{MAX_CONSTRUCTION_DEPTH, preflight},
    serialized_json_bytes,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_AUCTION_BODY: usize = 0x0004_0000;
pub(crate) const MAX_AUCTION_IMPRESSIONS: u16 = 100;

/// Borrowed representations share one policy and execution pipeline.
#[derive(Clone, Copy)]
pub enum AuctionInput<'input> {
    Json {
        bytes: &'input [u8],
        headers: RequestHeaders<'input>,
    },
    Typed {
        request: &'input BidRequest,
        headers: RequestHeaders<'input>,
    },
}

/// Project accepted settings into a funded, request-owned operation.
///
/// # Errors
/// Returns the existing bounded input/account/privacy/resource categories.
/// Input admission precedes account projection, which precedes typed validation.
///
/// # Examples
/// ```no_run
/// use auctioneer_core::{auction::{AuctionFailure, AuctionFunding, AuctionInput,
///     AuctionServices, PreparedAuction, prepare_auction}, config::AuctioneerAppConfig};
/// use auctioneer_openrtb::{RequestHeaders, types::bid_request::BidRequest};
/// fn prepare_local<'host>(settings: &AuctioneerAppConfig, request: &BidRequest,
///     services: AuctionServices<'host>, funding: AuctionFunding)
///     -> Result<PreparedAuction<'host>, AuctionFailure>
/// {
///     prepare_auction(settings, AuctionInput::Typed {
///         request, headers: RequestHeaders::default(),
///     }, services, funding)
/// }
/// ```
pub fn prepare_auction<'host>(
    settings: &AuctioneerAppConfig,
    input: AuctionInput<'_>,
    host: AuctionServices<'host>,
    funding: AuctionFunding,
) -> Result<PreparedAuction<'host>, AuctionFailure> {
    let mut debug_requested = host.debug_requested;
    let state = match prepare_state(settings, input, &host, &funding, &mut debug_requested) {
        Ok(state) => state,
        Err(reason) => {
            return Err(AuctionFailure::new(reason, funding).with_debug_requested(debug_requested));
        }
    };
    Ok(PreparedAuction::new(state, host, funding))
}

fn prepare_state(
    settings: &AuctioneerAppConfig,
    input: AuctionInput<'_>,
    services: &AuctionServices<'_>,
    funding: &AuctionFunding,
    debug_requested: &mut bool,
) -> Result<PreparedState, AuctionFailureReason> {
    funding
        .ensure(services.resources)
        .map_err(AuctionFailureReason::Resource)?;
    if services.deadline_spent() {
        return Err(AuctionFailureReason::Deadline);
    }
    if !services.bindings_match() {
        return Err(AuctionFailureReason::Unavailable(
            "bidder implementation bindings do not match registry",
        ));
    }
    let limits = &services.resources.inputs().limits;
    let host = resolve_policy_host(services.product_host, settings, limits);
    let AdmittedInput {
        raw,
        account,
        original_ext,
        headers,
    } = admit_input(settings, input, &host, services)?;
    let request = prepare(raw, &headers, &normalization_limits(limits))
        .map_err(|error| AuctionFailureReason::invalid(&error))?;
    *debug_requested = DebugPermission::requested(&request, *debug_requested);
    auctioneer_privacy::validate_wire_signal_input(&request)
        .map_err(AuctionFailureReason::Privacy)?;
    let disabled = disabled_from_overlay(settings, services.bidders);
    let aliases = resolve_aliases(&request, services.bidders, &disabled, &host.aliases)
        .map_err(AuctionFailureReason::Engine)?;
    let requested = participating_bidders(&request, services.bidders, &aliases.map);
    let needed = |name: &str| {
        requested.iter().any(|candidate| {
            candidate.as_str() == name
                || aliases
                    .map
                    .bases
                    .get(candidate)
                    .is_some_and(|base| base.as_str() == name)
        })
    };
    let budgets = resolve_policy_budgets(
        settings,
        services.product_host,
        &account.selection,
        services.bidders,
        &aliases,
    );
    let sample_rates = settings
        .bidders
        .iter()
        .filter(|(name, _)| needed(name))
        .filter_map(|(name, row)| {
            row.sample_rate
                .map(|rate| (BidderName::new(name.as_str()), rate))
        })
        .collect();
    let timeouts = settings
        .bidders
        .iter()
        .filter(|(name, _)| needed(name))
        .filter_map(|(name, row)| {
            row.timeout_ms
                .map(|timeout| (BidderName::new(name.as_str()), timeout))
        })
        .collect::<BTreeMap<_, _>>();
    let bidder_settings = RuntimeBidderSettings::project(settings, &requested, &aliases.map);
    let plan = AuctionPlan::new(aliases, disabled, budgets, sample_rates).with_boot_envelope(
        BootEnvelope {
            max_transport_bytes: limits.max_transport_bytes,
            max_decoded_bytes: limits.max_decoded_bytes,
            construction_costs: services.resources.inputs().construction_costs,
        },
    );
    if services.deadline_spent() {
        return Err(AuctionFailureReason::Deadline);
    }
    Ok(PreparedState {
        request,
        host,
        account,
        plan,
        settings: bidder_settings,
        floor_currency: settings
            .floor_currency
            .as_ref()
            .map(|snapshot| FloorCurrencyConfig {
                expires_at: snapshot.expires_at,
                usd_per_unit: snapshot.usd_per_unit.clone(),
            }),
        timeouts,
        features: resolve::resolved_features(settings),
        original_ext,
        debug_requested: *debug_requested,
    })
}

fn normalization_limits(limits: &auctioneer_runtime::ResolvedIsolateLimits) -> PrepareLimits {
    PrepareLimits {
        max_impressions: u32::from(
            limits
                .max_impressions_per_auction
                .min(MAX_AUCTION_IMPRESSIONS),
        ),
        max_copied_header_bytes: limits.max_copied_header_bytes,
    }
}

struct AdmittedInput<'input> {
    raw: BidRequest,
    account: resolve::ResolvedAccountOwned,
    original_ext: bounded_json::JsonFieldShape,
    headers: RequestHeaders<'input>,
}

fn admit_input<'input>(
    settings: &AuctioneerAppConfig,
    input: AuctionInput<'input>,
    host: &HostConfig,
    services: &AuctionServices<'_>,
) -> Result<AdmittedInput<'input>, AuctionFailureReason> {
    let body_cap = services
        .resources
        .inputs()
        .limits
        .inbound_request_bytes
        .min(u64::try_from(MAX_AUCTION_BODY).unwrap_or(u64::MAX));
    let max_nodes = host.resource_limits.max_parsed_nodes;
    match input {
        AuctionInput::Json { bytes, headers } => {
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > body_cap {
                return Err(AuctionFailureReason::invalid(&"request body too large"));
            }
            let (_, original_ext) =
                bounded_json::count_json_nodes_with_field(bytes, max_nodes, "ext")
                    .map_err(node_error)?;
            let account =
                resolve::resolve_account(bytes, settings, services.product_account, max_nodes)
                    .map_err(AuctionFailureReason::Engine)?;
            let raw = serde_json::from_slice(bytes)
                .map_err(|error| AuctionFailureReason::invalid(&error))?;
            Ok(AdmittedInput {
                raw,
                account,
                original_ext,
                headers,
            })
        }
        AuctionInput::Typed { request, headers } => {
            preflight(request, u64::from(max_nodes), MAX_CONSTRUCTION_DEPTH)
                .map_err(|error| AuctionFailureReason::invalid(&error))?;
            let bytes = serialized_json_bytes(request)
                .map_err(|error| AuctionFailureReason::invalid(&error))?;
            if bytes > body_cap {
                return Err(AuctionFailureReason::invalid(&"request body too large"));
            }
            let account =
                resolve::resolve_account_from_request(request, settings, services.product_account)
                    .map_err(AuctionFailureReason::Engine)?;
            Ok(AdmittedInput {
                raw: request.clone(),
                account,
                original_ext: bounded_json::JsonFieldShape::Unavailable,
                headers,
            })
        }
    }
}

fn node_error(error: BoundedJsonError) -> AuctionFailureReason {
    match error {
        BoundedJsonError::Syntax(syntax) => AuctionFailureReason::invalid(&syntax),
        BoundedJsonError::NodeBudget(_) | BoundedJsonError::RawValue(_) => {
            AuctionFailureReason::invalid(&"too many JSON nodes")
        }
    }
}

pub(crate) fn disabled_from_overlay(
    settings: &AuctioneerAppConfig,
    registry: &BidderRegistry,
) -> BTreeSet<BidderName> {
    let mut disabled = settings
        .bidders
        .keys()
        .map(|name| BidderName::new(name.as_str()))
        .filter(|name| !resolve::bidder_enabled(settings, name))
        .collect::<BTreeSet<_>>();
    disabled.extend(
        registry
            .infos()
            .filter(|info| {
                !resolve::effective_bidder_enabled(settings, info, |name| {
                    registry.get(name).map(|(ancestor, _)| ancestor)
                })
            })
            .map(|info| BidderName::new(info.name.as_str())),
    );
    disabled
}

#[cfg(test)]
mod tests {
    use super::super::{
        AuctionFailureReason, AuctionFunding, AuctionInput, AuctionResourceInputs,
        AuctionResourceProfile, AuctionServices, CompletionStatus, SchemaAllowance,
        prepare_auction,
    };
    use crate::bidder_fixture;
    use crate::config::AuctioneerAppConfig;
    use crate::test_support::boot_parity_fixture::profile_for_adapter;
    use auctioneer_bidder::{BidderHttpResponse, EntryPoint};
    use auctioneer_config_data::types::{AccountConfig, BidderImplementationBinding, HostConfig};
    use auctioneer_exchange::{
        AuctionDeadline, AuctionError, BidderCall, BidderCallResult, BidderFanout,
        FanoutInternalError,
    };
    use auctioneer_openrtb::{RequestHeaders, types::bid_request::BidRequest};
    use auctioneer_privacy::{UnrestrictedHostRestrictions, gcl::Gcl, gvl::GvlStore};
    use auctioneer_runtime::{AdmissionGate, AuctionBudget, Clock as _, FakeClock, SeededRng};
    use futures::{executor::block_on, future::pending, task::noop_waker_ref};
    use serde_json::json;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicU64, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    #[derive(Default)]
    struct FakeFanout {
        calls: AtomicUsize,
        timeout_ms: AtomicU64,
    }

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for FakeFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.calls.fetch_add(calls.len(), Ordering::SeqCst);
            if let Some(call) = calls.first() {
                self.timeout_ms.store(
                    u64::try_from(call.timeout.as_millis()).unwrap(),
                    Ordering::SeqCst,
                );
            }
            Ok(calls.into_iter().map(|_call| BidderCallResult {
                result: Ok(BidderHttpResponse::new(200, Vec::new(), serde_json::to_vec(&json!({
                    "id": "fake_response", "cur": "USD", "seatbid": [{"seat": "fake_upstream", "bid": [{
                        "id": "fake_bid", "impid": "fake_imp", "price": 1.5_f64,
                        "adm": "creative", "crid": "fake_creative", "w": 300_u32, "h": 250_u32, "mtype": 1_u8
                    }]}]
                })).unwrap())), elapsed: Duration::from_millis(1),
            }).collect())
        }
    }

    struct Fixture {
        profile: AuctionResourceProfile,
        product: HostConfig,
        account: AccountConfig,
        clock: FakeClock,
        rng: SeededRng,
        gate: Arc<AdmissionGate>,
        budget: Arc<AuctionBudget>,
        fanout: FakeFanout,
    }

    const BINDINGS: [BidderImplementationBinding; 3] = [
        BidderImplementationBinding {
            bidder: "fake_aaa",
            implementation: "fake_protocol",
        },
        BidderImplementationBinding {
            bidder: "fake_bbb",
            implementation: "fake_protocol",
        },
        BidderImplementationBinding {
            bidder: "fake_pp",
            implementation: "fake_protocol",
        },
    ];

    impl Fixture {
        fn new() -> Self {
            let row = profile_for_adapter("unequal_explicit_caps", "axum");
            let profile = AuctionResourceProfile::new(
                AuctionResourceInputs {
                    limits: row.limits,
                    target: row.target,
                    outbound_meta_bytes: row.outbound_meta_bytes,
                    construction_costs: row.construction_costs,
                    product_default_bytes: 271,
                    catalog_costs: row.catalog_costs,
                },
                SchemaAllowance::Disabled,
            )
            .unwrap();
            let budget = Arc::new(AuctionBudget::new(
                profile.derived().auction_charges.total().unwrap(),
            ));
            let mut product = auctioneer_config_data::product_host_default().clone();
            product.gdpr.enabled = false;
            Self {
                profile,
                budget,
                product,
                account: AccountConfig::default(),
                clock: FakeClock::new(time::OffsetDateTime::UNIX_EPOCH),
                rng: SeededRng::new(13),
                gate: AdmissionGate::new(1),
                fanout: FakeFanout::default(),
            }
        }

        fn services(&self) -> AuctionServices<'_> {
            AuctionServices {
                resources: &self.profile,
                product_host: &self.product,
                product_account: &self.account,
                bidders: bidder_fixture::registry(),
                bidder_bindings: &BINDINGS,
                fanout: &self.fanout,
                clock: &self.clock,
                rng: &self.rng,
                gvl_store: GvlStore::baked(),
                gcl: Gcl::baked(),
                restrictions: &UnrestrictedHostRestrictions,
                credentials: None,
                request_start: self.clock.now_monotonic(),
                outer_deadline: None,
                now: self.clock.now(),
                debug_requested: false,
                entry_point: EntryPoint::Auction,
            }
        }

        fn funding(&self) -> AuctionFunding {
            AuctionFunding::prepay(
                &self.profile,
                &self.budget,
                self.gate.try_acquire().unwrap(),
            )
            .unwrap()
        }
    }

    fn request() -> BidRequest {
        serde_json::from_value(json!({"id": "fake_request", "site": {"id": "fake_site"},
            "imp": [{"id": "fake_imp", "banner": {"w": 300, "h": 250},
                "ext": {"prebid": {"bidder": {"fake_aaa": {}}}}}]}))
        .unwrap()
    }

    #[derive(Clone, Copy)]
    enum AliasSource {
        Direct,
        Request,
        Host,
    }

    fn dispatch_with_override(
        alias_source: AliasSource,
        overrides: &serde_json::Value,
    ) -> (usize, u64) {
        let fixture = Fixture::new();
        let requested = if matches!(alias_source, AliasSource::Direct) {
            "fake_aaa"
        } else {
            "fake_alias"
        };
        let mut policy_json = json!({"schema_version": 1_u8, "bidders": {"fake_aaa": overrides}});
        let mut body = serde_json::to_value(request()).unwrap();
        body["tmax"] = json!(1000_u32);
        body["imp"][0]["ext"]["prebid"]["bidder"] = json!({(requested): {}});
        match alias_source {
            AliasSource::Direct => {}
            AliasSource::Request => {
                body["ext"] = json!({"prebid": {"aliases": {"fake_alias": "fake_aaa"}}});
            }
            AliasSource::Host => {
                policy_json["host"] = json!({"aliases": {"fake_alias": "fake_aaa"}});
            }
        }
        let policy: AuctioneerAppConfig = serde_json::from_value(policy_json).unwrap();
        let input: BidRequest = serde_json::from_value(body.clone()).unwrap();
        let prepared = prepare_auction(
            &policy,
            AuctionInput::Typed {
                request: &input,
                headers: RequestHeaders::default(),
            },
            fixture.services(),
            fixture.funding(),
        )
        .unwrap();
        let mut completed = block_on(prepared.run()).unwrap();
        assert_eq!(serde_json::to_value(&input).unwrap(), body);
        assert_eq!(fixture.gate.in_use(), 1);
        assert!(fixture.budget.in_use() > 0);
        completed.finish(CompletionStatus::Success);
        drop(completed);
        assert_eq!(fixture.gate.in_use(), 0);
        assert_eq!(fixture.budget.in_use(), 0);
        (
            fixture.fanout.calls.load(Ordering::SeqCst),
            fixture.fanout.timeout_ms.load(Ordering::SeqCst),
        )
    }

    #[test]
    fn preparation_preserves_base_timeout_for_request_and_host_aliases() {
        for alias in [AliasSource::Direct, AliasSource::Request, AliasSource::Host] {
            let observed = dispatch_with_override(alias, &json!({"timeout_ms": 321_u32}));
            assert_eq!(observed, (1, 321));
        }
    }

    #[test]
    fn preparation_preserves_base_sampling_for_request_and_host_aliases() {
        for alias in [AliasSource::Direct, AliasSource::Request, AliasSource::Host] {
            for (rate, expected_calls) in [(0.0_f64, 0_usize), (1.0_f64, 1)] {
                let (calls, _) = dispatch_with_override(alias, &json!({"sample_rate": rate}));
                assert_eq!(calls, expected_calls);
            }
        }
    }

    #[test]
    fn settings_first_preparation_owns_borrowed_input_and_retains_the_acquired_slot() {
        let fixture = Fixture::new();
        let policy = AuctioneerAppConfig::default();
        let input = request();
        let before = serde_json::to_value(&input).unwrap();
        let prepared = prepare_auction(
            &policy,
            AuctionInput::Typed {
                request: &input,
                headers: RequestHeaders::default(),
            },
            fixture.services(),
            fixture.funding(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(&input).unwrap(), before);
        assert_eq!(fixture.gate.in_use(), 1);
        assert!(fixture.budget.in_use() > 0);
        drop(input);
        drop(policy);
        let completed = block_on(prepared.run()).unwrap();
        assert_eq!(completed.outcome().request_id(), "fake_request");
        assert_eq!(completed.outcome().bidders().len(), 1);
        assert_eq!(
            completed
                .outcome()
                .bidders()
                .get("fake_aaa")
                .unwrap()
                .bids
                .len(),
            1
        );
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.gate.in_use(), 1);
        let response = completed.into_wire_response().unwrap();
        assert_eq!(response.response().id, "fake_request");
        assert!(fixture.gate.try_acquire().is_none());
        drop(response);
        assert_eq!(fixture.gate.in_use(), 0);
        assert_eq!(fixture.budget.in_use(), 0);
    }

    #[test]
    fn expired_outer_cap_precedes_parsing_and_binding_validation() {
        let fixture = Fixture::new();
        let mut services = fixture.services();
        services.bidder_bindings = &[];
        services.outer_deadline = Some(services.request_start);
        let failure = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Json {
                bytes: b"{",
                headers: RequestHeaders::default(),
            },
            services,
            fixture.funding(),
        )
        .unwrap_err();
        assert!(matches!(failure.reason(), AuctionFailureReason::Deadline));
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.gate.in_use(), 1);
        assert!(fixture.budget.in_use() > 0);
        drop(failure);
        assert_eq!(fixture.gate.in_use(), 0);
        assert_eq!(fixture.budget.in_use(), 0);
    }

    #[test]
    fn explicit_bindings_cannot_be_invented_or_missing() {
        let fixture = Fixture::new();
        let mut services = fixture.services();
        services.bidder_bindings = &BINDINGS[..2];
        let failure = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Typed {
                request: &request(),
                headers: RequestHeaders::default(),
            },
            services,
            fixture.funding(),
        )
        .unwrap_err();
        assert!(matches!(
            failure.reason(),
            AuctionFailureReason::Unavailable(_)
        ));
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
        drop(failure);
        assert_eq!(fixture.gate.in_use(), 0);
    }

    #[test]
    fn disabled_account_precedes_typed_validation_in_both_representations() {
        let fixture = Fixture::new();
        let policy = serde_json::from_value(json!({"schema_version": 1_u8,
            "accounts": {"fake_disabled": {"disabled": true}}}))
        .unwrap();
        let value = json!({"id": "", "imp": [],
            "site": {"publisher": {"id": "fake_disabled"}}});
        let typed: BidRequest = serde_json::from_value(value.clone()).unwrap();
        let raw = serde_json::to_vec(&value).unwrap();
        for input in [
            AuctionInput::Typed {
                request: &typed,
                headers: RequestHeaders::default(),
            },
            AuctionInput::Json {
                bytes: &raw,
                headers: RequestHeaders::default(),
            },
        ] {
            let failure =
                prepare_auction(&policy, input, fixture.services(), fixture.funding()).unwrap_err();
            assert!(
                matches!(failure.reason(), AuctionFailureReason::Engine(AuctionError::AccountDisabled(id)) if id == "fake_disabled")
            );
            assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
            drop(failure);
            assert_eq!(fixture.budget.in_use(), 0);
        }
    }

    #[test]
    fn queue_time_cannot_refresh_the_original_outer_cap() {
        let fixture = Fixture::new();
        let mut services = fixture.services();
        services.outer_deadline = services
            .request_start
            .checked_add(Duration::from_millis(10));
        let prepared = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Typed {
                request: &request(),
                headers: RequestHeaders::default(),
            },
            services,
            fixture.funding(),
        )
        .unwrap();
        fixture.clock.advance(Duration::from_millis(10));
        let failure = block_on(prepared.run()).unwrap_err();
        assert!(matches!(failure.reason(), AuctionFailureReason::Deadline));
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.gate.in_use(), 1);
        drop(failure);
        assert_eq!(fixture.budget.in_use(), 0);
    }

    #[test]
    fn raw_syntax_and_typed_depth_refuse_before_disabled_account_projection() {
        let fixture = Fixture::new();
        let policy = serde_json::from_value(json!({"schema_version": 1_u8,
            "account_defaults": {"disabled": true}}))
        .unwrap();
        let mut raw = serde_json::to_vec(&request()).unwrap();
        raw.push(b'[');
        let syntax_failure = prepare_auction(
            &policy,
            AuctionInput::Json {
                bytes: &raw,
                headers: RequestHeaders::default(),
            },
            fixture.services(),
            fixture.funding(),
        )
        .unwrap_err();
        assert!(matches!(
            syntax_failure.reason(),
            AuctionFailureReason::InvalidRequest(_)
        ));
        drop(syntax_failure);
        let mut typed = request();
        let mut nested = json!(0_u8);
        for _depth in 0_u32..129 {
            nested = json!([nested]);
        }
        typed.other.insert("fake_deep".into(), nested);
        let before = typed.other["fake_deep"].clone();
        let failure = prepare_auction(
            &policy,
            AuctionInput::Typed {
                request: &typed,
                headers: RequestHeaders::default(),
            },
            fixture.services(),
            fixture.funding(),
        )
        .unwrap_err();
        assert!(matches!(
            failure.reason(),
            AuctionFailureReason::InvalidRequest(_)
        ));
        assert_eq!(typed.other["fake_deep"], before);
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
        drop(failure);
        assert_eq!(fixture.budget.in_use(), 0);
    }

    #[test]
    fn suspended_owned_execution_keeps_funding_until_the_future_is_destroyed() {
        use std::{future::Future as _, task::Context};

        struct PendingFanout(AtomicUsize);
        #[async_trait::async_trait(?Send)]
        impl BidderFanout for PendingFanout {
            async fn send_all(
                &self,
                calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                assert_eq!(calls.len(), 1);
                self.0.fetch_add(1, Ordering::SeqCst);
                pending().await
            }
        }

        let fixture = Fixture::new();
        let pending = PendingFanout(AtomicUsize::new(0));
        let mut services = fixture.services();
        services.fanout = &pending;
        let prepared = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Typed {
                request: &request(),
                headers: RequestHeaders::default(),
            },
            services,
            fixture.funding(),
        )
        .unwrap();
        let mut running = Box::pin(prepared.run());
        let mut context = Context::from_waker(noop_waker_ref());
        assert!(running.as_mut().poll(&mut context).is_pending());
        assert_eq!(pending.0.load(Ordering::SeqCst), 1);
        assert!(fixture.budget.in_use() > 0);
        assert_eq!(fixture.gate.in_use(), 1);
        assert!(fixture.gate.try_acquire().is_none());
        drop(running);
        assert_eq!(fixture.budget.in_use(), 0);
        assert_eq!(fixture.gate.in_use(), 0);
        assert!(fixture.gate.try_acquire().is_some());
    }

    #[test]
    fn suspended_credential_loading_keeps_funding_and_never_dispatches_after_drop() {
        use crate::bidder_fixture::fake_bidder::FakeBidder;
        use auctioneer_bidder::BidderRegistry;
        use auctioneer_runtime::bidder_credentials::{
            BidderCredentialSource, BidderCredentials, CredentialUnavailable,
        };
        use std::{future::Future as _, task::Context};

        struct PendingCredentials(AtomicUsize);
        #[async_trait::async_trait(?Send)]
        impl BidderCredentialSource for PendingCredentials {
            async fn load(
                &self,
                reference: &str,
                _deadline: auctioneer_runtime::MonotonicInstant,
            ) -> Result<BidderCredentials, CredentialUnavailable> {
                assert_eq!(reference, "fake_credentials");
                self.0.fetch_add(1, Ordering::SeqCst);
                pending().await
            }
        }

        let fixture = Fixture::new();
        let mut registry = BidderRegistry::new();
        registry
            .register_with_implementation(
                bidder_fixture::registry().get("fake_aaa").unwrap().0,
                "fake_protocol",
                Box::new(FakeBidder {
                    credential_reference: Some("fake_credentials"),
                    ..FakeBidder::default()
                }),
            )
            .unwrap();
        let credentials = PendingCredentials(AtomicUsize::new(0));
        let mut services = fixture.services();
        services.bidders = &registry;
        services.credentials = Some(&credentials);
        let prepared = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Typed {
                request: &request(),
                headers: RequestHeaders::default(),
            },
            services,
            fixture.funding(),
        )
        .unwrap();
        let mut running = Box::pin(prepared.run());
        let mut context = Context::from_waker(noop_waker_ref());
        assert!(running.as_mut().poll(&mut context).is_pending());
        assert_eq!(credentials.0.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
        assert!(fixture.budget.in_use() > 0);
        assert_eq!(fixture.gate.in_use(), 1);
        drop(running);
        assert_eq!(fixture.budget.in_use(), 0);
        assert_eq!(fixture.gate.in_use(), 0);
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn explicit_policy_and_auction_input_vary_independently_on_one_host() {
        let fixture = Fixture::new();
        let mut combinations = 0_usize;
        for enabled in [false, true] {
            let policy: AuctioneerAppConfig = serde_json::from_value(json!({
                "schema_version": 1_u8, "bidders": {"fake_aaa": {"enabled": enabled}}
            }))
            .unwrap();
            for id in ["fake_first_request", "fake_second_request"] {
                let mut input = request();
                input.id = id.to_owned();
                let before = serde_json::to_value(&input).unwrap();
                let calls_before = fixture.fanout.calls.load(Ordering::SeqCst);
                let result = block_on(
                    prepare_auction(
                        &policy,
                        AuctionInput::Typed {
                            request: &input,
                            headers: RequestHeaders::default(),
                        },
                        fixture.services(),
                        fixture.funding(),
                    )
                    .unwrap()
                    .run(),
                );
                if enabled {
                    let completed = result.unwrap();
                    assert_eq!(completed.outcome().request_id(), id);
                    assert!(completed.outcome().selected_bid(0).is_some());
                    drop(completed);
                } else {
                    let failure = result.unwrap_err();
                    assert!(matches!(failure.reason(), AuctionFailureReason::Engine(
                        AuctionError::BadInput(message)
                    ) if message == "imp fake_imp has no valid bidders"));
                    assert_eq!(fixture.gate.in_use(), 1);
                    assert!(fixture.budget.in_use() > 0);
                    drop(failure);
                }
                assert_eq!(
                    fixture.fanout.calls.load(Ordering::SeqCst) - calls_before,
                    usize::from(enabled)
                );
                assert_eq!(serde_json::to_value(&input).unwrap(), before);
                assert_eq!(policy.bidders["fake_aaa"].enabled, Some(enabled));
                assert_eq!(fixture.budget.in_use(), 0);
                assert_eq!(fixture.gate.in_use(), 0);
                combinations += 1;
            }
        }
        assert_eq!(combinations, 4);
    }

    #[test]
    fn normalized_body_debug_survives_a_later_fatal_privacy_refusal() {
        let fixture = Fixture::new();
        for requested in [false, true] {
            let mut value = serde_json::to_value(request()).unwrap();
            value["test"] = json!(u8::from(requested));
            value["regs"] = json!({"gdpr": 2_i32});
            let typed = serde_json::from_value(value.clone()).unwrap();
            let raw = serde_json::to_vec(&value).unwrap();
            for input in [
                AuctionInput::Json {
                    bytes: &raw,
                    headers: RequestHeaders::default(),
                },
                AuctionInput::Typed {
                    request: &typed,
                    headers: RequestHeaders::default(),
                },
            ] {
                let failure = prepare_auction(
                    &AuctioneerAppConfig::default(),
                    input,
                    fixture.services(),
                    fixture.funding(),
                )
                .unwrap_err();
                assert!(matches!(failure.reason(), AuctionFailureReason::Privacy(_)));
                assert_eq!(failure.debug_requested(), requested);
                assert!(fixture.budget.in_use() > 0);
                assert_eq!(fixture.gate.in_use(), 1);
                drop(failure);
                assert_eq!(fixture.budget.in_use(), 0);
            }
        }
    }

    #[test]
    fn fatal_privacy_signal_precedes_alias_errors_in_both_representations() {
        let fixture = Fixture::new();
        let mut value = serde_json::to_value(request()).unwrap();
        value["regs"] = json!({"gdpr": 2_i32});
        value["ext"] = json!({"prebid": {"aliases": {"fake_alias": "fake_missing"}}});
        let typed = serde_json::from_value(value.clone()).unwrap();
        let raw = serde_json::to_vec(&value).unwrap();
        for input in [
            AuctionInput::Json {
                bytes: &raw,
                headers: RequestHeaders::default(),
            },
            AuctionInput::Typed {
                request: &typed,
                headers: RequestHeaders::default(),
            },
        ] {
            let failure = prepare_auction(
                &AuctioneerAppConfig::default(),
                input,
                fixture.services(),
                fixture.funding(),
            )
            .unwrap_err();
            assert!(matches!(
                failure.reason(),
                AuctionFailureReason::Privacy(auctioneer_privacy::PrivacyError::InvalidRegsGdpr {
                    got: 2_i32
                })
            ));
            assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
            drop(failure);
        }
    }

    #[test]
    fn raw_and_typed_inputs_reach_the_same_engine_and_wire_projection() {
        let fixture = Fixture::new();
        let policy = AuctioneerAppConfig::default();
        let typed = request();
        let raw = serde_json::to_vec(&typed).unwrap();
        let run = |input| {
            let prepared =
                prepare_auction(&policy, input, fixture.services(), fixture.funding()).unwrap();
            let completed = block_on(prepared.run()).unwrap();
            let selected = completed.outcome().selected_bid(0).unwrap();
            assert_eq!(selected.requested_bidder.as_str(), "fake_aaa");
            assert_eq!(selected.implementation, Some("fake_protocol"));
            assert_eq!(selected.bid.source_seat.as_deref(), Some("fake_upstream"));
            let mut response = completed.into_wire_response().unwrap();
            response.finish(super::super::CompletionStatus::Success);
            serde_json::to_value(response.response()).unwrap()
        };
        let raw_response = run(AuctionInput::Json {
            bytes: &raw,
            headers: RequestHeaders::default(),
        });
        let typed_response = run(AuctionInput::Typed {
            request: &typed,
            headers: RequestHeaders::default(),
        });
        assert_eq!(raw_response, typed_response);
        assert_eq!(raw_response["seatbid"][0]["seat"], "fake_aaa");
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn dropping_an_unpolled_execution_releases_original_funding_without_dispatch() {
        let fixture = Fixture::new();
        let prepared = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Typed {
                request: &request(),
                headers: RequestHeaders::default(),
            },
            fixture.services(),
            fixture.funding(),
        )
        .unwrap();
        let execution = prepared.run();
        assert_eq!(fixture.gate.in_use(), 1);
        assert!(fixture.budget.in_use() > 0);
        drop(execution);
        assert_eq!(fixture.gate.in_use(), 0);
        assert_eq!(fixture.budget.in_use(), 0);
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn receipts_for_a_smaller_profile_cannot_admit_even_a_malformed_request() {
        let fixture = Fixture::new();
        let mut facts = *fixture.profile.inputs();
        facts.catalog_costs.context_per_auction_bytes += 1;
        let larger = AuctionResourceProfile::new(facts, SchemaAllowance::Disabled).unwrap();
        assert!(
            larger.derived().auction_charges.projection
                > fixture.profile.derived().auction_charges.projection
        );
        let mut services = fixture.services();
        services.resources = &larger;
        let failure = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Json {
                bytes: b"{",
                headers: RequestHeaders::default(),
            },
            services,
            fixture.funding(),
        )
        .unwrap_err();
        assert!(matches!(
            failure.reason(),
            AuctionFailureReason::Resource(auctioneer_runtime::ResourceError::Exhausted {
                kind: auctioneer_runtime::ChargeKind::Projection
            })
        ));
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
        assert!(fixture.budget.in_use() > 0);
        drop(failure);
        assert_eq!(fixture.budget.in_use(), 0);
    }

    #[test]
    fn binding_order_uniqueness_and_canonical_identity_are_independent_requirements() {
        let fixture = Fixture::new();
        for bindings in [
            [BINDINGS[1], BINDINGS[0], BINDINGS[2]],
            [BINDINGS[0], BINDINGS[0], BINDINGS[2]],
            [
                BidderImplementationBinding {
                    bidder: "fake_aaa",
                    implementation: "",
                },
                BINDINGS[1],
                BINDINGS[2],
            ],
        ] {
            let mut services = fixture.services();
            services.bidder_bindings = &bindings;
            let failure = prepare_auction(
                &AuctioneerAppConfig::default(),
                AuctionInput::Typed {
                    request: &request(),
                    headers: RequestHeaders::default(),
                },
                services,
                fixture.funding(),
            )
            .unwrap_err();
            assert!(matches!(
                failure.reason(),
                AuctionFailureReason::Unavailable(_)
            ));
            assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
            drop(failure);
        }
    }

    #[test]
    fn plausible_but_substituted_implementation_is_refused_before_input_work() {
        let fixture = Fixture::new();
        let bindings = [
            BidderImplementationBinding {
                bidder: "fake_aaa",
                implementation: "fake_substituted",
            },
            BINDINGS[1],
            BINDINGS[2],
        ];
        let mut services = fixture.services();
        services.bidder_bindings = &bindings;
        let failure = prepare_auction(
            &AuctioneerAppConfig::default(),
            AuctionInput::Json {
                bytes: b"{",
                headers: RequestHeaders::default(),
            },
            services,
            fixture.funding(),
        )
        .unwrap_err();
        assert!(matches!(
            failure.reason(),
            AuctionFailureReason::Unavailable(_)
        ));
        assert_eq!(fixture.fanout.calls.load(Ordering::SeqCst), 0);
        drop(failure);
        assert_eq!(fixture.budget.in_use(), 0);
    }
}

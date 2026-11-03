//! `POST /openrtb2/auction` handler.

use crate::bidder_registry::BidderInfoIndex;
use crate::bidder_registry::registry_handle;
use crate::config::bidder_settings::RuntimeBidderSettings;
use crate::config::budgets::resolve_budgets;
use crate::config::runtime as config_runtime;
use crate::config::{AuctioneerAppConfig, resolve};
use crate::handlers::debug_signals;
use crate::handlers::egress_deadline::{self, EgressDeadline, EgressRefusal};
use crate::handlers::health_gvl;
use crate::handlers::request_disclosure_audit::{self, AuctionDisclosureScope, CompletionStatus};
use crate::host_primitives::EdgeZeroFanout;
use crate::host_primitives::error_renderer::{MAX_RENDERED_BODY_BYTES, REQUEST_TIMEOUT_BODY};
use crate::host_primitives::gates::{BootGates, boot_gates, boot_limits, egress_gate, stream_gate};
use crate::host_primitives::ingress_lease::{
    AuctioneerIngressLease, RouteEgressPermit, install_route_permit,
};
use crate::host_primitives::registry::{
    self as host_registry, TestClockHandle, TestMonotonicClockHandle, TestRngHandle,
    TestStrategyHandle, TestTimerHandle,
};
use crate::host_primitives::request_clock::RequestClock;
use crate::host_primitives::strategy::FanoutStrategy;
use crate::serialize::serialize_response_fitting_debug;
use auctioneer_bidder::{BidderName, BidderRegistry, FloorConversion};
#[cfg(test)]
use auctioneer_config_data::types::BidderInfo;
use auctioneer_config_data::types::{HostConfig, MAX_BIDDER_NAME_BYTES};
use auctioneer_exchange::context::RequestConstructionLimits;
use auctioneer_exchange::debug::DebugPermission;
use auctioneer_exchange::scrub::DisclosureAudit;
use auctioneer_exchange::{
    AuctionContext, AuctionError, AuctionPlan, BidderFanout, BootEnvelope, ResolvedAccount,
    resolve_aliases, run_auction,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::types::bid_response::BidResponse;
use auctioneer_openrtb::{
    NormalizedBidRequest, PrepareLimits, RequestHeaders, WarningCode, prepare,
};
use auctioneer_privacy::gcl::Gcl;
use auctioneer_privacy::gvl::GvlStore;
use auctioneer_runtime::budget::{NORMALIZATION_IMPLICIT_NODES, normalization_wire_allowance};
use auctioneer_runtime::{
    AuctionBudget, BoundedJsonError, ChargeKind, Clock, MonotonicInstant, Reservation,
    ResolvedIsolateLimits, Rng, Timer, bounded_json,
};
use edgezero_core::action;
use edgezero_core::body::Body;
use edgezero_core::context::RequestContext;
use edgezero_core::error::{EdgeError, RequestBodyOverflowOrigin};
use edgezero_core::http::{
    HeaderMap, HeaderValue, Response, StatusCode,
    header::{CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE},
    response_builder,
};
use edgezero_core::response_egress::ResponseEgressResource;
use edgezero_core::time::MonotonicClock;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::iter::once;
use std::sync::{Arc, OnceLock};
use time::OffsetDateTime;

const MAX_AUCTION_BODY: usize = 0x0004_0000;

/// The ceiling one terminal error body may occupy.
///
/// The envelope's `refusal_body_bytes`, which is what the terminal
/// response reservation funds. Shared with the error renderer's own
/// constant rather than restated, so the two surfaces cannot fund
/// different numbers.
const MAX_ERROR_BODY_BYTES: usize = MAX_RENDERED_BODY_BYTES;

/// Marks a diagnostic the reservation cut short.
const ELLIPSIS: &str = "...";

/// One dynamic diagnostic, prefix included, bounded to
/// [`MAX_ERROR_BODY_BYTES`] with room for the ellipsis and the trailing
/// newline the renderer adds.
///
/// The bound covers the PREFIX as well as the detail. An earlier
/// revision reduced only the detail's room and pushed the prefix
/// unconditionally, so `room` saturating to zero bounded nothing: the
/// message came out at `prefix.len() + ELLIPSIS.len()`, which for a
/// 256-byte prefix renders 260 bytes against a 256-byte reservation.
/// Every shipped caller passes a 17-byte literal, so that was latent —
/// but a helper whose whole purpose is this bound cannot exempt its own
/// parameter from it.
///
/// Both cuts go through [`prefix_within`], which walks character
/// boundaries, so neither can split a multi-byte sequence.
///
/// Bounding at CONSTRUCTION as well as at the render is deliberate: the
/// renderer keeps the wire honest whatever a construction site does,
/// and this keeps the `String` the error value carries in memory — held
/// across the whole return path — bounded too.
fn bounded_message(prefix: &str, detail: &impl fmt::Display) -> String {
    let reserved = ELLIPSIS.len().saturating_add(1);
    let budget = MAX_ERROR_BODY_BYTES.saturating_sub(reserved);
    let kept_prefix = prefix_within(prefix, budget);
    let room = budget.saturating_sub(kept_prefix.len());
    let rendered = detail.to_string();
    let kept = prefix_within(&rendered, room);
    let mut msg = String::with_capacity(MAX_ERROR_BODY_BYTES);
    msg.push_str(kept_prefix);
    msg.push_str(kept);
    // Truncation of EITHER part is what the ellipsis marks.
    if kept_prefix.len() < prefix.len() || kept.len() < rendered.len() {
        msg.push_str(ELLIPSIS);
    }
    msg
}

/// The longest prefix of `text` that fits `limit` bytes.
///
/// `str::floor_char_boundary` is still unstable, and slicing a
/// multi-byte sequence in half panics. Caller-influenced text may hold
/// any UTF-8, so the cut is found by walking character boundaries
/// rather than assumed — and returned as a `&str` so no caller has to
/// index back into the string with the byte offset.
fn prefix_within(text: &str, limit: usize) -> &str {
    if limit >= text.len() {
        return text;
    }
    text.char_indices()
        .map(|(offset, _ch)| offset)
        .chain(once(text.len()))
        .take_while(|offset| *offset <= limit)
        .last()
        .and_then(|end| text.get(..end))
        .unwrap_or("")
}
/// Fallback impression cap when no boot envelope is installed.
///
/// Declared in the NARROWER of its two consumers' widths — the outbound
/// meta builder's imp-id offset table is `u16` — so the only conversion
/// needed is the lossless widening to the `u32` `PrepareLimits` takes.
/// Declaring it `u32` instead would have forced a narrowing cast that
/// could silently truncate.
const MAX_AUCTION_IMPRESSIONS: u16 = 100;

/// Test-side handle a test installs into the `RequestContext` to swap
/// the fanout. The handler consults this handle before constructing the
/// production fanout from boot-installed outbound capabilities.
#[derive(Clone)]
pub struct TestFanoutHandle(pub Arc<dyn BidderFanout + Send + Sync>);

/// In-process resource fault injection. Only a ceiling at or below the boot
/// derivation is accepted; request headers cannot install this extension.
#[derive(Clone)]
pub struct TestAuctionBudget(pub Arc<AuctionBudget>);

#[action]
pub async fn auction(ctx: RequestContext) -> Result<Response, EdgeError> {
    let mut debug_requested = false;
    let response = auction_response(ctx, &mut debug_requested).await?;
    if debug_requested {
        Ok(no_store(response))
    } else {
        Ok(response)
    }
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn auction_response(
    ctx: RequestContext,
    debug_requested: &mut bool,
) -> Result<Response, EdgeError> {
    // ONE clock source for the whole request. The monotonic half is
    // EdgeZero's own per-request clock, so the exchange, the fanout
    // and EdgeZero itself all read the same instants; the wall half is
    // the boot-installed host clock, which is the only side that knows
    // the calendar.
    let monotonic = monotonic_clock_handle(&ctx);
    // `request_start` anchors tmax. It is EdgeZero's own admission
    // instant, read as-is rather than resampled, so tmax counts from
    // the moment the request was admitted rather than the moment this
    // handler happened to run — and it is already in this clock's
    // domain, so no projection sits between the two. A test that
    // overrides the monotonic source reads `request_start` off that
    // source instead, so one clock drives the whole request.
    let request_start = if ctx.extensions().get::<TestMonotonicClockHandle>().is_some() {
        monotonic.now()
    } else {
        ctx.request_start()
    };
    let configured = boot_gates();
    let default_budget =
        AuctionBudget::new(configured.map_or(0, |gates| gates.derived.per_auction_peak));
    let injected = ctx
        .extensions()
        .get::<TestAuctionBudget>()
        .filter(|candidate| candidate.0.limit() <= default_budget.limit())
        .cloned();
    let budget = injected
        .as_ref()
        .map_or(&default_budget, |candidate| candidate.0.as_ref());
    // Prepay before the config read, and retain every phase through serialized
    // response construction. RAII covers early returns and suspended cancellation.
    let reservations = match configured
        .map(|gates| gates.derived.auction_charges.prepay(budget))
        .transpose()
    {
        Ok(held) => held,
        Err(_resource) => {
            // Refuse without inspecting signals or allocating decoder scratch.
            // Static overload answers are uncacheable even without debug.
            return handler_error_to_response(HandlerError::Overloaded("overloaded\n")).map(
                |response| {
                    bound(
                        no_store(response),
                        EgressDeadline::for_refusal(monotonic.now()),
                    )
                },
            );
        }
    };
    *debug_requested = debug_signals::http_requested(ctx.uri().query(), ctx.headers());
    // Retain the whole lease through observation. Reverse drop order keeps
    // the audit funded on cancellation during either config or body reads.
    let lease = ctx
        .take_ingress_grant()
        .and_then(|grant| grant.downcast::<AuctioneerIngressLease>().ok());
    let mut disclosure = AuctionDisclosureScope::new();
    let clock: Arc<dyn Clock + Send + Sync> =
        Arc::new(RequestClock::new(monotonic, wall_clock_handle(&ctx)));
    // ONE config-resolution owner, called from the OUTER action because
    // its `Err` arm renders an `EdgeError` response that `auction_inner`
    // (which returns `HandlerError`) could not return. It also has to
    // run before the body is consumed: the extractor borrows `ctx`,
    // while reading the body moves it.
    let app_cfg = match config_runtime::resolve_runtime_config(&ctx).await {
        Ok(cfg) => cfg,
        // Bounded like every other answer this handler gives: a
        // config-store outage is exactly when an unbounded write leg
        // costs most. It is a STATIC-class answer, so its budget is
        // fresh and anchored here rather than carved out of a request
        // allowance that a slow config store has already spent.
        Err(err) => {
            disclosure.finish(CompletionStatus::Error);
            let refusal = EgressDeadline::for_refusal(clock.now_monotonic());
            return err.into_response().map(|response| bound(response, refusal));
        }
    };
    let rng = rng_handle(&ctx);
    let registry = match registry_handle() {
        Ok(registry) => registry,
        Err(error) => {
            disclosure.finish(CompletionStatus::Error);
            let refusal = EgressDeadline::for_refusal(clock.now_monotonic());
            return config_runtime::RuntimeConfigError::from(error)
                .into_response()
                .map(|response| bound(response, refusal));
        }
    };
    // Borrow only the egress resource; the lease also owns auction admission.
    let egress_resource = lease.as_ref().map(|held| &held.route_egress_resource);
    // `auction_inner` returns its SUCCESS response already bound: the
    // normal auction's budget is anchored at egress entry, which is an
    // instant only the egress leg knows. Everything the error arm
    // produces is a static-class answer and is bound here, with a fresh
    // budget anchored at the refusal rather than at admission.
    let response = match auction_inner(
        ctx,
        app_cfg,
        Arc::clone(&clock),
        rng,
        request_start,
        egress_resource,
        reservations.as_ref(),
        disclosure.audit_mut(),
        debug_requested,
        registry,
    )
    .await
    {
        Ok(response) => {
            disclosure.finish(CompletionStatus::Success);
            response
        }
        Err(err) => {
            disclosure.finish(CompletionStatus::Error);
            bound(
                handler_error_to_response(err)?,
                EgressDeadline::for_refusal(clock.now_monotonic()),
            )
        }
    };
    // Destroy the finished audit owner before releasing its charged resources.
    drop(disclosure);
    // The auction's work is over, so the concurrency slot returns here
    // rather than at the response's terminal egress. The egress permit
    // the slot inside this lease holds is owned by the completion and is
    // unaffected.
    drop(lease);
    drop(reservations);
    Ok(response)
}

/// Attach the write deadline the host enforces the transmission leg
/// from.
///
/// Applied on EVERY response this handler returns — success, refusal,
/// 500, config-store outage — so no answer leaves with an unbounded
/// write leg. It is the fourth checkpoint, and the only one auctioneer does
/// not run itself. What varies per caller is the ANCHOR, never whether
/// the attachment happens: a success binds the envelope budget from
/// egress entry, every static answer binds a fresh refusal budget from
/// itself.
fn bound(mut response: Response, deadline: EgressDeadline) -> Response {
    response
        .extensions_mut()
        .insert(deadline.to_response_extension());
    response
}

fn boot_prepare_limits() -> PrepareLimits {
    PrepareLimits {
        max_impressions: boot_limits().map_or(u32::from(MAX_AUCTION_IMPRESSIONS), |limits| {
            u32::from(limits.max_impressions_per_auction).min(u32::from(MAX_AUCTION_IMPRESSIONS))
        }),
        max_copied_header_bytes: boot_limits().map_or_else(
            || PrepareLimits::default().max_copied_header_bytes,
            |limits| limits.max_copied_header_bytes,
        ),
    }
}

fn node_budget_error(failure: BoundedJsonError) -> HandlerError {
    match failure {
        BoundedJsonError::Syntax(err) => {
            HandlerError::bad_request_bounded("Invalid request: ", &err)
        }
        BoundedJsonError::NodeBudget(_) | BoundedJsonError::RawValue(_) => {
            HandlerError::bad_request(NODE_BUDGET_BODY)
        }
    }
}

fn normalized_construction_limits(
    input_nodes: u64,
    inbound_bytes: u64,
    copied_header_bytes: u64,
) -> Result<RequestConstructionLimits, HandlerError> {
    let max_nodes = input_nodes
        .checked_add(NORMALIZATION_IMPLICIT_NODES)
        .ok_or(HandlerError::Overloaded("overloaded\n"))?;
    let max_serialized_bytes =
        normalization_wire_allowance(inbound_bytes, copied_header_bytes, input_nodes)
            .map_err(|_overflow| HandlerError::Overloaded("overloaded\n"))?;
    let construction_costs = boot_gates()
        .map_or_else(request_disclosure_audit::construction_costs, |gates| {
            gates.construction_costs
        });
    Ok(RequestConstructionLimits {
        max_nodes,
        max_serialized_bytes,
        reusable_scratch_bytes: construction_costs.reusable_scratch_bytes,
        retained_per_scope_bytes: construction_costs.retained_per_scope_bytes,
    })
}

/// Exercise normal handler preparation and exchange dispatch with an explicit
/// registry. Only conformance builds expose this seam; serving uses the shared
/// published registry and its ingress/resource owners.
/// Supplied phase receipts must remain prepaid through response construction.
///
/// # Errors
/// Returns the handler's normal rendering error if a refusal cannot be built.
#[cfg(feature = "conformance-suite")]
pub async fn auction_for_conformance(
    ctx: RequestContext,
    app_cfg: AuctioneerAppConfig,
    clock: Arc<dyn Clock + Send + Sync>,
    rng: Arc<dyn Rng + Send + Sync>,
    registry: &BidderRegistry,
    reservations: Option<&[Reservation<'_>; 6]>,
) -> Result<Response, EdgeError> {
    let request_start = clock.now_monotonic();
    match auction_inner(
        ctx,
        app_cfg,
        clock,
        rng,
        request_start,
        None,
        reservations,
        &mut DisclosureAudit::default(),
        &mut false,
        registry,
    )
    .await
    {
        Ok(response) => Ok(response),
        Err(error) => handler_error_to_response(error),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "request-scoped resource owners and mutable disclosure audit are passed explicitly"
)]
#[expect(
    clippy::too_many_lines,
    reason = "one ordered handler keeps validation, charged request construction and audit lifetimes together"
)]
async fn auction_inner(
    ctx: RequestContext,
    mut app_cfg: AuctioneerAppConfig,
    clock: Arc<dyn Clock + Send + Sync>,
    rng: Arc<dyn Rng + Send + Sync>,
    request_start: MonotonicInstant,
    egress_resource: Option<&ResponseEgressResource<RouteEgressPermit>>,
    reservations: Option<&[Reservation<'_>; 6]>,
    audit: &mut DisclosureAudit,
    debug_requested: &mut bool,
    registry: &BidderRegistry,
) -> Result<Response, HandlerError> {
    // Host request-body, population and node limits resolve before their
    // consumers. Scoped response caps resolve later, once the account and
    // aliases are known; the fanout retains the funded response ceilings.
    let baked_host = host_config_handle();
    let mut host = resolve::resolve_host(&app_cfg, baked_host);
    if let Some(limits) = boot_limits() {
        resolve::clamp_host_to_boot(&mut host, limits);
    }
    let fanout = fanout_from_context(&ctx, Arc::clone(&clock), &host)
        .ok_or_else(HandlerError::fanout_unavailable)?;
    let credential_loader = super::bidder_credentials::CredentialLoader::capture(&ctx);
    let raw_headers = ctx.headers().clone();
    if let Some(enc) = raw_headers.get(CONTENT_ENCODING)
        && !enc.is_empty()
    {
        // Bounded: the header VALUE is caller input, and the admitted
        // header block allows far more of it than a terminal body is
        // funded for.
        return Err(HandlerError::bad_request_bounded(
            "Invalid request: Content-Encoding of type ",
            &format_args!("{} is not supported", enc.to_str().unwrap_or("<unknown>")),
        ));
    }
    let body_cap = boot_limits().map_or(MAX_AUCTION_BODY, |limits| {
        usize::try_from(limits.inbound_request_bytes)
            .unwrap_or(usize::MAX)
            .min(MAX_AUCTION_BODY)
    });
    let request = ctx
        .into_request()
        .map_err(|e| HandlerError::from_body_read_error(&e, body_cap))?;
    let (_parts, body) = request.into_parts();
    let body_bytes = body
        .into_bytes_bounded(body_cap)
        .await
        .map_err(|e| HandlerError::from_body_read_error(&e, body_cap))?;

    // Validate every syntactic node before account discovery or typed
    // allocation. The prepaid ingress receipt funds the later typed parse,
    // which preserves opaque RawValue extensions rather than rebuilding them.
    //
    // A byte cap does not bound what a parse RETAINS. The inbound-parse
    // term in the memory envelope is sized as a node count times a
    // per-node retained-byte figure, so the thing that has to be
    // counted is nodes — and a flat array of small numbers is a few
    // bytes per node on the wire against hundreds in the heap, which is
    // how a body well inside the 256 KiB cap outruns the term that paid
    // for it. Counting is syntactic, so passthrough content a typed
    // target discards costs exactly what it cost the parser to walk.
    //
    // This runs BEFORE account resolution, which is what makes the node
    // budget decide first: looking up a publisher id means walking the
    // tree, and walking an over-budget tree to find out whose account
    // to blame is the cost the budget exists to refuse. An over-budget
    // body is a 400 even when it also names a disabled account.
    //
    // The budget comes from the RESOLVED config rather than the
    // envelope row directly, so a deployment that lowered
    // `max_parsed_nodes` gets the smaller number here. Resolution
    // already clamped it to the funded figure, so this can only be at
    // or below what the envelope charged for.
    let (_, original_request_ext_shape) = bounded_json::count_json_nodes_with_field(
        &body_bytes,
        host.resource_limits.max_parsed_nodes,
        "ext",
    )
    .map_err(node_budget_error)?;

    let baked_account = auctioneer_config_data::account_default();
    // Both parses share the installed syntactic node ceiling. Direct
    // in-process calls without an envelope have no configured ceiling.
    let max_nodes = host.resource_limits.max_parsed_nodes;
    // The node budget precedes account state; otherwise a disabled
    // account still wins over typed/structural request validation.
    let owned_account = resolve::resolve_account(&body_bytes, &app_cfg, baked_account, max_nodes)
        .map_err(HandlerError::from_auction_error)?;

    let raw: BidRequest = serde_json::from_slice(&body_bytes)
        .map_err(|err| HandlerError::bad_request_bounded("Invalid request: ", &err))?;
    let http_headers = headers_from_http(&raw_headers);
    let normalized = prepare(raw, &http_headers, &boot_prepare_limits())
        .map_err(|e| HandlerError::bad_request_bounded("Invalid request: ", &e))?;
    *debug_requested = DebugPermission::requested(&normalized, *debug_requested);

    let features = resolve::resolved_features(&app_cfg);
    let bidder_timeouts = resolve::resolved_bidder_timeouts(&app_cfg);
    let resolved = ResolvedAccount {
        id: owned_account.id.clone(),
        source: owned_account.source,
        config: &owned_account.config,
    };

    // The fatal-input gate runs before any alias work so an alias
    // failure cannot preempt the 400 an invalid wire signal mandates.
    // It is precedence-aware — a non-empty `gpp_sid` overrides an
    // invalid `regs.gdpr` — and malformed TCF/GPP stays
    // warn-and-continue inside the resolver.
    auctioneer_privacy::validate_wire_signal_input(&normalized)
        .map_err(HandlerError::from_privacy_error)?;

    let plan = resolve_auction_plan(
        &normalized,
        &app_cfg,
        &host,
        baked_host,
        &resolved,
        registry,
    )?;
    // Move the parsed settings owner; unrelated accounts/overlays die before fanout.
    let floor_currency = app_cfg.floor_currency.take();
    let bidder_settings = RuntimeBidderSettings::from(app_cfg);

    let mut ctx_auction = AuctionContext::with_request_start_ext(
        request_start,
        &host,
        &resolved,
        registry,
        fanout.as_ref(),
        clock.as_ref(),
        rng.as_ref(),
        GvlStore::baked(),
        Gcl::baked(),
        clock.now(),
        *debug_requested,
        features,
        &bidder_timeouts,
    );
    ctx_auction.original_request_ext_shape = original_request_ext_shape;
    ctx_auction.bidder_settings = Some(&bidder_settings);
    ctx_auction.credential_source = Some(&credential_loader);
    ctx_auction.schema_scratch_bytes =
        boot_gates().map_or(0, BootGates::native_schema_scratch_bytes);
    ctx_auction.floor_currency =
        FloorConversion::new(floor_currency.as_ref(), ctx_auction.now.unix_timestamp());
    ctx_auction.projection_reservation = reservations.and_then(|held| {
        held.iter()
            .find(|receipt| receipt.kind() == ChargeKind::Projection)
    });
    ctx_auction.response_reservation = reservations.and_then(|held| {
        held.iter()
            .find(|receipt| receipt.kind() == ChargeKind::Response)
    });
    ctx_auction.construction_limits = boot_limits()
        .map(|limits| {
            normalized_construction_limits(
                u64::from(limits.max_parsed_nodes),
                limits.inbound_request_bytes,
                u64::from(limits.max_copied_header_bytes),
            )
        })
        .transpose()?;
    let mut resp = run_auction(&normalized, &ctx_auction, &plan, audit)
        .await
        .map_err(HandlerError::from_auction_error)?;
    record_gvl_unavailable(
        &resp,
        clock.now(),
        health_gvl::counter(),
        health_gvl::gcl_stale_counter(),
    );

    egress_leg(&mut resp, clock.as_ref(), egress_resource)
}

fn resolve_auction_plan(
    request: &NormalizedBidRequest,
    app_cfg: &AuctioneerAppConfig,
    host: &HostConfig,
    baked_host: &HostConfig,
    account: &ResolvedAccount<'_>,
    registry: &BidderRegistry,
) -> Result<AuctionPlan, HandlerError> {
    // The disabled set is resolved before alias resolution: a
    // request-declared alias whose base (or own name) is disabled
    // must never make it into the resolved alias map either, so
    // `resolve_aliases` needs the set up front.
    let disabled_bidders = disabled_from_overlay(app_cfg, registry);
    // Keyed by the observable `[bidders.<name>]` name exactly as
    // written — sample_rate on a request-declared alias's OWN name is a
    // distinct override from one on its base bidder; `run_auction`'s
    // sampler resolves observable-then-base, mirroring per-bidder
    // timeout resolution.
    let bidder_sample_rates: BTreeMap<BidderName, f64> = app_cfg
        .bidders
        .iter()
        .filter_map(|(name, o)| o.sample_rate.map(|r| (BidderName::new(name.as_str()), r)))
        .collect();
    let aliases = resolve_aliases(request, registry, &disabled_bidders, &host.aliases)
        .map_err(HandlerError::from_auction_error)?;
    let budgets = resolve_budgets(
        app_cfg,
        baked_host,
        &account.id,
        account.source,
        registry,
        &aliases,
    );
    let mut plan = AuctionPlan::new(aliases, disabled_bidders, budgets, bidder_sample_rates);
    if let Some(envelope) = boot_limits() {
        plan = plan.with_boot_envelope(BootEnvelope {
            max_transport_bytes: envelope.max_transport_bytes,
            max_decoded_bytes: envelope.max_decoded_bytes,
            construction_costs: request_disclosure_audit::construction_costs(),
        });
    }

    Ok(plan)
}

/// The resolved pre-encoding cap on one adapter-built outbound body, in
/// the width the per-bidder sink's `CappedWriter` speaks.
///
/// The config field is a `u64` because the boot envelope declares it as
/// one; the sink takes a `usize` because that is a buffer length. On a
/// 32-bit wasm target a cap above `usize` saturates to the widest
/// allocation this isolate could make, which is the tightest honest
/// bound available and not a silent truncation — the same rule
/// `drive_fanout` applies to the encoded ceiling.
#[cfg(test)]
fn resolved_raw_body_cap(host: &HostConfig) -> usize {
    usize::try_from(host.resource_limits.max_outbound_raw_bytes).unwrap_or(usize::MAX)
}

/// The wire body a node-budget refusal carries.
///
/// A `&'static str`, so its length is a compile-time fact and cannot be
/// a function of the input it is refusing. The caller's own wrapper
/// supplies neither prefix nor newline for this one — both are part of
/// the pinned contract and are spelled here.
const NODE_BUDGET_BODY: &str = "Invalid request: too many JSON nodes";

/// The egress leg: the three auctioneer-owned write-deadline checkpoints,
/// the egress-class permit, and the response they produce.
///
/// Split out from the auction body because it is one cohesive
/// decision — acquire, serialize, bind — that reads the same whatever
/// produced `resp`, and because keeping it inline made the auction body
/// a mix of two unrelated concerns.
fn egress_leg(
    resp: &mut BidResponse,
    clock: &(dyn Clock + Send + Sync),
    egress_resource: Option<&ResponseEgressResource<RouteEgressPermit>>,
) -> Result<Response, HandlerError> {
    // THE ANCHOR. The write budget starts HERE, at egress entry, not at
    // request admission: the auction has resolved, the response exists,
    // and the budget this value carries bounds the leg that is about to
    // begin. The auction's own elapsed time is bounded by tmax and is
    // not charged a second time — a request that spent most of its tmax
    // would otherwise reach this line with almost no write allowance
    // left, and a slow enough one with none at all, so a completed
    // auction's answer would be shed as overload and even the refusal
    // would go out carrying the same expired deadline.
    //
    // Sampled before the first checkpoint reads it, so all three
    // checkpoints and the host's own fourth measure one allowance from
    // one instant.
    let deadline = EgressDeadline::from_boot_envelope(clock.now_monotonic());
    // CHECKPOINTS 1 and 2, as one decision: the queue wait races the
    // write deadline, and the granted permit is rechecked before a
    // single byte is serialized. A response that lost its budget in the
    // queue is never serialized at all, and a permit granted past the
    // budget is returned rather than spent producing bytes nobody will
    // read.
    //
    // The permit is acquired HERE rather than at admission because it
    // bounds the EGRESS class, and until the auction has resolved there
    // is nothing to egress. It is handed to the response completion
    // below, so it outlives this frame.
    let egress_permit = match egress_gate() {
        Some(gate) => Some(
            egress_deadline::acquire_within_deadline(&gate, deadline, || clock.now_monotonic())
                .map_err(HandlerError::from_egress_refusal)?,
        ),
        // No gate means boot has not finished installing one. The
        // auction already ran, so refusing now would discard completed
        // work for a condition that cannot recur once boot finishes;
        // the response goes out unbounded by the egress class and
        // bounded by the deadline alone.
        None => None,
    };

    let serialized =
        serialize_response_fitting_debug(resp).map_err(|e| HandlerError::Internal(e.message()))?;
    // CHECKPOINT 3: serialization itself takes time, so recheck before
    // the first byte. The buffer is passed BY VALUE and only comes back
    // when the budget survived, so an expired response has no bytes
    // left to send.
    let resp_bytes =
        egress_deadline::transmit_within_deadline(serialized, deadline, clock.now_monotonic())
            .map_err(HandlerError::from_egress_refusal)?;

    // Bind the permit to the RESPONSE, not to this frame. Releasing at
    // handler return would free the slot while the response is still
    // unwritten; the completion holds it until the host reports
    // terminal egress. A refused install drops the permit, which
    // releases it rather than leaking it.
    match (egress_resource, egress_permit) {
        (Some(resource), Some(permit)) => {
            // The result is not an error to propagate: both refusals
            // drop the permit and so release its slot, and both are
            // logged inside. A closed slot in particular is the normal
            // client-disconnected-mid-auction shape, not a defect.
            let _installed = install_route_permit(resource, RouteEgressPermit::Auction(permit));
        }
        // A permit with no slot to bind it to. On a SERVED request this
        // no longer happens: `configure_app` installs
        // [`crate::ingress::decide`], which hands every auction
        // admission a late-bound slot. What reaches this arm is a
        // handler called directly — every in-process test — where no
        // admission ran at all, and a request that arrived before boot
        // finished, which finds no gate and is refused rather than
        // admitted. Dropping the permit here releases its slot EARLY,
        // before the response is written, which over-admits by exactly
        // one. That is accepted rather than hidden, because the
        // alternative is holding a permit nothing will ever release.
        (None, Some(permit)) => {
            log::debug!(
                "no late-bound egress slot for this response; releasing the egress permit at \
                 handler return instead of at terminal egress"
            );
            drop(permit);
        }
        (_, None) => {}
    }

    let content_length = resp_bytes.len();
    response_builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .header(CONTENT_LENGTH, content_length.to_string())
        .body(Body::from_bytes(resp_bytes))
        // Bound HERE rather than by the caller: this is the only frame
        // that holds the egress-entry anchor, so handing the response
        // back unbound would mean handing the anchor back with it.
        .map(|response| bound(response, deadline))
        .map_err(|e| HandlerError::Internal(format!("response builder: {e}")))
}

fn handler_error_to_response(err: HandlerError) -> Result<Response, EdgeError> {
    let (status, content_type, body) = err.into_parts();
    response_builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static(content_type))
        .body(Body::from_bytes(body))
        .map_err(EdgeError::internal)
}

enum HandlerError {
    BadRequest(String),
    FanoutUnavailable,
    Internal(String),
    /// The response could not be sent within its write budget, or the
    /// egress class was full. A `&'static str` body rather than a
    /// `String`: this is the shed path under load, and it must not
    /// allocate to describe itself.
    Overloaded(&'static str),
    /// The ingress read of the request body ran out of time.
    ///
    /// Carries no message. The body is the single pinned string the
    /// design names for this surface, so there is nothing per-request
    /// to describe and nothing from the host's error text to carry.
    RequestTimeout,
    ServiceUnavailable(String),
}

impl HandlerError {
    /// The dynamic message this error RETAINS, for the guards that
    /// assert the construction-site bound.
    ///
    /// Separate from `into_parts` on purpose. The renderer bounds the
    /// wire for every variant, so a guard that reads only the rendered
    /// body passes whether or not the construction site bounded
    /// anything — which is a guard that cannot fail. The two claims are
    /// independent and are asserted independently: this one is "the
    /// `String` held across the return path is bounded", and the
    /// rendered-length one is "the wire is bounded whatever the site
    /// did".
    #[cfg(test)]
    fn retained_message(&self) -> Option<&str> {
        match self {
            Self::BadRequest(m) | Self::Internal(m) | Self::ServiceUnavailable(m) => {
                Some(m.as_str())
            }
            Self::FanoutUnavailable | Self::Overloaded(_) | Self::RequestTimeout => None,
        }
    }
    fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    /// A 400 whose text is derived from CALLER input, bounded to the
    /// terminal-response reservation.
    ///
    /// Terminal responses are funded for a 256-byte body. A serde
    /// diagnostic quotes the offending value, so a 200 KiB string in a
    /// numeric field renders a 204,837-byte diagnostic — held during
    /// egress, past handler return, with no auction reservation behind
    /// it. Bounding at the RENDER is the fix rather than buying wider
    /// egress capacity: the overage is unbounded in caller input, so
    /// any reservation that covered it would have to be sized for the
    /// whole body cap, and the bytes past the first few hundred are
    /// untrusted upstream text that carries no diagnostic value anyway.
    fn bad_request_bounded(prefix: &str, detail: &impl fmt::Display) -> Self {
        Self::BadRequest(bounded_message(prefix, detail))
    }
    /// A 500 whose text is derived from caller input.
    ///
    /// `InvalidImpFirstPartyData` is the live case: `split.rs` builds it
    /// from `serde_json::Error::to_string()` on the bidder-scoped
    /// request AFTER the FPD merge-patch, so a long string under
    /// `imp.ext.prebid.imp.<bidder>` reaches it. The inbound node budget
    /// does not bound that — one oversized string is a single node.
    fn internal_bounded(prefix: &str, detail: &impl fmt::Display) -> Self {
        Self::Internal(bounded_message(prefix, detail))
    }
    /// A 503 whose text is derived from caller- or store-influenced
    /// input — an account id the request named, or a disabled-account
    /// reason the config store supplied.
    fn service_unavailable_bounded(prefix: &str, detail: &impl fmt::Display) -> Self {
        Self::ServiceUnavailable(bounded_message(prefix, detail))
    }
    const fn fanout_unavailable() -> Self {
        Self::FanoutUnavailable
    }
    /// An invalid wire privacy signal is bad client input, so it maps to
    /// the same 400 shape every other malformed-request rejection uses.
    ///
    /// The match is exhaustive by value on purpose, mirroring
    /// `From<PrivacyError> for AuctionError`: a new privacy error has to
    /// state its own HTTP fate rather than inheriting the 400.
    fn from_privacy_error(err: auctioneer_privacy::PrivacyError) -> Self {
        match err {
            invalid @ (auctioneer_privacy::PrivacyError::InvalidRegsGdpr { .. }
            | auctioneer_privacy::PrivacyError::InvalidNoSale
            | auctioneer_privacy::PrivacyError::GppLimitExceeded) => {
                Self::bad_request_bounded("Invalid request: ", &invalid)
            }
        }
    }
    fn from_auction_error(err: AuctionError) -> Self {
        match err {
            // Caller-derived: an alias name or a privacy value the
            // request supplied reaches this text, so it is bounded to
            // the same reservation the parse diagnostics are.
            AuctionError::BadInput(m) => Self::bad_request_bounded("Invalid request: ", &m),
            // Caller-derived too, and the reviewer's point: this sits
            // beside `BadInput` and carries the same serde diagnostic
            // over the same untrusted input, so it takes the same bound.
            AuctionError::InvalidImpFirstPartyData(m) => {
                Self::internal_bounded("Invalid request: ", &m)
            }
            AuctionError::FanoutUnavailable => Self::FanoutUnavailable,
            AuctionError::Internal(m) => Self::internal_bounded("internal: ", &m),
            AuctionError::AccountDisabled(m) => {
                Self::service_unavailable_bounded("Invalid request: ", &m)
            }
            AuctionError::MalformedAcct(m) => Self::internal_bounded("Invalid request: ", &m),
            AuctionError::Resource(_resource) => Self::Overloaded("overloaded\n"),
        }
    }
    /// An egress refusal is isolate-capacity overload from the client's
    /// side: the auction completed and the answer could not be sent
    /// within its budget. It maps to the same 503 wire shape admission
    /// overload uses, because that is what it is — and deliberately does
    /// NOT leak which internal checkpoint fired, which is a server
    /// detail. The checkpoint goes to the log instead.
    fn from_egress_refusal(refusal: EgressRefusal) -> Self {
        log::warn!(
            "auction response shed at the {} egress checkpoint",
            refusal.checkpoint()
        );
        Self::Overloaded(refusal.body())
    }
    /// Classifies a failed ingress body read.
    ///
    /// Three outcomes, not two. An over-cap read is the client's 400; a
    /// read that ran out of TIME is the client's 408, which every
    /// adapter wrapper can raise as a typed `RequestTimeout` once the
    /// boot-owned ingress read deadline expires; everything else is
    /// auctioneer's 500.
    /// The SDK collector's typed inbound 413 keeps this endpoint's existing
    /// 400 size response; the host error renderer still forwards SDK 413s.
    ///
    /// Matched on the VARIANT rather than the status, for the reason
    /// the error renderer gives: a second variant that happened to map
    /// to 408 would be a compile-visible addition here instead of
    /// silently inheriting this row's answer.
    fn from_body_read_error(err: &EdgeError, cap_bytes: usize) -> Self {
        match err {
            EdgeError::RequestTimeout { .. } => Self::RequestTimeout,
            EdgeError::PayloadTooLarge {
                origin: RequestBodyOverflowOrigin::Inbound,
            } => Self::BadRequest(format!(
                "Invalid request: request size exceeded max size of {cap_bytes} bytes."
            )),
            EdgeError::BadGateway { .. }
            | EdgeError::BadRequest { .. }
            | EdgeError::ConfigOutOfDate { .. }
            | EdgeError::GatewayTimeout { .. }
            | EdgeError::Internal { .. }
            | EdgeError::MethodNotAllowed { .. }
            | EdgeError::NotFound { .. }
            | EdgeError::NotImplemented { .. }
            | EdgeError::PayloadTooLarge { .. }
            | EdgeError::RequestHeaderFieldsTooLarge { .. }
            | EdgeError::ResponseTooLarge { .. }
            | EdgeError::ServiceUnavailable { .. }
            | EdgeError::StoreExtraction { .. }
            | EdgeError::UriTooLong { .. }
            | EdgeError::Validation { .. }
            | _ => Self::internal_bounded("Invalid request: body read failed: ", &err),
        }
    }
    /// Render one dynamic diagnostic as a terminal body within
    /// [`MAX_ERROR_BODY_BYTES`], trailing newline included.
    ///
    /// Every variant that carries caller- or store-influenced text goes
    /// through here, so the bound is a property of the RENDERER rather
    /// than of each construction site. A variant built by `format!` at
    /// a site that forgot to bound itself is still bounded on the wire,
    /// which is the difference between a reservation and a convention:
    /// the 500 and 503 arms were previously unbounded for exactly that
    /// reason, and so was the 400 built directly from a
    /// `Content-Encoding` header value.
    ///
    /// The newline is inside the budget, not added after it. Counting
    /// it afterwards is what let a body sized exactly to the
    /// reservation render one byte over.
    fn bounded_body(message: &str) -> Vec<u8> {
        // One byte for the newline, plus room for the ellipsis a
        // truncation adds, so the FINAL bytes fit the reservation.
        let reserved = ELLIPSIS.len().saturating_add(1);
        let room = MAX_ERROR_BODY_BYTES.saturating_sub(reserved);
        let kept = prefix_within(message, room);
        let mut body = String::with_capacity(MAX_ERROR_BODY_BYTES);
        body.push_str(kept);
        if kept.len() < message.len() {
            body.push_str(ELLIPSIS);
        }
        body.push('\n');
        body.into_bytes()
    }

    fn into_parts(self) -> (StatusCode, &'static str, Vec<u8>) {
        match self {
            Self::BadRequest(m) => (
                StatusCode::BAD_REQUEST,
                "text/plain; charset=utf-8",
                Self::bounded_body(&m),
            ),
            Self::FanoutUnavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "text/plain; charset=utf-8",
                b"outbound http unavailable\n".to_vec(),
            ),
            Self::Internal(m) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "text/plain; charset=utf-8",
                Self::bounded_body(&m),
            ),
            Self::Overloaded(body) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "text/plain; charset=utf-8",
                body.as_bytes().to_vec(),
            ),
            // The pinned bytes the design names for an expired ingress
            // read, trailing newline included — the same string the
            // error renderer and the ingress drain already answer with,
            // so all three surfaces agree on the wire.
            Self::RequestTimeout => (
                StatusCode::REQUEST_TIMEOUT,
                "text/plain; charset=utf-8",
                REQUEST_TIMEOUT_BODY.as_bytes().to_vec(),
            ),
            Self::ServiceUnavailable(m) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "text/plain; charset=utf-8",
                Self::bounded_body(&m),
            ),
        }
    }
}

/// The WALL-clock half of the request clock.
///
/// `TestClockHandle` overrides wall time only. Monotonic time on the
/// auction path has its own source and its own override, so a test
/// that wants to drive the exchange's timing installs a
/// `TestMonotonicClockHandle` instead of swapping this one.
fn wall_clock_handle(ctx: &RequestContext) -> Arc<dyn Clock + Send + Sync> {
    if let Some(handle) = ctx.extensions().get::<TestClockHandle>() {
        return Arc::clone(&handle.0);
    }
    host_registry::clock_handle()
}

/// The MONOTONIC half of the request clock: `EdgeZero`'s own
/// per-request clock, or a test-installed source.
fn monotonic_clock_handle(ctx: &RequestContext) -> MonotonicClock {
    if let Some(handle) = ctx.extensions().get::<TestMonotonicClockHandle>() {
        return handle.0.clone();
    }
    ctx.monotonic_clock()
}

fn rng_handle(ctx: &RequestContext) -> Arc<dyn Rng + Send + Sync> {
    if let Some(handle) = ctx.extensions().get::<TestRngHandle>() {
        return Arc::clone(&handle.0);
    }
    host_registry::rng_handle()
}

fn timer_handle(ctx: &RequestContext) -> Arc<dyn Timer + Send + Sync> {
    if let Some(handle) = ctx.extensions().get::<TestTimerHandle>() {
        return Arc::clone(&handle.0);
    }
    host_registry::timer_handle()
}

fn strategy_handle(ctx: &RequestContext) -> Arc<dyn FanoutStrategy + Send + Sync> {
    if let Some(handle) = ctx.extensions().get::<TestStrategyHandle>() {
        return Arc::clone(&handle.0);
    }
    host_registry::strategy_handle()
}

/// Pick the drive this request's fanout runs on.
///
/// `host` supplies the resolved request-body and JSON-node caps. Response
/// ceilings stay at the funded envelope: per-call caps carry the scoped
/// resolution and may be higher than the host defaults.
fn fanout_from_context(
    ctx: &RequestContext,
    clock: Arc<dyn Clock + Send + Sync>,
    host: &HostConfig,
) -> Option<Arc<dyn BidderFanout + Send + Sync>> {
    // 1. Test-side seam: any handler-path test injects a
    //    `TestFanoutHandle` via extensions.
    if let Some(handle) = ctx.extensions().get::<TestFanoutHandle>() {
        return Some(Arc::clone(&handle.0));
    }
    // 2. Production seam: `ctx.http_client()` returns
    //    `Option<HttpClient>` — owned, cloned from extensions.
    //
    //    The timer and strategy are resolved HERE rather than by the
    //    caller: both registry accessors panic when no adapter
    //    installed them, and only this branch actually needs them. An
    //    injected fanout above supplies its own driving, so a test that
    //    uses one must not be forced to install host primitives.
    let client = ctx.http_client()?;
    if boot_limits().is_none() && !host_registry::fanout_primitives_installed() {
        return None;
    }
    let timer = timer_handle(ctx);
    let strategy = strategy_handle(ctx);
    if let (Some(limits), Some(gate)) = (boot_limits(), stream_gate()) {
        return Some(Arc::new(EdgeZeroFanout::with_wave_scheduler(
            client,
            clock,
            timer,
            host_registry::decode_cooperator_handle(),
            strategy,
            ResolvedIsolateLimits {
                max_transport_bytes: limits.max_transport_bytes,
                max_decoded_bytes: limits.max_decoded_bytes,
                ..resolve::limits_for_request(limits, host)
            },
            gate,
        )));
    }
    // Direct in-process development calls can precede envelope installation.
    // Served requests still fail closed in ingress when boot gates are absent.
    Some(Arc::new(EdgeZeroFanout::new(
        client, clock, timer, strategy,
    )))
}

/// Resolve runtime controls and compiled activation/ancestry without mutating the registry.
/// Overlay-only names remain visible to the existing host/request alias veto path.
pub(crate) fn disabled_from_overlay(
    app: &AuctioneerAppConfig,
    registry: &BidderRegistry,
) -> BTreeSet<BidderName> {
    let mut disabled: BTreeSet<_> = app
        .bidders
        .keys()
        .map(|name| BidderName::new(name.as_str()))
        .filter(|name| !resolve::bidder_enabled(app, name))
        .collect();
    disabled.extend(
        registry
            .infos()
            .filter(|info| {
                !resolve::effective_bidder_enabled(app, info, |name| {
                    registry.get(name).map(|(ancestor, _)| ancestor)
                })
            })
            .map(|info| BidderName::new(info.name.as_str())),
    );
    disabled
}

/// Feed the rolling privacy-deployment counters that
/// `/_auctioneer/health/gvl` reports.
///
/// Every prebid-owned `GvlVersionUnavailable` warning is one lookup
/// that found no usable vendor list in the baked archive, and every
/// prebid-owned `GclStale` warning is one request that could not
/// authenticate its CMP because the bundled Global CMP List is past its expiry. The two
/// have separate counters: they are separate deployment faults with
/// separate fixes, and one number could not tell them apart. The
/// response is the observation point rather than the resolver because
/// the resolver lives in `auctioneer-privacy`, which owns no host
/// primitives and must stay side-effect free.
fn record_gvl_unavailable(
    resp: &BidResponse,
    now: OffsetDateTime,
    unavailable_counter: &health_gvl::GvlUnavailableCounter,
    gcl_stale_counter: &health_gvl::GvlUnavailableCounter,
) {
    let unavailable = u64::from(WarningCode::GvlVersionUnavailable.as_u32());
    let gcl_stale = u64::from(WarningCode::GclStale.as_u32());
    let Some(warnings) = resp
        .ext
        .as_ref()
        .and_then(|ext| ext.get("warnings"))
        .and_then(|warnings| warnings.get("prebid"))
        .and_then(serde_json::Value::as_array)
    else {
        return;
    };
    // PrivacyContext warnings are serialized under `prebid`; warnings
    // owned by other bidder groups must not feed deployment counters.
    for entry in warnings {
        match entry.get("code").and_then(serde_json::Value::as_u64) {
            Some(code) if code == unavailable => unavailable_counter.record(now),
            Some(code) if code == gcl_stale => gcl_stale_counter.record(now),
            Some(_) | None => {}
        }
    }
}

pub(crate) fn host_config_handle() -> &'static HostConfig {
    static HOST: OnceLock<HostConfig> = OnceLock::new();
    HOST.get_or_init(|| auctioneer_config_data::host_default().clone())
}

/// Drop aliases outside the bidder identity byte bounds or whose base
/// this build does not carry, matching the auction's routing eligibility.
///
/// `host_aliases` is `HostConfig.aliases` AFTER `resolve_host` has
/// layered the runtime `[aliases]` overlay on the baked map, so it can
/// still name a base the registry has never heard of — the overlay is
/// operator input and is not build-validated the way the baked TOML is.
/// Advertising such an alias from `/info/bidders` would list a name that
/// `/info/bidders/<name>` then 404s, so the same registry check the
/// auction's `resolve_aliases` applies runs here and the two surfaces
/// agree on one catalog.
pub(crate) fn effective_host_aliases(
    host_aliases: &BTreeMap<String, String>,
    registry: &BidderInfoIndex,
) -> BTreeMap<String, String> {
    host_aliases
        .iter()
        .filter(|&(alias, base)| {
            alias.len() <= MAX_BIDDER_NAME_BYTES
                && base.len() <= MAX_BIDDER_NAME_BYTES
                && registry.get(base).is_some()
        })
        .map(|(alias, base)| (alias.clone(), base.clone()))
        .collect()
}

fn headers_from_http(headers: &HeaderMap) -> RequestHeaders<'_> {
    RequestHeaders {
        referer: headers.get("referer").and_then(|v| v.to_str().ok()),
        user_agent: headers.get("user-agent").and_then(|v| v.to_str().ok()),
        dnt: headers.get("dnt").and_then(|v| v.to_str().ok()),
        sec_gpc: headers.get("sec-gpc").and_then(|v| v.to_str().ok()),
        true_client_ip: headers.get("true-client-ip").and_then(|v| v.to_str().ok()),
        x_forwarded_for: headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()),
        x_real_ip: headers.get("x-real-ip").and_then(|v| v.to_str().ok()),
        remote_addr: None,
        content_encoding: headers
            .get("content-encoding")
            .and_then(|v| v.to_str().ok()),
    }
}

#[cfg(test)]
mod tests {
    use super::BidderInfoIndex;
    mod original_extension {
        use super::super::{
            BidRequest, BidderFanout, BidderInfo, BidderRegistry, Body, DisclosureAudit,
            StatusCode, TestFanoutHandle, auction_inner,
        };
        use auctioneer_bidder::{
            Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
            DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
        };
        use auctioneer_exchange::{
            AuctionDeadline, BidderCall, BidderCallResult, FanoutInternalError,
        };
        use auctioneer_runtime::{Clock as _, FakeClock, SeededRng, bounded_json::JsonFieldShape};
        use edgezero_core::{context::RequestContext, params::PathParams};
        use futures::executor::block_on;
        use serde_json::{Value, json};
        use std::sync::{Arc, Mutex};
        use time::OffsetDateTime;

        struct ShapeBidder {
            observed: Arc<Mutex<Vec<JsonFieldShape>>>,
        }

        impl Bidder for ShapeBidder {
            fn debug_body_schema(&self) -> DebugBodySchema {
                DebugBodySchema::Unsupported
            }

            fn make_requests(
                &self,
                _request: &BidRequest,
                info: &BidderRequestInfo<'_>,
                _sink: &mut OutboundRequestSink<'_>,
            ) -> Result<(), Vec<BidderError>> {
                let Ok(mut observed) = self.observed.lock() else {
                    return Err(vec![BidderError::bad_input("shape observer poisoned")]);
                };
                observed.push(info.original_request_ext_shape);
                Ok(())
            }

            fn make_bids(
                &self,
                _request: &BidRequest,
                _sent: &OutboundRequestMeta,
                _response: &BidderHttpResponse,
            ) -> Result<BidderResponse, Vec<BidderError>> {
                Err(vec![BidderError::bad_server_response("no calls expected")])
            }
        }

        struct NoCalls;

        #[async_trait::async_trait(?Send)]
        impl BidderFanout for NoCalls {
            async fn send_all(
                &self,
                _calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                panic!("the shape spy must not dispatch");
            }
        }

        #[test]
        fn raw_handler_supplies_each_original_extension_shape_to_the_bidder() {
            for (extension, expected) in [
                (None, JsonFieldShape::Absent),
                (Some(Value::Null), JsonFieldShape::Null),
                (Some(json!({})), JsonFieldShape::Object),
            ] {
                let observed = Arc::new(Mutex::new(Vec::new()));
                let mut registry = BidderRegistry::new();
                registry
                    .register(
                        Box::leak(Box::new(BidderInfo {
                            name: "fake_shape_fixture".into(),
                            endpoint: "https://bidder.example/auction".into(),
                            uses_https: true,
                            ..BidderInfo::default()
                        })),
                        Box::new(ShapeBidder {
                            observed: Arc::clone(&observed),
                        }),
                    )
                    .expect("unique test bidder");
                let mut input = json!({
                    "id": "wire-shape", "site": {"id": "publisher"}, "tmax": 1000_i64,
                    "imp": [{"id": "1", "banner": {"w": 300_i32, "h": 250_i32},
                        "ext": {"prebid": {"bidder": {"fake_shape_fixture": {}}}}}],
                });
                if let Some(value) = extension {
                    input["ext"] = value;
                }
                let mut request = http::Request::builder()
                    .method("POST")
                    .uri("/openrtb2/auction")
                    .body(Body::from_bytes(serde_json::to_vec(&input).unwrap()))
                    .unwrap();
                request
                    .extensions_mut()
                    .insert(TestFanoutHandle(Arc::new(NoCalls)));
                let context = RequestContext::new(request, PathParams::default());
                let clock = Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
                let config = toml::from_str("[gdpr]\nenabled=false\n").unwrap();
                let response = block_on(auction_inner(
                    context,
                    config,
                    Arc::<FakeClock>::clone(&clock),
                    Arc::new(SeededRng::new(42)),
                    clock.now_monotonic(),
                    None,
                    None,
                    &mut DisclosureAudit::default(),
                    &mut false,
                    &registry,
                ))
                .unwrap_or_else(|_error| panic!("valid wire request must reach the shape bidder"));
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(observed.lock().unwrap().as_slice(), [expected]);
            }
        }
    }

    #[test]
    fn normalized_construction_limits_include_funded_header_and_node_additions() {
        let limits = super::normalized_construction_limits(512, 1024, 4096)
            .unwrap_or_else(|_error| panic!("funded additions fit"));
        assert_eq!(limits.max_nodes, 542);
        assert_eq!(
            limits.max_serialized_bytes,
            1024_u64
                .checked_mul(3)
                .unwrap()
                .checked_add(4096_u64.checked_mul(8).unwrap())
                .unwrap()
                .checked_add(189)
                .unwrap()
                .checked_add(512_u64.checked_mul(23).unwrap())
                .unwrap()
        );
        let installed = super::boot_gates().map_or_else(
            super::request_disclosure_audit::construction_costs,
            |gates| gates.construction_costs,
        );
        assert_eq!(
            limits.reusable_scratch_bytes,
            installed.reusable_scratch_bytes
        );
        assert_eq!(
            limits.retained_per_scope_bytes,
            installed.retained_per_scope_bytes
        );
    }

    #[test]
    fn normalized_construction_overflow_is_a_static_resource_refusal() {
        for (nodes, inbound, headers) in [
            (u64::MAX, 0, 0),
            (0, u64::MAX, 0),
            (0, 0, u64::MAX),
            (u64::MAX.checked_div(23).unwrap() + 1, 0, 0),
            (u64::MAX.checked_div(23).unwrap(), 0, 0),
        ] {
            let error = super::normalized_construction_limits(nodes, inbound, headers)
                .expect_err("overflow refuses rather than widening the allowance");
            assert!(matches!(
                error,
                super::HandlerError::Overloaded("overloaded\n")
            ));
        }
    }

    fn privacy_warning_counts(ext: Option<serde_json::Value>) -> (u32, u32) {
        use super::health_gvl::GvlUnavailableCounter;
        use auctioneer_openrtb::types::bid_response::BidResponse;
        use time::OffsetDateTime;

        let unavailable = GvlUnavailableCounter::new();
        let stale = GvlUnavailableCounter::new();
        let now = OffsetDateTime::UNIX_EPOCH;
        let response = BidResponse {
            ext,
            ..BidResponse::default()
        };
        super::record_gvl_unavailable(&response, now, &unavailable, &stale);
        (unavailable.count_last_hour(now), stale.count_last_hour(now))
    }

    #[test]
    fn privacy_observer_ignores_bidder_owned_warning_codes() {
        use auctioneer_openrtb::WarningCode;

        assert_eq!(
            privacy_warning_counts(Some(serde_json::json!({
                "warnings": {
                    "fake_pp": [
                        {"code": WarningCode::GvlVersionUnavailable.as_u32()},
                        {"code": WarningCode::GclStale.as_u32()},
                    ],
                    "fake_alias": [{"code": WarningCode::OutboundBudgetClamp.as_u32()}],
                },
            }))),
            (0, 0),
        );
    }

    #[test]
    fn privacy_observer_counts_only_prebid_privacy_warnings() {
        use auctioneer_openrtb::WarningCode;

        assert_eq!(
            privacy_warning_counts(Some(serde_json::json!({
                "warnings": {
                    "prebid": [
                        {"code": WarningCode::GvlVersionUnavailable.as_u32()},
                        {"code": WarningCode::GvlVersionUnavailable.as_u32()},
                        {"code": WarningCode::GclStale.as_u32()},
                        {"code": WarningCode::OutboundBudgetClamp.as_u32()},
                        {"code": WarningCode::InvalidPrivacyConsent.as_u32()},
                    ],
                    "fake_pp": [
                        {"code": WarningCode::GvlVersionUnavailable.as_u32()},
                        {"code": WarningCode::GclStale.as_u32()},
                    ],
                },
            }))),
            (2, 1),
        );
    }

    #[test]
    fn privacy_observer_ignores_absent_or_malformed_warning_shapes() {
        for ext in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!({"warnings": []})),
            Some(serde_json::json!({"warnings": {"prebid": {"code": 10_019_u32}}})),
            Some(serde_json::json!({"warnings": {"prebid": [
                null, {}, {"code": null}, {"code": "10019"}, {"code": -1_i32},
            ]}})),
        ] {
            assert_eq!(privacy_warning_counts(ext), (0, 0));
        }
    }

    #[test]
    fn catalog_aliases_require_bounded_utf8_identities_and_a_registered_base() {
        use auctioneer_config_data::types::{BidderInfo, MAX_BIDDER_NAME_BYTES};
        use std::collections::BTreeMap;

        let long_base = "b".repeat(MAX_BIDDER_NAME_BYTES + 1);
        let base = Box::leak(Box::new(BidderInfo {
            name: "fake_base".to_owned(),
            ..BidderInfo::default()
        }));
        let oversized = Box::leak(Box::new(BidderInfo {
            name: long_base.clone(),
            ..BidderInfo::default()
        }));
        let boundary = format!("fake_{}", "a".repeat(MAX_BIDDER_NAME_BYTES - 5));
        let aliases = BTreeMap::from([
            (boundary.clone(), "fake_base".to_owned()),
            (
                "a".repeat(MAX_BIDDER_NAME_BYTES + 1),
                "fake_base".to_owned(),
            ),
            ("\u{e9}".repeat(33), "fake_base".to_owned()),
            ("fake_long_base".to_owned(), long_base),
            ("fake_unknown_base".to_owned(), "fake_missing".to_owned()),
        ]);
        assert_eq!(
            super::effective_host_aliases(
                &aliases,
                &BidderInfoIndex::new([&*base, &*oversized].into_iter())
            ),
            BTreeMap::from([(boundary, "fake_base".to_owned())]),
        );
    }

    use super::{
        AuctionError, AuctioneerAppConfig, BidderName, ELLIPSIS, EdgeError, HandlerError,
        HostConfig, MAX_AUCTION_BODY, MAX_ERROR_BODY_BYTES, REQUEST_TIMEOUT_BODY, StatusCode,
        disabled_from_overlay, resolved_raw_body_cap,
    };

    /// The sink's raw-body cap is the RESOLVED config's figure, not the
    /// inbound-body constant it used to be.
    ///
    /// The constant and the shipped baked default are the same 256 KiB,
    /// so a fixture at the default could not tell the two sources
    /// apart: this one lowers the config field and asserts the cap
    /// follows it. Without the wiring the writer would keep enforcing
    /// 256 KiB however far an operator lowered the knob, which is a
    /// config surface that silently does nothing.
    #[test]
    fn the_sinks_raw_body_cap_follows_the_resolved_config() {
        let mut host = HostConfig::default();
        host.resource_limits.max_outbound_raw_bytes = 0x0000_4000;
        assert_eq!(
            resolved_raw_body_cap(&host),
            0x0000_4000,
            "a lowered `max_outbound_raw_bytes` must reach the sink's writer",
        );
        assert_ne!(
            resolved_raw_body_cap(&host),
            MAX_AUCTION_BODY,
            "a cap still equal to the inbound-body constant would mean the config \
             field never reached the writer",
        );
    }

    /// The finding's own fixture: a 200 KiB string in the numeric
    /// `device.dnt`. The serde diagnostic quotes the offending value,
    /// so the UNBOUNDED render was 204,837 bytes of caller text against
    /// a 256-byte terminal reservation.
    ///
    /// Asserted against the reservation rather than a literal length,
    /// and the oversize input is built here rather than pinned, so the
    /// row keeps meaning the same thing if serde's wording changes.
    #[test]
    fn a_caller_sized_parse_diagnostic_renders_within_the_terminal_reservation() {
        let oversize = "A".repeat(200 * 1024);
        let tree = serde_json::json!({"id": "a", "imp": [], "device": {"dnt": oversize}});
        let err = serde_json::from_value::<super::BidRequest>(tree)
            .expect_err("dnt is numeric, so the typed conversion must fail");
        let raw = err.to_string();
        assert!(
            raw.len() > MAX_ERROR_BODY_BYTES,
            "the fixture must really produce an oversized diagnostic, else this row              asserts nothing: got {} bytes",
            raw.len(),
        );

        let (status, _ct, rendered) =
            HandlerError::bad_request_bounded("Invalid request: ", &err).into_parts();
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "it is still the client's 400"
        );
        assert!(
            rendered.len() <= MAX_ERROR_BODY_BYTES,
            "the rendered body must fit the terminal reservation, got {} bytes against {}",
            rendered.len(),
            MAX_ERROR_BODY_BYTES,
        );
        let text = String::from_utf8(rendered).expect("the body stays valid UTF-8");
        assert!(
            text.starts_with("Invalid request: "),
            "the prefix survives truncation: {text}",
        );
        assert!(
            text.trim_end().ends_with(ELLIPSIS),
            "a truncated diagnostic says it was cut: {text}",
        );
        // The real property: what travels is bounded by the
        // reservation, not by how much the caller sent. A prefix of the
        // caller's bytes legitimately fits inside 256 bytes; three
        // orders of magnitude more of them do not.
        assert!(
            text.matches('A').count() < oversize.len(),
            "the caller's bytes are bounded by the reservation, not echoed wholesale",
        );
    }

    /// A short diagnostic is passed through untouched, so the bound
    /// costs nothing on the ordinary path.
    #[test]
    fn a_short_diagnostic_is_not_truncated() {
        let (_ok_status, _ok_ct, rendered) =
            HandlerError::bad_request_bounded("Invalid request: ", &"imp is required").into_parts();
        assert_eq!(
            String::from_utf8_lossy(&rendered),
            "Invalid request: imp is required\n",
            "a diagnostic inside the reservation is rendered verbatim",
        );
    }

    /// Truncation lands on a char boundary, so multi-byte caller text
    /// cannot panic the render.
    #[test]
    fn truncation_respects_multi_byte_boundaries() {
        // Three-byte characters, so a byte-sliced cut would split one.
        let wide = "\u{3042}".repeat(4096);
        let (_ok_status, _ok_ct, rendered) =
            HandlerError::bad_request_bounded("Invalid request: ", &wide).into_parts();
        assert!(rendered.len() <= MAX_ERROR_BODY_BYTES);
        String::from_utf8(rendered).expect("a boundary-respecting cut stays valid UTF-8");
    }

    /// A long PREFIX is bounded by the reservation too, not just the
    /// detail.
    ///
    /// The length assertion is the whole test. Its previous revision
    /// asserted `starts_with(&long_prefix)` — "the prefix is never
    /// truncated" — and said in its own doc that the result was "still
    /// within it" while never checking that. It was not: the prefix was
    /// pushed unconditionally, so a 256-byte prefix rendered 260 bytes
    /// against the 256-byte reservation. A guard that pins the
    /// behaviour cannot report that the behaviour is over the bound.
    ///
    /// Every shipped prefix is a short literal, so this is a guard on
    /// the helper's contract rather than a live overflow — which is
    /// exactly why it has to assert the contract.
    #[test]
    fn a_prefix_at_the_reservation_is_bounded_by_it() {
        for prefix_len in [
            MAX_ERROR_BODY_BYTES - 3,
            MAX_ERROR_BODY_BYTES,
            MAX_ERROR_BODY_BYTES * 4,
        ] {
            let long_prefix = "P".repeat(prefix_len);
            let err = HandlerError::bad_request_bounded(&long_prefix, &"detail that cannot fit");
            // The CONSTRUCTION bound, asserted before the renderer gets
            // a chance to bound it: this is the claim the helper makes.
            let retained = err.retained_message().expect("a 400 carries a message");
            assert!(
                retained.len() <= MAX_ERROR_BODY_BYTES,
                "a {prefix_len}-byte prefix built a {}-byte message against the \
                 {MAX_ERROR_BODY_BYTES}-byte terminal reservation",
                retained.len(),
            );
            let (_ok_status, _ok_ct, rendered) = err.into_parts();
            assert!(
                rendered.len() <= MAX_ERROR_BODY_BYTES,
                "a {prefix_len}-byte prefix rendered {} bytes against the \
                 {MAX_ERROR_BODY_BYTES}-byte terminal reservation",
                rendered.len(),
            );
            let text = String::from_utf8(rendered).expect("still valid UTF-8");
            assert!(
                text.trim_end().ends_with(ELLIPSIS),
                "and the cut is reported: {text}",
            );
        }
    }

    /// A multi-byte PREFIX is cut on a character boundary, the same way
    /// a multi-byte detail is.
    ///
    /// The prefix goes through `prefix_within` rather than being pushed
    /// whole, so this is the property that cut has to keep.
    #[test]
    fn a_long_multi_byte_prefix_stays_valid_utf8() {
        let wide_prefix = "\u{3042}".repeat(MAX_ERROR_BODY_BYTES);
        let (_ok_status, _ok_ct, rendered) =
            HandlerError::bad_request_bounded(&wide_prefix, &"detail").into_parts();
        assert!(rendered.len() <= MAX_ERROR_BODY_BYTES);
        String::from_utf8(rendered).expect("a boundary-respecting prefix cut stays valid UTF-8");
    }

    /// The 500 and 503 shapes carry the same bound as the 400.
    ///
    /// `InvalidImpFirstPartyData` is the live case the reviewer named:
    /// `split.rs` builds it from a serde diagnostic over the
    /// bidder-scoped request AFTER the FPD merge-patch, so a long
    /// string under `imp.ext.prebid.imp.<bidder>` reaches it. It
    /// rendered a ~200 KB 500 body against the same 256-byte
    /// reservation the 400 is held to.
    #[test]
    fn every_dynamic_error_variant_is_bounded() {
        let oversize = "A".repeat(200 * 1024);
        let cases = [
            (
                "InvalidImpFirstPartyData",
                AuctionError::InvalidImpFirstPartyData(oversize.clone()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                "Internal",
                AuctionError::Internal(oversize.clone()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                "MalformedAcct",
                AuctionError::MalformedAcct(oversize.clone()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                "AccountDisabled",
                AuctionError::AccountDisabled(oversize.clone()),
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                "BadInput",
                AuctionError::BadInput(oversize.clone()),
                StatusCode::BAD_REQUEST,
            ),
        ];
        for (name, err, expected_status) in cases {
            let mapped = HandlerError::from_auction_error(err);
            // The construction bound first. Reading only the rendered
            // body would pass on an unbounded construction site,
            // because the renderer bounds the wire regardless — and
            // every one of these arms WAS such a site.
            let retained = mapped
                .retained_message()
                .expect("every arm here carries a message");
            assert!(
                retained.len() <= MAX_ERROR_BODY_BYTES,
                "{name} retained a {}-byte message against the \
                 {MAX_ERROR_BODY_BYTES}-byte terminal reservation, so the \
                 oversized diagnostic lives across the whole return path",
                retained.len(),
            );
            let (status, _ct, rendered) = mapped.into_parts();
            assert_eq!(status, expected_status, "{name} keeps its wire status");
            assert!(
                rendered.len() <= MAX_ERROR_BODY_BYTES,
                "{name} rendered {} bytes against the {MAX_ERROR_BODY_BYTES}-byte \
                 terminal reservation",
                rendered.len(),
            );
            let text = String::from_utf8(rendered).expect("valid UTF-8");
            assert!(
                text.trim_end().ends_with(ELLIPSIS),
                "{name} reports the cut: {text}",
            );
        }
    }

    /// The RENDERER bounds the wire whatever a construction site did.
    ///
    /// Built by `format!` straight into the variant, bypassing every
    /// bounded constructor — which is how the 500 and 503 arms came to
    /// be unbounded in the first place. A future site that forgets the
    /// constructor is still bounded on the wire.
    #[test]
    fn the_renderer_bounds_a_variant_built_without_a_constructor() {
        let oversize = "A".repeat(200 * 1024);
        for (name, err) in [
            ("Internal", HandlerError::Internal(oversize.clone())),
            ("BadRequest", HandlerError::BadRequest(oversize.clone())),
            (
                "ServiceUnavailable",
                HandlerError::ServiceUnavailable(oversize.clone()),
            ),
        ] {
            let (_status, _ct, rendered) = err.into_parts();
            assert!(
                rendered.len() <= MAX_ERROR_BODY_BYTES,
                "{name} built directly rendered {} bytes, so the bound depends on \
                 every construction site remembering it",
                rendered.len(),
            );
        }
    }

    /// An ingress read that ran out of time is the CLIENT's 408 with the
    /// pinned body, not auctioneer's 500 carrying the host's error text.
    ///
    /// All three arms are asserted together, because the bug this pins
    /// was one arm swallowing another: every non-400 read failure became
    /// `Internal`, so a typed timeout the adapters can all raise left as
    /// a 500 whose body echoed upstream text.
    #[test]
    fn a_body_read_timeout_is_a_bounded_408_and_an_oversize_read_is_still_400() {
        use edgezero_core::body::Body;
        use edgezero_core::error::RequestBodyOverflowOrigin;
        use futures::executor::block_on;

        let (status, content_type, body) =
            HandlerError::from_body_read_error(&EdgeError::request_timeout("read expired"), 1024)
                .into_parts();
        assert_eq!(status, StatusCode::REQUEST_TIMEOUT, "a read expiry is 408");
        assert_eq!(content_type, "text/plain; charset=utf-8");
        assert_eq!(
            body,
            REQUEST_TIMEOUT_BODY.as_bytes(),
            "the 408 body is the pinned string, byte for byte",
        );

        let over_error = block_on(
            Body::from_bytes(vec![b'x'; MAX_AUCTION_BODY + 1]).into_bytes_bounded(MAX_AUCTION_BODY),
        )
        .expect_err("the real SDK collector rejects cap plus one");
        assert!(matches!(
            over_error,
            EdgeError::PayloadTooLarge {
                origin: RequestBodyOverflowOrigin::Inbound,
            }
        ));
        assert_eq!(over_error.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let (over_status, over_ct, over_body) =
            HandlerError::from_body_read_error(&over_error, MAX_AUCTION_BODY).into_parts();
        assert_eq!(
            over_status,
            StatusCode::BAD_REQUEST,
            "an over-cap read stays the client's 400",
        );
        assert!(
            String::from_utf8_lossy(&over_body).contains("max size"),
            "and still names the cap it exceeded",
        );
        assert_eq!(over_ct, "text/plain; charset=utf-8");
        assert_eq!(
            over_body, b"Invalid request: request size exceeded max size of 262144 bytes.\n",
            "the SDK's typed 413 must retain the auction endpoint's exact 400 body",
        );
        assert!(over_body.len() <= MAX_ERROR_BODY_BYTES);

        let (other_status, _other_ct, other_body) =
            HandlerError::from_body_read_error(&EdgeError::bad_gateway("upstream broke"), 1024)
                .into_parts();
        assert_eq!(
            other_status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "anything else is still auctioneer's 500",
        );
        assert!(
            !String::from_utf8_lossy(&other_body).is_empty(),
            "which describes itself",
        );
    }

    #[test]
    fn streamed_sdk_body_overflow_keeps_the_configured_auction_cap() {
        use edgezero_core::body::Body;
        use edgezero_core::error::RequestBodyOverflowOrigin;
        use futures::{executor::block_on, stream};

        let cap = 1024;
        let chunk = Body::from_bytes(vec![b'x'; cap + 1]).into_bytes().unwrap();
        let error = block_on(Body::from_stream(stream::iter([Ok(chunk)])).into_bytes_bounded(cap))
            .expect_err("a real streaming read exceeds the configured cap");
        assert!(matches!(
            error,
            EdgeError::PayloadTooLarge {
                origin: RequestBodyOverflowOrigin::Inbound,
            }
        ));
        let (status, content_type, body) =
            HandlerError::from_body_read_error(&error, cap).into_parts();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(content_type, "text/plain; charset=utf-8");
        assert_eq!(
            body,
            b"Invalid request: request size exceeded max size of 1024 bytes.\n",
        );
        assert!(body.len() <= MAX_ERROR_BODY_BYTES);
    }

    #[test]
    fn unrelated_body_read_errors_are_not_inbound_size_failures() {
        use edgezero_core::error::RequestBodyOverflowOrigin;

        for error in [
            EdgeError::bad_request("unrelated body source failure"),
            EdgeError::payload_too_large(RequestBodyOverflowOrigin::OutboundUpload),
        ] {
            let (status, content_type, body) =
                HandlerError::from_body_read_error(&error, 1024).into_parts();
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(content_type, "text/plain; charset=utf-8");
            assert!(body.starts_with(b"Invalid request: body read failed: "));
            assert!(!String::from_utf8_lossy(&body).contains("max size"));
            assert!(body.len() <= MAX_ERROR_BODY_BYTES);
        }
    }

    #[test]
    fn disabled_from_overlay_distinguishes_explicit_false_from_true_and_absent() {
        use crate::bidder_fixture::registry;

        let app: AuctioneerAppConfig = toml::from_str(
            "[bidders.fake_aaa]\nenabled = false\n\n\
             [bidders.fake_bbb]\nenabled = true\n\n\
             [bidders.fake_pp]\ntimeout_ms = 300\n",
        )
        .unwrap();
        let disabled = disabled_from_overlay(&app, registry());
        assert!(
            disabled.contains(&BidderName::new("fake_aaa")),
            "an explicit enabled = false must disable",
        );
        assert!(
            !disabled.contains(&BidderName::new("fake_bbb")),
            "an explicit enabled = true must not disable",
        );
        // `fake_pp` has an overlay ENTRY (it sets timeout_ms) but omits
        // `enabled` — that must default to enabled, same as a bidder
        // with no overlay entry at all.
        assert!(
            !disabled.contains(&BidderName::new("fake_pp")),
            "an overlay entry that omits `enabled` must default to enabled",
        );
        assert!(
            !disabled.contains(&BidderName::new("fake_unlisted")),
            "a bidder absent from the overlay entirely must default to enabled",
        );
    }

    #[test]
    fn disabled_set_tracks_compiled_defaults_and_base_vetoes_without_registry_rebuilds() {
        use crate::bidder_fixture::fake_bidder::FakeBidder;
        use auctioneer_bidder::BidderRegistry;
        use auctioneer_config_data::types::BidderInfo;

        let mut registry = BidderRegistry::new();
        for (name, parent) in [
            ("fake_controlled", None),
            ("fake_alias", Some("fake_controlled")),
        ] {
            registry
                .register(
                    Box::leak(Box::new(BidderInfo {
                        name: name.into(),
                        alias_of: parent.map(str::to_owned),
                        default_enabled: Some(false),
                        ..BidderInfo::default()
                    })),
                    Box::<FakeBidder>::default(),
                )
                .unwrap();
        }
        for (overlay, base_off, alias_off) in [
            ("", true, true),
            ("[bidders.fake_alias]\nenabled = true\n", true, true),
            (
                "[bidders.fake_controlled]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
                false,
                false,
            ),
            (
                "[bidders.fake_controlled]\nenabled = false\n[bidders.fake_alias]\nenabled = true\n",
                true,
                true,
            ),
            (
                "[bidders.fake_controlled]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
                false,
                false,
            ),
        ] {
            let app: AuctioneerAppConfig = toml::from_str(overlay).unwrap();
            let disabled = disabled_from_overlay(&app, &registry);
            assert_eq!(disabled.contains("fake_controlled"), base_off);
            assert_eq!(disabled.contains("fake_alias"), alias_off);
            assert_eq!(registry.infos().len(), 2);
        }
    }

    #[test]
    fn handler_maps_malformed_acct_to_500() {
        let err = HandlerError::from_auction_error(AuctionError::MalformedAcct("acct-9".into()));
        let (status, _content_type, _body) = err.into_parts();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn handler_maps_privacy_errors_to_fixed_400() {
        for (privacy, message) in [
            (
                auctioneer_privacy::PrivacyError::InvalidNoSale,
                "ext.prebid.nosale contains an invalid bidder exemption",
            ),
            (
                auctioneer_privacy::PrivacyError::GppLimitExceeded,
                "regs.gpp exceeds privacy parsing limits",
            ),
        ] {
            let expected = format!("Invalid request: {message}\n").into_bytes();
            let direct = HandlerError::from_privacy_error(privacy.clone()).into_parts();
            let from_exchange =
                HandlerError::from_auction_error(AuctionError::from(privacy)).into_parts();
            for (status, content_type, body) in [direct, from_exchange] {
                assert_eq!(status, StatusCode::BAD_REQUEST);
                assert_eq!(content_type, "text/plain; charset=utf-8");
                assert_eq!(body, expected);
            }
        }
    }

    #[test]
    fn handler_maps_account_disabled_to_503() {
        let err = HandlerError::from_auction_error(AuctionError::AccountDisabled("acct-42".into()));
        let (status, content_type, body) = err.into_parts();
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(content_type, "text/plain; charset=utf-8");
        assert_eq!(body, b"Invalid request: acct-42\n".to_vec());
    }

    #[test]
    fn handler_maps_malformed_acct_body_is_invalid_request() {
        let err = HandlerError::from_auction_error(AuctionError::MalformedAcct("acct-9".into()));
        let (status, content_type, body) = err.into_parts();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(content_type, "text/plain; charset=utf-8");
        assert_eq!(body, b"Invalid request: acct-9\n".to_vec());
    }
}

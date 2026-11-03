//! `run_auction` — the engine seam.
//!
//! Performs auction steps 6-11. Steps 1-5 (capture, body read, parse, host +
//! account resolution, `prepare`) and step 12 (serialize) are the
//! caller's responsibility.

mod resource;
mod schema;

use crate::construction_costs;
use crate::construction_diagnostics::construction_diagnostic_costs;
use crate::{
    account::UNRESOLVED_ACCOUNT_ID,
    alias::AliasMap,
    bookkeeping::BookkeepingBudget,
    budgets::resolve_budget,
    context::{AuctionContext, ExchangeFeatures, RequestConstructionLimits},
    debug::{
        DebugPermission, account_denied_warning, bidder_denied_warning,
        capture::{Capture, TentativeRequest},
    },
    error::AuctionError,
    fanout::{
        BidderCall, BidderCallResult, Compression, FanoutError, FanoutInternalError,
        OversizeRequestStage, TransportError, TransportErrorKind,
    },
    limits::{BidderCapDrop, OutboundCapDrop, apply_max_bidder_names},
    plan::AuctionPlan,
    privacy_projection::apply_legacy_projection,
    response::{
        BidderResultBundle, BrowserAuctionResult, ErrorEntry, NonBidRow, ResponseInput,
        build_response,
    },
    responsetime::ResponseTimeStat,
    scrub::{DisclosureAudit, ScrubDecisions, apply_classified, apply_classified_with_limits},
    split::{PerBidderRequest, group_by_bidder, materialize_per_bidder},
    targeting::{
        BidRounding, PriceGranularity, PriceGranularityScope, TargetingInput, Winner, build_keys,
        parse_price_granularity, pick_winner, price_bucket,
    },
    tmax::{TmaxResolution, resolve_tmax},
    validate_endpoint::ExchangeEndpointValidator,
};
use auctioneer_bidder::schema::{SchemaWorkBudget, native_schema_work_budget};
use auctioneer_bidder::{
    Bidder, BidderHttpRequest, BidderName, BidderRequestInfo, BidderResponse, CallBudget,
    ConstructionDiagnostics, DebugBodySchema, FinalizeError, OutboundBuildError,
    OutboundRequestMeta, OutboundRequestSink, SinkLimits, SinkOutcome, TransportFinalize, TypedBid,
    finalize_for_transport,
};
use auctioneer_config::types::{BidderInfo, Compression as CfgCompression, IpKeepBits, MediaType};
use auctioneer_openrtb::NormalizedBidRequest;
use auctioneer_openrtb::ext::bid::{ExtBid, ExtBidPrebid};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::types::bid_response::BidResponse;
use auctioneer_privacy::{
    AuctionPermissions, BidderDecision, PrivacyContext, ResolveOptions, ResolvedPrivacy,
    ccpa::NoSale, sources::LegacyProjection,
};
use auctioneer_runtime::bidder_credentials::BidderCredentials;
use auctioneer_runtime::json_preflight::MAX_CONSTRUCTION_DEPTH;
use auctioneer_runtime::{ChargeKind, ResourceError, Rng};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// Drive the auction end-to-end.
///
/// # Errors
/// - `AuctionError::BadInput` — malformed request: bubbled up from the
///   split, or from an out-of-domain `regs.gdpr` the privacy resolver
///   refuses.
/// - `AuctionError::InvalidImpFirstPartyData` — imp FPD merge produced
///   a changed impression identity or a shape that fails typed round-trip
///   or request validation.
/// - `AuctionError::FanoutUnavailable` — the fanout implementation has
///   no outbound capability on this target.
/// - `AuctionError::Internal` — response builder or clock conversion
///   failure.
#[expect(
    clippy::too_many_lines,
    reason = "auction driver walks the fixed step sequence; helpers are already extracted"
)]
pub async fn run_auction(
    req: &NormalizedBidRequest,
    ctx: &AuctionContext<'_>,
    plan: &AuctionPlan,
    audit: &mut DisclosureAudit,
) -> Result<BidResponse, AuctionError> {
    let underlying: &BidRequest = req;
    let debug_permission =
        DebugPermission::resolve(underlying, ctx.debug_requested, ctx.account.config);
    let max_calls =
        u32::try_from(ctx.host.resource_limits.max_outbound_requests).unwrap_or(u32::MAX);
    if let Some(limits) = ctx.construction_limits {
        limits.ensure(underlying)?;
    }

    // Privacy resolves FIRST: its decisions gate which bidders reach
    // fan-out at all, and its warnings must survive every later exit.
    let ResolvedPrivacy {
        context: mut privacy,
        projection,
    } = resolve_privacy(ctx, underlying, plan)?;

    // The bookkeeping maps outlive the tmax check on purpose. The
    // privacy live-filter writes into them BEFORE tmax is evaluated, and
    // the tmax short-circuit AUGMENTS them rather than building fresh
    // ones — so an already-expired budget still returns the privacy
    // bookkeeping, not just timeouts.
    let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
    let mut seatnonbid: BTreeMap<BidderName, Vec<NonBidRow>> = BTreeMap::new();
    let mut decisions = BTreeMap::new();

    // Alias resolution ran in the handler; the plan is immutable for the
    // auction's lifetime and `run_auction` never re-resolves. The
    // splitter and the call-list builder take `&AliasMap`, so this reads
    // through to the map rather than the bundle.
    let aliases = &plan.aliases.map;
    let mut grouped = group_by_bidder(
        underlying,
        aliases,
        ctx.bidders,
        &plan.disabled_bidders,
        &plan.aliases.dropped_alias_names,
    )?;
    let mut disabled_bidder_warnings = grouped.take_disabled_bidder_warnings();
    let return_all_bid_status = returnallbidstatus(underlying, ctx.features);
    let mut bookkeeping = BookkeepingBudget::new(
        ctx.host.resource_limits.max_outbound_raw_bytes,
        ctx.host.resource_limits.max_parsed_nodes,
    );
    bookkeeping.response(&underlying.id)?;
    for (bidder, indices) in grouped.bidder_imps() {
        bookkeeping.bidder(bidder.as_str())?;
        if return_all_bid_status {
            for &index in indices {
                let imp = underlying.imp.get(index).ok_or_else(|| {
                    AuctionError::Internal("grouped impression index out of bounds".into())
                })?;
                bookkeeping.nonbid(&imp.id)?;
            }
        }
    }
    for (bidder, _) in disabled_bidder_warnings
        .iter()
        .chain(&plan.aliases.disabled_warnings)
    {
        bookkeeping.bidder(bidder.as_str())?;
    }
    disabled_bidder_warnings.extend(plan.aliases.disabled_warnings.iter().cloned());

    if let Some(receipt) = ctx.response_reservation {
        receipt.ensure_capacity(
            ChargeKind::Response,
            resource::with_debug(bookkeeping.retained_peak()?, max_calls)?,
        )?;
    }

    // DECIDE on identities only. Even an expired auction retains each denial
    // and its original impression IDs, without inspecting bidder-local FPD.
    let mut live = Vec::new();
    for (bidder, indices) in grouped.bidder_imps() {
        let base = aliases.bases.get(bidder).unwrap_or(bidder);
        let decision = cached_privacy_decision(&mut decisions, bidder, || {
            privacy.bidder_decision(base.as_str(), bidder.as_str())
        });
        for &index in indices {
            let imp = underlying.imp.get(index).ok_or_else(|| {
                AuctionError::Internal("grouped impression index out of bounds".into())
            })?;
            privacy_allows_bidder(
                &decision,
                bidder,
                &imp.id,
                &mut bundles,
                &mut seatnonbid,
                return_all_bid_status,
            );
        }
        if decision.fetch_denial.is_none() {
            live.push(bidder.clone());
        }
    }

    // The load-control sampler runs BEFORE the max-bidders cap so a
    // sampled-out bidder frees its slot for another eligible bidder
    // instead of consuming one that never reaches fan-out anyway.
    let sampled = apply_sampling(
        live,
        aliases,
        &plan.bidder_sample_rates,
        ctx.rng,
        &mut bundles,
    );

    // The max-bidders cap runs AFTER the privacy filter and the sampler
    // so a bidder either of those already dropped frees a slot instead
    // of consuming one.
    let cap = ctx.host.resource_limits.max_bidders_per_auction.max(1);
    let (survivors, bidder_cap_drops) = apply_max_bidder_names(sampled, cap);

    grouped.ensure_supplement_capacity(
        &survivors,
        ctx.host.resource_limits.max_parsed_nodes,
        usize::try_from(ctx.host.resource_limits.max_outbound_raw_bytes).unwrap_or(usize::MAX),
    )?;
    if let Some(receipt) = ctx.projection_reservation {
        let engine = construction_costs();
        let reusable_scratch_bytes =
            ctx.construction_limits
                .map_or(engine.reusable_scratch_bytes, |limits| {
                    limits
                        .reusable_scratch_bytes
                        .max(engine.reusable_scratch_bytes)
                });
        let retained_per_scope_bytes =
            ctx.construction_limits
                .map_or(engine.retained_per_scope_bytes, |limits| {
                    limits
                        .retained_per_scope_bytes
                        .max(engine.retained_per_scope_bytes)
                });
        let retained_growth = u64::try_from(survivors.len())
            .map_err(|_overflow| ResourceError::CapacityOverflow)?
            .checked_mul(retained_per_scope_bytes)
            .ok_or(ResourceError::CapacityOverflow)?;
        let charge = grouped
            .projection_charge(&survivors)?
            .checked_add(retained_growth)
            .ok_or(ResourceError::CapacityOverflow)?
            .checked_add(reusable_scratch_bytes)
            .and_then(|bytes| {
                bytes.checked_add(
                    construction_diagnostic_costs(u32::try_from(survivors.len()).ok()?)
                        .context_per_auction_bytes,
                )
            })
            .ok_or(ResourceError::CapacityOverflow)?;
        receipt.ensure_capacity(ChargeKind::Projection, charge)?;
    }

    // MATERIALIZE only post-cap survivors, then SCRUB recorded decisions so
    // FPD cannot restore identifiers or transaction IDs. Invalid survivor FPD
    // remains fatal even when the later tmax check short-circuits fanout.
    let mut capped = materialize_per_bidder(
        &grouped,
        &survivors,
        aliases,
        ctx.bidders,
        &plan.aliases.dropped_alias_names,
    )?;
    drop(survivors);
    drop(grouped);
    // Drained once after the pre-fanout warning-emitting steps have run.
    // Exactly one exit consumes it: the tmax short-circuit takes it by
    // value, or the normal path folds it into its own `ResponseInput`.
    let mut prebid_warnings = collect_prebid_warnings(req, &mut privacy, &mut bookkeeping)?;
    if debug_permission == DebugPermission::AccountDenied {
        bookkeeping.warning("debug disabled for account")?;
        prebid_warnings.push(account_denied_warning());
    }
    let bookkeeping_charge = bookkeeping.retained_peak()?;
    if let Some(receipt) = ctx.response_reservation {
        receipt.ensure_capacity(
            ChargeKind::Response,
            resource::with_debug(bookkeeping_charge, max_calls)?,
        )?;
    }

    let req_tmax_ms = underlying.tmax.and_then(|t| u64::try_from(t).ok());
    let resolved = resolve_tmax(req_tmax_ms, ctx.host, ctx.request_start)?;
    let bidder_visible_tmax_ms = resolved.bidder_visible_tmax_ms;
    let deadline = resolved.deadline;
    let mut debug = AuctionDebug {
        permission: debug_permission,
        capture: (debug_permission == DebugPermission::Allowed).then(|| Capture::new(max_calls)),
        deadline_at: deadline.deadline_at,
    };
    let now = ctx.clock.now_monotonic();
    let remaining = deadline.deadline_at.saturating_duration_since(now);
    let auction_timestamp_ms = auction_timestamp(ctx);
    let scoped_limits = ctx
        .construction_limits
        .map(|limits| {
            limits.with_supplement(
                ctx.host.resource_limits.max_parsed_nodes,
                ctx.host.resource_limits.max_outbound_raw_bytes,
            )
        })
        .transpose()?;
    // Invalid survivor FPD has already been decoded/validated. Preserve that
    // error precedence, but do not scrub or construct calls on a spent budget.
    if remaining == Duration::ZERO
        || !scrub_bidder_requests(
            &mut capped,
            &decisions,
            ctx.account.config.ip_keep_bits(),
            audit,
            scoped_limits,
            ctx.clock,
            deadline.deadline_at,
        )?
    {
        return short_circuit_tmax_expired(
            underlying,
            &capped,
            &disabled_bidder_warnings,
            &bidder_cap_drops,
            bundles,
            BTreeMap::new(),
            seatnonbid,
            bidder_visible_tmax_ms,
            auction_timestamp_ms,
            ctx.features,
            prebid_warnings,
        )
        .map(|response| debug.attach(response, ctx));
    }
    drop(decisions);

    let gpc = underlying
        .regs
        .as_ref()
        .and_then(|r| r.ext.as_ref())
        .and_then(|e| e.gpc.as_deref());

    let mut make_requests_output = MakeRequestsOutput {
        bidder_pairs: Vec::new(),
        bidder_pre_errors: BTreeMap::new(),
        construction_diagnostics: BTreeMap::new(),
        timing: BTreeMap::new(),
        budget: CallBudget::new(
            u32::try_from(ctx.host.resource_limits.max_outbound_requests).unwrap_or(u32::MAX),
        ),
        outbound_drops: Vec::new(),
        schema_work: native_schema_work_budget(ctx.schema_scratch_bytes),
    };

    for pbr in &mut capped {
        if ctx.clock.now_monotonic() >= deadline.deadline_at {
            break;
        }
        run_make_requests(
            ctx,
            underlying,
            pbr,
            gpc,
            &resolved,
            projection,
            &mut make_requests_output,
        )
        .await;
    }
    let MakeRequestsOutput {
        bidder_pairs,
        bidder_pre_errors,
        construction_diagnostics,
        mut timing,
        outbound_drops,
        ..
    } = make_requests_output;

    // Fold once before the final checkpoints so expiry retains errors and cap
    // warnings produced by siblings that completed construction earlier.
    fold_prefanout_bookkeeping(
        bidder_pre_errors,
        disabled_bidder_warnings,
        bidder_cap_drops,
        outbound_drops,
        &mut bundles,
    );
    if ctx.clock.now_monotonic() >= deadline.deadline_at {
        drop(bidder_pairs);
        return short_circuit_tmax_expired(
            underlying,
            &capped,
            &[],
            &[],
            bundles,
            construction_diagnostics,
            seatnonbid,
            bidder_visible_tmax_ms,
            auction_timestamp_ms,
            ctx.features,
            prebid_warnings,
        )
        .map(|response| debug.attach(response, ctx));
    }

    let (calls, slots) = build_call_list(
        ctx,
        plan,
        bidder_pairs,
        gpc,
        &resolved,
        &mut bundles,
        &mut debug,
    )?;
    if ctx.clock.now_monotonic() >= deadline.deadline_at {
        // Dropping funded bodies/slots releases accepted call ownership; no
        // fanout receives a refreshed deadline or partially prepared batch.
        drop(calls);
        drop(slots);
        return short_circuit_tmax_expired(
            underlying,
            &capped,
            &[],
            &[],
            bundles,
            construction_diagnostics,
            seatnonbid,
            bidder_visible_tmax_ms,
            auction_timestamp_ms,
            ctx.features,
            prebid_warnings,
        )
        .map(|response| debug.attach(response, ctx));
    }
    if debug.permission == DebugPermission::Allowed {
        for slot in &slots {
            if slot.call_index.is_some()
                && !debug
                    .permission
                    .bidder_allowed(&slot.bidder, aliases, ctx.bidders)
            {
                let warnings = &mut bundles.entry(slot.bidder.clone()).or_default().warnings;
                if !warnings.iter().any(|warning| warning.code == 10003) {
                    warnings.push(bidder_denied_warning());
                }
            }
        }
    }
    let expected_results = calls.len();

    let results = if calls.is_empty() {
        Vec::new()
    } else {
        ctx.fanout
            .send_all(calls, deadline)
            .await
            .map_err(|err| match err {
                FanoutInternalError::NoOutboundCapability => AuctionError::FanoutUnavailable,
                FanoutInternalError::Internal(msg) => AuctionError::Internal(msg),
                // A positional breach carries the two numbers that prove
                // it, but the wire sees only the fixed category: the
                // counts are an operator-side diagnostic, and a response
                // that reported how many bidders auctioneer dispatched would
                // leak auction shape to the caller.
                FanoutInternalError::BatchContractViolation { .. } => {
                    AuctionError::Internal("outbound batch contract violation".to_owned())
                }
            })?
    };

    if results.len() != expected_results {
        return Err(AuctionError::Internal(
            "outbound batch contract violation".to_owned(),
        ));
    }
    let decoded_charge = if let Some(receipt) = ctx.response_reservation {
        let charge = resource::decoded(&results)?;
        let retained = bookkeeping_charge
            .checked_add(charge)
            .ok_or(auctioneer_runtime::ResourceError::CapacityOverflow)?;
        receipt.ensure_capacity(
            ChargeKind::Response,
            resource::with_debug(retained, max_calls)?,
        )?;
        charge
    } else {
        0
    };

    // The auction clock is the ONLY admissible evidence that the
    // auction deadline elapsed. A per-slot
    // `Timeout(TimeoutCause::AuctionDeadline)` is provenance — it names
    // which composed budget was smaller when the call was dispatched,
    // not that this auction's own budget ran out. A best-effort target
    // can report that cause for a timeout its platform enforced in an
    // earlier phase while the auction clock still has budget left, so
    // folding those slot causes in here would raise `tmax expired` on
    // an auction that never expired. The slot still grades as a bidder
    // timeout on its own: code 1 / statuscode 101, unchanged.
    let auction_deadline_elapsed = ctx.clock.now_monotonic() >= deadline.deadline_at;

    process_results(
        ctx,
        &capped,
        aliases,
        slots,
        results,
        &mut bundles,
        &mut seatnonbid,
        &mut timing,
        &mut debug,
    );

    if let Some(receipt) = ctx.response_reservation {
        receipt.ensure_capacity(
            ChargeKind::Response,
            resource::with_debug(
                resource::targeting(
                    bookkeeping_charge,
                    decoded_charge,
                    targeting_prefix(ctx, underlying),
                    &bundles,
                )?,
                max_calls,
            )?,
        )?;
    }
    apply_targeting(ctx, underlying, &mut bundles, &mut prebid_warnings);

    let response_input = ResponseInput {
        construction_diagnostics,
        bidder_visible_tmax_ms,
        auction_timestamp_ms,
        return_all_bid_status,
        seatnonbid,
        bundles,
        prebid_errors: if auction_deadline_elapsed {
            vec![tmax_expired_error()]
        } else {
            vec![]
        },
        prebid_warnings,
    };
    let resp = build_response(response_input)
        .map_err(|err| AuctionError::Internal(format!("build_response: {err}")))?;
    Ok(debug.attach(with_id(resp, underlying.id.clone()), ctx))
}

struct AuctionDebug {
    permission: DebugPermission,
    capture: Option<Capture>,
    deadline_at: auctioneer_runtime::MonotonicInstant,
}

impl AuctionDebug {
    fn prepare_request(
        &mut self,
        request: &BidderHttpRequest,
        schema: DebugBodySchema,
        ctx: &AuctionContext<'_>,
    ) -> Option<TentativeRequest> {
        self.capture.as_mut()?.prepare_request(
            request,
            schema,
            ctx.host.resource_limits.max_parsed_nodes,
            ctx.clock,
            self.deadline_at,
        )
    }

    fn attach(self, mut response: BidResponse, ctx: &AuctionContext<'_>) -> BidResponse {
        if let Some(value) = self
            .capture
            .and_then(|capture| capture.finish(ctx.clock, self.deadline_at))
            && let Some(ext) = response
                .ext
                .as_mut()
                .and_then(serde_json::Value::as_object_mut)
        {
            ext.insert("debug".into(), value);
        }
        response
    }
}

/// Validate exemptions before resolving all independent privacy policies.
/// NSP exempts only GDPR, never CCPA, LMT, COPPA, or activity controls.
///
/// The id comparison lives here rather than inside the resolver because
/// `auctioneer-privacy` cannot name `ResolvedAccount` without inverting the
/// dependency graph.
///
/// The gate never fires on [`UNRESOLVED_ACCOUNT_ID`]: that id is
/// synthesized when the request named no publisher, so matching it would
/// let one host line (`non_standard_publishers = ["unknown"]`) opt every
/// publisher-less request out of enforcement.
fn resolve_privacy<'a>(
    ctx: &AuctionContext<'_>,
    underlying: &'a BidRequest,
    plan: &AuctionPlan,
) -> Result<ResolvedPrivacy<'a>, AuctionError> {
    let exemptions = underlying
        .ext
        .as_ref()
        .and_then(|ext| ext.prebid.as_ref())
        .map_or(&[][..], |prebid| prebid.nosale.as_slice());
    let no_sale = NoSale::validate(exemptions, |name| {
        ctx.bidders.get(name).is_some() || {
            let identity = BidderName::new(name);
            plan.aliases.map.bases.contains_key(&identity)
                || plan.aliases.dropped_alias_names.contains(&identity)
        }
    })?;
    let is_nsp = ctx.account.id.as_str() != UNRESOLVED_ACCOUNT_ID
        && ctx
            .host
            .gdpr
            .non_standard_publishers
            .iter()
            .any(|listed| listed.as_str() == ctx.account.id.as_str());

    PrivacyContext::resolve(
        underlying,
        ctx.account.config,
        ctx.host,
        ctx.gvl_store,
        ctx.gcl,
        ctx.now,
        ResolveOptions {
            gdpr_exempt: is_nsp,
            no_sale,
        },
    )
    .map_err(AuctionError::from)
}

/// Borrow the key on cache hits so repeated impressions allocate no identity copies.
fn cached_privacy_decision<F>(
    decisions: &mut BTreeMap<BidderName, BidderDecision>,
    bidder: &BidderName,
    resolve: F,
) -> BidderDecision
where
    F: FnOnce() -> BidderDecision,
{
    if let Some(decision) = decisions.get(bidder) {
        return decision.clone();
    }
    let decision = resolve();
    decisions.insert(bidder.clone(), decision.clone());
    decision
}

/// Record one bidder warning and one exclusion per covered impression,
/// before the splitter applies that bidder's FPD or clones its request.
fn privacy_allows_bidder(
    decision: &BidderDecision,
    bidder: &BidderName,
    imp_id: &str,
    bundles: &mut BTreeMap<BidderName, BidderResultBundle>,
    seatnonbid: &mut BTreeMap<BidderName, Vec<NonBidRow>>,
    return_all_bid_status: bool,
) -> bool {
    if decision.fetch_denial.is_none() {
        return true;
    }
    bundles
        .entry(bidder.clone())
        .or_insert_with(|| BidderResultBundle {
            warnings: vec![ErrorEntry {
                code: 10017,
                message: format!("bidder blocked by privacy settings: {bidder}"),
            }],
            ..BidderResultBundle::default()
        });
    if return_all_bid_status {
        seatnonbid
            .entry(bidder.clone())
            .or_default()
            .push(NonBidRow {
                imp_id: imp_id.to_owned(),
                statuscode: 300,
            });
    }
    false
}

/// Scrub permitted bidders independently after all bidder-local input projection.
fn scrub_bidder_requests(
    per_bidder: &mut [PerBidderRequest],
    decisions: &BTreeMap<BidderName, BidderDecision>,
    keep_bits: IpKeepBits,
    audit: &mut DisclosureAudit,
    limits: Option<RequestConstructionLimits>,
    clock: &dyn auctioneer_runtime::Clock,
    deadline_at: auctioneer_runtime::MonotonicInstant,
) -> Result<bool, AuctionError> {
    for pbr in per_bidder {
        if clock.now_monotonic() >= deadline_at {
            return Ok(false);
        }
        if let Some(scope_limits) = limits
            && let Err(error) = scope_limits.ensure(&pbr.request)
        {
            audit.start_scope();
            audit.fail_scope();
            return Err(error);
        }
        let decision = decisions
            .get(&pbr.bidder)
            .ok_or_else(|| AuctionError::Internal("missing survivor privacy decision".into()))?;
        let (pass_id, pass_geo) = match decision.gdpr {
            AuctionPermissions::AllowAll => (true, true),
            AuctionPermissions::Partial {
                pass_id, pass_geo, ..
            } => (pass_id, pass_geo),
            AuctionPermissions::DenyBidRequest => (false, false),
        };
        let disclosure = ScrubDecisions {
            pass_id,
            pass_geo,
            transmit_ufpd: decision.transmit_ufpd,
            transmit_precise_geo: decision.transmit_precise_geo,
            transmit_tid: decision.transmit_tid,
            ccpa: decision.ccpa,
            lmt: decision.lmt,
            coppa: decision.coppa,
        };
        match limits {
            Some(scope_limits) => apply_classified_with_limits(
                &mut pbr.request,
                disclosure,
                keep_bits,
                audit,
                scope_limits.max_nodes,
                MAX_CONSTRUCTION_DEPTH,
            )?,
            None => apply_classified(&mut pbr.request, disclosure, keep_bits, audit)?,
        }
    }
    Ok(clock.now_monotonic() < deadline_at)
}

/// The `[bidders.<name>] sample_rate` load-control filter — the quiet
/// sibling of the `enabled = false` kill-switch. A bidder that fails the
/// draw is removed from the returned list with NO warning code and NO
/// `seatnonbid` row (the standard RTB load-control convention), but its
/// bundle entry is still created — `BidderResultBundle::default()`,
/// `dispatched` stays `false` — so the "no live bidders" branch in the
/// response builder is still reachable and `nbr = NoBidInvalidRequest`
/// still fires per spec Appendix E.5.
///
/// Rate resolution is observable name, then base bidder, then bypass —
/// the same precedence `resolve_bidder_timeout` uses for per-bidder
/// timeouts. Looking up only `bidder` would let a request-declared alias
/// dodge its base bidder's load-control setting; a bidder with no
/// override on EITHER name is not sampled at all.
fn apply_sampling(
    names: Vec<BidderName>,
    aliases: &AliasMap,
    overrides: &BTreeMap<BidderName, f64>,
    rng: &dyn Rng,
    bundles: &mut BTreeMap<BidderName, BidderResultBundle>,
) -> Vec<BidderName> {
    let mut kept = Vec::with_capacity(names.len());
    for bidder in names {
        let base = aliases.bases.get(&bidder).unwrap_or(&bidder);
        let rate = overrides
            .get(&bidder)
            .or_else(|| overrides.get(base))
            .copied();
        match rate {
            Some(r) if !should_keep(r, rng) => {
                bundles.entry(bidder).or_default();
            }
            _ => kept.push(bidder),
        }
    }
    kept
}

// The workspace `restriction` group denies `as_conversions` and
// `float_arithmetic`, and pedantic `cast_precision_loss` is promoted to
// an error under `-D warnings`. The [0, 1) sampler genuinely needs all
// three: `u64 -> f64` has no `From` impl (only `u32`/`i32`/`f32` do), so
// the mantissa cast must use `as`, and the scale-down is float
// division by construction. This is the one place the conversion
// is correct and unavoidable, so the `#[expect]` (not a blanket
// `#[allow]`) scopes to this single fn — `expect` also fails the build
// if a future refactor removes the need, keeping the escape hatch
// honest.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    reason = "u64->f64 [0,1) sampler: no From<u64> for f64; the \
              53-bit mantissa cast and scale are the correct, \
              bounded conversion (raw <= 2^53 - 1 is exact in f64)"
)]
fn should_keep(rate: f64, rng: &dyn Rng) -> bool {
    // Boundary short-circuits: rate == 1.0 keeps every bidder, rate ==
    // 0.0 drops every bidder. Neither branch consults the RNG so the
    // endpoints are deterministic. Config-side validation guarantees
    // rate is in [0.0, 1.0]. (`<`/`>=` float comparisons are fine —
    // `float_cmp` only fires on `==`/`!=`.)
    if rate >= 1.0_f64 {
        return true;
    }
    if rate <= 0.0_f64 {
        return false;
    }
    // Half-open [0, 1) conversion: shift the u64 into the 53-bit
    // mantissa so the largest value this can produce is strictly less
    // than 1.0. `raw` lands in `0..=2^53 - 1`, where every integer is
    // exactly representable in f64, so `raw as f64` loses nothing
    // despite the lint name; dividing by the power of two
    // `9_007_199_254_740_992.0` (`2^53`) only shifts the exponent and
    // is likewise exact, and the divisor needs no cast of its own.
    let raw = rng.next_u64() >> 11_u32;
    let u01 = raw as f64 / 9_007_199_254_740_992.0_f64;
    u01 < rate
}

fn fold_prefanout_bookkeeping(
    bidder_pre_errors: BTreeMap<BidderName, Vec<ErrorEntry>>,
    disabled_bidder_warnings: Vec<(BidderName, ErrorEntry)>,
    bidder_cap_drops: Vec<BidderCapDrop>,
    outbound_drops: Vec<OutboundCapDrop>,
    bundles: &mut BTreeMap<BidderName, BidderResultBundle>,
) {
    for (bidder, errs) in bidder_pre_errors {
        bundles.entry(bidder).or_default().errors.extend(errs);
    }
    for (bidder, warning) in disabled_bidder_warnings {
        bundles.entry(bidder).or_default().warnings.push(warning);
    }
    for drop_entry in bidder_cap_drops {
        bundles
            .entry(drop_entry.bidder)
            .or_default()
            .warnings
            .push(bidder_cap_drop_warning(drop_entry.cap));
    }
    for drop_entry in outbound_drops {
        bundles
            .entry(drop_entry.bidder)
            .or_default()
            .warnings
            .push(ErrorEntry {
                code: 10999,
                message: format!(
                    "{} outbound request attempt(s) rejected by max_outbound_requests",
                    drop_entry.dropped_count
                ),
            });
    }
}

fn collect_prebid_warnings(
    req: &NormalizedBidRequest,
    privacy: &mut PrivacyContext,
    bookkeeping: &mut BookkeepingBudget,
) -> Result<Vec<ErrorEntry>, AuctionError> {
    let mut out: Vec<ErrorEntry> = Vec::new();
    for warning in req.warnings() {
        bookkeeping.warning(&warning.message)?;
        out.push(ErrorEntry {
            code: warning.code.as_u32(),
            message: warning.message.clone(),
        });
    }
    for warning in privacy.take_warnings() {
        bookkeeping.warning(&warning.message)?;
        out.push(ErrorEntry {
            code: warning.code.as_u32(),
            message: warning.message,
        });
    }
    Ok(out)
}

/// The pre-fanout expiry exit.
///
/// It AUGMENTS the bookkeeping maps the caller already populated rather
/// than building fresh ones: a bidder the privacy filter denied is
/// already recorded there with its code-10017 warning and non-bid row,
/// and rebuilding would erase it. The maps arrive BY VALUE because
/// `ResponseInput` owns them — the arm returns, so the caller never
/// touches them afterwards.
#[expect(
    clippy::too_many_arguments,
    reason = "single-caller early return whose parameters are the `ResponseInput` \
              it builds; a params struct here would near-duplicate that type"
)]
fn short_circuit_tmax_expired(
    underlying: &BidRequest,
    capped: &[PerBidderRequest],
    disabled_bidder_warnings: &[(BidderName, ErrorEntry)],
    bidder_cap_drops: &[BidderCapDrop],
    mut bundles: BTreeMap<BidderName, BidderResultBundle>,
    construction_diagnostics: BTreeMap<BidderName, ConstructionDiagnostics>,
    mut seatnonbid: BTreeMap<BidderName, Vec<NonBidRow>>,
    bidder_visible_tmax_ms: u32,
    auction_timestamp_ms: u64,
    features: ExchangeFeatures,
    prebid_warnings: Vec<ErrorEntry>,
) -> Result<BidResponse, AuctionError> {
    let return_all_bid_status = returnallbidstatus(underlying, features);
    for pbr in capped {
        let bundle = bundles.entry(pbr.bidder.clone()).or_default();
        bundle.errors.push(ErrorEntry {
            code: 1,
            message: "timeout".into(),
        });
        bundle.dispatched = false;
        if return_all_bid_status {
            seatnonbid.insert(
                pbr.bidder.clone(),
                pbr.request
                    .imp
                    .iter()
                    .map(|imp| NonBidRow {
                        imp_id: imp.id.clone(),
                        statuscode: 101,
                    })
                    .collect(),
            );
        }
    }
    for (bidder, warning) in disabled_bidder_warnings {
        bundles
            .entry(bidder.clone())
            .or_default()
            .warnings
            .push(warning.clone());
    }
    // Cap-dropped bidders still need their `ext.warnings[<bidder>]`
    // slot on the tmax-expired short-circuit — same shape as the
    // normal path in `fold_prefanout_bookkeeping`. Skipping the
    // emission here would drop the cap signal whenever tmax happens
    // to expire before fanout starts.
    for drop_entry in bidder_cap_drops {
        bundles
            .entry(drop_entry.bidder.clone())
            .or_default()
            .warnings
            .push(bidder_cap_drop_warning(drop_entry.cap));
    }
    let input = ResponseInput {
        construction_diagnostics,
        bidder_visible_tmax_ms,
        auction_timestamp_ms,
        return_all_bid_status,
        seatnonbid,
        bundles,
        prebid_errors: vec![tmax_expired_error()],
        prebid_warnings,
    };
    let resp = build_response(input)
        .map_err(|err| AuctionError::Internal(format!("build_response: {err}")))?;
    Ok(with_id(resp, underlying.id.clone()))
}

/// The auction-wide `ext.errors.prebid` entry for an expired budget.
/// Shared so the pre-fanout short-circuit and the post-fanout check
/// cannot drift into two different wire shapes for the same event.
fn tmax_expired_error() -> ErrorEntry {
    ErrorEntry {
        code: 12,
        message: "tmax expired".into(),
    }
}

fn bidder_cap_drop_warning(cap: usize) -> ErrorEntry {
    ErrorEntry {
        code: 10999,
        message: format!("bidder dropped: max_bidders_per_auction ({cap}) exceeded"),
    }
}

fn auction_timestamp(ctx: &AuctionContext<'_>) -> u64 {
    let now = ctx.clock.now();
    let seconds = u64::try_from(now.unix_timestamp()).unwrap_or(0);
    let sub_second_millis = u64::from(now.millisecond());
    seconds
        .saturating_mul(1000)
        .saturating_add(sub_second_millis)
}

/// What the make-requests pass accumulates across the per-bidder loop:
/// the outbound requests that survived endpoint validation, the errors
/// raised before dispatch, and the per-bidder timing column. They are
/// filled together and read together, so they travel as one value
/// rather than three parallel `&mut` parameters.
struct MakeRequestsOutput {
    bidder_pairs: Vec<(BidderName, Vec<BidderHttpRequest>)>,
    bidder_pre_errors: BTreeMap<BidderName, Vec<ErrorEntry>>,
    construction_diagnostics: BTreeMap<BidderName, ConstructionDiagnostics>,
    timing: BTreeMap<BidderName, ResponseTimeStat>,
    budget: CallBudget,
    outbound_drops: Vec<OutboundCapDrop>,
    schema_work: SchemaWorkBudget,
}

fn bind_bidder_settings<'a>(
    ctx: &'a AuctionContext<'_>,
    pbr: &PerBidderRequest,
    info: &mut BidderRequestInfo<'a>,
) -> Result<(), auctioneer_bidder::BidderError> {
    let requested = ctx
        .bidder_settings
        .and_then(|source| source.settings(pbr.bidder.as_str()));
    let base = ctx
        .bidder_settings
        .and_then(|source| source.settings(pbr.base.as_str()));
    let implementation = ctx
        .bidder_bindings
        .binary_search_by_key(&pbr.base.as_str(), |binding| binding.bidder)
        .ok()
        .and_then(|index| ctx.bidder_bindings.get(index))
        .map(|binding| binding.implementation);
    info.set_settings(implementation, requested, base)
}

async fn run_make_requests(
    ctx: &AuctionContext<'_>,
    underlying: &BidRequest,
    pbr: &mut PerBidderRequest,
    gpc: Option<&str>,
    tmax: &TmaxResolution,
    projection: LegacyProjection<'_>,
    out: &mut MakeRequestsOutput,
) {
    let t_mr = ctx.clock.now_monotonic();
    let Some((info, adapter)) = ctx.bidders.get(pbr.base.as_str()) else {
        return;
    };
    let macros: Vec<&str> = info.endpoint_macros.iter().map(String::as_str).collect();
    let allow: Option<Vec<&str>> = info
        .host_allowlist
        .as_ref()
        .map(|hosts| hosts.iter().map(String::as_str).collect());
    let publisher_id = extract_publisher_id(underlying);
    let mut info_for_adapter = BidderRequestInfo::new(
        pbr.bidder.clone(),
        pbr.base.clone(),
        &ctx.account.id,
        publisher_id,
        &info.endpoint,
        &macros,
        allow.as_deref(),
        gpc,
        ctx.entry_point,
        &underlying.cur,
    );
    info_for_adapter.original_request_ext_shape = ctx.original_request_ext_shape;
    info_for_adapter.floor_currency = ctx.floor_currency;
    if let Err(error) = bind_bidder_settings(ctx, pbr, &mut info_for_adapter) {
        out.bidder_pre_errors
            .entry(pbr.bidder.clone())
            .or_default()
            .push(ErrorEntry {
                code: error.code,
                message: error.message,
            });
        return;
    }
    pbr.request.tmax = Some(i64::from(tmax.bidder_visible_tmax_ms));
    apply_legacy_projection(&mut pbr.request, projection, info.openrtb.gpp_supported);
    if !schema::admit(ctx, pbr, tmax, out) {
        return;
    }
    let credentials = match credentials_for(
        ctx,
        adapter,
        &pbr.request,
        &info_for_adapter,
        tmax,
        &out.budget,
    )
    .await
    {
        Ok(credentials) => credentials,
        Err(error) => {
            out.bidder_pre_errors
                .entry(pbr.bidder.clone())
                .or_default()
                .push(ErrorEntry {
                    code: error.code,
                    message: error.message,
                });
            return;
        }
    };
    info_for_adapter.credentials = credentials.as_ref();

    let validator = ExchangeEndpointValidator::new(info);
    let mut sink = OutboundRequestSink::new(
        SinkLimits::new(
            usize::try_from(ctx.host.resource_limits.max_outbound_raw_bytes).unwrap_or(usize::MAX),
            u16::try_from(pbr.request.imp.len()).unwrap_or(u16::MAX),
        ),
        &out.budget,
        &validator,
    );
    let outcome = match adapter.make_requests(&pbr.request, &info_for_adapter, &mut sink) {
        Ok(()) => sink.into_outcome(),
        Err(errs) => {
            let outcome = sink.into_outcome_discarding_accepted();
            for err in errs {
                out.bidder_pre_errors
                    .entry(pbr.bidder.clone())
                    .or_default()
                    .push(ErrorEntry {
                        code: err.code,
                        message: err.message,
                    });
            }
            outcome
        }
    };
    record_construction_outcome(
        &pbr.bidder,
        outcome,
        ctx.clock.now_monotonic().saturating_duration_since(t_mr),
        out,
    );
}

fn record_construction_outcome(
    bidder: &BidderName,
    outcome: SinkOutcome,
    elapsed: Duration,
    out: &mut MakeRequestsOutput,
) {
    if !outcome.diagnostics.is_empty() {
        out.construction_diagnostics
            .insert(bidder.clone(), outcome.diagnostics);
    }
    if outcome.rejected_call_count > 0 {
        out.outbound_drops.push(OutboundCapDrop {
            bidder: bidder.clone(),
            dropped_count: usize::try_from(outcome.rejected_call_count).unwrap_or(usize::MAX),
        });
    }
    let errors = out.bidder_pre_errors.entry(bidder.clone()).or_default();
    errors.extend(
        outcome
            .errors
            .iter()
            .filter(|error| !matches!(error, OutboundBuildError::CallBudgetExhausted { .. }))
            .map(|_| ErrorEntry {
                code: 5,
                message: "outbound request construction failed".into(),
            }),
    );
    if outcome.suppressed_error_count > 0 {
        errors.push(ErrorEntry {
            code: 5,
            message: "additional outbound request construction failures".into(),
        });
    }
    out.bidder_pairs.push((bidder.clone(), outcome.accepted));
    out.timing.entry(bidder.clone()).or_default().make_requests = elapsed;
}

async fn credentials_for(
    ctx: &AuctionContext<'_>,
    adapter: &dyn Bidder,
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    tmax: &TmaxResolution,
    budget: &CallBudget,
) -> Result<Option<BidderCredentials>, auctioneer_bidder::BidderError> {
    let unavailable =
        || auctioneer_bidder::BidderError::failed_to_request_bids("bidder credentials unavailable");
    let Some(reference) = adapter.credential_key(request, info)? else {
        return Ok(None);
    };
    if ctx.clock.now_monotonic() >= tmax.deadline.deadline_at
        || ctx.projection_reservation.is_none()
    {
        return Err(unavailable());
    }
    // Construction is sequential. Keep a real call permit across the read;
    // return it before the synchronous sink reserves its actual calls.
    let _permit = budget.try_reserve().ok_or_else(unavailable)?;
    let source = ctx.credential_source.ok_or_else(unavailable)?;
    let credentials = source
        .load(reference, tmax.deadline.deadline_at)
        .await
        .map_err(|_error| unavailable())?;
    if ctx.clock.now_monotonic() >= tmax.deadline.deadline_at {
        return Err(unavailable());
    }
    Ok(Some(credentials))
}

fn build_call_list(
    ctx: &AuctionContext<'_>,
    plan: &AuctionPlan,
    bidder_pairs: Vec<(BidderName, Vec<BidderHttpRequest>)>,
    gpc: Option<&str>,
    tmax: &TmaxResolution,
    bundles: &mut BTreeMap<BidderName, BidderResultBundle>,
    debug: &mut AuctionDebug,
) -> Result<(Vec<BidderCall>, Vec<PreparedSlot>), AuctionError> {
    let deadline_at = tmax.deadline.deadline_at;
    let aliases = &plan.aliases.map;
    let mut calls: Vec<BidderCall> = Vec::new();
    let mut slots = Vec::new();
    for (bidder_name, requests) in bidder_pairs {
        if ctx.clock.now_monotonic() >= deadline_at {
            break;
        }
        let Some((info, adapter)) = resolve_bidder_or_alias(ctx, aliases, &bidder_name) else {
            continue;
        };
        let capture_allowed = debug
            .permission
            .bidder_allowed(&bidder_name, aliases, ctx.bidders);
        let debug_schema = if capture_allowed {
            adapter.debug_body_schema()
        } else {
            DebugBodySchema::Unsupported
        };
        let mut budget_warnings = Vec::new();
        let budget = resolve_budget(
            &plan.budgets,
            &plan.aliases,
            &bidder_name,
            plan.boot_envelope,
            &mut budget_warnings,
        )?;
        if !budget_warnings.is_empty() {
            bundles
                .entry(bidder_name.clone())
                .or_default()
                .warnings
                .extend(budget_warnings);
        }
        // Feature `gzip_outbound = false` strips compression before
        // dispatch even when the bidder YAML says `endpoint_compression:
        // gzip`. Used as a kill-switch when a downstream proxy
        // handles gzip incorrectly.
        let endpoint_compression = if ctx.features.gzip_outbound {
            info.endpoint_compression.map(|c| match c {
                CfgCompression::Gzip => Compression::Gzip,
            })
        } else {
            None
        };
        // Per-bidder timeout resolution:
        //   1. `auctioneer.toml [bidders.<observable-name>].timeout_ms`
        //      (aliases may carry their own override for narrower
        //      per-alias tuning),
        //   2. `auctioneer.toml [bidders.<base-name>].timeout_ms` (falls
        //      back through the alias map — an alias inherits the
        //      base bidder's override, mirroring how
        //      `BidderInfo.bidder_timeout_ms` is resolved),
        //   3. baked `BidderInfo.bidder_timeout_ms`,
        //   4. auction-wide `bidder_visible_tmax_ms` pass-through.
        let timeout = Duration::from_millis(resolve_bidder_timeout(
            ctx.bidder_timeout_override,
            aliases,
            &bidder_name,
            info.bidder_timeout_ms,
            tmax.bidder_visible_tmax_ms,
        ));
        for outbound in requests {
            if ctx.clock.now_monotonic() >= deadline_at {
                // The caller folds auction-wide expiry, releases the partial
                // batch, and retains original participation in timeout rows.
                return Ok((calls, slots));
            }
            let tentative = if capture_allowed {
                debug.prepare_request(&outbound, debug_schema, ctx)
            } else {
                None
            };
            // Capture may spend the remaining budget before gzip allocates.
            if ctx.clock.now_monotonic() >= deadline_at {
                return Ok((calls, slots));
            }
            let (mut slot, finalized) = PreparedSlot::finalize(
                bidder_name.clone(),
                outbound,
                &TransportFinalize {
                    gpc,
                    compression: endpoint_compression,
                    max_encoded_bytes: ctx.host.resource_limits.max_outbound_request_bytes,
                },
                calls.len(),
                debug_schema,
            );
            if let Some(request) = finalized {
                if capture_allowed {
                    slot.capture_association = debug.capture.as_mut().and_then(|capture| {
                        capture.commit_request(
                            calls.len(),
                            bidder_name.as_str(),
                            tentative,
                            request.headers(),
                            ctx.clock,
                            deadline_at,
                        )
                    });
                }
                calls.push(BidderCall {
                    request,
                    timeout,
                    max_transport_bytes: budget.max_transport_bytes,
                    max_decoded_bytes: budget.max_decoded_bytes,
                });
            }
            slots.push(slot);
        }
    }
    Ok((calls, slots))
}

struct PreparedSlot {
    bidder: BidderName,
    sent: Arc<OutboundRequestMeta>,
    rejected: Option<BidderCallResult>,
    call_index: Option<usize>,
    capture_association: Option<usize>,
    debug_schema: DebugBodySchema,
}

impl PreparedSlot {
    fn finalize(
        bidder: BidderName,
        outbound: BidderHttpRequest,
        options: &TransportFinalize<'_>,
        call_index: usize,
        debug_schema: DebugBodySchema,
    ) -> (Self, Option<BidderHttpRequest>) {
        let (sent, result) = finalize_for_transport(outbound, options);
        let (finalized, rejected) = match result {
            Ok(request) => (Some(request), None),
            Err(error) => (
                None,
                Some(BidderCallResult {
                    result: Err(match error {
                        FinalizeError::OversizeEncoded { .. } => FanoutError::OversizeRequest {
                            stage: OversizeRequestStage::Encoded,
                        },
                        FinalizeError::ReserveOverflow => {
                            FanoutError::Other("finalize reserve overflow".into())
                        }
                    }),
                    elapsed: Duration::ZERO,
                }),
            ),
        };
        (
            Self {
                bidder,
                sent,
                rejected,
                call_index: finalized.as_ref().map(|_| call_index),
                capture_association: None,
                debug_schema,
            },
            finalized,
        )
    }
}

struct PreparedResult {
    sent: Arc<OutboundRequestMeta>,
    result: BidderCallResult,
    capture_association: Option<usize>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "the three `&mut` accumulators are also read by `apply_targeting` \
              and `ResponseInput` in the same scope, so grouping them here \
              would only move the unpacking to three other call sites"
)]
fn process_results(
    ctx: &AuctionContext<'_>,
    bidder_requests: &[PerBidderRequest],
    aliases: &AliasMap,
    slots: Vec<PreparedSlot>,
    results: Vec<BidderCallResult>,
    bundles: &mut BTreeMap<BidderName, BidderResultBundle>,
    seatnonbid: &mut BTreeMap<BidderName, Vec<NonBidRow>>,
    timing: &mut BTreeMap<BidderName, ResponseTimeStat>,
    debug: &mut AuctionDebug,
) {
    let mut per_bidder_results: BTreeMap<BidderName, Vec<PreparedResult>> = BTreeMap::new();
    let mut remaining_results = results.into_iter();
    for PreparedSlot {
        bidder,
        sent,
        rejected,
        call_index,
        capture_association,
        debug_schema,
    } in slots
    {
        if call_index.is_some() {
            bundles.entry(bidder.clone()).or_default().dispatched = true;
        }
        let Some(mut call_result) = rejected.or_else(|| remaining_results.next()) else {
            continue;
        };
        if let Ok(response) = call_result.result {
            call_result.result =
                Ok(response.within_node_budget(ctx.host.resource_limits.max_parsed_nodes));
        }
        if let Some(capture) = debug.capture.as_mut() {
            capture.complete(
                capture_association,
                &call_result,
                debug_schema,
                ctx.clock,
                debug.deadline_at,
            );
        }
        per_bidder_results
            .entry(bidder)
            .or_default()
            .push(PreparedResult {
                sent,
                result: call_result,
                capture_association,
            });
    }
    for (bidder_name, results_for_bidder) in per_bidder_results {
        let Some(bidder_request) = bidder_requests.iter().find(|pbr| pbr.bidder == bidder_name)
        else {
            continue;
        };
        let max_elapsed = results_for_bidder
            .iter()
            .map(|row| row.result.elapsed)
            .max()
            .unwrap_or(Duration::ZERO);
        let bundle = bundles.entry(bidder_name.clone()).or_default();

        let stat = timing.entry(bidder_name.clone()).or_default();
        stat.max_call_elapsed = max_elapsed;
        let t_mb_start = ctx.clock.now_monotonic();

        for row in &results_for_bidder {
            process_one_result(
                ctx,
                &bidder_request.request,
                aliases,
                &bidder_name,
                row,
                bundle,
                seatnonbid,
                debug,
            );
        }
        stat.make_bids = ctx
            .clock
            .now_monotonic()
            .saturating_duration_since(t_mb_start);
        bundle.timing = *stat;
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "per-result view of `process_results`' own parameters; a second \
              params struct would duplicate that function's signature"
)]
fn process_one_result(
    ctx: &AuctionContext<'_>,
    sanitized_request: &BidRequest,
    aliases: &AliasMap,
    bidder_name: &BidderName,
    row: &PreparedResult,
    bundle: &mut BidderResultBundle,
    seatnonbid: &mut BTreeMap<BidderName, Vec<NonBidRow>>,
    debug: &mut AuctionDebug,
) {
    let http_request = &row.sent;
    match &row.result.result {
        Ok(http_response) => {
            let base_name = aliases
                .bases
                .get(bidder_name)
                .cloned()
                .unwrap_or_else(|| bidder_name.clone());
            let Some((_info, adapter)) = ctx.bidders.get(base_name.as_str()) else {
                return;
            };
            match adapter.make_bids(sanitized_request, http_request, http_response) {
                Ok(BidderResponse {
                    mut bids,
                    currency,
                    errors,
                    mut auction_configs,
                }) => {
                    if let Some(capture) = debug.capture.as_mut() {
                        capture.adapter_errors(row.capture_association, &errors);
                    }
                    bundle
                        .errors
                        .extend(errors.into_iter().map(|error| ErrorEntry {
                            code: error.code,
                            message: error.message,
                        }));
                    if retain_auction_configs(sanitized_request, http_request, &mut auction_configs)
                    {
                        bundle.errors.push(ErrorEntry {
                            code: 4,
                            message: "browser configuration contains an impression not requested by this call; uncorrelated configurations dropped".into(),
                        });
                    }
                    if !auction_configs.is_empty() {
                        if let Some(browser) = bundle.browser_auctions.as_mut() {
                            browser.configs.extend(auction_configs);
                        } else {
                            bundle.browser_auctions = Some(BrowserAuctionResult {
                                adapter: base_name,
                                configs: auction_configs,
                            });
                        }
                    }
                    // Non-USD response bids are dropped: response `cur`
                    // is hardcoded to "USD" in the response builder,
                    // so admitting foreign-currency bids would emit
                    // wrong-labelled prices. Request-floor conversion does not
                    // provide a response-price conversion path.
                    if currency != "USD" && !currency.is_empty() {
                        bundle.warnings.push(ErrorEntry {
                            code: 10004,
                            message: "non-USD currency; bids dropped".into(),
                        });
                    } else {
                        let mut rejected = false;
                        // Call metadata may narrow participation, never enlarge it.
                        // Empty metadata falls back to the retained bidder-scoped request.
                        bids.retain(|bid| {
                            let imp_id = bid.bid.impid.as_str();
                            let in_scope = sanitized_request.imp.iter().any(|imp| imp.id == imp_id)
                                && (http_request.imp_id_count() == 0
                                    || http_request.covered_imp_ids().any(|id| id == imp_id));
                            rejected |= !in_scope;
                            in_scope
                        });
                        bundle.bids.extend(bids);
                        if rejected {
                            bundle.errors.push(ErrorEntry {
                                code: 4,
                                message: "bid response contains an impression not requested by this call; uncorrelated bids dropped".into(),
                            });
                        }
                    }
                }
                Err(errs) => {
                    if let Some(capture) = debug.capture.as_mut() {
                        capture.adapter_errors(row.capture_association, &errs);
                    }
                    for err in errs {
                        bundle.errors.push(ErrorEntry {
                            code: err.code,
                            message: err.message,
                        });
                    }
                }
            }
        }
        Err(fanout_err) => {
            let (err_code, sn_code) = fanout_error_codes(fanout_err);
            bundle.errors.push(ErrorEntry {
                code: err_code,
                // Fixed category only. The error's `Debug` form may carry
                // an adapter-supplied payload; the wire never does.
                message: fanout_err.public_message().to_owned(),
            });
            if returnallbidstatus(sanitized_request, ctx.features) {
                let rows = http_request.covered_imp_ids().map(|id| NonBidRow {
                    imp_id: id.to_owned(),
                    statuscode: sn_code,
                });
                seatnonbid
                    .entry(bidder_name.clone())
                    .or_default()
                    .extend(rows);
            }
        }
    }
}

fn retain_auction_configs(
    request: &BidRequest,
    sent: &OutboundRequestMeta,
    configs: &mut Vec<auctioneer_bidder::BrowserAuctionConfig>,
) -> bool {
    let before = configs.len();
    configs.retain(|record| {
        request.imp.iter().any(|imp| imp.id == record.imp_id)
            && sent.covered_imp_ids().any(|id| id == record.imp_id)
    });
    configs.len() != before
}

/// The wire pair for one per-bidder failure: `ext.errors` code, then
/// `seatnonbid` statuscode.
///
/// The oversize arm ignores its origin on purpose. The origin is auctioneer's
/// internal diagnostic fidelity — which ceiling a bidder breached — and
/// the wire has no field for it: every size refusal is code 4 with
/// statuscode 100, exactly as before the origin existed. Discriminating
/// here would invent a wire shape no consumer expects. Outbound request
/// size refusals are separate: code 5 with statuscode 100.
#[must_use]
pub fn fanout_error_codes(err: &FanoutError) -> (u32, u32) {
    match err {
        FanoutError::Timeout(_) => (1, 101),
        FanoutError::Unreachable
        | FanoutError::Transport(TransportError {
            kind: TransportErrorKind::Unreachable,
        }) => (999, 103),
        FanoutError::Transport(TransportError {
            kind: TransportErrorKind::Initial,
        })
        | FanoutError::BadServerResponse
        | FanoutError::OversizeResponse(_)
        | FanoutError::UndecodableResponse(_)
        | FanoutError::Preflight => (4, 100),
        FanoutError::OversizeRequest { .. } => (5, 100),
        FanoutError::Other(_) => (999, 100),
    }
}

fn apply_targeting(
    ctx: &AuctionContext<'_>,
    underlying: &BidRequest,
    bundles: &mut BTreeMap<BidderName, BidderResultBundle>,
    prebid_warnings: &mut Vec<ErrorEntry>,
) {
    let targeting_cfg = underlying
        .ext
        .as_ref()
        .and_then(|e| e.prebid.as_ref())
        .and_then(|p| p.targeting.as_ref());
    let granularity = targeting_cfg
        .and_then(|t| t.pricegranularity.as_ref())
        .map_or_else(
            || PriceGranularity::from_account(ctx.account.config),
            |value| parse_price_granularity(value, PriceGranularityScope::Global, prebid_warnings),
        );
    let rounding = BidRounding::from_account(ctx.account.config);
    let include_winners = targeting_cfg.and_then(|t| t.includewinners).unwrap_or(true);
    let include_bidder_keys = targeting_cfg
        .and_then(|t| t.includebidderkeys)
        .unwrap_or(true);
    let include_format = targeting_cfg.and_then(|t| t.includeformat).unwrap_or(false);
    let prefer_deals = targeting_cfg.and_then(|t| t.preferdeals).unwrap_or(false);
    let always_include_deals = targeting_cfg
        .and_then(|t| t.alwaysincludedeals)
        .unwrap_or(false);
    let media_type_granularity: BTreeMap<&str, PriceGranularity> = targeting_cfg
        .and_then(|t| t.other.get("mediatypepricegranularity"))
        .and_then(serde_json::Value::as_object)
        .map(|m| {
            let mut out: BTreeMap<&str, PriceGranularity> = BTreeMap::new();
            for (key, scope) in [
                ("banner", PriceGranularityScope::Banner),
                ("video", PriceGranularityScope::Video),
                ("native", PriceGranularityScope::Native),
                ("audio", PriceGranularityScope::Audio),
            ] {
                if let Some(value) = m.get(key) {
                    out.insert(key, parse_price_granularity(value, scope, prebid_warnings));
                }
            }
            out
        })
        .unwrap_or_default();
    // Prefix resolution: request-level > account-level >
    // default `"hb"`. `truncate_target_attribute` comes from the
    // account when set; `build_keys` falls back to `MAX_KEY_LENGTH`
    // (20) when the caller passes `None`.
    let prefix = targeting_prefix(ctx, underlying);
    let truncate_target_attribute = ctx.account.config.truncate_target_attribute;
    let env = resolve_env(underlying);

    let cfg = TargetingConfig {
        prefix,
        truncate_target_attribute,
        granularity,
        media_type_granularity,
        rounding,
        include_winners,
        include_bidder_keys,
        include_format,
        always_include_deals,
        env,
    };

    // Winners are resolved for EVERY impression first, off borrowed
    // bundles, and only then is anything mutated.
    //
    // This used to clone `bundle.bids` for every bidder so the winner
    // scan could read them while the originals stayed live for the
    // mutation pass — a deep copy of every `TypedBid`, including the
    // arbitrary nested `ext` JSON a bidder may return, retained
    // alongside the originals.
    //
    // What the copy costs is not what a PEAK-memory guard can see, and
    // that is recorded here so a later attempt does not repeat the
    // measurement: targeting runs after the parse and the response
    // assembly, so the copy allocates in a valley, and restoring it
    // moved the auction's peak by exactly zero bytes. A
    // cumulative-bytes measurement does see it — one more copy of the
    // bids, to the byte — but the absolute total varies by a third of
    // a megabyte with which rows ran before it in the same binary, so
    // no stable ceiling can be drawn from that either. The change
    // therefore rests on the scan's own requirements, below, rather
    // than on a memory bound.
    //
    // The scan needs four scalars per bid — `impid`, `price`,
    // `deal_id`, and the index — and `Winner` already owns a copy of
    // the ones it keeps, so nothing in the mutation pass depends on
    // the scan's borrow surviving it. Splitting the two phases is what
    // makes the borrow checker accept the borrowed form.
    //
    // `pick_winner` takes slices rather than owned vectors for the
    // same reason: its only caller is here, so there is no owned form
    // left to keep it honest.
    let imps: Vec<&str> = underlying.imp.iter().map(|imp| imp.id.as_str()).collect();
    let winners: Vec<Option<Winner>> = {
        let borrowed: Vec<(BidderName, &[TypedBid])> = bundles
            .iter()
            .map(|(name, bundle)| (name.clone(), bundle.bids.as_slice()))
            .collect();
        imps.iter()
            .map(|imp_id| pick_winner(&borrowed, imp_id, prefer_deals))
            .collect()
    };
    for (imp_id, winner) in imps.iter().zip(winners.iter()) {
        for (bidder_name, bundle) in bundles.iter_mut() {
            attach_targeting_for_imp(bidder_name, bundle, imp_id, winner.as_ref(), &cfg, ctx.rng);
        }
    }
}

fn targeting_prefix<'a>(ctx: &'a AuctionContext<'_>, request: &'a BidRequest) -> &'a str {
    request
        .ext
        .as_ref()
        .and_then(|ext| ext.prebid.as_ref())
        .and_then(|prebid| prebid.targeting.as_ref())
        .and_then(|targeting| targeting.prefix.as_deref())
        .or_else(|| {
            let prefix = ctx.account.config.targeting_prefix.as_str();
            (!prefix.is_empty()).then_some(prefix)
        })
        .unwrap_or("hb")
}

/// Resolve `hb_env`: `"mobile-app"` when the request
/// carries `app`, `"amp"` when the request carries `site.ext.amp == 1`,
/// otherwise unset.
fn resolve_env(underlying: &BidRequest) -> Option<&'static str> {
    if underlying.app.is_some() {
        return Some("mobile-app");
    }
    if underlying
        .site
        .as_ref()
        .and_then(|s| s.ext.as_ref())
        .and_then(|ext| ext.amp)
        == Some(1_i32)
    {
        return Some("amp");
    }
    None
}

/// Targeting configuration resolved once per auction from the request
/// `ext.prebid.targeting` block and the account config, then read
/// unchanged for every (imp, bidder, bid). Holding it as one value
/// keeps the resolution in one place and stops the per-imp loop from
/// threading a dozen unrelated settings by hand.
struct TargetingConfig<'a> {
    prefix: &'a str,
    truncate_target_attribute: Option<usize>,
    granularity: PriceGranularity,
    media_type_granularity: BTreeMap<&'a str, PriceGranularity>,
    rounding: BidRounding,
    include_winners: bool,
    include_bidder_keys: bool,
    include_format: bool,
    always_include_deals: bool,
    env: Option<&'static str>,
}

fn attach_targeting_for_imp(
    bidder_name: &BidderName,
    bundle: &mut BidderResultBundle,
    imp_id: &str,
    winner: Option<&Winner>,
    cfg: &TargetingConfig<'_>,
    rng: &dyn auctioneer_runtime::Rng,
) {
    for (bid_index, typed) in bundle.bids.iter_mut().enumerate() {
        if typed.bid.impid != *imp_id {
            continue;
        }
        let format_str = match typed.media_type {
            MediaType::Banner => "banner",
            MediaType::Video => "video",
            MediaType::Native => "native",
            MediaType::Audio => "audio",
        };
        let effective_granularity = cfg
            .media_type_granularity
            .get(format_str)
            .unwrap_or(&cfg.granularity);
        let bucket = price_bucket(typed.bid.price, effective_granularity, cfg.rounding, rng);
        // Only THIS specific bid — matched by (bidder, bid_index) —
        // gets the unsuffixed winner keys. `pick_winner` records
        // `bid_index` inside the bidder's own bids vec, so the
        // enumerate() above lines up 1:1.
        let is_winner =
            winner.is_some_and(|w| &w.bidder == bidder_name && w.bid_index == bid_index);
        let mut keys = build_keys(&TargetingInput {
            prefix: cfg.prefix,
            truncate_target_attribute: cfg.truncate_target_attribute,
            bidder: bidder_name.as_str(),
            price_bucket: &bucket,
            size_wh: Some((
                u32::try_from(typed.bid.w.unwrap_or(0_i32)).unwrap_or(0_u32),
                u32::try_from(typed.bid.h.unwrap_or(0_i32)).unwrap_or(0_u32),
            )),
            deal_id: typed.bid.deal_id.as_deref(),
            env: cfg.env,
            include_winners: cfg.include_winners,
            include_bidder_keys: cfg.include_bidder_keys,
            include_format: cfg.include_format,
            always_include_deals: cfg.always_include_deals,
            format: Some(format_str),
            is_overall_winner: is_winner,
        });
        bundle.warnings.append(&mut keys.warnings);
        let mut combined: BTreeMap<String, String> = BTreeMap::new();
        for (key, value) in &keys.winner {
            combined.insert(key.clone(), value.clone());
        }
        for (key, value) in &keys.per_bidder {
            combined.insert(key.clone(), value.clone());
        }
        let ext = typed.bid.ext.get_or_insert_with(ExtBid::default);
        let prebid = ext.prebid.get_or_insert_with(ExtBidPrebid::default);
        prebid.bid_type = Some(format_str.to_owned());
        if prebid.meta.is_none() {
            prebid.meta = Some(serde_json::Value::Object(serde_json::Map::new()));
        }
        prebid.targeting = combined;
    }
}

/// Per-bidder timeout resolution chain. See the
/// call-site comment for the four-step ladder. Extracted as a pure
/// helper so the alias-fallback lookup is unit-testable and the
/// call site stays readable.
fn resolve_bidder_timeout(
    overrides: &BTreeMap<BidderName, u32>,
    aliases: &AliasMap,
    bidder_name: &BidderName,
    baked_bidder_timeout_ms: Option<u32>,
    bidder_visible_tmax_ms: u32,
) -> u64 {
    if let Some(value) = overrides.get(bidder_name) {
        return u64::from(*value);
    }
    if let Some(base) = aliases.bases.get(bidder_name)
        && let Some(value) = overrides.get(base)
    {
        return u64::from(*value);
    }
    if let Some(value) = baked_bidder_timeout_ms {
        return u64::from(value);
    }
    u64::from(bidder_visible_tmax_ms)
}

fn returnallbidstatus(req: &BidRequest, features: ExchangeFeatures) -> bool {
    req.ext
        .as_ref()
        .and_then(|e| e.prebid.as_ref())
        .and_then(|p| p.returnallbidstatus)
        .unwrap_or(features.returnallbidstatus_default)
}

fn with_id(mut resp: BidResponse, id: String) -> BidResponse {
    resp.id = id;
    resp
}

fn extract_publisher_id(underlying: &BidRequest) -> Option<&str> {
    if let Some(site) = underlying.site.as_ref()
        && let Some(publisher) = site.publisher.as_ref()
        && let Some(id) = publisher.id.as_deref()
    {
        return Some(id);
    }
    let app = underlying.app.as_ref()?;
    let publisher = app.publisher.as_ref()?;
    publisher.id.as_deref()
}

/// The registry entry one observable bidder name resolves to.
///
/// ALIAS MAP FIRST. A name that is declared as an alias resolves to its
/// base, even when the registry also carries a bidder of that name.
///
/// The order matters only for a collision — an alias whose own name is
/// also a registered bidder, as `{"aliases": {"fake_shadow": "fake_base"}}`
/// is — and before this it ran the other way here while running
/// alias-first everywhere else. `PerBidderGroup::base` comes from the
/// same alias map, so construction picked the base adapter and its
/// endpoint while this function handed back the shadowed bidder's
/// `BidderInfo`: the call went out to the base endpoint under the
/// shadowed bidder's compression and timeout policy. The privacy
/// permissions and sampling overrides read `group.base()` too, so this
/// was the one site that disagreed.
///
/// Resolving consistently rather than REJECTING the collision, for two
/// reasons. A rejection would have to be a 400, and `resolve_aliases`
/// deliberately does not 400 on a host-declared alias — an operator
/// typo in deployment config must not take down every auction on the
/// deployment — so a collision declared in `[aliases]` would need a
/// different and quieter answer than one declared on the request,
/// giving two rules for one shape. And the alias map is already
/// authoritative at every other site, so the fix is to stop one
/// function from being the exception, not to add a new error class.
///
/// What a publisher gets is therefore what they asked for: the alias
/// names the base, and the base decides the adapter, the endpoint, the
/// compression and the timeout alike. The observable name still travels
/// — `seat`, `responsetimemillis` and the per-bidder timeout override
/// are keyed on it — so aliasing over a registered name shadows that
/// bidder for the request rather than silently blending the two.
fn resolve_bidder_or_alias<'reg>(
    ctx: &'reg AuctionContext<'_>,
    aliases: &AliasMap,
    bidder_name: &BidderName,
) -> Option<(&'reg BidderInfo, &'reg dyn Bidder)> {
    ctx.bidders
        .get(base_identity(aliases, bidder_name).as_str())
}

/// The BASE identity one observable bidder name carries.
///
/// The alias map decides, and this is the one definition of that. It is
/// the same answer `PerBidderGroup::base` holds — the splitter computes
/// it from this same map — so a caller that resolves through here and a
/// caller that reads `group.base()` cannot disagree about which bidder a
/// name is.
///
/// Separated from [`resolve_bidder_or_alias`] because it is the whole
/// claim and the registry lookup around it is not: a test can assert
/// the ORDER here without building an `AuctionContext`, which is what
/// `an_alias_over_a_registered_name_resolves_to_its_base` does.
fn base_identity(aliases: &AliasMap, bidder_name: &BidderName) -> BidderName {
    aliases
        .bases
        .get(bidder_name)
        .cloned()
        .unwrap_or_else(|| bidder_name.clone())
}

#[cfg(test)]
mod tests {
    use super::FanoutError;
    use super::{
        AliasMap, BidderResultBundle, Rng, apply_sampling, base_identity, cached_privacy_decision,
        fanout_error_codes, resolve_bidder_timeout, should_keep,
    };
    use crate::fanout::OversizeRequestStage;
    use crate::scrub::DisclosureAudit;
    use auctioneer_bidder::BidderName;
    use auctioneer_config::types::IpKeepBits;
    use auctioneer_privacy::{FetchDenial, PrivacyContext};
    use auctioneer_runtime::SeededRng;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
    use std::time::Duration;

    #[test]
    fn browser_configurations_require_request_and_exact_nonempty_call_coverage() {
        use auctioneer_bidder::{BrowserAuctionConfig, OutboundRequestMetaBuilder};
        use auctioneer_openrtb::types::bid_request::BidRequest;
        use serde_json::json;

        let request: BidRequest = serde_json::from_value(json!({"id":"request", "imp":[
            {"id":"eligible"}, {"id":"other_call"}
        ]}))
        .unwrap();
        for (sent_ids, expected) in [
            (vec!["eligible", "not_in_request"], vec!["eligible"]),
            (vec!["other_call"], vec!["other_call"]),
            (Vec::new(), Vec::new()),
        ] {
            let mut storage = Vec::with_capacity(auctioneer_bidder::META_BYTES_CAPACITY);
            let mut builder = OutboundRequestMetaBuilder::new(&mut storage, 3);
            builder.set_method("POST").unwrap();
            builder.set_url("https://example.test/bid").unwrap();
            for id in sent_ids {
                builder.imp_id(id).unwrap();
            }
            let sent = builder.finish();
            let mut configs: Vec<_> = ["eligible", "other_call", "not_in_request"]
                .into_iter()
                .map(|imp_id| BrowserAuctionConfig {
                    imp_id: imp_id.into(),
                    config: json!({"payload":"kept"}),
                })
                .collect();
            assert!(super::retain_auction_configs(&request, &sent, &mut configs));
            assert_eq!(
                configs
                    .iter()
                    .map(|row| row.imp_id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }

    struct ScopeClock {
        start: auctioneer_runtime::MonotonicInstant,
        reads: AtomicU32,
        live_reads: u32,
    }

    impl auctioneer_runtime::Clock for ScopeClock {
        fn now(&self) -> time::OffsetDateTime {
            time::OffsetDateTime::UNIX_EPOCH
        }
        fn now_monotonic(&self) -> auctioneer_runtime::MonotonicInstant {
            if self.reads.fetch_add(1, Ordering::Relaxed) < self.live_reads {
                self.start
            } else {
                self.start
                    .checked_add(Duration::from_secs(1))
                    .expect("one-second test deadline fits")
            }
        }
    }

    fn disclosure_scopes() -> (
        Vec<super::PerBidderRequest>,
        BTreeMap<BidderName, auctioneer_privacy::BidderDecision>,
    ) {
        let mut decisions = BTreeMap::new();
        let scopes = ["fake_first", "fake_sibling"]
            .into_iter()
            .map(|name| {
                let bidder = BidderName::new(name);
                let mut decision =
                    PrivacyContext::allow_all_no_scrub().bidder_decision("fake_bb", name);
                decision.transmit_ufpd = false;
                decisions.insert(bidder.clone(), decision);
                super::PerBidderRequest {
                    bidder,
                    base: BidderName::new("fake_bb"),
                    request: serde_json::from_value(serde_json::json!({
                        "id": "request", "user": {"id": name}
                    }))
                    .unwrap(),
                }
            })
            .collect();
        (scopes, decisions)
    }

    #[test]
    fn original_deadline_stops_disclosure_before_first_and_between_siblings() {
        for live_reads in [0, 1] {
            let clock = ScopeClock {
                start: auctioneer_runtime::MonotonicInstant::now(),
                reads: AtomicU32::new(0),
                live_reads,
            };
            let (mut scopes, decisions) = disclosure_scopes();
            let mut audit = DisclosureAudit::default();
            let completed = super::scrub_bidder_requests(
                &mut scopes,
                &decisions,
                IpKeepBits { v4: 24, v6: 56 },
                &mut audit,
                None,
                &clock,
                clock.start.checked_add(Duration::from_secs(1)).unwrap(),
            )
            .unwrap();
            assert!(!completed);
            assert_eq!(audit.attempted_scopes, u64::from(live_reads));
            assert_eq!(audit.failed_scopes, 0);
            assert_eq!(
                scopes[0].request.user.as_ref().unwrap().id.is_none(),
                live_reads == 1
            );
            assert_eq!(
                scopes[1].request.user.as_ref().unwrap().id.as_deref(),
                Some("fake_sibling")
            );
        }
    }

    #[test]
    fn later_scope_refusal_keeps_prior_changes_and_both_audit_outcomes() {
        let clock = ScopeClock {
            start: auctioneer_runtime::MonotonicInstant::now(),
            reads: AtomicU32::new(0),
            live_reads: u32::MAX,
        };
        let (mut scopes, decisions) = disclosure_scopes();
        scopes[1]
            .request
            .other
            .insert("unknown".into(), serde_json::json!(vec![0_i32; 20]));
        let mut audit = DisclosureAudit::default();
        let result = super::scrub_bidder_requests(
            &mut scopes,
            &decisions,
            IpKeepBits { v4: 24, v6: 56 },
            &mut audit,
            Some(super::RequestConstructionLimits {
                max_nodes: 16,
                max_serialized_bytes: 1024,
                reusable_scratch_bytes: 0,
                retained_per_scope_bytes: 0,
            }),
            &clock,
            clock.start.checked_add(Duration::from_secs(1)).unwrap(),
        );
        assert!(matches!(result, Err(crate::AuctionError::Internal(_))));
        assert!(scopes[0].request.user.as_ref().unwrap().id.is_none());
        assert_eq!(
            scopes[1].request.user.as_ref().unwrap().id.as_deref(),
            Some("fake_sibling")
        );
        assert_eq!(audit.attempted_scopes, 2);
        assert_eq!(audit.failed_scopes, 1);
        assert_eq!(audit.counts(), [1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn privacy_decision_cache_evaluates_once_per_requested_identity() {
        let privacy = PrivacyContext::allow_all_no_scrub();
        let mut decisions = BTreeMap::new();
        let mut evaluations = 0_usize;
        for _ in 0..100_usize {
            for name in ["fake_denied_alias", "fake_allowed_alias", "fake_bb"] {
                let bidder = BidderName::new(name);
                let decision = cached_privacy_decision(&mut decisions, &bidder, || {
                    evaluations += 1;
                    let mut decision = privacy.bidder_decision("fake_bb", name);
                    if name == "fake_denied_alias" {
                        decision.fetch_denial = Some(FetchDenial::Activity);
                    }
                    decision
                });
                assert_eq!(
                    decision.fetch_denial,
                    (name == "fake_denied_alias").then_some(FetchDenial::Activity)
                );
            }
        }
        assert_eq!(evaluations, 3, "aliases must not share their base decision");
        assert_eq!(decisions.len(), 3);
    }

    /// An alias whose own name is ALSO a registered bidder resolves to
    /// its base, not to the registry entry that shares its name.
    ///
    /// `{"aliases": {"fake_ccc": "fake_bb"}}` is the collision. The
    /// splitter computes `PerBidderGroup::base` from this same map, so
    /// it already answers `fake_bb` — the adapter, the endpoint and the
    /// privacy permissions all follow from that. `build_call_list`
    /// looked the observable name up in the registry FIRST, found the
    /// shadowed base entry, and took its `endpoint_compression` and
    /// `bidder_timeout_ms`. So the call went to a resolved base endpoint under
    /// the shadowed bidder's compression and timeout policy: one bidder's adapter
    /// with another's transport.
    ///
    /// Asserted on the base IDENTITY rather than on a resolved
    /// `BidderInfo`, because the identity is the decision and the
    /// registry lookup after it is not. A row that built an
    /// `AuctionContext` to compare two `BidderInfo`s would be asserting
    /// the same thing through more machinery.
    ///
    /// The three non-colliding cases are here too, so the row cannot
    /// pass by `base_identity` simply always returning the alias map's
    /// answer or always returning its argument.
    #[test]
    fn an_alias_over_a_registered_name_resolves_to_its_base() {
        let mut aliases = AliasMap::default();
        // The collision: an alias named after a bidder that exists.
        aliases
            .bases
            .insert(BidderName::new("fake_ccc"), BidderName::new("fake_bb"));
        // An ordinary alias, over a name no bidder has.
        aliases
            .bases
            .insert(BidderName::new("fake_resell"), BidderName::new("fake_aaa"));

        assert_eq!(
            base_identity(&aliases, &BidderName::new("fake_ccc")),
            BidderName::new("fake_bb"),
            "a declared alias decides the base even when its own name is a \
             registered bidder. Preferring the registry here is what sent a Generic \
             endpoint under the shadowed bidder's compression and timeout policy, while \
             PerBidderGroup::base -- read from this same map -- said fake_bb",
        );
        assert_eq!(
            base_identity(&aliases, &BidderName::new("fake_resell")),
            BidderName::new("fake_aaa"),
            "an ordinary alias still resolves to its base",
        );
        // Not an alias: the name IS the base.
        assert_eq!(
            base_identity(&aliases, &BidderName::new("fake_aaa")),
            BidderName::new("fake_aaa"),
            "a plain bidder name resolves to itself",
        );
        assert_eq!(
            base_identity(&aliases, &BidderName::new("fake_bb")),
            BidderName::new("fake_bb"),
            "including one that is some alias's base",
        );
        // An undeclared name resolves to itself, so the registry lookup
        // that follows is the thing that reports it unknown. Resolving
        // it to something else here would turn an unknown bidder into a
        // silently different one.
        assert_eq!(
            base_identity(&aliases, &BidderName::new("fake_nobody")),
            BidderName::new("fake_nobody"),
            "an undeclared name is left alone for the registry to refuse",
        );
    }

    /// The two outbound-side failures render the codes the spec's wire
    /// table assigns, and neither renders `999`.
    ///
    /// Asserted on the RENDERED pair, not on the intermediate category:
    /// a row that pinned the category would hold for whatever pair the
    /// mapping gave it, which is how both of these came to render
    /// 999/100. The `Other` row is kept alongside them so the
    /// assertions cannot pass by every arm agreeing.
    #[test]
    fn outbound_side_failures_render_their_specified_codes() {
        assert_eq!(
            fanout_error_codes(&FanoutError::OversizeRequest {
                stage: OversizeRequestStage::Encoded,
            }),
            (5, 100),
            "an encoded-size overflow is FailedToRequestBids: auctioneer built a \
             request it had no budget to carry, so the bidder never reached \
             the fan-out",
        );
        assert_eq!(
            fanout_error_codes(&FanoutError::Preflight),
            (4, 100),
            "a preflight refusal is the code the spec's wire table assigns it",
        );
        assert_eq!(
            fanout_error_codes(&FanoutError::Other("x".to_owned())),
            (999, 100),
            "999 stays reserved for an uncategorised fault, which neither of \
             the two above is",
        );
    }

    #[test]
    fn timeout_alias_inherits_base_override() {
        let mut overrides: BTreeMap<BidderName, u32> = BTreeMap::new();
        overrides.insert(BidderName::new("fake_aaa"), 250_u32);
        let mut aliases = AliasMap::default();
        aliases
            .bases
            .insert(BidderName::new("fake_bz"), BidderName::new("fake_aaa"));

        // Alias inherits the base bidder's override.
        assert_eq!(
            resolve_bidder_timeout(
                &overrides,
                &aliases,
                &BidderName::new("fake_bz"),
                None,
                1000,
            ),
            250,
            "fake_bz must fall back to [bidders.fake_aaa].timeout_ms",
        );

        // Alias with its OWN override wins over the base fallback.
        overrides.insert(BidderName::new("fake_bz"), 400_u32);
        assert_eq!(
            resolve_bidder_timeout(
                &overrides,
                &aliases,
                &BidderName::new("fake_bz"),
                None,
                1000,
            ),
            400,
            "an explicit alias override wins over the base fallback",
        );

        // Bidder with no override, no alias, no baked → tmax passthrough.
        assert_eq!(
            resolve_bidder_timeout(
                &overrides,
                &aliases,
                &BidderName::new("fake_ccc"),
                None,
                750,
            ),
            750,
            "unlisted bidder falls all the way through to bidder_visible_tmax_ms",
        );

        // Baked BidderInfo.bidder_timeout_ms wins when no overrides.
        overrides.clear();
        assert_eq!(
            resolve_bidder_timeout(
                &overrides,
                &aliases,
                &BidderName::new("fake_aaa"),
                Some(300),
                1000,
            ),
            300,
            "baked BidderInfo.bidder_timeout_ms is the third step",
        );
    }

    fn alias_map(bidder: &str, base: &str) -> AliasMap {
        let mut aliases = AliasMap::default();
        aliases
            .bases
            .insert(BidderName::new(bidder), BidderName::new(base));
        aliases
    }

    /// Counts `next_u64` calls so a boundary short-circuit that returns
    /// before drawing is distinguishable from a general-path draw that
    /// happens to agree with it.
    struct CountingRng {
        calls: AtomicU64,
        value: u64,
    }

    impl CountingRng {
        fn new(value: u64) -> Self {
            Self {
                calls: AtomicU64::new(0),
                value,
            }
        }

        fn call_count(&self) -> u64 {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl Rng for CountingRng {
        fn next_u64(&self) -> u64 {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.value
        }
    }

    #[test]
    fn sample_rate_1_0_keeps_every_bidder() {
        // Worst-case draw for the buggy naive formula this guards
        // against (`next_u64() as f64 / u64::MAX as f64` hits exactly
        // 1.0 here). The half-open formula never reaches 1.0, and the
        // short-circuit never even draws.
        let rng = CountingRng::new(u64::MAX);
        assert!(should_keep(1.0, &rng), "sample_rate = 1.0 must always keep");
        assert_eq!(
            rng.call_count(),
            0,
            "the rate >= 1.0 short-circuit must return before drawing",
        );
    }

    #[test]
    fn sample_rate_0_0_drops_every_bidder_silently() {
        let rng = CountingRng::new(0);
        let mut overrides: BTreeMap<BidderName, f64> = BTreeMap::new();
        overrides.insert(BidderName::new("fake_x"), 0.0_f64);
        overrides.insert(BidderName::new("fake_y"), 0.0_f64);
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        let kept = apply_sampling(
            vec![BidderName::new("fake_x"), BidderName::new("fake_y")],
            &AliasMap::default(),
            &overrides,
            &rng,
            &mut bundles,
        );
        assert!(kept.is_empty(), "every bidder at rate 0.0 must be dropped");
        for bundle in bundles.values() {
            assert!(
                bundle.warnings.is_empty(),
                "silent sampling must not emit a warning code",
            );
            assert!(
                !bundle.dispatched,
                "a sampled-out bidder was never dispatched"
            );
        }
        assert_eq!(
            rng.call_count(),
            0,
            "the rate <= 0.0 short-circuit must return before drawing",
        );
    }

    /// Lock-in regression guard: after every bidder samples out, `bundles`
    /// still carries one entry per dropped bidder so the response
    /// builder's "no live bidders" branch stays reachable and
    /// `nbr = NoBidInvalidRequest` still fires.
    #[test]
    fn sample_rate_0_0_writes_sentinel_bundle_with_no_warning() {
        let rng = SeededRng::new(42);
        let mut overrides: BTreeMap<BidderName, f64> = BTreeMap::new();
        overrides.insert(BidderName::new("fake_z"), 0.0_f64);
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        let kept = apply_sampling(
            vec![BidderName::new("fake_z")],
            &AliasMap::default(),
            &overrides,
            &rng,
            &mut bundles,
        );
        assert!(kept.is_empty());
        assert_eq!(bundles.len(), 1, "one sentinel entry per dropped bidder");
        let bundle = bundles
            .get(&BidderName::new("fake_z"))
            .expect("sentinel bundle present for the dropped bidder");
        assert!(
            !bundle.dispatched,
            "sentinel bundle carries dispatched = false"
        );
        assert!(
            bundle.warnings.is_empty(),
            "no warning code for silent sampling"
        );
        assert!(bundle.errors.is_empty());
        assert!(bundle.bids.is_empty());
    }

    #[test]
    fn sample_rate_between_bounds_drops_proportionally_over_many_rng_draws() {
        let draws = 5000_u32;
        // Two mid-range rates, not one. A single rate passes even when
        // the sampler compares against a hardcoded constant instead of
        // its `rate` argument; the bands below are disjoint, so only a
        // sampler that actually reads `rate` satisfies both.
        for (rate, lo, hi) in [
            (0.25_f64, 1000_u32, 1500_u32),
            (0.75_f64, 3500_u32, 4000_u32),
        ] {
            let rng = SeededRng::new(42);
            let mut kept = 0_u32;
            for _ in 0_u32..draws {
                if should_keep(rate, &rng) {
                    kept += 1;
                }
            }
            // Expected ~1250 and ~3750 (std dev ~31). Each band is
            // roughly 8 standard deviations wide -- not flaky -- yet
            // still excludes a flipped comparison, an "always keep" /
            // "always drop" bug, and a rate the sampler ignored.
            assert!(
                (lo..=hi).contains(&kept),
                "kept {kept} of {draws} draws at sample_rate {rate}; expected within {lo}..={hi}",
            );
        }
    }

    #[test]
    fn bidder_without_override_bypasses_sampler() {
        let rng = CountingRng::new(0);
        let mut overrides: BTreeMap<BidderName, f64> = BTreeMap::new();
        overrides.insert(BidderName::new("fake_someone_else"), 0.0_f64);
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        let kept = apply_sampling(
            vec![BidderName::new("fake_unlisted")],
            &AliasMap::default(),
            &overrides,
            &rng,
            &mut bundles,
        );
        assert_eq!(
            kept.len(),
            1,
            "a bidder with no override must not be sampled"
        );
        assert_eq!(kept[0].as_str(), "fake_unlisted");
        assert!(
            bundles.is_empty(),
            "a bidder that bypasses the sampler gets no sentinel entry",
        );
        assert_eq!(
            rng.call_count(),
            0,
            "a bidder with no override on either name must never consult the RNG",
        );
    }

    #[test]
    fn request_alias_inherits_base_sample_rate_when_no_alias_override() {
        let rng = SeededRng::new(42);
        let mut overrides: BTreeMap<BidderName, f64> = BTreeMap::new();
        overrides.insert(BidderName::new("fake_aaa"), 0.0_f64);
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        let kept = apply_sampling(
            vec![BidderName::new("fake_my")],
            &alias_map("fake_my", "fake_aaa"),
            &overrides,
            &rng,
            &mut bundles,
        );
        assert!(
            kept.is_empty(),
            "alias `fake_my` must inherit fake_aaa's 0.0 rate via the base fallback",
        );
        assert!(
            bundles.contains_key(&BidderName::new("fake_my")),
            "the sentinel bundle is keyed by the observable alias name",
        );
    }

    #[test]
    fn alias_override_wins_over_base_sample_rate() {
        let rng = SeededRng::new(42);
        let mut overrides: BTreeMap<BidderName, f64> = BTreeMap::new();
        overrides.insert(BidderName::new("fake_aaa"), 0.0_f64);
        overrides.insert(BidderName::new("fake_my"), 1.0_f64);
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        let kept = apply_sampling(
            vec![BidderName::new("fake_my")],
            &alias_map("fake_my", "fake_aaa"),
            &overrides,
            &rng,
            &mut bundles,
        );
        assert_eq!(
            kept.len(),
            1,
            "the alias's own override (1.0) must win over the base's 0.0",
        );
        assert_eq!(kept[0].as_str(), "fake_my");
        assert!(bundles.is_empty());
    }
}

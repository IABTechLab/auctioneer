//! Deadline composition and the completion-driven wave scheduler.
//!
//! Two functions, two anchors. [`compose_all`] runs ONCE at fanout entry
//! and fixes every call's immutable absolute deadline. [`run_wave`] runs
//! once per acquired gate lease with its own batch-entry instant, which
//! anchors nothing but the preflight elapsed, and dispatches at most the
//! calls that lease covers.
//!
//! # The four observable outcomes of a batch poll
//!
//! `OutboundBatch::next()` has exactly four, and each one means something
//! different about the slots that are still unfilled:
//!
//! | poll result | what it says | what [`run_wave`] does |
//! |---|---|---|
//! | `Ok(Item(item))` | one slot is terminal | record it at `item.index` |
//! | `Ok(Finished(Completed))` | EVERY dispatched slot resolved | an unfilled slot is a positional breach ⇒ [`FanoutInternalError::BatchContractViolation`] |
//! | `Ok(Finished(Cutoff))` | the observation window closed | unfilled slots become expired positional results — NOT a breach |
//! | `Err(_)` or a future variant | the driver broke an invariant | [`FanoutInternalError::Internal`] ⇒ 500 |
//!
//! The `Completed`/`Cutoff` split is the whole point. Under `Completed`
//! upstream is asserting it resolved everything, so a hole is a genuine
//! positional bug and must surface as one. Under `Cutoff` a hole is the
//! expected shape, and treating it as a breach would turn every slow
//! bidder into a 500.
//!
//! # Why the loop is completion-driven rather than collected
//!
//! `collect()` is correct and its failure type even preserves the slots
//! it had already gathered, but it returns only when the whole batch is
//! done. Polling `next()` alongside a decode `FuturesUnordered` means
//! decoding one bidder's response does not delay OBSERVING a sibling's
//! completion. There is exactly one live path through this scheduler; a
//! second one would double the surface where the terminal/cutoff
//! distinction can drift.
//!
//! A full slot tally is not completion: upstream validates the driver
//! through clean EOF. A late failure, duplicate, out-of-range item or
//! cutoff after the final slot still fails the whole fanout, including
//! successes from earlier waves. The wave lease remains held while EOF
//! is pending and is released if the fanout is dropped.
//!
//! # Why nothing here calls `cancel()`
//!
//! There is no non-fatal trigger for it. A cutoff arrives as
//! `Finished(Cutoff)` and the loop ends normally; a driver fault arrives
//! as `Err` and becomes the outer error. Handler teardown drops the
//! batch, and the drop — not an explicit cancel — is what applies the
//! adapter's teardown.

use super::{
    BoundedOutboundRequest, BoundedProxyClientApi, BoundedProxyResponse, BoundedSlotError,
    BoundedSlotResult, CapRow, ComposedDeadline, DecodePolicy, ResponseBodyPolicy,
    policy_from_limits, to_outbound,
};
use crate::host_primitives::fanout::is_biddable_status;
use auctioneer_bidder::BidderHttpResponse;
use auctioneer_exchange::fanout::{TransportError, TransportErrorKind};
use auctioneer_exchange::{
    AuctionDeadline, BidderCall, BidderCallResult, FanoutError, FanoutInternalError,
    OversizeOrigin, OversizeRequestStage, TimeoutCause,
};
use auctioneer_runtime::{
    Clock, DecodeCooperator, MonotonicInstant, ResolvedIsolateLimits, StreamGate, Timer,
};
use edgezero_core::error::{BadGatewayReason, EdgeError, ResponseLimitReason};
use edgezero_core::outbound::{
    OutboundBatch, OutboundBatchNext, OutboundBatchTermination, OutboundRequest, OutboundSlotResult,
};
use edgezero_core::time::Deadline;
use futures::StreamExt as _;
use futures::future::{Either, select};
use futures::stream::FuturesUnordered;
use std::collections::VecDeque;
use std::future::{Future, poll_fn};
use std::iter::repeat_with;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

/// One slot on its way through the scheduler, with everything it needs
/// to survive the conversion that consumes its call.
#[derive(Debug)]
pub struct Slot {
    /// This slot's terminal result.
    pub result: BoundedSlotResult,
    /// The deadline composed for it at fanout entry.
    pub deadline: ComposedDeadline,
    /// The decode caps copied out of its policy before conversion.
    pub decode: DecodePolicy,
}

/// A call that cleared preflight and is going upstream.
struct Pending {
    index: usize,
    deadline: ComposedDeadline,
    decode: DecodePolicy,
    request: OutboundRequest,
}

/// What [`compose_all`] made of one call.
///
/// Composition can refuse a call outright when its budget is already due.
/// A refusal is POSITIONAL, so it has to travel in the same ordered
/// vector as the calls that survived rather than
/// aborting the fanout. Keeping both in one enum is what lets
/// [`drive_fanout`] enumerate ONCE and have every original index already
/// correct, whichever arm it lands on.
pub enum Composed {
    /// The call is finalized and awaits a wave.
    ///
    /// Boxed because a [`BoundedOutboundRequest`] is by far the larger
    /// arm, and an unboxed variant would make every refusal carry its
    /// footprint too.
    Dispatch(Box<BoundedOutboundRequest>),
    /// Composition refused the call. Nothing was sent and nothing will
    /// be; this is already its terminal result.
    Refused {
        /// Why composition could not produce a request.
        error: BoundedSlotError,
        /// The deadline composed before the refusal, so the slot still
        /// reports the cause auctioneer selected for it.
        deadline: ComposedDeadline,
        /// The decode caps this call would have carried.
        decode: DecodePolicy,
    },
}

impl Composed {
    /// The deadline composed for this call, whichever arm it landed in.
    ///
    /// Present on BOTH arms rather than only on the dispatched one: the
    /// deadline is fixed before composition can refuse anything, and a
    /// refused slot still publishes the cause auctioneer selected for it. A
    /// caller asking "what budget did this call get" therefore has one
    /// answer regardless of whether the call was ever sent.
    #[must_use]
    pub fn deadline(&self) -> ComposedDeadline {
        match self {
            Self::Dispatch(request) => request.deadline,
            Self::Refused { deadline, .. } => *deadline,
        }
    }
}

/// The host primitives the wave scheduler reads time and yields through.
///
/// Bundled rather than passed one by one because they are a single
/// concern — this target's notion of time — and because passing them
/// separately made the two scheduler entry points the widest signatures
/// in the crate. Borrowed, not owned: every one of them is a boot-time
/// singleton the caller already holds.
#[derive(Clone, Copy)]
pub struct WaveHost<'a> {
    /// Reads both the wall and the monotonic axis.
    pub clock: &'a dyn Clock,
    /// Sleeps the acquisition race's deadline arm.
    pub timer: &'a dyn Timer,
    /// Returns to the host between auctioneer's own decode and the one
    /// deadline recheck that follows it, so a runtime whose clock is
    /// frozen for the duration of a turn reports a fresh instant.
    pub cooperator: &'a dyn DecodeCooperator,
}

/// Independently narrow both response caps to the call's resolved values.
/// The policy supplies funded ceilings, never lower-priority host defaults.
fn narrow_to_call(policy: ResponseBodyPolicy, call: &BidderCall) -> ResponseBodyPolicy {
    ResponseBodyPolicy {
        max_transport_bytes: policy.max_transport_bytes.min(call.max_transport_bytes),
        max_decoded_bytes: policy.max_decoded_bytes.min(call.max_decoded_bytes),
        ..policy
    }
}

/// Fixes every finalized call's absolute deadline and response caps
/// once, at fanout entry.
///
/// `deadline_at` is `min(fanout_entry_at + call.timeout,
/// auction.deadline_at)` and `selected_cause` records which operand won,
/// with equality selecting [`TimeoutCause::Bidder`] for a live auction. Composing here
/// rather than per wave is what stops a call that lost gate contention
/// from silently receiving its full budget again on a later wave.
///
/// Exchange construction already finalized encoding and retained rejected
/// positions. This layer never rewrites the request body or metadata.
///
/// # Errors
/// [`FanoutInternalError::Internal`] when `fanout_entry_at +
/// call.timeout` is not a representable instant. The overflow is
/// reported BEFORE any dispatch and is deliberately not saturated: a
/// saturating add would hand the call a far-future deadline, which reads
/// as "this bidder has essentially forever" — the opposite of what an
/// unrepresentable budget means. An auction already due at entry returns
/// positional expiries before this arithmetic; zero-timeout calls are
/// also terminal at composition.
pub fn compose_all(
    calls: Vec<BidderCall>,
    auction: &AuctionDeadline,
    fanout_entry_at: MonotonicInstant,
    policy: ResponseBodyPolicy,
    _max_encoded_bytes: usize,
) -> Result<Vec<Composed>, FanoutInternalError> {
    calls
        .into_iter()
        .map(|call| {
            let deadline = if auction.deadline_at <= fanout_entry_at {
                ComposedDeadline {
                    deadline_at: auction.deadline_at,
                    selected_cause: TimeoutCause::AuctionDeadline,
                }
            } else {
                let bidder_at = fanout_entry_at.checked_add(call.timeout).ok_or_else(|| {
                    FanoutInternalError::Internal("deadline composition overflow".to_owned())
                })?;
                let (deadline_at, selected_cause) = if bidder_at <= auction.deadline_at {
                    (bidder_at, TimeoutCause::Bidder)
                } else {
                    (auction.deadline_at, TimeoutCause::AuctionDeadline)
                };
                ComposedDeadline {
                    deadline_at,
                    selected_cause,
                }
            };
            let narrowed = narrow_to_call(policy, &call);
            if deadline.deadline_at <= fanout_entry_at {
                return Ok(Composed::Refused {
                    error: BoundedSlotError::Expired,
                    deadline,
                    decode: DecodePolicy::from(&narrowed),
                });
            }
            Ok(Composed::Dispatch(Box::new(BoundedOutboundRequest {
                request: call.request,
                policy: narrowed,
                deadline,
            })))
        })
        .collect()
}

/// Runs one wave: preflight, dispatch, observe, decode.
///
/// `out` is indexed by ORIGINAL call position, so a slot that fails
/// preflight and a slot that comes back from upstream land in the same
/// place they would have either way. `wave` carries original indices for
/// the same reason.
///
/// The wave's gate lease is the CALLER's: it is held across this whole
/// call, including each slot's settle, and released by RAII when the
/// caller drops it. The lease bounds the concurrent body memory a wave
/// may hold, which is why it spans collection rather than only
/// dispatch.
///
/// # Errors
/// [`FanoutInternalError::BatchContractViolation`] when upstream reports
/// `Completed` with a slot still unfilled, and
/// [`FanoutInternalError::Internal`] on a driver fault. auctioneer publishes
/// a wave whole or not at all, so either becomes a 500 rather than a
/// partial answer — even though the items already recorded are valid.
pub async fn run_wave(
    wave: Vec<(usize, BoundedOutboundRequest)>,
    out: &mut [Option<Slot>],
    batch_entry_at: MonotonicInstant,
    host: WaveHost<'_>,
    http: &dyn BoundedProxyClientApi,
    limits: &ResolvedIsolateLimits,
) -> Result<(), FanoutInternalError> {
    let clock = host.clock;
    let (pending, latest) = preflight(wave, out, batch_entry_at, clock, limits)?;
    // An all-invalid wave never constructs an upstream batch. Dispatching
    // an empty one would be a poll against a driver with nothing to
    // report.
    if pending.is_empty() {
        return Ok(());
    }

    let expected = pending.len();
    // The LATEST dispatched deadline. Upstream folds this cutoff into
    // every member's effective budget as the tighter operand, so anything
    // earlier would silently shorten the calls that were given longer —
    // see [`preflight`]. Each request still carries its own, earlier
    // absolute deadline: the cutoff bounds OBSERVATION, the per-request
    // deadline bounds the CALL.
    let cutoff = Deadline::at_instant(
        latest.ok_or_else(|| FanoutInternalError::Internal("empty wave cutoff".to_owned()))?,
    );
    let mut keys: Vec<(usize, ComposedDeadline, DecodePolicy)> = Vec::with_capacity(expected);
    let mut requests: Vec<OutboundRequest> = Vec::with_capacity(expected);
    for Pending {
        index,
        deadline,
        decode,
        request,
    } in pending
    {
        keys.push((index, deadline, decode));
        requests.push(request);
    }

    let mut batch = http
        .start_wave(requests, cutoff)
        .map_err(|_err| FanoutInternalError::Internal("outbound batch refused".to_owned()))?;
    let mut filled: Vec<bool> = vec![false; expected];
    let mut decoding = FuturesUnordered::new();
    let mut window_closed = false;

    // The observation loop. `batch.next()` and the decode futures are
    // polled together, so decoding one response never delays noticing a
    // sibling became terminal. The loop keeps polling `next()` after its
    // own tally is full, because `Finished(Completed)` arrives on the
    // poll AFTER the last item — inferring completion from a full tally
    // would skip the positional check entirely.
    let mut observing = true;
    // The instant the window was observed to close, captured at
    // observation rather than read later. See the `window_closed`
    // publication below for why that distinction is the whole point.
    let mut cutoff_at: Option<MonotonicInstant> = None;
    while observing {
        // Both halves are polled on every wake, SETTLERS first — see
        // `next_step` for why a ready settler cannot wait a pass while a
        // ready batch event can. Neither half is starved: both are
        // polled on the same wake, so the bias only decides the order
        // between two that are simultaneously ready.
        let step = next_step(&mut batch, &mut decoding).await;
        match step {
            Step::Decoded(index, slot) => place(out, index, slot)?,
            Step::Batch(Ok(OutboundBatchNext::Item(item))) => {
                // A resolved slot is never revisited or rewritten.
                let (slot_index, result) = item.into_parts();
                let (Some(seen), Some(&(index, deadline, decode))) =
                    (filled.get_mut(slot_index), keys.get(slot_index))
                else {
                    return Err(FanoutInternalError::Internal(
                        "batch slot index out of range".to_owned(),
                    ));
                };
                *seen = true;
                decoding.push(finish_slot(index, deadline, decode, result, host));
            }
            // Upstream resolved everything, so a hole is a real positional
            // breach. A RUNTIME check with a typed outer error — never a
            // debug assertion, never a zip that truncates, never an
            // expect.
            Step::Batch(Ok(OutboundBatchNext::Finished(OutboundBatchTermination::Completed))) => {
                let got = filled.iter().filter(|seen| **seen).count();
                if got != expected {
                    return Err(FanoutInternalError::BatchContractViolation { expected, got });
                }
                observing = false;
            }
            // The window closed. Unfilled slots become positional expiries
            // below and completed siblings are untouched: this is NOT a
            // contract violation.
            Step::Batch(Ok(OutboundBatchNext::Finished(OutboundBatchTermination::Cutoff))) => {
                window_closed = true;
                // THE CUTOFF INSTANT, read here. The expiries below are
                // published after the outstanding settlers drain, and a
                // clock read at that point would charge every unresolved
                // slot for however long a SIBLING took to settle — a
                // cutoff observed at 50 ms reported as 150 ms when one
                // sibling's settlement ran 100 ms. The window closed
                // when upstream said it closed; nothing auctioneer does
                // afterwards moves that instant.
                cutoff_at = Some(clock.now_monotonic());
                observing = false;
            }
            // A driver fault: a duplicate or out-of-range index, or a
            // premature end. The batch is poisoned and is not polled
            // again.
            Step::Batch(Err(())) => {
                return Err(FanoutInternalError::Internal(
                    "outbound batch driver failed".to_owned(),
                ));
            }
        }
    }

    // Observation is over; the remaining decodes still owe their slots.
    while let Some((index, slot)) = decoding.next().await {
        place(out, index, slot)?;
    }

    if window_closed {
        // Each hole becomes a positional expiry carrying the cause
        // selected at composition. `elapsed` measures the window this
        // slot actually spent upstream, which is what the caller's
        // `responsetimemillis` reports — so it is measured to the
        // instant the window CLOSED, captured when the cutoff was
        // observed, not to now. The settler drain above sits between the
        // two, and charging an unresolved slot for a sibling's
        // settlement would report a latency no call experienced.
        //
        // `window_closed` is only ever set alongside `cutoff_at`, so the
        // fallback is unreachable; reading the clock is the conservative
        // answer if it were, because it can only overstate.
        let closed_at = cutoff_at.unwrap_or_else(|| clock.now_monotonic());
        let elapsed = closed_at.saturating_duration_since(batch_entry_at);
        for (position, &(index, deadline, decode)) in keys.iter().enumerate() {
            if filled.get(position).copied().unwrap_or(false) {
                continue;
            }
            place(
                out,
                index,
                Slot {
                    result: BoundedSlotResult::expired(elapsed),
                    deadline,
                    decode,
                },
            )?;
        }
    }
    Ok(())
}

/// Publishes each due position as a positional expiry and removes it
/// from the pending set.
///
/// Removal is BACK TO FRONT so taking one entry does not shift the index
/// of the next, and `elapsed` is measured once by the caller from fanout
/// entry: every call in this batch expired at the same observed instant,
/// so reading the clock per entry would report a spread that does not
/// exist.
///
/// # Errors
/// Only what [`place`] reports — a positional collision — plus the
/// internal error a vanished position would be, which is a contract
/// breach rather than a normal outcome.
fn drain_expired(
    pending: &mut VecDeque<(usize, BoundedOutboundRequest)>,
    out: &mut [Option<Slot>],
    due: Vec<usize>,
    elapsed: Duration,
) -> Result<(), FanoutInternalError> {
    for position in due.into_iter().rev() {
        let Some((index, call)) = pending.remove(position) else {
            return Err(FanoutInternalError::Internal(
                "pending position vanished".to_owned(),
            ));
        };
        let decode = DecodePolicy::from(&call.policy);
        place(
            out,
            index,
            Slot {
                result: BoundedSlotResult::expired(elapsed),
                deadline: call.deadline,
                decode,
            },
        )?;
    }
    Ok(())
}

/// Converts every wave member, recording the failures in place.
///
/// Returns the calls that cleared conversion and the LATEST deadline
/// among THOSE CALLS ONLY.
///
/// Both halves of that sentence are load-bearing, because the value
/// becomes the batch's observation cutoff and upstream treats a cutoff
/// as two things at once: it bounds observation AND it is folded into
/// every member's effective dispatch budget as the tighter operand.
///
/// Upstream now names the parameter `observation_cutoff` and documents
/// the semantics directly: it "stops batch observation regardless of
/// any remaining per-request budget or later per-request deadline",
/// and "passing an earlier observation cutoff intentionally leaves
/// unresolved slots". Leaving slots unresolved is a deliberate use of
/// the parameter, NOT a signal that an early value is harmless — which
/// is precisely why this function returns the latest and not the
/// earliest.
///
/// Taking the MINIMUM would therefore shrink every sibling's budget to
/// the tightest member's, so a call given 50 ms and answering at 20 ms
/// would be refused at 10 ms because some other member of the same wave
/// had a 10 ms budget. Each request already carries its OWN absolute
/// deadline, set in [`to_outbound`], so the cutoff only has to be late
/// enough not to truncate anyone: the latest is exactly that bound, and
/// no later.
///
/// Restricting it to the calls that CLEARED conversion matters for the
/// same reason in reverse. A call that failed preflight is never
/// dispatched and already holds its terminal result, so letting its
/// deadline widen the window would hand the surviving siblings
/// observation time bought by a call that is not there — and, on the
/// minimum, letting it shrink the window would truncate siblings on
/// behalf of a call that was never sent.
///
/// # Errors
/// Only what [`place`] reports — a positional collision, which is a
/// contract breach rather than a conversion outcome.
fn preflight(
    wave: Vec<(usize, BoundedOutboundRequest)>,
    out: &mut [Option<Slot>],
    batch_entry_at: MonotonicInstant,
    clock: &dyn Clock,
    limits: &ResolvedIsolateLimits,
) -> Result<(Vec<Pending>, Option<MonotonicInstant>), FanoutInternalError> {
    let mut pending: Vec<Pending> = Vec::with_capacity(wave.len());
    let mut latest: Option<MonotonicInstant> = None;
    for (index, call) in wave {
        let deadline = call.deadline;
        // BEFORE the conversion, which consumes the call.
        let decode = DecodePolicy::from(&call.policy);
        match to_outbound(call, limits) {
            Ok(request) => {
                latest = Some(match latest {
                    Some(current) => current.max(deadline.deadline_at),
                    None => deadline.deadline_at,
                });
                pending.push(Pending {
                    index,
                    deadline,
                    decode,
                    request,
                });
            }
            Err(err) => {
                // The failure's elapsed is auctioneer-MEASURED from batch
                // entry. Zero is a legal reading, not a missing one: a
                // conversion that fails before the clock ticks genuinely
                // took no measurable time.
                let elapsed = clock
                    .now_monotonic()
                    .saturating_duration_since(batch_entry_at);
                place(
                    out,
                    index,
                    Slot {
                        result: BoundedSlotResult {
                            elapsed,
                            outcome: Err(err),
                        },
                        deadline,
                        decode,
                    },
                )?;
            }
        }
    }
    Ok((pending, latest))
}

/// One thing the observation loop can learn on a wake.
enum Step {
    /// The batch produced an item or a termination. Upstream's error is
    /// flattened to a unit: it is diagnostic detail about an invariant
    /// auctioneer cannot repair, every fault maps to the same outer 500, and
    /// keeping it would only invite classifying on it.
    Batch(Result<OutboundBatchNext, ()>),
    /// A decode finished, carrying the ORIGINAL index it belongs at.
    Decoded(usize, Slot),
}

/// Awaits whichever of the two halves is ready first, SETTLER-biased.
///
/// # Why the settlers go first
///
/// The two halves are not symmetric in what a delay costs them.
///
/// A ready batch event is a fact already established upstream: the item
/// carries its own elapsed figure and the termination carries its own
/// instant, so taking it one pass later changes nothing it reports.
///
/// A ready settler is not. [`finish_slot`] rechecks its slot's deadline
/// against the clock AFTER it resumes, so every pass it waits is time
/// charged to the call it is settling. Under a driver that produces
/// items without yielding — a synchronous or already-buffered one, which
/// is what the batch half looks like when upstream has several results
/// in hand — a batch-biased poll never reaches the settlers until the
/// batch runs dry. Measured, an unsupported-encoding response ready to
/// settle at 5 ms was held behind a sibling poll until 80 ms and then
/// rechecked past its 50 ms deadline: it published as
/// [`BoundedSlotError::Expired`] instead of
/// [`BoundedSlotError::Undecodable`].
///
/// That is an ERROR CLASSIFICATION defect, not a lost bid. The call had
/// no usable body either way — the encoding was one nothing here
/// implements — so no healthy bid was dropped. What was wrong is the
/// reason the operator reads: a timeout points at the bidder's latency,
/// and an encoding refusal points at its `Content-Encoding`. Acting on
/// the wrong one is the cost.
///
/// So the bias is the other way round, and the batch half is polled on
/// the same wake rather than on a later one — nothing is starved, the
/// order between two simultaneously-ready halves is just resolved in
/// favour of the one a delay would misreport.
async fn next_step<DecodeFuture>(
    batch: &mut OutboundBatch,
    decoding: &mut FuturesUnordered<DecodeFuture>,
) -> Step
where
    DecodeFuture: Future<Output = (usize, Slot)>,
{
    let mut batch_step = Box::pin(batch.next());
    poll_fn(|cx| {
        // An empty set reports `Ready(None)`, which must NOT be read as
        // "nothing left to do": the batch is still running, so fall
        // through to it rather than resolving.
        if let Poll::Ready(Some((index, slot))) = decoding.poll_next_unpin(cx) {
            return Poll::Ready(Step::Decoded(index, slot));
        }
        // Polled on the SAME wake, so a batch event ready now is taken
        // now whenever no settler was.
        if let Poll::Ready(step) = batch_step.as_mut().poll(cx) {
            return Poll::Ready(Step::Batch(step.map_err(|_err| ())));
        }
        Poll::Pending
    })
    .await
}

/// Lifts one upstream slot and settles the encoding it arrived in.
///
/// Returns the ORIGINAL index alongside the slot, because these futures
/// complete out of order and the position has to come from the item, not
/// from arrival.
///
/// # Why no decode runs here
///
/// `EdgeZero` decodes every coding either side implements — a single
/// bare `br`, `gzip`, or `deflate` — and strips `Content-Encoding` when
/// it does, so those bodies arrive as [`CapRow::ReadyToParse`] and owe
/// nothing. What it hands back encoded is only what no decoder can
/// finish: a stacked chain, a coding nothing here implements, or a
/// malformed header. Adding a decoder to this function would not change
/// any of those verdicts.
///
/// So a [`CapRow::RawPassthrough`] body is a REFUSAL, not deferred work.
/// It classifies as [`FanoutError::UndecodableResponse`] carrying the
/// cause upstream typed, and the cooperative yield below is the one
/// remaining piece of auctioneer-owned work on this path: the host round
/// trip that lets a frozen-clock runtime observe the lift's own CPU time
/// before the deadline recheck reads it.
///
/// [`DecodePolicy`] still travels with the [`Slot`]: it is the bound
/// auctioneer reports upstream and charges its isolate budget for, and the
/// slot is where that per-call figure is recoverable.
async fn finish_slot(
    index: usize,
    deadline: ComposedDeadline,
    decode: DecodePolicy,
    upstream: OutboundSlotResult,
    host: WaveHost<'_>,
) -> (usize, Slot) {
    let clock = host.clock;
    let mut result = super::lift_slot(upstream).await;
    // Read out before the outcome is rewritten below. The clone copies a
    // short coding token on a path that is already refusing the call;
    // mutating the row in place to dodge it would leave the live `Ok`
    // briefly describing a body it does not have.
    let undecodable = match &result.outcome {
        Ok(resp) => match &resp.cap_row {
            CapRow::RawPassthrough(cause) => Some(cause.clone()),
            CapRow::ReadyToParse => None,
        },
        Err(_) => None,
    };
    if let Some(cause) = undecodable {
        let settle_start = clock.now_monotonic();
        // Return to the host BEFORE reading the clock for the recheck.
        // On a runtime whose clock is frozen for the duration of a
        // synchronous turn, this slot's own CPU time is invisible until
        // something yields, so without this the comparison below would
        // read the instant the turn began and could never fire. On a
        // target with a live clock the yield is a no-op the cooperator
        // implements as such.
        host.cooperator.yield_to_host().await;
        // THE ONE recheck auctioneer owns, over work auctioneer itself performs.
        // The upstream phase's outcome is `EdgeZero`'s to report and is
        // never reinterpreted here. An expiry outranks the encoding
        // verdict: the call is out of time either way, and a timeout is
        // the fact the operator acts on.
        result.outcome = if clock.now_monotonic() >= deadline.deadline_at {
            Err(BoundedSlotError::Expired)
        } else {
            Err(BoundedSlotError::Undecodable(cause))
        };
        result.elapsed = result.elapsed.saturating_add(
            clock
                .now_monotonic()
                .saturating_duration_since(settle_start),
        );
    }
    (
        index,
        Slot {
            result,
            deadline,
            decode,
        },
    )
}

/// Writes one slot at its original position.
///
/// A position already occupied means two terminal results claimed the
/// same call, which is the same positional contract breach an unfilled
/// slot is.
fn place(out: &mut [Option<Slot>], index: usize, slot: Slot) -> Result<(), FanoutInternalError> {
    let total = out.len();
    let Some(cell) = out.get_mut(index) else {
        return Err(FanoutInternalError::BatchContractViolation {
            expected: total,
            got: index,
        });
    };
    if cell.is_some() {
        return Err(FanoutInternalError::Internal(
            "two terminal results for one call".to_owned(),
        ));
    }
    *cell = Some(slot);
    Ok(())
}

/// The positions in `pending` whose deadline has passed at `now`.
///
/// INCLUSIVE: `deadline_at <= now` is due. The race in `drive_fanout`
/// can resolve in the acquisition's favour on the very poll where the
/// timer is also due, and expired work is never dispatched.
///
/// A free function because the recheck runs after EVERY acquisition —
/// the raced one and the unbounded fallback both — and a recheck that
/// existed at only one of those sites is exactly the defect this shape
/// prevents.
fn due_positions(
    pending: &VecDeque<(usize, BoundedOutboundRequest)>,
    now: MonotonicInstant,
) -> Vec<usize> {
    pending
        .iter()
        .enumerate()
        .filter(|entry| entry.1.1.deadline.deadline_at <= now)
        .map(|(position, _entry)| position)
        .collect()
}

/// Races that may resolve for the timer without producing either a gate
/// lease or an expiry before the wait stops being raced at all.
///
/// A productive race either wins permits or finds a call due; anything
/// else made no progress. Under a real host timer the count never gets
/// near this, because a timer that fired means its instant passed and the
/// recheck finds the call due. Under an immediate-resolve stub every race
/// is unproductive, so the bound is what turns an unbounded spin into a
/// documented `BestEffort` wait.
const MAX_UNPRODUCTIVE_RACES: u32 = 8;

/// The gate width one wave may claim.
///
/// `min(gate capacity, remaining calls)`, so a wave is never wider than
/// the gate could grant — an over-wide acquisition can only fail — and
/// never wider than there is work for.
#[must_use]
pub fn wave_width(gate_capacity: u32, remaining: usize) -> u32 {
    // A call count beyond `u32::MAX` saturates rather than wrapping: the
    // gate capacity clamps it back down on the next line anyway, and a
    // wrap would silently ask for a narrower wave than there is work for.
    let wanted = u32::try_from(remaining).unwrap_or(u32::MAX);
    gate_capacity.min(wanted)
}

/// The relative sleep that arms a race against `instant`.
///
/// auctioneer's `Timer` takes a relative `Duration`, so the value is derived
/// at the moment of arming rather than stored: storing a remaining
/// duration and wrapping it later would let the time in between silently
/// extend the deadline.
#[must_use]
pub fn remaining_until(instant: MonotonicInstant, now: MonotonicInstant) -> Duration {
    instant.saturating_duration_since(now)
}

/// Maps one terminal slot onto the exchange's per-bidder taxonomy.
///
/// Exhaustive over [`BoundedSlotError`] with no wildcard: a new failure
/// owner is a review item, not something to silently fold into `Other`.
/// `selected_cause` is auctioneer's own, stored at composition, and is what
/// every timeout classifies through — the upstream error carries a cause
/// of its own but it describes the platform's budget, not which of
/// auctioneer's two budgets was the smaller one.
fn classify_slot(
    outcome: Result<BoundedProxyResponse, BoundedSlotError>,
    selected_cause: TimeoutCause,
    max_response_bytes: u64,
) -> Result<BidderHttpResponse, FanoutError> {
    match outcome {
        Ok(response)
            if u64::try_from(response.body.len()).unwrap_or(u64::MAX) > max_response_bytes =>
        {
            Err(FanoutError::OversizeResponse(
                OversizeOrigin::CallResponseCap,
            ))
        }
        // A status that cannot carry an auction answer is refused HERE,
        // before the body is lifted into the shape an adapter parses.
        // The predicate is shared with the legacy classifier rather
        // than restated, so the two paths cannot drift: a 3xx admitted
        // on one and refused on the other is exactly the split this
        // guard exists to prevent.
        Ok(response) if !is_biddable_status(response.status) => Err(FanoutError::BadServerResponse),
        Ok(response) => Ok(response.into_bidder_response()),
        // auctioneer's own clock, over auctioneer's own work.
        Err(BoundedSlotError::Expired) => Err(FanoutError::Timeout(selected_cause)),
        // Nothing was sent, so neither of these is a transport fact
        // about the bidder — but each is still this bidder's positional
        // failure, and they are DIFFERENT failures with different wire
        // codes. A request auctioneer could not represent is code 4; a
        // request auctioneer built and had no budget to carry is code 5.
        // Folding both into `Other` rendered 999, which is reserved for
        // a genuine fan-out transport fault.
        Err(BoundedSlotError::Preflight) => Err(FanoutError::Preflight),
        Err(BoundedSlotError::OversizeEncodedRequest) => Err(FanoutError::OversizeRequest {
            stage: OversizeRequestStage::Encoded,
        }),
        // The bidder's own answer, in a codec nothing here reads. The
        // cause passes through unchanged: it is already auctioneer's neutral
        // classification, made once where upstream's typed reason was in
        // hand.
        Err(BoundedSlotError::Undecodable(cause)) => Err(FanoutError::UndecodableResponse(cause)),
        Err(BoundedSlotError::Edge(err)) => classify_edge(&err, selected_cause),
    }
}

/// Maps the platform's response-limit reason onto auctioneer's own origin.
///
/// [`ResponseLimitReason`] is `#[non_exhaustive]`, so the wildcard arm is
/// unavoidable. It deliberately lands on
/// [`OversizeOrigin::Unclassified`] rather than on the nearest named
/// class: a reason a future platform release adds must read as "a size
/// ceiling auctioneer cannot name", never as a class it is not. `Unspecified`
/// takes the same answer, because that is precisely what upstream means
/// by it.
///
/// The named reasons are listed individually rather than grouped so that
/// a rename upstream is a compile error here.
fn oversize_origin(reason: ResponseLimitReason) -> OversizeOrigin {
    match reason {
        ResponseLimitReason::BrotliWindow => OversizeOrigin::BrotliWindow,
        ResponseLimitReason::BufferedBody => OversizeOrigin::BufferedBody,
        ResponseLimitReason::DecodedBody => OversizeOrigin::DecodedBody,
        ResponseLimitReason::DecoderMemory => OversizeOrigin::DecoderMemory,
        ResponseLimitReason::EncodedBody => OversizeOrigin::EncodedBody,
        ResponseLimitReason::HeaderBytes => OversizeOrigin::HeaderBytes,
        ResponseLimitReason::HeaderCount => OversizeOrigin::HeaderCount,
        ResponseLimitReason::Unspecified | _ => OversizeOrigin::Unclassified,
    }
}

/// Maps one upstream failure onto the exchange's taxonomy.
///
/// Matches on the VARIANT, never on a substring of the message: the
/// message is untrusted upstream text and must not reach the wire or the
/// log, so every arm below returns a fixed category.
fn classify_edge(
    err: &EdgeError,
    selected_cause: TimeoutCause,
) -> Result<BidderHttpResponse, FanoutError> {
    match *err {
        // Upstream observed the expiry itself; auctioneer's stored cause is
        // what names WHICH budget it was.
        EdgeError::GatewayTimeout { .. } | EdgeError::RequestTimeout { .. } => {
            Err(FanoutError::Timeout(selected_cause))
        }
        // The reason names WHICH ceiling upstream enforced; it is a typed
        // field, not a parsed message, so carrying it forward leaks no
        // upstream text.
        EdgeError::ResponseTooLarge { reason, .. } => {
            Err(FanoutError::OversizeResponse(oversize_origin(reason)))
        }
        // A refused REQUEST header block, which shares the oversize wire
        // contract but not the cause: nothing was read back, so this is
        // auctioneer's composition being too big rather than the bidder's
        // answer. It gets its own origin instead of borrowing a response
        // class it has no claim to.
        EdgeError::RequestHeaderFieldsTooLarge { .. } => Err(FanoutError::OversizeResponse(
            OversizeOrigin::RequestHeaders,
        )),
        // A 502 whose TYPED reason says the upstream was never reached:
        // the adapters classify connect and DNS failures as
        // `Unreachable`, and that is a different wire fact from a bidder
        // that answered badly — 999 / statuscode 103 rather than 4 /
        // 100. Read off the typed field, never off the message, so the
        // distinction costs no upstream text.
        EdgeError::BadGateway {
            reason: BadGatewayReason::Unreachable,
            ..
        } => Err(FanoutError::Transport(TransportError {
            kind: TransportErrorKind::Unreachable,
        })),
        // A protocol or decode failure describes a received answer.
        // Initial, unspecified and future transport failures stay distinct.
        EdgeError::BadGateway {
            reason: BadGatewayReason::Decode(_) | BadGatewayReason::Protocol,
            ..
        } => Err(FanoutError::BadServerResponse),
        // Everything else is an upstream fault auctioneer cannot refine. The
        // arms are named rather than wildcarded so a new upstream variant
        // is a compile-time review item, and the trailing `_` is only
        // there because `EdgeError` is `#[non_exhaustive]`.
        EdgeError::BadGateway { .. }
        | EdgeError::ServiceUnavailable { .. }
        | EdgeError::NotFound { .. }
        | EdgeError::BadRequest { .. }
        | EdgeError::ConfigOutOfDate { .. }
        | EdgeError::Internal { .. }
        | EdgeError::MethodNotAllowed { .. }
        | EdgeError::NotImplemented { .. }
        | EdgeError::StoreExtraction { .. }
        | EdgeError::UriTooLong { .. }
        | EdgeError::Validation { .. }
        | _ => {
            // The untrusted upstream string goes NOWHERE: not the wire,
            // not the log. Scrubbing it is bypassable, so the only safe
            // rule is to emit the fixed category and drop the free text.
            log::warn!(target: "auctioneer::fanout", "bidder outbound error");
            Err(FanoutError::Transport(TransportError {
                kind: TransportErrorKind::Initial,
            }))
        }
    }
}

/// Splits composition's output into the calls that still need a wave
/// and the refusals, publishing the latter where they belong.
///
/// A refusal never enters the pending set, because nothing about it is
/// waiting on a gate lease — it already holds its terminal result, and
/// queueing it would spend a permit on a call that will not be sent.
/// Its `elapsed` is zero: composition runs before the first clock read a
/// dispatch would take, so zero is the honest reading rather than a
/// missing one.
///
/// # Errors
/// Only what [`place`] reports — a positional collision, which is a
/// contract breach rather than a composition outcome.
fn split_composed(
    composed: Vec<Composed>,
    out: &mut [Option<Slot>],
) -> Result<VecDeque<(usize, BoundedOutboundRequest)>, FanoutInternalError> {
    let mut pending: VecDeque<(usize, BoundedOutboundRequest)> =
        VecDeque::with_capacity(composed.len());
    for (index, entry) in composed.into_iter().enumerate() {
        match entry {
            Composed::Dispatch(request) => pending.push_back((index, *request)),
            Composed::Refused {
                error,
                deadline,
                decode,
            } => place(
                out,
                index,
                Slot {
                    result: BoundedSlotResult {
                        elapsed: Duration::ZERO,
                        outcome: Err(error),
                    },
                    deadline,
                    decode,
                },
            )?,
        }
    }
    Ok(pending)
}

/// Composes, then runs waves under gate leases until the call list drains.
///
/// The wave loop is the reason the gate exists: the upstream batch is one
/// atomic dispatch that never consults it, so acquiring permits one at a
/// time around a batch would deadlock at capacity and passing a batch
/// wider than the lease would bypass the cap. Every wave is
/// all-or-nothing and exactly one batch wide.
///
/// Members are the FIRST `k` pending calls in ORIGINAL INPUT ORDER, not
/// earliest-deadline-first: reordering would change which bidders expire
/// while waiting, which is a different auction.
///
/// # Errors
/// Whatever [`run_wave`] or [`compose_all`] reports, plus
/// [`FanoutInternalError::BatchContractViolation`] if publication finds
/// an unfilled position — the same positional breach, observed one layer
/// out.
pub async fn drive_fanout(
    calls: Vec<BidderCall>,
    auction: &AuctionDeadline,
    host: WaveHost<'_>,
    http: &dyn BoundedProxyClientApi,
    limits: &ResolvedIsolateLimits,
    gate: &Arc<StreamGate>,
) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
    let clock = host.clock;
    let fanout_entry_at = clock.now_monotonic();
    // The encoded ceiling is a `u64` in the envelope and a buffer
    // length here; on a 32-bit wasm target a cap above `usize` saturates
    // to the widest allocation this isolate could make, which is the
    // tightest honest bound available.
    let max_encoded_bytes =
        usize::try_from(limits.max_outbound_request_bytes).unwrap_or(usize::MAX);
    let composed = compose_all(
        calls,
        auction,
        fanout_entry_at,
        policy_from_limits(limits),
        max_encoded_bytes,
    )?;
    let total = composed.len();
    let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(total).collect();
    let mut pending = split_composed(composed, &mut out)?;
    let mut unproductive_races: u32 = 0;

    while !pending.is_empty() {
        // Composition and earlier waves can consume a pending budget.
        // Sweep before creating either acquisition arm, using this same
        // instant for the remaining duration so no due call arms sleep(0).
        let pre_acquire_at = clock.now_monotonic();
        let pre_acquire_due = due_positions(&pending, pre_acquire_at);
        if !pre_acquire_due.is_empty() {
            let elapsed = pre_acquire_at.saturating_duration_since(fanout_entry_at);
            drain_expired(&mut pending, &mut out, pre_acquire_due, elapsed)?;
            continue;
        }
        let earliest = pending
            .iter()
            .map(|entry| entry.1.deadline.deadline_at)
            .min()
            .ok_or_else(|| FanoutInternalError::Internal("empty pending set".to_owned()))?;
        let width = wave_width(gate.capacity(), pending.len());
        if width == 0 {
            return Err(FanoutInternalError::Internal(
                "stream gate admits no calls".to_owned(),
            ));
        }

        // Race the acquisition against the earliest pending deadline, so a
        // wave that cannot get permits in time emits its positional
        // timeouts instead of holding the whole auction behind the queue.
        let acquired = {
            let acquire = gate.acquire_wave(width);
            let sleep = host.timer.sleep(remaining_until(earliest, pre_acquire_at));
            match select(Box::pin(acquire), Box::pin(sleep)).await {
                Either::Left((lease, _sleep)) => Some(lease.map_err(|_err| {
                    FanoutInternalError::Internal("wave exceeds gate capacity".to_owned())
                })?),
                // The deadline won. Dropping the acquisition releases
                // nothing, by the gate's own cancellation contract.
                Either::Right(((), _acquire)) => None,
            }
        };

        // The recheck, run whether or not the lease was won.
        let now = clock.now_monotonic();
        let due = due_positions(&pending, now);
        if !due.is_empty() {
            // Dropping the lease here is the point: an expired wave gives
            // its permits back rather than spending them.
            drop(acquired);
            let elapsed = now.saturating_duration_since(fanout_entry_at);
            drain_expired(&mut pending, &mut out, due, elapsed)?;
            continue;
        }

        // No lease means the timer won but the recheck found nothing due,
        // so no call has an outcome yet. Re-racing immediately would be a
        // busy loop under a timer that resolves instantly — which is what
        // an immediate-resolve stub does, and what the three WASM adapters
        // install today — so the re-race is BOUNDED: after
        // `MAX_UNPRODUCTIVE_RACES` passes that produced neither a lease
        // nor an expiry, the wait is served by awaiting the acquisition
        // alone. Spinning the isolate instead would be strictly worse.
        let lease = if let Some(lease) = acquired {
            lease
        } else {
            unproductive_races = unproductive_races.saturating_add(1);
            if unproductive_races <= MAX_UNPRODUCTIVE_RACES {
                continue;
            }
            // THE UNBOUNDED WAIT. Nothing races it, so it can return
            // arbitrarily late — and the call deadlines it was supposed
            // to protect have been passing the whole time it waited.
            let fallback = gate.acquire_wave(width).await.map_err(|_err| {
                FanoutInternalError::Internal("wave exceeds gate capacity".to_owned())
            })?;
            // So RECHECK, here, after it. Without this the loop fell
            // straight into `run_wave` with whatever the wait had made
            // stale: measured, an 80 ms queue wait dispatched a call
            // against a 50 ms deadline, and because `batch_entry_at` is
            // read AFTER the acquisition the slot reported 0 ms spent.
            // Both halves of that are fixed by rechecking before the
            // batch starts rather than after it.
            //
            // Every acquisition is followed by a recheck now. Reaching
            // `run_wave` is not evidence a call was sent in time; only
            // the recheck is.
            let now_after_wait = clock.now_monotonic();
            let due_after_wait = due_positions(&pending, now_after_wait);
            if !due_after_wait.is_empty() {
                drop(fallback);
                let elapsed = now_after_wait.saturating_duration_since(fanout_entry_at);
                drain_expired(&mut pending, &mut out, due_after_wait, elapsed)?;
                continue;
            }
            fallback
        };
        unproductive_races = 0;

        let take = usize::try_from(lease.weight()).unwrap_or(pending.len());
        let wave: Vec<(usize, BoundedOutboundRequest)> =
            pending.drain(..take.min(pending.len())).collect();
        let batch_entry_at = clock.now_monotonic();
        run_wave(wave, &mut out, batch_entry_at, host, http, limits).await?;
        // Explicit, so the release point — after the batch AND after
        // auctioneer's own decode — is visible rather than implied by scope.
        drop(lease);
    }

    publish_slots(out)
}

// Every slot is terminal here. Reading the clock during publication would
// wrongly attribute the last wave's duration to earlier completed slots.
fn publish_slots(out: Vec<Option<Slot>>) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
    let total = out.len();
    let mut results: Vec<BidderCallResult> = Vec::with_capacity(total);
    for entry in out {
        let Some(Slot {
            result,
            deadline,
            decode,
        }) = entry
        else {
            return Err(FanoutInternalError::BatchContractViolation {
                expected: total,
                got: results.len(),
            });
        };
        results.push(BidderCallResult {
            elapsed: result.elapsed,
            result: classify_slot(
                result.outcome,
                deadline.selected_cause,
                decode.max_decoded_bytes,
            ),
        });
    }
    Ok(results)
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::test_support::outbound_request;

    use super::{
        BoundedOutboundRequest, Composed, MAX_UNPRODUCTIVE_RACES, Slot, WaveHost, classify_edge,
        classify_slot, compose_all, drive_fanout, poll_fn, remaining_until, run_wave, wave_width,
    };
    use crate::outbound::{
        BoundedProxyClientApi, BoundedProxyResponse, BoundedSlotError, CapRow, DecodePolicy,
        MAX_CHUNK_BYTES, MAX_RESPONSE_HEADER_BYTES, MAX_RESPONSE_HEADER_COUNT, ResponseBodyPolicy,
    };
    use async_trait::async_trait;
    use auctioneer_bidder::{
        BidderHttpRequest, FinalizeError, TransportFinalize, finalize_for_transport,
    };
    use auctioneer_exchange::fanout::{TransportError, TransportErrorKind};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, Compression, FanoutError, FanoutInternalError,
        OversizeRequestStage, TimeoutCause, TmaxSource, UndecodableEncoding, fanout_error_codes,
    };
    use auctioneer_runtime::{
        Clock as _, DecodeCooperator, FakeClock, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR,
        MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR, MonotonicInstant,
        NoopDecodeCooperator, ResolvedIsolateLimits, StreamGate, Timer,
    };
    use bytes::Bytes;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::error::{BadGatewayDecodeReason, BadGatewayReason, EdgeError};
    use edgezero_core::http::{HeaderMap, Method, StatusCode};
    use edgezero_core::outbound::{
        ContentTypeProvenance, HttpClient, IndependentOutboundProgress, OutboundBatch,
        OutboundBatchDriverEvent as RawBatchEvent, OutboundBatchNext, OutboundBatchTermination,
        OutboundCollectedSlot, OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture,
        OutboundHead, OutboundHeaderFidelity, OutboundHeaderInput, OutboundHttpClient,
        OutboundPending, OutboundPreparedBatch, OutboundProgressRuntime, OutboundRequest,
        OutboundResponse, OutboundSendMode, OutboundSlotResult, OutboundTime,
        ResponseHeaderLimiter,
    };
    use edgezero_core::time::{Deadline, MonotonicClock};
    use flate2::read::GzDecoder;
    use futures::executor::block_on;
    use futures::stream;
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::io::Read as _;
    use std::iter::repeat_with;
    use std::mem;
    use std::pin::Pin;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;
    use std::vec::IntoIter;

    pub(crate) fn test_limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 512 * 1024 * 1024,
            baseline_bytes: 0,
            inbound_request_bytes: 256 * 1024,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: 256 * 1024,
            max_outbound_raw_bytes: 256 * 1024,
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 256 * 1024,
            egress_permits: 4,
            response_write_deadline_ms: 10_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 2,
            aux_route_peak_bytes: 1024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 1,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 8,
            max_outbound_requests: 8,
            max_concurrent_auctions: 4,
            max_active_body_streams: 4,
            max_runtime_config_bytes: 1024,
            max_runtime_config_blob_bytes: 1024,
            max_parsed_nodes: 1024,
            max_copied_header_bytes: 1024,
            max_admitted_header_bytes: 8192,
            max_admitted_header_count: 64,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 2,
            refusal_body_bytes: 1024,
            transmit_state_bytes: 1024,
        }
    }

    /// The host bundle every row here drives the scheduler with.
    ///
    /// `FakeClock` is the timer too, and the no-op cooperator is correct
    /// for it: the fake clock is not turn-frozen, so a test that wants
    /// decode time to pass advances it directly.
    fn host(clock: &FakeClock) -> WaveHost<'_> {
        WaveHost {
            clock,
            timer: clock,
            cooperator: &NoopDecodeCooperator,
        }
    }

    fn policy() -> ResponseBodyPolicy {
        ResponseBodyPolicy {
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            max_decoder_bytes: 20_971_520,
            max_header_bytes: MAX_RESPONSE_HEADER_BYTES,
            max_header_count: MAX_RESPONSE_HEADER_COUNT,
            max_chunk_bytes: MAX_CHUNK_BYTES,
        }
    }

    /// The encoded ceiling the finalization rows charge against.
    ///
    /// Matches the shipped outbound request cap, and is wide enough
    /// that no row trips it except the one that means to.
    const TEST_MAX_ENCODED: usize = 256 * 1024;

    fn request(url: &str) -> BidderHttpRequest {
        outbound_request::request("POST".into(), url.into(), vec![], b"{}".to_vec(), vec![])
            .expect("sink-built fixture")
    }

    fn call(url: &str, timeout_ms: u64) -> BidderCall {
        BidderCall {
            request: request(url),
            timeout: Duration::from_millis(timeout_ms),
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
        }
    }

    fn invalid_header_call(timeout_ms: u64) -> BidderCall {
        BidderCall {
            request: outbound_request::request(
                "POST".into(),
                "https://invalid.example/rtb".into(),
                vec![("bad header name".into(), "v".into())],
                b"{}".to_vec(),
                vec![],
            )
            .unwrap(),
            timeout: Duration::from_millis(timeout_ms),
            max_transport_bytes: 256 * 1024,
            max_decoded_bytes: 256 * 1024,
        }
    }

    fn auction(clock: &FakeClock, millis: u64) -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: clock
                .origin()
                .checked_add(Duration::from_millis(millis))
                .expect("the test horizon is representable"),
            source: TmaxSource::Request,
        }
    }

    /// The scheduler's own clock, in `EdgeZero`'s monotonic domain.
    ///
    /// A canned response carries deferred body deadlines, and upstream
    /// reads them off the clock handed to `OutboundResponse::new`. Rows
    /// here advance the fake clock to drive the wave, so the response
    /// has to read that same source rather than process-default real
    /// time.
    fn monotonic(clock: &Arc<FakeClock>) -> MonotonicClock {
        let source = Arc::clone(clock);
        MonotonicClock::new(move || source.now_monotonic())
    }

    fn ok_response(clock: &Arc<FakeClock>) -> OutboundResponse {
        OutboundResponse::new(
            Method::POST,
            StatusCode::OK,
            HeaderMap::new(),
            Body::from_bytes(b"{}".to_vec()),
            monotonic(clock),
            edgezero_core::ContentTypeProvenance::Unknown,
        )
    }

    fn ok_slot(clock: &Arc<FakeClock>) -> OutboundSlotResult {
        OutboundSlotResult::new(Duration::ZERO, Ok(ok_response(clock)))
    }

    struct ResponseRecipe {
        status: StatusCode,
        headers: HeaderMap,
        body: Bytes,
        provenance: ContentTypeProvenance,
    }

    struct SlotRecipe {
        elapsed: Duration,
        outcome: Result<ResponseRecipe, EdgeError>,
    }

    struct ScriptItem {
        index: usize,
        result: SlotRecipe,
    }

    impl ScriptItem {
        fn new(index: usize, result: OutboundSlotResult) -> Self {
            let outcome = result.outcome.map(|response| {
                let (_, status, headers, body, provenance) = response.into_parts();
                ResponseRecipe {
                    status,
                    headers,
                    body: body
                        .into_bytes()
                        .expect("wave scripts carry buffered bodies"),
                    provenance,
                }
            });
            Self {
                index,
                result: SlotRecipe {
                    elapsed: result.elapsed,
                    outcome,
                },
            }
        }
    }

    enum ScriptEvent {
        Item(ScriptItem),
        Failed(EdgeError),
        Cutoff,
    }

    struct ScriptProvider {
        clock: Arc<FakeClock>,
        script: Mutex<Option<DriverScript>>,
        per_event: Duration,
        stage_events: bool,
    }

    impl OutboundHttpClient for ScriptProvider {
        fn begin(&self, _request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            Err(EdgeError::internal(anyhow::anyhow!(
                "wave scripts only submit batches"
            )))
        }

        fn dispatch_head(&self, _request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            Box::pin(async {
                Err(EdgeError::internal(anyhow::anyhow!(
                    "wave scripts only submit batches"
                )))
            })
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, batch: OutboundPreparedBatch) -> OutboundBatch {
            let (context, slots) = batch.into_slots();
            let script = self
                .script
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take()
                .expect("each wave owns one script");
            let mut requests = slots
                .into_iter()
                .map(|(index, request)| (index, Some(request)))
                .collect::<Vec<_>>();
            let started_at = context.started_at();
            let events = script
                .events
                .into_iter()
                .map(|event| match event {
                    ScriptEvent::Cutoff => RawBatchEvent::Cutoff,
                    ScriptEvent::Failed(error) => RawBatchEvent::Failed(error),
                    ScriptEvent::Item(item) => {
                        let completed_at = started_at
                            .checked_add(item.result.elapsed)
                            .expect("the scripted completion is representable");
                        let prepared = requests
                            .iter_mut()
                            .find(|(index, _)| *index == item.index)
                            .and_then(|(_, request)| request.take());
                        let outcome = item.result.outcome.and_then(|response| {
                            let request = prepared.ok_or_else(|| {
                                EdgeError::internal(anyhow::anyhow!(
                                    "scripted duplicate or out-of-range response"
                                ))
                            })?;
                            let (parts, response_context) = request.into_parts();
                            let (headers, _, _) = OutboundHeaderInput::new(
                                "wave_fixture",
                                response.headers,
                                OutboundHeaderFidelity::ProviderFieldBoundariesUnknown,
                            )
                            .normalize(
                                ResponseHeaderLimiter::new(
                                    parts.max_response_header_bytes,
                                    parts.max_response_header_count,
                                ),
                                &parts.method,
                                response.status,
                                parts.proxy_marker,
                            )?;
                            Ok(OutboundResponse::new(
                                parts.method,
                                response.status,
                                headers,
                                Body::from_bytes(response.body),
                                response_context.clock().clone(),
                                response.provenance,
                            ))
                        });
                        RawBatchEvent::Item {
                            index: item.index,
                            completed_at,
                            outcome,
                        }
                    }
                })
                .collect::<Vec<_>>();
            let mut driver = FinalizingDriver {
                events: events.into_iter(),
                control: script.control,
                clock: Arc::clone(&self.clock),
                cutoff: context.observation_cutoff(),
                per_event: self.per_event,
                stage_events: self.stage_events,
                yield_before_event: false,
            };
            OutboundBatch::from_driver(
                context,
                stream::poll_fn(move |cx| driver.poll(cx)),
                OutboundExchangeOwner::new(|| {}),
            )
        }

        fn time(&self) -> OutboundTime {
            let source = Arc::clone(&self.clock);
            OutboundTime::new(monotonic(&self.clock), move |deadline, clock| {
                let sleeper = Arc::clone(&source);
                Box::pin(async move {
                    sleeper
                        .sleep(deadline.remaining_at(clock.now()).unwrap_or_default())
                        .await;
                    Ok(())
                })
            })
        }

        fn validate_supported_options(
            &self,
            _request: &OutboundRequest,
            _mode: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Ok(())
        }
    }

    fn prepared_driver(
        requests: Vec<OutboundRequest>,
        cutoff: Deadline,
        clock: &Arc<FakeClock>,
        script: DriverScript,
        per_event: Duration,
        stage_events: bool,
    ) -> OutboundBatch {
        HttpClient::with_client(ScriptProvider {
            clock: Arc::clone(clock),
            script: Mutex::new(Some(script)),
            per_event,
            stage_events,
        })
        .start_batch_until(requests, cutoff)
    }

    fn prepared_batch(
        requests: Vec<OutboundRequest>,
        cutoff: Deadline,
        clock: &Arc<FakeClock>,
        events: Vec<ScriptEvent>,
    ) -> OutboundBatch {
        prepared_driver(
            requests,
            cutoff,
            clock,
            DriverScript {
                events,
                control: Arc::new(DriverControl::new(true)),
            },
            Duration::ZERO,
            false,
        )
    }

    /// Serves a scripted event sequence, counting dispatches and how
    /// many requests each one carried.
    struct ScriptedClient {
        clock: Arc<FakeClock>,
        events: Cell<Option<Vec<ScriptEvent>>>,
        dispatched: Cell<usize>,
        slot_count: Cell<usize>,
    }

    impl ScriptedClient {
        fn new(clock: &Arc<FakeClock>, events: Vec<ScriptEvent>) -> Self {
            Self {
                clock: Arc::clone(clock),
                events: Cell::new(Some(events)),
                dispatched: Cell::new(0),
                slot_count: Cell::new(0),
            }
        }
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for ScriptedClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            self.dispatched.set(self.dispatched.get().saturating_add(1));
            self.slot_count.set(requests.len());
            let events = self.events.take().unwrap_or_default();
            Ok(prepared_batch(
                requests,
                observation_cutoff,
                &self.clock,
                events,
            ))
        }
    }

    struct SharedCell<T>(Mutex<T>);

    impl<T> SharedCell<T> {
        const fn new(value: T) -> Self {
            Self(Mutex::new(value))
        }
        fn borrow(&self) -> MutexGuard<'_, T> {
            self.0.lock().unwrap_or_else(PoisonError::into_inner)
        }
        fn borrow_mut(&self) -> MutexGuard<'_, T> {
            self.borrow()
        }
        fn set(&self, value: T) {
            *self.borrow() = value;
        }
    }

    impl<T: Copy> SharedCell<T> {
        fn get(&self) -> T {
            *self.borrow()
        }
    }

    impl<T: Default> SharedCell<T> {
        fn take(&self) -> T {
            mem::take(&mut *self.borrow())
        }
    }

    /// Counts source polls independently of the batch's finalization guards.
    struct DriverControl {
        polls: SharedCell<usize>,
        items: SharedCell<usize>,
        pending_polls: SharedCell<usize>,
        drops: SharedCell<usize>,
        eof_ready: SharedCell<bool>,
        waker: SharedCell<Option<Waker>>,
    }

    impl DriverControl {
        const fn new(eof_ready: bool) -> Self {
            Self {
                polls: SharedCell::new(0),
                items: SharedCell::new(0),
                pending_polls: SharedCell::new(0),
                drops: SharedCell::new(0),
                eof_ready: SharedCell::new(eof_ready),
                waker: SharedCell::new(None),
            }
        }

        fn release_eof(&self) {
            self.eof_ready.set(true);
            if let Some(waker) = self.waker.take() {
                waker.wake();
            }
        }

        fn assert_counts(&self, polls: usize, items: usize, drops: usize) {
            assert_eq!(self.polls.get(), polls, "driver source polls");
            assert_eq!(self.items.get(), items, "driver item events delivered");
            assert_eq!(self.drops.get(), drops, "driver source drops");
        }
    }

    struct DriverScript {
        events: Vec<ScriptEvent>,
        control: Arc<DriverControl>,
    }

    struct FinalizingDriver {
        events: IntoIter<RawBatchEvent>,
        control: Arc<DriverControl>,
        clock: Arc<FakeClock>,
        cutoff: Deadline,
        per_event: Duration,
        stage_events: bool,
        yield_before_event: bool,
    }

    impl FinalizingDriver {
        fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Option<RawBatchEvent>> {
            self.control
                .polls
                .set(self.control.polls.get().saturating_add(1));
            if self.yield_before_event {
                self.yield_before_event = false;
                self.control
                    .pending_polls
                    .set(self.control.pending_polls.get().saturating_add(1));
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            if let Some(mut event) = self.events.next() {
                self.clock.advance(self.per_event);
                match &mut event {
                    RawBatchEvent::Item { completed_at, .. } => {
                        if self.per_event.is_zero() {
                            self.clock.advance_to(*completed_at);
                        } else {
                            *completed_at = self.clock.now_monotonic();
                        }
                        self.control
                            .items
                            .set(self.control.items.get().saturating_add(1));
                    }
                    RawBatchEvent::Cutoff => self.clock.advance_to(self.cutoff.instant()),
                    RawBatchEvent::Failed(_) => {}
                }
                self.yield_before_event = self.stage_events;
                return Poll::Ready(Some(event));
            }
            if self.control.eof_ready.get() {
                return Poll::Ready(None);
            }
            self.control
                .pending_polls
                .set(self.control.pending_polls.get().saturating_add(1));
            *self.control.waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for FinalizingDriver {
        fn drop(&mut self) {
            self.control
                .drops
                .set(self.control.drops.get().saturating_add(1));
            drop(self.control.waker.take());
        }
    }

    /// Each wave owns a source with observable EOF, polling and teardown.
    struct FinalizingClient {
        clock: Arc<FakeClock>,
        scripts: RefCell<VecDeque<DriverScript>>,
        dispatched: RefCell<Vec<usize>>,
    }

    impl FinalizingClient {
        fn new(clock: &Arc<FakeClock>, scripts: Vec<DriverScript>) -> Self {
            Self {
                clock: Arc::clone(clock),
                scripts: RefCell::new(scripts.into()),
                dispatched: RefCell::new(Vec::new()),
            }
        }
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for FinalizingClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            self.dispatched.borrow_mut().push(requests.len());
            let script = self
                .scripts
                .borrow_mut()
                .pop_front()
                .expect("each dispatched wave has a driver script");
            Ok(prepared_driver(
                requests,
                observation_cutoff,
                &self.clock,
                script,
                Duration::ZERO,
                false,
            ))
        }
    }

    fn terminal_item(clock: &Arc<FakeClock>, index: usize, elapsed_millis: u64) -> ScriptEvent {
        ScriptEvent::Item(ScriptItem::new(
            index,
            OutboundSlotResult::new(
                Duration::from_millis(elapsed_millis),
                Ok(ok_response(clock)),
            ),
        ))
    }

    fn completed_pair(clock: &Arc<FakeClock>) -> Vec<ScriptEvent> {
        vec![terminal_item(clock, 1, 20), terminal_item(clock, 0, 10)]
    }

    fn assert_gate_is_idle(gate: &StreamGate) {
        assert_eq!(gate.free(), gate.capacity(), "all stream permits returned");
        assert_eq!(gate.in_flight(), 0, "no live wave leases");
        assert_eq!(gate.granted(), 0, "no unclaimed grants");
        assert_eq!(gate.waiting_len(), 0, "no waiting wave");
        assert_eq!(gate.node_count(), 0, "no retained acquisition node");
    }

    /// Panics if the batch handle is ever explicitly cancelled.
    ///
    /// `OutboundBatch::cancel` takes the handle by value and auctioneer never
    /// holds a wrapper around it, so the proof is structural: this client
    /// hands out a real batch and the test asserts the scheduler drove it
    /// to a `Finished` step, which is the only non-fatal exit that exists.
    struct WitnessClient {
        clock: Arc<FakeClock>,
        events: Cell<Option<Vec<ScriptEvent>>>,
        started: Cell<usize>,
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for WitnessClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            self.started.set(self.started.get().saturating_add(1));
            let events = self.events.take().unwrap_or_default();
            Ok(prepared_batch(
                requests,
                observation_cutoff,
                &self.clock,
                events,
            ))
        }
    }

    /// The one composed request behind a `Dispatch` arm.
    fn dispatched(composed: &mut [Composed], index: usize) -> &mut BoundedOutboundRequest {
        match composed.get_mut(index) {
            Some(Composed::Dispatch(request)) => request,
            _ => panic!("entry {index} was expected to be dispatchable"),
        }
    }

    /// The dispatchable arm of every entry, positioned by ORIGINAL index.
    ///
    /// Rows that drive `run_wave` directly compose calls none of which
    /// composition refuses, so a `Refused` arm here means the row set up
    /// something other than what it meant to.
    fn dispatchable(composed: Vec<Composed>) -> Vec<(usize, BoundedOutboundRequest)> {
        composed
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Composed::Dispatch(request) => (index, *request),
                Composed::Refused { .. } => {
                    panic!("this row composes only dispatchable calls")
                }
            })
            .collect()
    }

    fn drive(
        calls: Vec<BidderCall>,
        client: &dyn BoundedProxyClientApi,
        clock: &FakeClock,
        auction_millis: u64,
    ) -> Result<Vec<Option<Slot>>, FanoutInternalError> {
        let limits = test_limits();
        let deadline = auction(clock, auction_millis);
        let entry = clock.now_monotonic();
        let composed = compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED)?;
        let total = composed.len();
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(total).collect();
        let wave: Vec<(usize, BoundedOutboundRequest)> = dispatchable(composed);
        block_on(run_wave(
            wave,
            &mut out,
            clock.now_monotonic(),
            host(clock),
            client,
            &limits,
        ))?;
        Ok(out)
    }

    /// Delays observation only after the SDK has frozen the original budgets.
    struct ObservedLate<'a> {
        client: &'a dyn BoundedProxyClientApi,
        clock: &'a FakeClock,
        delay: Duration,
    }

    impl BoundedProxyClientApi for ObservedLate<'_> {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            let batch = self.client.start_wave(requests, observation_cutoff)?;
            self.clock.advance(self.delay);
            Ok(batch)
        }
    }

    /// a driver delivering terminal indices 2, 0, 1 still publishes
    /// in INPUT order: position comes from `OutboundBatchItem.index`,
    /// never from arrival order.
    #[test]
    fn batch_item_index_drives_position_not_arrival_order() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(
                    2,
                    OutboundSlotResult::new(Duration::from_millis(30), Ok(ok_response(&clock))),
                )),
                ScriptEvent::Item(ScriptItem::new(
                    0,
                    OutboundSlotResult::new(Duration::from_millis(10), Ok(ok_response(&clock))),
                )),
                ScriptEvent::Item(ScriptItem::new(
                    1,
                    OutboundSlotResult::new(Duration::from_millis(20), Ok(ok_response(&clock))),
                )),
            ],
        );
        let calls = vec![
            call("https://a.example/rtb", 500),
            call("https://b.example/rtb", 500),
            call("https://c.example/rtb", 500),
        ];
        let out = drive(calls, &client, &clock, 1_000).expect("a completed wave publishes");
        assert_eq!(out.len(), 3, "one published slot per input call");
        // Arrival order was 2, 0, 1; the elapsed values identify which
        // upstream result landed where, so input order is checkable.
        let elapsed: Vec<Duration> = out
            .iter()
            .map(|slot| {
                slot.as_ref()
                    .expect("every slot is published")
                    .result
                    .elapsed
            })
            .collect();
        assert_eq!(
            elapsed,
            vec![
                Duration::from_millis(10),
                Duration::from_millis(20),
                Duration::from_millis(30),
            ],
            "results must be positional, not in the order they arrived",
        );
    }

    #[test]
    fn sdk_ready_burst_drains_and_sorts_before_yielding_with_logical_timeout_elapsed() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let composed = compose_all(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
                call("https://c.example/rtb", 500),
                call("https://short.example/rtb", 15),
            ],
            &deadline,
            entry,
            policy(),
            TEST_MAX_ENCODED,
        )
        .expect("the characterization calls compose");
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(4).collect();
        let (pending, latest) = super::preflight(
            dispatchable(composed),
            &mut out,
            entry,
            clock.as_ref(),
            &limits,
        )
        .expect("the characterization calls clear preflight");
        let requests = pending
            .into_iter()
            .map(|pending_slot| pending_slot.request)
            .collect();
        let control = Arc::new(DriverControl::new(true));
        let mut batch = prepared_driver(
            requests,
            Deadline::at_instant(latest.expect("a live observation cutoff")),
            &clock,
            DriverScript {
                events: vec![
                    terminal_item(&clock, 2, 30),
                    terminal_item(&clock, 3, 40),
                    terminal_item(&clock, 0, 10),
                    terminal_item(&clock, 1, 20),
                ],
                control: Arc::clone(&control),
            },
            Duration::ZERO,
            false,
        );
        for (index, elapsed_ms) in [(0_usize, 10_u64), (3, 15), (1, 20), (2, 30)] {
            let OutboundBatchNext::Item(item) =
                block_on(batch.next()).expect("the SDK returns a receipt")
            else {
                panic!("all four inputs must resolve before completion");
            };
            // Even the FIRST receipt follows all four ready events and clean
            // EOF. This is SDK source draining, not Auctioneer consumer fairness.
            control.assert_counts(5, 4, 1);
            assert_eq!(
                clock.now_monotonic().saturating_duration_since(entry),
                Duration::from_millis(40)
            );
            let (actual_index, result) = item.into_parts();
            assert_eq!(actual_index, index);
            assert_eq!(result.elapsed, Duration::from_millis(elapsed_ms));
            if index == 3 {
                assert!(
                    matches!(result.outcome, Err(EdgeError::GatewayTimeout { .. })),
                    "the late observation keeps the SDK's logical 15 ms expiry, not 40 ms"
                );
            } else {
                assert_eq!(result.outcome.unwrap().status(), StatusCode::OK);
            }
        }
        assert!(matches!(
            block_on(batch.next()).expect("the SDK observed clean EOF"),
            OutboundBatchNext::Finished(OutboundBatchTermination::Completed)
        ));
    }

    /// the loop keeps polling once its own tally is full and still
    /// observes `Finished(Completed)`, so the positional check always
    /// runs. The driver here emits both items and then ENDS; upstream
    /// reports `Completed` only after observing clean EOF on the poll
    /// after the last item. The controlled-EOF row below also proves
    /// that a full tally alone cannot finish the fanout.
    #[test]
    fn completed_is_observed_on_the_poll_after_the_last_item() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Item(ScriptItem::new(1, ok_slot(&clock))),
            ],
        );
        let out = drive(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &client,
            &clock,
            1_000,
        )
        .expect("a full tally followed by Completed is the normal path");
        assert!(
            out.iter().all(|slot| slot
                .as_ref()
                .is_some_and(|inner| inner.result.outcome.is_ok())),
            "both slots publish their upstream Ok",
        );
    }

    /// Premature EOF cannot publish a `Completed` wave with holes. The SDK
    /// now rejects that driver before Auctioneer's defensive tally check.
    #[test]
    fn completed_with_a_hole_is_rejected_by_the_prepared_batch() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(&clock, vec![]);
        let outcome = drive(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &client,
            &clock,
            1_000,
        );
        assert!(
            matches!(&outcome, Err(FanoutInternalError::Internal(reason))
            if reason == "outbound batch driver failed")
        );
        assert_eq!(
            client.slot_count.get(),
            2,
            "both original slots were prepared"
        );
    }

    /// A cutoff is NOT a contract violation: completed siblings keep
    /// their results and the unresolved slots become positional
    /// expiries carrying the cause composed for them.
    #[test]
    fn cutoff_fills_holes_with_expiries_not_a_violation() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Cutoff,
            ],
        );
        let out = drive(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &client,
            &clock,
            1_000,
        )
        .expect("a cutoff is a normal termination");
        let first = out[0].as_ref().expect("slot 0 published");
        assert!(
            first.result.outcome.is_ok(),
            "the slot that completed before the cutoff keeps its result",
        );
        let second = out[1].as_ref().expect("slot 1 published");
        assert!(
            matches!(second.result.outcome, Err(BoundedSlotError::Expired)),
            "the unresolved slot becomes a positional expiry",
        );
        assert_eq!(
            second.deadline.selected_cause,
            TimeoutCause::Bidder,
            "the cause is the one stored at composition",
        );
    }

    /// A driver fault is the outer 500. The items already recorded are
    /// valid, but auctioneer publishes a wave whole or not at all.
    #[test]
    fn driver_failure_is_the_outer_internal_error() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Failed(EdgeError::internal(anyhow::anyhow!("driver broke"))),
            ],
        );
        let outcome = drive(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &client,
            &clock,
            1_000,
        );
        assert!(
            matches!(outcome, Err(FanoutInternalError::Internal(_))),
            "a poisoned driver maps to the outer internal error",
        );
    }

    fn assert_final_slot_failure(clock: &Arc<FakeClock>, tail: ScriptEvent) {
        let item_events = if matches!(&tail, ScriptEvent::Item(_)) {
            3
        } else {
            2
        };
        let control = Arc::new(DriverControl::new(true));
        let mut events = completed_pair(clock);
        events.push(tail);
        // The source must be dropped at the fault, never reaching this sentinel.
        // Upstream hides post-poison `next()` calls, so these counters
        // witness source polling rather than batch method invocations.
        events.push(terminal_item(clock, 0, 90));
        let client = FinalizingClient::new(
            clock,
            vec![DriverScript {
                events,
                control: Arc::clone(&control),
            }],
        );
        let limits = test_limits();
        let deadline = auction(clock, 1_000);
        let gate = StreamGate::new(2);
        let outcome = block_on(drive_fanout(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &deadline,
            host(clock),
            &client,
            &limits,
            &gate,
        ));
        assert!(
            matches!(outcome, Err(FanoutInternalError::Internal(_))),
            "a post-final-slot fault discards every success, not just the bad event",
        );
        assert_eq!(*client.dispatched.borrow(), vec![2]);
        control.assert_counts(3, item_events, 1);
        assert_eq!(control.pending_polls.get(), 0);
        assert_gate_is_idle(&gate);
    }

    #[test]
    fn a_failure_after_the_final_slot_discards_the_entire_fanout() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        assert_final_slot_failure(
            &clock,
            ScriptEvent::Failed(EdgeError::internal(anyhow::anyhow!("late driver failure"))),
        );
    }

    #[test]
    fn a_duplicate_after_the_final_slot_discards_the_entire_fanout() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        assert_final_slot_failure(&clock, terminal_item(&clock, 0, 90));
    }

    #[test]
    fn an_out_of_range_index_after_the_final_slot_discards_the_entire_fanout() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        assert_final_slot_failure(&clock, terminal_item(&clock, 2, 90));
    }

    #[test]
    fn a_cutoff_after_the_final_slot_discards_the_entire_fanout() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        assert_final_slot_failure(&clock, ScriptEvent::Cutoff);
    }

    #[test]
    fn a_late_failure_in_a_later_wave_discards_previous_successes() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let first = Arc::new(DriverControl::new(true));
        let second = Arc::new(DriverControl::new(true));
        let client = FinalizingClient::new(
            &clock,
            vec![
                DriverScript {
                    events: vec![terminal_item(&clock, 0, 10)],
                    control: Arc::clone(&first),
                },
                DriverScript {
                    events: vec![
                        terminal_item(&clock, 0, 20),
                        ScriptEvent::Failed(EdgeError::internal(anyhow::anyhow!(
                            "second wave finalization failed"
                        ))),
                        terminal_item(&clock, 0, 90),
                    ],
                    control: Arc::clone(&second),
                },
            ],
        );
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let gate = StreamGate::new(1);
        let outcome = block_on(drive_fanout(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
                call("https://c.example/rtb", 500),
            ],
            &deadline,
            host(&clock),
            &client,
            &limits,
            &gate,
        ));
        assert!(
            matches!(outcome, Err(FanoutInternalError::Internal(_))),
            "neither the completed first wave nor the second wave can publish",
        );
        assert_eq!(
            *client.dispatched.borrow(),
            vec![1, 1],
            "the third wave must not start after the second wave is poisoned",
        );
        first.assert_counts(2, 1, 1);
        second.assert_counts(2, 1, 1);
        assert_eq!(first.pending_polls.get(), 0);
        assert_eq!(second.pending_polls.get(), 0);
        assert_gate_is_idle(&gate);
    }

    #[test]
    fn a_full_tally_waits_for_clean_eof_without_rewriting_elapsed() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let control = Arc::new(DriverControl::new(false));
        let client = FinalizingClient::new(
            &clock,
            vec![DriverScript {
                events: completed_pair(&clock),
                control: Arc::clone(&control),
            }],
        );
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let gate = StreamGate::new(2);
        let mut fanout = Box::pin(drive_fanout(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &deadline,
            host(&clock),
            &client,
            &limits,
            &gate,
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(fanout.as_mut().poll(&mut cx).is_pending());
        // The SDK drains two ready items, then rechecks pending EOF before
        // each buffered receipt and again before waiting for termination.
        control.assert_counts(5, 2, 0);
        assert_eq!(control.pending_polls.get(), 3);
        assert!(control.waker.borrow().is_some());
        assert_eq!(gate.free(), 0, "EOF pending still holds the wave lease");
        assert_eq!(gate.in_flight(), 2);

        // Both slots have settled on time. Late EOF must not charge them
        // for finalization or reinterpret them as bidder timeouts.
        clock.advance(Duration::from_millis(600));
        assert!(fanout.as_mut().poll(&mut cx).is_pending());
        control.assert_counts(6, 2, 0);
        assert_eq!(control.pending_polls.get(), 4);
        control.release_eof();
        let results = block_on(fanout).expect("clean EOF completes the full tally");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].elapsed, Duration::from_millis(10));
        assert_eq!(results[1].elapsed, Duration::from_millis(20));
        for result in &results {
            let response = result.result.as_ref().expect("an on-time slot stays Ok");
            assert_eq!(response.status(), 200);
            assert_eq!(response.body_len(), 2);
            // The real SDK facade normalizes headers and adds its adapter
            // marker; no unrelated header may appear while EOF is pending.
            assert_eq!(
                response.headers(),
                &[("x-edgezero-proxy".to_owned(), "wave_fixture".to_owned())],
            );
        }
        assert_eq!(*client.dispatched.borrow(), vec![2]);
        control.assert_counts(7, 2, 1);
        assert_eq!(control.pending_polls.get(), 4);
        assert!(control.waker.borrow().is_none());
        assert_gate_is_idle(&gate);
    }

    #[test]
    fn dropping_fanout_while_eof_is_pending_releases_driver_and_stream_permits() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let control = Arc::new(DriverControl::new(false));
        let client = FinalizingClient::new(
            &clock,
            vec![DriverScript {
                events: completed_pair(&clock),
                control: Arc::clone(&control),
            }],
        );
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let gate = StreamGate::new(2);
        let mut fanout = Box::pin(drive_fanout(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &deadline,
            host(&clock),
            &client,
            &limits,
            &gate,
        ));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(fanout.as_mut().poll(&mut cx).is_pending());
        // SDK ready-burst draining still requires clean EOF; the two
        // buffered receipts each cause another pending source check.
        control.assert_counts(5, 2, 0);
        assert_eq!(control.pending_polls.get(), 3);
        assert!(control.waker.borrow().is_some());
        assert_eq!(gate.free(), 0);
        assert_eq!(gate.in_flight(), 2);
        drop(fanout);

        assert_eq!(*client.dispatched.borrow(), vec![2]);
        control.assert_counts(5, 2, 1);
        assert_eq!(control.pending_polls.get(), 3);
        assert!(control.waker.borrow().is_none());
        assert_gate_is_idle(&gate);
        let lease = block_on(gate.acquire_wave(2)).expect("the released permits are reusable");
        assert_eq!(lease.weight(), 2);
        assert_eq!(gate.in_flight(), 2);
        drop(lease);
        assert_gate_is_idle(&gate);
    }

    /// A full wave — dispatch, cutoff and fatal paths alike — completes
    /// without `cancel()` ever being reachable. `cancel` consumes the
    /// handle, auctioneer never moves it out of the loop, and each path exits
    /// through `Finished` or the outer error instead.
    #[test]
    fn no_explicit_cancel_on_the_normal_fanout_path() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        for events in [
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Item(ScriptItem::new(1, ok_slot(&clock))),
            ],
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Cutoff,
            ],
            vec![ScriptEvent::Failed(EdgeError::internal(anyhow::anyhow!(
                "x"
            )))],
        ] {
            let client = WitnessClient {
                clock: Arc::clone(&clock),
                events: Cell::new(Some(events)),
                started: Cell::new(0),
            };
            let outcome = drive(
                vec![
                    call("https://a.example/rtb", 500),
                    call("https://b.example/rtb", 500),
                ],
                &client,
                &clock,
                1_000,
            );
            assert_eq!(
                client.started.get(),
                1,
                "exactly one batch per wave, on every path",
            );
            // Whether the path published or errored, it left through a
            // `Finished` step or the outer error — never a cancel.
            match outcome {
                Ok(_) | Err(FanoutInternalError::Internal(_)) => {}
                Err(other) => panic!("unexpected wave outcome: {other:?}"),
            }
        }
    }

    /// A mixed wave dispatches only the valid siblings, restores original
    /// indices, and carries the auctioneer-MEASURED elapsed on the preflight
    /// slots.
    #[test]
    fn preflight_compaction_restores_indices_and_skips_empty_upstream() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Item(ScriptItem::new(1, ok_slot(&clock))),
            ],
        );
        // Index 1 carries an unrepresentable header, so it fails preflight while
        // 0 and 2 dispatch.
        let calls = vec![
            call("https://a.example/rtb", 500),
            invalid_header_call(500),
            call("https://c.example/rtb", 500),
        ];
        let out = drive(calls, &client, &clock, 1_000).expect("valid siblings still publish");
        assert_eq!(
            client.slot_count.get(),
            2,
            "only the two valid calls went upstream",
        );
        assert!(
            out[0]
                .as_ref()
                .expect("slot 0 published")
                .result
                .outcome
                .is_ok(),
            "slot 0 kept its position and its upstream result",
        );
        assert!(
            matches!(
                out[1].as_ref().expect("slot 1 published").result.outcome,
                Err(BoundedSlotError::Preflight)
            ),
            "the preflight failure stays at its ORIGINAL index",
        );
        assert!(
            out[2]
                .as_ref()
                .expect("slot 2 published")
                .result
                .outcome
                .is_ok(),
            "slot 2 kept its position too",
        );
        assert_eq!(
            out[1].as_ref().expect("slot 1 published").result.elapsed,
            Duration::ZERO,
            "a clock that never advanced measures zero (a legal reading)",
        );
    }

    /// An all-invalid wave never constructs an upstream batch.
    #[test]
    fn an_all_invalid_wave_never_calls_upstream() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(&clock, vec![]);
        let out = drive(
            vec![invalid_header_call(500), invalid_header_call(500)],
            &client,
            &clock,
            1_000,
        )
        .expect("an all-invalid wave is not a fanout failure");
        assert_eq!(client.dispatched.get(), 0, "nothing was dispatched");
        assert!(
            out.iter().all(|slot| matches!(
                slot.as_ref().expect("published").result.outcome,
                Err(BoundedSlotError::Preflight)
            )),
            "every slot is its own preflight failure",
        );
    }

    /// Slot A finishes well inside its deadline while slot B delays the
    /// batch past A's deadline. A still publishes as `Ok`: there is no
    /// batch-time and no publication-time recheck, so a slow sibling
    /// cannot retroactively time out an on-time bidder.
    #[test]
    fn on_time_slot_is_not_rewritten_by_batch_or_publication_time() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(
                    0,
                    OutboundSlotResult::new(Duration::from_millis(5), Ok(ok_response(&clock))),
                )),
                ScriptEvent::Item(ScriptItem::new(
                    1,
                    OutboundSlotResult::new(Duration::from_millis(400), Ok(ok_response(&clock))),
                )),
            ],
        );
        // A's budget is 50ms, B's is 500ms.
        let calls = vec![
            call("https://a.example/rtb", 50),
            call("https://b.example/rtb", 500),
        ];
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let entry = clock.now_monotonic();
        let composed =
            compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED).expect("composes");
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(2).collect();
        let wave: Vec<(usize, BoundedOutboundRequest)> = dispatchable(composed);
        // The provider advances the shared clock to B's 400-ms completion
        // after SDK preparation, while retaining A's earlier terminal sample.
        block_on(run_wave(
            wave,
            &mut out,
            entry,
            host(&clock),
            &client,
            &limits,
        ))
        .expect("the wave completes");
        assert!(
            out[0]
                .as_ref()
                .expect("slot 0 published")
                .result
                .outcome
                .is_ok(),
            "A finished at 5ms upstream and publishes as Ok despite the \
             batch running to 400ms",
        );
        assert_eq!(
            out[0].as_ref().expect("slot 0 published").result.elapsed,
            Duration::from_millis(5),
            "A's own elapsed is upstream's, not the batch's",
        );
    }

    /// The decode caps arrive at each slot individually, so a wave with
    /// two different policies does not collapse to one shared value.
    #[test]
    fn decode_policy_travels_with_the_slot() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Item(ScriptItem::new(1, ok_slot(&clock))),
            ],
        );
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let entry = clock.now_monotonic();
        let mut wide = policy();
        wide.max_decoded_bytes = 0x0010_0000;
        let narrow = policy();
        let mut composed = compose_all(
            vec![
                call("https://a.example/rtb", 500),
                call("https://b.example/rtb", 500),
            ],
            &deadline,
            entry,
            narrow,
            TEST_MAX_ENCODED,
        )
        .expect("composes");
        // Give slot 1 a wider decode cap than slot 0.
        dispatched(&mut composed, 1).policy = wide;
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(2).collect();
        let wave: Vec<(usize, BoundedOutboundRequest)> = dispatchable(composed);
        block_on(run_wave(
            wave,
            &mut out,
            entry,
            host(&clock),
            &client,
            &limits,
        ))
        .expect("the wave completes");
        assert_eq!(
            out[0]
                .as_ref()
                .expect("slot 0 published")
                .decode
                .max_decoded_bytes,
            narrow.max_decoded_bytes,
            "slot 0 carries its OWN cap",
        );
        assert_eq!(
            out[1]
                .as_ref()
                .expect("slot 1 published")
                .decode
                .max_decoded_bytes,
            wide.max_decoded_bytes,
            "slot 1 carries its OWN, different cap, not a wave-wide one",
        );
    }

    /// A passthrough slot whose settle crosses `deadline_at` on a clock
    /// that only the cooperative yield advances classifies as an expiry,
    /// never as an encoding verdict and never as an oversize.
    ///
    /// The coding is one upstream refuses — a `gzip` or `deflate` body
    /// is decoded and stripped upstream, so it would never reach the
    /// recheck at all.
    #[test]
    fn settle_expiry_under_frozen_cpu_clock_is_timeout() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "zstd".parse().expect("valid"));
        let response = OutboundResponse::new(
            Method::POST,
            StatusCode::OK,
            headers,
            Body::from_bytes(b"\x28\xb5\x2f\xfd".to_vec()),
            monotonic(&clock),
            edgezero_core::ContentTypeProvenance::Unknown,
        );
        let client = ScriptedClient::new(
            &clock,
            vec![ScriptEvent::Item(ScriptItem::new(
                0,
                OutboundSlotResult::new(Duration::ZERO, Ok(response)),
            ))],
        );
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let entry = clock.now_monotonic();
        let composed = compose_all(
            vec![call("https://a.example/rtb", 50)],
            &deadline,
            entry,
            policy(),
            TEST_MAX_ENCODED,
        )
        .expect("composes");
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(1).collect();
        let wave: Vec<(usize, BoundedOutboundRequest)> = dispatchable(composed);
        // Retain the on-time upstream result, but delay observing it until
        // the Auctioneer-owned settlement phase is beyond its 50-ms deadline.
        let observed = ObservedLate {
            client: &client,
            clock: &clock,
            delay: Duration::from_millis(60),
        };
        block_on(run_wave(
            wave,
            &mut out,
            entry,
            host(&clock),
            &observed,
            &limits,
        ))
        .expect("the wave completes");
        let slot = out[0].as_ref().expect("slot 0 published");
        assert_eq!(
            slot.result
                .outcome
                .as_ref()
                .err()
                .map(|err| matches!(err, BoundedSlotError::Expired)),
            Some(true),
            "a settle that crosses its deadline is an expiry, not an \
             encoding verdict and not an oversize",
        );
        assert_eq!(
            slot.deadline.selected_cause,
            TimeoutCause::Bidder,
            "and it classifies through the cause stored at composition",
        );
    }

    /// An undecodable body is REFUSED with its cause, never published as
    /// compressed bytes for a parser to choke on.
    ///
    /// `deflate` is deliberately NOT the coding here: upstream decodes
    /// it and strips the header, which is what retired the gap this row
    /// used to characterize. What is left is the residue no decoder can
    /// finish, and an unknown coding is the case that carries a token.
    #[test]
    fn an_undecodable_body_is_refused_with_its_cause() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "zstd".parse().expect("valid"));
        let response = OutboundResponse::new(
            Method::POST,
            StatusCode::OK,
            headers,
            Body::from_bytes(b"\x28\xb5\x2f\xfd\x20\x05\x29\x00\x00".to_vec()),
            monotonic(&clock),
            edgezero_core::ContentTypeProvenance::Unknown,
        );
        let client = ScriptedClient::new(
            &clock,
            vec![ScriptEvent::Item(ScriptItem::new(
                0,
                OutboundSlotResult::new(Duration::ZERO, Ok(response)),
            ))],
        );
        let out = drive(
            vec![call("https://a.example/rtb", 500)],
            &client,
            &clock,
            1_000,
        )
        .expect("the wave completes");
        let slot = out[0].as_ref().expect("slot 0 published");
        match &slot.result.outcome {
            Err(BoundedSlotError::Undecodable(cause)) => assert_eq!(
                *cause,
                UndecodableEncoding::Unsupported("zstd".to_owned()),
                "the refusal names the coding upstream parsed",
            ),
            other => panic!("an undecodable body must not publish as a response: {other:?}"),
        }
        assert_eq!(
            classify_slot(
                Err(BoundedSlotError::Undecodable(
                    UndecodableEncoding::Unsupported("zstd".to_owned()),
                )),
                TimeoutCause::Bidder,
                policy().max_decoded_bytes,
            )
            .expect_err("an undecodable slot is a per-bidder failure")
            .public_message(),
            "bidder response encoding unsupported",
            "and the wire category is fixed text, never the token",
        );
    }

    /// The two nothing-was-sent refusals render DIFFERENT wire pairs,
    /// and neither renders `999`.
    ///
    /// `classify_slot` used to answer `FanoutError::Other` for both,
    /// which renders 999/100 — the pair the spec reserves for an
    /// uncategorised fan-out fault. The assertion is on the rendered
    /// pair rather than the category, because the category is exactly
    /// what was already right-looking and wrong.
    #[test]
    fn the_two_undispatched_refusals_render_distinct_specified_codes() {
        let rendered = |error| {
            fanout_error_codes(
                &classify_slot(Err(error), TimeoutCause::Bidder, policy().max_decoded_bytes)
                    .expect_err("an undispatched slot is a per-bidder failure"),
            )
        };
        assert_eq!(
            rendered(BoundedSlotError::OversizeEncodedRequest),
            (5, 100),
            "a request auctioneer built and had no budget to carry is \
             FailedToRequestBids",
        );
        assert_eq!(
            rendered(BoundedSlotError::Preflight),
            (4, 100),
            "a request auctioneer could not represent is the pair the spec's \
             wire table assigns it",
        );
        assert_ne!(
            rendered(BoundedSlotError::OversizeEncodedRequest),
            rendered(BoundedSlotError::Preflight),
            "the two are different wire facts; collapsing them is the bug",
        );
    }

    /// A status that cannot carry an auction answer never becomes a
    /// publishable response on the bounded path either.
    ///
    /// `classify_slot` is the only thing between a lifted slot and the
    /// `BidderCallResult` the exchange parses, so refusing here is what
    /// keeps the body away from every adapter. The rows carry valid,
    /// bid-bearing `OpenRTB`: a body that could not parse would not tell
    /// a status rule from a parse failure.
    #[test]
    fn only_2xx_publishes_as_a_bidder_response() {
        use auctioneer_exchange::debug::metadata::DebugError;

        // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
        // `const` can hold. Converting means turning this into a function
        // returning a `String` and re-rendering it at every use.
        const BID_JSON: &[u8] =
            br#"{"id":"a","cur":"USD","seatbid":[{"seat":"fake_s","bid":[{"id":"b","impid":"i","price":1.5}]}]}"#;
        let slot = |status: u16| {
            Ok(BoundedProxyResponse {
                status,
                headers: HeaderMap::new(),
                content_type_provenance: ContentTypeProvenance::Unknown,
                body: BID_JSON.to_vec(),
                cap_row: CapRow::ReadyToParse,
            })
        };
        for status in [
            100_u16, 101, 103, 300, 301, 302, 307, 308, 400, 404, 500, 503,
        ] {
            let outcome = classify_slot(
                slot(status),
                TimeoutCause::Bidder,
                policy().max_decoded_bytes,
            );
            assert!(
                matches!(outcome, Err(FanoutError::BadServerResponse)),
                "status {status} cannot carry a bid, got {outcome:?}",
            );
            assert_eq!(
                DebugError::from_transport(outcome.as_ref().unwrap_err()),
                DebugError::Transport,
            );
        }
        for status in [200_u16, 204, 299] {
            let outcome = classify_slot(
                slot(status),
                TimeoutCause::Bidder,
                policy().max_decoded_bytes,
            );
            assert!(
                matches!(outcome, Ok(resp) if resp.status() == status),
                "status {status} is a bidder answer and publishes",
            );
        }
    }

    /// A stacked chain and a malformed value refuse through their own
    /// causes — the three reasons are never collapsed into one.
    #[test]
    fn stacked_and_malformed_bodies_keep_distinct_causes() {
        for (encoding, expected) in [
            ("gzip, br", UndecodableEncoding::Stacked),
            ("gzip;q=1", UndecodableEncoding::Malformed),
        ] {
            let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
            let mut headers = HeaderMap::new();
            headers.insert("content-encoding", encoding.parse().expect("valid"));
            let response = OutboundResponse::new(
                Method::POST,
                StatusCode::OK,
                headers,
                Body::from_bytes(b"opaque".to_vec()),
                monotonic(&clock),
                edgezero_core::ContentTypeProvenance::Unknown,
            );
            let client = ScriptedClient::new(
                &clock,
                vec![ScriptEvent::Item(ScriptItem::new(
                    0,
                    OutboundSlotResult::new(Duration::ZERO, Ok(response)),
                ))],
            );
            let out = drive(
                vec![call("https://a.example/rtb", 500)],
                &client,
                &clock,
                1_000,
            )
            .expect("the wave completes");
            let slot = out[0].as_ref().expect("slot 0 published");
            match &slot.result.outcome {
                Err(BoundedSlotError::Undecodable(cause)) => assert_eq!(
                    *cause, expected,
                    "`{encoding}` must refuse through its own cause",
                ),
                other => panic!("`{encoding}` must not publish as a response: {other:?}"),
            }
        }
    }

    /// The passthrough row is what selects the decode recheck; a
    /// ready-to-parse body never enters it.
    #[test]
    fn a_ready_to_parse_body_skips_the_decode_recheck() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let client = ScriptedClient::new(
            &clock,
            vec![ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock)))],
        );
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let entry = clock.now_monotonic();
        let composed = compose_all(
            vec![call("https://a.example/rtb", 50)],
            &deadline,
            entry,
            policy(),
            TEST_MAX_ENCODED,
        )
        .expect("composes");
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(1).collect();
        let wave: Vec<(usize, BoundedOutboundRequest)> = dispatchable(composed);
        let observed = ObservedLate {
            client: &client,
            clock: &clock,
            delay: Duration::from_millis(60),
        };
        block_on(run_wave(
            wave,
            &mut out,
            entry,
            host(&clock),
            &observed,
            &limits,
        ))
        .expect("the wave completes");
        let slot = out[0].as_ref().expect("slot 0 published");
        match &slot.result.outcome {
            Ok(resp) => assert_eq!(
                resp.cap_row,
                CapRow::ReadyToParse,
                "an unencoded body owes no decode",
            ),
            Err(err) => panic!("a ready-to-parse slot must not be rewritten: {err:?}"),
        }
    }

    /// `OutboundBatchFailure` preserves the slots completed before the
    /// driver broke. The DISCARD is deliberate: auctioneer publishes a wave
    /// whole or not at all, so the outer answer is still 500 — but the
    /// destructure has to be written out to prove `?` was not used by
    /// accident, and the type is `#[non_exhaustive]`, so the `..` is
    /// mandatory outside `EdgeZero`'s crate.
    #[test]
    fn collect_failure_preserves_completed_slots_is_documented() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        // Three slots, two of which complete before the fault. This row
        // exercises partial-tally preservation; the final-slot rows above
        // cover faults observed after every index has already resolved.
        let client = ScriptedClient::new(
            &clock,
            vec![
                ScriptEvent::Item(ScriptItem::new(0, ok_slot(&clock))),
                ScriptEvent::Item(ScriptItem::new(1, ok_slot(&clock))),
                ScriptEvent::Failed(EdgeError::internal(anyhow::anyhow!("driver broke"))),
            ],
        );
        let batch = client
            .start_wave(
                repeat_with(|| OutboundRequest::get("https://fake.example/rtb").unwrap())
                    .take(3)
                    .collect(),
                Deadline::at_instant(clock.origin().checked_add(Duration::from_secs(1)).unwrap()),
            )
            .unwrap();
        // NEVER `.await?` here: `?` coerces the failure into an
        // `EdgeError` through its `From` impl and throws `slots` away —
        // the bidders that DID complete. The `..` is required because
        // `OutboundBatchFailure` is `#[non_exhaustive]`.
        let failure = match block_on(batch.collect()) {
            Ok(results) => panic!("the driver fails, so collect cannot succeed: {results:?}"),
            Err(failure) => failure.into_parts(),
        };
        let (error, slots) = failure;
        assert_eq!(slots.len(), 3, "the vector is index-aligned to the input");
        assert_eq!(
            slots
                .iter()
                .filter(|slot| matches!(slot, OutboundCollectedSlot::Resolved(_)))
                .count(),
            2,
            "both slots that completed before the fault are PRESERVED",
        );
        assert!(
            matches!(slots.get(2), Some(OutboundCollectedSlot::Unresolved)),
            "the slot that never resolved is the hole, not a fabricated result",
        );
        assert!(
            !format!("{error}").is_empty(),
            "the fault itself is carried alongside them",
        );
        // And auctioneer's own answer is still the outer 500, because a wave
        // publishes whole or not at all.
        let mapped = FanoutInternalError::Internal("outbound batch driver failed".to_owned());
        assert!(matches!(mapped, FanoutInternalError::Internal(_)));
    }

    /// Composition overflow is the outer internal error BEFORE any
    /// dispatch, never a saturating far-future instant that would read as
    /// "this bidder has forever".
    #[test]
    fn deadline_overflow_is_internal_error_not_saturation() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        // `Duration::MAX` is representable as a timeout but not as an
        // instant offset — `Duration::from_millis(u64::MAX)` still adds
        // cleanly, so a weaker fixture would prove nothing.
        let mut unrepresentable = call("https://a.example/rtb", 0);
        unrepresentable.timeout = Duration::MAX;
        let outcome = compose_all(
            vec![unrepresentable],
            &auction(&clock, 1_000),
            clock.now_monotonic(),
            policy(),
            TEST_MAX_ENCODED,
        );
        assert!(
            matches!(outcome, Err(FanoutInternalError::Internal(_))),
            "an unrepresentable per-call deadline is the outer 500",
        );
    }

    /// Equality selects `Bidder`, matching the per-call race's own tie
    /// rule, and a bidder budget past the auction deadline clamps to the
    /// auction's instant with the auction's cause.
    #[test]
    fn composition_selects_the_earlier_budget_with_bidder_on_a_tie() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let auction_at = auction(&clock, 100);
        let mut composed = compose_all(
            vec![
                call("https://a.example/rtb", 50),
                call("https://b.example/rtb", 100),
                call("https://c.example/rtb", 200),
            ],
            &auction_at,
            entry,
            policy(),
            TEST_MAX_ENCODED,
        )
        .expect("composes");
        assert_eq!(
            dispatched(&mut composed, 0).deadline.selected_cause,
            TimeoutCause::Bidder,
        );
        assert_eq!(
            dispatched(&mut composed, 1).deadline.selected_cause,
            TimeoutCause::Bidder,
            "equality attributes to the bidder",
        );
        assert_eq!(
            dispatched(&mut composed, 2).deadline.selected_cause,
            TimeoutCause::AuctionDeadline,
        );
        assert_eq!(
            dispatched(&mut composed, 2).deadline.deadline_at,
            auction_at.deadline_at,
            "a bidder budget past the auction clamps to the auction instant",
        );
    }

    #[test]
    fn wave_width_never_exceeds_the_gate_or_the_work() {
        assert_eq!(wave_width(4, 10), 4, "the gate bounds it");
        assert_eq!(wave_width(4, 2), 2, "the remaining work bounds it");
        assert_eq!(wave_width(0, 5), 0, "a zero gate grants nothing");
    }

    #[test]
    fn remaining_until_is_derived_at_arming_time_and_saturates() {
        let now = MonotonicInstant::now();
        let later = now
            .checked_add(Duration::from_millis(40))
            .expect("representable");
        assert_eq!(remaining_until(later, now), Duration::from_millis(40));
        assert_eq!(
            remaining_until(now, later),
            Duration::ZERO,
            "an instant already past arms a zero sleep, never a wrap",
        );
    }

    /// A client that ENFORCES the batch cutoff the way the real adapters
    /// do, which is the only way a wrong cutoff is observable.
    ///
    /// Two behaviours are modelled, and both come from the pinned
    /// adapter. Upstream folds the cutoff into every member's effective
    /// budget as the tighter operand (`dispatch_budget`), so a member
    /// whose scripted response lands after the cutoff never completes at
    /// all; and a terminal observation at or past the cutoff is DROPPED
    /// rather than yielded (`finish_batch_item`), ending the batch in
    /// `Cutoff`. The scripted client in the rows above ignores its
    /// cutoff entirely and therefore cannot see any of this.
    struct CutoffEnforcingClient {
        clock: Arc<FakeClock>,
        /// Each dispatched slot's scripted completion offset from batch
        /// entry, in the order the wave hands them over.
        answers_at: Vec<Duration>,
        entry: MonotonicInstant,
        observed: Cell<Option<MonotonicInstant>>,
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for CutoffEnforcingClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            let cutoff = observation_cutoff;
            self.observed.set(Some(cutoff.instant()));
            let slots = requests.len();
            let mut events = Vec::with_capacity(slots);
            for (index, answer_at) in self.answers_at.iter().take(slots).enumerate() {
                let completed_at = self
                    .entry
                    .checked_add(*answer_at)
                    .expect("the test horizon is representable");
                // `finish_batch_item`: a terminal observation at or past
                // the cutoff is not an item, and it ends the batch.
                if cutoff.is_expired_at(completed_at) {
                    events.push(ScriptEvent::Cutoff);
                    break;
                }
                events.push(ScriptEvent::Item(ScriptItem::new(
                    index,
                    OutboundSlotResult::new(*answer_at, Ok(ok_response_at(self.entry))),
                )));
            }
            Ok(prepared_batch(requests, cutoff, &self.clock, events))
        }
    }

    /// A response whose deferred body deadlines read a FIXED instant.
    ///
    /// The cutoff rows never advance the clock, so a response built
    /// against a moving source would be read at a different instant than
    /// the one the row reasons about.
    fn ok_response_at(entry: MonotonicInstant) -> OutboundResponse {
        OutboundResponse::new(
            Method::POST,
            StatusCode::OK,
            HeaderMap::new(),
            Body::from_bytes(b"{}".to_vec()),
            MonotonicClock::new(move || entry),
            edgezero_core::ContentTypeProvenance::Unknown,
        )
    }

    /// The whole-wave cutoff is the LATEST dispatched deadline, so a
    /// short-budget member cannot truncate its siblings.
    ///
    /// Budgets 10/50/50 ms, every bidder answering at 20 ms. Under the
    /// earliest-deadline cutoff the batch closed at 10 ms and the two
    /// 50 ms calls lost a budget they were plainly inside; under the
    /// latest they keep it, and the 10 ms call still expires — on its own
    /// per-request deadline, which is where that verdict belongs.
    #[test]
    fn the_wave_cutoff_is_the_latest_deadline_not_the_earliest() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let client = CutoffEnforcingClient {
            clock: Arc::clone(&clock),
            answers_at: vec![
                Duration::from_millis(20),
                Duration::from_millis(20),
                Duration::from_millis(20),
            ],
            entry,
            observed: Cell::new(None),
        };
        let calls = vec![
            call("https://a.example/rtb", 10),
            call("https://b.example/rtb", 50),
            call("https://c.example/rtb", 50),
        ];
        let out = drive(calls, &client, &clock, 1_000).expect("the wave publishes");

        let cutoff = client.observed.get().expect("a batch was dispatched");
        assert_eq!(
            cutoff,
            entry
                .checked_add(Duration::from_millis(50))
                .expect("representable"),
            "the cutoff is the LATEST composed deadline in the wave, not the earliest",
        );
        for (index, slot) in out.iter().enumerate().skip(1) {
            assert!(
                slot.as_ref().expect("published").result.outcome.is_ok(),
                "the 50ms call at index {index} answered at 20ms and keeps its result; \
                 an earliest-deadline cutoff would have closed the batch at 10ms",
            );
        }
    }

    /// A call REJECTED by preflight never moves the observation
    /// window for the siblings that did dispatch.
    ///
    /// The two rejected calls BRACKET the dispatched range: index 1
    /// carries the shortest budget in the wave (5 ms) and index 2 the
    /// longest (900 ms), while both dispatched calls sit between them at
    /// 30 ms. So a cutoff folded over the whole wave lands off 30 ms
    /// whichever direction it folds in — `max` stretches it to 900 ms and
    /// `min` truncates it to 5 ms, and the 5 ms case also expires the two
    /// dispatched answers at 10 ms. One rejected call on one side of the
    /// range only catches the fold that reaches toward it.
    #[test]
    fn a_preflight_rejected_call_does_not_stretch_the_cutoff() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let client = CutoffEnforcingClient {
            clock: Arc::clone(&clock),
            answers_at: vec![Duration::from_millis(10), Duration::from_millis(10)],
            entry,
            observed: Cell::new(None),
        };
        let calls = vec![
            call("https://a.example/rtb", 30),
            invalid_header_call(5),
            invalid_header_call(900),
            call("https://d.example/rtb", 30),
        ];
        let out = drive(calls, &client, &clock, 1_000).expect("valid siblings publish");

        assert_eq!(
            client.observed.get().expect("a batch was dispatched"),
            entry
                .checked_add(Duration::from_millis(30))
                .expect("representable"),
            "the cutoff covers the DISPATCHED calls only; the 5ms and 900ms \
             calls both failed preflight and were never sent",
        );
        for index in [0, 3] {
            assert!(
                out[index]
                    .as_ref()
                    .expect("published")
                    .result
                    .outcome
                    .is_ok(),
                "the 30ms call at index {index} answered at 10ms and keeps its \
                 result; a cutoff that counted the rejected calls' 5ms deadline \
                 would have closed the batch before it",
            );
        }
        for index in [1, 2] {
            assert!(
                matches!(
                    out[index].as_ref().expect("published").result.outcome,
                    Err(BoundedSlotError::Preflight)
                ),
                "the rejected call at index {index} still holds its own \
                 positional failure",
            );
        }
    }

    /// The per-call response cap reaches the dispatched request
    /// through the BOUNDED constructor, and the tighter of it and the
    /// envelope ceiling wins.
    #[test]
    fn the_per_call_response_cap_narrows_the_envelope_policy() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let deadline = auction(&clock, 1_000);
        // Slot 0 asks for 8 bytes — far tighter than the 256 KiB
        // envelope. Slot 1 asks for more than the envelope allows.
        let calls = vec![
            BidderCall {
                max_transport_bytes: 16,
                max_decoded_bytes: 8,
                ..call("https://a.example/rtb", 500)
            },
            BidderCall {
                max_transport_bytes: 8 * 1024 * 1024,
                max_decoded_bytes: 8 * 1024 * 1024,
                ..call("https://b.example/rtb", 500)
            },
        ];
        let mut composed =
            compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED).expect("composes");

        assert_eq!(
            dispatched(&mut composed, 0).policy.max_decoded_bytes,
            8,
            "the call's own 8-byte cap is the tighter one and wins",
        );
        assert_eq!(
            dispatched(&mut composed, 0).policy.max_transport_bytes,
            16,
            "the transport cap narrows independently",
        );
        assert_eq!(
            dispatched(&mut composed, 1).policy.max_transport_bytes,
            policy().max_transport_bytes,
            "the transport cap cannot exceed the envelope either",
        );
        assert_eq!(
            dispatched(&mut composed, 1).policy.max_decoded_bytes,
            policy().max_decoded_bytes,
            "a call cap wider than the envelope loses; it cannot escalate",
        );
        assert_eq!(
            DecodePolicy::from(&dispatched(&mut composed, 0).policy).max_decoded_bytes,
            8,
            "the narrowed cap is what travels with the slot to the decoder",
        );
    }

    #[test]
    fn independent_response_caps_reach_the_transport_and_decode_policies() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let calls = vec![BidderCall {
            max_transport_bytes: 4_096,
            max_decoded_bytes: 0x4000,
            ..call("https://a.example/rtb", 500)
        }];
        let mut composed = compose_all(
            calls,
            &auction(&clock, 1_000),
            clock.now_monotonic(),
            policy(),
            TEST_MAX_ENCODED,
        )
        .expect("composes");
        let outbound = dispatched(&mut composed, 0);
        assert_eq!(outbound.policy.max_transport_bytes, 4_096);
        assert_eq!(outbound.policy.max_decoded_bytes, 0x4000);
        let decode = DecodePolicy::from(&outbound.policy);
        assert_eq!(decode.max_decoded_bytes, 0x4000);
    }

    /// A gzip endpoint gets gzip BYTES and a matching
    /// `Content-Encoding`, through the bounded constructor.
    #[test]
    fn gzip_endpoint_compression_survives_composition() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let deadline = auction(&clock, 1_000);
        // Raw string, not `serde_json::json!`: this fixture's own `len()` is
        // read below, so the exact rendered text is part of what the row
        // asserts. `json!` renders its own spacing.
        let plain = br#"{"id":"gzip-me","imp":[]}"#.to_vec();
        let raw_request = outbound_request::request(
            "POST".into(),
            "https://gz.example/rtb".into(),
            vec![],
            plain.clone(),
            vec![],
        )
        .unwrap();
        let (_, request) = finalize_for_transport(
            raw_request,
            &TransportFinalize {
                gpc: None,
                compression: Some(Compression::Gzip),
                max_encoded_bytes: 1 << 18,
            },
        );
        let calls = vec![BidderCall {
            request: request.unwrap(),
            ..call("https://gz.example/rtb", 500)
        }];
        let mut composed =
            compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED).expect("composes");
        let sent = &dispatched(&mut composed, 0).request;

        assert_ne!(sent.body(), plain, "the body was actually encoded");
        assert_eq!(
            sent.body().get(..2),
            Some(&[0x1f_u8, 0x8b_u8][..]),
            "a gzip member starts with the gzip magic bytes",
        );
        let mut decoded = Vec::new();
        GzDecoder::new(sent.body())
            .read_to_end(&mut decoded)
            .expect("the encoded body round-trips");
        assert_eq!(
            decoded, plain,
            "gzip of the original body, not of something else"
        );

        let encodings: Vec<&str> = sent
            .headers()
            .filter(|(name, _value)| name.eq_ignore_ascii_case("content-encoding"))
            .map(|(_name, value)| value)
            .collect();
        assert_eq!(
            encodings,
            vec!["gzip"],
            "exactly one Content-Encoding, declaring gzip",
        );
        let lengths: Vec<&str> = sent
            .headers()
            .filter(|(name, _value)| name.eq_ignore_ascii_case("content-length"))
            .map(|(_name, value)| value)
            .collect();
        assert_eq!(
            lengths,
            Vec::<&str>::new(),
            "only the transport may choose framing headers",
        );
    }

    /// The encoded cap bounds the ENCODER, not just the result.
    ///
    /// A body whose gzip output cannot fit the cap is refused
    /// positionally at exchange finalization, before the host would have refused
    /// the oversized request anyway — so the isolate never holds an
    /// encoder output it had no budget for. Incompressible input is
    /// used deliberately: compressible filler would shrink under the
    /// cap and the row would prove nothing about the bound.
    #[test]
    fn a_body_that_cannot_be_compressed_within_the_cap_is_refused_positionally() {
        // Pseudorandom bytes: gzip cannot shrink these meaningfully, so
        // the encoded size stays above the tiny cap below.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut incompressible: Vec<u8> = Vec::with_capacity(8192);
        while incompressible.len() < 8192 {
            state ^= state << 13_u32;
            state ^= state >> 7_u32;
            state ^= state << 17_u32;
            incompressible.push(u8::try_from(state & 0xff_u64).unwrap_or(0));
        }
        let raw_request = outbound_request::request(
            "POST".into(),
            "https://gz.example/rtb".into(),
            vec![],
            incompressible,
            vec![],
        )
        .expect("the raw body fits the sink");
        let tiny_cap = 64;
        let (sent, finalized) = finalize_for_transport(
            raw_request,
            &TransportFinalize {
                gpc: None,
                compression: Some(Compression::Gzip),
                max_encoded_bytes: tiny_cap,
            },
        );
        assert_eq!(
            sent.url(),
            "https://gz.example/rtb",
            "the refused slot keeps its metadata"
        );
        let refusal = match finalized.expect_err("an over-cap encode is refused before composition")
        {
            FinalizeError::OversizeEncoded { limit } => {
                assert_eq!(limit, tiny_cap);
                FanoutError::OversizeRequest {
                    stage: OversizeRequestStage::Encoded,
                }
            }
            other @ FinalizeError::ReserveOverflow => panic!(
                "an encode that cannot fit the cap is this call's own positional refusal, got {other:?}"
            ),
        };
        // Asserted on the RENDERED pair, through the exchange's own
        // mapping. The earlier row stopped at the intermediate variant,
        // which held for whatever code that variant rendered — and it
        // rendered 999/100, the pair reserved for an uncategorised
        // fan-out fault.
        let rendered = fanout_error_codes(&refusal);
        assert_eq!(
            rendered,
            (5, 100),
            "an encoded-size overflow renders FailedToRequestBids, not 999",
        );
    }

    /// A body that DOES fit still finalizes and composes, so the cap refuses
    /// only what it has to. Guards the row above against passing for the wrong
    /// reason — a finalization that refused everything would satisfy it.
    #[test]
    fn a_body_that_fits_the_cap_still_encodes() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let deadline = auction(&clock, 1_000);
        // Raw string, not `serde_json::json!`: this fixture's own `len()` is
        // read below, so the exact rendered text is part of what the row
        // asserts. `json!` renders its own spacing.
        let plain = br#"{"id":"small","imp":[]}"#.to_vec();
        let raw_request = outbound_request::request(
            "POST".into(),
            "https://gz.example/rtb".into(),
            vec![],
            plain.clone(),
            vec![],
        )
        .expect("the raw body fits the sink");
        let (_, finalized) = finalize_for_transport(
            raw_request,
            &TransportFinalize {
                gpc: None,
                compression: Some(Compression::Gzip),
                max_encoded_bytes: u64::try_from(TEST_MAX_ENCODED).expect("the cap fits"),
            },
        );
        let calls = vec![BidderCall {
            request: finalized.expect("a body inside the cap finalizes"),
            ..call("https://gz.example/rtb", 500)
        }];
        let mut composed =
            compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED).expect("composes");
        let sent = &dispatched(&mut composed, 0).request;
        assert_eq!(
            sent.body().get(..2),
            Some(&[0x1f_u8, 0x8b_u8][..]),
            "a body inside the cap is still gzip-encoded",
        );
        let mut decoded = Vec::new();
        GzDecoder::new(sent.body())
            .read_to_end(&mut decoded)
            .expect("and round-trips");
        assert_eq!(decoded, plain);
    }

    /// An oversized PLAIN body is refused at exchange finalization, exactly as a
    /// compressed one is.
    ///
    /// A request with no declared compression is its own final wire
    /// body, so `max_outbound_request_bytes` is the ceiling it has to
    /// clear — the encoder is simply the identity. The compressed arm
    /// was measured through the cap and the plain arm was returned
    /// unmeasured, so an oversized uncompressed body reached the
    /// transport and the host refused it there, rendering an
    /// uncategorised 999 instead of the code 5 the spec assigns.
    ///
    /// The caps here are deliberately UNEQUAL. The shipped rows set the
    /// raw and encoded ceilings to the same number, and a body that
    /// crosses both is refused during construction by the sink's
    /// `CappedWriter` before finalization ever sees it — so an equal-cap
    /// fixture cannot distinguish a finalize that checks the plain
    /// length from one that does not. This body sits INSIDE the raw cap
    /// and outside the encoded one, which is the only configuration
    /// where the missing check is observable.
    #[test]
    fn an_oversize_plain_body_is_refused_positionally() {
        // Inside a generous raw cap, outside the tight encoded cap
        // below: the gap between the two is where this defect lives.
        let plain_body = vec![b'x'; 4_096];
        let raw_request = outbound_request::request(
            "POST".into(),
            "https://plain.example/rtb".into(),
            vec![],
            plain_body,
            vec![],
        )
        .expect("the raw body fits the sink");
        let tight_encoded_cap = 64;
        let (sent, finalized) = finalize_for_transport(
            raw_request,
            &TransportFinalize {
                gpc: None,
                compression: None,
                max_encoded_bytes: tight_encoded_cap,
            },
        );
        assert_eq!(
            sent.url(),
            "https://plain.example/rtb",
            "the refused slot keeps its metadata"
        );
        let refusal = match finalized
            .expect_err("an over-cap plain body is refused before composition")
        {
            FinalizeError::OversizeEncoded { limit } => {
                assert_eq!(limit, tight_encoded_cap);
                FanoutError::OversizeRequest {
                    stage: OversizeRequestStage::Encoded,
                }
            }
            other @ FinalizeError::ReserveOverflow => panic!(
                "a plain body larger than the encoded cap must be this call's own positional refusal rather than a dispatch, got {other:?}"
            ),
        };
        // Asserted on the RENDERED pair, for the reason the compressed
        // row gives: an intermediate variant holds for whatever code it
        // renders, and the code is the thing a bidder's operator reads.
        let rendered = fanout_error_codes(&refusal);
        assert_eq!(
            rendered,
            (5, 100),
            "an oversized uncompressed body renders FailedToRequestBids, the same as an \
             oversized compressed one. A 999 here is the defect: the body reached the \
             transport and the host refused it as an uncategorised fan-out fault.",
        );
    }

    /// A plain body INSIDE the encoded cap still dispatches untouched.
    ///
    /// Guards the row above against passing for the wrong reason: a
    /// finalize that refused every plain request would satisfy it.
    #[test]
    fn a_plain_body_within_the_encoded_cap_still_dispatches() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let deadline = auction(&clock, 1_000);
        let plain = br#"{"id":"small","imp":[]}"#.to_vec();
        // A cap that the body clears with room to spare, still well
        // under the raw ceiling: the check refuses only what it must.
        let encoded_cap = 1_024;
        let raw_request = outbound_request::request(
            "POST".into(),
            "https://plain.example/rtb".into(),
            vec![],
            plain.clone(),
            vec![],
        )
        .expect("the raw body fits the sink");
        let (_, finalized) = finalize_for_transport(
            raw_request,
            &TransportFinalize {
                gpc: None,
                compression: None,
                max_encoded_bytes: u64::try_from(encoded_cap).expect("the cap fits"),
            },
        );
        let calls = vec![BidderCall {
            request: finalized.expect("a body inside the cap finalizes"),
            ..call("https://plain.example/rtb", 500)
        }];
        let mut composed = compose_all(calls, &deadline, entry, policy(), encoded_cap)
            .expect("a body inside the cap composes");
        let sent = &dispatched(&mut composed, 0).request;
        assert_eq!(
            sent.body(),
            plain,
            "a plain body inside the encoded cap is dispatched byte for byte",
        );
    }

    /// A call with no declared compression is passed through byte
    /// for byte, and gains no encoding header.
    #[test]
    fn a_call_without_endpoint_compression_is_unchanged() {
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let entry = clock.now_monotonic();
        let deadline = auction(&clock, 1_000);
        let mut composed = compose_all(
            vec![call("https://plain.example/rtb", 500)],
            &deadline,
            entry,
            policy(),
            TEST_MAX_ENCODED,
        )
        .expect("composes");
        let sent = &dispatched(&mut composed, 0).request;
        assert_eq!(sent.body(), b"{}".to_vec(), "the body is untouched");
        assert!(
            !sent
                .headers()
                .any(|(name, _value)| name.eq_ignore_ascii_case("content-encoding")),
            "no encoding was declared, so none is added",
        );
    }

    /// A typed `Unreachable` 502 keeps its wire classification.
    ///
    /// The adapters emit this reason for connect and DNS failures, and it
    /// is error code 999 / `seatnonbid` statuscode 103 on the wire, not
    /// the 4 / 100 every other 502 gets.
    #[test]
    fn an_unreachable_bad_gateway_is_unreachable_not_a_bad_response() {
        let err = EdgeError::bad_gateway_with_reason(
            "connection refused by 10.1.2.3:443",
            BadGatewayReason::Unreachable,
        );
        assert!(
            matches!(
                classify_edge(&err, TimeoutCause::Bidder),
                Err(FanoutError::Transport(TransportError {
                    kind: TransportErrorKind::Unreachable,
                }))
            ),
            "a typed Unreachable 502 retains its category (999 / 103)",
        );
    }

    /// Received-answer failures and initial transport failures share 4/100,
    /// but round-147 #6 keeps their typed categories distinct.
    #[test]
    fn bad_gateway_reasons_keep_response_and_initial_transport_distinct() {
        use auctioneer_exchange::debug::metadata::DebugError;

        for reason in [
            BadGatewayReason::Decode(BadGatewayDecodeReason::Brotli),
            BadGatewayReason::Decode(BadGatewayDecodeReason::Gzip),
            BadGatewayReason::Decode(BadGatewayDecodeReason::Json),
            BadGatewayReason::Protocol,
            BadGatewayReason::Transport,
            BadGatewayReason::Unspecified,
        ] {
            let err = EdgeError::bad_gateway_with_reason("upstream detail", reason);
            let outcome = classify_edge(&err, TimeoutCause::Bidder);
            assert_eq!(
                DebugError::from_transport(outcome.as_ref().unwrap_err()),
                DebugError::Transport,
                "a collapsed decode/protocol failure must not be guessed back into decode",
            );
            if matches!(
                reason,
                BadGatewayReason::Transport | BadGatewayReason::Unspecified
            ) {
                assert!(
                    matches!(
                        outcome,
                        Err(FanoutError::Transport(TransportError {
                            kind: TransportErrorKind::Initial,
                        }))
                    ),
                    "{reason:?} is an initial transport failure (4 / 100)"
                );
            } else {
                assert!(
                    matches!(outcome, Err(FanoutError::BadServerResponse)),
                    "{reason:?} is a received-answer failure (4 / 100)"
                );
            }
        }
    }

    /// The classification reads the TYPED reason, never the
    /// message, so no upstream text can reach the verdict.
    ///
    /// Two errors carrying the same misleading message classify by their
    /// reasons alone and land in different places.
    #[test]
    fn the_bad_gateway_verdict_ignores_the_upstream_message() {
        let misleading = "unreachable: decode failed";
        let unreachable =
            EdgeError::bad_gateway_with_reason(misleading, BadGatewayReason::Unreachable);
        let decode = EdgeError::bad_gateway_with_reason(
            misleading,
            BadGatewayReason::Decode(BadGatewayDecodeReason::Gzip),
        );
        assert!(
            matches!(
                classify_edge(&unreachable, TimeoutCause::Bidder),
                Err(FanoutError::Transport(TransportError {
                    kind: TransportErrorKind::Unreachable,
                }))
            ),
            "the reason decides, not the text",
        );
        assert!(
            matches!(
                classify_edge(&decode, TimeoutCause::Bidder),
                Err(FanoutError::BadServerResponse)
            ),
            "the same text with a different reason classifies differently",
        );
    }

    // -----------------------------------------------------------------
    // Scheduler TIMING. Each row below advances a real clock and then
    // asserts on a figure the scheduler reports, because each defect
    // these cover was a correct-looking value sampled at the wrong
    // moment. A row that only checked an outcome would pass against all
    // three.
    // -----------------------------------------------------------------

    /// A `Timer` that ADVANCES the fake clock by whatever it was asked
    /// to sleep, then resolves.
    ///
    /// `FakeClock`'s own `Timer` parks until something calls `advance`,
    /// which no single-threaded `block_on` row can do from outside. This
    /// one makes the requested duration actually elapse, so the
    /// scheduler's deadline arithmetic is exercised against a clock that
    /// moved rather than one frozen at the origin.
    ///
    /// `Timer` is `Sync`, so the state is an `Arc<FakeClock>` and
    /// nothing else — a `Cell` of observed durations would not compile
    /// here, and no row below needs one.
    struct AdvancingTimer {
        clock: Arc<FakeClock>,
    }

    impl AdvancingTimer {
        fn new(clock: &Arc<FakeClock>) -> Self {
            Self {
                clock: Arc::clone(clock),
            }
        }
    }

    #[async_trait(?Send)]
    impl Timer for AdvancingTimer {
        async fn sleep(&self, duration: Duration) {
            self.clock.advance(duration);
        }
    }

    /// A `Timer` that resolves immediately WITHOUT advancing the clock.
    ///
    /// This is `ImmediateResolveTimer`'s behaviour, which is what all
    /// three wasm adapters install today, and it is the only thing that
    /// reaches `drive_fanout`'s unbounded fallback: every race resolves
    /// for the timer, the recheck finds nothing due because no time
    /// passed, and the re-race bound is spent on passes that made no
    /// progress. A timer that advanced to the deadline would expire the
    /// call on the FIRST race and the fallback would be unreachable —
    /// which is exactly how an earlier revision of the fallback row
    /// passed against the defect it was written for.
    struct NonAdvancingTimer;

    #[async_trait(?Send)]
    impl Timer for NonAdvancingTimer {
        async fn sleep(&self, _duration: Duration) {}
    }

    /// A `DecodeCooperator` that returns to the executor once and then
    /// advances the fake clock before resuming.
    ///
    /// Both halves matter, and they model the same thing a real
    /// cooperator does. `finish_slot` yields precisely so a
    /// frozen-clock runtime reports a fresh instant afterwards, so a
    /// yield IS a suspension point and IS where a settler's own cost
    /// lands.
    ///
    /// * the pend makes the settler genuinely OUTSTANDING for a pass, so
    ///   the batch half is reached while it is still in flight. Without
    ///   it, settler-first polling finishes every ready settler before
    ///   the next batch event is taken, and no row can put a settler
    ///   and a cutoff in flight together.
    /// * the advance makes the settlement COST something, so a figure
    ///   sampled after the drain differs from one sampled at the cutoff.
    struct CostlyCooperator {
        clock: Arc<FakeClock>,
        per_yield: Duration,
    }

    impl CostlyCooperator {
        fn new(clock: &Arc<FakeClock>, per_yield: Duration) -> Self {
            Self {
                clock: Arc::clone(clock),
                per_yield,
            }
        }
    }

    #[async_trait(?Send)]
    impl DecodeCooperator for CostlyCooperator {
        async fn yield_to_host(&self) {
            PendOnce::new().await;
            self.clock.advance(self.per_yield);
        }
    }

    /// Returns `Pending` on its first poll and `Ready` on the next,
    /// waking itself in between.
    ///
    /// A plain `futures::pending!()` never resolves; this is the
    /// one-pass suspension a yield point actually is.
    struct PendOnce {
        polled: bool,
    }

    impl PendOnce {
        const fn new() -> Self {
            Self { polled: false }
        }
    }

    impl Future for PendOnce {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.polled {
                return Poll::Ready(());
            }
            self.polled = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    /// A response carrying a coding nothing here decodes, so lifting it
    /// takes the settle path with its deadline recheck.
    fn undecodable_response(clock: &Arc<FakeClock>) -> OutboundResponse {
        let mut headers = HeaderMap::new();
        headers.insert("content-encoding", "zstd".parse().expect("valid"));
        OutboundResponse::new(
            Method::POST,
            StatusCode::OK,
            headers,
            Body::from_bytes(b"\x28\xb5\x2f\xfd\x20\x05\x29\x00\x00".to_vec()),
            monotonic(clock),
            edgezero_core::ContentTypeProvenance::Unknown,
        )
    }

    /// A cutoff's reported latency is measured to the instant the
    /// window CLOSED, not to whenever the outstanding settlers finished
    /// draining afterwards.
    ///
    /// The scheduler observes `Finished(Cutoff)` and then awaits the
    /// settlers still in flight before it publishes the unresolved
    /// slots as expiries. Reading the clock at publication charges every
    /// expired slot for a SIBLING's settlement time — a latency no call
    /// in this wave experienced, and one that grows with how slow an
    /// unrelated bidder's body was to settle.
    ///
    /// The fixture makes the two instants far apart on purpose: the
    /// cutoff is observed at 50 ms and the one outstanding settler then
    /// spends 100 ms, so a figure sampled at publication reads 150 ms
    /// against a true 50 ms. The assertion is on the NUMBER, because the
    /// outcome — a positional expiry — is identical either way, which is
    /// how a wrongly-sampled figure stays invisible to an
    /// outcome-shaped test.
    #[test]
    fn cutoff_latency_excludes_sibling_settlement_time() {
        const CUTOFF_AT_MS: u64 = 50;
        const SETTLE_MS: u64 = 100;

        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        // Slot 1's settler pends for a pass at its yield point, so it is
        // still outstanding when the cutoff is taken on the next one.
        // That is the state this row needs: the window closed at 50 ms,
        // a sibling's 100 ms of settlement still to come, and slot 0
        // unresolved throughout.
        //
        // A settler that never pends does NOT reproduce it. Settlers are
        // polled before the batch half, so a ready one finishes before
        // the next batch event is taken, and the cutoff would then be
        // observed after the 100 ms rather than before it — the two
        // sampling points would coincide and the row would pass
        // whichever one the code used.
        // Explicit source staging prevents the SDK's ready-burst drain from
        // consuming the cutoff before Auctioneer can start the sibling settler.
        let client = CostlyEventClient {
            events: Cell::new(Some(vec![
                ScriptEvent::Item(ScriptItem::new(
                    1,
                    OutboundSlotResult::new(Duration::ZERO, Ok(undecodable_response(&clock))),
                )),
                ScriptEvent::Cutoff,
            ])),
            clock: Arc::clone(&clock),
            per_event: Duration::ZERO,
        };
        let cooperator = CostlyCooperator::new(&clock, Duration::from_millis(SETTLE_MS));
        let timer = AdvancingTimer::new(&clock);
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let entry = clock.now_monotonic();
        let composed = compose_all(
            vec![
                call("https://a.example/rtb", CUTOFF_AT_MS),
                call("https://b.example/rtb", CUTOFF_AT_MS),
            ],
            &deadline,
            entry,
            policy(),
            TEST_MAX_ENCODED,
        )
        .expect("the calls compose");
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(composed.len()).collect();
        let wave = dispatchable(composed);
        let batch_entry_at = clock.now_monotonic();
        // SDK preparation captures entry now; the staged cutoff event advances
        // this same clock to the actual latest request deadline at 50 ms.
        block_on(run_wave(
            wave,
            &mut out,
            batch_entry_at,
            WaveHost {
                clock: clock.as_ref(),
                timer: &timer,
                cooperator: &cooperator,
            },
            &client,
            &limits,
        ))
        .expect("a cutoff is a normal termination");

        // The settler did run and did cost its 100 ms, or the two
        // sampling points would coincide and this row would prove
        // nothing.
        let settled_at = clock
            .now_monotonic()
            .saturating_duration_since(batch_entry_at);
        assert!(
            settled_at >= Duration::from_millis(CUTOFF_AT_MS + SETTLE_MS),
            "the fixture must leave the clock past the cutoff by the settler's own \
             cost, or the correct and incorrect sampling points are the same instant \
             and this row cannot tell them apart; it reached {settled_at:?}",
        );

        let expired = out[0]
            .as_ref()
            .expect("the unresolved slot is published")
            .result
            .elapsed;
        assert_eq!(
            expired,
            Duration::from_millis(CUTOFF_AT_MS),
            "an unresolved slot's elapsed must be the window it spent upstream, which \
             ended when the cutoff was observed at {CUTOFF_AT_MS} ms. It reported \
             {expired:?}; sampling at publication instead would report \
             {settled_at:?}, charging this call for a sibling's {SETTLE_MS} ms \
             settlement",
        );
    }

    /// Serves a scripted event sequence, ADVANCING the fake clock by a
    /// fixed amount per event delivered.
    ///
    /// This is the batch half costing time, which is what makes the
    /// polling order observable at all. An upstream that produces
    /// several terminal results does real work between them — reading
    /// bodies, decoding, bookkeeping — and `stream::iter` models the
    /// delivery but not the cost. This fixture also explicitly stages a pending
    /// poll between events; the SDK otherwise drains every ready source event.
    /// Without the cost, a settler held
    /// behind the batch half is held behind an instant that never
    /// moves, and no clock-based assertion can tell the two polling
    /// orders apart. That is exactly what an earlier revision of this
    /// row did, and it passed against the defect it was written for.
    struct CostlyEventClient {
        events: Cell<Option<Vec<ScriptEvent>>>,
        clock: Arc<FakeClock>,
        per_event: Duration,
    }

    #[async_trait(?Send)]
    impl BoundedProxyClientApi for CostlyEventClient {
        fn start_wave(
            &self,
            requests: Vec<OutboundRequest>,
            observation_cutoff: Deadline,
        ) -> Result<OutboundBatch, EdgeError> {
            let events = self.events.take().unwrap_or_default();
            Ok(prepared_driver(
                requests,
                observation_cutoff,
                &self.clock,
                DriverScript {
                    events,
                    control: Arc::new(DriverControl::new(true)),
                },
                self.per_event,
                true,
            ))
        }
    }

    /// A settler ready to finish is polled before the next batch event,
    /// so its deadline recheck reads the clock at the moment it was
    /// ready rather than after a sibling's turn.
    ///
    /// `finish_slot` rechecks its slot's deadline after it resumes, and
    /// an expiry outranks the encoding verdict. So a settler held behind
    /// a batch half that keeps producing publishes `Expired` for a call
    /// whose real verdict was `Undecodable`: at the moment it was ready
    /// the deadline had not passed, and by the time it was polled it
    /// had.
    ///
    /// This is a CLASSIFICATION defect and nothing more. The body was
    /// unusable either way — the coding is one nothing here implements —
    /// so no healthy bid is at stake. What differs is the reason the
    /// operator is given: `Expired` sends them to look at the bidder's
    /// latency, `Undecodable` to its `Content-Encoding`.
    ///
    /// The fixture: slot 0 arrives needing settlement with 50 ms of
    /// deadline left; the driver then has three more items queued, each
    /// of whose settlement costs 30 ms. Batch-first, slot 0 is not
    /// polled until all three have gone through and the clock is past
    /// its deadline. Settler-first, it settles on its own encoding.
    #[test]
    fn a_ready_settler_is_not_held_behind_the_batch_half() {
        const BIDDER_TIMEOUT_MS: u64 = 50;
        const PER_EVENT_MS: u64 = 10;
        const SLOTS: usize = 8;

        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        // Eight slots, every one needing settlement, from a driver that
        // costs `PER_EVENT_MS` per event — a buffered upstream doing
        // real work between results.
        //
        // The arithmetic the row turns on. Slot 0's result is the FIRST
        // event, so its settler is spawned at 10 ms. A settler suspends
        // once at its yield point — that is what a yield IS, and
        // `finish_slot` needs it so a frozen-clock runtime reports a
        // fresh instant — so one further batch event goes through while
        // it is pending and it settles at 20 ms. That is inside its
        // 50 ms deadline, and its verdict is the encoding.
        //
        // Batch-first, it is not polled while pending OR while ready:
        // all eight events go through first, so it settles at 80 ms and
        // its recheck finds the deadline gone.
        //
        // So the deadline has to admit two events and refuse eight,
        // which is what `the_settler_fixture_admits_a_yield_but_not_the_        // whole_batch` below states as arithmetic rather than leaving
        // to these three numbers happening to line up.
        let client = CostlyEventClient {
            events: Cell::new(Some(
                (0..SLOTS)
                    .map(|index| {
                        ScriptEvent::Item(ScriptItem::new(
                            index,
                            OutboundSlotResult::new(
                                Duration::ZERO,
                                Ok(undecodable_response(&clock)),
                            ),
                        ))
                    })
                    .collect(),
            )),
            clock: Arc::clone(&clock),
            per_event: Duration::from_millis(PER_EVENT_MS),
        };
        // The settlers themselves cost nothing: the time comes from the
        // batch half, which is the half a starved settler waits on. A
        // costly settler too would conflate the two.
        let cooperator = CostlyCooperator::new(&clock, Duration::ZERO);
        let timer = AdvancingTimer::new(&clock);
        let limits = test_limits();
        let deadline = auction(&clock, 1_000);
        let entry = clock.now_monotonic();
        let calls: Vec<BidderCall> = (0..SLOTS)
            .map(|index| {
                // Slot 0 is the fairness witness. Siblings keep the batch's
                // observation cutoff open past the full 80 ms workload.
                let timeout = if index == 0 { BIDDER_TIMEOUT_MS } else { 500 };
                call(&format!("https://b{index}.example/rtb"), timeout)
            })
            .collect();
        let composed = compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED)
            .expect("the calls compose");
        let mut out: Vec<Option<Slot>> = repeat_with(|| None).take(composed.len()).collect();
        let wave = dispatchable(composed);
        block_on(run_wave(
            wave,
            &mut out,
            clock.now_monotonic(),
            WaveHost {
                clock: clock.as_ref(),
                timer: &timer,
                cooperator: &cooperator,
            },
            &client,
            &limits,
        ))
        .expect("the wave completes");

        // Two things the fixture must be true of, or the row is vacuous.
        //
        // The run must outlast the deadline, so a misclassification is
        // possible at all:
        let elapsed = clock.now_monotonic().saturating_duration_since(entry);
        assert!(
            elapsed > Duration::from_millis(BIDDER_TIMEOUT_MS),
            "the batch half must together outlast the {BIDDER_TIMEOUT_MS} ms \
             deadline, or no slot could be misclassified as expired and this row \
             asserts nothing; it took {elapsed:?}",
        );
        // And slot 0 must be ready INSIDE its deadline, so settling it
        // on its encoding is the correct answer rather than a lucky
        // one. Two events' worth: one to deliver it, one more while its
        // yield point is suspended.
        assert!(
            PER_EVENT_MS.saturating_mul(2) < BIDDER_TIMEOUT_MS,
            "slot 0 settles after two events, at \
             {} ms, and must still be inside its {BIDDER_TIMEOUT_MS} ms deadline \
             there, or Expired would be the right verdict and this row would assert \
             the wrong one",
            PER_EVENT_MS.saturating_mul(2),
        );

        // Slot 0 was ready first, with its deadline still ahead of it.
        // It must report the encoding, not a timeout.
        let first = out[0].as_ref().expect("slot 0 is published");
        match &first.result.outcome {
            Err(BoundedSlotError::Undecodable(cause)) => assert_eq!(
                *cause,
                UndecodableEncoding::Unsupported("zstd".to_owned()),
                "the refusal names the coding upstream parsed",
            ),
            Err(BoundedSlotError::Expired) => panic!(
                "slot 0 was ready to settle at {PER_EVENT_MS} ms, inside its \
                 {BIDDER_TIMEOUT_MS} ms deadline, and published as Expired. Its \
                 settler was held behind the batch half until the remaining events \
                 had spent {elapsed:?}, so its deadline recheck read a clock that \
                 moved for other calls. The verdict is an encoding refusal; a \
                 timeout sends an operator to look at this bidder's latency instead \
                 of its Content-Encoding",
            ),
            other => panic!("slot 0 published something else entirely: {other:?}"),
        }
    }

    /// The settler row's three numbers separate the two polling orders
    /// rather than happening to agree with one.
    ///
    /// Stated as arithmetic because the row above depends on it and
    /// cannot say it: a deadline that admitted the whole batch would
    /// leave the row green under either order, and one that refused
    /// even a yield would leave it red under both. Only the window
    /// between those two tells them apart. Changing any of the three
    /// constants fails HERE, with the reason, rather than quietly
    /// turning that row into a tautology.
    #[test]
    fn the_settler_fixture_admits_a_yield_but_not_the_whole_batch() {
        // Mirrors `a_ready_settler_is_not_held_behind_the_batch_half`'s
        // own constants; a drift between the two shows up as this row
        // disagreeing with that one's behaviour.
        const BIDDER_TIMEOUT_MS: u64 = 50;
        const PER_EVENT_MS: u64 = 10;
        const SLOTS: u64 = 8;

        // Settler-first: the result arrives on event 1 and the settler
        // is suspended across event 2, so it settles at two events.
        let settler_first_at = PER_EVENT_MS.saturating_mul(2);
        // Batch-first: every event goes through before any settler is
        // polled.
        let batch_first_at = PER_EVENT_MS.saturating_mul(SLOTS);
        assert!(
            settler_first_at < BIDDER_TIMEOUT_MS,
            "settling after a yield lands at {settler_first_at} ms and must be inside \
             the {BIDDER_TIMEOUT_MS} ms deadline, or the correct order would also \
             report Expired and the row cannot fail on the defect",
        );
        assert!(
            batch_first_at > BIDDER_TIMEOUT_MS,
            "settling after the whole batch lands at {batch_first_at} ms and must be \
             OUTSIDE the {BIDDER_TIMEOUT_MS} ms deadline, or the defective order \
             would also report the encoding and the row would pass against it",
        );
    }

    /// After the unbounded fallback acquisition, expiry is rechecked
    /// before a batch is started.
    ///
    /// The raced acquisition is followed by a recheck. The FALLBACK — the
    /// bare `acquire_wave().await` the loop drops to once the re-race
    /// bound is spent — was not, so however long that wait took, the
    /// loop went straight on to dispatch. And because the batch's entry
    /// instant is read after the acquisition, the slot it dispatched
    /// reported about 0 ms spent: an expired call shown as instant.
    ///
    /// The fixture spends the re-race bound against a timer that makes
    /// each race's sleep genuinely elapse, so by the time the fallback
    /// is reached the one pending call is 80 ms past a 50 ms deadline.
    /// What must come out is a positional expiry carrying the real
    /// elapsed figure, and no batch at all.
    #[test]
    fn the_fallback_acquisition_rechecks_expiry_before_dispatching() {
        const BIDDER_TIMEOUT_MS: u64 = 50;
        /// How long the fallback acquisition waits before it resolves.
        /// Past the deadline, which is the whole point.
        const QUEUE_WAIT_MS: u64 = 80;

        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        // A client that must never be asked to start a batch: the call
        // is expired before any acquisition succeeds.
        let client = ScriptedClient::new(&clock, vec![]);
        let timer = NonAdvancingTimer;
        let limits = test_limits();
        let gate = StreamGate::new(1);
        let deadline = auction(&clock, 1_000);
        let outcome = block_on(async {
            // Hold the gate's only permit, so every acquisition in the
            // scheduler has to wait.
            let held = gate
                .acquire_wave(1)
                .await
                .expect("one permit fits the gate");
            let fanout = drive_fanout(
                vec![call("https://a.example/rtb", BIDDER_TIMEOUT_MS)],
                &deadline,
                WaveHost {
                    clock: clock.as_ref(),
                    timer: &timer,
                    cooperator: &NoopDecodeCooperator,
                },
                &client,
                &limits,
                &gate,
            );
            let mut scheduler = Box::pin(fanout);
            // Spend the re-race bound against a timer that resolves
            // without advancing — so every race is unproductive and the
            // loop drops to its unbounded fallback. THEN advance the
            // clock past the call's deadline and release the permit, so
            // the fallback resolves late, with the call already expired.
            //
            // That ordering is the whole fixture. The permit must come
            // back AFTER the loop is parked on the fallback, or the
            // fallback is never reached and the row proves nothing.
            let releasing = async {
                // `PendOnce`, not `futures::pending!()`: the latter
                // parks without registering a waker, so once the
                // scheduler is parked on the gate nothing wakes this
                // side and the two deadlock. A self-waking pend keeps
                // the pass counter turning.
                for _ in 0..=MAX_UNPRODUCTIVE_RACES {
                    PendOnce::new().await;
                }
                clock.advance(Duration::from_millis(QUEUE_WAIT_MS));
                drop(held);
            };
            let mut release = Box::pin(releasing);
            poll_fn(|cx| {
                let _releasing = release.as_mut().poll(cx);
                scheduler.as_mut().poll(cx)
            })
            .await
        });

        // The dispatch count is checked FIRST and on its own, because
        // it is the observation that distinguishes the two
        // implementations. Without the recheck the loop starts a batch
        // for the expired call, and what that produces downstream
        // depends on the client — this one has no scripted events, so
        // it surfaces as a driver fault rather than as anything about
        // deadlines. Asserting on the published result alone would
        // report that fault instead of the defect.
        assert_eq!(
            client.dispatched.get(),
            0,
            "no batch may be started for a call that expired while the fallback \
             acquisition waited {QUEUE_WAIT_MS} ms against a {BIDDER_TIMEOUT_MS} ms \
             deadline. Constructing a batch is not evidence a network send happened \
             in time; only a recheck after the acquisition is",
        );

        let results = outcome.expect("the scheduler publishes every position");
        assert_eq!(results.len(), 1, "one published result per input call");
        let reported = results[0].elapsed;
        assert!(
            reported >= Duration::from_millis(BIDDER_TIMEOUT_MS),
            "the expired call must report the time it actually waited, at least its \
             own {BIDDER_TIMEOUT_MS} ms deadline and in this fixture the full \
             {QUEUE_WAIT_MS} ms the fallback acquisition took. It reported \
             {reported:?} — and 0 ms is what a figure sampled after the acquisition \
             gives, because batch_entry_at is read once the wait is over",
        );
        assert!(
            results[0].result.is_err(),
            "a call that never reached the network is a failure, not a result",
        );
    }

    mod entry_deadline {
        use super::{
            Arc, BidderCall, BoundedSlotError, Composed, Compression, Duration, FakeClock,
            FanoutError, NoopDecodeCooperator, Pin, ScriptedClient, StreamGate, TEST_MAX_ENCODED,
            TimeoutCause, Timer, TransportFinalize, WaveHost, assert_gate_is_idle, auction,
            block_on, call, completed_pair, compose_all, dispatched, drive_fanout,
            finalize_for_transport, policy, test_limits,
        };
        use auctioneer_runtime::Clock as _;
        use std::future::Future;
        use std::sync::Mutex;

        fn gzip_finalized_call(url: &str, timeout_ms: u64) -> BidderCall {
            let fixture = call(url, timeout_ms);
            let (_, finalized) = finalize_for_transport(
                fixture.request,
                &TransportFinalize {
                    gpc: None,
                    compression: Some(Compression::Gzip),
                    max_encoded_bytes: u64::try_from(TEST_MAX_ENCODED).expect("test cap fits"),
                },
            );
            let request = finalized.expect("gzip fixture finalizes before fanout");
            assert_eq!(
                request.body().get(..2),
                Some(b"\x1f\x8b".as_slice()),
                "the fixture is already gzip-encoded before composition",
            );
            BidderCall { request, ..fixture }
        }

        fn assert_composition_entry_expiry(now_ms: u64) {
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            clock.advance(Duration::from_millis(now_ms));
            let entry = clock.now_monotonic();
            let deadline = auction(&clock, 50);
            let calls = vec![gzip_finalized_call("https://expired.example/rtb", 500)];
            let composed = compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED)
                .expect("entry expiry is positional, not an outer error");
            assert_eq!(composed.len(), 1);
            match &composed[0] {
                Composed::Refused {
                    error,
                    deadline: selected,
                    decode,
                } => {
                    assert!(
                        matches!(error, BoundedSlotError::Expired),
                        "an already-finalized gzip call must expire positionally, got {error:?}",
                    );
                    assert_eq!(selected.deadline_at, deadline.deadline_at);
                    assert_eq!(selected.selected_cause, TimeoutCause::AuctionDeadline);
                    assert_eq!(decode.max_decoded_bytes, policy().max_decoded_bytes);
                }
                Composed::Dispatch(_) => panic!("an already-due call is terminal at composition"),
            }
        }

        #[test]
        fn composition_pre_expired_auction_refuses_already_finalized_gzip() {
            assert_composition_entry_expiry(51);
        }

        #[test]
        fn composition_at_auction_deadline_refuses_already_finalized_gzip() {
            assert_composition_entry_expiry(50);
        }

        #[test]
        fn composition_pre_expired_auction_skips_deadline_overflow() {
            for now_ms in [50, 51] {
                let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
                clock.advance(Duration::from_millis(now_ms));
                let entry = clock.now_monotonic();
                let deadline = auction(&clock, 50);
                assert!(
                    entry.checked_add(Duration::MAX).is_none(),
                    "fixture must overflow"
                );
                let calls = vec![BidderCall {
                    timeout: Duration::MAX,
                    ..gzip_finalized_call("https://expired.example/rtb", 500)
                }];
                let composed = compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED)
                    .expect("expired auction must return before composing an overflowing budget");
                assert_eq!(composed.len(), 1);
                assert!(matches!(
                    &composed[0],
                    Composed::Refused {
                        error: BoundedSlotError::Expired,
                        ..
                    },
                ));
                assert_eq!(
                    composed[0].deadline().selected_cause,
                    TimeoutCause::AuctionDeadline
                );
            }
        }

        #[test]
        fn composition_zero_timeout_refuses_finalized_gzip_without_reordering_live_siblings() {
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let entry = clock.now_monotonic();
            let deadline = auction(&clock, 1_000);
            let calls = vec![
                call("https://a.example/rtb", 100),
                BidderCall {
                    max_decoded_bytes: 17,
                    ..gzip_finalized_call("https://expired.example/rtb", 0)
                },
                call("https://b.example/rtb", 50),
            ];
            let mut composed = compose_all(calls, &deadline, entry, policy(), TEST_MAX_ENCODED)
                .expect("one due call must not fail healthy siblings");
            assert_eq!(composed.len(), 3);
            match &composed[1] {
                Composed::Refused {
                    error,
                    deadline: selected,
                    decode,
                } => {
                    assert!(
                        matches!(error, BoundedSlotError::Expired),
                        "a zero-timeout finalized gzip call must expire positionally, got {error:?}",
                    );
                    assert_eq!(selected.deadline_at, entry);
                    assert_eq!(selected.selected_cause, TimeoutCause::Bidder);
                    assert_eq!(
                        decode.max_decoded_bytes, 17,
                        "per-call policy survives expiry"
                    );
                }
                Composed::Dispatch(_) => panic!("a zero-timeout call is already expired"),
            }
            for (index, url, timeout_ms) in [
                (0, "https://a.example/rtb", 100),
                (2, "https://b.example/rtb", 50),
            ] {
                let healthy = dispatched(&mut composed, index);
                assert_eq!(
                    healthy.request.url(),
                    url,
                    "healthy input positions never reorder"
                );
                assert_eq!(healthy.request.body(), b"{}");
                assert_eq!(
                    healthy.deadline.deadline_at,
                    entry
                        .checked_add(Duration::from_millis(timeout_ms))
                        .expect("test horizon"),
                );
            }
        }

        struct CreationCountingTimer<'a> {
            clock: &'a FakeClock,
            created: Mutex<Vec<Duration>>,
        }

        impl<'a> CreationCountingTimer<'a> {
            fn new(clock: &'a FakeClock) -> Self {
                Self {
                    clock,
                    created: Mutex::new(Vec::new()),
                }
            }

            fn durations(&self) -> Vec<Duration> {
                self.created.lock().expect("test timer lock").clone()
            }
        }

        // Implement the async-trait signature directly: counting in an async
        // body misses a sleep future created and dropped by a ready gate arm.
        impl Timer for CreationCountingTimer<'_> {
            fn sleep<'life0, 'async_trait>(
                &'life0 self,
                duration: Duration,
            ) -> Pin<Box<dyn Future<Output = ()> + 'async_trait>>
            where
                'life0: 'async_trait,
                Self: 'async_trait,
            {
                self.created.lock().expect("test timer lock").push(duration);
                Box::pin(async move { self.clock.sleep(duration).await })
            }
        }

        fn assert_fanout_entry_creates_no_timer_or_batch(now_ms: u64) {
            let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
            clock.advance(Duration::from_millis(now_ms));
            let timer = CreationCountingTimer::new(&clock);
            let client = ScriptedClient::new(&clock, vec![]);
            let limits = test_limits();
            let gate = StreamGate::new(2);
            let deadline = auction(&clock, 50);
            let results = block_on(drive_fanout(
                vec![
                    gzip_finalized_call("https://a.example/rtb", 500),
                    gzip_finalized_call("https://b.example/rtb", 250),
                ],
                &deadline,
                WaveHost {
                    clock: clock.as_ref(),
                    timer: &timer,
                    cooperator: &NoopDecodeCooperator,
                },
                &client,
                &limits,
                &gate,
            ))
            .expect("entry expiry is positional");
            assert_eq!(
                client.dispatched.get(),
                0,
                "no batch submission for expired calls"
            );
            assert_gate_is_idle(&gate);
            assert_eq!(results.len(), 2, "exactly one result per input call");
            for result in results {
                assert_eq!(result.elapsed, Duration::ZERO);
                assert!(matches!(
                    result.result,
                    Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline))
                ));
            }
            assert_eq!(
                timer.durations(),
                Vec::<Duration>::new(),
                "expiry must be checked before even creating a sleep future",
            );
        }

        #[test]
        fn fanout_pre_expired_auction_creates_no_timer_or_batch_for_finalized_gzip() {
            assert_fanout_entry_creates_no_timer_or_batch(51);
        }

        #[test]
        fn fanout_at_auction_deadline_creates_no_timer_or_batch_for_finalized_gzip() {
            assert_fanout_entry_creates_no_timer_or_batch(50);
        }

        #[test]
        fn fanout_zero_timeout_never_arms_timer_or_submits_finalized_gzip_sibling() {
            let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
            let timer = CreationCountingTimer::new(&clock);
            let client = ScriptedClient::new(&clock, completed_pair(&clock));
            let limits = test_limits();
            let gate = StreamGate::new(3);
            let deadline = auction(&clock, 1_000);
            let results = block_on(drive_fanout(
                vec![
                    call("https://a.example/rtb", 100),
                    gzip_finalized_call("https://expired.example/rtb", 0),
                    call("https://b.example/rtb", 50),
                ],
                &deadline,
                WaveHost {
                    clock: clock.as_ref(),
                    timer: &timer,
                    cooperator: &NoopDecodeCooperator,
                },
                &client,
                &limits,
                &gate,
            ))
            .expect("an expired member must not fail healthy siblings");
            assert_eq!(client.dispatched.get(), 1, "one wave, no retries");
            assert_eq!(
                client.slot_count.get(),
                2,
                "only healthy siblings are submitted"
            );
            assert_gate_is_idle(&gate);
            assert_eq!(
                results.len(),
                3,
                "exactly one terminal result per original position"
            );
            assert_eq!(results[0].result.as_ref().unwrap().status(), 200);
            assert_eq!(results[0].elapsed, Duration::from_millis(10));
            assert!(matches!(
                results[1].result,
                Err(FanoutError::Timeout(TimeoutCause::Bidder))
            ));
            assert_eq!(results[1].elapsed, Duration::ZERO);
            assert_eq!(results[2].result.as_ref().unwrap().status(), 200);
            assert_eq!(results[2].elapsed, Duration::from_millis(20));
            assert_eq!(
                timer.durations(),
                vec![Duration::from_millis(50)],
                "only healthy budgets create acquisition timers, never the zero-timeout member",
            );
        }
    }
}

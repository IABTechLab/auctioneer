//! The `app!` configuration callback.
//!
//! `edgezero_core::app!` invokes [`configure_app`] once per `App` BUILD,
//! not once per process. On a serverless adapter that can be once per
//! request. Everything here must therefore be:
//!
//! - **per-`App`**, and
//! - **idempotent by construction** — each step overwrites a field on
//!   the `App` being built, so running it twice leaves the same state as
//!   running it once, with no accumulation and no counter to drift.
//!
//! PROCESS-GLOBAL state — the boot envelope and the concurrency gates —
//! deliberately does NOT live here. Gates bound concurrency across the
//! whole isolate, so installing them per build would give each `App` its
//! own private capacity and bound nothing. They are installed once by
//! `bootstrap_adapter_with_envelope` and this callback only READS them.
//!
//! # What this installs
//!
//! - **The bounded error renderer.** The host renders framework and
//!   handler errors on auctioneer's behalf, and its default renderer
//!   interpolates the error's `message` — which for an unmatched route
//!   is the whole caller-supplied path. That makes the response size a
//!   function of caller input and echoes caller bytes back onto the
//!   wire. auctioneer's renderer emits a `&'static str` for every error, so
//!   the size is a compile-time fact. See
//!   [`error_renderer`](crate::host_primitives::error_renderer) for why
//!   it is a two-arm match rather than one.
//! - **The detached-egress decision factory.** Pre-admission
//!   normalized-head failures — 400, 414, 431 — are emitted BEFORE
//!   application admission runs, so they never reach an admission
//!   decision and have no permit. The factory is the seam that gives
//!   them one: it returns a completion that owns a static-class permit,
//!   and it ABORTS when that class is exhausted, so a saturated isolate
//!   resets the connection rather than emitting ungated. Without this,
//!   these responses would sit outside the egress accounting entirely.
//! - **The ingress admission policy.** The one callback the host runs
//!   before it owns a request body. It is what binds a route-class
//!   egress permit to the response through terminal egress — without it
//!   the host's own default admits every request with an empty grant and
//!   an empty completion, so a handler finds no lease and whatever
//!   permit it holds releases at handler return rather than at terminal
//!   egress. It belongs here rather than in the boot sequence for the
//!   same reason the renderer does: it is an `App` FIELD, so installing
//!   it is an idempotent overwrite, and the process-global gates it
//!   reads are installed elsewhere and only read from here. See
//!   [`ingress`](crate::ingress).
//! - **The fallback response-egress policy.** The host consults it for
//!   every response whose budget nothing else set. Without it those
//!   responses inherit the host's portable default, an order of
//!   magnitude wider than any auctioneer egress class is sized against.
//!   The bounded fallback drain is why it is needed: its 400 and 408
//!   answers travel as `BufferedIngressResponse`, which carries no
//!   extensions, so they cannot carry a deadline of their own while
//!   still holding an aux permit. The policy preserves an explicit
//!   deadline wherever one exists. See [`fallback_egress_policy`].
//! - **The typed-config extraction limits.** The config loader runs
//!   while the request already holds its admission lease, in a phase
//!   neither the ingress-read deadline nor the egress write deadline
//!   covers. Without a bound, enough stalled store lookups exhaust
//!   auction capacity with no auctioneer allocation to blame. auctioneer
//!   INSTALLS its value here rather than wrapping the read in a local
//!   timer: the host already applies one deadline across the root blob
//!   and every secret it walks, so a local race would leave that
//!   deadline in force for the reads auctioneer did not wrap and compete
//!   with a budget that already exists. See [`extraction_limits`].
//!
//! # What is deliberately still absent
//!
//! **Independent secret-byte measurements.** The envelope supplies the
//! backend/envelope and blob byte caps and the JSON value-count cap.
//! Secret limits retain SDK defaults. The total read allowance is kept
//! at least as large as the SDK default and either individual value cap;
//! it is not a new measured memory bound.
//!
//! The installed limits are live on Apps configured after boot.
//! All four adapters now call
//! `bootstrap_adapter_with_envelope`, so none of them reaches this code
//! with `boot_limits()` unset by choice. Axum's row validates and the
//! extraction caps resolve from it.
//!
//! Cloudflare, Fastly and Spin currently declare worst-case demand above
//! their ceilings. That specific budget error WARNS and still installs
//! normalized caps and gates; other validation errors remain fatal.
//! Serving therefore does not establish worst-case fit. An App built
//! before boot retains SDK extraction defaults even if boot is installed
//! later: this callback captures per-App limits rather than a live view
//! of the process-global envelope.
//!
//! The TIMEOUT is unaffected either way, because it is baked rather
//! than envelope-derived, which is why the fetch bound is live on all
//! four targets.

use crate::handlers::egress_deadline::REFUSAL_WRITE_BUDGET;
use crate::host_primitives::error_renderer::render_error_response;
use crate::host_primitives::gates::{boot_limits, boot_target};
use crate::host_primitives::static_egress::detached_egress_decision;
use crate::ingress::decide as decide_admission;
use auctioneer_bidder::schema::{SchemaArtifactState, schema_artifact_state};
use auctioneer_config::RUNTIME_BOOTSTRAP;
use auctioneer_runtime::BudgetError;
use edgezero_core::app::App;
use edgezero_core::config_store::ConfigExtractionLimits;
use edgezero_core::error::EdgeError;
use edgezero_core::ingress::IngressHeadLimits;
use edgezero_core::response_egress::{ResponseEgressHead, ResponseEgressPolicy};
use edgezero_core::time::{Deadline, MonotonicInstant};
use std::time::Duration;

/// Apply auctioneer's per-`App` configuration.
///
/// Runs on every `App` build. Idempotent: it sets fields rather than
/// accumulating into them, so repeated builds converge on the same
/// configuration.
///
/// # Errors
/// Returns the host's typed startup-policy error when the extraction
/// limits are rejected. The baked fetch bound is checked for being
/// nonzero at BUILD time, and the byte caps come from an envelope the
/// boot sequence already validated, so a rejection here means the two
/// validators disagree — which is a boot failure, not something to
/// serve through.
pub fn configure_app(app: &mut App) -> Result<(), EdgeError> {
    if schema_artifact_state() == SchemaArtifactState::Unqualified {
        return Err(EdgeError::internal(BudgetError::UnqualifiedSchemaArtifact));
    }
    // Eager registry startup remains deferred until the live join has precharged storage.
    // App construction is also supported before boot and must not materialize bidders.
    // All three are plain field overwrites on the `App` being built,
    // which is what makes repeated builds idempotent without a guard:
    // the second build installs the same values over the first build's.
    //
    // The closure on the renderer is an ownership adapter and nothing
    // more: the host hands it an owned error, and auctioneer's renderer
    // only reads one.
    app.set_error_response_renderer(|error| render_error_response(&error));
    app.set_detached_response_egress_decision_factory(detached_egress_decision);
    app.set_response_egress_policy(fallback_egress_policy);
    app.set_ingress_admission_policy(decide_admission);
    app.set_config_extraction_limits(extraction_limits())?;
    // The head bound the envelope's memory model already charges for.
    // Without this call the host validates heads at ITS defaults
    // (65_536 header bytes, 8_192 target bytes) while the budget
    // reserves against the envelope's own figures -- a request the
    // accounting never funded is admitted.
    if let Some(limits) = head_limits()? {
        app.set_ingress_head_limits(limits);
    }
    Ok(())
}

/// The write policy for every response whose budget nothing else set.
///
/// A response that carries its own [`ResponseEgressDeadline`] keeps it:
/// the host resolves that extension into
/// `ResponseEgressHead::application_deadline`, and an explicit budget a
/// handler or a refusal chose is a decision, not a default to override.
///
/// Everything else gets the short refusal budget instead of the host's
/// portable default, which is an order of magnitude wider than any
/// auctioneer egress class is sized against. The responses that land here
/// are the ones no auctioneer wrapper touched, and the buffered ingress
/// terminals are the reason this seam is needed at all: the bounded
/// fallback drain answers 400 or 408 through
/// `BufferedIngressResponse`, which carries status, headers and body
/// and NO extensions, so there is no response to attach a deadline to.
/// Those answers bypass every handler wrapper while still holding an
/// aux permit, and without a policy here they would hold it for the
/// host's full default under a pending write.
///
/// The budget is the existing `REFUSAL_WRITE_BUDGET` rather than a new
/// envelope field, which keeps this a policy decision about a static
/// class of small fixed bodies rather than a new tunable: every
/// response reaching the fallback arm is one of those.
fn fallback_egress_policy(
    head: &ResponseEgressHead<'_>,
    egress_started_at: MonotonicInstant,
) -> ResponseEgressPolicy {
    if let Some(explicit) = head.application_deadline() {
        return ResponseEgressPolicy {
            write_deadline: explicit,
        };
    }
    // Anchored at the host's egress start, which is the instant the
    // permit actually begins being held — the thing the budget bounds.
    // Saturating rather than wrapping: a clock near the end of its
    // domain yields an already-due deadline, which sheds, where a wrap
    // would yield a far-future one that never does.
    let deadline = egress_started_at
        .checked_add(REFUSAL_WRITE_BUDGET)
        .unwrap_or(egress_started_at);
    ResponseEgressPolicy {
        write_deadline: Deadline::at_instant(deadline),
    }
}

/// The ingress head bound this target's envelope already charges for.
///
/// `None` when no envelope was installed: a target that opted out of
/// the memory model has no figures to install, and the host's defaults
/// are then the only bound there is.
///
/// The two header figures are the ones the envelope charges at the
/// ingress seam, so admitting more than either is admitting a request
/// the budget never funded.
///
/// Boot retains `TargetPlatformConstants` alongside the envelope, so
/// its request-target cap is installed with the two header limits.
///
/// # Errors
/// Returns the host's typed startup-policy error when a figure is
/// zero. Envelope validation already rejects a zero in any of these
/// fields, so a rejection here means the two validators disagree.
fn head_limits() -> Result<Option<IngressHeadLimits>, EdgeError> {
    let Some(limits) = boot_limits() else {
        return Ok(None);
    };
    let Some(target) = boot_target() else {
        return Ok(None);
    };
    let installed = IngressHeadLimits::default()
        .with_max_request_header_bytes(u64::from(limits.max_admitted_header_bytes))?
        .with_max_request_header_count(u64::from(limits.max_admitted_header_count))?
        .with_max_request_target_bytes(u64::from(target.max_request_target_bytes))?;
    Ok(Some(installed))
}

/// The extraction limits auctioneer installs, from the baked fetch bound and
/// whatever byte and node caps the boot envelope declares.
///
/// The TIMEOUT is always auctioneer's: it is a baked constant, available
/// before any envelope is, and it is the whole point of this seam. The
/// host turns it into an absolute deadline anchored at EXTRACTION entry
/// -- `started_at + timeout` inside its own extraction budget -- so the
/// bound covers the store read and the secret walk, not the upload or
/// the parse that precede them. That is narrower than a whole-request
/// bound and is stated rather than hidden: the value exists to stop a
/// hung store from hanging a request that is holding admission
/// capacity, and it still does exactly that.
///
/// The BYTE CAPS follow the envelope. With no envelope installed the
/// host's defaults stand, for the same reason bounded serialization
/// leaves its cap off in that state: a byte cap is a memory reservation
/// the envelope makes, so with no envelope there is no reservation to
/// respect and no measured number to enforce. That is not the
/// fail-closed reading the gate accessors take, because there is no
/// capacity here to over-admit -- only a ceiling on how much one read may
/// allocate.
///
/// `max_total_bytes` has to cover the largest single value the caps
/// admit or the host rejects the set, so it is raised to that maximum
/// rather than left below it. A cap the envelope leaves at ZERO keeps the
/// host's default for the same reason: the envelope validator permits a
/// zero runtime-config cap while the host's rejects one, and a set the
/// host rejects would fail boot for an envelope that was otherwise
/// valid. Zero is read as "unmeasured", not as "admit nothing" -- an
/// extraction cap admitting zero bytes could never read any config at
/// all, so it can only be the absence of a figure.
///
/// The NODE CAP follows the same rule: a nonzero declaration is installed
/// unchanged, while zero or an absent envelope keeps the host's default.
/// Boot charges and floor-checks the resulting effective cap, independently
/// of the byte caps.
fn extraction_limits() -> ConfigExtractionLimits {
    extraction_limits_for(boot_limits())
}

/// Shared with boot accounting so the effective config caps are funded.
/// Nonzero byte and node declarations are preserved; zero resolves to the
/// host's default. Boot normalizes its retained row to these effective caps
/// before deriving charges and checking floors.
pub(crate) fn extraction_limits_for(
    envelope: Option<&auctioneer_runtime::ResolvedIsolateLimits>,
) -> ConfigExtractionLimits {
    let defaults = ConfigExtractionLimits::default();
    let timeout = Duration::from_millis(u64::from(RUNTIME_BOOTSTRAP.max_config_fetch_ms));
    let Some(limits) = envelope else {
        return ConfigExtractionLimits {
            timeout,
            ..defaults
        };
    };
    let measured = |envelope_cap: u64, fallback: u64| {
        if envelope_cap == 0 {
            fallback
        } else {
            envelope_cap
        }
    };
    let max_blob_bytes = measured(
        limits.max_runtime_config_blob_bytes,
        defaults.max_blob_bytes,
    );
    // The raw-envelope cap is what the BACKEND is allowed to hand back:
    // the extractor sees the envelope bytes before it sees the config
    // inside, so this is the figure that bounds the transfer.
    let max_backend_bytes = measured(limits.max_runtime_config_bytes, defaults.max_backend_bytes);
    ConfigExtractionLimits {
        max_backend_bytes,
        max_blob_bytes,
        max_json_nodes: if limits.max_config_json_nodes == 0 {
            defaults.max_json_nodes
        } else {
            limits.max_config_json_nodes
        },
        max_total_bytes: defaults
            .max_total_bytes
            .max(max_blob_bytes)
            .max(defaults.max_secret_bytes),
        timeout,
        ..defaults
    }
}

#[cfg(test)]
mod tests {
    use super::{REFUSAL_WRITE_BUDGET, extraction_limits_for, fallback_egress_policy};
    use auctioneer_runtime::ResolvedIsolateLimits;
    use edgezero_core::config_store::ConfigExtractionLimits;
    use edgezero_core::http::{HeaderMap, StatusCode, Version};
    use edgezero_core::response_egress::ResponseEgressHead;
    use edgezero_core::time::{Deadline, MonotonicInstant};
    use std::time::Duration;

    fn limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 4 << 30,
            baseline_bytes: 96_000_000,
            inbound_request_bytes: 0x0004_0000,
            inbound_parse_factor: auctioneer_runtime::MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: 0x0004_0000,
            max_outbound_raw_bytes: 0x0004_0000,
            max_transport_bytes: 0x0004_0000,
            max_decoded_bytes: 0x0004_0000,
            response_parse_factor: auctioneer_runtime::MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: auctioneer_runtime::MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 0x0100_0000,
            egress_permits: 3,
            response_write_deadline_ms: 30_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 5,
            aux_route_peak_bytes: auctioneer_runtime::min_aux_route_peak_bytes(
                1 << 20,
                1 << 20,
                8_192,
            ),
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 4,
            gvl_parsed_bytes: auctioneer_runtime::MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 2,
            max_outbound_requests: 2,
            max_concurrent_auctions: 2,
            max_active_body_streams: 2,
            max_runtime_config_bytes: 0x0010_0000,
            max_runtime_config_blob_bytes: 0x0010_0000,
            max_parsed_nodes: 0x0001_0000,
            max_copied_header_bytes: 4_096,
            max_admitted_header_bytes: 0x4000,
            max_admitted_header_count: 128,
            max_impressions_per_auction: 100,
            max_config_json_nodes: 8_192,
            terminal_response_permits: 68,
            refusal_body_bytes: 256,
            transmit_state_bytes: 4_096,
        }
    }

    #[test]
    fn extraction_node_cap_honors_every_nonzero_declaration() {
        let default_nodes = ConfigExtractionLimits::default().max_json_nodes;
        for nodes in [
            1,
            default_nodes.saturating_div(2),
            default_nodes,
            default_nodes * 2,
            u32::MAX,
        ] {
            let mut envelope = limits();
            envelope.max_config_json_nodes = nodes;
            let extraction = extraction_limits_for(Some(&envelope));
            assert_eq!(extraction.max_json_nodes, nodes);
            // SDK validation checks representation, including u32::MAX;
            // it does not impose a semantic application-policy maximum.
            assert_eq!(extraction.validate().unwrap(), extraction);
            assert_eq!(envelope.max_config_json_nodes, nodes);
        }
    }

    #[test]
    fn extraction_zero_caps_and_absent_envelope_use_sdk_defaults() {
        let defaults = ConfigExtractionLimits::default();
        let absent = extraction_limits_for(None);
        assert_eq!(absent.max_json_nodes, defaults.max_json_nodes);
        assert_eq!(absent.max_backend_bytes, defaults.max_backend_bytes);
        assert_eq!(absent.max_blob_bytes, defaults.max_blob_bytes);
        let mut envelope = limits();
        envelope.max_config_json_nodes = 0;
        envelope.max_runtime_config_bytes = 0;
        envelope.max_runtime_config_blob_bytes = 0;
        assert_eq!(extraction_limits_for(Some(&envelope)), absent);
        assert_eq!(absent.validate().unwrap(), absent);
    }

    #[test]
    fn extraction_declared_caps_keep_total_and_unmeasured_limits_valid() {
        let defaults = ConfigExtractionLimits::default();
        let mut envelope = limits();
        envelope.max_config_json_nodes = defaults.max_json_nodes * 2;
        envelope.max_runtime_config_bytes = 0x0001_0000;
        envelope.max_runtime_config_blob_bytes = defaults.max_total_bytes + 1;
        let extraction = extraction_limits_for(Some(&envelope));
        assert_eq!(extraction.max_json_nodes, envelope.max_config_json_nodes);
        assert_eq!(
            extraction.max_backend_bytes,
            envelope.max_runtime_config_bytes
        );
        assert_eq!(
            extraction.max_blob_bytes,
            envelope.max_runtime_config_blob_bytes
        );
        assert_eq!(
            extraction.max_total_bytes,
            envelope.max_runtime_config_blob_bytes
        );
        assert_eq!(extraction.max_json_depth, defaults.max_json_depth);
        assert_eq!(extraction.max_secret_bytes, defaults.max_secret_bytes);
        assert_eq!(extraction.validate().unwrap(), extraction);
    }

    /// A head with no application deadline, standing for every response
    /// no auctioneer wrapper attached a budget to — the buffered ingress
    /// drain terminals among them.
    fn unattached_head(headers: &HeaderMap) -> ResponseEgressHead<'_> {
        ResponseEgressHead::new(
            StatusCode::REQUEST_TIMEOUT,
            Version::HTTP_11,
            headers,
            None,
            MonotonicInstant::now(),
            None,
        )
    }

    /// An unattached response is bounded by the refusal budget, not by
    /// the host's much wider portable default.
    ///
    /// Asserted as a span from the egress start rather than against a
    /// literal, so the row follows the constant if it is ever retuned.
    #[test]
    fn an_unattached_response_gets_the_short_refusal_budget() {
        let headers = HeaderMap::new();
        let started_at = MonotonicInstant::now();
        let policy = fallback_egress_policy(&unattached_head(&headers), started_at);
        let expected = started_at
            .checked_add(REFUSAL_WRITE_BUDGET)
            .expect("the refusal budget is a small span");
        // Compared through `Debug`, the way the write-budget anchor
        // tests do: `Deadline` is deliberately not `PartialEq`.
        assert_eq!(
            format!("{:?}", policy.write_deadline),
            format!("{:?}", Deadline::at_instant(expected)),
            "an unattached response is bounded by the refusal budget",
        );
    }

    /// An explicit budget is a decision, so the fallback preserves it
    /// instead of overwriting it with its own.
    #[test]
    fn an_explicit_deadline_survives_the_fallback() {
        let headers = HeaderMap::new();
        let started_at = MonotonicInstant::now();
        // Deliberately WIDER than the refusal budget: a fallback that
        // ignored the extension would narrow this, and a fallback that
        // always won would be invisible against a narrower one.
        let chosen = Deadline::at_instant(
            started_at
                .checked_add(REFUSAL_WRITE_BUDGET + Duration::from_secs(5))
                .expect("a few seconds past the budget"),
        );
        let head = ResponseEgressHead::new(
            StatusCode::OK,
            Version::HTTP_11,
            &headers,
            Some(chosen),
            started_at,
            None,
        );
        assert_eq!(
            format!(
                "{:?}",
                fallback_egress_policy(&head, started_at).write_deadline
            ),
            format!("{chosen:?}"),
            "an explicitly chosen write deadline is preserved exactly",
        );
    }
}

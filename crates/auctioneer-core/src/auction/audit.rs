//! Bounded, aggregate-only request disclosure observation.

use crate::handlers::auction::execution_frame_bytes;
use crate::host_primitives::error_renderer::MAX_RENDERED_BODY_BYTES;
use auctioneer_exchange::construction_costs as exchange_construction_costs;
use auctioneer_exchange::scrub::audit::{ChangeKind, DisclosureAudit};
use auctioneer_runtime::RequestConstructionCosts;
use std::fmt::{self, Write as _};
use std::mem::size_of;
use std::str::from_utf8;

const DISCLOSURE_LOG_TARGET: &str = "auctioneer::request_disclosure";
const SUMMARY_TEXT_BYTES: usize = 512;
// Parser tuple, handler local, auction context and bidder info each allow
// eight bytes of layout growth for the heap-free original root ext marker.
const ORIGINAL_REQUEST_EXT_SHAPE_RESERVE_BYTES: u64 = 32;

// Check the four-owner reserve on every compiled ABI, not only native tests.
const _: () = {
    use auctioneer_bidder::BidderRequestInfo;
    use auctioneer_exchange::AuctionContext;
    use auctioneer_runtime::bounded_json::JsonFieldShape;
    use std::mem::{align_of, needs_drop};
    assert!(
        size_of::<JsonFieldShape>() == 1,
        "wire shape must remain one byte"
    );
    assert!(
        !needs_drop::<JsonFieldShape>(),
        "wire shape must not need destruction"
    );
    assert!(
        align_of::<(u32, JsonFieldShape)>() <= 8,
        "parser result outgrew its shape reserve"
    );
    assert!(
        align_of::<AuctionContext<'_>>() <= 8,
        "auction context outgrew its shape reserve"
    );
    assert!(
        align_of::<BidderRequestInfo<'_>>() <= 8,
        "bidder info outgrew its shape reserve"
    );
};

/// How the charged auction work ended, independent of its HTTP status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionStatus {
    Success,
    Error,
    Cancelled,
}

impl CompletionStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Cancelled => "cancelled",
        }
    }
}

/// One auction's plain audit owner, created only after resource prepayment.
///
/// Store before the permit and receipts in `AuctionFunding` so cancellation
/// observes before their release. Finalization observes once without releasing
/// either owner.
pub struct AuctionDisclosureScope {
    audit: DisclosureAudit,
    finished: bool,
}

impl AuctionDisclosureScope {
    #[must_use]
    pub fn new() -> Self {
        Self {
            audit: DisclosureAudit::default(),
            finished: false,
        }
    }

    /// Borrow the aggregate for exchange construction and mutation.
    pub fn audit_mut(&mut self) -> &mut DisclosureAudit {
        &mut self.audit
    }

    /// Observe once. Further calls and subsequent drop cannot publish again.
    pub fn finish(&mut self, status: CompletionStatus) {
        if self.finished {
            return;
        }
        // Mark first so a panicking installed sink cannot cause a second log on drop.
        self.finished = true;
        observe_request_disclosure(&self.audit, status);
    }
}

impl Default for AuctionDisclosureScope {
    fn default() -> Self {
        Self::new()
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for AuctionDisclosureScope {
    fn drop(&mut self) {
        self.finish(CompletionStatus::Cancelled);
    }
}

/// Complete charged construction layouts installed by each host adapter.
///
/// Add core ownership/observation and fixed wire-shape storage to exchange's
/// audit and mutation scratch once, independently of outbound call count.
/// The prepared-state and failure boxes may coexist in the HTTP future;
/// their payloads and that future's inline receipt handles are distinct
/// storage. The shared ledger allocation is counted once, not per receipt.
///
/// # Panics
/// Panics if adding the actual layout sizes overflows the construction costs.
#[must_use]
#[expect(
    clippy::as_conversions,
    reason = "actual layout byte sizes on the native and wasm32 targets fit u64"
)]
#[expect(
    clippy::panic,
    reason = "layout overflow must reject const evaluation rather than wrap charged construction costs"
)]
pub const fn construction_costs() -> RequestConstructionCosts {
    // Constant evaluation rejects underflow; the scope contains this audit.
    const SCOPE_OVERHEAD_BYTES: usize =
        size_of::<AuctionDisclosureScope>() - size_of::<DisclosureAudit>();
    let exchange = exchange_construction_costs();
    let Some(audit_ownership_bytes) = exchange
        .fixed_per_auction_bytes
        .checked_add(SCOPE_OVERHEAD_BYTES as u64)
    else {
        panic!("request disclosure ownership layout overflow");
    };
    let Some(mut fixed_per_auction_bytes) =
        audit_ownership_bytes.checked_add(ORIGINAL_REQUEST_EXT_SHAPE_RESERVE_BYTES)
    else {
        panic!("original request ext shape layout overflow");
    };
    let ledger = match auctioneer_runtime::AuctionBudget::shared_owner_layout() {
        Ok(layout) => layout.size(),
        Err(_) => panic!("shared auction ledger layout overflow"),
    };
    let owners = [
        size_of::<super::operation::PreparedState>(),
        super::result::failure_storage_bytes(),
        MAX_RENDERED_BODY_BYTES,
        execution_frame_bytes(),
        ledger,
    ];
    let mut remaining = owners.as_slice();
    while let Some((owner, rest)) = remaining.split_first() {
        let Some(complete) = fixed_per_auction_bytes.checked_add(*owner as u64) else {
            panic!("owned auction construction layout overflow");
        };
        fixed_per_auction_bytes = complete;
        remaining = rest;
    }
    let Some(reusable_scratch_bytes) = exchange
        .reusable_scratch_bytes
        .checked_add(size_of::<ObservationBuffer>() as u64)
    else {
        panic!("request disclosure scratch layout overflow");
    };
    RequestConstructionCosts {
        fixed_per_auction_bytes,
        reusable_scratch_bytes,
        retained_per_scope_bytes: exchange.retained_per_scope_bytes,
    }
}

/// Deliver a fixed numeric summary to the boot-installed sink without payloads.
///
/// Debug filtering suppresses the log, not completion of the owning scope.
pub fn observe_request_disclosure(audit: &DisclosureAudit, status: CompletionStatus) {
    #[cfg(test)]
    tests::record_observation_event();

    let summary = DisclosureSummary {
        counts: audit.counts(),
        attempted_scopes: audit.attempted_scopes,
        failed_scopes: audit.failed_scopes,
        saturated: audit.saturated,
        status,
    };
    if !log::log_enabled!(target: DISCLOSURE_LOG_TARGET, log::Level::Debug) {
        return;
    }
    let mut buffer = ObservationBuffer::new(summary);
    match buffer.format() {
        Ok(record) => log::debug!(target: DISCLOSURE_LOG_TARGET, "{record}"),
        Err(_format) => log::debug!(
            target: DISCLOSURE_LOG_TARGET,
            "request_disclosure status={} summary_format_failed=1",
            status.label()
        ),
    }
}

struct DisclosureSummary {
    counts: [u64; ChangeKind::COUNT],
    attempted_scopes: u64,
    failed_scopes: u64,
    saturated: bool,
    status: CompletionStatus,
}

// Charge the summary snapshot as well as its text, length and layout padding.
struct ObservationBuffer {
    summary: DisclosureSummary,
    text: SummaryText,
}

impl ObservationBuffer {
    const fn new(summary: DisclosureSummary) -> Self {
        Self {
            summary,
            text: SummaryText::new(),
        }
    }

    fn format(&mut self) -> Result<&str, fmt::Error> {
        let DisclosureSummary {
            counts:
                [
                    id,
                    geo,
                    coarsen,
                    fpd,
                    tid,
                    unknown,
                    malformed,
                    coppa_removed,
                ],
            attempted_scopes,
            failed_scopes,
            saturated,
            status,
        } = &self.summary;
        self.text.len = 0;
        write!(
            self.text,
            "request_disclosure status={} attempted_scopes={attempted_scopes} \
             failed_scopes={failed_scopes} id={id} geo={geo} coarsen={coarsen} \
             fpd={fpd} tid={tid} unknown={unknown} malformed={malformed} \
             coppa_removed={coppa_removed} saturated={}",
            status.label(),
            u8::from(*saturated)
        )?;
        self.text.as_str()
    }
}

struct SummaryText {
    bytes: [u8; SUMMARY_TEXT_BYTES],
    len: usize,
}

impl SummaryText {
    const fn new() -> Self {
        Self {
            bytes: [0; SUMMARY_TEXT_BYTES],
            len: 0,
        }
    }

    fn as_str(&self) -> Result<&str, fmt::Error> {
        let written = self.bytes.get(..self.len).ok_or(fmt::Error)?;
        from_utf8(written).map_err(|_utf8| fmt::Error)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "provided write_char and write_fmt delegate to the bounded write_str without allocating"
)]
impl fmt::Write for SummaryText {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len.checked_add(s.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_exchange::scrub::audit::ChangeKind;
    use auctioneer_runtime::{AdmissionGate, AuctionBudget};
    use std::cell::RefCell;
    use std::future::{Future as _, pending};
    use std::sync::{Arc, Weak};
    use std::task::{Context, Poll, Waker};

    fn zero_record(status: &str) -> String {
        format!(
            "request_disclosure status={status} attempted_scopes=0 failed_scopes=0 \
             id=0 geo=0 coarsen=0 fpd=0 tid=0 unknown=0 malformed=0 coppa_removed=0 saturated=0"
        )
    }

    fn assert_record(expected: &str) {
        testing_logger::validate(|captured| {
            assert_eq!(captured.len(), 1, "one aggregate observation");
            let record = &captured[0];
            assert_eq!(record.level, log::Level::Debug);
            assert_eq!(record.target, DISCLOSURE_LOG_TARGET);
            assert_eq!(record.body, expected);
        });
    }

    #[test]
    fn observer_emits_each_completion_status_through_installed_logger() {
        for (status, label) in [
            (CompletionStatus::Success, "success"),
            (CompletionStatus::Error, "error"),
            (CompletionStatus::Cancelled, "cancelled"),
        ] {
            testing_logger::setup();
            observe_request_disclosure(&DisclosureAudit::default(), status);
            assert_record(&zero_record(label));
        }
    }

    #[test]
    fn scope_starts_with_plain_zero_counters() {
        testing_logger::setup();
        let mut scope = AuctionDisclosureScope::new();
        assert_eq!(scope.audit_mut().counts(), [0; ChangeKind::COUNT]);
        assert_eq!(scope.audit_mut().attempted_scopes, 0);
        assert_eq!(scope.audit_mut().failed_scopes, 0);
        scope.finish(CompletionStatus::Success);
        assert_record(&zero_record("success"));
    }

    #[test]
    fn zero_change_error_observes_no_request_payload() {
        testing_logger::setup();
        let mut scope = AuctionDisclosureScope::new();
        scope.finish(CompletionStatus::Error);
        drop(scope);
        assert_record(&zero_record("error"));
    }

    #[test]
    fn repeated_finish_and_drop_observe_only_first_completion() {
        testing_logger::setup();
        let mut scope = AuctionDisclosureScope::new();
        scope.finish(CompletionStatus::Success);
        scope.finish(CompletionStatus::Error);
        scope.finish(CompletionStatus::Cancelled);
        drop(scope);
        assert_record(&zero_record("success"));
    }

    #[test]
    fn unfinished_scope_observes_cancellation_once() {
        testing_logger::setup();
        let scope = AuctionDisclosureScope::new();
        drop(scope);
        assert_record(&zero_record("cancelled"));
    }

    #[test]
    fn observer_keeps_attempted_and_failed_scope_counts() {
        testing_logger::setup();
        let mut scope = AuctionDisclosureScope::new();
        scope.audit_mut().attempted_scopes = 11;
        scope.audit_mut().failed_scopes = 3;
        scope.finish(CompletionStatus::Error);
        assert_record(&zero_record("error").replace(
            "attempted_scopes=0 failed_scopes=0",
            "attempted_scopes=11 failed_scopes=3",
        ));
    }

    #[test]
    fn fixed_summary_formats_each_counter_and_saturation() {
        let mut buffer = ObservationBuffer::new(DisclosureSummary {
            counts: [1, 2, 3, 4, 5, 6, 7, 8],
            attempted_scopes: 11,
            failed_scopes: 3,
            saturated: true,
            status: CompletionStatus::Error,
        });
        assert_eq!(
            buffer.format().expect("fixed summary fits"),
            "request_disclosure status=error attempted_scopes=11 failed_scopes=3 \
             id=1 geo=2 coarsen=3 fpd=4 tid=5 unknown=6 malformed=7 coppa_removed=8 saturated=1"
        );
    }

    #[test]
    fn observer_delivers_actual_exchange_enum_counts_without_schema_dump() {
        testing_logger::setup();
        let mut scope = AuctionDisclosureScope::new();
        for (kind, count) in [
            (ChangeKind::Id, 1_u64),
            (ChangeKind::Geo, 2),
            (ChangeKind::Coarsen, 3),
            (ChangeKind::Fpd, 4),
            (ChangeKind::Tid, 5),
            (ChangeKind::Unknown, 6),
            (ChangeKind::Malformed, 7),
            (ChangeKind::Coppa, 8),
        ] {
            for _ in 0..count {
                scope.audit_mut().record(kind);
            }
        }
        scope.audit_mut().attempted_scopes = 11;
        scope.audit_mut().failed_scopes = 3;
        scope.finish(CompletionStatus::Error);
        drop(scope);
        assert_record(
            "request_disclosure status=error attempted_scopes=11 failed_scopes=3 \
             id=1 geo=2 coarsen=3 fpd=4 tid=5 unknown=6 malformed=7 coppa_removed=8 saturated=0",
        );
    }

    #[test]
    fn coppa_removals_have_a_dedicated_summary_counter() {
        testing_logger::setup();
        let mut scope = AuctionDisclosureScope::new();
        scope.audit_mut().record(ChangeKind::Coppa);
        scope.finish(CompletionStatus::Success);
        drop(scope);
        assert_record(&zero_record("success").replace("coppa_removed=0", "coppa_removed=1"));
    }

    #[test]
    fn observer_delivers_exchange_saturation_and_maximum_counts() {
        testing_logger::setup();
        let mut audit = DisclosureAudit::default();
        for kind in [
            ChangeKind::Id,
            ChangeKind::Geo,
            ChangeKind::Coarsen,
            ChangeKind::Fpd,
            ChangeKind::Tid,
            ChangeKind::Unknown,
            ChangeKind::Malformed,
            ChangeKind::Coppa,
        ] {
            audit.record(kind);
        }
        for _ in 0..64_u32 {
            let earlier = audit.clone();
            audit.merge(&earlier);
        }
        audit.attempted_scopes = u64::MAX;
        audit.failed_scopes = u64::MAX;
        observe_request_disclosure(&audit, CompletionStatus::Error);
        testing_logger::validate(|captured| {
            assert_eq!(captured.len(), 1);
            assert_eq!(captured[0].level, log::Level::Debug);
            assert_eq!(captured[0].target, DISCLOSURE_LOG_TARGET);
            assert_eq!(
                captured[0].body.matches("18446744073709551615").count(),
                ChangeKind::COUNT + 2
            );
            assert!(captured[0].body.len() <= SUMMARY_TEXT_BYTES);
            assert!(captured[0].body.ends_with("saturated=1"));
        });
    }

    #[test]
    fn fixed_buffer_fits_maximum_width_numbers_for_every_status() {
        for status in [
            CompletionStatus::Success,
            CompletionStatus::Error,
            CompletionStatus::Cancelled,
        ] {
            for saturated in [false, true] {
                let mut buffer = ObservationBuffer::new(DisclosureSummary {
                    counts: [u64::MAX; ChangeKind::COUNT],
                    attempted_scopes: u64::MAX,
                    failed_scopes: u64::MAX,
                    saturated,
                    status,
                });
                assert_eq!(buffer.text.bytes.len(), 512);
                let record = buffer.format().expect("all maximum-width numbers fit");
                assert!(record.len() <= SUMMARY_TEXT_BYTES);
                assert_eq!(
                    record.len(),
                    match status {
                        CompletionStatus::Success => 341,
                        CompletionStatus::Error => 339,
                        CompletionStatus::Cancelled => 343,
                    }
                );
                assert_eq!(
                    record.matches("18446744073709551615").count(),
                    ChangeKind::COUNT + 2
                );
                assert!(
                    record.starts_with(&format!("request_disclosure status={} ", status.label()))
                );
                assert!(record.ends_with(if saturated {
                    "saturated=1"
                } else {
                    "saturated=0"
                }));
            }
        }
    }

    #[test]
    fn formatting_overflow_is_refused_without_partial_append() {
        use std::fmt::Write as _;
        let mut text = SummaryText::new();
        let full = "x".repeat(SUMMARY_TEXT_BYTES);
        text.write_str(&full).expect("exact bound fits");
        assert!(text.write_str("x").is_err());
        assert_eq!(text.as_str().expect("valid text"), full);
    }

    #[test]
    fn original_request_ext_shape_has_fixed_auction_construction_reserve() {
        let costs = construction_costs();
        let exchange = exchange_construction_costs();
        let audit_ownership =
            u64::try_from(size_of::<AuctionDisclosureScope>() - size_of::<DisclosureAudit>())
                .unwrap();
        let existing_fixed = exchange
            .fixed_per_auction_bytes
            .checked_add(audit_ownership)
            .unwrap()
            .checked_add(owned_layout_bytes())
            .unwrap();
        assert_eq!(
            costs.fixed_per_auction_bytes.checked_sub(existing_fixed),
            Some(32),
            "original root ext shape storage is funded even without outbound calls"
        );
    }

    #[test]
    fn construction_charge_includes_owned_preparation_and_retained_funding() {
        let costs = construction_costs();
        let prior = exchange_construction_costs().fixed_per_auction_bytes
            + u64::try_from(size_of::<AuctionDisclosureScope>() - size_of::<DisclosureAudit>())
                .unwrap()
            + 32;
        let additional = costs.fixed_per_auction_bytes.checked_sub(prior).unwrap();
        let prepared = size_of::<super::super::operation::PreparedState>();
        let receipts = size_of::<[auctioneer_runtime::Reservation<'static>; 6]>();
        assert!(
            additional >= u64::try_from(prepared + receipts).unwrap(),
            "owned preparation and receipt storage must be explicitly charged: added={additional} prepared={prepared} receipts={receipts}"
        );
        assert_eq!(additional, owned_layout_bytes());
        #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
        assert_eq!(additional, 19_144, "reviewed native owner layout snapshot");
    }

    fn owned_layout_bytes() -> u64 {
        [
            size_of::<super::super::operation::PreparedState>(),
            super::super::result::failure_storage_bytes(),
            MAX_RENDERED_BODY_BYTES,
            execution_frame_bytes(),
            AuctionBudget::shared_owner_layout().unwrap().size(),
        ]
        .into_iter()
        .map(|bytes| u64::try_from(bytes).unwrap())
        .try_fold(0_u64, u64::checked_add)
        .unwrap()
    }

    #[test]
    fn construction_costs_charge_actual_scope_overhead_and_observer_buffer() {
        let costs = construction_costs();
        let exchange = exchange_construction_costs();
        assert_eq!(
            costs.fixed_per_auction_bytes,
            exchange.fixed_per_auction_bytes
                + u64::try_from(size_of::<AuctionDisclosureScope>() - size_of::<DisclosureAudit>())
                    .expect("native and wasm layouts fit u64")
                + 32
                + owned_layout_bytes()
        );
        assert_eq!(
            costs.reusable_scratch_bytes,
            exchange.reusable_scratch_bytes
                + u64::try_from(size_of::<ObservationBuffer>())
                    .expect("native and wasm layouts fit u64")
        );
        assert_eq!(
            costs.retained_per_scope_bytes,
            exchange.retained_per_scope_bytes
        );
        assert_eq!(costs.retained_per_scope_bytes, 2 * 39);
        assert!(costs.fixed_per_auction_bytes > 0);
        assert!(
            costs.reusable_scratch_bytes
                >= u64::try_from(SUMMARY_TEXT_BYTES).expect("buffer bound fits")
        );
    }

    std::thread_local! {
        static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
        static FUNDING_OBSERVATION: RefCell<Option<FundingObservation>> = const { RefCell::new(None) };
    }

    struct FundingObservation {
        budget: Weak<AuctionBudget>,
        gate: Arc<AdmissionGate>,
        charged: u64,
        count: usize,
    }

    pub(super) fn record_observation_event() {
        EVENTS.with(|events| events.borrow_mut().push("observed"));
        FUNDING_OBSERVATION.with(|slot| {
            if let Some(observation) = slot.borrow_mut().as_mut() {
                let budget = observation
                    .budget
                    .upgrade()
                    .expect("receipts still own the ledger");
                assert_eq!(budget.in_use(), observation.charged);
                assert_eq!(observation.gate.in_use(), 1);
                assert!(observation.gate.try_acquire().is_none());
                observation.count = observation
                    .count
                    .checked_add(1)
                    .expect("observation count fits");
            }
        });
    }

    struct HeldResource(&'static str);

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for HeldResource {
        fn drop(&mut self) {
            EVENTS.with(|events| events.borrow_mut().push(self.0));
        }
    }

    fn assert_release_order() {
        EVENTS.with(|events| {
            assert_eq!(
                *events.borrow(),
                ["observed", "lease_released", "reservation_released"]
            );
            events.borrow_mut().clear();
        });
    }

    fn assert_funded_field_order() {
        // Match real declarations, not ownership claims in comments.
        for (source, declaration, fields) in [
            (
                include_str!("services.rs"),
                "pub struct AuctionFunding {",
                &[
                    "audit: AuctionDisclosureScope,",
                    "_permit: AdmissionPermit,",
                    "receipts: [Reservation<'static>; 6],",
                ][..],
            ),
            (
                include_str!("operation.rs"),
                "pub struct PreparedAuction<'host> {",
                &["state: Box<PreparedState>,", "funding: AuctionFunding,"][..],
            ),
            (
                include_str!("result.rs"),
                "struct FundedFailure {",
                &["reason: AuctionFailureReason,", "_funding: AuctionFunding,"][..],
            ),
            (
                include_str!("result.rs"),
                "pub struct CompletedAuction {",
                &["outcome: AuctionOutcome,", "funding: AuctionFunding,"][..],
            ),
            (
                include_str!("result.rs"),
                "pub struct CompletedResponse {",
                &["response: BidResponse,", "funding: AuctionFunding,"][..],
            ),
        ] {
            let mut lines = source
                .lines()
                .skip_while(|line| line.trim() != declaration)
                .skip(1)
                .take_while(|line| line.trim() != "}");
            for field in fields {
                assert!(
                    lines.any(|line| line.trim() == *field),
                    "{declaration} must drop {field} in funded ownership order"
                );
            }
        }
    }

    #[test]
    fn handler_drops_disclosure_owner_before_charged_resources() {
        use crate::auction::{
            AuctionFunding, AuctionResourceInputs, AuctionResourceProfile, SchemaAllowance,
        };
        use crate::test_support::boot_parity_fixture::profile_for_adapter;
        use auctioneer_runtime::{AdmissionGate, AuctionBudget};
        use std::sync::Arc;

        let handler = include_str!("../handlers/auction.rs");
        let prepay = handler
            .find("AuctionFunding::prepay(")
            .expect("owned funding");
        let config = handler
            .find("config_runtime::resolve_runtime_config(&ctx).await")
            .expect("configuration extraction");
        assert!(prepay < config, "prepay before configuration acquisition");
        assert_funded_field_order();

        testing_logger::setup();
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
        let charged = profile.derived().auction_charges.total().unwrap();
        assert!(charged > 0);
        let budget = Arc::new(AuctionBudget::new(charged));
        let weak = Arc::downgrade(&budget);
        let gate = AdmissionGate::new(1);
        let funding =
            AuctionFunding::prepay(&profile, &budget, gate.try_acquire().unwrap()).unwrap();
        assert_eq!(budget.in_use(), charged);
        assert_eq!(gate.in_use(), 1);
        FUNDING_OBSERVATION.with(|observation| {
            *observation.borrow_mut() = Some(FundingObservation {
                budget: Weak::clone(&weak),
                gate: Arc::clone(&gate),
                charged,
                count: 0,
            });
        });
        drop(budget);
        drop(funding);
        FUNDING_OBSERVATION.with(|observation| {
            assert_eq!(observation.borrow_mut().take().unwrap().count, 1);
        });
        assert!(weak.upgrade().is_none());
        assert_eq!(gate.in_use(), 0);
        assert!(gate.try_acquire().is_some());
        EVENTS.with(|events| {
            assert_eq!(*events.borrow(), ["observed"]);
            events.borrow_mut().clear();
        });
        assert_record(&zero_record("cancelled"));
    }

    #[test]
    fn explicit_completion_observes_before_resources_are_released() {
        testing_logger::setup();
        let reservation = HeldResource("reservation_released");
        let lease = HeldResource("lease_released");
        let mut scope = AuctionDisclosureScope::new();
        scope.finish(CompletionStatus::Success);
        drop(scope);
        drop(lease);
        drop(reservation);
        assert_release_order();
        assert_record(&zero_record("success"));
    }

    #[test]
    fn suspended_future_cancellation_observes_before_resources_are_released() {
        testing_logger::setup();
        let mut work = Box::pin(async {
            let _reservation = HeldResource("reservation_released");
            let _lease = HeldResource("lease_released");
            let _scope = AuctionDisclosureScope::new();
            pending::<()>().await;
        });
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(work.as_mut().poll(&mut context), Poll::Pending);
        EVENTS.with(|events| assert!(events.borrow().is_empty()));
        drop(work);
        assert_release_order();
        assert_record(&zero_record("cancelled"));
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn disabled_debug_logging_still_completes_and_releases_resources() {
        use std::env::{current_exe, var_os};
        use std::process::Command;

        const CHILD_ENV: &str = "AUCTIONEER__TEST__DISCLOSURE_DISABLED_CHILD";
        // The log facade filter is process-global; isolate it from parallel tests.
        if var_os(CHILD_ENV).is_none() {
            let child = Command::new(current_exe().expect("unit test executable"))
                .args(["--exact", "auction::audit::tests::disabled_debug_logging_still_completes_and_releases_resources", "--nocapture"])
                .env(CHILD_ENV, "1")
                .output()
                .expect("isolated logging test executes");
            assert!(
                child.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr)
            );
            let output = String::from_utf8_lossy(&child.stdout);
            assert!(output.contains("running 1 test"), "{output}");
            assert!(output.contains("1 passed; 0 failed"), "{output}");
            return;
        }
        testing_logger::setup();
        log::set_max_level(log::LevelFilter::Off);
        assert!(!log::log_enabled!(target: DISCLOSURE_LOG_TARGET, log::Level::Debug));
        let reservation = HeldResource("reservation_released");
        let lease = HeldResource("lease_released");
        let mut scope = AuctionDisclosureScope::new();
        scope.audit_mut().record(ChangeKind::Id);
        scope.finish(CompletionStatus::Success);
        assert!(scope.finished);
        scope.finish(CompletionStatus::Error);
        drop(scope);
        drop(lease);
        drop(reservation);
        assert_release_order();
        {
            let _reservation = HeldResource("reservation_released");
            let _lease = HeldResource("lease_released");
            let _scope = AuctionDisclosureScope::new();
        }
        assert_release_order();
        testing_logger::validate(|captured| assert!(captured.is_empty()));
        log::set_max_level(log::LevelFilter::Trace);
    }
}

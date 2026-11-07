//! Resource-retaining results and bounded, redacted failure diagnostics.

use super::{AuctionFunding, CompletionStatus};
use crate::host_primitives::error_renderer::MAX_RENDERED_BODY_BYTES;
use auctioneer_exchange::{AuctionError, AuctionOutcome};
use auctioneer_openrtb::types::bid_response::BidResponse;
use auctioneer_privacy::PrivacyError;
use auctioneer_runtime::ResourceError;
use std::{
    error::Error,
    fmt::{self, Write as _},
    iter::once,
};

/// Stable application failure categories; not a clean no-bid result.
pub enum AuctionFailureReason {
    InvalidRequest(String),
    Engine(AuctionError),
    Privacy(PrivacyError),
    Resource(ResourceError),
    Deadline,
    Unavailable(&'static str),
}

impl AuctionFailureReason {
    pub(super) fn invalid(detail: &impl fmt::Display) -> Self {
        Self::InvalidRequest(bounded_detail(detail))
    }

    fn label(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::Engine(_) => "engine",
            Self::Privacy(_) => "privacy",
            Self::Resource(_) => "resource",
            Self::Deadline => "deadline",
            Self::Unavailable(_) => "unavailable",
        }
    }

    fn bounded(self) -> Self {
        let bounded = |message: String| {
            if message.len() < MAX_RENDERED_BODY_BYTES
                && message.capacity() <= MAX_RENDERED_BODY_BYTES
            {
                message
            } else {
                bounded_detail(&message)
            }
        };
        match self {
            Self::InvalidRequest(message) => Self::InvalidRequest(bounded(message)),
            Self::Engine(error) => Self::Engine(match error {
                AuctionError::BadInput(message) => AuctionError::BadInput(bounded(message)),
                AuctionError::InvalidImpFirstPartyData(message) => {
                    AuctionError::InvalidImpFirstPartyData(bounded(message))
                }
                AuctionError::Internal(message) => AuctionError::Internal(bounded(message)),
                AuctionError::MalformedAcct(message) => {
                    AuctionError::MalformedAcct(bounded(message))
                }
                AuctionError::AccountDisabled(message) => {
                    AuctionError::AccountDisabled(bounded(message))
                }
                fixed @ (AuctionError::Resource(_) | AuctionError::FanoutUnavailable) => fixed,
            }),
            fixed @ (Self::Privacy(_)
            | Self::Resource(_)
            | Self::Deadline
            | Self::Unavailable(_)) => fixed,
        }
    }
}

impl fmt::Debug for AuctionFailureReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Charged diagnostics are destroyed before their original funding owner.
pub struct AuctionFailure {
    inner: Box<FundedFailure>,
}

struct FundedFailure {
    reason: AuctionFailureReason,
    debug_requested: bool,
    _funding: AuctionFunding,
}

pub(super) const fn failure_storage_bytes() -> usize {
    size_of::<FundedFailure>()
}

impl AuctionFailure {
    pub(super) fn new(reason: AuctionFailureReason, mut funding: AuctionFunding) -> Self {
        funding.finish(CompletionStatus::Error);
        Self {
            inner: Box::new(FundedFailure {
                reason: reason.bounded(),
                debug_requested: false,
                _funding: funding,
            }),
        }
    }

    #[must_use]
    pub fn reason(&self) -> &AuctionFailureReason {
        &self.inner.reason
    }

    pub(super) fn with_debug_requested(mut self, requested: bool) -> Self {
        self.inner.debug_requested = requested;
        self
    }

    pub(crate) fn debug_requested(&self) -> bool {
        self.inner.debug_requested
    }
}

impl fmt::Debug for AuctionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuctionFailure")
            .field("category", &self.inner.reason.label())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for AuctionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.inner.reason.label())
    }
}

impl Error for AuctionFailure {}

/// Typed results retain their admission slot through inspection and projection.
pub struct CompletedAuction {
    outcome: AuctionOutcome,
    funding: AuctionFunding,
}

impl CompletedAuction {
    pub(super) fn new(outcome: AuctionOutcome, funding: AuctionFunding) -> Self {
        Self { outcome, funding }
    }

    #[must_use]
    pub fn outcome(&self) -> &AuctionOutcome {
        &self.outcome
    }

    /// Finalize the caller's local work without releasing its retained data.
    pub fn finish(&mut self, status: CompletionStatus) {
        self.funding.finish(status);
    }

    /// Consume the original reducer result without a second selection or RNG pass.
    ///
    /// # Errors
    /// Retains original funding with any projection failure.
    pub fn into_wire_response(self) -> Result<CompletedResponse, AuctionFailure> {
        let Self { outcome, funding } = self;
        match outcome.into_bid_response() {
            Ok(response) => Ok(CompletedResponse { response, funding }),
            Err(error) => Err(AuctionFailure::new(
                AuctionFailureReason::Engine(error),
                funding,
            )),
        }
    }
}

impl fmt::Debug for CompletedAuction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletedAuction").finish_non_exhaustive()
    }
}

/// Wire data cannot be detached from the resources funding its retention.
pub struct CompletedResponse {
    response: BidResponse,
    funding: AuctionFunding,
}

impl CompletedResponse {
    #[must_use]
    pub fn response(&self) -> &BidResponse {
        &self.response
    }

    pub(crate) fn response_mut(&mut self) -> &mut BidResponse {
        &mut self.response
    }

    pub fn finish(&mut self, status: CompletionStatus) {
        self.funding.finish(status);
    }
}

// Format directly into bounded diagnostic storage, without a to_string intermediate.
fn bounded_detail(detail: &impl fmt::Display) -> String {
    struct Bounded {
        text: String,
        truncated: bool,
    }
    impl fmt::Write for Bounded {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let cap = MAX_RENDERED_BODY_BYTES.saturating_sub(4);
            let room = cap.saturating_sub(self.text.len());
            let end = s
                .char_indices()
                .map(|(index, _)| index)
                .chain(once(s.len()))
                .take_while(|index| *index <= room)
                .last()
                .unwrap_or(0);
            self.text.push_str(s.get(..end).unwrap_or(""));
            self.truncated |= end < s.len();
            Ok(())
        }
    }
    let mut output = Bounded {
        text: String::with_capacity(MAX_RENDERED_BODY_BYTES),
        truncated: false,
    };
    let _formatted = write!(&mut output, "{detail}");
    if output.truncated {
        output.text.push_str("...");
    }
    output.text
}

#[cfg(test)]
mod tests {
    use super::bounded_detail;
    use crate::auction::{
        AuctionFailure, AuctionFailureReason, AuctionFunding, AuctionResourceInputs,
        AuctionResourceProfile, SchemaAllowance,
    };
    use crate::test_support::boot_parity_fixture::profile_for_adapter;
    use auctioneer_exchange::AuctionError;
    use auctioneer_runtime::{AdmissionGate, AuctionBudget};
    use std::sync::Arc;

    #[test]
    fn retained_failure_diagnostics_are_bounded_and_never_exposed_by_debug() {
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
        let gate = AdmissionGate::new(1);
        let secret = "fake_private_detail_".repeat(1024);
        for reason in [
            AuctionFailureReason::InvalidRequest(secret.clone()),
            AuctionFailureReason::Engine(AuctionError::BadInput(secret.clone())),
            AuctionFailureReason::Engine(AuctionError::AccountDisabled(secret.clone())),
            AuctionFailureReason::Engine(AuctionError::Internal(secret.clone())),
            AuctionFailureReason::Engine(AuctionError::MalformedAcct(secret.clone())),
            AuctionFailureReason::Engine(AuctionError::InvalidImpFirstPartyData(secret.clone())),
        ] {
            let funding =
                AuctionFunding::prepay(&profile, &budget, gate.try_acquire().unwrap()).unwrap();
            let failure = AuctionFailure::new(reason, funding);
            let (AuctionFailureReason::InvalidRequest(message)
            | AuctionFailureReason::Engine(
                AuctionError::BadInput(message)
                | AuctionError::AccountDisabled(message)
                | AuctionError::Internal(message)
                | AuctionError::MalformedAcct(message)
                | AuctionError::InvalidImpFirstPartyData(message),
            )) = failure.reason()
            else {
                panic!("the same failure category must survive bounding");
            };
            assert!(
                message.len() <= 255,
                "retained diagnostic has {} bytes",
                message.len()
            );
            assert!(message.ends_with("..."));
            assert!(!format!("{failure:?}").contains("fake_private_detail"));
            assert!(!failure.to_string().contains("fake_private_detail"));
            assert!(budget.in_use() > 0);
            assert!(gate.try_acquire().is_none());
            drop(failure);
            assert_eq!(budget.in_use(), 0);
            assert_eq!(gate.in_use(), 0);
        }
    }

    #[test]
    fn bounded_formatting_keeps_short_diagnostics_and_utf8_boundaries() {
        assert_eq!(bounded_detail(&"fake short"), "fake short");
        let text = "\u{00e9}".repeat(200);
        let bounded = bounded_detail(&text);
        assert!(bounded.len() <= 255);
        assert!(bounded.ends_with("..."));
        assert_eq!(bounded, format!("{}...", "\u{00e9}".repeat(126)));
    }

    #[test]
    fn short_diagnostics_with_large_backing_allocations_are_repacked() {
        let constructors: [fn(String) -> AuctionFailureReason; 6] = [
            AuctionFailureReason::InvalidRequest,
            |message| AuctionFailureReason::Engine(AuctionError::BadInput(message)),
            |message| AuctionFailureReason::Engine(AuctionError::Internal(message)),
            |message| AuctionFailureReason::Engine(AuctionError::AccountDisabled(message)),
            |message| AuctionFailureReason::Engine(AuctionError::MalformedAcct(message)),
            |message| AuctionFailureReason::Engine(AuctionError::InvalidImpFirstPartyData(message)),
        ];
        for construct in constructors {
            let mut message = String::with_capacity(0x5000);
            message.push_str("fake_short");
            assert!(message.capacity() >= 0x5000);
            let reason = construct(message).bounded();
            let (AuctionFailureReason::InvalidRequest(retained)
            | AuctionFailureReason::Engine(
                AuctionError::BadInput(retained)
                | AuctionError::Internal(retained)
                | AuctionError::AccountDisabled(retained)
                | AuctionError::MalformedAcct(retained)
                | AuctionError::InvalidImpFirstPartyData(retained),
            )) = reason
            else {
                panic!("the diagnostic category must be preserved");
            };
            assert_eq!(retained, "fake_short");
            assert!(
                retained.capacity() <= 256,
                "retained {} bytes",
                retained.capacity()
            );
        }
    }
}

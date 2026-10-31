//! Allocation accounting for one admitted auction, separate from concurrency.
//!
//! Callers reserve measured phase charges before constructing their data and
//! keep the reservation until that data is dropped. This ledger does not
//! allocate the storage or intercept the allocator. It therefore cannot prove
//! unmeasured allocations fit, or prevent physical OOM when the configured
//! whole-isolate total exceeds the platform ceiling.

use std::alloc::Layout;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Stable categories for accounting diagnostics, never request-derived text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargeKind {
    RuntimeConfig,
    Ingress,
    Privacy,
    Projection,
    Outbound,
    Response,
}

/// Disjoint conservative charges retained for the whole auction's work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuctionCharges {
    pub config: u64,
    pub ingress: u64,
    pub privacy: u64,
    pub projection: u64,
    pub outbound: u64,
    pub response: u64,
}

impl AuctionCharges {
    /// # Errors
    /// Returns capacity overflow if the aggregate cannot be represented.
    pub fn total(&self) -> Result<u64, ResourceError> {
        [
            self.config,
            self.ingress,
            self.privacy,
            self.projection,
            self.outbound,
            self.response,
        ]
        .into_iter()
        .try_fold(0_u64, |sum, bytes| {
            sum.checked_add(bytes)
                .ok_or(ResourceError::CapacityOverflow)
        })
    }

    /// Reserve every phase before its work can allocate. An error releases
    /// all preceding reservations; a successful array must be held through
    /// response serialization, including all suspended work.
    ///
    /// # Errors
    /// Returns the first exhausted phase without leaving partial charges held.
    pub fn prepay<'a>(
        &self,
        budget: &'a AuctionBudget,
    ) -> Result<[Reservation<'a>; 6], ResourceError> {
        Ok([
            budget.try_reserve(ChargeKind::RuntimeConfig, self.config)?,
            budget.try_reserve(ChargeKind::Ingress, self.ingress)?,
            budget.try_reserve(ChargeKind::Privacy, self.privacy)?,
            budget.try_reserve(ChargeKind::Projection, self.projection)?,
            budget.try_reserve(ChargeKind::Outbound, self.outbound)?,
            budget.try_reserve(ChargeKind::Response, self.response)?,
        ])
    }
}

/// Terminal accounting failures. These are not consent syntax errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceError {
    Exhausted { kind: ChargeKind },
    CapacityOverflow,
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exhausted { .. } => "auction resource limit exceeded",
            Self::CapacityOverflow => "allocation capacity is not representable",
        })
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "remaining Error methods are deprecated or unstable on the pinned toolchain"
)]
impl Error for ResourceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// A fixed, per-auction accounting ceiling. Construction allocates nothing.
#[derive(Debug)]
pub struct AuctionBudget {
    limit: u64,
    in_use: AtomicU64,
}

impl AuctionBudget {
    #[must_use]
    pub const fn new(limit: u64) -> Self {
        Self {
            limit,
            in_use: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    #[must_use]
    pub fn in_use(&self) -> u64 {
        self.in_use.load(Ordering::Acquire)
    }

    /// Reserve before allocating; hold the returned lease for the data's life.
    ///
    /// A charge may cover several allocations, so it is an aggregate `u64`,
    /// not a single buffer capacity. Use [`checked_allocation_capacity`] at a
    /// buffer allocation site in addition to this aggregate check.
    ///
    /// # Errors
    /// Returns [`ResourceError::Exhausted`] without changing the ledger when
    /// the sum overflows or exceeds this auction's ceiling.
    pub fn try_reserve(
        &self,
        kind: ChargeKind,
        bytes: u64,
    ) -> Result<Reservation<'_>, ResourceError> {
        self.in_use
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                held.checked_add(bytes).filter(|total| *total <= self.limit)
            })
            .map_err(|_held| ResourceError::Exhausted { kind })?;
        Ok(Reservation {
            budget: self,
            bytes,
            kind,
        })
    }
}

/// A non-cloneable charge that releases on drop, including early exits and
/// cancellation. Borrowing prevents the ledger from disappearing first.
#[derive(Debug)]
#[must_use = "keep the reservation alive until the charged data is dropped"]
pub struct Reservation<'a> {
    budget: &'a AuctionBudget,
    bytes: u64,
    kind: ChargeKind,
}

impl Reservation<'_> {
    #[must_use]
    pub const fn kind(&self) -> ChargeKind {
        self.kind
    }

    /// Check a measured projection against the actual held phase receipt.
    /// This is a peak check, not another charge against the same ledger.
    ///
    /// # Errors
    /// Refuses a different phase or a peak exceeding this reservation.
    pub fn ensure_capacity(&self, kind: ChargeKind, bytes: u64) -> Result<(), ResourceError> {
        if kind != self.kind || bytes > self.bytes {
            return Err(ResourceError::Exhausted { kind });
        }
        Ok(())
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        // Only a successful checked increment constructs this non-cloneable
        // lease, so its single release cannot underflow the held total.
        self.budget.in_use.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

/// Check a byte-buffer layout before converting a configured `u64` capacity.
///
/// # Errors
/// Returns [`ResourceError::CapacityOverflow`] if the target's `usize` or
/// `isize` allocation-layout ceiling cannot represent the buffer, on native
/// and 32-bit WASM alike. Does not allocate or promise allocator success.
pub fn checked_allocation_capacity(bytes: u64) -> Result<usize, ResourceError> {
    let capacity = usize::try_from(bytes).map_err(|_overflow| ResourceError::CapacityOverflow)?;
    Layout::array::<u8>(capacity).map_err(|_layout| ResourceError::CapacityOverflow)?;
    Ok(capacity)
}

#[cfg(test)]
mod tests {
    use super::{AuctionBudget, ChargeKind, ResourceError};
    use crate::alloc_track::allocated_on_this_thread;

    #[test]
    fn accounting_itself_allocates_nothing_on_success_or_refusal() {
        let before = allocated_on_this_thread();
        let budget = AuctionBudget::new(10);
        let held = budget.try_reserve(ChargeKind::Privacy, 10).unwrap();
        assert_eq!(
            budget.try_reserve(ChargeKind::Ingress, 1).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Ingress
            }
        );
        drop(held);
        let after = allocated_on_this_thread();
        assert_eq!(after, before);
        assert_eq!(budget.in_use(), 0);
    }
}

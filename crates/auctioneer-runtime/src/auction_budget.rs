//! Allocation accounting for one admitted auction, separate from concurrency.
//!
//! Callers reserve measured phase charges before constructing their data and
//! keep the reservation until that data is dropped. This ledger does not
//! allocate the storage or intercept the allocator. It therefore cannot prove
//! unmeasured allocations fit, or prevent physical OOM when the configured
//! whole-isolate total exceeds the platform ceiling.

use std::alloc::{Layout, LayoutError};
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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

    /// Reserve every phase on the existing ledger, retaining an `Arc` per
    /// receipt so suspended work can outlive the caller's budget handle.
    /// Like [`Self::prepay`], an error releases all preceding reservations.
    /// Keep successful receipts through response serialization and suspended work.
    /// No additional allocation is needed after constructing the budget's `Arc`.
    ///
    /// # Errors
    /// Returns the first exhausted phase without leaving partial charges held.
    pub fn prepay_owned(
        &self,
        budget: &Arc<AuctionBudget>,
    ) -> Result<[Reservation<'static>; 6], ResourceError> {
        Ok([
            budget.try_reserve_owned(ChargeKind::RuntimeConfig, self.config)?,
            budget.try_reserve_owned(ChargeKind::Ingress, self.ingress)?,
            budget.try_reserve_owned(ChargeKind::Privacy, self.privacy)?,
            budget.try_reserve_owned(ChargeKind::Projection, self.projection)?,
            budget.try_reserve_owned(ChargeKind::Outbound, self.outbound)?,
            budget.try_reserve_owned(ChargeKind::Response, self.response)?,
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
    /// Requested storage for one shared ledger, including its reference counts.
    ///
    /// Rust 1.98.1 `alloc/sync.rs` uses a C-layout strong/weak atomic header,
    /// followed by the aligned payload. Allocation controls independently
    /// compare this layout with the real `Arc` allocation; receipts only clone
    /// its handle and must not multiply this storage by their count.
    ///
    /// # Errors
    /// Returns a layout error if the target cannot represent the shared owner.
    pub const fn shared_owner_layout() -> Result<Layout, LayoutError> {
        match Layout::new::<[AtomicUsize; 2]>().extend(Layout::new::<Self>()) {
            Ok((layout, _offset)) => Ok(layout.pad_to_align()),
            Err(error) => Err(error),
        }
    }

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
        Reservation::reserve(ReservationBudget::Borrowed(self), kind, bytes)
    }

    /// Reserve on this same ledger while retaining its existing `Arc`.
    /// The non-cloneable receipt can outlive the caller's budget handle;
    /// reserving and releasing do not allocate.
    ///
    /// # Errors
    /// Returns [`ResourceError::Exhausted`] without changing the ledger when
    /// the sum overflows or exceeds this auction's ceiling.
    pub fn try_reserve_owned(
        self: &Arc<Self>,
        kind: ChargeKind,
        bytes: u64,
    ) -> Result<Reservation<'static>, ResourceError> {
        Reservation::reserve(ReservationBudget::Owned(Arc::clone(self)), kind, bytes)
    }
}

#[derive(Debug)]
enum ReservationBudget<'a> {
    Borrowed(&'a AuctionBudget),
    Owned(Arc<AuctionBudget>),
}

impl ReservationBudget<'_> {
    fn as_budget(&self) -> &AuctionBudget {
        match self {
            Self::Borrowed(budget) => budget,
            Self::Owned(budget) => budget,
        }
    }
}

/// A non-cloneable charge that releases on drop, including early exits and
/// cancellation. A borrow or an owned `Arc` keeps the ledger alive until release.
#[derive(Debug)]
#[must_use = "keep the reservation alive until the charged data is dropped"]
pub struct Reservation<'a> {
    budget: ReservationBudget<'a>,
    bytes: u64,
    kind: ChargeKind,
}

impl<'a> Reservation<'a> {
    fn reserve(
        budget: ReservationBudget<'a>,
        kind: ChargeKind,
        bytes: u64,
    ) -> Result<Self, ResourceError> {
        let ledger = budget.as_budget();
        ledger
            .in_use
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                held.checked_add(bytes)
                    .filter(|total| *total <= ledger.limit)
            })
            .map_err(|_held| ResourceError::Exhausted { kind })?;
        Ok(Self {
            budget,
            bytes,
            kind,
        })
    }

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
        self.budget
            .as_budget()
            .in_use
            .fetch_sub(self.bytes, Ordering::AcqRel);
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
    use super::{
        AuctionBudget, AuctionCharges, ChargeKind, Reservation, ResourceError,
        checked_allocation_capacity,
    };
    use crate::alloc_track::allocated_on_this_thread;
    use futures::{FutureExt as _, future::pending};
    use std::iter;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Arc, Barrier};

    #[test]
    fn shared_ledger_layout_charges_one_allocation_not_each_receipt() {
        use crate::alloc_track::measure_on_this_thread;
        use std::hint::black_box;

        for limit in [0_u64, 1, u64::MAX] {
            let expected = if cfg!(target_pointer_width = "64") {
                32
            } else {
                24
            };
            let (budget, measured) =
                measure_on_this_thread(|| black_box(Arc::new(AuctionBudget::new(limit)))).unwrap();
            assert_eq!(measured.retained_bytes, expected);
            assert_eq!(measured.peak_bytes, expected);
            assert_eq!(
                u64::try_from(AuctionBudget::shared_owner_layout().unwrap().size()).unwrap(),
                expected
            );
            let charges = AuctionCharges {
                config: 0,
                ingress: 0,
                privacy: 0,
                projection: 0,
                outbound: 0,
                response: 0,
            };
            let before = allocated_on_this_thread();
            let receipts = charges.prepay_owned(&budget).unwrap();
            assert_eq!(allocated_on_this_thread(), before);
            assert_eq!(Arc::strong_count(&budget), 7);
            drop(receipts);
            assert_eq!(Arc::strong_count(&budget), 1);
            drop(budget);
        }
    }
    use std::thread;

    #[test]
    fn owned_reservations_share_the_original_ledger() {
        let budget = Arc::new(AuctionBudget::new(100));
        let borrowed = budget.try_reserve(ChargeKind::RuntimeConfig, 60).unwrap();
        let owned: Reservation<'static> = budget
            .try_reserve_owned(ChargeKind::Projection, 40)
            .unwrap();
        assert_eq!(budget.in_use(), 100);
        assert_eq!(Arc::strong_count(&budget), 2);
        assert_eq!(owned.kind(), ChargeKind::Projection);
        assert_eq!(owned.ensure_capacity(ChargeKind::Projection, 40), Ok(()));
        assert_eq!(
            owned.ensure_capacity(ChargeKind::Projection, 41),
            Err(ResourceError::Exhausted {
                kind: ChargeKind::Projection
            })
        );
        assert_eq!(
            owned.ensure_capacity(ChargeKind::Privacy, 1),
            Err(ResourceError::Exhausted {
                kind: ChargeKind::Privacy
            })
        );
        assert_eq!(
            budget
                .try_reserve_owned(ChargeKind::Outbound, 1)
                .unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Outbound
            }
        );
        assert_eq!(
            budget.try_reserve(ChargeKind::Response, 1).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Response
            }
        );
        assert_eq!(budget.in_use(), 100);
        assert_eq!(Arc::strong_count(&budget), 2);
        drop(borrowed);
        assert_eq!(budget.in_use(), 40);
        let replacement = budget.try_reserve(ChargeKind::Privacy, 60).unwrap();
        assert_eq!(budget.in_use(), 100);
        drop(owned);
        assert_eq!(budget.in_use(), 60);
        assert_eq!(Arc::strong_count(&budget), 1);
        drop(replacement);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn owned_prepayment_rolls_back_each_failed_phase() {
        let phases = [
            ChargeKind::RuntimeConfig,
            ChargeKind::Ingress,
            ChargeKind::Privacy,
            ChargeKind::Projection,
            ChargeKind::Outbound,
            ChargeKind::Response,
        ];
        let mut refused = [false; 6];
        for varied_phase in 0..6 {
            for varied_bytes in [0, 1, 17] {
                let mut amounts = [1_u64, 2, 4, 8, 16, 32];
                amounts[varied_phase] = varied_bytes;
                let charges = AuctionCharges {
                    config: amounts[0],
                    ingress: amounts[1],
                    privacy: amounts[2],
                    projection: amounts[3],
                    outbound: amounts[4],
                    response: amounts[5],
                };
                for limit in 3..=90 {
                    let budget = Arc::new(AuctionBudget::new(limit));
                    let borrowed = budget.try_reserve(ChargeKind::Privacy, 3).unwrap();
                    let mut expected_total = 3_u64;
                    let exhausted = amounts.iter().position(|bytes| {
                        expected_total = expected_total.checked_add(*bytes).unwrap();
                        expected_total > limit
                    });
                    if let Some(index) = exhausted {
                        assert_eq!(
                            charges.prepay_owned(&budget).unwrap_err(),
                            ResourceError::Exhausted {
                                kind: phases[index]
                            }
                        );
                        refused[index] = true;
                    } else {
                        let paid = charges.prepay_owned(&budget).unwrap();
                        assert_eq!(budget.in_use(), expected_total);
                        assert_eq!(Arc::strong_count(&budget), 7);
                        for ((receipt, kind), bytes) in paid.iter().zip(phases).zip(amounts) {
                            assert_eq!(receipt.kind(), kind);
                            assert_eq!(receipt.ensure_capacity(kind, bytes), Ok(()));
                            assert_eq!(
                                receipt.ensure_capacity(kind, bytes.checked_add(1).unwrap()),
                                Err(ResourceError::Exhausted { kind })
                            );
                        }
                        drop(paid);
                    }
                    assert_eq!(budget.in_use(), 3);
                    assert_eq!(Arc::strong_count(&budget), 1);
                    drop(borrowed);
                    assert_eq!(budget.in_use(), 0);
                }
            }
        }
        assert_eq!(refused, [true; 6]);
    }

    #[test]
    fn owned_receipts_survive_the_callers_budget_handle() {
        let charges = AuctionCharges {
            config: 1,
            ingress: 2,
            privacy: 4,
            projection: 8,
            outbound: 16,
            response: 32,
        };
        let budget = Arc::new(AuctionBudget::new(63));
        let weak = Arc::downgrade(&budget);
        let paid: [Reservation<'static>; 6] = charges.prepay_owned(&budget).unwrap();
        assert_eq!(budget.in_use(), 63);
        assert_eq!(Arc::strong_count(&budget), 7);
        drop(budget);
        assert_eq!(weak.strong_count(), 6);
        assert_eq!(weak.upgrade().unwrap().in_use(), 63);
        let [config, ingress, privacy, projection, outbound, response] = paid;
        drop(config);
        assert_eq!(weak.upgrade().unwrap().in_use(), 62);
        drop((ingress, privacy, projection, outbound));
        assert_eq!(weak.strong_count(), 1);
        assert_eq!(weak.upgrade().unwrap().in_use(), 32);
        drop(response);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn owned_reservations_accept_zero_and_refuse_sum_overflow() {
        for limit in [0, u64::MAX] {
            let budget = Arc::new(AuctionBudget::new(limit));
            let full = budget
                .try_reserve_owned(ChargeKind::Ingress, limit)
                .unwrap();
            let empty = budget.try_reserve_owned(ChargeKind::Privacy, 0).unwrap();
            assert_eq!(budget.in_use(), limit);
            assert_eq!(Arc::strong_count(&budget), 3);
            assert_eq!(
                budget
                    .try_reserve_owned(ChargeKind::Outbound, 1)
                    .unwrap_err(),
                ResourceError::Exhausted {
                    kind: ChargeKind::Outbound
                }
            );
            assert_eq!(
                budget.try_reserve(ChargeKind::Response, 1).unwrap_err(),
                ResourceError::Exhausted {
                    kind: ChargeKind::Response
                }
            );
            drop(empty);
            assert_eq!(budget.in_use(), limit);
            assert_eq!(Arc::strong_count(&budget), 2);
            drop(full);
            assert_eq!(budget.in_use(), 0);
            assert_eq!(Arc::strong_count(&budget), 1);
        }
        let budget = Arc::new(AuctionBudget::new(u64::MAX));
        let charges = AuctionCharges {
            config: u64::MAX,
            ingress: 1,
            privacy: 0,
            projection: 0,
            outbound: 0,
            response: 0,
        };
        assert_eq!(
            charges.prepay_owned(&budget).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Ingress
            }
        );
        assert_eq!(budget.in_use(), 0);
        assert_eq!(Arc::strong_count(&budget), 1);
    }

    #[test]
    fn owned_prepayment_releases_on_cancellation_and_unwind() {
        let charges = AuctionCharges {
            config: 1,
            ingress: 2,
            privacy: 4,
            projection: 8,
            outbound: 16,
            response: 32,
        };
        let budget = Arc::new(AuctionBudget::new(70));
        let borrowed = budget.try_reserve(ChargeKind::Privacy, 7).unwrap();
        let weak = Arc::downgrade(&budget);
        let paid = charges.prepay_owned(&budget).unwrap();
        assert_eq!(budget.in_use(), 70);
        let auction = async move {
            let _retained = paid;
            assert_eq!(weak.upgrade().unwrap().in_use(), 70);
            pending::<()>().await;
        };
        assert!(auction.now_or_never().is_none());
        assert_eq!(budget.in_use(), 7);
        assert_eq!(Arc::strong_count(&budget), 1);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _retained = charges.prepay_owned(&budget).unwrap();
            assert_eq!(budget.in_use(), 70);
            panic!("exercise owned reservation unwind");
        }));
        let _panic = result.unwrap_err();
        assert_eq!(budget.in_use(), 7);
        assert_eq!(Arc::strong_count(&budget), 1);
        drop(borrowed);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn owned_accounting_allocates_nothing_after_arc_construction() {
        let budget = Arc::new(AuctionBudget::new(10));
        let weak = Arc::downgrade(&budget);
        let charges = AuctionCharges {
            config: 1,
            ingress: 2,
            privacy: 1,
            projection: 2,
            outbound: 1,
            response: 3,
        };
        let before = allocated_on_this_thread();
        let borrowed = budget.try_reserve(ChargeKind::Privacy, 4).unwrap();
        let owned = budget.try_reserve_owned(ChargeKind::Projection, 6).unwrap();
        let empty = budget.try_reserve_owned(ChargeKind::Ingress, 0).unwrap();
        assert_eq!(budget.in_use(), 10);
        assert_eq!(owned.ensure_capacity(ChargeKind::Projection, 6), Ok(()));
        assert_eq!(
            budget
                .try_reserve_owned(ChargeKind::Response, 1)
                .unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Response
            }
        );
        drop((borrowed, owned, empty));
        assert_eq!(budget.in_use(), 0);
        let paid = charges.prepay_owned(&budget).unwrap();
        assert_eq!(budget.in_use(), 10);
        drop(paid);
        assert_eq!(budget.in_use(), 0);
        assert_eq!(
            AuctionCharges {
                response: 4,
                ..charges
            }
            .prepay_owned(&budget)
            .unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Response
            }
        );
        assert_eq!(budget.in_use(), 0);
        assert_eq!(Arc::strong_count(&budget), 1);
        drop(budget);
        assert!(weak.upgrade().is_none());
        let after = allocated_on_this_thread();
        assert_eq!(after, before);
    }

    #[test]
    fn phase_prepayment_rolls_back_at_every_boundary() {
        let charges = AuctionCharges {
            config: 1,
            ingress: 2,
            privacy: 4,
            projection: 8,
            outbound: 16,
            response: 32,
        };
        let phases = [
            ChargeKind::RuntimeConfig,
            ChargeKind::Ingress,
            ChargeKind::Privacy,
            ChargeKind::Projection,
            ChargeKind::Outbound,
            ChargeKind::Response,
        ];
        for (limit, phase) in [0, 1, 3, 7, 15, 31].into_iter().zip(phases) {
            let budget = AuctionBudget::new(limit);
            assert_eq!(
                charges.prepay(&budget).unwrap_err(),
                ResourceError::Exhausted { kind: phase }
            );
            assert_eq!(budget.in_use(), 0);
        }
        assert_eq!(charges.total().unwrap(), 63);
        let budget = AuctionBudget::new(63);
        let paid = charges.prepay(&budget).unwrap();
        assert_eq!(budget.in_use(), 63);
        drop(paid);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn phase_total_overflow_is_terminal() {
        let charges = AuctionCharges {
            config: u64::MAX,
            ingress: 1,
            privacy: 0,
            projection: 0,
            outbound: 0,
            response: 0,
        };
        assert_eq!(charges.total(), Err(ResourceError::CapacityOverflow));
    }

    #[test]
    fn a_projection_can_only_spend_its_own_held_reservation() {
        let budget = AuctionBudget::new(100);
        let receipt = budget.try_reserve(ChargeKind::Projection, 100).unwrap();
        assert_eq!(receipt.kind(), ChargeKind::Projection);
        assert_eq!(receipt.ensure_capacity(ChargeKind::Projection, 100), Ok(()));
        assert_eq!(
            receipt.ensure_capacity(ChargeKind::Projection, 101),
            Err(ResourceError::Exhausted {
                kind: ChargeKind::Projection
            })
        );
        assert_eq!(
            receipt.ensure_capacity(ChargeKind::RuntimeConfig, 1),
            Err(ResourceError::Exhausted {
                kind: ChargeKind::RuntimeConfig
            })
        );
        assert_eq!(budget.in_use(), 100);
    }

    #[test]
    fn exact_capacity_refuses_one_more_byte_and_returns_every_charge() {
        let budget = AuctionBudget::new(100);
        let config = budget.try_reserve(ChargeKind::RuntimeConfig, 60).unwrap();
        let privacy = budget.try_reserve(ChargeKind::Privacy, 40).unwrap();
        assert_eq!(budget.in_use(), 100);
        assert_eq!(budget.limit(), 100);
        assert_eq!(
            budget.try_reserve(ChargeKind::Projection, 1).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Projection
            }
        );
        assert_eq!(budget.in_use(), 100);
        drop(config);
        assert_eq!(budget.in_use(), 40);
        let outbound = budget.try_reserve(ChargeKind::Outbound, 60).unwrap();
        drop(privacy);
        drop(outbound);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn zero_charge_is_valid_even_when_full() {
        let budget = AuctionBudget::new(0);
        let empty = budget.try_reserve(ChargeKind::Ingress, 0).unwrap();
        assert_eq!(
            budget.try_reserve(ChargeKind::Ingress, 1).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Ingress
            }
        );
        drop(empty);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn sum_overflow_never_wraps_into_an_admission() {
        let budget = AuctionBudget::new(u64::MAX);
        let full = budget.try_reserve(ChargeKind::Ingress, u64::MAX).unwrap();
        assert_eq!(
            budget.try_reserve(ChargeKind::Privacy, 1).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Privacy
            }
        );
        assert_eq!(budget.in_use(), u64::MAX);
        drop(full);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn early_error_and_unwind_release_held_bytes() {
        fn fail(budget: &AuctionBudget) -> Result<(), ResourceError> {
            let _held = budget.try_reserve(ChargeKind::Privacy, 20)?;
            let _refused = budget.try_reserve(ChargeKind::Outbound, 1)?;
            Ok(())
        }
        let budget = AuctionBudget::new(20);
        assert_eq!(
            fail(&budget).unwrap_err(),
            ResourceError::Exhausted {
                kind: ChargeKind::Outbound
            }
        );
        assert_eq!(budget.in_use(), 0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _held = budget.try_reserve(ChargeKind::Privacy, 20).unwrap();
            panic!("exercise reservation unwind");
        }));
        let _panic = result.unwrap_err();
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn cancelling_a_suspended_auction_releases_its_charge() {
        let budget = AuctionBudget::new(100);
        let auction = async {
            let _retained = budget.try_reserve(ChargeKind::Projection, 100).unwrap();
            assert_eq!(budget.in_use(), 100);
            pending::<()>().await;
        };
        assert!(auction.now_or_never().is_none());
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn concurrent_reservations_cannot_overcommit_one_auction() {
        let budget = Arc::new(AuctionBudget::new(7));
        let barrier = Arc::new(Barrier::new(16));
        let threads: Vec<_> = iter::repeat_with(|| {
            let worker_budget = Arc::clone(&budget);
            let worker_barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                worker_barrier.wait();
                let reservation = worker_budget.try_reserve(ChargeKind::Response, 1).ok();
                worker_barrier.wait();
                let held = worker_budget.in_use();
                worker_barrier.wait();
                assert_eq!(held, 7);
                reservation.is_some()
            })
        })
        .take(16)
        .collect();
        let admitted = threads
            .into_iter()
            .map(|worker| usize::from(worker.join().unwrap()))
            .sum::<usize>();
        assert_eq!(admitted, 7);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn separate_auctions_do_not_share_a_physical_memory_gate() {
        let first = AuctionBudget::new(50);
        let second = AuctionBudget::new(50);
        let a = first.try_reserve(ChargeKind::Ingress, 50).unwrap();
        let b = second.try_reserve(ChargeKind::Ingress, 50).unwrap();
        assert_eq!(first.in_use(), 50);
        assert_eq!(second.in_use(), 50);
        drop(a);
        assert_eq!(second.in_use(), 50);
        drop(b);
    }

    #[test]
    fn single_allocation_capacity_checks_layout_width_before_conversion() {
        assert_eq!(checked_allocation_capacity(0).unwrap(), 0);
        assert_eq!(checked_allocation_capacity(4096).unwrap(), 4096);
        let max = u64::try_from(isize::MAX).unwrap();
        assert_eq!(
            checked_allocation_capacity(max).unwrap(),
            usize::try_from(max).unwrap()
        );
        assert_eq!(
            checked_allocation_capacity(max.checked_add(1).unwrap()),
            Err(ResourceError::CapacityOverflow)
        );
        assert_eq!(
            checked_allocation_capacity(u64::MAX),
            Err(ResourceError::CapacityOverflow)
        );
    }

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

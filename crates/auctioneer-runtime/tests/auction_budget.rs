#[cfg(test)]
mod tests {
    use auctioneer_runtime::{
        AuctionBudget, AuctionCharges, ChargeKind, ResourceError, checked_allocation_capacity,
    };
    use futures::{FutureExt as _, future::pending};
    use std::iter;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Arc, Barrier};
    use std::thread;

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
}

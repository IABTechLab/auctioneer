//! Runtime-neutral three-arm biased race.
//!
//! Deliberately not built on a runtime's `select!` macro so it compiles
//! for every target the adapters ship to, wasm included.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

/// Polls `arms[0]`, then `arms[1]`, then `arms[2]` on every wake and
/// returns the first `Ready`. The earlier index wins a simultaneous
/// wake — that bias is the whole point, and it is what lets a caller
/// decide ties by argument order.
///
/// The race OWNS its arms. Dropping it (which happens as soon as the
/// `await` completes) drops the two losers, cancelling their in-flight
/// work. Borrowed arms would leave the losers alive in the caller's
/// frame and cancel nothing. Three small box allocations per call are
/// negligible against a network round-trip.
pub struct BiasedRace3<'a, T> {
    arms: [Pin<Box<dyn Future<Output = T> + 'a>>; 3],
}

impl<'a, T> BiasedRace3<'a, T> {
    /// Take the three arms by value, in bias order.
    #[must_use]
    pub fn new<A, B, C>(a: A, b: B, c: C) -> Self
    where
        A: Future<Output = T> + 'a,
        B: Future<Output = T> + 'a,
        C: Future<Output = T> + 'a,
    {
        Self {
            arms: [Box::pin(a), Box::pin(b), Box::pin(c)],
        }
    }
}

impl<T> Future for BiasedRace3<'_, T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        for arm in &mut self.arms {
            if let Poll::Ready(value) = arm.as_mut().poll(cx) {
                // The other two arms drop with `self`.
                return Poll::Ready(value);
            }
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::BiasedRace3;
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};
    use futures::executor::block_on;
    use std::future::ready;
    use std::sync::{Arc, Mutex};

    /// Never resolves, and records its own destruction — so a test can
    /// prove the race cancelled it rather than leaking it.
    struct PendingWithDropFlag(Arc<Mutex<bool>>);

    impl Future for PendingWithDropFlag {
        type Output = u8;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u8> {
            Poll::Pending
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for PendingWithDropFlag {
        fn drop(&mut self) {
            *self.0.lock().expect("uncontended") = true;
        }
    }

    #[test]
    fn earlier_arm_wins_on_simultaneous_ready() {
        let winner = block_on(BiasedRace3::new(ready(1_u8), ready(2_u8), ready(3_u8)));
        assert_eq!(winner, 1_u8, "arm 0 wins when every arm is ready at once");
    }

    #[test]
    fn waits_for_first_ready() {
        let flag_a = Arc::new(Mutex::new(false));
        let flag_b = Arc::new(Mutex::new(false));
        let winner = block_on(BiasedRace3::new(
            PendingWithDropFlag(Arc::clone(&flag_a)),
            PendingWithDropFlag(Arc::clone(&flag_b)),
            ready(3_u8),
        ));
        assert_eq!(winner, 3_u8, "the only ready arm wins regardless of index");
    }

    #[test]
    fn pending_arms_are_dropped_on_completion() {
        let flag_b = Arc::new(Mutex::new(false));
        let flag_c = Arc::new(Mutex::new(false));
        let winner = block_on(BiasedRace3::new(
            ready(1_u8),
            PendingWithDropFlag(Arc::clone(&flag_b)),
            PendingWithDropFlag(Arc::clone(&flag_c)),
        ));
        assert_eq!(winner, 1_u8, "arm 0 completes first");
        assert!(
            *flag_b.lock().expect("uncontended"),
            "the losing arm 1 must be dropped, i.e. cancelled",
        );
        assert!(
            *flag_c.lock().expect("uncontended"),
            "the losing arm 2 must be dropped, i.e. cancelled",
        );
    }
}

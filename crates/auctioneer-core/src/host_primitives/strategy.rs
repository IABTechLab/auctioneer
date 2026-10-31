//! Per-call fanout strategies.
//!
//! A strategy is a boot-time singleton: it does NOT own the proxy
//! client, which is per-request, so `drive_one` receives the client as
//! an argument alongside the call, the auction deadline, and the two
//! host primitives it may need.

use crate::host_primitives::fanout::{ProxyClientApi, classify};
use crate::host_primitives::race::BiasedRace3;
use async_trait::async_trait;
use auctioneer_bidder::{BidderHttpResponse, ProxyError};
use auctioneer_exchange::{
    AuctionDeadline, BidderCall, BidderCallResult, FanoutError, TimeoutCause,
};
use auctioneer_runtime::{Clock, Timer};
use futures::FutureExt as _;
use std::sync::Arc;
use std::time::Duration;

/// The production fanout strategy for the Axum (tokio) adapter.
///
/// The entrypoint installs EXACTLY this, and a test asserts it is the
/// racing strategy — an inline `Arc::new(..)` in a bin-only crate is
/// unreachable from any test, so a strategy that silently reverted to
/// the delegating path would ship with every conformance row still
/// green.
#[must_use]
pub fn axum_production_strategy() -> Arc<dyn FanoutStrategy + Send + Sync> {
    Arc::new(RaceTimersStrategy)
}

/// How one bidder call is driven end to end.
#[async_trait(?Send)]
pub trait FanoutStrategy: Send + Sync {
    /// Drive a single bidder call to a result.
    ///
    /// Never fails at the outer level: a per-bidder timeout or transport
    /// failure is an `Err` INSIDE the returned [`BidderCallResult`].
    async fn drive_one<'a>(
        &'a self,
        client: &'a dyn ProxyClientApi,
        call: BidderCall,
        deadline: &'a AuctionDeadline,
        clock: &'a dyn Clock,
        timer: &'a dyn Timer,
    ) -> BidderCallResult;
}

/// Which arm of [`RaceTimersStrategy`]'s race finished first.
pub enum RaceOutcome {
    /// A timer arm elapsed, carrying which budget it was.
    Timeout(TimeoutCause),
    /// The send arm produced an outcome.
    Sent(Result<BidderHttpResponse, ProxyError>),
}

/// Single-call delegate: send, classify. Never awaits the
/// `Timer`, so an immediate-resolve stub is inert under it.
pub struct DelegateToDevStrategy;

#[async_trait(?Send)]
impl FanoutStrategy for DelegateToDevStrategy {
    async fn drive_one<'a>(
        &'a self,
        client: &'a dyn ProxyClientApi,
        call: BidderCall,
        _deadline: &'a AuctionDeadline,
        clock: &'a dyn Clock,
        _timer: &'a dyn Timer,
    ) -> BidderCallResult {
        let start = clock.now_monotonic();
        let outcome = client.send_one(call.request).await;
        let elapsed = clock.now_monotonic().saturating_duration_since(start);
        BidderCallResult {
            result: classify(outcome, call.max_decoded_bytes),
            elapsed,
        }
    }
}

/// Races the per-bidder timeout, the absolute auction deadline, and the
/// send itself. Requires a real host `Timer`; pairing it with an
/// immediate-resolve stub would time every call out at once.
pub struct RaceTimersStrategy;

#[async_trait(?Send)]
impl FanoutStrategy for RaceTimersStrategy {
    async fn drive_one<'a>(
        &'a self,
        client: &'a dyn ProxyClientApi,
        call: BidderCall,
        deadline: &'a AuctionDeadline,
        clock: &'a dyn Clock,
        timer: &'a dyn Timer,
    ) -> BidderCallResult {
        let start = clock.now_monotonic();
        // Pre-expired guard, before ANY work. If the absolute auction
        // deadline is already at or behind now, short-circuit: do not
        // compress and do not dispatch. Compressing first and measuring
        // the deadline afterwards would make an already-expired call pay
        // the encoding cost and still touch the proxy.
        if deadline.deadline_at <= start {
            return BidderCallResult {
                result: Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
                elapsed: Duration::ZERO,
            };
        }
        // Recompute the remaining budget against NOW, not against
        // `start`. The auction deadline is absolute, so measuring it
        // before encoding and only then starting the arm would add the
        // encoding time back on and push the effective tmax past
        // `deadline.deadline_at`.
        let deadline_remaining = deadline
            .deadline_at
            .saturating_duration_since(clock.now_monotonic());

        // Bias order is bidder, deadline, send — so an exactly equal
        // per-bidder timeout and auction deadline attribute to the
        // bidder.
        let bidder_arm = timer
            .sleep(call.timeout)
            .map(|()| RaceOutcome::Timeout(TimeoutCause::Bidder));
        let deadline_arm = timer
            .sleep(deadline_remaining)
            .map(|()| RaceOutcome::Timeout(TimeoutCause::AuctionDeadline));
        let send_arm = client.send_one(call.request).map(RaceOutcome::Sent);

        let outcome = BiasedRace3::new(bidder_arm, deadline_arm, send_arm).await;
        let elapsed = clock.now_monotonic().saturating_duration_since(start);
        match outcome {
            RaceOutcome::Timeout(cause) => BidderCallResult {
                result: Err(FanoutError::Timeout(cause)),
                elapsed,
            },
            RaceOutcome::Sent(sent) => BidderCallResult {
                result: classify(sent, call.max_decoded_bytes),
                elapsed,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::outbound_request;

    use super::{DelegateToDevStrategy, FanoutStrategy as _, RaceTimersStrategy};
    use crate::host_primitives::fanout::ProxyClientApi;
    use async_trait::async_trait;
    use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse, ProxyError};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, FanoutError, TimeoutCause, TmaxSource,
    };
    use auctioneer_runtime::{Clock, FakeClock, ImmediateResolveTimer, MonotonicInstant, Timer};
    use futures::executor::block_on;
    use std::future::{Future, pending};
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;
    use time::OffsetDateTime;

    const HOSTILE_DETAIL: &str = "https://user:pw@bidder.example/rtb?key=abc token=xyz refused";

    const SECRET_NEEDLES: [&str; 5] = ["bidder.example", "user", "pw", "key=abc", "token=xyz"];

    struct FailingClient;

    #[async_trait(?Send)]
    impl ProxyClientApi for FailingClient {
        async fn send_one(
            &self,
            _req: BidderHttpRequest,
        ) -> Result<BidderHttpResponse, ProxyError> {
            Err(ProxyError::Transport(HOSTILE_DETAIL.to_owned()))
        }
    }

    fn call_with_timeout(timeout: Duration) -> BidderCall {
        BidderCall {
            request: outbound_request::request(
                "POST".into(),
                "https://x".into(),
                vec![],
                vec![],
                vec![],
            )
            .expect("sink-built fixture"),
            timeout,
            max_transport_bytes: 1024,
            max_decoded_bytes: 1024,
        }
    }

    fn call() -> BidderCall {
        call_with_timeout(Duration::from_millis(100))
    }

    fn fake_clock() -> FakeClock {
        FakeClock::new(OffsetDateTime::UNIX_EPOCH)
    }

    /// An absolute deadline `millis` past the clock's axis origin —
    /// the same anchor `advance(millis)` moves the clock to, so a test
    /// reads as before even though the axis is now a real instant.
    fn deadline_at(clock: &FakeClock, millis: u64) -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: clock
                .origin()
                .checked_add(Duration::from_millis(millis))
                .expect("no overflow"),
            source: TmaxSource::Request,
        }
    }

    /// The strategy path — not just the classifier in isolation — must
    /// keep the untrusted transport string out of the log.
    #[test]
    fn fanout_warn_line_logs_category_only_never_transport_detail() {
        testing_logger::setup();
        let result = block_on(async {
            let clock = fake_clock();
            let deadline = deadline_at(&clock, 1000);
            DelegateToDevStrategy
                .drive_one(
                    &FailingClient,
                    call(),
                    &deadline,
                    &clock,
                    &ImmediateResolveTimer,
                )
                .await
        });
        match &result.result {
            Err(FanoutError::Other(payload)) => assert_eq!(
                payload, "transport error",
                "the delegate carries the fixed category, not the detail",
            ),
            other => panic!("expected Other(\"transport error\"), got {other:?}"),
        }
        testing_logger::validate(|captured| {
            let warns: Vec<&String> = captured
                .iter()
                .filter(|line| line.level == log::Level::Warn)
                .map(|line| &line.body)
                .collect();
            assert_eq!(warns.len(), 1, "one warning per driven call");
            assert_eq!(
                warns[0], "bidder transport error",
                "the warning is the fixed category and nothing else",
            );
            for line in captured {
                for needle in SECRET_NEEDLES {
                    assert!(
                        !line.body.contains(needle),
                        "log line leaked {needle}: {}",
                        line.body,
                    );
                }
            }
        });
    }

    struct OkClient;

    #[async_trait(?Send)]
    impl ProxyClientApi for OkClient {
        async fn send_one(
            &self,
            _req: BidderHttpRequest,
        ) -> Result<BidderHttpResponse, ProxyError> {
            Ok(BidderHttpResponse::new(200, vec![], b"{}".to_vec()))
        }
    }

    /// The delegate reads the injected clock rather than any ambient
    /// time source, and never awaits the timer — which is what makes an
    /// immediate-resolve stub inert under it.
    #[test]
    fn delegate_measures_elapsed_from_the_injected_clock() {
        let clock = fake_clock();
        let result = block_on(async {
            let deadline = deadline_at(&clock, 1000);
            DelegateToDevStrategy
                .drive_one(&OkClient, call(), &deadline, &clock, &ImmediateResolveTimer)
                .await
        });
        assert!(result.result.is_ok(), "the happy path returns the response");
        assert_eq!(
            result.elapsed,
            Duration::ZERO,
            "a clock that never advances yields a zero elapsed",
        );
        assert_eq!(
            clock.now_monotonic(),
            clock.origin(),
            "the delegate must not advance the caller's clock",
        );
    }

    /// Panics on every use. Handed to the race as BOTH the client and
    /// the timer, it proves the pre-expired guard returned before an arm
    /// was ever built — building one calls `sleep`, and polling the race
    /// calls `send_one`.
    struct PanicOnUse;

    #[async_trait(?Send)]
    impl ProxyClientApi for PanicOnUse {
        async fn send_one(
            &self,
            _req: BidderHttpRequest,
        ) -> Result<BidderHttpResponse, ProxyError> {
            panic!("an expired deadline must not dispatch");
        }
    }

    #[async_trait(?Send)]
    impl Timer for PanicOnUse {
        async fn sleep(&self, _duration: Duration) {
            panic!("an expired deadline must not arm a timer");
        }
    }

    /// A send that never resolves, so only a timer arm can win.
    struct NeverResponds;

    #[async_trait(?Send)]
    impl ProxyClientApi for NeverResponds {
        async fn send_one(
            &self,
            _req: BidderHttpRequest,
        ) -> Result<BidderHttpResponse, ProxyError> {
            pending::<Result<BidderHttpResponse, ProxyError>>().await
        }
    }

    /// Reads `now` off a `FakeClock`, but charges `step` to the SECOND
    /// read — the one the strategy makes after setup. That models
    /// time passing during setup, which is the only way to observe
    /// which `now` the deadline arm was measured against.
    struct SetupCostClock<'a> {
        fake: &'a FakeClock,
        step: Duration,
        reads: AtomicU32,
    }

    impl Clock for SetupCostClock<'_> {
        fn now(&self) -> OffsetDateTime {
            self.fake.now()
        }

        fn now_monotonic(&self) -> MonotonicInstant {
            if self.reads.fetch_add(1, Ordering::Relaxed) == 1 {
                self.fake.advance(self.step);
            }
            self.fake.now_monotonic()
        }
    }

    /// The race can only be resolved by advancing the `FakeClock`
    /// between polls, which no executor would do for us — so the tests
    /// below drive the future by hand instead of blocking on it.
    fn poll_once<F>(driving: &mut Pin<Box<F>>) -> Poll<BidderCallResult>
    where
        F: Future<Output = BidderCallResult> + ?Sized,
    {
        driving
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
    }

    fn drive_pre_expired(now_millis: u64, deadline_millis: u64) -> BidderCallResult {
        let clock = fake_clock();
        clock.advance(Duration::from_millis(now_millis));
        let deadline = deadline_at(&clock, deadline_millis);
        block_on(RaceTimersStrategy.drive_one(&PanicOnUse, call(), &deadline, &clock, &PanicOnUse))
    }

    /// A deadline that has already arrived short-circuits before any
    /// work: no timer arms or dispatch.
    #[test]
    fn pre_expired_deadline_returns_without_arming_or_dispatching() {
        let at_deadline = drive_pre_expired(50, 50);
        assert!(
            matches!(
                at_deadline.result,
                Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
            ),
            "a deadline exactly at now is expired, and attributes to the auction",
        );
        assert_eq!(
            at_deadline.elapsed,
            Duration::ZERO,
            "the guard costs nothing measurable",
        );

        let past_deadline = drive_pre_expired(50, 10);
        assert!(
            matches!(
                past_deadline.result,
                Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
            ),
            "a deadline behind now is expired too",
        );
        assert_eq!(past_deadline.elapsed, Duration::ZERO, "same for the guard");
    }

    /// The per-bidder budget elapsing while the send hangs maps to
    /// `Timeout(Bidder)`.
    #[test]
    fn elapsed_bidder_timeout_wins_over_a_hanging_send() {
        let clock = fake_clock();
        let call = call_with_timeout(Duration::from_millis(100));
        let deadline = deadline_at(&clock, 500);
        let client = NeverResponds;
        let strategy = RaceTimersStrategy;
        let mut driving = strategy.drive_one(&client, call, &deadline, &clock, &clock);
        assert!(
            poll_once(&mut driving).is_pending(),
            "nothing has elapsed at t=0 and the send never resolves",
        );

        clock.advance(Duration::from_millis(100));
        let Poll::Ready(result) = poll_once(&mut driving) else {
            panic!("the elapsed per-bidder timer must resolve the race")
        };
        assert!(
            matches!(
                result.result,
                Err(FanoutError::Timeout(TimeoutCause::Bidder)),
            ),
            "the bidder budget ran out first, so the cause is the bidder",
        );
        assert_eq!(
            result.elapsed,
            Duration::from_millis(100),
            "elapsed is measured on the injected clock",
        );
    }

    /// Bias order is bidder, deadline, send — so when both timers come
    /// due on the same wake, the bidder arm is the one that is read.
    #[test]
    fn equal_bidder_timeout_and_deadline_ties_to_the_bidder_cause() {
        let clock = fake_clock();
        let call = call_with_timeout(Duration::from_millis(100));
        let deadline = deadline_at(&clock, 100);
        let client = NeverResponds;
        let strategy = RaceTimersStrategy;
        let mut driving = strategy.drive_one(&client, call, &deadline, &clock, &clock);
        assert!(
            poll_once(&mut driving).is_pending(),
            "both timers are still running at t=0",
        );

        clock.advance(Duration::from_millis(100));
        let Poll::Ready(result) = poll_once(&mut driving) else {
            panic!("both timers are due, so the race must resolve")
        };
        assert!(
            matches!(
                result.result,
                Err(FanoutError::Timeout(TimeoutCause::Bidder)),
            ),
            "a tie attributes to the bidder because its arm is polled first",
        );
    }

    /// The auction deadline is ABSOLUTE: setup has to come out of
    /// it, not extend it. Measuring the remaining budget before the
    /// setup would arm the deadline at t=140 here, and this race would
    /// still be pending at t=100.
    #[test]
    fn deadline_arm_is_measured_after_setup_not_before_it() {
        let fake = fake_clock();
        let clock = SetupCostClock {
            fake: &fake,
            step: Duration::from_millis(40),
            reads: AtomicU32::new(0),
        };
        let call = call_with_timeout(Duration::from_millis(80));
        let deadline = deadline_at(&fake, 100);
        let client = NeverResponds;
        let strategy = RaceTimersStrategy;
        let mut driving = strategy.drive_one(&client, call, &deadline, &clock, &fake);
        assert!(
            poll_once(&mut driving).is_pending(),
            "setup consumed 40ms; both arms are still running",
        );

        // Arms were built at t=40: the deadline arm holds the remaining
        // 60ms (due t=100), the bidder arm its full 80ms (due t=120).
        fake.advance(Duration::from_millis(60));
        let Poll::Ready(result) = poll_once(&mut driving) else {
            panic!("the recomputed deadline must resolve the race at t=100")
        };
        assert!(
            matches!(
                result.result,
                Err(FanoutError::Timeout(TimeoutCause::AuctionDeadline)),
            ),
            "the absolute deadline expired before the per-bidder budget",
        );
    }
}

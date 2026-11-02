//! Existence-and-callability smoke tests for the upstream lifecycle
//! primitives the `EdgeZero` pin introduces.
//!
//! These assert ONLY that each API is present and can be called with
//! the signature auctioneer's consumers will use. They deliberately assert
//! nothing about auctioneer behaviour through those primitives: the permit
//! release, the static-egress pool and the error renderer are owned by
//! the units that install them, and their tests live there. A smoke
//! test that grew a behavioural assertion would be testing a surface
//! this crate does not yet wire.

#[cfg(test)]
mod tests {
    use edgezero_core::ingress::AdmissionDecision;
    use edgezero_core::response_egress::{
        DetachedResponseEgressDecision, ResponseEgressCompletion, ResponseEgressDeadline,
    };
    use edgezero_core::time::Deadline;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// `late_bound` hands back an installer and its paired completion,
    /// and the installer accepts a resource of the requested type.
    #[test]
    fn late_bound_pairs_an_installer_with_a_completion() {
        let (resource, completion) = ResponseEgressCompletion::late_bound::<Vec<u8>>();
        assert!(
            resource.install(vec![1_u8, 2, 3]).is_ok(),
            "a freshly created installer accepts one resource",
        );
        // Dropping the completion without a terminal report is the
        // abandonment path; it must not panic.
        drop(completion);
    }

    /// `join` composes two completions into one value of the same type.
    #[test]
    fn join_composes_two_completions() {
        let ran = Arc::new(AtomicUsize::new(0));
        let left = Arc::clone(&ran);
        let right = Arc::clone(&ran);
        let joined = ResponseEgressCompletion::new(move |_report| {
            left.fetch_add(1, Ordering::Relaxed);
        })
        .join(ResponseEgressCompletion::new(move |_report| {
            right.fetch_add(1, Ordering::Relaxed);
        }));
        // Abandoned rather than delivered: this crate has no terminal
        // report to hand it, and producing one is the consuming unit's
        // job, not this smoke test's.
        drop(joined);
        assert_eq!(
            ran.load(Ordering::Relaxed),
            0,
            "an abandoned completion runs neither callback",
        );
    }

    /// `ResponseEgressDeadline::after` is const-callable with a plain
    /// `Duration`, which is the form auctioneer's response path will use.
    #[test]
    fn response_egress_deadline_after_takes_a_duration() {
        const DEADLINE: ResponseEgressDeadline =
            ResponseEgressDeadline::after(Duration::from_millis(250));
        // The kind is private upstream, so the observable fact is that
        // the value constructs and is `Copy` — used twice from one
        // binding without a move.
        let copied = DEADLINE;
        assert_eq!(
            format!("{copied:?}"),
            format!("{DEADLINE:?}"),
            "a copied deadline is the same value",
        );
    }

    /// `AdmissionDecision::Abort` exists as a no-response variant.
    #[test]
    fn admission_decision_abort_is_constructible() {
        let decision = AdmissionDecision::Abort;
        assert!(
            matches!(decision, AdmissionDecision::Abort),
            "Abort is the no-response admission disposition",
        );
    }

    /// `DetachedResponseEgressDecision` offers both the abort and the
    /// send-with-completion dispositions auctioneer will return from its
    /// pre-admission factory.
    #[test]
    fn detached_decision_carries_abort_and_send() {
        let abort = DetachedResponseEgressDecision::Abort;
        assert!(matches!(abort, DetachedResponseEgressDecision::Abort));

        let send = DetachedResponseEgressDecision::Send {
            completion: ResponseEgressCompletion::empty(),
            deadline: Deadline::after(Duration::from_millis(50)),
        };
        assert!(
            matches!(send, DetachedResponseEgressDecision::Send { .. }),
            "Send carries a completion and a write deadline",
        );
    }
}

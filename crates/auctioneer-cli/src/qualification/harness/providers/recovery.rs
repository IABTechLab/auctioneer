use crate::qualification::harness::providers::mutation_gate;
use crate::qualification::harness::{HarnessError, Result};
use futures::future::{Either, join, select};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, future::Future, pin::Pin, time::Duration};
use tokio::sync::watch;
use tokio::time::{sleep, timeout};

#[derive(Clone)]
struct Cancellation(watch::Sender<bool>);

impl Cancellation {
    fn new() -> Self {
        Self(watch::channel(false).0)
    }

    fn cancel(&self) {
        self.0.send_replace(true);
    }

    fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }

    async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        while !*receiver.borrow_and_update() {
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Boolean(bool),
    Failed,
    Invalid,
}

type ControlFuture = Pin<Box<dyn Future<Output = Outcome>>>;
type Run = Box<dyn FnOnce(Cancellation) -> Option<ControlFuture>>;
type Terminate = Box<dyn FnOnce() -> ControlFuture>;

// These are injected offline controls, not provider writes or a command runner.
struct Operation {
    run: Option<Run>,
    terminate: Option<Terminate>,
}

type Operations = BTreeMap<&'static str, Operation>;

struct Options {
    signal: Option<Cancellation>,
    timeout_ms: u64,
    termination_timeout_ms: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            signal: None,
            timeout_ms: 1000,
            termination_timeout_ms: 1000,
        }
    }
}

#[derive(Debug, Serialize)]
struct RecoveryReport {
    kind: &'static str,
    restored: bool,
    deployed_handler_acceptance: &'static str,
}

enum StepResult {
    Completed(Outcome),
    Stopped(&'static str),
}

async fn bounded_step(
    operation: Operation,
    timeout_ms: u64,
    termination_timeout_ms: u64,
    signal: Option<&Cancellation>,
) -> Result<bool> {
    if signal.is_some_and(Cancellation::is_cancelled) {
        return Err(HarnessError::new("interrupted"));
    }
    let run = operation.run.ok_or(HarnessError::new("invalid_control"))?;
    let terminate = operation
        .terminate
        .ok_or(HarnessError::new("invalid_control"))?;
    let cancellation = Cancellation::new();
    let completion = async {
        match run(cancellation.clone()) {
            Some(fence) => fence.await,
            None => Outcome::Invalid,
        }
    };
    tokio::pin!(completion);
    let external = signal.cloned().unwrap_or_else(Cancellation::new);
    let raced = match select(
        Box::pin(select(
            Box::pin(external.cancelled()),
            Box::pin(sleep(Duration::from_millis(timeout_ms))),
        )),
        &mut completion,
    )
    .await
    {
        Either::Left((Either::Left(((), _timeout)), _pending)) => {
            StepResult::Stopped("interrupted")
        }
        Either::Left((Either::Right(((), _signal)), _pending)) => {
            StepResult::Stopped("control_timeout")
        }
        Either::Right((outcome, _pending)) => StepResult::Completed(outcome),
    };
    let (reason, settled) = match raced {
        StepResult::Completed(outcome) if external.is_cancelled() => ("interrupted", Some(outcome)),
        StepResult::Completed(Outcome::Boolean(value)) => return Ok(value),
        StepResult::Completed(Outcome::Failed) => return Err(HarnessError::new("control_failed")),
        StepResult::Completed(Outcome::Invalid) => {
            ("operation_not_quiescent", Some(Outcome::Invalid))
        }
        StepResult::Stopped(reason) => (reason, None),
    };

    cancellation.cancel();
    // Neither an abort nor a termination acknowledgement fences side effects by itself.
    let quiescent = timeout(Duration::from_millis(termination_timeout_ms), async {
        let termination = async { terminate().await == Outcome::Boolean(true) };
        let fence = async {
            match settled {
                Some(outcome) => outcome,
                None => completion.await,
            }
        };
        let (terminated, completed) = join(termination, fence).await;
        terminated && completed != Outcome::Invalid
    })
    .await
    .unwrap_or(false);
    if !quiescent {
        return Err(HarnessError::new("operation_not_quiescent"));
    }
    Err(HarnessError::new(reason))
}

async fn exercise_recovery_control(
    proof: Value,
    mut operations: Operations,
    options: Options,
) -> Result<RecoveryReport> {
    mutation_gate(proof)?;
    if !(1..=5000).contains(&options.timeout_ms)
        || !(1..=5000).contains(&options.termination_timeout_ms)
    {
        return Err(HarnessError::new("invalid_input"));
    }
    if operations.keys().copied().collect::<Vec<_>>() != ["inject", "observe", "recover", "restore"]
        || operations
            .values()
            .any(|operation| operation.run.is_none() || operation.terminate.is_none())
    {
        return Err(HarnessError::new("invalid_control"));
    }
    let mut failure = None;
    for stage in ["inject", "observe"] {
        let operation = operations
            .remove(stage)
            .ok_or(HarnessError::new("invalid_control"))?;
        match bounded_step(
            operation,
            options.timeout_ms,
            options.termination_timeout_ms,
            options.signal.as_ref(),
        )
        .await
        {
            Ok(true) => {}
            result => {
                failure = Some(match result {
                    Err(HarnessError {
                        code: "interrupted",
                        ..
                    }) => "interrupted",
                    Err(HarnessError {
                        code: "operation_not_quiescent",
                        ..
                    }) => "operation_not_quiescent",
                    _ => "injection_unobserved",
                });
                break;
            }
        }
    }
    if failure != Some("operation_not_quiescent") {
        for stage in ["restore", "recover"] {
            let operation = operations
                .remove(stage)
                .ok_or(HarnessError::new("invalid_control"))?;
            // Each cleanup step gets a fresh deadline, independent of run interruption.
            match bounded_step(
                operation,
                options.timeout_ms,
                options.termination_timeout_ms,
                None,
            )
            .await
            {
                Ok(true) => {}
                result => {
                    failure = Some(match result {
                        Err(HarnessError {
                            code: "operation_not_quiescent",
                            ..
                        }) => "operation_not_quiescent",
                        _ => "restoration_failed",
                    });
                    break;
                }
            }
        }
    }
    if let Some(code) = failure {
        return Err(HarnessError::new(code));
    }
    Ok(RecoveryReport {
        kind: "offline_handler_control",
        restored: true,
        deployed_handler_acceptance: "pending",
    })
}

#[derive(Debug)]
struct CorruptionFixtures {
    malformed: String,
    digest_mismatch: String,
}

fn corruption_fixtures(limit: usize) -> Result<CorruptionFixtures> {
    #[derive(Serialize)]
    struct Envelope {
        version: u8,
        generated_at: &'static str,
        sha256: String,
        data: BTreeMap<&'static str, Value>,
    }
    if !(1..=1024).contains(&limit) {
        return Err(HarnessError::new("invalid_input"));
    }
    let fixtures = CorruptionFixtures {
        malformed: "{\"data\":".to_owned(),
        digest_mismatch: serde_json::to_string(&Envelope {
            version: 1,
            generated_at: "2026-10-07T00:00:00Z",
            sha256: "0".repeat(64),
            data: BTreeMap::new(),
        })
        .map_err(|_error| HarnessError::new("invalid_input"))?,
    };
    if fixtures.malformed.len() >= limit || fixtures.digest_mismatch.len() >= limit {
        return Err(HarnessError::new("fixture_limit"));
    }
    Ok(fixtures)
}

#[cfg(test)]
mod tests {
    use crate::qualification::harness::providers::recovery::{
        Cancellation, Operation, Operations, Options, Outcome, corruption_fixtures,
        exercise_recovery_control,
    };
    use crate::qualification::harness::providers::tests::proof;
    use serde_json::Value;
    use serde_json::json;
    use std::future::{Future, pending};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::sync::oneshot;
    use tokio::time::Instant;

    type Calls = Arc<Mutex<Vec<&'static str>>>;

    fn new_calls() -> Calls {
        Arc::new(Mutex::new(Vec::new()))
    }
    fn record(calls: &Calls, call: &'static str) {
        calls.lock().unwrap().push(call);
    }
    fn recorded(calls: &Calls) -> Vec<&'static str> {
        calls.lock().unwrap().clone()
    }

    fn operation<R, RF, T, TF>(run: R, terminate: T) -> Operation
    where
        R: FnOnce(Cancellation) -> RF + 'static,
        RF: Future<Output = Outcome> + 'static,
        T: FnOnce() -> TF + 'static,
        TF: Future<Output = Outcome> + 'static,
    {
        Operation {
            run: Some(Box::new(move |signal| Some(Box::pin(run(signal))))),
            terminate: Some(Box::new(move || Box::pin(terminate()))),
        }
    }

    fn result(outcome: Outcome) -> Operation {
        operation(
            move |_| async move { outcome },
            || async { Outcome::Boolean(true) },
        )
    }

    fn yes() -> Operation {
        result(Outcome::Boolean(true))
    }

    fn controls(inject: Operation) -> Operations {
        [
            ("inject", inject),
            ("observe", yes()),
            ("restore", yes()),
            ("recover", yes()),
        ]
        .into()
    }

    fn logged(calls: &Calls, name: &'static str, value: bool) -> Operation {
        let log = Arc::clone(calls);
        operation(
            move |_| async move {
                record(&log, name);
                Outcome::Boolean(value)
            },
            || async { Outcome::Boolean(true) },
        )
    }

    fn quick() -> Options {
        Options {
            timeout_ms: 5,
            termination_timeout_ms: 100,
            signal: None,
        }
    }

    async fn refuses(proof: Value, ops: Operations, options: Options, code: &str) {
        let error = exercise_recovery_control(proof, ops, options)
            .await
            .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(serde_json::to_value(error).unwrap(), json!({"code":code}));
    }

    #[tokio::test]
    async fn successful_control_is_ordered_and_grants_no_hosted_acceptance() {
        let calls = new_calls();
        let ops = ["inject", "observe", "restore", "recover"]
            .into_iter()
            .map(|name| (name, logged(&calls, name, true)))
            .collect();
        let report = exercise_recovery_control(proof(), ops, Options::default())
            .await
            .unwrap();
        assert_eq!(
            recorded(&calls),
            ["inject", "observe", "restore", "recover"]
        );
        assert_eq!(
            serde_json::to_value(report).unwrap(),
            json!({
            "kind":"offline_handler_control","restored":true,"deployed_handler_acceptance":"pending"})
        );
    }

    #[tokio::test]
    async fn failed_or_unobserved_injection_still_restores_with_safe_failure() {
        let mut checked = 0_usize;
        for stage in ["inject", "observe"] {
            for outcome in [Outcome::Boolean(false), Outcome::Failed] {
                let calls = new_calls();
                let mut ops = controls(yes());
                ops.insert(stage, result(outcome));
                ops.insert("restore", logged(&calls, "restore", true));
                ops.insert("recover", logged(&calls, "recover", true));
                refuses(proof(), ops, quick(), "injection_unobserved").await;
                assert_eq!(recorded(&calls), ["restore", "recover"]);
                checked = checked.checked_add(1_usize).unwrap();
            }
        }
        assert_eq!(checked, 4);
        let calls = new_calls();
        let mut ops = controls(result(Outcome::Failed));
        ops.insert("observe", logged(&calls, "observe", true));
        ops.insert("restore", logged(&calls, "restore", true));
        refuses(proof(), ops, quick(), "injection_unobserved").await;
        assert_eq!(recorded(&calls), ["restore"]);
    }

    #[tokio::test]
    async fn failed_restore_or_recover_cannot_be_masked_by_an_injection_failure() {
        let mut checked = 0_usize;
        for injection in [Outcome::Boolean(true), Outcome::Failed] {
            for stage in ["restore", "recover"] {
                for outcome in [Outcome::Boolean(false), Outcome::Failed] {
                    let calls = new_calls();
                    let mut ops = controls(result(injection));
                    ops.insert(stage, result(outcome));
                    if stage == "restore" {
                        ops.insert("recover", logged(&calls, "recover", true));
                    }
                    refuses(proof(), ops, quick(), "restoration_failed").await;
                    assert!(recorded(&calls).is_empty());
                    checked = checked.checked_add(1_usize).unwrap();
                }
            }
        }
        assert_eq!(checked, 8);
    }

    #[tokio::test]
    async fn interrupted_injection_restores_on_a_fresh_non_aborted_deadline() {
        let calls = new_calls();
        let signal = Cancellation::new();
        let external = signal.clone();
        let inject = operation(
            move |_| async move {
                external.cancel();
                Outcome::Boolean(true)
            },
            || async { Outcome::Boolean(true) },
        );
        let mut ops = controls(inject);
        ops.insert("restore", logged(&calls, "restore", true));
        ops.insert("recover", logged(&calls, "recover", true));
        refuses(
            proof(),
            ops,
            Options {
                signal: Some(signal),
                ..quick()
            },
            "interrupted",
        )
        .await;
        assert_eq!(recorded(&calls), ["restore", "recover"]);
    }

    #[tokio::test]
    async fn already_cancelled_run_does_not_inject_but_still_recovers() {
        let calls = new_calls();
        let signal = Cancellation::new();
        signal.cancel();
        let mut ops = controls(logged(&calls, "inject", true));
        ops.insert("restore", logged(&calls, "restore", true));
        ops.insert("recover", logged(&calls, "recover", true));
        refuses(
            proof(),
            ops,
            Options {
                signal: Some(signal),
                ..quick()
            },
            "interrupted",
        )
        .await;
        assert_eq!(recorded(&calls), ["restore", "recover"]);
    }

    #[tokio::test]
    async fn cooperative_cancellation_drains_before_restore() {
        let calls = new_calls();
        let running = Arc::clone(&calls);
        let stopping = Arc::clone(&calls);
        let (done_tx, done_rx) = oneshot::channel();
        let inject = operation(
            move |signal| async move {
                signal.cancelled().await;
                record(&running, "aborted");
                let _done_tx_result = done_tx.send(());
                Outcome::Boolean(false)
            },
            move || async move {
                let _done_rx_result = done_rx.await;
                record(&stopping, "quiescent");
                Outcome::Boolean(true)
            },
        );
        let mut ops = controls(inject);
        ops.insert("restore", logged(&calls, "restore", true));
        ops.insert("recover", logged(&calls, "recover", true));
        refuses(proof(), ops, quick(), "injection_unobserved").await;
        assert_eq!(
            recorded(&calls),
            ["aborted", "quiescent", "restore", "recover"]
        );
    }

    #[tokio::test]
    async fn ignored_abort_and_late_write_are_fenced_before_restoration() {
        let calls = new_calls();
        let state = Arc::new(Mutex::new("good"));
        let running = Arc::clone(&calls);
        let stopping = Arc::clone(&calls);
        let write_state = Arc::clone(&state);
        let (release_tx, release_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let writer = tokio::spawn(async move {
            let _release_rx_result = release_rx.await;
            record(&running, "late-inject");
            *write_state.lock().unwrap() = "corrupt";
            let _done_tx_result = done_tx.send(());
        });
        let start = Arc::clone(&calls);
        let inject = operation(
            move |_ignored_signal| async move {
                record(&start, "inject");
                let _done_rx_result = done_rx.await;
                Outcome::Boolean(true)
            },
            move || async move {
                record(&stopping, "terminate");
                let _release_tx_result = release_tx.send(());
                Outcome::Boolean(true)
            },
        );
        let restore_calls = Arc::clone(&calls);
        let restore_state = Arc::clone(&state);
        let restore = operation(
            move |signal| async move {
                assert!(!signal.is_cancelled());
                record(&restore_calls, "restore");
                *restore_state.lock().unwrap() = "good";
                Outcome::Boolean(true)
            },
            || async { Outcome::Boolean(true) },
        );
        let recover_calls = Arc::clone(&calls);
        let recover_state = Arc::clone(&state);
        let recover = operation(
            move |_| async move {
                record(&recover_calls, "recover");
                Outcome::Boolean(*recover_state.lock().unwrap() == "good")
            },
            || async { Outcome::Boolean(true) },
        );
        let mut ops = controls(inject);
        ops.insert("restore", restore);
        ops.insert("recover", recover);
        refuses(proof(), ops, quick(), "injection_unobserved").await;
        writer.await.unwrap();
        assert_eq!(
            recorded(&calls),
            ["inject", "terminate", "late-inject", "restore", "recover"]
        );
        assert_eq!(*state.lock().unwrap(), "good");
    }

    #[tokio::test]
    async fn termination_ack_without_completion_prevents_restoration_even_when_a_write_arrives_later()
     {
        let calls = new_calls();
        let start = Arc::clone(&calls);
        let stopping = Arc::clone(&calls);
        let late = Arc::clone(&calls);
        let (release_tx, release_rx) = oneshot::channel();
        let (done_tx, done_rx) = oneshot::channel();
        let writer = tokio::spawn(async move {
            let _release_rx_result = release_rx.await;
            record(&late, "late-inject");
            let _done_tx_result = done_tx.send(());
        });
        let inject = operation(
            move |_| async move {
                record(&start, "inject");
                let _done_rx_result = done_rx.await;
                Outcome::Boolean(true)
            },
            move || async move {
                record(&stopping, "terminate");
                Outcome::Boolean(true)
            },
        );
        let mut ops = controls(inject);
        ops.insert("restore", logged(&calls, "restore", true));
        ops.insert("recover", logged(&calls, "recover", true));
        refuses(
            proof(),
            ops,
            Options {
                termination_timeout_ms: 5,
                ..quick()
            },
            "operation_not_quiescent",
        )
        .await;
        assert_eq!(recorded(&calls), ["inject", "terminate"]);
        release_tx.send(()).unwrap();
        writer.await.unwrap();
        assert_eq!(recorded(&calls), ["inject", "terminate", "late-inject"]);
    }

    #[tokio::test]
    async fn completed_injection_without_successful_termination_is_not_quiescent() {
        for termination in [Outcome::Boolean(false), Outcome::Failed, Outcome::Invalid] {
            let calls = new_calls();
            let inject = operation(
                |signal| async move {
                    signal.cancelled().await;
                    Outcome::Boolean(false)
                },
                move || async move { termination },
            );
            let mut ops = controls(inject);
            ops.insert("restore", logged(&calls, "restore", true));
            refuses(proof(), ops, quick(), "operation_not_quiescent").await;
            assert!(recorded(&calls).is_empty());
        }
    }

    #[tokio::test]
    async fn stalled_termination_and_stalled_restore_remain_bounded() {
        let start = Instant::now();
        let inject = operation(
            |signal| async move {
                signal.cancelled().await;
                Outcome::Boolean(false)
            },
            || async { pending::<Outcome>().await },
        );
        refuses(
            proof(),
            controls(inject),
            Options {
                termination_timeout_ms: 5,
                ..quick()
            },
            "operation_not_quiescent",
        )
        .await;
        let mut ops = controls(yes());
        ops.insert(
            "restore",
            operation(
                |_| async { pending::<Outcome>().await },
                || async { Outcome::Boolean(false) },
            ),
        );
        refuses(
            proof(),
            ops,
            Options {
                termination_timeout_ms: 5,
                ..quick()
            },
            "operation_not_quiescent",
        )
        .await;
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn quiescent_restore_timeout_is_restoration_failed_not_success() {
        for stage in ["restore", "recover"] {
            let mut ops = controls(yes());
            ops.insert(
                stage,
                operation(
                    |signal| async move {
                        signal.cancelled().await;
                        Outcome::Boolean(false)
                    },
                    || async { Outcome::Boolean(true) },
                ),
            );
            refuses(proof(), ops, quick(), "restoration_failed").await;
        }
    }

    #[tokio::test]
    async fn recovery_contract_is_validated_in_full_before_injection_starts() {
        let mut checked = 0_usize;
        for stage in ["inject", "observe", "restore", "recover"] {
            for missing in ["operation", "run", "terminate"] {
                let calls = new_calls();
                let mut ops = controls(logged(&calls, "inject", true));
                match missing {
                    "operation" => drop(ops.remove(stage)),
                    "run" => ops.get_mut(stage).unwrap().run = None,
                    _ => ops.get_mut(stage).unwrap().terminate = None,
                }
                refuses(proof(), ops, quick(), "invalid_control").await;
                assert!(recorded(&calls).is_empty());
                checked = checked.checked_add(1_usize).unwrap();
            }
        }
        let extra_calls = new_calls();
        let mut ops = controls(logged(&extra_calls, "inject", true));
        ops.insert("SECRET_CANARY", yes());
        refuses(proof(), ops, quick(), "invalid_control").await;
        assert!(recorded(&extra_calls).is_empty());
        assert_eq!(checked, 12);
        for (timeout_ms, termination_timeout_ms) in [(0, 5), (5001, 5), (5, 0), (5, 5001)] {
            let calls = new_calls();
            refuses(
                proof(),
                controls(logged(&calls, "inject", true)),
                Options {
                    timeout_ms,
                    termination_timeout_ms,
                    signal: None,
                },
                "invalid_input",
            )
            .await;
            assert!(recorded(&calls).is_empty());
        }
    }

    #[tokio::test]
    async fn mutation_gate_refuses_all_prerequisite_violations_before_callbacks() {
        let mut checked = 0_usize;
        for (field, code) in [
            ("isolated", "mutation_not_isolated"),
            ("authorized", "mutation_not_isolated"),
            ("addressing_verified", "mutation_not_isolated"),
            ("known_good_observed", "restoration_failed"),
            ("recovery_armed", "restoration_failed"),
        ] {
            for bad in [json!(false), json!(null), json!(1_usize), json!("true")] {
                let calls = new_calls();
                let mut p = proof();
                p[field] = bad;
                refuses(p, controls(logged(&calls, "inject", true)), quick(), code).await;
                assert!(recorded(&calls).is_empty());
                checked = checked.checked_add(1_usize).unwrap();
            }
        }
        assert_eq!(checked, 20);
        let fields: Vec<_> = proof().as_object().unwrap().keys().cloned().collect();
        for field in fields {
            let calls = new_calls();
            let mut p = proof();
            p.as_object_mut().unwrap().remove(&field);
            refuses(
                p,
                controls(logged(&calls, "inject", true)),
                quick(),
                "invalid_input",
            )
            .await;
            assert!(recorded(&calls).is_empty());
        }
        for p in [
            json!(null),
            json!([]),
            json!({"isolated,authorized,addressing_verified":true}),
        ] {
            refuses(p, controls(yes()), quick(), "invalid_input").await;
        }
        // Recovery's prerequisite gate does not pretend to observe injection or a restored write.
        let mut p = proof();
        p["injection"] = Value::Null;
        p["recovered"] = json!(false);
        p["interrupted"] = json!("unused");
        exercise_recovery_control(p, controls(yes()), quick())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn missing_or_invalid_completion_fence_cannot_trigger_restoration() {
        let mut checked = 0_usize;
        for stage in ["inject", "observe", "restore", "recover"] {
            for missing in [true, false] {
                let calls = new_calls();
                let stopping = Arc::clone(&calls);
                let mut invalid = operation(
                    |_| async { Outcome::Invalid },
                    move || async move {
                        record(&stopping, "terminate");
                        Outcome::Boolean(true)
                    },
                );
                if missing {
                    invalid.run = Some(Box::new(|_| None));
                }
                let mut ops = controls(yes());
                ops.insert(stage, invalid);
                if ["inject", "observe"].contains(&stage) {
                    ops.insert("restore", logged(&calls, "restore", true));
                }
                ops.insert(
                    if stage == "recover" {
                        "restore"
                    } else {
                        "recover"
                    },
                    yes(),
                );
                refuses(proof(), ops, quick(), "operation_not_quiescent").await;
                assert_eq!(recorded(&calls), ["terminate"]);
                checked = checked.checked_add(1_usize).unwrap();
            }
        }
        assert_eq!(checked, 8);
    }

    #[tokio::test]
    async fn no_completion_fence_can_hide_a_detached_late_write() {
        let calls = new_calls();
        let late = Arc::clone(&calls);
        let (release_tx, release_rx) = oneshot::channel();
        let writer = tokio::spawn(async move {
            let _release_rx_result = release_rx.await;
            record(&late, "late-inject");
        });
        let mut inject = yes();
        inject.run = Some(Box::new(|_| None));
        let mut ops = controls(inject);
        ops.insert("restore", logged(&calls, "restore", true));
        ops.insert("recover", logged(&calls, "recover", true));
        refuses(proof(), ops, quick(), "operation_not_quiescent").await;
        assert!(recorded(&calls).is_empty());
        release_tx.send(()).unwrap();
        writer.await.unwrap();
        assert_eq!(recorded(&calls), ["late-inject"]);
    }

    #[test]
    fn corruption_templates_have_exact_bounded_bytes_and_a_wrong_digest() {
        let fixtures = corruption_fixtures(1024).unwrap();
        assert_eq!(fixtures.malformed, "{\"data\":");
        serde_json::from_str::<Value>(&fixtures.malformed).unwrap_err();
        let envelope: Value = serde_json::from_str(&fixtures.digest_mismatch).unwrap();
        assert_eq!(
            envelope,
            json!({"version":1_usize,"generated_at":"2026-10-07T00:00:00Z","sha256":"0".repeat(64_usize),"data":{}})
        );
        assert_ne!(
            envelope["sha256"],
            "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
        );
        let len = fixtures.digest_mismatch.len();
        assert!(fixtures.malformed.len() < 1024 && len < 1024);
        assert_eq!(corruption_fixtures(20).unwrap_err().code, "fixture_limit");
        assert_eq!(corruption_fixtures(len).unwrap_err().code, "fixture_limit");
        corruption_fixtures(len.checked_add(1_usize).unwrap()).unwrap();
        for limit in [0, 1025] {
            assert_eq!(
                corruption_fixtures(limit).unwrap_err().code,
                "invalid_input"
            );
        }
    }

    #[tokio::test]
    async fn cancellation_inside_a_completing_operation_still_terminates_and_restores() {
        for stage in ["inject", "observe"] {
            let calls = new_calls();
            let signal = Cancellation::new();
            let external = signal.clone();
            let execution_log = Arc::clone(&calls);
            let termination_log = Arc::clone(&calls);
            let mut ops = controls(logged(&calls, "inject", true));
            ops.insert(
                stage,
                operation(
                    move |_internal| async move {
                        record(&execution_log, stage);
                        external.cancel();
                        Outcome::Boolean(true)
                    },
                    move || async move {
                        record(&termination_log, "terminate");
                        Outcome::Boolean(true)
                    },
                ),
            );
            ops.insert("restore", logged(&calls, "restore", true));
            ops.insert("recover", logged(&calls, "recover", true));
            refuses(
                proof(),
                ops,
                Options {
                    signal: Some(signal),
                    ..quick()
                },
                "interrupted",
            )
            .await;
            let expected = if stage == "inject" {
                vec!["inject", "terminate", "restore", "recover"]
            } else {
                vec!["inject", "observe", "terminate", "restore", "recover"]
            };
            assert_eq!(recorded(&calls), expected);
        }
    }
}

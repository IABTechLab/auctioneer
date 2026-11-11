use super::{
    HarnessError, Result, fixture, io,
    native::{frozen_probe, runner},
    process::Supervisor,
    providers, stdio,
};
use crate::qualification::diagnostic;
use clap::{Args, Subcommand};
use serde_json::Value;
use std::path::PathBuf;
use tokio::{runtime::Builder, task::JoinHandle};

#[derive(Debug, Subcommand)]
pub enum ProviderCommand {
    /// Validate captured provider-handler controls without deployed execution.
    Offline {
        #[arg(long)]
        input: PathBuf,
    },
    /// Prepare isolated bidder metadata and a digest-bound build plan.
    Prepare {
        #[arg(long)]
        input: PathBuf,
    },
    /// Bind exact Cargo-emitted target, profile, features and artifact identity.
    CaptureBuild {
        #[arg(long)]
        input: PathBuf,
    },
    /// Stage the verified Spin component and structured manifest.
    StageSpin {
        #[arg(long)]
        input: PathBuf,
    },
    /// Stage a complete verified Fastly application release.
    StageFastly {
        #[arg(long)]
        input: PathBuf,
    },
    /// Run the pinned SDK's offline archive verifier, without deploying.
    VerifyFastlyArchive {
        #[arg(long)]
        input: PathBuf,
    },
}

#[derive(Debug, Args)]
pub struct NativeArgs {
    #[arg(long)]
    profile: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    source_commit: Option<String>,
    #[arg(long)]
    no_build: bool,
    #[arg(long)]
    prepare: bool,
}

pub enum Invocation {
    Provider(ProviderCommand),
    Native(NativeArgs),
    Fixture,
    FrozenProbe(PathBuf),
}

struct Signals {
    task: JoinHandle<()>,
    watchdog: JoinHandle<()>,
}

impl Signals {
    #[cfg(unix)]
    fn install(supervisor: &Supervisor) -> Result<Self> {
        use futures::future::{Either, select};
        use tokio::signal::unix::{SignalKind, signal};
        let mut interrupt =
            signal(SignalKind::interrupt()).map_err(|_error| HarnessError::new("signal_failed"))?;
        let mut terminate =
            signal(SignalKind::terminate()).map_err(|_error| HarnessError::new("signal_failed"))?;
        let cancellation = supervisor.clone();
        let task = tokio::spawn(async move {
            match select(Box::pin(interrupt.recv()), Box::pin(terminate.recv())).await {
                Either::Left((Some(()), _other)) => cancellation.interrupt(130),
                Either::Right((Some(()), _other)) => cancellation.interrupt(143),
                Either::Left((None, _other)) | Either::Right((None, _other)) => {
                    cancellation.interrupt(2);
                }
            }
        });
        let deadlines = supervisor.clone();
        let watchdog = tokio::spawn(async move {
            let _cancelled = deadlines.cancelled().await;
        });
        Ok(Self { task, watchdog })
    }

    #[cfg(not(unix))]
    fn install(_supervisor: &Supervisor) -> Result<Self> {
        Err(HarnessError::new("identity_unsupported"))
    }

    async fn close(self) -> Result<()> {
        self.task.abort();
        self.watchdog.abort();
        let mut failed = false;
        for task in [self.task, self.watchdog] {
            if task.await.is_err_and(|error| !error.is_cancelled()) {
                failed = true;
            }
        }
        if failed {
            Err(HarnessError::new("cleanup_failed"))
        } else {
            Ok(())
        }
    }
}

async fn provider(command: ProviderCommand, supervisor: &Supervisor) -> Result<Value> {
    match command {
        ProviderCommand::Offline { input } => {
            providers::evaluate_offline(io::read_json(&input, 1 << 20_u32)?)
        }
        ProviderCommand::Prepare { input } => {
            providers::preparation::prepare_build(&input, supervisor).await
        }
        ProviderCommand::CaptureBuild { input } => {
            providers::artifacts::capture_build(&input, supervisor).await
        }
        ProviderCommand::StageSpin { input } => {
            providers::artifacts::stage_spin(&input, supervisor).await
        }
        ProviderCommand::StageFastly { input } => {
            providers::artifacts::stage_fastly(&input, supervisor).await
        }
        ProviderCommand::VerifyFastlyArchive { input } => {
            providers::sdk::verify_fastly_archive(&input, supervisor).await
        }
    }
}

async fn emit(value: &Value) -> Result<()> {
    #[cfg(test)]
    if tests::CAPTURED_OUTPUT
        .try_with(|output| output.borrow_mut().push(value.clone()))
        .is_ok()
    {
        return Ok(());
    }
    stdio::write_json(value).await
}

#[derive(Debug, Eq, PartialEq)]
enum Completed {
    Output { value: Value, exit_code: i32 },
    Fixture,
}

async fn execute(invocation: Invocation, supervisor: &Supervisor) -> Result<Completed> {
    match invocation {
        Invocation::Fixture => fixture::protocol::serve_stdio(supervisor)
            .await
            .map(|()| Completed::Fixture),
        Invocation::FrozenProbe(evidence) => {
            supervisor.set_deadline(30_000).await?;
            let value = Box::pin(frozen_probe::run(&evidence, supervisor)).await?;
            Ok(Completed::Output {
                value,
                exit_code: 0_i32,
            })
        }
        Invocation::Provider(command) => {
            let value = provider(command, supervisor).await?;
            Ok(Completed::Output {
                value,
                exit_code: 0_i32,
            })
        }
        Invocation::Native(args) => {
            let summary = Box::pin(runner::run(
                runner::Options {
                    profile: args.profile,
                    output: args.output,
                    source_commit: args.source_commit,
                    no_build: args.no_build,
                    prepare: args.prepare,
                },
                supervisor,
            ))
            .await?;
            Ok(Completed::Output {
                value: serde_json::to_value(&summary)
                    .map_err(|_error| HarnessError::new("output_limit"))?,
                exit_code: summary.exit_code(),
            })
        }
    }
}

fn completion_decision(
    result: Result<Completed>,
    cleanup: Result<()>,
    cancellation: Result<()>,
) -> Result<Completed> {
    cleanup.and(cancellation).and(result)
}

async fn publish_after_cleanup(
    result: Result<Completed>,
    cleanup: Result<()>,
    supervisor: &Supervisor,
) -> Result<i32> {
    let cancellation = supervisor.check_completion().await;
    let completion = completion_decision(result, cleanup, cancellation);
    let published = match completion? {
        Completed::Output { value, exit_code } => emit(&value).await.map(|()| exit_code),
        Completed::Fixture => Ok(0_i32),
    };
    let after_publication = supervisor.check_completion().await;
    published.and_then(|code| after_publication.map(|()| code))
}

fn completion_code(native: bool, interruption: i32, outcome: &Result<i32>) -> i32 {
    match interruption {
        code @ (130_i32 | 143_i32) => code,
        1_i32 => 1_i32,
        0_i32 => match outcome {
            Ok(code) => *code,
            Err(error)
                if native
                    && matches!(
                        error.code,
                        "envelope_over_budget" | "candidate_mismatch" | "hardware_budget"
                    ) =>
            {
                1_i32
            }
            Err(_error) => 2_i32,
        },
        _ => 2_i32,
    }
}

async fn finish(
    result: Result<Completed>,
    native: bool,
    supervisor: &Supervisor,
    signals: Signals,
) -> i32 {
    let cleanup = supervisor.cleanup().await;
    let publication = publish_after_cleanup(result, cleanup, supervisor).await;
    // Keep signal handlers alive through cleanup and the bounded stdout write.
    let signal_cleanup = signals.close().await;
    let cancellation = supervisor.check_completion().await;
    let outcome = signal_cleanup
        .and(publication)
        .and_then(|code| cancellation.map(|()| code));
    if let Err(error) = outcome {
        let _diagnostic = diagnostic(error.code);
        if let Some(stage) = error.stage {
            let _stage = diagnostic(stage);
        }
    }
    completion_code(native, supervisor.exit_code(), &outcome)
}

async fn supervised(invocation: Invocation) -> i32 {
    let native = matches!(invocation, Invocation::Native(_));
    let supervisor = Supervisor::new();
    let signals = match Signals::install(&supervisor) {
        Ok(signals) => signals,
        Err(error) => {
            let _diagnostic = diagnostic(error.code);
            return 2;
        }
    };
    let result = Box::pin(execute(invocation, &supervisor)).await;
    finish(result, native, &supervisor, signals).await
}

pub fn run(invocation: Invocation) -> i32 {
    match Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime.block_on(supervised(invocation)),
        Err(_error) => {
            let _diagnostic = diagnostic("runtime_failed");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::pending;
    use serde_json::json;
    use std::cell::RefCell;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::{Receiver, sync_channel};
    use std::time::Duration;
    use tokio::sync::oneshot::{Sender, channel};
    use tokio::time::{Instant, sleep, timeout};

    tokio::task_local! {
        pub(super) static CAPTURED_OUTPUT: RefCell<Vec<Value>>;
    }

    fn offline_input() -> Value {
        let template: Value = serde_json::from_str(include_str!(
            "../../../../../docs/testing/fixtures/provider-handlers/cases.json"
        ))
        .unwrap();
        let cases: Vec<_> = template["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let kind = row["kind"].as_str().unwrap();
                let mutation = match kind {
                    "malformed_config" | "digest_mismatch" | "config_replacement" => json!({
                        "isolated":true,"authorized":true,"addressing_verified":true,
                        "known_good_observed":true,"recovery_armed":true,
                        "injection":"provider_write","write_visible":true,"handler_observed":true,
                        "refusal":match kind {
                            "malformed_config" => "malformed",
                            "digest_mismatch" => "integrity",
                            _ => "none",
                        },
                        "restoration":"typed_push","recovered":true,"interrupted":false
                    }),
                    _ => Value::Null,
                };
                let bids = u8::from(matches!(
                    kind,
                    "banner"
                        | "debug_denied"
                        | "debug_redacted"
                        | "headers"
                        | "completion"
                        | "config_replacement"
                ));
                json!({"id":row["id"],"kind":kind,"transport":"http",
                    "status":row["status"].as_u64().unwrap_or(200_u64),
                    "dispatch_before":0_u32,"dispatch_after":row["dispatches"],
                    "fixture_case_id":row["id"],"bid_count":bids,
                    "failure":match kind {
                        "bidder_failure" => "bidder_failure",
                        "bidder_timeout" => "bidder_timeout",
                        _ => "none",
                    },
                    "assertions_verified":true,"mutation":mutation})
            })
            .collect();
        assert_eq!(cases.len(), 14_usize);
        let digest = "a".repeat(64_usize);
        let identity = json!({"auctioneer_commit":"a".repeat(40_usize),
            "edgezero_pin":"a7d7fd0f2face027e3810ed67e80f6341fcf68b6",
            "artifact_sha256":digest,"source_sha256":digest,
            "build_features":["spin","auctioneer-config-data/test-harness-override"],
            "target":"wasm32-wasip2","runtime_version":{"major":3_u32,"minor":0_u32,"patch":0_u32},
            "harness_sha256":digest,"fixtures_sha256":digest,"reference_data_sha256":digest,
            "baked_config_sha256":digest,"fixture_metadata_sha256":digest});
        json!({"schema_version":1_u32,"kind":"offline_handler_control","provider":"spin",
            "environment":{"kind":"isolated_staging","deployment_sha256":digest,"binding_sha256":digest},
            "identity":identity,"expected_identity":identity,"cases":cases})
    }

    #[tokio::test]
    async fn output_capture_observes_the_real_publication_boundary() {
        CAPTURED_OUTPUT
            .scope(RefCell::new(Vec::new()), async {
                let value = json!({"offline_checks":"pass","publisher_pilot_ready":false});
                emit(&value).await.unwrap();
                assert_eq!(
                    CAPTURED_OUTPUT.with(|output| output.borrow().clone()),
                    vec![value]
                );
            })
            .await;
    }

    #[tokio::test]
    async fn execute_buffers_a_real_offline_report_until_final_cleanup() {
        let scratch = tempfile::tempdir().unwrap();
        let input = scratch.path().join("control.json");
        let control = offline_input();
        let expected = providers::evaluate_offline(control.clone()).unwrap();
        assert_eq!(expected["offline_checks"], "pass");
        io::write_json(&input, &control, 1 << 20_u32).unwrap();
        CAPTURED_OUTPUT
            .scope(RefCell::new(Vec::new()), async {
                let supervisor = Supervisor::new();
                let executed = execute(
                    Invocation::Provider(ProviderCommand::Offline { input }),
                    &supervisor,
                )
                .await;
                let published_before_cleanup =
                    CAPTURED_OUTPUT.with(|output| output.borrow().clone());
                supervisor.cleanup().await.unwrap();
                assert_eq!(
                    executed.unwrap(),
                    Completed::Output {
                        value: expected,
                        exit_code: 0_i32,
                    }
                );
                assert!(
                    published_before_cleanup.is_empty(),
                    "execute published an offline report before final cleanup"
                );
            })
            .await;
    }

    fn summary(category: &str, exit_code: i32) -> Completed {
        Completed::Output {
            value: json!({"category":category,"publisher_pilot_ready":false}),
            exit_code,
        }
    }

    #[test]
    fn completion_requires_cleanup_and_no_cancellation_before_any_summary_is_publishable() {
        for category in ["prepared", "evaluated", "offline", "frozen_probe"] {
            for (cleanup, cancellation, error) in [
                (
                    Err(HarnessError::new("cleanup_failed")),
                    Ok(()),
                    "cleanup_failed",
                ),
                (Ok(()), Err(HarnessError::new("interrupted")), "interrupted"),
                (
                    Ok(()),
                    Err(HarnessError::new("run_deadline")),
                    "run_deadline",
                ),
                (
                    Err(HarnessError::new("cleanup_failed")),
                    Err(HarnessError::new("interrupted")),
                    "cleanup_failed",
                ),
            ] {
                assert_eq!(
                    completion_decision(Ok(summary(category, 0_i32)), cleanup, cancellation),
                    Err(HarnessError::new(error)),
                    "{category} must not become publishable after {error}"
                );
            }
        }
        assert_eq!(
            completion_decision(Err(HarnessError::new("invalid_input")), Ok(()), Ok(())),
            Err(HarnessError::new("invalid_input"))
        );
        assert_eq!(
            completion_decision(Ok(Completed::Fixture), Ok(()), Ok(())),
            Ok(Completed::Fixture)
        );
    }

    #[test]
    fn completion_exit_codes_preserve_signal_deadline_refusal_and_output_failure_semantics() {
        for (native, interruption, outcome, expected) in [
            (false, 0_i32, Ok(0_i32), 0_i32),
            (true, 0_i32, Ok(1_i32), 1_i32),
            (false, 0_i32, Err(HarnessError::new("output_limit")), 2_i32),
            (
                true,
                0_i32,
                Err(HarnessError::new("envelope_over_budget")),
                1_i32,
            ),
            (
                true,
                0_i32,
                Err(HarnessError::new("candidate_mismatch")),
                1_i32,
            ),
            (
                true,
                0_i32,
                Err(HarnessError::new("hardware_budget")),
                1_i32,
            ),
            (false, 0_i32, Err(HarnessError::new("invalid_input")), 2_i32),
            (
                true,
                130_i32,
                Err(HarnessError::new("cleanup_failed")),
                130_i32,
            ),
            (
                false,
                143_i32,
                Err(HarnessError::new("interrupted")),
                143_i32,
            ),
            (true, 1_i32, Err(HarnessError::new("run_deadline")), 1_i32),
            (false, 2_i32, Ok(0_i32), 2_i32),
        ] {
            assert_eq!(completion_code(native, interruption, &outcome), expected);
        }
    }

    #[tokio::test]
    async fn successful_publication_allows_closed_admission_without_granting_new_work() {
        CAPTURED_OUTPUT
            .scope(RefCell::new(Vec::new()), async {
                let supervisor = Supervisor::new();
                supervisor.cleanup().await.unwrap();
                assert_eq!(
                    supervisor.check_active().await.unwrap_err().code,
                    "cleanup_failed"
                );
                for (category, code) in [("prepared", 0_i32), ("evaluated", 1_i32)] {
                    let result =
                        publish_after_cleanup(Ok(summary(category, code)), Ok(()), &supervisor)
                            .await;
                    assert_eq!(result, Ok(code));
                }
                let published = CAPTURED_OUTPUT.with(|output| output.borrow().clone());
                assert_eq!(published.len(), 2_usize);
                assert_eq!(published[0]["category"], "prepared");
                assert_eq!(published[1]["category"], "evaluated");
            })
            .await;
    }

    #[tokio::test]
    async fn cancellation_between_actual_cleanup_and_emission_suppresses_every_summary() {
        CAPTURED_OUTPUT
            .scope(RefCell::new(Vec::new()), async {
                for code in [130_i32, 143_i32, 1_i32] {
                    let supervisor = Supervisor::new();
                    let signals = Signals::install(&supervisor).unwrap();
                    supervisor.cleanup().await.unwrap();
                    assert!(!signals.task.is_finished());
                    supervisor.interrupt(code);
                    let result =
                        publish_after_cleanup(Ok(summary("prepared", 0_i32)), Ok(()), &supervisor)
                            .await;
                    signals.close().await.unwrap();
                    assert_eq!(
                        result.unwrap_err(),
                        HarnessError::new(if code == 1_i32 {
                            "run_deadline"
                        } else {
                            "interrupted"
                        })
                    );
                    assert_eq!(completion_code(true, supervisor.exit_code(), &result), code);
                    assert!(CAPTURED_OUTPUT.with(|output| output.borrow().is_empty()));
                }
            })
            .await;
    }

    struct CleanupBarrier {
        entered: Option<Sender<()>>,
        release: Receiver<()>,
        released: Arc<AtomicBool>,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for CleanupBarrier {
        fn drop(&mut self) {
            if let Some(sender) = self.entered.take() {
                let _notified = sender.send(());
            }
            self.released.store(
                self.release.recv_timeout(Duration::from_secs(3)).is_ok(),
                Ordering::Release,
            );
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn signal_during_actual_supervisor_cleanup_cannot_publish_a_prepared_summary() {
        CAPTURED_OUTPUT
            .scope(RefCell::new(Vec::new()), async {
                let supervisor = Supervisor::new();
                let signals = Signals::install(&supervisor).unwrap();
                let (started_sender, started_receiver) = channel();
                let (entered_sender, entered_receiver) = channel();
                let (release_sender, release_receiver) = sync_channel(1_usize);
                let released = Arc::new(AtomicBool::new(false));
                let barrier = CleanupBarrier {
                    entered: Some(entered_sender),
                    release: release_receiver,
                    released: Arc::clone(&released),
                };
                let _managed = supervisor
                    .task(async move {
                        let _barrier = barrier;
                        let _started = started_sender.send(());
                        pending::<()>().await;
                    })
                    .await
                    .unwrap();
                timeout(Duration::from_secs(2), started_receiver)
                    .await
                    .unwrap()
                    .unwrap();
                let interruption = async {
                    let at_cleanup = timeout(Duration::from_secs(2), entered_receiver)
                        .await
                        .is_ok_and(|entered| entered.is_ok());
                    if at_cleanup {
                        supervisor.interrupt(143_i32);
                    }
                    let released_barrier = release_sender.send(()).is_ok();
                    (at_cleanup, released_barrier)
                };
                let (code, (at_cleanup, released_barrier)) = tokio::join!(
                    finish(Ok(summary("prepared", 0_i32)), true, &supervisor, signals),
                    interruption,
                );
                assert!(at_cleanup);
                assert!(released_barrier);
                assert!(released.load(Ordering::Acquire));
                assert_eq!(code, 143_i32);
                assert!(CAPTURED_OUTPUT.with(|output| output.borrow().is_empty()));
            })
            .await;
    }

    #[tokio::test]
    async fn command_watchdog_latches_the_deadline_without_an_active_io_operation() {
        let supervisor = Supervisor::new();
        let signals = Signals::install(&supervisor).unwrap();
        supervisor.set_deadline(20).await.unwrap();
        let end = Instant::now() + Duration::from_secs(1);
        while supervisor.exit_code() == 0_i32 && Instant::now() < end {
            sleep(Duration::from_millis(5)).await;
        }
        let code = supervisor.exit_code();
        supervisor.cleanup().await.unwrap();
        signals.close().await.unwrap();
        assert_eq!(code, 1_i32);
    }
}

pub mod commands;
mod fixture;
mod http;
mod meter;
mod native;
mod providers;
mod stdio;

pub use auctioneer_cli::native_control::{HarnessError, Result};
use auctioneer_cli::native_control::{io, process};

#[cfg(test)]
mod socket_test_support {
    use std::env::{args_os, current_exe, var_os};
    use std::ffi::{OsStr, OsString};
    use std::fmt;
    use std::fs::{File, metadata};
    use std::future::Future;
    use std::io::{Read as _, Result as IoResult};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, ExitStatus, Stdio};
    use std::str::from_utf8;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::time::{Instant, sleep};

    const MARKER: &str = "AUCTIONEER__TEST__FIXTURE_SOCKET_CASE";
    const READY: &str = "SOCKET_CHILD_READY";
    const WITNESS: &str = "SOCKET_CHILD_COMPLETED";
    const CAPTURE_LIMIT: usize = 0x4000;
    const POLL: Duration = Duration::from_millis(5);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum SocketCase {
        ProtocolEof,
        ProtocolMalformed,
        FixtureStop,
        PortRebind,
        SocketCapacity,
        HeaderTimeout,
    }

    impl SocketCase {
        const ALL: [Self; 6] = [
            Self::ProtocolEof,
            Self::ProtocolMalformed,
            Self::FixtureStop,
            Self::PortRebind,
            Self::SocketCapacity,
            Self::HeaderTimeout,
        ];
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Control {
        Probe,
        Panic,
        Hold,
        Stdout,
        Stderr,
        Missing,
    }

    #[derive(Clone, Copy)]
    enum Target {
        Original(SocketCase),
        Control(Control),
    }

    impl Target {
        fn selector(self) -> &'static str {
            match self {
                Self::Original(SocketCase::ProtocolEof) => {
                    "qualification::harness::fixture::protocol::tests::private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof"
                }
                Self::Original(SocketCase::ProtocolMalformed) => {
                    "qualification::harness::fixture::protocol::tests::malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener"
                }
                Self::Original(SocketCase::FixtureStop) => {
                    "qualification::harness::fixture::server::tests::delay_applies_to_responses_and_stop_joins_held_connections_before_returning"
                }
                Self::Original(SocketCase::PortRebind) => {
                    "qualification::harness::native::system::tests::reserve_port_releases_the_loopback_listener_before_returning"
                }
                Self::Original(SocketCase::SocketCapacity) => {
                    "qualification::harness::fixture::server::tests::both_socket_populations_are_bounded_without_unbounded_waiting_tasks"
                }
                Self::Original(SocketCase::HeaderTimeout) => {
                    "qualification::harness::fixture::server::tests::header_timeout_releases_connection_capacity_without_dispatch"
                }
                Self::Control(Control::Probe) => {
                    "qualification::harness::socket_test_support::tests::parent_delegation_defers_body_and_child_runs_it_once"
                }
                Self::Control(Control::Panic) => {
                    "qualification::harness::socket_test_support::tests::child_assertion_failure_preserves_only_bounded_stderr"
                }
                Self::Control(Control::Hold) => {
                    "qualification::harness::socket_test_support::tests::timeout_refuses_and_observes_reap"
                }
                Self::Control(Control::Stdout | Control::Stderr) => {
                    "qualification::harness::socket_test_support::tests::stdout_and_stderr_overflow_are_independent_refusals"
                }
                Self::Control(Control::Missing) => {
                    "qualification::harness::socket_test_support::tests::intentionally_absent_socket_case"
                }
            }
        }

        fn marker(self) -> &'static str {
            match self {
                Self::Original(SocketCase::ProtocolEof) => "protocol-eof",
                Self::Original(SocketCase::ProtocolMalformed) => "protocol-malformed",
                Self::Original(SocketCase::FixtureStop) => "fixture-stop",
                Self::Original(SocketCase::PortRebind) => "port-rebind",
                Self::Original(SocketCase::SocketCapacity) => "socket-capacity",
                Self::Original(SocketCase::HeaderTimeout) => "header-timeout",
                Self::Control(Control::Probe) => "control-probe",
                Self::Control(Control::Panic) => "control-panic",
                Self::Control(Control::Hold) => "control-hold",
                Self::Control(Control::Stdout) => "control-stdout",
                Self::Control(Control::Stderr) => "control-stderr",
                Self::Control(Control::Missing) => "control-missing",
            }
        }

        fn needs_readiness(self) -> bool {
            matches!(
                self,
                Self::Control(Control::Hold | Control::Stdout | Control::Stderr)
            )
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Kind {
        Control,
        Setup,
        Spawn,
        Wait,
        Timeout,
        StdoutLimit,
        StderrLimit,
        Capture,
        ReportUtf8,
        Dispatch,
        Witness,
        ChildExit,
        Cleanup,
        CaptureCleanup,
    }

    // Only fixed case names and bounded stderr enter a returned diagnostic.
    #[derive(PartialEq, Eq)]
    pub(super) struct Failure {
        case: &'static str,
        kind: Kind,
        cause: Option<Kind>,
        reaped: bool,
        live: bool,
        status: Option<ExitStatus>,
        kill_failed: bool,
        stderr: String,
    }

    impl fmt::Debug for Failure {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("SocketChildFailure")
                .field("case", &self.case)
                .field("kind", &self.kind)
                .field("cause", &self.cause)
                .field("reaped", &self.reaped)
                .field("live", &self.live)
                .field("status", &self.status)
                .field("kill_failed", &self.kill_failed)
                .field("stderr", &self.stderr)
                .finish()
        }
    }

    impl Failure {
        fn new(target: Target, kind: Kind) -> Self {
            Self {
                case: target.selector(),
                kind,
                cause: None,
                reaped: false,
                live: false,
                status: None,
                kill_failed: false,
                stderr: String::new(),
            }
        }
    }

    #[derive(Clone, Copy, Default, PartialEq, Eq)]
    enum Fault {
        #[default]
        None,
        AfterReady,
    }

    #[derive(Clone, Copy)]
    struct Policy {
        execution: Duration,
        cleanup: Duration,
        fault: Fault,
    }

    impl Default for Policy {
        fn default() -> Self {
            Self {
                execution: Duration::from_secs(20),
                cleanup: Duration::from_secs(2),
                fault: Fault::None,
            }
        }
    }

    impl Policy {
        fn control() -> Self {
            Self {
                execution: Duration::from_secs(1),
                ..Self::default()
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Role {
        Parent,
        Child,
    }

    #[derive(Debug)]
    struct Receipt {
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: String,
    }

    #[derive(Debug)]
    enum Execution {
        Parent(Receipt),
        Child,
    }

    fn child_args(target: Target) -> Vec<OsString> {
        [
            "--exact",
            target.selector(),
            "--test-threads=1",
            "--nocapture",
            "--color=never",
        ]
        .map(OsString::from)
        .to_vec()
    }

    fn role(target: Target, marker: Option<&OsStr>, args: &[OsString]) -> Result<Role, Kind> {
        match marker {
            None => Ok(Role::Parent),
            Some(value) if value == target.marker() && args == child_args(target) => {
                Ok(Role::Child)
            }
            Some(_) => Err(Kind::Control),
        }
    }

    pub(super) async fn run<F, B>(case: SocketCase, body: F) -> Result<(), Failure>
    where
        F: FnOnce() -> B,
        B: Future<Output = ()>,
    {
        enter(Target::Original(case), Policy::default(), body)
            .await
            .map(|_execution| ())
    }

    async fn enter<F, B>(target: Target, policy: Policy, body: F) -> Result<Execution, Failure>
    where
        F: FnOnce() -> B,
        B: Future<Output = ()>,
    {
        let marker = var_os(MARKER);
        let args: Vec<_> = args_os().skip(1).collect();
        enter_checked(target, policy, marker.as_deref(), &args, body).await
    }

    async fn enter_checked<F, B>(
        target: Target,
        policy: Policy,
        marker: Option<&OsStr>,
        args: &[OsString],
        body: F,
    ) -> Result<Execution, Failure>
    where
        F: FnOnce() -> B,
        B: Future<Output = ()>,
    {
        match role(target, marker, args).map_err(|kind| Failure::new(target, kind))? {
            Role::Child => {
                body().await;
                Ok(Execution::Child)
            }
            Role::Parent => supervise(target, policy).await.map(Execution::Parent),
        }
    }

    fn validate_report(bytes: &[u8], selector: &str) -> Result<(), Kind> {
        let text = from_utf8(bytes).map_err(|_error| Kind::ReportUtf8)?;
        let lines: Vec<_> = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();
        let [run, case, summary] = lines.as_slice() else {
            return Err(Kind::Dispatch);
        };
        if *run != "running 1 test" || *case != format!("test {selector} ... ok") {
            return Err(Kind::Dispatch);
        }
        let tail = summary
            .strip_prefix("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; ")
            .ok_or(Kind::Dispatch)?;
        let (filtered, duration) = tail
            .split_once(" filtered out; finished in ")
            .ok_or(Kind::Dispatch)?;
        if filtered.is_empty()
            || !filtered.bytes().all(|byte| byte.is_ascii_digit())
            || filtered.parse::<usize>().is_err()
        {
            return Err(Kind::Dispatch);
        }
        let seconds = duration.strip_suffix('s').ok_or(Kind::Dispatch)?;
        let mut segments = seconds.split('.');
        let first = segments.next().ok_or(Kind::Dispatch)?;
        let fraction = segments.next();
        if first.is_empty()
            || !first.bytes().all(|byte| byte.is_ascii_digit())
            || fraction.is_some_and(|part| {
                part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit())
            })
            || segments.next().is_some()
            || !seconds.parse::<f64>().is_ok_and(f64::is_finite)
        {
            return Err(Kind::Dispatch);
        }
        Ok(())
    }

    fn verify_receipt(target: Target, receipt: &Receipt) -> Result<(), Kind> {
        if !receipt.status.success() {
            return Err(Kind::ChildExit);
        }
        validate_report(&receipt.stdout, target.selector())?;
        if matches!(target, Target::Control(Control::Probe))
            && receipt
                .stderr
                .lines()
                .filter(|line| *line == WITNESS)
                .count()
                != 1
        {
            return Err(Kind::Witness);
        }
        Ok(())
    }

    #[derive(Clone, Copy)]
    enum Stream {
        Stdout,
        Stderr,
    }

    impl Stream {
        fn limit(self) -> Kind {
            match self {
                Self::Stdout => Kind::StdoutLimit,
                Self::Stderr => Kind::StderrLimit,
            }
        }
    }

    fn read_capture(path: &Path, stream: Stream) -> Result<Vec<u8>, Kind> {
        super::io::read_bytes(path, CAPTURE_LIMIT).map_err(|error| {
            if error.code == "input_limit" {
                stream.limit()
            } else {
                Kind::Capture
            }
        })
    }

    struct ChildOwner {
        child: Child,
        directory: Option<TempDir>,
        stdout: PathBuf,
        stderr: PathBuf,
        status: Option<ExitStatus>,
        live: bool,
    }

    impl Drop for ChildOwner {
        fn drop(&mut self) {
            if self.status.is_none() {
                let _ignored = self.child.kill();
                if let Some(directory) = self.directory.take() {
                    drop(directory.keep());
                }
            }
        }
    }

    async fn watch(
        owner: &mut ChildOwner,
        target: Target,
        policy: Policy,
        deadline: Instant,
    ) -> Result<ExitStatus, Kind> {
        loop {
            if Instant::now() >= deadline {
                return Err(Kind::Timeout);
            }
            let observed_status = owner.child.try_wait().map_err(|_error| Kind::Wait)?;
            if let Some(status) = observed_status {
                owner.status = Some(status);
                return if Instant::now() >= deadline {
                    Err(Kind::Timeout)
                } else {
                    Ok(status)
                };
            }
            if target.needs_readiness() {
                let mut prefix = Vec::new();
                File::open(&owner.stderr)
                    .map_err(|_error| Kind::Capture)?
                    .take(u64::try_from(READY.len()).map_err(|_error| Kind::Capture)?)
                    .read_to_end(&mut prefix)
                    .map_err(|_error| Kind::Capture)?;
                owner.live |= prefix == READY.as_bytes();
            }
            if policy.fault == Fault::AfterReady && owner.live {
                return Err(Kind::Setup);
            }
            for (path, stream) in [
                (&owner.stdout, Stream::Stdout),
                (&owner.stderr, Stream::Stderr),
            ] {
                if metadata(path).map_err(|_error| Kind::Capture)?.len()
                    > u64::try_from(CAPTURE_LIMIT).map_err(|_error| Kind::Capture)?
                {
                    return Err(stream.limit());
                }
            }
            if Instant::now() >= deadline {
                return Err(Kind::Timeout);
            }
            sleep(POLL).await;
        }
    }

    trait CleanupOps {
        fn elapsed(&self) -> Duration;
        fn kill(&mut self) -> IoResult<()>;
        fn poll(&mut self) -> IoResult<Option<ExitStatus>>;
        async fn pause(&mut self);
    }

    struct ProcessCleanup<'a> {
        child: &'a mut Child,
        started: Instant,
    }

    impl CleanupOps for ProcessCleanup<'_> {
        fn elapsed(&self) -> Duration {
            self.started.elapsed()
        }

        fn kill(&mut self) -> IoResult<()> {
            self.child.kill()
        }

        fn poll(&mut self) -> IoResult<Option<ExitStatus>> {
            self.child.try_wait()
        }

        async fn pause(&mut self) {
            sleep(POLL).await;
        }
    }

    struct Reaped {
        status: Option<ExitStatus>,
        timely: bool,
        kill_failed: bool,
    }

    async fn terminate(operations: &mut impl CleanupOps, fence: Duration) -> Reaped {
        let kill_failed = operations.kill().is_err();
        loop {
            let status = operations.poll().ok().flatten();
            let timely = operations.elapsed() < fence;
            if status.is_some() || !timely {
                return Reaped {
                    status,
                    timely,
                    kill_failed,
                };
            }
            operations.pause().await;
        }
    }

    fn refusal(target: Target, reason: Kind, live: bool, reaped: &Reaped) -> Failure {
        let mut error = Failure::new(target, reason);
        error.reaped = reaped.status.is_some();
        error.status = reaped.status;
        error.live = live;
        error.kill_failed = reaped.kill_failed;
        if !error.reaped || !reaped.timely {
            error.kind = Kind::Cleanup;
            error.cause = Some(reason);
        }
        error
    }

    fn finish<T>(
        target: Target,
        result: Result<T, Failure>,
        reaped: bool,
        remove: impl FnOnce() -> IoResult<()>,
    ) -> Result<T, Failure> {
        if !reaped {
            let mut error = result
                .err()
                .unwrap_or_else(|| Failure::new(target, Kind::Cleanup));
            if error.kind != Kind::Cleanup && error.cause.is_none() {
                error.cause = Some(error.kind);
            }
            error.kind = Kind::Cleanup;
            return Err(error);
        }
        if remove().is_err() {
            let mut error = result
                .err()
                .unwrap_or_else(|| Failure::new(target, Kind::CaptureCleanup));
            if error.kind != Kind::CaptureCleanup && error.cause.is_none() {
                error.cause = Some(error.kind);
            }
            error.kind = Kind::CaptureCleanup;
            error.reaped = true;
            return Err(error);
        }
        result
    }

    async fn supervise(target: Target, policy: Policy) -> Result<Receipt, Failure> {
        let deadline = Instant::now()
            .checked_add(policy.execution)
            .ok_or_else(|| Failure::new(target, Kind::Setup))?;
        let directory = tempfile::tempdir().map_err(|_error| Failure::new(target, Kind::Setup))?;
        let stdout = directory.path().join("stdout");
        let stderr = directory.path().join("stderr");
        let output = File::create(&stdout).map_err(|_error| Failure::new(target, Kind::Setup))?;
        let diagnostic =
            File::create(&stderr).map_err(|_error| Failure::new(target, Kind::Setup))?;
        let child =
            Command::new(current_exe().map_err(|_error| Failure::new(target, Kind::Setup))?)
                .args(child_args(target))
                .env_clear()
                .env(MARKER, target.marker())
                .stdin(Stdio::null())
                .stdout(output)
                .stderr(diagnostic)
                .spawn()
                .map_err(|_error| Failure::new(target, Kind::Spawn))?;
        // No fallible registration or ownership transfer is allowed after spawn.
        let mut owner = ChildOwner {
            child,
            directory: Some(directory),
            stdout,
            stderr,
            status: None,
            live: false,
        };
        let observed = if Instant::now() >= deadline {
            Err(Kind::Timeout)
        } else {
            watch(&mut owner, target, policy, deadline).await
        };
        let execution_result = match observed {
            Ok(status) => {
                owner.status = Some(status);
                read_receipt(&owner, target, status)
            }
            Err(reason) => {
                let reaped = if let Some(status) = owner.status {
                    Reaped {
                        status: Some(status),
                        timely: true,
                        kill_failed: false,
                    }
                } else {
                    let mut operations = ProcessCleanup {
                        child: &mut owner.child,
                        started: Instant::now(),
                    };
                    terminate(&mut operations, policy.cleanup).await
                };
                owner.status = reaped.status;
                Err(refusal(target, reason, owner.live, &reaped))
            }
        };
        let reaped = owner.status.is_some();
        let final_result = if let Some(captures) = owner.directory.take() {
            if reaped {
                finish(target, execution_result, true, || captures.close())
            } else {
                drop(captures.keep());
                finish(target, execution_result, false, || Ok(()))
            }
        } else {
            Err(Failure::new(target, Kind::CaptureCleanup))
        };
        final_result.map_err(|mut error| {
            error.reaped = reaped;
            error.status = owner.status;
            error.live = owner.live;
            error
        })
    }

    fn read_receipt(
        owner: &ChildOwner,
        target: Target,
        status: ExitStatus,
    ) -> Result<Receipt, Failure> {
        let stdout = read_capture(&owner.stdout, Stream::Stdout)
            .map_err(|kind| Failure::new(target, kind))?;
        let stderr = read_capture(&owner.stderr, Stream::Stderr)
            .map_err(|kind| Failure::new(target, kind))?;
        let receipt = Receipt {
            status,
            stdout,
            stderr: String::from_utf8(stderr)
                .map_err(|_error| Failure::new(target, Kind::Capture))?,
        };
        match verify_receipt(target, &receipt) {
            Ok(()) => Ok(receipt),
            Err(kind) => {
                let mut error = Failure::new(target, kind);
                if kind == Kind::ChildExit {
                    error.stderr = receipt.stderr;
                }
                Err(error)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use quote::ToTokens as _;
        use std::cell::Cell;
        use std::collections::VecDeque;
        use std::fs;
        use std::io::{Error as IoError, Write as _, stderr, stdout};
        use std::pin::Pin;

        const PROBE: &str = "qualification::harness::socket_test_support::tests::parent_delegation_defers_body_and_child_runs_it_once";
        const ROW_IDS: [&str; 20] = [
            "success",
            "success_other_filtered",
            "success_whitespace",
            "failed",
            "ignored",
            "measured",
            "zero_tests",
            "multiple_tests",
            "wrong_selector",
            "missing_run",
            "duplicate_run",
            "missing_case",
            "duplicate_case",
            "missing_summary",
            "duplicate_summary",
            "truncated_summary",
            "trailing_result",
            "invalid_utf8",
            "invalid_filtered",
            "invalid_duration",
        ];

        fn transcript(selector: &str, filtered: &str, duration: &str) -> Vec<u8> {
            format!(
                "\nrunning 1 test\ntest {selector} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; {filtered} filtered out; finished in {duration}s\n"
            )
            .into_bytes()
        }

        fn report_rows() -> Vec<(&'static str, Vec<u8>, Result<(), Kind>)> {
            let valid = String::from_utf8(transcript(PROBE, "779", "0.01")).unwrap();
            let run = "running 1 test\n";
            let case = format!("test {PROBE} ... ok\n");
            let summary = valid
                .lines()
                .find(|line| line.starts_with("test result:"))
                .unwrap();
            let bad = Err(Kind::Dispatch);
            vec![
                ("success", valid.as_bytes().to_vec(), Ok(())),
                (
                    "success_other_filtered",
                    transcript(PROBE, "8", "2"),
                    Ok(()),
                ),
                (
                    "success_whitespace",
                    format!(" \n{valid}\t\n").into_bytes(),
                    Ok(()),
                ),
                (
                    "failed",
                    valid
                        .replace("1 passed; 0 failed", "0 passed; 1 failed")
                        .into_bytes(),
                    bad,
                ),
                (
                    "ignored",
                    valid.replace("0 ignored", "1 ignored").into_bytes(),
                    bad,
                ),
                (
                    "measured",
                    valid.replace("0 measured", "1 measured").into_bytes(),
                    bad,
                ),
                (
                    "zero_tests",
                    valid
                        .replace("running 1 test", "running 0 tests")
                        .replace("1 passed", "0 passed")
                        .into_bytes(),
                    bad,
                ),
                (
                    "multiple_tests",
                    valid
                        .replace("running 1 test", "running 2 tests")
                        .into_bytes(),
                    bad,
                ),
                (
                    "wrong_selector",
                    valid.replace(PROBE, "wrong::case").into_bytes(),
                    bad,
                ),
                ("missing_run", valid.replace(run, "").into_bytes(), bad),
                (
                    "duplicate_run",
                    valid.replace(run, &format!("{run}{run}")).into_bytes(),
                    bad,
                ),
                ("missing_case", valid.replace(&case, "").into_bytes(), bad),
                (
                    "duplicate_case",
                    valid.replace(&case, &format!("{case}{case}")).into_bytes(),
                    bad,
                ),
                (
                    "missing_summary",
                    valid.replace(summary, "").into_bytes(),
                    bad,
                ),
                (
                    "duplicate_summary",
                    format!("{valid}{summary}\n").into_bytes(),
                    bad,
                ),
                (
                    "truncated_summary",
                    valid.trim_end().trim_end_matches('s').as_bytes().to_vec(),
                    bad,
                ),
                (
                    "trailing_result",
                    format!("{valid}test another::case ... ok\n").into_bytes(),
                    bad,
                ),
                ("invalid_utf8", vec![0xff], Err(Kind::ReportUtf8)),
                ("invalid_filtered", transcript(PROBE, "NaN", "0.01"), bad),
                ("invalid_duration", transcript(PROBE, "779", "NaN"), bad),
            ]
        }

        fn complete_rows(rows: &[(&str, Vec<u8>, Result<(), Kind>)]) -> bool {
            let mut ids: Vec<_> = rows.iter().map(|row| row.0).collect();
            ids.sort_unstable();
            let mut expected = ROW_IDS.to_vec();
            expected.sort_unstable();
            ids == expected
        }

        fn report_contract(parser: impl Fn(&[u8], &str) -> Result<(), Kind>) -> bool {
            let rows = report_rows();
            complete_rows(&rows)
                && rows
                    .iter()
                    .all(|(_, bytes, expected)| parser(bytes, PROBE) == *expected)
        }

        fn controls(allowed: &[Control]) -> Vec<Control> {
            let Some(marker) = var_os(MARKER) else {
                return allowed.to_vec();
            };
            let selected = allowed
                .iter()
                .copied()
                .find(|control| marker == Target::Control(*control).marker())
                .expect("invalid socket child control");
            vec![selected]
        }

        async fn held(control: Control) {
            writeln!(stderr(), "{READY}").unwrap();
            let payload = vec![b'x'; 0x4001];
            match control {
                Control::Stdout => {
                    stdout().write_all(&payload).unwrap();
                    stdout().flush().unwrap();
                }
                Control::Stderr => {
                    stderr().write_all(&payload).unwrap();
                    stderr().flush().unwrap();
                }
                Control::Hold => {}
                Control::Probe | Control::Panic | Control::Missing => {
                    panic!("invalid hold control");
                }
            }
            sleep(Duration::from_secs(20)).await;
        }

        fn assert_killed(error: &Failure, kind: Kind) {
            assert_eq!(error.kind, kind);
            assert!(
                error.live,
                "child readiness and live state were not observed"
            );
            assert!(error.reaped, "child OS exit was not observed");
            let status = error.status.unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt as _;
                assert_eq!(status.signal(), Some(9_i32));
            }
            #[cfg(not(unix))]
            assert!(!status.success());
        }

        #[test]
        fn closed_case_roster_matches_six_owned_tests() {
            assert_eq!(
                SocketCase::ALL.map(|case| Target::Original(case).selector()),
                [
                    "qualification::harness::fixture::protocol::tests::private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof",
                    "qualification::harness::fixture::protocol::tests::malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener",
                    "qualification::harness::fixture::server::tests::delay_applies_to_responses_and_stop_joins_held_connections_before_returning",
                    "qualification::harness::native::system::tests::reserve_port_releases_the_loopback_listener_before_returning",
                    "qualification::harness::fixture::server::tests::both_socket_populations_are_bounded_without_unbounded_waiting_tasks",
                    "qualification::harness::fixture::server::tests::header_timeout_releases_connection_capacity_without_dispatch",
                ]
            );
            let mut markers = SocketCase::ALL
                .map(|case| Target::Original(case).marker())
                .to_vec();
            markers.sort_unstable();
            markers.dedup();
            assert_eq!(markers.len(), 6);
        }

        #[test]
        fn report_acceptance_requires_one_exact_success() {
            assert!(report_contract(validate_report));
        }

        #[test]
        fn report_control_roster_is_complete() {
            let rows = report_rows();
            assert!(complete_rows(&rows));
            assert_eq!(rows.len(), 20);
            for index in 0..rows.len() {
                let mut missing = rows.clone();
                let removed = missing.remove(index);
                assert_eq!(removed.0, rows[index].0);
                assert!(!complete_rows(&missing));
            }
            let mut duplicate = rows.clone();
            duplicate.push(rows.first().unwrap().clone());
            assert!(!complete_rows(&duplicate));
        }

        #[tokio::test]
        async fn child_role_rejects_invalid_controls_before_body() {
            let target = Target::Control(Control::Probe);
            let args = child_args(target);
            assert_eq!(role(target, None, &[]), Ok(Role::Parent));
            assert_eq!(
                role(target, Some(OsStr::new(target.marker())), &args),
                Ok(Role::Child)
            );
            for marker in ["", "unknown", "protocol-eof"] {
                assert_eq!(
                    role(target, Some(OsStr::new(marker)), &args),
                    Err(Kind::Control)
                );
                let constructed = Cell::new(0_usize);
                let error = enter_checked(
                    target,
                    Policy::default(),
                    Some(OsStr::new(marker)),
                    &args,
                    || {
                        constructed.set(1);
                        async {}
                    },
                )
                .await
                .unwrap_err();
                assert_eq!(error.kind, Kind::Control);
                assert_eq!(constructed.get(), 0);
            }
            for index in 0..args.len() {
                let mut bad = args.clone();
                bad.remove(index);
                assert_eq!(
                    role(target, Some(OsStr::new(target.marker())), &bad),
                    Err(Kind::Control)
                );
            }
            let mut extra = args.clone();
            extra.push(OsString::from("--ignored"));
            assert_eq!(
                role(target, Some(OsStr::new(target.marker())), &extra),
                Err(Kind::Control)
            );
        }

        #[tokio::test]
        async fn parent_delegation_defers_body_and_child_runs_it_once() {
            assert_eq!(delegation_contract(real_probe).await, Ok(()));
        }

        #[tokio::test]
        async fn zero_test_child_success_is_refused() {
            let error = supervise(Target::Control(Control::Missing), Policy::default())
                .await
                .unwrap_err();
            assert_eq!(error.kind, Kind::Dispatch);
            assert!(error.reaped);
            assert!(error.status.unwrap().success());
        }

        #[tokio::test]
        async fn child_assertion_failure_preserves_only_bounded_stderr() {
            let result = enter(
                Target::Control(Control::Panic),
                Policy::default(),
                || async {
                    writeln!(stdout(), "PRIVATE_STDOUT_CANARY").unwrap();
                    panic!("fixed socket child panic");
                },
            )
            .await;
            let error = result.unwrap_err();
            assert_eq!(error.kind, Kind::ChildExit);
            assert!(error.reaped);
            let diagnostic = format!("{error:?}");
            assert!(diagnostic.contains("fixed socket child panic"));
            assert!(diagnostic.contains("mod.rs:"));
            assert!(!diagnostic.contains("PRIVATE_STDOUT_CANARY"));
            assert!(!diagnostic.contains("test result:"));
            assert!(error.stderr.len() <= 0x4000);
        }

        #[tokio::test]
        async fn post_spawn_setup_failure_retains_owner_until_reap() {
            let policy = Policy {
                fault: Fault::AfterReady,
                ..Policy::control()
            };
            let error = supervise(Target::Control(Control::Hold), policy)
                .await
                .unwrap_err();
            assert_killed(&error, Kind::Setup);
        }

        #[tokio::test]
        async fn timeout_refuses_and_observes_reap() {
            let result = enter(Target::Control(Control::Hold), Policy::control(), || {
                held(Control::Hold)
            })
            .await;
            if matches!(result, Ok(Execution::Child)) {
                return;
            }
            assert_killed(&result.unwrap_err(), Kind::Timeout);
        }

        #[tokio::test]
        async fn stdout_and_stderr_overflow_are_independent_refusals() {
            for control in controls(&[Control::Stdout, Control::Stderr]) {
                let result = enter(Target::Control(control), Policy::control(), || {
                    held(control)
                })
                .await;
                if matches!(result, Ok(Execution::Child)) {
                    return;
                }
                let expected = match control {
                    Control::Stdout => Kind::StdoutLimit,
                    Control::Stderr => Kind::StderrLimit,
                    Control::Probe | Control::Panic | Control::Hold | Control::Missing => {
                        panic!("unexpected stream control");
                    }
                };
                assert_killed(&result.unwrap_err(), expected);
            }
        }

        #[test]
        fn capture_reads_pin_cap_minus_one_cap_and_plus_one() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("capture");
            for stream in [Stream::Stdout, Stream::Stderr] {
                for size in [0x3fff_usize, 0x4000, 0x4001] {
                    let bytes = vec![b'x'; size];
                    fs::write(&path, &bytes).unwrap();
                    if size <= 0x4000 {
                        assert_eq!(read_capture(&path, stream).unwrap(), bytes);
                    } else {
                        assert_eq!(read_capture(&path, stream), Err(stream.limit()));
                    }
                }
            }
            directory.close().unwrap();
        }

        #[test]
        fn helper_policy_defaults_match_independent_literals() {
            let policy = Policy::default();
            assert_eq!(policy.execution, Duration::from_secs(20));
            assert_eq!(policy.cleanup, Duration::from_secs(2));
            assert_eq!(CAPTURE_LIMIT, 0x4000);
            assert!(POLL <= Duration::from_millis(10));
        }

        fn receipt(stderr: &str) -> Receipt {
            Receipt {
                status: ExitStatus::default(),
                stdout: transcript(PROBE, "779", "0.01"),
                stderr: stderr.to_owned(),
            }
        }

        fn witness_contract(validator: impl Fn(&Receipt) -> Result<(), Kind>) -> bool {
            validator(&receipt(&format!("{WITNESS}\n"))) == Ok(())
                && validator(&receipt("")) == Err(Kind::Witness)
        }

        #[test]
        fn witness_only_candidate_is_refused() {
            assert!(witness_contract(|receipt| verify_receipt(
                Target::Control(Control::Probe),
                receipt
            )));
            assert!(!witness_contract(|receipt| validate_report(
                &receipt.stdout,
                PROBE
            )));
        }

        type Deferred<'a> = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + 'a>> + 'a>;
        type ProbeRun<'a> = Pin<Box<dyn Future<Output = Result<Execution, Failure>> + 'a>>;

        fn real_probe(body: Deferred<'_>) -> ProbeRun<'_> {
            Box::pin(enter(
                Target::Control(Control::Probe),
                Policy::default(),
                body,
            ))
        }

        fn passthrough_probe(body: Deferred<'_>) -> ProbeRun<'_> {
            Box::pin(async move {
                body().await;
                Ok(Execution::Parent(receipt(&format!("{WITNESS}\n"))))
            })
        }

        fn empty_probe(_body: Deferred<'_>) -> ProbeRun<'_> {
            Box::pin(async { Ok(Execution::Parent(receipt(""))) })
        }

        fn claimed_child_probe(body: Deferred<'_>) -> ProbeRun<'_> {
            Box::pin(async move {
                body().await;
                Ok(Execution::Child)
            })
        }

        async fn delegation_contract(
            candidate: for<'a> fn(Deferred<'a>) -> ProbeRun<'a>,
        ) -> Result<(), Kind> {
            // The caller's role is independent of the candidate's return label.
            let child_invocation = match var_os(MARKER) {
                None => false,
                Some(marker) if marker == "control-probe" => true,
                Some(_) => return Err(Kind::Control),
            };
            let constructed = Cell::new(0_usize);
            let polled = Cell::new(0_usize);
            let built = &constructed;
            let entered = &polled;
            let body: Deferred<'_> = Box::new(move || {
                built.set(built.get().checked_add(1).unwrap());
                Box::pin(async move {
                    entered.set(entered.get().checked_add(1).unwrap());
                    if child_invocation {
                        writeln!(stderr(), "{WITNESS}").unwrap();
                    }
                })
            });
            match candidate(body).await.map_err(|error| error.kind)? {
                Execution::Parent(receipt) => {
                    if child_invocation || (constructed.get(), polled.get()) != (0, 0) {
                        return Err(Kind::Control);
                    }
                    verify_receipt(Target::Control(Control::Probe), &receipt)
                }
                Execution::Child
                    if child_invocation && (constructed.get(), polled.get()) == (1, 1) =>
                {
                    Ok(())
                }
                Execution::Child => Err(Kind::Control),
            }
        }

        #[tokio::test]
        async fn contract_controls_reject_passthrough_empty_success_and_accept_all() {
            assert_eq!(
                delegation_contract(passthrough_probe).await,
                Err(Kind::Control)
            );
            assert_eq!(
                delegation_contract(claimed_child_probe).await,
                Err(Kind::Control)
            );
            assert_eq!(delegation_contract(empty_probe).await, Err(Kind::Witness));
            assert!(!report_contract(|_bytes, _selector| Ok(())));
            assert_eq!(delegation_contract(real_probe).await, Ok(()));
            assert!(report_contract(validate_report));
            assert!(witness_contract(|receipt| verify_receipt(
                Target::Control(Control::Probe),
                receipt
            )));
        }

        struct FakeCleanup {
            polls: VecDeque<Option<ExitStatus>>,
            kill_error: bool,
            elapsed: Duration,
            poll_count: usize,
        }

        impl CleanupOps for FakeCleanup {
            fn elapsed(&self) -> Duration {
                self.elapsed
            }

            fn kill(&mut self) -> IoResult<()> {
                if self.kill_error {
                    Err(IoError::other("fixed kill error"))
                } else {
                    Ok(())
                }
            }

            fn poll(&mut self) -> IoResult<Option<ExitStatus>> {
                self.poll_count = self.poll_count.checked_add(1).unwrap();
                Ok(self.polls.pop_front().flatten())
            }

            async fn pause(&mut self) {
                self.elapsed = self.elapsed.checked_add(Duration::from_secs(1)).unwrap();
            }
        }

        #[tokio::test]
        async fn cleanup_polls_after_kill_error() {
            let mut operations = FakeCleanup {
                polls: [None, Some(ExitStatus::default())].into(),
                kill_error: true,
                elapsed: Duration::ZERO,
                poll_count: 0,
            };
            let result = terminate(&mut operations, Duration::from_secs(2)).await;
            let error = refusal(
                Target::Control(Control::Hold),
                Kind::Timeout,
                false,
                &result,
            );
            assert_eq!(operations.poll_count, 2);
            assert_eq!(error.kind, Kind::Timeout);
            assert!(error.kill_failed);
            assert!(error.reaped);
        }

        #[tokio::test]
        async fn cleanup_fence_failure_overrides_original_refusal() {
            let mut operations = FakeCleanup {
                polls: [None, None, None].into(),
                kill_error: true,
                elapsed: Duration::ZERO,
                poll_count: 0,
            };
            let result = terminate(&mut operations, Duration::from_secs(2)).await;
            let error = refusal(
                Target::Control(Control::Hold),
                Kind::Timeout,
                false,
                &result,
            );
            assert_eq!(operations.poll_count, 3);
            assert_eq!(error.kind, Kind::Cleanup);
            assert_eq!(error.cause, Some(Kind::Timeout));
            assert!(!error.reaped);
            assert!(error.kill_failed);
        }

        #[test]
        fn capture_removal_failure_is_reported() {
            let target = Target::Control(Control::Hold);
            let removed = Cell::new(0_usize);
            let failure = Failure::new(target, Kind::Timeout);
            let error = finish::<()>(target, Err(failure), true, || {
                removed.set(removed.get().checked_add(1).unwrap());
                Err(IoError::other("fixed removal error"))
            })
            .unwrap_err();
            assert_eq!(error.kind, Kind::CaptureCleanup);
            assert_eq!(error.cause, Some(Kind::Timeout));
            assert_eq!(removed.get(), 1);
            assert_eq!(
                finish(target, Ok(()), true, || {
                    removed.set(removed.get().checked_add(1).unwrap());
                    Ok(())
                }),
                Ok(())
            );
            assert_eq!(removed.get(), 2);
            let unreaped_error = finish::<()>(
                target,
                Err(Failure::new(target, Kind::Timeout)),
                false,
                || {
                    removed.set(removed.get().checked_add(1).unwrap());
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(unreaped_error.kind, Kind::Cleanup);
            assert_eq!(removed.get(), 2);
        }

        #[test]
        fn capture_removal_failure_preserves_original_cleanup_cause() {
            let target = Target::Control(Control::Hold);
            let late_reap = Reaped {
                status: Some(ExitStatus::default()),
                timely: false,
                kill_failed: false,
            };
            let original = refusal(target, Kind::Timeout, true, &late_reap);
            assert_eq!(original.kind, Kind::Cleanup);
            assert_eq!(original.cause, Some(Kind::Timeout));
            assert!(original.reaped);
            let combined = finish::<()>(target, Err(original), true, || {
                Err(IoError::other("fixed removal error"))
            })
            .unwrap_err();
            assert_eq!(combined.kind, Kind::CaptureCleanup);
            assert_eq!(combined.cause, Some(Kind::Timeout));
            assert!(combined.reaped);
        }

        const OWNERS: [(&str, &str, usize); 6] = [
            (
                "private_fixture_channel_announces_the_bound_port_and_closes_it_on_eof",
                "ProtocolEof",
                0,
            ),
            (
                "malformed_or_unknown_private_frames_cannot_echo_input_or_leave_a_listener",
                "ProtocolMalformed",
                0,
            ),
            (
                "delay_applies_to_responses_and_stop_joins_held_connections_before_returning",
                "FixtureStop",
                1,
            ),
            (
                "reserve_port_releases_the_loopback_listener_before_returning",
                "PortRebind",
                2,
            ),
            (
                "both_socket_populations_are_bounded_without_unbounded_waiting_tasks",
                "SocketCapacity",
                1,
            ),
            (
                "header_timeout_releases_connection_capacity_without_dispatch",
                "HeaderTimeout",
                1,
            ),
        ];

        fn sources() -> Vec<syn::File> {
            [
                include_str!("fixture/protocol.rs"),
                include_str!("fixture/server/mod.rs"),
                include_str!("native/system.rs"),
            ]
            .map(|source| syn::parse_file(source).unwrap())
            .to_vec()
        }

        fn disabled(attributes: &[syn::Attribute]) -> bool {
            attributes.iter().any(|attribute| {
                attribute.path().is_ident("ignore")
                    || attribute.path().is_ident("cfg_attr")
                    || (attribute.path().is_ident("cfg")
                        && !attribute
                            .parse_args::<syn::Path>()
                            .is_ok_and(|path| path.is_ident("test")))
            })
        }

        fn functions<'a>(
            items: &'a [syn::Item],
            blocked: bool,
            found: &mut Vec<(&'a syn::ItemFn, bool)>,
        ) {
            for item in items {
                if let syn::Item::Fn(function) = item {
                    found.push((function, blocked || disabled(&function.attrs)));
                } else if let syn::Item::Mod(module) = item
                    && let Some((_, contents)) = &module.content
                {
                    functions(contents, blocked || disabled(&module.attrs), found);
                }
            }
        }

        fn wrapper(function: &syn::ItemFn, variant: &str) -> bool {
            if !function
                .attrs
                .iter()
                .any(|attribute| attribute.path().to_token_stream().to_string() == "tokio :: test")
            {
                return false;
            }
            if function.block.stmts.len() != 1 {
                return false;
            }
            let Some(syn::Stmt::Expr(syn::Expr::MethodCall(unwrap), Some(_))) =
                function.block.stmts.first()
            else {
                return false;
            };
            if unwrap.method != "unwrap" || !unwrap.args.is_empty() {
                return false;
            }
            let syn::Expr::Await(awaited) = unwrap.receiver.as_ref() else {
                return false;
            };
            let syn::Expr::Call(call) = awaited.base.as_ref() else {
                return false;
            };
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return false;
            };
            if path.path.to_token_stream().to_string() != "socket_test_support :: run"
                || call.args.len() != 2
            {
                return false;
            }
            let Some(syn::Expr::Path(case)) = call.args.first() else {
                return false;
            };
            if case.path.to_token_stream().to_string() != format!("SocketCase :: {variant}") {
                return false;
            }
            let Some(syn::Expr::Closure(factory)) = call.args.last() else {
                return false;
            };
            if !factory.inputs.is_empty() {
                return false;
            }
            let syn::Expr::Async(body) = factory.body.as_ref() else {
                return false;
            };
            !body.block.stmts.is_empty()
        }

        fn wrappers(files: &[syn::File]) -> bool {
            OWNERS.iter().all(|(name, variant, owner)| {
                let mut found = Vec::new();
                for (index, file) in files.iter().enumerate() {
                    let mut candidates = Vec::new();
                    functions(&file.items, disabled(&file.attrs), &mut candidates);
                    found.extend(
                        candidates
                            .into_iter()
                            .filter(|(function, _)| function.sig.ident == *name)
                            .map(|(function, blocked)| (function, blocked, index)),
                    );
                }
                matches!(found.as_slice(), [(function, false, index)] if index == owner && wrapper(function, variant))
            })
        }

        fn change(items: &mut [syn::Item], name: &str, mutate: fn(&mut syn::ItemFn)) {
            for item in items {
                if let syn::Item::Fn(function) = item {
                    if function.sig.ident == name {
                        mutate(function);
                    }
                } else if let syn::Item::Mod(module) = item
                    && let Some((_, contents)) = &mut module.content
                {
                    change(contents, name, mutate);
                }
            }
        }

        #[test]
        fn owned_wrappers_delegate_before_constructing_resources() {
            assert!(wrappers(&sources()));
        }

        #[test]
        fn wrapper_guard_rejects_missing_mismatched_and_disabled_cases() {
            let original = sources();
            assert!(wrappers(&original));
            let mut absent = original.clone();
            change(
                &mut absent.first_mut().unwrap().items,
                OWNERS.first().unwrap().0,
                |function| {
                    *function.block = syn::parse_quote!({});
                },
            );
            assert!(!wrappers(&absent));
            let mut disabled = original.clone();
            change(
                &mut disabled.first_mut().unwrap().items,
                OWNERS.first().unwrap().0,
                |function| {
                    function.attrs.push(syn::parse_quote!(#[cfg(any())]));
                },
            );
            assert!(!wrappers(&disabled));
            let blocked_file_attributes: [syn::Attribute; 2] = [
                syn::parse_quote!(#![cfg(any())]),
                syn::parse_quote!(#![cfg_attr(test, cfg(any()))]),
            ];
            for attribute in blocked_file_attributes {
                let mut disabled_file = original.clone();
                disabled_file.first_mut().unwrap().attrs.push(attribute);
                assert!(!wrappers(&disabled_file));
            }
            let mut enabled_file = original.clone();
            enabled_file
                .first_mut()
                .unwrap()
                .attrs
                .push(syn::parse_quote!(#![cfg(test)]));
            assert!(wrappers(&enabled_file));
            let mut unregistered = original.clone();
            change(
                &mut unregistered.first_mut().unwrap().items,
                OWNERS.first().unwrap().0,
                |function| {
                    function.attrs.clear();
                },
            );
            assert!(!wrappers(&unregistered));
            let mut wrong = original.clone();
            change(
                &mut wrong.first_mut().unwrap().items,
                OWNERS.first().unwrap().0,
                |function| {
                    *function.block = syn::parse_quote!({
                        socket_test_support::run(SocketCase::HeaderTimeout, || async {
                            assert!(true);
                        })
                        .await
                        .unwrap();
                    });
                },
            );
            assert!(!wrappers(&wrong));
        }
    }
}

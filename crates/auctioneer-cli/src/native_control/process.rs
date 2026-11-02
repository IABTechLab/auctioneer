//! Bounded child-process and task ownership with explicit cleanup fences.
//!
//! Captured stdout is bounded, not trusted or redacted. Callers must validate it
//! before use or publication; stderr is counted and hashed but not returned.

use super::{HarnessError, Result, io};
use futures::future::{Either, join_all, select};
#[cfg(unix)]
use rustix::{
    io::Errno,
    process::{Pid, Signal, kill_process_group, test_kill_process_group},
};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWriteExt as _};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, Notify};
use tokio::task::{AbortHandle, JoinHandle};
use tokio::time::{Instant, sleep, sleep_until, timeout};

/// A child command with an explicitly populated environment and bounded output.
pub struct ProcessSpec {
    /// Executable selected by the caller.
    pub program: PathBuf,
    /// Arguments passed without a shell wrapper.
    pub args: Vec<OsString>,
    /// Working directory, or the parent's directory when absent.
    pub cwd: Option<PathBuf>,
    /// Complete environment after clearing all inherited variables.
    pub env: BTreeMap<OsString, OsString>,
    /// Child deadline in milliseconds, from 1 through 1,200,000.
    pub timeout_ms: u64,
    /// Combined stdout/stderr byte cap, from 1 through 128 MiB.
    pub output_cap: usize,
    /// Whether to provide a private stdin pipe instead of a null input.
    pub stdin: bool,
}

impl ProcessSpec {
    /// Select a program with a 10-second deadline and 32-KiB output cap.
    #[must_use]
    pub fn new<P: Into<PathBuf>>(program: P) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
            timeout_ms: 10_000,
            output_cap: 32 << 10,
            stdin: false,
        }
    }
}

/// Aggregate child-output identity without retaining stderr contents.
#[derive(Clone, Debug, Serialize)]
pub struct ProcessSummary {
    /// Combined bytes observed from stdout and stderr.
    pub output_bytes: usize,
    /// Combined output digest in observed read order.
    pub output_sha256: String,
    /// Whether the direct child has been reaped, not proof of group cleanup.
    pub closed: bool,
}

/// Bounded child stdout and aggregate output identity.
pub struct CommandOutput {
    /// Untrusted, unredacted stdout; callers must validate it before publication.
    pub stdout: Vec<u8>,
    /// Byte count, digest and direct-child reap status.
    pub summary: ProcessSummary,
}

impl fmt::Debug for CommandOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandOutput")
            .field("stdout_bytes", &self.stdout.len())
            .field("summary", &self.summary)
            .finish()
    }
}

/// Shared owner of child processes and tasks with latched cancellation.
#[derive(Clone)]
pub struct Supervisor {
    inner: Arc<SupervisorState>,
}

/// A supervised task retained until its abort and join fence completes.
#[derive(Clone)]
pub struct ManagedTask {
    inner: Arc<TaskState>,
}

struct TaskState {
    handle: Mutex<Option<JoinHandle<()>>>,
    abort: AbortHandle,
    joined: AtomicBool,
    failed: AtomicBool,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for TaskState {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

impl ManagedTask {
    /// Request cancellation; the supervisor still owns the task until it joins.
    pub fn abort(&self) {
        self.inner.abort.abort();
    }

    async fn join(&self) -> Result<()> {
        let mut guard = self.inner.handle.lock().await;
        if let Some(handle) = guard.as_mut() {
            let completed = match timeout(Duration::from_secs(12), &mut *handle).await {
                Ok(Ok(())) => true,
                Ok(Err(error)) if error.is_cancelled() => true,
                Ok(Err(_error)) => {
                    self.inner.failed.store(true, Ordering::Release);
                    true
                }
                Err(_elapsed) => {
                    self.inner.failed.store(true, Ordering::Release);
                    self.abort();
                    match timeout(Duration::from_secs(2), &mut *handle).await {
                        Ok(_result) => true,
                        Err(_join_deadline) => false,
                    }
                }
            };
            if completed {
                guard.take();
                self.inner.joined.store(true, Ordering::Release);
            }
        }
        if self.inner.failed.load(Ordering::Acquire) {
            Err(HarnessError::new("cleanup_failed"))
        } else {
            Ok(())
        }
    }
}

struct SupervisorState {
    processes: Mutex<Vec<ManagedProcess>>,
    tasks: Mutex<Vec<ManagedTask>>,
    deadline: Mutex<Option<Instant>>,
    interruption: AtomicI32,
    closing: AtomicBool,
    cleanup_failed: AtomicBool,
    changed: Notify,
}

impl Supervisor {
    /// Observe registered task ownership without mutating it.
    #[cfg(any(test, feature = "test-support"))]
    pub async fn registered_task_count(&self) -> usize {
        self.inner.tasks.lock().await.len()
    }
    /// Create an active owner with no deadline or inherited interruption.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SupervisorState {
                processes: Mutex::new(Vec::new()),
                tasks: Mutex::new(Vec::new()),
                deadline: Mutex::new(None),
                interruption: AtomicI32::new(0),
                closing: AtomicBool::new(false),
                cleanup_failed: AtomicBool::new(false),
                changed: Notify::new(),
            }),
        }
    }

    /// Set the run deadline relative to the current instant.
    ///
    /// # Errors
    /// Returns `invalid_input` unless the deadline is from 1 through 1,200,000 ms.
    pub async fn set_deadline(&self, milliseconds: u64) -> Result<()> {
        if !(1..=1_200_000).contains(&milliseconds) {
            return Err(HarnessError::new("invalid_input"));
        }
        *self.inner.deadline.lock().await =
            Instant::now().checked_add(Duration::from_millis(milliseconds));
        self.inner.changed.notify_waiters();
        Ok(())
    }

    /// Latch the first interruption code and wake cancellation observers.
    pub fn interrupt(&self, code: i32) {
        let _latched =
            self.inner
                .interruption
                .compare_exchange(0, code, Ordering::AcqRel, Ordering::Acquire);
        self.inner.changed.notify_waiters();
    }

    /// Return the latched interruption code, or zero while none is latched.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        self.inner.interruption.load(Ordering::Acquire)
    }

    /// Refuse new work once cancelled, expired or closing.
    ///
    /// # Errors
    /// Returns the fixed interruption, deadline or cleanup failure category.
    pub async fn check_active(&self) -> Result<()> {
        self.check_completion().await?;
        if self.inner.closing.load(Ordering::Acquire) {
            return Err(HarnessError::new("cleanup_failed"));
        }
        Ok(())
    }

    /// Check interruption and deadline state, including after cleanup.
    ///
    /// # Errors
    /// Returns the fixed interruption, deadline or invalid-state category.
    pub async fn check_completion(&self) -> Result<()> {
        if self
            .inner
            .deadline
            .lock()
            .await
            .is_some_and(|end| Instant::now() >= end)
        {
            self.interrupt(1);
        }
        match self.exit_code() {
            0_i32 => Ok(()),
            1_i32 => Err(HarnessError::new("run_deadline")),
            130_i32 | 143_i32 => Err(HarnessError::new("interrupted")),
            _ => Err(HarnessError::new("cleanup_failed")),
        }
    }

    /// Wait until the owner refuses further work and return its fixed error.
    pub async fn cancelled(&self) -> HarnessError {
        loop {
            let changed = self.inner.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if let Err(error) = self.check_active().await {
                return error;
            }
            let deadline = *self.inner.deadline.lock().await;
            if let Some(end) = deadline {
                let _wake = select(changed, Box::pin(sleep_until(end))).await;
            } else {
                changed.await;
            }
        }
    }

    /// Start and register a bounded child while the owner remains active.
    ///
    /// # Errors
    /// Returns fixed state, child-limit, input, platform, spawn or cleanup errors.
    pub async fn spawn(&self, spec: ProcessSpec) -> Result<ManagedProcess> {
        self.check_active().await?;
        let mut processes = self.inner.processes.lock().await;
        self.check_active().await?;
        if processes.len() >= 64 {
            return Err(HarnessError::new("child_limit"));
        }
        let process = ManagedProcess::spawn(spec).await?;
        processes.push(process.clone());
        drop(processes);
        if let Err(error) = self.check_active().await {
            self.stop(&process).await?;
            return Err(error);
        }
        Ok(process)
    }

    /// Run a child to completion or cancellation, then stop its process group.
    ///
    /// # Errors
    /// Returns fixed spawn, cancellation, child or cleanup errors, never stderr.
    pub async fn run(&self, spec: ProcessSpec) -> Result<CommandOutput> {
        let process = self.spawn(spec).await?;
        let result = match select(Box::pin(self.cancelled()), Box::pin(process.result())).await {
            Either::Left((error, _pending)) => Err(error),
            Either::Right((output, _pending)) => output,
        };
        self.stop(&process).await?;
        result
    }

    /// Register an asynchronous task while the owner remains active.
    ///
    /// # Errors
    /// Returns a fixed state error or `child_limit` when task slots are exhausted.
    pub async fn task<F: Future<Output = ()> + Send + 'static>(
        &self,
        future: F,
    ) -> Result<ManagedTask> {
        self.check_active().await?;
        let mut tasks = self.inner.tasks.lock().await;
        self.check_active().await?;
        if tasks.len() >= 64 {
            return Err(HarnessError::new("child_limit"));
        }
        let handle = tokio::spawn(future);
        let task = ManagedTask {
            inner: Arc::new(TaskState {
                abort: handle.abort_handle(),
                handle: Mutex::new(Some(handle)),
                joined: AtomicBool::new(false),
                failed: AtomicBool::new(false),
            }),
        };
        tasks.push(task.clone());
        Ok(task)
    }

    /// Join a task before releasing its ownership slot.
    ///
    /// # Errors
    /// Returns `cleanup_failed` if the task fails or cannot be joined in time.
    pub async fn join_task(&self, task: &ManagedTask) -> Result<()> {
        task.join().await?;
        self.inner
            .tasks
            .lock()
            .await
            .retain(|registered| !Arc::ptr_eq(&registered.inner, &task.inner));
        Ok(())
    }

    /// Stop a child group before releasing its ownership slot.
    ///
    /// # Errors
    /// Returns `cleanup_failed` unless group termination and joining are proven.
    pub async fn stop(&self, process: &ManagedProcess) -> Result<()> {
        process.stop().await?;
        self.inner
            .processes
            .lock()
            .await
            .retain(|registered| !Arc::ptr_eq(&registered.inner, &process.inner));
        Ok(())
    }

    /// Close admission, abort tasks and stop all registered child groups.
    ///
    /// # Errors
    /// Returns latched `cleanup_failed` if any cleanup fence cannot be proven.
    pub async fn cleanup(&self) -> Result<()> {
        self.inner.closing.store(true, Ordering::Release);
        self.inner.changed.notify_waiters();
        let tasks = self.inner.tasks.lock().await.clone();
        for task in &tasks {
            task.abort();
        }
        let processes = self.inner.processes.lock().await.clone();
        let results = join_all(processes.iter().map(ManagedProcess::stop)).await;
        let async_results = join_all(tasks.iter().map(ManagedTask::join)).await;
        self.inner
            .processes
            .lock()
            .await
            .retain(|process| !process.inner.cleaned.load(Ordering::Acquire));
        self.inner
            .tasks
            .lock()
            .await
            .retain(|task| !task.inner.joined.load(Ordering::Acquire));
        if results
            .into_iter()
            .chain(async_results)
            .any(|result| result.is_err())
        {
            self.inner.cleanup_failed.store(true, Ordering::Release);
        }
        if self.inner.cleanup_failed.load(Ordering::Acquire) {
            return Err(HarnessError::new("cleanup_failed"));
        }
        Ok(())
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

/// A registered child whose process group remains owned until cleanup.
#[derive(Clone)]
pub struct ManagedProcess {
    inner: Arc<ProcessState>,
}

struct CapturedOutput {
    bytes: usize,
    digest: Sha256,
    stdout: VecDeque<u8>,
}

struct ProcessState {
    pid: u32,
    cap: usize,
    output: Mutex<CapturedOutput>,
    stdin: Mutex<Option<ChildStdin>>,
    error: Mutex<Option<HarnessError>>,
    closed: AtomicBool,
    cleaned: AtomicBool,
    stopping: AtomicBool,
    readers_done: AtomicU8,
    failure: Notify,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    cleanup_lock: Mutex<()>,
}

impl fmt::Debug for ManagedProcess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManagedProcess")
            .field("pid", &self.pid())
            .finish_non_exhaustive()
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for ProcessState {
    fn drop(&mut self) {
        if !self.cleaned.load(Ordering::Acquire) {
            let _termination = signal_group(self.pid, GroupSignal::Kill);
        }
    }
}

impl ProcessState {
    async fn fail(&self, code: &'static str) {
        let mut error = self.error.lock().await;
        if error.is_none() || code == "cleanup_failed" {
            *error = Some(HarnessError::new(code));
        }
        self.failure.notify_one();
    }
}

impl ManagedProcess {
    async fn spawn(spec: ProcessSpec) -> Result<Self> {
        if !(1..=128 * 1024 * 1024).contains(&spec.output_cap)
            || !(1..=1_200_000).contains(&spec.timeout_ms)
        {
            return Err(HarnessError::new("invalid_input"));
        }
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .env_clear()
            .envs(spec.env)
            .stdin(if spec.stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = spec.cwd {
            command.current_dir(cwd);
        }
        #[cfg(unix)]
        command.process_group(0);
        #[cfg(not(unix))]
        return Err(HarnessError::new("platform_unsupported"));
        let mut child = command
            .spawn()
            .map_err(|_error| HarnessError::new("child_spawn"))?;
        let pid = child
            .id()
            .filter(|id| *id > 1)
            .ok_or(HarnessError::new("child_spawn"))?;
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            signal_group(pid, GroupSignal::Kill)?;
            let _wait = timeout(Duration::from_secs(2), child.wait()).await;
            return Err(HarnessError::new("child_spawn"));
        };
        let state = Arc::new(ProcessState {
            pid,
            cap: spec.output_cap,
            output: Mutex::new(CapturedOutput {
                bytes: 0,
                digest: Sha256::new(),
                stdout: VecDeque::new(),
            }),
            stdin: Mutex::new(child.stdin.take()),
            error: Mutex::new(None),
            closed: AtomicBool::new(false),
            cleaned: AtomicBool::new(false),
            stopping: AtomicBool::new(false),
            readers_done: AtomicU8::new(0),
            failure: Notify::new(),
            tasks: Mutex::new(Vec::new()),
            cleanup_lock: Mutex::new(()),
        });
        let stdout_state = Arc::clone(&state);
        let stdout_task = tokio::spawn(async move {
            collect(stdout, stdout_state, true).await;
        });
        let stderr_state = Arc::clone(&state);
        let stderr_task = tokio::spawn(async move {
            collect(stderr, stderr_state, false).await;
        });
        let monitor = tokio::spawn(monitor_child(child, Arc::clone(&state), spec.timeout_ms));
        state
            .tasks
            .lock()
            .await
            .extend([stdout_task, stderr_task, monitor]);
        Ok(Self { inner: state })
    }

    /// Return the direct child's PID, also its Unix process-group ID.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.inner.pid
    }
    /// Return whether the direct child has been reaped, not group cleanup state.
    #[must_use]
    pub fn closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }

    /// Check that the direct child has no recorded failure and has not exited.
    ///
    /// # Errors
    /// Returns the recorded child failure or `child_exited`.
    pub async fn assert_running(&self) -> Result<()> {
        if let Some(error) = *self.inner.error.lock().await {
            return Err(error);
        }
        if self.closed() {
            return Err(HarnessError::new("child_exited"));
        }
        Ok(())
    }

    /// Snapshot aggregate output counters and direct-child reap status.
    pub async fn summary(&self) -> ProcessSummary {
        let output = self.inner.output.lock().await;
        ProcessSummary {
            output_bytes: output.bytes,
            output_sha256: format!("{:x}", output.digest.clone().finalize()),
            closed: self.closed(),
        }
    }

    /// Write and flush a private JSON line with a 4096-byte body cap.
    ///
    /// # Errors
    /// Returns fixed serialization, child-state, timeout or pipe errors.
    pub async fn write_json<T: Serialize>(&self, value: &T) -> Result<()> {
        let bytes = io::json_bytes(value, 4096)?;
        self.assert_running().await?;
        let mut guard = self.inner.stdin.lock().await;
        let stdin = guard
            .as_mut()
            .ok_or(HarnessError::new("invalid_child_output"))?;
        timeout(Duration::from_secs(2), async {
            stdin.write_all(&bytes).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await
        })
        .await
        .map_err(|_error| HarnessError::new("child_timeout"))?
        .map_err(|_error| HarnessError::new("child_exited"))
    }

    async fn read_line(&self) -> Result<Vec<u8>> {
        let end = Instant::now()
            .checked_add(Duration::from_secs(2))
            .ok_or(HarnessError::new("child_timeout"))?;
        loop {
            if let Some(error) = *self.inner.error.lock().await {
                return Err(error);
            }
            {
                let mut output = self.inner.output.lock().await;
                if let Some(newline) = output.stdout.iter().position(|byte| *byte == b'\n') {
                    if newline > 4096 {
                        return Err(HarnessError::new("invalid_child_output"));
                    }
                    let line = output.stdout.drain(..newline).collect();
                    output.stdout.pop_front();
                    return Ok(line);
                }
                if output.stdout.len() > 4096 {
                    return Err(HarnessError::new("invalid_child_output"));
                }
            }
            if self.closed() && self.inner.readers_done.load(Ordering::Acquire) == 2 {
                return Err(HarnessError::new("child_exited"));
            }
            if Instant::now() >= end {
                return Err(HarnessError::new("child_timeout"));
            }
            sleep(Duration::from_millis(5)).await;
        }
    }

    /// Read one typed private JSON line within the 4096-byte and 2-second limits.
    ///
    /// # Errors
    /// Returns fixed child, timeout or invalid-output errors.
    pub async fn read_json_line<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.read_line().await?)
            .map_err(|_error| HarnessError::new("invalid_child_output"))
    }

    /// Wait for child completion and prove cleanup before returning stdout.
    ///
    /// # Errors
    /// Returns recorded child errors or `cleanup_failed` when cleanup is unproven.
    pub async fn result(&self) -> Result<CommandOutput> {
        while !self.closed() {
            if self
                .inner
                .error
                .lock()
                .await
                .is_some_and(|error| error.code == "cleanup_failed")
            {
                self.stop().await?;
                return Err(HarnessError::new("cleanup_failed"));
            }
            sleep(Duration::from_millis(5)).await;
        }
        self.stop().await?;
        if let Some(error) = *self.inner.error.lock().await {
            return Err(error);
        }
        let stdout = self
            .inner
            .output
            .lock()
            .await
            .stdout
            .iter()
            .copied()
            .collect();
        Ok(CommandOutput {
            stdout,
            summary: self.summary().await,
        })
    }

    /// Escalate group termination and join all child-owned tasks.
    ///
    /// # Errors
    /// Returns `cleanup_failed` unless reap, group absence and joins are proven.
    pub async fn stop(&self) -> Result<()> {
        let _cleanup = self.inner.cleanup_lock.lock().await;
        if self.inner.cleaned.load(Ordering::Acquire) {
            return Ok(());
        }
        self.inner.stopping.store(true, Ordering::Release);
        self.inner.stdin.lock().await.take();
        for signal in [GroupSignal::Interrupt, GroupSignal::Term, GroupSignal::Kill] {
            if self.closed() && !group_alive(self.pid())? {
                break;
            }
            let end = Instant::now()
                .checked_add(Duration::from_millis(if signal == GroupSignal::Kill {
                    1000
                } else {
                    100
                }))
                .ok_or(HarnessError::new("cleanup_failed"))?;
            while Instant::now() < end {
                // A signal may race the kernel's reap (Darwin can return EPERM
                // for that dead group). Only the reap + absence fence below
                // proves cleanup; permission-unknown/live groups still fail it.
                let _request = signal_group(self.pid(), signal);
                if self.closed() && !group_alive(self.pid())? {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
            }
        }
        if !self.closed() || group_alive(self.pid())? {
            return Err(HarnessError::new("cleanup_failed"));
        }
        let mut tasks = self.inner.tasks.lock().await;
        while let Some(task) = tasks.last_mut() {
            match timeout(Duration::from_secs(2), &mut *task).await {
                Ok(Ok(())) => {
                    let _completed = tasks.pop();
                }
                Ok(Err(_error)) => {
                    let _completed = tasks.pop();
                    self.inner.fail("cleanup_failed").await;
                    return Err(HarnessError::new("cleanup_failed"));
                }
                Err(_timeout) => {
                    task.abort();
                    if timeout(Duration::from_secs(2), &mut *task).await.is_ok() {
                        let _completed = tasks.pop();
                    }
                    self.inner.fail("cleanup_failed").await;
                    return Err(HarnessError::new("cleanup_failed"));
                }
            }
        }
        if self
            .inner
            .error
            .lock()
            .await
            .is_some_and(|error| error.code == "cleanup_failed")
        {
            return Err(HarnessError::new("cleanup_failed"));
        }
        self.inner.cleaned.store(true, Ordering::Release);
        Ok(())
    }
}

async fn monitor_child(mut child: Child, state: Arc<ProcessState>, timeout_ms: u64) {
    let first_status = match select(
        Box::pin(child.wait()),
        Box::pin(select(
            Box::pin(sleep(Duration::from_millis(timeout_ms))),
            Box::pin(state.failure.notified()),
        )),
    )
    .await
    {
        Either::Left((status, _pending)) => Some(status),
        Either::Right((Either::Left(((), _pending)), _wait)) => {
            state.fail("child_timeout").await;
            None
        }
        Either::Right((Either::Right(((), _pending)), _wait)) => None,
    };
    let status = if let Some(exit_status) = first_status {
        Some(exit_status)
    } else {
        let _term = signal_group(state.pid, GroupSignal::Term);
        sleep(Duration::from_millis(100)).await;
        let _kill = signal_group(state.pid, GroupSignal::Kill);
        timeout(Duration::from_secs(2), child.wait()).await.ok()
    };
    match status {
        Some(Ok(exit)) => {
            if !exit.success() && !state.stopping.load(Ordering::Acquire) {
                state.fail("child_exited").await;
            }
            state.closed.store(true, Ordering::Release);
        }
        Some(Err(_)) | None => state.fail("cleanup_failed").await,
    }
}

async fn collect<R: AsyncRead + Unpin>(mut reader: R, state: Arc<ProcessState>, stdout: bool) {
    let mut chunk = [0_u8; 4096];
    loop {
        let count = match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(count) => count,
            Err(_) => {
                state.fail("child_exited").await;
                break;
            }
        };
        let Some(bytes) = chunk.get(..count) else {
            state.fail("child_output_limit").await;
            break;
        };
        let mut output = state.output.lock().await;
        let Some(next) = output.bytes.checked_add(count) else {
            drop(output);
            state.fail("child_output_limit").await;
            break;
        };
        output.bytes = next;
        output.digest.update(bytes);
        if next > state.cap {
            drop(output);
            state.fail("child_output_limit").await;
            break;
        }
        if stdout {
            output.stdout.extend(bytes);
        }
    }
    state.readers_done.fetch_add(1, Ordering::AcqRel);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GroupSignal {
    Interrupt,
    Term,
    Kill,
}

#[cfg(unix)]
fn group_pid(pid: u32) -> Result<Pid> {
    i32::try_from(pid)
        .ok()
        .filter(|value| *value > 1_i32)
        .and_then(Pid::from_raw)
        .ok_or(HarnessError::new("cleanup_failed"))
}

#[cfg(unix)]
fn group_alive(pid: u32) -> Result<bool> {
    match test_kill_process_group(group_pid(pid)?) {
        Ok(()) | Err(Errno::PERM) => Ok(true),
        Err(Errno::SRCH) => Ok(false),
        Err(_) => Err(HarnessError::new("cleanup_failed")),
    }
}

#[cfg(unix)]
fn signal_group(pid: u32, signal: GroupSignal) -> Result<()> {
    let native_signal = match signal {
        GroupSignal::Interrupt => Signal::INT,
        GroupSignal::Term => Signal::TERM,
        GroupSignal::Kill => Signal::KILL,
    };
    match kill_process_group(group_pid(pid)?, native_signal) {
        Ok(()) | Err(Errno::SRCH) => Ok(()),
        Err(Errno::PERM) if !group_alive(pid)? => Ok(()),
        Err(_) => Err(HarnessError::new("cleanup_failed")),
    }
}

#[cfg(not(unix))]
fn group_alive(_pid: u32) -> Result<bool> {
    Err(HarnessError::new("platform_unsupported"))
}
#[cfg(not(unix))]
fn signal_group(_pid: u32, _signal: GroupSignal) -> Result<()> {
    Err(HarnessError::new("platform_unsupported"))
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use futures::future::pending;
    use std::future::poll_fn;
    use std::task::Poll;
    use tokio::sync::oneshot;
    use tokio::task::spawn_blocking;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_unacknowledged_abort_keeps_the_task_owned_and_returns_within_cleanup_deadlines() {
        use std::sync::{Condvar, Mutex as BlockingMutex};
        let supervisor = Supervisor::new();
        let process = supervisor.spawn(shell("exit 0")).await.unwrap();
        timeout(Duration::from_secs(2), async {
            while !process.closed() {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let released = Arc::new((BlockingMutex::new(false), Condvar::new()));
        let holding = Arc::clone(&released);
        let (started, observed) = oneshot::channel();
        let blocked = spawn_blocking(move || {
            started.send(()).unwrap();
            let (lock, changed) = &*holding;
            let mut done = lock.lock().unwrap();
            while !*done {
                done = changed.wait(done).unwrap();
            }
        });
        observed.await.unwrap();
        process.inner.tasks.lock().await.push(blocked);
        let result = timeout(Duration::from_secs(8), supervisor.stop(&process)).await;
        let retained = process.inner.tasks.lock().await.len();
        let (lock, changed) = &*released;
        *lock.lock().unwrap() = true;
        changed.notify_all();
        let cleanup = supervisor.cleanup().await;
        assert_eq!(result.unwrap(), Err(HarnessError::new("cleanup_failed")));
        assert_ne!(retained, 0);
        assert_eq!(cleanup, Err(HarnessError::new("cleanup_failed")));
    }

    fn shell(source: &str) -> ProcessSpec {
        let mut spec = ProcessSpec::new("/bin/sh");
        spec.args = vec!["-c".into(), source.into()];
        spec.timeout_ms = 2000;
        spec.output_cap = 8192;
        spec
    }

    #[tokio::test]
    async fn supervised_commands_capture_stdout_but_never_surface_stderr() {
        let supervisor = Supervisor::new();
        let output = supervisor
            .run(shell("printf ready; printf SECRET >&2"))
            .await
            .unwrap();
        assert_eq!(output.stdout, b"ready");
        assert_eq!(output.summary.output_bytes, 11);
        assert!(output.summary.closed);
        assert_eq!(output.summary.output_sha256.len(), 64);
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn command_failures_keep_only_the_fixed_failure_category() {
        let supervisor = Supervisor::new();
        assert_eq!(
            supervisor
                .run(shell("printf SECRET >&2; exit 7"))
                .await
                .unwrap_err(),
            HarnessError::new("child_exited")
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn output_overflow_and_timeout_reap_the_group() {
        for (source, cap, timeout, expected) in [
            (
                "while :; do printf SECRET; done",
                64_usize,
                2000_u64,
                "child_output_limit",
            ),
            (
                "trap '' TERM INT; while :; do /bin/sleep 1; done",
                1024,
                100,
                "child_timeout",
            ),
        ] {
            let supervisor = Supervisor::new();
            let mut spec = shell(source);
            spec.output_cap = cap;
            spec.timeout_ms = timeout;
            let process = supervisor.spawn(spec).await.unwrap();
            let pid = process.pid();
            assert_eq!(
                process.result().await.unwrap_err(),
                HarnessError::new(expected)
            );
            supervisor.cleanup().await.unwrap();
            assert!(!group_alive(pid).unwrap());
        }
    }

    #[tokio::test]
    async fn successful_wrapper_exit_does_not_abandon_descendants_or_their_pipes() {
        let supervisor = Supervisor::new();
        let process = supervisor
            .spawn(shell(
                "(trap '' TERM INT; while :; do /bin/sleep 1; done) & printf 'ready\\n'; exit 0",
            ))
            .await
            .unwrap();
        let pid = process.pid();
        assert_eq!(process.read_line().await.unwrap(), b"ready");
        process.result().await.unwrap();
        assert!(!group_alive(pid).unwrap());
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn private_json_lines_are_bounded_and_round_trip_without_a_control_route() {
        let supervisor = Supervisor::new();
        let mut spec = shell("while IFS= read -r line; do printf '%s\\n' \"$line\"; done");
        spec.stdin = true;
        let process = supervisor.spawn(spec).await.unwrap();
        let value = serde_json::json!({"id":1_u32,"safe":true});
        process.write_json(&value).await.unwrap();
        assert_eq!(
            process.read_json_line::<serde_json::Value>().await.unwrap(),
            value
        );
        assert_eq!(
            process.write_json(&"x".repeat(4096)).await.unwrap_err(),
            HarnessError::new("output_limit")
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn deadline_and_signal_latches_prevent_every_later_spawn() {
        for code in [1_i32, 130_i32, 143_i32] {
            let supervisor = Supervisor::new();
            if code == 1_i32 {
                supervisor.set_deadline(1).await.unwrap();
                sleep(Duration::from_millis(5)).await;
            } else {
                supervisor.interrupt(code);
            }
            let expected = if code == 1_i32 {
                "run_deadline"
            } else {
                "interrupted"
            };
            assert_eq!(supervisor.cancelled().await, HarnessError::new(expected));
            for _ in 0_u8..2 {
                assert_eq!(
                    supervisor.spawn(shell("exit 0")).await.unwrap_err(),
                    HarnessError::new(expected)
                );
            }
            assert_eq!(supervisor.exit_code(), code);
            supervisor.cleanup().await.unwrap();
        }
    }

    #[tokio::test]
    async fn cancellation_during_a_command_refuses_and_reaps_before_returning() {
        for code in [1_i32, 130_i32, 143_i32] {
            let supervisor = Supervisor::new();
            let runner = supervisor.clone();
            let run =
                tokio::spawn(async move { runner.run(shell("printf ready; /bin/sleep 1")).await });
            loop {
                if !supervisor.inner.processes.lock().await.is_empty() {
                    break;
                }
                sleep(Duration::from_millis(1)).await;
            }
            let pid = supervisor
                .inner
                .processes
                .lock()
                .await
                .first()
                .unwrap()
                .pid();
            if code == 1_i32 {
                supervisor.set_deadline(1).await.unwrap();
            } else {
                supervisor.interrupt(code);
            }
            assert_eq!(
                run.await.unwrap().unwrap_err(),
                HarnessError::new(if code == 1_i32 {
                    "run_deadline"
                } else {
                    "interrupted"
                })
            );
            assert!(!group_alive(pid).unwrap());
            assert_eq!(supervisor.exit_code(), code);
            supervisor.cleanup().await.unwrap();
        }
    }

    #[tokio::test]
    async fn cleanup_failure_remains_a_failure_on_every_retry() {
        let supervisor = Supervisor::new();
        supervisor
            .inner
            .cleanup_failed
            .store(true, Ordering::Release);
        for _ in 0_u8..2 {
            assert_eq!(
                supervisor.cleanup().await.unwrap_err(),
                HarnessError::new("cleanup_failed")
            );
        }
    }

    #[tokio::test]
    async fn failed_cleanup_overrides_an_earlier_command_failure_without_hanging() {
        let supervisor = Supervisor::new();
        let process = supervisor.spawn(shell("/bin/sleep 1")).await.unwrap();
        process.inner.fail("child_timeout").await;
        process.inner.fail("cleanup_failed").await;
        assert_eq!(
            *process.inner.error.lock().await,
            Some(HarnessError::new("cleanup_failed"))
        );
        assert_eq!(
            timeout(Duration::from_secs(3), process.result())
                .await
                .unwrap()
                .unwrap_err(),
            HarnessError::new("cleanup_failed")
        );
        assert!(!group_alive(process.pid()).unwrap());
    }

    #[tokio::test]
    async fn queued_spawn_cannot_cross_a_concurrent_cleanup_fence() {
        let supervisor = Supervisor::new();
        let registry = supervisor.inner.processes.lock().await;
        let launch = supervisor.spawn(ProcessSpec::new("/nonexistent/SECRET_PROGRAM"));
        tokio::pin!(launch);
        assert!(matches!(
            poll_fn(|cx| Poll::Ready(launch.as_mut().poll(cx))).await,
            Poll::Pending
        ));
        let cleanup = supervisor.cleanup();
        tokio::pin!(cleanup);
        assert!(matches!(
            poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx))).await,
            Poll::Pending
        ));
        drop(registry);
        assert_eq!(
            launch.await.unwrap_err(),
            HarnessError::new("cleanup_failed")
        );
        cleanup.await.unwrap();
        assert!(supervisor.inner.processes.lock().await.is_empty());
    }

    #[tokio::test]
    async fn cancelling_a_pending_cleanup_keeps_its_join_handle_owned() {
        let supervisor = Supervisor::new();
        let process = supervisor.spawn(shell("exit 0")).await.unwrap();
        while !process.closed() {
            sleep(Duration::from_millis(1)).await;
        }
        let (send, receive) = oneshot::channel::<()>();
        process.inner.tasks.lock().await.push(tokio::spawn(async {
            let _released = receive.await;
        }));
        let before = process.inner.tasks.lock().await.len();
        let was_pending = {
            let cleanup = process.stop();
            tokio::pin!(cleanup);
            matches!(
                poll_fn(|cx| Poll::Ready(cleanup.as_mut().poll(cx))).await,
                Poll::Pending
            )
        };
        assert!(was_pending);
        assert_eq!(process.inner.tasks.lock().await.len(), before);
        send.send(()).unwrap();
        supervisor.cleanup().await.unwrap();
        assert!(process.inner.tasks.lock().await.is_empty());
        assert!(!group_alive(process.pid()).unwrap());
    }

    #[tokio::test]
    async fn supervisor_owns_abandoned_async_work_until_its_abort_and_join_fence() {
        struct Witness(Arc<AtomicBool>);
        #[expect(
            clippy::missing_trait_methods,
            reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
        )]
        impl Drop for Witness {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let supervisor = Supervisor::new();
        let dropped = Arc::new(AtomicBool::new(false));
        let witness = Witness(Arc::clone(&dropped));
        let (started, running) = oneshot::channel();
        let task = supervisor
            .task(async move {
                let _owned = witness;
                started.send(()).unwrap();
                pending::<()>().await;
            })
            .await
            .unwrap();
        running.await.unwrap();
        drop(task);
        assert!(!dropped.load(Ordering::Acquire));
        supervisor.cleanup().await.unwrap();
        assert!(dropped.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn async_task_slots_are_reused_only_after_join_and_close_before_cleanup() {
        let supervisor = Supervisor::new();
        let mut checked = 0_u32;
        for _round in 0_i32..80_i32 {
            let (send, receive) = oneshot::channel();
            let task = supervisor
                .task(async move {
                    send.send(()).unwrap();
                })
                .await
                .unwrap();
            receive.await.unwrap();
            supervisor.join_task(&task).await.unwrap();
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 80);
        supervisor.cleanup().await.unwrap();
        assert!(matches!(
            supervisor.task(async {}).await,
            Err(HarnessError {
                code: "cleanup_failed",
                ..
            })
        ));
    }
}

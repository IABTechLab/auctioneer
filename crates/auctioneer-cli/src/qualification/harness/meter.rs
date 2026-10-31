use super::process::{ManagedProcess, ProcessSpec, Supervisor};
use super::{HarnessError, Result, io};
use crate::qualification::profile::Meter;
use crate::qualification::report::{Memory, MeterLimitation, PeakMethod, Units};
use futures::future::{Either, select};
#[cfg(unix)]
use rustix::{
    io::Errno,
    process::{Pid, test_kill_process},
};
use std::collections::BTreeSet;
use std::env::consts::OS;
use std::fs::{self, File, Permissions};
use std::future::Future;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str::from_utf8;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::{Instant, sleep};

const REPORT_CAP: usize = 8192;
const PROCESS_TABLE_CAP: usize = 256 << 10;
const STATUS_CAP: usize = 64 << 10;
const SAMPLE_CAP: u16 = 128;

struct MeterReport {
    file: File,
    path: PathBuf,
}

impl MeterReport {
    fn create(cwd: &Path) -> Result<Self> {
        let report = tempfile::Builder::new()
            .prefix("meter-")
            .suffix(".txt")
            .tempfile_in(cwd)
            .map_err(|_error| HarnessError::new("meter_failed"))?;
        #[cfg(unix)]
        let () = {
            use std::os::unix::fs::PermissionsExt as _;
            report
                .as_file()
                .set_permissions(Permissions::from_mode(0o600))
                .map_err(|_error| HarnessError::new("meter_failed"))?;
        };
        // Persist before the first spawn await: dropping startup must not unlink
        // a report that a still-supervised time(1) process has not written yet.
        let (file, path) = report
            .keep()
            .map_err(|_error| HarnessError::new("meter_failed"))?;
        Ok(Self { file, path })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn high_water(&self) -> Result<u64> {
        let metadata = self
            .file
            .metadata()
            .map_err(|_error| HarnessError::new("meter_failed"))?;
        if !metadata.is_file() || metadata.len() > 8192 {
            return Err(HarnessError::new("meter_failed"));
        }
        let bytes = io::read_bytes(self.path(), REPORT_CAP)
            .map_err(|_error| HarnessError::new("meter_failed"))?;
        let output = from_utf8(&bytes).map_err(|_error| HarnessError::new("meter_failed"))?;
        parse_high_water(OS, output)
    }

    fn discard(self) -> Result<()> {
        match fs::remove_file(self.path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_error) => Err(HarnessError::new("cleanup_failed")),
        }
    }
}

pub struct NativeMeter {
    supervisor: Supervisor,
    process: ManagedProcess,
    report: Option<MeterReport>,
    cwd: PathBuf,
    service_pid: u32,
    high_water: Option<u64>,
    stop_result: Option<Result<()>>,
}

#[derive(Debug)]
pub struct Window {
    pub samples: u16,
    pub first: u64,
    pub last: u64,
    pub sampled_peak_bytes: u64,
    pub elapsed_ms: u64,
}

impl NativeMeter {
    pub async fn start(service: ProcessSpec, supervisor: &Supervisor) -> Result<Self> {
        supported_platform(OS)?;
        let directory = service
            .cwd
            .as_ref()
            .ok_or(HarnessError::new("invalid_input"))?;
        let cwd =
            fs::canonicalize(directory).map_err(|_error| HarnessError::new("invalid_input"))?;
        if service.timeout_ms == 0 {
            return Err(HarnessError::new("invalid_input"));
        }
        supervisor.check_active().await?;
        let report = MeterReport::create(&cwd)?;
        let mut wrapper = ProcessSpec::new("/usr/bin/time");
        wrapper.args = vec![
            if OS == "macos" {
                "-l".into()
            } else {
                "-v".into()
            },
            "-o".into(),
            report.path().as_os_str().to_owned(),
            service.program.into_os_string(),
        ];
        wrapper.args.extend(service.args);
        wrapper.cwd = Some(cwd.clone());
        wrapper.env = service.env;
        wrapper.env.insert("LC_ALL".into(), "C".into());
        wrapper.timeout_ms = service.timeout_ms.min(599_999);
        wrapper.output_cap = service.output_cap;
        wrapper.stdin = service.stdin;
        let process = match supervisor.spawn(wrapper).await {
            Ok(process) => process,
            Err(error) => {
                if error.code != "cleanup_failed" {
                    report.discard()?;
                }
                return Err(error);
            }
        };
        let mut meter = Self {
            supervisor: supervisor.clone(),
            process,
            report: Some(report),
            cwd,
            service_pid: 0,
            high_water: None,
            stop_result: None,
        };
        if let Err(error) = meter.discover_service().await {
            // No measurement is valid on startup failure. Keep the report if
            // the registered wrapper cannot be proven stopped.
            supervisor
                .stop(&meter.process)
                .await
                .map_err(|_error| HarnessError::new("cleanup_failed"))?;
            meter.discard_report(true)?;
            return Err(error);
        }
        Ok(meter)
    }

    pub fn pid(&self) -> u32 {
        self.service_pid
    }

    pub fn closed(&self) -> bool {
        self.process.closed()
    }

    pub async fn assert_running(&self) -> Result<()> {
        self.supervisor.check_active().await?;
        self.process.assert_running().await?;
        if !service_alive(self.pid())? {
            return Err(HarnessError::new("child_exited"));
        }
        Ok(())
    }

    pub async fn window<F, T>(&self, duration_ms: u64, work: F) -> Result<(Window, T)>
    where
        F: Future<Output = Result<T>>,
    {
        if !(1..=60_000).contains(&duration_ms) {
            return Err(HarnessError::new("invalid_input"));
        }
        self.supervisor.check_active().await?;
        let finished = AtomicBool::new(false);
        let sampling = Box::pin(self.sample_window(duration_ms, &finished));
        match select(sampling, Box::pin(work)).await {
            Either::Left((sampled, _work)) => {
                // A successful window requires work completion, signalled below.
                sampled.and(Err(HarnessError::new("meter_failed")))
            }
            Either::Right((outcome, remaining_samples)) => {
                finished.store(true, Ordering::Release);
                let sampled = remaining_samples.await;
                if matches!(
                    outcome,
                    Err(HarnessError {
                        code: "cleanup_failed",
                        ..
                    })
                ) {
                    return Err(HarnessError::new("cleanup_failed"));
                }
                let window = sampled?;
                self.assert_running().await?;
                Ok((window, outcome?))
            }
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(result) = self.stop_result {
            return result;
        }
        // A cancelled stop cannot turn a later retry into a success claim.
        self.stop_result = Some(Err(HarnessError::new("cleanup_failed")));
        let termination = self.terminate_service().await;
        let natural_end = Instant::now()
            .checked_add(Duration::from_secs(1))
            .ok_or(HarnessError::new("cleanup_failed"))?;
        while !self.closed() && Instant::now() < natural_end {
            sleep(Duration::from_millis(10)).await;
        }
        // Do not call result(): it stops the group. time(1) must first have
        // reaped the service and written its wait4 result on its own.
        let closed_naturally = self.closed();
        let wrapper_cleanup = self.supervisor.stop(&self.process).await;
        let completion = if wrapper_cleanup.is_ok() {
            // The group is already stopped. This only inspects its final status;
            // a supervisor timeout/output kill is not a natural wrapper close.
            self.process.result().await.map(|_output| ())
        } else {
            Err(HarnessError::new("cleanup_failed"))
        };
        let natural_status = matches!(
            completion,
            Ok(())
                | Err(HarnessError {
                    code: "child_exited",
                    ..
                })
        );
        let service_gone = matches!(service_alive(self.pid()), Ok(false));
        let measured = self
            .report
            .as_ref()
            .ok_or(HarnessError::new("meter_failed"))
            .and_then(MeterReport::high_water);
        let discarded = self.discard_report(wrapper_cleanup.is_ok() && service_gone);
        let result = if termination.is_err()
            || wrapper_cleanup.is_err()
            || !closed_naturally
            || !natural_status
            || !service_gone
            || discarded.is_err()
        {
            Err(HarnessError::new("cleanup_failed"))
        } else {
            measured.map(|bytes| {
                self.high_water = Some(bytes);
            })
        };
        self.stop_result = Some(result);
        result
    }

    pub fn memory(&self, window: &Window) -> Result<Memory> {
        if let Some(Err(error)) = self.stop_result {
            return Err(error);
        }
        if self.stop_result != Some(Ok(())) || !(1..=SAMPLE_CAP).contains(&window.samples) {
            return Err(HarnessError::new("meter_failed"));
        }
        let peak_bytes = self
            .high_water
            .filter(|bytes| *bytes > 0)
            .ok_or(HarnessError::new("meter_failed"))?;
        Ok(Memory {
            limitation: MeterLimitation::ProcessLifetimeHighWater,
            source: Meter::ProcessRss,
            method: PeakMethod::HighWater,
            units: Units::Bytes,
            window_ms: window.elapsed_ms,
            peak_bytes,
            retained_growth_bytes: window.last.saturating_sub(window.first),
            service_only: true,
        })
    }

    async fn discover_service(&mut self) -> Result<()> {
        let end = Instant::now()
            .checked_add(Duration::from_secs(2))
            .ok_or(HarnessError::new("meter_failed"))?;
        loop {
            self.supervisor.check_active().await?;
            self.process.assert_running().await?;
            let remaining = end.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(HarnessError::new("meter_failed"));
            }
            let mut spec = self.tool("/bin/ps");
            spec.args = vec!["-axo".into(), "pid=,ppid=".into()];
            spec.output_cap = PROCESS_TABLE_CAP;
            spec.timeout_ms = u64::try_from(remaining.as_millis())
                .map_err(|_error| HarnessError::new("meter_failed"))?
                .max(1);
            let output = self.supervisor.run(spec).await?;
            if Instant::now() >= end {
                return Err(HarnessError::new("meter_failed"));
            }
            let table =
                from_utf8(&output.stdout).map_err(|_error| HarnessError::new("meter_failed"))?;
            if let Some(pid) = parse_children(table, self.process.pid())? {
                self.service_pid = pid;
                return self.assert_running().await;
            }
            match select(
                Box::pin(self.supervisor.cancelled()),
                Box::pin(sleep(Duration::from_millis(10))),
            )
            .await
            {
                Either::Left((error, _sleep)) => return Err(error),
                Either::Right(((), _cancelled)) => {}
            }
        }
    }

    fn tool(&self, program: &str) -> ProcessSpec {
        let mut spec = ProcessSpec::new(program);
        spec.cwd = Some(self.cwd.clone());
        spec.env.insert("LC_ALL".into(), "C".into());
        spec.timeout_ms = 2000;
        spec.output_cap = 1024;
        spec
    }

    async fn rss(&self) -> Result<u64> {
        self.assert_running().await?;
        let bytes = match OS {
            "linux" => io::read_bytes(
                Path::new(&format!("/proc/{}/status", self.pid())),
                STATUS_CAP,
            )
            .map_err(|_error| HarnessError::new("meter_failed"))?,
            "macos" => {
                let mut spec = self.tool("/bin/ps");
                spec.args = vec![
                    "-o".into(),
                    "rss=".into(),
                    "-p".into(),
                    self.pid().to_string().into(),
                ];
                self.supervisor.run(spec).await?.stdout
            }
            _ => return Err(HarnessError::new("meter_unsupported")),
        };
        let output = from_utf8(&bytes).map_err(|_error| HarnessError::new("meter_failed"))?;
        let rss = parse_rss(OS, output)?;
        self.assert_running().await?;
        Ok(rss)
    }

    async fn sample_window(&self, duration_ms: u64, finished: &AtomicBool) -> Result<Window> {
        sample_window(&self.supervisor, duration_ms, finished, || self.rss()).await
    }

    async fn terminate_service(&self) -> Result<()> {
        let mut failed = false;
        for signal in [
            ServiceSignal::Interrupt,
            ServiceSignal::Term,
            ServiceSignal::Kill,
        ] {
            if matches!(service_alive(self.pid()), Ok(false)) && self.closed() {
                break;
            }
            if signal_service(self.pid(), signal).is_err() {
                failed = true;
            }
            let end = Instant::now()
                .checked_add(Duration::from_millis(500))
                .ok_or(HarnessError::new("cleanup_failed"))?;
            loop {
                match service_alive(self.pid()) {
                    Ok(false) if self.closed() => break,
                    Ok(_) => {}
                    Err(_error) => failed = true,
                }
                if Instant::now() >= end {
                    break;
                }
                sleep(Duration::from_millis(10)).await;
            }
        }
        if failed {
            return Err(HarnessError::new("cleanup_failed"));
        }
        Ok(())
    }

    fn discard_report(&mut self, quiescent: bool) -> Result<()> {
        if !quiescent {
            return Err(HarnessError::new("cleanup_failed"));
        }
        self.report.take().map_or(Ok(()), MeterReport::discard)
    }
}

async fn sample_window<F, R>(
    supervisor: &Supervisor,
    duration_ms: u64,
    finished: &AtomicBool,
    mut sample: F,
) -> Result<Window>
where
    F: FnMut() -> R,
    R: Future<Output = Result<u64>>,
{
    let started = Instant::now();
    let cadence = Duration::from_millis(duration_ms.div_ceil(100).max(50));
    let mut window = Window {
        samples: 0,
        first: 0,
        last: 0,
        sampled_peak_bytes: 0,
        elapsed_ms: 0,
    };
    loop {
        supervisor.check_active().await?;
        if window.samples >= SAMPLE_CAP {
            return Err(HarnessError::new("sample_limit"));
        }
        // Both window boundaries must precede the final RSS read.
        let final_read = started.elapsed() >= Duration::from_millis(duration_ms)
            && finished.load(Ordering::Acquire);
        let rss = sample().await?;
        supervisor.check_active().await?;
        window.samples = window
            .samples
            .checked_add(1)
            .ok_or(HarnessError::new("sample_limit"))?;
        if window.samples == 1 {
            window.first = rss;
        }
        window.last = rss;
        window.sampled_peak_bytes = window.sampled_peak_bytes.max(rss);
        window.elapsed_ms = u64::try_from(started.elapsed().as_millis())
            .map_err(|_error| HarnessError::new("meter_failed"))?;
        if final_read {
            return Ok(window);
        }
        match select(Box::pin(supervisor.cancelled()), Box::pin(sleep(cadence))).await {
            Either::Left((error, _sleep)) => return Err(error),
            Either::Right(((), _cancelled)) => {}
        }
        supervisor.check_active().await?;
    }
}

fn supported_platform(platform: &str) -> Result<()> {
    match platform {
        "linux" | "macos" => Ok(()),
        _ => Err(HarnessError::new("meter_unsupported")),
    }
}

fn decimal(value: &str) -> Result<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(HarnessError::new("meter_failed"));
    }
    value
        .parse()
        .map_err(|_error| HarnessError::new("meter_failed"))
}

fn nonzero_scaled(value: &str, scale: u64) -> Result<u64> {
    decimal(value)?
        .checked_mul(scale)
        .filter(|bytes| *bytes > 0)
        .ok_or(HarnessError::new("meter_failed"))
}

fn parse_high_water(platform: &str, output: &str) -> Result<u64> {
    supported_platform(platform)?;
    if output.len() > REPORT_CAP {
        return Err(HarnessError::new("meter_failed"));
    }
    let mut reading = None;
    for line in output.lines() {
        if !line.contains("maximum resident set size")
            && !line.contains("Maximum resident set size")
        {
            continue;
        }
        if reading.is_some() {
            return Err(HarnessError::new("meter_failed"));
        }
        reading = Some(if platform == "macos" {
            let mut fields = line.split_whitespace();
            let value = fields.next().ok_or(HarnessError::new("meter_failed"))?;
            if !fields.eq(["maximum", "resident", "set", "size"]) {
                return Err(HarnessError::new("meter_failed"));
            }
            nonzero_scaled(value, 1)?
        } else {
            let value = line
                .trim()
                .strip_prefix("Maximum resident set size (kbytes):")
                .ok_or(HarnessError::new("meter_failed"))?;
            nonzero_scaled(value.trim(), 1024)?
        });
    }
    reading.ok_or(HarnessError::new("meter_failed"))
}

fn parse_children(table: &str, wrapper: u32) -> Result<Option<u32>> {
    if table.len() > PROCESS_TABLE_CAP || !(2..=0x7FFF_FFFF).contains(&wrapper) {
        return Err(HarnessError::new("meter_failed"));
    }
    let mut seen = BTreeSet::new();
    let mut child = None;
    for line in table.lines() {
        let mut fields = line.split_whitespace();
        let pid = decimal(fields.next().ok_or(HarnessError::new("meter_failed"))?)?;
        let parent = decimal(fields.next().ok_or(HarnessError::new("meter_failed"))?)?;
        if fields.next().is_some()
            || !(1..=0x7FFF_FFFF).contains(&pid)
            || parent > 0x7FFF_FFFF
            || !seen.insert(pid)
        {
            return Err(HarnessError::new("meter_failed"));
        }
        if parent == u64::from(wrapper) {
            if child.is_some() || pid <= 1 || pid == u64::from(wrapper) {
                return Err(HarnessError::new("meter_failed"));
            }
            child = Some(u32::try_from(pid).map_err(|_error| HarnessError::new("meter_failed"))?);
        }
    }
    Ok(child)
}

fn parse_rss(platform: &str, output: &str) -> Result<u64> {
    supported_platform(platform)?;
    if output.len() > STATUS_CAP {
        return Err(HarnessError::new("meter_failed"));
    }
    if platform == "macos" {
        return nonzero_scaled(output.trim(), 1024);
    }
    let mut reading = None;
    for line in output.lines() {
        if !line.trim_start().starts_with("VmRSS") {
            continue;
        }
        if reading.is_some() {
            return Err(HarnessError::new("meter_failed"));
        }
        let payload = line
            .trim()
            .strip_prefix("VmRSS:")
            .ok_or(HarnessError::new("meter_failed"))?;
        let mut fields = payload.split_whitespace();
        let value = fields.next().ok_or(HarnessError::new("meter_failed"))?;
        if fields.next() != Some("kB") || fields.next().is_some() {
            return Err(HarnessError::new("meter_failed"));
        }
        reading = Some(nonzero_scaled(value, 1024)?);
    }
    reading.ok_or(HarnessError::new("meter_failed"))
}

#[derive(Clone, Copy)]
enum ServiceSignal {
    Interrupt,
    Term,
    Kill,
}

#[cfg(unix)]
fn positive_pid(pid: u32) -> Result<Pid> {
    i32::try_from(pid)
        .ok()
        .filter(|raw| *raw > 1_i32)
        .and_then(Pid::from_raw)
        .ok_or(HarnessError::new("cleanup_failed"))
}

#[cfg(unix)]
fn service_alive(pid: u32) -> Result<bool> {
    match test_kill_process(positive_pid(pid)?) {
        Ok(()) => Ok(true),
        Err(Errno::SRCH) => Ok(false),
        Err(_error) => Err(HarnessError::new("cleanup_failed")),
    }
}

#[cfg(unix)]
fn signal_service(pid: u32, signal: ServiceSignal) -> Result<()> {
    use rustix::process::{Signal, kill_process};
    let code = match signal {
        ServiceSignal::Interrupt => Signal::INT,
        ServiceSignal::Term => Signal::TERM,
        ServiceSignal::Kill => Signal::KILL,
    };
    match kill_process(positive_pid(pid)?, code) {
        Ok(()) | Err(Errno::SRCH) => Ok(()),
        Err(_error) => Err(HarnessError::new("cleanup_failed")),
    }
}

#[cfg(not(unix))]
fn service_alive(_pid: u32) -> Result<bool> {
    Err(HarnessError::new("meter_unsupported"))
}

#[cfg(not(unix))]
fn signal_service(_pid: u32, _signal: ServiceSignal) -> Result<()> {
    Err(HarnessError::new("meter_unsupported"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::future::ready;

    #[tokio::test(start_paused = true)]
    async fn sample_window_observes_growth_after_work_completion() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let baseline = 0x0080_0000;
        let growth = 0x0200_0000;
        let rss = Cell::new(baseline);
        let work = async {
            sleep(Duration::from_millis(75)).await;
            rss.set(baseline + growth);
            finished.store(true, Ordering::Release);
        };
        let (result, ()) = tokio::join!(
            sample_window(&supervisor, 100, &finished, || ready(Ok(rss.get()))),
            work
        );
        let window = result.unwrap();
        assert_eq!(window.first, baseline);
        assert_eq!(window.last.saturating_sub(window.first), growth);
        assert_eq!(window.sampled_peak_bytes, baseline + growth);
        assert_eq!(window.samples, 3);
        assert_eq!(window.elapsed_ms, 100);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_observes_growth_before_work_completion() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let baseline = 0x0080_0000;
        let growth = 0x0200_0000;
        let rss = Cell::new(baseline);
        let work = async {
            sleep(Duration::from_millis(25)).await;
            rss.set(baseline + growth);
            sleep(Duration::from_millis(50)).await;
            finished.store(true, Ordering::Release);
        };
        let (result, ()) = tokio::join!(
            sample_window(&supervisor, 100, &finished, || ready(Ok(rss.get()))),
            work
        );
        let window = result.unwrap();
        assert_eq!(window.first, baseline);
        assert_eq!(window.last.saturating_sub(window.first), growth);
        assert_eq!(window.sampled_peak_bytes, baseline + growth);
        assert_eq!(window.elapsed_ms, 100);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_reads_after_minimum_duration_when_work_already_finished() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(true);
        let reads = Cell::new(0_u16);
        let result = sample_window(&supervisor, 1, &finished, || {
            reads.set(reads.get().checked_add(1).unwrap());
            ready(Ok(1024))
        })
        .await;
        let window = result.unwrap();
        assert_eq!(reads.get(), 2);
        assert_eq!(window.samples, 2);
        assert_eq!(window.elapsed_ms, 50);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_observes_growth_at_minimum_duration_boundary() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let baseline = 0x0080_0000;
        let growth = 0x0200_0000;
        let rss = Cell::new(baseline);
        let changes = async {
            sleep(Duration::from_millis(25)).await;
            finished.store(true, Ordering::Release);
            sleep(Duration::from_millis(50)).await;
            rss.set(baseline + growth);
        };
        let (result, ()) = tokio::join!(
            sample_window(&supervisor, 100, &finished, || ready(Ok(rss.get()))),
            changes
        );
        let window = result.unwrap();
        assert_eq!(window.first, baseline);
        assert_eq!(window.last.saturating_sub(window.first), growth);
        assert_eq!(window.sampled_peak_bytes, baseline + growth);
        assert_eq!(window.samples, 3);
        assert_eq!(window.elapsed_ms, 100);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_counts_final_read_against_sample_cap() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let reads = Cell::new(0_u16);
        let result = sample_window(&supervisor, 1, &finished, || {
            let count = reads.get().checked_add(1).unwrap();
            reads.set(count);
            if count == 128 {
                finished.store(true, Ordering::Release);
            }
            ready(Ok(1024))
        })
        .await;
        assert_eq!(result.unwrap_err(), HarnessError::new("sample_limit"));
        assert_eq!(reads.get(), 128);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_accepts_final_read_at_sample_cap() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let reads = Cell::new(0_u16);
        let result = sample_window(&supervisor, 1, &finished, || {
            let count = reads.get().checked_add(1).unwrap();
            reads.set(count);
            if count == 127 {
                finished.store(true, Ordering::Release);
            }
            ready(Ok(u64::from(count)))
        })
        .await;
        let window = result.unwrap();
        assert_eq!(reads.get(), 128);
        assert_eq!(window.samples, 128);
        assert_eq!(window.last, 128);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_rejects_failed_final_read() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let reads = Cell::new(0_u16);
        let result = sample_window(&supervisor, 1, &finished, || {
            let count = reads.get().checked_add(1).unwrap();
            reads.set(count);
            finished.store(true, Ordering::Release);
            ready(if count == 1 {
                Ok(1024)
            } else {
                Err(HarnessError::new("meter_failed"))
            })
        })
        .await;
        assert_eq!(result.unwrap_err(), HarnessError::new("meter_failed"));
        assert_eq!(reads.get(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_rejects_cancelled_final_read() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let reads = Cell::new(0_u16);
        let result = sample_window(&supervisor, 1, &finished, || {
            let count = reads.get().checked_add(1).unwrap();
            reads.set(count);
            finished.store(true, Ordering::Release);
            if count == 2 {
                supervisor.interrupt(130);
            }
            ready(Ok(1024))
        })
        .await;
        assert_eq!(result.unwrap_err(), HarnessError::new("interrupted"));
        assert_eq!(reads.get(), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn sample_window_includes_final_read_time_in_elapsed() {
        let supervisor = Supervisor::new();
        let finished = AtomicBool::new(false);
        let reads = Cell::new(0_u16);
        let result = sample_window(&supervisor, 1, &finished, || async {
            let count = reads.get().checked_add(1).unwrap();
            reads.set(count);
            finished.store(true, Ordering::Release);
            if count == 2 {
                sleep(Duration::from_millis(37)).await;
            }
            Ok(1024)
        })
        .await;
        let window = result.unwrap();
        assert_eq!(reads.get(), 2);
        assert_eq!(window.elapsed_ms, 87);
    }

    #[test]
    fn bsd_high_water_is_already_bytes() {
        assert_eq!(
            parse_high_water(
                "macos",
                "        0.01 real         0.00 user         0.00 sys\n\
                 7340032  maximum resident set size\n\
                 0  page reclaims\n"
            ),
            Ok(0x0070_0000)
        );
    }

    #[test]
    fn gnu_high_water_converts_kibibytes_to_literal_bytes() {
        assert_eq!(
            parse_high_water(
                "linux",
                "Command being timed: SECRET_COMMAND\n\
                 \tMaximum resident set size (kbytes): 7168\n\
                 \tExit status: 0\n"
            ),
            Ok(0x0070_0000)
        );
    }

    #[test]
    fn high_water_refuses_missing_zero_duplicate_malformed_and_overflow() {
        let cases = [
            ("macos", "SECRET_PATH"),
            ("macos", "0 maximum resident set size"),
            (
                "macos",
                "1 maximum resident set size\n2 maximum resident set size",
            ),
            (
                "macos",
                "1 maximum resident set size\nSECRET maximum resident set size",
            ),
            ("macos", "SECRET maximum resident set size"),
            ("macos", "+1 maximum resident set size"),
            ("macos", "1 maximum resident set size SECRET"),
            ("macos", "18446744073709551616 maximum resident set size"),
            ("linux", "Maximum resident set size (kbytes): 0"),
            ("linux", "Maximum resident set size (kbytes): -1"),
            ("linux", "Maximum resident set size (kbytes): 1.5"),
            ("linux", "Maximum resident set size (kbytes): 1 SECRET"),
            (
                "linux",
                "Maximum resident set size (kbytes): 18014398509481984",
            ),
            (
                "linux",
                "Maximum resident set size (kbytes): 1\nMaximum resident set size (kbytes): 2",
            ),
            (
                "linux",
                "Maximum resident set size (kbytes): 1\nMaximum resident set size (kbytes): SECRET",
            ),
            ("linux", "7340032 maximum resident set size"),
            ("macos", "Maximum resident set size (kbytes): 7168"),
        ];
        let mut checked = 0_usize;
        for (platform, output) in cases {
            assert_eq!(
                parse_high_water(platform, output),
                Err(HarnessError::new("meter_failed"))
            );
            checked += 1;
        }
        assert_eq!(checked, 17);
    }

    #[test]
    fn unsupported_high_water_and_rss_platforms_refuse() {
        for platform in ["windows", "freebsd", "SECRET_PLATFORM"] {
            assert_eq!(
                parse_high_water(platform, "7340032 maximum resident set size"),
                Err(HarnessError::new("meter_unsupported"))
            );
            assert_eq!(
                parse_rss(platform, "7168"),
                Err(HarnessError::new("meter_unsupported"))
            );
        }
    }

    #[test]
    fn rss_units_are_kibibytes_on_both_supported_platforms() {
        assert_eq!(
            parse_rss(
                "linux",
                "Name:\tsleep\nVmSize:\t99999 kB\nVmRSS:\t7168 kB\n"
            ),
            Ok(0x0070_0000)
        );
        assert_eq!(parse_rss("macos", "  7168\n"), Ok(0x0070_0000));
    }

    #[test]
    fn rss_refuses_zero_duplicates_missing_wrong_units_and_overflow() {
        let cases = [
            ("linux", "VmSize: 7168 kB"),
            ("linux", "VmRSS: 0 kB"),
            ("linux", "VmRSS: +1 kB"),
            ("linux", "VmRSS: 7168 KB"),
            ("linux", "VmRSS: 7168 kB SECRET"),
            ("linux", "VmRSS: 7168 kB\nVmRSS: 1 kB"),
            ("linux", "VmRSS: 7168 kB\nVmRSS: SECRET kB"),
            ("linux", "VmRSS: 18014398509481984 kB"),
            ("macos", "0"),
            ("macos", "+7168"),
            ("macos", "7168\n1"),
            ("macos", "SECRET"),
            ("macos", "18014398509481984"),
        ];
        let mut checked = 0_usize;
        for (platform, output) in cases {
            assert_eq!(
                parse_rss(platform, output),
                Err(HarnessError::new("meter_failed"))
            );
            checked += 1;
        }
        assert_eq!(checked, 13);
    }

    #[test]
    fn child_discovery_selects_the_only_direct_child_not_the_wrapper_or_grandchild() {
        assert_eq!(
            parse_children("1 0\n40 1\n41 40\n42 41\n43 1\n", 40),
            Ok(Some(41))
        );
        assert_eq!(parse_children("1 0\n40 1\n43 1\n", 40), Ok(None));
    }

    #[test]
    fn child_discovery_refuses_multiple_children_or_any_malformed_row() {
        for table in [
            "41 40\n42 40\n",
            "41 40\n41 40\n",
            "41 40\nSECRET\n",
            "41 40\n42 1 EXTRA\n",
            "0 40\n",
            "-41 40\n",
            "+41 40\n",
            "2147483648 40\n",
            "41 -1\n",
        ] {
            assert_eq!(
                parse_children(table, 40),
                Err(HarnessError::new("meter_failed"))
            );
        }
    }
}

#[cfg(test)]
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_tests {
    use super::*;
    use crate::qualification::profile::Meter;
    use crate::qualification::report::{MeterLimitation, PeakMethod, Units};
    use rustix::io::Errno;
    use rustix::process::{Pid, Signal, kill_process, test_kill_process};
    use std::future::pending;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;
    use tokio::time::{Instant, sleep, timeout};

    fn service(cwd: &Path, seconds: &str) -> ProcessSpec {
        let mut spec = ProcessSpec::new("/bin/sleep");
        spec.args.push(seconds.into());
        spec.cwd = Some(cwd.to_owned());
        spec.env.insert("LC_ALL".into(), "C".into());
        spec.timeout_ms = 10_000;
        spec.output_cap = 8192;
        spec
    }

    async fn start(supervisor: &Supervisor, cwd: &Path) -> NativeMeter {
        match NativeMeter::start(service(cwd, "30"), supervisor).await {
            Ok(meter) => meter,
            Err(error) => {
                supervisor.cleanup().await.unwrap();
                panic!("meter start failed: {}", error.code);
            }
        }
    }

    #[tokio::test]
    async fn an_unproven_writer_keeps_its_private_meter_report_until_proven_cleanup() {
        let directory = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, directory.path()).await;
        let report = meter.report.as_ref().unwrap().path().to_owned();
        assert!(report.is_file());
        assert!(service_alive(meter.pid()).unwrap());
        let discarded = meter.discard_report(false);
        let kept_file = report.is_file();
        let kept_owner = meter.report.is_some();
        let stopped = meter.stop().await;
        let cleanup = supervisor.cleanup().await;
        assert_eq!(discarded, Err(HarnessError::new("cleanup_failed")));
        assert!(kept_file);
        assert!(kept_owner);
        assert_eq!(stopped, Ok(()));
        assert_eq!(cleanup, Ok(()));
        assert!(!report.exists());
    }

    fn service_is_gone(raw: u32) -> bool {
        let pid = Pid::from_raw(i32::try_from(raw).unwrap()).unwrap();
        matches!(test_kill_process(pid), Err(Errno::SRCH))
    }

    struct Dropped(Arc<AtomicBool>);

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[tokio::test]
    async fn real_sleep_has_distinct_service_pid_private_report_and_wait4_high_water() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let pid = meter.pid();
        let wrapper = meter.process.pid();
        let report = meter.report.as_ref().unwrap().path().to_owned();
        let mode = fs::metadata(&report).unwrap().permissions().mode() & 0o777;
        let running = meter.assert_running().await;
        let was_closed = meter.closed();
        let before_stop = meter.high_water;
        let stop = meter.stop().await;
        let high_water = meter.high_water;
        let closed = meter.closed();
        let second_stop = meter.stop().await;
        supervisor.cleanup().await.unwrap();
        assert!(pid > 1);
        assert_ne!(pid, wrapper);
        assert_eq!(mode, 0o600);
        assert_eq!(running, Ok(()));
        assert!(!was_closed);
        assert_eq!(before_stop, None);
        assert_eq!(stop, Ok(()));
        assert_eq!(second_stop, Ok(()));
        assert!(closed);
        assert!(service_is_gone(pid));
        assert!(high_water.is_some_and(|bytes| bytes > 0));
        assert!(!report.exists());
    }

    #[tokio::test]
    async fn start_requires_cwd_and_preserves_fixed_safe_spawn_failure() {
        let supervisor = Supervisor::new();
        let no_cwd = NativeMeter::start(ProcessSpec::new("/bin/sleep"), &supervisor).await;
        assert!(matches!(
            no_cwd,
            Err(HarnessError {
                code: "invalid_input",
                ..
            })
        ));
        let root = tempfile::tempdir().unwrap();
        let mut spec = service(root.path(), "30");
        spec.program = "/nonexistent/SECRET_EXECUTABLE".into();
        let failed = NativeMeter::start(spec, &supervisor).await;
        supervisor.cleanup().await.unwrap();
        assert!(failed.is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn idle_window_observes_duration_and_only_bounded_service_rss() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let started = Instant::now();
        let result = meter.window(120, async { Ok(()) }).await;
        let elapsed = started.elapsed();
        meter.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
        let (window, ()) = result.unwrap();
        assert!(elapsed >= Duration::from_millis(120));
        assert!(window.elapsed_ms >= 120);
        assert!((1..=128).contains(&window.samples));
        assert!(
            u64::from(window.samples)
                <= window
                    .elapsed_ms
                    .checked_div(50)
                    .unwrap()
                    .checked_add(1)
                    .unwrap()
        );
        assert!(window.first > 0);
        assert!(window.last > 0);
        assert!(window.sampled_peak_bytes >= window.first.max(window.last));
    }

    #[tokio::test]
    async fn window_polls_work_concurrently_and_waits_for_slower_work() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let started = Instant::now();
        let result = meter
            .window(80, async {
                sleep(Duration::from_millis(180)).await;
                Ok(42_u32)
            })
            .await;
        let elapsed = started.elapsed();
        meter.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
        let (window, value) = result.unwrap();
        assert_eq!(value, 42);
        assert!(elapsed >= Duration::from_millis(180));
        assert!(window.elapsed_ms >= 180);
        assert!(window.samples >= 2);
    }

    #[tokio::test]
    async fn window_preserves_minimum_duration_when_work_fails_early() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let started = Instant::now();
        let result = meter
            .window(80, async { Err::<(), _>(HarnessError::new("work_failed")) })
            .await;
        let elapsed = started.elapsed();
        meter.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
        assert_eq!(result.unwrap_err(), HarnessError::new("work_failed"));
        assert!(elapsed >= Duration::from_millis(80));
    }

    #[tokio::test]
    async fn invalid_window_durations_never_poll_work() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let mut results = Vec::new();
        let polled = AtomicBool::new(false);
        for duration in [0_u64, 60_001, u64::MAX] {
            results.push(
                meter
                    .window(duration, async {
                        polled.store(true, Ordering::Release);
                        Ok(())
                    })
                    .await,
            );
        }
        meter.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
        assert_eq!(results.len(), 3);
        for result in results {
            assert_eq!(result.unwrap_err(), HarnessError::new("invalid_input"));
        }
        assert!(!polled.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn supervisor_cancellation_drops_in_flight_work_before_window_returns() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let entered = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let result = {
            let work = async {
                let _guard = Dropped(Arc::clone(&dropped));
                entered.store(true, Ordering::Release);
                pending::<Result<()>>().await
            };
            let cancel = async {
                while !entered.load(Ordering::Acquire) {
                    sleep(Duration::from_millis(1)).await;
                }
                supervisor.interrupt(130);
            };
            let (result, ()) = timeout(Duration::from_secs(2), async {
                tokio::join!(meter.window(60_000, work), cancel)
            })
            .await
            .unwrap();
            result
        };
        let was_dropped = dropped.load(Ordering::Acquire);
        meter.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
        assert_eq!(result.unwrap_err(), HarnessError::new("interrupted"));
        assert!(was_dropped);
    }

    #[tokio::test]
    async fn service_exit_fails_window_and_drops_work_without_detaching_it() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let dropped = Arc::new(AtomicBool::new(false));
        let pid = Pid::from_raw(i32::try_from(meter.pid()).unwrap()).unwrap();
        let result = timeout(
            Duration::from_secs(2),
            meter.window(1000, async {
                let _guard = Dropped(Arc::clone(&dropped));
                kill_process(pid, Signal::KILL).unwrap();
                pending::<Result<()>>().await
            }),
        )
        .await
        .unwrap();
        let was_dropped = dropped.load(Ordering::Acquire);
        meter.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
        result.unwrap_err();
        assert!(was_dropped);
    }

    #[tokio::test]
    async fn memory_requires_stop_and_reuses_lifetime_peak_not_sampled_peak() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        let window = Window {
            samples: 3,
            first: 100,
            last: 180,
            sampled_peak_bytes: u64::MAX,
            elapsed_ms: 125,
        };
        let before = meter.memory(&window);
        meter.stop().await.unwrap();
        let high_water = meter.high_water.unwrap();
        let measured = meter.memory(&window);
        let smaller = Window { last: 12, ..window };
        let decreased = meter.memory(&smaller);
        let empty = Window {
            samples: 0,
            ..smaller
        };
        let no_samples = meter.memory(&empty);
        meter.high_water = Some(0);
        let zero_high_water = meter.memory(&window);
        supervisor.cleanup().await.unwrap();
        assert_eq!(before.unwrap_err(), HarnessError::new("meter_failed"));
        let memory = measured.unwrap();
        assert_eq!(memory.limitation, MeterLimitation::ProcessLifetimeHighWater);
        assert_eq!(memory.source, Meter::ProcessRss);
        assert_eq!(memory.method, PeakMethod::HighWater);
        assert_eq!(memory.units, Units::Bytes);
        assert_eq!(memory.window_ms, 125);
        assert_eq!(memory.peak_bytes, high_water);
        assert_eq!(memory.retained_growth_bytes, 80);
        assert!(memory.service_only);
        assert_eq!(decreased.unwrap().retained_growth_bytes, 0);
        assert_eq!(no_samples.unwrap_err(), HarnessError::new("meter_failed"));
        assert_eq!(
            zero_high_water.unwrap_err(),
            HarnessError::new("meter_failed")
        );
    }

    #[tokio::test]
    async fn failed_cleanup_stays_latched_and_invalidates_memory() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = Supervisor::new();
        let mut meter = start(&supervisor, root.path()).await;
        meter.stop().await.unwrap();
        meter.stop_result = Some(Err(HarnessError::new("cleanup_failed")));
        let window = Window {
            samples: 1,
            first: 100,
            last: 100,
            sampled_peak_bytes: 100,
            elapsed_ms: 50,
        };
        let first = meter.stop().await;
        let second = meter.stop().await;
        let memory = meter.memory(&window);
        supervisor.cleanup().await.unwrap();
        assert_eq!(first, Err(HarnessError::new("cleanup_failed")));
        assert_eq!(second, Err(HarnessError::new("cleanup_failed")));
        assert_eq!(memory.unwrap_err(), HarnessError::new("cleanup_failed"));
    }
}

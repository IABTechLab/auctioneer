use super::super::{
    HarnessError, Result,
    fixture::{
        Mode,
        client::{Client, FixtureControl as Control},
    },
    http::{self, RequestSpec},
    meter::{NativeMeter, Window},
    process::{ProcessSpec, Supervisor},
};
use super::{
    geometry::{self, AuctionOptions},
    inputs::{self, Candidate, Export},
    load::{self, Category, ConfigStatus, Outcome},
    oracles,
    snapshots::{self, InstalledVariant},
    system,
};
use crate::qualification::profile::{Digest, Load};
use ::http::{Method, header::CACHE_CONTROL};
use bytes::Bytes;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::Instant;

pub struct Context<'a> {
    pub cli: &'a Path,
    pub binary: &'a Path,
    pub artifact: &'a Digest,
    pub repo: &'a Path,
    pub candidate: &'a Candidate,
    pub scratch: &'a Path,
    pub fixture: &'a mut Client,
    pub export: &'a Export,
    pub supervisor: &'a Supervisor,
}

#[derive(Clone, Copy)]
pub enum Ready<'a> {
    Snapshot(&'a InstalledVariant),
    Degraded,
}

pub async fn run(context: Context<'_>) -> Result<Vec<Value>> {
    context.supervisor.check_active().await?;
    let dir = context.scratch.join("snapshot-probes");
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700)
    };
    builder
        .create(&dir)
        .map_err(|_error| HarnessError::new("snapshot_probe_failed").at("snapshot_install_a"))?;
    let port = system::reserve_port()
        .await
        .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_start_a"))?;
    let environment = inputs::service_environment(port, &context.candidate.sdk)
        .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_start_a"))?;
    let supervisor = context.supervisor;
    // The caller owns scratch even when a process cannot be proven stopped.
    let mut backend = NativeBackend {
        context,
        dir,
        origin: format!("http://127.0.0.1:{port}"),
        environment,
        current: None,
        a: None,
        b: None,
        sequence: 0,
        attestations: Vec::with_capacity(4),
    };
    lifecycle(&mut backend, supervisor).await?;
    if backend.attestations.len() != 4 {
        return Err(HarnessError::new("snapshot_probe_failed").at("snapshot_restore_stop"));
    }
    Ok(backend.attestations)
}

pub async fn wait_ready(
    meter: &NativeMeter,
    origin: &str,
    expected: Ready<'_>,
    supervisor: &Supervisor,
) -> Result<()> {
    let end = Instant::now()
        .checked_add(Duration::from_secs(5))
        .ok_or(HarnessError::new("readiness_timeout"))?;
    loop {
        supervisor.check_active().await?;
        meter.assert_running().await?;
        let remaining = end.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(HarnessError::new("readiness_timeout"));
        }
        let timeout_ms = u64::try_from(remaining.as_millis())
            .map_err(|_error| HarnessError::new("readiness_timeout"))?
            .clamp(1, 250);
        let attempt = load::call(
            origin,
            "/status",
            RequestSpec {
                timeout_ms,
                body_cap: 4096,
                ..RequestSpec::default()
            },
            supervisor,
        )
        .await;
        let running = meter.assert_running().await;
        if matches!(
            attempt,
            Err(HarnessError {
                code: "cleanup_failed",
                ..
            })
        ) {
            return attempt.map(|_response| ());
        }
        running?;
        let response = attempt?;
        if Instant::now() >= end {
            return Err(HarnessError::new("readiness_timeout"));
        }
        if ready_observed(&response, expected) {
            return Ok(());
        }
        let next_probe = Instant::now()
            .checked_add(Duration::from_millis(25))
            .ok_or(HarnessError::new("readiness_timeout"))?
            .min(end);
        load::pause_until(next_probe, supervisor).await?;
    }
}

pub async fn observe_settings(
    origin: &str,
    expected: &InstalledVariant,
    candidate: &Candidate,
    fixture: &mut Client,
    supervisor: &Supervisor,
) -> Result<()> {
    observe_settings_with(origin, expected, candidate, fixture, supervisor).await
}

pub fn snapshot_observed(
    variant: &InstalledVariant,
    pid: u32,
    before: &snapshots::Snapshot,
    after: &snapshots::Snapshot,
    settings: &Digest,
    observed: bool,
    restarted: bool,
) -> bool {
    pid > 0
        && before.config_sha256 == variant.config_sha256
        && after.config_sha256 == before.config_sha256
        && settings == &variant.settings_sha256
        && observed
        && restarted
}

fn ready_observed(result: &Outcome, expected: Ready<'_>) -> bool {
    match expected {
        Ready::Snapshot(variant) => oracles::status_observed(result, variant),
        Ready::Degraded => {
            result.category == Category::Success && result.config_status == ConfigStatus::Degraded
        }
    }
}

fn config_refused(response: &http::Response) -> bool {
    response.category == http::Category::HttpError
        && response.status == 500
        && response
            .headers
            .get(CACHE_CONTROL)
            .is_some_and(|value| value == "no-store")
        && response.headers.get_all(CACHE_CONTROL).iter().count() == 1
        && Digest::of(&response.body) == Digest::of(b"Malformed host configuration")
}

async fn observe_corrupt(
    origin: &str,
    candidate: &Candidate,
    supervisor: &Supervisor,
) -> Result<bool> {
    let body = geometry::make_auction(candidate.base_load, AuctionOptions::default())?;
    let response = http::request(
        &format!("{origin}/openrtb2/auction"),
        RequestSpec {
            method: Method::POST,
            body: Bytes::from(body),
            timeout_ms: candidate.request_timeout_ms,
            body_cap: 4096,
            inspect: true,
            ..RequestSpec::default()
        },
        supervisor,
    )
    .await?;
    supervisor.check_active().await?;
    Ok(config_refused(&response))
}

async fn observe_settings_with(
    origin: &str,
    expected: &InstalledVariant,
    candidate: &Candidate,
    fixture: &mut impl Control,
    supervisor: &Supervisor,
) -> Result<()> {
    let status = load::call(
        origin,
        "/status",
        RequestSpec {
            body_cap: 4096,
            ..RequestSpec::default()
        },
        supervisor,
    )
    .await
    .map_err(|error| phase_error(error, "effective_settings_failed", "settings_status"))?;
    if !oracles::status_observed(&status, expected) {
        return Err(HarnessError::new("effective_settings_failed").at("settings_status"));
    }
    observe_node_refusal(origin, expected, candidate, supervisor)
        .await
        .map_err(|error| {
            phase_error(error, "effective_settings_failed", "settings_node_refusal")
        })?;
    observe_dispatch(origin, candidate, fixture, supervisor)
        .await
        .map_err(|error| phase_error(error, "effective_settings_failed", "settings_dispatch"))
}

async fn observe_node_refusal(
    origin: &str,
    expected: &InstalledVariant,
    candidate: &Candidate,
    supervisor: &Supervisor,
) -> Result<()> {
    let over = Load {
        request_bytes: candidate.maximum_load.request_bytes,
        nodes: expected
            .nodes
            .checked_add(1)
            .ok_or(HarnessError::new("invalid_input"))?,
        ..candidate.base_load
    };
    let body = geometry::make_auction(
        over,
        AuctionOptions {
            id: 999_999,
            ..AuctionOptions::default()
        },
    )?;
    let refused = load::call(
        origin,
        "/openrtb2/auction",
        RequestSpec {
            method: Method::POST,
            body: Bytes::from(body),
            body_cap: 4096,
            ..RequestSpec::default()
        },
        supervisor,
    )
    .await?;
    if refused.status != 400
        || refused.category != Category::Error
        || refused.body_sha256 != Digest::of(b"Invalid request: too many JSON nodes\n")
    {
        return Err(HarnessError::new("effective_settings_failed"));
    }
    Ok(())
}

async fn observe_dispatch(
    origin: &str,
    candidate: &Candidate,
    fixture: &mut impl Control,
    supervisor: &Supervisor,
) -> Result<()> {
    oracles::idle(fixture, supervisor).await?;
    fixture
        .configure(&oracles::settings(
            candidate.base_load,
            Mode::Gzip,
            candidate.seed,
            10,
            0,
        ))
        .await?;
    supervisor.check_active().await?;
    let body = geometry::make_auction(
        candidate.base_load,
        AuctionOptions {
            id: 999_998,
            debug: true,
            ..AuctionOptions::default()
        },
    )?;
    let auction = load::call(
        origin,
        "/openrtb2/auction",
        RequestSpec {
            method: Method::POST,
            body: Bytes::from(body),
            ..RequestSpec::default()
        },
        supervisor,
    )
    .await?;
    let calls = fixture.stats().await?;
    supervisor.check_active().await?;
    if auction.category != Category::Success
        || auction.auction.bids == 0
        || auction.auction.debug_captures != candidate.base_load.calls
        || calls.max_calls_per_auction != candidate.base_load.calls
    {
        return Err(HarnessError::new("effective_settings_failed"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Probe {
    A,
    B,
    Bad,
}

// Only the startup-snapshot lifecycle uses this failure-injection boundary.
trait Backend {
    type Process;

    fn install(&mut self, variant: Probe) -> impl Future<Output = Result<()>>;
    fn start(&mut self) -> impl Future<Output = Result<Self::Process>>;
    fn observe(
        &mut self,
        process: &Self::Process,
        expected: Probe,
    ) -> impl Future<Output = Result<bool>>;
    fn stop(&mut self, process: &mut Self::Process) -> impl Future<Output = Result<()>>;
    fn corrupt(&mut self) -> impl Future<Output = Result<()>>;
}

fn phase_error(error: HarnessError, category: &'static str, stage: &'static str) -> HarnessError {
    let code = match error.code {
        "interrupted" | "run_deadline" | "cleanup_failed" => error.code,
        _ => category,
    };
    HarnessError::new(code).at(stage)
}

async fn check_phase(supervisor: &Supervisor, stage: &'static str) -> Result<()> {
    supervisor
        .check_active()
        .await
        .map_err(|error| phase_error(error, "snapshot_probe_failed", stage))
}

async fn observe_phase<B: Backend>(
    backend: &mut B,
    process: Option<&B::Process>,
    expected: Probe,
    supervisor: &Supervisor,
    stage: &'static str,
) -> Result<()> {
    check_phase(supervisor, stage).await?;
    let child = process.ok_or(HarnessError::new("snapshot_probe_failed").at(stage))?;
    let observed = backend
        .observe(child, expected)
        .await
        .map_err(|error| phase_error(error, "snapshot_probe_failed", stage))?;
    check_phase(supervisor, stage).await?;
    if !observed {
        return Err(HarnessError::new("snapshot_probe_failed").at(stage));
    }
    Ok(())
}

async fn close<B: Backend>(
    backend: &mut B,
    active: &mut Option<B::Process>,
    stage: &'static str,
) -> Result<()> {
    if let Some(process) = active.as_mut() {
        backend
            .stop(process)
            .await
            .map_err(|_error| HarnessError::new("cleanup_failed").at(stage))?;
    }
    *active = None;
    Ok(())
}

async fn probe_phase<B: Backend>(
    backend: &mut B,
    active: &mut Option<B::Process>,
    stop_failed: &mut bool,
    supervisor: &Supervisor,
) -> Result<()> {
    check_phase(supervisor, "snapshot_install_a").await?;
    backend
        .install(Probe::A)
        .await
        .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_install_a"))?;
    check_phase(supervisor, "snapshot_start_a").await?;
    *active = Some(
        backend
            .start()
            .await
            .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_start_a"))?,
    );
    observe_phase(
        backend,
        active.as_ref(),
        Probe::A,
        supervisor,
        "snapshot_observe_a",
    )
    .await?;
    check_phase(supervisor, "snapshot_install_b").await?;
    backend
        .install(Probe::B)
        .await
        .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_install_b"))?;
    observe_phase(
        backend,
        active.as_ref(),
        Probe::A,
        supervisor,
        "snapshot_observe_running_a",
    )
    .await?;
    let first_stop = close(backend, active, "snapshot_stop_a").await;
    *stop_failed = first_stop.is_err();
    first_stop?;
    check_phase(supervisor, "snapshot_start_b").await?;
    *active = Some(
        backend
            .start()
            .await
            .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_start_b"))?,
    );
    observe_phase(
        backend,
        active.as_ref(),
        Probe::B,
        supervisor,
        "snapshot_observe_b",
    )
    .await?;
    let second_stop = close(backend, active, "snapshot_stop_b").await;
    *stop_failed = second_stop.is_err();
    second_stop?;
    check_phase(supervisor, "snapshot_corrupt").await?;
    backend
        .corrupt()
        .await
        .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_corrupt"))?;
    check_phase(supervisor, "snapshot_start_bad").await?;
    *active = Some(
        backend
            .start()
            .await
            .map_err(|error| phase_error(error, "snapshot_probe_failed", "snapshot_start_bad"))?,
    );
    observe_phase(
        backend,
        active.as_ref(),
        Probe::Bad,
        supervisor,
        "snapshot_observe_bad",
    )
    .await
}

async fn lifecycle(backend: &mut impl Backend, supervisor: &Supervisor) -> Result<()> {
    supervisor.check_active().await?;
    let mut active = None;
    let mut stop_failed = false;
    let phase = probe_phase(backend, &mut active, &mut stop_failed, supervisor).await;
    // A failed stop is a hard fence: do not retry it or spawn a restore process.
    if stop_failed {
        return phase;
    }
    close(backend, &mut active, "snapshot_cleanup").await?;
    if matches!(
        phase,
        Err(HarnessError {
            code: "cleanup_failed" | "interrupted" | "run_deadline",
            ..
        })
    ) {
        return phase;
    }
    check_phase(supervisor, "snapshot_restore_install").await?;
    let restoration = async {
        backend.install(Probe::A).await.map_err(|error| {
            phase_error(error, "snapshot_probe_failed", "snapshot_restore_install")
        })?;
        check_phase(supervisor, "snapshot_restore_start").await?;
        active = Some(backend.start().await.map_err(|error| {
            phase_error(error, "snapshot_probe_failed", "snapshot_restore_start")
        })?);
        observe_phase(
            backend,
            active.as_ref(),
            Probe::A,
            supervisor,
            "snapshot_restore_observe",
        )
        .await
    }
    .await;
    close(backend, &mut active, "snapshot_restore_stop").await?;
    if matches!(
        restoration,
        Err(HarnessError {
            code: "cleanup_failed" | "interrupted" | "run_deadline",
            ..
        })
    ) {
        return restoration;
    }
    check_phase(supervisor, "snapshot_restore_stop").await?;
    phase.and(restoration)
}

struct ProbeProcess {
    meter: NativeMeter,
    sequence: u32,
    cold: Window,
    warm: Window,
}

struct NativeBackend<'a> {
    context: Context<'a>,
    dir: PathBuf,
    origin: String,
    environment: BTreeMap<OsString, OsString>,
    current: Option<Probe>,
    a: Option<InstalledVariant>,
    b: Option<InstalledVariant>,
    sequence: u32,
    attestations: Vec<Value>,
}

impl NativeBackend<'_> {
    fn variant(&self, expected: Probe) -> Result<&InstalledVariant> {
        match expected {
            Probe::A => self.a.as_ref(),
            Probe::B => self.b.as_ref(),
            Probe::Bad => None,
        }
        .ok_or(HarnessError::new("snapshot_invalid"))
    }
}

impl Backend for NativeBackend<'_> {
    type Process = ProbeProcess;

    async fn install(&mut self, variant: Probe) -> Result<()> {
        let id = match variant {
            Probe::A => 1,
            Probe::B => 2,
            Probe::Bad => return Err(HarnessError::new("snapshot_invalid")),
        };
        let installed = snapshots::install_variant(
            self.context.cli,
            self.context.repo,
            &self.dir,
            self.context.candidate,
            id,
            self.context.supervisor,
        )
        .await?;
        match variant {
            Probe::A => {
                if self.a.as_ref().is_some_and(|previous| {
                    previous.config_sha256 != installed.config_sha256
                        || previous.settings_sha256 != installed.settings_sha256
                }) {
                    return Err(HarnessError::new("identity_mismatch"));
                }
                self.a = Some(installed);
            }
            Probe::B => self.b = Some(installed),
            Probe::Bad => return Err(HarnessError::new("snapshot_invalid")),
        }
        self.current = Some(variant);
        Ok(())
    }

    async fn start(&mut self) -> Result<ProbeProcess> {
        let current = self.current.ok_or(HarnessError::new("snapshot_invalid"))?;
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(HarnessError::new("invalid_input"))?;
        self.context.supervisor.check_active().await?;
        if current != Probe::Bad {
            let expected = self.variant(current)?;
            let snapshot = snapshots::read_snapshot(&self.dir)?;
            if snapshot.config_sha256 != expected.config_sha256
                || snapshot.store_sha256 != expected.store_sha256
            {
                return Err(HarnessError::new("identity_mismatch"));
            }
        }
        snapshots::assert_artifact(self.context.binary, self.context.artifact)?;
        let exported = system::inspect_export(
            self.context.binary,
            &self.environment,
            &self.dir,
            self.context.supervisor,
        )
        .await?;
        system::assert_same_export(self.context.export, &exported)?;
        snapshots::assert_artifact(self.context.binary, self.context.artifact)?;
        let mut spec = ProcessSpec::new(self.context.binary);
        spec.cwd = Some(self.dir.clone());
        spec.env = self.environment.clone();
        spec.timeout_ms = self.context.candidate.max_run_ms;
        let mut meter = NativeMeter::start(spec, self.context.supervisor).await?;
        self.sequence = sequence;
        let windows = async {
            let expected = match current {
                Probe::Bad => Ready::Degraded,
                Probe::A | Probe::B => Ready::Snapshot(self.variant(current)?),
            };
            let (cold, ()) = meter
                .window(
                    self.context.candidate.cold_window_ms,
                    wait_ready(&meter, &self.origin, expected, self.context.supervisor),
                )
                .await?;
            let (warm, ()) = meter
                .window(self.context.candidate.warm_window_ms, async { Ok(()) })
                .await?;
            Ok((cold, warm))
        }
        .await;
        match windows {
            Ok((cold, warm)) => Ok(ProbeProcess {
                meter,
                sequence,
                cold,
                warm,
            }),
            Err(error) => {
                meter
                    .stop()
                    .await
                    .map_err(|_error| HarnessError::new("cleanup_failed"))?;
                Err(error)
            }
        }
    }

    async fn observe(&mut self, process: &ProbeProcess, expected: Probe) -> Result<bool> {
        process.meter.assert_running().await?;
        let observation = async {
            if expected != Probe::Bad {
                let variant = self.variant(expected)?.clone();
                observe_settings(
                    &self.origin,
                    &variant,
                    self.context.candidate,
                    self.context.fixture,
                    self.context.supervisor,
                )
                .await?;
                return Ok(true);
            }
            observe_corrupt(
                &self.origin,
                self.context.candidate,
                self.context.supervisor,
            )
            .await
        }
        .await;
        let running = process.meter.assert_running().await;
        if matches!(
            observation,
            Err(HarnessError {
                code: "cleanup_failed",
                ..
            })
        ) {
            return observation;
        }
        running?;
        observation
    }

    async fn stop(&mut self, process: &mut ProbeProcess) -> Result<()> {
        process.meter.stop().await?;
        let cold_memory = process.meter.memory(&process.cold)?;
        let warm_memory = process.meter.memory(&process.warm)?;
        if process.meter.pid() == 0 || process.sequence == 0 {
            return Err(HarnessError::new("meter_failed"));
        }
        self.attestations.push(json!({
            "process_id": process.meter.pid(), "sequence": process.sequence,
            "cold_memory": cold_memory, "warm_memory": warm_memory,
        }));
        Ok(())
    }

    async fn corrupt(&mut self) -> Result<()> {
        self.context.supervisor.check_active().await?;
        snapshots::corrupt_snapshot(&self.dir)?;
        self.current = Some(Probe::Bad);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::fixture::{Coverage, Mode, Settings, Stats};
    use super::super::load::{Auction, Category, ConfigStatus};
    use super::*;
    use ::http::{HeaderMap, HeaderValue, Request as HttpRequest, Response as HttpResponse};
    use bytes::Bytes;
    use http_body_util::{BodyExt as _, Full};
    use hyper::{body::Incoming, server::conn::http1, service::service_fn};
    use hyper_util::rt::TokioIo;
    use serde_json::json;
    use std::convert::Infallible;
    use std::net::Ipv4Addr;
    use std::path::PathBuf;
    use std::result::Result as StdResult;
    use std::sync::{Arc, Mutex};
    use tokio::{net::TcpListener, time::sleep};

    const CANARY: &str = "PRIVATE_PAYLOAD_PATH_CREDENTIAL_CANARY";

    fn variant() -> InstalledVariant {
        InstalledVariant {
            id: 1,
            dir: PathBuf::from(CANARY),
            config_sha256: Digest::of(b"config-a"),
            settings_sha256: Digest::of(b"settings-a"),
            nodes: 4096,
            banner_sha256: Digest::of(b"q2_a"),
            store_sha256: Digest::of(b"outer-store-a"),
        }
    }

    fn snapshot(variant: &InstalledVariant) -> snapshots::Snapshot {
        snapshots::Snapshot {
            config_sha256: variant.config_sha256.clone(),
            store_sha256: variant.store_sha256.clone(),
        }
    }

    fn status(variant: &InstalledVariant) -> Outcome {
        Outcome {
            category: Category::Success,
            status: 200,
            latency_us: 1,
            slow_reads: 0,
            body_sha256: variant.banner_sha256.clone(),
            config_status: ConfigStatus::Ok,
            auction: Auction::default(),
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Event {
        Install(Probe),
        Start(u32, Probe),
        Observe(u32, Probe),
        Stop(u32),
        Corrupt,
    }

    enum Failure {
        Refuse,
        Error(HarnessError),
        Interrupt(i32),
    }

    struct Process {
        sequence: u32,
        loaded: Probe,
    }

    struct FakeBackend {
        supervisor: Supervisor,
        disk: Option<Probe>,
        active: Option<u32>,
        sequence: u32,
        events: Vec<Event>,
        failures: Vec<(usize, Failure)>,
    }

    impl FakeBackend {
        fn new(supervisor: &Supervisor) -> Self {
            Self {
                supervisor: supervisor.clone(),
                disk: None,
                active: None,
                sequence: 0,
                events: Vec::new(),
                failures: Vec::new(),
            }
        }

        async fn record(&mut self, event: Event) -> Result<bool> {
            let index = self.events.len();
            self.events.push(event);
            let Some(position) = self.failures.iter().position(|(at, _failure)| *at == index)
            else {
                return Ok(true);
            };
            match self.failures.remove(position).1 {
                Failure::Refuse => Ok(false),
                Failure::Error(error) => Err(error),
                Failure::Interrupt(code) => {
                    self.supervisor.interrupt(code);
                    self.supervisor.check_active().await?;
                    Err(HarnessError::new("interrupted"))
                }
            }
        }
    }

    impl Backend for FakeBackend {
        type Process = Process;

        async fn install(&mut self, variant: Probe) -> Result<()> {
            assert_ne!(variant, Probe::Bad);
            assert!(self.record(Event::Install(variant)).await?);
            self.disk = Some(variant);
            Ok(())
        }

        async fn start(&mut self) -> Result<Process> {
            assert_eq!(
                self.active, None,
                "a new process requires proven prior stop"
            );
            self.supervisor.check_active().await?;
            self.sequence = self
                .sequence
                .checked_add(1)
                .ok_or(HarnessError::new("invalid_input"))?;
            let loaded = self.disk.ok_or(HarnessError::new("snapshot_invalid"))?;
            assert!(self.record(Event::Start(self.sequence, loaded)).await?);
            self.active = Some(self.sequence);
            Ok(Process {
                sequence: self.sequence,
                loaded,
            })
        }

        async fn observe(&mut self, process: &Process, expected: Probe) -> Result<bool> {
            assert_eq!(self.active, Some(process.sequence));
            let permitted = self
                .record(Event::Observe(process.sequence, expected))
                .await?;
            Ok(permitted && process.loaded == expected)
        }

        async fn stop(&mut self, process: &mut Process) -> Result<()> {
            assert_eq!(self.active, Some(process.sequence));
            assert!(self.record(Event::Stop(process.sequence)).await?);
            self.active = None;
            Ok(())
        }

        async fn corrupt(&mut self) -> Result<()> {
            assert_eq!(self.active, None);
            assert!(self.record(Event::Corrupt).await?);
            self.disk = Some(Probe::Bad);
            Ok(())
        }
    }

    fn ordered_events() -> Vec<Event> {
        use Event::{Corrupt, Install, Observe, Start, Stop};
        use Probe::{A, B, Bad};
        vec![
            Install(A),
            Start(1, A),
            Observe(1, A),
            Install(B),
            Observe(1, A),
            Stop(1),
            Start(2, B),
            Observe(2, B),
            Stop(2),
            Corrupt,
            Start(3, Bad),
            Observe(3, Bad),
            Stop(3),
            Install(A),
            Start(4, A),
            Observe(4, A),
            Stop(4),
        ]
    }

    fn assert_restored(backend: &FakeBackend) {
        let sequence = backend.sequence;
        assert!(backend.events.ends_with(&[
            Event::Install(Probe::A),
            Event::Start(sequence, Probe::A),
            Event::Observe(sequence, Probe::A),
            Event::Stop(sequence),
        ]));
        assert_eq!(backend.disk, Some(Probe::A));
        assert_eq!(backend.active, None);
    }

    #[tokio::test]
    async fn lifecycle_uses_four_fresh_processes_and_observes_live_a_after_installing_b() {
        let supervisor = Supervisor::new();
        let mut backend = FakeBackend::new(&supervisor);
        lifecycle(&mut backend, &supervisor).await.unwrap();
        assert_eq!(backend.events, ordered_events());
        assert_eq!(backend.sequence, 4);
        assert_restored(&backend);
    }

    #[tokio::test]
    async fn a_false_bad_observation_still_stops_then_restores_through_a_fresh_process() {
        let supervisor = Supervisor::new();
        let mut backend = FakeBackend::new(&supervisor);
        backend.failures.push((11, Failure::Refuse));
        let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
        assert_eq!(
            error,
            HarnessError::new("snapshot_probe_failed").at("snapshot_observe_bad")
        );
        assert_eq!(backend.events, ordered_events());
        assert_restored(&backend);
    }

    #[tokio::test]
    async fn b_observation_failure_has_a_fixed_stage_without_underlying_canaries() {
        let supervisor = Supervisor::new();
        let mut backend = FakeBackend::new(&supervisor);
        backend
            .failures
            .push((7, Failure::Error(HarnessError::new(CANARY).at(CANARY))));
        let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
        assert_eq!(
            error,
            HarnessError::new("snapshot_probe_failed").at("snapshot_observe_b")
        );
        assert_eq!(
            serde_json::to_value(error).unwrap(),
            json!({
                "code": "snapshot_probe_failed", "stage": "snapshot_observe_b",
            })
        );
        assert!(!format!("{error:?}").contains(CANARY));
        let mut expected = ordered_events()[..9].to_vec();
        expected.extend([
            Event::Install(Probe::A),
            Event::Start(3, Probe::A),
            Event::Observe(3, Probe::A),
            Event::Stop(3),
        ]);
        assert_eq!(backend.events, expected);
        assert_restored(&backend);
    }

    #[tokio::test]
    async fn every_normal_probe_failure_restores_and_retains_its_literal_phase() {
        let cases = [
            (0, "snapshot_install_a"),
            (1, "snapshot_start_a"),
            (2, "snapshot_observe_a"),
            (3, "snapshot_install_b"),
            (4, "snapshot_observe_running_a"),
            (6, "snapshot_start_b"),
            (7, "snapshot_observe_b"),
            (9, "snapshot_corrupt"),
            (10, "snapshot_start_bad"),
            (11, "snapshot_observe_bad"),
        ];
        let mut checked = 0_u8;
        for (index, stage) in cases {
            let supervisor = Supervisor::new();
            let mut backend = FakeBackend::new(&supervisor);
            backend
                .failures
                .push((index, Failure::Error(HarnessError::new(CANARY).at(CANARY))));
            let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
            assert_eq!(error, HarnessError::new("snapshot_probe_failed").at(stage));
            assert!(!serde_json::to_string(&error).unwrap().contains(CANARY));
            assert_eq!(&backend.events[..=index], &ordered_events()[..=index]);
            assert_restored(&backend);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 10);
    }

    #[tokio::test]
    async fn restoration_failures_have_distinct_fixed_phases_and_stop_any_started_child() {
        let cases = [
            (13, "snapshot_restore_install"),
            (14, "snapshot_restore_start"),
            (15, "snapshot_restore_observe"),
        ];
        let mut checked = 0_u8;
        for (index, stage) in cases {
            let supervisor = Supervisor::new();
            let mut backend = FakeBackend::new(&supervisor);
            backend
                .failures
                .push((index, Failure::Error(HarnessError::new(CANARY))));
            let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
            assert_eq!(error, HarnessError::new("snapshot_probe_failed").at(stage));
            let end = if index == 15 {
                17
            } else {
                index.checked_add(1).unwrap()
            };
            assert_eq!(backend.events, ordered_events()[..end]);
            assert_eq!(backend.active, None);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 3);
    }

    #[tokio::test]
    async fn all_unproven_stops_block_restoration_and_new_spawns() {
        let cases = [
            (5, "snapshot_stop_a", 1),
            (8, "snapshot_stop_b", 2),
            (12, "snapshot_cleanup", 3),
            (16, "snapshot_restore_stop", 4),
        ];
        let mut checked = 0_u8;
        for (index, stage, active) in cases {
            let supervisor = Supervisor::new();
            let mut backend = FakeBackend::new(&supervisor);
            backend
                .failures
                .push((index, Failure::Error(HarnessError::new(CANARY))));
            let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
            assert_eq!(error, HarnessError::new("cleanup_failed").at(stage));
            assert_eq!(backend.events, ordered_events()[..=index]);
            assert_eq!(backend.active, Some(active));
            assert_eq!(backend.sequence, active);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 4);
    }

    #[tokio::test]
    async fn cleanup_failure_wins_over_an_earlier_probe_failure() {
        let supervisor = Supervisor::new();
        let mut backend = FakeBackend::new(&supervisor);
        backend
            .failures
            .push((7, Failure::Error(HarnessError::new(CANARY))));
        backend
            .failures
            .push((8, Failure::Error(HarnessError::new(CANARY))));
        let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
        assert_eq!(
            error,
            HarnessError::new("cleanup_failed").at("snapshot_cleanup")
        );
        assert_eq!(backend.events, ordered_events()[..9]);
        assert_eq!(backend.active, Some(2));
    }

    #[tokio::test]
    async fn restoration_observation_failure_does_not_replace_the_original_probe_failure() {
        let supervisor = Supervisor::new();
        let mut backend = FakeBackend::new(&supervisor);
        backend.failures.push((11, Failure::Refuse));
        backend
            .failures
            .push((15, Failure::Error(HarnessError::new(CANARY).at(CANARY))));
        let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
        assert_eq!(
            error,
            HarnessError::new("snapshot_probe_failed").at("snapshot_observe_bad")
        );
        assert_eq!(backend.events, ordered_events());
        assert_eq!(backend.active, None);
        assert!(!serde_json::to_string(&error).unwrap().contains(CANARY));
    }

    #[tokio::test]
    async fn restore_cleanup_failure_also_wins_over_an_earlier_probe_failure() {
        let supervisor = Supervisor::new();
        let mut backend = FakeBackend::new(&supervisor);
        backend.failures.push((11, Failure::Refuse));
        backend
            .failures
            .push((16, Failure::Error(HarnessError::new(CANARY))));
        let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
        assert_eq!(
            error,
            HarnessError::new("cleanup_failed").at("snapshot_restore_stop")
        );
        assert_eq!(backend.events, ordered_events());
        assert_eq!(backend.active, Some(4));
    }

    #[tokio::test]
    async fn an_unquiescent_start_failure_never_attempts_a_restoration_spawn() {
        let supervisor = Supervisor::new();
        let mut backend = FakeBackend::new(&supervisor);
        backend
            .failures
            .push((6, Failure::Error(HarnessError::new("cleanup_failed"))));
        let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
        assert_eq!(
            error,
            HarnessError::new("cleanup_failed").at("snapshot_start_b")
        );
        assert_eq!(backend.events, ordered_events()[..7]);
        assert_eq!(backend.sequence, 2);
    }

    #[tokio::test]
    async fn interruption_stops_the_active_process_without_installing_or_spawning_restore() {
        let mut checked = 0_u8;
        for code in [130_i32, 143_i32] {
            let supervisor = Supervisor::new();
            let mut backend = FakeBackend::new(&supervisor);
            backend.failures.push((11, Failure::Interrupt(code)));
            let error = lifecycle(&mut backend, &supervisor).await.unwrap_err();
            assert_eq!(error.code, "interrupted");
            assert_eq!(supervisor.exit_code(), code);
            assert_eq!(backend.events, ordered_events()[..13]);
            assert_eq!(backend.active, None);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 2);
    }

    #[tokio::test]
    async fn a_preinterrupted_supervisor_does_no_probe_work() {
        let supervisor = Supervisor::new();
        supervisor.interrupt(130);
        let mut backend = FakeBackend::new(&supervisor);
        assert_eq!(
            lifecycle(&mut backend, &supervisor).await.unwrap_err().code,
            "interrupted"
        );
        assert!(backend.events.is_empty());
    }

    fn refusal() -> http::Response {
        let body = b"Malformed host configuration".to_vec();
        http::Response {
            category: http::Category::HttpError,
            status: 500,
            bytes: body.len(),
            latency_us: 1,
            slow_reads: 0,
            body,
            headers: HeaderMap::from_iter([(CACHE_CONTROL, HeaderValue::from_static("no-store"))]),
        }
    }

    #[test]
    fn config_refusal_accepts_only_the_complete_exact_http_refusal() {
        assert!(config_refused(&refusal()));
    }

    #[test]
    fn config_refusal_requires_the_no_store_header_independently() {
        assert!(config_refused(&refusal()));
        let mut missing = refusal();
        missing.headers.clear();
        assert!(!config_refused(&missing));
        for value in ["private", "no-cache", "no-store, private", "NO-STORE"] {
            let mut changed = refusal();
            changed
                .headers
                .insert(CACHE_CONTROL, value.parse().unwrap());
            assert!(!config_refused(&changed));
        }
    }

    #[test]
    fn config_refusal_requires_the_exact_body_independently() {
        assert!(config_refused(&refusal()));
        for body in [
            "",
            "Malformed host configuration\n",
            "Malformed host configurations",
            CANARY,
        ] {
            let mut changed = refusal();
            changed.body = body.as_bytes().to_vec();
            changed.bytes = changed.body.len();
            assert!(!config_refused(&changed));
        }
    }

    #[test]
    fn config_refusal_requires_500_and_a_completed_http_response_independently() {
        assert!(config_refused(&refusal()));
        for status in [0, 200, 400, 503] {
            let mut changed = refusal();
            changed.status = status;
            assert!(!config_refused(&changed));
        }
        for category in [
            http::Category::Timeout,
            http::Category::Disconnect,
            http::Category::BodyLimit,
            http::Category::Interrupted,
            http::Category::NetworkError,
        ] {
            let mut changed = refusal();
            changed.category = category;
            assert!(!config_refused(&changed));
        }
    }

    #[test]
    fn readiness_requires_the_banner_digest_and_ok_or_cached_status() {
        let variant = variant();
        for marker in [ConfigStatus::Ok, ConfigStatus::Cached] {
            let mut result = status(&variant);
            result.config_status = marker;
            assert!(ready_observed(&result, Ready::Snapshot(&variant)));
            result.body_sha256 = Digest::of(CANARY.as_bytes());
            assert!(!ready_observed(&result, Ready::Snapshot(&variant)));
        }
        for marker in [ConfigStatus::Degraded, ConfigStatus::Missing] {
            let mut result = status(&variant);
            result.config_status = marker;
            assert!(!ready_observed(&result, Ready::Snapshot(&variant)));
        }
    }

    #[test]
    fn degraded_readiness_requires_success_and_the_actual_degraded_header() {
        let variant = variant();
        let mut result = status(&variant);
        result.config_status = ConfigStatus::Degraded;
        result.body_sha256 = Digest::of(b"degraded status body");
        assert!(ready_observed(&result, Ready::Degraded));
        for marker in [
            ConfigStatus::Ok,
            ConfigStatus::Cached,
            ConfigStatus::Missing,
        ] {
            result.config_status = marker;
            assert!(!ready_observed(&result, Ready::Degraded));
        }
        for category in [
            Category::Rejected,
            Category::Error,
            Category::Timeout,
            Category::Disconnect,
            Category::Interrupted,
        ] {
            result.config_status = ConfigStatus::Degraded;
            result.category = category;
            assert!(!ready_observed(&result, Ready::Degraded));
            let mut snapshot_result = status(&variant);
            snapshot_result.category = category;
            assert!(!ready_observed(&snapshot_result, Ready::Snapshot(&variant)));
        }
    }

    #[test]
    fn snapshot_observation_accepts_only_matching_restarted_observed_settings() {
        let variant = variant();
        let before = snapshot(&variant);
        assert!(snapshot_observed(
            &variant,
            42,
            &before,
            &before,
            &variant.settings_sha256,
            true,
            true
        ));
    }

    #[test]
    fn every_snapshot_observation_axis_is_independent() {
        let variant = variant();
        let before = snapshot(&variant);
        let wrong = Digest::of(b"different");
        let mut changed = before.clone();
        changed.config_sha256 = wrong.clone();
        assert!(snapshot_observed(
            &variant,
            42,
            &before,
            &before,
            &variant.settings_sha256,
            true,
            true
        ));
        let cases = [
            (0, &before, &before, &variant.settings_sha256, true, true),
            (42, &changed, &before, &variant.settings_sha256, true, true),
            (42, &changed, &changed, &variant.settings_sha256, true, true),
            (42, &before, &changed, &variant.settings_sha256, true, true),
            (42, &before, &before, &wrong, true, true),
            (42, &before, &before, &variant.settings_sha256, false, true),
            (42, &before, &before, &variant.settings_sha256, true, false),
        ];
        let mut checked = 0_u8;
        for (pid, prior, later, settings, observed, restarted) in cases {
            assert!(!snapshot_observed(
                &variant, pid, prior, later, settings, observed, restarted
            ));
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 7);
    }

    #[test]
    fn outer_store_immutability_is_not_inferred_by_snapshot_observation() {
        let variant = variant();
        let mut before = snapshot(&variant);
        before.store_sha256 = Digest::of(b"different outer before");
        let mut after = before.clone();
        after.store_sha256 = Digest::of(b"different outer after");
        assert!(snapshot_observed(
            &variant,
            42,
            &before,
            &after,
            &variant.settings_sha256,
            true,
            true
        ));
    }

    #[derive(Clone, Copy)]
    enum SettingsCase {
        Good,
        Cached,
        WrongBanner,
        MissingMarker,
        StatusFailure,
        WrongRefusalStatus,
        WrongRefusalBody,
        NoBids,
        MissingCaptures,
        WrongFixtureCalls,
        CorruptRefusal,
    }

    #[derive(Clone, Copy, Default)]
    struct ReplyDelays {
        status: Duration,
        node: Duration,
        dispatch: Duration,
        corrupt: Duration,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum SettingsEvent {
        Status,
        Auction(Vec<u8>),
        Stats,
        Configure,
    }

    struct SettingsFixture {
        events: Arc<Mutex<Vec<SettingsEvent>>>,
        calls: u32,
        configurations: Vec<Settings>,
    }

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "immediate test controls must read and mutate state only when polled"
    )]
    impl Control for SettingsFixture {
        async fn configure(&mut self, settings: &Settings) -> Result<Stats> {
            self.events.lock().unwrap().push(SettingsEvent::Configure);
            self.configurations.push(settings.clone());
            Ok(Stats::default())
        }

        async fn stats(&mut self) -> Result<Stats> {
            self.events.lock().unwrap().push(SettingsEvent::Stats);
            Ok(Stats {
                max_calls_per_auction: self.calls,
                ..Stats::default()
            })
        }

        async fn probe(&mut self, _first: u32, _second: u32) -> Result<Stats> {
            Err(HarnessError::new("invalid_input"))
        }

        async fn release(&mut self, _all: bool) -> Result<Stats> {
            Err(HarnessError::new("invalid_input"))
        }

        async fn coverage(&mut self, _ids: &[u32], _calls: u32) -> Result<Coverage> {
            Err(HarnessError::new("invalid_input"))
        }
    }

    fn settings_status_response(case: SettingsCase) -> HttpResponse<Full<Bytes>> {
        let response_builder = HttpResponse::builder();
        let status_response = match case {
            SettingsCase::MissingMarker => response_builder,
            SettingsCase::Cached => {
                response_builder.header("x-auctioneer-config-status", "ok-cached")
            }
            SettingsCase::Good
            | SettingsCase::WrongBanner
            | SettingsCase::StatusFailure
            | SettingsCase::WrongRefusalStatus
            | SettingsCase::WrongRefusalBody
            | SettingsCase::NoBids
            | SettingsCase::MissingCaptures
            | SettingsCase::WrongFixtureCalls
            | SettingsCase::CorruptRefusal => {
                response_builder.header("x-auctioneer-config-status", "ok")
            }
        };
        let code = if matches!(case, SettingsCase::StatusFailure) {
            503
        } else {
            200
        };
        let banner = if matches!(case, SettingsCase::WrongBanner) {
            CANARY
        } else {
            "q2_a"
        };
        status_response
            .status(code)
            .body(Full::new(Bytes::from_static(banner.as_bytes())))
            .unwrap()
    }

    async fn settings_route(
        request: HttpRequest<Incoming>,
        case: SettingsCase,
        calls: u32,
        events: Arc<Mutex<Vec<SettingsEvent>>>,
        delays: ReplyDelays,
    ) -> StdResult<HttpResponse<Full<Bytes>>, Infallible> {
        if request.uri().path() == "/status" {
            assert_eq!(request.method(), Method::GET);
            events.lock().unwrap().push(SettingsEvent::Status);
            sleep(delays.status).await;
            return Ok(settings_status_response(case));
        }
        let response_builder = HttpResponse::builder();
        assert_eq!(request.uri().path(), "/openrtb2/auction");
        assert_eq!(request.method(), Method::POST);
        let request_bytes = request.into_body().collect().await.unwrap().to_bytes();
        events
            .lock()
            .unwrap()
            .push(SettingsEvent::Auction(request_bytes.to_vec()));
        let value: Value = serde_json::from_slice(&request_bytes).unwrap();
        if matches!(case, SettingsCase::CorruptRefusal) {
            assert_eq!(value["id"], "q1");
            sleep(delays.corrupt).await;
            return Ok(response_builder
                .status(500)
                .header(CACHE_CONTROL, "no-store")
                .body(Full::new(Bytes::from_static(
                    b"Malformed host configuration",
                )))
                .unwrap());
        }
        if value["id"] == "q999999" {
            sleep(delays.node).await;
            let code = if matches!(case, SettingsCase::WrongRefusalStatus) {
                200
            } else {
                400
            };
            let refusal_body = if matches!(case, SettingsCase::WrongRefusalBody) {
                CANARY
            } else {
                "Invalid request: too many JSON nodes\n"
            };
            return Ok(response_builder
                .status(code)
                .body(Full::new(Bytes::from_static(refusal_body.as_bytes())))
                .unwrap());
        }
        assert_eq!(value["id"], "q999998");
        assert_eq!(value["ext"]["prebid"]["debug"], true);
        sleep(delays.dispatch).await;
        let captures = if matches!(case, SettingsCase::MissingCaptures) {
            calls.checked_sub(1).unwrap()
        } else {
            calls
        };
        let mut httpcalls = serde_json::Map::new();
        for index in 0..captures {
            httpcalls.insert(
                format!("fake_{index:02}"),
                json!([{"response": {"body": CANARY}}]),
            );
        }
        let bids = if matches!(case, SettingsCase::NoBids) {
            json!([])
        } else {
            json!([{"price": 1_u8}])
        };
        let auction_body = serde_json::to_vec(&json!({
            "seatbid": [{"bid": bids}], "ext": {"debug": {"httpcalls": httpcalls}},
        }))
        .unwrap();
        Ok(response_builder
            .body(Full::new(Bytes::from(auction_body)))
            .unwrap())
    }

    fn candidate() -> Candidate {
        serde_json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/testing/fixtures/resource-qualification/native-axum.json"
        )))
        .unwrap()
    }

    async fn settings_server(
        case: SettingsCase,
        calls: u32,
        delays: ReplyDelays,
        events: Arc<Mutex<Vec<SettingsEvent>>>,
        supervisor: &Supervisor,
    ) -> String {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let scope = supervisor.clone();
        supervisor
            .task(async move {
                while let Ok((stream, _peer)) = listener.accept().await {
                    let connection_events = Arc::clone(&events);
                    if scope
                        .task(async move {
                            let service = service_fn(move |request| {
                                settings_route(
                                    request,
                                    case,
                                    calls,
                                    Arc::clone(&connection_events),
                                    delays,
                                )
                            });
                            let _served = http1::Builder::new()
                                .serve_connection(TokioIo::new(stream), service)
                                .await;
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        origin
    }

    async fn exercise_settings(
        case: SettingsCase,
    ) -> (Result<()>, Vec<SettingsEvent>, SettingsFixture, Candidate) {
        exercise_settings_with(case, ReplyDelays::default(), None).await
    }

    async fn exercise_settings_with(
        case: SettingsCase,
        delays: ReplyDelays,
        request_timeout_ms: Option<u64>,
    ) -> (Result<()>, Vec<SettingsEvent>, SettingsFixture, Candidate) {
        let mut candidate = candidate();
        if let Some(timeout_ms) = request_timeout_ms {
            candidate.request_timeout_ms = timeout_ms;
        }
        let supervisor = Supervisor::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let calls = candidate.base_load.calls;
        let origin = settings_server(case, calls, delays, Arc::clone(&events), &supervisor).await;
        let mut fixture = SettingsFixture {
            events: Arc::clone(&events),
            calls: if matches!(case, SettingsCase::WrongFixtureCalls) {
                calls.checked_add(1).unwrap()
            } else {
                calls
            },
            configurations: Vec::new(),
        };
        let result =
            observe_settings_with(&origin, &variant(), &candidate, &mut fixture, &supervisor).await;
        supervisor.cleanup().await.unwrap();
        let observed = events.lock().unwrap().clone();
        (result, observed, fixture, candidate)
    }

    async fn exercise_corrupt(
        delay: Duration,
        timeout_ms: u64,
    ) -> (Result<bool>, Vec<SettingsEvent>) {
        let mut candidate = candidate();
        candidate.request_timeout_ms = timeout_ms;
        let supervisor = Supervisor::new();
        let events = Arc::new(Mutex::new(Vec::new()));
        let origin = settings_server(
            SettingsCase::CorruptRefusal,
            candidate.base_load.calls,
            ReplyDelays {
                corrupt: delay,
                ..ReplyDelays::default()
            },
            Arc::clone(&events),
            &supervisor,
        )
        .await;
        let result = observe_corrupt(&origin, &candidate, &supervisor).await;
        supervisor.cleanup().await.unwrap();
        let observed = events.lock().unwrap().clone();
        (result, observed)
    }

    async fn assert_settings_probe_delay(delays: ReplyDelays) {
        let started = Instant::now();
        let (result, events, _fixture, _candidate) =
            exercise_settings_with(SettingsCase::Good, delays, Some(100)).await;
        result.unwrap();
        assert_eq!(events.len(), 6);
        assert!(started.elapsed() >= Duration::from_millis(300));
    }

    #[tokio::test]
    async fn settings_probe_accepts_status_delayed_300ms_even_with_a_shorter_candidate_timeout() {
        assert_settings_probe_delay(ReplyDelays {
            status: Duration::from_millis(300),
            ..ReplyDelays::default()
        })
        .await;
    }

    #[tokio::test]
    async fn settings_probe_accepts_node_refusal_delayed_300ms_even_with_a_shorter_candidate_timeout()
     {
        assert_settings_probe_delay(ReplyDelays {
            node: Duration::from_millis(300),
            ..ReplyDelays::default()
        })
        .await;
    }

    #[tokio::test]
    async fn settings_probe_accepts_dispatch_delayed_300ms_even_with_a_shorter_candidate_timeout() {
        assert_settings_probe_delay(ReplyDelays {
            dispatch: Duration::from_millis(300),
            ..ReplyDelays::default()
        })
        .await;
    }

    #[tokio::test]
    async fn settings_probe_still_refuses_after_1000ms_with_a_longer_candidate_timeout() {
        let late = Duration::from_millis(1_100);
        let cases = [
            (
                ReplyDelays {
                    status: late,
                    ..ReplyDelays::default()
                },
                "settings_status",
                1_usize,
            ),
            (
                ReplyDelays {
                    node: late,
                    ..ReplyDelays::default()
                },
                "settings_node_refusal",
                2_usize,
            ),
            (
                ReplyDelays {
                    dispatch: late,
                    ..ReplyDelays::default()
                },
                "settings_dispatch",
                6_usize,
            ),
        ];
        let mut checked = 0_u8;
        for (delays, stage, count) in cases {
            let (result, events, _fixture, _candidate) =
                exercise_settings_with(SettingsCase::Good, delays, Some(2_000)).await;
            assert_eq!(
                result.unwrap_err(),
                HarnessError::new("effective_settings_failed").at(stage)
            );
            assert_eq!(events.len(), count);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 3);
    }

    #[tokio::test]
    async fn snapshot_refusal_accepts_delayed_300ms_within_the_candidate_timeout() {
        let started = Instant::now();
        let (result, events) = exercise_corrupt(Duration::from_millis(300), 600).await;
        assert!(result.unwrap());
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], SettingsEvent::Auction(_)));
        assert!(started.elapsed() >= Duration::from_millis(300));
    }

    #[tokio::test]
    async fn snapshot_refusal_still_uses_the_shorter_candidate_timeout() {
        let (result, events) = exercise_corrupt(Duration::from_millis(300), 100).await;
        assert!(!result.unwrap());
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], SettingsEvent::Auction(_)));
    }

    #[tokio::test]
    async fn settings_require_real_status_exact_node_refusal_and_positive_debug_dispatch() {
        let (result, events, fixture, candidate) = exercise_settings(SettingsCase::Good).await;
        result.unwrap();
        assert_eq!(events.len(), 6);
        assert_eq!(events[0], SettingsEvent::Status);
        assert_eq!(events[2], SettingsEvent::Stats);
        assert_eq!(events[3], SettingsEvent::Configure);
        assert_eq!(events[5], SettingsEvent::Stats);
        let SettingsEvent::Auction(refused_body) = &events[1] else {
            panic!("missing node probe")
        };
        assert_eq!(
            u64::try_from(refused_body.len()).unwrap(),
            candidate.maximum_load.request_bytes
        );
        assert_eq!(
            auctioneer_runtime::count_json_nodes(refused_body, 0x0001_0000).unwrap(),
            variant().nodes.checked_add(1).unwrap()
        );
        let refused_request: Value = serde_json::from_slice(refused_body).unwrap();
        assert_eq!(refused_request["id"], "q999999");
        assert_eq!(
            refused_request["imp"].as_array().unwrap().len(),
            usize::from(candidate.base_load.impressions)
        );
        assert_eq!(
            refused_request["imp"][0]["ext"]["bidder"]
                .as_object()
                .unwrap()
                .len(),
            usize::try_from(candidate.base_load.calls).unwrap()
        );
        let SettingsEvent::Auction(dispatched) = &events[4] else {
            panic!("missing dispatch probe")
        };
        assert_eq!(
            u64::try_from(dispatched.len()).unwrap(),
            candidate.base_load.request_bytes
        );
        assert_eq!(
            auctioneer_runtime::count_json_nodes(dispatched, 0x0001_0000).unwrap(),
            candidate.base_load.nodes
        );
        assert_eq!(fixture.configurations.len(), 1);
        let settings = &fixture.configurations[0];
        assert!(matches!(settings.mode, Mode::Gzip));
        assert_eq!(settings.encoded_bytes, candidate.base_load.encoded_bytes);
        assert_eq!(settings.decoded_bytes, candidate.base_load.decoded_bytes);
        assert_eq!(settings.seed, candidate.seed);
    }

    #[tokio::test]
    async fn settings_accept_the_real_cached_status_marker() {
        exercise_settings(SettingsCase::Cached).await.0.unwrap();
    }

    #[tokio::test]
    async fn settings_failures_report_only_the_fixed_independent_stage() {
        let cases = [
            (SettingsCase::WrongBanner, "settings_status", 1),
            (SettingsCase::MissingMarker, "settings_status", 1),
            (SettingsCase::StatusFailure, "settings_status", 1),
            (SettingsCase::WrongRefusalStatus, "settings_node_refusal", 2),
            (SettingsCase::WrongRefusalBody, "settings_node_refusal", 2),
            (SettingsCase::NoBids, "settings_dispatch", 6),
            (SettingsCase::MissingCaptures, "settings_dispatch", 6),
            (SettingsCase::WrongFixtureCalls, "settings_dispatch", 6),
        ];
        let mut checked = 0_u8;
        for (case, stage, count) in cases {
            let (result, events, _fixture, _candidate) = exercise_settings(case).await;
            let error = result.unwrap_err();
            assert_eq!(
                error,
                HarnessError::new("effective_settings_failed").at(stage)
            );
            assert_eq!(events.len(), count);
            assert!(!serde_json::to_string(&error).unwrap().contains(CANARY));
            assert!(!format!("{error:?}").contains(CANARY));
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 8);
    }
}

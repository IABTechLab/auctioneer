use super::super::{
    HarnessError, Result,
    fixture::{
        Mode, Options as FixtureOptions, Settings, Stats,
        client::{Client, FixtureControl as _},
    },
    http::RequestSpec,
    io,
    meter::NativeMeter,
    process::{ProcessSpec, Supervisor},
};
use super::{
    consents::{self, Consent},
    geometry::{self, AuctionOptions},
    inputs::{self, Candidate},
    load::{self, Category, Outcome},
    oracles, probes, snapshots, system,
};
use crate::qualification::profile::{self, Backend, Digest, Load, Profile, Target};
use bytes::Bytes;
use http::Method;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{env, fs};
use tokio::time::Instant;

const INPUT_CAP: usize = 1 << 20_u32;
const ARTIFACT_CAP: usize = 512 << 20_u32;

struct Declarations {
    profile: Profile,
    cli: Digest,
}

struct Frozen {
    root: PathBuf,
    declared: Declarations,
    before: [Digest; 6],
}

impl Frozen {
    fn open(evidence: &Path) -> Result<Self> {
        let root =
            fs::canonicalize(evidence).map_err(|_error| HarnessError::new("invalid_input"))?;
        let before = fingerprints(&root)?;
        let profile_bytes = io::read_bytes(&root.join("profile.json"), INPUT_CAP)?;
        let runner_bytes = io::read_bytes(&root.join("runner.json"), INPUT_CAP)?;
        let [
            profile_hash,
            runner_hash,
            _report,
            _observations,
            binary_hash,
            cli_hash,
        ] = &before;
        let declared = declarations(&profile_bytes, &runner_bytes)?;
        if &Digest::of(&profile_bytes) != profile_hash
            || &Digest::of(&runner_bytes) != runner_hash
            || &declared.profile.identity.artifact_sha256 != binary_hash
            || &declared.cli != cli_hash
        {
            return Err(HarnessError::new("identity_mismatch"));
        }
        Ok(Self {
            root,
            declared,
            before,
        })
    }

    fn binary(&self) -> PathBuf {
        self.root.join("artifacts/auctioneer-adapter-axum")
    }

    fn cli(&self) -> PathBuf {
        self.root.join("artifacts/auctioneer-cli")
    }

    fn verify(&self) -> Result<()> {
        snapshots::assert_artifact(
            &self.binary(),
            &self.declared.profile.identity.artifact_sha256,
        )?;
        snapshots::assert_artifact(&self.cli(), &self.declared.cli)?;
        unchanged(&self.before, &fingerprints(&self.root)?)
    }

    async fn exercise(
        &self,
        repo: &Path,
        candidate: &Candidate,
        scratch: &Path,
        supervisor: &Supervisor,
    ) -> Result<Value> {
        self.verify()?;
        let installed =
            snapshots::install_variant(&self.cli(), repo, scratch, candidate, 1, supervisor)
                .await
                .map_err(|failure| phase_error(failure, "frozen_install"))?;
        self.verify()?;
        let consents = consents::fixtures(repo)?;
        let selected = consents
            .get(..14)
            .ok_or(HarnessError::new("reference_unavailable"))?;
        if selected
            .iter()
            .map(Consent::version)
            .collect::<BTreeSet<_>>()
            .len()
            != 14
        {
            return Err(HarnessError::new("reference_unavailable"));
        }
        // Historical frozen CLIs predate the Rust fixture protocol. The current
        // executable owns that protocol; only the frozen CLI installs config.
        let fixture_binary =
            env::current_exe().map_err(|_error| HarnessError::new("frozen_probe_failed"))?;
        let mut fixture = Client::start(
            &fixture_binary,
            &FixtureOptions::Native {
                port: 8787,
                body_cap: 32 << 10_u32,
                max_requests: 100,
                max_sockets: 16,
            },
            supervisor,
        )
        .await?;
        let mut meter = None;
        let phase = async {
            fixture.configure(&fixture_settings(Mode::Gzip)).await?;
            let port = system::reserve_port().await?;
            let origin = format!("http://127.0.0.1:{port}");
            let mut service = ProcessSpec::new(self.binary());
            service.cwd = Some(scratch.to_owned());
            service.env = inputs::service_environment(port, &candidate.sdk)?;
            service.timeout_ms = 15_000;
            meter = Some(NativeMeter::start(service, supervisor).await?);
            let active = meter
                .as_ref()
                .ok_or(HarnessError::new("frozen_probe_failed"))?;
            probes::wait_ready(
                active,
                &origin,
                probes::Ready::Snapshot(&installed),
                supervisor,
            )
            .await
            .map_err(|failure| phase_error(failure, "frozen_ready"))?;
            let context = oracles::Context {
                origin: &origin,
                load: candidate.base_load,
                seed: 7,
                timeout_ms: 1500,
                supervisor,
            };
            let summary = held_probe(&context, &mut fixture, selected).await?;
            active.assert_running().await?;
            let snapshot = snapshots::read_snapshot(scratch)?;
            if snapshot.config_sha256 != installed.config_sha256
                || snapshot.store_sha256 != installed.store_sha256
            {
                return Err(HarnessError::new("identity_mismatch"));
            }
            Ok(summary)
        }
        .await;
        let service_stop = if let Some(active) = meter.as_mut() {
            active.stop().await
        } else {
            Ok(())
        };
        let fixture_stop = fixture.stop().await;
        finish_phase(phase, service_stop, fixture_stop)
    }
}

pub async fn run(evidence: &Path, supervisor: &Supervisor) -> Result<Value> {
    supervisor.check_active().await?;
    let frozen = Frozen::open(evidence)?;
    let repo = system::repository_root()?;
    let candidate = inputs::parse_candidate(&io::read_bytes(
        &repo.join("docs/testing/fixtures/resource-qualification/native-axum.json"),
        INPUT_CAP,
    )?)?;
    let catalog = inputs::parse_catalog(&io::read_bytes(
        &repo.join("docs/testing/fixtures/resource-qualification/catalog.json"),
        INPUT_CAP,
    )?)?;
    inputs::validate_candidate(&candidate, &catalog)?;
    // Keep scratch owned by this command until all registered processes/tasks
    // have joined, including if the run future is externally dropped.
    let scratch = tempfile::Builder::new()
        .prefix("auctioneer-frozen-probe-")
        .tempdir()
        .map_err(|_error| HarnessError::new("output_io"))?
        .keep();
    let attempted = frozen
        .exercise(&repo, &candidate, &scratch, supervisor)
        .await;
    let phase = match attempted {
        Ok(value) => supervisor.check_active().await.map(|()| value),
        Err(failure) => Err(failure),
    };
    let cleanup = supervisor.cleanup().await;
    if cleanup.is_err()
        || phase
            .as_ref()
            .err()
            .is_some_and(|failure| failure.code == "cleanup_failed")
    {
        return Err(HarnessError::new("cleanup_failed").at("frozen_cleanup"));
    }
    let stable = frozen.verify();
    let completion = supervisor.check_completion().await;
    if completion
        .as_ref()
        .err()
        .is_some_and(|failure| failure.code == "cleanup_failed")
    {
        return Err(HarnessError::new("cleanup_failed").at("frozen_cleanup"));
    }
    fs::remove_dir_all(&scratch)
        .map_err(|_error| HarnessError::new("cleanup_failed").at("frozen_cleanup"))?;
    completion?;
    stable?;
    phase
}

fn finish_phase(
    phase: Result<Value>,
    service_stop: Result<()>,
    fixture_stop: Result<()>,
) -> Result<Value> {
    if fixture_stop.is_err()
        || service_stop
            .as_ref()
            .err()
            .is_some_and(|failure| failure.code == "cleanup_failed")
        || phase
            .as_ref()
            .err()
            .is_some_and(|failure| failure.code == "cleanup_failed")
    {
        return Err(HarnessError::new("cleanup_failed").at("frozen_cleanup"));
    }
    service_stop.map_err(|failure| phase_error(failure, "frozen_service_stop"))?;
    phase
}

fn fingerprints(evidence: &Path) -> Result<[Digest; 6]> {
    let mut values = Vec::with_capacity(6);
    for (relative, cap) in [
        ("profile.json", INPUT_CAP),
        ("runner.json", INPUT_CAP),
        ("report.json", INPUT_CAP),
        ("observations.json", INPUT_CAP),
        ("artifacts/auctioneer-adapter-axum", ARTIFACT_CAP),
        ("artifacts/auctioneer-cli", ARTIFACT_CAP),
    ] {
        values.push(
            Digest::try_from(io::file_digest(&evidence.join(relative), cap)?)
                .map_err(|_error| HarnessError::new("identity_mismatch"))?,
        );
    }
    values
        .try_into()
        .map_err(|_error| HarnessError::new("identity_mismatch"))
}

fn fixture_settings(mode: Mode) -> Settings {
    Settings {
        mode,
        encoded_bytes: 4096,
        decoded_bytes: 8192,
        seed: 7,
        delay_ms: 10,
        stall_after_calls: 0,
    }
}

fn phase_error(failure: HarnessError, stage: &'static str) -> HarnessError {
    let code = match failure.code {
        "cleanup_failed" | "interrupted" | "run_deadline" | "identity_mismatch" => failure.code,
        _ => "frozen_probe_failed",
    };
    HarnessError::new(code).at(stage)
}

async fn held_probe(
    context: &oracles::Context<'_>,
    fixture: &mut Client,
    proofs: &[Consent],
) -> Result<Value> {
    context.supervisor.check_active().await?;
    if proofs.len() != 14 {
        return Err(HarnessError::new("reference_unavailable"));
    }
    fixture.configure(&fixture_settings(Mode::Park)).await?;
    let primary = oracles::Primary::start(context, 890_000, false).await?;
    let mut seen = BTreeSet::new();
    let phase = held_controls(context, fixture, &primary, proofs, &mut seen).await;
    // Release and explicitly join the same ordinary request even if a consent
    // assertion failed. Its managed task stays registered until joined.
    let released = fixture.release(true).await;
    let ordinary = primary.drain(context.supervisor).await;
    if [
        phase.as_ref().err(),
        released.as_ref().err(),
        ordinary.as_ref().err(),
    ]
    .into_iter()
    .flatten()
    .any(|failure| failure.code == "cleanup_failed")
    {
        return Err(HarnessError::new("cleanup_failed").at("frozen_ordinary_drain"));
    }
    let parses = phase?;
    released.map_err(|failure| phase_error(failure, "frozen_release"))?;
    let completed = ordinary.map_err(|failure| phase_error(failure, "frozen_ordinary_drain"))?;
    overlap_complete(parses, &seen, &completed, &fixture.stats().await?)?;
    let positive = auction(
        context,
        geometry::make_auction(
            context.load,
            AuctionOptions {
                id: 999_990,
                ..AuctionOptions::default()
            },
        )?,
    )
    .await?;
    let final_stats = fixture.stats().await?;
    recovery_complete(&positive, final_stats.calls)?;
    let coverage = fixture.coverage(&[890_000, 999_990], 16).await?;
    if coverage.auctions != 2 || coverage.complete_auctions != 2 {
        return Err(HarnessError::new("frozen_probe_failed").at("frozen_recovery"));
    }
    Ok(json!({
        "schema_version": 1_u16,
        "category": "frozen_probe_passed",
        "gvl_parses": parses,
        "distinct_versions": seen.len(),
        "invalid_consent_controls": 3_u8,
        "invalid_consent_credits": 0_u8,
        "ordinary_bids": completed.auction.bids,
        "recovery_bids": positive.auction.bids,
        "fixture_calls": final_stats.calls,
        "evidence_unchanged": true,
        "resource_qualification": "not_evaluated",
        "publisher_pilot_ready": false,
    }))
}

async fn held_controls(
    context: &oracles::Context<'_>,
    fixture: &mut Client,
    primary: &oracles::Primary,
    proofs: &[Consent],
    seen: &mut BTreeSet<u16>,
) -> Result<u64> {
    wait_for_hold(context, fixture, primary).await?;
    let mut parses = 0_u64;
    for (index, proof) in proofs.iter().enumerate() {
        assert_held(context, fixture, primary).await?;
        let id = 900_000_u32
            .checked_add(
                u32::try_from(index).map_err(|_error| HarnessError::new("frozen_probe_failed"))?,
            )
            .ok_or(HarnessError::new("frozen_probe_failed"))?;
        let result = auction(
            context,
            geometry::make_auction(
                context.load,
                AuctionOptions {
                    id,
                    consent: Some(proof.value()),
                    ..AuctionOptions::default()
                },
            )?,
        )
        .await?;
        assert_held(context, fixture, primary).await?;
        if result.category != Category::Success
            || result.status != 200
            || !result.auction.privacy_blocked
            || result.auction.privacy_refused
            || result.auction.bids != 0
            || consents::credit(&result, proof, Some(proof.value()), seen) != 1
        {
            return Err(HarnessError::new("frozen_probe_failed").at("frozen_gvl"));
        }
        parses = parses
            .checked_add(1)
            .ok_or(HarnessError::new("frozen_probe_failed"))?;
    }
    let first = proofs
        .first()
        .ok_or(HarnessError::new("reference_unavailable"))?;
    invalid_controls(context, fixture, primary, first).await?;
    Ok(parses)
}

async fn invalid_controls(
    context: &oracles::Context<'_>,
    fixture: &mut Client,
    primary: &oracles::Primary,
    first: &Consent,
) -> Result<()> {
    let core = first
        .value()
        .split_once('.')
        .map(|(core, _trailing)| core)
        .ok_or(HarnessError::new("reference_unavailable"))?;
    let mut controls = 0_u8;
    for invalid in [None, Some("not-a-TC-string"), Some(core)] {
        assert_held(context, fixture, primary).await?;
        let result = auction(context, invalid_body(context.load, invalid)?).await?;
        assert_held(context, fixture, primary).await?;
        // A fresh set is essential: reusing the fourteen-version set would
        // make even a falsely accepted consent earn zero duplicate credit.
        // Missing consent denies without a malformed warning; the unrelated
        // valid proof must not turn that denial into parse evidence.
        let credit = consents::credit(&result, first, invalid, &mut BTreeSet::new());
        invalid_consent(&result, credit, invalid.is_some())
            .map_err(|failure| phase_error(failure, "frozen_invalid_consent"))?;
        controls = controls
            .checked_add(1)
            .ok_or(HarnessError::new("frozen_probe_failed"))?;
    }
    if controls != 3 {
        return Err(HarnessError::new("frozen_probe_failed"));
    }
    Ok(())
}

async fn wait_for_hold(
    context: &oracles::Context<'_>,
    fixture: &mut Client,
    primary: &oracles::Primary,
) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(400))
        .ok_or(HarnessError::new("frozen_probe_failed"))?;
    loop {
        context.supervisor.check_active().await?;
        let stats = fixture.stats().await?;
        if Instant::now() >= deadline || stats.calls > 2 || stats.current_active_calls > 2 {
            return Err(HarnessError::new("frozen_probe_failed").at("frozen_hold"));
        }
        if held_stats(&stats).is_ok() && primary.held() {
            return Ok(());
        }
        let next = Instant::now()
            .checked_add(Duration::from_millis(5))
            .ok_or(HarnessError::new("frozen_probe_failed"))?;
        load::pause_until(next.min(deadline), context.supervisor).await?;
    }
}

async fn assert_held(
    context: &oracles::Context<'_>,
    fixture: &mut Client,
    primary: &oracles::Primary,
) -> Result<()> {
    context.supervisor.check_active().await?;
    held_stats(&fixture.stats().await?)?;
    if !primary.held() {
        return Err(HarnessError::new("frozen_probe_failed").at("frozen_hold"));
    }
    Ok(())
}

async fn auction(context: &oracles::Context<'_>, body: Vec<u8>) -> Result<Outcome> {
    load::call(
        context.origin,
        "/openrtb2/auction",
        RequestSpec {
            method: Method::POST,
            body: Bytes::from(body),
            timeout_ms: context.timeout_ms,
            ..RequestSpec::default()
        },
        context.supervisor,
    )
    .await
}

fn invalid_body(load: Load, consent: Option<&str>) -> Result<Vec<u8>> {
    let mut value: Value = inputs::parse_json(
        &geometry::make_auction(
            load,
            AuctionOptions {
                id: 950_000,
                ..AuctionOptions::default()
            },
        )?,
        INPUT_CAP,
    )?;
    let regs = value
        .get_mut("regs")
        .and_then(Value::as_object_mut)
        .ok_or(HarnessError::new("invalid_input"))?;
    regs.insert("gdpr".to_owned(), json!(1_u8));
    if let Some(consent_value) = consent {
        value
            .as_object_mut()
            .ok_or(HarnessError::new("invalid_input"))?
            .insert("user".to_owned(), json!({"consent": consent_value}));
    }
    io::json_bytes(&value, INPUT_CAP)
}

fn declarations(profile_bytes: &[u8], runner_bytes: &[u8]) -> Result<Declarations> {
    #[derive(Deserialize)]
    struct RunnerIdentity {
        schema_version: u16,
        cli_sha256: Digest,
    }

    let profile =
        profile::parse(profile_bytes).map_err(|_error| HarnessError::new("invalid_input"))?;
    let runner: RunnerIdentity = inputs::parse_json(runner_bytes, 1 << 20_u32)?;
    if profile.identity.target != Target::Axum
        || profile.backend != Backend::StartupSnapshot
        || runner.schema_version != 1
    {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(Declarations {
        profile,
        cli: runner.cli_sha256,
    })
}

fn unchanged(before: &[Digest; 6], after: &[Digest; 6]) -> Result<()> {
    if before != after {
        return Err(HarnessError::new("identity_mismatch"));
    }
    Ok(())
}

fn held_stats(stats: &Stats) -> Result<()> {
    if stats.calls != 2 || stats.current_active_calls != 2 {
        return Err(HarnessError::new("frozen_probe_failed"));
    }
    Ok(())
}

fn overlap_complete(
    parses: u64,
    seen: &BTreeSet<u16>,
    ordinary: &Outcome,
    stats: &Stats,
) -> Result<()> {
    if parses != 14 || seen.len() != 14 || !sixteen_bids(ordinary) || stats.calls != 16 {
        return Err(HarnessError::new("frozen_probe_failed"));
    }
    Ok(())
}

fn invalid_consent(result: &Outcome, credit: u64, expected_refusal: bool) -> Result<()> {
    if !result.auction.privacy_blocked
        || result.auction.bids != 0
        || credit != 0
        || result.auction.privacy_refused != expected_refusal
    {
        return Err(HarnessError::new("frozen_probe_failed"));
    }
    Ok(())
}

fn recovery_complete(result: &Outcome, calls: u32) -> Result<()> {
    if !sixteen_bids(result) || calls != 32 {
        return Err(HarnessError::new("frozen_probe_failed"));
    }
    Ok(())
}

fn sixteen_bids(result: &Outcome) -> bool {
    result.category == Category::Success && result.status == 200 && result.auction.bids == 16
}

#[cfg(test)]
mod tests {
    use super::super::load::{Auction, ConfigStatus};
    use super::*;
    use serde_json::json;
    use std::array::from_fn;

    const PROFILE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/tests/fixtures/resource-qualification/profile.json"
    ));
    const CLI_DIGEST: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const ARTIFACT_DIGEST: &str =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const CANARY: &str = "SECRET_FROZEN_EVIDENCE_PATH_OR_OUTPUT";

    fn current_profile_bytes() -> Vec<u8> {
        // Exercise current probe declarations, not historical requalification.
        let mut value = serde_json::to_value(profile::tests::synthetic_profile(PROFILE)).unwrap();
        value["resources"]["limits"]["isolate_total_bytes"] = json!(5_000_000_000_u64);
        value["platform"]["memory_bytes"] = json!(5_000_000_000_u64);
        value["hardware"]["memory_bytes"] = json!(0x0002_0000_0000_u64);
        value["headroom_bytes"] = json!(312_500_000_u64);
        profile::tests::debug_references(&mut value);
        serde_json::to_vec(&value).unwrap()
    }

    fn outcome(bids: u32) -> Outcome {
        Outcome {
            category: Category::Success,
            status: 200,
            latency_us: 1,
            slow_reads: 0,
            body_sha256: Digest::try_from(ARTIFACT_DIGEST.to_owned()).unwrap(),
            config_status: ConfigStatus::Ok,
            auction: Auction {
                bids,
                ..Auction::default()
            },
        }
    }

    fn error(result: Result<()>, code: &'static str) {
        let failure = result.unwrap_err();
        assert_eq!(failure, HarnessError::new(code));
        assert!(!serde_json::to_string(&failure).unwrap().contains(CANARY));
        assert!(!format!("{failure:?}").contains(CANARY));
    }

    #[test]
    fn prior_cleanup_failure_wins_over_a_later_meter_failure_or_successful_cleanup() {
        let failed = HarnessError::new("cleanup_failed");
        let meter = HarnessError::new("meter_failed");
        let probe = HarnessError::new("frozen_probe_failed");
        let mut checked = 0_usize;
        for (phase, service_stop, fixture_stop) in [
            (Err(failed), Err(meter), Ok(())),
            (Err(failed), Ok(()), Ok(())),
            (Err(probe), Err(failed), Ok(())),
            (Ok(Value::Null), Err(failed), Ok(())),
            (Err(probe), Ok(()), Err(failed)),
        ] {
            assert_eq!(
                finish_phase(phase, service_stop, fixture_stop).unwrap_err(),
                HarnessError::new("cleanup_failed").at("frozen_cleanup")
            );
            checked += 1;
        }
        assert_eq!(checked, 5);
    }

    #[test]
    fn typed_declarations_preserve_literal_frozen_binary_and_cli_digests() {
        let runner = serde_json::to_vec(&json!({
            "schema_version": 1_u16,
            "cli_sha256": CLI_DIGEST,
            "category": "evaluated",
            "historical_runner_metadata": CANARY,
        }))
        .unwrap();
        let declared = declarations(&current_profile_bytes(), &runner).unwrap();
        assert_eq!(
            declared.profile.identity.artifact_sha256,
            Digest::try_from(ARTIFACT_DIGEST.to_owned()).unwrap()
        );
        assert_eq!(
            declared.cli,
            Digest::try_from(CLI_DIGEST.to_owned()).unwrap()
        );
    }

    #[test]
    fn malformed_missing_or_duplicate_cli_identity_never_becomes_a_probe_prerequisite() {
        let valid = serde_json::to_vec(&json!({
            "schema_version": 1_u16,
            "cli_sha256": CLI_DIGEST,
        }))
        .unwrap();
        let duplicate = format!(
            "{{\"schema_version\":1,\"cli_sha256\":\"{CLI_DIGEST}\",\"cli_sha256\":\"{CLI_DIGEST}\"}}"
        );
        let cases = [
            CANARY.as_bytes().to_vec(),
            b"{}".to_vec(),
            serde_json::to_vec(&json!({"schema_version": 1_u16})).unwrap(),
            serde_json::to_vec(&json!({"schema_version": 0_u16, "cli_sha256": CLI_DIGEST}))
                .unwrap(),
            serde_json::to_vec(&json!({"schema_version": 1_u16, "cli_sha256": CANARY})).unwrap(),
            duplicate.into_bytes(),
        ];
        let mut checked = 0_usize;
        for runner in cases {
            error(
                declarations(&current_profile_bytes(), &runner).map(|_value| ()),
                "invalid_input",
            );
            checked += 1;
        }
        let mut profile: Value = serde_json::from_slice(&current_profile_bytes()).unwrap();
        profile["identity"]["artifact_sha256"] = json!(CANARY);
        error(
            declarations(&serde_json::to_vec(&profile).unwrap(), &valid).map(|_value| ()),
            "invalid_input",
        );
        assert_eq!(checked, 6);
    }

    #[test]
    fn original_four_capture_files_and_both_executables_must_each_remain_unchanged() {
        let before = from_fn(|_index| Digest::try_from(ARTIFACT_DIGEST.to_owned()).unwrap());
        assert_eq!(unchanged(&before, &before), Ok(()));
        let mut checked = 0_usize;
        for index in 0..6 {
            let mut changed = before.clone();
            *changed.get_mut(index).unwrap() = Digest::try_from(CLI_DIGEST.to_owned()).unwrap();
            error(unchanged(&before, &changed), "identity_mismatch");
            checked += 1;
        }
        assert_eq!(checked, 6);
    }

    #[test]
    fn held_work_requires_exactly_two_actual_parked_calls_not_worker_intent() {
        assert_eq!(
            held_stats(&Stats {
                calls: 2,
                current_active_calls: 2,
                ..Stats::default()
            }),
            Ok(())
        );
        let mut checked = 0_usize;
        for (calls, active) in [(0, 0), (1, 1), (2, 1), (3, 2), (2, 0)] {
            error(
                held_stats(&Stats {
                    calls,
                    current_active_calls: active,
                    ..Stats::default()
                }),
                "frozen_probe_failed",
            );
            checked += 1;
        }
        assert_eq!(checked, 5);
    }

    #[test]
    fn overlap_requires_fourteen_distinct_credits_and_sixteen_ordinary_bids_and_calls() {
        let seen = (1..=14_u16).collect::<BTreeSet<_>>();
        let ordinary = outcome(16);
        let stats = Stats {
            calls: 16,
            ..Stats::default()
        };
        assert_eq!(overlap_complete(14, &seen, &ordinary, &stats), Ok(()));
        let mut checked = 0_usize;
        for (parses, versions, bids, calls, category, status) in [
            (13, 14, 16, 16, Category::Success, 200),
            (15, 14, 16, 16, Category::Success, 200),
            (14, 13, 16, 16, Category::Success, 200),
            (14, 15, 16, 16, Category::Success, 200),
            (14, 14, 15, 16, Category::Success, 200),
            (14, 14, 17, 16, Category::Success, 200),
            (14, 14, 16, 17, Category::Success, 200),
            (14, 14, 16, 15, Category::Success, 200),
            (14, 14, 16, 16, Category::Error, 200),
            (14, 14, 16, 16, Category::Success, 503),
        ] {
            let mut observed = outcome(bids);
            observed.category = category;
            observed.status = status;
            error(
                overlap_complete(
                    parses,
                    &(1..=versions).collect(),
                    &observed,
                    &Stats {
                        calls,
                        ..Stats::default()
                    },
                ),
                "frozen_probe_failed",
            );
            checked += 1;
        }
        assert_eq!(checked, 10);
    }

    #[test]
    fn invalid_consent_requires_observed_privacy_denial_no_bids_and_zero_credit() {
        let mut denied = outcome(0);
        denied.auction.privacy_blocked = true;
        denied.auction.privacy_refused = true;
        assert_eq!(invalid_consent(&denied, 0, true), Ok(()));
        let mut not_blocked = denied.clone();
        not_blocked.auction.privacy_blocked = false;
        let mut bids = denied.clone();
        bids.auction.bids = 1;
        let mut failed = denied.clone();
        failed.category = Category::Error;
        let mut status = denied.clone();
        status.status = 503;
        assert_eq!(invalid_consent(&failed, 0, true), Ok(()));
        assert_eq!(invalid_consent(&status, 0, true), Ok(()));
        let cases = [(not_blocked, 0), (bids, 0), (denied, 1)];
        let mut checked = 0_usize;
        for (outcome, credit) in cases {
            error(
                invalid_consent(&outcome, credit, true),
                "frozen_probe_failed",
            );
            checked += 1;
        }
        assert_eq!(checked, 3);
    }

    #[test]
    fn invalid_controls_distinguish_missing_consent_from_malformed_refusal() {
        let mut checked = 0_u8;
        for (observed, expected, passes) in [
            (false, false, true),
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            let mut denied = outcome(0);
            denied.auction.privacy_blocked = true;
            denied.auction.privacy_refused = observed;
            let result = invalid_consent(&denied, 0, expected);
            if passes {
                assert_eq!(result, Ok(()));
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    HarnessError::new("frozen_probe_failed")
                );
            }
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 4);
    }

    #[test]
    fn invalid_control_bodies_carry_the_declared_signal_and_exact_consent_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let proof = consents::fixtures(&root).unwrap().remove(0);
        let core = proof.value().split_once('.').unwrap().0;
        let candidate = inputs::parse_candidate(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/testing/fixtures/resource-qualification/native-axum.json"
        )))
        .unwrap();
        let mut checked = 0_u8;
        for supplied in [None, Some("not-a-TC-string"), Some(core)] {
            let body = invalid_body(candidate.base_load, supplied).unwrap();
            let decoded: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(decoded["regs"]["gdpr"], 1_u8);
            assert_eq!(
                decoded
                    .get("user")
                    .and_then(|user| user.get("consent"))
                    .and_then(Value::as_str),
                supplied
            );
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 3);
    }

    #[test]
    fn recovery_requires_sixteen_bids_and_exactly_thirty_two_total_calls() {
        assert_eq!(recovery_complete(&outcome(16), 32), Ok(()));
        let mut checked = 0_usize;
        for (bids, calls, category, status) in [
            (15, 32, Category::Success, 200),
            (17, 32, Category::Success, 200),
            (16, 31, Category::Success, 200),
            (16, 33, Category::Success, 200),
            (16, 32, Category::Timeout, 200),
            (16, 32, Category::Success, 503),
        ] {
            let mut result = outcome(bids);
            result.category = category;
            result.status = status;
            error(recovery_complete(&result, calls), "frozen_probe_failed");
            checked += 1;
        }
        assert_eq!(checked, 6);
    }
}

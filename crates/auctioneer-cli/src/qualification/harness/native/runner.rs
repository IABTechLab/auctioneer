use super::super::meter::NativeMeter;
use super::super::{
    HarnessError, Result,
    fixture::{Options as FixtureOptions, client::Client},
    io,
    process::{ProcessSpec, Supervisor},
};
use super::{
    consents,
    inputs::{self, Candidate, Catalog, Export},
    probes, scenario,
    snapshots::{self, InstalledVariant},
    system,
};
use crate::qualification::{evaluate, profile, report, serialize};
use crate::qualification::{
    profile::{Digest, Profile},
    report::{Observations, Qualification, Report, ResultStatus},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::env::current_exe;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs,
    path::Path,
};

pub struct Options {
    pub profile: PathBuf,
    pub output: PathBuf,
    pub source_commit: Option<String>,
    pub no_build: bool,
    pub prepare: bool,
}

#[derive(Serialize)]
pub struct Summary {
    category: &'static str,
    exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_sha256: Option<Digest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_acceptance: Option<ResultStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    profile_qualification: Option<Qualification>,
    publisher_pilot_ready: bool,
}

impl Summary {
    pub const fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

struct Inputs {
    candidate: Candidate,
    catalog: Catalog,
    declared: Option<Profile>,
}

struct Prepared {
    repo: PathBuf,
    candidate: Candidate,
    catalog: Catalog,
    binary: PathBuf,
    cli: PathBuf,
    artifact: Digest,
    cli_digest: Digest,
    exported: Export,
    profile: Profile,
    digest: Digest,
    variants: Vec<InstalledVariant>,
    environment: BTreeMap<OsString, OsString>,
    origin: String,
}

#[derive(Serialize)]
struct Failure {
    category: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subcase: Option<profile::Subcase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repetition: Option<u8>,
}

enum Completion {
    Prepared(Digest),
    Measured(Box<Measurement>),
}

struct Measurement {
    profile: Profile,
    digest: Digest,
    observations: Observations,
    cli_digest: Digest,
    failures: Vec<Failure>,
    attestations: Vec<Value>,
    snapshot_probes: Vec<Value>,
}

fn read_inputs(path: &Path, repo: &Path) -> Result<Inputs> {
    let bytes = io::read_bytes(path, 1 << 20_u32)?;
    let value: Value = inputs::parse_json(&bytes, 1 << 20_u32)?;
    let (candidate, declared) =
        if value.get("kind").and_then(Value::as_str) == Some("native_axum_candidate") {
            (inputs::parse_candidate(&bytes)?, None)
        } else {
            let declared =
                profile::parse(&bytes).map_err(|_error| HarnessError::new("invalid_input"))?;
            (
                inputs::parse_candidate(&io::read_bytes(
                    &repo.join("docs/testing/fixtures/resource-qualification/native-axum.json"),
                    1 << 20_u32,
                )?)?,
                Some(declared),
            )
        };
    let catalog = inputs::parse_catalog(&io::read_bytes(
        &repo.join("docs/testing/fixtures/resource-qualification/catalog.json"),
        1 << 20_u32,
    )?)?;
    inputs::validate_candidate(&candidate, &catalog)?;
    Ok(Inputs {
        candidate,
        catalog,
        declared,
    })
}

fn private_directory(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700)
    };
    builder
        .create(path)
        .map_err(|_error| HarnessError::new("output_io"))
}

fn profile_bytes(profile: &Profile) -> Result<Vec<u8>> {
    let mut bytes = serialize(profile).map_err(|_error| HarnessError::new("output_limit"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_preflight(directory: &Path, report: &Report) -> Result<()> {
    private_directory(directory)?;
    io::write_json(
        &directory.join("profile.json"),
        &report.profile,
        256 << 10_u32,
    )?;
    io::write_json(
        &directory.join("observations.json"),
        &report.observations,
        256 << 10_u32,
    )?;
    io::write_json(&directory.join("report.json"), report, 256 << 10_u32)?;
    Ok(())
}

async fn prepare(
    options: &Options,
    repo: PathBuf,
    input: Inputs,
    scratch: &Path,
    supervisor: &Supervisor,
) -> Result<Prepared> {
    let revision =
        system::source_commit(&repo, options.source_commit.as_deref(), supervisor).await?;
    let hardware = system::hardware_identity(&input.candidate.platform, supervisor).await?;
    let artifacts = system::cargo_artifacts(&repo, !options.no_build, supervisor).await?;
    private_directory(&options.output)?;
    let artifact_dir = options.output.join("artifacts");
    private_directory(&artifact_dir)?;
    let binary = artifact_dir.join("auctioneer-adapter-axum");
    let cli = artifact_dir.join("auctioneer-cli");
    let artifact = snapshots::freeze_artifact(&artifacts.axum, &binary)?;
    let cli_digest = snapshots::freeze_artifact(&artifacts.cli, &cli)?;
    let port = system::reserve_port().await?;
    let environment = inputs::service_environment(port, &input.candidate.sdk)?;
    let exported = system::inspect_export(&binary, &environment, scratch, supervisor).await?;
    check_envelope(&options.output, &exported, &input.candidate, &artifact)?;
    let variants = install_variants(&cli, &repo, scratch, &input.candidate, supervisor).await?;
    let identity = profile::Identity {
        auctioneer_commit: revision,
        artifact_sha256: artifact.clone(),
        build_features: exported.build_features.clone(),
        edgezero_pin: exported.edgezero_pin.clone(),
        target: profile::Target::Axum,
        runtime_version: artifacts.runtime_version,
        harness_sha256: system::harness_digest(
            &repo,
            &current_exe().map_err(|_error| HarnessError::new("identity_mismatch"))?,
        )?,
        fixtures_sha256: system::fixtures_digest(&input.candidate, &input.catalog, &repo)?,
        reference_data_sha256: exported.reference_data_sha256.clone(),
        baked_config_sha256: exported.baked_config_sha256.clone(),
    };
    let declared_variants: Vec<_> = variants
        .iter()
        .map(|variant| profile::Variant {
            id: variant.id,
            config_sha256: variant.config_sha256.clone(),
            settings_sha256: variant.settings_sha256.clone(),
            cold_window_ms: input.candidate.cold_window_ms,
            warm_window_ms: input.candidate.warm_window_ms,
            max_cold_peak_bytes: input.candidate.max_peak_bytes,
            max_warm_peak_bytes: input.candidate.max_peak_bytes,
        })
        .collect();
    let profile = inputs::bind_profile(
        &input.candidate,
        &input.catalog,
        &exported,
        identity,
        hardware,
        &declared_variants,
    )?;
    if let Some(declared) = input.declared
        && serde_json::to_value(&declared)
            .map_err(|_error| HarnessError::new("identity_mismatch"))?
            != serde_json::to_value(&profile)
                .map_err(|_error| HarnessError::new("identity_mismatch"))?
    {
        return Err(HarnessError::new("identity_mismatch"));
    }
    let digest = Digest::of(&profile_bytes(&profile)?);
    let report = preflight(profile.clone(), digest.clone())?;
    write_preflight(&options.output.join("preflight"), &report)?;
    io::write_json(
        &options.output.join("profile.json"),
        &profile,
        256 << 10_u32,
    )?;
    Ok(Prepared {
        repo,
        candidate: input.candidate,
        catalog: input.catalog,
        binary,
        cli,
        artifact,
        cli_digest,
        exported,
        profile,
        digest,
        variants,
        environment,
        origin: format!("http://127.0.0.1:{port}"),
    })
}

fn check_envelope(
    output: &Path,
    exported: &Export,
    candidate: &Candidate,
    artifact: &Digest,
) -> Result<()> {
    if exported
        .derived
        .total_bytes
        .checked_add(candidate.headroom_bytes)
        .is_none_or(|total| {
            total
                > candidate
                    .platform
                    .memory_bytes
                    .min(exported.resources.limits.isolate_total_bytes)
        })
    {
        io::write_json(
            &output.join("runner.json"),
            &json!({"schema_version":1_i32,"category":"envelope_over_budget","artifact_sha256":artifact,"accounted_bytes":exported.derived.total_bytes,"platform_bytes":candidate.platform.memory_bytes}),
            256 << 10_u32,
        )?;
        return Err(HarnessError::new("envelope_over_budget"));
    }
    Ok(())
}

async fn install_variants(
    cli: &Path,
    repo: &Path,
    scratch: &Path,
    candidate: &Candidate,
    supervisor: &Supervisor,
) -> Result<Vec<InstalledVariant>> {
    let mut variants = Vec::new();
    for id in 1..=3_u16 {
        let directory = scratch.join(format!("variant-{id}"));
        private_directory(&directory)?;
        variants.push(
            snapshots::install_variant(cli, repo, &directory, candidate, id, supervisor).await?,
        );
    }
    Ok(variants)
}

pub async fn run(options: Options, supervisor: &Supervisor) -> Result<Summary> {
    let repo = system::repository_root()?;
    let inputs = read_inputs(&options.profile, &repo)?;
    supervisor.set_deadline(inputs.candidate.max_run_ms).await?;
    let scratch = tempfile::Builder::new()
        .prefix("auctioneer-qualification-")
        .tempdir()
        .map_err(|_error| HarnessError::new("output_io"))?;
    let result = async {
        let prepared = prepare(&options, repo, inputs, scratch.path(), supervisor).await?;
        if options.prepare {
            return Ok(Completion::Prepared(prepared.digest));
        }
        measure(prepared, scratch.path(), supervisor)
            .await
            .map(|measurement| Completion::Measured(Box::new(measurement)))
    }
    .await;
    let active = match result {
        Ok(completion) => supervisor.check_active().await.map(|()| completion),
        Err(error) => Err(error),
    };
    let cleaned = after_cleanup(active, supervisor.cleanup().await);
    let completion = finish_scratch(scratch, cleaned)?;
    supervisor.check_completion().await?;
    match completion {
        Completion::Prepared(digest) => Ok(Summary {
            category: "prepared",
            exit_code: 0,
            profile_sha256: Some(digest),
            local_acceptance: None,
            profile_qualification: None,
            publisher_pilot_ready: false,
        }),
        Completion::Measured(measurement) => finish(&options.output, *measurement),
    }
}

fn failure(
    error: HarnessError,
    variant_id: Option<u16>,
    scenario: Option<(&profile::Scenario, u8)>,
) -> Failure {
    Failure {
        category: error.code,
        stage: error.stage,
        variant_id,
        subcase: scenario.map(|(row, _repetition)| row.subcase),
        repetition: scenario.map(|(_row, repetition)| repetition),
    }
}

fn fatal(error: HarnessError) -> bool {
    matches!(
        error.code,
        "interrupted" | "run_deadline" | "cleanup_failed"
    )
}

fn after_cleanup<T>(work: Result<T>, cleanup: Result<()>) -> Result<T> {
    if let Err(error) = &work
        && error.code == "cleanup_failed"
    {
        return Err(*error);
    }
    if let Err(error) = cleanup
        && error.code == "cleanup_failed"
    {
        return Err(error);
    }
    if let Err(error) = &work
        && matches!(error.code, "interrupted" | "run_deadline")
    {
        return Err(*error);
    }
    cleanup.and(work)
}

fn finish_scratch(scratch: tempfile::TempDir, outcome: Result<Completion>) -> Result<Completion> {
    if matches!(
        outcome,
        Err(HarnessError {
            code: "cleanup_failed",
            ..
        })
    ) {
        let _retained = scratch.keep();
        return outcome;
    }
    drop(scratch);
    outcome
}

fn service_spec(prepared: &Prepared, variant: &InstalledVariant) -> ProcessSpec {
    let mut spec = ProcessSpec::new(&prepared.binary);
    spec.cwd = Some(variant.dir.clone());
    spec.env = prepared.environment.clone();
    spec.timeout_ms = prepared.candidate.max_run_ms;
    spec
}

async fn observe_scenarios(
    prepared: &Prepared,
    variant: &InstalledVariant,
    fixture: &mut Client,
    meter: &NativeMeter,
    consents: &[consents::Consent],
    supervisor: &Supervisor,
    failures: &mut Vec<Failure>,
) -> Result<Vec<scenario::Measured>> {
    let mut measured = Vec::new();
    let mut parsed_versions = BTreeSet::new();
    for scenario in prepared
        .profile
        .scenarios
        .iter()
        .filter(|row| row.variant_id == variant.id)
    {
        let row = prepared
            .catalog
            .cases
            .iter()
            .find(|row| row.id == scenario.fixture.id)
            .ok_or(HarnessError::new("invalid_input"))?;
        for repetition in 1..=scenario.repetitions {
            supervisor.check_active().await?;
            meter.assert_running().await?;
            let result = Box::pin(scenario::run(scenario::Context {
                candidate: &prepared.candidate,
                scenario,
                row,
                repetition,
                origin: &prepared.origin,
                fixture,
                meter,
                variant,
                resources: &prepared.exported.resources,
                supervisor,
                consents,
                parsed_versions: &mut parsed_versions,
            }))
            .await;
            match result {
                Ok(observation) => measured.push(observation),
                Err(error) => {
                    failures.push(failure(
                        error,
                        Some(variant.id),
                        Some((scenario, repetition)),
                    ));
                    if fatal(error) {
                        return Err(error);
                    }
                }
            }
        }
    }
    Ok(measured)
}

async fn run_variant(
    prepared: &Prepared,
    variant: &InstalledVariant,
    fixture: &mut Client,
    consents: &[consents::Consent],
    supervisor: &Supervisor,
    failures: &mut Vec<Failure>,
) -> Result<(report::VariantObservation, Vec<scenario::Measured>)> {
    snapshots::assert_artifact(&prepared.binary, &prepared.artifact)?;
    snapshots::assert_artifact(&prepared.cli, &prepared.cli_digest)?;
    system::assert_same_export(
        &prepared.exported,
        &system::inspect_export(
            &prepared.binary,
            &prepared.environment,
            &variant.dir,
            supervisor,
        )
        .await?,
    )?;
    let before = snapshots::read_snapshot(&variant.dir)?;
    if before.config_sha256 != variant.config_sha256 {
        return Err(HarnessError::new("identity_mismatch"));
    }
    let mut meter = NativeMeter::start(service_spec(prepared, variant), supervisor).await?;
    let result = async {
        let (cold, ()) = meter
            .window(
                prepared.candidate.cold_window_ms,
                probes::wait_ready(
                    &meter,
                    &prepared.origin,
                    probes::Ready::Snapshot(variant),
                    supervisor,
                ),
            )
            .await?;
        probes::observe_settings(
            &prepared.origin,
            variant,
            &prepared.candidate,
            fixture,
            supervisor,
        )
        .await?;
        let (warm, ()) = meter
            .window(prepared.candidate.warm_window_ms, async { Ok(()) })
            .await?;
        let measured = observe_scenarios(
            prepared, variant, fixture, &meter, consents, supervisor, failures,
        )
        .await?;
        let after = snapshots::read_snapshot(&variant.dir)?;
        Ok((cold, warm, measured, after))
    }
    .await;
    let stopped = meter.stop().await;
    let (cold, warm, mut measured, after) = after_cleanup(result, stopped)?;
    for row in &mut measured {
        row.observation.memory = Some(meter.memory(&row.window)?);
    }
    let startup_sha256=Digest::canonical(&json!({"artifact_sha256":prepared.artifact,"snapshot":before.config_sha256,"settings":variant.settings_sha256,"process_id":meter.pid(),"variant_id":variant.id,
        "started_nonce":SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_error|HarnessError::new("identity_unsupported"))?.as_nanos().to_string()})).map_err(|_error|HarnessError::new("identity_mismatch"))?;
    let observation = report::VariantObservation {
        cold_memory: Some(meter.memory(&cold)?),
        warm_memory: Some(meter.memory(&warm)?),
        id: variant.id,
        process_id: meter.pid(),
        startup_sha256,
        config_sha256: after.config_sha256.clone(),
        startup_snapshot_sha256: before.config_sha256.clone(),
        settings_sha256: variant.settings_sha256.clone(),
        restarted: true,
        input_immutable: before.store_sha256 == after.store_sha256,
        effective_settings_observed: probes::snapshot_observed(
            variant,
            meter.pid(),
            &before,
            &after,
            &variant.settings_sha256,
            true,
            true,
        ),
        cold_window_ms: cold.elapsed_ms,
        warm_window_ms: warm.elapsed_ms,
    };
    Ok((observation, measured))
}

async fn start_fixture(prepared: &Prepared, supervisor: &Supervisor) -> Result<Client> {
    Client::start(
        &prepared.cli,
        &FixtureOptions::Native {
            port: 8787,
            body_cap: usize::try_from(
                prepared
                    .exported
                    .resources
                    .limits
                    .max_outbound_request_bytes
                    .min(1 << 20_u64),
            )
            .map_err(|_error| HarnessError::new("invalid_input"))?,
            max_requests: 100_000,
            max_sockets: 16,
        },
        supervisor,
    )
    .await
}

async fn measure(
    prepared: Prepared,
    scratch: &Path,
    supervisor: &Supervisor,
) -> Result<Measurement> {
    let mut fixture = start_fixture(&prepared, supervisor).await?;
    let result = async {
        let consents = consents::fixtures(&prepared.repo)?;
        let mut failures = Vec::new();
        let mut observations = empty_observations(&prepared.profile, prepared.digest.clone())?;
        let snapshot_probes = match Box::pin(probes::run(probes::Context {
            cli: &prepared.cli,
            binary: &prepared.binary,
            artifact: &prepared.artifact,
            repo: &prepared.repo,
            candidate: &prepared.candidate,
            scratch,
            fixture: &mut fixture,
            export: &prepared.exported,
            supervisor,
        }))
        .await
        {
            Ok(probes) => probes,
            Err(error) => {
                failures.push(failure(error, None, None));
                if fatal(error) {
                    return Err(error);
                }
                Vec::new()
            }
        };
        let mut attestations = Vec::new();
        for variant in &prepared.variants {
            supervisor.check_active().await?;
            match run_variant(
                &prepared,
                variant,
                &mut fixture,
                &consents,
                supervisor,
                &mut failures,
            )
            .await
            {
                Ok((observation, measured)) => {
                    let process_id = observation.process_id;
                    observations.variants.push(observation);
                    for row in measured {
                        attestations.push(json!({
                            "subcase": row.observation.subcase,
                            "variant_id": variant.id,
                            "repetition": row.observation.repetition,
                            "process_id": process_id,
                            "work": row.attestation,
                            "rss_samples": row.window.samples,
                            "sampled_peak_bytes": row.window.sampled_peak_bytes,
                        }));
                        observations.scenarios.push(row.observation);
                    }
                }
                Err(error) => {
                    failures.push(failure(error, Some(variant.id), None));
                    if fatal(error) {
                        return Err(error);
                    }
                }
            }
        }
        if snapshot_probes.is_empty() {
            for row in &mut observations.scenarios {
                if matches!(
                    row.subcase,
                    profile::Subcase::ConfigSnapshot
                        | profile::Subcase::ConfigVariant
                        | profile::Subcase::ConfigRestoration
                ) {
                    row.recovered = false;
                }
            }
        }
        Ok((observations, failures, attestations, snapshot_probes))
    }
    .await;
    fixture.stop().await?;
    let (observations, failures, attestations, snapshot_probes) = result?;
    snapshots::assert_artifact(&prepared.binary, &prepared.artifact)?;
    snapshots::assert_artifact(&prepared.cli, &prepared.cli_digest)?;
    system::source_commit(
        &prepared.repo,
        Some(prepared.profile.identity.auctioneer_commit.as_str()),
        supervisor,
    )
    .await?;
    Ok(Measurement {
        profile: prepared.profile,
        digest: prepared.digest,
        observations,
        cli_digest: prepared.cli_digest,
        failures,
        attestations,
        snapshot_probes,
    })
}

fn finish(output: &Path, measurement: Measurement) -> Result<Summary> {
    let Measurement {
        profile,
        digest,
        observations,
        cli_digest,
        failures,
        attestations,
        snapshot_probes,
    } = measurement;
    verify_profile_file(output, &profile, &digest)?;
    let bytes = serialize(&observations).map_err(|_error| HarnessError::new("output_limit"))?;
    let checked = report::parse(&bytes).map_err(|_error| HarnessError::new("evaluator_failed"))?;
    let report = evaluate::evaluate(profile, checked, digest)
        .map_err(|_error| HarnessError::new("evaluator_failed"))?;
    io::write_json(
        &output.join("observations.json"),
        &report.observations,
        256 << 10_u32,
    )?;
    io::write_json(&output.join("report.json"), &report, 256 << 10_u32)?;
    verify_profile_file(output, &report.profile, &report.profile_sha256)?;
    let evidence = json!({"profile":report.profile_sha256,"observations":Digest::of(&io::read_bytes(&output.join("observations.json"),1<<20_u32)?),"report":Digest::of(&io::read_bytes(&output.join("report.json"),1<<20_u32)?)});
    io::write_json(
        &output.join("runner.json"),
        &json!({"schema_version":1_i32,"category":"evaluated","cli_sha256":cli_digest,"evidence_sha256":evidence,"failures":failures,"attestations":attestations,"snapshot_probes":snapshot_probes}),
        256 << 10_u32,
    )?;
    Ok(Summary {
        category: "evaluated",
        exit_code: qualification_exit(report.profile_qualification),
        profile_sha256: Some(report.profile_sha256),
        local_acceptance: Some(report.local_acceptance),
        profile_qualification: Some(report.profile_qualification),
        publisher_pilot_ready: report.publisher_pilot_ready,
    })
}

fn verify_profile_file(output: &Path, profile: &Profile, digest: &Digest) -> Result<()> {
    let stored = io::read_bytes(&output.join("profile.json"), 256 << 10_u32)
        .map_err(|_error| HarnessError::new("identity_mismatch"))?;
    if Digest::of(&stored) != *digest || Digest::of(&profile_bytes(profile)?) != *digest {
        return Err(HarnessError::new("identity_mismatch"));
    }
    Ok(())
}

fn empty_observations(profile: &Profile, digest: Digest) -> Result<Observations> {
    let observations = Observations {
        schema_version: profile::RESOURCE_SCHEMA_VERSION,
        profile_sha256: digest,
        identity: profile.identity.clone(),
        resources: profile.resources.clone(),
        hardware: profile.hardware.clone(),
        platform: profile.platform.clone(),
        backend: profile.backend,
        variants: Vec::new(),
        capabilities: profile::REQUIRED_CAPABILITIES
            .iter()
            .map(|capability| report::CapabilityEvidence {
                capability: *capability,
                status: report::EvidenceStatus::Pending,
                evidence_sha256: None,
            })
            .collect(),
        scenarios: Vec::new(),
    };
    let bytes = serialize(&observations).map_err(|_error| HarnessError::new("evaluator_failed"))?;
    report::parse(&bytes).map_err(|_error| HarnessError::new("evaluator_failed"))
}

fn preflight(profile: Profile, digest: Digest) -> Result<Report> {
    profile
        .validate()
        .map_err(|_error| HarnessError::new("evaluator_failed"))?;
    let observations = empty_observations(&profile, digest.clone())?;
    let report = evaluate::evaluate(profile, observations, digest)
        .map_err(|_error| HarnessError::new("evaluator_failed"))?;
    if !report.accounted.fits {
        return Err(HarnessError::new("envelope_over_budget"));
    }
    Ok(report)
}

const fn qualification_exit(qualification: Qualification) -> i32 {
    match qualification {
        Qualification::Qualified => 0,
        Qualification::Nonqualified | Qualification::Pending | Qualification::Unsupported => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::report::EvidenceStatus;

    fn profile() -> Profile {
        profile::tests::synthetic_profile(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/testing/fixtures/resource-qualification/native-axum-local-82075f1.json"
        )))
    }

    #[test]
    fn measurement_abort_codes_are_fatal_independent_of_error_stage() {
        for (code, expected) in [
            ("interrupted", true),
            ("run_deadline", true),
            ("cleanup_failed", true),
            ("fixture_failed", false),
            ("meter_failed", false),
            ("invalid_child_output", false),
        ] {
            for stage in [None, Some("scenario")] {
                assert_eq!(
                    fatal(HarnessError { code, stage }),
                    expected,
                    "{code}/{stage:?}"
                );
            }
        }
    }

    #[test]
    fn empty_preflight_contains_no_measurements_or_sdk_qualification_credit() {
        let profile = profile();
        let digest = Digest::of(b"predeclared-profile");
        let observations = empty_observations(&profile, digest.clone()).unwrap();
        assert_eq!(observations.schema_version, 2);
        assert_eq!(observations.profile_sha256, digest);
        assert_eq!(observations.identity, profile.identity);
        assert_eq!(observations.resources, profile.resources);
        assert_eq!(observations.hardware, profile.hardware);
        assert_eq!(observations.platform, profile.platform);
        assert!(observations.variants.is_empty());
        assert!(observations.scenarios.is_empty());
        assert_eq!(observations.capabilities.len(), 6);
        for row in &observations.capabilities {
            assert_eq!(row.status, EvidenceStatus::Pending);
            assert!(row.evidence_sha256.is_none());
        }
    }

    #[test]
    fn real_evaluator_refuses_an_over_budget_empty_preflight_without_changing_criteria() {
        let original = profile();
        let valid = preflight(original.clone(), Digest::of(b"valid")).unwrap();
        assert!(valid.accounted.fits);
        assert_ne!(valid.profile_qualification, Qualification::Qualified);
        assert!(!valid.publisher_pilot_ready);
        let mut oversized = original;
        oversized.resources.limits.baseline_bytes = oversized.resources.limits.isolate_total_bytes;
        let digest = Digest::canonical(&oversized.resources).unwrap();
        for row in &mut oversized.scenarios {
            if let Some(debug) = &mut row.debug_workload {
                debug.reference.resources_sha256 = digest.clone();
                debug.reference_sha256 = Digest::canonical(&debug.reference).unwrap();
            }
        }
        oversized.validate().unwrap();
        assert_eq!(
            preflight(oversized, Digest::of(b"too-large")).unwrap_err(),
            HarnessError::new("envelope_over_budget")
        );
    }

    #[test]
    fn only_qualified_results_have_a_zero_measurement_exit_code() {
        for (qualification, expected) in [
            (Qualification::Qualified, 0_i32),
            (Qualification::Nonqualified, 1_i32),
            (Qualification::Pending, 1_i32),
            (Qualification::Unsupported, 1_i32),
        ] {
            assert_eq!(qualification_exit(qualification), expected);
        }
    }

    #[test]
    fn preflight_profile_file_bytes_match_the_digest_recorded_in_both_evidence_documents() {
        let profile = profile();
        let digest = Digest::of(&profile_bytes(&profile).unwrap());
        let report = preflight(profile, digest.clone()).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("preflight");
        write_preflight(&directory, &report).unwrap();
        let actual = Digest::of(&fs::read(directory.join("profile.json")).unwrap());
        assert_eq!(actual, digest);
        assert_eq!(report.observations.profile_sha256, actual);
        let stored: Value =
            serde_json::from_slice(&fs::read(directory.join("report.json")).unwrap()).unwrap();
        assert_eq!(
            stored["profile_sha256"],
            serde_json::to_value(actual).unwrap()
        );
    }

    #[test]
    fn final_publication_rejects_changed_predeclarations_before_writing_any_evaluated_files() {
        let original = profile();
        let bytes = profile_bytes(&original).unwrap();
        let digest = Digest::of(&bytes);
        let mut checked = 0_u8;
        for changed in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let retained = if changed {
                b"{}\n".to_vec()
            } else {
                bytes.clone()
            };
            fs::write(directory.path().join("profile.json"), &retained).unwrap();
            let measurement = Measurement {
                profile: original.clone(),
                digest: digest.clone(),
                observations: empty_observations(&original, digest.clone()).unwrap(),
                cli_digest: Digest::of(b"frozen-cli"),
                failures: Vec::new(),
                attestations: Vec::new(),
                snapshot_probes: Vec::new(),
            };
            let result = finish(directory.path(), measurement);
            if changed {
                assert_eq!(
                    result.err().unwrap(),
                    HarnessError::new("identity_mismatch")
                );
                for name in ["observations.json", "report.json", "runner.json"] {
                    assert!(!directory.path().join(name).exists(), "{name}");
                }
            } else {
                assert_eq!(result.unwrap().category, "evaluated");
                let report: Value = serde_json::from_slice(
                    &fs::read(directory.path().join("report.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    report["profile_sha256"],
                    serde_json::to_value(&digest).unwrap()
                );
                let runner: Value = serde_json::from_slice(
                    &fs::read(directory.path().join("runner.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    runner["evidence_sha256"]["profile"],
                    serde_json::to_value(&digest).unwrap()
                );
            }
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 2);
    }

    #[test]
    fn cleanup_and_cancellation_failures_cannot_be_masked_by_a_later_meter_read_failure() {
        let mut checked = 0_u8;
        for (work, cleanup, expected) in [
            (
                Err(HarnessError::new("cleanup_failed")),
                Err(HarnessError::new("meter_failed")),
                Err(HarnessError::new("cleanup_failed")),
            ),
            (
                Err(HarnessError::new("interrupted")),
                Err(HarnessError::new("meter_failed")),
                Err(HarnessError::new("interrupted")),
            ),
            (
                Err(HarnessError::new("run_deadline")),
                Err(HarnessError::new("meter_failed")),
                Err(HarnessError::new("run_deadline")),
            ),
            (
                Err(HarnessError::new("interrupted")),
                Err(HarnessError::new("cleanup_failed")),
                Err(HarnessError::new("cleanup_failed")),
            ),
            (
                Ok(7_u8),
                Err(HarnessError::new("meter_failed")),
                Err(HarnessError::new("meter_failed")),
            ),
            (
                Err(HarnessError::new("fixture_failed")),
                Ok(()),
                Err(HarnessError::new("fixture_failed")),
            ),
            (Ok(7_u8), Ok(()), Ok(7_u8)),
        ] {
            assert_eq!(after_cleanup(work, cleanup), expected);
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 7);
    }

    #[test]
    fn a_previous_cleanup_failure_retains_scratch_even_after_a_successful_retry() {
        let mut checked = 0_u8;
        for code in ["cleanup_failed", "meter_failed", "interrupted"] {
            let scratch = tempfile::tempdir().unwrap();
            let path = scratch.path().to_owned();
            fs::write(path.join("private-writer-state"), b"retained").unwrap();
            let result = finish_scratch(scratch, Err(HarnessError::new(code)));
            let kept = path.join("private-writer-state").is_file();
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            assert_eq!(result.err().unwrap(), HarnessError::new(code));
            assert_eq!(kept, code == "cleanup_failed");
            checked = checked.checked_add(1).unwrap();
        }
        assert_eq!(checked, 3);
    }
}

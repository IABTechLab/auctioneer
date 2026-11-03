//! The type `config push` / `config diff` / `config validate` pass as
//! `C` to `EdgeZero`'s typed config entry points.
//!
//! `run_config_*_typed::<C>(args)` takes only command args and
//! deserializes the file ROOT itself, so there is no projection
//! parameter to hand it. But the pushed payload is
//! `serde_json::to_value(typed)` — whatever `Serialize` emits — so the
//! projection happens INSIDE `C`. [`AuctioneerPushSource`] deserializes
//! the combined `auctioneer.toml` root and serializes ONLY the `[runtime]`
//! subtree, which is what makes the pushed envelope's `data` identical
//! to what the pre-fold `auctioneer.toml` produced. The `[baked.*]` tables
//! ride along as opaque `toml::Value`s and are dropped on the way out.
//!
//! `Serialize`, `Validate` and `AppConfigMeta` are all HAND-WRITTEN.
//! Deriving `edgezero_core::AppConfig` would generate its own
//! `AppConfigMeta` (conflicting with a manual one), and reaching the
//! runtime secrets through it would need `#[app_config(nested)]`,
//! which prefixes every secret path with `runtime.` — a prefix the
//! deployed blob does not have.

use auctioneer_config::AuctioneerConfigSource;
use auctioneer_core::config::{
    AuctioneerAppConfig, EffectiveConfigError, SdkAuctioneerAppConfig, validate_effective_defaults,
};
use edgezero_core::app_config::{AppConfigMeta, SecretField};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use validator::{Validate, ValidationError, ValidationErrors};

/// `auctioneer.toml` seen from the push side: `[baked.host]` and
/// `[baked.account]` opaque, `[runtime]` typed.
///
/// A typo inside `[baked.host]` is caught at BUILD, not here: the CLI
/// cannot see `HostDefaultRaw` / `AccountRaw` without the dependency
/// cycle those build-script-only types exist to avoid. What this side
/// still enforces is that both namespaces are PRESENT, which the
/// strict root does.
#[derive(Debug)]
pub struct AuctioneerPushSource(
    AuctioneerConfigSource<toml::Value, toml::Value, AuctioneerAppConfig>,
);

#[cfg(test)]
impl AuctioneerPushSource {
    /// The `[runtime]` subtree — the only part that reaches the store.
    ///
    /// Test-only: production code never reads it directly, because
    /// `Serialize` is what hands `[runtime]` to the push pipeline. The
    /// overlay tests need to see the resolved value rather than its
    /// serialised form.
    fn runtime(&self) -> &AuctioneerAppConfig {
        &self.0.runtime
    }
}

impl<'de> Deserialize<'de> for AuctioneerPushSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        AuctioneerConfigSource::deserialize(deserializer).map(Self)
    }

    /// Forwarded to the inner root so the wrapper reuses whatever
    /// in-place strategy `AuctioneerConfigSource` has, rather than
    /// silently falling back to a fresh allocation.
    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        AuctioneerConfigSource::deserialize_in_place(deserializer, &mut place.0)
    }
}

/// Emits ONLY `[runtime]`, so `build_config_envelope` wraps exactly the
/// document the pre-fold `auctioneer.toml` pushed. The blob schema is
/// unchanged and no deployed blob is invalidated.
impl Serialize for AuctioneerPushSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.runtime.serialize(serializer)
    }
}

/// Delegates to the runtime overlay: the baked tables were validated at
/// build time by the structs that own them.
impl Validate for AuctioneerPushSource {
    fn validate(&self) -> Result<(), ValidationErrors> {
        self.0.runtime.validate()?;
        validate_effective_defaults(&self.0.runtime)
            .map_err(|detail| effective_config_errors(&detail))
    }
}

fn effective_config_errors(detail: &EffectiveConfigError) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    let code = match detail {
        EffectiveConfigError::Registry(_) => "registry_build_failed",
        EffectiveConfigError::Malformed(_) => "invalid_effective_config",
    };
    let mut error = ValidationError::new(code);
    error.message = Some(detail.to_string().into());
    errors.add("runtime", error);
    errors
}

/// Hand-written so secret paths match the DEPLOYED blob exactly, with
/// no `runtime.` prefix: the blob's root is the runtime table, so the
/// paths the runtime walk resolves are `SdkAuctioneerAppConfig`'s own.
impl AppConfigMeta for AuctioneerPushSource {
    fn secret_fields() -> Vec<SecretField> {
        SdkAuctioneerAppConfig::secret_fields()
    }
}

#[cfg(test)]
mod tests {
    use super::AuctioneerPushSource;
    use auctioneer_core::config::AuctioneerAppConfig;
    use edgezero_core::app_config::{AppConfigLoadOptions, load_app_config_with_options};
    use edgezero_core::blob_envelope::BlobEnvelope;
    use std::path::PathBuf;
    use std::process::{Command, id as process_id};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::{env, fs};
    use validator::Validate as _;

    #[test]
    fn effective_config_failures_keep_registry_rebuilds_distinct_from_runtime_repairs() {
        use auctioneer_core::config::{EffectiveConfigError, RegistryBuildError};

        let registry = super::effective_config_errors(&EffectiveConfigError::Registry(
            RegistryBuildError::MissingBinding {
                bidder: "fake_missing",
            },
        ));
        assert_eq!(
            registry.field_errors()["runtime"][0].code,
            "registry_build_failed"
        );
        let malformed = super::effective_config_errors(&EffectiveConfigError::Malformed(
            "fake_invalid_runtime".to_owned(),
        ));
        assert_eq!(
            malformed.field_errors()["runtime"][0].code,
            "invalid_effective_config"
        );
    }

    /// The shipped `auctioneer.toml` — the document `config push` actually
    /// reads. Testing against the real file rather than a hand-written
    /// fixture is the point: a fold that broke the projection would
    /// have to break it here.
    const REFERENCE: &str = include_str!("../../../auctioneer.toml");

    /// The `[runtime]` subtree alone, spelled the way the PRE-FOLD
    /// `auctioneer.toml` spelled it — four empty tables at the document
    /// root. That is the exact input today's `config push` typed and
    /// serialised, so it is the yardstick for "the envelope did not
    /// change".
    const PRE_FOLD_RUNTIME: &str = "[auction]\n[targeting]\n[status]\n[features]\n";

    /// The env var the two overlay cases drive. Under the namespaced
    /// layout the path carries `RUNTIME` because the wrapper
    /// deserializes the COMBINED root, not the runtime table alone.
    const TMAX_ENV_VAR: &str = "AUCTIONEER__RUNTIME__AUCTION__DEFAULT_TMAX_MS";

    /// What the env var is set to in the child, and what the live-key
    /// case must observe.
    const TMAX_ENV_VALUE: &str = "1500";

    /// Fixture whose `default_tmax_ms` is LIVE, so the overlay has a
    /// leaf to coerce.
    const LIVE_KEY_DOC: &str =
        "[baked.host]\n[baked.account]\n[runtime.auction]\ndefault_tmax_ms = 1000\n";

    /// The same fixture with the key COMMENTED OUT — the shape the
    /// shipped `auctioneer.toml` ships.
    const ABSENT_KEY_DOC: &str =
        "[baked.host]\n[baked.account]\n[runtime.auction]\n# default_tmax_ms = 1000\n";

    static FIXTURE_NONCE: AtomicU64 = AtomicU64::new(0);

    /// Write `body` outside the source checkout. Cargo's target path can
    /// be relative to a different working directory or set only through
    /// command-line/config options, so use the OS temporary directory.
    /// Unique per call and process so concurrent runs cannot collide.
    fn fixture_file(label: &str, body: &str) -> PathBuf {
        let nonce = FIXTURE_NONCE.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir()
            .join("auctioneer_config_data_source_fixtures")
            .join(format!("{label}_{pid}_{nonce}", pid = process_id()));
        fs::create_dir_all(&dir).expect("create fixture dir");
        let path = dir.join("auctioneer.toml");
        fs::write(&path, body).expect("write fixture");
        path
    }

    #[test]
    fn config_fixtures_are_independent_of_cargo_output_paths() {
        let path = fixture_file("temporary_directory", LIVE_KEY_DOC);
        assert!(path.starts_with(env::temp_dir()));
        assert_eq!(fs::read_to_string(&path).unwrap(), LIVE_KEY_DOC);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    /// `build_config_envelope` is private to `edgezero-cli`, so the
    /// tests rebuild what it does: `serde_json::to_value(typed)` inside
    /// a `BlobEnvelope`. That is the whole of the payload contract.
    fn envelope_of<C>(typed: &C) -> BlobEnvelope
    where
        C: serde::Serialize,
    {
        let data = serde_json::to_value(typed).expect("typed config serialises");
        BlobEnvelope::new(data, "2026-01-01T00:00:00Z".to_owned())
    }

    /// Env var naming the file a child probe writes its answer to.
    /// A file rather than stdout because the workspace denies
    /// `clippy::print_stdout`, and libtest's own capture would swallow
    /// it in some configurations anyway.
    const PROBE_OUT_VAR: &str = "AUCTIONEER__TEST__PROBE_OUT";

    /// Re-run THIS test binary as a child with `TMAX_ENV_VAR` set, and
    /// return what the named inner case wrote to its answer file.
    ///
    /// The overlay reads the PROCESS environment, and `set_var` is
    /// `unsafe` — which this workspace denies outright and which would
    /// be racy across libtest's threads regardless. A child process
    /// gives the overlay a real environment to read without either
    /// problem. The inner cases are `#[ignore]`d so an ordinary run
    /// never executes them with the variable absent.
    fn env_overlay_probe(label: &str, inner_test: &str) -> String {
        let answer = fixture_file(label, "").with_file_name("probe_answer.txt");
        let exe = env::current_exe().expect("the test binary knows its own path");
        let output = Command::new(exe)
            .args(["--exact", inner_test, "--ignored"])
            .env(TMAX_ENV_VAR, TMAX_ENV_VALUE)
            .env(PROBE_OUT_VAR, &answer)
            .output()
            .expect("re-spawn the test binary");
        assert!(
            output.status.success(),
            "the child probe `{inner_test}` failed.\nstderr:\n{stderr}",
            stderr = String::from_utf8_lossy(&output.stderr),
        );
        fs::read_to_string(&answer).expect("the child probe must write its answer file")
    }

    /// Load a fixture document through `EdgeZero`'s env-aware loader and
    /// record the resolved `default_tmax_ms` for the parent to read.
    /// Shared by the two `#[ignore]`d probes below.
    fn record_resolved_tmax(label: &str, body: &str) {
        let path = fixture_file(label, body);
        let source = load_app_config_with_options::<AuctioneerPushSource>(
            &path,
            "auctioneer",
            &AppConfigLoadOptions::default(),
        )
        .expect("the fixture document must load");
        let answer = match source.runtime().auction.default_tmax_ms {
            Some(value) => format!("RESOLVED_TMAX=Some({value})"),
            None => "RESOLVED_TMAX=None".to_owned(),
        };
        let out = env::var(PROBE_OUT_VAR).expect("the parent sets the answer-file path");
        fs::write(out, answer).expect("write the probe answer file");
    }

    #[test]
    fn pushed_blob_is_the_runtime_table_only() {
        let source: AuctioneerPushSource =
            toml::from_str(REFERENCE).expect("the shipped auctioneer.toml must deserialise");
        let data = serde_json::to_value(&source).expect("the push wrapper serialises");

        let object = data.as_object().expect("the blob's root is a JSON object");
        assert!(
            !object.contains_key("baked"),
            "the pushed blob must carry NO `baked` key — the baked layer is \
             compiled in, not pushed. Got keys: {:?}",
            object.keys().collect::<Vec<_>>(),
        );
        assert!(
            !object.contains_key("runtime"),
            "the blob's root IS the runtime table; a `runtime` key would mean the \
             wrapper wrapped instead of projecting. Got keys: {:?}",
            object.keys().collect::<Vec<_>>(),
        );
        for expected in ["auction", "targeting", "status", "features"] {
            assert!(
                object.contains_key(expected),
                "the blob must still carry `{expected}` at its root, exactly as the \
                 pre-fold document did",
            );
        }
    }

    #[test]
    fn pushed_envelope_data_matches_todays() {
        // `generated_at` changes on every push and is deliberately NOT
        // part of the SHA, so the comparison is `data` + `sha256` —
        // never literal envelope bytes.
        let folded: AuctioneerPushSource =
            toml::from_str(REFERENCE).expect("the shipped auctioneer.toml must deserialise");
        let pre_fold: AuctioneerAppConfig =
            toml::from_str(PRE_FOLD_RUNTIME).expect("the pre-fold document must deserialise");

        let after = envelope_of(&folded);
        let before = envelope_of(&pre_fold);

        assert_eq!(
            after.data, before.data,
            "the fold must not change the pushed payload by one byte, or every \
             deployed blob's meaning changes with it",
        );
        assert_eq!(
            after.sha256, before.sha256,
            "same data must hash the same; a differing SHA would mean the runtime \
             sees a changed blob after a no-op config change",
        );
    }

    #[test]
    fn the_wrapper_requires_both_namespaces() {
        // The root is strict on both levels, so a document missing
        // either namespace is a push-time refusal rather than a
        // silently-empty baked layer.
        toml::from_str::<AuctioneerPushSource>("[runtime]\n")
            .expect_err("a document with no `[baked]` must be rejected");
        toml::from_str::<AuctioneerPushSource>("[baked.host]\n[baked.account]\n")
            .expect_err("a document with no `[runtime]` must be rejected");
        toml::from_str::<AuctioneerPushSource>("[baked.host]\n[runtime]\n")
            .expect_err("`[baked.account]` is required too");
    }

    #[test]
    fn a_typo_in_a_root_namespace_is_rejected_at_push() {
        let typo = REFERENCE.replace("\n[runtime]\n", "\n[runtim]\n[runtime]\n");
        let err = toml::from_str::<AuctioneerPushSource>(&typo)
            .expect_err("an unknown root namespace must not sail past unread");
        assert!(
            err.to_string().contains("unknown field"),
            "the refusal should be serde's unknown-field error. Got: {err}",
        );
    }

    #[test]
    fn push_validates_effective_sync_config_and_projects_only_runtime() {
        let valid = "[baked.host]\n[baked.account]\n[runtime.host_cookie]\nmax_cookie_size_bytes = 0\n[runtime.user_sync]\npriority_groups = []\n";
        let source: AuctioneerPushSource = toml::from_str(valid).expect("valid source parses");
        source
            .validate()
            .expect("valid merged config passes push validation");
        let data = serde_json::to_value(&source).expect("push data serializes");
        assert_eq!(data["host_cookie"]["max_cookie_size_bytes"], 0_i32);
        assert_eq!(data["user_sync"]["priority_groups"], serde_json::json!([]));
        assert!(data.get("runtime").is_none());
        assert!(data.get("baked").is_none());

        let invalid =
            "[baked.host]\n[baked.account]\n[runtime.host_cookie]\ncookie_name = 'uids'\n";
        let invalid_source: AuctioneerPushSource =
            toml::from_str(invalid).expect("invalid value parses");
        assert!(
            invalid_source.validate().is_err(),
            "push rejects a merged cookie invariant"
        );
    }

    #[test]
    fn env_overlay_reaches_a_live_runtime_key() {
        let answer =
            env_overlay_probe("live_probe", "config_source::tests::live_runtime_key_probe");
        assert!(
            answer == "RESOLVED_TMAX=Some(1500)",
            "an env var must reach a `[runtime]` key the file already sets, and \
             the path must carry the `RUNTIME` namespace segment. Got: {answer}",
        );
    }

    #[test]
    fn absent_runtime_key_is_not_env_overridable() {
        // THE DOCUMENTED LIMITATION, pinned rather than worked around.
        // The overlay only overrides keys already present in the parsed
        // tree, so a commented-out `[runtime]` key stays absent no
        // matter what the environment says. Shipping live `[runtime]`
        // defaults to "fix" this would PIN every such value and stop
        // the blob inheriting future baked defaults — the opposite of
        // the partial-overlay property the fold rests on. An operator
        // who wants the variable to bite uncomments the line first,
        // which is a deliberate, visible choice to pin.
        let answer = env_overlay_probe(
            "absent_probe",
            "config_source::tests::absent_runtime_key_probe",
        );
        assert!(
            answer == "RESOLVED_TMAX=None",
            "a commented-out `[runtime]` key is NOT env-overridable, and no \
             wording anywhere may imply otherwise. Got: {answer}",
        );
    }

    /// Child-process half of `env_overlay_reaches_a_live_runtime_key`.
    /// `#[ignore]` so it only runs when the parent spawns it with the
    /// env var set.
    #[test]
    #[ignore = "spawned by env_overlay_reaches_a_live_runtime_key with the env var set"]
    fn live_runtime_key_probe() {
        record_resolved_tmax("live_key", LIVE_KEY_DOC);
    }

    /// Child-process half of `absent_runtime_key_is_not_env_overridable`.
    #[test]
    #[ignore = "spawned by absent_runtime_key_is_not_env_overridable with the env var set"]
    fn absent_runtime_key_probe() {
        record_resolved_tmax("absent_key", ABSENT_KEY_DOC);
    }
}

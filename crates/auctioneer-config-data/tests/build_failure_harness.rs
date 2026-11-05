//! End-to-end build-script failure harness. Proves the full codegen
//! path actually rejects bad TOML by spawning a child `cargo build`
//! against this crate after pointing `AUCTIONEER__BIDDERS__DIR` at a
//! tempdir that contains the planted bad fixture.
//!
//! The product defaults are covered the same way:
//! `AUCTIONEER__DEFAULTS__PATH_OVERRIDE` points the child build at a
//! planted `product-defaults.toml`, exercising the GDPR/privacy render
//! validators (tri-state / `enforce_algo` / `default_value` /
//! vendor-id / key-range / IP-keep-bits) end-to-end through
//! `build.rs`.
//!
//! One build-data document carries BOTH product namespaces, so a fixture
//! planting a defect in `[host]` still has to ship a valid
//! `[account]` (and vice versa), without an operator configuration root.
//! `product_data_toml` assembles that around whichever half the case
//! is about.
//!
//! Build-time assertions for invalid bidder-owned metadata fixtures. The
//! sibling `url_safety.rs` proves the validator algorithm; this file
//! proves it actually fires through `build.rs` end-to-end — so if a
//! future refactor stopped calling `validate_endpoint_url` or
//! `validate_syncer_template`, the helper tests would still pass but
//! THIS test would catch the regression.
//! Target-dependent host aliases also build for `wasm32-wasip1`, which must
//! be installed locally; the native CI job already installs that target.
//!
//! ## Hermeticity
//!
//! A non-hermetic variant of this harness would write bad TOML directly
//! into the checked-out source tree. Even with an in-process lock, any
//! concurrent `cargo check` or `cargo test` elsewhere on the workspace
//! would see the planted file and trip over the build script.
//!
//! This version is hermetic. Each harness test:
//!   1. Creates a tempdir for bidder-owned packages.
//!   2. Writes a `fake_fixture_adapter/bidder.toml` with a good baseline and the bad
//!      effective definition under test, plus explicit factory source.
//!   3. Spawns `cargo build -p auctioneer-config-data` with
//!      `AUCTIONEER__BIDDERS__DIR=<tempdir>` and a separate
//!      `--target-dir` leased exclusively from this test process, so
//!      dependency artifacts are reused without colliding with the
//!      outer `cargo test` run or another fixture.
//!   4. `build.rs` honors the env var and walks the tempdir — the
//!      source tree's `auctioneer-bidder/src/bidders/` is never touched.
//!   5. The tempdir is dropped at the end of the test, cleaning up.
//!
//! All `#[test]` functions live inside `mod tests` so clippy's
//! `tests_outside_test_module` restriction is satisfied without a
//! file-level allow. The helpers (`build_bidder_tempdir`,
//! `cargo_build_with_bidder_dir`, etc.) stay at file scope and are
//! re-imported via `use super::*` inside the test module.

#![expect(
    clippy::expect_used,
    reason = "harness helpers fail loud on setup errors; allow-expect-in-tests \
              in clippy.toml covers #[test] fns only, not their helpers"
)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::thread;
use std::time::{Duration, Instant};

mod generated_syntax;

const CHILD_TIMEOUT: Duration = Duration::from_mins(5);

/// A timeout is an infrastructure error, never an accepted config rejection.
/// Files avoid the pipe-buffer deadlock of waiting before draining two pipes.
fn bounded_child_output(
    command: &mut Command,
    directory: &Path,
    timeout: Duration,
) -> io::Result<process::Output> {
    let stdout_path = directory.join("child.stdout");
    let stderr_path = directory.join("child.stderr");
    command
        .stdout(fs::File::create(&stdout_path)?)
        .stderr(fs::File::create(&stderr_path)?);
    #[cfg(unix)]
    command.process_group(0);
    let mut guard = ChildGuard(command.spawn()?);
    let started = Instant::now();
    loop {
        if let Some(status) = guard.0.try_wait()? {
            return Ok(process::Output {
                status,
                stdout: fs::read(&stdout_path)?,
                stderr: fs::read(&stderr_path)?,
            });
        }
        if started.elapsed() >= timeout {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "child {} timed out after {timeout:?}; stderr: {}",
                    guard.0.id(),
                    String::from_utf8_lossy(&fs::read(&stderr_path)?),
                ),
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// Reap the child on every exit, and terminate its own process group on Unix.
struct ChildGuard(process::Child);

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        // Cargo may exit while a compiler descendant still owns the target slot.
        // Group teardown is required even when the leader has already been reaped.
        #[cfg(unix)]
        {
            let _termination = Command::new("/bin/kill")
                .args(["-KILL", "--", &format!("-{}", self.0.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _termination = self.0.kill();
        let _reaping = self.0.wait();
    }
}

/// Per-invocation nonce. Combined with the process id it makes every
/// harness directory unique ACROSS PROCESSES — the default and
/// `--features insecure-bidders` workspace runs (or a rerun) can
/// execute the same `#[test]` concurrently, and a fixed per-label path
/// let one run's `remove_dir_all` delete the dir another run had just
/// created, so its `fs::write(fake_fixture_adapter.toml)` failed with "No such file
/// or directory". Unique dirs remove the shared path entirely.
static HARNESS_NONCE: AtomicU64 = AtomicU64::new(0);

/// Bound concurrent children and reusable dependency target directories.
const MAX_CONCURRENT_BUILDS: usize = 6;

static BUILD_TARGETS: Mutex<Weak<TargetPool>> = Mutex::new(Weak::new());

/// Each slot is leased through fixture teardown, including generated-file reads.
/// The last active/waiting harness drops the pool and removes its target trees.
/// Reuse therefore spans overlapping fixtures in libtest's default parallel run;
/// a deliberately serialized run trades reuse for immediate idle-tree cleanup.
struct TargetPool {
    root: PathBuf,
    available: Mutex<Vec<PathBuf>>,
    released: Condvar,
}

impl TargetPool {
    fn new(slots: usize) -> Arc<Self> {
        let root = crate_dir()
            .join("target/build_failure_harness")
            .join(unique_label("dependency_cache"));
        fs::create_dir_all(&root).expect("create harness dependency cache");
        Arc::new(Self {
            available: Mutex::new(
                (0..slots)
                    .map(|slot| root.join(format!("slot_{slot}")))
                    .collect(),
            ),
            root,
            released: Condvar::new(),
        })
    }

    fn acquire(self: &Arc<Self>) -> TargetLease {
        let mut available = self
            .available
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            if let Some(path) = available.pop() {
                return TargetLease {
                    pool: Arc::clone(self),
                    path,
                };
            }
            available = self
                .released
                .wait(available)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for TargetPool {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.root);
    }
}

struct TargetLease {
    pool: Arc<TargetPool>,
    path: PathBuf,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for TargetLease {
    fn drop(&mut self) {
        let mut available = self
            .pool
            .available
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        available.push(self.path.clone());
        drop(available);
        self.pool.released.notify_one();
    }
}

fn target_pool() -> Arc<TargetPool> {
    let mut weak = BUILD_TARGETS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(pool) = weak.upgrade() {
        return pool;
    }
    let pool = TargetPool::new(MAX_CONCURRENT_BUILDS);
    *weak = Arc::downgrade(&pool);
    pool
}

/// Minimal valid baseline so the tempdir always has at least one
/// good TOML — keeps the validator's failure pinned to the bad file
/// we're testing.
const BASELINE_FIXTURE: &str = r#"implementation = "fake_fixture_adapter"
endpoint = "https://fixture.example/openrtb2/auction"

[maintainer]
email = "baseline@auctioneer.test"

[capabilities]

[capabilities.app]
mediaTypes = ["banner"]

[capabilities.site]
mediaTypes = ["banner"]
"#;

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Workspace root = auctioneer/ (two levels up from the crate dir).
fn workspace_root() -> PathBuf {
    crate_dir()
        .parent()
        .and_then(Path::parent)
        .expect("crate two parents up = workspace root")
        .to_owned()
}

/// A directory name unique to this invocation across processes:
/// `<label>.<pid>.<nonce>`. The pid separates concurrent test binaries
/// (default vs insecure workspace runs); the nonce separates repeated
/// calls within one binary.
fn unique_label(label: &str) -> String {
    let n = HARNESS_NONCE.fetch_add(1, Ordering::Relaxed);
    format!("{label}.{}.{n}", process::id())
}

/// An independent fixture and exclusive target slot, held until teardown.
struct Harness {
    /// The env-var override points here: the bidder-owned root, or the
    /// dir holding a planted `product-defaults.toml`.
    fixture_dir: PathBuf,
    /// Dependency cache owned by this test process, not the outer Cargo build.
    target_dir: PathBuf,
    generated: RefCell<Option<PathBuf>>,
    _lease: TargetLease,
}

impl Harness {
    /// Path to a file inside the fixture dir (used by the positive-
    /// control tests that rewrite an owned definition after setup, and by
    /// the defaults-override runner to address the planted TOML).
    fn fixture(&self, name: &str) -> PathBuf {
        self.fixture_dir.join(name)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for Harness {
    fn drop(&mut self) {
        let _cleanup_fixture = fs::remove_dir_all(&self.fixture_dir);
    }
}

fn harness_dirs(label: &str) -> Harness {
    harness_dirs_in(label, &target_pool())
}

fn harness_dirs_in(label: &str, pool: &Arc<TargetPool>) -> Harness {
    let lease = pool.acquire();
    let unique = unique_label(label);
    let root = crate_dir().join("target/build_failure_harness");
    let fixture_dir = root.join(&unique);
    // The unique path won't normally exist; this is a belt-and-braces
    // guard against pid reuse across sequential runs of this binary.
    let _cleanup = fs::remove_dir_all(&fixture_dir);
    fs::create_dir_all(&fixture_dir).expect("create harness tempdir");
    Harness {
        fixture_dir,
        target_dir: lease.path.clone(),
        generated: RefCell::new(None),
        _lease: lease,
    }
}

/// Populate a fresh owner manifest with the baseline and supplied definition.
fn build_bidder_tempdir(label: &str, bad_filename: &str, bad_body: &str) -> Harness {
    let harness = harness_dirs(label);
    write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE)
        .expect("write baseline");
    write_bidder_metadata(&harness, bad_filename, bad_body).expect("write bad fixture");
    harness
}

fn write_bidder_metadata(harness: &Harness, filename: &str, body: &str) -> io::Result<()> {
    let name = Path::new(filename)
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| io::Error::other("invalid fixture public name"))?;
    write_owned_definition(harness, "fake_fixture_adapter", name, None, body)
}

fn write_owned_definition(
    harness: &Harness,
    key: &str,
    name: &str,
    required_feature: Option<&str>,
    body: &str,
) -> io::Result<()> {
    let definition: toml::Table = toml::from_str(body)
        .map_err(|_error| io::Error::other("invalid structured metadata fixture"))?;
    let directory = harness.fixture(key);
    fs::create_dir_all(&directory)?;
    let path = directory.join("bidder.toml");
    let mut manifest: toml::Table = if path.exists() {
        toml::from_str(&fs::read_to_string(&path)?)
            .map_err(|_error| io::Error::other("invalid owned fixture manifest"))?
    } else {
        let mut fields = toml::Table::new();
        fields.insert("key".into(), key.into());
        fields.insert("factory".into(), true.into());
        fields.insert(
            "endpoint".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "fields".into(),
                toml::Value::Table(toml::Table::from_iter([
                    ("account_id".into(), "AccountID".into()),
                    ("publisher_id".into(), "PublisherID".into()),
                ])),
            )])),
        );
        fields.insert("definitions".into(), toml::Value::Table(toml::Table::new()));
        fields.insert(
            "provenance".into(),
            toml::Value::Table(toml::Table::from_iter([("origin".into(), "local".into())])),
        );
        if let Some(feature) = required_feature {
            fields.insert("required_feature".into(), feature.into());
        }
        fields
    };
    if manifest
        .get("required_feature")
        .and_then(toml::Value::as_str)
        != required_feature
    {
        return Err(io::Error::other("fixture feature ownership changed"));
    }
    manifest
        .get_mut("definitions")
        .and_then(toml::Value::as_table_mut)
        .ok_or_else(|| io::Error::other("missing fixture definitions"))?
        .insert(name.into(), toml::Value::Table(definition));
    fs::write(
        &path,
        toml::to_string(&manifest)
            .map_err(|_error| io::Error::other("cannot serialize owned fixture"))?,
    )?;
    fs::write(
        directory.join("mod.rs"),
        "// Config consumes data, never this factory source.\n",
    )
}

fn write_endpoint_contract(
    harness: &Harness,
    key: &str,
    fields: &[(&str, &str)],
) -> io::Result<()> {
    let path = harness.fixture(key).join("bidder.toml");
    let mut manifest: toml::Table = toml::from_str(&fs::read_to_string(&path)?)
        .map_err(|_error| io::Error::other("invalid owned fixture manifest"))?;
    if fields.is_empty() {
        manifest
            .remove("endpoint")
            .ok_or_else(|| io::Error::other("missing fixture endpoint contract"))?;
    } else {
        manifest.insert(
            "endpoint".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "fields".into(),
                toml::Value::Table(
                    fields
                        .iter()
                        .map(|(field, name)| ((*field).into(), (*name).into()))
                        .collect(),
                ),
            )])),
        );
    }
    fs::write(
        path,
        toml::to_string(&manifest)
            .map_err(|_error| io::Error::other("cannot serialize owned fixture"))?,
    )
}

/// Keep deliberately malformed TOML intact for the child build's parser.
fn write_raw_owned_definition(harness: &Harness, body: &str) {
    write_owned_definition(
        harness,
        "fake_probe",
        "fake_probe",
        Some("insecure-bidders"),
        "",
    )
    .expect("write raw fixture owner");
    let path = harness.fixture("fake_probe/bidder.toml");
    let mut manifest: toml::Table =
        toml::from_str(&fs::read_to_string(&path).expect("read raw fixture owner"))
            .expect("parse raw fixture owner");
    manifest.remove("definitions");
    let header = toml::to_string(&manifest).expect("serialize raw fixture owner");
    fs::write(path, format!("{header}\n[definitions.fake_probe]\n{body}"))
        .expect("write malformed definition verbatim");
}

fn write_catalog_bidders(harness: &Harness, count: usize) -> io::Result<()> {
    for index in 0..count {
        write_bidder_metadata(
            harness,
            &format!("fake_catalog{index:03}.toml"),
            BASELINE_FIXTURE,
        )?;
    }
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(
        harness.fixture("fake_fixture_adapter/bidder.toml"),
    )?)
    .map_err(|_error| io::Error::other("invalid catalog fixture"))?;
    let actual = manifest
        .get("definitions")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| io::Error::other("missing catalog definitions"))?
        .len();
    if actual != count {
        return Err(io::Error::other("catalog fixture lost definitions"));
    }
    Ok(())
}

/// `build_bidder_tempdir`'s sibling for the host/account defaults
/// overrides: the fixture dir holds a single planted defaults TOML.
fn build_defaults_tempdir(label: &str, file_name: &str, body: &str) -> Harness {
    let harness = harness_dirs(label);
    fs::write(harness.fixture(file_name), body).expect("write defaults fixture");
    harness
}

fn build_alias_defaults_tempdir(label: &str, aliases: &str) -> Harness {
    let harness = build_defaults_tempdir(
        label,
        "product-defaults.toml",
        &product_data_toml(
            &product_host_toml(HOST_GDPR_BASELINE, aliases),
            &account_baseline(),
        ),
    );
    for key in ["fake_fixture_base", "fake_fixture_other"] {
        write_owned_definition(
            &harness,
            key,
            key,
            None,
            &format!("implementation = '{key}'\nendpoint = 'https://bidder.example.test/rtb'\n"),
        )
        .expect("write synthetic alias base");
    }
    harness
}

fn build_disclosure_tempdir(label: &str) -> Harness {
    let harness = harness_dirs(label);
    let data = crate_dir().join("data");
    for name in [
        "openrtb-2.6-source.md",
        "openrtb-2.6-source.toml",
        "openrtb-2.6-catalogue.toml",
        "supported-extension-catalogue.toml",
        "request-disclosure-policy.toml",
    ] {
        fs::copy(data.join(name), harness.fixture(name)).expect("copy disclosure input");
    }
    fs::create_dir_all(harness.fixture("disclosure-sources"))
        .expect("create disclosure source directory");
    for name in [
        "LICENSE-extensions",
        "user-extension.source",
        "device-extension.source",
        "ownership-classification.md",
    ] {
        let relative = Path::new("disclosure-sources").join(name);
        fs::copy(data.join(&relative), harness.fixture_dir.join(relative))
            .expect("copy disclosure source artifact");
    }
    harness
}

fn cargo_build_with_disclosure_dir(
    harness: &Harness,
    enable_override: bool,
) -> (ExitStatus, String) {
    let mut command = Command::new(env!("CARGO"));
    command.args(["build", "--locked", "-p", "auctioneer-config-data"]);
    if enable_override {
        command.args(["--features", "auctioneer-config-data/test-harness-override"]);
    }
    command
        .arg("--target-dir")
        .arg(&harness.target_dir)
        .env(
            "AUCTIONEER__DISCLOSURE__DIR",
            harness.fixture_dir.as_os_str(),
        )
        .env_remove(OsStr::new("AUCTIONEER__BIDDERS__DIR"))
        .env_remove(OsStr::new("AUCTIONEER__DEFAULTS__PATH_OVERRIDE"))
        .env_remove(OsStr::new("AUCTIONEER__GVL__ARCHIVE__DIR"))
        .current_dir(workspace_root());
    let output = cargo_output(
        without_bundle_check(without_compiler_wrappers(&mut command)),
        harness,
    )
    .expect("spawn disclosure policy build");
    (
        output.status,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Reuse Cargo artifacts, not daemon workers outside the owned process group.
fn without_compiler_wrappers(command: &mut Command) -> &mut Command {
    command
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
}

/// Turn off the build script's brotli compressed-bundle estimate for
/// this child.
///
/// Every case in this file asserts something about CONFIG validation;
/// none asserts anything about how the GVL corpus compresses. The
/// estimate is pure overhead repeated once per child, so each child
/// opts out and the gate is left to the builds that actually ship a
/// bundle.
///
/// `build.rs` honours the var only when `test-harness-override` is
/// enabled, which these children pass explicitly on the command line
/// and no production or CI build enables. The var travels through
/// `Command::env`, so it reaches this one child and never the harness
/// process's own environment.
fn without_bundle_check(command: &mut Command) -> &mut Command {
    command.env("AUCTIONEER__GVL__ARCHIVE__SKIP_BUNDLE_CHECK", "1")
}

/// Spawn `cargo build -p auctioneer-config-data` against the workspace root,
/// with `AUCTIONEER__BIDDERS__DIR` pointed at the harness fixture dir
/// and the harness `--target-dir` so concurrent runs and the outer
/// cargo test don't share a build cache.
fn cargo_build_with_bidder_dir(harness: &Harness) -> (ExitStatus, String) {
    cargo_build_with_bidder_dir_and_defaults_override(harness, None)
}

/// A non-TOML filename keeps the defaults out of the bidder directory scan.
fn cargo_build_with_bidder_dir_and_defaults_override(
    harness: &Harness,
    defaults_file: Option<&str>,
) -> (ExitStatus, String) {
    cargo_build_with_metadata_overrides(harness, defaults_file, true)
}

fn cargo_build_with_metadata_overrides(
    harness: &Harness,
    defaults_file: Option<&str>,
    enable_override: bool,
) -> (ExitStatus, String) {
    let mut command = metadata_build_command(harness, defaults_file, enable_override);
    let output = cargo_output(
        without_bundle_check(without_compiler_wrappers(&mut command)),
        harness,
    )
    .expect("spawn cargo build");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status, stderr)
}

fn metadata_build_command(
    harness: &Harness,
    defaults_file: Option<&str>,
    enable_override: bool,
) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .args([
            "build",
            "--locked",
            "-p",
            "auctioneer-config-data",
            "--target-dir",
        ])
        .arg(&harness.target_dir)
        .env("AUCTIONEER__BIDDERS__DIR", harness.fixture_dir.as_os_str())
        // Clear inherited defaults-path overrides so
        // the test stays pinned to its own fixture_dir and the
        // committed defaults TOMLs.
        .env_remove(OsStr::new("AUCTIONEER__DEFAULTS__PATH_OVERRIDE"))
        .env_remove(OsStr::new("AUCTIONEER__DISCLOSURE__DIR"))
        .current_dir(workspace_root());
    if enable_override {
        command.args(["--features", "auctioneer-config-data/test-harness-override"]);
    }
    if let Some(file_name) = defaults_file {
        command.env(
            "AUCTIONEER__DEFAULTS__PATH_OVERRIDE",
            harness.fixture(file_name),
        );
    }
    command
}

/// Spawn `cargo build -p auctioneer-config-data` with
/// `AUCTIONEER__DEFAULTS__PATH_OVERRIDE` pointed at the planted
/// `product-defaults.toml`. The bidder-owned metadata overrides are cleared so the child
/// reads the committed sibling `auctioneer-bidder/src/bidders/` tree.
///
/// One explicit test redirect covers both product-data namespaces.
fn cargo_build_with_defaults_override(harness: &Harness, file_name: &str) -> (ExitStatus, String) {
    let mut command = Command::new(env!("CARGO"));
    command
        .args([
            "build",
            "--locked",
            "-p",
            "auctioneer-config-data",
            // The env-var override is gated behind the
            // `test-harness-override` cargo feature, same as the
            // bidder-owned metadata redirect above.
            "--features",
            "auctioneer-config-data/test-harness-override",
            "--target-dir",
        ])
        .arg(&harness.target_dir)
        .env_remove(OsStr::new("AUCTIONEER__BIDDERS__DIR"))
        .env_remove(OsStr::new("AUCTIONEER__DISCLOSURE__DIR"))
        // A later `env` for the same key overrides the `env_remove`
        // above, so the one redirect under test survives the sweep.
        .env(
            "AUCTIONEER__DEFAULTS__PATH_OVERRIDE",
            harness.fixture(file_name).as_os_str(),
        )
        .current_dir(workspace_root());
    let output = cargo_output(
        without_bundle_check(without_compiler_wrappers(&mut command)),
        harness,
    )
    .expect("spawn cargo build");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status, stderr)
}

/// Only the planted defaults may redirect a test-harness child build.
/// Network policy is inherited from Cargo configuration / `CARGO_NET_OFFLINE`;
/// a config-only parent may not have fetched the core test dependencies yet.
fn privacy_config_parity_command(harness: &Harness) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .args([
            "test",
            "--locked",
            "-p",
            "auctioneer-core",
            "--features",
            "auctioneer-config-data/test-harness-override",
            "--test",
            "privacy_config_parity",
            "--target-dir",
        ])
        .arg(&harness.target_dir)
        .args([
            "--",
            "--ignored",
            "--exact",
            "tests::emitted_baked_config_matches_runtime_fixture",
        ])
        .env_remove(OsStr::new("AUCTIONEER__BIDDERS__DIR"))
        .env_remove(OsStr::new("AUCTIONEER__GVL__ARCHIVE__DIR"))
        .env_remove(OsStr::new("AUCTIONEER__DISCLOSURE__DIR"))
        .env(
            "AUCTIONEER__DEFAULTS__PATH_OVERRIDE",
            harness.fixture("product-defaults.toml"),
        )
        .current_dir(workspace_root());
    command
}

/// Compile a nondefault fixture and execute its generated values against the
/// runtime resolver. The child test target cannot recurse into this harness.
fn cargo_test_privacy_config_parity(harness: &Harness) -> (ExitStatus, String) {
    let mut command = privacy_config_parity_command(harness);
    let output = bounded_child_output(
        without_compiler_wrappers(&mut command),
        &harness.fixture_dir,
        CHILD_TIMEOUT,
    )
    .expect("spawn child configuration parity test");
    (
        output.status,
        format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

/// Spawn `cargo build -p auctioneer-config-data` with `AUCTIONEER__GVL__ARCHIVE__DIR`
/// pointed at a CHECKED-IN fixture directory under `tests/fixtures/`
/// (each carries its own `manifest.json`). The bidder-owned metadata and defaults
/// overrides are cleared so only the archive source is redirected, and
/// `AUCTIONEER__GVL__ARCHIVE__MAX_AGE_DAYS` is raised so the fixtures'
/// fixed `generated_at` never trips the build-freshness SLA — these
/// cases target the manifest/schema validators, not the age gate.
fn cargo_build_with_gvl_archive_dir(fixture_name: &str) -> (ExitStatus, String) {
    let harness = harness_dirs(fixture_name);
    let fixture_dir = crate_dir().join("tests/fixtures").join(fixture_name);
    let mut command = Command::new(env!("CARGO"));
    command
        .args([
            "build",
            "--locked",
            "-p",
            "auctioneer-config-data",
            "--features",
            "auctioneer-config-data/test-harness-override",
            "--target-dir",
        ])
        .arg(&harness.target_dir)
        .env_remove(OsStr::new("AUCTIONEER__BIDDERS__DIR"))
        .env_remove(OsStr::new("AUCTIONEER__DEFAULTS__PATH_OVERRIDE"))
        .env_remove(OsStr::new("AUCTIONEER__DISCLOSURE__DIR"))
        .env("AUCTIONEER__GVL__ARCHIVE__DIR", fixture_dir.as_os_str())
        .env("AUCTIONEER__GVL__ARCHIVE__MAX_AGE_DAYS", "100000")
        .current_dir(workspace_root());
    let output = cargo_output(
        without_bundle_check(without_compiler_wrappers(&mut command)),
        &harness,
    )
    .expect("spawn cargo build");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status, stderr)
}

/// Select this invocation's `OUT_DIR` from Cargo, not a stale feature's files.
fn cargo_output(command: &mut Command, harness: &Harness) -> io::Result<process::Output> {
    *harness.generated.borrow_mut() = None;
    command.args(["--message-format=json", "--jobs", "2"]);
    let output = bounded_child_output(command, &harness.fixture_dir, CHILD_TIMEOUT)?;
    for line in output.stdout.split(|byte| *byte == b'\n') {
        let Ok(event) = serde_json::from_slice::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("reason").and_then(serde_json::Value::as_str) != Some("build-script-executed")
        {
            continue;
        }
        let Some(out_dir) = event
            .get("out_dir")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from)
        else {
            continue;
        };
        let owner = out_dir
            .parent()
            .and_then(Path::file_name)
            .and_then(OsStr::to_str);
        if owner
            .and_then(|name| name.strip_prefix("auctioneer-config-data-"))
            .is_some_and(|hash| {
                !hash.is_empty() && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            *harness.generated.borrow_mut() = Some(out_dir.join("generated.rs"));
        }
    }
    Ok(output)
}

fn read_generated_rs(harness: &Harness) -> String {
    fs::read_to_string(
        harness
            .generated
            .borrow()
            .as_ref()
            .expect("Cargo must report this build's auctioneer-config-data OUT_DIR"),
    )
    .expect("read generated.rs")
}

fn read_generated_config(harness: &Harness) -> syn::Result<generated_syntax::GeneratedConfig> {
    generated_syntax::GeneratedConfig::parse(&read_generated_rs(harness))
}

fn read_build_directives(harness: &Harness) -> String {
    let generated = harness.generated.borrow();
    let directory = generated
        .as_ref()
        .and_then(|path| path.parent())
        .and_then(Path::parent)
        .expect("Cargo must report this build's output directory");
    fs::read_to_string(directory.join("output")).expect("read build-script directives")
}

/// Copy selected build trees without Git state, target caches or symlinks.
fn copy_build_sources(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for result in fs::read_dir(source)? {
        let dir_entry = result?;
        let name = dir_entry.file_name();
        if name == "target" || name == ".git" {
            continue;
        }
        let target = destination.join(name);
        let source_path = dir_entry.path();
        let metadata = fs::symlink_metadata(&source_path)?;
        if metadata.is_dir() {
            copy_build_sources(&source_path, &target)?;
        } else if metadata.is_file() {
            fs::copy(source_path, target)?;
        } else {
            return Err(io::Error::other(
                "build source must be a regular file or directory",
            ));
        }
    }
    Ok(())
}

/// Assemble strict product build data, never an operator configuration.
fn product_data_toml(host: &str, account: &str) -> String {
    format!("{host}\n{account}\n")
}

/// The `[host]` namespace: complete and valid apart from the
/// caller-supplied `[gdpr]` body and trailing tables. Mirrors the
/// shipped `product-defaults.toml`; `[host.gdpr.tcf2]` keeps its
/// serde-default empty maps so `trailing` can append
/// `[host.gdpr.tcf2.purposes.N]` /
/// `[host.gdpr.tcf2.special_features.N]` tables.
fn product_host_toml(gdpr_lines: &str, trailing: &str) -> String {
    let indented = prefix_tables(gdpr_lines, "host.gdpr");
    let trailing_tables = prefix_tables(trailing, "host");
    format!(
        r#"[host]
status_response = ""

[host.gdpr]
{indented}

[host.gdpr.tcf2]
enabled = false

[host.ccpa]
enforce = false

[host.lmt]
enforce = false

[host.host_cookie]
cookie_name = "auctioneer_uid"
family = "auctioneer"
ttl_days = 90

[host.user_sync]
external_url = "https://sync.example.test"

[host.resource_limits]
default_tmax_ms = 1000
max_tmax_ms = 2000
max_bidder_response_bytes = 262144
max_bidders_per_auction = 50
max_outbound_requests = 100
max_outbound_raw_bytes = 262144
max_outbound_request_bytes = 262144
max_transport_bytes = 262144
max_decoded_bytes = 262144
max_parsed_nodes = 65536
{trailing_tables}"#
    )
}

/// The `[account]` namespace, same contract as
/// [`product_host_toml`].
fn product_account_toml(gdpr_lines: &str, trailing: &str) -> String {
    let indented = prefix_tables(gdpr_lines, "account.gdpr");
    let trailing_tables = prefix_tables(trailing, "account");
    format!(
        r#"[account]
disabled = false
debug_allow = false
bid_rounding = "down"
targeting_prefix = "hb"

[account.gdpr]
{indented}

[account.gdpr.channel_enabled]
web = "unset"
app = "unset"
amp = "unset"
video = "unset"
dooh = "unset"
{trailing_tables}"#
    )
}

/// Rewrite every `[table]` header in `body` to sit under `namespace`.
///
/// The case bodies below are written in the SUBTREE-relative spelling
/// each Raw struct sees (`[gdpr.tcf2.purposes.1]`, `[privacy.ipv4]`),
/// which is what makes them readable next to the error message they
/// assert. This is the one place that knows those subtrees now live
/// under `[host]` or `[account]`, so a case never repeats the prefix.
fn prefix_tables(body: &str, namespace: &str) -> String {
    body.lines()
        .map(|line| {
            let trimmed = line.trim();
            match trimmed
                .strip_prefix('[')
                .and_then(|rest| rest.strip_suffix(']'))
            {
                Some(inner) => format!("[{namespace}.{inner}]"),
                None => line.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Valid `[gdpr]` body for `product_host_toml` callers that plant their
/// breakage elsewhere.
const HOST_GDPR_BASELINE: &str = "enabled = false\ndefault_value = \"0\"";

/// The `[host]` namespace with nothing wrong with it — for
/// cases whose defect is on the account side.
fn host_baseline() -> String {
    product_host_toml(HOST_GDPR_BASELINE, "")
}

/// The `[account]` namespace with nothing wrong with it.
fn account_baseline() -> String {
    product_account_toml(ACCOUNT_GDPR_BASELINE, "")
}

const ACCOUNT_GDPR_BASELINE: &str = "enabled = \"unset\"";

#[cfg(test)]
mod tests {
    use super::generated_syntax::{
        boolean, field, integer, map_entries, path_is, some, string, vector,
    };
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn build_source_copy_preserves_files_but_excludes_git_and_target_trees() {
        let harness = harness_dirs("copy_regular_sources");
        let source = harness.fixture("source");
        let destination = harness.fixture("destination");
        for path in ["nested", ".git", "target", "nested/target"] {
            fs::create_dir_all(source.join(path)).unwrap();
        }
        for path in [
            "root.rs",
            "nested/child.rs",
            ".git/config",
            "target/cache",
            "nested/target/cache",
        ] {
            fs::write(source.join(path), path).unwrap();
        }
        copy_build_sources(&source, &destination).unwrap();
        for path in ["root.rs", "nested/child.rs"] {
            assert_eq!(fs::read_to_string(destination.join(path)).unwrap(), path);
        }
        for path in [".git", "target", "nested/target"] {
            assert!(!destination.join(path).exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn build_source_copy_refuses_file_and_directory_symlinks_without_following_them() {
        use std::os::unix::fs::symlink;

        let harness = harness_dirs("copy_link_refusal");
        for label in ["file", "directory"] {
            let source = harness.fixture(label);
            fs::create_dir_all(&source).unwrap();
            let input = source.join("unexpected");
            let outside = harness.fixture(format!("outside-{label}").as_str());
            if label == "file" {
                fs::write(&outside, "must not be copied").unwrap();
            } else {
                fs::create_dir_all(&outside).unwrap();
            }
            symlink(&outside, &input).unwrap();
            let destination = harness.fixture(format!("copy-{label}").as_str());
            let error = copy_build_sources(&source, &destination).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert_eq!(
                error.to_string(),
                "build source must be a regular file or directory"
            );
            assert!(!destination.join("unexpected").exists());
        }
    }

    #[test]
    fn release_product_output_is_independent_of_root_operator_file_and_override_controls() {
        use quote::ToTokens as _;

        let harness = harness_dirs("operator_neutral_release");
        let input_overrides = [
            "AUCTIONEER__DEFAULTS__PATH_OVERRIDE",
            "AUCTIONEER__BIDDERS__DIR",
            "AUCTIONEER__DISCLOSURE__DIR",
            "AUCTIONEER__GVL__ARCHIVE__DIR",
        ];
        let mut expected = None;
        for state in ["missing", "changed"] {
            let source = harness.fixture(state);
            fs::create_dir_all(&source).unwrap();
            for name in ["Cargo.toml", "Cargo.lock"] {
                fs::copy(workspace_root().join(name), source.join(name)).unwrap();
            }
            for directory in ["crates", "vendor"] {
                copy_build_sources(&workspace_root().join(directory), &source.join(directory))
                    .unwrap();
            }
            let operator = source.join("auctioneer.toml");
            let original = "[runtime]\noperator_marker = 'original'\n";
            fs::write(&operator, original).unwrap();
            assert_eq!(fs::read_to_string(&operator).unwrap(), original);
            if state == "missing" {
                fs::remove_file(&operator).unwrap();
                assert!(!operator.exists());
            } else {
                fs::write(&operator, "changed operator bytes: deliberately not TOML [").unwrap();
                assert_ne!(fs::read_to_string(&operator).unwrap(), original);
            }

            let mut command = Command::new(env!("CARGO"));
            command
                .args([
                    "build",
                    "--release",
                    "--locked",
                    "-p",
                    "auctioneer-config-data",
                    "--target-dir",
                ])
                .arg(harness.target_dir.join(state))
                .current_dir(&source);
            // These nonexistent inputs must be ignored without the explicit feature.
            for variable in input_overrides {
                command.env(variable, harness.fixture("nonexistent-override"));
            }
            let output = cargo_output(
                without_bundle_check(without_compiler_wrappers(&mut command)),
                &harness,
            )
            .unwrap();
            assert!(
                output.status.success(),
                "{state}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let generated = read_generated_config(&harness).unwrap();
            let host = generated.initializer("host_default").unwrap();
            let account = generated.initializer("account_default").unwrap();
            assert!(path_is(field(host, &["user_sync"]).unwrap(), &["None"]));
            assert!(
                map_entries(field(host, &["aliases"]).unwrap())
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                integer(field(host, &["resource_limits", "default_tmax_ms"]).unwrap()).unwrap(),
                1000
            );
            assert_eq!(
                integer(field(account, &["privacy", "ipv4", "anon_keep_bits"]).unwrap()).unwrap(),
                24
            );
            let product = (
                host.to_token_stream().to_string(),
                account.to_token_stream().to_string(),
            );
            if let Some(first_product) = &expected {
                assert_eq!(
                    &product, first_product,
                    "operator bytes must not change product output"
                );
            } else {
                expected = Some(product);
            }
            let directives = read_build_directives(&harness);
            assert!(directives.contains("product-defaults.toml"), "{directives}");
            assert!(!directives.contains("auctioneer.toml"), "{directives}");
            assert!(
                !directives.contains("runtime-bootstrap.toml"),
                "{directives}"
            );
            assert!(!directives.contains("SKIP_BUNDLE_CHECK"), "{directives}");
            for variable in input_overrides {
                assert!(!directives.contains(variable), "{directives}");
            }
        }
    }

    #[test]
    fn child_builds_do_not_delegate_workers_outside_the_owned_process_group() {
        let mut command = Command::new(env!("CARGO"));
        command
            .env("RUSTC_WRAPPER", "synthetic-global-cache-wrapper")
            .env(
                "RUSTC_WORKSPACE_WRAPPER",
                "synthetic-workspace-cache-wrapper",
            );
        without_compiler_wrappers(&mut command);
        for key in ["RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"] {
            assert_eq!(
                command.get_envs().find(|(name, _)| *name == key).unwrap().1,
                None,
                "{key}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn child_timeout_is_an_infrastructure_failure_not_a_validation_rejection() {
        let harness = harness_dirs("timeout_control");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf 'child diagnostic' >&2; sleep 2"]);
        let result = bounded_child_output(
            &mut command,
            &harness.fixture_dir,
            Duration::from_millis(50),
        );
        let failure = result.unwrap_err();
        assert_eq!(failure.kind(), io::ErrorKind::TimedOut);
        assert!(failure.to_string().contains("child diagnostic"));
    }

    #[cfg(unix)]
    #[test]
    fn child_output_larger_than_both_pipe_buffers_is_drained_without_deadlock() {
        let harness = harness_dirs("output_control");
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "i=0; while [ $i -lt 20000 ]; do printf 'output-row\\n'; printf 'error-row\\n' >&2; i=$((i+1)); done"]);
        let output =
            bounded_child_output(&mut command, &harness.fixture_dir, Duration::from_secs(30))
                .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 220_000);
        assert_eq!(output.stderr.len(), 200_000);
    }

    #[cfg(unix)]
    #[test]
    fn child_timeout_terminates_the_owned_process_group() {
        let harness = harness_dirs("reaping_control");
        let descendant = harness.fixture("descendant.pid");
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 60 & printf '%s' $! > \"$1\"; wait", "probe"])
            .arg(&descendant);
        let result =
            bounded_child_output(&mut command, &harness.fixture_dir, Duration::from_secs(1));
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        let pid = fs::read_to_string(descendant).unwrap();
        let started = Instant::now();
        loop {
            let alive = Command::new("/bin/kill")
                .args(["-0", &pid])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success();
            if !alive {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "descendant {pid} survived the timeout"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[cfg(unix)]
    #[test]
    fn exited_leader_does_not_leave_a_worker_writing_into_a_returned_slot() {
        let harness = harness_dirs("exited_leader_control");
        let descendant = harness.fixture("descendant.pid");
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "sleep 60 & printf '%s' $! > \"$1\"; exit 23", "probe"])
            .arg(&descendant);
        let output =
            bounded_child_output(&mut command, &harness.fixture_dir, Duration::from_secs(5))
                .unwrap();
        assert_eq!(output.status.code(), Some(23_i32));
        let pid = fs::read_to_string(descendant).unwrap();
        let started = Instant::now();
        loop {
            let alive = Command::new("/bin/kill")
                .args(["-0", &pid])
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success();
            if !alive {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "worker {pid} survived leader exit"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn target_leases_are_exclusive_reusable_and_cleaned_up_after_the_last_owner() {
        let pool = TargetPool::new(2);
        let root = pool.root.clone();
        let first = pool.acquire();
        let second = pool.acquire();
        assert_ne!(first.path, second.path);
        assert!(!first.path.starts_with(workspace_root().join("target")));
        let first_path = first.path.clone();
        fs::create_dir_all(&first.path).unwrap();
        fs::write(first.path.join("dependency-sentinel"), b"cached dependency").unwrap();
        drop(first);
        let reused = pool.acquire();
        assert_eq!(reused.path, first_path);
        assert_eq!(
            fs::read(reused.path.join("dependency-sentinel")).unwrap(),
            b"cached dependency"
        );
        drop(pool);
        assert!(root.is_dir());
        drop(second);
        assert!(root.is_dir());
        drop(reused);
        assert!(!root.exists());
    }

    #[test]
    fn reused_dependencies_revalidate_distinct_good_and_bad_default_fixtures() {
        // Keep the real suite's child ceiling while forcing reuse of one slot.
        let _suite_lease = target_pool().acquire();
        let pool = TargetPool::new(1);
        let mut targets = Vec::new();
        let mut fixtures = BTreeSet::new();
        for (marker, valid) in [
            ("cache-first", true),
            ("cache-bad-first", false),
            ("cache-second", true),
            ("cache-bad-second", false),
        ] {
            let harness = harness_dirs_in(marker, &pool);
            assert!(fixtures.insert(harness.fixture_dir.clone()));
            targets.push(harness.target_dir.clone());
            let gdpr = if valid {
                HOST_GDPR_BASELINE.to_owned()
            } else {
                HOST_GDPR_BASELINE.replace("default_value = \"0\"", "default_value = \"7\"")
            };
            let host = product_host_toml(&gdpr, "").replace(
                "status_response = \"\"",
                &format!("status_response = \"{marker}\""),
            );
            fs::write(
                harness.fixture("product-defaults.toml"),
                product_data_toml(&host, &account_baseline()),
            )
            .unwrap();
            let (status, stderr) =
                cargo_build_with_defaults_override(&harness, "product-defaults.toml");
            assert_eq!(status.success(), valid, "{marker}: {stderr}");
            if valid {
                let generated = read_generated_config(&harness).unwrap();
                let status_response = string(
                    field(
                        generated.initializer("host_default").unwrap(),
                        &["status_response"],
                    )
                    .unwrap(),
                )
                .unwrap();
                assert!(
                    status_response == marker,
                    "the current fixture must reach codegen"
                );
                if marker == "cache-second" {
                    assert_ne!(status_response, "cache-first");
                }
            } else {
                assert!(stderr.contains("default_value"), "{stderr}");
                assert!(
                    harness.generated.borrow().is_none(),
                    "a failed child must not select stale generated output"
                );
            }
        }
        assert!(targets.iter().all(|target| *target == targets[0]));
        let root = pool.root.clone();
        drop(pool);
        assert!(!root.exists());
    }

    #[test]
    fn child_build_accepts_reviewed_disclosure_inventory() {
        let harness = build_disclosure_tempdir("reviewed_disclosure");
        let (status, stderr) = cargo_build_with_disclosure_dir(&harness, true);
        assert!(status.success(), "reviewed policy build failed:\n{stderr}");
    }

    #[test]
    fn child_build_rejects_missing_disclosure_disposition() {
        let harness = build_disclosure_tempdir("missing_disclosure_disposition");
        let path = harness.fixture("request-disclosure-policy.toml");
        let mut policy: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        policy["fields"].as_array_mut().unwrap().pop().unwrap();
        fs::write(&path, toml::to_string(&policy).unwrap()).unwrap();
        let (status, stderr) = cargo_build_with_disclosure_dir(&harness, true);
        assert!(!status.success(), "unclassified field compiled:\n{stderr}");
        assert!(
            stderr.contains("disclosure") && stderr.contains("missing"),
            "failure must be the missing disclosure disposition:\n{stderr}"
        );
    }

    #[test]
    fn child_build_rejects_disclosure_source_drift() {
        let harness = build_disclosure_tempdir("disclosure_source_drift");
        let path = harness.fixture("openrtb-2.6-source.md");
        let source = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{source}\n")).unwrap();
        let (status, stderr) = cargo_build_with_disclosure_dir(&harness, true);
        assert!(!status.success(), "source drift compiled:\n{stderr}");
        assert!(
            stderr.contains("digest"),
            "failure must be the digest gate:\n{stderr}"
        );
    }

    #[test]
    fn production_build_ignores_disclosure_fixture_override() {
        let harness = build_disclosure_tempdir("ignored_disclosure_override");
        fs::write(harness.fixture("request-disclosure-policy.toml"), "invalid").unwrap();
        let (status, stderr) = cargo_build_with_disclosure_dir(&harness, false);
        assert!(
            status.success(),
            "production honored a test override:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_http_endpoint_in_secure_dir() {
        let bidder_dir = build_bidder_tempdir(
            "http_endpoint",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "http://example.com/openrtb2"

[maintainer]
email = "harness@auctioneer.test"

[capabilities]

[capabilities.site]
mediaTypes = ["banner"]
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when bidder-owned metadata/ ships an http:// \
         endpoint, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("http://")
                && stderr.contains("without an insecure-bidders requirement"),
            "stderr should carry the HttpInDefaultBuild error message verbatim. \
         Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_malformed_endpoint_url() {
        let bidder_dir = build_bidder_tempdir(
            "malformed",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when bidder-owned metadata/ ships a malformed \
         URL (https:// with no host), but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("malformed"),
            "stderr should carry the MalformedUrl error message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_authority_macro_in_syncer_url() {
        let bidder_dir = build_bidder_tempdir(
            "authority_macro",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.com/openrtb2"

[maintainer]
email = "harness@auctioneer.test"

[userSync]
key = "harness"

[userSync.redirect]
url = "https://{{.Host}}/sync"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a syncer URL contains a \
         template macro in its authority, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("template macro") && stderr.contains("authority"),
            "stderr should carry the AuthorityMacro error message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_rejects_syncer_url_userinfo_and_fragment() {
        for (case, url) in [
            ("syncer_userinfo", "https://user:pass@example.com/sync"),
            ("syncer_fragment", "https://example.com/sync#fragment"),
        ] {
            let bidder_dir = build_bidder_tempdir(
                case,
                "fake_bad.toml",
                &format!(
                    "implementation = \"fake_fixture_adapter\"\nendpoint = \"https://example.com/openrtb2\"\n\
                     [maintainer]\nemail = \"harness@auctioneer.test\"\n\
                     [userSync]\nkey = \"harness\"\n\
                     [userSync.redirect]\nurl = {url:?}\n"
                ),
            );
            let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
            assert!(!status.success(), "{case} unexpectedly built: {stderr}");
            assert!(
                stderr.contains("userinfo or fragment"),
                "{case} did not report the syncer URL validation error: {stderr}"
            );
        }
    }

    #[test]
    fn child_build_fails_with_unknown_syncer_macro() {
        // Bidder syncer URL contains {{.ExternalURL}}, which is NOT on
        // the bidder-syncer macro allow-list. It belongs in the
        // redirect_url template — never in a bidder syncer URL.
        // Build-time error.
        let bidder_dir = build_bidder_tempdir(
            "unknown_macro",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.com/openrtb2"

[maintainer]
email = "harness@auctioneer.test"

[userSync]
key = "harness"

[userSync.redirect]
url = "https://example.com/sync?from={{.ExternalURL}}&r={{.RedirectURL}}"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a syncer URL contains an \
         unknown macro, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("{{.ExternalURL}}") && stderr.contains("allow-list"),
            "stderr should name the offending macro and reference the allow-list. \
         Got:\n{stderr}"
        );
    }

    #[test]
    fn bidder_gvl_vendor_id_over_65535_fails_build() {
        // The GVL vendor namespace ends at u16::MAX. `Option<u32>` on
        // BidderInfo does not enforce that, and a runtime `as u16` would
        // authorize against whichever vendor the low 16 bits name, so
        // the range check belongs in the renderer.
        let bidder_dir = build_bidder_tempdir(
            "gvl_vendor_id_over_u16",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://gvl.example.test/openrtb"
gvlVendorID = 65536

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when gvlVendorID exceeds u16::MAX, \
         but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("gvlVendorID out of range") && stderr.contains("65536"),
            "stderr should name the gvlVendorID rule and the offending value. \
         Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_private_ip_in_host_allowlist() {
        // hostAllowlist entries are run through the same ruleset as
        // the endpoint itself, so a private-IP literal (10.0.0.1)
        // MUST be rejected at build time even if the endpoint template
        // itself is otherwise valid.
        let bidder_dir = build_bidder_tempdir(
            "host_allowlist_private_ip",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}/openrtb"
hostAllowlist = ["eu.example.test", "10.0.0.1"]

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when hostAllowlist contains a \
         private-IP literal, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("private-IP authority") && stderr.contains("hostAllowlist entry"),
            "stderr should reference the private-IP rule and the hostAllowlist \
         entry field. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_when_host_allowlist_entry_is_url() {
        let bidder_dir = build_bidder_tempdir(
            "host_allowlist_url",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}/openrtb"
hostAllowlist = ["https://eu.example.test/"]

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a hostAllowlist entry includes \
         a scheme or path, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("looks like a URL"),
            "stderr should explain that entries must be bare authorities. \
         Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_when_host_varying_endpoint_omits_host_allowlist() {
        // An endpoint containing {{.Host}} MUST declare a non-empty
        // hostAllowlist. Without this gate, endpoint and allowlist
        // would be validated independently, letting host_allowlist:
        // None ship for a host-varying bidder.
        let bidder_dir = build_bidder_tempdir(
            "host_varying_missing_allowlist",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}/openrtb"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a host-varying endpoint omits \
         hostAllowlist, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("{{.Host}}") && stderr.contains("hostAllowlist"),
            "stderr should explain that hostAllowlist is required for \
         host-varying endpoints. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_when_fixed_endpoint_declares_host_allowlist() {
        // Symmetric guard: a fixed-host endpoint with a hostAllowlist is
        // a misconfig — the runtime check is host-equality and the
        // allowlist would be silently ignored.
        let bidder_dir = build_bidder_tempdir(
            "fixed_endpoint_with_allowlist",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://hb.example.test/openrtb"
hostAllowlist = ["eu.example.test"]

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a fixed-host endpoint declares \
         a hostAllowlist, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("no `{{.Host}}` macro") && stderr.contains("hostAllowlist"),
            "stderr should explain that hostAllowlist is only meaningful for \
         host-varying endpoints. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_unclosed_syncer_macro() {
        // Unclosed `{{...` macros must not slip past scan_unknown_macro.
        let bidder_dir = build_bidder_tempdir(
            "unclosed_macro",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.com/openrtb2"

[maintainer]
email = "harness@auctioneer.test"

[userSync]
key = "harness"

[userSync.redirect]
url = "https://example.com/sync?r={{.RedirectURL"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a syncer URL contains an \
         unclosed macro, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("unclosed template macro"),
            "stderr should reference the unclosed-macro error. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_unclosed_endpoint_macro() {
        // The syncer variant was fixed earlier; the endpoint template
        // path had the same gap — `extract_endpoint_macros` returned
        // the set seen so far on the first unclosed `{{`, so a bidder
        // shipping `?acct={{.AccountID` (no closing braces) passed
        // codegen and only failed at runtime.
        let bidder_dir = build_bidder_tempdir(
            "unclosed_endpoint",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.com/openrtb2?acct={{.AccountID"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when an endpoint template contains \
             an unclosed macro, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("unclosed template macro"),
            "stderr should reference the unclosed-macro error. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_host_macro_in_path() {
        // `{{.Host}}` is an authority-substitution token; it must
        // never appear in the path or query. Silently allowing it
        // there would let a fixed-host bidder be misclassified as
        // host-varying (substring scans would pick it up in the path).
        let bidder_dir = build_bidder_tempdir(
            "host_macro_in_path",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://fixed.example.test/rtb/{{.Host}}"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when `{{{{.Host}}}}` appears in the \
             endpoint path, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("`{{.Host}}` in the") || stderr.contains("path or query"),
            "stderr should reference the host-macro-outside-authority error. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_localhost_alias_endpoint() {
        // 127.0.0.1.nip.io always resolves to loopback; reject as a
        // localhost alias.
        let bidder_dir = build_bidder_tempdir(
            "localhost_alias",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://127.0.0.1.nip.io/openrtb"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when the endpoint is a localhost \
         alias, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("DNS-trick loopback alias"),
            "stderr should reference the localhost-alias rejection. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_unsupported_endpoint_authority_macro() {
        // {{.RegionHost}} is NOT the spec-sanctioned authority macro.
        // Only an authority of exactly `{{.Host}}` is allowed.
        let bidder_dir = build_bidder_tempdir(
            "unsupported_authority_macro",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.RegionHost}}/openrtb"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when the endpoint authority contains a \
         macro other than {{.Host}}, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("authority") && stderr.contains("unsupported template macro"),
            "stderr should name the offending macro and reference the spec rule. \
         Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_compound_authority_macro() {
        // {{.Host}}{{.Port}} adds a second authority macro alongside
        // the allow-listed one.
        let bidder_dir = build_bidder_tempdir(
            "compound_authority_macro",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}{{.Port}}/openrtb"
hostAllowlist = ["eu.example.test"]

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when the endpoint authority combines \
         {{.Host}} with another macro, but it succeeded. stderr:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_unspecified_ip_endpoint() {
        // 0.0.0.0 must not pass codegen.
        let bidder_dir = build_bidder_tempdir(
            "unspecified_ip",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://0.0.0.0/openrtb"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when the endpoint is the IPv4 \
         unspecified literal, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("private-IP authority"),
            "stderr should reference the private/non-public-IP rule. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_documentation_ip_endpoint() {
        let bidder_dir = build_bidder_tempdir(
            "documentation_ip",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://192.0.2.1/openrtb"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when the endpoint is in the RFC 5737 \
         TEST-NET-1 documentation range, but it succeeded. stderr:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_userinfo_in_host_allowlist_entry() {
        // hostAllowlist entries must be bare authorities — no
        // userinfo (@).
        let bidder_dir = build_bidder_tempdir(
            "host_allowlist_userinfo",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}/openrtb"
hostAllowlist = ["attacker@example.test"]

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a hostAllowlist entry contains \
         userinfo, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("forbidden character") && stderr.contains("'@'"),
            "stderr should reference the forbidden-char rule and name `@`. \
         Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_fragment_in_host_allowlist_entry() {
        let bidder_dir = build_bidder_tempdir(
            "host_allowlist_fragment",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}/openrtb"
hostAllowlist = ["example.test#frag"]

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a hostAllowlist entry contains a \
         fragment marker, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("forbidden character") && stderr.contains("'#'"),
            "stderr should reference the forbidden-char rule and name `#`. \
         Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_unknown_endpoint_compression() {
        // Typos in endpointCompression must fail the build rather than
        // silently downgrading to None.
        let bidder_dir = build_bidder_tempdir(
            "unknown_compression",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.com/openrtb"
endpointCompression = "gizp"

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when endpointCompression is not \
         recognized, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("endpointCompression") && stderr.contains("allow-list"),
            "stderr should name the field and the offending value. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_unknown_media_type() {
        // Typos / future formats in mediaTypes must fail the build
        // rather than silently dropping.
        let bidder_dir = build_bidder_tempdir(
            "unknown_media_type",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.com/openrtb"

[maintainer]
email = "harness@auctioneer.test"

[capabilities]

[capabilities.site]
mediaTypes = ["banner", "vide"]
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a mediaTypes entry is not \
         recognized, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("mediaTypes") && stderr.contains("allow-list"),
            "stderr should name the field and the offending value. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_port_in_host_allowlist_entry() {
        // hostAllowlist entries are host-only. Port-bearing entries
        // are rejected at build time so the runtime comparison
        // (Url::host_str() vs stored entries) is unambiguous.
        let bidder_dir = build_bidder_tempdir(
            "host_allowlist_port",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}/openrtb"
hostAllowlist = ["eu.example.test:8443"]

[maintainer]
email = "harness@auctioneer.test"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when a hostAllowlist entry includes a \
         port, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("includes a port") && stderr.contains("host-only"),
            "stderr should reference the port-rejection rule and the host-only \
         representation. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_succeeds_with_valid_host_allowlist() {
        let bidder_dir =
            build_bidder_tempdir("host_allowlist_ok", "fake_unused.toml", BASELINE_FIXTURE);
        // Overwrite the baseline definition with a host-varying flavour
        // so the allowlist is exercised.
        write_bidder_metadata(
            &bidder_dir,
            "fake_fixture_adapter.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://{{.Host}}/openrtb"
hostAllowlist = ["eu.example.test", "us.example.test"]

[maintainer]
email = "baseline@auctioneer.test"

[capabilities]

[capabilities.site]
mediaTypes = ["banner"]
"#,
        )
        .expect("rewrite fixture definition");

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            status.success(),
            "child cargo build with a valid hostAllowlist should succeed, but \
         it failed. stderr:\n{stderr}"
        );
    }

    /// Positive control: a clean tempdir with just the baseline toml must
    /// build successfully. Proves the env-var redirect itself works — if
    /// build.rs failed to honor `AUCTIONEER__BIDDERS__DIR`, this test would
    /// either fall back to the source-tree fixtures (and pass for the wrong
    /// reason) or fail with a missing-dir error.
    #[test]
    fn child_build_succeeds_with_clean_tempdir() {
        let bidder_dir = build_bidder_tempdir(
            "clean",
            "fake_unused.toml",
            BASELINE_FIXTURE, // identical to baseline; harmless extra
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            status.success(),
            "child cargo build with a clean tempdir + env override should \
         succeed, but it failed. stderr:\n{stderr}"
        );
        let generated = read_generated_config(&bidder_dir).unwrap();
        assert_eq!(
            generated.bidder_names().unwrap(),
            ["fake_fixture_adapter", "fake_unused"]
        );
    }

    #[test]
    fn production_build_ignores_bidder_fixture_override() {
        let harness = harness_dirs("production_bidder_override");
        fs::create_dir_all(harness.fixture("fake_probe")).unwrap();
        fs::write(harness.fixture("fake_probe/bidder.toml"), "not valid TOML").unwrap();
        let (status, stderr) = cargo_build_with_metadata_overrides(&harness, None, false);
        assert!(
            status.success(),
            "production honored a bidder override: {stderr}"
        );
        let generated = read_generated_config(&harness).unwrap();
        // This child enables no bidder features. Its expected roster comes
        // from committed source, not the parent's possibly feature-enabled view.
        let mut expected = BTreeSet::new();
        let root = crate_dir()
            .with_file_name("auctioneer-bidder")
            .join("src/bidders");
        for directory in fs::read_dir(root).unwrap() {
            let path = directory.unwrap().path().join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let manifest: toml::Table = toml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            match manifest
                .get("required_feature")
                .and_then(toml::Value::as_str)
            {
                None => {}
                Some("insecure-bidders") => continue,
                Some(feature) => panic!("add explicit child feature selection for {feature}"),
            }
            if let Some(definitions) = manifest.get("definitions").and_then(toml::Value::as_table) {
                for name in definitions.keys() {
                    assert!(expected.insert(name.clone()));
                }
            }
        }
        assert!(!expected.is_empty());
        let names = generated.bidder_names().unwrap();
        assert_eq!(names.len(), expected.len());
        assert_eq!(names.into_iter().collect::<BTreeSet<_>>(), expected);
        assert!(generated.bidder("fake_probe").unwrap().is_none());
    }

    #[test]
    fn child_build_fails_with_unsupported_endpoint_macro() {
        // Privacy placeholders remain unsupported even with an endpoint contract.
        let bidder_dir = build_bidder_tempdir(
            "unsupported_endpoint_macro",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.test/path?gdpr={{.GDPR}}"

[maintainer]
email = "harness@auctioneer.test"

[capabilities]

[capabilities.site]
mediaTypes = ["banner"]
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            !status.success(),
            "child cargo build should fail when an endpoint template uses an \
             unsupported macro, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("endpoint") && stderr.contains("unsupported macro"),
            "stderr should name the unsupported GDPR macro. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_succeeds_with_publisher_id_macro() {
        // Positive control: the fixture's owner explicitly declares PublisherID.
        let bidder_dir = build_bidder_tempdir(
            "publisher_id_macro_ok",
            "fake_unused.toml",
            BASELINE_FIXTURE,
        );
        write_bidder_metadata(
            &bidder_dir,
            "fake_fixture_adapter.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://example.test/openrtb?pub={{.PublisherID}}"

[maintainer]
email = "baseline@auctioneer.test"

[capabilities]

[capabilities.site]
mediaTypes = ["banner"]
"#,
        )
        .expect("rewrite fixture definition");

        let (status, stderr) = cargo_build_with_bidder_dir(&bidder_dir);
        assert!(
            status.success(),
            "PublisherID is declared by the owner; build should succeed. \
             stderr:\n{stderr}"
        );
    }

    // ---- host/account defaults overrides (GDPR + privacy schema) ----

    #[test]
    fn endpoint_binding_owner_contract_controls_active_and_inactive_aliases() {
        let pool = target_pool();
        for (feature, declaration, should_build) in [
            (None, Some("RouteID"), true),
            (Some("insecure-bidders"), Some("RouteID"), true),
            (None, Some("OtherRoute"), false),
            (Some("insecure-bidders"), Some("OtherRoute"), false),
            (None, None, false),
        ] {
            let harness = harness_dirs_in("endpoint_binding_owner", &pool);
            write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE).unwrap();
            write_owned_definition(
                &harness,
                "fake_future_owner",
                "fake_public_alias",
                feature,
                "endpoint = 'https://fixture.example/{{.RouteID}}'\n",
            )
            .unwrap();
            let fields: Vec<_> = declaration
                .map(|name| ("route", name))
                .into_iter()
                .collect();
            write_endpoint_contract(&harness, "fake_future_owner", &fields).unwrap();
            // Another family's declaration cannot supply the alias's binding.
            write_owned_definition(
                &harness,
                "fake_other_owner",
                "fake_other_public",
                None,
                "endpoint = 'https://other.example/static'\n",
            )
            .unwrap();
            write_endpoint_contract(&harness, "fake_other_owner", &[("route", "RouteID")]).unwrap();
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            assert_eq!(
                status.success(),
                should_build,
                "feature {feature:?}, declaration {declaration:?}: {stderr}"
            );
            if should_build {
                let generated = read_generated_config(&harness).unwrap();
                let maybe_bidder = generated.bidder("fake_public_alias").unwrap();
                assert_eq!(maybe_bidder.is_some(), feature.is_none());
                if let Some(bidder) = maybe_bidder {
                    let macros = vector(field(&bidder, &["endpoint_macros"]).unwrap()).unwrap();
                    assert_eq!(
                        macros
                            .iter()
                            .map(string)
                            .collect::<syn::Result<Vec<_>>>()
                            .unwrap(),
                        ["RouteID"]
                    );
                }
            } else {
                assert!(stderr.contains("unsupported macro"), "{stderr}");
            }
        }
    }

    #[test]
    fn endpoint_binding_definition_cannot_grant_itself_supported_names() {
        let harness = build_bidder_tempdir(
            "endpoint_binding_self_grant",
            "fake_unused.toml",
            BASELINE_FIXTURE,
        );
        write_owned_definition(
            &harness, "fake_future_owner", "fake_public_alias", None,
            "endpoint = 'https://fixture.example/{{.RouteID}}'\nsupported_endpoint_macros = ['RouteID']\n",
        ).unwrap();
        write_endpoint_contract(&harness, "fake_future_owner", &[]).unwrap();
        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(!status.success());
        assert!(
            stderr.contains("invalid bidder metadata fields"),
            "{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_invalid_enforce_algo() {
        // `enforce_algo` is `full`/`basic`, NOT a tri-state; anything
        // else must fail codegen instead of baking a default.
        let harness = build_defaults_tempdir(
            "gdpr_bad_enforce_algo",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    HOST_GDPR_BASELINE,
                    "\n[gdpr.tcf2.purposes.1]\nenforce_algo = \"strict\"\n\
                 enforce_purpose = \"enabled\"\nenforce_vendors = \"enabled\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when a purpose's enforce_algo is not \
             full/basic, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("invalid enforce_algo") && stderr.contains("strict"),
            "stderr should carry the InvalidEnforceAlgo message naming the value. \
             Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_scalar_basic_enforcement_vendors() {
        // A scalar where a list is required must fail deserialization.
        let harness = build_defaults_tempdir(
            "gdpr_scalar_vendors",
            "product-defaults.toml",
            &product_data_toml(
                &product_account_toml(
                    "enabled = \"unset\"\nbasic_enforcement_vendors = \"notalist\"",
                    "",
                ),
                &host_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when basic_enforcement_vendors is a \
             scalar instead of a list, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("basic_enforcement_vendors") && stderr.contains("expected a sequence"),
            "stderr should name the field and the sequence type error. Got:\n{stderr}"
        );
    }

    #[test]
    fn host_alias_bases_follow_native_only_selection_on_real_cargo_targets() {
        let compiler = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--print", "host-tuple"])
            .output()
            .unwrap();
        assert!(compiler.status.success(), "{compiler:?}");
        let host_output = String::from_utf8(compiler.stdout).unwrap();
        let host = host_output.trim();
        assert!(!host.is_empty() && !host.starts_with("wasm"));
        let harness = harness_dirs("host_alias_target_selection");
        for (key, native_only) in [("fake_portable_base", false), ("fake_native_base", true)] {
            write_owned_definition(
                &harness,
                key,
                key,
                None,
                "endpoint = 'https://fixture.example/auction'\n",
            )
            .unwrap();
            let path = harness.fixture(key).join("bidder.toml");
            let mut manifest: toml::Table =
                toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert!(
                manifest
                    .insert("native_only".into(), native_only.into())
                    .is_none()
            );
            fs::write(path, toml::to_string(&manifest).unwrap()).unwrap();
        }
        for target in [host, "wasm32-wasip1"] {
            for base in ["fake_portable_base", "fake_native_base"] {
                let should_build = target == host || base == "fake_portable_base";
                let source = product_data_toml(
                    &product_host_toml(
                        HOST_GDPR_BASELINE,
                        &format!("\n[aliases]\nfake_host_alias = '{base}'\n"),
                    ),
                    &account_baseline(),
                );
                fs::write(harness.fixture("target-defaults.toml"), source).unwrap();
                let mut command =
                    metadata_build_command(&harness, Some("target-defaults.toml"), true);
                command.args(["--target", target]).env(
                    "CARGO_BUILD_TARGET",
                    if target == host {
                        "wasm32-wasip1"
                    } else {
                        host
                    },
                );
                let output = cargo_output(
                    without_bundle_check(without_compiler_wrappers(&mut command)),
                    &harness,
                )
                .unwrap();
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert_eq!(
                    output.status.success(),
                    should_build,
                    "target {target:?}, base {base}: {stderr}"
                );
                if !should_build {
                    assert!(
                        stderr.contains("fake_host_alias")
                            && stderr.contains("fake_native_base")
                            && stderr.contains("bidder registry does not carry"),
                        "the guest must reject the absent alias base, not an unrelated build error: {stderr}"
                    );
                }
            }
        }
    }

    #[test]
    fn host_alias_referencing_unknown_base_fails_build() {
        // `fake_unknown_bidder` has no effective definition in the bidder-owned
        // catalog, so this case fails identically
        // with and without `--features insecure-bidders`. Naming a
        // bidder that requires insecure-bidders would make the
        // outcome depend on which feature set the workspace run used.
        let harness = build_alias_defaults_tempdir(
            "host_alias_unknown_base",
            "\n[aliases]\nfake_my_alias = \"fake_unknown_bidder\"\n",
        );

        let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
            &harness,
            Some("product-defaults.toml"),
        );
        assert!(
            !status.success(),
            "child cargo build should fail when a host `[aliases]` row names a \
             bidder the registry does not carry, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("fake_my_alias")
                && stderr.contains("fake_unknown_bidder")
                && stderr.contains("bidder registry does not carry"),
            "stderr should name the offending alias, its base, and why the row \
             was rejected. Got:\n{stderr}"
        );
    }

    #[test]
    fn host_alias_namespace_child_build_rejects_ascii_fold_collision() {
        let harness = build_alias_defaults_tempdir(
            "host_alias_fold_collision",
            "\n[aliases]\nfake_Foo = \"fake_fixture_base\"\nfake_foo = \"fake_fixture_other\"\n",
        );

        let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
            &harness,
            Some("product-defaults.toml"),
        );
        assert!(
            !status.success(),
            "ASCII-fold-colliding host aliases must fail the child build: {stderr}"
        );
        assert!(
            stderr.contains("alias namespace collides") && stderr.contains("fake_foo"),
            "stderr should identify the alias namespace collision: {stderr}"
        );
    }

    #[test]
    fn host_alias_namespace_child_build_rejects_differently_cased_base() {
        let harness = build_alias_defaults_tempdir(
            "host_alias_cased_base_shadow",
            "\n[aliases]\nfake_Fixture_base = \"fake_fixture_other\"\n",
        );

        let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
            &harness,
            Some("product-defaults.toml"),
        );
        assert!(
            !status.success(),
            "a differently cased base shadow must fail the child build: {stderr}"
        );
        assert!(
            stderr.contains("fake_Fixture_base")
                && stderr.contains("shadows a differently cased base bidder"),
            "stderr should identify the differently cased base shadow: {stderr}"
        );
    }

    #[test]
    fn host_alias_namespace_child_build_rejects_empty_name() {
        let harness = build_alias_defaults_tempdir(
            "host_alias_empty_name",
            "\n[aliases]\n\"\" = \"fake_fixture_other\"\n",
        );

        let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
            &harness,
            Some("product-defaults.toml"),
        );
        assert!(
            !status.success(),
            "an empty host alias name must fail the child build: {stderr}"
        );
        assert!(
            stderr.contains("host alias name must be nonempty"),
            "stderr should identify the empty alias name: {stderr}"
        );
    }

    #[test]
    fn host_alias_namespace_child_build_rejects_name_above_128_utf8_bytes() {
        let alias = format!("{}\u{e9}", "x".repeat(127));
        assert_eq!(alias.len(), 129);
        assert_eq!(alias.chars().count(), 128);
        let harness = build_alias_defaults_tempdir(
            "host_alias_overlong_utf8_name",
            &format!("\n[aliases]\n\"{alias}\" = \"fake_fixture_other\"\n"),
        );

        let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
            &harness,
            Some("product-defaults.toml"),
        );
        assert!(
            !status.success(),
            "a 129-byte host alias must fail even with only 128 characters: {stderr}"
        );
        assert!(
            stderr.contains("host alias name exceeds 128 UTF-8 bytes"),
            "stderr should identify the alias byte-length limit: {stderr}"
        );
    }

    #[test]
    fn host_alias_namespace_child_build_allows_exact_base_override_and_name_boundaries() {
        let ascii_boundary = "x".repeat(128);
        let utf8_boundary = "\u{e9}".repeat(64);
        assert_eq!(utf8_boundary.len(), 128);
        let harness = build_alias_defaults_tempdir(
            "host_alias_exact_override",
            &format!(
                "\n[aliases]\nfake_fixture_base = \"fake_fixture_other\"\nfake_Foo = \"fake_fixture_other\"\n\
                 \"{ascii_boundary}\" = \"fake_fixture_other\"\n\
                 \"{utf8_boundary}\" = \"fake_fixture_other\"\n"
            ),
        );

        let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
            &harness,
            Some("product-defaults.toml"),
        );
        assert!(
            status.success(),
            "exact base overrides and valid 128-byte alias names must build: {stderr}"
        );
        let generated = read_generated_config(&harness).unwrap();
        let alias_entries = map_entries(
            field(generated.initializer("host_default").unwrap(), &["aliases"]).unwrap(),
        )
        .unwrap();
        let aliases = alias_entries
            .iter()
            .map(|(key, value)| Ok((string(key)?, string(value)?)))
            .collect::<syn::Result<BTreeMap<_, _>>>()
            .unwrap();
        for alias in [
            "fake_fixture_base",
            "fake_Foo",
            &ascii_boundary,
            &utf8_boundary,
        ] {
            assert_eq!(
                aliases.get(alias).map(String::as_str),
                Some("fake_fixture_other"),
                "the child build must preserve alias {alias:?}"
            );
        }
    }

    #[test]
    fn child_build_fails_with_host_vendor_id_above_u16_max() {
        // Reject, do not clamp: `u32 as u16` would silently wrap
        // 65536 → 0.
        let harness = build_defaults_tempdir(
            "gdpr_vendor_id_overflow",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    "enabled = false\ndefault_value = \"0\"\nhost_vendor_id = 65536",
                    "",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when host_vendor_id exceeds u16::MAX, \
             but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("host_vendor_id 65536 out of range"),
            "stderr should carry the HostVendorIdOutOfRange message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_negative_host_vendor_id() {
        let harness = build_defaults_tempdir(
            "gdpr_vendor_id_negative",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    "enabled = false\ndefault_value = \"0\"\nhost_vendor_id = -1",
                    "",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when host_vendor_id is negative, but \
             it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("host_vendor_id = -1") && stderr.contains("expected u32"),
            "stderr should point at the negative value failing the u32 parse. \
             Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_invalid_enforce_purpose_tristate() {
        // The fail-open renderer bug class: an unknown tri-state value
        // must fail the build, not map to `TriState::Unset`.
        let harness = build_defaults_tempdir(
            "gdpr_bad_enforce_purpose",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    HOST_GDPR_BASELINE,
                    "\n[gdpr.tcf2.purposes.1]\nenforce_algo = \"full\"\n\
                 enforce_purpose = \"yes\"\nenforce_vendors = \"enabled\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when enforce_purpose is not a \
             tri-state, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("invalid tri-state") && stderr.contains("yes"),
            "stderr should carry the InvalidTristate message naming the value. \
             Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_invalid_enforce_vendors_tristate() {
        let harness = build_defaults_tempdir(
            "gdpr_bad_enforce_vendors",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    HOST_GDPR_BASELINE,
                    "\n[gdpr.tcf2.purposes.1]\nenforce_algo = \"full\"\n\
                 enforce_purpose = \"enabled\"\nenforce_vendors = \"yes\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when enforce_vendors is not a \
             tri-state, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("invalid tri-state") && stderr.contains("yes"),
            "stderr should carry the InvalidTristate message naming the value. \
             Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_invalid_gdpr_default_value() {
        // Only literal "0" / "1" are legal consent-scope fallbacks.
        let harness = build_defaults_tempdir(
            "gdpr_bad_default_value",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml("enabled = false\ndefault_value = \"2\"", ""),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when gdpr.default_value is not \"0\" \
             or \"1\", but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("invalid gdpr default_value"),
            "stderr should carry the InvalidGdprDefaultValue message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_ipv4_keep_bits_above_32() {
        let harness = build_defaults_tempdir(
            "privacy_ipv4_out_of_range",
            "product-defaults.toml",
            &product_data_toml(
                &product_account_toml(
                    ACCOUNT_GDPR_BASELINE,
                    "\n[privacy.ipv4]\nanon_keep_bits = 33\n",
                ),
                &host_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when ipv4.anon_keep_bits exceeds 32, \
             but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("ipv4.anon_keep_bits 33 exceeds max 32"),
            "stderr should carry the ipv4 IpKeepBitsOutOfRange message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_ipv6_keep_bits_above_128() {
        let harness = build_defaults_tempdir(
            "privacy_ipv6_out_of_range",
            "product-defaults.toml",
            &product_data_toml(
                &product_account_toml(
                    ACCOUNT_GDPR_BASELINE,
                    "\n[privacy.ipv6]\nanon_keep_bits = 129\n",
                ),
                &host_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when ipv6.anon_keep_bits exceeds 128, \
             but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("ipv6.anon_keep_bits 129 exceeds max 128"),
            "stderr should carry the ipv6 IpKeepBitsOutOfRange message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_purpose_key_zero() {
        let harness = build_defaults_tempdir(
            "gdpr_purpose_key_zero",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    HOST_GDPR_BASELINE,
                    "\n[gdpr.tcf2.purposes.0]\nenforce_algo = \"full\"\n\
                 enforce_purpose = \"enabled\"\nenforce_vendors = \"enabled\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on purpose key 0, but it succeeded. \
             stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("purpose id 0 out of range"),
            "stderr should carry the PurposeIdOutOfRange message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_purpose_key_eleven() {
        // Exercises the overflow edge exactly one past the 1..=10 range.
        let harness = build_defaults_tempdir(
            "gdpr_purpose_key_eleven",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    HOST_GDPR_BASELINE,
                    "\n[gdpr.tcf2.purposes.11]\nenforce_algo = \"full\"\n\
                 enforce_purpose = \"enabled\"\nenforce_vendors = \"enabled\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on purpose key 11, but it succeeded. \
             stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("purpose id 11 out of range"),
            "stderr should carry the PurposeIdOutOfRange message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_special_feature_key_zero() {
        let harness = build_defaults_tempdir(
            "gdpr_sf_key_zero",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    HOST_GDPR_BASELINE,
                    "\n[gdpr.tcf2.special_features.0]\nenforce = \"enabled\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on special-feature key 0, but it \
             succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("special feature id 0 out of range"),
            "stderr should carry the SpecialFeatureIdOutOfRange message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_account_special_feature_key_thirteen() {
        // Proves the ACCOUNT renderer runs the same key check as the
        // host one.
        let harness = build_defaults_tempdir(
            "gdpr_account_sf_key_thirteen",
            "product-defaults.toml",
            &product_data_toml(
                &product_account_toml(
                    ACCOUNT_GDPR_BASELINE,
                    "\n[gdpr.special_features.13]\nenforce = \"enabled\"\n",
                ),
                &host_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on account special-feature key 13, \
             but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("special feature id 13 out of range"),
            "stderr should carry the SpecialFeatureIdOutOfRange message. Got:\n{stderr}"
        );
    }

    #[test]
    fn child_build_fails_with_misspelled_privacy_key() {
        // The raw graph is `#[serde(deny_unknown_fields)]`: a
        // privacy-critical typo must fail deserialization, not
        // silently select a default.
        let harness = build_defaults_tempdir(
            "gdpr_misspelled_key",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    HOST_GDPR_BASELINE,
                    "\n[gdpr.tcf2.purposes.1]\nenforce_algo = \"full\"\n\
                 enforce_purpose = \"enabled\"\nenforce_vendors = \"enabled\"\n\
                 enforce_purpsoe = \"enabled\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on a misspelled privacy key, but it \
             succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("unknown field") && stderr.contains("enforce_purpsoe"),
            "stderr should carry serde's unknown-field error naming the typo. \
             Got:\n{stderr}"
        );
    }

    /// Positive control: a VALID `[host]` override differing from
    /// the baked one must be honored — proving the env var is actually
    /// read, not silently ignored (mirrors the bidder-dir positive
    /// control). The populated purpose also proves non-empty tcf2 maps
    /// render and compile, including the conditional type imports the
    /// account section must then NOT re-import.
    #[test]
    fn child_build_succeeds_with_host_defaults_override_honored() {
        let harness = build_defaults_tempdir(
            "host_defaults_override_ok",
            "product-defaults.toml",
            &product_data_toml(
                &product_host_toml(
                    "enabled = true\ndefault_value = \"1\"\nhost_vendor_id = 12",
                    "\n[gdpr.tcf2.purposes.5]\nenforce_algo = \"basic\"\n\
                 enforce_purpose = \"enabled\"\nenforce_vendors = \"enabled\"\n",
                ),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            status.success(),
            "child cargo build with a valid `[host]` override should \
             succeed, but it failed. stderr:\n{stderr}"
        );
        let generated = read_generated_config(&harness).unwrap();
        let host = generated.initializer("host_default").unwrap();
        assert_eq!(
            integer(some(field(host, &["gdpr", "host_vendor_id"]).unwrap()).unwrap()).unwrap(),
            12
        );
        let purposes = map_entries(field(host, &["gdpr", "tcf2", "purposes"]).unwrap()).unwrap();
        let (_, purpose) = purposes
            .iter()
            .find(|(key, _)| integer(key).unwrap() == 5)
            .unwrap();
        assert!(path_is(
            field(purpose, &["enforce_algo"]).unwrap(),
            &["EnforceAlgo", "Basic"]
        ));
    }

    /// Positive control for the account side: the override must reach
    /// codegen (privacy keep-bits + a populated account purpose map).
    #[test]
    fn child_build_succeeds_with_account_defaults_override_honored() {
        let harness = build_defaults_tempdir(
            "account_defaults_override_ok",
            "product-defaults.toml",
            &product_data_toml(
                &product_account_toml(
                    ACCOUNT_GDPR_BASELINE,
                    "\n[gdpr.purposes.2]\nenforce_algo = \"basic\"\n\
                 enforce_purpose = \"enabled\"\nenforce_vendors = \"unset\"\n\
                 \n[privacy.ipv4]\nanon_keep_bits = 20\n",
                ),
                &host_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            status.success(),
            "child cargo build with a valid `[account]` override should \
             succeed, but it failed. stderr:\n{stderr}"
        );
        let generated = read_generated_config(&harness).unwrap();
        let account = generated.initializer("account_default").unwrap();
        for (family, expected) in [("ipv4", 20), ("ipv6", 56)] {
            assert_eq!(
                integer(field(account, &["privacy", family, "anon_keep_bits"]).unwrap()).unwrap(),
                expected
            );
        }
        let purposes = map_entries(field(account, &["gdpr", "purposes"]).unwrap()).unwrap();
        let (_, purpose) = purposes
            .iter()
            .find(|(key, _)| integer(key).unwrap() == 2)
            .unwrap();
        assert!(path_is(
            field(purpose, &["enforce_algo"]).unwrap(),
            &["EnforceAlgo", "Basic"]
        ));
    }

    #[test]
    fn privacy_parity_command_isolates_unrelated_build_overrides() {
        let harness = harness_dirs("privacy_parity_command");
        let command = privacy_config_parity_command(&harness);
        for variable in [
            "AUCTIONEER__BIDDERS__DIR",
            "AUCTIONEER__GVL__ARCHIVE__DIR",
            "AUCTIONEER__DISCLOSURE__DIR",
        ] {
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == OsStr::new(variable) && value.is_none()),
                "child must clear {variable}",
            );
        }
        assert!(command.get_envs().any(|(key, value)| {
            key == OsStr::new("AUCTIONEER__DEFAULTS__PATH_OVERRIDE")
                && value == Some(harness.fixture("product-defaults.toml").as_os_str())
        }));
        assert!(!command.get_args().any(|arg| arg == OsStr::new("--offline")));
        assert!(
            !command
                .get_envs()
                .any(|(key, _)| key == OsStr::new("CARGO_NET_OFFLINE"))
        );
    }

    #[test]
    fn child_build_executes_privacy_baked_runtime_parity() {
        use serde_json::json;

        let baseline: toml::Value =
            toml::from_str(&product_data_toml(&host_baseline(), &account_baseline()))
                .expect("valid baseline");
        let mut source = serde_json::to_value(baseline).expect("baseline as JSON");
        source["host"]["ccpa"] = json!({"enforce": true});
        source["host"]["lmt"] = json!({"enforce": true});
        source["account"]["ccpa"] = json!({
            "enabled": "enabled", "channel_enabled": {
                "web": "enabled", "app": "disabled", "amp": "unset",
                "video": "disabled", "dooh": "enabled"
            }
        });
        let mut activities = json!({});
        for (activity, default) in [
            ("fetchBids", None),
            ("transmitUfpd", Some(true)),
            ("transmitPreciseGeo", Some(false)),
            ("transmitTid", None),
            ("syncUser", Some(true)),
            ("reportAnalytics", Some(false)),
            ("enrichUfpd", None),
            ("transmitUniqueRequestIds", Some(true)),
        ] {
            let mut config = json!({"rules": [
                {"allow": true, "condition": {
                    "componentName": ["fake_Future\u{212a}", "fake_Alias/\u{df}~", "fake_Future\u{212a}", "fake_quote\"\\\n", format!("fake_{activity}")],
                    "componentType": ["BIDDER", "aNaLyTiCs", "RtD", "GENERAL"]
                }},
                {"allow": false, "condition": {}},
                {"allow": true}
            ]});
            if let Some(value) = default {
                config["default"] = json!(value);
            }
            if activity == "enrichUfpd" {
                config["rules"] = json!([]);
            }
            activities[activity] = config;
        }
        source["account"]["privacy"]["allowactivities"] = activities;
        let harness = build_defaults_tempdir(
            "privacy_config",
            "product-defaults.toml",
            &toml::to_string(&source).expect("serialize the shared fixture"),
        );
        let (status, output) = cargo_test_privacy_config_parity(&harness);
        assert!(
            status.success(),
            "generated privacy values must match runtime resolution: {output}"
        );
        for expected in [
            "running 1 test",
            "test tests::emitted_baked_config_matches_runtime_fixture ... ok",
        ] {
            assert_eq!(
                output.lines().filter(|line| *line == expected).count(),
                1,
                "{output}"
            );
        }
        let summaries = output
            .lines()
            .filter(|line| line.starts_with("test result:"))
            .collect::<Vec<_>>();
        assert_eq!(summaries.len(), 1, "{output}");
        assert!(summaries[0].starts_with("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in "), "{output}");
    }

    // ---- product-data strictness ------------------------------------------

    #[test]
    fn build_rejects_unknown_root_key() {
        // Product data must not acquire an operator/runtime namespace.
        for namespace in ["runtim", "runtime", "baked"] {
            let harness = build_defaults_tempdir(
                "root_unknown_key",
                "product-defaults.toml",
                &format!(
                    "{host}\n{account}\n[{namespace}]\n",
                    host = host_baseline(),
                    account = account_baseline(),
                ),
            );

            let (status, stderr) =
                cargo_build_with_defaults_override(&harness, "product-defaults.toml");
            assert!(
                !status.success(),
                "child cargo build should fail on an unknown ROOT namespace, but it \
                 succeeded. stderr:\n{stderr}"
            );
            assert!(
                stderr.contains("unknown field") && stderr.contains(namespace),
                "stderr should name the unknown root key. Got:\n{stderr}"
            );
        }
    }

    #[test]
    fn build_rejects_unknown_product_namespace() {
        let harness = build_defaults_tempdir(
            "product_unknown_namespace",
            "product-defaults.toml",
            &format!(
                "{source}\n[other]\nx = 1\n",
                source = product_data_toml(&host_baseline(), &account_baseline()),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on a stray product namespace, but \
             it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("unknown field") && stderr.contains("other"),
            "stderr should name the unknown product namespace. Got:\n{stderr}"
        );
    }

    #[test]
    fn build_rejects_missing_product_host() {
        let harness = build_defaults_tempdir(
            "product_missing_host",
            "product-defaults.toml",
            &account_baseline(),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when product `[host]` is absent, but it \
             succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("missing field") && stderr.contains("host"),
            "stderr should name the missing `host` namespace. Got:\n{stderr}"
        );
    }

    #[test]
    fn build_rejects_missing_product_account() {
        let harness = build_defaults_tempdir(
            "product_missing_account",
            "product-defaults.toml",
            &host_baseline(),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when product `[account]` is absent, but it \
             succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("missing field") && stderr.contains("account"),
            "stderr should name the missing `account` namespace. Got:\n{stderr}"
        );
    }

    #[test]
    fn build_rejects_unknown_key_in_baked_host() {
        // Each namespace keeps `deny_unknown_fields` independently:
        // a top-level typo under `[host]` is a build failure,
        // not a silently-defaulted host config.
        let harness = build_defaults_tempdir(
            "baked_host_unknown_key",
            "product-defaults.toml",
            &product_data_toml(
                &format!("{host}\nstatsu_response = \"\"\n", host = host_baseline()),
                &account_baseline(),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on an unknown `[host]` key, but \
             it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("unknown field") && stderr.contains("statsu_response"),
            "stderr should name the misspelled host key. Got:\n{stderr}"
        );
    }

    #[test]
    fn build_rejects_unknown_key_in_baked_account() {
        let harness = build_defaults_tempdir(
            "baked_account_unknown_key",
            "product-defaults.toml",
            &product_data_toml(
                &host_baseline(),
                &format!(
                    "{account}\ndebug_alow = false\n",
                    account = account_baseline()
                ),
            ),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail on an unknown `[account]` key, \
             but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("unknown field") && stderr.contains("debug_alow"),
            "stderr should name the misspelled account key. Got:\n{stderr}"
        );
    }

    #[test]
    fn build_fails_on_missing_required_default() {
        // Defaults stay DATA, not `impl Default for HostConfig`: a
        // required key that vanishes must stop the build rather than
        // resolve to a language-level zero.
        let stripped = host_baseline().replace("cookie_name = \"auctioneer_uid\"\n", "");
        let harness = build_defaults_tempdir(
            "missing_required_default",
            "product-defaults.toml",
            &product_data_toml(&stripped, &account_baseline()),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "child cargo build should fail when a required baked key is absent, \
             but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("missing field") && stderr.contains("cookie_name"),
            "stderr should name the missing required key. Got:\n{stderr}"
        );
    }

    /// `CcpaRaw`, `LmtRaw`, `HostCookieRaw` and `ResourceLimitsRaw` did NOT
    /// carry `deny_unknown_fields` before the fold, so a misspelling
    /// inside any of them was silently ignored and the operator kept
    /// the default — `ccpa.enfroce = true` read as "CCPA enforcement
    /// on" and shipped with it off. The check is PER-STRUCT rather
    /// than per-document: a single top-level fixture would pass while
    /// three of the four stayed broken.
    #[test]
    fn build_rejects_unknown_key_in_each_nested_baked_table() {
        for (label, table, typo) in [
            ("ccpa", "[host.ccpa]\nenforce = false\n", "enfroce = true"),
            ("lmt", "[host.lmt]\nenforce = false\n", "enfroce = true"),
            (
                "host_cookie",
                "[host.host_cookie]\ncookie_name = \"auctioneer_uid\"\nfamily = \"auctioneer\"\nttl_days = 90\n",
                "ttl_dyas = 30",
            ),
            (
                "user_sync",
                "[host.user_sync]\nexternal_url = \"https://sync.example.test\"\n",
                "external_urll = \"https://other.example.test\"",
            ),
            (
                "resource_limits",
                "max_outbound_requests = 100\n",
                "max_outbound_reqeusts = 5",
            ),
        ] {
            let planted = host_baseline().replace(table, &format!("{table}{typo}\n"));
            assert_ne!(
                planted,
                host_baseline(),
                "the `{label}` fixture planted nothing, so the case would prove \
                 nothing",
            );
            let harness = build_defaults_tempdir(
                &format!("nested_unknown_{label}"),
                "product-defaults.toml",
                &product_data_toml(&planted, &account_baseline()),
            );

            let (status, stderr) =
                cargo_build_with_defaults_override(&harness, "product-defaults.toml");
            assert!(
                !status.success(),
                "child cargo build should fail on a misspelled key inside \
                 `[host.{label}]`, but it succeeded. stderr:\n{stderr}"
            );
            let misspelling = typo.split(' ').next().expect("the typo names a key");
            assert!(
                stderr.contains("unknown field") && stderr.contains(misspelling),
                "stderr should name the `{label}` misspelling `{misspelling}`. \
                 Got:\n{stderr}"
            );
        }
    }

    // ---- user-sync generation and collection bounds ------------------

    #[test]
    fn sync_config_child_build_emits_typed_skip_when_from_bidder_toml() {
        let harness = build_bidder_tempdir(
            "typed_skip_when",
            "fake_typed.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://typed.example/openrtb2/auction"

[maintainer]
email = "sync@auctioneer.test"

[userSync]
enabled = false
skipWhen = { gdpr = true, GPPSID = ["02,6", "-1"] }

[userSync.redirect]
url = "https://typed.example/sync?r={{.RedirectURL}}"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(status.success(), "typed skipWhen should build: {stderr}");
        let generated = read_generated_config(&harness).unwrap();
        let bidder = generated.bidder("fake_typed").unwrap().unwrap();
        assert!(!boolean(field(&bidder, &["user_sync", "enabled"]).unwrap()).unwrap());
        assert!(boolean(field(&bidder, &["user_sync", "skip_when", "gdpr"]).unwrap()).unwrap());
        let sections =
            vector(field(&bidder, &["user_sync", "skip_when", "gpp_sid"]).unwrap()).unwrap();
        assert_eq!(
            sections
                .iter()
                .map(string)
                .collect::<syn::Result<Vec<_>>>()
                .unwrap(),
            ["02,6", "-1"]
        );
    }

    #[test]
    fn sync_config_child_build_rejects_legacy_flat_skip_when_shape() {
        let harness = build_bidder_tempdir(
            "flat_skip_when",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://typed.example/openrtb2/auction"

[maintainer]
email = "sync@auctioneer.test"

[userSync]
skipWhen = ["2,6"]
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(
            !status.success(),
            "legacy flat skipWhen should fail: {stderr}"
        );
        assert!(
            stderr.contains("invalid bidder metadata fields"),
            "{stderr}"
        );
    }

    #[test]
    fn sync_config_child_build_rejects_skip_when_sid_collection_above_256() {
        let sid_list = vec!["\"2\""; 257].join(", ");
        let body = format!(
            "implementation = \"fake_fixture_adapter\"\nendpoint = \"https://typed.example/openrtb2/auction\"\n\
             [maintainer]\nemail = \"sync@auctioneer.test\"\n\
             [userSync]\nskipWhen = {{ gdpr = false, gpp_sid = [{sid_list}] }}\n"
        );
        let harness = build_bidder_tempdir("skip_when_sid_overflow", "fake_bad.toml", &body);

        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(!status.success(), "257 SID entries should fail: {stderr}");
        assert!(
            stderr.contains("userSync.skipWhen exceeds its bounds"),
            "{stderr}"
        );
    }

    #[test]
    fn sync_config_child_build_rejects_unsafe_iframe_user_macro() {
        let harness = build_bidder_tempdir(
            "unsafe_iframe_user_macro",
            "fake_bad.toml",
            r#"implementation = "fake_fixture_adapter"
endpoint = "https://typed.example/openrtb2/auction"

[maintainer]
email = "sync@auctioneer.test"

[userSync.iframe]
url = "https://typed.example/sync?r={{.RedirectURL}}"
userMacro = "bad&uid"
"#,
        );

        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(!status.success(), "unsafe userMacro should fail: {stderr}");
        assert!(stderr.contains("userMacro"), "{stderr}");
    }

    #[test]
    fn sync_config_child_build_rejects_overlong_redirect_user_macro() {
        let body = format!(
            "implementation = \"fake_fixture_adapter\"\nendpoint = \"https://typed.example/openrtb2/auction\"\n\
             [maintainer]\nemail = \"sync@auctioneer.test\"\n\
             [userSync.redirect]\nurl = \"https://typed.example/sync?r={{{{.RedirectURL}}}}\"\n\
             userMacro = \"{}\"\n",
            "x".repeat(257)
        );
        let harness = build_bidder_tempdir("overlong_redirect_user_macro", "fake_bad.toml", &body);

        let (status, stderr) = cargo_build_with_bidder_dir(&harness);
        assert!(
            !status.success(),
            "257-byte userMacro should fail: {stderr}"
        );
        assert!(stderr.contains("userMacro"), "{stderr}");
    }

    #[test]
    fn sync_config_child_build_rejects_host_priority_group_collection_above_32() {
        let groups = vec!["[]"; 33].join(", ");
        let host = host_baseline().replace(
            "external_url = \"https://sync.example.test\"\n",
            &format!(
                "external_url = \"https://sync.example.test\"\npriority_groups = [{groups}]\n"
            ),
        );
        let harness = build_defaults_tempdir(
            "sync_priority_overflow",
            "product-defaults.toml",
            &product_data_toml(&host, &account_baseline()),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "33 priority groups should fail: {stderr}"
        );
        assert!(
            stderr.contains("user-sync priority groups exceed bounds"),
            "{stderr}"
        );
    }

    #[test]
    fn sync_config_child_build_rejects_allowed_origins_collection_above_256() {
        let origins = (0_usize..257_usize)
            .map(|index| format!("\"https://sync{index}.example.test\""))
            .collect::<Vec<_>>()
            .join(", ");
        let host = host_baseline().replace(
            "external_url = \"https://sync.example.test\"\n",
            &format!(
                "external_url = \"https://sync.example.test\"\nallowed_origins = [{origins}]\n"
            ),
        );
        let harness = build_defaults_tempdir(
            "sync_allowed_origins_overflow",
            "product-defaults.toml",
            &product_data_toml(&host, &account_baseline()),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            !status.success(),
            "257 allowed origins should fail: {stderr}"
        );
        assert!(stderr.contains("user_sync.allowed_origins"), "{stderr}");
    }

    #[test]
    fn sync_config_child_build_rejects_overlong_cookie_names() {
        let overlong = "x".repeat(129);
        let baseline = host_baseline();
        for (label, host, diagnostic) in [
            (
                "cookie_name",
                baseline.replace(
                    "cookie_name = \"auctioneer_uid\"",
                    &format!("cookie_name = \"{overlong}\""),
                ),
                "host_cookie.cookie_name",
            ),
            (
                "family",
                baseline.replace(
                    "family = \"auctioneer\"",
                    &format!("family = \"{overlong}\""),
                ),
                "host_cookie.family",
            ),
            (
                "optout_name",
                baseline.replace(
                    "ttl_days = 90\n",
                    &format!(
                        "ttl_days = 90\n[host.host_cookie.optout_cookie]\nname = \"{overlong}\"\nvalue = \"1\"\n"
                    ),
                ),
                "host_cookie.optout_cookie",
            ),
        ] {
            assert_ne!(host, baseline, "{label} fixture planted nothing");
            let harness = build_defaults_tempdir(
                &format!("sync_overlong_{label}"),
                "product-defaults.toml",
                &product_data_toml(&host, &account_baseline()),
            );

            let (status, stderr) = cargo_build_with_defaults_override(&harness, "product-defaults.toml");
            assert!(!status.success(), "overlong {label} should fail: {stderr}");
            assert!(stderr.contains(diagnostic), "{label}: {stderr}");
        }
    }

    #[test]
    fn bidder_metadata_derives_implementation_from_owner_and_rejects_conflicts() {
        let pool = target_pool();
        for implementation in [
            None,
            Some("fake_probe"),
            Some("fake_fixture_adapter"),
            Some("Not_A_Key"),
        ] {
            let harness = harness_dirs_in("owned_implementation", &pool);
            let mut definition = toml::Table::new();
            definition.insert("endpoint".into(), "https://fixture.example/openrtb2".into());
            if let Some(key) = implementation {
                definition.insert("implementation".into(), key.into());
            }
            write_owned_definition(
                &harness,
                "fake_probe",
                "fake_PublicProbe",
                None,
                &toml::to_string(&definition).unwrap(),
            )
            .unwrap();
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            if implementation.is_none_or(|key| key == "fake_probe") {
                assert!(status.success(), "owned binding must build: {stderr}");
                let generated = read_generated_config(&harness).unwrap();
                let bindings = generated
                    .static_rows("bidder_implementation_bindings", "BINDINGS")
                    .unwrap();
                let binding = bindings
                    .iter()
                    .find(|row| {
                        string(field(row, &["bidder"]).unwrap()).unwrap() == "fake_PublicProbe"
                    })
                    .unwrap();
                assert_eq!(
                    string(field(binding, &["implementation"]).unwrap()).unwrap(),
                    "fake_probe"
                );
                assert!(generated.bidder("fake_PublicProbe").unwrap().is_some());
            } else {
                assert!(
                    !status.success(),
                    "conflicting binding must refuse: {stderr}"
                );
                assert!(
                    stderr.contains("definitions.implementation must match the owning key"),
                    "{stderr}"
                );
            }
        }
    }

    #[test]
    fn inactive_metadata_bindings_are_validated_before_feature_filtering() {
        let pool = target_pool();
        let mut checked = 0_usize;
        for (implementation, feature, extra, diagnostic) in [
            ("fake_probe", "insecure-bidders", "", None),
            ("Not_A_Key", "insecure-bidders", "", Some("implementation")),
            ("fake_probe", "unknown-feature", "", Some("feature")),
            ("fake_probe", "test-harness-override", "", Some("feature")),
            (
                "fake_probe",
                "insecure-bidders",
                "unexpected = true\n",
                Some("bidder metadata fields"),
            ),
        ] {
            let harness = harness_dirs_in("inactive_binding", &pool);
            write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE).unwrap();
            let source = format!(
                "implementation = {implementation:?}\nrequired_feature = {feature:?}\n{extra}endpoint = 'https://fixture.example/openrtb2'\n"
            );
            write_owned_definition(&harness, "fake_probe", "fake_probe", Some(feature), &source)
                .unwrap();
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            if let Some(message) = diagnostic {
                assert!(
                    !status.success(),
                    "inactive bad binding must fail: {stderr}"
                );
                assert!(stderr.contains(message), "{stderr}");
                assert!(!stderr.contains("missing field"), "{stderr}");
            } else {
                assert!(
                    status.success(),
                    "inactive valid binding must build: {stderr}"
                );
                let generated = read_generated_config(&harness).unwrap();
                let declarations = generated
                    .static_rows("bidder_metadata_declarations", "DECLARATIONS")
                    .unwrap();
                let declaration = declarations
                    .iter()
                    .find(|row| string(field(row, &["bidder"]).unwrap()).unwrap() == "fake_probe")
                    .unwrap();
                assert_eq!(
                    string(field(declaration, &["implementation"]).unwrap()).unwrap(),
                    "fake_probe"
                );
                assert_eq!(
                    string(some(field(declaration, &["required_feature"]).unwrap()).unwrap())
                        .unwrap(),
                    "insecure-bidders"
                );
                assert!(generated.bidder("fake_fixture_adapter").unwrap().is_some());
                assert!(generated.bidder("fake_probe").unwrap().is_none());
            }
            checked += 1;
        }
        assert_eq!(checked, 5);
    }

    #[test]
    fn inactive_metadata_retains_render_time_validation() {
        let pool = target_pool();
        for (field, diagnostic) in [
            ("bidderTimeoutMs = 0\n", "bidderTimeoutMs"),
            ("gvlVendorID = 65536\n", "65535"),
            (
                "endpoint = 'https://fixture.example/{{.Unsupported}}'\n",
                "unsupported",
            ),
        ] {
            let harness = harness_dirs_in("inactive_render_validation", &pool);
            write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE).unwrap();
            let endpoint = if field.starts_with("endpoint") {
                ""
            } else {
                "endpoint = 'https://fixture.example/openrtb2'\n"
            };
            write_owned_definition(
                &harness,
                "fake_probe",
                "fake_probe",
                Some("insecure-bidders"),
                &format!("{endpoint}{field}"),
            )
            .unwrap();
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            assert!(
                !status.success(),
                "inactive invalid metadata must fail: {stderr}"
            );
            assert!(stderr.contains(diagnostic), "{stderr}");
        }
    }

    #[test]
    fn bidder_metadata_parse_errors_never_echo_values() {
        const MARKER: &str = "credential_marker_7d4aa7c59a324b31";
        let pool = target_pool();
        for invalid in [
            format!("api_key = {MARKER:?}\n"),
            format!("bidderTimeoutMs = {MARKER:?}\n"),
            format!("invalid = \"{MARKER}\n"),
        ] {
            let harness = harness_dirs_in("metadata_redaction", &pool);
            write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE).unwrap();
            write_raw_owned_definition(
                &harness,
                &format!("endpoint = 'https://fixture.example/openrtb2'\n{invalid}"),
            );
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            assert!(!status.success(), "malformed metadata must fail: {stderr}");
            assert!(!stderr.contains(MARKER), "metadata values reached the log");
            assert!(
                !stderr.contains("credential_marker"),
                "credential prefix reached the log"
            );
            assert!(
                !stderr.contains("7d4aa7c59a324b31"),
                "credential suffix reached the log"
            );
            assert!(stderr.contains("invalid bidder"), "{stderr}");
        }
    }

    #[test]
    fn bidder_endpoint_macro_errors_never_echo_templates() {
        const MARKER: &str = "endpoint_credential_5b8327de19594fd6";
        let pool = target_pool();
        for endpoint in [
            format!("https://fixture.example/rtb?token={MARKER}&acct={{{{.AccountID"),
            format!("https://fixture.example/{{{{.Host}}}}?token={MARKER}"),
            format!("https://fixture.example/rtb?token={{{{{MARKER}}}}}"),
        ] {
            let harness = harness_dirs_in("endpoint_redaction", &pool);
            write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE).unwrap();
            write_owned_definition(
                &harness,
                "fake_probe",
                "fake_probe",
                Some("insecure-bidders"),
                &format!("endpoint = {endpoint:?}\n"),
            )
            .unwrap();
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            assert!(
                !status.success(),
                "bad endpoint template must fail: {stderr}"
            );
            assert!(!stderr.contains(MARKER), "endpoint values reached the log");
            assert!(
                !stderr.contains("endpoint_credential"),
                "endpoint credential prefix reached the log"
            );
            assert!(
                !stderr.contains("5b8327de19594fd6"),
                "endpoint credential suffix reached the log"
            );
            assert!(stderr.contains("endpoint"), "{stderr}");
        }
    }

    #[test]
    fn bidder_metadata_non_macro_errors_never_echo_values() {
        const MARKER: &str = "semantic_credential_53dbead38cfa4ca9";
        let pool = target_pool();
        for (required_feature, fields) in [
            (
                Some("insecure-bidders"),
                format!("endpoint = 'ftp://fixture.example/?token={MARKER}'\n"),
            ),
            (
                None,
                format!("endpoint = 'https://127.0.0.1/?token={MARKER}'\n"),
            ),
            (
                Some("insecure-bidders"),
                format!("endpoint = 'https://{{{{.{MARKER}}}}}/rtb'\n"),
            ),
            (
                Some("insecure-bidders"),
                format!(
                    "endpoint = 'https://{{{{.Host}}}}/rtb'\nhostAllowlist = ['https://{MARKER}.example']\n"
                ),
            ),
            (
                Some("insecure-bidders"),
                format!(
                    "endpoint = 'https://fixture.example/rtb'\nendpointCompression = '{MARKER}'\n"
                ),
            ),
            (
                Some("insecure-bidders"),
                format!(
                    "endpoint = 'https://fixture.example/rtb'\n[capabilities.site]\nmediaTypes = ['{MARKER}']\n"
                ),
            ),
            (
                Some("insecure-bidders"),
                format!(
                    "endpoint = 'https://fixture.example/rtb'\n[userSync.redirect]\nurl = 'https://user:{MARKER}@sync.example.test/x'\n"
                ),
            ),
            (
                Some("insecure-bidders"),
                format!(
                    "endpoint = 'https://fixture.example/rtb'\n[userSync.redirect]\nurl = 'https://sync.example.test/?unknown={{{{.{MARKER}}}}}'\n"
                ),
            ),
        ] {
            let harness = harness_dirs_in("semantic_redaction", &pool);
            write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE).unwrap();
            write_owned_definition(
                &harness,
                "fake_probe",
                "fake_probe",
                required_feature,
                &fields,
            )
            .unwrap();
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            assert!(!status.success(), "invalid metadata must fail: {stderr}");
            assert!(!stderr.contains(MARKER), "semantic values reached the log");
            assert!(
                !stderr.contains("semantic_credential"),
                "credential prefix reached the log"
            );
            assert!(
                !stderr.contains("53dbead38cfa4ca9"),
                "credential suffix reached the log"
            );
        }
    }

    #[test]
    fn insecure_metadata_requires_its_owning_feature_requirement() {
        let pool = target_pool();
        for feature in [None, Some("insecure-bidders")] {
            let harness = harness_dirs_in("insecure_binding", &pool);
            write_bidder_metadata(&harness, "fake_fixture_adapter.toml", BASELINE_FIXTURE).unwrap();
            write_owned_definition(
                &harness,
                "fake_probe",
                "fake_probe",
                feature,
                "endpoint = 'http://127.0.0.1:8787/openrtb2'\n",
            )
            .unwrap();
            let (status, stderr) = cargo_build_with_bidder_dir(&harness);
            if feature.is_some() {
                assert!(
                    status.success(),
                    "valid inactive insecure binding: {stderr}"
                );
                let generated = read_generated_config(&harness).unwrap();
                assert!(generated.bidder("fake_probe").unwrap().is_none());
            } else {
                assert!(
                    !status.success(),
                    "unconditional insecure binding must fail: {stderr}"
                );
                assert!(
                    stderr.contains("without an insecure-bidders requirement"),
                    "{stderr}"
                );
            }
        }
    }

    #[test]
    fn sync_config_child_build_catalog_metadata_511_512_513() {
        let pool = target_pool();
        // The 512 sentinel control proves defaults parsing is reached only
        // after the metadata count check. At 513 it must never mask that owner.
        for (bidders, malformed_defaults) in
            [(511_usize, false), (512, false), (512, true), (513, true)]
        {
            let harness = harness_dirs_in("sync_catalog_metadata", &pool);
            write_catalog_bidders(&harness, bidders).unwrap();
            assert_eq!(fs::read_dir(&harness.fixture_dir).unwrap().count(), 1);
            let valid_defaults = product_data_toml(&host_baseline(), &account_baseline());
            let defaults = if malformed_defaults {
                format!("metadata_owner_sentinel = true\n{valid_defaults}")
            } else {
                valid_defaults
            };
            fs::write(harness.fixture("defaults.fixture"), defaults).unwrap();

            let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
                &harness,
                Some("defaults.fixture"),
            );
            if bidders == 513 {
                assert!(!status.success(), "513 metadata names must fail: {stderr}");
                assert!(
                    stderr.lines().any(|line| line.trim()
                        == "Error: \"user-sync catalog exceeds 512 names\""),
                    "metadata owner must reject before defaults parsing: {stderr}"
                );
                assert!(!stderr.contains("metadata_owner_sentinel"), "{stderr}");
            } else if malformed_defaults {
                assert!(
                    !status.success(),
                    "the defaults sentinel must fail: {stderr}"
                );
                assert!(
                    stderr.contains("metadata_owner_sentinel") && stderr.contains("unknown field"),
                    "512 metadata names must reach the defaults sentinel: {stderr}"
                );
                assert!(!stderr.contains("user-sync catalog exceeds"), "{stderr}");
            } else {
                assert!(
                    status.success(),
                    "{bidders} metadata names must build: {stderr}"
                );
                let generated = read_generated_config(&harness).unwrap();
                let expected = (0..bidders)
                    .map(|index| format!("fake_catalog{index:03}"))
                    .collect::<Vec<_>>();
                assert_eq!(generated.bidder_names().unwrap(), expected);
            }
        }
    }

    #[test]
    fn sync_config_child_build_catalog_defaults_511_512_513() {
        let pool = target_pool();
        for (bidders, total) in [(447_usize, 511_usize), (448, 512), (449, 513)] {
            let harness = harness_dirs_in("sync_catalog_defaults", &pool);
            write_catalog_bidders(&harness, bidders).unwrap();
            assert_eq!(fs::read_dir(&harness.fixture_dir).unwrap().count(), 1);
            let aliases = (0_usize..64)
                .map(|index| format!("fake_hostalias{index:03} = \"fake_catalog000\""))
                .collect::<Vec<_>>()
                .join("\n");
            let host = product_host_toml(HOST_GDPR_BASELINE, &format!("\n[aliases]\n{aliases}\n"));
            let defaults = product_data_toml(&host, &account_baseline());
            let parsed: toml::Value = toml::from_str(&defaults).unwrap();
            let alias_count = parsed["host"]["aliases"].as_table().unwrap().len();
            assert_eq!(alias_count, 64);
            assert_eq!(bidders + alias_count, total);
            fs::write(harness.fixture("defaults.fixture"), defaults).unwrap();

            let (status, stderr) = cargo_build_with_bidder_dir_and_defaults_override(
                &harness,
                Some("defaults.fixture"),
            );
            if total == 513 {
                assert!(
                    !status.success(),
                    "513 combined catalog names must fail: {stderr}"
                );
                assert!(
                    stderr.lines().any(|line| line.trim()
                        == "Error: \"user-sync catalog exceeds 512 names\""),
                    "host-default owner must reject the combined catalog: {stderr}"
                );
            } else {
                assert!(
                    status.success(),
                    "{total} combined names must build: {stderr}"
                );
                let generated = read_generated_config(&harness).unwrap();
                let expected = (0..bidders)
                    .map(|index| format!("fake_catalog{index:03}"))
                    .collect::<Vec<_>>();
                assert_eq!(generated.bidder_names().unwrap(), expected);
                let alias_entries = map_entries(
                    field(generated.initializer("host_default").unwrap(), &["aliases"]).unwrap(),
                )
                .unwrap();
                assert_eq!(alias_entries.len(), 64);
                let generated_aliases = alias_entries
                    .iter()
                    .map(|(key, value)| Ok((string(key)?, string(value)?)))
                    .collect::<syn::Result<BTreeMap<_, _>>>()
                    .unwrap();
                assert_eq!(generated_aliases.len(), 64);
                for index in 0_usize..64 {
                    assert_eq!(
                        generated_aliases
                            .get(&format!("fake_hostalias{index:03}"))
                            .map(String::as_str),
                        Some("fake_catalog000"),
                        "the overridden defaults must emit alias {index}"
                    );
                }
            }
        }
    }

    #[test]
    fn sync_config_child_build_preserves_explicit_false_zero_empty_sync_defaults() {
        let host_source = host_baseline()
            .replace(
                "ttl_days = 90\n",
                "ttl_days = 90\nmax_cookie_size_bytes = 0\n",
            )
            .replace(
                "external_url = \"https://sync.example.test\"\n",
                "external_url = \"https://sync.example.test\"\npriority_groups = []\n\
                 [host.user_sync.cooperative]\nenabled_by_default = false\n",
            );
        let account_source = format!(
            "{}\n[account.cookie_sync]\ndefault_limit = 0\nmax_limit = 0\n\
             default_coop_sync = false\npriority_groups = []\n",
            account_baseline()
        );
        let harness = build_defaults_tempdir(
            "sync_explicit_empty_defaults",
            "product-defaults.toml",
            &product_data_toml(&host_source, &account_source),
        );

        let (status, stderr) =
            cargo_build_with_defaults_override(&harness, "product-defaults.toml");
        assert!(
            status.success(),
            "explicit zero/false/empty should build: {stderr}"
        );
        let generated = read_generated_config(&harness).unwrap();
        let host = generated.initializer("host_default").unwrap();
        assert_eq!(
            integer(field(host, &["host_cookie", "max_cookie_size_bytes"]).unwrap()).unwrap(),
            0
        );
        let sync = some(field(host, &["user_sync"]).unwrap()).unwrap();
        assert!(!boolean(field(sync, &["cooperative", "enabled_by_default"]).unwrap()).unwrap());
        let account = generated.initializer("account_default").unwrap();
        for name in ["default_limit", "max_limit"] {
            assert_eq!(
                integer(some(field(account, &["cookie_sync", name]).unwrap()).unwrap()).unwrap(),
                0
            );
        }
        assert!(
            !boolean(some(field(account, &["cookie_sync", "default_coop_sync"]).unwrap()).unwrap())
                .unwrap()
        );
        assert!(
            vector(some(field(account, &["cookie_sync", "priority_groups"]).unwrap()).unwrap())
                .unwrap()
                .is_empty()
        );
    }

    // ---- GVL archive gate (emit_gvl_archive) --------------------------

    #[test]
    fn broken_gvl_archive_via_env_override_fails_build() {
        // The fixture's spec3/vendor-list-v100.json deliberately omits
        // the top-level tcfPolicyVersion, so the shared schema
        // validator must fail the build with the exact field named.
        let (status, stderr) = cargo_build_with_gvl_archive_dir("gvl_archive_broken");
        assert!(
            !status.success(),
            "child cargo build against the broken GVL archive fixture should \
             fail, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("tcfPolicyVersion"),
            "stderr should reference the missing tcfPolicyVersion field. \
             Got:\n{stderr}"
        );
    }

    #[test]
    fn archive_manifest_rejects_duplicate_pair() {
        let (status, stderr) = cargo_build_with_gvl_archive_dir("gvl_archive_duplicate_pair");
        assert!(
            !status.success(),
            "child cargo build should fail on a manifest with two (3, 348) \
             entries, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("duplicate") && stderr.contains("348"),
            "stderr should reference the duplicate (spec, list) pair. \
             Got:\n{stderr}"
        );
    }

    #[test]
    fn archive_manifest_rejects_out_of_range_spec() {
        let (status, stderr) = cargo_build_with_gvl_archive_dir("gvl_archive_out_of_range_spec");
        assert!(
            !status.success(),
            "child cargo build should fail on a manifest entry with spec 1, \
             but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("spec 1 is not 2 or 3"),
            "stderr should reference the out-of-range spec. Got:\n{stderr}"
        );
    }

    #[test]
    fn archive_manifest_rejects_traversal_path() {
        let (status, stderr) = cargo_build_with_gvl_archive_dir("gvl_archive_traversal_path");
        assert!(
            !status.success(),
            "child cargo build should fail on a manifest entry whose path \
             escapes the archive dir, but it succeeded. stderr:\n{stderr}"
        );
        assert!(
            stderr.contains("../secrets.json"),
            "stderr should reference the rejected traversal path. Got:\n{stderr}"
        );
    }
}

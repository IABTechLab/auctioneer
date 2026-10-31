//! Shared owned-source workspace, reviewed lock closure, and bounded child runner.

#![cfg(test)]

use rustix::process::{Pid, Signal, kill_process_group};
use serde::Deserialize;
use std::cell::Cell;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read as _, Seek as _, Write as _, stderr};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use toml::Value;

const OUTPUT_LIMIT: u64 = 256 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_mins(5);
const SUITE_TIMEOUT: Duration = Duration::from_mins(15);
const SOURCE_LIMIT: u64 = 64 * 1024 * 1024;
const SCRATCH_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const OWNED_CRATES: &[&str] = &[
    "auctioneer-bidder",
    "auctioneer-config",
    "auctioneer-config-schema",
    "auctioneer-gvl-schema",
    "auctioneer-openrtb",
    "auctioneer-openrtb-catalogue",
    "auctioneer-runtime",
];

#[must_use]
pub struct BuildWorkspace {
    directory: TempDir,
    bidder: PathBuf,
    config: PathBuf,
    target: PathBuf,
    started: Instant,
    runtime: Cell<Duration>,
    peak_logical_bytes: Cell<u64>,
    children: Cell<u32>,
}

impl BuildWorkspace {
    pub fn new_synthetic(probe: &str, probe_source: &str) -> Self {
        let workspace = Self::new(probe, probe_source);
        prepare_synthetic_catalog(workspace.bidder());
        install_synthetic_fake(workspace.bidder());
        workspace
    }

    /// # Panics
    /// Panics if scratch inputs, manifests, or the reviewed lock cannot be prepared.
    pub fn new(probe: &str, probe_source: &str) -> Self {
        Self::with_crates(probe, probe_source, &[])
    }

    /// # Panics
    /// Panics if scratch inputs, manifests, or the reviewed lock cannot be prepared.
    pub fn with_crates(probe: &str, probe_source: &str, extra_crates: &[&str]) -> Self {
        let started = Instant::now();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap();
        let root = source.join("../..").canonicalize().unwrap();
        let original: Value =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let temporary = directory.path().canonicalize().unwrap();
        let (bidder, config) = prepare_workspace(&root, &temporary, &original, extra_crates);
        let mut manifest: Value =
            toml::from_str(&fs::read_to_string(bidder.join("Cargo.toml")).unwrap()).unwrap();
        let mut binary = toml::Table::new();
        binary.insert("name".into(), probe.into());
        binary.insert("path".into(), "src/bin/build_probe.rs".into());
        manifest
            .as_table_mut()
            .unwrap()
            .entry("bin")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .unwrap()
            .push(Value::Table(binary));
        fs::create_dir_all(bidder.join("src/bin")).unwrap();
        fs::write(bidder.join("src/bin/build_probe.rs"), probe_source).unwrap();
        fs::write(
            bidder.join("Cargo.toml"),
            toml::to_string(&manifest).unwrap(),
        )
        .unwrap();
        let reviewed_lock = fs::read_to_string(root.join("Cargo.lock")).unwrap();
        fs::write(temporary.join("Cargo.lock"), &reviewed_lock).unwrap();
        let harness = Self {
            directory,
            bidder,
            config,
            target: temporary.join("target"),
            started,
            runtime: Cell::new(Duration::ZERO),
            peak_logical_bytes: Cell::new(0),
            children: Cell::new(0),
        };
        assert!(
            harness
                .config()
                .starts_with(harness.root().canonicalize().unwrap())
        );
        // Adapt only the workspace package closure, then reject any changed dependency pin.
        require_success(
            &harness.cargo("update", &["--workspace"], false),
            "offline seeded child lock materialization",
        );
        let child_lock = fs::read_to_string(harness.root().join("Cargo.lock")).unwrap();
        verify_child_lock(&child_lock, &reviewed_lock)
            .unwrap_or_else(|error| panic!("build infrastructure failure: {error}"));
        harness
    }

    pub fn root(&self) -> &Path {
        self.directory.path()
    }

    pub fn bidder(&self) -> &Path {
        &self.bidder
    }

    pub fn config(&self) -> &Path {
        &self.config
    }

    pub fn target(&self) -> &Path {
        &self.target
    }

    /// # Panics
    /// Panics on child-runner infrastructure failures or checked accounting overflow.
    pub fn cargo(&self, action: &str, extra: &[&str], locked: bool) -> Output {
        self.cargo_package("auctioneer-bidder", action, extra, locked)
    }

    /// # Panics
    /// Panics on child-runner infrastructure failures or checked accounting overflow.
    pub fn cargo_package(
        &self,
        package: &str,
        action: &str,
        extra: &[&str],
        locked: bool,
    ) -> Output {
        let mut command = Command::new(env!("CARGO"));
        command.env_clear();
        for name in [
            "PATH",
            "HOME",
            "CARGO_HOME",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
            "TMPDIR",
        ] {
            if let Some(value) = env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .current_dir(self.directory.path())
            .env("CARGO_NET_OFFLINE", "true")
            .env("CARGO_TERM_COLOR", "never")
            .env("CARGO_BUILD_JOBS", "2")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("CARGO_PROFILE_DEV_INCREMENTAL", "false")
            .env("CARGO_PROFILE_DEV_STRIP", "none")
            .arg(action)
            .arg("--offline")
            .arg("--manifest-path")
            .arg(self.bidder.join("Cargo.toml"));
        if locked {
            command
                .args(["--locked", "--package", package, "--no-default-features"])
                .arg("--target-dir")
                .arg(&self.target);
        }
        command.args(extra);
        writeln!(stderr(), "build child: cargo {action} {}", extra.join(" ")).unwrap();
        let started = Instant::now();
        let result = bounded_output(&mut command, self.started, || self.sample_disk());
        self.runtime
            .set(self.runtime.get().checked_add(started.elapsed()).unwrap());
        self.children
            .set(self.children.get().checked_add(1).unwrap());
        writeln!(
            stderr(),
            "build child runtime: {:.3}s; sampled peak scratch logical bytes: {}",
            started.elapsed().as_secs_f64(),
            self.peak_logical_bytes.get()
        )
        .unwrap();
        result.unwrap_or_else(|error| {
            panic!("build infrastructure failure running {action}: {error}")
        })
    }

    fn sample_disk(&self) -> io::Result<()> {
        let bytes = logical_bytes(self.directory.path())?;
        self.peak_logical_bytes
            .set(self.peak_logical_bytes.get().max(bytes));
        if bytes > SCRATCH_LIMIT {
            return Err(io::Error::other("scratch logical-byte limit"));
        }
        Ok(())
    }
    pub fn report(&self) {
        let _reported = writeln!(
            stderr(),
            "build suite: {} children; child runtime {:.3}s; suite {:.3}s; sampled peak scratch logical bytes {} (not physical allocation); TempDir owns cleanup",
            self.children.get(),
            self.runtime.get().as_secs_f64(),
            self.started.elapsed().as_secs_f64(),
            self.peak_logical_bytes.get()
        );
    }
}

// Keep the real registry API and discovery include, but omit production bidder
// modules and convenience re-exports from the isolated synthetic build.
fn synthetic_source(source: &str, bidder_module: bool) -> String {
    use syn::parse::{ParseStream, Parser as _};
    let parser = |input: ParseStream<'_>| {
        let mut retained = proc_macro2::TokenStream::new();
        let attributes = input.fork();
        let _attributes = input.call(syn::Attribute::parse_inner)?;
        while attributes.cursor() != input.cursor() {
            retained.extend([attributes.parse::<proc_macro2::TokenTree>()?]);
        }
        while !input.is_empty() {
            let original = input.fork();
            let parsed_item: syn::Item = input.parse()?;
            let mut tokens = proc_macro2::TokenStream::new();
            while original.cursor() != input.cursor() {
                tokens.extend([original.parse::<proc_macro2::TokenTree>()?]);
            }
            let omit = if let syn::Item::Use(import) = &parsed_item {
                if bidder_module {
                    matches!(import.vis, syn::Visibility::Public(_))
                } else {
                    matches!(&import.tree, syn::UseTree::Path(path) if path.ident == "bidders")
                }
            } else {
                matches!(&parsed_item, syn::Item::Mod(module) if bidder_module && module.content.is_none())
            };
            if !omit {
                retained.extend(tokens);
            }
        }
        Ok(retained)
    };
    let mut result = parser.parse_str(source).unwrap().to_string();
    if !bidder_module {
        result.push_str(
            &quote::quote! {
                pub use bidders::{
                    BidderDeclaration, BidderRegistration, accepts_sync_uid,
                    bidder_declarations, compiled_bidders,
                };
            }
            .to_string(),
        );
    }
    result
}

pub(crate) fn prepare_synthetic_catalog(bidder: &Path) {
    let root = bidder.join("src/bidders");
    let mut removed = 0_usize;
    for result in fs::read_dir(&root).unwrap() {
        let path = result.unwrap().path();
        if path.is_dir() && path.join("bidder.toml").is_file() {
            fs::remove_dir_all(path).unwrap();
            removed = removed.checked_add(1).unwrap();
        }
    }
    assert!(
        removed > 0,
        "scratch catalog pruning must not silently do nothing"
    );
    for (path, bidder_module) in [
        (root.join("mod.rs"), true),
        (bidder.join("src/lib.rs"), false),
    ] {
        let source = fs::read_to_string(&path).unwrap();
        fs::write(path, synthetic_source(&source, bidder_module)).unwrap();
    }
}

pub(crate) fn install_synthetic_fake(bidder: &Path) {
    let library_path = bidder.join("src/lib.rs");
    let library = fs::read_to_string(&library_path).unwrap();
    fs::write(
        library_path,
        format!("{library}\nextern crate self as auctioneer_bidder;\npub mod fake_synthetic;\n"),
    )
    .unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/test_support/fake_bidder");
    let mut copied_bytes = 0;
    copy_source_tree(
        &source,
        &bidder.join("src/fake_synthetic"),
        &mut copied_bytes,
    );
}

pub(crate) fn prepare_workspace(
    root: &Path,
    temporary: &Path,
    original: &Value,
    extra_crates: &[&str],
) -> (PathBuf, PathBuf) {
    let crates: Vec<_> = OWNED_CRATES.iter().chain(extra_crates).copied().collect();
    let mut workspace = toml::Table::new();
    for key in ["package", "lints", "dependencies", "resolver"] {
        workspace.insert(key.into(), original["workspace"][key].clone());
    }
    absolute_dependency_paths(workspace.get_mut("dependencies").unwrap(), root);
    let dependencies = workspace
        .get_mut("dependencies")
        .unwrap()
        .as_table_mut()
        .unwrap();
    let mut copied_bytes = 0;
    for name in &crates {
        let relative = format!("crates/{name}");
        let from = root.join(&relative);
        let to = temporary.join(&relative);
        copy_source_tree(&from.join("src"), &to.join("src"), &mut copied_bytes);
        copy_file(
            &from.join("Cargo.toml"),
            &to.join("Cargo.toml"),
            &mut copied_bytes,
        );
        if from.join("benches").is_dir() {
            copy_source_tree(
                &from.join("benches"),
                &to.join("benches"),
                &mut copied_bytes,
            );
        }
        for entry_result in fs::read_dir(&from).unwrap() {
            let entry = entry_result.unwrap();
            let filename = entry.file_name();
            let text = filename.to_str().unwrap();
            if entry.path().extension().is_some_and(|ext| ext == "rs") {
                copy_file(&entry.path(), &to.join(text), &mut copied_bytes);
            }
        }
        dependencies.get_mut(*name).unwrap()["path"] = relative.into();
    }
    let bidder = temporary.join("crates/auctioneer-bidder");
    let config = temporary.join("crates/auctioneer-config");
    copy_source_tree(
        &root.join("crates/auctioneer-config/data"),
        &config.join("data"),
        &mut copied_bytes,
    );
    copy_file(
        &root.join("auctioneer.toml"),
        &temporary.join("auctioneer.toml"),
        &mut copied_bytes,
    );
    writeln!(
        stderr(),
        "owned source closure: {} crates; {copied_bytes} logical bytes",
        crates.len()
    )
    .unwrap();
    workspace.insert(
        "members".into(),
        Value::Array(
            crates
                .iter()
                .map(|name| format!("crates/{name}").into())
                .collect(),
        ),
    );
    let mut document = toml::Table::new();
    document.insert("workspace".into(), Value::Table(workspace));
    for key in ["profile", "patch"] {
        if let Some(original_section) = original.get(key) {
            let mut section = original_section.clone();
            if key == "patch" {
                absolute_dependency_paths(&mut section, root);
            }
            document.insert(key.into(), section);
        }
    }
    fs::write(
        temporary.join("Cargo.toml"),
        toml::to_string(&document).unwrap(),
    )
    .unwrap();
    (bidder, config)
}

#[derive(Deserialize, Eq, PartialEq)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

#[derive(Deserialize)]
struct Lockfile {
    package: Vec<LockPackage>,
}

pub(crate) fn verify_child_lock(child_lock: &str, reviewed_lock: &str) -> Result<(), &'static str> {
    let reviewed: Lockfile =
        toml::from_str(reviewed_lock).map_err(|_error| "invalid reviewed root lock")?;
    let child: Lockfile =
        toml::from_str(child_lock).map_err(|_error| "invalid materialized child lock")?;
    for package in child.package {
        if !reviewed.package.contains(&package) {
            return Err("child lock package is absent from reviewed root lock");
        }
    }
    Ok(())
}

fn absolute_dependency_paths(value: &mut Value, base: &Path) {
    if let Some(table) = value.as_table_mut() {
        if let Some(path) = table.get_mut("path") {
            let absolute = base.join(path.as_str().unwrap()).canonicalize().unwrap();
            *path = absolute.to_str().unwrap().into();
        }
        for (_, child) in table.iter_mut() {
            absolute_dependency_paths(child, base);
        }
    }
}

pub(crate) fn copy_source_tree(source: &Path, destination: &Path, bytes: &mut u64) {
    assert!(
        fs::symlink_metadata(source).unwrap().is_dir(),
        "non-directory owned source input"
    );
    fs::create_dir_all(destination).unwrap();
    for entry_result in fs::read_dir(source).unwrap() {
        let entry = entry_result.unwrap();
        let name = entry.file_name();
        let text = name.to_str().unwrap();
        if text.starts_with('.') || matches!(text, "tests" | "target") {
            continue;
        }
        let kind = entry.file_type().unwrap();
        assert!(
            !kind.is_symlink(),
            "symlink owned source input: {}",
            entry.path().display()
        );
        if kind.is_dir() {
            copy_source_tree(&entry.path(), &destination.join(name), bytes);
        } else {
            copy_file(&entry.path(), &destination.join(name), bytes);
        }
    }
}

fn copy_file(source: &Path, destination: &Path, bytes: &mut u64) {
    let metadata = fs::symlink_metadata(source).unwrap();
    assert!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "nonregular source input: {}",
        source.display()
    );
    assert!(
        matches!(
            source.extension().and_then(|ext| ext.to_str()),
            Some(
                "rs" | "toml" | "json" | "yaml" | "yml" | "md" | "txt" | "source" | "lock" | "png"
            )
        ) || source
            .file_name()
            .is_some_and(|name| name == "LICENSE-extensions" || name == "LICENSE.reference"),
        "unexpected owned input: {}",
        source.display()
    );
    *bytes = bytes.checked_add(metadata.len()).unwrap();
    assert!(*bytes <= SOURCE_LIMIT, "owned source copy limit");
    fs::copy(source, destination).unwrap();
}

fn logical_bytes(root: &Path) -> io::Result<u64> {
    let mut total = 0_u64;
    for entry_result in fs::read_dir(root)? {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let metadata = match entry.path().symlink_metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
            return Err(io::Error::other("nonregular scratch input"));
        }
        if metadata.is_dir() {
            // Cargo can remove a temporary directory between samples.
            match logical_bytes(&entry.path()) {
                Ok(bytes) => {
                    total = total
                        .checked_add(bytes)
                        .ok_or_else(|| io::Error::other("scratch size overflow"))?;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        } else {
            total = total
                .checked_add(metadata.len())
                .ok_or_else(|| io::Error::other("scratch size overflow"))?;
        }
    }
    Ok(total)
}

fn bounded_output(
    command: &mut Command,
    suite_started: Instant,
    mut sample: impl FnMut() -> io::Result<()>,
) -> io::Result<Output> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    let mut child = command.spawn()?;
    let started = Instant::now();
    let mut last_sample: Option<Instant> = None;
    let bounded_result = (|| {
        loop {
            check_output_size(&stdout, &stderr)?;
            if last_sample.is_none_or(|at| at.elapsed() >= Duration::from_secs(1)) {
                sample()?;
                last_sample = Some(Instant::now());
            }
            if started.elapsed() >= COMMAND_TIMEOUT || suite_started.elapsed() >= SUITE_TIMEOUT {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "child Cargo watchdog deadline",
                ));
            }
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    // Even successful Cargo exits must not leave build-script descendants alive.
    let _group_signal = kill_process_group(Pid::from_child(&child), Signal::KILL);
    let _child_signal = child.kill();
    let _reaped = child.wait();
    let status = bounded_result?;
    sample()?;
    check_output_size(&stdout, &stderr)?;
    stdout.rewind()?;
    stderr.rewind()?;
    let mut output = Output {
        status,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    stdout.take(OUTPUT_LIMIT).read_to_end(&mut output.stdout)?;
    stderr.take(OUTPUT_LIMIT).read_to_end(&mut output.stderr)?;
    Ok(output)
}

fn check_output_size(stdout: &File, stderr: &File) -> io::Result<()> {
    if stdout.metadata()?.len() > OUTPUT_LIMIT || stderr.metadata()?.len() > OUTPUT_LIMIT {
        return Err(io::Error::other("child Cargo output limit"));
    }
    Ok(())
}

pub(crate) fn logs(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub(crate) fn require_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "build infrastructure/control failure ({label}): {}\n{}",
        output.status,
        logs(output)
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    const REVIEWED_LOCK: &str = r#"
version = 4

[[package]]
name = "auctioneer-bidder"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.228"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

[[package]]
name = "unneeded-package"
version = "2.0.0"
"#;

    #[test]
    fn child_lock_accepts_reviewed_subset_including_local_packages() {
        let mut child: Value = toml::from_str(REVIEWED_LOCK).unwrap();
        child["package"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(
            verify_child_lock(&toml::to_string(&child).unwrap(), REVIEWED_LOCK),
            Ok(())
        );
    }

    fn reject_changed_lock_field(field: &str, replacement: &str) {
        let mut child: Value = toml::from_str(REVIEWED_LOCK).unwrap();
        child["package"][1][field] = replacement.into();
        assert_eq!(
            verify_child_lock(&toml::to_string(&child).unwrap(), REVIEWED_LOCK),
            Err("child lock package is absent from reviewed root lock"),
            "changed lock field: {field}"
        );
    }

    #[test]
    fn child_lock_rejects_changed_version() {
        reject_changed_lock_field("version", "9.9.9");
    }

    #[test]
    fn child_lock_rejects_changed_source() {
        reject_changed_lock_field("source", "registry+https://unreviewed.example/index");
    }

    #[test]
    fn child_lock_rejects_changed_checksum() {
        reject_changed_lock_field(
            "checksum",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        );
    }

    #[test]
    fn child_lock_rejects_unexpected_package() {
        reject_changed_lock_field("name", "unexpected-package");
    }

    #[test]
    fn synthetic_catalog_prunes_fixture_owners_without_changing_the_source_or_framework() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        for (relative, content) in [
            (
                "src/lib.rs",
                "pub use bidders::FixtureExport;\npub struct Framework;\n",
            ),
            (
                "src/bidders/mod.rs",
                "mod fake_fixture_owner;\npub use fake_fixture_owner::FixtureExport;\ninclude!(concat!(env!(\"OUT_DIR\"), \"/discovery.rs\"));\n",
            ),
            (
                "src/bidders/fake_fixture_owner/bidder.toml",
                "key = \"fake_fixture_owner\"\nfactory = true\n",
            ),
            (
                "src/bidders/fake_fixture_owner/mod.rs",
                "pub(crate) use crate::fake_synthetic::create_bidder;\n",
            ),
            ("src/bidder.rs", "// synthetic framework input\n"),
        ] {
            let path = source.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let mut copied_bytes = 0;
        copy_source_tree(
            &source.path().join("src"),
            &destination.path().join("src"),
            &mut copied_bytes,
        );
        prepare_synthetic_catalog(destination.path());
        assert!(copied_bytes > 0);
        assert!(
            source
                .path()
                .join("src/bidders/fake_fixture_owner/bidder.toml")
                .is_file()
        );
        assert!(
            !destination
                .path()
                .join("src/bidders/fake_fixture_owner")
                .exists()
        );
        assert_eq!(
            fs::read(source.path().join("src/bidder.rs")).unwrap(),
            fs::read(destination.path().join("src/bidder.rs")).unwrap(),
        );
        assert_eq!(
            fs::read_to_string(source.path().join("src/lib.rs")).unwrap(),
            "pub use bidders::FixtureExport;\npub struct Framework;\n"
        );
        let catalog = syn::parse_file(
            &fs::read_to_string(destination.path().join("src/bidders/mod.rs")).unwrap(),
        )
        .unwrap();
        assert_eq!(catalog.items.len(), 1);
        assert!(
            matches!(&catalog.items[0], syn::Item::Macro(item) if item.mac.path.is_ident("include"))
        );
        let library = fs::read_to_string(destination.path().join("src/lib.rs")).unwrap();
        assert!(!library.contains("FixtureExport"));
        assert!(library.contains("Framework"));
    }

    #[test]
    fn owned_copy_excludes_hidden_credentials_tests_and_build_outputs() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        for name in [".git", ".aws", ".env", "target", "tests"] {
            let directory = source.path().join(name);
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("excluded.rs"), "not source").unwrap();
        }
        fs::write(source.path().join("mod.rs"), "// owned\n").unwrap();
        let mut bytes = 0;
        copy_source_tree(source.path(), destination.path(), &mut bytes);
        assert_eq!(bytes, 9);
        assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(destination.path().join("mod.rs")).unwrap(),
            "// owned\n"
        );
    }

    #[test]
    fn owned_copy_preserves_bidder_manifests_schemas_and_captures() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        for (relative, contents) in [
            ("src/bidders/fake_example/mod.rs", "// owned\n"),
            (
                "src/bidders/fake_example/bidder.toml",
                "key = \"fake_example\"\n",
            ),
            ("src/bidders/LICENSE.reference", "captured license"),
            ("src/bidders/fake_example/fixtures/protocol/case.json", "{}"),
            ("src/bidders/fake_example/parameters.json", "{}\n"),
            (
                "src/bidders/fake_example/schemas/shared.schema.json",
                "{}\n",
            ),
            ("src/runtime/fixtures/kept.json", "{}"),
        ] {
            let path = source.path().join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        let mut bytes = 0;
        copy_source_tree(
            &source.path().join("src"),
            &destination.path().join("src"),
            &mut bytes,
        );
        for relative in [
            "src/bidders/fake_example/mod.rs",
            "src/bidders/fake_example/bidder.toml",
            "src/bidders/LICENSE.reference",
            "src/bidders/fake_example/fixtures/protocol/case.json",
            "src/bidders/fake_example/parameters.json",
            "src/bidders/fake_example/schemas/shared.schema.json",
            "src/runtime/fixtures/kept.json",
        ] {
            assert_eq!(
                fs::read(destination.path().join(relative)).unwrap(),
                fs::read(source.path().join(relative)).unwrap()
            );
        }
        assert_eq!(bytes, 9 + 21 + 16 + 2 + 3 + 3 + 2);
    }

    #[test]
    fn owned_copy_preserves_binary_handler_assets_without_changing_bytes() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let asset = Path::new("src/handlers/assets/pixel.png");
        let path = source.path().join(asset);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, [0x89_u8, b'P', b'N', b'G', 0, 0xff]).unwrap();
        let mut bytes = 0;
        copy_source_tree(
            &source.path().join("src"),
            &destination.path().join("src"),
            &mut bytes,
        );
        assert_eq!(
            fs::read(destination.path().join(asset)).unwrap(),
            [0x89_u8, b'P', b'N', b'G', 0, 0xff]
        );
        assert_eq!(bytes, 6);
    }

    #[test]
    fn logical_disk_sample_counts_nested_file_lengths_and_rejects_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("nested")).unwrap();
        fs::write(directory.path().join("one"), b"abc").unwrap();
        fs::write(directory.path().join("nested/two"), b"12345").unwrap();
        assert_eq!(logical_bytes(directory.path()).unwrap(), 8);
        symlink(directory.path().join("one"), directory.path().join("link")).unwrap();
        assert_eq!(
            logical_bytes(directory.path()).unwrap_err().kind(),
            io::ErrorKind::Other
        );
    }
    #[test]
    fn synthetic_library_keeps_real_framework_items_without_bidder_reexports_or_dangling_cfg() {
        let source = r#"
            //! Synthetic framework source.
            pub mod registry;
            #[cfg(feature = "insecure-bidders")]
            pub use bidders::FixtureExport;
            pub use bidders::{BidderDeclaration, FixtureOtherExport};
            pub struct Unconditional;
        "#;
        let result = synthetic_source(source, false);
        let parsed = syn::parse_file(&result).unwrap();
        assert_eq!(parsed.attrs.len(), 1);
        assert_eq!(parsed.items.len(), 3);
        let syn::Item::Struct(item) = &parsed.items[1] else {
            panic!("framework item lost")
        };
        assert_eq!(item.ident, "Unconditional");
        assert!(item.attrs.is_empty());
        assert!(!result.contains("FixtureExport"));
        assert!(!result.contains("FixtureOtherExport"));
        for symbol in [
            "BidderDeclaration",
            "BidderRegistration",
            "accepts_sync_uid",
            "bidder_declarations",
            "compiled_bidders",
        ] {
            assert!(result.contains(symbol));
        }
    }

    #[test]
    fn synthetic_library_preserves_the_registry_admission_wrapper_without_reexporting_it() {
        use quote::ToTokens as _;

        let source =
            fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs")).unwrap();
        let original = syn::parse_file(&source).unwrap();
        let rewritten = syn::parse_file(&synthetic_source(&source, false)).unwrap();
        let wrapper = |file: &syn::File| {
            file.items
                .iter()
                .filter_map(|item| {
                    if let syn::Item::Fn(function) = item
                        && function.sig.ident == "configured_registry"
                    {
                        Some(function.to_token_stream().to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        let expected = wrapper(&original);
        assert_eq!(expected.len(), 1);
        assert_eq!(wrapper(&rewritten), expected);
        for item in &rewritten.items {
            if let syn::Item::Use(import) = item {
                assert!(
                    !import
                        .to_token_stream()
                        .to_string()
                        .contains("configured_registry")
                );
            }
        }
    }

    #[test]
    fn synthetic_catalog_keeps_the_real_registration_contract_and_generated_discovery() {
        let source = r#"
            mod fake_fixture_implementation;
            #[cfg(test)] mod fixture_helper;
            pub use fake_fixture_implementation::FixtureExport;
            include!(concat!(env!("OUT_DIR"), "/discovery.rs"));
            pub struct BidderDeclaration { pub key: &'static str }
            pub struct BidderRegistration { pub key: &'static str }
            pub fn bidder_declarations() -> &'static [BidderDeclaration] { ALL_DECLARATIONS }
        "#;
        let result = synthetic_source(source, true);
        let parsed = syn::parse_file(&result).unwrap();
        assert_eq!(parsed.items.len(), 4);
        assert!(matches!(parsed.items[0], syn::Item::Macro(_)));
        assert!(matches!(parsed.items[1], syn::Item::Struct(_)));
        assert!(matches!(parsed.items[2], syn::Item::Struct(_)));
        assert!(matches!(parsed.items[3], syn::Item::Fn(_)));
        assert!(!result.contains("FixtureExport"));
        assert!(!result.contains("fake_fixture_implementation"));
        assert!(!result.contains("fixture_helper"));
        assert!(result.contains("ALL_DECLARATIONS"));
    }

    #[test]
    fn synthetic_workspace_installs_the_shared_fake_source_verbatim_with_its_crate_alias() {
        use std::collections::BTreeMap;

        fn source_files(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for result in fs::read_dir(directory).unwrap() {
                let entry = result.unwrap();
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path).unwrap();
                if metadata.is_file() {
                    files.insert(
                        path.strip_prefix(root).unwrap().to_owned(),
                        fs::read(path).unwrap(),
                    );
                } else if metadata.is_dir() {
                    source_files(root, &path, files);
                } else {
                    panic!("owned source must be a regular file or directory");
                }
            }
        }

        let directory = tempfile::tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::write(
            directory.path().join("src/lib.rs"),
            "pub struct FrameworkSentinel;\n",
        )
        .unwrap();
        install_synthetic_fake(directory.path());
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/test_support/fake_bidder");
        let mut expected = BTreeMap::new();
        source_files(&source, &source, &mut expected);
        assert!(
            expected.len() > 1,
            "the fixture includes real child modules"
        );
        let destination = directory.path().join("src/fake_synthetic");
        let mut actual = BTreeMap::new();
        if destination.is_dir() {
            source_files(&destination, &destination, &mut actual);
        }
        assert_eq!(
            actual.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>(),
            "the synthetic dependency must retain every owned module"
        );
        for (path, bytes) in expected {
            assert_eq!(actual[&path], bytes, "{}", path.display());
        }
        let library =
            syn::parse_file(&fs::read_to_string(directory.path().join("src/lib.rs")).unwrap())
                .unwrap();
        assert_eq!(library.items.len(), 3);
        assert!(
            matches!(&library.items[0], syn::Item::Struct(item) if item.ident == "FrameworkSentinel")
        );
        assert!(matches!(&library.items[1], syn::Item::ExternCrate(item)
            if item.ident == "self" && item.rename.as_ref().is_some_and(|(_, name)| name == "auctioneer_bidder")));
        assert!(matches!(&library.items[2], syn::Item::Mod(item)
            if item.ident == "fake_synthetic" && item.content.is_none()
                && matches!(item.vis, syn::Visibility::Public(_))));
    }
}

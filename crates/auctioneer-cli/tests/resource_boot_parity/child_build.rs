//! One isolated real CLI build, following the config child-build harness style.

#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};
use serde_json::{Value, from_slice, from_value, json};
use std::collections::{BTreeMap, btree_map::Entry};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{ErrorKind, Read as _};
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::str::from_utf8;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tempfile::{TempDir, tempdir};

pub const MUTATION_ENV: &str = "AUCTIONEER__TEST__RESOURCE_BOOT_PARITY_MUTATION";
const OUTPUT_LIMIT: u64 = 16 << 20_u32;
const BUILD_TIMEOUT: Duration = Duration::from_mins(15);

pub struct Captured {
    pub pid: u32,
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub fn command<P: AsRef<OsStr>>(program: P) -> Command {
    let mut command = Command::new(program);
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
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    command
}

struct ChildGuard(Child);

#[expect(clippy::missing_trait_methods, reason = "Drop::pin_drop is unstable")]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Ok(raw) = i32::try_from(self.0.id())
            && let Some(pid) = Pid::from_raw(raw)
        {
            let _kill = kill_process_group(pid, Signal::KILL);
        }
        let _kill = self.0.kill();
        let _reap = self.0.wait();
    }
}

pub fn run(command: &mut Command, timeout: Duration, limit: u64) -> Captured {
    let logs = tempdir().unwrap();
    let stdout = logs.path().join("stdout");
    let stderr = logs.path().join("stderr");
    command
        .stdout(File::create(&stdout).unwrap())
        .stderr(File::create(&stderr).unwrap());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = ChildGuard(command.spawn().unwrap());
    let pid = child.0.id();
    let started = Instant::now();
    let status = loop {
        for path in [&stdout, &stderr] {
            assert!(
                fs::metadata(path).unwrap().len() <= limit,
                "child {pid}: output overflow"
            );
        }
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < timeout,
            "child {pid}: timeout after {timeout:?}"
        );
        sleep(Duration::from_millis(20));
    };
    // Kill descendants before reading; no pipe-reader thread can outlive a timeout.
    drop(child);
    let read = |path| {
        let mut bytes = Vec::new();
        File::open(path)
            .unwrap()
            .take(limit.checked_add(1).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(
            u64::try_from(bytes.len()).unwrap() <= limit,
            "child {pid}: output overflow"
        );
        bytes
    };
    Captured {
        pid,
        status,
        stdout: read(stdout),
        stderr: read(stderr),
    }
}

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let output = run(
        command("git")
            .current_dir(root)
            .args(["--no-replace-objects"])
            .args(args),
        Duration::from_secs(60),
        OUTPUT_LIMIT,
    );
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn copy_source(workspace: &Path, source: &Path) {
    // Object alternates are read-only. Neither the original index nor any
    // active-tree file is changed, including when exercising red controls.
    let cloned = run(
        command("git")
            .args(["clone", "--shared", "--no-checkout", "--quiet", "--"])
            .arg(workspace)
            .arg(source),
        Duration::from_secs(60),
        OUTPUT_LIMIT,
    );
    assert!(
        cloned.status.success(),
        "{}",
        String::from_utf8_lossy(&cloned.stderr)
    );
    git(source, &["read-tree", "HEAD"]);
    let paths = git(
        workspace,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    );
    let mut copied = 0_u64;
    let mut count = 0_u32;
    for bytes in paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = Path::new(from_utf8(bytes).unwrap());
        assert!(
            path.components()
                .all(|part| matches!(part, Component::Normal(_)))
        );
        assert!(!path.starts_with(".git"));
        let original = workspace.join(path);
        let metadata = match fs::symlink_metadata(&original) {
            Ok(value) => value,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => panic!("{}: {error}", original.display()),
        };
        assert!(
            metadata.is_file(),
            "source must be a regular file: {}",
            path.display()
        );
        assert!(metadata.len() <= 32 << 20_u32);
        copied = copied.checked_add(metadata.len()).unwrap();
        count = count.checked_add(1).unwrap();
        assert!(
            copied <= 256 << 20_u32 && count <= 20_000,
            "source copy bound"
        );
        let destination = source.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(original, destination).unwrap();
    }
}

fn inject_adapter(source: &Path, name: &str) {
    let adapter_directory = source.join(format!("crates/auctioneer-adapter-{name}/src"));
    let adapter = adapter_directory.join("envelope.rs");
    let original = fs::read_to_string(&adapter).unwrap();
    let mut parsed = syn::parse_file(&original).unwrap();
    let mut replacements = 0_usize;
    for item in &mut parsed.items {
        if let syn::Item::Fn(function) = item
            && function.sig.ident == "resource_profile"
        {
            *function.block = syn::parse_quote!({
                use auctioneer_core::test_support::boot_parity_fixture;
                boot_parity_fixture::profile_for_adapter(
                    &std::env::var(boot_parity_fixture::CASE_ENV).unwrap(),
                    #name,
                )
            });
            replacements = replacements.checked_add(1).unwrap();
        }
    }
    assert_eq!(replacements, 1);
    fs::write(adapter, quote::quote!(#parsed).to_string()).unwrap();
}

fn enable_host_and_target_test_support(source: &Path) {
    // Boot's integration target enables these helpers through dev-dependencies.
    // Select the same complete fixture graph for the scratch CLI, including
    // runtime's host build, without weakening owner-feature comparisons.
    let path = source.join("Cargo.toml");
    let mut manifest: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    for owner in [
        "auctioneer-core",
        "auctioneer-bidder",
        "auctioneer-exchange",
        "auctioneer-privacy",
        "auctioneer-runtime",
    ] {
        let dependency = manifest["workspace"]["dependencies"][owner]
            .as_table_mut()
            .unwrap();
        let features = dependency
            .entry("features")
            .or_insert_with(|| toml::Value::Array(Vec::new()))
            .as_array_mut()
            .unwrap();
        let feature = toml::Value::String("test-support".into());
        if !features.contains(&feature) {
            features.push(feature);
        }
    }
    fs::write(path, toml::to_string(&manifest).unwrap()).unwrap();
}

fn inject_cli_controls(source: &Path) {
    // Fixed, scratch-only red controls share the single artifact. They
    // perturb the actual CLI row resolver, never the boot-side receipt.
    let owner = source.join("crates/auctioneer-cli/src/bidders/resources/mod.rs");
    let original = fs::read_to_string(&owner).unwrap();
    let mut syntax = syn::parse_file(&original).unwrap();
    let needle: syn::Stmt = syn::parse_quote! {
        let inputs = resolve_budget_inputs(boot.limits)?;
    };
    let marker = MUTATION_ENV;
    let controls: syn::Block = syn::parse_quote!({
        let mut inputs = inputs;
        let corrupted = match std::env::var(#marker).ok().as_deref() {
            None => None,
            Some("raw_zero") => { inputs.limits = *boot.limits; Some("raw_zero") },
            Some("zero_baked") => { inputs.baked_config_bytes = 0; Some("zero_baked") },
            Some("raw_zero_cloudflare") if adapter == AdapterId::Cloudflare => {
                inputs.limits = *boot.limits; Some("raw_zero_cloudflare")
            },
            Some("zero_baked_spin") if adapter == AdapterId::Spin => {
                inputs.baked_config_bytes = 0; Some("zero_baked_spin")
            },
            Some("raw_zero_cloudflare" | "zero_baked_spin") => None,
            Some(_) => return Err(Error::ResourceInput),
        };
        if let Some(control) = corrupted {
            eprintln!("RESOURCE_PARITY_MUTATION {control} {adapter:?}");
        }
    });
    let mut matched = 0_usize;
    for item in &mut syntax.items {
        if let syn::Item::Fn(function) = item
            && function.sig.ident == "row"
        {
            let positions: Vec<_> = function
                .block
                .stmts
                .iter()
                .enumerate()
                .filter(|(_, statement)| {
                    quote::quote!(#statement).to_string() == quote::quote!(#needle).to_string()
                })
                .map(|(index, _)| index.checked_add(1).unwrap())
                .collect();
            assert_eq!(positions.len(), 1);
            function
                .block
                .stmts
                .splice(positions[0]..positions[0], controls.stmts.clone());
            matched = matched.checked_add(1).unwrap();
        }
    }
    assert_eq!(matched, 1);
    fs::write(owner, quote::quote!(#syntax).to_string()).unwrap();
}

pub struct ScratchCli {
    directory: TempDir,
    executable: PathBuf,
    boot_executable: PathBuf,
}

impl ScratchCli {
    pub fn source(&self) -> PathBuf {
        self.directory.path().join("source")
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn boot_executable(&self) -> &Path {
        &self.boot_executable
    }

    pub fn build() -> Self {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        // The staging guard rejects symlink ancestors, including macOS /var.
        let directory = tempfile::tempdir_in(env::temp_dir().canonicalize().unwrap()).unwrap();
        let source = directory.path().join("source");
        let target = directory.path().join("target");
        copy_source(&workspace, &source);
        enable_host_and_target_test_support(&source);
        for adapter in ["axum", "cloudflare", "fastly", "spin"] {
            inject_adapter(&source, adapter);
        }
        inject_cli_controls(&source);

        // Match the boot probe's dev support without enabling conformance.
        let output = run(
            cargo_command(&source, &target).args([
                "build",
                "--offline",
                "--locked",
                "--jobs",
                "2",
                "-p",
                "auctioneer-cli",
                "--bin",
                "auctioneer-cli",
                "--features",
                "auctioneer-core/test-support,auctioneer-privacy/test-support,auctioneer-runtime/test-support",
                "--message-format=json",
            ]),
            BUILD_TIMEOUT,
            OUTPUT_LIMIT,
        );
        assert!(
            output.status.success(),
            "scratch CLI build failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        let executables: Vec<PathBuf> = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| from_slice::<Value>(line).ok())
            .filter(|event| {
                event["reason"] == "compiler-artifact"
                    && event["target"]["name"] == "auctioneer-cli"
                    && event["target"]["kind"] == json!(["bin"])
            })
            .filter_map(|event| event["executable"].as_str().map(PathBuf::from))
            .collect();
        assert_eq!(
            executables.len(),
            1,
            "Cargo must identify the actual CLI executable"
        );
        let executable = executables.into_iter().next().unwrap();
        assert!(executable.starts_with(&target) && executable.is_file());
        let boot_executable = build_boot_probe(&source, &target, &owner_features(&output.stdout));
        Self {
            directory,
            executable,
            boot_executable,
        }
    }
}

fn cargo_command(source: &Path, target: &Path) -> Command {
    let mut cargo = command(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    cargo
        .current_dir(source)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_DEV_INCREMENTAL", "false")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_INCREMENTAL", "false");
    cargo
}

fn owner_features(output: &[u8]) -> BTreeMap<String, (String, Vec<String>)> {
    let owners = [
        "auctioneer_core",
        "auctioneer_config_data",
        "auctioneer_bidder",
        "auctioneer_exchange",
        "auctioneer_privacy",
        "auctioneer_runtime",
    ];
    let mut receipts = BTreeMap::new();
    for event in output
        .split(|byte| *byte == b'\n')
        .filter_map(|line| from_slice::<Value>(line).ok())
    {
        if event["reason"] != "compiler-artifact" || event["target"]["kind"] != json!(["lib"]) {
            continue;
        }
        let name = event["target"]["name"].as_str().unwrap();
        if !owners.contains(&name) {
            continue;
        }
        let package = event["package_id"].as_str().unwrap().to_owned();
        let mut features: Vec<String> = from_value(event["features"].clone()).unwrap();
        features.sort();
        match receipts.entry(name.to_owned()) {
            Entry::Vacant(entry) => {
                entry.insert((package, features));
            }
            Entry::Occupied(entry) => {
                assert_eq!(
                    entry.get(),
                    &(package, features),
                    "resource owner source and features must agree across compilation units"
                );
            }
        }
    }
    assert_eq!(
        receipts.len(),
        owners.len(),
        "both Cargo builds must report every resource owner's actual source and features"
    );
    receipts
}

fn build_boot_probe(
    source: &Path,
    target: &Path,
    cli_features: &BTreeMap<String, (String, Vec<String>)>,
) -> PathBuf {
    let output = run(
        cargo_command(source, target).args([
            "test",
            "--no-run",
            "--offline",
            "--locked",
            "--jobs",
            "2",
            "-p",
            "auctioneer-core",
            "--test",
            "boot_strict_parity",
            "--message-format=json",
        ]),
        BUILD_TIMEOUT,
        OUTPUT_LIMIT,
    );
    assert!(
        output.status.success(),
        "existing boot probe build failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        &owner_features(&output.stdout),
        cli_features,
        "scratch CLI and boot probe must share effective source packages and features, regardless of the outer workspace build"
    );
    let executables: Vec<PathBuf> = output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| from_slice::<Value>(line).ok())
        .filter(|event| {
            event["reason"] == "compiler-artifact"
                && event["target"]["name"] == "boot_strict_parity"
                && event["target"]["kind"] == json!(["test"])
                && event["profile"]["test"] == true
        })
        .filter_map(|event| event["executable"].as_str().map(PathBuf::from))
        .collect();
    assert_eq!(
        executables.len(),
        1,
        "Cargo must identify the existing boot test executable"
    );
    let executable = executables.into_iter().next().unwrap();
    assert!(executable.starts_with(target) && executable.is_file());
    executable
}

#[cfg(test)]
mod tests {
    use super::{
        enable_host_and_target_test_support, inject_adapter, inject_cli_controls, owner_features,
    };
    use serde_json::{Value, json};
    use std::{fs, path::Path};

    #[test]
    fn scratch_cli_controls_mutate_the_current_budget_resolver() {
        let scratch = tempfile::tempdir().unwrap();
        let relative = "crates/auctioneer-cli/src/bidders/resources/mod.rs";
        let owner = scratch.path().join(relative);
        fs::create_dir_all(owner.parent().unwrap()).unwrap();
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        fs::copy(workspace.join(relative), &owner).unwrap();
        inject_cli_controls(scratch.path());
        let syntax = syn::parse_file(&fs::read_to_string(owner).unwrap()).unwrap();
        let mutated = syntax
            .items
            .iter()
            .find_map(|item| {
                if let syn::Item::Fn(function) = item
                    && function.sig.ident == "row"
                {
                    Some(function)
                } else {
                    None
                }
            })
            .unwrap();
        let source = quote::quote!(#mutated).to_string();
        assert_eq!(source.matches("let mut inputs = inputs").count(), 1);
        for control in [
            "raw_zero",
            "zero_baked",
            "raw_zero_cloudflare",
            "zero_baked_spin",
        ] {
            assert!(source.contains(control), "missing {control}");
        }
    }

    #[test]
    fn scratch_test_support_aligns_every_owner_and_preserves_dependency_properties() {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("Cargo.toml");
        let original: toml::Value = toml::from_str(
            "
[workspace.dependencies.auctioneer-runtime]
path = 'crates/runtime'
default-features = false
features = ['fake_existing']
[workspace.dependencies.auctioneer-core]
path = 'crates/core'
[workspace.dependencies.auctioneer-bidder]
path = 'crates/bidder'
[workspace.dependencies.auctioneer-exchange]
path = 'crates/exchange'
[workspace.dependencies.auctioneer-privacy]
path = 'crates/privacy'
[workspace.dependencies.fake-unrelated]
path = 'crates/unrelated'
features = ['fake_unrelated']
",
        )
        .unwrap();
        fs::write(&path, toml::to_string(&original).unwrap()).unwrap();
        enable_host_and_target_test_support(scratch.path());
        let first = fs::read_to_string(&path).unwrap();
        enable_host_and_target_test_support(scratch.path());
        assert_eq!(fs::read_to_string(&path).unwrap(), first);
        let mut expected = original;
        expected["workspace"]["dependencies"]["auctioneer-runtime"]["features"]
            .as_array_mut()
            .unwrap()
            .push(toml::Value::String("test-support".into()));
        for owner in [
            "auctioneer-core",
            "auctioneer-bidder",
            "auctioneer-exchange",
            "auctioneer-privacy",
        ] {
            expected["workspace"]["dependencies"][owner]
                .as_table_mut()
                .unwrap()
                .insert(
                    "features".into(),
                    toml::Value::Array(vec![toml::Value::String("test-support".into())]),
                );
        }
        assert_eq!(toml::from_str::<toml::Value>(&first).unwrap(), expected);
    }

    #[test]
    fn scratch_adapters_call_the_canonical_boot_fixture_without_copying_source() {
        let scratch = tempfile::tempdir().unwrap();
        let root = scratch.path();
        for name in ["axum", "cloudflare", "fastly", "spin"] {
            let directory = root.join(format!("crates/auctioneer-adapter-{name}/src"));
            fs::create_dir_all(&directory).unwrap();
            let adapter = directory.join("envelope.rs");
            let original = quote::quote! {
                pub fn resource_profile() -> ResourceProfile {
                    unreachable!()
                }
            };
            fs::write(&adapter, original.to_string()).unwrap();
            inject_adapter(root, name);

            let source = fs::read_to_string(adapter).unwrap();
            let syntax = syn::parse_file(&source).unwrap();
            let modules: Vec<_> = syntax
                .items
                .iter()
                .filter_map(|item| {
                    if let syn::Item::Mod(module) = item {
                        Some(module)
                    } else {
                        None
                    }
                })
                .collect();
            assert!(modules.is_empty(), "the fixture is owned by its library");
            let expected: syn::File = syn::parse_quote! {
                pub fn resource_profile() -> ResourceProfile {
                    use auctioneer_core::test_support::boot_parity_fixture;
                    boot_parity_fixture::profile_for_adapter(
                        &std::env::var(boot_parity_fixture::CASE_ENV).unwrap(),
                        #name,
                    )
                }
            };
            assert_eq!(
                quote::quote!(#syntax).to_string(),
                quote::quote!(#expected).to_string()
            );
            assert!(!directory.join("envelope").exists());
            assert!(!directory.join("resource_boot_parity_fixture.rs").exists());
        }
    }

    fn events() -> Vec<Value> {
        [
            "auctioneer_core",
            "auctioneer_config_data",
            "auctioneer_bidder",
            "auctioneer_exchange",
            "auctioneer_privacy",
            "auctioneer_runtime",
        ]
        .into_iter()
        .map(|name| {
            json!({
                "reason": "compiler-artifact",
                "target": {"kind": ["lib"], "name": name},
                "package_id": format!("path+file:///reviewed-source/{name}#0.1.0"),
                "features": ["alpha", "beta"],
            })
        })
        .collect()
    }

    fn encode(events: &[Value]) -> Vec<u8> {
        let mut output = Vec::new();
        for event in events {
            serde_json::to_writer(&mut output, event).unwrap();
            output.push(b'\n');
        }
        output
    }

    #[test]
    fn identical_host_and_target_units_preserve_complete_resource_owner_identity() {
        let mut units = events();
        let expected = owner_features(&encode(&units));
        assert_eq!(expected.len(), 6);
        let mut runtime = units.last().unwrap().clone();
        runtime["features"] = json!(["beta", "alpha"]);
        units.push(runtime);
        assert_eq!(owner_features(&encode(&units)), expected);
    }

    #[test]
    #[should_panic(
        expected = "resource owner source and features must agree across compilation units"
    )]
    fn duplicate_owner_with_a_different_source_refuses() {
        let mut units = events();
        let mut changed = units.last().unwrap().clone();
        changed["package_id"] = json!("path+file:///other-source/auctioneer_runtime#0.1.0");
        units.push(changed);
        owner_features(&encode(&units));
    }

    #[test]
    #[should_panic(
        expected = "resource owner source and features must agree across compilation units"
    )]
    fn duplicate_owner_with_different_features_refuses() {
        let mut units = events();
        let mut changed = units.last().unwrap().clone();
        changed["features"] = json!(["alpha"]);
        units.push(changed);
        owner_features(&encode(&units));
    }

    #[test]
    #[should_panic(
        expected = "both Cargo builds must report every resource owner's actual source and features"
    )]
    fn duplicate_units_cannot_replace_a_missing_resource_owner() {
        let mut units = events();
        units.pop().unwrap();
        units.push(units.last().unwrap().clone());
        owner_features(&encode(&units));
    }
}

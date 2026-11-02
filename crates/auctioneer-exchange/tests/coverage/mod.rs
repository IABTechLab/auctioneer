//! Scratch-workspace ownership and exact compiler-diagnostic matching.

use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::ops::Range;
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{self, Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const ADAPTER: &str = "crates/auctioneer-exchange/src/scrub/fields.rs";
const CHECK_TIMEOUT: Duration = Duration::from_mins(5);
const MAX_SIBLING_ROUNDS: usize = 16;
static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

pub(super) struct ModelMutation {
    pub model: &'static str,
    pub source: &'static str,
    pub field: &'static str,
    pub sparse: bool,
}

impl ModelMutation {
    pub(super) fn insert_into(&self, source: &str) -> String {
        let anchor = format!("pub struct {} {{\n", self.model);
        assert_eq!(source.matches(&anchor).count(), 1, "unique model anchor");
        assert!(!source.contains(self.field), "mutation field must be new");
        let (field_type, omit_if) = if self.sparse {
            ("JsonField<String>", "JsonField::is_missing")
        } else {
            ("Option<String>", "Option::is_none")
        };
        // The exact, unique struct opener is the only changed source location.
        source.replacen(
            &anchor,
            &format!(
                "{anchor}    #[serde(default, skip_serializing_if = \"{omit_if}\")]\n    pub {}: {field_type},\n",
                self.field
            ),
            1,
        )
    }
}

struct ScratchDir(PathBuf);

impl ScratchDir {
    #[expect(
        clippy::create_dir,
        reason = "atomically claim a fresh path; never reuse and later delete another owner's directory"
    )]
    fn new() -> Self {
        let base = env::temp_dir()
            .canonicalize()
            .expect("resolve the temporary source root");
        loop {
            let nonce = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
            let path = base.join(format!(
                "auctioneer-disclosure-model-{}-{nonce}",
                process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => panic!("create scratch directory: {error}"),
            }
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.0);
    }
}

pub(super) struct ScratchWorkspace {
    scratch: ScratchDir,
    workspace: PathBuf,
    target: PathBuf,
    toolchain: String,
    host: String,
}

impl ScratchWorkspace {
    pub(super) fn new() -> Self {
        let original = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("exchange belongs to the workspace crates directory");
        let scratch = ScratchDir::new();
        let workspace = scratch.0.join("workspace");
        let target = scratch.0.join("target");
        fs::create_dir_all(&workspace).expect("create copied workspace");
        for name in [
            "Cargo.toml",
            "Cargo.lock",
            "auctioneer.toml",
            ".tool-versions",
        ] {
            fs::copy(original.join(name), workspace.join(name)).expect("copy workspace input");
        }
        copy_tree(&original.join("crates"), &workspace.join("crates"));
        // The root manifest's local compiler patch is part of the source
        // closure even when the checked library does not invoke that compiler.
        copy_tree(&original.join("vendor"), &workspace.join("vendor"));
        let versions = fs::read_to_string(workspace.join(".tool-versions"))
            .expect("read the existing repository toolchain pin");
        let toolchain = versions
            .lines()
            .find_map(|line| line.strip_prefix("rust "))
            .expect("repository pins its Rust toolchain")
            .trim()
            .to_owned();
        let mut rustc = Command::new(rustc_path());
        rustc.arg("-vV").current_dir(&workspace);
        isolate_environment(&mut rustc, &toolchain);
        let compiler = run_bounded(rustc, &scratch.0, "compiler");
        assert!(compiler.status.success(), "host compiler probe: {compiler}");
        let host = compiler
            .stdout
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .expect("rustc reports its native host triple")
            .to_owned();
        assert!(!host.starts_with("wasm"), "this is a native host harness");
        Self {
            scratch,
            workspace,
            target,
            toolchain,
            host,
        }
    }

    fn check(&self) -> CheckOutput {
        run_bounded(
            check_command(&self.workspace, &self.target, &self.toolchain, &self.host),
            &self.scratch.0,
            "check",
        )
    }

    pub(super) fn assert_checks(&self, label: &str) {
        let output = self.check();
        assert!(output.status.success(), "{label} failed: {output}");
        let messages: Vec<Value> = output
            .stdout
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        assert!(
            messages.iter().any(|message| {
                message["reason"] == "build-finished" && message["success"] == true
            }),
            "{label} lacks a successful Cargo build-finished message: {output}"
        );
        let library = self.workspace.join("crates/auctioneer-exchange/src/lib.rs");
        assert!(
            messages.iter().any(|message| {
                message["reason"] == "compiler-artifact"
                    && message["target"]["name"] == "auctioneer_exchange"
                    && message["target"]["src_path"].as_str() == library.to_str()
                    && message["filenames"].as_array().is_some_and(|filenames| {
                        filenames.iter().filter_map(Value::as_str).any(|name| {
                            let path = Path::new(name);
                            path.starts_with(&self.target) && path.is_file()
                        })
                    })
            }),
            "{label} must check the copied exchange library and emit an artifact: {output}"
        );
    }

    pub(super) fn assert_missing_coverage(&self, mutation: &ModelMutation) {
        let path = self.workspace.join(mutation.source);
        let original = fs::read_to_string(&path).expect("read copied model source");
        fs::write(&path, mutation.insert_into(&original)).expect("mutate only copied source");
        let mut originals = BTreeMap::new();
        // Dependency guards can fail before rustc reaches the exchange. Only
        // acknowledge their exact new-field refusal in this disposable tree.
        let mut output = self.check();
        for _ in 0..MAX_SIBLING_ROUNDS {
            if output.status.success()
                || has_missing_field_diagnostic(&output.stdout, mutation.model, mutation.field)
            {
                break;
            }
            let edits = sibling_edits(&self.workspace, &output.stdout, mutation)
                .unwrap_or_else(|error| panic!("unrelated compiler failure: {error}\n{output}"));
            apply_sibling_edits(edits, &mut originals)
                .unwrap_or_else(|error| panic!("unsafe sibling suggestion: {error}\n{output}"));
            output = self.check();
        }
        fs::write(&path, original).expect("restore copied model before the next case");
        restore_siblings(originals);
        assert!(
            !output.status.success(),
            "{} gained an unclassified field without refusing compilation: {output}",
            mutation.model
        );
        assert!(
            has_missing_field_diagnostic(&output.stdout, mutation.model, mutation.field),
            "{} must fail with E0027 for `{}` in {ADAPTER}, not an unrelated failure: {output}",
            mutation.model,
            mutation.field
        );
    }
}

#[expect(
    clippy::filetype_is_file,
    reason = "reject special files as well as symlinks before copying hermetic source bytes"
)]
fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create copied source directory");
    for entry_result in fs::read_dir(source).expect("read workspace source directory") {
        let entry = entry_result.expect("read workspace source entry");
        let name = entry.file_name();
        if ["target", ".git", ".cargo", ".claude", ".codex"]
            .iter()
            .any(|excluded| name == *excluded)
        {
            continue;
        }
        let kind = entry.file_type().expect("inspect copied entry");
        assert!(
            !kind.is_symlink(),
            "scratch sources must not alias live files"
        );
        let copied = destination.join(&name);
        if kind.is_dir() {
            copy_tree(&entry.path(), &copied);
        } else {
            assert!(kind.is_file(), "only regular source files can be copied");
            fs::copy(entry.path(), copied).expect("copy source bytes, not hard links");
        }
    }
}

fn rustc_path() -> PathBuf {
    Path::new(env!("CARGO")).with_file_name(format!("rustc{}", env::consts::EXE_SUFFIX))
}

fn isolate_environment(command: &mut Command, toolchain: &str) {
    for (key, _) in env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("AUCTIONEER__")
            || name.starts_with("CARGO_TARGET_")
            || name.starts_with("CARGO_PROFILE_")
        {
            command.env_remove(key);
        }
    }
    for name in [
        "CARGO_BUILD_TARGET",
        "CARGO_TARGET_DIR",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_ENCODED_RUSTDOCFLAGS",
        "RUSTFLAGS",
        "RUSTDOCFLAGS",
        "CARGO_BUILD_RUSTFLAGS",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(name);
    }
    command
        .env("RUSTUP_TOOLCHAIN", toolchain)
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env("RUSTC", rustc_path())
        .env("RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", "")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_TERM_COLOR", "never");
}

fn check_command(workspace: &Path, target: &Path, toolchain: &str, host: &str) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .args([
            "check",
            "--locked",
            "--offline",
            "--lib",
            "-p",
            "auctioneer-exchange",
            "--jobs",
            "1",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(workspace.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target)
        .arg("--target")
        .arg(host)
        .current_dir(workspace);
    isolate_environment(&mut command, toolchain);
    command
}

fn has_missing_field_diagnostic(stdout: &str, model: &str, field: &str) -> bool {
    let expected = format!("pattern does not mention field `{field}`");
    let pattern = format!("{model} {{");
    stdout.lines().any(|line| {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        let diagnostic = &message["message"];
        message["reason"] == "compiler-message"
            && message["target"]["name"] == "auctioneer_exchange"
            && diagnostic["level"] == "error"
            && diagnostic["code"]["code"] == "E0027"
            && diagnostic["message"].as_str() == Some(expected.as_str())
            && diagnostic["spans"].as_array().is_some_and(|spans| {
                spans.iter().any(|span| {
                    span["is_primary"] == true
                        && span["file_name"]
                            .as_str()
                            .is_some_and(|name| Path::new(name).ends_with(ADAPTER))
                        && span["text"].as_array().is_some_and(|lines| {
                            lines.iter().any(|source_line| {
                                source_line["text"]
                                    .as_str()
                                    .is_some_and(|text| text.contains(&pattern))
                            })
                        })
                })
            })
    })
}

struct CheckOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
struct SiblingEdit {
    path: PathBuf,
    start: usize,
    end: usize,
    replacement: String,
}

fn sibling_edits(
    workspace: &Path,
    stdout: &str,
    mutation: &ModelMutation,
) -> Result<Vec<SiblingEdit>, String> {
    let mut edits = Vec::new();
    for message in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        let diagnostic = &message["message"];
        if message["reason"] != "compiler-message" || diagnostic["level"] != "error" {
            continue;
        }
        let expected = format!("pattern does not mention field `{}`", mutation.field);
        if diagnostic["code"]["code"] != "E0027" || diagnostic["message"] != expected {
            return Err("not the mutated field's E0027 diagnostic".to_owned());
        }
        let spans = diagnostic["spans"]
            .as_array()
            .ok_or("missing primary spans")?;
        let primaries: Vec<_> = spans
            .iter()
            .filter(|span| span["is_primary"] == true)
            .collect();
        let [primary] = primaries.as_slice() else {
            return Err("ambiguous primary span".to_owned());
        };
        let path = copied_source_path(workspace, primary)?;
        if path.starts_with(workspace.join("crates/auctioneer-exchange"))
            || path == workspace.join(mutation.source)
        {
            return Err("cannot acknowledge the exchange guard or model declaration".to_owned());
        }
        let pattern = format!("{} {{", mutation.model);
        if !primary["text"].as_array().is_some_and(|lines| {
            lines.iter().any(|line| {
                line["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(&pattern))
            })
        }) {
            return Err("diagnostic names a different model".to_owned());
        }
        let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let primary_range = byte_range(primary)?;
        if !source
            .get(primary_range.clone())
            .is_some_and(|text| text.contains(&pattern))
        {
            return Err("primary span no longer matches copied source".to_owned());
        }
        let replacement = format!(", {}: _ }}", mutation.field);
        let children = diagnostic["children"]
            .as_array()
            .ok_or("missing compiler suggestions")?;
        let candidates: Vec<_> = children
            .iter()
            .filter_map(|child| {
                let [span] = child["spans"].as_array()?.as_slice() else {
                    return None;
                };
                (child["level"] == "help" && span["suggested_replacement"] == replacement)
                    .then_some(span)
            })
            .collect();
        let [suggestion] = candidates.as_slice() else {
            return Err("missing or ambiguous explicit named-field suggestion".to_owned());
        };
        let range = byte_range(suggestion)?;
        if suggestion["suggestion_applicability"] != "MachineApplicable"
            || !suggestion["expansion"].is_null()
            || copied_source_path(workspace, suggestion)? != path
            || range.start < primary_range.start
            || range.end > primary_range.end
            || !source.get(range.clone()).is_some_and(|text| {
                text.chars()
                    .filter(|ch| !ch.is_ascii_whitespace())
                    .eq(",}".chars())
            })
        {
            return Err("unsafe explicit named-field suggestion".to_owned());
        }
        edits.push(SiblingEdit {
            path,
            start: range.start,
            end: range.end,
            replacement,
        });
    }
    if edits.is_empty() {
        return Err(
            "no verified sibling guard; unrelated build failures cannot satisfy coverage"
                .to_owned(),
        );
    }
    Ok(edits)
}

fn byte_range(span: &Value) -> Result<Range<usize>, String> {
    let start = span["byte_start"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("invalid byte start")?;
    let end = span["byte_end"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or("invalid byte end")?;
    if start >= end {
        return Err("empty or reversed source span".to_owned());
    }
    Ok(start..end)
}

fn copied_source_path(workspace: &Path, span: &Value) -> Result<PathBuf, String> {
    let path = workspace.join(span["file_name"].as_str().ok_or("missing source path")?);
    if !path.starts_with(workspace.join("crates"))
        || path.extension().is_none_or(|extension| extension != "rs")
        || path.canonicalize().map_err(|error| error.to_string())? != path
    {
        return Err("suggestion must target canonical copied Rust sources".to_owned());
    }
    Ok(path)
}

fn apply_sibling_edits(
    mut edits: Vec<SiblingEdit>,
    originals: &mut BTreeMap<PathBuf, String>,
) -> Result<(), String> {
    edits.sort_by(|left, right| (&left.path, left.start).cmp(&(&right.path, right.start)));
    if edits.windows(2).any(
        |pair| matches!(pair, [left, right] if left.path == right.path && left.end > right.start),
    ) {
        return Err("overlapping sibling suggestions".to_owned());
    }
    for edit in edits.into_iter().rev() {
        let mut source = fs::read_to_string(&edit.path).map_err(|error| error.to_string())?;
        if source.get(edit.start..edit.end).is_none() {
            return Err("suggestion outside current copied source".to_owned());
        }
        originals
            .entry(edit.path.clone())
            .or_insert_with(|| source.clone());
        source.replace_range(edit.start..edit.end, &edit.replacement);
        fs::write(&edit.path, source).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn restore_siblings(originals: BTreeMap<PathBuf, String>) {
    for (path, source) in originals {
        fs::write(path, source).expect("restore every acknowledged sibling guard");
    }
}

impl fmt::Display for CheckOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n{}\n{}", self.status, self.stdout, self.stderr)
    }
}

struct ChildGuard {
    child: Child,
    finished: bool,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.finished {
            // Kill the entire build tree before reclaiming its scratch files.
            #[cfg(unix)]
            let _group = Command::new("/bin/kill")
                .args(["-KILL", &format!("-{}", self.child.id())])
                .status();
            #[cfg(windows)]
            let _group = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &self.child.id().to_string()])
                .status();
            let _kill = self.child.kill();
            let _reap = self.child.wait();
        }
    }
}

fn run_bounded(mut command: Command, root: &Path, label: &str) -> CheckOutput {
    let stdout = root.join(format!("{label}.stdout"));
    let stderr = root.join(format!("{label}.stderr"));
    command
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout).expect("create child stdout log"))
        .stderr(fs::File::create(&stderr).expect("create child stderr log"));
    #[cfg(unix)]
    command.process_group(0);
    let mut child = ChildGuard {
        child: command.spawn().expect("spawn host check command"),
        finished: false,
    };
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.child.try_wait().expect("poll host check command") {
            child.finished = true;
            break status;
        }
        if started.elapsed() >= CHECK_TIMEOUT {
            drop(child);
            panic!(
                "{label} exceeded {} seconds; timeout is not a coverage refusal\n{}\n{}",
                CHECK_TIMEOUT.as_secs(),
                fs::read_to_string(&stdout).expect("read timed-out child stdout"),
                fs::read_to_string(&stderr).expect("read timed-out child stderr")
            );
        }
        thread::sleep(Duration::from_millis(100));
    };
    CheckOutput {
        status,
        stdout: fs::read_to_string(stdout).expect("read child stdout"),
        stderr: fs::read_to_string(stderr).expect("read child stderr"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::ffi::OsStr;

    fn user_mutation() -> ModelMutation {
        ModelMutation {
            model: "User",
            source: "crates/auctioneer-openrtb/src/types/user.rs",
            field: "scrub_coverage_user",
            sparse: false,
        }
    }

    fn sibling_diagnostic(workspace: &Path) -> Value {
        let path = workspace.join("crates/sibling/src/lib.rs");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source = "let User { other, } = value;";
        fs::write(&path, source).unwrap();
        let mut message = diagnostic();
        message["target"]["name"] = json!("sibling");
        message["message"]["spans"][0]["file_name"] = json!(path);
        let end = source.find(" = value;").unwrap();
        message["message"]["spans"][0]["byte_start"] = json!(4_usize);
        message["message"]["spans"][0]["byte_end"] = json!(end);
        message["message"]["children"] = json!([{
            "level": "help",
            "spans": [{
                "file_name": path,
                "byte_start": source.find(',').unwrap(),
                "byte_end": end,
                "suggestion_applicability": "MachineApplicable",
                "suggested_replacement": ", scrub_coverage_user: _ }",
                "expansion": null
            }]
        }]);
        message
    }

    #[test]
    fn sibling_guards_accept_only_explicit_compiler_edits_for_the_new_field() {
        let scratch = ScratchDir::new();
        let message = sibling_diagnostic(&scratch.0);
        let edits = sibling_edits(&scratch.0, &message.to_string(), &user_mutation()).unwrap();
        assert_eq!(edits.len(), 1);
        let edit = &edits[0];
        assert_eq!(edit.path, scratch.0.join("crates/sibling/src/lib.rs"));
        assert_eq!(edit.replacement, ", scrub_coverage_user: _ }");
        let mut source = fs::read_to_string(&edit.path).unwrap();
        source.replace_range(edit.start..edit.end, &edit.replacement);
        assert_eq!(
            source,
            "let User { other, scrub_coverage_user: _ } = value;"
        );

        for (pointer, replacement) in [
            ("/message/code/code", json!("E0063")),
            (
                "/message/message",
                json!("pattern does not mention field `other`"),
            ),
            ("/message/spans/0/byte_start", json!(99_usize)),
            ("/message/spans/0/text/0/text", json!("let Device {")),
            (
                "/message/children/0/spans/0/suggestion_applicability",
                json!("MaybeIncorrect"),
            ),
            (
                "/message/children/0/spans/0/suggested_replacement",
                json!(", .. }"),
            ),
            (
                "/message/children/0/spans/0/suggested_replacement",
                json!(", other: _ }"),
            ),
            ("/message/children/0/spans/0/byte_start", json!(0_usize)),
            (
                "/message/children/0/spans/0/expansion",
                json!({"macro_decl_name":"foreign!"}),
            ),
            (
                "/message/children/0/spans/0/file_name",
                json!("/outside/lib.rs"),
            ),
        ] {
            let mut wrong = message.clone();
            *wrong.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                sibling_edits(&scratch.0, &wrong.to_string(), &user_mutation()).is_err(),
                "accepted {pointer}"
            );
        }
    }

    #[test]
    fn sibling_guards_cannot_patch_the_exchange_or_escape_copied_sources() {
        let scratch = ScratchDir::new();
        let mut message = sibling_diagnostic(&scratch.0);
        for (path, expected) in [
            (
                scratch.0.join(ADAPTER),
                "cannot acknowledge the exchange guard or model declaration",
            ),
            (
                scratch
                    .0
                    .join("crates/auctioneer-exchange/src/another_guard.rs"),
                "cannot acknowledge the exchange guard or model declaration",
            ),
            (
                scratch.0.join("outside.rs"),
                "suggestion must target canonical copied Rust sources",
            ),
            (
                scratch.0.join(user_mutation().source),
                "cannot acknowledge the exchange guard or model declaration",
            ),
        ] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "let User { other, } = value;").unwrap();
            message["message"]["spans"][0]["file_name"] = json!(path);
            message["message"]["children"][0]["spans"][0]["file_name"] = json!(path);
            assert_eq!(
                sibling_edits(&scratch.0, &message.to_string(), &user_mutation()).unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn sibling_edits_restore_sources_and_reject_overlapping_or_unrelated_errors() {
        let scratch = ScratchDir::new();
        let message = sibling_diagnostic(&scratch.0);
        let path = scratch.0.join("crates/sibling/src/lib.rs");
        let original = fs::read_to_string(&path).unwrap();
        let edits = sibling_edits(&scratch.0, &message.to_string(), &user_mutation()).unwrap();
        let mut originals = BTreeMap::new();
        apply_sibling_edits(edits, &mut originals).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "let User { other, scrub_coverage_user: _ } = value;"
        );
        assert_eq!(originals.get(&path), Some(&original));
        restore_siblings(originals);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);

        let repeated = format!("{message}\n{message}");
        let overlapping = sibling_edits(&scratch.0, &repeated, &user_mutation()).unwrap();
        let mut unchanged = BTreeMap::new();
        assert_eq!(
            apply_sibling_edits(overlapping, &mut unchanged).unwrap_err(),
            "overlapping sibling suggestions"
        );
        assert!(unchanged.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);

        let mut unrelated = message.clone();
        unrelated["message"]["code"]["code"] = json!("E0063");
        for (output, expected) in [
            (
                unrelated.to_string(),
                "not the mutated field's E0027 diagnostic",
            ),
            (
                format!("{message}\n{unrelated}"),
                "not the mutated field's E0027 diagnostic",
            ),
            (
                "error: dependency unavailable offline".to_owned(),
                "no verified sibling guard; unrelated build failures cannot satisfy coverage",
            ),
        ] {
            assert_eq!(
                sibling_edits(&scratch.0, &output, &user_mutation()).unwrap_err(),
                expected
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn sibling_suggestions_cannot_follow_a_symlink_to_live_sources() {
        use std::os::unix::fs::symlink;

        let scratch = ScratchDir::new();
        let live = ScratchDir::new();
        let mut message = sibling_diagnostic(&live.0);
        fs::create_dir_all(scratch.0.join("crates")).unwrap();
        symlink(
            live.0.join("crates/sibling"),
            scratch.0.join("crates/sibling"),
        )
        .unwrap();
        let path = scratch.0.join("crates/sibling/src/lib.rs");
        message["message"]["spans"][0]["file_name"] = json!(path);
        message["message"]["children"][0]["spans"][0]["file_name"] = json!(path);
        assert_eq!(
            sibling_edits(&scratch.0, &message.to_string(), &user_mutation()).unwrap_err(),
            "suggestion must target canonical copied Rust sources"
        );
        assert_eq!(
            fs::read_to_string(live.0.join("crates/sibling/src/lib.rs")).unwrap(),
            "let User { other, } = value;"
        );
    }

    #[test]
    fn scratch_directories_have_canonical_distinct_source_roots() {
        let first = ScratchDir::new();
        let second = ScratchDir::new();
        assert_eq!(first.0, first.0.canonicalize().unwrap());
        assert_eq!(second.0, second.0.canonicalize().unwrap());
        assert_ne!(first.0, second.0);
    }

    fn diagnostic() -> Value {
        json!({
            "reason": "compiler-message",
            "target": {"name": "auctioneer_exchange"},
            "message": {
                "level": "error",
                "code": {"code": "E0027"},
                "message": "pattern does not mention field `scrub_coverage_user`",
                "spans": [{
                    "is_primary": true,
                    "file_name": ADAPTER,
                    "text": [{"text": "    let User {"}]
                }]
            }
        })
    }

    #[test]
    fn missing_coverage_requires_the_exact_field_code_model_and_adapter_span() {
        let expected = diagnostic();
        assert!(has_missing_field_diagnostic(
            &expected.to_string(),
            "User",
            "scrub_coverage_user"
        ));
        for (pointer, replacement) in [
            ("/reason", json!("build-script-executed")),
            ("/target/name", json!("auctioneer_openrtb")),
            ("/message/level", json!("warning")),
            ("/message/code/code", json!("E0063")),
            (
                "/message/message",
                json!("pattern does not mention field `other`"),
            ),
            ("/message/spans/0/is_primary", json!(false)),
            (
                "/message/spans/0/file_name",
                json!("crates/auctioneer-openrtb/src/types/user.rs"),
            ),
            ("/message/spans/0/text/0/text", json!("    let Device {")),
        ] {
            let mut wrong = expected.clone();
            *wrong.pointer_mut(pointer).unwrap() = replacement;
            assert!(
                !has_missing_field_diagnostic(&wrong.to_string(), "User", "scrub_coverage_user"),
                "accepted unrelated diagnostic after changing {pointer}"
            );
        }
    }

    #[test]
    fn cargo_or_build_script_failure_is_not_missing_adapter_coverage() {
        let build_finished = json!({"reason": "build-finished", "success": false}).to_string();
        for output in [
            "error: failed to download a dependency in offline mode",
            "error[E0027]: pattern does not mention field `scrub_coverage_user`",
            build_finished.as_str(),
            "error: GCL freshness check failed",
        ] {
            assert!(!has_missing_field_diagnostic(
                output,
                "User",
                "scrub_coverage_user"
            ));
        }
    }

    #[test]
    fn missing_coverage_accepts_absolute_scratch_spans_and_sparse_backing_patterns() {
        let mut message = diagnostic();
        message["message"]["message"] =
            json!("pattern does not mention field `scrub_coverage_backing`");
        message["message"]["spans"][0]["file_name"] = json!(format!("/tmp/scratch/{ADAPTER}"));
        message["message"]["spans"][0]["text"][0]["text"] =
            json!("    let Some(BrandVersionFields {");
        let output = format!("cargo progress\n{message}\n");
        assert!(has_missing_field_diagnostic(
            &output,
            "BrandVersionFields",
            "scrub_coverage_backing"
        ));
    }

    #[test]
    fn child_checks_are_offline_locked_serial_and_explicitly_host_targeted() {
        let scratch = ScratchDir::new();
        let workspace = scratch.0.join("workspace");
        let target = scratch.0.join("target");
        let manifest = workspace.join("Cargo.toml");
        let command = check_command(&workspace, &target, "existing-toolchain", "native-host");
        let args: Vec<_> = command.get_args().collect();
        for arg in [
            "check",
            "--locked",
            "--offline",
            "--lib",
            "--message-format=json",
        ] {
            assert!(args.contains(&OsStr::new(arg)));
        }
        for (flag, value) in [
            ("--jobs", OsStr::new("1")),
            ("--target", OsStr::new("native-host")),
            ("--target-dir", target.as_os_str()),
            ("--manifest-path", manifest.as_os_str()),
        ] {
            assert!(
                args.windows(2)
                    .any(|pair| pair == [OsStr::new(flag), value])
            );
        }
        let settings: Vec<_> = command.get_envs().collect();
        for (key, value) in [
            ("CARGO_NET_OFFLINE", Some("true")),
            ("CARGO_INCREMENTAL", Some("0")),
            ("RUSTUP_AUTO_INSTALL", Some("0")),
            ("RUSTUP_TOOLCHAIN", Some("existing-toolchain")),
            ("RUSTC_WRAPPER", Some("")),
            ("RUSTC_WORKSPACE_WRAPPER", Some("")),
            ("CARGO_BUILD_TARGET", None),
            ("CARGO_TARGET_DIR", None),
            ("RUSTFLAGS", None),
            ("CARGO_ENCODED_RUSTFLAGS", None),
        ] {
            assert!(settings.contains(&(OsStr::new(key), value.map(OsStr::new))));
        }
        assert_eq!(command.get_current_dir(), Some(workspace.as_path()));
    }

    #[test]
    fn scratch_source_copies_are_independent_and_cleaned_on_drop() {
        let original = ScratchDir::new();
        let copied = ScratchDir::new();
        let source = original.0.join("source");
        let destination = copied.0.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("model.rs"), "original model").unwrap();
        fs::create_dir_all(source.join("target")).unwrap();
        fs::write(source.join("target/artifact"), "not source").unwrap();
        copy_tree(&source, &destination);
        fs::write(destination.join("model.rs"), "mutated copy").unwrap();
        assert_eq!(
            fs::read_to_string(source.join("model.rs")).unwrap(),
            "original model"
        );
        assert!(!destination.join("target").exists());
        let path = copied.0.clone();
        drop(copied);
        assert!(!path.exists());
        assert!(source.join("model.rs").is_file());
    }
}

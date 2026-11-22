//! Offline Git data for the reviewed catalog-growth reference.
//!
//! Only source provenance is described here. Historical `GeneratorSource` bytes
//! are not the generator executed by a measurement. Consumers must separately
//! freeze current measurement sources, dependencies, lock, options and ABI.
//! This module never builds, expands, executes or activates source code.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

// Same existing per-input staging cap, kept local to this test-only reader.
// This is not a new application limit or numeric resource policy.
const MAX_INPUT_BYTES: u64 = 1 << 20;

/// Maintainer-approved official source commit, never a moving reference.
pub const REFERENCE_COMMIT: &str = "d26fcf7f566db6d1dce2e1847d79c9f44581edb8";
/// Root tree independently pinned with the reference commit.
pub const REFERENCE_TREE: &str = "2d54c6f53ef3d062ca9292449a4546bb76e5ddd8";

// Census of the trusted tree's unchanged-scanner materialization closure,
// not application limits, allocation bounds or deployment qualifications.
const REFERENCE_FILES: usize = 1_134;
const REFERENCE_BYTES: u64 = 7_571_136;
const FIXED_INPUTS: [&str; 15] = [
    "Cargo.toml",
    "Cargo.lock",
    "crates/auctioneer-bidder/Cargo.toml",
    "crates/auctioneer-bidder/build.rs",
    "crates/auctioneer-bidder/src/schema/staging.rs",
    "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml",
    "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml",
    "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/supplemental-schemas.toml",
    "crates/auctioneer-config/Cargo.toml",
    "crates/auctioneer-config/src/schema_staging.rs",
    "crates/auctioneer-config/src/schema_staging_codegen.rs",
    "crates/auctioneer-config/src/schema_staging_build.rs",
    "crates/auctioneer-config/src/bidder_discovery.rs",
    "crates/auctioneer-config/src/build_catalog.rs",
    "crates/auctioneer-config/src/bidder_manifest.rs",
];
const TREE_ROOTS: [&str; 3] = [
    "crates/auctioneer-bidder/src/bidders",
    "vendor/jsonschema-macros-core",
    "vendor/regex-automata",
];

/// A refusal never represents a matched or empty reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Missing, nonregular, indirect, or non-repository root.
    Root,
    /// HEAD, commit, or root-tree identity differs from the required pin.
    Identity,
    /// Partial/promisor or externally supplied alternate object storage.
    Partial,
    /// A required commit, tree or blob object is unavailable.
    MissingObject,
    /// Non-normal, non-UTF-8 or otherwise unsafe repository-relative path.
    Path,
    /// A listed object is not a directory tree or regular file blob.
    Mode,
    /// Existing staging input cap or inherited Git operation budget exceeded.
    Limit,
    /// Object hash, byte length, closure membership or census mismatch.
    Witness,
    /// Unsafe/existing output location or a materialization write failed.
    Destination,
    /// A local Git command or capture failed.
    Git,
}

/// Identity of one consumed regular source file.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct Witness {
    pub path: String,
    pub mode: String,
    pub blob: String,
    pub bytes: u64,
    pub sha256: String,
}

/// Source bytes and their Git-bound witness; bytes are not serialized.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SourceFile {
    pub witness: Witness,
    #[serde(skip)]
    pub contents: Vec<u8>,
}

/// Verified source snapshot, not measurement identity or numeric qualification.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SourceSnapshot {
    commit: String,
    tree: String,
    files: BTreeMap<String, SourceFile>,
}

/// Read only the trusted reference's required Git-object data.
///
/// The caller supplies the repository root explicitly. No test-control
/// environment variable, checkout file, network acquisition or fallback is
/// consulted. Consumers own runtime input requirements and materialization.
///
/// # Errors
/// Refuses unmatched repositories, partial or missing objects, unsafe paths,
/// unsupported modes, over-limit inputs and inconsistent witnesses.
pub fn read(root: &Path) -> Result<SourceSnapshot, Error> {
    read_pinned(
        root,
        REFERENCE_COMMIT,
        REFERENCE_TREE,
        REFERENCE_FILES,
        REFERENCE_BYTES,
    )
}

impl SourceSnapshot {
    /// Source commit, not the current measurement implementation revision.
    #[must_use]
    pub fn commit(&self) -> &str {
        &self.commit
    }

    /// Root tree of the verified source snapshot.
    #[must_use]
    pub fn tree(&self) -> &str {
        &self.tree
    }

    /// Every consumed regular-file witness, in repository-relative path order.
    #[must_use]
    pub fn witnesses(&self) -> impl ExactSizeIterator<Item = &Witness> {
        self.files.values().map(|file| &file.witness)
    }

    /// Borrow the verified object bytes without exposing mutable snapshot state.
    #[must_use]
    pub fn files(&self) -> impl ExactSizeIterator<Item = &SourceFile> {
        self.files.values()
    }

    /// Materialize only captured Git-object bytes into a new private directory.
    ///
    /// No Git repository or checkout is reopened. Existing destinations are
    /// never merged, overwritten or cleaned. Files are data, not executables;
    /// Unix directories/files use owner-only permissions. The caller owns the
    /// isolated destination and must exclude concurrent mutation. A write
    /// failure can leave a partial destination, which must not be consumed.
    ///
    /// # Errors
    /// Refuses changed captured data, unsafe paths, existing destinations,
    /// symlinked ancestors, nonregular outputs and filesystem write failures.
    pub fn materialize(&self, destination: &Path) -> Result<(), Error> {
        materialize(self, destination)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Entry {
    path: String,
    mode: String,
    blob: String,
    bytes: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct Listing {
    files: BTreeMap<String, Entry>,
    trees: BTreeSet<String>,
}

use sha2::{Digest as _, Sha256};
use std::fs;
use std::io::{self, ErrorKind, Read as _, Write as _};
use std::path::{Component, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::str::from_utf8;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

// Same tooling deadlines as CLI's existing offline source reader. These are
// subprocess fences, not serving/configuration limits or resource allowances.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(120);

fn read_pinned(
    root: &Path,
    commit: &str,
    tree: &str,
    files: usize,
    bytes: u64,
) -> Result<SourceSnapshot, Error> {
    regular_directory(root)?;
    if !valid_oid(commit) || !valid_oid(tree) {
        return Err(Error::Identity);
    }
    if files == 0 || bytes == 0 {
        return Err(Error::Witness);
    }
    let deadline = Instant::now()
        .checked_add(OPERATION_TIMEOUT)
        .ok_or(Error::Limit)?;
    let prefix = run(root, &["rev-parse", "--show-prefix"], None, deadline)?;
    let top = run(root, &["rev-parse", "--show-toplevel"], None, deadline)?;
    if !prefix.status.success()
        || prefix.stdout != b"\n"
        || !top.status.success()
        || text_line(&top.stdout).map_err(|_error| Error::Root)?
            != root.to_str().ok_or(Error::Root)?
    {
        return Err(Error::Root);
    }
    reject_partial(root, deadline)?;
    check_head(root, commit, deadline)?;
    let commit_data = object(root, "commit", commit, deadline)?;
    if !commit_data.starts_with(format!("tree {tree}\n").as_bytes()) {
        return Err(Error::Identity);
    }
    drop(object(root, "tree", tree, deadline)?);
    let mut arguments = vec!["ls-tree", "-r", "-t", "-l", "-z", "--full-tree", tree, "--"];
    arguments.extend(FIXED_INPUTS);
    arguments.extend(TREE_ROOTS);
    let output = run(root, &arguments, None, deadline)?;
    if !output.status.success() {
        return Err(Error::MissingObject);
    }
    let listing = parse_listing(&output.stdout)?;
    if listing.files.len() != files
        || FIXED_INPUTS
            .iter()
            .any(|path| !listing.files.contains_key(*path))
        || listing.files.values().try_fold(0_u64, |sum, entry| {
            sum.checked_add(entry.bytes).ok_or(Error::Witness)
        })? != bytes
    {
        return Err(Error::Witness);
    }
    // -t retains traversed ancestors as well as descendant tree objects.
    // Rehash them, too: merely trusting a corrupt loose object's filename
    // would not establish that the listed child blobs belong to the pin.
    for oid in listing.trees {
        drop(object(root, "tree", &oid, deadline)?);
    }
    let mut captured = BTreeMap::new();
    for (path, entry) in listing.files {
        let contents = object(root, "blob", &entry.blob, deadline)?;
        if u64::try_from(contents.len()).map_err(|_error| Error::Limit)? != entry.bytes {
            return Err(Error::Witness);
        }
        let witness = Witness {
            path: entry.path,
            mode: entry.mode,
            blob: entry.blob,
            bytes: entry.bytes,
            sha256: hex::encode(Sha256::digest(&contents)),
        };
        captured.insert(path, SourceFile { witness, contents });
    }
    reject_partial(root, deadline)?;
    check_head(root, commit, deadline)?;
    Ok(SourceSnapshot {
        commit: commit.to_owned(),
        tree: tree.to_owned(),
        files: captured,
    })
}

fn valid_oid(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn normal_relative(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn normal_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .to_str()
            .is_some_and(|text| !text.chars().any(char::is_control))
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
        && path.components().collect::<PathBuf>().as_os_str() == path.as_os_str()
}

fn regular_directory(path: &Path) -> Result<(), Error> {
    if !normal_absolute(path) {
        return Err(Error::Root);
    }
    let mut prefix = PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        let metadata = fs::symlink_metadata(&prefix).map_err(|_error| Error::Root)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::Root);
        }
    }
    Ok(())
}

fn text_line(bytes: &[u8]) -> Result<&str, Error> {
    let text = from_utf8(bytes).map_err(|_error| Error::Witness)?;
    let line = text.strip_suffix('\n').ok_or(Error::Witness)?;
    if line.is_empty() || line.chars().any(char::is_control) {
        return Err(Error::Witness);
    }
    Ok(line)
}

fn oid_line(bytes: &[u8]) -> Result<&str, Error> {
    let oid = text_line(bytes)?;
    if !valid_oid(oid) {
        return Err(Error::Witness);
    }
    Ok(oid)
}

fn check_head(root: &Path, expected: &str, deadline: Instant) -> Result<(), Error> {
    let output = run(
        root,
        &["rev-parse", "--verify", "HEAD^{commit}"],
        None,
        deadline,
    )?;
    if !output.status.success()
        || oid_line(&output.stdout).map_err(|_error| Error::Identity)? != expected
    {
        return Err(Error::Identity);
    }
    Ok(())
}

fn reject_partial(root: &Path, deadline: Instant) -> Result<(), Error> {
    let config = run(
        root,
        &[
            "config",
            "--get-regexp",
            "^(extensions\\.partialclone|remote\\..*\\.(promisor|partialclonefilter))$",
        ],
        None,
        deadline,
    )?;
    match config.status.code() {
        Some(0_i32) => return Err(Error::Partial),
        Some(1_i32) if config.stdout.is_empty() => {}
        _ => return Err(Error::Git),
    }
    let output = run(
        root,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ],
        None,
        deadline,
    )?;
    if !output.status.success() {
        return Err(Error::Root);
    }
    let objects = PathBuf::from(text_line(&output.stdout).map_err(|_error| Error::Root)?);
    regular_directory(&objects)?;
    for name in ["info", "pack"] {
        let directory = objects.join(name);
        match fs::symlink_metadata(&directory) {
            Ok(_) => regular_directory(&directory)?,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => return Err(Error::Root),
        }
        if name == "info" {
            match fs::symlink_metadata(directory.join("alternates")) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                    return Err(Error::Root);
                }
                // A self-contained reference must not obtain missing data
                // from another repository's uninspected object store.
                Ok(metadata) if metadata.len() != 0 => return Err(Error::Partial),
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(_) => return Err(Error::Root),
            }
        } else {
            for entry in fs::read_dir(directory).map_err(|_error| Error::Root)? {
                if entry
                    .map_err(|_error| Error::Root)?
                    .path()
                    .extension()
                    .is_some_and(|suffix| suffix == "promisor")
                {
                    return Err(Error::Partial);
                }
            }
        }
    }
    Ok(())
}

fn included(path: &str) -> bool {
    if FIXED_INPUTS.contains(&path) || path.starts_with("crates/auctioneer-bidder/src/bidders/") {
        return true;
    }
    ["vendor/jsonschema-macros-core/", "vendor/regex-automata/"]
        .iter()
        .any(|prefix| {
            path.strip_prefix(prefix).is_some_and(|relative| {
                Path::new(relative)
                    .extension()
                    .is_some_and(|suffix| suffix == "rs")
                    || Path::new(relative)
                        .file_name()
                        .is_some_and(|name| name == "Cargo.toml")
            })
        })
}

fn parse_listing(bytes: &[u8]) -> Result<Listing, Error> {
    if bytes.is_empty() || !bytes.ends_with(&[0]) {
        return Err(Error::Witness);
    }
    let mut listing = Listing {
        files: BTreeMap::new(),
        trees: BTreeSet::new(),
    };
    let mut paths = BTreeSet::new();
    let body = bytes.strip_suffix(&[0]).ok_or(Error::Witness)?;
    for encoded in body.split(|byte| *byte == 0) {
        if encoded.is_empty() {
            return Err(Error::Witness);
        }
        let record = from_utf8(encoded).map_err(|_error| Error::Path)?;
        let (header, path) = record.split_once('\t').ok_or(Error::Witness)?;
        if !normal_relative(path) {
            return Err(Error::Path);
        }
        if !paths.insert(path.to_owned()) {
            return Err(Error::Witness);
        }
        let mut fields = header.split_ascii_whitespace();
        let mode = fields.next().ok_or(Error::Witness)?;
        let kind = fields.next().ok_or(Error::Witness)?;
        let oid = fields.next().ok_or(Error::Witness)?;
        let size = fields.next().ok_or(Error::Witness)?;
        if fields.next().is_some() || !valid_oid(oid) {
            return Err(Error::Witness);
        }
        if mode == "040000" && kind == "tree" && size == "-" {
            listing.trees.insert(oid.to_owned());
            continue;
        }
        if !matches!(mode, "100644" | "100755") || kind != "blob" {
            return Err(Error::Mode);
        }
        if !included(path) {
            continue;
        }
        let length = size.parse::<u64>().map_err(|_error| Error::MissingObject)?;
        if length > MAX_INPUT_BYTES {
            return Err(Error::Limit);
        }
        listing.files.insert(
            path.to_owned(),
            Entry {
                path: path.to_owned(),
                mode: mode.to_owned(),
                blob: oid.to_owned(),
                bytes: length,
            },
        );
    }
    if listing.files.is_empty() {
        return Err(Error::Witness);
    }
    Ok(listing)
}

fn object(root: &Path, kind: &str, oid: &str, deadline: Instant) -> Result<Vec<u8>, Error> {
    let output = run(root, &["cat-file", kind, oid], None, deadline)?;
    if !output.status.success() {
        return Err(Error::MissingObject);
    }
    // Without -w, hash-object only hashes supplied bytes. No working-tree
    // filters, path-based conversion, object writes or historical code run.
    let hash = run(
        root,
        &["hash-object", "-t", kind, "--stdin", "--no-filters"],
        Some(&output.stdout),
        deadline,
    )?;
    if !hash.status.success() {
        return Err(Error::Git);
    }
    if oid_line(&hash.stdout)? != oid {
        return Err(Error::Witness);
    }
    Ok(output.stdout)
}

fn git_command(root: &Path) -> Command {
    // Native CI has the system Git here. A fixed executable avoids consulting
    // PATH or any other caller environment as an executable/configuration input.
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .current_dir(root)
        .args([
            "--no-replace-objects",
            "--no-optional-locks",
            "--literal-pathspecs",
            "--no-pager",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "protocol.allow=never",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
        ])
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
}

struct GitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
}

fn bounded(mut pipe: impl io::Read) -> Result<Vec<u8>, Error> {
    let cap = MAX_INPUT_BYTES;
    let mut bytes = Vec::new();
    pipe.by_ref()
        .take(cap.checked_add(1).ok_or(Error::Limit)?)
        .read_to_end(&mut bytes)
        .map_err(|_error| Error::Git)?;
    if u64::try_from(bytes.len()).map_err(|_error| Error::Limit)? > cap {
        return Err(Error::Limit);
    }
    Ok(bytes)
}

fn run(
    root: &Path,
    arguments: &[&str],
    input: Option<&[u8]>,
    operation_deadline: Instant,
) -> Result<GitOutput, Error> {
    let now = Instant::now();
    if now >= operation_deadline {
        return Err(Error::Limit);
    }
    let deadline = now
        .checked_add(COMMAND_TIMEOUT)
        .ok_or(Error::Limit)?
        .min(operation_deadline);
    let mut command = git_command(root);
    command.args(arguments);
    if input.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command.spawn().map_err(|_error| Error::Git)?;
    let Some(stdout) = child.stdout.take() else {
        let _killed = child.kill();
        let _reaped = child.wait();
        return Err(Error::Git);
    };
    let stdin = child.stdin.take();
    thread::scope(|scope| {
        let (send, receive) = mpsc::channel();
        let reader = scope.spawn(move || {
            let _sent = send.send(bounded(stdout));
        });
        let writer = scope.spawn(move || match (input, stdin) {
            (Some(bytes), Some(mut pipe)) => pipe.write_all(bytes).map_err(|_error| Error::Git),
            (None, None) => Ok(()),
            _ => Err(Error::Git),
        });
        let mut captured = None;
        let result = loop {
            if captured.is_none() {
                match receive.try_recv() {
                    Ok(Ok(bytes)) => captured = Some(bytes),
                    Ok(Err(error)) => break Err(error),
                    Err(TryRecvError::Disconnected) => break Err(Error::Git),
                    Err(TryRecvError::Empty) => {}
                }
            }
            match child.try_wait() {
                Ok(Some(status)) if captured.is_some() => {
                    break Ok(GitOutput {
                        status,
                        stdout: captured.take().ok_or(Error::Git)?,
                    });
                }
                Ok(_) => {}
                Err(_) => break Err(Error::Git),
            }
            if Instant::now() >= deadline {
                break Err(Error::Limit);
            }
            thread::sleep(Duration::from_millis(2));
        };
        if result.is_err() {
            let _killed = child.kill();
            let _reaped = child.wait();
        }
        let written = writer.join().map_err(|_error| Error::Git)?;
        reader.join().map_err(|_error| Error::Git)?;
        match result {
            Ok(output) => {
                if output.status.success() {
                    written?;
                }
                Ok(output)
            }
            Err(error) => Err(error),
        }
    })
}

fn materialize(snapshot: &SourceSnapshot, destination: &Path) -> Result<(), Error> {
    // Preflight the whole captured set before creating any output.
    if snapshot.files.is_empty() {
        return Err(Error::Witness);
    }
    for (path, file) in &snapshot.files {
        let witness = &file.witness;
        if !normal_relative(path) || !normal_relative(&witness.path) {
            return Err(Error::Path);
        }
        if !matches!(witness.mode.as_str(), "100644" | "100755") {
            return Err(Error::Mode);
        }
        if witness.path != *path
            || !valid_oid(&witness.blob)
            || witness.bytes > MAX_INPUT_BYTES
            || u64::try_from(file.contents.len()).map_err(|_error| Error::Limit)? != witness.bytes
            || hex::encode(Sha256::digest(&file.contents)) != witness.sha256
        {
            return Err(Error::Witness);
        }
    }
    if !normal_absolute(destination) {
        return Err(Error::Destination);
    }
    let parent = destination.parent().ok_or(Error::Destination)?;
    regular_directory(parent).map_err(|_error| Error::Destination)?;
    private_directory(destination).map_err(|_error| Error::Destination)?;
    for (path, file) in &snapshot.files {
        let relative = Path::new(path);
        let mut directory = destination.to_path_buf();
        if let Some(relative_parent) = relative.parent() {
            for part in relative_parent.components() {
                directory.push(part);
                match private_directory(&directory) {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                    Err(_) => return Err(Error::Destination),
                }
                regular_directory(&directory).map_err(|_error| Error::Destination)?;
            }
        }
        let target = destination.join(relative);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600)
        };
        let mut output = options.open(&target).map_err(|_error| Error::Destination)?;
        output
            .write_all(&file.contents)
            .map_err(|_error| Error::Destination)?;
        let metadata = fs::symlink_metadata(&target).map_err(|_error| Error::Destination)?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != file.witness.bytes
        {
            return Err(Error::Destination);
        }
    }
    Ok(())
}

fn private_directory(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700)
    };
    builder.create(path)
}

#[cfg(test)]
mod tests {
    use super::{Error, SourceSnapshot, included, parse_listing, read_pinned};
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Write as _;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use tempfile::TempDir;

    // This literal fixture is independent of the reader's selectors and census.
    const FILES: [&str; 20] = [
        "Cargo.toml",
        "Cargo.lock",
        "crates/auctioneer-bidder/Cargo.toml",
        "crates/auctioneer-bidder/build.rs",
        "crates/auctioneer-bidder/src/schema/staging.rs",
        "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-inventory.toml",
        "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml",
        "crates/auctioneer-bidder/tests/conformance/fixtures/reference/catalog/supplemental-schemas.toml",
        "crates/auctioneer-config/Cargo.toml",
        "crates/auctioneer-config/src/schema_staging.rs",
        "crates/auctioneer-config/src/schema_staging_codegen.rs",
        "crates/auctioneer-config/src/schema_staging_build.rs",
        "crates/auctioneer-config/src/bidder_discovery.rs",
        "crates/auctioneer-config/src/build_catalog.rs",
        "crates/auctioneer-config/src/bidder_manifest.rs",
        "crates/auctioneer-bidder/src/bidders/fake_fixture/bidder.toml",
        "crates/auctioneer-bidder/src/bidders/fake_fixture/mod.rs",
        "crates/auctioneer-bidder/src/bidders/fake_fixture/parameters.json",
        "vendor/jsonschema-macros-core/Cargo.toml",
        "vendor/regex-automata/src/lib.rs",
    ];

    // These commands mutate only each test's disposable repository when CI runs
    // this owner. The reader never invokes these fixture setup operations.
    fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> Vec<u8> {
        let mut command = Command::new("/usr/bin/git");
        command
            .env_clear()
            .current_dir(root)
            .args([
                "--no-replace-objects",
                "--no-optional-locks",
                "--literal-pathspecs",
                "--no-pager",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "protocol.allow=never",
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
            ])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_ALLOW_PROTOCOL", "")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.test")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.test")
            .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
            .env("LC_ALL", "C")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("fixture Git is installed in CI");
        if let Some(bytes) = input {
            child
                .stdin
                .take()
                .expect("fixture stdin")
                .write_all(bytes)
                .expect("fixture input");
        }
        let output = child.wait_with_output().expect("fixture command completes");
        assert!(
            output.status.success(),
            "fixture Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stdout.len() <= 4096,
            "fixture output is bounded metadata"
        );
        output.stdout
    }

    fn line(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes)
            .expect("fixture metadata UTF-8")
            .strip_suffix('\n')
            .expect("fixture metadata newline")
            .to_owned()
    }

    struct Fixture {
        _directory: TempDir,
        root: PathBuf,
        files: BTreeMap<String, Vec<u8>>,
        commit: String,
        tree: String,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().expect("fixture temporary directory");
            let root = directory.path().canonicalize().expect("real fixture root");
            git(&root, &["init", "--quiet", "--object-format=sha1"], None);
            let mut fixture = Self {
                _directory: directory,
                root,
                files: BTreeMap::new(),
                commit: String::new(),
                tree: String::new(),
            };
            for path in FILES {
                let bytes = if path == "Cargo.lock" {
                    b"abc".to_vec()
                } else if path.ends_with("parameters.json") {
                    serde_json::to_vec(&serde_json::json!({"type": "string"}))
                        .expect("fixture schema bytes")
                } else {
                    // Deliberately not Rust implementations: the reader treats
                    // source presence and bytes as data, never executable input.
                    format!("source fixture data for {path}\n").into_bytes()
                };
                let full = fixture.root.join(path);
                fs::create_dir_all(full.parent().expect("fixture parent")).expect("directories");
                fs::write(full, &bytes).expect("fixture source");
                fixture.files.insert(path.to_owned(), bytes);
            }
            git(&fixture.root, &["add", "--all"], None);
            fixture.commit_index();
            fixture
        }

        fn commit_index(&mut self) {
            git(
                &self.root,
                &["commit", "--quiet", "--no-gpg-sign", "-m", "fixture"],
                None,
            );
            self.commit = line(git(&self.root, &["rev-parse", "HEAD"], None));
            self.tree = line(git(&self.root, &["rev-parse", "HEAD^{tree}"], None));
        }

        fn bytes(&self) -> u64 {
            self.files
                .values()
                .map(|bytes| u64::try_from(bytes.len()).expect("fixture length"))
                .sum()
        }

        fn read(&self) -> Result<SourceSnapshot, Error> {
            read_pinned(&self.root, &self.commit, &self.tree, 20, self.bytes())
        }

        fn blob(&self, path: &str) -> String {
            line(git(
                &self.root,
                &["rev-parse", &format!("HEAD:{path}")],
                None,
            ))
        }

        fn object(&self, oid: &str) -> PathBuf {
            let (directory, filename) = oid.split_at(2);
            self.root
                .join(".git/objects")
                .join(directory)
                .join(filename)
        }

        fn corrupt_blob(&self, replacement: &[u8]) {
            let original = self.blob("Cargo.lock");
            let replacement_oid = line(git(
                &self.root,
                &["hash-object", "-w", "--stdin"],
                Some(replacement),
            ));
            assert_ne!(original, replacement_oid);
            // Git loose objects are read-only on Linux. Replace only this
            // disposable fixture's file rather than overwriting its mode.
            fs::remove_file(self.object(&original)).expect("remove disposable original object");
            fs::copy(self.object(&replacement_oid), self.object(&original))
                .expect("corrupt only disposable loose object");
        }
    }

    #[test]
    fn complete_pinned_data_has_exact_files_and_per_blob_witnesses() {
        let fixture = Fixture::new();
        let snapshot = fixture
            .read()
            .expect("complete pinned source must be accepted");
        assert_eq!(snapshot.commit, fixture.commit);
        assert_eq!(snapshot.tree, fixture.tree);
        assert_eq!(snapshot.files.len(), 20);
        assert_eq!(
            snapshot.files.keys().collect::<Vec<_>>(),
            fixture.files.keys().collect::<Vec<_>>()
        );
        for (path, expected) in &fixture.files {
            let file = &snapshot.files[path];
            assert_eq!(file.contents, *expected);
            assert_eq!(file.witness.path, *path);
            assert_eq!(file.witness.mode, "100644");
            assert_eq!(file.witness.blob, fixture.blob(path));
            assert_eq!(file.witness.bytes, u64::try_from(expected.len()).unwrap());
            assert_eq!(file.witness.sha256.len(), 64);
        }
        let schema =
            &snapshot.files["crates/auctioneer-bidder/src/bidders/fake_fixture/parameters.json"];
        assert_eq!(
            schema.contents,
            serde_json::to_vec(&serde_json::json!({"type":"string"})).unwrap()
        );
        assert_eq!(
            snapshot.files["Cargo.lock"].witness.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let reread = fixture
            .read()
            .expect("reader must leave repository unchanged");
        assert_eq!(snapshot, reread);
    }

    #[test]
    fn a_complete_shallow_tip_does_not_require_its_parent_commit() {
        let mut fixture = Fixture::new();
        let parent = fixture.commit.clone();
        fs::write(fixture.root.join("outside-closure"), b"second tip").unwrap();
        git(&fixture.root, &["add", "outside-closure"], None);
        fixture.commit_index();
        fs::write(
            fixture.root.join(".git/shallow"),
            format!("{}\n", fixture.commit),
        )
        .unwrap();
        fs::remove_file(fixture.object(&parent)).unwrap();
        assert_eq!(
            git(
                &fixture.root,
                &["rev-parse", "--is-shallow-repository"],
                None
            ),
            b"true\n"
        );
        let snapshot = fixture.read().expect("complete shallow object closure");
        assert_eq!(snapshot.files.len(), 20);
    }

    #[test]
    fn mutable_checkout_edits_are_not_reference_input() {
        let fixture = Fixture::new();
        let before = fixture.read().expect("initial source");
        fs::write(fixture.root.join("Cargo.lock"), b"uncommitted substitution").unwrap();
        fs::write(
            fixture
                .root
                .join("crates/auctioneer-bidder/src/bidders/untracked.rs"),
            b"untracked",
        )
        .unwrap();
        assert_eq!(
            fixture.read().expect("Git objects remain unchanged"),
            before
        );
    }

    #[test]
    fn a_replacement_commit_cannot_redirect_pinned_source() {
        let fixture = Fixture::new();
        let other = Fixture::new();
        let replacement = line(git(
            &fixture.root,
            &["commit-tree", &fixture.tree, "-m", "different commit"],
            None,
        ));
        git(
            &fixture.root,
            &[
                "update-ref",
                &format!("refs/replace/{}", fixture.commit),
                &replacement,
            ],
            None,
        );
        assert_ne!(replacement, fixture.commit);
        assert_eq!(
            fixture
                .read()
                .expect("replacement objects must be disabled"),
            other.read().expect("independent identical fixture source")
        );
    }

    #[test]
    fn absent_directory_refuses_instead_of_discovering_a_parent_repository() {
        let fixture = Fixture::new();
        assert_eq!(
            read_pinned(
                &fixture.root.join("absent"),
                &fixture.commit,
                &fixture.tree,
                20,
                fixture.bytes()
            ),
            Err(Error::Root)
        );
        let child = fixture.root.join("child");
        assert!(!child.exists());
        fs::create_dir_all(&child).unwrap();
        assert_eq!(
            read_pinned(&child, &fixture.commit, &fixture.tree, 20, fixture.bytes()),
            Err(Error::Root)
        );
    }

    #[test]
    fn head_and_tree_are_both_mandatory_pins() {
        let fixture = Fixture::new();
        assert_eq!(
            read_pinned(
                &fixture.root,
                &"0".repeat(40),
                &fixture.tree,
                20,
                fixture.bytes()
            ),
            Err(Error::Identity)
        );
        assert_eq!(
            read_pinned(
                &fixture.root,
                &fixture.commit,
                &"f".repeat(40),
                20,
                fixture.bytes()
            ),
            Err(Error::Identity)
        );
    }

    #[test]
    fn partial_promisor_and_filter_configuration_refuse_even_with_complete_objects() {
        for (key, value) in [
            ("extensions.partialclone", "origin"),
            ("remote.fixture.promisor", "true"),
            ("remote.fixture.partialclonefilter", "blob:none"),
        ] {
            let fixture = Fixture::new();
            git(
                &fixture.root,
                &["config", "core.repositoryformatversion", "1"],
                None,
            );
            git(&fixture.root, &["config", key, value], None);
            assert_eq!(fixture.read(), Err(Error::Partial), "{key}");
        }
    }

    #[test]
    fn promisor_pack_marker_refuses_without_a_configuration_flag() {
        let fixture = Fixture::new();
        let pack = fixture.root.join(".git/objects/pack");
        fs::create_dir_all(&pack).unwrap();
        fs::write(pack.join(format!("{}.promisor", "0".repeat(40))), b"").unwrap();
        assert_eq!(fixture.read(), Err(Error::Partial));
    }

    #[test]
    fn missing_required_blob_is_not_an_empty_or_matched_snapshot() {
        let fixture = Fixture::new();
        let blob = fixture.blob("Cargo.lock");
        fs::remove_file(fixture.object(&blob)).unwrap();
        assert_eq!(fixture.read(), Err(Error::MissingObject));
    }

    #[test]
    fn same_length_object_substitution_does_not_pass_a_length_only_check() {
        let fixture = Fixture::new();
        let before = &fixture.files["Cargo.lock"];
        let after = vec![b'x'; before.len()];
        assert_eq!(before.len(), after.len());
        assert_ne!(before, &after);
        fixture.corrupt_blob(&after);
        assert_eq!(fixture.read(), Err(Error::Witness));
    }

    #[test]
    fn changed_object_length_refuses() {
        let fixture = Fixture::new();
        let mut changed = fixture.files["Cargo.lock"].clone();
        changed.push(b'x');
        fixture.corrupt_blob(&changed);
        assert_eq!(fixture.read(), Err(Error::Witness));
    }

    #[test]
    fn independently_wrong_closure_census_refuses() {
        let fixture = Fixture::new();
        assert_eq!(
            read_pinned(
                &fixture.root,
                &fixture.commit,
                &fixture.tree,
                21,
                fixture.bytes()
            ),
            Err(Error::Witness)
        );
        assert_eq!(
            read_pinned(
                &fixture.root,
                &fixture.commit,
                &fixture.tree,
                20,
                fixture.bytes() + 1
            ),
            Err(Error::Witness)
        );
    }

    #[test]
    fn existing_staging_per_blob_cap_is_not_relaxed() {
        let mut fixture = Fixture::new();
        let cap = usize::try_from(super::MAX_INPUT_BYTES).unwrap();
        let bytes = vec![b'x'; cap + 1];
        fs::write(fixture.root.join("Cargo.lock"), &bytes).unwrap();
        fixture.files.insert("Cargo.lock".into(), bytes);
        git(&fixture.root, &["add", "Cargo.lock"], None);
        fixture.commit_index();
        assert_eq!(fixture.read(), Err(Error::Limit));
    }

    #[test]
    fn nonregular_modes_cannot_be_consumed_as_source_files() {
        for mode in ["120000", "160000"] {
            let mut fixture = Fixture::new();
            let oid = if mode == "160000" {
                fixture.commit.clone()
            } else {
                fixture.blob("Cargo.lock")
            };
            git(
                &fixture.root,
                &["update-index", "--cacheinfo", mode, &oid, "Cargo.lock"],
                None,
            );
            fixture.commit_index();
            assert_eq!(fixture.read(), Err(Error::Mode), "{mode}");
        }
    }

    #[test]
    fn parsed_paths_reject_traversal_aliasing_and_platform_separators() {
        for path in ["../escape", "/escape", "a//b", "a/./b", "a/../b", "a\\b"] {
            let record = format!("100644 blob {} 3\t{path}\0", "a".repeat(40));
            assert_eq!(parse_listing(record.as_bytes()), Err(Error::Path), "{path}");
        }
        assert_eq!(parse_listing(b""), Err(Error::Witness));
    }

    #[test]
    fn closure_filters_are_exact_and_do_not_select_old_waves_or_all_vendor_data() {
        for path in [
            "Cargo.lock",
            "crates/auctioneer-bidder/src/bidders/fake_fixture/tests/raw-capture.bin",
            "crates/auctioneer-bidder/src/bidders/fake_first/tests/schema/inventory-exclusion.toml",
            "crates/auctioneer-bidder/src/bidders/fake_second/tests/schema/inventory-exclusion.toml",
            "vendor/jsonschema-macros-core/Cargo.toml",
            "vendor/jsonschema-macros-core/benches/generate.rs",
            "vendor/regex-automata/src/lib.rs",
        ] {
            assert!(included(path), "{path}");
        }
        for path in [
            "crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority.toml",
            "crates/auctioneer-bidder/tests/conformance/fixtures/waves/priority-supplemental.toml",
            "crates/auctioneer-bidder/src/bidders_extra/file.rs",
            "vendor/jsonschema-macros-core/Cargo.toml.orig",
            "vendor/jsonschema-macros-core/Cargo.lock",
            "vendor/jsonschema-macros-core/src/example.snap",
            "vendor/regex-automata-extra/src/lib.rs",
            "vendor/regex-automata/src/ignored.RS",
            "vendor/unknown-reference/source.rs",
        ] {
            assert!(!included(path), "{path}");
        }
    }
    #[test]
    fn missing_required_subtree_is_not_recovered_from_the_checkout() {
        let fixture = Fixture::new();
        let oid = fixture.blob("crates/auctioneer-bidder/src/bidders");
        fs::remove_file(fixture.object(&oid)).unwrap();
        assert_eq!(fixture.read(), Err(Error::MissingObject));
    }

    #[test]
    fn a_nonempty_alternate_store_is_not_a_reference_fallback() {
        let fixture = Fixture::new();
        let other = Fixture::new();
        fs::write(
            fixture.root.join(".git/objects/info/alternates"),
            format!("{}\n", other.root.join(".git/objects").display()),
        )
        .unwrap();
        assert_eq!(fixture.read(), Err(Error::Partial));
    }

    #[test]
    fn file_and_relative_paths_are_not_repository_roots() {
        let fixture = Fixture::new();
        let file = fixture.root.join("Cargo.lock");
        for root in [Path::new("."), file.as_path()] {
            assert_eq!(
                read_pinned(root, &fixture.commit, &fixture.tree, 20, fixture.bytes()),
                Err(Error::Root)
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn a_symlink_root_is_not_a_regular_snapshot_location() {
        let fixture = Fixture::new();
        let aliases = tempfile::tempdir().unwrap();
        let alias = aliases.path().join("reference");
        symlink(&fixture.root, &alias).unwrap();
        assert_eq!(
            read_pinned(&alias, &fixture.commit, &fixture.tree, 20, fixture.bytes()),
            Err(Error::Root)
        );
    }

    #[test]
    fn a_regular_executable_blob_is_not_confused_with_a_symlink_or_tree() {
        let record = format!("100755 blob {} 3\tCargo.lock\0", "a".repeat(40));
        let listing = parse_listing(record.as_bytes()).expect("regular executable mode");
        assert_eq!(listing.files.len(), 1);
        assert_eq!(listing.files["Cargo.lock"].mode, "100755");
        assert_eq!(listing.files["Cargo.lock"].bytes, 3);
        let duplicate = format!("{record}{record}");
        assert_eq!(parse_listing(duplicate.as_bytes()), Err(Error::Witness));
        assert_eq!(
            parse_listing(format!("{record}\0").as_bytes()),
            Err(Error::Witness)
        );
    }

    #[test]
    fn command_construction_disables_replacements_configuration_and_network_helpers() {
        use std::ffi::OsStr;

        let command = super::git_command(Path::new("/"));
        assert_eq!(command.get_program(), "/usr/bin/git");
        let arguments = command
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>();
        for required in [
            "--no-replace-objects",
            "--no-optional-locks",
            "--literal-pathspecs",
            "core.fsmonitor=false",
            "core.hooksPath=/dev/null",
            "protocol.allow=never",
            "gc.auto=0",
            "maintenance.auto=false",
        ] {
            assert!(arguments.contains(&required), "{required}");
        }
        let environment = command.get_envs().collect::<BTreeMap<_, _>>();
        for (key, value) in [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_SYSTEM", "/dev/null"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_ALLOW_PROTOCOL", ""),
            ("GIT_TERMINAL_PROMPT", "0"),
        ] {
            assert_eq!(
                environment.get(OsStr::new(key)),
                Some(&Some(OsStr::new(value)))
            );
        }
    }

    #[test]
    fn materialization_uses_captured_bytes_after_the_repository_is_gone() {
        let fixture = Fixture::new();
        let expected = fixture.files.clone();
        let snapshot = fixture.read().expect("verified Git data");
        let commit = fixture.commit.clone();
        let tree = fixture.tree.clone();
        drop(fixture);
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().canonicalize().unwrap().join("source");
        snapshot
            .materialize(&destination)
            .expect("captured bytes need no repository");
        assert_eq!(snapshot.commit(), commit);
        assert_eq!(snapshot.tree(), tree);
        assert_eq!(snapshot.files().len(), 20);
        assert_eq!(snapshot.witnesses().len(), 20);
        assert_eq!(
            snapshot
                .witnesses()
                .map(|witness| &witness.path)
                .collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>()
        );
        for (path, bytes) in expected {
            let target = destination.join(path);
            assert_eq!(fs::read(&target).unwrap(), bytes);
            assert!(fs::symlink_metadata(&target).unwrap().is_file());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                assert_eq!(
                    fs::metadata(target).unwrap().permissions().mode() & 0o777,
                    0o600
                );
            }
        }
    }

    #[test]
    fn materialization_never_merges_or_overwrites_an_existing_destination() {
        let fixture = Fixture::new();
        let snapshot = fixture.read().unwrap();
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().canonicalize().unwrap().join("source");
        assert!(!destination.exists());
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("sentinel"), b"keep").unwrap();
        assert_eq!(snapshot.materialize(&destination), Err(Error::Destination));
        assert_eq!(fs::read(destination.join("sentinel")).unwrap(), b"keep");
        assert!(!destination.join("Cargo.lock").exists());
    }

    #[test]
    fn materialization_preflights_changed_bytes_before_creating_output() {
        let fixture = Fixture::new();
        let mut snapshot = fixture.read().unwrap();
        snapshot
            .files
            .get_mut("Cargo.lock")
            .unwrap()
            .contents
            .push(b'x');
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().canonicalize().unwrap().join("source");
        assert_eq!(snapshot.materialize(&destination), Err(Error::Witness));
        assert!(!destination.exists());
    }

    #[test]
    fn materialization_refuses_a_changed_captured_path_before_creating_output() {
        let fixture = Fixture::new();
        let mut snapshot = fixture.read().unwrap();
        snapshot.files.get_mut("Cargo.lock").unwrap().witness.path = "../escape".into();
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().canonicalize().unwrap().join("source");
        assert_eq!(snapshot.materialize(&destination), Err(Error::Path));
        assert!(!destination.exists());
    }

    #[test]
    #[cfg(unix)]
    fn materialization_does_not_follow_a_symlinked_destination_parent() {
        let fixture = Fixture::new();
        let snapshot = fixture.read().unwrap();
        let output = tempfile::tempdir().unwrap();
        let root = output.path().canonicalize().unwrap();
        let actual = root.join("actual");
        assert!(!actual.exists());
        fs::create_dir_all(&actual).unwrap();
        let alias = root.join("alias");
        symlink(&actual, &alias).unwrap();
        assert_eq!(
            snapshot.materialize(&alias.join("source")),
            Err(Error::Destination)
        );
        assert!(!actual.join("source").exists());
    }
}

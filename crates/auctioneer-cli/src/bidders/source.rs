use super::Error;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::from_utf8;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::fs::canonicalize;
use tokio::io::{AsyncRead, AsyncReadExt as _};
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout_at};

pub(super) const REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";

const PINNED: &[(&str, &str, &str, &str)] = &[
    (
        "static/bidder-info",
        "040000",
        "tree",
        "79921209b19c85f0a2ed50c6651080e707ce7fbc",
    ),
    (
        "static/bidder-params",
        "040000",
        "tree",
        "210ccd2c6466edc72f0d738493119a448e11b7fe",
    ),
    (
        "adapters",
        "040000",
        "tree",
        "801ecb3279997b0d1d668d72cc23259053d1138f",
    ),
    (
        "exchange/adapter_builders.go",
        "100755",
        "blob",
        "630fbe348b94d3196cb2a330a7f8ff0790eb5e03",
    ),
    (
        "LICENSE",
        "100644",
        "blob",
        "b7fa401fa4b79b208b1dbc4679b09b8f4691adbf",
    ),
];
const COMMAND_CAP: usize = 4 << 20;
const BLOB_CAP: usize = COMMAND_CAP;
const STDERR_CAP: usize = 64 << 10;
const TOTAL_CAP: usize = 64 << 20;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug)]
pub(super) struct Entry {
    pub path: String,
    pub oid: String,
    pub bytes: usize,
}

#[derive(Debug)]
pub(super) struct GitSource {
    path: PathBuf,
    deadline: Instant,
    remaining: AtomicUsize,
    known: BTreeMap<String, (String, usize)>,
}

impl GitSource {
    pub async fn open(path: &Path) -> Result<Self, Error> {
        let deadline = Instant::now()
            .checked_add(OPERATION_TIMEOUT)
            .ok_or(Error::SourceLimit)?;
        let repository = timeout_at(deadline, canonicalize(path))
            .await
            .map_err(|_error| Error::SourceLimit)?
            .map_err(|_error| Error::Source)?;
        let mut source = Self {
            path: repository,
            deadline,
            remaining: AtomicUsize::new(TOTAL_CAP),
            known: BTreeMap::new(),
        };
        // Do not discover an unrelated repository above the supplied directory.
        if source.run(&["rev-parse", "--show-prefix"], 4096).await? != b"\n" {
            return Err(Error::InvalidSource);
        }
        if source.run(&["cat-file", "-t", REVISION], 32).await? != b"commit\n" {
            return Err(Error::InvalidSource);
        }
        let mut arguments = vec!["ls-tree", "-z", "--full-tree", REVISION, "--"];
        arguments.extend(PINNED.iter().map(|(root, _mode, _kind, _oid)| *root));
        verify_pins(&source.run(&arguments, COMMAND_CAP).await?)?;
        for (_path, _mode, kind, oid) in PINNED {
            let output = source.run(&["cat-file", "-t", oid], 32).await?;
            if output != format!("{kind}\n").as_bytes() {
                return Err(Error::InvalidSource);
            }
        }
        Ok(source)
    }

    pub async fn entries(&mut self, path: &str) -> Result<Vec<Entry>, Error> {
        if !owned_root(path) {
            return Err(Error::InvalidSource);
        }
        let output = self
            .run(
                &["ls-tree", "-rlz", "--full-tree", REVISION, "--", path],
                COMMAND_CAP,
            )
            .await?;
        let entries = parse_entries(&output, path)?;
        if entries.is_empty() {
            return Err(Error::InvalidSource);
        }
        for entry in &entries {
            self.known
                .insert(entry.path.clone(), (entry.oid.clone(), entry.bytes));
        }
        Ok(entries)
    }

    pub async fn implementation_trees(&mut self) -> Result<BTreeMap<String, String>, Error> {
        let object = format!("{REVISION}:adapters");
        let output = self.run(&["ls-tree", "-dz", &object], COMMAND_CAP).await?;
        parse_implementation_trees(&output)
    }

    pub async fn blob(&mut self, entry: &Entry) -> Result<Vec<u8>, Error> {
        if !owned_path(&entry.path) || !valid_oid(&entry.oid) {
            return Err(Error::InvalidSource);
        }
        if entry.bytes > BLOB_CAP {
            return Err(Error::SourceLimit);
        }
        if !self
            .known
            .get(&entry.path)
            .is_some_and(|(oid, bytes)| oid == &entry.oid && *bytes == entry.bytes)
        {
            return Err(Error::InvalidSource);
        }
        let bytes = self
            .run(&["cat-file", "blob", &entry.oid], entry.bytes)
            .await?;
        if bytes.len() != entry.bytes {
            return Err(Error::InvalidSource);
        }
        Ok(bytes)
    }

    async fn run(&mut self, arguments: &[&str], stdout_cap: usize) -> Result<Vec<u8>, Error> {
        if Instant::now() >= self.deadline || self.remaining.load(Ordering::Relaxed) == 0 {
            return Err(Error::SourceLimit);
        }
        let mut child = git_command(&self.path)
            .args(arguments)
            .spawn()
            .map_err(|_error| Error::Source)?;
        self.capture(&mut child, stdout_cap).await
    }

    async fn capture(&mut self, child: &mut Child, stdout_cap: usize) -> Result<Vec<u8>, Error> {
        let deadline = Instant::now()
            .checked_add(COMMAND_TIMEOUT)
            .map_or(self.deadline, |end| self.deadline.min(end));
        let command_remaining = AtomicUsize::new(COMMAND_CAP);
        let result = timeout_at(deadline, async {
            let pipe = child.stdout.take().ok_or(Error::Source)?;
            let stderr = child.stderr.take().ok_or(Error::Source)?;
            let (stdout, _stderr, status) = tokio::try_join!(
                read_bounded(pipe, stdout_cap, &command_remaining, &self.remaining),
                read_bounded(stderr, STDERR_CAP, &command_remaining, &self.remaining),
                async { child.wait().await.map_err(|_error| Error::Source) },
            )?;
            if !status.success() {
                return Err(Error::Source);
            }
            Ok(stdout)
        })
        .await
        .unwrap_or(Err(Error::SourceLimit));
        if result.is_err() {
            // Fence failed captures as well as successful reads against the operation budget.
            self.remaining.store(0, Ordering::Relaxed);
            let killed = child.start_kill();
            let reaped = child.wait().await;
            if killed.is_err() || reaped.is_err() {
                return Err(Error::Source);
            }
        }
        result
    }
}

pub(super) fn git_command(path: &Path) -> Command {
    let mut command = Command::new("git");
    command.env_clear();
    if let Some(search_path) = env::var_os("PATH") {
        command.env("PATH", search_path);
    }
    command
        .current_dir(path)
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
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

pub(super) async fn read_bounded<R: AsyncRead + Unpin>(
    mut reader: R,
    cap: usize,
    command_remaining: &AtomicUsize,
    total_remaining: &AtomicUsize,
) -> Result<Vec<u8>, Error> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let available = cap
            .saturating_sub(output.len())
            .min(command_remaining.load(Ordering::Relaxed))
            .min(total_remaining.load(Ordering::Relaxed));
        // One extra byte distinguishes an exact-size stream from a truncated one.
        let read_cap = available.saturating_add(1).min(buffer.len());
        let chunk = buffer.get_mut(..read_cap).ok_or(Error::SourceLimit)?;
        let count = reader.read(chunk).await.map_err(|_error| Error::Source)?;
        if count == 0 {
            return Ok(output);
        }
        for remaining in [command_remaining, total_remaining] {
            remaining
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_sub(count)
                })
                .map_err(|_value| Error::SourceLimit)?;
        }
        if count > cap.saturating_sub(output.len()) {
            return Err(Error::SourceLimit);
        }
        output.extend_from_slice(buffer.get(..count).ok_or(Error::SourceLimit)?);
    }
}

fn valid_oid(oid: &str) -> bool {
    oid.len() == 40
        && oid
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn owned_root(path: &str) -> bool {
    PINNED
        .iter()
        .any(|(root, _mode, _kind, _oid)| path == *root)
}

fn clean_path(path: &str) -> bool {
    !path
        .bytes()
        .any(|byte| byte.is_ascii_control() || byte == b'\\')
        && !path
            .split('/')
            .any(|component| matches!(component, "" | "." | ".."))
}

fn within_root(path: &str, root: &str, kind: &str) -> bool {
    if kind == "blob" {
        path == root
    } else {
        path.strip_prefix(root)
            .and_then(|suffix| suffix.strip_prefix('/'))
            .is_some_and(|suffix| !suffix.is_empty())
    }
}

fn owned_path(path: &str) -> bool {
    clean_path(path)
        && PINNED
            .iter()
            .any(|(root, _mode, kind, _oid)| within_root(path, root, kind))
}

fn regular_mode(root: &str, mode: &str) -> bool {
    match root {
        "static/bidder-info" | "static/bidder-params" | "LICENSE" => mode == "100644",
        "exchange/adapter_builders.go" => mode == "100755",
        "adapters" => matches!(mode, "100644" | "100755"),
        _ => false,
    }
}

fn parse_entries(output: &[u8], root: &str) -> Result<Vec<Entry>, Error> {
    if !owned_root(root) {
        return Err(Error::InvalidSource);
    }
    if output.len() > COMMAND_CAP {
        return Err(Error::SourceLimit);
    }
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    if output.is_empty() {
        return Ok(entries);
    }
    let records = output.strip_suffix(&[0]).ok_or(Error::InvalidSource)?;
    for raw_record in records.split(|byte| *byte == 0) {
        let record = from_utf8(raw_record).map_err(|_error| Error::InvalidSource)?;
        let (header, path) = record.split_once('\t').ok_or(Error::InvalidSource)?;
        if header.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(Error::InvalidSource);
        }
        let mut fields = header.split_ascii_whitespace();
        let mode = fields.next().ok_or(Error::InvalidSource)?;
        if !regular_mode(root, mode) || fields.next() != Some("blob") {
            return Err(Error::InvalidSource);
        }
        let oid = fields.next().ok_or(Error::InvalidSource)?;
        let size = fields.next().ok_or(Error::InvalidSource)?;
        let (_, _, kind, _) = PINNED
            .iter()
            .find(|(pinned_path, _, _, _)| *pinned_path == root)
            .ok_or(Error::InvalidSource)?;
        if fields.next().is_some()
            || !valid_oid(oid)
            || !clean_path(path)
            || !within_root(path, root, kind)
            || !seen.insert(path)
            || !size.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(Error::InvalidSource);
        }
        let bytes = size
            .parse::<usize>()
            .map_err(|_error| Error::InvalidSource)?;
        if bytes > BLOB_CAP {
            return Err(Error::SourceLimit);
        }
        entries.push(Entry {
            path: path.to_owned(),
            oid: oid.to_owned(),
            bytes,
        });
    }
    Ok(entries)
}

fn verify_pins(output: &[u8]) -> Result<(), Error> {
    if output.len() > COMMAND_CAP {
        return Err(Error::SourceLimit);
    }
    let mut seen = BTreeSet::new();
    let records = output.strip_suffix(&[0]).ok_or(Error::InvalidSource)?;
    for raw_record in records.split(|byte| *byte == 0) {
        let record = from_utf8(raw_record).map_err(|_error| Error::InvalidSource)?;
        let (header, path) = record.split_once('\t').ok_or(Error::InvalidSource)?;
        if header.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(Error::InvalidSource);
        }
        let (_root, mode, kind, oid) = PINNED
            .iter()
            .find(|(root, _mode, _kind, _oid)| path == *root)
            .ok_or(Error::InvalidSource)?;
        let mut fields = header.split_ascii_whitespace();
        if fields.next() != Some(*mode)
            || fields.next() != Some(*kind)
            || fields.next() != Some(*oid)
            || fields.next().is_some()
            || !seen.insert(path)
        {
            return Err(Error::InvalidSource);
        }
    }
    if seen.len() != PINNED.len() {
        return Err(Error::InvalidSource);
    }
    Ok(())
}

fn parse_implementation_trees(output: &[u8]) -> Result<BTreeMap<String, String>, Error> {
    if output.len() > COMMAND_CAP {
        return Err(Error::SourceLimit);
    }
    let records = output.strip_suffix(&[0]).ok_or(Error::InvalidSource)?;
    let mut trees = BTreeMap::new();
    for record in records.split(|byte| *byte == 0) {
        let text = from_utf8(record).map_err(|_error| Error::InvalidSource)?;
        let (header, name) = text.split_once('\t').ok_or(Error::InvalidSource)?;
        let mut fields = header.split_ascii_whitespace();
        if fields.next() != Some("040000") || fields.next() != Some("tree") {
            return Err(Error::InvalidSource);
        }
        let oid = fields.next().ok_or(Error::InvalidSource)?;
        if fields.next().is_some()
            || header.bytes().any(|byte| byte.is_ascii_control())
            || !valid_oid(oid)
            || !clean_path(name)
            || name.contains('/')
            || trees
                .insert(format!("adapters/{name}"), oid.to_owned())
                .is_some()
        {
            return Err(Error::InvalidSource);
        }
    }
    if trees.is_empty() {
        return Err(Error::InvalidSource);
    }
    Ok(trees)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::fmt::Write as _;
    use std::fs;
    use std::io::{self, Write as _};
    use std::thread;
    use tempfile::TempDir;

    const OID: &str = "37909b12066ad9239b488b9751d070212e90d61a";
    const ROOT: &str = "static/bidder-params";

    #[test]
    fn implementation_tree_witnesses_require_exact_directory_records() {
        let record = format!("040000 tree {OID}\tfake_one\0");
        assert_eq!(
            parse_implementation_trees(record.as_bytes()),
            Ok(BTreeMap::from([(
                "adapters/fake_one".to_owned(),
                OID.to_owned()
            )]))
        );
        for rejected in [
            String::new(),
            "\0".to_owned(),
            record.trim_end_matches('\0').to_owned(),
            record.replace("040000", "100644"),
            record.replace(" tree ", " blob "),
            record.replace(OID, "not-an-object"),
            record.replace("tree ", "tree\n"),
            record.replace("\tfake_one", " extra\tfake_one"),
            record.replace("\tfake_one", "\t../escape"),
            record.replace("\tfake_one", "\tother/child"),
            format!("{record}{record}"),
        ] {
            assert_eq!(
                parse_implementation_trees(rejected.as_bytes()),
                Err(Error::InvalidSource)
            );
        }
        assert_eq!(
            parse_implementation_trees(&vec![b'a'; COMMAND_CAP + 1]),
            Err(Error::SourceLimit)
        );
    }

    fn record(mode: &str, kind: &str, oid: &str, size: &str, path: &str) -> Vec<u8> {
        format!("{mode} {kind} {oid} {size}\t{path}\0").into_bytes()
    }

    #[test]
    fn tree_records_preserve_raw_sizes_and_paths() {
        let mut bytes = record(
            "100644",
            "blob",
            OID,
            "   3778",
            &format!("{ROOT}/fake_one.json"),
        );
        bytes.extend(record(
            "100644",
            "blob",
            OID,
            "0",
            &format!("{ROOT}/fake_a file.json"),
        ));
        let entries = parse_entries(&bytes, ROOT).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "static/bidder-params/fake_one.json");
        assert_eq!(entries[0].oid, OID);
        assert_eq!(entries[0].bytes, 3778);
        assert_eq!(entries[1].bytes, 0);
        assert!(parse_entries(b"", ROOT).unwrap().is_empty());
        let adapters = parse_entries(
            &record(
                "100755",
                "blob",
                OID,
                "1",
                "adapters/fake_example/source.go",
            ),
            "adapters",
        )
        .unwrap();
        assert_eq!(adapters.len(), 1);
        let builders = parse_entries(
            &record("100755", "blob", OID, "1", "exchange/adapter_builders.go"),
            "exchange/adapter_builders.go",
        )
        .unwrap();
        assert_eq!(builders.len(), 1);
    }

    #[test]
    fn tree_records_reject_nonregular_modes_and_unknown_types() {
        for (mode, kind) in [
            ("120000", "blob"),
            ("040000", "tree"),
            ("160000", "commit"),
            ("100600", "blob"),
            ("100644", "unknown"),
            ("100644", "tree"),
            ("100755", "blob"),
        ] {
            let bytes = record(mode, kind, OID, "1", &format!("{ROOT}/fake_a.json"));
            assert_eq!(
                parse_entries(&bytes, ROOT).unwrap_err(),
                Error::InvalidSource
            );
        }
        assert_eq!(
            parse_entries(
                &record("100644", "blob", OID, "1", "exchange/adapter_builders.go"),
                "exchange/adapter_builders.go"
            )
            .unwrap_err(),
            Error::InvalidSource
        );
    }

    #[test]
    fn tree_records_reject_malformed_headers_sizes_and_delimiters() {
        for bytes in [
            format!("100644 blob {OID} 1\t{ROOT}/fake_a.json").into_bytes(),
            format!("100644 blob {OID}\t{ROOT}/fake_a.json\0").into_bytes(),
            format!("100644 blob {OID} 1 extra\t{ROOT}/fake_a.json\0").into_bytes(),
            format!("100644\nblob {OID} 1\t{ROOT}/fake_a.json\0").into_bytes(),
            record("100644", "blob", OID, "-1", &format!("{ROOT}/fake_a.json")),
            record("100644", "blob", OID, "+1", &format!("{ROOT}/fake_a.json")),
            record(
                "100644",
                "blob",
                OID,
                "184467440737095516160",
                &format!("{ROOT}/fake_a.json"),
            ),
            vec![0],
            vec![0xff, 0],
        ] {
            assert_eq!(
                parse_entries(&bytes, ROOT).unwrap_err(),
                Error::InvalidSource,
                "accepted {bytes:?}"
            );
        }
        let mut duplicate = record("100644", "blob", OID, "1", &format!("{ROOT}/fake_a.json"));
        duplicate.extend_from_within(..);
        assert_eq!(
            parse_entries(&duplicate, ROOT).unwrap_err(),
            Error::InvalidSource
        );
    }

    #[test]
    fn paths_must_stay_in_the_exact_owned_roots() {
        for path in [
            "",
            "/static/bidder-params/a",
            "static/bidder-params",
            "static/bidder-params/",
            "static/bidder-params/../a",
            "static/bidder-params/./a",
            "static/bidder-params//a",
            "static/bidder-params-evil/a",
            "static/bidder-params/a\\b",
            "static/bidder-params/a\nb",
            "static/bidder-info/a.yaml",
            "exchange/adapter_builders.go/extra",
            ":(glob)**",
        ] {
            let bytes = record("100644", "blob", OID, "1", path);
            assert_eq!(
                parse_entries(&bytes, ROOT).unwrap_err(),
                Error::InvalidSource,
                "accepted {path:?}"
            );
        }
        assert_eq!(
            parse_entries(b"", ":(glob)**").unwrap_err(),
            Error::InvalidSource
        );
        assert_eq!(
            parse_entries(b"", "adapters/adapterstest").unwrap_err(),
            Error::InvalidSource
        );
        assert!(owned_path("adapters/fake_example/source.go"));
        assert!(owned_path("exchange/adapter_builders.go"));
        assert!(!owned_path("exchange/other.go"));
    }

    #[test]
    fn object_ids_are_exact_lowercase_sha1_names() {
        assert!(valid_oid(REVISION));
        assert!(valid_oid(OID));
        for oid in [
            "HEAD",
            "--help",
            "",
            OID.get(..39).unwrap(),
            &format!("{OID}0"),
            &OID.to_uppercase(),
            &"0".repeat(64),
        ] {
            assert!(!valid_oid(oid), "accepted {oid:?}");
        }
        assert!(!valid_oid("g7909b12066ad9239b488b9751d070212e90d61a"));
    }

    #[test]
    fn tree_blob_limits_include_the_exact_boundary() {
        let path = format!("{ROOT}/fake_a.json");
        let exact = record("100644", "blob", OID, &BLOB_CAP.to_string(), &path);
        assert_eq!(parse_entries(&exact, ROOT).unwrap()[0].bytes, BLOB_CAP);
        let over = record("100644", "blob", OID, &(BLOB_CAP + 1).to_string(), &path);
        assert_eq!(parse_entries(&over, ROOT).unwrap_err(), Error::SourceLimit);
        assert_eq!(
            parse_entries(&vec![b'x'; COMMAND_CAP + 1], ROOT).unwrap_err(),
            Error::SourceLimit
        );
    }

    fn pinned_listing() -> Vec<u8> {
        let mut listing = String::new();
        for (path, mode, kind, oid) in PINNED {
            write!(&mut listing, "{mode} {kind} {oid}\t{path}\0").unwrap();
        }
        listing.into_bytes()
    }

    #[test]
    fn pinned_objects_require_exact_members_types_and_identities() {
        let bytes = pinned_listing();
        verify_pins(&bytes).unwrap();
        let first_end = bytes.iter().position(|byte| *byte == 0).unwrap();
        for changed in [
            bytes[..bytes.len().checked_sub(1).unwrap()].to_vec(),
            bytes[..=first_end].to_vec(),
            [bytes.clone(), bytes.clone()].concat(),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replacen("79921209", "09921209", 1)
                .into_bytes(),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replacen("040000 tree", "120000 blob", 1)
                .into_bytes(),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replacen("100755 blob", "100644 blob", 1)
                .into_bytes(),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replacen("040000 tree", "040000\ntree", 1)
                .into_bytes(),
            String::from_utf8(bytes)
                .unwrap()
                .replacen("static/bidder-info", "static/not-owned", 1)
                .into_bytes(),
        ] {
            assert_eq!(verify_pins(&changed).unwrap_err(), Error::InvalidSource);
        }
    }

    #[tokio::test]
    async fn bounded_reads_accept_exact_limits_and_reject_one_extra_byte() {
        for cap in [0, 1, 8192] {
            let bytes = vec![b'x'; cap];
            let command = AtomicUsize::new(cap);
            let total = AtomicUsize::new(cap);
            assert_eq!(
                read_bounded(bytes.as_slice(), cap, &command, &total)
                    .await
                    .unwrap(),
                bytes
            );
            assert_eq!(total.load(Ordering::Relaxed), 0);
            let over = cap.checked_add(1).unwrap();
            let excess_bytes = vec![b'x'; over];
            let excess_command = AtomicUsize::new(over);
            let excess_total = AtomicUsize::new(over);
            assert_eq!(
                read_bounded(excess_bytes.as_slice(), cap, &excess_command, &excess_total)
                    .await
                    .unwrap_err(),
                Error::SourceLimit
            );
        }
    }

    #[tokio::test]
    async fn bounded_reads_charge_both_command_and_aggregate_budgets() {
        let command = AtomicUsize::new(4);
        let total = AtomicUsize::new(3);
        read_bounded(&b"ab"[..], 4, &command, &total).await.unwrap();
        read_bounded(&b"c"[..], 4, &command, &total).await.unwrap();
        assert_eq!(
            read_bounded(&b"d"[..], 4, &command, &total)
                .await
                .unwrap_err(),
            Error::SourceLimit
        );
        let exhausted_command = AtomicUsize::new(0);
        let next_total = AtomicUsize::new(4);
        assert_eq!(
            read_bounded(&b"x"[..], 4, &exhausted_command, &next_total)
                .await
                .unwrap_err(),
            Error::SourceLimit
        );
    }

    #[tokio::test]
    async fn open_rejects_missing_paths_and_nonrepositories() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            GitSource::open(&root.path().join("missing"))
                .await
                .unwrap_err(),
            Error::Source
        );
        assert_eq!(
            GitSource::open(root.path()).await.unwrap_err(),
            Error::Source
        );
    }

    async fn fixture_blob(contents: &[u8]) -> (TempDir, GitSource, Entry) {
        let root = tempfile::tempdir().unwrap();
        let mut source = test_source(root.path());
        // Only fixture setup writes Git objects; the reader itself never does.
        let mut init = git_command(root.path())
            .args(["init", "--quiet"])
            .spawn()
            .unwrap();
        source.capture(&mut init, 4096).await.unwrap();
        fs::write(root.path().join("fixture"), contents).unwrap();
        let mut hash = git_command(root.path())
            .args(["hash-object", "-w", "--", "fixture"])
            .spawn()
            .unwrap();
        let output = source.capture(&mut hash, 41).await.unwrap();
        let oid = from_utf8(&output)
            .unwrap()
            .strip_suffix('\n')
            .unwrap()
            .to_owned();
        assert!(valid_oid(&oid));
        let entry = Entry {
            path: format!("{ROOT}/fake_a.json"),
            oid,
            bytes: contents.len(),
        };
        source
            .known
            .insert(entry.path.clone(), (entry.oid.clone(), entry.bytes));
        (root, source, entry)
    }

    #[tokio::test]
    async fn repository_without_the_pinned_commit_is_rejected() {
        let (root, _source, _entry) = fixture_blob(b"unrelated object").await;
        assert_eq!(
            GitSource::open(root.path()).await.unwrap_err(),
            Error::Source
        );
    }

    #[tokio::test]
    async fn blobs_read_object_bytes_not_dirty_checkout_contents() {
        for contents in [&b""[..], &b"original object\0bytes\n"[..]] {
            let (root, mut source, entry) = fixture_blob(contents).await;
            let checkout = root.path().join(&entry.path);
            fs::create_dir_all(checkout.parent().unwrap()).unwrap();
            fs::write(checkout, b"dirty checkout bytes").unwrap();
            assert_eq!(source.blob(&entry).await.unwrap(), contents);
        }
    }

    #[tokio::test]
    async fn blobs_must_have_the_exact_advertised_length() {
        for declared in [2, 4] {
            let (_root, mut source, mut entry) = fixture_blob(b"abc").await;
            entry.bytes = declared;
            source
                .known
                .insert(entry.path.clone(), (entry.oid.clone(), declared));
            let error = source.blob(&entry).await.unwrap_err();
            let expected = if declared == 2 {
                Error::SourceLimit
            } else {
                Error::InvalidSource
            };
            assert_eq!(error, expected);
        }
    }

    #[tokio::test]
    async fn blob_rejects_invalid_or_unlisted_entries_before_spawning() {
        let root = tempfile::tempdir().unwrap();
        let mut source = test_source(root.path());
        for (path, oid, bytes, expected) in [
            (
                "static/bidder-params/fake_a.json",
                "HEAD",
                1,
                Error::InvalidSource,
            ),
            ("../escape", OID, 1, Error::InvalidSource),
            (
                "static/bidder-params/fake_a.json",
                OID,
                1,
                Error::InvalidSource,
            ),
            (
                "static/bidder-params/fake_a.json",
                OID,
                BLOB_CAP + 1,
                Error::SourceLimit,
            ),
        ] {
            let entry = Entry {
                path: path.into(),
                oid: oid.into(),
                bytes,
            };
            assert_eq!(source.blob(&entry).await.unwrap_err(), expected);
        }
        assert_eq!(
            source.entries("../escape").await.unwrap_err(),
            Error::InvalidSource
        );
        source
            .known
            .insert(format!("{ROOT}/fake_a.json"), (OID.to_owned(), 2));
        let entry = Entry {
            path: format!("{ROOT}/fake_a.json"),
            oid: OID.to_owned(),
            bytes: 1,
        };
        assert_eq!(source.blob(&entry).await.unwrap_err(), Error::InvalidSource);
    }

    #[test]
    fn git_commands_disable_ambient_configuration_helpers_and_fetching() {
        let command = git_command(Path::new("."));
        let args: Vec<_> = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect();
        for required in [
            "--no-replace-objects",
            "--no-optional-locks",
            "--literal-pathspecs",
            "core.fsmonitor=false",
            "core.hooksPath=/dev/null",
            "protocol.allow=never",
        ] {
            assert!(args.contains(&required), "missing {required}");
        }
        let environment: BTreeMap<_, _> = command.as_std().get_envs().collect();
        for (key, value) in [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_SYSTEM", "/dev/null"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_TERMINAL_PROMPT", "0"),
        ] {
            assert_eq!(
                environment.get(OsStr::new(key)),
                Some(&Some(OsStr::new(value)))
            );
        }
        assert!(!environment.contains_key(OsStr::new("GIT_DIR")));
        assert!(!environment.contains_key(OsStr::new("GIT_ALTERNATE_OBJECT_DIRECTORIES")));
    }

    #[tokio::test]
    async fn exhausted_operation_budget_refuses_before_spawning() {
        let root = tempfile::tempdir().unwrap();
        let mut source = test_source(root.path());
        source.remaining.store(0, Ordering::Relaxed);
        assert_eq!(source.entries(ROOT).await.unwrap_err(), Error::SourceLimit);
        source.remaining.store(TOTAL_CAP, Ordering::Relaxed);
        source.deadline = Instant::now();
        assert_eq!(source.entries(ROOT).await.unwrap_err(), Error::SourceLimit);
    }

    fn child_command(mode: &str) -> Command {
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "bidders::source::tests::subprocess_fixture",
                "--nocapture",
            ])
            .env("AUCTIONEER__TEST__SOURCE_TEST_CHILD", mode)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }

    fn test_source(path: &Path) -> GitSource {
        GitSource {
            path: path.to_owned(),
            deadline: Instant::now().checked_add(OPERATION_TIMEOUT).unwrap(),
            remaining: AtomicUsize::new(TOTAL_CAP),
            known: BTreeMap::new(),
        }
    }

    #[tokio::test]
    async fn timeout_and_output_caps_kill_and_reap_children() {
        for mode in ["sleep", "stdout", "stderr"] {
            let mut source = test_source(Path::new("."));
            if mode == "sleep" {
                source.deadline = Instant::now()
                    .checked_add(Duration::from_millis(100))
                    .unwrap();
            }
            let mut child = child_command(mode).spawn().unwrap();
            assert_eq!(
                source.capture(&mut child, 1024).await.unwrap_err(),
                Error::SourceLimit,
                "mode {mode}"
            );
            assert!(
                child.try_wait().unwrap().is_some(),
                "unreaped child: {mode}"
            );
        }
    }

    #[test]
    fn subprocess_fixture() -> io::Result<()> {
        match env::var("AUCTIONEER__TEST__SOURCE_TEST_CHILD").as_deref() {
            Ok("sleep") => thread::sleep(Duration::from_secs(30)),
            Ok("stdout") => io::stdout()
                .lock()
                .write_all(&vec![b'x'; COMMAND_CAP + 1])?,
            Ok("stderr") => io::stderr().lock().write_all(&vec![b'x'; STDERR_CAP + 1])?,
            _ => {}
        }
        Ok(())
    }
}

//! Frozen Git scope checks are mechanical validation, not human authorization.

use crate::bidders::Error;
use crate::bidders::source::{git_command, read_bounded};
use serde::Serialize;
use sha2::Digest as _;
use std::collections::BTreeSet;
use std::path::Path;
use std::str::from_utf8;
use std::sync::atomic::AtomicUsize;
use std::time::Duration;
use tokio::process::Child;
use tokio::time::{Instant, timeout, timeout_at};

const AUDIT_PATH: &str = "crates/auctioneer-core/tests/fixtures/resource_envelopes/audit.toml";
const PLAN_PATH: &str =
    "crates/auctioneer-core/tests/fixtures/resource_envelopes/remediation-plan.toml";
const HUMAN_AUDIT_PATH: &str = "docs/testing/adapter-resource-envelopes.md";
const FILE_CAP: usize = 1 << 20;
const OPERATION_CAP: usize = 16 << 20;
const STDERR_CAP: usize = 64 << 10;

#[derive(Clone, Copy)]
pub(crate) enum Selection<'a> {
    Staged,
    Head(&'a str),
}

#[derive(Debug, Serialize)]
pub(crate) struct ScopeReport {
    pub scope: &'static str,
    pub code_base_commit: String,
    pub review_commit: String,
    pub checked_head: String,
    pub staged: bool,
    pub audit_sha256: Option<String>,
    pub changed_files: BTreeSet<String>,
}

pub(crate) fn serialize(report: &ScopeReport) -> Result<Vec<u8>, Error> {
    let mut bytes = serde_json::to_vec(report).map_err(|_error| Error::Output)?;
    bytes.push(b'\n');
    if bytes.len() > super::MAX_BYTES {
        return Err(Error::Output);
    }
    Ok(bytes)
}

pub(crate) async fn run(
    root: &Path,
    review: &str,
    selection: Selection<'_>,
) -> Result<ScopeReport, Error> {
    let git = Git::open(root).await?;
    git.exact_commit(review).await?;
    let audit = git.blob(review, AUDIT_PATH).await?;
    let plan = git.blob(review, PLAN_PATH).await?;
    let validated = super::records::validate_frozen_pair(&audit, &plan)?;
    check_baseline_identity(
        &git,
        &validated.code_base_commit,
        &validated.lock_sha256,
        &validated.sdk_revision,
        &validated.toolchain,
    )
    .await?;
    let mut report = check_scope_with_git(
        &git,
        &validated.code_base_commit,
        review,
        &validated.files,
        selection,
    )
    .await?;
    report.audit_sha256 = Some(validated.audit_sha256);
    Ok(report)
}

#[cfg(test)]
async fn check_scope(
    root: &Path,
    base: &str,
    review: &str,
    files: &BTreeSet<String>,
    selection: Selection<'_>,
) -> Result<ScopeReport, Error> {
    check_scope_with_git(&Git::open(root).await?, base, review, files, selection).await
}

async fn check_scope_with_git(
    git: &Git<'_>,
    base: &str,
    review: &str,
    files: &BTreeSet<String>,
    selection: Selection<'_>,
) -> Result<ScopeReport, Error> {
    git.exact_commit(base).await?;
    git.exact_commit(review).await?;
    git.ancestor(base, review).await?;
    let baseline_changes = git.diff(&[base, review]).await?;
    let records = [AUDIT_PATH, PLAN_PATH, HUMAN_AUDIT_PATH];
    if base == review
        || ![AUDIT_PATH, PLAN_PATH]
            .iter()
            .all(|path| baseline_changes.contains(*path))
        || baseline_changes
            .iter()
            .any(|path| !records.contains(&path.as_str()))
    {
        return Err(Error::ResourceInput);
    }
    let head = git
        .resolve_commit(match selection {
            Selection::Head(head) => head,
            Selection::Staged => "HEAD",
        })
        .await?;
    git.ancestor(review, &head).await?;
    let mut changed_files = git.diff(&[review, &head]).await?;
    if matches!(selection, Selection::Staged) {
        // Union rather than diffing review-to-index: a staged revert must not
        // hide an out-of-scope change already committed since the review.
        changed_files.extend(git.diff(&["--cached", &head]).await?);
    }
    if changed_files
        .iter()
        .any(|path| matches!(path.as_str(), AUDIT_PATH | PLAN_PATH) || !files.contains(path))
    {
        return Err(Error::ResourceInput);
    }
    Ok(ScopeReport {
        scope: "mechanical_scope_only_not_human_approval_or_fit",
        code_base_commit: base.to_owned(),
        review_commit: review.to_owned(),
        checked_head: head,
        staged: matches!(selection, Selection::Staged),
        audit_sha256: None,
        changed_files,
    })
}

fn paths(bytes: &[u8]) -> Result<BTreeSet<String>, Error> {
    if bytes.is_empty() {
        return Ok(BTreeSet::new());
    }
    let data = bytes.strip_suffix(b"\0").ok_or(Error::ResourceInput)?;
    data.split(|byte| *byte == 0)
        .map(|raw_path| {
            let path = from_utf8(raw_path).map_err(|_error| Error::ResourceInput)?;
            if !super::records::exact_path(path) {
                return Err(Error::ResourceInput);
            }
            Ok(path.to_owned())
        })
        .collect()
}

fn raw_paths(bytes: &[u8]) -> Result<BTreeSet<String>, Error> {
    let mut output = BTreeSet::new();
    let mut fields = bytes.split_inclusive(|byte| *byte == 0);
    while let Some(raw_header) = fields.next() {
        let header_bytes = raw_header.strip_suffix(b"\0").ok_or(Error::ResourceInput)?;
        let header = from_utf8(header_bytes).map_err(|_error| Error::ResourceInput)?;
        let words: Vec<_> = header.split(' ').collect();
        let [prefixed_old_mode, new_mode, old_oid, new_oid, status] = words.as_slice() else {
            return Err(Error::ResourceInput);
        };
        let old_mode = prefixed_old_mode
            .strip_prefix(':')
            .ok_or(Error::ResourceInput)?;
        if ![old_mode, *new_mode]
            .iter()
            .all(|mode| matches!(*mode, "000000" | "100644" | "100755"))
            || !valid_sha(old_oid)
            || !valid_sha(new_oid)
            || !matches!(*status, "A" | "D" | "M" | "T")
            || (old_mode == "000000") != (*status == "A")
            || (*new_mode == "000000") != (*status == "D")
        {
            return Err(Error::ResourceInput);
        }
        // A symlink or gitlink is not an exact regular-file grant, even if its
        // path is listed. --no-renames emits both sides as independent rows.
        let path = fields.next().ok_or(Error::ResourceInput)?;
        output.extend(paths(path)?);
    }
    Ok(output)
}

async fn check_baseline_identity(
    git: &Git<'_>,
    base: &str,
    lock_sha256: &str,
    sdk_revision: &str,
    compiler_verbose: &str,
) -> Result<(), Error> {
    git.exact_commit(base).await?;
    let lock = git.blob(base, "Cargo.lock").await?;
    let manifest_bytes = git.blob(base, "Cargo.toml").await?;
    let versions_bytes = git.blob(base, ".tool-versions").await?;
    let manifest: toml::Value =
        toml::from_str(from_utf8(&manifest_bytes).map_err(|_error| Error::ResourceInput)?)
            .map_err(|_error| Error::ResourceInput)?;
    let pinned_sdk = manifest
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("edgezero-core"))
        .and_then(|value| value.get("rev"))
        .and_then(toml::Value::as_str)
        .ok_or(Error::ResourceInput)?;
    let versions = from_utf8(&versions_bytes).map_err(|_error| Error::ResourceInput)?;
    let rust: Vec<_> = versions
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            if words.next() != Some("rust") {
                return None;
            }
            Some((words.next(), words.next()))
        })
        .collect();
    let mut compiler = compiler_verbose
        .lines()
        .next()
        .ok_or(Error::ResourceInput)?
        .split_whitespace();
    if lock_sha256 != hex::encode(sha2::Sha256::digest(&lock))
        || pinned_sdk != sdk_revision
        || compiler.next() != Some("rustc")
        || rust.len() != 1
        || rust.first().is_none_or(|(version, extra)| {
            version.is_none() || extra.is_some() || *version != compiler.next()
        })
    {
        return Err(Error::ResourceInput);
    }
    Ok(())
}

struct Git<'a> {
    root: &'a Path,
    deadline: Instant,
    remaining: AtomicUsize,
    command_timeout: Duration,
}

impl<'a> Git<'a> {
    async fn open(root: &'a Path) -> Result<Self, Error> {
        let git = Self {
            root,
            deadline: Instant::now()
                .checked_add(Duration::from_secs(120))
                .ok_or(Error::ResourceInput)?,
            remaining: AtomicUsize::new(OPERATION_CAP),
            command_timeout: Duration::from_secs(10),
        };
        if git.run(&["rev-parse", "--show-prefix"], 4096).await? != b"\n" {
            return Err(Error::ResourceInput);
        }
        Ok(git)
    }

    async fn run(&self, args: &[&str], cap: usize) -> Result<Vec<u8>, Error> {
        if Instant::now() >= self.deadline {
            return Err(Error::ResourceInput);
        }
        let mut child = git_command(self.root)
            .args(args)
            .spawn()
            .map_err(|_error| Error::ResourceInput)?;
        self.capture(&mut child, cap).await
    }

    async fn capture(&self, child: &mut Child, cap: usize) -> Result<Vec<u8>, Error> {
        let deadline = Instant::now()
            .checked_add(self.command_timeout)
            .ok_or(Error::ResourceInput)?
            .min(self.deadline);
        let command_remaining = AtomicUsize::new(FILE_CAP + STDERR_CAP);
        let result = timeout_at(deadline, async {
            let stdout = child.stdout.take().ok_or(Error::ResourceInput)?;
            let stderr = child.stderr.take().ok_or(Error::ResourceInput)?;
            let (output, _diagnostic, status) = tokio::try_join!(
                read_bounded(stdout, cap, &command_remaining, &self.remaining),
                read_bounded(stderr, STDERR_CAP, &command_remaining, &self.remaining),
                async { child.wait().await.map_err(|_error| Error::ResourceInput) },
            )?;
            if !status.success() {
                return Err(Error::ResourceInput);
            }
            Ok(output)
        })
        .await
        .unwrap_or(Err(Error::ResourceInput));
        if result.is_err() {
            let _kill = child.start_kill();
            let _reap = timeout(Duration::from_secs(1), child.wait()).await;
        }
        result.map_err(|_error| Error::ResourceInput)
    }

    async fn resolve_commit(&self, revision: &str) -> Result<String, Error> {
        if revision.is_empty()
            || revision.len() > 128
            || revision.bytes().any(|byte| byte.is_ascii_control())
        {
            return Err(Error::ResourceInput);
        }
        let output = self
            .run(
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{revision}^{{commit}}"),
                ],
                128,
            )
            .await?;
        let resolved = from_utf8(&output)
            .map_err(|_error| Error::ResourceInput)?
            .strip_suffix('\n')
            .ok_or(Error::ResourceInput)?;
        if !valid_sha(resolved) {
            return Err(Error::ResourceInput);
        }
        Ok(resolved.to_owned())
    }

    async fn exact_commit(&self, sha: &str) -> Result<(), Error> {
        if !valid_sha(sha) || self.resolve_commit(sha).await? != sha {
            return Err(Error::ResourceInput);
        }
        Ok(())
    }

    async fn ancestor(&self, before: &str, after: &str) -> Result<(), Error> {
        self.run(&["merge-base", "--is-ancestor", before, after], 128)
            .await
            .map(|_output| ())
    }

    async fn diff(&self, revisions: &[&str]) -> Result<BTreeSet<String>, Error> {
        let mut args = vec![
            "diff",
            "--raw",
            "--no-abbrev",
            "-z",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=none",
        ];
        args.extend_from_slice(revisions);
        args.push("--");
        raw_paths(&self.run(&args, FILE_CAP).await?)
    }

    async fn blob(&self, revision: &str, path: &str) -> Result<Vec<u8>, Error> {
        let entry_bytes = self
            .run(
                &["ls-tree", "-z", "--full-tree", revision, "--", path],
                4096,
            )
            .await?;
        let entry = from_utf8(&entry_bytes)
            .map_err(|_error| Error::ResourceInput)?
            .strip_suffix('\0')
            .ok_or(Error::ResourceInput)?;
        let (metadata, actual_path) = entry.split_once('\t').ok_or(Error::ResourceInput)?;
        let mut words = metadata.split(' ');
        let mode = words.next().ok_or(Error::ResourceInput)?;
        let kind = words.next().ok_or(Error::ResourceInput)?;
        let oid = words.next().ok_or(Error::ResourceInput)?;
        if !matches!(mode, "100644" | "100755")
            || kind != "blob"
            || !valid_sha(oid)
            || words.next().is_some()
            || actual_path != path
        {
            return Err(Error::ResourceInput);
        }
        let size_bytes = self.run(&["cat-file", "-s", oid], 128).await?;
        let size: usize = from_utf8(&size_bytes)
            .map_err(|_error| Error::ResourceInput)?
            .trim()
            .parse()
            .map_err(|_error| Error::ResourceInput)?;
        if size > FILE_CAP {
            return Err(Error::ResourceInput);
        }
        let bytes = self.run(&["cat-file", "blob", oid], size).await?;
        if bytes.len() != size {
            return Err(Error::ResourceInput);
        }
        Ok(bytes)
    }
}

fn valid_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::var_os;
    use std::fs;
    use std::process::Command;
    use tempfile::{TempDir, tempdir};
    use tokio::process::Command as TokioCommand;

    struct Repository {
        directory: TempDir,
        base: String,
        review: String,
    }

    impl Repository {
        fn new() -> Self {
            let directory = tempdir().unwrap();
            let root = directory.path();
            command(root, &["init", "--quiet"]);
            fs::create_dir_all(root.join("src")).unwrap();
            fs::write(root.join("src/owner.rs"), "before\n").unwrap();
            fs::write(root.join("src/unrelated.rs"), "unchanged\n").unwrap();
            for path in ["Cargo.toml", "Cargo.lock", ".tool-versions"] {
                fs::copy(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../..")
                        .join(path),
                    root.join(path),
                )
                .unwrap();
            }
            command(root, &["add", "."]);
            command(root, &["commit", "--quiet", "-m", "code baseline"]);
            let base = command(root, &["rev-parse", "HEAD"]);
            for path in [AUDIT_PATH, PLAN_PATH, HUMAN_AUDIT_PATH] {
                let destination = root.join(path);
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::write(destination, "frozen review\n").unwrap();
            }
            command(root, &["add", "."]);
            command(root, &["commit", "--quiet", "-m", "records only"]);
            let review = command(root, &["rev-parse", "HEAD"]);
            Self {
                directory,
                base,
                review,
            }
        }

        fn root(&self) -> &Path {
            self.directory.path()
        }

        fn stage(&self, path: &str, bytes: &str) {
            let destination = self.root().join(path);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, bytes).unwrap();
            command(self.root(), &["add", "--", path]);
        }

        fn commit(&self) -> String {
            command(self.root(), &["commit", "--quiet", "-m", "implementation"]);
            command(self.root(), &["rev-parse", "HEAD"])
        }

        async fn check(
            &self,
            allowed: &[&str],
            selection: Selection<'_>,
        ) -> Result<ScopeReport, Error> {
            let files = allowed.iter().map(|path| (*path).to_owned()).collect();
            check_scope(self.root(), &self.base, &self.review, &files, selection).await
        }
    }

    fn command(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .env_clear()
            .env("PATH", var_os("PATH").unwrap())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "user.name=Scope Test",
                "-c",
                "user.email=scope@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[tokio::test]
    async fn staged_scope_includes_committed_and_index_changes_but_not_unstaged_user_edits() {
        let repository = Repository::new();
        repository.stage("src/owner.rs", "committed\n");
        repository.commit();
        repository.stage("src/new.rs", "staged\n");
        fs::write(repository.root().join("src/unrelated.rs"), "user work\n").unwrap();
        let allowed = ["src/owner.rs", "src/new.rs"];
        let report = repository.check(&allowed, Selection::Staged).await.unwrap();
        assert_eq!(report.changed_files, allowed.map(str::to_owned).into());
        assert_eq!(
            repository
                .check(&["src/new.rs"], Selection::Staged)
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
        assert_eq!(
            repository
                .check(&["src/owner.rs"], Selection::Staged)
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn committed_scope_checks_both_sides_of_renames_and_deletions() {
        let repository = Repository::new();
        command(repository.root(), &["mv", "src/owner.rs", "src/renamed.rs"]);
        command(repository.root(), &["rm", "src/unrelated.rs"]);
        let head = repository.commit();
        let allowed = ["src/owner.rs", "src/renamed.rs", "src/unrelated.rs"];
        assert_eq!(
            repository
                .check(&allowed, Selection::Head(&head))
                .await
                .unwrap()
                .changed_files,
            allowed.map(str::to_owned).into()
        );
        for missing in allowed {
            let reduced: Vec<_> = allowed
                .into_iter()
                .filter(|path| *path != missing)
                .collect();
            assert_eq!(
                repository
                    .check(&reduced, Selection::Head(&head))
                    .await
                    .unwrap_err(),
                Error::ResourceInput,
                "{missing}"
            );
        }
    }

    #[tokio::test]
    async fn frozen_records_cannot_change_even_if_listed_in_the_package() {
        for frozen in [AUDIT_PATH, PLAN_PATH] {
            let repository = Repository::new();
            repository.stage(frozen, "altered\n");
            assert_eq!(
                repository
                    .check(&[frozen], Selection::Staged)
                    .await
                    .unwrap_err(),
                Error::ResourceInput
            );
            let head = repository.commit();
            assert_eq!(
                repository
                    .check(&[frozen], Selection::Head(&head))
                    .await
                    .unwrap_err(),
                Error::ResourceInput
            );
        }
    }

    #[tokio::test]
    async fn baseline_to_review_accepts_only_the_two_records_and_human_audit() {
        let mut repository = Repository::new();
        repository.stage("src/owner.rs", "drift before review\n");
        repository.review = repository.commit();
        assert_eq!(
            repository
                .check(&["src/owner.rs"], Selection::Head("HEAD"))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn revisions_must_be_commits_with_both_ancestry_relations() {
        let mut repository = Repository::new();
        let review = repository.review.clone();
        assert_eq!(
            repository
                .check(&[], Selection::Head(&repository.base))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
        repository.base = review.clone();
        repository.review = "0".repeat(40);
        assert_eq!(
            repository
                .check(&[], Selection::Head("HEAD"))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
        repository.review = "HEAD".to_owned();
        assert_eq!(
            repository
                .check(&[], Selection::Head("HEAD"))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
        repository.review = review;
        repository.base = command(repository.root(), &["hash-object", "src/owner.rs"]);
        assert_eq!(
            repository
                .check(&[], Selection::Head("HEAD"))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn cancelled_index_changes_do_not_erase_a_committed_scope_violation() {
        let repository = Repository::new();
        repository.stage("src/unrelated.rs", "committed violation\n");
        repository.commit();
        repository.stage("src/unrelated.rs", "unchanged\n");
        assert_eq!(
            repository.check(&[], Selection::Staged).await.unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn a_conflicted_index_is_not_a_valid_regular_file_scope() {
        use std::process::Stdio;
        use tokio::io::AsyncWriteExt as _;
        let repository = Repository::new();
        let first = command(repository.root(), &["hash-object", "src/owner.rs"]);
        let ours = command(repository.root(), &["hash-object", "src/unrelated.rs"]);
        let theirs = command(repository.root(), &["hash-object", "Cargo.toml"]);
        let entries = format!(
            "0 {}\tsrc/owner.rs\n100644 {first} 1\tsrc/owner.rs\n100644 {ours} 2\tsrc/owner.rs\n100644 {theirs} 3\tsrc/owner.rs\n",
            "0".repeat(40)
        );
        let mut child = git_command(repository.root())
            .args(["update-index", "--index-info"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        input.write_all(entries.as_bytes()).await.unwrap();
        drop(input);
        assert!(child.wait_with_output().await.unwrap().status.success());
        assert!(!command(repository.root(), &["ls-files", "--unmerged"]).is_empty());
        assert_eq!(
            repository
                .check(&["src/owner.rs"], Selection::Staged)
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn unrelated_histories_cannot_supply_either_review_ancestry_edge() {
        let mut repository = Repository::new();
        command(
            repository.root(),
            &["checkout", "--quiet", "--orphan", "other-history"],
        );
        let other = repository.commit();
        assert_eq!(
            repository
                .check(&[], Selection::Head(&other))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
        repository.base = other;
        let review = repository.review.clone();
        assert_eq!(
            repository
                .check(&[], Selection::Head(&review))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn repository_settings_cannot_hide_changed_gitlinks_from_scope() {
        let mut repository = Repository::new();
        command(
            repository.root(),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{},vendor", repository.base),
            ],
        );
        repository.base = repository.commit();
        repository.stage(AUDIT_PATH, "next frozen audit\n");
        repository.stage(PLAN_PATH, "next frozen plan\n");
        repository.review = repository.commit();
        command(
            repository.root(),
            &["config", "diff.ignoreSubmodules", "all"],
        );
        command(
            repository.root(),
            &[
                "update-index",
                "--cacheinfo",
                &format!("160000,{},vendor", repository.review),
            ],
        );
        assert_eq!(
            repository.check(&[], Selection::Staged).await.unwrap_err(),
            Error::ResourceInput
        );
        let head = repository.commit();
        assert_eq!(
            repository
                .check(&[], Selection::Head(&head))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
        assert_eq!(
            repository
                .check(&["vendor"], Selection::Head(&head))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn ignored_gitlinks_cannot_hide_baseline_to_review_code_drift() {
        let mut repository = Repository::new();
        command(
            repository.root(),
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{},vendor", repository.base),
            ],
        );
        repository.base = repository.commit();
        repository.stage(AUDIT_PATH, "next frozen audit\n");
        repository.stage(PLAN_PATH, "next frozen plan\n");
        command(
            repository.root(),
            &["config", "diff.ignoreSubmodules", "all"],
        );
        command(
            repository.root(),
            &[
                "update-index",
                "--cacheinfo",
                &format!("160000,{},vendor", repository.review),
            ],
        );
        repository.review = repository.commit();
        let review = repository.review.clone();
        assert_eq!(
            repository
                .check(&[], Selection::Head(&review))
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn git_record_reads_use_frozen_blobs_and_enforce_exact_input_limits() {
        let mut repository = Repository::new();
        repository.stage(AUDIT_PATH, &" ".repeat(FILE_CAP));
        repository.review = repository.commit();
        fs::write(repository.root().join(AUDIT_PATH), "mutable worktree\n").unwrap();
        let git = Git::open(repository.root()).await.unwrap();
        assert_eq!(
            git.blob(&repository.review, AUDIT_PATH).await.unwrap(),
            vec![b' '; FILE_CAP]
        );
        assert_eq!(
            git.blob(&repository.review, "missing.toml")
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
        repository.stage(AUDIT_PATH, &" ".repeat(FILE_CAP + 1));
        let oversized = repository.commit();
        assert_eq!(
            git.blob(&oversized, AUDIT_PATH).await.unwrap_err(),
            Error::ResourceInput
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn frozen_record_paths_cannot_name_git_symlinks() {
        use std::os::unix::fs::symlink;
        let repository = Repository::new();
        fs::remove_file(repository.root().join(AUDIT_PATH)).unwrap();
        symlink("target.toml", repository.root().join(AUDIT_PATH)).unwrap();
        command(repository.root(), &["add", "--", AUDIT_PATH]);
        let head = repository.commit();
        let git = Git::open(repository.root()).await.unwrap();
        assert_eq!(
            git.blob(&head, AUDIT_PATH).await.unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn audit_identity_is_bound_to_baseline_lock_sdk_and_declared_compiler_independently() {
        let repository = Repository::new();
        let git = Git::open(repository.root()).await.unwrap();
        let build: super::super::resource_identity::BuildIdentity = serde_json::from_slice(
            include_bytes!(concat!(env!("OUT_DIR"), "/resource-identity.json")),
        )
        .unwrap();
        let lock = hex::encode(sha2::Sha256::digest(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Cargo.lock"
        ))));
        let manifest: toml::Value = toml::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../Cargo.toml"
        )))
        .unwrap();
        let sdk = manifest["workspace"]["dependencies"]["edgezero-core"]["rev"]
            .as_str()
            .unwrap();
        let compiler = build.compiler_verbose;
        check_baseline_identity(&git, &repository.base, &lock, sdk, &compiler)
            .await
            .unwrap();
        // Mutable copies cannot repair a claim about the frozen baseline.
        fs::write(repository.root().join("Cargo.lock"), "later lock\n").unwrap();
        check_baseline_identity(&git, &repository.base, &lock, sdk, &compiler)
            .await
            .unwrap();
        for (bad_lock, bad_sdk, bad_compiler) in [
            ("0".repeat(64), sdk.to_owned(), compiler.clone()),
            (lock.clone(), "0".repeat(40), compiler.clone()),
            (
                lock.clone(),
                sdk.to_owned(),
                "rustc 0.0.0 (unrelated compiler)\n".to_owned(),
            ),
            (lock.clone(), sdk.to_owned(), "not-rustc".to_owned()),
            (lock.clone(), sdk.to_owned(), "rustc".to_owned()),
        ] {
            assert_eq!(
                check_baseline_identity(&git, &repository.base, &bad_lock, &bad_sdk, &bad_compiler)
                    .await,
                Err(Error::ResourceInput)
            );
        }
        for versions in [
            "nodejs 24.20.0\n",
            "rust 1.98.1\nrust 1.98.1\n",
            "rust 1.98.1 extra\n",
            "rust\n",
        ] {
            repository.stage(".tool-versions", versions);
            let head = repository.commit();
            assert_eq!(
                check_baseline_identity(&git, &head, &lock, sdk, &compiler).await,
                Err(Error::ResourceInput),
                "{versions:?}"
            );
        }
    }

    #[tokio::test]
    async fn review_entrypoint_reads_both_frozen_records_and_preserves_the_scope_only_boundary() {
        let repository = Repository::new();
        let (audit, plan) = super::super::records::test_pair(&repository.base);
        repository.stage(AUDIT_PATH, from_utf8(&audit).unwrap());
        repository.stage(PLAN_PATH, from_utf8(&plan).unwrap());
        let review = repository.commit();
        let owned = "crates/auctioneer-runtime/src/budget.rs";
        repository.stage(owned, "synthetic implementation\n");
        let head = repository.commit();
        // An implementer's mutable records are never the approval source.
        fs::write(
            repository.root().join(AUDIT_PATH),
            "malformed mutable audit\n",
        )
        .unwrap();
        fs::write(
            repository.root().join(PLAN_PATH),
            "malformed mutable plan\n",
        )
        .unwrap();
        for selection in [Selection::Staged, Selection::Head(&head)] {
            let report = run(repository.root(), &review, selection).await.unwrap();
            assert_eq!(
                report.scope,
                "mechanical_scope_only_not_human_approval_or_fit"
            );
            assert_eq!(report.code_base_commit, repository.base);
            assert_eq!(report.review_commit, review);
            assert_eq!(report.checked_head, head);
            assert_eq!(report.changed_files, BTreeSet::from([owned.to_owned()]));
            assert_eq!(
                report.audit_sha256,
                Some(hex::encode(sha2::Sha256::digest(&audit)))
            );
            let output = serialize(&report).unwrap();
            assert!(output.ends_with(b"\n"));
            assert!(output.len() <= FILE_CAP);
        }
        repository.stage("src/unrelated.rs", "not approved\n");
        assert_eq!(
            run(repository.root(), &review, Selection::Staged)
                .await
                .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[tokio::test]
    async fn bounded_git_capture_limits_both_pipes_and_reaps_failed_children() {
        use std::env::{current_exe, var};
        use std::io::{Write as _, stderr, stdout};
        use std::process::Stdio;
        use std::thread::sleep;

        const MODE: &str = "AUCTIONEER__TEST__SCOPE_CAPTURE_PROBE";
        if let Ok(mode) = var(MODE) {
            match mode.as_str() {
                "stdout" => stdout()
                    .lock()
                    .write_all(&vec![b'x'; FILE_CAP + 4096])
                    .unwrap(),
                "stderr" => stderr()
                    .lock()
                    .write_all(&vec![b'x'; STDERR_CAP + 4096])
                    .unwrap(),
                "wait" => sleep(Duration::from_secs(30)),
                "exit" => panic!("intentional scope-capture probe failure"),
                "small" => stdout().lock().write_all(b"probe_work_occurred\n").unwrap(),
                _ => panic!("unknown scope-capture probe"),
            }
            return;
        }
        let repository = Repository::new();
        for (mode, timeout_kind, expected_success) in [
            ("small", None, true),
            ("exit", None, false),
            ("stdout", None, false),
            ("stderr", None, false),
            ("wait", Some("command"), false),
            ("wait", Some("operation"), false),
        ] {
            let mut git = Git::open(repository.root()).await.unwrap();
            if timeout_kind == Some("command") {
                git.command_timeout = Duration::from_millis(25);
            }
            if timeout_kind == Some("operation") {
                git.deadline = Instant::now() + Duration::from_millis(25);
            }
            let mut child = TokioCommand::new(current_exe().unwrap())
                .args(["--exact", "bidders::resources::review::tests::bounded_git_capture_limits_both_pipes_and_reaps_failed_children", "--nocapture"])
                .env(MODE, mode)
                .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
                .kill_on_drop(true).spawn().unwrap();
            let watchdog_result =
                timeout(Duration::from_secs(15), git.capture(&mut child, FILE_CAP)).await;
            assert!(
                watchdog_result.is_ok(),
                "capture must finish before watchdog: {mode}/{timeout_kind:?}"
            );
            let result = watchdog_result.unwrap();
            assert_eq!(result.is_ok(), expected_success, "{mode}/{timeout_kind:?}");
            assert!(
                child.try_wait().unwrap().is_some(),
                "completed or failed child must be reaped: {mode}"
            );
            if expected_success {
                assert!(
                    String::from_utf8(result.unwrap())
                        .unwrap()
                        .contains("probe_work_occurred")
                );
            }
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn git_capture_shares_one_operation_budget_across_successive_commands() {
        use std::process::Stdio;
        use std::sync::atomic::Ordering;
        let repository = Repository::new();
        let mut git = Git::open(repository.root()).await.unwrap();
        // Capture controlled bytes directly: macOS's Git launcher can emit an
        // additional sandbox warning, which must also consume the real budget.
        let payload = "x".repeat(41);
        let spawn = || {
            TokioCommand::new("/usr/bin/printf")
                .args(["%s", &payload])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .unwrap()
        };
        git.remaining = AtomicUsize::new(82);
        for remaining in [41, 0] {
            let bytes = git.capture(&mut spawn(), 41).await.unwrap();
            assert_eq!(bytes.len(), 41);
            assert_eq!(git.remaining.load(Ordering::Relaxed), remaining);
        }
        assert_eq!(
            git.capture(&mut spawn(), 41).await,
            Err(Error::ResourceInput)
        );
        let fresh_git = Git::open(repository.root()).await.unwrap();
        assert_eq!(
            fresh_git.capture(&mut spawn(), 40).await,
            Err(Error::ResourceInput)
        );
    }

    #[test]
    fn changed_paths_require_complete_nul_delimited_exact_names() {
        assert_eq!(
            paths(b"src/a.rs\0src/b.rs\0").unwrap(),
            BTreeSet::from(["src/a.rs".to_owned(), "src/b.rs".to_owned()])
        );
        for invalid in [
            &b"src/a.rs"[..],
            &b"\0"[..],
            &b"../escape\0"[..],
            &b"src/./a\0"[..],
            &b"src/*.rs\0"[..],
            &b"src/a\\b\0"[..],
            &b"\xff\0"[..],
        ] {
            assert_eq!(paths(invalid), Err(Error::ResourceInput), "{invalid:?}");
        }
    }

    #[test]
    fn raw_diff_rows_allow_only_complete_regular_file_changes() {
        let zero = "0".repeat(40);
        let old = "1".repeat(40);
        let new = "2".repeat(40);
        for (old_mode, new_mode, before, after, status) in [
            ("000000", "100644", &zero, &new, "A"),
            ("100644", "000000", &old, &zero, "D"),
            ("100644", "100755", &old, &new, "M"),
        ] {
            let bytes = format!(":{old_mode} {new_mode} {before} {after} {status}\0src/owner.rs\0");
            assert_eq!(
                raw_paths(bytes.as_bytes()).unwrap(),
                BTreeSet::from(["src/owner.rs".to_owned()])
            );
        }
        for bytes in [
            format!(":100644 100644 {old} {new} M\0"),
            format!(":100644 100644 {old} {new} M\0src/owner.rs"),
            format!(":100644 120000 {old} {new} T\0src/link\0"),
            format!(":160000 160000 {old} {new} M\0vendor\0"),
            format!(":000000 000000 {zero} {zero} U\0src/owner.rs\0"),
            format!(":000000 100644 {zero} {new} M\0src/owner.rs\0"),
            format!(":100644 000000 {old} {zero} M\0src/owner.rs\0"),
            format!(":100644 100644 short {new} M\0src/owner.rs\0"),
            format!(":100644 100644 {old} {new} R100\0src/old.rs\0src/new.rs\0"),
        ] {
            assert_eq!(
                raw_paths(bytes.as_bytes()),
                Err(Error::ResourceInput),
                "{bytes:?}"
            );
        }
    }
}

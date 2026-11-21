//! Offline Git data for the Task 8 catalog-growth reference.
//!
//! Only source provenance is described here. Historical GeneratorSource bytes
//! are not the generator executed by a measurement. Consumers must separately
//! freeze current measurement sources, dependencies, lock, options and ABI.
//! This module never builds, expands, materializes or activates a catalog.

use serde::Serialize;
use crate::schema_staging_build::MAX_INPUT_BYTES;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

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
    /// Deliberately unimplemented red-phase reader.
    Incomplete,
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
    pub fn materialize(&self, _destination: &Path) -> Result<(), Error> {
        Err(Error::Incomplete)
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

fn read_pinned(
    _root: &Path,
    _commit: &str,
    _tree: &str,
    _files: usize,
    _bytes: u64,
) -> Result<SourceSnapshot, Error> {
    let _ = (MAX_INPUT_BYTES, FIXED_INPUTS, TREE_ROOTS, parse_listing, included, git_command);
    Err(Error::Incomplete)
}

fn parse_listing(_bytes: &[u8]) -> Result<Listing, Error> {
    let empty = Listing { files: BTreeMap::new(), trees: BTreeSet::new() };
    drop((empty.files, empty.trees));
    Err(Error::Incomplete)
}

fn included(_path: &str) -> bool {
    false
}

fn git_command(_root: &Path) -> Command {
    Command::new("/usr/bin/git")
}

#[cfg(test)]
mod tests {
    use super::{Error, SourceSnapshot, included, parse_listing, read_pinned};
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Write as _;
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
                "--no-replace-objects", "--no-optional-locks", "--literal-pathspecs",
                "--no-pager", "-c", "core.fsmonitor=false", "-c",
                "core.hooksPath=/dev/null", "-c", "protocol.allow=never",
                "-c", "gc.auto=0", "-c", "maintenance.auto=false",
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
            .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("fixture Git is installed in CI");
        if let Some(bytes) = input {
            child.stdin.take().expect("fixture stdin")
                .write_all(bytes).expect("fixture input");
        }
        let output = child.wait_with_output().expect("fixture command completes");
        assert!(output.status.success(), "fixture Git failed: {}",
            String::from_utf8_lossy(&output.stderr));
        assert!(output.stdout.len() <= 4096, "fixture output is bounded metadata");
        output.stdout
    }

    fn line(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).expect("fixture metadata UTF-8")
            .strip_suffix('\n').expect("fixture metadata newline").to_owned()
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
                _directory: directory, root, files: BTreeMap::new(),
                commit: String::new(), tree: String::new(),
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
            git(&self.root, &["commit", "--quiet", "--no-gpg-sign", "-m", "fixture"], None);
            self.commit = line(git(&self.root, &["rev-parse", "HEAD"], None));
            self.tree = line(git(&self.root, &["rev-parse", "HEAD^{tree}"], None));
        }

        fn bytes(&self) -> u64 {
            self.files.values().map(|bytes| u64::try_from(bytes.len()).expect("fixture length"))
                .sum()
        }

        fn read(&self) -> Result<SourceSnapshot, Error> {
            read_pinned(&self.root, &self.commit, &self.tree, 20, self.bytes())
        }

        fn blob(&self, path: &str) -> String {
            line(git(&self.root, &["rev-parse", &format!("HEAD:{path}")], None))
        }

        fn object(&self, oid: &str) -> PathBuf {
            let (directory, filename) = oid.split_at(2);
            self.root.join(".git/objects").join(directory).join(filename)
        }

        fn corrupt_blob(&self, replacement: &[u8]) {
            let original = self.blob("Cargo.lock");
            let replacement = line(git(&self.root, &["hash-object", "-w", "--stdin"],
                Some(replacement)));
            assert_ne!(original, replacement);
            fs::copy(self.object(&replacement), self.object(&original))
                .expect("corrupt only disposable loose object");
        }
    }

    #[test]
    fn complete_pinned_data_has_exact_files_and_per_blob_witnesses() {
        let fixture = Fixture::new();
        let snapshot = fixture.read().expect("complete pinned source must be accepted");
        assert_eq!(snapshot.commit, fixture.commit);
        assert_eq!(snapshot.tree, fixture.tree);
        assert_eq!(snapshot.files.len(), 20);
        assert_eq!(snapshot.files.keys().collect::<Vec<_>>(),
            fixture.files.keys().collect::<Vec<_>>());
        for (path, expected) in &fixture.files {
            let file = &snapshot.files[path];
            assert_eq!(file.contents, *expected);
            assert_eq!(file.witness.path, *path);
            assert_eq!(file.witness.mode, "100644");
            assert_eq!(file.witness.blob, fixture.blob(path));
            assert_eq!(file.witness.bytes, u64::try_from(expected.len()).unwrap());
            assert_eq!(file.witness.sha256.len(), 64);
        }
        let schema = &snapshot.files[
            "crates/auctioneer-bidder/src/bidders/fake_fixture/parameters.json"];
        assert_eq!(schema.contents, serde_json::to_vec(&serde_json::json!({"type":"string"})).unwrap());
        assert_eq!(snapshot.files["Cargo.lock"].witness.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let reread = fixture.read().expect("reader must leave repository unchanged");
        assert_eq!(snapshot, reread);
    }

    #[test]
    fn a_complete_shallow_tip_does_not_require_its_parent_commit() {
        let mut fixture = Fixture::new();
        let parent = fixture.commit.clone();
        fs::write(fixture.root.join("outside-closure"), b"second tip").unwrap();
        git(&fixture.root, &["add", "outside-closure"], None);
        fixture.commit_index();
        fs::write(fixture.root.join(".git/shallow"), format!("{}\n", fixture.commit)).unwrap();
        fs::remove_file(fixture.object(&parent)).unwrap();
        assert_eq!(git(&fixture.root, &["rev-parse", "--is-shallow-repository"], None), b"true\n");
        let snapshot = fixture.read().expect("complete shallow object closure");
        assert_eq!(snapshot.files.len(), 20);
    }

    #[test]
    fn mutable_checkout_edits_are_not_reference_input() {
        let fixture = Fixture::new();
        let before = fixture.read().expect("initial source");
        fs::write(fixture.root.join("Cargo.lock"), b"uncommitted substitution").unwrap();
        fs::write(fixture.root.join(
            "crates/auctioneer-bidder/src/bidders/untracked.rs"), b"untracked").unwrap();
        assert_eq!(fixture.read().expect("Git objects remain unchanged"), before);
    }

    #[test]
    fn a_replacement_commit_cannot_redirect_pinned_source() {
        let fixture = Fixture::new();
        let other = Fixture::new();
        let replacement = line(git(&fixture.root, &["commit-tree", &fixture.tree, "-m",
            "different commit"], None));
        git(&fixture.root, &["update-ref", &format!("refs/replace/{}", fixture.commit),
            &replacement], None);
        assert_ne!(replacement, fixture.commit);
        assert_eq!(fixture.read().expect("replacement objects must be disabled"),
            other.read().expect("independent identical fixture source"));
    }

    #[test]
    fn absent_directory_refuses_instead_of_discovering_a_parent_repository() {
        let fixture = Fixture::new();
        assert_eq!(read_pinned(&fixture.root.join("absent"), &fixture.commit, &fixture.tree,
            20, fixture.bytes()), Err(Error::Root));
        let child = fixture.root.join("child");
        fs::create_dir(&child).unwrap();
        assert_eq!(read_pinned(&child, &fixture.commit, &fixture.tree,
            20, fixture.bytes()), Err(Error::Root));
    }

    #[test]
    fn head_and_tree_are_both_mandatory_pins() {
        let fixture = Fixture::new();
        assert_eq!(read_pinned(&fixture.root, &"0".repeat(40), &fixture.tree,
            20, fixture.bytes()), Err(Error::Identity));
        assert_eq!(read_pinned(&fixture.root, &fixture.commit, &"f".repeat(40),
            20, fixture.bytes()), Err(Error::Identity));
    }

    #[test]
    fn partial_promisor_and_filter_configuration_refuse_even_with_complete_objects() {
        for (key, value) in [
            ("extensions.partialclone", "origin"),
            ("remote.fixture.promisor", "true"),
            ("remote.fixture.partialclonefilter", "blob:none"),
        ] {
            let fixture = Fixture::new();
            git(&fixture.root, &["config", "core.repositoryformatversion", "1"], None);
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
        assert_eq!(read_pinned(&fixture.root, &fixture.commit, &fixture.tree,
            21, fixture.bytes()), Err(Error::Witness));
        assert_eq!(read_pinned(&fixture.root, &fixture.commit, &fixture.tree,
            20, fixture.bytes() + 1), Err(Error::Witness));
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
            git(&fixture.root, &["update-index", "--cacheinfo", mode, &oid, "Cargo.lock"], None);
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
            "crates/auctioneer-bidder/src/bidders/aps/tests/schema/inventory-exclusion.toml",
            "crates/auctioneer-bidder/src/bidders/mocktioneer/tests/schema/inventory-exclusion.toml",
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
        fs::write(fixture.root.join(".git/objects/info/alternates"),
            format!("{}\n", other.root.join(".git/objects").display())).unwrap();
        assert_eq!(fixture.read(), Err(Error::Partial));
    }

    #[test]
    fn file_and_relative_paths_are_not_repository_roots() {
        let fixture = Fixture::new();
        let file = fixture.root.join("Cargo.lock");
        for root in [Path::new("."), file.as_path()] {
            assert_eq!(read_pinned(root, &fixture.commit, &fixture.tree, 20, fixture.bytes()),
                Err(Error::Root));
        }
    }

    #[test]
    #[cfg(unix)]
    fn a_symlink_root_is_not_a_regular_snapshot_location() {
        let fixture = Fixture::new();
        let aliases = tempfile::tempdir().unwrap();
        let alias = aliases.path().join("reference");
        std::os::unix::fs::symlink(&fixture.root, &alias).unwrap();
        assert_eq!(read_pinned(&alias, &fixture.commit, &fixture.tree, 20, fixture.bytes()),
            Err(Error::Root));
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
        assert_eq!(parse_listing(format!("{record}\0").as_bytes()), Err(Error::Witness));
    }

    #[test]
    fn command_construction_disables_replacements_configuration_and_network_helpers() {
        use std::ffi::OsStr;

        let command = super::git_command(Path::new("/"));
        assert_eq!(command.get_program(), "/usr/bin/git");
        let arguments = command.get_args().map(|arg| arg.to_str().unwrap()).collect::<Vec<_>>();
        for required in [
            "--no-replace-objects", "--no-optional-locks", "--literal-pathspecs",
            "core.fsmonitor=false", "core.hooksPath=/dev/null", "protocol.allow=never",
            "gc.auto=0", "maintenance.auto=false",
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
            assert_eq!(environment.get(OsStr::new(key)), Some(&Some(OsStr::new(value))));
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
        snapshot.materialize(&destination).expect("captured bytes need no repository");
        assert_eq!(snapshot.commit(), commit);
        assert_eq!(snapshot.tree(), tree);
        assert_eq!(snapshot.files().len(), 20);
        assert_eq!(snapshot.witnesses().len(), 20);
        assert_eq!(snapshot.witnesses().map(|witness| &witness.path).collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>());
        for (path, bytes) in expected {
            let target = destination.join(path);
            assert_eq!(fs::read(&target).unwrap(), bytes);
            assert!(fs::symlink_metadata(&target).unwrap().is_file());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                assert_eq!(fs::metadata(target).unwrap().permissions().mode() & 0o777, 0o600);
            }
        }
    }

    #[test]
    fn materialization_never_merges_or_overwrites_an_existing_destination() {
        let fixture = Fixture::new();
        let snapshot = fixture.read().unwrap();
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().canonicalize().unwrap().join("source");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("sentinel"), b"keep").unwrap();
        assert_eq!(snapshot.materialize(&destination), Err(Error::Destination));
        assert_eq!(fs::read(destination.join("sentinel")).unwrap(), b"keep");
        assert!(!destination.join("Cargo.lock").exists());
    }

    #[test]
    fn materialization_preflights_changed_bytes_before_creating_output() {
        let fixture = Fixture::new();
        let mut snapshot = fixture.read().unwrap();
        snapshot.files.get_mut("Cargo.lock").unwrap().contents.push(b'x');
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
        fs::create_dir(&actual).unwrap();
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&actual, &alias).unwrap();
        assert_eq!(snapshot.materialize(&alias.join("source")), Err(Error::Destination));
        assert!(!actual.join("source").exists());
    }

}

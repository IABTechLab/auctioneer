//! Leased child target, separate from an invoking Cargo process's target lock.
//! The sampled logical-byte ceiling is not a physical disk quota.

use super::Refusal;
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Seek as _, Write as _};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use rustix::fs::OFlags;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

const NAME: &str = "bidder-evidence-conformance-v2";
const LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const ENTRIES: usize = 100_000;

pub(super) fn directory(managed_target: &Path) -> PathBuf {
    managed_target.join(NAME)
}

fn lease_file(root: &Path, mut options: OpenOptions) -> Result<File, Refusal> {
    if root.file_name().and_then(|name| name.to_str()) != Some(NAME) {
        return Err(Refusal::Storage);
    }
    let parent = root.parent().ok_or(Refusal::Storage)?;
    #[cfg(unix)]
    options.mode(0o600).custom_flags(
        (OFlags::NOFOLLOW | OFlags::NONBLOCK)
            .bits()
            .try_into()
            .map_err(|_error| Refusal::Storage)?,
    );
    let lease = options
        .open(parent.join("bidder-evidence-conformance-v2.lock"))
        .map_err(|_error| Refusal::Storage)?;
    let metadata = lease.metadata().map_err(|_error| Refusal::Storage)?;
    if !metadata.is_file() {
        return Err(Refusal::Storage);
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(Refusal::Storage);
    }
    Ok(lease)
}

pub(super) struct ReadLease {
    lease: File,
}

impl ReadLease {
    pub(super) fn acquire(root: &Path) -> Result<Self, Refusal> {
        let mut options = OpenOptions::new();
        options.read(true);
        let lease = lease_file(root, options)?;
        FileExt::try_lock_shared(&lease).map_err(|_error| Refusal::BuildBusy)?;
        Self { lease }.validate(root)
    }

    fn validate(self, root: &Path) -> Result<Self, Refusal> {
        require_empty_lease(&self.lease)?;
        if !fs::symlink_metadata(root)
            .map_err(|_error| Refusal::Storage)?
            .is_dir()
        {
            return Err(Refusal::Storage);
        }
        Ok(self)
    }
}

pub(super) struct Cache {
    root: PathBuf,
    lease: File,
}

impl Cache {
    pub(super) fn claim(root: &Path) -> Result<Self, Refusal> {
        let owned_root = root.to_owned();
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        let lease = lease_file(root, options)?;
        FileExt::try_lock_exclusive(&lease).map_err(|_error| Refusal::BuildBusy)?;
        Self {
            root: owned_root,
            lease,
        }
        .prepare()
    }

    fn prepare(self) -> Result<Self, Refusal> {
        require_empty_lease(&self.lease)?;
        match fs::symlink_metadata(&self.root) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_metadata) => return Err(Refusal::Storage),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                fs::create_dir_all(&self.root).map_err(|_error| Refusal::Storage)?;
            }
            Err(_error) => return Err(Refusal::Storage),
        }
        Ok(self)
    }

    pub(super) fn quarantine(&self) -> Result<(), Refusal> {
        // Establish nonempty refusal state before the marker write can fail.
        // The lease stays exclusively owned until this state is synced.
        self.lease.set_len(1).map_err(|_error| Refusal::Cleanup)?;
        let mut lease = &self.lease;
        lease.rewind().map_err(|_error| Refusal::Cleanup)?;
        lease.write_all(b"!").map_err(|_error| Refusal::Cleanup)?;
        lease.sync_all().map_err(|_error| Refusal::Cleanup)
    }

    pub(super) fn sample(&self) -> Result<(), Refusal> {
        logical_bytes(&self.root, LIMIT).map(|_bytes| ())
    }

    pub(super) fn discard(&self) -> Result<(), Refusal> {
        // Only our fixed, leased child tree is removed, after child cleanup.
        match fs::remove_dir_all(&self.root) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(_error) => Err(Refusal::Cleanup),
        }
    }
}

fn require_empty_lease(lease: &File) -> Result<(), Refusal> {
    if lease.metadata().map_err(|_error| Refusal::Storage)?.len() != 0 {
        return Err(Refusal::Storage);
    }
    Ok(())
}

// Releasing ownership must not wait for a descriptor duplicated by a concurrent
// fork to close on exec. Unproven cleanup quarantines the lease before release.
impl Drop for Cache {
    fn drop(&mut self) {
        let _unlock = FileExt::unlock(&self.lease);
    }
}

impl Drop for ReadLease {
    fn drop(&mut self) {
        let _unlock = FileExt::unlock(&self.lease);
    }
}

fn logical_bytes(root: &Path, limit: u64) -> Result<u64, Refusal> {
    let mut pending = vec![(root.to_owned(), 0_u8)];
    let mut entries = 0_usize;
    let mut total = 0_u64;
    while let Some((parent_path, depth)) = pending.pop() {
        if depth > 64 {
            return Err(Refusal::Storage);
        }
        let children = match fs::read_dir(parent_path) {
            Ok(children) => children,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_error) => return Err(Refusal::Storage),
        };
        for entry_result in children {
            let entry = entry_result.map_err(|_error| Refusal::Storage)?;
            entries = entries.checked_add(1).ok_or(Refusal::Storage)?;
            if entries > ENTRIES {
                return Err(Refusal::Storage);
            }
            let entry_path = entry.path();
            let metadata = match fs::symlink_metadata(&entry_path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == ErrorKind::NotFound => continue,
                Err(_error) => return Err(Refusal::Storage),
            };
            if metadata.is_dir() {
                pending.push((entry_path, depth.checked_add(1).ok_or(Refusal::Storage)?));
            } else if metadata.is_file() {
                total = total.checked_add(metadata.len()).ok_or(Refusal::Storage)?;
                if total > limit {
                    return Err(Refusal::Storage);
                }
            } else {
                return Err(Refusal::Storage);
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use tempfile::tempdir_in;

    #[cfg(unix)]
    #[test]
    fn source_conformance_fifo_lease_is_refused_without_waiting_for_a_writer() {
        use std::env::{current_exe, var_os};
        use std::io::{Read as _, Write as _, stdout};
        use std::process::{Command, Stdio};
        use std::thread::sleep;
        use std::time::{Duration, Instant};

        const CHILD_ROOT: &str = "AUCTIONEER__TEST__EVIDENCE_FIFO_CHILD";
        if let Some(root) = var_os(CHILD_ROOT) {
            assert!(matches!(
                ReadLease::acquire(Path::new(&root)),
                Err(Refusal::Storage)
            ));
            stdout()
                .lock()
                .write_all(b"\nevidence_fifo_refused\n")
                .unwrap();
            return;
        }
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory(temporary.path());
        fs::create_dir_all(&root).unwrap();
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(root.with_extension("lock"))
                .status()
                .unwrap()
                .success()
        );
        let mut child = Command::new(current_exe().unwrap())
            .args([
                "--exact",
                "bidders::evidence::conformance::scratch::tests::source_conformance_fifo_lease_is_refused_without_waiting_for_a_writer",
                "--nocapture",
            ])
            .env(CHILD_ROOT, &root)
            .stdout(Stdio::piped())
            .spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "FIFO refusal child failed: {status}");
                let mut output = String::new();
                child
                    .stdout
                    .take()
                    .unwrap()
                    .read_to_string(&mut output)
                    .unwrap();
                assert!(
                    output.lines().any(|line| line == "evidence_fifo_refused"),
                    "{output}"
                );
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("opening the evidence lease blocked on a FIFO");
            }
            sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn source_conformance_read_lease_never_creates_missing_storage() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory(temporary.path());
        assert!(matches!(ReadLease::acquire(&root), Err(Refusal::Storage)));
        assert!(!root.exists());
        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
        fs::create_dir_all(&root).unwrap();
        assert!(matches!(ReadLease::acquire(&root), Err(Refusal::Storage)));
        assert!(!root.with_extension("lock").exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 1);
    }

    #[test]
    fn lease_ownership_ends_even_when_a_descriptor_is_inherited() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory(temporary.path());
        let producer = Cache::claim(&root).unwrap();
        // dup models the same open-file description inherited by a concurrent
        // fork before exec closes it, without a scheduler-dependent child.
        let inherited_producer = producer.lease.try_clone().unwrap();
        assert!(matches!(ReadLease::acquire(&root), Err(Refusal::BuildBusy)));
        drop(producer);
        let reader = ReadLease::acquire(&root).expect("producer ownership ended");
        let inherited_reader = reader.lease.try_clone().unwrap();
        assert!(matches!(Cache::claim(&root), Err(Refusal::BuildBusy)));
        drop(reader);
        let next = Cache::claim(&root).expect("reader ownership ended");
        next.sample().unwrap();
        drop(inherited_reader);
        drop(inherited_producer);
    }

    #[test]
    fn constructor_validation_errors_unlock_even_with_inherited_descriptors() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory(temporary.path());
        fs::write(&root, b"not a directory").unwrap();
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        let producer_file = lease_file(&root, options).unwrap();
        FileExt::try_lock_exclusive(&producer_file).unwrap();
        let producer = Cache {
            root: root.clone(),
            lease: producer_file,
        };
        let inherited_producer = producer.lease.try_clone().unwrap();
        assert!(matches!(producer.prepare(), Err(Refusal::Storage)));

        let mut reader_options = OpenOptions::new();
        reader_options.read(true);
        let reader_file = lease_file(&root, reader_options).unwrap();
        FileExt::try_lock_shared(&reader_file).expect("failed producer constructor unlocked");
        let reader = ReadLease { lease: reader_file };
        let inherited_reader = reader.lease.try_clone().unwrap();
        assert!(matches!(reader.validate(&root), Err(Refusal::Storage)));

        fs::remove_file(&root).unwrap();
        let next = Cache::claim(&root).expect("failed reader constructor unlocked");
        assert_eq!(fs::read(root.with_extension("lock")).unwrap(), b"");
        next.sample().unwrap();
        drop(inherited_reader);
        drop(inherited_producer);
    }

    #[test]
    fn quarantined_leases_refuse_readers_and_writers_without_touching_cache_files() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory(temporary.path());
        let cache = Cache::claim(&root).unwrap();
        let artifact = root.join("retained-artifact");
        fs::write(&artifact, b"preserved").unwrap();
        cache.quarantine().unwrap();
        assert_eq!(cache.lease.metadata().unwrap().len(), 1);
        drop(cache);
        for _attempt in 0_usize..2 {
            assert!(matches!(Cache::claim(&root), Err(Refusal::Storage)));
            assert!(matches!(ReadLease::acquire(&root), Err(Refusal::Storage)));
            assert_eq!(fs::read(&artifact).unwrap(), b"preserved");
            assert_eq!(fs::read(root.with_extension("lock")).unwrap(), b"!");
        }
        fs::remove_file(&artifact).unwrap();
        fs::remove_dir(&root).unwrap();
        assert!(matches!(Cache::claim(&root), Err(Refusal::Storage)));
        assert!(!root.exists(), "quarantine cannot recreate a removed cache");
    }

    #[test]
    fn source_conformance_read_leases_share_and_exclude_producers_without_deleting_artifacts() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = directory(temporary.path());
        let initial = Cache::claim(&root).unwrap();
        let artifact = root.join("retained-artifact");
        fs::write(&artifact, b"unchanged").unwrap();
        drop(initial);
        let first = ReadLease::acquire(&root).unwrap();
        let second = ReadLease::acquire(&root).unwrap();
        assert!(matches!(Cache::claim(&root), Err(Refusal::BuildBusy)));
        assert_eq!(fs::read(&artifact).unwrap(), b"unchanged");
        drop(first);
        assert!(matches!(Cache::claim(&root), Err(Refusal::BuildBusy)));
        drop(second);
        let owner = Cache::claim(&root).unwrap();
        assert!(matches!(ReadLease::acquire(&root), Err(Refusal::BuildBusy)));
        assert_eq!(fs::read(artifact).unwrap(), b"unchanged");
        owner.discard().unwrap();
        assert!(!root.exists());
    }

    #[test]
    fn source_conformance_scratch_is_isolated_exclusive_and_cleans_only_its_child() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let parent = temporary.path();
        fs::write(parent.join("parent-artifact"), b"parent").unwrap();
        let root = directory(parent);
        assert_ne!(root, parent);
        let cache = Cache::claim(&root).unwrap();
        assert!(matches!(Cache::claim(&root), Err(Refusal::BuildBusy)));
        fs::write(root.join("child-artifact"), b"child").unwrap();
        cache.sample().unwrap();
        cache.discard().unwrap();
        assert!(!root.exists());
        assert_eq!(fs::read(parent.join("parent-artifact")).unwrap(), b"parent");
        drop(cache);
        Cache::claim(&root).unwrap().sample().unwrap();
    }

    #[test]
    fn source_conformance_scratch_counts_logical_bytes_and_refuses_oversize_before_reading() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = temporary.path();
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("first"), b"four").unwrap();
        fs::write(root.join("nested/second"), b"more").unwrap();
        assert_eq!(logical_bytes(root, 8).unwrap(), 8);
        assert_eq!(logical_bytes(root, 7), Err(Refusal::Storage));
        assert_eq!(LIMIT, 4 * 1024 * 1024 * 1024);
    }

    #[cfg(unix)]
    #[test]
    fn source_conformance_scratch_refuses_symlinked_storage() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let cache = Cache::claim(&directory(temporary.path())).unwrap();
        symlink(temporary.path(), cache.root.join("escape")).unwrap();
        assert_eq!(cache.sample(), Err(Refusal::Storage));
        cache.discard().unwrap();
        drop(cache);
        symlink(temporary.path(), directory(temporary.path())).unwrap();
        assert!(matches!(
            Cache::claim(&directory(temporary.path())),
            Err(Refusal::Storage)
        ));
    }
}

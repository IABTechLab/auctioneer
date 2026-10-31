use super::artifacts::{ReleaseMetadata, release_path};
use super::preparation::{FILE_CAP, INPUT_CAP};
use crate::qualification::harness::{HarnessError, Result, process::Supervisor};
use flate2::read::MultiGzDecoder;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Take};
use std::path::Path;
use std::str::from_utf8;

const MEMBER_CAP: usize = 128;
const PAYLOAD_CAP: usize = 2 * FILE_CAP + 3 * INPUT_CAP;
// Payload plus at most one bounded extension per raw entry, tar block padding
// and a conventional 20-block trailer. The limit covers the complete gzip stream.
const EXPANDED_CAP: usize = PAYLOAD_CAP + MEMBER_CAP * INPUT_CAP + (MEMBER_CAP + 20) * 512;

#[derive(Default)]
struct RawBudget {
    members: usize,
    files: usize,
    payload: usize,
}

impl RawBudget {
    fn admit(&mut self, kind: tar::EntryType, size: u64) -> Result<()> {
        let bytes = usize::try_from(size).map_err(|_error| invalid())?;
        self.members = self.members.checked_add(1).ok_or_else(invalid)?;
        if self.members > MEMBER_CAP {
            return Err(invalid());
        }
        if kind.is_file() {
            self.files = self.files.checked_add(1).ok_or_else(invalid)?;
            self.payload = self.payload.checked_add(bytes).ok_or_else(invalid)?;
            if self.files > 5 || bytes > FILE_CAP || self.payload > PAYLOAD_CAP {
                return Err(invalid());
            }
        } else if kind.is_dir() {
            if size != 0 {
                return Err(invalid());
            }
        } else if kind.is_gnu_longname() || kind.is_pax_local_extensions() {
            if bytes > INPUT_CAP {
                return Err(invalid());
            }
        } else {
            return Err(invalid());
        }
        Ok(())
    }
}

const fn invalid() -> HarnessError {
    HarnessError::new("release_invalid")
}

fn reader(bytes: &[u8]) -> Result<Take<MultiGzDecoder<&[u8]>>> {
    let cap = u64::try_from(EXPANDED_CAP).map_err(|_error| invalid())?;
    Ok(MultiGzDecoder::new(bytes).take(cap.checked_add(1).ok_or_else(invalid)?))
}

async fn drain<R: Read>(input: &mut R, supervisor: &Supervisor, zeros: bool) -> Result<()> {
    let mut buffer = [0_u8; 8192];
    loop {
        supervisor.check_active().await?;
        let count = input.read(&mut buffer).map_err(|_error| invalid())?;
        if count == 0 {
            return Ok(());
        }
        if zeros
            && buffer
                .get(..count)
                .ok_or_else(invalid)?
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err(invalid());
        }
    }
}

fn pax_keys(bytes: &[u8]) -> Result<()> {
    // The library's PAX iterator is line-based. Accept only complete single-line
    // records and forbid blank lines that would hide an unvisited suffix.
    let records = bytes.strip_suffix(b"\n").ok_or_else(invalid)?;
    if records.split(|byte| *byte == b'\n').any(<[u8]>::is_empty) {
        return Err(invalid());
    }
    let mut seen = BTreeSet::new();
    for item in tar::PaxExtensions::new(bytes) {
        let extension = item.map_err(|_error| invalid())?;
        let key = extension.key().map_err(|_error| invalid())?;
        if !matches!(
            key,
            "uid" | "gid" | "uname" | "gname" | "mtime" | "atime" | "ctime" | "comment"
        ) || !seen.insert(key.to_owned())
        {
            return Err(invalid());
        }
    }
    Ok(())
}

// Bound raw extension records before tar may buffer GNU/PAX names. The second
// pass checks bounded GNU names. PAX cannot override paths/sizes: tar parsers
// disagree about their precedence and the resulting record boundaries.
// This pass owns work upper bounds; ReleaseMetadata::validate owns the exact
// required member set after the semantic pass, not a second raw file count.
async fn raw_bounds(bytes: &[u8], supervisor: &Supervisor) -> Result<()> {
    let mut archive = tar::Archive::new(reader(bytes)?);
    let mut budget = RawBudget::default();
    for item in archive.entries().map_err(|_error| invalid())?.raw(true) {
        supervisor.check_active().await?;
        let mut entry = item.map_err(|_error| invalid())?;
        let kind = entry.header().entry_type();
        budget.admit(kind, entry.size())?;
        if kind.is_pax_local_extensions() {
            let mut data = Vec::new();
            entry.read_to_end(&mut data).map_err(|_error| invalid())?;
            pax_keys(&data)?;
        }
        drain(&mut entry, supervisor, false).await?;
    }
    let mut remaining = archive.into_inner();
    // Validate gzip checksums and reject hidden archives after the tar terminator.
    drain(&mut remaining, supervisor, true).await?;
    if remaining.limit() == 0 {
        return Err(invalid());
    }
    Ok(())
}

pub(super) async fn preflight(
    bytes: &[u8],
    source_revision: &str,
    supervisor: &Supervisor,
) -> Result<()> {
    raw_bounds(bytes, supervisor).await?;
    let mut archive = tar::Archive::new(reader(bytes)?);
    let mut files = BTreeMap::new();
    let mut parsed_metadata = None;
    for item in archive.entries().map_err(|_error| invalid())? {
        supervisor.check_active().await?;
        let mut entry = item.map_err(|_error| invalid())?;
        let kind = entry.header().entry_type();
        let path = entry.path_bytes();
        let encoded_name = from_utf8(&path).map_err(|_error| invalid())?;
        let name = if kind.is_dir() {
            encoded_name.strip_suffix('/').unwrap_or(encoded_name)
        } else {
            encoded_name
        };
        release_path(Path::new("/release"), name)?;
        let size = usize::try_from(entry.size()).map_err(|_error| invalid())?;
        if kind.is_dir() {
            if size != 0 {
                return Err(invalid());
            }
            continue;
        }
        if !kind.is_file() || size > FILE_CAP {
            return Err(invalid());
        }
        if files.insert(name.to_owned(), size).is_some() {
            return Err(invalid());
        }
        if name == "release.json" {
            if size > INPUT_CAP {
                return Err(invalid());
            }
            let mut data = Vec::new();
            entry.read_to_end(&mut data).map_err(|_error| invalid())?;
            parsed_metadata =
                Some(serde_json::from_slice::<ReleaseMetadata>(&data).map_err(|_error| invalid())?);
        }
        drain(&mut entry, supervisor, false).await?;
    }
    let metadata = parsed_metadata.ok_or_else(invalid)?;
    metadata.validate(source_revision, &files.keys().cloned().collect())?;
    for member in [&metadata.manifests.edgezero, &metadata.manifests.adapter] {
        if files.get(&member.path).is_none_or(|size| *size > INPUT_CAP) {
            return Err(invalid());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use serde_json::json;
    use std::io::{Write as _, copy, repeat, sink};

    fn members() -> Vec<(&'static str, Vec<u8>)> {
        let member = |path: &str| json!({"path":path,"sha256":"a".repeat(64)});
        vec![
            (
                "release.json",
                serde_json::to_vec(&json!({
                    "format":1,"lifecycle_protocol":1,"source_revision":"a".repeat(40),
                    "adapter":"fastly","app_cli":member("auctioneer-cli"),
                    "package":member("package.tar.gz"),
                    "manifests":{"edgezero":member("edgezero.toml"),"adapter":member("fastly.toml")}
                }))
                .unwrap(),
            ),
            ("auctioneer-cli", b"cli".to_vec()),
            ("package.tar.gz", b"package".to_vec()),
            ("edgezero.toml", b"name = 'test'".to_vec()),
            ("fastly.toml", b"name = 'test'".to_vec()),
        ]
    }

    fn tar_bytes(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut archive = tar::Builder::new(Vec::new());
        for (name, bytes) in members {
            let mut header = tar::Header::new_ustar();
            header.set_size(u64::try_from(bytes.len()).unwrap());
            header.set_mode(0o600);
            header.set_cksum();
            archive
                .append_data(&mut header, name, bytes.as_slice())
                .unwrap();
        }
        archive.into_inner().unwrap()
    }

    fn compressed(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[tokio::test]
    async fn archive_terminators_cannot_hide_a_sixth_member_in_either_gzip_layout() {
        let supervisor = Supervisor::new();
        let source = "a".repeat(40);
        let mut plain = tar_bytes(&members());
        assert_eq!(
            preflight(&compressed(&plain), &source, &supervisor).await,
            Ok(())
        );
        plain.extend_from_slice(&[0_u8; 1024]);
        assert_eq!(
            preflight(&compressed(&plain), &source, &supervisor).await,
            Ok(())
        );

        let hidden = tar_bytes(&[("hidden-sixth-member", b"hidden".to_vec())]);
        let mut single_stream = plain.clone();
        single_stream.extend_from_slice(&hidden);
        let mut multiple_streams = compressed(&plain);
        multiple_streams.extend_from_slice(&compressed(&hidden));
        for bytes in [compressed(&single_stream), multiple_streams] {
            assert_eq!(raw_bounds(&bytes, &supervisor).await, Err(invalid()));
            assert_eq!(
                preflight(&bytes, &source, &supervisor).await,
                Err(invalid())
            );
        }
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn raw_archive_file_and_entry_caps_bind_without_semantic_backstops() {
        let supervisor = Supervisor::new();
        let complete = members();
        assert_eq!(
            raw_bounds(&compressed(&tar_bytes(&complete)), &supervisor).await,
            Ok(())
        );
        let mut extra = complete.clone();
        extra.push(("extra", Vec::new()));
        assert_eq!(
            raw_bounds(&compressed(&tar_bytes(&extra)), &supervisor).await,
            Err(invalid())
        );

        for directories in [122_usize, 123, 124] {
            let mut archive = tar::Builder::new(Vec::new());
            for index in 0..directories {
                let mut header = tar::Header::new_ustar();
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
                header.set_mode(0o700);
                header.set_cksum();
                archive
                    .append_data(&mut header, format!("directory-{index}"), [].as_slice())
                    .unwrap();
            }
            for (name, bytes) in &complete {
                let mut header = tar::Header::new_ustar();
                header.set_size(u64::try_from(bytes.len()).unwrap());
                header.set_mode(0o600);
                header.set_cksum();
                archive
                    .append_data(&mut header, name, bytes.as_slice())
                    .unwrap();
            }
            let bytes = compressed(&archive.into_inner().unwrap());
            assert_eq!(
                raw_bounds(&bytes, &supervisor).await,
                if directories <= 123 {
                    Ok(())
                } else {
                    Err(invalid())
                },
                "{directories} directories plus five release files"
            );
        }
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn archive_preflight_owns_each_release_identity_term_without_sdk_validation() {
        let supervisor = Supervisor::new();
        let source = "a".repeat(40);
        for (field, value) in [
            ("format", json!(2_u8)),
            ("lifecycle_protocol", json!(2_u8)),
            ("adapter", json!("spin")),
            ("source_revision", json!("b".repeat(40))),
        ] {
            let mut records = members();
            let mut metadata: serde_json::Value = serde_json::from_slice(&records[0].1).unwrap();
            metadata[field] = value;
            records[0].1 = serde_json::to_vec(&metadata).unwrap();
            let bytes = compressed(&tar_bytes(&records));
            assert_eq!(
                preflight(&bytes, &source, &supervisor).await,
                Err(invalid()),
                "{field}"
            );
        }
        assert_eq!(
            preflight(&compressed(&tar_bytes(&members())), &source, &supervisor).await,
            Ok(())
        );
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn archive_preflight_requires_the_complete_declared_member_set() {
        let supervisor = Supervisor::new();
        let source = "a".repeat(40);
        for index in 0..5 {
            let mut missing = members();
            missing.remove(index);
            assert_eq!(
                preflight(&compressed(&tar_bytes(&missing)), &source, &supervisor).await,
                Err(invalid())
            );
            let mut replaced = members();
            replaced[index].0 = "undeclared";
            assert_eq!(
                preflight(&compressed(&tar_bytes(&replaced)), &source, &supervisor).await,
                Err(invalid())
            );
        }
        assert_eq!(
            preflight(&compressed(&tar_bytes(&members())), &source, &supervisor).await,
            Ok(())
        );
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn pax_metadata_requires_complete_single_line_records() {
        for bytes in [
            b"".as_slice(),
            b"\n".as_slice(),
            b"\n8 uid=1\n".as_slice(),
            b"8 uid=1\n\n".as_slice(),
            b"8 uid=1".as_slice(),
        ] {
            assert_eq!(pax_keys(bytes), Err(invalid()));
        }
        assert_eq!(pax_keys(b"8 uid=1\n"), Ok(()));
        assert_eq!(pax_keys(b"22 comment=line\nextra\n"), Err(invalid()));
    }

    #[test]
    fn raw_budget_caps_file_and_extension_sizes_independently() {
        for kind in [
            tar::EntryType::Regular,
            tar::EntryType::GNULongName,
            tar::EntryType::XHeader,
        ] {
            let cap = if kind.is_file() {
                0x0800_0000
            } else {
                0x0001_0000
            };
            for size in [cap - 1, cap, cap + 1] {
                let mut budget = RawBudget::default();
                assert_eq!(
                    budget.admit(kind, size),
                    if size <= cap { Ok(()) } else { Err(invalid()) }
                );
            }
        }
    }

    #[test]
    fn raw_budget_caps_aggregate_payload_independent_of_member_sizes() {
        for final_size in [0xFFFF, 0x0001_0000, 0x0001_0001] {
            let mut budget = RawBudget::default();
            for size in [0x0800_0000, 0x0800_0000, 0x0001_0000, 0x0001_0000] {
                budget.admit(tar::EntryType::Regular, size).unwrap();
            }
            assert_eq!(
                budget.admit(tar::EntryType::Regular, final_size),
                if final_size <= 0x0001_0000 {
                    Ok(())
                } else {
                    Err(invalid())
                }
            );
        }
    }

    #[test]
    fn raw_budget_caps_entry_and_regular_file_counts_independently() {
        let mut directories = RawBudget::default();
        for _ in 0_u16..128 {
            directories.admit(tar::EntryType::Directory, 0).unwrap();
        }
        assert_eq!(
            directories.admit(tar::EntryType::Directory, 0),
            Err(invalid())
        );
        let mut files = RawBudget::default();
        for _ in 0_u8..5 {
            files.admit(tar::EntryType::Regular, 0).unwrap();
        }
        assert_eq!(files.admit(tar::EntryType::Regular, 0), Err(invalid()));
    }

    #[test]
    fn raw_budget_refuses_links_sparse_global_and_special_files() {
        for kind in [
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::GNUSparse,
            tar::EntryType::XGlobalHeader,
            tar::EntryType::Fifo,
            tar::EntryType::Char,
            tar::EntryType::Block,
        ] {
            assert_eq!(RawBudget::default().admit(kind, 0), Err(invalid()));
        }
        assert_eq!(
            RawBudget::default().admit(tar::EntryType::Directory, 1),
            Err(invalid())
        );
    }

    #[tokio::test]
    async fn gzip_reader_caps_expansion_even_when_the_trailer_is_all_zero() {
        let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
        copy(&mut repeat(0).take(277_096_450), &mut gzip).unwrap();
        let bytes = gzip.finish().unwrap();
        let mut input = reader(&bytes).unwrap();
        let supervisor = Supervisor::new();
        let read = copy(&mut input, &mut sink()).unwrap();
        assert_eq!(read, 277_096_449);
        assert_eq!(input.limit(), 0);
        // A zero-only archive passes the raw resource stage, so metadata or
        // minimum-member checks cannot mask its expanded-stream refusal.
        assert_eq!(
            raw_bounds(&compressed(&[0_u8; 1024]), &supervisor).await,
            Ok(())
        );
        assert_eq!(raw_bounds(&bytes, &supervisor).await, Err(invalid()));
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn archive_draining_observes_cancellation_before_reading() {
        let supervisor = Supervisor::new();
        supervisor.interrupt(130);
        let mut bytes = b"not read".as_slice();
        assert_eq!(
            drain(&mut bytes, &supervisor, false).await,
            Err(HarnessError::new("interrupted"))
        );
        assert_eq!(bytes, b"not read");
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn pax_metadata_cannot_add_structural_sparse_or_duplicate_keys() {
        fn encoded(entries: &[(&str, &[u8])]) -> Vec<u8> {
            let mut builder = tar::Builder::new(Vec::new());
            builder
                .append_pax_extensions(entries.iter().copied())
                .unwrap();
            let bytes = builder.into_inner().unwrap();
            let mut archive = tar::Archive::new(bytes.as_slice());
            let mut entry = archive
                .entries()
                .unwrap()
                .raw(true)
                .next()
                .unwrap()
                .unwrap();
            let mut data = Vec::new();
            entry.read_to_end(&mut data).unwrap();
            data
        }
        assert_eq!(
            pax_keys(&encoded(&[("mtime", b"0"), ("uid", b"1")])),
            Ok(())
        );
        for entries in [
            vec![("GNU.sparse.size", b"1".as_slice())],
            vec![("path", b"release.json".as_slice())],
            vec![("size", b"0".as_slice())],
            vec![("uid", b"1".as_slice()), ("uid", b"2".as_slice())],
            vec![("size", b"134217729".as_slice())],
            vec![("size", b"-1".as_slice())],
            vec![("size", b"1e9".as_slice())],
        ] {
            assert_eq!(pax_keys(&encoded(&entries)), Err(invalid()));
        }
    }
}

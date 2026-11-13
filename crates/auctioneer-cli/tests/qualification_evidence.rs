//! Retained evidence integrity, not a new qualification or historical reevaluation.

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::str::from_utf8;

    struct Capture {
        directory: &'static str,
        profile: &'static str,
        extra: Option<&'static str>,
    }

    const CAPTURES: &[Capture] = &[
        Capture {
            directory: "native-axum-7106af6-failed",
            profile: "native-axum-local-7106af6.json",
            extra: None,
        },
        Capture {
            directory: "native-axum-4bcc666-failed",
            profile: "native-axum-local-4bcc666.json",
            extra: None,
        },
        Capture {
            directory: "native-axum-35ef524-failed",
            profile: "native-axum-local-35ef524.json",
            extra: None,
        },
        Capture {
            directory: "native-axum-82075f1-local-pass",
            profile: "native-axum-local-82075f1.json",
            extra: None,
        },
        Capture {
            directory: "native-axum-e0a540f-local-pass",
            profile: "native-axum-local-e0a540f.json",
            extra: None,
        },
        Capture {
            directory: "native-axum-rust-aefcf47-local-pass",
            profile: "native-axum-rust-aefcf47.json",
            extra: Some("frozen-probe-diagnostics.txt"),
        },
        Capture {
            directory: "native-axum-rust-d68ba17-local-pass",
            profile: "native-axum-rust-d68ba17.json",
            extra: Some("frozen-probe.json"),
        },
    ];
    const TABLE_HEADER: &str = "| File | SHA-256 |\n| --- | --- |\n";
    const PAYLOADS: &[&str] = &["report.json", "observations.json", "runner.json"];
    type Files = BTreeMap<String, Vec<u8>>;

    fn evidence_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/testing/evidence")
    }

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn exact_members(actual: &BTreeSet<&str>, expected: &[&str]) -> Result<(), String> {
        if *actual != expected.iter().copied().collect() {
            return Err(format!("expected {expected:?}, found {actual:?}"));
        }
        Ok(())
    }

    fn read_regular(path: &Path) -> Result<Vec<u8>, String> {
        let metadata =
            fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
        if !metadata.is_file() {
            return Err(format!("{} is not a regular file", path.display()));
        }
        fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn read_capture(path: &Path) -> Result<Files, String> {
        let mut files = BTreeMap::new();
        for result in fs::read_dir(path).map_err(|error| error.to_string())? {
            let entry = result.map_err(|error| error.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|name| format!("invalid evidence filename: {name:?}"))?;
            files.insert(name, read_regular(&entry.path())?);
        }
        Ok(files)
    }

    fn parse_index(text: &str, manifest: bool) -> Result<BTreeMap<&str, &str>, String> {
        let body = if manifest {
            text
        } else {
            text.split_once(TABLE_HEADER)
                .ok_or_else(|| "missing evidence digest table".to_owned())?
                .1
        };
        let mut entries = BTreeMap::new();
        for line in body.lines() {
            let (name, hash) = if manifest {
                let (hash, name) = line
                    .split_once("  ")
                    .ok_or_else(|| format!("malformed checksum row: {line:?}"))?;
                (name, hash)
            } else {
                line.strip_prefix("| ")
                    .and_then(|prefix_stripped| prefix_stripped.strip_suffix(" |"))
                    .and_then(|cell_text| cell_text.split_once(" | "))
                    .ok_or_else(|| format!("malformed digest table row: {line:?}"))?
            };
            if name.is_empty()
                || matches!(name, "." | "..")
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
                || hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                || entries.insert(name, hash).is_some()
            {
                return Err(format!("invalid or duplicate digest entry: {name:?}"));
            }
        }
        if entries.is_empty() {
            return Err("empty evidence digest index".to_owned());
        }
        Ok(entries)
    }

    fn index_name(capture: &Capture) -> &'static str {
        capture.extra.map_or("README.md", |_extra| "SHA256SUMS")
    }

    fn verify_capture(capture: &Capture, files: &Files) -> Result<usize, String> {
        let mut payloads = PAYLOADS.to_vec();
        let mut members = vec!["README.md"];
        if let Some(extra) = capture.extra {
            payloads.extend(["profile.json", extra]);
            members.push("SHA256SUMS");
        }
        members.extend(payloads.iter().copied());
        exact_members(&files.keys().map(String::as_str).collect(), &members)?;
        let text = from_utf8(&files[index_name(capture)]).map_err(|error| error.to_string())?;
        let entries = parse_index(text, capture.extra.is_some())?;
        exact_members(&entries.keys().copied().collect(), &payloads)?;
        for (name, expected) in &entries {
            if digest(&files[*name]) != *expected {
                return Err(format!("{}: {name} digest mismatch", capture.directory));
            }
        }
        Ok(entries.len())
    }

    fn verify_profile_binding(
        capture: &Capture,
        files: &Files,
        declared: &[u8],
    ) -> Result<(), String> {
        let declared_digest = digest(declared);
        for name in ["observations.json", "report.json"] {
            let document: Value =
                serde_json::from_slice(&files[name]).map_err(|error| error.to_string())?;
            if document["profile_sha256"].as_str() != Some(declared_digest.as_str()) {
                return Err(format!("{name}: profile binding mismatch"));
            }
        }
        if capture.extra.is_some() && files["profile.json"] != declared {
            return Err("profile.json: snapshot mismatch".to_owned());
        }
        Ok(())
    }

    #[test]
    fn retained_indexes_bind_seven_captures_and_twenty_five_raw_payloads() {
        let root = evidence_root();
        let mut directories = BTreeSet::new();
        for result in fs::read_dir(&root).unwrap() {
            let entry = result.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "{}", entry.path().display());
            if kind.is_dir() {
                directories.insert(entry.file_name().into_string().unwrap());
            }
        }
        let expected: Vec<_> = CAPTURES.iter().map(|capture| capture.directory).collect();
        assert_eq!(expected.len(), 7);
        exact_members(&directories.iter().map(String::as_str).collect(), &expected).unwrap();
        let mut payloads = 0_usize;
        for capture in CAPTURES {
            let files = read_capture(&root.join(capture.directory)).unwrap();
            payloads += verify_capture(capture, &files)
                .unwrap_or_else(|error| panic!("{}: {error}", capture.directory));
            let declared = read_regular(
                &root
                    .join("../fixtures/resource-qualification")
                    .join(capture.profile),
            )
            .unwrap();
            verify_profile_binding(capture, &files, &declared)
                .unwrap_or_else(|error| panic!("{}: {error}", capture.directory));
        }
        assert_eq!(payloads, 25);
        // Historical runner/report hash discrepancies are retained, not repaired here.
    }

    fn replace_indexed_payload(capture: &Capture, files: &mut Files, name: &str, bytes: Vec<u8>) {
        let old = digest(&files[name]);
        let new = digest(&bytes);
        let index = index_name(capture);
        let text = from_utf8(&files[index]).unwrap();
        assert!(text.contains(&old));
        let revised = text.replacen(&old, &new, 1).into_bytes();
        files.insert(name.to_owned(), bytes);
        files.insert(index.to_owned(), revised);
        verify_capture(capture, files).unwrap();
    }

    #[test]
    fn profile_bindings_reject_reindexed_observations_reports_and_snapshots() {
        for capture in CAPTURES {
            let files = read_capture(&evidence_root().join(capture.directory)).unwrap();
            let declared = read_regular(
                &evidence_root()
                    .join("../fixtures/resource-qualification")
                    .join(capture.profile),
            )
            .unwrap();
            verify_profile_binding(capture, &files, &declared).unwrap();
            for name in ["observations.json", "report.json"] {
                let mut changed = files.clone();
                let mut document: Value = serde_json::from_slice(&changed[name]).unwrap();
                document["profile_sha256"] = Value::String("0".repeat(64));
                replace_indexed_payload(
                    capture,
                    &mut changed,
                    name,
                    serde_json::to_vec(&document).unwrap(),
                );
                assert_eq!(
                    verify_profile_binding(capture, &changed, &declared),
                    Err(format!("{name}: profile binding mismatch"))
                );
            }
            if capture.extra.is_some() {
                let mut changed = files.clone();
                let mut bytes = changed["profile.json"].clone();
                bytes.push(b' ');
                replace_indexed_payload(capture, &mut changed, "profile.json", bytes);
                assert_eq!(
                    verify_profile_binding(capture, &changed, &declared),
                    Err("profile.json: snapshot mismatch".to_owned())
                );
            }
        }
    }

    #[test]
    fn each_payload_and_each_index_digest_is_checked_without_normalizing_bytes() {
        for capture in CAPTURES {
            let files = read_capture(&evidence_root().join(capture.directory)).unwrap();
            let index = index_name(capture);
            let text = from_utf8(&files[index]).unwrap();
            for (name, hash) in parse_index(text, capture.extra.is_some()).unwrap() {
                let mut changed_bytes = files.clone();
                changed_bytes.get_mut(name).unwrap().push(b' ');
                assert!(
                    verify_capture(capture, &changed_bytes).is_err(),
                    "{name}: changed bytes"
                );

                let replacement = format!(
                    "{}{}",
                    if hash.starts_with('0') { '1' } else { '0' },
                    hash.get(1..).unwrap()
                );
                let mut changed_digest = files.clone();
                changed_digest.insert(
                    index.to_owned(),
                    text.replacen(hash, &replacement, 1).into_bytes(),
                );
                assert!(
                    verify_capture(capture, &changed_digest).is_err(),
                    "{name}: changed digest"
                );

                let row = if capture.extra.is_some() {
                    format!("{hash}  {name}\n")
                } else {
                    format!("| {name} | {hash} |\n")
                };
                for edited in [
                    text.replacen(&row, "", 1),
                    format!("{text}{row}"),
                    text.replacen(name, "renamed.json", 1),
                ] {
                    let mut changed_members = files.clone();
                    changed_members.insert(index.to_owned(), edited.into_bytes());
                    assert!(
                        verify_capture(capture, &changed_members).is_err(),
                        "{name}: changed membership"
                    );
                }
            }
        }
    }

    #[test]
    fn missing_or_extra_capture_files_and_directories_cannot_reduce_coverage() {
        let expected: Vec<_> = CAPTURES.iter().map(|capture| capture.directory).collect();
        let directories: BTreeSet<_> = expected.iter().copied().collect();
        for capture in CAPTURES {
            let mut missing_directory = directories.clone();
            missing_directory.remove(capture.directory);
            exact_members(&missing_directory, &expected).unwrap_err();
            let files = read_capture(&evidence_root().join(capture.directory)).unwrap();
            for name in files.keys() {
                let mut missing = files.clone();
                missing.remove(name);
                assert!(
                    verify_capture(capture, &missing).is_err(),
                    "{}: missing {name}",
                    capture.directory
                );
            }
            let mut extra = files.clone();
            extra.insert("unexpected.json".to_owned(), Vec::new());
            verify_capture(capture, &extra).unwrap_err();
        }
        let mut extra = directories;
        extra.insert("unexpected-capture");
        exact_members(&extra, &expected).unwrap_err();
        read_regular(&evidence_root()).unwrap_err();
        read_regular(&evidence_root().join("missing-evidence-file")).unwrap_err();
    }

    #[test]
    fn digest_parsers_refuse_empty_malformed_duplicate_and_unsafe_entries() {
        let hash = "a".repeat(64);
        for manifest in [false, true] {
            let header = if manifest { "" } else { TABLE_HEADER };
            let row = |name: &str, digest: &str| {
                if manifest {
                    format!("{digest}  {name}\n")
                } else {
                    format!("| {name} | {digest} |\n")
                }
            };
            let valid = format!("{header}{}", row("report.json", &hash));
            parse_index(&valid, manifest).unwrap();
            for malformed in [
                String::new(),
                header.to_owned(),
                format!("{valid}{}", row("report.json", &hash)),
                format!("{header}{hash} report.json\n"),
                format!("{valid}unexpected trailing text\n"),
            ] {
                assert!(parse_index(&malformed, manifest).is_err(), "{malformed:?}");
            }
            for bad_hash in [
                String::new(),
                "a".repeat(63),
                "a".repeat(65),
                "A".repeat(64),
                "g".repeat(64),
            ] {
                parse_index(
                    &format!("{header}{}", row("report.json", &bad_hash)),
                    manifest,
                )
                .unwrap_err();
            }
            for name in [
                "",
                ".",
                "..",
                "../report.json",
                "/report.json",
                "sub/report.json",
                "..\\report.json",
                "report.json extra",
                "report.json | extra",
            ] {
                assert!(
                    parse_index(&format!("{header}{}", row(name, &hash)), manifest).is_err(),
                    "{name:?}"
                );
            }
        }
    }
}

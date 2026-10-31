//! Bounded file and JSON controls with fixed, content-free error categories.

use super::{HarnessError, Result};
use serde::{
    Serialize,
    de::{
        DeserializeOwned, DeserializeSeed, Deserializer, Error as DeError, MapAccess, SeqAccess,
        Visitor,
    },
};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::fmt::{self, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{self as stdio, Read as _, Write};
use std::path::Path;
use std::result::Result as StdResult;

/// Read at most `cap` bytes from a regular file.
///
/// # Errors
/// Returns a fixed input error for unreadable, nonregular or oversized files.
pub fn read_bytes(path: &Path, cap: usize) -> Result<Vec<u8>> {
    let limit = u64::try_from(cap).map_err(|_error| HarnessError::new("input_limit"))?;
    let file = open_input(path, false)?;
    let metadata = file
        .metadata()
        .map_err(|_error| HarnessError::new("input_io"))?;
    if !metadata.is_file() {
        return Err(HarnessError::new("invalid_input"));
    }
    if metadata.len() > limit {
        return Err(HarnessError::new("input_limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_error| HarnessError::new("input_io"))?;
    if bytes.len() > cap {
        return Err(HarnessError::new("input_limit"));
    }
    Ok(bytes)
}

/// Read typed JSON with byte, node and depth bounds and duplicate-key rejection.
///
/// # Errors
/// Returns a fixed input error for file failures, limits or invalid JSON.
pub fn read_json<T: DeserializeOwned>(path: &Path, cap: usize) -> Result<T> {
    let bytes = read_bytes(path, cap)?;
    parse_json(&bytes, cap)
}

/// Parse the same bounded bytes used by an input digest, rejecting duplicate keys.
///
/// # Errors
/// Returns a fixed input error for limits or invalid JSON.
pub fn parse_json<T: DeserializeOwned>(bytes: &[u8], cap: usize) -> Result<T> {
    if bytes.len() > cap {
        return Err(HarnessError::new("input_limit"));
    }
    auctioneer_runtime::count_json_nodes(bytes, 1 << 16)
        .map_err(|_error| HarnessError::new("invalid_input"))?;
    // Walk discarded fields too, retaining serde_json's depth cap and checking
    // decoded keys before a Value or typed deserializer can discard duplicates.
    let mut input = serde_json::Deserializer::from_slice(bytes);
    UniqueJson
        .deserialize(&mut input)
        .and_then(|()| input.end())
        .map_err(|_error| HarnessError::new("invalid_input"))?;
    serde_json::from_slice(bytes).map_err(|_error| HarnessError::new("invalid_input"))
}

struct UniqueJson;

impl<'de> DeserializeSeed<'de> for UniqueJson {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> StdResult<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "JSON visitor retains Serde defaults for alternate scalars, unsupported types and private hooks"
)]
impl<'de> Visitor<'de> for UniqueJson {
    type Value = ();

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON with unique object keys")
    }

    fn visit_unit<E: DeError>(self) -> StdResult<(), E> {
        Ok(())
    }

    fn visit_bool<E: DeError>(self, _value: bool) -> StdResult<(), E> {
        Ok(())
    }

    fn visit_i64<E: DeError>(self, _value: i64) -> StdResult<(), E> {
        Ok(())
    }

    fn visit_u64<E: DeError>(self, _value: u64) -> StdResult<(), E> {
        Ok(())
    }

    fn visit_f64<E: DeError>(self, _value: f64) -> StdResult<(), E> {
        Ok(())
    }

    fn visit_str<E: DeError>(self, _value: &str) -> StdResult<(), E> {
        Ok(())
    }

    fn visit_seq<A>(self, mut seq: A) -> StdResult<(), A::Error>
    where
        A: SeqAccess<'de>,
    {
        while seq.next_element_seed(UniqueJson)?.is_some() {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> StdResult<(), A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(DeError::custom("duplicate object key"));
            }
            map.next_value_seed(UniqueJson)?;
        }
        Ok(())
    }
}

/// Serialize a JSON body of at most `cap` bytes, without a trailing newline.
///
/// # Errors
/// Returns `output_limit` if serialization fails or exceeds the byte cap.
pub fn json_bytes<T: Serialize>(value: &T, cap: usize) -> Result<Vec<u8>> {
    let size = auctioneer_runtime::serialized_json_bytes(value)
        .map_err(|_error| HarnessError::new("output_limit"))?;
    let capacity = usize::try_from(size).map_err(|_error| HarnessError::new("output_limit"))?;
    if capacity > cap {
        return Err(HarnessError::new("output_limit"));
    }
    serde_json::to_vec(value).map_err(|_error| HarnessError::new("output_limit"))
}

/// Exclusively create, write newline-terminated JSON, and sync the file.
///
/// The cap excludes the final newline. Unix files are created with mode `0600`.
///
/// # Errors
/// Returns `output_limit` for serialization limits or `output_io` for file failures.
pub fn write_json<T: Serialize>(path: &Path, value: &T, cap: usize) -> Result<()> {
    let bytes = json_bytes(value, cap)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600)
    };
    let mut file = options
        .open(path)
        .map_err(|_error| HarnessError::new("output_io"))?;
    write_json_bytes(&mut file, &bytes, |output| output.sync_all())
}

fn write_json_bytes<W: Write>(
    writer: &mut W,
    bytes: &[u8],
    sync_all: impl FnOnce(&mut W) -> stdio::Result<()>,
) -> Result<()> {
    writer
        .write_all(bytes)
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| sync_all(writer))
        .map_err(|_error| HarnessError::new("output_io"))
}

/// Return the lowercase SHA-256 digest of the supplied bytes.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hash compact JSON with canonically ordered object keys.
///
/// # Errors
/// Returns `invalid_input` if the value cannot be serialized.
pub fn canonical_digest<T: Serialize>(value: &T) -> Result<String> {
    let ordered =
        serde_json::to_value(value).map_err(|_error| HarnessError::new("invalid_input"))?;
    let bytes =
        serde_json::to_vec(&ordered).map_err(|_error| HarnessError::new("invalid_input"))?;
    Ok(sha256(&bytes))
}

/// Hash a bounded regular file, permitting links.
///
/// # Errors
/// Returns a fixed input error for unreadable, nonregular or oversized files.
pub fn hash_file(path: &Path, cap: usize) -> Result<String> {
    hash_open_file(open_input(path, false)?, cap)
}

fn hash_open_file(mut file: File, cap: usize) -> Result<String> {
    let metadata = file
        .metadata()
        .map_err(|_error| HarnessError::new("input_io"))?;
    let limit = u64::try_from(cap).map_err(|_error| HarnessError::new("input_limit"))?;
    if !metadata.is_file() {
        return Err(HarnessError::new("invalid_input"));
    }
    if metadata.len() > limit {
        return Err(HarnessError::new("input_limit"));
    }
    let mut digest = Sha256::new();
    let mut bytes = 0_usize;
    let mut chunk = [0_u8; 8192];
    loop {
        let count = file
            .read(&mut chunk)
            .map_err(|_error| HarnessError::new("input_io"))?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count)
            .ok_or(HarnessError::new("input_limit"))?;
        if bytes > cap {
            return Err(HarnessError::new("input_limit"));
        }
        digest.update(chunk.get(..count).ok_or(HarnessError::new("input_io"))?);
    }
    Ok(format!("{:x}", digest.finalize()))
}

/// Hash a bounded regular file, refusing final symlinks and hardlinks on Unix.
///
/// # Errors
/// Returns a fixed input error for unsafe, unreadable or oversized files.
pub fn file_digest(path: &Path, cap: usize) -> Result<String> {
    hash_open_file(open_input(path, true)?, cap)
}

fn open_input(path: &Path, strict: bool) -> Result<File> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags, open};
        let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let flags_for_path = if strict {
            flags | OFlags::NOFOLLOW
        } else {
            flags
        };
        let fd = open(path, flags_for_path, Mode::empty())
            .map_err(|_error| HarnessError::new("invalid_input"))?;
        File::from(fd)
    };
    #[cfg(not(unix))]
    let file = File::open(path).map_err(|_error| HarnessError::new("input_io"))?;
    let metadata = file
        .metadata()
        .map_err(|_error| HarnessError::new("input_io"))?;
    if !metadata.is_file() {
        return Err(HarnessError::new("invalid_input"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        if strict && metadata.nlink() != 1 {
            return Err(HarnessError::new("invalid_input"));
        }
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fmt::Debug;
    use std::fs;
    use std::io::{Cursor, Error as IoError};

    #[test]
    fn byte_parser_accepts_exact_cap_and_refuses_overflow_and_invalid_json() {
        let value = json!({"safe": true});
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            parse_json::<serde_json::Value>(&bytes, bytes.len()).unwrap(),
            value
        );
        assert_eq!(
            parse_json::<serde_json::Value>(&bytes, bytes.len() - 1).unwrap_err(),
            HarnessError::new("input_limit")
        );
        assert_eq!(
            parse_json::<serde_json::Value>(b"{", 1).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn bounded_json_accepts_the_exact_cap_and_refuses_overflow_without_source() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("SECRET_FILE");
        let value = json!({"safe":true});
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&file, &bytes).unwrap();
        assert_eq!(
            read_json::<serde_json::Value>(&file, bytes.len()).unwrap(),
            value
        );
        assert_eq!(
            read_json::<serde_json::Value>(&file, bytes.len() - 1).unwrap_err(),
            HarnessError::new("input_limit")
        );
        fs::write(&file, b"SECRET_INVALID_JSON").unwrap();
        assert_eq!(
            read_json::<serde_json::Value>(&file, 1024).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    fn assert_duplicate_json_refused<T: DeserializeOwned + Debug>(bytes: &[u8]) {
        assert_eq!(
            parse_json::<T>(bytes, bytes.len()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("input.json");
        fs::write(&file, bytes).unwrap();
        assert_eq!(
            read_json::<T>(&file, bytes.len()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn duplicate_json_rejects_top_level_authorization_shadowing() {
        for bytes in [
            br#"{"authorized":false,"authorized":true}"#.as_slice(),
            br#"{"authorized":true,"authorized":false}"#.as_slice(),
            br#"{"authorized":true,"authorized":true}"#.as_slice(),
        ] {
            assert_duplicate_json_refused::<serde_json::Value>(bytes);
        }
    }

    #[test]
    fn duplicate_json_rejects_nested_artifact_digest_shadowing() {
        assert_duplicate_json_refused::<serde_json::Value>(
            br#"{"artifact":{"sha256":"MISMATCHED_DIGEST","sha256":"MATCHING_DIGEST"}}"#,
        );
    }

    #[test]
    fn duplicate_json_rejects_duplicate_keys_in_array_objects() {
        assert_duplicate_json_refused::<serde_json::Value>(
            br#"[{"authorized":true},{"evidence":[{"authorized":false,"authorized":true}]}]"#,
        );
    }

    #[test]
    fn duplicate_json_rejects_duplicate_keys_in_ignored_fields() {
        #[derive(Debug, serde::Deserialize)]
        struct Discard {}

        for bytes in [
            br#"{"ignored":false,"ignored":true}"#.as_slice(),
            br#"{"ignored":{"authorized":false,"authorized":true}}"#.as_slice(),
            br#"{"ignored":[{"sha256":"MISMATCHED_DIGEST","sha256":"MATCHING_DIGEST"}]}"#
                .as_slice(),
        ] {
            assert_duplicate_json_refused::<Discard>(bytes);
        }
    }

    #[test]
    fn duplicate_json_rejects_identical_decoded_escaped_keys() {
        for bytes in [
            br#"{"authorized":false,"\u0061uthorized":true}"#.as_slice(),
            br#"{"\u0061uthorized":false,"authorized":true}"#.as_slice(),
            br#"{"nested":[{"\u006bey":1,"k\u0065y":2}]}"#.as_slice(),
            br#"{"\ud83d\udd12":false,"\uD83D\uDD12":true}"#.as_slice(),
        ] {
            assert_duplicate_json_refused::<serde_json::Value>(bytes);
        }
    }

    #[test]
    fn duplicate_json_errors_do_not_expose_keys_or_values() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("SECRET_FILE");
        let bytes = br#"{"SECRET_KEY":"SECRET_FIRST_VALUE","SECRET_KEY":"SECRET_SECOND_VALUE"}"#;
        fs::write(&file, bytes).unwrap();
        let error = read_json::<serde_json::Value>(&file, bytes.len()).unwrap_err();
        assert_eq!(error, HarnessError::new("invalid_input"));
        assert!(!format!("{error:?}").contains("SECRET"));
        assert!(!serde_json::to_string(&error).unwrap().contains("SECRET"));
    }

    #[test]
    fn duplicate_json_allows_the_same_key_in_separate_objects() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("input.json");
        let value = json!({
            "authorized": true,
            "nested": {"authorized": false},
            "evidence": [
                {"authorized": false, "sha256": "FIRST_DIGEST"},
                {"authorized": true, "sha256": "SECOND_DIGEST"}
            ],
            "scalars": [null, false, -1_i32, 2_i32, 1.5_f64, "text"],
            "empty_object": {},
            "empty_array": []
        });
        let bytes = serde_json::to_vec(&value).unwrap();
        fs::write(&file, &bytes).unwrap();
        assert_eq!(
            read_json::<serde_json::Value>(&file, bytes.len()).unwrap(),
            value
        );
    }

    #[test]
    fn duplicate_json_keeps_typed_semantic_validation() {
        #[derive(Debug, serde::Deserialize)]
        struct Evidence {
            #[serde(alias = "allowed")]
            authorized: bool,
        }

        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("input.json");
        let bytes = serde_json::to_vec(&json!({"authorized": true})).unwrap();
        fs::write(&file, &bytes).unwrap();
        assert!(
            read_json::<Evidence>(&file, bytes.len())
                .unwrap()
                .authorized
        );

        let aliased = serde_json::to_vec(&json!({"authorized": false, "allowed": true})).unwrap();
        fs::write(&file, &aliased).unwrap();
        assert_eq!(
            read_json::<Evidence>(&file, aliased.len()).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn private_json_writer_propagates_sync_all_failure_without_source() {
        let bytes = serde_json::to_vec(&json!({"safe": true})).unwrap();
        let expected = [bytes.as_slice(), b"\n"].concat();
        let mut output = Vec::new();
        let mut synced = false;
        let error = write_json_bytes(&mut output, &bytes, |written| {
            assert_eq!(*written, expected);
            synced = true;
            Err(IoError::other("SECRET_SYNC_FAILURE"))
        })
        .unwrap_err();
        assert!(synced);
        assert_eq!(output, expected);
        assert_eq!(error, HarnessError::new("output_io"));
        assert!(!format!("{error:?}").contains("SECRET"));
        assert!(!serde_json::to_string(&error).unwrap().contains("SECRET"));
    }

    #[test]
    fn private_json_writer_syncs_complete_output_before_success() {
        let bytes = serde_json::to_vec(&json!({"safe": true})).unwrap();
        let expected = [bytes.as_slice(), b"\n"].concat();
        let mut output = Vec::new();
        let mut synced = false;
        write_json_bytes(&mut output, &bytes, |written| {
            assert_eq!(*written, expected);
            synced = true;
            Ok(())
        })
        .unwrap();
        assert!(synced);
        assert_eq!(output, expected);
    }

    #[test]
    fn private_json_writer_does_not_sync_after_body_or_newline_failure() {
        let bytes = serde_json::to_vec(&json!({"safe": true})).unwrap();
        for capacity in [bytes.len() - 1, bytes.len()] {
            let mut storage = vec![0_u8; capacity];
            let mut output = Cursor::new(storage.as_mut_slice());
            let mut synced = false;
            assert_eq!(
                write_json_bytes(&mut output, &bytes, |_written| {
                    synced = true;
                    Ok(())
                })
                .unwrap_err(),
                HarnessError::new("output_io")
            );
            assert!(!synced);
            assert_eq!(output.position(), u64::try_from(capacity).unwrap());
            assert_eq!(output.into_inner(), &bytes[..capacity]);
        }
    }

    #[test]
    fn private_json_is_bounded_complete_and_does_not_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("output.json");
        let value = json!({"safe":true});
        let bytes = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            write_json(&file, &value, bytes.len() - 1).unwrap_err(),
            HarnessError::new("output_limit")
        );
        assert!(!file.exists());
        write_json(&file, &value, bytes.len()).unwrap();
        assert_eq!(fs::read(&file).unwrap(), [bytes.as_slice(), b"\n"].concat());
        assert_eq!(
            write_json(&file, &value, 1024).unwrap_err(),
            HarnessError::new("output_io")
        );
        assert_eq!(read_json::<serde_json::Value>(&file, 1024).unwrap(), value);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn canonical_digests_ignore_object_order_but_not_values_or_array_order() {
        let first = json!({"b":[1_u32,2_u32],"a":true});
        let second = json!({"a":true,"b":[1_u32,2_u32]});
        assert_eq!(
            canonical_digest(&first).unwrap(),
            canonical_digest(&second).unwrap()
        );
        assert_ne!(
            canonical_digest(&first).unwrap(),
            canonical_digest(&json!({"a":true,"b":[2_u32,1_u32]})).unwrap()
        );
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(unix)]
    #[test]
    fn artifact_hashing_refuses_links_and_over_limit_files() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        fs::write(&file, b"abc").unwrap();
        assert_eq!(file_digest(&file, 3).unwrap(), sha256(b"abc"));
        assert_eq!(
            file_digest(&file, 2).unwrap_err(),
            HarnessError::new("input_limit")
        );
        let soft = root.path().join("soft");
        symlink(&file, &soft).unwrap();
        assert_eq!(hash_file(&soft, 3).unwrap(), sha256(b"abc"));
        assert_eq!(
            file_digest(&soft, 3).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        let hard = root.path().join("hard");
        fs::hard_link(&file, &hard).unwrap();
        assert_eq!(
            file_digest(&hard, 3).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[test]
    fn structural_limits_count_discarded_values_before_typed_parsing() {
        #[derive(Debug, serde::Deserialize)]
        struct Discard {}
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("input.json");
        let exact = serde_json::to_vec(&json!({"discard": vec![0_u8; (1 << 16)-3]})).unwrap();
        fs::write(&file, exact).unwrap();
        read_json::<Discard>(&file, 1 << 20).unwrap();
        fs::write(
            &file,
            serde_json::to_vec(&json!({"discard": vec![0_u8; (1 << 16)-2]})).unwrap(),
        )
        .unwrap();
        assert_eq!(
            read_json::<Discard>(&file, 1 << 20).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        fs::write(
            &file,
            format!("{{\"discard\":{}0{}}}", "[".repeat(129), "]".repeat(129)),
        )
        .unwrap();
        assert_eq!(
            read_json::<Discard>(&file, 1 << 20).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }

    #[cfg(unix)]
    #[test]
    fn nonregular_inputs_refuse_without_waiting_for_a_writer() {
        use std::process::Command;
        let root = tempfile::tempdir().unwrap();
        let pipe = root.path().join("pipe");
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(&pipe)
                .env_clear()
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            read_bytes(&pipe, 1024).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            hash_file(&pipe, 1024).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            file_digest(&pipe, 1024).unwrap_err(),
            HarnessError::new("invalid_input")
        );
        assert_eq!(
            read_bytes(root.path(), 1024).unwrap_err(),
            HarnessError::new("invalid_input")
        );
    }
}

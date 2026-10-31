use super::super::{HarnessError, Result, io, process};
use super::SDK_PIN;
use super::{archive, artifacts, preparation};
use serde::Deserialize;
use std::env::temp_dir;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
#[cfg(all(test, unix))]
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};
use std::str::from_utf8;

const VERIFIER: &str = ".github/actions/release-core/scripts/prepare-release.sh";
const COMMON: &str = ".github/actions/deploy-core/scripts/common.sh";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyFastlyArchiveRequest {
    pub sdk_root: PathBuf,
    pub archive: PathBuf,
    pub archive_sha256: String,
    pub source_revision: String,
    pub destination: PathBuf,
}

pub async fn verify_fastly_archive(
    input: &Path,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    let request: VerifyFastlyArchiveRequest = io::read_json(input, preparation::INPUT_CAP)?;
    verify(request, supervisor).await
}

fn destination_path(request: &Path) -> Result<PathBuf> {
    let refusal = HarnessError::new("workspace_not_isolated");
    if !request.is_absolute()
        || request
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(refusal);
    }
    let basename = request.file_name().ok_or(refusal)?;
    let parent = preparation::canonical_directory(
        request.parent().ok_or(refusal)?,
        "workspace_not_isolated",
    )?;
    let temporary = fs::canonicalize(temp_dir()).map_err(|_error| refusal)?;
    let repository = preparation::canonical_directory(
        &preparation::repository_root(),
        "workspace_not_isolated",
    )?;
    if !parent.starts_with(&temporary) || parent.starts_with(&repository) {
        return Err(refusal);
    }
    let destination = parent.join(basename);
    match fs::symlink_metadata(&destination) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(destination),
        _ => Err(refusal),
    }
}

async fn pinned_sdk(
    root: &Path,
    supervisor: &process::Supervisor,
) -> Result<(PathBuf, Vec<u8>, Vec<u8>)> {
    let canonical = preparation::canonical_directory(root, "sdk_identity")?;
    let revision = preparation::git(
        supervisor,
        &canonical,
        &["rev-parse", "HEAD"],
        preparation::INPUT_CAP,
    )
    .await
    .map_err(|_error| HarnessError::new("sdk_identity"))?;
    if from_utf8(&revision)
        .map_err(|_error| HarnessError::new("sdk_identity"))?
        .trim()
        != SDK_PIN
    {
        return Err(HarnessError::new("sdk_identity"));
    }
    let mut trusted = Vec::new();
    for relative in [VERIFIER, COMMON] {
        let bytes =
            preparation::strict_file(&canonical, Path::new(relative), preparation::INPUT_CAP)
                .map_err(|_error| HarnessError::new("sdk_identity"))?;
        let committed = preparation::git(
            supervisor,
            &canonical,
            &["show", &format!("{SDK_PIN}:{relative}")],
            preparation::INPUT_CAP,
        )
        .await
        .map_err(|_error| HarnessError::new("sdk_identity"))?;
        if bytes != committed {
            return Err(HarnessError::new("sdk_identity"));
        }
        trusted.push(bytes);
    }
    let common = trusted.pop().ok_or(HarnessError::new("sdk_identity"))?;
    let verifier = trusted.pop().ok_or(HarnessError::new("sdk_identity"))?;
    Ok((canonical, verifier, common))
}

pub(super) async fn verify(
    request: VerifyFastlyArchiveRequest,
    supervisor: &process::Supervisor,
) -> Result<serde_json::Value> {
    supervisor.check_active().await?;
    if !preparation::hex(&request.archive_sha256, 64) {
        return Err(HarnessError::new("invalid_input"));
    }
    if !preparation::hex(&request.source_revision, 40)
        && !preparation::hex(&request.source_revision, 64)
    {
        return Err(HarnessError::new("release_identity"));
    }
    let destination = destination_path(&request.destination)?;
    let archive = fs::canonicalize(&request.archive)
        .map_err(|_error| HarnessError::new("release_invalid"))?;
    let archive_parent = preparation::canonical_directory(
        archive
            .parent()
            .ok_or(HarnessError::new("release_invalid"))?,
        "release_invalid",
    )?;
    // Check the supplied final component too: canonicalization must not hide a
    // symlink before the strict reader sees it.
    let supplied = fs::symlink_metadata(&request.archive)
        .map_err(|_error| HarnessError::new("release_invalid"))?;
    if !supplied.is_file() || supplied.file_type().is_symlink() {
        return Err(HarnessError::new("release_invalid"));
    }
    let bytes = preparation::strict_file(
        &archive_parent,
        Path::new(
            archive
                .file_name()
                .ok_or(HarnessError::new("release_invalid"))?,
        ),
        preparation::FILE_CAP,
    )
    .map_err(|_error| HarnessError::new("release_invalid"))?;
    if io::sha256(&bytes) != request.archive_sha256 {
        return Err(HarnessError::new("release_invalid"));
    }
    archive::preflight(&bytes, &request.source_revision, supervisor).await?;
    // An explicit invalid checkout is always a refusal, never an optional skip.
    let (sdk_root, verifier_bytes, common_bytes) =
        pinned_sdk(&request.sdk_root, supervisor).await?;
    let scratch = tempfile::Builder::new()
        .prefix("auctioneer-sdk-")
        .tempdir()
        .map_err(|_error| HarnessError::new("output_io"))?;
    let scratch_root = preparation::canonical_directory(scratch.path(), "output_io")?;
    let output = run_verifier(
        &request,
        supervisor,
        &scratch_root,
        &verifier_bytes,
        &common_bytes,
        &bytes,
    )
    .await?;
    let verified_root = scratch_root.join("verified");
    let (metadata, release_sha256) =
        artifacts::verify_bundle(&verified_root, &request.source_revision)
            .map_err(|_error| HarnessError::new("release_invalid"))?;
    let (_sdk, post_verifier, post_common) = pinned_sdk(&sdk_root, supervisor).await?;
    if post_verifier != verifier_bytes || post_common != common_bytes {
        return Err(HarnessError::new("sdk_identity"));
    }
    if io::file_digest(&archive, preparation::FILE_CAP)
        .map_err(|_error| HarnessError::new("release_invalid"))?
        != request.archive_sha256
    {
        return Err(HarnessError::new("release_invalid"));
    }
    supervisor.check_active().await?;
    preparation::private_directory(&destination)?;
    for relative in [
        "release.json",
        &metadata.app_cli.path,
        &metadata.package.path,
        &metadata.manifests.edgezero.path,
        &metadata.manifests.adapter.path,
    ] {
        supervisor.check_active().await?;
        let member =
            preparation::strict_file(&verified_root, Path::new(relative), preparation::FILE_CAP)
                .map_err(|_error| HarnessError::new("release_invalid"))?;
        artifacts::create_member_parents(&destination, relative)?;
        preparation::write_private(
            &artifacts::release_path(&destination, relative)?,
            &member,
            preparation::FILE_CAP,
        )?;
    }
    let (_staged, staged_digest) = artifacts::verify_bundle(&destination, &request.source_revision)
        .map_err(|_error| HarnessError::new("release_invalid"))?;
    if staged_digest != release_sha256 {
        return Err(HarnessError::new("release_invalid"));
    }
    supervisor.check_active().await?;
    Ok(
        serde_json::json!({ "sdk_verification": "verified_offline", "edgezero_pin": SDK_PIN,
        "archive_sha256": request.archive_sha256, "release_sha256": release_sha256, "verifier": output.summary }),
    )
}

async fn run_verifier(
    request: &VerifyFastlyArchiveRequest,
    supervisor: &process::Supervisor,
    scratch_root: &Path,
    verifier_bytes: &[u8],
    common_bytes: &[u8],
    archive_bytes: &[u8],
) -> Result<process::CommandOutput> {
    // The pinned shell verifier's EXIT trap quotes its own scratch path. Keep
    // that generated path free of shell metacharacters even under custom TMPDIR.
    if !scratch_root
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(byte))
    {
        return Err(HarnessError::new("workspace_not_isolated"));
    }
    for relative in [
        ".github",
        ".github/actions",
        ".github/actions/release-core",
        ".github/actions/release-core/scripts",
        ".github/actions/deploy-core",
        ".github/actions/deploy-core/scripts",
    ] {
        preparation::private_directory(&scratch_root.join(relative))?;
    }
    preparation::write_private(
        &scratch_root.join(VERIFIER),
        verifier_bytes,
        preparation::INPUT_CAP,
    )?;
    preparation::write_private(
        &scratch_root.join(COMMON),
        common_bytes,
        preparation::INPUT_CAP,
    )?;
    let trusted_archive = scratch_root.join("application-release.tar.gz");
    preparation::write_private(&trusted_archive, archive_bytes, preparation::FILE_CAP)?;
    let verified_root = scratch_root.join("verified");
    let mut spec = process::ProcessSpec::new("/bin/bash");
    spec.args = vec![scratch_root.join(VERIFIER).into_os_string()];
    spec.cwd = Some(scratch_root.to_path_buf());
    spec.timeout_ms = 30_000;
    spec.output_cap = preparation::INPUT_CAP;
    spec.env = preparation::tool_environment();
    for (name, value) in [
        (
            "EDGEZERO__APP__RELEASE__ARCHIVE",
            trusted_archive.into_os_string(),
        ),
        (
            "EDGEZERO__APP__RELEASE__SHA256",
            OsString::from(&request.archive_sha256),
        ),
        (
            "EDGEZERO__APP__RELEASE__EXPECTED_SOURCE_REVISION",
            OsString::from(&request.source_revision),
        ),
        (
            "EDGEZERO__APP__RELEASE__EXPECTED_ADAPTER",
            OsString::from("fastly"),
        ),
        (
            "EDGEZERO__APP__RELEASE__EXPECTED_LIFECYCLE_PROTOCOL",
            OsString::from("1"),
        ),
        (
            "EDGEZERO__APP__RELEASE__ROOT",
            verified_root.clone().into_os_string(),
        ),
    ] {
        spec.env.insert(OsString::from(name), value);
    }
    let output = supervisor
        .run(spec)
        .await
        .map_err(|_error| HarnessError::new("release_invalid"))?;
    if !output.summary.closed {
        return Err(HarnessError::new("release_invalid"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use serde_json::{Value, json};
    use std::env::var_os;
    use std::io::empty;

    async fn checkout(supervisor: &process::Supervisor) -> PathBuf {
        if let Some(explicit) = var_os("EDGEZERO_SDK_ROOT") {
            let path = PathBuf::from(explicit);
            pinned_sdk(&path, supervisor).await.unwrap();
            return path;
        }
        let mut command = process::ProcessSpec::new(env!("CARGO"));
        command.args = ["metadata", "--locked", "--offline", "--format-version", "1"]
            .into_iter()
            .map(OsString::from)
            .collect();
        command.cwd = Some(preparation::repository_root());
        command.output_cap = 8 << 20_u32;
        command.env = preparation::tool_environment();
        for name in ["HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN"] {
            if let Some(value) = var_os(name) {
                command.env.insert(OsString::from(name), value);
            }
        }
        let output = supervisor.run(command).await.unwrap();
        let metadata: Value = serde_json::from_slice(&output.stdout).unwrap();
        let packages = metadata["packages"].as_array().unwrap();
        let sdk: Vec<_> = packages
            .iter()
            .filter(|package| package["name"] == "edgezero-core")
            .collect();
        assert_eq!(sdk.len(), 1);
        let manifest = Path::new(sdk[0]["manifest_path"].as_str().unwrap());
        let root = manifest
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();
        pinned_sdk(&root, supervisor).await.unwrap();
        root
    }

    #[tokio::test]
    async fn explicit_invalid_sdk_roots_always_fail_without_executing_a_caller_command() {
        let root = tempfile::tempdir().unwrap();
        let supervisor = process::Supervisor::new();
        for candidate in [
            root.path().join("missing"),
            root.path().to_owned(),
            root.path().join("not-a-checkout"),
        ] {
            assert_eq!(
                pinned_sdk(&candidate, &supervisor).await.unwrap_err().code,
                "sdk_identity"
            );
        }
        supervisor.cleanup().await.unwrap();
    }

    fn small_release_members() -> Vec<(&'static str, Vec<u8>)> {
        let member = |name: &str| json!({"path":name,"sha256":"a".repeat(64)});
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

    fn compressed_release(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (name, bytes) in members {
            let mut header = tar::Header::new_ustar();
            header.set_size(u64::try_from(bytes.len()).unwrap());
            header.set_mode(0o600);
            header.set_cksum();
            archive
                .append_data(&mut header, name, bytes.as_slice())
                .unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    async fn verify_with_unavailable_sdk(bytes: &[u8]) -> Result<Value> {
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("release.tar.gz");
        fs::write(&archive, bytes).unwrap();
        let supervisor = process::Supervisor::new();
        let result = verify(
            VerifyFastlyArchiveRequest {
                sdk_root: root.path().join("unavailable-sdk"),
                archive,
                archive_sha256: io::sha256(bytes),
                source_revision: "a".repeat(40),
                destination: root.path().join("verified"),
            },
            &supervisor,
        )
        .await;
        let cleanup = supervisor.cleanup().await;
        assert_eq!(cleanup, Ok(()));
        assert!(!root.path().join("verified").exists());
        result
    }

    #[tokio::test]
    async fn archive_preflight_rejects_expanded_metadata_before_sdk_lookup() {
        for index in [0_usize, 3, 4] {
            let mut members = small_release_members();
            members[index].1.resize(0x0001_0001, b' ');
            let archive = compressed_release(&members);
            assert!(archive.len() < 4096);
            assert_eq!(
                verify_with_unavailable_sdk(&archive).await.unwrap_err(),
                HarnessError::new("release_invalid"),
                "{}",
                members[index].0
            );
        }
    }

    #[tokio::test]
    async fn archive_preflight_accepts_metadata_at_the_exact_expanded_cap() {
        let mut members = small_release_members();
        for index in [0_usize, 3, 4] {
            members[index].1.resize(0x0001_0000, b' ');
        }
        assert_eq!(
            verify_with_unavailable_sdk(&compressed_release(&members))
                .await
                .unwrap_err(),
            HarnessError::new("sdk_identity")
        );
    }

    #[tokio::test]
    async fn archive_preflight_rejects_extra_files_before_sdk_lookup() {
        let mut members = small_release_members();
        members.push(("extra", Vec::new()));
        assert_eq!(
            verify_with_unavailable_sdk(&compressed_release(&members))
                .await
                .unwrap_err(),
            HarnessError::new("release_invalid")
        );
    }

    #[tokio::test]
    async fn archive_preflight_rejects_duplicate_members_before_sdk_lookup() {
        let mut members = small_release_members();
        members[4].0 = "edgezero.toml";
        assert_eq!(
            verify_with_unavailable_sdk(&compressed_release(&members))
                .await
                .unwrap_err(),
            HarnessError::new("release_invalid")
        );
    }

    #[tokio::test]
    async fn archive_preflight_rejects_nonregular_members_before_sdk_lookup() {
        let mut header = tar::Header::new_ustar();
        header.set_path("auctioneer-cli").unwrap();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_link_name("SECRET_OUTSIDE").unwrap();
        header.set_size(0);
        header.set_mode(0o600);
        header.set_cksum();
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        archive.append(&header, empty()).unwrap();
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        assert_eq!(
            verify_with_unavailable_sdk(&bytes).await.unwrap_err(),
            HarnessError::new("release_invalid")
        );
    }

    #[tokio::test]
    async fn archive_preflight_rejects_pax_structural_overrides_before_sdk_lookup() {
        let mut results = Vec::new();
        for key in ["path", "size", "mtime"] {
            let members = small_release_members();
            let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
            for (index, (name, bytes)) in members.iter().enumerate() {
                if index == 0 {
                    let value = if key == "path" {
                        (*name).to_owned()
                    } else {
                        bytes.len().to_string()
                    };
                    // Even an override that repeats the header value is refused:
                    // the parser boundary never permits structural PAX fields.
                    archive
                        .append_pax_extensions([(key, value.as_bytes())])
                        .unwrap();
                }
                let mut header = tar::Header::new_ustar();
                header.set_size(u64::try_from(bytes.len()).unwrap());
                header.set_mode(0o600);
                header.set_cksum();
                archive
                    .append_data(&mut header, name, bytes.as_slice())
                    .unwrap();
            }
            let bytes = archive.into_inner().unwrap().finish().unwrap();
            results.push((key, verify_with_unavailable_sdk(&bytes).await.unwrap_err()));
        }
        assert_eq!(
            results,
            vec![
                ("path", HarnessError::new("release_invalid")),
                ("size", HarnessError::new("release_invalid")),
                ("mtime", HarnessError::new("sdk_identity"))
            ]
        );
    }

    #[test]
    fn verifier_destinations_are_new_temporary_children_not_repository_paths() {
        let temporary = tempfile::tempdir().unwrap();
        destination_path(&temporary.path().join("verified")).unwrap();
        destination_path(temporary.path()).unwrap_err();
        destination_path(Path::new("relative/verified")).unwrap_err();
        destination_path(&preparation::repository_root().join("verified")).unwrap_err();
        fs::write(temporary.path().join("existing"), b"x").unwrap();
        destination_path(&temporary.path().join("existing")).unwrap_err();
        #[cfg(unix)]
        {
            symlink("missing", temporary.path().join("link")).unwrap();
            destination_path(&temporary.path().join("link")).unwrap_err();
        }
    }

    #[tokio::test]
    async fn pinned_sdk_verifies_an_actual_complete_archive_without_provider_credentials() {
        let supervisor = process::Supervisor::new();
        let sdk_root = checkout(&supervisor).await;
        let (_source, _work, bundle, plan, _build, _metadata, _attestation) =
            artifacts::tests::release(&supervisor).await;
        let output = tempfile::tempdir().unwrap();
        let archive = output.path().join("release.tar.gz");
        let mut command = process::ProcessSpec::new("/usr/bin/tar");
        command.args = [
            OsString::from("--no-xattrs"),
            OsString::from("--no-acls"),
            OsString::from("-czf"),
            archive.as_os_str().to_owned(),
            OsString::from("-C"),
            bundle.path().as_os_str().to_owned(),
            OsString::from("release.json"),
            OsString::from("auctioneer-cli"),
            OsString::from("package.tar.gz"),
            OsString::from("edgezero.toml"),
            OsString::from("fastly.toml"),
        ]
        .into();
        command.env = preparation::tool_environment();
        command.env.insert("COPYFILE_DISABLE".into(), "1".into());
        supervisor.run(command).await.unwrap();
        let digest = io::file_digest(&archive, preparation::FILE_CAP).unwrap();
        archive::preflight(
            &fs::read(&archive).unwrap(),
            &plan.auctioneer_commit,
            &supervisor,
        )
        .await
        .unwrap();
        let input = output.path().join("input.json");
        let request = json!({"sdk_root":sdk_root,"archive":archive,"archive_sha256":digest,"source_revision":plan.auctioneer_commit,"destination":output.path().join("verified")});
        io::write_json(&input, &request, preparation::INPUT_CAP).unwrap();
        let result = verify_fastly_archive(&input, &supervisor).await.unwrap();
        assert_eq!(result["sdk_verification"], "verified_offline");
        assert_eq!(result["edgezero_pin"], SDK_PIN);
        assert_eq!(result["archive_sha256"], digest);
        let mut bad = request;
        bad["archive_sha256"] = json!("b".repeat(64));
        bad["destination"] = json!(output.path().join("bad"));
        fs::write(&input, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert_eq!(
            verify_fastly_archive(&input, &supervisor)
                .await
                .unwrap_err(),
            HarnessError::new("release_invalid")
        );
        supervisor.cleanup().await.unwrap();
    }

    async fn replacement_checkout(supervisor: &process::Supervisor, root: &Path) {
        let source = checkout(supervisor).await;
        let mut clone = process::ProcessSpec::new("/usr/bin/git");
        clone.args = [
            OsString::from("clone"),
            OsString::from("--local"),
            OsString::from("--shared"),
            OsString::from("--no-checkout"),
            OsString::from("--template="),
            source.as_os_str().to_owned(),
            root.as_os_str().to_owned(),
        ]
        .into();
        clone.env = preparation::tool_environment();
        supervisor.run(clone).await.unwrap();
        preparation::git(
            supervisor,
            root,
            &["update-ref", "--no-deref", "HEAD", SDK_PIN],
            preparation::INPUT_CAP,
        )
        .await
        .unwrap();
        preparation::git(
            supervisor,
            root,
            &["read-tree", SDK_PIN],
            preparation::INPUT_CAP,
        )
        .await
        .unwrap();
        preparation::git(
            supervisor,
            root,
            &["checkout-index", "-f", "--", VERIFIER, COMMON],
            preparation::INPUT_CAP,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn replacement_refs_cannot_substitute_scripts_under_the_pinned_revision() {
        let supervisor = process::Supervisor::new();
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("checkout");
        replacement_checkout(&supervisor, &root).await;
        let substitute = b"#!/bin/bash\nexit 0\n";
        fs::write(root.join(VERIFIER), substitute).unwrap();
        let blob = preparation::git(
            &supervisor,
            &root,
            &["hash-object", "-w", VERIFIER],
            preparation::INPUT_CAP,
        )
        .await
        .unwrap();
        preparation::git(
            &supervisor,
            &root,
            &[
                "update-index",
                "--cacheinfo",
                "100644",
                from_utf8(&blob).unwrap().trim(),
                VERIFIER,
            ],
            preparation::INPUT_CAP,
        )
        .await
        .unwrap();
        let tree = preparation::git(&supervisor, &root, &["write-tree"], preparation::INPUT_CAP)
            .await
            .unwrap();
        let commit = preparation::git(
            &supervisor,
            &root,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit-tree",
                from_utf8(&tree).unwrap().trim(),
                "-p",
                SDK_PIN,
                "-m",
                "substitution control",
            ],
            preparation::INPUT_CAP,
        )
        .await
        .unwrap();
        preparation::git(
            &supervisor,
            &root,
            &[
                "update-ref",
                &format!("refs/replace/{SDK_PIN}"),
                from_utf8(&commit).unwrap().trim(),
            ],
            preparation::INPUT_CAP,
        )
        .await
        .unwrap();
        let mut unsafe_read = process::ProcessSpec::new("/usr/bin/git");
        unsafe_read.args = [
            OsString::from("-C"),
            root.as_os_str().to_owned(),
            OsString::from("show"),
            OsString::from(format!("{SDK_PIN}:{VERIFIER}")),
        ]
        .into();
        unsafe_read.env = preparation::tool_environment();
        assert_eq!(
            supervisor.run(unsafe_read).await.unwrap().stdout,
            substitute
        );
        assert_eq!(
            preparation::git(
                &supervisor,
                &root,
                &["rev-parse", "HEAD"],
                preparation::INPUT_CAP
            )
            .await
            .unwrap(),
            format!("{SDK_PIN}\n").as_bytes()
        );
        assert_eq!(
            pinned_sdk(&root, &supervisor).await.err(),
            Some(HarnessError::new("sdk_identity"))
        );
        supervisor.cleanup().await.unwrap();
    }
}

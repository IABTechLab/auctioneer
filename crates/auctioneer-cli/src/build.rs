//! Capture build identity without parsing application configuration.

#[cfg(feature = "application")]
mod resource_identity;

#[cfg(feature = "application")]
use resource_identity::git_metadata;

#[cfg(feature = "application")]
use std::{collections::BTreeSet, env, fs, io, path::PathBuf, process::Command, str::from_utf8};

#[cfg(feature = "application")]
fn main() -> io::Result<()> {
    track_shared_inputs();
    application_identity()
}

#[cfg(not(feature = "application"))]
fn main() {
    track_shared_inputs();
}

fn track_shared_inputs() {
    println!("cargo:rerun-if-changed=src/build.rs");
    println!("cargo:rerun-if-changed=src/resource_identity");
}

#[cfg(feature = "application")]
fn application_identity() -> io::Result<()> {
    let root = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| io::Error::other("source_identity_root"))?,
    )
    .join("../..");
    for path in git_metadata::watch_paths(&root)? {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let snapshot = resource_identity::snapshot(&root)?;
    let mut directories = BTreeSet::new();
    directories.insert(root.join("docs"));
    for path in snapshot.files.keys() {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
        let relative = PathBuf::from(path);
        if relative.components().count() > 1
            && let Some(first) = relative.components().next()
        {
            directories.insert(root.join(first.as_os_str()));
        }
    }
    for directory in directories {
        println!("cargo:rerun-if-changed={}", directory.display());
    }
    let rustc =
        env::var_os("RUSTC").ok_or_else(|| io::Error::other("compiler_identity_missing"))?;
    let compiler = Command::new(rustc)
        .args(["--version", "--verbose"])
        .output()?;
    if !compiler.status.success() || compiler.stdout.len() > 8 << 10_usize {
        return Err(io::Error::other("compiler_identity_invalid"));
    }
    let compiler_verbose = from_utf8(&compiler.stdout)
        .map_err(io::Error::other)?
        .trim()
        .to_owned();
    let committed = Command::new("git")
        .current_dir(&root)
        .args(["--no-replace-objects", "rev-parse", "--verify", "HEAD"])
        .output()?;
    let code_base_commit = from_utf8(&committed.stdout)
        .map_err(io::Error::other)?
        .trim()
        .to_owned();
    if !committed.status.success()
        || code_base_commit.len() != 40
        || !code_base_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(io::Error::other("source_identity_commit"));
    }
    let status = Command::new("git")
        .current_dir(&root)
        .args([
            "--no-replace-objects",
            "--no-optional-locks",
            "--literal-pathspecs",
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
        ])
        .args(snapshot.files.keys())
        .output()?;
    if !status.status.success() {
        return Err(io::Error::other("source_identity_status"));
    }
    let identity = resource_identity::BuildIdentity {
        snapshot,
        compiler_verbose,
        compiled_target: env::var("TARGET").map_err(io::Error::other)?,
        code_base_commit,
        source_tree_matches_commit: status.stdout.is_empty(),
    };
    let destination = PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("source_identity_output"))?,
    )
    .join("resource-identity.json");
    fs::write(
        destination,
        serde_json::to_vec(&identity).map_err(io::Error::other)?,
    )
}

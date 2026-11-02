//! Cargo inputs for the Git identity embedded by the application build script.

use std::collections::BTreeSet;
use std::env::var_os;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::str::from_utf8;

fn git(root: &Path, args: &[&str]) -> io::Result<Output> {
    let output = Command::new("git")
        .current_dir(root)
        .env_clear()
        .env("PATH", var_os("PATH").unwrap_or_default())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(["--no-replace-objects", "--no-optional-locks"])
        .args(args)
        .output()?;
    if output.stdout.len() > 8192 {
        return Err(io::Error::other("git_identity_metadata_limit"));
    }
    Ok(output)
}

fn line(output: &Output) -> io::Result<&str> {
    let value = from_utf8(&output.stdout)
        .map_err(io::Error::other)?
        .strip_suffix('\n')
        .ok_or_else(|| io::Error::other("git_identity_metadata_line"))?;
    if value.is_empty() || value.chars().any(char::is_control) {
        return Err(io::Error::other("git_identity_metadata_line"));
    }
    Ok(value)
}

fn git_path(root: &Path, name: &str) -> io::Result<PathBuf> {
    let output = git(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", name],
    )?;
    if !output.status.success() {
        return Err(io::Error::other("git_identity_metadata_path"));
    }
    let path = PathBuf::from(line(&output)?);
    if !path.is_absolute() {
        return Err(io::Error::other("git_identity_metadata_path"));
    }
    Ok(path)
}

/// Git metadata paths whose changes invalidate an embedded build identity.
///
/// # Errors
/// Returns an error for invalid Git metadata, paths or references.
pub fn watch_paths(root: &Path) -> io::Result<BTreeSet<PathBuf>> {
    let mut paths = BTreeSet::new();
    for name in ["HEAD", "index", "packed-refs"] {
        let path = git_path(root, name)?;
        if path.exists() {
            paths.insert(path);
        }
    }
    let branch = git(root, &["symbolic-ref", "--quiet", "HEAD"])?;
    match branch.status.code() {
        Some(0_i32) => {
            let name = line(&branch)?;
            if !name.starts_with("refs/")
                || name.split('/').any(|part| matches!(part, "" | "." | ".."))
            {
                return Err(io::Error::other("git_identity_metadata_ref"));
            }
            let reference = git_path(root, name)?;
            if reference.exists() {
                paths.insert(reference.clone());
            }
            // Watch the existing ref directory too: packing removes the loose ref,
            // and the next commit can recreate it beside the packed-ref file.
            let mut parent = reference
                .parent()
                .ok_or_else(|| io::Error::other("git_identity_metadata_ref"))?;
            while !parent.exists() {
                parent = parent
                    .parent()
                    .ok_or_else(|| io::Error::other("git_identity_metadata_ref"))?;
            }
            paths.insert(parent.to_owned());
        }
        Some(1_i32) => {}
        Some(_) | None => return Err(io::Error::other("git_identity_metadata_ref")),
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn command(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .env_clear()
            .env("PATH", var_os("PATH").unwrap())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "-c",
                "user.name=Identity Test",
                "-c",
                "user.email=identity@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    #[test]
    fn provenance_watch_paths_cover_commit_staging_and_packed_ref_changes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        command(&root, &["init", "--quiet", "--initial-branch", "main"]);
        command(
            &root,
            &["commit", "--quiet", "--allow-empty", "-m", "baseline"],
        );
        let paths = watch_paths(&root).unwrap();
        for path in ["HEAD", "index", "refs/heads/main", "refs/heads"] {
            assert!(
                paths.contains(&root.join(".git").join(path)),
                "missing {path}: {paths:?}"
            );
        }
        assert!(
            paths.iter().all(|path| path.exists()),
            "do not watch nonexistent files on every build"
        );
        command(&root, &["pack-refs", "--all"]);
        let packed_paths = watch_paths(&root).unwrap();
        assert!(packed_paths.contains(&root.join(".git/packed-refs")));
        assert!(packed_paths.contains(&root.join(".git/refs/heads")));
        assert!(packed_paths.iter().all(|path| path.exists()));
        command(&root, &["checkout", "--quiet", "--detach"]);
        assert!(
            watch_paths(&root)
                .unwrap()
                .contains(&root.join(".git/HEAD"))
        );
    }

    #[test]
    fn provenance_watch_paths_resolve_linked_worktree_metadata_not_its_git_pointer_file() {
        let directory = tempfile::tempdir().unwrap();
        let base = directory.path().canonicalize().unwrap();
        let root = base.join("primary");
        let linked = base.join("linked");
        fs::create_dir_all(&root).unwrap();
        command(&root, &["init", "--quiet", "--initial-branch", "main"]);
        command(
            &root,
            &["commit", "--quiet", "--allow-empty", "-m", "baseline"],
        );
        command(
            &root,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                "linked",
                linked.to_str().unwrap(),
            ],
        );
        let paths = watch_paths(&linked).unwrap();
        for path in [
            "worktrees/linked/HEAD",
            "worktrees/linked/index",
            "refs/heads/linked",
            "refs/heads",
        ] {
            assert!(
                paths.contains(&root.join(".git").join(path)),
                "missing {path}: {paths:?}"
            );
        }
        assert!(!paths.contains(&linked.join(".git")));
    }
}

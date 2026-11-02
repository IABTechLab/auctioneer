//! Bounded, fixed-membership local artifacts. Preflights do not authenticate
//! paths against concurrent replacement by a hostile process.

use super::{CAP, Error, Failure};
use auctioneer_cli::native_control::io::write_json;
use serde::Serialize;
use std::collections::BTreeSet;
use std::env::current_dir;
use std::fs::{self, symlink_metadata};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

fn safe(path: &Path, fresh: bool) -> Result<PathBuf, Error> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        current_dir().map_err(|_error| Failure::Input)?.join(path)
    };
    if absolute
        .components()
        .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(Failure::Input.into());
    }
    for ancestor in absolute.ancestors() {
        match symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || (ancestor != absolute && !metadata.is_dir())
                {
                    return Err(Failure::Input.into());
                }
            }
            Err(error) if fresh && ancestor == absolute && error.kind() == ErrorKind::NotFound => {}
            Err(_error) => return Err(Failure::Input.into()),
        }
    }
    Ok(absolute)
}

pub(super) fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let resolved = safe(path, false)?;
    super::check::read_input(&resolved).map_err(|_error| Failure::Input.into())
}

#[expect(
    clippy::create_dir,
    reason = "exclusive run and producer claims must refuse an existing directory"
)]
pub(super) fn fresh_dir(path: &Path) -> Result<(), Error> {
    let resolved = safe(path, true)?;
    fs::create_dir(resolved).map_err(|_error| Failure::Output.into())
}

pub(super) fn publish<T: Serialize>(path: &Path, value: &T) -> Result<(), Error> {
    let resolved = safe(path, true)?;
    write_json(&resolved, value, CAP).map_err(|_error| Failure::Output.into())
}

pub(super) fn members(path: &Path, names: &[&str]) -> Result<(), Error> {
    subset(path, names, names)
}

pub(super) fn subset(path: &Path, allowed: &[&str], required: &[&str]) -> Result<(), Error> {
    let resolved = safe(path, false)?;
    let expected: BTreeSet<_> = allowed.iter().map(|name| (*name).to_owned()).collect();
    let mut actual = BTreeSet::new();
    for result in fs::read_dir(resolved).map_err(|_error| Failure::Membership)? {
        let entry = result.map_err(|_error| Failure::Membership)?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_error| Failure::Membership)?;
        if !expected.contains(&name) || !actual.insert(name) {
            return Err(Failure::Membership.into());
        }
        if entry
            .file_type()
            .map_err(|_error| Failure::Input)?
            .is_symlink()
        {
            return Err(Failure::Input.into());
        }
    }
    if required.iter().any(|name| !actual.contains(*name)) {
        return Err(Failure::Membership.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use tempfile::tempdir_in;

    #[test]
    fn distributed_members_require_the_named_slot_without_allowing_unknown_members() {
        let temporary = tempdir_in(temp_dir().canonicalize().unwrap()).unwrap();
        let root = temporary.path();
        fs::write(root.join("manifest.json"), "{}").unwrap();
        let allowed = ["manifest.json", "first", "second"];
        subset(root, &allowed, &["manifest.json"]).unwrap();
        assert_eq!(
            subset(root, &allowed, &["manifest.json", "first"]),
            Err(Failure::Membership.into())
        );
        fs::create_dir_all(root.join("second")).unwrap();
        subset(root, &allowed, &["manifest.json", "second"]).unwrap();
        fs::create_dir_all(root.join("first")).unwrap();
        subset(root, &allowed, &["manifest.json", "first"]).unwrap();
        members(root, &allowed).unwrap();
        fs::write(root.join("unknown"), "{}").unwrap();
        assert_eq!(
            subset(root, &allowed, &["manifest.json"]),
            Err(Failure::Membership.into())
        );
    }
}

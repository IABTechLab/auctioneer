//! Inputs for shared synthetic controls and the separate source-conformance target.

#![cfg(test)]

use crate::schema_qualification::{Inventory, fixture, inventory};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::str;
use std::sync::LazyLock;

pub(crate) struct Inputs {
    pub snapshot: Inventory,
    pub fixture: Option<fixture::Fixture>,
}

pub(crate) fn inputs() -> Inputs {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        Inputs {
            snapshot: inventory(
                &root.join("src/bidders"),
                &root.join("tests/conformance/fixtures/reference/catalog/source-inventory.toml"),
            )
            .unwrap(),
            fixture: None,
        }
    } else {
        let fixture = fixture::Fixture::new().unwrap();
        Inputs {
            snapshot: fixture::inventory(&fixture.bidders, &fixture.source).unwrap(),
            fixture: Some(fixture),
        }
    }
}

pub(crate) const fn schema_count() -> usize {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        261
    } else {
        fixture::SCHEMAS
    }
}
pub(crate) const fn helper_count() -> usize {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        23
    } else {
        2
    }
}
pub(crate) const fn sampled_count() -> usize {
    if crate::SCHEMA_SOURCE_CONFORMANCE {
        323
    } else {
        fixture::SCHEMAS + 9
    }
}

pub(crate) fn declarations(snapshot: &Inventory, filename: &str) -> BTreeMap<String, Value> {
    let mut declarations = BTreeMap::new();
    let owners: BTreeSet<_> = snapshot
        .schemas
        .iter()
        .filter_map(|row| row.witness.path.split('/').next())
        .collect();
    for owner in owners {
        let path = snapshot
            .root
            .join(owner)
            .join("tests/schema")
            .join(filename);
        if !path.try_exists().unwrap() {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert!(metadata.len() <= 64 * 1024);
        let bytes = fs::read(path).unwrap();
        assert!(bytes.len() <= 64 * 1024);
        let data: Value = if Path::new(filename)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        {
            serde_json::to_value(
                toml::from_str::<toml::Value>(str::from_utf8(&bytes).unwrap()).unwrap(),
            )
            .unwrap()
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        let name = data["schema"].as_str().unwrap().to_owned();
        assert!(
            snapshot
                .schemas
                .iter()
                .any(|row| row.witness.name == name
                    && row.witness.path.split('/').next() == Some(owner)),
            "case ownership"
        );
        assert!(declarations.insert(name, data).is_none());
    }
    declarations
}

pub(crate) fn workloads() -> &'static BTreeMap<String, Value> {
    static WORKLOADS: LazyLock<BTreeMap<String, Value>> = LazyLock::new(|| {
        let workloads = declarations(&inputs().snapshot, "workload.toml");
        assert_eq!(
            workloads.len(),
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                11
            } else {
                2
            }
        );
        for (flag, real, fake) in [
            ("maximum_string", 7, 1),
            ("maximum_collections", 3, 0),
            ("string_refusals", 5, 1),
            ("pattern_initialization", 7, 1),
            ("retained_constructor", 3, 1),
            ("fractional_allocation", 1, 0),
        ] {
            assert_eq!(
                workloads.values().filter(|data| data[flag] == true).count(),
                if crate::SCHEMA_SOURCE_CONFORMANCE {
                    real
                } else {
                    fake
                },
                "nonvacuous workload capability: {flag}"
            );
        }
        workloads
    });
    &WORKLOADS
}

pub(crate) fn helpers() -> &'static BTreeMap<String, Value> {
    static HELPERS: LazyLock<BTreeMap<String, Value>> = LazyLock::new(|| {
        let mut result = BTreeMap::new();
        for (name, data) in declarations(&inputs().snapshot, "helpers.toml") {
            for helper in data["helpers"].as_array().unwrap() {
                let key = format!(
                    "{name}:__JSONSCHEMA_REGEX_{}_REGEX",
                    helper["index"].as_u64().unwrap()
                );
                assert!(result.insert(key, helper.clone()).is_none());
            }
        }
        assert_eq!(result.len(), helper_count());
        result
    });
    &HELPERS
}

#[cfg(test)]
mod tests {
    use super::{
        declarations, fixture, helper_count, helpers, inputs, sampled_count, schema_count,
        workloads,
    };
    use std::fs;
    use std::panic::catch_unwind;

    #[test]
    fn selected_profile_counts_match_observed_schema_helper_and_callback_populations() {
        let selected = inputs();
        assert_eq!(selected.snapshot.schemas.len(), schema_count());
        assert_eq!(helpers().len(), helper_count());
        let callbacks = workloads()
            .values()
            .map(|data| data["rows"].as_array().unwrap().len())
            .sum::<usize>();
        assert_eq!(
            sampled_count(),
            selected
                .snapshot
                .schemas
                .len()
                .checked_add(callbacks)
                .unwrap()
        );
        assert_eq!(
            selected.fixture.is_some(),
            !crate::SCHEMA_SOURCE_CONFORMANCE
        );
    }

    #[test]
    fn adjacent_cases_are_discovered_after_add_edit_and_remove_without_a_roster() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        assert_eq!(declarations(&snapshot, "workload.toml").len(), 2);
        let directory = inputs.bidders.join("fake_secondary/tests/schema");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("workload.toml");
        fs::write(&path, "schema='fake_secondary'\norder=2\nrows=[]\n").unwrap();
        let added = declarations(&snapshot, "workload.toml");
        assert_eq!(added.len(), 3);
        assert_eq!(added["fake_secondary"]["order"], 2_u64);
        fs::write(&path, "schema='fake_secondary'\norder=3\nrows=[]\n").unwrap();
        assert_eq!(
            declarations(&snapshot, "workload.toml")["fake_secondary"]["order"],
            3_u64
        );
        fs::remove_file(path).unwrap();
        assert_eq!(declarations(&snapshot, "workload.toml").len(), 2);
    }

    #[test]
    fn adjacent_case_identity_must_belong_to_its_schema_owner() {
        let inputs = fixture::Fixture::new().unwrap();
        let snapshot = fixture::inventory(&inputs.bidders, &inputs.source).unwrap();
        fs::write(
            inputs.bidders.join("fake_base/tests/schema/workload.toml"),
            "schema='fake_secondary'\norder=0\nrows=[]\n",
        )
        .unwrap();
        catch_unwind(|| declarations(&snapshot, "workload.toml")).unwrap_err();
    }
}

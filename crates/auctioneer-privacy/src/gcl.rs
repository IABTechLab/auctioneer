//! Runtime Global CMP List — lowered ONCE from the baked artifact.
//!
//! Single-parse-path rule: the ONLY JSON parse of the GCL at runtime
//! goes through `auctioneer_gvl_schema::validate_gcl_json`, the same
//! validator the build gate ran. No independent serde call.

use std::collections::BTreeMap;
use std::sync::OnceLock;
use time::OffsetDateTime;

// Module scope, so `is_baked()` can observe it.
static BAKED: OnceLock<Gcl> = OnceLock::new();

pub struct CmpRecord {
    pub id: u16,
    pub deleted_date: Option<OffsetDateTime>,
}

pub struct Gcl {
    pub cmps: BTreeMap<u16, CmpRecord>,
    pub expires_at: OffsetDateTime,
}

impl Gcl {
    /// Boot/baked constructor: lowers `auctioneer_config_data::baked_gcl_json()`
    /// once into a process-static. Called at boot alongside the
    /// host-primitives install, so its heap sits inside the measured
    /// baseline (no separate budget term).
    pub fn baked() -> &'static Gcl {
        BAKED.get_or_init(|| {
            let expires_at = OffsetDateTime::from_unix_timestamp(
                auctioneer_config_data::gcl_expires_at_unix_secs(),
            )
            // Fail-closed: an unconvertible expiry reads as already
            // stale, never as never-expiring.
            .unwrap_or(OffsetDateTime::UNIX_EPOCH);
            match auctioneer_gvl_schema::validate_gcl_json(auctioneer_config_data::baked_gcl_json())
            {
                Ok(validated) => Gcl {
                    cmps: validated
                        .cmps
                        .into_iter()
                        .map(|(id, cmp)| {
                            (
                                id,
                                CmpRecord {
                                    id,
                                    deleted_date: cmp.deleted_date,
                                },
                            )
                        })
                        .collect(),
                    expires_at,
                },
                // Unreachable for a build-gate-validated artifact, but
                // defined anyway: an EMPTY list with a PAST expiry —
                // every TCF request then takes the stale-GCL
                // fail-closed path and emits its warning, which is the
                // observable signal; non-TCF traffic is unaffected, and
                // this crate stays log-free on this path (the
                // silent-runtime rule).
                Err(_) => Gcl {
                    cmps: BTreeMap::new(),
                    expires_at: OffsetDateTime::UNIX_EPOCH,
                },
            }
        })
    }

    /// Boot-order probe — true once `baked()` has initialized the
    /// static. Used by the adapter-bootstrap sequence test (a plain pub
    /// fn: cross-crate tests cannot see `#[cfg(test)]` items).
    #[must_use]
    pub fn is_baked() -> bool {
        BAKED.get().is_some()
    }

    /// Fixture constructor — direct fields, no JSON.
    ///
    /// Deliberately NOT `#[cfg(test)]`: a `cfg(test)` item is invisible
    /// to every other crate, so downstream handler-path tests (which
    /// need a GCL whose CMP set or expiry differs from the baked one)
    /// could not build a fixture at all.
    #[must_use]
    pub fn for_test(cmps: Vec<CmpRecord>, expires_at: OffsetDateTime) -> Gcl {
        Gcl {
            cmps: cmps.into_iter().map(|cmp| (cmp.id, cmp)).collect(),
            expires_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn baked_gcl_lowering_is_fresh_and_nonempty() {
        let gcl = Gcl::baked();
        assert!(!gcl.cmps.is_empty(), "the baked GCL carries CMP records");
        assert!(
            gcl.expires_at > OffsetDateTime::UNIX_EPOCH,
            "the baked expiry converts to a real instant"
        );
        assert!(Gcl::is_baked());
        // A known official CMP id (Usercentrics, id 5) is present.
        assert!(gcl.cmps.contains_key(&5));
    }

    #[test]
    fn gcl_for_test_maps_ids_and_deleted_dates() {
        let deleted_at = datetime!(2024-11-13 14:55:31 UTC);
        let gcl = Gcl::for_test(
            vec![
                CmpRecord {
                    id: 2,
                    deleted_date: None,
                },
                CmpRecord {
                    id: 3,
                    deleted_date: Some(deleted_at),
                },
            ],
            datetime!(2026-09-03 16:00:00 UTC),
        );
        assert!(
            gcl.cmps
                .get(&2)
                .is_some_and(|cmp| cmp.deleted_date.is_none())
        );
        assert_eq!(
            gcl.cmps.get(&3).and_then(|cmp| cmp.deleted_date),
            Some(deleted_at)
        );
        assert_eq!(gcl.expires_at, datetime!(2026-09-03 16:00:00 UTC));
    }
}

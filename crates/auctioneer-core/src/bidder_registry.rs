//! Shared bidder-registry access.
//!
//! Both the auction handler and the runtime-config helper need the
//! registry — the handler to run bidders, the config helper to check
//! that a runtime `[aliases]` overlay only references base bidders this
//! build actually carries. Config-layer code must not depend on
//! handler-layer symbols, so the singleton lives here and both sides
//! import it.

use auctioneer_bidder::{BidderName, BidderRegistry, RegistryBuildError};
use auctioneer_config_data::types::BidderInfo;
#[cfg(test)]
use std::cell::Cell;
use std::cmp::Ordering;
use std::sync::OnceLock;

fn lookup_info(infos: &[&'static BidderInfo], name: &str) -> Option<(usize, &'static BidderInfo)> {
    #[cfg(test)]
    let comparisons = Cell::new(0_usize);
    let mut remaining = infos;
    let mut found = None;
    let mut offset = 0_usize;
    while let Some((before, after)) = remaining.split_at_checked(remaining.len() >> 1)
        && let Some((info, tail)) = after.split_first()
    {
        let position = offset.checked_add(before.len())?;
        #[cfg(test)]
        comparisons.set(comparisons.get().saturating_add(1));
        match info.name.as_str().cmp(name) {
            Ordering::Less => {
                remaining = tail;
                offset = position.checked_add(1)?;
            }
            Ordering::Greater => remaining = before,
            Ordering::Equal => {
                found = Some((position, *info));
                break;
            }
        }
    }
    #[cfg(test)]
    info_work::record_probe(comparisons.get());
    found
}

/// The existing resident metadata vector, ordered once before route reuse.
pub(crate) struct BidderInfoIndex {
    infos: Vec<&'static BidderInfo>,
}

impl BidderInfoIndex {
    pub(crate) fn new(source: impl Iterator<Item = &'static BidderInfo>) -> Self {
        let mut infos = source.collect::<Vec<_>>();
        infos.sort_unstable_by(|left, right| left.name.cmp(&right.name));
        Self { infos }
    }

    pub(crate) fn infos(&self) -> &[&'static BidderInfo] {
        &self.infos
    }

    pub(crate) fn get(&self, name: &str) -> Option<&'static BidderInfo> {
        self.find(name).map(|(_, info)| info)
    }

    pub(crate) fn find(&self, name: &str) -> Option<(usize, &'static BidderInfo)> {
        lookup_info(&self.infos, name)
    }
}

pub(crate) fn info_index() -> Result<&'static BidderInfoIndex, RegistryBuildError> {
    static INFOS: OnceLock<BidderInfoIndex> = OnceLock::new();
    let registry = registry_handle()?;
    Ok(INFOS.get_or_init(|| BidderInfoIndex::new(registry.infos())))
}

#[cfg(test)]
pub(crate) mod info_work {
    use std::cell::RefCell;
    use std::mem;

    #[derive(Default)]
    pub(crate) struct Counts {
        pub probes: Vec<usize>,
        pub rendering: usize,
    }

    thread_local! {
        static COUNTS: RefCell<Counts> = const { RefCell::new(Counts { probes: Vec::new(), rendering: 0 }) };
    }

    pub(crate) fn record_probe(comparisons: usize) {
        COUNTS.with_borrow_mut(|counts| counts.probes.push(comparisons));
    }

    pub(crate) fn record_rendering() {
        COUNTS.with_borrow_mut(|counts| counts.rendering = counts.rendering.saturating_add(1));
    }

    pub(crate) fn take() -> Counts {
        COUNTS.with_borrow_mut(mem::take)
    }
}

static REGISTRY: OnceLock<Result<BidderRegistry, RegistryBuildError>> = OnceLock::new();

pub(crate) fn registry_handle() -> Result<&'static BidderRegistry, RegistryBuildError> {
    cached_registry(&REGISTRY, auctioneer_bidder::configured_registry)
}

/// Inspect an established fatal failure without constructing an unfunded registry.
pub(crate) fn cached_registry_error() -> Option<RegistryBuildError> {
    cached_error(&REGISTRY)
}

fn cached_error(
    cell: &OnceLock<Result<BidderRegistry, RegistryBuildError>>,
) -> Option<RegistryBuildError> {
    cell.get()?.as_ref().err().copied()
}

fn cached_registry(
    cell: &OnceLock<Result<BidderRegistry, RegistryBuildError>>,
    build: impl FnOnce() -> Result<BidderRegistry, RegistryBuildError>,
) -> Result<&BidderRegistry, RegistryBuildError> {
    cell.get_or_init(build).as_ref().map_err(|error| *error)
}

/// Every base bidder name this build's registry carries. A disabled
/// bidder is still a valid alias base — disabling is a runtime overlay
/// decision, while this list is the compile-time roster.
pub(crate) fn live_name_slice() -> Result<&'static [BidderName], RegistryBuildError> {
    static NAMES: OnceLock<Vec<BidderName>> = OnceLock::new();
    let registry = registry_handle()?;
    Ok(NAMES.get_or_init(|| registry.base_names()))
}

#[cfg(test)]
mod tests {
    use super::{BidderInfo, registry_handle};
    use std::{cell::Cell, ptr, sync::OnceLock};

    #[test]
    fn info_metadata_index_reuses_the_existing_resident_vector() {
        let index = super::info_index().unwrap();
        assert!(ptr::eq(index, super::info_index().unwrap()));
        assert_eq!(
            size_of::<super::BidderInfoIndex>(),
            size_of::<Vec<&BidderInfo>>()
        );
        assert_eq!(
            index.infos().len(),
            registry_handle().unwrap().infos().len()
        );
        assert!(
            index
                .infos()
                .windows(2)
                .all(|pair| matches!(pair, [left, right] if left.name < right.name))
        );
        for info in registry_handle().unwrap().infos() {
            assert!(ptr::eq(index.get(&info.name).unwrap(), info));
        }
    }

    #[test]
    fn initialization_caches_failure_without_retrying_the_builder() {
        let cell = OnceLock::new();
        let calls = Cell::new(0_u32);
        let error = auctioneer_bidder::RegistryBuildError::MissingBinding {
            bidder: "fake_missing",
        };
        assert_eq!(super::cached_error(&cell), None);
        assert!(
            cell.get().is_none(),
            "inspection must not initialize the cache"
        );
        for _ in 0_usize..2 {
            let result = super::cached_registry(&cell, || {
                calls.set(calls.get() + 1);
                Err(error)
            });
            assert!(matches!(result, Err(actual) if actual == error));
            assert_eq!(super::cached_error(&cell), Some(error));
        }
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn initialization_caches_success_without_reconstructing() {
        let cell = OnceLock::new();
        let calls = Cell::new(0_u32);
        let mut prior = None;
        for _ in 0_usize..2 {
            let registry = super::cached_registry(&cell, || {
                calls.set(calls.get() + 1);
                Ok(auctioneer_bidder::BidderRegistry::new())
            })
            .unwrap();
            let pointer = ptr::from_ref(registry);
            assert_eq!(super::cached_error(&cell), None);
            if let Some(previous) = prior {
                assert_eq!(pointer, previous);
            }
            prior = Some(pointer);
        }
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn shared_registry_borrows_original_generated_rows() {
        let infos = auctioneer_config_data::bidders();
        let registry = registry_handle().unwrap();
        let repeated = registry_handle().unwrap();
        assert!(
            ptr::eq(registry, repeated),
            "production handles must share one registry"
        );
        assert!(!infos.is_empty());
        assert_eq!(registry.infos().len(), infos.len());

        for info in infos {
            let (registered, _) = registry.get(info.name.as_str()).unwrap();
            assert!(
                ptr::eq(registered, info),
                "registry must borrow the original metadata for {}",
                info.name.as_str()
            );
        }
    }
}

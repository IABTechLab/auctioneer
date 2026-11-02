//! Bidder registry and its compiled-catalog validation phase.

mod catalog;

pub use catalog::{
    RegistryAlias, RegistryBuildError, RegistryDefinition, RegistryInputs, ValidatedRegistryPlan,
};

use crate::{bidder::Bidder, types::BidderName};
use auctioneer_config::types::BidderInfo;
use std::collections::{BTreeMap, btree_map::Entry};

/// Holds `Box<dyn Bidder>` bidder instances paired with their `&'static`
/// `BidderInfo`. `BTreeMap` keyed by `BidderName` so iteration is in
/// lexicographic order automatically.
pub struct BidderRegistry {
    entries: BTreeMap<BidderName, RegistryEntry>,
}

struct RegistryEntry {
    info: &'static BidderInfo,
    bidder: Box<dyn Bidder>,
}

impl BidderRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Insert a public identity without replacing an existing registration.
    ///
    /// # Errors
    /// Returns `DuplicateBidder` and preserves the incumbent on duplicate names.
    pub fn register(
        &mut self,
        info: &'static BidderInfo,
        bidder: Box<dyn Bidder>,
    ) -> Result<(), RegistryBuildError> {
        let name = BidderName::new(info.name.as_str());
        match self.entries.entry(name) {
            Entry::Occupied(_) => Err(RegistryBuildError::DuplicateBidder {
                bidder: info.name.as_str(),
            }),
            Entry::Vacant(entry) => {
                entry.insert(RegistryEntry { info, bidder });
                Ok(())
            }
        }
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<(&'static BidderInfo, &dyn Bidder)> {
        self.entries
            .get(name)
            .map(|entry| (entry.info, entry.bidder.as_ref()))
    }

    /// Borrow the registered metadata in public-name order without copying names.
    #[must_use]
    pub fn infos(&self) -> impl ExactSizeIterator<Item = &'static BidderInfo> {
        self.entries.values().map(|entry| entry.info)
    }

    /// Returns base bidder names in lexicographic order. Used to
    /// construct the participating-set for an auction (combined with
    /// request-defined alias names + host-config aliases at the exchange
    /// boundary).
    #[must_use]
    pub fn base_names(&self) -> Vec<BidderName> {
        self.entries.keys().cloned().collect()
    }
}

impl Default for BidderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{BidderInfo, BidderRegistry};
    use crate::{
        DebugBodySchema,
        bidder::{Bidder, BidderError},
        meta::OutboundRequestMeta,
        request_info::BidderRequestInfo,
        sink::OutboundRequestSink,
        types::{BidderHttpResponse, BidderResponse},
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use std::ptr;

    struct DummyBidder(u32);
    impl Bidder for DummyBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            _r: &BidRequest,
            _i: &BidderRequestInfo<'_>,
            _s: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            Err(vec![BidderError::unknown(format!("{}", self.0))])
        }
        fn make_bids(
            &self,
            _r: &BidRequest,
            _q: &OutboundRequestMeta,
            _p: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            Err(vec![])
        }
    }

    fn leak_info(name: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            ..BidderInfo::default()
        }))
    }

    #[test]
    fn duplicate_insertion_preserves_incumbent_info_and_bidder() {
        let mut registry = BidderRegistry::new();
        let incumbent = leak_info("fake_duplicate");
        let bidder: Box<dyn Bidder> = Box::new(DummyBidder(1));
        let incumbent_bidder = ptr::from_ref(bidder.as_ref());
        registry.register(incumbent, bidder).unwrap();
        assert_eq!(
            registry.register(leak_info("fake_duplicate"), Box::new(DummyBidder(2))),
            Err(super::RegistryBuildError::DuplicateBidder {
                bidder: "fake_duplicate"
            })
        );

        let (info, stored_bidder) = registry.get("fake_duplicate").unwrap();
        assert!(ptr::eq(info, incumbent));
        assert!(ptr::addr_eq(ptr::from_ref(stored_bidder), incumbent_bidder));
        assert_eq!(registry.infos().len(), 1);
    }

    #[test]
    fn base_names_are_lexicographic() {
        let mut registry = BidderRegistry::new();
        registry
            .register(leak_info("fake_zzz"), Box::new(DummyBidder(1)))
            .expect("unique test bidder");
        registry
            .register(leak_info("fake_one"), Box::new(DummyBidder(2)))
            .expect("unique test bidder");
        registry
            .register(leak_info("fake_two"), Box::new(DummyBidder(3)))
            .expect("unique test bidder");
        let names: Vec<String> = registry
            .base_names()
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["fake_one", "fake_two", "fake_zzz"]);
    }

    #[test]
    fn metadata_iteration_borrows_original_rows_in_public_name_order() {
        let mut registry = BidderRegistry::new();
        let last = leak_info("fake_z_last");
        let first = leak_info("fake_a_first");
        registry
            .register(last, Box::new(DummyBidder(1)))
            .expect("unique test bidder");
        registry
            .register(first, Box::new(DummyBidder(2)))
            .expect("unique test bidder");
        let mut infos = registry.infos();
        assert_eq!(infos.len(), 2);
        assert!(ptr::eq(infos.next().unwrap(), first));
        assert_eq!(infos.len(), 1);
        assert!(ptr::eq(infos.next().unwrap(), last));
        assert_eq!(infos.len(), 0);
        assert!(infos.next().is_none());
    }

    #[test]
    fn get_returns_exact_paired_info_and_bidder() {
        let mut registry = BidderRegistry::new();
        let expected = [
            ("fake_ax", 7),
            ("fake_AX", 8),
            ("fake_caf\u{e9}", 9),
            ("fake_cafe\u{301}", 10),
            ("fake_\u{6771}\u{4eac}", 11),
        ]
        .map(|(name, id)| {
            let info = leak_info(name);
            let bidder: Box<dyn Bidder> = Box::new(DummyBidder(id));
            let bidder_ptr = ptr::from_ref(bidder.as_ref());
            registry.register(info, bidder).expect("unique test bidder");
            (info, bidder_ptr)
        });

        for (expected_info, expected_bidder) in expected {
            let (info, bidder) = registry.get(expected_info.name.as_str()).unwrap();
            assert!(ptr::eq(info, expected_info));
            assert!(ptr::addr_eq(ptr::from_ref(bidder), expected_bidder));
        }
    }

    #[test]
    fn get_is_case_sensitive_and_does_not_normalize_unicode() {
        let mut registry = BidderRegistry::new();
        registry
            .register(leak_info("fake_AX"), Box::new(DummyBidder(1)))
            .expect("unique test bidder");
        registry
            .register(leak_info("fake_caf\u{e9}"), Box::new(DummyBidder(2)))
            .expect("unique test bidder");

        for name in ["fake_ax", "fake_Ax", "fake_CAF\u{c9}", "fake_cafe\u{301}"] {
            assert!(registry.get(name).is_none());
        }
    }

    #[test]
    fn unknown_name_returns_none() {
        let empty_registry = BidderRegistry::new();
        assert!(empty_registry.get("fake_nope").is_none());

        let mut registry = BidderRegistry::new();
        registry
            .register(leak_info("fake_ax"), Box::new(DummyBidder(1)))
            .expect("unique test bidder");
        for name in ["", "fake_a", "fake_ax_suffix", "fake_nope"] {
            assert!(registry.get(name).is_none());
        }
    }
}

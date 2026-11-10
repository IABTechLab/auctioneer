//! Synthetic metadata and opt-in capabilities for the shared test fixture.

use crate::BidderRequestInfo;
use auctioneer_config_data::types::BidderInfo;
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::{
    cell::Cell,
    num::NonZeroUsize,
    sync::{LazyLock, atomic::AtomicUsize},
};

thread_local! {
    static FACTORY_CALLS: Cell<usize> = const { Cell::new(0) };
}

/// Thread-local constructor observation, independent of validation calls.
#[must_use]
pub fn factory_calls() -> usize {
    FACTORY_CALLS.with(Cell::get)
}

pub(super) fn record_factory_call() {
    FACTORY_CALLS.with(|calls| calls.set(calls.get().saturating_add(1)));
}

#[must_use]
pub fn info() -> &'static BidderInfo {
    static INFO: LazyLock<BidderInfo> = LazyLock::new(|| BidderInfo {
        name: "fake_bb".into(),
        endpoint: Some("https://bidder.example.test/openrtb".into()),
        uses_https: true,
        ..BidderInfo::default()
    });
    &INFO
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RequestMode {
    #[default]
    Aggregate,
    Split,
    Chunked(NonZeroUsize),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ResponseCoverage {
    #[default]
    SentCall,
    /// Deliberately leave call-local filtering to the exchange under test.
    ScopedRequest,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum QueryMode<'a> {
    #[default]
    Preserve,
    Append(&'a [(&'a str, &'a str)]),
    Set {
        key: &'a str,
        value: &'a str,
    },
}

pub struct IdentityEnvelope<'a> {
    pub marker: &'a str,
    pub module: &'a str,
    pub constructor_count: usize,
    pub executions: &'a AtomicUsize,
}

pub type RequestObserver = dyn Fn(&BidRequest, &BidderRequestInfo<'_>) + Send + Sync;

#[cfg(test)]
mod tests {
    use super::info;
    use std::ptr;

    #[test]
    fn constructor_observation_counts_actual_factory_calls() {
        let before = super::factory_calls();
        drop(super::super::create_bidder());
        assert_eq!(super::factory_calls(), before + 1);
    }

    #[test]
    fn synthetic_metadata_is_shared_and_keeps_its_explicit_endpoint_policy() {
        let metadata = info();
        assert!(ptr::eq(metadata, info()));
        assert_eq!(metadata.name, "fake_bb");
        assert_eq!(
            metadata.endpoint.as_deref(),
            Some("https://bidder.example.test/openrtb")
        );
        assert!(metadata.uses_https);
    }
}

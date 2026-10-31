//! Synthetic metadata and opt-in capabilities for the shared test fixture.

use crate::BidderRequestInfo;
use auctioneer_config::types::BidderInfo;
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::{
    num::NonZeroUsize,
    sync::{LazyLock, atomic::AtomicUsize},
};

#[must_use]
pub fn info() -> &'static BidderInfo {
    static INFO: LazyLock<BidderInfo> = LazyLock::new(|| BidderInfo {
        name: "fake_bb".into(),
        endpoint: "https://bidder.example.test/openrtb".into(),
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
    fn synthetic_metadata_is_shared_and_keeps_its_explicit_endpoint_policy() {
        let metadata = info();
        assert!(ptr::eq(metadata, info()));
        assert_eq!(metadata.name, "fake_bb");
        assert_eq!(metadata.endpoint, "https://bidder.example.test/openrtb");
        assert!(metadata.uses_https);
    }
}

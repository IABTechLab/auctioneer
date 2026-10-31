//! Opt-in shared test fixtures that exercise the enforcing bidder interfaces.

#![allow(
    clippy::expect_used,
    reason = "Assertion-based setup is intentional in test-only fixture APIs."
)]

use crate::schema::staging::inspection_manifest;

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod factory_contract;
pub mod fake_bidder;
#[cfg(not(target_arch = "wasm32"))]
pub mod privacy_wire;
#[cfg(not(target_arch = "wasm32"))]
pub mod serving_catalog;

/// Generated inspection metadata only; never a validator or resource grant.
#[must_use]
pub fn schema_staging_manifest() -> Option<&'static str> {
    inspection_manifest()
}

#[cfg(test)]
use crate::{
    BidderHttpRequest, BidderName, BidderRequestInfo, CallBudget, EndpointRejectReason,
    EndpointValidator, EntryPoint, OutboundRequestSink, SinkLimits,
};

#[cfg(test)]
struct Accept;
#[cfg(test)]
impl EndpointValidator for Accept {
    fn validate(
        &self,
        _: &url::Url,
        _: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn request(headers: &[(&str, &str)]) -> BidderHttpRequest {
    let info = BidderRequestInfo::new(
        BidderName::new("fake_fixture"),
        BidderName::new("fake_fixture"),
        "account",
        None,
        "https://example.test/bid",
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    );
    let budget = CallBudget::new(1);
    let mut sink = OutboundRequestSink::new(SinkLimits::new(1024, 100), &budget, &Accept);
    sink.push_request(
        &info,
        |meta| {
            meta.set_method("POST")?;
            meta.set_url(info.endpoint_template)?;
            for &(name, value) in headers {
                meta.header(name, value)?;
            }
            meta.imp_id("imp-1")
        },
        |writer| writer.write_all(b"{}"),
    )
    .unwrap();
    sink.into_outcome().accepted.pop().unwrap()
}

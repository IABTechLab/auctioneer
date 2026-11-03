// Real-handler wiring must use a serving-registered identity. The direct
// exchange fixtures remain synthetic; this request never installs a fake
// registry or changes the App's dispatch path.

use crate::bidder_registry;
use auctioneer_config_data::types::MediaType;
use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::types::bid_request::BidRequestExt;

#[expect(
    clippy::expect_used,
    reason = "real-handler conformance requires a valid published registry"
)]
pub(super) fn request() -> Option<BidRequest> {
    // Guest conformance cannot read native owner fixture files. Select only
    // declared metadata here; the actual dispatch below must still succeed.
    let info = auctioneer_config_data::bidders().iter().find(|info| {
        !info.disabled
            && info.uses_https
            && info.gvl_vendor_id.is_none()
            && info.capabilities.site.contains(&MediaType::Banner)
            && info.debug_allow
            && info.endpoint_macros.is_empty()
            && info.endpoint_compression.is_none()
    })?;
    if !matches!(
        bidder_registry::registry_handle().expect("published registry").get(&info.name),
        Some((registered, _)) if !registered.disabled
    ) {
        return None;
    }
    let mut request = super::conformance_request_for("fake_bidder");
    request.ext = Some(BidRequestExt {
        prebid: Some(ExtRequestPrebid {
            aliases: [("fake_bidder".to_owned(), info.name.clone())].into(),
            ..ExtRequestPrebid::default()
        }),
        ..BidRequestExt::default()
    });
    Some(request)
}

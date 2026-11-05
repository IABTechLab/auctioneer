// Checked fixtures: no test-only construction bypass of the outbound sink.

use auctioneer_bidder::{
    BidderHttpRequest, BidderName, BidderRequestInfo, CallBudget, EntryPoint, OutboundBuildError,
    OutboundRequestSink, SinkLimits,
};
use auctioneer_config_data::types::{BidderInfo, Capabilities, OpenRtbCapabilities, SyncerConfig};
use auctioneer_exchange::ExchangeEndpointValidator;

/// Build a request through the same checked sink used by bidders.
///
/// # Errors
/// Returns the sink's validation error if any fixture metadata or body is invalid.
#[expect(
    clippy::needless_pass_by_value,
    reason = "owned fixture arguments preserve migrated request-construction call sites"
)]
pub fn request(
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    imp_ids: Vec<String>,
) -> Result<BidderHttpRequest, OutboundBuildError> {
    let info = BidderRequestInfo::new(
        BidderName::new("fake_fixture"),
        BidderName::new("fake_fixture"),
        "account",
        None,
        &url,
        &[],
        None,
        None,
        EntryPoint::Auction,
        &[],
    );
    let budget = CallBudget::new(1);
    let bidder = BidderInfo {
        alias_of: None,
        default_enabled: None,
        name: "fake_fixture".into(),
        endpoint: url.clone(),
        uses_https: true,
        disabled: false,
        debug_allow: false,
        endpoint_compression: None,
        gvl_vendor_id: None,
        host_allowlist: None,
        bidder_timeout_ms: None,
        endpoint_macros: vec![],
        maintainer_email: None,
        capabilities: Capabilities::default(),
        user_sync: SyncerConfig::default(),
        openrtb: OpenRtbCapabilities::default(),
    };
    let validator = ExchangeEndpointValidator::new(&bidder);
    let mut sink = OutboundRequestSink::new(
        SinkLimits::new(body.len(), u16::try_from(imp_ids.len()).unwrap_or(u16::MAX)),
        &budget,
        &validator,
    );
    sink.push_request(
        &info,
        |meta| {
            meta.set_method(&method)?;
            meta.set_url(&url)?;
            for (name, value) in &headers {
                meta.header(name, value)?;
            }
            for id in &imp_ids {
                meta.imp_id(id)?;
            }
            Ok(())
        },
        |writer| writer.write_all(&body),
    )?;
    sink.into_outcome()
        .accepted
        .pop()
        .ok_or(OutboundBuildError::MissingMethod)
}

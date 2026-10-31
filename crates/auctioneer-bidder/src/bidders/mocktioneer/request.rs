use crate::bidders::url::write_endpoint;
use crate::{BidderRequestInfo, OutboundRequestSink};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) {
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            write_endpoint(meta, info)?;
            meta.header("Content-Type", "application/json")?;
            for imp in &request.imp {
                meta.imp_id(&imp.id)?;
            }
            Ok(())
        },
        request,
    );
}

#[cfg(test)]
mod tests {
    use super::super::MocktioneerBidder;
    use crate::bidders::harness;
    use crate::{BidderRequestInfo, bidder::EntryPoint, types::BidderName};
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};

    #[test]
    fn make_requests_targets_127_0_0_1_template() {
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![Imp {
                id: "i-1".into(),
                ..Imp::default()
            }],
            ..BidRequest::default()
        };
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let info = BidderRequestInfo::new(
            BidderName::new("mocktioneer"),
            BidderName::new("mocktioneer"),
            "acct-x",
            None,
            "http://127.0.0.1:8787/openrtb2/auction",
            macros,
            None,
            None,
            EntryPoint::Auction,
            currency,
        );
        let out = harness::drive_ok(&MocktioneerBidder, &req, &info);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url(), "http://127.0.0.1:8787/openrtb2/auction");
    }
}

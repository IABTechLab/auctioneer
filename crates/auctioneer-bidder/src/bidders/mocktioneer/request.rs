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
    use crate::{
        Bidder as _, BidderRequestInfo, BidderSettings, bidder::EntryPoint, types::BidderName,
    };
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
    use serde_json::json;

    #[test]
    fn configured_endpoint_is_used_without_request_parameter_override_or_mutation() {
        let request: BidRequest = serde_json::from_value(json!({
            "id":"configured-auction",
            "imp":[{"id":"i-1", "banner":{"w":300_i32,"h":250_i32}, "ext":{"bidder":{
                "endpoint":"https://request-param.example.test:9443/untrusted?route=other", "bid":1.25_f64
            }}}]
        })).unwrap();
        let before = serde_json::to_value(&request).unwrap();
        assert_eq!(
            before["imp"][0]["ext"]["bidder"]["endpoint"],
            "https://request-param.example.test:9443/untrusted?route=other"
        );
        let settings: BidderSettings = serde_json::from_value(json!({
            "implementation":"mocktioneer", "settings":{"endpoint":"https://operator.example.test:8443/openrtb2/auction?route=one"}
        })).unwrap();
        for (requested, base) in [(Some(&settings), None), (None, Some(&settings))] {
            let endpoint = MocktioneerBidder
                .configured_endpoint(requested, base)
                .unwrap()
                .unwrap();
            let mut info = BidderRequestInfo::new(
                BidderName::new("mock-alias"),
                BidderName::new("mocktioneer"),
                "acct-x",
                None,
                endpoint.as_str(),
                &[],
                None,
                None,
                EntryPoint::Auction,
                &[],
            );
            info.set_settings(Some("mocktioneer"), requested, base)
                .unwrap();
            let out = harness::drive_ok(&MocktioneerBidder, &request, &info);
            assert_eq!(out.len(), 1);
            assert_eq!(
                out[0].url(),
                "https://operator.example.test:8443/openrtb2/auction?route=one"
            );
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(out[0].body()).unwrap(),
                before
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

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

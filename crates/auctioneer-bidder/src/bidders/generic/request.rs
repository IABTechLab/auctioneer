use crate::bidders::url::json_request_meta;
use crate::{BidderRequestInfo, OutboundRequestSink};
use auctioneer_openrtb::types::bid_request::BidRequest;

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) {
    let _recorded = sink.write_json(info, |meta| json_request_meta(meta, request, info), request);
}

#[cfg(test)]
mod tests {
    use super::super::GenericBidder;
    use crate::bidders::harness;
    use crate::{BidderRequestInfo, bidder::EntryPoint, types::BidderName};
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};

    #[test]
    fn make_requests_emits_one_post_with_serialized_request() {
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
            BidderName::new("generic"),
            BidderName::new("generic"),
            "acct-x",
            None,
            "https://demo.example.test/openrtb",
            macros,
            None,
            None,
            EntryPoint::Auction,
            currency,
        );
        let out = harness::drive_ok(&GenericBidder, &req, &info);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].method(), "POST");
        assert_eq!(out[0].url(), "https://demo.example.test/openrtb");
        assert_eq!(
            out[0].covered_imp_ids().collect::<Vec<_>>(),
            vec!["i-1".to_owned()]
        );
        let body: serde_json::Value = serde_json::from_slice(out[0].body()).unwrap();
        assert_eq!(body["id"], "auc-1");
    }

    #[test]
    fn make_requests_substitutes_account_id_macro() {
        let req = BidRequest {
            id: "auc-2".into(),
            imp: vec![Imp {
                id: "i-1".into(),
                ..Imp::default()
            }],
            ..BidRequest::default()
        };
        let macros: &[&str] = &["AccountID"];
        let currency: &[String] = &[];
        let info = BidderRequestInfo::new(
            BidderName::new("generic"),
            BidderName::new("generic"),
            "acct/space encoded?",
            None,
            "https://demo.example.test/openrtb?acct={{.AccountID}}",
            macros,
            None,
            None,
            EntryPoint::Auction,
            currency,
        );
        let out = harness::drive_ok(&GenericBidder, &req, &info);
        assert_eq!(out.len(), 1);
        assert!(
            out[0].url().ends_with("acct=acct%2Fspace%20encoded%3F"),
            "expected encoded account id, got {}",
            out[0].url(),
        );
    }
}

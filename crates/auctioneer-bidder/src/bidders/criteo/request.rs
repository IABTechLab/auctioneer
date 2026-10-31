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
    use super::super::CriteoBidder;
    use crate::bidders::harness;
    use crate::{
        BidderName, BidderRequestInfo, EndpointRejectReason, EndpointValidator, EntryPoint,
        OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    const ENDPOINT: &str = "https://bidder.example.test/openrtb?profile=230";

    struct Policy(bool);

    impl EndpointValidator for Policy {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if self.0 {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
    }

    fn request() -> BidRequest {
        serde_json::from_value(json!({
            "id":"request", "imp":[
                {"id":"i", "banner":{"w":300_i32,"h":250_i32},
                 "ext":{"bidder":{"zoneId":123_i32},"preserve":"imp"}},
                {"id":"v", "video":{"mimes":["video/mp4"]}},
            ],
            "ext":{"preserve":{"privacy_scrubbed":true}},
        }))
        .unwrap()
    }

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("criteo-alias"),
            BidderName::new("criteo"),
            "account",
            None,
            ENDPOINT,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn one_borrowed_post_preserves_body_order_and_has_no_bidder_headers() {
        let request = request();
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(&CriteoBidder, &request, &info());
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.method(), "POST");
        assert_eq!(call.url(), ENDPOINT);
        assert_eq!(call.headers().count(), 0);
        assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i", "v"]);
        assert_eq!(
            serde_json::from_slice::<Value>(call.body()).unwrap(),
            before
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn construction_refusals_use_the_real_sink_and_refund_the_call() {
        for (limit, calls, allowed, expected) in [
            (
                256 * 1024,
                0,
                true,
                OutboundBuildError::CallBudgetExhausted { max_calls: 0 },
            ),
            (1, 1, true, OutboundBuildError::OversizeRaw { limit: 1 }),
            (
                256 * 1024,
                1,
                false,
                OutboundBuildError::EndpointRejected {
                    reason: EndpointRejectReason::HostNotAllowed,
                },
            ),
        ] {
            let request = request();
            let before = serde_json::to_value(&request).unwrap();
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &CriteoBidder,
                &request,
                &info(),
                SinkLimits::new(limit, 2),
                calls,
                &Policy(allowed),
            );
            semantic.unwrap();
            assert!(outcome.accepted.is_empty());
            assert_eq!(outcome.errors, [expected]);
            assert_eq!(remaining, calls);
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }
}

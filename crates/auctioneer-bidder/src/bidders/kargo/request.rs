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
    use super::super::KargoBidder;
    use crate::bidders::harness;
    use crate::{
        BidderName, BidderRequestInfo, EndpointRejectReason, EndpointValidator, EntryPoint,
        OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use url::Url;

    struct EndpointPolicy(bool);

    impl EndpointValidator for EndpointPolicy {
        fn validate(
            &self,
            _url: &Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if self.0 {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
    }

    fn info() -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("kargo-alias"),
            BidderName::new("kargo"),
            "account",
            None,
            "https://bidder.example.test/bid",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn request(impressions: &Value) -> BidRequest {
        serde_json::from_value(json!({
            "id": "request", "imp": impressions,
            "site": {"domain": "publisher.example"},
            "user": {"keywords": "already scoped"},
            "ext": {"preserved": {"flag": true}},
        }))
        .unwrap()
    }

    fn minimal() -> BidRequest {
        request(&json!([{"id": "i", "banner": {}, "ext": {"bidder": {"placementId": "slot"}}}]))
    }

    #[test]
    fn passthrough_covers_every_impression_without_rewriting_the_scoped_request() {
        for count in [1_usize, 8, 100] {
            let impressions: Vec<_> = (0..count).map(|index| json!({
                "id": format!("imp-{index}"), "banner": {}, "ext": {"bidder": {"adSlotID": "slot"}},
            })).collect();
            let request = request(&json!(impressions));
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(&KargoBidder, &request, &info());
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method(), "POST");
            assert_eq!(calls[0].url(), "https://bidder.example.test/bid");
            assert_eq!(
                calls[0].covered_imp_ids().collect::<Vec<_>>(),
                request
                    .imp
                    .iter()
                    .map(|imp| imp.id.as_str())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                calls[0].headers().collect::<Vec<_>>(),
                [
                    ("Content-Type", "application/json"),
                    ("Accept", "application/json")
                ]
            );
            assert_eq!(
                serde_json::from_slice::<Value>(calls[0].body()).unwrap(),
                before
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn raw_body_and_impression_caps_are_enforced_at_the_exact_boundary() {
        let request = minimal();
        let body_bytes = serde_json::to_vec(&request).unwrap().len();
        for (bytes, ids, expected) in [
            (body_bytes, 1, None),
            (
                body_bytes - 1,
                1,
                Some(OutboundBuildError::OversizeRaw {
                    limit: u64::try_from(body_bytes - 1).unwrap(),
                }),
            ),
            (
                body_bytes,
                0,
                Some(OutboundBuildError::TooManyImpIds { limit: 0 }),
            ),
        ] {
            let (semantic, outcome, remaining) = harness::drive_with_limits(
                &KargoBidder,
                &request,
                &info(),
                SinkLimits::new(bytes, ids),
                1,
                &EndpointPolicy(true),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), usize::from(expected.is_none()));
            assert_eq!(remaining, u32::from(expected.is_some()));
            assert_eq!(outcome.errors, expected.into_iter().collect::<Vec<_>>());
            assert!(outcome.diagnostics.is_empty());
        }
    }

    #[test]
    fn call_exhaustion_and_endpoint_denial_remain_separate_sink_failures() {
        for (calls, allow, expected, remaining) in [
            (
                0,
                true,
                OutboundBuildError::CallBudgetExhausted { max_calls: 0 },
                0,
            ),
            (
                1,
                false,
                OutboundBuildError::EndpointRejected {
                    reason: EndpointRejectReason::HostNotAllowed,
                },
                1,
            ),
        ] {
            let (semantic, outcome, actual_remaining) = harness::drive_with_limits(
                &KargoBidder,
                &minimal(),
                &info(),
                SinkLimits::new(64 * 1024, 1),
                calls,
                &EndpointPolicy(allow),
            );
            semantic.unwrap();
            assert!(outcome.accepted.is_empty());
            assert_eq!(outcome.errors, [expected]);
            assert_eq!(actual_remaining, remaining);
        }
    }
}

//! Shared HTTP admission retains null compatibility while inspecting wire shape.

mod support;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::{config_funding, serving_catalog, support};
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_core::{
        handlers::{TestFanoutHandle, auction},
        host_primitives::{TestClockHandle, TestMonotonicClockHandle, TestRngHandle},
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{Clock as _, FakeClock, SeededRng};
    use edgezero_core::{body::Body, http::StatusCode, time::MonotonicClock};
    use futures::executor::block_on;
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    struct CountCalls(AtomicUsize);

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for CountCalls {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.0.fetch_add(calls.len(), Ordering::SeqCst);
            Ok(calls
                .into_iter()
                .map(|_call| BidderCallResult {
                    result: Ok(BidderHttpResponse::new(204, vec![], vec![])),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    fn send(wire: Vec<u8>) -> (StatusCode, usize) {
        config_funding::install();
        let fanout = Arc::new(CountCalls(AtomicUsize::new(0)));
        let clock = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let mut request = http::Request::builder()
            .method("POST")
            .uri("/openrtb2/auction")
            .body(Body::from_bytes(wire))
            .unwrap();
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::<CountCalls>::clone(&fanout)));
        request
            .extensions_mut()
            .insert(TestClockHandle(Arc::<FakeClock>::clone(&clock)));
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                clock.now_monotonic()
            })));
        request
            .extensions_mut()
            .insert(TestRngHandle(Arc::new(SeededRng::new(42))));
        let config = toml::to_string(&json!({
            "schema_version": 1_u64,
            "host": {
                "gdpr": {"enabled":false},
                "aliases": {"fake_bidder":serving_catalog::passthrough_info().name},
            },
        }))
        .unwrap();
        let response = block_on(auction(support::ctx_with_config(&config, request))).unwrap();
        (response.status(), fanout.0.load(Ordering::SeqCst))
    }

    fn request() -> Value {
        json!({
            "id": "root-extension", "site": {"id": "publisher"}, "tmax": 1000_i64,
            "imp": [{"id": "1", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"prebid": {"bidder": {"fake_bidder": {}}}}}],
        })
    }

    #[test]
    fn null_root_extension_is_not_globally_rejected_by_wire_inspection() {
        for extension in [None, Some(Value::Null), Some(json!({}))] {
            let mut input = request();
            if let Some(value) = extension {
                input["ext"] = value;
            }
            assert_eq!(
                send(serde_json::to_vec(&input).unwrap()),
                (StatusCode::OK, 1)
            );
        }
    }

    #[test]
    fn wrong_shaped_and_duplicate_root_extensions_refuse_before_dispatch() {
        for extension in [json!([]), json!("object"), json!(1_i32), json!(true)] {
            let mut input = request();
            input["ext"] = extension;
            assert_eq!(
                send(serde_json::to_vec(&input).unwrap()),
                (StatusCode::BAD_REQUEST, 0)
            );
        }
        let mut duplicate = serde_json::to_vec(&request()).unwrap();
        assert_eq!(duplicate.pop(), Some(b'}'));
        // json! cannot retain duplicate keys or distinguish their wire order.
        duplicate.extend_from_slice(b",\"ext\":null,\"ext\":{}}");
        assert_eq!(send(duplicate), (StatusCode::BAD_REQUEST, 0));
    }
}

//! Numeric canonicalization must fit the installed source allowance.

mod support;

#[cfg(test)]
pub mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use super::support;
    use crate::config_funding;
    use crate::config_funding::envelope;
    use crate::serving_catalog;
    use auctioneer_bidder::BidderHttpResponse;
    use auctioneer_core::App as AuctioneerApp;
    use auctioneer_core::handlers::request_disclosure_audit::construction_costs;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{
        TestClockHandle, TestMonotonicClockHandle, TestRngHandle, boot_gates, install_boot_envelope,
    };
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_runtime::{
        Clock as _, FakeClock, SeededRng, TargetPlatformConstants, count_json_nodes,
    };
    use edgezero_core::app::App;
    use edgezero_core::body::Body;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[derive(Default)]
    struct CapturingFanout(AtomicUsize);

    #[async_trait::async_trait(?Send)]
    impl BidderFanout for CapturingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            self.0.fetch_add(calls.len(), Ordering::Relaxed);
            Ok(calls
                .iter()
                .map(|call| {
                    let body: serde_json::Value =
                        serde_json::from_slice(call.request.body()).unwrap();
                    assert!(body["device"].get("opaque").is_none());
                    BidderCallResult {
                        result: Ok(BidderHttpResponse::new(204, vec![], Vec::new())),
                        elapsed: Duration::ZERO,
                    }
                })
                .collect())
        }
    }

    fn install_fixture_envelope() {
        let mut limits = envelope::limits();
        let target = TargetPlatformConstants {
            platform_concurrent_ceiling: 384,
            header_entry_overhead_bytes: 64,
            parser_header_ceiling_bytes: 21_632,
            parser_header_knob_bytes: 21_633,
            max_request_target_bytes: 768,
            h1_max_buf_size_bytes: 24_576,
            h1_max_headers: 128,
            parser_max_header_entries: 676,
            pseudoheader_allowance_bytes: 1024,
            parser_transient_bytes: 0x4000,
            egress_handoff_factor: 2,
        };
        config_funding::fund_sdk_defaults(&mut limits, &target);
        install_boot_envelope(
            &limits,
            &target,
            auctioneer_bidder::outbound_meta_bytes(limits.max_impressions_per_auction),
            construction_costs(),
            auctioneer_runtime::CatalogResourceCosts::UNCHANGED,
        )
        .unwrap();
        let _app = App::build::<AuctioneerApp>(edgezero_core::PlatformMetadata::default()).unwrap();
    }

    fn compact_numeric_body() -> Vec<u8> {
        // Exact wire spelling is the measurement: json! would canonicalize
        // the compact exponent tokens before ingress and hide source growth.
        let mut body = br#"{"id":"numeric-source","tmax":1000,"site":{"id":"s"},"imp":[{"id":"1","banner":{"w":300,"h":250},"ext":{"prebid":{"bidder":{"fake_bidder":{}}}}}],"regs":{"coppa":1},"device":{"opaque":["#.to_vec();
        for index in 0_usize..50_000_usize {
            if index != 0 {
                body.push(b',');
            }
            body.extend_from_slice(b"1e15");
        }
        body.extend_from_slice(b"]}}");
        body
    }

    #[test]
    fn compact_exponent_source_is_admitted_before_coppa_scrub_and_dispatch() {
        install_fixture_envelope();
        let gates = boot_gates().unwrap();
        let body = compact_numeric_body();
        assert!(body.len() > 250_000);
        assert!(u64::try_from(body.len()).unwrap() <= gates.limits.inbound_request_bytes);
        count_json_nodes(&body, gates.limits.max_parsed_nodes).unwrap();
        let decoded: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            decoded["device"]["opaque"].as_array().unwrap().len(),
            50_000
        );
        assert!(serde_json::to_vec(&decoded).unwrap().len() > 950_000);

        let fanout = Arc::new(CapturingFanout::default());
        let fake = Arc::new(FakeClock::new(time::OffsetDateTime::UNIX_EPOCH));
        let mut request = http::Request::builder()
            .method("POST")
            .uri("/openrtb2/auction")
            .body(Body::from_bytes(body))
            .unwrap();
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::<CapturingFanout>::clone(&fanout)));
        request
            .extensions_mut()
            .insert(TestClockHandle(Arc::<FakeClock>::clone(&fake)));
        request
            .extensions_mut()
            .insert(TestMonotonicClockHandle(MonotonicClock::new(move || {
                fake.now_monotonic()
            })));
        request
            .extensions_mut()
            .insert(TestRngHandle(Arc::new(SeededRng::new(42))));
        let config = toml::to_string(&serde_json::json!({
            "schema_version": 1_u64,
            "host": {
                "gdpr": {"enabled":false},
                "aliases": {"fake_bidder":serving_catalog::passthrough_info().name},
            },
        }))
        .unwrap();
        let response = block_on(auction(support::ctx_with_config(&config, request))).unwrap();
        assert_eq!(response.status(), http::StatusCode::OK);
        assert_eq!(fanout.0.load(Ordering::Relaxed), 1);
        assert_eq!(gates.admission.in_use(), 0, "auction admission is released");
    }
}

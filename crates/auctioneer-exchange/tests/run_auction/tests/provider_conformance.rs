// Published-decoder response diagnostics and currency evidence. Factories and
// parameters come from serving bindings and owner fixtures, not a candidate roster.

use super::*;
use auctioneer_bidder::test_support::serving_catalog::{
    ServingBidder, default_active_publications,
};

fn auction_with_bidder_response(body: &[u8], bidders: &[ServingBidder]) -> BidResponse {
    let mut registry = BidderRegistry::new();
    let endpoint = "https://bidder.example.test/openrtb";
    for bidder in bidders {
        registry
            .register(leak_info(bidder.name(), endpoint), (bidder.factory)())
            .expect("unique test bidder");
    }
    let params: serde_json::Map<String, Value> = bidders
        .iter()
        .map(|bidder| (bidder.name().to_owned(), bidder.params.clone()))
        .collect();
    let request = prepare(
        serde_json::from_value(json!({
            "id": "auction-diagnostics", "site": {"id": "site"},
            "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"bidder": params}}]
        }))
        .unwrap(),
        &RequestHeaders::default(),
        &IMP_CAP_NOT_UNDER_TEST,
    )
    .unwrap();
    let host = host_with_caps(1000, 2000);
    let config = AccountConfig::default();
    let account = ResolvedAccount {
        id: "publisher".into(),
        source: AccountSource::DefaultFallback,
        config: &config,
    };
    let fanout = CannedFanout {
        bodies: vec![body.to_vec(); bidders.len()],
    };
    let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
    let rng = SeededRng::new(42);
    let ctx = AuctionContext::with_request_start(
        clock.now_monotonic(),
        &host,
        &account,
        &registry,
        &fanout,
        &clock,
        &rng,
        GvlStore::baked(),
        Gcl::baked(),
        clock.now(),
        false,
    );
    let plan = AuctionPlan::new(
        ResolvedAliases::empty(),
        BTreeSet::new(),
        response_budgets(ctx.bidders, ctx.host),
        BTreeMap::new(),
    );
    let mut audit = DisclosureAudit::default();
    let response = block_on(run_auction(&request, &ctx, &plan, &mut audit)).unwrap();
    let timings = response.ext.as_ref().unwrap()["responsetimemillis"]
        .as_object()
        .unwrap();
    assert_eq!(timings.len(), bidders.len());
    for bidder in bidders {
        let name = bidder.name();
        assert!(timings.contains_key(name), "{name} must reach dispatch");
    }
    response
}

fn bid_response_with_currency(currency: Option<&str>) -> Vec<u8> {
    let mut response = json!({
        "id": "r", "seatbid": [{"seat": "fake_s", "bid": [{
            "id": "b", "impid": "imp-1", "price": 1.5_f64, "w": 300_i32, "h": 250_i32, "mtype": 1_i32
        }]}]
    });
    if let Some(value) = currency {
        response["cur"] = json!(value);
    }
    serde_json::to_vec(&response).unwrap()
}

#[test]
fn bidder_response_parse_errors_do_not_echo_payloads() {
    let bidders = default_active_publications();
    for marker in [
        "RESPONSE_SECRET".to_owned(),
        "RESPONSE_SECRET".repeat(5000),
        "RESPONSE_SECRET\n\"\\\u{2603}".to_owned(),
    ] {
        let mut truncated = serde_json::to_vec(&json!({"id": "r", "secret": marker})).unwrap();
        truncated.pop();
        let bodies = [
            serde_json::to_vec(&json!({"id": "r", "seatbid": marker})).unwrap(),
            serde_json::to_vec(&json!({"id": "r", "seatbid": [{
                "seat": "fake_s", "bid": [{"id": "b", "impid": "imp-1", "price": marker}]
            }]}))
            .unwrap(),
            truncated,
        ];
        for body in bodies {
            let response = auction_with_bidder_response(&body, &bidders);
            assert!(response.seatbid.is_empty());
            assert!(
                !serde_json::to_string(&response)
                    .unwrap()
                    .contains("RESPONSE_SECRET")
            );
            let ext = response.ext.as_ref().unwrap();
            for bidder in &bidders {
                let name = bidder.name();
                assert_eq!(
                    ext["errors"][name],
                    json!([{
                        "code": 14_u32, "message": "invalid bidder response"
                    }]),
                    "{name}"
                );
            }
            assert!(ext.get("warnings").is_none());
        }
    }
}

#[test]
fn unsupported_currency_warnings_do_not_echo_payloads() {
    let bidders = default_active_publications();
    for currency in [
        "EUR".to_owned(),
        "RESPONSE_SECRET".repeat(5000),
        "RESPONSE_SECRET\n\"\\\u{2603}".to_owned(),
    ] {
        let response =
            auction_with_bidder_response(&bid_response_with_currency(Some(&currency)), &bidders);
        assert!(
            response.seatbid.is_empty(),
            "unsupported currency bids must stay dropped"
        );
        assert!(
            !serde_json::to_string(&response)
                .unwrap()
                .contains(&currency)
        );
        let ext = response.ext.as_ref().unwrap();
        for bidder in &bidders {
            let name = bidder.name();
            assert_eq!(
                ext["warnings"][name],
                json!([{
                    "code": 10_004_u32, "message": "non-USD currency; bids dropped"
                }]),
                "{name}"
            );
        }
        assert!(ext.get("errors").is_none());
    }
}

#[test]
fn default_or_usd_currency_keeps_bids_without_response_diagnostics() {
    let bidders = default_active_publications();
    for currency in [None, Some(""), Some("USD")] {
        let response =
            auction_with_bidder_response(&bid_response_with_currency(currency), &bidders);
        assert_eq!(response.cur, "USD");
        assert_eq!(response.seatbid.len(), bidders.len());
        for seat in &response.seatbid {
            assert_eq!(seat.bid.len(), 1);
            assert_eq!(seat.bid[0].id, "b");
        }
        let ext = response.ext.as_ref().unwrap();
        assert!(ext.get("errors").is_none());
        assert!(ext.get("warnings").is_none());
    }
}

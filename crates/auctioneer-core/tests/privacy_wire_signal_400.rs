//! Fatal privacy inputs, driven through the real auction handler.
//!
//! `regs.gdpr` outside `{0, 1}` with no `regs.gpp_sid` to disambiguate
//! is a client error, so the handler must answer 400 with the exact
//! plain-text body — not a 200 carrying a warning, and not a 500. The
//! gate runs before alias resolution, so an otherwise-invalid request
//! still gets the privacy 400.

mod support;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use crate::serving_catalog::{ServingBidder, default_active_publications};
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse};
    use auctioneer_config_data::types::MediaType;
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{TestClockHandle, TestRngHandle};
    use auctioneer_core::test_support::config_store::auction_context_for_route;
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, BidRequestExt, Imp};
    use auctioneer_openrtb::types::field::JsonField;
    use auctioneer_openrtb::types::regs::Regs;
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    use std::mem;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use time::OffsetDateTime;

    const WIDE_CAP: usize = 1 << 20;

    static AUCTIONS: Mutex<()> = Mutex::new(());

    /// Answers every call with a well-formed empty response, so a
    /// request that gets PAST the privacy gate reaches a clean 200 and
    /// the 400 rows cannot pass for an unrelated reason.
    struct QuietFanout(Arc<Mutex<Vec<BidderHttpRequest>>>);

    #[async_trait(?Send)]
    impl BidderFanout for QuietFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut captured = self.0.lock().expect("capture lock");
            Ok(calls
                .into_iter()
                .map(|call| {
                    captured.push(call.request);
                    BidderCallResult {
                        result: Ok(BidderHttpResponse::new(204_u16, vec![], vec![])),
                        elapsed: Duration::from_millis(1_u64),
                    }
                })
                .collect())
        }
    }

    fn auction_bidder() -> ServingBidder {
        default_active_publications()
            .into_iter()
            .find(|row| {
                let info = row.info;
                !info.disabled
                    && info.uses_https
                    && !info.endpoint.contains("{{")
                    && info.endpoint_compression.is_none()
                    && info.capabilities.site.contains(&MediaType::Banner)
                    && row.owner_expected_request_body.as_ref() == Some(&row.owner_request)
            })
            .expect("privacy wire coverage requires a fixed-endpoint banner passthrough")
    }

    fn request_with_regs(regs: Regs) -> BidRequest {
        let mut bidders = serde_json::Map::new();
        bidders.insert("fake_fixture_bidder".to_owned(), json!({}));
        BidRequest {
            id: "auc-wire-signal".to_owned(),
            tmax: Some(1000_i64),
            imp: vec![Imp {
                id: "imp-1".to_owned(),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Banner::default()
                }),
                ext: Some(ImpExt {
                    bidder: JsonField::Value(serde_json::Value::Object(bidders)),
                    ..ImpExt::default()
                }),
                ..Imp::default()
            }],
            cur: vec!["USD".to_owned()],
            regs: Some(regs),
            ..BidRequest::default()
        }
    }

    fn with_serving_bidder(mut request: BidRequest) -> BidRequest {
        let bidder = auction_bidder();
        request.imp[0].ext.as_mut().expect("fixture imp.ext").bidder =
            JsonField::Value(json!({bidder.name(): &bidder.params}));
        request
    }

    /// POST the fixture through the real handler and hand back the
    /// status plus the decoded body.
    fn drive(regs: Regs) -> (StatusCode, String) {
        drive_request(&with_serving_bidder(request_with_regs(regs)))
    }

    fn drive_request(fixture: &BidRequest) -> (StatusCode, String) {
        let (status, body, _) = drive_counted(fixture, "schema_version = 1\n");
        (status, body)
    }

    fn drive_counted(fixture: &BidRequest, config: &str) -> (StatusCode, String, usize) {
        let (status, body, captured) = drive_captured(fixture, config);
        (status, body, captured.len())
    }

    fn drive_captured(
        fixture: &BidRequest,
        config: &str,
    ) -> (StatusCode, String, Vec<BidderHttpRequest>) {
        let _serialized = AUCTIONS.lock().expect("auction fixture lock");
        config_funding::install();
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH));
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let serialized = serde_json::to_vec(fixture).expect("the fixture serializes");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(serialized))
            .expect("request builds");
        request
            .extensions_mut()
            .insert(TestFanoutHandle(Arc::new(QuietFanout(Arc::clone(
                &captured,
            )))));
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        let (ctx, mut envelope) = auction_context_for_route(
            ctx_with_config(config, request).into_request().unwrap(),
            "/openrtb2/auction",
            MonotonicClock::default(),
        );

        block_on(async {
            *envelope.response_mut() = auction(ctx).await.expect("the handler answers");
            let response = mem::take(envelope.response_mut());
            let status = response.status();
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            drop(envelope);
            (
                status,
                String::from_utf8(bytes.to_vec()).expect("the body is UTF-8 text"),
                mem::take(&mut *captured.lock().expect("capture lock")),
            )
        })
    }

    fn conflicting_usp_request(no_sale: &str) -> BidRequest {
        let mut fixture = serde_json::to_value(request_with_regs(Regs {
            gpp: Some("DBABTA~1YYN".into()),
            gpp_sid: Some(vec![6_i32]),
            us_privacy: Some("1YNN".into()),
            ..Regs::default()
        }))
        .expect("the fixture serializes");
        fixture["ext"] = json!({"prebid": {"nosale": [no_sale]}});
        fixture["user"] = json!({
            "id": "private-user", "buyeruid": "private-buyer",
            "eids": [{"source": "id.example", "uids": [{"id": "private-eid"}]}],
            "keywords": "retained-keywords"
        });
        fixture["device"] = json!({
            "ifa": "private-ifa", "ip": "8.8.4.123",
            "geo": {"lat": 48.858_844_f64, "lon": -1.125_f64}
        });
        serde_json::from_value(fixture).expect("the fixture is a typed request")
    }

    #[test]
    fn conflicting_gpp_usp_with_invalid_no_sale_is_400_before_ccpa() {
        let fixture = conflicting_usp_request("not-registered");
        for enforce in [false, true] {
            let (status, body, sends) = drive_counted(
                &fixture,
                &format!("schema_version = 1\n[host.ccpa]\nenforce = {enforce}"),
            );
            assert_eq!(status, StatusCode::BAD_REQUEST, "enforce={enforce}: {body}");
            assert_eq!(
                body,
                "Invalid request: ext.prebid.nosale contains an invalid bidder exemption\n"
            );
            assert_eq!(sends, 0_usize, "enforce={enforce}");
        }
    }

    #[test]
    fn conflicting_gpp_usp_with_valid_no_sale_warns_and_obeys_ccpa() {
        // A registered, nonparticipating exemption leaves the actual bidder subject to CCPA.
        let participating = auction_bidder();
        let exempt = auctioneer_config_data::bidders()
            .iter()
            .find(|info| !info.disabled && info.name != participating.name())
            .expect("CCPA coverage requires a distinct registered nonparticipant");
        let fixture = with_serving_bidder(conflicting_usp_request(&exempt.name));
        let original = serde_json::to_value(&fixture).expect("the fixture serializes");
        for enforce in [false, true] {
            let (status, body, captured) = drive_captured(
                &fixture,
                &format!("schema_version = 1\n[host.ccpa]\nenforce = {enforce}"),
            );
            assert_eq!(status, StatusCode::OK, "enforce={enforce}: {body}");
            assert_eq!(captured.len(), 1_usize, "enforce={enforce}: {body}");
            let response: Value = serde_json::from_str(&body).expect("response JSON");
            assert_eq!(
                response["ext"]["warnings"],
                json!({"prebid": [{
                    "code": 10_001_u32,
                    "message": "regs.gpp section 6 and regs.us_privacy conflict; using section 6"
                }]})
            );
            let outbound: Value =
                serde_json::from_slice(captured[0_usize].body()).expect("outbound JSON");
            assert_eq!(outbound["regs"]["gpp"], "DBABTA~1YYN");
            assert_eq!(outbound["regs"]["gpp_sid"], json!([6_i32]));
            assert_eq!(outbound["regs"]["us_privacy"], "1YNN");
            assert_eq!(outbound["user"]["keywords"], "retained-keywords");
            for path in ["/user/id", "/user/buyeruid", "/user/eids", "/device/ifa"] {
                assert!(original.pointer(path).is_some(), "seeded {path}");
                if enforce {
                    assert!(outbound.pointer(path).is_none(), "scrubbed {path}");
                } else {
                    assert_eq!(outbound.pointer(path), original.pointer(path), "{path}");
                }
            }
            if enforce {
                assert_eq!(outbound["device"]["ip"], "8.8.4.0");
                assert_eq!(
                    outbound["device"]["geo"],
                    json!({"lat": 48.86_f64, "lon": -1.12_f64})
                );
            } else {
                assert_eq!(outbound["device"]["ip"], original["device"]["ip"]);
                assert_eq!(outbound["device"]["geo"], original["device"]["geo"]);
            }
        }
    }

    #[test]
    fn invalid_no_sale_is_400_even_with_disabled_ccpa_and_invalid_usp() {
        let mut fixture = request_with_regs(Regs {
            gdpr: Some(0_i32),
            us_privacy: Some("private-invalid-consent".into()),
            ..Regs::default()
        });
        fixture.ext = Some(BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                nosale: vec!["not-registered".into()],
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        });
        let (status, body, sends) =
            drive_counted(&fixture, "schema_version = 1\n[host.ccpa]\nenforce = false");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            "Invalid request: ext.prebid.nosale contains an invalid bidder exemption\n"
        );
        assert_eq!(sends, 0);
    }

    #[test]
    fn gpp_limit_is_400_before_outbound_even_when_gdpr_is_off() {
        let fixture = request_with_regs(Regs {
            gdpr: Some(0_i32),
            gpp: Some(format!("DBAA{}", "~".repeat(65))),
            gpp_sid: Some(vec![]),
            ..Regs::default()
        });
        let (status, body, sends) = drive_counted(&fixture, "schema_version = 1\n");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            body,
            "Invalid request: regs.gpp exceeds privacy parsing limits\n"
        );
        assert_eq!(sends, 0);
    }

    #[test]
    fn runtime_activity_config_reaches_the_real_handler() {
        let mut value =
            serde_json::to_value(with_serving_bidder(request_with_regs(Regs::default()))).unwrap();
        value["site"] = json!({"publisher":{"id":"fake_account"}});
        let fixture: BidRequest = serde_json::from_value(value).unwrap();
        let config = "schema_version = 1\n\n[accounts.fake_account.privacy.allowactivities.fetchBids]\ndefault = false\n";
        let (status, body, sends) = drive_counted(&fixture, config);
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(sends, 0);
        let response: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            response["ext"]["warnings"][auction_bidder().name()][0]["code"],
            10017_i32
        );

        let (control_status, control_body, control_sends) =
            drive_counted(&fixture, "schema_version = 1\n");
        assert_eq!(control_status, StatusCode::OK, "{control_body}");
        assert_eq!(control_sends, 1, "control must reach the real fanout hook");
    }

    #[test]
    fn invalid_gdpr_without_gpp_sid_returns_400() {
        let (status, body) = drive_request(&request_with_regs(Regs {
            gdpr: Some(2_i32),
            ..Regs::default()
        }));
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "an out-of-domain regs.gdpr is a client error, not a warning",
        );
        assert_eq!(
            body, "Invalid request: regs.gdpr must be 0 or 1, got 2\n",
            "the 400 body must name the offending value verbatim",
        );
    }

    #[test]
    fn regs_gdpr_out_of_range_without_gpp_sid() {
        for got in [-1_i32, 2_i32, 7_i32] {
            let (status, body) = drive(Regs {
                gdpr: Some(got),
                ..Regs::default()
            });
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "regs.gdpr = {got} is out of the 0/1 domain",
            );
            assert_eq!(
                body,
                format!("Invalid request: regs.gdpr must be 0 or 1, got {got}\n"),
                "every out-of-range value reports itself in the body",
            );
        }

        // The precedence rule: a non-empty `gpp_sid` outranks
        // `regs.gdpr`, so the same out-of-domain value is never
        // consulted and never fatal.
        let (gpp_status, gpp_body) = drive(Regs {
            gdpr: Some(2_i32),
            gpp_sid: Some(vec![2_i32]),
            ..Regs::default()
        });
        assert_eq!(
            gpp_status,
            StatusCode::OK,
            "gpp_sid outranks regs.gdpr, so the request must not 400; body {gpp_body}",
        );

        // In-domain values are never fatal either.
        for got in [0_i32, 1_i32] {
            let (valid_status, valid_body) = drive(Regs {
                gdpr: Some(got),
                ..Regs::default()
            });
            assert_eq!(
                valid_status,
                StatusCode::OK,
                "regs.gdpr = {got} is valid; body {valid_body}",
            );
        }
    }

    /// The privacy gate runs BEFORE alias resolution, so a request that
    /// would 400 on both reports the privacy failure. Without the
    /// ordering, the alias rejection would preempt it and the client
    /// would be told to fix the wrong field.
    #[test]
    fn the_privacy_400_preempts_a_concurrent_alias_400() {
        let mut aliases: BTreeMap<String, String> = BTreeMap::new();
        aliases.insert(
            "fake_fixture_bidder".to_owned(),
            "fake_fixture_bidder".to_owned(),
        );

        // Sanity leg: the alias fault alone is what 400s when the wire
        // signal is clean, which is what makes the ordering leg
        // meaningful.
        let mut alias_only = request_with_regs(Regs::default());
        alias_only.ext = Some(BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                aliases: aliases.clone(),
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        });
        let (alias_status, alias_body) = drive_request(&alias_only);
        assert_eq!(alias_status, StatusCode::BAD_REQUEST);
        assert_eq!(
            alias_body, "Invalid request: alias fake_fixture_bidder is a no-op\n",
            "the alias fault is a 400 in its own right",
        );

        // Ordering leg: both faults present, privacy wins.
        let mut both = request_with_regs(Regs {
            gdpr: Some(2_i32),
            ..Regs::default()
        });
        both.ext = Some(BidRequestExt {
            prebid: Some(ExtRequestPrebid {
                aliases,
                ..ExtRequestPrebid::default()
            }),
            ..BidRequestExt::default()
        });
        let (both_status, both_body) = drive_request(&both);
        assert_eq!(both_status, StatusCode::BAD_REQUEST);
        assert_eq!(
            both_body, "Invalid request: regs.gdpr must be 0 or 1, got 2\n",
            "the privacy gate must run first, so its message is the one the client sees",
        );
    }
}

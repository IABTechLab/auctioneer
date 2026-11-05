//! `validate_endpoint` SSRF-gate integration tests.

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod tests {
    use crate::fake_bidder::FakeBidder;
    use auctioneer_bidder::{
        Bidder as _, BidderName, BidderRequestInfo, CallBudget, EndpointRejectReason, EntryPoint,
        OutboundBuildError, OutboundRequestSink, SinkLimits,
    };
    use auctioneer_config_data::types::BidderInfo;
    use auctioneer_exchange::{ExchangeEndpointValidator, validate_endpoint};
    use serde_json::json;
    use std::cell::Cell;

    fn info_fixed_host_https() -> BidderInfo {
        BidderInfo {
            name: "fake_ax".into(),
            endpoint: "https://hb.example.test/openrtb".into(),
            uses_https: true,
            host_allowlist: None,
            ..BidderInfo::default()
        }
    }

    fn info_host_varying() -> BidderInfo {
        BidderInfo {
            name: "fake_ax".into(),
            endpoint: "https://{{.Host}}/openrtb".into(),
            uses_https: true,
            host_allowlist: Some(vec!["eu.example.test".into(), "us.example.test".into()]),
            ..BidderInfo::default()
        }
    }

    #[test]
    fn fixed_host_match_accepted() {
        let info = info_fixed_host_https();
        validate_endpoint("https://hb.example.test/openrtb?acct=42", &info).unwrap();
    }

    fn macro_info(template: &str) -> BidderRequestInfo<'_> {
        BidderRequestInfo::new(
            BidderName::new("fake_bb"),
            BidderName::new("fake_bb"),
            "account",
            Some("{{.literal}}"),
            template,
            &["PublisherID"],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn encoded_publisher_macro_data_is_not_an_unexpanded_template() {
        for (template, expanded) in [
            (
                "https://hb.example.test/{{.PublisherID}}",
                "https://hb.example.test/%7B%7B.literal%7D%7D",
            ),
            (
                "https://hb.example.test/rtb?publisher={{.PublisherID}}",
                "https://hb.example.test/rtb?publisher=%7B%7B.literal%7D%7D",
            ),
        ] {
            let request =
                serde_json::from_value(json!({"id": "auction", "imp": [{"id": "imp-1"}]})).unwrap();
            let info = macro_info(template);
            let budget = CallBudget::new(1);
            let configured = BidderInfo {
                endpoint: template.into(),
                ..info_fixed_host_https()
            };
            let validator = ExchangeEndpointValidator::new(&configured);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(1024, 1), &budget, &validator);
            FakeBidder {
                endpoint_values: &[("PublisherID", info.publisher_id.unwrap())],
                ..FakeBidder::default()
            }
            .make_requests(&request, &info, &mut sink)
            .unwrap();
            let outcome = sink.into_outcome();
            assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(outcome.accepted[0].url(), expanded);
            validate_endpoint(expanded, &configured).unwrap();
        }
    }

    #[test]
    fn literal_unexpanded_templates_fail_before_body_construction() {
        for template in [
            "https://hb.example.test/{{.PublisherID}}",
            "https://hb.example.test/rtb?publisher={{.PublisherID}}",
        ] {
            let info = macro_info(template);
            let budget = CallBudget::new(1);
            let configured = BidderInfo {
                endpoint: template.into(),
                ..info_fixed_host_https()
            };
            let validator = ExchangeEndpointValidator::new(&configured);
            let mut sink = OutboundRequestSink::new(SinkLimits::new(1024, 1), &budget, &validator);
            let body_calls = Cell::new(0_usize);
            let result = sink.push_request(
                &info,
                |meta| {
                    meta.set_method("POST")?;
                    meta.set_url(template)?;
                    Ok(())
                },
                |writer| {
                    body_calls.set(body_calls.get().saturating_add(1));
                    writer.write_all(b"{}")
                },
            );
            assert!(matches!(
                result,
                Err(OutboundBuildError::EndpointRejected {
                    reason: EndpointRejectReason::TemplateUnresolved
                })
            ));
            assert_eq!(body_calls.get(), 0);
            let error = validate_endpoint(template, &configured).unwrap_err();
            assert_eq!(error.code, 5);
            assert_eq!(error.message, "outbound template unresolved");
        }
    }

    #[test]
    fn fixed_host_mismatch_rejected() {
        let info = info_fixed_host_https();
        let err = validate_endpoint("https://rogue.example.test/openrtb", &info).unwrap_err();
        assert_eq!(err.code, 5);
    }

    #[test]
    fn host_varying_member_accepted() {
        let info = info_host_varying();
        validate_endpoint("https://us.example.test/openrtb", &info).unwrap();
    }

    #[test]
    fn host_varying_off_allowlist_rejected() {
        let info = info_host_varying();
        let err = validate_endpoint("https://rogue.example.test/openrtb", &info).unwrap_err();
        assert_eq!(err.code, 5);
    }

    #[cfg(not(feature = "insecure-bidders"))]
    #[test]
    fn https_only_in_default_build() {
        let info = info_fixed_host_https();
        let err = validate_endpoint("http://hb.example.test/openrtb", &info).unwrap_err();
        assert_eq!(err.code, 5);
    }

    #[test]
    fn fixed_host_rejects_userinfo() {
        let info = info_fixed_host_https();
        let err = validate_endpoint("https://attacker@hb.example.test/openrtb", &info).unwrap_err();
        assert_eq!(err.code, 5);
        assert!(err.message.contains("userinfo"));
    }

    #[test]
    fn fixed_host_rejects_port_mismatch() {
        let info = info_fixed_host_https();
        let err = validate_endpoint("https://hb.example.test:444/openrtb", &info).unwrap_err();
        assert_eq!(err.code, 5);
        assert!(err.message.contains("authority"));
    }

    #[test]
    fn private_ip_literals_rejected() {
        let info = BidderInfo {
            name: "fake_ax".into(),
            endpoint: "https://10.0.0.1/x".into(),
            uses_https: true,
            host_allowlist: None,
            ..BidderInfo::default()
        };
        let err = validate_endpoint("https://10.0.0.1/x", &info).unwrap_err();
        assert_eq!(err.code, 5);
    }

    #[cfg(not(feature = "insecure-bidders"))]
    #[test]
    fn localhost_literal_rejected_in_default_build() {
        let info = BidderInfo {
            name: "fake_ax".into(),
            endpoint: "https://localhost/x".into(),
            uses_https: true,
            host_allowlist: None,
            ..BidderInfo::default()
        };
        let err = validate_endpoint("https://localhost/x", &info).unwrap_err();
        assert_eq!(err.code, 5);
    }

    #[cfg(not(feature = "insecure-bidders"))]
    #[test]
    fn ipv6_loopback_rejected_in_default_build() {
        let info = BidderInfo {
            name: "fake_ax".into(),
            endpoint: "https://[::1]/x".into(),
            uses_https: true,
            host_allowlist: None,
            ..BidderInfo::default()
        };
        let err = validate_endpoint("https://[::1]/x", &info).unwrap_err();
        assert_eq!(err.code, 5);
    }

    #[test]
    fn ipv6_reserved_literals_rejected_in_default_build() {
        for literal in [
            "fc00::1",         // unique-local
            "fe80::1",         // link-local
            "::",              // unspecified
            "ff02::1",         // multicast
            "::ffff:10.0.0.1", // IPv4-mapped private
        ] {
            let url = format!("https://[{literal}]/x");
            let info = BidderInfo {
                name: "fake_ax".into(),
                endpoint: url.clone(),
                uses_https: true,
                host_allowlist: None,
                ..BidderInfo::default()
            };
            let err = validate_endpoint(&url, &info)
                .expect_err("expected reject for IPv6 reserved literal");
            assert_eq!(err.code, 5, "literal {literal}");
        }
    }

    #[test]
    fn ipv4_reserved_literals_rejected_in_default_build() {
        for literal in [
            "0.0.0.1",         // 0.0.0.0/8
            "100.64.0.1",      // 100.64.0.0/10 CGN
            "192.0.2.1",       // documentation TEST-NET-1
            "198.18.0.1",      // benchmarking (RFC 2544)
            "198.51.100.1",    // documentation TEST-NET-2
            "203.0.113.1",     // documentation TEST-NET-3
            "240.0.0.1",       // 240.0.0.0/4 reserved
            "255.255.255.255", // limited broadcast
        ] {
            let url = format!("https://{literal}/x");
            let info = BidderInfo {
                name: "fake_ax".into(),
                endpoint: url.clone(),
                uses_https: true,
                host_allowlist: None,
                ..BidderInfo::default()
            };
            let err = validate_endpoint(&url, &info)
                .expect_err("expected reject for IPv4 reserved literal");
            assert_eq!(err.code, 5, "literal {literal}");
        }
    }

    #[cfg(feature = "insecure-bidders")]
    #[test]
    fn loopback_allowed_under_feature() {
        let info = BidderInfo {
            name: "fake_ddddd".into(),
            endpoint: "http://127.0.0.1:8787/openrtb2/auction".into(),
            uses_https: false,
            host_allowlist: None,
            ..BidderInfo::default()
        };
        validate_endpoint("http://127.0.0.1:8787/openrtb2/auction", &info).unwrap();
    }
}

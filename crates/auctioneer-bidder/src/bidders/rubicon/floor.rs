use crate::BidderRequestInfo;
use auctioneer_openrtb::types::bid_request::Imp;

pub(super) fn floor(
    imp: &Imp,
    info: &BidderRequestInfo<'_>,
) -> Result<(Option<f64>, Option<&'static str>), ()> {
    let amount = imp.bidfloor.unwrap_or_default();
    let code = imp.bidfloorcur.as_deref().unwrap_or("");
    if !amount.is_finite() {
        return Err(());
    }
    if amount > 0.0_f64 && !code.is_empty() && !code.eq_ignore_ascii_case("USD") {
        info.floor_currency
            .convert_floor_to_usd(amount, code)
            .map(|floor| (Some(floor), Some("USD")))
            .map_err(|_error| ())
    } else {
        Ok((
            imp.bidfloor,
            (amount >= 0.0_f64 && !code.is_empty()).then_some("USD"),
        ))
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::FloorConversion;
    use crate::bidders::harness;
    use crate::bidders::rubicon::test_support::{
        assert_credential_eligibility, banner, body, context, outbound, request,
    };
    use crate::bidders::rubicon::{RubiconBidder, tests as conformance};
    use auctioneer_config_data::currency::FloorCurrencyConfig;
    use serde_json::{Value, json};

    fn floor_snapshot(rate: f64) -> FloorCurrencyConfig {
        serde_json::from_value(json!({"expires_at":2000_i64,"usd_per_unit":{"QAA":rate}})).unwrap()
    }

    fn foreign_impression(amount: f64, code: &str) -> Value {
        let mut imp = banner("foreign");
        imp["bidfloor"] = json!(amount);
        imp["bidfloorcur"] = json!(code);
        imp
    }

    fn assert_floor_sibling_rescue(amount: f64, code: &str, conversion: FloorConversion<'_>) {
        for invalid_first in [false, true] {
            let invalid = foreign_impression(amount, code);
            let valid = banner("valid");
            let imps = if invalid_first {
                json!([invalid, valid])
            } else {
                json!([valid, invalid])
            };
            assert_credential_eligibility(&request(&imps), conversion, true);
        }
    }

    #[test]
    fn credential_eligibility_foreign_floor_without_snapshot_is_rejected() {
        assert_credential_eligibility(
            &request(&json!([foreign_impression(1.0_f64, "QAA")])),
            FloorConversion::default(),
            false,
        );
    }

    #[test]
    fn credential_eligibility_foreign_floor_with_expired_snapshot_is_rejected() {
        let snapshot = floor_snapshot(1.25_f64);
        assert_credential_eligibility(
            &request(&json!([foreign_impression(1.0_f64, "QAA")])),
            FloorConversion::new(Some(&snapshot), 2000),
            false,
        );
    }

    #[test]
    fn credential_eligibility_foreign_floor_without_matching_rate_is_rejected() {
        let snapshot = floor_snapshot(1.25_f64);
        assert_credential_eligibility(
            &request(&json!([foreign_impression(1.0_f64, "QAB")])),
            FloorConversion::new(Some(&snapshot), 1000),
            false,
        );
    }

    #[test]
    fn credential_eligibility_foreign_floor_overflow_is_rejected() {
        let snapshot = floor_snapshot(f64::MAX);
        assert_credential_eligibility(
            &request(&json!([foreign_impression(2.0_f64, "QAA")])),
            FloorConversion::new(Some(&snapshot), 1000),
            false,
        );
    }

    #[test]
    fn credential_eligibility_foreign_floor_positive_underflow_is_rejected() {
        let snapshot = floor_snapshot(0.25_f64);
        assert_credential_eligibility(
            &request(&json!([foreign_impression(f64::from_bits(1), "QAA")])),
            FloorConversion::new(Some(&snapshot), 1000),
            false,
        );
    }

    #[test]
    fn credential_eligibility_foreign_floor_with_invalid_snapshot_is_rejected() {
        let snapshot = floor_snapshot(0.0_f64);
        assert_credential_eligibility(
            &request(&json!([foreign_impression(1.0_f64, "QAA")])),
            FloorConversion::new(Some(&snapshot), 1000),
            false,
        );
    }

    #[test]
    fn credential_eligibility_foreign_floor_with_malformed_currency_is_rejected() {
        let snapshot = floor_snapshot(1.25_f64);
        assert_credential_eligibility(
            &request(&json!([foreign_impression(1.0_f64, "BAD-CODE")])),
            FloorConversion::new(Some(&snapshot), 1000),
            false,
        );
    }

    #[test]
    fn credential_eligibility_nonfinite_typed_floor_is_rejected() {
        for amount in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut request = request(&json!([banner("invalid")]));
            request.imp[0].bidfloor = Some(amount);
            request.imp[0].bidfloorcur = Some("QAA".into());
            assert_credential_eligibility(&request, FloorConversion::default(), false);
            assert_eq!(request.imp[0].bidfloor.unwrap().to_bits(), amount.to_bits());
        }
    }

    #[test]
    fn credential_eligibility_valid_sibling_rescues_each_unusable_foreign_floor() {
        let ordinary = floor_snapshot(1.25_f64);
        let overflow = floor_snapshot(f64::MAX);
        let underflow = floor_snapshot(0.25_f64);
        let invalid = floor_snapshot(0.0_f64);
        for (amount, code, conversion) in [
            (1.0_f64, "QAA", FloorConversion::default()),
            (1.0_f64, "QAA", FloorConversion::new(Some(&ordinary), 2000)),
            (1.0_f64, "QAB", FloorConversion::new(Some(&ordinary), 1000)),
            (2.0_f64, "QAA", FloorConversion::new(Some(&overflow), 1000)),
            (
                f64::from_bits(1),
                "QAA",
                FloorConversion::new(Some(&underflow), 1000),
            ),
            (1.0_f64, "QAA", FloorConversion::new(Some(&invalid), 1000)),
            (
                1.0_f64,
                "BAD-CODE",
                FloorConversion::new(Some(&ordinary), 1000),
            ),
        ] {
            assert_floor_sibling_rescue(amount, code, conversion);
        }
    }

    #[test]
    fn credential_eligibility_valid_floor_controls_vary_expiry_rate_amount_and_currency() {
        let ordinary = floor_snapshot(1.25_f64);
        let large = floor_snapshot(f64::MAX);
        let small = floor_snapshot(0.25_f64);
        for (amount, code, conversion) in [
            (1.0_f64, "QAA", FloorConversion::new(Some(&ordinary), 1999)),
            (1.0_f64, "qaa", FloorConversion::new(Some(&ordinary), 1000)),
            (0.5_f64, "QAA", FloorConversion::new(Some(&large), 1000)),
            (
                f64::from_bits(4),
                "QAA",
                FloorConversion::new(Some(&small), 1000),
            ),
            (1.0_f64, "USD", FloorConversion::default()),
            (0.0_f64, "QAA", FloorConversion::default()),
            (-1.0_f64, "QAA", FloorConversion::default()),
        ] {
            assert_credential_eligibility(
                &request(&json!([foreign_impression(amount, code)])),
                conversion,
                true,
            );
        }
    }

    #[test]
    fn floor_conversion_uses_the_borrowed_snapshot_per_impression() {
        let snapshot = serde_json::from_value(json!({"expires_at":2000_i64,
            "usd_per_unit":{"EUR":1.2_f64}}))
        .unwrap();
        let mut imp = banner("i");
        imp["bidfloor"] = json!(1.0_f64);
        imp["bidfloorcur"] = json!("EUR");
        let request = request(&json!([imp]));
        let before = serde_json::to_value(&request).unwrap();
        conformance::with_source_context(|source| {
            let mut info = context();
            info.set_settings(Some("rubicon"), source.settings(), source.base_settings())
                .unwrap();
            info.credentials = source.credentials;
            info.floor_currency = FloorConversion::new(Some(&snapshot), 1000);
            let calls = harness::drive_ok(&RubiconBidder, &request, &info);
            assert_eq!(calls.len(), 1);
            assert_eq!(
                body(&calls[0])["imp"][0]["bidfloor"].as_f64(),
                Some(1.2_f64)
            );
            assert_eq!(body(&calls[0])["imp"][0]["bidfloorcur"], "USD");
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn zero_negative_empty_and_ascii_usd_floors_preserve_source_conversion_scope() {
        for (floor, currency, expected_currency) in [
            (0.0_f64, "EUR", "USD"),
            (-1.0_f64, "CZK", "CZK"),
            (1.0_f64, "", ""),
            (1.0_f64, "usd", "USD"),
        ] {
            let mut imp = banner("i");
            imp["bidfloor"] = json!(floor);
            imp["bidfloorcur"] = json!(currency);
            let calls = outbound(&request(&json!([imp])));
            assert_eq!(calls.len(), 1);
            assert_eq!(body(&calls[0])["imp"][0]["bidfloor"].as_f64(), Some(floor));
            assert_eq!(
                body(&calls[0])["imp"][0]["bidfloorcur"]
                    .as_str()
                    .unwrap_or(""),
                expected_currency
            );
        }
    }
}

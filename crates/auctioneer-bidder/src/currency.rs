//! Borrowed floor-conversion context; source protocols choose error disposition.

use auctioneer_config_data::currency::FloorCurrencyConfig;
use std::str::from_utf8;

/// Floor-specific refusal reasons, without request/impression error policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloorConversionError {
    /// The floor is negative or nonfinite.
    InvalidFloor,
    /// The caller did not supply exactly three ASCII letters.
    InvalidCurrency,
    /// No operator snapshot was supplied for a foreign floor.
    MissingSnapshot,
    /// The supplied handler time is at or past the snapshot's expiry.
    ExpiredSnapshot,
    /// The snapshot fails the independent config shape/count checks.
    InvalidSnapshot,
    /// The valid snapshot has no rate for this foreign currency.
    MissingRate,
    /// Multiplication produced a nonfinite result.
    Overflow,
    /// A strictly positive floor rounded to zero during multiplication.
    Underflow,
}

/// Floor-only view borrowing the existing operator-config owner.
///
/// Construction validates at most 256 rates once; each conversion does a
/// borrowed map lookup without copying rates or allocating normalization text.
/// Default is USD identity only. This is not response-currency conversion.
#[derive(Clone, Copy, Debug)]
pub struct FloorConversion<'req> {
    snapshot: Result<&'req FloorCurrencyConfig, FloorConversionError>,
}

impl Default for FloorConversion<'_> {
    fn default() -> Self {
        Self {
            snapshot: Err(FloorConversionError::MissingSnapshot),
        }
    }
}

impl<'req> FloorConversion<'req> {
    /// Borrow a snapshot using the current handler instant as Unix seconds.
    ///
    /// Invalid or expired snapshots remain refusals for foreign floors, without
    /// preventing USD identity. No clock reads, rate clones or external feeds.
    #[must_use]
    pub fn new(configured: Option<&'req FloorCurrencyConfig>, now_unix_seconds: i64) -> Self {
        let snapshot = configured
            .ok_or(FloorConversionError::MissingSnapshot)
            .and_then(|snapshot| {
                snapshot
                    .validate()
                    .map_err(|_reason| FloorConversionError::InvalidSnapshot)?;
                if now_unix_seconds >= snapshot.expires_at {
                    return Err(FloorConversionError::ExpiredSnapshot);
                }
                Ok(snapshot)
            });
        Self { snapshot }
    }

    /// Convert a finite, nonnegative floor, normalizing three ASCII letters on stack.
    ///
    /// ASCII-case-equivalent `USD` is returned bit-for-bit unchanged, independently of snapshot
    /// availability or validity. Foreign floors require a valid, fresh rate,
    /// even when the input is zero. The protocol caller chooses whether any
    /// refusal is request-fatal or local to one impression.
    ///
    /// # Errors
    /// Refuses invalid inputs, unusable snapshots, missing rates, overflow,
    /// or strictly positive multiplication underflow to zero.
    #[expect(
        clippy::float_arithmetic,
        reason = "floor conversion multiplies finite values and rejects overflow and positive underflow"
    )]
    pub fn convert_floor_to_usd(
        &self,
        amount: f64,
        currency: &str,
    ) -> Result<f64, FloorConversionError> {
        if !amount.is_finite() || amount < 0.0_f64 {
            return Err(FloorConversionError::InvalidFloor);
        }
        let code: &[u8; 3] = currency
            .as_bytes()
            .try_into()
            .map_err(|_length| FloorConversionError::InvalidCurrency)?;
        if !code.iter().all(u8::is_ascii_alphabetic) {
            return Err(FloorConversionError::InvalidCurrency);
        }
        let normalized = code.map(|byte| byte.to_ascii_uppercase());
        if &normalized == b"USD" {
            return Ok(amount);
        }
        let normalized_code =
            from_utf8(&normalized).map_err(|_ascii| FloorConversionError::InvalidCurrency)?;

        let snapshot = self.snapshot?;
        let rate = snapshot
            .usd_per_unit
            .get(normalized_code)
            .ok_or(FloorConversionError::MissingRate)?;
        let converted = amount * rate;
        if !converted.is_finite() {
            return Err(FloorConversionError::Overflow);
        }
        if amount > 0.0_f64 && converted == 0.0_f64 {
            return Err(FloorConversionError::Underflow);
        }
        Ok(converted)
    }
}

#[cfg(test)]
mod tests {
    use super::{FloorConversion, FloorConversionError};
    use auctioneer_config_data::currency::FloorCurrencyConfig;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn snapshot(rate: f64) -> FloorCurrencyConfig {
        FloorCurrencyConfig {
            expires_at: 2_000,
            usd_per_unit: BTreeMap::from([("QAA".to_owned(), rate)]),
        }
    }

    #[test]
    fn floor_currency_default_is_exact_usd_identity_without_a_snapshot() {
        let conversion = FloorConversion::default();
        for amount in [
            0.0_f64,
            -0.0_f64,
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            2.5_f64,
            f64::MAX,
        ] {
            let converted = conversion.convert_floor_to_usd(amount, "USD").unwrap();
            assert_eq!(converted.to_bits(), amount.to_bits());
        }
        assert_eq!(
            conversion.convert_floor_to_usd(2.0, "QAA"),
            Err(FloorConversionError::MissingSnapshot)
        );
        assert_eq!(
            FloorConversion::new(None, 1_000).convert_floor_to_usd(2.0, "QAA"),
            Err(FloorConversionError::MissingSnapshot)
        );
    }

    #[test]
    fn floor_currency_uses_usd_per_unit_in_the_multiplication_direction() {
        let config = snapshot(2.5);
        let conversion = FloorConversion::new(Some(&config), 1_000);
        assert_eq!(conversion.convert_floor_to_usd(4.0, "QAA"), Ok(10.0_f64));
        let cheaper = snapshot(0.25);
        let cheaper_conversion = FloorConversion::new(Some(&cheaper), 1_000);
        assert_eq!(
            cheaper_conversion.convert_floor_to_usd(4.0, "QAA"),
            Ok(1.0_f64)
        );
    }

    #[test]
    fn floor_currency_never_invents_a_missing_foreign_rate_even_for_zero() {
        let config = snapshot(2.5);
        let conversion = FloorConversion::new(Some(&config), 1_000);
        for amount in [0.0_f64, 2.0_f64] {
            assert_eq!(
                conversion.convert_floor_to_usd(amount, "QAB"),
                Err(FloorConversionError::MissingRate)
            );
        }
        let empty = FloorCurrencyConfig {
            expires_at: 2_000,
            usd_per_unit: BTreeMap::new(),
        };
        assert_eq!(
            FloorConversion::new(Some(&empty), 1_000).convert_floor_to_usd(2.0, "QAA"),
            Err(FloorConversionError::MissingRate)
        );
    }

    #[test]
    fn floor_currency_expiry_is_exclusive_at_the_supplied_handler_second() {
        let config = snapshot(2.5);
        assert_eq!(
            FloorConversion::new(Some(&config), 1_999).convert_floor_to_usd(4.0, "QAA"),
            Ok(10.0_f64)
        );
        for now in [2_000, 2_001, i64::MAX] {
            let conversion = FloorConversion::new(Some(&config), now);
            for amount in [0.0_f64, 4.0_f64] {
                assert_eq!(
                    conversion.convert_floor_to_usd(amount, "QAA"),
                    Err(FloorConversionError::ExpiredSnapshot)
                );
            }
            assert_eq!(conversion.convert_floor_to_usd(4.0, "USD"), Ok(4.0_f64));
        }
    }

    #[test]
    fn floor_currency_rejects_invalid_programmatic_snapshots_but_preserves_usd() {
        let mut invalid = Vec::new();
        for rate in [
            0.0_f64,
            -0.0_f64,
            -2.0_f64,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            invalid.push(snapshot(rate));
        }
        for code in ["qaa", "QA", "Q1A", "USD"] {
            let mut config = snapshot(2.5);
            config.usd_per_unit.insert(code.to_owned(), 1.0_f64);
            invalid.push(config);
        }
        let mut expired = snapshot(2.5);
        expired.expires_at = 0;
        invalid.push(expired);
        let mut nonfinite = snapshot(2.5);
        nonfinite.usd_per_unit.insert("QAB".to_owned(), f64::NAN);
        invalid.push(nonfinite);

        for config in &invalid {
            let conversion = FloorConversion::new(Some(config), 1_000);
            assert_eq!(
                conversion.convert_floor_to_usd(4.0, "QAA"),
                Err(FloorConversionError::InvalidSnapshot)
            );
            assert_eq!(conversion.convert_floor_to_usd(4.0, "USD"), Ok(4.0_f64));
        }
    }

    #[test]
    fn floor_currency_view_also_refuses_a_programmatic_257_entry_snapshot() {
        let mut config = snapshot(2.5);
        for index in 0_usize..257 {
            let code = String::from_utf8(vec![
                b'Q',
                b'A'.checked_add(u8::try_from(index.checked_div(26).unwrap()).unwrap())
                    .unwrap(),
                b'A'.checked_add(u8::try_from(index.checked_rem(26).unwrap()).unwrap())
                    .unwrap(),
            ])
            .unwrap();
            config.usd_per_unit.insert(code, 1.0_f64);
        }
        assert_eq!(config.usd_per_unit.len(), 257);
        let conversion = FloorConversion::new(Some(&config), 1_000);
        assert_eq!(
            conversion.convert_floor_to_usd(4.0, "QAA"),
            Err(FloorConversionError::InvalidSnapshot)
        );
        assert_eq!(conversion.convert_floor_to_usd(4.0, "USD"), Ok(4.0_f64));
    }

    #[test]
    fn floor_currency_normalizes_three_ascii_letters_without_allocating() {
        let config = snapshot(2.5);
        let conversion = FloorConversion::new(Some(&config), 1_000);
        for code in ["QAA", "qaa", "Qaa", "qAa"] {
            assert_eq!(conversion.convert_floor_to_usd(4.0, code), Ok(10.0_f64));
        }
        for code in ["USD", "usd", "Usd", "uSd"] {
            assert_eq!(
                FloorConversion::default().convert_floor_to_usd(4.0, code),
                Ok(4.0_f64)
            );
        }
    }

    #[test]
    fn floor_currency_rejects_codes_other_than_three_ascii_letters() {
        let config = snapshot(2.5);
        let conversion = FloorConversion::new(Some(&config), 1_000);
        for code in ["", " USD", "USD ", "Q1A", "QAAA", "Q\u{e9}A"] {
            assert_eq!(
                conversion.convert_floor_to_usd(4.0, code),
                Err(FloorConversionError::InvalidCurrency)
            );
        }
    }

    #[test]
    fn floor_currency_rejects_negative_and_nonfinite_floors_including_usd() {
        let config = snapshot(2.5);
        let conversion = FloorConversion::new(Some(&config), 1_000);
        for currency in ["USD", "QAA"] {
            for amount in [
                -1.0_f64,
                -f64::from_bits(1),
                f64::NAN,
                f64::INFINITY,
                f64::NEG_INFINITY,
            ] {
                assert_eq!(
                    conversion.convert_floor_to_usd(amount, currency),
                    Err(FloorConversionError::InvalidFloor)
                );
            }
        }
    }

    #[test]
    fn floor_currency_refuses_overflow_instead_of_returning_infinity() {
        let config = snapshot(2.0);
        assert_eq!(
            FloorConversion::new(Some(&config), 1_000).convert_floor_to_usd(f64::MAX, "QAA"),
            Err(FloorConversionError::Overflow)
        );
    }

    #[test]
    fn floor_currency_refuses_positive_underflow_to_zero() {
        let config = snapshot(0.5);
        assert_eq!(
            FloorConversion::new(Some(&config), 1_000)
                .convert_floor_to_usd(f64::from_bits(1), "QAA"),
            Err(FloorConversionError::Underflow)
        );
    }

    #[test]
    fn floor_currency_allows_nonzero_subnormals_and_actual_zero() {
        let config = snapshot(0.5);
        let conversion = FloorConversion::new(Some(&config), 1_000);
        assert_eq!(
            conversion.convert_floor_to_usd(f64::from_bits(2), "QAA"),
            Ok(f64::from_bits(1))
        );
        assert_eq!(conversion.convert_floor_to_usd(0.0, "QAA"), Ok(0.0_f64));
    }

    #[test]
    fn floor_currency_view_is_copyable_without_owning_the_rate_map() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<FloorConversion<'_>>();
        let config: FloorCurrencyConfig = serde_json::from_value(json!({
            "expires_at": 2_000_i32,
            "usd_per_unit": {"QAA": 2.5_f64}
        }))
        .unwrap();
        let first = FloorConversion::new(Some(&config), 1_000);
        let second = first;
        assert_eq!(first.convert_floor_to_usd(4.0, "QAA"), Ok(10.0_f64));
        assert_eq!(second.convert_floor_to_usd(4.0, "QAA"), Ok(10.0_f64));
    }
}

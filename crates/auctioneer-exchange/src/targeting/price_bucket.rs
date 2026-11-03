//! Price granularity + bucket computation.
//!
//! The bucket math is inherently f64 arithmetic — the formula is
//! defined in floating-point CPM. Silence the `restriction`
//! float-arithmetic lint at the module boundary rather than at every
//! division / multiplication site. `expect` rather than `allow` so the
//! escape hatch fails the build if the module ever stops needing it.
#![expect(
    clippy::float_arithmetic,
    reason = "price-bucket math per spec Appendix E.2"
)]

use crate::response::ErrorEntry;
use auctioneer_config::types::AccountConfig;
use auctioneer_openrtb::WarningCode;
use auctioneer_runtime::{Rng, gen_range};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidRounding {
    Down,
    Up,
    Nearest,
    TimeSplit,
}

#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub increment: f64,
}

#[derive(Clone, Debug)]
pub enum PriceGranularity {
    Low,
    Med,
    High,
    Auto,
    Dense,
    Custom { precision: u8, ranges: Vec<Range> },
}

/// Fixed request settings that can emit a price-granularity warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceGranularityScope {
    Global,
    Banner,
    Video,
    Native,
    Audio,
}

impl PriceGranularityScope {
    fn field_path(self) -> &'static str {
        match self {
            Self::Global => "ext.prebid.targeting.pricegranularity",
            Self::Banner => "ext.prebid.targeting.mediatypepricegranularity.banner",
            Self::Video => "ext.prebid.targeting.mediatypepricegranularity.video",
            Self::Native => "ext.prebid.targeting.mediatypepricegranularity.native",
            Self::Audio => "ext.prebid.targeting.mediatypepricegranularity.audio",
        }
    }
}

impl PriceGranularity {
    /// Account-level fallback. Account `targeting.pricegranularity`
    /// is not modeled by `AccountConfig`, so the account fallback is `Med`.
    /// Request-level granularity is resolved separately.
    #[must_use]
    pub fn from_account(_account: &AccountConfig) -> Self {
        Self::Med
    }

    fn ranges(&self) -> (u8, Vec<Range>) {
        match self {
            Self::Low => (
                2,
                vec![Range {
                    min: 0.0,
                    max: 5.0,
                    increment: 0.5,
                }],
            ),
            Self::Med => (
                2,
                vec![Range {
                    min: 0.0,
                    max: 20.0,
                    increment: 0.1,
                }],
            ),
            Self::High => (
                2,
                vec![Range {
                    min: 0.0,
                    max: 20.0,
                    increment: 0.01,
                }],
            ),
            Self::Auto => (
                2,
                vec![
                    Range {
                        min: 0.0,
                        max: 5.0,
                        increment: 0.05,
                    },
                    Range {
                        min: 5.0,
                        max: 10.0,
                        increment: 0.1,
                    },
                    Range {
                        min: 10.0,
                        max: 20.0,
                        increment: 0.5,
                    },
                ],
            ),
            Self::Dense => (
                2,
                vec![
                    Range {
                        min: 0.0,
                        max: 3.0,
                        increment: 0.01,
                    },
                    Range {
                        min: 3.0,
                        max: 8.0,
                        increment: 0.05,
                    },
                    Range {
                        min: 8.0,
                        max: 20.0,
                        increment: 0.5,
                    },
                ],
            ),
            Self::Custom { precision, ranges } if valid_custom_ranges(ranges) => {
                ((*precision).min(15), ranges.clone())
            }
            Self::Custom { .. } => Self::Med.ranges(),
        }
    }
}

fn valid_custom_ranges(ranges: &[Range]) -> bool {
    if ranges.is_empty() {
        return false;
    }
    let mut previous_max = None;
    for range in ranges {
        if !range.min.is_finite()
            || !range.max.is_finite()
            || range.min < 0.0_f64
            || range.max <= 0.0_f64
            || range.min > range.max
            || !range.increment.is_normal()
            || range.increment <= 0.0_f64
        {
            return false;
        }
        if previous_max.is_some_and(|max| range.max <= max || !range.min.total_cmp(&max).is_eq()) {
            return false;
        }
        // Validate the largest quotient and the furthest upward-rounded
        // bucket, not just the input operands: finite division can overflow.
        let increments = (range.max - range.min) / range.increment;
        if !increments.is_finite()
            || !increments
                .ceil()
                .mul_add(range.increment, range.min)
                .is_finite()
        {
            return false;
        }
        previous_max = Some(range.max);
    }
    true
}

#[must_use]
pub fn price_bucket(
    cpm: f64,
    granularity: &PriceGranularity,
    mode: BidRounding,
    rng: &dyn Rng,
) -> String {
    if cpm.is_nan() || cpm < 0.0_f64 {
        return String::new();
    }
    let (precision, ranges) = granularity.ranges();
    let bucket_max = ranges
        .iter()
        .map(|r| r.max)
        .fold(f64::NEG_INFINITY, f64::max);
    if cpm > bucket_max {
        return format_at(bucket_max, precision);
    }
    // Find the range with min <= cpm <= max. On exact boundary the
    // later range wins.
    let mut chosen: Option<Range> = None;
    for range in &ranges {
        if cpm >= range.min && cpm <= range.max {
            chosen = Some(*range);
        }
    }
    let Some(range) = chosen else {
        return String::new();
    };
    let increments = (cpm - range.min) / range.increment;
    if !increments.is_finite() {
        return price_bucket(cpm, &PriceGranularity::Med, mode, rng);
    }
    let rounded = match mode {
        BidRounding::Down => increments.floor(),
        BidRounding::Up => increments.ceil(),
        BidRounding::Nearest => increments.round(),
        BidRounding::TimeSplit => {
            let Some(two) = NonZeroU64::new(2) else {
                return String::new();
            };
            if gen_range(rng, two) == 0 {
                increments.floor()
            } else {
                increments.ceil()
            }
        }
    };
    let bucket = rounded.mul_add(range.increment, range.min);
    if !bucket.is_finite() {
        return price_bucket(cpm, &PriceGranularity::Med, mode, rng);
    }
    format_at(bucket, precision)
}

fn format_at(value: f64, precision: u8) -> String {
    format!("{:.*}", usize::from(precision), value)
}

impl BidRounding {
    /// Account-level fallback — the `bid_rounding` string on
    /// `AccountConfig` (already shipped on foundations) maps to one of `Down` /
    /// `Up` / `Nearest` / `TimeSplit`. Unknown values fall back to
    /// `Down`.
    #[must_use]
    pub fn from_account(account: &AccountConfig) -> Self {
        match account.bid_rounding.to_ascii_lowercase().as_str() {
            "up" | "ceil" => Self::Up,
            // The legacy boolean `true` maps to nearest.
            "nearest" | "round" | "true" => Self::Nearest,
            "timesplit" | "time_split" => Self::TimeSplit,
            _ => Self::Down,
        }
    }
}

/// Parse request-level `pricegranularity` JSON.
///
/// String aliases: `low | med | medium | high | auto | dense` —
/// case-insensitive; `med` and `medium` interchangeable. Custom-bucket
/// shape `{"precision": N, "ranges": [...]}` with precision clamped to
/// `0..=15`, strictly-increasing `max`, and missing `min` defaulting
/// to the previous range's `max` (NOT a blanket 0.0). Retained ranges
/// must meet at that exact boundary, with finite, nonnegative bounds
/// and positive normal increments whose bucket arithmetic is finite.
/// Malformed rows are dropped without advancing the implicit minimum;
/// an explicitly invalid `min` is not treated as an omitted one.
/// Invalid topology or arithmetic defaults the entire config to `Med`.
/// The first range may start above zero, including equal bounds.
/// Each fallback appends one fixed, input-free warning for the setting.
/// Tolerated row drops that leave a valid custom config do not warn.
/// Resolve each of the five supported settings once per auction, not per bid.
#[must_use]
pub fn parse_price_granularity(
    value: &serde_json::Value,
    scope: PriceGranularityScope,
    warnings: &mut Vec<ErrorEntry>,
) -> PriceGranularity {
    parse_price_granularity_value(value).unwrap_or_else(|| {
        warnings.push(ErrorEntry {
            code: WarningCode::InvalidPriceGranularity.as_u32(),
            message: format!("invalid {}; falling back to Med", scope.field_path()),
        });
        PriceGranularity::Med
    })
}

fn parse_price_granularity_value(value: &serde_json::Value) -> Option<PriceGranularity> {
    if let Some(name) = value.as_str() {
        return match name.to_ascii_lowercase().as_str() {
            "low" => Some(PriceGranularity::Low),
            "med" | "medium" => Some(PriceGranularity::Med),
            "high" => Some(PriceGranularity::High),
            "auto" => Some(PriceGranularity::Auto),
            "dense" => Some(PriceGranularity::Dense),
            _ => None,
        };
    }
    let obj = value.as_object()?;
    let precision = obj
        .get("precision")
        .and_then(serde_json::Value::as_u64)
        .map_or(2_u8, |n| u8::try_from(n.min(15)).unwrap_or(2_u8));
    let ranges_json = obj.get("ranges").and_then(serde_json::Value::as_array)?;
    let mut ranges: Vec<Range> = Vec::new();
    let mut previous_max = 0.0_f64;
    for entry in ranges_json {
        let Some(row) = entry.as_object() else {
            continue;
        };
        let max = row
            .get("max")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0_f64);
        let inc = row
            .get("increment")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0_f64);
        if max <= 0.0_f64 || inc <= 0.0_f64 {
            continue;
        }
        if max <= previous_max {
            continue;
        }
        let min = match row.get("min").map(serde_json::Value::as_f64) {
            None => previous_max,
            Some(Some(min)) => min,
            Some(None) => continue,
        };
        if !min.is_finite() || min < 0.0_f64 || min > max {
            continue;
        }
        ranges.push(Range {
            min,
            max,
            increment: inc,
        });
        previous_max = max;
    }
    if !valid_custom_ranges(&ranges) {
        return None;
    }
    Some(PriceGranularity::Custom { precision, ranges })
}

#[cfg(test)]
mod tests {
    use super::{BidRounding, PriceGranularity, PriceGranularityScope, Range, price_bucket};
    use crate::response::ErrorEntry;
    use auctioneer_openrtb::WarningCode;
    use auctioneer_runtime::SeededRng;

    fn rng() -> SeededRng {
        SeededRng::new(1)
    }

    fn parse_price_granularity(value: &serde_json::Value) -> PriceGranularity {
        super::parse_price_granularity_value(value).unwrap_or(PriceGranularity::Med)
    }

    #[test]
    fn fallback_diagnostics_are_fixed_scoped_and_preserve_existing_warnings() {
        for scope in [
            PriceGranularityScope::Global,
            PriceGranularityScope::Banner,
            PriceGranularityScope::Video,
            PriceGranularityScope::Native,
            PriceGranularityScope::Audio,
        ] {
            let earlier = ErrorEntry {
                code: WarningCode::Unknown.as_u32(),
                message: "earlier warning".into(),
            };
            for value in [
                serde_json::json!(null),
                serde_json::json!("private-granularity-marker".repeat(256)),
                serde_json::json!({"ranges": [], "payload": "private-granularity-marker"}),
            ] {
                let mut warnings = vec![earlier.clone()];
                let granularity = super::parse_price_granularity(&value, scope, &mut warnings);
                assert!(matches!(granularity, PriceGranularity::Med));
                assert_eq!(warnings.len(), 2);
                assert_eq!(warnings[0], earlier);
                assert_eq!(
                    warnings[1].code,
                    WarningCode::InvalidPriceGranularity.as_u32()
                );
                assert_eq!(
                    warnings[1].message,
                    format!("invalid {}; falling back to Med", scope.field_path()),
                );
                assert!(warnings[1].message.len() <= 384);
                assert!(!warnings[1].message.contains("private-granularity-marker"));
            }
        }
    }

    fn dropped_middle_rows() -> Vec<(&'static str, serde_json::Value)> {
        let mut cases = vec![
            ("null row", serde_json::json!(null)),
            ("boolean row", serde_json::json!(true)),
            ("number row", serde_json::json!(5_u32)),
            ("string row", serde_json::json!("invalid")),
            ("array row", serde_json::json!([])),
            ("missing max", serde_json::json!({"increment": 0.5_f64})),
            (
                "zero max",
                serde_json::json!({"max": 0.0_f64, "increment": 0.5_f64}),
            ),
            (
                "negative max",
                serde_json::json!({"max": -1.0_f64, "increment": 0.5_f64}),
            ),
            (
                "equal max",
                serde_json::json!({"max": 2.0_f64, "increment": 0.5_f64}),
            ),
            (
                "decreasing max",
                serde_json::json!({"max": 1.0_f64, "increment": 0.5_f64}),
            ),
            ("missing increment", serde_json::json!({"max": 5.0_f64})),
            (
                "zero increment",
                serde_json::json!({"max": 5.0_f64, "increment": 0.0_f64}),
            ),
            (
                "negative increment",
                serde_json::json!({"max": 5.0_f64, "increment": -0.5_f64}),
            ),
            (
                "negative min",
                serde_json::json!({"min": -1.0_f64, "max": 5.0_f64, "increment": 0.5_f64}),
            ),
            (
                "reversed bounds",
                serde_json::json!({"min": 6.0_f64, "max": 5.0_f64, "increment": 0.5_f64}),
            ),
        ];
        for field in ["max", "increment", "min"] {
            for value in [
                serde_json::json!(null),
                serde_json::json!("invalid"),
                serde_json::json!(true),
                serde_json::json!([]),
                serde_json::json!({}),
            ] {
                let mut row = serde_json::json!({"max": 5.0_f64, "increment": 0.5_f64});
                row[field] = value;
                cases.push((field, row));
            }
        }
        cases
    }

    #[test]
    fn dropped_middle_rows_preserve_valid_siblings_and_the_implicit_minimum() {
        let expected = PriceGranularity::Custom {
            precision: 2,
            ranges: vec![
                Range {
                    min: 0.0,
                    max: 2.0,
                    increment: 0.01,
                },
                Range {
                    min: 2.0,
                    max: 3.0,
                    increment: 0.25,
                },
            ],
        };
        for (label, row) in dropped_middle_rows() {
            let input = serde_json::json!({
                "ranges": [
                    {"max": 2.0_f64, "increment": 0.01_f64},
                    row,
                    {"max": 3.0_f64, "increment": 0.25_f64},
                ],
            });
            let granularity = parse_price_granularity(&input);
            let PriceGranularity::Custom { precision, ranges } = &granularity else {
                panic!("dropped {label} must preserve valid siblings: {input}");
            };
            assert_eq!(*precision, 2);
            assert_eq!(ranges.len(), 2, "{label}: {input}");
            for (range, (min, max, increment)) in ranges
                .iter()
                .zip([(0.0_f64, 2.0_f64, 0.01_f64), (2.0_f64, 3.0_f64, 0.25_f64)])
            {
                assert_eq!(range.min.to_bits(), min.to_bits(), "{label}: {input}");
                assert_eq!(range.max.to_bits(), max.to_bits(), "{label}: {input}");
                assert_eq!(
                    range.increment.to_bits(),
                    increment.to_bits(),
                    "{label}: {input}"
                );
            }
            assert_eq!(
                price_bucket(1.23, &granularity, BidRounding::Down, &rng()),
                "1.23"
            );
            for (mode, seed) in [
                (BidRounding::Down, 1_u64),
                (BidRounding::Up, 1_u64),
                (BidRounding::Nearest, 1_u64),
                (BidRounding::TimeSplit, 1_u64),
                (BidRounding::TimeSplit, 2_u64),
            ] {
                for cpm in [0.0_f64, 1.23_f64, 2.0_f64, 2.63_f64, 3.5_f64] {
                    assert_eq!(
                        price_bucket(cpm, &granularity, mode, &SeededRng::new(seed)),
                        price_bucket(cpm, &expected, mode, &SeededRng::new(seed)),
                        "{label}, {mode:?}, seed {seed}, cpm {cpm}: {input}",
                    );
                }
            }
        }
    }

    #[test]
    fn unsafe_retained_middle_topology_and_arithmetic_use_med_in_all_modes() {
        for (label, ranges) in [
            (
                "explicit gap after dropped increment",
                serde_json::json!([
                    {"max": 2.0_f64, "increment": 0.01_f64},
                    {"max": 5.0_f64, "increment": 0.0_f64},
                    {"min": 5.0_f64, "max": 6.0_f64, "increment": 0.25_f64},
                ]),
            ),
            (
                "explicit overlap after dropped max",
                serde_json::json!([
                    {"max": 2.0_f64, "increment": 0.01_f64},
                    {"max": 1.0_f64, "increment": 0.5_f64},
                    {"min": 1.0_f64, "max": 3.0_f64, "increment": 0.25_f64},
                ]),
            ),
            (
                "retained subnormal increment",
                serde_json::json!([
                    {"max": 2.0_f64, "increment": 0.01_f64},
                    {"max": 5.0_f64, "increment": f64::from_bits(1)},
                    {"max": 6.0_f64, "increment": 0.25_f64},
                ]),
            ),
            (
                "retained quotient overflow",
                serde_json::json!([
                    {"max": 2.0_f64, "increment": 0.01_f64},
                    {"max": 10.0_f64, "increment": f64::MIN_POSITIVE},
                    {"max": 12.0_f64, "increment": 0.25_f64},
                ]),
            ),
            (
                "retained upward bucket overflow",
                serde_json::json!([
                    {"max": f64::MAX / 2.0_f64, "increment": f64::MAX / 2.0_f64},
                    {"max": f64::MAX, "increment": f64::MAX},
                ]),
            ),
        ] {
            let input = serde_json::json!({"precision": 3_u32, "ranges": ranges});
            let granularity = parse_price_granularity(&input);
            assert!(
                matches!(granularity, PriceGranularity::Med),
                "{label}: {input}"
            );
            assert_eq!(
                price_bucket(1.23, &granularity, BidRounding::Down, &rng()),
                "1.20"
            );
            for (mode, seed) in [
                (BidRounding::Down, 1_u64),
                (BidRounding::Up, 1_u64),
                (BidRounding::Nearest, 1_u64),
                (BidRounding::TimeSplit, 1_u64),
                (BidRounding::TimeSplit, 2_u64),
            ] {
                for cpm in [1.23_f64, 2.63_f64, 5.5_f64, f64::INFINITY] {
                    assert_eq!(
                        price_bucket(cpm, &granularity, mode, &SeededRng::new(seed)),
                        price_bucket(cpm, &PriceGranularity::Med, mode, &SeededRng::new(seed)),
                        "{label}, {mode:?}, seed {seed}, cpm {cpm}: {input}",
                    );
                }
            }
        }
    }

    #[test]
    fn reversed_custom_range_falls_back_to_med_without_a_blank_bucket() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "precision": 3_u32,
            "ranges": [{"min": 5.0_f64, "max": 1.0_f64, "increment": 0.1_f64}],
        }));
        assert!(matches!(granularity, PriceGranularity::Med));
        assert_eq!(
            price_bucket(0.5_f64, &granularity, BidRounding::Down, &rng()),
            "0.50",
        );
    }

    #[test]
    fn reversed_custom_range_does_not_advance_the_next_implicit_minimum() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "precision": 3_u32,
            "ranges": [
                {"min": 6.0_f64, "max": 5.0_f64, "increment": 0.1_f64},
                {"max": 2.0_f64, "increment": 0.5_f64},
            ],
        }));
        let PriceGranularity::Custom { ranges, .. } = &granularity else {
            panic!("the valid range must survive");
        };
        assert_eq!(ranges.len(), 1);
        for (cpm, expected) in [(0.0_f64, "0.000"), (0.5_f64, "0.500"), (3.0_f64, "2.000")] {
            assert_eq!(
                price_bucket(cpm, &granularity, BidRounding::Down, &rng()),
                expected,
            );
        }
    }

    #[test]
    fn negative_custom_minimum_does_not_discard_valid_siblings() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "ranges": [
                {"max": 2.0_f64, "increment": 0.01_f64},
                {"min": -1.0_f64, "max": 3.0_f64, "increment": 0.1_f64},
            ],
        }));
        assert_eq!(
            price_bucket(1.23_f64, &granularity, BidRounding::Down, &rng()),
            "1.23",
        );
        let PriceGranularity::Custom { ranges, .. } = granularity else {
            panic!("the valid range must survive");
        };
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn malformed_custom_minimum_does_not_advance_the_next_implicit_minimum() {
        for min in [
            serde_json::json!(-1.0_f64),
            serde_json::json!("invalid"),
            serde_json::json!(null),
            serde_json::json!(true),
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            let granularity = parse_price_granularity(&serde_json::json!({
                "precision": 3_u32,
                "ranges": [
                    {"max": 1.0_f64, "increment": 0.01_f64},
                    {"min": min, "max": 5.0_f64, "increment": 0.5_f64},
                    {"max": 2.0_f64, "increment": 0.25_f64},
                ],
            }));
            let PriceGranularity::Custom { ranges, .. } = &granularity else {
                panic!("malformed minimum {min} must not discard valid ranges");
            };
            assert_eq!(ranges.len(), 2, "malformed minimum: {min}");
            let last = ranges.last().unwrap();
            assert_eq!(last.min.to_bits(), 1.0_f64.to_bits(), "minimum: {min}");
            assert_eq!(last.max.to_bits(), 2.0_f64.to_bits(), "minimum: {min}");
            assert_eq!(
                price_bucket(1.5_f64, &granularity, BidRounding::Down, &rng()),
                "1.500",
                "malformed minimum: {min}",
            );
        }
    }

    #[test]
    fn malformed_custom_minimum_uses_med_when_no_valid_ranges_remain() {
        for min in [serde_json::json!("invalid"), serde_json::json!(null)] {
            let granularity = parse_price_granularity(&serde_json::json!({
                "ranges": [{"min": min, "max": 2.0_f64, "increment": 0.01_f64}],
            }));
            assert!(matches!(granularity, PriceGranularity::Med));
            assert_eq!(
                price_bucket(1.23_f64, &granularity, BidRounding::Down, &rng()),
                "1.20",
            );
        }
    }

    #[test]
    fn parsed_custom_range_above_zero_preserves_no_match() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "precision": 3_u32,
            "ranges": [{"min": 1.0_f64, "max": 2.0_f64, "increment": 0.25_f64}],
        }));
        assert!(matches!(granularity, PriceGranularity::Custom { .. }));
        for mode in [
            BidRounding::Down,
            BidRounding::Up,
            BidRounding::Nearest,
            BidRounding::TimeSplit,
        ] {
            assert_eq!(price_bucket(0.5_f64, &granularity, mode, &rng()), "");
            assert_eq!(price_bucket(1.0_f64, &granularity, mode, &rng()), "1.000");
        }
    }

    #[test]
    fn custom_range_with_equal_bounds_remains_valid() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "precision": 3_u32,
            "ranges": [{"min": 1.0_f64, "max": 1.0_f64, "increment": 0.1_f64}],
        }));
        assert!(matches!(granularity, PriceGranularity::Custom { .. }));
        for mode in [
            BidRounding::Down,
            BidRounding::Up,
            BidRounding::Nearest,
            BidRounding::TimeSplit,
        ] {
            assert_eq!(price_bucket(0.5_f64, &granularity, mode, &rng()), "");
            assert_eq!(price_bucket(1.0_f64, &granularity, mode, &rng()), "1.000");
            assert_eq!(price_bucket(1.5_f64, &granularity, mode, &rng()), "1.000");
        }
    }

    #[test]
    fn unsafe_custom_increments_fall_back_to_med() {
        for (max, increment) in [(2.0_f64, f64::from_bits(1)), (f64::MAX, f64::MIN_POSITIVE)] {
            let granularity = parse_price_granularity(&serde_json::json!({
                "precision": 3_u32,
                "ranges": [{"max": max, "increment": increment}],
            }));
            assert!(matches!(granularity, PriceGranularity::Med));
            assert_eq!(
                price_bucket(0.75_f64, &granularity, BidRounding::Down, &rng()),
                "0.70",
            );
        }
    }

    #[test]
    fn overlapping_custom_ranges_fall_back_to_med() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "precision": 3_u32,
            "ranges": [
                {"max": 1.0_f64, "increment": 0.1_f64},
                {"min": 0.5_f64, "max": 2.0_f64, "increment": 0.5_f64},
            ],
        }));
        assert!(matches!(granularity, PriceGranularity::Med));
        assert_eq!(
            price_bucket(0.75_f64, &granularity, BidRounding::Down, &rng()),
            "0.70",
        );
    }

    #[test]
    fn interior_custom_range_gaps_fall_back_to_med() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "precision": 3_u32,
            "ranges": [
                {"max": 1.0_f64, "increment": 0.1_f64},
                {"min": 2.0_f64, "max": 3.0_f64, "increment": 0.5_f64},
            ],
        }));
        assert!(matches!(granularity, PriceGranularity::Med));
        assert_eq!(
            price_bucket(1.5_f64, &granularity, BidRounding::Down, &rng()),
            "1.50",
        );
    }

    #[test]
    fn negative_custom_minimum_falls_back_to_med() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "ranges": [{"min": -1.0_f64, "max": 2.0_f64, "increment": 0.5_f64}],
        }));
        assert!(matches!(granularity, PriceGranularity::Med));
    }

    #[test]
    fn exact_shared_boundary_uses_the_later_custom_range() {
        let granularity = parse_price_granularity(&serde_json::json!({
            "precision": 3_u32,
            "ranges": [
                {"max": 1.0_f64, "increment": 0.3_f64},
                {"max": 2.0_f64, "increment": 0.5_f64},
            ],
        }));
        assert!(matches!(granularity, PriceGranularity::Custom { .. }));
        assert_eq!(
            price_bucket(1.0_f64, &granularity, BidRounding::Down, &rng()),
            "1.000",
        );
    }

    #[test]
    fn a_custom_range_may_start_above_zero() {
        let granularity = PriceGranularity::Custom {
            precision: 3,
            ranges: vec![Range {
                min: 1.0,
                max: 1.0,
                increment: 0.1,
            }],
        };
        assert_eq!(
            price_bucket(0.5, &granularity, BidRounding::Down, &rng()),
            "",
        );
        assert_eq!(
            price_bucket(1.0, &granularity, BidRounding::Down, &rng()),
            "1.000",
        );
    }

    #[test]
    fn direct_custom_ranges_cannot_bypass_numeric_or_topology_validation() {
        let valid = Range {
            min: 0.0,
            max: 1.0,
            increment: 0.1,
        };
        let invalid_ranges = [
            vec![],
            vec![Range {
                increment: f64::from_bits(1),
                ..valid
            }],
            vec![Range {
                max: f64::MAX,
                increment: f64::MIN_POSITIVE,
                ..valid
            }],
            vec![Range { min: -1.0, ..valid }],
            vec![Range { min: 2.0, ..valid }],
            vec![Range {
                min: f64::NAN,
                ..valid
            }],
            vec![Range {
                max: f64::INFINITY,
                ..valid
            }],
            vec![Range {
                max: f64::NAN,
                ..valid
            }],
            vec![Range {
                increment: 0.0,
                ..valid
            }],
            vec![Range {
                increment: -0.1,
                ..valid
            }],
            vec![Range {
                increment: f64::INFINITY,
                ..valid
            }],
            vec![Range {
                increment: f64::NAN,
                ..valid
            }],
            vec![
                valid,
                Range {
                    min: 0.5,
                    max: 2.0,
                    increment: 0.5,
                },
            ],
            vec![
                valid,
                Range {
                    min: 2.0,
                    max: 3.0,
                    increment: 0.5,
                },
            ],
            vec![
                valid,
                Range {
                    min: 0.0,
                    max: 0.5,
                    increment: 0.1,
                },
            ],
            vec![Range {
                min: f64::MAX / 2.0,
                max: f64::MAX,
                increment: f64::MAX,
            }],
        ];
        for ranges in invalid_ranges {
            let granularity = PriceGranularity::Custom {
                precision: 3,
                ranges,
            };
            for mode in [
                BidRounding::Down,
                BidRounding::Up,
                BidRounding::Nearest,
                BidRounding::TimeSplit,
            ] {
                for cpm in [0.75_f64, 1.5_f64, f64::INFINITY] {
                    assert_eq!(
                        price_bucket(cpm, &granularity, mode, &rng()),
                        price_bucket(cpm, &PriceGranularity::Med, mode, &rng()),
                        "invalid directly constructed ranges must use Med: {granularity:?}, {mode:?}",
                    );
                }
            }
        }
    }

    #[test]
    fn direct_custom_precision_is_clamped_to_the_parser_limit() {
        let granularity = PriceGranularity::Custom {
            precision: u8::MAX,
            ranges: vec![Range {
                min: 0.0,
                max: 2.0,
                increment: 0.5,
            }],
        };
        assert_eq!(
            price_bucket(1.0, &granularity, BidRounding::Down, &rng()),
            "1.000000000000000",
        );
    }

    #[test]
    fn med_bucket_3_12_floors_to_3_10() {
        assert_eq!(
            price_bucket(3.12, &PriceGranularity::Med, BidRounding::Down, &rng()),
            "3.10",
        );
    }

    #[test]
    fn med_bucket_above_max_returns_max() {
        assert_eq!(
            price_bucket(25.0, &PriceGranularity::Med, BidRounding::Down, &rng()),
            "20.00",
        );
    }

    #[test]
    fn auto_picks_correct_range() {
        // 7.4 → in (5, 10) range with 0.1 → 7.40.
        assert_eq!(
            price_bucket(7.4, &PriceGranularity::Auto, BidRounding::Down, &rng()),
            "7.40",
        );
    }

    #[test]
    fn high_bucket_uses_001() {
        assert_eq!(
            price_bucket(0.123, &PriceGranularity::High, BidRounding::Down, &rng()),
            "0.12",
        );
    }

    #[test]
    fn negative_cpm_returns_empty() {
        assert_eq!(
            price_bucket(-1.0, &PriceGranularity::Med, BidRounding::Down, &rng()),
            "",
        );
    }

    #[test]
    fn timesplit_is_deterministic_under_seeded_rng() {
        let a = price_bucket(
            3.13,
            &PriceGranularity::Med,
            BidRounding::TimeSplit,
            &SeededRng::new(42),
        );
        let b = price_bucket(
            3.13,
            &PriceGranularity::Med,
            BidRounding::TimeSplit,
            &SeededRng::new(42),
        );
        assert_eq!(a, b);
    }
}

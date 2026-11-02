//! Targeting-key emission.

use crate::response::ErrorEntry;
use auctioneer_runtime::diagnostic::{bounded, prefix_within};
use std::collections::BTreeMap;

/// How much of a rejected targeting prefix the 10015 warning quotes.
///
/// The warning names the prefix so an operator can recognise the one
/// they configured; it does not need the whole of it. The bound
/// matters because `run_auction` appends this warning once per
/// returned bid, so whatever the message carries is multiplied by the
/// bid count.
const MAX_PREFIX_IN_DIAGNOSTIC_BYTES: usize = 128;

const MIN_KEY_LENGTH: usize = 12;
const MAX_KEY_LENGTH: usize = 20;

#[derive(Debug, Default)]
pub struct TargetingKeys {
    /// Un-suffixed winner keys (`hb_pb`, `hb_bidder`, …) emitted only for
    /// the overall imp winner.
    pub winner: BTreeMap<String, String>,
    /// Per-bidder keys (`hb_pb_<bidder>`, …).
    pub per_bidder: BTreeMap<String, String>,
    /// Warnings emitted during key generation. Currently used for the
    /// prefix-fallback case (`TooLongTargetingPrefix`, code 10015). The
    /// caller folds these into the per-bidder bundle warnings.
    pub warnings: Vec<ErrorEntry>,
}

#[derive(Clone, Debug)]
pub struct TargetingInput<'a> {
    pub prefix: &'a str,
    pub truncate_target_attribute: Option<usize>,
    pub bidder: &'a str,
    pub price_bucket: &'a str,
    pub size_wh: Option<(u32, u32)>,
    pub deal_id: Option<&'a str>,
    pub env: Option<&'a str>,
    pub include_winners: bool,
    pub include_bidder_keys: bool,
    pub include_format: bool,
    /// Per-bidder keys are emitted when `include_bidder_keys` OR
    /// (`always_include_deals` AND the bid carries a deal). This field
    /// carries the request-level `alwaysincludedeals` flag through to
    /// `build_keys`'s OR check.
    pub always_include_deals: bool,
    pub format: Option<&'a str>,
    pub is_overall_winner: bool,
}

#[must_use]
pub fn build_keys(input: &TargetingInput<'_>) -> TargetingKeys {
    let mut out = TargetingKeys::default();

    let (effective_prefix, max_len, fallback_warning) =
        resolve_prefix_and_max(input.prefix, input.truncate_target_attribute);
    if let Some(warning) = fallback_warning {
        out.warnings.push(warning);
    }

    let mut pairs: Vec<(&str, String)> = Vec::new();
    if !input.price_bucket.is_empty() {
        pairs.push(("_pb", input.price_bucket.to_owned()));
    }
    pairs.push(("_bidder", input.bidder.to_owned()));
    if let Some((w, h)) = input.size_wh
        && w > 0
        && h > 0
    {
        pairs.push(("_size", format!("{w}x{h}")));
    }
    if let Some(deal) = input.deal_id
        && !deal.is_empty()
    {
        pairs.push(("_deal", deal.into()));
    }
    if let Some(env) = input.env {
        pairs.push(("_env", env.into()));
    }
    if input.include_format
        && let Some(fmt) = input.format
    {
        pairs.push(("_format", fmt.into()));
    }

    let has_deal = input.deal_id.is_some_and(|d| !d.is_empty());
    let emit_per_bidder = input.include_bidder_keys || (input.always_include_deals && has_deal);

    for (suffix, value) in pairs {
        if input.include_winners && input.is_overall_winner {
            let key = bounded_key(&[&effective_prefix, suffix], max_len);
            out.winner.insert(key, value.clone());
        }
        if emit_per_bidder {
            let key = bounded_key(&[&effective_prefix, suffix, "_", input.bidder], max_len);
            out.per_bidder.insert(key, value);
        }
    }

    out
}

fn resolve_prefix_and_max(
    prefix: &str,
    truncate: Option<usize>,
) -> (String, usize, Option<ErrorEntry>) {
    let cap = truncate
        .filter(|n| (MIN_KEY_LENGTH..=MAX_KEY_LENGTH).contains(n))
        .unwrap_or(MAX_KEY_LENGTH);
    // Reserve MIN_KEY_LENGTH bytes after the prefix. If that would
    // exceed the cap, fall back to "hb" and emit code-10015
    // `TooLongTargetingPrefix`.
    let required_length = prefix.len().saturating_add(MIN_KEY_LENGTH);
    if required_length > cap {
        // The prefix is request-influenced (request-level targeting
        // overrides the account's), and `run.rs` appends this warning
        // once per returned bid — so an unbounded prefix is multiplied
        // by the bid count. A 64 KiB prefix across 32 bids rendered
        // 2,099,744 bytes of warning text.
        let clipped = bounded(prefix, MAX_PREFIX_IN_DIAGNOSTIC_BYTES);
        let shown = bounded(
            &clipped.escape_debug().to_string(),
            MAX_PREFIX_IN_DIAGNOSTIC_BYTES,
        );
        let warning = ErrorEntry {
            code: 10015,
            message: format!(
                "configured targeting prefix \"{shown}\" exceeds the {cap}-byte cap; \
                 falling back to \"hb\"",
            ),
        };
        return ("hb".into(), cap, Some(warning));
    }
    (prefix.into(), cap, None)
}

fn bounded_key(parts: &[&str], max_len: usize) -> String {
    let mut value = String::with_capacity(max_len);
    for part in parts {
        let kept = prefix_within(part, max_len.saturating_sub(value.len()));
        value.push_str(kept);
        if kept.len() < part.len() {
            break;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_PREFIX_IN_DIAGNOSTIC_BYTES, TargetingInput, build_keys, resolve_prefix_and_max,
    };

    fn input_for<'a>(prefix: &'a str, bidder: &'a str) -> TargetingInput<'a> {
        TargetingInput {
            prefix,
            truncate_target_attribute: None,
            bidder,
            price_bucket: "1.20",
            size_wh: Some((300, 250)),
            deal_id: None,
            env: None,
            include_winners: true,
            include_bidder_keys: true,
            include_format: false,
            always_include_deals: false,
            format: None,
            is_overall_winner: true,
        }
    }

    #[test]
    fn emits_winner_and_per_bidder_pb() {
        let out = build_keys(&input_for("hb", "fake_aaa"));
        assert_eq!(out.winner.get("hb_pb").map(String::as_str), Some("1.20"));
        assert_eq!(
            out.per_bidder.get("hb_pb_fake_aaa").map(String::as_str),
            Some("1.20"),
        );
    }

    #[test]
    fn empty_price_bucket_omits_only_price_keys() {
        for (prefix, effective_prefix) in [("hb", "hb"), ("req", "req"), ("too_long_prefix", "hb")]
        {
            for (include_winners, is_overall_winner, include_bidder_keys, always_include_deals) in [
                (true, true, true, false),
                (true, true, false, true),
                (false, true, true, false),
                (true, false, true, false),
                (true, true, false, false),
                (false, false, false, true),
                (false, false, false, false),
            ] {
                let mut input = input_for(prefix, "fake_ax");
                input.deal_id = Some("D42");
                input.env = Some("mobile-app");
                input.include_format = true;
                input.format = Some("banner");
                input.include_winners = include_winners;
                input.is_overall_winner = is_overall_winner;
                input.include_bidder_keys = include_bidder_keys;
                input.always_include_deals = always_include_deals;
                let mut expected = build_keys(&input);
                expected.winner.remove(&format!("{effective_prefix}_pb"));
                expected
                    .per_bidder
                    .remove(&format!("{effective_prefix}_pb_fake_ax"));

                input.price_bucket = "";
                let out = build_keys(&input);
                assert_eq!(out.winner, expected.winner, "input: {input:?}");
                assert_eq!(out.per_bidder, expected.per_bidder, "input: {input:?}");
                assert_eq!(
                    out.warnings
                        .iter()
                        .map(|w| (w.code, &w.message))
                        .collect::<Vec<_>>(),
                    expected
                        .warnings
                        .iter()
                        .map(|w| (w.code, &w.message))
                        .collect::<Vec<_>>(),
                    "input: {input:?}",
                );
            }
        }
    }

    #[test]
    fn zero_price_bucket_is_not_empty_targeting() {
        let mut input = input_for("hb", "fake_ax");
        input.price_bucket = "0.00";
        let out = build_keys(&input);
        assert_eq!(out.winner.get("hb_pb").map(String::as_str), Some("0.00"));
        assert_eq!(
            out.per_bidder.get("hb_pb_fake_ax").map(String::as_str),
            Some("0.00")
        );
    }

    #[test]
    fn truncated_keys_do_not_retain_the_untruncated_allocation() {
        let bidder = "a".repeat(64 * 1024);
        let out = build_keys(&input_for("hb", &bidder));
        for key in out.per_bidder.keys() {
            assert!(key.len() <= 20);
            assert!(
                key.capacity() <= 20,
                "key capacity must follow its byte cap"
            );
        }
    }

    #[test]
    fn a_configured_key_length_cannot_widen_the_hard_cap() {
        let prefix = "p".repeat(64 * 1024);
        let mut input = input_for(&prefix, "fake_bidder");
        input.truncate_target_attribute = Some(usize::MAX);
        let out = build_keys(&input);
        assert!(out.per_bidder.keys().all(|key| key.len() <= 20));
    }

    #[test]
    fn emits_size_only_when_both_dims_positive() {
        let mut input = input_for("hb", "fake_ax");
        input.size_wh = Some((0, 250));
        let out = build_keys(&input);
        assert!(!out.winner.contains_key("hb_size"));
    }

    #[test]
    fn prefix_too_long_falls_back_to_hb() {
        let mut input = input_for("verylongprefix_abc", "fake_ax");
        input.truncate_target_attribute = Some(16);
        let out = build_keys(&input);
        // hb_pb_fake_ax fits under the cap after the fallback.
        assert!(out.per_bidder.contains_key("hb_pb_fake_ax"));
        // A 10015 warning was emitted.
        assert!(out.warnings.iter().any(|w| w.code == 10015));
    }

    #[test]
    fn eight_byte_prefix_is_accepted_at_twenty_byte_cap() {
        for truncate in [None, Some(20)] {
            let (prefix, cap, warning) = resolve_prefix_and_max("abcdefgh", truncate);
            assert_eq!(prefix, "abcdefgh");
            assert_eq!(cap, 20);
            assert!(warning.is_none());
        }
    }

    #[test]
    fn nine_byte_prefix_falls_back_at_twenty_byte_cap() {
        for truncate in [None, Some(20)] {
            let (prefix, cap, warning) = resolve_prefix_and_max("abcdefghi", truncate);
            assert_eq!(prefix, "hb");
            assert_eq!(cap, 20);
            assert_eq!(warning.map(|entry| entry.code), Some(10015));
        }
    }

    #[test]
    fn four_byte_prefix_is_accepted_at_sixteen_byte_cap() {
        let (prefix, cap, warning) = resolve_prefix_and_max("abcd", Some(16));
        assert_eq!(prefix, "abcd");
        assert_eq!(cap, 16);
        assert!(warning.is_none());
    }

    #[test]
    fn five_byte_prefix_falls_back_at_sixteen_byte_cap() {
        let (prefix, cap, warning) = resolve_prefix_and_max("abcde", Some(16));
        assert_eq!(prefix, "hb");
        assert_eq!(cap, 16);
        assert_eq!(warning.map(|entry| entry.code), Some(10015));
    }

    #[test]
    fn multibyte_prefix_uses_byte_boundary_at_twenty_byte_cap() {
        for (configured, expected, warning_code) in [
            ("\u{1f600}\u{1f600}", "\u{1f600}\u{1f600}", None),
            ("\u{e9}\u{e9}\u{e9}\u{20ac}", "hb", Some(10015)),
        ] {
            let (prefix, cap, warning) = resolve_prefix_and_max(configured, None);
            assert_eq!(prefix, expected, "{}-byte prefix", configured.len());
            assert_eq!(cap, 20);
            assert_eq!(warning.map(|entry| entry.code), warning_code);
        }
    }

    #[test]
    fn multibyte_prefix_uses_byte_boundary_at_sixteen_byte_cap() {
        for (configured, expected, warning_code) in [
            ("\u{1f600}", "\u{1f600}", None),
            ("\u{e9}\u{20ac}", "hb", Some(10015)),
        ] {
            let (prefix, cap, warning) = resolve_prefix_and_max(configured, Some(16));
            assert_eq!(prefix, expected, "{}-byte prefix", configured.len());
            assert_eq!(cap, 16);
            assert_eq!(warning.map(|entry| entry.code), warning_code);
        }
    }

    /// The 10015 warning does not carry the whole prefix, because
    /// `run_auction` appends it once per returned bid.
    ///
    /// Asserted on TOTAL bytes across a realistic bid count rather
    /// than on one message: a per-message bound is what was missing,
    /// but the cost that matters is the product, and a guard on a
    /// single call would pass against a message that is still
    /// multiplied 32 times. The unbounded form rendered 2,099,744
    /// bytes here.
    #[test]
    fn the_prefix_warning_is_bounded_against_the_bid_count() {
        const BIDS: usize = 32;
        let prefix = "p".repeat(64 * 1024);
        let mut input = input_for(&prefix, "fake_ax");
        input.truncate_target_attribute = Some(16);

        let mut total = 0_usize;
        for _ in 0..BIDS {
            let out = build_keys(&input);
            for warning in &out.warnings {
                total += warning.message.len();
            }
        }

        // The warning is still emitted and still identifies itself.
        let one = build_keys(&input);
        assert!(
            one.warnings.iter().any(|w| w.code == 10015),
            "bounding the text must not drop the warning",
        );
        assert!(
            one.warnings
                .iter()
                .any(|w| w.message.contains("falling back")),
            "the warning must still say what it did",
        );

        // Room for the fixed prose plus the quoted prefix, per bid.
        let ceiling = BIDS * (MAX_PREFIX_IN_DIAGNOSTIC_BYTES + 256);
        assert!(
            total <= ceiling,
            "{BIDS} bids rendered {total} warning bytes from a {} byte prefix; \
             the bound allows {ceiling}",
            prefix.len(),
        );
    }

    #[test]
    fn escaped_control_prefix_warning_keeps_its_total_byte_bound() {
        for control in ['\u{7f}', '\0', '\u{1b}', '"', '\\'] {
            let prefix = control.to_string().repeat(64 * 1024);
            let keys = build_keys(&input_for(&prefix, "fake_ax"));
            let warning = &keys.warnings[0];
            assert_eq!(warning.code, 10015);
            assert!(warning.message.len() <= MAX_PREFIX_IN_DIAGNOSTIC_BYTES + 256);
            assert!(warning.message.contains("falling back"));
            assert!(keys.winner.contains_key("hb_pb"));
        }
    }

    #[test]
    fn multibyte_bidder_name_does_not_panic_at_truncate_boundary() {
        // 20-byte cap; after the 6-byte "hb_pb_" prefix, 14 bytes of
        // name remain. Each `\u{e9}` ("é") encodes to 2 bytes in
        // UTF-8, so eight of them make a 16-byte name (a 22-byte key
        // total). Truncation at 20 bytes lands mid-"é", which would
        // panic without the char-boundary walk in `truncate`.
        let bidder = "\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}\u{e9}";
        let mut input = input_for("hb", bidder);
        input.truncate_target_attribute = Some(20);
        let out = build_keys(&input);
        // No panic; the emitted key stops at the previous char
        // boundary — the string may be shorter than 20 bytes.
        let key = out
            .per_bidder
            .keys()
            .find(|k| k.starts_with("hb_pb_"))
            .expect("per-bidder key must be present");
        assert!(key.is_char_boundary(key.len()));
        assert!(key.len() <= 20);
    }

    #[test]
    fn always_include_deals_emits_per_bidder_even_when_bidder_keys_disabled() {
        let mut input = input_for("hb", "fake_ax");
        input.include_bidder_keys = false;
        input.always_include_deals = true;
        input.deal_id = Some("D42");
        let out = build_keys(&input);
        assert_eq!(
            out.per_bidder.get("hb_pb_fake_ax").map(String::as_str),
            Some("1.20"),
        );
    }
}

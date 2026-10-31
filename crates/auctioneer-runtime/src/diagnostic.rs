//! Bounds for diagnostic text whose inputs the caller influences.
//!
//! A diagnostic built with `format!` from request-shaped or
//! bidder-shaped text inherits that text's size. Two allocation paths
//! in this workspace were sized as if it did not:
//!
//! - a configuration warning embedding the whole targeting prefix,
//!   appended once per returned bid, so the prefix is multiplied by the
//!   bid count;
//! - a parse failure retaining serde's own message, which quotes the
//!   offending input, so a caller-sized malformed body inflates into a
//!   larger `String` than the body itself.
//!
//! Neither allocation is bounded by the cap on the text it derives
//! from: the response cap bounds the body, not a message built from it.
//! So the bound belongs at construction, which is what this module is.
//!
//! It lives in `auctioneer-runtime` because the two call sites are in
//! `auctioneer-exchange` and `auctioneer-bidder`, which both depend on it and
//! neither of which depends on the other.

/// The longest prefix of `text` that fits `limit` bytes.
///
/// `str::floor_char_boundary` is still unstable, and slicing a
/// multi-byte sequence in half panics. Caller-influenced text may hold
/// any UTF-8, so the cut is found by walking character boundaries
/// rather than assumed — and returned as a `&str` so no caller has to
/// index back into the string with the byte offset.
#[must_use]
pub fn prefix_within(text: &str, limit: usize) -> &str {
    if limit >= text.len() {
        return text;
    }
    // `is_char_boundary(0)` always holds, so the search never fails;
    // `unwrap_or(0)` is for the type, not a reachable case.
    let end = (0..=limit)
        .rev()
        .find(|offset| text.is_char_boundary(*offset))
        .unwrap_or(0);
    text.get(..end).unwrap_or("")
}

/// Marks a diagnostic the bound cut short.
pub const ELLIPSIS: &str = "...";

/// `text`, bounded to `limit` bytes with the cut marked.
///
/// The returned `String` is never longer than `limit`: the ellipsis is
/// reserved out of the budget rather than appended past it. An earlier
/// helper elsewhere in this workspace appended the marker after
/// filling the budget, so a bounded message came out at
/// `limit + ELLIPSIS.len()` — the bound has to cover its own marker.
///
/// A `limit` below the ellipsis itself yields the bare truncated text
/// with no marker, because there is no room to say anything about it.
#[must_use]
pub fn bounded(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let Some(room) = limit.checked_sub(ELLIPSIS.len()) else {
        return prefix_within(text, limit).to_owned();
    };
    let kept = prefix_within(text, room);
    let mut out = String::with_capacity(limit);
    out.push_str(kept);
    out.push_str(ELLIPSIS);
    out
}

#[cfg(test)]
mod tests {
    use super::{ELLIPSIS, bounded, prefix_within};

    #[test]
    fn text_within_the_limit_is_returned_whole() {
        assert_eq!(bounded("short", 64), "short");
        assert_eq!(prefix_within("short", 64), "short");
    }

    /// The bound covers the ellipsis, so the result fits the limit.
    ///
    /// Stated as `<=` on the OUTPUT length rather than on the kept
    /// prefix: appending the marker after filling the budget is the
    /// defect this asserts against, and that form passes any check
    /// made on the prefix alone.
    #[test]
    fn the_marker_is_reserved_out_of_the_budget_not_appended_past_it() {
        for limit in [4, 8, 16, 64, 100] {
            let out = bounded(&"x".repeat(10_000), limit);
            assert!(
                out.len() <= limit,
                "a {limit}-byte bound produced {} bytes",
                out.len(),
            );
            assert!(out.ends_with(ELLIPSIS), "a cut message must say so: {out}");
        }
    }

    /// A cut never lands mid-sequence, for any limit across a
    /// multi-byte run.
    ///
    /// Every offset is exercised rather than one hand-picked value:
    /// the panic only happens at the offsets interior to a sequence,
    /// so a single limit can miss it entirely.
    #[test]
    fn no_limit_splits_a_multibyte_sequence() {
        // 4-byte sequences, so three interior offsets per character.
        let text = "\u{1f600}".repeat(32);
        for limit in 0..=text.len() {
            let out = bounded(&text, limit);
            assert!(
                out.len() <= limit.max(ELLIPSIS.len()),
                "limit {limit} produced {} bytes",
                out.len(),
            );
            // The real claim: it did not panic, and what came back is
            // valid UTF-8 by construction of the return type.
            assert!(out.is_char_boundary(out.len()));
        }
    }

    /// A limit with no room for the marker still bounds the text.
    #[test]
    fn a_limit_below_the_marker_still_bounds() {
        let out = bounded(&"y".repeat(100), 2);
        assert_eq!(out.len(), 2, "the text is still cut to the limit: {out}");
        assert!(!out.ends_with(ELLIPSIS));
    }

    #[test]
    fn a_zero_limit_yields_nothing() {
        assert_eq!(bounded("anything", 0), "");
        assert_eq!(prefix_within("anything", 0), "");
    }
}

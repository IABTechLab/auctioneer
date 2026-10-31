//! A parse failure's diagnostic is bounded at its construction.
//!
//! Serde quotes the offending input into its own message, so a
//! malformed bidder body does not merely survive the response cap — it
//! inflates past it. A 240,100-byte malformed body rendered a
//! 1,440,083-byte message, which `auctioneer-exchange`'s assembly then
//! clones per bidder. The per-call assembly allowance is 786,432
//! bytes, so one such diagnostic exceeds it on its own.
//!
//! The response cap bounds the BODY. It cannot bound a string built
//! from the body, which is why the bound belongs at the error's
//! construction and this file asserts it there.

#[cfg(test)]
mod tests {
    use auctioneer_bidder::{BidderError, BidderHttpResponse};

    /// The allowance a malformed-response diagnostic must stay inside:
    /// `max_decoded_bytes` x `ASSEMBLY_FACTOR` on the shipped row.
    const ASSEMBLY_ALLOWANCE_BYTES: usize = 786_432;

    /// How much filler the fixture carries.
    ///
    /// Sized so the UNBOUNDED diagnostic would exceed
    /// `ASSEMBLY_ALLOWANCE_BYTES`. A smaller body produces a message
    /// that is still oversized relative to the bound but fits the
    /// allowance, so the row would pass with the bound removed —
    /// measured: 240,000 bytes of filler yields a 240,061-byte
    /// message, well under the 786,432-byte allowance.
    const FILLER_BYTES: usize = 900_000;

    /// A malformed body whose serde diagnostic QUOTES the input.
    ///
    /// Which shape is used is load-bearing, and most are useless
    /// here: measured against this response type, an unterminated
    /// string yields a 50-byte message, trailing garbage 39, and a
    /// huge unknown key 42 — all independent of the body size, so the
    /// row would pass with the bound removed and prove nothing. A
    /// string where a number belongs is the shape that inflates:
    /// serde quotes the value it could not coerce, rendering 200,061
    /// bytes from a 200,036-byte body.
    fn inflating_malformed_body() -> Vec<u8> {
        let filler = "a".repeat(FILLER_BYTES);
        format!("{{\"seatbid\":[{{\"bid\":[{{\"price\":\"{filler}\"}}]}}]}}").into_bytes()
    }

    /// The diagnostic a malformed response produces is bounded.
    ///
    /// Asserted against the assembly allowance, not against an
    /// arbitrary number: that is the bound the unbounded form broke,
    /// and tying the row to it means a later change to the allowance
    /// re-examines this claim instead of silently passing.
    #[test]
    fn a_malformed_response_diagnostic_fits_the_assembly_allowance() {
        let body = inflating_malformed_body();
        let response =
            BidderHttpResponse::new(200, Vec::new(), body.clone()).within_node_budget(8_192);

        let errors = response
            .parse_bid_response()
            .expect_err("the fixture is malformed, so the parse must fail");

        let total: usize = errors.iter().map(|err| err.message.len()).sum();
        assert!(
            total < ASSEMBLY_ALLOWANCE_BYTES,
            "a {body} byte malformed body produced {total} bytes of diagnostic, \
             which the {ASSEMBLY_ALLOWANCE_BYTES} byte per-call assembly \
             allowance cannot hold",
            body = body.len(),
        );
        // The bound must not have cost the diagnosis: a code-14 entry
        // still identifies itself. A guard that only checked the
        // length would pass against a constructor that returned an
        // empty message.
        assert!(
            errors.iter().any(|err| err.code == 14),
            "the parse failure must still be reported as code 14",
        );
        assert!(
            errors.iter().all(|err| !err.message.is_empty()),
            "bounding a message must not empty it",
        );
    }

    /// Every constructor bounds its message, not just the parse path.
    ///
    /// The parse path is where the defect was observed, but all five
    /// constructors took an unbounded `Into<String>`, and a bidder
    /// may pass caller-shaped text to any of them. Stated per
    /// constructor so adding a sixth that bypasses the bound fails
    /// here rather than in production.
    #[test]
    fn every_constructor_bounds_its_message() {
        let huge = "z".repeat(2_000_000);
        for (label, err) in [
            ("bad_input", BidderError::bad_input(huge.clone())),
            (
                "bad_server_response",
                BidderError::bad_server_response(huge.clone()),
            ),
            (
                "failed_to_request_bids",
                BidderError::failed_to_request_bids(huge.clone()),
            ),
            (
                "failed_to_unmarshal",
                BidderError::failed_to_unmarshal(huge.clone()),
            ),
            ("unknown", BidderError::unknown(huge.clone())),
        ] {
            assert!(
                err.message.len() < ASSEMBLY_ALLOWANCE_BYTES,
                "{label} retained {} bytes of a {} byte input",
                err.message.len(),
                huge.len(),
            );
        }
    }

    /// A message already inside the bound is untouched.
    ///
    /// The shipped constructors are called overwhelmingly with short
    /// literals; a bound that rewrote those would make every existing
    /// error message harder to read for no gain.
    #[test]
    fn a_short_message_is_carried_verbatim() {
        let err = BidderError::bad_input("synthetic: imp.ext.bidder missing");
        assert_eq!(err.message, "synthetic: imp.ext.bidder missing");
    }
}

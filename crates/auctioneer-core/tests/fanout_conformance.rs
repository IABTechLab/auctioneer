//! `BidderFanout` conformance suite, auctioneer-core binary.
//!
//! Every case body lives in `auctioneer_core::conformance` so this binary,
//! Cloudflare, Spin and Fastly run the SAME assertions. What
//! differs between them is only which crate compiles them.
//!
//! Bucket A rows run against `DevFanout` and against
//! `EdgeZeroFanout` on both of its drives — the per-call strategy
//! bridge and the wave scheduler — so a failure report names which
//! side broke. Bucket B rows need a real `Timer` to synthesize a
//! `TimeoutCause` at all, so they skip `DevFanout` and run against
//! both `EdgeZeroFanout` drives. The wave arm is the one a served
//! request reaches.

#[cfg(test)]
mod transport_contract {
    use auctioneer_core::conformance as rows;

    #[test]
    fn transport_error_yields_initial_code_4_statuscode_100_on_bounded_wave() {
        rows::transport_error_yields_initial_code_4_statuscode_100_on_bounded_wave();
    }
}

#[cfg(test)]
mod entry_deadline {
    use auctioneer_core::conformance::{self as rows, FanoutUnderTest};

    #[test]
    fn pre_expired_deadline_at_send_all_entry_short_circuits_on_bounded_wave() {
        rows::pre_expired_deadline_at_send_all_entry_short_circuits(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn deadline_equal_to_send_all_entry_short_circuits_on_bounded_wave() {
        rows::deadline_equal_to_send_all_entry_short_circuits(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn pre_expired_deadline_at_send_all_entry_short_circuits_on_per_call_bridge() {
        rows::pre_expired_deadline_at_send_all_entry_short_circuits(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn deadline_equal_to_send_all_entry_short_circuits_on_per_call_bridge() {
        rows::deadline_equal_to_send_all_entry_short_circuits(FanoutUnderTest::PerCallBridge);
    }
}

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use auctioneer_core::conformance::{
        FanoutUnderTest, deadline_expires_mid_fanout_yields_auction_deadline_cause,
        equal_call_timeout_and_deadline_ties_to_bidder_cause,
        no_outbound_capability_maps_to_fanout_unavailable_and_500,
        oversize_response_yields_oversize_error,
        per_bidder_isolation_fast_bidder_elapsed_matches_own_delay,
        per_call_timeout_expires_yields_bidder_cause,
        redirect_is_bad_server_response_and_never_parsed,
        results_preserve_input_order_when_calls_complete_out_of_order,
        retry_on_transient_error_never_happens_m3_is_no_retry,
        single_call_success_returns_ok_and_elapsed_within_bounds,
        transport_error_preserves_category_and_wire_codes, zero_calls_returns_empty_result_set,
    };

    // ---------------------------------------------------------------------
    // Bucket A — interim-bridge side
    // ---------------------------------------------------------------------

    #[test]
    fn oversize_response_yields_oversize_error_on_interim_bridge() {
        oversize_response_yields_oversize_error(FanoutUnderTest::InterimBridge);
    }

    #[test]
    fn transport_error_preserves_category_and_wire_codes_on_dev_fanout() {
        transport_error_preserves_category_and_wire_codes(FanoutUnderTest::InterimBridge);
    }

    #[test]
    fn zero_calls_returns_empty_result_set_on_interim_bridge() {
        zero_calls_returns_empty_result_set(FanoutUnderTest::InterimBridge);
    }

    #[test]
    fn single_call_success_returns_ok_and_elapsed_within_bounds_on_interim_bridge() {
        single_call_success_returns_ok_and_elapsed_within_bounds(FanoutUnderTest::InterimBridge);
    }

    #[test]
    fn retry_on_transient_error_never_happens_m3_is_no_retry_on_interim_bridge() {
        retry_on_transient_error_never_happens_m3_is_no_retry(FanoutUnderTest::InterimBridge);
    }

    #[test]
    fn results_preserve_input_order_when_calls_complete_out_of_order_on_interim_bridge() {
        results_preserve_input_order_when_calls_complete_out_of_order(
            FanoutUnderTest::InterimBridge,
        );
    }

    #[test]
    fn per_bidder_isolation_fast_bidder_elapsed_matches_own_delay_on_interim_bridge() {
        per_bidder_isolation_fast_bidder_elapsed_matches_own_delay(FanoutUnderTest::InterimBridge);
    }

    #[test]
    fn redirect_is_bad_server_response_and_never_parsed_on_interim_bridge() {
        redirect_is_bad_server_response_and_never_parsed(FanoutUnderTest::InterimBridge);
    }

    // ---------------------------------------------------------------------
    // Bucket A — per-call-bridge side
    // ---------------------------------------------------------------------

    #[test]
    fn oversize_response_yields_oversize_error_on_per_call_bridge() {
        oversize_response_yields_oversize_error(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn transport_error_preserves_category_and_wire_codes_on_per_call_bridge() {
        transport_error_preserves_category_and_wire_codes(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn zero_calls_returns_empty_result_set_on_per_call_bridge() {
        zero_calls_returns_empty_result_set(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn single_call_success_returns_ok_and_elapsed_within_bounds_on_per_call_bridge() {
        single_call_success_returns_ok_and_elapsed_within_bounds(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn retry_on_transient_error_never_happens_m3_is_no_retry_on_per_call_bridge() {
        retry_on_transient_error_never_happens_m3_is_no_retry(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn results_preserve_input_order_when_calls_complete_out_of_order_on_per_call_bridge() {
        results_preserve_input_order_when_calls_complete_out_of_order(
            FanoutUnderTest::PerCallBridge,
        );
    }

    #[test]
    fn per_bidder_isolation_fast_bidder_elapsed_matches_own_delay_on_per_call_bridge() {
        per_bidder_isolation_fast_bidder_elapsed_matches_own_delay(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn redirect_is_bad_server_response_and_never_parsed_on_per_call_bridge() {
        redirect_is_bad_server_response_and_never_parsed(FanoutUnderTest::PerCallBridge);
    }

    // ---------------------------------------------------------------------
    // Bucket A — bounded-wave side
    //
    // `InterimBridge` and `PerCallBridge` both classify through the per-call bridge.
    // The wave scheduler owns a SECOND classifier, and it is the one a
    // served request reaches once boot installs the limits and the
    // stream gate. A status rule has to hold on both.
    //
    // The oversize row asserts CallResponseCap. A real bounded host can
    // enforce the same cap earlier with DecodedBody origin; both render 4/100.
    //
    // * `oversize_response_yields_oversize_error` asserts the
    //   `CallResponseCap` origin. On the wave arm the per-call cap
    //   travels to the host as `max_decoded_response_bytes` and the
    //   host enforces it, so the origin is `DecodedBody`. Same wire
    //   contract (code 4 / statuscode 100), different diagnostic.
    // ---------------------------------------------------------------------

    #[test]
    fn transport_error_preserves_category_and_wire_codes_on_bounded_wave() {
        transport_error_preserves_category_and_wire_codes(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn zero_calls_returns_empty_result_set_on_bounded_wave() {
        zero_calls_returns_empty_result_set(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn single_call_success_returns_ok_and_elapsed_within_bounds_on_bounded_wave() {
        single_call_success_returns_ok_and_elapsed_within_bounds(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn retry_on_transient_error_never_happens_m3_is_no_retry_on_bounded_wave() {
        retry_on_transient_error_never_happens_m3_is_no_retry(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn results_preserve_input_order_when_calls_complete_out_of_order_on_bounded_wave() {
        results_preserve_input_order_when_calls_complete_out_of_order(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn per_bidder_isolation_fast_bidder_elapsed_matches_own_delay_on_bounded_wave() {
        per_bidder_isolation_fast_bidder_elapsed_matches_own_delay(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn redirect_is_bad_server_response_and_never_parsed_on_bounded_wave() {
        redirect_is_bad_server_response_and_never_parsed(FanoutUnderTest::BoundedWave);
    }

    // ---------------------------------------------------------------------
    // Bucket B — per-call-bridge side
    // ---------------------------------------------------------------------

    #[test]
    fn deadline_expires_mid_fanout_yields_auction_deadline_cause_on_per_call_bridge() {
        deadline_expires_mid_fanout_yields_auction_deadline_cause(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn per_call_timeout_expires_yields_bidder_cause_on_per_call_bridge() {
        per_call_timeout_expires_yields_bidder_cause(FanoutUnderTest::PerCallBridge);
    }

    #[test]
    fn equal_call_timeout_and_deadline_ties_to_bidder_cause_on_per_call_bridge() {
        equal_call_timeout_and_deadline_ties_to_bidder_cause(FanoutUnderTest::PerCallBridge);
    }

    // -----------------------------------------------------------------
    // Bucket B — bounded-wave side
    //
    // The drive a served auction takes. A timeout attribution proven
    // only on the strategy bridge says nothing about this path.
    // -----------------------------------------------------------------

    #[test]
    fn deadline_expires_mid_fanout_yields_auction_deadline_cause_on_bounded_wave() {
        deadline_expires_mid_fanout_yields_auction_deadline_cause(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn per_call_timeout_expires_yields_bidder_cause_on_bounded_wave() {
        per_call_timeout_expires_yields_bidder_cause(FanoutUnderTest::BoundedWave);
    }

    #[test]
    fn equal_call_timeout_and_deadline_ties_to_bidder_cause_on_bounded_wave() {
        equal_call_timeout_and_deadline_ties_to_bidder_cause(FanoutUnderTest::BoundedWave);
    }

    // ---------------------------------------------------------------------
    // Batch-level failure mapping — no fanout implementation involved
    // ---------------------------------------------------------------------

    #[test]
    fn no_outbound_capability_maps_to_fanout_unavailable_and_500_end_to_end() {
        config_funding::install();
        no_outbound_capability_maps_to_fanout_unavailable_and_500();
    }
}

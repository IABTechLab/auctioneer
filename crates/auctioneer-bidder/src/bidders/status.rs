//! The opt-in 204/no-bid, 200/decode status contract. Never reads a response body.

use crate::{BidderError, BidderResponse};

/// `None` continues to the bidder's sealed parser; `Some` is the final no-bid result.
/// The caller retains its exact error wording and classification, including 400.
pub(super) fn standard_status(
    status: u16,
    unexpected: impl FnOnce(u16) -> BidderError,
) -> Result<Option<BidderResponse>, Vec<BidderError>> {
    match status {
        200 => Ok(None),
        204 => Ok(Some(BidderResponse {
            bids: Vec::new(),
            currency: "USD".into(),
            errors: Vec::new(),
            auction_configs: Vec::new(),
        })),
        _ => Err(vec![unexpected(status)]),
    }
}

/// The common error classification, independent of protocol-specific wording.
pub(super) fn bad_input_or_server(status: u16, message: impl Into<String>) -> BidderError {
    if status == 400 {
        BidderError::bad_input(message)
    } else {
        BidderError::bad_server_response(message)
    }
}

/// Exact debug wording used by protocols without a terminal period.
pub(super) fn debug_status_error(status: u16) -> BidderError {
    bad_input_or_server(
        status,
        format!("Unexpected status code: {status}. Run with request.debug = 1 for more info"),
    )
}

#[cfg(test)]
mod tests {
    use super::{bad_input_or_server, debug_status_error, standard_status};
    use crate::{BidderError, BidderErrorKind};

    #[test]
    fn success_continues_without_constructing_an_error_or_no_bid() {
        assert!(
            standard_status(200, |_| panic!("200 is not an error"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn no_content_returns_only_an_empty_usd_response() {
        let response = standard_status(204, |_| panic!("204 is not an error"))
            .unwrap()
            .unwrap();
        assert!(response.bids.is_empty());
        assert_eq!(response.currency, "USD");
        assert!(response.errors.is_empty());
        assert!(response.auction_configs.is_empty());
    }

    #[test]
    fn bad_request_preserves_the_callers_bad_input_contract() {
        let errors = standard_status(400, |status| {
            assert_eq!(status, 400);
            BidderError::bad_input("fixed publisher error.")
        })
        .unwrap_err();
        assert_eq!(errors, [BidderError::bad_input("fixed publisher error.")]);
        assert_eq!(errors[0].kind, BidderErrorKind::Other);
    }

    #[test]
    fn bad_request_can_remain_a_bad_server_response() {
        let errors = standard_status(400, |status| {
            BidderError::bad_server_response(format!("status={status}"))
        })
        .unwrap_err();
        assert_eq!(errors, [BidderError::bad_server_response("status=400")]);
    }

    #[test]
    fn every_other_status_uses_exactly_one_caller_supplied_error() {
        for status in [
            0_u16,
            100,
            199,
            201,
            202,
            205,
            206,
            301,
            302,
            401,
            429,
            500,
            u16::MAX,
        ] {
            let mut calls = 0_usize;
            let errors = standard_status(status, |actual| {
                calls += 1;
                assert_eq!(actual, status);
                BidderError::bad_server_response(format!("Unexpected status code: {actual}."))
            })
            .unwrap_err();
            assert_eq!(calls, 1);
            assert_eq!(
                errors,
                [BidderError::bad_server_response(format!(
                    "Unexpected status code: {status}."
                ))]
            );
        }
    }

    #[test]
    fn non_200_errors_are_not_reclassified() {
        let errors =
            standard_status(500, |_| BidderError::unknown("fixed protocol error")).unwrap_err();
        assert_eq!(errors, [BidderError::unknown("fixed protocol error")]);
    }

    #[test]
    fn bad_input_or_server_preserves_the_message_and_400_classification() {
        assert_eq!(
            bad_input_or_server(400, "fixed status message."),
            BidderError::bad_input("fixed status message.")
        );
        for status in [0_u16, 201, 301, 401, 500] {
            assert_eq!(
                bad_input_or_server(status, "fixed status message."),
                BidderError::bad_server_response("fixed status message.")
            );
        }
    }

    #[test]
    fn debug_error_pins_case_punctuation_and_both_error_codes() {
        assert_eq!(
            debug_status_error(400),
            BidderError::bad_input(
                "Unexpected status code: 400. Run with request.debug = 1 for more info"
            )
        );
        assert_eq!(
            debug_status_error(500),
            BidderError::bad_server_response(
                "Unexpected status code: 500. Run with request.debug = 1 for more info"
            )
        );
    }
}

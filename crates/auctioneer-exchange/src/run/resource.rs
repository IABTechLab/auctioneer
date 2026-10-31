//! Preflight request-controlled multiplication in response output.

use crate::BidderCallResult;
use crate::response::BidderResultBundle;
use auctioneer_bidder::BidderName;
use auctioneer_runtime::budget::{
    ASSEMBLY_FACTOR, HEADER_MAP_CHARGE, RESPONSE_ASSEMBLY_FACTOR, RESPONSE_PARSE_FACTOR,
};
use auctioneer_runtime::debug_budget::debug_peak_bytes;
use auctioneer_runtime::{ResourceError, serialized_json_bytes};
use std::collections::BTreeMap;

fn add(left: u64, right: u64) -> Result<u64, ResourceError> {
    left.checked_add(right)
        .ok_or(ResourceError::CapacityOverflow)
}

fn mul(left: u64, right: u64) -> Result<u64, ResourceError> {
    left.checked_mul(right)
        .ok_or(ResourceError::CapacityOverflow)
}

/// Keep ordinary response peaks out of the prepaid debug allowance. This is a
/// coexistence check against the held receipt, not a second ledger reservation.
pub(super) fn with_debug(ordinary: u64, max_calls: u32) -> Result<u64, ResourceError> {
    let debug = debug_peak_bytes(max_calls).map_err(|_overflow| ResourceError::CapacityOverflow)?;
    add(ordinary, debug)
}

fn bytes<T: serde::Serialize + ?Sized>(value: &T) -> Result<u64, ResourceError> {
    serialized_json_bytes(value).map_err(|_serialize| ResourceError::CapacityOverflow)
}

/// Retained transport bodies, parse/assembly peaks and bounded response headers.
pub(super) fn decoded(results: &[BidderCallResult]) -> Result<u64, ResourceError> {
    let mut body_bytes = 0_u64;
    for result in results {
        if let Ok(response) = &result.result {
            body_bytes = add(
                body_bytes,
                u64::try_from(response.body_len())
                    .map_err(|_size| ResourceError::CapacityOverflow)?,
            )?;
        }
    }
    let copies = add(
        add(RESPONSE_PARSE_FACTOR, RESPONSE_ASSEMBLY_FACTOR)?,
        add(1, ASSEMBLY_FACTOR)?,
    )?;
    let count = u64::try_from(results.len()).map_err(|_size| ResourceError::CapacityOverflow)?;
    add(mul(body_bytes, copies)?, mul(count, HEADER_MAP_CHARGE)?)
}

/// Targeting prefixes and browser-output identities can grow independently of
/// provider bytes. Charge their escaped wire sizes before expanding wrappers.
pub(super) fn targeting(
    bookkeeping: u64,
    decoded: u64,
    prefix: &str,
    bundles: &BTreeMap<BidderName, BidderResultBundle>,
) -> Result<u64, ResourceError> {
    let prefix_bytes = bytes(prefix)?;
    let mut source = 0_u64;
    for (bidder, bundle) in bundles {
        let count =
            u64::try_from(bundle.bids.len()).map_err(|_size| ResourceError::CapacityOverflow)?;
        // Fixed targeting fields and map entries are funded even for tiny bids.
        let per_bid = add(add(prefix_bytes, bytes(bidder.as_str())?)?, 64)?;
        source = add(source, mul(count, per_bid)?)?;
        if let Some(browser) = &bundle.browser_auctions {
            let config_count = u64::try_from(browser.configs.len())
                .map_err(|_size| ResourceError::CapacityOverflow)?;
            let per_config = add(
                add(bytes(bidder.as_str())?, bytes(browser.adapter.as_str())?)?,
                64,
            )?;
            source = add(source, mul(config_count, per_config)?)?;
        }
    }
    add(
        add(bookkeeping, decoded)?,
        mul(source, RESPONSE_ASSEMBLY_FACTOR)?,
    )
}

#[cfg(test)]
mod tests {
    use super::{targeting, with_debug};
    use crate::response::{BidderResultBundle, BrowserAuctionResult};
    use auctioneer_bidder::{BidderName, BrowserAuctionConfig};
    use auctioneer_runtime::ResourceError;
    use auctioneer_runtime::debug_budget::debug_peak_bytes;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::iter;

    #[test]
    fn browser_output_preflight_charges_count_and_both_escaped_identities_independently() {
        for count in [0_usize, 1, 7] {
            for (bidder, adapter, bidder_bytes, adapter_bytes) in [
                ("fake_short", "fake_base", 12_u64, 11_u64),
                ("fake_esc\n\"", "fake_base", 14, 11),
                ("fake_short", "fake_esc\t\\", 12, 14),
            ] {
                let bundles = BTreeMap::from([(
                    BidderName::new(bidder),
                    BidderResultBundle {
                        browser_auctions: Some(BrowserAuctionResult {
                            adapter: BidderName::new(adapter),
                            configs: iter::repeat_with(|| BrowserAuctionConfig {
                                imp_id: "i".into(),
                                config: json!({}),
                            })
                            .take(count)
                            .collect(),
                        }),
                        ..BidderResultBundle::default()
                    },
                )]);
                let expected = 19
                    + 37
                    + u64::try_from(count).unwrap() * (bidder_bytes + adapter_bytes + 64) * 128;
                assert_eq!(
                    targeting(19, 37, "unused-prefix", &bundles).unwrap(),
                    expected
                );
            }
        }
    }

    #[test]
    fn response_coexistence_keeps_the_entire_debug_peak_disjoint_from_ordinary_work() {
        for calls in [0, 1, 64, 100, u32::MAX] {
            let debug = debug_peak_bytes(calls).unwrap();
            for ordinary in [0, 1, 16 * 1024 * 1024] {
                assert_eq!(with_debug(ordinary, calls).unwrap(), ordinary + debug);
            }
        }
    }

    #[test]
    fn response_coexistence_refuses_overflow_instead_of_borrowing_debug_capacity() {
        assert_eq!(
            with_debug(u64::MAX, 64),
            Err(ResourceError::CapacityOverflow)
        );
        let debug = debug_peak_bytes(64).unwrap();
        assert_eq!(with_debug(u64::MAX - debug, 64), Ok(u64::MAX));
        assert_eq!(
            with_debug(u64::MAX - debug + 1, 64),
            Err(ResourceError::CapacityOverflow)
        );
    }
}

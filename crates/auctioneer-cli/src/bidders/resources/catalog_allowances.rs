//! C/I/V declaration ceilings only. K and other owner-specific allowances remain
//! pending; the settings/secret-view limit is not a ceiling on complete K.
//! These checks are not descriptor/allocation qualification,
//! matched relative acceptance, per-target total gates, or absolute readiness.

use super::Error;
use auctioneer_runtime::CatalogResourceCosts;

pub(super) fn check_allowances(costs: CatalogResourceCosts) -> Result<(), Error> {
    if costs.resident_bytes > 16 * 1024 * 1024
        || costs.initialization_bytes > 8 * 1024 * 1024
        || costs.schema_validation_per_auction_bytes > 512 * 1024
    {
        Err(Error::ResourceInput)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogResourceCosts, Error, check_allowances};

    #[test]
    fn resident_below_ceiling_is_accepted() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 0x00FF_FFFF,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            }),
            Ok(())
        );
    }

    #[test]
    fn resident_at_ceiling_is_accepted() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 0x0100_0000,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            }),
            Ok(())
        );
    }

    #[test]
    fn resident_above_ceiling_is_refused() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 0x0100_0001,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            }),
            Err(Error::ResourceInput)
        );
    }

    #[test]
    fn initialization_below_ceiling_is_accepted() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 0x007F_FFFF,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            }),
            Ok(())
        );
    }

    #[test]
    fn initialization_at_ceiling_is_accepted() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 0x0080_0000,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            }),
            Ok(())
        );
    }

    #[test]
    fn initialization_above_ceiling_is_refused() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 0x0080_0001,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 401,
            }),
            Err(Error::ResourceInput)
        );
    }

    #[test]
    fn schema_validation_below_ceiling_is_accepted() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 0x0007_FFFF,
                context_per_auction_bytes: 401,
            }),
            Ok(())
        );
    }

    #[test]
    fn schema_validation_at_ceiling_is_accepted() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 0x0008_0000,
                context_per_auction_bytes: 401,
            }),
            Ok(())
        );
    }

    #[test]
    fn schema_validation_above_ceiling_is_refused() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 0x0008_0001,
                context_per_auction_bytes: 401,
            }),
            Err(Error::ResourceInput)
        );
    }

    #[test]
    fn context_above_settings_only_limit_is_accepted() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 101,
                initialization_bytes: 211,
                schema_validation_per_auction_bytes: 307,
                context_per_auction_bytes: 0x0010_0001,
            }),
            Ok(())
        );
    }

    #[test]
    fn combined_component_boundaries_accept_larger_context() {
        assert_eq!(
            check_allowances(CatalogResourceCosts {
                resident_bytes: 0x0100_0000,
                initialization_bytes: 0x0080_0000,
                schema_validation_per_auction_bytes: 0x0008_0000,
                context_per_auction_bytes: 0x0010_0001,
            }),
            Ok(())
        );
    }
}

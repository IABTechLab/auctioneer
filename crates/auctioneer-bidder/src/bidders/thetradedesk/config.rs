//! Supply may come from an impression. Missing, null and empty host values keep
//! that fallback; route safety and request parameters remain request-time checks.
//! There is no additional deployment-only semantic rule for this owner.

use crate::{BidderConfigError, BidderConfigInput};
use validator::{Validate, ValidationErrors};

pub(super) struct Config<'a> {
    _input: BidderConfigInput<'a>,
}

impl<'a> From<BidderConfigInput<'a>> for Config<'a> {
    fn from(input: BidderConfigInput<'a>) -> Self {
        Self { _input: input }
    }
}

impl Validate for Config<'_> {
    fn validate(&self) -> Result<(), ValidationErrors> {
        Ok(())
    }
}

pub(super) fn error(_: ValidationErrors) -> BidderConfigError {
    BidderConfigError {
        field: "settings",
        reason: "invalid",
    }
}

#[cfg(test)]
mod tests {
    use crate::BidderConfigInput;
    use auctioneer_config::{BidderSettings, types::BidderInfo};
    use serde_json::json;

    #[test]
    fn supply_is_not_deployment_required_and_empty_keeps_impression_fallback() {
        let definition = BidderInfo::default();
        for values in [
            json!({}),
            json!({"supply_id":null}),
            json!({"supply_id":""}),
            json!({"supply_id":"fake_supply"}),
        ] {
            let supplied: BidderSettings =
                serde_json::from_value(json!({"implementation":"thetradedesk", "values":values}))
                    .unwrap();
            for enabled in [false, true] {
                for (requested, base) in [
                    (None, None),
                    (Some(&supplied), None),
                    (None, Some(&supplied)),
                    (Some(&supplied), Some(&supplied)),
                ] {
                    assert_eq!(
                        super::super::validate_config(BidderConfigInput {
                            requested,
                            base,
                            definition: Some(&definition),
                            enabled
                        }),
                        Ok(())
                    );
                }
            }
        }
    }
}

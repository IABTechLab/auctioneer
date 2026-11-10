//! Supply may come from an impression. Missing, null and empty host values keep
//! that fallback; route safety and request parameters remain request-time checks.
//! There is no additional deployment-only semantic rule for this owner.

use crate::{BidderConfigError, BidderConfigInput};
use validator::{Validate, ValidationErrors};

/// Explicitly rule-free: supply can be impression-owned, so this view neither
/// invents a deployment requirement nor retains unused configuration input.
pub(super) struct Config;

impl From<BidderConfigInput<'_>> for Config {
    fn from(_input: BidderConfigInput<'_>) -> Self {
        Self
    }
}

impl Validate for Config {
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
    use crate::{BidderSettings, bidder_settings::thetradedesk};
    use auctioneer_config_data::types::BidderInfo;
    use serde_json::json;

    #[test]
    fn typed_supply_preserves_absence_empty_and_host_fallback() {
        let definition = BidderInfo::default();
        let parent: BidderSettings = serde_json::from_value(json!({
            "implementation":"thetradedesk", "settings":{"supply_id":"fake_parent"}
        }))
        .unwrap();
        for (values, expected) in [
            (json!({}), "fake_parent"),
            (json!({"supply_id":null}), "fake_parent"),
            (json!({"supply_id":""}), ""),
            (json!({"supply_id":"fake_supply"}), "fake_supply"),
        ] {
            let supplied: BidderSettings =
                serde_json::from_value(json!({"implementation":"thetradedesk", "settings":values}))
                    .unwrap();
            assert_eq!(
                thetradedesk::resolve(supplied.as_thetradedesk(), parent.as_thetradedesk())
                    .supply_id(),
                Some(expected)
            );
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

//! Signed host source values have no additional semantic constraint. The request
//! owner retains its documented zero/absence normalization and entrypoint defaults.

use crate::{BidderConfigError, BidderConfigInput};
use validator::{Validate, ValidationErrors};

/// Explicitly rule-free; strict shape/binding admission is shared, while source
/// normalization belongs to request construction. No configuration is retained.
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
    use crate::{BidderSettings, bidder_settings::msft};
    use auctioneer_config_data::types::BidderInfo;
    use serde_json::json;

    #[test]
    fn typed_signed_sources_preserve_values_and_independent_field_fallback() {
        let definition = BidderInfo::default();
        let base: BidderSettings = serde_json::from_value(json!({"implementation":"msft", "settings":{"hb_source":i64::MIN, "hb_source_video":i64::MAX}})).unwrap();
        for (values, expected) in [
            (json!({}), (i64::MIN, i64::MAX)),
            (
                json!({"hb_source":null, "hb_source_video":null}),
                (i64::MIN, i64::MAX),
            ),
            (json!({"hb_source":0_i64}), (0, i64::MAX)),
            (json!({"hb_source_video":-7_i64}), (i64::MIN, -7)),
        ] {
            let requested: BidderSettings =
                serde_json::from_value(json!({"implementation":"msft", "settings":values}))
                    .unwrap();
            let resolved = msft::resolve(requested.as_msft(), base.as_msft());
            assert_eq!(
                (resolved.hb_source(), resolved.hb_source_video()),
                (Some(expected.0), Some(expected.1))
            );
            for enabled in [false, true] {
                assert_eq!(
                    super::super::validate_config(BidderConfigInput {
                        requested: Some(&requested),
                        base: Some(&base),
                        definition: Some(&definition),
                        enabled
                    }),
                    Ok(())
                );
            }
        }
        assert_eq!(
            super::super::validate_config(BidderConfigInput {
                requested: None,
                base: None,
                definition: Some(&definition),
                enabled: true
            }),
            Ok(())
        );
    }
}

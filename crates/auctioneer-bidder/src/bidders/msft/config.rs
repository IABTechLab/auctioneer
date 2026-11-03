//! Signed host source values have no additional semantic constraint. The request
//! owner retains its documented zero/absence normalization and entrypoint defaults.

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
    fn optional_signed_sources_keep_zero_null_and_independent_field_fallback() {
        let definition = BidderInfo::default();
        let base: BidderSettings = serde_json::from_value(json!({"implementation":"msft", "values":{"hb_source":i64::MIN, "hb_source_video":i64::MAX}})).unwrap();
        for values in [
            json!({}),
            json!({"hb_source":null, "hb_source_video":null}),
            json!({"hb_source":0_i64}),
            json!({"hb_source_video":-7_i64}),
        ] {
            let requested: BidderSettings =
                serde_json::from_value(json!({"implementation":"msft", "values":values})).unwrap();
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

//! Borrowed deployment rules; no credential lookup or request construction.

use super::request::{regional_host_for, valid_credential_reference};
use crate::{BidderConfigError, BidderConfigInput};
use auctioneer_config::{
    BidderSettings,
    bidder_settings::rubicon::{Settings, resolve},
};
use validator::{Validate, ValidationError, ValidationErrors, ValidationErrorsKind};

pub(super) struct Config<'a> {
    requested: Option<&'a Settings>,
    base: Option<&'a Settings>,
    resolvable: bool,
    enabled: bool,
}

impl<'a> From<BidderConfigInput<'a>> for Config<'a> {
    fn from(input: BidderConfigInput<'a>) -> Self {
        Self {
            requested: input.requested.and_then(BidderSettings::as_rubicon),
            base: input.base.and_then(BidderSettings::as_rubicon),
            resolvable: input.definition.is_some(),
            enabled: input.enabled,
        }
    }
}

impl Validate for Config<'_> {
    fn validate(&self) -> Result<(), ValidationErrors> {
        for supplied in [self.requested, self.base].into_iter().flatten() {
            if supplied
                .region
                .as_deref()
                .is_some_and(|region| regional_host_for(region).is_none())
            {
                return Err(failure("region", "invalid"));
            }
            if supplied
                .credentials_secret
                .as_deref()
                .is_some_and(|key| !valid_credential_reference(key))
            {
                return Err(failure("credentials_secret", "invalid"));
            }
        }
        if self.enabled && self.resolvable {
            let settings = resolve(self.requested, self.base);
            if settings.region().is_none() {
                return Err(failure("region", "required"));
            }
            if settings.credentials_secret().is_none() {
                return Err(failure("credentials_secret", "required"));
            }
        }
        Ok(())
    }
}

fn failure(field: &'static str, reason: &'static str) -> ValidationErrors {
    let mut errors = ValidationErrors::new();
    errors.add(field, ValidationError::new(reason));
    errors
}

pub(super) fn error(errors: &ValidationErrors) -> BidderConfigError {
    for field in ["region", "credentials_secret"] {
        if let Some(ValidationErrorsKind::Field(issues)) = errors.errors().get(field) {
            let reason = if issues.iter().any(|issue| issue.code == "required") {
                "required"
            } else {
                "invalid"
            };
            return BidderConfigError { field, reason };
        }
    }
    BidderConfigError {
        field: "settings",
        reason: "invalid",
    }
}

#[cfg(test)]
mod tests {
    use crate::{BidderConfigError, BidderConfigInput};
    use auctioneer_config::{
        BidderSettings, bidder_settings::rubicon::Settings, types::BidderInfo,
    };
    use serde_json::{Value, json};
    use validator::Validate as _;

    fn settings(values: Value) -> BidderSettings {
        let mut document = json!({"implementation":"rubicon"});
        document["values"] = values;
        serde_json::from_value(document).unwrap()
    }

    fn check(
        requested: Option<&BidderSettings>,
        base: Option<&BidderSettings>,
        definition: Option<&BidderInfo>,
        enabled: bool,
    ) -> Result<(), BidderConfigError> {
        super::super::validate_config(BidderConfigInput {
            requested,
            base,
            definition,
            enabled,
        })
    }

    #[test]
    fn supplied_regions_are_checked_independently_even_when_disabled_or_overridden() {
        let good = settings(json!({"region":"eu", "credentials_secret":"fake_secret"}));
        let mut cases = 0_usize;
        for invalid in ["", "US-EAST", "unknown", "../private"] {
            let bad = settings(json!({"region":invalid}));
            for enabled in [false, true] {
                for (requested, base) in [
                    (Some(&bad), Some(&good)),
                    (Some(&good), Some(&bad)),
                    (None, Some(&bad)),
                    (Some(&bad), None),
                ] {
                    assert_eq!(
                        check(requested, base, None, enabled),
                        Err(BidderConfigError {
                            field: "region",
                            reason: "invalid"
                        })
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 32);
    }

    #[test]
    fn enabled_resolvable_identity_requires_intrinsic_fields_and_inherits_each_field() {
        let definition = BidderInfo::default();
        assert_eq!(
            check(None, None, Some(&definition), true),
            Err(BidderConfigError {
                field: "region",
                reason: "required"
            })
        );
        let region = settings(json!({"region":"us-east"}));
        assert_eq!(
            check(Some(&region), None, Some(&definition), true),
            Err(BidderConfigError {
                field: "credentials_secret",
                reason: "required"
            })
        );
        let credential = settings(json!({"credentials_secret":"fake_secret"}));
        for (requested, base) in [
            (Some(&region), Some(&credential)),
            (Some(&credential), Some(&region)),
        ] {
            assert_eq!(check(requested, base, Some(&definition), true), Ok(()));
        }
        for (known, enabled) in [(true, false), (false, true), (false, false)] {
            assert_eq!(
                check(None, None, known.then_some(&definition), enabled),
                Ok(())
            );
        }
    }

    #[test]
    fn null_and_absent_fall_back_but_optional_external_url_may_stay_empty() {
        let definition = BidderInfo::default();
        for region in ["us-east", "us-west", "eu", "apac"] {
            let base = settings(json!({"region":region, "credentials_secret":"fake_secret"}));
            for values in [
                json!({}),
                json!({"region":null,"credentials_secret":null,"external_url":null}),
                json!({"external_url":""}),
            ] {
                let requested = settings(values);
                assert_eq!(
                    check(Some(&requested), Some(&base), Some(&definition), true),
                    Ok(())
                );
            }
        }
    }

    #[test]
    fn programmatic_credential_errors_are_static_and_do_not_expose_canary() {
        const CANARY: &str = "fake_private_credential_location";
        let good = settings(json!({"region":"eu", "credentials_secret":"fake_secret"}));
        for invalid in [String::new(), "x".repeat(129), format!("{CANARY}\n")] {
            let bad = Settings {
                credentials_secret: Some(invalid),
                ..Settings::default()
            };
            for enabled in [false, true] {
                for (requested, base) in [
                    (Some(&bad), good.as_rubicon()),
                    (good.as_rubicon(), Some(&bad)),
                ] {
                    let error = super::Config {
                        requested,
                        base,
                        resolvable: false,
                        enabled,
                    }
                    .validate()
                    .map_err(|errors| super::error(&errors))
                    .unwrap_err();
                    assert_eq!(
                        error,
                        BidderConfigError {
                            field: "credentials_secret",
                            reason: "invalid"
                        }
                    );
                    assert!(!format!("{error:?} {error}").contains(CANARY));
                }
            }
        }
        let supplied = settings(json!({"region":"unknown", "credentials_secret":CANARY}));
        let error = check(Some(&supplied), None, None, false).unwrap_err();
        assert_eq!(
            error,
            BidderConfigError {
                field: "region",
                reason: "invalid"
            }
        );
        assert!(!format!("{error:?} {error}").contains(CANARY));
    }
}

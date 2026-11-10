//! Borrowed deployment endpoint rules, shared with request-time selection.

use crate::bidder_settings::mocktioneer::{Settings, resolve};
use crate::{BidderConfigError, BidderConfigInput, BidderSettings};
use validator::{Validate, ValidationError, ValidationErrors};

pub(super) struct Config<'a> {
    requested: Option<&'a Settings>,
    base: Option<&'a Settings>,
    enabled: bool,
}

impl<'a> From<BidderConfigInput<'a>> for Config<'a> {
    fn from(input: BidderConfigInput<'a>) -> Self {
        Self {
            requested: input.requested.and_then(BidderSettings::as_mocktioneer),
            base: input.base.and_then(BidderSettings::as_mocktioneer),
            enabled: input.enabled,
        }
    }
}

impl Validate for Config<'_> {
    fn validate(&self) -> Result<(), ValidationErrors> {
        let selected = endpoint(self.requested, self.base).map_err(|()| {
            let mut errors = ValidationErrors::new();
            errors.add("endpoint", ValidationError::new("invalid"));
            errors
        })?;
        if self.enabled && selected.is_none() {
            let mut errors = ValidationErrors::new();
            errors.add("endpoint", ValidationError::new("required_when_enabled"));
            return Err(errors);
        }
        Ok(())
    }
}

/// Check both supplied entries before resolving requested/base fallback.
pub(super) fn endpoint<'a>(
    requested: Option<&'a Settings>,
    base: Option<&'a Settings>,
) -> Result<Option<&'a str>, ()> {
    for supplied in [requested, base].into_iter().flatten() {
        if let Some(endpoint) = supplied.endpoint.as_deref() {
            auctioneer_config::validate_configured_bidder_endpoint(
                endpoint,
                cfg!(feature = "insecure-bidders"),
            )
            .map_err(|_reason| ())?;
        }
    }
    Ok(resolve(requested, base).endpoint())
}

pub(super) fn error(errors: &ValidationErrors) -> BidderConfigError {
    BidderConfigError {
        field: "endpoint",
        reason: if errors
            .field_errors()
            .get("endpoint")
            .is_some_and(|entries| {
                entries
                    .iter()
                    .any(|entry| entry.code == "required_when_enabled")
            }) {
            "required when enabled"
        } else {
            "invalid"
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::{MocktioneerBidder, validate_config};
    use crate::bidder_settings::mocktioneer::{Settings, resolve};
    use crate::{Bidder as _, BidderConfigError, BidderConfigInput, BidderSettings};
    use auctioneer_config_data::types::BidderInfo;
    use serde_json::{Value, json};
    use validator::Validate as _;

    fn settings(values: Value) -> BidderSettings {
        let mut document = json!({"implementation":"mocktioneer"});
        document["settings"] = values;
        serde_json::from_value(document).unwrap()
    }

    #[test]
    fn an_enabled_identity_requires_an_explicit_resolved_endpoint() {
        let empty = settings(json!({}));
        let null = settings(json!({"endpoint":null}));
        for requested in [None, Some(&empty), Some(&null)] {
            for base in [None, Some(&empty), Some(&null)] {
                for enabled in [false, true] {
                    assert_eq!(
                        validate_config(BidderConfigInput {
                            requested,
                            base,
                            definition: None,
                            enabled,
                        }),
                        if enabled {
                            Err(BidderConfigError {
                                field: "endpoint",
                                reason: "required when enabled",
                            })
                        } else {
                            Ok(())
                        }
                    );
                }
                assert!(
                    MocktioneerBidder
                        .configured_endpoint(requested, base)
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn omitted_and_null_endpoint_inherit_base_without_an_implicit_destination() {
        let base = settings(json!({"endpoint":"https://base.example.test:8443/base?route=one"}));
        let explicit =
            settings(json!({"endpoint":"https://requested.example.test:9443/requested?route=two"}));
        let definition = BidderInfo::default();
        for values in [json!({}), json!({"endpoint":null})] {
            let absent = settings(values);
            assert!(absent.as_mocktioneer().unwrap().endpoint.is_none());
            for (requested, parent, expected) in [
                (
                    Some(&absent),
                    Some(&base),
                    Some("https://base.example.test:8443/base?route=one"),
                ),
                (
                    Some(&base),
                    Some(&absent),
                    Some("https://base.example.test:8443/base?route=one"),
                ),
                (
                    Some(&explicit),
                    Some(&base),
                    Some("https://requested.example.test:9443/requested?route=two"),
                ),
                (
                    Some(&base),
                    Some(&explicit),
                    Some("https://base.example.test:8443/base?route=one"),
                ),
                (
                    None,
                    Some(&base),
                    Some("https://base.example.test:8443/base?route=one"),
                ),
                (
                    Some(&base),
                    None,
                    Some("https://base.example.test:8443/base?route=one"),
                ),
                (Some(&absent), None, None),
                (None, Some(&absent), None),
                (Some(&absent), Some(&absent), None),
                (None, None, None),
            ] {
                assert_eq!(
                    resolve(
                        requested.and_then(BidderSettings::as_mocktioneer),
                        parent.and_then(BidderSettings::as_mocktioneer)
                    )
                    .endpoint(),
                    expected
                );
                let selected = MocktioneerBidder.configured_endpoint(requested, parent);
                if expected.is_some() {
                    assert_eq!(
                        selected.unwrap().map(crate::ConfiguredEndpoint::as_str),
                        expected
                    );
                } else {
                    assert!(selected.is_err());
                }
                for enabled in [false, true] {
                    for candidate_definition in [None, Some(&definition)] {
                        assert_eq!(
                            validate_config(BidderConfigInput {
                                requested,
                                base: parent,
                                definition: candidate_definition,
                                enabled
                            }),
                            if enabled && expected.is_none() {
                                Err(BidderConfigError {
                                    field: "endpoint",
                                    reason: "required when enabled",
                                })
                            } else {
                                Ok(())
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn selected_https_literal_preserves_independent_path_query_port_and_borrow() {
        for literal in [
            "https://mock-bidder.example.test",
            "https://mock-bidder.example.test/openrtb2/auction",
            "https://mock-bidder.example.test?route=one&format=json",
            "https://mock-bidder.example.test:8443",
            "https://mock-bidder.example.test:443",
            "https://mock-bidder.example.test:8443/a%20b?route=a%2Fb",
        ] {
            let configured = settings(json!({"endpoint":literal}));
            let stored = configured
                .as_mocktioneer()
                .unwrap()
                .endpoint
                .as_deref()
                .unwrap();
            for (requested, base) in [(Some(&configured), None), (None, Some(&configured))] {
                let selected = MocktioneerBidder
                    .configured_endpoint(requested, base)
                    .unwrap()
                    .unwrap()
                    .as_str();
                assert_eq!(selected, literal);
                assert_eq!(selected.as_ptr(), stored.as_ptr());
                assert_eq!(
                    validate_config(BidderConfigInput {
                        requested,
                        base,
                        definition: None,
                        enabled: true
                    }),
                    Ok(())
                );
            }
        }
    }

    #[test]
    fn exact_loopbacks_require_explicit_development_transport() {
        for literal in [
            "http://127.0.0.1:8787/openrtb2/auction",
            "http://[::1]:8787/openrtb2/auction?route=one",
            "http://localhost:8787/openrtb2/auction",
            "https://localhost:9443/openrtb2/auction",
        ] {
            let configured = settings(json!({"endpoint":literal}));
            for (requested, base) in [(Some(&configured), None), (None, Some(&configured))] {
                assert_eq!(
                    validate_config(BidderConfigInput {
                        requested,
                        base,
                        definition: None,
                        enabled: false
                    }),
                    if cfg!(feature = "insecure-bidders") {
                        Ok(())
                    } else {
                        Err(BidderConfigError {
                            field: "endpoint",
                            reason: "invalid",
                        })
                    }
                );
                let selected = MocktioneerBidder.configured_endpoint(requested, base);
                if cfg!(feature = "insecure-bidders") {
                    assert_eq!(selected.unwrap().unwrap().as_str(), literal);
                } else {
                    assert!(selected.is_err());
                }
            }
        }
    }

    #[test]
    fn supplied_invalid_endpoints_are_independently_rejected_disabled_or_shadowed() {
        let good = settings(json!({"endpoint":"https://approved.example.test/auction"}));
        let invalid = [
            "",
            "http://public.example.test/private-canary",
            "https://10.0.0.1/private-canary",
            "https://172.16.0.1/private-canary",
            "https://192.168.0.1/private-canary",
            "https://169.254.0.1/private-canary",
            "https://192.0.2.1/private-canary",
            "https://[fc00::1]/private-canary",
            "https://[::ffff:127.0.0.1]/private-canary",
            "http://127.0.0.2:8787/private-canary",
            "http://127.1:8787/private-canary",
            "http://2130706433:8787/private-canary",
            "http://0x7f000001:8787/private-canary",
            "http://0177.0.0.1:8787/private-canary",
            "http://[0:0:0:0:0:0:0:1]:8787/private-canary",
            "http://localhost.:8787/private-canary",
            "https://127.0.0.1.nip.io/private-canary",
            "https://app.localtest.me/private-canary",
            "https://sub.localhost/private-canary",
            "https://@public.example.test/private-canary",
            "https://private-canary:secret@public.example.test/auction",
            "https://public.example.test/private-canary#",
            "https://public.example.test/private-canary#fragment",
            "https://{{.Host}}/private-canary",
            "https://public.example.test/{{.Path}}/private-canary",
            "https://public.example.test/private-canary?route={{.Route}}",
            "https://public.example.test/private-canary?route=}}",
            "public.example.test/private-canary",
            "//public.example.test/private-canary",
            "https:public.example.test/private-canary",
            "https:/public.example.test/private-canary",
            "https:///public.example.test/private-canary",
            "https://",
            "https://public.example.test/private-canary\\path",
            " https://public.example.test/private-canary",
            "https://public.example.test/private-canary ",
            "https://public.example.test/private canary",
            "https://public.example.test/private\ncanary",
            "https://public.example.test/private\u{7f}canary",
            "https://public.example.test/private\u{2003}canary",
            "ftp://public.example.test/private-canary",
        ];
        let definition = BidderInfo::default();
        let mut cases = 0_usize;
        for literal in invalid {
            let bad = settings(json!({"endpoint":literal}));
            for enabled in [false, true] {
                for candidate_definition in [None, Some(&definition)] {
                    for (requested, base) in [
                        (Some(&bad), Some(&good)),
                        (Some(&good), Some(&bad)),
                        (Some(&bad), None),
                        (None, Some(&bad)),
                    ] {
                        let error = validate_config(BidderConfigInput {
                            requested,
                            base,
                            definition: candidate_definition,
                            enabled,
                        })
                        .unwrap_err();
                        assert_eq!(
                            error,
                            BidderConfigError {
                                field: "endpoint",
                                reason: "invalid"
                            }
                        );
                        assert_eq!(
                            error.to_string(),
                            "bidder configuration invalid: endpoint (invalid)"
                        );
                        assert!(!format!("{error:?} {error}").contains("private-canary"));
                        let refusal = MocktioneerBidder
                            .configured_endpoint(requested, base)
                            .err()
                            .unwrap();
                        assert_eq!(refusal.code, 5);
                        assert_eq!(refusal.message, "invalid configured bidder endpoint");
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 656);
    }

    #[test]
    fn programmatic_settings_enforce_utf8_byte_limit_and_sanitize_validator_errors() {
        let at_limit = format!("https://bidder.example/{}a", "\u{e9}".repeat(1012));
        let over_limit = format!("https://bidder.example/{}aa", "\u{e9}".repeat(1012));
        assert_eq!(at_limit.len(), 2048);
        assert_eq!(over_limit.len(), 2049);
        let good = Settings {
            endpoint: Some(at_limit),
        };
        for (requested, base) in [(Some(&good), None), (None, Some(&good))] {
            super::Config {
                requested,
                base,
                enabled: true,
            }
            .validate()
            .unwrap();
        }
        for literal in [
            String::new(),
            over_limit,
            "https://private-canary:secret@public.example.test/auction".to_owned(),
        ] {
            let bad = Settings {
                endpoint: Some(literal.clone()),
            };
            for (requested, base) in [(Some(&bad), Some(&good)), (Some(&good), Some(&bad))] {
                let errors = super::Config {
                    requested,
                    base,
                    enabled: true,
                }
                .validate()
                .unwrap_err();
                assert!(!format!("{errors:?}").contains("private-canary"));
                let fields = errors.field_errors();
                assert_eq!(fields.len(), 1);
                assert_eq!(fields["endpoint"][0].code, "invalid");
                assert!(fields["endpoint"][0].params.is_empty());
                assert_eq!(
                    super::error(&errors),
                    BidderConfigError {
                        field: "endpoint",
                        reason: "invalid"
                    }
                );
            }
            let direct = settings(json!({"endpoint":literal}));
            let error = MocktioneerBidder
                .configured_endpoint(Some(&direct), None)
                .err()
                .unwrap();
            assert_eq!(error.code, 5);
            assert_eq!(error.message, "invalid configured bidder endpoint");
        }
    }
}

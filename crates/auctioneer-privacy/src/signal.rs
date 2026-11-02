//! GDPR `gdpr_applies` signal math (spec §7.1).
//!
//! Two signals, deliberately separate so call sites cannot confuse
//! them. `resolve_wire_signal` reads only the wire-visible request
//! fields (`regs.gdpr`, `regs.gpp_sid`) and never consults geo or the
//! host default. `auction_geo_signal` is the auction-only geo ladder
//! that resolves an `Ambiguous` wire signal against `user.geo` /
//! `device.geo` and the host `default_value`. The sync path uses the
//! raw host default instead and never calls the geo ladder at all.

use auctioneer_openrtb::{NonFatalWarning, NormalizedBidRequest};

use crate::error::PrivacyError;
use crate::warnings;

/// GPP section id for TCF EU v2 — its presence in a non-empty
/// `regs.gpp_sid` is what makes GDPR apply.
const GPP_SECTION_TCFEU2: i32 = 2;

/// Host `gdpr.default_value` that means "GDPR does not apply".
/// Anything else means it does.
const HOST_DEFAULT_NO: &str = "0";

/// GDPR `gdpr_applies` signal. `Yes(1)` / `No(0)` / `Ambiguous(-1)`.
///
/// Variant order follows the wire numbering (1 / 0 / -1), which
/// reads better than alphabetical when cross-referencing wire semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Yes,
    No,
    Ambiguous,
}

/// The wire-visible GDPR inputs. No geo, no host default — those
/// belong to [`auction_geo_signal`].
pub struct WireSignalInput<'a> {
    pub regs_gdpr: Option<i32>,
    pub gpp_sid: Option<&'a [i32]>,
}

/// One geo object's country. The distinction that matters is the
/// OUTER `Option` on the fields of [`AuctionGeoInput`]: a present
/// object with an absent country resolves differently from an absent
/// object.
pub struct GeoView<'a> {
    pub country: Option<&'a str>,
}

pub struct AuctionGeoInput<'a> {
    /// `None` = the `user.geo` OBJECT is absent; `Some(view)` = the
    /// object is present (its `country` may still be `None`/malformed).
    pub user_geo: Option<GeoView<'a>>,
    /// `None` = the `device.geo` OBJECT is absent.
    pub device_geo: Option<GeoView<'a>>,
    pub host_default_value: &'a str,
    pub eea_countries: &'a [&'a str],
}

/// Resolve the wire GDPR signal from `regs.gdpr` + `regs.gpp_sid`.
///
/// A non-empty `gpp_sid` overrides `regs.gdpr` entirely, so an invalid
/// `regs.gdpr` alongside one is never consulted and never fatal. Both
/// fields absent is a legitimate terminal `Ambiguous`; the geo-aware
/// default belongs to [`auction_geo_signal`], not here.
///
/// # Errors
/// Returns [`PrivacyError::InvalidRegsGdpr`] when `regs.gdpr` carries a
/// value other than 0 or 1 AND `gpp_sid` is absent or empty. The
/// exchange boundary maps it to `AuctionError::BadInput` → 400.
pub fn resolve_wire_signal(
    input: &WireSignalInput<'_>,
) -> Result<(Signal, Vec<NonFatalWarning>), PrivacyError> {
    let mut emitted = Vec::new();
    // Diagnostic only, and independent of the value: any present
    // `regs.gdpr` alongside any present `gpp_sid` — empty or not —
    // warns, then resolution continues unchanged.
    if input.regs_gdpr.is_some() && input.gpp_sid.is_some() {
        emitted.push(warnings::regs_gpp_conflict());
    }
    if let Some(sections) = input.gpp_sid
        && !sections.is_empty()
    {
        let signal = if sections.contains(&GPP_SECTION_TCFEU2) {
            Signal::Yes
        } else {
            Signal::No
        };
        return Ok((signal, emitted));
    }
    match input.regs_gdpr {
        Some(0_i32) => Ok((Signal::No, emitted)),
        Some(1_i32) => Ok((Signal::Yes, emitted)),
        Some(got) => Err(PrivacyError::InvalidRegsGdpr { got }),
        None => Ok((Signal::Ambiguous, emitted)),
    }
}

/// Surface only the fatal half of [`resolve_wire_signal`], so the
/// handler can reject an invalid `regs.gdpr` before any alias or
/// disabled-bidder work preempts it.
///
/// Precedence-aware by construction: it runs the SAME resolver, so a
/// non-empty `gpp_sid` overrides an invalid `regs.gdpr` here exactly as
/// it does in the auction. Malformed `regs.gpp` / `user.consent` are
/// §7.7 warn-and-continue inputs and are not inspected at all.
///
/// # Errors
/// Returns [`PrivacyError::InvalidRegsGdpr`] on the same input
/// [`resolve_wire_signal`] rejects.
pub fn validate_wire_signal_input(req: &NormalizedBidRequest) -> Result<(), PrivacyError> {
    let input = WireSignalInput {
        regs_gdpr: req.regs.as_ref().and_then(|regs| regs.gdpr),
        gpp_sid: req.regs.as_ref().and_then(|regs| regs.gpp_sid.as_deref()),
    };
    resolve_wire_signal(&input).map(|_| ())
}

/// The auction-only geo ladder. Never emits warnings — invalid inputs
/// are already handled by ingest.
///
/// The `user.geo` OBJECT being present is what selects user-geo and
/// blocks the `device.geo` fallback, even when its country is
/// malformed. A well-formed ISO-3 country decides (EEA → `Yes`,
/// non-EEA → `No`); anything else falls to the host `default_value`.
#[must_use]
pub fn auction_geo_signal(input: &AuctionGeoInput<'_>) -> Signal {
    let default = if input.host_default_value == HOST_DEFAULT_NO {
        Signal::No
    } else {
        Signal::Yes
    };
    let Some(selected) = input.user_geo.as_ref().or(input.device_geo.as_ref()) else {
        return default;
    };
    let Some(country) = selected.country.filter(|code| is_iso3_country(code)) else {
        return default;
    };
    if input
        .eea_countries
        .iter()
        .any(|eea| eea.eq_ignore_ascii_case(country))
    {
        Signal::Yes
    } else {
        Signal::No
    }
}

/// ISO 3166-1 alpha-3 shape: exactly three ASCII letters. Anything
/// else (empty, alpha-2, digits, a longer name) is malformed and the
/// caller falls back to the host default rather than reading it as a
/// non-EEA country.
fn is_iso3_country(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::{
        AuctionGeoInput, GeoView, PrivacyError, Signal, WireSignalInput, auction_geo_signal,
        resolve_wire_signal, validate_wire_signal_input,
    };
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use auctioneer_openrtb::types::inventory::Site;
    use auctioneer_openrtb::types::regs::Regs;
    use auctioneer_openrtb::types::user::User;
    use auctioneer_openrtb::{
        NonFatalWarning, NormalizedBidRequest, PrepareLimits, RequestHeaders, WarningCode, prepare,
    };

    /// Impression cap wide enough that the fixtures below never reach
    /// it; the cap itself is exercised by the max-impressions tests in
    /// `auctioneer-openrtb`.
    const IMP_CAP_NOT_UNDER_TEST: PrepareLimits = PrepareLimits {
        max_impressions: 100,
        max_copied_header_bytes: 4_096,
    };

    fn wire(
        regs_gdpr: Option<i32>,
        gpp_sid: Option<&[i32]>,
    ) -> Result<(Signal, Vec<NonFatalWarning>), PrivacyError> {
        resolve_wire_signal(&WireSignalInput { regs_gdpr, gpp_sid })
    }

    fn geo(
        user_geo: Option<GeoView<'_>>,
        device_geo: Option<GeoView<'_>>,
        host_default_value: &str,
    ) -> Signal {
        auction_geo_signal(&AuctionGeoInput {
            user_geo,
            device_geo,
            host_default_value,
            eea_countries: auctioneer_config::eea_set(),
        })
    }

    fn normalized(regs: Option<Regs>, user: Option<User>) -> NormalizedBidRequest {
        let req = BidRequest {
            id: "req-1".to_owned(),
            imp: vec![Imp {
                id: "imp-1".to_owned(),
                banner: Some(Banner::default()),
                ..Imp::default()
            }],
            site: Some(Site::default()),
            regs,
            user,
            ..BidRequest::default()
        };
        prepare(req, &RequestHeaders::default(), &IMP_CAP_NOT_UNDER_TEST)
            .expect("fixture request is structurally valid")
    }

    fn conflict_only(warnings: &[NonFatalWarning]) {
        assert_eq!(warnings.len(), 1, "exactly one conflict warning");
        let warning = warnings.first().expect("one warning");
        assert_eq!(warning.code, WarningCode::InvalidPrivacyConsent);
        assert_eq!(warning.scope.as_deref(), Some("regs.gpp_sid"));
    }

    #[test]
    fn signal_is_copy_and_eq() {
        let signal = Signal::Yes;
        let copied = signal;
        assert_eq!(signal, copied);
        assert_ne!(Signal::Yes, Signal::No);
        assert_ne!(Signal::Yes, Signal::Ambiguous);
    }

    #[test]
    fn wire_gpp_sid_section_2_present_yields_yes_ignoring_regs_gdpr_zero() {
        let (signal, warnings) = wire(Some(0_i32), Some(&[2_i32])).expect("gpp_sid decides");
        assert_eq!(signal, Signal::Yes);
        conflict_only(&warnings);
    }

    #[test]
    fn wire_gpp_sid_section_2_absent_but_nonempty_yields_no_ignoring_regs_gdpr_one() {
        let (signal, warnings) = wire(Some(1_i32), Some(&[6_i32])).expect("gpp_sid decides");
        assert_eq!(signal, Signal::No);
        conflict_only(&warnings);
    }

    #[test]
    fn wire_empty_gpp_sid_then_regs_gdpr_zero_yields_no() {
        let (signal, _) = wire(Some(0_i32), Some(&[])).expect("regs.gdpr decides");
        assert_eq!(signal, Signal::No);
    }

    #[test]
    fn wire_empty_gpp_sid_then_regs_gdpr_one_yields_yes() {
        let (signal, _) = wire(Some(1_i32), Some(&[])).expect("regs.gdpr decides");
        assert_eq!(signal, Signal::Yes);
    }

    #[test]
    fn wire_empty_gpp_sid_then_regs_gdpr_two_returns_bad_input_400() {
        // The exchange boundary maps this to AuctionError::BadInput → 400.
        assert_eq!(
            wire(Some(2_i32), Some(&[])),
            Err(PrivacyError::InvalidRegsGdpr { got: 2_i32 })
        );
    }

    #[test]
    fn wire_nonempty_gpp_sid_with_invalid_regs_gdpr_is_ok() {
        // §7.1 rule 1: a non-empty gpp_sid overrides regs.gdpr
        // entirely, so the invalid value is never evaluated.
        let (signal, warnings) = wire(Some(2_i32), Some(&[2_i32])).expect("gpp_sid overrides");
        assert_eq!(signal, Signal::Yes);
        conflict_only(&warnings);
    }

    #[test]
    fn wire_no_gpp_no_regs_yields_raw_ambiguous_and_no_warning() {
        // Geo and the host default are NOT applied here.
        let (signal, warnings) = wire(None, None).expect("both fields absent is terminal");
        assert_eq!(signal, Signal::Ambiguous);
        assert!(warnings.is_empty());
    }

    #[test]
    fn wire_gpp_sid_only_section_6_uspv1_yields_no_gdpr() {
        let (signal, warnings) = wire(None, Some(&[6_i32])).expect("gpp_sid decides");
        assert_eq!(signal, Signal::No);
        assert!(warnings.is_empty(), "no regs.gdpr means no conflict");
    }

    #[test]
    fn wire_regs_gdpr_negative_int_returns_bad_input_400() {
        assert_eq!(
            wire(Some(-1_i32), None),
            Err(PrivacyError::InvalidRegsGdpr { got: -1_i32 })
        );
    }

    #[test]
    fn wire_regs_gdpr_1_and_present_empty_gpp_sid_warns_but_falls_through_to_yes() {
        let (signal, warnings) = wire(Some(1_i32), Some(&[])).expect("regs.gdpr decides");
        assert_eq!(signal, Signal::Yes);
        conflict_only(&warnings);
    }

    #[test]
    fn wire_regs_gdpr_0_and_present_empty_gpp_sid_warns_but_falls_through_to_no() {
        let (signal, warnings) = wire(Some(0_i32), Some(&[])).expect("regs.gdpr decides");
        assert_eq!(signal, Signal::No);
        conflict_only(&warnings);
    }

    #[test]
    fn wire_regs_gdpr_1_and_present_nonempty_gpp_sid_still_warns_conflict() {
        let (signal, warnings) = wire(Some(1_i32), Some(&[2_i32])).expect("gpp_sid decides");
        assert_eq!(signal, Signal::Yes);
        conflict_only(&warnings);
    }

    #[test]
    fn wire_regs_gdpr_set_and_absent_gpp_sid_no_conflict_no_warning() {
        let (signal, warnings) = wire(Some(1_i32), None).expect("regs.gdpr decides");
        assert_eq!(signal, Signal::Yes);
        assert!(warnings.is_empty(), "absent gpp_sid is not a conflict");
    }

    #[test]
    fn geo_default_zero_no_country_yields_no() {
        assert_eq!(geo(None, None, "0"), Signal::No);
    }

    #[test]
    fn geo_default_one_no_country_yields_yes() {
        assert_eq!(geo(None, None, "1"), Signal::Yes);
    }

    #[test]
    fn geo_eea_country_forces_yes_over_default_no() {
        let user = GeoView {
            country: Some("DEU"),
        };
        assert_eq!(geo(Some(user), None, "0"), Signal::Yes);
    }

    #[test]
    fn geo_non_eea_iso3_forces_no_over_default_yes() {
        let user = GeoView {
            country: Some("USA"),
        };
        assert_eq!(geo(Some(user), None, "1"), Signal::No);
    }

    #[test]
    fn geo_malformed_iso3_falls_back_to_default() {
        // Host default "1" makes the fallback observable: a
        // well-formed non-EEA code would have yielded No.
        for country in ["US", "USAX", "U5A"] {
            let user = GeoView {
                country: Some(country),
            };
            assert_eq!(geo(Some(user), None, "1"), Signal::Yes, "{country}");
        }
    }

    #[test]
    fn geo_user_takes_precedence_over_device_when_both_present() {
        let user = GeoView {
            country: Some("USA"),
        };
        let device = GeoView {
            country: Some("DEU"),
        };
        assert_eq!(geo(Some(user), Some(device), "1"), Signal::No);
    }

    #[test]
    fn geo_empty_string_country_ignored_falls_back_to_default() {
        let empty = || GeoView { country: Some("") };
        assert_eq!(geo(Some(empty()), None, "0"), Signal::No);
        assert_eq!(geo(Some(empty()), None, "1"), Signal::Yes);
    }

    #[test]
    fn geo_never_emits_warnings() {
        // The signature carries the guarantee: binding the ladder to a
        // `-> Signal` fn pointer only compiles while there is no
        // warnings tuple for a caller to be handed.
        let ladder: fn(&AuctionGeoInput<'_>) -> Signal = auction_geo_signal;
        assert_eq!(
            ladder(&AuctionGeoInput {
                user_geo: Some(GeoView {
                    country: Some("DEU"),
                }),
                device_geo: None,
                host_default_value: "0",
                eea_countries: auctioneer_config::eea_set(),
            }),
            Signal::Yes
        );
    }

    #[test]
    fn geo_user_object_present_malformed_country_uses_host_default_not_device() {
        let user = GeoView { country: None };
        let device = GeoView {
            country: Some("DEU"),
        };
        assert_eq!(geo(Some(user), Some(device), "0"), Signal::No);
    }

    #[test]
    fn geo_user_object_absent_falls_through_to_device() {
        let device = GeoView {
            country: Some("DEU"),
        };
        assert_eq!(geo(None, Some(device), "0"), Signal::Yes);
    }

    #[test]
    fn wire_signal_input_accepts_absent() {
        validate_wire_signal_input(&normalized(None, None)).expect("no regs is fine");
        let regs = Regs {
            gdpr: None,
            gpp_sid: Some(Vec::new()),
            ..Regs::default()
        };
        validate_wire_signal_input(&normalized(Some(regs), None))
            .expect("absent gdpr with an empty gpp_sid is fine");
    }

    #[test]
    fn wire_signal_input_accepts_gdpr_0_and_1() {
        for gdpr in [0_i32, 1_i32] {
            let regs = Regs {
                gdpr: Some(gdpr),
                ..Regs::default()
            };
            validate_wire_signal_input(&normalized(Some(regs), None)).expect("legal gdpr value");
        }
    }

    #[test]
    fn wire_signal_input_rejects_invalid_gdpr_without_gpp_sid() {
        let regs = Regs {
            gdpr: Some(2_i32),
            ..Regs::default()
        };
        assert_eq!(
            validate_wire_signal_input(&normalized(Some(regs), None)),
            Err(PrivacyError::InvalidRegsGdpr { got: 2_i32 })
        );
    }

    #[test]
    fn wire_signal_input_gpp_sid_overrides_invalid_gdpr() {
        // §7.1 rule 1 consults gpp_sid first, so the invalid
        // regs.gdpr is never evaluated and the request is not a 400.
        let regs = Regs {
            gdpr: Some(2_i32),
            gpp_sid: Some(vec![2_i32]),
            ..Regs::default()
        };
        validate_wire_signal_input(&normalized(Some(regs), None))
            .expect("non-empty gpp_sid overrides an invalid regs.gdpr");
        let (signal, _) = wire(Some(2_i32), Some(&[2_i32])).expect("resolver agrees");
        assert_eq!(signal, Signal::Yes);
    }

    #[test]
    fn wire_signal_input_ignores_malformed_gpp_and_consent() {
        // Malformed TCF/GPP is §7.7 warn-and-continue, never a 400.
        let regs = Regs {
            gdpr: Some(1_i32),
            gpp: Some("!!!not-a-gpp-string!!!".to_owned()),
            ..Regs::default()
        };
        let user = User {
            consent: Some("~~~not-a-tc-string~~~".to_owned()),
            ..User::default()
        };
        validate_wire_signal_input(&normalized(Some(regs), Some(user)))
            .expect("malformed consent strings are not fatal");
    }
}

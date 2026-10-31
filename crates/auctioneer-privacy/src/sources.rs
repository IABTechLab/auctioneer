//! Auction privacy source selection, separate from consent authorization.

use std::fmt;

use auctioneer_openrtb::{NonFatalWarning, WarningCode, types::bid_request::BidRequest};

use crate::{PrivacyError, Signal, Usp, UspParseError, WireSignalInput, gpp, resolve_wire_signal};

/// The source selected for TCF authorization and its diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsentSource {
    Legacy,
    Gpp,
}

/// Within-limit outer framing state. Resource violations never enter this enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeState {
    Absent,
    Valid,
    Malformed,
}

/// Borrowed downgrade inputs, separate from the immutable authorization decisions.
#[derive(Clone, Copy)]
pub struct LegacyProjection<'a> {
    pub gdpr: Option<i32>,
    pub consent: Option<&'a str>,
    pub usp: Option<Usp>,
    pub clear_legacy_usp: bool,
}

impl fmt::Debug for LegacyProjection<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyProjection").finish_non_exhaustive()
    }
}

/// Borrowed TCF input. Present-empty GPP remains authoritative, not absent.
#[derive(Clone, Copy)]
pub struct SelectedConsent<'a> {
    raw: Option<&'a str>,
    source: ConsentSource,
}

impl fmt::Debug for SelectedConsent<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectedConsent")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<'a> SelectedConsent<'a> {
    #[must_use]
    pub fn raw(self) -> Option<&'a str> {
        self.raw
    }

    #[must_use]
    pub fn source(self) -> ConsentSource {
        self.source
    }

    /// Fixed source-scoped warning, emitted only if authorization parsed TCF.
    #[must_use]
    pub fn malformed_warning(self) -> NonFatalWarning {
        match self.source {
            ConsentSource::Legacy => warning(
                "user.consent",
                "user.consent is not a valid TCF v2 string; treating as no consent",
            ),
            ConsentSource::Gpp => warning(
                "regs.gpp",
                "regs.gpp section 2 is not a valid TCF v2 string; treating as no consent",
            ),
        }
    }
}

/// Immutable normalized inputs. No TCF decoding or policy enablement occurs here.
pub struct NormalizedSources<'a> {
    envelope: EnvelopeState,
    wire_signal: Signal,
    tcf: SelectedConsent<'a>,
    usp: Result<Option<Usp>, UspParseError>,
    active_tcf: Option<&'a str>,
    active_usp: Option<Usp>,
    downgrade_gdpr: Option<i32>,
    clear_legacy_usp: bool,
    wire_warnings: Vec<NonFatalWarning>,
    tcf_conflict: bool,
    usp_conflict: bool,
    selected_usp_invalid: bool,
}

impl fmt::Debug for NormalizedSources<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NormalizedSources")
            .field("envelope", &self.envelope)
            .finish_non_exhaustive()
    }
}

impl<'a> NormalizedSources<'a> {
    #[must_use]
    pub fn projection(&self) -> LegacyProjection<'a> {
        LegacyProjection {
            gdpr: self.downgrade_gdpr,
            consent: self.active_tcf,
            usp: self.active_usp,
            clear_legacy_usp: self.clear_legacy_usp,
        }
    }
    #[must_use]
    pub fn envelope_state(&self) -> EnvelopeState {
        self.envelope
    }

    #[must_use]
    pub fn wire_signal(&self) -> Signal {
        self.wire_signal
    }

    #[must_use]
    pub fn tcf(&self) -> SelectedConsent<'a> {
        self.tcf
    }

    #[must_use]
    pub fn usp(&self) -> Option<Usp> {
        self.usp.ok().flatten()
    }

    /// Whether the selected nonempty USP is malformed, excluding unselected inputs.
    /// This carries no raw consent value and never fabricates a valid opt-out.
    #[must_use]
    pub fn usp_is_malformed(&self) -> bool {
        self.usp.is_err()
    }

    /// Active section 2, including empty/opaque values, for legacy projection.
    #[must_use]
    pub fn active_tcf(&self) -> Option<&'a str> {
        self.active_tcf
    }

    /// Only a valid nonempty active section 6 may be projected into legacy USP.
    #[must_use]
    pub fn active_usp(&self) -> Option<Usp> {
        self.active_usp
    }

    /// A present-empty SID projects zero, although scope resolution treats it as absent.
    #[must_use]
    pub fn downgrade_gdpr(&self) -> Option<i32> {
        self.downgrade_gdpr
    }

    #[must_use]
    pub fn clear_legacy_usp(&self) -> bool {
        self.clear_legacy_usp
    }

    /// Emit bounded warnings once, in source order, after optional TCF authorization.
    ///
    /// The authorization path supplies at most one TCF syntax warning and one
    /// authenticity/GVL warning. Inactive TCF authorization supplies neither.
    #[must_use]
    pub fn into_warnings(
        self,
        tcf_validation: Option<NonFatalWarning>,
        authenticity: Option<NonFatalWarning>,
    ) -> Vec<NonFatalWarning> {
        let mut warnings = self.wire_warnings;
        if self.envelope == EnvelopeState::Malformed {
            warnings.push(warning(
                "regs.gpp",
                "regs.gpp is not a valid supported GPP envelope",
            ));
        }
        if self.clear_legacy_usp {
            warnings.push(warning(
                "regs.us_privacy",
                "regs.us_privacy is not a valid USP value; clearing legacy value",
            ));
        }
        if self.tcf_conflict {
            warnings.push(warning(
                "regs.gpp",
                "regs.gpp section 2 and user.consent conflict; using section 2",
            ));
        }
        if self.usp_conflict {
            warnings.push(warning(
                "regs.gpp",
                "regs.gpp section 6 and regs.us_privacy conflict; using section 6",
            ));
        }
        warnings.extend(tcf_validation);
        if self.selected_usp_invalid {
            warnings.push(warning(
                "regs.gpp",
                "regs.gpp section 6 is not a valid USP value; no usable USP",
            ));
        }
        warnings.extend(authenticity);
        warnings
    }
}

fn warning(scope: &str, message: &str) -> NonFatalWarning {
    NonFatalWarning {
        code: WarningCode::InvalidPrivacyConsent,
        message: message.to_owned(),
        scope: Some(scope.to_owned()),
    }
}

/// Normalize outer framing, source precedence, and USP without mutating the request.
///
/// TCF remains raw until the GDPR resolver establishes that authorization is active.
/// Unknown sections remain opaque. Call before any permissive policy short-circuit.
///
/// # Errors
/// Returns [`PrivacyError::InvalidRegsGdpr`] for an invalid applicable wire signal
/// or [`PrivacyError::GppLimitExceeded`] for terminal envelope resource limits.
pub fn normalize(
    req: &BidRequest,
    limits: gpp::Limits,
) -> Result<NormalizedSources<'_>, PrivacyError> {
    let regs = req.regs.as_ref();
    let sid = regs.and_then(|value| value.gpp_sid.as_deref());
    let (wire_signal, wire_warnings) = resolve_wire_signal(&WireSignalInput {
        regs_gdpr: regs.and_then(|value| value.gdpr),
        gpp_sid: sid,
    })?;
    let gpp_value = regs.and_then(|value| value.gpp.as_deref());
    let (envelope_state, envelope) = match gpp_value {
        None | Some("") => (EnvelopeState::Absent, None),
        Some(raw) => match gpp::parse(raw, limits) {
            Ok(envelope) => (EnvelopeState::Valid, Some(envelope)),
            Err(gpp::ParseError::Malformed) => (EnvelopeState::Malformed, None),
            Err(gpp::ParseError::LimitExceeded) => return Err(PrivacyError::GppLimitExceeded),
        },
    };
    let active_tcf = sid
        .filter(|ids| ids.contains(&2_i32))
        .and_then(|_ids| envelope.as_ref()?.section(2));
    let active_usp_raw = sid
        .filter(|ids| ids.contains(&6_i32))
        .and_then(|_ids| envelope.as_ref()?.section(6))
        .filter(|raw| !raw.is_empty());
    let legacy_tcf = req.user.as_ref().and_then(|value| value.consent.as_deref());
    let legacy_usp = regs.and_then(|value| value.us_privacy.as_deref());
    let parsed_legacy_usp = Usp::parse(legacy_usp);
    let parsed_active_usp = Usp::parse(active_usp_raw);
    let tcf = active_tcf.map_or(
        SelectedConsent {
            raw: legacy_tcf,
            source: ConsentSource::Legacy,
        },
        |raw| SelectedConsent {
            raw: Some(raw),
            source: ConsentSource::Gpp,
        },
    );
    let usp = if active_usp_raw.is_some() {
        parsed_active_usp
    } else {
        parsed_legacy_usp
    };
    Ok(NormalizedSources {
        envelope: envelope_state,
        wire_signal,
        tcf,
        usp,
        active_tcf,
        active_usp: parsed_active_usp.ok().flatten(),
        downgrade_gdpr: sid.map(|ids| i32::from(ids.contains(&2_i32))),
        clear_legacy_usp: parsed_legacy_usp.is_err(),
        wire_warnings,
        tcf_conflict: conflicts(active_tcf, legacy_tcf),
        usp_conflict: conflicts(active_usp_raw, legacy_usp),
        selected_usp_invalid: parsed_active_usp.is_err(),
    })
}

fn conflicts(active: Option<&str>, legacy: Option<&str>) -> bool {
    active.zip(legacy).is_some_and(|(selected, previous)| {
        !selected.is_empty() && !previous.is_empty() && selected != previous
    })
}

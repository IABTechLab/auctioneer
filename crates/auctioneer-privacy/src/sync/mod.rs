//! Bounded sync-endpoint privacy source normalization.
//!
//! Permission decisions are separate from extraction. In particular, a
//! nonempty malformed TCF value is retained for the endpoint permission stage.

mod input;
mod permission;

use std::fmt;

use permission::ConsentState;

use auctioneer_config_data::types::{
    AccountConfig, BidderInfo, HostConfig, SyncerConfig, TriState,
};
use time::OffsetDateTime;

use crate::{
    activity::{Activity, Component},
    gcl::Gcl,
    gpp,
    gvl::GvlStore,
    signal::Signal,
    tcf::MAX_TOTAL_INPUT_LEN,
    usp::Usp,
};

pub use input::{GdprSignalIn, GppSid, SyncPrivacyError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncEndpoint {
    CookieSync,
    SetUid,
}

/// A resolved bidder identity and its typed sync configuration.
pub struct SyncerLookup<'a> {
    pub requested_name: &'a str,
    pub normalized_name: &'a str,
    pub base_name: &'a str,
    pub bidder: &'a BidderInfo,
    pub syncer: &'a SyncerConfig,
    pub syncer_key: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDecision {
    Allowed,
    MalformedTcf,
    ResourceLimit,
    BlockedByActivity,
    BlockedByDisabledUsersync,
    BlockedByPrivacy,
    BlockedByRegulationScope,
}

// Kept as non-allocating type aliases for the existing crate-root export.
// Normalization does not manufacture callbacks or make permission decisions.
pub type SyncCallback<'a> = fn(&SyncerLookup<'a>) -> SyncDecision;
pub type ActivityCallback<'a> = fn(Activity, Component<'a>) -> bool;

/// Wire inputs have already passed the endpoint's body/query framing limits.
/// The normalizer applies privacy-specific structural limits as well.
#[derive(Clone, Copy)]
pub struct SyncPrivacyInput<'a> {
    pub gdpr: GdprSignalIn<'a>,
    pub consent: Option<&'a str>,
    pub us_privacy: Option<&'a str>,
    pub gpp: Option<&'a str>,
    pub gpp_sid: Option<&'a str>,
    pub account: &'a AccountConfig,
    pub host: &'a HostConfig,
    pub endpoint: SyncEndpoint,
    pub gvl: &'a GvlStore,
    pub gcl: &'a Gcl,
    pub now: OffsetDateTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncConsentSource {
    Legacy,
    GppSection2,
}

/// Permission-stage TCF failure. The original consent is never formatted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SyncTcfError {
    #[error("malformed TCF consent")]
    Malformed,
    #[error("TCF resource limit exceeded")]
    ResourceLimit,
}

/// Normalized extraction result with cached permission state.
/// Debug intentionally never exposes wire values.
#[expect(
    clippy::partial_pub_fields,
    reason = "decision outputs are public; raw consent and cached permission state stay private"
)]
pub struct SyncPrivacyContext<'a> {
    pub parsed_request_signal: Signal,
    pub normalized_request_signal: Signal,
    pub gdpr_permissions_enabled: bool,
    sid: GppSid,
    raw_gpp_sid: Option<&'a str>,
    validated_gpp: Option<&'a str>,
    selected_consent: Option<&'a str>,
    consent_source: SyncConsentSource,
    selected_usp: Option<&'a str>,
    pub account: &'a AccountConfig,
    pub host: &'a HostConfig,
    pub gvl: &'a GvlStore,
    pub gcl: &'a Gcl,
    pub now: OffsetDateTime,
    pub endpoint: SyncEndpoint,
    consent_state: ConsentState,
}

impl fmt::Debug for SyncPrivacyContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncPrivacyContext")
            .field("parsed_request_signal", &self.parsed_request_signal)
            .field("normalized_request_signal", &self.normalized_request_signal)
            .field("gdpr_permissions_enabled", &self.gdpr_permissions_enabled)
            .finish_non_exhaustive()
    }
}

impl<'a> SyncPrivacyContext<'a> {
    #[must_use]
    pub fn sid(&self) -> &GppSid {
        &self.sid
    }

    #[must_use]
    pub fn raw_gpp_sid(&self) -> Option<&'a str> {
        self.raw_gpp_sid
    }

    #[must_use]
    pub fn validated_gpp(&self) -> Option<&'a str> {
        self.validated_gpp
    }

    /// Raw selected consent is intentionally not decoded during extraction.
    #[must_use]
    pub fn selected_consent(&self) -> Option<&'a str> {
        self.selected_consent
    }

    #[must_use]
    pub fn consent_source(&self) -> SyncConsentSource {
        self.consent_source
    }

    /// Only a validated USP v1 value is returned.
    #[must_use]
    pub fn selected_usp(&self) -> Option<&'a str> {
        self.selected_usp
    }
}

/// Normalize sync wire sources without running TCF permission checks.
///
/// # Errors
/// Invalid or over-limit SID/GPP, unresolved legacy GDPR, and required-empty
/// consent produce closed extraction errors. Malformed nonempty TCF does not.
pub fn resolve_sync_privacy(
    input: SyncPrivacyInput<'_>,
) -> Result<SyncPrivacyContext<'_>, SyncPrivacyError> {
    if input.gdpr.exceeds_limit()
        || input
            .consent
            .is_some_and(|raw| raw.len() > MAX_TOTAL_INPUT_LEN)
        || input.us_privacy.is_some_and(|raw| raw.len() > 32 * 1024)
    {
        return Err(SyncPrivacyError::InputLimitExceeded);
    }
    let sid = GppSid::parse(input.gpp_sid)?;
    let envelope = match input.gpp {
        None | Some("") => None,
        Some(raw) => Some(gpp::parse(raw, gpp::Limits::default()).map_err(
            |error| match error {
                gpp::ParseError::Malformed => SyncPrivacyError::MalformedGpp,
                gpp::ParseError::LimitExceeded => SyncPrivacyError::GppLimitExceeded,
            },
        )?),
    };
    let parsed_request_signal = if sid.is_specified() {
        if sid.contains(2) {
            Signal::Yes
        } else {
            Signal::No
        }
    } else {
        input.gdpr.parse(input.endpoint)?
    };
    let normalized_request_signal = match parsed_request_signal {
        Signal::Ambiguous if input.host.gdpr.default_value == "0" => Signal::No,
        Signal::Ambiguous => Signal::Yes,
        definite @ (Signal::Yes | Signal::No) => definite,
    };

    let active_tcf = if sid.contains(2) {
        envelope.as_ref().and_then(|value| value.section(2))
    } else {
        None
    };
    let (selected_consent, consent_source) = match active_tcf {
        Some(raw) => (Some(raw), SyncConsentSource::GppSection2),
        None => (input.consent, SyncConsentSource::Legacy),
    };
    if selected_consent.is_some_and(|raw| raw.len() > MAX_TOTAL_INPUT_LEN) {
        return Err(SyncPrivacyError::InputLimitExceeded);
    }
    let consent_empty = selected_consent.is_none_or(str::is_empty);
    if consent_empty
        && (parsed_request_signal == Signal::Yes
            || (input.endpoint == SyncEndpoint::CookieSync
                && normalized_request_signal == Signal::Yes))
    {
        return Err(SyncPrivacyError::MissingRequiredConsent);
    }

    let active_usp = if sid.contains(6) {
        envelope.as_ref().and_then(|value| value.section(6))
    } else {
        None
    }
    .filter(|raw| !raw.is_empty());
    let selected_usp_raw = active_usp.or(input.us_privacy);
    if selected_usp_raw.is_some_and(|raw| raw.len() > 32 * 1024) {
        return Err(SyncPrivacyError::InputLimitExceeded);
    }
    let selected_usp = if Usp::parse(selected_usp_raw).ok().flatten().is_some() {
        selected_usp_raw
    } else {
        None
    };

    let gdpr_permissions_enabled = input.host.gdpr.enabled
        && match input.account.gdpr.channel_enabled.web {
            TriState::Enabled => true,
            TriState::Disabled => false,
            TriState::Unset => match input.account.gdpr.enabled {
                TriState::Enabled => true,
                TriState::Disabled => false,
                TriState::Unset => input.host.gdpr.tcf2.enabled,
            },
        };
    Ok(SyncPrivacyContext {
        parsed_request_signal,
        normalized_request_signal,
        gdpr_permissions_enabled,
        sid,
        raw_gpp_sid: input.gpp_sid,
        validated_gpp: envelope.as_ref().map(gpp::Envelope::raw),
        selected_consent,
        consent_source,
        selected_usp,
        account: input.account,
        host: input.host,
        gvl: input.gvl,
        gcl: input.gcl,
        now: input.now,
        endpoint: input.endpoint,
        consent_state: ConsentState::Unparsed,
    })
}

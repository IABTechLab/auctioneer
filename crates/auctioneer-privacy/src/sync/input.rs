//! Closed sync wire coercion and bounded SID parsing.

use std::fmt;

use crate::signal::Signal;

use super::SyncEndpoint;

const MAX_SID_ENTRIES: usize = 64;
const MAX_SID_BYTES: usize = 32 * 1024;
const MAX_QUERY_GDPR_BYTES: usize = 32 * 1024;

/// The endpoint's structural GDPR wire value, without retaining invalid data.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GdprSignalIn<'a> {
    CookieAbsent,
    CookieNull,
    CookieNumber(i64),
    CookieOther,
    SetUidQuery(Option<&'a str>),
}

impl fmt::Debug for GdprSignalIn<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CookieAbsent => "CookieAbsent",
            Self::CookieNull => "CookieNull",
            Self::CookieNumber(_) => "CookieNumber",
            Self::CookieOther => "CookieOther",
            Self::SetUidQuery(_) => "SetUidQuery",
        })
    }
}

impl GdprSignalIn<'_> {
    pub(super) fn exceeds_limit(&self) -> bool {
        matches!(self, Self::SetUidQuery(Some(raw)) if raw.len() > MAX_QUERY_GDPR_BYTES)
    }

    pub(super) fn parse(self, endpoint: SyncEndpoint) -> Result<Signal, SyncPrivacyError> {
        match (endpoint, self) {
            (SyncEndpoint::CookieSync, Self::CookieAbsent | Self::CookieNull)
            | (SyncEndpoint::SetUid, Self::SetUidQuery(None | Some(""))) => Ok(Signal::Ambiguous),
            (SyncEndpoint::CookieSync, Self::CookieNumber(0))
            | (SyncEndpoint::SetUid, Self::SetUidQuery(Some("0"))) => Ok(Signal::No),
            (SyncEndpoint::CookieSync, Self::CookieNumber(1))
            | (SyncEndpoint::SetUid, Self::SetUidQuery(Some("1"))) => Ok(Signal::Yes),
            _ => Err(SyncPrivacyError::MalformedGdpr),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SyncPrivacyError {
    #[error("invalid GDPR signal")]
    MalformedGdpr,
    #[error("malformed GPP envelope")]
    MalformedGpp,
    #[error("GPP envelope limit exceeded")]
    GppLimitExceeded,
    #[error("invalid GPP section IDs")]
    MalformedGppSid,
    #[error("GPP section ID limit exceeded")]
    GppSidLimitExceeded,
    #[error("required GDPR consent is empty")]
    MissingRequiredConsent,
    #[error("sync privacy input limit exceeded")]
    InputLimitExceeded,
}

/// Numeric applicability, separate from the exact decoded wire string.
#[derive(Clone)]
pub struct GppSid {
    ids: [u16; MAX_SID_ENTRIES],
    len: usize,
    no_applicable_section: bool,
}

impl fmt::Debug for GppSid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GppSid")
            .field("entries", &self.len)
            .field("no_applicable_section", &self.no_applicable_section)
            .finish_non_exhaustive()
    }
}

impl GppSid {
    pub(super) fn parse(raw: Option<&str>) -> Result<Self, SyncPrivacyError> {
        let mut result = Self {
            ids: [0; MAX_SID_ENTRIES],
            len: 0,
            no_applicable_section: false,
        };
        let Some(raw_sid) = raw.filter(|value| !value.is_empty()) else {
            return Ok(result);
        };
        if raw_sid.len() > MAX_SID_BYTES {
            return Err(SyncPrivacyError::GppSidLimitExceeded);
        }
        if raw_sid == "-1" {
            result.no_applicable_section = true;
            return Ok(result);
        }
        for component in raw_sid.split(',') {
            if result.len == MAX_SID_ENTRIES {
                return Err(SyncPrivacyError::GppSidLimitExceeded);
            }
            let mut value = 0_u32;
            if component.is_empty() {
                return Err(SyncPrivacyError::MalformedGppSid);
            }
            for byte in component.bytes() {
                if !byte.is_ascii_digit() {
                    return Err(SyncPrivacyError::MalformedGppSid);
                }
                let digit = byte
                    .checked_sub(b'0')
                    .ok_or(SyncPrivacyError::MalformedGppSid)?;
                value = value
                    .checked_mul(10)
                    .and_then(|sum| sum.checked_add(u32::from(digit)))
                    .ok_or(SyncPrivacyError::MalformedGppSid)?;
                if value > u32::from(u16::MAX) {
                    return Err(SyncPrivacyError::MalformedGppSid);
                }
            }
            if value == 0 {
                return Err(SyncPrivacyError::MalformedGppSid);
            }
            let slot = result
                .ids
                .get_mut(result.len)
                .ok_or(SyncPrivacyError::GppSidLimitExceeded)?;
            *slot = u16::try_from(value).map_err(|_overflow| SyncPrivacyError::MalformedGppSid)?;
            result.len = result
                .len
                .checked_add(1)
                .ok_or(SyncPrivacyError::GppSidLimitExceeded)?;
        }
        Ok(result)
    }

    #[must_use]
    pub fn contains(&self, id: u16) -> bool {
        self.ids
            .get(..self.len)
            .is_some_and(|ids| ids.contains(&id))
    }

    #[must_use]
    pub fn is_no_applicable_section(&self) -> bool {
        self.no_applicable_section
    }

    #[must_use]
    pub fn is_specified(&self) -> bool {
        self.no_applicable_section || self.len != 0
    }
}

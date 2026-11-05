//! Bounded protocol facts associated with one accepted outbound call.

use auctioneer_config_data::types::MediaType;
use std::{fmt, mem};

/// Maximum protocol-header payload stored alongside a call, never on the wire.
pub const MAX_CALL_CORRELATION_HEADER_BYTES: usize = 32;

/// A fixed construction refusal; no request data appears in diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorrelationError {
    InvalidMedia,
    HeaderOverCap,
    DuplicateHeader,
    UnknownOrdinal,
    DuplicateOrdinal,
    DuplicateCoverage,
}

/// Media carried by a particular call, independently of another call's coverage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CorrelationMedia {
    Banner = 1,
    Video = 2,
    Audio = 3,
    Native = 4,
}

impl TryFrom<u8> for CorrelationMedia {
    type Error = CorrelationError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Banner),
            2 => Ok(Self::Video),
            3 => Ok(Self::Audio),
            4 => Ok(Self::Native),
            _ => Err(CorrelationError::InvalidMedia),
        }
    }
}

impl From<CorrelationMedia> for MediaType {
    fn from(value: CorrelationMedia) -> Self {
        match value {
            CorrelationMedia::Banner => Self::Banner,
            CorrelationMedia::Video => Self::Video,
            CorrelationMedia::Audio => Self::Audio,
            CorrelationMedia::Native => Self::Native,
        }
    }
}

/// Small call-local row. Its protocol token is not part of Debug output.
#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(C)]
pub struct ImpCorrelation {
    token: u32,
    media: CorrelationMedia,
}

impl ImpCorrelation {
    #[must_use]
    pub const fn media(self) -> CorrelationMedia {
        self.media
    }

    #[must_use]
    pub const fn token(self) -> u32 {
        self.token
    }
}

impl fmt::Debug for ImpCorrelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImpCorrelation")
            .field("media", &self.media)
            .finish_non_exhaustive()
    }
}

#[derive(Default, Eq, PartialEq)]
pub(crate) struct CallCorrelation {
    header: [u8; MAX_CALL_CORRELATION_HEADER_BYTES],
    header_len: Option<u8>,
    rows: Vec<Option<ImpCorrelation>>,
}

impl CallCorrelation {
    pub(crate) fn set_header(&mut self, value: &[u8]) -> Result<(), CorrelationError> {
        if self.header_len.is_some() {
            return Err(CorrelationError::DuplicateHeader);
        }
        if value.len() > MAX_CALL_CORRELATION_HEADER_BYTES {
            return Err(CorrelationError::HeaderOverCap);
        }
        let length = u8::try_from(value.len()).map_err(|_error| CorrelationError::HeaderOverCap)?;
        self.header
            .get_mut(..value.len())
            .ok_or(CorrelationError::HeaderOverCap)?
            .copy_from_slice(value);
        self.header_len = Some(length);
        Ok(())
    }

    pub(crate) fn header(&self) -> Option<&[u8]> {
        let length = self.header_len?;
        self.header.get(..usize::from(length))
    }

    pub(crate) fn set_row(
        &mut self,
        ordinal: u16,
        covered: u16,
        maximum: u16,
        media: CorrelationMedia,
        token: u32,
    ) -> Result<(), CorrelationError> {
        if ordinal >= covered || covered > maximum {
            return Err(CorrelationError::UnknownOrdinal);
        }
        if self.rows.is_empty() {
            self.rows = Vec::with_capacity(usize::from(maximum));
            self.rows.resize(usize::from(covered), None);
        }
        if self.rows.len() != usize::from(covered) {
            return Err(CorrelationError::UnknownOrdinal);
        }
        let row = self
            .rows
            .get_mut(usize::from(ordinal))
            .ok_or(CorrelationError::UnknownOrdinal)?;
        if row.is_some() {
            return Err(CorrelationError::DuplicateOrdinal);
        }
        *row = Some(ImpCorrelation { token, media });
        Ok(())
    }

    pub(crate) fn extend_coverage(&mut self) -> Result<(), CorrelationError> {
        if !self.rows.is_empty() {
            if self.rows.len() == self.rows.capacity() {
                return Err(CorrelationError::UnknownOrdinal);
            }
            self.rows.push(None);
        }
        Ok(())
    }

    pub(crate) fn row(&self, ordinal: u16) -> Option<ImpCorrelation> {
        self.rows.get(usize::from(ordinal)).copied().flatten()
    }
}

const _: () = {
    assert!(
        mem::size_of::<Option<ImpCorrelation>>() == 8,
        "the metadata charge funds eight bytes per correlation row"
    );
    assert!(
        mem::size_of::<CallCorrelation>() <= 64,
        "the metadata charge funds at most 64 inline correlation bytes"
    );
};

#[cfg(test)]
mod tests {
    use super::{CallCorrelation, CorrelationError, CorrelationMedia};
    use auctioneer_config_data::types::MediaType;

    #[test]
    fn media_codes_are_exact_and_invalid_values_never_become_a_hint() {
        for (code, media, standard) in [
            (1, CorrelationMedia::Banner, MediaType::Banner),
            (2, CorrelationMedia::Video, MediaType::Video),
            (3, CorrelationMedia::Audio, MediaType::Audio),
            (4, CorrelationMedia::Native, MediaType::Native),
        ] {
            assert_eq!(CorrelationMedia::try_from(code), Ok(media));
            assert_eq!(MediaType::from(media), standard);
        }
        for code in [0, 5, 255] {
            assert_eq!(
                CorrelationMedia::try_from(code),
                Err(CorrelationError::InvalidMedia)
            );
        }
    }

    #[test]
    fn header_accepts_exact_capacity_and_preserves_the_incumbent_on_failure() {
        let mut context = CallCorrelation::default();
        assert_eq!(context.header(), None);
        assert_eq!(
            context.set_header(&[7; 33]),
            Err(CorrelationError::HeaderOverCap)
        );
        assert_eq!(context.header(), None);
        assert_eq!(context.set_header(&[9; 32]), Ok(()));
        assert_eq!(context.header(), Some([9_u8; 32].as_slice()));
        assert_eq!(
            context.set_header(&[1]),
            Err(CorrelationError::DuplicateHeader)
        );
        assert_eq!(context.header(), Some([9_u8; 32].as_slice()));
        let mut empty = CallCorrelation::default();
        assert_eq!(empty.set_header(&[]), Ok(()));
        assert_eq!(empty.header(), Some([].as_slice()));
        assert_eq!(
            empty.set_header(&[]),
            Err(CorrelationError::DuplicateHeader)
        );
    }

    #[test]
    fn rows_are_call_local_bounded_and_cannot_overwrite_an_ordinal() {
        let mut first = CallCorrelation::default();
        let mut second = CallCorrelation::default();
        assert_eq!(
            first.set_row(2, 2, 3, CorrelationMedia::Banner, 11),
            Err(CorrelationError::UnknownOrdinal)
        );
        assert_eq!(
            first.set_row(0, 2, 1, CorrelationMedia::Banner, 11),
            Err(CorrelationError::UnknownOrdinal)
        );
        assert_eq!(first.rows.capacity(), 0);
        assert_eq!(first.set_row(0, 2, 3, CorrelationMedia::Banner, 11), Ok(()));
        assert_eq!(second.set_row(0, 1, 1, CorrelationMedia::Video, 22), Ok(()));
        assert_eq!(first.rows.capacity(), 3);
        assert_eq!(first.row(0).unwrap().media(), CorrelationMedia::Banner);
        assert_eq!(first.row(0).unwrap().token(), 11);
        assert_eq!(second.row(0).unwrap().media(), CorrelationMedia::Video);
        assert_eq!(second.row(0).unwrap().token(), 22);
        assert_eq!(first.row(1), None);
        assert_eq!(
            first.set_row(0, 2, 3, CorrelationMedia::Video, 99),
            Err(CorrelationError::DuplicateOrdinal)
        );
        assert_eq!(first.row(0).unwrap().token(), 11);
        first.extend_coverage().unwrap();
        assert_eq!(first.rows.len(), 3);
        assert_eq!(first.rows.capacity(), 3);
        assert_eq!(first.set_row(2, 3, 3, CorrelationMedia::Native, 33), Ok(()));
        assert_eq!(first.row(2).unwrap().media(), CorrelationMedia::Native);
        assert_eq!(first.row(2).unwrap().token(), 33);
        assert_eq!(first.row(3), None);
        assert_eq!(
            first.extend_coverage(),
            Err(CorrelationError::UnknownOrdinal)
        );
        assert_eq!(first.rows.capacity(), 3);
    }

    #[test]
    fn row_debug_does_not_disclose_the_protocol_token() {
        let mut context = CallCorrelation::default();
        context
            .set_row(0, 1, 1, CorrelationMedia::Banner, 876_543_210)
            .unwrap();
        assert_eq!(
            format!("{:?}", context.row(0).unwrap()),
            "ImpCorrelation { media: Banner, .. }"
        );
    }
}

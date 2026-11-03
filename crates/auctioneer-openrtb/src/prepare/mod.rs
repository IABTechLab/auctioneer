//! `prepare(req, headers, limits)` — impression cap + implicit-field fill
//! + 2.6 privacy checks.
//!
//! **No consent-string parsing here.**
//! Consent warnings come from privacy resolution in `auctioneer-privacy`.
//! `prepare()` returns a `NormalizedBidRequest` with an empty warnings vector;
//! the exchange surfaces downstream privacy warnings separately.

mod checks;
mod implicit_fill;

use crate::normalized::NormalizedBidRequest;
use crate::request_headers::RequestHeaders;
use crate::types::bid_request::BidRequest;
use crate::validate::ValidationError;

/// Caps `prepare()` accepts before doing any per-imp work. Checked as
/// step 0, ahead of implicit fill and validation, so an oversized
/// `imp[]` is rejected without allocating anything scoped to an
/// individual imp.
#[derive(Clone, Copy, Debug)]
pub struct PrepareLimits {
    pub max_impressions: u32,
    /// Combined cap on the advisory headers implicit fill COPIES into
    /// the typed request — `User-Agent` into `device.ua`, `Referer`
    /// into `site.page`.
    ///
    /// The budget charges a fixed allowance for this pair and
    /// multiplies it by `1 + bidders`, because each per-bidder clone
    /// carries both strings. Nothing bounded either value, so one
    /// oversize `User-Agent` broke the reservation on every clone at
    /// once. Cut rather than rejected: these are advisory enrichment,
    /// and a request is not malformed for carrying a long header.
    pub max_copied_header_bytes: u32,
}

impl Default for PrepareLimits {
    /// The shipped figures. `max_copied_header_bytes` matches the
    /// envelope's own `max_copied_header_bytes`, which is what the
    /// budget reserves against.
    fn default() -> Self {
        Self {
            max_impressions: u32::from(u16::MAX),
            max_copied_header_bytes: 4_096,
        }
    }
}

/// Single reusable preparation gate. Impression-count cap + implicit
/// fill + validate + 2.6 privacy gate + channel-empty-name reject.
///
/// Programmatic callers must admit arbitrary-depth inputs while borrowed before
/// transferring ownership here. This gate does not protect the caller's earlier
/// allocation or take over safe cleanup of an unadmitted recursive tree.
///
/// # Errors
/// Returns [`ValidationError`] when `imp[]` exceeds `limits.max_impressions`,
/// when structural validation fails, or when any 2.6 privacy gate rejects
/// the request.
#[inline]
pub fn prepare(
    mut req: BidRequest,
    headers: &RequestHeaders<'_>,
    limits: &PrepareLimits,
) -> Result<NormalizedBidRequest, ValidationError> {
    let got = u32::try_from(req.imp.len()).unwrap_or(u32::MAX);
    if got > limits.max_impressions {
        return Err(ValidationError::TooManyImpressions {
            got,
            max: limits.max_impressions,
        });
    }
    implicit_fill::run(&mut req, headers, limits.max_copied_header_bytes);
    req.validate()?;
    checks::reject_legacy_25_privacy(&req)?;
    checks::reject_empty_channel_name(&req)?;
    Ok(NormalizedBidRequest {
        inner: req,
        warnings: Vec::new(),
    })
}

//! Outbound request metadata built into a fixed byte pool.
//!
//! A bidder never builds a free-standing `String` for the method, the
//! URL, a header or an imp id. Every component is appended into one
//! pre-allocated byte pool through a bounded append that errors before
//! it would cross capacity, so the pool never reallocates and the
//! allocation the isolate budget charges is the allocation that happens.
//!
//! The finished [`OutboundRequestMeta`] keeps the pool plus typed
//! `(start, len)` offsets; the accessors hand out `&str` slices borrowed
//! from `&self`.

use crate::capped_writer::BodyWriteError;
use crate::correlation::{CallCorrelation, CorrelationError, CorrelationMedia, ImpCorrelation};
use crate::finalize::FinalizeError;
use std::mem;
use std::ops::Range;
use std::{fmt, str};

/// Total byte-pool capacity of one outbound request meta.
pub const META_BYTES_CAPACITY: usize = 3_584;
/// The prefix of the byte pool a bidder may append into. The balance
/// is the finalize reserve, which only the transport finalizer writes.
pub const META_BIDDER_REGION_BYTES: usize = 3_328;
/// Tail of the byte pool reserved for the finalizer's own appends
/// (`Content-Encoding`, `Sec-GPC`).
pub const META_FINALIZE_RESERVE_BYTES: usize = 256;
/// How many headers a bidder may push before `header()` refuses.
pub const MAX_BIDDER_HEADERS: u16 = 32;
/// Header slots the exchange keeps for its own finalize-time appends.
pub const RESERVED_EXCHANGE_HEADER_SLOTS: u16 = 4;

/// The two regions partition the pool exactly — no slack, no overlap.
const _: () = assert!(
    META_BIDDER_REGION_BYTES + META_FINALIZE_RESERVE_BYTES == META_BYTES_CAPACITY,
    "the bidder region and the finalize reserve must partition the byte pool"
);

/// The same constants in the widths the budget and `SinkAbort` speak.
/// `usize`/`u32` conversions are not const-callable on stable, so each
/// width is written once and pinned to its `usize` twin below.
const META_BYTES_CAPACITY_U32: u32 = 3_584;
const META_BIDDER_REGION_BYTES_U64: u64 = 3_328;
/// Header entries the offset table charges: every bidder slot plus
/// every reserved exchange slot.
const HEADER_ENTRIES: u32 = 36;

const _: () = assert!(
    META_BYTES_CAPACITY == 3_584
        && META_BIDDER_REGION_BYTES == 3_328
        && MAX_BIDDER_HEADERS + RESERVED_EXCHANGE_HEADER_SLOTS == 36,
    "a width-specific layout constant drifted from its usize twin"
);

/// Bytes of offset table backing one meta at `max_impressions_per_auction`.
///
/// One 4-byte `(start, len)` each for the method and the URL, then
/// `MAX_BIDDER_HEADERS + RESERVED_EXCHANGE_HEADER_SLOTS` eight-byte
/// header entries (`(n_start, n_len, v_start, v_len)`), then one 4-byte
/// entry per imp id — rounded up to 64.
const fn offset_table_bytes(max_impressions_per_auction: u32) -> u32 {
    const METHOD_AND_URL_ENTRIES: u32 = 2;
    const OFFSET_PAIR_BYTES: u32 = 4;
    const HEADER_ENTRY_BYTES: u32 = 8;
    const ALIGN: u32 = 64;

    let raw = METHOD_AND_URL_ENTRIES
        .saturating_mul(OFFSET_PAIR_BYTES)
        .saturating_add(HEADER_ENTRIES.saturating_mul(HEADER_ENTRY_BYTES))
        .saturating_add(max_impressions_per_auction.saturating_mul(OFFSET_PAIR_BYTES));
    raw.div_ceil(ALIGN).saturating_mul(ALIGN)
}

/// Struct overhead charged per meta: the three `Vec` handles the meta
/// carries plus the retained call reservation, at 64-bit widths. A
/// 32-bit target's handles are narrower, so the charge over-reserves
/// there rather than under-reserving on the wider one.
const META_STRUCT_OVERHEAD_BYTES: u32 = 96;

/// Peak bytes one outbound request meta costs at a given impression limit.
///
/// This is the single bidder-derived input to the isolate budget
/// validator, which takes the RESULT as a scalar — no layout constant
/// crosses the crate boundary and nothing is re-derived elsewhere.
/// Correlation adds at most 64 inline bytes and one eight-byte optional
/// row per covered-impression slot, separately from the offset table.
#[must_use]
pub const fn outbound_meta_bytes(max_impressions_per_auction: u16) -> u32 {
    #[expect(
        clippy::as_conversions,
        reason = "u16 -> u32 is lossless widening; `u32::from` is not \
                  callable in a const fn on stable"
    )]
    let impressions = max_impressions_per_auction as u32;
    META_BYTES_CAPACITY_U32
        .saturating_add(offset_table_bytes(impressions))
        .saturating_add(META_STRUCT_OVERHEAD_BYTES)
        .saturating_add(64)
        .saturating_add(impressions.saturating_mul(8))
}

/// Header names a bidder may never push. The transport owns framing
/// and routing; these fields belong to the connection layer (RFC 9110
/// §7.6.1) or to the exchange's own finalize pass.
const RESERVED_HEADER_NAMES: &[&str] = &[
    "content-encoding",
    "content-length",
    "sec-gpc",
    "host",
    ":authority",
    "transfer-encoding",
    "te",
    "connection",
    "trailer",
    "upgrade",
    "proxy-connection",
    "keep-alive",
];

/// Which single-valued meta field a [`SinkAbort::DuplicateMeta`] refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetaField {
    Method,
    Url,
}

/// Why a bounded outbound-request build was abandoned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SinkAbort {
    /// A malformed or duplicate call-local correlation component.
    Correlation(CorrelationError),
    /// An append would have crossed the bidder region of the byte pool.
    MetaOverCap { limit: u64 },
    /// A header name the transport or the exchange owns.
    ReservedHeader { name: String },
    /// The 33rd bidder header.
    TooManyHeaders { limit: u16 },
    /// The offset table has no room for another imp id.
    TooManyImpIds { limit: u16 },
    /// A second `set_method` or `set_url`.
    DuplicateMeta { field: MetaField },
    /// A body write failed.
    Body(BodyWriteError),
}

impl From<CorrelationError> for SinkAbort {
    fn from(error: CorrelationError) -> Self {
        Self::Correlation(error)
    }
}

impl From<BodyWriteError> for SinkAbort {
    fn from(err: BodyWriteError) -> Self {
        Self::Body(err)
    }
}

/// A built outbound request's metadata: one byte pool plus typed offsets.
///
/// `Debug` reports only counts. Bidder-built URLs and headers can contain
/// credentials and identities; formatting must not expose the raw pool.
#[derive(PartialEq, Eq)]
pub struct OutboundRequestMeta {
    /// Flat byte pool: the concatenated method, URL, header names and
    /// values, and imp ids, indexed by the `(start, len)` tuples below.
    /// Allocated once at `META_BYTES_CAPACITY` and never reallocated.
    bytes: Vec<u8>,
    method: (u16, u16),
    url: (u16, u16),
    headers: Vec<(u16, u16, u16, u16)>,
    imp_ids: Vec<(u16, u16)>,
    correlation: CallCorrelation,
    finalized: bool,
}

const _: () = assert!(
    mem::size_of::<OutboundRequestMeta>() <= 160,
    "metadata exceeds its 96-byte base plus 64-byte correlation charge"
);

impl fmt::Debug for OutboundRequestMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutboundRequestMeta")
            .field("header_count", &self.header_count())
            .field("imp_id_count", &self.imp_id_count())
            .finish_non_exhaustive()
    }
}

impl OutboundRequestMeta {
    /// Call-local protocol bytes, never serialized as an outbound header.
    #[must_use]
    pub fn correlation_header(&self) -> Option<&[u8]> {
        self.correlation.header()
    }

    /// Correlation for an ordinal in this call's ordered covered-impression list.
    #[must_use]
    pub fn imp_correlation(&self, ordinal: u16) -> Option<ImpCorrelation> {
        self.correlation.row(ordinal)
    }

    pub(crate) const fn is_finalized(&self) -> bool {
        self.finalized
    }

    pub(crate) const fn mark_finalized(&mut self) {
        self.finalized = true;
    }

    pub(crate) fn append_exchange_header(
        &mut self,
        name: &str,
        value: &str,
    ) -> Result<(), FinalizeError> {
        let start = self.bytes.len().max(META_BIDDER_REGION_BYTES);
        let middle = start
            .checked_add(name.len())
            .ok_or(FinalizeError::ReserveOverflow)?;
        let end = middle
            .checked_add(value.len())
            .ok_or(FinalizeError::ReserveOverflow)?;
        if end > META_BYTES_CAPACITY
            || end > self.bytes.capacity()
            || self.headers.len()
                >= usize::from(MAX_BIDDER_HEADERS + RESERVED_EXCHANGE_HEADER_SLOTS)
            || self.headers.len() >= self.headers.capacity()
        {
            return Err(FinalizeError::ReserveOverflow);
        }
        let offset =
            |n| u16::try_from(n).map_err(|_conversion_error| FinalizeError::ReserveOverflow);
        let entry = (
            offset(start)?,
            offset(name.len())?,
            offset(middle)?,
            offset(value.len())?,
        );
        self.bytes.resize(start, 0);
        self.bytes.extend_from_slice(name.as_bytes());
        self.bytes.extend_from_slice(value.as_bytes());
        self.headers.push(entry);
        Ok(())
    }

    #[must_use]
    pub fn method(&self) -> &str {
        self.slice(self.method.0, self.method.1)
    }

    #[must_use]
    pub fn url(&self) -> &str {
        self.slice(self.url.0, self.url.1)
    }

    /// Header pairs in insertion order, duplicates preserved. Reserved
    /// names were rejected at push, so nothing here needs filtering.
    pub fn headers(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.headers
            .iter()
            .map(|&(n_start, n_len, v_start, v_len)| {
                (self.slice(n_start, n_len), self.slice(v_start, v_len))
            })
    }

    /// The imp ids this call covers, in append order, for `seatnonbid`
    /// mapping. Empty when the bidder did not pre-correlate.
    pub fn covered_imp_ids(&self) -> impl Iterator<Item = &str> + '_ {
        self.imp_ids
            .iter()
            .map(|&(start, len)| self.slice(start, len))
    }

    /// Header count, so a caller can size-check without collecting.
    #[must_use]
    pub fn header_count(&self) -> u16 {
        // The builder refuses past `MAX_BIDDER_HEADERS +
        // RESERVED_EXCHANGE_HEADER_SLOTS`, so this never truncates.
        u16::try_from(self.headers.len()).unwrap_or(u16::MAX)
    }

    /// Imp-id count, so a caller can size-check without collecting.
    #[must_use]
    pub fn imp_id_count(&self) -> u16 {
        u16::try_from(self.imp_ids.len()).unwrap_or(u16::MAX)
    }

    fn slice(&self, start: u16, len: u16) -> &str {
        let from = usize::from(start);
        let to = from.saturating_add(usize::from(len));
        // Every range was produced by `append`, which only ever stores
        // whole `&str` inputs at UTF-8 boundaries within `bytes`.
        self.bytes
            .get(from..to)
            .and_then(|raw| str::from_utf8(raw).ok())
            .unwrap_or("")
    }
}

/// Bounded builder over one meta's byte pool and offset tables.
pub struct OutboundRequestMetaBuilder<'a> {
    bytes: &'a mut Vec<u8>,
    method: Option<(u16, u16)>,
    url: Option<(u16, u16)>,
    headers: Vec<(u16, u16, u16, u16)>,
    imp_ids: Vec<(u16, u16)>,
    max_imp_ids: u16,
    correlation: CallCorrelation,
}

impl<'a> OutboundRequestMetaBuilder<'a> {
    /// Set a bounded protocol header exactly once.
    ///
    /// # Errors
    /// Returns a fixed correlation refusal without retaining rejected bytes.
    pub fn set_correlation_header(&mut self, header: &[u8]) -> Result<(), SinkAbort> {
        self.correlation.set_header(header).map_err(SinkAbort::from)
    }

    /// Attach media and a small protocol token to one covered-impression ordinal.
    ///
    /// # Errors
    /// Refuses unknown or repeated ordinals without replacing existing context.
    pub fn correlate_imp(
        &mut self,
        ordinal: u16,
        media: CorrelationMedia,
        token: u32,
    ) -> Result<(), SinkAbort> {
        let covered =
            u16::try_from(self.imp_ids.len()).map_err(|_error| CorrelationError::UnknownOrdinal)?;
        self.correlation
            .set_row(ordinal, covered, self.max_imp_ids, media, token)
            .map_err(SinkAbort::from)
    }

    /// Start a builder over a caller-owned byte pool.
    ///
    /// `bytes` must be an empty `Vec::with_capacity(META_BYTES_CAPACITY)`;
    /// the builder appends only within
    /// `[0, META_BIDDER_REGION_BYTES)` and never grows it.
    #[must_use]
    pub fn new(bytes: &'a mut Vec<u8>, max_impressions_per_auction: u16) -> Self {
        Self {
            bytes,
            method: None,
            url: None,
            headers: Vec::with_capacity(
                usize::from(MAX_BIDDER_HEADERS)
                    .saturating_add(usize::from(RESERVED_EXCHANGE_HEADER_SLOTS)),
            ),
            imp_ids: Vec::with_capacity(usize::from(max_impressions_per_auction)),
            max_imp_ids: max_impressions_per_auction,
            correlation: CallCorrelation::default(),
        }
    }

    /// Set the outbound HTTP method. Callable once.
    ///
    /// # Errors
    /// [`SinkAbort::DuplicateMeta`] on a second call, leaving the first
    /// value untouched, or [`SinkAbort::MetaOverCap`] if the value does
    /// not fit the bidder region.
    pub fn set_method(&mut self, method: &str) -> Result<(), SinkAbort> {
        if self.method.is_some() {
            return Err(SinkAbort::DuplicateMeta {
                field: MetaField::Method,
            });
        }
        self.method = Some(self.append(method)?);
        Ok(())
    }

    /// Set the outbound URL. Callable once.
    ///
    /// # Errors
    /// [`SinkAbort::DuplicateMeta`] on a second call, leaving the first
    /// value untouched, or [`SinkAbort::MetaOverCap`] if the value does
    /// not fit the bidder region.
    pub fn set_url(&mut self, url: &str) -> Result<(), SinkAbort> {
        if self.url.is_some() {
            return Err(SinkAbort::DuplicateMeta {
                field: MetaField::Url,
            });
        }
        self.url = Some(self.append(url)?);
        Ok(())
    }

    /// Stream a URL into the pool, rolling back the component on any error.
    ///
    /// # Errors
    /// Returns a duplicate-URL error, a pool overflow, or the closure's error.
    pub fn set_url_with<F>(&mut self, build: F) -> Result<(), SinkAbort>
    where
        F: FnOnce(&mut dyn fmt::Write) -> Result<(), SinkAbort>,
    {
        self.set_url_buffer_with(|url| build(url.writer()))
    }

    pub(crate) fn set_url_buffer_with<F>(&mut self, build: F) -> Result<(), SinkAbort>
    where
        F: FnOnce(&mut UrlBuffer<'_>) -> Result<(), SinkAbort>,
    {
        if self.url.is_some() {
            return Err(SinkAbort::DuplicateMeta {
                field: MetaField::Url,
            });
        }
        let start = self.bytes.len();
        let result = build(&mut UrlBuffer {
            pool: MetaPool {
                bytes: self.bytes,
                overflowed: false,
            },
            start,
        });
        if let Err(error) = result {
            self.bytes.truncate(start);
            return Err(error);
        }
        let Some(range) = u16::try_from(start)
            .ok()
            .zip(u16::try_from(self.bytes.len().saturating_sub(start)).ok())
        else {
            self.bytes.truncate(start);
            return Err(Self::over_cap());
        };
        self.url = Some(range);
        Ok(())
    }

    /// Append one header. Order is preserved and duplicates are allowed.
    ///
    /// # Errors
    /// [`SinkAbort::ReservedHeader`] for a name the transport or the
    /// exchange owns (compared case-insensitively),
    /// [`SinkAbort::TooManyHeaders`] past [`MAX_BIDDER_HEADERS`], and
    /// [`SinkAbort::MetaOverCap`] when the pair does not fit the
    /// bidder region — in every case nothing is appended.
    pub fn header(&mut self, name: &str, value: &str) -> Result<(), SinkAbort> {
        if RESERVED_HEADER_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(name))
        {
            return Err(SinkAbort::ReservedHeader { name: name.into() });
        }
        if self.headers.len() >= usize::from(MAX_BIDDER_HEADERS) {
            return Err(SinkAbort::TooManyHeaders {
                limit: MAX_BIDDER_HEADERS,
            });
        }
        // Size-check the pair as a unit so a rejected value cannot leave
        // an orphaned name in the pool.
        if self.remaining() < name.len().saturating_add(value.len()) {
            return Err(Self::over_cap());
        }
        let (n_start, n_len) = self.append(name)?;
        let (v_start, v_len) = self.append(value)?;
        self.headers.push((n_start, n_len, v_start, v_len));
        Ok(())
    }

    /// Stream a header value into the fixed pool, preserving order and duplicates.
    /// The callback runs only after the name and header count pass preflight.
    /// Reserved names use ASCII case-insensitive matching and take priority
    /// over count and capacity failures.
    ///
    /// # Errors
    /// Returns [`SinkAbort::ReservedHeader`], [`SinkAbort::TooManyHeaders`],
    /// [`SinkAbort::MetaOverCap`], or the callback's error. Every refusal rolls
    /// back the name and value; a swallowed formatting overflow still refuses.
    pub fn header_with<F>(&mut self, name: &str, build: F) -> Result<(), SinkAbort>
    where
        F: FnOnce(&mut dyn fmt::Write) -> Result<(), SinkAbort>,
    {
        if RESERVED_HEADER_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(name))
        {
            return Err(SinkAbort::ReservedHeader { name: name.into() });
        }
        if self.headers.len() >= usize::from(MAX_BIDDER_HEADERS) {
            return Err(SinkAbort::TooManyHeaders {
                limit: MAX_BIDDER_HEADERS,
            });
        }
        let start = self.bytes.len();
        let Some(value_start) = start
            .checked_add(name.len())
            .filter(|&end| end <= META_BIDDER_REGION_BYTES.min(self.bytes.capacity()))
        else {
            return Err(Self::over_cap());
        };
        let (Ok(n_start), Ok(n_len), Ok(v_start)) = (
            u16::try_from(start),
            u16::try_from(name.len()),
            u16::try_from(value_start),
        ) else {
            return Err(Self::over_cap());
        };
        self.bytes.extend_from_slice(name.as_bytes());
        let mut pool = MetaPool {
            bytes: self.bytes,
            overflowed: false,
        };
        let result = build(&mut pool).and_then(|()| {
            if pool.overflowed {
                return Err(Self::over_cap());
            }
            u16::try_from(pool.bytes.len().saturating_sub(value_start))
                .map_err(|_error| Self::over_cap())
        });
        let v_len = match result {
            Ok(len) => len,
            Err(error) => {
                pool.bytes.truncate(start);
                return Err(error);
            }
        };
        self.headers.push((n_start, n_len, v_start, v_len));
        Ok(())
    }

    /// Append one covered imp id.
    ///
    /// # Errors
    /// [`SinkAbort::TooManyImpIds`] when the offset table is full, or
    /// [`SinkAbort::MetaOverCap`] when the id does not fit the bidder
    /// region.
    pub fn imp_id(&mut self, id: &str) -> Result<(), SinkAbort> {
        if self.imp_ids.len() >= usize::from(self.max_imp_ids) {
            return Err(SinkAbort::TooManyImpIds {
                limit: self.max_imp_ids,
            });
        }
        if self.imp_ids.iter().any(|&(start, length)| {
            let from = usize::from(start);
            let to = from.saturating_add(usize::from(length));
            self.bytes.get(from..to) == Some(id.as_bytes())
        }) {
            return Err(CorrelationError::DuplicateCoverage.into());
        }
        let before = self.bytes.len();
        let range = self.append(id)?;
        if let Err(error) = self.correlation.extend_coverage() {
            self.bytes.truncate(before);
            return Err(error.into());
        }
        self.imp_ids.push(range);
        Ok(())
    }

    /// Freeze the builder into a finished meta.
    ///
    /// An unset method or URL reads back as the empty string; the sink
    /// requires both before it accepts a request.
    #[must_use]
    pub fn finish(self) -> OutboundRequestMeta {
        OutboundRequestMeta {
            bytes: mem::take(self.bytes),
            method: self.method.unwrap_or((0, 0)),
            url: self.url.unwrap_or((0, 0)),
            headers: self.headers,
            imp_ids: self.imp_ids,
            correlation: self.correlation,
            finalized: false,
        }
    }

    /// Bytes appended into the bidder region so far.
    #[must_use]
    pub fn written(&self) -> usize {
        self.bytes.len()
    }

    fn remaining(&self) -> usize {
        META_BIDDER_REGION_BYTES.saturating_sub(self.bytes.len())
    }

    const fn over_cap() -> SinkAbort {
        SinkAbort::MetaOverCap {
            limit: META_BIDDER_REGION_BYTES_U64,
        }
    }

    /// Append one component and return its `(start, len)`. Errors BEFORE
    /// writing anything, so a refused append leaves the pool untouched.
    fn append(&mut self, value: &str) -> Result<(u16, u16), SinkAbort> {
        let len = value.len();
        if self.remaining() < len {
            return Err(Self::over_cap());
        }
        let offset = self.bytes.len();
        // The bidder region is 3_328 bytes, so both fit a u16 given the
        // bound check above.
        let (Ok(start), Ok(width)) = (u16::try_from(offset), u16::try_from(len)) else {
            return Err(Self::over_cap());
        };
        self.bytes.extend_from_slice(value.as_bytes());
        Ok((start, width))
    }
}

/// URL-only transactional view. Editing cannot touch earlier metadata or grow
/// outside the existing pool; the owning builder commits the final range.
pub(crate) struct UrlBuffer<'a> {
    pool: MetaPool<'a>,
    start: usize,
}

impl UrlBuffer<'_> {
    pub(crate) fn writer(&mut self) -> &mut dyn fmt::Write {
        &mut self.pool
    }

    pub(crate) fn as_str(&self) -> Result<&str, SinkAbort> {
        self.pool
            .bytes
            .get(self.start..)
            .and_then(|bytes| str::from_utf8(bytes).ok())
            .ok_or_else(OutboundRequestMetaBuilder::over_cap)
    }

    pub(crate) fn remove(&mut self, range: Range<usize>) -> Result<(), SinkAbort> {
        if self.as_str()?.get(range.clone()).is_none() {
            return Err(OutboundRequestMetaBuilder::over_cap());
        }
        let start = self.start.saturating_add(range.start);
        let end = self.start.saturating_add(range.end);
        self.pool.bytes.drain(start..end);
        Ok(())
    }
}

struct MetaPool<'a> {
    bytes: &'a mut Vec<u8>,
    // Remember overflow without changing later writes or URL error precedence.
    overflowed: bool,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "provided write_char and write_fmt delegate to the bounded write_str"
)]
impl fmt::Write for MetaPool<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let available = META_BIDDER_REGION_BYTES
            .min(self.bytes.capacity())
            .saturating_sub(self.bytes.len());
        if s.len() > available {
            self.overflowed = true;
            return Err(fmt::Error);
        }
        self.bytes.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_BIDDER_HEADERS, META_BIDDER_REGION_BYTES, META_BIDDER_REGION_BYTES_U64,
        META_BYTES_CAPACITY, MetaField, MetaPool, OutboundRequestMetaBuilder, SinkAbort, UrlBuffer,
        outbound_meta_bytes,
    };

    use std::ops::Range;
    use std::str::from_utf8;

    const DEFAULT_MAX_IMPS: u16 = 100;

    #[test]
    fn url_edits_are_relative_utf8_checked_and_cannot_change_earlier_components() {
        let mut bytes = b"POSTheaderhttps://test/\xc3\xa9?a=1".to_vec();
        let mut url = UrlBuffer {
            pool: MetaPool {
                bytes: &mut bytes,
                overflowed: false,
            },
            start: 10,
        };
        let before = url.as_str().unwrap().to_owned();
        for range in [14..15, 0..usize::MAX, Range { start: 4, end: 1 }] {
            assert_eq!(
                url.remove(range),
                Err(OutboundRequestMetaBuilder::over_cap())
            );
            assert_eq!(url.as_str().unwrap(), before);
        }
        url.remove(13..15).unwrap();
        assert_eq!(url.as_str().unwrap(), "https://test/?a=1");
        assert_eq!(bytes, b"POSTheaderhttps://test/?a=1");
    }

    #[test]
    fn call_correlation_survives_metadata_freeze_without_entering_wire_fields() {
        use crate::correlation::CorrelationMedia;
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, 2);
        builder.set_method("POST").unwrap();
        builder.set_url("https://bidder.example.test/").unwrap();
        builder.imp_id("shared").unwrap();
        builder.set_correlation_header(b"private-token").unwrap();
        builder
            .correlate_imp(0, CorrelationMedia::Video, 4321)
            .unwrap();
        let meta = builder.finish();
        assert_eq!(meta.correlation_header(), Some(b"private-token".as_slice()));
        let row = meta.imp_correlation(0).unwrap();
        assert_eq!(row.media(), CorrelationMedia::Video);
        assert_eq!(row.token(), 4321);
        assert_eq!(meta.imp_correlation(1), None);
        assert_eq!(meta.covered_imp_ids().collect::<Vec<_>>(), ["shared"]);
        assert_eq!(meta.header_count(), 0);
        assert_eq!(meta.url(), "https://bidder.example.test/");
        assert!(!format!("{meta:?}").contains("private-token"));
    }

    #[test]
    fn duplicate_coverage_and_correlation_refusals_leave_metadata_unchanged() {
        use crate::correlation::{CorrelationError, CorrelationMedia};
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, 2);
        builder.imp_id("first").unwrap();
        let before = builder.written();
        assert_eq!(
            builder.imp_id("first"),
            Err(SinkAbort::Correlation(CorrelationError::DuplicateCoverage))
        );
        assert_eq!(builder.written(), before);
        assert_eq!(
            builder.correlate_imp(1, CorrelationMedia::Video, 5),
            Err(SinkAbort::Correlation(CorrelationError::UnknownOrdinal))
        );
        builder
            .correlate_imp(0, CorrelationMedia::Banner, 7)
            .unwrap();
        assert_eq!(
            builder.correlate_imp(0, CorrelationMedia::Video, 9),
            Err(SinkAbort::Correlation(CorrelationError::DuplicateOrdinal))
        );
        builder.imp_id("second").unwrap();
        builder
            .correlate_imp(1, CorrelationMedia::Audio, 11)
            .unwrap();
        assert_eq!(
            builder.set_correlation_header(&[3; 33]),
            Err(SinkAbort::Correlation(CorrelationError::HeaderOverCap))
        );
        let meta = builder.finish();
        assert_eq!(meta.imp_id_count(), 2);
        assert_eq!(meta.imp_correlation(0).unwrap().token(), 7);
        assert_eq!(meta.imp_correlation(1).unwrap().token(), 11);
        assert_eq!(meta.correlation_header(), None);
    }

    fn pool() -> Vec<u8> {
        Vec::with_capacity(META_BYTES_CAPACITY)
    }

    #[test]
    fn outbound_meta_bytes_matches_worked_values() {
        assert_eq!(outbound_meta_bytes(0), 4_064);
        assert_eq!(outbound_meta_bytes(8), 4_192);
        assert_eq!(outbound_meta_bytes(100), 5_248);
        assert_eq!(outbound_meta_bytes(1_000), 16_096);

        // Monotonic in between, so the budget can never diverge from the
        // byte pool as the impression limit moves.
        let mut previous = outbound_meta_bytes(1);
        for imps in 2..=1_000_u16 {
            let current = outbound_meta_bytes(imps);
            assert!(
                current >= previous,
                "outbound_meta_bytes({imps}) = {current} < {previous}"
            );
            previous = current;
        }
    }

    #[test]
    fn meta_builder_admits_32_bidder_headers() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);

        let names: Vec<String> = (0..MAX_BIDDER_HEADERS)
            .map(|index| format!("X-Bidder-{index}"))
            .collect();
        for (index, name) in names.iter().enumerate() {
            builder.header(name, &format!("v{index}")).unwrap();
        }

        let meta = builder.finish();
        assert_eq!(meta.header_count(), MAX_BIDDER_HEADERS);
        let seen: Vec<(String, String)> = meta
            .headers()
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect();
        assert_eq!(seen.len(), usize::from(MAX_BIDDER_HEADERS));
        for (index, (name, value)) in seen.iter().enumerate() {
            assert_eq!(name.as_str(), names[index].as_str());
            assert_eq!(value.as_str(), format!("v{index}").as_str());
        }
    }

    #[test]
    fn metadata_debug_omits_the_raw_pool_and_credential_offsets() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.set_method("POST").unwrap();
        builder
            .set_url("https://example.test/credential-sentinel")
            .unwrap();
        builder.header("Authorization", "header-sentinel").unwrap();
        builder.imp_id("imp-sentinel").unwrap();
        let meta = builder.finish();
        assert_eq!(
            format!("{meta:?}"),
            "OutboundRequestMeta { header_count: 1, imp_id_count: 1, .. }"
        );
        assert_eq!(
            format!("{meta:#?}"),
            "OutboundRequestMeta {\n    header_count: 1,\n    imp_id_count: 1,\n    ..\n}"
        );
    }

    #[test]
    fn meta_builder_rejects_33rd_bidder_header_without_growing() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);

        for index in 0..MAX_BIDDER_HEADERS {
            builder.header(&format!("X-H-{index}"), "v").unwrap();
        }
        let len_before = builder.written();

        let err = builder.header("X-One-Too-Many", "v").unwrap_err();
        assert_eq!(
            err,
            SinkAbort::TooManyHeaders {
                limit: MAX_BIDDER_HEADERS
            }
        );
        assert_eq!(builder.written(), len_before);

        drop(builder);
        assert_eq!(bytes.len(), len_before);
        assert_eq!(
            bytes.capacity(),
            META_BYTES_CAPACITY,
            "a refused header must not grow the byte pool"
        );
    }

    #[test]
    fn meta_builder_reserved_name_rejected_case_insensitively() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);

        for name in ["content-LENGTH", "hOsT", "transfer-encoding"] {
            let err = builder.header(name, "x").unwrap_err();
            assert_eq!(
                err,
                SinkAbort::ReservedHeader {
                    name: name.to_owned()
                }
            );
        }
        assert_eq!(builder.written(), 0);
    }

    #[test]
    fn meta_builder_over_cap_errors_before_write() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);

        let huge = "v".repeat(META_BIDDER_REGION_BYTES);
        let err = builder.header("X-Big", &huge).unwrap_err();
        assert_eq!(
            err,
            SinkAbort::MetaOverCap {
                limit: META_BIDDER_REGION_BYTES_U64
            }
        );
        assert_eq!(
            builder.written(),
            0,
            "an over-cap header must not append its name"
        );

        let meta = builder.finish();
        assert_eq!(meta.header_count(), 0);
    }

    #[test]
    fn set_url_with_preserves_callback_error_after_capacity_failure() {
        let filler = [b'u'; META_BIDDER_REGION_BYTES];
        let oversized = from_utf8(&filler).unwrap();
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.set_method("POST").unwrap();
        builder.header("X-Keep", "first").unwrap();
        let before = builder.bytes.clone();
        let callback_error = SinkAbort::DuplicateMeta {
            field: MetaField::Method,
        };
        assert_eq!(
            builder.set_url_with(|url| {
                url.write_str("https://")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())?;
                assert!(url.write_str(oversized).is_err());
                Err(callback_error.clone())
            }),
            Err(callback_error)
        );
        assert_eq!(builder.bytes.as_slice(), before.as_slice());
        assert_eq!(builder.url, None);
        builder
            .set_url_with(|url| {
                url.write_str("https://example.test/retry")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            })
            .unwrap();
        let meta = builder.finish();
        assert_eq!(meta.method(), "POST");
        assert_eq!(meta.url(), "https://example.test/retry");
        assert_eq!(meta.headers().collect::<Vec<_>>(), [("X-Keep", "first")]);
    }

    #[test]
    fn set_url_with_preserves_success_after_swallowed_capacity_failure() {
        let filler = [b'u'; META_BIDDER_REGION_BYTES];
        let oversized = from_utf8(&filler).unwrap();
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.set_method("POST").unwrap();
        builder
            .set_url_with(|url| {
                url.write_str("https://")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())?;
                assert!(url.write_str(oversized).is_err());
                url.write_str("example.test/")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            })
            .unwrap();
        let meta = builder.finish();
        assert_eq!(meta.method(), "POST");
        assert_eq!(meta.url(), "https://example.test/");
    }

    #[test]
    fn header_with_streams_formatted_values_preserving_order_and_duplicates() {
        let mut bytes = pool();
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.set_method("POST").unwrap();
        builder
            .set_url_with(|url| {
                url.write_str("https://example.test/bid")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            })
            .unwrap();
        builder.header("Cookie", "first=1").unwrap();
        let mut calls = 0_u32;
        let id = 7_i32;
        let token = "abc";
        builder
            .header_with("Cookie", |value| {
                calls += 1;
                write!(value, "id={id}; token={token}")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            })
            .unwrap();
        builder.header("X-After", "last").unwrap();
        builder.imp_id("imp-1").unwrap();

        assert_eq!(calls, 1);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);
        let meta = builder.finish();
        assert_eq!(meta.method(), "POST");
        assert_eq!(meta.url(), "https://example.test/bid");
        assert_eq!(meta.covered_imp_ids().collect::<Vec<_>>(), ["imp-1"]);
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [
                ("Cookie", "first=1"),
                ("Cookie", "id=7; token=abc"),
                ("X-After", "last"),
            ]
        );
    }

    #[test]
    fn header_with_accepts_empty_and_utf8_components_without_unicode_case_folding() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        let mut calls = 0_u32;
        for (name, text) in [
            ("", ""),
            ("X-Empty", ""),
            ("", "value"),
            ("X-\u{e9}", "caf\u{e9} \u{1f680}"),
            ("Content-Length-Extra", "allowed"),
            ("ho\u{17f}t", "not an ASCII reserved name"),
        ] {
            builder
                .header_with(name, |value| {
                    calls += 1;
                    value
                        .write_str(text)
                        .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
                })
                .unwrap();
        }
        assert_eq!(calls, 6);
        let meta = builder.finish();
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [
                ("", ""),
                ("X-Empty", ""),
                ("", "value"),
                ("X-\u{e9}", "caf\u{e9} \u{1f680}"),
                ("Content-Length-Extra", "allowed"),
                ("ho\u{17f}t", "not an ASCII reserved name"),
            ]
        );
    }

    #[test]
    fn header_with_accepts_exact_and_one_below_pool_boundary() {
        let filler = [b'v'; META_BIDDER_REGION_BYTES];
        for spare in [1, 0] {
            let mut bytes = pool();
            let pointer = bytes.as_ptr();
            let capacity = bytes.capacity();
            let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
            builder.set_method("POST").unwrap();
            builder.set_url("https://example.test/").unwrap();
            builder.header("X-Keep", "first").unwrap();
            let name = "Cookie";
            let start = builder.written();
            let value_len = META_BIDDER_REGION_BYTES - start - name.len() - spare;
            let prefix = from_utf8(&filler[..value_len - 2]).unwrap();
            let mut calls = 0_u32;
            builder
                .header_with(name, |value| {
                    calls += 1;
                    value
                        .write_str(prefix)
                        .map_err(|_error| OutboundRequestMetaBuilder::over_cap())?;
                    value
                        .write_char('\u{e9}')
                        .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
                })
                .unwrap();
            assert_eq!(calls, 1);
            assert_eq!(builder.written(), META_BIDDER_REGION_BYTES - spare);
            assert_eq!(
                builder.headers[1],
                (
                    u16::try_from(start).unwrap(),
                    u16::try_from(name.len()).unwrap(),
                    u16::try_from(start + name.len()).unwrap(),
                    u16::try_from(value_len).unwrap(),
                )
            );

            let end = builder.written();
            builder
                .header_with("", |_value| {
                    calls += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(calls, 2);
            assert_eq!(builder.written(), end);
            assert_eq!(builder.bytes.as_ptr(), pointer);
            assert_eq!(builder.bytes.capacity(), capacity);
            let meta = builder.finish();
            let mut headers = meta.headers();
            assert_eq!(headers.next(), Some(("X-Keep", "first")));
            let (seen_name, seen_value) = headers.next().unwrap();
            assert_eq!(seen_name, name);
            assert_eq!(seen_value.len(), value_len);
            assert_eq!(seen_value.strip_suffix('\u{e9}'), Some(prefix));
            assert_eq!(headers.next(), Some(("", "")));
            assert_eq!(headers.next(), None);
            assert_eq!(meta.method(), "POST");
            assert_eq!(meta.url(), "https://example.test/");
        }
    }

    #[test]
    fn header_with_one_byte_over_capacity_rolls_back_and_allows_exact_retry() {
        let filler = [b'v'; META_BIDDER_REGION_BYTES];
        let mut bytes = pool();
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.set_method("POST").unwrap();
        builder.set_url("https://example.test/").unwrap();
        builder.header("X-Keep", "first").unwrap();
        let before = builder.bytes.clone();
        let offsets_before = builder.headers.clone();
        let available = META_BIDDER_REGION_BYTES - builder.written() - "Cookie".len();
        let oversized_prefix = from_utf8(&filler[..available - 1]).unwrap();
        let mut calls = 0_u32;

        assert_eq!(
            builder.header_with("Cookie", |value| {
                calls += 1;
                value
                    .write_str(oversized_prefix)
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())?;
                value
                    .write_char('\u{e9}')
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            }),
            Err(OutboundRequestMetaBuilder::over_cap())
        );
        assert_eq!(calls, 1);
        assert_eq!(builder.bytes.as_slice(), before.as_slice());
        assert_eq!(builder.headers, offsets_before);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);

        let prefix = from_utf8(&filler[..available - 2]).unwrap();
        builder
            .header_with("Cookie", |value| {
                calls += 1;
                value
                    .write_str(prefix)
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())?;
                value
                    .write_char('\u{e9}')
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            })
            .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(builder.written(), META_BIDDER_REGION_BYTES);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);
        let meta = builder.finish();
        assert_eq!(meta.header_count(), 2);
        let mut headers = meta.headers();
        assert_eq!(headers.next(), Some(("X-Keep", "first")));
        let (name, value) = headers.next().unwrap();
        assert_eq!(name, "Cookie");
        assert_eq!(value.len(), available);
        assert_eq!(value.strip_suffix('\u{e9}'), Some(prefix));
        assert_eq!(headers.next(), None);
        assert_eq!(meta.method(), "POST");
        assert_eq!(meta.url(), "https://example.test/");
    }

    #[test]
    fn header_with_partial_callback_error_rolls_back_and_allows_retry() {
        let mut bytes = pool();
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.set_method("POST").unwrap();
        builder.set_url("https://example.test/").unwrap();
        builder.header("X-Keep", "first").unwrap();
        let before = builder.bytes.clone();
        let offsets_before = builder.headers.clone();
        let callback_error = SinkAbort::DuplicateMeta {
            field: MetaField::Url,
        };
        let mut calls = 0_u32;

        assert_eq!(
            builder.header_with("Cookie", |value| {
                calls += 1;
                value
                    .write_str("partial=\u{e9}")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())?;
                Err(callback_error.clone())
            }),
            Err(callback_error)
        );
        assert_eq!(calls, 1);
        assert_eq!(builder.bytes.as_slice(), before.as_slice());
        assert_eq!(builder.headers, offsets_before);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);

        builder
            .header_with("Cookie", |value| {
                calls += 1;
                value
                    .write_str("retry=ok")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            })
            .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);
        let meta = builder.finish();
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [("X-Keep", "first"), ("Cookie", "retry=ok")]
        );
        assert_eq!(meta.method(), "POST");
        assert_eq!(meta.url(), "https://example.test/");
    }

    #[test]
    fn header_with_does_not_commit_a_suppressed_capacity_error() {
        let filler = [b'v'; META_BIDDER_REGION_BYTES];
        let oversized = from_utf8(&filler).unwrap();
        let mut bytes = pool();
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.header("X-Keep", "first").unwrap();
        let before = builder.bytes.clone();
        let offsets_before = builder.headers.clone();
        let mut calls = 0_u32;
        assert_eq!(
            builder.header_with("Cookie", |value| {
                calls += 1;
                value
                    .write_str("partial")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())?;
                assert!(value.write_str(oversized).is_err());
                let _result = value.write_str("ignored");
                Ok(())
            }),
            Err(OutboundRequestMetaBuilder::over_cap())
        );
        assert_eq!(calls, 1);
        assert_eq!(builder.bytes.as_slice(), before.as_slice());
        assert_eq!(builder.headers, offsets_before);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);
        builder
            .header_with("Cookie", |value| {
                value
                    .write_str("retry=ok")
                    .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
            })
            .unwrap();
        let meta = builder.finish();
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [("X-Keep", "first"), ("Cookie", "retry=ok")]
        );
    }

    #[test]
    fn header_with_over_cap_name_skips_callback_and_does_not_commit_offsets() {
        let names = [b'n'; META_BIDDER_REGION_BYTES];
        let mut bytes = pool();
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        builder.set_method("POST").unwrap();
        builder.header("X-Keep", "first").unwrap();
        let before = builder.bytes.clone();
        let offsets_before = builder.headers.clone();
        let available = META_BIDDER_REGION_BYTES - builder.written();
        let oversized = from_utf8(&names[..=available]).unwrap();
        let mut called = false;
        assert_eq!(
            builder.header_with(oversized, |_value| {
                called = true;
                Ok(())
            }),
            Err(OutboundRequestMetaBuilder::over_cap())
        );
        assert!(!called);
        assert_eq!(builder.bytes.as_slice(), before.as_slice());
        assert_eq!(builder.headers, offsets_before);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);

        let exact = from_utf8(&names[..available]).unwrap();
        builder
            .header_with(exact, |_value| {
                called = true;
                Ok(())
            })
            .unwrap();
        assert!(called);
        assert_eq!(builder.written(), META_BIDDER_REGION_BYTES);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);
        let meta = builder.finish();
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [("X-Keep", "first"), (exact, "")]
        );
        assert_eq!(meta.method(), "POST");
    }

    #[test]
    fn header_with_reserved_names_take_priority_and_skip_callback() {
        let filler = [b'u'; META_BIDDER_REGION_BYTES];
        for full_table in [false, true] {
            let mut bytes = pool();
            let pointer = bytes.as_ptr();
            let capacity = bytes.capacity();
            let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
            builder.header("X-Keep", "first").unwrap();
            if full_table {
                for _index in 1..MAX_BIDDER_HEADERS {
                    builder.header("X", "v").unwrap();
                }
            }
            let remaining = META_BIDDER_REGION_BYTES - builder.written();
            builder
                .set_url(from_utf8(&filler[..remaining]).unwrap())
                .unwrap();
            let before = builder.bytes.clone();
            let offsets_before = builder.headers.clone();
            for name in [
                "content-ENCODING",
                "CONTENT-length",
                "Sec-GpC",
                "hOsT",
                ":AuThOrItY",
                "Transfer-Encoding",
                "tE",
                "CONNECTION",
                "tRaIlEr",
                "UpGrAdE",
                "Proxy-Connection",
                "KEEP-alive",
            ] {
                let mut called = false;
                assert_eq!(
                    builder.header_with(name, |_value| {
                        called = true;
                        Ok(())
                    }),
                    Err(SinkAbort::ReservedHeader {
                        name: name.to_owned()
                    })
                );
                assert!(!called);
                assert_eq!(builder.bytes.as_slice(), before.as_slice());
                assert_eq!(builder.headers, offsets_before);
                assert_eq!(builder.bytes.as_ptr(), pointer);
                assert_eq!(builder.bytes.capacity(), capacity);
            }
        }
    }

    #[test]
    fn header_with_admits_32_bidder_headers_in_order() {
        let mut bytes = pool();
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        let offsets_pointer = builder.headers.as_ptr();
        let offsets_capacity = builder.headers.capacity();
        let mut calls = 0;
        for _index in 0..MAX_BIDDER_HEADERS {
            builder
                .header_with("X-Duplicate", |value| {
                    calls += 1;
                    value
                        .write_str("v")
                        .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
                })
                .unwrap();
        }
        assert_eq!(calls, MAX_BIDDER_HEADERS);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);
        assert_eq!(builder.headers.as_ptr(), offsets_pointer);
        assert_eq!(builder.headers.capacity(), offsets_capacity);
        let meta = builder.finish();
        assert_eq!(meta.header_count(), MAX_BIDDER_HEADERS);
        assert!(meta.headers().all(|pair| pair == ("X-Duplicate", "v")));
    }

    #[test]
    fn header_with_full_table_skips_callback_before_capacity_check() {
        let filler = [b'u'; META_BIDDER_REGION_BYTES];
        let mut bytes = pool();
        let pointer = bytes.as_ptr();
        let capacity = bytes.capacity();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
        for _index in 0..MAX_BIDDER_HEADERS {
            builder.header("X", "v").unwrap();
        }
        let remaining = META_BIDDER_REGION_BYTES - builder.written();
        builder
            .set_url(from_utf8(&filler[..remaining]).unwrap())
            .unwrap();
        let before = builder.bytes.clone();
        let offsets_before = builder.headers.clone();
        let mut called = false;
        assert_eq!(
            builder.header_with("X-One-Too-Many", |_value| {
                called = true;
                Ok(())
            }),
            Err(SinkAbort::TooManyHeaders {
                limit: MAX_BIDDER_HEADERS
            })
        );
        assert!(!called);
        assert_eq!(builder.bytes.as_slice(), before.as_slice());
        assert_eq!(builder.headers, offsets_before);
        assert_eq!(builder.bytes.as_ptr(), pointer);
        assert_eq!(builder.bytes.capacity(), capacity);
        let meta = builder.finish();
        assert_eq!(meta.header_count(), MAX_BIDDER_HEADERS);
        assert!(meta.headers().all(|pair| pair == ("X", "v")));
    }

    #[test]
    fn header_with_never_reallocates_a_smaller_backing_pool() {
        let filler = [b'v'; META_BYTES_CAPACITY];
        for name_fits in [false, true] {
            let mut bytes = Vec::with_capacity(16);
            let pointer = bytes.as_ptr();
            let capacity = bytes.capacity();
            let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);
            builder.set_method("POST").unwrap();
            let before = builder.bytes.clone();
            let remaining = capacity - builder.written();
            let text = from_utf8(&filler[..=remaining]).unwrap();
            let name = if name_fits { "X" } else { text };
            let mut called = false;
            assert_eq!(
                builder.header_with(name, |value| {
                    called = true;
                    value
                        .write_str(text)
                        .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
                }),
                Err(OutboundRequestMetaBuilder::over_cap())
            );
            assert_eq!(called, name_fits);
            assert_eq!(builder.bytes.as_slice(), before.as_slice());
            assert!(builder.headers.is_empty());
            assert_eq!(builder.bytes.as_ptr(), pointer);
            assert_eq!(builder.bytes.capacity(), capacity);

            let exact = from_utf8(&filler[..remaining - 1]).unwrap();
            builder
                .header_with("X", |value| {
                    value
                        .write_str(exact)
                        .map_err(|_error| OutboundRequestMetaBuilder::over_cap())
                })
                .unwrap();
            assert_eq!(builder.written(), capacity);
            assert_eq!(builder.bytes.as_ptr(), pointer);
            assert_eq!(builder.bytes.capacity(), capacity);
            let meta = builder.finish();
            assert_eq!(meta.headers().collect::<Vec<_>>(), [("X", exact)]);
            assert_eq!(meta.method(), "POST");
        }
    }

    #[test]
    fn meta_builder_duplicate_method_or_url_setter_is_rejected() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, DEFAULT_MAX_IMPS);

        builder.set_method("POST").unwrap();
        builder.set_url("https://example.test/a").unwrap();

        assert_eq!(
            builder.set_method("GET").unwrap_err(),
            SinkAbort::DuplicateMeta {
                field: MetaField::Method
            }
        );
        assert_eq!(
            builder.set_url("https://example.test/b").unwrap_err(),
            SinkAbort::DuplicateMeta {
                field: MetaField::Url
            }
        );

        let meta = builder.finish();
        assert_eq!(meta.method(), "POST");
        assert_eq!(meta.url(), "https://example.test/a");
    }

    #[test]
    fn meta_builder_records_imp_ids_in_order_and_caps_the_table() {
        let mut bytes = pool();
        let mut builder = OutboundRequestMetaBuilder::new(&mut bytes, 3);

        for id in ["imp-1", "imp-2", "imp-3"] {
            builder.imp_id(id).unwrap();
        }
        assert_eq!(
            builder.imp_id("imp-4").unwrap_err(),
            SinkAbort::TooManyImpIds { limit: 3 }
        );

        let meta = builder.finish();
        assert_eq!(meta.imp_id_count(), 3);
        let ids: Vec<&str> = meta.covered_imp_ids().collect();
        assert_eq!(ids, ["imp-1", "imp-2", "imp-3"]);
    }
}

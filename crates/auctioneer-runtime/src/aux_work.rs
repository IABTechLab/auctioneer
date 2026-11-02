//! Checked peak work for one auxiliary request, independent of auction permits.
//!
//! Core must hold the receipt from before config extraction until terminal
//! egress. These are conservative allowances, not an allocator interceptor:
//! callers still check each input, node count, and intermediate before it
//! materializes, and the isolate equation must fund the derived peak.

use std::alloc::Layout;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::bounded_json::NODE_RETAINED_BYTES;
use crate::budget::{
    CONFIG_NODE_RETAINED_BYTES, CONFIG_PARSE_FACTOR, MAX_GVL_PARSED_BYTES,
    MIN_INBOUND_PARSE_FACTOR, PRIVACY_FIXED_BYTES,
};

pub const MAX_SYNC_BODY_BYTES: u64 = 32 * 1024;
pub const MAX_SYNC_QUERY_BYTES: u64 = 32 * 1024;
pub const MAX_SYNC_QUERY_PARAMETERS: u32 = 32;
pub const MAX_CATALOG_IDENTITIES: u32 = 512;
pub const MAX_SYNC_PRIORITY_GROUPS: u32 = 32;
pub const MAX_SYNC_PRIORITY_ENTRIES: u32 = 256;
pub const MAX_SYNC_REQUESTED_NAMES: u32 = 256;
pub const MAX_SYNC_FILTER_NAMES_PER_LIST: u32 = 256;
pub const MAX_SYNC_DEBUG_ROWS: u32 = 256;
pub const MAX_SYNC_URL_BYTES: u64 = 32 * 1024;
pub const MAX_SYNC_OUTPUT_BYTES: u64 = 256 * 1024;
pub const MAX_SYNC_RESPONSE_HEADER_BYTES: u64 = 16 * 1024;
pub const MAX_SYNC_RESPONSE_HEADER_COUNT: u32 = 16;
pub const MAX_SYNC_SET_COOKIE_BYTES: u64 = 4 * 1024;
pub const MAX_SYNC_KEYRING_BYTES: usize = 4 * 1024;
pub const MAX_SYNC_NAME_BYTES: u64 = 128;

const ENTRY_OVERHEAD_BYTES: u64 = 128;
const URL_STAGES: u64 = 3;
const PERCENT_EXPANSION: u64 = 3;
const JSON_ESCAPE_EXPANSION: u64 = 6;

/// Inputs supplied by the owning config/handler code, before any aux work.
/// Config uses the SDK value count; body/cookie node counts use byte allowances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxWorkLimits {
    pub config_envelope_bytes: u64,
    pub config_blob_bytes: u64,
    pub baked_config_bytes: u64,
    /// SDK whole-envelope JSON values, excluding object keys.
    pub config_json_values: u32,
    pub body_bytes: u64,
    /// Syntactic-node storage, at `NODE_RETAINED_BYTES` per node including keys.
    pub body_node_bytes: u64,
    pub query_bytes: u64,
    pub query_parameters: u32,
    pub admitted_header_bytes: u64,
    pub admitted_header_count: u32,
    /// Target-measured header-map entry overhead, shared with the isolate row.
    pub header_entry_overhead_bytes: u32,
    pub cookie_bytes: u64,
    /// Syntactic-node storage, independent of the config SDK value count.
    pub cookie_node_bytes: u64,
    pub privacy_input_bytes: u64,
    pub selected_tcf_bytes: u64,
    /// One distinct parsed list can remain live after cache eviction.
    pub gvl_parsed_bytes: u64,
    pub catalog_names: u32,
    pub priority_groups: u32,
    pub priority_entries: u32,
    pub requested_names: u32,
    pub filter_names_per_list: u32,
    pub debug_rows: u32,
    pub url_bytes: u64,
    pub output_body_bytes: u64,
    /// Response header bytes excluding the separately capped Set-Cookie.
    pub response_header_bytes: u64,
    pub response_header_count: u32,
    pub set_cookie_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxLimitField {
    BodyBytes,
    QueryBytes,
    QueryParameters,
    CatalogNames,
    PriorityGroups,
    PriorityEntries,
    RequestedNames,
    FilterNames,
    DebugRows,
    UrlBytes,
    OutputBodyBytes,
    SetCookieBytes,
    GvlParsedBytes,
    BodyNodes,
    CookieNodes,
    HeaderEntryOverheadBytes,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxBuffer {
    ConfigEnvelope,
    ConfigBlob,
    BakedConfig,
    Body,
    Query,
    DecodedQuery,
    AdmittedHeaders,
    Cookie,
    DecodedCookie,
    PrivacyInput,
    SelectedTcf,
    Url,
    RetainedUrls,
    OutputBody,
    ResponseHeaders,
    SetCookie,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxNodeSet {
    Config,
    Body,
    Cookie,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuxWorkError {
    InvalidLimit(AuxLimitField),
    ArithmeticOverflow,
    AllocationCapacity,
    Exhausted,
    BufferLimit(AuxBuffer),
    NodeLimit(AuxNodeSet),
}

impl fmt::Display for AuxWorkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimit(_) => "invalid auxiliary work limit",
            Self::ArithmeticOverflow => "auxiliary work capacity overflow",
            Self::AllocationCapacity => "auxiliary allocation is not representable",
            Self::Exhausted => "auxiliary work reservation exhausted",
            Self::BufferLimit(_) => "auxiliary buffer limit exceeded",
            Self::NodeLimit(_) => "auxiliary JSON node limit exceeded",
        })
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "remaining Error methods are deprecated or unstable on the pinned toolchain"
)]
impl Error for AuxWorkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// Conservative simultaneous peak terms, excluding separately pinned GVL
/// storage. Body and query coexist even when a route uses only one;
/// discarded/duplicate inputs still count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxWorkCharges {
    pub config: u64,
    pub body: u64,
    pub query: u64,
    pub input_headers: u64,
    pub cookie: u64,
    /// Accepted secret bytes, parsing, retained keys and fixed MAC scratch.
    pub signing: u64,
    /// Config SDK value storage plus body/cookie syntactic-node storage.
    pub nodes: u64,
    pub privacy: u64,
    pub chooser: u64,
    pub urls: u64,
    /// Candidate URL strings held while the bounded response is serialized.
    pub retained_urls: u64,
    pub output: u64,
}

impl AuxWorkCharges {
    /// # Errors
    /// Refuses an unrepresentable aggregate.
    pub fn total(&self) -> Result<u64, AuxWorkError> {
        [
            self.config,
            self.body,
            self.query,
            self.input_headers,
            self.cookie,
            self.signing,
            self.nodes,
            self.privacy,
            self.chooser,
            self.urls,
            self.retained_urls,
            self.output,
        ]
        .into_iter()
        .try_fold(0_u64, checked_add)
    }
}

/// Caps to pass to parsers and bounded materializers. Work is rejected at the
/// cap, rather than measured after allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxWorkCaps {
    /// SDK whole-envelope JSON values, excluding object keys.
    pub config_nodes: u32,
    pub body_nodes: u32,
    pub cookie_nodes: u32,
    pub query_decoded_bytes: u64,
    pub cookie_decoded_bytes: u64,
    pub url_bytes: u64,
    pub retained_urls_bytes: u64,
    pub output_body_bytes: u64,
    pub set_cookie_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxWorkDerived {
    charges: AuxWorkCharges,
    caps: AuxWorkCaps,
    peak_bytes: u64,
    pinned_gvl_bytes: u64,
    limits: AuxWorkLimits,
}

impl AuxWorkDerived {
    #[must_use]
    pub const fn charges(&self) -> AuxWorkCharges {
        self.charges
    }

    #[must_use]
    pub const fn caps(&self) -> AuxWorkCaps {
        self.caps
    }

    #[must_use]
    pub const fn peak_bytes(&self) -> u64 {
        self.peak_bytes
    }

    /// One distinct parsed GVL per live aux request, funded by the isolate's
    /// GVL population term rather than by `peak_bytes`.
    #[must_use]
    pub const fn pinned_gvl_bytes(&self) -> u64 {
        self.pinned_gvl_bytes
    }

    #[must_use]
    pub const fn limits(&self) -> AuxWorkLimits {
        self.limits
    }

    /// Total non-GVL aux-ledger capacity for the admitted aux population.
    /// # Errors
    /// Refuses an unrepresentable concurrency product.
    pub fn concurrent_peak(&self, permits: u32) -> Result<u64, AuxWorkError> {
        checked_mul(self.peak_bytes, u64::from(permits))
    }

    /// Reserve the entire simultaneous peak before config extraction.
    /// # Errors
    /// Refuses when the aux ledger has insufficient unreserved capacity.
    pub fn reserve<'a>(
        &'a self,
        budget: &'a AuxWorkBudget,
    ) -> Result<AuxWorkReservation<'a>, AuxWorkError> {
        budget
            .in_use
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                held.checked_add(self.peak_bytes)
                    .filter(|total| *total <= budget.limit)
            })
            .map_err(|_held| AuxWorkError::Exhausted)?;
        Ok(AuxWorkReservation {
            budget,
            derived: self,
        })
    }

    fn buffer_limit(&self, buffer: AuxBuffer) -> u64 {
        match buffer {
            AuxBuffer::ConfigEnvelope => self.limits.config_envelope_bytes,
            AuxBuffer::ConfigBlob => self.limits.config_blob_bytes,
            AuxBuffer::BakedConfig => self.limits.baked_config_bytes,
            AuxBuffer::Body => self.limits.body_bytes,
            AuxBuffer::Query => self.limits.query_bytes,
            AuxBuffer::DecodedQuery => self.caps.query_decoded_bytes,
            AuxBuffer::AdmittedHeaders => self.limits.admitted_header_bytes,
            AuxBuffer::Cookie => self.limits.cookie_bytes,
            AuxBuffer::DecodedCookie => self.caps.cookie_decoded_bytes,
            AuxBuffer::PrivacyInput => self.limits.privacy_input_bytes,
            AuxBuffer::SelectedTcf => self.limits.selected_tcf_bytes,
            AuxBuffer::Url => self.caps.url_bytes,
            AuxBuffer::RetainedUrls => self.caps.retained_urls_bytes,
            AuxBuffer::OutputBody => self.caps.output_body_bytes,
            AuxBuffer::ResponseHeaders => self.limits.response_header_bytes,
            AuxBuffer::SetCookie => self.caps.set_cookie_bytes,
        }
    }
}

/// Derive one request's simultaneously live work and all preallocation caps.
/// The fixed parse term funds cold privacy parsing. A distinct GVL held after
/// LRU eviction is exposed separately for the isolate's GVL population term;
/// it is not included in `peak_bytes` or `concurrent_peak`.
///
/// # Errors
/// Rejects invalid surface ceilings, unrepresentable node/allocation caps,
/// and checked-arithmetic overflow before returning any allowance.
#[expect(
    clippy::too_many_lines,
    reason = "keeping the checked simultaneous-work equation together makes each term auditable"
)]
pub fn derive_aux_work(limits: AuxWorkLimits) -> Result<AuxWorkDerived, AuxWorkError> {
    validate_limits(&limits)?;

    let config = checked_add(
        limits.config_envelope_bytes,
        checked_mul(
            checked_add(
                checked_add(limits.config_envelope_bytes, limits.config_blob_bytes)?,
                limits.baked_config_bytes,
            )?,
            CONFIG_PARSE_FACTOR,
        )?,
    )?;
    let body = checked_mul(limits.body_bytes, checked_add(1, MIN_INBOUND_PARSE_FACTOR)?)?;
    let query = checked_add(
        checked_mul(limits.query_bytes, 2)?,
        checked_mul(u64::from(limits.query_parameters), 3 * ENTRY_OVERHEAD_BYTES)?,
    )?;
    let input_headers = checked_add(
        limits.admitted_header_bytes,
        checked_mul(
            u64::from(limits.admitted_header_count),
            u64::from(limits.header_entry_overhead_bytes),
        )?,
    )?;
    // Raw JSON, escaped JSON and base64/envelope stages can coexist.
    let cookie = checked_mul(limits.cookie_bytes, 3)?;
    let signing = checked_add(
        checked_mul(
            u64::try_from(MAX_SYNC_KEYRING_BYTES)
                .map_err(|_error| AuxWorkError::ArithmeticOverflow)?,
            checked_add(1, CONFIG_PARSE_FACTOR)?,
        )?,
        checked_add(checked_mul(8, 32 + 32 + ENTRY_OVERHEAD_BYTES)?, 512)?,
    )?;
    let nodes = checked_add(
        checked_add(
            checked_mul(
                u64::from(limits.config_json_values),
                CONFIG_NODE_RETAINED_BYTES,
            )?,
            limits.body_node_bytes,
        )?,
        limits.cookie_node_bytes,
    )?;
    let privacy = checked_add(
        PRIVACY_FIXED_BYTES,
        checked_mul(
            checked_add(limits.privacy_input_bytes, limits.selected_tcf_bytes)?,
            2,
        )?,
    )?;
    let name_slots = checked_add(
        checked_add(
            u64::from(limits.catalog_names),
            u64::from(limits.requested_names),
        )?,
        checked_add(
            checked_mul(u64::from(limits.filter_names_per_list), 2)?,
            checked_add(
                u64::from(limits.priority_entries),
                u64::from(limits.debug_rows),
            )?,
        )?,
    )?;
    let chooser = checked_add(
        checked_mul(
            name_slots,
            checked_add(MAX_SYNC_NAME_BYTES, ENTRY_OVERHEAD_BYTES)?,
        )?,
        checked_mul(u64::from(limits.priority_groups), ENTRY_OVERHEAD_BYTES)?,
    )?;
    let urls = checked_mul(limits.url_bytes, URL_STAGES)?;
    let retained_urls = limits.output_body_bytes;
    let output = checked_add(
        checked_add(
            checked_mul(limits.output_body_bytes, 2)?,
            limits.response_header_bytes,
        )?,
        checked_add(
            checked_mul(
                u64::from(limits.response_header_count),
                u64::from(limits.header_entry_overhead_bytes),
            )?,
            limits.set_cookie_bytes,
        )?,
    )?;
    let charges = AuxWorkCharges {
        config,
        body,
        query,
        input_headers,
        cookie,
        signing,
        nodes,
        privacy,
        chooser,
        urls,
        retained_urls,
        output,
    };
    let peak_bytes = charges.total()?;

    // Aggregate charges are not individual allocations. These fields can
    // become one buffer each and must fit the target's layout first.
    for bytes in [
        limits.config_envelope_bytes,
        limits.config_blob_bytes,
        limits.baked_config_bytes,
        limits.body_bytes,
        limits.query_bytes,
        limits.admitted_header_bytes,
        limits.cookie_bytes,
        limits.privacy_input_bytes,
        limits.selected_tcf_bytes,
        limits.url_bytes,
        limits.output_body_bytes,
        limits.response_header_bytes,
        limits.set_cookie_bytes,
    ] {
        checked_buffer_capacity(bytes)?;
    }

    let caps = AuxWorkCaps {
        config_nodes: limits.config_json_values,
        body_nodes: node_cap(limits.body_node_bytes, AuxLimitField::BodyNodes)?,
        cookie_nodes: node_cap(limits.cookie_node_bytes, AuxLimitField::CookieNodes)?,
        query_decoded_bytes: limits.query_bytes,
        cookie_decoded_bytes: limits.cookie_bytes,
        url_bytes: limits.url_bytes,
        retained_urls_bytes: limits.output_body_bytes,
        output_body_bytes: limits.output_body_bytes,
        set_cookie_bytes: limits.set_cookie_bytes,
    };
    Ok(AuxWorkDerived {
        charges,
        caps,
        peak_bytes,
        pinned_gvl_bytes: limits.gvl_parsed_bytes,
        limits,
    })
}

fn validate_limits(limits: &AuxWorkLimits) -> Result<(), AuxWorkError> {
    for (bad, field) in [
        (
            limits.body_bytes > MAX_SYNC_BODY_BYTES,
            AuxLimitField::BodyBytes,
        ),
        (
            limits.query_bytes > MAX_SYNC_QUERY_BYTES,
            AuxLimitField::QueryBytes,
        ),
        (
            limits.query_parameters > MAX_SYNC_QUERY_PARAMETERS,
            AuxLimitField::QueryParameters,
        ),
        (
            limits.catalog_names > MAX_CATALOG_IDENTITIES,
            AuxLimitField::CatalogNames,
        ),
        (
            limits.priority_groups > MAX_SYNC_PRIORITY_GROUPS,
            AuxLimitField::PriorityGroups,
        ),
        (
            limits.priority_entries > MAX_SYNC_PRIORITY_ENTRIES,
            AuxLimitField::PriorityEntries,
        ),
        (
            limits.requested_names > MAX_SYNC_REQUESTED_NAMES,
            AuxLimitField::RequestedNames,
        ),
        (
            limits.filter_names_per_list > MAX_SYNC_FILTER_NAMES_PER_LIST,
            AuxLimitField::FilterNames,
        ),
        (
            limits.debug_rows > MAX_SYNC_DEBUG_ROWS,
            AuxLimitField::DebugRows,
        ),
        (
            limits.url_bytes > MAX_SYNC_URL_BYTES,
            AuxLimitField::UrlBytes,
        ),
        (
            limits.output_body_bytes > MAX_SYNC_OUTPUT_BYTES,
            AuxLimitField::OutputBodyBytes,
        ),
        (
            limits.set_cookie_bytes > MAX_SYNC_SET_COOKIE_BYTES,
            AuxLimitField::SetCookieBytes,
        ),
        (
            limits.gvl_parsed_bytes < MAX_GVL_PARSED_BYTES,
            AuxLimitField::GvlParsedBytes,
        ),
        (
            limits.header_entry_overhead_bytes == 0,
            AuxLimitField::HeaderEntryOverheadBytes,
        ),
    ] {
        if bad {
            return Err(AuxWorkError::InvalidLimit(field));
        }
    }
    Ok(())
}

#[expect(
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    reason = "floor division yields only the whole JSON nodes funded by the byte allowance"
)]
fn node_cap(bytes: u64, field: AuxLimitField) -> Result<u32, AuxWorkError> {
    u32::try_from(bytes / NODE_RETAINED_BYTES)
        .map_err(|_overflow| AuxWorkError::InvalidLimit(field))
}

fn checked_add(a: u64, b: u64) -> Result<u64, AuxWorkError> {
    a.checked_add(b).ok_or(AuxWorkError::ArithmeticOverflow)
}

fn checked_mul(a: u64, b: u64) -> Result<u64, AuxWorkError> {
    a.checked_mul(b).ok_or(AuxWorkError::ArithmeticOverflow)
}

fn checked_buffer_capacity(bytes: u64) -> Result<usize, AuxWorkError> {
    let capacity = usize::try_from(bytes).map_err(|_overflow| AuxWorkError::AllocationCapacity)?;
    Layout::array::<u8>(capacity).map_err(|_layout| AuxWorkError::AllocationCapacity)?;
    Ok(capacity)
}

/// A separate aux-work ledger, not an auction or admission permit.
#[derive(Debug)]
pub struct AuxWorkBudget {
    limit: u64,
    in_use: AtomicU64,
}

impl AuxWorkBudget {
    #[must_use]
    pub const fn new(limit: u64) -> Self {
        Self {
            limit,
            in_use: AtomicU64::new(0),
        }
    }

    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    #[must_use]
    pub fn in_use(&self) -> u64 {
        self.in_use.load(Ordering::Acquire)
    }
}

/// Non-cloneable receipt to hold through response completion or cancellation.
#[derive(Debug)]
#[must_use = "hold the auxiliary work reservation until its data is dropped"]
pub struct AuxWorkReservation<'a> {
    budget: &'a AuxWorkBudget,
    derived: &'a AuxWorkDerived,
}

impl AuxWorkReservation<'_> {
    /// Check a single buffer before copying, decoding, or allocating it.
    /// # Errors
    /// Refuses a buffer above its configured cap or target layout ceiling.
    pub fn ensure_buffer(&self, buffer: AuxBuffer, bytes: u64) -> Result<usize, AuxWorkError> {
        if bytes > self.derived.buffer_limit(buffer) {
            return Err(AuxWorkError::BufferLimit(buffer));
        }
        checked_buffer_capacity(bytes)
    }

    /// Check SDK values for config, or syntactic nodes for body/cookie.
    /// # Errors
    /// Refuses the first node above the derived parser ceiling.
    pub fn ensure_nodes(&self, source: AuxNodeSet, nodes: u32) -> Result<(), AuxWorkError> {
        let cap = match source {
            AuxNodeSet::Config => self.derived.caps.config_nodes,
            AuxNodeSet::Body => self.derived.caps.body_nodes,
            AuxNodeSet::Cookie => self.derived.caps.cookie_nodes,
        };
        if nodes > cap {
            return Err(AuxWorkError::NodeLimit(source));
        }
        Ok(())
    }

    /// Check one new URL and the cumulative retained URL bytes before append.
    /// # Errors
    /// Refuses arithmetic overflow, an oversized URL, or a retained set that
    /// would exceed the bounded response body allowance.
    pub fn ensure_retained_url_append(
        &self,
        retained_bytes: u64,
        next_url_bytes: u64,
    ) -> Result<usize, AuxWorkError> {
        let cumulative = checked_add(retained_bytes, next_url_bytes)?;
        self.ensure_buffer(AuxBuffer::Url, next_url_bytes)?;
        self.ensure_buffer(AuxBuffer::RetainedUrls, cumulative)
    }

    /// Bound worst-case percent expansion before constructing one URL stage.
    /// # Errors
    /// Refuses overflow or a stage above the URL cap.
    pub fn ensure_percent_encoded_url(&self, input_bytes: u64) -> Result<usize, AuxWorkError> {
        self.ensure_buffer(AuxBuffer::Url, checked_mul(input_bytes, PERCENT_EXPANSION)?)
    }

    /// Bound worst-case JSON string escaping before constructing output.
    /// # Errors
    /// Refuses overflow or a body above the serialized-output cap.
    pub fn ensure_json_escaped_output(&self, input_bytes: u64) -> Result<usize, AuxWorkError> {
        self.ensure_buffer(
            AuxBuffer::OutputBody,
            checked_mul(input_bytes, JSON_ESCAPE_EXPANSION)?,
        )
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for AuxWorkReservation<'_> {
    fn drop(&mut self) {
        self.budget
            .in_use
            .fetch_sub(self.derived.peak_bytes, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::decimal_literal_representation,
        reason = "resource charges are audited and reported as decimal byte counts"
    )]

    use crate::{
        AuctionBudget,
        aux_work::{
            AuxBuffer, AuxLimitField, AuxNodeSet, AuxWorkBudget, AuxWorkCharges, AuxWorkError,
            AuxWorkLimits, derive_aux_work,
        },
        bounded_json::NODE_RETAINED_BYTES,
        budget::MAX_GVL_PARSED_BYTES,
    };
    use futures::{FutureExt as _, future::pending};

    fn limits() -> AuxWorkLimits {
        AuxWorkLimits {
            config_envelope_bytes: 1_024,
            config_blob_bytes: 512,
            baked_config_bytes: 256,
            config_json_values: 100,
            body_bytes: 32 * 1_024,
            body_node_bytes: 200 * NODE_RETAINED_BYTES,
            query_bytes: 32 * 1_024,
            query_parameters: 32,
            admitted_header_bytes: 16 * 1_024,
            admitted_header_count: 128,
            header_entry_overhead_bytes: 64,
            cookie_bytes: 8 * 1_024,
            cookie_node_bytes: 50 * NODE_RETAINED_BYTES,
            privacy_input_bytes: 32 * 1_024,
            selected_tcf_bytes: 8 * 1_024,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            catalog_names: 256,
            priority_groups: 32,
            priority_entries: 256,
            requested_names: 256,
            filter_names_per_list: 256,
            debug_rows: 256,
            url_bytes: 32 * 1_024,
            output_body_bytes: 256 * 1_024,
            response_header_bytes: 16 * 1_024,
            response_header_count: 32,
            set_cookie_bytes: 4 * 1_024,
        }
    }

    #[test]
    fn catalog_identities_accept_511_512_and_refuse_513() {
        for count in [511, 512] {
            let mut bounds = limits();
            bounds.catalog_names = count;
            assert_eq!(
                derive_aux_work(bounds).unwrap().limits().catalog_names,
                count
            );
        }
        for count in [513, u32::MAX] {
            let mut bounds = limits();
            bounds.catalog_names = count;
            assert_eq!(
                derive_aux_work(bounds),
                Err(AuxWorkError::InvalidLimit(AuxLimitField::CatalogNames))
            );
        }
    }

    #[test]
    fn caller_priority_entries_accept_255_256_and_refuse_257() {
        for count in [255, 256] {
            let mut bounds = limits();
            bounds.priority_entries = count;
            assert_eq!(
                derive_aux_work(bounds).unwrap().limits().priority_entries,
                count
            );
        }
        for count in [257, u32::MAX] {
            let mut bounds = limits();
            bounds.priority_entries = count;
            assert_eq!(
                derive_aux_work(bounds),
                Err(AuxWorkError::InvalidLimit(AuxLimitField::PriorityEntries))
            );
        }
    }

    #[test]
    fn caller_requested_names_accept_255_256_and_refuse_257() {
        for count in [255, 256] {
            let mut bounds = limits();
            bounds.requested_names = count;
            assert_eq!(
                derive_aux_work(bounds).unwrap().limits().requested_names,
                count
            );
        }
        for count in [257, u32::MAX] {
            let mut bounds = limits();
            bounds.requested_names = count;
            assert_eq!(
                derive_aux_work(bounds),
                Err(AuxWorkError::InvalidLimit(AuxLimitField::RequestedNames))
            );
        }
    }

    #[test]
    fn caller_filter_names_per_list_accept_255_256_and_refuse_257() {
        for count in [255, 256] {
            let mut bounds = limits();
            bounds.filter_names_per_list = count;
            assert_eq!(
                derive_aux_work(bounds)
                    .unwrap()
                    .limits()
                    .filter_names_per_list,
                count
            );
        }
        for count in [257, u32::MAX] {
            let mut bounds = limits();
            bounds.filter_names_per_list = count;
            assert_eq!(
                derive_aux_work(bounds),
                Err(AuxWorkError::InvalidLimit(AuxLimitField::FilterNames))
            );
        }
    }

    #[test]
    fn caller_debug_rows_accept_255_256_and_refuse_257() {
        for count in [255, 256] {
            let mut bounds = limits();
            bounds.debug_rows = count;
            assert_eq!(derive_aux_work(bounds).unwrap().limits().debug_rows, count);
        }
        for count in [257, u32::MAX] {
            let mut bounds = limits();
            bounds.debug_rows = count;
            assert_eq!(
                derive_aux_work(bounds),
                Err(AuxWorkError::InvalidLimit(AuxLimitField::DebugRows))
            );
        }
    }

    #[test]
    fn priority_groups_accept_31_32_and_refuse_33() {
        assert_eq!(super::MAX_SYNC_PRIORITY_GROUPS, 32);
        for count in [31, 32] {
            let mut bounds = limits();
            bounds.priority_groups = count;
            assert_eq!(
                derive_aux_work(bounds).unwrap().limits().priority_groups,
                count
            );
        }
        for count in [33, u32::MAX] {
            let mut bounds = limits();
            bounds.priority_groups = count;
            assert_eq!(
                derive_aux_work(bounds),
                Err(AuxWorkError::InvalidLimit(AuxLimitField::PriorityGroups))
            );
        }
    }

    fn unequal_count_limits() -> AuxWorkLimits {
        AuxWorkLimits {
            catalog_names: 113,
            priority_groups: 3,
            priority_entries: 17,
            requested_names: 19,
            filter_names_per_list: 23,
            debug_rows: 29,
            ..limits()
        }
    }

    #[test]
    fn unequal_count_owners_retain_mapping_and_literal_chooser_charge() {
        let derived = derive_aux_work(unequal_count_limits()).unwrap();
        let bounds = derived.limits();
        assert_eq!(
            (
                bounds.catalog_names,
                bounds.priority_entries,
                bounds.requested_names,
                bounds.filter_names_per_list,
                bounds.debug_rows,
            ),
            (113, 17, 19, 23, 29)
        );
        assert_eq!(bounds.priority_groups, 3);
        assert_eq!(derived.charges().chooser, 57_728);
    }

    fn assert_chooser_summand(bounds: AuxWorkLimits, delta: u64) {
        let baseline = derive_aux_work(unequal_count_limits()).unwrap();
        let changed = derive_aux_work(bounds).unwrap();
        assert_eq!(baseline.charges().chooser, 57_728);
        assert_eq!(
            changed.charges(),
            AuxWorkCharges {
                chooser: 57_728_u64.checked_add(delta).unwrap(),
                ..baseline.charges()
            }
        );
        assert_eq!(
            changed
                .peak_bytes()
                .checked_sub(baseline.peak_bytes())
                .unwrap(),
            delta
        );
        assert_eq!(changed.caps(), baseline.caps());
        assert_eq!(changed.pinned_gvl_bytes(), baseline.pinned_gvl_bytes());
    }

    #[test]
    fn chooser_catalog_names_keep_their_accounting_summand() {
        let mut bounds = unequal_count_limits();
        bounds.catalog_names += 1;
        assert_chooser_summand(bounds, 256);
    }

    #[test]
    fn chooser_priority_groups_keep_their_accounting_summand() {
        let mut bounds = unequal_count_limits();
        bounds.priority_groups += 1;
        assert_chooser_summand(bounds, 128);
    }

    #[test]
    fn chooser_priority_entries_keep_their_accounting_summand() {
        let mut bounds = unequal_count_limits();
        bounds.priority_entries += 1;
        assert_chooser_summand(bounds, 256);
    }

    #[test]
    fn chooser_requested_names_keep_their_accounting_summand() {
        let mut bounds = unequal_count_limits();
        bounds.requested_names += 1;
        assert_chooser_summand(bounds, 256);
    }

    #[test]
    fn chooser_both_filter_lists_keep_their_accounting_summands() {
        let mut bounds = unequal_count_limits();
        bounds.filter_names_per_list += 1;
        assert_chooser_summand(bounds, 512);
    }

    #[test]
    fn chooser_debug_rows_keep_their_accounting_summand() {
        let mut bounds = unequal_count_limits();
        bounds.debug_rows += 1;
        assert_chooser_summand(bounds, 256);
    }

    #[test]
    fn catalog_growth_only_adds_65536_chooser_bytes() {
        let baseline = derive_aux_work(limits()).unwrap();
        let mut bounds = limits();
        bounds.catalog_names = 512;
        let expanded = derive_aux_work(bounds).unwrap();
        assert_eq!(baseline.charges().chooser, 397_312);
        assert_eq!(
            expanded.charges(),
            AuxWorkCharges {
                chooser: 462_848,
                ..baseline.charges()
            }
        );
        assert_eq!(expanded.peak_bytes() - baseline.peak_bytes(), 65_536);
        assert_eq!(expanded.caps(), baseline.caps());
        assert_eq!(expanded.pinned_gvl_bytes(), baseline.pinned_gvl_bytes());

        let unfunded = AuxWorkBudget::new(baseline.peak_bytes());
        assert_eq!(
            expanded.reserve(&unfunded).unwrap_err(),
            AuxWorkError::Exhausted
        );
        assert_eq!(unfunded.in_use(), 0);
        let funded = AuxWorkBudget::new(expanded.peak_bytes());
        let receipt = expanded.reserve(&funded).unwrap();
        assert_eq!(funded.in_use(), baseline.peak_bytes() + 65_536);
        drop(receipt);
        assert_eq!(funded.in_use(), 0);
    }

    #[test]
    fn full_catalog_is_funded_with_output_limit_one() {
        let mut bounds = limits();
        bounds.catalog_names = 512;
        bounds.output_body_bytes = 1;
        let derived = derive_aux_work(bounds).unwrap();
        assert_eq!(derived.limits().catalog_names, 512);
        assert_eq!(derived.charges().chooser, 462_848);
        assert_eq!(derived.charges().retained_urls, 1);
        let budget = AuxWorkBudget::new(derived.peak_bytes());
        let receipt = derived.reserve(&budget).unwrap();
        assert_eq!(receipt.ensure_buffer(AuxBuffer::OutputBody, 1), Ok(1));
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::OutputBody, 2),
            Err(AuxWorkError::BufferLimit(AuxBuffer::OutputBody))
        );
    }

    #[test]
    fn aux_charge_total_retains_every_independent_summand() {
        let charges = AuxWorkCharges {
            config: 2,
            body: 3,
            query: 5,
            input_headers: 7,
            cookie: 11,
            signing: 13,
            nodes: 17,
            privacy: 19,
            chooser: 23,
            urls: 29,
            retained_urls: 31,
            output: 37,
        };
        assert_eq!(charges.total(), Ok(197));
        assert_eq!(
            AuxWorkCharges {
                output: u64::MAX,
                ..charges
            }
            .total(),
            Err(AuxWorkError::ArithmeticOverflow)
        );
    }

    #[test]
    fn peak_sums_simultaneously_live_config_inputs_privacy_chooser_urls_and_output() {
        let derived = derive_aux_work(limits()).unwrap();
        let charges = derived.charges();
        assert_eq!(
            charges,
            AuxWorkCharges {
                config: 230_400,
                body: 1_212_416,
                query: 77_824,
                input_headers: 24_576,
                cookie: 24_576,
                signing: 530_432,
                nodes: 100 * 623 + (200 + 50) * 384,
                privacy: 10_567_680,
                chooser: 397_312,
                urls: 98_304,
                retained_urls: 262_144,
                output: 546_816,
            }
        );
        assert_eq!(charges.total().unwrap(), derived.peak_bytes());
        assert_eq!(derived.peak_bytes(), 14_130_780);
        assert_eq!(derived.pinned_gvl_bytes(), 512_000);
        assert_eq!(derived.caps().config_nodes, 100);
        assert_eq!(derived.caps().body_nodes, 200);
        assert_eq!(derived.caps().cookie_nodes, 50);
        assert_eq!(derived.caps().query_decoded_bytes, 32 * 1_024);
    }

    #[test]
    fn body_and_cookie_syntactic_node_bytes_do_not_change_config_values() {
        let baseline = derive_aux_work(limits()).unwrap();
        for (body_delta, cookie_delta) in [(384, 0), (0, 384)] {
            let mut inputs = limits();
            inputs.body_node_bytes += body_delta;
            inputs.cookie_node_bytes += cookie_delta;
            let work = derive_aux_work(inputs).unwrap();
            assert_eq!(work.charges().nodes - baseline.charges().nodes, 384);
            assert_eq!(work.peak_bytes() - baseline.peak_bytes(), 384);
            assert_eq!(work.charges().config, baseline.charges().config);
            assert_eq!(work.caps().config_nodes, baseline.caps().config_nodes);
            assert_eq!(work.caps().body_nodes, 200 + u32::from(body_delta != 0));
            assert_eq!(work.caps().cookie_nodes, 50 + u32::from(cookie_delta != 0));
        }

        for (body_delta, cookie_delta, expected) in [(1, 0, 37), (0, 1, 3)] {
            let mut inputs = limits();
            inputs.body_bytes -= body_delta;
            inputs.cookie_bytes -= cookie_delta;
            let work = derive_aux_work(inputs).unwrap();
            assert_eq!(baseline.peak_bytes() - work.peak_bytes(), expected);
            assert_eq!(work.charges().nodes, baseline.charges().nodes);
            assert_eq!(work.charges().config, baseline.charges().config);
            assert_eq!(
                work.caps(),
                super::AuxWorkCaps {
                    cookie_decoded_bytes: inputs.cookie_bytes,
                    ..baseline.caps()
                }
            );
        }
    }

    #[test]
    fn complete_cookie_cap_changes_output_charge_and_peak_by_exact_bytes() {
        let mut bounds = limits();
        bounds.set_cookie_bytes = 1_024;
        let smaller = derive_aux_work(bounds).unwrap();
        bounds.set_cookie_bytes = 4_096;
        let larger = derive_aux_work(bounds).unwrap();
        assert_eq!(smaller.caps().set_cookie_bytes, 1_024);
        assert_eq!(larger.caps().set_cookie_bytes, 4_096);
        assert_eq!(larger.charges().output - smaller.charges().output, 3_072);
        assert_eq!(larger.peak_bytes() - smaller.peak_bytes(), 3_072);
        assert_eq!(larger.charges().signing, smaller.charges().signing);

        let budget = AuxWorkBudget::new(smaller.peak_bytes());
        let receipt = smaller.reserve(&budget).unwrap();
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::SetCookie, 1_024),
            Ok(1_024)
        );
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::SetCookie, 1_025),
            Err(AuxWorkError::BufferLimit(AuxBuffer::SetCookie))
        );
    }

    #[test]
    fn signing_charge_funds_keyring_parsing_retained_keys_and_fixed_mac_scratch() {
        // 4096 backend bytes + 4096*128 parsing + 8*(32 key + 32 id + 128 entry)
        // + 512 MAC scratch, independent of incoming and outgoing cookie sizes.
        let baseline = derive_aux_work(limits()).unwrap();
        assert_eq!(baseline.charges().signing, 530_432);
        let mut bounds = limits();
        bounds.cookie_bytes = 1_024;
        bounds.set_cookie_bytes = 512;
        let smaller = derive_aux_work(bounds).unwrap();
        assert_eq!(smaller.charges().signing, 530_432);
        assert_eq!(baseline.charges().cookie - smaller.charges().cookie, 21_504);
        assert_eq!(baseline.charges().output - smaller.charges().output, 3_584);
        assert_eq!(baseline.peak_bytes() - smaller.peak_bytes(), 25_088);
    }

    #[test]
    fn full_peak_is_reserved_before_work_and_released_only_with_the_receipt() {
        let derived = derive_aux_work(limits()).unwrap();
        let budget = AuxWorkBudget::new(derived.peak_bytes() * 2);
        let auction = AuctionBudget::new(1);
        let first = derived.reserve(&budget).unwrap();
        let second = derived.reserve(&budget).unwrap();
        assert_eq!(budget.in_use(), derived.peak_bytes() * 2);
        assert_eq!(auction.in_use(), 0);
        assert_eq!(
            derived.reserve(&budget).unwrap_err(),
            AuxWorkError::Exhausted
        );
        drop(first);
        assert_eq!(budget.in_use(), derived.peak_bytes());
        drop(second);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn cancelling_suspended_aux_work_releases_its_memory_receipt() {
        let derived = derive_aux_work(limits()).unwrap();
        let budget = AuxWorkBudget::new(derived.peak_bytes());
        let work = async {
            let _receipt = derived.reserve(&budget).unwrap();
            assert_eq!(budget.in_use(), derived.peak_bytes());
            pending::<()>().await;
        };
        assert!(work.now_or_never().is_none());
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn pinned_gvl_is_disjoint_from_aux_peak_and_scales_with_permits() {
        let baseline = derive_aux_work(limits()).unwrap();
        let mut larger_limits = limits();
        larger_limits.gvl_parsed_bytes += 64 * 1_024;
        let larger = derive_aux_work(larger_limits).unwrap();
        assert_eq!(
            larger.pinned_gvl_bytes() - baseline.pinned_gvl_bytes(),
            65_536
        );
        assert_eq!(larger.charges().privacy, baseline.charges().privacy);
        assert_eq!(larger.peak_bytes(), baseline.peak_bytes());
        assert_eq!(larger.pinned_gvl_bytes() * 2, 1_155_072);

        let budget = AuxWorkBudget::new(larger.peak_bytes() * 2);
        let evicted_but_held = larger.reserve(&budget).unwrap();
        let current = larger.reserve(&budget).unwrap();
        assert_eq!(budget.in_use(), 2 * larger.peak_bytes());
        drop(current);
        assert_eq!(budget.in_use(), larger.peak_bytes());
        drop(evicted_but_held);
        assert_eq!(budget.in_use(), 0);
    }

    #[test]
    fn concurrent_aux_capacity_is_checked_before_ledger_construction() {
        let derived = derive_aux_work(limits()).unwrap();
        assert_eq!(derived.concurrent_peak(16), Ok(226_092_480));

        let mut enormous_nodes = limits();
        enormous_nodes.config_json_values = u32::MAX;
        let large_work = derive_aux_work(enormous_nodes).unwrap();
        assert_eq!(
            large_work.concurrent_peak(u32::MAX),
            Err(AuxWorkError::ArithmeticOverflow)
        );
    }

    #[test]
    fn target_header_entry_overhead_scales_both_live_header_maps() {
        let baseline = derive_aux_work(limits()).unwrap();
        let mut target = limits();
        target.header_entry_overhead_bytes += 16;
        let larger = derive_aux_work(target).unwrap();
        assert_eq!(
            larger.charges().input_headers - baseline.charges().input_headers,
            2_048
        );
        assert_eq!(larger.charges().output - baseline.charges().output, 512);
        assert_eq!(larger.peak_bytes() - baseline.peak_bytes(), 2_560);
    }

    #[test]
    fn reduced_config_gvl_input_and_output_allowances_refuse_before_materialization() {
        let baseline = limits();
        let mut work_started = false;
        let too_small = AuxWorkBudget::new(derive_aux_work(baseline).unwrap().peak_bytes() - 1);
        let derived = derive_aux_work(baseline).unwrap();
        let refused = derived.reserve(&too_small);
        if refused.is_ok() {
            work_started = true;
        }
        assert_eq!(refused.unwrap_err(), AuxWorkError::Exhausted);
        assert!(!work_started);

        let mut smaller_gvl = baseline;
        smaller_gvl.gvl_parsed_bytes = MAX_GVL_PARSED_BYTES - 1;
        assert_eq!(
            derive_aux_work(smaller_gvl).unwrap_err(),
            AuxWorkError::InvalidLimit(AuxLimitField::GvlParsedBytes)
        );

        let mut smaller_config = baseline;
        smaller_config.config_blob_bytes = 128;
        let config_work = derive_aux_work(smaller_config).unwrap();
        let config_budget = AuxWorkBudget::new(config_work.peak_bytes());
        let config_receipt = config_work.reserve(&config_budget).unwrap();
        assert_eq!(
            config_receipt
                .ensure_buffer(AuxBuffer::ConfigBlob, 129)
                .unwrap_err(),
            AuxWorkError::BufferLimit(AuxBuffer::ConfigBlob)
        );

        let mut smaller_input = baseline;
        smaller_input.query_bytes = 16;
        let input_work = derive_aux_work(smaller_input).unwrap();
        let input_budget = AuxWorkBudget::new(input_work.peak_bytes());
        let input_receipt = input_work.reserve(&input_budget).unwrap();
        assert_eq!(
            input_receipt
                .ensure_buffer(AuxBuffer::Query, 17)
                .unwrap_err(),
            AuxWorkError::BufferLimit(AuxBuffer::Query)
        );

        let mut smaller_output = baseline;
        smaller_output.output_body_bytes = 64;
        let output_work = derive_aux_work(smaller_output).unwrap();
        let output_budget = AuxWorkBudget::new(output_work.peak_bytes());
        let output_receipt = output_work.reserve(&output_budget).unwrap();
        assert_eq!(
            output_receipt
                .ensure_buffer(AuxBuffer::OutputBody, 65)
                .unwrap_err(),
            AuxWorkError::BufferLimit(AuxBuffer::OutputBody)
        );
    }

    #[test]
    fn node_and_escaped_intermediate_caps_reject_before_allocation() {
        let derived = derive_aux_work(limits()).unwrap();
        let budget = AuxWorkBudget::new(derived.peak_bytes());
        let receipt = derived.reserve(&budget).unwrap();
        assert_eq!(receipt.ensure_nodes(AuxNodeSet::Body, 200), Ok(()));
        assert_eq!(
            receipt.ensure_nodes(AuxNodeSet::Body, 201),
            Err(AuxWorkError::NodeLimit(AuxNodeSet::Body))
        );
        assert_eq!(receipt.ensure_percent_encoded_url(10_922), Ok(32_766));
        assert_eq!(
            receipt.ensure_percent_encoded_url(10_923),
            Err(AuxWorkError::BufferLimit(AuxBuffer::Url))
        );
        assert_eq!(receipt.ensure_json_escaped_output(43_690), Ok(262_140));
        assert_eq!(
            receipt.ensure_json_escaped_output(43_691),
            Err(AuxWorkError::BufferLimit(AuxBuffer::OutputBody))
        );
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::SetCookie, 4_096),
            Ok(4_096)
        );
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::SetCookie, 4_097),
            Err(AuxWorkError::BufferLimit(AuxBuffer::SetCookie))
        );
    }

    #[test]
    fn retained_candidate_urls_cannot_outgrow_bounded_output_before_serialization() {
        let derived = derive_aux_work(limits()).unwrap();
        assert_eq!(derived.charges().urls, 98_304);
        assert_eq!(derived.charges().retained_urls, 262_144);
        let budget = AuxWorkBudget::new(derived.peak_bytes());
        let receipt = derived.reserve(&budget).unwrap();
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::RetainedUrls, 262_144),
            Ok(262_144)
        );
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::RetainedUrls, 262_145),
            Err(AuxWorkError::BufferLimit(AuxBuffer::RetainedUrls))
        );
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::RetainedUrls, 256 * 32 * 1_024),
            Err(AuxWorkError::BufferLimit(AuxBuffer::RetainedUrls))
        );
        assert_eq!(
            receipt.ensure_retained_url_append(7 * 32 * 1_024, 32 * 1_024),
            Ok(262_144)
        );
        assert_eq!(
            receipt.ensure_retained_url_append(8 * 32 * 1_024, 32 * 1_024),
            Err(AuxWorkError::BufferLimit(AuxBuffer::RetainedUrls))
        );
        assert_eq!(
            receipt.ensure_retained_url_append(u64::MAX, 1),
            Err(AuxWorkError::ArithmeticOverflow)
        );
    }

    #[test]
    fn copied_caps_cannot_widen_a_live_aux_reservation() {
        let derived = derive_aux_work(limits()).unwrap();
        let mut copied_caps = derived.caps();
        copied_caps.output_body_bytes = u64::MAX;
        assert_eq!(copied_caps.output_body_bytes, u64::MAX);
        assert_eq!(derived.caps().output_body_bytes, 262_144);
        let budget = AuxWorkBudget::new(derived.peak_bytes());
        let receipt = derived.reserve(&budget).unwrap();
        assert_eq!(
            receipt.ensure_buffer(AuxBuffer::OutputBody, 262_145),
            Err(AuxWorkError::BufferLimit(AuxBuffer::OutputBody))
        );
    }

    #[test]
    #[expect(clippy::unreachable, reason = "case ids are fixed by this test table")]
    fn sync_surface_ceilings_fail_closed_at_derivation() {
        for (field, change) in [
            (AuxLimitField::BodyBytes, 0_u8),
            (AuxLimitField::QueryBytes, 1),
            (AuxLimitField::QueryParameters, 2),
            (AuxLimitField::CatalogNames, 3),
            (AuxLimitField::PriorityGroups, 4),
            (AuxLimitField::PriorityEntries, 5),
            (AuxLimitField::RequestedNames, 6),
            (AuxLimitField::FilterNames, 7),
            (AuxLimitField::UrlBytes, 8),
            (AuxLimitField::OutputBodyBytes, 9),
            (AuxLimitField::SetCookieBytes, 10),
        ] {
            let mut value = limits();
            match change {
                0 => value.body_bytes += 1,
                1 => value.query_bytes += 1,
                2 => value.query_parameters += 1,
                3 => value.catalog_names = 513,
                4 => value.priority_groups += 1,
                5 => value.priority_entries += 1,
                6 => value.requested_names += 1,
                7 => value.filter_names_per_list += 1,
                8 => value.url_bytes += 1,
                9 => value.output_body_bytes += 1,
                10 => value.set_cookie_bytes += 1,
                _ => unreachable!(),
            }
            assert_eq!(
                derive_aux_work(value).unwrap_err(),
                AuxWorkError::InvalidLimit(field)
            );
        }
    }

    #[test]
    fn checked_arithmetic_and_buffer_layout_reject_unrepresentable_work() {
        let mut overflow = limits();
        overflow.baked_config_bytes = u64::MAX;
        assert_eq!(
            derive_aux_work(overflow).unwrap_err(),
            AuxWorkError::ArithmeticOverflow
        );
        let mut unallocatable = limits();
        unallocatable.response_header_bytes = u64::try_from(isize::MAX).unwrap() + 1;
        assert_eq!(
            derive_aux_work(unallocatable).unwrap_err(),
            AuxWorkError::AllocationCapacity
        );
    }
}

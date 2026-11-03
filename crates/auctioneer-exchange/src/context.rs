//! `AuctionContext`.

use crate::{account::ResolvedAccount, fanout::BidderFanout};
use auctioneer_bidder::{
    BidderName, BidderRegistry, EntryPoint, FloorConversion, request_info::BidderSettingsSource,
};
use auctioneer_config::types::{BidderImplementationBinding, HostConfig};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_privacy::gcl::Gcl;
use auctioneer_privacy::gvl::GvlStore;
use auctioneer_runtime::bidder_credentials::BidderCredentialSource;
use auctioneer_runtime::bounded_json::JsonFieldShape;
use auctioneer_runtime::json_preflight::{MAX_CONSTRUCTION_DEPTH, preflight};
use auctioneer_runtime::{Clock, MonotonicInstant, Reservation, Rng, serialized_json_bytes};
use std::collections::BTreeMap;
use std::sync::OnceLock;
use time::OffsetDateTime;

/// Runtime feature toggles resolved by the caller from
/// `auctioneer.toml` `[features]` before the auction starts.
///
/// `run_auction` reads these instead of hard-coding behavior. Adding
/// a knob here is the seam that lets `auctioneer config push` genuinely
/// change exchange behavior. Defaults retain the baseline exchange policy.
///
/// **Non-USD response bids are unconditionally dropped** — the exchange
/// has no response-price conversion path, and admitting mixed-currency bids would either
/// mis-label the response `cur` as USD or force a per-seat currency
/// column that the response does not carry. Request-floor conversion does not
/// change this response-price policy.
#[derive(Clone, Copy, Debug)]
pub struct ExchangeFeatures {
    /// Send outbound `Content-Encoding: gzip` when a bidder declares
    /// `endpoint_compression = "gzip"` in its `bidder.toml`. `false` = strip the
    /// compression setting before dispatch (send plaintext).
    pub gzip_outbound: bool,
    /// Default value of `returnallbidstatus` when the request omits
    /// `ext.prebid.returnallbidstatus`. Baseline is `false`.
    pub returnallbidstatus_default: bool,
}

impl Default for ExchangeFeatures {
    fn default() -> Self {
        Self {
            gzip_outbound: true,
            returnallbidstatus_default: false,
        }
    }
}

/// Glue passed to `run_auction`. `#[non_exhaustive]` so external callers
/// MUST use `with_request_start` (which enforces the `request_start`
/// anchor invariant). In-crate tests construct via field literals
/// directly.
#[non_exhaustive]
pub struct AuctionContext<'a> {
    pub request_start: MonotonicInstant,
    /// Trusted caller entrypoint; never inferred from payload fields or media.
    /// Current HTTP auction handling retains the constructor's `Auction` default.
    pub entry_point: EntryPoint,
    pub host: &'a HostConfig,
    pub account: &'a ResolvedAccount<'a>,
    pub bidders: &'a BidderRegistry,
    pub fanout: &'a dyn BidderFanout,
    pub clock: &'a dyn Clock,
    pub rng: &'a dyn Rng,
    /// The versioned GVL archive the privacy resolver picks the exact
    /// `(spec, list)` pair from, named by the request's TC string.
    pub gvl_store: &'a GvlStore,
    /// The baked Global CMP List, used to authenticate the CMP the TC
    /// string names and to detect a stale bundled list.
    pub gcl: &'a Gcl,
    /// The handler-captured wall-clock instant. A VALUE, not a borrow:
    /// resolve-time and permission-time checks share this single
    /// reading so one request can never straddle two instants.
    pub now: OffsetDateTime,
    /// Caller-side debug request (for example query/header normalization).
    /// The debug permission policy independently ORs normalized `test=1` and
    /// `ext.prebid.debug=1/true`; false here cannot cancel a body signal and true
    /// cannot bypass account/base-bidder gates. The engine owns bounded sanitized
    /// capture and response attachment; callers must not inject transport traces.
    pub debug_requested: bool,
    /// Runtime feature toggles resolved from `auctioneer.toml`. `None`
    /// (via `with_request_start`) → `ExchangeFeatures::default()`.
    pub features: ExchangeFeatures,
    /// Per-bidder timeout overrides sourced from
    /// `[bidders.<name>] timeout_ms`. Keyed by bidder name; empty
    /// map means "use baked `BidderInfo.bidder_timeout_ms`".
    pub bidder_timeout_override: &'a BTreeMap<BidderName, u32>,
    /// Held by the caller through serialization. Direct engine tests may omit
    /// it; the booted HTTP handler always supplies its prepaid projection receipt.
    pub projection_reservation: Option<&'a Reservation<'a>>,
    /// Funds diagnostics, decoded responses, targeting and the unconditional
    /// prepaid debug peak. Ordinary response work cannot borrow that allowance.
    pub response_reservation: Option<&'a Reservation<'a>>,
    /// Installed sequential schema reservation. Zero keeps controlled-native
    /// flows closed; it is not inferred from a compiled factory or schema.
    pub schema_scratch_bytes: u64,
    /// Boot-funded normalized source allowance. Programmatic callers that omit
    /// this must admit their owned input's byte/node width before normalization.
    /// Borrowed depth admission still runs even when no receipt is installed.
    pub construction_limits: Option<RequestConstructionLimits>,
    /// Original root-extension provenance, independent of normalized or
    /// privacy-transformed request contents. Typed-only construction is unknown.
    pub original_request_ext_shape: JsonFieldShape,
    /// The extracted config owner supplies borrowed protocol settings only.
    pub bidder_settings: Option<&'a dyn BidderSettingsSource>,
    /// Borrowed operator snapshot for request-floor conversion, never bid prices.
    pub floor_currency: FloorConversion<'a>,
    /// Request-bound host service; never consulted for a refused bidder.
    pub credential_source: Option<&'a dyn BidderCredentialSource>,
    /// Independent compiled public-name bindings; sorted by public bidder name.
    /// Embedders supplying their own registry must supply its matching bindings.
    pub bidder_bindings: &'a [BidderImplementationBinding],
}

/// Boot-funded source, retained growth and scratch checked before projection.
#[derive(Clone, Copy, Debug)]
pub struct RequestConstructionLimits {
    pub max_nodes: u64,
    pub max_serialized_bytes: u64,
    /// Installed reusable scrub scratch sharing the prepaid projection receipt.
    pub reusable_scratch_bytes: u64,
    /// Installed destination growth retained for each survivor scope.
    pub retained_per_scope_bytes: u64,
}

impl RequestConstructionLimits {
    /// Admit borrowed data without allocating an output or mutating the caller.
    ///
    /// # Errors
    /// Returns a static construction error on depth, width or arithmetic refusal.
    pub fn ensure(&self, request: &BidRequest) -> Result<(), crate::AuctionError> {
        let refuse = || crate::AuctionError::Internal("request construction limit exceeded".into());
        preflight(request, self.max_nodes, MAX_CONSTRUCTION_DEPTH).map_err(|_error| refuse())?;
        let bytes = serialized_json_bytes(request).map_err(|_error| refuse())?;
        if bytes > self.max_serialized_bytes {
            return Err(refuse());
        }
        Ok(())
    }

    pub(crate) fn with_supplement(
        self,
        nodes: u32,
        bytes: u64,
    ) -> Result<Self, crate::AuctionError> {
        Ok(Self {
            max_nodes: self
                .max_nodes
                .checked_add(u64::from(nodes))
                .ok_or(auctioneer_runtime::ResourceError::CapacityOverflow)?,
            max_serialized_bytes: self
                .max_serialized_bytes
                .checked_add(bytes)
                .ok_or(auctioneer_runtime::ResourceError::CapacityOverflow)?,
            reusable_scratch_bytes: self.reusable_scratch_bytes,
            retained_per_scope_bytes: self.retained_per_scope_bytes,
        })
    }
}

impl<'a> AuctionContext<'a> {
    /// Construct an `AuctionContext` anchored at the wall-clock instant
    /// the request hit the handler. This is the only constructor — the
    /// `request_start` value MUST be captured as the first action in
    /// the request path.
    ///
    /// Features and per-bidder timeout override default to the empty
    /// / baseline values so existing call sites (tests, alternate
    /// drivers) compile unchanged. Use `with_request_start_ext` to
    /// pass explicit values.
    #[expect(
        clippy::too_many_arguments,
        reason = "explicit constructor enforces the request_start anchor invariant"
    )]
    pub fn with_request_start(
        request_start: MonotonicInstant,
        host: &'a HostConfig,
        account: &'a ResolvedAccount<'a>,
        bidders: &'a BidderRegistry,
        fanout: &'a dyn BidderFanout,
        clock: &'a dyn Clock,
        rng: &'a dyn Rng,
        gvl_store: &'a GvlStore,
        gcl: &'a Gcl,
        now: OffsetDateTime,
        debug_requested: bool,
    ) -> Self {
        static EMPTY_OVERRIDES: OnceLock<BTreeMap<BidderName, u32>> = OnceLock::new();
        Self {
            request_start,
            entry_point: EntryPoint::Auction,
            host,
            account,
            bidders,
            fanout,
            clock,
            rng,
            gvl_store,
            gcl,
            now,
            debug_requested,
            features: ExchangeFeatures::default(),
            bidder_timeout_override: EMPTY_OVERRIDES.get_or_init(BTreeMap::new),
            projection_reservation: None,
            response_reservation: None,
            schema_scratch_bytes: 0,
            construction_limits: None,
            original_request_ext_shape: JsonFieldShape::Unavailable,
            bidder_settings: None,
            floor_currency: FloorConversion::default(),
            credential_source: None,
            bidder_bindings: auctioneer_config::bidder_implementation_bindings(),
        }
    }

    /// Full-fat constructor — same as `with_request_start` plus the
    /// resolved `ExchangeFeatures` and per-bidder timeout override
    /// map. `auctioneer_core::handlers::auction` uses this after
    /// `resolved_host` / `resolved_account` and `resolved_features`
    /// merge the pushed `auctioneer.toml` blob over baked defaults.
    #[expect(
        clippy::too_many_arguments,
        reason = "explicit constructor mirrors `with_request_start` shape"
    )]
    pub fn with_request_start_ext(
        request_start: MonotonicInstant,
        host: &'a HostConfig,
        account: &'a ResolvedAccount<'a>,
        bidders: &'a BidderRegistry,
        fanout: &'a dyn BidderFanout,
        clock: &'a dyn Clock,
        rng: &'a dyn Rng,
        gvl_store: &'a GvlStore,
        gcl: &'a Gcl,
        now: OffsetDateTime,
        debug_requested: bool,
        features: ExchangeFeatures,
        bidder_timeout_override: &'a BTreeMap<BidderName, u32>,
    ) -> Self {
        Self {
            request_start,
            entry_point: EntryPoint::Auction,
            host,
            account,
            bidders,
            fanout,
            clock,
            rng,
            gvl_store,
            gcl,
            now,
            debug_requested,
            features,
            bidder_timeout_override,
            projection_reservation: None,
            response_reservation: None,
            schema_scratch_bytes: 0,
            construction_limits: None,
            original_request_ext_shape: JsonFieldShape::Unavailable,
            bidder_settings: None,
            floor_currency: FloorConversion::default(),
            credential_source: None,
            bidder_bindings: auctioneer_config::bidder_implementation_bindings(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RequestConstructionLimits;

    #[test]
    fn supplement_allowance_preserves_installed_construction_costs() {
        let limits = RequestConstructionLimits {
            max_nodes: 1,
            max_serialized_bytes: 2,
            reusable_scratch_bytes: 17,
            retained_per_scope_bytes: 19,
        }
        .with_supplement(3, 4)
        .unwrap();
        assert_eq!(limits.max_nodes, 4);
        assert_eq!(limits.max_serialized_bytes, 6);
        assert_eq!(limits.reusable_scratch_bytes, 17);
        assert_eq!(limits.retained_per_scope_bytes, 19);
    }
}

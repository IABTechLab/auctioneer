//! Context passed to `Bidder::make_requests`.

use crate::BidderSettings;
use crate::bidder_settings::ensure_limits as ensure_settings_limits;
use crate::{
    bidder::{BidderError, EntryPoint},
    currency::FloorConversion,
    types::BidderName,
};
use auctioneer_runtime::bidder_credentials::BidderCredentials;
use auctioneer_runtime::bounded_json::JsonFieldShape;

/// Borrow runtime settings from their existing configuration owner.
pub trait BidderSettingsSource {
    fn settings(&self, name: &str) -> Option<&BidderSettings>;
}

/// Nonempty views can only be installed through the checked binding method.
#[derive(Default)]
pub struct BidderSettingsView<'req> {
    requested: Option<&'req BidderSettings>,
    base: Option<&'req BidderSettings>,
}

/// `#[non_exhaustive]` so adding fields later is non-breaking — fields
/// intentionally omitted in phase 1 but reserved for later.
///
/// Field order: identity -> account -> publisher -> endpoint ->
/// allowlist -> per-request signals -> entrypoint -> currency.
#[non_exhaustive]
pub struct BidderRequestInfo<'req> {
    /// Alias name (or base name when no aliasing happened).
    pub bidder: BidderName,
    /// Base bidder name (i.e. the bidder's own name in the registry).
    pub core_name: BidderName,
    /// `ResolvedAccount.id` post-fallback. Drives `{{.AccountID}}` macro.
    pub account_id: &'req str,
    /// Raw `site.publisher.id` / `app.publisher.id` from the normalized
    /// request — whichever is present. Drives the `{{.PublisherID}}`
    /// endpoint macro. `None` when neither inventory branch carries a
    /// publisher id; a bidder whose template uses `{{.PublisherID}}`
    /// against such a request will fail `build()` with
    /// `MissingMacro("PublisherID")` — surfaced as code-5
    /// `FailedToRequestBids`. Publisher id and account id are
    /// intentionally distinct — do NOT fall back to `account_id`.
    pub publisher_id: Option<&'req str>,
    /// The selected operator URL or a compiled `BidderInfo.endpoint` template.
    /// The bidder constructs the final URL via `EndpointBuilder`; configured
    /// literals have no endpoint macros.
    pub endpoint_template: &'req str,
    /// Macro names permitted by this bidder's endpoint template.
    /// E.g. `&["AccountID", "PublisherID"]`.
    pub endpoint_macros: &'req [&'req str],
    /// `Some(allowed_hosts)` for host-varying endpoints; `None` for
    /// fixed-host endpoints.
    pub host_allowlist: Option<&'req [&'req str]>,
    /// `regs.ext.gpc` value if set on the normalized request.
    pub gpc: Option<&'req str>,
    /// Trusted caller entrypoint. The current HTTP auction route uses `Auction`;
    /// AMP/video context does not imply those public routes are implemented.
    pub entry_point: EntryPoint,
    /// `req.cur` (currency list, unchanged).
    pub currency: &'req [String],
    /// Borrowed floor-only rates; empty unless installed by the trusted caller.
    pub floor_currency: FloorConversion<'req>,
    /// One admitted coherent rotation, borrowed only during construction.
    pub credentials: Option<&'req BidderCredentials>,
    /// Shape captured from the original HTTP root `ext`, before normalization
    /// or per-bidder privacy transforms. Typed-only callers have no wire proof.
    pub original_request_ext_shape: JsonFieldShape,
    pub settings: BidderSettingsView<'req>,
}

impl<'req> BidderRequestInfo<'req> {
    /// Explicit constructor — `#[non_exhaustive]` blocks the struct
    /// literal outside this crate, so `run_auction` (in
    /// `auctioneer-exchange`) uses this seam.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors the non_exhaustive field set 1:1; grouping into a builder\
                  would force `auctioneer-exchange` to depend on a builder type"
    )]
    pub fn new(
        bidder: BidderName,
        core_name: BidderName,
        account_id: &'req str,
        publisher_id: Option<&'req str>,
        endpoint_template: &'req str,
        endpoint_macros: &'req [&'req str],
        host_allowlist: Option<&'req [&'req str]>,
        gpc: Option<&'req str>,
        entry_point: EntryPoint,
        currency: &'req [String],
    ) -> Self {
        Self {
            bidder,
            core_name,
            account_id,
            publisher_id,
            endpoint_template,
            endpoint_macros,
            host_allowlist,
            gpc,
            entry_point,
            currency,
            floor_currency: FloorConversion::default(),
            credentials: None,
            original_request_ext_shape: JsonFieldShape::Unavailable,
            settings: BidderSettingsView::default(),
        }
    }

    /// Attach settings only after matching both entries to the resolved implementation.
    ///
    /// # Errors
    /// Returns a fixed configuration diagnostic for any mismatched entry.
    pub fn set_settings(
        &mut self,
        implementation: Option<&str>,
        requested: Option<&'req BidderSettings>,
        base: Option<&'req BidderSettings>,
    ) -> Result<(), BidderError> {
        self.settings = BidderSettingsView::default();
        if requested
            .into_iter()
            .chain(base)
            .any(|settings| Some(settings.implementation_key()) != implementation)
        {
            return Err(BidderError::failed_to_request_bids(
                "bidder settings implementation mismatch",
            ));
        }
        for settings in requested.into_iter().chain(base) {
            ensure_settings_limits(settings).map_err(|_reason| {
                BidderError::failed_to_request_bids("bidder settings exceed limits")
            })?;
        }
        self.settings = BidderSettingsView { requested, base };
        Ok(())
    }

    #[must_use]
    pub const fn settings(&self) -> Option<&'req BidderSettings> {
        self.settings.requested
    }

    #[must_use]
    pub const fn base_settings(&self) -> Option<&'req BidderSettings> {
        self.settings.base
    }
}

#[cfg(test)]
mod tests {
    // Settings-bearing cases run in tests/generated_settings.rs against fake declarations.
    use super::{BidderRequestInfo, EntryPoint};
    use crate::currency::{FloorConversion, FloorConversionError};
    use crate::types::BidderName;
    use auctioneer_runtime::bounded_json::JsonFieldShape;

    #[test]
    fn floor_currency_request_info_constructor_defaults_to_usd_identity_only() {
        let info = BidderRequestInfo::new(
            BidderName::new("fake_fixture"),
            BidderName::new("fake_fixture"),
            "account",
            None,
            "https://example.test/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        assert_eq!(
            info.floor_currency.convert_floor_to_usd(2.5, "USD"),
            Ok(2.5_f64)
        );
        assert_eq!(
            info.floor_currency.convert_floor_to_usd(2.5, "QAA"),
            Err(FloorConversionError::MissingSnapshot)
        );
    }

    #[test]
    fn floor_currency_request_info_borrows_snapshot_without_changing_request_currency() {
        let config = serde_json::from_value(serde_json::json!({
            "expires_at": 2_000_i32,
            "usd_per_unit": {"QAA": 2.5_f64}
        }))
        .unwrap();
        let request_currency = ["QAB".to_owned()];
        let mut info = BidderRequestInfo::new(
            BidderName::new("fake_fixture"),
            BidderName::new("fake_fixture"),
            "account",
            None,
            "https://example.test/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &request_currency,
        );
        info.floor_currency = FloorConversion::new(Some(&config), 1_000);
        assert_eq!(
            info.floor_currency.convert_floor_to_usd(4.0, "QAA"),
            Ok(10.0_f64)
        );
        assert_eq!(info.currency, &request_currency);
    }

    #[test]
    fn typed_request_info_does_not_claim_original_extension_absence() {
        let info = BidderRequestInfo::new(
            BidderName::new("fake_fixture"),
            BidderName::new("fake_fixture"),
            "account",
            None,
            "https://bidder.example/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        assert_eq!(info.original_request_ext_shape, JsonFieldShape::Unavailable);
    }

    #[test]
    fn request_info_builds() {
        let macros: &[&str] = &["AccountID"];
        let currency: &[String] = &[];
        let info = BidderRequestInfo {
            bidder: BidderName::new("fake_ax"),
            core_name: BidderName::new("fake_one"),
            account_id: "acct-42",
            publisher_id: None,
            endpoint_template: "https://example.test/openrtb?acct={{.AccountID}}",
            endpoint_macros: macros,
            host_allowlist: None,
            gpc: None,
            entry_point: EntryPoint::Auction,
            currency,
            floor_currency: super::FloorConversion::default(),
            credentials: None,
            original_request_ext_shape: JsonFieldShape::Unavailable,
            settings: super::BidderSettingsView::default(),
        };
        assert_eq!(info.bidder.as_str(), "fake_ax");
        assert_eq!(info.core_name.as_str(), "fake_one");
        assert_eq!(info.account_id, "acct-42");
    }
}

//! Independent policy state and its per-bidder composition.

use crate::{Activity, AuctionPermissions, Component, PrivacyContext, ccpa::NoSale};

#[derive(Debug, Default)]
pub(crate) struct IndependentPolicies {
    pub ccpa: bool,
    pub lmt: bool,
    pub coppa: bool,
    pub no_sale: NoSale,
}

/// The first failed fetch gate. Scrub permissions remain independent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchDenial {
    Activity,
    Gdpr,
}

/// Immutable outcomes for one requested bidder identity, before projection or scrubbing.
#[derive(Clone, Debug)]
pub struct BidderDecision {
    pub fetch_denial: Option<FetchDenial>,
    pub gdpr: AuctionPermissions,
    pub transmit_ufpd: bool,
    pub transmit_precise_geo: bool,
    pub transmit_tid: bool,
    pub ccpa: bool,
    pub lmt: bool,
    pub coppa: bool,
}

#[expect(
    clippy::multiple_inherent_impl,
    reason = "independent policy composition is separate from GDPR legal-basis evaluation"
)]
impl PrivacyContext {
    #[must_use]
    pub fn bidder_decision(&self, core_name: &str, bidder: &str) -> BidderDecision {
        let component = Component::Bidder(bidder);
        let fetch_allowed = self.activity_allow(Activity::FetchBids, component);
        let gdpr = self.permissions(core_name, bidder);
        let gdpr_denied = matches!(
            gdpr,
            AuctionPermissions::DenyBidRequest
                | AuctionPermissions::Partial {
                    allow_bid_request: false,
                    ..
                }
        );
        BidderDecision {
            fetch_denial: if !fetch_allowed {
                Some(FetchDenial::Activity)
            } else if gdpr_denied {
                Some(FetchDenial::Gdpr)
            } else {
                None
            },
            gdpr,
            transmit_ufpd: self.activity_allow(Activity::TransmitUfpd, component),
            transmit_precise_geo: self.activity_allow(Activity::TransmitPreciseGeo, component),
            transmit_tid: self.activity_allow(Activity::TransmitTid, component),
            ccpa: self.independent.ccpa && !self.independent.no_sale.exempts(bidder),
            lmt: self.independent.lmt,
            coppa: self.independent.coppa,
        }
    }
}

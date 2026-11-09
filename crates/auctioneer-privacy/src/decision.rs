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
    Host,
}

/// Independent host ceilings, not consent or legal-basis facts. Defaults deny all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostRestrictions {
    pub allow_fetch: bool,
    pub allow_ids: bool,
    pub allow_precise_geo: bool,
    pub allow_user_fpd: bool,
    pub allow_tid: bool,
}

/// Synchronous request-bound policy. Implementations must not perform I/O.
pub trait HostRestrictionSource {
    /// Return ceilings for the requested bidder/alias and its independently bound
    /// canonical implementation. These ceilings cannot grant engine permissions.
    fn restrictions(
        &self,
        requested_bidder: &str,
        canonical_implementation: &str,
    ) -> HostRestrictions;
}

/// Explicit standalone choice imposing no additional host restrictions.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnrestrictedHostRestrictions;

impl HostRestrictionSource for UnrestrictedHostRestrictions {
    fn restrictions(&self, _: &str, _: &str) -> HostRestrictions {
        HostRestrictions {
            allow_fetch: true,
            allow_ids: true,
            allow_precise_geo: true,
            allow_user_fpd: true,
            allow_tid: true,
        }
    }
}

/// Immutable outcomes for one requested bidder identity, before projection or scrubbing.
#[derive(Clone, Debug)]
pub struct BidderDecision {
    pub fetch_denial: Option<FetchDenial>,
    pub gdpr: AuctionPermissions,
    /// Independent host ID ceiling; never encoded as a GDPR permission.
    pub allow_host_ids: bool,
    pub transmit_ufpd: bool,
    pub transmit_precise_geo: bool,
    pub transmit_tid: bool,
    pub ccpa: bool,
    pub lmt: bool,
    pub coppa: bool,
}

impl BidderDecision {
    /// Intersect host ceilings without changing decoded consent or engine facts.
    /// Repeated composition can only restrict an existing decision further.
    pub fn restrict(&mut self, ceilings: HostRestrictions) {
        if !ceilings.allow_fetch && self.fetch_denial.is_none() {
            self.fetch_denial = Some(FetchDenial::Host);
        }
        self.allow_host_ids &= ceilings.allow_ids;
        self.transmit_precise_geo &= ceilings.allow_precise_geo;
        self.transmit_ufpd &= ceilings.allow_user_fpd;
        self.transmit_tid &= ceilings.allow_tid;
    }
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
            allow_host_ids: true,
            transmit_ufpd: self.activity_allow(Activity::TransmitUfpd, component),
            transmit_precise_geo: self.activity_allow(Activity::TransmitPreciseGeo, component),
            transmit_tid: self.activity_allow(Activity::TransmitTid, component),
            ccpa: self.independent.ccpa && !self.independent.no_sale.exempts(bidder),
            lmt: self.independent.lmt,
            coppa: self.independent.coppa,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FetchDenial, HostRestrictionSource as _, HostRestrictions, UnrestrictedHostRestrictions,
    };
    use crate::{AuctionPermissions, PrivacyContext};

    #[test]
    fn host_ceilings_intersect_each_dimension_without_rewriting_gdpr() {
        for engine_allows in [false, true] {
            for host_allows in [false, true] {
                for dimension in 0_u8..5 {
                    let mut decision = PrivacyContext::allow_all_no_scrub()
                        .bidder_decision("fake_base", "fake_alias");
                    match dimension {
                        0 => {
                            decision.fetch_denial = (!engine_allows).then_some(FetchDenial::Gdpr);
                            decision.gdpr = AuctionPermissions::Partial {
                                allow_bid_request: engine_allows,
                                pass_id: true,
                                pass_geo: true,
                            };
                        }
                        1 => {
                            decision.gdpr = AuctionPermissions::Partial {
                                allow_bid_request: true,
                                pass_id: engine_allows,
                                pass_geo: true,
                            }
                        }
                        2 => decision.transmit_precise_geo = engine_allows,
                        3 => decision.transmit_ufpd = engine_allows,
                        4 => decision.transmit_tid = engine_allows,
                        _ => panic!("unknown permission dimension"),
                    }
                    let original_gdpr = decision.gdpr.clone();
                    let mut ceilings =
                        UnrestrictedHostRestrictions.restrictions("fake_alias", "fake_impl");
                    match dimension {
                        0 => ceilings.allow_fetch = host_allows,
                        1 => ceilings.allow_ids = host_allows,
                        2 => ceilings.allow_precise_geo = host_allows,
                        3 => ceilings.allow_user_fpd = host_allows,
                        4 => ceilings.allow_tid = host_allows,
                        _ => panic!("unknown permission dimension"),
                    }
                    decision.restrict(ceilings);
                    assert_eq!(decision.gdpr, original_gdpr);
                    assert_eq!(
                        decision.fetch_denial.is_none(),
                        dimension != 0 || engine_allows && host_allows
                    );
                    assert_eq!(decision.allow_host_ids, dimension != 1 || host_allows);
                    let engine_ids = match decision.gdpr {
                        AuctionPermissions::AllowAll => true,
                        AuctionPermissions::Partial { pass_id, .. } => pass_id,
                        AuctionPermissions::DenyBidRequest => false,
                    };
                    assert_eq!(
                        engine_ids && decision.allow_host_ids,
                        dimension != 1 || engine_allows && host_allows
                    );
                    assert_eq!(
                        decision.transmit_precise_geo,
                        dimension != 2 || engine_allows && host_allows
                    );
                    assert_eq!(
                        decision.transmit_ufpd,
                        dimension != 3 || engine_allows && host_allows
                    );
                    assert_eq!(
                        decision.transmit_tid,
                        dimension != 4 || engine_allows && host_allows
                    );
                    if dimension == 0 && !engine_allows {
                        assert_eq!(decision.fetch_denial, Some(FetchDenial::Gdpr));
                    } else if dimension == 0 && !host_allows {
                        assert_eq!(decision.fetch_denial, Some(FetchDenial::Host));
                    } else {
                        assert_eq!(decision.fetch_denial, None);
                    }
                    assert!(!decision.ccpa);
                    assert!(!decision.lmt);
                    assert!(!decision.coppa);
                }
            }
        }
    }

    #[test]
    fn host_denial_preserves_first_engine_fetch_gate_and_independent_policies() {
        for denial in [FetchDenial::Activity, FetchDenial::Gdpr] {
            let mut decision =
                PrivacyContext::allow_all_no_scrub().bidder_decision("fake_base", "fake_alias");
            decision.fetch_denial = Some(denial);
            decision.ccpa = true;
            decision.lmt = true;
            decision.coppa = true;
            decision.restrict(HostRestrictions::default());
            assert_eq!(decision.fetch_denial, Some(denial));
            assert_eq!(decision.gdpr, AuctionPermissions::AllowAll);
            assert!(decision.ccpa);
            assert!(decision.lmt);
            assert!(decision.coppa);
        }
    }

    #[test]
    fn host_restrictions_default_closed_and_cannot_be_reopened() {
        assert_eq!(
            HostRestrictions::default(),
            HostRestrictions {
                allow_fetch: false,
                allow_ids: false,
                allow_precise_geo: false,
                allow_user_fpd: false,
                allow_tid: false,
            }
        );
        let mut decision =
            PrivacyContext::allow_all_no_scrub().bidder_decision("fake_base", "fake_alias");
        decision.restrict(HostRestrictions::default());
        decision.restrict(UnrestrictedHostRestrictions.restrictions("fake_alias", "fake_impl"));
        assert_eq!(decision.gdpr, AuctionPermissions::AllowAll);
        assert_eq!(decision.fetch_denial, Some(FetchDenial::Host));
        assert!(!decision.allow_host_ids);
        assert!(!decision.transmit_precise_geo);
        assert!(!decision.transmit_ufpd);
        assert!(!decision.transmit_tid);
    }
}

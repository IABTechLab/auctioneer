//! Owned preparation; engine views borrow only inside the consuming future.

use super::{
    AuctionFailure, AuctionFailureReason, AuctionFunding, AuctionServices, CompletedAuction,
};
use crate::config::{bidder_settings::RuntimeBidderSettings, resolve::ResolvedAccountOwned};
use auctioneer_bidder::{BidderName, FloorConversion};
use auctioneer_config::currency::FloorCurrencyConfig;
use auctioneer_config_data::types::HostConfig;
use auctioneer_exchange::{
    AuctionContext, AuctionPlan, ExchangeFeatures, ResolvedAccount, run_auction,
};
use auctioneer_openrtb::NormalizedBidRequest;
use auctioneer_runtime::{ChargeKind, bounded_json::JsonFieldShape};
use std::{collections::BTreeMap, fmt, mem::take};

pub(super) struct PreparedState {
    pub request: NormalizedBidRequest,
    pub host: HostConfig,
    pub account: ResolvedAccountOwned,
    pub plan: AuctionPlan,
    pub settings: RuntimeBidderSettings,
    pub floor_currency: Option<FloorCurrencyConfig>,
    pub timeouts: BTreeMap<BidderName, u32>,
    pub features: ExchangeFeatures,
    pub original_ext: JsonFieldShape,
    pub debug_requested: bool,
}

/// Request-owned policy projection followed by explicit borrowed host services.
pub struct PreparedAuction<'host> {
    state: Box<PreparedState>,
    services: AuctionServices<'host>,
    funding: AuctionFunding,
}

impl<'host> PreparedAuction<'host> {
    pub(super) fn new(
        state: PreparedState,
        services: AuctionServices<'host>,
        funding: AuctionFunding,
    ) -> Self {
        Self {
            state: Box::new(state),
            services,
            funding,
        }
    }

    pub(crate) fn debug_requested(&self) -> bool {
        self.state.debug_requested
    }
    /// Run the one engine path; no request context or config owner is retained.
    ///
    /// # Errors
    /// Returns a funded failure for expired execution, construction or engine refusal.
    pub async fn run(mut self) -> Result<CompletedAuction, AuctionFailure> {
        let services = &self.services;
        if services.deadline_spent() {
            return Err(AuctionFailure::new(
                AuctionFailureReason::Deadline,
                self.funding,
            ));
        }
        let construction = match services.resources.construction_limits() {
            Ok(limits) => limits,
            Err(error) => {
                return Err(AuctionFailure::new(
                    AuctionFailureReason::Resource(error),
                    self.funding,
                ));
            }
        };
        let id = take(&mut self.state.account.id);
        let state = &self.state;
        let resolved = ResolvedAccount {
            id,
            source: state.account.source,
            config: &state.account.config,
        };
        let result = {
            let (audit, receipts) = self.funding.parts();
            let mut context = AuctionContext::with_request_start_ext(
                services.request_start,
                &state.host,
                &resolved,
                services.bidders,
                services.fanout,
                services.clock,
                services.rng,
                services.gvl_store,
                services.gcl,
                services.now,
                state.debug_requested,
                state.features,
                &state.timeouts,
            );
            context.entry_point = services.entry_point;
            context.outer_deadline = services.outer_deadline;
            context.host_restrictions = Some(services.restrictions);
            context.bidder_bindings = services.bidder_bindings;
            context.original_request_ext_shape = state.original_ext;
            context.bidder_settings = Some(&state.settings);
            context.credential_source = services.credentials;
            context.schema_scratch_bytes = services.resources.schema_scratch_bytes();
            context.floor_currency =
                FloorConversion::new(state.floor_currency.as_ref(), services.now.unix_timestamp());
            context.projection_reservation = receipts
                .iter()
                .find(|held| held.kind() == ChargeKind::Projection);
            context.response_reservation = receipts
                .iter()
                .find(|held| held.kind() == ChargeKind::Response);
            context.construction_limits = Some(construction);
            run_auction(&state.request, &context, &state.plan, audit).await
        };
        drop(resolved);
        drop(self.state);
        match result {
            Ok(outcome) => Ok(CompletedAuction::new(outcome, self.funding)),
            Err(error) => Err(AuctionFailure::new(
                AuctionFailureReason::Engine(error),
                self.funding,
            )),
        }
    }
}

impl fmt::Debug for PreparedAuction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedAuction").finish_non_exhaustive()
    }
}

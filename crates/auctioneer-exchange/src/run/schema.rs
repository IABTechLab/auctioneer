//! Schema admission on privacy-surviving parameters, before host services.

use super::{AuctionContext, ErrorEntry, MakeRequestsOutput, PerBidderRequest, TmaxResolution};
use auctioneer_bidder::schema::{
    NATIVE_SCHEMA_SCRATCH_BYTES, NativeSchemaError, native_schema_available,
    native_schema_required, validate_native_parameters,
};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::Value;

pub(super) fn admit(
    ctx: &AuctionContext<'_>,
    pbr: &mut PerBidderRequest,
    tmax: &TmaxResolution,
    out: &mut MakeRequestsOutput,
) -> bool {
    let mandatory = native_schema_required(pbr.base.as_str());
    let funded_legacy = ctx.schema_scratch_bytes >= NATIVE_SCHEMA_SCRATCH_BYTES
        && native_schema_available(pbr.base.as_str());
    if !mandatory && !funded_legacy {
        return true;
    }
    let errors = out.bidder_pre_errors.entry(pbr.bidder.clone()).or_default();
    if ctx.schema_scratch_bytes < NATIVE_SCHEMA_SCRATCH_BYTES
        || ctx.projection_reservation.is_none()
    {
        errors.push(error(NativeSchemaError::Unavailable));
        return false;
    }
    retain_valid(&mut pbr.request, errors, |value| {
        validate_native_parameters(pbr.base.as_str(), value, &mut out.schema_work, || {
            ctx.clock.now_monotonic() >= tmax.deadline.deadline_at
        })
    });
    !pbr.request.imp.is_empty()
}

fn error(reason: NativeSchemaError) -> ErrorEntry {
    let (code, message) = match reason {
        NativeSchemaError::InvalidParameters => (2, "invalid bidder parameters"),
        NativeSchemaError::Unavailable => (5, "bidder schema unavailable"),
        NativeSchemaError::Resource => (5, "bidder schema resource allowance exhausted"),
        NativeSchemaError::Deadline => (5, "bidder schema deadline expired"),
    };
    ErrorEntry {
        code,
        message: message.into(),
    }
}

fn retain_valid<F>(request: &mut BidRequest, errors: &mut Vec<ErrorEntry>, mut validate: F)
where
    F: FnMut(&Value) -> Result<(), NativeSchemaError>,
{
    request.imp.retain(|imp| {
        let result = imp
            .ext
            .as_ref()
            .and_then(|ext| ext.bidder.as_ref())
            .ok_or(NativeSchemaError::InvalidParameters)
            .and_then(&mut validate);
        match result {
            Ok(()) => true,
            Err(reason) => {
                errors.push(error(reason));
                false
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parameter_refusals_are_impression_local_and_preserve_survivor_order() {
        let mut request: BidRequest = serde_json::from_value(json!({
            "id":"fake", "imp":[
                {"id":"a","ext":{"bidder":{"valid":true}}},
                {"id":"b","ext":{"bidder":{"valid":false}}},
                {"id":"c","ext":{"bidder":{"valid":true}}},
                {"id":"d"}
            ]
        }))
        .unwrap();
        let mut errors = Vec::new();
        let mut visits = 0_u32;
        retain_valid(&mut request, &mut errors, |value| {
            visits += 1;
            if value["valid"] == true {
                Ok(())
            } else {
                Err(NativeSchemaError::InvalidParameters)
            }
        });
        assert_eq!(visits, 3);
        assert_eq!(
            request
                .imp
                .iter()
                .map(|imp| imp.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert_eq!(errors.len(), 2);
        assert!(
            errors
                .iter()
                .all(|row| row.code == 2 && row.message == "invalid bidder parameters")
        );
    }

    #[test]
    fn schema_resource_errors_cannot_be_misreported_as_invalid_parameters() {
        for (reason, message) in [
            (NativeSchemaError::Unavailable, "bidder schema unavailable"),
            (
                NativeSchemaError::Resource,
                "bidder schema resource allowance exhausted",
            ),
            (
                NativeSchemaError::Deadline,
                "bidder schema deadline expired",
            ),
        ] {
            let actual = error(reason);
            assert_eq!(actual.code, 5);
            assert_eq!(actual.message, message);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native {
        use super::*;
        use crate::{
            AccountSource, AuctionDeadline, BidderCall, BidderCallResult, BidderFanout,
            FanoutInternalError, ResolvedAccount, resolve_tmax,
        };
        use auctioneer_bidder::schema::{
            SchemaWorkBudget, SchemaWorkLimits, controlled_native_costs, initialize_native_schemas,
            native_schema_work_budget,
        };
        use auctioneer_bidder::test_support::serving_catalog::{ServingBidder, published};
        use auctioneer_bidder::{BidderName, BidderRegistry, CallBudget};
        use auctioneer_config_data::types::{AccountConfig, HostConfig, ResourceLimits};
        use auctioneer_privacy::{gcl::Gcl, gvl::GvlStore};
        use auctioneer_runtime::bounded_json::count_json_nodes;
        use auctioneer_runtime::{
            AuctionBudget, CatalogResourceCosts, ChargeKind, Clock, MonotonicInstant, SeededRng,
        };
        use std::collections::BTreeMap;
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::time::Duration;
        use time::OffsetDateTime;

        struct AdvancingClock {
            request_start: MonotonicInstant,
            deadline: MonotonicInstant,
            live_reads: u32,
            reads: AtomicU32,
        }

        impl AdvancingClock {
            fn new(live_reads: u32) -> Self {
                let request_start = MonotonicInstant::now();
                Self {
                    request_start,
                    deadline: request_start.checked_add(Duration::from_millis(1)).unwrap(),
                    live_reads,
                    reads: AtomicU32::new(0),
                }
            }

            fn reads(&self) -> u32 {
                self.reads.load(Ordering::Relaxed)
            }
        }

        impl Clock for AdvancingClock {
            fn now(&self) -> OffsetDateTime {
                OffsetDateTime::UNIX_EPOCH
            }

            fn now_monotonic(&self) -> MonotonicInstant {
                if self.reads.fetch_add(1, Ordering::Relaxed) < self.live_reads {
                    self.deadline.checked_sub(Duration::from_nanos(1)).unwrap()
                } else {
                    self.deadline
                }
            }
        }

        struct NoFanout;

        #[async_trait::async_trait(?Send)]
        impl BidderFanout for NoFanout {
            async fn send_all(
                &self,
                _calls: Vec<BidderCall>,
                _deadline: AuctionDeadline,
            ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
                panic!("schema admission must not dispatch host services")
            }
        }

        fn with_context(
            clock: &AdvancingClock,
            test: impl FnOnce(&AuctionContext<'_>, &TmaxResolution),
        ) {
            let host = HostConfig {
                resource_limits: ResourceLimits {
                    default_tmax_ms: 1,
                    max_tmax_ms: 1,
                    ..ResourceLimits::default()
                },
                ..HostConfig::default()
            };
            let config = AccountConfig::default();
            let account = ResolvedAccount {
                id: "fake_account".into(),
                source: AccountSource::Provided,
                config: &config,
            };
            let registry = BidderRegistry::new();
            let rng = SeededRng::new(1);
            let budget = AuctionBudget::new(NATIVE_SCHEMA_SCRATCH_BYTES);
            let projection = budget
                .try_reserve(ChargeKind::Projection, NATIVE_SCHEMA_SCRATCH_BYTES)
                .unwrap();
            let mut ctx = AuctionContext::with_request_start(
                clock.request_start,
                &host,
                &account,
                &registry,
                &NoFanout,
                clock,
                &rng,
                GvlStore::baked(),
                Gcl::baked(),
                OffsetDateTime::UNIX_EPOCH,
                false,
            );
            ctx.schema_scratch_bytes = NATIVE_SCHEMA_SCRATCH_BYTES;
            ctx.projection_reservation = Some(&projection);
            let tmax = resolve_tmax(Some(1), &host, clock.request_start).unwrap();
            assert_eq!(tmax.deadline.deadline_at, clock.deadline);
            test(&ctx, &tmax);
        }

        fn owner() -> ServingBidder {
            assert!(initialize_native_schemas(controlled_native_costs(
                CatalogResourceCosts::UNCHANGED
            )));
            published()
                .into_iter()
                .find(|row| row.info.alias_of.is_none() && native_schema_required(row.name()))
                .expect("the controlled-native catalog supplies a canonical owner fixture")
        }

        fn output(schema_work: SchemaWorkBudget) -> MakeRequestsOutput {
            MakeRequestsOutput {
                bidder_pairs: Vec::new(),
                bidder_pre_errors: BTreeMap::new(),
                construction_diagnostics: BTreeMap::new(),
                timing: BTreeMap::new(),
                budget: CallBudget::new(1),
                outbound_drops: Vec::new(),
                schema_work,
            }
        }

        fn scope(name: &str, owner: &ServingBidder, params: &[(&str, &Value)]) -> PerBidderRequest {
            let imps: Vec<_> = params
                .iter()
                .map(|(id, value)| json!({"id":id,"ext":{"bidder":value}}))
                .collect();
            PerBidderRequest {
                bidder: BidderName::new(name),
                base: BidderName::new(owner.name()),
                request: serde_json::from_value(json!({"id":"fake_request","imp":imps})).unwrap(),
            }
        }

        fn work(value: &Value) -> u64 {
            // Independent byte/node witnesses pin the frozen positive-charge policy.
            let bytes = serde_json::to_vec(value).unwrap();
            let nodes = count_json_nodes(&bytes, 4_096).unwrap();
            let byte_work = u64::try_from(bytes.len()).unwrap().checked_mul(16).unwrap();
            let node_work = u64::from(nodes).checked_mul(128).unwrap();
            byte_work
                .checked_add(node_work)
                .and_then(|units| units.checked_add(1_024))
                .expect("frozen work policy fits the bounded fixture")
        }

        fn assert_imps(pbr: &PerBidderRequest, expected: &[&str]) {
            assert_eq!(
                pbr.request
                    .imp
                    .iter()
                    .map(|imp| imp.id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
        }

        fn assert_errors(out: &MakeRequestsOutput, name: &str, expected: &[(u32, &str)]) {
            let errors = &out.bidder_pre_errors[&BidderName::new(name)];
            assert_eq!(
                errors
                    .iter()
                    .map(|row| (row.code, row.message.as_str()))
                    .collect::<Vec<_>>(),
                expected
            );
        }

        #[test]
        fn aggregate_work_retains_invalid_debits_across_sibling_calls_at_exact_limit() {
            let owner = owner();
            let invalid = json!(false);
            let valid_work = work(&owner.params);
            let invalid_work = work(&invalid);
            assert_eq!(invalid_work, 1_232);
            assert!(valid_work > 0);
            let total = invalid_work + 2 * valid_work;
            for shortfall in [0, 1] {
                let clock = AdvancingClock::new(u32::MAX);
                with_context(&clock, |ctx, tmax| {
                    let mut out = output(SchemaWorkBudget::new(SchemaWorkLimits::new(
                        0x0040_0000,
                        total - shortfall,
                        NATIVE_SCHEMA_SCRATCH_BYTES,
                    )));
                    let mut first = scope(
                        "fake_first",
                        &owner,
                        &[("invalid", &invalid), ("first_valid", &owner.params)],
                    );
                    assert!(admit(ctx, &mut first, tmax, &mut out));
                    assert_imps(&first, &["first_valid"]);
                    assert_errors(&out, "fake_first", &[(2, "invalid bidder parameters")]);
                    assert_eq!(out.schema_work.remaining(), valid_work - shortfall);
                    assert_eq!(clock.reads(), 6);

                    let mut sibling =
                        scope("fake_sibling", &owner, &[("second_valid", &owner.params)]);
                    assert_eq!(admit(ctx, &mut sibling, tmax, &mut out), shortfall == 0);
                    if shortfall == 0 {
                        assert_imps(&sibling, &["second_valid"]);
                        assert_errors(&out, "fake_sibling", &[]);
                        assert_eq!(out.schema_work.remaining(), 0);
                        assert_eq!(clock.reads(), 9);
                    } else {
                        assert_imps(&sibling, &[]);
                        assert_errors(
                            &out,
                            "fake_sibling",
                            &[(5, "bidder schema resource allowance exhausted")],
                        );
                        assert_eq!(out.schema_work.remaining(), valid_work - 1);
                        assert_eq!(clock.reads(), 8);
                    }

                    // Fixed attempts, including a zero balance: never poll for exhaustion.
                    let remaining = out.schema_work.remaining();
                    let reads = clock.reads();
                    let mut later = scope("fake_later", &owner, &[("third_valid", &owner.params)]);
                    assert!(!admit(ctx, &mut later, tmax, &mut out));
                    assert_imps(&later, &[]);
                    assert_errors(
                        &out,
                        "fake_later",
                        &[(5, "bidder schema resource allowance exhausted")],
                    );
                    assert_eq!(out.schema_work.remaining(), remaining);
                    assert_eq!(clock.reads(), reads + 2);
                    assert_imps(&first, &["first_valid"]);
                });
            }
        }

        #[test]
        fn advancing_deadline_checks_precharge_and_post_evaluation_at_exact_instant() {
            let owner = owner();
            let invalid = json!(false);
            for params in [&owner.params, &invalid] {
                for live_reads in [0, 1, 2, 3] {
                    let clock = AdvancingClock::new(live_reads);
                    with_context(&clock, |ctx, tmax| {
                        let mut out =
                            output(native_schema_work_budget(NATIVE_SCHEMA_SCRATCH_BYTES));
                        let initial = out.schema_work.remaining();
                        let mut pbr = scope("fake_deadline", &owner, &[("candidate", params)]);
                        let accepted = admit(ctx, &mut pbr, tmax, &mut out);
                        assert_eq!(accepted, live_reads == 3 && params.is_object());
                        assert_eq!(clock.reads(), (live_reads + 1).min(3));
                        assert_eq!(
                            out.schema_work.remaining(),
                            initial - if live_reads >= 2 { work(params) } else { 0 }
                        );
                        if live_reads < 3 {
                            assert_imps(&pbr, &[]);
                            assert_errors(
                                &out,
                                "fake_deadline",
                                &[(5, "bidder schema deadline expired")],
                            );
                        } else if params.is_object() {
                            assert_imps(&pbr, &["candidate"]);
                            assert_errors(&out, "fake_deadline", &[]);
                        } else {
                            assert_imps(&pbr, &[]);
                            assert_errors(
                                &out,
                                "fake_deadline",
                                &[(2, "invalid bidder parameters")],
                            );
                        }
                    });
                }
            }
        }

        #[test]
        fn post_evaluation_deadline_keeps_prior_sibling_and_never_renews_work_or_time() {
            let owner = owner();
            let invalid = json!(false);
            let clock = AdvancingClock::new(5);
            with_context(&clock, |ctx, tmax| {
                let mut out = output(native_schema_work_budget(NATIVE_SCHEMA_SCRATCH_BYTES));
                let initial = out.schema_work.remaining();
                let mut first = scope("fake_first", &owner, &[("first_valid", &owner.params)]);
                assert!(admit(ctx, &mut first, tmax, &mut out));
                assert_eq!(clock.reads(), 3);
                assert_eq!(out.schema_work.remaining(), initial - work(&owner.params));

                let mut late = scope("fake_late", &owner, &[("late_invalid", &invalid)]);
                assert!(!admit(ctx, &mut late, tmax, &mut out));
                assert_eq!(clock.reads(), 6);
                assert_imps(&late, &[]);
                assert_errors(&out, "fake_late", &[(5, "bidder schema deadline expired")]);
                let remaining = initial - work(&owner.params) - work(&invalid);
                assert_eq!(out.schema_work.remaining(), remaining);

                let mut later = scope("fake_later", &owner, &[("later_valid", &owner.params)]);
                assert!(!admit(ctx, &mut later, tmax, &mut out));
                assert_eq!(clock.reads(), 7);
                assert_eq!(out.schema_work.remaining(), remaining);
                assert_imps(&later, &[]);
                assert_errors(&out, "fake_later", &[(5, "bidder schema deadline expired")]);
                assert_imps(&first, &["first_valid"]);
                assert_errors(&out, "fake_first", &[]);
                assert_eq!(tmax.deadline.deadline_at, clock.deadline);
            });
        }
    }
}

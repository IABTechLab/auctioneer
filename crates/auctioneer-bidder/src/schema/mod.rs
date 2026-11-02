//! Borrowed input admission and precharging for schema evaluations.
//!
//! This boundary does not establish bound qualification or enforce allocator
//! behavior. The caller owns the documented bound or approved tested reservation,
//! admitted instance shape, funded limits, and original auction clock. Sharing
//! one budget across all bidders and impressions is the composition owner's
//! responsibility. The controlled-native integration uses tested reservations;
//! complete engine bounds remain a separate qualification obligation.

mod input;
mod native;
pub(crate) mod staging;

pub use native::{
    NATIVE_CATALOG_INITIALIZATION_BYTES, NATIVE_CATALOG_RESIDENT_BYTES,
    NATIVE_SCHEMA_SCRATCH_BYTES, NativeSchemaError, controlled_native_costs,
    initialize_native_schemas, native_schema_available, native_schema_funded,
    native_schema_required, native_schema_work_budget, validate_native_parameters,
};

/// Build mode only; `Normal` does not assert resource qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaArtifactState {
    Normal,
    Unqualified,
}

/// Inspect staging state without exposing validators or initializing caches.
#[must_use]
pub fn schema_artifact_state() -> SchemaArtifactState {
    if staging::has_unqualified_artifact() {
        SchemaArtifactState::Unqualified
    } else {
        SchemaArtifactState::Normal
    }
}

pub use input::{
    MAX_SCHEMA_INPUT_DEPTH, SchemaInputError, SchemaInputLimits, SchemaInputStats,
    measure_schema_input,
};

/// Scalar limits supplied by the owner composing the auction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaWorkLimits {
    /// Maximum work charge for one evaluation.
    pub single_evaluation_work_units: u64,
    /// Total work available across the auction.
    pub auction_work_units: u64,
    /// Already-funded peak scratch allowance for one evaluation.
    pub evaluation_scratch_bytes: u64,
}

impl SchemaWorkLimits {
    /// Build limits without defaults, workload substitution, or clamping.
    #[must_use]
    pub const fn new(
        single_evaluation_work_units: u64,
        auction_work_units: u64,
        evaluation_scratch_bytes: u64,
    ) -> Self {
        Self {
            single_evaluation_work_units,
            auction_work_units,
            evaluation_scratch_bytes,
        }
    }
}

/// A caller's projection from a documented bound or approved reservation.
///
/// Public scalar fields do not prove provenance or bound correctness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaEvaluationCost {
    /// Work charged even if validation returns false or panics.
    pub work_units: u64,
    /// Peak evaluation scratch, not a cumulative auction charge.
    pub scratch_bytes: u64,
}

/// Failure to project a cost using checked arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaCostError {
    /// The caller's cost projection overflowed.
    Overflow,
}

/// Fixed refusals, containing no instance values or validator diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaEvaluationError {
    /// The cost projection overflowed.
    CostOverflow,
    /// A zero work charge cannot fund an evaluation.
    ZeroWork,
    /// The cost exceeds the single-evaluation work limit.
    SingleWork,
    /// The cost exceeds the funded evaluation scratch limit.
    Scratch,
    /// The cost exceeds the auction's remaining work.
    AggregateWork,
    /// The original auction deadline has expired.
    Deadline,
}

/// The one auction-global work budget, deliberately not `Clone`.
#[derive(Debug)]
pub struct SchemaWorkBudget {
    limits: SchemaWorkLimits,
    remaining: u64,
}

impl SchemaWorkBudget {
    /// Use exactly the limits supplied by the composition owner.
    #[must_use]
    pub const fn new(limits: SchemaWorkLimits) -> Self {
        Self {
            limits,
            remaining: limits.auction_work_units,
        }
    }

    /// Work still available across all evaluations sharing this budget.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Check admission and charge work before invoking the validator.
    ///
    /// `is_expired` must check the original auction deadline. Positive charges
    /// exactly at a limit are allowed. Scratch is compared with its funded
    /// allowance, not allocated or deducted. Once admitted, work is debited
    /// before `validate` runs and is never refunded, including on false or panic.
    /// Validator panics propagate to the caller.
    ///
    /// # Errors
    /// Returns [`SchemaEvaluationError::Deadline`] if the clock is expired,
    /// then checks cost overflow, zero work, single work, scratch, and remaining
    /// aggregate work in that order. Every refusal leaves the balance unchanged
    /// and never invokes `validate`.
    pub fn evaluate<C: FnOnce() -> bool, V: FnOnce() -> bool>(
        &mut self,
        projected_cost: Result<SchemaEvaluationCost, SchemaCostError>,
        is_expired: C,
        validate: V,
    ) -> Result<bool, SchemaEvaluationError> {
        if is_expired() {
            return Err(SchemaEvaluationError::Deadline);
        }
        let cost = projected_cost
            .map_err(|SchemaCostError::Overflow| SchemaEvaluationError::CostOverflow)?;
        if cost.work_units == 0 {
            return Err(SchemaEvaluationError::ZeroWork);
        }
        if cost.work_units > self.limits.single_evaluation_work_units {
            return Err(SchemaEvaluationError::SingleWork);
        }
        if cost.scratch_bytes > self.limits.evaluation_scratch_bytes {
            return Err(SchemaEvaluationError::Scratch);
        }
        let remaining = self
            .remaining
            .checked_sub(cost.work_units)
            .ok_or(SchemaEvaluationError::AggregateWork)?;
        self.remaining = remaining;
        Ok(validate())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SchemaCostError, SchemaEvaluationCost, SchemaEvaluationError, SchemaWorkBudget,
        SchemaWorkLimits,
    };
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn budget() -> SchemaWorkBudget {
        SchemaWorkBudget::new(SchemaWorkLimits::new(7, 19, 11))
    }

    fn cost(work_units: u64, scratch_bytes: u64) -> SchemaEvaluationCost {
        SchemaEvaluationCost {
            work_units,
            scratch_bytes,
        }
    }

    fn assert_refused(
        budget: &mut SchemaWorkBudget,
        cost: Result<SchemaEvaluationCost, SchemaCostError>,
        expired: bool,
        error: SchemaEvaluationError,
    ) {
        let remaining = budget.remaining();
        let mut calls = 0_usize;
        let result = budget.evaluate(
            cost,
            || expired,
            || {
                calls = calls.checked_add(1).unwrap();
                true
            },
        );
        assert_eq!(result, Err(error));
        assert_eq!(calls, 0, "refusal must precede the validator callback");
        assert_eq!(budget.remaining(), remaining, "refusal must not debit work");
    }

    #[test]
    fn auction_balance_is_not_clamped_to_single_work_or_scratch() {
        assert_eq!(budget().remaining(), 19);
    }

    #[test]
    fn single_work_costs_under_and_at_limit_are_charged() {
        for (work, remaining) in [(6, 13), (7, 12)] {
            let mut budget = budget();
            let mut calls = 0_usize;
            assert_eq!(
                budget.evaluate(
                    Ok(cost(work, 3)),
                    || false,
                    || {
                        calls = calls.checked_add(1).unwrap();
                        true
                    }
                ),
                Ok(true)
            );
            assert_eq!(calls, 1);
            assert_eq!(budget.remaining(), remaining);
        }
    }

    #[test]
    fn single_work_cost_over_limit_never_calls_validator() {
        assert_refused(
            &mut budget(),
            Ok(cost(8, 3)),
            false,
            SchemaEvaluationError::SingleWork,
        );
    }

    #[test]
    fn changing_single_work_limit_does_not_change_other_limits() {
        for single in [6, 7, 8] {
            let mut budget = SchemaWorkBudget::new(SchemaWorkLimits::new(single, 19, 11));
            if single == 6 {
                assert_refused(
                    &mut budget,
                    Ok(cost(7, 11)),
                    false,
                    SchemaEvaluationError::SingleWork,
                );
            } else {
                assert_eq!(
                    budget.evaluate(Ok(cost(7, 11)), || false, || true),
                    Ok(true)
                );
                assert_eq!(budget.remaining(), 12);
            }
        }
    }

    #[test]
    fn scratch_costs_under_and_at_limit_are_admitted() {
        for scratch in [10, 11] {
            let mut budget = budget();
            let mut calls = 0_usize;
            assert_eq!(
                budget.evaluate(
                    Ok(cost(3, scratch)),
                    || false,
                    || {
                        calls = calls.checked_add(1).unwrap();
                        true
                    }
                ),
                Ok(true)
            );
            assert_eq!(calls, 1);
            assert_eq!(budget.remaining(), 16);
        }
    }

    #[test]
    fn scratch_cost_over_funded_limit_never_calls_validator() {
        assert_refused(
            &mut budget(),
            Ok(cost(3, 12)),
            false,
            SchemaEvaluationError::Scratch,
        );
    }

    #[test]
    fn changing_scratch_limit_does_not_change_work_limits() {
        for scratch in [10, 11, 12] {
            let mut budget = SchemaWorkBudget::new(SchemaWorkLimits::new(7, 19, scratch));
            if scratch == 10 {
                assert_refused(
                    &mut budget,
                    Ok(cost(7, 11)),
                    false,
                    SchemaEvaluationError::Scratch,
                );
            } else {
                assert_eq!(
                    budget.evaluate(Ok(cost(7, 11)), || false, || true),
                    Ok(true)
                );
                assert_eq!(budget.remaining(), 12);
            }
        }
    }

    #[test]
    fn scratch_allowance_is_reusable_not_an_aggregate_charge() {
        let mut budget = budget();
        for _ in 0..2_u8 {
            assert_eq!(
                budget.evaluate(Ok(cost(1, 11)), || false, || true),
                Ok(true)
            );
        }
        assert_eq!(budget.remaining(), 17);
    }

    #[test]
    fn aggregate_costs_under_at_and_over_remaining_are_independent_of_single_limit() {
        for (work, remaining) in [(4, 1), (5, 0), (6, 5)] {
            let mut budget = budget();
            for _ in 0..2_u8 {
                assert_eq!(budget.evaluate(Ok(cost(7, 3)), || false, || true), Ok(true));
            }
            assert_eq!(budget.remaining(), 5);
            if work == 6 {
                assert_refused(
                    &mut budget,
                    Ok(cost(work, 3)),
                    false,
                    SchemaEvaluationError::AggregateWork,
                );
            } else {
                let mut calls = 0_usize;
                assert_eq!(
                    budget.evaluate(
                        Ok(cost(work, 3)),
                        || false,
                        || {
                            calls = calls.checked_add(1).unwrap();
                            true
                        }
                    ),
                    Ok(true)
                );
                assert_eq!(calls, 1);
            }
            assert_eq!(budget.remaining(), remaining);
        }
    }

    #[test]
    fn changing_auction_limit_does_not_change_single_work_or_scratch_limits() {
        for (aggregate, remaining) in [(18, 4), (19, 0), (20, 1)] {
            let mut budget = SchemaWorkBudget::new(SchemaWorkLimits::new(7, aggregate, 11));
            for _ in 0..2_u8 {
                assert_eq!(
                    budget.evaluate(Ok(cost(7, 11)), || false, || true),
                    Ok(true)
                );
            }
            if aggregate == 18 {
                assert_refused(
                    &mut budget,
                    Ok(cost(5, 11)),
                    false,
                    SchemaEvaluationError::AggregateWork,
                );
            } else {
                assert_eq!(
                    budget.evaluate(Ok(cost(5, 11)), || false, || true),
                    Ok(true)
                );
            }
            assert_eq!(budget.remaining(), remaining);
        }
    }

    #[test]
    fn zero_work_never_calls_validator() {
        for scratch in [0, 11] {
            assert_refused(
                &mut budget(),
                Ok(cost(0, scratch)),
                false,
                SchemaEvaluationError::ZeroWork,
            );
        }
    }

    #[test]
    fn overflowing_cost_projection_never_calls_validator() {
        assert_refused(
            &mut budget(),
            Err(SchemaCostError::Overflow),
            false,
            SchemaEvaluationError::CostOverflow,
        );
    }

    #[test]
    fn false_validation_keeps_its_charge_for_later_evaluations() {
        let mut budget = budget();
        let mut calls = 0_usize;
        assert_eq!(
            budget.evaluate(
                Ok(cost(7, 11)),
                || false,
                || {
                    calls = calls.checked_add(1).unwrap();
                    false
                }
            ),
            Ok(false)
        );
        assert_eq!(calls, 1);
        assert_eq!(budget.remaining(), 12);
        assert_eq!(
            budget.evaluate(Ok(cost(7, 11)), || false, || true),
            Ok(true)
        );
        assert_eq!(budget.remaining(), 5);
        assert_refused(
            &mut budget,
            Ok(cost(6, 11)),
            false,
            SchemaEvaluationError::AggregateWork,
        );
        assert_eq!(
            budget.evaluate(Ok(cost(5, 11)), || false, || true),
            Ok(true)
        );
        assert_eq!(budget.remaining(), 0);
    }

    #[test]
    fn exhausted_budget_never_calls_another_validator() {
        let mut budget = budget();
        for work in [7, 7, 5] {
            assert_eq!(
                budget.evaluate(Ok(cost(work, 3)), || false, || true),
                Ok(true)
            );
        }
        assert_eq!(budget.remaining(), 0);
        assert_refused(
            &mut budget,
            Ok(cost(1, 3)),
            false,
            SchemaEvaluationError::AggregateWork,
        );
    }

    #[test]
    fn zero_limits_refuse_unfunded_positive_costs() {
        for (limits, error) in [
            (
                SchemaWorkLimits::new(0, 19, 11),
                SchemaEvaluationError::SingleWork,
            ),
            (
                SchemaWorkLimits::new(7, 0, 11),
                SchemaEvaluationError::AggregateWork,
            ),
            (
                SchemaWorkLimits::new(7, 19, 0),
                SchemaEvaluationError::Scratch,
            ),
        ] {
            assert_refused(
                &mut SchemaWorkBudget::new(limits),
                Ok(cost(1, 1)),
                false,
                error,
            );
        }
    }

    #[test]
    fn zero_scratch_cost_fits_a_zero_scratch_allowance() {
        let mut budget = SchemaWorkBudget::new(SchemaWorkLimits::new(7, 19, 0));
        assert_eq!(budget.evaluate(Ok(cost(7, 0)), || false, || true), Ok(true));
        assert_eq!(budget.remaining(), 12);
    }

    #[test]
    fn expired_deadline_preserves_previously_debited_balance() {
        let mut budget = budget();
        assert_eq!(budget.evaluate(Ok(cost(7, 3)), || false, || true), Ok(true));
        assert_eq!(budget.remaining(), 12);
        assert_refused(
            &mut budget,
            Ok(cost(7, 3)),
            true,
            SchemaEvaluationError::Deadline,
        );
        assert_eq!(budget.remaining(), 12);
    }

    #[test]
    fn expired_deadline_wins_over_each_resource_refusal() {
        for projected in [
            Err(SchemaCostError::Overflow),
            Ok(cost(0, 1)),
            Ok(cost(8, 1)),
            Ok(cost(3, 12)),
            Ok(cost(6, 1)),
        ] {
            let mut budget = budget();
            for _ in 0..2_u8 {
                assert_eq!(budget.evaluate(Ok(cost(7, 1)), || false, || true), Ok(true));
            }
            assert_eq!(budget.remaining(), 5);
            assert_refused(
                &mut budget,
                projected,
                true,
                SchemaEvaluationError::Deadline,
            );
            assert_eq!(budget.remaining(), 5);
        }
    }

    #[test]
    fn clock_is_checked_before_each_validator_call() {
        let mut budget = budget();
        let clock_checks = Cell::new(0_usize);
        let validations = Cell::new(0_usize);
        for (expected, remaining) in [(Ok(true), 12), (Err(SchemaEvaluationError::Deadline), 12)] {
            let result = budget.evaluate(
                Ok(cost(7, 3)),
                || {
                    let expired = clock_checks.get() != 0;
                    clock_checks.set(clock_checks.get().checked_add(1).unwrap());
                    expired
                },
                || {
                    assert_eq!(clock_checks.get(), 1);
                    validations.set(validations.get().checked_add(1).unwrap());
                    true
                },
            );
            assert_eq!(result, expected);
            assert_eq!(budget.remaining(), remaining);
        }
        assert_eq!(clock_checks.get(), 2);
        assert_eq!(validations.get(), 1);
    }

    #[test]
    fn clock_and_validator_can_consume_their_captures_once() {
        let mut budget = budget();
        let clock_capture = String::from("original clock owner");
        let validator_capture = String::from("borrowed instance owner");
        let result = budget.evaluate(
            Ok(cost(7, 3)),
            move || {
                drop(clock_capture);
                false
            },
            move || {
                drop(validator_capture);
                true
            },
        );
        assert_eq!(result, Ok(true));
        assert_eq!(budget.remaining(), 12);
    }

    #[test]
    fn panicking_validator_keeps_its_pre_call_charge() {
        let mut budget = budget();
        let mut calls = 0_usize;
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.evaluate(
                Ok(cost(7, 11)),
                || false,
                || {
                    calls = calls.checked_add(1).unwrap();
                    panic!("validator panic");
                },
            )
        }));
        let panic = result.unwrap_err();
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"validator panic"));
        assert_eq!(calls, 1);
        assert_eq!(budget.remaining(), 12);
        assert_eq!(
            budget.evaluate(Ok(cost(7, 11)), || false, || true),
            Ok(true)
        );
        assert_eq!(budget.remaining(), 5);
        assert_refused(
            &mut budget,
            Ok(cost(6, 11)),
            false,
            SchemaEvaluationError::AggregateWork,
        );
    }

    #[test]
    fn maximum_scalar_work_charge_does_not_wrap() {
        let mut budget = SchemaWorkBudget::new(SchemaWorkLimits::new(u64::MAX, u64::MAX, 11));
        assert_eq!(budget.remaining(), u64::MAX);
        assert_eq!(
            budget.evaluate(Ok(cost(u64::MAX, 11)), || false, || true),
            Ok(true)
        );
        assert_eq!(budget.remaining(), 0);
        assert_refused(
            &mut budget,
            Ok(cost(1, 11)),
            false,
            SchemaEvaluationError::AggregateWork,
        );
    }
}

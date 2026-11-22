//! Controlled-native admission using tested reservations, not worst-case proofs.

#[cfg(not(target_arch = "wasm32"))]
use super::{
    SchemaCostError, SchemaEvaluationCost, SchemaEvaluationError, SchemaInputLimits,
    measure_schema_input,
};
use super::{SchemaWorkBudget, SchemaWorkLimits};
use auctioneer_runtime::CatalogResourceCosts;
use serde_json::Value;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

pub const NATIVE_CATALOG_RESIDENT_BYTES: u64 = 16 << 20;
pub const NATIVE_CATALOG_INITIALIZATION_BYTES: u64 = 8 << 20;
pub const NATIVE_SCHEMA_SCRATCH_BYTES: u64 = 512 << 10;
const SINGLE_WORK: u64 = 0x0040_0000;
const AUCTION_WORK: u64 = 0x0400_0000;

/// Add the frozen native reservations without replacing existing context costs.
#[must_use]
pub const fn controlled_native_costs(mut costs: CatalogResourceCosts) -> CatalogResourceCosts {
    costs.resident_bytes = NATIVE_CATALOG_RESIDENT_BYTES;
    costs.initialization_bytes = NATIVE_CATALOG_INITIALIZATION_BYTES;
    costs.schema_validation_per_auction_bytes = NATIVE_SCHEMA_SCRATCH_BYTES;
    costs
}

#[must_use]
pub const fn native_schema_funded(costs: CatalogResourceCosts) -> bool {
    costs.resident_bytes >= NATIVE_CATALOG_RESIDENT_BYTES
        && costs.initialization_bytes >= NATIVE_CATALOG_INITIALIZATION_BYTES
        && costs.schema_validation_per_auction_bytes >= NATIVE_SCHEMA_SCRATCH_BYTES
}

#[cfg(not(target_arch = "wasm32"))]
static INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Prewarm only after installing the frozen reservation. Failure is cached.
#[must_use]
pub fn initialize_native_schemas(costs: CatalogResourceCosts) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        initialize_native_schemas_with_cell(
            costs,
            cfg!(feature = "schema-artifact-staging"),
            &INITIALIZED,
            super::staging::initialize_native,
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _: CatalogResourceCosts = costs;
        false
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn initialize_native_schemas_with_cell(
    costs: CatalogResourceCosts,
    staging_only: bool,
    cell: &OnceLock<bool>,
    initialize: impl FnOnce() -> bool,
) -> bool {
    if !native_schema_funded(costs) || staging_only {
        return false;
    }
    *cell.get_or_init(initialize)
}

#[must_use]
pub fn native_schema_required(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        super::staging::native_required(name)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _: &str = name;
        false
    }
}

/// Selected public schemas, including legacy definitions on a funded native boot.
#[must_use]
pub fn native_schema_available(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        super::staging::native_owner(name).is_some()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _: &str = name;
        false
    }
}

/// One sequential evaluation allowance; the caller shares it across an auction.
#[must_use]
pub const fn native_schema_work_budget(funded_scratch: u64) -> SchemaWorkBudget {
    SchemaWorkBudget::new(SchemaWorkLimits::new(
        SINGLE_WORK,
        AUCTION_WORK,
        funded_scratch,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeSchemaError {
    InvalidParameters,
    Unavailable,
    Resource,
    Deadline,
}

/// Validate the final borrowed subtree without cloning or retaining it.
///
/// Work tokens are a predeclared admission policy (1024 + 16/byte + 128/node),
/// not a proven regex instruction or wall-clock bound. Full certification is resource qualification.
///
/// # Errors
/// Refuses missing funding/initialization, oversized inputs, exhausted work,
/// invalid parameters and the original deadline, without exposing input values.
pub fn validate_native_parameters<F>(
    name: &str,
    value: &Value,
    budget: &mut SchemaWorkBudget,
    mut expired: F,
) -> Result<(), NativeSchemaError>
where
    F: FnMut() -> bool,
{
    if expired() {
        return Err(NativeSchemaError::Deadline);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if INITIALIZED.get() != Some(&true) || cfg!(feature = "schema-artifact-staging") {
            return Err(NativeSchemaError::Unavailable);
        }
        let owner = super::staging::native_owner(name).ok_or(NativeSchemaError::Unavailable)?;
        let stats = measure_schema_input(
            value,
            SchemaInputLimits {
                serialized_bytes: 0x0001_0000,
                nodes: 4_096,
                depth: 32,
            },
        )
        .map_err(|_error| NativeSchemaError::InvalidParameters)?;
        let work = stats
            .serialized_bytes
            .checked_mul(16)
            .and_then(|bytes| {
                let nodes = stats.nodes.checked_mul(128)?;
                bytes.checked_add(nodes)
            })
            .and_then(|units| units.checked_add(1_024));
        let cost = work
            .map(|work_units| SchemaEvaluationCost {
                work_units,
                scratch_bytes: NATIVE_SCHEMA_SCRATCH_BYTES,
            })
            .ok_or(SchemaCostError::Overflow);
        let mut available = true;
        let valid = budget
            .evaluate(cost, &mut expired, || {
                super::staging::validate_native(owner, value).unwrap_or_else(|| {
                    available = false;
                    false
                })
            })
            .map_err(|error| match error {
                SchemaEvaluationError::Deadline => NativeSchemaError::Deadline,
                SchemaEvaluationError::CostOverflow
                | SchemaEvaluationError::ZeroWork
                | SchemaEvaluationError::SingleWork
                | SchemaEvaluationError::Scratch
                | SchemaEvaluationError::AggregateWork => NativeSchemaError::Resource,
            })?;
        if expired() {
            return Err(NativeSchemaError::Deadline);
        }
        if !available {
            return Err(NativeSchemaError::Unavailable);
        }
        if valid {
            Ok(())
        } else {
            Err(NativeSchemaError::InvalidParameters)
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _: (&str, &Value, &mut SchemaWorkBudget) = (name, value, budget);
        Err(NativeSchemaError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn native_reservations_are_independent_and_preserve_context() {
        let costs = controlled_native_costs(CatalogResourceCosts {
            context_per_auction_bytes: 777,
            ..CatalogResourceCosts::UNCHANGED
        });
        assert_eq!(costs.resident_bytes, 0x0100_0000);
        assert_eq!(costs.initialization_bytes, 0x0080_0000);
        assert_eq!(costs.schema_validation_per_auction_bytes, 0x0008_0000);
        assert_eq!(costs.context_per_auction_bytes, 777);
        assert!(native_schema_funded(costs));
        for unfunded in [
            CatalogResourceCosts {
                resident_bytes: 0x00ff_ffff,
                ..costs
            },
            CatalogResourceCosts {
                initialization_bytes: 0x007f_ffff,
                ..costs
            },
            CatalogResourceCosts {
                schema_validation_per_auction_bytes: 0x0007_ffff,
                ..costs
            },
        ] {
            assert!(!native_schema_funded(unfunded));
            assert!(!initialize_native_schemas(unfunded));
        }
    }

    #[test]
    fn expired_native_validation_refuses_before_unknown_owner_or_initialization() {
        let mut budget = native_schema_work_budget(0x0008_0000);
        assert_eq!(
            validate_native_parameters("fake_missing", &json!({}), &mut budget, || true),
            Err(NativeSchemaError::Deadline)
        );
        assert_eq!(budget.remaining(), 0x0400_0000);
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod fresh_initialization {
        use super::super::{controlled_native_costs, initialize_native_schemas_with_cell};
        use auctioneer_runtime::CatalogResourceCosts;
        use regex_automata::{
            MatchKind,
            meta::{BuildError, Regex},
            util::syntax,
        };
        use std::{
            array,
            cell::{Cell, RefCell},
            iter::repeat_with,
            sync::{
                Arc, Barrier, OnceLock, Weak,
                atomic::{AtomicUsize, Ordering},
            },
            thread,
        };

        fn build_program(pattern: &str) -> Result<Arc<Regex>, Box<BuildError>> {
            Regex::builder()
                .configure(
                    Regex::config()
                        .match_kind(MatchKind::LeftmostFirst)
                        .utf8_empty(true)
                        .nfa_size_limit(Some(0x0001_0000_usize))
                        .hybrid_cache_capacity(0x0001_0000_usize)
                        .pool_capacity(1_usize),
                )
                .syntax(syntax::Config::default().utf8(true))
                .build(pattern)
                .map(Arc::new)
                .map_err(Box::new)
        }

        #[test]
        fn fresh_success_retains_programs_in_order_and_repeat_does_not_construct() {
            let costs = controlled_native_costs(CatalogResourceCosts::UNCHANGED);
            let cell = OnceLock::new();
            let programs: [OnceLock<Option<Arc<Regex>>>; 3] = array::from_fn(|_| OnceLock::new());
            let trace = RefCell::new(Vec::new());
            assert!(cell.get().is_none());
            assert!(programs.iter().all(|program| program.get().is_none()));

            assert!(initialize_native_schemas_with_cell(
                costs,
                false,
                &cell,
                || {
                    for (program, pattern, name) in [
                        (&programs[0], "^shared[0-9]+$", "shared-format"),
                        (&programs[1], "^owner-a[0-9]+$", "owner-a"),
                        (&programs[2], "^owner-b[0-9]+$", "owner-b"),
                    ] {
                        if program
                            .get_or_init(|| {
                                trace.borrow_mut().push(name);
                                Some(build_program(pattern).unwrap())
                            })
                            .is_none()
                        {
                            return false;
                        }
                    }
                    true
                }
            ));
            assert_eq!(cell.get(), Some(&true));
            assert_eq!(
                trace.borrow().as_slice(),
                &["shared-format", "owner-a", "owner-b"]
            );
            let witnesses = programs
                .iter()
                .map(|program| {
                    let retained = program.get().unwrap().as_ref().unwrap();
                    assert!(retained.memory_usage() > 0_usize);
                    Arc::downgrade(retained)
                })
                .collect::<Vec<_>>();
            assert!(witnesses.iter().all(|witness| witness.upgrade().is_some()));
            assert!(initialize_native_schemas_with_cell(
                costs,
                false,
                &cell,
                || { panic!("successful initialization must not repeat constructors") }
            ));
            assert_eq!(trace.borrow().len(), 3_usize);
            drop(programs);
            assert!(witnesses.iter().all(|witness| witness.upgrade().is_none()));
        }

        #[test]
        fn prelive_prefix_success_constructs_only_later_program_and_repeat_reuses_both() {
            let costs = controlled_native_costs(CatalogResourceCosts::UNCHANGED);
            let cell = OnceLock::new();
            let prefix = OnceLock::<Option<Arc<Regex>>>::new();
            let later = OnceLock::<Option<Arc<Regex>>>::new();
            let prefix_attempts = Cell::new(0_usize);
            let later_attempts = Cell::new(0_usize);
            let prefix_witness = {
                let retained = prefix
                    .get_or_init(|| {
                        prefix_attempts.set(prefix_attempts.get() + 1_usize);
                        Some(build_program("^live[0-9]+$").unwrap())
                    })
                    .as_ref()
                    .unwrap();
                assert!(retained.memory_usage() > 0_usize);
                Arc::downgrade(retained)
            };
            assert_eq!(prefix_attempts.get(), 1_usize);
            assert_eq!(later_attempts.get(), 0_usize);
            assert!(prefix_witness.upgrade().is_some());
            assert!(cell.get().is_none());
            assert!(later.get().is_none());

            assert!(initialize_native_schemas_with_cell(
                costs,
                false,
                &cell,
                || {
                    assert!(
                        prefix
                            .get_or_init(|| panic!("already-live prefix must not reconstruct"))
                            .is_some()
                    );
                    assert!(prefix_witness.upgrade().is_some());
                    later
                        .get_or_init(|| {
                            later_attempts.set(later_attempts.get() + 1_usize);
                            Some(build_program("^later[0-9]+$").unwrap())
                        })
                        .is_some()
                }
            ));
            assert_eq!(cell.get(), Some(&true));
            assert_eq!(prefix_attempts.get(), 1_usize);
            assert_eq!(later_attempts.get(), 1_usize);
            let later_witness = {
                let retained = later.get().unwrap().as_ref().unwrap();
                assert!(retained.memory_usage() > 0_usize);
                Arc::downgrade(retained)
            };
            assert!(prefix_witness.upgrade().is_some());
            assert!(later_witness.upgrade().is_some());
            assert!(initialize_native_schemas_with_cell(
                costs,
                false,
                &cell,
                || panic!("successful initialization must not repeat constructors")
            ));
            assert_eq!(cell.get(), Some(&true));
            assert_eq!(prefix_attempts.get(), 1_usize);
            assert_eq!(later_attempts.get(), 1_usize);
            drop(later);
            assert!(later_witness.upgrade().is_none());
            assert!(prefix_witness.upgrade().is_some());
            drop(prefix);
            assert!(prefix_witness.upgrade().is_none());
        }

        #[test]
        fn fresh_late_failure_keeps_live_prefix_drops_temporary_and_caches_failure() {
            let costs = controlled_native_costs(CatalogResourceCosts::UNCHANGED);
            let cell = OnceLock::new();
            let prefix = OnceLock::<Option<Arc<Regex>>>::new();
            prefix
                .set(Some(build_program("^live[0-9]+$").unwrap()))
                .unwrap();
            let prefix_witness = Arc::downgrade(prefix.get().unwrap().as_ref().unwrap());
            let failing = OnceLock::<Option<Arc<Regex>>>::new();
            let attempts = Cell::new(0_usize);
            let temporary_witness = RefCell::new(None::<Weak<Regex>>);
            assert!(prefix_witness.upgrade().is_some());
            assert!(cell.get().is_none());
            assert!(failing.get().is_none());

            assert!(!initialize_native_schemas_with_cell(
                costs,
                false,
                &cell,
                || {
                    assert!(
                        prefix
                            .get_or_init(|| panic!("already-live prefix must not reconstruct"))
                            .is_some()
                    );
                    failing
                        .get_or_init(|| {
                            attempts.set(attempts.get() + 1_usize);
                            let temporary = build_program("^temporary[0-9]+$").unwrap();
                            assert!(temporary.memory_usage() > 0_usize);
                            *temporary_witness.borrow_mut() = Some(Arc::downgrade(&temporary));
                            // Fixed production-sized NFA cap, supported non-syntax
                            // failure. This is not an admitted generated pattern.
                            let Err(error) = build_program("^[ab]{65536}$") else {
                                panic!("large bounded construction must fail");
                            };
                            assert_eq!(error.size_limit(), Some(0x0001_0000_usize));
                            assert!(error.syntax_error().is_none());
                            assert!(temporary.is_match("temporary7"));
                            None
                        })
                        .is_some()
                }
            ));
            assert_eq!(cell.get(), Some(&false));
            assert!(matches!(failing.get(), Some(None)));
            assert_eq!(attempts.get(), 1_usize);
            // Weak witnesses our real temporary program's release, not the
            // failed builder's internal allocation count or peak.
            assert!(
                temporary_witness
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .upgrade()
                    .is_none()
            );
            assert!(prefix_witness.upgrade().is_some());
            assert!(!initialize_native_schemas_with_cell(
                costs,
                false,
                &cell,
                || { panic!("cached failed initialization must not retry constructors") }
            ));
            assert_eq!(attempts.get(), 1_usize);
            assert!(prefix_witness.upgrade().is_some());
            drop(prefix);
            assert!(prefix_witness.upgrade().is_none());
        }

        #[test]
        fn fresh_concurrent_callers_run_one_real_constructor_sequence() {
            let costs = controlled_native_costs(CatalogResourceCosts::UNCHANGED);
            let cell = OnceLock::new();
            let program = OnceLock::<Arc<Regex>>::new();
            let attempts = AtomicUsize::new(0_usize);
            let barrier = Barrier::new(4_usize);
            assert!(cell.get().is_none());
            assert!(program.get().is_none());

            let outcomes = thread::scope(|scope| {
                let handles = repeat_with(|| {
                    scope.spawn(|| {
                        barrier.wait();
                        initialize_native_schemas_with_cell(costs, false, &cell, || {
                            attempts.fetch_add(1_usize, Ordering::SeqCst);
                            let retained = program
                                .get_or_init(|| build_program("^concurrent[0-9]+$").unwrap());
                            retained.is_match("concurrent7")
                        })
                    })
                })
                .take(4_usize)
                .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .map(|handle| handle.join().unwrap())
                    .collect::<Vec<_>>()
            });
            assert_eq!(outcomes, vec![true; 4_usize]);
            assert_eq!(attempts.load(Ordering::SeqCst), 1_usize);
            assert_eq!(cell.get(), Some(&true));
            let witness = Arc::downgrade(program.get().unwrap());
            assert!(witness.upgrade().is_some());
            drop(program);
            assert!(witness.upgrade().is_none());
        }

        #[test]
        fn fresh_funding_and_staging_refusals_precede_cell_access_and_cold_construction() {
            let funded = controlled_native_costs(CatalogResourceCosts::UNCHANGED);
            for (costs, staging_only) in [
                (
                    CatalogResourceCosts {
                        resident_bytes: 0x00ff_ffff_u64,
                        ..funded
                    },
                    false,
                ),
                (
                    CatalogResourceCosts {
                        initialization_bytes: 0x007f_ffff_u64,
                        ..funded
                    },
                    false,
                ),
                (
                    CatalogResourceCosts {
                        schema_validation_per_auction_bytes: 0x0007_ffff_u64,
                        ..funded
                    },
                    false,
                ),
                (funded, true),
            ] {
                let cell = OnceLock::new();
                assert!(!initialize_native_schemas_with_cell(
                    costs,
                    staging_only,
                    &cell,
                    || panic!("refused admission must not run cold constructors"),
                ));
                assert!(cell.get().is_none());
                for previous in [false, true] {
                    let cached = OnceLock::new();
                    cached.set(previous).unwrap();
                    assert!(!initialize_native_schemas_with_cell(
                        costs,
                        staging_only,
                        &cached,
                        || panic!("refused admission must not consult a cached constructor"),
                    ));
                    assert_eq!(cached.get(), Some(&previous));
                }
                let program = OnceLock::<Arc<Regex>>::new();
                assert!(initialize_native_schemas_with_cell(
                    funded,
                    false,
                    &cell,
                    || {
                        program
                            .set(build_program("^funded[0-9]+$").unwrap())
                            .unwrap();
                        true
                    }
                ));
                assert_eq!(cell.get(), Some(&true));
                assert!(program.get().unwrap().memory_usage() > 0_usize);
            }
        }
    }
}

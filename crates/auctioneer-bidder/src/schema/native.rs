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
    if !native_schema_funded(costs) || cfg!(feature = "schema-artifact-staging") {
        return false;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        *INITIALIZED.get_or_init(super::staging::initialize_native)
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
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
}

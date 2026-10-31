//! Auctioneer's adaptation boundary to the pinned SDK memory validator.

use auctioneer_runtime::{HeadroomClass, minimum_headroom_bytes};
use edgezero_core::{
    MemoryCeiling, MemoryCeilingScope, MemoryEnvelope, MemoryEnvelopeValidation,
    MemoryEnvelopeValidationError, PlatformMetadata,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

use super::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StackRequirement {
    pub bytes: u64,
    pub proof: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Input {
    pub total: u64,
    pub configured: u64,
    pub class: HeadroomClass,
    pub scope: Option<MemoryCeilingScope>,
    pub population: u32,
    pub headroom: Option<u64>,
    pub stack: Option<StackRequirement>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Validation {
    #[serde(skip)]
    pub sdk: Option<MemoryEnvelopeValidation>,
    #[serde(skip)]
    pub sdk_error: Option<MemoryEnvelopeValidationError>,
    pub sdk_verdict: SdkResult,
    pub sdk_result: String,
    pub external_blockers: Vec<String>,
    pub minimum_headroom_bytes: Option<u64>,
    pub selected_headroom_bytes: Option<u64>,
    pub effective_ceiling_bytes: Option<u64>,
    pub gap_bytes: Option<u64>,
    pub configured_cap_exceeded: bool,
    pub population_covered: bool,
    pub platform_cap_exceeded: bool,
    pub profile_matches_dispatch: bool,
    pub required_fit: bool,
    pub separate_stack_allowance_bytes: Option<u64>,
    pub separate_stack_requirement: Option<StackRequirement>,
}

impl Validation {
    pub(super) fn canonical_sdk_result(&self) -> String {
        sdk_description(self.sdk.as_ref(), self.sdk_error)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SdkResult {
    Fits {
        available_primary_bytes: u64,
        available_separate_stack_bytes: Option<u64>,
        required_primary_bytes: u64,
        required_separate_stack_bytes: Option<u64>,
    },
    Exceeds {
        available_bytes: u64,
        required_bytes: u64,
        resource: String,
    },
    Indeterminate {
        available_primary_bytes: Option<u64>,
        minimum_required_primary_bytes: u64,
        reasons: Vec<String>,
    },
    Invalid {
        detail: String,
    },
    Pending(#[serde(with = "super::empty_object")] ()),
    Unrecognized {
        detail: String,
    },
}

fn sdk_description(
    result: Option<&MemoryEnvelopeValidation>,
    error: Option<MemoryEnvelopeValidationError>,
) -> String {
    error.map_or_else(
        || {
            result.map_or_else(
                || "pending_unknown_scope".to_owned(),
                |value| format!("{value:?}"),
            )
        },
        |value| format!("{value:?}"),
    )
}

fn sdk_export(
    input: Option<&MemoryEnvelopeValidation>,
    error: Option<MemoryEnvelopeValidationError>,
) -> SdkResult {
    if error.is_some() {
        return SdkResult::Invalid {
            detail: sdk_description(None, error),
        };
    }
    match input {
        Some(MemoryEnvelopeValidation::Fits {
            available_primary_bytes,
            available_separate_stack_bytes,
            required_primary_bytes,
            required_separate_stack_bytes,
        }) => SdkResult::Fits {
            available_primary_bytes: *available_primary_bytes,
            available_separate_stack_bytes: *available_separate_stack_bytes,
            required_primary_bytes: *required_primary_bytes,
            required_separate_stack_bytes: *required_separate_stack_bytes,
        },
        Some(MemoryEnvelopeValidation::Exceeds {
            available_bytes,
            required_bytes,
            resource,
        }) => SdkResult::Exceeds {
            available_bytes: *available_bytes,
            required_bytes: *required_bytes,
            resource: format!("{resource:?}"),
        },
        Some(MemoryEnvelopeValidation::Indeterminate {
            available_primary_bytes,
            minimum_required_primary_bytes,
            reasons,
        }) => SdkResult::Indeterminate {
            available_primary_bytes: *available_primary_bytes,
            minimum_required_primary_bytes: *minimum_required_primary_bytes,
            reasons: reasons.iter().map(|reason| format!("{reason:?}")).collect(),
        },
        None => SdkResult::Pending(()),
        Some(unrecognized) => SdkResult::Unrecognized {
            detail: format!("{unrecognized:?}"),
        },
    }
}

pub(super) fn evaluate(platform: PlatformMetadata, input: Input) -> Result<Validation, Error> {
    let ceiling = platform.memory_ceiling().value();
    let population = platform.inbound_request_population_bound().value();
    let scope_matches = input
        .scope
        .zip(ceiling)
        .is_some_and(|(scope, cap)| scope == cap.scope());
    let effective_ceiling_bytes = ceiling
        .filter(|_cap| scope_matches)
        .map(|cap| cap.total_bytes().min(input.configured));
    let minimum_headroom_bytes = effective_ceiling_bytes
        .and_then(NonZeroU64::new)
        .map(|cap| minimum_headroom_bytes(cap, input.class));
    if input.configured == 0
        || effective_ceiling_bytes == Some(0)
        || input
            .headroom
            .zip(minimum_headroom_bytes)
            .is_some_and(|(selected, floor)| selected < floor)
    {
        return Err(Error::ResourceInput);
    }
    let selected_headroom_bytes = input.headroom.or(minimum_headroom_bytes);
    let required = input
        .total
        .checked_add(selected_headroom_bytes.unwrap_or(0))
        .ok_or(Error::ResourceInput)?;
    let mut external_blockers = Vec::new();
    if ceiling.is_none() {
        external_blockers.push("unknown_memory_ceiling");
    }
    if population.is_none() {
        external_blockers.push("unknown_inbound_request_population");
    }
    if platform.host_ingress_memory_accounting().value().is_none() {
        external_blockers.push("unknown_host_ingress_memory_accounting");
    }
    if input.scope.is_none() {
        external_blockers.push("unknown_scope");
    }
    let stack = input.stack.as_ref().filter(|requirement| {
        requirement.bytes != 0
            && requirement
                .proof
                .as_ref()
                .is_some_and(|proof| !proof.is_empty())
    });
    if ceiling.is_some_and(|cap| cap.stack_bytes().is_some()) && stack.is_none() {
        external_blockers.push("missing_proved_nonzero_separate_stack_requirement");
    }
    let population_covered =
        population.is_some_and(|bound| input.population >= bound.max_live_requests().get());
    let validation = input.scope.map(|scope| {
        platform.validate_memory_envelope(MemoryEnvelope::new(
            scope,
            required,
            0,
            stack.map(|requirement| requirement.bytes),
        ))
    });
    let (sdk, sdk_error) = match validation {
        Some(Ok(result)) => (Some(result), None),
        Some(Err(MemoryEnvelopeValidationError::ArithmeticOverflow { .. })) => {
            return Err(Error::ResourceInput);
        }
        Some(Err(error)) => (None, Some(error)),
        None => (None, None),
    };
    let configured_cap_exceeded = required > input.configured;
    let platform_cap_exceeded = ceiling.is_some_and(|cap| required > cap.total_bytes());
    let required_fit = matches!(sdk, Some(MemoryEnvelopeValidation::Fits { .. }))
        && population_covered
        && scope_matches
        && minimum_headroom_bytes.is_some()
        && !configured_cap_exceeded
        && !platform_cap_exceeded
        && external_blockers.is_empty();
    Ok(Validation {
        sdk_verdict: sdk_export(sdk.as_ref(), sdk_error),
        sdk_result: sdk_description(sdk.as_ref(), sdk_error),
        sdk,
        sdk_error,
        external_blockers: external_blockers.into_iter().map(str::to_owned).collect(),
        minimum_headroom_bytes,
        selected_headroom_bytes,
        effective_ceiling_bytes,
        gap_bytes: effective_ceiling_bytes.map(|cap| required.saturating_sub(cap)),
        configured_cap_exceeded,
        population_covered,
        platform_cap_exceeded,
        profile_matches_dispatch: true,
        required_fit,
        separate_stack_allowance_bytes: ceiling.and_then(MemoryCeiling::stack_bytes),
        separate_stack_requirement: input.stack,
    })
}

#[cfg(test)]
mod tests {
    use super::{Error, Input, SdkResult, StackRequirement, evaluate};
    use auctioneer_runtime::HeadroomClass;
    use edgezero_core::{
        HostIngressMemoryAccounting, InboundRequestPopulationBound, MemoryCeiling,
        MemoryCeilingScope, MemoryEnvelopeValidation, MemoryEnvelopeValidationError,
        MemoryResource, PlatformFact, PlatformMetadata, PlatformResourceSource,
        PlatformUnknownReason,
    };
    use std::num::NonZeroU32;

    const SCOPE: MemoryCeilingScope = MemoryCeilingScope::PerInstance;

    fn known<T>(value: T) -> PlatformFact<T> {
        PlatformFact::known(
            value,
            PlatformResourceSource::PlatformLimit {
                provider: "fixture",
            },
        )
    }

    fn platform(ceiling: u64, stack: Option<u64>, population: u32) -> PlatformMetadata {
        PlatformMetadata::new(
            known(MemoryCeiling::new(ceiling, SCOPE, stack)),
            known(InboundRequestPopulationBound::new(
                NonZeroU32::new(population).unwrap(),
            )),
            known(HostIngressMemoryAccounting::CountsTowardCeiling),
        )
    }

    fn input() -> Input {
        Input {
            total: 80_000_000,
            configured: 100_000_001,
            class: HeadroomClass::Wasm,
            scope: Some(SCOPE),
            population: 3,
            headroom: None,
            stack: None,
        }
    }

    fn assert_invalid(facts: PlatformMetadata, row: Input, error: MemoryEnvelopeValidationError) {
        let result = evaluate(facts, row).unwrap();
        assert!(!result.required_fit);
        assert_eq!(result.sdk_error, Some(error));
        assert_eq!(result.canonical_sdk_result(), result.sdk_result);
        assert_eq!(result.sdk_result, format!("{error:?}"));
        let value = serde_json::to_value(&result).unwrap();
        assert_eq!(value["sdk_verdict"]["status"], "invalid");
        assert_eq!(value["sdk_verdict"]["detail"], format!("{error:?}"));
    }

    #[test]
    fn scope_mismatch_is_reported_as_nonpassing_sdk_invalid() {
        assert_invalid(
            platform(100_000_001, None, 3),
            Input {
                scope: Some(MemoryCeilingScope::PerExecution),
                ..input()
            },
            MemoryEnvelopeValidationError::ScopeMismatch {
                ceiling_scope: SCOPE,
                envelope_scope: MemoryCeilingScope::PerExecution,
            },
        );
    }

    #[test]
    fn unexpected_separate_stack_is_reported_as_nonpassing_sdk_invalid() {
        assert_invalid(
            platform(100_000_001, None, 3),
            Input {
                stack: Some(StackRequirement {
                    bytes: 1,
                    proof: Some("fixture-artifact-abi-toolchain-paths".to_owned()),
                }),
                ..input()
            },
            MemoryEnvelopeValidationError::UnexpectedSeparateStackRequirement { required_bytes: 1 },
        );
    }

    #[test]
    fn nonunit_execution_population_is_reported_as_nonpassing_sdk_invalid() {
        let facts = platform(100_000_001, None, 3);
        let facts_execution = PlatformMetadata::new(
            known(MemoryCeiling::new(
                100_000_001,
                MemoryCeilingScope::PerExecution,
                None,
            )),
            facts.inbound_request_population_bound(),
            facts.host_ingress_memory_accounting(),
        );
        assert_invalid(
            facts_execution,
            Input {
                scope: Some(MemoryCeilingScope::PerExecution),
                ..input()
            },
            MemoryEnvelopeValidationError::NonUnitPerExecutionPopulation {
                max_live_requests: NonZeroU32::new(3).unwrap(),
            },
        );
    }

    #[test]
    fn known_instance_total_is_adapted_once_with_rounded_headroom() {
        let result = evaluate(platform(100_000_001, None, 3), input()).unwrap();
        assert_eq!(result.minimum_headroom_bytes, Some(6_250_001));
        assert_eq!(result.selected_headroom_bytes, Some(6_250_001));
        assert_eq!(result.gap_bytes, Some(0));
        assert!(result.required_fit);
        assert_eq!(
            result.sdk,
            Some(MemoryEnvelopeValidation::Fits {
                available_primary_bytes: 100_000_001,
                available_separate_stack_bytes: None,
                required_primary_bytes: 86_250_001,
                required_separate_stack_bytes: None,
            })
        );
    }

    #[test]
    fn separate_stack_requires_nonzero_proved_bound_and_exact_decimal_allowance() {
        let facts = platform(100_000_001, Some(1_000_000), 3);
        for stack in [
            None,
            Some(StackRequirement {
                bytes: 0,
                proof: Some("fixture-artifact-abi-toolchain-paths".to_owned()),
            }),
            Some(StackRequirement {
                bytes: 999_999,
                proof: None,
            }),
        ] {
            let result = evaluate(facts, Input { stack, ..input() }).unwrap();
            assert!(!result.required_fit);
            assert!(!result.external_blockers.is_empty());
            assert!(matches!(
                result.sdk,
                Some(MemoryEnvelopeValidation::Indeterminate { .. })
            ));
        }
        for (bytes, fits) in [(999_999, true), (1_000_000, true), (1_000_001, false)] {
            let result = evaluate(
                facts,
                Input {
                    stack: Some(StackRequirement {
                        bytes,
                        proof: Some("fixture-artifact-abi-toolchain-paths".to_owned()),
                    }),
                    ..input()
                },
            )
            .unwrap();
            assert_eq!(result.required_fit, fits);
            if !fits {
                assert_eq!(
                    result.sdk,
                    Some(MemoryEnvelopeValidation::Exceeds {
                        available_bytes: 1_000_000,
                        required_bytes: 1_000_001,
                        resource: MemoryResource::SeparateStack,
                    })
                );
            }
        }
    }

    #[test]
    fn configured_failure_and_external_unknowns_are_independent() {
        let unknown = PlatformFact::unknown(PlatformUnknownReason::ProviderUnpublished);
        let facts = PlatformMetadata::new(
            unknown,
            PlatformFact::unknown(PlatformUnknownReason::ProviderUnpublished),
            PlatformFact::unknown(PlatformUnknownReason::ProviderUnpublished),
        );
        for (total, failed) in [
            (89_999_999, false),
            (90_000_000, false),
            (90_000_001, true),
            (100_000_001, true),
        ] {
            let result = evaluate(
                facts,
                Input {
                    total,
                    configured: 100_000_000,
                    headroom: Some(10_000_000),
                    ..input()
                },
            )
            .unwrap();
            assert_eq!(result.configured_cap_exceeded, failed);
            assert_eq!(result.minimum_headroom_bytes, None);
            assert_eq!(result.effective_ceiling_bytes, None);
            assert_eq!(result.gap_bytes, None);
            assert_eq!(result.selected_headroom_bytes, Some(10_000_000));
            assert!(!result.required_fit);
            assert_eq!(result.external_blockers.len(), 3);
            assert!(matches!(
                result.sdk,
                Some(MemoryEnvelopeValidation::Indeterminate { .. })
            ));
        }
    }

    #[test]
    fn unknown_population_ingress_and_scope_cannot_pass_zero_per_live_mapping() {
        let all = platform(100_000_001, None, 3);
        let unknown = PlatformFact::unknown(PlatformUnknownReason::ProviderUnpublished);
        for facts in [
            PlatformMetadata::new(
                all.memory_ceiling(),
                unknown,
                all.host_ingress_memory_accounting(),
            ),
            PlatformMetadata::new(
                all.memory_ceiling(),
                all.inbound_request_population_bound(),
                PlatformFact::unknown(PlatformUnknownReason::ProviderUnpublished),
            ),
        ] {
            let result = evaluate(facts, input()).unwrap();
            assert!(!result.required_fit);
            assert!(!result.external_blockers.is_empty());
        }
        let result = evaluate(
            all,
            Input {
                scope: None,
                ..input()
            },
        )
        .unwrap();
        assert!(!result.required_fit);
        assert_eq!(result.sdk, None);
        assert_eq!(result.minimum_headroom_bytes, None);
        assert_eq!(result.gap_bytes, None);
        let uncovered = evaluate(
            all,
            Input {
                population: 2,
                ..input()
            },
        )
        .unwrap();
        assert!(!uncovered.population_covered);
        assert!(!uncovered.required_fit);
        let mismatched = evaluate(
            all,
            Input {
                scope: Some(MemoryCeilingScope::PerExecution),
                ..input()
            },
        )
        .unwrap();
        assert!(!mismatched.required_fit);
        assert!(matches!(mismatched.sdk_verdict, SdkResult::Invalid { .. }));
    }

    #[test]
    fn headroom_and_stack_shape_are_validated_with_both_ceiling_orders() {
        for (configured, published) in [(100_000_001, 120_000_000), (120_000_000, 100_000_001)] {
            let facts = platform(published, None, 3);
            assert_eq!(
                evaluate(
                    facts,
                    Input {
                        configured,
                        headroom: Some(6_250_000),
                        ..input()
                    }
                )
                .unwrap_err(),
                Error::ResourceInput
            );
            assert!(
                evaluate(
                    facts,
                    Input {
                        configured,
                        headroom: Some(6_250_001),
                        ..input()
                    }
                )
                .unwrap()
                .required_fit
            );
            assert!(
                !evaluate(
                    facts,
                    Input {
                        configured,
                        headroom: Some(30_000_000),
                        ..input()
                    }
                )
                .unwrap()
                .required_fit
            );
            let unexpected_stack = evaluate(
                facts,
                Input {
                    configured,
                    stack: Some(StackRequirement {
                        bytes: 1,
                        proof: Some("fixture-artifact-abi-toolchain-paths".to_owned()),
                    }),
                    ..input()
                },
            )
            .unwrap();
            assert!(!unexpected_stack.required_fit);
            assert!(matches!(
                unexpected_stack.sdk_verdict,
                SdkResult::Invalid { .. }
            ));
        }
        assert_eq!(
            evaluate(
                platform(u64::MAX, None, 3),
                Input {
                    total: u64::MAX,
                    configured: u64::MAX,
                    ..input()
                }
            )
            .unwrap_err(),
            Error::ResourceInput
        );
    }

    #[test]
    fn numeric_excess_does_not_hide_external_inventory() {
        let known_facts = platform(100_000_001, Some(1_000_000), 3);
        let facts = PlatformMetadata::new(
            known_facts.memory_ceiling(),
            known_facts.inbound_request_population_bound(),
            PlatformFact::unknown(PlatformUnknownReason::ProviderUnpublished),
        );
        let result = evaluate(
            facts,
            Input {
                total: 100_000_002,
                ..input()
            },
        )
        .unwrap();
        assert!(result.configured_cap_exceeded);
        assert!(!result.external_blockers.is_empty());
        assert!(matches!(
            result.sdk,
            Some(MemoryEnvelopeValidation::Exceeds {
                resource: MemoryResource::Primary,
                ..
            })
        ));
    }
}

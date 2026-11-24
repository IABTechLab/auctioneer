//! Tests-first conditional correspondence fixtures, not generated-source proof.
//!
//! For each source component, the intended premise is V <= a + b*B + c*J.
//! Q = max(ceil(a/1024), ceil(b/16), ceil(c/128)) then bounds admitted work
//! by Q*sum(T), using a maximum across declarations of that same component.
//! Attempts P and external N_clock*H are disjoint from the admitted closure.
//! Q need not be one: it is not a CPU weight or an intrinsic token unit, and
//! establishes neither numerical policy adequacy nor latency qualification.
//! These interfaces neither implement a budget nor issue serving authority.
//! The parent owns module registration, hosted RED and the actual source join;
//! no fixture witness below is authority for an actual generated source.

use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Component {
    ControlVisits,
    BytesScanned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Axis {
    Constant,
    Bytes,
    Nodes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Obligation {
    Operations,
    Ownership,
    Shape,
    Lifetimes,
    Storage,
    StructuralStack,
    Attempts,
    ExternalClock,
    DisjointStages,
}

const OBLIGATIONS: [Obligation; 9] = [
    Obligation::Operations,
    Obligation::Ownership,
    Obligation::Shape,
    Obligation::Lifetimes,
    Obligation::Storage,
    Obligation::StructuralStack,
    Obligation::Attempts,
    Obligation::ExternalClock,
    Obligation::DisjointStages,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetFact {
    ClockWork,
    RequestedStorage,
    RetainedState,
    StackFrame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    MissingCommonProof(Obligation),
    MissingCoefficient(Axis),
    UnwitnessedZero,
    PolicyTuple,
    ComponentMismatch,
    Overflow,
    MissingTargetFact(TargetFact),
    TargetBinding,
    HypotheticalFacts,
    NumericalPolicyNotEvaluated,
    RetiredCandidate,
    NonCanonicalField,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Coefficient {
    Derived { count: u64, witness: &'static str },
    ProvenAbsent { witness: &'static str },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AffineCoefficients {
    component: Component,
    a: Option<Coefficient>,
    b: Option<Coefficient>,
    c: Option<Coefficient>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PolicyTuple {
    constant: u64,
    bytes: u64,
    nodes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ComponentRatio {
    component: Component,
    source_units_per_debit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeclarationFixture {
    owner: Option<&'static str>,
    coefficients: AffineCoefficients,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommonFixture {
    premises: [Option<&'static str>; 9],
    declarations: Vec<DeclarationFixture>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NumericalPolicyAdequacy {
    NotEvaluated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttemptFunding {
    NotEstablished,
}

// This is a conditional fixture result, not a common-source completion token.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ConditionalShape {
    component_maxima: Vec<ComponentRatio>,
    numeric_policy_adequacy: NumericalPolicyAdequacy,
    attempted_work_funding: AttemptFunding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceWork(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RequestedBytes(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameBytes(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct AttemptPopulation {
    attempts: u64,
    per_attempt: SourceWork,
    admitted_work: Vec<SourceWork>,
    admitted_debits: Vec<u64>,
    clock_invocations: u64,
    clock_binding: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalEnvelope {
    component: Component,
    attempts: SourceWork,
    admitted: SourceWork,
    local: SourceWork,
    admitted_policy_bound: SourceWork,
    local_policy_bound: SourceWork,
    attempted_work_funding: AttemptFunding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SuppliedFact<T> {
    Value(T),
    ProvenAbsent { witness: &'static str },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HypotheticalTargetFacts {
    clock_binding: &'static str,
    clock_work: Option<SuppliedFact<SourceWork>>,
    requested_storage: Option<SuppliedFact<RequestedBytes>>,
    retained_state: Option<SuppliedFact<RequestedBytes>>,
    stack_frame: Option<SuppliedFact<FrameBytes>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HypotheticalEvaluation {
    local: LocalEnvelope,
    external_work: SourceWork,
    total_work: SourceWork,
    total_policy_bound: SourceWork,
    requested_storage: RequestedBytes,
    retained_state: RequestedBytes,
    stack_frame: FrameBytes,
    numeric_policy_adequacy: NumericalPolicyAdequacy,
}

// Deliberate RED stubs. No source join, policy reader or arithmetic is claimed.
fn captured_policy_tuple() -> Result<PolicyTuple, Refusal> {
    Err(Refusal::Unsupported)
}

fn check_policy_tuple(_actual: PolicyTuple) -> Result<(), Refusal> {
    Err(Refusal::Unsupported)
}

fn component_ratio(
    _coefficients: &AffineCoefficients,
    _policy: PolicyTuple,
) -> Result<ComponentRatio, Refusal> {
    Err(Refusal::Unsupported)
}

fn declaration_maximum(_ratios: &[ComponentRatio]) -> Result<ComponentRatio, Refusal> {
    Err(Refusal::Unsupported)
}

fn check_common_fixture(_fixture: &CommonFixture) -> Result<ConditionalShape, Refusal> {
    Err(Refusal::MissingCommonProof(Obligation::Operations))
}

fn affine_bound(
    _coefficients: &AffineCoefficients,
    _bytes: u64,
    _nodes: u64,
) -> Result<SourceWork, Refusal> {
    Err(Refusal::Unsupported)
}

fn local_envelope(
    _ratio: ComponentRatio,
    _population: &AttemptPopulation,
) -> Result<LocalEnvelope, Refusal> {
    Err(Refusal::Unsupported)
}

fn evaluate_hypothetical(
    _ratio: ComponentRatio,
    _population: &AttemptPopulation,
    _facts: &HypotheticalTargetFacts,
) -> Result<HypotheticalEvaluation, Refusal> {
    Err(Refusal::Unsupported)
}

// Only refusals are representable here: no qualified receipt or serving result.
fn qualification_refusal(
    _shape: &ConditionalShape,
    _evaluation: Option<&HypotheticalEvaluation>,
) -> Refusal {
    Refusal::Unsupported
}

// In-memory output field check only; this cannot read or return active costs.
fn check_current_report_fields(_report: &Value) -> Result<(), Refusal> {
    Err(Refusal::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ZERO: Option<Coefficient> = Some(Coefficient::ProvenAbsent {
        witness: "fake_explicit_source_absence",
    });

    fn derived(count: u64) -> Coefficient {
        Coefficient::Derived {
            count,
            witness: "fake_source_derivation",
        }
    }

    fn coefficients(
        a: Option<Coefficient>,
        b: Option<Coefficient>,
        c: Option<Coefficient>,
    ) -> AffineCoefficients {
        AffineCoefficients {
            component: Component::ControlVisits,
            a,
            b,
            c,
        }
    }

    fn policy() -> PolicyTuple {
        PolicyTuple {
            constant: 1_024,
            bytes: 16,
            nodes: 128,
        }
    }

    fn ratio(source_units_per_debit: u64) -> ComponentRatio {
        ComponentRatio {
            component: Component::ControlVisits,
            source_units_per_debit,
        }
    }

    fn common_fixture() -> CommonFixture {
        CommonFixture {
            premises: [
                Some("fake_operations"),
                Some("fake_ownership"),
                Some("fake_affine_shape"),
                Some("fake_lifetimes"),
                Some("fake_storage"),
                Some("fake_structural_stack"),
                Some("fake_attempt_envelope"),
                Some("fake_external_clock_boundary"),
                Some("fake_disjoint_attempt_admitted_external_stages"),
            ],
            declarations: vec![DeclarationFixture {
                owner: Some("fake_declaration"),
                coefficients: coefficients(Some(derived(3)), Some(derived(5)), Some(derived(7))),
            }],
        }
    }

    fn shape_fixture() -> ConditionalShape {
        ConditionalShape {
            component_maxima: vec![ratio(1)],
            numeric_policy_adequacy: NumericalPolicyAdequacy::NotEvaluated,
            attempted_work_funding: AttemptFunding::NotEstablished,
        }
    }

    fn population() -> AttemptPopulation {
        AttemptPopulation {
            attempts: 3,
            per_attempt: SourceWork(5),
            admitted_work: vec![SourceWork(11)],
            admitted_debits: vec![1_024],
            clock_invocations: 3,
            clock_binding: "fake_clock_source_and_target_identity",
        }
    }

    fn facts() -> HypotheticalTargetFacts {
        HypotheticalTargetFacts {
            clock_binding: "fake_clock_source_and_target_identity",
            clock_work: Some(SuppliedFact::Value(SourceWork(7))),
            requested_storage: Some(SuppliedFact::Value(RequestedBytes(17))),
            retained_state: Some(SuppliedFact::Value(RequestedBytes(19))),
            stack_frame: Some(SuppliedFact::Value(FrameBytes(23))),
        }
    }

    fn local_fixture() -> LocalEnvelope {
        LocalEnvelope {
            component: Component::ControlVisits,
            attempts: SourceWork(15),
            admitted: SourceWork(11),
            local: SourceWork(26),
            admitted_policy_bound: SourceWork(1_024),
            local_policy_bound: SourceWork(1_039),
            attempted_work_funding: AttemptFunding::NotEstablished,
        }
    }

    fn evaluation_fixture() -> HypotheticalEvaluation {
        HypotheticalEvaluation {
            local: local_fixture(),
            external_work: SourceWork(21),
            total_work: SourceWork(47),
            total_policy_bound: SourceWork(1_060),
            requested_storage: RequestedBytes(17),
            retained_state: RequestedBytes(19),
            stack_frame: FrameBytes(23),
            numeric_policy_adequacy: NumericalPolicyAdequacy::NotEvaluated,
        }
    }

    #[test]
    fn missing_common_proof_cannot_complete() {
        let original = common_fixture();
        assert_eq!(check_common_fixture(&original), Ok(shape_fixture()));
        for (index, obligation) in OBLIGATIONS.into_iter().enumerate() {
            let mut missing = original.clone();
            assert!(missing.premises[index].take().is_some());
            assert_eq!(
                check_common_fixture(&missing),
                Err(Refusal::MissingCommonProof(obligation))
            );
        }
        let mut ownerless = original.clone();
        assert!(ownerless.declarations[0].owner.take().is_some());
        assert_eq!(
            check_common_fixture(&ownerless),
            Err(Refusal::MissingCommonProof(Obligation::Ownership))
        );
        let mut empty = original.clone();
        assert_eq!(empty.declarations.len(), 1);
        empty.declarations.clear();
        assert_eq!(
            check_common_fixture(&empty),
            Err(Refusal::MissingCommonProof(Obligation::Ownership))
        );
        for axis in [Axis::Constant, Axis::Bytes, Axis::Nodes] {
            let mut missing = original.clone();
            let work = &mut missing.declarations[0].coefficients;
            let coefficient = match axis {
                Axis::Constant => &mut work.a,
                Axis::Bytes => &mut work.b,
                Axis::Nodes => &mut work.c,
            };
            assert!(coefficient.take().is_some());
            assert_eq!(
                check_common_fixture(&missing),
                Err(Refusal::MissingCoefficient(axis))
            );
        }
    }

    #[test]
    fn hypothetical_facts_cannot_qualify() {
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &facts()),
            Ok(evaluation_fixture())
        );
        assert_eq!(
            qualification_refusal(&shape_fixture(), Some(&evaluation_fixture())),
            Refusal::HypotheticalFacts
        );
    }

    #[test]
    fn component_ratios_are_not_cpu_weights() {
        for (work, expected) in [
            (coefficients(Some(derived(2_049)), ZERO, ZERO), 3),
            (coefficients(ZERO, Some(derived(33)), ZERO), 3),
            (coefficients(ZERO, ZERO, Some(derived(129))), 2),
        ] {
            assert_eq!(component_ratio(&work, policy()), Ok(ratio(expected)));
        }
        let mut byte_work = coefficients(ZERO, Some(derived(33)), ZERO);
        byte_work.component = Component::BytesScanned;
        assert_eq!(
            component_ratio(&byte_work, policy()),
            Ok(ComponentRatio {
                component: Component::BytesScanned,
                source_units_per_debit: 3,
            })
        );
        assert_eq!(
            affine_bound(
                &coefficients(Some(derived(3)), Some(derived(5)), Some(derived(7))),
                2,
                3,
            ),
            Ok(SourceWork(34))
        );
        assert_eq!(
            shape_fixture().numeric_policy_adequacy,
            NumericalPolicyAdequacy::NotEvaluated
        );
    }

    #[test]
    fn coefficient_boundaries_vary_independently() {
        for (work, expected) in [
            (coefficients(Some(derived(1_023)), ZERO, ZERO), 1),
            (coefficients(Some(derived(1_024)), ZERO, ZERO), 1),
            (coefficients(Some(derived(1_025)), ZERO, ZERO), 2),
            (coefficients(ZERO, Some(derived(15)), ZERO), 1),
            (coefficients(ZERO, Some(derived(16)), ZERO), 1),
            (coefficients(ZERO, Some(derived(17)), ZERO), 2),
            (coefficients(ZERO, ZERO, Some(derived(127))), 1),
            (coefficients(ZERO, ZERO, Some(derived(128))), 1),
            (coefficients(ZERO, ZERO, Some(derived(129))), 2),
            (
                coefficients(Some(derived(1_025)), Some(derived(33)), Some(derived(129))),
                3,
            ),
            (coefficients(ZERO, ZERO, ZERO), 0),
        ] {
            assert_eq!(component_ratio(&work, policy()), Ok(ratio(expected)));
        }
        for axis in [Axis::Constant, Axis::Bytes, Axis::Nodes] {
            let mut missing = coefficients(ZERO, ZERO, ZERO);
            let coefficient = match axis {
                Axis::Constant => &mut missing.a,
                Axis::Bytes => &mut missing.b,
                Axis::Nodes => &mut missing.c,
            };
            assert_eq!(coefficient.take(), ZERO);
            assert_eq!(
                component_ratio(&missing, policy()),
                Err(Refusal::MissingCoefficient(axis))
            );
            *match axis {
                Axis::Constant => &mut missing.a,
                Axis::Bytes => &mut missing.b,
                Axis::Nodes => &mut missing.c,
            } = Some(derived(0));
            assert_eq!(
                component_ratio(&missing, policy()),
                Err(Refusal::UnwitnessedZero)
            );
        }
    }

    #[test]
    fn uncharged_attempts_survive_zero_debit() {
        let admitted = population();
        assert_eq!(local_envelope(ratio(1), &admitted), Ok(local_fixture()));
        let mut refused = admitted.clone();
        assert_eq!(refused.admitted_work, vec![SourceWork(11)]);
        assert_eq!(refused.admitted_debits, vec![1_024]);
        refused.admitted_work.clear();
        refused.admitted_debits.clear();
        for (attempts, expected) in [(0, 0), (1, 5), (3, 15)] {
            refused.attempts = attempts;
            assert_eq!(
                local_envelope(ratio(1), &refused),
                Ok(LocalEnvelope {
                    component: Component::ControlVisits,
                    attempts: SourceWork(expected),
                    admitted: SourceWork(0),
                    local: SourceWork(expected),
                    admitted_policy_bound: SourceWork(0),
                    local_policy_bound: SourceWork(expected),
                    attempted_work_funding: AttemptFunding::NotEstablished,
                })
            );
        }
        assert_eq!(
            evaluate_hypothetical(ratio(1), &admitted, &facts()),
            Ok(evaluation_fixture())
        );
        let mut no_clock_calls = admitted;
        assert_eq!(no_clock_calls.clock_invocations, 3);
        no_clock_calls.clock_invocations = 0;
        let mut expected = evaluation_fixture();
        expected.external_work = SourceWork(0);
        expected.total_work = SourceWork(26);
        expected.total_policy_bound = SourceWork(1_039);
        assert_eq!(
            evaluate_hypothetical(ratio(1), &no_clock_calls, &facts()),
            Ok(expected)
        );
    }

    #[test]
    fn shape_complete_cannot_qualify_policy() {
        assert_eq!(check_common_fixture(&common_fixture()), Ok(shape_fixture()));
        let shape = shape_fixture();
        assert_eq!(shape.component_maxima, vec![ratio(1)]);
        assert_eq!(
            shape.numeric_policy_adequacy,
            NumericalPolicyAdequacy::NotEvaluated
        );
        assert_eq!(shape.attempted_work_funding, AttemptFunding::NotEstablished);
        assert_eq!(
            qualification_refusal(&shape, None),
            Refusal::NumericalPolicyNotEvaluated
        );
    }

    #[test]
    fn missing_target_facts_cannot_be_zero() {
        let original = facts();
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &original),
            Ok(evaluation_fixture())
        );
        for fact in [
            TargetFact::ClockWork,
            TargetFact::RequestedStorage,
            TargetFact::RetainedState,
            TargetFact::StackFrame,
        ] {
            let mut missing = original.clone();
            match fact {
                TargetFact::ClockWork => assert!(missing.clock_work.take().is_some()),
                TargetFact::RequestedStorage => {
                    assert!(missing.requested_storage.take().is_some());
                }
                TargetFact::RetainedState => assert!(missing.retained_state.take().is_some()),
                TargetFact::StackFrame => assert!(missing.stack_frame.take().is_some()),
            }
            for calls in [0, 3] {
                let mut attempted = population();
                attempted.clock_invocations = calls;
                assert_eq!(
                    evaluate_hypothetical(ratio(1), &attempted, &missing),
                    Err(Refusal::MissingTargetFact(fact))
                );
            }
        }
        let mut stale = original.clone();
        assert_eq!(stale.clock_binding, population().clock_binding);
        stale.clock_binding = "fake_other_clock_identity";
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &stale),
            Err(Refusal::TargetBinding)
        );
        let zero = HypotheticalTargetFacts {
            clock_binding: original.clock_binding,
            clock_work: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_clock_work",
            }),
            requested_storage: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_storage",
            }),
            retained_state: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_retention",
            }),
            stack_frame: Some(SuppliedFact::ProvenAbsent {
                witness: "fake_explicit_zero_frame",
            }),
        };
        let mut expected = evaluation_fixture();
        expected.external_work = SourceWork(0);
        expected.total_work = SourceWork(26);
        expected.total_policy_bound = SourceWork(1_039);
        expected.requested_storage = RequestedBytes(0);
        expected.retained_state = RequestedBytes(0);
        expected.stack_frame = FrameBytes(0);
        assert_eq!(
            evaluate_hypothetical(ratio(1), &population(), &zero),
            Ok(expected)
        );
    }

    #[test]
    fn retired_candidate_cannot_supply_costs() {
        let current = json!({
            "kind": "conditional_source_shape",
            "numeric_policy_adequacy": "not_evaluated",
            "attempted_work_funding": "not_established"
        });
        assert_eq!(check_current_report_fields(&current), Ok(()));
        let retired = json!({
            "kind": "conditional_native_schema_cache_heap_candidate",
            "native_cache_heap_candidate": {"requested_bytes": 7}
        });
        assert_eq!(
            check_current_report_fields(&retired),
            Err(Refusal::RetiredCandidate)
        );
        let mut hybrid = current.clone();
        assert!(hybrid.get("native_cache_heap_candidate").is_none());
        hybrid["native_cache_heap_candidate"] = json!({"requested_bytes": 7});
        assert_eq!(
            check_current_report_fields(&hybrid),
            Err(Refusal::RetiredCandidate)
        );
        let mut fallback = current;
        assert!(fallback.get("heuristic_fallback").is_none());
        fallback["heuristic_fallback"] = json!({"work_units": 1_024});
        assert_eq!(
            check_current_report_fields(&fallback),
            Err(Refusal::NonCanonicalField)
        );
    }

    #[test]
    fn existing_policy_tuple_is_checked_independently() {
        assert_eq!(
            captured_policy_tuple(),
            Ok(PolicyTuple {
                constant: 1_024,
                bytes: 16,
                nodes: 128,
            })
        );
        assert_eq!(check_policy_tuple(policy()), Ok(()));
        for changed in [
            PolicyTuple {
                constant: 1_023,
                ..policy()
            },
            PolicyTuple {
                constant: 1_025,
                ..policy()
            },
            PolicyTuple {
                bytes: 15,
                ..policy()
            },
            PolicyTuple {
                bytes: 17,
                ..policy()
            },
            PolicyTuple {
                nodes: 127,
                ..policy()
            },
            PolicyTuple {
                nodes: 129,
                ..policy()
            },
            PolicyTuple {
                constant: 0,
                ..policy()
            },
            PolicyTuple {
                bytes: 0,
                ..policy()
            },
            PolicyTuple {
                nodes: 0,
                ..policy()
            },
        ] {
            assert_ne!(changed, policy());
            assert_eq!(check_policy_tuple(changed), Err(Refusal::PolicyTuple));
            assert_eq!(
                component_ratio(
                    &coefficients(Some(derived(3)), Some(derived(5)), Some(derived(7))),
                    changed,
                ),
                Err(Refusal::PolicyTuple)
            );
        }
    }

    #[test]
    fn declaration_maxima_preserve_components() {
        assert_eq!(
            declaration_maximum(&[ratio(2), ratio(5), ratio(3)]),
            Ok(ratio(5))
        );
        assert_eq!(declaration_maximum(&[ratio(0), ratio(0)]), Ok(ratio(0)));
        assert_eq!(
            declaration_maximum(&[]),
            Err(Refusal::MissingCommonProof(Obligation::Ownership))
        );
        assert_eq!(
            declaration_maximum(&[
                ratio(3),
                ComponentRatio {
                    component: Component::BytesScanned,
                    source_units_per_debit: 7,
                },
            ]),
            Err(Refusal::ComponentMismatch)
        );
        let mut attempted = population();
        attempted.admitted_work = vec![SourceWork(11), SourceWork(13)];
        attempted.admitted_debits = vec![1_024, 2_048];
        assert_eq!(
            local_envelope(ratio(3), &attempted),
            Ok(LocalEnvelope {
                component: Component::ControlVisits,
                attempts: SourceWork(15),
                admitted: SourceWork(24),
                local: SourceWork(39),
                admitted_policy_bound: SourceWork(9_216),
                local_policy_bound: SourceWork(9_231),
                attempted_work_funding: AttemptFunding::NotEstablished,
            })
        );
    }

    #[test]
    fn checked_arithmetic_refuses_overflow() {
        // Ceiling division must not spuriously overflow at numerator + divisor - 1.
        assert_eq!(
            component_ratio(&coefficients(Some(derived(u64::MAX)), ZERO, ZERO), policy()),
            Ok(ratio(18_014_398_509_481_984))
        );
        for (work, bytes, nodes) in [
            (coefficients(ZERO, Some(derived(u64::MAX)), ZERO), 2, 0),
            (coefficients(ZERO, ZERO, Some(derived(u64::MAX))), 0, 2),
            (
                coefficients(Some(derived(u64::MAX)), Some(derived(1)), ZERO),
                1,
                0,
            ),
        ] {
            assert_eq!(affine_bound(&work, bytes, nodes), Err(Refusal::Overflow));
        }
        let mut attempts_overflow = population();
        attempts_overflow.attempts = u64::MAX;
        attempts_overflow.per_attempt = SourceWork(2);
        assert_eq!(
            local_envelope(ratio(1), &attempts_overflow),
            Err(Refusal::Overflow)
        );
        assert_eq!(
            local_envelope(ratio(u64::MAX), &population()),
            Err(Refusal::Overflow)
        );
        let mut debit_sum_overflow = population();
        debit_sum_overflow.admitted_debits = vec![u64::MAX, 1];
        debit_sum_overflow.admitted_work = vec![SourceWork(1), SourceWork(1)];
        assert_eq!(
            local_envelope(ratio(1), &debit_sum_overflow),
            Err(Refusal::Overflow)
        );
        let mut local_sum_overflow = population();
        local_sum_overflow.attempts = 1;
        local_sum_overflow.per_attempt = SourceWork(u64::MAX);
        assert_eq!(
            local_envelope(ratio(1), &local_sum_overflow),
            Err(Refusal::Overflow)
        );
        let mut clock_overflow = population();
        clock_overflow.clock_invocations = u64::MAX;
        assert_eq!(
            evaluate_hypothetical(ratio(1), &clock_overflow, &facts()),
            Err(Refusal::Overflow)
        );
    }
}

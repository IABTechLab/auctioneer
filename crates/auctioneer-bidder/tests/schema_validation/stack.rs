//! Hypothetical Task 6 composition, not an actual-source closure or frame certificate.
//! Path witnesses, identities and numeric facts here carry no source authority.
//! The authenticated operation-graph/source join remains a separate obligation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameBytes(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RequestedBytes(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkUnits(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HeapStackRecords(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceHelper {
    source_owner: &'static str,
    source_sha256: &'static str,
    helper: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetIdentity {
    artifact_sha256: &'static str,
    compiler: &'static str,
    target: &'static str,
    profile: &'static str,
    features: &'static str,
    abi: &'static str,
    panic_mode: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DepthAxis {
    Schema,
    Instance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DepthInputs {
    // Independent bounds on simultaneously active levels, not work counts.
    schema_levels: u64,
    instance_containers: u64,
}

// A future operation-graph join must prove these witnesses, not trust labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecreasingBound {
    recursive_edge: &'static str,
    remaining_measure: &'static str,
    decrease_by: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CallPath {
    Frame(SourceHelper),
    Coexisting(Vec<Self>),
    Alternatives {
        exclusivity: Option<&'static str>,
        paths: Vec<Self>,
    },
    Recursion {
        axis: DepthAxis,
        decreasing: Option<DecreasingBound>,
        body: Box<Self>,
    },
    RegexHeapStack {
        owner: SourceHelper,
        records: HeapStackRecords,
    },
    UnclassifiedEdge {
        caller: SourceHelper,
        callee: SourceHelper,
    },
}

// Fixture shape only: this is deliberately not a source-completeness capability.
#[derive(Clone, Debug, Eq, PartialEq)]
struct LogicalFormula {
    path: CallPath,
    required_frames: Vec<SourceHelper>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameFact {
    owner: SourceHelper,
    identity: TargetIdentity,
    bytes: FrameBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommonGap {
    OperationGraph,
    Edge {
        caller: SourceHelper,
        callee: SourceHelper,
    },
    FrameOwner(SourceHelper),
    DecreasingBound(DepthAxis),
    Exclusivity,
    ExpirySchedule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpiryField {
    Work,
    TemporaryHeap,
    RetainedState,
    Frame,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArithmeticDomain {
    Frames,
    Work,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    MissingCommonProof(CommonGap),
    MissingTargetFact(SourceHelper),
    FrameIdentity(SourceHelper),
    MissingExpiryFact(ExpiryField),
    InvalidExpiryFact(ExpiryField),
    ExpiryIdentity,
    Overflow(ArithmeticDomain),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HypotheticalStack {
    peak: FrameBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpirySchedule {
    owner: SourceHelper,
    identity: TargetIdentity,
    invocations: u64,
    serial_temporary_witness: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpiryFacts {
    owner: SourceHelper,
    identity: TargetIdentity,
    work_per_call: Option<WorkUnits>,
    temporary_heap_peak: Option<RequestedBytes>,
    // A required bound on state retained across the entire supplied schedule.
    retained_state: Option<RequestedBytes>,
    frame: Option<FrameBytes>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HypotheticalExpiry {
    cumulative_work: WorkUnits,
    temporary_heap_peak: RequestedBytes,
    retained_state: RequestedBytes,
    peak_frame: FrameBytes,
}

// This checks fixture consistency only; it cannot authenticate source witnesses.
fn check_common(formula: &LogicalFormula) -> Result<(), Refusal> {
    let mut owners = Vec::new();
    collect_frame_owners(&formula.path, &mut owners)?;
    if owners.is_empty() {
        return Err(Refusal::MissingCommonProof(CommonGap::OperationGraph));
    }
    for owner in &owners {
        if !formula.required_frames.contains(owner) {
            return Err(Refusal::MissingCommonProof(CommonGap::FrameOwner(*owner)));
        }
    }
    for (index, owner) in formula.required_frames.iter().enumerate() {
        if !owners.contains(owner)
            || formula
                .required_frames
                .iter()
                .take(index)
                .any(|prior| prior == owner)
        {
            return Err(Refusal::MissingCommonProof(CommonGap::FrameOwner(*owner)));
        }
    }
    Ok(())
}

fn collect_frame_owners(path: &CallPath, owners: &mut Vec<SourceHelper>) -> Result<(), Refusal> {
    match path {
        CallPath::Frame(owner) => {
            if !owners.contains(owner) {
                owners.push(*owner);
            }
            Ok(())
        }
        CallPath::Alternatives { exclusivity, .. } if exclusivity.is_none_or(str::is_empty) => {
            Err(Refusal::MissingCommonProof(CommonGap::Exclusivity))
        }
        CallPath::Coexisting(paths) | CallPath::Alternatives { paths, .. } => {
            if paths.is_empty() {
                return Err(Refusal::MissingCommonProof(CommonGap::OperationGraph));
            }
            for child in paths {
                collect_frame_owners(child, owners)?;
            }
            Ok(())
        }
        CallPath::Recursion {
            axis,
            decreasing,
            body,
        } => {
            if decreasing.is_none_or(|bound| {
                bound.recursive_edge.is_empty()
                    || bound.remaining_measure.is_empty()
                    || bound.decrease_by == 0
            }) {
                return Err(Refusal::MissingCommonProof(CommonGap::DecreasingBound(
                    *axis,
                )));
            }
            collect_frame_owners(body, owners)
        }
        CallPath::RegexHeapStack { .. } => Ok(()),
        CallPath::UnclassifiedEdge { caller, callee } => {
            Err(Refusal::MissingCommonProof(CommonGap::Edge {
                caller: *caller,
                callee: *callee,
            }))
        }
    }
}

fn evaluate_hypothetical(
    formula: &LogicalFormula,
    depth: DepthInputs,
    identity: TargetIdentity,
    facts: &[FrameFact],
) -> Result<HypotheticalStack, Refusal> {
    check_common(formula)?;
    for owner in &formula.required_frames {
        let mut matching = facts.iter().filter(|fact| fact.owner == *owner);
        let fact = matching.next().ok_or(Refusal::MissingTargetFact(*owner))?;
        if matching.next().is_some() || fact.identity != identity {
            return Err(Refusal::FrameIdentity(*owner));
        }
    }
    for fact in facts {
        if !formula.required_frames.contains(&fact.owner) {
            return Err(Refusal::FrameIdentity(fact.owner));
        }
    }
    Ok(HypotheticalStack {
        peak: FrameBytes(path_peak(&formula.path, depth, facts)?),
    })
}

fn path_peak(path: &CallPath, depth: DepthInputs, facts: &[FrameFact]) -> Result<u64, Refusal> {
    match path {
        CallPath::Frame(owner) => facts
            .iter()
            .find(|fact| fact.owner == *owner)
            .map(|fact| fact.bytes.0)
            .ok_or(Refusal::MissingTargetFact(*owner)),
        CallPath::Coexisting(paths) => paths.iter().try_fold(0_u64, |peak, child| {
            peak.checked_add(path_peak(child, depth, facts)?)
                .ok_or(Refusal::Overflow(ArithmeticDomain::Frames))
        }),
        CallPath::Alternatives { paths, .. } => paths.iter().try_fold(0_u64, |peak, child| {
            Ok(peak.max(path_peak(child, depth, facts)?))
        }),
        CallPath::Recursion { axis, body, .. } => {
            let levels = match axis {
                DepthAxis::Schema => depth.schema_levels,
                DepthAxis::Instance => depth.instance_containers,
            };
            path_peak(body, depth, facts)?
                .checked_mul(levels)
                .ok_or(Refusal::Overflow(ArithmeticDomain::Frames))
        }
        // An explicitly classified heap record contributes no machine frame.
        CallPath::RegexHeapStack { .. } => Ok(0),
        CallPath::UnclassifiedEdge { caller, callee } => {
            Err(Refusal::MissingCommonProof(CommonGap::Edge {
                caller: *caller,
                callee: *callee,
            }))
        }
    }
}

fn evaluate_expiry_hypothetical(
    schedule: ExpirySchedule,
    facts: ExpiryFacts,
) -> Result<HypotheticalExpiry, Refusal> {
    if schedule.serial_temporary_witness.is_none_or(str::is_empty) {
        return Err(Refusal::MissingCommonProof(CommonGap::ExpirySchedule));
    }
    if schedule.owner != facts.owner || schedule.identity != facts.identity {
        return Err(Refusal::ExpiryIdentity);
    }
    let work = required_expiry_fact(facts.work_per_call.map(|value| value.0), ExpiryField::Work)?;
    let temporary = required_expiry_fact(
        facts.temporary_heap_peak.map(|value| value.0),
        ExpiryField::TemporaryHeap,
    )?;
    let retained = required_expiry_fact(
        facts.retained_state.map(|value| value.0),
        ExpiryField::RetainedState,
    )?;
    let frame = required_expiry_fact(facts.frame.map(|value| value.0), ExpiryField::Frame)?;
    let cumulative_work = work
        .checked_mul(schedule.invocations)
        .ok_or(Refusal::Overflow(ArithmeticDomain::Work))?;
    Ok(HypotheticalExpiry {
        cumulative_work: WorkUnits(cumulative_work),
        temporary_heap_peak: RequestedBytes(temporary),
        retained_state: RequestedBytes(retained),
        peak_frame: FrameBytes(frame),
    })
}

fn required_expiry_fact(value: Option<u64>, field: ExpiryField) -> Result<u64, Refusal> {
    match value {
        None => Err(Refusal::MissingExpiryFact(field)),
        Some(0) => Err(Refusal::InvalidExpiryFact(field)),
        Some(value) => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArithmeticDomain, CallPath, CommonGap, DecreasingBound, DepthAxis, DepthInputs,
        ExpiryFacts, ExpiryField, ExpirySchedule, FrameBytes, FrameFact, HeapStackRecords,
        HypotheticalExpiry, HypotheticalStack, LogicalFormula, Refusal, RequestedBytes,
        SourceHelper, TargetIdentity, WorkUnits, check_common, evaluate_expiry_hypothetical,
        evaluate_hypothetical,
    };

    const ROOT: SourceHelper = source("fake_coordinator", "fake_root_body", "fake_entry");
    const VALIDATOR: SourceHelper =
        source("fake_generated", "fake_validator_body", "fake_validate");
    const CALLBACK: SourceHelper = source("fake_callback", "fake_callback_body", "fake_clock");
    const CLEANUP: SourceHelper = source("fake_cleanup", "fake_cleanup_body", "fake_drop");

    const fn source(
        source_owner: &'static str,
        source_sha256: &'static str,
        helper: &'static str,
    ) -> SourceHelper {
        SourceHelper {
            source_owner,
            source_sha256,
            helper,
        }
    }

    fn identity() -> TargetIdentity {
        TargetIdentity {
            artifact_sha256: "fake_artifact_a",
            compiler: "fake_compiler_a",
            target: "fake_target_a",
            profile: "fake_profile_a",
            features: "fake_features_a",
            abi: "fake_abi_a",
            panic_mode: "fake_unwind",
        }
    }

    fn changed_identities() -> Vec<(&'static str, TargetIdentity)> {
        let original = identity();
        vec![
            (
                "artifact",
                TargetIdentity {
                    artifact_sha256: "fake_other",
                    ..original
                },
            ),
            (
                "compiler",
                TargetIdentity {
                    compiler: "fake_other",
                    ..original
                },
            ),
            (
                "target",
                TargetIdentity {
                    target: "fake_other",
                    ..original
                },
            ),
            (
                "profile",
                TargetIdentity {
                    profile: "fake_other",
                    ..original
                },
            ),
            (
                "features",
                TargetIdentity {
                    features: "fake_other",
                    ..original
                },
            ),
            (
                "abi",
                TargetIdentity {
                    abi: "fake_other",
                    ..original
                },
            ),
            (
                "panic_mode",
                TargetIdentity {
                    panic_mode: "fake_abort",
                    ..original
                },
            ),
        ]
    }

    fn changed_sources(original: SourceHelper) -> Vec<(&'static str, SourceHelper)> {
        vec![
            (
                "source_owner",
                SourceHelper {
                    source_owner: "fake_other",
                    ..original
                },
            ),
            (
                "source_sha256",
                SourceHelper {
                    source_sha256: "fake_other",
                    ..original
                },
            ),
            (
                "helper",
                SourceHelper {
                    helper: "fake_other",
                    ..original
                },
            ),
        ]
    }

    fn depth() -> DepthInputs {
        DepthInputs {
            schema_levels: 1,
            instance_containers: 1,
        }
    }

    fn fixture_path(exclusivity: Option<&'static str>) -> CallPath {
        CallPath::Coexisting(vec![
            CallPath::Frame(ROOT),
            CallPath::Alternatives {
                exclusivity,
                paths: vec![
                    CallPath::Coexisting(vec![
                        CallPath::Frame(VALIDATOR),
                        CallPath::Frame(CALLBACK),
                    ]),
                    CallPath::Frame(CLEANUP),
                ],
            },
        ])
    }

    fn fixture() -> (LogicalFormula, Vec<FrameFact>) {
        let formula = LogicalFormula {
            path: fixture_path(Some("fake_normal_or_cleanup_edge_witness")),
            required_frames: vec![ROOT, VALIDATOR, CALLBACK, CLEANUP],
        };
        let facts = [(ROOT, 2), (VALIDATOR, 7), (CALLBACK, 11), (CLEANUP, 19)]
            .into_iter()
            .map(|(owner, bytes)| FrameFact {
                owner,
                identity: identity(),
                bytes: FrameBytes(bytes),
            })
            .collect();
        (formula, facts)
    }

    fn recursive(axis: DepthAxis, owner: SourceHelper) -> CallPath {
        CallPath::Recursion {
            axis,
            decreasing: Some(DecreasingBound {
                recursive_edge: "fake_recursive_edge",
                remaining_measure: match axis {
                    DepthAxis::Schema => "fake_remaining_schema_levels",
                    DepthAxis::Instance => "fake_remaining_instance_containers",
                },
                decrease_by: 1,
            }),
            body: Box::new(CallPath::Frame(owner)),
        }
    }

    fn expiry_fixture() -> (ExpirySchedule, ExpiryFacts, HypotheticalExpiry) {
        (
            ExpirySchedule {
                owner: CALLBACK,
                identity: identity(),
                invocations: 3,
                serial_temporary_witness: Some("fake_serial_calls_and_separate_retained_state"),
            },
            ExpiryFacts {
                owner: CALLBACK,
                identity: identity(),
                work_per_call: Some(WorkUnits(11)),
                temporary_heap_peak: Some(RequestedBytes(7)),
                retained_state: Some(RequestedBytes(9)),
                frame: Some(FrameBytes(5)),
            },
            HypotheticalExpiry {
                cumulative_work: WorkUnits(33),
                temporary_heap_peak: RequestedBytes(7),
                retained_state: RequestedBytes(9),
                peak_frame: FrameBytes(5),
            },
        )
    }

    #[test]
    fn nested_calls_sum_and_alternatives_take_max() {
        let (formula, mut facts) = fixture();
        assert!(matches!(
            &formula.path,
            CallPath::Coexisting(calls)
                if matches!(&calls[0], CallPath::Frame(owner) if *owner == ROOT)
                    && matches!(
                        &calls[1],
                        CallPath::Alternatives { exclusivity, paths }
                            if *exclusivity == Some("fake_normal_or_cleanup_edge_witness")
                                && paths.len() == 2
                    )
        ));
        // Independent literal oracle: 2 + max(7 + 11, 19) = 21.
        assert_eq!(
            evaluate_hypothetical(&formula, depth(), identity(), &facts),
            Ok(HypotheticalStack {
                peak: FrameBytes(21)
            })
        );
        assert_eq!(facts[2].owner, CALLBACK);
        assert_eq!(facts[2].bytes, FrameBytes(11));
        facts[2].bytes = FrameBytes(13);
        assert_eq!(
            evaluate_hypothetical(&formula, depth(), identity(), &facts),
            Ok(HypotheticalStack {
                peak: FrameBytes(22)
            })
        );
        let mut unproved = formula;
        unproved.path = fixture_path(None);
        assert_eq!(
            check_common(&unproved),
            Err(Refusal::MissingCommonProof(CommonGap::Exclusivity))
        );
    }

    #[test]
    fn instance_and_schema_depth_are_independent() {
        let formula = LogicalFormula {
            path: CallPath::Coexisting(vec![
                CallPath::Frame(ROOT),
                recursive(DepthAxis::Schema, VALIDATOR),
                recursive(DepthAxis::Instance, CALLBACK),
            ]),
            required_frames: vec![ROOT, VALIDATOR, CALLBACK],
        };
        let facts = [(ROOT, 2), (VALIDATOR, 3), (CALLBACK, 5)]
            .into_iter()
            .map(|(owner, bytes)| FrameFact {
                owner,
                identity: identity(),
                bytes: FrameBytes(bytes),
            })
            .collect::<Vec<_>>();
        for (schema_levels, instance_containers, peak) in [(2, 4, 28), (3, 4, 31), (2, 5, 33)] {
            let input = DepthInputs {
                schema_levels,
                instance_containers,
            };
            assert_ne!(input.schema_levels, input.instance_containers);
            assert_eq!(
                evaluate_hypothetical(&formula, input, identity(), &facts),
                Ok(HypotheticalStack {
                    peak: FrameBytes(peak)
                })
            );
        }
        for axis in [DepthAxis::Schema, DepthAxis::Instance] {
            for decreasing in [
                None,
                Some(DecreasingBound {
                    recursive_edge: "fake_recursive_edge",
                    remaining_measure: "fake_non_decreasing_measure",
                    decrease_by: 0,
                }),
            ] {
                let mut unproved = formula.clone();
                let bound = match &mut unproved.path {
                    CallPath::Coexisting(paths) => paths.iter_mut().find_map(|path| match path {
                        CallPath::Recursion {
                            axis: candidate,
                            decreasing,
                            body,
                        } if *candidate == axis => {
                            let owner = match axis {
                                DepthAxis::Schema => VALIDATOR,
                                DepthAxis::Instance => CALLBACK,
                            };
                            assert!(matches!(
                                body.as_ref(),
                                CallPath::Frame(actual) if *actual == owner
                            ));
                            Some(decreasing)
                        }
                        _ => None,
                    }),
                    _ => None,
                }
                .expect("fixture contains each independent recursive edge");
                assert_eq!(
                    bound.as_ref().map(|proof| proof.recursive_edge),
                    Some("fake_recursive_edge")
                );
                assert_eq!(
                    bound.as_ref().map(|proof| proof.remaining_measure),
                    Some(match axis {
                        DepthAxis::Schema => "fake_remaining_schema_levels",
                        DepthAxis::Instance => "fake_remaining_instance_containers",
                    })
                );
                assert_eq!(bound.as_ref().map(|proof| proof.decrease_by), Some(1));
                *bound = decreasing;
                assert_eq!(
                    check_common(&unproved),
                    Err(Refusal::MissingCommonProof(CommonGap::DecreasingBound(
                        axis
                    )))
                );
            }
        }
    }

    #[test]
    fn heap_stack_records_are_not_frames() {
        let facts = [FrameFact {
            owner: ROOT,
            identity: identity(),
            bytes: FrameBytes(2),
        }];
        for records in [1, 900, u64::MAX] {
            let formula = LogicalFormula {
                path: CallPath::Coexisting(vec![
                    CallPath::Frame(ROOT),
                    CallPath::RegexHeapStack {
                        owner: VALIDATOR,
                        records: HeapStackRecords(records),
                    },
                ]),
                required_frames: vec![ROOT],
            };
            assert!(matches!(
                &formula.path,
                CallPath::Coexisting(calls)
                    if matches!(
                        &calls[1],
                        CallPath::RegexHeapStack { owner, records: actual }
                            if *owner == VALIDATOR && actual.0 == records
                    )
            ));
            assert_eq!(
                evaluate_hypothetical(&formula, depth(), identity(), &facts),
                Ok(HypotheticalStack {
                    peak: FrameBytes(2)
                })
            );
            let mut needs_frame = formula;
            needs_frame.required_frames.push(VALIDATOR);
            needs_frame.path =
                CallPath::Coexisting(vec![needs_frame.path, CallPath::Frame(VALIDATOR)]);
            assert_eq!(
                evaluate_hypothetical(&needs_frame, depth(), identity(), &facts),
                Err(Refusal::MissingTargetFact(VALIDATOR))
            );
        }
    }

    #[test]
    fn missing_common_edge_differs_from_missing_target_fact() {
        let (formula, facts) = fixture();
        // A common formula can require a target slot without supplying its bytes.
        assert_eq!(check_common(&formula), Ok(()));
        let mut unclassified = formula.clone();
        unclassified.path = CallPath::Coexisting(vec![
            unclassified.path,
            CallPath::UnclassifiedEdge {
                caller: ROOT,
                callee: CALLBACK,
            },
        ]);
        assert!(matches!(
            &unclassified.path,
            CallPath::Coexisting(calls)
                if matches!(
                    &calls[1],
                    CallPath::UnclassifiedEdge { caller, callee }
                        if (*caller, *callee) == (ROOT, CALLBACK)
                )
        ));
        let missing_edge = Refusal::MissingCommonProof(CommonGap::Edge {
            caller: ROOT,
            callee: CALLBACK,
        });
        assert_eq!(check_common(&unclassified), Err(missing_edge));
        assert_eq!(
            evaluate_hypothetical(&unclassified, depth(), identity(), &facts),
            Err(missing_edge)
        );
        let mut absent = facts;
        assert_eq!(absent.remove(2).owner, CALLBACK);
        assert_eq!(
            evaluate_hypothetical(&unclassified, depth(), identity(), &absent),
            Err(missing_edge)
        );
        assert_eq!(check_common(&formula), Ok(()));
        assert_eq!(
            evaluate_hypothetical(&formula, depth(), identity(), &absent),
            Err(Refusal::MissingTargetFact(CALLBACK))
        );
        for path in [
            CallPath::Coexisting(Vec::new()),
            CallPath::RegexHeapStack {
                owner: VALIDATOR,
                records: HeapStackRecords(900),
            },
        ] {
            let incomplete = LogicalFormula {
                path,
                required_frames: Vec::new(),
            };
            assert_eq!(
                check_common(&incomplete),
                Err(Refusal::MissingCommonProof(CommonGap::OperationGraph))
            );
            assert_eq!(
                evaluate_hypothetical(&incomplete, depth(), identity(), &[]),
                Err(Refusal::MissingCommonProof(CommonGap::OperationGraph))
            );
        }
    }

    #[test]
    fn each_frame_owner_and_target_slot_is_independently_required() {
        let (formula, facts) = fixture();
        assert_eq!(formula.required_frames.len(), 4);
        assert_eq!(check_common(&formula), Ok(()));
        for (index, owner) in formula.required_frames.iter().copied().enumerate() {
            let mut undeclared = formula.clone();
            assert_eq!(undeclared.required_frames.remove(index), owner);
            assert_eq!(
                check_common(&undeclared),
                Err(Refusal::MissingCommonProof(CommonGap::FrameOwner(owner)))
            );
            assert_eq!(
                evaluate_hypothetical(&undeclared, depth(), identity(), &facts),
                Err(Refusal::MissingCommonProof(CommonGap::FrameOwner(owner)))
            );
            let mut absent = facts.clone();
            assert_eq!(absent.remove(index).owner, owner);
            assert_eq!(
                evaluate_hypothetical(&formula, depth(), identity(), &absent),
                Err(Refusal::MissingTargetFact(owner)),
                "missing target frame: {owner:?}"
            );
            for (field, changed) in changed_sources(owner) {
                assert_ne!(changed, owner);
                let mut misbound = facts.clone();
                assert_eq!(misbound[index].owner, owner);
                misbound[index].owner = changed;
                assert_eq!(
                    evaluate_hypothetical(&formula, depth(), identity(), &misbound),
                    Err(Refusal::MissingTargetFact(owner)),
                    "frame source field: {field}"
                );
            }
        }
    }

    #[test]
    fn abi_and_panic_mode_must_match() {
        let (formula, facts) = fixture();
        assert_eq!(
            evaluate_hypothetical(&formula, depth(), identity(), &facts),
            Ok(HypotheticalStack {
                peak: FrameBytes(21)
            })
        );
        for (field, changed) in changed_identities() {
            assert_ne!(changed, identity());
            for (index, fact) in facts.iter().enumerate() {
                let mut mismatched = facts.clone();
                assert_eq!(mismatched[index].identity, identity());
                mismatched[index].identity = changed;
                assert_eq!(
                    evaluate_hypothetical(&formula, depth(), identity(), &mismatched),
                    Err(Refusal::FrameIdentity(fact.owner)),
                    "frame identity field: {field}, owner: {:?}",
                    fact.owner
                );
            }
        }
    }

    #[test]
    fn expiry_callback_facts_are_nonzero_and_identity_bound() {
        let (schedule, facts, expected) = expiry_fixture();
        assert_eq!(schedule.owner, facts.owner);
        assert_eq!(schedule.identity, facts.identity);
        assert_eq!(schedule.invocations, 3);
        assert_eq!(
            schedule.serial_temporary_witness,
            Some("fake_serial_calls_and_separate_retained_state")
        );
        assert_eq!(expected.cumulative_work.0, 33);
        assert_eq!(expected.temporary_heap_peak.0, 7);
        assert_eq!(expected.retained_state.0, 9);
        assert_eq!(expected.peak_frame.0, 5);
        assert_eq!(evaluate_expiry_hypothetical(schedule, facts), Ok(expected));
        for (field, changed) in changed_identities() {
            assert_ne!(changed, facts.identity);
            assert_eq!(
                evaluate_expiry_hypothetical(
                    schedule,
                    ExpiryFacts {
                        identity: changed,
                        ..facts
                    }
                ),
                Err(Refusal::ExpiryIdentity),
                "expiry identity field: {field}"
            );
        }
        for (field, owner) in changed_sources(facts.owner) {
            assert_ne!(owner, facts.owner);
            assert_eq!(
                evaluate_expiry_hypothetical(schedule, ExpiryFacts { owner, ..facts }),
                Err(Refusal::ExpiryIdentity),
                "expiry source field: {field}"
            );
        }
        // These nonzero fixtures carry no source-proven zero-absence witness.
        for field in [
            ExpiryField::Work,
            ExpiryField::TemporaryHeap,
            ExpiryField::RetainedState,
            ExpiryField::Frame,
        ] {
            let mut absent = facts;
            let mut zero = facts;
            match field {
                ExpiryField::Work => {
                    assert_eq!(absent.work_per_call, Some(WorkUnits(11)));
                    absent.work_per_call = None;
                    zero.work_per_call = Some(WorkUnits(0));
                }
                ExpiryField::TemporaryHeap => {
                    assert_eq!(absent.temporary_heap_peak, Some(RequestedBytes(7)));
                    absent.temporary_heap_peak = None;
                    zero.temporary_heap_peak = Some(RequestedBytes(0));
                }
                ExpiryField::RetainedState => {
                    assert_eq!(absent.retained_state, Some(RequestedBytes(9)));
                    absent.retained_state = None;
                    zero.retained_state = Some(RequestedBytes(0));
                }
                ExpiryField::Frame => {
                    assert_eq!(absent.frame, Some(FrameBytes(5)));
                    absent.frame = None;
                    zero.frame = Some(FrameBytes(0));
                }
            }
            assert_eq!(
                evaluate_expiry_hypothetical(schedule, absent),
                Err(Refusal::MissingExpiryFact(field))
            );
            assert_eq!(
                evaluate_expiry_hypothetical(schedule, zero),
                Err(Refusal::InvalidExpiryFact(field))
            );
        }
        assert_eq!(
            evaluate_expiry_hypothetical(
                ExpirySchedule {
                    serial_temporary_witness: None,
                    ..schedule
                },
                facts,
            ),
            Err(Refusal::MissingCommonProof(CommonGap::ExpirySchedule))
        );
    }

    #[test]
    fn expiry_work_storage_and_frame_dimensions_vary_independently() {
        let (schedule, facts, expected) = expiry_fixture();
        assert_eq!(evaluate_expiry_hypothetical(schedule, facts), Ok(expected));
        assert_eq!(
            evaluate_expiry_hypothetical(
                ExpirySchedule {
                    invocations: 4,
                    ..schedule
                },
                facts
            ),
            Ok(HypotheticalExpiry {
                cumulative_work: WorkUnits(44),
                ..expected
            })
        );
        for (changed, result) in [
            (
                ExpiryFacts {
                    work_per_call: Some(WorkUnits(13)),
                    ..facts
                },
                HypotheticalExpiry {
                    cumulative_work: WorkUnits(39),
                    ..expected
                },
            ),
            (
                ExpiryFacts {
                    temporary_heap_peak: Some(RequestedBytes(17)),
                    ..facts
                },
                HypotheticalExpiry {
                    temporary_heap_peak: RequestedBytes(17),
                    ..expected
                },
            ),
            (
                ExpiryFacts {
                    retained_state: Some(RequestedBytes(23)),
                    ..facts
                },
                HypotheticalExpiry {
                    retained_state: RequestedBytes(23),
                    ..expected
                },
            ),
            (
                ExpiryFacts {
                    frame: Some(FrameBytes(29)),
                    ..facts
                },
                HypotheticalExpiry {
                    peak_frame: FrameBytes(29),
                    ..expected
                },
            ),
        ] {
            assert_ne!(changed, facts);
            assert_ne!(result, expected);
            assert_eq!(evaluate_expiry_hypothetical(schedule, changed), Ok(result));
        }
    }

    #[test]
    fn checked_overflow_refuses_instead_of_wrapping() {
        let (mut formula, mut facts) = fixture();
        assert_eq!(facts[2].owner, CALLBACK);
        facts[2].bytes = FrameBytes(u64::MAX);
        assert_eq!(
            evaluate_hypothetical(&formula, depth(), identity(), &facts),
            Err(Refusal::Overflow(ArithmeticDomain::Frames))
        );
        formula.path = recursive(DepthAxis::Instance, ROOT);
        formula.required_frames = vec![ROOT];
        assert_eq!(facts[0].bytes, FrameBytes(2));
        assert_eq!(
            evaluate_hypothetical(
                &formula,
                DepthInputs {
                    instance_containers: u64::MAX,
                    ..depth()
                },
                identity(),
                &facts[..1],
            ),
            Err(Refusal::Overflow(ArithmeticDomain::Frames))
        );
        let (schedule, expiry, _) = expiry_fixture();
        assert_eq!(
            evaluate_expiry_hypothetical(
                ExpirySchedule {
                    invocations: u64::MAX,
                    ..schedule
                },
                expiry
            ),
            Err(Refusal::Overflow(ArithmeticDomain::Work))
        );
    }
}

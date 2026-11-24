//! Tests-first lifetime-composition contract, not a source proof or serving API.
//!
//! All facts below are supplied hypotheses, including coverage and lifetimes.
//! Evaluating them must never issue source-proof authority. Later source joins
//! must reuse the existing MetaCache/BundleHeap owners, not duplicate them here.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnerId(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationId(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StateId(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HeapBytes(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InlineBytes(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceState {
    FreshConstruction,
    Callback,
    HostnameValidation,
    PartialFailure,
    Released,
}

// Half-open: live at `first`, already released on entry to `released_at`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Lifetime {
    first: StateId,
    released_at: StateId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HeapKind {
    DisjointLiveOwner,
    TransientRelocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HeapOwner {
    owner: OwnerId,
    allocation: AllocationId,
    kind: HeapKind,
    bytes: HeapBytes,
    lifetime: Lifetime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InlineWrapper {
    owner: OwnerId,
    bytes: InlineBytes,
    lifetime: Lifetime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoverageObligation {
    child: OwnerId,
    lifetime: Lifetime,
}

// The child's heap is included in the parent's bytes; this is not an allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CoveredBy {
    child: OwnerId,
    parent: OwnerId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Release {
    owner: OwnerId,
    at: StateId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LifetimeTrace {
    states: Vec<SourceState>,
    heap: Vec<HeapOwner>,
    wrapper: InlineWrapper,
    coverage_obligations: Vec<CoverageObligation>,
    covered_by: Vec<CoveredBy>,
    releases: Vec<Release>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct HypotheticalPeak {
    heap: HeapBytes,
    wrapper_inline: InlineBytes,
    combined: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SerialBoundary {
    UnchangedAndExclusive,
    OverlappingCall,
    EscapingCallback,
    CacheReset,
    SourceChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StorageError {
    MissingSourceProof,
    MissingCoverage(OwnerId),
    DuplicateCoverage(OwnerId),
    InvalidCoverage { child: OwnerId, parent: OwnerId },
    CoveredChildAlsoOwned(OwnerId),
    DuplicateOwnership(AllocationId),
    InvalidLifetime(OwnerId),
    ReleaseMismatch { owner: OwnerId, expected: StateId },
    UnexpectedRelease(Release),
    LiveHeapOverflow(StateId),
    CombinedOverflow(StateId),
    SerialBoundaryCount,
    InvalidSerialComposition(SerialBoundary),
}

// Normative target: checked sum of disjoint live heap plus live relocation at
// each state, then maximum across states. Inline wrapper bytes stay separate;
// combined is the checked per-state heap + inline maximum, not another heap row.
fn evaluate_supplied_facts_hypothetically(
    _trace: &LifetimeTrace,
) -> Result<HypotheticalPeak, StorageError> {
    Err(StorageError::MissingSourceProof)
}

// Exactly one unchanged/exclusive boundary per adjacent call is required.
// Calls compose by maximum storage, never by call count or cumulative sums.
fn evaluate_serial_facts_hypothetically(
    _calls: &[LifetimeTrace],
    _boundaries: &[SerialBoundary],
) -> Result<HypotheticalPeak, StorageError> {
    Err(StorageError::MissingSourceProof)
}

#[cfg(test)]
mod tests {
    use super::{
        AllocationId, CoverageObligation, CoveredBy, HeapBytes, HeapKind, HeapOwner,
        HypotheticalPeak, InlineBytes, InlineWrapper, Lifetime, LifetimeTrace, OwnerId, Release,
        SerialBoundary, SourceState, StateId, StorageError, evaluate_serial_facts_hypothetically,
        evaluate_supplied_facts_hypothetically,
    };

    const META: OwnerId = OwnerId("hypothetical-meta-cache");
    const SIBLING: OwnerId = OwnerId("hypothetical-sibling-cache");
    const CALLBACK: OwnerId = OwnerId("hypothetical-callback-work");
    const RELOCATION: OwnerId = OwnerId("hypothetical-relocation");
    const FALLBACK: OwnerId = OwnerId("hypothetical-covered-fallback");
    const WRAPPER: OwnerId = OwnerId("hypothetical-inline-wrapper");
    const HOSTNAME: OwnerId = OwnerId("hypothetical-hostname-cache");
    const CONSTRUCTION: OwnerId = OwnerId("hypothetical-construction-work");

    fn lifetime(first: usize, released_at: usize) -> Lifetime {
        Lifetime {
            first: StateId(first),
            released_at: StateId(released_at),
        }
    }

    fn heap_owner(owner: OwnerId, bytes: u64, first: usize, released_at: usize) -> HeapOwner {
        HeapOwner {
            owner,
            allocation: AllocationId(owner.0),
            kind: HeapKind::DisjointLiveOwner,
            bytes: HeapBytes(bytes),
            lifetime: lifetime(first, released_at),
        }
    }

    fn peak(heap: u64, wrapper_inline: u64, combined: u64) -> HypotheticalPeak {
        HypotheticalPeak {
            heap: HeapBytes(heap),
            wrapper_inline: InlineBytes(wrapper_inline),
            combined,
        }
    }

    fn hypothetical_trace() -> LifetimeTrace {
        LifetimeTrace {
            states: vec![
                SourceState::FreshConstruction,
                SourceState::Callback,
                SourceState::Released,
            ],
            heap: vec![
                heap_owner(META, 11, 0, 2),
                heap_owner(SIBLING, 29, 0, 2),
                heap_owner(CALLBACK, 13, 1, 2),
                HeapOwner {
                    kind: HeapKind::TransientRelocation,
                    ..heap_owner(RELOCATION, 17, 1, 2)
                },
            ],
            wrapper: InlineWrapper {
                owner: WRAPPER,
                bytes: InlineBytes(7),
                lifetime: lifetime(0, 2),
            },
            coverage_obligations: vec![CoverageObligation {
                child: FALLBACK,
                lifetime: lifetime(0, 2),
            }],
            covered_by: vec![CoveredBy {
                child: FALLBACK,
                parent: META,
            }],
            releases: vec![
                Release {
                    owner: META,
                    at: StateId(2),
                },
                Release {
                    owner: SIBLING,
                    at: StateId(2),
                },
                Release {
                    owner: CALLBACK,
                    at: StateId(2),
                },
                Release {
                    owner: RELOCATION,
                    at: StateId(2),
                },
                Release {
                    owner: WRAPPER,
                    at: StateId(2),
                },
            ],
        }
    }

    fn set_heap_bytes(trace: &mut LifetimeTrace, owner: OwnerId, bytes: u64) {
        let row = trace
            .heap
            .iter_mut()
            .find(|row| row.owner == owner)
            .unwrap();
        row.bytes = HeapBytes(bytes);
    }

    #[test]
    fn fallback_is_covered_once() {
        let original = hypothetical_trace();
        assert_eq!(original.coverage_obligations[0].child, FALLBACK);
        assert_eq!(original.coverage_obligations[0].lifetime, lifetime(0, 2));
        assert_eq!(original.covered_by.len(), 1);
        assert_eq!(
            (original.covered_by[0].child, original.covered_by[0].parent),
            (FALLBACK, META)
        );
        assert!(!original.heap.iter().any(|row| row.owner == FALLBACK));
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&original),
            Ok(peak(70, 7, 77))
        );

        let mut missing = original.clone();
        assert_eq!(missing.covered_by.remove(0), original.covered_by[0]);
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&missing),
            Err(StorageError::MissingCoverage(child))
                if StorageError::MissingCoverage(child) == StorageError::MissingCoverage(FALLBACK)
        ));

        let mut duplicate = original.clone();
        duplicate.covered_by.push(original.covered_by[0]);
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&duplicate),
            Err(StorageError::DuplicateCoverage(child))
                if StorageError::DuplicateCoverage(child) == StorageError::DuplicateCoverage(FALLBACK)
        ));

        let mut counted_twice = original.clone();
        counted_twice.heap.push(heap_owner(FALLBACK, 5, 0, 2));
        counted_twice.releases.push(Release {
            owner: FALLBACK,
            at: StateId(2),
        });
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&counted_twice),
            Err(StorageError::CoveredChildAlsoOwned(child))
                if StorageError::CoveredChildAlsoOwned(child)
                    == StorageError::CoveredChildAlsoOwned(FALLBACK)
        ));

        let mut unknown_parent = original.clone();
        let unknown = OwnerId("hypothetical-unknown-parent");
        unknown_parent.covered_by[0].parent = unknown;
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&unknown_parent),
            Err(StorageError::InvalidCoverage { child, parent })
                if (child, parent) == (FALLBACK, unknown)
        ));

        let mut outliving_parent = original.clone();
        outliving_parent.coverage_obligations[0]
            .lifetime
            .released_at = StateId(3);
        outliving_parent.states.push(SourceState::Released);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&outliving_parent),
            Err(StorageError::InvalidCoverage {
                child: FALLBACK,
                parent: META
            })
        );
    }

    #[test]
    fn heap_terms_and_inline_wrapper_vary_independently() {
        let original = hypothetical_trace();
        let supplied_heap = original
            .heap
            .iter()
            .map(|row| {
                (
                    row.owner,
                    row.allocation.0,
                    row.kind,
                    row.bytes.0,
                    row.lifetime.first.0,
                    row.lifetime.released_at.0,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            supplied_heap,
            vec![
                (META, META.0, HeapKind::DisjointLiveOwner, 11, 0, 2),
                (SIBLING, SIBLING.0, HeapKind::DisjointLiveOwner, 29, 0, 2),
                (CALLBACK, CALLBACK.0, HeapKind::DisjointLiveOwner, 13, 1, 2),
                (
                    RELOCATION,
                    RELOCATION.0,
                    HeapKind::TransientRelocation,
                    17,
                    1,
                    2
                ),
            ]
        );
        // Literal oracle: heap children 11 + 29, callback 13, relocation 17.
        // The inline 7 explains combined 77; it is NOT another heap allocation.
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&original).map(|value| (
                value.heap.0,
                value.wrapper_inline.0,
                value.combined
            )),
            Ok((70, 7, 77))
        );
        for (owner, bytes, expected) in [
            (META, 16, peak(75, 7, 82)),
            (SIBLING, 37, peak(78, 7, 85)),
            (CALLBACK, 19, peak(76, 7, 83)),
            (RELOCATION, 23, peak(76, 7, 83)),
        ] {
            let mut changed = original.clone();
            set_heap_bytes(&mut changed, owner, bytes);
            assert_eq!(
                evaluate_supplied_facts_hypothetically(&changed),
                Ok(expected)
            );
        }
        let mut inline_only = original.clone();
        inline_only.wrapper.bytes = InlineBytes(31);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&inline_only),
            Ok(peak(70, 31, 101))
        );
        assert_eq!(inline_only.heap, original.heap);
    }

    #[test]
    fn hostname_overlaps_the_live_wrapper() {
        let mut original = hypothetical_trace();
        original.states[1] = SourceState::HostnameValidation;
        original.heap.push(heap_owner(HOSTNAME, 19, 1, 2));
        original.releases.push(Release {
            owner: HOSTNAME,
            at: StateId(2),
        });
        assert_eq!(original.wrapper.owner, WRAPPER);
        assert_eq!(original.wrapper.bytes, InlineBytes(7));
        assert_eq!(original.wrapper.lifetime, lifetime(0, 2));
        assert!(
            original
                .heap
                .iter()
                .any(|row| { row.owner == SIBLING && row.lifetime == lifetime(0, 2) })
        );
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&original),
            Ok(peak(89, 7, 96))
        );

        let mut hostname_only = original.clone();
        set_heap_bytes(&mut hostname_only, HOSTNAME, 23);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&hostname_only),
            Ok(peak(93, 7, 100))
        );
        let mut sibling_only = original.clone();
        set_heap_bytes(&mut sibling_only, SIBLING, 31);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&sibling_only),
            Ok(peak(91, 7, 98))
        );
        let mut inline_only = original.clone();
        inline_only.wrapper.bytes = InlineBytes(9);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&inline_only),
            Ok(peak(89, 9, 98))
        );
    }

    #[test]
    fn fresh_cache_construction_and_partial_failure_are_included() {
        let mut fresh = hypothetical_trace();
        fresh.heap.push(HeapOwner {
            kind: HeapKind::TransientRelocation,
            ..heap_owner(CONSTRUCTION, 41, 0, 1)
        });
        fresh.releases.push(Release {
            owner: CONSTRUCTION,
            at: StateId(1),
        });
        // Construction 11 + 29 + 41 = 81 exceeds the later 70, not 81 + 70.
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&fresh),
            Ok(peak(81, 7, 88))
        );

        let mut partial_failure = fresh.clone();
        partial_failure.states[1] = SourceState::PartialFailure;
        set_heap_bytes(&mut partial_failure, RELOCATION, 61);
        // Failure still owns 11 + 29 + 13 + 61 until the exact release state.
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&partial_failure),
            Ok(peak(114, 7, 121))
        );

        let mut missing_release = partial_failure.clone();
        let index = missing_release
            .releases
            .iter()
            .position(|row| row.owner == CONSTRUCTION)
            .unwrap();
        assert_eq!(
            missing_release.releases.remove(index),
            Release {
                owner: CONSTRUCTION,
                at: StateId(1)
            }
        );
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&missing_release),
            Err(StorageError::ReleaseMismatch { owner, expected })
                if (owner, expected) == (CONSTRUCTION, StateId(1))
        ));
    }

    #[test]
    fn exact_release_rejects_missing_early_late_and_duplicate_events() {
        let original = hypothetical_trace();
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&original),
            Ok(peak(70, 7, 77))
        );
        for (index, release) in original.releases.iter().enumerate() {
            let expected = Err(StorageError::ReleaseMismatch {
                owner: release.owner,
                expected: release.at,
            });
            let mut missing = original.clone();
            assert_eq!(missing.releases.remove(index), *release);
            assert_eq!(evaluate_supplied_facts_hypothetically(&missing), expected);
            let mut duplicate = original.clone();
            duplicate.releases.push(*release);
            assert_eq!(evaluate_supplied_facts_hypothetically(&duplicate), expected);
            for at in [StateId(1), StateId(3)] {
                let mut changed = original.clone();
                changed.states.push(SourceState::Released);
                changed.releases[index].at = at;
                assert_eq!(evaluate_supplied_facts_hypothetically(&changed), expected);
            }
        }
        let mut extra = original.clone();
        let release = Release {
            owner: FALLBACK,
            at: StateId(2),
        };
        extra.releases.push(release);
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&extra),
            Err(StorageError::UnexpectedRelease(actual))
                if StorageError::UnexpectedRelease(actual) == StorageError::UnexpectedRelease(release)
        ));
    }

    #[test]
    fn serial_calls_do_not_multiply_peak() {
        let original = hypothetical_trace();
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&original),
            Ok(peak(70, 7, 77))
        );
        assert_eq!(
            evaluate_serial_facts_hypothetically(&[original.clone()], &[]),
            Ok(peak(70, 7, 77))
        );
        let calls = [original.clone(), original.clone(), original.clone()];
        let boundaries = [SerialBoundary::UnchangedAndExclusive; 2];
        assert_eq!(
            evaluate_serial_facts_hypothetically(&calls, &boundaries),
            Ok(peak(70, 7, 77))
        );

        let mut different_calls = calls.clone();
        set_heap_bytes(&mut different_calls[1], CALLBACK, 31);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&different_calls[1]),
            Ok(peak(88, 7, 95))
        );
        assert_eq!(
            evaluate_serial_facts_hypothetically(&different_calls, &boundaries),
            Ok(peak(88, 7, 95))
        );

        let mut at_limit = original.clone();
        set_heap_bytes(&mut at_limit, META, u64::MAX - 66);
        let large_calls = [at_limit.clone(), at_limit];
        assert_eq!(
            evaluate_serial_facts_hypothetically(
                &large_calls,
                &[SerialBoundary::UnchangedAndExclusive]
            ),
            Ok(peak(u64::MAX - 7, 7, u64::MAX))
        );
    }

    #[test]
    fn overlap_change_invalidates_serial_composition() {
        let original = hypothetical_trace();
        let calls = [original.clone(), original];
        assert_eq!(
            evaluate_serial_facts_hypothetically(&calls, &[SerialBoundary::UnchangedAndExclusive]),
            Ok(peak(70, 7, 77))
        );
        for invalidation in [
            SerialBoundary::OverlappingCall,
            SerialBoundary::EscapingCallback,
            SerialBoundary::CacheReset,
            SerialBoundary::SourceChanged,
        ] {
            assert!(matches!(
                evaluate_serial_facts_hypothetically(&calls, &[invalidation]),
                Err(StorageError::InvalidSerialComposition(actual))
                    if StorageError::InvalidSerialComposition(actual)
                        == StorageError::InvalidSerialComposition(invalidation)
            ));
        }
        assert_eq!(
            evaluate_serial_facts_hypothetically(&calls, &[]),
            Err(StorageError::SerialBoundaryCount)
        );
        assert_eq!(
            evaluate_serial_facts_hypothetically(
                &calls,
                &[SerialBoundary::UnchangedAndExclusive; 2]
            ),
            Err(StorageError::SerialBoundaryCount)
        );
    }

    #[test]
    fn duplicate_ownership_is_not_a_disjoint_live_child() {
        let original = hypothetical_trace();
        assert_ne!(original.heap[0].allocation, original.heap[1].allocation);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&original),
            Ok(peak(70, 7, 77))
        );
        for owner in [SIBLING, RELOCATION] {
            let mut aliased = original.clone();
            let allocation = aliased.heap[0].allocation;
            let row = aliased
                .heap
                .iter_mut()
                .find(|row| row.owner == owner)
                .unwrap();
            assert_ne!(row.owner, META);
            row.allocation = allocation;
            assert!(matches!(
                evaluate_supplied_facts_hypothetically(&aliased),
                Err(StorageError::DuplicateOwnership(actual))
                    if StorageError::DuplicateOwnership(actual)
                        == StorageError::DuplicateOwnership(allocation)
            ));
        }
    }

    #[test]
    fn checked_sums_reject_heap_and_combined_overflow() {
        let mut boundary = hypothetical_trace();
        set_heap_bytes(&mut boundary, META, u64::MAX - 66);
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&boundary),
            Ok(peak(u64::MAX - 7, 7, u64::MAX))
        );
        let mut heap_overflow = boundary.clone();
        set_heap_bytes(&mut heap_overflow, META, u64::MAX - 58);
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&heap_overflow),
            Err(StorageError::LiveHeapOverflow(at))
                if StorageError::LiveHeapOverflow(at) == StorageError::LiveHeapOverflow(StateId(1))
        ));
        let mut combined_overflow = boundary.clone();
        combined_overflow.wrapper.bytes = InlineBytes(8);
        assert!(matches!(
            evaluate_supplied_facts_hypothetically(&combined_overflow),
            Err(StorageError::CombinedOverflow(at))
                if StorageError::CombinedOverflow(at) == StorageError::CombinedOverflow(StateId(1))
        ));
    }

    #[test]
    fn lifetimes_require_ordered_in_trace_release_states() {
        let original = hypothetical_trace();
        assert_eq!(
            evaluate_supplied_facts_hypothetically(&original),
            Ok(peak(70, 7, 77))
        );
        for invalid in [lifetime(1, 1), lifetime(2, 1), lifetime(0, 3)] {
            let mut changed = original.clone();
            let row = changed
                .heap
                .iter_mut()
                .find(|row| row.owner == CALLBACK)
                .unwrap();
            row.lifetime = invalid;
            assert!(matches!(
                evaluate_supplied_facts_hypothetically(&changed),
                Err(StorageError::InvalidLifetime(owner))
                    if StorageError::InvalidLifetime(owner) == StorageError::InvalidLifetime(CALLBACK)
            ));
        }
    }
}

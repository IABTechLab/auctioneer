//! Disjoint constructor lifetimes in requested-byte units, never physical fit.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub(super) struct Allocation<'a> {
    pub(super) owner: &'a str,
    pub(super) bytes: u64,
}

pub(super) struct Trace<'a> {
    pub(super) label: &'a str,
    pub(super) retained: &'a [Allocation<'a>],
    pub(super) scratch: &'a [Allocation<'a>],
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Totals {
    pub(super) retained: u64,
    pub(super) construction_only: u64,
    pub(super) additive_envelope: u64,
    pub(super) trace_peaks: Vec<u64>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    Owners,
    Traces,
    RetainedCoverage,
    Arithmetic,
}

pub(super) fn compose(
    final_owners: &[Allocation<'_>],
    required_traces: &[&str],
    traces: &[Trace<'_>],
) -> Result<Totals, Refusal> {
    if final_owners.is_empty() {
        return Err(Refusal::Owners);
    }
    let mut retained = 0_u64;
    let mut limits = BTreeMap::new();
    for row in final_owners {
        if row.owner.is_empty() || row.bytes == 0 || limits.insert(row.owner, row.bytes).is_some() {
            return Err(Refusal::Owners);
        }
        retained = retained.checked_add(row.bytes).ok_or(Refusal::Arithmetic)?;
    }
    let labels = required_traces.iter().copied().collect::<BTreeSet<_>>();
    if labels.is_empty()
        || labels.len() != required_traces.len()
        || labels.contains("")
        || !traces
            .iter()
            .map(|trace| trace.label)
            .eq(required_traces.iter().copied())
    {
        return Err(Refusal::Traces);
    }
    let mut construction_only = 0_u64;
    let mut trace_peaks = Vec::new();
    for trace in traces {
        let mut seen = BTreeSet::new();
        let mut live = 0_u64;
        for row in trace.retained {
            if row.bytes == 0 || !seen.insert(row.owner) {
                return Err(Refusal::Owners);
            }
            if limits.get(row.owner).is_none_or(|limit| row.bytes > *limit) {
                return Err(Refusal::RetainedCoverage);
            }
            live = live.checked_add(row.bytes).ok_or(Refusal::Arithmetic)?;
        }
        let mut scratch = 0_u64;
        for row in trace.scratch {
            if row.owner.is_empty()
                || row.bytes == 0
                || !seen.insert(row.owner)
                || limits.contains_key(row.owner)
            {
                return Err(Refusal::Owners);
            }
            scratch = scratch.checked_add(row.bytes).ok_or(Refusal::Arithmetic)?;
        }
        trace_peaks.push(live.checked_add(scratch).ok_or(Refusal::Arithmetic)?);
        construction_only = construction_only.max(scratch);
    }
    Ok(Totals {
        retained,
        construction_only,
        additive_envelope: retained
            .checked_add(construction_only)
            .ok_or(Refusal::Arithmetic)?,
        trace_peaks,
    })
}

#[cfg(test)]
mod tests {
    use super::{Allocation, Refusal, Totals, Trace, compose};

    const FIRST: Allocation<'static> = Allocation {
        owner: "fake_first",
        bytes: 3,
    };
    const SECOND: Allocation<'static> = Allocation {
        owner: "fake_second",
        bytes: 7,
    };
    const PHASES: &[&str] = &["first", "second", "late_failure"];

    #[test]
    fn constructor_lifetimes_preserve_each_retained_prefix_and_failure_peak() {
        let traces = [
            Trace {
                label: "first",
                retained: &[FIRST],
                scratch: &[Allocation {
                    owner: "fake_first_scratch",
                    bytes: 5,
                }],
            },
            Trace {
                label: "second",
                retained: &[FIRST, SECOND],
                scratch: &[Allocation {
                    owner: "fake_second_scratch",
                    bytes: 11,
                }],
            },
            Trace {
                label: "late_failure",
                retained: &[FIRST, Allocation { bytes: 6, ..SECOND }],
                scratch: &[Allocation {
                    owner: "fake_failed_scratch",
                    bytes: 13,
                }],
            },
        ];
        assert_eq!(
            compose(&[FIRST, SECOND], PHASES, &traces),
            Ok(Totals {
                retained: 10,
                construction_only: 13,
                additive_envelope: 23,
                trace_peaks: vec![8, 21, 22],
            })
        );
    }

    #[test]
    fn constructor_prefix_partial_and_scratch_axes_change_independent_terms() {
        for (first, partial, scratch, expected) in [
            (3, 6, 13, (10, 13, 22, 23)),
            (4, 6, 13, (11, 13, 23, 24)),
            (3, 7, 13, (10, 13, 23, 23)),
            (3, 6, 14, (10, 14, 23, 24)),
        ] {
            let first = Allocation {
                bytes: first,
                ..FIRST
            };
            let trace = Trace {
                label: "late_failure",
                retained: &[
                    first,
                    Allocation {
                        bytes: partial,
                        ..SECOND
                    },
                ],
                scratch: &[Allocation {
                    owner: "fake_scratch",
                    bytes: scratch,
                }],
            };
            let totals = compose(&[first, SECOND], &["late_failure"], &[trace]).unwrap();
            assert_eq!(
                (
                    totals.retained,
                    totals.construction_only,
                    totals.trace_peaks[0],
                    totals.additive_envelope
                ),
                expected
            );
        }
    }

    #[test]
    fn constructor_lifetimes_reject_missing_or_duplicate_phases() {
        let trace = Trace {
            label: "first",
            retained: &[FIRST],
            scratch: &[],
        };
        assert_eq!(compose(&[FIRST], PHASES, &[trace]), Err(Refusal::Traces));
        let traces = [
            Trace {
                label: "first",
                retained: &[FIRST],
                scratch: &[],
            },
            Trace {
                label: "first",
                retained: &[FIRST],
                scratch: &[],
            },
        ];
        assert_eq!(compose(&[FIRST], &["first"], &traces), Err(Refusal::Traces));
    }

    #[test]
    fn constructor_lifetimes_reject_overlapping_or_unfunded_retained_roles() {
        let duplicate = Trace {
            label: "first",
            retained: &[FIRST],
            scratch: &[FIRST],
        };
        assert_eq!(
            compose(&[FIRST], &["first"], &[duplicate]),
            Err(Refusal::Owners)
        );
        for retained in [Allocation { bytes: 4, ..FIRST }, SECOND] {
            let trace = Trace {
                label: "first",
                retained: &[retained],
                scratch: &[],
            };
            assert_eq!(
                compose(&[FIRST], &["first"], &[trace]),
                Err(Refusal::RetainedCoverage)
            );
        }
    }

    #[test]
    fn constructor_lifetimes_refuse_retained_scratch_and_additive_overflow() {
        let maximum = Allocation {
            bytes: u64::MAX,
            ..FIRST
        };
        let scratch = Allocation {
            owner: "fake_scratch",
            bytes: 1,
        };
        let trace = Trace {
            label: "first",
            retained: &[],
            scratch: &[scratch],
        };
        assert_eq!(
            compose(&[maximum, SECOND], &["first"], &[trace]),
            Err(Refusal::Arithmetic)
        );
        let trace = Trace {
            label: "first",
            retained: &[],
            scratch: &[
                Allocation {
                    bytes: u64::MAX,
                    ..scratch
                },
                SECOND,
            ],
        };
        assert_eq!(
            compose(&[FIRST], &["first"], &[trace]),
            Err(Refusal::Arithmetic)
        );
        let trace = Trace {
            label: "first",
            retained: &[],
            scratch: &[scratch],
        };
        assert_eq!(
            compose(&[maximum], &["first"], &[trace]),
            Err(Refusal::Arithmetic)
        );
    }
}

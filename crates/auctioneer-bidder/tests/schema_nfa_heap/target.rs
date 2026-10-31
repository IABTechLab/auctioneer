//! Conditional target requested heap for the selected retained NFA owners.
//! The producer must bind the supplied layouts and fresh compiler-produced NFA
//! topology to the pinned source, std features, Rust 1.98.1 and target ABI.
//! Excludes constructor peaks, other programs, caches, work, stack and physical
//! memory. Native topology is structural input, never a target layout witness.

use super::{GroupInfoRetainedHeap, NativeField, NfaRetainedHeap};
use regex_automata::{
    nfa::thompson::{NFA, State, Transition},
    util::{
        captures::GroupInfo,
        look::Look,
        primitives::{PatternID, SmallIndex, StateID},
    },
};
use std::{
    any::type_name,
    collections::{BTreeMap, HashMap},
    sync::{Arc, LazyLock, atomic::AtomicUsize},
};

// Verified declaration names in the pinned vendor source, not public wrappers.
pub(crate) const NFA_INNER: &str = "regex_automata::nfa::thompson::nfa::Inner";
pub(crate) const GROUP_INFO_INNER: &str = "regex_automata::util::captures::GroupInfoInner";
pub(crate) const ABI_POINTER: &str = "compiler_abi::Pointer";

/// Required fact keys, not assumed DWARF spellings for tuples or generics.
/// The producer must verify each compiler-name mapping; aliases and public
/// wrapper layouts cannot substitute for either private root payload.
pub(crate) static REQUIRED_TYPES: LazyLock<[&'static str; 11]> = LazyLock::new(|| {
    [
        NFA_INNER,
        GROUP_INFO_INNER,
        type_name::<State>(),
        type_name::<Transition>(),
        type_name::<StateID>(),
        type_name::<(SmallIndex, SmallIndex)>(),
        type_name::<HashMap<Arc<str>, SmallIndex>>(),
        type_name::<Vec<Option<Arc<str>>>>(),
        type_name::<Option<Arc<str>>>(),
        type_name::<AtomicUsize>(),
        ABI_POINTER,
    ]
});

pub(crate) fn fact_keys() -> [&'static str; 11] {
    *REQUIRED_TYPES
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum TargetHeapError {
    MissingType(String),
    InvalidLayout(String),
    UnsupportedAbi,
    Arithmetic,
    NamesUnqualified,
    UnboundShape,
}

pub(crate) fn project_nfa(
    nfa: &NFA,
    facts: &BTreeMap<String, NativeField>,
) -> Result<NfaRetainedHeap, TargetHeapError> {
    let layout = TargetLayout::new(facts)?;
    let (group_info, sparse_bytes, union_bytes) = selected_topology(nfa, &layout)?;
    let states = nfa.states().len();
    // nfa.rs:1339-1343,1384,1402: fresh monotonic states, then shrink;
    // start_pattern is an exact slice copy. Children are boxed by builder.rs.
    let arc_bytes = layout.arc(NFA_INNER)?;
    let states_bytes = layout.append(type_name::<State>(), states)?;
    let start_patterns_bytes = layout.array(type_name::<StateID>(), nfa.pattern_len())?;
    let requested_heap_bytes = sum(&[
        arc_bytes,
        states_bytes,
        start_patterns_bytes,
        sparse_bytes,
        union_bytes,
        group_info.requested_heap_bytes,
    ])?;
    Ok(NfaRetainedHeap {
        arc_bytes,
        states_bytes,
        start_patterns_bytes,
        sparse_bytes,
        dense_bytes: 0,
        union_bytes,
        group_info,
        states,
        requested_heap_bytes,
    })
}

fn selected_topology(
    nfa: &NFA,
    layout: &TargetLayout<'_>,
) -> Result<(GroupInfoRetainedHeap, usize, usize), TargetHeapError> {
    let states = nfa.states().len();
    let anchor = if nfa.is_reverse() {
        Look::Start
    } else {
        Look::End
    };
    if states == 0
        || nfa.pattern_len() != 1
        || !nfa.is_utf8()
        || nfa.has_empty()
        || !nfa.look_set_any().contains(anchor)
    {
        return Err(TargetHeapError::UnboundShape);
    }
    let group_info = group_info_heap(nfa.group_info(), layout)?;
    if group_info.patterns != usize::from(!nfa.is_reverse()) {
        return Err(TargetHeapError::UnboundShape);
    }
    let valid_id = |id: StateID| id.as_usize() < states;
    if !valid_id(nfa.start_anchored())
        || !valid_id(nfa.start_unanchored())
        || nfa
            .start_pattern(PatternID::ZERO)
            .is_none_or(|id| !valid_id(id))
    {
        return Err(TargetHeapError::UnboundShape);
    }
    let mut sparse_bytes = 0_usize;
    let mut union_bytes = 0_usize;
    for state in nfa.states() {
        match state {
            State::ByteRange { trans } if valid_transition(*trans) && valid_id(trans.next) => {}
            State::Sparse(sparse) => {
                if sparse.transitions.len() < 2
                    || !sparse
                        .transitions
                        .iter()
                        .copied()
                        .all(|trans| valid_transition(trans) && valid_id(trans.next))
                    || sparse
                        .transitions
                        .array_windows::<2>()
                        .any(|[left, right]| left.end >= right.start)
                {
                    return Err(TargetHeapError::UnboundShape);
                }
                sparse_bytes = add(
                    sparse_bytes,
                    layout.array(type_name::<Transition>(), sparse.transitions.len())?,
                )?;
            }
            State::Union { alternates } => {
                if alternates.len() < 3 || !alternates.iter().copied().all(valid_id) {
                    return Err(TargetHeapError::UnboundShape);
                }
                union_bytes = add(
                    union_bytes,
                    layout.array(type_name::<StateID>(), alternates.len())?,
                )?;
            }
            State::Look {
                look: Look::Start | Look::End,
                next,
            } if valid_id(*next) => {}
            State::BinaryUnion { alt1, alt2 } if valid_id(*alt1) && valid_id(*alt2) => {}
            State::Capture {
                next,
                pattern_id,
                group_index,
                slot,
            } if valid_id(*next)
                && *pattern_id == PatternID::ZERO
                && group_index.as_usize() < group_info.groups
                && slot.as_usize() < nfa.group_info().slot_len() => {}
            State::Match { pattern_id } if *pattern_id == PatternID::ZERO => {}
            State::Fail => {}
            State::ByteRange { .. }
            | State::Dense(_)
            | State::Look { .. }
            | State::BinaryUnion { .. }
            | State::Capture { .. }
            | State::Match { .. } => return Err(TargetHeapError::UnboundShape),
        }
    }
    Ok((group_info, sparse_bytes, union_bytes))
}

pub(crate) fn project_group_info(
    info: &GroupInfo,
    facts: &BTreeMap<String, NativeField>,
) -> Result<GroupInfoRetainedHeap, TargetHeapError> {
    group_info_heap(info, &TargetLayout::new(facts)?)
}

fn group_info_heap(
    info: &GroupInfo,
    layout: &TargetLayout<'_>,
) -> Result<GroupInfoRetainedHeap, TargetHeapError> {
    let patterns = info.pattern_len();
    if patterns > 1 {
        return Err(TargetHeapError::UnboundShape);
    }
    let groups = if patterns == 0 {
        0
    } else {
        info.group_len(PatternID::ZERO)
    };
    if (patterns != 0 && groups == 0) || info.slot_len() != mul(groups, 2)? {
        return Err(TargetHeapError::UnboundShape);
    }
    let mut names = 0_usize;
    for (pid, index, name) in info.all_names() {
        if name.is_some() {
            return Err(TargetHeapError::NamesUnqualified);
        }
        if pid != PatternID::ZERO || index != names {
            return Err(TargetHeapError::UnboundShape);
        }
        names = add(names, 1)?;
    }
    if names != groups {
        return Err(TargetHeapError::UnboundShape);
    }
    // captures.rs:2172-2207,2253: unnamed maps stay empty, while outer
    // vectors and each vec![None]/push vector retain their own capacities.
    let arc_bytes = layout.arc(GROUP_INFO_INNER)?;
    let slot_ranges_bytes = layout.append(type_name::<(SmallIndex, SmallIndex)>(), patterns)?;
    let name_maps_bytes = layout.append(type_name::<HashMap<Arc<str>, SmallIndex>>(), patterns)?;
    let index_vectors_bytes = layout.append(type_name::<Vec<Option<Arc<str>>>>(), patterns)?;
    let name_entries_bytes = layout.append(type_name::<Option<Arc<str>>>(), groups)?;
    let requested_heap_bytes = sum(&[
        arc_bytes,
        slot_ranges_bytes,
        name_maps_bytes,
        index_vectors_bytes,
        name_entries_bytes,
    ])?;
    Ok(GroupInfoRetainedHeap {
        arc_bytes,
        slot_ranges_bytes,
        name_maps_bytes,
        index_vectors_bytes,
        name_entries_bytes,
        patterns,
        groups,
        requested_heap_bytes,
    })
}

struct TargetLayout<'a> {
    facts: &'a BTreeMap<String, NativeField>,
    maximum: usize,
}

impl<'a> TargetLayout<'a> {
    fn new(facts: &'a BTreeMap<String, NativeField>) -> Result<Self, TargetHeapError> {
        for name in fact_keys() {
            let field = facts
                .get(name)
                .ok_or_else(|| TargetHeapError::MissingType(name.into()))?;
            if field.bytes == 0
                || !field.alignment.is_power_of_two()
                || !field.bytes.is_multiple_of(field.alignment)
            {
                return Err(TargetHeapError::InvalidLayout(name.into()));
            }
        }
        let word = facts[ABI_POINTER];
        if !matches!(word.bytes, 4 | 8)
            || word.alignment != word.bytes
            || facts[type_name::<AtomicUsize>()] != word
        {
            return Err(TargetHeapError::UnsupportedAbi);
        }
        let maximum = if word.bytes == 4 {
            usize::try_from(i32::MAX)
        } else {
            usize::try_from(i64::MAX)
        }
        .map_err(|_error| TargetHeapError::Arithmetic)?;
        let layout = Self { facts, maximum };
        for name in fact_keys() {
            let field = facts[name];
            layout.request(field.bytes, field.alignment)?;
        }
        Ok(layout)
    }

    fn request(&self, bytes: usize, alignment: usize) -> Result<usize, TargetHeapError> {
        // Rust 1.98.1 core/alloc/layout.rs:77-97: maximum size is
        // target isize::MAX + 1 - alignment, including rounded-up padding.
        let padding = alignment
            .checked_sub(1)
            .ok_or(TargetHeapError::Arithmetic)?;
        let maximum = self
            .maximum
            .checked_sub(padding)
            .ok_or(TargetHeapError::Arithmetic)?;
        if bytes > maximum {
            return Err(TargetHeapError::Arithmetic);
        }
        Ok(bytes)
    }

    fn arc(&self, name: &str) -> Result<usize, TargetHeapError> {
        // Rust 1.98.1 alloc/sync.rs:388-413: repr(C, align(2)), two
        // Atomic<usize> counters, followed by the aligned payload and tail pad.
        let atomic = self.facts[type_name::<AtomicUsize>()];
        let payload = self.facts[name];
        let alignment = atomic.alignment.max(payload.alignment);
        let offset = mul(atomic.bytes, 2)?
            .checked_next_multiple_of(payload.alignment)
            .ok_or(TargetHeapError::Arithmetic)?;
        let bytes = add(offset, payload.bytes)?
            .checked_next_multiple_of(alignment)
            .ok_or(TargetHeapError::Arithmetic)?;
        self.request(bytes, alignment)
    }

    fn array(&self, name: &str, count: usize) -> Result<usize, TargetHeapError> {
        let field = self.facts[name];
        self.request(mul(field.bytes, count)?, field.alignment)
    }

    fn append(&self, name: &str, high_water: usize) -> Result<usize, TargetHeapError> {
        if high_water == 0 {
            return Ok(0);
        }
        let field = self.facts[name];
        // Rust 1.98.1 alloc/raw_vec/mod.rs:158-171,503-524. On monotonic
        // append paths cap <= max(2*high_water, MIN_NON_ZERO_CAP), before shrink.
        let minimum = if field.bytes == 1 {
            8
        } else if field.bytes <= 1024 {
            4
        } else {
            1
        };
        self.array(name, mul(high_water, 2)?.max(minimum))
    }
}

fn valid_transition(trans: Transition) -> bool {
    // The only selected non-ASCII byte range is the compiler's unanchored
    // AnyByte prefix, not an unqualified Unicode compilation path.
    trans.start <= trans.end && (trans.end.is_ascii() || (trans.start == 0 && trans.end == 255))
}

fn add(left: usize, right: usize) -> Result<usize, TargetHeapError> {
    left.checked_add(right).ok_or(TargetHeapError::Arithmetic)
}

fn mul(left: usize, right: usize) -> Result<usize, TargetHeapError> {
    left.checked_mul(right).ok_or(TargetHeapError::Arithmetic)
}

fn sum(values: &[usize]) -> Result<usize, TargetHeapError> {
    values.iter().try_fold(0, |total, value| add(total, *value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};
    use crate::schema_formats::HOSTNAME_SOURCE;
    use regex_automata::{
        nfa::thompson::State,
        nfa::thompson::Transition,
        util::primitives::{SmallIndex, StateID},
    };
    use std::{any::type_name, collections::HashMap, sync::Arc, sync::atomic::AtomicUsize};

    // Deliberately synthetic layouts, never observations of native or guest ABI.
    // Generic Rust identities here do not establish their DWARF query names.
    fn facts(word: usize) -> BTreeMap<String, NativeField> {
        let rows = match word {
            4 => [
                (NFA_INNER, 512, 16),
                (GROUP_INFO_INNER, 48, 8),
                (type_name::<State>(), 16, 4),
                (type_name::<Transition>(), 8, 4),
                (type_name::<StateID>(), 4, 4),
                (type_name::<(SmallIndex, SmallIndex)>(), 8, 4),
                (type_name::<HashMap<Arc<str>, SmallIndex>>(), 24, 4),
                (type_name::<Vec<Option<Arc<str>>>>(), 12, 4),
                (type_name::<Option<Arc<str>>>(), 8, 4),
                (type_name::<AtomicUsize>(), 4, 4),
                (ABI_POINTER, 4, 4),
            ],
            8 => [
                (NFA_INNER, 640, 16),
                (GROUP_INFO_INNER, 96, 16),
                (type_name::<State>(), 24, 8),
                (type_name::<Transition>(), 8, 4),
                (type_name::<StateID>(), 4, 4),
                (type_name::<(SmallIndex, SmallIndex)>(), 8, 4),
                (type_name::<HashMap<Arc<str>, SmallIndex>>(), 40, 8),
                (type_name::<Vec<Option<Arc<str>>>>(), 24, 8),
                (type_name::<Option<Arc<str>>>(), 16, 8),
                (type_name::<AtomicUsize>(), 8, 8),
                (ABI_POINTER, 8, 8),
            ],
            _ => panic!("unsupported fixture word"),
        };
        rows.into_iter()
            .map(|(name, bytes, alignment)| (name.to_owned(), NativeField { bytes, alignment }))
            .collect()
    }

    #[test]
    fn target_group_charges_use_explicit_slots_and_arc_layouts() {
        let info = GroupInfo::new([[None::<&str>, None, None]]).unwrap();
        for (word, arc, maps, vectors, entries, total) in
            [(4, 56, 96, 48, 48, 280), (8, 112, 160, 96, 96, 496)]
        {
            assert_eq!(
                project_group_info(&info, &facts(word)).unwrap(),
                GroupInfoRetainedHeap {
                    arc_bytes: arc,
                    slot_ranges_bytes: 32,
                    name_maps_bytes: maps,
                    index_vectors_bytes: vectors,
                    name_entries_bytes: entries,
                    patterns: 1,
                    groups: 3,
                    requested_heap_bytes: total,
                }
            );
        }
    }

    #[test]
    fn target_empty_group_still_charges_its_aligned_arc() {
        for (word, arc) in [(4, 56), (8, 112)] {
            let heap = project_group_info(&GroupInfo::empty(), &facts(word)).unwrap();
            assert_eq!(heap.arc_bytes, arc);
            assert_eq!(heap.requested_heap_bytes, arc);
            assert_eq!((heap.patterns, heap.groups), (0, 0));
        }
    }

    #[test]
    fn target_selected_nfa_directions_keep_topology_and_target_state_slots() {
        for (pattern, groups) in [
            ("^[0-9]+$", 1),
            ("^[0-9]+$", 1),
            (r"^[0-9]+(\.[0-9]+)?$", 2),
            (r"\.[a-zA-Z]{2,3}$", 1),
            (HOSTNAME_SOURCE, 4),
        ] {
            for reverse in [false, true] {
                let nfa = if reverse {
                    reverse_nfa(pattern)
                } else {
                    forward_nfa(pattern)
                }
                .unwrap();
                for (word, arc, state_slot) in [(4, 528, 16), (8, 656, 24)] {
                    let heap = project_nfa(&nfa, &facts(word)).unwrap();
                    assert_eq!(heap.states, nfa.states().len());
                    assert_eq!(heap.arc_bytes, arc);
                    assert_eq!(heap.states_bytes, (2 * heap.states).max(4) * state_slot);
                    assert_eq!(heap.start_patterns_bytes, 4);
                    assert_eq!(heap.group_info.groups, if reverse { 0 } else { groups });
                    assert_eq!(heap.dense_bytes, 0);
                    assert_eq!(
                        heap.requested_heap_bytes,
                        heap.arc_bytes
                            + heap.states_bytes
                            + heap.start_patterns_bytes
                            + heap.sparse_bytes
                            + heap.union_bytes
                            + heap.group_info.requested_heap_bytes
                    );
                }
            }
        }
    }

    #[test]
    fn target_each_fact_is_mandatory_and_shape_checked_before_projection() {
        let nfa = forward_nfa("^[0-9]+$").unwrap();
        let complete = facts(4);
        for name in complete.keys() {
            let mut missing = complete.clone();
            missing.remove(name);
            assert_eq!(
                project_nfa(&nfa, &missing),
                Err(TargetHeapError::MissingType(name.clone()))
            );
            for field in [
                NativeField {
                    bytes: 0,
                    alignment: 1,
                },
                NativeField {
                    bytes: 8,
                    alignment: 3,
                },
            ] {
                let mut invalid = complete.clone();
                invalid.insert(name.clone(), field);
                assert_eq!(
                    project_nfa(&nfa, &invalid),
                    Err(TargetHeapError::InvalidLayout(name.clone()))
                );
            }
        }
    }

    #[test]
    fn target_arc_request_obeys_target_limit_not_inspecting_host_limit() {
        let nfa = forward_nfa("^[0-9]+$").unwrap();
        let mut narrow = facts(4);
        narrow.insert(
            NFA_INNER.into(),
            NativeField {
                bytes: 0x7fff_fff0,
                alignment: 16,
            },
        );
        assert_eq!(project_nfa(&nfa, &narrow), Err(TargetHeapError::Arithmetic));
        let mut wide = facts(8);
        wide.insert(
            NFA_INNER.into(),
            NativeField {
                bytes: 0x7fff_fff0,
                alignment: 16,
            },
        );
        assert_eq!(project_nfa(&nfa, &wide).unwrap().arc_bytes, 0x8000_0000);
    }

    #[test]
    fn target_named_groups_refuse_unfunded_name_allocations() {
        let info = GroupInfo::new([[None::<&str>, Some("named")]]).unwrap();
        assert_eq!(
            project_group_info(&info, &facts(4)),
            Err(TargetHeapError::NamesUnqualified)
        );
        let nfa = forward_nfa("^(?P<named>[0-9]+)$").unwrap();
        assert_eq!(
            project_nfa(&nfa, &facts(4)),
            Err(TargetHeapError::NamesUnqualified)
        );
    }

    #[test]
    fn target_unknown_or_inconsistent_word_facts_refuse() {
        let nfa = forward_nfa("^[0-9]+$").unwrap();
        for (name, field) in [
            (
                ABI_POINTER,
                NativeField {
                    bytes: 2,
                    alignment: 2,
                },
            ),
            (
                ABI_POINTER,
                NativeField {
                    bytes: 8,
                    alignment: 4,
                },
            ),
            (
                type_name::<AtomicUsize>(),
                NativeField {
                    bytes: 8,
                    alignment: 8,
                },
            ),
        ] {
            let mut invalid = facts(4);
            invalid.insert(name.into(), field);
            assert_eq!(
                project_nfa(&nfa, &invalid),
                Err(TargetHeapError::UnsupportedAbi)
            );
        }
    }

    #[test]
    fn target_unselected_look_or_multiple_patterns_refuse() {
        let word_boundary = forward_nfa(r"^[0-9]+\b$").unwrap();
        assert_eq!(
            project_nfa(&word_boundary, &facts(4)),
            Err(TargetHeapError::UnboundShape)
        );
        let multiple = NFA::compiler().build_many(&["^a$", "^b$"]).unwrap();
        assert_eq!(
            project_nfa(&multiple, &facts(4)),
            Err(TargetHeapError::UnboundShape)
        );
    }
}

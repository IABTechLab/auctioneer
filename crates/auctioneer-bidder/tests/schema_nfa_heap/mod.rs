//! Source-derived retained native NFA trees, not compiler peak or full programs.
//! Pinned Rust 1.98.1 std Global requested bytes, not allocator usable size/RSS.

pub(crate) mod target;

use super::schema_cache_bounds::{forward_nfa, reverse_nfa};
use super::schema_heap_layout::{
    HeapLayout, LayoutError, NativeField, native_arc_layout, private_struct_slot_bound,
};
use super::schema_hybrid_prefix::{PrefixError, candidates, reverse_prefix_nfa};
use regex_automata::nfa::thompson::{NFA, State, Transition};
use regex_automata::util::{
    alphabet::ByteClasses,
    captures::GroupInfo,
    look::{LookMatcher, LookSet},
    primitives::{PatternID, SmallIndex, StateID},
    syntax,
};
use regex_syntax::hir::Look;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum NfaHeapError {
    Layout(LayoutError),
    Arithmetic,
    NamesUnqualified,
    Shape,
    Compilation,
    Prefix(PrefixError),
}

impl From<LayoutError> for NfaHeapError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GroupInfoRetainedHeap {
    pub arc_bytes: usize,
    pub slot_ranges_bytes: usize,
    pub name_maps_bytes: usize,
    pub index_vectors_bytes: usize,
    pub name_entries_bytes: usize,
    pub patterns: usize,
    pub groups: usize,
    pub requested_heap_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NfaRetainedHeap {
    pub arc_bytes: usize,
    pub states_bytes: usize,
    pub start_patterns_bytes: usize,
    pub sparse_bytes: usize,
    pub dense_bytes: usize,
    pub union_bytes: usize,
    pub group_info: GroupInfoRetainedHeap,
    pub states: usize,
    pub requested_heap_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrefixAbsence {
    Anchored,
    NoConcat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrefixTrees {
    Absent(PrefixAbsence),
    Candidates { count: usize, max_tree_bytes: usize },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PatternNfaEnvelope {
    pub forward: NfaRetainedHeap,
    pub reverse: NfaRetainedHeap,
    pub prefix: PrefixTrees,
    pub requested_heap_bytes: usize,
}

#[derive(Clone, Copy)]
struct GroupLiteralInputs<'a> {
    fields: &'a [NativeField],
    slot_range_bytes: usize,
    name_map_bytes: usize,
    index_vector_bytes: usize,
    name_entry_bytes: usize,
    groups: &'a [usize],
    slots: usize,
    names_count: usize,
}

#[derive(Clone, Copy)]
struct NfaLiteralInputs<'a> {
    fields: &'a [NativeField],
    state_bytes: usize,
    id_bytes: usize,
    transition_bytes: usize,
    states: usize,
    patterns: usize,
    sparse_lengths: &'a [usize],
    dense_lengths: &'a [usize],
    union_lengths: &'a [usize],
    group_info: GroupInfoRetainedHeap,
}

impl GroupLiteralInputs<'_> {
    fn project(self, layout: HeapLayout) -> Result<GroupInfoRetainedHeap, NfaHeapError> {
        let groups = owner_sum(self.groups)?;
        let slots = groups.checked_mul(2).ok_or(NfaHeapError::Arithmetic)?;
        if self.groups.contains(&0) || self.slots != slots || self.names_count != groups {
            return Err(NfaHeapError::Shape);
        }
        let arc_bytes = native_arc_layout(private_struct_slot_bound(self.fields)?)?.size();
        let patterns = self.groups.len();
        // captures.rs:1580-1611,2205-2207,2243/2253: three monotonic outer
        // vectors, shrunk at finish; inner vec![None] then push, never shrunk.
        // V's retained envelope deliberately overfunds successful shrinking.
        let slot_ranges_bytes = layout
            .append_vector(self.slot_range_bytes, patterns)?
            .retained_bytes;
        let name_maps_bytes = layout
            .append_vector(self.name_map_bytes, patterns)?
            .retained_bytes;
        let index_vectors_bytes = layout
            .append_vector(self.index_vector_bytes, patterns)?
            .retained_bytes;
        let name_entries_bytes = self.groups.iter().try_fold(0_usize, |sum, length| {
            let bytes = layout
                .append_vector(self.name_entry_bytes, *length)?
                .retained_bytes;
            sum.checked_add(bytes).ok_or(NfaHeapError::Arithmetic)
        })?;
        let requested_heap_bytes = owner_sum(&[
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
}

impl NfaLiteralInputs<'_> {
    fn project(self, layout: HeapLayout) -> Result<NfaRetainedHeap, NfaHeapError> {
        if self.states == 0 || self.dense_lengths.iter().any(|length| *length != 256) {
            return Err(NfaHeapError::Shape);
        }
        let arc_bytes = native_arc_layout(private_struct_slot_bound(self.fields)?)?.size();
        // builder.rs:423-592 constructs a fresh Inner even when the reusable
        // builder has spare capacity; its intermediate removals are NOT Inner
        // removals. nfa.rs:1197-1267,1341-1343,1384: Inner only appends
        // states, never removes any. Final length is its lifetime high-water.
        // Retained V, not length*sizeof(State) or construction growth overlap.
        let states_bytes = layout
            .append_vector(self.state_bytes, self.states)?
            .retained_bytes;
        // nfa.rs:1402: slice.to_vec() exact native request (std slice.rs:425,
        // 448), then shrink_to_fit. Pinned Global shrink returns requested len
        // or diverges on allocation error, not hidden retained excess capacity.
        let start_patterns_bytes = layout.repeat_vector(self.id_bytes, self.patterns)?;
        // builder.rs:464-466,523-524,540-542: retained children are boxed
        // slices, not the builder's arbitrary-capacity transition vectors
        // (pinned std vec/mod.rs:1731 shrinks before boxing).
        let children = |element, lengths: &[usize]| {
            lengths.iter().try_fold(0_usize, |sum, length| {
                let bytes = layout.repeat_vector(element, *length)?;
                sum.checked_add(bytes).ok_or(NfaHeapError::Arithmetic)
            })
        };
        let sparse_bytes = children(self.transition_bytes, self.sparse_lengths)?;
        let dense_bytes = children(self.id_bytes, self.dense_lengths)?;
        let union_bytes = children(self.id_bytes, self.union_lengths)?;
        let requested_heap_bytes = owner_sum(&[
            arc_bytes,
            states_bytes,
            start_patterns_bytes,
            sparse_bytes,
            dense_bytes,
            union_bytes,
            self.group_info.requested_heap_bytes,
        ])?;
        Ok(NfaRetainedHeap {
            arc_bytes,
            states_bytes,
            start_patterns_bytes,
            sparse_bytes,
            dense_bytes,
            union_bytes,
            group_info: self.group_info,
            states: self.states,
            requested_heap_bytes,
        })
    }
}

impl GroupInfoRetainedHeap {
    pub(crate) fn from_info(info: &GroupInfo) -> Result<Self, NfaHeapError> {
        let mut groups = Vec::new();
        for index in 0_usize..info.pattern_len() {
            let pid = PatternID::new(index).map_err(|_error| NfaHeapError::Shape)?;
            groups.push(info.group_len(pid));
        }
        let mut names_count = 0_usize;
        let mut seen = vec![0_usize; info.pattern_len()];
        for (pid, index, name) in info.all_names() {
            // Only this positive check permits omission of hash backing and
            // Arc<str> children: captures.rs:2206,2236-2253 inserts only Some.
            if name.is_some() {
                return Err(NfaHeapError::NamesUnqualified);
            }
            let count = seen.get_mut(pid.as_usize()).ok_or(NfaHeapError::Shape)?;
            if *count != index {
                return Err(NfaHeapError::Shape);
            }
            *count = count.checked_add(1).ok_or(NfaHeapError::Arithmetic)?;
            names_count = names_count.checked_add(1).ok_or(NfaHeapError::Arithmetic)?;
        }
        if seen != groups {
            return Err(NfaHeapError::Shape);
        }
        // captures.rs:2172,2179-2184 (std): actual public constituent layouts,
        // not a repr(Rust) mirror. memory_extra is an inline usize, not a heap
        // authority. The Arc is unconditional, including Q=0/slot_len=0.
        let fields = group_fields();
        GroupLiteralInputs {
            fields: &fields,
            slot_range_bytes: size_of::<(SmallIndex, SmallIndex)>(),
            name_map_bytes: size_of::<HashMap<Arc<str>, SmallIndex>>(),
            index_vector_bytes: size_of::<Vec<Option<Arc<str>>>>(),
            name_entry_bytes: size_of::<Option<Arc<str>>>(),
            groups: &groups,
            slots: info.slot_len(),
            names_count,
        }
        .project(native_layout()?)
    }
}

impl NfaRetainedHeap {
    pub(crate) fn from_nfa(nfa: &NFA) -> Result<Self, NfaHeapError> {
        // Public construction always finalizes a fresh Inner (builder.rs:423);
        // compiler reuse clears only its intermediate builder (compiler:971).
        // No arbitrary Vec capacity is inferred from an unaudited getter.
        let group_info = GroupInfoRetainedHeap::from_info(nfa.group_info())?;
        if group_info.patterns != 0 && group_info.patterns != nfa.pattern_len() {
            return Err(NfaHeapError::Shape);
        }
        let mut sparse_lengths = Vec::new();
        let mut dense_lengths = Vec::new();
        let mut union_lengths = Vec::new();
        for state in nfa.states() {
            let (sparse, dense, union) = state_child(state)?;
            sparse_lengths.push(sparse);
            dense_lengths.extend((dense != 0).then_some(dense));
            union_lengths.push(union);
        }
        let fields = nfa_fields()?;
        NfaLiteralInputs {
            fields: &fields,
            state_bytes: size_of::<State>(),
            id_bytes: size_of::<StateID>(),
            transition_bytes: size_of::<Transition>(),
            states: nfa.states().len(),
            patterns: nfa.pattern_len(),
            sparse_lengths: &sparse_lengths,
            dense_lengths: &dense_lengths,
            union_lengths: &union_lengths,
            group_info,
        }
        .project(native_layout()?)
    }
}

impl PatternNfaEnvelope {
    pub(crate) fn from_pattern(pattern: &str) -> Result<Self, NfaHeapError> {
        let forward = NfaRetainedHeap::from_nfa(
            &forward_nfa(pattern).map_err(|_error| NfaHeapError::Compilation)?,
        )?;
        let reverse = NfaRetainedHeap::from_nfa(
            &reverse_nfa(pattern).map_err(|_error| NfaHeapError::Compilation)?,
        )?;
        let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true))
            .map_err(|_error| NfaHeapError::Prefix(PrefixError::Parse))?;
        // This bounded structural superset is independent of cache graphs and
        // capacity proofs. Inspect even anchored HIRs before excluding them,
        // exactly as for_meta_pattern does (4096 nodes, 128 depth, 64 prefixes).
        let prefixes = candidates(pattern).map_err(NfaHeapError::Prefix)?;
        let prefix = if hir.properties().look_set_prefix().contains(Look::Start) {
            PrefixTrees::Absent(PrefixAbsence::Anchored)
        } else if prefixes.is_empty() {
            PrefixTrees::Absent(PrefixAbsence::NoConcat)
        } else {
            let count = prefixes.len();
            let max_tree_bytes = prefix_tree_maximum(prefixes.into_iter().map(|candidate| {
                let nfa =
                    reverse_prefix_nfa(&candidate).map_err(|_error| NfaHeapError::Compilation)?;
                Ok(NfaRetainedHeap::from_nfa(&nfa)?.requested_heap_bytes)
            }))?;
            PrefixTrees::Candidates {
                count,
                max_tree_bytes,
            }
        };
        let requested_heap_bytes = owner_sum(&[
            forward.requested_heap_bytes,
            reverse.requested_heap_bytes,
            match prefix {
                PrefixTrees::Absent(_) => 0,
                PrefixTrees::Candidates { max_tree_bytes, .. } => max_tree_bytes,
            },
        ])?;
        // A construction upper envelope, not installed-strategy inspection.
        // Core wrappers share the forward tree: Pike/backtrack/onepass/hybrid
        // handles do not own additional trees. Pre's standalone GroupInfo and
        // other strategy allocations are EXCLUDED, never qualified as zero.
        Ok(Self {
            forward,
            reverse,
            prefix,
            requested_heap_bytes,
        })
    }
}

fn prefix_tree_maximum(
    costs: impl IntoIterator<Item = Result<usize, NfaHeapError>>,
) -> Result<usize, NfaHeapError> {
    let mut maximum: Option<usize> = None;
    for candidate in costs {
        let cost = candidate?;
        if cost == 0 {
            return Err(NfaHeapError::Shape);
        }
        maximum = Some(maximum.map_or(cost, |previous| previous.max(cost)));
    }
    maximum.ok_or(NfaHeapError::Shape)
}

fn state_child(state: &State) -> Result<(usize, usize, usize), NfaHeapError> {
    // nfa.rs:1514-1633,1795-1797,1882-1885. State is exhaustive in 0.4.18;
    // a future new variant fails compilation instead of silently costing zero.
    match state {
        State::Sparse(sparse) => Ok((sparse.transitions.len(), 0, 0)),
        State::Dense(dense) => {
            if dense.transitions.len() != 256 {
                return Err(NfaHeapError::Shape);
            }
            Ok((0, dense.transitions.len(), 0))
        }
        State::Union { alternates } => Ok((0, 0, alternates.len())),
        State::ByteRange { .. }
        | State::Look { .. }
        | State::BinaryUnion { .. }
        | State::Capture { .. }
        | State::Fail
        | State::Match { .. } => Ok((0, 0, 0)),
    }
}

fn owner_sum(terms: &[usize]) -> Result<usize, NfaHeapError> {
    terms.iter().try_fold(0_usize, |sum, term| {
        sum.checked_add(*term).ok_or(NfaHeapError::Arithmetic)
    })
}

fn native_layout() -> Result<HeapLayout, NfaHeapError> {
    HeapLayout::new(size_of::<usize>(), 16).map_err(NfaHeapError::Layout)
}

fn group_fields() -> [NativeField; 4] {
    [
        NativeField::of::<Vec<(SmallIndex, SmallIndex)>>(),
        NativeField::of::<Vec<HashMap<Arc<str>, SmallIndex>>>(),
        NativeField::of::<Vec<Vec<Option<Arc<str>>>>>(),
        NativeField::of::<usize>(),
    ]
}

fn nfa_fields() -> Result<[NativeField; 15], NfaHeapError> {
    // alphabet.rs:685,742-749: ByteClassSet(ByteSet { bits: BitSet([u128;2]) }).
    // Source-backed wrapper slot bound, no heap and no private ABI mirror.
    let bit_set = private_struct_slot_bound(&[NativeField::of::<[u128; 2]>()])?;
    let byte_set = private_struct_slot_bound(&[bit_set])?;
    let byte_class_set = private_struct_slot_bound(&[byte_set])?;
    // ByteClasses([u8;256]) (alphabet:215), LookMatcher's DebugByte (look:577),
    // LookSets, flags and memory_extra are inline; none authorizes extra heap.
    Ok([
        NativeField::of::<Vec<State>>(),
        NativeField::of::<StateID>(),
        NativeField::of::<StateID>(),
        NativeField::of::<Vec<StateID>>(),
        NativeField::of::<GroupInfo>(),
        byte_class_set,
        NativeField::of::<ByteClasses>(),
        NativeField::of::<bool>(),
        NativeField::of::<bool>(),
        NativeField::of::<bool>(),
        NativeField::of::<bool>(),
        NativeField::of::<LookMatcher>(),
        NativeField::of::<LookSet>(),
        NativeField::of::<LookSet>(),
        NativeField::of::<usize>(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};
    use regex_automata::nfa::thompson::{DenseTransitions, SparseTransitions, Transition};
    use regex_automata::util::primitives::StateID;

    const FIELDS: &[NativeField] = &[
        NativeField {
            bytes: 24,
            alignment: 8,
        },
        NativeField {
            bytes: 4,
            alignment: 4,
        },
    ];
    const NFA_FIELDS: &[NativeField] = &[NativeField {
        bytes: 64,
        alignment: 8,
    }];
    const GROUP: GroupLiteralInputs<'static> = GroupLiteralInputs {
        fields: FIELDS,
        slot_range_bytes: 8,
        name_map_bytes: 48,
        index_vector_bytes: 24,
        name_entry_bytes: 16,
        groups: &[1, 5],
        slots: 12,
        names_count: 6,
    };

    fn layout() -> HeapLayout {
        HeapLayout::new(size_of::<usize>(), 16).unwrap()
    }

    #[test]
    fn nfa_heap_literal_group_owners_are_independently_unequal() {
        let group = GROUP.project(layout()).unwrap();
        assert_eq!(
            group,
            GroupInfoRetainedHeap {
                arc_bytes: 48,
                slot_ranges_bytes: 32,
                name_maps_bytes: 192,
                index_vectors_bytes: 96,
                name_entries_bytes: 224,
                patterns: 2,
                groups: 6,
                requested_heap_bytes: 592,
            }
        );
        let empty = GroupLiteralInputs {
            groups: &[],
            slots: 0,
            names_count: 0,
            ..GROUP
        }
        .project(layout())
        .unwrap();
        assert_eq!(empty.requested_heap_bytes, 48);
        assert_eq!(empty.arc_bytes, 48);
    }

    #[test]
    fn nfa_heap_literal_nfa_owners_charge_children_and_group_once() {
        let input = NfaLiteralInputs {
            fields: NFA_FIELDS,
            state_bytes: 24,
            id_bytes: 4,
            transition_bytes: 12,
            states: 7,
            patterns: 3,
            sparse_lengths: &[3, 5],
            dense_lengths: &[256],
            union_lengths: &[3, 4],
            group_info: GROUP.project(layout()).unwrap(),
        };
        let tree = input.project(layout()).unwrap();
        assert_eq!(tree.arc_bytes, 80);
        assert_eq!(tree.states_bytes, 336);
        assert_eq!(tree.start_patterns_bytes, 12);
        assert_eq!(tree.sparse_bytes, 96);
        assert_eq!(tree.dense_bytes, 1024);
        assert_eq!(tree.union_bytes, 28);
        assert_eq!(tree.group_info.requested_heap_bytes, 592);
        assert_eq!(tree.requested_heap_bytes, 2168);
        assert_eq!(
            NfaLiteralInputs { states: 8, ..input }
                .project(layout())
                .unwrap()
                .requested_heap_bytes,
            2216
        );
        assert_eq!(
            NfaLiteralInputs {
                patterns: 4,
                ..input
            }
            .project(layout())
            .unwrap()
            .requested_heap_bytes,
            2172
        );
        assert_eq!(
            NfaLiteralInputs {
                sparse_lengths: &[4, 5],
                ..input
            }
            .project(layout())
            .unwrap()
            .requested_heap_bytes,
            2180
        );
        assert_eq!(
            NfaLiteralInputs {
                union_lengths: &[4, 4],
                ..input
            }
            .project(layout())
            .unwrap()
            .requested_heap_bytes,
            2172
        );
        assert_eq!(
            NfaLiteralInputs {
                dense_lengths: &[],
                ..input
            }
            .project(layout())
            .unwrap()
            .requested_heap_bytes,
            1144
        );
    }

    #[test]
    fn nfa_heap_literal_root_and_element_layouts_affect_their_own_terms() {
        let input = NfaLiteralInputs {
            fields: NFA_FIELDS,
            state_bytes: 24,
            id_bytes: 4,
            transition_bytes: 12,
            states: 7,
            patterns: 3,
            sparse_lengths: &[3, 5],
            dense_lengths: &[256],
            union_lengths: &[3, 4],
            group_info: GROUP.project(layout()).unwrap(),
        };
        let larger_root = NfaLiteralInputs {
            fields: &[NativeField {
                bytes: 72,
                alignment: 8,
            }],
            ..input
        }
        .project(layout())
        .unwrap();
        assert_eq!(larger_root.arc_bytes, 88);
        assert_eq!(larger_root.states_bytes, 336);
        assert_eq!(larger_root.group_info.requested_heap_bytes, 592);
        assert_eq!(larger_root.requested_heap_bytes, 2176);
        assert_eq!(
            NfaLiteralInputs {
                state_bytes: 32,
                ..input
            }
            .project(layout())
            .unwrap()
            .states_bytes,
            448
        );
        let wider_ids = NfaLiteralInputs {
            id_bytes: 8,
            ..input
        }
        .project(layout())
        .unwrap();
        assert_eq!(wider_ids.start_patterns_bytes, 24);
        assert_eq!(wider_ids.dense_bytes, 2048);
        assert_eq!(wider_ids.union_bytes, 56);
        assert_eq!(wider_ids.sparse_bytes, 96);
        assert_eq!(
            NfaLiteralInputs {
                transition_bytes: 16,
                ..input
            }
            .project(layout())
            .unwrap()
            .sparse_bytes,
            128
        );
    }

    #[test]
    fn nfa_heap_literal_each_group_layout_and_inner_history_is_independent() {
        for (input, expected) in [
            (
                GroupLiteralInputs {
                    fields: &[NativeField {
                        bytes: 40,
                        alignment: 8,
                    }],
                    ..GROUP
                },
                600_usize,
            ),
            (
                GroupLiteralInputs {
                    slot_range_bytes: 12,
                    ..GROUP
                },
                608,
            ),
            (
                GroupLiteralInputs {
                    name_map_bytes: 56,
                    ..GROUP
                },
                624,
            ),
            (
                GroupLiteralInputs {
                    index_vector_bytes: 32,
                    ..GROUP
                },
                624,
            ),
            (
                GroupLiteralInputs {
                    name_entry_bytes: 24,
                    ..GROUP
                },
                704,
            ),
            (
                GroupLiteralInputs {
                    groups: &[2, 5],
                    slots: 14,
                    names_count: 7,
                    ..GROUP
                },
                592,
            ),
            (
                GroupLiteralInputs {
                    groups: &[1, 6],
                    slots: 14,
                    names_count: 7,
                    ..GROUP
                },
                624,
            ),
        ] {
            assert_eq!(
                input.project(layout()).unwrap().requested_heap_bytes,
                expected
            );
        }
    }

    #[test]
    fn nfa_heap_shape_and_checked_group_arithmetic_refuse() {
        for input in [
            GroupLiteralInputs {
                groups: &[0, 6],
                ..GROUP
            },
            GroupLiteralInputs { slots: 10, ..GROUP },
            GroupLiteralInputs {
                names_count: 5,
                ..GROUP
            },
        ] {
            assert_eq!(input.project(layout()), Err(NfaHeapError::Shape));
        }
        for input in [
            GroupLiteralInputs {
                groups: &[usize::MAX, 1],
                ..GROUP
            },
            GroupLiteralInputs {
                groups: &[usize::MAX],
                ..GROUP
            },
        ] {
            assert_eq!(input.project(layout()), Err(NfaHeapError::Arithmetic));
        }
        assert_eq!(owner_sum(&[usize::MAX, 1]), Err(NfaHeapError::Arithmetic));
        assert_eq!(owner_sum(&[17, 31, 43]), Ok(91));
    }

    #[test]
    fn nfa_heap_invalid_layouts_and_each_request_overflow_refuse() {
        for input in [
            GroupLiteralInputs {
                fields: &[],
                ..GROUP
            },
            GroupLiteralInputs {
                fields: &[NativeField {
                    bytes: 7,
                    alignment: 8,
                }],
                ..GROUP
            },
        ] {
            assert_eq!(
                input.project(layout()),
                Err(NfaHeapError::Layout(LayoutError::Shape))
            );
        }
        for input in [
            GroupLiteralInputs {
                slot_range_bytes: usize::MAX,
                ..GROUP
            },
            GroupLiteralInputs {
                name_map_bytes: usize::MAX,
                ..GROUP
            },
            GroupLiteralInputs {
                index_vector_bytes: usize::MAX,
                ..GROUP
            },
            GroupLiteralInputs {
                name_entry_bytes: usize::MAX,
                ..GROUP
            },
            GroupLiteralInputs {
                fields: &[NativeField {
                    bytes: usize::MAX,
                    alignment: 1,
                }],
                ..GROUP
            },
        ] {
            assert_eq!(
                input.project(layout()),
                Err(NfaHeapError::Layout(LayoutError::Arithmetic))
            );
        }
    }

    #[test]
    fn nfa_heap_owner_sum_overflow_does_not_require_an_invalid_allocation() {
        let maximum = usize::try_from(isize::MAX).unwrap();
        let element = maximum.checked_div(4).unwrap();
        layout().append_vector(element, 2).unwrap();
        assert_eq!(
            GroupLiteralInputs {
                slot_range_bytes: element,
                name_map_bytes: element,
                index_vector_bytes: element,
                ..GROUP
            }
            .project(layout()),
            Err(NfaHeapError::Arithmetic)
        );
        let input = NfaLiteralInputs {
            fields: FIELDS,
            state_bytes: 24,
            id_bytes: 4,
            transition_bytes: 12,
            states: 7,
            patterns: 3,
            sparse_lengths: &[3],
            dense_lengths: &[256],
            union_lengths: &[5],
            group_info: GroupInfoRetainedHeap {
                requested_heap_bytes: usize::MAX,
                ..GROUP.project(layout()).unwrap()
            },
        };
        assert_eq!(input.project(layout()), Err(NfaHeapError::Arithmetic));
    }

    #[test]
    fn nfa_heap_nfa_shape_layout_and_request_errors_are_independent() {
        let input = NfaLiteralInputs {
            fields: FIELDS,
            state_bytes: 24,
            id_bytes: 4,
            transition_bytes: 12,
            states: 7,
            patterns: 3,
            sparse_lengths: &[3],
            dense_lengths: &[256],
            union_lengths: &[5],
            group_info: GROUP.project(layout()).unwrap(),
        };
        for invalid in [
            NfaLiteralInputs { states: 0, ..input },
            NfaLiteralInputs {
                dense_lengths: &[255],
                ..input
            },
        ] {
            assert_eq!(invalid.project(layout()), Err(NfaHeapError::Shape));
        }
        for invalid in [
            NfaLiteralInputs {
                state_bytes: 0,
                ..input
            },
            NfaLiteralInputs {
                id_bytes: 0,
                ..input
            },
            NfaLiteralInputs {
                transition_bytes: 0,
                ..input
            },
            NfaLiteralInputs {
                fields: &[],
                ..input
            },
        ] {
            assert_eq!(
                invalid.project(layout()),
                Err(NfaHeapError::Layout(LayoutError::Shape))
            );
        }
        for invalid in [
            NfaLiteralInputs {
                states: usize::MAX,
                ..input
            },
            NfaLiteralInputs {
                patterns: usize::MAX,
                ..input
            },
            NfaLiteralInputs {
                state_bytes: usize::MAX,
                ..input
            },
            NfaLiteralInputs {
                id_bytes: usize::MAX,
                ..input
            },
            NfaLiteralInputs {
                sparse_lengths: &[usize::MAX],
                ..input
            },
            NfaLiteralInputs {
                union_lengths: &[usize::MAX],
                ..input
            },
            NfaLiteralInputs {
                fields: &[NativeField {
                    bytes: usize::MAX,
                    alignment: 1,
                }],
                ..input
            },
        ] {
            assert_eq!(
                invalid.project(layout()),
                Err(NfaHeapError::Layout(LayoutError::Arithmetic))
            );
        }
    }

    #[test]
    fn nfa_heap_native_field_inventory_uses_public_constituents_not_private_mirrors() {
        assert_eq!(size_of::<usize>(), 8);
        assert_eq!(size_of::<SmallIndex>(), 4);
        assert_eq!(size_of::<StateID>(), 4);
        assert_eq!(size_of::<(SmallIndex, SmallIndex)>(), 8);
        assert_eq!(size_of::<Option<Arc<str>>>(), 16);
        assert_eq!(
            group_fields(),
            [
                NativeField::of::<Vec<(SmallIndex, SmallIndex)>>(),
                NativeField::of::<Vec<HashMap<Arc<str>, SmallIndex>>>(),
                NativeField::of::<Vec<Vec<Option<Arc<str>>>>>(),
                NativeField::of::<usize>(),
            ]
        );
        let fields = nfa_fields().unwrap();
        assert_eq!(fields.len(), 15);
        assert_eq!(fields[0], NativeField::of::<Vec<State>>());
        assert_eq!(fields[3], NativeField::of::<Vec<StateID>>());
        assert_eq!(fields[4], NativeField::of::<GroupInfo>());
        assert_eq!(fields[5], NativeField::of::<[u128; 2]>());
        assert_eq!(fields[6], NativeField::of::<ByteClasses>());
        assert_eq!(fields[11], NativeField::of::<LookMatcher>());
        assert_eq!(fields[12], NativeField::of::<LookSet>());
        assert_eq!(fields[13], NativeField::of::<LookSet>());
        assert_eq!(fields[14], NativeField::of::<usize>());
        assert_eq!(
            native_arc_layout(private_struct_slot_bound(&group_fields()).unwrap())
                .unwrap()
                .size(),
            96
        );
        assert_eq!(
            native_arc_layout(private_struct_slot_bound(&fields).unwrap())
                .unwrap()
                .size(),
            544
        );
    }

    #[test]
    fn nfa_heap_public_state_variants_charge_only_boxed_children() {
        let transition = Transition {
            start: b'a',
            end: b'z',
            next: StateID::ZERO,
        };
        assert_eq!(
            state_child(&State::Sparse(SparseTransitions {
                transitions: vec![transition; 3].into_boxed_slice()
            })),
            Ok((3, 0, 0))
        );
        assert_eq!(
            state_child(&State::Dense(DenseTransitions {
                transitions: vec![StateID::ZERO; 256].into_boxed_slice()
            })),
            Ok((0, 256, 0))
        );
        assert_eq!(
            state_child(&State::Union {
                alternates: vec![StateID::ZERO; 5].into_boxed_slice()
            }),
            Ok((0, 0, 5))
        );
        for state in [
            State::Fail,
            State::ByteRange { trans: transition },
            State::BinaryUnion {
                alt1: StateID::ZERO,
                alt2: StateID::ZERO,
            },
        ] {
            assert_eq!(state_child(&state), Ok((0, 0, 0)));
        }
        assert_eq!(
            state_child(&State::Dense(DenseTransitions {
                transitions: vec![StateID::ZERO; 255].into_boxed_slice()
            })),
            Err(NfaHeapError::Shape)
        );
    }

    #[test]
    fn nfa_heap_real_named_captures_refuse_and_unnamed_nested_are_valid() {
        let named = forward_nfa("(?P<fake_name>a)").unwrap();
        assert_eq!(
            GroupInfoRetainedHeap::from_info(named.group_info()),
            Err(NfaHeapError::NamesUnqualified)
        );
        assert_eq!(
            NfaRetainedHeap::from_nfa(&named),
            Err(NfaHeapError::NamesUnqualified)
        );
        assert_eq!(
            PatternNfaEnvelope::from_pattern("(?P<fake_name>a)"),
            Err(NfaHeapError::NamesUnqualified)
        );
        let nested = forward_nfa("((a)(b))").unwrap();
        let tree = NfaRetainedHeap::from_nfa(&nested).unwrap();
        assert_eq!(tree.group_info.groups, 4);
        assert_eq!(tree.group_info.patterns, 1);
        assert!(tree.group_info.name_entries_bytes >= 128_usize);
        let reverse = reverse_nfa("((a)(b))").unwrap();
        let capture_free = NfaRetainedHeap::from_nfa(&reverse).unwrap();
        assert_eq!(capture_free.group_info.groups, 0);
        assert_eq!(capture_free.group_info.patterns, 0);
        assert!(capture_free.group_info.arc_bytes > 0_usize);
        assert_eq!(
            capture_free.group_info.requested_heap_bytes,
            capture_free.group_info.arc_bytes
        );
    }

    #[test]
    fn nfa_heap_pattern_prefix_absence_is_typed_and_failures_are_not_zero() {
        assert_eq!(
            PatternNfaEnvelope::from_pattern("^a+z$").unwrap().prefix,
            PrefixTrees::Absent(PrefixAbsence::Anchored)
        );
        assert_eq!(
            PatternNfaEnvelope::from_pattern("literal").unwrap().prefix,
            PrefixTrees::Absent(PrefixAbsence::NoConcat)
        );
        assert!(matches!(
            PatternNfaEnvelope::from_pattern("["),
            Err(NfaHeapError::Compilation)
        ));
        let long = "a+".repeat(66);
        assert_eq!(
            PatternNfaEnvelope::from_pattern(&long),
            Err(NfaHeapError::Prefix(PrefixError::PrefixLimit))
        );
    }

    #[test]
    fn nfa_heap_prefix_cost_reducer_interior_maximum_is_order_independent() {
        for costs in [
            [17_usize, 101, 43],
            [17, 43, 101],
            [101, 17, 43],
            [101, 43, 17],
            [43, 17, 101],
            [43, 101, 17],
        ] {
            assert_eq!(prefix_tree_maximum(costs.map(Ok)), Ok(101), "{costs:?}");
        }
        assert_eq!(prefix_tree_maximum([Ok(usize::MAX)]), Ok(usize::MAX));
    }

    #[test]
    fn nfa_heap_prefix_cost_reducer_refuses_unproved_candidates_and_preserves_errors() {
        assert_eq!(prefix_tree_maximum([]), Err(NfaHeapError::Shape));
        for costs in [vec![0_usize], vec![17, 0], vec![0, 43]] {
            assert_eq!(
                prefix_tree_maximum(costs.into_iter().map(Ok)),
                Err(NfaHeapError::Shape)
            );
        }
        assert_eq!(
            prefix_tree_maximum([Ok(17), Err(NfaHeapError::Compilation), Ok(101)]),
            Err(NfaHeapError::Compilation)
        );
        assert_eq!(
            prefix_tree_maximum([Ok(101), Err(NfaHeapError::Arithmetic)]),
            Err(NfaHeapError::Arithmetic)
        );
        assert_eq!(
            prefix_tree_maximum([
                Err(NfaHeapError::Prefix(PrefixError::TraversalLimit)),
                Ok(43),
            ]),
            Err(NfaHeapError::Prefix(PrefixError::TraversalLimit))
        );
    }

    #[test]
    fn nfa_heap_prefix_program_maximum_is_not_the_sum_or_cache_envelope() {
        use crate::schema_hybrid_prefix::{candidates, reverse_prefix_nfa};
        let pattern = "a+b+c+z";
        let bound = PatternNfaEnvelope::from_pattern(pattern).unwrap();
        let sizes: Vec<usize> = candidates(pattern)
            .unwrap()
            .iter()
            .map(|prefix| {
                NfaRetainedHeap::from_nfa(&reverse_prefix_nfa(prefix).unwrap())
                    .unwrap()
                    .requested_heap_bytes
            })
            .collect();
        assert_eq!(sizes.len(), 3);
        let maximum = sizes.iter().copied().max().unwrap();
        assert!(sizes.iter().sum::<usize>() > maximum);
        assert_eq!(
            bound.prefix,
            PrefixTrees::Candidates {
                count: 3,
                max_tree_bytes: maximum
            }
        );
        assert_eq!(
            bound.requested_heap_bytes,
            bound
                .forward
                .requested_heap_bytes
                .checked_add(bound.reverse.requested_heap_bytes)
                .unwrap()
                .checked_add(maximum)
                .unwrap()
        );
    }
}

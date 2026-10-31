//! Conditional native requested-heap and source-visit bounds for the selected
//! HIR-to-Thompson paths. Excludes HIR production, meta/one-pass construction,
//! machine stack frames, allocator overhead and guest/profile correspondence.

use super::schema_heap_layout::{HeapLayout, LayoutError, NativeField, private_struct_slot_bound};
use super::schema_nfa_heap::NfaRetainedHeap;
use regex_automata::{
    nfa::thompson::{self, Builder, NFA, State, Transition, WhichCaptures},
    util::{
        look::LookMatcher,
        primitives::{SmallIndex, StateID},
        syntax,
    },
};
use regex_syntax::hir::{Class, ClassBytesRange, Hir, HirKind, Look, Repetition};
use std::{
    alloc::Layout,
    any::type_name,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ConstructionCounts {
    pub states: usize,
    pub empties: usize,
    pub sparse_states: usize,
    pub sparse_transitions: usize,
    pub unions: usize,
    pub union_alternates: usize,
    pub captures: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ConstructionError {
    Arithmetic,
    Layout,
    Scope,
    Compilation,
    Shape,
    UnboundOwner(&'static str),
}

impl From<LayoutError> for ConstructionError {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Arithmetic => Self::Arithmetic,
            LayoutError::Shape => Self::Layout,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NfaConstructionCertificate {
    pub counts: ConstructionCounts,
    pub state_slot_bytes: usize,
    pub trie_initial_bytes: usize,
    pub intermediate_peak_bytes: usize,
    pub conversion_scratch_bytes: usize,
    pub initialization_scratch_bytes: usize,
    pub final_nfa_bytes: usize,
    pub failure_prefix_peak_bytes: usize,
    pub peak_requested_bytes: usize,
    /// Modeled source-level byte/record visits, not instructions, elapsed time
    /// or an enforced engine progress counter.
    pub work_upper_bound: usize,
}

impl ConstructionCounts {
    fn checked_add(self, other: Self) -> Result<Self, ConstructionError> {
        Ok(Self {
            states: add(self.states, other.states)?,
            empties: add(self.empties, other.empties)?,
            sparse_states: add(self.sparse_states, other.sparse_states)?,
            sparse_transitions: add(self.sparse_transitions, other.sparse_transitions)?,
            unions: add(self.unions, other.unions)?,
            union_alternates: add(self.union_alternates, other.union_alternates)?,
            captures: add(self.captures, other.captures)?,
        })
    }

    fn checked_repeat(self, times: usize) -> Result<Self, ConstructionError> {
        Ok(Self {
            states: mul(self.states, times)?,
            empties: mul(self.empties, times)?,
            sparse_states: mul(self.sparse_states, times)?,
            sparse_transitions: mul(self.sparse_transitions, times)?,
            unions: mul(self.unions, times)?,
            union_alternates: mul(self.union_alternates, times)?,
            captures: mul(self.captures, times)?,
        })
    }
}

/// Same natural-layout premise as the existing private-struct heap models.
/// range_trie.rs:203-206 has one Vec field and no repr/align modifier. This
/// bounds the payload; it does not assert an exact private ABI or use a peak.
pub(crate) fn source_trie_state_layout() -> Result<Layout, ConstructionError> {
    let field = private_struct_slot_bound(&[NativeField::of::<Vec<()>>()])?;
    Layout::from_size_align(field.bytes, field.alignment)
        .map_err(|_layout_error| ConstructionError::Layout)
}

pub(crate) const BUILDER_STATE: &str = "regex_automata::nfa::thompson::builder::State";
pub(crate) const TRIE_STATE: &str = "regex_automata::nfa::thompson::range_trie::State";
pub(crate) const HIR_PROPERTIES: &str = "regex_syntax::hir::PropertiesI";

pub(crate) fn additional_layout_keys() -> [&'static str; 6] {
    [
        BUILDER_STATE,
        TRIE_STATE,
        type_name::<(StateID, StateID)>(),
        type_name::<bool>(),
        type_name::<ClassBytesRange>(),
        HIR_PROPERTIES,
    ]
}

#[derive(Clone, Copy)]
struct ConstructionStorage {
    heap: HeapLayout,
    state_slot: usize,
    trie: usize,
    state: usize,
    id: usize,
    id_pair: usize,
    transition: usize,
    group_pair: usize,
    name_map: usize,
    name_vector: usize,
    name_slot: usize,
    boolean: usize,
    class_range: usize,
    properties: usize,
}

impl ConstructionStorage {
    fn target(facts: &BTreeMap<String, NativeField>) -> Result<Self, ConstructionError> {
        use super::schema_nfa_heap::target::{ABI_POINTER, fact_keys};
        use std::sync::atomic::AtomicUsize;
        for name in fact_keys().into_iter().chain(additional_layout_keys()) {
            facts
                .get(name)
                .ok_or(ConstructionError::Layout)?
                .validate()?;
        }
        let word = facts[ABI_POINTER];
        if !matches!(word.bytes, 4 | 8)
            || word.alignment != word.bytes
            || facts[type_name::<AtomicUsize>()] != word
        {
            return Err(ConstructionError::Layout);
        }
        let heap = HeapLayout::new(word.bytes, word.bytes)?;
        for name in fact_keys().into_iter().chain(additional_layout_keys()) {
            heap.repeat_vector(facts[name].bytes, 1)?;
        }
        Ok(Self {
            heap,
            state_slot: facts[BUILDER_STATE].bytes,
            trie: facts[TRIE_STATE].bytes,
            state: facts[type_name::<State>()].bytes,
            id: facts[type_name::<StateID>()].bytes,
            id_pair: facts[type_name::<(StateID, StateID)>()].bytes,
            transition: facts[type_name::<Transition>()].bytes,
            group_pair: facts[type_name::<(SmallIndex, SmallIndex)>()].bytes,
            name_map: facts[type_name::<HashMap<Arc<str>, SmallIndex>>()].bytes,
            name_vector: facts[type_name::<Vec<Option<Arc<str>>>>()].bytes,
            name_slot: facts[type_name::<Option<Arc<str>>>()].bytes,
            boolean: facts[type_name::<bool>()].bytes,
            class_range: facts[type_name::<ClassBytesRange>()].bytes,
            properties: facts[HIR_PROPERTIES].bytes,
        })
    }
}

impl NfaConstructionCertificate {
    pub(crate) fn from_hir_with_layouts(
        hir: &Hir,
        reverse: bool,
        facts: &BTreeMap<String, NativeField>,
    ) -> Result<Self, ConstructionError> {
        use super::schema_nfa_heap::target::{TargetHeapError, project_nfa};
        let storage = ConstructionStorage::target(facts)?;
        Self::project(hir, reverse, &storage, |nfa| {
            project_nfa(nfa, facts).map_err(|error| match error {
                TargetHeapError::Arithmetic => ConstructionError::Arithmetic,
                TargetHeapError::MissingType(_)
                | TargetHeapError::InvalidLayout(_)
                | TargetHeapError::UnsupportedAbi => ConstructionError::Layout,
                TargetHeapError::NamesUnqualified | TargetHeapError::UnboundShape => {
                    ConstructionError::Shape
                }
            })
        })
    }

    pub(crate) fn from_hir(
        hir: &Hir,
        reverse: bool,
        trie_state: Layout,
    ) -> Result<Self, ConstructionError> {
        if trie_state.size() < size_of::<Vec<()>>()
            || trie_state.align() < align_of::<Vec<()>>()
            || !trie_state.size().is_multiple_of(trie_state.align())
        {
            return Err(ConstructionError::Layout);
        }
        let mut witness = Builder::new();
        witness
            .add_empty()
            .map_err(|_build_error| ConstructionError::Compilation)?;
        let storage = ConstructionStorage {
            heap: HeapLayout::new(size_of::<usize>(), size_of::<usize>())?,
            state_slot: witness.memory_usage(),
            trie: trie_state.size(),
            state: size_of::<State>(),
            id: size_of::<StateID>(),
            id_pair: size_of::<(StateID, StateID)>(),
            transition: size_of::<Transition>(),
            group_pair: size_of::<(SmallIndex, SmallIndex)>(),
            name_map: size_of::<HashMap<Arc<str>, SmallIndex>>(),
            name_vector: size_of::<Vec<Option<Arc<str>>>>(),
            name_slot: size_of::<Option<Arc<str>>>(),
            boolean: size_of::<bool>(),
            class_range: size_of::<ClassBytesRange>(),
            properties: hir.properties().memory_usage(),
        };
        if storage.state_slot == 0 {
            return Err(ConstructionError::Layout);
        }
        Self::project(hir, reverse, &storage, |nfa| {
            NfaRetainedHeap::from_nfa(nfa).map_err(|_error| ConstructionError::Shape)
        })
    }

    fn project<F>(
        hir: &Hir,
        reverse: bool,
        storage: &ConstructionStorage,
        retained_heap: F,
    ) -> Result<Self, ConstructionError>
    where
        F: FnOnce(&NFA) -> Result<NfaRetainedHeap, ConstructionError>,
    {
        let (model, anchored) = Model::selected(hir, reverse)?;
        let counts = model.counts;
        let logical = sum(&[
            mul(counts.states, storage.state_slot)?,
            mul(counts.sparse_transitions, storage.transition)?,
            mul(counts.union_alternates, storage.id)?,
        ])?;
        if logical > 0x0001_0000 {
            return Err(ConstructionError::Scope);
        }
        let layout = storage.heap;
        let trie_initial_bytes = append_peak(layout, storage.trie, 2)?;
        let synthetic_hir = if anchored {
            0
        } else {
            add(
                layout.fresh_resize_vector(storage.class_range, 1)?,
                storage.properties,
            )?
        };
        let intermediate_peak_bytes =
            model.intermediate_peak(storage, trie_initial_bytes, synthetic_hir)?;

        // Compile the same fixed configuration only after the source expansion
        // and request arithmetic have passed. Public NFA structure supplies
        // retained-tree inputs, never allocator observations or peak rounding.
        let nfa = compile_nfa(hir, reverse)?;
        let final_states = counts
            .states
            .checked_sub(counts.empties)
            .ok_or(ConstructionError::Shape)?;
        if nfa.states().len() != final_states
            || nfa.group_info().slot_len() != mul(model.groups, 2)?
        {
            return Err(ConstructionError::Shape);
        }
        let retained = retained_heap(&nfa)?;
        // Builder::build owns remap/empties/remapped while into_nfa owns its
        // closure stack and two SparseSet ID arrays. Duplicated stack entries
        // are bounded by all possible pushes, not just distinct states.
        let closure_pushes = sum(&[1, final_states, counts.union_alternates])?;
        let conversion_scratch_bytes =
            model.conversion_scratch(storage, final_states, closure_pushes, &retained)?;
        let initialization_scratch_bytes = add(intermediate_peak_bytes, conversion_scratch_bytes)?;
        let peak_requested_bytes =
            add(initialization_scratch_bytes, retained.requested_heap_bytes)?;
        // A failed add/patch occurs after mutation and size checking. The full
        // monotonic source high-waters cover that mutated prefix and dropping
        // its buffers. Selected unnamed scalar/ID children need no drop heap.
        let failure_prefix_peak_bytes = peak_requested_bytes;
        let work_upper_bound = sum(&[
            model.calls,
            model.patches,
            counts.states,
            counts.sparse_transitions,
            counts.union_alternates,
            model.groups,
            mul(counts.states, 3)?,
            mul(mul(counts.empties, counts.states)?, 2)?,
            final_states,
            counts.sparse_transitions,
            counts.union_alternates,
            closure_pushes,
            mul(
                add(counts.states, counts.sparse_transitions)?,
                usize::from(u8::MAX) + 1,
            )?,
            // Geometric growth copies and final drops visit at most twice
            // each capacity envelope in bytes (each record has nonzero size).
            mul(peak_requested_bytes, 2)?,
        ])?;
        Ok(Self {
            counts,
            state_slot_bytes: storage.state_slot,
            trie_initial_bytes,
            intermediate_peak_bytes,
            conversion_scratch_bytes,
            initialization_scratch_bytes,
            final_nfa_bytes: retained.requested_heap_bytes,
            failure_prefix_peak_bytes,
            peak_requested_bytes,
            work_upper_bound,
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Model {
    counts: ConstructionCounts,
    calls: usize,
    patches: usize,
    groups: usize,
}

impl Model {
    fn selected(hir: &Hir, reverse: bool) -> Result<(Self, bool), ConstructionError> {
        // This is the selected grammar, not an arbitrary compiler backend.
        if !hir.properties().look_set_suffix().contains(Look::End) {
            return Err(ConstructionError::Scope);
        }
        let body = Self::hir(hir, reverse, 0)?.capture(0, reverse)?;
        let anchored = if reverse {
            hir.properties().look_set_suffix().contains(Look::End)
        } else {
            hir.properties().look_set_prefix().contains(Look::Start)
        };
        let prefix = if anchored {
            Self::empty()
        } else {
            // c_at_least(Hir::dot(AnyByte), false, 0): one byte-class
            // range plus a looping union; no Unicode UTF-8 compiler is used.
            let mut prefix = Self::class(1).plus(Self::union(2))?.patch(2)?;
            prefix.calls = 1;
            prefix
        };
        let model = prefix.plus(body)?.plus(Self::state())?.patch(2)?;
        // Parsed selected HIR has contiguous capture indices. Do not permit
        // a hand-built sparse index to allocate a huge placeholder name vector.
        if model.groups
            > model
                .counts
                .captures
                .checked_div(2)
                .ok_or(ConstructionError::Arithmetic)?
        {
            return Err(ConstructionError::Scope);
        }
        Ok((model, anchored))
    }

    fn intermediate_peak(
        self,
        storage: &ConstructionStorage,
        trie_initial_bytes: usize,
        synthetic_hir: usize,
    ) -> Result<usize, ConstructionError> {
        // Every intermediate state survives until Builder::build returns.
        // Sparse vectors use with_capacity(ranges.len()); union/name vectors
        // append. Summed per-vector growth overlap is a conservative envelope.
        let layout = storage.heap;
        let counts = self.counts;
        let union_entries = add(mul(counts.union_alternates, 2)?, mul(counts.unions, 4)?)?;
        let union_bytes = sum(&[
            layout.repeat_vector(storage.id, union_entries)?,
            layout.repeat_vector(storage.id, counts.union_alternates)?,
        ])?;
        let capture_bytes = if self.groups == 0 {
            0
        } else {
            add(
                append_peak(layout, storage.name_vector, 1)?,
                append_peak(layout, storage.name_slot, self.groups)?,
            )?
        };
        sum(&[
            append_peak(layout, storage.state_slot, counts.states)?,
            layout.repeat_vector(storage.transition, counts.sparse_transitions)?,
            union_bytes,
            append_peak(layout, storage.id, 1)?,
            capture_bytes,
            trie_initial_bytes,
            synthetic_hir,
        ])
    }

    fn conversion_scratch(
        self,
        storage: &ConstructionStorage,
        final_states: usize,
        closure_pushes: usize,
        retained: &NfaRetainedHeap,
    ) -> Result<usize, ConstructionError> {
        let layout = storage.heap;
        let counts = self.counts;
        let state_growth = layout.append_vector(storage.state, final_states)?;
        let group_shrink = mul(
            retained.group_info.patterns,
            sum(&[storage.group_pair, storage.name_map, storage.name_vector])?,
        )?;
        sum(&[
            layout.fresh_resize_vector(storage.id, counts.states)?,
            append_peak(layout, storage.id_pair, counts.empties)?,
            layout.repeat_vector(storage.boolean, counts.states)?,
            append_peak(layout, storage.id, closure_pushes)?,
            mul(layout.fresh_resize_vector(storage.id, final_states)?, 2)?,
            state_growth.old_growth_bytes,
            layout.repeat_vector(storage.state, final_states)?,
            group_shrink,
            // GroupInfo starts its inner names vector at vec![None], not
            // RawVec's append minimum; even the first growth can overlap.
            layout.repeat_vector(storage.name_slot, self.groups.saturating_sub(1))?,
            // Inner::default's empty GroupInfo Arc lives until replacement.
            retained.group_info.arc_bytes,
        ])
    }

    fn state() -> Self {
        Self {
            counts: ConstructionCounts {
                states: 1,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn empty() -> Self {
        Self {
            counts: ConstructionCounts {
                states: 1,
                empties: 1,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn class(ranges: usize) -> Self {
        Self {
            counts: ConstructionCounts {
                states: 2,
                empties: 1,
                sparse_states: 1,
                sparse_transitions: ranges,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn union(alternates: usize) -> Self {
        Self {
            counts: ConstructionCounts {
                states: 1,
                unions: 1,
                union_alternates: alternates,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn plus(self, other: Self) -> Result<Self, ConstructionError> {
        Ok(Self {
            counts: self.counts.checked_add(other.counts)?,
            calls: add(self.calls, other.calls)?,
            patches: add(self.patches, other.patches)?,
            groups: self.groups.max(other.groups),
        })
    }
    fn repeat(self, times: usize) -> Result<Self, ConstructionError> {
        Ok(Self {
            counts: self.counts.checked_repeat(times)?,
            calls: mul(self.calls, times)?,
            patches: mul(self.patches, times)?,
            groups: if times == 0 { 0 } else { self.groups },
        })
    }
    fn patch(mut self, count: usize) -> Result<Self, ConstructionError> {
        self.patches = add(self.patches, count)?;
        Ok(self)
    }
    fn capture(self, index: usize, reverse: bool) -> Result<Self, ConstructionError> {
        if reverse {
            return Ok(self);
        }
        SmallIndex::new(index).map_err(|_index_error| ConstructionError::Scope)?;
        self.plus(Self {
            counts: ConstructionCounts {
                states: 2,
                captures: 2,
                ..Default::default()
            },
            groups: add(index, 1)?,
            ..Default::default()
        })?
        .patch(2)
    }
    fn exact(self, count: usize) -> Result<Self, ConstructionError> {
        if count == 0 {
            Ok(Self::empty())
        } else {
            self.repeat(count)?
                .patch(count.checked_sub(1).ok_or(ConstructionError::Arithmetic)?)
        }
    }
    fn hir(hir: &Hir, reverse: bool, depth: u32) -> Result<Self, ConstructionError> {
        if depth > syntax::Config::default().get_nest_limit() {
            return Err(ConstructionError::Scope);
        }
        let child_depth = depth.checked_add(1).ok_or(ConstructionError::Arithmetic)?;
        let child = |sub_hir| Self::hir(sub_hir, reverse, child_depth);
        let mut model = match hir.kind() {
            HirKind::Empty => Self::empty(),
            HirKind::Literal(bytes) => Self::state().exact(bytes.0.len())?,
            HirKind::Look(Look::Start | Look::End) => Self::state(),
            HirKind::Class(Class::Unicode(class))
                if class.is_ascii() && !class.ranges().is_empty() =>
            {
                Self::class(class.ranges().len())
            }
            HirKind::Class(Class::Unicode(_)) => {
                return Err(ConstructionError::UnboundOwner(if reverse {
                    "thompson::map::Utf8SuffixEntry"
                } else {
                    "thompson::compiler::Utf8Node"
                }));
            }
            HirKind::Class(Class::Bytes(class)) if !class.ranges().is_empty() => {
                Self::class(class.ranges().len())
            }
            HirKind::Capture(capture) if capture.name.is_none() => child(&capture.sub)?.capture(
                usize::try_from(capture.index)
                    .map_err(|_conversion_error| ConstructionError::Arithmetic)?,
                reverse,
            )?,
            HirKind::Concat(parts) => {
                let mut m = Self::default();
                for part in parts {
                    m = m.plus(child(part)?)?;
                }
                if parts.is_empty() {
                    Self::empty()
                } else {
                    m.patch(
                        parts
                            .len()
                            .checked_sub(1)
                            .ok_or(ConstructionError::Arithmetic)?,
                    )?
                }
            }
            HirKind::Alternation(parts) => {
                if parts.len() < 2
                    || parts
                        .iter()
                        .all(|p| matches!(p.kind(), HirKind::Literal(_)))
                {
                    return Err(ConstructionError::Scope);
                }
                let mut m = Self::union(parts.len()).plus(Self::empty())?;
                for part in parts {
                    m = m.plus(child(part)?)?;
                }
                m.patch(mul(parts.len(), 2)?)?
            }
            HirKind::Repetition(rep) => child(&rep.sub)?.repetition(rep)?,
            HirKind::Class(_) | HirKind::Look(_) | HirKind::Capture(_) => {
                return Err(ConstructionError::Scope);
            }
        };
        model.calls = add(model.calls, 1)?;
        Ok(model)
    }

    fn repetition(self, rep: &Repetition) -> Result<Self, ConstructionError> {
        let min =
            usize::try_from(rep.min).map_err(|_conversion_error| ConstructionError::Arithmetic)?;
        match rep.max {
            Some(1) if min == 0 => self.plus(Self::union(2))?.plus(Self::empty())?.patch(3),
            Some(max) => {
                let max_count = usize::try_from(max)
                    .map_err(|_conversion_error| ConstructionError::Arithmetic)?;
                let extra = max_count.checked_sub(min).ok_or(ConstructionError::Scope)?;
                let prefix = self.exact(min)?;
                if extra == 0 {
                    Ok(prefix)
                } else {
                    prefix
                        .plus(Self::empty())?
                        .plus(self.plus(Self::union(2))?.repeat(extra)?)?
                        .patch(add(mul(extra, 3)?, 1)?)
                }
            }
            None if min == 0 && rep.sub.properties().minimum_len().is_some_and(|n| n > 0) => {
                self.plus(Self::union(2))?.patch(2)
            }
            None if min == 0 => self
                .plus(Self::union(2).repeat(2)?)?
                .plus(Self::empty())?
                .patch(5),
            None if min == 1 => self.plus(Self::union(2))?.patch(2),
            None => self
                .exact(min.checked_sub(1).ok_or(ConstructionError::Arithmetic)?)?
                .plus(self)?
                .plus(Self::union(2))?
                .patch(3),
        }
    }
}

fn compile_nfa(hir: &Hir, reverse: bool) -> Result<thompson::NFA, ConstructionError> {
    let mut look = LookMatcher::new();
    look.set_line_terminator(b'\n');
    thompson::Compiler::new()
        .configure(
            thompson::Config::new()
                .utf8(true)
                .reverse(reverse)
                .shrink(false)
                .which_captures(if reverse {
                    WhichCaptures::None
                } else {
                    WhichCaptures::All
                })
                .nfa_size_limit(Some(0x0001_0000))
                .look_matcher(look),
        )
        .build_from_hir(hir)
        .map_err(|_build_error| ConstructionError::Compilation)
}

fn add(a: usize, b: usize) -> Result<usize, ConstructionError> {
    a.checked_add(b).ok_or(ConstructionError::Arithmetic)
}
fn mul(a: usize, b: usize) -> Result<usize, ConstructionError> {
    a.checked_mul(b).ok_or(ConstructionError::Arithmetic)
}
fn sum(values: &[usize]) -> Result<usize, ConstructionError> {
    values.iter().try_fold(0, |total, &n| add(total, n))
}
fn append_peak(
    layout: HeapLayout,
    element: usize,
    count: usize,
) -> Result<usize, ConstructionError> {
    let bound = layout.append_vector(element, count)?;
    add(bound.retained_bytes, bound.old_growth_bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        ConstructionCounts, ConstructionError, NativeField, NfaConstructionCertificate,
        source_trie_state_layout,
    };
    use regex_automata::{nfa::thompson::Builder, util::syntax};
    use regex_syntax::hir::{Class, Hir, HirKind};
    use std::{alloc::Layout, any::type_name, collections::BTreeMap, sync::atomic::AtomicUsize};

    fn target_facts(word: usize) -> BTreeMap<String, NativeField> {
        use crate::schema_nfa_heap::target::{ABI_POINTER, fact_keys};
        use regex_automata::nfa::thompson::{State, Transition};
        use regex_automata::util::primitives::{SmallIndex, StateID};
        use regex_syntax::hir::ClassBytesRange;
        use std::{collections::HashMap, sync::Arc};
        let words = |count| word.checked_mul(count).expect("synthetic ABI byte size");
        let mut rows = fact_keys()
            .into_iter()
            .map(|key| {
                (
                    key.to_owned(),
                    NativeField {
                        bytes: words(16),
                        alignment: word,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        for (key, bytes, alignment) in [
            (ABI_POINTER, word, word),
            (type_name::<AtomicUsize>(), word, word),
            (type_name::<StateID>(), 4, 4),
            (type_name::<Transition>(), 8, 4),
            (type_name::<State>(), words(4), word),
            (type_name::<(SmallIndex, SmallIndex)>(), 8, 4),
            (type_name::<HashMap<Arc<str>, SmallIndex>>(), words(8), word),
            (type_name::<Vec<Option<Arc<str>>>>(), words(3), word),
            (type_name::<Option<Arc<str>>>(), words(2), word),
            (
                "regex_automata::nfa::thompson::builder::State",
                words(8),
                word,
            ),
            (
                "regex_automata::nfa::thompson::range_trie::State",
                words(3),
                word,
            ),
            (type_name::<(StateID, StateID)>(), 8, 4),
            (type_name::<bool>(), 1, 1),
            (type_name::<ClassBytesRange>(), 2, 1),
            ("regex_syntax::hir::PropertiesI", words(12), word),
        ] {
            rows.insert(key.to_owned(), NativeField { bytes, alignment });
        }
        rows
    }

    #[test]
    fn target_nfa_construction_uses_independent_abis_without_changing_topology() {
        let hir = syntax::parse(r"\.[a-zA-Z]{2,3}$").unwrap();
        for reverse in [false, true] {
            let small =
                NfaConstructionCertificate::from_hir_with_layouts(&hir, reverse, &target_facts(4))
                    .expect("explicit synthetic 32-bit facts");
            let large =
                NfaConstructionCertificate::from_hir_with_layouts(&hir, reverse, &target_facts(8))
                    .expect("explicit synthetic 64-bit facts");
            assert_eq!(small.counts, large.counts);
            assert_eq!(small.state_slot_bytes, 32);
            assert_eq!(large.state_slot_bytes, 64);
            assert!(small.peak_requested_bytes < large.peak_requested_bytes);
            assert!(small.final_nfa_bytes < large.final_nfa_bytes);
            assert!(small.work_upper_bound < large.work_upper_bound);
        }
    }

    #[test]
    fn target_nfa_construction_requires_every_fact_and_valid_layouts() {
        let hir = syntax::parse(r"\.[a-zA-Z]{2,3}$").unwrap();
        let facts = target_facts(4);
        NfaConstructionCertificate::from_hir_with_layouts(&hir, false, &facts)
            .expect("complete valid synthetic target layout facts");
        for key in facts.keys() {
            let mut missing = facts.clone();
            missing.remove(key).unwrap();
            assert_eq!(
                NfaConstructionCertificate::from_hir_with_layouts(&hir, false, &missing),
                Err(ConstructionError::Layout),
                "missing {key}"
            );
            let mut invalid = facts.clone();
            invalid.get_mut(key).unwrap().alignment = 3;
            assert_eq!(
                NfaConstructionCertificate::from_hir_with_layouts(&hir, false, &invalid),
                Err(ConstructionError::Layout),
                "invalid {key}"
            );
        }
    }

    #[test]
    fn target_nfa_construction_each_owned_payload_affects_the_envelope() {
        let hir = syntax::parse(r"\.[a-zA-Z]{2,3}$").unwrap();
        let facts = target_facts(4);
        let base = NfaConstructionCertificate::from_hir_with_layouts(&hir, false, &facts).unwrap();
        for key in facts.keys().filter(|key| {
            key.as_str() != "compiler_abi::Pointer" && key.as_str() != type_name::<AtomicUsize>()
        }) {
            let mut changed = facts.clone();
            changed.get_mut(key).unwrap().bytes *= 2;
            let altered = NfaConstructionCertificate::from_hir_with_layouts(&hir, false, &changed)
                .unwrap_or_else(|error| panic!("{key}: {error:?}"));
            assert!(
                altered.peak_requested_bytes > base.peak_requested_bytes,
                "{key}"
            );
            assert_eq!(altered.counts, base.counts, "{key}");
        }
    }

    fn certificate(pattern: &str, reverse: bool) -> NfaConstructionCertificate {
        let hir = syntax::parse_with(pattern, &syntax::Config::default().utf8(true)).unwrap();
        NfaConstructionCertificate::from_hir(&hir, reverse, source_trie_state_layout().unwrap())
            .unwrap()
    }

    fn counts(values: [usize; 7]) -> ConstructionCounts {
        let [
            states,
            empties,
            sparse_states,
            sparse_transitions,
            unions,
            union_alternates,
            captures,
        ] = values;
        ConstructionCounts {
            states,
            empties,
            sparse_states,
            sparse_transitions,
            unions,
            union_alternates,
            captures,
        }
    }

    #[test]
    fn nfa_construction_digit_counts_include_discarded_empty_and_implicit_capture_states() {
        // compiler.rs:978-1015,1139-1156,1299-1309,1403-1414.
        assert_eq!(
            certificate("^[0-9]+$", false).counts,
            counts([9, 2, 1, 1, 1, 2, 2])
        );
        assert_eq!(
            certificate("^[0-9]+$", true).counts,
            counts([7, 2, 1, 1, 1, 2, 0])
        );
    }

    #[test]
    fn nfa_construction_optional_capture_and_bounded_literal_counts_are_independent() {
        assert_eq!(
            certificate("^(a{2,3})?$", false).counts,
            counts([15, 3, 0, 0, 2, 4, 4])
        );
        assert_eq!(
            certificate("^(a{2,3})?$", true).counts,
            counts([11, 3, 0, 0, 2, 4, 0])
        );
        assert_eq!(
            certificate("^[a-z]{0,2}$", false).counts,
            counts([14, 5, 2, 2, 2, 4, 2])
        );
    }

    #[test]
    fn nfa_construction_suffix_counts_include_only_the_forward_unanchored_prefix() {
        assert_eq!(
            certificate(r"\.[a-zA-Z]{2,3}$", false).counts,
            counts([16, 5, 4, 7, 2, 4, 2])
        );
        assert_eq!(
            certificate(r"\.[a-zA-Z]{2,3}$", true).counts,
            counts([12, 5, 3, 6, 1, 2, 0])
        );
    }

    #[test]
    fn nfa_construction_ledger_keeps_growth_conversion_and_failure_cleanup_funded() {
        let mut witness = Builder::new();
        assert_eq!(witness.memory_usage(), 0);
        witness.add_empty().unwrap();
        let slot_bytes = witness.memory_usage();
        assert!(slot_bytes > 0);

        let bound = certificate("^[0-9]+$", false);
        assert_eq!(bound.state_slot_bytes, slot_bytes);
        assert!(bound.intermediate_peak_bytes > bound.counts.states * slot_bytes);
        assert!(bound.conversion_scratch_bytes > 0);
        assert!(bound.final_nfa_bytes > 0);
        assert_eq!(
            bound.initialization_scratch_bytes,
            bound
                .intermediate_peak_bytes
                .checked_add(bound.conversion_scratch_bytes)
                .unwrap()
        );
        let conversion_peak = bound
            .intermediate_peak_bytes
            .checked_add(bound.conversion_scratch_bytes)
            .and_then(|bytes| bytes.checked_add(bound.final_nfa_bytes))
            .unwrap();
        assert!(bound.peak_requested_bytes >= conversion_peak);
        assert!(bound.failure_prefix_peak_bytes >= bound.intermediate_peak_bytes);
        assert!(bound.peak_requested_bytes >= bound.failure_prefix_peak_bytes);
        assert!(bound.work_upper_bound >= bound.counts.states);
    }

    #[test]
    fn nfa_construction_each_count_addition_and_repetition_refuses_overflow() {
        for field in 0..7 {
            let mut maximum = [0; 7];
            maximum[field] = usize::MAX;
            let mut one = [0; 7];
            one[field] = 1;
            assert_eq!(
                counts(maximum).checked_add(counts(one)),
                Err(ConstructionError::Arithmetic)
            );
            assert_eq!(
                counts(maximum).checked_repeat(2),
                Err(ConstructionError::Arithmetic)
            );
        }
        let small = counts([9, 2, 1, 1, 1, 2, 2]);
        assert_eq!(small.checked_add(small), Ok(counts([18, 4, 2, 2, 2, 4, 4])));
        assert_eq!(small.checked_repeat(0), Ok(ConstructionCounts::default()));
    }

    fn has_non_ascii_class(hir: &Hir) -> bool {
        matches!(hir.kind(), HirKind::Class(Class::Unicode(class)) if !class.is_ascii())
            || hir.kind().subs().iter().any(has_non_ascii_class)
    }

    #[test]
    fn nfa_construction_raw_unicode_digit_cannot_use_the_ascii_certificate() {
        let hir = syntax::parse_with(r"^\d+$", &syntax::Config::default().utf8(true)).unwrap();
        assert!(has_non_ascii_class(&hir));
        for reverse in [false, true] {
            assert_eq!(
                NfaConstructionCertificate::from_hir(
                    &hir,
                    reverse,
                    source_trie_state_layout().unwrap()
                ),
                Err(ConstructionError::UnboundOwner(if reverse {
                    "thompson::map::Utf8SuffixEntry"
                } else {
                    "thompson::compiler::Utf8Node"
                }))
            );
        }
    }

    #[test]
    fn nfa_construction_trie_layout_is_explicit_validated_and_fully_funded() {
        let hir = syntax::parse_with("^[0-9]+$", &syntax::Config::default().utf8(true)).unwrap();
        let native = source_trie_state_layout().unwrap();
        assert_eq!(
            NfaConstructionCertificate::from_hir(&hir, false, Layout::new::<()>()),
            Err(ConstructionError::Layout)
        );
        let maximum = usize::try_from(isize::MAX).unwrap();
        let overflowing = Layout::from_size_align(
            maximum
                .checked_sub(maximum.checked_rem(native.align()).unwrap())
                .unwrap(),
            native.align(),
        )
        .unwrap();
        assert_eq!(
            NfaConstructionCertificate::from_hir(&hir, false, overflowing),
            Err(ConstructionError::Arithmetic)
        );
        let larger =
            Layout::from_size_align(native.size().checked_mul(2).unwrap(), native.align()).unwrap();
        let base = NfaConstructionCertificate::from_hir(&hir, false, native).unwrap();
        let changed = NfaConstructionCertificate::from_hir(&hir, false, larger).unwrap();
        assert!(changed.trie_initial_bytes > base.trie_initial_bytes);
        assert_eq!(changed.counts, base.counts);
        assert_eq!(changed.final_nfa_bytes, base.final_nfa_bytes);
        assert_eq!(
            changed.peak_requested_bytes - base.peak_requested_bytes,
            changed.trie_initial_bytes - base.trie_initial_bytes
        );
        assert!(changed.work_upper_bound > base.work_upper_bound);
    }
}

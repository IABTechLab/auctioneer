//! Conditional requested-heap/source-visit bounds for the selected
//! one-pass constructor attempt, including refusal cleanup. Excludes caller-live
//! HIR, shared NFA heap, other retained programs, machine stack, allocator
//! overhead and search caches. No work, frame or serving qualification is implied.
//! Premises: pinned regex-automata 0.4.18/Rust 1.98.1 collection paths,
//! syntax enabled and logging disabled. The native API uses reviewed natural-
//! layout envelopes. Target projection requires compiler-owned payload/ABI facts;
//! native NFA topology is structural input, never target ABI evidence. The stack
//! tuple retains the reviewed natural-field-slot envelope, using target facts
//! for both fields, not substitute layouts for missing private payloads.

use crate::schema_heap_layout::{HeapLayout, LayoutError, NativeField, private_struct_slot_bound};
use crate::schema_nfa_construction::source_trie_state_layout;
use regex_automata::{
    nfa::thompson::{NFA, State},
    util::{alphabet::ByteClasses, look::Look, primitives::StateID},
};
use std::collections::BTreeMap;

pub(crate) const TRIE_STATE: &str = "regex_automata::nfa::thompson::range_trie::State";
pub(crate) const EPSILONS: &str = "regex_automata::dfa::onepass::Epsilons";
pub(crate) const TRANSITION: &str = "regex_automata::dfa::onepass::Transition";
pub(crate) const STATE_ID: &str = "regex_automata::util::primitives::StateID";
pub(crate) const BYTE_CLASSES: &str = "regex_automata::util::alphabet::ByteClasses";
/// Supplied from the compiler artifact's verified pointer ABI, not a type query.
pub(crate) const ABI_POINTER: &str = "compiler_abi::Pointer";

/// Exact required keys; missing compiler payloads have no native fallback.
pub(crate) fn fact_keys() -> [&'static str; 6] {
    [
        TRIE_STATE,
        EPSILONS,
        TRANSITION,
        STATE_ID,
        BYTE_CLASSES,
        ABI_POINTER,
    ]
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ConstructionError {
    Scope,
    Arithmetic,
    Layout,
    MissingType(&'static str),
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
pub(crate) struct OnePassConstructionCertificate {
    pub nfa_states: usize,
    pub rows_upper_bound: usize,
    pub stride: usize,
    pub scaffold_bytes: usize,
    pub queue_peak_bytes: usize,
    pub nfa_map_bytes: usize,
    pub stack_peak_bytes: usize,
    pub seen_bytes: usize,
    pub table_peak_bytes: usize,
    pub starts_peak_bytes: usize,
    pub remapper_peak_bytes: usize,
    pub shrink_overlap_bytes: usize,
    /// Already included in the constructor peak, not an additive heap owner.
    pub retained_tables_upper_bytes: usize,
    pub peak_requested_bytes: usize,
    pub failure_prefix_peak_bytes: usize,
    pub closure_visits_upper_bound: usize,
    pub byte_visits_upper_bound: usize,
    pub remap_visits_upper_bound: usize,
    /// Traversal visits plus byte initialization/copy/drop envelopes, not every
    /// scalar field access, instructions or an enforced engine counter.
    pub work_upper_bound: usize,
}

impl OnePassConstructionCertificate {
    /// For `candidate_onepass`'s fixed `LeftmostFirst`, byte-class, per-pattern
    /// starts, 1 MiB configuration. A bound on an attempt, not proof it succeeds.
    pub(crate) fn from_nfa(nfa: &NFA) -> Result<Self, ConstructionError> {
        Self::project(nfa, &ConstructionLayout::native()?)
    }

    /// Conditional target-layout projection of the same selected attempt. The
    /// producer must bind all `fact_keys()` and this topology to the pinned
    /// source/features/compiler/target. This does not execute or qualify a guest
    /// constructor, nor establish work, frame or serving qualification.
    pub(crate) fn from_nfa_with_layouts(
        nfa: &NFA,
        facts: &BTreeMap<String, NativeField>,
    ) -> Result<Self, ConstructionError> {
        Self::project(nfa, &ConstructionLayout::with_layouts(facts)?)
    }

    fn project(nfa: &NFA, layout: &ConstructionLayout) -> Result<Self, ConstructionError> {
        let (consuming, epsilon) = selected_edges(nfa)?;
        let nfa_states = nfa.states().len();
        // onepass.rs:838-895: DEAD plus one row per previously unseen NFA ID.
        // Include the prospective row: size checking follows table extension.
        let rows_upper_bound = add(nfa_states, 1)?;
        let stride_shift = u32::try_from(nfa.byte_classes().stride2())
            .map_err(|_error| ConstructionError::Arithmetic)?;
        let stride = 1_usize
            .checked_shl(stride_shift)
            .ok_or(ConstructionError::Arithmetic)?;
        let entries = mul(rows_upper_bound, stride)?;
        // Builder::new eagerly owns a Compiler, even for build_from_nfa.
        // Only RangeTrie's two empty states allocate; UTF-8 maps remain lazy.
        let scaffold_bytes = layout.append(layout.trie, 2)?;
        let queue_peak_bytes = layout.append(layout.state_id, nfa_states)?;
        let nfa_map_bytes = layout.repeat(layout.state_id, nfa_states)?;
        // stack_push inserts into seen before pushing; duplicate insertion
        // refuses before push. Each queue ID enters only once, before its pop.
        let stack_peak_bytes = layout.append(layout.stack, nfa_states)?;
        let seen_bytes = mul(layout.fresh(layout.state_id, nfa_states)?, 2)?;
        let table_peak_bytes = layout.append(layout.transition, entries)?;
        let starts_peak_bytes = layout.append(layout.state_id, 2)?;
        // remapper.rs:94-97,133: trusted-length collect, then an exact Vec clone.
        let remapper_peak_bytes = mul(layout.repeat(layout.state_id, rows_upper_bound)?, 2)?;
        let retained_tables_upper_bytes = add(
            layout.repeat(layout.transition, entries)?,
            layout.repeat(layout.state_id, 2)?,
        )?;
        // Both shrinks run while InternalBuilder's scratch remains live.
        // Fund old capacities plus new len-sized requests without subtracting
        // any deallocation; the pinned successful shrink requests exact len.
        let shrink_overlap_bytes = retained_tables_upper_bytes;
        let peak_requested_bytes = sum(&[
            scaffold_bytes,
            queue_peak_bytes,
            nfa_map_bytes,
            stack_peak_bytes,
            seen_bytes,
            table_peak_bytes,
            starts_peak_bytes,
            remapper_peak_bytes,
            shrink_overlap_bytes,
        ])?;
        let closure_visits_upper_bound = mul(rows_upper_bound, add(nfa_states, epsilon)?)?;
        // alphabet.rs:564-588 scans at most 256 bytes per edge; each yielded
        // representative is also handled once in compile_transition.
        let byte_visits_upper_bound = mul(mul(mul(rows_upper_bound, consuming)?, 256)?, 2)?;
        // remapper.rs:134-149 follows one permutation cycle per row, not once
        // globally. Cycles have at most D entries, so charge D squared.
        let remap_visits_upper_bound = mul(rows_upper_bound, rows_upper_bound)?;
        let work_upper_bound = sum(&[
            closure_visits_upper_bound,
            byte_visits_upper_bound,
            remap_visits_upper_bound,
            // Queue pop/root push; match scan/map outer iteration; two starts.
            mul(nfa_states, 2)?,
            mul(rows_upper_bound, 2)?,
            2,
            // The look-check loop and two explicit inline ByteClasses clones.
            nfa.look_set_any().iter().count(),
            mul(layout.byte_classes_bytes, 2)?,
            // Per table entry: initialize, two swap visits, final remap visit.
            mul(entries, 4)?,
            // Geometric copies, initialization and drops of the owned buffers.
            mul(peak_requested_bytes, 2)?,
        ])?;
        Ok(Self {
            nfa_states,
            rows_upper_bound,
            stride,
            scaffold_bytes,
            queue_peak_bytes,
            nfa_map_bytes,
            stack_peak_bytes,
            seen_bytes,
            table_peak_bytes,
            starts_peak_bytes,
            remapper_peak_bytes,
            shrink_overlap_bytes,
            retained_tables_upper_bytes,
            peak_requested_bytes,
            // Reachable refusal diagnostics are scalars/static strings. Every
            // mutated prefix and cleanup is covered by the monotonic envelope.
            failure_prefix_peak_bytes: peak_requested_bytes,
            closure_visits_upper_bound,
            byte_visits_upper_bound,
            remap_visits_upper_bound,
            work_upper_bound,
        })
    }
}

struct ConstructionLayout {
    heap: HeapLayout,
    maximum: usize,
    trie: NativeField,
    state_id: NativeField,
    stack: NativeField,
    transition: NativeField,
    byte_classes_bytes: usize,
}

impl ConstructionLayout {
    fn native() -> Result<Self, ConstructionError> {
        let word = NativeField::of::<usize>();
        let trie = source_trie_state_layout().map_err(|_error| ConstructionError::Layout)?;
        Ok(Self {
            heap: HeapLayout::new(word.bytes, word.bytes)?,
            maximum: allocation_maximum(word)?,
            trie: NativeField {
                bytes: trie.size(),
                alignment: trie.align(),
            },
            state_id: NativeField::of::<StateID>(),
            // onepass.rs:2608,2803: sole-u64 wrappers, without repr/align overrides.
            stack: private_struct_slot_bound(&[
                NativeField::of::<StateID>(),
                NativeField::of::<u64>(),
            ])?,
            transition: private_struct_slot_bound(&[NativeField::of::<u64>()])?,
            byte_classes_bytes: size_of::<ByteClasses>(),
        })
    }

    fn with_layouts(facts: &BTreeMap<String, NativeField>) -> Result<Self, ConstructionError> {
        for name in fact_keys() {
            facts
                .get(name)
                .ok_or(ConstructionError::MissingType(name))?
                .validate()?;
        }
        let word = facts[ABI_POINTER];
        let layout = Self {
            heap: HeapLayout::new(word.bytes, word.bytes)?,
            maximum: allocation_maximum(word)?,
            trie: facts[TRIE_STATE],
            state_id: facts[STATE_ID],
            // Bound the tuple, not either private payload. Both fields must
            // have their own compiler facts even when their native sizes agree.
            stack: private_struct_slot_bound(&[facts[STATE_ID], facts[EPSILONS]])?,
            transition: facts[TRANSITION],
            byte_classes_bytes: facts[BYTE_CLASSES].bytes,
        };
        for field in fact_keys()
            .into_iter()
            .map(|name| facts[name])
            .chain([layout.stack])
        {
            layout.request(field.bytes, field.alignment)?;
        }
        Ok(layout)
    }

    fn request(&self, bytes: usize, alignment: usize) -> Result<usize, ConstructionError> {
        // core/alloc/layout.rs: target isize::MAX + 1 - alignment, not the
        // inspecting host's limit. Check requests, not sums of live owners.
        let padding = alignment.checked_sub(1).ok_or(ConstructionError::Layout)?;
        let maximum = self
            .maximum
            .checked_sub(padding)
            .ok_or(ConstructionError::Arithmetic)?;
        if bytes > maximum {
            return Err(ConstructionError::Arithmetic);
        }
        Ok(bytes)
    }

    fn repeat(&self, field: NativeField, count: usize) -> Result<usize, ConstructionError> {
        self.request(
            self.heap.repeat_vector(field.bytes, count)?,
            field.alignment,
        )
    }

    fn fresh(&self, field: NativeField, count: usize) -> Result<usize, ConstructionError> {
        self.request(
            self.heap.fresh_resize_vector(field.bytes, count)?,
            field.alignment,
        )
    }

    fn append(&self, field: NativeField, count: usize) -> Result<usize, ConstructionError> {
        let growth = self.heap.append_vector(field.bytes, count)?;
        self.request(growth.retained_bytes, field.alignment)?;
        self.request(growth.old_growth_bytes, field.alignment)?;
        add(growth.retained_bytes, growth.old_growth_bytes)
    }
}

fn allocation_maximum(word: NativeField) -> Result<usize, ConstructionError> {
    if word.alignment != word.bytes {
        return Err(ConstructionError::Layout);
    }
    let maximum = match word.bytes {
        4 => i64::from(i32::MAX),
        8 => i64::MAX,
        _ => return Err(ConstructionError::Layout),
    };
    usize::try_from(maximum).map_err(|_error| ConstructionError::Arithmetic)
}

fn selected_edges(nfa: &NFA) -> Result<(usize, usize), ConstructionError> {
    if nfa.pattern_len() != 1
        || !nfa.is_utf8()
        || nfa.is_reverse()
        || !nfa.is_always_start_anchored()
        || !(1..=32).contains(&nfa.group_info().explicit_slot_len())
    {
        return Err(ConstructionError::Scope);
    }
    let mut consuming = 0;
    let mut epsilon = 0;
    for state in nfa.states() {
        let (bytes, next) = match state {
            State::ByteRange { trans } if trans.end.is_ascii() => (1, 0),
            State::Sparse(sparse)
                if sparse.transitions.iter().all(|trans| trans.end.is_ascii()) =>
            {
                (sparse.transitions.len(), 0)
            }
            State::Look {
                look: Look::Start | Look::End,
                ..
            }
            | State::Capture { .. } => (0, 1),
            State::Union { alternates } => (0, alternates.len()),
            State::BinaryUnion { .. } => (0, 2),
            State::Fail | State::Match { .. } => (0, 0),
            State::ByteRange { .. } | State::Sparse(_) | State::Dense(_) | State::Look { .. } => {
                return Err(ConstructionError::Scope);
            }
        };
        consuming = add(consuming, bytes)?;
        epsilon = add(epsilon, next)?;
    }
    Ok((consuming, epsilon))
}

fn add(a: usize, b: usize) -> Result<usize, ConstructionError> {
    a.checked_add(b).ok_or(ConstructionError::Arithmetic)
}
fn mul(a: usize, b: usize) -> Result<usize, ConstructionError> {
    a.checked_mul(b).ok_or(ConstructionError::Arithmetic)
}
fn sum(values: &[usize]) -> Result<usize, ConstructionError> {
    values.iter().try_fold(0, |total, &value| add(total, value))
}

#[cfg(test)]
mod tests {
    use super::{
        ABI_POINTER, BYTE_CLASSES, ConstructionError, ConstructionLayout, EPSILONS,
        OnePassConstructionCertificate, STATE_ID, TRANSITION, TRIE_STATE, fact_keys,
    };
    use crate::schema_cache_bounds::forward_nfa;
    use crate::schema_heap_layout::{HeapLayout, NativeField, private_struct_slot_bound};
    use crate::schema_nfa_construction::source_trie_state_layout;
    use crate::schema_onepass_bounds::{candidate_onepass, nfa_table_upper_bound};
    use regex_automata::{
        nfa::thompson::{NFA, State},
        util::{alphabet::ByteClasses, primitives::StateID},
    };
    use std::{any::type_name, collections::BTreeMap};

    const CAPTURE_PATTERN: &str = r"^[0-9]+(\.[0-9]+)?$";

    fn native_layout() -> HeapLayout {
        HeapLayout::new(size_of::<usize>(), size_of::<usize>()).unwrap()
    }

    fn append_peak(element: usize, entries: usize) -> usize {
        let growth = native_layout().append_vector(element, entries).unwrap();
        growth
            .retained_bytes
            .checked_add(growth.old_growth_bytes)
            .unwrap()
    }

    fn bound(nfa: &NFA) -> OnePassConstructionCertificate {
        OnePassConstructionCertificate::from_nfa(nfa).unwrap()
    }

    // Deliberately unequal synthetic inputs, not native/guest ABI observations.
    fn facts(word: usize) -> BTreeMap<String, NativeField> {
        let rows = match word {
            4 => [
                (TRIE_STATE, 12, 4),
                (EPSILONS, 8, 4),
                (TRANSITION, 16, 8),
                (STATE_ID, 4, 4),
                (BYTE_CLASSES, 272, 16),
                (ABI_POINTER, 4, 4),
            ],
            8 => [
                (TRIE_STATE, 40, 8),
                (EPSILONS, 16, 8),
                (TRANSITION, 24, 8),
                (STATE_ID, 8, 8),
                (BYTE_CLASSES, 320, 16),
                (ABI_POINTER, 8, 8),
            ],
            _ => panic!("unsupported fixture word"),
        };
        rows.into_iter()
            .map(|(name, bytes, alignment)| (name.to_owned(), NativeField { bytes, alignment }))
            .collect()
    }

    fn assert_target_work_equation(
        nfa: &NFA,
        proof: &OnePassConstructionCertificate,
        facts: &BTreeMap<String, NativeField>,
        states: usize,
        rows: usize,
        entries: usize,
    ) {
        let total = [
            proof.scaffold_bytes,
            proof.queue_peak_bytes,
            proof.nfa_map_bytes,
            proof.stack_peak_bytes,
            proof.seen_bytes,
            proof.table_peak_bytes,
            proof.starts_peak_bytes,
            proof.remapper_peak_bytes,
            proof.shrink_overlap_bytes,
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .unwrap();
        assert_eq!(proof.peak_requested_bytes, total);
        assert_eq!(proof.failure_prefix_peak_bytes, total);
        assert_eq!(
            proof.work_upper_bound,
            super::sum(&[
                proof.closure_visits_upper_bound,
                proof.byte_visits_upper_bound,
                proof.remap_visits_upper_bound,
                states.checked_mul(2).unwrap(),
                rows.checked_mul(2).unwrap(),
                2,
                nfa.look_set_any().iter().count(),
                facts[BYTE_CLASSES].bytes.checked_mul(2).unwrap(),
                entries.checked_mul(4).unwrap(),
                total.checked_mul(2).unwrap(),
            ])
            .unwrap()
        );
    }

    #[test]
    fn target_onepass_uses_unequal_layouts_for_every_buffer_and_inline_clone() {
        assert_eq!(STATE_ID, type_name::<StateID>());
        assert_eq!(BYTE_CLASSES, type_name::<ByteClasses>());
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        let native = bound(&nfa);
        for word in [4, 8] {
            let facts = facts(word);
            assert_eq!(facts.len(), fact_keys().len());
            assert!(fact_keys().into_iter().all(|name| facts.contains_key(name)));
            let proof =
                OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &facts).unwrap();
            let layout = HeapLayout::new(word, word).unwrap();
            let append = |name: &str, count| {
                let growth = layout.append_vector(facts[name].bytes, count).unwrap();
                growth
                    .retained_bytes
                    .checked_add(growth.old_growth_bytes)
                    .unwrap()
            };
            let states = nfa.states().len();
            let rows = states.checked_add(1).unwrap();
            let entries = rows.checked_mul(proof.stride).unwrap();
            let stack = private_struct_slot_bound(&[facts[STATE_ID], facts[EPSILONS]]).unwrap();
            let stack_growth = layout.append_vector(stack.bytes, states).unwrap();
            let retained = layout
                .repeat_vector(facts[TRANSITION].bytes, entries)
                .unwrap()
                .checked_add(layout.repeat_vector(facts[STATE_ID].bytes, 2).unwrap())
                .unwrap();
            for (actual, expected) in [
                (proof.scaffold_bytes, append(TRIE_STATE, 2)),
                (proof.queue_peak_bytes, append(STATE_ID, states)),
                (
                    proof.nfa_map_bytes,
                    layout.repeat_vector(facts[STATE_ID].bytes, states).unwrap(),
                ),
                (
                    proof.stack_peak_bytes,
                    stack_growth
                        .retained_bytes
                        .checked_add(stack_growth.old_growth_bytes)
                        .unwrap(),
                ),
                (
                    proof.seen_bytes,
                    layout
                        .fresh_resize_vector(facts[STATE_ID].bytes, states)
                        .unwrap()
                        .checked_mul(2)
                        .unwrap(),
                ),
                (proof.table_peak_bytes, append(TRANSITION, entries)),
                (proof.starts_peak_bytes, append(STATE_ID, 2)),
                (
                    proof.remapper_peak_bytes,
                    layout
                        .repeat_vector(facts[STATE_ID].bytes, rows)
                        .unwrap()
                        .checked_mul(2)
                        .unwrap(),
                ),
                (proof.shrink_overlap_bytes, retained),
                (proof.retained_tables_upper_bytes, retained),
            ] {
                assert_eq!(actual, expected);
            }
            assert_eq!(proof.nfa_states, native.nfa_states);
            assert_eq!(proof.stride, native.stride);
            assert_eq!(
                proof.closure_visits_upper_bound,
                native.closure_visits_upper_bound
            );
            assert_eq!(
                proof.byte_visits_upper_bound,
                native.byte_visits_upper_bound
            );
            assert_eq!(
                proof.remap_visits_upper_bound,
                native.remap_visits_upper_bound
            );
            assert_target_work_equation(&nfa, &proof, &facts, states, rows, entries);
        }
    }

    #[test]
    fn target_onepass_each_payload_fact_changes_its_charge() {
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        let base = facts(8);
        let original = OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &base).unwrap();
        for name in fact_keys().into_iter().filter(|name| *name != ABI_POINTER) {
            let mut changed = base.clone();
            let field = changed.get_mut(name).unwrap();
            field.bytes = field.bytes.checked_mul(2).unwrap();
            let mutated =
                OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &changed).unwrap();
            let (before, after) = match name {
                TRIE_STATE => (original.scaffold_bytes, mutated.scaffold_bytes),
                EPSILONS => (original.stack_peak_bytes, mutated.stack_peak_bytes),
                TRANSITION => (original.table_peak_bytes, mutated.table_peak_bytes),
                STATE_ID => (original.queue_peak_bytes, mutated.queue_peak_bytes),
                BYTE_CLASSES => {
                    assert_eq!(original.peak_requested_bytes, mutated.peak_requested_bytes);
                    assert_eq!(
                        mutated
                            .work_upper_bound
                            .checked_sub(original.work_upper_bound)
                            .unwrap(),
                        base[BYTE_CLASSES].bytes.checked_mul(2).unwrap()
                    );
                    (original.work_upper_bound, mutated.work_upper_bound)
                }
                _ => panic!("all required payload facts are charged"),
            };
            assert!(after > before, "{name}");
        }
        let mut realigned = base;
        realigned.get_mut(EPSILONS).unwrap().alignment = 16;
        let mutated =
            OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &realigned).unwrap();
        assert!(mutated.stack_peak_bytes > original.stack_peak_bytes);
    }

    #[test]
    fn target_onepass_requires_every_fact_and_rejects_invalid_shapes_and_abis() {
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        for name in fact_keys() {
            let mut missing = facts(8);
            missing.remove(name).unwrap();
            assert_eq!(
                OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &missing),
                Err(ConstructionError::MissingType(name))
            );
            for invalid in [
                NativeField {
                    bytes: 0,
                    alignment: 1,
                },
                NativeField {
                    bytes: 8,
                    alignment: 0,
                },
                NativeField {
                    bytes: 12,
                    alignment: 3,
                },
                NativeField {
                    bytes: 3,
                    alignment: 2,
                },
            ] {
                let mut changed = facts(8);
                changed.insert(name.to_owned(), invalid);
                assert_eq!(
                    OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &changed),
                    Err(ConstructionError::Layout),
                    "{name}: {invalid:?}"
                );
            }
        }
        for (bytes, alignment) in [(2, 2), (16, 16), (4, 8), (8, 4)] {
            let mut changed = facts(8);
            changed.insert(ABI_POINTER.to_owned(), NativeField { bytes, alignment });
            assert_eq!(
                OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &changed),
                Err(ConstructionError::Layout)
            );
        }
    }

    #[test]
    fn target_onepass_pointer_fact_controls_allocation_refusal_not_host_word_size() {
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        let mut wide = facts(8);
        wide.insert(
            TRIE_STATE.to_owned(),
            NativeField {
                bytes: 1 << 30,
                alignment: 8,
            },
        );
        OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &wide).unwrap();
        let mut narrow = wide;
        narrow.insert(
            ABI_POINTER.to_owned(),
            NativeField {
                bytes: 4,
                alignment: 4,
            },
        );
        assert_eq!(
            OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &narrow),
            Err(ConstructionError::Arithmetic)
        );
    }

    #[test]
    fn target_onepass_request_boundaries_include_alignment_and_checked_growth() {
        for word in [4, 8] {
            let layout = ConstructionLayout::with_layouts(&facts(word)).unwrap();
            let maximum = usize::try_from(if word == 4 {
                i64::from(i32::MAX)
            } else {
                i64::MAX
            })
            .unwrap();
            let byte = NativeField {
                bytes: 1,
                alignment: 1,
            };
            let aligned = NativeField {
                bytes: 16,
                alignment: 16,
            };
            let maximum_plus_one = maximum.checked_add(1).unwrap();
            let repeat_count = maximum.checked_div(16).unwrap();
            let repeat_overflow_count = repeat_count.checked_add(1).unwrap();
            let append_count = maximum.checked_div(32).unwrap();
            let append_overflow_count = append_count.checked_add(1).unwrap();
            assert_eq!(layout.repeat(byte, maximum), Ok(maximum));
            assert_eq!(
                layout.repeat(byte, maximum_plus_one),
                Err(ConstructionError::Arithmetic)
            );
            assert_eq!(
                layout.repeat(aligned, repeat_count),
                Ok(maximum.checked_sub(15).unwrap())
            );
            assert_eq!(
                layout.repeat(aligned, repeat_overflow_count),
                Err(ConstructionError::Arithmetic)
            );
            layout.append(aligned, append_count).unwrap();
            assert_eq!(
                layout.append(aligned, append_overflow_count),
                Err(ConstructionError::Arithmetic)
            );
            layout.fresh(aligned, repeat_count).unwrap();
            assert_eq!(
                layout.fresh(aligned, repeat_overflow_count),
                Err(ConstructionError::Arithmetic)
            );
            assert_eq!(
                layout.append(aligned, usize::MAX),
                Err(ConstructionError::Arithmetic)
            );
            assert_eq!(
                layout.repeat(aligned, usize::MAX),
                Err(ConstructionError::Arithmetic)
            );
            assert_eq!(layout.append(aligned, 0), Ok(0));
            assert_eq!(layout.repeat(aligned, 0), Ok(0));
            assert_eq!(layout.fresh(aligned, 0), Ok(0));
        }
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        let mut oversized_stack = facts(4);
        oversized_stack.insert(
            EPSILONS.to_owned(),
            NativeField {
                bytes: 1 << 30,
                alignment: 1 << 30,
            },
        );
        assert_eq!(
            OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &oversized_stack),
            Err(ConstructionError::Arithmetic)
        );
        let mut overflowing = facts(8);
        overflowing.insert(
            TRANSITION.to_owned(),
            NativeField {
                bytes: usize::MAX,
                alignment: 1,
            },
        );
        assert_eq!(
            OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &overflowing),
            Err(ConstructionError::Arithmetic)
        );
        let mut work_overflow = facts(8);
        work_overflow.insert(
            BYTE_CLASSES.to_owned(),
            NativeField {
                bytes: usize::try_from(i64::MAX).unwrap(),
                alignment: 1,
            },
        );
        assert_eq!(
            OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &work_overflow),
            Err(ConstructionError::Arithmetic)
        );
    }

    #[test]
    fn target_onepass_preserves_scope_and_refusal_cleanup_without_guest_execution() {
        for word in [4, 8] {
            let facts = facts(word);
            let nfa = forward_nfa("^(a*a)$").unwrap();
            candidate_onepass(&nfa).unwrap_err();
            let proof =
                OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, &facts).unwrap();
            assert_eq!(proof.failure_prefix_peak_bytes, proof.peak_requested_bytes);
            assert!(proof.failure_prefix_peak_bytes >= proof.table_peak_bytes);
            for refused_nfa in [
                forward_nfa("^[0-9]+$").unwrap(),
                forward_nfa("(a)").unwrap(),
                forward_nfa(r"^(\b[a-z])$").unwrap(),
                forward_nfa(r"^(\x{E9})$").unwrap(),
                NFA::new_many(&["^(a)$", "^(b)$"]).unwrap(),
            ] {
                assert_eq!(
                    OnePassConstructionCertificate::from_nfa_with_layouts(&refused_nfa, &facts),
                    Err(ConstructionError::Scope)
                );
            }
        }
    }

    #[test]
    fn onepass_construction_dimensions_bound_the_real_pinned_candidate() {
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        let proof = bound(&nfa);
        let dfa = candidate_onepass(&nfa).unwrap();
        assert_eq!(proof.nfa_states, nfa.states().len());
        assert_eq!(
            proof.rows_upper_bound,
            nfa.states().len().checked_add(1).unwrap()
        );
        assert_eq!(proof.stride, dfa.stride());
        assert!(dfa.state_len() <= proof.rows_upper_bound);
        assert!(dfa.memory_usage() <= proof.retained_tables_upper_bytes);
    }

    #[test]
    fn onepass_construction_funds_each_buffer_growth_clone_and_shrink_overlap() {
        // onepass.rs:475-571,725-726,838-895; remapper.rs:94-97,133.
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        let proof = bound(&nfa);
        let layout = native_layout();
        let states = nfa.states().len();
        let rows = states.checked_add(1).unwrap();
        let entries = rows.checked_mul(proof.stride).unwrap();
        // Sole-u64 private wrappers and the stack tuple use the same reviewed
        // natural-field-order envelope, not guessed exact private layouts.
        let transition = private_struct_slot_bound(&[NativeField::of::<u64>()]).unwrap();
        let stack =
            private_struct_slot_bound(&[NativeField::of::<StateID>(), NativeField::of::<u64>()])
                .unwrap();
        let trie = source_trie_state_layout().unwrap();
        let retained = nfa_table_upper_bound(states, proof.stride, 1).unwrap();
        for (actual, expected) in [
            (proof.scaffold_bytes, append_peak(trie.size(), 2)),
            (
                proof.queue_peak_bytes,
                append_peak(size_of::<StateID>(), states),
            ),
            (
                proof.nfa_map_bytes,
                layout.repeat_vector(size_of::<StateID>(), states).unwrap(),
            ),
            (proof.stack_peak_bytes, append_peak(stack.bytes, states)),
            (
                proof.seen_bytes,
                layout
                    .fresh_resize_vector(size_of::<StateID>(), states)
                    .unwrap()
                    .checked_mul(2)
                    .unwrap(),
            ),
            (
                proof.table_peak_bytes,
                append_peak(transition.bytes, entries),
            ),
            (
                proof.starts_peak_bytes,
                append_peak(size_of::<StateID>(), 2),
            ),
            (
                proof.remapper_peak_bytes,
                layout
                    .repeat_vector(size_of::<StateID>(), rows)
                    .unwrap()
                    .checked_mul(2)
                    .unwrap(),
            ),
            (proof.shrink_overlap_bytes, retained.total),
            (proof.retained_tables_upper_bytes, retained.total),
        ] {
            assert_eq!(actual, expected);
        }
        let total = [
            proof.scaffold_bytes,
            proof.queue_peak_bytes,
            proof.nfa_map_bytes,
            proof.stack_peak_bytes,
            proof.seen_bytes,
            proof.table_peak_bytes,
            proof.starts_peak_bytes,
            proof.remapper_peak_bytes,
            proof.shrink_overlap_bytes,
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .unwrap();
        assert_eq!(proof.peak_requested_bytes, total);
        assert!(proof.table_peak_bytes > retained.table_bytes);
    }

    #[test]
    fn onepass_construction_failure_prefix_is_funded_without_requiring_success() {
        // A duplicate closure path/transition can refuse after mutation. The
        // source envelope also covers the prospective row before size checking.
        let nfa = forward_nfa("^(a*a)$").unwrap();
        let refusal = candidate_onepass(&nfa)
            .expect_err("ambiguous capture must refuse one-pass construction");
        assert!(refusal.to_string().contains("conflicting transition"));
        let proof = bound(&nfa);
        assert_eq!(proof.failure_prefix_peak_bytes, proof.peak_requested_bytes);
        assert!(proof.failure_prefix_peak_bytes >= proof.table_peak_bytes);
        assert!(proof.queue_peak_bytes > 0 && proof.stack_peak_bytes > 0);
    }

    #[test]
    fn onepass_construction_work_includes_closures_byte_domain_and_quadratic_remap() {
        let nfa = forward_nfa(CAPTURE_PATTERN).unwrap();
        let proof = bound(&nfa);
        let rows = proof.rows_upper_bound;
        assert!(proof.closure_visits_upper_bound >= rows.checked_mul(proof.nfa_states).unwrap());
        // Representative enumeration covers each consuming edge's full byte
        // domain even when the NFA uses byte classes.
        let edges = nfa
            .states()
            .iter()
            .map(|state| match state {
                State::ByteRange { .. } => 1,
                State::Sparse(sparse) => sparse.transitions.len(),
                State::Dense(_)
                | State::Look { .. }
                | State::Union { .. }
                | State::BinaryUnion { .. }
                | State::Capture { .. }
                | State::Fail
                | State::Match { .. } => 0,
            })
            .try_fold(0_usize, usize::checked_add)
            .unwrap();
        assert!(edges > 0);
        assert!(
            proof.byte_visits_upper_bound
                >= rows.checked_mul(edges).unwrap().checked_mul(256).unwrap()
        );
        assert!(proof.remap_visits_upper_bound >= rows.checked_mul(rows).unwrap());
        let charged = [
            proof.closure_visits_upper_bound,
            proof.byte_visits_upper_bound,
            proof.remap_visits_upper_bound,
            proof.peak_requested_bytes.checked_mul(2).unwrap(),
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .unwrap();
        assert!(proof.work_upper_bound >= charged);
    }

    #[test]
    fn onepass_construction_refuses_paths_outside_the_selected_attempt_scope() {
        // The meta wrapper skips capture-free selected helpers. This API is
        // only for an actual attempted single-pattern constructor, not a gate
        // that can promote a skipped or different backend to qualification.
        for nfa in [
            forward_nfa("^[0-9]+$").unwrap(),
            NFA::new_many(&["^(a)$", "^(b)$"]).unwrap(),
        ] {
            assert_eq!(
                OnePassConstructionCertificate::from_nfa(&nfa),
                Err(ConstructionError::Scope)
            );
        }
    }
}

//! Source-scoped suffix extraction scratch. Retained Memchr storage is separate.
//! Audited 0.8.12 only derives the existing Prefix default; literal/trie records
//! and extraction operations are unchanged from 0.8.11.

use crate::schema_heap_layout::{AllocationEnvelope, HeapLayout, LayoutError, NativeField};
use std::collections::BTreeMap;

pub(super) const PATTERN: &str = r"\.[a-zA-Z]{2,3}$";
pub(super) const KEYS: [&str; 5] = [
    "regex_syntax::hir::literal::Literal",
    "regex_syntax::hir::literal::State",
    "core::option::Option<core::num::nonzero::NonZeroUsize>",
    "(u8, usize)",
    "compiler_abi::Pointer",
];

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Scratch {
    pub cross: usize,
    pub repeat: usize,
    pub union: usize,
    pub trie: usize,
    pub peak: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    Missing(&'static str),
    Layout,
    Arithmetic,
    Scope,
}

impl From<LayoutError> for Refusal {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Shape => Self::Layout,
            LayoutError::Arithmetic => Self::Arithmetic,
        }
    }
}

fn add(terms: &[usize]) -> Result<usize, Refusal> {
    terms.iter().try_fold(0_usize, |sum, term| {
        sum.checked_add(*term).ok_or(Refusal::Arithmetic)
    })
}

fn multiply(value: usize, count: usize) -> Result<usize, Refusal> {
    value.checked_mul(count).ok_or(Refusal::Arithmetic)
}

fn request(field: NativeField, bytes: usize, word: usize) -> Result<usize, Refusal> {
    let maximum = match word {
        4 => usize::try_from(i32::MAX).map_err(|_error| Refusal::Arithmetic)?,
        8 => usize::try_from(i64::MAX).map_err(|_error| Refusal::Arithmetic)?,
        _ => return Err(Refusal::Layout),
    };
    let padding = field
        .alignment
        .checked_sub(1_usize)
        .ok_or(Refusal::Arithmetic)?;
    if bytes > maximum.checked_sub(padding).ok_or(Refusal::Arithmetic)? {
        return Err(Refusal::Arithmetic);
    }
    Ok(bytes)
}

fn append(
    heap: HeapLayout,
    field: NativeField,
    count: usize,
) -> Result<AllocationEnvelope, Refusal> {
    let allocation = heap.append_vector(field.bytes, count)?;
    request(field, allocation.retained_bytes, heap.word_bytes())?;
    request(field, allocation.old_growth_bytes, heap.word_bytes())?;
    Ok(allocation)
}

pub(super) fn suffix(
    pattern: &str,
    facts: &BTreeMap<String, NativeField>,
) -> Result<Scratch, Refusal> {
    if pattern != PATTERN {
        return Err(Refusal::Scope);
    }
    let mut fields = Vec::new();
    for key in KEYS {
        let field = *facts.get(key).ok_or(Refusal::Missing(key))?;
        field.validate()?;
        fields.push(field);
    }
    let [literal, state, match_slot, transition, pointer] = fields.as_slice() else {
        return Err(Refusal::Layout);
    };
    if pointer.alignment != pointer.bytes {
        return Err(Refusal::Layout);
    }
    // Only vector operations are used; the hash-group parameter is irrelevant.
    let heap = HeapLayout::new(pointer.bytes, 16)?;
    let exact = request(
        *literal,
        heap.repeat_vector(literal.bytes, 1)?,
        pointer.bytes,
    )?;
    let byte = heap.repeat_vector(1, 1)?;
    let union_backing = append(heap, *literal, 1)?.retained_bytes;
    let states = append(heap, *state, 2)?;
    let matches = append(heap, *match_slot, 2)?;
    let transitions = append(heap, *transition, 1)?;
    let cross = add(&[multiply(exact, 3)?, multiply(byte, 2)?])?;
    let repeat = add(&[multiply(exact, 2)?, byte])?;
    let union = add(&[exact, union_backing, byte])?;
    // Each Vec growth completes before the next push can start. Prefix
    // backing and all final trie buffers remain live across those calls.
    let trie = add(&[
        union_backing,
        byte,
        states.retained_bytes,
        matches.retained_bytes,
        transitions.retained_bytes,
        states
            .old_growth_bytes
            .max(matches.old_growth_bytes)
            .max(transitions.old_growth_bytes),
    ])?;
    Ok(Scratch {
        cross,
        repeat,
        union,
        trie,
        peak: cross.max(repeat).max(union).max(trie),
    })
}

#[cfg(test)]
mod tests {
    use super::{KEYS, PATTERN, Refusal, suffix};
    use crate::schema_heap_layout::NativeField;
    use regex_syntax::hir::literal::ExtractKind;
    use std::collections::BTreeMap;

    fn facts() -> BTreeMap<String, NativeField> {
        KEYS.into_iter()
            .zip([24, 16, 8, 16, 8])
            .map(|(key, bytes)| {
                (
                    key.to_owned(),
                    NativeField {
                        bytes,
                        alignment: 8,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn reviewed_literal_extraction_default_remains_prefix() {
        assert!(matches!(ExtractKind::default(), ExtractKind::Prefix));
    }

    #[test]
    fn suffix_scratch_pins_cross_repeat_union_and_preference_trie_separately() {
        let scratch = suffix(PATTERN, &facts()).unwrap();
        assert_eq!(scratch.cross, 74);
        assert_eq!(scratch.repeat, 49);
        assert_eq!(scratch.union, 121);
        assert_eq!(scratch.trie, 257);
        assert_eq!(scratch.peak, 257);
    }

    #[test]
    fn suffix_scratch_varies_each_record_layout_without_host_substitution() {
        for (index, bytes, alignment, peak) in [
            (0, 32, 8, 289),
            (1, 24, 8, 289),
            (2, 4, 4, 241),
            (3, 24, 8, 289),
            (4, 4, 4, 257),
        ] {
            let mut inputs = facts();
            assert!(
                inputs
                    .insert(KEYS[index].to_owned(), NativeField { bytes, alignment })
                    .is_some()
            );
            assert_eq!(
                suffix(PATTERN, &inputs).unwrap().peak,
                peak,
                "{}",
                KEYS[index]
            );
        }
    }

    #[test]
    fn suffix_scratch_refuses_each_missing_or_malformed_target_fact() {
        for key in KEYS {
            let mut inputs = facts();
            assert!(inputs.remove(key).is_some());
            assert_eq!(suffix(PATTERN, &inputs), Err(Refusal::Missing(key)));
            let mut malformed = facts();
            assert!(
                malformed
                    .insert(
                        key.to_owned(),
                        NativeField {
                            bytes: 0,
                            alignment: 8
                        }
                    )
                    .is_some()
            );
            assert_eq!(suffix(PATTERN, &malformed), Err(Refusal::Layout));
        }
    }

    #[test]
    fn suffix_scratch_rejects_unmodeled_patterns_and_target_request_overflow() {
        assert_eq!(suffix("[a-z]+", &facts()), Err(Refusal::Scope));
        let mut inputs = facts();
        inputs.insert(
            KEYS[4].to_owned(),
            NativeField {
                bytes: 4,
                alignment: 4,
            },
        );
        inputs.insert(
            KEYS[0].to_owned(),
            NativeField {
                bytes: 0x4000_0000,
                alignment: 8,
            },
        );
        assert_eq!(suffix(PATTERN, &inputs), Err(Refusal::Arithmetic));
    }
}

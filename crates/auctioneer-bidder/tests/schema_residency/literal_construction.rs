//! Source-scoped suffix extraction scratch. Retained Memchr storage is separate.

use crate::schema_heap_layout::NativeField;
use std::collections::BTreeMap;

const PATTERN: &str = r"\.[a-zA-Z]{2,3}$";
const KEYS: [&str; 5] = [
    "regex_syntax::hir::literal::Literal",
    "regex_syntax::hir::literal::State",
    "core::option::Option<core::num::nonzero::NonZeroUsize>",
    "(u8, usize)",
    "compiler_abi::Pointer",
];

#[derive(Debug, Eq, PartialEq)]
struct Scratch {
    cross: usize,
    repeat: usize,
    union: usize,
    trie: usize,
    peak: usize,
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Missing(&'static str),
    Layout,
    Arithmetic,
    Scope,
}

fn suffix(pattern: &str, facts: &BTreeMap<String, NativeField>) -> Result<Scratch, Refusal> {
    if pattern != PATTERN {
        return Err(Refusal::Scope);
    }
    for key in KEYS {
        facts.get(key).ok_or(Refusal::Missing(key))?.validate().map_err(|error| match error {
            crate::schema_heap_layout::LayoutError::Shape => Refusal::Layout,
            crate::schema_heap_layout::LayoutError::Arithmetic => Refusal::Arithmetic,
        })?;
    }
    Err(Refusal::Scope)
}

#[cfg(test)]
mod tests {
    use super::{KEYS, PATTERN, Refusal, suffix};
    use crate::schema_heap_layout::NativeField;
    use std::collections::BTreeMap;

    fn facts() -> BTreeMap<String, NativeField> {
        KEYS.into_iter().zip([24, 16, 8, 16, 8]).map(|(key, bytes)| {
            (key.to_owned(), NativeField { bytes, alignment: 8 })
        }).collect()
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
            (0, 32, 8, 289), (1, 24, 8, 289), (2, 4, 4, 241),
            (3, 24, 8, 289), (4, 4, 4, 257),
        ] {
            let mut inputs = facts();
            assert!(inputs.insert(KEYS[index].to_owned(), NativeField { bytes, alignment }).is_some());
            assert_eq!(suffix(PATTERN, &inputs).unwrap().peak, peak, "{}", KEYS[index]);
        }
    }

    #[test]
    fn suffix_scratch_refuses_each_missing_or_malformed_target_fact() {
        for key in KEYS {
            let mut inputs = facts();
            assert!(inputs.remove(key).is_some());
            assert_eq!(suffix(PATTERN, &inputs), Err(Refusal::Missing(key)));
            let mut inputs = facts();
            assert!(inputs.insert(key.to_owned(), NativeField { bytes: 0, alignment: 8 }).is_some());
            assert_eq!(suffix(PATTERN, &inputs), Err(Refusal::Layout));
        }
    }

    #[test]
    fn suffix_scratch_rejects_unmodeled_patterns_and_target_request_overflow() {
        assert_eq!(suffix("[a-z]+", &facts()), Err(Refusal::Scope));
        let mut inputs = facts();
        inputs.insert(KEYS[4].to_owned(), NativeField { bytes: 4, alignment: 4 });
        inputs.insert(KEYS[0].to_owned(), NativeField { bytes: 0x4000_0000, alignment: 8 });
        assert_eq!(suffix(PATTERN, &inputs), Err(Refusal::Arithmetic));
    }
}

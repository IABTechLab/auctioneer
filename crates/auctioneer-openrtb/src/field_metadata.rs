//! Compiler-checked storage facts for request-model disclosure validation.

use crate::types::bid_request::BidRequest;
use crate::types::field::JsonField;
use serde_json::value::RawValue;
use serde_json::{Map, Number, Value};
use std::any::TypeId;
use std::collections::BTreeMap;

/// Storage capability, independent of a field's disclosure policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    Typed,
    RawJson,
    JsonMap,
}

impl Storage {
    const fn wrapped(self) -> Self {
        match self {
            Self::RawJson => Self::RawJson,
            Self::Typed | Self::JsonMap => Self::Typed,
        }
    }
}

/// Explicit compiler-resolved storage facts. Unknown types have no implementation.
pub trait FieldMetadata: 'static {
    const STORAGE: Storage;
    const MODEL: Option<&'static Model> = None;
}

/// The actual request graph; no source-file inventory or model registry is needed.
pub const REQUEST: &Model = match <BidRequest as FieldMetadata>::MODEL {
    Some(model) => model,
    None => panic!("request model has no field metadata"),
};

/// A compiled model's potential wire fields, not an instance or privacy decision.
#[derive(Clone, Copy, Debug)]
pub struct Model {
    pub wire_name: &'static str,
    pub rust_name: &'static str,
    pub fields: &'static [Field],
    /// Compiler-local identity; not a persistent or cross-target identifier.
    pub declaration: TypeId,
}

impl Model {
    /// Construct a descriptor for use in a model's associated constant.
    ///
    /// # Panics
    /// Invalid or duplicate wire fields and unsupported catch-alls fail const evaluation.
    #[must_use]
    pub const fn new<T: 'static>(
        wire_name: &'static str,
        rust_name: &'static str,
        fields: &'static [Field],
    ) -> Self {
        assert!(valid_name(wire_name), "invalid metadata object name");
        assert!(!rust_name.is_empty(), "empty metadata Rust identity");
        let mut remaining = fields;
        let mut catch_all = false;
        while let Some((field, rest)) = remaining.split_first() {
            assert!(valid_name(field.name), "invalid metadata field name");
            if field.flattened {
                assert!(!catch_all, "multiple metadata catch-alls");
                assert!(
                    matches!(field.storage, Storage::JsonMap) && field.child.is_none(),
                    "flattened metadata requires a direct serde_json map"
                );
                catch_all = true;
            } else {
                let mut siblings = rest;
                while let Some((other, tail)) = siblings.split_first() {
                    assert!(
                        other.flattened || !text_eq(field.name, other.name),
                        "duplicate metadata wire field"
                    );
                    siblings = tail;
                }
            }
            remaining = rest;
        }
        Self {
            wire_name,
            rust_name,
            fields,
            declaration: TypeId::of::<T>(),
        }
    }

    /// Compare actual declarations, even when diagnostic source locations coincide.
    #[must_use]
    pub fn same_declaration(&self, other: &Self) -> bool {
        self.declaration == other.declaration
    }
}

/// A named slot or a direct flattened map, with compiler-resolved child metadata.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    pub name: &'static str,
    pub storage: Storage,
    pub flattened: bool,
    pub child: Option<&'static Model>,
}

impl Field {
    /// Read capabilities from the actual field type, including aliases.
    ///
    /// # Panics
    /// A flattened field that is not the direct JSON map fails const evaluation.
    #[must_use]
    pub const fn of<T: FieldMetadata + ?Sized>(name: &'static str, flattened: bool) -> Self {
        assert!(
            !flattened || matches!(T::STORAGE, Storage::JsonMap),
            "flattened metadata requires a direct serde_json map"
        );
        Self {
            name,
            storage: T::STORAGE,
            flattened,
            child: T::MODEL,
        }
    }
}

/// A wire-object name and diagnostic source label in a structural signature.
/// Compiler declaration identity is process-local and is not serialized here.
#[derive(Clone, Copy, Debug)]
pub struct Identity<'a> {
    pub wire_name: &'a str,
    pub rust_name: &'a str,
}

/// Flat expected field facts emitted by the build compiler.
#[derive(Clone, Copy, Debug)]
pub struct ExpectedField<'a> {
    pub name: &'a str,
    pub storage: Storage,
    pub flattened: bool,
    pub child: Option<Identity<'a>>,
}

/// One independently addressable model in a host/target agreement signature.
#[derive(Clone, Copy, Debug)]
pub struct ExpectedModel<'a> {
    pub wire_name: &'a str,
    pub rust_name: &'a str,
    pub fields: &'a [ExpectedField<'a>],
}

macro_rules! scalar_metadata {
    ($($ty:ty),* $(,)?) => {$ (
        impl FieldMetadata for $ty {
            const STORAGE: Storage = Storage::Typed;
        }
    )*};
}

scalar_metadata!(
    (),
    bool,
    char,
    str,
    String,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    f32,
    f64,
    Number,
    Value,
);

impl FieldMetadata for RawValue {
    const STORAGE: Storage = Storage::RawJson;
}

impl FieldMetadata for Map<String, Value> {
    const STORAGE: Storage = Storage::JsonMap;
}

macro_rules! wrapper_metadata {
    ($($wrapper:ident),* $(,)?) => {$ (
        impl<T: FieldMetadata> FieldMetadata for $wrapper<T> {
            const STORAGE: Storage = T::STORAGE.wrapped();
            const MODEL: Option<&'static Model> = T::MODEL;
        }
    )*};
}

wrapper_metadata!(Option, Vec, JsonField);

impl<T: FieldMetadata + ?Sized> FieldMetadata for Box<T> {
    const STORAGE: Storage = T::STORAGE.wrapped();
    const MODEL: Option<&'static Model> = T::MODEL;
}

impl<T: FieldMetadata> FieldMetadata for BTreeMap<String, T> {
    const STORAGE: Storage = T::STORAGE.wrapped();
    const MODEL: Option<&'static Model> = T::MODEL;
}

/// Compare the complete compiled graph with the build host's structural signature.
/// The signature is a consistency check, not a privacy or memory oracle.
#[must_use]
pub const fn graph_matches(root: &Model, expected: &[ExpectedModel<'_>]) -> bool {
    if !unique_expected(expected) || !matches_model(root, expected, expected) {
        return false;
    }
    let mut remaining = expected;
    while let Some((model, rest)) = remaining.split_first() {
        if !reachable(root, model, expected) {
            return false;
        }
        remaining = rest;
    }
    true
}

const fn unique_expected(expected: &[ExpectedModel<'_>]) -> bool {
    let mut remaining = expected;
    while let Some((model, rest)) = remaining.split_first() {
        let mut siblings = rest;
        while let Some((other, tail)) = siblings.split_first() {
            if text_eq(model.wire_name, other.wire_name) {
                return false;
            }
            siblings = tail;
        }
        remaining = rest;
    }
    true
}

const fn matches_model(
    actual: &Model,
    expected: &[ExpectedModel<'_>],
    depth: &[ExpectedModel<'_>],
) -> bool {
    // An acyclic graph cannot be deeper than its distinct-model population.
    let Some((_, next_depth)) = depth.split_first() else {
        return false;
    };
    let Some(model) = find_expected(actual, expected) else {
        return false;
    };
    let mut fields = actual.fields;
    let mut wanted = model.fields;
    loop {
        match (fields.split_first(), wanted.split_first()) {
            (None, None) => return true,
            (Some((field, rest)), Some((other, tail))) => {
                if !matches_field(field, other) {
                    return false;
                }
                if let Some(child) = field.child
                    && !matches_model(child, expected, next_depth)
                {
                    return false;
                }
                fields = rest;
                wanted = tail;
            }
            _ => return false,
        }
    }
}

const fn find_expected<'a>(
    model: &Model,
    expected: &'a [ExpectedModel<'a>],
) -> Option<&'a ExpectedModel<'a>> {
    let mut remaining = expected;
    while let Some((candidate, rest)) = remaining.split_first() {
        if text_eq(model.rust_name, candidate.rust_name)
            && text_eq(model.wire_name, candidate.wire_name)
        {
            return Some(candidate);
        }
        remaining = rest;
    }
    None
}

const fn matches_field(actual: &Field, expected: &ExpectedField<'_>) -> bool {
    if !text_eq(actual.name, expected.name) || actual.flattened != expected.flattened {
        return false;
    }
    if !matches!(
        (actual.storage, expected.storage),
        (Storage::Typed, Storage::Typed)
            | (Storage::RawJson, Storage::RawJson)
            | (Storage::JsonMap, Storage::JsonMap)
    ) {
        return false;
    }
    match (actual.child, expected.child) {
        (None, None) => true,
        (Some(child), Some(identity)) => {
            text_eq(child.wire_name, identity.wire_name)
                && text_eq(child.rust_name, identity.rust_name)
        }
        _ => false,
    }
}

const fn reachable(
    actual: &Model,
    expected: &ExpectedModel<'_>,
    depth: &[ExpectedModel<'_>],
) -> bool {
    let Some((_, next_depth)) = depth.split_first() else {
        return false;
    };
    if text_eq(actual.rust_name, expected.rust_name)
        && text_eq(actual.wire_name, expected.wire_name)
    {
        return true;
    }
    let mut fields = actual.fields;
    while let Some((field, rest)) = fields.split_first() {
        if let Some(child) = field.child
            && reachable(child, expected, next_depth)
        {
            return true;
        }
        fields = rest;
    }
    false
}

const fn text_eq(left: &str, right: &str) -> bool {
    let mut left_bytes = left.as_bytes();
    let mut right_bytes = right.as_bytes();
    loop {
        match (left_bytes.split_first(), right_bytes.split_first()) {
            (None, None) => return true,
            (Some((first, rest)), Some((other, tail))) if *first == *other => {
                left_bytes = rest;
                right_bytes = tail;
            }
            _ => return false,
        }
    }
}

const fn valid_name(name: &str) -> bool {
    let Some((first, rest)) = name.as_bytes().split_first() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && *first != b'_' {
        return false;
    }
    let mut non_underscore = *first != b'_';
    let mut remaining = rest;
    while let Some((byte, tail)) = remaining.split_first() {
        if !byte.is_ascii_alphanumeric() && *byte != b'_' {
            return false;
        }
        non_underscore |= *byte != b'_';
        remaining = tail;
    }
    non_underscore
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::field::JsonField;
    use serde_json::value::RawValue;
    use serde_json::{Map, Value};
    use std::panic::catch_unwind;

    struct Child;

    impl FieldMetadata for Child {
        const STORAGE: Storage = Storage::Typed;
        const MODEL: Option<&'static Model> = Some(&Model::new::<Self>(
            "Child",
            "fake::Child",
            &[Field::of::<Box<RawValue>>("ext", false)],
        ));
    }

    struct Root;

    const ROOT: &Model = &Model::new::<Root>(
        "Root",
        "fake::Root",
        &[
            Field::of::<Option<Child>>("child", false),
            Field::of::<Map<String, Value>>("other", true),
        ],
    );
    const CHILD_FIELDS: &[ExpectedField<'static>] = &[ExpectedField {
        name: "ext",
        storage: Storage::RawJson,
        flattened: false,
        child: None,
    }];
    const ROOT_FIELDS: &[ExpectedField<'static>] = &[
        ExpectedField {
            name: "child",
            storage: Storage::Typed,
            flattened: false,
            child: Some(Identity {
                wire_name: "Child",
                rust_name: "fake::Child",
            }),
        },
        ExpectedField {
            name: "other",
            storage: Storage::JsonMap,
            flattened: true,
            child: None,
        },
    ];
    const EXPECTED: &[ExpectedModel<'static>] = &[
        ExpectedModel {
            wire_name: "Child",
            rust_name: "fake::Child",
            fields: CHILD_FIELDS,
        },
        ExpectedModel {
            wire_name: "Root",
            rust_name: "fake::Root",
            fields: ROOT_FIELDS,
        },
    ];
    const _: () = assert!(graph_matches(ROOT, EXPECTED));

    #[test]
    fn declaration_equality_uses_compiler_identity_not_diagnostic_location() {
        macro_rules! declarations {
            () => {{
                fn first() -> &'static Model {
                    struct Slot;
                    const MODEL: Model = Model::new::<Slot>("Slot", "same expansion location", &[]);
                    &MODEL
                }
                fn second() -> &'static Model {
                    struct Slot;
                    const MODEL: Model = Model::new::<Slot>("Slot", "same expansion location", &[]);
                    &MODEL
                }
                (first(), second())
            }};
        }
        let (first, second) = declarations!();
        assert!(first.same_declaration(first));
        assert_eq!(first.rust_name, second.rust_name);
        assert!(!first.same_declaration(second));
    }

    #[test]
    fn different_wire_models_may_share_a_diagnostic_source_label() {
        struct First;
        struct Second;
        const FIRST: &Model = &Model::new::<First>("First", "same source label", &[]);
        const SECOND: &Model = &Model::new::<Second>("Second", "same source label", &[]);
        const ROOT: &Model = &Model::new::<Root>(
            "Root",
            "root",
            &[
                Field {
                    name: "first",
                    storage: Storage::Typed,
                    flattened: false,
                    child: Some(FIRST),
                },
                Field {
                    name: "second",
                    storage: Storage::Typed,
                    flattened: false,
                    child: Some(SECOND),
                },
            ],
        );
        const EXPECTED: &[ExpectedModel<'static>] = &[
            ExpectedModel {
                wire_name: "Root",
                rust_name: "root",
                fields: &[
                    ExpectedField {
                        name: "first",
                        storage: Storage::Typed,
                        flattened: false,
                        child: Some(Identity {
                            wire_name: "First",
                            rust_name: "same source label",
                        }),
                    },
                    ExpectedField {
                        name: "second",
                        storage: Storage::Typed,
                        flattened: false,
                        child: Some(Identity {
                            wire_name: "Second",
                            rust_name: "same source label",
                        }),
                    },
                ],
            },
            ExpectedModel {
                wire_name: "First",
                rust_name: "same source label",
                fields: &[],
            },
            ExpectedModel {
                wire_name: "Second",
                rust_name: "same source label",
                fields: &[],
            },
        ];
        assert!(graph_matches(ROOT, EXPECTED));
    }

    #[test]
    fn raw_storage_propagates_through_wrappers_without_tainting_parent_models() {
        assert_eq!(<RawValue as FieldMetadata>::STORAGE, Storage::RawJson);
        assert_eq!(
            <Option<JsonField<Box<RawValue>>> as FieldMetadata>::STORAGE,
            Storage::RawJson
        );
        assert_eq!(
            <Vec<Box<RawValue>> as FieldMetadata>::STORAGE,
            Storage::RawJson
        );
        assert_eq!(<Option<Child> as FieldMetadata>::STORAGE, Storage::Typed);
        assert_eq!(<Value as FieldMetadata>::STORAGE, Storage::Typed);
    }

    #[test]
    fn only_the_direct_json_map_carries_catch_all_capability() {
        type Alias = Map<String, Value>;
        assert_eq!(<Alias as FieldMetadata>::STORAGE, Storage::JsonMap);
        assert_eq!(<Box<Alias> as FieldMetadata>::STORAGE, Storage::Typed);
        assert_eq!(<Option<Alias> as FieldMetadata>::STORAGE, Storage::Typed);
        assert_eq!(
            <BTreeMap<String, Value> as FieldMetadata>::STORAGE,
            Storage::Typed
        );
        assert!(!Field::of::<Alias>("named", false).flattened);
    }

    #[test]
    fn structural_agreement_requires_the_complete_independent_graph() {
        assert_eq!(EXPECTED.len(), 2);
        assert!(graph_matches(ROOT, EXPECTED));
        assert!(!graph_matches(ROOT, &EXPECTED[..1]));
        assert!(!graph_matches(ROOT, &[EXPECTED[1]]));
        assert!(!graph_matches(
            ROOT,
            &[EXPECTED[0], EXPECTED[1], EXPECTED[1]]
        ));
        let extra = ExpectedModel {
            wire_name: "Unreachable",
            rust_name: "fake::Unreachable",
            fields: &[],
        };
        assert!(!graph_matches(ROOT, &[EXPECTED[0], EXPECTED[1], extra]));
    }

    #[test]
    fn structural_agreement_binds_each_storage_name_edge_and_catch_all_fact() {
        assert_eq!(ROOT_FIELDS.len(), 2);
        assert_eq!(EXPECTED.len(), 2);
        for modified in [
            ExpectedField {
                storage: Storage::RawJson,
                ..ROOT_FIELDS[0]
            },
            ExpectedField {
                name: "renamed",
                ..ROOT_FIELDS[0]
            },
            ExpectedField {
                flattened: true,
                ..ROOT_FIELDS[0]
            },
            ExpectedField {
                child: None,
                ..ROOT_FIELDS[0]
            },
            ExpectedField {
                child: Some(Identity {
                    wire_name: "Child",
                    rust_name: "fake::Different",
                }),
                ..ROOT_FIELDS[0]
            },
            ExpectedField {
                child: Some(Identity {
                    wire_name: "Different",
                    rust_name: "fake::Child",
                }),
                ..ROOT_FIELDS[0]
            },
        ] {
            let fields = [modified, ROOT_FIELDS[1]];
            let models = [
                EXPECTED[0],
                ExpectedModel {
                    fields: &fields,
                    ..EXPECTED[1]
                },
            ];
            assert!(!graph_matches(ROOT, &models));
        }
        let fields = [ExpectedField {
            storage: Storage::Typed,
            ..CHILD_FIELDS[0]
        }];
        let models = [
            ExpectedModel {
                fields: &fields,
                ..EXPECTED[0]
            },
            EXPECTED[1],
        ];
        assert!(!graph_matches(ROOT, &models));
    }

    #[test]
    #[should_panic(expected = "flattened metadata requires a direct serde_json map")]
    fn wrapped_json_maps_cannot_be_flattened() {
        let _invalid = Field::of::<Option<Map<String, Value>>>("other", true);
    }

    #[test]
    #[should_panic(expected = "duplicate metadata wire field")]
    fn duplicate_named_wire_fields_are_rejected() {
        const FIELDS: &[Field] = &[
            Field::of::<String>("id", false),
            Field::of::<String>("id", false),
        ];
        let _invalid = Model::new::<Root>("Duplicate", "fake::Duplicate", FIELDS);
    }

    #[test]
    #[should_panic(expected = "multiple metadata catch-alls")]
    fn multiple_catch_alls_are_rejected() {
        const FIELDS: &[Field] = &[
            Field::of::<Map<String, Value>>("first", true),
            Field::of::<Map<String, Value>>("second", true),
        ];
        let _invalid = Model::new::<Root>("Duplicate", "fake::Duplicate", FIELDS);
    }

    #[test]
    fn invalid_metadata_names_fail_without_a_permissive_fallback() {
        for name in ["", "___", "9field", "a.b", "a/b", "a\0b", "bad-name"] {
            let panic =
                catch_unwind(|| Model::new::<Root>(name, "fake::Invalid", &[])).unwrap_err();
            assert_eq!(
                panic.downcast_ref::<&str>().copied(),
                Some("invalid metadata object name")
            );
        }
    }

    #[test]
    fn cycles_refuse_with_the_structural_population_bound() {
        static CYCLE: Model = Model {
            wire_name: "Cycle",
            rust_name: "fake::Cycle",
            declaration: TypeId::of::<Root>(),
            fields: &[Field {
                name: "again",
                storage: Storage::Typed,
                flattened: false,
                child: Some(&CYCLE),
            }],
        };
        const CYCLE_FIELDS: &[ExpectedField<'static>] = &[ExpectedField {
            name: "again",
            storage: Storage::Typed,
            flattened: false,
            child: Some(Identity {
                wire_name: "Cycle",
                rust_name: "fake::Cycle",
            }),
        }];
        const CYCLE_EXPECTED: &[ExpectedModel<'static>] = &[ExpectedModel {
            wire_name: "Cycle",
            rust_name: "fake::Cycle",
            fields: CYCLE_FIELDS,
        }];
        const _: () = assert!(!graph_matches(&CYCLE, CYCLE_EXPECTED));
        assert!(!graph_matches(&CYCLE, CYCLE_EXPECTED));
    }
}

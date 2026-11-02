//! Integer-only observation ABI for the unknown-unknown compiler probe.

use super::RawVerdict;
use crate::schema_reference::Case;
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::BTreeSet;

pub struct SemanticInput<'a> {
    pub schema: &'a str,
    pub input_json: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub enum LayoutField {
    Word,
    CacheBytes,
    CacheAlignment,
    WrapperBytes,
    WrapperAlignment,
}

#[derive(Clone, Copy, Debug)]
pub enum Call {
    Version,
    SchemaCount,
    CaseCount(u32),
    Replay {
        schema: u32,
        start: u32,
        reuse: bool,
    },
    Initialize(u32),
    Empty(u32),
    Layout {
        schema: u32,
        field: LayoutField,
    },
    SemanticCount(u32),
    SemanticReplay {
        schema: u32,
        start: u32,
        reuse: bool,
    },
}

impl Call {
    pub fn invocation(self) -> (&'static str, Vec<String>) {
        // WASM has an i32 word, not a signed/unsigned parameter distinction.
        // Wasmtime's CLI accepts signed decimal, preserving these u32 bits.
        let argument = |value: u32| value.cast_signed().to_string();
        match self {
            Self::Version => ("auctioneer_probe_version", vec![]),
            Self::SchemaCount => ("auctioneer_probe_schemas", vec![]),
            Self::CaseCount(schema) => ("auctioneer_probe_cases", vec![argument(schema)]),
            Self::Replay {
                schema,
                start,
                reuse,
            } => (
                "auctioneer_probe_replay",
                vec![
                    argument(schema),
                    argument(start),
                    u32::from(reuse).to_string(),
                ],
            ),
            Self::Initialize(schema) => ("auctioneer_probe_initialize", vec![argument(schema)]),
            Self::Empty(schema) => ("auctioneer_probe_empty", vec![argument(schema)]),
            Self::Layout { schema, field } => {
                let index = match field {
                    LayoutField::Word => 0_u32,
                    LayoutField::CacheBytes => 1,
                    LayoutField::CacheAlignment => 2,
                    LayoutField::WrapperBytes => 3,
                    LayoutField::WrapperAlignment => 4,
                };
                (
                    "auctioneer_probe_layout",
                    vec![argument(schema), argument(index)],
                )
            }
            Self::SemanticCount(schema) => {
                ("auctioneer_probe_semantic_cases", vec![argument(schema)])
            }
            Self::SemanticReplay {
                schema,
                start,
                reuse,
            } => (
                "auctioneer_probe_semantic_replay",
                vec![
                    argument(schema),
                    argument(start),
                    u32::from(reuse).to_string(),
                ],
            ),
        }
    }
}

pub fn captured_data(names: &[&str], cases: &[Case]) -> Result<String, String> {
    let mut unique = BTreeSet::new();
    if names.is_empty()
        || names
            .iter()
            .any(|name| name.is_empty() || !unique.insert(*name))
    {
        return Err("invalid bare capture membership".to_owned());
    }
    let mut rows = Vec::new();
    for name in names {
        let inputs: Vec<_> = cases
            .iter()
            .filter(|case| case.schema == *name)
            .map(|case| case.input.as_str())
            .collect();
        if inputs.iter().any(|input| input.len() > 0x0001_0000) {
            return Err("bare captured input limit".to_owned());
        }
        rows.push((*name, inputs));
    }
    serde_json::to_string(&rows).map_err(|_error| "bare capture encoding refused".to_owned())
}

pub fn semantic_data(names: &[&str], rows: &[SemanticInput<'_>]) -> Result<String, String> {
    let mut unique = BTreeSet::new();
    if names.is_empty()
        || names
            .iter()
            .any(|name| name.is_empty() || !unique.insert(*name))
    {
        return Err("invalid bare semantic membership".to_owned());
    }
    for row in rows {
        if !unique.contains(row.schema) {
            return Err("unselected bare semantic schema".to_owned());
        }
        if row.input_json.len() > 0x0001_0000 {
            return Err("bare semantic input limit".to_owned());
        }
    }
    let table = names
        .iter()
        .map(|name| {
            let inputs = rows
                .iter()
                .filter(|row| row.schema == *name)
                .map(|row| row.input_json)
                .collect::<Vec<_>>();
            (*name, inputs)
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&table).map_err(|_error| "bare semantic encoding refused".to_owned())
}

pub fn decode(packet: u64, count: usize) -> Result<Vec<RawVerdict>, String> {
    if count == 0 || count > 16 || packet > u64::from(u32::MAX) {
        return Err("invalid bare result packet".to_owned());
    }
    let mut rows = Vec::with_capacity(count);
    for index in 0_usize..16 {
        let bits = index.checked_mul(2).ok_or("invalid bare result packet")?;
        let shift = u32::try_from(bits).map_err(|_error| "invalid bare result packet")?;
        let code = (packet >> shift) & 3;
        if index >= count {
            if code != 3 {
                return Err("invalid bare result packet".to_owned());
            }
            continue;
        }
        rows.push(match code {
            0 => RawVerdict::Schema { valid: false },
            1 => RawVerdict::Schema { valid: true },
            2 => RawVerdict::InvalidJson,
            _ => return Err("invalid bare result packet".to_owned()),
        });
    }
    Ok(rows)
}

pub fn verify_data(bytes: &[u8], data: &str) -> Result<(), String> {
    if data.is_empty() {
        return Err("bare captured inputs missing from linked data".to_owned());
    }
    let mut found = false;
    for parsed_payload in wasmparser::Parser::new(0).parse_all(bytes) {
        let payload = parsed_payload.map_err(|_error| "invalid bare module data")?;
        if let wasmparser::Payload::DataSection(segments) = payload {
            for parsed_segment in segments {
                let segment = parsed_segment.map_err(|_error| "invalid bare module data")?;
                found |= segment
                    .data
                    .windows(data.len())
                    .any(|window| window == data.as_bytes());
            }
        }
    }
    if !found {
        return Err("bare captured inputs missing from linked data".to_owned());
    }
    Ok(())
}

pub fn trapped_imports(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut imports = BTreeSet::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::ImportSection(section) =
            payload.map_err(|_error| "invalid bare module imports")?
        {
            for parsed_import in section.into_imports() {
                let import = parsed_import.map_err(|_error| "invalid bare module imports")?;
                let allowed = matches!(
                    (import.module, import.name),
                    ("__wbindgen_placeholder__", "__wbindgen_describe")
                        | (
                            "__wbindgen_externref_xform__",
                            "__wbindgen_externref_table_set_null"
                                | "__wbindgen_externref_table_grow"
                        )
                );
                if !allowed
                    || !matches!(import.ty, wasmparser::TypeRef::Func(_))
                    || !imports.insert((import.module.to_owned(), import.name.to_owned()))
                {
                    return Err("unreviewed bare probe import".to_owned());
                }
            }
        }
    }
    Ok(imports.into_iter().collect())
}

pub fn check_header(version: u64, count: u64, expected: usize) -> Result<(), String> {
    if version != 1 {
        return Err("bare ABI version mismatch".to_owned());
    }
    if expected == 0 || u64::try_from(expected).ok() != Some(count) {
        return Err("bare schema count mismatch".to_owned());
    }
    Ok(())
}

pub fn source(data: &str) -> TokenStream {
    let literal = super::literal(data);
    quote! {
        const BARE_CASES: &str = #literal;

        fn bare_inputs(index: u32) -> Option<Vec<String>> {
            let ordinal = usize::try_from(index).ok()?;
            let rows: Vec<(String, Vec<String>)> = serde_json::from_str(BARE_CASES).ok()?;
            let row = rows.into_iter().nth(ordinal)?;
            let (name, _, _) = VALIDATORS.get(ordinal)?;
            if row.0 != *name { return None; }
            Some(row.1)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_version() -> u64 { 1 }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_schemas() -> u64 {
            u64::try_from(VALIDATORS.len()).unwrap_or(u64::MAX)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_cases(index: u32) -> u64 {
            bare_inputs(index).and_then(|rows| u64::try_from(rows.len()).ok()).unwrap_or(u64::MAX)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_replay(index: u32, start: u32, reuse: u32) -> u64 {
            if reuse > 1 { return u64::MAX; }
            let Ok(ordinal) = usize::try_from(index) else { return u64::MAX; };
            let Some((_, factory, validate)) = VALIDATORS.get(ordinal) else { return u64::MAX; };
            let Some(rows) = bare_inputs(index) else { return u64::MAX; };
            schema_replay::replay(&rows, start, reuse == 1, *factory, *validate)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_initialize(index: u32) -> u64 {
            let Ok(ordinal) = usize::try_from(index) else { return u64::MAX; };
            let Some((name, initialize)) = INITIALIZERS.get(ordinal) else { return u64::MAX; };
            let Some((expected, _, _)) = VALIDATORS.get(ordinal) else { return u64::MAX; };
            if name != expected { return u64::MAX; }
            u64::from(initialize()) | (u64::from(initialize()) << 1)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_empty(index: u32) -> u64 {
            let Ok(ordinal) = usize::try_from(index) else { return u64::MAX; };
            let Some((_, factory, validate)) = VALIDATORS.get(ordinal) else { return u64::MAX; };
            let Some(mut owner) = factory() else { return u64::MAX; };
            u64::from(validate(&serde_json::json!({}), &mut owner))
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_layout(index: u32, field: u32) -> u64 {
            let Ok(ordinal) = usize::try_from(index) else { return u64::MAX; };
            let Some((_, bytes, alignment)) = CACHE_LAYOUTS.get(ordinal) else { return u64::MAX; };
            let value = match field {
                0 => std::mem::size_of::<usize>(),
                1 => std::mem::size_of::<regex_automata::meta::Cache>(),
                2 => std::mem::align_of::<regex_automata::meta::Cache>(),
                3 => *bytes,
                4 => *alignment,
                _ => return u64::MAX,
            };
            u64::try_from(value).unwrap_or(u64::MAX)
        }
    }
}

pub fn semantic_source(data: &str) -> TokenStream {
    let literal = super::literal(data);
    quote! {
        const BARE_SEMANTIC_CASES: &str = #literal;

        fn bare_semantic_inputs(index: u32) -> Option<Vec<String>> {
            let ordinal = usize::try_from(index).ok()?;
            let rows: Vec<(String, Vec<String>)> = serde_json::from_str(BARE_SEMANTIC_CASES).ok()?;
            let row = rows.into_iter().nth(ordinal)?;
            let (name, _, _) = VALIDATORS.get(ordinal)?;
            if row.0 != *name { return None; }
            Some(row.1)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_semantic_cases(index: u32) -> u64 {
            bare_semantic_inputs(index)
                .and_then(|rows| u64::try_from(rows.len()).ok()).unwrap_or(u64::MAX)
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn auctioneer_probe_semantic_replay(index: u32, start: u32, reuse: u32) -> u64 {
            if reuse > 1 { return u64::MAX; }
            let Ok(ordinal) = usize::try_from(index) else { return u64::MAX; };
            let Some((_, factory, validate)) = VALIDATORS.get(ordinal) else { return u64::MAX; };
            let Some(rows) = bare_semantic_inputs(index) else { return u64::MAX; };
            schema_replay::replay(&rows, start, reuse == 1, *factory, *validate)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Call, LayoutField, SemanticInput, captured_data, check_header, decode, semantic_data,
        semantic_source, source, trapped_imports, verify_data,
    };
    use crate::schema_build::RawVerdict;
    use crate::schema_reference::{Case, Group};
    use quote::quote;
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::iter::repeat_with;
    use std::slice;

    fn fake_case(name: &str, input: &str) -> Case {
        Case {
            schema: name.to_owned(),
            group: Group::Invalid,
            index: 0,
            input: input.to_owned(),
            valid: false,
            source_offset: 0,
        }
    }

    #[test]
    fn bare_table_retains_raw_lexical_inputs_malformed_rows_and_empty_schema_members() {
        let cases = [
            fake_case("fake_b", "["),
            fake_case("fake_a", "{\"x\":1, \"x\":2}"),
            fake_case("fake_b", "null"),
            fake_case("fake_unselected", "true"),
        ];
        let data = captured_data(&["fake_a", "fake_b", "fake_empty"], &cases).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&data).unwrap(),
            json!([
                ["fake_a", ["{\"x\":1, \"x\":2}"]],
                ["fake_b", ["[", "null"]],
                ["fake_empty", []]
            ])
        );
        for names in [vec![], vec![""], vec!["fake_a", "fake_a"]] {
            assert_eq!(
                captured_data(&names, &cases).unwrap_err(),
                "invalid bare capture membership"
            );
        }
        captured_data(
            &["fake_a"],
            &[fake_case("fake_a", &"x".repeat(0x0001_0000))],
        )
        .unwrap();
        assert_eq!(
            captured_data(
                &["fake_a"],
                &[fake_case("fake_a", &"x".repeat(0x0001_0001))]
            )
            .unwrap_err(),
            "bare captured input limit"
        );
    }

    #[test]
    fn bare_packets_decode_independently_fixed_values_and_refuse_missing_or_extra_slots() {
        assert_eq!(
            decode(0xffff_ffe1, 3).unwrap(),
            vec![
                RawVerdict::Schema { valid: true },
                RawVerdict::Schema { valid: false },
                RawVerdict::InvalidJson
            ]
        );
        assert_eq!(
            decode(0x5555_5555, 16).unwrap(),
            repeat_with(|| RawVerdict::Schema { valid: true })
                .take(16)
                .collect::<Vec<_>>()
        );
        for (packet, count) in [
            (u64::MAX, 1_usize),
            (0xffff_ffff, 1),
            (0xffff_ffe1, 2),
            (0xffff_ffe1, 4),
            (0, 1),
            (0xffff_fffd, 0),
            (0, 17),
            (0x1_ffff_fffd, 1),
        ] {
            assert_eq!(
                decode(packet, count).unwrap_err(),
                "invalid bare result packet"
            );
        }
    }

    #[test]
    fn bare_input_identity_requires_linked_data_not_an_unrelated_custom_section() {
        // Minimal passive data segment, with no code or active memory access.
        let bytes = [0_u8, 97, 115, 109, 1, 0, 0, 0, 11, 6, 1, 1, 3, 97, 98, 99];
        assert_eq!(verify_data(&bytes, "abc"), Ok(()));
        assert_eq!(verify_data(&bytes, "ab"), Ok(()));
        for data in ["", "abd", "abcd"] {
            assert_eq!(
                verify_data(&bytes, data).unwrap_err(),
                "bare captured inputs missing from linked data"
            );
        }
        let custom = [0_u8, 97, 115, 109, 1, 0, 0, 0, 0, 5, 1, 120, 97, 98, 99];
        assert_eq!(
            verify_data(&custom, "abc").unwrap_err(),
            "bare captured inputs missing from linked data"
        );
        verify_data(&bytes[..12], "abc").unwrap_err();
    }

    #[test]
    fn bare_header_refuses_version_and_membership_drift_independently() {
        check_header(1, 3, 3).unwrap();
        for version in [0_u64, 2, u64::MAX] {
            assert_eq!(
                check_header(version, 3, 3).unwrap_err(),
                "bare ABI version mismatch"
            );
        }
        for (count, expected) in [(0_u64, 3_usize), (2, 3), (4, 3), (3, 2), (0, 0)] {
            assert_eq!(
                check_header(1, count, expected).unwrap_err(),
                "bare schema count mismatch"
            );
        }
    }

    fn import_module(imports: &[(&str, &str, u8)]) -> Vec<u8> {
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        bytes.extend([1, 4, 1, 0x60, 0, 0]);
        let mut section = vec![u8::try_from(imports.len()).unwrap()];
        for (module, name, kind) in imports {
            for word in [module, name] {
                section.push(u8::try_from(word.len()).unwrap());
                section.extend(word.as_bytes());
            }
            section.extend([*kind, 0]);
            if *kind == 2 {
                section.push(0);
            }
        }
        let length = u8::try_from(section.len()).unwrap();
        bytes.extend([2, length]);
        // These fixed fixtures fit in 255 bytes; only duplicate long import
        // names require the second LEB128 length byte.
        if length >= 128 {
            bytes.push(1);
        }
        bytes.extend(section);
        bytes
    }

    #[test]
    fn bare_import_policy_only_admits_the_three_trapping_binding_functions() {
        for (module, name) in [
            ("__wbindgen_placeholder__", "__wbindgen_describe"),
            (
                "__wbindgen_externref_xform__",
                "__wbindgen_externref_table_set_null",
            ),
            (
                "__wbindgen_externref_xform__",
                "__wbindgen_externref_table_grow",
            ),
        ] {
            assert_eq!(
                trapped_imports(&import_module(&[(module, name, 0)])).unwrap(),
                [(module.to_owned(), name.to_owned())]
            );
            for imports in [
                vec![("fake_provider", name, 0)],
                vec![(module, "fake_import", 0)],
                vec![(module, name, 2)],
                vec![(module, name, 0), (module, name, 0)],
            ] {
                assert_eq!(
                    trapped_imports(&import_module(&imports)).unwrap_err(),
                    "unreviewed bare probe import"
                );
            }
        }
        assert!(trapped_imports(b"\0asm\x01\0\0\0").unwrap().is_empty());
        trapped_imports(b"fake_invalid_wasm").unwrap_err();
    }

    #[test]
    fn bare_calls_only_construct_fixed_export_names_and_integer_arguments() {
        assert_eq!(
            Call::Version.invocation(),
            ("auctioneer_probe_version", vec![])
        );
        assert_eq!(
            Call::SchemaCount.invocation(),
            ("auctioneer_probe_schemas", vec![])
        );
        assert_eq!(
            Call::CaseCount(7).invocation(),
            ("auctioneer_probe_cases", vec!["7".to_owned()])
        );
        assert_eq!(
            Call::Initialize(2).invocation(),
            ("auctioneer_probe_initialize", vec!["2".to_owned()])
        );
        assert_eq!(
            Call::Empty(3).invocation(),
            ("auctioneer_probe_empty", vec!["3".to_owned()])
        );
        for reuse in [false, true] {
            assert_eq!(
                Call::Replay {
                    schema: 4,
                    start: 16,
                    reuse
                }
                .invocation(),
                (
                    "auctioneer_probe_replay",
                    vec![
                        "4".to_owned(),
                        "16".to_owned(),
                        if reuse { "1" } else { "0" }.to_owned()
                    ]
                )
            );
        }
        for (field, ordinal) in [
            (LayoutField::Word, "0"),
            (LayoutField::CacheBytes, "1"),
            (LayoutField::CacheAlignment, "2"),
            (LayoutField::WrapperBytes, "3"),
            (LayoutField::WrapperAlignment, "4"),
        ] {
            assert_eq!(
                Call::Layout { schema: 5, field }.invocation(),
                (
                    "auctioneer_probe_layout",
                    vec!["5".to_owned(), ordinal.to_owned()]
                )
            );
        }
    }

    #[test]
    fn bare_arguments_preserve_unsigned_bits_through_the_signed_wasm_cli() {
        for (value, expected) in [
            (0_u32, "0"),
            (0x7fff_ffff, "2147483647"),
            (0x8000_0000, "-2147483648"),
            (u32::MAX, "-1"),
        ] {
            assert_eq!(Call::CaseCount(value).invocation().1, [expected]);
            assert_eq!(Call::Initialize(value).invocation().1, [expected]);
            assert_eq!(Call::Empty(value).invocation().1, [expected]);
            assert_eq!(
                Call::Layout {
                    schema: value,
                    field: LayoutField::Word
                }
                .invocation()
                .1,
                [expected, "0"]
            );
            assert_eq!(
                Call::Replay {
                    schema: value,
                    start: value,
                    reuse: true
                }
                .invocation()
                .1,
                [expected, expected, "1"]
            );
        }
    }

    #[test]
    fn bare_generated_entries_have_only_integer_c_abi_arguments_and_results() {
        let text = source("[]").to_string();
        let syntax = syn::parse_file(&text).unwrap();
        let mut exports = Vec::new();
        for item in syntax.items {
            let syn::Item::Fn(function) = item else {
                continue;
            };
            if !function
                .sig
                .ident
                .to_string()
                .starts_with("auctioneer_probe_")
            {
                continue;
            }
            assert_eq!(function.sig.abi.unwrap().name.unwrap().value(), "C");
            for parameter in function.sig.inputs {
                let syn::FnArg::Typed(argument) = parameter else {
                    panic!("integer ABI only");
                };
                let syn::Type::Path(ty) = *argument.ty else {
                    panic!("no references or pointers");
                };
                assert!(ty.path.is_ident("u32"));
            }
            let syn::ReturnType::Type(_, result) = function.sig.output else {
                panic!("scalar result required");
            };
            let syn::Type::Path(ty) = *result else {
                panic!("no borrowed output");
            };
            assert!(ty.path.is_ident("u64"));
            exports.push(function.sig.ident.to_string());
        }
        assert_eq!(
            exports,
            [
                "auctioneer_probe_version",
                "auctioneer_probe_schemas",
                "auctioneer_probe_cases",
                "auctioneer_probe_replay",
                "auctioneer_probe_initialize",
                "auctioneer_probe_empty",
                "auctioneer_probe_layout"
            ]
        );
    }

    fn fake_semantic_input<'a>(schema: &'a str, input_json: &'a str) -> SemanticInput<'a> {
        SemanticInput { schema, input_json }
    }

    #[test]
    fn fake_semantic_data_preserves_owner_order_raw_text_duplicates_and_empty_members() {
        let raw = " \n{\"fake_key\":1, \"fake_key\":2} \t";
        let rows = [
            fake_semantic_input("fake_a", raw),
            fake_semantic_input("fake_b", "null"),
            fake_semantic_input("fake_a", "[]"),
            fake_semantic_input("fake_a", raw),
        ];
        let names = ["fake_b", "fake_a", "fake_empty"];
        let data = semantic_data(&names, &rows).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&data).unwrap(),
            json!([
                ["fake_b", ["null"]],
                ["fake_a", [raw, "[]", raw]],
                ["fake_empty", []]
            ])
        );
        assert_eq!(
            semantic_data(&["fake_empty"], &[]).unwrap(),
            "[[\"fake_empty\",[]]]"
        );
    }

    #[test]
    fn fake_semantic_data_refuses_ambiguous_or_unselected_membership_and_bounds_raw_bytes() {
        let row = fake_semantic_input("fake_a", "{}");
        for names in [vec![], vec![""], vec!["fake_a", "fake_a"]] {
            assert_eq!(
                semantic_data(&names, slice::from_ref(&row)).unwrap_err(),
                "invalid bare semantic membership"
            );
        }
        assert_eq!(
            semantic_data(&["fake_b"], &[row]).unwrap_err(),
            "unselected bare semantic schema"
        );
        let input = format!("\"{}\"", "\u{e9}".repeat(0x7fff));
        assert_eq!(input.len(), 0x0001_0000);
        let data = semantic_data(&["fake_a"], &[fake_semantic_input("fake_a", &input)]).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&data).unwrap(),
            json!([["fake_a", [&input]]])
        );
        let oversized = format!("{input} ");
        assert_eq!(
            semantic_data(&["fake_a"], &[fake_semantic_input("fake_a", &oversized)]).unwrap_err(),
            "bare semantic input limit"
        );
    }

    #[test]
    fn fake_semantic_invocations_use_separate_fixed_exports_and_preserve_unsigned_bits() {
        for (value, expected) in [
            (0_u32, "0"),
            (7, "7"),
            (0x7fff_ffff, "2147483647"),
            (0x8000_0000, "-2147483648"),
            (u32::MAX, "-1"),
        ] {
            assert_eq!(
                Call::SemanticCount(value).invocation(),
                ("auctioneer_probe_semantic_cases", vec![expected.to_owned()])
            );
            for reuse in [false, true] {
                assert_eq!(
                    Call::SemanticReplay {
                        schema: value,
                        start: value,
                        reuse
                    }
                    .invocation(),
                    (
                        "auctioneer_probe_semantic_replay",
                        vec![
                            expected.to_owned(),
                            expected.to_owned(),
                            if reuse { "1" } else { "0" }.to_owned()
                        ]
                    )
                );
            }
        }
    }

    #[test]
    fn fake_semantic_source_has_a_separate_table_integer_exports_and_shared_replay() {
        let data =
            semantic_data(&["fake_a"], &[fake_semantic_input("fake_a", " \nnull \t")]).unwrap();
        let syntax: syn::File = syn::parse2(semantic_source(&data)).unwrap();
        let mut exports = Vec::new();
        let mut tables = Vec::new();
        for item in syntax.items {
            if let syn::Item::Const(table) = item {
                let syn::Expr::Lit(expression) = *table.expr else {
                    panic!("literal semantic table required");
                };
                let syn::Lit::Str(literal) = expression.lit else {
                    panic!("string semantic table required");
                };
                tables.push((table.ident.to_string(), literal.value()));
                continue;
            }
            let syn::Item::Fn(function) = item else {
                panic!("unexpected semantic source item");
            };
            let name = function.sig.ident.to_string();
            if !name.starts_with("auctioneer_probe_") {
                assert_eq!(function.sig.ident, "bare_semantic_inputs");
                continue;
            }
            let arity = function.sig.inputs.len();
            assert_eq!(
                function
                    .sig
                    .abi
                    .as_ref()
                    .unwrap()
                    .name
                    .as_ref()
                    .unwrap()
                    .value(),
                "C"
            );
            assert!(function.attrs.iter().any(|attribute| {
                attribute.path().is_ident("unsafe")
                    && attribute
                        .parse_args::<syn::Path>()
                        .unwrap()
                        .is_ident("no_mangle")
            }));
            for parameter in &function.sig.inputs {
                let syn::FnArg::Typed(argument) = parameter else {
                    panic!("integer ABI only");
                };
                let syn::Type::Path(ty) = argument.ty.as_ref() else {
                    panic!("no borrowed arguments");
                };
                assert!(ty.path.is_ident("u32"));
            }
            let syn::ReturnType::Type(_, result) = &function.sig.output else {
                panic!("scalar result required");
            };
            let syn::Type::Path(ty) = result.as_ref() else {
                panic!("no borrowed results");
            };
            assert!(ty.path.is_ident("u64"));
            if name == "auctioneer_probe_semantic_replay" {
                let last = function.block.stmts.last().unwrap();
                assert_eq!(
                    quote!(#last).to_string(),
                    quote!(schema_replay::replay(
                        &rows,
                        start,
                        reuse == 1,
                        *factory,
                        *validate
                    ))
                    .to_string()
                );
            }
            exports.push((name, arity));
        }
        assert_eq!(tables, [("BARE_SEMANTIC_CASES".to_owned(), data)]);
        assert_eq!(
            exports,
            [
                ("auctioneer_probe_semantic_cases".to_owned(), 1_usize),
                ("auctioneer_probe_semantic_replay".to_owned(), 3_usize),
            ]
        );
    }

    #[test]
    fn fake_semantic_source_appends_without_colliding_with_captured_entries() {
        let mut combined = source("[[\"fake_a\",[\"[\"]]]");
        combined.extend(semantic_source("[[\"fake_a\",[\"{}\"]]]"));
        let combined_syntax: syn::File = syn::parse2(combined).unwrap();
        let mut identifiers = BTreeSet::new();
        for item in combined_syntax.items {
            let name = if let syn::Item::Fn(function) = item {
                function.sig.ident.to_string()
            } else {
                let syn::Item::Const(table) = item else {
                    panic!("unexpected combined bare source item");
                };
                table.ident.to_string()
            };
            assert!(identifiers.insert(name));
        }
        assert_eq!(identifiers.len(), 13);
    }
}

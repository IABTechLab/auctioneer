//! Test-only linked-artifact observations, not residency bounds or qualification.

use object::{File as ObjectFile, ObjectSection};
use serde::Serialize;
use wasmparser::{DataKind, Encoding, Parser, Payload, TypeRef, Validator, WasmFeatures};

const MAX_ARTIFACT_BYTES: usize = 64 << 20;
const INVALID: &str = "invalid WASM artifact";
const OVERFLOW: &str = "WASM artifact arithmetic overflow";

pub(super) fn validator() -> Validator {
    // Spin's actual component declares stream types. This enables their static
    // validation only; it says nothing about a deployment's async support.
    Validator::new_with_features(WasmFeatures::default() | WasmFeatures::CM_ASYNC)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactFormat {
    Module,
    Component,
}

/// Static declarations across every core module, including nested components.
/// These are not instantiated-module counts or actual resident-memory sizes.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct WasmArtifact {
    /// Entire input length, counted once regardless of nesting.
    pub artifact_bytes: u64,
    /// Outermost binary encoding, not the encoding of the last nested module.
    pub format: ArtifactFormat,
    pub core_modules: u64,
    pub components: u64,
    pub defined_functions: u64,
    pub imported_functions: u64,
    pub code_bodies: u64,
    /// `FunctionBody` ranges: locals and operators, excluding section/body framing.
    pub code_body_bytes: u64,
    pub data_segments: u64,
    /// Data payloads only; offsets, lengths and section framing are excluded.
    pub active_data_bytes: u64,
    pub passive_data_bytes: u64,
    pub defined_memories: u64,
    pub imported_memories: u64,
    /// Defined memories' declared minimum bytes, excluding imported memories.
    /// Not actual allocation, instantiation, residency or qualification evidence.
    pub initial_memory_bytes: u64,
}

/// Measure a validated module or component of at most 64 MiB.
/// Unsupported proposals, including custom memory page sizes, are refused.
pub fn measure(bytes: &[u8]) -> Result<WasmArtifact, &'static str> {
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err("WASM artifact exceeds 64 MiB");
    }
    // The pinned validator's default features reject custom page sizes and
    // validate every nested module's code, not just its section framing.
    validator().validate_all(bytes).map_err(|_error| INVALID)?;

    let mut payloads = Parser::new(0).parse_all(bytes);
    let (format, core_modules, components) = match payloads.next() {
        Some(Ok(Payload::Version {
            encoding: Encoding::Module,
            ..
        })) => (ArtifactFormat::Module, 1, 0),
        Some(Ok(Payload::Version {
            encoding: Encoding::Component,
            ..
        })) => (ArtifactFormat::Component, 0, 1),
        _ => return Err(INVALID),
    };
    let mut artifact = WasmArtifact {
        artifact_bytes: u64::try_from(bytes.len()).map_err(|_error| OVERFLOW)?,
        format,
        core_modules,
        components,
        defined_functions: 0,
        imported_functions: 0,
        code_bodies: 0,
        code_body_bytes: 0,
        data_segments: 0,
        active_data_bytes: 0,
        passive_data_bytes: 0,
        defined_memories: 0,
        imported_memories: 0,
        initial_memory_bytes: 0,
    };

    // parse_all traverses child parsers automatically. Count their headers and
    // leaf payloads, never the enclosing module/component or code-section sizes.
    for payload in payloads {
        record_payload(&mut artifact, payload.map_err(|_error| INVALID)?)?;
    }

    // Validation reconciles each core module; also reconcile the observed sum.
    if artifact.defined_functions != artifact.code_bodies {
        return Err(INVALID);
    }
    Ok(artifact)
}

fn record_payload(artifact: &mut WasmArtifact, payload: Payload<'_>) -> Result<(), &'static str> {
    match payload {
        Payload::Version { encoding, .. } => match encoding {
            Encoding::Module => add(&mut artifact.core_modules, 1)?,
            Encoding::Component => add(&mut artifact.components, 1)?,
        },
        Payload::FunctionSection(section) => {
            add(&mut artifact.defined_functions, u64::from(section.count()))?;
        }
        Payload::ImportSection(section) => {
            for import in section.into_imports() {
                match import.map_err(|_error| INVALID)?.ty {
                    TypeRef::Func(_) | TypeRef::FuncExact(_) => {
                        add(&mut artifact.imported_functions, 1)?;
                    }
                    TypeRef::Memory(_) => add(&mut artifact.imported_memories, 1)?,
                    TypeRef::Table(_) | TypeRef::Global(_) | TypeRef::Tag(_) => {}
                }
            }
        }
        Payload::MemorySection(section) => {
            for memory_result in section {
                let memory = memory_result.map_err(|_error| INVALID)?;
                if memory.page_size_log2.is_some() {
                    return Err(INVALID);
                }
                let initial = memory.initial.checked_mul(0x0001_0000).ok_or(OVERFLOW)?;
                add(&mut artifact.defined_memories, 1)?;
                add(&mut artifact.initial_memory_bytes, initial)?;
            }
        }
        Payload::CodeSectionEntry(body) => {
            add(&mut artifact.code_bodies, 1)?;
            let range = body.range();
            add(
                &mut artifact.code_body_bytes,
                range.end.checked_sub(range.start).ok_or(OVERFLOW)?,
            )?;
        }
        Payload::DataSection(section) => {
            for segment_result in section {
                let segment = segment_result.map_err(|_error| INVALID)?;
                let data_bytes = u64::try_from(segment.data.len()).map_err(|_error| OVERFLOW)?;
                add(&mut artifact.data_segments, 1)?;
                match segment.kind {
                    DataKind::Active { .. } => {
                        add(&mut artifact.active_data_bytes, data_bytes)?;
                    }
                    DataKind::Passive => add(&mut artifact.passive_data_bytes, data_bytes)?,
                }
            }
        }
        Payload::TypeSection(_)
        | Payload::TableSection(_)
        | Payload::TagSection(_)
        | Payload::GlobalSection(_)
        | Payload::ExportSection(_)
        | Payload::StartSection { .. }
        | Payload::ElementSection(_)
        | Payload::DataCountSection { .. }
        | Payload::CodeSectionStart { .. }
        | Payload::ModuleSection { .. }
        | Payload::InstanceSection(_)
        | Payload::CoreTypeSection(_)
        | Payload::ComponentSection { .. }
        | Payload::ComponentInstanceSection(_)
        | Payload::ComponentAliasSection(_)
        | Payload::ComponentTypeSection(_)
        | Payload::ComponentCanonicalSection(_)
        | Payload::ComponentStartSection { .. }
        | Payload::ComponentImportSection(_)
        | Payload::ComponentExportSection(_)
        | Payload::CustomSection(_)
        | Payload::UnknownSection { .. }
        | Payload::End(_) => {}
        _ => return Err(INVALID),
    }
    Ok(())
}

fn add(total: &mut u64, value: u64) -> Result<(), &'static str> {
    *total = total.checked_add(value).ok_or(OVERFLOW)?;
    Ok(())
}

/// Native section observations only; excludes mappings, loader and allocator costs.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct NativeArtifact {
    pub artifact_bytes: u64,
    pub code_bytes: u64,
    pub data_bytes: u64,
    pub uninitialized_bytes: u64,
    pub matched_data_witnesses: u64,
}

pub fn measure_native(bytes: &[u8], required: &[&str]) -> Result<NativeArtifact, &'static str> {
    use object::{BinaryFormat, File, Object as _, ObjectSection as _, SectionKind};
    use std::collections::BTreeSet;

    if required.is_empty()
        || required.len() > 512
        || required
            .iter()
            .any(|value| value.is_empty() || value.len() > 4096)
        || required.iter().copied().collect::<BTreeSet<_>>().len() != required.len()
    {
        return Err("invalid native artifact witnesses");
    }
    if bytes.is_empty() || bytes.len() > 256_usize << 20_u32 {
        return Err("invalid native artifact");
    }
    let file = File::parse(bytes).map_err(|_error| "invalid native artifact")?;
    if !matches!(file.format(), BinaryFormat::Elf | BinaryFormat::MachO) {
        return Err("invalid native artifact");
    }
    let mut result = NativeArtifact {
        artifact_bytes: u64::try_from(bytes.len()).map_err(|_error| OVERFLOW)?,
        code_bytes: 0,
        data_bytes: 0,
        uninitialized_bytes: 0,
        matched_data_witnesses: 0,
    };
    let mut found = vec![false; required.len()];
    for section in file.sections() {
        if section.kind() == SectionKind::Text {
            add(&mut result.code_bytes, section.size())?;
        } else if section.kind().is_bss() {
            add(&mut result.uninitialized_bytes, section.size())?;
        } else if native_program_data(&file, &section)? {
            let data = section.data().map_err(|_error| "invalid native artifact")?;
            add(&mut result.data_bytes, section.size())?;
            for (present, witness) in found.iter_mut().zip(required) {
                *present |= data
                    .windows(witness.len())
                    .any(|window| window == witness.as_bytes());
            }
        } else {
            // Debug, relocation and other non-program sections supply no
            // code/data residency observations or retained witnesses.
        }
    }
    if found.iter().any(|present| !present) {
        return Err("native artifact lost required data witness");
    }
    result.matched_data_witnesses = u64::try_from(found.len()).map_err(|_error| OVERFLOW)?;
    Ok(result)
}

/// Segment/section flags also cover loadable data with an unclassified name.
pub(super) fn native_program_data<'data>(
    file: &ObjectFile<'data>,
    section: &impl ObjectSection<'data>,
) -> Result<bool, &'static str> {
    use object::{
        Object as _, ObjectSegment as _, SectionFlags, SectionKind, SegmentFlags, elf, macho,
    };
    if section.kind().is_bss()
        || matches!(
            section.kind(),
            SectionKind::Text
                | SectionKind::Debug
                | SectionKind::DebugString
                | SectionKind::OtherString
        )
    {
        return Ok(false);
    }
    let section_flags = section.flags();
    if let SectionFlags::Elf { sh_flags, .. } = section_flags {
        return Ok(sh_flags.contains(elf::SHF_ALLOC) && !sh_flags.intersects(elf::SHF_EXECINSTR));
    }
    let SectionFlags::MachO { flags, .. } = section_flags else {
        return Err("unsupported native inspection section");
    };
    if flags.intersects(
        macho::S_ATTR_DEBUG | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
    ) {
        return Ok(false);
    }
    let start = section.address();
    let end = start
        .checked_add(section.size())
        .ok_or("native inspection address overflow")?;
    for segment in file.segments() {
        if matches!(segment.flags(), SegmentFlags::MachO {initprot, ..} if initprot.contains(macho::VM_PROT_READ))
        {
            let segment_end = segment
                .address()
                .checked_add(segment.size())
                .ok_or("native inspection address overflow")?;
            if segment.address() <= start && end <= segment_end {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::{ArtifactFormat, WasmArtifact, measure};
    use object::{Architecture, BinaryFormat, Endianness, SectionKind, write};

    fn fake_stream_component(payload: wasm_encoder::ComponentValType) -> Vec<u8> {
        let mut types = wasm_encoder::ComponentTypeSection::new();
        types.defined_type().stream(Some(payload));
        let mut component = wasm_encoder::Component::new();
        component.section(&types);
        component.finish()
    }

    #[test]
    fn component_stream_types_validate_with_explicit_async_proposal_support() {
        let bytes = fake_stream_component(wasm_encoder::ComponentValType::Primitive(
            wasm_encoder::PrimitiveValType::U8,
        ));
        // New parser releases enable async components by default. The negative
        // control must actually disable the proposal instead of relying on that default.
        let mut features = wasmparser::WasmFeatures::default();
        features.remove(wasmparser::WasmFeatures::CM_ASYNC);
        assert!(
            wasmparser::Validator::new_with_features(features)
                .validate_all(&bytes)
                .is_err()
        );
        assert!(
            super::validator()
                .features()
                .contains(wasmparser::WasmFeatures::CM_ASYNC)
        );
        let measured = measure(&bytes).expect("valid stream type used by the Spin component");
        assert_eq!(measured.format, ArtifactFormat::Component);
        assert_eq!(measured.components, 1);
        assert_eq!(measured.core_modules, 0);
        assert_eq!(measured.code_body_bytes, 0);
        assert_eq!(measured.initial_memory_bytes, 0);
    }

    #[test]
    fn component_stream_support_does_not_accept_an_undefined_payload_type() {
        let bytes = fake_stream_component(wasm_encoder::ComponentValType::Type(99));
        assert_eq!(measure(&bytes), Err("invalid WASM artifact"));
    }

    fn fake_native_object(format: BinaryFormat, marker_kind: SectionKind) -> Vec<u8> {
        let mut object = write::Object::new(format, Architecture::Aarch64, Endianness::Little);
        let code = object.section_id(write::StandardSection::Text);
        object.append_section_data(code, &[0xc0, 0x03, 0x5f, 0xd6], 4);
        let data = if marker_kind == SectionKind::ReadOnlyData {
            object.section_id(write::StandardSection::ReadOnlyData)
        } else {
            object.add_section(b"__DWARF".to_vec(), b".debug_fake".to_vec(), marker_kind)
        };
        object.append_section_data(data, b"fake_artifact_identity", 1);
        let zero = object.section_id(write::StandardSection::UninitializedData);
        object.append_section_bss(zero, 19, 1);
        object.write().expect("synthetic native object")
    }

    #[test]
    fn native_artifact_inspection_keeps_code_data_and_uninitialized_storage_separate() {
        for format in [BinaryFormat::Elf, BinaryFormat::MachO] {
            let bytes = fake_native_object(format, SectionKind::ReadOnlyData);
            let measured = super::measure_native(&bytes, &["fake_artifact_identity"])
                .expect("native section observations");
            assert_eq!(measured.artifact_bytes, u64::try_from(bytes.len()).unwrap());
            assert_eq!(measured.code_bytes, 4);
            assert_eq!(measured.data_bytes, 22);
            assert_eq!(measured.uninitialized_bytes, 19);
            assert_eq!(measured.matched_data_witnesses, 1);
        }
    }

    #[test]
    fn native_artifact_membership_cannot_be_satisfied_by_debug_metadata() {
        let bytes = fake_native_object(BinaryFormat::Elf, SectionKind::Debug);
        assert_eq!(
            super::measure_native(&bytes, &["fake_artifact_identity"]),
            Err("native artifact lost required data witness")
        );
    }

    #[test]
    fn native_artifact_membership_refuses_missing_empty_and_duplicate_witnesses() {
        let bytes = fake_native_object(BinaryFormat::Elf, SectionKind::ReadOnlyData);
        for required in [
            vec!["fake_absent_identity"],
            vec!["fake_artifact_identity", "fake_absent_identity"],
        ] {
            assert_eq!(
                super::measure_native(&bytes, &required),
                Err("native artifact lost required data witness")
            );
        }
        for required in [vec![], vec![""], vec!["fake_artifact_identity"; 2]] {
            assert_eq!(
                super::measure_native(&bytes, &required),
                Err("invalid native artifact witnesses")
            );
        }
        assert_eq!(
            super::measure_native(b"not an object", &["fake_artifact_identity"]),
            Err("invalid native artifact")
        );
    }

    const EMPTY_MODULE: &[u8] = b"\0asm\x01\0\0\0";
    const EMPTY_COMPONENT: &[u8] = b"\0asm\x0d\0\x01\0";

    // Explicit binary test data only; the implementation must use wasmparser.
    // One imported function, two defined functions, one imported 3-page memory,
    // one defined 2-page memory, bodies of 2 and 7 bytes, and 3/4 data bytes.
    const MIXED_MODULE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x02,
        0x0e, 0x02, 0x01, 0x6d, 0x01, 0x66, 0x00, 0x00, 0x01, 0x6d, 0x01, 0x69, 0x02, 0x00, 0x03,
        0x03, 0x03, 0x02, 0x00, 0x00, 0x05, 0x03, 0x01, 0x00, 0x02, 0x0a, 0x0c, 0x02, 0x02, 0x00,
        0x0b, 0x07, 0x01, 0x01, 0x7f, 0x41, 0x00, 0x1a, 0x0b, 0x0b, 0x0f, 0x02, 0x00, 0x41, 0x00,
        0x0b, 0x03, 0x61, 0x62, 0x63, 0x01, 0x04, 0x77, 0x78, 0x79, 0x7a,
    ];

    // One defined function (4-byte body), one defined 5-page memory,
    // and one passive segment containing 5 bytes.
    const OTHER_MODULE: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03,
        0x02, 0x01, 0x00, 0x05, 0x03, 0x01, 0x00, 0x05, 0x0a, 0x06, 0x01, 0x04, 0x00, 0x01, 0x01,
        0x0b, 0x0b, 0x08, 0x01, 0x01, 0x05, 0x61, 0x62, 0x63, 0x64, 0x65,
    ];

    #[test]
    fn empty_module_has_zero_code_data_and_memory_observations() {
        assert_eq!(
            measure(EMPTY_MODULE),
            Ok(WasmArtifact {
                artifact_bytes: 8,
                format: ArtifactFormat::Module,
                core_modules: 1,
                components: 0,
                defined_functions: 0,
                imported_functions: 0,
                code_bodies: 0,
                code_body_bytes: 0,
                data_segments: 0,
                active_data_bytes: 0,
                passive_data_bytes: 0,
                defined_memories: 0,
                imported_memories: 0,
                initial_memory_bytes: 0,
            })
        );
    }

    #[test]
    fn empty_component_is_not_a_core_module() {
        assert_eq!(
            measure(EMPTY_COMPONENT),
            Ok(WasmArtifact {
                artifact_bytes: 8,
                format: ArtifactFormat::Component,
                core_modules: 0,
                components: 1,
                defined_functions: 0,
                imported_functions: 0,
                code_bodies: 0,
                code_body_bytes: 0,
                data_segments: 0,
                active_data_bytes: 0,
                passive_data_bytes: 0,
                defined_memories: 0,
                imported_memories: 0,
                initial_memory_bytes: 0,
            })
        );
    }

    #[test]
    fn module_measures_unequal_bodies_and_data_without_section_framing() {
        assert_eq!(
            measure(MIXED_MODULE),
            Ok(WasmArtifact {
                artifact_bytes: 71,
                format: ArtifactFormat::Module,
                core_modules: 1,
                components: 0,
                defined_functions: 2,
                imported_functions: 1,
                code_bodies: 2,
                code_body_bytes: 9,
                data_segments: 2,
                active_data_bytes: 3,
                passive_data_bytes: 4,
                defined_memories: 1,
                imported_memories: 1,
                initial_memory_bytes: 0x0002_0000,
            })
        );
    }

    #[test]
    fn second_module_has_independent_nonzero_totals() {
        assert_eq!(
            measure(OTHER_MODULE),
            Ok(WasmArtifact {
                artifact_bytes: 41,
                format: ArtifactFormat::Module,
                core_modules: 1,
                components: 0,
                defined_functions: 1,
                imported_functions: 0,
                code_bodies: 1,
                code_body_bytes: 4,
                data_segments: 1,
                active_data_bytes: 0,
                passive_data_bytes: 5,
                defined_memories: 1,
                imported_memories: 0,
                initial_memory_bytes: 0x0005_0000,
            })
        );
    }

    #[test]
    fn nested_component_observes_all_core_modules_once_and_keeps_outer_format() {
        for bytes in [
            [
                EMPTY_COMPONENT,
                &[0x01, 0x47],
                MIXED_MODULE,
                &[0x04, 0x33],
                EMPTY_COMPONENT,
                &[0x01, 0x29],
                OTHER_MODULE,
            ]
            .concat(),
            // Resume the outer parser after the nested component ends.
            [
                EMPTY_COMPONENT,
                &[0x04, 0x33],
                EMPTY_COMPONENT,
                &[0x01, 0x29],
                OTHER_MODULE,
                &[0x01, 0x47],
                MIXED_MODULE,
            ]
            .concat(),
        ] {
            assert_eq!(
                measure(&bytes),
                Ok(WasmArtifact {
                    artifact_bytes: 134,
                    format: ArtifactFormat::Component,
                    core_modules: 2,
                    components: 2,
                    defined_functions: 3,
                    imported_functions: 1,
                    code_bodies: 3,
                    code_body_bytes: 13,
                    data_segments: 3,
                    active_data_bytes: 3,
                    passive_data_bytes: 9,
                    defined_memories: 2,
                    imported_memories: 1,
                    initial_memory_bytes: 0x0007_0000,
                })
            );
        }
    }

    #[test]
    fn imported_memory_minimum_is_not_defined_initial_memory() {
        let bytes = [
            EMPTY_MODULE,
            &[0x02, 0x08, 0x01, 0x01, 0x6d, 0x01, 0x69, 0x02, 0x00, 0x07],
        ]
        .concat();
        assert_eq!(
            measure(&bytes),
            Ok(WasmArtifact {
                artifact_bytes: 18,
                format: ArtifactFormat::Module,
                core_modules: 1,
                components: 0,
                defined_functions: 0,
                imported_functions: 0,
                code_bodies: 0,
                code_body_bytes: 0,
                data_segments: 0,
                active_data_bytes: 0,
                passive_data_bytes: 0,
                defined_memories: 0,
                imported_memories: 1,
                initial_memory_bytes: 0,
            })
        );
    }

    #[test]
    fn multiple_defined_memory_minima_are_added_without_code_or_data() {
        let bytes = [EMPTY_MODULE, &[0x05, 0x05, 0x02, 0x00, 0x02, 0x00, 0x05]].concat();
        assert_eq!(
            measure(&bytes),
            Ok(WasmArtifact {
                artifact_bytes: 15,
                format: ArtifactFormat::Module,
                core_modules: 1,
                components: 0,
                defined_functions: 0,
                imported_functions: 0,
                code_bodies: 0,
                code_body_bytes: 0,
                data_segments: 0,
                active_data_bytes: 0,
                passive_data_bytes: 0,
                defined_memories: 2,
                imported_memories: 0,
                initial_memory_bytes: 0x0007_0000,
            })
        );
    }

    #[test]
    fn memory64_minimum_is_measured_with_checked_byte_conversion() {
        let bytes = [EMPTY_MODULE, &[0x05, 0x03, 0x01, 0x04, 0x03]].concat();
        assert_eq!(
            measure(&bytes),
            Ok(WasmArtifact {
                artifact_bytes: 13,
                format: ArtifactFormat::Module,
                core_modules: 1,
                components: 0,
                defined_functions: 0,
                imported_functions: 0,
                code_bodies: 0,
                code_body_bytes: 0,
                data_segments: 0,
                active_data_bytes: 0,
                passive_data_bytes: 0,
                defined_memories: 1,
                imported_memories: 0,
                initial_memory_bytes: 0x0003_0000,
            })
        );
    }

    #[test]
    fn memory64_page_to_byte_overflow_is_refused() {
        // 2^48 standard pages: a valid declaration whose byte count is 2^64.
        let bytes = [
            EMPTY_MODULE,
            &[
                0x05, 0x09, 0x01, 0x04, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x40,
            ],
        ]
        .concat();
        assert_eq!(measure(&bytes), Err("WASM artifact arithmetic overflow"));
    }

    #[test]
    fn sum_of_valid_memory_minima_cannot_wrap() {
        // Each defined memory has 2^47 pages, or 2^63 bytes independently.
        let bytes = [
            EMPTY_MODULE,
            &[
                0x05, 0x11, 0x02, 0x04, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x20, 0x04, 0x80, 0x80,
                0x80, 0x80, 0x80, 0x80, 0x20,
            ],
        ]
        .concat();
        assert_eq!(measure(&bytes), Err("WASM artifact arithmetic overflow"));
    }

    #[test]
    fn custom_memory_page_sizes_are_refused_instead_of_assuming_64k_pages() {
        for declaration in [
            &[0x05, 0x04, 0x01, 0x08, 0x03, 0x00][..],
            &[0x05, 0x04, 0x01, 0x08, 0x03, 0x10],
            &[0x05, 0x04, 0x01, 0x08, 0x03, 0x3f],
            &[0x05, 0x04, 0x01, 0x08, 0x03, 0x40],
        ] {
            assert_eq!(
                measure(&[EMPTY_MODULE, declaration].concat()),
                Err("invalid WASM artifact")
            );
        }
    }

    #[test]
    fn truncated_or_bad_magic_artifacts_are_refused() {
        for bytes in [
            &b""[..],
            &b"\0asm"[..],
            &b"nope\x01\0\0\0"[..],
            &b"\0asm\x02\0\0\0"[..],
            &EMPTY_MODULE[..7],
            &MIXED_MODULE[..70],
            &OTHER_MODULE[..40],
        ] {
            assert_eq!(measure(bytes), Err("invalid WASM artifact"));
        }
        let truncated_child = [EMPTY_COMPONENT, &[0x01, 0x29], &OTHER_MODULE[..40]].concat();
        assert_eq!(measure(&truncated_child), Err("invalid WASM artifact"));
    }

    #[test]
    fn duplicate_sections_and_semantically_invalid_bodies_are_refused() {
        for (sections, length) in [
            // Duplicate memory sections, even with zero entries.
            (&[0x05, 0x01, 0x00, 0x05, 0x01, 0x00][..], 0x0e),
            // Declared function without a code body.
            (
                &[0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00],
                0x12,
            ),
            // Two defined functions but only one code body.
            (
                &[
                    0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03, 0x03, 0x02, 0x00, 0x00, 0x0a, 0x04,
                    0x01, 0x02, 0x00, 0x0b,
                ],
                0x19,
            ),
            // Syntactically readable code, invalid operand stack (drop on empty).
            (
                &[
                    0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x03, 0x02, 0x01, 0x00, 0x0a, 0x05, 0x01,
                    0x03, 0x00, 0x1a, 0x0b,
                ],
                0x19,
            ),
            // Hostile section length, not an allocation request.
            (&[0x0b, 0xff, 0xff, 0xff, 0xff, 0x0f], 0x0e),
        ] {
            let module = [EMPTY_MODULE, sections].concat();
            assert_eq!(measure(&module), Err("invalid WASM artifact"));
            assert_eq!(module.len(), usize::from(length));
            assert!(length < 128);
            let nested = [EMPTY_COMPONENT, &[0x01, length], module.as_slice()].concat();
            assert_eq!(measure(&nested), Err("invalid WASM artifact"));
        }
    }

    #[test]
    fn byte_cap_is_enforced_before_attempting_validation() {
        // Intentionally invalid magic; a validation-first implementation gives
        // the wrong error. This literal is independent of the implementation cap.
        let bytes = vec![0; 0x0400_0001];
        assert_eq!(measure(&bytes), Err("WASM artifact exceeds 64 MiB"));
    }

    #[test]
    fn exact_byte_cap_is_allowed_without_counting_custom_section_as_data() {
        // Module plus a custom section with an empty name and zero-filled data.
        // Its literal ULEB length is 67,108,851, excluding 13 framing bytes.
        let mut bytes = b"\0asm\x01\0\0\0\0\xf3\xff\xff\x1f\0".to_vec();
        bytes.resize(0x0400_0000, 0);
        assert_eq!(
            measure(&bytes),
            Ok(WasmArtifact {
                artifact_bytes: 0x0400_0000,
                format: ArtifactFormat::Module,
                core_modules: 1,
                components: 0,
                defined_functions: 0,
                imported_functions: 0,
                code_bodies: 0,
                code_body_bytes: 0,
                data_segments: 0,
                active_data_bytes: 0,
                passive_data_bytes: 0,
                defined_memories: 0,
                imported_memories: 0,
                initial_memory_bytes: 0,
            })
        );
    }
}

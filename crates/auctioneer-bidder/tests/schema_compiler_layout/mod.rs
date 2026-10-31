//! Bounded compiler-layout metadata reader, not a resource qualification.
//!
//! Input is bytes only: filesystem/symlink policy, source/cfg/compiler/profile
//! correspondence and application-artifact identity belong to the driver.
//! Native objects require library-applied DWARF relocations. Linked core Wasm
//! must have no remaining `reloc.*` sections; no Wasm relocation is interpreted.
//! Missing size/alignment is a refusal, never an inferred layout or zero cost.

use crate::schema_heap_layout::NativeField;
use gimli::{
    AttributeValue, DebuggingInformationEntry, Dwarf, DwarfSections, EndianSlice, Reader, Relocate,
    RelocateReader, RunTimeEndian, Section as _, Unit,
};
use object::{Object as _, ObjectSection as _};
use std::collections::{BTreeMap, BTreeSet};
use wasmparser::{Encoding, Parser, Payload};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Limit {
    ObjectBytes,
    DebugBytes,
    Units,
    Entries,
    Depth,
    Names,
    Requests,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    pub object_bytes: usize,
    pub debug_bytes: usize,
    pub units: usize,
    pub entries: usize,
    pub depth: usize,
    pub name_bytes: usize,
    pub requests: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Error {
    InvalidRequest,
    InvalidObject,
    MissingDwarf,
    MalformedDwarf,
    Relocation,
    RelocatableWasm,
    Limit(Limit),
    MissingType(String),
    MissingByteSize(String),
    MissingAlignment(String),
    InvalidLayout(String),
    ConflictingType(String),
}

// Operational reader caps, not constructor/serving memory allowances. Drivers
// may impose smaller caps with layouts_with_limits; no read is unbounded.
const DEFAULT_LIMITS: Limits = Limits {
    object_bytes: 0x0400_0000,
    debug_bytes: 0x0400_0000,
    units: 4096,
    entries: 1_000_000,
    depth: 256,
    name_bytes: 0x0400_0000,
    requests: 256,
};

pub(crate) fn layouts(
    bytes: &[u8],
    requested: &[&str],
) -> Result<BTreeMap<String, NativeField>, Error> {
    layouts_with_limits(bytes, requested, DEFAULT_LIMITS)
}

pub(crate) fn layouts_with_limits(
    bytes: &[u8],
    requested: &[&str],
    limits: Limits,
) -> Result<BTreeMap<String, NativeField>, Error> {
    requests(requested, limits)?;
    if bytes.len() > limits.object_bytes {
        return Err(Error::Limit(Limit::ObjectBytes));
    }
    if bytes.starts_with(b"\0asm") {
        wasm_layouts(bytes, requested, limits)
    } else {
        native_layouts(bytes, requested, limits)
    }
}

// Shared core for relocated native readers and relocation-free Wasm sections.
pub(crate) fn dwarf_layouts<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    requested: &[&str],
    limits: Limits,
) -> Result<BTreeMap<String, NativeField>, Error> {
    let wanted = requests(requested, limits)?;
    if dwarf.debug_info.reader().is_empty() || dwarf.debug_abbrev.reader().is_empty() {
        return Err(Error::MissingDwarf);
    }
    check_dwarf_bytes(dwarf, limits)?;
    let mut budget = ReadBudget {
        limits,
        units: 0,
        entries: 0,
        names: 0,
    };
    for name in &wanted {
        budget.name(name.len())?;
    }
    let mut result = BTreeMap::new();
    let mut units = dwarf.units();
    while let Some(header) = units.next().map_err(|_error| Error::MalformedDwarf)? {
        spend(&mut budget.units, 1, limits.units, Limit::Units)?;
        let unit = dwarf.unit(header).map_err(|_error| Error::MalformedDwarf)?;
        read_unit(dwarf, &unit, &wanted, &mut result, &mut budget)?;
    }
    // Do not stop when all requested types first appear: later definitions
    // must still be checked for conflicts and malformed input.
    for name in wanted {
        if !result.contains_key(name) {
            return Err(Error::MissingType(name.to_owned()));
        }
    }
    Ok(result)
}

fn check_dwarf_bytes<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    limits: Limits,
) -> Result<(), Error> {
    // The common reader can be called without an object loader. Bound every
    // section it can consult for unit headers, abbreviations, lines and names.
    // Supplementary files would add an unbounded second input, so refuse them.
    if dwarf.sup.is_some() {
        return Err(Error::MalformedDwarf);
    }
    let sizes = [
        dwarf.debug_info.reader().len(),
        dwarf.debug_abbrev.reader().len(),
        dwarf.debug_addr.reader().len(),
        dwarf.debug_aranges.reader().len(),
        dwarf.debug_line.reader().len(),
        dwarf.debug_line_str.reader().len(),
        dwarf.debug_macinfo.reader().len(),
        dwarf.debug_macro.reader().len(),
        dwarf.debug_str.reader().len(),
        dwarf.debug_str_offsets.reader().len(),
        dwarf.debug_types.reader().len(),
        dwarf.ranges.debug_ranges().reader().len(),
        dwarf.ranges.debug_rnglists().reader().len(),
    ];
    let mut total = 0;
    for size in sizes {
        spend(&mut total, size, limits.debug_bytes, Limit::DebugBytes)?;
    }
    Ok(())
}

fn requests<'a>(requested: &[&'a str], limits: Limits) -> Result<BTreeSet<&'a str>, Error> {
    if requested.is_empty() {
        return Err(Error::InvalidRequest);
    }
    if requested.len() > limits.requests {
        return Err(Error::Limit(Limit::Requests));
    }
    let mut wanted = BTreeSet::new();
    let mut bytes = 0;
    for name in requested {
        spend(&mut bytes, name.len(), limits.name_bytes, Limit::Names)?;
        if name.contains('\0') || name.split("::").any(str::is_empty) || !wanted.insert(*name) {
            return Err(Error::InvalidRequest);
        }
    }
    Ok(wanted)
}

fn spend(used: &mut usize, amount: usize, maximum: usize, kind: Limit) -> Result<(), Error> {
    *used = used
        .checked_add(amount)
        .filter(|total| *total <= maximum)
        .ok_or(Error::Limit(kind))?;
    Ok(())
}

struct NativeSection<'a> {
    data: &'a [u8],
    relocations: object::RelocationMap,
}

#[derive(Clone, Copy, Debug)]
struct LibraryRelocations<'a>(&'a object::RelocationMap);

impl Relocate for LibraryRelocations<'_> {
    fn relocate_address(&self, offset: usize, value: u64) -> gimli::Result<u64> {
        let offset_u64 = u64::try_from(offset).map_err(|_error| gimli::Error::UnsupportedOffset)?;
        Ok(self.0.relocate(offset_u64, value))
    }

    fn relocate_offset(&self, offset: usize, value: usize) -> gimli::Result<usize> {
        let value_u64 = u64::try_from(value).map_err(|_error| gimli::Error::UnsupportedOffset)?;
        let relocated = self.relocate_address(offset, value_u64)?;
        usize::try_from(relocated).map_err(|_error| gimli::Error::UnsupportedOffset)
    }
}

fn native_layouts(
    bytes: &[u8],
    requested: &[&str],
    limits: Limits,
) -> Result<BTreeMap<String, NativeField>, Error> {
    let object = object::File::parse(bytes).map_err(|_error| Error::InvalidObject)?;
    let endian = if object.is_little_endian() {
        RunTimeEndian::Little
    } else {
        RunTimeEndian::Big
    };
    let mut debug_bytes = 0;
    let sections = DwarfSections::load(|id| native_section(&object, id, limits, &mut debug_bytes))?;
    let dwarf = sections.borrow(|section| {
        RelocateReader::new(
            EndianSlice::new(section.data, endian),
            LibraryRelocations(&section.relocations),
        )
    });
    dwarf_layouts(&dwarf, requested, limits)
}

fn native_section<'a>(
    object: &object::File<'a>,
    id: gimli::SectionId,
    limits: Limits,
    used: &mut usize,
) -> Result<NativeSection<'a>, Error> {
    // object handles Mach-O section aliases and section-relative relocations.
    let Some(section) = object.section_by_name(id.name()) else {
        return Ok(NativeSection {
            data: &[],
            relocations: object::RelocationMap::default(),
        });
    };
    let data = section
        .compressed_data()
        .map_err(|_error| Error::MalformedDwarf)?;
    let size = usize::try_from(data.uncompressed_size)
        .map_err(|_error| Error::Limit(Limit::DebugBytes))?;
    spend(used, size, limits.debug_bytes, Limit::DebugBytes)?;
    // No decompression allocation or dependency feature is introduced. These
    // diagnostic leaf objects are uncompressed; other inputs refuse explicitly.
    if data.format != object::CompressionFormat::None || size != data.data.len() {
        return Err(Error::MalformedDwarf);
    }
    let relocations = section
        .relocation_map()
        .map_err(|_error| Error::Relocation)?;
    Ok(NativeSection {
        data: data.data,
        relocations,
    })
}

fn wasm_layouts(
    bytes: &[u8],
    requested: &[&str],
    limits: Limits,
) -> Result<BTreeMap<String, NativeField>, Error> {
    let debug = wasm_sections(bytes, limits)?;
    let sections =
        DwarfSections::load(|id| Ok::<_, Error>(debug.get(id.name()).copied().unwrap_or(&[])))?;
    let dwarf = sections.borrow(|section| EndianSlice::new(section, gimli::LittleEndian));
    dwarf_layouts(&dwarf, requested, limits)
}

fn wasm_sections(bytes: &[u8], limits: Limits) -> Result<BTreeMap<&str, &[u8]>, Error> {
    let mut debug = BTreeMap::new();
    let mut used = 0;
    let mut payloads = 0;
    for payload in Parser::new(0).parse_all(bytes) {
        spend(&mut payloads, 1, limits.entries, Limit::Entries)?;
        let parsed = payload.map_err(|_error| Error::InvalidObject)?;
        if let Payload::Version { num, encoding, .. } = &parsed
            && (*num != 1 || *encoding != Encoding::Module)
        {
            return Err(Error::InvalidObject);
        }
        if let Payload::CustomSection(section) = parsed {
            if section.name().starts_with("reloc.") {
                return Err(Error::RelocatableWasm);
            }
            if section.name().starts_with(".debug_") {
                spend(
                    &mut used,
                    section.data().len(),
                    limits.debug_bytes,
                    Limit::DebugBytes,
                )?;
                if debug.insert(section.name(), section.data()).is_some() {
                    return Err(Error::MalformedDwarf);
                }
            }
        }
    }
    // All custom sections were checked before parsing DWARF, including any
    // reloc.* section appearing after the requested type definitions.
    Ok(debug)
}

struct ReadBudget {
    limits: Limits,
    units: usize,
    entries: usize,
    names: usize,
}

impl ReadBudget {
    fn name(&mut self, bytes: usize) -> Result<(), Error> {
        spend(&mut self.names, bytes, self.limits.name_bytes, Limit::Names)
    }
}

enum Scope {
    Transparent,
    Named(String),
    Opaque,
}

fn structural(tag: gimli::DwTag) -> bool {
    matches!(
        tag,
        gimli::DW_TAG_structure_type
            | gimli::DW_TAG_class_type
            | gimli::DW_TAG_union_type
            | gimli::DW_TAG_enumeration_type
    )
}

fn named_scope(tag: gimli::DwTag) -> bool {
    structural(tag) || matches!(tag, gimli::DW_TAG_namespace | gimli::DW_TAG_subprogram)
}

fn read_unit<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    wanted: &BTreeSet<&str>,
    result: &mut BTreeMap<String, NativeField>,
    budget: &mut ReadBudget,
) -> Result<(), Error> {
    let mut entries = unit.entries();
    let mut scopes = Vec::new();
    let mut depth = 0_isize;
    while let Some((delta, entry)) = entries.next_dfs().map_err(|_error| Error::MalformedDwarf)? {
        spend(
            &mut budget.entries,
            1,
            budget.limits.entries,
            Limit::Entries,
        )?;
        depth = depth.checked_add(delta).ok_or(Error::MalformedDwarf)?;
        let level = usize::try_from(depth).map_err(|_error| Error::MalformedDwarf)?;
        if level > budget.limits.depth {
            return Err(Error::Limit(Limit::Depth));
        }
        scopes.truncate(level);
        if scopes.len() != level {
            return Err(Error::MalformedDwarf);
        }
        let leaf_name = if named_scope(entry.tag()) {
            entry_name(dwarf, unit, entry, budget)?
        } else {
            None
        };
        if structural(entry.tag())
            && !declaration(entry)?
            && let Some(type_name) = &leaf_name
            && let Some(path) = qualified_name(&scopes, type_name, budget)?
            && wanted.contains(path.as_str())
        {
            let field = type_field(entry, &path)?;
            if result.get(&path).is_some_and(|previous| *previous != field) {
                return Err(Error::ConflictingType(path));
            }
            result.insert(path, field);
        }
        scopes.push(match (named_scope(entry.tag()), leaf_name) {
            (_, Some(scope_name)) => Scope::Named(scope_name),
            (true, None) => Scope::Opaque,
            (false, None) => Scope::Transparent,
        });
    }
    Ok(())
}

fn entry_name<R: Reader<Offset = usize>>(
    dwarf: &Dwarf<R>,
    unit: &Unit<R>,
    entry: &DebuggingInformationEntry<'_, '_, R>,
    budget: &mut ReadBudget,
) -> Result<Option<String>, Error> {
    let Some(value) = entry
        .attr_value(gimli::DW_AT_name)
        .map_err(|_error| Error::MalformedDwarf)?
    else {
        return Ok(None);
    };
    let raw_name = dwarf
        .attr_string(unit, value)
        .map_err(|_error| Error::MalformedDwarf)?;
    budget.name(raw_name.len())?;
    let name = raw_name
        .to_string()
        .map_err(|_error| Error::MalformedDwarf)?
        .into_owned();
    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some(name))
}

fn qualified_name(
    scopes: &[Scope],
    name: &str,
    budget: &mut ReadBudget,
) -> Result<Option<String>, Error> {
    let mut bytes = name.len();
    for scope in scopes {
        match scope {
            Scope::Named(parent) => {
                bytes = bytes
                    .checked_add(parent.len())
                    .and_then(|size| size.checked_add(2))
                    .ok_or(Error::Limit(Limit::Names))?;
            }
            Scope::Opaque => return Ok(None),
            Scope::Transparent => {}
        }
    }
    budget.name(bytes)?;
    let mut path = String::with_capacity(bytes);
    for scope in scopes {
        if let Scope::Named(parent) = scope {
            path.push_str(parent);
            path.push_str("::");
        }
    }
    path.push_str(name);
    Ok(Some(path))
}

fn declaration<R: Reader<Offset = usize>>(
    entry: &DebuggingInformationEntry<'_, '_, R>,
) -> Result<bool, Error> {
    match entry
        .attr_value(gimli::DW_AT_declaration)
        .map_err(|_error| Error::MalformedDwarf)?
    {
        None | Some(AttributeValue::Flag(false)) => Ok(false),
        Some(AttributeValue::Flag(true)) => Ok(true),
        _ => Err(Error::MalformedDwarf),
    }
}

fn type_field<R: Reader<Offset = usize>>(
    entry: &DebuggingInformationEntry<'_, '_, R>,
    name: &str,
) -> Result<NativeField, Error> {
    let size = entry
        .attr_value(gimli::DW_AT_byte_size)
        .map_err(|_error| Error::MalformedDwarf)?
        .and_then(|value| value.udata_value())
        .ok_or_else(|| Error::MissingByteSize(name.to_owned()))?;
    let encoded_alignment = entry
        .attr_value(gimli::DW_AT_alignment)
        .map_err(|_error| Error::MalformedDwarf)?
        .and_then(|value| value.udata_value())
        .ok_or_else(|| Error::MissingAlignment(name.to_owned()))?;
    let bytes = usize::try_from(size).map_err(|_error| Error::InvalidLayout(name.to_owned()))?;
    let alignment = usize::try_from(encoded_alignment)
        .map_err(|_error| Error::InvalidLayout(name.to_owned()))?;
    let field = NativeField { bytes, alignment };
    field
        .validate()
        .map_err(|_error| Error::InvalidLayout(name.to_owned()))?;
    Ok(field)
}

#[cfg(test)]
mod tests {
    use super::{Error, Limit, Limits, NativeField, dwarf_layouts, layouts, layouts_with_limits};
    use gimli::write::{
        AttributeValue, DwarfUnit, EndianVec, RelocateWriter, Relocation, RelocationTarget,
        Sections,
    };
    use gimli::{DwarfSections, Encoding, EndianSlice, Format, LittleEndian, SectionId};
    use object::write as object_write;
    use object::{
        Architecture, BinaryFormat, Endianness, Object as _, ObjectSection as _,
        RelocationEncoding, RelocationFlags, RelocationKind, SectionKind,
    };
    use std::collections::BTreeMap;
    use std::convert::Infallible;

    const AST_FRAME: &str = "fake_crate::ast::parse::Frame";
    const HIR_FRAME: &str = "fake_crate::hir::translate::Frame";

    #[derive(Clone, Copy)]
    struct FakeType {
        path: &'static str,
        size: Option<u64>,
        alignment: Option<u64>,
        tag: gimli::DwTag,
        declaration: bool,
    }

    fn fake_type(path: &'static str, size: u64, alignment: u64) -> FakeType {
        FakeType {
            path,
            size: Some(size),
            alignment: Some(alignment),
            tag: gimli::DW_TAG_structure_type,
            declaration: false,
        }
    }

    fn limits() -> Limits {
        // Fixture-reader caps, not constructor/serving memory allowances.
        Limits {
            object_bytes: 0x4000,
            debug_bytes: 0x4000,
            units: 16,
            entries: 256,
            depth: 16,
            name_bytes: 4096,
            requests: 16,
        }
    }

    fn dwarf_fixture(types: &[FakeType]) -> DwarfUnit {
        let mut dwarf = DwarfUnit::new(Encoding {
            address_size: 8,
            format: Format::Dwarf32,
            version: 5,
        });
        // All real names have nonzero string offsets. Ignoring relocations
        // would therefore resolve them to this unrelated string instead.
        dwarf.strings.add(b"fake_relocation_padding");
        for ty in types {
            let mut parts = ty.path.split("::").peekable();
            let mut parent = dwarf.unit.root();
            while let Some(part) = parts.next() {
                let tag = if parts.peek().is_some() {
                    gimli::DW_TAG_namespace
                } else {
                    ty.tag
                };
                let entry = dwarf.unit.add(parent, tag);
                let name = dwarf.strings.add(part.as_bytes());
                dwarf
                    .unit
                    .get_mut(entry)
                    .set(gimli::DW_AT_name, AttributeValue::StringRef(name));
                parent = entry;
            }
            let entry = dwarf.unit.get_mut(parent);
            if let Some(size) = ty.size {
                entry.set(gimli::DW_AT_byte_size, AttributeValue::Udata(size));
            }
            if let Some(alignment) = ty.alignment {
                entry.set(gimli::DW_AT_alignment, AttributeValue::Udata(alignment));
            }
            if ty.declaration {
                entry.set(gimli::DW_AT_declaration, AttributeValue::Flag(true));
            }
        }
        dwarf
    }

    #[derive(Clone)]
    struct RelocSection {
        writer: EndianVec<LittleEndian>,
        relocations: Vec<Relocation>,
    }

    impl RelocateWriter for RelocSection {
        type Writer = EndianVec<LittleEndian>;

        fn writer(&self) -> &Self::Writer {
            &self.writer
        }

        fn writer_mut(&mut self) -> &mut Self::Writer {
            &mut self.writer
        }

        fn relocate(&mut self, relocation: Relocation) {
            self.relocations.push(relocation);
        }
    }

    fn native_fixture(types: &[FakeType], format: BinaryFormat) -> Vec<u8> {
        let mut sections = Sections::new(RelocSection {
            writer: EndianVec::new(LittleEndian),
            relocations: Vec::new(),
        });
        dwarf_fixture(types).write(&mut sections).unwrap();
        let mut object =
            object_write::Object::new(format, Architecture::Aarch64, Endianness::Little);
        let mut ids = Vec::new();
        sections
            .for_each(|id, section| {
                if !section.writer.slice().is_empty() {
                    let name = if format == BinaryFormat::MachO {
                        format!("__{}", id.name().trim_start_matches('.')).into_bytes()
                    } else {
                        id.name().as_bytes().to_vec()
                    };
                    let index = object.add_section(b"__DWARF".to_vec(), name, SectionKind::Debug);
                    object.append_section_data(index, section.writer.slice(), 1);
                    ids.push((id, index));
                }
                Ok::<_, Infallible>(())
            })
            .unwrap();
        add_native_relocations(&mut object, &sections, &ids);
        object.write().unwrap()
    }

    fn add_native_relocations(
        object: &mut object_write::Object<'_>,
        sections: &Sections<RelocSection>,
        ids: &[(SectionId, object_write::SectionId)],
    ) {
        sections
            .for_each(|id, section| {
                for relocation in &section.relocations {
                    let RelocationTarget::Section(section_target) = relocation.target else {
                        panic!("fixture has only section relocations");
                    };
                    let target_section = ids
                        .iter()
                        .find(|(section_id, _)| *section_id == section_target)
                        .unwrap()
                        .1;
                    let symbol = object.section_symbol(target_section);
                    let source = ids.iter().find(|(other, _)| *other == id).unwrap().1;
                    object
                        .add_relocation(
                            source,
                            object_write::Relocation {
                                offset: u64::try_from(relocation.offset).unwrap(),
                                symbol,
                                addend: relocation.addend,
                                flags: RelocationFlags::Generic {
                                    kind: RelocationKind::Absolute,
                                    encoding: RelocationEncoding::Generic,
                                    size: relocation.size.checked_mul(8).unwrap(),
                                },
                            },
                        )
                        .unwrap();
                }
                Ok::<_, Infallible>(())
            })
            .unwrap();
    }

    fn raw_sections(types: &[FakeType]) -> Sections<EndianVec<LittleEndian>> {
        let mut sections = Sections::new(EndianVec::new(LittleEndian));
        // Without RelocateWriter, gimli writes section-relative offsets. These
        // model already-linked DWARF, not relocatable Wasm DWARF.
        dwarf_fixture(types).write(&mut sections).unwrap();
        sections
    }

    fn append_uleb(bytes: &mut Vec<u8>, size: usize) {
        let mut remaining = u32::try_from(size).unwrap();
        loop {
            let mut byte = u8::try_from(remaining & 0x7f).unwrap();
            remaining >>= 7_u32;
            if remaining != 0 {
                byte |= 0x80;
            }
            bytes.push(byte);
            if remaining == 0 {
                break;
            }
        }
    }

    fn append_custom(bytes: &mut Vec<u8>, name: &str, data: &[u8]) {
        let mut custom = Vec::new();
        append_uleb(&mut custom, name.len());
        custom.extend_from_slice(name.as_bytes());
        custom.extend_from_slice(data);
        bytes.push(0);
        append_uleb(bytes, custom.len());
        bytes.extend(custom);
    }

    fn wasm_fixture(types: &[FakeType]) -> Vec<u8> {
        let mut bytes = b"\0asm\x01\0\0\0".to_vec();
        raw_sections(types)
            .for_each(|id, section| {
                if !section.slice().is_empty() {
                    append_custom(&mut bytes, id.name(), section.slice());
                }
                Ok::<_, Infallible>(())
            })
            .unwrap();
        bytes
    }

    fn expected() -> BTreeMap<String, NativeField> {
        BTreeMap::from([
            (
                AST_FRAME.to_owned(),
                NativeField {
                    bytes: 24,
                    alignment: 8,
                },
            ),
            (
                HIR_FRAME.to_owned(),
                NativeField {
                    bytes: 48,
                    alignment: 8,
                },
            ),
        ])
    }

    #[test]
    fn compiler_layout_native_qualified_names_and_library_relocations() {
        for format in [BinaryFormat::Elf, BinaryFormat::MachO] {
            let bytes = native_fixture(
                &[fake_type(AST_FRAME, 24, 8), fake_type(HIR_FRAME, 48, 8)],
                format,
            );
            let object = object::File::parse(bytes.as_slice()).unwrap();
            assert!(
                object
                    .sections()
                    .any(|section| section.relocations().next().is_some())
            );
            assert_eq!(layouts(&bytes, &[AST_FRAME, HIR_FRAME]), Ok(expected()));
        }
    }

    #[test]
    fn compiler_layout_shared_dwarf_core_reads_section_relative_offsets() {
        let sections = raw_sections(&[fake_type(AST_FRAME, 24, 8), fake_type(HIR_FRAME, 48, 8)]);
        let loaded = DwarfSections::load(|id| {
            Ok::<_, Infallible>(sections.get(id).map_or(&[][..], |section| section.slice()))
        })
        .unwrap();
        let dwarf = loaded.borrow(|section| EndianSlice::new(section, LittleEndian));
        assert_eq!(
            dwarf_layouts(&dwarf, &[AST_FRAME, HIR_FRAME], limits()),
            Ok(expected())
        );
    }

    #[test]
    fn compiler_layout_linked_core_wasm_uses_the_same_qualified_facts() {
        let bytes = wasm_fixture(&[fake_type(AST_FRAME, 24, 8), fake_type(HIR_FRAME, 48, 8)]);
        assert_eq!(layouts(&bytes, &[AST_FRAME, HIR_FRAME]), Ok(expected()));
    }

    #[test]
    fn compiler_layout_wasm_refuses_any_relocation_section_even_after_debug_data() {
        let mut bytes = wasm_fixture(&[fake_type(AST_FRAME, 24, 8)]);
        append_custom(&mut bytes, "reloc..debug_info", &[]);
        assert_eq!(layouts(&bytes, &[AST_FRAME]), Err(Error::RelocatableWasm));
        let mut before = b"\0asm\x01\0\0\0".to_vec();
        append_custom(&mut before, "reloc..debug_str", &[]);
        before.extend_from_slice(&wasm_fixture(&[fake_type(AST_FRAME, 24, 8)])[8..]);
        assert_eq!(layouts(&before, &[AST_FRAME]), Err(Error::RelocatableWasm));
    }

    #[test]
    fn compiler_layout_requires_exact_namespace_not_a_leaf_or_suffix_match() {
        let bytes = native_fixture(&[fake_type(AST_FRAME, 24, 8)], BinaryFormat::Elf);
        for wrong in [
            "Frame",
            "ast::parse::Frame",
            "fake_crate::hir::translate::Frame",
        ] {
            assert_eq!(
                layouts(&bytes, &[wrong]),
                Err(Error::MissingType(wrong.to_owned()))
            );
        }
    }

    #[test]
    fn compiler_layout_identical_duplicate_definitions_merge() {
        let ty = fake_type(AST_FRAME, 24, 8);
        let bytes = native_fixture(&[ty, ty], BinaryFormat::Elf);
        assert_eq!(
            layouts(&bytes, &[AST_FRAME]),
            Ok(BTreeMap::from([(
                AST_FRAME.to_owned(),
                NativeField {
                    bytes: 24,
                    alignment: 8
                }
            ),]))
        );
    }

    #[test]
    fn compiler_layout_conflicting_size_or_alignment_refuses() {
        for conflicting in [fake_type(AST_FRAME, 32, 8), fake_type(AST_FRAME, 24, 4)] {
            let bytes = native_fixture(
                &[fake_type(AST_FRAME, 24, 8), conflicting],
                BinaryFormat::Elf,
            );
            assert_eq!(
                layouts(&bytes, &[AST_FRAME]),
                Err(Error::ConflictingType(AST_FRAME.to_owned()))
            );
        }
    }

    #[test]
    fn compiler_layout_missing_size_or_alignment_is_not_an_inferred_fact() {
        let mut missing_size = fake_type(AST_FRAME, 24, 8);
        missing_size.size = None;
        let missing_size_bytes = native_fixture(&[missing_size], BinaryFormat::Elf);
        assert_eq!(
            layouts(&missing_size_bytes, &[AST_FRAME]),
            Err(Error::MissingByteSize(AST_FRAME.to_owned()))
        );
        let mut missing_alignment = fake_type(AST_FRAME, 24, 8);
        missing_alignment.alignment = None;
        let missing_alignment_bytes = native_fixture(&[missing_alignment], BinaryFormat::Elf);
        assert_eq!(
            layouts(&missing_alignment_bytes, &[AST_FRAME]),
            Err(Error::MissingAlignment(AST_FRAME.to_owned()))
        );
    }

    #[test]
    fn compiler_layout_only_complete_structural_type_definitions_count() {
        for tag in [
            gimli::DW_TAG_structure_type,
            gimli::DW_TAG_enumeration_type,
            gimli::DW_TAG_union_type,
        ] {
            let mut ty = fake_type(AST_FRAME, 24, 8);
            ty.tag = tag;
            let bytes = native_fixture(&[ty], BinaryFormat::Elf);
            assert_eq!(layouts(&bytes, &[AST_FRAME]).unwrap()[AST_FRAME].bytes, 24);
        }
        let mut namespace = fake_type(AST_FRAME, 24, 8);
        namespace.tag = gimli::DW_TAG_namespace;
        let namespace_bytes = native_fixture(&[namespace], BinaryFormat::Elf);
        assert_eq!(
            layouts(&namespace_bytes, &[AST_FRAME]),
            Err(Error::MissingType(AST_FRAME.to_owned()))
        );
        let mut declaration = fake_type(AST_FRAME, 24, 8);
        declaration.declaration = true;
        declaration.size = None;
        declaration.alignment = None;
        let declaration_bytes = native_fixture(
            &[declaration, fake_type(AST_FRAME, 24, 8)],
            BinaryFormat::Elf,
        );
        assert_eq!(
            layouts(&declaration_bytes, &[AST_FRAME]).unwrap()[AST_FRAME].bytes,
            24
        );
    }

    #[test]
    fn compiler_layout_invalid_sizes_and_alignments_refuse() {
        for (size, alignment) in [(0, 8), (24, 0), (24, 3), (24, 16)] {
            let bytes = native_fixture(&[fake_type(AST_FRAME, size, alignment)], BinaryFormat::Elf);
            assert_eq!(
                layouts(&bytes, &[AST_FRAME]),
                Err(Error::InvalidLayout(AST_FRAME.to_owned()))
            );
        }
    }

    #[test]
    fn compiler_layout_invalid_objects_requests_and_missing_debug_refuse() {
        assert_eq!(
            layouts(b"not an object", &[AST_FRAME]),
            Err(Error::InvalidObject)
        );
        let bytes = wasm_fixture(&[fake_type(AST_FRAME, 24, 8)]);
        assert_eq!(layouts(&bytes, &[]), Err(Error::InvalidRequest));
        assert_eq!(
            layouts(&bytes, &[AST_FRAME, AST_FRAME]),
            Err(Error::InvalidRequest)
        );
        assert_eq!(
            layouts(b"\0asm\x01\0\0\0", &[AST_FRAME]),
            Err(Error::MissingDwarf)
        );
    }

    #[test]
    fn compiler_layout_all_reader_work_dimensions_have_explicit_caps() {
        let bytes = wasm_fixture(&[fake_type(AST_FRAME, 24, 8)]);
        let cases = [
            (
                Limits {
                    object_bytes: bytes.len() - 1,
                    ..limits()
                },
                Limit::ObjectBytes,
            ),
            (
                Limits {
                    debug_bytes: 1,
                    ..limits()
                },
                Limit::DebugBytes,
            ),
            (
                Limits {
                    units: 0,
                    ..limits()
                },
                Limit::Units,
            ),
            (
                Limits {
                    entries: 0,
                    ..limits()
                },
                Limit::Entries,
            ),
            (
                Limits {
                    depth: 0,
                    ..limits()
                },
                Limit::Depth,
            ),
            (
                Limits {
                    name_bytes: 0,
                    ..limits()
                },
                Limit::Names,
            ),
            (
                Limits {
                    requests: 0,
                    ..limits()
                },
                Limit::Requests,
            ),
        ];
        for (limits, reason) in cases {
            assert_eq!(
                layouts_with_limits(&bytes, &[AST_FRAME], limits),
                Err(Error::Limit(reason))
            );
        }
    }
}

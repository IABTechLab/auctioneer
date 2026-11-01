//! Fail-closed native DWARF reads with opaque ELF TLS location operands.
//!
//! TLS addresses are never synthesized. An opaque expression can retain its
//! bytes, but reading any part of its unsupported relocation is a refusal.
//! Supported address/offset relocations remain delegated to object.

use gimli::{EndianSlice, Reader as _, ReaderOffsetId, RunTimeEndian};
use object::{Object as _, ObjectSection as _, ObjectSymbol as _, elf};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

#[derive(Debug, Default)]
pub(crate) struct Relocations {
    supported: object::RelocationMap,
    deferred: BTreeMap<u64, u64>,
}

impl Relocations {
    pub(crate) fn load(
        file: &object::File<'_>,
        section: &object::Section<'_, '_>,
    ) -> Result<Self, String> {
        let mut result = Self::default();
        let mut offsets = BTreeSet::new();
        for (offset, relocation) in section.relocations() {
            if !offsets.insert(offset) {
                return Err("Multiple relocations for offset".into());
            }
            let deferred_width = match (
                file.format(),
                file.architecture(),
                section.name().map_err(|error| error.to_string())?,
                relocation.flags(),
            ) {
                (
                    object::BinaryFormat::Elf,
                    object::Architecture::X86_64,
                    ".debug_info",
                    object::RelocationFlags::Elf {
                        r_type: elf::R_X86_64_DTPOFF32,
                    },
                ) => Some(4),
                (
                    object::BinaryFormat::Elf,
                    object::Architecture::X86_64,
                    ".debug_info",
                    object::RelocationFlags::Elf {
                        r_type: elf::R_X86_64_DTPOFF64,
                    },
                ) => Some(8),
                _ => None,
            };
            if let Some(width) = deferred_width {
                let object::RelocationTarget::Symbol(index) = relocation.target() else {
                    return Err("unsupported DWARF TLS relocation target".into());
                };
                let symbol = file
                    .symbol_by_index(index)
                    .map_err(|error| error.to_string())?;
                let target = symbol
                    .section_index()
                    .ok_or("undefined DWARF TLS relocation target")?;
                if symbol.kind() != object::SymbolKind::Tls
                    || !matches!(
                        file.section_by_index(target)
                            .map_err(|error| error.to_string())?
                            .kind(),
                        object::SectionKind::Tls | object::SectionKind::UninitializedTls
                    )
                {
                    return Err("non-TLS DWARF TLS relocation target".into());
                }
                let end = offset
                    .checked_add(width)
                    .filter(|end| *end <= section.size())
                    .ok_or("DWARF TLS relocation outside section")?;
                result.deferred.insert(offset, end);
            } else {
                let flags = relocation.flags();
                let kind = relocation.kind();
                result
                    .supported
                    .add(file, offset, relocation)
                    .map_err(|error| {
                        format!(
                            "{}+{offset:#x} {flags:?} ({kind:?}): {error}",
                            section.name().unwrap_or("<unnamed>")
                        )
                    })?;
            }
        }
        Ok(result)
    }

    fn check(&self, offset: usize, len: usize) -> gimli::Result<()> {
        if len == 0 {
            return Ok(());
        }
        let start = u64::try_from(offset).map_err(|_error| gimli::Error::UnsupportedOffset)?;
        let size = u64::try_from(len).map_err(|_error| gimli::Error::UnsupportedOffset)?;
        let end = start
            .checked_add(size)
            .ok_or(gimli::Error::UnsupportedOffset)?;
        // Deferred TLS operands are only 4 or 8 bytes wide, so no overlapping
        // span can start more than 7 bytes before this read.
        if self
            .deferred
            .range(start.saturating_sub(7)..end)
            .any(|(_, deferred_end)| *deferred_end > start)
        {
            return Err(gimli::Error::UnsupportedOffset);
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Reader<'a> {
    section: EndianSlice<'a, RunTimeEndian>,
    cursor: EndianSlice<'a, RunTimeEndian>,
    relocations: Rc<Relocations>,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8], endian: RunTimeEndian, relocations: Rc<Relocations>) -> Self {
        let section = EndianSlice::new(data, endian);
        Self {
            section,
            cursor: section,
            relocations,
        }
    }

    fn check_read(&self, len: usize) -> gimli::Result<()> {
        self.relocations
            .check(gimli::Reader::offset_from(&self.cursor, &self.section), len)
    }
}

impl gimli::Reader for Reader<'_> {
    type Endian = RunTimeEndian;
    type Offset = usize;

    fn read_address(&mut self, address_size: u8) -> gimli::Result<u64> {
        self.check_read(usize::from(address_size))?;
        let offset = gimli::Reader::offset_from(&self.cursor, &self.section);
        let value = self.cursor.read_address(address_size)?;
        Ok(self.relocations.supported.relocate(
            u64::try_from(offset).map_err(|_error| gimli::Error::UnsupportedOffset)?,
            value,
        ))
    }

    fn read_offset(&mut self, format: gimli::Format) -> gimli::Result<usize> {
        self.read_sized_offset(format.word_size())
    }

    fn read_sized_offset(&mut self, size: u8) -> gimli::Result<usize> {
        self.check_read(usize::from(size))?;
        let offset = gimli::Reader::offset_from(&self.cursor, &self.section);
        let value = self.cursor.read_sized_offset(size)?;
        let relocated = self.relocations.supported.relocate(
            u64::try_from(offset).map_err(|_error| gimli::Error::UnsupportedOffset)?,
            u64::try_from(value).map_err(|_error| gimli::Error::UnsupportedOffset)?,
        );
        usize::try_from(relocated).map_err(|_error| gimli::Error::UnsupportedOffset)
    }

    fn read_slice(&mut self, buf: &mut [u8]) -> gimli::Result<()> {
        self.check_read(buf.len())?;
        self.cursor.read_slice(buf)
    }

    fn split(&mut self, len: usize) -> gimli::Result<Self> {
        let mut other = self.clone();
        other.cursor.truncate(len)?;
        self.cursor.skip(len)?;
        Ok(other)
    }

    fn endian(&self) -> Self::Endian {
        self.cursor.endian()
    }

    fn len(&self) -> usize {
        self.cursor.len()
    }

    fn empty(&mut self) {
        self.cursor.empty();
    }

    fn truncate(&mut self, len: usize) -> gimli::Result<()> {
        self.cursor.truncate(len)
    }

    fn offset_from(&self, base: &Self) -> usize {
        gimli::Reader::offset_from(&self.cursor, &base.cursor)
    }

    fn offset_id(&self) -> ReaderOffsetId {
        self.cursor.offset_id()
    }

    fn lookup_offset_id(&self, id: ReaderOffsetId) -> Option<usize> {
        self.cursor.lookup_offset_id(id)
    }

    fn find(&self, byte: u8) -> gimli::Result<usize> {
        let offset = gimli::Reader::find(&self.cursor, byte)?;
        self.check_read(
            offset
                .checked_add(1)
                .ok_or(gimli::Error::UnsupportedOffset)?,
        )?;
        Ok(offset)
    }

    fn skip(&mut self, len: usize) -> gimli::Result<()> {
        // Opaque expression blocks may be skipped, but a split reader retains
        // the guard and cannot read or evaluate an unsupported TLS operand.
        self.cursor.skip(len)
    }

    fn to_slice(&self) -> gimli::Result<Cow<'_, [u8]>> {
        self.check_read(self.len())?;
        self.cursor.to_slice()
    }

    fn to_string(&self) -> gimli::Result<Cow<'_, str>> {
        self.check_read(self.len())?;
        gimli::Reader::to_string(&self.cursor)
    }

    fn to_string_lossy(&self) -> gimli::Result<Cow<'_, str>> {
        self.check_read(self.len())?;
        gimli::Reader::to_string_lossy(&self.cursor)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use gimli::write::{
        AttributeValue, DwarfUnit, EndianVec, Expression, RelocateWriter, Relocation,
        RelocationTarget, Sections, Writer as _,
    };
    use gimli::{Encoding, Format, LittleEndian, SectionId};
    use object::write::{
        Object, Relocation as ObjectRelocation, SectionId as ObjectSectionId, Symbol, SymbolSection,
    };
    use object::{
        Architecture, BinaryFormat, Endianness, RelocationFlags, SectionFlags, SectionKind,
        SymbolKind,
    };
    use std::convert::Infallible;

    pub(crate) const PRODUCER: &str = "clang LLVM (rustc version 1.98.1 (fakehash 2026-10-12))";
    #[derive(Clone, Copy)]
    pub(crate) enum Site {
        TlsExpression,
        Tls32Expression,
        TlsProducer,
        RelativeExpression,
    }

    #[derive(Clone)]
    struct Section {
        data: EndianVec<LittleEndian>,
        relocations: Vec<Relocation>,
    }

    impl RelocateWriter for Section {
        type Writer = EndianVec<LittleEndian>;

        fn writer(&self) -> &Self::Writer {
            &self.data
        }

        fn writer_mut(&mut self) -> &mut Self::Writer {
            &mut self.data
        }

        fn relocate(&mut self, relocation: Relocation) {
            self.relocations.push(relocation);
        }
    }

    pub(crate) fn elf_fixture(site: Site) -> Vec<u8> {
        elf_fixture_with(site, |_, _, _| {})
    }

    fn expression_bytes(site: Site) -> Vec<u8> {
        let mut expression = EndianVec::new(LittleEndian);
        if matches!(site, Site::Tls32Expression) {
            expression.write_u8(gimli::DW_OP_const4u.0).unwrap();
            expression.write_u32(0x4433_2211).unwrap();
        } else {
            expression.write_u8(gimli::DW_OP_const8u.0).unwrap();
            expression.write_u64(0x8877_6655_4433_2211).unwrap();
        }
        expression
            .write_u8(gimli::DW_OP_form_tls_address.0)
            .unwrap();
        expression.into_vec()
    }

    fn dwarf_sections(site: Site) -> (Sections<Section>, usize) {
        let mut dwarf = DwarfUnit::new(Encoding {
            format: Format::Dwarf32,
            version: 4,
            address_size: 8,
        });
        let root = dwarf.unit.root();
        let producer = dwarf.strings.add(PRODUCER.as_bytes());
        dwarf
            .unit
            .get_mut(root)
            .set(gimli::DW_AT_producer, AttributeValue::StringRef(producer));
        let namespace = dwarf.unit.add(root, gimli::DW_TAG_namespace);
        dwarf
            .unit
            .get_mut(namespace)
            .set(gimli::DW_AT_name, AttributeValue::String(b"fake".to_vec()));
        let ty = dwarf.unit.add(namespace, gimli::DW_TAG_structure_type);
        let type_entry = dwarf.unit.get_mut(ty);
        type_entry.set(gimli::DW_AT_name, AttributeValue::String(b"Frame".to_vec()));
        type_entry.set(gimli::DW_AT_byte_size, AttributeValue::Udata(24));
        type_entry.set(gimli::DW_AT_alignment, AttributeValue::Udata(8));
        let expression = expression_bytes(site);
        let variable = dwarf.unit.add(root, gimli::DW_TAG_variable);
        let variable_entry = dwarf.unit.get_mut(variable);
        variable_entry.set(
            gimli::DW_AT_name,
            AttributeValue::String(b"fake_tls".to_vec()),
        );
        variable_entry.set(
            gimli::DW_AT_location,
            AttributeValue::Exprloc(Expression::raw(expression.clone())),
        );
        let mut sections = Sections::new(Section {
            data: EndianVec::new(LittleEndian),
            relocations: Vec::new(),
        });
        dwarf.write(&mut sections).unwrap();
        let info = &sections.debug_info;
        let expression_offset = info
            .data
            .slice()
            .windows(expression.len())
            .position(|bytes| bytes == expression)
            .unwrap();
        let producer_offset = info
            .relocations
            .iter()
            .find(|relocation| {
                matches!(
                    relocation.target,
                    RelocationTarget::Section(SectionId::DebugStr)
                )
            })
            .unwrap()
            .offset;
        let unsupported_offset = match site {
            Site::TlsProducer => producer_offset,
            Site::TlsExpression | Site::Tls32Expression | Site::RelativeExpression => {
                expression_offset.checked_add(1).unwrap()
            }
        };
        (sections, unsupported_offset)
    }

    fn object_sections(
        sections: &Sections<Section>,
    ) -> (Object<'static>, Vec<(SectionId, ObjectSectionId)>) {
        let mut object = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
        let mut ids = Vec::new();
        sections
            .for_each(|id, section| {
                if !section.data.slice().is_empty() {
                    let index =
                        object.add_section(Vec::new(), id.name().into(), SectionKind::Debug);
                    object.append_section_data(index, section.data.slice(), 1);
                    ids.push((id, index));
                }
                Ok::<_, Infallible>(())
            })
            .unwrap();
        (object, ids)
    }

    fn relocate_sections(
        object: &mut Object<'_>,
        sections: &Sections<Section>,
        ids: &[(SectionId, ObjectSectionId)],
        replaced_offset: Option<usize>,
    ) {
        sections
            .for_each(|id, section| {
                for relocation in &section.relocations {
                    if id == SectionId::DebugInfo && Some(relocation.offset) == replaced_offset {
                        continue;
                    }
                    let RelocationTarget::Section(target_id) = relocation.target else {
                        panic!("fixture section relocation")
                    };
                    let source = ids
                        .iter()
                        .find(|(candidate, _)| *candidate == id)
                        .unwrap()
                        .1;
                    let target_section = ids
                        .iter()
                        .find(|(candidate, _)| *candidate == target_id)
                        .unwrap()
                        .1;
                    let symbol = object.section_symbol(target_section);
                    object
                        .add_relocation(
                            source,
                            ObjectRelocation {
                                offset: u64::try_from(relocation.offset).unwrap(),
                                symbol,
                                addend: relocation.addend,
                                flags: RelocationFlags::Generic {
                                    kind: object::RelocationKind::Absolute,
                                    encoding: object::RelocationEncoding::Generic,
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

    fn elf_fixture_with(
        site: Site,
        configure: impl FnOnce(&mut Object<'_>, ObjectSectionId, &mut ObjectRelocation),
    ) -> Vec<u8> {
        let (sections, unsupported_offset) = dwarf_sections(site);
        let (mut object, ids) = object_sections(&sections);
        relocate_sections(
            &mut object,
            &sections,
            &ids,
            matches!(site, Site::TlsProducer).then_some(unsupported_offset),
        );
        let tls = object.add_section(Vec::new(), b".tdata".to_vec(), SectionKind::Tls);
        object.append_section_data(tls, &[0; 8], 8);
        let symbol = object.add_symbol(Symbol {
            name: b"fake_tls".to_vec(),
            value: 0,
            size: 8,
            kind: SymbolKind::Tls,
            scope: object::SymbolScope::Compilation,
            weak: false,
            section: SymbolSection::Section(tls),
            flags: object::SymbolFlags::None,
        });
        let info_section = ids
            .iter()
            .find(|(id, _)| *id == SectionId::DebugInfo)
            .unwrap()
            .1;
        let mut tls_relocation = ObjectRelocation {
            offset: u64::try_from(unsupported_offset).unwrap(),
            symbol,
            addend: 0,
            flags: RelocationFlags::Elf {
                r_type: if matches!(site, Site::RelativeExpression) {
                    elf::R_X86_64_PC32
                } else if matches!(site, Site::Tls32Expression) {
                    elf::R_X86_64_DTPOFF32
                } else {
                    elf::R_X86_64_DTPOFF64
                },
            },
        };
        configure(&mut object, info_section, &mut tls_relocation);
        object.add_relocation(info_section, tls_relocation).unwrap();
        object.write().unwrap()
    }

    fn assert_valid_tls_fixture(bytes: &[u8], width: u64) -> (u64, u64) {
        let file = object::File::parse(bytes).unwrap();
        let section = file.section_by_name(".debug_info").unwrap();
        let symbol = file
            .symbols()
            .find(|symbol| symbol.name() == Ok("fake_tls"))
            .unwrap();
        assert!(!symbol.is_undefined());
        assert_eq!(symbol.kind(), SymbolKind::Tls);
        assert_eq!(
            file.section_by_index(symbol.section_index().unwrap())
                .unwrap()
                .kind(),
            SectionKind::Tls
        );
        let relocations = Relocations::load(&file, &section).unwrap();
        assert_eq!(relocations.deferred.len(), 1);
        let (&start, &end) = relocations.deferred.iter().next().unwrap();
        assert_eq!(end.checked_sub(start), Some(width));
        assert!(end <= section.size());
        (start, end)
    }

    fn selected_reader<'a>(reader: &Reader<'a>, start: usize, len: usize) -> Reader<'a> {
        let mut selected = reader.clone();
        selected.skip(start).unwrap();
        selected.truncate(len).unwrap();
        selected
    }

    #[test]
    fn elf_tls_relocations_refuse_undefined_symbols() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            assert_valid_tls_fixture(&elf_fixture(site), width);
            let bytes = elf_fixture_with(site, |object, _, relocation| {
                let symbol = object.symbol_mut(relocation.symbol);
                assert!(matches!(symbol.section, SymbolSection::Section(_)));
                symbol.section = SymbolSection::Undefined;
            });
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            assert_eq!(
                Relocations::load(&file, &section).unwrap_err(),
                "undefined DWARF TLS relocation target"
            );
        }
    }

    #[test]
    fn elf_tls_relocations_refuse_non_tls_symbols() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            assert_valid_tls_fixture(&elf_fixture(site), width);
            let bytes = elf_fixture_with(site, |object, _, relocation| {
                let symbol = object.symbol_mut(relocation.symbol);
                assert_eq!(symbol.kind, SymbolKind::Tls);
                symbol.kind = SymbolKind::Data;
            });
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            assert_eq!(
                Relocations::load(&file, &section).unwrap_err(),
                "non-TLS DWARF TLS relocation target"
            );
        }
    }

    #[test]
    fn elf_tls_relocations_refuse_non_tls_sections() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            assert_valid_tls_fixture(&elf_fixture(site), width);
            let bytes = elf_fixture_with(site, |object, _, relocation| {
                let SymbolSection::Section(target) = object.symbol(relocation.symbol).section
                else {
                    panic!("fixture TLS symbol must have a section");
                };
                assert_eq!(object.section(target).flags, SectionFlags::None);
                object.section_mut(target).flags = SectionFlags::Elf {
                    sh_flags: u64::from(elf::SHF_ALLOC | elf::SHF_WRITE),
                };
            });
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            assert_eq!(
                Relocations::load(&file, &section).unwrap_err(),
                "non-TLS DWARF TLS relocation target"
            );
        }
    }

    #[test]
    fn elf_tls_relocation_end_must_not_exceed_the_section() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            let baseline = elf_fixture(site);
            assert_valid_tls_fixture(&baseline, width);
            let file = object::File::parse(baseline.as_slice()).unwrap();
            let size = file.section_by_name(".debug_info").unwrap().size();
            let at_limit = size.checked_sub(width).unwrap();
            let good = elf_fixture_with(site, |_, _, relocation| relocation.offset = at_limit);
            assert_eq!(assert_valid_tls_fixture(&good, width), (at_limit, size));
            let bad = elf_fixture_with(site, |_, _, relocation| {
                relocation.offset = at_limit.checked_add(1).unwrap();
            });
            let artifact = object::File::parse(bad.as_slice()).unwrap();
            let section = artifact.section_by_name(".debug_info").unwrap();
            assert_eq!(
                Relocations::load(&artifact, &section).unwrap_err(),
                "DWARF TLS relocation outside section"
            );
        }
    }

    #[test]
    fn elf_tls_relocations_refuse_duplicate_offsets() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            assert_valid_tls_fixture(&elf_fixture(site), width);
            let bytes = elf_fixture_with(site, |object, info, relocation| {
                object
                    .add_relocation(
                        info,
                        ObjectRelocation {
                            offset: relocation.offset,
                            symbol: relocation.symbol,
                            addend: relocation.addend,
                            flags: relocation.flags,
                        },
                    )
                    .unwrap();
            });
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            assert_eq!(
                Relocations::load(&file, &section).unwrap_err(),
                "Multiple relocations for offset"
            );
        }
    }

    #[test]
    fn elf_tls_find_refuses_partial_operand_access_but_can_stop_before_it() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            let bytes = elf_fixture(site);
            let (offset, end) = assert_valid_tls_fixture(&bytes, width);
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            let data = section.data().unwrap();
            let relocations = Rc::new(Relocations::load(&file, &section).unwrap());
            let reader = Reader::new(data, RunTimeEndian::Little, relocations);
            let start = usize::try_from(offset.checked_sub(1).unwrap()).unwrap();
            let crossing = selected_reader(&reader, start, 2);
            let opcode = if width == 4 {
                gimli::DW_OP_const4u.0
            } else {
                gimli::DW_OP_const8u.0
            };
            let raw = EndianSlice::new(&data[start..start + 2], RunTimeEndian::Little);
            assert_eq!(gimli::Reader::find(&raw, 0x11), Ok(1));
            assert_eq!(crossing.find(opcode), Ok(0));
            assert_eq!(crossing.find(0x11), Err(gimli::Error::UnsupportedOffset));
            for position in [offset, offset + 1, end - 1] {
                let index = usize::try_from(position).unwrap();
                let selected = selected_reader(&reader, index, 1);
                let plain = EndianSlice::new(&data[index..=index], RunTimeEndian::Little);
                assert_eq!(gimli::Reader::find(&plain, data[index]), Ok(0));
                assert_eq!(
                    selected.find(data[index]),
                    Err(gimli::Error::UnsupportedOffset)
                );
            }
        }
    }

    #[test]
    fn elf_tls_partial_slice_and_string_conversions_retain_the_guard() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            let bytes = elf_fixture(site);
            let (offset, end) = assert_valid_tls_fixture(&bytes, width);
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            let data = section.data().unwrap();
            let relocations = Rc::new(Relocations::load(&file, &section).unwrap());
            let reader = Reader::new(data, RunTimeEndian::Little, relocations);
            let before = selected_reader(&reader, usize::try_from(offset - 1).unwrap(), 1);
            let opcode = if width == 4 {
                gimli::DW_OP_const4u.0
            } else {
                gimli::DW_OP_const8u.0
            };
            assert_eq!(before.to_slice().unwrap().as_ref(), &[opcode]);
            assert_eq!(before.to_string().unwrap().as_bytes(), &[opcode]);
            assert_eq!(before.to_string_lossy().unwrap().as_bytes(), &[opcode]);
            let first = selected_reader(&reader, usize::try_from(offset).unwrap(), 1);
            let mut first_plain = EndianSlice::new(
                &data[usize::try_from(offset).unwrap()..],
                RunTimeEndian::Little,
            );
            first_plain.truncate(1).unwrap();
            assert_eq!(
                gimli::Reader::to_string(&first_plain).unwrap().as_bytes(),
                &[0x11]
            );
            assert_eq!(first.to_string(), Err(gimli::Error::UnsupportedOffset));
            for position in [offset, offset + 1, end - 1] {
                let selected = selected_reader(&reader, usize::try_from(position).unwrap(), 1);
                assert_eq!(selected.to_slice(), Err(gimli::Error::UnsupportedOffset));
                assert_eq!(
                    selected.to_string_lossy(),
                    Err(gimli::Error::UnsupportedOffset)
                );
            }
            let after = selected_reader(&reader, usize::try_from(end).unwrap(), 1);
            assert_eq!(
                after.to_slice().unwrap().as_ref(),
                &[gimli::DW_OP_form_tls_address.0]
            );
        }
    }

    #[test]
    fn elf_tls_reads_starting_before_the_operand_refuse_crossing_its_start() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            let bytes = elf_fixture(site);
            let (offset, _) = assert_valid_tls_fixture(&bytes, width);
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            let data = section.data().unwrap();
            let relocations = Rc::new(Relocations::load(&file, &section).unwrap());
            let reader = Reader::new(data, RunTimeEndian::Little, relocations);
            let start = usize::try_from(offset.checked_sub(1).unwrap()).unwrap();
            let mut before = selected_reader(&reader, start, 1);
            let opcode = if width == 4 {
                gimli::DW_OP_const4u.0
            } else {
                gimli::DW_OP_const8u.0
            };
            assert_eq!(before.read_u8(), Ok(opcode));
            let raw = EndianSlice::new(&data[start..start + 2], RunTimeEndian::Little);
            assert_eq!(
                gimli::Reader::to_string(&raw).unwrap().as_bytes(),
                &[opcode, 0x11]
            );
            let mut crossing = selected_reader(&reader, start, 2);
            let mut buffer = [0; 2];
            assert_eq!(
                crossing.read_slice(&mut buffer),
                Err(gimli::Error::UnsupportedOffset)
            );
            assert_eq!(buffer, [0; 2]);
            assert_eq!(crossing.len(), 2);
            assert_eq!(crossing.to_slice(), Err(gimli::Error::UnsupportedOffset));
            assert_eq!(crossing.to_string(), Err(gimli::Error::UnsupportedOffset));
            assert_eq!(
                crossing.to_string_lossy(),
                Err(gimli::Error::UnsupportedOffset)
            );
        }
    }

    #[test]
    fn elf_tls_overlapping_four_and_eight_byte_spans_keep_the_longer_guard() {
        let baseline = elf_fixture(Site::TlsExpression);
        let (offset, end) = assert_valid_tls_fixture(&baseline, 8);
        let bytes = elf_fixture_with(Site::TlsExpression, |object, info, relocation| {
            object
                .add_relocation(
                    info,
                    ObjectRelocation {
                        offset: relocation.offset.checked_add(1).unwrap(),
                        symbol: relocation.symbol,
                        addend: relocation.addend,
                        flags: RelocationFlags::Elf {
                            r_type: elf::R_X86_64_DTPOFF32,
                        },
                    },
                )
                .unwrap();
        });
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let section = file.section_by_name(".debug_info").unwrap();
        let relocations = Rc::new(Relocations::load(&file, &section).unwrap());
        assert_eq!(
            relocations.deferred,
            BTreeMap::from([(offset, end), (offset + 1, offset + 5)])
        );
        let reader = Reader::new(section.data().unwrap(), RunTimeEndian::Little, relocations);
        for position in [offset + 1, offset + 4, end - 1] {
            let mut selected = selected_reader(&reader, usize::try_from(position).unwrap(), 1);
            assert_eq!(selected.read_u8(), Err(gimli::Error::UnsupportedOffset));
        }
        let mut before = selected_reader(&reader, usize::try_from(offset - 1).unwrap(), 1);
        assert_eq!(before.read_u8(), Ok(gimli::DW_OP_const8u.0));
        let mut after = selected_reader(&reader, usize::try_from(end).unwrap(), 1);
        assert_eq!(after.read_u8(), Ok(gimli::DW_OP_form_tls_address.0));
    }

    #[test]
    fn elf_tls_empty_reads_inside_operands_do_not_access_relocated_bytes() {
        for (site, width) in [(Site::Tls32Expression, 4), (Site::TlsExpression, 8)] {
            let bytes = elf_fixture(site);
            let (offset, end) = assert_valid_tls_fixture(&bytes, width);
            let file = object::File::parse(bytes.as_slice()).unwrap();
            let section = file.section_by_name(".debug_info").unwrap();
            let relocations = Rc::new(Relocations::load(&file, &section).unwrap());
            let reader = Reader::new(section.data().unwrap(), RunTimeEndian::Little, relocations);
            for position in [offset, offset + 1, end - 1] {
                let mut empty = selected_reader(&reader, usize::try_from(position).unwrap(), 0);
                assert_eq!(empty.read_slice(&mut []), Ok(()));
                assert!(empty.to_slice().unwrap().is_empty());
                assert_eq!(empty.to_string().unwrap(), "");
                assert_eq!(empty.to_string_lossy().unwrap(), "");
            }
        }
    }

    // Only 64-bit usize inputs can overflow check's u64 range arithmetic.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn tls_guard_refuses_read_range_overflow_without_wrapping() {
        let start = u64::MAX.checked_sub(8).unwrap();
        let relocations = Relocations {
            deferred: BTreeMap::from([(start, u64::MAX)]),
            ..Relocations::default()
        };
        let before = usize::try_from(start.checked_sub(1).unwrap()).unwrap();
        assert_eq!(relocations.check(before, 1), Ok(()));
        assert_eq!(relocations.check(usize::MAX, 0), Ok(()));
        assert_eq!(
            relocations.check(usize::MAX, 1),
            Err(gimli::Error::UnsupportedOffset)
        );
    }

    #[test]
    fn elf_tls_bytes_refuse_direct_reads_including_partial_overlaps() {
        let bytes = elf_fixture(Site::TlsExpression);
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let section = file.section_by_name(".debug_info").unwrap();
        assert_eq!(
            section.relocation_map().unwrap_err().to_string(),
            "Unsupported relocation type"
        );
        let relocations = Rc::new(Relocations::load(&file, &section).unwrap());
        let (&offset, &end) = relocations.deferred.iter().next().unwrap();
        assert_eq!(end - offset, 8);
        let reader = Reader::new(section.data().unwrap(), RunTimeEndian::Little, relocations);
        for start in [offset, offset + 1, end - 1] {
            let mut selected = reader.clone();
            selected.skip(usize::try_from(start).unwrap()).unwrap();
            assert_eq!(selected.read_u8(), Err(gimli::Error::UnsupportedOffset));
        }
        let mut address = reader.clone();
        address.skip(usize::try_from(offset).unwrap()).unwrap();
        assert_eq!(
            address.read_address(8),
            Err(gimli::Error::UnsupportedOffset)
        );
        let mut encoded_size = reader.clone();
        encoded_size.skip(usize::try_from(offset).unwrap()).unwrap();
        assert_eq!(
            encoded_size.read_uleb128(),
            Err(gimli::Error::UnsupportedOffset)
        );
        let mut after = reader;
        after.skip(usize::try_from(end).unwrap()).unwrap();
        assert_eq!(after.read_u8(), Ok(gimli::DW_OP_form_tls_address.0));
    }

    #[test]
    fn elf_split_expression_readers_do_not_lose_tls_guards() {
        let bytes = elf_fixture(Site::Tls32Expression);
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let section = file.section_by_name(".debug_info").unwrap();
        let relocations = Rc::new(Relocations::load(&file, &section).unwrap());
        let (&offset, &end) = relocations.deferred.iter().next().unwrap();
        assert_eq!(end - offset, 4);
        let mut reader = Reader::new(section.data().unwrap(), RunTimeEndian::Little, relocations);
        reader.skip(usize::try_from(offset).unwrap()).unwrap();
        let mut expression = reader.split(4).unwrap();
        assert_eq!(expression.read_u32(), Err(gimli::Error::UnsupportedOffset));
        assert_eq!(expression.to_slice(), Err(gimli::Error::UnsupportedOffset));
        assert_eq!(
            reader.read_u8(),
            Ok(gimli::DW_OP_form_tls_address.0),
            "only the opaque operand was skipped"
        );
    }

    #[test]
    fn elf_non_tls_unsupported_relocations_still_refuse_loading() {
        let bytes = elf_fixture(Site::RelativeExpression);
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let section = file.section_by_name(".debug_info").unwrap();
        assert!(
            Relocations::load(&file, &section)
                .unwrap_err()
                .contains("Unsupported relocation type")
        );
    }
}

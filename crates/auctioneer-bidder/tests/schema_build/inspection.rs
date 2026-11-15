//! Decoder for our compiler-checked private inspection records, not Rust layouts.

use object::{File as ObjectFile, elf};
use std::collections::BTreeMap;
use std::str::from_utf8;

#[derive(Debug, Eq, PartialEq)]
pub struct Record {
    pub owner: String,
    pub functions: [u64; 3],
}

pub fn native_records(
    bytes: &[u8],
    identity: &str,
    owners: &[&str],
) -> Result<Vec<Record>, &'static str> {
    use object::{
        File, Object as _, ObjectKind, ObjectSection as _, ObjectSymbol as _, SymbolKind,
    };
    if bytes.len() > 256_usize << 20_u32 {
        return Err("native inspection artifact exceeds byte cap");
    }
    let file = File::parse(bytes).map_err(|_error| "invalid native inspection artifact")?;
    if file.kind() != ObjectKind::Executable
        && !(file.kind() == ObjectKind::Dynamic && file.entry() != 0)
    {
        return Err("inspection requires a linked native executable");
    }
    let word = if file.is_64() { 8 } else { 4 };
    check_native_relocation_encodings(&file)?;
    let relocations = native_relocations(&file)?;
    let mut rows = Vec::new();
    for section in file.sections() {
        if super::schema_artifact::native_program_data(&file, &section)? {
            let data = section
                .data()
                .map_err(|_error| "invalid native inspection data")?;
            let found = records_with(
                data,
                word,
                file.is_little_endian(),
                identity,
                |row_offset, functions| {
                    let row_address = section
                        .address()
                        .checked_add(
                            u64::try_from(row_offset)
                                .map_err(|_error| "native inspection address overflow")?,
                        )
                        .ok_or("native inspection address overflow")?;
                    relocations.apply(row_address, word, functions)
                },
            )?;
            if rows
                .len()
                .checked_add(found.len())
                .is_none_or(|count| count > 512)
            {
                return Err("too many inspection records");
            }
            rows.extend(found);
        }
    }
    reconcile(&rows, owners)?;
    let mut definitions = Vec::new();
    for symbol in file.symbols() {
        if symbol.is_definition() && symbol.kind() == SymbolKind::Text {
            let address = symbol.address();
            definitions.push((
                address,
                address
                    .checked_add(1)
                    .ok_or("native inspection address overflow")?,
            ));
        }
    }
    for row in &rows {
        native_functions(row, &definitions)?;
    }
    Ok(rows)
}

#[derive(Default)]
struct NativeRelocations {
    entries: BTreeMap<u64, NativeRelocation>,
    max_write_bytes: u64,
}

struct NativeRelocation {
    end: u64,
    value: Result<u64, &'static str>,
}

impl NativeRelocations {
    fn apply(
        &self,
        row_address: u64,
        word: usize,
        functions: &mut [u64; 3],
    ) -> Result<(), &'static str> {
        let pointers = row_address
            .checked_add(if word == 8 { 152 } else { 148 })
            .ok_or("native inspection address overflow")?;
        let word_bytes =
            u64::try_from(word).map_err(|_error| "unsupported inspection pointer width")?;
        let span_bytes = word_bytes
            .checked_mul(3)
            .ok_or("native inspection address overflow")?;
        let end = pointers
            .checked_add(span_bytes)
            .ok_or("native inspection address overflow")?;
        let offsets = [
            0,
            word_bytes,
            word_bytes
                .checked_mul(2)
                .ok_or("native inspection address overflow")?,
        ];
        for (offset, relocated) in self
            .entries
            .range(row_address.saturating_sub(self.max_write_bytes.saturating_sub(1))..end)
        {
            if relocated.end <= row_address {
                continue;
            }
            let relative = offset
                .checked_sub(pointers)
                .ok_or("inspection relocation outside function slot")?;
            let slot = offsets
                .iter()
                .position(|candidate| *candidate == relative)
                .ok_or("inspection relocation outside function slot")?;
            *functions
                .get_mut(slot)
                .ok_or("inspection relocation outside function slot")? = relocated.value?;
        }
        Ok(())
    }
}

fn check_native_relocation_encodings(file: &ObjectFile<'_>) -> Result<(), &'static str> {
    use object::read::elf::SectionHeader as _;
    let unsupported = |kind: elf::SectionType, flags: elf::SectionFlags| {
        flags.contains(elf::SHF_ALLOC)
            && matches!(kind, elf::SHT_REL | elf::SHT_RELR | elf::SHT_CREL)
    };
    let unaccounted = if let ObjectFile::Elf32(elf_file) = file {
        elf_file.elf_section_table().iter().any(|header| {
            unsupported(
                header.sh_type(elf_file.endian()),
                header.sh_flags(elf_file.endian()),
            )
        })
    } else if let ObjectFile::Elf64(elf_file) = file {
        elf_file.elf_section_table().iter().any(|header| {
            unsupported(
                header.sh_type(elf_file.endian()),
                header.sh_flags(elf_file.endian()),
            )
        })
    } else {
        false
    };
    if unaccounted {
        Err("unsupported inspection relocation encoding")
    } else {
        Ok(())
    }
}

fn native_relocations(file: &ObjectFile<'_>) -> Result<NativeRelocations, &'static str> {
    use object::{Architecture, Object as _, RelocationFlags, RelocationTarget};
    use std::collections::btree_map::Entry;

    let mut result = NativeRelocations::default();
    if let Some(relocations) = file.dynamic_relocations() {
        for (address, relocation) in relocations {
            let width = native_loader_write_bytes(file, &relocation)?;
            if width == 0 {
                continue;
            }
            let end = address
                .checked_add(width)
                .ok_or("inspection relocation address overflow")?;
            result.max_write_bytes = result.max_write_bytes.max(width);
            // PIE function slots carry virtual addresses in explicit RELA addends,
            // not necessarily in their on-disk data bytes. Never resolve symbols
            // or treat an unsupported loader operation as a code pointer.
            let relative = matches!(
                (file.architecture(), relocation.flags()),
                (
                    Architecture::X86_64,
                    RelocationFlags::Elf {
                        r_type: elf::R_X86_64_RELATIVE
                    }
                ) | (
                    Architecture::Aarch64,
                    RelocationFlags::Elf {
                        r_type: elf::R_AARCH64_RELATIVE
                    }
                )
            ) && file.is_64()
                && relocation.target() == RelocationTarget::Absolute
                && !relocation.has_implicit_addend();
            let value = if relative {
                u64::try_from(relocation.addend())
                    .map_err(|_error| "invalid inspection relocation addend")
            } else {
                Err("unsupported inspection relocation")
            };
            match result.entries.entry(address) {
                Entry::Vacant(entry) => {
                    entry.insert(NativeRelocation { end, value });
                }
                Entry::Occupied(mut entry) => {
                    let duplicate = entry.get_mut();
                    duplicate.end = duplicate.end.max(end);
                    duplicate.value = Err("duplicate inspection relocation");
                }
            }
        }
    }
    Ok(result)
}

fn native_loader_write_bytes(
    file: &ObjectFile<'_>,
    relocation: &object::Relocation,
) -> Result<u64, &'static str> {
    use object::{Architecture, Object as _, RelocationFlags};

    if !file.is_64() {
        return Err("unsupported inspection relocation");
    }
    // A TLSDESC writes two pointers, not one. COPY and unknown operations
    // have no proven extent here and cannot be assumed disjoint from a row.
    match (file.architecture(), relocation.flags()) {
        (
            Architecture::X86_64,
            RelocationFlags::Elf {
                r_type: elf::R_X86_64_NONE,
            },
        )
        | (
            Architecture::Aarch64,
            RelocationFlags::Elf {
                r_type: elf::R_AARCH64_NONE,
            },
        ) => Ok(0),
        (
            Architecture::X86_64,
            RelocationFlags::Elf {
                r_type: elf::R_X86_64_TLSDESC,
            },
        )
        | (
            Architecture::Aarch64,
            RelocationFlags::Elf {
                r_type: elf::R_AARCH64_TLSDESC,
            },
        ) => Ok(16),
        (
            Architecture::X86_64,
            RelocationFlags::Elf {
                r_type:
                    elf::R_X86_64_64
                    | elf::R_X86_64_RELATIVE
                    | elf::R_X86_64_RELATIVE64
                    | elf::R_X86_64_GLOB_DAT
                    | elf::R_X86_64_JUMP_SLOT
                    | elf::R_X86_64_IRELATIVE
                    | elf::R_X86_64_DTPMOD64
                    | elf::R_X86_64_DTPOFF64
                    | elf::R_X86_64_TPOFF64
                    | elf::R_X86_64_SIZE64,
            },
        )
        | (
            Architecture::Aarch64,
            RelocationFlags::Elf {
                r_type:
                    elf::R_AARCH64_ABS64
                    | elf::R_AARCH64_RELATIVE
                    | elf::R_AARCH64_GLOB_DAT
                    | elf::R_AARCH64_JUMP_SLOT
                    | elf::R_AARCH64_IRELATIVE
                    | elf::R_AARCH64_TLS_DTPMOD
                    | elf::R_AARCH64_TLS_DTPREL
                    | elf::R_AARCH64_TLS_TPREL,
            },
        ) => Ok(8),
        _ => Err("unsupported inspection relocation"),
    }
}

pub fn wasm_records(
    bytes: &[u8],
    identity: &str,
    owners: &[&str],
) -> Result<Vec<Record>, &'static str> {
    use wasmparser::{Encoding, Parser, Payload, TypeRef};
    super::schema_artifact::measure(bytes)?;
    let mut stack = Vec::<Option<CoreRecords>>::new();
    let mut result = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|_error| "invalid WASM inspection artifact")? {
            Payload::Version { encoding, .. } => stack.push(match encoding {
                Encoding::Module => Some(CoreRecords::default()),
                Encoding::Component => None,
            }),
            Payload::End(_) => {
                if let Some(frame) = stack.pop().ok_or("invalid WASM inspection nesting")? {
                    for row in &frame.rows {
                        wasm_functions(row, &frame.table, frame.imported, frame.defined)?;
                    }
                    result.extend(frame.rows);
                }
            }
            Payload::ImportSection(section) => {
                let frame = core(&mut stack)?;
                for entry in section.into_imports() {
                    match entry.map_err(|_error| "invalid WASM inspection import")?.ty {
                        TypeRef::Func(_) | TypeRef::FuncExact(_) => {
                            frame.imported = frame
                                .imported
                                .checked_add(1)
                                .ok_or("inspection function arithmetic overflow")?;
                        }
                        TypeRef::Table(table) => frame.add_table(table.initial, table.table64)?,
                        TypeRef::Memory(_) | TypeRef::Global(_) | TypeRef::Tag(_) => {}
                    }
                }
            }
            Payload::FunctionSection(section) => {
                core(&mut stack)?.defined = u64::from(section.count());
            }
            Payload::TableSection(section) => {
                for entry in section {
                    let table = entry.map_err(|_error| "invalid WASM inspection table")?;
                    core(&mut stack)?.add_table(table.ty.initial, table.ty.table64)?;
                }
            }
            Payload::ElementSection(section) => {
                for entry in section {
                    core(&mut stack)?
                        .elements(entry.map_err(|_error| "invalid WASM inspection element")?)?;
                }
            }
            Payload::DataSection(section) => {
                for entry in section {
                    let data = entry.map_err(|_error| "invalid WASM inspection data")?;
                    core(&mut stack)?
                        .rows
                        .extend(records(data.data, 4, true, identity)?);
                }
            }
            Payload::TypeSection(_)
            | Payload::MemorySection(_)
            | Payload::TagSection(_)
            | Payload::GlobalSection(_)
            | Payload::ExportSection(_)
            | Payload::StartSection { .. }
            | Payload::DataCountSection { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::CodeSectionEntry(_)
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
            | Payload::UnknownSection { .. } => {}
            _ => return Err("unsupported WASM inspection payload"),
        }
    }
    if !stack.is_empty() {
        return Err("invalid WASM inspection nesting");
    }
    reconcile(&result, owners)?;
    Ok(result)
}

#[derive(Default)]
struct CoreRecords {
    imported: u64,
    defined: u64,
    tables: u32,
    initial_table: Option<u64>,
    table: BTreeMap<u64, u64>,
    rows: Vec<Record>,
}

fn core(stack: &mut [Option<CoreRecords>]) -> Result<&mut CoreRecords, &'static str> {
    stack
        .last_mut()
        .and_then(Option::as_mut)
        .ok_or("WASM inspection payload outside core module")
}

impl CoreRecords {
    fn add_table(&mut self, initial: u64, table64: bool) -> Result<(), &'static str> {
        if self.tables == 0 {
            if table64 {
                return Err("unsupported inspection table width");
            }
            self.initial_table = Some(initial);
        }
        self.tables = self
            .tables
            .checked_add(1)
            .ok_or("inspection function arithmetic overflow")?;
        Ok(())
    }

    fn elements(&mut self, element: wasmparser::Element<'_>) -> Result<(), &'static str> {
        use wasmparser::{ElementItems, ElementKind, Operator};
        let ElementKind::Active {
            table_index,
            offset_expr,
        } = element.kind
        else {
            return Ok(());
        };
        if table_index.unwrap_or(0) != 0 {
            return Ok(());
        }
        let mut operations = offset_expr.get_operators_reader();
        let Operator::I32Const { value } = operations
            .read()
            .map_err(|_error| "invalid inspection table offset")?
        else {
            return Err("unsupported inspection table offset");
        };
        let offset = u64::from(value.cast_unsigned());
        if !matches!(operations.read(), Ok(Operator::End)) || !operations.eof() {
            return Err("unsupported inspection table offset");
        }
        match element.items {
            ElementItems::Functions(functions) => {
                for (index, entry) in functions.into_iter().enumerate() {
                    let function = entry.map_err(|_error| "invalid inspection table function")?;
                    self.insert(offset, index, Some(u64::from(function)))?;
                }
            }
            ElementItems::Expressions(_, expressions) => {
                for (index, entry) in expressions.into_iter().enumerate() {
                    let expression =
                        entry.map_err(|_error| "invalid inspection table expression")?;
                    let mut reader = expression.get_operators_reader();
                    let operation = reader
                        .read()
                        .map_err(|_error| "invalid inspection table expression")?;
                    let function = if let Operator::RefFunc { function_index } = operation {
                        Some(u64::from(function_index))
                    } else if matches!(operation, Operator::RefNull { .. }) {
                        None
                    } else {
                        return Err("unsupported inspection table expression");
                    };
                    if !matches!(reader.read(), Ok(Operator::End)) || !reader.eof() {
                        return Err("unsupported inspection table expression");
                    }
                    self.insert(offset, index, function)?;
                }
            }
        }
        Ok(())
    }

    fn insert(
        &mut self,
        offset: u64,
        index: usize,
        function: Option<u64>,
    ) -> Result<(), &'static str> {
        let position = offset
            .checked_add(
                u64::try_from(index).map_err(|_error| "inspection function arithmetic overflow")?,
            )
            .ok_or("inspection function arithmetic overflow")?;
        if position
            >= self
                .initial_table
                .ok_or("inspection function table missing")?
        {
            return Err("inspection element exceeds initial table");
        }
        if let Some(target) = function {
            self.table.insert(position, target);
        } else {
            self.table.remove(&position);
        }
        Ok(())
    }
}

pub fn reconcile(rows: &[Record], owners: &[&str]) -> Result<(), &'static str> {
    use std::collections::BTreeSet;
    let expected = owners.iter().copied().collect::<BTreeSet<_>>();
    if expected.is_empty() || expected.len() != owners.len() || expected.contains("") {
        return Err("invalid inspection owners");
    }
    let actual = rows
        .iter()
        .map(|row| row.owner.as_str())
        .collect::<BTreeSet<_>>();
    if actual.len() != rows.len() || actual != expected {
        return Err("inspection owner membership mismatch");
    }
    Ok(())
}

pub fn native_functions(row: &Record, code: &[(u64, u64)]) -> Result<(), &'static str> {
    if row.functions.iter().any(|pointer| {
        !code
            .iter()
            .any(|(start, end)| *start <= *pointer && *pointer < *end)
    }) {
        return Err("inspection function is not defined native code");
    }
    Ok(())
}

pub fn wasm_functions(
    row: &Record,
    table: &BTreeMap<u64, u64>,
    imported: u64,
    defined: u64,
) -> Result<(), &'static str> {
    let end = imported
        .checked_add(defined)
        .ok_or("inspection function arithmetic overflow")?;
    for pointer in row.functions {
        let function = table
            .get(&pointer)
            .ok_or("inspection function table entry missing")?;
        if *function < imported || *function >= end {
            return Err("inspection function is not defined WASM code");
        }
    }
    Ok(())
}

pub fn records(
    data: &[u8],
    word: usize,
    little_endian: bool,
    identity: &str,
) -> Result<Vec<Record>, &'static str> {
    records_with(data, word, little_endian, identity, |_, _| Ok(()))
}

fn records_with(
    data: &[u8],
    word: usize,
    little_endian: bool,
    identity: &str,
    mut relocate: impl FnMut(usize, &mut [u64; 3]) -> Result<(), &'static str>,
) -> Result<Vec<Record>, &'static str> {
    const MAGIC: &[u8; 16] = b"AUCTIONEERSCHEMA";
    let (length, pointers) = match word {
        4 => (160, 148),
        8 => (176, 152),
        _ => return Err("unsupported inspection pointer width"),
    };
    if identity.len() != 64 || !identity.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid inspection identity");
    }
    let mut result = Vec::new();
    let mut cursor = 0_usize;
    while let Some(relative) = data
        .get(cursor..)
        .ok_or("invalid inspection range")?
        .windows(MAGIC.len())
        .position(|window| window == MAGIC)
    {
        let start = cursor
            .checked_add(relative)
            .ok_or("invalid inspection range")?;
        cursor = start
            .checked_add(length)
            .ok_or("invalid inspection range")?;
        let row = data
            .get(start..cursor)
            .ok_or("truncated inspection record")?;
        if row.get(16..80) != Some(identity.as_bytes()) {
            return Err("inspection identity mismatch");
        }
        let owner_len = usize::try_from(integer(
            row.get(144..148).ok_or("invalid inspection owner")?,
            little_endian,
        )?)
        .map_err(|_error| "invalid inspection owner")?;
        let owner = row.get(80..144).ok_or("invalid inspection owner")?;
        if owner_len == 0
            || owner_len > owner.len()
            || owner
                .get(owner_len..)
                .ok_or("invalid inspection owner")?
                .iter()
                .any(|byte| *byte != 0)
        {
            return Err("invalid inspection owner");
        }
        let name = owner.get(..owner_len).ok_or("invalid inspection owner")?;
        if !name.is_ascii() || name.iter().any(u8::is_ascii_control) {
            return Err("invalid inspection owner");
        }
        let mut functions = [0_u64; 3];
        let encoded_functions = row.get(pointers..).ok_or("truncated inspection record")?;
        for (pointer, encoded) in functions
            .iter_mut()
            .zip(encoded_functions.chunks_exact(word))
        {
            *pointer = integer(encoded, little_endian)?;
        }
        relocate(start, &mut functions)?;
        if functions.contains(&0) {
            return Err("null inspection function");
        }
        if result.len() >= 512 {
            return Err("too many inspection records");
        }
        result.push(Record {
            owner: from_utf8(name)
                .map_err(|_error| "invalid inspection owner")?
                .to_owned(),
            functions,
        });
    }
    Ok(result)
}

#[expect(
    clippy::little_endian_bytes,
    clippy::big_endian_bytes,
    reason = "inspection integers use the artifact's declared byte order"
)]
fn integer(bytes: &[u8], little: bool) -> Result<u64, &'static str> {
    match bytes.len() {
        4 => {
            let raw = bytes
                .try_into()
                .map_err(|_error| "invalid inspection integer")?;
            Ok(u64::from(if little {
                u32::from_le_bytes(raw)
            } else {
                u32::from_be_bytes(raw)
            }))
        }
        8 => {
            let raw = bytes
                .try_into()
                .map_err(|_error| "invalid inspection integer")?;
            Ok(if little {
                u64::from_le_bytes(raw)
            } else {
                u64::from_be_bytes(raw)
            })
        }
        _ => Err("invalid inspection integer"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Record, native_functions, native_records, reconcile, records, wasm_functions, wasm_records,
    };
    use object::elf;
    use object::write::elf::Rel;
    use std::array;
    use std::borrow::Cow;
    use std::collections::BTreeMap;
    use std::mem::size_of;
    use wasm_encoder::{
        CodeSection, ConstExpr, DataSection, ElementSection, Elements, EntityType, Function,
        FunctionSection, ImportSection, Instruction, MemorySection, MemoryType, Module, RefType,
        TableSection, TableType, TypeSection,
    };

    fn fake_row() -> Record {
        Record {
            owner: "fake_owner".into(),
            functions: [101, 111, 121],
        }
    }

    #[test]
    fn inspection_membership_is_closed_and_does_not_collapse_duplicate_owners() {
        assert_eq!(reconcile(&[fake_row()], &["fake_owner"]), Ok(()));
        for rows in [
            vec![],
            vec![fake_row(), fake_row()],
            vec![Record {
                owner: "fake_other_owner".into(),
                ..fake_row()
            }],
        ] {
            assert_eq!(
                reconcile(&rows, &["fake_owner"]),
                Err("inspection owner membership mismatch")
            );
        }
        assert_eq!(
            reconcile(&[fake_row()], &[]),
            Err("invalid inspection owners")
        );
        assert_eq!(
            reconcile(&[fake_row()], &["fake_owner", "fake_owner"]),
            Err("invalid inspection owners")
        );
    }

    #[test]
    fn inspection_native_references_must_point_to_actual_code_for_every_method() {
        let code = [(100, 105), (110, 115), (120, 125)];
        assert_eq!(native_functions(&fake_row(), &code), Ok(()));
        for index in 0..3 {
            let mut row = fake_row();
            row.functions[index] = 105;
            assert_eq!(
                native_functions(&row, &code),
                Err("inspection function is not defined native code")
            );
        }
        assert_eq!(
            native_functions(&fake_row(), &[]),
            Err("inspection function is not defined native code")
        );
    }

    #[test]
    fn inspection_wasm_references_refuse_imports_missing_entries_and_code_overflow() {
        let table = BTreeMap::from([(101, 2), (111, 3), (121, 4)]);
        assert_eq!(wasm_functions(&fake_row(), &table, 2, 3), Ok(()));
        for index in [101, 111, 121] {
            let mut missing = table.clone();
            missing.remove(&index);
            assert_eq!(
                wasm_functions(&fake_row(), &missing, 2, 3),
                Err("inspection function table entry missing")
            );
            for bad in [0, 1, 5, 99] {
                let mut invalid = table.clone();
                invalid.insert(index, bad);
                assert_eq!(
                    wasm_functions(&fake_row(), &invalid, 2, 3),
                    Err("inspection function is not defined WASM code")
                );
            }
        }
        assert_eq!(
            wasm_functions(&fake_row(), &table, u64::MAX, 1),
            Err("inspection function arithmetic overflow")
        );
    }

    #[expect(
        clippy::little_endian_bytes,
        clippy::big_endian_bytes,
        reason = "fixtures exercise both artifact byte orders independently of the host"
    )]
    pub(super) fn fake_record(word: usize, little: bool) -> Vec<u8> {
        let mut data = b"AUCTIONEERSCHEMA".to_vec();
        data.extend_from_slice(&[b'a'; 64]);
        let mut owner = [0_u8; 64];
        owner[..10].copy_from_slice(b"fake_owner");
        data.extend_from_slice(&owner);
        data.extend_from_slice(&if little {
            10_u32.to_le_bytes()
        } else {
            10_u32.to_be_bytes()
        });
        if word == 8 {
            data.extend_from_slice(&[0; 4]);
        }
        for pointer in [0x1020_3040_u64, 7, 99] {
            if word == 4 {
                let value = u32::try_from(pointer).unwrap();
                data.extend_from_slice(&if little {
                    value.to_le_bytes()
                } else {
                    value.to_be_bytes()
                });
            } else {
                data.extend_from_slice(&if little {
                    pointer.to_le_bytes()
                } else {
                    pointer.to_be_bytes()
                });
            }
        }
        assert_eq!(data.len(), if word == 4 { 160 } else { 176 });
        data
    }

    #[test]
    fn inspection_records_decode_compiler_row_offsets_on_both_pointer_widths() {
        let mut checked = 0_usize;
        for word in [4, 8] {
            for little in [false, true] {
                let mut data = vec![11, 22, 33];
                data.extend_from_slice(&fake_record(word, little));
                assert_eq!(
                    records(&data, word, little, &"a".repeat(64)),
                    Ok(vec![Record {
                        owner: "fake_owner".into(),
                        functions: [0x1020_3040, 7, 99],
                    }])
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 4);
    }

    #[test]
    #[expect(
        clippy::little_endian_bytes,
        reason = "malformed fixtures mutate the encoded little-endian owner length"
    )]
    fn inspection_records_refuse_foreign_truncated_and_malformed_rows() {
        let good = fake_record(4, true);
        let mut foreign = good.clone();
        foreign[16] = b'b';
        assert_eq!(
            records(&foreign, 4, true, &"a".repeat(64)),
            Err("inspection identity mismatch")
        );
        for length in [16, 80, 148, 159] {
            assert_eq!(
                records(&good[..length], 4, true, &"a".repeat(64)),
                Err("truncated inspection record")
            );
        }
        for length in [0_u32, 65] {
            let mut row = good.clone();
            row[144..148].copy_from_slice(&length.to_le_bytes());
            assert_eq!(
                records(&row, 4, true, &"a".repeat(64)),
                Err("invalid inspection owner")
            );
        }
        let mut invalid_padding = good.clone();
        invalid_padding[90] = 1;
        assert_eq!(
            records(&invalid_padding, 4, true, &"a".repeat(64)),
            Err("invalid inspection owner")
        );
        for offset in [148, 152, 156] {
            let mut row = good.clone();
            row[offset..offset + 4].fill(0);
            assert_eq!(
                records(&row, 4, true, &"a".repeat(64)),
                Err("null inspection function")
            );
        }
        assert_eq!(
            records(&good, 2, true, &"a".repeat(64)),
            Err("unsupported inspection pointer width")
        );
    }

    #[test]
    fn inspection_records_keep_every_row_for_closed_membership_reconciliation() {
        let mut bytes = fake_record(8, true);
        bytes.extend_from_slice(&fake_record(8, true));
        let rows = records(&bytes, 8, true, &"a".repeat(64)).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], rows[1]);
        assert_eq!(records(b"no record", 8, true, &"a".repeat(64)), Ok(vec![]));
    }

    #[expect(
        clippy::little_endian_bytes,
        reason = "Wasm fixtures encode function-table indices in little-endian order"
    )]
    fn fake_module(imported: bool, functions: [u32; 3], custom_only: bool) -> Vec<u8> {
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], []);
        module.section(&types);
        if imported {
            let mut imports = ImportSection::new();
            imports.import("fake_host", "fake_function", EntityType::Function(0));
            module.section(&imports);
        }
        let mut declarations = FunctionSection::new();
        declarations.function(0);
        module.section(&declarations);
        let mut tables = TableSection::new();
        tables.table(TableType {
            element_type: RefType::FUNCREF,
            minimum: 4,
            maximum: Some(4),
            table64: false,
            shared: false,
        });
        module.section(&tables);
        let mut memories = MemorySection::new();
        memories.memory(MemoryType {
            minimum: 1,
            maximum: Some(1),
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        module.section(&memories);
        let mut elements = ElementSection::new();
        elements.active(
            Some(0),
            &ConstExpr::i32_const(1),
            Elements::Functions(Cow::Borrowed(&functions)),
        );
        module.section(&elements);
        let mut code = CodeSection::new();
        let mut function = Function::new([]);
        function.instruction(&Instruction::End);
        code.function(&function);
        module.section(&code);
        let mut row = fake_record(4, true);
        row[148..152].copy_from_slice(&1_u32.to_le_bytes());
        row[152..156].copy_from_slice(&2_u32.to_le_bytes());
        row[156..160].copy_from_slice(&3_u32.to_le_bytes());
        if custom_only {
            module.section(&wasm_encoder::CustomSection {
                name: Cow::Borrowed("fake_inspection"),
                data: Cow::Owned(row),
            });
        } else {
            let mut data = DataSection::new();
            data.active(0, &ConstExpr::i32_const(1024), row);
            module.section(&data);
        }
        module.finish()
    }

    #[test]
    fn wasm_inspection_follows_actual_defined_function_table_entries() {
        for imported in [false, true] {
            let index = u32::from(imported);
            let bytes = fake_module(imported, [index; 3], false);
            let rows = wasm_records(&bytes, &"a".repeat(64), &["fake_owner"]).unwrap();
            assert_eq!(
                rows,
                vec![Record {
                    owner: "fake_owner".into(),
                    functions: [1, 2, 3]
                }]
            );
        }
    }

    #[test]
    fn wasm_inspection_refuses_imports_and_custom_section_only_membership() {
        let bytes = fake_module(true, [1, 0, 1], false);
        assert_eq!(
            wasm_records(&bytes, &"a".repeat(64), &["fake_owner"]),
            Err("inspection function is not defined WASM code")
        );
        let custom_only_bytes = fake_module(false, [0; 3], true);
        assert_eq!(
            wasm_records(&custom_only_bytes, &"a".repeat(64), &["fake_owner"]),
            Err("inspection owner membership mismatch")
        );
    }

    fn fake_component(modules: &[Vec<u8>]) -> Vec<u8> {
        use wasm_encoder::{Component, ComponentSectionId, RawSection};
        let mut component = Component::new();
        for module in modules {
            component.section(&RawSection {
                id: ComponentSectionId::CoreModule.into(),
                data: module,
            });
        }
        component.finish()
    }

    #[test]
    fn wasm_inspection_keeps_function_tables_local_to_nested_modules() {
        let owned = fake_module(false, [0; 3], false);
        let other = fake_module(true, [1; 3], true);
        for modules in [[owned.clone(), other.clone()], [other, owned]] {
            let rows =
                wasm_records(&fake_component(&modules), &"a".repeat(64), &["fake_owner"]).unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].owner, "fake_owner");
        }
        let bad = fake_module(true, [1, 0, 1], false);
        let unrelated = fake_module(false, [0; 3], true);
        assert_eq!(
            wasm_records(
                &fake_component(&[unrelated, bad]),
                &"a".repeat(64),
                &["fake_owner"]
            ),
            Err("inspection function is not defined WASM code")
        );
    }

    #[test]
    fn wasm_inspection_refuses_duplicate_retained_owners_across_core_modules() {
        let bytes = fake_module(false, [0; 3], false);
        assert_eq!(
            wasm_records(
                &fake_component(&[bytes.clone(), bytes]),
                &"a".repeat(64),
                &["fake_owner"]
            ),
            Err("inspection owner membership mismatch")
        );
    }

    // The synthetic ELF is parsed, never executed.

    fn fake_relative_relocations() -> [Rel; 3] {
        let addresses = [0x1010, 0x1020, 0x1030];
        array::from_fn(|index| Rel {
            // The record starts 16 bytes into .data.rel.ro at VA 0x2000.
            r_offset: [0x20a8, 0x20b0, 0x20b8][index],
            r_sym: 0,
            r_type: elf::R_X86_64_RELATIVE,
            r_addend: addresses[index],
        })
    }

    // None omits .rela.dyn; Some(&[]) retains an empty relocation section.
    // Callers may remove, replace, negate or duplicate individual typed Rel rows.
    fn fake_linked_elf(relocations: Option<&[Rel]>) -> Vec<u8> {
        fake_linked_elf_for_machine(relocations, elf::EM_X86_64)
    }

    fn fake_linked_elf_for_machine(relocations: Option<&[Rel]>, machine: elf::Machine) -> Vec<u8> {
        fake_linked_elf_with_extra_encoding(relocations, machine, None)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "typed ELF writing reserves then emits one matching segment, section and symbol layout"
    )]
    fn fake_linked_elf_with_extra_encoding(
        relocations: Option<&[Rel]>,
        machine: elf::Machine,
        extra_encoding: Option<elf::SectionType>,
    ) -> Vec<u8> {
        use object::write::elf::{FileHeader, ProgramHeader, SectionHeader, Sym, Writer};
        use object::{Endianness, elf};

        let mut row = fake_record(8, true);
        row[152..176].fill(0);
        let mut data = vec![0; 16];
        data.extend_from_slice(&row);
        let text = [0xc3_u8; 64];
        let mut bytes = Vec::new();
        let mut writer = Writer::new(Endianness::Little, true, &mut bytes);

        writer.reserve_file_header();
        writer.reserve_program_headers(2);
        let text_section = writer.reserve_section_index();
        let text_name = writer.add_section_name(b".text");
        writer.reserve_section_index();
        let data_name = writer.add_section_name(b".data.rel.ro");
        writer.require_dynstr();
        writer.reserve_dynstr_section_index();
        let dynsym_section = writer.reserve_dynsym_section_index();
        let rela_name = relocations.map(|_| {
            writer.reserve_section_index();
            writer.add_section_name(b".rela.dyn")
        });
        let extra_name = extra_encoding.map(|_| {
            writer.reserve_section_index();
            writer.add_section_name(b".fake.unsupported")
        });
        writer.reserve_symtab_section_index();
        writer.reserve_strtab_section_index();
        writer.reserve_shstrtab_section_index();

        let function_names: [&[u8]; 3] = [b"fake_first", b"fake_second", b"fake_third"];
        let function_symbols = function_names.map(|name| writer.add_string(name));
        writer.reserve_null_symbol_index();
        for _ in &function_symbols {
            writer.reserve_symbol_index(Some(text_section));
        }
        writer.reserve_null_dynamic_symbol_index();

        let text_offset = writer.reserve(u64::try_from(text.len()).unwrap(), 16);
        let data_offset = writer.reserve(u64::try_from(data.len()).unwrap(), 8);
        let dynstr_offset = writer.reserve_dynstr().unwrap();
        let dynsym_offset = writer.reserve_dynsym();
        let rela_offset = relocations.map(|rows| writer.reserve_relocations(rows.len(), true));
        let extra_offset = extra_encoding.map(|_| writer.reserve(0, 8));
        let allocated_end = writer.reserved_len();
        writer.reserve_symtab();
        writer.reserve_strtab().unwrap();
        writer.reserve_shstrtab().unwrap();
        writer.reserve_section_headers();
        let reserved_end = writer.reserved_len();
        let data_address = |offset: u64| {
            0x2000_u64
                .checked_add(offset.checked_sub(data_offset).unwrap())
                .unwrap()
        };
        let data_size = allocated_end.checked_sub(data_offset).unwrap();

        writer
            .write_file_header(&FileHeader {
                os_abi: elf::ELFOSABI_SYSV,
                abi_version: 0,
                e_type: elf::ET_DYN,
                e_machine: machine,
                e_entry: 0x1010,
                e_flags: elf::FileFlags::default(),
            })
            .unwrap();
        writer.write_align_program_headers();
        writer.write_program_header(&ProgramHeader {
            p_type: elf::PT_LOAD,
            p_flags: elf::PF_R | elf::PF_X,
            p_offset: text_offset,
            p_vaddr: 0x1000,
            p_paddr: 0x1000,
            p_filesz: u64::try_from(text.len()).unwrap(),
            p_memsz: u64::try_from(text.len()).unwrap(),
            p_align: 16,
        });
        writer.write_program_header(&ProgramHeader {
            p_type: elf::PT_LOAD,
            p_flags: elf::PF_R | elf::PF_W,
            p_offset: data_offset,
            p_vaddr: 0x2000,
            p_paddr: 0x2000,
            p_filesz: data_size,
            p_memsz: data_size,
            p_align: 8,
        });

        writer.pad_until(text_offset);
        writer.write(&text);
        writer.pad_until(data_offset);
        writer.write(&data);
        writer.write_dynstr();
        writer.write_null_dynamic_symbol();
        if let Some(rows) = relocations {
            writer.write_align_relocation();
            for relocation in rows {
                writer.write_relocation(true, relocation);
            }
        }
        writer.write_null_symbol();
        for (name, address) in function_symbols.into_iter().zip([0x1010, 0x1020, 0x1030]) {
            writer.write_symbol(&Sym {
                st_name: writer.string_offset(Some(name)),
                section: Some(text_section.0),
                st_info: elf::SymbolInfo::new(elf::STB_GLOBAL, elf::STT_FUNC),
                st_other: elf::STV_DEFAULT.into(),
                st_shndx: elf::SHN_UNDEF,
                st_value: address,
                st_size: 1,
            });
        }
        writer.write_strtab();
        writer.write_shstrtab();

        writer.write_null_section_header();
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(Some(text_name)),
            sh_type: elf::SHT_PROGBITS,
            sh_flags: elf::SHF_ALLOC | elf::SHF_EXECINSTR,
            sh_addr: 0x1000,
            sh_offset: text_offset,
            sh_size: u64::try_from(text.len()).unwrap(),
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 16,
            sh_entsize: 0,
        });
        writer.write_section_header(&SectionHeader {
            sh_name: writer.section_name_offset(Some(data_name)),
            sh_type: elf::SHT_PROGBITS,
            sh_flags: elf::SHF_ALLOC | elf::SHF_WRITE,
            sh_addr: 0x2000,
            sh_offset: data_offset,
            sh_size: u64::try_from(data.len()).unwrap(),
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 8,
            sh_entsize: 0,
        });
        writer.write_dynstr_section_header(data_address(dynstr_offset));
        writer.write_dynsym_section_header(data_address(dynsym_offset), 1);
        if let (Some(name), Some(offset), Some(rows)) = (rela_name, rela_offset, relocations) {
            let entry_size = size_of::<elf::Rela64<Endianness>>();
            writer.write_section_header(&SectionHeader {
                sh_name: writer.section_name_offset(Some(name)),
                sh_type: elf::SHT_RELA,
                sh_flags: elf::SHF_ALLOC,
                sh_addr: data_address(offset),
                sh_offset: offset,
                sh_size: u64::try_from(rows.len().checked_mul(entry_size).unwrap()).unwrap(),
                // object discovers dynamic relocations through this .dynsym link.
                sh_link: dynsym_section.0,
                sh_info: 0,
                sh_addralign: 8,
                sh_entsize: u64::try_from(entry_size).unwrap(),
            });
        }
        if let (Some(kind), Some(name), Some(offset)) = (extra_encoding, extra_name, extra_offset) {
            writer.write_section_header(&SectionHeader {
                sh_name: writer.section_name_offset(Some(name)),
                sh_type: kind,
                sh_flags: elf::SHF_ALLOC,
                sh_addr: data_address(offset),
                sh_offset: offset,
                sh_size: 0,
                sh_link: dynsym_section.0,
                sh_info: 0,
                sh_addralign: 8,
                sh_entsize: 0,
            });
        }
        writer.write_symtab_section_header(1);
        writer.write_strtab_section_header();
        writer.write_shstrtab_section_header();
        assert_eq!(writer.len(), reserved_end);
        bytes
    }

    #[test]
    fn native_inspection_resolves_pie_function_slots_from_dynamic_relocations() {
        use object::{Object as _, ObjectSection as _};

        let bytes = fake_linked_elf(Some(&fake_relative_relocations()));
        let artifact = object::File::parse(bytes.as_slice()).unwrap();
        let data = artifact
            .section_by_name(".data.rel.ro")
            .unwrap()
            .data()
            .unwrap();
        assert!(data[16 + 152..16 + 176].iter().all(|byte| *byte == 0));
        assert_eq!(
            records(data, 8, true, &"a".repeat(64)),
            Err("null inspection function")
        );
        assert_eq!(
            native_records(&bytes, &"a".repeat(64), &["fake_owner"]),
            Ok(vec![Record {
                owner: "fake_owner".into(),
                functions: [0x1010, 0x1020, 0x1030],
            }])
        );
    }

    #[test]
    fn native_inspection_requires_every_dynamic_function_relocation() {
        for relocations in [None, Some(&[][..])] {
            assert_eq!(
                native_records(
                    &fake_linked_elf(relocations),
                    &"a".repeat(64),
                    &["fake_owner"]
                ),
                Err("null inspection function")
            );
        }
        for missing in 0..3 {
            let mut relocations = fake_relative_relocations().to_vec();
            relocations.remove(missing);
            assert_eq!(
                native_records(
                    &fake_linked_elf(Some(&relocations)),
                    &"a".repeat(64),
                    &["fake_owner"]
                ),
                Err("null inspection function")
            );
        }
    }

    #[test]
    fn native_inspection_resolves_aarch64_relative_function_slots() {
        let mut relocations = fake_relative_relocations();
        for relocation in &mut relocations {
            relocation.r_type = elf::R_AARCH64_RELATIVE;
        }
        let bytes = fake_linked_elf_for_machine(Some(&relocations), elf::EM_AARCH64);
        assert_eq!(
            native_records(&bytes, &"a".repeat(64), &["fake_owner"]),
            Ok(vec![Record {
                owner: "fake_owner".into(),
                functions: [0x1010, 0x1020, 0x1030],
            }])
        );
    }

    #[test]
    fn native_inspection_refuses_unresolved_or_invalid_dynamic_function_relocations() {
        for index in 0..3 {
            for (kind, addend, expected) in [
                (
                    elf::R_X86_64_64,
                    0x1010,
                    "unsupported inspection relocation",
                ),
                (
                    elf::R_X86_64_RELATIVE,
                    -1,
                    "invalid inspection relocation addend",
                ),
                (elf::R_X86_64_RELATIVE, 0, "null inspection function"),
                (
                    elf::R_X86_64_RELATIVE,
                    0x2000,
                    "inspection function is not defined native code",
                ),
            ] {
                let mut relocations = fake_relative_relocations();
                relocations[index].r_type = kind;
                relocations[index].r_addend = addend;
                assert_eq!(
                    native_records(
                        &fake_linked_elf(Some(&relocations)),
                        &"a".repeat(64),
                        &["fake_owner"]
                    ),
                    Err(expected),
                    "slot {index}, kind {kind}, addend {addend}"
                );
            }
        }
    }

    #[test]
    fn native_inspection_refuses_duplicate_or_misplaced_dynamic_relocations() {
        for index in 0..3 {
            for conflicting in [false, true] {
                let mut relocations = fake_relative_relocations().to_vec();
                let duplicate = relocations[index].clone();
                let changed_addend = [0x1020, 0x1030, 0x1010][index];
                assert_ne!(changed_addend, duplicate.r_addend);
                relocations.push(Rel {
                    r_addend: if conflicting {
                        changed_addend
                    } else {
                        duplicate.r_addend
                    },
                    ..duplicate
                });
                assert_eq!(
                    native_records(
                        &fake_linked_elf(Some(&relocations)),
                        &"a".repeat(64),
                        &["fake_owner"]
                    ),
                    Err("duplicate inspection relocation")
                );
            }
        }
        for offset in [0x2000 + 16, 0x2000 + 16 + 152 + 1] {
            let mut relocations = fake_relative_relocations().to_vec();
            relocations.push(Rel {
                r_offset: offset,
                r_sym: 0,
                r_type: elf::R_X86_64_RELATIVE,
                r_addend: 0x1010,
            });
            assert_eq!(
                native_records(
                    &fake_linked_elf(Some(&relocations)),
                    &"a".repeat(64),
                    &["fake_owner"]
                ),
                Err("inspection relocation outside function slot")
            );
        }
    }

    #[test]
    fn native_inspection_x86_64_tlsdesc_respects_sixteen_byte_write_boundaries() {
        let identity = "a".repeat(64);
        let expected = vec![Record {
            owner: "fake_owner".into(),
            functions: [0x1010, 0x1020, 0x1030],
        }];
        let mut relocations = fake_relative_relocations().to_vec();
        let baseline = fake_linked_elf_for_machine(Some(&relocations), elf::EM_X86_64);
        assert_eq!(
            native_records(&baseline, &identity, &["fake_owner"]).as_ref(),
            Ok(&expected)
        );
        // [0x2000, 0x2010) ends exactly where the retained record starts.
        relocations.push(Rel {
            r_offset: 0x2000,
            r_sym: 0,
            r_type: elf::R_X86_64_TLSDESC,
            r_addend: 0,
        });
        let disjoint = fake_linked_elf_for_machine(Some(&relocations), elf::EM_X86_64);
        assert_eq!(
            native_records(&disjoint, &identity, &["fake_owner"]).as_ref(),
            Ok(&expected)
        );
        for offset in [0x2001, 0x2008] {
            let mut overlapping = relocations.clone();
            let descriptor = overlapping.last_mut().unwrap();
            assert_eq!(descriptor.r_offset, 0x2000);
            descriptor.r_offset = offset;
            let bytes = fake_linked_elf_for_machine(Some(&overlapping), elf::EM_X86_64);
            assert_eq!(
                native_records(&bytes, &identity, &["fake_owner"]),
                Err("inspection relocation outside function slot"),
                "x86_64 TLSDESC at {offset:#x} overlaps the record at 0x2010"
            );
        }
    }

    #[test]
    fn native_inspection_aarch64_tlsdesc_respects_sixteen_byte_write_boundaries() {
        let identity = "a".repeat(64);
        let expected = vec![Record {
            owner: "fake_owner".into(),
            functions: [0x1010, 0x1020, 0x1030],
        }];
        let mut relocations = fake_relative_relocations().to_vec();
        for relocation in &mut relocations {
            relocation.r_type = elf::R_AARCH64_RELATIVE;
        }
        let baseline = fake_linked_elf_for_machine(Some(&relocations), elf::EM_AARCH64);
        assert_eq!(
            native_records(&baseline, &identity, &["fake_owner"]).as_ref(),
            Ok(&expected)
        );
        relocations.push(Rel {
            r_offset: 0x2000,
            r_sym: 0,
            r_type: elf::R_AARCH64_TLSDESC,
            r_addend: 0,
        });
        let disjoint = fake_linked_elf_for_machine(Some(&relocations), elf::EM_AARCH64);
        assert_eq!(
            native_records(&disjoint, &identity, &["fake_owner"]).as_ref(),
            Ok(&expected)
        );
        for offset in [0x2001, 0x2008] {
            let mut overlapping = relocations.clone();
            let descriptor = overlapping.last_mut().unwrap();
            assert_eq!(descriptor.r_offset, 0x2000);
            descriptor.r_offset = offset;
            let bytes = fake_linked_elf_for_machine(Some(&overlapping), elf::EM_AARCH64);
            assert_eq!(
                native_records(&bytes, &identity, &["fake_owner"]),
                Err("inspection relocation outside function slot"),
                "AArch64 TLSDESC at {offset:#x} overlaps the record at 0x2010"
            );
        }
    }

    #[test]
    fn native_inspection_copy_before_record_requires_a_proven_write_extent() {
        let identity = "a".repeat(64);
        for (machine, relative, copy) in [
            (elf::EM_X86_64, elf::R_X86_64_RELATIVE, elf::R_X86_64_COPY),
            (
                elf::EM_AARCH64,
                elf::R_AARCH64_RELATIVE,
                elf::R_AARCH64_COPY,
            ),
        ] {
            let mut relocations = fake_relative_relocations().to_vec();
            for relocation in &mut relocations {
                relocation.r_type = relative;
            }
            let baseline = fake_linked_elf_for_machine(Some(&relocations), machine);
            assert_eq!(
                native_records(&baseline, &identity, &["fake_owner"]),
                Ok(vec![Record {
                    owner: "fake_owner".into(),
                    functions: [0x1010, 0x1020, 0x1030],
                }])
            );
            for offset in [0x2000, 0x2001, 0x2008] {
                assert!(offset < 0x2010);
                let mut unbounded = relocations.clone();
                unbounded.push(Rel {
                    r_offset: offset,
                    r_sym: 0,
                    r_type: copy,
                    r_addend: 0,
                });
                let bytes = fake_linked_elf_for_machine(Some(&unbounded), machine);
                assert_eq!(
                    native_records(&bytes, &identity, &["fake_owner"]),
                    Err("unsupported inspection relocation"),
                    "machine {machine}, COPY at {offset:#x} has no proven disjoint extent"
                );
            }
        }
    }

    #[test]
    fn native_inspection_unknown_operation_before_record_is_not_assumed_disjoint() {
        let identity = "a".repeat(64);
        for (machine, relative) in [
            (elf::EM_X86_64, elf::R_X86_64_RELATIVE),
            (elf::EM_AARCH64, elf::R_AARCH64_RELATIVE),
        ] {
            let mut relocations = fake_relative_relocations().to_vec();
            for relocation in &mut relocations {
                relocation.r_type = relative;
            }
            let baseline = fake_linked_elf_for_machine(Some(&relocations), machine);
            assert_eq!(
                native_records(&baseline, &identity, &["fake_owner"]),
                Ok(vec![Record {
                    owner: "fake_owner".into(),
                    functions: [0x1010, 0x1020, 0x1030],
                }])
            );
            for offset in [0x2000, 0x2001, 0x2008] {
                assert!(offset < 0x2010);
                let mut unbounded = relocations.clone();
                unbounded.push(Rel {
                    r_offset: offset,
                    r_sym: 0,
                    r_type: elf::RelocationType(u32::MAX),
                    r_addend: 0,
                });
                let bytes = fake_linked_elf_for_machine(Some(&unbounded), machine);
                assert_eq!(
                    native_records(&bytes, &identity, &["fake_owner"]),
                    Err("unsupported inspection relocation"),
                    "machine {machine}, unknown op at {offset:#x} has no proven disjoint extent"
                );
            }
        }
    }

    #[test]
    fn native_inspection_relative_predecessor_respects_eight_byte_write_boundaries() {
        let identity = "a".repeat(64);
        for (machine, relative) in [
            (elf::EM_X86_64, elf::R_X86_64_RELATIVE),
            (elf::EM_AARCH64, elf::R_AARCH64_RELATIVE),
        ] {
            let expected = vec![Record {
                owner: "fake_owner".into(),
                functions: [0x1010, 0x1020, 0x1030],
            }];
            let mut relocations = fake_relative_relocations().to_vec();
            for relocation in &mut relocations {
                relocation.r_type = relative;
            }
            let baseline = fake_linked_elf_for_machine(Some(&relocations), machine);
            assert_eq!(
                native_records(&baseline, &identity, &["fake_owner"]).as_ref(),
                Ok(&expected)
            );
            // This eight-byte write ends at, but does not enter, the record.
            relocations.push(Rel {
                r_offset: 0x2008,
                r_sym: 0,
                r_type: relative,
                r_addend: 0x1010,
            });
            let disjoint = fake_linked_elf_for_machine(Some(&relocations), machine);
            assert_eq!(
                native_records(&disjoint, &identity, &["fake_owner"]).as_ref(),
                Ok(&expected)
            );
            let predecessor = relocations.last_mut().unwrap();
            assert_eq!(predecessor.r_offset, 0x2008);
            predecessor.r_offset = 0x2009;
            let overlapping = fake_linked_elf_for_machine(Some(&relocations), machine);
            assert_eq!(
                native_records(&overlapping, &identity, &["fake_owner"]),
                Err("inspection relocation outside function slot"),
                "machine {machine}, eight-byte write at 0x2009 overlaps the record by one byte"
            );
        }
    }

    #[test]
    fn native_inspection_relative_write_address_overflow_refuses_globally() {
        let identity = "a".repeat(64);
        for (machine, relative) in [
            (elf::EM_X86_64, elf::R_X86_64_RELATIVE),
            (elf::EM_AARCH64, elf::R_AARCH64_RELATIVE),
        ] {
            let mut relocations = fake_relative_relocations().to_vec();
            for relocation in &mut relocations {
                relocation.r_type = relative;
            }
            let at_limit = u64::MAX.checked_sub(8).unwrap();
            assert_eq!(at_limit.checked_add(8), Some(u64::MAX));
            relocations.push(Rel {
                r_offset: at_limit,
                r_sym: 0,
                r_type: relative,
                r_addend: 0x1010,
            });
            let disjoint = fake_linked_elf_for_machine(Some(&relocations), machine);
            assert_eq!(
                native_records(&disjoint, &identity, &["fake_owner"]),
                Ok(vec![Record {
                    owner: "fake_owner".into(),
                    functions: [0x1010, 0x1020, 0x1030],
                }])
            );
            let overflowing = at_limit.checked_add(1).unwrap();
            assert_eq!(overflowing, u64::MAX.checked_sub(7).unwrap());
            assert_eq!(overflowing.checked_add(8), None);
            let distant = relocations.last_mut().unwrap();
            assert_eq!(distant.r_offset, at_limit);
            distant.r_offset = overflowing;
            let bytes = fake_linked_elf_for_machine(Some(&relocations), machine);
            assert_eq!(
                native_records(&bytes, &identity, &["fake_owner"]),
                Err("inspection relocation address overflow"),
                "machine {machine}, extent arithmetic must refuse even far beyond the record"
            );
        }
    }

    #[test]
    fn native_inspection_none_operations_do_not_poison_relative_function_slots() {
        let identity = "a".repeat(64);
        for (machine, relative, none) in [
            (elf::EM_X86_64, elf::R_X86_64_RELATIVE, elf::R_X86_64_NONE),
            (
                elf::EM_AARCH64,
                elf::R_AARCH64_RELATIVE,
                elf::R_AARCH64_NONE,
            ),
        ] {
            let expected = vec![Record {
                owner: "fake_owner".into(),
                functions: [0x1010, 0x1020, 0x1030],
            }];
            let mut baseline_relocations = fake_relative_relocations().to_vec();
            for relocation in &mut baseline_relocations {
                relocation.r_type = relative;
            }
            let baseline = fake_linked_elf_for_machine(Some(&baseline_relocations), machine);
            assert_eq!(
                native_records(&baseline, &identity, &["fake_owner"]).as_ref(),
                Ok(&expected)
            );
            for offset in [0x2008, 0x20a8, 0x20b0, 0x20b8] {
                for none_first in [false, true] {
                    let mut relocations = baseline_relocations.clone();
                    let noop = Rel {
                        r_offset: offset,
                        r_sym: 0,
                        r_type: none,
                        r_addend: 0,
                    };
                    if none_first {
                        relocations.insert(0, noop);
                    } else {
                        relocations.push(noop);
                    }
                    let bytes = fake_linked_elf_for_machine(Some(&relocations), machine);
                    assert_eq!(
                        native_records(&bytes, &identity, &["fake_owner"]).as_ref(),
                        Ok(&expected),
                        "machine {machine}, NONE at {offset:#x}, none_first={none_first}"
                    );
                }
            }
        }
    }

    #[test]
    fn native_inspection_duplicate_extents_retain_the_widest_write_in_both_orders() {
        let identity = "a".repeat(64);
        for (machine, relative, tlsdesc) in [
            (
                elf::EM_X86_64,
                elf::R_X86_64_RELATIVE,
                elf::R_X86_64_TLSDESC,
            ),
            (
                elf::EM_AARCH64,
                elf::R_AARCH64_RELATIVE,
                elf::R_AARCH64_TLSDESC,
            ),
        ] {
            let expected = vec![Record {
                owner: "fake_owner".into(),
                functions: [0x1010, 0x1020, 0x1030],
            }];
            let mut baseline_relocations = fake_relative_relocations().to_vec();
            for relocation in &mut baseline_relocations {
                relocation.r_type = relative;
            }
            let predecessor = Rel {
                r_offset: 0x2008,
                r_sym: 0,
                r_type: relative,
                r_addend: 0x1010,
            };
            assert_eq!(predecessor.r_offset.checked_add(8), Some(0x2010));
            let mut disjoint_relocations = baseline_relocations.clone();
            disjoint_relocations.push(predecessor.clone());
            let disjoint = fake_linked_elf_for_machine(Some(&disjoint_relocations), machine);
            assert_eq!(
                native_records(&disjoint, &identity, &["fake_owner"]).as_ref(),
                Ok(&expected)
            );
            let descriptor = Rel {
                r_type: tlsdesc,
                ..predecessor.clone()
            };
            assert_eq!(descriptor.r_offset.checked_add(16), Some(0x2018));
            for descriptor_first in [false, true] {
                let mut relocations = baseline_relocations.clone();
                relocations.extend(if descriptor_first {
                    [descriptor.clone(), predecessor.clone()]
                } else {
                    [predecessor.clone(), descriptor.clone()]
                });
                let bytes = fake_linked_elf_for_machine(Some(&relocations), machine);
                assert_eq!(
                    native_records(&bytes, &identity, &["fake_owner"]),
                    Err("inspection relocation outside function slot"),
                    "machine {machine}, descriptor_first={descriptor_first}: the 16-byte write must survive coalescing"
                );
            }
        }
    }

    #[test]
    fn native_inspection_refuses_unaccounted_loader_relocation_encodings() {
        for kind in [elf::SHT_REL, elf::SHT_RELR, elf::SHT_CREL] {
            let bytes = fake_linked_elf_with_extra_encoding(
                Some(&fake_relative_relocations()),
                elf::EM_X86_64,
                Some(kind),
            );
            assert_eq!(
                native_records(&bytes, &"a".repeat(64), &["fake_owner"]),
                Err("unsupported inspection relocation encoding"),
                "section kind {kind}"
            );
        }
    }

    #[test]
    fn native_inspection_refuses_relocatable_objects_instead_of_application_artifacts() {
        use object::{Architecture, BinaryFormat, Endianness, SectionKind, write};
        let mut object =
            write::Object::new(BinaryFormat::Elf, Architecture::Aarch64, Endianness::Little);
        let section =
            object.add_section(Vec::new(), b".rodata".to_vec(), SectionKind::ReadOnlyData);
        object.append_section_data(section, &fake_record(8, true), 8);
        let bytes = object.write().unwrap();
        assert_eq!(
            native_records(&bytes, &"a".repeat(64), &["fake_owner"]),
            Err("inspection requires a linked native executable")
        );
    }
}

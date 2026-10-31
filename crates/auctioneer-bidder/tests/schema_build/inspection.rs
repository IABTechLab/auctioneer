//! Decoder for our compiler-checked private inspection records, not Rust layouts.

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
    let mut rows = Vec::new();
    for section in file.sections() {
        if super::schema_artifact::native_program_data(&file, &section)? {
            let data = section
                .data()
                .map_err(|_error| "invalid native inspection data")?;
            let found = records(data, word, file.is_little_endian(), identity)?;
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
            if *pointer == 0 {
                return Err("null inspection function");
            }
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
    use std::borrow::Cow;
    use std::collections::BTreeMap;
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

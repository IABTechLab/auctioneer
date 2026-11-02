//! Fresh-parser, default-Unicode/UTF-8 construction for the selected ASCII grammar.
//! Requested bytes use explicit compiler-owned payload and ABI layouts, never
//! the inspecting host's sizes. Collection-record visits remain a conditional
//! source model. Excludes allocator overhead, machine stack, diagnostic formatting,
//! HIR consumers and admission.

use super::schema_heap_layout::{HeapLayout, LayoutError, NativeField, private_struct_slot_bound};
use regex_syntax::{
    ast::{self, Ast},
    hir,
};
use sha2::{Digest as _, Sha256};
use std::any::type_name;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::Path;

const MAX_SOURCE_BYTES: u64 = 0x0010_0000;

pub(crate) const SOURCE_PINS: [(&str, &str); 10] = [
    (
        "Cargo.toml",
        "10c3f14629c0900ac452dd6d55a62aab561bbabbdf5774a802c8d223933cfa3b",
    ),
    (
        "src/lib.rs",
        "c51d1e55a8b6c4608e21a278ed0ef9480f73ab5b814b6ca6127f4a049c4d5007",
    ),
    (
        "src/parser.rs",
        "6b2f4f27e3331a01a25b87c89368dd2e54396bd425dac57941f9c1ebfd238ac8",
    ),
    (
        "src/error.rs",
        "e308b3ccad0bea927f4e3957170302e9cfa743bfdf9376f3a5f4137b44ca6cfc",
    ),
    (
        "src/ast/mod.rs",
        "38df06574a3816eae2796c757d9d268b7b42ce3fb1feee86750e7d04de7649c7",
    ),
    (
        "src/ast/parse.rs",
        "af6d82e62e97379a91840e99c10be1279c0ae1298a0d8e5da429e9e7bc3ec339",
    ),
    (
        "src/ast/visitor.rs",
        "2af2efd77727803b8d15a6244af92267e96edbfc1a211bcbea2b32e1b5483918",
    ),
    (
        "src/hir/mod.rs",
        "13ee5b65fac1f2c9780ce48a500b1e9d198cb0bc07c0d7f4a4391aab87424563",
    ),
    (
        "src/hir/translate.rs",
        "8c01783e74261a8562c2694394d94ccb8cb173274932731fa77118041fab4507",
    ),
    (
        "src/hir/interval.rs",
        "170949e6fd9e9641830415ec3ea813ff2431c8594d49c559cd41d4903f3d29d0",
    ),
];

pub(crate) const PRIVATE_TYPES: [&str; 6] = [
    "regex_syntax::ast::parse::GroupState",
    "regex_syntax::ast::parse::ClassState",
    "regex_syntax::hir::translate::HirFrame",
    "regex_syntax::ast::visitor::Frame",
    "regex_syntax::ast::visitor::ClassFrame",
    "regex_syntax::ast::visitor::ClassInduct",
];

pub(crate) const PAYLOAD_TYPES: [&str; 17] = [
    "regex_syntax::ast::Ast",
    "regex_syntax::ast::Span",
    "regex_syntax::ast::Group",
    "regex_syntax::ast::Literal",
    "regex_syntax::ast::Assertion",
    "regex_syntax::ast::ClassBracketed",
    "regex_syntax::ast::Repetition",
    "regex_syntax::ast::Concat",
    "regex_syntax::ast::Alternation",
    "regex_syntax::ast::ClassSetItem",
    "regex_syntax::ast::Comment",
    "regex_syntax::ast::CaptureName",
    "regex_syntax::ast::ClassSet",
    "regex_syntax::hir::Hir",
    "regex_syntax::hir::ClassUnicodeRange",
    "regex_syntax::hir::ClassBytesRange",
    "regex_syntax::hir::PropertiesI",
];

/// Supplied from the inspected artifact's verified pointer ABI, not a Rust type query.
pub(crate) const ABI_POINTER: &str = "compiler_abi::Pointer";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ConstructionError {
    Arithmetic,
    Layout,
    Parse,
    Scope(&'static str),
    Source(&'static str),
    UnboundOwner(&'static str),
}

impl From<LayoutError> for ConstructionError {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Shape => Self::Layout,
            LayoutError::Arithmetic => Self::Arithmetic,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SyntaxCounts {
    pub input_bytes: usize,
    pub ast_nodes: usize,
    pub class_items: usize,
    pub groups: usize,
    pub repetitions: usize,
    pub concatenations: usize,
    pub alternations: usize,
    pub literals: usize,
    pub class_ranges: usize,
    pub classes: usize,
    pub alternate_branches: usize,
    pub captures: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnerCharge {
    pub owner: &'static str,
    pub phase: &'static str,
    pub instances: usize,
    pub element_bytes: usize,
    pub high_water: usize,
    pub retained_bytes: usize,
    pub old_growth_bytes: usize,
    pub allocation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SyntaxConstructionCertificate {
    pub counts: SyntaxCounts,
    pub ast_peak_bytes: usize,
    pub parser_stack_peak_bytes: usize,
    pub translation_stack_peak_bytes: usize,
    pub hir_construction_peak_bytes: usize,
    pub drop_scratch_bytes: usize,
    pub failure_prefix_peak_bytes: usize,
    pub peak_requested_bytes: usize,
    /// Source byte/record visits, not instructions, time or an engine counter.
    pub work_upper_bound: usize,
    pub owners: Vec<OwnerCharge>,
    pub work_terms: BTreeMap<&'static str, usize>,
}

impl SyntaxConstructionCertificate {
    pub(crate) fn from_pattern(
        pattern: &str,
        layouts: &BTreeMap<String, NativeField>,
        source: &Path,
    ) -> Result<Self, ConstructionError> {
        verify_source(source)?;
        conditional_certificate(pattern, layouts)
    }
}

fn verify_source(source: &Path) -> Result<(), ConstructionError> {
    let canonical = fs::canonicalize(source)
        .map_err(|_error| ConstructionError::Source("missing regex-syntax source"))?;
    if canonical != source
        || source
            .file_name()
            .is_none_or(|name| name != "regex-syntax-0.8.11")
    {
        return Err(ConstructionError::Source(
            "noncanonical regex-syntax-0.8.11 source",
        ));
    }
    for (relative, expected) in SOURCE_PINS {
        let mut path = source.to_owned();
        for component in Path::new(relative).components() {
            path.push(component);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_error| ConstructionError::Source(relative))?;
            if metadata.file_type().is_symlink() {
                return Err(ConstructionError::Source(relative));
            }
        }
        let file = File::open(&path).map_err(|_error| ConstructionError::Source(relative))?;
        let metadata = file
            .metadata()
            .map_err(|_error| ConstructionError::Source(relative))?;
        if !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES {
            return Err(ConstructionError::Source(relative));
        }
        let mut bytes = Vec::new();
        file.take(MAX_SOURCE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_error| ConstructionError::Source(relative))?;
        verify_source_bytes(relative, expected, &bytes)?;
    }
    Ok(())
}

fn verify_source_bytes(
    relative: &'static str,
    expected: &str,
    bytes: &[u8],
) -> Result<(), ConstructionError> {
    if format!("{:x}", Sha256::digest(bytes)) != expected {
        return Err(ConstructionError::Source(relative));
    }
    Ok(())
}

#[derive(Default)]
struct AstCounts {
    counts: SyntaxCounts,
    boxes: BTreeMap<&'static str, (&'static str, usize)>,
}

impl AstCounts {
    fn boxed<T>(&mut self, owner: &'static str, instances: usize) -> Result<(), ConstructionError> {
        let entry = self.boxes.entry(owner).or_insert((type_name::<T>(), 0));
        entry.1 = add(entry.1, instances)?;
        Ok(())
    }

    fn group(&mut self, group: &ast::Group) -> Result<(), ConstructionError> {
        if matches!(group.kind, ast::GroupKind::CaptureName { .. })
            || group.flags().is_some_and(|flags| !flags.items.is_empty())
        {
            return Err(ConstructionError::Scope("flags or named capture"));
        }
        self.counts.groups = add(self.counts.groups, 1)?;
        if group.is_capturing() {
            self.counts.captures = add(self.counts.captures, 1)?;
        }
        self.boxed::<ast::Group>("AST Group payload", 1)?;
        // parse_group creates Empty, then pop_group replaces that Box<Ast>.
        self.boxed::<Ast>("AST group placeholder/body boxes", 2)?;
        self.boxed::<ast::Span>("AST group placeholder Empty", 1)
    }
}

impl ast::Visitor for AstCounts {
    type Output = Self;
    type Err = ConstructionError;

    fn finish(self) -> Result<Self, Self::Err> {
        Ok(self)
    }

    fn visit_pre(&mut self, node: &Ast) -> Result<(), Self::Err> {
        self.counts.ast_nodes = add(self.counts.ast_nodes, 1)?;
        match node {
            Ast::Empty(_) => self.boxed::<ast::Span>("AST Empty payload", 1)?,
            Ast::Literal(literal) => {
                ascii(literal.c)?;
                self.counts.literals = add(self.counts.literals, 1)?;
                self.boxed::<ast::Literal>("AST Literal payload", 1)?;
            }
            Ast::Assertion(assertion) => {
                if !matches!(
                    assertion.kind,
                    ast::AssertionKind::StartLine
                        | ast::AssertionKind::EndLine
                        | ast::AssertionKind::StartText
                        | ast::AssertionKind::EndText
                ) {
                    return Err(ConstructionError::Scope("non-anchor assertion"));
                }
                self.boxed::<ast::Assertion>("AST Assertion payload", 1)?;
            }
            Ast::ClassBracketed(class) => {
                if class.negated {
                    return Err(ConstructionError::Scope("negated class"));
                }
                if class_coverage(&class.kind)?.count_ones() <= 1 {
                    return Err(ConstructionError::UnboundOwner(
                        "Hir::class singleton/empty rewrite",
                    ));
                }
                self.counts.classes = add(self.counts.classes, 1)?;
                self.boxed::<ast::ClassBracketed>("AST bracketed class payload", 1)?;
            }
            Ast::Repetition(_) => {
                self.counts.repetitions = add(self.counts.repetitions, 1)?;
                self.boxed::<ast::Repetition>("AST Repetition payload", 1)?;
                self.boxed::<Ast>("AST Repetition child box", 1)?;
            }
            Ast::Group(group) => self.group(group)?,
            Ast::Concat(_) => {
                self.counts.concatenations = add(self.counts.concatenations, 1)?;
                self.boxed::<ast::Concat>("AST Concat payload", 1)?;
            }
            Ast::Alternation(alternation) => {
                selected_alternation(alternation)?;
                self.counts.alternations = add(self.counts.alternations, 1)?;
                self.counts.alternate_branches =
                    add(self.counts.alternate_branches, alternation.asts.len())?;
                self.boxed::<ast::Alternation>("AST Alternation payload", 1)?;
            }
            Ast::Flags(_) | Ast::Dot(_) | Ast::ClassUnicode(_) | Ast::ClassPerl(_) => {
                return Err(ConstructionError::Scope(
                    "dot, flags or untranslated Unicode/Perl class",
                ));
            }
        }
        Ok(())
    }

    fn visit_class_set_item_pre(&mut self, item: &ast::ClassSetItem) -> Result<(), Self::Err> {
        self.counts.class_items = add(self.counts.class_items, 1)?;
        match item {
            ast::ClassSetItem::Literal(literal) => {
                ascii(literal.c)?;
            }
            ast::ClassSetItem::Range(range) => {
                ascii(range.start.c)?;
                ascii(range.end.c)?;
            }
            ast::ClassSetItem::Union(_) => return Ok(()),
            ast::ClassSetItem::Empty(_)
            | ast::ClassSetItem::Ascii(_)
            | ast::ClassSetItem::Unicode(_)
            | ast::ClassSetItem::Perl(_)
            | ast::ClassSetItem::Bracketed(_) => {
                return Err(ConstructionError::Scope(
                    "nested, named, Unicode or Perl class item",
                ));
            }
        }
        self.counts.class_ranges = add(self.counts.class_ranges, 1)?;
        Ok(())
    }

    fn visit_class_set_binary_op_pre(
        &mut self,
        _: &ast::ClassSetBinaryOp,
    ) -> Result<(), Self::Err> {
        Err(ConstructionError::Scope("binary class operation"))
    }
}

fn ascii(character: char) -> Result<(), ConstructionError> {
    if character.is_ascii() {
        Ok(())
    } else {
        Err(ConstructionError::Scope("non-ASCII literal"))
    }
}

fn class_coverage(set: &ast::ClassSet) -> Result<u128, ConstructionError> {
    match set {
        ast::ClassSet::Item(item) => item_coverage(item),
        ast::ClassSet::BinaryOp(_) => Err(ConstructionError::Scope("binary class operation")),
    }
}

fn item_coverage(item: &ast::ClassSetItem) -> Result<u128, ConstructionError> {
    let (start, end) = match item {
        ast::ClassSetItem::Literal(literal) => (literal.c, literal.c),
        ast::ClassSetItem::Range(range) => (range.start.c, range.end.c),
        ast::ClassSetItem::Union(union) => {
            return union
                .items
                .iter()
                .try_fold(0, |bits, union_item| Ok(bits | item_coverage(union_item)?));
        }
        ast::ClassSetItem::Empty(_)
        | ast::ClassSetItem::Ascii(_)
        | ast::ClassSetItem::Unicode(_)
        | ast::ClassSetItem::Perl(_)
        | ast::ClassSetItem::Bracketed(_) => {
            return Err(ConstructionError::Scope(
                "nested, named, Unicode or Perl class item",
            ));
        }
    };
    ascii(start)?;
    ascii(end)?;
    Ok((u32::from(start)..=u32::from(end)).fold(0_u128, |bits, byte| bits | (1_u128 << byte)))
}

fn selected_alternation(alternation: &ast::Alternation) -> Result<(), ConstructionError> {
    // In the selected hostname alternations the first child is a non-singleton
    // Unicode class and the second has multiple consuming children. Thus the
    // singleton probes allocate nothing; both class-union probes stop after
    // their first canonical class; lift_common_prefix returns before allocating.
    let [Ast::ClassBracketed(_), Ast::Concat(concat)] = alternation.asts.as_slice() else {
        return Err(ConstructionError::UnboundOwner(
            "Hir::alternation non-selected rewrite",
        ));
    };
    if concat.asts.len() < 2
        || !concat.asts.iter().all(|node| {
            matches!(
                node,
                Ast::ClassBracketed(_) | Ast::Literal(_) | Ast::Repetition(_)
            )
        })
        || concat
            .asts
            .iter()
            .filter(|node| matches!(node, Ast::ClassBracketed(_) | Ast::Literal(_)))
            .count()
            < 2
    {
        return Err(ConstructionError::UnboundOwner(
            "Hir::alternation prefix factoring",
        ));
    }
    Ok(())
}

struct Ledger {
    layout: HeapLayout,
    owners: Vec<OwnerCharge>,
    work: BTreeMap<&'static str, usize>,
}

impl Ledger {
    fn boxes(
        &mut self,
        owner: &'static str,
        phase: &'static str,
        bytes: usize,
        instances: usize,
    ) -> Result<(), ConstructionError> {
        let retained_bytes = self.layout.repeat_vector(bytes, instances)?;
        self.owners.push(OwnerCharge {
            owner,
            phase,
            instances,
            element_bytes: bytes,
            high_water: 1,
            retained_bytes,
            old_growth_bytes: 0,
            allocation: "fixed_payload",
        });
        Ok(())
    }

    fn vector(
        &mut self,
        owner: &'static str,
        phase: &'static str,
        bytes: usize,
        instances: usize,
        high_water: usize,
    ) -> Result<(), ConstructionError> {
        let envelope = self.layout.append_vector(bytes, high_water)?;
        self.owners.push(OwnerCharge {
            owner,
            phase,
            instances,
            element_bytes: bytes,
            high_water,
            retained_bytes: mul(instances, envelope.retained_bytes)?,
            old_growth_bytes: mul(instances, envelope.old_growth_bytes)?,
            allocation: "append_vector",
        });
        // Except the reused class parser stack (charged separately), these
        // high-waters also bound total appends, or sum the disjoint drop trees.
        // Each growth/shift copies at most high_water initialized records.
        self.work
            .insert(owner, mul(instances, mul(high_water, high_water)?)?);
        Ok(())
    }

    fn phase(&self, phase: &str) -> Result<usize, ConstructionError> {
        self.owners
            .iter()
            .filter(|owner| owner.phase == phase)
            .try_fold(0, |bytes, owner| {
                add(bytes, add(owner.retained_bytes, owner.old_growth_bytes)?)
            })
    }
}

fn conditional_certificate(
    pattern: &str,
    layouts: &BTreeMap<String, NativeField>,
) -> Result<SyntaxConstructionCertificate, ConstructionError> {
    for name in PRIVATE_TYPES
        .into_iter()
        .chain(PAYLOAD_TYPES)
        .chain([ABI_POINTER])
    {
        layouts
            .get(name)
            .ok_or(ConstructionError::UnboundOwner(name))?
            .validate()?;
    }
    let pointer = layouts[ABI_POINTER];
    if !matches!(pointer.bytes, 4 | 8) || pointer.alignment != pointer.bytes {
        return Err(ConstructionError::Layout);
    }
    if pattern.is_empty() || pattern.len() > 4096 || !pattern.is_ascii() {
        return Err(ConstructionError::Scope(
            "empty, oversized or non-ASCII pattern",
        ));
    }
    let ast = ast::parse::Parser::new()
        .parse(pattern)
        .map_err(|_error| ConstructionError::Parse)?;
    let mut model = ast::visit(&ast, AstCounts::default())?;
    model.counts.input_bytes = pattern.len();
    // The public translator is the engine, not a hand-written replacement.
    let _hir = hir::translate::Translator::new()
        .translate(pattern, &ast)
        .map_err(|_error| ConstructionError::Parse)?;
    let mut ledger = Ledger {
        layout: HeapLayout::new(pointer.bytes, pointer.bytes)?,
        owners: Vec::new(),
        work: BTreeMap::new(),
    };
    parser_owners(&mut ledger, &model, layouts)?;
    translator_owners(&mut ledger, model.counts, layouts)?;
    hir_owners(
        &mut ledger,
        model.counts,
        layouts["regex_syntax::hir::PropertiesI"].bytes,
        layouts,
    )?;
    source_work(&mut ledger, model.counts)?;
    let ast_peak_bytes = ledger.phase("ast")?;
    let parser_stack_peak_bytes = ledger.phase("parser")?;
    let translation_stack_peak_bytes = ledger.phase("translation")?;
    let hir_construction_peak_bytes = ledger.phase("hir")?;
    let drop_scratch_bytes = ledger.phase("drop")?;
    let peak_requested_bytes = sum(&[
        ast_peak_bytes,
        parser_stack_peak_bytes,
        translation_stack_peak_bytes,
        hir_construction_peak_bytes,
        drop_scratch_bytes,
        ledger.phase("failure")?,
    ])?;
    let work_upper_bound = ledger
        .work
        .values()
        .try_fold(0, |total, value| add(total, *value))?;
    Ok(SyntaxConstructionCertificate {
        counts: model.counts,
        ast_peak_bytes,
        parser_stack_peak_bytes,
        translation_stack_peak_bytes,
        hir_construction_peak_bytes,
        drop_scratch_bytes,
        failure_prefix_peak_bytes: peak_requested_bytes,
        peak_requested_bytes,
        work_upper_bound,
        owners: ledger.owners,
        work_terms: ledger.work,
    })
}

fn visitor_owners(
    ledger: &mut Ledger,
    phase: &'static str,
    c: SyntaxCounts,
    layouts: &BTreeMap<String, NativeField>,
) -> Result<(), ConstructionError> {
    let ast_pair = private_struct_slot_bound(&[layouts[ABI_POINTER], layouts[PRIVATE_TYPES[3]]])?;
    let class_pair =
        private_struct_slot_bound(&[layouts[PRIVATE_TYPES[5]], layouts[PRIVATE_TYPES[4]]])?;
    let (ast_name, class_name) = if phase == "parser" {
        ("nest visitor AST stack", "nest visitor class stack")
    } else {
        (
            "translate visitor AST stack",
            "translate visitor class stack",
        )
    };
    ledger.vector(ast_name, phase, ast_pair.bytes, 1, c.ast_nodes)?;
    ledger.vector(class_name, phase, class_pair.bytes, 1, c.class_items)?;
    Ok(())
}

fn parser_owners(
    ledger: &mut Ledger,
    model: &AstCounts,
    layouts: &BTreeMap<String, NativeField>,
) -> Result<(), ConstructionError> {
    let c = model.counts;
    for (owner, (name, instances)) in &model.boxes {
        let payload = layouts
            .get(*name)
            .ok_or(ConstructionError::UnboundOwner(name))?;
        ledger.boxes(owner, "ast", payload.bytes, *instances)?;
    }
    // One root concat, one per group, one per alternate separator. Alternation
    // vectors add one owner per alternation, yielding 1 + groups + branches.
    let vectors = sum(&[1, c.groups, c.alternate_branches])?;
    ledger.vector(
        "parser concat/alternate vectors",
        "ast",
        layouts["regex_syntax::ast::Ast"].bytes,
        vectors,
        c.ast_nodes,
    )?;
    // A class starts a union Vec even when into_item later removes that Vec.
    ledger.vector(
        "parser class item vectors",
        "ast",
        layouts["regex_syntax::ast::ClassSetItem"].bytes,
        c.classes,
        c.class_items,
    )?;
    ledger.vector(
        "parser group stack",
        "parser",
        layouts[PRIVATE_TYPES[0]].bytes,
        1,
        add(c.groups, c.alternations)?,
    )?;
    // Nested brackets and binary operations refuse, so only one Open can live.
    ledger.vector(
        "parser class stack",
        "parser",
        layouts[PRIVATE_TYPES[1]].bytes,
        1,
        usize::from(c.classes != 0),
    )?;
    // This stack has depth <=1, but is pushed and popped for multiple classes.
    // Its capacity is retained after the first class; no later growth occurs.
    ledger.work.insert("parser class stack", mul(2, c.classes)?);
    // Every scratch push consumes an ASCII source byte (decimal/hex parsing).
    ledger.vector("parser scratch String", "parser", 1, 1, c.input_bytes)?;
    // Default x=false and the no-flags/no-named-captures scope make these zero.
    ledger.vector(
        "parser comments",
        "parser",
        layouts["regex_syntax::ast::Comment"].bytes,
        1,
        0,
    )?;
    ledger.vector(
        "parser capture names",
        "parser",
        layouts["regex_syntax::ast::CaptureName"].bytes,
        1,
        0,
    )?;
    visitor_owners(ledger, "parser", c, layouts)?;
    ledger.boxes("failure error pattern String", "failure", c.input_bytes, 1)?;
    Ok(())
}

fn translator_owners(
    ledger: &mut Ledger,
    c: SyntaxCounts,
    layouts: &BTreeMap<String, NativeField>,
) -> Result<(), ConstructionError> {
    // pre/post each AST node, one branch sentinel per branch and one class
    // pop/push per range: all lifetime pushes, not an observed stack peak.
    let pushes = sum(&[mul(2, c.ast_nodes)?, c.alternate_branches, c.class_ranges])?;
    ledger.vector(
        "translator HirFrame stack",
        "translation",
        layouts[PRIVATE_TYPES[2]].bytes,
        1,
        pushes,
    )?;
    visitor_owners(ledger, "translation", c, layouts)?;
    ledger.vector(
        "translator literal Vec",
        "hir",
        1,
        c.literals,
        c.input_bytes,
    )?;
    // push_char's to_vec may seed capacity one, below the ordinary push
    // minimum. Fund that first-growth overlap separately from the boxed shrink.
    ledger.boxes(
        "translator literal initial copy",
        "hir",
        c.input_bytes,
        c.literals,
    )?;
    ledger.boxes(
        "translator literal boxed slice",
        "hir",
        c.input_bytes,
        c.literals,
    )?;
    ledger.vector(
        "translator class ranges",
        "hir",
        layouts["regex_syntax::hir::ClassUnicodeRange"].bytes,
        c.classes,
        c.class_ranges,
    )?;
    let constructors = add(c.concatenations, c.alternations)?;
    ledger.vector(
        "translator concat/alternate input Vec",
        "hir",
        layouts["regex_syntax::hir::Hir"].bytes,
        constructors,
        c.ast_nodes,
    )?;
    Ok(())
}

fn hir_owners(
    ledger: &mut Ledger,
    c: SyntaxCounts,
    properties_bytes: usize,
    layouts: &BTreeMap<String, NativeField>,
) -> Result<(), ConstructionError> {
    let constructors = add(c.concatenations, c.alternations)?;
    let rebuilt_literals = mul(c.concatenations, c.literals)?;
    let hir_nodes = add(c.ast_nodes, rebuilt_literals)?;
    let unary = add(c.repetitions, c.captures)?;
    let consumed = mul(constructors, c.ast_nodes)?;
    ledger.boxes(
        "translator capture/repetition Box<Hir>",
        "hir",
        layouts["regex_syntax::hir::Hir"].bytes,
        unary,
    )?;
    ledger.vector(
        "Hir concat/alternate rebuilt Vec",
        "hir",
        layouts["regex_syntax::hir::Hir"].bytes,
        constructors,
        c.ast_nodes,
    )?;
    ledger.vector(
        "Hir concat rebuilt literal Vec",
        "hir",
        1,
        rebuilt_literals,
        c.input_bytes,
    )?;
    ledger.boxes(
        "Hir concat literal initial copy",
        "hir",
        c.input_bytes,
        rebuilt_literals,
    )?;
    ledger.boxes(
        "Hir concat rebuilt boxed literals",
        "hir",
        c.input_bytes,
        rebuilt_literals,
    )?;
    // Failed class_chars/class_bytes probes retain at most their first class.
    // Canonical ranges are copied in order, so canonicalize returns before sort.
    ledger.vector(
        "alternate Unicode probe ranges",
        "hir",
        layouts["regex_syntax::hir::ClassUnicodeRange"].bytes,
        c.alternations,
        c.class_ranges,
    )?;
    ledger.vector(
        "alternate byte probe ranges",
        "hir",
        layouts["regex_syntax::hir::ClassBytesRange"].bytes,
        c.alternations,
        c.class_ranges,
    )?;
    ledger.vector(
        "alternate Unicode-to-byte conversion",
        "hir",
        layouts["regex_syntax::hir::ClassBytesRange"].bytes,
        c.alternations,
        c.class_ranges,
    )?;
    ledger.boxes(
        "HIR Properties constructors",
        "hir",
        properties_bytes,
        hir_nodes,
    )?;
    ledger.boxes(
        "Hir into_parts replacement Properties",
        "hir",
        properties_bytes,
        consumed,
    )?;
    drop_owners(ledger, c, properties_bytes, hir_nodes, unary, layouts)
}

fn drop_owners(
    ledger: &mut Ledger,
    c: SyntaxCounts,
    properties_bytes: usize,
    hir_nodes: usize,
    unary: usize,
    layouts: &BTreeMap<String, NativeField>,
) -> Result<(), ConstructionError> {
    // Drop's manual stacks avoid nested same-type drop stacks. AST's stack may
    // coexist with a class-set drop stack, and all retained parser/HIR owners.
    ledger.vector(
        "AST drop stack",
        "drop",
        layouts["regex_syntax::ast::Ast"].bytes,
        1,
        c.ast_nodes,
    )?;
    ledger.vector(
        "ClassSet drop stack",
        "drop",
        layouts["regex_syntax::ast::ClassSet"].bytes,
        1,
        add(c.class_items, c.classes)?,
    )?;
    ledger.vector(
        "HIR drop stack",
        "drop",
        layouts["regex_syntax::hir::Hir"].bytes,
        1,
        hir_nodes,
    )?;
    ledger.boxes(
        "AST drop replacement Span boxes",
        "drop",
        layouts["regex_syntax::ast::Span"].bytes,
        sum(&[c.ast_nodes, c.groups, c.repetitions])?,
    )?;
    ledger.boxes(
        "HIR drop replacement Properties",
        "drop",
        properties_bytes,
        add(hir_nodes, unary)?,
    )?;
    Ok(())
}

fn source_work(ledger: &mut Ledger, c: SyntaxCounts) -> Result<(), ConstructionError> {
    let n = c.ast_nodes;
    let k = add(c.concatenations, c.alternations)?;
    let ranges = c.class_ranges;
    // Units are collection record/byte visits on these loops. Constant-time
    // scalar helpers, instructions and diagnostic formatting are not units.
    for (name, visits) in [
        ("parser ASCII cursor", c.input_bytes),
        ("parser scratch reparse", c.input_bytes),
        ("nest visitor pre/post", mul(2, add(n, c.class_items)?)?),
        (
            "translator visitor pre/post",
            mul(2, add(n, c.class_items)?)?,
        ),
        ("translator child pops and sentinel", mul(k, add(n, 1)?)?),
        ("translator input reverse", mul(k, n)?),
        ("Hir concat flatten", mul(c.concatenations, n)?),
        (
            "Hir concat literal copies",
            mul(mul(c.concatenations, c.literals)?, c.input_bytes)?,
        ),
        (
            "literal boxed-shrink byte copies",
            mul(
                add(c.literals, mul(c.concatenations, c.literals)?)?,
                c.input_bytes,
            )?,
        ),
        // Properties::concat has all-child, prefix and suffix passes.
        (
            "Properties concat passes",
            mul(3, mul(c.concatenations, n)?)?,
        ),
        ("Hir alternate flatten", mul(c.alternations, n)?),
        (
            "alternate singleton probes",
            mul(2, mul(c.alternations, n)?)?,
        ),
        ("alternate class probes", mul(2, mul(c.alternations, n)?)?),
        ("alternate first-child prefix probe", c.alternations),
        ("Properties alternate union", mul(c.alternations, n)?),
        // Each push bounds binary-search comparisons, following-interval
        // unions, and insertion movement by the lifetime input range count.
        (
            "interval push searches/unions/shifts",
            mul(3, mul(ranges, ranges)?)?,
        ),
        // For the selected first-class-only probes: Unicode extend/check,
        // Unicode-to-byte ASCII scan/map/check, and byte extend/check.
        (
            "alternate range probe passes",
            mul(7, mul(c.alternations, ranges)?)?,
        ),
        (
            "AST drop node and replacement visits",
            sum(&[mul(2, n)?, c.groups, c.repetitions])?,
        ),
        (
            "class drop and empty replacement visits",
            add(mul(2, c.class_items)?, c.classes)?,
        ),
        (
            "HIR drop node and replacement visits",
            add(
                mul(2, add(n, mul(c.concatenations, c.literals)?)?)?,
                add(c.repetitions, c.captures)?,
            )?,
        ),
    ] {
        ledger.work.insert(name, visits);
    }
    Ok(())
}

fn add(left: usize, right: usize) -> Result<usize, ConstructionError> {
    left.checked_add(right).ok_or(ConstructionError::Arithmetic)
}

fn mul(left: usize, right: usize) -> Result<usize, ConstructionError> {
    left.checked_mul(right).ok_or(ConstructionError::Arithmetic)
}

fn sum(values: &[usize]) -> Result<usize, ConstructionError> {
    values.iter().try_fold(0, |total, value| add(total, *value))
}

#[cfg(test)]
mod tests {
    use super::{
        ABI_POINTER, ConstructionError, NativeField, PAYLOAD_TYPES, PRIVATE_TYPES,
        conditional_certificate,
    };
    use crate::schema_formats::HOSTNAME_SOURCE;
    use regex_syntax::{ast, hir};
    use std::any::type_name;
    use std::collections::BTreeMap;

    fn fake_facts() -> BTreeMap<String, NativeField> {
        let mut facts: BTreeMap<_, _> = PRIVATE_TYPES
            .into_iter()
            .map(|name| (name.to_owned(), NativeField::of::<[usize; 5]>()))
            .collect();
        facts.extend(
            [
                (type_name::<ast::Ast>(), NativeField::of::<ast::Ast>()),
                (type_name::<ast::Span>(), NativeField::of::<ast::Span>()),
                (type_name::<ast::Group>(), NativeField::of::<ast::Group>()),
                (
                    type_name::<ast::Literal>(),
                    NativeField::of::<ast::Literal>(),
                ),
                (
                    type_name::<ast::Assertion>(),
                    NativeField::of::<ast::Assertion>(),
                ),
                (
                    type_name::<ast::ClassBracketed>(),
                    NativeField::of::<ast::ClassBracketed>(),
                ),
                (
                    type_name::<ast::Repetition>(),
                    NativeField::of::<ast::Repetition>(),
                ),
                (type_name::<ast::Concat>(), NativeField::of::<ast::Concat>()),
                (
                    type_name::<ast::Alternation>(),
                    NativeField::of::<ast::Alternation>(),
                ),
                (
                    type_name::<ast::ClassSetItem>(),
                    NativeField::of::<ast::ClassSetItem>(),
                ),
                (
                    type_name::<ast::Comment>(),
                    NativeField::of::<ast::Comment>(),
                ),
                (
                    type_name::<ast::CaptureName>(),
                    NativeField::of::<ast::CaptureName>(),
                ),
                (
                    type_name::<ast::ClassSet>(),
                    NativeField::of::<ast::ClassSet>(),
                ),
                (type_name::<hir::Hir>(), NativeField::of::<hir::Hir>()),
                (
                    type_name::<hir::ClassUnicodeRange>(),
                    NativeField::of::<hir::ClassUnicodeRange>(),
                ),
                (
                    type_name::<hir::ClassBytesRange>(),
                    NativeField::of::<hir::ClassBytesRange>(),
                ),
                (ABI_POINTER, NativeField::of::<usize>()),
            ]
            .into_iter()
            .map(|(name, layout)| (name.to_owned(), layout)),
        );
        facts.insert(
            "regex_syntax::hir::PropertiesI".into(),
            NativeField {
                bytes: regex_syntax::parse("a")
                    .unwrap()
                    .properties()
                    .memory_usage(),
                alignment: align_of::<usize>(),
            },
        );
        facts
    }

    #[test]
    fn syntax_construction_selected_shapes_fund_every_phase_with_explicit_facts() {
        for pattern in ["^[0-9]+$", "[0-9]+", r"\.[a-zA-Z]{2,3}$", HOSTNAME_SOURCE] {
            let certificate = conditional_certificate(pattern, &fake_facts()).unwrap();
            assert_eq!(certificate.counts.input_bytes, pattern.len());
            assert!(certificate.ast_peak_bytes > 0);
            assert!(certificate.parser_stack_peak_bytes > 0);
            assert!(certificate.translation_stack_peak_bytes > 0);
            assert!(certificate.hir_construction_peak_bytes > 0);
            assert!(certificate.drop_scratch_bytes > 0);
            assert_eq!(
                certificate.failure_prefix_peak_bytes,
                certificate.peak_requested_bytes
            );
            assert!(certificate.work_upper_bound > certificate.counts.ast_nodes);
        }
        let numeric = conditional_certificate("^[0-9]+$", &fake_facts()).unwrap();
        assert_eq!(numeric.counts.ast_nodes, 5);
        assert_eq!(numeric.counts.repetitions, 1);
        assert_eq!(numeric.counts.class_ranges, 1);
    }

    #[test]
    fn syntax_construction_every_payload_and_pointer_fact_is_mandatory_and_shape_checked() {
        for name in PRIVATE_TYPES
            .into_iter()
            .chain(PAYLOAD_TYPES)
            .chain([ABI_POINTER])
        {
            let mut missing_facts = fake_facts();
            missing_facts.remove(name);
            assert_eq!(
                conditional_certificate("^[0-9]+$", &missing_facts),
                Err(ConstructionError::UnboundOwner(name))
            );
            let mut invalid_facts = fake_facts();
            invalid_facts.insert(
                name.to_owned(),
                NativeField {
                    bytes: 0,
                    alignment: 1,
                },
            );
            assert_eq!(
                conditional_certificate("^[0-9]+$", &invalid_facts),
                Err(ConstructionError::Layout)
            );
        }
    }

    #[test]
    fn syntax_construction_each_private_slot_changes_the_hostname_envelope() {
        let facts = fake_facts();
        let baseline = conditional_certificate(HOSTNAME_SOURCE, &facts).unwrap();
        for name in PRIVATE_TYPES {
            let mut changed = facts.clone();
            changed.insert(name.to_owned(), NativeField::of::<[usize; 17]>());
            let expanded = conditional_certificate(HOSTNAME_SOURCE, &changed).unwrap();
            assert!(
                expanded.peak_requested_bytes > baseline.peak_requested_bytes,
                "{name}"
            );
            assert_eq!(expanded.counts, baseline.counts);
        }
    }

    #[test]
    fn syntax_construction_uses_reported_public_payloads_instead_of_host_sizes() {
        let mut facts = fake_facts();
        for name in [
            "regex_syntax::ast::Ast",
            "regex_syntax::ast::Group",
            "regex_syntax::ast::ClassSet",
            "regex_syntax::hir::Hir",
            "regex_syntax::hir::PropertiesI",
        ] {
            facts.insert(
                name.into(),
                NativeField {
                    bytes: 4096,
                    alignment: 8,
                },
            );
        }
        let certificate = conditional_certificate(HOSTNAME_SOURCE, &facts).unwrap();
        for owner in [
            "AST Group payload",
            "AST group placeholder/body boxes",
            "translator capture/repetition Box<Hir>",
            "HIR Properties constructors",
            "ClassSet drop stack",
        ] {
            let row = certificate
                .owners
                .iter()
                .find(|row| row.owner == owner)
                .unwrap();
            assert!(row.instances > 0, "{owner}");
            assert_eq!(row.element_bytes, 4096, "{owner}");
        }
    }

    #[test]
    fn syntax_construction_pointer_width_changes_only_layout_not_logical_counts() {
        let mut narrow_facts = fake_facts();
        narrow_facts.insert(
            ABI_POINTER.into(),
            NativeField {
                bytes: 4,
                alignment: 4,
            },
        );
        narrow_facts.insert(
            PRIVATE_TYPES[3].into(),
            NativeField {
                bytes: 8,
                alignment: 4,
            },
        );
        let mut wide_facts = narrow_facts.clone();
        wide_facts.insert(
            ABI_POINTER.into(),
            NativeField {
                bytes: 8,
                alignment: 8,
            },
        );
        let narrow = conditional_certificate("a", &narrow_facts).unwrap();
        let wide = conditional_certificate("a", &wide_facts).unwrap();
        assert_eq!(narrow.counts, wide.counts);
        for owner in ["nest visitor AST stack", "translate visitor AST stack"] {
            let bytes = |certificate: &super::SyntaxConstructionCertificate| {
                certificate
                    .owners
                    .iter()
                    .find(|row| row.owner == owner)
                    .unwrap()
                    .element_bytes
            };
            assert_eq!(bytes(&narrow), 12, "{owner}");
            assert_eq!(bytes(&wide), 16, "{owner}");
        }
    }

    #[test]
    fn syntax_construction_rejects_unknown_or_inconsistent_pointer_abis() {
        for pointer in [
            NativeField {
                bytes: 1,
                alignment: 1,
            },
            NativeField {
                bytes: 2,
                alignment: 2,
            },
            NativeField {
                bytes: 3,
                alignment: 1,
            },
            NativeField {
                bytes: 16,
                alignment: 16,
            },
            NativeField {
                bytes: 4,
                alignment: 1,
            },
            NativeField {
                bytes: 8,
                alignment: 4,
            },
        ] {
            let mut facts = fake_facts();
            facts.insert(ABI_POINTER.into(), pointer);
            assert_eq!(
                conditional_certificate("a", &facts),
                Err(ConstructionError::Layout)
            );
        }
    }

    #[test]
    fn syntax_construction_refuses_untranslated_or_unmodeled_syntax() {
        for pattern in [
            r"\d+",
            r"\p{L}",
            "(?i:a)",
            "(?P<fake_name>a)",
            "[^a]",
            "[a&&b]",
            "[a--b]",
            ".",
            "\u{e9}",
        ] {
            assert!(
                matches!(
                    conditional_certificate(pattern, &fake_facts()),
                    Err(ConstructionError::Scope(_))
                ),
                "{pattern:?}"
            );
        }
        assert_eq!(
            conditional_certificate("[", &fake_facts()),
            Err(ConstructionError::Parse)
        );
    }

    #[test]
    fn syntax_construction_literal_growth_and_drop_replacements_are_not_final_hir_only() {
        let short = conditional_certificate("a", &fake_facts()).unwrap();
        let long = conditional_certificate(&"a".repeat(65), &fake_facts()).unwrap();
        assert!(long.ast_peak_bytes > short.ast_peak_bytes);
        assert!(long.hir_construction_peak_bytes > short.hir_construction_peak_bytes);
        assert!(long.drop_scratch_bytes > short.drop_scratch_bytes);
        assert!(long.work_upper_bound > short.work_upper_bound);
        assert!(
            long.peak_requested_bytes >= long.ast_peak_bytes + long.hir_construction_peak_bytes
        );
    }

    #[test]
    fn syntax_construction_source_digest_checks_refuse_changed_bytes() {
        use sha2::{Digest as _, Sha256};
        let bytes = b"fake_reviewed_source";
        let expected = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(
            super::verify_source_bytes("fake_source.rs", &expected, bytes),
            Ok(())
        );
        assert_eq!(
            super::verify_source_bytes("fake_source.rs", &expected, b"fake_changed_source"),
            Err(ConstructionError::Source("fake_source.rs"))
        );
        for (relative, pinned_expected) in super::SOURCE_PINS {
            assert_eq!(
                super::verify_source_bytes(relative, pinned_expected, b"fake_changed_source"),
                Err(ConstructionError::Source(relative))
            );
        }
    }

    #[test]
    fn syntax_construction_owner_ledger_funds_seed_growth_shrink_and_all_phase_totals() {
        let certificate = conditional_certificate("a", &fake_facts()).unwrap();
        for name in [
            "translator literal Vec",
            "translator literal initial copy",
            "translator literal boxed slice",
        ] {
            let owner = certificate
                .owners
                .iter()
                .find(|owner| owner.owner == name)
                .unwrap();
            assert_eq!(owner.instances, 1, "{name}");
            assert_eq!(owner.element_bytes, 1, "{name}");
            assert!(owner.retained_bytes > 0, "{name}");
        }
        let layout = super::HeapLayout::new(size_of::<usize>(), size_of::<usize>()).unwrap();
        let mut total = 0_usize;
        for owner in &certificate.owners {
            if owner.allocation == "append_vector" {
                let envelope = layout
                    .append_vector(owner.element_bytes, owner.high_water)
                    .unwrap();
                assert_eq!(
                    owner.retained_bytes,
                    envelope.retained_bytes * owner.instances
                );
                assert_eq!(
                    owner.old_growth_bytes,
                    envelope.old_growth_bytes * owner.instances
                );
            } else {
                assert_eq!(owner.allocation, "fixed_payload");
                assert_eq!(owner.retained_bytes, owner.element_bytes * owner.instances);
                assert_eq!(owner.old_growth_bytes, 0);
            }
            total += owner.retained_bytes + owner.old_growth_bytes;
        }
        assert_eq!(total, certificate.peak_requested_bytes);
        assert_eq!(
            certificate.work_terms.values().sum::<usize>(),
            certificate.work_upper_bound
        );
    }

    #[test]
    fn syntax_construction_refuses_non_selected_alternation_and_singleton_rewrites() {
        for (pattern, owner) in [
            ("ab|ac", "Hir::alternation non-selected rewrite"),
            ("[a]", "Hir::class singleton/empty rewrite"),
        ] {
            assert_eq!(
                conditional_certificate(pattern, &fake_facts()),
                Err(ConstructionError::UnboundOwner(owner))
            );
        }
    }
}

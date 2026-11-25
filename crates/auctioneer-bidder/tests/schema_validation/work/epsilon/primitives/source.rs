//! Fallible, finite AST recipes. Supplied captures are data, never authority.
//! Only literal outer documentation and optional trailing commas in supported
//! declaration lists are ignored. Executable attributes, signatures, impl headers
//! and bodies match independently of receipt hashes and independently of the
//! test oracles.

use super::{CapturedData, CompilerData, EQUALITY_COMPILER_COMMIT, Refusal, SourceUnit};
use crate::schema_validation::handoff::SourceInputs;
use crate::schema_validation::operations::source_syntax::ConstTraitImpl;
use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, TokenStream, TokenTree};
use quote::{ToTokens, quote};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::mem;
use std::path::Path;
use std::str::from_utf8;
use syn::punctuated::Punctuated;
use syn::visit_mut::{self, VisitMut};

struct Sources(BTreeMap<SourceUnit, syn::File>);

fn const_header(implementation: &mut ConstTraitImpl, unit: SourceUnit) -> Result<String, Refusal> {
    attributes(implementation.attrs_mut(), unit)?;
    let attrs = implementation.attrs();
    let header = implementation.header_tokens();
    Ok(spelling(&quote!(#(#attrs)* #header)))
}

const PATHS: [(SourceUnit, &str, bool); 12] = [
    (
        SourceUnit::Nfa,
        "vendor/regex-automata/src/nfa/thompson/nfa.rs",
        false,
    ),
    (
        SourceUnit::Sparse,
        "vendor/regex-automata/src/util/sparse_set.rs",
        false,
    ),
    (
        SourceUnit::Ids,
        "vendor/regex-automata/src/util/primitives.rs",
        false,
    ),
    (SourceUnit::Arc, "alloc/src/sync.rs", true),
    (SourceUnit::Vec, "alloc/src/vec/mod.rs", true),
    (SourceUnit::RawVec, "alloc/src/raw_vec/mod.rs", true),
    (SourceUnit::SliceIndex, "core/src/slice/index.rs", true),
    (SourceUnit::NonNull, "core/src/ptr/non_null.rs", true),
    (SourceUnit::Unique, "core/src/ptr/unique.rs", true),
    (SourceUnit::MutPtr, "core/src/ptr/mut_ptr.rs", true),
    (SourceUnit::Intrinsics, "core/src/intrinsics/mod.rs", true),
    (SourceUnit::Macros, "core/src/macros/mod.rs", true),
];

fn supported_compiler(executable: &Path, verbose: &str) -> bool {
    let commits: Vec<&str> = verbose
        .lines()
        .filter_map(|line| line.strip_prefix("commit-hash: "))
        .collect();
    commits == [EQUALITY_COMPILER_COMMIT]
        && executable.file_name().is_some_and(|name| name == "rustc")
        && executable.is_absolute()
}

fn capture(data: &CapturedData, expected: &CompilerData) -> Result<Sources, Refusal> {
    if data.compiler != *expected
        || !supported_compiler(&expected.executable, &expected.verbose)
        || !data.workspace.is_absolute()
    {
        return Err(Refusal::CompilerMismatch);
    }
    let root = expected
        .executable
        .parent()
        .and_then(Path::parent)
        .ok_or(Refusal::CompilerMismatch)?;
    let library = root.join("lib/rustlib/src/rust/library");
    let mut files = BTreeMap::new();
    for (unit, relative, standard) in PATHS {
        let mut records = data.sources.iter().filter(|source| source.unit == unit);
        let record = records.next().ok_or(Refusal::MissingSource(unit))?;
        if records.next().is_some() {
            return Err(Refusal::DuplicateSource(unit));
        }
        let path = if standard {
            library.join(relative)
        } else {
            data.workspace.join(relative)
        };
        if record.path != path {
            return Err(Refusal::SourcePath(unit));
        }
        if record.before != record.after
            || record.sha256 != hex::encode(Sha256::digest(&record.before))
        {
            return Err(Refusal::SourceChanged(unit));
        }
        files.insert(unit, parse_source(unit, &record.before)?);
    }
    Ok(Sources(files))
}

fn parse_source(unit: SourceUnit, bytes: &[u8]) -> Result<syn::File, Refusal> {
    let source = from_utf8(bytes).map_err(|_error| Refusal::UnsupportedBody(unit))?;
    let file = syn::parse_file(source).map_err(|_error| Refusal::UnsupportedBody(unit))?;
    root_attributes(&file, unit)?;
    Ok(file)
}

// Borrow the bytes already consumed by the real fixed-roster window. This
// performs no source I/O and does not use CapturedData's unrelated cost facts.
pub(super) fn check_consumed_setup(inputs: &SourceInputs<'_>) -> Result<(), Refusal> {
    let (executable, verbose) = inputs.compiler();
    if !supported_compiler(executable, verbose) {
        return Err(Refusal::CompilerMismatch);
    }
    if inputs.source_count() != PATHS.len() {
        return Err(Refusal::InvalidFacts("source_roster"));
    }
    let mut files = BTreeMap::new();
    for (unit, _, _) in PATHS {
        let bytes = inputs
            .source(&format!("{unit:?}"))
            .ok_or(Refusal::MissingSource(unit))?;
        files.insert(unit, parse_source(unit, bytes)?);
    }
    check_recipes(&Sources(files), |_name| {})
}

fn spelling(value: &impl ToTokens) -> String {
    value.to_token_stream().to_string()
}

fn root_attributes(file: &syn::File, unit: SourceUnit) -> Result<(), Refusal> {
    let actual: Vec<&syn::Attribute> = file.attrs.iter().filter(|attribute| {
        !matches!(&attribute.meta, syn::Meta::NameValue(meta)
            if attribute.path().is_ident("doc")
            && matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_))))
    }).collect();
    let expected = match unit {
        SourceUnit::Arc | SourceUnit::Vec => quote!(#![stable(feature = "rust1", since = "1.0.0")]),
        SourceUnit::RawVec => quote!(
            #![unstable(feature = "raw_vec_internals", reason = "unstable const warnings", issue = "none")]
            #![cfg_attr(test, allow(dead_code))]
        ),
        SourceUnit::Intrinsics => quote!(
            #![unstable(
                feature = "core_intrinsics",
                reason = "intrinsics are unlikely to ever be stabilized, instead \
                      they should be used through stabilized interfaces \
                      in the rest of the standard library",
                issue = "none"
            )]
        ),
        SourceUnit::Nfa
        | SourceUnit::Sparse
        | SourceUnit::Ids
        | SourceUnit::SliceIndex
        | SourceUnit::NonNull
        | SourceUnit::Unique
        | SourceUnit::MutPtr
        | SourceUnit::Macros => TokenStream::new(),
    };
    if file.shebang.is_some() || spelling(&quote!(#(#actual)*)) != spelling(&expected) {
        return Err(Refusal::UnsupportedBody(unit));
    }
    Ok(())
}

fn attributes(attributes: &mut Vec<syn::Attribute>, unit: SourceUnit) -> Result<(), Refusal> {
    attributes.retain(|attribute| {
        !matches!(&attribute.meta, syn::Meta::NameValue(meta)
        if attribute.path().is_ident("doc") && matches!(attribute.style, syn::AttrStyle::Outer)
        && matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_))))
    });
    for attribute in attributes {
        if attribute.path().is_ident("derive") {
            let paths = attribute
                .parse_args_with(Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
                .map_err(|_error| Refusal::UnsupportedBody(unit))?;
            let normalized: Punctuated<syn::Path, syn::Token![,]> = paths.into_iter().collect();
            let syn::Meta::List(list) = &mut attribute.meta else {
                return Err(Refusal::UnsupportedBody(unit));
            };
            list.tokens = normalized.to_token_stream();
        }
    }
    Ok(())
}

impl Sources {
    fn file(&self, unit: SourceUnit) -> Result<&syn::File, Refusal> {
        self.0.get(&unit).ok_or(Refusal::MissingSource(unit))
    }

    fn structure(&self, unit: SourceUnit, mut expected: syn::ItemStruct) -> Result<(), Refusal> {
        let mut declarations = self.file(unit)?.items.iter().filter_map(|item| {
            let syn::Item::Struct(declaration) = item else {
                return None;
            };
            (declaration.ident == expected.ident).then_some(declaration)
        });
        let mut actual = declarations
            .next()
            .cloned()
            .ok_or(Refusal::UnsupportedBody(unit))?;
        if declarations.next().is_some() {
            return Err(Refusal::UnsupportedBody(unit));
        }
        if let (syn::Fields::Named(found), syn::Fields::Named(wanted)) =
            (&mut actual.fields, &expected.fields)
        {
            found.named = mem::take(&mut found.named)
                .into_iter()
                .filter(|field| {
                    wanted
                        .named
                        .iter()
                        .any(|required| required.ident == field.ident)
                })
                .collect();
        }
        for declaration in [&mut actual, &mut expected] {
            attributes(&mut declaration.attrs, unit)?;
            for field in &mut declaration.fields {
                attributes(&mut field.attrs, unit)?;
            }
            declaration.generics.params = mem::take(&mut declaration.generics.params)
                .into_iter()
                .collect();
            match &mut declaration.fields {
                syn::Fields::Named(fields) => {
                    fields.named = mem::take(&mut fields.named).into_iter().collect();
                }
                syn::Fields::Unnamed(fields) => {
                    fields.unnamed = mem::take(&mut fields.unnamed).into_iter().collect();
                }
                syn::Fields::Unit => {}
            }
        }
        if spelling(&actual) != spelling(&expected) {
            return Err(Refusal::UnsupportedBody(unit));
        }
        Ok(())
    }

    fn implementation(&self, unit: SourceUnit, mut expected: syn::ItemImpl) -> Result<(), Refusal> {
        let methods = mem::take(&mut expected.items);
        attributes(&mut expected.attrs, unit)?;
        for expected_item in methods {
            let syn::ImplItem::Fn(mut wanted) = expected_item else {
                return Err(Refusal::UnsupportedBody(unit));
            };
            attributes(&mut wanted.attrs, unit)?;
            let mut found = Vec::new();
            for item in &self.file(unit)?.items {
                let syn::Item::Impl(implementation) = item else {
                    continue;
                };
                if spelling(&implementation.self_ty) != spelling(&expected.self_ty) {
                    continue;
                }
                for member in &implementation.items {
                    let syn::ImplItem::Fn(method) = member else {
                        continue;
                    };
                    if method.sig.ident == wanted.sig.ident {
                        found.push((implementation, method));
                    }
                }
            }
            let [(implementation, method)] = found.as_slice() else {
                return Err(Refusal::UnsupportedBody(unit));
            };
            let mut header = (*implementation).clone();
            header.items.clear();
            attributes(&mut header.attrs, unit)?;
            let mut body = (*method).clone();
            attributes(&mut body.attrs, unit)?;
            if spelling(&header) != spelling(&expected) || spelling(&body) != spelling(&wanted) {
                return Err(unknown_call(unit, &body).unwrap_or(Refusal::UnsupportedBody(unit)));
            }
        }
        Ok(())
    }

    fn import(&self, unit: SourceUnit, name: &str, expected: &syn::Path) -> Result<(), Refusal> {
        let mut matches = Vec::new();
        for item in &self.file(unit)?.items {
            let syn::Item::Use(import) = item else {
                continue;
            };
            let mut bindings = Vec::new();
            import_leaves(&import.tree, Vec::new(), &mut bindings);
            for (local, path) in bindings {
                if local != name {
                    continue;
                }
                let expected_attrs = if unit == SourceUnit::Ids {
                    quote!(#[cfg(feature = "alloc")])
                } else {
                    TokenStream::new()
                };
                let attrs = &import.attrs;
                if spelling(&quote!(#(#attrs)*)) != spelling(&expected_attrs)
                    || import.leading_colon.is_some()
                    || !matches!(import.vis, syn::Visibility::Inherited)
                {
                    return Err(Refusal::UnsupportedBody(unit));
                }
                matches.push(path);
            }
        }
        if matches != [spelling(expected)] {
            return Err(Refusal::UnsupportedBody(unit));
        }
        Ok(())
    }

    fn constant_implementation(
        &self,
        unit: SourceUnit,
        expected: TokenStream,
    ) -> Result<(), Refusal> {
        let mut recipe = syn::parse2::<ConstTraitImpl>(expected)
            .map_err(|_error| Refusal::UnsupportedBody(unit))?;
        let header = const_header(&mut recipe, unit)?;
        let mut found = Vec::new();
        for item in &self.file(unit)?.items {
            let syn::Item::Verbatim(tokens) = item else {
                continue;
            };
            let Ok(mut candidate) = syn::parse2::<ConstTraitImpl>(tokens.clone()) else {
                continue;
            };
            if const_header(&mut candidate, unit)? == header {
                found.push(candidate.into_items());
            }
        }
        let [actual] = found.as_slice() else {
            return Err(Refusal::UnsupportedBody(unit));
        };
        constant_members(unit, actual, recipe.into_items())
    }
}

fn constant_member_name(item: &syn::ImplItem) -> Option<&Ident> {
    if let syn::ImplItem::Fn(method) = item {
        return Some(&method.sig.ident);
    }
    if let syn::ImplItem::Type(declaration) = item {
        return Some(&declaration.ident);
    }
    None
}

fn constant_member_attributes(item: &mut syn::ImplItem, unit: SourceUnit) -> Result<(), Refusal> {
    if let syn::ImplItem::Fn(method) = item {
        return attributes(&mut method.attrs, unit);
    }
    if let syn::ImplItem::Type(declaration) = item {
        return attributes(&mut declaration.attrs, unit);
    }
    Err(Refusal::UnsupportedBody(unit))
}

fn constant_members(
    unit: SourceUnit,
    actual: &[syn::ImplItem],
    expected: Vec<syn::ImplItem>,
) -> Result<(), Refusal> {
    for mut wanted in expected {
        let name = constant_member_name(&wanted)
            .ok_or(Refusal::UnsupportedBody(unit))?
            .clone();
        let mut matching = actual
            .iter()
            .filter(|member| constant_member_name(member) == Some(&name));
        let mut found = matching
            .next()
            .cloned()
            .ok_or(Refusal::UnsupportedBody(unit))?;
        if matching.next().is_some() {
            return Err(Refusal::UnsupportedBody(unit));
        }
        constant_member_attributes(&mut wanted, unit)?;
        constant_member_attributes(&mut found, unit)?;
        if spelling(&found) != spelling(&wanted) {
            if let syn::ImplItem::Fn(method) = &found {
                return Err(unknown_call(unit, method).unwrap_or(Refusal::UnsupportedBody(unit)));
            }
            return Err(Refusal::UnsupportedBody(unit));
        }
    }
    Ok(())
}

fn import_leaves(tree: &syn::UseTree, mut prefix: Vec<Ident>, result: &mut Vec<(String, String)>) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.clone());
            import_leaves(&path.tree, prefix, result);
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                import_leaves(item, prefix.clone(), result);
            }
        }
        syn::UseTree::Name(item) => {
            if item.ident != "self" {
                prefix.push(item.ident.clone());
            }
            if let Some(local) = prefix.last() {
                result.push((local.to_string(), spelling(&quote!(#(#prefix)::*))));
            }
        }
        syn::UseTree::Rename(item) => {
            if item.ident != "self" {
                prefix.push(item.ident.clone());
            }
            result.push((item.rename.to_string(), spelling(&quote!(#(#prefix)::*))));
        }
        syn::UseTree::Glob(_) => {}
    }
}

// This is diagnostic classification only, never an accepting whitelist. The
// entire supported body/signature/header must still match above.
fn unknown_call(unit: SourceUnit, method: &syn::ImplItemFn) -> Option<Refusal> {
    struct Calls {
        name: Option<String>,
    }
    impl VisitMut for Calls {
        fn visit_expr_call_mut(&mut self, i: &mut syn::ExprCall) {
            if let syn::Expr::Path(path) = i.func.as_ref()
                && let Some(segment) = path.path.segments.last()
            {
                let name = segment.ident.to_string();
                if ![
                    "new_unchecked",
                    "SmallIndex",
                    "StateID",
                    "transmute",
                    "aggregate_raw_ptr",
                    "assume",
                    "index",
                    "index_mut",
                ]
                .contains(&name.as_str())
                {
                    self.name.get_or_insert(name);
                }
            }
            visit_mut::visit_expr_call_mut(self, i);
        }
    }
    let mut calls = Calls { name: None };
    calls.visit_impl_item_fn_mut(&mut method.clone());
    calls
        .name
        .map(|callee| Refusal::UnknownCall { unit, callee })
}

fn vendor_declarations(sources: &Sources) -> Result<(), Refusal> {
    use SourceUnit::{Ids, Nfa, Sparse};
    sources.structure(
        Nfa,
        syn::parse_quote!(
            #[derive(Clone)]
            pub struct NFA(Arc<Inner>);
        ),
    )?;
    sources.structure(
        Nfa,
        syn::parse_quote!(
            #[derive(Default)]
            pub(super) struct Inner {
                states: Vec<State>,
            }
        ),
    )?;
    sources.structure(
        Sparse,
        syn::parse_quote!(
            #[derive(Clone)]
            pub(crate) struct SparseSet {
                len: usize,
                dense: Vec<StateID>,
                sparse: Vec<StateID>,
            }
        ),
    )?;
    sources.structure(
        Ids,
        syn::parse_quote!(
            #[derive(Clone, Copy, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
            #[repr(transparent)]
            pub struct StateID(SmallIndex);
        ),
    )?;
    sources.structure(
        Ids,
        syn::parse_quote!(
            #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
            #[repr(transparent)]
            pub struct SmallIndex(u32);
        ),
    )?;
    for item in &sources.file(Ids)?.items {
        let syn::Item::Impl(implementation) = item else {
            continue;
        };
        if ["StateID", "SmallIndex"].contains(&spelling(&implementation.self_ty).as_str())
            && implementation.trait_.as_ref().is_some_and(|(path, _)| {
                path.segments
                    .last()
                    .is_some_and(|segment| segment.ident == "PartialEq")
            })
        {
            return Err(Refusal::UnsupportedBody(Ids));
        }
    }
    Ok(())
}

fn vendor_methods(sources: &Sources) -> Result<(), Refusal> {
    sparse_macro_scope(sources)?;
    sources.implementation(
        SourceUnit::Nfa,
        syn::parse_quote!(impl NFA {
            #[inline] pub fn state(&self, id: StateID) -> &State { &self.states()[id] }
            #[inline] pub fn states(&self) -> &[State] { &self.0.states }
        }),
    )?;
    sources.implementation(
        SourceUnit::Nfa,
        syn::parse_quote!(impl State {
            #[inline] pub fn is_epsilon(&self) -> bool {
                match *self {
                    State::ByteRange { .. } | State::Sparse { .. } | State::Dense { .. }
                    | State::Fail | State::Match { .. } => false,
                    State::Look { .. } | State::Union { .. } | State::BinaryUnion { .. }
                    | State::Capture { .. } => true,
                }
            }
        }),
    )?;
    sources.implementation(SourceUnit::Sparse, syn::parse_quote!(impl SparseSet {
        #[cfg_attr(feature = "perf-inline", inline(always))]
        pub(crate) fn insert(&mut self, id: StateID) -> bool {
            if self.contains(id) { return false; }
            let i = self.len();
            assert!(i < self.capacity(), "{:?} exceeds capacity of {:?} when inserting {:?}", i, self.capacity(), id,);
            let index = StateID::new_unchecked(i);
            self.dense[index] = id;
            self.sparse[id] = index;
            self.len += 1;
            true
        }
        #[inline] pub(crate) fn contains(&self, id: StateID) -> bool {
            let index = self.sparse[id];
            index.as_usize() < self.len() && self.dense[index] == id
        }
        #[inline] pub(crate) fn len(&self) -> usize { self.len }
        #[inline] pub(crate) fn capacity(&self) -> usize { self.dense.len() }
    }))?;
    sources.implementation(SourceUnit::Ids, syn::parse_quote!(impl SmallIndex {
        #[inline] pub const fn new_unchecked(index: usize) -> SmallIndex { SmallIndex(index as u32) }
        #[inline] pub const fn as_usize(&self) -> usize { self.0 as usize }
    }))?;
    state_variants(sources)
}

fn sparse_macro_scope(sources: &Sources) -> Result<(), Refusal> {
    // These local gates do not resolve macros inherited from a parent module,
    // crate root or prelude. Actual use requires the source owner's independent
    // builtin-assert binding premise across those scopes; DATA is not a witness.
    let unit = SourceUnit::Sparse;
    let mut imports = Vec::new();
    for item in &sources.file(unit)?.items {
        if matches!(
            item,
            syn::Item::Macro(_)
                | syn::Item::Mod(_)
                | syn::Item::ExternCrate(_)
                | syn::Item::Verbatim(_)
        ) {
            return Err(Refusal::UnsupportedBody(unit));
        }
        if let syn::Item::Use(import) = item {
            imports.push(spelling(import));
        }
    }
    let expected: syn::File = syn::parse_quote!(
        use alloc::{vec, vec::Vec};
        use crate::util::primitives::StateID;
    );
    let mut wanted: Vec<String> = expected.items.iter().map(spelling).collect();
    imports.sort();
    wanted.sort();
    if imports != wanted {
        return Err(Refusal::UnsupportedBody(unit));
    }
    Ok(())
}

fn state_variants(sources: &Sources) -> Result<(), Refusal> {
    let unit = SourceUnit::Nfa;
    let mut declarations = sources.file(unit)?.items.iter().filter_map(|item| {
        let syn::Item::Enum(declaration) = item else {
            return None;
        };
        (declaration.ident == "State").then_some(declaration)
    });
    let actual = declarations.next().ok_or(Refusal::UnsupportedBody(unit))?;
    if declarations.next().is_some() {
        return Err(Refusal::UnsupportedBody(unit));
    }
    let mut normalized = actual.clone();
    let mut expected: syn::ItemEnum = syn::parse_quote!(
        #[derive(Clone, Eq, PartialEq)]
        pub enum State {
            ByteRange {
                trans: Transition,
            },
            Sparse(SparseTransitions),
            Dense(DenseTransitions),
            Look {
                look: Look,
                next: StateID,
            },
            Union {
                alternates: Box<[StateID]>,
            },
            BinaryUnion {
                alt1: StateID,
                alt2: StateID,
            },
            Capture {
                next: StateID,
                pattern_id: PatternID,
                group_index: SmallIndex,
                slot: SmallIndex,
            },
            Fail,
            Match {
                pattern_id: PatternID,
            },
        }
    );
    for declaration in [&mut normalized, &mut expected] {
        attributes(&mut declaration.attrs, unit)?;
        for variant in &mut declaration.variants {
            attributes(&mut variant.attrs, unit)?;
            for field in &mut variant.fields {
                attributes(&mut field.attrs, unit)?;
            }
            if let syn::Fields::Named(fields) = &mut variant.fields {
                fields.named = mem::take(&mut fields.named).into_iter().collect();
            }
        }
        declaration.variants = mem::take(&mut declaration.variants).into_iter().collect();
    }
    if spelling(&normalized) != spelling(&expected) {
        return Err(Refusal::UnsupportedBody(unit));
    }
    Ok(())
}

fn imports(sources: &Sources) -> Result<(), Refusal> {
    use SourceUnit::{Arc, Ids, Nfa, NonNull, RawVec, Sparse, Unique, Vec as VecUnit};
    for (unit, name, path) in [
        (Nfa, "Arc", syn::parse_quote!(alloc::sync::Arc)),
        (Nfa, "Vec", syn::parse_quote!(alloc::vec::Vec)),
        (
            Nfa,
            "StateID",
            syn::parse_quote!(crate::util::primitives::StateID),
        ),
        (Sparse, "Vec", syn::parse_quote!(alloc::vec::Vec)),
        (
            Sparse,
            "StateID",
            syn::parse_quote!(crate::util::primitives::StateID),
        ),
        (Ids, "Vec", syn::parse_quote!(alloc::vec::Vec)),
        (Arc, "NonNull", syn::parse_quote!(core::ptr::NonNull)),
        (VecUnit, "RawVec", syn::parse_quote!(crate::raw_vec::RawVec)),
        (VecUnit, "Index", syn::parse_quote!(core::ops::Index)),
        (VecUnit, "IndexMut", syn::parse_quote!(core::ops::IndexMut)),
        (
            VecUnit,
            "SliceIndex",
            syn::parse_quote!(core::slice::SliceIndex),
        ),
        (VecUnit, "intrinsics", syn::parse_quote!(core::intrinsics)),
        (VecUnit, "ops", syn::parse_quote!(core::ops)),
        (
            VecUnit,
            "SizedTypeProperties",
            syn::parse_quote!(core::mem::SizedTypeProperties),
        ),
        (Arc, "Deref", syn::parse_quote!(core::ops::Deref)),
        (SourceUnit::SliceIndex, "ops", syn::parse_quote!(crate::ops)),
        (RawVec, "Unique", syn::parse_quote!(core::ptr::Unique)),
        (RawVec, "NonNull", syn::parse_quote!(core::ptr::NonNull)),
        (Unique, "NonNull", syn::parse_quote!(crate::ptr::NonNull)),
        (
            Unique,
            "PhantomData",
            syn::parse_quote!(crate::marker::PhantomData),
        ),
        (
            NonNull,
            "transmute",
            syn::parse_quote!(crate::mem::transmute),
        ),
        (NonNull, "mem", syn::parse_quote!(crate::mem)),
    ] {
        sources.import(unit, name, &path)?;
    }
    allocator_imports(sources)
}

fn allocator_imports(sources: &Sources) -> Result<(), Refusal> {
    for unit in [SourceUnit::Arc, SourceUnit::Vec, SourceUnit::RawVec] {
        sources.import(
            unit,
            "Allocator",
            &syn::parse_quote!(crate::alloc::Allocator),
        )?;
        sources.import(unit, "Global", &syn::parse_quote!(crate::alloc::Global))?;
    }
    for unit in [SourceUnit::Unique, SourceUnit::NonNull, SourceUnit::MutPtr] {
        sources.import(
            unit,
            "PointeeSized",
            &syn::parse_quote!(crate::marker::PointeeSized),
        )?;
    }
    sources.import(
        SourceUnit::Intrinsics,
        "ptr",
        &syn::parse_quote!(crate::ptr),
    )
}

fn template_items(sources: &Sources) -> Result<Vec<TokenStream>, Refusal> {
    let unit = SourceUnit::Ids;
    let mut definitions = sources.file(unit)?.items.iter().filter_map(|item| {
        let syn::Item::Macro(definition) = item else {
            return None;
        };
        definition
            .ident
            .as_ref()
            .is_some_and(|name| name == "index_type_impls")
            .then_some(definition)
    });
    let definition = definitions.next().ok_or(Refusal::UnsupportedBody(unit))?;
    if definitions.next().is_some()
        || !definition.mac.path.is_ident("macro_rules")
        || !definition.attrs.is_empty()
    {
        return Err(Refusal::UnsupportedBody(unit));
    }
    let tokens: Vec<TokenTree> = definition.mac.tokens.clone().into_iter().collect();
    let [
        TokenTree::Group(parameters),
        TokenTree::Punct(equal),
        TokenTree::Punct(arrow),
        TokenTree::Group(body),
        TokenTree::Punct(semi),
    ] = tokens.as_slice()
    else {
        return Err(Refusal::UnsupportedBody(unit));
    };
    if (
        parameters.delimiter(),
        equal.as_char(),
        arrow.as_char(),
        body.delimiter(),
        semi.as_char(),
    ) != (Delimiter::Parenthesis, '=', '>', Delimiter::Brace, ';')
        || spelling(&parameters.stream())
            != spelling(&quote!($name:ident, $err:ident, $iter:ident, $withiter:ident))
    {
        return Err(Refusal::UnsupportedBody(unit));
    }
    let mut items = Vec::new();
    let mut current = TokenStream::new();
    for token in body.stream() {
        let boundary = matches!(&token, TokenTree::Group(group) if group.delimiter() == Delimiter::Brace)
            || matches!(&token, TokenTree::Punct(punct) if punct.as_char() == ';');
        current.extend([token]);
        if boundary {
            items.push(mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        return Err(Refusal::UnsupportedBody(unit));
    }
    for expected in [
        quote!(index_type_impls!(PatternID, PatternIDError, PatternIDIter, WithPatternIDIter);),
        quote!(index_type_impls!(StateID, StateIDError, StateIDIter, WithStateIDIter);),
    ] {
        if sources
            .file(unit)?
            .items
            .iter()
            .filter(|item| spelling(item) == spelling(&expected))
            .count()
            != 1
        {
            return Err(Refusal::UnsupportedBody(unit));
        }
    }
    Ok(items)
}

fn index_macro(sources: &Sources) -> Result<(), Refusal> {
    let items = template_items(sources)?;
    // Rust's source lexer joins '<' to the following '$'. Quote's literal '<'
    // does not; spell that one reviewed boundary explicitly, not normalization.
    let open = Punct::new('<', Spacing::Joint);
    for expected in [
        quote!(impl<T> core::ops::Index #open $name> for [T] {
            type Output = T;
            #[inline] fn index(&self, index: $name) -> &T { &self[index.as_usize()] }
        }),
        quote!(impl<T> core::ops::IndexMut #open $name> for [T] {
            #[inline] fn index_mut(&mut self, index: $name) -> &mut T { &mut self[index.as_usize()] }
        }),
        quote!(#[cfg(feature = "alloc")] impl<T> core::ops::Index #open $name> for Vec<T> {
            type Output = T;
            #[inline] fn index(&self, index: $name) -> &T { &self[index.as_usize()] }
        }),
        quote!(#[cfg(feature = "alloc")] impl<T> core::ops::IndexMut #open $name> for Vec<T> {
            #[inline] fn index_mut(&mut self, index: $name) -> &mut T { &mut self[index.as_usize()] }
        }),
    ] {
        if items
            .iter()
            .filter(|item| spelling(*item) == spelling(&expected))
            .count()
            != 1
        {
            return Err(Refusal::UnsupportedBody(SourceUnit::Ids));
        }
    }
    macro_id_methods(&items)
}

fn macro_id_methods(items: &[TokenStream]) -> Result<(), Refusal> {
    // Substitute only the four literal metavariables in a private copy so syn
    // can inspect the inherent impl. This is token substitution, not expansion
    // or generated Rust execution. Unsupported '$' uses fail closed.
    fn substitute(stream: TokenStream) -> Result<TokenStream, Refusal> {
        let mut tokens = stream.into_iter();
        let mut result = TokenStream::new();
        while let Some(token) = tokens.next() {
            match token {
                TokenTree::Punct(punct) if punct.as_char() == '$' => {
                    let Some(TokenTree::Ident(name)) = tokens.next() else {
                        return Err(Refusal::UnsupportedBody(SourceUnit::Ids));
                    };
                    let replacement = match name.to_string().as_str() {
                        "name" => quote!(StateID),
                        "err" => quote!(StateIDError),
                        "iter" => quote!(StateIDIter),
                        "withiter" => quote!(WithStateIDIter),
                        _ => return Err(Refusal::UnsupportedBody(SourceUnit::Ids)),
                    };
                    result.extend(replacement);
                }
                TokenTree::Group(group) => result.extend([TokenTree::Group(Group::new(
                    group.delimiter(),
                    substitute(group.stream())?,
                ))]),
                TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => {
                    result.extend([token]);
                }
            }
        }
        Ok(result)
    }
    let mut implementations = Vec::new();
    for item in items {
        // Other macro items include repetitions; only this exact literal
        // inherent impl is relevant. No unsupported item becomes a free call.
        let mut tokens = item.clone().into_iter();
        if !matches!(tokens.next(), Some(TokenTree::Ident(name)) if name == "impl")
            || !matches!(tokens.next(), Some(TokenTree::Punct(punct)) if punct.as_char() == '$')
            || !matches!(tokens.next(), Some(TokenTree::Ident(name)) if name == "name")
        {
            continue;
        }
        let implementation = syn::parse2::<syn::Item>(substitute(item.clone())?)
            .map_err(|_error| Refusal::UnsupportedBody(SourceUnit::Ids))?;
        implementations.push(implementation);
    }
    if implementations.len() != 1 {
        return Err(Refusal::UnsupportedBody(SourceUnit::Ids));
    }
    let file = syn::File {
        shebang: None,
        frontmatter: None,
        attrs: Vec::new(),
        items: implementations,
    };
    let sources = Sources(BTreeMap::from([(SourceUnit::Ids, file)]));
    sources.implementation(SourceUnit::Ids, syn::parse_quote!(impl StateID {
        #[inline] pub const fn new_unchecked(value: usize) -> StateID { StateID(SmallIndex::new_unchecked(value)) }
        #[inline] pub const fn as_usize(&self) -> usize { self.0.as_usize() }
    }))
}

fn standard_declarations(sources: &Sources) -> Result<(), Refusal> {
    sources.structure(SourceUnit::Arc, syn::parse_quote!(
        #[doc(search_unbox)]
        #[rustc_diagnostic_item = "Arc"]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_insignificant_dtor]
        #[diagnostic::on_move(
            message = "the type `{Self}` does not implement `Copy`",
            label = "this move could be avoided by cloning the original `{Self}`, which is inexpensive",
            note = "consider using `Arc::clone`"
        )]
        pub struct Arc<T: ?Sized, #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global> {
            ptr: NonNull<ArcInner<T>>,
        }
    ))?;
    sources.structure(
        SourceUnit::Arc,
        syn::parse_quote!(
            #[repr(C, align(2))]
            struct ArcInner<T: ?Sized> {
                data: T,
            }
        ),
    )?;
    sources.structure(
        SourceUnit::Vec,
        syn::parse_quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_diagnostic_item = "Vec"]
            #[rustc_insignificant_dtor]
            #[doc(alias = "list")]
            #[doc(alias = "vector")]
            pub struct Vec<
                T,
                #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
            > {
                buf: RawVec<T, A>,
                len: usize,
            }
        ),
    )?;
    sources.structure(
        SourceUnit::RawVec,
        syn::parse_quote!(
            #[allow(missing_debug_implementations)]
            pub(crate) struct RawVec<T, A: Allocator = Global> {
                inner: RawVecInner<A>,
            }
        ),
    )?;
    sources.structure(
        SourceUnit::RawVec,
        syn::parse_quote!(
            #[allow(missing_debug_implementations)]
            struct RawVecInner<A: Allocator = Global> {
                ptr: Unique<u8>,
            }
        ),
    )?;
    pointer_declarations(sources)
}

fn pointer_declarations(sources: &Sources) -> Result<(), Refusal> {
    sources.structure(
        SourceUnit::Unique,
        syn::parse_quote!(
            #[unstable(
                feature = "ptr_internals",
                issue = "none",
                reason = "use `NonNull` instead and consider `PhantomData<T>` \
              (if you also use `#[may_dangle]`), `Send`, and/or `Sync`"
            )]
            #[doc(hidden)]
            #[repr(transparent)]
            pub struct Unique<T: PointeeSized> {
                pointer: NonNull<T>,
                _marker: PhantomData<T>,
            }
        ),
    )?;
    sources.structure(
        SourceUnit::NonNull,
        syn::parse_quote!(
            #[stable(feature = "nonnull", since = "1.25.0")]
            #[repr(transparent)]
            #[rustc_nonnull_optimization_guaranteed]
            #[rustc_diagnostic_item = "NonNull"]
            pub struct NonNull<T: PointeeSized> {
                pointer: crate::pattern_type!(*const T is !null),
            }
        ),
    )
}

fn arc_methods(sources: &Sources) -> Result<(), Refusal> {
    sources.implementation(
        SourceUnit::Arc,
        syn::parse_quote!(
            impl<T: ?Sized, A: Allocator> Arc<T, A> {
                #[inline]
                fn inner(&self) -> &ArcInner<T> {
                    unsafe { self.ptr.as_ref() }
                }
            }
        ),
    )?;
    sources.implementation(
        SourceUnit::Arc,
        syn::parse_quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            impl<T: ?Sized, A: Allocator> Deref for Arc<T, A> {
                #[inline]
                fn deref(&self) -> &T {
                    &self.inner().data
                }
            }
        ),
    )
}

// Only the two authored Vec recipes use this constructor. Rebuild their
// expected lists after formatting; no actual method syntax is rewritten.
fn expected_slice_call(mut call: syn::ExprCall) -> Result<syn::ExprCall, Refusal> {
    let unit = SourceUnit::Vec;
    call.args = mem::take(&mut call.args).into_iter().collect();
    let syn::Expr::Path(function) = call.func.as_mut() else {
        return Err(Refusal::UnsupportedBody(unit));
    };
    let segment = function
        .path
        .segments
        .last_mut()
        .ok_or(Refusal::UnsupportedBody(unit))?;
    let syn::PathArguments::AngleBracketed(generics) = &mut segment.arguments else {
        return Err(Refusal::UnsupportedBody(unit));
    };
    generics.args = mem::take(&mut generics.args).into_iter().collect();
    Ok(call)
}

fn vector_methods(sources: &Sources) -> Result<(), Refusal> {
    let as_slice = expected_slice_call(syn::parse_quote!(core::intrinsics::aggregate_raw_ptr::<
        *const [T],
        _,
        _,
    >(self.as_ptr(), self.len)))?;
    let as_mut_slice =
        expected_slice_call(syn::parse_quote!(core::intrinsics::aggregate_raw_ptr::<
            *mut [T],
            _,
            _,
        >(self.as_mut_ptr(), self.len)))?;
    sources.implementation(
        SourceUnit::Vec,
        syn::parse_quote!(
            impl<T, A: Allocator> Vec<T, A> {
                #[inline]
                #[stable(feature = "vec_as_slice", since = "1.7.0")]
                #[rustc_diagnostic_item = "vec_as_slice"]
                #[rustc_const_stable(feature = "const_vec_string_slice", since = "1.87.0")]
                pub const fn as_slice(&self) -> &[T] {
                    unsafe {
                        &*#as_slice
                    }
                }
                #[inline]
                #[stable(feature = "vec_as_slice", since = "1.7.0")]
                #[rustc_diagnostic_item = "vec_as_mut_slice"]
                #[rustc_const_stable(feature = "const_vec_string_slice", since = "1.87.0")]
                pub const fn as_mut_slice(&mut self) -> &mut [T] {
                    unsafe {
                        &mut *#as_mut_slice
                    }
                }
                #[stable(feature = "vec_as_ptr", since = "1.37.0")]
                #[rustc_const_stable(feature = "const_vec_string_slice", since = "1.87.0")]
                #[rustc_never_returns_null_ptr]
                #[rustc_as_ptr]
                #[inline]
                pub const fn as_ptr(&self) -> *const T {
                    self.buf.ptr()
                }
                #[stable(feature = "vec_as_ptr", since = "1.37.0")]
                #[rustc_const_stable(feature = "const_vec_string_slice", since = "1.87.0")]
                #[rustc_never_returns_null_ptr]
                #[rustc_as_ptr]
                #[inline]
                pub const fn as_mut_ptr(&mut self) -> *mut T {
                    self.buf.ptr()
                }
                #[inline]
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_stable(feature = "const_vec_string_slice", since = "1.87.0")]
                #[rustc_confusables("length", "size")]
                pub const fn len(&self) -> usize {
                    let len = self.len;
                    unsafe { intrinsics::assume(len <= T::MAX_SLICE_LEN) };
                    len
                }
            }
        ),
    )?;
    vector_traits(sources)
}

fn vector_traits(sources: &Sources) -> Result<(), Refusal> {
    // Syn preserves these const bounds verbatim. Match the source lexer's
    // joint closing angle before the comma without rewriting actual bounds.
    let bound_close = Punct::new('>', Spacing::Joint);
    sources.constant_implementation(
        SourceUnit::Vec,
        quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
            const impl<T, A: Allocator> ops::Deref for Vec<T, A> {
                type Target = [T];
                #[inline]
                fn deref(&self) -> &[T] {
                    self.as_slice()
                }
            }
        ),
    )?;
    sources.constant_implementation(
        SourceUnit::Vec,
        quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
            const impl<T, A: Allocator> ops::DerefMut for Vec<T, A> {
                #[inline]
                fn deref_mut(&mut self) -> &mut [T] {
                    self.as_mut_slice()
                }
            }
        ),
    )?;
    sources.constant_implementation(
        SourceUnit::Vec,
        quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_index", issue = "143775")]
            const impl<T, I: [const] SliceIndex<[T]#bound_close, A: Allocator> Index<I> for Vec<T, A> {
                type Output = I::Output;
                #[inline]
                fn index(&self, index: I) -> &Self::Output {
                    Index::index(&**self, index)
                }
            }
        ),
    )?;
    sources.constant_implementation(
        SourceUnit::Vec,
        quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_index", issue = "143775")]
            const impl<T, I: [const] SliceIndex<[T]#bound_close, A: Allocator> IndexMut<I> for Vec<T, A> {
                #[inline]
                fn index_mut(&mut self, index: I) -> &mut Self::Output {
                    IndexMut::index_mut(&mut **self, index)
                }
            }
        ),
    )
}

fn pointer_methods(sources: &Sources) -> Result<(), Refusal> {
    let mut unique_cast: syn::ExprStruct = syn::parse_quote!(Unique {
        pointer: self.pointer.cast(),
        _marker: PhantomData,
    });
    unique_cast.fields = mem::take(&mut unique_cast.fields).into_iter().collect();
    sources.implementation(
        SourceUnit::RawVec,
        syn::parse_quote!(
            impl<T, A: Allocator> RawVec<T, A> {
                #[inline]
                pub(crate) const fn ptr(&self) -> *mut T {
                    self.inner.ptr()
                }
            }
        ),
    )?;
    sources.implementation(
        SourceUnit::RawVec,
        syn::parse_quote!(
            impl<A: Allocator> RawVecInner<A> {
                #[inline]
                const fn ptr<T>(&self) -> *mut T {
                    self.non_null::<T>().as_ptr()
                }
                #[inline]
                const fn non_null<T>(&self) -> NonNull<T> {
                    self.ptr.cast().as_non_null_ptr()
                }
            }
        ),
    )?;
    sources.implementation(
        SourceUnit::Unique,
        syn::parse_quote!(
            #[unstable(feature = "ptr_internals", issue = "none")]
            impl<T: PointeeSized> Unique<T> {
                #[must_use = "`self` will be dropped if the result is not used"]
                #[inline]
                pub const fn cast<U>(self) -> Unique<U> {
                    #unique_cast
                }
                #[must_use = "`self` will be dropped if the result is not used"]
                #[inline]
                pub const fn as_non_null_ptr(self) -> NonNull<T> {
                    self.pointer
                }
            }
        ),
    )?;
    nonnull_methods(sources)
}

fn nonnull_methods(sources: &Sources) -> Result<(), Refusal> {
    sources.implementation(
        SourceUnit::NonNull,
        syn::parse_quote!(
            impl<T: PointeeSized> NonNull<T> {
                #[stable(feature = "nonnull", since = "1.25.0")]
                #[rustc_const_stable(feature = "const_nonnull_as_ptr", since = "1.32.0")]
                #[rustc_never_returns_null_ptr]
                #[must_use]
                #[inline(always)]
                pub const fn as_ptr(self) -> *mut T {
                    unsafe { mem::transmute::<Self, *mut T>(self) }
                }
                #[stable(feature = "nonnull", since = "1.25.0")]
                #[rustc_const_stable(feature = "const_nonnull_as_ref", since = "1.73.0")]
                #[must_use]
                #[inline(always)]
                pub const unsafe fn as_ref<'a>(&self) -> &'a T {
                    unsafe { &*self.as_ptr().cast_const() }
                }
                #[stable(feature = "nonnull_cast", since = "1.27.0")]
                #[rustc_const_stable(feature = "const_nonnull_cast", since = "1.36.0")]
                #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
                #[inline]
                pub const fn cast<U>(self) -> NonNull<U> {
                    unsafe { transmute(self.as_ptr() as *mut U) }
                }
            }
        ),
    )?;
    sources.implementation(
        SourceUnit::MutPtr,
        syn::parse_quote!(
            impl<T: PointeeSized> *mut T {
                #[stable(feature = "ptr_const_cast", since = "1.65.0")]
                #[rustc_const_stable(feature = "ptr_const_cast", since = "1.65.0")]
                #[rustc_diagnostic_item = "ptr_cast_const"]
                #[inline(always)]
                pub const fn cast_const(self) -> *const T {
                    self as _
                }
            }
        ),
    )
}

fn slice_methods(sources: &Sources) -> Result<(), Refusal> {
    let bound_close = Punct::new('>', Spacing::Joint);
    sources.constant_implementation(
        SourceUnit::SliceIndex,
        quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_index", issue = "143775")]
            const impl<T, I> ops::Index<I> for [T]
            where
                I: [const] SliceIndex<[T]#bound_close,
            {
                type Output = I::Output;
                #[inline(always)]
                fn index(&self, index: I) -> &I::Output {
                    index.index(self)
                }
            }
        ),
    )?;
    sources.constant_implementation(
        SourceUnit::SliceIndex,
        quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_index", issue = "143775")]
            const impl<T, I> ops::IndexMut<I> for [T]
            where
                I: [const] SliceIndex<[T]#bound_close,
            {
                #[inline(always)]
                #[rustc_no_writable]
                fn index_mut(&mut self, index: I) -> &mut I::Output {
                    index.index_mut(self)
                }
            }
        ),
    )?;
    sources.constant_implementation(
        SourceUnit::SliceIndex,
        quote!(
            #[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
            #[rustc_const_unstable(feature = "const_index", issue = "143775")]
            const unsafe impl<T> SliceIndex<[T]> for usize {
                type Output = T;
                #[inline]
                fn index(self, slice: &[T]) -> &T {
                    &(*slice)[self]
                }
                #[inline]
                #[rustc_no_writable]
                fn index_mut(self, slice: &mut [T]) -> &mut T {
                    &mut (*slice)[self]
                }
            }
        ),
    )
}

fn intrinsic_declaration(
    sources: &Sources,
    mut expected: syn::ForeignItemFn,
) -> Result<(), Refusal> {
    let unit = SourceUnit::Intrinsics;
    attributes(&mut expected.attrs, unit)?;
    let mut found = Vec::new();
    for item in &sources.file(unit)?.items {
        let syn::Item::Verbatim(tokens) = item else {
            continue;
        };
        let Ok(mut declaration) = syn::parse2::<syn::ForeignItemFn>(tokens.clone()) else {
            continue;
        };
        if declaration.sig.ident != expected.sig.ident {
            continue;
        }
        attributes(&mut declaration.attrs, unit)?;
        found.push(declaration);
    }
    let [actual] = found.as_slice() else {
        return Err(Refusal::UnsupportedBody(unit));
    };
    if spelling(actual) != spelling(&expected) {
        return Err(Refusal::UnsupportedBody(unit));
    }
    Ok(())
}

fn intrinsic_boundaries(sources: &Sources) -> Result<(), Refusal> {
    // These are compiler intrinsics, not ordinary callees assigned zero work.
    // The call expression is charged at each site. Interpretation is conditional
    // on authentic compiler/library bindings, including mem's re-export; the
    // supplied compiler record and these ASTs do not establish that authority.
    // The actual-source owner must close those bindings before consumption.
    intrinsic_declaration(
        sources,
        syn::parse_quote!(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_allowed_through_unstable_modules = "import this function via `std::mem` instead"]
            #[rustc_const_stable(feature = "const_transmute", since = "1.56.0")]
            #[rustc_diagnostic_item = "transmute"]
            #[rustc_nounwind]
            #[rustc_intrinsic]
            pub const unsafe fn transmute<Src, Dst>(src: Src) -> Dst;
        ),
    )?;
    intrinsic_declaration(
        sources,
        syn::parse_quote!(
            #[rustc_nounwind]
            #[unstable(feature = "core_intrinsics", issue = "none")]
            #[rustc_intrinsic_const_stable_indirect]
            #[rustc_intrinsic]
            pub const fn aggregate_raw_ptr<P: bounds::BuiltinDeref, D, M>(data: D, meta: M) -> P
            where
                <P as bounds::BuiltinDeref>::Pointee: ptr::Pointee<Metadata = M>;
        ),
    )?;
    let expected: syn::ItemFn = syn::parse_quote!(
        #[rustc_intrinsic_const_stable_indirect]
        #[rustc_nounwind]
        #[unstable(feature = "core_intrinsics", issue = "none")]
        #[rustc_intrinsic]
        pub const unsafe fn assume(b: bool) {
            if !b {
                unsafe { unreachable() }
            }
        }
    );
    let mut found = Vec::new();
    for item in &sources.file(SourceUnit::Intrinsics)?.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        if function.sig.ident != "assume" {
            continue;
        }
        let mut actual = function.clone();
        attributes(&mut actual.attrs, SourceUnit::Intrinsics)?;
        found.push(actual);
    }
    let [actual] = found.as_slice() else {
        return Err(Refusal::UnsupportedBody(SourceUnit::Intrinsics));
    };
    if spelling(actual) != spelling(&expected) {
        return Err(Refusal::UnsupportedBody(SourceUnit::Intrinsics));
    }
    assert_macro(sources)
}

fn assert_macro(sources: &Sources) -> Result<(), Refusal> {
    fn collect(items: &[syn::Item], found: &mut Vec<syn::ItemMacro>) {
        for item in items {
            if let syn::Item::Macro(definition) = item
                && definition
                    .ident
                    .as_ref()
                    .is_some_and(|name| name == "assert")
            {
                found.push(definition.clone());
            }
            if let syn::Item::Mod(module) = item
                && let Some((_, children)) = &module.content
            {
                collect(children, found);
            }
        }
    }
    let unit = SourceUnit::Macros;
    let mut found = Vec::new();
    collect(&sources.file(unit)?.items, &mut found);
    let [actual] = found.as_mut_slice() else {
        return Err(Refusal::UnsupportedBody(unit));
    };
    attributes(&mut actual.attrs, unit)?;
    let expected: syn::ItemMacro = syn::parse_quote!(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_builtin_macro]
        #[macro_export]
        #[rustc_diagnostic_item = "assert_macro"]
        #[allow_internal_unstable(
            core_intrinsics,
            panic_internals,
            edition_panic,
            generic_assert_internals
        )]
        macro_rules! assert {
            ($cond:expr $(,)?) => {{}};
            ($cond:expr, $($arg:tt)+) => {{}};
        }
    );
    if spelling(actual) != spelling(&expected) {
        return Err(Refusal::UnsupportedBody(unit));
    }
    Ok(())
}

pub(super) fn check(data: &CapturedData, expected: &CompilerData) -> Result<(), Refusal> {
    check_with_stages(data, expected, |_name| {})
}

// Used only by an assertion's lazy failure message. Reusing the same data-only
// kernel avoids a second classifier, mutable global state or direct printing.
pub(super) fn failure_stage(data: &CapturedData, expected: &CompilerData) -> Option<&'static str> {
    let mut stage = None;
    let result = check_with_stages(data, expected, |name| stage = Some(name));
    if result.is_err() { stage } else { None }
}

fn check_with_stages(
    data: &CapturedData,
    expected: &CompilerData,
    mut enter: impl FnMut(&'static str),
) -> Result<(), Refusal> {
    enter("capture");
    let sources = capture(data, expected)?;
    check_recipes(&sources, enter)
}

fn check_recipes(sources: &Sources, mut enter: impl FnMut(&'static str)) -> Result<(), Refusal> {
    enter("vendor_declarations");
    vendor_declarations(sources)?;
    enter("imports");
    imports(sources)?;
    enter("vendor_methods");
    vendor_methods(sources)?;
    enter("id_limits");
    id_limits(sources)?;
    enter("index_macro");
    index_macro(sources)?;
    enter("standard_declarations");
    standard_declarations(sources)?;
    enter("arc_methods");
    arc_methods(sources)?;
    enter("vector_methods");
    vector_methods(sources)?;
    enter("pointer_methods");
    pointer_methods(sources)?;
    enter("slice_methods");
    slice_methods(sources)?;
    enter("intrinsic_boundaries");
    intrinsic_boundaries(sources)
}

fn id_limits(sources: &Sources) -> Result<(), Refusal> {
    let unit = SourceUnit::Ids;
    let mut expected: syn::ItemImpl = syn::parse_quote!(impl SmallIndex {
        #[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
        pub const MAX: SmallIndex = SmallIndex::new_unchecked(i32::MAX as usize - 1);
        #[cfg(target_pointer_width = "16")]
        pub const MAX: SmallIndex = SmallIndex::new_unchecked(isize::MAX - 1);
        pub const LIMIT: usize = SmallIndex::MAX.as_usize() + 1;
    });
    let mut wanted: Vec<String> = mem::take(&mut expected.items)
        .iter()
        .map(spelling)
        .collect();
    let mut found = Vec::new();
    for item in &sources.file(unit)?.items {
        let syn::Item::Impl(implementation) = item else {
            continue;
        };
        if spelling(&implementation.self_ty) != spelling(&expected.self_ty) {
            continue;
        }
        for member in &implementation.items {
            let syn::ImplItem::Const(constant) = member else {
                continue;
            };
            if constant.ident != "MAX" && constant.ident != "LIMIT" {
                continue;
            }
            let mut header = implementation.clone();
            header.items.clear();
            attributes(&mut header.attrs, unit)?;
            if spelling(&header) != spelling(&expected) {
                return Err(Refusal::UnsupportedBody(unit));
            }
            let mut actual = constant.clone();
            attributes(&mut actual.attrs, unit)?;
            found.push(spelling(&actual));
        }
    }
    wanted.sort();
    found.sort();
    if found != wanted {
        return Err(Refusal::UnsupportedBody(unit));
    }
    Ok(())
}

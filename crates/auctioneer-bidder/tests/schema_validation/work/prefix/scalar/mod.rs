//! Conditional concrete string/Counter semantics, not source or artifact authority.
//!
//! Source snapshots, roots, feature clauses, dispatch and pointer width are DATA.
//! Actual use still requires the existing registry authentication and bracketed
//! source-window owner, same-artifact library features and compiler/builtin
//! binding. Fixture equality does not establish any of those joins.
//! Only native `str` and `Value::{Null,Bool,Number}` calls with
//! `CompactFormatter` are in scope.
//! Raw bounds count strings/keys passing the precheck and reaching serialization,
//! including a final serialization whose byte charge fails, not rejected operands.
//! Emitted lengths are virtual Counter bytes, not copied/scanned work. Numeric
//! formatter safety/termination/work, map navigation, ordinary error/unwind
//! cleanup and matched frame facts remain missing. No generic Serialize claim
//! or new error-service exclusion follows from a concrete Counter schedule.

use crate::schema_validation::operations::source_syntax::{
    ConstTraitImpl, literal_doc, parse_file, tokens,
};
use crate::schema_validation::work::{AttemptContext, SchemaInputLimits};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use syn::parse::{Parse, ParseStream, Parser as _};
use syn::visit_mut::{self, VisitMut};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::schema_validation) enum SourceUnit {
    Native,
    Input,
    JsonSize,
    SerdeFacade,
    SerdeCoreRoot,
    SerdeCoreExports,
    SerdeTraits,
    SerdeStr,
    JsonRoot,
    JsonIo,
    JsonSerializer,
    JsonValue,
    JsonValueSerialize,
    JsonNumber,
    IoWrite,
    IoForward,
    StrBytes,
    Slice,
    SliceIndex,
    Default,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceData {
    unit: SourceUnit,
    path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Roots {
    workspace: PathBuf,
    registry: PathBuf,
    library: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum FeatureClause {
    SerdeStd,
    SerdeCoreStd,
    JsonStd,
    ArbitraryPrecision,
    Docsrs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FeatureData {
    serde_std: Option<bool>,
    serde_core_std: Option<bool>,
    json_std: Option<bool>,
    arbitrary_precision: Option<bool>,
    docsrs: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation::work) enum Dispatch {
    NativeScalars,
    GenericSerialize,
    OtherWriter,
    PrettyFormatter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation::work) struct ScalarData {
    roots: Roots,
    sources: Vec<SourceData>,
    features: FeatureData,
    pointer_width: Option<u16>,
    dispatch: Dispatch,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ScalarContract {
    pub(super) raw_utf8_with_quotes_upper: u64,
    string_virtual_bytes_upper: u64,
    pub(super) writes_per_nonempty_request_upper: u64,
    pub(super) writes_per_empty_request_upper: u64,
    pub(super) interrupted_retries_upper: u64,
    required_sources: BTreeSet<SourceUnit>,
    unresolved: Vec<&'static str>,
    library_artifact_binding: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Refusal {
    ContextSourceMismatch(SourceUnit),
    NativeLimitsMismatch,
    MissingSourceProof,
    MissingSource(SourceUnit),
    DuplicateSource(SourceUnit),
    SourcePath(SourceUnit),
    SourceChanged(SourceUnit),
    SourceShape(SourceUnit),
    MissingFeature(FeatureClause),
    FeatureMismatch(FeatureClause),
    MissingWidth,
    UnsupportedWidth(u16),
    UnsupportedDispatch,
    Overflow,
}

// The source tuple establishes conditional lengths/protocols only. Its
// identities still need the existing source window at actual use. No test
// fixture or expected-contract constructor is consumed by this checker.
fn derive_scalar_sources(
    data: &ScalarData,
    limits: SchemaInputLimits,
    attempts: u64,
) -> Result<ScalarContract, Refusal> {
    let sources = checked_sources(data)?;
    let extracted = extract_native_limits(&sources)?;
    if extracted != limits {
        return Err(Refusal::SourceShape(SourceUnit::Native));
    }
    derive_checked_sources(&sources, extracted, attempts)
}

fn checked_sources(data: &ScalarData) -> Result<Sources<'_>, Refusal> {
    check_tuple(data)?;
    Sources::check(data)
}

// This joins conditional DATA only. The actual-source window and artifact
// correspondence still belong to their existing owners.
pub(super) fn derive_prefix_sources(
    context: &AttemptContext<'_>,
    data: &ScalarData,
    attempts: u64,
) -> Result<ScalarContract, Refusal> {
    let sources = checked_sources(data)?;
    for (unit, expected) in [
        (SourceUnit::Native, context.sources.native),
        (SourceUnit::Input, context.sources.input),
        (SourceUnit::JsonSize, context.sources.json_size),
    ] {
        if sources.bytes(unit)? != expected {
            return Err(Refusal::ContextSourceMismatch(unit));
        }
    }
    let limits = extract_native_limits(&sources)?;
    if limits != context.native_input_limits {
        return Err(Refusal::NativeLimitsMismatch);
    }
    derive_checked_sources(&sources, limits, attempts)
}

fn derive_checked_sources(
    sources: &Sources<'_>,
    limits: SchemaInputLimits,
    attempts: u64,
) -> Result<ScalarContract, Refusal> {
    serde_sources(sources)?;
    json_sources(sources)?;
    primitive_sources(sources)?;
    let lengths = string_sources(sources)?;
    let writes = writer_sources(sources)?;
    let raw_utf8_with_quotes_upper = attempts
        .checked_mul(limits.serialized_bytes)
        .ok_or(Refusal::Overflow)?;
    let raw_string = limits
        .serialized_bytes
        .checked_sub(lengths.quotes)
        .ok_or(Refusal::Overflow)?;
    let string_virtual_bytes_upper = raw_string
        .checked_mul(lengths.escape)
        .and_then(|bytes| bytes.checked_add(lengths.quotes))
        .ok_or(Refusal::Overflow)?;
    // Matched Counter starts at zero and writes the whole offered slice.
    // String requests have the checked cumulative bound above. The matched
    // numeric methods offer one completed str slice; 32/64-bit usize fits u64.
    // This does not prove the numeric formatter's safety/termination or work.
    Ok(ScalarContract {
        raw_utf8_with_quotes_upper,
        string_virtual_bytes_upper,
        writes_per_nonempty_request_upper: writes,
        writes_per_empty_request_upper: 0,
        interrupted_retries_upper: 0,
        required_sources: sources.0.keys().copied().collect(),
        unresolved: vec![
            "actual registry/feature/artifact join",
            "compiler/builtin and primitive semantic identity",
            "itoa/zmij safety, termination and ordinary work",
            "serde/string/slice ordinary helper work",
            "caller-owned map height/navigation",
            "ordinary error/unwind cleanup",
            "matched helper frame bytes",
        ],
        library_artifact_binding: None,
    })
}

fn check_tuple(data: &ScalarData) -> Result<(), Refusal> {
    for (clause, value, wanted) in [
        (FeatureClause::SerdeStd, data.features.serde_std, true),
        (
            FeatureClause::SerdeCoreStd,
            data.features.serde_core_std,
            true,
        ),
        (FeatureClause::JsonStd, data.features.json_std, true),
        (
            FeatureClause::ArbitraryPrecision,
            data.features.arbitrary_precision,
            false,
        ),
        (FeatureClause::Docsrs, data.features.docsrs, false),
    ] {
        if value.ok_or(Refusal::MissingFeature(clause))? != wanted {
            return Err(Refusal::FeatureMismatch(clause));
        }
    }
    match data.pointer_width.ok_or(Refusal::MissingWidth)? {
        32 | 64 => {}
        width => return Err(Refusal::UnsupportedWidth(width)),
    }
    if data.dispatch != Dispatch::NativeScalars {
        return Err(Refusal::UnsupportedDispatch);
    }
    Ok(())
}

fn source_paths(roots: &Roots) -> [(SourceUnit, PathBuf); 20] {
    use SourceUnit::{
        Default, Input, IoForward, IoWrite, JsonIo, JsonNumber, JsonRoot, JsonSerializer, JsonSize,
        JsonValue, JsonValueSerialize, Native, SerdeCoreExports, SerdeCoreRoot, SerdeFacade,
        SerdeStr, SerdeTraits, Slice, SliceIndex, StrBytes,
    };
    [
        (
            Native,
            roots
                .workspace
                .join("crates/auctioneer-bidder/src/schema/native.rs"),
        ),
        (
            Input,
            roots
                .workspace
                .join("crates/auctioneer-bidder/src/schema/input.rs"),
        ),
        (
            JsonSize,
            roots
                .workspace
                .join("crates/auctioneer-runtime/src/json_size.rs"),
        ),
        (SerdeFacade, roots.registry.join("serde-1.0.229/src/lib.rs")),
        (
            SerdeCoreRoot,
            roots.registry.join("serde_core-1.0.229/src/lib.rs"),
        ),
        (
            SerdeCoreExports,
            roots.registry.join("serde_core-1.0.229/src/crate_root.rs"),
        ),
        (
            SerdeTraits,
            roots.registry.join("serde_core-1.0.229/src/ser/mod.rs"),
        ),
        (
            SerdeStr,
            roots.registry.join("serde_core-1.0.229/src/ser/impls.rs"),
        ),
        (
            JsonRoot,
            roots.registry.join("serde_json-1.0.150/src/lib.rs"),
        ),
        (
            JsonIo,
            roots.registry.join("serde_json-1.0.150/src/io/mod.rs"),
        ),
        (
            JsonSerializer,
            roots.registry.join("serde_json-1.0.150/src/ser.rs"),
        ),
        (
            JsonValue,
            roots.registry.join("serde_json-1.0.150/src/value/mod.rs"),
        ),
        (
            JsonValueSerialize,
            roots.registry.join("serde_json-1.0.150/src/value/ser.rs"),
        ),
        (
            JsonNumber,
            roots.registry.join("serde_json-1.0.150/src/number.rs"),
        ),
        (IoWrite, roots.library.join("std/src/io/mod.rs")),
        (IoForward, roots.library.join("std/src/io/impls.rs")),
        (StrBytes, roots.library.join("core/src/str/mod.rs")),
        (Slice, roots.library.join("core/src/slice/mod.rs")),
        (SliceIndex, roots.library.join("core/src/slice/index.rs")),
        (Default, roots.library.join("core/src/default.rs")),
    ]
}

struct Sources<'a>(BTreeMap<SourceUnit, syn::File>, &'a [SourceData]);

impl<'a> Sources<'a> {
    // Index only the supplied snapshots. This performs no filesystem read,
    // registry authentication, library capture or artifact/feature discovery.
    fn check(data: &'a ScalarData) -> Result<Self, Refusal> {
        let mut files = BTreeMap::new();
        for (unit, path) in source_paths(&data.roots) {
            let mut matching = data.sources.iter().filter(|source| source.unit == unit);
            let source = matching.next().ok_or(Refusal::MissingSource(unit))?;
            if matching.next().is_some() {
                return Err(Refusal::DuplicateSource(unit));
            }
            if !path.is_absolute() || source.path != path {
                return Err(Refusal::SourcePath(unit));
            }
            if source.before != source.after
                || source.sha256 != hex::encode(Sha256::digest(&source.before))
            {
                return Err(Refusal::SourceChanged(unit));
            }
            let file = parse_file(&source.before).map_err(|()| Refusal::SourceShape(unit))?;
            check_root_attributes(&file, unit)?;
            files.insert(unit, file);
        }
        Ok(Self(files, &data.sources))
    }

    fn file(&self, unit: SourceUnit) -> Result<&syn::File, Refusal> {
        self.0.get(&unit).ok_or(Refusal::MissingSource(unit))
    }

    fn bytes(&self, unit: SourceUnit) -> Result<&[u8], Refusal> {
        self.1
            .iter()
            .find(|source| source.unit == unit)
            .map(|source| source.before.as_slice())
            .ok_or(Refusal::MissingSource(unit))
    }

    fn exact(&self, unit: SourceUnit, supported: syn::Item) -> Result<syn::Item, Refusal> {
        let mut found = self.file(unit)?.items.iter().filter(|item| {
            item_identity(item) == item_identity(&supported)
                && item_identity(&supported).is_some()
                && item_configuration(item) == item_configuration(&supported)
        });
        let item = unique(&mut found, unit)?.clone();
        same(unit, &project(item.clone()), &project(supported))?;
        Ok(item)
    }

    fn function(&self, unit: SourceUnit, supported: syn::ItemFn) -> Result<syn::ItemFn, Refusal> {
        let syn::Item::Fn(actual) = self.exact(unit, syn::Item::Fn(supported))? else {
            return Err(Refusal::SourceShape(unit));
        };
        Ok(actual)
    }

    fn implementation(
        &self,
        unit: SourceUnit,
        supported: syn::ItemImpl,
    ) -> Result<Vec<syn::ImplItemFn>, Refusal> {
        let mut candidates = self.file(unit)?.items.iter().filter_map(|item| {
            if let syn::Item::Impl(implementation) = item
                && tokens(&implementation.self_ty) == tokens(&supported.self_ty)
                && trait_identity(implementation) == trait_identity(&supported)
            {
                Some(implementation)
            } else {
                None
            }
        });
        let actual = unique(&mut candidates, unit)?;
        if supported.items.is_empty() && !actual.items.is_empty() {
            return Err(Refusal::SourceShape(unit));
        }
        let mut header = actual.clone();
        header.items.clear();
        let mut expected_header = supported.clone();
        expected_header.items.clear();
        same(
            unit,
            &project(syn::Item::Impl(header)),
            &project(syn::Item::Impl(expected_header)),
        )?;
        let mut matched = Vec::new();
        for member in supported.items {
            if let syn::ImplItem::Type(expected) = &member {
                let mut types = actual.items.iter().filter_map(|item| {
                    if let syn::ImplItem::Type(associated) = item
                        && associated.ident == expected.ident
                    {
                        Some(associated)
                    } else {
                        None
                    }
                });
                let mut associated = unique(&mut types, unit)?.clone();
                associated
                    .attrs
                    .retain(|attribute| !supported_doc(attribute));
                same(unit, &associated, expected)?;
                continue;
            }
            let syn::ImplItem::Fn(expected) = member else {
                return Err(Refusal::SourceShape(unit));
            };
            let mut methods = actual.items.iter().filter_map(|item| {
                if let syn::ImplItem::Fn(method) = item
                    && method.sig.ident == expected.sig.ident
                    && configuration(&method.attrs) == configuration(&expected.attrs)
                {
                    Some(method)
                } else {
                    None
                }
            });
            let method = unique(&mut methods, unit)?;
            same(
                unit,
                &project_method(method.clone()),
                &project_method(expected),
            )?;
            matched.push(method.clone());
        }
        Ok(matched)
    }

    fn trait_methods(
        &self,
        unit: SourceUnit,
        supported: syn::ItemTrait,
    ) -> Result<Vec<syn::TraitItemFn>, Refusal> {
        let mut traits = self.file(unit)?.items.iter().filter_map(|item| {
            if let syn::Item::Trait(trait_) = item
                && trait_.ident == supported.ident
            {
                Some(trait_)
            } else {
                None
            }
        });
        let actual = unique(&mut traits, unit)?;
        let mut header = actual.clone();
        header.items.clear();
        let mut expected_header = supported.clone();
        expected_header.items.clear();
        same(
            unit,
            &project(syn::Item::Trait(header)),
            &project(syn::Item::Trait(expected_header)),
        )?;
        let mut matched = Vec::new();
        for member in supported.items {
            let syn::TraitItem::Fn(mut expected) = member else {
                return Err(Refusal::SourceShape(unit));
            };
            let mut methods = actual.items.iter().filter_map(|item| {
                if let syn::TraitItem::Fn(method) = item
                    && method.sig.ident == expected.sig.ident
                {
                    Some(method)
                } else {
                    None
                }
            });
            let mut method = unique(&mut methods, unit)?.clone();
            method.attrs.retain(|attribute| !supported_doc(attribute));
            expected.attrs.retain(|attribute| !supported_doc(attribute));
            signature_lists(&mut method.sig);
            signature_lists(&mut expected.sig);
            same(unit, &method, &expected)?;
            matched.push(method);
        }
        Ok(matched)
    }
}

fn unique<'a, T: 'a>(
    items: &mut impl Iterator<Item = &'a T>,
    unit: SourceUnit,
) -> Result<&'a T, Refusal> {
    let first = items.next().ok_or(Refusal::SourceShape(unit))?;
    if items.next().is_some() {
        return Err(Refusal::SourceShape(unit));
    }
    Ok(first)
}

fn trait_identity(implementation: &syn::ItemImpl) -> Option<String> {
    implementation.trait_.as_ref().map(|(path, _)| tokens(path))
}

fn configuration(attrs: &[syn::Attribute]) -> String {
    let selected = attrs.iter().filter(|attribute| {
        attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
    });
    tokens(&quote!(#(#selected)*))
}

fn item_configuration(item: &syn::Item) -> String {
    match item {
        syn::Item::Mod(value) => configuration(&value.attrs),
        syn::Item::Use(value) => configuration(&value.attrs),
        _ => String::new(),
    }
}

fn same(
    unit: SourceUnit,
    actual: &impl ToTokens,
    supported: &impl ToTokens,
) -> Result<(), Refusal> {
    if tokens(actual) == tokens(supported) {
        Ok(())
    } else {
        Err(Refusal::SourceShape(unit))
    }
}

fn supported_doc(attribute: &syn::Attribute) -> bool {
    literal_doc(attribute)
        && matches!(&attribute.meta, syn::Meta::NameValue(meta)
        if matches!(&meta.value, syn::Expr::Lit(value) if value.attrs.is_empty()))
}

fn item_identity(item: &syn::Item) -> Option<(u8, String)> {
    match item {
        syn::Item::Fn(value) => Some((0, value.sig.ident.to_string())),
        syn::Item::Struct(value) => Some((1, value.ident.to_string())),
        syn::Item::Enum(value) => Some((2, value.ident.to_string())),
        syn::Item::Const(value) => Some((3, value.ident.to_string())),
        syn::Item::Static(value) => Some((4, value.ident.to_string())),
        syn::Item::Mod(value) => Some((5, value.ident.to_string())),
        syn::Item::Macro(value) => Some((6, tokens(&value.ident))),
        syn::Item::Use(value) => Some((7, tokens(&value.tree))),
        syn::Item::ExternCrate(value) => Some((8, value.ident.to_string())),
        syn::Item::Trait(value) => Some((9, value.ident.to_string())),
        syn::Item::Type(value) => Some((10, value.ident.to_string())),
        _ => None,
    }
}

// Only literal outer documentation and signature-list trailing punctuation
// are projected. Attributes, expression syntax and bodies otherwise survive.
fn signature_lists(signature: &mut syn::Signature) {
    signature.inputs = std::mem::take(&mut signature.inputs).into_iter().collect();
    signature.generics.params = std::mem::take(&mut signature.generics.params)
        .into_iter()
        .collect();
    if let Some(clause) = &mut signature.generics.where_clause {
        clause.predicates = std::mem::take(&mut clause.predicates).into_iter().collect();
    }
}

struct Documentation;

macro_rules! documentation_nodes {
    ($($method:ident, $ty:ty;)+) => {$(
        fn $method(&mut self, node: &mut $ty) {
            node.attrs.retain(|attribute| !supported_doc(attribute));
            visit_mut::$method(self, node);
        }
    )+};
}

impl VisitMut for Documentation {
    documentation_nodes! {
        visit_item_fn_mut, syn::ItemFn;
        visit_item_impl_mut, syn::ItemImpl;
        visit_item_struct_mut, syn::ItemStruct;
        visit_item_enum_mut, syn::ItemEnum;
        visit_item_const_mut, syn::ItemConst;
        visit_item_static_mut, syn::ItemStatic;
        visit_item_trait_mut, syn::ItemTrait;
        visit_impl_item_fn_mut, syn::ImplItemFn;
        visit_trait_item_fn_mut, syn::TraitItemFn;
        visit_field_mut, syn::Field;
        visit_variant_mut, syn::Variant;
    }

    fn visit_signature_mut(&mut self, signature: &mut syn::Signature) {
        signature_lists(signature);
        visit_mut::visit_signature_mut(self, signature);
    }
}

fn project(mut item: syn::Item) -> syn::Item {
    Documentation.visit_item_mut(&mut item);
    item
}

fn project_method(mut method: syn::ImplItemFn) -> syn::ImplItemFn {
    Documentation.visit_impl_item_fn_mut(&mut method);
    method
}

fn extract_native_limits(sources: &Sources<'_>) -> Result<SchemaInputLimits, Refusal> {
    use crate::schema_validation::work::{self, ContextSources, SourceRole};
    // Reuse the existing source-recipe owner, not a copied input recognizer.
    // Other fixed context operands remain conditional owner data; this opens
    // no JSON binding, descriptor constructor or actual-source window.
    let context = ContextSources {
        native: sources.bytes(SourceUnit::Native)?,
        input: sources.bytes(SourceUnit::Input)?,
        json_size: sources.bytes(SourceUnit::JsonSize)?,
        budget: work::BUDGET,
        exchange_schema: work::EXCHANGE_SCHEMA,
        exchange_run: work::EXCHANGE_RUN,
        exchange_split: work::EXCHANGE_SPLIT,
        exchange_limits: work::EXCHANGE_LIMITS,
        dispatch_generator: work::DISPATCH_GENERATOR,
        formats: work::FORMATS,
    };
    work::context::check_sources(&context).map_err(|error| match error {
        work::Refusal::SourceShape(SourceRole::Native) => Refusal::SourceShape(SourceUnit::Native),
        work::Refusal::SourceShape(SourceRole::Input) => Refusal::SourceShape(SourceUnit::Input),
        work::Refusal::SourceShape(SourceRole::JsonSize) => {
            Refusal::SourceShape(SourceUnit::JsonSize)
        }
        work::Refusal::SourceShape(
            SourceRole::Budget
            | SourceRole::ExchangeSchema
            | SourceRole::ExchangeRun
            | SourceRole::ExchangeSplit
            | SourceRole::ExchangeLimits
            | SourceRole::DispatchGenerator
            | SourceRole::Formats,
        )
        | work::Refusal::UnsupportedContext
        | work::Refusal::BindingMismatch
        | work::Refusal::MissingCallerFact(_)
        | work::Refusal::UnsupportedNativePath
        | work::Refusal::NoSourceLeafDuty(_)
        | work::Refusal::MissingRuntimeLemma(_)
        | work::Refusal::InputRejected(_)
        | work::Refusal::Overflow
        | work::Refusal::ScalarSource(_) => Refusal::MissingSourceProof,
    })
}

fn check_root_attributes(file: &syn::File, unit: SourceUnit) -> Result<(), Refusal> {
    if file.shebang.is_some() || file.frontmatter.is_some() {
        return Err(Refusal::SourceShape(unit));
    }
    let actual: Vec<_> = file
        .attrs
        .iter()
        .filter(|attribute| {
            !matches!(&attribute.meta, syn::Meta::NameValue(meta)
            if attribute.path().is_ident("doc")
            && matches!(&meta.value, syn::Expr::Lit(value)
                if value.attrs.is_empty() && matches!(&value.lit, syn::Lit::Str(_))))
        })
        .collect();
    let expected = root_attributes(unit);
    let parsed: syn::File = syn::parse2(expected).map_err(|_error| Refusal::SourceShape(unit))?;
    let attrs = &parsed.attrs;
    same(unit, &quote!(#(#actual)*), &quote!(#(#attrs)*))
}

fn root_attributes(unit: SourceUnit) -> TokenStream {
    match unit {
        SourceUnit::SerdeFacade => {
            let lint = serde_lints();
            quote! {
                #![doc(html_root_url = "https://docs.rs/serde/1.0.229")]
                #![cfg_attr(not(feature = "std"), no_std)]
                #![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
                #![cfg_attr(docsrs, allow(internal_features))]
                #![cfg_attr(all(feature = "unstable", docsrs), feature(never_type))]
                #![allow(unknown_lints, bare_trait_objects, deprecated, mismatched_lifetime_syntaxes)]
                #lint
                #![deny(clippy::question_mark_used)]
                #![deny(missing_docs, unused_imports)]
            }
        }
        SourceUnit::SerdeCoreRoot => {
            let lint = serde_lints();
            quote! {
                #![doc(html_root_url = "https://docs.rs/serde_core/1.0.229")]
                #![cfg_attr(not(feature = "std"), no_std)]
                #![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
                #![cfg_attr(docsrs, allow(internal_features))]
                #![cfg_attr(feature = "unstable", feature(never_type))]
                #![allow(unknown_lints, bare_trait_objects, deprecated)]
                #lint
                #![deny(clippy::question_mark_used)]
                #![deny(missing_docs, unused_imports)]
            }
        }
        SourceUnit::JsonRoot => json_root_attributes(),
        SourceUnit::IoWrite | SourceUnit::StrBytes | SourceUnit::Slice | SourceUnit::Default => {
            quote!(#![stable(feature = "rust1", since = "1.0.0")])
        }
        SourceUnit::Native
        | SourceUnit::Input
        | SourceUnit::JsonSize
        | SourceUnit::SerdeCoreExports
        | SourceUnit::SerdeTraits
        | SourceUnit::SerdeStr
        | SourceUnit::JsonIo
        | SourceUnit::JsonSerializer
        | SourceUnit::JsonValue
        | SourceUnit::JsonValueSerialize
        | SourceUnit::JsonNumber
        | SourceUnit::IoForward
        | SourceUnit::SliceIndex => TokenStream::new(),
    }
}

// These attributes are quoted external-source DATA, not local lint policy.
fn serde_lints() -> TokenStream {
    quote! {
        #![allow(
            clippy::unnested_or_patterns, clippy::semicolon_if_nothing_returned,
            clippy::empty_enums, clippy::type_repetition_in_bounds,
            clippy::cast_possible_truncation, clippy::cast_possible_wrap,
            clippy::cast_precision_loss, clippy::cast_sign_loss,
            clippy::cast_lossless, clippy::module_name_repetitions,
            clippy::single_match_else, clippy::type_complexity, clippy::use_self,
            clippy::zero_prefixed_literal, clippy::derive_partial_eq_without_eq,
            clippy::enum_glob_use, clippy::explicit_auto_deref, clippy::incompatible_msrv,
            clippy::let_underscore_untyped, clippy::map_err_ignore, clippy::new_without_default,
            clippy::result_unit_err, clippy::wildcard_imports, clippy::needless_pass_by_value,
            clippy::similar_names, clippy::too_many_lines, clippy::doc_markdown,
            clippy::elidable_lifetime_names, clippy::needless_lifetimes,
            clippy::unseparated_literal_suffix, clippy::needless_doctest_main,
            clippy::missing_errors_doc, clippy::must_use_candidate,
        )]
    }
}

fn json_root_attributes() -> TokenStream {
    quote! {
        #![doc(html_root_url = "https://docs.rs/serde_json/1.0.150")]
        #![allow(
            clippy::collapsible_else_if, clippy::comparison_chain, clippy::deprecated_cfg_attr,
            clippy::doc_markdown, clippy::elidable_lifetime_names, clippy::excessive_precision,
            clippy::explicit_auto_deref, clippy::float_cmp, clippy::manual_range_contains,
            clippy::match_like_matches_macro, clippy::match_single_binding,
            clippy::needless_doctest_main, clippy::needless_late_init, clippy::needless_lifetimes,
            clippy::return_self_not_must_use, clippy::transmute_ptr_to_ptr,
            clippy::unbuffered_bytes, clippy::unconditional_recursion,
            clippy::uninlined_format_args, clippy::unnecessary_wraps
        )]
        #![allow(
            clippy::should_implement_trait, clippy::cast_possible_truncation,
            clippy::cast_possible_wrap, clippy::cast_precision_loss, clippy::cast_sign_loss,
            clippy::enum_glob_use, clippy::if_not_else, clippy::integer_division,
            clippy::let_underscore_untyped, clippy::map_err_ignore, clippy::match_same_arms,
            clippy::similar_names, clippy::unused_self, clippy::wildcard_imports,
            clippy::cast_lossless, clippy::items_after_statements, clippy::module_name_repetitions,
            clippy::redundant_else, clippy::shadow_unrelated, clippy::single_match_else,
            clippy::too_many_lines, clippy::unreadable_literal, clippy::unseparated_literal_suffix,
            clippy::use_self, clippy::zero_prefixed_literal, clippy::checked_conversions,
            clippy::mem_replace_with_default, clippy::missing_errors_doc, clippy::must_use_candidate,
        )]
        #![deny(clippy::question_mark_used)]
        #![allow(non_upper_case_globals)]
        #![deny(missing_docs)]
        #![no_std]
        #![cfg_attr(docsrs, feature(doc_cfg))]
        #![allow(unknown_lints, mismatched_lifetime_syntaxes)]
    }
}

// Only the known zero-argument, single-arm crate_root wrapper is projected.
// This neither expands arbitrary macros nor discards their original source.
struct CrateRoot(syn::File);

impl Parse for CrateRoot {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let matcher;
        syn::parenthesized!(matcher in input);
        if !matcher.is_empty() {
            return Err(matcher.error("nonempty crate_root matcher"));
        }
        input.parse::<syn::Token![=>]>()?;
        let expansion;
        syn::braced!(expansion in input);
        let file: syn::File = expansion.parse()?;
        if !file.attrs.is_empty() || file.shebang.is_some() || file.frontmatter.is_some() {
            return Err(expansion.error("unsupported crate_root attributes"));
        }
        input.parse::<syn::Token![;]>()?;
        if !input.is_empty() {
            return Err(input.error("extra crate_root arm"));
        }
        Ok(Self(file))
    }
}

fn crate_root(
    sources: &Sources<'_>,
    unit: SourceUnit,
    attrs: TokenStream,
) -> Result<syn::File, Refusal> {
    let mut definitions = sources.file(unit)?.items.iter().filter_map(|item| {
        if let syn::Item::Macro(macro_) = item
            && macro_
                .ident
                .as_ref()
                .is_some_and(|name| name == "crate_root")
            && macro_.mac.path.is_ident("macro_rules")
        {
            Some(macro_)
        } else {
            None
        }
    });
    let definition = unique(&mut definitions, unit)?;
    let actual_attrs = &definition.attrs;
    same(unit, &quote!(#(#actual_attrs)*), &attrs)?;
    let CrateRoot(file) =
        syn::parse2(definition.mac.tokens.clone()).map_err(|_error| Refusal::SourceShape(unit))?;
    Ok(file)
}

fn file_exact(file: &syn::File, unit: SourceUnit, expected: syn::Item) -> Result<(), Refusal> {
    let identity = item_identity(&expected);
    let mut matches = file
        .items
        .iter()
        .filter(|item| item_identity(item) == identity);
    let actual = unique(&mut matches, unit)?;
    same(unit, &project(actual.clone()), &project(expected))
}

fn macro_call(
    file: &syn::File,
    unit: SourceUnit,
    expected: &syn::ItemMacro,
) -> Result<(), Refusal> {
    let mut calls = file.items.iter().filter_map(|item| {
        if let syn::Item::Macro(macro_) = item
            && macro_.ident.is_none()
            && tokens(&macro_.mac.path) == tokens(&expected.mac.path)
        {
            Some(macro_)
        } else {
            None
        }
    });
    same(unit, unique(&mut calls, unit)?, expected)
}

fn serde_sources(sources: &Sources<'_>) -> Result<(), Refusal> {
    let facade = SourceUnit::SerdeFacade;
    let exports = crate_root(sources, facade, quote!(#[cfg(not(docsrs))]))?;
    file_exact(
        &exports,
        facade,
        syn::parse_quote! {
            pub use serde_core::{
                de, forward_to_deserialize_any, ser, Deserialize, Deserializer, Serialize, Serializer,
            };
        },
    )?;
    macro_call(
        sources.file(facade)?,
        facade,
        &syn::parse_quote!(crate_root!();),
    )?;
    let core = SourceUnit::SerdeCoreRoot;
    sources.exact(
        core,
        syn::parse_quote!(
            #[macro_use]
            mod crate_root;
        ),
    )?;
    macro_call(
        sources.file(core)?,
        core,
        &syn::parse_quote!(crate_root!();),
    )?;
    let exported = SourceUnit::SerdeCoreExports;
    let core_exports = crate_root(sources, exported, quote!())?;
    file_exact(
        &core_exports,
        exported,
        syn::parse_quote! {
            #[cfg_attr(all(docsrs, if_docsrs_then_no_serde_core), path = "core/ser/mod.rs")]
            pub mod ser;
        },
    )?;
    file_exact(
        &core_exports,
        exported,
        syn::parse_quote! {
            #[doc(inline)] pub use crate::ser::{Serialize, Serializer};
        },
    )?;
    serde_dispatch(sources)
}

fn serde_dispatch(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::SerdeTraits;
    sources.exact(
        unit,
        syn::parse_quote!(
            use crate::lib::*;
        ),
    )?;
    sources.exact(
        unit,
        syn::parse_quote!(
            mod impls;
        ),
    )?;
    sources.trait_methods(unit, syn::parse_quote! {
        #[cfg_attr(
            not(no_diagnostic_namespace),
            diagnostic::on_unimplemented(
                message = "the trait bound `{Self}: serde::Serialize` is not satisfied",
                note = "for local types consider adding `#[derive(serde::Serialize)]` to your `{Self}` type",
                note = "for types from other crates check whether the crate offers a `serde` feature flag",
            )
        )]
        pub trait Serialize {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer;
        }
    })?;
    sources.trait_methods(
        unit,
        syn::parse_quote! {
            #[cfg_attr(
                not(no_diagnostic_namespace),
                diagnostic::on_unimplemented(
                    message = "the trait bound `{Self}: serde::Serializer` is not satisfied",
                )
            )]
            pub trait Serializer: Sized {
                fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error>;
                fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error>;
                fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error>;
                fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error>;
                fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error>;
                fn serialize_unit(self) -> Result<Self::Ok, Self::Error>;
            }
        },
    )?;
    sources.exact(
        SourceUnit::SerdeStr,
        syn::parse_quote!(
            use crate::lib::*;
        ),
    )?;
    sources.exact(
        SourceUnit::SerdeStr,
        syn::parse_quote! {
            use crate::ser::{Error, Serialize, SerializeTuple, Serializer};
        },
    )?;
    sources.implementation(SourceUnit::SerdeStr, syn::parse_quote! {
        impl Serialize for str {
            #[inline]
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
                serializer.serialize_str(self)
            }
        }
    })?;
    Ok(())
}

fn json_sources(sources: &Sources<'_>) -> Result<(), Refusal> {
    json_exports(sources)?;
    json_construction(sources)?;
    json_dispatch(sources)?;
    value_dispatch(sources)?;
    number_dispatch(sources)?;
    numeric_formatter(sources)
}

fn exact_items(
    sources: &Sources<'_>,
    unit: SourceUnit,
    items: &[syn::Item],
) -> Result<(), Refusal> {
    for item in items {
        sources.exact(unit, item.clone())?;
    }
    Ok(())
}

fn json_exports(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::JsonRoot;
    exact_items(
        sources,
        unit,
        &[
            syn::parse_quote!(
                extern crate alloc;
            ),
            syn::parse_quote!(
                #[cfg(feature = "std")]
                extern crate std;
            ),
            syn::parse_quote!(
                extern crate serde_core as serde;
            ),
            syn::parse_quote! {
                #[cfg(feature = "std")]
                #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
                #[doc(inline)]
                pub use crate::ser::{to_writer, to_writer_pretty, Serializer};
            },
            syn::parse_quote! {
                #[cfg(feature = "std")]
                #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
                pub mod ser;
            },
            syn::parse_quote!(
                mod io;
            ),
            syn::parse_quote!(
                pub mod value;
            ),
            syn::parse_quote!(
                mod number;
            ),
            syn::parse_quote! {
                macro_rules! tri {
                    ($e:expr $(,)?) => {
                        match $e {
                            core::result::Result::Ok(val) => val,
                            core::result::Result::Err(err) => return core::result::Result::Err(err),
                        }
                    };
                }
            },
        ],
    )?;
    exact_items(
        sources,
        SourceUnit::JsonIo,
        &[
            syn::parse_quote!(
                pub use self::imp::{Error, ErrorKind, Result, Write};
            ),
            syn::parse_quote!(
                #[cfg(feature = "std")]
                use std::io as imp;
            ),
        ],
    )?;
    exact_items(
        sources,
        SourceUnit::JsonSerializer,
        &[
            syn::parse_quote!(
                use crate::error::{Error, ErrorCode, Result};
            ),
            syn::parse_quote!(
                use crate::io;
            ),
            syn::parse_quote!(
                use core::hint;
            ),
            syn::parse_quote!(
                use core::num::FpCategory;
            ),
            syn::parse_quote!(
                use core::str;
            ),
            syn::parse_quote!(
                use serde::ser::{self, Impossible, Serialize};
            ),
        ],
    )
}

fn json_construction(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::JsonSerializer;
    sources.exact(
        unit,
        syn::parse_quote! {
            #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
            pub struct Serializer<W, F = CompactFormatter> { writer: W, formatter: F, }
        },
    )?;
    sources.implementation(
        unit,
        terminated_expected_impl(syn::parse_quote! {
            impl<W> Serializer<W> where W: io::Write {
                #[inline]
                pub fn new(writer: W) -> Self {
                    Serializer::with_formatter(writer, CompactFormatter)
                }
            }
        }),
    )?;
    sources.implementation(
        unit,
        terminated_expected_impl(syn::parse_quote! {
            impl<W, F> Serializer<W, F> where W: io::Write, F: Formatter {
                #[inline]
                pub fn with_formatter(writer: W, formatter: F) -> Self {
                    Serializer { writer, formatter }
                }
            }
        }),
    )?;
    sources.function(
        unit,
        syn::parse_quote! {
            #[inline]
            #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
            pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
            where W: io::Write, T: ?Sized + Serialize {
                let mut ser = Serializer::new(writer);
                value.serialize(&mut ser)
            }
        },
    )?;
    sources.exact(
        unit,
        syn::parse_quote! {
            #[derive(Clone, Debug, Default)]
            pub struct CompactFormatter;
        },
    )?;
    sources.implementation(
        unit,
        syn::parse_quote!(impl Formatter for CompactFormatter {}),
    )?;
    Ok(())
}

// These three locked serde_json impl headers end their where lists with a
// comma. Set only the expected AST punctuation, independently of rustfmt.
fn terminated_expected_impl(mut expected: syn::ItemImpl) -> syn::ItemImpl {
    if let Some(clause) = &mut expected.generics.where_clause {
        let predicates: &mut syn::punctuated::Punctuated<syn::WherePredicate, syn::Token![,]> =
            &mut clause.predicates;
        if !predicates.is_empty() && !predicates.trailing_punct() {
            predicates.push_punct(<syn::Token![,]>::default());
        }
    }
    expected
}

fn json_dispatch(sources: &Sources<'_>) -> Result<(), Refusal> {
    sources.implementation(SourceUnit::JsonSerializer, terminated_expected_impl(syn::parse_quote! {
        impl<'a, W, F> ser::Serializer for &'a mut Serializer<W, F>
        where W: io::Write, F: Formatter {
            type Ok = ();
            type Error = Error;
            #[inline]
            fn serialize_bool(self, value: bool) -> Result<()> {
                self.formatter.write_bool(&mut self.writer, value).map_err(Error::io)
            }
            #[inline]
            fn serialize_i64(self, value: i64) -> Result<()> {
                self.formatter.write_i64(&mut self.writer, value).map_err(Error::io)
            }
            #[inline]
            fn serialize_u64(self, value: u64) -> Result<()> {
                self.formatter.write_u64(&mut self.writer, value).map_err(Error::io)
            }
            #[inline]
            fn serialize_f64(self, value: f64) -> Result<()> {
                match value.classify() {
                    FpCategory::Nan | FpCategory::Infinite =>
                        self.formatter.write_null(&mut self.writer).map_err(Error::io),
                    _ => self.formatter.write_f64(&mut self.writer, value).map_err(Error::io),
                }
            }
            #[inline]
            fn serialize_str(self, value: &str) -> Result<()> {
                format_escaped_str(&mut self.writer, &mut self.formatter, value).map_err(Error::io)
            }
            #[inline]
            fn serialize_unit(self) -> Result<()> {
                self.formatter.write_null(&mut self.writer).map_err(Error::io)
            }
        }
    }))?;
    Ok(())
}

fn value_dispatch(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::JsonValue;
    exact_items(
        sources,
        unit,
        &[
            syn::parse_quote!(
                use alloc::string::String;
            ),
            syn::parse_quote!(
                use alloc::vec::Vec;
            ),
            syn::parse_quote!(
                pub use crate::number::Number;
            ),
            syn::parse_quote!(
                pub use crate::map::Map;
            ),
            syn::parse_quote! {
                #[derive(Clone, Eq, PartialEq, Hash)]
                pub enum Value {
                    Null, Bool(bool), Number(Number), String(String),
                    Array(Vec<Value>), Object(Map<String, Value>),
                }
            },
        ],
    )?;
    exact_items(
        sources,
        SourceUnit::JsonValueSerialize,
        &[
            syn::parse_quote!(
                use crate::value::{to_value, Value};
            ),
            syn::parse_quote!(
                use core::result;
            ),
            syn::parse_quote!(
                use serde::ser::{Impossible, Serialize};
            ),
        ],
    )?;
    sources.implementation(
        SourceUnit::JsonValueSerialize,
        syn::parse_quote! {
            impl Serialize for Value {
                #[inline]
                fn serialize<S>(&self, serializer: S) -> result::Result<S::Ok, S::Error>
                where S: ::serde::Serializer {
                    match self {
                        Value::Null => serializer.serialize_unit(),
                        Value::Bool(b) => serializer.serialize_bool(*b),
                        Value::Number(n) => n.serialize(serializer),
                        Value::String(s) => serializer.serialize_str(s),
                        Value::Array(v) => v.serialize(serializer),
                        #[cfg(any(feature = "std", feature = "alloc"))]
                        Value::Object(m) => {
                            use serde::ser::SerializeMap;
                            let mut map = tri!(serializer.serialize_map(Some(m.len())));
                            for (k, v) in m { tri!(map.serialize_entry(k, v)); }
                            map.end()
                        }
                        #[cfg(not(any(feature = "std", feature = "alloc")))]
                        Value::Object(_) => unreachable!(),
                    }
                }
            }
        },
    )?;
    Ok(())
}

fn number_dispatch(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::JsonNumber;
    exact_items(
        sources,
        unit,
        &[
            syn::parse_quote! {
                use serde::{forward_to_deserialize_any, Deserialize, Deserializer, Serialize, Serializer};
            },
            syn::parse_quote! {
                #[derive(Clone, PartialEq, Eq, Hash)]
                pub struct Number { n: N, }
            },
            syn::parse_quote! {
                #[cfg(not(feature = "arbitrary_precision"))]
                #[derive(Copy, Clone)]
                enum N { PosInt(u64), NegInt(i64), Float(f64), }
            },
        ],
    )?;
    sources.implementation(
        unit,
        syn::parse_quote! {
            impl Serialize for Number {
                #[cfg(not(feature = "arbitrary_precision"))]
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: Serializer {
                    match self.n {
                        N::PosInt(u) => serializer.serialize_u64(u),
                        N::NegInt(i) => serializer.serialize_i64(i),
                        N::Float(f) => serializer.serialize_f64(f),
                    }
                }
            }
        },
    )?;
    Ok(())
}

// A completed formatter result is one str slice, not a proved itoa/zmij cost
// or termination theorem. Each concrete scalar Counter starts afresh.
fn numeric_formatter(sources: &Sources<'_>) -> Result<(), Refusal> {
    sources.trait_methods(
        SourceUnit::JsonSerializer,
        syn::parse_quote! {
            pub trait Formatter {
                #[inline]
                fn write_null<W>(&mut self, writer: &mut W) -> io::Result<()>
                where W: ?Sized + io::Write { writer.write_all(b"null") }
                #[inline]
                fn write_bool<W>(&mut self, writer: &mut W, value: bool) -> io::Result<()>
                where W: ?Sized + io::Write {
                    let s = if value { b"true" as &[u8] } else { b"false" as &[u8] };
                    writer.write_all(s)
                }
                #[inline]
                fn write_i64<W>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
                where W: ?Sized + io::Write {
                    let mut buffer = itoa::Buffer::new();
                    let s = buffer.format(value);
                    writer.write_all(s.as_bytes())
                }
                #[inline]
                fn write_u64<W>(&mut self, writer: &mut W, value: u64) -> io::Result<()>
                where W: ?Sized + io::Write {
                    let mut buffer = itoa::Buffer::new();
                    let s = buffer.format(value);
                    writer.write_all(s.as_bytes())
                }
                #[inline]
                fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
                where W: ?Sized + io::Write {
                    let mut buffer = zmij::Buffer::new();
                    let s = buffer.format_finite(value);
                    writer.write_all(s.as_bytes())
                }
            }
        },
    )?;
    Ok(())
}

struct StringLengths {
    quotes: u64,
    escape: u64,
}

fn string_sources(sources: &Sources<'_>) -> Result<StringLengths, Refusal> {
    string_scan(sources)?;
    escape_table(sources)?;
    let mut framing = sources.trait_methods(SourceUnit::JsonSerializer, syn::parse_quote! {
        pub trait Formatter {
            #[inline]
            fn begin_string<W>(&mut self, writer: &mut W) -> io::Result<()>
            where W: ?Sized + io::Write { writer.write_all(b"\"") }
            #[inline]
            fn end_string<W>(&mut self, writer: &mut W) -> io::Result<()>
            where W: ?Sized + io::Write { writer.write_all(b"\"") }
            #[inline]
            fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
            where W: ?Sized + io::Write { writer.write_all(fragment.as_bytes()) }
        }
    })?;
    let mut literals = RequestWidths::default();
    for method in &mut framing {
        literals.visit_trait_item_fn_mut(method);
    }
    let quotes = literals
        .byte_strings
        .into_iter()
        .try_fold(0_u64, |total, length| {
            total
                .checked_add(u64::try_from(length).map_err(|_error| Refusal::Overflow)?)
                .ok_or(Refusal::Overflow)
        })?;
    let mut escaping = sources.trait_methods(SourceUnit::JsonSerializer, escape_formatter())?;
    let mut arrays = RequestWidths::default();
    for method in &mut escaping {
        arrays.visit_trait_item_fn_mut(method);
    }
    let escape = u64::try_from(
        arrays
            .arrays
            .into_iter()
            .max()
            .ok_or(Refusal::SourceShape(SourceUnit::JsonSerializer))?,
    )
    .map_err(|_error| Refusal::Overflow)?;
    // The exact scanner partitions raw bytes into unescaped runs and ASCII
    // escapes. Each consumed byte offers at least one and at most escape bytes;
    // the two framing requests add quotes. Thus r+quotes <= L <= escape*r+quotes.
    // With the matched precheck/charge order, successful prior operands charge
    // at least r+quotes; a final serialization-reaching operand fits the
    // remaining byte budget even if its expanded charge is rejected.
    Ok(StringLengths { quotes, escape })
}

#[derive(Default)]
struct RequestWidths {
    byte_strings: Vec<usize>,
    arrays: Vec<usize>,
    writes: Vec<syn::Ident>,
}

impl VisitMut for RequestWidths {
    fn visit_lit_byte_str_mut(&mut self, literal: &mut syn::LitByteStr) {
        self.byte_strings.push(literal.value().len());
    }

    fn visit_expr_array_mut(&mut self, array: &mut syn::ExprArray) {
        self.arrays.push(array.elems.len());
        visit_mut::visit_expr_array_mut(self, array);
    }

    fn visit_expr_method_call_mut(&mut self, call: &mut syn::ExprMethodCall) {
        if call.method == "write" {
            self.writes.push(call.method.clone());
        }
        visit_mut::visit_expr_method_call_mut(self, call);
    }
}

fn string_scan(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::JsonSerializer;
    sources.function(unit, syn::parse_quote! {
        fn format_escaped_str<W, F>(writer: &mut W, formatter: &mut F, value: &str) -> io::Result<()>
        where W: ?Sized + io::Write, F: ?Sized + Formatter {
            tri!(formatter.begin_string(writer));
            tri!(format_escaped_str_contents(writer, formatter, value));
            formatter.end_string(writer)
        }
    })?;
    sources.function(
        unit,
        syn::parse_quote! {
            fn format_escaped_str_contents<W, F>(
                writer: &mut W, formatter: &mut F, value: &str,
            ) -> io::Result<()>
            where W: ?Sized + io::Write, F: ?Sized + Formatter {
                let mut bytes = value.as_bytes();
                let mut i = 0;
                while i < bytes.len() {
                    let (string_run, rest) = bytes.split_at(i);
                    let (&byte, rest) = rest.split_first().unwrap();
                    let escape = ESCAPE[byte as usize];
                    i += 1;
                    if escape == 0 { continue; }
                    bytes = rest;
                    i = 0;
                    let string_run = unsafe { str::from_utf8_unchecked(string_run) };
                    if !string_run.is_empty() {
                        tri!(formatter.write_string_fragment(writer, string_run));
                    }
                    let char_escape = match escape {
                        self::BB => CharEscape::Backspace,
                        self::TT => CharEscape::Tab,
                        self::NN => CharEscape::LineFeed,
                        self::FF => CharEscape::FormFeed,
                        self::RR => CharEscape::CarriageReturn,
                        self::QU => CharEscape::Quote,
                        self::BS => CharEscape::ReverseSolidus,
                        self::UU => CharEscape::AsciiControl(byte),
                        _ => unsafe { hint::unreachable_unchecked() },
                    };
                    tri!(formatter.write_char_escape(writer, char_escape));
                }
                let string_run = unsafe { str::from_utf8_unchecked(bytes) };
                if string_run.is_empty() { return Ok(()); }
                formatter.write_string_fragment(writer, string_run)
            }
        },
    )?;
    Ok(())
}

fn escape_formatter() -> syn::ItemTrait {
    syn::parse_quote! {
        pub trait Formatter {
            #[inline]
            fn write_char_escape<W>(&mut self, writer: &mut W, char_escape: CharEscape) -> io::Result<()>
            where W: ?Sized + io::Write {
                use self::CharEscape::*;
                let escape_char = match char_escape {
                    Quote => b'"',
                    ReverseSolidus => b'\\',
                    Solidus => b'/',
                    Backspace => b'b',
                    FormFeed => b'f',
                    LineFeed => b'n',
                    CarriageReturn => b'r',
                    Tab => b't',
                    AsciiControl(_) => b'u',
                };
                match char_escape {
                    AsciiControl(byte) => {
                        static HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";
                        let bytes = &[
                            b'\\', escape_char, b'0', b'0',
                            HEX_DIGITS[(byte >> 4) as usize],
                            HEX_DIGITS[(byte & 0xF) as usize],
                        ];
                        writer.write_all(bytes)
                    }
                    _ => writer.write_all(&[b'\\', escape_char]),
                }
            }
        }
    }
}

fn escape_table(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::JsonSerializer;
    exact_items(
        sources,
        unit,
        &[
            syn::parse_quote! {
                pub enum CharEscape {
                    Quote, ReverseSolidus, Solidus, Backspace, FormFeed, LineFeed,
                    CarriageReturn, Tab, AsciiControl(u8),
                }
            },
            syn::parse_quote!(
                const BB: u8 = b'b';
            ),
            syn::parse_quote!(
                const TT: u8 = b't';
            ),
            syn::parse_quote!(
                const NN: u8 = b'n';
            ),
            syn::parse_quote!(
                const FF: u8 = b'f';
            ),
            syn::parse_quote!(
                const RR: u8 = b'r';
            ),
            syn::parse_quote!(
                const QU: u8 = b'"';
            ),
            syn::parse_quote!(
                const BS: u8 = b'\\';
            ),
            syn::parse_quote!(
                const UU: u8 = b'u';
            ),
            syn::parse_quote!(
                const __: u8 = 0;
            ),
        ],
    )?;
    // Every entry is checked, including all non-ASCII bytes. This is the
    // supported ASCII JSON escape classification, not a serializer or walker.
    let entries = (0_u16..=255).map(|byte| match byte {
        8 => quote!(BB),
        9 => quote!(TT),
        10 => quote!(NN),
        12 => quote!(FF),
        13 => quote!(RR),
        34 => quote!(QU),
        92 => quote!(BS),
        0..=31 => quote!(UU),
        _ => quote!(__),
    });
    sources.exact(
        unit,
        syn::parse_quote! {
            static ESCAPE: [u8; 256] = [#(#entries,)*];
        },
    )?;
    Ok(())
}

fn writer_sources(sources: &Sources<'_>) -> Result<u64, Refusal> {
    let mut methods = sources.trait_methods(
        SourceUnit::IoWrite,
        syn::parse_quote! {
            #[stable(feature = "rust1", since = "1.0.0")]
            #[doc(notable_trait)]
            #[cfg_attr(not(test), rustc_diagnostic_item = "IoWrite")]
            pub trait Write {
                #[stable(feature = "rust1", since = "1.0.0")]
                fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
                    while !buf.is_empty() {
                        match self.write(buf) {
                            Ok(0) => { return Err(Error::WRITE_ALL_EOF); }
                            Ok(n) => buf = &buf[n..],
                            Err(ref e) if e.is_interrupted() => {}
                            Err(e) => return Err(e),
                        }
                    }
                    Ok(())
                }
            }
        },
    )?;
    sources.exact(SourceUnit::IoForward, syn::parse_quote! {
        use crate::io::{self, BorrowedCursor, BufRead, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write};
    })?;
    sources.implementation(
        SourceUnit::IoForward,
        syn::parse_quote! {
            #[stable(feature = "rust1", since = "1.0.0")]
            impl<W: Write + ?Sized> Write for &mut W {
                #[inline]
                fn write(&mut self, buf: &[u8]) -> io::Result<usize> { (**self).write(buf) }
                #[inline]
                fn write_all(&mut self, buf: &[u8]) -> io::Result<()> { (**self).write_all(buf) }
            }
        },
    )?;
    let mut requests = RequestWidths::default();
    for method in &mut methods {
        requests.visit_trait_item_fn_mut(method);
    }
    // Only the full-length Ok arm can be reached by this concrete Counter.
    // One write empties buf; initially empty buf bypasses the loop. Its Err
    // arms therefore add neither retries nor a hidden error-handler premise.
    u64::try_from(requests.writes.len()).map_err(|_error| Refusal::Overflow)
}

fn primitive_sources(sources: &Sources<'_>) -> Result<(), Refusal> {
    str_lengths(sources)?;
    slice_lengths(sources)?;
    slice_splitting(sources)?;
    slice_indices(sources)?;
    default_zero(sources)
}

fn str_lengths(sources: &Sources<'_>) -> Result<(), Refusal> {
    sources.implementation(
        SourceUnit::StrBytes,
        syn::parse_quote! {
            impl str {
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_stable(feature = "const_str_len", since = "1.39.0")]
                #[rustc_diagnostic_item = "str_len"]
                #[rustc_no_implicit_autorefs]
                #[must_use]
                #[inline]
                pub const fn len(&self) -> usize { self.as_bytes().len() }
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_stable(feature = "const_str_is_empty", since = "1.39.0")]
                #[rustc_no_implicit_autorefs]
                #[must_use]
                #[inline]
                pub const fn is_empty(&self) -> bool { self.len() == 0 }
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_stable(feature = "str_as_bytes", since = "1.39.0")]
                #[must_use]
                #[inline(always)]
                #[allow(unused_attributes)]
                pub const fn as_bytes(&self) -> &[u8] { unsafe { mem::transmute(self) } }
            }
        },
    )?;
    Ok(())
}

fn slice_lengths(sources: &Sources<'_>) -> Result<(), Refusal> {
    sources.implementation(
        SourceUnit::Slice,
        syn::parse_quote! {
            impl<T> [T] {
                #[lang = "slice_len_fn"]
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_stable(feature = "const_slice_len", since = "1.39.0")]
                #[rustc_no_implicit_autorefs]
                #[inline]
                #[must_use]
                pub const fn len(&self) -> usize { ptr::metadata(self) }
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_stable(feature = "const_slice_is_empty", since = "1.39.0")]
                #[rustc_no_implicit_autorefs]
                #[inline]
                #[must_use]
                pub const fn is_empty(&self) -> bool { self.len() == 0 }
                #[stable(feature = "slice_splits", since = "1.5.0")]
                #[rustc_const_stable(feature = "const_slice_first_last_not_mut", since = "1.56.0")]
                #[inline]
                #[must_use]
                pub const fn split_first(&self) -> Option<(&T, &[T])> {
                    if let [first, tail @ ..] = self { Some((first, tail)) } else { None }
                }
            }
        },
    )?;
    Ok(())
}

fn slice_splitting(sources: &Sources<'_>) -> Result<(), Refusal> {
    sources.implementation(
        SourceUnit::Slice,
        syn::parse_quote! {
            impl<T> [T] {
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_stable(feature = "const_slice_split_at_not_mut", since = "1.71.0")]
                #[inline]
                #[track_caller]
                #[must_use]
                pub const fn split_at(&self, mid: usize) -> (&[T], &[T]) {
                    match self.split_at_checked(mid) {
                        Some(pair) => pair,
                        None => panic!("mid > len"),
                    }
                }
                #[stable(feature = "split_at_checked", since = "1.80.0")]
                #[rustc_const_stable(feature = "split_at_checked", since = "1.80.0")]
                #[inline]
                #[must_use]
                pub const fn split_at_checked(&self, mid: usize) -> Option<(&[T], &[T])> {
                    if mid <= self.len() { Some(unsafe { self.split_at_unchecked(mid) }) }
                    else { None }
                }
            }
        },
    )?;
    Ok(())
}

// Syn retains these const impls as Verbatim. Reuse the existing narrow parser;
// constness, safety, bounds and non-doc attributes remain part of the identity.
fn constant_implementation(sources: &Sources<'_>, recipe: TokenStream) -> Result<(), Refusal> {
    let unit = SourceUnit::SliceIndex;
    let supported: ConstTraitImpl =
        syn::parse2(recipe).map_err(|_error| Refusal::SourceShape(unit))?;
    let mut found = Vec::new();
    for item in &sources.file(unit)?.items {
        let syn::Item::Verbatim(body) = item else {
            continue;
        };
        let Ok(mut candidate) = syn::parse2::<ConstTraitImpl>(body.clone()) else {
            continue;
        };
        if tokens(&candidate.header) != tokens(&supported.header) {
            continue;
        }
        candidate
            .attrs
            .retain(|attribute| !supported_doc(attribute));
        let attrs = &candidate.attrs;
        let expected_attrs = &supported.attrs;
        same(unit, &quote!(#(#attrs)*), &quote!(#(#expected_attrs)*))?;
        found.push(candidate);
    }
    let [actual] = found.as_slice() else {
        return Err(Refusal::SourceShape(unit));
    };
    for member in supported.items {
        match member {
            syn::ImplItem::Fn(expected) => {
                let mut methods = actual.items.iter().filter_map(|item| {
                    if let syn::ImplItem::Fn(method) = item
                        && method.sig.ident == expected.sig.ident
                    {
                        Some(method)
                    } else {
                        None
                    }
                });
                same(
                    unit,
                    &project_method(unique(&mut methods, unit)?.clone()),
                    &project_method(expected),
                )?;
            }
            syn::ImplItem::Type(expected) => {
                let mut types = actual.items.iter().filter_map(|item| {
                    if let syn::ImplItem::Type(associated) = item
                        && associated.ident == expected.ident
                    {
                        Some(associated)
                    } else {
                        None
                    }
                });
                same(unit, unique(&mut types, unit)?, &expected)?;
            }
            _ => return Err(Refusal::SourceShape(unit)),
        }
    }
    Ok(())
}

fn slice_indices(sources: &Sources<'_>) -> Result<(), Refusal> {
    constant_implementation(
        sources,
        quote! {
            #[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
            #[rustc_const_unstable(feature = "const_index", issue = "143775")]
            const unsafe impl<T> SliceIndex<[T]> for usize {
                type Output = T;
                #[inline]
                fn index(self, slice: &[T]) -> &T { &(*slice)[self] }
            }
        },
    )?;
    constant_implementation(
        sources,
        quote! {
            #[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
            #[rustc_const_unstable(feature = "const_index", issue = "143775")]
            const unsafe impl<T> SliceIndex<[T]> for ops::RangeFrom<usize> {
                type Output = [T];
                #[inline]
                fn index(self, slice: &[T]) -> &[T] {
                    if self.start > slice.len() {
                        slice_index_fail(self.start, slice.len(), slice.len())
                    }
                    unsafe {
                        let new_len = crate::intrinsics::unchecked_sub(slice.len(), self.start);
                        &*get_offset_len_noubcheck(slice, self.start, new_len)
                    }
                }
            }
        },
    )
}

fn default_zero(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::Default;
    sources.exact(
        unit,
        syn::parse_quote! {
            macro_rules! default_impl {
                ($t:ty, $v:expr, $doc:tt) => {
                    #[stable(feature = "rust1", since = "1.0.0")]
                    #[rustc_const_unstable(feature = "const_default", issue = "143894")]
                    const impl Default for $t {
                        #[inline(always)]
                        #[doc = $doc]
                        fn default() -> $t { $v }
                    }
                };
            }
        },
    )?;
    let expected: syn::ItemMacro = syn::parse_quote! {
        default_impl! { u64, 0, "Returns the default value of `0`" }
    };
    let mut invocations = sources.file(unit)?.items.iter().filter_map(|item| {
        let syn::Item::Macro(invocation) = item else {
            return None;
        };
        if !invocation.mac.path.is_ident("default_impl") {
            return None;
        }
        let first = invocation.mac.tokens.clone().into_iter().next()?;
        if matches!(first, proc_macro2::TokenTree::Ident(name) if name == "u64") {
            Some(invocation)
        } else {
            None
        }
    });
    same(unit, unique(&mut invocations, unit)?, &expected)?;
    default_trait(sources)
}

fn default_trait(sources: &Sources<'_>) -> Result<(), Refusal> {
    let unit = SourceUnit::Default;
    // Check the concrete const header before parsing its single member; this
    // is not a const-to-ordinary-trait conversion or arbitrary macro expansion.
    let declaration = |input: ParseStream<'_>| -> syn::Result<syn::TraitItemFn> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let active: Vec<_> = attrs
            .iter()
            .filter(|attribute| !supported_doc(attribute))
            .collect();
        let supported = quote! {
            #[rustc_diagnostic_item = "Default"]
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_default", issue = "143894")]
        };
        if tokens(&quote!(#(#active)*)) != tokens(&supported) {
            return Err(input.error("unsupported Default attributes"));
        }
        input.parse::<syn::Token![pub]>()?;
        input.parse::<syn::Token![const]>()?;
        input.parse::<syn::Token![trait]>()?;
        let name: syn::Ident = input.parse()?;
        input.parse::<syn::Token![:]>()?;
        let bound: syn::Ident = input.parse()?;
        if name != "Default" || bound != "Sized" {
            return Err(input.error("unsupported Default header"));
        }
        let content;
        syn::braced!(content in input);
        let mut method: syn::TraitItemFn = content.parse()?;
        if !content.is_empty() {
            return Err(content.error("extra Default member"));
        }
        method.attrs.retain(|attribute| !supported_doc(attribute));
        Ok(method)
    };
    let mut declarations = Vec::new();
    for item in &sources.file(unit)?.items {
        if let syn::Item::Verbatim(body) = item
            && let Ok(method) = declaration.parse2(body.clone())
        {
            declarations.push(method);
        }
    }
    let [method] = declarations.as_slice() else {
        return Err(Refusal::SourceShape(unit));
    };
    let expected: syn::TraitItemFn = syn::parse_quote! {
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_diagnostic_item = "default_fn"]
        fn default() -> Self;
    };
    same(unit, method, &expected)
}

#[cfg(test)]
pub(in crate::schema_validation::work) mod fixture;

#[cfg(test)]
mod tests {
    use super::fixture::{
        SOURCES, changed, digest, feature_slot, fixture, implementation_body, row, source_path,
        syntax, trait_body,
    };
    use super::{
        Dispatch, FeatureClause, Refusal, ScalarContract, ScalarData, SchemaInputLimits,
        SourceUnit, derive_scalar_sources,
    };
    use crate::schema_validation::work::{
        INPUT, JSON_SIZE, NATIVE, SchemaInputError, SchemaInputStats, measure_schema_input,
    };
    use auctioneer_runtime::serialized_json_bytes;
    use quote::{ToTokens as _, quote};
    use serde_json::{Value, json};
    use std::collections::BTreeSet;

    fn required_sources() -> BTreeSet<SourceUnit> {
        BTreeSet::from([
            SourceUnit::Native,
            SourceUnit::Input,
            SourceUnit::JsonSize,
            SourceUnit::SerdeFacade,
            SourceUnit::SerdeCoreRoot,
            SourceUnit::SerdeCoreExports,
            SourceUnit::SerdeTraits,
            SourceUnit::SerdeStr,
            SourceUnit::JsonRoot,
            SourceUnit::JsonIo,
            SourceUnit::JsonSerializer,
            SourceUnit::JsonValue,
            SourceUnit::JsonValueSerialize,
            SourceUnit::JsonNumber,
            SourceUnit::IoWrite,
            SourceUnit::IoForward,
            SourceUnit::StrBytes,
            SourceUnit::Slice,
            SourceUnit::SliceIndex,
            SourceUnit::Default,
        ])
    }

    fn before(data: &ScalarData) {
        assert!(data.roots.workspace.is_absolute());
        assert!(data.roots.registry.is_absolute());
        assert!(data.roots.library.is_absolute());
        assert_eq!(data.sources.len(), 20);
        assert_eq!(
            data.sources
                .iter()
                .map(|source| source.unit)
                .collect::<BTreeSet<_>>(),
            required_sources()
        );
        for (unit, root, relative) in SOURCES {
            let source = row(data, unit);
            assert_eq!(source.path, source_path(&data.roots, root, relative));
            assert_eq!(source.before, source.after);
            assert_eq!(source.sha256, digest(&source.before));
            assert!(!syntax(&source.before).items.is_empty());
        }
        assert_eq!(row(data, SourceUnit::Native).before, NATIVE);
        assert_eq!(row(data, SourceUnit::Input).before, INPUT);
        assert_eq!(row(data, SourceUnit::JsonSize).before, JSON_SIZE);
        assert_eq!(
            (
                data.features.serde_std,
                data.features.serde_core_std,
                data.features.json_std
            ),
            (Some(true), Some(true), Some(true))
        );
        assert_eq!(
            (data.features.arbitrary_precision, data.features.docsrs),
            (Some(false), Some(false))
        );
        assert_eq!(data.pointer_width, Some(64));
        assert_eq!(data.dispatch, Dispatch::NativeScalars);
        assert_str_dispatch(data);
    }

    fn assert_str_dispatch(data: &ScalarData) {
        let mut file = syntax(&row(data, SourceUnit::SerdeStr).before);
        let body = implementation_body(&mut file, &syn::parse_quote!(str), "serialize");
        let expected: syn::Block = syn::parse_quote!({ serializer.serialize_str(self) });
        assert_eq!(
            body.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
        let mut serializer = syntax(&row(data, SourceUnit::JsonSerializer).before);
        let begin = trait_body(&mut serializer, "Formatter", "begin_string");
        let quote_body: syn::Block = syn::parse_quote!({ writer.write_all(b"\"") });
        assert_eq!(
            begin.to_token_stream().to_string(),
            quote_body.to_token_stream().to_string()
        );
        let finish = trait_body(&mut serializer, "Formatter", "end_string");
        assert_eq!(
            finish.to_token_stream().to_string(),
            quote_body.to_token_stream().to_string()
        );
    }

    fn expected(raw_upper: u64) -> ScalarContract {
        ScalarContract {
            raw_utf8_with_quotes_upper: raw_upper,
            string_virtual_bytes_upper: 393_206,
            writes_per_nonempty_request_upper: 1,
            writes_per_empty_request_upper: 0,
            interrupted_retries_upper: 0,
            required_sources: required_sources(),
            unresolved: vec![
                "actual registry/feature/artifact join",
                "compiler/builtin and primitive semantic identity",
                "itoa/zmij safety, termination and ordinary work",
                "serde/string/slice ordinary helper work",
                "caller-owned map height/navigation",
                "ordinary error/unwind cleanup",
                "matched helper frame bytes",
            ],
            library_artifact_binding: None,
        }
    }

    fn pending(contract: &ScalarContract) {
        assert_eq!(contract.required_sources, required_sources());
        assert_eq!(contract.library_artifact_binding, None);
        assert_eq!(
            contract.unresolved,
            [
                "actual registry/feature/artifact join",
                "compiler/builtin and primitive semantic identity",
                "itoa/zmij safety, termination and ordinary work",
                "serde/string/slice ordinary helper work",
                "caller-owned map height/navigation",
                "ordinary error/unwind cleanup",
                "matched helper frame bytes",
            ]
        );
    }

    fn refuse_shape(data: &ScalarData, limits: SchemaInputLimits, unit: SourceUnit) {
        for attempts in [1_u64, 0_u64] {
            assert_eq!(
                derive_scalar_sources(data, limits, attempts),
                Err(Refusal::SourceShape(unit))
            );
        }
    }

    fn assert_string_source_mutations(data: &ScalarData, limits: SchemaInputLimits) {
        let unframed = changed(data, SourceUnit::JsonSerializer, |file| {
            let begin = trait_body(file, "Formatter", "begin_string");
            let original: syn::Block = syn::parse_quote!({ writer.write_all(b"\"") });
            assert_eq!(
                begin.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *begin = syn::parse_quote!({ writer.write_all(b"") });
        });
        refuse_shape(&unframed, limits, SourceUnit::JsonSerializer);
        let redirected = changed(data, SourceUnit::SerdeStr, |file| {
            let body = implementation_body(file, &syn::parse_quote!(str), "serialize");
            let original: syn::Block = syn::parse_quote!({ serializer.serialize_str(self) });
            assert_eq!(
                body.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *body = syn::parse_quote!({ serializer.collect_str(self) });
        });
        refuse_shape(&redirected, limits, SourceUnit::SerdeStr);
        let escaped = changed(data, SourceUnit::JsonSerializer, |file| {
            let mut hits = 0_usize;
            for item in &mut file.items {
                if let syn::Item::Static(table) = item
                    && table.ident == "ESCAPE"
                {
                    let syn::Expr::Array(values) = table.expr.as_mut() else {
                        panic!("actual byte escape table");
                    };
                    assert_eq!(values.elems.len(), 256);
                    let first = values.elems.first_mut().unwrap();
                    assert_eq!(first.to_token_stream().to_string(), quote!(UU).to_string());
                    *first = syn::parse_quote!(__);
                    hits = hits.checked_add(1).unwrap();
                }
            }
            assert_eq!(hits, 1);
        });
        refuse_shape(&escaped, limits, SourceUnit::JsonSerializer);
    }

    fn assert_string_witnesses(limits: SchemaInputLimits) {
        let raw = "a".repeat(65_535);
        assert_eq!(raw.len(), 65_535);
        assert_eq!(
            measure_schema_input(&Value::String(raw), limits),
            Err(SchemaInputError::Bytes)
        );
        let ascii = "a".repeat(65_534);
        assert_eq!(serialized_json_bytes(ascii.as_str()).unwrap(), 65_536);
        let nul = "\0".repeat(65_534);
        assert_eq!(nul.len(), 65_534);
        assert_eq!(serialized_json_bytes(nul.as_str()).unwrap(), 393_206);
        // Virtual bytes only; neither this witness nor the contract prices scans.
        assert_eq!(
            measure_schema_input(&Value::String(nul), limits),
            Err(SchemaInputError::Bytes)
        );
        let mixed = json!({"\u{00e9}\"\n":"\u{1f642}\\\t\u{0000}"});
        assert_eq!(
            measure_schema_input(&mixed, limits),
            Ok(SchemaInputStats {
                serialized_bytes: 27,
                nodes: 3,
                max_depth: 1,
            })
        );
        for (text, literal) in [("", 2_u64), ("a", 3), ("\"", 4), ("\0", 8), ("\u{00e9}", 4)] {
            assert_eq!(serialized_json_bytes(text).unwrap(), literal);
        }
    }

    #[test]
    fn string_framing_bounds_rejected_prefixes() {
        let (data, limits) = fixture();
        before(&data);
        let result = derive_scalar_sources(&data, limits, 3);
        assert_eq!(result, Ok(expected(196_608)));
        let contract = result.unwrap();
        assert_eq!(contract.raw_utf8_with_quotes_upper, 196_608);
        assert_eq!(contract.string_virtual_bytes_upper, 393_206);
        pending(&contract);
        assert_eq!(
            derive_scalar_sources(&data, limits, 1),
            Ok(expected(65_536))
        );
        assert_eq!(derive_scalar_sources(&data, limits, 0), Ok(expected(0)));
        assert_string_witnesses(limits);
        assert_string_source_mutations(&data, limits);
        assert_eq!(
            derive_scalar_sources(&data, limits, u64::MAX),
            Err(Refusal::Overflow)
        );
    }

    fn assert_feature_and_width_controls(data: &ScalarData, limits: SchemaInputLimits) {
        for clause in [
            FeatureClause::SerdeStd,
            FeatureClause::SerdeCoreStd,
            FeatureClause::JsonStd,
            FeatureClause::ArbitraryPrecision,
            FeatureClause::Docsrs,
        ] {
            let mut missing = data.clone();
            let original = feature_slot(&mut missing.features, clause).take().unwrap();
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&missing, limits, attempts),
                    Err(Refusal::MissingFeature(clause))
                );
            }
            let mut incompatible = data.clone();
            let slot = feature_slot(&mut incompatible.features, clause);
            assert_eq!(slot.replace(!original), Some(original));
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&incompatible, limits, attempts),
                    Err(Refusal::FeatureMismatch(clause))
                );
            }
        }
        let mut absent_width = data.clone();
        assert_eq!(absent_width.pointer_width.take(), Some(64));
        let mut wide = data.clone();
        assert_eq!(wide.pointer_width.replace(128), Some(64));
        for attempts in [1_u64, 0_u64] {
            assert_eq!(
                derive_scalar_sources(&absent_width, limits, attempts),
                Err(Refusal::MissingWidth)
            );
            assert_eq!(
                derive_scalar_sources(&wide, limits, attempts),
                Err(Refusal::UnsupportedWidth(128))
            );
        }
        for dispatch in [
            Dispatch::GenericSerialize,
            Dispatch::OtherWriter,
            Dispatch::PrettyFormatter,
        ] {
            let mut unsupported = data.clone();
            assert_eq!(unsupported.dispatch, Dispatch::NativeScalars);
            unsupported.dispatch = dispatch;
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&unsupported, limits, attempts),
                    Err(Refusal::UnsupportedDispatch)
                );
            }
        }
    }

    fn assert_writer_source_mutations(data: &ScalarData, limits: SchemaInputLimits) {
        let skipped = changed(data, SourceUnit::IoWrite, |file| {
            let body = trait_body(file, "Write", "write_all");
            let first = body.stmts.first_mut().unwrap();
            let syn::Stmt::Expr(syn::Expr::While(repeated), None) = first else {
                panic!("actual Write::write_all loop");
            };
            assert_eq!(
                repeated.cond.to_token_stream().to_string(),
                quote!(!buf.is_empty()).to_string()
            );
            repeated.cond = Box::new(syn::parse_quote!(false));
        });
        refuse_shape(&skipped, limits, SourceUnit::IoWrite);
        let short = changed(data, SourceUnit::JsonSize, |file| {
            let body = implementation_body(file, &syn::parse_quote!(Counter), "write");
            let tail = body.stmts.last_mut().unwrap();
            let original = syn::Stmt::Expr(syn::parse_quote!(Ok(buf.len())), None);
            assert_eq!(
                tail.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *tail = syn::Stmt::Expr(syn::parse_quote!(Ok(buf.len().saturating_sub(1))), None);
        });
        refuse_shape(&short, limits, SourceUnit::JsonSize);
        let no_forward = changed(data, SourceUnit::IoForward, |file| {
            let body = implementation_body(file, &syn::parse_quote!(&mut W), "write_all");
            let original: syn::Block = syn::parse_quote!({ (**self).write_all(buf) });
            assert_eq!(
                body.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *body = syn::parse_quote!({ Ok(()) });
        });
        refuse_shape(&no_forward, limits, SourceUnit::IoForward);
    }

    #[test]
    fn concrete_counter_protocol_requires_scalar_dispatch() {
        let (data, limits) = fixture();
        before(&data);
        let result = derive_scalar_sources(&data, limits, 1);
        assert_eq!(result, Ok(expected(65_536)));
        let contract = result.unwrap();
        assert_eq!(contract.writes_per_nonempty_request_upper, 1);
        assert_eq!(contract.writes_per_empty_request_upper, 0);
        assert_eq!(contract.interrupted_retries_upper, 0);
        pending(&contract);
        let mut width_32 = data.clone();
        assert_eq!(width_32.pointer_width.replace(32), Some(64));
        assert_eq!(
            derive_scalar_sources(&width_32, limits, 1),
            Ok(expected(65_536))
        );
        // The protocol theorem still needs its source duties when no request
        // occurs; 1/0/0 is per request, not the zero-attempt aggregate.
        assert_eq!(derive_scalar_sources(&data, limits, 0), Ok(expected(0)));
        assert_feature_and_width_controls(&data, limits);
        assert_writer_source_mutations(&data, limits);
    }

    fn assert_missing_duplicate_and_changed(data: &ScalarData, limits: SchemaInputLimits) {
        for unit in required_sources() {
            let mut missing = data.clone();
            let index = missing
                .sources
                .iter()
                .position(|source| source.unit == unit)
                .unwrap();
            assert_eq!(missing.sources.remove(index).unit, unit);
            let mut duplicate = data.clone();
            duplicate.sources.push(row(data, unit).clone());
            let mut drift = data.clone();
            let source = drift
                .sources
                .iter_mut()
                .find(|source| source.unit == unit)
                .unwrap();
            source.after.push(b'\n');
            assert_ne!(source.before, source.after);
            let mut retargeted = data.clone();
            let target = retargeted
                .sources
                .iter_mut()
                .find(|source| source.unit == unit)
                .unwrap();
            let original_path = target.path.clone();
            target.path.set_file_name("fake_changed_scalar_source.rs");
            assert_ne!(target.path, original_path);
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&missing, limits, attempts),
                    Err(Refusal::MissingSource(unit))
                );
                assert_eq!(
                    derive_scalar_sources(&duplicate, limits, attempts),
                    Err(Refusal::DuplicateSource(unit))
                );
                assert_eq!(
                    derive_scalar_sources(&drift, limits, attempts),
                    Err(Refusal::SourceChanged(unit))
                );
                assert_eq!(
                    derive_scalar_sources(&retargeted, limits, attempts),
                    Err(Refusal::SourcePath(unit))
                );
            }
            let active_attribute = changed(data, unit, |file| {
                let attribute: syn::Attribute =
                    syn::parse_quote!(#![fake_unapproved_scalar_attribute]);
                file.attrs.push(attribute);
            });
            refuse_shape(&active_attribute, limits, unit);
        }
    }

    fn assert_number_type_mutation(data: &ScalarData, limits: SchemaInputLimits) {
        let changed_number = changed(data, SourceUnit::JsonNumber, |file| {
            let mut hits = 0_usize;
            for item in &mut file.items {
                if let syn::Item::Enum(number) = item
                    && number.ident == "N"
                {
                    for variant in &mut number.variants {
                        if variant.ident == "PosInt" {
                            let syn::Fields::Unnamed(fields) = &mut variant.fields else {
                                panic!("actual positive integer tuple");
                            };
                            assert_eq!(fields.unnamed.len(), 1);
                            let field = fields.unnamed.first_mut().unwrap();
                            assert_eq!(
                                field.ty.to_token_stream().to_string(),
                                quote!(u64).to_string()
                            );
                            field.ty = syn::parse_quote!(u128);
                            hits = hits.checked_add(1).unwrap();
                        }
                    }
                }
            }
            assert_eq!(hits, 1);
        });
        refuse_shape(&changed_number, limits, SourceUnit::JsonNumber);
    }

    #[test]
    fn missing_and_rehashed_scalar_sources_refuse_at_zero_attempts() {
        let (data, limits) = fixture();
        before(&data);
        assert_eq!(derive_scalar_sources(&data, limits, 0), Ok(expected(0)));
        assert_missing_duplicate_and_changed(&data, limits);
        assert_number_type_mutation(&data, limits);
    }
}

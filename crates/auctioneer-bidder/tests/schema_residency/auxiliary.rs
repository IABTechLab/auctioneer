//! Closed auxiliary source grammar; no byte costs or serving qualification.

use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::str;
use syn::{
    Expr, Item, Stmt, Token,
    parse::{Parse, ParseStream},
    visit_mut::{self, VisitMut},
};

const REFUSAL: &str = "unsupported captured staging syntax";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Status {
    Absent,
    Complete,
}

impl Status {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "Absent",
            Self::Complete => "Complete",
        }
    }
}

pub(super) struct Classified {
    pub status: Status,
    pub roots: Vec<Value>,
    pub dispatch_edges: Vec<Value>,
}

struct Row {
    ordinal: usize,
    entry: syn::Ident,
    marker: syn::Ident,
    cache: syn::Ident,
    owner: String,
}

fn error() -> String {
    REFUSAL.to_owned()
}
// Only parsed syn nodes may enter the spacing-insensitive comparison.
// Macro predicates are parsed by Assertion/Required before their comparison.
trait ParsedFragment: ToTokens {}
macro_rules! parsed_fragments {
    ($($fragment:ty),* $(,)?) => { $(impl ParsedFragment for $fragment {})* };
}
parsed_fragments!(
    syn::ItemStruct,
    syn::ItemUse,
    syn::ItemConst,
    syn::Visibility,
    syn::Path,
    syn::Type,
    syn::Pat,
    syn::Expr,
    syn::Stmt,
    syn::Signature,
    syn::Attribute,
    Box<syn::Type>,
    Box<syn::Pat>,
);
impl<T: ParsedFragment + ?Sized> ParsedFragment for &T {}

fn same(value: &impl ParsedFragment, expected: TokenStream) -> bool {
    token_trees_equal(value.to_token_stream(), expected)
}

fn token_trees_equal(left: TokenStream, right: TokenStream) -> bool {
    let mut left_tokens = left.into_iter();
    let mut right_tokens = right.into_iter();
    loop {
        let equal = match (left_tokens.next(), right_tokens.next()) {
            (None, None) => return true,
            (Some(TokenTree::Group(a)), Some(TokenTree::Group(b))) => {
                a.delimiter() == b.delimiter() && token_trees_equal(a.stream(), b.stream())
            }
            (Some(TokenTree::Ident(a)), Some(TokenTree::Ident(b))) => a == b,
            (Some(TokenTree::Punct(a)), Some(TokenTree::Punct(b))) => {
                a.as_char() == b.as_char() && (a.as_char() == '>' || a.spacing() == b.spacing())
            }
            (Some(TokenTree::Literal(a)), Some(TokenTree::Literal(b))) => {
                a.to_string() == b.to_string()
            }
            _ => false,
        };
        if !equal {
            return false;
        }
    }
}

fn path(expr: &Expr, names: &[&str]) -> bool {
    matches!(expr, Expr::Path(value) if value.attrs.is_empty()
        && value.qself.is_none() && value.path.leading_colon.is_none()
        && value.path.segments.len() == names.len()
        && value.path.segments.iter().zip(names).all(|(part, name)|
            part.ident == *name && part.arguments.is_empty()))
}

fn call<'a>(expr: &'a Expr, names: &[&str], arity: usize) -> Option<&'a syn::ExprCall> {
    let Expr::Call(value) = expr else {
        return None;
    };
    (value.attrs.is_empty() && path(&value.func, names) && value.args.len() == arity)
        .then_some(value)
}

fn string(expr: &Expr) -> Option<String> {
    let Expr::Lit(value) = expr else {
        return None;
    };
    let syn::Lit::Str(text) = &value.lit else {
        return None;
    };
    value.attrs.is_empty().then(|| text.value())
}

fn bytes(expr: &Expr) -> Option<Vec<u8>> {
    let Expr::Unary(value) = expr else {
        return None;
    };
    if !value.attrs.is_empty() || !matches!(&value.op, syn::UnOp::Deref(_)) {
        return None;
    }
    let Expr::Lit(literal) = value.expr.as_ref() else {
        return None;
    };
    if !literal.attrs.is_empty() {
        return None;
    }
    let syn::Lit::ByteStr(text) = &literal.lit else {
        return None;
    };
    Some(text.value())
}

fn number(expr: &Expr) -> Option<usize> {
    let Expr::Lit(value) = expr else {
        return None;
    };
    if !value.attrs.is_empty() {
        return None;
    }
    let syn::Lit::Int(integer) = &value.lit else {
        return None;
    };
    if integer.suffix().is_empty() {
        integer.base10_parse().ok()
    } else {
        None
    }
}

fn registry_type(item: &syn::ItemStruct) -> bool {
    same(
        item,
        quote! {
            #[repr(C)]
            struct StagedValidator<C> {
                magic: [u8; 16], identity: [u8; 64], owner: [u8; 64], owner_len: u32,
                validate: fn(&serde_json::Value, &mut C) -> bool,
                new_cache: fn() -> Option<C>, initialize: fn() -> bool,
            }
        },
    )
}

fn retention_row(
    item: &syn::ItemStatic,
    bindings: &BTreeMap<String, syn::Ident>,
    identity: &str,
) -> Result<Row, String> {
    if !item.attrs.is_empty()
        || !same(&item.vis, quote!())
        || !item.mutability.to_token_stream().is_empty()
    {
        return Err(error());
    }
    let name = item.ident.to_string();
    let ordinal = name
        .strip_prefix("STAGED_VALIDATOR_")
        .ok_or_else(error)?
        .parse::<usize>()
        .map_err(|_error| error())?;
    if name != format!("STAGED_VALIDATOR_{ordinal}") {
        return Err(error());
    }
    let Expr::Struct(value) = item.expr.as_ref() else {
        return Err(error());
    };
    if !value.attrs.is_empty()
        || value.qself.is_some()
        || value.rest.is_some()
        || !same(&value.path, quote!(StagedValidator))
    {
        return Err(error());
    }
    let mut fields = BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(field_name) = &field.member else {
            return Err(error());
        };
        if !field.attrs.is_empty()
            || field.colon_token.is_none()
            || fields.insert(field_name.to_string(), &field.expr).is_some()
        {
            return Err(error());
        }
    }
    if fields.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != BTreeSet::from([
            "magic",
            "identity",
            "owner",
            "owner_len",
            "validate",
            "new_cache",
            "initialize",
        ])
    {
        return Err(error());
    }
    let field = |key: &str| fields.get(key).copied().ok_or_else(error);
    let owner_bytes = bytes(field("owner")?).ok_or_else(error)?;
    let length = number(field("owner_len")?).ok_or_else(error)?;
    let (owner_prefix, padding) = owner_bytes.split_at_checked(length).ok_or_else(error)?;
    if !(1..=64).contains(&length)
        || owner_bytes.len() != 64
        || padding.iter().any(|byte| *byte != 0)
    {
        return Err(error());
    }
    let owner = str::from_utf8(owner_prefix)
        .map_err(|_error| error())?
        .to_owned();
    let marker = bindings.get(&owner).ok_or_else(error)?.clone();
    let marker_text = marker.to_string();
    let cache = syn::parse_str::<syn::Ident>(&format!("{marker_text}SearchCache"))
        .map_err(|_error| error())?;
    if !same(&item.ty, quote!(StagedValidator<#cache>))
        || bytes(field("magic")?).as_deref() != Some(b"AUCTIONEERSCHEMA".as_slice())
        || bytes(field("identity")?).as_deref() != Some(identity.as_bytes())
        || !path(field("validate")?, &[&marker_text, "is_valid"])
        || !path(field("new_cache")?, &[&marker_text, "new_search_cache"])
        || !path(field("initialize")?, &[&marker_text, "initialize_regexes"])
    {
        return Err(error());
    }
    Ok(Row {
        ordinal,
        entry: item.ident.clone(),
        marker,
        cache,
        owner,
    })
}

struct Assertion {
    condition: Expr,
    message: syn::LitStr,
}
impl Parse for Assertion {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let condition = input.parse()?;
        input.parse::<Token![,]>()?;
        Ok(Self {
            condition,
            message: input.parse()?,
        })
    }
}

fn statement_macro(statement: &Stmt) -> Option<&syn::Macro> {
    if let Stmt::Macro(value) = statement
        && value.attrs.is_empty()
        && value.semi_token.is_some()
    {
        Some(&value.mac)
    } else if let Stmt::Expr(Expr::Macro(value), Some(_)) = statement
        && value.attrs.is_empty()
    {
        Some(&value.mac)
    } else {
        None
    }
}

fn abi_predicates(cache: &syn::Ident) -> [(TokenStream, &'static str); 12_usize] {
    [
        (
            quote!(size_of::<fn(&serde_json::Value, &mut #cache) -> bool>() == word),
            "validation function pointer width",
        ),
        (
            quote!(size_of::<fn() -> Option<#cache>>() == word),
            "cache function pointer width",
        ),
        (
            quote!(size_of::<fn() -> bool>() == word),
            "initialization function pointer width",
        ),
        (
            quote!(align_of::<StagedValidator<#cache>>() == word),
            "schema inspection row alignment",
        ),
        (
            quote!(offset_of!(StagedValidator<#cache>, magic) == 0),
            "schema inspection magic offset",
        ),
        (
            quote!(offset_of!(StagedValidator<#cache>, identity) == 16),
            "schema inspection identity offset",
        ),
        (
            quote!(offset_of!(StagedValidator<#cache>, owner) == 80),
            "schema inspection owner offset",
        ),
        (
            quote!(offset_of!(StagedValidator<#cache>, owner_len) == 144),
            "schema inspection owner length offset",
        ),
        (
            quote!(offset_of!(StagedValidator<#cache>, validate) == validate),
            "schema inspection validation pointer offset",
        ),
        (
            quote!(offset_of!(StagedValidator<#cache>, new_cache) == new_cache),
            "schema inspection cache pointer offset",
        ),
        (
            quote!(offset_of!(StagedValidator<#cache>, initialize) == initialize),
            "schema inspection initialization pointer offset",
        ),
        (
            quote!(size_of::<StagedValidator<#cache>>() == row_bytes),
            "schema inspection row size",
        ),
    ]
}

fn abi(item: &syn::ItemConst, cache: &syn::Ident) -> bool {
    let initializer = &item.expr;
    if !same(item, quote!(const _: () = #initializer;)) {
        return false;
    }
    if !item.attrs.is_empty()
        || item.ident != "_"
        || !same(&item.vis, quote!())
        || !same(&item.ty, quote!(()))
    {
        return false;
    }
    let Expr::Block(value) = item.expr.as_ref() else {
        return false;
    };
    if !value.attrs.is_empty() || value.label.is_some() {
        return false;
    }
    let predicates = abi_predicates(cache);
    let [imports, word, offsets, assertions @ ..] = value.block.stmts.as_slice() else {
        return false;
    };
    if assertions.len() != predicates.len()
        || !same(
            imports,
            quote!(
                use std::mem::{align_of, offset_of, size_of};
            ),
        )
        || !same(word, quote!(let word = size_of::<usize>();))
        || !same(
            offsets,
            quote! {
                let (validate, new_cache, initialize, row_bytes) = match word {
                    4 => (148, 152, 156, 160),
                    8 => (152, 160, 168, 176),
                    _ => panic!("unsupported schema inspection pointer width"),
                };
            },
        )
    {
        return false;
    }
    assertions
        .iter()
        .zip(predicates)
        .all(|(statement, (predicate, message))| {
            let Some(mac) = statement_macro(statement) else {
                return false;
            };
            if !mac.path.is_ident("assert")
                || !matches!(&mac.delimiter, syn::MacroDelimiter::Paren(_))
            {
                return false;
            }
            let Ok(assertion) = syn::parse2::<Assertion>(mac.tokens.clone()) else {
                return false;
            };
            same(&assertion.condition, predicate) && assertion.message.value() == message
        })
}

// Reject attributes and nested items even inside otherwise allowed arguments.
fn plain_body(block: &syn::Block) -> bool {
    struct Plain(bool);
    impl VisitMut for Plain {
        fn visit_attribute_mut(&mut self, _attribute: &mut syn::Attribute) {
            self.0 = false;
        }
        fn visit_item_mut(&mut self, _item: &mut Item) {
            self.0 = false;
        }
    }
    let mut inspected = block.clone();
    let mut plain = Plain(true);
    visit_mut::visit_block_mut(&mut plain, &mut inspected);
    plain.0
}

fn tail(block: &syn::Block) -> Option<&Expr> {
    match block.stmts.as_slice() {
        [Stmt::Expr(expr, None)] => Some(expr),
        _ => None,
    }
}

fn patterns(pattern: &syn::Pat, output: &mut Vec<String>) -> bool {
    if let syn::Pat::Lit(value) = pattern
        && value.attrs.is_empty()
    {
        if let syn::Lit::Str(text) = &value.lit {
            output.push(text.value());
            true
        } else {
            false
        }
    } else if let syn::Pat::Or(value) = pattern
        && value.attrs.is_empty()
        && value.leading_vert.is_none()
    {
        value.cases.iter().all(|case| patterns(case, output))
    } else {
        false
    }
}

fn named_match<'a>(expr: &'a Expr, selector: &str) -> Option<&'a syn::ExprMatch> {
    let Expr::Match(value) = expr else {
        return None;
    };
    (value.attrs.is_empty()
        && path(&value.expr, &[selector])
        && value
            .arms
            .iter()
            .all(|arm| arm.attrs.is_empty() && !matches!(&arm.pat, syn::Pat::Guard(_)))
        && value.arms.last().is_some_and(|arm| {
            matches!(&arm.pat, syn::Pat::Wild(pattern) if pattern.attrs.is_empty())
                && path(&arm.body, &["None"])
        }))
    .then_some(value)
}

struct Required {
    selector: syn::Ident,
    pattern: syn::Pat,
}
impl Parse for Required {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let selector = input.parse()?;
        input.parse::<Token![,]>()?;
        Ok(Self {
            selector,
            pattern: input.call(syn::Pat::parse_multi_with_leading_vert)?,
        })
    }
}

fn edge(edges: &mut Vec<Value>, function: &str, target: &str, role: &str) {
    edges.push(json!({"from":function,"to":target,"role":role,"ownership":"callee-owned"}));
}

fn initialization(expr: &Expr, output: &mut Vec<String>) -> bool {
    if let Expr::Binary(value) = expr
        && value.attrs.is_empty()
        && matches!(&value.op, syn::BinOp::And(_))
    {
        initialization(&value.left, output) && initialization(&value.right, output)
    } else if let Expr::Call(value) = expr
        && value.attrs.is_empty()
        && value.args.is_empty()
    {
        let Expr::Path(name) = value.func.as_ref() else {
            return false;
        };
        if !name.attrs.is_empty()
            || name.qself.is_some()
            || name.path.leading_colon.is_some()
            || name.path.segments.len() != 2
        {
            return false;
        }
        let segments = name.path.segments.iter().collect::<Vec<_>>();
        let [owner, method] = segments.as_slice() else {
            return false;
        };
        if segments.iter().any(|part| !part.arguments.is_empty())
            || method.ident != "initialize_regexes"
        {
            return false;
        }
        output.push(owner.ident.to_string());
        true
    } else {
        false
    }
}

fn retain_dispatch(item: &syn::ItemFn, rows: &[Row], edges: &mut Vec<Value>) -> Result<(), String> {
    if !item.attrs.is_empty() || !same(&item.sig, quote!(fn retain_for_inspection())) {
        return Err(error());
    }
    let [Stmt::Expr(expr, Some(_))] = item.block.stmts.as_slice() else {
        return Err(error());
    };
    let invocation = call(expr, &["black_box"], 1).ok_or_else(error)?;
    let argument = invocation.args.first().ok_or_else(error)?;
    if rows.is_empty() {
        if !matches!(argument, Expr::Paren(value) if value.attrs.is_empty()
            && path(&value.expr, &["STAGING_INPUT_ID"]))
        {
            return Err(error());
        }
    } else {
        let Expr::Tuple(tuple) = argument else {
            return Err(error());
        };
        if !tuple.attrs.is_empty()
            || tuple.elems.len() != rows.len().checked_add(1_usize).ok_or_else(error)?
        {
            return Err(error());
        }
        for (reference, row) in tuple.elems.iter().zip(rows) {
            let Expr::Reference(value) = reference else {
                return Err(error());
            };
            if !value.attrs.is_empty()
                || value.mutability.is_some()
                || !path(&value.expr, &[&row.entry.to_string()])
            {
                return Err(error());
            }
            edge(
                edges,
                "retain_for_inspection",
                &row.entry.to_string(),
                "inspection-reference",
            );
        }
        if !tuple
            .elems
            .last()
            .is_some_and(|identity| path(identity, &["STAGING_INPUT_ID"]))
        {
            return Err(error());
        }
    }
    Ok(())
}

fn initialize_dispatch(
    item: &syn::ItemFn,
    expr: &Expr,
    rows: &[Row],
    edges: &mut Vec<Value>,
) -> Result<(), String> {
    if !same(&item.sig, quote!(fn initialize_native() -> bool)) {
        return Err(error());
    }
    let mut calls = Vec::new();
    if rows.is_empty() && !same(expr, quote!(true)) {
        return Err(error());
    }
    if !rows.is_empty()
        && (!initialization(expr, &mut calls)
            || calls
                != rows
                    .iter()
                    .map(|row| row.marker.to_string())
                    .collect::<Vec<_>>())
    {
        return Err(error());
    }
    for marker in calls {
        edge(edges, "initialize_native", &marker, "initialize-regexes");
    }
    Ok(())
}

fn validate_dispatch(
    item: &syn::ItemFn,
    expr: &Expr,
    rows: &[Row],
    edges: &mut Vec<Value>,
) -> Result<(), String> {
    if !same(
        &item.sig,
        quote!(
            fn validate_native(owner: &str, value: &serde_json::Value) -> Option<bool>
        ),
    ) {
        return Err(error());
    }
    let dispatch = named_match(expr, "owner").ok_or_else(error)?;
    let mut seen = BTreeSet::new();
    for arm in dispatch.arms.split_last().ok_or_else(error)?.1 {
        let mut owners = Vec::new();
        if !patterns(&arm.pat, &mut owners) || owners.len() != 1 {
            return Err(error());
        }
        let owner = owners.first().ok_or_else(error)?;
        let row = rows
            .iter()
            .find(|row| &row.owner == owner)
            .ok_or_else(error)?;
        if !seen.insert(owner.clone()) {
            return Err(error());
        }
        let Expr::Block(block) = arm.body.as_ref() else {
            return Err(error());
        };
        if !block.attrs.is_empty() || block.label.is_some() {
            return Err(error());
        }
        let [Stmt::Local(local), Stmt::Expr(result, None)] = block.block.stmts.as_slice() else {
            return Err(error());
        };
        let marker = row.marker.to_string();
        if !local.attrs.is_empty() || !same(&local.pat, quote!(mut cache)) {
            return Err(error());
        }
        let initial = local.init.as_ref().ok_or_else(error)?;
        let Expr::Try(attempt) = initial.expr.as_ref() else {
            return Err(error());
        };
        if initial.diverge.is_some()
            || !attempt.attrs.is_empty()
            || call(&attempt.expr, &[&marker, "new_search_cache"], 0).is_none()
        {
            return Err(error());
        }
        let some = call(result, &["Some"], 1).ok_or_else(error)?;
        let validate = call(
            some.args.first().ok_or_else(error)?,
            &[&marker, "is_valid"],
            2,
        )
        .ok_or_else(error)?;
        let mut arguments = validate.args.iter();
        if !arguments
            .next()
            .is_some_and(|argument| path(argument, &["value"]))
            || !arguments
                .next()
                .is_some_and(|argument| same(argument, quote!(&mut cache)))
        {
            return Err(error());
        }
        edge(
            edges,
            "validate_native",
            &marker,
            "validation-call-cache-factory",
        );
        edge(
            edges,
            "validate_native",
            &marker,
            "is-valid-with-call-cache",
        );
    }
    if seen != rows.iter().map(|row| row.owner.clone()).collect() {
        return Err(error());
    }
    Ok(())
}

fn owner_dispatch(
    item: &syn::ItemFn,
    expr: &Expr,
    members: &BTreeMap<String, Option<String>>,
    edges: &mut Vec<Value>,
) -> Result<(), String> {
    if path(expr, &["None"]) {
        if !same(
            &item.sig,
            quote!(fn native_owner(_: &str) -> Option<&'static str>),
        ) {
            return Err(error());
        }
        return Ok(());
    }
    if !same(
        &item.sig,
        quote!(fn native_owner(name: &str) -> Option<&'static str>),
    ) {
        return Err(error());
    }
    let dispatch = named_match(expr, "name").ok_or_else(error)?;
    let mut seen = BTreeSet::new();
    for arm in dispatch.arms.split_last().ok_or_else(error)?.1 {
        let some = call(&arm.body, &["Some"], 1).ok_or_else(error)?;
        let owner = string(some.args.first().ok_or_else(error)?).ok_or_else(error)?;
        let mut names = Vec::new();
        if !patterns(&arm.pat, &mut names) || names.is_empty() {
            return Err(error());
        }
        for member in names {
            if !seen.insert(member.clone()) || members.get(&member) != Some(&Some(owner.clone())) {
                return Err(error());
            }
            edges.push(json!({"from":"native_owner","member":member,"owner":owner,
                "role":"native-owner-lookup","ownership":"literal-only"}));
        }
    }
    Ok(())
}

fn required_dispatch(
    item: &syn::ItemFn,
    expr: &Expr,
    members: &BTreeMap<String, Option<String>>,
    edges: &mut Vec<Value>,
) -> Result<(), String> {
    if same(expr, quote!(false)) {
        if !same(&item.sig, quote!(fn native_required(_: &str) -> bool)) {
            return Err(error());
        }
        return Ok(());
    }
    if !same(&item.sig, quote!(fn native_required(name: &str) -> bool)) {
        return Err(error());
    }
    let Expr::Macro(value) = expr else {
        return Err(error());
    };
    if !value.attrs.is_empty()
        || !value.mac.path.is_ident("matches")
        || !matches!(&value.mac.delimiter, syn::MacroDelimiter::Paren(_))
    {
        return Err(error());
    }
    let required = syn::parse2::<Required>(value.mac.tokens.clone()).map_err(|_error| error())?;
    let mut names = Vec::new();
    if required.selector != "name" || !patterns(&required.pattern, &mut names) || names.is_empty() {
        return Err(error());
    }
    let mut seen = BTreeSet::new();
    for member in names {
        if !members.contains_key(&member) || !seen.insert(member.clone()) {
            return Err(error());
        }
        edges.push(
            json!({"from":"native_required","member":member,"role":"native-required-predicate",
            "policy_correspondence":"accepted-source-owner","ownership":"literal-only"}),
        );
    }
    Ok(())
}

fn function(
    item: &syn::ItemFn,
    rows: &[Row],
    members: &BTreeMap<String, Option<String>>,
    edges: &mut Vec<Value>,
) -> Result<(), String> {
    let name = item.sig.ident.to_string();
    if !same(&item.vis, quote!(pub(super))) || !plain_body(&item.block) {
        return Err(error());
    }
    if name == "retain_for_inspection" {
        return retain_dispatch(item, rows, edges);
    }
    let [attribute] = item.attrs.as_slice() else {
        return Err(error());
    };
    if !same(attribute, quote!(#[cfg(not(target_arch = "wasm32"))])) {
        return Err(error());
    }
    let expr = tail(&item.block).ok_or_else(error)?;
    match name.as_str() {
        "initialize_native" => initialize_dispatch(item, expr, rows, edges),
        "validate_native" => validate_dispatch(item, expr, rows, edges),
        "native_owner" => owner_dispatch(item, expr, members, edges),
        "native_required" => required_dispatch(item, expr, members, edges),
        _ => Err(error()),
    }
}

fn bound_owners(bindings: &[(&str, &str)]) -> Result<BTreeMap<String, syn::Ident>, String> {
    let mut by_owner = BTreeMap::new();
    let mut markers = BTreeSet::new();
    for &(marker, owner) in bindings {
        let marker_id = syn::parse_str::<syn::Ident>(marker).map_err(|_error| error())?;
        if owner.is_empty()
            || owner.len() > 64
            || !owner.is_ascii()
            || !markers.insert(marker)
            || by_owner.insert(owner.to_owned(), marker_id).is_some()
        {
            return Err(error());
        }
    }
    Ok(by_owner)
}

fn selected_members(selection: &Value) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut members = BTreeMap::new();
    for member in selection["members"].as_array().ok_or_else(error)? {
        let name = member["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or_else(error)?;
        let schema_owner = member.get("schema_owner").ok_or_else(error)?;
        let owner = if schema_owner.is_null() {
            None
        } else if let Value::String(owner) = schema_owner
            && !owner.is_empty()
        {
            Some(owner.clone())
        } else {
            return Err(error());
        };
        if members.insert(name.to_owned(), owner).is_some() {
            return Err(error());
        }
    }
    Ok(members)
}

fn complete_header(
    rows: &mut [Row],
    constants: Vec<(usize, &syn::ItemConst)>,
    functions: &BTreeMap<String, &syn::ItemFn>,
    members: &BTreeMap<String, Option<String>>,
    roots: &mut [Value],
) -> Result<Vec<Value>, String> {
    rows.sort_by_key(|row| row.ordinal);
    let mut owners = BTreeSet::new();
    for (ordinal, row) in rows.iter().enumerate() {
        if row.ordinal != ordinal || !owners.insert(row.owner.as_str()) {
            return Err(error());
        }
    }
    let mut asserted = BTreeSet::new();
    for (root, constant) in constants {
        let row = rows
            .iter()
            .find(|row| abi(constant, &row.cache))
            .ok_or_else(error)?;
        if !asserted.insert(row.ordinal) {
            return Err(error());
        }
        let details = roots
            .get_mut(root)
            .and_then(|record| record.get_mut("details"))
            .ok_or_else(error)?;
        *details = json!({"cache":row.cache.to_string(),"owner":row.owner});
    }
    let mut dispatch_edges = Vec::new();
    for name in [
        "retain_for_inspection",
        "initialize_native",
        "validate_native",
        "native_owner",
        "native_required",
    ] {
        function(
            functions.get(name).ok_or_else(error)?,
            rows,
            members,
            &mut dispatch_edges,
        )?;
    }
    Ok(dispatch_edges)
}

/// Bind marker/owner pairs from accepted declarations, not a central bidder list.
/// Wholly absent auxiliaries remain explicitly incomplete for header purposes.
pub(super) fn classify(
    items: &[&Item],
    bindings: &[(&str, &str)],
    identity: &str,
    selection: &Value,
) -> Result<Classified, String> {
    if items.is_empty() {
        return Ok(Classified {
            status: Status::Absent,
            roots: Vec::new(),
            dispatch_edges: Vec::new(),
        });
    }
    if identity.len() != 64 || !identity.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(error());
    }
    let by_owner = bound_owners(bindings)?;
    let members = selected_members(selection)?;
    let mut has_import = false;
    let mut has_type = false;
    let mut has_identity = false;
    let mut rows = Vec::new();
    let mut constants = Vec::new();
    let mut functions = BTreeMap::new();
    let mut roots = Vec::new();
    let id = syn::LitStr::new(identity, proc_macro2::Span::call_site());
    for (ordinal, item) in items.iter().enumerate() {
        let mut details = json!({});
        let kind = if let Item::Use(value) = item
            && !has_import
            && same(
                value,
                quote!(
                    use std::hint::black_box;
                ),
            ) {
            has_import = true;
            "inspection-import"
        } else if let Item::Struct(value) = item
            && !has_type
            && registry_type(value)
        {
            has_type = true;
            "inline-retention-type"
        } else if let Item::Static(value) = item {
            let row = retention_row(value, &by_owner, identity)?;
            details = json!({"entry":row.entry.to_string(),"marker":row.marker.to_string(),
                    "owner":row.owner,"cache":row.cache.to_string()});
            rows.push(row);
            "inline-retention-row"
        } else if let Item::Const(value) = item
            && value.ident == "_"
        {
            constants.push((ordinal, value));
            "const-abi-assertions"
        } else if let Item::Const(value) = item
            && !has_identity
            && same(value, quote!(const STAGING_INPUT_ID: &str = #id;))
        {
            has_identity = true;
            "readonly-staging-identity"
        } else if let Item::Fn(value) = item
            && functions
                .insert(value.sig.ident.to_string(), value)
                .is_none()
        {
            "dispatch"
        } else {
            return Err(error());
        };
        roots.push(json!({"ordinal":ordinal,"kind":kind,"details":details,
            "source_sha256":super::expanded::digest(item.to_token_stream().to_string().as_bytes())}));
    }
    if !has_import
        || !has_type
        || !has_identity
        || rows.len() != bindings.len()
        || constants.len() != rows.len()
        || functions.len() != 5
    {
        return Err(error());
    }
    let dispatch_edges = complete_header(&mut rows, constants, &functions, &members, &mut roots)?;
    Ok(Classified {
        status: Status::Complete,
        roots,
        dispatch_edges,
    })
}

#[cfg(test)]
mod tests {
    use super::{Classified, Status, classify};
    use quote::quote;
    use serde_json::{Value, json};
    use syn::visit_mut::VisitMut;

    fn fixture() -> (syn::File, String, Value) {
        let identity = "a".repeat(64);
        let id = syn::LitStr::new(&identity, proc_macro2::Span::call_site());
        let id_bytes = syn::LitByteStr::new(identity.as_bytes(), proc_macro2::Span::call_site());
        let mut owner = [0_u8; 64];
        owner[..10].copy_from_slice(b"fake_owner");
        let owner_literal = syn::LitByteStr::new(&owner, proc_macro2::Span::call_site());
        let syntax = syn::parse2(quote! {
            use std::hint::black_box;
            #[repr(C)]
            struct StagedValidator<C> {
                magic: [u8; 16], identity: [u8; 64], owner: [u8; 64], owner_len: u32,
                validate: fn(&serde_json::Value, &mut C) -> bool,
                new_cache: fn() -> Option<C>, initialize: fn() -> bool,
            }
            static STAGED_VALIDATOR_0: StagedValidator<FakeMarkerSearchCache> = StagedValidator {
                magic: *b"AUCTIONEERSCHEMA", identity: *#id_bytes,
                owner: *#owner_literal, owner_len: 10,
                validate: FakeMarker::is_valid, new_cache: FakeMarker::new_search_cache,
                initialize: FakeMarker::initialize_regexes,
            };
            const _: () = {
                use std::mem::{align_of, offset_of, size_of};
                let word = size_of::<usize>();
                let (validate, new_cache, initialize, row_bytes) = match word {
                    4 => (148, 152, 156, 160),
                    8 => (152, 160, 168, 176),
                    _ => panic!("unsupported schema inspection pointer width"),
                };
                assert!(size_of::<fn(&serde_json::Value, &mut FakeMarkerSearchCache) -> bool>() == word,
                    "validation function pointer width");
                assert!(size_of::<fn() -> Option<FakeMarkerSearchCache>>() == word,
                    "cache function pointer width");
                assert!(size_of::<fn() -> bool>() == word, "initialization function pointer width");
                assert!(align_of::<StagedValidator<FakeMarkerSearchCache>>() == word, "schema inspection row alignment");
                assert!(offset_of!(StagedValidator<FakeMarkerSearchCache>, magic) == 0, "schema inspection magic offset");
                assert!(offset_of!(StagedValidator<FakeMarkerSearchCache>, identity) == 16, "schema inspection identity offset");
                assert!(offset_of!(StagedValidator<FakeMarkerSearchCache>, owner) == 80, "schema inspection owner offset");
                assert!(offset_of!(StagedValidator<FakeMarkerSearchCache>, owner_len) == 144, "schema inspection owner length offset");
                assert!(offset_of!(StagedValidator<FakeMarkerSearchCache>, validate) == validate, "schema inspection validation pointer offset");
                assert!(offset_of!(StagedValidator<FakeMarkerSearchCache>, new_cache) == new_cache, "schema inspection cache pointer offset");
                assert!(offset_of!(StagedValidator<FakeMarkerSearchCache>, initialize) == initialize, "schema inspection initialization pointer offset");
                assert!(size_of::<StagedValidator<FakeMarkerSearchCache>>() == row_bytes, "schema inspection row size");
            };
            const STAGING_INPUT_ID: &str = #id;
            pub(super) fn retain_for_inspection() {
                black_box((&STAGED_VALIDATOR_0, STAGING_INPUT_ID));
            }
            #[cfg(not(target_arch = "wasm32"))]
            pub(super) fn initialize_native() -> bool { FakeMarker::initialize_regexes() }
            #[cfg(not(target_arch = "wasm32"))]
            pub(super) fn validate_native(owner: &str, value: &serde_json::Value) -> Option<bool> {
                match owner {
                    "fake_owner" => {
                        let mut cache = FakeMarker::new_search_cache()?;
                        Some(FakeMarker::is_valid(value, &mut cache))
                    },
                    _ => None,
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            pub(super) fn native_owner(name: &str) -> Option<&'static str> {
                match name { "fake_owner" | "fake_alias" => Some("fake_owner"), _ => None }
            }
            #[cfg(not(target_arch = "wasm32"))]
            pub(super) fn native_required(name: &str) -> bool {
                matches!(name, "fake_owner")
            }
        }).unwrap();
        let selection = json!({"members":[
            {"name":"fake_owner","schema_owner":"fake_owner"},
            {"name":"fake_alias","schema_owner":"fake_owner"}
        ]});
        (syntax, identity, selection)
    }

    fn inspect(
        syntax: &syn::File,
        identity: &str,
        selection: &Value,
    ) -> Result<Classified, String> {
        let roots = syntax.items.iter().collect::<Vec<_>>();
        classify(&roots, &[("FakeMarker", "fake_owner")], identity, selection)
    }

    fn function<'a>(syntax: &'a mut syn::File, name: &str) -> &'a mut syn::ItemFn {
        syntax
            .items
            .iter_mut()
            .find_map(|item| {
                if let syn::Item::Fn(function) = item
                    && function.sig.ident == name
                {
                    Some(function)
                } else {
                    None
                }
            })
            .unwrap()
    }

    #[test]
    fn auxiliary_wholly_absent_is_explicitly_not_complete() {
        let result = classify(&[], &[("FakeMarker", "fake_owner")], "", &Value::Null).unwrap();
        assert_eq!(result.status, Status::Absent);
        assert_ne!(result.status, Status::Complete);
        assert!(result.roots.is_empty());
        assert!(result.dispatch_edges.is_empty());
    }

    #[test]
    fn auxiliary_independent_fake_header_is_accepted() {
        let (syntax, identity, selection) = fixture();
        let result = inspect(&syntax, &identity, &selection).unwrap();
        assert_eq!(result.status, Status::Complete);
        assert_eq!(result.roots.len(), 10);
        assert_eq!(result.dispatch_edges.len(), 7);
    }

    #[test]
    fn auxiliary_complete_fixture_rejects_each_unclassified_extra_root() {
        for extra in [
            quote! { static FAKE_HIDDEN: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| vec![0_u8; 17]); },
            quote! { fn fake_hidden() {} },
            quote! { const FAKE_UNCLASSIFIED: &str = "unattributed"; },
        ] {
            let (mut syntax, identity, selection) = fixture();
            syntax.items.push(syn::parse2(extra).unwrap());
            assert_eq!(
                inspect(&syntax, &identity, &selection).err().as_deref(),
                Some("unsupported captured staging syntax")
            );
        }
    }

    #[test]
    fn auxiliary_missing_or_duplicate_root_refuses_instead_of_becoming_absent() {
        let (original, identity, selection) = fixture();
        for index in 0..original.items.len() {
            let mut missing = original.clone();
            missing.items.remove(index);
            assert_eq!(
                inspect(&missing, &identity, &selection).err().as_deref(),
                Some("unsupported captured staging syntax"),
                "missing {index}"
            );
            let mut duplicate = original.clone();
            duplicate.items.push(original.items[index].clone());
            assert_eq!(
                inspect(&duplicate, &identity, &selection).err().as_deref(),
                Some("unsupported captured staging syntax"),
                "duplicate {index}"
            );
        }
    }

    #[test]
    fn auxiliary_each_known_function_refuses_hidden_retained_storage() {
        for name in [
            "retain_for_inspection",
            "initialize_native",
            "validate_native",
            "native_owner",
            "native_required",
        ] {
            let (mut syntax, identity, selection) = fixture();
            function(&mut syntax, name).block.stmts.insert(
                0,
                syn::parse_quote! {
                    static HIDDEN: std::sync::LazyLock<Vec<u8>> =
                        std::sync::LazyLock::new(|| vec![0_u8; 17]);
                },
            );
            assert_eq!(
                inspect(&syntax, &identity, &selection).err().as_deref(),
                Some("unsupported captured staging syntax"),
                "{name}"
            );
        }
    }

    #[test]
    fn auxiliary_each_known_function_refuses_unknown_runtime_calls() {
        for name in [
            "retain_for_inspection",
            "initialize_native",
            "validate_native",
            "native_owner",
            "native_required",
        ] {
            let (mut syntax, identity, selection) = fixture();
            function(&mut syntax, name).block.stmts.insert(
                0,
                syn::parse_quote! {
                    std::mem::drop(Vec::<u8>::new());
                },
            );
            assert_eq!(
                inspect(&syntax, &identity, &selection).err().as_deref(),
                Some("unsupported captured staging syntax"),
                "{name}"
            );
        }
    }

    fn assert_known_call_arguments_cannot_hide_storage() {
        let (mut syntax, identity, selection) = fixture();
        *function(&mut syntax, "retain_for_inspection").block = syn::parse_quote! {{
            black_box(({
                static HIDDEN: std::sync::LazyLock<Vec<u8>> =
                    std::sync::LazyLock::new(|| vec![0_u8; 17]);
                &STAGED_VALIDATOR_0
            }, STAGING_INPUT_ID));
        }};
        assert_eq!(
            inspect(&syntax, &identity, &selection).err().as_deref(),
            Some("unsupported captured staging syntax")
        );
    }

    #[test]
    fn auxiliary_known_call_arguments_and_matches_guard_cannot_hide_storage() {
        assert_known_call_arguments_cannot_hide_storage();

        let (mut syntax, identity, selection) = fixture();
        *function(&mut syntax, "native_required").block = syn::parse_quote! {{
            matches!(name, "fake_owner" if {
                static HIDDEN: std::sync::LazyLock<Vec<u8>> =
                    std::sync::LazyLock::new(|| vec![0_u8; 17]);
                !HIDDEN.is_empty()
            })
        }};
        assert_eq!(
            inspect(&syntax, &identity, &selection).err().as_deref(),
            Some("unsupported captured staging syntax")
        );
    }

    fn assert_wrong_retention_identity_refuses() {
        let (mut syntax, identity, selection) = fixture();
        let row = syntax
            .items
            .iter_mut()
            .find_map(|item| {
                if let syn::Item::Static(row) = item {
                    Some(row)
                } else {
                    None
                }
            })
            .unwrap();
        let syn::Expr::Struct(value) = row.expr.as_mut() else {
            panic!("row fixture");
        };
        value
            .fields
            .iter_mut()
            .find(|field| {
                field.member
                    == syn::Member::Named(syn::Ident::new(
                        "owner_len",
                        proc_macro2::Span::call_site(),
                    ))
            })
            .unwrap()
            .expr = syn::parse_quote!(9);
        assert_eq!(
            inspect(&syntax, &identity, &selection).err().as_deref(),
            Some("unsupported captured staging syntax")
        );
    }

    #[test]
    fn auxiliary_wrong_retention_identity_and_abi_refuse() {
        assert_wrong_retention_identity_refuses();

        let (mut syntax, identity, selection) = fixture();
        let abi = syntax
            .items
            .iter_mut()
            .find_map(|item| {
                if let syn::Item::Const(constant) = item
                    && constant.ident == "_"
                {
                    Some(constant)
                } else {
                    None
                }
            })
            .unwrap();
        let syn::Expr::Block(block) = abi.expr.as_mut() else {
            panic!("ABI fixture");
        };
        block.block.stmts[3] = syn::parse_quote! {
            assert!(size_of::<usize>() == word, "validation function pointer width");
        };
        assert_eq!(
            inspect(&syntax, &identity, &selection).err().as_deref(),
            Some("unsupported captured staging syntax")
        );
    }

    #[test]
    fn auxiliary_empty_catalog_header_is_complete_not_absent() {
        let (mut syntax, identity, _) = fixture();
        syntax.items.retain(|item| {
            !matches!(item, syn::Item::Static(_))
                && !matches!(item, syn::Item::Const(constant) if constant.ident == "_")
        });
        *function(&mut syntax, "retain_for_inspection").block = syn::parse_quote! {{
            black_box((STAGING_INPUT_ID));
        }};
        *function(&mut syntax, "initialize_native").block = syn::parse_quote! {{ true }};
        *function(&mut syntax, "validate_native").block = syn::parse_quote! {{
            match owner { _ => None }
        }};
        function(&mut syntax, "native_owner").sig = syn::parse_quote! {
            fn native_owner(_: &str) -> Option<&'static str>
        };
        *function(&mut syntax, "native_owner").block = syn::parse_quote! {{ None }};
        function(&mut syntax, "native_required").sig = syn::parse_quote! {
            fn native_required(_: &str) -> bool
        };
        *function(&mut syntax, "native_required").block = syn::parse_quote! {{ false }};
        let roots = syntax.items.iter().collect::<Vec<_>>();
        let result = classify(&roots, &[], &identity, &json!({"members":[]})).unwrap();
        assert_eq!(result.status, Status::Complete);
    }
    // Syn visits macro tokens opaquely. Rename the fixture's identifiers and
    // parsed literals recursively without changing groups or punctuation.
    fn rename_fixture_tokens(visitor: &mut impl VisitMut, tokens: &mut proc_macro2::TokenStream) {
        use proc_macro2::{Group, TokenTree};
        use quote::ToTokens as _;
        let mut output = proc_macro2::TokenStream::new();
        for token in tokens.clone() {
            match token {
                TokenTree::Group(group) => {
                    let mut contents = group.stream();
                    visitor.visit_token_stream_mut(&mut contents);
                    let mut renamed = Group::new(group.delimiter(), contents);
                    renamed.set_span(group.span());
                    output.extend([TokenTree::Group(renamed)]);
                }
                TokenTree::Ident(mut ident) => {
                    visitor.visit_ident_mut(&mut ident);
                    output.extend([TokenTree::Ident(ident)]);
                }
                TokenTree::Literal(literal) => {
                    let mut parsed_literal: syn::Lit = syn::parse2(quote!(#literal)).unwrap();
                    visitor.visit_lit_mut(&mut parsed_literal);
                    parsed_literal.to_tokens(&mut output);
                }
                TokenTree::Punct(punct) => output.extend([TokenTree::Punct(punct)]),
            }
        }
        *tokens = output;
    }

    #[test]
    fn auxiliary_accepts_dynamic_marker_owner_and_alias_rows() {
        struct Rename;
        impl VisitMut for Rename {
            fn visit_token_stream_mut(&mut self, i: &mut proc_macro2::TokenStream) {
                rename_fixture_tokens(self, i);
            }
            fn visit_ident_mut(&mut self, i: &mut syn::Ident) {
                if *i == "FakeMarker" {
                    *i = syn::Ident::new("OtherMarker", i.span());
                }
                if *i == "FakeMarkerSearchCache" {
                    *i = syn::Ident::new("OtherMarkerSearchCache", i.span());
                }
            }
            fn visit_lit_str_mut(&mut self, i: &mut syn::LitStr) {
                if i.value() == "fake_owner" {
                    *i = syn::LitStr::new("fake_second_owner", i.span());
                }
            }
            fn visit_lit_byte_str_mut(&mut self, i: &mut syn::LitByteStr) {
                let old = i.value();
                if old.len() == 64 && old.starts_with(b"fake_owner") {
                    let mut bytes = [0_u8; 64];
                    bytes[..17].copy_from_slice(b"fake_second_owner");
                    *i = syn::LitByteStr::new(&bytes, i.span());
                }
            }
        }
        let (mut syntax, identity, _) = fixture();
        Rename.visit_file_mut(&mut syntax);
        let row = syntax
            .items
            .iter_mut()
            .find_map(|item| {
                if let syn::Item::Static(row) = item {
                    Some(row)
                } else {
                    None
                }
            })
            .unwrap();
        let syn::Expr::Struct(value) = row.expr.as_mut() else {
            panic!("row fixture");
        };
        for field in &mut value.fields {
            if matches!(&field.member, syn::Member::Named(name) if name == "owner_len") {
                field.expr = syn::parse_quote!(17);
            }
        }
        let selection = json!({"members":[
            {"name":"fake_second_owner","schema_owner":"fake_second_owner"},
            {"name":"fake_alias","schema_owner":"fake_second_owner"}
        ]});
        let roots = syntax.items.iter().collect::<Vec<_>>();
        let classified = classify(
            &roots,
            &[("OtherMarker", "fake_second_owner")],
            &identity,
            &selection,
        )
        .unwrap();
        assert_eq!(classified.status, Status::Complete);
    }

    fn gt_spacing(
        tokens: proc_macro2::TokenStream,
        spacing: proc_macro2::Spacing,
    ) -> proc_macro2::TokenStream {
        use proc_macro2::{Group, Punct, TokenTree};
        tokens
            .into_iter()
            .map(|token| match token {
                TokenTree::Group(group) => TokenTree::Group(Group::new(
                    group.delimiter(),
                    gt_spacing(group.stream(), spacing),
                )),
                TokenTree::Punct(punct) if punct.as_char() == '>' => {
                    TokenTree::Punct(Punct::new('>', spacing))
                }
                other @ (TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Punct(_)) => {
                    other
                }
            })
            .collect()
    }

    #[test]
    fn auxiliary_comparison_ignores_gt_spacing_only_after_ast_parse() {
        use quote::ToTokens as _;
        let parsed: syn::Type = syn::parse_quote! {
            StagedValidator<fn() -> Option<FakeMarkerSearchCache>>
        };
        // Both variants originate from a successfully parsed type, not raw input.
        for spacing in [proc_macro2::Spacing::Alone, proc_macro2::Spacing::Joint] {
            assert!(super::same(
                &parsed,
                gt_spacing(parsed.to_token_stream(), spacing)
            ));
        }
    }

    #[test]
    fn auxiliary_comparison_distinguishes_joint_operators_from_borrow_tokens() {
        use quote::ToTokens as _;
        let conjunction: syn::Expr = syn::parse_quote!(a && b);
        let bitand_borrow: syn::Expr = syn::parse_quote!(a & &b);
        assert!(super::same(&conjunction, quote!(a && b)));
        assert!(super::same(&bitand_borrow, quote!(a & &b)));
        assert!(!super::same(&conjunction, bitand_borrow.to_token_stream()));
        assert!(!super::same(&bitand_borrow, conjunction.to_token_stream()));
    }

    #[test]
    fn auxiliary_comparison_preserves_groups_literals_and_punctuation_characters() {
        let literal: syn::Expr = syn::parse_quote!(("fake_first_literal"));
        assert!(!super::same(&literal, quote!(["fake_first_literal"])));
        assert!(!super::same(&literal, quote!(("fake_second_literal"))));
        assert!(!super::same(&literal, quote!(("fake_first_literal",))));
        let reference: syn::Type = syn::parse_quote!(&mut FakeMarkerSearchCache);
        assert!(!super::same(&reference, quote!(*mut FakeMarkerSearchCache)));
    }
}

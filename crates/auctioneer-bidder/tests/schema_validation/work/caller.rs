//! Caller-cap expressions and scope duties, never source authority.
//!
//! The fixed workspace bytes below are DATA. The existing context recipe check
//! remains conditional data validation, not a sealed caller or artifact join.
//! Resource/policy fields are symbolic operands, not installed numerical facts.
//! Successful core preparation is the entry premise; arbitrary low-level owned
//! preparation and cleanup before that entry are not covered. Panic mode,
//! ordinary cleanup costs and compiled-artifact correspondence stay missing.
//! The eventual join must reuse the context/source-window owners, not the
//! independent test-side source assertions as its implementation.

use super::ContextSources;
use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::ValidationView;
use crate::schema_validation::handoff::SourceInputs;
use crate::schema_validation::operations::source_syntax::{
    literal_doc, parse_file, tokens, without_docs,
};
use quote::{ToTokens, quote};
use sha2::{Digest as _, Sha256};
use std::mem;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum CallerUnit {
    CorePrepare,
    CoreServices,
    CoreOperation,
    PolicyHost,
    PolicyResolve,
    NormalizedPrepare,
    ImplicitFill,
    ExchangeContext,
    RuntimeBudget,
    CoreAuctionRoot,
    CoreConfigRoot,
    OpenRtbRoot,
    RuntimeRoot,
    ExchangeRoot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallerSource {
    unit: CallerUnit,
    path: &'static str,
    bytes: Vec<u8>,
    sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Owner {
    Caller(CallerUnit),
    Context(super::SourceRole),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAnchor {
    owner: Owner,
    item: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapField {
    ResourceImpressions,
    RequestedHostBidders,
    FundedBidders,
    CoreInputNodes,
    HostParsedNodes,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CapExpression {
    Field(CapField),
    Literal { source: SourceAnchor, value: u64 },
    Min(Box<Self>, Box<Self>),
    Max(Box<Self>, Box<Self>),
    Add(Box<Self>, Box<Self>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopeGuard {
    DeadlineBeforeScope,
    RegistryBeforeSchema,
    ConfigurationBeforeSchema,
    OptionalSchemaBypass,
    FundingAndProjectionBeforeRetain,
    WholeScopedRequestBeforeScrub,
    RetainedNonemptyResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GuardSource {
    guard: ScopeGuard,
    source: SourceAnchor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupDuty {
    RejectedImpression,
    RetainErrorAndUnwind,
    ScopedRequestRelease,
    PartialMaterialization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BorrowDuty {
    UnchangedBidderParametersFromScopedEnsure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct CallerContract {
    original_impressions_upper: CapExpression,
    effective_bidder_cap: CapExpression,
    scoped_nodes_upper: CapExpression,
    required_borrow: BorrowDuty,
    parameter_borrow_preserved: Option<bool>,
    source_chain: Vec<SourceAnchor>,
    scope_guards: Vec<GuardSource>,
    cleanup_duties: Vec<CleanupDuty>,
    unresolved_helpers: Vec<&'static str>,
    panic_unwinds: Option<bool>,
    cleanup_work: Option<u64>,
    tree_navigation_work: Option<u64>,
    compiled_artifact_binding: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Refusal {
    MissingSourceProof,
    MissingSource(CallerUnit),
    SourceShape(CallerUnit),
    Context(super::Refusal),
}

const CALLER_PATHS: [(CallerUnit, &str); 14] = [
    (
        CallerUnit::CorePrepare,
        "crates/auctioneer-core/src/auction/prepare.rs",
    ),
    (
        CallerUnit::CoreServices,
        "crates/auctioneer-core/src/auction/services.rs",
    ),
    (
        CallerUnit::CoreOperation,
        "crates/auctioneer-core/src/auction/operation.rs",
    ),
    (
        CallerUnit::PolicyHost,
        "crates/auctioneer-core/src/config/policy_host.rs",
    ),
    (
        CallerUnit::PolicyResolve,
        "crates/auctioneer-core/src/config/resolve.rs",
    ),
    (
        CallerUnit::NormalizedPrepare,
        "crates/auctioneer-openrtb/src/prepare/mod.rs",
    ),
    (
        CallerUnit::ImplicitFill,
        "crates/auctioneer-openrtb/src/prepare/implicit_fill.rs",
    ),
    (
        CallerUnit::ExchangeContext,
        "crates/auctioneer-exchange/src/context.rs",
    ),
    (
        CallerUnit::RuntimeBudget,
        "crates/auctioneer-runtime/src/budget.rs",
    ),
    (
        CallerUnit::CoreAuctionRoot,
        "crates/auctioneer-core/src/auction/mod.rs",
    ),
    (
        CallerUnit::CoreConfigRoot,
        "crates/auctioneer-core/src/config/mod.rs",
    ),
    (
        CallerUnit::OpenRtbRoot,
        "crates/auctioneer-openrtb/src/lib.rs",
    ),
    (
        CallerUnit::RuntimeRoot,
        "crates/auctioneer-runtime/src/lib.rs",
    ),
    (
        CallerUnit::ExchangeRoot,
        "crates/auctioneer-exchange/src/lib.rs",
    ),
];

struct ParsedSources {
    files: Vec<(Owner, syn::File)>,
}

pub(in crate::schema_validation) fn source_paths() -> &'static [(CallerUnit, &'static str)] {
    &CALLER_PATHS
}

// Tests-first byte-correspondence seam only. No compiled/type/cfg/full-closure
// authority follows from these inputs. Replace only after hosted assertion RED,
// reusing ParsedSources/derive_caller_sources through borrowed source views.
pub(in crate::schema_validation) fn join_consumed_sources(
    _inputs: &SourceInputs<'_>,
    _seed: &CapturedSeed,
    _view: ValidationView<'_>,
    _attempts: u64,
) -> Result<CallerContract, Refusal> {
    Err(Refusal::MissingSourceProof)
}

fn shape(owner: Owner) -> Refusal {
    match owner {
        Owner::Caller(unit) => Refusal::SourceShape(unit),
        Owner::Context(role) => Refusal::Context(super::Refusal::SourceShape(role)),
    }
}

fn same(owner: Owner, actual: &impl ToTokens, supported: &impl ToTokens) -> Result<(), Refusal> {
    if tokens(actual) == tokens(supported) {
        Ok(())
    } else {
        Err(shape(owner))
    }
}

fn root_attributes(file: &syn::File) -> bool {
    file.shebang.is_none()
        && file.attrs.iter().all(|attribute| {
            matches!(&attribute.meta, syn::Meta::NameValue(meta)
            if attribute.path().is_ident("doc")
            && matches!(&meta.value, syn::Expr::Lit(value)
                if matches!(&value.lit, syn::Lit::Str(_))))
        })
}

impl ParsedSources {
    fn new(context: &ContextSources<'_>, sources: &[CallerSource]) -> Result<Self, Refusal> {
        let mut files = Vec::new();
        for (unit, path) in CALLER_PATHS {
            let owner = Owner::Caller(unit);
            let mut matching = sources.iter().filter(|source| source.unit == unit);
            let source = matching.next().ok_or(Refusal::MissingSource(unit))?;
            if matching.next().is_some()
                || source.path != path
                || source.sha256 != hex::encode(Sha256::digest(&source.bytes))
            {
                return Err(shape(owner));
            }
            let file = parse_file(&source.bytes).map_err(|()| shape(owner))?;
            if !root_attributes(&file) {
                return Err(shape(owner));
            }
            files.push((owner, file));
        }
        if sources.len() != CALLER_PATHS.len() {
            return Err(Refusal::SourceShape(CallerUnit::CorePrepare));
        }
        for (role, bytes) in [
            (super::SourceRole::ExchangeRun, context.exchange_run),
            (super::SourceRole::ExchangeLimits, context.exchange_limits),
            (super::SourceRole::ExchangeSplit, context.exchange_split),
            (super::SourceRole::ExchangeSchema, context.exchange_schema),
        ] {
            let owner = Owner::Context(role);
            let file = parse_file(bytes).map_err(|()| shape(owner))?;
            if !root_attributes(&file) {
                return Err(shape(owner));
            }
            files.push((owner, file));
        }
        Ok(Self { files })
    }

    fn file(&self, owner: Owner) -> Result<&syn::File, Refusal> {
        self.files
            .iter()
            .find(|(key, _)| *key == owner)
            .map(|(_, file)| file)
            .ok_or_else(|| shape(owner))
    }
}

fn caller_anchor(unit: CallerUnit, item: &'static str) -> SourceAnchor {
    SourceAnchor {
        owner: Owner::Caller(unit),
        item,
    }
}

fn context_anchor(role: super::SourceRole, item: &'static str) -> SourceAnchor {
    SourceAnchor {
        owner: Owner::Context(role),
        item,
    }
}

enum Declaration {
    Free(syn::ItemFn),
    Method(syn::ItemImpl),
}

struct Rule {
    anchor: SourceAnchor,
    declaration: Declaration,
}

fn free_rule(anchor: SourceAnchor, declaration: syn::ItemFn) -> Rule {
    Rule {
        anchor,
        declaration: Declaration::Free(declaration),
    }
}

fn method_rule(anchor: SourceAnchor, declaration: syn::ItemImpl) -> Rule {
    Rule {
        anchor,
        declaration: Declaration::Method(declaration),
    }
}

struct Matched {
    anchor: SourceAnchor,
    body: syn::Block,
}

fn only<'a, T: 'a>(mut items: impl Iterator<Item = &'a T>, owner: Owner) -> Result<&'a T, Refusal> {
    let first = items.next().ok_or_else(|| shape(owner))?;
    if items.next().is_some() {
        return Err(shape(owner));
    }
    Ok(first)
}

fn match_free(
    file: &syn::File,
    rule: &Rule,
    expected: &syn::ItemFn,
) -> Result<syn::Block, Refusal> {
    let actual = only(
        file.items.iter().filter_map(|item| {
            if let syn::Item::Fn(function) = item
                && function.sig.ident == rule.anchor.item
            {
                Some(function)
            } else {
                None
            }
        }),
        rule.anchor.owner,
    )?;
    let mut header = actual.clone();
    header.block.stmts.clear();
    normalize_signature(&mut header.sig);
    let mut wanted = expected.clone();
    normalize_signature(&mut wanted.sig);
    same(
        rule.anchor.owner,
        &without_docs(syn::Item::Fn(header)),
        &without_docs(syn::Item::Fn(wanted)),
    )?;
    Ok(actual.block.as_ref().clone())
}

fn same_impl_kind(actual: &syn::ItemImpl, expected: &syn::ItemImpl) -> bool {
    match (&actual.trait_, &expected.trait_) {
        (None, None) => true,
        (Some((actual_path, _)), Some((expected_path, _))) => {
            tokens(actual_path) == tokens(expected_path)
        }
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn match_method(
    file: &syn::File,
    rule: &Rule,
    expected: &syn::ItemImpl,
) -> Result<syn::Block, Refusal> {
    let actual = only(
        file.items.iter().filter_map(|item| {
            if let syn::Item::Impl(implementation) = item
                && tokens(&implementation.self_ty) == tokens(&expected.self_ty)
                && same_impl_kind(implementation, expected)
            {
                Some(implementation)
            } else {
                None
            }
        }),
        rule.anchor.owner,
    )?;
    let mut actual_header = actual.clone();
    actual_header.items.clear();
    let mut expected_header = expected.clone();
    expected_header.items.clear();
    same(
        rule.anchor.owner,
        &without_docs(syn::Item::Impl(actual_header)),
        &without_docs(syn::Item::Impl(expected_header)),
    )?;
    let wanted = only(
        expected.items.iter().filter_map(|member| {
            if let syn::ImplItem::Fn(method) = member {
                Some(method)
            } else {
                None
            }
        }),
        rule.anchor.owner,
    )?;
    let method = only(
        actual.items.iter().filter_map(|member| {
            if let syn::ImplItem::Fn(method) = member
                && method.sig.ident == wanted.sig.ident
            {
                Some(method)
            } else {
                None
            }
        }),
        rule.anchor.owner,
    )?;
    let mut header = method.clone();
    header.attrs.retain(|attribute| !literal_doc(attribute));
    header.block.stmts.clear();
    normalize_signature(&mut header.sig);
    let mut wanted_header = wanted.clone();
    normalize_signature(&mut wanted_header.sig);
    same(rule.anchor.owner, &header, &wanted_header)?;
    Ok(method.block.clone())
}

fn match_rules(sources: &ParsedSources) -> Result<Vec<Matched>, Refusal> {
    let rules = core_rules()
        .into_iter()
        .chain(policy_rules())
        .chain(operation_rules())
        .chain(exchange_rules());
    let mut matched: Vec<Matched> = Vec::new();
    for rule in rules {
        if matched.iter().any(|prior| prior.anchor == rule.anchor) {
            return Err(shape(rule.anchor.owner));
        }
        let file = sources.file(rule.anchor.owner)?;
        let body = match &rule.declaration {
            Declaration::Free(expected) => match_free(file, &rule, expected)?,
            Declaration::Method(expected) => match_method(file, &rule, expected)?,
        };
        matched.push(Matched {
            anchor: rule.anchor,
            body,
        });
    }
    Ok(matched)
}

fn body(matched: &[Matched], anchor: SourceAnchor) -> Result<&syn::Block, Refusal> {
    matched
        .iter()
        .find(|function| function.anchor == anchor)
        .map(|function| &function.body)
        .ok_or_else(|| shape(anchor.owner))
}

fn local<'a>(
    block: &'a syn::Block,
    anchor: SourceAnchor,
    name: &str,
) -> Result<(usize, &'a syn::Local), Refusal> {
    let mut found = block
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            if let syn::Stmt::Local(binding) = statement
                && match &binding.pat {
                    syn::Pat::Ident(pattern) => pattern.ident == name,
                    syn::Pat::Type(typed) => {
                        matches!(typed.pat.as_ref(), syn::Pat::Ident(pattern) if pattern.ident == name)
                    }
                    syn::Pat::Const(_) | syn::Pat::Guard(_) | syn::Pat::Lit(_) | syn::Pat::Macro(_)
                    | syn::Pat::Or(_) | syn::Pat::Paren(_) | syn::Pat::Path(_) | syn::Pat::Range(_)
                    | syn::Pat::Reference(_) | syn::Pat::Rest(_) | syn::Pat::Slice(_) | syn::Pat::Struct(_)
                    | syn::Pat::Tuple(_) | syn::Pat::TupleStruct(_) | syn::Pat::Verbatim(_) | syn::Pat::Wild(_) | _ => false,
                }
            {
                Some((index, binding))
            } else {
                None
            }
        });
    let first = found.next().ok_or_else(|| shape(anchor.owner))?;
    if found.next().is_some() {
        return Err(shape(anchor.owner));
    }
    Ok(first)
}

fn statement(
    block: &syn::Block,
    anchor: SourceAnchor,
    ordinal: usize,
    expected: &syn::Stmt,
) -> Result<(), Refusal> {
    same(
        anchor.owner,
        block
            .stmts
            .get(ordinal)
            .ok_or_else(|| shape(anchor.owner))?,
        expected,
    )
}

fn exact_body(
    matched: &[Matched],
    anchor: SourceAnchor,
    supported: &syn::Block,
) -> Result<(), Refusal> {
    same(anchor.owner, body(matched, anchor)?, supported)
}

fn constant(
    sources: &ParsedSources,
    unit: CallerUnit,
    supported: syn::ItemConst,
) -> Result<u64, Refusal> {
    let owner = Owner::Caller(unit);
    let actual = only(
        sources.file(owner)?.items.iter().filter_map(|item| {
            if let syn::Item::Const(value) = item
                && value.ident == supported.ident
            {
                Some(value)
            } else {
                None
            }
        }),
        owner,
    )?;
    same(
        owner,
        &without_docs(syn::Item::Const(actual.clone())),
        &syn::Item::Const(supported),
    )?;
    let syn::Expr::Lit(value) = actual.expr.as_ref() else {
        return Err(shape(owner));
    };
    let syn::Lit::Int(integer) = &value.lit else {
        return Err(shape(owner));
    };
    integer.base10_parse().map_err(|_error| shape(owner))
}

// This is only a lexical import-binding projection for the finite caller
// recipes, not general Rust resolution. The target modules are matched below.
fn import_leaves(tree: &syn::UseTree, prefix: &str, leaves: &mut Vec<(String, String)>) {
    let join = |name: &str| {
        if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}::{name}")
        }
    };
    match tree {
        syn::UseTree::Path(path) => {
            import_leaves(&path.tree, &join(&path.ident.to_string()), leaves);
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                import_leaves(item, prefix, leaves);
            }
        }
        syn::UseTree::Name(name) => {
            leaves.push((name.ident.to_string(), join(&name.ident.to_string())));
        }
        syn::UseTree::Rename(rename) => {
            leaves.push((rename.rename.to_string(), join(&rename.ident.to_string())));
        }
        syn::UseTree::Glob(_) => leaves.push(("*".into(), join("*"))),
    }
}

fn imports(sources: &ParsedSources, owner: Owner, wanted: &[(&str, &str)]) -> Result<(), Refusal> {
    let file = sources.file(owner)?;
    for (name, target) in wanted {
        let mut found = Vec::new();
        for item in &file.items {
            let syn::Item::Use(import) = item else {
                continue;
            };
            let mut leaves = Vec::new();
            import_leaves(&import.tree, "", &mut leaves);
            for (local, destination) in leaves {
                if local == "*" && destination != "auctioneer_config::source::*" {
                    return Err(shape(owner));
                }
                if local == *name {
                    let public = matches!(
                        owner,
                        Owner::Caller(
                            CallerUnit::CoreAuctionRoot
                                | CallerUnit::OpenRtbRoot
                                | CallerUnit::RuntimeRoot
                                | CallerUnit::ExchangeRoot
                        )
                    );
                    let visibility_matches = if public {
                        matches!(import.vis, syn::Visibility::Public(_))
                    } else {
                        matches!(import.vis, syn::Visibility::Inherited)
                    };
                    if import.leading_colon.is_some()
                        || !import.attrs.is_empty()
                        || !visibility_matches
                    {
                        return Err(shape(owner));
                    }
                    found.push(destination);
                }
            }
        }
        if found != [target.to_string()] {
            return Err(shape(owner));
        }
        // A local definition cannot redirect an explicitly bound recipe name.
        if file.items.iter().any(|item| match item {
            syn::Item::Fn(value) => value.sig.ident == *name,
            syn::Item::Type(value) => value.ident == *name,
            syn::Item::Struct(value) => value.ident == *name,
            syn::Item::Enum(value) => value.ident == *name,
            syn::Item::Const(value) => value.ident == *name,
            syn::Item::Static(value) => value.ident == *name,
            syn::Item::Mod(value) => {
                value.ident == *name
                    && !(owner == Owner::Caller(CallerUnit::OpenRtbRoot) && *name == "prepare")
            }
            syn::Item::ExternCrate(_)
            | syn::Item::ForeignMod(_)
            | syn::Item::Impl(_)
            | syn::Item::Macro(_)
            | syn::Item::Trait(_)
            | syn::Item::TraitAlias(_)
            | syn::Item::Union(_)
            | syn::Item::Use(_)
            | syn::Item::Verbatim(_)
            | _ => false,
        }) {
            return Err(shape(owner));
        }
    }
    Ok(())
}

fn module(
    sources: &ParsedSources,
    unit: CallerUnit,
    expected: &syn::ItemMod,
) -> Result<(), Refusal> {
    let owner = Owner::Caller(unit);
    let actual = only(
        sources.file(owner)?.items.iter().filter_map(|item| {
            if let syn::Item::Mod(module) = item
                && module.ident == expected.ident
            {
                Some(module)
            } else {
                None
            }
        }),
        owner,
    )?;
    same(owner, actual, expected)
}

fn core_bindings(sources: &ParsedSources) -> Result<(), Refusal> {
    use CallerUnit::{CoreOperation, CorePrepare, CoreServices};
    imports(
        sources,
        Owner::Caller(CorePrepare),
        &[
            ("PreparedAuction", "super::operation::PreparedAuction"),
            ("PreparedState", "super::operation::PreparedState"),
            ("AuctionFunding", "super::AuctionFunding"),
            ("AuctionServices", "super::AuctionServices"),
            ("AuctioneerAppConfig", "crate::config::AuctioneerAppConfig"),
            (
                "resolve_policy_host",
                "crate::config::policy_host::resolve_policy_host",
            ),
            ("prepare", "auctioneer_openrtb::prepare"),
            ("PrepareLimits", "auctioneer_openrtb::PrepareLimits"),
            ("preflight", "auctioneer_runtime::json_preflight::preflight"),
            ("bounded_json", "auctioneer_runtime::bounded_json"),
        ],
    )?;
    imports(
        sources,
        Owner::Caller(CoreServices),
        &[
            (
                "NORMALIZATION_IMPLICIT_NODES",
                "auctioneer_runtime::budget::NORMALIZATION_IMPLICIT_NODES",
            ),
            (
                "RequestConstructionLimits",
                "auctioneer_exchange::context::RequestConstructionLimits",
            ),
            (
                "ResolvedIsolateLimits",
                "auctioneer_runtime::ResolvedIsolateLimits",
            ),
        ],
    )?;
    imports(
        sources,
        Owner::Caller(CoreOperation),
        &[
            ("run_auction", "auctioneer_exchange::run_auction"),
            ("AuctionContext", "auctioneer_exchange::AuctionContext"),
            (
                "NormalizedBidRequest",
                "auctioneer_openrtb::NormalizedBidRequest",
            ),
            ("AuctionServices", "super::AuctionServices"),
        ],
    )?;
    core_module_bindings(sources)
}

fn core_module_bindings(sources: &ParsedSources) -> Result<(), Refusal> {
    use CallerUnit::{CoreAuctionRoot, CoreConfigRoot};
    imports(
        sources,
        Owner::Caller(CoreAuctionRoot),
        &[
            ("AuctionServices", "services::AuctionServices"),
            ("AuctionFunding", "services::AuctionFunding"),
        ],
    )?;
    imports(
        sources,
        Owner::Caller(CoreConfigRoot),
        &[
            (
                "SharedAppConfig",
                "auctioneer_config::policy::OperatorPolicy",
            ),
            ("BidderSettings", "auctioneer_bidder::BidderSettings"),
        ],
    )?;
    for expected in [
        syn::parse_quote!(
            mod prepare;
        ),
        syn::parse_quote!(
            mod services;
        ),
        syn::parse_quote!(
            mod operation;
        ),
    ] {
        module(sources, CoreAuctionRoot, &expected)?;
    }
    for expected in [
        syn::parse_quote!(
            pub mod policy_host;
        ),
        syn::parse_quote!(
            pub mod resolve;
        ),
    ] {
        module(sources, CoreConfigRoot, &expected)?;
    }
    let file = sources.file(Owner::Caller(CoreConfigRoot))?;
    let alias = only(
        file.items.iter().filter_map(|item| {
            if let syn::Item::Type(alias) = item
                && alias.ident == "AuctioneerAppConfig"
            {
                Some(alias)
            } else {
                None
            }
        }),
        Owner::Caller(CoreConfigRoot),
    )?;
    let mut declaration = alias.clone();
    declaration
        .attrs
        .retain(|attribute| !literal_doc(attribute));
    let supported: syn::ItemType = syn::parse_quote! {
        pub type AuctioneerAppConfig = SharedAppConfig<BidderSettings>;
    };
    same(Owner::Caller(CoreConfigRoot), &declaration, &supported)
}

fn runtime_bindings(sources: &ParsedSources) -> Result<(), Refusal> {
    use CallerUnit::{ExchangeRoot, OpenRtbRoot, RuntimeRoot};
    imports(
        sources,
        Owner::Caller(OpenRtbRoot),
        &[
            ("prepare", "prepare::prepare"),
            ("PrepareLimits", "prepare::PrepareLimits"),
            ("NormalizedBidRequest", "normalized::NormalizedBidRequest"),
        ],
    )?;
    imports(
        sources,
        Owner::Caller(RuntimeRoot),
        &[
            ("ResolvedIsolateLimits", "budget::ResolvedIsolateLimits"),
            (
                "RequestConstructionCosts",
                "budget::RequestConstructionCosts",
            ),
            ("ResourceError", "auction_budget::ResourceError"),
        ],
    )?;
    imports(
        sources,
        Owner::Caller(ExchangeRoot),
        &[
            ("run_auction", "run::run_auction"),
            ("AuctionContext", "context::AuctionContext"),
        ],
    )?;
    module(
        sources,
        OpenRtbRoot,
        &syn::parse_quote!(
            pub mod prepare;
        ),
    )?;
    module(
        sources,
        OpenRtbRoot,
        &syn::parse_quote!(
            mod normalized;
        ),
    )?;
    module(
        sources,
        RuntimeRoot,
        &syn::parse_quote!(
            pub mod budget;
        ),
    )?;
    module(
        sources,
        ExchangeRoot,
        &syn::parse_quote!(
            pub mod context;
        ),
    )?;
    module(
        sources,
        ExchangeRoot,
        &syn::parse_quote!(
            pub mod run;
        ),
    )?;
    runtime_leaf_bindings(sources)
}

fn runtime_leaf_bindings(sources: &ParsedSources) -> Result<(), Refusal> {
    imports(
        sources,
        Owner::Caller(CallerUnit::PolicyHost),
        &[
            ("clamp_host_to_boot", "super::resolve::clamp_host_to_boot"),
            (
                "apply_resource_overrides",
                "super::resolve::apply_resource_overrides",
            ),
        ],
    )?;
    imports(
        sources,
        Owner::Caller(CallerUnit::NormalizedPrepare),
        &[
            ("BidRequest", "crate::types::bid_request::BidRequest"),
            (
                "NormalizedBidRequest",
                "crate::normalized::NormalizedBidRequest",
            ),
        ],
    )?;
    imports(
        sources,
        Owner::Context(super::SourceRole::ExchangeRun),
        &[
            (
                "apply_max_bidder_names",
                "crate::limits::apply_max_bidder_names",
            ),
            (
                "materialize_per_bidder",
                "crate::split::materialize_per_bidder",
            ),
            ("group_by_bidder", "crate::split::group_by_bidder"),
            (
                "RequestConstructionLimits",
                "crate::context::RequestConstructionLimits",
            ),
        ],
    )
}

fn core_rules() -> Vec<Rule> {
    use CallerUnit::{CorePrepare, CoreServices, ImplicitFill, NormalizedPrepare};
    vec![
        free_rule(
            caller_anchor(CorePrepare, "prepare_auction"),
            syn::parse_quote! {
                pub fn prepare_auction<'host>(
                    settings: &AuctioneerAppConfig, input: AuctionInput<'_>,
                    host: AuctionServices<'host>, funding: AuctionFunding,
                ) -> Result<PreparedAuction<'host>, AuctionFailure> {}
            },
        ),
        free_rule(
            caller_anchor(CorePrepare, "prepare_state"),
            syn::parse_quote! {
                fn prepare_state(
                    settings: &AuctioneerAppConfig, input: AuctionInput<'_>,
                    services: &AuctionServices<'_>, funding: &AuctionFunding,
                    debug_requested: &mut bool,
                ) -> Result<PreparedState, AuctionFailureReason> {}
            },
        ),
        free_rule(
            caller_anchor(CorePrepare, "admit_input"),
            syn::parse_quote! {
                fn admit_input<'input>(
                    settings: &AuctioneerAppConfig, input: AuctionInput<'input>,
                    host: &HostConfig, services: &AuctionServices<'_>,
                ) -> Result<AdmittedInput<'input>, AuctionFailureReason> {}
            },
        ),
        free_rule(
            caller_anchor(CorePrepare, "normalization_limits"),
            syn::parse_quote! {
                fn normalization_limits(limits: &auctioneer_runtime::ResolvedIsolateLimits) -> PrepareLimits {}
            },
        ),
        free_rule(
            caller_anchor(NormalizedPrepare, "prepare"),
            syn::parse_quote! {
                #[inline]
                pub fn prepare(mut req: BidRequest, headers: &RequestHeaders<'_>, limits: &PrepareLimits)
                    -> Result<NormalizedBidRequest, ValidationError> {}
            },
        ),
        free_rule(
            caller_anchor(ImplicitFill, "run"),
            syn::parse_quote! {
                pub(super) fn run(req: &mut BidRequest, headers: &RequestHeaders<'_>, max_copied_header_bytes: u32) {}
            },
        ),
        method_rule(
            caller_anchor(CoreServices, "AuctionFunding::ensure"),
            syn::parse_quote! {
                impl AuctionFunding {
                    pub(crate) fn ensure(&self, profile: &AuctionResourceProfile) -> Result<(), ResourceError> {}
                }
            },
        ),
        method_rule(
            caller_anchor(CoreServices, "AuctionResourceProfile::construction_limits"),
            syn::parse_quote! {
                impl AuctionResourceProfile {
                    pub(crate) fn construction_limits(&self) -> Result<RequestConstructionLimits, ResourceError> {}
                }
            },
        ),
        free_rule(
            caller_anchor(CoreServices, "normalized_construction_limits"),
            syn::parse_quote! {
                fn normalized_construction_limits(
                    input_nodes: u64, inbound_bytes: u64, copied_header_bytes: u64,
                    costs: RequestConstructionCosts,
                ) -> Result<RequestConstructionLimits, ResourceError> {}
            },
        ),
    ]
}

fn policy_rules() -> Vec<Rule> {
    vec![
        free_rule(
            caller_anchor(CallerUnit::PolicyHost, "resolve_policy_host"),
            syn::parse_quote! {
                #[must_use]
                pub fn resolve_policy_host(product: &HostConfig, policy: &OperatorPolicy, funded: &ResolvedIsolateLimits)
                    -> HostConfig {}
            },
        ),
        free_rule(
            caller_anchor(CallerUnit::PolicyHost, "requested_policy_host"),
            syn::parse_quote! {
                pub(super) fn requested_policy_host(product: &HostConfig, overlay: &HostPolicy) -> HostConfig {}
            },
        ),
        free_rule(
            caller_anchor(CallerUnit::PolicyResolve, "apply_resource_overrides"),
            syn::parse_quote! {
                pub(super) fn apply_resource_overrides(base: &mut ResourceLimits, requested: &AuctionConfig) {}
            },
        ),
        free_rule(
            caller_anchor(CallerUnit::PolicyResolve, "clamp_host_to_boot"),
            syn::parse_quote! {
                pub(crate) fn clamp_host_to_boot(host: &mut HostConfig, boot: &auctioneer_runtime::ResolvedIsolateLimits) {}
            },
        ),
    ]
}

fn operation_rules() -> Vec<Rule> {
    use super::SourceRole::ExchangeRun;
    vec![
        method_rule(
            caller_anchor(CallerUnit::CoreOperation, "PreparedAuction::run"),
            syn::parse_quote! {
                impl<'host> PreparedAuction<'host> {
                    pub async fn run(mut self) -> Result<CompletedAuction, AuctionFailure> {}
                }
            },
        ),
        method_rule(
            caller_anchor(
                CallerUnit::ExchangeContext,
                "RequestConstructionLimits::ensure",
            ),
            syn::parse_quote! {
                impl RequestConstructionLimits {
                    pub fn ensure(&self, request: &BidRequest) -> Result<(), crate::AuctionError> {}
                }
            },
        ),
        method_rule(
            caller_anchor(
                CallerUnit::ExchangeContext,
                "RequestConstructionLimits::with_supplement",
            ),
            syn::parse_quote! {
                impl RequestConstructionLimits {
                    pub(crate) fn with_supplement(self, nodes: u32, bytes: u64) -> Result<Self, crate::AuctionError> {}
                }
            },
        ),
        free_rule(
            context_anchor(ExchangeRun, "scrub_bidder_requests"),
            syn::parse_quote! {
                fn scrub_bidder_requests(
                    per_bidder: &mut [PerBidderRequest], decisions: &BTreeMap<BidderName, BidderDecision>,
                    keep_bits: IpKeepBits, audit: &mut DisclosureAudit, limits: Option<RequestConstructionLimits>,
                    clock: &dyn auctioneer_runtime::Clock, deadline_at: auctioneer_runtime::MonotonicInstant,
                ) -> Result<bool, AuctionError> {}
            },
        ),
        free_rule(
            context_anchor(ExchangeRun, "run_auction"),
            syn::parse_quote! {
                #[expect(
                    clippy::too_many_lines,
                    reason = "auction driver walks the fixed step sequence; helpers are already extracted"
                )]
                pub async fn run_auction(
                    req: &NormalizedBidRequest, ctx: &AuctionContext<'_>, plan: &AuctionPlan,
                    audit: &mut DisclosureAudit,
                ) -> Result<AuctionOutcome, AuctionError> {}
            },
        ),
    ]
}

fn exchange_rules() -> Vec<Rule> {
    use super::SourceRole::{ExchangeLimits, ExchangeRun, ExchangeSchema, ExchangeSplit};
    vec![
        free_rule(
            context_anchor(ExchangeLimits, "apply_max_bidder_names"),
            syn::parse_quote! {
                pub(crate) fn apply_max_bidder_names(names: Vec<BidderName>, cap: usize)
                    -> (Vec<BidderName>, Vec<BidderCapDrop>) {}
            },
        ),
        free_rule(
            context_anchor(ExchangeLimits, "cap_entries"),
            syn::parse_quote! {
                fn cap_entries<T>(mut per_bidder: Vec<T>, cap: usize, bidder: impl Fn(T) -> BidderName)
                    -> (Vec<T>, Vec<BidderCapDrop>) {}
            },
        ),
        free_rule(
            context_anchor(ExchangeSplit, "group_by_bidder"),
            syn::parse_quote! {
                pub fn group_by_bidder(
                    req: &BidRequest, aliases: &AliasMap, registry: &BidderRegistry,
                    disabled: &BTreeSet<BidderName>, dropped_alias_names: &BTreeSet<BidderName>,
                ) -> Result<GroupedBidders, AuctionError> {}
            },
        ),
        free_rule(
            context_anchor(ExchangeSplit, "group_one_imp"),
            syn::parse_quote! {
                fn group_one_imp(
                    imp_idx: usize, imp: &Value, inputs: &SplitInputs<'_>,
                    disabled: &BTreeSet<BidderName>,
                    per_bidder_imps: &mut BTreeMap<BidderName, Vec<usize>>,
                    disabled_bidder_warnings: &mut Vec<(BidderName, ErrorEntry)>,
                ) -> Result<(), AuctionError> {}
            },
        ),
        free_rule(
            context_anchor(ExchangeSplit, "materialize_per_bidder"),
            syn::parse_quote! {
                pub fn materialize_per_bidder(
                    grouped: &GroupedBidders, survivors: &[BidderName], aliases: &AliasMap,
                    registry: &BidderRegistry, dropped_alias_names: &BTreeSet<BidderName>,
                ) -> Result<Vec<PerBidderRequest>, AuctionError> {}
            },
        ),
        free_rule(
            context_anchor(ExchangeRun, "run_make_requests"),
            syn::parse_quote! {
                async fn run_make_requests(
                    ctx: &AuctionContext<'_>, underlying: &BidRequest, pbr: &mut PerBidderRequest,
                    gpc: Option<&str>, tmax: &TmaxResolution, projection: LegacyProjection<'_>,
                    out: &mut MakeRequestsOutput,
                ) {}
            },
        ),
        free_rule(
            context_anchor(ExchangeSchema, "admit"),
            syn::parse_quote! {
                pub(super) fn admit(ctx: &AuctionContext<'_>, pbr: &mut PerBidderRequest,
                    tmax: &TmaxResolution, out: &mut MakeRequestsOutput) -> bool {}
            },
        ),
        free_rule(
            context_anchor(ExchangeSchema, "retain_valid"),
            syn::parse_quote! {
                fn retain_valid<F>(request: &mut BidRequest, errors: &mut Vec<ErrorEntry>, mut validate: F)
                where F: FnMut(&Value) -> Result<(), NativeSchemaError>, {}
            },
        ),
    ]
}

fn normalize_signature(signature: &mut syn::Signature) {
    signature.inputs = mem::take(&mut signature.inputs).into_iter().collect();
    signature.generics.params = mem::take(&mut signature.generics.params)
        .into_iter()
        .collect();
    if let Some(clause) = &mut signature.generics.where_clause {
        clause.predicates = mem::take(&mut clause.predicates).into_iter().collect();
    }
}

fn prepare_gate() -> syn::Block {
    syn::parse_quote! {{
        let mut debug_requested = host.debug_requested;
        let state = match prepare_state(settings, input, &host, &funding, &mut debug_requested) {
            Ok(state) => state,
            Err(reason) => {
                return Err(AuctionFailure::new(reason, funding).with_debug_requested(debug_requested));
            }
        };
        Ok(PreparedAuction::new(state, host, funding))
    }}
}

fn admission_gate() -> syn::Block {
    syn::parse_quote! {{
        let body_cap = services.resources.inputs().limits.inbound_request_bytes
            .min(u64::try_from(MAX_AUCTION_BODY).unwrap_or(u64::MAX));
        let max_nodes = host.resource_limits.max_parsed_nodes;
        match input {
            AuctionInput::Json { bytes, headers } => {
                if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > body_cap {
                    return Err(AuctionFailureReason::invalid(&"request body too large"));
                }
                let (_, original_ext) = bounded_json::count_json_nodes_with_field(bytes, max_nodes, "ext")
                    .map_err(node_error)?;
                let account = resolve::resolve_account(bytes, settings, services.product_account, max_nodes)
                    .map_err(AuctionFailureReason::Engine)?;
                let raw = serde_json::from_slice(bytes)
                    .map_err(|error| AuctionFailureReason::invalid(&error))?;
                Ok(AdmittedInput { raw, account, original_ext, headers, })
            }
            AuctionInput::Typed { request, headers } => {
                preflight(request, u64::from(max_nodes), MAX_CONSTRUCTION_DEPTH)
                    .map_err(|error| AuctionFailureReason::invalid(&error))?;
                let bytes = serialized_json_bytes(request)
                    .map_err(|error| AuctionFailureReason::invalid(&error))?;
                if bytes > body_cap {
                    return Err(AuctionFailureReason::invalid(&"request body too large"));
                }
                let account = resolve::resolve_account_from_request(request, settings, services.product_account)
                    .map_err(AuctionFailureReason::Engine)?;
                Ok(AdmittedInput {
                    raw: request.clone(), account,
                    original_ext: bounded_json::JsonFieldShape::Unavailable, headers,
                })
            }
        }
    }}
}

fn normalization_gate() -> syn::Block {
    syn::parse_quote! {{
        let got = u32::try_from(req.imp.len()).unwrap_or(u32::MAX);
        if got > limits.max_impressions {
            return Err(ValidationError::TooManyImpressions { got, max: limits.max_impressions, });
        }
        implicit_fill::run(&mut req, headers, limits.max_copied_header_bytes);
        req.validate()?;
        checks::reject_legacy_25_privacy(&req)?;
        checks::reject_empty_channel_name(&req)?;
        Ok(NormalizedBidRequest { inner: req, warnings: Vec::new(), })
    }}
}

fn implicit_gate() -> syn::Block {
    syn::parse_quote! {{
        let combined = usize::try_from(max_copied_header_bytes).unwrap_or(usize::MAX);
        let per_field = combined.div_euclid(2);
        let mut page_budget = per_field;
        let mut ua_budget = combined.saturating_sub(per_field);
        site_default(req, headers, &mut page_budget, combined);
        fill_device(req, headers, &mut ua_budget);
        fill_at(req);
        fill_channel(req);
        ios_lmt_override(req);
        gpc(req, headers);
    }}
}

fn prefix(block: &syn::Block, anchor: SourceAnchor, supported: syn::Block) -> Result<(), Refusal> {
    for (ordinal, expected) in supported.stmts.into_iter().enumerate() {
        statement(block, anchor, ordinal, &expected)?;
    }
    Ok(())
}

fn core_gates(matched: &[Matched]) -> Result<(), Refusal> {
    use CallerUnit::{CorePrepare, ImplicitFill, NormalizedPrepare};
    exact_body(
        matched,
        caller_anchor(CorePrepare, "prepare_auction"),
        &prepare_gate(),
    )?;
    exact_body(
        matched,
        caller_anchor(CorePrepare, "admit_input"),
        &admission_gate(),
    )?;
    exact_body(
        matched,
        caller_anchor(NormalizedPrepare, "prepare"),
        &normalization_gate(),
    )?;
    exact_body(
        matched,
        caller_anchor(ImplicitFill, "run"),
        &implicit_gate(),
    )?;
    let anchor = caller_anchor(CorePrepare, "prepare_state");
    prefix(
        body(matched, anchor)?,
        anchor,
        syn::parse_quote! {{
            funding.ensure(services.resources).map_err(AuctionFailureReason::Resource)?;
            if services.deadline_spent() { return Err(AuctionFailureReason::Deadline); }
            if !services.bindings_match() {
                return Err(AuctionFailureReason::Unavailable(
                    "bidder implementation bindings do not match registry",
                ));
            }
            let limits = &services.resources.inputs().limits;
            let host = resolve_policy_host(services.product_host, settings, limits);
            let AdmittedInput { raw, account, original_ext, headers, } =
                admit_input(settings, input, &host, services)?;
            let request = prepare(raw, &headers, &normalization_limits(limits))
                .map_err(|error| AuctionFailureReason::invalid(&error))?;
        }},
    )
}

fn normalization_limits_gate() -> syn::Block {
    syn::parse_quote! {{
        PrepareLimits {
            max_impressions: u32::from(
                limits.max_impressions_per_auction.min(MAX_AUCTION_IMPRESSIONS),
            ),
            max_copied_header_bytes: limits.max_copied_header_bytes,
        }
    }}
}

fn construction_limits_gate() -> syn::Block {
    syn::parse_quote! {{
        let limits = &self.inputs.limits;
        normalized_construction_limits(
            u64::from(limits.max_parsed_nodes),
            limits.inbound_request_bytes,
            u64::from(limits.max_copied_header_bytes),
            self.inputs.construction_costs,
        )
    }}
}

fn normalized_construction_gate() -> syn::Block {
    syn::parse_quote! {{
        let max_nodes = input_nodes.checked_add(NORMALIZATION_IMPLICIT_NODES)
            .ok_or(ResourceError::CapacityOverflow)?;
        let max_serialized_bytes =
            normalization_wire_allowance(inbound_bytes, copied_header_bytes, input_nodes)
                .map_err(|_error| ResourceError::CapacityOverflow)?;
        Ok(RequestConstructionLimits {
            max_nodes, max_serialized_bytes,
            reusable_scratch_bytes: costs.reusable_scratch_bytes,
            retained_per_scope_bytes: costs.retained_per_scope_bytes,
        })
    }}
}

fn scoped_construction_gate() -> syn::Block {
    syn::parse_quote! {{
        Ok(Self {
            max_nodes: self.max_nodes.checked_add(u64::from(nodes))
                .ok_or(auctioneer_runtime::ResourceError::CapacityOverflow)?,
            max_serialized_bytes: self.max_serialized_bytes.checked_add(bytes)
                .ok_or(auctioneer_runtime::ResourceError::CapacityOverflow)?,
            reusable_scratch_bytes: self.reusable_scratch_bytes,
            retained_per_scope_bytes: self.retained_per_scope_bytes,
        })
    }}
}

fn scoped_ensure_gate() -> syn::Block {
    syn::parse_quote! {{
        let refuse = || crate::AuctionError::Internal("request construction limit exceeded".into());
        preflight(request, self.max_nodes, MAX_CONSTRUCTION_DEPTH).map_err(|_error| refuse())?;
        let bytes = serialized_json_bytes(request).map_err(|_error| refuse())?;
        if bytes > self.max_serialized_bytes { return Err(refuse()); }
        Ok(())
    }}
}

fn storage_gates(matched: &[Matched]) -> Result<(), Refusal> {
    use CallerUnit::{CoreServices, ExchangeContext};
    exact_body(
        matched,
        caller_anchor(CoreServices, "AuctionResourceProfile::construction_limits"),
        &construction_limits_gate(),
    )?;
    exact_body(
        matched,
        caller_anchor(CoreServices, "normalized_construction_limits"),
        &normalized_construction_gate(),
    )?;
    exact_body(
        matched,
        caller_anchor(
            ExchangeContext,
            "RequestConstructionLimits::with_supplement",
        ),
        &scoped_construction_gate(),
    )?;
    exact_body(
        matched,
        caller_anchor(ExchangeContext, "RequestConstructionLimits::ensure"),
        &scoped_ensure_gate(),
    )
}

fn clamp_gate() -> syn::Block {
    syn::parse_quote! {{
        let limits = &mut host.resource_limits;
        limits.max_bidders_per_auction = limits.max_bidders_per_auction
            .min(usize::try_from(boot.max_bidders_per_auction).unwrap_or(usize::MAX));
        limits.max_outbound_requests = limits.max_outbound_requests
            .min(usize::try_from(boot.max_outbound_requests).unwrap_or(usize::MAX));
        limits.max_bidder_response_bytes = limits.max_bidder_response_bytes
            .min(usize::try_from(boot.max_decoded_bytes).unwrap_or(usize::MAX));
        limits.max_outbound_raw_bytes = limits.max_outbound_raw_bytes.min(boot.max_outbound_raw_bytes);
        limits.max_outbound_request_bytes = limits.max_outbound_request_bytes.min(boot.max_outbound_request_bytes);
        limits.max_transport_bytes = limits.max_transport_bytes.min(boot.max_transport_bytes);
        limits.max_decoded_bytes = limits.max_decoded_bytes.min(boot.max_decoded_bytes);
        limits.max_parsed_nodes = limits.max_parsed_nodes.min(boot.max_parsed_nodes);
    }}
}

fn policy_gates(matched: &[Matched]) -> Result<(), Refusal> {
    exact_body(
        matched,
        caller_anchor(CallerUnit::PolicyResolve, "clamp_host_to_boot"),
        &clamp_gate(),
    )?;
    exact_body(
        matched,
        caller_anchor(CallerUnit::PolicyHost, "resolve_policy_host"),
        &syn::parse_quote! {{
            let mut host = requested_policy_host(product, &policy.host);
            clamp_host_to_boot(&mut host, funded);
            host
        }},
    )?;
    let anchor = caller_anchor(CallerUnit::PolicyHost, "requested_policy_host");
    let block = body(matched, anchor)?;
    prefix(
        block,
        anchor,
        syn::parse_quote! {{
            let mut host = product.clone();
            apply_resource_overrides(&mut host.resource_limits, &overlay.resource_limits);
        }},
    )?;
    let expected = syn::Stmt::Expr(syn::parse_quote!(host), None);
    same(
        anchor.owner,
        block.stmts.last().ok_or_else(|| shape(anchor.owner))?,
        &expected,
    )
}

fn cap_gate() -> syn::Block {
    syn::parse_quote! {{
        if cap == 0 || per_bidder.len() <= cap { return (per_bidder, vec![]); }
        let dropped = per_bidder.split_off(cap);
        let drops = dropped.into_iter().map(|entry| BidderCapDrop {
            bidder: bidder(entry), cap,
        }).collect();
        (per_bidder, drops)
    }}
}

fn scope_gate() -> syn::Block {
    syn::parse_quote! {{
        for pbr in per_bidder {
            if clock.now_monotonic() >= deadline_at { return Ok(false); }
            if let Some(scope_limits) = limits
                && let Err(error) = scope_limits.ensure(&pbr.request)
            {
                audit.start_scope();
                audit.fail_scope();
                return Err(error);
            }
            let decision = decisions.get(&pbr.bidder)
                .ok_or_else(|| AuctionError::Internal("missing survivor privacy decision".into()))?;
            let (pass_id, pass_geo) = match decision.gdpr {
                AuctionPermissions::AllowAll => (true, true),
                AuctionPermissions::Partial { pass_id, pass_geo, .. } => (pass_id, pass_geo),
                AuctionPermissions::DenyBidRequest => (false, false),
            };
            let disclosure = ScrubDecisions {
                pass_id: pass_id && decision.allow_host_ids, pass_geo,
                transmit_ufpd: decision.transmit_ufpd,
                transmit_precise_geo: decision.transmit_precise_geo,
                transmit_tid: decision.transmit_tid, ccpa: decision.ccpa,
                lmt: decision.lmt, coppa: decision.coppa,
            };
            match limits {
                Some(scope_limits) => apply_classified_with_limits(
                    &mut pbr.request, disclosure, keep_bits, audit,
                    scope_limits.max_nodes, MAX_CONSTRUCTION_DEPTH,
                )?,
                None => apply_classified(&mut pbr.request, disclosure, keep_bits, audit)?,
            }
        }
        Ok(clock.now_monotonic() < deadline_at)
    }}
}

fn run_locals(matched: &[Matched]) -> Result<(), Refusal> {
    use super::SourceRole::ExchangeRun;
    let anchor = context_anchor(ExchangeRun, "run_auction");
    let block = body(matched, anchor)?;
    let (cap_at, cap) = local(block, anchor, "cap")?;
    let wanted_cap: syn::Stmt = syn::parse_quote! {
        let cap = ctx.host.resource_limits.max_bidders_per_auction.max(1);
    };
    same(anchor.owner, cap, &wanted_cap)?;
    let (capped_at, capped) = local(block, anchor, "capped")?;
    let wanted_capped: syn::Stmt = syn::parse_quote! {
        let mut capped = materialize_per_bidder(
            &grouped, &survivors, aliases, ctx.bidders, &plan.aliases.dropped_alias_names,
        )?;
    };
    same(anchor.owner, capped, &wanted_capped)?;
    let survivor_pattern: syn::Pat = syn::parse_quote!((survivors, bidder_cap_drops));
    let mut survivor_position = None;
    for (ordinal, statement) in block.stmts.iter().enumerate() {
        if let syn::Stmt::Local(binding) = statement
            && tokens(&binding.pat) == tokens(&survivor_pattern)
        {
            if survivor_position.is_some() {
                return Err(shape(anchor.owner));
            }
            let supported_survivors: syn::Stmt = syn::parse_quote! {
                let (survivors, bidder_cap_drops) = apply_max_bidder_names(sampled, cap);
            };
            same(anchor.owner, binding, &supported_survivors)?;
            survivor_position = Some(ordinal);
        }
    }
    let survivor_at = survivor_position.ok_or_else(|| shape(anchor.owner))?;
    if cap_at >= survivor_at || survivor_at >= capped_at {
        return Err(shape(anchor.owner));
    }
    let (scoped_at, scoped) = local(block, anchor, "scoped_limits")?;
    let supported_scoped: syn::Stmt = syn::parse_quote! {
        let scoped_limits = ctx.construction_limits.map(|limits| {
            limits.with_supplement(
                ctx.host.resource_limits.max_parsed_nodes,
                ctx.host.resource_limits.max_outbound_raw_bytes,
            )
        }).transpose()?;
    };
    same(anchor.owner, scoped, &supported_scoped)?;
    if capped_at >= scoped_at {
        return Err(shape(anchor.owner));
    }
    let loops: Vec<_> = block
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| {
            if let syn::Stmt::Expr(syn::Expr::ForLoop(loop_), None) = statement
                && tokens(&loop_.pat) == "pbr"
            {
                Some((ordinal, statement))
            } else {
                None
            }
        })
        .collect();
    let [(loop_at, loop_statement)] = loops.as_slice() else {
        return Err(shape(anchor.owner));
    };
    let supported_loop: syn::Stmt = syn::parse_quote! {
        for pbr in &mut capped {
            if ctx.clock.now_monotonic() >= deadline.deadline_at { break; }
            run_make_requests(
                ctx, underlying, pbr, gpc, &resolved, projection, &mut make_requests_output,
            ).await;
        }
    };
    same(anchor.owner, *loop_statement, &supported_loop)?;
    if scoped_at >= *loop_at {
        return Err(shape(anchor.owner));
    }
    Ok(())
}

fn schema_scope_gates(matched: &[Matched]) -> Result<Vec<GuardSource>, Refusal> {
    use super::SourceRole::{ExchangeRun, ExchangeSchema};
    let anchor = context_anchor(ExchangeRun, "run_make_requests");
    let block = body(matched, anchor)?;
    statement(
        block,
        anchor,
        1,
        &syn::parse_quote! {
            let Some((info, adapter)) = ctx.bidders.get(pbr.base.as_str()) else { return; };
        },
    )?;
    let (configuration_at, configuration) = local(block, anchor, "has_configured_endpoint")?;
    let expected: syn::Stmt = syn::parse_quote! {
        let has_configured_endpoint = match bind_bidder_configuration(ctx, pbr, adapter, &mut info_for_adapter) {
            Ok(selected) => selected,
            Err(error) => {
                out.bidder_pre_errors.entry(pbr.bidder.clone()).or_default()
                    .push(ErrorEntry { code: error.code, message: error.message, });
                return;
            }
        };
    };
    same(anchor.owner, configuration, &expected)?;
    statement(
        block,
        anchor,
        configuration_at
            .checked_add(3)
            .ok_or_else(|| shape(anchor.owner))?,
        &syn::parse_quote! { if !schema::admit(ctx, pbr, tmax, out) { return; } },
    )?;
    let admit = context_anchor(ExchangeSchema, "admit");
    statement(
        body(matched, admit)?,
        admit,
        2,
        &syn::parse_quote! {
            if !mandatory && !funded_legacy { return true; }
        },
    )?;
    statement(
        body(matched, admit)?,
        admit,
        4,
        &syn::parse_quote! {
            if ctx.schema_scratch_bytes < NATIVE_SCHEMA_SCRATCH_BYTES || ctx.projection_reservation.is_none() {
                errors.push(error(NativeSchemaError::Unavailable));
                return false;
            }
        },
    )?;
    let returned = syn::Stmt::Expr(syn::parse_quote!(!pbr.request.imp.is_empty()), None);
    same(
        admit.owner,
        body(matched, admit)?
            .stmts
            .last()
            .ok_or_else(|| shape(admit.owner))?,
        &returned,
    )?;
    Ok(vec![
        GuardSource {
            guard: ScopeGuard::DeadlineBeforeScope,
            source: context_anchor(ExchangeRun, "run_auction"),
        },
        GuardSource {
            guard: ScopeGuard::RegistryBeforeSchema,
            source: anchor,
        },
        GuardSource {
            guard: ScopeGuard::ConfigurationBeforeSchema,
            source: anchor,
        },
        GuardSource {
            guard: ScopeGuard::OptionalSchemaBypass,
            source: admit,
        },
        GuardSource {
            guard: ScopeGuard::FundingAndProjectionBeforeRetain,
            source: admit,
        },
        GuardSource {
            guard: ScopeGuard::WholeScopedRequestBeforeScrub,
            source: context_anchor(ExchangeRun, "scrub_bidder_requests"),
        },
        GuardSource {
            guard: ScopeGuard::RetainedNonemptyResult,
            source: admit,
        },
    ])
}

fn split_gates(matched: &[Matched]) -> Result<(), Refusal> {
    use super::SourceRole::{ExchangeLimits, ExchangeSplit};
    exact_body(
        matched,
        context_anchor(ExchangeLimits, "cap_entries"),
        &cap_gate(),
    )?;
    exact_body(
        matched,
        context_anchor(ExchangeLimits, "apply_max_bidder_names"),
        &syn::parse_quote! {{
            cap_entries(names, cap, |name| name)
        }},
    )?;
    let grouping = context_anchor(ExchangeSplit, "group_one_imp");
    let grouping_body = body(matched, grouping)?;
    let (_, names) = local(grouping_body, grouping, "bidders_on_imp")?;
    let expected_membership: syn::Stmt = syn::parse_quote! {
        let mut bidders_on_imp: BTreeSet<&str> = BTreeSet::new();
    };
    same(grouping.owner, names, &expected_membership)?;
    let materialization = context_anchor(ExchangeSplit, "materialize_per_bidder");
    let materialization_body = body(matched, materialization)?;
    let (_, output) = local(materialization_body, materialization, "out")?;
    let expected_capacity: syn::Stmt =
        syn::parse_quote!(let mut out = Vec::with_capacity(survivors.len()););
    same(materialization.owner, output, &expected_capacity)?;
    Ok(())
}

fn operation_gates(matched: &[Matched]) -> Result<(), Refusal> {
    let anchor = caller_anchor(CallerUnit::CoreOperation, "PreparedAuction::run");
    let block = body(matched, anchor)?;
    let (_, construction) = local(block, anchor, "construction")?;
    let expected: syn::Stmt = syn::parse_quote! {
        let construction = match services.resources.construction_limits() {
            Ok(limits) => limits,
            Err(error) => {
                return Err(AuctionFailure::new(AuctionFailureReason::Resource(error), self.funding,));
            }
        };
    };
    same(anchor.owner, construction, &expected)?;
    let (_, result) = local(block, anchor, "result")?;
    let initial = result.init.as_ref().ok_or_else(|| shape(anchor.owner))?;
    let syn::Expr::Block(scope) = initial.expr.as_ref() else {
        return Err(shape(anchor.owner));
    };
    let position = scope
        .block
        .stmts
        .iter()
        .position(|statement| {
            tokens(statement) == tokens(&quote!(context.construction_limits = Some(construction);))
        })
        .ok_or_else(|| shape(anchor.owner))?;
    let final_call = syn::Stmt::Expr(
        syn::parse_quote! {
            run_auction(&state.request, &context, &state.plan, audit).await
        },
        None,
    );
    same(
        anchor.owner,
        scope
            .block
            .stmts
            .last()
            .ok_or_else(|| shape(anchor.owner))?,
        &final_call,
    )?;
    if position.checked_add(1) != Some(scope.block.stmts.len().saturating_sub(1)) {
        return Err(shape(anchor.owner));
    }
    Ok(())
}

fn expressions(
    sources: &ParsedSources,
    matched: &[Matched],
) -> Result<(CapExpression, CapExpression, CapExpression), Refusal> {
    let impression_ceiling = constant(
        sources,
        CallerUnit::CorePrepare,
        syn::parse_quote! {
            pub(crate) const MAX_AUCTION_IMPRESSIONS: u16 = 100;
        },
    )?;
    let implicit_nodes = constant(
        sources,
        CallerUnit::RuntimeBudget,
        syn::parse_quote! {
            pub const NORMALIZATION_IMPLICIT_NODES: u64 = 30;
        },
    )?;
    exact_body(
        matched,
        caller_anchor(CallerUnit::CorePrepare, "normalization_limits"),
        &normalization_limits_gate(),
    )?;
    let run = context_anchor(super::SourceRole::ExchangeRun, "run_auction");
    let (_, cap) = local(body(matched, run)?, run, "cap")?;
    let initial = cap.init.as_ref().ok_or_else(|| shape(run.owner))?;
    let syn::Expr::MethodCall(maximum) = initial.expr.as_ref() else {
        return Err(shape(run.owner));
    };
    if maximum.args.len() != 1 {
        return Err(shape(run.owner));
    }
    let Some(syn::Expr::Lit(literal)) = maximum.args.first() else {
        return Err(shape(run.owner));
    };
    let syn::Lit::Int(integer) = &literal.lit else {
        return Err(shape(run.owner));
    };
    let floor = integer
        .base10_parse::<u64>()
        .map_err(|_error| shape(run.owner))?;
    Ok((
        CapExpression::Min(
            Box::new(CapExpression::Field(CapField::ResourceImpressions)),
            Box::new(CapExpression::Literal {
                source: caller_anchor(CallerUnit::CorePrepare, "MAX_AUCTION_IMPRESSIONS"),
                value: impression_ceiling,
            }),
        ),
        CapExpression::Max(
            Box::new(CapExpression::Min(
                Box::new(CapExpression::Field(CapField::RequestedHostBidders)),
                Box::new(CapExpression::Field(CapField::FundedBidders)),
            )),
            Box::new(CapExpression::Literal {
                source: run,
                value: floor,
            }),
        ),
        CapExpression::Add(
            Box::new(CapExpression::Add(
                Box::new(CapExpression::Field(CapField::CoreInputNodes)),
                Box::new(CapExpression::Literal {
                    source: caller_anchor(
                        CallerUnit::RuntimeBudget,
                        "NORMALIZATION_IMPLICIT_NODES",
                    ),
                    value: implicit_nodes,
                }),
            )),
            Box::new(CapExpression::Field(CapField::HostParsedNodes)),
        ),
    ))
}

// Known local source shapes produce DATA, conditional on the named helper
// semantics below. They do not close an actual caller/artifact or cost proof.
fn derive_caller_sources(
    context: &ContextSources<'_>,
    sources: &[CallerSource],
    _attempts: u64,
) -> Result<CallerContract, Refusal> {
    super::context::check_sources(context).map_err(Refusal::Context)?;
    let parsed = ParsedSources::new(context, sources)?;
    core_bindings(&parsed)?;
    runtime_bindings(&parsed)?;
    let matched = match_rules(&parsed)?;
    core_gates(&matched)?;
    policy_gates(&matched)?;
    storage_gates(&matched)?;
    operation_gates(&matched)?;
    split_gates(&matched)?;
    run_locals(&matched)?;
    exact_body(
        &matched,
        context_anchor(super::SourceRole::ExchangeRun, "scrub_bidder_requests"),
        &scope_gate(),
    )?;
    let scope_guards = schema_scope_gates(&matched)?;
    let (original_impressions_upper, effective_bidder_cap, scoped_nodes_upper) =
        expressions(&parsed, &matched)?;
    Ok(CallerContract {
        original_impressions_upper,
        effective_bidder_cap,
        scoped_nodes_upper,
        required_borrow: BorrowDuty::UnchangedBidderParametersFromScopedEnsure,
        parameter_borrow_preserved: None,
        source_chain: matched.iter().map(|function| function.anchor).collect(),
        scope_guards,
        cleanup_duties: vec![
            CleanupDuty::RejectedImpression,
            CleanupDuty::RetainErrorAndUnwind,
            CleanupDuty::ScopedRequestRelease,
            CleanupDuty::PartialMaterialization,
        ],
        unresolved_helpers: vec![
            "authentic parent and remaining type ancestry",
            "integer ordering/conversion semantics and costs",
            "typed JSON and normalization impression preservation",
            "Vec/BTreeSet/BTreeMap cardinality and iterator protocols",
            "scoped parameter preservation after ensure",
        ],
        panic_unwinds: None,
        cleanup_work: None,
        tree_navigation_work: None,
        compiled_artifact_binding: None,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::context::check_sources;
    use super::{
        BorrowDuty, CALLER_PATHS, CallerContract, CallerSource, CallerUnit, CapExpression,
        CapField, CleanupDuty, GuardSource, Owner, Refusal, ScopeGuard, SourceAnchor,
        derive_caller_sources, match_method, method_rule,
    };
    use crate::schema_build::application::CapturedSeed;
    use crate::schema_residency::descriptor::ValidationView;
    use crate::schema_validation::handoff::SourceInputs;
    use crate::schema_validation::work::{
        BUDGET, ContextBinding, ContextSources, DISPATCH_GENERATOR, EXCHANGE_LIMITS, EXCHANGE_RUN,
        EXCHANGE_SCHEMA, EXCHANGE_SPLIT, FORMATS, INPUT, JSON_SIZE, NATIVE, SourceRole,
        bind_attempt_context, context_source_paths,
    };
    use quote::{ToTokens as _, quote};
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::str::from_utf8;

    const CALLER_SOURCES: [(CallerUnit, &str, &[u8]); 14] = [
        (
            CallerUnit::CorePrepare,
            "crates/auctioneer-core/src/auction/prepare.rs",
            include_bytes!("../../../../auctioneer-core/src/auction/prepare.rs"),
        ),
        (
            CallerUnit::CoreServices,
            "crates/auctioneer-core/src/auction/services.rs",
            include_bytes!("../../../../auctioneer-core/src/auction/services.rs"),
        ),
        (
            CallerUnit::CoreOperation,
            "crates/auctioneer-core/src/auction/operation.rs",
            include_bytes!("../../../../auctioneer-core/src/auction/operation.rs"),
        ),
        (
            CallerUnit::PolicyHost,
            "crates/auctioneer-core/src/config/policy_host.rs",
            include_bytes!("../../../../auctioneer-core/src/config/policy_host.rs"),
        ),
        (
            CallerUnit::PolicyResolve,
            "crates/auctioneer-core/src/config/resolve.rs",
            include_bytes!("../../../../auctioneer-core/src/config/resolve.rs"),
        ),
        (
            CallerUnit::NormalizedPrepare,
            "crates/auctioneer-openrtb/src/prepare/mod.rs",
            include_bytes!("../../../../auctioneer-openrtb/src/prepare/mod.rs"),
        ),
        (
            CallerUnit::ImplicitFill,
            "crates/auctioneer-openrtb/src/prepare/implicit_fill.rs",
            include_bytes!("../../../../auctioneer-openrtb/src/prepare/implicit_fill.rs"),
        ),
        (
            CallerUnit::ExchangeContext,
            "crates/auctioneer-exchange/src/context.rs",
            include_bytes!("../../../../auctioneer-exchange/src/context.rs"),
        ),
        (
            CallerUnit::RuntimeBudget,
            "crates/auctioneer-runtime/src/budget.rs",
            include_bytes!("../../../../auctioneer-runtime/src/budget.rs"),
        ),
        (
            CallerUnit::CoreAuctionRoot,
            "crates/auctioneer-core/src/auction/mod.rs",
            include_bytes!("../../../../auctioneer-core/src/auction/mod.rs"),
        ),
        (
            CallerUnit::CoreConfigRoot,
            "crates/auctioneer-core/src/config/mod.rs",
            include_bytes!("../../../../auctioneer-core/src/config/mod.rs"),
        ),
        (
            CallerUnit::OpenRtbRoot,
            "crates/auctioneer-openrtb/src/lib.rs",
            include_bytes!("../../../../auctioneer-openrtb/src/lib.rs"),
        ),
        (
            CallerUnit::RuntimeRoot,
            "crates/auctioneer-runtime/src/lib.rs",
            include_bytes!("../../../../auctioneer-runtime/src/lib.rs"),
        ),
        (
            CallerUnit::ExchangeRoot,
            "crates/auctioneer-exchange/src/lib.rs",
            include_bytes!("../../../../auctioneer-exchange/src/lib.rs"),
        ),
    ];

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn sources() -> Vec<CallerSource> {
        CALLER_SOURCES
            .iter()
            .map(|&(unit, path, bytes)| CallerSource {
                unit,
                path,
                bytes: bytes.to_vec(),
                sha256: digest(bytes),
            })
            .collect()
    }

    fn context() -> ContextSources<'static> {
        ContextSources {
            native: NATIVE,
            budget: BUDGET,
            input: INPUT,
            json_size: JSON_SIZE,
            exchange_schema: EXCHANGE_SCHEMA,
            exchange_run: EXCHANGE_RUN,
            exchange_split: EXCHANGE_SPLIT,
            exchange_limits: EXCHANGE_LIMITS,
            dispatch_generator: DISPATCH_GENERATOR,
            formats: FORMATS,
        }
    }

    // Independent setup oracle only, never the implementation of the join.
    // Return diagnostics so the caller checks them after the window closes.
    pub(in crate::schema_validation) fn consumed_source_setup(
        inputs: &SourceInputs<'_>,
        seed: &CapturedSeed,
        view: ValidationView<'_>,
    ) -> Result<(), &'static str> {
        if inputs.source_count() != 44 {
            return Err("exact caller/context/primitive source census");
        }
        for unknown in [
            "mod.rs",
            "native.rs",
            "Nfa",
            "../src/schema/native.rs",
            "/crates/auctioneer-bidder/src/schema/native.rs",
        ] {
            if inputs.source_record(unknown).is_some() {
                return Err("undeclared or suffix-only source lookup");
            }
        }
        let frozen = context();
        for &(role, path) in context_source_paths() {
            let (absolute, bytes, sha256) = inputs.source_record(path).ok_or("context source")?;
            if !absolute.ends_with(path)
                || bytes != source_bytes(&frozen, &[], Owner::Context(role))
                || sha256 != digest(bytes)
            {
                return Err("context byte correspondence");
            }
            if role == SourceRole::Native {
                let matched = ContextSources {
                    native: bytes,
                    ..frozen
                };
                let limits =
                    check_sources(&matched).map_err(|_error| "matched native source limits")?;
                if (limits.serialized_bytes, limits.nodes, limits.depth) != (65_536, 4_096, 32) {
                    return Err("independent native input limits");
                }
            }
        }
        for (unit, path) in CALLER_PATHS {
            let (absolute, bytes, sha256) = inputs.source_record(path).ok_or("caller source")?;
            let mut matching = CALLER_SOURCES.iter().filter(|row| row.0 == unit);
            let frozen_row = matching.next().ok_or("caller fixture before-state")?;
            if matching.next().is_some()
                || frozen_row.1 != path
                || !absolute.ends_with(path)
                || frozen_row.2 != bytes
                || sha256 != digest(bytes)
            {
                return Err("caller byte correspondence");
            }
        }
        consumed_binding_setup(inputs, seed, view)
    }

    fn consumed_binding_setup(
        inputs: &SourceInputs<'_>,
        seed: &CapturedSeed,
        view: ValidationView<'_>,
    ) -> Result<(), &'static str> {
        let manifest: Value = serde_json::from_slice(&seed.manifest_bytes)
            .map_err(|_error| "captured manifest JSON")?;
        let rows = manifest
            .pointer("/identity/inputs")
            .and_then(Value::as_array)
            .ok_or("captured source input rows")?;
        for (role, kind, runtime) in [
            (SourceRole::Native, None, Some("native")),
            (
                SourceRole::Formats,
                Some("FormatHelperSource"),
                Some("staging"),
            ),
            (SourceRole::DispatchGenerator, Some("GeneratorSource"), None),
        ] {
            let path = context_source_paths()
                .iter()
                .find(|row| row.0 == role)
                .map(|row| row.1)
                .ok_or("owning context path")?;
            let (_, bytes, sha256) = inputs.source_record(path).ok_or("bound context source")?;
            if sha256 != digest(bytes) {
                return Err("bound context digest");
            }
            if let Some(receipt) = runtime {
                let recorded = &view.binding()["runtime"][receipt];
                if recorded["path"].as_str() != Some(path)
                    || recorded["sha256"].as_str() != Some(sha256)
                {
                    return Err("descriptor runtime path/digest correspondence");
                }
            }
            if let Some(input_kind) = kind {
                let matching: Vec<_> = rows.iter().filter(|row| row["id"] == path).collect();
                let [row] = matching.as_slice() else {
                    return Err("manifest source input coverage");
                };
                if row["kind"] != input_kind || row["sha256"].as_str() != Some(sha256) {
                    return Err("manifest generator/format correspondence");
                }
            }
        }
        Ok(())
    }

    pub(in crate::schema_validation) fn assert_consumed_projection(
        projections: [Result<CallerContract, Refusal>; 2],
    ) {
        // Literal cap expressions, source chain and every unresolved duty come
        // from the existing independent oracle, never from the deriver.
        let wanted = expected();
        pending(&wanted);
        assert_eq!(
            project(&wanted.original_impressions_upper, 7, 19, 3, 23, 41),
            7
        );
        assert_eq!(
            project(&wanted.original_impressions_upper, 137, 19, 3, 23, 41),
            100
        );
        assert_eq!(project(&wanted.effective_bidder_cap, 7, 19, 3, 23, 41), 3);
        assert_eq!(project(&wanted.effective_bidder_cap, 7, 2, 13, 23, 41), 2);
        assert_eq!(project(&wanted.effective_bidder_cap, 7, 0, 13, 23, 41), 1);
        assert_eq!(project(&wanted.scoped_nodes_upper, 7, 19, 3, 23, 41), 94);
        assert_eq!(project(&wanted.scoped_nodes_upper, 7, 19, 3, 29, 41), 100);
        assert_eq!(project(&wanted.scoped_nodes_upper, 7, 19, 3, 23, 47), 100);
        for projection in projections {
            assert_eq!(
                projection.as_ref(),
                Ok(&wanted),
                "actual byte correspondence only; tests-first refusal is MissingSourceProof"
            );
        }
    }

    fn caller(unit: CallerUnit, item: &'static str) -> SourceAnchor {
        SourceAnchor {
            owner: Owner::Caller(unit),
            item,
        }
    }

    fn existing(role: SourceRole, item: &'static str) -> SourceAnchor {
        SourceAnchor {
            owner: Owner::Context(role),
            item,
        }
    }

    fn expected_source_chain() -> Vec<SourceAnchor> {
        vec![
            caller(CallerUnit::CorePrepare, "prepare_auction"),
            caller(CallerUnit::CorePrepare, "prepare_state"),
            caller(CallerUnit::CorePrepare, "admit_input"),
            caller(CallerUnit::CorePrepare, "normalization_limits"),
            caller(CallerUnit::NormalizedPrepare, "prepare"),
            caller(CallerUnit::ImplicitFill, "run"),
            caller(CallerUnit::CoreServices, "AuctionFunding::ensure"),
            caller(
                CallerUnit::CoreServices,
                "AuctionResourceProfile::construction_limits",
            ),
            caller(CallerUnit::CoreServices, "normalized_construction_limits"),
            caller(CallerUnit::PolicyHost, "resolve_policy_host"),
            caller(CallerUnit::PolicyHost, "requested_policy_host"),
            caller(CallerUnit::PolicyResolve, "apply_resource_overrides"),
            caller(CallerUnit::PolicyResolve, "clamp_host_to_boot"),
            caller(CallerUnit::CoreOperation, "PreparedAuction::run"),
            caller(
                CallerUnit::ExchangeContext,
                "RequestConstructionLimits::ensure",
            ),
            caller(
                CallerUnit::ExchangeContext,
                "RequestConstructionLimits::with_supplement",
            ),
            existing(SourceRole::ExchangeRun, "scrub_bidder_requests"),
            existing(SourceRole::ExchangeRun, "run_auction"),
            existing(SourceRole::ExchangeLimits, "apply_max_bidder_names"),
            existing(SourceRole::ExchangeLimits, "cap_entries"),
            existing(SourceRole::ExchangeSplit, "group_by_bidder"),
            existing(SourceRole::ExchangeSplit, "group_one_imp"),
            existing(SourceRole::ExchangeSplit, "materialize_per_bidder"),
            existing(SourceRole::ExchangeRun, "run_make_requests"),
            existing(SourceRole::ExchangeSchema, "admit"),
            existing(SourceRole::ExchangeSchema, "retain_valid"),
        ]
    }

    fn expected_scope_guards() -> Vec<GuardSource> {
        vec![
            GuardSource {
                guard: ScopeGuard::DeadlineBeforeScope,
                source: existing(SourceRole::ExchangeRun, "run_auction"),
            },
            GuardSource {
                guard: ScopeGuard::RegistryBeforeSchema,
                source: existing(SourceRole::ExchangeRun, "run_make_requests"),
            },
            GuardSource {
                guard: ScopeGuard::ConfigurationBeforeSchema,
                source: existing(SourceRole::ExchangeRun, "run_make_requests"),
            },
            GuardSource {
                guard: ScopeGuard::OptionalSchemaBypass,
                source: existing(SourceRole::ExchangeSchema, "admit"),
            },
            GuardSource {
                guard: ScopeGuard::FundingAndProjectionBeforeRetain,
                source: existing(SourceRole::ExchangeSchema, "admit"),
            },
            GuardSource {
                guard: ScopeGuard::WholeScopedRequestBeforeScrub,
                source: existing(SourceRole::ExchangeRun, "scrub_bidder_requests"),
            },
            GuardSource {
                guard: ScopeGuard::RetainedNonemptyResult,
                source: existing(SourceRole::ExchangeSchema, "admit"),
            },
        ]
    }

    fn expected() -> CallerContract {
        let original_impressions_upper = CapExpression::Min(
            Box::new(CapExpression::Field(CapField::ResourceImpressions)),
            Box::new(CapExpression::Literal {
                source: caller(CallerUnit::CorePrepare, "MAX_AUCTION_IMPRESSIONS"),
                value: 100,
            }),
        );
        let effective_bidder_cap = CapExpression::Max(
            Box::new(CapExpression::Min(
                Box::new(CapExpression::Field(CapField::RequestedHostBidders)),
                Box::new(CapExpression::Field(CapField::FundedBidders)),
            )),
            Box::new(CapExpression::Literal {
                source: existing(SourceRole::ExchangeRun, "run_auction"),
                value: 1,
            }),
        );
        let scoped_nodes_upper = CapExpression::Add(
            Box::new(CapExpression::Add(
                Box::new(CapExpression::Field(CapField::CoreInputNodes)),
                Box::new(CapExpression::Literal {
                    source: caller(CallerUnit::RuntimeBudget, "NORMALIZATION_IMPLICIT_NODES"),
                    value: 30,
                }),
            )),
            Box::new(CapExpression::Field(CapField::HostParsedNodes)),
        );
        CallerContract {
            original_impressions_upper,
            effective_bidder_cap,
            scoped_nodes_upper,
            required_borrow: BorrowDuty::UnchangedBidderParametersFromScopedEnsure,
            parameter_borrow_preserved: None,
            source_chain: expected_source_chain(),
            scope_guards: expected_scope_guards(),
            cleanup_duties: vec![
                CleanupDuty::RejectedImpression,
                CleanupDuty::RetainErrorAndUnwind,
                CleanupDuty::ScopedRequestRelease,
                CleanupDuty::PartialMaterialization,
            ],
            unresolved_helpers: vec![
                "authentic parent and remaining type ancestry",
                "integer ordering/conversion semantics and costs",
                "typed JSON and normalization impression preservation",
                "Vec/BTreeSet/BTreeMap cardinality and iterator protocols",
                "scoped parameter preservation after ensure",
            ],
            panic_unwinds: None,
            cleanup_work: None,
            tree_navigation_work: None,
            compiled_artifact_binding: None,
        }
    }

    fn source_bytes<'a>(
        context: &'a ContextSources<'_>,
        rows: &'a [CallerSource],
        owner: Owner,
    ) -> &'a [u8] {
        match owner {
            Owner::Caller(unit) => {
                let matching: Vec<_> = rows.iter().filter(|row| row.unit == unit).collect();
                assert_eq!(matching.len(), 1);
                &matching[0].bytes
            }
            Owner::Context(role) => match role {
                SourceRole::Native => context.native,
                SourceRole::Budget => context.budget,
                SourceRole::Input => context.input,
                SourceRole::JsonSize => context.json_size,
                SourceRole::ExchangeSchema => context.exchange_schema,
                SourceRole::ExchangeRun => context.exchange_run,
                SourceRole::ExchangeSplit => context.exchange_split,
                SourceRole::ExchangeLimits => context.exchange_limits,
                SourceRole::DispatchGenerator => context.dispatch_generator,
                SourceRole::Formats => context.formats,
            },
        }
    }

    // Independent fixture inspection/mutation uses Syn, as the existing source
    // owner does. This is not a new executable recognizer or a source receipt.
    fn syntax(bytes: &[u8]) -> syn::File {
        syn::parse_file(from_utf8(bytes).unwrap()).unwrap()
    }

    fn function(file: &syn::File, name: &str) -> syn::Block {
        let mut matching = Vec::new();
        for item in &file.items {
            if let syn::Item::Fn(definition) = item
                && definition.sig.ident == name
            {
                matching.push(definition.block.as_ref().clone());
            }
            if let Some((owner, method)) = name.split_once("::")
                && let syn::Item::Impl(implementation) = item
                && let syn::Type::Path(ty) = implementation.self_ty.as_ref()
                && ty.qself.is_none()
                && ty
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == owner)
            {
                for member in &implementation.items {
                    if let syn::ImplItem::Fn(definition) = member
                        && definition.sig.ident == method
                    {
                        matching.push(definition.block.clone());
                    }
                }
            }
        }
        assert_eq!(
            matching.len(),
            1,
            "one actual top-level source function: {name}"
        );
        matching.pop().unwrap()
    }

    fn local<'a>(body: &'a syn::Block, name: &str) -> &'a syn::Local {
        let matching: Vec<_> = body
            .stmts
            .iter()
            .filter_map(|statement| {
                let syn::Stmt::Local(binding) = statement else {
                    return None;
                };
                matches!(&binding.pat, syn::Pat::Ident(pattern) if pattern.ident == name)
                    .then_some(binding)
            })
            .collect();
        assert_eq!(matching.len(), 1, "one source local: {name}");
        matching[0]
    }

    fn before(context: &ContextSources<'_>, rows: &[CallerSource]) {
        // These deliberately fake JSON labels exercise only the existing
        // conditional context recipe check. They cannot bind a real capture.
        let identity = json!({"fixture":"fake_caller_identity"});
        let roots = json!({"fixture":"fake_caller_roots"});
        let census = json!({"fixture":"fake_caller_census"});
        let binding = ContextBinding {
            descriptor_binding: &identity,
            capture_binding: &identity,
            context_roots: &roots,
            compiled_sources: &census,
        };
        assert_eq!(bind_attempt_context(&binding, context).map(|_| ()), Ok(()));
        assert_eq!(rows.len(), 14);
        for (unit, path, bytes) in CALLER_SOURCES {
            let matching: Vec<_> = rows.iter().filter(|row| row.unit == unit).collect();
            assert_eq!(matching.len(), 1);
            assert_eq!(matching[0].path, path);
            assert_eq!(matching[0].bytes, bytes);
            assert_eq!(matching[0].sha256, digest(bytes));
        }
        for anchor in expected().source_chain {
            let file = syntax(source_bytes(context, rows, anchor.owner));
            assert!(!function(&file, anchor.item).stmts.is_empty());
        }
    }

    // Arithmetic over explicitly hypothetical field values, not runtime policy
    // facts and not the implementation of source-derived expressions.
    fn project(
        expression: &CapExpression,
        impressions: u64,
        requested: u64,
        funded: u64,
        core_nodes: u64,
        host_nodes: u64,
    ) -> u64 {
        match expression {
            CapExpression::Field(field) => match field {
                CapField::ResourceImpressions => impressions,
                CapField::RequestedHostBidders => requested,
                CapField::FundedBidders => funded,
                CapField::CoreInputNodes => core_nodes,
                CapField::HostParsedNodes => host_nodes,
            },
            CapExpression::Literal { source, value } => {
                assert!(matches!(
                    source.owner,
                    Owner::Caller(CallerUnit::CorePrepare | CallerUnit::RuntimeBudget)
                        | Owner::Context(SourceRole::ExchangeRun)
                ));
                assert!(!source.item.is_empty());
                *value
            }
            CapExpression::Min(left, right) => {
                project(left, impressions, requested, funded, core_nodes, host_nodes).min(project(
                    right,
                    impressions,
                    requested,
                    funded,
                    core_nodes,
                    host_nodes,
                ))
            }
            CapExpression::Max(left, right) => {
                project(left, impressions, requested, funded, core_nodes, host_nodes).max(project(
                    right,
                    impressions,
                    requested,
                    funded,
                    core_nodes,
                    host_nodes,
                ))
            }
            CapExpression::Add(left, right) => {
                project(left, impressions, requested, funded, core_nodes, host_nodes)
                    .checked_add(project(
                        right,
                        impressions,
                        requested,
                        funded,
                        core_nodes,
                        host_nodes,
                    ))
                    .expect("small hypothetical scoped-population operands")
            }
        }
    }

    fn pending(contract: &CallerContract) {
        assert_eq!(
            contract.unresolved_helpers,
            [
                "authentic parent and remaining type ancestry",
                "integer ordering/conversion semantics and costs",
                "typed JSON and normalization impression preservation",
                "Vec/BTreeSet/BTreeMap cardinality and iterator protocols",
                "scoped parameter preservation after ensure",
            ]
        );
        assert_eq!(contract.panic_unwinds, None);
        assert_eq!(contract.cleanup_work, None);
        assert_eq!(contract.tree_navigation_work, None);
        assert_eq!(contract.parameter_borrow_preserved, None);
        assert_eq!(
            contract.required_borrow,
            BorrowDuty::UnchangedBidderParametersFromScopedEnsure
        );
        assert_eq!(contract.compiled_artifact_binding, None);
        assert_eq!(
            contract.cleanup_duties,
            [
                CleanupDuty::RejectedImpression,
                CleanupDuty::RetainErrorAndUnwind,
                CleanupDuty::ScopedRequestRelease,
                CleanupDuty::PartialMaterialization,
            ]
        );
    }

    #[test]
    fn original_impressions_follow_both_core_admission_representations() {
        let context = context();
        let rows = sources();
        before(&context, &rows);
        let file = syntax(source_bytes(
            &context,
            &rows,
            Owner::Caller(CallerUnit::CorePrepare),
        ));
        let constants: Vec<_> = file
            .items
            .iter()
            .filter_map(|item| {
                if let syn::Item::Const(value) = item
                    && value.ident == "MAX_AUCTION_IMPRESSIONS"
                {
                    Some(value)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(constants.len(), 1);
        assert_eq!(constants[0].ty.to_token_stream().to_string(), "u16");
        assert_eq!(constants[0].expr.to_token_stream().to_string(), "100");
        let limits = function(&file, "normalization_limits");
        let expected_limits: syn::Block = syn::parse_quote! {{
            PrepareLimits {
                max_impressions: u32::from(
                    limits.max_impressions_per_auction.min(MAX_AUCTION_IMPRESSIONS),
                ),
                max_copied_header_bytes: limits.max_copied_header_bytes,
            }
        }};
        assert_eq!(
            limits.to_token_stream().to_string(),
            expected_limits.to_token_stream().to_string()
        );
        let admitted = function(&file, "admit_input");
        let syn::Stmt::Expr(syn::Expr::Match(branches), None) = admitted.stmts.last().unwrap()
        else {
            panic!("actual representation dispatch");
        };
        assert_eq!(branches.arms.len(), 2);
        assert_eq!(
            branches.arms[0].pat.to_token_stream().to_string(),
            quote!(AuctionInput::Json { bytes, headers }).to_string()
        );
        assert_eq!(
            branches.arms[1].pat.to_token_stream().to_string(),
            quote!(AuctionInput::Typed { request, headers }).to_string()
        );
        let normalized = syntax(source_bytes(
            &context,
            &rows,
            Owner::Caller(CallerUnit::NormalizedPrepare),
        ));
        let prepare = function(&normalized, "prepare");
        let got = local(&prepare, "got");
        let expected_got: syn::Stmt = syn::parse_quote! {
            let got = u32::try_from(req.imp.len()).unwrap_or(u32::MAX);
        };
        assert_eq!(
            got.to_token_stream().to_string(),
            expected_got.to_token_stream().to_string()
        );
        let contract = expected();
        pending(&contract);
        assert_eq!(
            project(&contract.original_impressions_upper, 7, 19, 3, 23, 41),
            7
        );
        assert_eq!(
            project(&contract.original_impressions_upper, 137, 19, 3, 23, 41),
            100
        );
        assert_eq!(
            project(&contract.original_impressions_upper, 0, 19, 3, 23, 41),
            0
        );
        assert_eq!(
            derive_caller_sources(&context, &rows, 1),
            Ok(contract.clone())
        );
        assert_eq!(derive_caller_sources(&context, &rows, 0), Ok(contract));
    }

    #[test]
    fn bidder_cap_records_clamp_then_nonzero_effective_cap() {
        let context = context();
        let rows = sources();
        before(&context, &rows);
        let run = function(&syntax(context.exchange_run), "run_auction");
        let cap = local(&run, "cap");
        let expected_cap: syn::Stmt = syn::parse_quote! {
            let cap = ctx.host.resource_limits.max_bidders_per_auction.max(1);
        };
        assert_eq!(
            cap.to_token_stream().to_string(),
            expected_cap.to_token_stream().to_string()
        );
        let host = function(
            &syntax(source_bytes(
                &context,
                &rows,
                Owner::Caller(CallerUnit::PolicyHost),
            )),
            "resolve_policy_host",
        );
        let expected_host: syn::Block = syn::parse_quote! {{
            let mut host = requested_policy_host(product, &policy.host);
            clamp_host_to_boot(&mut host, funded);
            host
        }};
        assert_eq!(
            host.to_token_stream().to_string(),
            expected_host.to_token_stream().to_string()
        );
        let contract = expected();
        // Only one hypothetical input changes at each adjacent boundary.
        for (impressions, requested, funded, original, bidders) in [
            (7_u64, 9_u64, 3_u64, 7_u64, 3_u64),
            (137, 9, 3, 100, 3),
            (137, 9, 11, 100, 9),
            (137, 2, 11, 100, 2),
            (137, 0, 11, 100, 1),
            (137, 0, 0, 100, 1),
            (137, 9, 0, 100, 1),
        ] {
            assert_eq!(
                project(
                    &contract.original_impressions_upper,
                    impressions,
                    requested,
                    funded,
                    23,
                    41,
                ),
                original
            );
            assert_eq!(
                project(
                    &contract.effective_bidder_cap,
                    impressions,
                    requested,
                    funded,
                    23,
                    41,
                ),
                bidders
            );
        }
        pending(&contract);
        assert_eq!(
            derive_caller_sources(&context, &rows, 1),
            Ok(contract.clone())
        );
        assert_eq!(derive_caller_sources(&context, &rows, 0), Ok(contract));
    }

    fn assert_scope_entry_and_parameter_borrow(context: &ContextSources<'_>) {
        let admit = function(&syntax(context.exchange_schema), "admit");
        let expected_bypass: syn::Stmt = syn::parse_quote! {
            if !mandatory && !funded_legacy { return true; }
        };
        assert_eq!(
            admit.stmts[2].to_token_stream().to_string(),
            expected_bypass.to_token_stream().to_string()
        );
        let expected_funding: syn::Stmt = syn::parse_quote! {
            if ctx.schema_scratch_bytes < NATIVE_SCHEMA_SCRATCH_BYTES
                || ctx.projection_reservation.is_none()
            {
                errors.push(error(NativeSchemaError::Unavailable));
                return false;
            }
        };
        assert_eq!(
            admit.stmts[4].to_token_stream().to_string(),
            expected_funding.to_token_stream().to_string()
        );
        let retain = function(&syntax(context.exchange_schema), "retain_valid");
        assert_eq!(retain.stmts.len(), 1);
        let syn::Stmt::Expr(syn::Expr::MethodCall(call), Some(_)) = &retain.stmts[0] else {
            panic!("actual retain call");
        };
        assert_eq!(call.method, "retain");
        assert_eq!(
            call.receiver.to_token_stream().to_string(),
            quote!(request.imp).to_string()
        );
        assert_eq!(call.args.len(), 1);
        let syn::Expr::Closure(predicate) = call.args.first().unwrap() else {
            panic!("actual retain predicate");
        };
        let syn::Expr::Block(predicate_body) = predicate.body.as_ref() else {
            panic!("actual predicate body");
        };
        let borrowed = local(&predicate_body.block, "result");
        let expected_borrow: syn::Stmt = syn::parse_quote! {
            let result = imp.ext.as_ref()
                .and_then(|ext| ext.bidder.as_ref())
                .ok_or(NativeSchemaError::InvalidParameters)
                .and_then(&mut validate);
        };
        assert_eq!(
            borrowed.to_token_stream().to_string(),
            expected_borrow.to_token_stream().to_string()
        );
    }

    fn assert_scoped_ensure_source(context: &ContextSources<'_>, rows: &[CallerSource]) {
        let budget = syntax(source_bytes(
            context,
            rows,
            Owner::Caller(CallerUnit::RuntimeBudget),
        ));
        let implicit: Vec<_> = budget
            .items
            .iter()
            .filter_map(|item| {
                if let syn::Item::Const(value) = item
                    && value.ident == "NORMALIZATION_IMPLICIT_NODES"
                {
                    Some(value)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(implicit.len(), 1);
        assert_eq!(implicit[0].expr.to_token_stream().to_string(), "30");
        let scrub = function(&syntax(context.exchange_run), "scrub_bidder_requests");
        let syn::Stmt::Expr(syn::Expr::ForLoop(scopes), None) = &scrub.stmts[0] else {
            panic!("actual serial scrub loop");
        };
        let expected_ensure: syn::Stmt = syn::parse_quote! {
            if let Some(scope_limits) = limits
                && let Err(error) = scope_limits.ensure(&pbr.request)
            {
                audit.start_scope();
                audit.fail_scope();
                return Err(error);
            }
        };
        assert_eq!(
            scopes.body.stmts[1].to_token_stream().to_string(),
            expected_ensure.to_token_stream().to_string()
        );
    }

    fn assert_scoped_limit_composition(context: &ContextSources<'_>, rows: &[CallerSource]) {
        let services = syntax(source_bytes(
            context,
            rows,
            Owner::Caller(CallerUnit::CoreServices),
        ));
        let normalized = function(&services, "normalized_construction_limits");
        let expected_nodes: syn::Stmt = syn::parse_quote! {
            let max_nodes = input_nodes
                .checked_add(NORMALIZATION_IMPLICIT_NODES)
                .ok_or(ResourceError::CapacityOverflow)?;
        };
        assert_eq!(
            local(&normalized, "max_nodes")
                .to_token_stream()
                .to_string(),
            expected_nodes.to_token_stream().to_string()
        );
        let run = function(&syntax(context.exchange_run), "run_auction");
        let expected_scoped: syn::Stmt = syn::parse_quote! {
            let scoped_limits = ctx
                .construction_limits
                .map(|limits| {
                    limits.with_supplement(
                        ctx.host.resource_limits.max_parsed_nodes,
                        ctx.host.resource_limits.max_outbound_raw_bytes,
                    )
                })
                .transpose()?;
        };
        assert_eq!(
            local(&run, "scoped_limits").to_token_stream().to_string(),
            expected_scoped.to_token_stream().to_string()
        );
    }

    fn assert_scope_guard_sources(
        context: &ContextSources<'_>,
        rows: &[CallerSource],
        contract: &CallerContract,
    ) {
        for gate in &contract.scope_guards {
            let owner = function(
                &syntax(source_bytes(context, rows, gate.source.owner)),
                gate.source.item,
            );
            assert!(!owner.stmts.is_empty());
            let anchor = match gate.guard {
                ScopeGuard::DeadlineBeforeScope => existing(SourceRole::ExchangeRun, "run_auction"),
                ScopeGuard::RegistryBeforeSchema | ScopeGuard::ConfigurationBeforeSchema => {
                    existing(SourceRole::ExchangeRun, "run_make_requests")
                }
                ScopeGuard::OptionalSchemaBypass
                | ScopeGuard::FundingAndProjectionBeforeRetain
                | ScopeGuard::RetainedNonemptyResult => {
                    existing(SourceRole::ExchangeSchema, "admit")
                }
                ScopeGuard::WholeScopedRequestBeforeScrub => {
                    existing(SourceRole::ExchangeRun, "scrub_bidder_requests")
                }
            };
            assert_eq!(gate.source, anchor);
        }
    }

    #[test]
    fn scope_gates_do_not_discharge_retention_or_cleanup() {
        let context = context();
        let rows = sources();
        before(&context, &rows);
        assert_scope_entry_and_parameter_borrow(&context);
        let contract = expected();
        assert_scoped_ensure_source(&context, &rows);
        assert_scoped_limit_composition(&context, &rows);
        assert_eq!(project(&contract.scoped_nodes_upper, 7, 9, 3, 23, 41), 94);
        assert_eq!(project(&contract.scoped_nodes_upper, 7, 9, 3, 31, 41), 102);
        assert_eq!(project(&contract.scoped_nodes_upper, 7, 9, 3, 31, 53), 114);
        // This expression is not a B-tree invariant or navigation-work proof.
        assert_eq!(contract.tree_navigation_work, None);
        assert_eq!(contract.scope_guards.len(), 7);
        assert_scope_guard_sources(&context, &rows, &contract);
        pending(&contract);
        // No reached retain predicate is not evidence of free scope setup/drop.
        assert_eq!(
            derive_caller_sources(&context, &rows, 0),
            Ok(contract.clone())
        );
        assert_eq!(derive_caller_sources(&context, &rows, 3), Ok(contract));
    }

    fn assert_missing_caller_sources(context: &ContextSources<'_>, rows: &[CallerSource]) {
        for (unit, _, _) in CALLER_SOURCES {
            let mut missing = rows.to_vec();
            let index = missing.iter().position(|row| row.unit == unit).unwrap();
            assert_eq!(missing.remove(index).unit, unit);
            for attempts in [1, 0] {
                assert_eq!(
                    derive_caller_sources(context, &missing, attempts),
                    Err(Refusal::MissingSource(unit))
                );
            }
        }
    }

    fn assert_rehashed_caller_constants(context: &ContextSources<'_>, rows: &[CallerSource]) {
        for (unit, name, literal, substitution) in [
            (
                CallerUnit::CorePrepare,
                "MAX_AUCTION_IMPRESSIONS",
                "100",
                syn::parse_quote!(101),
            ),
            (
                CallerUnit::RuntimeBudget,
                "NORMALIZATION_IMPLICIT_NODES",
                "30",
                syn::parse_quote!(31),
            ),
        ] {
            let replacement: syn::Expr = substitution;
            let mut changed = rows.to_vec();
            let row = changed.iter_mut().find(|row| row.unit == unit).unwrap();
            let original = row.bytes.clone();
            let mut file = syntax(&row.bytes);
            let mut hits = 0_usize;
            for item in &mut file.items {
                if let syn::Item::Const(value) = item
                    && value.ident == name
                {
                    assert_eq!(value.expr.to_token_stream().to_string(), literal);
                    *value.expr = replacement.clone();
                    hits = hits.checked_add(1).unwrap();
                }
            }
            assert_eq!(hits, 1);
            row.bytes = file.to_token_stream().to_string().into_bytes();
            row.sha256 = digest(&row.bytes);
            assert_eq!(row.sha256, digest(&row.bytes));
            assert_ne!(row.bytes, original);
            for attempts in [1, 0] {
                assert_eq!(
                    derive_caller_sources(context, &changed, attempts),
                    Err(Refusal::SourceShape(unit))
                );
            }
        }
    }

    fn assert_rehashed_bidder_clamp(context: &ContextSources<'_>, rows: &[CallerSource]) {
        let mut clamped = rows.to_vec();
        let row = clamped
            .iter_mut()
            .find(|row| row.unit == CallerUnit::PolicyResolve)
            .unwrap();
        let original = row.bytes.clone();
        let mut file = syntax(&row.bytes);
        let mut hits = 0_usize;
        for item in &mut file.items {
            if let syn::Item::Fn(function) = item
                && function.sig.ident == "clamp_host_to_boot"
            {
                let expected: syn::Stmt = syn::parse_quote! {
                    limits.max_bidders_per_auction = limits.max_bidders_per_auction
                        .min(usize::try_from(boot.max_bidders_per_auction).unwrap_or(usize::MAX));
                };
                assert_eq!(
                    function.block.stmts[1].to_token_stream().to_string(),
                    expected.to_token_stream().to_string()
                );
                function.block.stmts[1] = syn::parse_quote! {
                    limits.max_bidders_per_auction = limits.max_bidders_per_auction
                        .max(usize::try_from(boot.max_bidders_per_auction).unwrap_or(usize::MAX));
                };
                hits = hits.checked_add(1).unwrap();
            }
        }
        assert_eq!(hits, 1);
        row.bytes = file.to_token_stream().to_string().into_bytes();
        row.sha256 = digest(&row.bytes);
        assert_eq!(row.sha256, digest(&row.bytes));
        assert_ne!(row.bytes, original);
        for attempts in [1, 0] {
            assert_eq!(
                derive_caller_sources(context, &clamped, attempts),
                Err(Refusal::SourceShape(CallerUnit::PolicyResolve))
            );
        }
    }

    fn assert_changed_effective_floor(context: &ContextSources<'_>, rows: &[CallerSource]) {
        let mut file = syntax(context.exchange_run);
        let mut hits = 0_usize;
        for item in &mut file.items {
            if let syn::Item::Fn(function) = item
                && function.sig.ident == "run_auction"
            {
                for statement in &mut function.block.stmts {
                    if let syn::Stmt::Local(binding) = statement
                        && matches!(&binding.pat, syn::Pat::Ident(name) if name.ident == "cap")
                    {
                        let initial = &mut binding.init.as_mut().unwrap().expr;
                        let expected: syn::Expr = syn::parse_quote!(
                            ctx.host.resource_limits.max_bidders_per_auction.max(1)
                        );
                        assert_eq!(
                            initial.to_token_stream().to_string(),
                            expected.to_token_stream().to_string()
                        );
                        **initial = syn::parse_quote!(
                            ctx.host.resource_limits.max_bidders_per_auction.max(0)
                        );
                        hits = hits.checked_add(1).unwrap();
                    }
                }
            }
        }
        assert_eq!(hits, 1);
        let bytes = file.to_token_stream().to_string().into_bytes();
        assert_ne!(bytes, context.exchange_run);
        let changed_context = ContextSources {
            exchange_run: &bytes,
            ..*context
        };
        for attempts in [1, 0] {
            assert_eq!(
                derive_caller_sources(&changed_context, rows, attempts),
                Err(Refusal::Context(super::super::Refusal::SourceShape(
                    SourceRole::ExchangeRun
                )))
            );
        }
    }

    fn assert_rehashed_import_redirects(context: &ContextSources<'_>, rows: &[CallerSource]) {
        for (unit, prefix, binding) in [
            (CallerUnit::CoreAuctionRoot, "services", "AuctionServices"),
            (
                CallerUnit::CoreConfigRoot,
                "auctioneer_config",
                "SharedAppConfig",
            ),
            (CallerUnit::OpenRtbRoot, "prepare", "PrepareLimits"),
            (CallerUnit::RuntimeRoot, "budget", "ResolvedIsolateLimits"),
            (CallerUnit::ExchangeRoot, "run", "run_auction"),
        ] {
            let mut redirected = rows.to_vec();
            let row = redirected
                .iter_mut()
                .find(|source| source.unit == unit)
                .unwrap();
            let before = row.bytes.clone();
            let mut file = syntax(&row.bytes);
            let mut hits = 0_usize;
            for item in &mut file.items {
                if let syn::Item::Use(import) = item
                    && import.to_token_stream().to_string().contains(binding)
                    && let syn::UseTree::Path(path) = &mut import.tree
                    && path.ident == prefix
                {
                    assert_eq!(path.ident, prefix);
                    path.ident = syn::parse_quote!(fake_caller_module);
                    hits = hits.checked_add(1).unwrap();
                }
            }
            assert_eq!(hits, 1);
            row.bytes = file.to_token_stream().to_string().into_bytes();
            row.sha256 = digest(&row.bytes);
            assert_eq!(row.sha256, digest(&row.bytes));
            assert_ne!(row.bytes, before);
            for attempts in [1, 0] {
                assert_eq!(
                    derive_caller_sources(context, &redirected, attempts),
                    Err(Refusal::SourceShape(unit))
                );
            }
        }
    }

    // Selector DATA only: these ASTs cannot establish a caller/source binding.
    fn assert_impl_kind_selection() {
        let expected: syn::ItemImpl = syn::parse_quote! {
            impl FakeMethodOwner {
                fn selected(&self) -> u64 {}
            }
        };
        let rule = method_rule(
            caller(CallerUnit::CoreServices, "FakeMethodOwner::selected"),
            expected.clone(),
        );
        let actual: syn::File = syn::parse_quote! {
            impl FakeMethodOwner {
                fn selected(&self) -> u64 { 7 }
            }
            impl FakeUnrelatedTrait for FakeMethodOwner {
                fn selected(&self) -> u64 { 19 }
            }
        };
        assert_eq!(actual.items.len(), 2);
        let selected: syn::Block = syn::parse_quote!({ 7 });
        assert_eq!(
            match_method(&actual, &rule, &expected)
                .map(|block| block.to_token_stream().to_string()),
            Ok(selected.to_token_stream().to_string())
        );
        let mut duplicate = actual.clone();
        let syn::Item::Impl(inherent) = actual.items.first().unwrap() else {
            panic!("inherent selector before-state");
        };
        assert!(inherent.trait_.is_none());
        duplicate.items.push(syn::Item::Impl(inherent.clone()));
        assert_eq!(
            match_method(&duplicate, &rule, &expected).map(|_| ()),
            Err(Refusal::SourceShape(CallerUnit::CoreServices))
        );
        let mut wrong_trait = actual.clone();
        assert!(matches!(
            wrong_trait.items.first(),
            Some(syn::Item::Impl(implementation)) if implementation.trait_.is_none()
        ));
        *wrong_trait.items.first_mut().unwrap() = syn::parse_quote! {
            impl FakeRedirectedTrait for FakeMethodOwner {
                fn selected(&self) -> u64 { 7 }
            }
        };
        assert_eq!(
            match_method(&wrong_trait, &rule, &expected).map(|_| ()),
            Err(Refusal::SourceShape(CallerUnit::CoreServices))
        );
    }

    #[test]
    fn missing_and_rehashed_caller_sources_refuse_at_zero_attempts() {
        let context = context();
        let rows = sources();
        before(&context, &rows);
        assert_eq!(derive_caller_sources(&context, &rows, 1), Ok(expected()));
        assert_missing_caller_sources(&context, &rows);
        assert_rehashed_caller_constants(&context, &rows);
        assert_rehashed_bidder_clamp(&context, &rows);
        assert_changed_effective_floor(&context, &rows);
        assert_rehashed_import_redirects(&context, &rows);
        assert_impl_kind_selection();
    }
}

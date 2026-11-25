//! Tests-first caller-cap expressions and scope duties, never source authority.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallerUnit {
    CorePrepare,
    CoreServices,
    CoreOperation,
    PolicyHost,
    PolicyResolve,
    NormalizedPrepare,
    ImplicitFill,
    ExchangeContext,
    RuntimeBudget,
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
struct CallerContract {
    original_impressions_upper: CapExpression,
    effective_bidder_cap: CapExpression,
    scoped_nodes_upper: CapExpression,
    required_borrow: BorrowDuty,
    parameter_borrow_preserved: Option<bool>,
    source_chain: Vec<SourceAnchor>,
    scope_guards: Vec<GuardSource>,
    cleanup_duties: Vec<CleanupDuty>,
    panic_unwinds: Option<bool>,
    cleanup_work: Option<u64>,
    tree_navigation_work: Option<u64>,
    compiled_artifact_binding: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    MissingCallerSourceProof,
    MissingSource(CallerUnit),
    SourceShape(CallerUnit),
    Context(super::Refusal),
}

// Attempts are a diagnostic hypothesis only, never a source for a caller cap.
// Zero attempts must not bypass source recognition or erase missing duties.
// No parser, cap model or authority constructor is implemented before hosted RED.
fn derive_caller_sources(
    _context: &ContextSources<'_>,
    _sources: &[CallerSource],
    _attempts: u64,
) -> Result<CallerContract, Refusal> {
    Err(Refusal::MissingCallerSourceProof)
}

#[cfg(test)]
mod tests {
    use super::{
        BorrowDuty, CallerContract, CallerSource, CallerUnit, CapExpression, CapField, CleanupDuty,
        GuardSource, Owner, Refusal, ScopeGuard, SourceAnchor, derive_caller_sources,
    };
    use crate::schema_validation::work::{
        BUDGET, ContextBinding, ContextSources, DISPATCH_GENERATOR, EXCHANGE_LIMITS, EXCHANGE_RUN,
        EXCHANGE_SCHEMA, EXCHANGE_SPLIT, FORMATS, INPUT, JSON_SIZE, NATIVE, SourceRole,
        bind_attempt_context,
    };
    use quote::{ToTokens as _, quote};
    use serde_json::json;
    use sha2::{Digest as _, Sha256};
    use std::str::from_utf8;

    const CALLER_SOURCES: [(CallerUnit, &str, &[u8]); 9] = [
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
        assert_eq!(rows.len(), 9);
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
                    value.expr = Box::new(replacement.clone());
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
                        *initial = Box::new(syn::parse_quote!(
                            ctx.host.resource_limits.max_bidders_per_auction.max(0)
                        ));
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
    }
}

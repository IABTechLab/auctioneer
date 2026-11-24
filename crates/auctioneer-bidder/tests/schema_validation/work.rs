//! Attempted-prefix contracts, awaiting hosted RED and source derivations.
//!
//! Binding/provenance below is DATA, not an artifact capability. The parent
//! joins these bytes to the sealed descriptor and its compiled-source census,
//! checking current regular-file bytes before/after the same capture lifetime.
//! This module neither opens that filesystem boundary nor invents checksums.
//! A supported source recipe, not a matching digest, must justify every count.
//!
//! Native clock calls = native entries + budget entries + post-evaluate checks:
//! early refusal 1, budget refusal 2, ordinary return 3, validator panic 2.
//! Missing parameters invoke native zero times but still visit retain/error/drop
//! paths. P includes those outer paths and every reached preflight, before debit.
//! V includes fresh cache construction, validation and release. Clock and seed
//! bodies remain separate N_clock*H and N_seed*E requirements; no warm TLS premise.
//! Seed multiplicity requires the matched construction closure, not search count.
//! No result here is CommonSourceComplete, a numeric work cost or qualification.

mod epsilon;

use super::operations::{self, Primitive, model::Recipe};
use auctioneer_bidder::schema::{
    SchemaEvaluationError, SchemaInputError, SchemaInputLimits, SchemaInputStats,
};
use serde_json::Value;
use std::collections::BTreeMap;

const NATIVE: &[u8] = include_bytes!("../../src/schema/native.rs");
const BUDGET: &[u8] = include_bytes!("../../src/schema/mod.rs");
const INPUT: &[u8] = include_bytes!("../../src/schema/input.rs");
const JSON_SIZE: &[u8] = include_bytes!("../../../auctioneer-runtime/src/json_size.rs");
const EXCHANGE_SCHEMA: &[u8] = include_bytes!("../../../auctioneer-exchange/src/run/schema.rs");
const EXCHANGE_RUN: &[u8] = include_bytes!("../../../auctioneer-exchange/src/run/mod.rs");
const EXCHANGE_SPLIT: &[u8] = include_bytes!("../../../auctioneer-exchange/src/split/mod.rs");
const EXCHANGE_LIMITS: &[u8] = include_bytes!("../../../auctioneer-exchange/src/limits.rs");
const DISPATCH_GENERATOR: &[u8] =
    include_bytes!("../../../auctioneer-config/src/schema_staging_build.rs");
const FORMATS: &[u8] = include_bytes!("../../src/schema/staging.rs");

// Borrow existing parent provenance; none of these values creates authority.
#[derive(Debug)]
pub(super) struct ContextBinding<'a> {
    pub(super) descriptor_binding: &'a Value,
    pub(super) context_binding: &'a Value,
    pub(super) context_roots: &'a Value,
    pub(super) compiled_sources: &'a Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ContextSources<'a> {
    pub(super) native: &'a [u8],
    pub(super) budget: &'a [u8],
    pub(super) input: &'a [u8],
    pub(super) json_size: &'a [u8],
    pub(super) exchange_schema: &'a [u8],
    pub(super) exchange_run: &'a [u8],
    pub(super) exchange_split: &'a [u8],
    pub(super) exchange_limits: &'a [u8],
    pub(super) dispatch_generator: &'a [u8],
    pub(super) formats: &'a [u8],
}

#[derive(Debug)]
pub(super) struct AttemptContext<'a> {
    binding: &'a ContextBinding<'a>,
    sources: &'a ContextSources<'a>,
    native_input_limits: SchemaInputLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceRole {
    Native,
    Budget,
    Input,
    JsonSize,
    ExchangeSchema,
    ExchangeRun,
    ExchangeSplit,
    ExchangeLimits,
    DispatchGenerator,
    Formats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CallerFact {
    OriginalImpressions,
    BidderCap,
    PanicUnwind,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    UnsupportedContext,
    SourceShape(SourceRole),
    BindingMismatch,
    MissingCallerFact(CallerFact),
    UnsupportedNativePath,
    NoSourceLeafDuty(Primitive),
    MissingRuntimeLemma(Primitive),
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePath {
    EntryExpired,
    Unavailable,
    InputRejected(SchemaInputError),
    BudgetRejected(SchemaEvaluationError),
    Returned { valid: bool },
    ValidatorPanicked,
}

#[derive(Clone, Copy, Debug)]
struct CallerFacts {
    original_impressions: Option<u64>,
    configured_bidder_cap: Option<u64>,
    panic_unwinds: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
struct Population {
    scopes_entered: u64,
    retain_visits: u64,
    missing_parameters: u64,
    native_attempts: u64,
    native_path: Option<NativePath>,
}

#[derive(Debug, Eq, PartialEq)]
struct InputCounts {
    stats: SchemaInputStats,
    value_visits: u64,
    key_visits: u64,
    container_entries: u64,
    scalar_serializations: u64,
    unresolved_calls: Vec<&'static str>,
}

#[derive(Debug, Eq, PartialEq)]
struct PrefixCounts {
    outer_scopes: u64,
    retain_visits: u64,
    missing_parameter_errors: u64,
    error_rows: u64,
    removed_impressions: u64,
    native_entries: u64,
    preflight_entries: u64,
    budget_entries: u64,
    debit_events: u64,
    post_evaluate_checks: u64,
    native_clock_calls: u64,
    fresh_cache_calls: u64,
    cache_drop_entries: u64,
    seed_invocations: Option<u64>,
    unresolved: Vec<Primitive>,
}

#[derive(Debug, Eq, PartialEq)]
struct RecipeCounts {
    root_entries: u64,
    helpers: BTreeMap<String, u64>,
    iterator_callbacks: u64,
    searches: u64,
    fresh_cache_calls: u64,
    cache_drop_entries: u64,
    seed_invocations: Option<u64>,
    unresolved: Vec<Primitive>,
}

// RED stubs only. Actual caller AST correspondence and recipe interpretation
// must be implemented after hosted assertion failures, never inferred from data.
pub(super) fn bind_attempt_context<'a>(
    _binding: &'a ContextBinding<'a>,
    _sources: &'a ContextSources<'a>,
) -> Result<AttemptContext<'a>, Refusal> {
    Err(Refusal::UnsupportedContext)
}

fn input_counts(
    _context: &AttemptContext<'_>,
    _value: &Value,
    _limits: SchemaInputLimits,
) -> Result<InputCounts, Refusal> {
    Err(Refusal::UnsupportedContext)
}

fn prefix_counts(
    _context: &AttemptContext<'_>,
    _facts: CallerFacts,
    _population: Population,
) -> Result<PrefixCounts, Refusal> {
    Err(Refusal::UnsupportedContext)
}

fn recipe_counts(
    _context: &AttemptContext<'_>,
    _recipe: &Recipe,
    _dimensions: SchemaInputStats,
    _attempts: u64,
) -> Result<RecipeCounts, Refusal> {
    Err(Refusal::UnsupportedContext)
}

fn require_source_leaf_cost(
    _context: &AttemptContext<'_>,
    _recipe: &Recipe,
    _duty: Primitive,
) -> Result<(), Refusal> {
    Err(Refusal::UnsupportedContext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_bidder::schema::measure_schema_input;
    use quote::ToTokens as _;
    use serde_json::json;
    use std::collections::BTreeSet;
    use syn::visit_mut::{self, VisitMut};

    fn sources() -> ContextSources<'static> {
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

    fn successful<T: std::fmt::Debug>(result: Result<T, Refusal>) -> T {
        assert_eq!(result.as_ref().map(|_| ()), Ok(()));
        result.unwrap()
    }

    fn context<'a>(
        binding: &'a ContextBinding<'a>,
        sources: &'a ContextSources<'a>,
    ) -> AttemptContext<'a> {
        // These independent field checks prevent a role from silently borrowing
        // a different source. They are data checks, not artifact certification.
        for (role, actual, expected) in [
            (SourceRole::Native, sources.native, NATIVE),
            (SourceRole::Budget, sources.budget, BUDGET),
            (SourceRole::Input, sources.input, INPUT),
            (SourceRole::JsonSize, sources.json_size, JSON_SIZE),
            (
                SourceRole::ExchangeSchema,
                sources.exchange_schema,
                EXCHANGE_SCHEMA,
            ),
            (SourceRole::ExchangeRun, sources.exchange_run, EXCHANGE_RUN),
            (
                SourceRole::ExchangeSplit,
                sources.exchange_split,
                EXCHANGE_SPLIT,
            ),
            (
                SourceRole::ExchangeLimits,
                sources.exchange_limits,
                EXCHANGE_LIMITS,
            ),
            (
                SourceRole::DispatchGenerator,
                sources.dispatch_generator,
                DISPATCH_GENERATOR,
            ),
            (SourceRole::Formats, sources.formats, FORMATS),
        ] {
            assert_eq!(actual, expected, "source role: {role:?}");
        }
        let checked = successful(bind_attempt_context(binding, sources));
        assert_eq!(
            checked.binding.descriptor_binding,
            binding.descriptor_binding
        );
        assert_eq!(checked.binding.context_binding, binding.context_binding);
        assert_eq!(checked.binding.context_roots, binding.context_roots);
        assert_eq!(checked.binding.compiled_sources, binding.compiled_sources);
        assert_eq!(checked.sources, sources);
        assert_eq!(
            checked.native_input_limits,
            SchemaInputLimits {
                serialized_bytes: 65_536,
                nodes: 4_096,
                depth: 32,
            }
        );
        checked
    }

    fn facts() -> CallerFacts {
        CallerFacts {
            original_impressions: Some(3),
            configured_bidder_cap: Some(2),
            panic_unwinds: Some(true),
        }
    }

    fn population(path: NativePath) -> Population {
        Population {
            scopes_entered: 1,
            retain_visits: 1,
            missing_parameters: 0,
            native_attempts: 1,
            native_path: Some(path),
        }
    }

    fn assert_native_prefix(context: &AttemptContext<'_>, path: NativePath, expected: [u64; 9]) {
        let counts = successful(prefix_counts(context, facts(), population(path)));
        assert_eq!(counts.outer_scopes, 1);
        assert_eq!(counts.retain_visits, 1);
        assert_eq!(counts.missing_parameter_errors, 0);
        assert_eq!(counts.native_entries, 1);
        assert_eq!(
            [
                counts.preflight_entries,
                counts.budget_entries,
                counts.debit_events,
                counts.post_evaluate_checks,
                counts.native_clock_calls,
                counts.fresh_cache_calls,
                counts.cache_drop_entries,
                counts.error_rows,
                counts.removed_impressions,
            ],
            expected,
            "native path: {path:?}"
        );
    }

    fn classified(syntax: &syn::File) -> operations::BodyOperations {
        let result = operations::classify_source_body(syntax, &[], &[]);
        assert_eq!(result.as_ref().map(|_| ()), Ok(()));
        result.unwrap()
    }

    // Presence only, not an execution-count oracle. Follow source call edges
    // from roots so an unused helper cannot supply an obligation.
    fn reachable_duties(recipe: &Recipe) -> BTreeSet<Primitive> {
        use operations::model::{Kind, walk};

        let mut pending = recipe.roots.clone();
        let mut visited = BTreeSet::new();
        let mut duties = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let function = recipe
                .functions
                .get(&id)
                .expect("classified reachable function");
            walk(&function.body, &mut |op| {
                match &op.kind {
                    Kind::Call { target, .. } => pending.push(*target),
                    Kind::Primitive { lemma, .. }
                    | Kind::Construct {
                        cleanup: Some(lemma),
                        ..
                    } => {
                        duties.insert(*lemma);
                    }
                    Kind::Iterate { protocol, .. } | Kind::Callback { protocol, .. } => {
                        duties.insert(*protocol);
                    }
                    _ => {}
                }
                Ok(())
            })
            .unwrap();
        }
        duties
    }

    fn callback_body() -> syn::File {
        syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                is_branch_valid_0(instance) && is_branch_valid_0(instance)
            }
            fn is_branch_valid_0(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::Array(items) => items.iter().all(|item| is_branch_valid_1(item)),
                    _ => false,
                }
            }
            fn is_branch_valid_1(instance: &serde_json::Value) -> bool {
                match instance { serde_json::Value::Number(_) => true, _ => false }
            }
        }
    }

    // Mutations inspect handwritten caller source only. Generated bodies are
    // classified once by operations, then borrowed as its unique Recipe.
    fn edited(source: &[u8], name: &str, change: &mut dyn FnMut(&mut syn::Block)) -> Vec<u8> {
        struct Edit<'a> {
            name: &'a str,
            change: &'a mut dyn FnMut(&mut syn::Block),
            hits: usize,
        }
        impl VisitMut for Edit<'_> {
            fn visit_item_fn_mut(&mut self, item: &mut syn::ItemFn) {
                if item.sig.ident == self.name {
                    (self.change)(&mut item.block);
                    self.hits += 1;
                } else {
                    visit_mut::visit_item_fn_mut(self, item);
                }
            }
            fn visit_impl_item_fn_mut(&mut self, item: &mut syn::ImplItemFn) {
                if item.sig.ident == self.name {
                    (self.change)(&mut item.block);
                    self.hits += 1;
                } else {
                    visit_mut::visit_impl_item_fn_mut(self, item);
                }
            }
        }
        let mut syntax = syn::parse_file(std::str::from_utf8(source).unwrap()).unwrap();
        let mut edit = Edit {
            name,
            change,
            hits: 0,
        };
        edit.visit_file_mut(&mut syntax);
        assert_eq!(edit.hits, 1, "one actual caller root: {name}");
        syntax.to_token_stream().to_string().into_bytes()
    }

    #[test]
    fn dimensions_vary_independently() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            context_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        // B varies with J/D fixed; J varies with B/D fixed; D varies with B/J fixed.
        for (value, bytes, nodes, depth, values, keys, containers, serializations) in [
            (json!("a"), 3, 1, 0, 1, 0, 0, 1),
            (json!("abcd"), 6, 1, 0, 1, 0, 0, 1),
            (json!([1, 2]), 5, 3, 1, 3, 0, 1, 2),
            (json!([123]), 5, 2, 1, 2, 0, 1, 1),
            (json!([[0]]), 5, 3, 2, 3, 0, 2, 1),
            (json!([]), 2, 1, 1, 1, 0, 1, 0),
            (json!({}), 2, 1, 1, 1, 0, 1, 0),
            (json!(false), 5, 1, 0, 1, 0, 0, 1),
            (
                json!({"\u{00e9}\"\n":"\u{1f642}\\\t\u{0000}"}),
                27,
                3,
                1,
                2,
                1,
                1,
                2,
            ),
        ] {
            let expected = SchemaInputStats {
                serialized_bytes: bytes,
                nodes,
                max_depth: depth,
            };
            assert_eq!(
                measure_schema_input(&value, context.native_input_limits),
                Ok(expected)
            );
            let counts = successful(input_counts(&context, &value, context.native_input_limits));
            assert_eq!(counts.stats, expected);
            assert_eq!(counts.value_visits, values);
            assert_eq!(counts.key_visits, keys);
            assert_eq!(counts.container_entries, containers);
            assert_eq!(counts.scalar_serializations, serializations);
            if serializations != 0 {
                assert!(counts.unresolved_calls.contains(&"serde_json::to_writer"));
            }
        }
        let changed = edited(INPUT, "measure_schema_input", &mut |body| {
            assert_eq!(body.stmts.len(), 4);
            let guard = body.stmts.remove(0);
            body.stmts.insert(2, guard); // Traversal before the invalid-depth refusal.
        });
        let changed_sources = ContextSources {
            input: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::Input))
        );
        let changed = edited(JSON_SIZE, "serialized_json_bytes", &mut |body| {
            *body = syn::parse_quote!({
                let bytes = serde_json::to_vec(value)?;
                Ok(u64::try_from(bytes.len()).unwrap())
            });
        });
        let changed_sources = ContextSources {
            json_size: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::JsonSize))
        );
    }

    #[test]
    fn expiry_prefixes_bound_real_call_sites() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            context_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        assert_native_prefix(
            &context,
            NativePath::EntryExpired,
            [0, 0, 0, 0, 1, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            NativePath::Unavailable,
            [0, 0, 0, 0, 1, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            NativePath::InputRejected(SchemaInputError::Bytes),
            [1, 0, 0, 0, 1, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            NativePath::BudgetRejected(SchemaEvaluationError::Deadline),
            [1, 1, 0, 0, 2, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            NativePath::BudgetRejected(SchemaEvaluationError::AggregateWork),
            [1, 1, 0, 0, 2, 0, 0, 1, 1],
        );
        assert_native_prefix(
            &context,
            NativePath::Returned { valid: true },
            [1, 1, 1, 1, 3, 1, 1, 0, 0],
        );
        assert_native_prefix(
            &context,
            NativePath::Returned { valid: false },
            [1, 1, 1, 1, 3, 1, 1, 1, 1],
        );
        assert_native_prefix(
            &context,
            NativePath::ValidatorPanicked,
            [1, 1, 1, 0, 2, 1, 1, 0, 0],
        );
        let changed = edited(NATIVE, "validate_native_parameters", &mut |body| {
            body.stmts.swap(0, 1)
        });
        let changed_sources = ContextSources {
            native: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::Native))
        );
        let changed = edited(BUDGET, "evaluate", &mut |body| {
            let tail = body.stmts.pop().unwrap();
            let debit = body.stmts.pop().unwrap();
            assert!(matches!(
                &debit,
                syn::Stmt::Expr(syn::Expr::Assign(_), Some(_))
            ));
            assert!(matches!(&tail, syn::Stmt::Expr(syn::Expr::Call(_), None)));
            body.stmts
                .push(syn::parse_quote!(let result = Ok(validate());));
            body.stmts.push(debit);
            body.stmts.push(syn::parse_quote!(result));
        });
        let changed_sources = ContextSources {
            budget: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::Budget))
        );
    }

    #[test]
    fn attempts_before_admission_are_charged() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            context_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let mut outer_only = population(NativePath::Returned { valid: true });
        let before = successful(prefix_counts(&context, facts(), outer_only));
        assert_eq!((before.native_entries, before.debit_events), (1, 1));
        assert_eq!(outer_only.scopes_entered, 1);
        assert_eq!(outer_only.retain_visits, 1);
        outer_only.retain_visits = 3;
        assert_eq!(outer_only.missing_parameters, 0);
        outer_only.missing_parameters = 3;
        assert_eq!(outer_only.native_attempts, 1);
        outer_only.native_attempts = 0;
        assert_eq!(
            outer_only.native_path.take(),
            Some(NativePath::Returned { valid: true })
        );
        let counts = successful(prefix_counts(&context, facts(), outer_only));
        assert_eq!(
            (
                counts.outer_scopes,
                counts.retain_visits,
                counts.error_rows,
                counts.missing_parameter_errors,
                counts.removed_impressions
            ),
            (1, 3, 3, 3, 3)
        );
        assert_eq!(
            (
                counts.native_entries,
                counts.preflight_entries,
                counts.budget_entries,
                counts.debit_events,
                counts.native_clock_calls,
                counts.fresh_cache_calls
            ),
            (0, 0, 0, 0, 0, 0)
        );
        assert!(counts.unresolved.contains(&Primitive::Cleanup));
        for reason in [
            SchemaInputError::Bytes,
            SchemaInputError::Nodes,
            SchemaInputError::Depth,
            SchemaInputError::Representation,
        ] {
            let attempts = Population {
                native_attempts: 3,
                retain_visits: 3,
                ..population(NativePath::InputRejected(reason))
            };
            let counts = successful(prefix_counts(&context, facts(), attempts));
            assert_eq!(
                (
                    counts.native_entries,
                    counts.preflight_entries,
                    counts.native_clock_calls
                ),
                (3, 3, 3)
            );
            assert_eq!(
                (
                    counts.budget_entries,
                    counts.debit_events,
                    counts.fresh_cache_calls
                ),
                (0, 0, 0)
            );
        }
        // These generic API errors cannot arise from native's fixed, positive,
        // bounded projection. Do not manufacture additional native paths.
        for path in [
            NativePath::InputRejected(SchemaInputError::InvalidDepthLimit),
            NativePath::BudgetRejected(SchemaEvaluationError::ZeroWork),
            NativePath::BudgetRejected(SchemaEvaluationError::CostOverflow),
        ] {
            assert_eq!(
                prefix_counts(&context, facts(), population(path)),
                Err(Refusal::UnsupportedNativePath)
            );
        }
        let mut missing = facts();
        assert_eq!(missing.original_impressions.take(), Some(3));
        assert_eq!(
            prefix_counts(&context, missing, outer_only),
            Err(Refusal::MissingCallerFact(CallerFact::OriginalImpressions))
        );
        let mut missing = facts();
        assert_eq!(missing.configured_bidder_cap.take(), Some(2));
        assert_eq!(
            prefix_counts(&context, missing, outer_only),
            Err(Refusal::MissingCallerFact(CallerFact::BidderCap))
        );
        let changed = edited(EXCHANGE_SCHEMA, "retain_valid", &mut |body| {
            *body = syn::parse_quote!({
                request.imp.retain(|imp| {
                    let value = imp
                        .ext
                        .as_ref()
                        .and_then(|ext| ext.bidder.as_ref())
                        .unwrap_or(&Value::Null);
                    match validate(value) {
                        Ok(()) => true,
                        Err(reason) => {
                            errors.push(error(reason));
                            false
                        }
                    }
                });
            });
        });
        let changed_sources = ContextSources {
            exchange_schema: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::ExchangeSchema))
        );
        let other = json!({"identity":"fixture_capture_b"});
        let other_binding = ContextBinding {
            descriptor_binding: &other,
            ..binding
        };
        assert_eq!(
            bind_attempt_context(&other_binding, &sources).map(|_| ()),
            Err(Refusal::BindingMismatch)
        );
    }

    #[test]
    fn branches_and_callbacks_have_source_derived_multiplicity() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            context_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let syntax = callback_body();
        let operations = classified(&syntax);
        let dimensions = SchemaInputStats {
            serialized_bytes: 7,
            nodes: 4,
            max_depth: 1,
        };
        assert_eq!(
            measure_schema_input(&json!([1, 2, 3]), context.native_input_limits),
            Ok(dimensions)
        );
        let counts = successful(recipe_counts(&context, operations.recipe(), dimensions, 1));
        assert_eq!(counts.root_entries, 1);
        assert_eq!(counts.helpers.get("is_branch_valid_0"), Some(&2));
        assert_eq!(counts.helpers.get("is_branch_valid_1"), Some(&6));
        assert_eq!(counts.iterator_callbacks, 6); // One callback site executes repeatedly.
        assert_eq!(
            (
                counts.searches,
                counts.fresh_cache_calls,
                counts.cache_drop_entries
            ),
            (0, 0, 0)
        );
        assert_eq!(counts.seed_invocations, None); // Opaque primitive closure is not seed-free proof.
        assert!(counts.unresolved.contains(&Primitive::IteratorProtocol));
        let search_syntax: syn::File = syn::parse_quote! {
            struct Fixture;
            struct FixtureSearchCache {
                __inner: __fixture_impl::__SearchCache,
            }
            impl Fixture {
                /// Initializes every generated pattern regex, including after a
                /// failed constructor. Returns false if any cached construction
                /// failed. Repeated calls do not reconstruct or retry regexes.
                /// This does not initialize custom formats or bound search caches.
                pub fn initialize_regexes() -> bool {
                    __fixture_impl::initialize_regexes()
                }
                pub fn new_search_cache() -> Option<FixtureSearchCache> {
                    __fixture_impl::new_search_cache()
                        .map(|__inner| FixtureSearchCache { __inner })
                }
                pub fn is_valid(
                    input: &regex_automata::Input<'_>,
                    __cache: &mut FixtureSearchCache,
                ) -> bool {
                    { __fixture_impl::entry_is_valid(input, &mut __cache.__inner) }
                }
            }
            mod __fixture_impl {
                static PATTERN_REGEX: std::sync::LazyLock<Option<regex_automata::meta::Regex>> =
                    std::sync::LazyLock::new(|| regex_automata::meta::Regex::new("a").ok());

                pub(super) struct __SearchCache {
                    pattern_cache: regex_automata::meta::Cache,
                }
                impl __SearchCache {
                    fn reset(&mut self) {}
                }
                pub(super) fn initialize_regexes() -> bool {
                    PATTERN_REGEX.as_ref().is_some()
                }
                pub(super) fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache {
                        pattern_cache: PATTERN_REGEX.as_ref()?.create_cache(),
                    })
                }
                pub(super) fn entry_is_valid(
                    input: &regex_automata::Input<'_>,
                    cache: &mut __SearchCache,
                ) -> bool {
                    cache.reset();
                    PATTERN_REGEX.as_ref().is_some_and(|pattern| {
                        pattern.search_half_with(&mut cache.pattern_cache, input).is_some()
                            && pattern.search_half_with(&mut cache.pattern_cache, input).is_some()
                    })
                }
            }
        };
        let search = classified(&search_syntax);
        assert_eq!(
            search
                .recipe()
                .roots
                .iter()
                .map(|id| search.recipe().functions[id].identity.as_str())
                .collect::<Vec<_>>(),
            vec!["Fixture::is_valid", "Fixture::new_search_cache"]
        );
        assert_eq!(
            search
                .recipe()
                .initialization
                .get("__fixture_impl::initialize_regexes"),
            Some(&vec!["__fixture_impl::PATTERN_REGEX".to_owned()])
        );
        let counts = successful(recipe_counts(&context, search.recipe(), dimensions, 5));
        // Five episodes enter both coupled roots: ten function-root entries.
        // Constructor attempts remain a distinct count, not additional searches.
        assert_eq!(counts.root_entries, 10);
        assert_eq!(counts.fresh_cache_calls, 5);
        assert_eq!(counts.searches, 10); // Not J=4, callbacks=6, or a flat site census of 2.
        assert!(counts.unresolved.contains(&Primitive::RegexSearch));
        assert!(counts.unresolved.contains(&Primitive::CacheConstruction));
        assert!(counts.unresolved.contains(&Primitive::Cleanup));
        assert_eq!(counts.seed_invocations, None);
        let changed_bytes = edited(
            search_syntax.to_token_stream().to_string().as_bytes(),
            "entry_is_valid",
            &mut |body| {
                let before: syn::Block = syn::parse_quote!({
                    cache.reset();
                    PATTERN_REGEX.as_ref().is_some_and(|pattern| {
                        pattern
                            .search_half_with(&mut cache.pattern_cache, input)
                            .is_some()
                            && pattern
                                .search_half_with(&mut cache.pattern_cache, input)
                                .is_some()
                    })
                });
                assert_eq!(
                    body.to_token_stream().to_string(),
                    before.to_token_stream().to_string()
                );
                *body = syn::parse_quote!({
                    cache.reset();
                    PATTERN_REGEX.as_ref().is_some_and(|pattern| {
                        pattern
                            .search_half_with(&mut cache.pattern_cache, input)
                            .is_some()
                            && pattern
                                .search_half_with(&mut cache.pattern_cache, input)
                                .is_some()
                            && pattern
                                .search_half_with(&mut cache.pattern_cache, input)
                                .is_some()
                    })
                });
            },
        );
        let changed_syntax = syn::parse_file(std::str::from_utf8(&changed_bytes).unwrap()).unwrap();
        let changed = classified(&changed_syntax);
        assert_eq!(changed.recipe().roots, search.recipe().roots);
        assert_eq!(
            changed.recipe().initialization,
            search.recipe().initialization
        );
        let counts = successful(recipe_counts(&context, changed.recipe(), dimensions, 5));
        assert_eq!(counts.root_entries, 10);
        assert_eq!(counts.fresh_cache_calls, 5);
        assert_eq!(counts.searches, 15);
    }

    #[test]
    fn late_failure_includes_cleanup() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            context_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        for path in [
            NativePath::Returned { valid: false },
            NativePath::ValidatorPanicked,
        ] {
            let counts = successful(prefix_counts(&context, facts(), population(path)));
            assert_eq!(counts.debit_events, 1);
            assert_eq!(counts.fresh_cache_calls, 1);
            assert_eq!(counts.cache_drop_entries, 1);
            assert_eq!(counts.seed_invocations, None); // Not the cache count, nor a free seed body.
            assert!(counts.unresolved.contains(&Primitive::CacheConstruction));
            assert!(counts.unresolved.contains(&Primitive::Cleanup));
        }
        let mut missing = facts();
        assert_eq!(missing.panic_unwinds.take(), Some(true));
        assert_eq!(
            prefix_counts(&context, missing, population(NativePath::ValidatorPanicked)),
            Err(Refusal::MissingCallerFact(CallerFact::PanicUnwind))
        );
        let operations = classified(&callback_body());
        assert!(reachable_duties(operations.recipe()).contains(&Primitive::IteratorProtocol));
        assert_eq!(
            require_source_leaf_cost(&context, operations.recipe(), Primitive::IteratorProtocol),
            Err(Refusal::MissingRuntimeLemma(Primitive::IteratorProtocol))
        );
        let primitive_free: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                match instance { serde_json::Value::String(_) => true, _ => false }
            }
        };
        let primitive_free = classified(&primitive_free);
        assert!(!reachable_duties(primitive_free.recipe()).contains(&Primitive::IteratorProtocol));
        assert_eq!(
            require_source_leaf_cost(
                &context,
                primitive_free.recipe(),
                Primitive::IteratorProtocol
            ),
            Err(Refusal::NoSourceLeafDuty(Primitive::IteratorProtocol))
        );
        let changed = edited(DISPATCH_GENERATOR, "native_dispatch", &mut |body| {
            *body = syn::parse_quote!({
                quote! {
                    fn validate_native(owner: &str, value: &serde_json::Value) -> Option<bool> { None }
                }
            });
        });
        let changed_sources = ContextSources {
            dispatch_generator: &changed,
            ..sources
        };
        assert_eq!(
            bind_attempt_context(&binding, &changed_sources).map(|_| ()),
            Err(Refusal::SourceShape(SourceRole::DispatchGenerator))
        );
    }

    #[test]
    fn overflow_refuses() {
        let descriptor = json!({"identity":"fixture_capture_a"});
        let roots = json!({"fixture":"caller_roots"});
        let census = json!({"fixture":"existing_compiled_source_provenance"});
        let binding = ContextBinding {
            descriptor_binding: &descriptor,
            context_binding: &descriptor,
            context_roots: &roots,
            compiled_sources: &census,
        };
        let sources = sources();
        let context = context(&binding, &sources);
        let operations = classified(&callback_body());
        let dimensions = SchemaInputStats {
            serialized_bytes: 7,
            nodes: 4,
            max_depth: 1,
        };
        assert_eq!(
            recipe_counts(&context, operations.recipe(), dimensions, u64::MAX),
            Err(Refusal::Overflow)
        );
        let mut caller = facts();
        assert_eq!(caller.original_impressions.replace(u64::MAX), Some(3));
        let many = Population {
            retain_visits: u64::MAX,
            native_attempts: u64::MAX,
            ..population(NativePath::Returned { valid: true })
        };
        assert_eq!(
            prefix_counts(&context, caller, many),
            Err(Refusal::Overflow)
        );
    }
}

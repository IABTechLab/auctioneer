//! Task 3 finite source recognizer, not a common-source work certificate.
//!
//! Fixtures use the selected emitter's call shapes. Supplied ASTs, identities
//! and digests grant no source authority; the parent must join pristine bodies.
//! Primitive lemmas remain obligations, not zero-cost calls or fixture proofs.
//!
//! The ledger counts executed source events, never file/AST size, unique states,
//! elapsed time, machine instructions or native admission tokens:
//! - `source_visits`: a supported executable source-node/entry event, repeated
//!   whenever control reaches it, including primitive-library loop bodies.
//! - `control_visits`: an evaluated branch/guard, short-circuit or loop decision.
//! - `iterator_visits`: an iterator advancement, including a terminating `None`.
//! - `map_visits`: a bucket/probe or tree-node examination, not expected O(1).
//! - `property_visits`: an object-member/rule examination; key bytes are separate.
//! - `scalar_comparisons`: a scalar type, equality, order or predicate test.
//! - `scanned_bytes`: an inspected byte, including keys, strings and encoded states.
//! - `copied_bytes`: an initialized, formatted or copied byte outside relocation.
//! - `equality_visits`: a structural JSON node-pair comparison.
//! - `uniqueness_visits`: an item/pair examination by uniqueness machinery.
//! - `construction_visits`: an owner/element initialization or allocation request.
//! - `relocated_bytes`: a byte moved specifically for capacity growth.
//! - `cleanup_visits`: an owned element, bucket or reference release, including
//!   a deallocation request, on normal, partial-construction and failure paths.
//! - `search_transitions`: a search state/edge/transition processing event,
//!   including speculative lookups, misses and epsilon-closure processing.
//!
//! Different fields are not summed into a score. Immutable program construction
//! stays with C/I. Explicit service bodies have their own matched requirements;
//! their local call/setup/cleanup events remain here. Counting an allocation
//! request is not a claim about allocator CPU/latency or physical memory. Every
//! opaque primitive still needs its own supported source lemma; naming a call,
//! hashing its body or observing a sample cannot turn an unknown count into zero.
//! Compiler-inserted checks inside a matched intrinsic indexing entry belong
//! to its source-visit classification, not separate source predicate/control
//! events. Explicit Rust predicates still count normally. This grouping does
//! not assign zero CPU cost or establish an instruction/latency bound.

use std::collections::{BTreeMap, BTreeSet};

mod lower;
pub(super) mod model;
mod resolve;

// One service body covers warm/cold TLS, process state and platform seeding.
// HashMap setup, ownership, hashing, probing, growth and drop remain common-source obligations.
const ENTROPY_ENTRY: &str = "std::hash::RandomState::new";

// Distinct source units, never an additive total, CPU units or native tokens.
// None denotes an unproved count; Some(0) needs a source-proven absence.
#[derive(Debug, Default, Eq, PartialEq)]
pub(in crate::schema_validation) struct WorkLedger {
    source_visits: Option<u64>,
    control_visits: Option<u64>,
    iterator_visits: Option<u64>,
    map_visits: Option<u64>,
    property_visits: Option<u64>,
    scalar_comparisons: Option<u64>,
    scanned_bytes: Option<u64>,
    copied_bytes: Option<u64>,
    equality_visits: Option<u64>,
    uniqueness_visits: Option<u64>,
    construction_visits: Option<u64>,
    relocated_bytes: Option<u64>,
    cleanup_visits: Option<u64>,
    search_transitions: Option<u64>,
}

impl WorkLedger {
    // Supplied partial event counts, not a source-proof constructor. Every
    // other component stays unproved rather than becoming an implicit zero.
    pub(in crate::schema_validation) fn source_events(
        source: u64,
        control: u64,
        scalar: u64,
    ) -> Self {
        Self {
            source_visits: Some(source),
            control_visits: Some(control),
            scalar_comparisons: Some(scalar),
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Primitive {
    UuidFormat,
    StateMapConstruction,
    ValueAccess,
    StringAccess,
    MapAccess,
    IteratorProtocol,
    Numeric,
    RegexSearch,
    CacheConstruction,
    Readiness,
    HostnameFormat,
    Equality,
    Uniqueness,
    Cleanup,
}

#[derive(Clone)]
pub(super) struct PrimitiveSource {
    primitive: Primitive,
    callee: &'static str,
    receiver: &'static str,
    body: syn::ItemFn,
    source_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraversalMeasure {
    ValuesAndKeys,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExternalService {
    Clock,
    HostEntropy,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ServiceFact {
    Work,
    TemporaryStorage,
    RetainedState,
    Stack,
    ThreadMultiplicity,
    /// Requires a matched process-state contract covering identity, shared
    /// ownership, lifetime and process population; never an `N_seed` multiple.
    ProcessRetainedState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetainedScope {
    Caller,
    Thread,
    /// The whole service body may retain thread- and process-lifetime state.
    ThreadAndProcess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TemporaryScope {
    OutsideCache,
    CacheConstruction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ServiceBinding {
    service: ExternalService,
    entry: &'static str,
    receiver: &'static str,
    retained: RetainedScope,
    temporary: TemporaryScope,
    required_facts: BTreeSet<ServiceFact>,
}

#[derive(Debug, Eq, PartialEq)]
struct ServiceRequirement {
    binding: ServiceBinding,
    invocations: u64,
}

// A local classification only. There is no completed-proof/report conversion.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct BodyOperations {
    // The maps below are source-site/per-body test projections, not invocation
    // bounds. Work and stack must consume this one owned structured recipe.
    recipe: model::Recipe,
    ledger: WorkLedger,
    calls: BTreeMap<String, u64>,
    // Every recorded site has the one supported ValuesAndKeys measure.
    traversals: BTreeSet<String>,
    required_lemmas: BTreeSet<Primitive>,
    services: Vec<ServiceRequirement>,
}

impl BodyOperations {
    pub(super) fn recipe(&self) -> &model::Recipe {
        &self.recipe
    }

    fn traversal_measure(&self, site: &str) -> Option<TraversalMeasure> {
        self.traversals
            .contains(site)
            .then_some(TraversalMeasure::ValuesAndKeys)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    UnclassifiedCall,
    ReceiverMismatch,
    UnboundedTraversal,
    MissingDecreasingMeasure,
    PrimitiveSemanticsChanged,
    MissingServiceBinding(ExternalService),
    ServiceIdentity(ExternalService),
    ServiceLifetime(ExternalService),
    MissingServiceFact(ExternalService, ServiceFact),
    UnknownCallback,
    UnsupportedNestedLiteralOwner,
    UnsupportedSource,
    Overflow,
    AtSource {
        function: String,
        lexical_site: Vec<usize>,
        cause: Box<Self>,
    },
}

// Recognition only declares source edges and outstanding lemmas. Neither a
// supplied body nor its digest can complete work, storage or target-stack proof.
pub(super) fn classify_source_body(
    syntax: &syn::File,
    primitives: &[PrimitiveSource],
    services: &[ServiceBinding],
) -> Result<BodyOperations, Refusal> {
    let index = resolve::Index::collect(syntax)?;
    let recipe = lower::recognize(&index, primitives, services)?;
    recipe.project()
}

#[cfg(test)]
mod tests {
    use super::{
        ENTROPY_ENTRY, ExternalService, Primitive, PrimitiveSource, Refusal, RetainedScope,
        ServiceBinding, ServiceFact, TemporaryScope, TraversalMeasure, classify_source_body,
    };
    use quote::ToTokens as _;
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeSet;
    use std::slice;

    fn body() -> syn::File {
        syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                is_branch_valid_0(instance)
            }
            fn is_branch_valid_0(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::String(_) => true,
                    _ => false,
                }
            }
        }
    }

    fn root(syntax: &mut syn::File) -> &mut syn::ItemFn {
        let syn::Item::Fn(function) = syntax.items.first_mut().expect("fixture entry present")
        else {
            panic!("fixture entry must be a function");
        };
        function
    }

    fn fingerprint(body: &syn::ItemFn) -> String {
        hex::encode(Sha256::digest(
            body.to_token_stream().to_string().as_bytes(),
        ))
    }

    fn binding(service: ExternalService) -> ServiceBinding {
        let (entry, receiver, retained, temporary) = match service {
            ExternalService::Clock => (
                "fixture::RequestClock::expired",
                "fixture::RequestClock",
                RetainedScope::Caller,
                TemporaryScope::OutsideCache,
            ),
            ExternalService::HostEntropy => (
                ENTROPY_ENTRY,
                "std::hash::RandomState",
                RetainedScope::ThreadAndProcess,
                TemporaryScope::CacheConstruction,
            ),
        };
        let mut required_facts = BTreeSet::from([
            ServiceFact::Work,
            ServiceFact::TemporaryStorage,
            ServiceFact::RetainedState,
            ServiceFact::Stack,
        ]);
        if service == ExternalService::HostEntropy {
            required_facts.insert(ServiceFact::ThreadMultiplicity);
            required_facts.insert(ServiceFact::ProcessRetainedState);
        }
        ServiceBinding {
            service,
            entry,
            receiver,
            retained,
            temporary,
            required_facts,
        }
    }

    #[test]
    fn unclassified_call_refuses_completion() {
        let original = body();
        classify_source_body(&original, &[], &[]).expect("original body is classified");
        let mut attributed = original.clone();
        root(&mut attributed)
            .attrs
            .push(syn::parse_quote!(#[opaque_source_transform]));
        assert_eq!(
            classify_source_body(&attributed, &[], &[]),
            Err(Refusal::UnsupportedSource)
        );
        assert_coupled_source_contract();
        assert_call_refusals(&original);
    }

    fn assert_coupled_source_contract() {
        let coupled: syn::File = syn::parse_quote! {
            struct Captured;
            #[doc(hidden)]
            #[allow(non_snake_case, dead_code, unused_imports, unused_variables, unreachable_code, clippy::all)]
            mod __captured_impl {
                pub(super) struct __SearchCache {}
                pub(super) fn initialize_regexes() -> bool { true }
                pub(super) fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache {})
                }
                #[inline]
                pub(super) fn entry_is_valid(instance: &serde_json::Value, __cache: &mut __SearchCache) -> bool {
                    match instance { serde_json::Value::String(_) => true, _ => false }
                }
            }
            impl Captured {
                #[doc = " Initializes every generated pattern regex, including after a"]
                #[doc = " failed constructor. Returns false if any cached construction"]
                #[doc = " failed. Repeated calls do not reconstruct or retry regexes."]
                #[doc = " This does not initialize custom formats or bound search caches."]
                pub fn initialize_regexes() -> bool { __captured_impl::initialize_regexes() }
                fn new_search_cache() -> Option<CapturedSearchCache> {
                    __captured_impl::new_search_cache().map(|__inner| CapturedSearchCache { __inner })
                }
                fn is_valid(instance: &serde_json::Value, __cache: &mut CapturedSearchCache) -> bool {
                    __captured_impl::entry_is_valid(instance, &mut __cache.__inner)
                }
            }
            struct CapturedSearchCache { __inner: __captured_impl::__SearchCache }
        };
        classify_source_body(&coupled, &[], &[]).expect("coupled body is classified");
        let mut changed_documentation = coupled.clone();
        let syn::Item::Impl(marker) = &mut changed_documentation.items[2] else {
            panic!("coupled marker impl must be present");
        };
        let syn::ImplItem::Fn(initializer) = &mut marker.items[0] else {
            panic!("coupled initialization wrapper must be present");
        };
        initializer.attrs[0] = syn::parse_quote!(#[doc = "different documentation"]);
        assert_eq!(
            classify_source_body(&changed_documentation, &[], &[]),
            Err(Refusal::UnsupportedSource)
        );
        let mut changed_signature = coupled.clone();
        let syn::Item::Impl(signature_marker) = &mut changed_signature.items[2] else {
            panic!("coupled marker impl must be present");
        };
        let syn::ImplItem::Fn(signature_initializer) = &mut signature_marker.items[0] else {
            panic!("coupled initialization wrapper must be present");
        };
        signature_initializer
            .sig
            .inputs
            .push(syn::parse_quote!(unexpected: bool));
        assert_eq!(
            classify_source_body(&changed_signature, &[], &[]),
            Err(Refusal::UnsupportedSource)
        );
        let mut helper_attribute = coupled.clone();
        let syn::Item::Mod(module) = &mut helper_attribute.items[1] else {
            panic!("coupled module must be present");
        };
        let (_, items) = module.content.as_mut().unwrap();
        let syn::Item::Fn(helper) = &mut items[3] else {
            panic!("coupled entry helper must be present");
        };
        helper
            .attrs
            .push(syn::parse_quote!(#[cfg_attr(all(), inline)]));
        assert_eq!(
            classify_source_body(&helper_attribute, &[], &[]),
            Err(Refusal::UnsupportedSource)
        );
        let mut field_attribute = coupled.clone();
        let syn::Item::Struct(cache) = &mut field_attribute.items[3] else {
            panic!("coupled wrapper cache must be present");
        };
        cache
            .fields
            .iter_mut()
            .next()
            .unwrap()
            .attrs
            .push(syn::parse_quote!(#[opaque_source_transform]));
        assert_eq!(
            classify_source_body(&field_attribute, &[], &[]),
            Err(Refusal::UnsupportedSource)
        );
        let mut altered_allow = coupled;
        let syn::Item::Mod(altered_module) = &mut altered_allow.items[1] else {
            panic!("coupled module must be present");
        };
        altered_module.attrs[1] = syn::parse_quote!(#[allow(dead_code)]);
        assert_eq!(
            classify_source_body(&altered_allow, &[], &[]),
            Err(Refusal::UnsupportedSource)
        );
    }

    fn assert_call_refusals(original: &syn::File) {
        let mut changed = original.clone();
        *root(&mut changed).block = syn::parse_quote!({ unknown(instance) });
        assert_eq!(
            classify_source_body(&changed, &[], &[]),
            Err(Refusal::UnclassifiedCall)
        );
        let receiver: syn::File = syn::parse_quote! {
            fn is_valid(instance: &ForeignValue) -> bool { instance.is_string() }
        };
        assert_eq!(
            classify_source_body(&receiver, &[], &[]),
            Err(Refusal::ReceiverMismatch)
        );
        let bounded: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::Array(arr) => arr.iter().all(|item| {
                        match item { serde_json::Value::String(_) => true, _ => false }
                    }),
                    _ => false,
                }
            }
        };
        let selected_result = classify_source_body(&bounded, &[], &[]);
        assert_eq!(
            selected_result
                .as_ref()
                .map(|operations| operations.traversal_measure("is_valid::arr")),
            Ok(Some(TraversalMeasure::ValuesAndKeys))
        );
        let selected = selected_result.unwrap();
        let mut iteration_sites = 0_usize;
        for function in selected.recipe().functions.values() {
            super::model::walk(&function.body, &mut |op| {
                if let super::model::Kind::Iterate {
                    callee,
                    stop,
                    none_on_exhaustion,
                    bound,
                    parameters,
                    ..
                } = &op.kind
                {
                    iteration_sites += 1;
                    assert_eq!(callee, "Iterator::all");
                    assert_eq!(*stop, super::model::StopRule::FirstFalseOrExhaustion);
                    assert!(*none_on_exhaustion);
                    assert_eq!(bound.collection.ty, super::model::Ty::Array);
                    assert!(bound.collection.borrowed);
                    assert_eq!(bound.element, super::model::Ty::Value);
                    assert_eq!(parameters.len(), 1);
                    assert!(parameters[0].borrowed);
                }
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(iteration_sites, 1);
        let shadowed: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                let is_branch_valid_0 = false;
                is_branch_valid_0(instance)
            }
            fn is_branch_valid_0(instance: &serde_json::Value) -> bool { true }
        };
        assert_eq!(
            classify_source_body(&shadowed, &[], &[]),
            Err(Refusal::UnknownCallback)
        );
        let unbounded: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                loop { if instance.is_string() { return true; } }
            }
        };
        assert_eq!(
            classify_source_body(&unbounded, &[], &[]),
            Err(Refusal::UnboundedTraversal)
        );
        *root(&mut changed).block = syn::parse_quote!({ is_valid(instance) });
        assert_eq!(
            classify_source_body(&changed, &[], &[]),
            Err(Refusal::MissingDecreasingMeasure)
        );
    }

    #[test]
    fn changed_branch_multiplicity_changes_the_recipe() {
        let original = body();
        let before_result = classify_source_body(&original, &[], &[]);
        assert_eq!(
            before_result
                .as_ref()
                .map(|operations| operations.calls.get("is_branch_valid_0")),
            Ok(Some(&1))
        );
        let before = before_result.unwrap();
        let mut changed = original.clone();
        *root(&mut changed).block =
            syn::parse_quote!({ is_branch_valid_0(instance) && is_branch_valid_0(instance) });
        let after_result = classify_source_body(&changed, &[], &[]);
        assert_eq!(
            after_result
                .as_ref()
                .map(|operations| operations.calls.get("is_branch_valid_0")),
            Ok(Some(&2))
        );
        let after = after_result.unwrap();
        assert_ne!(before.calls, after.calls);
        assert_ne!(before.recipe(), after.recipe());
    }

    #[test]
    fn deep_boolean_source_preserves_every_occurrence() {
        let mut source: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool { first(instance) }
            fn first(instance: &serde_json::Value) -> bool {
                match instance { serde_json::Value::String(_) => true, _ => false }
            }
            fn second(instance: &serde_json::Value) -> bool {
                match instance { serde_json::Value::Bool(_) => true, _ => false }
            }
        };
        let before = classify_source_body(&source, &[], &[]);
        assert_eq!(
            before
                .as_ref()
                .map(|operations| operations.calls.get("first")),
            Ok(Some(&1))
        );
        drop(before);

        // Match the emitter's left fold, including both parentheses per join.
        // This is source-occurrence coverage, not actual-input execution credit.
        let mut expression: syn::Expr = syn::parse_quote!(first(instance));
        let mut expected_order = vec!["first".to_owned()];
        for ordinal in 1_usize..32 {
            let (next, name): (syn::Expr, &str) = if ordinal.is_multiple_of(2) {
                (syn::parse_quote!(first(instance)), "first")
            } else {
                (syn::parse_quote!(second(instance)), "second")
            };
            expression = syn::parse_quote!((#expression) && (#next));
            expected_order.push(name.to_owned());
        }
        *root(&mut source).block = syn::parse_quote!({ #expression });
        let classified = classify_source_body(&source, &[], &[]);
        assert_eq!(
            classified
                .as_ref()
                .map(|operations| operations.calls.get("first")),
            Ok(Some(&16))
        );
        assert_eq!(
            classified
                .as_ref()
                .map(|operations| operations.calls.get("second")),
            Ok(Some(&16))
        );
        assert_eq!(
            classified
                .as_ref()
                .map(|operations| operations.ledger.scalar_comparisons),
            Ok(Some(32))
        );
        let operations = classified.unwrap();
        let recipe = operations.recipe();
        assert_eq!(recipe.roots.len(), 1);
        let root_id = *recipe.roots.first().unwrap();
        let function = &recipe.functions[&root_id];
        let mut actual_order = Vec::new();
        let mut sites = BTreeSet::new();
        let mut conjunctions = 0_usize;
        super::model::walk(&function.body, &mut |op| {
            assert_eq!(op.site.function, root_id);
            match &op.kind {
                super::model::Kind::Call { target, .. } => {
                    actual_order.push(recipe.functions[target].identity.clone());
                    assert!(sites.insert(op.site.lexical.clone()));
                }
                super::model::Kind::ShortCircuit(junction, operands) => {
                    assert_eq!(*junction, super::model::Junction::And);
                    assert_eq!(operands.len(), 2);
                    conjunctions += 1;
                }
                super::model::Kind::Value(_)
                | super::model::Kind::Scope { .. }
                | super::model::Kind::Bind { .. }
                | super::model::Kind::Evaluate(..)
                | super::model::Kind::ScalarTest(..)
                | super::model::Kind::Choice { .. }
                | super::model::Kind::Primitive { .. }
                | super::model::Kind::Service(_)
                | super::model::Kind::Iterate { .. }
                | super::model::Kind::Callback { .. }
                | super::model::Kind::Construct { .. }
                | super::model::Kind::Exit(..) => {}
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(actual_order, expected_order);
        assert_eq!(sites.len(), 32);
        assert_eq!(conjunctions, 31);
        drop(operations);
        drop(source);
    }

    #[test]
    fn non_regex_declaration_has_a_nonempty_closure() {
        let operations = classify_source_body(&body(), &[], &[])
            .expect("non-regex type closure must be classified");
        let ledger = operations.ledger;
        assert!(ledger.source_visits.is_some_and(|count| count > 0));
        assert!(ledger.control_visits.is_some_and(|count| count > 0));
        assert_eq!(ledger.scalar_comparisons, Some(1));
        assert_eq!(ledger.iterator_visits, Some(0));
        assert_eq!(ledger.map_visits, Some(0));
        assert_eq!(ledger.property_visits, Some(0));
        assert_eq!(ledger.scanned_bytes, Some(0));
        assert_eq!(ledger.copied_bytes, Some(0));
        assert_eq!(ledger.equality_visits, Some(0));
        assert_eq!(ledger.uniqueness_visits, Some(0));
        assert_eq!(ledger.construction_visits, Some(0));
        assert_eq!(ledger.relocated_bytes, Some(0));
        assert_eq!(ledger.cleanup_visits, Some(0));
        assert_eq!(ledger.search_transitions, Some(0));
        assert_nested_decisions_remain_unproved();
        assert_owned_cleanup_remains_required();
        assert_guard_prefix_is_retained();
    }

    fn assert_nested_decisions_remain_unproved() {
        let nested_bool: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::Bool(true) => true,
                    _ => false,
                }
            }
        };
        let nested_some: syn::File = syn::parse_quote! {
            fn is_valid(instance: &Option<serde_json::Value>) -> bool {
                match instance {
                    Some(serde_json::Value::Bool(true)) => true,
                    _ => false,
                }
            }
        };
        for nested in [nested_bool, nested_some] {
            let classified = classify_source_body(&nested, &[], &[]);
            assert_eq!(
                classified
                    .as_ref()
                    .map(|operations| operations.ledger.scalar_comparisons),
                Ok(None)
            );
            assert_eq!(
                classified
                    .as_ref()
                    .map(|operations| operations.ledger.source_visits),
                Ok(None)
            );
        }
    }

    fn assert_owned_cleanup_remains_required() {
        let owned: syn::File = syn::parse_quote! {
            fn is_valid(value: String) -> bool { true }
        };
        let classified = classify_source_body(&owned, &[], &[])
            .expect("owned parameter must retain cleanup obligation");
        assert!(classified.required_lemmas.contains(&Primitive::Cleanup));
        assert_eq!(classified.ledger.cleanup_visits, None);
        assert_eq!(classified.recipe().roots.len(), 1);
    }

    fn assert_guard_prefix_is_retained() {
        let guarded: syn::File = syn::parse_quote! {
            fn is_valid(instance: &serde_json::Value) -> bool {
                match instance {
                    serde_json::Value::String(_) if false => true,
                    serde_json::Value::String(_) if true => false,
                    _ => true,
                }
            }
        };
        let classified = classify_source_body(&guarded, &[], &[])
            .expect("guard prefix must be retained, not maxed away");
        assert_eq!(classified.ledger.control_visits, None);
        assert_eq!(classified.ledger.scalar_comparisons, None);
        let mut guards = 0_usize;
        let mut booleans = BTreeSet::new();
        for function in classified.recipe().functions.values() {
            super::model::walk(&function.body, &mut |op| {
                if let super::model::Kind::Choice { arms, .. } = &op.kind {
                    guards += arms.iter().filter(|arm| arm.guard.is_some()).count();
                }
                if let super::model::Kind::Value(super::model::Atom::Bool(value)) = &op.kind {
                    booleans.insert(*value);
                }
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(guards, 2);
        assert_eq!(booleans, BTreeSet::from([false, true]));
    }

    #[test]
    fn changed_primitive_semantics_is_not_approved_by_rehashing() {
        let syntax: syn::File = syn::parse_quote! {
            fn is_valid(value: &str) -> bool { crate::schema::staging::formats::uuid(value) }
        };
        let original: syn::ItemFn = syn::parse_quote! {
            fn uuid(value: &str) -> bool {
                if value.bytes().any(|byte| byte.is_ascii_uppercase()) { return false; }
                let mut decoded = [0_u8; 16];
                uuid_simd::parse_hyphenated(value.as_bytes(), decoded.as_out()).is_ok()
            }
        };
        let mut primitive = PrimitiveSource {
            primitive: Primitive::UuidFormat,
            callee: "crate::schema::staging::formats::uuid",
            receiver: "str",
            source_sha256: fingerprint(&original),
            body: original,
        };
        assert_eq!(primitive.primitive, Primitive::UuidFormat);
        assert_eq!(primitive.callee, "crate::schema::staging::formats::uuid");
        assert_eq!(primitive.receiver, "str");
        let before_result = classify_source_body(&syntax, slice::from_ref(&primitive), &[]);
        assert_eq!(
            before_result
                .as_ref()
                .map(|operations| operations.required_lemmas.contains(&Primitive::UuidFormat)),
            Ok(true)
        );
        let before = before_result.unwrap();
        assert_eq!(before.ledger.scanned_bytes, None);
        let old_digest = primitive.source_sha256.clone();
        *primitive.body.block = syn::parse_quote!({
            if value.bytes().any(|byte| byte.is_ascii_lowercase()) {
                return false;
            }
            let mut decoded = [0_u8; 16];
            uuid_simd::parse_hyphenated(value.as_bytes(), decoded.as_out()).is_ok()
        });
        primitive.source_sha256 = fingerprint(&primitive.body);
        assert_ne!(primitive.source_sha256, old_digest);
        assert_eq!(
            classify_source_body(&syntax, &[primitive], &[]),
            Err(Refusal::PrimitiveSemanticsChanged)
        );
    }

    #[test]
    fn expiry_binding_is_explicit() {
        let syntax: syn::File = syn::parse_quote! {
            fn is_valid(expired: &mut dyn FnMut() -> bool) -> bool { !expired() }
        };
        let clock = binding(ExternalService::Clock);
        let operations_result = classify_source_body(&syntax, &[], slice::from_ref(&clock));
        assert_eq!(
            operations_result.as_ref().map(|operations| operations
                .services
                .first()
                .map(|service| service.invocations)),
            Ok(Some(1))
        );
        let operations = operations_result.unwrap();
        assert_eq!(operations.services[0].binding, clock);
        assert_eq!(operations.services[0].invocations, 1);
        assert_eq!(
            classify_source_body(&syntax, &[], &[]),
            Err(Refusal::MissingServiceBinding(ExternalService::Clock))
        );
    }

    #[test]
    fn unknown_callback_is_not_a_free_source_edge() {
        let clock = binding(ExternalService::Clock);
        let known: syn::File = syn::parse_quote! {
            fn is_valid(expired: &mut dyn FnMut() -> bool) -> bool { !expired() }
        };
        classify_source_body(&known, &[], slice::from_ref(&clock))
            .expect("known callback is classified");
        let unknown: syn::File = syn::parse_quote! {
            fn is_valid(other: &mut dyn FnMut() -> bool) -> bool { !other() }
        };
        assert_eq!(
            classify_source_body(&unknown, &[], &[clock]),
            Err(Refusal::UnknownCallback)
        );
    }

    #[test]
    fn new_nested_literal_owner_requires_a_supported_proof() {
        classify_source_body(&body(), &[], &[]).expect("original body is classified");
        let nested: syn::File = syn::parse_quote! {
            fn is_valid(matcher: &aho_corasick::AhoCorasick, haystack: &[u8]) -> bool {
                matcher.find(haystack).is_some()
            }
        };
        assert_eq!(
            classify_source_body(&nested, &[], &[]),
            Err(Refusal::UnsupportedNestedLiteralOwner)
        );
    }

    #[test]
    fn cold_entropy_has_exact_identity_lifetime_and_nonfree_obligations() {
        let syntax: syn::File = syn::parse_quote! {
            fn new_search_cache() -> (StateMap, StateMap) {
                (std::collections::HashMap::new(), std::collections::HashMap::new())
            }
        };
        let constructor: syn::ItemFn = syn::parse_quote! {
            fn new() -> HashMap<K, V, RandomState> { Default::default() }
        };
        let primitive = PrimitiveSource {
            primitive: Primitive::StateMapConstruction,
            callee: "std::collections::HashMap::new",
            receiver: "std::collections::HashMap",
            source_sha256: fingerprint(&constructor),
            body: constructor,
        };
        assert_eq!(primitive.primitive, Primitive::StateMapConstruction);
        let entropy = binding(ExternalService::HostEntropy);
        let operations_result = classify_source_body(
            &syntax,
            slice::from_ref(&primitive),
            slice::from_ref(&entropy),
        );
        assert_eq!(
            operations_result.as_ref().map(|operations| operations
                .required_lemmas
                .contains(&Primitive::StateMapConstruction)),
            Ok(true)
        );
        let operations = operations_result.unwrap();
        assert_eq!(operations.ledger.construction_visits, None);
        assert_eq!(operations.ledger.cleanup_visits, None);
        let requirement = &operations.services[0];
        assert_eq!(requirement.binding.service, ExternalService::HostEntropy);
        assert_eq!(requirement.binding.entry, ENTROPY_ENTRY);
        assert_eq!(requirement.binding.receiver, "std::hash::RandomState");
        assert_eq!(requirement.invocations, 2);
        assert_eq!(
            requirement.binding.retained,
            RetainedScope::ThreadAndProcess
        );
        assert_eq!(
            requirement.binding.temporary,
            TemporaryScope::CacheConstruction
        );
        assert_eq!(requirement.binding.required_facts, entropy.required_facts);
        assert!(
            requirement
                .binding
                .required_facts
                .contains(&ServiceFact::ThreadMultiplicity)
        );
        assert!(
            requirement
                .binding
                .required_facts
                .contains(&ServiceFact::ProcessRetainedState)
        );
        assert_entropy_refusals(&syntax, &primitive, &entropy);
    }

    fn assert_entropy_refusals(
        syntax: &syn::File,
        primitive: &PrimitiveSource,
        entropy: &ServiceBinding,
    ) {
        assert_eq!(
            classify_source_body(syntax, slice::from_ref(primitive), &[]),
            Err(Refusal::MissingServiceBinding(ExternalService::HostEntropy))
        );
        let mut changed_entry = entropy.clone();
        changed_entry.entry = "std::sys::random::hashmap_random_keys";
        assert_eq!(
            classify_source_body(syntax, slice::from_ref(primitive), &[changed_entry]),
            Err(Refusal::ServiceIdentity(ExternalService::HostEntropy))
        );
        let mut changed_thread = entropy.clone();
        changed_thread.retained = RetainedScope::Thread;
        assert_eq!(
            classify_source_body(syntax, slice::from_ref(primitive), &[changed_thread]),
            Err(Refusal::ServiceLifetime(ExternalService::HostEntropy))
        );
        let mut changed_caller = entropy.clone();
        changed_caller.retained = RetainedScope::Caller;
        assert_eq!(
            classify_source_body(syntax, slice::from_ref(primitive), &[changed_caller]),
            Err(Refusal::ServiceLifetime(ExternalService::HostEntropy))
        );
        let mut changed_temporary = entropy.clone();
        changed_temporary.temporary = TemporaryScope::OutsideCache;
        assert_eq!(
            classify_source_body(syntax, slice::from_ref(primitive), &[changed_temporary]),
            Err(Refusal::ServiceLifetime(ExternalService::HostEntropy))
        );
        for fact in [
            ServiceFact::Work,
            ServiceFact::TemporaryStorage,
            ServiceFact::RetainedState,
            ServiceFact::Stack,
            ServiceFact::ThreadMultiplicity,
            ServiceFact::ProcessRetainedState,
        ] {
            let mut changed = entropy.clone();
            assert!(changed.required_facts.remove(&fact));
            assert_eq!(
                classify_source_body(syntax, slice::from_ref(primitive), &[changed]),
                Err(Refusal::MissingServiceFact(
                    ExternalService::HostEntropy,
                    fact
                ))
            );
        }
    }
}

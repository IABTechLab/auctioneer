//! Task 3 source-recognizer RED contract, not a common-source work certificate.
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

use std::collections::{BTreeMap, BTreeSet};

// One service body covers warm/cold TLS and platform seeding. HashMap setup,
// ownership, hashing, probing, growth and drop remain common-source obligations.
const ENTROPY_ENTRY: &str = "std::hash::RandomState::new";

// Distinct source units, never an additive total, CPU units or native tokens.
// None denotes an unproved count; Some(0) needs a source-proven absence.
#[derive(Debug, Default, Eq, PartialEq)]
struct WorkLedger {
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Primitive {
    UuidFormat,
    StateMapConstruction,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetainedScope {
    Caller,
    Thread,
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
    ledger: WorkLedger,
    calls: BTreeMap<String, u64>,
    traversals: BTreeMap<String, TraversalMeasure>,
    required_lemmas: BTreeSet<Primitive>,
    services: Vec<ServiceRequirement>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    ClassifierNotImplemented,
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
}

// Only a named refusal until the actual source recognizer has hosted RED.
// No supplied primitive body or hash can make this a complete work proof.
pub(super) fn classify_source_body(
    _syntax: &syn::File,
    _primitives: &[PrimitiveSource],
    _services: &[ServiceBinding],
) -> Result<BodyOperations, Refusal> {
    Err(Refusal::ClassifierNotImplemented)
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
        match syntax.items.first_mut().expect("fixture entry present") {
            syn::Item::Fn(function) => function,
            _ => panic!("fixture entry must be a function"),
        }
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
                RetainedScope::Thread,
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
        assert!(classify_source_body(&original, &[], &[]).is_ok());
        let mut changed = original.clone();
        root(&mut changed).block = Box::new(syn::parse_quote!({ unknown(instance) }));
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
        let selected = classify_source_body(&bounded, &[], &[]);
        assert_eq!(
            selected
                .as_ref()
                .map(|operations| operations.traversals.get("is_valid::arr")),
            Ok(Some(&TraversalMeasure::ValuesAndKeys))
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
        root(&mut changed).block = Box::new(syn::parse_quote!({ is_valid(instance) }));
        assert_eq!(
            classify_source_body(&changed, &[], &[]),
            Err(Refusal::MissingDecreasingMeasure)
        );
    }

    #[test]
    fn changed_branch_multiplicity_changes_the_recipe() {
        let original = body();
        let before = classify_source_body(&original, &[], &[]);
        assert_eq!(
            before
                .as_ref()
                .map(|operations| operations.calls.get("is_branch_valid_0")),
            Ok(Some(&1))
        );
        let before = before.unwrap();
        let mut changed = original.clone();
        root(&mut changed).block = Box::new(syn::parse_quote!({
            is_branch_valid_0(instance) && is_branch_valid_0(instance)
        }));
        let after = classify_source_body(&changed, &[], &[]);
        assert_eq!(
            after
                .as_ref()
                .map(|operations| operations.calls.get("is_branch_valid_0")),
            Ok(Some(&2))
        );
        let after = after.unwrap();
        assert_ne!(before.calls, after.calls);
    }

    #[test]
    fn non_regex_declaration_has_a_nonempty_closure() {
        let operations = classify_source_body(&body(), &[], &[]);
        assert!(
            operations.is_ok(),
            "non-regex type closure must be classified"
        );
        let operations = operations.unwrap();
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
        let before = classify_source_body(&syntax, std::slice::from_ref(&primitive), &[]);
        assert_eq!(
            before
                .as_ref()
                .map(|operations| operations.required_lemmas.contains(&Primitive::UuidFormat)),
            Ok(true)
        );
        let before = before.unwrap();
        assert_eq!(before.ledger.scanned_bytes, None);
        let old_digest = primitive.source_sha256.clone();
        primitive.body.block = Box::new(syn::parse_quote!({
            if value.bytes().any(|byte| byte.is_ascii_lowercase()) {
                return false;
            }
            let mut decoded = [0_u8; 16];
            uuid_simd::parse_hyphenated(value.as_bytes(), decoded.as_out()).is_ok()
        }));
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
        let operations = classify_source_body(&syntax, &[], std::slice::from_ref(&clock));
        assert_eq!(
            operations.as_ref().map(|operations| operations
                .services
                .first()
                .map(|service| service.invocations)),
            Ok(Some(1))
        );
        let operations = operations.unwrap();
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
        assert!(classify_source_body(&known, &[], std::slice::from_ref(&clock)).is_ok());
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
        assert!(classify_source_body(&body(), &[], &[]).is_ok());
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
        let operations = classify_source_body(
            &syntax,
            std::slice::from_ref(&primitive),
            std::slice::from_ref(&entropy),
        );
        assert_eq!(
            operations.as_ref().map(|operations| operations
                .required_lemmas
                .contains(&Primitive::StateMapConstruction)),
            Ok(true)
        );
        let operations = operations.unwrap();
        assert_eq!(operations.ledger.construction_visits, None);
        assert_eq!(operations.ledger.cleanup_visits, None);
        let requirement = &operations.services[0];
        assert_eq!(requirement.binding.service, ExternalService::HostEntropy);
        assert_eq!(requirement.binding.entry, ENTROPY_ENTRY);
        assert_eq!(requirement.binding.receiver, "std::hash::RandomState");
        assert_eq!(requirement.invocations, 2);
        assert_eq!(requirement.binding.retained, RetainedScope::Thread);
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
        assert_eq!(
            classify_source_body(&syntax, std::slice::from_ref(&primitive), &[]),
            Err(Refusal::MissingServiceBinding(ExternalService::HostEntropy))
        );
        let mut changed = entropy.clone();
        changed.entry = "std::sys::random::hashmap_random_keys";
        assert_eq!(
            classify_source_body(&syntax, std::slice::from_ref(&primitive), &[changed]),
            Err(Refusal::ServiceIdentity(ExternalService::HostEntropy))
        );
        let mut changed = entropy.clone();
        changed.retained = RetainedScope::Caller;
        assert_eq!(
            classify_source_body(&syntax, std::slice::from_ref(&primitive), &[changed]),
            Err(Refusal::ServiceLifetime(ExternalService::HostEntropy))
        );
        let mut changed = entropy.clone();
        changed.temporary = TemporaryScope::OutsideCache;
        assert_eq!(
            classify_source_body(&syntax, std::slice::from_ref(&primitive), &[changed]),
            Err(Refusal::ServiceLifetime(ExternalService::HostEntropy))
        );
        for fact in entropy.required_facts.iter().copied() {
            let mut changed = entropy.clone();
            assert!(changed.required_facts.remove(&fact));
            assert_eq!(
                classify_source_body(&syntax, std::slice::from_ref(&primitive), &[changed]),
                Err(Refusal::MissingServiceFact(
                    ExternalService::HostEntropy,
                    fact
                ))
            );
        }
    }
}

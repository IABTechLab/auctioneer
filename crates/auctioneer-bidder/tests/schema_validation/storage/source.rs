//! Actual cache source-structure contract, not a storage total or cleanup proof.
//! Field programs and the enclosing wrapper retain the existing `MetaCache` and
//! `BundleHeap` cost owners. No engine heap model or serving authority lives here.

use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::ValidationView;
use crate::schema_validation::{
    handoff::{self, CheckedBodies},
    operations::{
        self, BodyOperations, Primitive,
        model::{Recipe, Site},
    },
};
use serde_json::Value;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct FieldInitializer {
    pub(super) field: String,
    pub(super) program: String,
    pub(super) site: Site,
    pub(super) completed_siblings: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CallbackOverlap {
    pub(super) site: Site,
    pub(super) helper: Primitive,
    pub(super) wrapper: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct OwnerSchedule {
    pub(super) wrapper: String,
    pub(super) factory_root: usize,
    pub(super) validation_root: usize,
    pub(super) initializers: Vec<FieldInitializer>,
    pub(super) callbacks: Vec<CallbackOverlap>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct OwnerAnalysis {
    pub(super) owner: String,
    pub(super) marker: String,
    pub(super) operations: BodyOperations,
    pub(super) schedule: OwnerSchedule,
}

// Projections of the already classified C/I auxiliary rows, not capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
struct NativeRoots {
    factory: Option<String>,
    validation: Option<String>,
    wrapper: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    NotDerived,
    Operations(operations::Refusal),
    InitializerOrder,
    Constructor,
    CacheEscape,
    CacheReset,
    CacheClone,
    CacheRebind,
    MissingRoot,
    CallbackOverlap,
}

fn native_roots(graph: &Value, owner: &str, marker: &str) -> NativeRoots {
    let auxiliary = graph.get("auxiliary");
    let method = |role: &str, name: &str| {
        let mut matching = auxiliary
            .and_then(|value| value.get("dispatch_edges"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|row| {
                row.get("from").and_then(Value::as_str) == Some("validate_native")
                    && row.get("to").and_then(Value::as_str) == Some(marker)
                    && row.get("role").and_then(Value::as_str) == Some(role)
            });
        let first = matching.next();
        (first.is_some() && matching.next().is_none()).then(|| format!("{marker}::{name}"))
    };
    let mut wrappers = auxiliary
        .and_then(|value| value.get("roots"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|row| {
            let details = row.get("details");
            row.get("kind").and_then(Value::as_str) == Some("inline-retention-row")
                && details
                    .and_then(|value| value.get("owner"))
                    .and_then(Value::as_str)
                    == Some(owner)
                && details
                    .and_then(|value| value.get("marker"))
                    .and_then(Value::as_str)
                    == Some(marker)
        });
    let first = wrappers.next();
    let wrapper = if wrappers.next().is_none() {
        first
            .and_then(|row| row.get("details"))
            .and_then(|details| details.get("cache"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    } else {
        None
    };
    NativeRoots {
        factory: method("validation-call-cache-factory", "new_search_cache"),
        validation: method("is-valid-with-call-cache", "is_valid"),
        wrapper,
    }
}

// Hosted RED must precede deriving these source facts. This stub does not
// discharge ordinary release, partial-construction cleanup work or helper costs.
fn derive_owner(_recipe: &Recipe, _roots: &NativeRoots) -> Result<OwnerSchedule, Refusal> {
    Err(Refusal::NotDerived)
}

pub(super) fn check_sources<'a>(
    seed: &'a CapturedSeed,
    view: ValidationView<'a>,
) -> Result<CheckedBodies<'a, Result<OwnerAnalysis, Refusal>>, handoff::Refusal> {
    handoff::checked_bodies(seed, view, |body| {
        let analysis = operations::classify_source_body(body.syntax, &[], &[])
            .map_err(Refusal::Operations)
            .and_then(|operations| {
                let roots = native_roots(view.graph(), body.owner, body.marker);
                let schedule = derive_owner(operations.recipe(), &roots)?;
                Ok(OwnerAnalysis {
                    owner: body.owner.to_owned(),
                    marker: body.marker.to_owned(),
                    operations,
                    schedule,
                })
            });
        Ok(analysis)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CallbackOverlap, FieldInitializer, NativeRoots, OwnerAnalysis, OwnerSchedule, Refusal,
        check_sources, derive_owner, native_roots,
    };
    use crate::schema_build::application::CapturedSeed;
    use crate::schema_residency::with_current_descriptor;
    use crate::schema_validation::{
        handoff,
        operations::{
            self, BodyOperations, Primitive,
            model::{self, Action, Atom, Binding, Exit, Kind, Op, Origin, Recipe, Site, Ty},
        },
    };
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn row_string<'a>(row: &'a Value, key: &str) -> &'a str {
        row.get(key)
            .and_then(Value::as_str)
            .expect("required descriptor string")
    }

    fn callbacks(recipe: &Recipe) -> Vec<(Site, Primitive)> {
        let mut result = Vec::new();
        for function in recipe.functions.values() {
            model::walk(&function.body, &mut |op| {
                if let Kind::Primitive { lemma, .. } = &op.kind
                    && matches!(lemma, Primitive::HostnameFormat | Primitive::UuidFormat)
                {
                    result.push((op.site.clone(), *lemma));
                }
                Ok(())
            })
            .expect("shared recipe traversal");
        }
        result
    }

    fn expected_initializers(
        recipe: &Recipe,
        factory_root: usize,
        declaration: &Value,
    ) -> Vec<FieldInitializer> {
        let mut factories = Vec::new();
        let public_factory = recipe.functions.get(&factory_root).expect("public factory");
        model::walk(&public_factory.body, &mut |op| {
            if let Kind::Call { target, .. } = &op.kind
                && recipe
                    .functions
                    .get(target)
                    .expect("classified call target")
                    .identity
                    .ends_with("::new_search_cache")
            {
                factories.push(*target);
            }
            Ok(())
        })
        .expect("public factory recipe");
        assert_eq!(factories.len(), 1);
        let factory_id = *factories.first().expect("single inner factory");
        let factory = recipe.functions.get(&factory_id).expect("inner factory");
        let Kind::Scope { steps, .. } = &factory.body.kind else {
            panic!("inner factory scope");
        };
        let Kind::Construct {
            fields: some_fields,
            ..
        } = &steps.last().unwrap().kind
        else {
            panic!("inner factory Some constructor");
        };
        assert_eq!(some_fields.len(), 1);
        let (tag, cache) = some_fields.first().expect("single Some field");
        assert_eq!(tag.as_str(), "Some");
        let Kind::Construct { fields, .. } = &cache.kind else {
            panic!("inner cache field constructor");
        };
        let expected: Vec<FieldInitializer> = fields
            .iter()
            .enumerate()
            .map(|(ordinal, (field, value))| {
                assert_eq!(
                    value.value.ty,
                    Ty::Cache("regex_automata::meta::Cache".into())
                );
                assert_eq!(value.site.function, factory_id);
                let Origin::Static(program) = &value.value.origin else {
                    panic!("initializer's complete static program identity");
                };
                FieldInitializer {
                    field: field.clone(),
                    program: program.clone(),
                    site: value.site.clone(),
                    completed_siblings: fields
                        .get(..ordinal)
                        .expect("preceding field prefix")
                        .iter()
                        .map(|row| row.0.clone())
                        .collect(),
                }
            })
            .collect();
        assert_initializer_programs(recipe, &factory.identity, declaration, &expected);
        expected
    }

    fn assert_initializer_programs(
        recipe: &Recipe,
        factory_identity: &str,
        declaration: &Value,
        initializers: &[FieldInitializer],
    ) {
        let programs: BTreeSet<_> = initializers.iter().map(|row| row.program.clone()).collect();
        assert_eq!(programs.len(), initializers.len());
        let module = factory_identity.rsplit_once("::").unwrap().0;
        let declared = declaration
            .get("programs")
            .and_then(Value::as_array)
            .expect("descriptor programs");
        let declared_programs: BTreeSet<_> = declared
            .iter()
            .map(|row| format!("{module}::{}", row_string(row, "symbol")))
            .collect();
        assert_eq!(declared_programs.len(), declared.len());
        assert_eq!(programs, declared_programs);
        let initialized: Vec<_> = recipe.initialization.values().flatten().cloned().collect();
        let initialized_programs: BTreeSet<_> = initialized.iter().cloned().collect();
        assert_eq!(initialized_programs.len(), initialized.len());
        assert_eq!(programs, initialized_programs);
    }

    fn assert_owner(analysis: &OwnerAnalysis, declaration: &Value, roots: &NativeRoots) {
        assert_eq!(analysis.owner, row_string(declaration, "owner"));
        assert_eq!(analysis.marker, row_string(declaration, "marker"));
        let recipe = analysis.operations.recipe();
        let schedule = &analysis.schedule;
        let wrapper = roots.wrapper.as_ref().expect("checked native wrapper");
        assert_eq!(&schedule.wrapper, wrapper);
        let root_id = |candidate: &Option<String>| {
            let name = candidate.as_deref().expect("checked native root");
            let mut matching = recipe
                .functions
                .iter()
                .filter(|(_, function)| function.identity == name);
            let (id, _) = matching.next().expect("exact recipe root");
            assert!(matching.next().is_none());
            assert!(recipe.roots.contains(id));
            *id
        };
        let factory_root = root_id(&roots.factory);
        let validation_root = root_id(&roots.validation);
        assert_eq!(schedule.factory_root, factory_root);
        assert_eq!(schedule.validation_root, validation_root);
        let initializers = expected_initializers(recipe, factory_root, declaration);
        assert_eq!(schedule.initializers.len(), initializers.len());
        for (actual, expected) in schedule.initializers.iter().zip(&initializers) {
            assert_eq!(actual.field, expected.field);
            assert_eq!(actual.program, expected.program);
            assert_eq!(actual.site, expected.site);
            assert_eq!(actual.completed_siblings, expected.completed_siblings);
        }
        let callbacks = callbacks(recipe)
            .into_iter()
            .map(|(site, helper)| CallbackOverlap {
                site,
                helper,
                wrapper: wrapper.clone(),
            })
            .collect::<Vec<_>>();
        assert_eq!(schedule.callbacks.len(), callbacks.len());
        for (actual, expected) in schedule.callbacks.iter().zip(&callbacks) {
            assert_eq!(actual.site, expected.site);
            assert_eq!(actual.helper, expected.helper);
        }
        for callback in &schedule.callbacks {
            assert_eq!(&callback.wrapper, wrapper);
        }
        assert_eq!(
            schedule,
            &OwnerSchedule {
                wrapper: wrapper.clone(),
                factory_root,
                validation_root,
                initializers,
                callbacks,
            }
        );
    }

    #[test]
    fn actual_cache_lifetimes_cover_discovered_owners() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let declarations = view.graph()["declarations"].as_array().unwrap();
            assert!(!declarations.is_empty());
            assert!(
                declarations
                    .iter()
                    .any(|row| !row["programs"].as_array().unwrap().is_empty())
            );
            assert!(
                declarations
                    .iter()
                    .any(|row| row["programs"].as_array().unwrap().is_empty())
            );
            for row in declarations {
                let roots = native_roots(
                    view.graph(),
                    row["owner"].as_str().unwrap(),
                    row["marker"].as_str().unwrap(),
                );
                assert!(roots.factory.is_some());
                assert!(roots.validation.is_some());
                assert!(roots.wrapper.is_some());
            }
            let check_result = check_sources(seed, view);
            assert!(
                check_result.is_ok(),
                "actual source must first pass the sealed handoff"
            );
            let checked = check_result.unwrap();
            assert_eq!(checked.recheck(view), Ok(()));
            assert_eq!(checked.analyses().len(), declarations.len());
            for result in checked.analyses() {
                if let Err(Refusal::Operations(reason)) = result {
                    panic!(
                        "actual body classification must precede lifetime derivation: {reason:?}"
                    );
                }
            }
            assert!(
                checked.analyses().iter().all(Result::is_ok),
                "actual cache source schedules must be derived"
            );
            let actual = checked
                .analyses()
                .iter()
                .map(|row| row.as_ref().unwrap().owner.as_str())
                .collect::<BTreeSet<_>>();
            let expected = declarations
                .iter()
                .map(|row| row["owner"].as_str().unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(actual, expected);
            for result in checked.analyses() {
                let analysis = result.as_ref().unwrap();
                let declaration = declarations
                    .iter()
                    .find(|row| row["owner"] == analysis.owner)
                    .unwrap();
                let roots = native_roots(view.graph(), &analysis.owner, &analysis.marker);
                assert_owner(analysis, declaration, &roots);
            }
            let mut changed = CapturedSeed {
                identity_sha256: seed.identity_sha256.clone(),
                manifest_bytes: seed.manifest_bytes.clone(),
                rust: seed.rust.clone(),
                build_root: seed.build_root.clone(),
                out_dir: seed.out_dir.clone(),
                snapshots: seed.snapshots.clone(),
            };
            changed.rust.push(b' ');
            assert_ne!(changed.rust, seed.rust);
            assert_eq!(
                check_sources(&changed, view).map(|_checked| ()),
                Err(handoff::Refusal::Binding)
            );
            assert_eq!(checked.recheck(view), Ok(()));
            Ok(())
        })
        .unwrap();
    }

    // Private emitter-shaped input only; it never enters the actual-source join.
    fn fixture(with_fields: bool) -> BodyOperations {
        let fields = if with_fields {
            quote::quote! {
                pattern_0_cache: __ra::meta::Cache,
                pattern_1_cache: __ra::meta::Cache,
            }
        } else {
            quote::quote! {}
        };
        let initializers = if with_fields {
            quote::quote! {
                pattern_0_cache: PATTERN_0_REGEX.as_ref()?.create_cache(),
                pattern_1_cache: PATTERN_1_REGEX.as_ref()?.create_cache(),
            }
        } else {
            quote::quote! {}
        };
        let programs = if with_fields {
            quote::quote! {
                static PATTERN_0_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| None);
                static PATTERN_1_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| None);
            }
        } else {
            quote::quote! {}
        };
        let syntax = syn::parse2(quote::quote! {
            struct FakeMarker;
            struct FakeMarkerSearchCache { __inner: __fakemarker_impl::__SearchCache }
            impl FakeMarker {
                fn is_valid(instance: &serde_json::Value, cache: &mut FakeMarkerSearchCache) -> bool {
                    { __fakemarker_impl::entry_is_valid(instance, &mut cache.__inner) }
                }
                fn new_search_cache() -> Option<FakeMarkerSearchCache> {
                    __fakemarker_impl::new_search_cache().map(|inner| FakeMarkerSearchCache { __inner: inner })
                }
                fn initialize_regexes() -> bool { __fakemarker_impl::initialize_regexes() }
            }
            mod __fakemarker_impl {
                use regex_automata as __ra;
                use std::sync::LazyLock as __Lazy;
                #programs
                struct __SearchCache { #fields }
                fn initialize_regexes() -> bool { true }
                fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache { #initializers })
                }
                fn entry_is_valid(instance: &serde_json::Value, cache: &mut __SearchCache) -> bool {
                    match instance {
                        serde_json::Value::String(value) => crate::schema::staging::formats::hostname(value),
                        _ => true,
                    }
                }
            }
        }).expect("private typed source input");
        let result = operations::classify_source_body(&syntax, &[], &[]);
        assert!(
            result.is_ok(),
            "private input must pass the shared classifier: {result:?}"
        );
        result.unwrap()
    }

    fn roots() -> NativeRoots {
        NativeRoots {
            factory: Some("FakeMarker::new_search_cache".into()),
            validation: Some("FakeMarker::is_valid".into()),
            wrapper: Some("FakeMarkerSearchCache".into()),
        }
    }

    fn fields_mut(recipe: &mut Recipe) -> &mut Vec<(String, Op)> {
        let factory = recipe
            .functions
            .values_mut()
            .find(|function| function.identity == "__fakemarker_impl::new_search_cache")
            .unwrap();
        let Kind::Scope { steps, .. } = &mut factory.body.kind else {
            panic!("private factory scope");
        };
        let Kind::Construct {
            fields: some_fields,
            ..
        } = &mut steps.last_mut().unwrap().kind
        else {
            panic!("private Some constructor");
        };
        assert_eq!(some_fields.len(), 1);
        let (tag, cache) = some_fields.first_mut().expect("single Some field");
        assert_eq!(tag.as_str(), "Some");
        let Kind::Construct { fields, .. } = &mut cache.kind else {
            panic!("private inner cache constructor");
        };
        fields
    }

    // A newly added fixture initializer gets fresh lexical anchors. Only the
    // reorder control deliberately keeps anchors inconsistent with field order.
    fn reanchor_initializer(op: &mut Op, slot: usize, ordinal: usize) {
        assert!(matches!(&op.value.origin, Origin::Static(_)));
        assert!(op.value.declaration.is_none());
        *op.site
            .lexical
            .get_mut(slot)
            .expect("initializer lexical slot") = ordinal;
        if let Kind::Primitive {
            arguments,
            source_calls,
            ..
        } = &mut op.kind
        {
            for child in arguments.iter_mut().chain(source_calls) {
                reanchor_initializer(child, slot, ordinal);
            }
        } else if let Kind::Exit(_, Some(value)) = &mut op.kind {
            reanchor_initializer(value, slot, ordinal);
        } else {
            assert!(
                matches!(&op.kind, Kind::Value(_)),
                "private cache initializer chain"
            );
        }
    }

    #[test]
    fn initializer_order_preserves_complete_sibling_prefixes() {
        let operations = fixture(true);
        let original = operations.recipe();
        let mut before = original.clone();
        let original_fields = fields_mut(&mut before);
        assert_eq!(
            original_fields
                .iter()
                .map(|row| row.0.as_str())
                .collect::<Vec<_>>(),
            ["pattern_0_cache", "pattern_1_cache"]
        );
        let anchors: Vec<Site> = original_fields
            .iter()
            .map(|row| row.1.site.clone())
            .collect();
        assert_ne!(anchors[0], anchors[1]);
        assert_eq!(
            derive_owner(original, &roots()).map(|schedule| schedule.initializers),
            Ok(vec![
                FieldInitializer {
                    field: "pattern_0_cache".into(),
                    program: "__fakemarker_impl::PATTERN_0_REGEX".into(),
                    site: anchors[0].clone(),
                    completed_siblings: vec![],
                },
                FieldInitializer {
                    field: "pattern_1_cache".into(),
                    program: "__fakemarker_impl::PATTERN_1_REGEX".into(),
                    site: anchors[1].clone(),
                    completed_siblings: vec!["pattern_0_cache".into()],
                },
            ])
        );
        let mut reordered = original.clone();
        fields_mut(&mut reordered).swap(0, 1);
        assert_eq!(fields_mut(&mut reordered)[0].1.site, anchors[1]);
        assert_eq!(
            derive_owner(&reordered, &roots()),
            Err(Refusal::InitializerOrder)
        );
        let mut extra = original.clone();
        let fields = fields_mut(&mut extra);
        let mut additional = fields[0].1.clone();
        let slot = additional.site.lexical.len().checked_sub(1).unwrap();
        reanchor_initializer(&mut additional, slot, fields.len());
        fields.push(("extra_cache".into(), additional));
        assert_eq!(fields.len(), 3);
        assert_eq!(derive_owner(&extra, &roots()), Err(Refusal::Constructor));
    }

    fn append_effect(recipe: &mut Recipe, effect: impl FnOnce(Op) -> Kind) {
        let (id, function) = recipe
            .functions
            .iter_mut()
            .find(|(_, function)| function.identity == "__fakemarker_impl::entry_is_valid")
            .unwrap();
        let parameter = function.parameters.get(1).unwrap().clone();
        assert!(parameter.borrowed);
        let cache_type = parameter.ty.clone();
        let Kind::Scope { steps, .. } = &mut function.body.kind else {
            panic!("private validation scope");
        };
        let site = Site {
            function: *id,
            lexical: vec![0, steps.len()],
        };
        let place = Op {
            site: Site {
                function: *id,
                lexical: vec![0, steps.len(), 0],
            },
            value: parameter,
            kind: Kind::Value(Atom::Place),
        };
        let kind = effect(place);
        let ty = if matches!(&kind, Kind::Primitive { callee, .. } if callee == "clone") {
            cache_type
        } else {
            Ty::Unit
        };
        steps.push(Op {
            site: site.clone(),
            value: Binding {
                ty,
                place: String::new(),
                borrowed: false,
                origin: Origin::Temporary(site),
                declaration: None,
            },
            kind,
        });
    }

    #[test]
    fn cache_escape_reset_clone_and_rebind_are_refused() {
        let operations = fixture(true);
        let original = operations.recipe();
        assert!(
            derive_owner(original, &roots()).is_ok(),
            "private positive schedule"
        );
        let mut escape = original.clone();
        append_effect(&mut escape, |place| {
            Kind::Exit(
                Exit::Return(Site {
                    function: place.site.function,
                    lexical: vec![0],
                }),
                Some(Box::new(place)),
            )
        });
        assert_eq!(derive_owner(&escape, &roots()), Err(Refusal::CacheEscape));
        let mut reset = original.clone();
        append_effect(&mut reset, |place| Kind::Primitive {
            lemma: Primitive::CacheConstruction,
            callee: "reset".into(),
            receiver: Some(place.value.clone()),
            arguments: vec![place],
            source_calls: Vec::new(),
        });
        assert_eq!(derive_owner(&reset, &roots()), Err(Refusal::CacheReset));
        let mut cloned = original.clone();
        append_effect(&mut cloned, |place| Kind::Primitive {
            lemma: Primitive::CacheConstruction,
            callee: "clone".into(),
            receiver: Some(place.value.clone()),
            arguments: vec![place],
            source_calls: Vec::new(),
        });
        assert_eq!(derive_owner(&cloned, &roots()), Err(Refusal::CacheClone));
        let mut rebind = original.clone();
        append_effect(&mut rebind, |place| {
            Kind::Evaluate(
                Action::Assign(place.value.clone()),
                vec![place.clone(), place],
            )
        });
        assert_eq!(derive_owner(&rebind, &roots()), Err(Refusal::CacheRebind));
    }

    #[test]
    fn zero_cache_owner_still_requires_wrapper_and_callback_obligations() {
        let operations = fixture(false);
        let original = operations.recipe();
        let mut before = original.clone();
        assert!(fields_mut(&mut before).is_empty());
        let callback_sites = callbacks(original);
        assert_eq!(callback_sites.len(), 1);
        assert_eq!(callback_sites[0].1, Primitive::HostnameFormat);
        let root_id = |name: &str| {
            let (id, _) = original
                .functions
                .iter()
                .find(|(_, function)| function.identity == name)
                .unwrap();
            assert!(original.roots.contains(id));
            *id
        };
        assert_eq!(
            derive_owner(original, &roots()),
            Ok(OwnerSchedule {
                wrapper: "FakeMarkerSearchCache".into(),
                factory_root: root_id("FakeMarker::new_search_cache"),
                validation_root: root_id("FakeMarker::is_valid"),
                initializers: vec![],
                callbacks: vec![CallbackOverlap {
                    site: callback_sites[0].0.clone(),
                    helper: Primitive::HostnameFormat,
                    wrapper: "FakeMarkerSearchCache".into(),
                }],
            })
        );
        let mut missing_factory = roots();
        assert!(missing_factory.factory.take().is_some());
        assert_eq!(
            derive_owner(original, &missing_factory),
            Err(Refusal::MissingRoot)
        );
        let mut missing_validation = roots();
        assert!(missing_validation.validation.take().is_some());
        assert_eq!(
            derive_owner(original, &missing_validation),
            Err(Refusal::MissingRoot)
        );
        let mut missing_overlap = roots();
        assert!(missing_overlap.wrapper.take().is_some());
        assert_eq!(
            derive_owner(original, &missing_overlap),
            Err(Refusal::CallbackOverlap)
        );
    }
}

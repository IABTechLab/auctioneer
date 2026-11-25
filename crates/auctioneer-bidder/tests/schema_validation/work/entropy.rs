//! Tests-first caller-side entropy contract. No actual-source or cost authority.
//!
//! `N_seed` counts the whole `std::hash::RandomState::new` entry, not OS calls.
//! Warm TLS does not erase an entry; cold acquisition/retries remain within the
//! existing whole-body `HostEntropy` service. Its fact/lifetime controls stay in
//! operations, including thread/process populations, never an `N_seed` multiple.
//! Map setup, initial-state insertion, hashing/probing/growth and cleanup remain
//! ordinary common obligations. No ledger unit is priced by this sidecar.
//!
//! All branch/pattern conditions, compiler/features, source snapshots, ordered
//! caller rows and invocation ceilings below are DATA. Actual use must join the
//! existing checked `Recipe` and storage owner schedule to the same captured
//! program certificates and bracket source interpretation with existing reads.
//! This file opens no new loader, classifier, engine or source-completion path.

use crate::schema_validation::operations::{
    Primitive, WorkLedger,
    model::{Recipe, Site},
};
use std::collections::BTreeSet;
use std::path::PathBuf;

// Source roles only, not additional WorkLedger units.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SourceUnit {
    MetaStrategy,
    MetaWrappers,
    HybridRegex,
    HybridDfa,
    HashMap,
    RandomState,
    HashExports,
    HashbrownMap,
    HashbrownRaw,
    Hostname,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Forward,
    Reverse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatternScope {
    SelectedAnchoredOrEndAnchored,
    ReverseInner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CacheBranch {
    PairedHybrid,
    ReverseInner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Condition {
    PatternScope,
    CacheBranch,
    HostnameGuard,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerData {
    executable: PathBuf,
    verbose: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceData {
    unit: SourceUnit,
    path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
    sha256: String,
}

// Supplied projection of a generated initializer, not a replacement storage
// schedule or a constructor of storage::source's private OwnerSchedule.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ConstructorData {
    site: Site,
    program: String,
    field: String,
    completed_siblings: Vec<String>,
    pattern: String,
    scope: Option<PatternScope>,
    branch: Option<CacheBranch>,
    directions: Vec<Direction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallbackData {
    site: Site,
    pattern: String,
    scope: Option<PatternScope>,
    branch: Option<CacheBranch>,
    guarded_creation: Option<bool>,
    directions: Vec<Direction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConstructionData {
    compiler: CompilerData,
    workspace: PathBuf,
    features: BTreeSet<String>,
    sources: Vec<SourceData>,
    constructors: Vec<ConstructorData>,
    hostname: CallbackData,
}

// Source-site invocation hypotheses. Zero still needs a present site record.
#[derive(Clone, Debug, Eq, PartialEq)]
struct InvocationData {
    constructors: Vec<(Site, u64)>,
    hostname: (Site, u64),
}

#[derive(Debug, Eq, PartialEq)]
struct SeedContract {
    entry: &'static str,
    constructor_sites: Vec<Site>,
    hostname_site: Site,
    programs: Vec<String>,
    // Potential ordered prefixes for one wrapper construction, not reachable-
    // path assertions or prices for partial cleanup: first 0/1/2, then 2/3/4.
    prefixes: Vec<[u64; 3]>,
    generated_cache_calls: u64,
    hostname_cache_calls: u64,
    seed_entries: u64,
    source_requirements: BTreeSet<SourceUnit>,
    ordinary_obligations: BTreeSet<Primitive>,
    ledger: WorkLedger,
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    MissingSourceProof,
    MissingSource(SourceUnit),
    DuplicateSource(SourceUnit),
    SourceChanged(SourceUnit),
    SourcePath(SourceUnit),
    SourceShape(SourceUnit),
    CompilerMismatch,
    FeatureMismatch,
    MissingCondition(Condition),
    UnsupportedBranch,
    Schedule,
    MissingDirection(Direction),
    DuplicateDirection(Direction),
    DirectionOrder,
    MissingInvocation,
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_build::schema_regex_vendor::FEATURES;
    use crate::schema_validation::operations::{
        self,
        model::{self, Kind, Origin},
    };
    use proc_macro2::TokenTree;
    use quote::{ToTokens as _, quote};
    use sha2::{Digest as _, Sha256};
    use std::path::Path;
    use std::str::from_utf8;

    fn source_requirements() -> BTreeSet<SourceUnit> {
        BTreeSet::from([
            SourceUnit::MetaStrategy,
            SourceUnit::MetaWrappers,
            SourceUnit::HybridRegex,
            SourceUnit::HybridDfa,
            SourceUnit::HashMap,
            SourceUnit::RandomState,
            SourceUnit::HashExports,
            SourceUnit::HashbrownMap,
            SourceUnit::HashbrownRaw,
            SourceUnit::Hostname,
        ])
    }

    fn ordinary_obligations() -> BTreeSet<Primitive> {
        BTreeSet::from([
            Primitive::CacheConstruction,
            Primitive::StateMapConstruction,
            Primitive::HostnameFormat,
            Primitive::IteratorProtocol,
            Primitive::Cleanup,
        ])
    }

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    // Existing bounded capture/reader owners, fixture DATA only. The cloned
    // expected compiler and FEATURES constant are not the actual manifest join.
    // No source is silently optional and no private file loader is introduced.
    fn capture() -> ConstructionData {
        let (executable, verbose) = crate::schema_build::selected::probe_compiler()
            .expect("existing bounded compiler capture");
        let library = executable
            .parent()
            .and_then(Path::parent)
            .expect("captured compiler sysroot")
            .join("lib/rustlib/src/rust/library");
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace crates parent")
            .to_owned();
        let mut sources = Vec::new();
        for (unit, relative, standard) in [
            (
                SourceUnit::MetaStrategy,
                "vendor/regex-automata/src/meta/strategy.rs",
                false,
            ),
            (
                SourceUnit::MetaWrappers,
                "vendor/regex-automata/src/meta/wrappers.rs",
                false,
            ),
            (
                SourceUnit::HybridRegex,
                "vendor/regex-automata/src/hybrid/regex.rs",
                false,
            ),
            (
                SourceUnit::HybridDfa,
                "vendor/regex-automata/src/hybrid/dfa.rs",
                false,
            ),
            (SourceUnit::HashMap, "std/src/collections/hash/map.rs", true),
            (SourceUnit::RandomState, "std/src/hash/random.rs", true),
            (SourceUnit::HashExports, "std/src/hash/mod.rs", true),
            (
                SourceUnit::HashbrownMap,
                "vendor/hashbrown-0.17.1/src/map.rs",
                true,
            ),
            (
                SourceUnit::HashbrownRaw,
                "vendor/hashbrown-0.17.1/src/raw.rs",
                true,
            ),
            (
                SourceUnit::Hostname,
                "crates/auctioneer-bidder/src/schema/staging.rs",
                false,
            ),
        ] {
            let path = if standard {
                library.join(relative)
            } else {
                workspace.join(relative)
            };
            let before = crate::schema_residency::read_regular(&path)
                .expect("existing regular bounded source read");
            let after = crate::schema_residency::read_regular(&path)
                .expect("existing regular bounded source reread");
            assert_eq!(before, after);
            sources.push(SourceData {
                unit,
                path,
                sha256: digest(&before),
                before,
                after,
            });
        }
        ConstructionData {
            compiler: CompilerData {
                executable,
                verbose,
            },
            workspace,
            features: FEATURES.into_iter().map(str::to_owned).collect(),
            sources,
            constructors: Vec::new(),
            hostname: CallbackData {
                site: Site {
                    function: 0,
                    lexical: Vec::new(),
                },
                pattern: crate::schema_formats::HOSTNAME_SOURCE.to_owned(),
                scope: Some(PatternScope::SelectedAnchoredOrEndAnchored),
                branch: Some(CacheBranch::PairedHybrid),
                guarded_creation: Some(true),
                directions: vec![Direction::Forward, Direction::Reverse],
            },
        }
    }

    // Ordinary classifier input, never selected source. No search of either
    // generated cache is present: constructing inactive siblings still seeds.
    fn fixture_recipe() -> Recipe {
        let syntax = syn::parse2(quote! {
            struct FakeEntropyMarker;
            struct FakeEntropyMarkerSearchCache { __inner: __fakeentropymarker_impl::__SearchCache }
            impl FakeEntropyMarker {
                fn is_valid(instance: &serde_json::Value, cache: &mut FakeEntropyMarkerSearchCache) -> bool {
                    { __fakeentropymarker_impl::entry_is_valid(instance, &mut cache.__inner) }
                }
                fn new_search_cache() -> Option<FakeEntropyMarkerSearchCache> {
                    __fakeentropymarker_impl::new_search_cache().map(|inner| FakeEntropyMarkerSearchCache { __inner: inner })
                }
                fn initialize_regexes() -> bool { __fakeentropymarker_impl::initialize_regexes() }
            }
            mod __fakeentropymarker_impl {
                use regex_automata as __ra;
                use std::sync::LazyLock as __Lazy;
                static PATTERN_0_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| None);
                static PATTERN_1_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| None);
                struct __SearchCache { pattern_0_cache: __ra::meta::Cache, pattern_1_cache: __ra::meta::Cache }
                fn initialize_regexes() -> bool { true }
                fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache {
                        pattern_0_cache: PATTERN_0_REGEX.as_ref()?.create_cache(),
                        pattern_1_cache: PATTERN_1_REGEX.as_ref()?.create_cache(),
                    })
                }
                fn entry_is_valid(instance: &serde_json::Value, cache: &mut __SearchCache) -> bool {
                    match instance {
                        serde_json::Value::Array(values) => values.iter().all(|value| match value {
                            serde_json::Value::String(text) => crate::schema::staging::formats::hostname(text),
                            _ => false,
                        }),
                        _ => false,
                    }
                }
            }
        }).expect("typed emitter-shaped fixture");
        operations::classify_source_body(&syntax, &[], &[])
            .expect("reuse the ordinary operation classifier")
            .recipe()
            .clone()
    }

    fn fixture() -> (Recipe, ConstructionData, CompilerData, InvocationData) {
        let recipe = fixture_recipe();
        let mut data = capture();
        let mut constructors = Vec::new();
        let mut callbacks = Vec::new();
        for function in recipe.functions.values() {
            model::walk(&function.body, &mut |op| {
                if let Kind::Primitive {
                    lemma: Primitive::CacheConstruction,
                    ..
                } = &op.kind
                {
                    let Origin::Static(program) = &op.value.origin else {
                        panic!("static fixture program");
                    };
                    constructors.push((op.site.clone(), program.clone()));
                }
                if matches!(
                    &op.kind,
                    Kind::Primitive {
                        lemma: Primitive::HostnameFormat,
                        ..
                    }
                ) {
                    callbacks.push(op.site.clone());
                }
                Ok(())
            })
            .expect("shared recipe walk");
        }
        assert_eq!(constructors.len(), 2);
        assert_eq!(callbacks.len(), 1);
        let fields = ["pattern_0_cache", "pattern_1_cache"];
        let patterns = ["^[0-9]+$", r"\.[a-zA-Z]{2,3}$"];
        for (ordinal, ((site, program), (field, pattern))) in constructors
            .into_iter()
            .zip(fields.into_iter().zip(patterns))
            .enumerate()
        {
            data.constructors.push(ConstructorData {
                site,
                program,
                field: field.to_owned(),
                completed_siblings: fields
                    .get(..ordinal)
                    .unwrap()
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect(),
                pattern: pattern.to_owned(),
                scope: Some(PatternScope::SelectedAnchoredOrEndAnchored),
                branch: Some(CacheBranch::PairedHybrid),
                directions: vec![Direction::Forward, Direction::Reverse],
            });
        }
        data.hostname.site = callbacks.pop().unwrap();
        assert_fixture_bindings(&recipe, &data);
        let expected = data.compiler.clone();
        let invocations = InvocationData {
            constructors: data
                .constructors
                .iter()
                .map(|row| (row.site.clone(), 1))
                .collect(),
            hostname: (data.hostname.site.clone(), 0),
        };
        (recipe, data, expected, invocations)
    }

    fn assert_fixture_bindings(recipe: &Recipe, data: &ConstructionData) {
        let programs = vec![
            "__fakeentropymarker_impl::PATTERN_0_REGEX".to_owned(),
            "__fakeentropymarker_impl::PATTERN_1_REGEX".to_owned(),
        ];
        assert_eq!(recipe.initialization.len(), 1);
        assert_eq!(recipe.initialization.values().next().unwrap(), &programs);
        assert_eq!(
            data.constructors
                .iter()
                .map(|row| row.program.clone())
                .collect::<Vec<_>>(),
            programs
        );
        assert_eq!(
            data.constructors
                .iter()
                .map(|row| row.field.as_str())
                .collect::<Vec<_>>(),
            ["pattern_0_cache", "pattern_1_cache"]
        );
        assert_eq!(
            data.constructors
                .iter()
                .map(|row| row.pattern.as_str())
                .collect::<Vec<_>>(),
            ["^[0-9]+$", r"\.[a-zA-Z]{2,3}$"]
        );
        assert_eq!(
            data.hostname.pattern,
            crate::schema_formats::HOSTNAME_SOURCE
        );
        assert_eq!(
            data.hostname.scope,
            Some(PatternScope::SelectedAnchoredOrEndAnchored)
        );
        assert_eq!(data.hostname.branch, Some(CacheBranch::PairedHybrid));
        assert!(data.compiler.executable.is_absolute());
    }

    fn assert_contract(
        recipe: &Recipe,
        data: &ConstructionData,
        compiler: &CompilerData,
        invocations: &InvocationData,
        expected_counts: [u64; 3],
    ) {
        let [generated_cache_calls, hostname_cache_calls, seed_entries] = expected_counts;
        let expected = SeedContract {
            entry: "std::hash::RandomState::new",
            constructor_sites: data
                .constructors
                .iter()
                .map(|row| row.site.clone())
                .collect(),
            hostname_site: data.hostname.site.clone(),
            programs: vec![
                "__fakeentropymarker_impl::PATTERN_0_REGEX".to_owned(),
                "__fakeentropymarker_impl::PATTERN_1_REGEX".to_owned(),
            ],
            prefixes: vec![[0, 1, 2], [2, 3, 4]],
            generated_cache_calls,
            hostname_cache_calls,
            seed_entries,
            source_requirements: source_requirements(),
            ordinary_obligations: ordinary_obligations(),
            ledger: WorkLedger::default(),
        };
        let view = |contract: SeedContract| {
            (
                contract.entry,
                contract.constructor_sites,
                contract.hostname_site,
                contract.programs,
                contract.prefixes,
                contract.generated_cache_calls,
                contract.hostname_cache_calls,
                contract.seed_entries,
                contract.source_requirements,
                contract.ordinary_obligations,
                contract.ledger,
            )
        };
        assert_eq!(
            source_contract(recipe, data, compiler, invocations).map(view),
            Ok(view(expected))
        );
    }

    fn edit_source(
        data: &mut ConstructionData,
        unit: SourceUnit,
        edit: impl FnOnce(&mut syn::File),
    ) {
        let source = data
            .sources
            .iter_mut()
            .find(|source| source.unit == unit)
            .unwrap();
        let mut syntax = syn::parse_file(from_utf8(&source.before).unwrap()).unwrap();
        let original = syntax.to_token_stream().to_string();
        edit(&mut syntax);
        let changed = syntax.to_token_stream().to_string();
        assert_ne!(changed, original, "actual source fixture must change");
        source.before = changed.into_bytes();
        source.after.clone_from(&source.before);
        source.sha256 = digest(&source.before);
    }

    fn changed_default(data: &ConstructionData, unit: SourceUnit) -> ConstructionData {
        let mut changed = data.clone();
        edit_source(&mut changed, unit, |syntax| {
            let owner: syn::Type = match unit {
                SourceUnit::HashMap => syn::parse_quote!(HashMap<K, V, RandomState>),
                SourceUnit::RandomState => syn::parse_quote!(RandomState),
                SourceUnit::MetaStrategy
                | SourceUnit::MetaWrappers
                | SourceUnit::HybridRegex
                | SourceUnit::HybridDfa
                | SourceUnit::HashExports
                | SourceUnit::HashbrownMap
                | SourceUnit::HashbrownRaw
                | SourceUnit::Hostname => panic!("default fixture unit"),
            };
            let mut matched = 0_usize;
            for item in &mut syntax.items {
                let syn::Item::Impl(implementation) = item else {
                    continue;
                };
                if implementation.self_ty.to_token_stream().to_string()
                    != owner.to_token_stream().to_string()
                {
                    continue;
                }
                for member in &mut implementation.items {
                    let syn::ImplItem::Fn(method) = member else {
                        continue;
                    };
                    if (unit == SourceUnit::HashMap && method.sig.ident == "new")
                        || (unit == SourceUnit::RandomState && method.sig.ident == "default")
                    {
                        let before: syn::Block = if unit == SourceUnit::HashMap {
                            syn::parse_quote!({ Default::default() })
                        } else {
                            syn::parse_quote!({ RandomState::new() })
                        };
                        assert_eq!(
                            method.block.to_token_stream().to_string(),
                            before.to_token_stream().to_string()
                        );
                        method.block = if unit == SourceUnit::HashMap {
                            syn::parse_quote!({
                                let extra_seed = RandomState::new();
                                Default::default()
                            })
                        } else {
                            syn::parse_quote!({
                                let extra_seed = RandomState::new();
                                RandomState::new()
                            })
                        };
                        matched = matched.checked_add(1).unwrap();
                    }
                }
            }
            assert_eq!(
                matched, 1,
                "one actual constructor/default body before mutation"
            );
        });
        changed
    }

    fn changed_map_default(data: &ConstructionData) -> ConstructionData {
        let mut changed = data.clone();
        edit_source(&mut changed, SourceUnit::HashMap, |syntax| {
            let mut matched = 0_usize;
            for item in &mut syntax.items {
                let syn::Item::Verbatim(tokens) = item else {
                    continue;
                };
                let header: Vec<TokenTree> = tokens.clone().into_iter().collect();
                if header
                    .iter()
                    .any(|token| matches!(token, TokenTree::Ident(name) if name == "Default"))
                    && header
                        .iter()
                        .any(|token| matches!(token, TokenTree::Ident(name) if name == "HashMap"))
                {
                    // Preserve the locked source's const impl and [const]
                    // bound. Only an extra S::default seed edge is introduced.
                    *item = syn::Item::Verbatim(quote!(
                        #[stable(feature = "rust1", since = "1.0.0")]
                        #[rustc_const_unstable(feature = "const_default", issue = "143894")]
                        const impl<K, V, S> Default for HashMap<K, V, S>
                        where
                            S: [const] Default,
                        {
                            #[inline]
                            fn default() -> HashMap<K, V, S> {
                                let extra_seed: S = Default::default();
                                HashMap::with_hasher(Default::default())
                            }
                        }
                    ));
                    matched = matched.checked_add(1).unwrap();
                }
            }
            assert_eq!(matched, 1, "one actual const HashMap Default impl");
        });
        changed
    }

    #[test]
    fn constructor_default_and_std_identity_are_required() {
        let (recipe, data, compiler, invocations) = fixture();
        assert_contract(&recipe, &data, &compiler, &invocations, [2, 0, 4]);
        assert_source_records(&recipe, &data, &compiler, &invocations);
        for unit in [SourceUnit::HashMap, SourceUnit::RandomState] {
            let changed = changed_default(&data, unit);
            assert_eq!(
                source_contract(&recipe, &changed, &compiler, &invocations),
                Err(Refusal::SourceShape(unit))
            );
        }
        let map_default = changed_map_default(&data);
        assert_eq!(
            source_contract(&recipe, &map_default, &compiler, &invocations),
            Err(Refusal::SourceShape(SourceUnit::HashMap))
        );
        for feature in ["std", "hybrid"] {
            let mut changed_features = data.clone();
            assert!(changed_features.features.remove(feature));
            assert_eq!(
                source_contract(&recipe, &changed_features, &compiler, &invocations),
                Err(Refusal::FeatureMismatch)
            );
        }
        let mut changed_compiler = compiler.clone();
        changed_compiler.verbose.push_str("\nunmatched compiler");
        assert_eq!(
            source_contract(&recipe, &data, &changed_compiler, &invocations),
            Err(Refusal::CompilerMismatch)
        );
        let mut changed_alias = data.clone();
        edit_source(&mut changed_alias, SourceUnit::HybridDfa, |syntax| {
            let mut matched = 0_usize;
            for item in &mut syntax.items {
                let syn::Item::Type(alias) = item else {
                    continue;
                };
                if alias.ident == "StateMap"
                    && alias.ty.to_token_stream().to_string()
                        == quote!(std::collections::HashMap<State, LazyStateID>).to_string()
                {
                    alias.ty = Box::new(
                        syn::parse_quote!(std::collections::HashMap<State, LazyStateID, ChangedHasher>),
                    );
                    matched = matched.checked_add(1).unwrap();
                }
            }
            assert_eq!(matched, 1);
        });
        assert_eq!(
            source_contract(&recipe, &changed_alias, &compiler, &invocations),
            Err(Refusal::SourceShape(SourceUnit::HybridDfa))
        );
    }

    fn assert_source_records(
        recipe: &Recipe,
        data: &ConstructionData,
        compiler: &CompilerData,
        invocations: &InvocationData,
    ) {
        for unit in source_requirements() {
            let record = data
                .sources
                .iter()
                .find(|source| source.unit == unit)
                .unwrap()
                .clone();
            assert_eq!(record.sha256, digest(&record.before));
            let mut missing = data.clone();
            missing.sources.retain(|source| source.unit != unit);
            assert_eq!(
                data.sources
                    .iter()
                    .filter(|source| source.unit == unit)
                    .count(),
                1
            );
            assert_eq!(
                source_contract(recipe, &missing, compiler, invocations),
                Err(Refusal::MissingSource(unit))
            );
            let mut duplicate = data.clone();
            duplicate.sources.push(record.clone());
            assert_eq!(
                source_contract(recipe, &duplicate, compiler, invocations),
                Err(Refusal::DuplicateSource(unit))
            );
            let mut unstable = data.clone();
            unstable
                .sources
                .iter_mut()
                .find(|source| source.unit == unit)
                .unwrap()
                .after
                .push(b' ');
            assert_eq!(
                source_contract(recipe, &unstable, compiler, invocations),
                Err(Refusal::SourceChanged(unit))
            );
            let mut wrong_path = data.clone();
            wrong_path
                .sources
                .iter_mut()
                .find(|source| source.unit == unit)
                .unwrap()
                .path = data.workspace.join("unmatched-source.rs");
            assert_ne!(
                record.path,
                wrong_path
                    .sources
                    .iter()
                    .find(|source| source.unit == unit)
                    .unwrap()
                    .path
            );
            assert_eq!(
                source_contract(recipe, &wrong_path, compiler, invocations),
                Err(Refusal::SourcePath(unit))
            );
        }
    }

    #[test]
    fn ordered_seed_prefixes_include_eager_inactive_siblings() {
        let (recipe, data, compiler, invocations) = fixture();
        assert_contract(&recipe, &data, &compiler, &invocations, [2, 0, 4]);
        assert_eq!(
            data.constructors[0].completed_siblings,
            Vec::<String>::new()
        );
        assert_eq!(
            data.constructors[1].completed_siblings,
            vec!["pattern_0_cache"]
        );
        for direction in [Direction::Forward, Direction::Reverse] {
            let mut missing = data.clone();
            assert!(missing.constructors[0].directions.contains(&direction));
            missing.constructors[0]
                .directions
                .retain(|value| *value != direction);
            assert_eq!(
                source_contract(&recipe, &missing, &compiler, &invocations),
                Err(Refusal::MissingDirection(direction))
            );
        }
        let mut duplicate = data.clone();
        duplicate.constructors[0].directions = vec![Direction::Forward, Direction::Forward];
        assert_eq!(
            source_contract(&recipe, &duplicate, &compiler, &invocations),
            Err(Refusal::DuplicateDirection(Direction::Forward))
        );
        let mut reversed = data.clone();
        reversed.constructors[0].directions.reverse();
        assert_eq!(
            source_contract(&recipe, &reversed, &compiler, &invocations),
            Err(Refusal::DirectionOrder)
        );
        let mut wrong_prefix = data.clone();
        wrong_prefix.constructors[1].completed_siblings.clear();
        assert_eq!(
            source_contract(&recipe, &wrong_prefix, &compiler, &invocations),
            Err(Refusal::Schedule)
        );
        assert_constructor_conditions(&recipe, &data, &compiler, &invocations);
        let mut changed_receiver = data.clone();
        edit_source(&mut changed_receiver, SourceUnit::HybridRegex, |syntax| {
            let mut matched = 0_usize;
            for item in &mut syntax.items {
                let syn::Item::Impl(implementation) = item else {
                    continue;
                };
                if implementation.self_ty.to_token_stream().to_string() != "Cache" {
                    continue;
                }
                for member in &mut implementation.items {
                    let syn::ImplItem::Fn(method) = member else {
                        continue;
                    };
                    if method.sig.ident == "new" {
                        method.block = syn::parse_quote!({
                            let forward = dfa::Cache::new(re.forward());
                            let reverse = dfa::Cache::new(re.forward());
                            Cache { forward, reverse }
                        });
                        matched = matched.checked_add(1).unwrap();
                    }
                }
            }
            assert_eq!(matched, 1);
        });
        assert_eq!(
            source_contract(&recipe, &changed_receiver, &compiler, &invocations),
            Err(Refusal::SourceShape(SourceUnit::HybridRegex))
        );
    }

    fn assert_constructor_conditions(
        recipe: &Recipe,
        data: &ConstructionData,
        compiler: &CompilerData,
        invocations: &InvocationData,
    ) {
        let mut missing_scope = data.clone();
        assert_eq!(
            missing_scope.constructors[0].scope.take(),
            Some(PatternScope::SelectedAnchoredOrEndAnchored)
        );
        assert_eq!(
            source_contract(recipe, &missing_scope, compiler, invocations),
            Err(Refusal::MissingCondition(Condition::PatternScope))
        );
        let mut missing_branch = data.clone();
        assert_eq!(
            missing_branch.constructors[0].branch.take(),
            Some(CacheBranch::PairedHybrid)
        );
        assert_eq!(
            source_contract(recipe, &missing_branch, compiler, invocations),
            Err(Refusal::MissingCondition(Condition::CacheBranch))
        );
        let mut unsupported_scope = data.clone();
        unsupported_scope.constructors[0].scope = Some(PatternScope::ReverseInner);
        assert_eq!(
            source_contract(recipe, &unsupported_scope, compiler, invocations),
            Err(Refusal::UnsupportedBranch)
        );
        let mut unsupported_branch = data.clone();
        unsupported_branch.constructors[0].branch = Some(CacheBranch::ReverseInner);
        assert_eq!(
            source_contract(recipe, &unsupported_branch, compiler, invocations),
            Err(Refusal::UnsupportedBranch)
        );
    }

    #[test]
    fn hostname_seed_work_accumulates_and_overflow_refuses() {
        let (recipe, data, compiler, mut invocations) = fixture();
        invocations.hostname.1 = 3;
        assert_contract(&recipe, &data, &compiler, &invocations, [2, 3, 10]);
        let mut one_hostname = invocations.clone();
        one_hostname.hostname.1 = 1;
        assert_contract(&recipe, &data, &compiler, &one_hostname, [2, 1, 6]);
        let mut repeated_factory = invocations.clone();
        for (_, count) in &mut repeated_factory.constructors {
            *count = 2;
        }
        assert_contract(&recipe, &data, &compiler, &repeated_factory, [4, 3, 14]);
        let mut missing_guard = data.clone();
        assert_eq!(missing_guard.hostname.guarded_creation.take(), Some(true));
        assert_eq!(
            source_contract(&recipe, &missing_guard, &compiler, &invocations),
            Err(Refusal::MissingCondition(Condition::HostnameGuard))
        );
        let mut missing_direction = data.clone();
        assert_eq!(
            missing_direction.hostname.directions.pop(),
            Some(Direction::Reverse)
        );
        assert_eq!(
            source_contract(&recipe, &missing_direction, &compiler, &invocations),
            Err(Refusal::MissingDirection(Direction::Reverse))
        );
        let mut missing_site = invocations.clone();
        assert!(missing_site.constructors.pop().is_some());
        assert_eq!(
            source_contract(&recipe, &data, &compiler, &missing_site),
            Err(Refusal::MissingInvocation)
        );
        let mut zero = invocations.clone();
        for (_, count) in &mut zero.constructors {
            *count = 0;
        }
        zero.hostname.1 = 0;
        assert_contract(&recipe, &data, &compiler, &zero, [0, 0, 0]);
        let mut zero_missing_source = data.clone();
        zero_missing_source
            .sources
            .retain(|source| source.unit != SourceUnit::RandomState);
        assert_eq!(
            source_contract(&recipe, &zero_missing_source, &compiler, &zero),
            Err(Refusal::MissingSource(SourceUnit::RandomState))
        );
        let mut constructor_overflow = zero.clone();
        constructor_overflow.constructors[0].1 = u64::MAX;
        assert_eq!(
            source_contract(&recipe, &data, &compiler, &constructor_overflow),
            Err(Refusal::Overflow)
        );
        let mut hostname_overflow = zero.clone();
        hostname_overflow.hostname.1 = u64::MAX;
        assert_eq!(
            source_contract(&recipe, &data, &compiler, &hostname_overflow),
            Err(Refusal::Overflow)
        );
        let mut sum_overflow = zero;
        sum_overflow.constructors[0].1 = u64::MAX.checked_div(2).unwrap();
        sum_overflow.hostname.1 = 1;
        assert_eq!(
            source_contract(&recipe, &data, &compiler, &sum_overflow),
            Err(Refusal::Overflow)
        );
    }
}

// Refusal-only until all three case names execute genuine hosted assertion RED.
// No source recognizer or multiplicity arithmetic is implemented in this file.
// Future matching must independently pin the constructor/default/std import
// chain, receivers, ordering and supported branch conditions. Fixture hashes,
// labels, supplied Recipe values and expected totals cannot supply that proof.
fn source_contract(
    _recipe: &Recipe,
    _data: &ConstructionData,
    _expected_compiler: &CompilerData,
    _invocations: &InvocationData,
) -> Result<SeedContract, Refusal> {
    Err(Refusal::MissingSourceProof)
}

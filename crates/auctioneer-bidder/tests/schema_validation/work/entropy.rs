//! Tests-first caller-side entropy contract. No actual-source or cost authority.
//!
//! `N_seed` counts the whole `std::hash::RandomState::new` entry, not OS calls.
//! Warm TLS does not erase an entry; cold acquisition/retries remain within the
//! existing whole-body `HostEntropy` service. Its fact/lifetime controls stay in
//! operations, including thread/process populations, never an `N_seed` multiple.
//! Map setup, initial-state insertion, hashing/probing/growth and cleanup remain
//! ordinary common obligations. No ledger unit is priced by this sidecar.
//! Entry counts cover matched constructor edges. Unclosed ordinary bodies may
//! contribute additional entries as well as work; they are not assigned zero.
//! Written receiver types do not resolve aliases, inherited macros or builtin
//! names. Those bindings still require actual-source witnesses.
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
        let changed_strategy = changed_strategy_receiver(
            &data,
            "Core",
            "hybrid",
            &syn::parse_quote!(wrappers::Hybrid),
            &syn::parse_quote!(impl Strategy for Core {
                #[cfg_attr(feature = "perf-inline", inline(always))]
                fn create_cache(&self) -> Cache {
                    Cache {
                        capmatches: Captures::all(self.group_info().clone()),
                        pikevm: self.pikevm.create_cache(),
                        backtrack: self.backtrack.create_cache(),
                        onepass: self.onepass.create_cache(),
                        hybrid: self.hybrid.create_cache(),
                        revhybrid: wrappers::ReverseHybridCache::none(),
                    }
                }
            }),
        );
        assert_eq!(
            source_contract(&recipe, &changed_strategy, &compiler, &invocations),
            Err(Refusal::SourceShape(SourceUnit::MetaStrategy))
        );
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
        let changed_strategy = changed_strategy_receiver(
            &data,
            "ReverseAnchored",
            "core",
            &syn::parse_quote!(Core),
            &syn::parse_quote!(impl Strategy for ReverseAnchored {
                #[cfg_attr(feature = "perf-inline", inline(always))]
                fn create_cache(&self) -> Cache { self.core.create_cache() }
            }),
        );
        assert_eq!(
            source_contract(&recipe, &changed_strategy, &compiler, &invocations),
            Err(Refusal::SourceShape(SourceUnit::MetaStrategy))
        );
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
        let changed_strategy = changed_strategy_receiver(
            &data,
            "ReverseSuffix",
            "core",
            &syn::parse_quote!(Core),
            &syn::parse_quote!(impl Strategy for ReverseSuffix {
                #[cfg_attr(feature = "perf-inline", inline(always))]
                fn create_cache(&self) -> Cache { self.core.create_cache() }
            }),
        );
        assert_eq!(
            source_contract(&recipe, &changed_strategy, &compiler, &invocations),
            Err(Refusal::SourceShape(SourceUnit::MetaStrategy))
        );
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

    fn assert_strategy_constructor(syntax: &syn::File, expected: &syn::ItemImpl) {
        let [syn::ImplItem::Fn(wanted)] = expected.items.as_slice() else {
            panic!("one independently authored strategy constructor");
        };
        let mut expected_header = expected.clone();
        expected_header.items.clear();
        let mut matched = 0_usize;
        for item in &syntax.items {
            let syn::Item::Impl(implementation) = item else {
                continue;
            };
            if implementation.self_ty.to_token_stream().to_string()
                != expected.self_ty.to_token_stream().to_string()
            {
                continue;
            }
            for member in &implementation.items {
                let syn::ImplItem::Fn(method) = member else {
                    continue;
                };
                if method.sig.ident != wanted.sig.ident {
                    continue;
                }
                let mut header = implementation.clone();
                header.items.clear();
                assert!(
                    header.to_token_stream().to_string()
                        == expected_header.to_token_stream().to_string(),
                    "strategy constructor header remains the independent recipe"
                );
                assert!(
                    method.to_token_stream().to_string() == wanted.to_token_stream().to_string(),
                    "strategy constructor signature, attributes and body remain unchanged"
                );
                matched = matched.checked_add(1).unwrap();
            }
        }
        assert_eq!(matched, 1, "one actual strategy constructor");
    }

    fn changed_strategy_receiver(
        data: &ConstructionData,
        owner: &str,
        field: &str,
        expected_type: &syn::Type,
        expected_method: &syn::ItemImpl,
    ) -> ConstructionData {
        let mut changed = data.clone();
        edit_source(&mut changed, SourceUnit::MetaStrategy, |syntax| {
            assert_strategy_constructor(syntax, expected_method);
            let mut matched = 0_usize;
            for item in &mut syntax.items {
                let syn::Item::Struct(declaration) = item else {
                    continue;
                };
                if declaration.ident != owner {
                    continue;
                }
                for member in &mut declaration.fields {
                    if !member.ident.as_ref().is_some_and(|ident| ident == field) {
                        continue;
                    }
                    assert!(
                        member.ty.to_token_stream().to_string()
                            == expected_type.to_token_stream().to_string(),
                        "receiver type before-state is present"
                    );
                    member.ty = syn::parse_quote!(ChangedReceiver);
                    matched = matched.checked_add(1).unwrap();
                }
            }
            assert_eq!(matched, 1, "one receiver type before mutation");
            assert_strategy_constructor(syntax, expected_method);
        });
        changed
    }
}

// This remains a conditional DATA result. The actual-source owner must join the
// same Recipe, graph, program conditions and ten source roles inside its source
// window. Inherited names/macros/prelude and all ordinary helper costs remain
// obligations; this function cannot establish CommonComplete.
fn source_contract(
    recipe: &Recipe,
    data: &ConstructionData,
    expected_compiler: &CompilerData,
    invocations: &InvocationData,
) -> Result<SeedContract, Refusal> {
    let counts = source::check(data, expected_compiler)?;
    for row in &data.constructors {
        conditions(row.scope, row.branch)?;
        directions(&row.directions)?;
        if row.pattern.is_empty() {
            return Err(Refusal::MissingCondition(Condition::PatternScope));
        }
    }
    conditions(data.hostname.scope, data.hostname.branch)?;
    match data.hostname.guarded_creation {
        Some(true) => {}
        Some(false) => return Err(Refusal::UnsupportedBranch),
        None => return Err(Refusal::MissingCondition(Condition::HostnameGuard)),
    }
    directions(&data.hostname.directions)?;
    let schedule = owner_schedule(recipe)?;
    check_schedule(&schedule, data)?;
    let prefixes = prefixes(&schedule, &counts)?;
    let (generated_cache_calls, hostname_cache_calls) = invocation_counts(data, invocations)?;
    let per_cache = counts
        .forward
        .checked_add(counts.reverse)
        .ok_or(Refusal::Overflow)?;
    let generated_entries = generated_cache_calls
        .checked_mul(per_cache)
        .ok_or(Refusal::Overflow)?;
    let hostname_entries = hostname_cache_calls
        .checked_mul(counts.hostname_caches)
        .and_then(|calls| calls.checked_mul(per_cache))
        .ok_or(Refusal::Overflow)?;
    let seed_entries = generated_entries
        .checked_add(hostname_entries)
        .ok_or(Refusal::Overflow)?;
    Ok(SeedContract {
        entry: "std::hash::RandomState::new",
        constructor_sites: data
            .constructors
            .iter()
            .map(|row| row.site.clone())
            .collect(),
        hostname_site: data.hostname.site.clone(),
        programs: schedule
            .initializers()
            .map(|(_, program, _, _)| program.to_owned())
            .collect(),
        prefixes,
        generated_cache_calls,
        hostname_cache_calls,
        seed_entries,
        source_requirements: source::requirements(),
        ordinary_obligations: BTreeSet::from([
            Primitive::CacheConstruction,
            Primitive::StateMapConstruction,
            Primitive::HostnameFormat,
            Primitive::IteratorProtocol,
            Primitive::Cleanup,
        ]),
        ledger: WorkLedger::default(),
    })
}

fn conditions(scope: Option<PatternScope>, branch: Option<CacheBranch>) -> Result<(), Refusal> {
    match scope {
        Some(PatternScope::SelectedAnchoredOrEndAnchored) => {}
        Some(PatternScope::ReverseInner) => return Err(Refusal::UnsupportedBranch),
        None => return Err(Refusal::MissingCondition(Condition::PatternScope)),
    }
    match branch {
        Some(CacheBranch::PairedHybrid) => Ok(()),
        Some(CacheBranch::ReverseInner) => Err(Refusal::UnsupportedBranch),
        None => Err(Refusal::MissingCondition(Condition::CacheBranch)),
    }
}

fn directions(found: &[Direction]) -> Result<(), Refusal> {
    for direction in [Direction::Forward, Direction::Reverse] {
        if found.iter().filter(|value| **value == direction).count() > 1 {
            return Err(Refusal::DuplicateDirection(direction));
        }
    }
    for direction in [Direction::Forward, Direction::Reverse] {
        if !found.contains(&direction) {
            return Err(Refusal::MissingDirection(direction));
        }
    }
    if found != [Direction::Forward, Direction::Reverse] {
        return Err(Refusal::DirectionOrder);
    }
    Ok(())
}

fn owner_schedule(
    recipe: &Recipe,
) -> Result<crate::schema_validation::storage::OwnerSchedule, Refusal> {
    use crate::schema_validation::operations::model::Ty;
    let mut factories = recipe
        .roots
        .iter()
        .filter_map(|id| recipe.functions.get(id))
        .filter(|function| function.identity.ends_with("::new_search_cache"));
    let factory = factories.next().ok_or(Refusal::MissingSourceProof)?;
    if factories.next().is_some() {
        return Err(Refusal::Schedule);
    }
    let (marker, _) = factory
        .identity
        .rsplit_once("::")
        .ok_or(Refusal::Schedule)?;
    let Ty::Option(result) = &factory.result else {
        return Err(Refusal::Schedule);
    };
    let Ty::Cache(wrapper) = result.as_ref() else {
        return Err(Refusal::Schedule);
    };
    // Synthetic selectors are DATA, never a descriptor receipt. The shared
    // owner must still recognize the complete factory/validation/cache links.
    let graph = serde_json::json!({
        "auxiliary": {
            "dispatch_edges": [
                {"from": "validate_native", "to": marker, "role": "validation-call-cache-factory"},
                {"from": "validate_native", "to": marker, "role": "is-valid-with-call-cache"}
            ],
            "roots": [{"kind": "inline-retention-row", "details": {
                "owner": marker, "marker": marker, "cache": wrapper
            }}]
        }
    });
    crate::schema_validation::storage::owner_schedule_data(recipe, &graph, marker, marker)
        .map_err(|_| Refusal::Schedule)
}

fn check_schedule(
    schedule: &crate::schema_validation::storage::OwnerSchedule,
    data: &ConstructionData,
) -> Result<(), Refusal> {
    let rows: Vec<_> = schedule.initializers().collect();
    if rows.len() != data.constructors.len() {
        return Err(Refusal::Schedule);
    }
    for ((field, program, site, completed), row) in rows.into_iter().zip(&data.constructors) {
        if field != row.field
            || program != row.program
            || site != &row.site
            || completed != row.completed_siblings
        {
            return Err(Refusal::Schedule);
        }
    }
    let callbacks: Vec<_> = schedule
        .callbacks()
        .filter(|(_, helper, _)| *helper == Primitive::HostnameFormat)
        .collect();
    let [(site, _, _)] = callbacks.as_slice() else {
        return Err(Refusal::Schedule);
    };
    if *site != &data.hostname.site {
        return Err(Refusal::Schedule);
    }
    Ok(())
}

fn prefixes(
    schedule: &crate::schema_validation::storage::OwnerSchedule,
    counts: &source::Counts,
) -> Result<Vec<[u64; 3]>, Refusal> {
    let mut completed = 0_u64;
    let mut result = Vec::new();
    for _ in schedule.initializers() {
        let forward = completed
            .checked_add(counts.forward)
            .ok_or(Refusal::Overflow)?;
        let reverse = forward
            .checked_add(counts.reverse)
            .ok_or(Refusal::Overflow)?;
        result.push([completed, forward, reverse]);
        completed = reverse;
    }
    Ok(result)
}

fn invocation_counts(
    data: &ConstructionData,
    calls: &InvocationData,
) -> Result<(u64, u64), Refusal> {
    if calls.constructors.len() != data.constructors.len() || calls.hostname.0 != data.hostname.site
    {
        return Err(Refusal::MissingInvocation);
    }
    let mut generated = 0_u64;
    for row in &data.constructors {
        let mut matching = calls
            .constructors
            .iter()
            .filter(|(site, _)| *site == row.site);
        let (_, count) = matching.next().ok_or(Refusal::MissingInvocation)?;
        if matching.next().is_some() {
            return Err(Refusal::MissingInvocation);
        }
        generated = generated.checked_add(*count).ok_or(Refusal::Overflow)?;
    }
    Ok((generated, calls.hostname.1))
}

mod source {
    use super::{CompilerData, ConstructionData, Refusal, SourceUnit};
    use crate::schema_validation::operations::source_syntax::ConstTraitImpl;
    use proc_macro2::TokenStream;
    use quote::{ToTokens, quote};
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::mem;
    use std::path::Path;
    use std::str::from_utf8;
    use syn::visit_mut::{self, VisitMut};

    const PATHS: [(SourceUnit, &str, bool); 10] = [
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
    ];

    pub(super) struct Counts {
        pub(super) forward: u64,
        pub(super) reverse: u64,
        pub(super) hostname_caches: u64,
    }

    struct Sources(BTreeMap<SourceUnit, syn::File>);

    pub(super) fn requirements() -> BTreeSet<SourceUnit> {
        PATHS.into_iter().map(|(unit, _, _)| unit).collect()
    }

    fn spelling(tokens: &impl ToTokens) -> String {
        tokens.to_token_stream().to_string()
    }

    fn strip_docs(attrs: &mut Vec<syn::Attribute>) {
        attrs.retain(|attribute| {
            !matches!(&attribute.meta, syn::Meta::NameValue(meta)
                if attribute.path().is_ident("doc")
                && matches!(attribute.style, syn::AttrStyle::Outer)
                && matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_))))
        });
    }

    fn capture(data: &ConstructionData, expected: &CompilerData) -> Result<Sources, Refusal> {
        if data.compiler != *expected
            || !data.workspace.is_absolute()
            || !expected.executable.is_absolute()
            || expected
                .executable
                .file_name()
                .is_none_or(|name| name != "rustc")
        {
            return Err(Refusal::CompilerMismatch);
        }
        // Expected compiler/features must come from the same selected manifest
        // at actual use. Fixture equality is not that authentic binding.
        let features: BTreeSet<String> = crate::schema_build::schema_regex_vendor::FEATURES
            .into_iter()
            .map(str::to_owned)
            .collect();
        if data.features != features {
            return Err(Refusal::FeatureMismatch);
        }
        let library = expected
            .executable
            .parent()
            .and_then(Path::parent)
            .ok_or(Refusal::CompilerMismatch)?
            .join("lib/rustlib/src/rust/library");
        let mut files = BTreeMap::new();
        for (unit, relative, standard) in PATHS {
            let mut records = data.sources.iter().filter(|record| record.unit == unit);
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
            let file =
                syn::parse_file(from_utf8(&record.before).map_err(|_| Refusal::SourceShape(unit))?)
                    .map_err(|_| Refusal::SourceShape(unit))?;
            let attrs: Vec<_> = file.attrs.iter().filter(|attribute| {
                !matches!(&attribute.meta, syn::Meta::NameValue(meta)
                    if attribute.path().is_ident("doc")
                    && matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_))))
            }).collect();
            let expected_attrs = if unit == SourceUnit::HashExports {
                quote!(#![stable(feature = "rust1", since = "1.0.0")])
            } else {
                TokenStream::new()
            };
            if file.shebang.is_some()
                || file.frontmatter.is_some()
                || spelling(&quote!(#(#attrs)*)) != spelling(&expected_attrs)
            {
                return Err(Refusal::SourceShape(unit));
            }
            files.insert(unit, file);
        }
        Ok(Sources(files))
    }

    impl Sources {
        fn items(&self, unit: SourceUnit) -> Result<&[syn::Item], Refusal> {
            self.0
                .get(&unit)
                .map(|file| file.items.as_slice())
                .ok_or(Refusal::MissingSource(unit))
        }

        fn implementation(
            &self,
            unit: SourceUnit,
            wanted: TokenStream,
        ) -> Result<Vec<syn::ImplItemFn>, Refusal> {
            let mut expected: syn::ItemImpl =
                syn::parse2(wanted).map_err(|_| Refusal::SourceShape(unit))?;
            let members = mem::take(&mut expected.items);
            strip_docs(&mut expected.attrs);
            let mut result = Vec::new();
            for member in members {
                let syn::ImplItem::Fn(mut method) = member else {
                    return Err(Refusal::SourceShape(unit));
                };
                strip_docs(&mut method.attrs);
                let mut matching = Vec::new();
                for item in self.items(unit)? {
                    let syn::Item::Impl(found) = item else {
                        continue;
                    };
                    if spelling(&found.self_ty) != spelling(&expected.self_ty) {
                        continue;
                    }
                    for found_member in &found.items {
                        if let syn::ImplItem::Fn(found_method) = found_member
                            && found_method.sig.ident == method.sig.ident
                        {
                            matching.push((found, found_method));
                        }
                    }
                }
                let [(found, found_method)] = matching.as_slice() else {
                    return Err(Refusal::SourceShape(unit));
                };
                let mut header: syn::ItemImpl = (**found).clone();
                header.items.clear();
                strip_docs(&mut header.attrs);
                let mut actual: syn::ImplItemFn = (**found_method).clone();
                strip_docs(&mut actual.attrs);
                if spelling(&header) != spelling(&expected)
                    || spelling(&actual) != spelling(&method)
                {
                    return Err(Refusal::SourceShape(unit));
                }
                result.push(actual);
            }
            Ok(result)
        }

        fn constant_default(&self) -> Result<syn::ImplItemFn, Refusal> {
            let unit = SourceUnit::HashMap;
            let mut expected = syn::parse2::<ConstTraitImpl>(quote!(
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_unstable(feature = "const_default", issue = "143894")]
                const impl<K, V, S> Default for HashMap<K, V, S>
                where
                    S: [const] Default,
                {
                    #[inline]
                    fn default() -> HashMap<K, V, S> {
                        HashMap::with_hasher(Default::default())
                    }
                }
            ))
            .map_err(|_| Refusal::SourceShape(unit))?;
            strip_docs(&mut expected.attrs);
            let mut matching = Vec::new();
            for item in self.items(unit)? {
                let syn::Item::Verbatim(tokens) = item else {
                    continue;
                };
                let Ok(mut actual) = syn::parse2::<ConstTraitImpl>(tokens.clone()) else {
                    continue;
                };
                strip_docs(&mut actual.attrs);
                if spelling(&actual.header) != spelling(&expected.header) {
                    continue;
                }
                for member in &mut actual.items {
                    let syn::ImplItem::Fn(method) = member else {
                        return Err(Refusal::SourceShape(unit));
                    };
                    strip_docs(&mut method.attrs);
                }
                matching.push(actual);
            }
            let [actual] = matching.as_slice() else {
                return Err(Refusal::SourceShape(unit));
            };
            let actual_attrs = &actual.attrs;
            let expected_attrs = &expected.attrs;
            let actual_items = &actual.items;
            let expected_items = &expected.items;
            if spelling(&quote!(#(#actual_attrs)* #(#actual_items)*))
                != spelling(&quote!(#(#expected_attrs)* #(#expected_items)*))
            {
                return Err(Refusal::SourceShape(unit));
            }
            let [syn::ImplItem::Fn(method)] = actual.items.as_slice() else {
                return Err(Refusal::SourceShape(unit));
            };
            Ok(method.clone())
        }

        fn structure(&self, unit: SourceUnit, wanted: TokenStream) -> Result<(), Refusal> {
            let mut expected: syn::ItemStruct =
                syn::parse2(wanted).map_err(|_| Refusal::SourceShape(unit))?;
            let mut found = self.items(unit)?.iter().filter_map(|item| match item {
                syn::Item::Struct(value) if value.ident == expected.ident => Some(value),
                _ => None,
            });
            let mut actual = found.next().cloned().ok_or(Refusal::SourceShape(unit))?;
            if found.next().is_some() {
                return Err(Refusal::SourceShape(unit));
            }
            if let (syn::Fields::Named(fields), syn::Fields::Named(required)) =
                (&mut actual.fields, &expected.fields)
            {
                fields.named = mem::take(&mut fields.named)
                    .into_iter()
                    .filter(|field| required.named.iter().any(|item| item.ident == field.ident))
                    .collect();
            }
            for declaration in [&mut actual, &mut expected] {
                strip_docs(&mut declaration.attrs);
                for field in &mut declaration.fields {
                    strip_docs(&mut field.attrs);
                }
                declaration.generics.params = mem::take(&mut declaration.generics.params)
                    .into_iter()
                    .collect();
                match &mut declaration.fields {
                    syn::Fields::Named(fields) => {
                        fields.named = mem::take(&mut fields.named).into_iter().collect()
                    }
                    syn::Fields::Unnamed(fields) => {
                        fields.unnamed = mem::take(&mut fields.unnamed).into_iter().collect()
                    }
                    syn::Fields::Unit => {}
                }
            }
            if spelling(&actual) != spelling(&expected) {
                return Err(Refusal::SourceShape(unit));
            }
            Ok(())
        }

        fn exact(&self, unit: SourceUnit, wanted: &TokenStream) -> Result<(), Refusal> {
            if self
                .items(unit)?
                .iter()
                .filter(|item| {
                    let mut actual: syn::Item = (**item).clone();
                    match &mut actual {
                        syn::Item::Type(value) => strip_docs(&mut value.attrs),
                        syn::Item::Use(value) => strip_docs(&mut value.attrs),
                        syn::Item::Static(value) => strip_docs(&mut value.attrs),
                        syn::Item::Const(value) => strip_docs(&mut value.attrs),
                        syn::Item::Mod(value) => strip_docs(&mut value.attrs),
                        _ => return false,
                    }
                    spelling(&actual) == spelling(wanted)
                })
                .count()
                != 1
            {
                return Err(Refusal::SourceShape(unit));
            }
            Ok(())
        }

        fn binding(
            &self,
            unit: SourceUnit,
            local: &str,
            wanted: &TokenStream,
            attrs: &TokenStream,
        ) -> Result<(), Refusal> {
            let mut found = Vec::new();
            for item in self.items(unit)? {
                let syn::Item::Use(import) = item else {
                    continue;
                };
                let mut leaves = Vec::new();
                import_leaves(&import.tree, Vec::new(), &mut leaves);
                for (name, path) in leaves {
                    if name != local {
                        continue;
                    }
                    let actual_attrs = &import.attrs;
                    if spelling(&quote!(#(#actual_attrs)*)) != spelling(attrs)
                        || import.leading_colon.is_some()
                        || !matches!(import.vis, syn::Visibility::Inherited)
                    {
                        return Err(Refusal::SourceShape(unit));
                    }
                    found.push(path);
                }
            }
            if found != [spelling(wanted)] {
                return Err(Refusal::SourceShape(unit));
            }
            Ok(())
        }
    }

    fn import_leaves(
        tree: &syn::UseTree,
        mut prefix: Vec<syn::Ident>,
        out: &mut Vec<(String, String)>,
    ) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.clone());
                import_leaves(&path.tree, prefix, out);
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    import_leaves(item, prefix.clone(), out);
                }
            }
            syn::UseTree::Name(name) => {
                if name.ident != "self" {
                    prefix.push(name.ident.clone());
                }
                if let Some(local) = prefix.last() {
                    out.push((local.to_string(), spelling(&quote!(#(#prefix)::*))));
                }
            }
            syn::UseTree::Rename(name) => {
                if name.ident != "self" {
                    prefix.push(name.ident.clone());
                }
                out.push((name.rename.to_string(), spelling(&quote!(#(#prefix)::*))));
            }
            syn::UseTree::Glob(_) => {}
        }
    }

    fn calls(method: &syn::ImplItemFn, callee: &TokenStream) -> Result<u64, Refusal> {
        struct Count {
            callee: String,
            count: Option<u64>,
        }
        impl VisitMut for Count {
            fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
                if spelling(&call.func) == self.callee || spelling(call) == self.callee {
                    self.count = self.count.and_then(|count| count.checked_add(1));
                }
                visit_mut::visit_expr_call_mut(self, call);
            }
        }
        let mut count = Count {
            callee: spelling(callee),
            count: Some(0),
        };
        count.visit_impl_item_fn_mut(&mut method.clone());
        count.count.ok_or(Refusal::Overflow)
    }

    fn one(
        mut methods: Vec<syn::ImplItemFn>,
        unit: SourceUnit,
    ) -> Result<syn::ImplItemFn, Refusal> {
        if methods.len() != 1 {
            return Err(Refusal::SourceShape(unit));
        }
        methods.pop().ok_or(Refusal::SourceShape(unit))
    }

    fn multiply(left: u64, right: u64) -> Result<u64, Refusal> {
        left.checked_mul(right).ok_or(Refusal::Overflow)
    }

    pub(super) fn check(
        data: &ConstructionData,
        expected: &CompilerData,
    ) -> Result<Counts, Refusal> {
        let sources = capture(data, expected)?;
        imports(&sources)?;
        declarations(&sources)?;
        strategy(&sources)?;
        let paired = paired_cache(&sources)?;
        let dfa = dfa_cache(&sources)?;
        let map = map_new(&sources)?;
        let map_default = sources.constant_default()?;
        let random_default = random_state(&sources)?;
        map_storage(&sources)?;
        let hostname_caches = hostname(&sources, &data.hostname.pattern)?;
        // Matched-callee event table: the leaf is one whole service entry.
        // Ordinary sibling calls are retained obligations, not zero-cost rows.
        let random_entries = calls(&random_default, &quote!(RandomState::new))?;
        let default_entries = multiply(
            calls(&map_default, &quote!(Default::default))?,
            random_entries,
        )?;
        let map_entries = multiply(calls(&map, &quote!(Default::default))?, default_entries)?;
        let direction_entries = multiply(calls(&dfa, &quote!(StateMap::new))?, map_entries)?;
        let forward = multiply(
            calls(&paired, &quote!(dfa::Cache::new(re.forward())))?,
            direction_entries,
        )?;
        let reverse = multiply(
            calls(&paired, &quote!(dfa::Cache::new(re.reverse())))?,
            direction_entries,
        )?;
        Ok(Counts {
            forward,
            reverse,
            hostname_caches,
        })
    }

    fn imports(sources: &Sources) -> Result<(), Refusal> {
        // Local shadowing cannot supply the selected trait/extern identity.
        // Crate-root/prelude/re-export resolution still belongs to actual use.
        for (unit, name) in [
            (SourceUnit::HashMap, "Default"),
            (SourceUnit::RandomState, "Default"),
            (SourceUnit::HybridDfa, "std"),
            (SourceUnit::HashMap, "hashbrown"),
        ] {
            unbound(sources, unit, name)?;
        }
        selected_imports(sources)
    }

    fn unbound(sources: &Sources, unit: SourceUnit, name: &str) -> Result<(), Refusal> {
        for item in sources.items(unit)? {
            let ident = match item {
                syn::Item::Const(value) => Some(&value.ident),
                syn::Item::Enum(value) => Some(&value.ident),
                syn::Item::ExternCrate(value) => Some(
                    value
                        .rename
                        .as_ref()
                        .map_or(&value.ident, |(_, local)| local),
                ),
                syn::Item::Fn(value) => Some(&value.sig.ident),
                syn::Item::Macro(value) => value.ident.as_ref(),
                syn::Item::Mod(value) => Some(&value.ident),
                syn::Item::Static(value) => Some(&value.ident),
                syn::Item::Struct(value) => Some(&value.ident),
                syn::Item::Trait(value) => Some(&value.ident),
                syn::Item::TraitAlias(value) => Some(&value.ident),
                syn::Item::Type(value) => Some(&value.ident),
                syn::Item::Union(value) => Some(&value.ident),
                _ => None,
            };
            if ident.is_some_and(|ident| ident == name) {
                return Err(Refusal::SourceShape(unit));
            }
            if let syn::Item::Use(import) = item {
                let mut bindings = Vec::new();
                import_leaves(&import.tree, Vec::new(), &mut bindings);
                if bindings.iter().any(|(local, _)| local == name) {
                    return Err(Refusal::SourceShape(unit));
                }
            }
        }
        Ok(())
    }

    fn selected_imports(sources: &Sources) -> Result<(), Refusal> {
        use SourceUnit::{
            HashMap, HashbrownMap, HashbrownRaw, HybridRegex, MetaStrategy, MetaWrappers,
            RandomState,
        };
        for (unit, local, path, attrs) in [
            (
                MetaStrategy,
                "wrappers",
                quote!(crate::meta::wrappers),
                quote!(),
            ),
            (
                MetaStrategy,
                "Cache",
                quote!(crate::meta::regex::Cache),
                quote!(),
            ),
            (
                MetaWrappers,
                "hybrid",
                quote!(crate::hybrid),
                quote!(#[cfg(feature = "hybrid")]),
            ),
            (HybridRegex, "dfa", quote!(crate::hybrid::dfa), quote!()),
            (
                HybridRegex,
                "DFA",
                quote!(crate::hybrid::dfa::DFA),
                quote!(),
            ),
            (
                HashMap,
                "RandomState",
                quote!(crate::hash::RandomState),
                quote!(),
            ),
            (HashMap, "base", quote!(hashbrown::hash_map), quote!()),
            (
                HashbrownMap,
                "RawTable",
                quote!(crate::raw::RawTable),
                quote!(),
            ),
            (
                HashbrownMap,
                "Global",
                quote!(crate::alloc::Global),
                quote!(),
            ),
            (
                HashbrownRaw,
                "Global",
                quote!(crate::alloc::Global),
                quote!(),
            ),
            (RandomState, "Cell", quote!(crate::cell::Cell), quote!()),
            (
                RandomState,
                "hashmap_random_keys",
                quote!(crate::sys::random::hashmap_random_keys),
                quote!(),
            ),
        ] {
            sources.binding(unit, local, &path, &attrs)?;
        }
        sources.exact(
            SourceUnit::HashExports,
            &quote!(
                #[stable(feature = "std_hash_exports", since = "1.76.0")]
                pub use self::random::{DefaultHasher, RandomState};
            ),
        )?;
        sources.exact(
            SourceUnit::HashExports,
            &quote!(
                pub(crate) mod random;
            ),
        )
    }

    fn declarations(sources: &Sources) -> Result<(), Refusal> {
        strategy_declarations(sources)?;
        for (unit, declaration) in [
            (
                SourceUnit::HybridRegex,
                quote!(
                    #[derive(Debug)]
                    pub struct Regex {
                        forward: DFA,
                        reverse: DFA,
                    }
                ),
            ),
            (
                SourceUnit::HybridRegex,
                quote!(
                    #[derive(Debug, Clone)]
                    pub struct Cache {
                        forward: dfa::Cache,
                        reverse: dfa::Cache,
                    }
                ),
            ),
            (
                SourceUnit::HybridDfa,
                quote!(
                    #[derive(Clone, Debug)]
                    pub struct Cache {
                        states_to_id: StateMap,
                    }
                ),
            ),
            (
                SourceUnit::MetaWrappers,
                quote!(
                    #[derive(Debug)]
                    pub(crate) struct Hybrid(Option<HybridEngine>);
                ),
            ),
            (
                SourceUnit::MetaWrappers,
                quote!(
                    #[derive(Debug)]
                    pub(crate) struct HybridEngine(
                        #[cfg(feature = "hybrid")] hybrid::regex::Regex,
                        #[cfg(not(feature = "hybrid"))] (),
                    );
                ),
            ),
            (
                SourceUnit::MetaWrappers,
                quote!(
                    #[derive(Clone, Debug)]
                    pub(crate) struct HybridCache(
                        #[cfg(feature = "hybrid")] Option<hybrid::regex::Cache>,
                        #[cfg(not(feature = "hybrid"))] (),
                    );
                ),
            ),
        ] {
            sources.structure(unit, declaration)?;
        }
        map_declarations(sources)
    }

    fn map_declarations(sources: &Sources) -> Result<(), Refusal> {
        for (unit, declaration) in [
            (
                SourceUnit::RandomState,
                quote!(
                    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
                    #[derive(Clone)]
                    pub struct RandomState {
                        k0: u64,
                        k1: u64,
                    }
                ),
            ),
            (
                SourceUnit::HashMap,
                quote!(
                    #[cfg_attr(not(test), rustc_diagnostic_item = "HashMap")]
                    #[stable(feature = "rust1", since = "1.0.0")]
                    #[rustc_insignificant_dtor]
                    pub struct HashMap<
                        K,
                        V,
                        S = RandomState,
                        #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
                    > {
                        base: base::HashMap<K, V, S, A>,
                    }
                ),
            ),
            (
                SourceUnit::HashbrownMap,
                quote!(
                    pub struct HashMap<K, V, S = DefaultHashBuilder, A: Allocator = Global> {
                        pub(crate) hash_builder: S,
                        pub(crate) table: RawTable<(K, V), A>,
                    }
                ),
            ),
        ] {
            sources.structure(unit, declaration)?;
        }
        sources.exact(
            SourceUnit::HybridDfa,
            &quote!(
                #[cfg(feature = "std")]
                type StateMap = std::collections::HashMap<State, LazyStateID>;
            ),
        )?;
        sources.exact(
            SourceUnit::HybridDfa,
            &quote!(
                #[cfg(not(feature = "std"))]
                type StateMap = alloc::collections::BTreeMap<State, LazyStateID>;
            ),
        )
    }

    fn strategy_declarations(sources: &Sources) -> Result<(), Refusal> {
        for declaration in [
            quote!(
                #[derive(Debug)]
                struct Core {
                    hybrid: wrappers::Hybrid,
                }
            ),
            quote!(
                #[derive(Debug)]
                struct ReverseAnchored {
                    core: Core,
                }
            ),
            quote!(
                #[derive(Debug)]
                struct ReverseSuffix {
                    core: Core,
                }
            ),
        ] {
            sources.structure(SourceUnit::MetaStrategy, declaration)?;
        }
        Ok(())
    }

    fn strategy(sources: &Sources) -> Result<(), Refusal> {
        sources.implementation(
            SourceUnit::MetaStrategy,
            quote!(impl Strategy for Core {
                #[cfg_attr(feature = "perf-inline", inline(always))]
                fn create_cache(&self) -> Cache {
                    Cache {
                        capmatches: Captures::all(self.group_info().clone()),
                        pikevm: self.pikevm.create_cache(),
                        backtrack: self.backtrack.create_cache(),
                        onepass: self.onepass.create_cache(),
                        hybrid: self.hybrid.create_cache(),
                        revhybrid: wrappers::ReverseHybridCache::none(),
                    }
                }
            }),
        )?;
        for owner in [quote!(ReverseAnchored), quote!(ReverseSuffix)] {
            sources.implementation(
                SourceUnit::MetaStrategy,
                quote!(impl Strategy for #owner {
                    #[cfg_attr(feature = "perf-inline", inline(always))]
                    fn create_cache(&self) -> Cache { self.core.create_cache() }
                }),
            )?;
        }
        sources.implementation(
            SourceUnit::MetaWrappers,
            quote!(impl Hybrid {
                pub(crate) fn create_cache(&self) -> HybridCache { HybridCache::new(self) }
            }),
        )?;
        sources.implementation(SourceUnit::MetaWrappers, quote!(impl HybridCache {
            pub(crate) fn new(builder: &Hybrid) -> HybridCache {
                #[cfg(feature = "hybrid")] { HybridCache(builder.0.as_ref().map(|e| e.0.create_cache())) }
                #[cfg(not(feature = "hybrid"))] { HybridCache(()) }
            }
        }))?;
        Ok(())
    }

    fn paired_cache(sources: &Sources) -> Result<syn::ImplItemFn, Refusal> {
        sources.implementation(
            SourceUnit::HybridRegex,
            quote!(impl Regex {
                pub fn create_cache(&self) -> Cache { Cache::new(self) }
                pub fn forward(&self) -> &DFA { &self.forward }
                pub fn reverse(&self) -> &DFA { &self.reverse }
            }),
        )?;
        one(
            sources.implementation(
                SourceUnit::HybridRegex,
                quote!(impl Cache {
                    pub fn new(re: &Regex) -> Cache {
                        let forward = dfa::Cache::new(re.forward());
                        let reverse = dfa::Cache::new(re.reverse());
                        Cache { forward, reverse }
                    }
                }),
            )?,
            SourceUnit::HybridRegex,
        )
    }

    fn dfa_cache(sources: &Sources) -> Result<syn::ImplItemFn, Refusal> {
        one(sources.implementation(SourceUnit::HybridDfa, quote!(impl Cache {
            pub fn new(dfa: &DFA) -> Cache {
                let mut cache = Cache {
                    trans: alloc::vec![], starts: alloc::vec![], states: alloc::vec![],
                    states_to_id: StateMap::new(), sparses: SparseSets::new(dfa.get_nfa().states().len()),
                    stack: alloc::vec![], scratch_state_builder: StateBuilderEmpty::new(),
                    state_saver: StateSaver::none(), memory_usage_state: 0, clear_count: 0,
                    bytes_searched: 0, progress: None,
                };
                debug!("pre-init lazy DFA cache size: {}", cache.memory_usage());
                Lazy { dfa, cache: &mut cache }.init_cache();
                debug!("post-init lazy DFA cache size: {}", cache.memory_usage());
                cache
            }
        }))?, SourceUnit::HybridDfa)
    }

    fn map_new(sources: &Sources) -> Result<syn::ImplItemFn, Refusal> {
        one(
            sources.implementation(
                SourceUnit::HashMap,
                quote!(
                    impl<K, V> HashMap<K, V, RandomState> {
                        #[inline]
                        #[must_use]
                        #[stable(feature = "rust1", since = "1.0.0")]
                        pub fn new() -> HashMap<K, V, RandomState> {
                            Default::default()
                        }
                    }
                ),
            )?,
            SourceUnit::HashMap,
        )
    }

    fn random_state(sources: &Sources) -> Result<syn::ImplItemFn, Refusal> {
        // Match the whole supported service entry without pricing its warm/cold
        // body or extracting an OS call multiplier. Existing service facts own it.
        sources.implementation(SourceUnit::RandomState, quote!(impl RandomState {
            #[inline] #[allow(deprecated)] #[must_use]
            #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
            pub fn new() -> RandomState {
                thread_local!(static KEYS: Cell<(u64, u64)> = { Cell::new(hashmap_random_keys()) });
                KEYS.with(|keys| {
                    let (k0, k1) = keys.get();
                    keys.set((k0.wrapping_add(1), k1));
                    RandomState { k0, k1 }
                })
            }
        }))?;
        one(
            sources.implementation(
                SourceUnit::RandomState,
                quote!(
                    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
                    impl Default for RandomState {
                        #[inline]
                        fn default() -> RandomState {
                            RandomState::new()
                        }
                    }
                ),
            )?,
            SourceUnit::RandomState,
        )
    }

    fn map_storage(sources: &Sources) -> Result<(), Refusal> {
        let mut initialized: syn::ExprStruct = syn::parse_quote!(HashMap {
            base: base::HashMap::with_hasher(hash_builder)
        });
        initialized.fields = mem::take(&mut initialized.fields).into_iter().collect();
        sources.implementation(
            SourceUnit::HashMap,
            quote!(
                impl<K, V, S> HashMap<K, V, S> {
                    #[inline]
                    #[must_use]
                    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
                    #[rustc_const_stable(
                        feature = "const_collections_with_hasher",
                        since = "1.85.0"
                    )]
                    pub const fn with_hasher(hash_builder: S) -> HashMap<K, V, S> {
                        #initialized
                    }
                }
            ),
        )?;
        sources.implementation(
            SourceUnit::HashbrownMap,
            quote!(
                impl<K, V, S> HashMap<K, V, S> {
                    #[must_use]
                    #[cfg_attr(feature = "inline-more", inline)]
                    #[cfg_attr(feature = "rustc-dep-of-std", rustc_const_stable_indirect)]
                    pub const fn with_hasher(hash_builder: S) -> Self {
                        Self {
                            hash_builder,
                            table: RawTable::new(),
                        }
                    }
                }
            ),
        )?;
        sources.implementation(
            SourceUnit::HashbrownRaw,
            quote!(
                impl<T> RawTable<T, Global> {
                    #[inline]
                    #[cfg_attr(feature = "rustc-dep-of-std", rustc_const_stable_indirect)]
                    pub(crate) const fn new() -> Self {
                        Self {
                            table: RawTableInner::NEW,
                            alloc: Global,
                            marker: PhantomData,
                        }
                    }
                }
            ),
        )?;
        Ok(())
    }

    fn hostname(sources: &Sources, pattern: &str) -> Result<u64, Refusal> {
        let unit = SourceUnit::Hostname;
        let mut modules = sources.items(unit)?.iter().filter_map(|item| match item {
            syn::Item::Mod(module) if module.ident == "formats" => Some(module),
            _ => None,
        });
        let module = modules.next().ok_or(Refusal::SourceShape(unit))?;
        if modules.next().is_some() {
            return Err(Refusal::SourceShape(unit));
        }
        let attrs = &module.attrs;
        if spelling(&quote!(#(#attrs)*))
            != spelling(
                &quote!(#[cfg(any(not(target_arch = "wasm32"), feature = "schema-artifact-staging"))]),
            )
            || !matches!(module.vis, syn::Visibility::Inherited)
        {
            return Err(Refusal::SourceShape(unit));
        }
        let (_, items) = module.content.as_ref().ok_or(Refusal::SourceShape(unit))?;
        let nested = Sources(BTreeMap::from([(
            unit,
            syn::File {
                shebang: None,
                frontmatter: None,
                attrs: Vec::new(),
                items: items.clone(),
            },
        )]));
        nested.binding(
            unit,
            "Regex",
            &quote!(regex_automata::meta::Regex),
            &quote!(),
        )?;
        nested.binding(unit, "Input", &quote!(regex_automata::Input), &quote!())?;
        nested.binding(
            unit,
            "MatchKind",
            &quote!(regex_automata::MatchKind),
            &quote!(),
        )?;
        nested.binding(
            unit,
            "syntax",
            &quote!(regex_automata::util::syntax),
            &quote!(),
        )?;
        nested.binding(unit, "LazyLock", &quote!(std::sync::LazyLock), &quote!())?;
        hostname_pattern(&nested, items, pattern)?;
        hostname_function(items)
    }

    fn hostname_pattern(
        nested: &Sources,
        items: &[syn::Item],
        pattern: &str,
    ) -> Result<(), Refusal> {
        let unit = SourceUnit::Hostname;
        let mut literals = items.iter().filter_map(|item| match item {
            syn::Item::Const(value) if value.ident == "HOSTNAME_SOURCE" => Some(value),
            _ => None,
        });
        let literal = literals.next().ok_or(Refusal::SourceShape(unit))?;
        if literals.next().is_some()
            || !literal.attrs.is_empty()
            || !matches!(literal.vis, syn::Visibility::Inherited)
            || spelling(&literal.ty) != spelling(&quote!(&str))
        {
            return Err(Refusal::SourceShape(unit));
        }
        let syn::Expr::Lit(expression) = literal.expr.as_ref() else {
            return Err(Refusal::SourceShape(unit));
        };
        let syn::Lit::Str(value) = &expression.lit else {
            return Err(Refusal::SourceShape(unit));
        };
        if value.value() != pattern || !expression.attrs.is_empty() {
            return Err(Refusal::SourceShape(unit));
        }
        nested.exact(
            unit,
            &quote!(
                static HOSTNAME_PATTERN: LazyLock<Option<Regex>> = LazyLock::new(|| {
                    Regex::builder()
                        .configure(
                            Regex::config()
                                .match_kind(MatchKind::LeftmostFirst)
                                .utf8_empty(true)
                                .nfa_size_limit(Some(0x0001_0000))
                                .hybrid_cache_capacity(0x0001_0000)
                                .pool_capacity(1),
                        )
                        .syntax(syntax::Config::default().utf8(true))
                        .build(HOSTNAME_SOURCE)
                        .ok()
                });
            ),
        )
    }

    fn hostname_function(items: &[syn::Item]) -> Result<u64, Refusal> {
        let unit = SourceUnit::Hostname;
        let expected: syn::ItemFn = syn::parse2(quote!(
            pub(super) fn hostname(value: &str) -> bool {
                value.len() < 256
                    && HOSTNAME_PATTERN.as_ref().is_some_and(|pattern| {
                        let mut cache = pattern.create_cache();
                        pattern
                            .search_half_with(&mut cache, &Input::new(value).earliest(true))
                            .is_some()
                    })
            }
        ))
        .map_err(|_| Refusal::SourceShape(unit))?;
        let mut functions = items.iter().filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "hostname" => Some(function),
            _ => None,
        });
        let mut function = functions
            .next()
            .cloned()
            .ok_or(Refusal::SourceShape(unit))?;
        if functions.next().is_some() {
            return Err(Refusal::SourceShape(unit));
        }
        strip_docs(&mut function.attrs);
        if spelling(&function) != spelling(&expected) {
            return Err(Refusal::SourceShape(unit));
        }
        struct Caches {
            count: Option<u64>,
        }
        impl VisitMut for Caches {
            fn visit_expr_method_call_mut(&mut self, call: &mut syn::ExprMethodCall) {
                if call.method == "create_cache" {
                    self.count = self.count.and_then(|count| count.checked_add(1));
                }
                visit_mut::visit_expr_method_call_mut(self, call);
            }
        }
        let mut caches = Caches { count: Some(0) };
        caches.visit_item_fn_mut(&mut function);
        caches.count.ok_or(Refusal::Overflow)
    }
}

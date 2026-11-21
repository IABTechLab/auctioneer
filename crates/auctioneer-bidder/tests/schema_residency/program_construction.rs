//! Conditional single-program constructor recipe for the current pattern shapes.
//! Native AST/HIR/NFA values are structural inputs, never target layout evidence.
//! No physical fit, serving, machine-stack, instruction or recoverable-OOM claim.

use crate::schema_heap_layout::NativeField;
use crate::schema_onepass_construction::OnePassConstructionCertificate;
use crate::schema_program_heap::{NativeProgramHeap, ProgramRecipe};
use regex_automata::meta::Config;
use std::{collections::{BTreeMap, BTreeSet}, path::{Path, PathBuf}};

use super::literal_construction::KEYS as SUFFIX_KEYS;

pub(crate) struct Sources<'a> {
    pub workspace: &'a Path,
    pub syntax: &'a Path,
    pub rust_release: &'a str,
    /// Actual resolved dependency features supplied by the existing build owner.
    pub features: &'a BTreeSet<String>,
}

pub(crate) struct Inputs<'a> {
    pub owner: &'a str,
    pub pattern: &'a str,
    pub config: &'a Config,
    pub program: &'a ProgramRecipe<'a>,
    pub layouts: &'a BTreeMap<String, NativeField>,
    pub sources: Sources<'a>,
}

/// Sealed source selection, not a physical cost or a constructor-completeness flag.
#[derive(Debug)]
pub(crate) struct SourceRecipe {
    owner: String,
    pattern: String,
    config: Config,
    workspace: PathBuf,
    syntax: PathBuf,
    rust_release: String,
    features: BTreeSet<String>,
    required_facts: BTreeSet<String>,
    phase_models: [Stage; 13],
    work_models: [Work; 15],
}

impl SourceRecipe {
    // Hosted-RED source-selection scaffold; no layout facts are accepted here.
    pub(crate) fn from_source(
        owner: &str, pattern: &str, config: &Config, sources: Sources<'_>,
    ) -> Result<Self, Refusal> {
        let _ = (owner, pattern, config, sources.workspace, sources.syntax,
            sources.rust_release, sources.features);
        Err(Refusal::NotImplemented)
    }

    pub(crate) fn owner(&self) -> &str { &self.owner }
    pub(crate) fn pattern(&self) -> &str { &self.pattern }
    pub(crate) fn required_facts(&self) -> &BTreeSet<String> { &self.required_facts }
    pub(crate) fn phase_models(&self) -> &[Stage] { &self.phase_models }
    pub(crate) fn work_models(&self) -> &[Work] { &self.work_models }

    /// Numbers require all matched target facts; work may still be Unbound.
    pub(crate) fn evaluate(
        &self, layouts: &BTreeMap<String, NativeField>, program: &ProgramRecipe<'_>,
    ) -> Result<Certificate, Refusal> {
        evaluate(&Inputs {
            owner: &self.owner, pattern: &self.pattern, config: &self.config,
            program, layouts,
            sources: Sources {
                workspace: &self.workspace, syntax: &self.syntax,
                rust_release: &self.rust_release, features: &self.features,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Stage {
    Syntax, Literal, ForwardNfa, PikeVm, Backtrack, OnePass, ReverseNfa,
    HybridForward, HybridReverse, HybridPair, Publication, Cleanup, FailureCleanup,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Work {
    Syntax, OuterVectors, ForwardNfa, ReverseNfa, OnePass, Literal,
    WrapperScaffolds, HybridChecks, Publication,
    LiteralHir, LiteralRanges, LiteralSequences, LiteralCopies, LiteralTrie, LiteralSelection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Visits {
    Bound(usize),
    Unbound(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailurePrefix {
    SyntaxReplacementAndCleanup,
    NfaMutationAndCleanup,
    OnePassRefusalAndCleanup,
    HybridCapacityAndCleanup,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    NotImplemented,
    Configuration,
    PatternScope,
    Features,
    Missing(String),
    Layout,
    Arithmetic,
    Source(&'static str),
    RetainedCoverage,
    UnboundWork(&'static str),
}

#[derive(Debug)]
pub(crate) struct Certificate {
    pub owner: String,
    pub retained: NativeProgramHeap,
    /// Each phase contains construction-only storage, disjoint from Root C.
    /// A conservative syntax reserve remains charged through outer AST/HIR drop;
    /// inactive parser/translator storage is not subtracted or counted as C.
    pub phase_scratch: BTreeMap<Stage, usize>,
    pub construction_only_bytes: usize,
    pub onepass_attempted: bool,
    pub work: BTreeMap<Work, Visits>,
    /// Source-prefix coverage, not an observed/reachable admitted failure.
    pub failure_prefixes: [FailurePrefix; 4],
}

impl Certificate {
    pub(crate) fn complete_work(&self) -> Result<usize, Refusal> {
        let total = self.work.values().try_fold(0_usize, |total, value| match value {
            Visits::Bound(value) => total.checked_add(*value).ok_or(Refusal::Arithmetic),
            Visits::Unbound(owner) => Err(Refusal::UnboundWork(owner)),
        })?;
        if [Work::Syntax, Work::OuterVectors, Work::ForwardNfa, Work::ReverseNfa,
            Work::OnePass, Work::Literal, Work::WrapperScaffolds, Work::HybridChecks,
            Work::Publication, Work::LiteralHir, Work::LiteralRanges, Work::LiteralSequences,
            Work::LiteralCopies, Work::LiteralTrie, Work::LiteralSelection]
            .into_iter().any(|owner| !self.work.contains_key(&owner))
        {
            return Err(Refusal::UnboundWork("missing source work owner"));
        }
        Ok(total)
    }
}

pub(crate) fn required_fact_keys() -> BTreeSet<String> {
    use crate::{schema_nfa_construction, schema_nfa_heap::target,
        schema_onepass_construction, schema_syntax_construction};
    schema_syntax_construction::PRIVATE_TYPES.into_iter()
        .chain(schema_syntax_construction::PAYLOAD_TYPES)
        .chain([schema_syntax_construction::ABI_POINTER])
        .chain(target::fact_keys())
        .chain(schema_nfa_construction::additional_layout_keys())
        .chain(schema_onepass_construction::fact_keys())
        .chain(SUFFIX_KEYS)
        .map(str::to_owned).collect()
}

// Hosted-RED API scaffold. No source/target completeness is claimed.
pub(crate) fn evaluate(input: &Inputs<'_>) -> Result<Certificate, Refusal> {
    let _ = (input.owner, input.pattern, input.config, input.program, input.layouts,
        input.sources.workspace, input.sources.syntax,
        input.sources.features, input.sources.rust_release);
    Err(Refusal::NotImplemented)
}

fn outer_vectors(_ast: NativeField, _hir: NativeField, _word: NativeField) -> Result<usize, Refusal> {
    Ok(0_usize)
}

fn hybrid_dead(_byte: NativeField, _header: NativeField, _word: NativeField) -> Result<usize, Refusal> {
    Ok(0_usize)
}

fn onepass_scratch(certificate: &OnePassConstructionCertificate, _covered_by_c: usize) -> Result<usize, Refusal> {
    Ok(certificate.peak_requested_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        schema_formats::HOSTNAME_SOURCE,
        schema_literal_heap::AutomaticPrefilterHeap,
        schema_metadata_heap::MetadataLayout,
        schema_nfa_construction,
        schema_nfa_heap::target,
        schema_onepass_construction,
        schema_pool_heap::PoolLayout,
    };
    use regex_automata::{
        MatchKind,
        nfa::thompson::{State, Transition, WhichCaptures},
        util::primitives::{SmallIndex, StateID},
    };
    use regex_syntax::hir::ClassBytesRange;
    use std::{any::type_name, env, fs, path::PathBuf, sync::atomic::AtomicUsize};

    fn field(bytes: usize, alignment: usize) -> NativeField { NativeField { bytes, alignment } }

    fn config() -> Config {
        Config::new().match_kind(MatchKind::LeftmostFirst).utf8_empty(true)
            .which_captures(WhichCaptures::All).byte_classes(true).line_terminator(b'\n')
            .nfa_size_limit(Some(0x0001_0000_usize)).hybrid_cache_capacity(0x0001_0000_usize)
            .onepass_size_limit(Some(0x0010_0000_usize)).pool_capacity(1_usize)
            .hybrid(true).dfa(false).onepass(true).auto_prefilter(true)
    }

    // Synthetic layout assumptions only; none is a compiler/guest observation.
    fn layouts() -> BTreeMap<String, NativeField> {
        let mut facts = required_fact_keys().into_iter()
            .map(|key| (key, field(64_usize, 8_usize))).collect::<BTreeMap<_, _>>();
        facts.insert(target::ABI_POINTER.into(), field(8_usize, 8_usize));
        facts.insert(type_name::<AtomicUsize>().into(), field(8_usize, 8_usize));
        facts.insert(type_name::<State>().into(), field(32_usize, 8_usize));
        facts.insert(type_name::<Transition>().into(), field(8_usize, 4_usize));
        facts.insert(type_name::<StateID>().into(), field(4_usize, 4_usize));
        facts.insert(type_name::<(SmallIndex, SmallIndex)>().into(), field(8_usize, 4_usize));
        facts.insert(type_name::<bool>().into(), field(1_usize, 1_usize));
        facts.insert(type_name::<ClassBytesRange>().into(), field(2_usize, 1_usize));
        facts.insert(schema_nfa_construction::BUILDER_STATE.into(), field(32_usize, 8_usize));
        facts.insert(schema_nfa_construction::TRIE_STATE.into(), field(24_usize, 8_usize));
        facts.insert(schema_nfa_construction::HIR_PROPERTIES.into(), field(80_usize, 8_usize));
        facts.insert(schema_onepass_construction::STATE_ID.into(), field(4_usize, 4_usize));
        facts.insert(schema_onepass_construction::TRANSITION.into(), field(8_usize, 8_usize));
        facts.insert(schema_onepass_construction::EPSILONS.into(), field(8_usize, 8_usize));
        facts.insert(schema_onepass_construction::BYTE_CLASSES.into(), field(256_usize, 8_usize));
        facts
    }

    fn program<'a>(pattern: &'a str, facts: &'a BTreeMap<String, NativeField>) -> ProgramRecipe<'a> {
        ProgramRecipe {
            pattern,
            metadata: MetadataLayout {
                regex_fields: [field(16_usize, 8_usize), field(8_usize, 8_usize)],
                info_fields: [field(40_usize, 8_usize), field(24_usize, 8_usize), field(8_usize, 8_usize)],
                props_payload: field(80_usize, 8_usize), static_inline: field(64_usize, 8_usize),
            },
            arc_header: field(16_usize, 8_usize), strategy_payload: field(64_usize, 8_usize),
            nfa_facts: facts, onepass_word: field(8_usize, 8_usize), memchr_payload: field(1_usize, 1_usize),
            pool: PoolLayout {
                fields: [field(16_usize, 8_usize), field(24_usize, 8_usize),
                    field(8_usize, 8_usize), field(40_usize, 8_usize)],
                shard: field(40_usize, 8_usize), closure: field(16_usize, 8_usize),
            },
        }
    }

    fn features() -> BTreeSet<String> {
        crate::schema_build::schema_regex_vendor::FEATURES
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn source_locations() -> (PathBuf, PathBuf) {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let cargo = env::var_os("CARGO_HOME").map_or_else(
            || PathBuf::from(env::var_os("HOME").unwrap()).join(".cargo"), PathBuf::from,
        );
        let mut source = None;
        for entry in fs::read_dir(cargo.join("registry/src")).unwrap() {
            let path = entry.unwrap().path().join("regex-syntax-0.8.11");
            if path.is_dir() { source = Some(path.canonicalize().unwrap()); break; }
        }
        (workspace, source.unwrap())
    }

    #[test]
    fn outer_vectors_and_dead_state_have_independent_literal_layout_axes() {
        assert_eq!(outer_vectors(field(32_usize, 8_usize), field(16_usize, 8_usize), field(8_usize, 8_usize)), Ok(200_usize));
        assert_eq!(outer_vectors(field(40_usize, 8_usize), field(16_usize, 8_usize), field(8_usize, 8_usize)), Ok(232_usize));
        assert_eq!(outer_vectors(field(32_usize, 8_usize), field(24_usize, 8_usize), field(8_usize, 8_usize)), Ok(232_usize));
        assert_eq!(hybrid_dead(field(1_usize, 1_usize), field(8_usize, 4_usize), field(4_usize, 4_usize)), Ok(29_usize));
        assert_eq!(hybrid_dead(field(1_usize, 1_usize), field(16_usize, 8_usize), field(8_usize, 8_usize)), Ok(41_usize));
        assert_eq!(hybrid_dead(field(1_usize, 1_usize), field(8_usize, 4_usize), field(8_usize, 8_usize)), Ok(29_usize));
    }

    fn onepass() -> OnePassConstructionCertificate {
        OnePassConstructionCertificate {
            nfa_states: 2_usize, rows_upper_bound: 3_usize, stride: 4_usize,
            scaffold_bytes: 3_usize, queue_peak_bytes: 5_usize, nfa_map_bytes: 7_usize,
            stack_peak_bytes: 11_usize, seen_bytes: 13_usize, table_peak_bytes: 17_usize,
            starts_peak_bytes: 19_usize, remapper_peak_bytes: 23_usize,
            shrink_overlap_bytes: 29_usize, retained_tables_upper_bytes: 29_usize,
            peak_requested_bytes: 127_usize, failure_prefix_peak_bytes: 127_usize,
            closure_visits_upper_bound: 31_usize, byte_visits_upper_bound: 37_usize,
            remap_visits_upper_bound: 41_usize, work_upper_bound: 139_usize,
        }
    }

    #[test]
    fn onepass_i_is_named_scratch_not_peak_minus_c_or_a_second_shrink_charge() {
        let certificate = onepass();
        assert_eq!(onepass_scratch(&certificate, 29_usize), Ok(98_usize));
        assert_eq!(onepass_scratch(&certificate, 28_usize), Err(Refusal::RetainedCoverage));
        let changed = OnePassConstructionCertificate {
            shrink_overlap_bytes: 37_usize, retained_tables_upper_bytes: 37_usize,
            peak_requested_bytes: 135_usize, ..certificate
        };
        assert_eq!(onepass_scratch(&changed, 37_usize), Ok(98_usize));
        let changed = OnePassConstructionCertificate { table_peak_bytes: 18_usize, ..certificate };
        assert_eq!(onepass_scratch(&changed, 29_usize), Ok(99_usize));
        let changed = OnePassConstructionCertificate {
            failure_prefix_peak_bytes: 128_usize, ..certificate
        };
        assert_eq!(onepass_scratch(&changed, 29_usize), Err(Refusal::RetainedCoverage));
    }

    #[test]
    fn five_distinct_owner_controls_keep_equal_digits_and_complete_fixed_scope_work() {
        let (workspace, syntax) = source_locations();
        let facts = layouts();
        let config = config();
        let enabled = features();
        let mut attempts = 0_usize;
        for (owner, pattern) in [
            ("fake_digits_a", "^[0-9]+$"), ("fake_digits_b", "^[0-9]+$"),
            ("fake_decimal", r"^[0-9]+(\.[0-9]+)?$"),
            ("fake_suffix", r"\.[a-zA-Z]{2,3}$"), ("fake_hostname", HOSTNAME_SOURCE),
        ] {
            let program = program(pattern, &facts);
            let certificate = evaluate(&Inputs {
                owner, pattern, config: &config, program: &program, layouts: &facts,
                sources: Sources { workspace: &workspace, syntax: &syntax,
                    rust_release: "1.98.1", features: &enabled },
            }).unwrap();
            assert_eq!(certificate.owner, owner);
            assert!(certificate.construction_only_bytes > 0_usize);
            assert!(certificate.retained.total > 0_usize);
            assert_eq!(certificate.phase_scratch.len(), 13_usize);
            for stage in [Stage::Syntax, Stage::Literal, Stage::ForwardNfa, Stage::PikeVm,
                Stage::Backtrack, Stage::OnePass, Stage::ReverseNfa, Stage::HybridForward,
                Stage::HybridReverse, Stage::HybridPair, Stage::Publication, Stage::Cleanup,
                Stage::FailureCleanup]
            {
                assert!(certificate.phase_scratch.contains_key(&stage));
                assert!(certificate.phase_scratch[&stage] <= certificate.construction_only_bytes);
            }
            assert_eq!(certificate.failure_prefixes, [
                FailurePrefix::SyntaxReplacementAndCleanup, FailurePrefix::NfaMutationAndCleanup,
                FailurePrefix::OnePassRefusalAndCleanup, FailurePrefix::HybridCapacityAndCleanup,
            ]);
            attempts += usize::from(certificate.onepass_attempted);
            assert!(certificate.complete_work().unwrap() > 0_usize);
            if pattern == r"\.[a-zA-Z]{2,3}$" {
                assert_eq!(certificate.work[&Work::LiteralHir], Visits::Bound(5_usize));
                assert_eq!(certificate.work[&Work::LiteralRanges], Visits::Bound(2_usize));
                assert_eq!(certificate.work[&Work::LiteralSequences], Visits::Bound(32_usize));
                assert_eq!(certificate.work[&Work::LiteralCopies], Visits::Bound(8_usize));
                assert_eq!(certificate.work[&Work::LiteralTrie], Visits::Bound(12_usize));
                assert_eq!(certificate.work[&Work::LiteralSelection], Visits::Bound(8_usize));
            }
        }
        assert_eq!(attempts, 2_usize);
    }

    #[test]
    fn every_required_fact_refuses_before_source_io_and_no_unknown_pattern_is_zero() {
        let original = layouts();
        let config = config();
        let enabled = features();
        let path = Path::new("/fake-unread-source");
        for key in required_fact_keys() {
            let mut missing = original.clone();
            assert!(missing.remove(&key).is_some());
            let program = program("^[0-9]+$", &missing);
            assert_eq!(
                evaluate(&Inputs {
                    owner: "fake_owner", pattern: "^[0-9]+$", config: &config,
                    program: &program, layouts: &missing,
                    sources: Sources { workspace: path, syntax: path, rust_release: "1.98.1", features: &enabled },
                }).err(),
                Some(Refusal::Missing(key))
            );
        }
        let program = program("^unknown$", &original);
        assert_eq!(
            evaluate(&Inputs {
                owner: "fake_owner", pattern: "^unknown$", config: &config,
                program: &program, layouts: &original,
                sources: Sources { workspace: path, syntax: path, rust_release: "1.98.1", features: &enabled },
            }).err(),
            Some(Refusal::PatternScope)
        );
    }

    #[test]
    fn work_refuses_unbound_owners_and_checked_sum_overflow() {
        let mut certificate = Certificate {
            owner: "fake_work".to_owned(),
            retained: NativeProgramHeap {
                metadata_bytes: 0_usize, strategy_arc_bytes: 0_usize, nfa_bytes: 0_usize,
                onepass_tables_bytes: 0_usize, automatic_prefilter_bytes: 0_usize,
                unused_pool_bytes: 0_usize, total: 0_usize, static_inline_bytes: 0_usize,
                automatic_prefilter: AutomaticPrefilterHeap::ExcludedStartAnchored,
            },
            phase_scratch: BTreeMap::new(), construction_only_bytes: 0_usize,
            onepass_attempted: false,
            work: BTreeMap::from([(Work::Literal, Visits::Unbound("unqualified literal"))]),
            failure_prefixes: [FailurePrefix::SyntaxReplacementAndCleanup,
                FailurePrefix::NfaMutationAndCleanup, FailurePrefix::OnePassRefusalAndCleanup,
                FailurePrefix::HybridCapacityAndCleanup],
        };
        assert_eq!(certificate.complete_work(), Err(Refusal::UnboundWork("unqualified literal")));
        certificate.work = BTreeMap::from([
            (Work::Syntax, Visits::Bound(usize::MAX)), (Work::OuterVectors, Visits::Bound(1_usize)),
        ]);
        assert_eq!(certificate.complete_work(), Err(Refusal::Arithmetic));
        certificate.work.clear();
        assert_eq!(certificate.complete_work(), Err(Refusal::UnboundWork("missing source work owner")));
    }

    #[test]
    fn facts_must_match_root_premises_and_configuration_before_source_io() {
        let facts = layouts();
        let config = config();
        let enabled = features();
        let path = Path::new("/fake-unread-source");
        let mut root = program("^[0-9]+$", &facts);
        root.arc_header = field(8_usize, 4_usize);
        let inputs = Inputs {
            owner: "fake_owner", pattern: "^[0-9]+$", config: &config,
            program: &root, layouts: &facts,
            sources: Sources { workspace: path, syntax: path, rust_release: "1.98.1", features: &enabled },
        };
        assert_eq!(evaluate(&inputs).err(), Some(Refusal::Layout));
        let changed = config.clone().hybrid(false);
        let inputs = Inputs { config: &changed, ..inputs };
        assert_eq!(evaluate(&inputs).err(), Some(Refusal::Configuration));
        let root = program("^[0-9]+$", &facts);
        let mut missing_feature = enabled.clone();
        assert!(missing_feature.remove("syntax"));
        let inputs = Inputs {
            owner: "fake_owner", pattern: "^[0-9]+$", config: &config,
            program: &root, layouts: &facts,
            sources: Sources { workspace: path, syntax: path, rust_release: "1.98.1", features: &missing_feature },
        };
        assert_eq!(evaluate(&inputs).err(), Some(Refusal::Features));
        let inputs = Inputs {
            sources: Sources { workspace: path, syntax: path, rust_release: "unqualified", features: &enabled },
            ..inputs
        };
        assert_eq!(evaluate(&inputs).err(), Some(Refusal::Source("Rust collection paths")));
    }

    #[test]
    fn source_recipe_seals_without_layouts_and_exposes_model_identities_only() {
        let (workspace, syntax) = source_locations();
        let config = config();
        let enabled = features();
        // No layout map, private payload facts or physical certificate exists yet.
        let recipe = SourceRecipe::from_source("fake_source_only", "^[0-9]+$", &config, Sources {
            workspace: &workspace, syntax: &syntax, rust_release: "1.98.1", features: &enabled,
        }).unwrap();
        assert_eq!(recipe.owner(), "fake_source_only");
        assert_eq!(recipe.pattern(), "^[0-9]+$");
        assert_eq!(recipe.required_facts(), &required_fact_keys());
        assert_eq!(recipe.phase_models().len(), 13_usize);
        assert_eq!(recipe.work_models().len(), 15_usize);
        // Physical projection is a separate call, with mandatory explicit facts.
        let facts = layouts();
        let root = program(recipe.pattern(), &facts);
        assert!(recipe.evaluate(&facts, &root).is_ok());
    }

    #[test]
    fn source_recipe_rejects_unknown_scope_features_release_and_unverified_source() {
        let config = config();
        let enabled = features();
        let path = Path::new("/fake-unread-source");
        let sources = || Sources {
            workspace: path, syntax: path, rust_release: "1.98.1", features: &enabled,
        };
        assert_eq!(SourceRecipe::from_source("fake_owner", "^unknown$", &config, sources()).err(),
            Some(Refusal::PatternScope));
        let mut unknown = enabled.clone();
        assert!(unknown.insert("perf-literal-inner".to_owned()));
        let sources = Sources { features: &unknown, ..sources() };
        assert_eq!(SourceRecipe::from_source("fake_owner", "^[0-9]+$", &config, sources).err(),
            Some(Refusal::Features));
        let sources = Sources {
            workspace: path, syntax: path, rust_release: "unqualified", features: &enabled,
        };
        assert_eq!(SourceRecipe::from_source("fake_owner", "^[0-9]+$", &config, sources).err(),
            Some(Refusal::Source("Rust collection paths")));
        let sources = Sources {
            workspace: path, syntax: path, rust_release: "1.98.1", features: &enabled,
        };
        assert_eq!(SourceRecipe::from_source("fake_owner", "^[0-9]+$", &config, sources).err(),
            Some(Refusal::Source("regex-automata source")));
    }

    #[test]
    fn explicit_request_overflow_refuses_instead_of_host_sized_or_zero_scratch() {
        assert_eq!(
            outer_vectors(field(0x4000_0000_usize, 8_usize), field(16_usize, 8_usize), field(4_usize, 4_usize)),
            Err(Refusal::Arithmetic)
        );
        assert_eq!(
            hybrid_dead(field(2_usize, 1_usize), field(16_usize, 8_usize), field(8_usize, 8_usize)),
            Err(Refusal::Layout)
        );
        assert_eq!(
            hybrid_dead(field(1_usize, 1_usize), field(0_usize, 8_usize), field(8_usize, 8_usize)),
            Err(Refusal::Layout)
        );
        assert_eq!(
            hybrid_dead(field(1_usize, 1_usize), field(0x8000_0000_usize, 8_usize), field(4_usize, 4_usize)),
            Err(Refusal::Arithmetic)
        );
    }
}

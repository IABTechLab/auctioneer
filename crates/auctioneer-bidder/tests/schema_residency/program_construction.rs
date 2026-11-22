//! Conditional single-program constructor recipe for the current pattern shapes.
//! Native AST/HIR/NFA values are structural inputs, never target layout evidence.
//! No physical fit, serving, machine-stack, instruction or recoverable-OOM claim.

use crate::{
    schema_build::schema_regex_vendor,
    schema_cache_bounds::forward_nfa,
    schema_formats::HOSTNAME_SOURCE,
    schema_heap_layout::{
        AllocationEnvelope, HeapLayout, LayoutError, NativeField, arc_layout_with_header,
    },
    schema_nfa_construction::{self, NfaConstructionCertificate},
    schema_nfa_heap::target,
    schema_onepass_construction::{self, OnePassConstructionCertificate},
    schema_program_heap::{self, NativeProgramHeap, ProgramRecipe, SourceRecipe as RetainedRecipe},
    schema_syntax_construction::{
        self, ConstructionError as SyntaxError, SyntaxConstructionCertificate,
    },
};
use regex_automata::{meta::Config, util::syntax};
use regex_syntax::hir::Hir;
use sha2::{Digest as _, Sha256};
use std::{
    any::type_name,
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read as _,
    path::{Path, PathBuf},
    sync::atomic::AtomicUsize,
};

use super::literal_construction::{self, KEYS as SUFFIX_KEYS};

#[cfg(test)]
pub(super) use tests::{layouts as hypothetical_layouts, program as hypothetical_program};

#[derive(Clone, Copy)]
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
    /// Seal source/model identities only. No layout projection or native
    /// constructor is performed until evaluate receives explicit target facts.
    pub(crate) fn from_source(
        owner: &str,
        pattern: &str,
        config: &Config,
        sources: Sources<'_>,
    ) -> Result<Self, Refusal> {
        require_selection(pattern, config, &sources)?;
        verify_sources(&sources)?;
        Ok(Self {
            owner: owner.to_owned(),
            pattern: pattern.to_owned(),
            config: config.clone(),
            workspace: sources.workspace.to_owned(),
            syntax: sources.syntax.to_owned(),
            rust_release: sources.rust_release.to_owned(),
            features: sources.features.clone(),
            required_facts: required_fact_keys(),
            phase_models: STAGES,
            work_models: WORK_OWNERS,
        })
    }

    pub(crate) fn owner(&self) -> &str {
        &self.owner
    }
    pub(crate) fn pattern(&self) -> &str {
        &self.pattern
    }
    pub(crate) fn required_facts(&self) -> &BTreeSet<String> {
        &self.required_facts
    }
    pub(crate) fn phase_models(&self) -> &[Stage] {
        &self.phase_models
    }
    pub(crate) fn work_models(&self) -> &[Work] {
        &self.work_models
    }

    /// Numbers require all matched target facts; work may still be Unbound.
    pub(crate) fn evaluate(
        &self,
        layouts: &BTreeMap<String, NativeField>,
        program: &ProgramRecipe<'_>,
    ) -> Result<Certificate, Refusal> {
        evaluate(&Inputs {
            owner: &self.owner,
            pattern: &self.pattern,
            config: &self.config,
            program,
            layouts,
            sources: Sources {
                workspace: &self.workspace,
                syntax: &self.syntax,
                rust_release: &self.rust_release,
                features: &self.features,
            },
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Stage {
    Syntax,
    Literal,
    ForwardNfa,
    PikeVm,
    Backtrack,
    OnePass,
    ReverseNfa,
    HybridForward,
    HybridReverse,
    HybridPair,
    Publication,
    Cleanup,
    FailureCleanup,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Work {
    Syntax,
    OuterVectors,
    ForwardNfa,
    ReverseNfa,
    OnePass,
    Literal,
    WrapperScaffolds,
    HybridChecks,
    Publication,
    LiteralHir,
    LiteralRanges,
    LiteralSequences,
    LiteralCopies,
    LiteralTrie,
    LiteralSelection,
}

const STAGES: [Stage; 13_usize] = [
    Stage::Syntax,
    Stage::Literal,
    Stage::ForwardNfa,
    Stage::PikeVm,
    Stage::Backtrack,
    Stage::OnePass,
    Stage::ReverseNfa,
    Stage::HybridForward,
    Stage::HybridReverse,
    Stage::HybridPair,
    Stage::Publication,
    Stage::Cleanup,
    Stage::FailureCleanup,
];

const WORK_OWNERS: [Work; 15_usize] = [
    Work::Syntax,
    Work::OuterVectors,
    Work::ForwardNfa,
    Work::ReverseNfa,
    Work::OnePass,
    Work::Literal,
    Work::WrapperScaffolds,
    Work::HybridChecks,
    Work::Publication,
    Work::LiteralHir,
    Work::LiteralRanges,
    Work::LiteralSequences,
    Work::LiteralCopies,
    Work::LiteralTrie,
    Work::LiteralSelection,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Visits {
    /// A checked upper bound, not an exact dynamic execution count.
    Bound(usize),
    Unbound(&'static str),
}

/// Every prefix includes cleanup of its owned temporary storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailurePrefix {
    SyntaxReplacement,
    NfaMutation,
    OnePassRefusal,
    HybridCapacity,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
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
        let total = self
            .work
            .values()
            .try_fold(0_usize, |total, visits| match visits {
                Visits::Bound(value) => total.checked_add(*value).ok_or(Refusal::Arithmetic),
                Visits::Unbound(owner) => Err(Refusal::UnboundWork(owner)),
            })?;
        if [
            Work::Syntax,
            Work::OuterVectors,
            Work::ForwardNfa,
            Work::ReverseNfa,
            Work::OnePass,
            Work::Literal,
            Work::WrapperScaffolds,
            Work::HybridChecks,
            Work::Publication,
            Work::LiteralHir,
            Work::LiteralRanges,
            Work::LiteralSequences,
            Work::LiteralCopies,
            Work::LiteralTrie,
            Work::LiteralSelection,
        ]
        .into_iter()
        .any(|owner| !self.work.contains_key(&owner))
        {
            return Err(Refusal::UnboundWork("missing source work owner"));
        }
        Ok(total)
    }
}

pub(crate) fn required_fact_keys() -> BTreeSet<String> {
    schema_syntax_construction::PRIVATE_TYPES
        .into_iter()
        .chain(schema_syntax_construction::PAYLOAD_TYPES)
        .chain([schema_syntax_construction::ABI_POINTER])
        .chain(target::fact_keys())
        .chain(schema_nfa_construction::additional_layout_keys())
        .chain(schema_onepass_construction::fact_keys())
        .chain(SUFFIX_KEYS)
        .map(str::to_owned)
        .collect()
}

/// Conditional C/I only, not a source-completeness or physical qualification.
/// Check selection and all target premises before opening dependency sources.
pub(crate) fn evaluate(input: &Inputs<'_>) -> Result<Certificate, Refusal> {
    require_selection(input.pattern, input.config, &input.sources)?;
    require_layouts(input)?;
    verify_sources(&input.sources)?;
    let retained = RetainedRecipe::from_pattern(input.pattern, input.config)
        .map_err(|error| program_error(&error))?
        .evaluate(input.program)
        .map_err(|error| program_error(&error))?;
    let models = Models::project(input)?;
    models.require_coverage(&retained)?;
    let phase_scratch = models.phases(input, &retained)?;
    let construction_only_bytes = phase_scratch
        .values()
        .copied()
        .max()
        .ok_or(Refusal::RetainedCoverage)?;
    let work = models.work(input, &retained)?;
    let certificate = Certificate {
        owner: input.owner.to_owned(),
        retained,
        phase_scratch,
        construction_only_bytes,
        onepass_attempted: models.onepass.is_some(),
        work,
        failure_prefixes: [
            FailurePrefix::SyntaxReplacement,
            FailurePrefix::NfaMutation,
            FailurePrefix::OnePassRefusal,
            FailurePrefix::HybridCapacity,
        ],
    };
    certificate.complete_work()?;
    Ok(certificate)
}

fn require_selection(pattern: &str, config: &Config, sources: &Sources<'_>) -> Result<(), Refusal> {
    schema_program_heap::require_configuration(config).map_err(|_error| Refusal::Configuration)?;
    // The other three Config fields must retain the pinned defaults too.
    let defaults = Config::new();
    if !config.get_backtrack()
        || config.get_dfa_size_limit() != defaults.get_dfa_size_limit()
        || config.get_dfa_state_limit() != defaults.get_dfa_state_limit()
    {
        return Err(Refusal::Configuration);
    }
    if !matches!(
        pattern,
        "^[0-9]+$" | r"^[0-9]+(\.[0-9]+)?$" | r"\.[a-zA-Z]{2,3}$"
    ) && pattern != HOSTNAME_SOURCE
    {
        return Err(Refusal::PatternScope);
    }
    let expected: BTreeSet<String> = schema_regex_vendor::FEATURES
        .into_iter()
        .map(str::to_owned)
        .collect();
    if sources.features != &expected {
        return Err(Refusal::Features);
    }
    if sources.rust_release != "1.98.1" {
        return Err(Refusal::Source("Rust collection paths"));
    }
    Ok(())
}

fn verify_sources(sources: &Sources<'_>) -> Result<(), Refusal> {
    // schema_vendor owns jsonschema, not this regex engine.
    schema_regex_vendor::verify(sources.workspace)
        .map_err(|_error| Refusal::Source("regex-automata source"))?;
    schema_syntax_construction::verify_source(sources.syntax)
        .map_err(|error| syntax_error(&error))?;
    // literal::is_poisonous calls rank; the syntax constructor's pins do not
    // cover this additional consumer dependency.
    let path = sources.syntax.join("src/rank.rs");
    let metadata =
        fs::symlink_metadata(&path).map_err(|_error| Refusal::Source("regex-syntax rank"))?;
    if !metadata.is_file() || metadata.len() > 0x0010_0000_u64 {
        return Err(Refusal::Source("regex-syntax rank"));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(0x0010_0001_u64).read_to_end(&mut bytes))
        .map_err(|_error| Refusal::Source("regex-syntax rank"))?;
    verify_rank_bytes(&bytes)
}

fn verify_rank_bytes(bytes: &[u8]) -> Result<(), Refusal> {
    if hex::encode(Sha256::digest(bytes))
        != "ff3d58b0cc5ffa69e2e8c56fc7d9ef41dd399d59a639a253a51551b858cb5bbd"
    {
        return Err(Refusal::Source("regex-syntax rank"));
    }
    Ok(())
}

fn require_layouts(input: &Inputs<'_>) -> Result<(), Refusal> {
    for key in required_fact_keys() {
        input
            .layouts
            .get(&key)
            .ok_or(Refusal::Missing(key))?
            .validate()?;
    }
    let word = input.layouts[target::ABI_POINTER];
    require_word(word)?;
    if input.program.pattern != input.pattern
        || input.program.onepass_word != word
        || input.layouts[type_name::<AtomicUsize>()] != word
        || input.program.arc_header.bytes != mul(word.bytes, 2_usize)?
        || input.program.arc_header.alignment != word.alignment
        || input.program.metadata.props_payload
            != input.layouts[schema_nfa_construction::HIR_PROPERTIES]
        || input.program.memchr_payload
            != (NativeField {
                bytes: 1_usize,
                alignment: 1_usize,
            })
    {
        return Err(Refusal::Layout);
    }
    for key in target::fact_keys() {
        if input.program.nfa_facts.get(key) != input.layouts.get(key) {
            return Err(Refusal::Layout);
        }
    }
    // Supplied root payloads are mandatory premises, including dormant tables.
    for field in input
        .program
        .metadata
        .regex_fields
        .into_iter()
        .chain(input.program.metadata.info_fields)
        .chain(input.program.pool.fields)
        .chain([
            input.program.metadata.props_payload,
            input.program.metadata.static_inline,
            input.program.arc_header,
            input.program.strategy_payload,
            input.program.memchr_payload,
            input.program.pool.shard,
            input.program.pool.closure,
        ])
    {
        field.validate()?;
        request(field.bytes, field.alignment, word)?;
    }
    Ok(())
}

struct Models {
    syntax: SyntaxConstructionCertificate,
    forward: NfaConstructionCertificate,
    reverse: NfaConstructionCertificate,
    onepass: Option<OnePassConstructionCertificate>,
}

impl Models {
    fn project(input: &Inputs<'_>) -> Result<Self, Refusal> {
        let syntax_certificate = SyntaxConstructionCertificate::from_pattern(
            input.pattern,
            input.layouts,
            input.sources.syntax,
        )
        .map_err(|error| syntax_error(&error))?;
        let hir: Hir = syntax::parse_with(input.pattern, &syntax::Config::default().utf8(true))
            .map_err(|_error| Refusal::Source("selected HIR"))?;
        let forward = NfaConstructionCertificate::from_hir_with_layouts(&hir, false, input.layouts)
            .map_err(|error| nfa_error(&error))?;
        let reverse = NfaConstructionCertificate::from_hir_with_layouts(&hir, true, input.layouts)
            .map_err(|error| nfa_error(&error))?;
        // wrappers.rs:361-393: only decimal and hostname have an incentive.
        // No selected pattern has a Unicode word boundary.
        let onepass = if hir.properties().explicit_captures_len() == 0_usize {
            None
        } else {
            let nfa = forward_nfa(input.pattern)
                .map_err(|_error| Refusal::Source("forward NFA topology"))?;
            Some(
                OnePassConstructionCertificate::from_nfa_with_layouts(&nfa, input.layouts)
                    .map_err(|error| onepass_error(&error))?,
            )
        };
        Ok(Self {
            syntax: syntax_certificate,
            forward,
            reverse,
            onepass,
        })
    }

    fn require_coverage(&self, retained: &NativeProgramHeap) -> Result<(), Refusal> {
        if self.syntax.failure_prefix_peak_bytes != self.syntax.peak_requested_bytes
            || self.forward.failure_prefix_peak_bytes != self.forward.peak_requested_bytes
            || self.reverse.failure_prefix_peak_bytes != self.reverse.peak_requested_bytes
            || add(self.forward.final_nfa_bytes, self.reverse.final_nfa_bytes)? > retained.nfa_bytes
        {
            return Err(Refusal::RetainedCoverage);
        }
        if let Some(certificate) = &self.onepass {
            onepass_scratch(certificate, retained.onepass_tables_bytes)?;
        }
        Ok(())
    }

    fn phases(
        &self,
        input: &Inputs<'_>,
        retained: &NativeProgramHeap,
    ) -> Result<BTreeMap<Stage, usize>, Refusal> {
        let facts = input.layouts;
        let word = facts[target::ABI_POINTER];
        // Keep the full syntax reserve, including AST/HIR drop replacements,
        // live until the caller drops both outer Vecs. Never reclassify it as C.
        let persistent = add(
            self.syntax.peak_requested_bytes,
            outer_vectors(
                facts["regex_syntax::ast::Ast"],
                facts["regex_syntax::hir::Hir"],
                word,
            )?,
        )?;
        let trie = append(facts[schema_nfa_construction::TRIE_STATE], 2_usize, word)?;
        let wrapper = add(trie.retained_bytes, trie.old_growth_bytes)?;
        let dead = hybrid_dead(input.program.memchr_payload, input.program.arc_header, word)?;
        let hybrid = add(wrapper, dead)?;
        let literal = if input.pattern == literal_construction::PATTERN {
            literal_construction::suffix(input.pattern, facts)
                .map_err(|error| literal_error(&error))?
                .peak
        } else {
            0_usize // strategy::new skips prefix extraction when start-anchored.
        };
        let onepass = self.onepass.as_ref().map_or(Ok(0_usize), |certificate| {
            onepass_scratch(certificate, retained.onepass_tables_bytes)
        })?;
        let extras = [
            (Stage::Syntax, 0_usize),
            (Stage::Literal, literal),
            (Stage::ForwardNfa, self.forward.initialization_scratch_bytes),
            (Stage::PikeVm, wrapper),
            (Stage::Backtrack, wrapper),
            (Stage::OnePass, onepass),
            (Stage::ReverseNfa, self.reverse.initialization_scratch_bytes),
            (Stage::HybridForward, hybrid),
            (Stage::HybridReverse, hybrid),
            (Stage::HybridPair, wrapper),
            (Stage::Publication, 0_usize),
            (Stage::Cleanup, 0_usize),
        ];
        // Every failure prefix has the same monotonic high-waters, not a
        // retained-only .ok()/None argument. Admitted parser/name errors are
        // unreachable; NFA size/state, onepass static-message and hybrid
        // capacity errors are inline scalar/static values, with no error Box.
        phase_envelopes(persistent, extras)
    }

    fn work(
        &self,
        input: &Inputs<'_>,
        retained: &NativeProgramHeap,
    ) -> Result<BTreeMap<Work, Visits>, Refusal> {
        // Units: source collection-record/byte visits, including buffer
        // initialization, relocation and cleanup. Excludes scalar moves,
        // atomics, allocator internals, machine operations and elapsed time.
        let mut work = WORK_OWNERS
            .into_iter()
            .map(|owner| (owner, Visits::Unbound("missing constructor work owner")))
            .collect::<BTreeMap<_, _>>();
        work.insert(Work::Syntax, Visits::Bound(self.syntax.work_upper_bound));
        // regex.rs:3611-3631,3741: AST push, HIR push, &Hir collect,
        // outer AST drop and outer HIR drop. Each buffer has one element;
        // no old allocation relocates. Child drops belong to syntax.
        work.insert(Work::OuterVectors, Visits::Bound(sum(&[1_usize; 5_usize])?));
        work.insert(
            Work::ForwardNfa,
            Visits::Bound(self.forward.work_upper_bound),
        );
        work.insert(
            Work::ReverseNfa,
            Visits::Bound(self.reverse.work_upper_bound),
        );
        if let Some(certificate) = &self.onepass {
            work.insert(Work::OnePass, Visits::Bound(certificate.work_upper_bound));
        } else {
            work.insert(Work::OnePass, Visits::Bound(0_usize));
        }
        work.insert(
            Work::WrapperScaffolds,
            Visits::Bound(wrapper_work(input.layouts)?),
        );
        work.insert(Work::HybridChecks, Visits::Bound(hybrid_work()?));
        work.insert(
            Work::Publication,
            Visits::Bound(publication_work([
                retained.metadata_bytes,
                retained.strategy_arc_bytes,
                retained.unused_pool_bytes,
            ])?),
        );
        if input.pattern == literal_construction::PATTERN {
            work.extend(suffix_work(input.layouts)?);
        } else {
            // All other admitted patterns are start-anchored: strategy::new
            // skips extraction, so each literal subledger is positively absent.
            for owner in [
                Work::LiteralHir,
                Work::LiteralRanges,
                Work::LiteralSequences,
                Work::LiteralCopies,
                Work::LiteralTrie,
                Work::LiteralSelection,
            ] {
                work.insert(owner, Visits::Bound(0_usize));
            }
        }
        // Literal is the residual owner, not a duplicate sum of its six
        // subledgers. All other zeroes mean positively excluded source paths.
        work.insert(Work::Literal, Visits::Bound(0_usize));
        Ok(work)
    }
}

fn onepass_scratch(
    certificate: &OnePassConstructionCertificate,
    covered_by_c: usize,
) -> Result<usize, Refusal> {
    let scratch = sum(&[
        certificate.scaffold_bytes,
        certificate.queue_peak_bytes,
        certificate.nfa_map_bytes,
        certificate.stack_peak_bytes,
        certificate.seen_bytes,
        certificate.table_peak_bytes,
        certificate.starts_peak_bytes,
        certificate.remapper_peak_bytes,
    ])?;
    let envelope = add(scratch, certificate.shrink_overlap_bytes)?;
    if certificate.retained_tables_upper_bytes > covered_by_c
        || certificate.shrink_overlap_bytes != certificate.retained_tables_upper_bytes
        || certificate.peak_requested_bytes > envelope
        || certificate.failure_prefix_peak_bytes > envelope
    {
        return Err(Refusal::RetainedCoverage);
    }
    Ok(scratch)
}

fn phase_envelopes(
    persistent: usize,
    extras: [(Stage, usize); 12_usize],
) -> Result<BTreeMap<Stage, usize>, Refusal> {
    let maximum = extras
        .iter()
        .map(|entry| entry.1)
        .max()
        .ok_or(Refusal::RetainedCoverage)?;
    let mut phases = BTreeMap::new();
    for (stage, scratch) in extras.into_iter().chain([(Stage::FailureCleanup, maximum)]) {
        if phases.insert(stage, add(persistent, scratch)?).is_some() {
            return Err(Refusal::RetainedCoverage);
        }
    }
    if STAGES.into_iter().any(|stage| !phases.contains_key(&stage)) {
        return Err(Refusal::RetainedCoverage);
    }
    Ok(phases)
}

fn outer_vectors(ast: NativeField, hir: NativeField, word: NativeField) -> Result<usize, Refusal> {
    let ast_buffer = append(ast, 1_usize, word)?;
    let hir_buffer = append(hir, 1_usize, word)?;
    // Iterator::collect over a mapped slice is trusted length: exactly one &Hir.
    let references = request(word.bytes, word.alignment, word)?;
    sum(&[
        ast_buffer.retained_bytes,
        ast_buffer.old_growth_bytes,
        hir_buffer.retained_bytes,
        hir_buffer.old_growth_bytes,
        references,
    ])
}

fn hybrid_dead(
    byte: NativeField,
    header: NativeField,
    word: NativeField,
) -> Result<usize, Refusal> {
    if byte
        != (NativeField {
            bytes: 1_usize,
            alignment: 1_usize,
        })
    {
        return Err(Refusal::Layout);
    }
    header.validate()?;
    let heap = require_word(word)?;
    // state.rs:130,199-201,287-288: empty Vec extends a nine-byte slice,
    // then Arc::from(&[u8]) copies it; both requests coexist until return.
    let buffer = request(
        heap.fresh_resize_vector(byte.bytes, 9_usize)?,
        byte.alignment,
        word,
    )?;
    let payload = NativeField {
        bytes: 9_usize,
        alignment: byte.alignment,
    };
    let arc = arc_layout_with_header(payload, header)?;
    add(buffer, request(arc.size(), arc.align(), word)?)
}

fn wrapper_work(facts: &BTreeMap<String, NativeField>) -> Result<usize, Refusal> {
    let field = facts[schema_nfa_construction::TRIE_STATE];
    let copied = relocated_records(field, 2_usize, facts[target::ABI_POINTER])?;
    // PikeVM, backtrack, hybrid forward, hybrid reverse, hybrid pair. Their
    // build_from_nfa/build_from_dfas paths do not call the eager Compiler.
    // Each fresh trie: one constructor, two state writes, two state drops.
    // Onepass's eager trie is already inside its component certificate.
    mul(5_usize, add(sum(&[1_usize, 2_usize, 2_usize])?, copied)?)
}

fn hybrid_work() -> Result<usize, Refusal> {
    // dfa.rs:3832-3894,4072-4128,4335-4391; alphabet.rs:722-737;
    // start.rs:216-251; determinize/state.rs:130,199-201,287-288.
    // Neither config clone nor hybrid::regex::build_from_dfas rebuilds these
    // buffers. Quit is empty, so add_set and Unicode quit filling are absent.
    let per_dfa = sum(&[
        32_usize, // quit ByteSet initialization (two u128s)
        32_usize, // ByteClassSet clone
        2_usize,  // quit is_empty compares two u128 records
        256_usize, 256_usize, // class zeroing and class-building loop
        256_usize, 3_usize, // start-map zeroing and three fixed writes
        10_usize, 26_usize, 26_usize, // start-map ASCII loops
        9_usize, 9_usize, // dead Vec initialization and Arc slice copy
    ])?;
    mul(2_usize, per_dfa)
}

fn publication_work(extents: [usize; 3_usize]) -> Result<usize, Refusal> {
    // Nine allocation constructors: five metadata, strategy Arc, closure Box,
    // pool Box, shard Vec. Three iterations: props clone, Properties::union,
    // one shard. Scalar-only Properties boxes are cloned/unioned once.
    // No growth of these one-element/exact buffers. Two byte extents cover
    // initialization/copy and cleanup. Extents exclude shared NFAs, onepass
    // tables and prefilter children, already owned by other work ledgers.
    sum(&[9_usize, 3_usize, mul(2_usize, sum(&extents)?)?])
}

fn suffix_sequence_terms() -> [(&'static str, usize); 18_usize] {
    // hir/literal.rs:171-198,395-410,448-497,559-574,959-984,
    // 1140-1175,1219-1235,1489-1495,1554-1600,1629-1654,1835-2010.
    // Three cross calls, but only the first has a finite Cartesian pair.
    // All dedup inputs have len <= 1; infinite clones have no records;
    // keep_first_bytes(100) does not shrink the sole one-byte literal.
    [
        ("concat iterations including stop", 3_usize),
        ("repeat iterations including stop", 2_usize),
        ("concat inexact predicates", 3_usize),
        ("repeat inexact predicate", 1_usize),
        ("literal and two cross length enforcements", 3_usize),
        ("two infinite-cross min-length scans", 2_usize),
        ("infinite-cross make-inexact", 1_usize),
        ("Cartesian outer record", 1_usize),
        ("finite Cartesian pair", 1_usize),
        ("optimizer min-length", 1_usize),
        ("optimizer common-prefix base", 1_usize),
        ("optimizer exactness probes", 2_usize),
        ("optimizer ATTEMPTS first row", 1_usize),
        ("optimizer poison predicate", 1_usize),
        ("singleton record initializations", 3_usize),
        ("cross result push", 1_usize),
        ("outer union transfer", 1_usize),
        ("replaced, drained and final literal drops", 4_usize),
    ]
}

fn suffix_audit(facts: &BTreeMap<String, NativeField>) -> Result<BTreeMap<Work, usize>, Refusal> {
    let word = facts[target::ABI_POINTER];
    let state_copies = relocated_records(facts[SUFFIX_KEYS[1]], 2_usize, word)?;
    let slot_copies = relocated_records(facts[SUFFIX_KEYS[2]], 2_usize, word)?;
    let sequence = suffix_sequence_terms()
        .into_iter()
        .try_fold(0_usize, |total, term| add(total, term.1))?;
    // Trie: retain predicate, inserted byte, two states, two match slots,
    // one transition, two state drops; at most one relocation per outer Vec.
    let trie = sum(&[
        1_usize,
        1_usize,
        2_usize,
        2_usize,
        1_usize,
        2_usize,
        state_copies,
        slot_copies,
    ])?;
    Ok(BTreeMap::from([
        // Four dispatches (concat, dot, repetition, class) plus outer HIR.
        (Work::LiteralHir, add(4_usize, 1_usize)?),
        (Work::LiteralRanges, 2_usize),
        (Work::LiteralSequences, sequence),
        // bytes.to_vec and the one nonempty Literal::extend. All other byte
        // extends are empty; no byte Vec grows or shrinks after its first fill.
        (Work::LiteralCopies, add(1_usize, 1_usize)?),
        (Work::LiteralTrie, trie),
        // Pre exactness, Choice's empty-needle predicate, max_needle_len's
        // iteration, Memchr needle and selected byte, then Arc construction
        // and its payload byte (prefilter/mod.rs:250-252,270; memchr.rs:18-24).
        (Work::LiteralSelection, sum(&[1_usize; 7_usize])?),
    ]))
}

fn suffix_work(facts: &BTreeMap<String, NativeField>) -> Result<BTreeMap<Work, Visits>, Refusal> {
    let audit = suffix_audit(facts)?;
    // Preserve the original controls as upper bounds, NOT invented exact
    // counts. The independent audit above must fit each bound separately.
    // In particular byte copies are two, not eight; trie visits are nine
    // plus target-layout-dependent relocations, not twelve unproved visits.
    let bounds = [
        (Work::LiteralHir, 5_usize),
        (Work::LiteralRanges, 2_usize),
        (Work::LiteralSequences, 32_usize),
        (Work::LiteralCopies, 8_usize),
        (Work::LiteralTrie, 12_usize),
        (Work::LiteralSelection, 8_usize),
    ];
    let mut work = BTreeMap::new();
    for (owner, bound) in bounds {
        if audit.get(&owner).is_none_or(|visits| *visits > bound) {
            return Err(Refusal::UnboundWork("suffix source ledger"));
        }
        work.insert(owner, Visits::Bound(bound));
    }
    Ok(work)
}

fn relocated_records(
    field: NativeField,
    count: usize,
    word: NativeField,
) -> Result<usize, Refusal> {
    let growth = append(field, count, word)?;
    growth
        .old_growth_bytes
        .checked_div(field.bytes)
        .ok_or(Refusal::Arithmetic)
}

fn append(
    field: NativeField,
    count: usize,
    word: NativeField,
) -> Result<AllocationEnvelope, Refusal> {
    field.validate()?;
    let heap = require_word(word)?;
    let allocation = heap.append_vector(field.bytes, count)?;
    request(allocation.retained_bytes, field.alignment, word)?;
    request(allocation.old_growth_bytes, field.alignment, word)?;
    Ok(allocation)
}

fn require_word(word: NativeField) -> Result<HeapLayout, Refusal> {
    word.validate()?;
    if word.bytes != word.alignment {
        return Err(Refusal::Layout);
    }
    HeapLayout::new(word.bytes, word.bytes).map_err(Refusal::from)
}

fn request(bytes: usize, alignment: usize, word: NativeField) -> Result<usize, Refusal> {
    require_word(word)?;
    if !alignment.is_power_of_two() {
        return Err(Refusal::Layout);
    }
    let maximum = match word.bytes {
        4_usize => usize::try_from(i32::MAX),
        8_usize => usize::try_from(i64::MAX),
        _ => return Err(Refusal::Layout),
    }
    .map_err(|_error| Refusal::Arithmetic)?;
    let padding = alignment.checked_sub(1_usize).ok_or(Refusal::Arithmetic)?;
    let limit = maximum
        .checked_sub(padding)
        .ok_or(Refusal::Arithmetic)?;
    if bytes > limit {
        return Err(Refusal::Arithmetic);
    }
    Ok(bytes)
}

impl From<LayoutError> for Refusal {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Shape => Self::Layout,
            LayoutError::Arithmetic => Self::Arithmetic,
        }
    }
}

fn syntax_error(error: &SyntaxError) -> Refusal {
    match error {
        SyntaxError::Arithmetic => Refusal::Arithmetic,
        SyntaxError::Layout => Refusal::Layout,
        SyntaxError::Source(owner)
        | SyntaxError::Scope(owner)
        | SyntaxError::UnboundOwner(owner) => Refusal::Source(owner),
        SyntaxError::Parse => Refusal::Source("syntax parser"),
    }
}

fn nfa_error(error: &schema_nfa_construction::ConstructionError) -> Refusal {
    use schema_nfa_construction::ConstructionError;
    match error {
        ConstructionError::Arithmetic => Refusal::Arithmetic,
        ConstructionError::Layout => Refusal::Layout,
        ConstructionError::UnboundOwner(owner) => Refusal::Source(owner),
        ConstructionError::Scope | ConstructionError::Shape | ConstructionError::Compilation => {
            Refusal::Source("NFA construction")
        }
    }
}

fn onepass_error(error: &schema_onepass_construction::ConstructionError) -> Refusal {
    use schema_onepass_construction::ConstructionError;
    match error {
        ConstructionError::Arithmetic => Refusal::Arithmetic,
        ConstructionError::Layout => Refusal::Layout,
        ConstructionError::MissingType(key) => Refusal::Missing((*key).to_owned()),
        ConstructionError::Scope => Refusal::Source("onepass construction"),
    }
}

fn program_error(error: &schema_program_heap::ProgramHeapError) -> Refusal {
    use schema_program_heap::ProgramHeapError;
    match error {
        ProgramHeapError::Arithmetic => Refusal::Arithmetic,
        ProgramHeapError::Configuration => Refusal::Configuration,
        ProgramHeapError::Layout | ProgramHeapError::RootLayout | ProgramHeapError::Shape => {
            Refusal::Layout
        }
        ProgramHeapError::Compilation
        | ProgramHeapError::Metadata
        | ProgramHeapError::Nfa
        | ProgramHeapError::OnePass
        | ProgramHeapError::AutomaticPrefilter(_)
        | ProgramHeapError::Pool => Refusal::RetainedCoverage,
    }
}

fn literal_error(error: &literal_construction::Refusal) -> Refusal {
    match error {
        literal_construction::Refusal::Missing(key) => Refusal::Missing((*key).to_owned()),
        literal_construction::Refusal::Layout => Refusal::Layout,
        literal_construction::Refusal::Arithmetic => Refusal::Arithmetic,
        literal_construction::Refusal::Scope => Refusal::PatternScope,
    }
}

fn add(left: usize, right: usize) -> Result<usize, Refusal> {
    left.checked_add(right).ok_or(Refusal::Arithmetic)
}

fn mul(value: usize, count: usize) -> Result<usize, Refusal> {
    value.checked_mul(count).ok_or(Refusal::Arithmetic)
}

fn sum(terms: &[usize]) -> Result<usize, Refusal> {
    terms
        .iter()
        .try_fold(0_usize, |total, term| add(total, *term))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        schema_formats::HOSTNAME_SOURCE, schema_literal_heap::AutomaticPrefilterHeap,
        schema_metadata_heap::MetadataLayout, schema_nfa_construction, schema_nfa_heap::target,
        schema_onepass_construction, schema_pool_heap::PoolLayout,
    };
    use regex_automata::{
        MatchKind,
        nfa::thompson::{State, Transition, WhichCaptures},
        util::primitives::{SmallIndex, StateID},
    };
    use regex_syntax::hir::ClassBytesRange;
    use std::{any::type_name, env, fs, path::PathBuf, sync::atomic::AtomicUsize};

    fn field(bytes: usize, alignment: usize) -> NativeField {
        NativeField { bytes, alignment }
    }

    fn config() -> Config {
        Config::new()
            .match_kind(MatchKind::LeftmostFirst)
            .utf8_empty(true)
            .which_captures(WhichCaptures::All)
            .byte_classes(true)
            .line_terminator(b'\n')
            .nfa_size_limit(Some(0x0001_0000_usize))
            .hybrid_cache_capacity(0x0001_0000_usize)
            .onepass_size_limit(Some(0x0010_0000_usize))
            .pool_capacity(1_usize)
            .hybrid(true)
            .dfa(false)
            .onepass(true)
            .auto_prefilter(true)
    }

    // Synthetic layout assumptions only; none is a compiler/guest observation.
    pub(crate) fn layouts() -> BTreeMap<String, NativeField> {
        let mut facts = required_fact_keys()
            .into_iter()
            .map(|key| (key, field(64_usize, 8_usize)))
            .collect::<BTreeMap<_, _>>();
        facts.insert(target::ABI_POINTER.into(), field(8_usize, 8_usize));
        facts.insert(type_name::<AtomicUsize>().into(), field(8_usize, 8_usize));
        facts.insert(type_name::<State>().into(), field(32_usize, 8_usize));
        facts.insert(type_name::<Transition>().into(), field(8_usize, 4_usize));
        facts.insert(type_name::<StateID>().into(), field(4_usize, 4_usize));
        facts.insert(
            type_name::<(SmallIndex, SmallIndex)>().into(),
            field(8_usize, 4_usize),
        );
        facts.insert(type_name::<bool>().into(), field(1_usize, 1_usize));
        facts.insert(
            type_name::<ClassBytesRange>().into(),
            field(2_usize, 1_usize),
        );
        facts.insert(
            schema_nfa_construction::BUILDER_STATE.into(),
            field(32_usize, 8_usize),
        );
        facts.insert(
            schema_nfa_construction::TRIE_STATE.into(),
            field(24_usize, 8_usize),
        );
        facts.insert(
            schema_nfa_construction::HIR_PROPERTIES.into(),
            field(80_usize, 8_usize),
        );
        facts.insert(
            schema_onepass_construction::STATE_ID.into(),
            field(4_usize, 4_usize),
        );
        facts.insert(
            schema_onepass_construction::TRANSITION.into(),
            field(8_usize, 8_usize),
        );
        facts.insert(
            schema_onepass_construction::EPSILONS.into(),
            field(8_usize, 8_usize),
        );
        facts.insert(
            schema_onepass_construction::BYTE_CLASSES.into(),
            field(256_usize, 8_usize),
        );
        facts
    }

    pub(crate) fn program<'a>(
        pattern: &'a str,
        facts: &'a BTreeMap<String, NativeField>,
    ) -> ProgramRecipe<'a> {
        ProgramRecipe {
            pattern,
            metadata: MetadataLayout {
                regex_fields: [field(16_usize, 8_usize), field(8_usize, 8_usize)],
                info_fields: [
                    field(40_usize, 8_usize),
                    field(24_usize, 8_usize),
                    field(8_usize, 8_usize),
                ],
                props_payload: field(80_usize, 8_usize),
                static_inline: field(64_usize, 8_usize),
            },
            arc_header: field(16_usize, 8_usize),
            strategy_payload: field(64_usize, 8_usize),
            nfa_facts: facts,
            onepass_word: field(8_usize, 8_usize),
            memchr_payload: field(1_usize, 1_usize),
            pool: PoolLayout {
                fields: [
                    field(16_usize, 8_usize),
                    field(24_usize, 8_usize),
                    field(8_usize, 8_usize),
                    field(40_usize, 8_usize),
                ],
                shard: field(40_usize, 8_usize),
                closure: field(16_usize, 8_usize),
            },
        }
    }

    fn features() -> BTreeSet<String> {
        schema_regex_vendor::FEATURES
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn source_locations() -> (PathBuf, PathBuf) {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let cargo = env::var_os("CARGO_HOME").map_or_else(
            || PathBuf::from(env::var_os("HOME").unwrap()).join(".cargo"),
            PathBuf::from,
        );
        let mut source = None;
        for entry in fs::read_dir(cargo.join("registry/src")).unwrap() {
            let path = entry.unwrap().path().join("regex-syntax-0.8.11");
            if path.is_dir() {
                source = Some(path.canonicalize().unwrap());
                break;
            }
        }
        (workspace, source.unwrap())
    }

    #[test]
    fn source_work_ledgers_have_independent_literal_totals() {
        let facts = layouts();
        assert_eq!(wrapper_work(&facts), Ok(25_usize));
        assert_eq!(hybrid_work(), Ok(1834_usize));
        assert_eq!(publication_work([5_usize, 7_usize, 11_usize]), Ok(58_usize));
        assert_eq!(publication_work([6_usize, 7_usize, 11_usize]), Ok(60_usize));
        assert_eq!(publication_work([5_usize, 8_usize, 11_usize]), Ok(60_usize));
        assert_eq!(publication_work([5_usize, 7_usize, 12_usize]), Ok(60_usize));
        assert_eq!(
            publication_work([usize::MAX, 0_usize, 0_usize]),
            Err(Refusal::Arithmetic)
        );
        let mut wide_trie = facts.clone();
        assert!(
            wide_trie
                .insert(
                    schema_nfa_construction::TRIE_STATE.to_owned(),
                    field(2048_usize, 8_usize),
                )
                .is_some()
        );
        assert_eq!(wrapper_work(&wide_trie), Ok(30_usize));
    }

    #[test]
    fn suffix_work_audits_real_calls_before_accepting_the_original_upper_bounds() {
        let facts = layouts();
        assert_eq!(
            suffix_audit(&facts).unwrap(),
            BTreeMap::from([
                (Work::LiteralHir, 5_usize),
                (Work::LiteralRanges, 2_usize),
                (Work::LiteralSequences, 32_usize),
                (Work::LiteralCopies, 2_usize),
                (Work::LiteralTrie, 9_usize),
                (Work::LiteralSelection, 7_usize),
            ])
        );
        let terms: BTreeMap<_, _> = suffix_sequence_terms().into_iter().collect();
        assert_eq!(terms["concat iterations including stop"], 3_usize);
        assert_eq!(terms["repeat iterations including stop"], 2_usize);
        assert_eq!(terms["finite Cartesian pair"], 1_usize);
        assert_eq!(terms["singleton record initializations"], 3_usize);
        assert_eq!(terms["cross result push"], 1_usize);
        assert_eq!(terms["outer union transfer"], 1_usize);
        assert_eq!(terms["replaced, drained and final literal drops"], 4_usize);
        assert_eq!(terms.values().sum::<usize>(), 32_usize);
        let mut relocated = facts.clone();
        assert!(
            relocated
                .insert(SUFFIX_KEYS[1].to_owned(), field(2048_usize, 8_usize))
                .is_some()
        );
        assert!(
            relocated
                .insert(SUFFIX_KEYS[2].to_owned(), field(4096_usize, 8_usize))
                .is_some()
        );
        let changed = suffix_audit(&relocated).unwrap();
        assert_eq!(changed[&Work::LiteralTrie], 11_usize);
        assert_eq!(changed[&Work::LiteralSequences], 32_usize);
        assert_eq!(changed[&Work::LiteralCopies], 2_usize);
        assert_eq!(
            suffix_work(&relocated).unwrap()[&Work::LiteralTrie],
            Visits::Bound(12_usize)
        );
    }

    fn serial_extras() -> [(Stage, usize); 12_usize] {
        [
            (Stage::Syntax, 0_usize),
            (Stage::Literal, 13_usize),
            (Stage::ForwardNfa, 17_usize),
            (Stage::PikeVm, 19_usize),
            (Stage::Backtrack, 23_usize),
            (Stage::OnePass, 29_usize),
            (Stage::ReverseNfa, 31_usize),
            (Stage::HybridForward, 37_usize),
            (Stage::HybridReverse, 41_usize),
            (Stage::HybridPair, 43_usize),
            (Stage::Publication, 0_usize),
            (Stage::Cleanup, 0_usize),
        ]
    }

    #[test]
    fn persistent_syntax_and_outer_storage_are_added_to_each_serial_failure_prefix() {
        let phases = phase_envelopes(100_usize, serial_extras()).unwrap();
        assert_eq!(
            phases,
            BTreeMap::from([
                (Stage::Syntax, 100_usize),
                (Stage::Literal, 113_usize),
                (Stage::ForwardNfa, 117_usize),
                (Stage::PikeVm, 119_usize),
                (Stage::Backtrack, 123_usize),
                (Stage::OnePass, 129_usize),
                (Stage::ReverseNfa, 131_usize),
                (Stage::HybridForward, 137_usize),
                (Stage::HybridReverse, 141_usize),
                (Stage::HybridPair, 143_usize),
                (Stage::Publication, 100_usize),
                (Stage::Cleanup, 100_usize),
                (Stage::FailureCleanup, 143_usize),
            ])
        );
        let mut pike_changed = serial_extras();
        pike_changed[3_usize] = (Stage::PikeVm, 47_usize);
        let raised = phase_envelopes(100_usize, pike_changed).unwrap();
        assert_eq!(raised[&Stage::FailureCleanup], 147_usize);
        assert_eq!(raised[&Stage::HybridPair], 143_usize);
        let persistent_changed = phase_envelopes(101_usize, serial_extras()).unwrap();
        assert_eq!(persistent_changed[&Stage::FailureCleanup], 144_usize);
        assert_eq!(persistent_changed[&Stage::Syntax], 101_usize);
        assert_eq!(
            phase_envelopes(usize::MAX, serial_extras()),
            Err(Refusal::Arithmetic)
        );
        let mut duplicate = serial_extras();
        duplicate[1_usize].0 = Stage::Syntax;
        assert_eq!(
            phase_envelopes(100_usize, duplicate),
            Err(Refusal::RetainedCoverage)
        );
    }

    #[test]
    fn every_named_onepass_scratch_term_is_independently_additive() {
        let original = onepass();
        for changed in [
            OnePassConstructionCertificate {
                scaffold_bytes: 4_usize,
                ..original
            },
            OnePassConstructionCertificate {
                queue_peak_bytes: 6_usize,
                ..original
            },
            OnePassConstructionCertificate {
                nfa_map_bytes: 8_usize,
                ..original
            },
            OnePassConstructionCertificate {
                stack_peak_bytes: 12_usize,
                ..original
            },
            OnePassConstructionCertificate {
                seen_bytes: 14_usize,
                ..original
            },
            OnePassConstructionCertificate {
                table_peak_bytes: 18_usize,
                ..original
            },
            OnePassConstructionCertificate {
                starts_peak_bytes: 20_usize,
                ..original
            },
            OnePassConstructionCertificate {
                remapper_peak_bytes: 24_usize,
                ..original
            },
        ] {
            assert_eq!(onepass_scratch(&changed, 29_usize), Ok(99_usize));
        }
        let overflow = OnePassConstructionCertificate {
            queue_peak_bytes: usize::MAX,
            ..original
        };
        assert_eq!(
            onepass_scratch(&overflow, 29_usize),
            Err(Refusal::Arithmetic)
        );
    }

    #[test]
    fn all_regex_features_are_required_and_no_instrumentation_feature_is_admitted() {
        let enabled = features();
        assert_eq!(
            enabled,
            BTreeSet::from(
                [
                    "alloc",
                    "dfa-onepass",
                    "dfa-search",
                    "hybrid",
                    "meta",
                    "nfa",
                    "nfa-backtrack",
                    "nfa-pikevm",
                    "nfa-thompson",
                    "perf",
                    "perf-inline",
                    "perf-literal",
                    "perf-literal-multisubstring",
                    "perf-literal-substring",
                    "std",
                    "syntax",
                    "unicode",
                    "unicode-age",
                    "unicode-bool",
                    "unicode-case",
                    "unicode-gencat",
                    "unicode-perl",
                    "unicode-script",
                    "unicode-segment",
                    "unicode-word-boundary",
                ]
                .map(str::to_owned)
            )
        );
        let configuration = config();
        let unread = Path::new("/fake-unread-source");
        for key in &enabled {
            let mut missing = enabled.clone();
            assert!(missing.remove(key));
            let sources = Sources {
                workspace: unread,
                syntax: unread,
                rust_release: "1.98.1",
                features: &missing,
            };
            assert_eq!(
                SourceRecipe::from_source(
                    "fake_feature_owner",
                    "^[0-9]+$",
                    &configuration,
                    sources,
                )
                .err(),
                Some(Refusal::Features)
            );
        }
        for extra in [
            "logging",
            "instrumentation",
            "perf-literal-inner",
            "fake_unknown",
        ] {
            let mut changed = enabled.clone();
            assert!(changed.insert(extra.to_owned()));
            let sources = Sources {
                workspace: unread,
                syntax: unread,
                rust_release: "1.98.1",
                features: &changed,
            };
            assert_eq!(
                SourceRecipe::from_source(
                    "fake_feature_owner",
                    "^[0-9]+$",
                    &configuration,
                    sources,
                )
                .err(),
                Some(Refusal::Features)
            );
        }
    }

    #[test]
    fn literal_rank_consumer_requires_its_own_exact_source_bytes() {
        let (_workspace, syntax_source) = source_locations();
        let original = fs::read(syntax_source.join("src/rank.rs")).unwrap();
        assert_eq!(verify_rank_bytes(&original), Ok(()));
        let mut changed = original.clone();
        changed.push(0_u8);
        assert_eq!(
            verify_rank_bytes(&changed),
            Err(Refusal::Source("regex-syntax rank"))
        );
        assert_eq!(
            verify_rank_bytes(b""),
            Err(Refusal::Source("regex-syntax rank"))
        );
    }

    #[test]
    fn remaining_configuration_fields_and_each_invalid_fact_refuse_before_source_io() {
        let original = layouts();
        let configuration = config();
        let enabled = features();
        let unread = Path::new("/fake-unread-source");
        let sources = || Sources {
            workspace: unread,
            syntax: unread,
            rust_release: "1.98.1",
            features: &enabled,
        };
        for changed in [
            configuration.clone().backtrack(false),
            configuration.clone().dfa_size_limit(Some(123_usize)),
            configuration.clone().dfa_state_limit(Some(123_usize)),
        ] {
            assert_eq!(
                SourceRecipe::from_source("fake_config_owner", "^[0-9]+$", &changed, sources(),)
                    .err(),
                Some(Refusal::Configuration)
            );
        }
        for key in required_fact_keys() {
            let mut invalid = original.clone();
            assert!(invalid.insert(key, field(64_usize, 3_usize)).is_some());
            let root = program("^[0-9]+$", &invalid);
            assert_eq!(
                evaluate(&Inputs {
                    owner: "fake_layout_owner",
                    pattern: "^[0-9]+$",
                    config: &configuration,
                    program: &root,
                    layouts: &invalid,
                    sources: sources(),
                })
                .err(),
                Some(Refusal::Layout)
            );
        }
        let mut atomic_mismatch = original.clone();
        assert!(
            atomic_mismatch
                .insert(
                    type_name::<AtomicUsize>().to_owned(),
                    field(16_usize, 8_usize)
                )
                .is_some()
        );
        let root = program("^[0-9]+$", &atomic_mismatch);
        assert_eq!(
            evaluate(&Inputs {
                owner: "fake_layout_owner",
                pattern: "^[0-9]+$",
                config: &configuration,
                program: &root,
                layouts: &atomic_mismatch,
                sources: sources(),
            })
            .err(),
            Some(Refusal::Layout)
        );
    }

    #[test]
    fn constructor_and_retained_facts_cannot_describe_different_root_premises() {
        let original = layouts();
        let configuration = config();
        let enabled = features();
        let unread = Path::new("/fake-unread-source");
        let mut root = program("^[0-9]+$", &original);
        let sources = || Sources {
            workspace: unread,
            syntax: unread,
            rust_release: "1.98.1",
            features: &enabled,
        };
        let mut changed = original.clone();
        assert!(
            changed
                .insert(type_name::<State>().to_owned(), field(40_usize, 8_usize))
                .is_some()
        );
        assert_eq!(
            evaluate(&Inputs {
                owner: "fake_root_owner",
                pattern: "^[0-9]+$",
                config: &configuration,
                program: &root,
                layouts: &changed,
                sources: sources(),
            })
            .err(),
            Some(Refusal::Layout)
        );
        root.metadata.props_payload = field(88_usize, 8_usize);
        assert_eq!(
            evaluate(&Inputs {
                owner: "fake_root_owner",
                pattern: "^[0-9]+$",
                config: &configuration,
                program: &root,
                layouts: &original,
                sources: sources(),
            })
            .err(),
            Some(Refusal::Layout)
        );
    }

    #[test]
    fn outer_vectors_and_dead_state_have_independent_literal_layout_axes() {
        assert_eq!(
            outer_vectors(
                field(32_usize, 8_usize),
                field(16_usize, 8_usize),
                field(8_usize, 8_usize)
            ),
            Ok(200_usize)
        );
        assert_eq!(
            outer_vectors(
                field(40_usize, 8_usize),
                field(16_usize, 8_usize),
                field(8_usize, 8_usize)
            ),
            Ok(232_usize)
        );
        assert_eq!(
            outer_vectors(
                field(32_usize, 8_usize),
                field(24_usize, 8_usize),
                field(8_usize, 8_usize)
            ),
            Ok(232_usize)
        );
        assert_eq!(
            hybrid_dead(
                field(1_usize, 1_usize),
                field(8_usize, 4_usize),
                field(4_usize, 4_usize)
            ),
            Ok(29_usize)
        );
        assert_eq!(
            hybrid_dead(
                field(1_usize, 1_usize),
                field(16_usize, 8_usize),
                field(8_usize, 8_usize)
            ),
            Ok(41_usize)
        );
        assert_eq!(
            hybrid_dead(
                field(1_usize, 1_usize),
                field(8_usize, 4_usize),
                field(8_usize, 8_usize)
            ),
            Ok(29_usize)
        );
    }

    fn onepass() -> OnePassConstructionCertificate {
        OnePassConstructionCertificate {
            nfa_states: 2_usize,
            rows_upper_bound: 3_usize,
            stride: 4_usize,
            scaffold_bytes: 3_usize,
            queue_peak_bytes: 5_usize,
            nfa_map_bytes: 7_usize,
            stack_peak_bytes: 11_usize,
            seen_bytes: 13_usize,
            table_peak_bytes: 17_usize,
            starts_peak_bytes: 19_usize,
            remapper_peak_bytes: 23_usize,
            shrink_overlap_bytes: 29_usize,
            retained_tables_upper_bytes: 29_usize,
            peak_requested_bytes: 127_usize,
            failure_prefix_peak_bytes: 127_usize,
            closure_visits_upper_bound: 31_usize,
            byte_visits_upper_bound: 37_usize,
            remap_visits_upper_bound: 41_usize,
            work_upper_bound: 139_usize,
        }
    }

    #[test]
    fn onepass_i_is_named_scratch_not_peak_minus_c_or_a_second_shrink_charge() {
        let certificate = onepass();
        assert_eq!(onepass_scratch(&certificate, 29_usize), Ok(98_usize));
        assert_eq!(
            onepass_scratch(&certificate, 28_usize),
            Err(Refusal::RetainedCoverage)
        );
        let different_shrink = OnePassConstructionCertificate {
            shrink_overlap_bytes: 37_usize,
            retained_tables_upper_bytes: 37_usize,
            peak_requested_bytes: 135_usize,
            ..certificate
        };
        assert_eq!(onepass_scratch(&different_shrink, 37_usize), Ok(98_usize));
        let different_table = OnePassConstructionCertificate {
            table_peak_bytes: 18_usize,
            ..certificate
        };
        assert_eq!(onepass_scratch(&different_table, 29_usize), Ok(99_usize));
        let different_failure = OnePassConstructionCertificate {
            failure_prefix_peak_bytes: 128_usize,
            ..certificate
        };
        assert_eq!(
            onepass_scratch(&different_failure, 29_usize),
            Err(Refusal::RetainedCoverage)
        );
    }

    #[test]
    fn five_distinct_owner_controls_keep_equal_digits_and_complete_fixed_scope_work() {
        let (workspace, syntax) = source_locations();
        let facts = layouts();
        let config = config();
        let enabled = features();
        let mut attempts = 0_usize;
        for (owner, pattern) in [
            ("fake_digits_a", "^[0-9]+$"),
            ("fake_digits_b", "^[0-9]+$"),
            ("fake_decimal", r"^[0-9]+(\.[0-9]+)?$"),
            ("fake_suffix", r"\.[a-zA-Z]{2,3}$"),
            ("fake_hostname", HOSTNAME_SOURCE),
        ] {
            let program = program(pattern, &facts);
            let certificate = evaluate(&Inputs {
                owner,
                pattern,
                config: &config,
                program: &program,
                layouts: &facts,
                sources: Sources {
                    workspace: &workspace,
                    syntax: &syntax,
                    rust_release: "1.98.1",
                    features: &enabled,
                },
            })
            .unwrap();
            assert_eq!(certificate.owner, owner);
            assert!(certificate.construction_only_bytes > 0_usize);
            assert!(certificate.retained.total > 0_usize);
            assert_eq!(certificate.phase_scratch.len(), 13_usize);
            for stage in [
                Stage::Syntax,
                Stage::Literal,
                Stage::ForwardNfa,
                Stage::PikeVm,
                Stage::Backtrack,
                Stage::OnePass,
                Stage::ReverseNfa,
                Stage::HybridForward,
                Stage::HybridReverse,
                Stage::HybridPair,
                Stage::Publication,
                Stage::Cleanup,
                Stage::FailureCleanup,
            ] {
                assert!(certificate.phase_scratch.contains_key(&stage));
                assert!(certificate.phase_scratch[&stage] <= certificate.construction_only_bytes);
            }
            assert_eq!(
                certificate.failure_prefixes,
                [
                    FailurePrefix::SyntaxReplacement,
                    FailurePrefix::NfaMutation,
                    FailurePrefix::OnePassRefusal,
                    FailurePrefix::HybridCapacity,
                ]
            );
            attempts += usize::from(certificate.onepass_attempted);
            assert!(certificate.complete_work().unwrap() > 0_usize);
            if pattern == r"\.[a-zA-Z]{2,3}$" {
                assert_eq!(certificate.work[&Work::LiteralHir], Visits::Bound(5_usize));
                assert_eq!(
                    certificate.work[&Work::LiteralRanges],
                    Visits::Bound(2_usize)
                );
                assert_eq!(
                    certificate.work[&Work::LiteralSequences],
                    Visits::Bound(32_usize)
                );
                assert_eq!(
                    certificate.work[&Work::LiteralCopies],
                    Visits::Bound(8_usize)
                );
                assert_eq!(
                    certificate.work[&Work::LiteralTrie],
                    Visits::Bound(12_usize)
                );
                assert_eq!(
                    certificate.work[&Work::LiteralSelection],
                    Visits::Bound(8_usize)
                );
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
                    owner: "fake_owner",
                    pattern: "^[0-9]+$",
                    config: &config,
                    program: &program,
                    layouts: &missing,
                    sources: Sources {
                        workspace: path,
                        syntax: path,
                        rust_release: "1.98.1",
                        features: &enabled
                    },
                })
                .err(),
                Some(Refusal::Missing(key))
            );
        }
        let program = program("^unknown$", &original);
        assert_eq!(
            evaluate(&Inputs {
                owner: "fake_owner",
                pattern: "^unknown$",
                config: &config,
                program: &program,
                layouts: &original,
                sources: Sources {
                    workspace: path,
                    syntax: path,
                    rust_release: "1.98.1",
                    features: &enabled
                },
            })
            .err(),
            Some(Refusal::PatternScope)
        );
    }

    #[test]
    fn work_refuses_unbound_owners_and_checked_sum_overflow() {
        let mut certificate = Certificate {
            owner: "fake_work".to_owned(),
            retained: NativeProgramHeap {
                metadata_bytes: 0_usize,
                strategy_arc_bytes: 0_usize,
                nfa_bytes: 0_usize,
                onepass_tables_bytes: 0_usize,
                automatic_prefilter_bytes: 0_usize,
                unused_pool_bytes: 0_usize,
                total: 0_usize,
                static_inline_bytes: 0_usize,
                automatic_prefilter: AutomaticPrefilterHeap::ExcludedStartAnchored,
            },
            phase_scratch: BTreeMap::new(),
            construction_only_bytes: 0_usize,
            onepass_attempted: false,
            work: BTreeMap::from([(Work::Literal, Visits::Unbound("unqualified literal"))]),
            failure_prefixes: [
                FailurePrefix::SyntaxReplacement,
                FailurePrefix::NfaMutation,
                FailurePrefix::OnePassRefusal,
                FailurePrefix::HybridCapacity,
            ],
        };
        assert_eq!(
            certificate.complete_work(),
            Err(Refusal::UnboundWork("unqualified literal"))
        );
        certificate.work = BTreeMap::from([
            (Work::Syntax, Visits::Bound(usize::MAX)),
            (Work::OuterVectors, Visits::Bound(1_usize)),
        ]);
        assert_eq!(certificate.complete_work(), Err(Refusal::Arithmetic));
        certificate.work.clear();
        assert_eq!(
            certificate.complete_work(),
            Err(Refusal::UnboundWork("missing source work owner"))
        );
    }

    #[test]
    fn facts_must_match_root_premises_and_configuration_before_source_io() {
        let facts = layouts();
        let config = config();
        let enabled = features();
        let path = Path::new("/fake-unread-source");
        let mut mismatched_root = program("^[0-9]+$", &facts);
        mismatched_root.arc_header = field(8_usize, 4_usize);
        let mismatched_inputs = Inputs {
            owner: "fake_owner",
            pattern: "^[0-9]+$",
            config: &config,
            program: &mismatched_root,
            layouts: &facts,
            sources: Sources {
                workspace: path,
                syntax: path,
                rust_release: "1.98.1",
                features: &enabled,
            },
        };
        assert_eq!(evaluate(&mismatched_inputs).err(), Some(Refusal::Layout));
        let disabled_hybrid = config.clone().hybrid(false);
        let configuration_inputs = Inputs {
            config: &disabled_hybrid,
            ..mismatched_inputs
        };
        assert_eq!(evaluate(&configuration_inputs).err(), Some(Refusal::Configuration));
        let root = program("^[0-9]+$", &facts);
        let mut missing_feature = enabled.clone();
        assert!(missing_feature.remove("syntax"));
        let feature_inputs = Inputs {
            owner: "fake_owner",
            pattern: "^[0-9]+$",
            config: &config,
            program: &root,
            layouts: &facts,
            sources: Sources {
                workspace: path,
                syntax: path,
                rust_release: "1.98.1",
                features: &missing_feature,
            },
        };
        assert_eq!(evaluate(&feature_inputs).err(), Some(Refusal::Features));
        let release_inputs = Inputs {
            sources: Sources {
                workspace: path,
                syntax: path,
                rust_release: "unqualified",
                features: &enabled,
            },
            ..feature_inputs
        };
        assert_eq!(
            evaluate(&release_inputs).err(),
            Some(Refusal::Source("Rust collection paths"))
        );
    }

    #[test]
    fn source_recipe_seals_without_layouts_and_exposes_model_identities_only() {
        let (workspace, syntax) = source_locations();
        let config = config();
        let enabled = features();
        // No layout map, private payload facts or physical certificate exists yet.
        let recipe = SourceRecipe::from_source(
            "fake_source_only",
            "^[0-9]+$",
            &config,
            Sources {
                workspace: &workspace,
                syntax: &syntax,
                rust_release: "1.98.1",
                features: &enabled,
            },
        )
        .unwrap();
        assert_eq!(recipe.owner(), "fake_source_only");
        assert_eq!(recipe.pattern(), "^[0-9]+$");
        assert_eq!(recipe.required_facts(), &required_fact_keys());
        assert_eq!(recipe.phase_models().len(), 13_usize);
        assert_eq!(recipe.work_models().len(), 15_usize);
        // Physical projection is a separate call, with mandatory explicit facts.
        let facts = layouts();
        let root = program(recipe.pattern(), &facts);
        recipe.evaluate(&facts, &root).unwrap();
    }

    #[test]
    fn source_recipe_rejects_unknown_scope_features_release_and_unverified_source() {
        let config = config();
        let enabled = features();
        let path = Path::new("/fake-unread-source");
        let sources = || Sources {
            workspace: path,
            syntax: path,
            rust_release: "1.98.1",
            features: &enabled,
        };
        assert_eq!(
            SourceRecipe::from_source("fake_owner", "^unknown$", &config, sources()).err(),
            Some(Refusal::PatternScope)
        );
        let mut unknown = enabled.clone();
        assert!(unknown.insert("perf-literal-inner".to_owned()));
        let unknown_sources = Sources {
            features: &unknown,
            ..sources()
        };
        assert_eq!(
            SourceRecipe::from_source("fake_owner", "^[0-9]+$", &config, unknown_sources).err(),
            Some(Refusal::Features)
        );
        let wrong_release_sources = Sources {
            workspace: path,
            syntax: path,
            rust_release: "unqualified",
            features: &enabled,
        };
        assert_eq!(
            SourceRecipe::from_source("fake_owner", "^[0-9]+$", &config, wrong_release_sources).err(),
            Some(Refusal::Source("Rust collection paths"))
        );
        let unverified_sources = Sources {
            workspace: path,
            syntax: path,
            rust_release: "1.98.1",
            features: &enabled,
        };
        assert_eq!(
            SourceRecipe::from_source("fake_owner", "^[0-9]+$", &config, unverified_sources).err(),
            Some(Refusal::Source("regex-automata source"))
        );
    }

    #[test]
    fn explicit_request_overflow_refuses_instead_of_host_sized_or_zero_scratch() {
        assert_eq!(
            outer_vectors(
                field(0x4000_0000_usize, 8_usize),
                field(16_usize, 8_usize),
                field(4_usize, 4_usize)
            ),
            Err(Refusal::Arithmetic)
        );
        assert_eq!(
            hybrid_dead(
                field(2_usize, 1_usize),
                field(16_usize, 8_usize),
                field(8_usize, 8_usize)
            ),
            Err(Refusal::Layout)
        );
        assert_eq!(
            hybrid_dead(
                field(1_usize, 1_usize),
                field(0_usize, 8_usize),
                field(8_usize, 8_usize)
            ),
            Err(Refusal::Layout)
        );
        assert_eq!(
            hybrid_dead(
                field(1_usize, 1_usize),
                field(0x8000_0000_usize, 8_usize),
                field(4_usize, 4_usize)
            ),
            Err(Refusal::Arithmetic)
        );
    }
}

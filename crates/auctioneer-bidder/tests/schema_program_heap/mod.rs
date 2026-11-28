//! Conditional native retained requested heap of one strictly configured regex.
//! Total excludes code/data/static linkage, static inline storage, constructor
//! peaks, caches, search work, allocator/RSS overhead, guest and serving qualification.
//! Native reconstruction does not establish serving-release layout correspondence.

use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};
use crate::schema_heap_layout::{
    LayoutError, NativeField, arc_layout_with_header, native_arc_value_layout,
};
use crate::schema_literal_heap::{
    AutomaticPrefilterError, AutomaticPrefilterHeap, AutomaticPrefilterSource,
};
use crate::schema_metadata_heap::{
    MetadataHeapError, MetadataLayout, NativeImmutableRegexMetadata,
};
use crate::schema_nfa_heap::{NfaHeapError, NfaRetainedHeap, target};
use crate::schema_onepass_bounds::{
    OnePassRetainedTables, OnePassTablesError, nfa_table_upper_bound,
    nfa_table_upper_bound_with_word,
};
use crate::schema_pool_heap::{PoolHeapError, PoolLayout, UnusedPoolHeap};
use regex_automata::{
    MatchKind,
    meta::{Config, Regex, RegexResourceLayout},
    nfa::thompson::{NFA, WhichCaptures},
    util::syntax,
};
use std::{any::type_name, collections::BTreeMap, sync::atomic::AtomicUsize};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ProgramHeapError {
    Configuration,
    Compilation,
    Metadata,
    RootLayout,
    Layout,
    Nfa,
    OnePass,
    AutomaticPrefilter(AutomaticPrefilterError),
    Pool,
    Shape,
    Arithmetic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeProgramHeap {
    pub metadata_bytes: usize,
    pub strategy_arc_bytes: usize,
    pub nfa_bytes: usize,
    pub onepass_tables_bytes: usize,
    pub automatic_prefilter_bytes: usize,
    pub unused_pool_bytes: usize,
    pub total: usize,
    pub static_inline_bytes: usize,
    pub automatic_prefilter: AutomaticPrefilterHeap,
}

/// Mandatory matched layout premises for one source-selected program.
/// The caller binds these fields to the actual source/features/target tuple.
/// Synthetic test fields are not target ABI observations.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProgramRecipe<'a> {
    pub pattern: &'a str,
    pub metadata: MetadataLayout,
    pub arc_header: NativeField,
    pub strategy_payload: NativeField,
    pub nfa_facts: &'a BTreeMap<String, NativeField>,
    pub onepass_word: NativeField,
    pub memchr_payload: NativeField,
    pub pool: PoolLayout,
}

/// Nonphysical reconstruction. Native topology is not a target layout witness.
#[derive(Debug)]
pub(crate) struct SourceRecipe {
    pattern: String,
    config: Config,
    native_roots: RegexResourceLayout,
    forward: NFA,
    reverse: NFA,
    prefilter: AutomaticPrefilterSource,
}

#[derive(Clone, Copy)]
enum Projection<'a> {
    Native,
    Supplied(&'a ProgramRecipe<'a>),
}

impl SourceRecipe {
    /// Immutable borrows of the already retained forward and reverse NFAs.
    pub(crate) fn nfas(&self) -> [&NFA; 2] {
        [&self.forward, &self.reverse]
    }

    pub(crate) fn from_pattern(pattern: &str, config: &Config) -> Result<Self, ProgramHeapError> {
        require_configuration(config)?;
        let candidate = Regex::builder()
            .configure(config.clone())
            .syntax(syntax::Config::default().utf8(true))
            .build(pattern)
            .map_err(|_error| ProgramHeapError::Compilation)?;
        let prefilter = AutomaticPrefilterHeap::source(pattern, config)
            .map_err(ProgramHeapError::AutomaticPrefilter)?;
        let forward = forward_nfa(pattern).map_err(|_error| ProgramHeapError::Nfa)?;
        let reverse = reverse_nfa(pattern).map_err(|_error| ProgramHeapError::Nfa)?;
        Ok(Self {
            pattern: pattern.to_owned(),
            config: config.clone(),
            native_roots: candidate.resource_layout(),
            forward,
            reverse,
            prefilter,
        })
    }

    pub(crate) fn evaluate(
        &self,
        facts: &ProgramRecipe<'_>,
    ) -> Result<NativeProgramHeap, ProgramHeapError> {
        if facts.pattern != self.pattern {
            return Err(ProgramHeapError::Shape);
        }
        let word = facts.onepass_word;
        if !matches!(word.bytes, 4 | 8) || word.alignment != word.bytes {
            return Err(ProgramHeapError::OnePass);
        }
        let pointer = facts
            .nfa_facts
            .get(target::ABI_POINTER)
            .ok_or(ProgramHeapError::Nfa)?;
        let atomic = facts
            .nfa_facts
            .get(type_name::<AtomicUsize>())
            .ok_or(ProgramHeapError::Nfa)?;
        if *pointer != word || *atomic != word {
            return Err(ProgramHeapError::Nfa);
        }
        if facts.arc_header.bytes
            != word
                .bytes
                .checked_mul(2)
                .ok_or(ProgramHeapError::Arithmetic)?
            || facts.arc_header.alignment != word.alignment
        {
            return Err(ProgramHeapError::Shape);
        }
        self.project(Projection::Supplied(facts))
    }

    fn project_nfas(&self, projection: Projection<'_>) -> Result<usize, ProgramHeapError> {
        let mut nfa_bytes = 0_usize;
        for nfa in [&self.forward, &self.reverse] {
            let tree = match projection {
                Projection::Native => {
                    NfaRetainedHeap::from_nfa(nfa).map_err(|error| match error {
                        NfaHeapError::Arithmetic
                        | NfaHeapError::Layout(LayoutError::Arithmetic) => {
                            ProgramHeapError::Arithmetic
                        }
                        NfaHeapError::Layout(LayoutError::Shape)
                        | NfaHeapError::NamesUnqualified
                        | NfaHeapError::Shape
                        | NfaHeapError::Compilation
                        | NfaHeapError::Prefix(_) => ProgramHeapError::Nfa,
                    })?
                }
                Projection::Supplied(facts) => {
                    target::project_nfa(nfa, facts.nfa_facts).map_err(|error| match error {
                        target::TargetHeapError::Arithmetic => ProgramHeapError::Arithmetic,
                        target::TargetHeapError::MissingType(_)
                        | target::TargetHeapError::InvalidLayout(_)
                        | target::TargetHeapError::UnsupportedAbi
                        | target::TargetHeapError::NamesUnqualified
                        | target::TargetHeapError::UnboundShape => ProgramHeapError::Nfa,
                    })?
                }
            };
            if tree.requested_heap_bytes == 0 {
                return Err(ProgramHeapError::Shape);
            }
            nfa_bytes = nfa_bytes
                .checked_add(tree.requested_heap_bytes)
                .ok_or(ProgramHeapError::Arithmetic)?;
        }
        Ok(nfa_bytes)
    }

    // Both adapters share the six-owner composition and structural dimensions.
    // Only explicit Native mode may obtain build-host layout values.
    fn project(&self, projection: Projection<'_>) -> Result<NativeProgramHeap, ProgramHeapError> {
        let metadata = match projection {
            Projection::Native => {
                NativeImmutableRegexMetadata::from_pattern(&self.pattern, &self.config, 1)
            }
            Projection::Supplied(facts) => NativeImmutableRegexMetadata::from_pattern_with_layouts(
                &self.pattern,
                &self.config,
                1,
                facts.metadata,
                facts.arc_header,
            ),
        }
        .map_err(metadata_error)?;
        if matches!(projection, Projection::Native) {
            require_metadata_roots(&metadata, self.native_roots)?;
        }
        let metadata_bytes = metadata
            .population(1, false)
            .map_err(metadata_error)?
            .requested_heap_bytes;
        let strategy_arc_bytes = match projection {
            Projection::Native => native_arc_value_layout(self.native_roots.strategy_payload()),
            Projection::Supplied(facts) => {
                arc_layout_with_header(facts.strategy_payload, facts.arc_header)
            }
        }
        .map_err(layout_error)?
        .size();
        let nfa_bytes = self.project_nfas(projection)?;
        let shift = u32::try_from(self.forward.byte_classes().stride2())
            .map_err(|_error| ProgramHeapError::Arithmetic)?;
        let stride = 1_usize
            .checked_shl(shift)
            .ok_or(ProgramHeapError::Arithmetic)?;
        let onepass = match projection {
            Projection::Native => nfa_table_upper_bound(
                self.forward.states().len(),
                stride,
                self.forward.pattern_len(),
            ),
            Projection::Supplied(facts) => nfa_table_upper_bound_with_word(
                self.forward.states().len(),
                stride,
                self.forward.pattern_len(),
                facts.onepass_word,
            ),
        }
        .map_err(|error| match error {
            OnePassTablesError::Arithmetic => ProgramHeapError::Arithmetic,
            OnePassTablesError::Configuration
            | OnePassTablesError::Compilation
            | OnePassTablesError::Shape => ProgramHeapError::OnePass,
        })?;
        let pool = match projection {
            Projection::Native => UnusedPoolHeap::native(&self.config),
            Projection::Supplied(facts) => UnusedPoolHeap::with_layout(&self.config, facts.pool),
        }
        .map_err(|error| match error {
            PoolHeapError::Arithmetic => ProgramHeapError::Arithmetic,
            PoolHeapError::Shape | PoolHeapError::Configuration => ProgramHeapError::Pool,
        })?;
        let (memchr, header) = match projection {
            Projection::Native => (
                NativeField::of::<u8>(),
                NativeField::of::<[AtomicUsize; 2]>(),
            ),
            Projection::Supplied(facts) => (facts.memchr_payload, facts.arc_header),
        };
        let prefilter = AutomaticPrefilterHeap::from_source(self.prefilter, memchr, header)
            .map_err(ProgramHeapError::AutomaticPrefilter)?;
        if let Projection::Supplied(facts) = projection {
            check_target_requests(
                facts,
                &metadata,
                &pool,
                strategy_arc_bytes,
                &onepass,
                prefilter,
            )?;
        }
        NativeProgramHeap::compose(
            [
                metadata_bytes,
                strategy_arc_bytes,
                nfa_bytes,
                onepass.total,
                prefilter.requested_bytes(),
                pool.requested_bytes,
            ],
            metadata.static_inline_bytes,
            prefilter,
        )
    }
}

fn metadata_error(error: MetadataHeapError) -> ProgramHeapError {
    match error {
        MetadataHeapError::Arithmetic => ProgramHeapError::Arithmetic,
        MetadataHeapError::Shape
        | MetadataHeapError::PatternCount
        | MetadataHeapError::Configuration
        | MetadataHeapError::Syntax => ProgramHeapError::Metadata,
    }
}

fn target_request(
    word: NativeField,
    bytes: usize,
    alignment: usize,
) -> Result<(), ProgramHeapError> {
    if !alignment.is_power_of_two() {
        return Err(ProgramHeapError::Layout);
    }
    let maximum = if word.bytes == 4 {
        usize::try_from(i32::MAX)
    } else {
        usize::try_from(i64::MAX)
    }
    .map_err(|_error| ProgramHeapError::Arithmetic)?;
    let padding = alignment
        .checked_sub(1_usize)
        .ok_or(ProgramHeapError::Arithmetic)?;
    let maximum_request = maximum
        .checked_sub(padding)
        .ok_or(ProgramHeapError::Arithmetic)?;
    if bytes > maximum_request {
        return Err(ProgramHeapError::Arithmetic);
    }
    Ok(())
}

fn check_target_requests(
    facts: &ProgramRecipe<'_>,
    metadata: &NativeImmutableRegexMetadata,
    pool: &UnusedPoolHeap,
    strategy: usize,
    onepass: &OnePassRetainedTables,
    prefilter: AutomaticPrefilterHeap,
) -> Result<(), ProgramHeapError> {
    let word = facts.onepass_word;
    let header = facts.arc_header.alignment;
    let regex_alignment = facts
        .metadata
        .regex_fields
        .iter()
        .map(|field| field.alignment)
        .max()
        .ok_or(ProgramHeapError::Shape)?;
    let info_alignment = facts
        .metadata
        .info_fields
        .iter()
        .map(|field| field.alignment)
        .max()
        .ok_or(ProgramHeapError::Shape)?;
    let pool_alignment = facts
        .pool
        .fields
        .iter()
        .map(|field| field.alignment)
        .max()
        .ok_or(ProgramHeapError::Shape)?;
    for field in facts
        .metadata
        .regex_fields
        .into_iter()
        .chain(facts.metadata.info_fields)
        .chain([
            facts.metadata.props_payload,
            facts.metadata.static_inline,
            facts.arc_header,
            facts.strategy_payload,
            facts.memchr_payload,
            facts.pool.shard,
            facts.pool.closure,
        ])
        .chain(facts.pool.fields)
    {
        target_request(word, field.bytes, field.alignment)?;
    }
    // Check requests separately, not the sum of disjoint allocations.
    for (bytes, alignment) in [
        (metadata.regex_arc_bytes, regex_alignment.max(header)),
        (metadata.info_arc_bytes, info_alignment.max(header)),
        (
            metadata.props_vec_bytes,
            facts.metadata.info_fields[2].alignment,
        ),
        (
            metadata.props_box_bytes,
            facts.metadata.props_payload.alignment,
        ),
        (
            metadata.props_union_box_bytes,
            facts.metadata.props_payload.alignment,
        ),
        (strategy, facts.strategy_payload.alignment.max(header)),
        (pool.boxed_pool_bytes, pool_alignment),
        (pool.shard_bytes, facts.pool.shard.alignment.max(64)),
        (pool.closure_bytes, facts.pool.closure.alignment),
        (onepass.table_bytes, 8),
        (onepass.starts_bytes, 4),
        (
            prefilter.requested_bytes(),
            facts.memchr_payload.alignment.max(header),
        ),
    ] {
        target_request(word, bytes, alignment)?;
    }
    Ok(())
}

impl NativeProgramHeap {
    pub(crate) fn from_pattern(pattern: &str, config: &Config) -> Result<Self, ProgramHeapError> {
        SourceRecipe::from_pattern(pattern, config)?.project(Projection::Native)
    }

    fn compose(
        charges: [usize; 6],
        static_inline_bytes: usize,
        automatic_prefilter: AutomaticPrefilterHeap,
    ) -> Result<Self, ProgramHeapError> {
        let [
            metadata_bytes,
            strategy_arc_bytes,
            nfa_bytes,
            onepass_tables_bytes,
            automatic_prefilter_bytes,
            unused_pool_bytes,
        ] = charges;
        if [
            metadata_bytes,
            strategy_arc_bytes,
            nfa_bytes,
            onepass_tables_bytes,
            unused_pool_bytes,
            static_inline_bytes,
        ]
        .contains(&0)
        {
            return Err(ProgramHeapError::Shape);
        }
        let consistent = match automatic_prefilter {
            AutomaticPrefilterHeap::ExcludedStartAnchored => automatic_prefilter_bytes == 0,
            AutomaticPrefilterHeap::SingleByte {
                requested_bytes, ..
            } => requested_bytes != 0 && automatic_prefilter_bytes == requested_bytes,
        };
        if !consistent {
            return Err(ProgramHeapError::Shape);
        }
        let total = charges
            .into_iter()
            .try_fold(0_usize, usize::checked_add)
            .ok_or(ProgramHeapError::Arithmetic)?;
        Ok(Self {
            metadata_bytes,
            strategy_arc_bytes,
            nfa_bytes,
            onepass_tables_bytes,
            automatic_prefilter_bytes,
            unused_pool_bytes,
            total,
            static_inline_bytes,
            automatic_prefilter,
        })
    }
}

pub(crate) fn require_configuration(config: &Config) -> Result<(), ProgramHeapError> {
    require_engine_flags(config.get_hybrid(), config.get_dfa(), config.get_onepass())?;
    if config.get_match_kind() != MatchKind::LeftmostFirst
        || !config.get_utf8_empty()
        || !matches!(config.get_which_captures(), WhichCaptures::All)
        || !config.get_byte_classes()
        || config.get_line_terminator() != b'\n'
        || config.get_nfa_size_limit() != Some(0x0001_0000)
        || config.get_hybrid_cache_capacity() != 0x0001_0000
        || config.get_onepass_size_limit() != Some(0x0010_0000)
        || config.get_pool_capacity() != 1
        || !config.get_auto_prefilter()
        || config.get_prefilter().is_some()
    {
        return Err(ProgramHeapError::Configuration);
    }
    Ok(())
}

fn require_engine_flags(
    hybrid: bool,
    full_dfa: bool,
    onepass: bool,
) -> Result<(), ProgramHeapError> {
    if hybrid && !full_dfa && onepass {
        Ok(())
    } else {
        Err(ProgramHeapError::Configuration)
    }
}

fn require_metadata_roots(
    metadata: &NativeImmutableRegexMetadata,
    actual: RegexResourceLayout,
) -> Result<(), ProgramHeapError> {
    let regex_arc_bytes = native_arc_value_layout(actual.regex_payload())
        .map_err(layout_error)?
        .size();
    let info_arc_bytes = native_arc_value_layout(actual.metadata_payload())
        .map_err(layout_error)?
        .size();
    if regex_arc_bytes > metadata.regex_arc_bytes || info_arc_bytes > metadata.info_arc_bytes {
        return Err(ProgramHeapError::RootLayout);
    }
    Ok(())
}

fn layout_error(error: LayoutError) -> ProgramHeapError {
    match error {
        LayoutError::Shape => ProgramHeapError::Layout,
        LayoutError::Arithmetic => ProgramHeapError::Arithmetic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};
    use crate::schema_heap_layout::native_arc_value_layout;
    use crate::schema_nfa_heap::NfaRetainedHeap;
    use crate::schema_onepass_bounds::nfa_table_upper_bound;
    use crate::schema_pool_heap::UnusedPoolHeap;
    use regex_automata::{
        MatchKind,
        meta::Regex,
        nfa::thompson::WhichCaptures,
        util::{prefilter::Prefilter, syntax},
    };
    use std::ptr;

    fn fake_config() -> Config {
        Config::new()
            .match_kind(MatchKind::LeftmostFirst)
            .utf8_empty(true)
            .which_captures(WhichCaptures::All)
            .byte_classes(true)
            .line_terminator(b'\n')
            .nfa_size_limit(Some(0x0001_0000))
            .hybrid_cache_capacity(0x0001_0000)
            .onepass_size_limit(Some(0x0010_0000))
            .pool_capacity(1)
            .hybrid(true)
            .dfa(false)
            .onepass(true)
            .auto_prefilter(true)
    }

    // Deliberately synthetic fields, not measurements of any native/guest ABI.
    fn recipe_nfa_facts() -> BTreeMap<String, NativeField> {
        use regex_automata::{
            nfa::thompson::{State, Transition},
            util::primitives::{SmallIndex, StateID},
        };
        use std::{
            any::type_name,
            collections::HashMap,
            sync::{Arc, atomic::AtomicUsize},
        };
        [
            (target::NFA_INNER, 512_usize, 16_usize),
            (target::GROUP_INFO_INNER, 48_usize, 8_usize),
            (type_name::<State>(), 16_usize, 4_usize),
            (type_name::<Transition>(), 8_usize, 4_usize),
            (type_name::<StateID>(), 4_usize, 4_usize),
            (type_name::<(SmallIndex, SmallIndex)>(), 8_usize, 4_usize),
            (
                type_name::<HashMap<Arc<str>, SmallIndex>>(),
                24_usize,
                4_usize,
            ),
            (type_name::<Vec<Option<Arc<str>>>>(), 12_usize, 4_usize),
            (type_name::<Option<Arc<str>>>(), 8_usize, 4_usize),
            (type_name::<AtomicUsize>(), 4_usize, 4_usize),
            (target::ABI_POINTER, 4_usize, 4_usize),
        ]
        .into_iter()
        .map(|(name, bytes, alignment)| (name.to_owned(), NativeField { bytes, alignment }))
        .collect()
    }

    fn recipe_facts(nfa_facts: &BTreeMap<String, NativeField>) -> ProgramRecipe<'_> {
        let field = |bytes, alignment| NativeField { bytes, alignment };
        ProgramRecipe {
            pattern: "q[a-z]{2,3}$",
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
            arc_header: field(8_usize, 4_usize),
            strategy_payload: field(64_usize, 8_usize),
            nfa_facts,
            onepass_word: field(4_usize, 4_usize),
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

    #[test]
    fn target_recipe_literal_leaf_charges_are_not_native_defaults() {
        let map = recipe_nfa_facts();
        let facts = recipe_facts(&map);
        let source = SourceRecipe::from_pattern(facts.pattern, &fake_config()).unwrap();
        let heap = source.evaluate(&facts).unwrap();
        assert_eq!(heap.metadata_bytes, 304_usize);
        assert_eq!(heap.strategy_arc_bytes, 72_usize);
        assert_eq!(heap.automatic_prefilter_bytes, 12_usize);
        assert_eq!(heap.unused_pool_bytes, 168_usize);
        assert_eq!(heap.static_inline_bytes, 64_usize);
        assert!(heap.nfa_bytes > 0_usize);
        assert!(heap.onepass_tables_bytes > 0_usize);
        // Four independently literal leaves; NFA/one-pass remain their own owners.
        assert_eq!(
            heap.total
                .checked_sub(heap.nfa_bytes)
                .unwrap()
                .checked_sub(heap.onepass_tables_bytes)
                .unwrap(),
            556_usize
        );
        let borrowed = source.nfas();
        assert!(ptr::eq(borrowed[0], &source.forward));
        assert!(ptr::eq(borrowed[1], &source.reverse));
        assert!(!borrowed[0].is_reverse());
        assert!(borrowed[1].is_reverse());
        let again = source.nfas();
        assert!(ptr::eq(borrowed[0], again[0]));
        assert!(ptr::eq(borrowed[1], again[1]));
    }

    #[test]
    fn target_recipe_independent_layout_axes_change_only_their_own_charge() {
        let map = recipe_nfa_facts();
        let facts = recipe_facts(&map);
        let source = SourceRecipe::from_pattern(facts.pattern, &fake_config()).unwrap();
        let base = source.evaluate(&facts).unwrap();
        for axis in 0_usize..4_usize {
            let mut changed = facts;
            let mut changed_map = map.clone();
            match axis {
                0 => changed.metadata.props_payload.bytes = 88_usize,
                1 => changed.strategy_payload.bytes = 72_usize,
                2 => changed.pool.closure.bytes = 24_usize,
                _ => {
                    changed_map.get_mut(target::NFA_INNER).unwrap().bytes += 16_usize;
                    changed.nfa_facts = &changed_map;
                }
            }
            let projected = source.evaluate(&changed).unwrap();
            let expected = match axis {
                0 => NativeProgramHeap {
                    metadata_bytes: base.metadata_bytes + 16_usize,
                    total: base.total + 16_usize,
                    ..base
                },
                1 => NativeProgramHeap {
                    strategy_arc_bytes: base.strategy_arc_bytes + 8_usize,
                    total: base.total + 8_usize,
                    ..base
                },
                2 => NativeProgramHeap {
                    unused_pool_bytes: base.unused_pool_bytes + 8_usize,
                    total: base.total + 8_usize,
                    ..base
                },
                _ => NativeProgramHeap {
                    nfa_bytes: base.nfa_bytes + 32_usize,
                    total: base.total + 32_usize,
                    ..base
                },
            };
            assert_eq!(projected, expected, "layout axis {axis}");
        }
        let mut inline = facts;
        inline.metadata.static_inline.bytes = 72_usize;
        assert_eq!(
            source.evaluate(&inline),
            Ok(NativeProgramHeap {
                static_inline_bytes: 72_usize,
                ..base
            })
        );
    }

    #[test]
    fn target_recipe_missing_fields_unknown_shapes_and_unmatched_pattern_refuse() {
        let map = recipe_nfa_facts();
        let facts = recipe_facts(&map);
        let source = SourceRecipe::from_pattern(facts.pattern, &fake_config()).unwrap();
        for name in target::fact_keys() {
            let mut missing = map.clone();
            assert!(missing.remove(name).is_some());
            assert_eq!(
                source.evaluate(&ProgramRecipe {
                    nfa_facts: &missing,
                    ..facts
                }),
                Err(ProgramHeapError::Nfa),
                "{name}"
            );
        }
        let mut mismatched_pattern = facts;
        mismatched_pattern.pattern = "^different$";
        assert_eq!(
            source.evaluate(&mismatched_pattern),
            Err(ProgramHeapError::Shape)
        );
        for axis in 0_usize..5_usize {
            let mut changed = facts;
            let error = match axis {
                0 => {
                    changed.metadata.static_inline.bytes = 0_usize;
                    ProgramHeapError::Metadata
                }
                1 => {
                    changed.strategy_payload.bytes = 0_usize;
                    ProgramHeapError::Layout
                }
                2 => {
                    changed.pool.closure.bytes = 0_usize;
                    ProgramHeapError::Pool
                }
                3 => {
                    changed.onepass_word.bytes = 0_usize;
                    ProgramHeapError::OnePass
                }
                _ => {
                    changed.memchr_payload.bytes = 2_usize;
                    ProgramHeapError::AutomaticPrefilter(AutomaticPrefilterError::Layout)
                }
            };
            assert_eq!(source.evaluate(&changed), Err(error), "shape axis {axis}");
        }
        assert_eq!(
            SourceRecipe::from_pattern("qr[a-z]+$", &fake_config()).err(),
            Some(ProgramHeapError::AutomaticPrefilter(
                AutomaticPrefilterError::Scope
            ))
        );
    }

    #[test]
    fn target_recipe_each_explicit_allocation_overflow_refuses() {
        let map = recipe_nfa_facts();
        let facts = recipe_facts(&map);
        let source = SourceRecipe::from_pattern(facts.pattern, &fake_config()).unwrap();
        for axis in 0_usize..4_usize {
            let mut changed = facts;
            let mut changed_map = map.clone();
            let huge = NativeField {
                bytes: usize::MAX - 7_usize,
                alignment: 8_usize,
            };
            match axis {
                0 => changed.metadata.props_payload = huge,
                1 => changed.strategy_payload = huge,
                2 => changed.pool.closure = huge,
                _ => {
                    changed_map.insert(target::NFA_INNER.to_owned(), huge);
                    changed.nfa_facts = &changed_map;
                }
            }
            assert_eq!(
                source.evaluate(&changed),
                Err(ProgramHeapError::Arithmetic),
                "axis {axis}"
            );
        }
    }

    #[test]
    fn fake_program_literal_composition_keeps_six_charges_and_inline_separate() {
        let disposition = AutomaticPrefilterHeap::SingleByte {
            byte: b'q',
            requested_bytes: 13,
        };
        assert_eq!(
            NativeProgramHeap::compose([11, 23, 37, 41, 13, 17], 29, disposition),
            Ok(NativeProgramHeap {
                metadata_bytes: 11,
                strategy_arc_bytes: 23,
                nfa_bytes: 37,
                onepass_tables_bytes: 41,
                automatic_prefilter_bytes: 13,
                unused_pool_bytes: 17,
                total: 142,
                static_inline_bytes: 29,
                automatic_prefilter: disposition,
            })
        );
        let anchored = NativeProgramHeap::compose(
            [11, 23, 37, 41, 0, 17],
            101,
            AutomaticPrefilterHeap::ExcludedStartAnchored,
        )
        .expect("fake anchored composition");
        assert_eq!(anchored.total, 129);
        assert_eq!(anchored.static_inline_bytes, 101);
    }

    #[test]
    fn fake_program_omitted_zero_and_inconsistent_owners_refuse() {
        let charges = [11, 23, 37, 41, 13, 17];
        let disposition = AutomaticPrefilterHeap::SingleByte {
            byte: b'q',
            requested_bytes: 13,
        };
        for index in 0..charges.len() {
            let mut changed = charges;
            changed[index] = 0;
            assert_eq!(
                NativeProgramHeap::compose(changed, 29, disposition),
                Err(ProgramHeapError::Shape),
                "fake omitted charge {index}"
            );
        }
        assert_eq!(
            NativeProgramHeap::compose(charges, 0, disposition),
            Err(ProgramHeapError::Shape)
        );
        assert_eq!(
            NativeProgramHeap::compose(charges, 29, AutomaticPrefilterHeap::ExcludedStartAnchored),
            Err(ProgramHeapError::Shape)
        );
        assert_eq!(
            NativeProgramHeap::compose(
                [11, 23, 37, 41, 0, 17],
                29,
                AutomaticPrefilterHeap::SingleByte {
                    byte: b'q',
                    requested_bytes: 0
                },
            ),
            Err(ProgramHeapError::Shape)
        );
    }

    #[test]
    fn fake_program_each_charge_overflow_refuses_without_partial_total() {
        for index in 0..6 {
            let mut charges = [11, 23, 37, 41, 13, 17];
            charges[index] = usize::MAX;
            let disposition = AutomaticPrefilterHeap::SingleByte {
                byte: b'q',
                requested_bytes: charges[4],
            };
            assert_eq!(
                NativeProgramHeap::compose(charges, 29, disposition),
                Err(ProgramHeapError::Arithmetic),
                "fake overflowing charge {index}"
            );
        }
    }

    #[test]
    fn fake_program_constructor_charges_existing_components_without_root_duplication() {
        let pattern = r"^fake[0-9]+(\.[0-9]+)?$";
        let config = fake_config();
        let candidate = Regex::builder()
            .configure(config.clone())
            .syntax(syntax::Config::default().utf8(true))
            .build(pattern)
            .expect("fake candidate");
        let metadata = NativeImmutableRegexMetadata::from_pattern(pattern, &config, 1)
            .expect("fake metadata owner");
        let strategy_arc_bytes =
            native_arc_value_layout(candidate.resource_layout().strategy_payload())
                .expect("fake strategy Arc layout")
                .size();
        let forward = forward_nfa(pattern).expect("fake forward NFA");
        let reverse = reverse_nfa(pattern).expect("fake reverse NFA");
        let nfa_bytes = NfaRetainedHeap::from_nfa(&forward)
            .expect("fake forward owner")
            .requested_heap_bytes
            .checked_add(
                NfaRetainedHeap::from_nfa(&reverse)
                    .expect("fake reverse owner")
                    .requested_heap_bytes,
            )
            .expect("fake NFA sum");
        let onepass_tables_bytes = nfa_table_upper_bound(
            forward.states().len(),
            1_usize
                .checked_shl(u32::try_from(forward.byte_classes().stride2()).unwrap())
                .unwrap(),
            forward.pattern_len(),
        )
        .expect("fake onepass table envelope")
        .total;
        let unused_pool_bytes = UnusedPoolHeap::native(&config)
            .expect("fake unused pool")
            .requested_bytes;
        let total = [
            metadata.requested_heap_bytes,
            strategy_arc_bytes,
            nfa_bytes,
            onepass_tables_bytes,
            unused_pool_bytes,
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .expect("fake total");
        assert_eq!(
            NativeProgramHeap::from_pattern(pattern, &config),
            Ok(NativeProgramHeap {
                metadata_bytes: metadata.requested_heap_bytes,
                strategy_arc_bytes,
                nfa_bytes,
                onepass_tables_bytes,
                automatic_prefilter_bytes: 0,
                unused_pool_bytes,
                total,
                static_inline_bytes: metadata.static_inline_bytes,
                automatic_prefilter: AutomaticPrefilterHeap::ExcludedStartAnchored,
            })
        );
    }

    #[test]
    fn fake_program_reported_root_layouts_must_fit_each_metadata_arc_bound() {
        let pattern = "^fake[0-9]+$";
        let config = fake_config();
        let candidate = Regex::builder()
            .configure(config.clone())
            .build(pattern)
            .unwrap();
        let actual = candidate.resource_layout();
        let metadata = NativeImmutableRegexMetadata::from_pattern(pattern, &config, 1).unwrap();
        require_metadata_roots(&metadata, actual).expect("fake bounded root payloads");
        let mut small_regex_root = metadata;
        small_regex_root.regex_arc_bytes = native_arc_value_layout(actual.regex_payload())
            .unwrap()
            .size()
            .checked_sub(1)
            .unwrap();
        assert_eq!(
            require_metadata_roots(&small_regex_root, actual),
            Err(ProgramHeapError::RootLayout)
        );
        let mut small_info_root = metadata;
        small_info_root.info_arc_bytes = native_arc_value_layout(actual.metadata_payload())
            .unwrap()
            .size()
            .checked_sub(1)
            .unwrap();
        assert_eq!(
            require_metadata_roots(&small_info_root, actual),
            Err(ProgramHeapError::RootLayout)
        );
    }

    #[test]
    fn fake_program_constructor_configuration_axes_refuse_independently() {
        let config = fake_config();
        for changed in [
            config.clone().match_kind(MatchKind::All),
            config.clone().utf8_empty(false),
            config.clone().which_captures(WhichCaptures::None),
            config.clone().byte_classes(false),
            config.clone().line_terminator(b'\r'),
            config.clone().nfa_size_limit(Some(0x0000_ffff)),
            config.clone().nfa_size_limit(None),
            config.clone().hybrid_cache_capacity(0x0000_ffff),
            config.clone().onepass_size_limit(Some(0x000f_ffff)),
            config.clone().onepass_size_limit(None),
            config.clone().pool_capacity(2),
            config.clone().hybrid(false),
            config.clone().onepass(false),
            config.clone().auto_prefilter(false),
            config
                .clone()
                .prefilter(Prefilter::new(MatchKind::LeftmostFirst, &[b"q"])),
        ] {
            assert_eq!(
                NativeProgramHeap::from_pattern("^fake[0-9]+$", &changed),
                Err(ProgramHeapError::Configuration)
            );
        }
        require_engine_flags(true, false, true).expect("fake admitted engine flags");
        // The pinned feature set omits dfa-build; its public getter masks dfa(true).
        for flags in [
            (false, false, true),
            (true, true, true),
            (true, false, false),
        ] {
            assert_eq!(
                require_engine_flags(flags.0, flags.1, flags.2),
                Err(ProgramHeapError::Configuration)
            );
        }
    }

    #[test]
    fn fake_program_anchored_and_suffix_owners_keep_their_prefilter_disposition() {
        let config = fake_config();
        let anchored = NativeProgramHeap::from_pattern("^fake[0-9]+$", &config).unwrap();
        assert_eq!(
            anchored.automatic_prefilter,
            AutomaticPrefilterHeap::ExcludedStartAnchored
        );
        assert_eq!(anchored.automatic_prefilter_bytes, 0);
        let suffix = NativeProgramHeap::from_pattern("q[a-z]{2,3}$", &config).unwrap();
        assert!(matches!(suffix.automatic_prefilter,
            AutomaticPrefilterHeap::SingleByte { byte: b'q', requested_bytes } if requested_bytes > 0));
        assert_eq!(
            suffix.automatic_prefilter_bytes,
            suffix.automatic_prefilter.requested_bytes()
        );
        assert_eq!(
            suffix.automatic_prefilter_bytes,
            if usize::BITS == 64 { 24 } else { 12 }
        );
        assert_eq!(
            suffix.total,
            [
                suffix.metadata_bytes,
                suffix.strategy_arc_bytes,
                suffix.nfa_bytes,
                suffix.onepass_tables_bytes,
                suffix.automatic_prefilter_bytes,
                suffix.unused_pool_bytes
            ]
            .into_iter()
            .try_fold(0_usize, usize::checked_add)
            .unwrap()
        );
    }

    #[test]
    fn fake_program_unproved_components_and_invalid_syntax_refuse() {
        let config = fake_config();
        assert_eq!(
            NativeProgramHeap::from_pattern("qr[a-z]+$", &config),
            Err(ProgramHeapError::AutomaticPrefilter(
                AutomaticPrefilterError::Scope
            ))
        );
        assert_eq!(
            NativeProgramHeap::from_pattern("^(?P<fake_capture>fake)$", &config),
            Err(ProgramHeapError::Nfa)
        );
        assert_eq!(
            NativeProgramHeap::from_pattern("[", &config),
            Err(ProgramHeapError::Compilation)
        );
    }
}

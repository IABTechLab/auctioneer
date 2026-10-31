//! Conditional native retained requested heap of one strictly configured regex.
//! Total excludes code/data/static linkage, static inline storage, constructor
//! peaks, caches, search work, allocator/RSS overhead, guest and serving qualification.
//! Native reconstruction does not establish serving-release layout correspondence.

use crate::schema_cache_bounds::{forward_nfa, reverse_nfa};
use crate::schema_heap_layout::{LayoutError, native_arc_value_layout};
use crate::schema_literal_heap::{AutomaticPrefilterError, AutomaticPrefilterHeap};
use crate::schema_metadata_heap::{MetadataHeapError, NativeImmutableRegexMetadata};
use crate::schema_nfa_heap::{NfaHeapError, NfaRetainedHeap};
use crate::schema_onepass_bounds::{OnePassTablesError, nfa_table_upper_bound};
use crate::schema_pool_heap::{PoolHeapError, UnusedPoolHeap};
use regex_automata::{
    MatchKind,
    meta::{Config, Regex, RegexResourceLayout},
    nfa::thompson::WhichCaptures,
    util::syntax,
};

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

impl NativeProgramHeap {
    pub(crate) fn from_pattern(pattern: &str, config: &Config) -> Result<Self, ProgramHeapError> {
        require_configuration(config)?;
        let candidate = Regex::builder()
            .configure(config.clone())
            .syntax(syntax::Config::default().utf8(true))
            .build(pattern)
            .map_err(|_error| ProgramHeapError::Compilation)?;
        let automatic_prefilter = AutomaticPrefilterHeap::from_pattern(pattern, config)
            .map_err(ProgramHeapError::AutomaticPrefilter)?;
        let metadata =
            NativeImmutableRegexMetadata::from_pattern(pattern, config, 1).map_err(|error| {
                match error {
                    MetadataHeapError::Arithmetic => ProgramHeapError::Arithmetic,
                    MetadataHeapError::Shape
                    | MetadataHeapError::PatternCount
                    | MetadataHeapError::Configuration
                    | MetadataHeapError::Syntax => ProgramHeapError::Metadata,
                }
            })?;
        let actual = candidate.resource_layout();
        require_metadata_roots(&metadata, actual)?;
        let metadata_bytes = metadata
            .population(1, false)
            .map_err(|error| match error {
                MetadataHeapError::Arithmetic => ProgramHeapError::Arithmetic,
                MetadataHeapError::Shape
                | MetadataHeapError::PatternCount
                | MetadataHeapError::Configuration
                | MetadataHeapError::Syntax => ProgramHeapError::Metadata,
            })?
            .requested_heap_bytes;
        // Metadata already charges RegexI and RegexInfo Arcs. Only the concrete
        // strategy Arc is additional; its inline wrappers contain shared handles.
        let strategy_arc_bytes = native_arc_value_layout(actual.strategy_payload())
            .map_err(layout_error)?
            .size();

        // The strict config matches these builders' UTF-8/capture/LF/limit flags.
        // Core shares each direction's NFA across its wrappers. Start anchoring
        // or the proved end-anchored fast prefilter excludes extra prefix NFAs.
        let forward = forward_nfa(pattern).map_err(|_error| ProgramHeapError::Nfa)?;
        let reverse = reverse_nfa(pattern).map_err(|_error| ProgramHeapError::Nfa)?;
        let mut nfa_bytes = 0_usize;
        for nfa in [&forward, &reverse] {
            let tree = NfaRetainedHeap::from_nfa(nfa).map_err(|error| match error {
                NfaHeapError::Arithmetic | NfaHeapError::Layout(LayoutError::Arithmetic) => {
                    ProgramHeapError::Arithmetic
                }
                NfaHeapError::Layout(LayoutError::Shape)
                | NfaHeapError::NamesUnqualified
                | NfaHeapError::Shape
                | NfaHeapError::Compilation
                | NfaHeapError::Prefix(_) => ProgramHeapError::Nfa,
            })?;
            if tree.requested_heap_bytes == 0 {
                return Err(ProgramHeapError::Shape);
            }
            nfa_bytes = nfa_bytes
                .checked_add(tree.requested_heap_bytes)
                .ok_or(ProgramHeapError::Arithmetic)?;
        }
        let shift = u32::try_from(forward.byte_classes().stride2())
            .map_err(|_error| ProgramHeapError::Arithmetic)?;
        let stride = 1_usize
            .checked_shl(shift)
            .ok_or(ProgramHeapError::Arithmetic)?;
        // Fund the retained-table envelope even when meta skips this optional
        // candidate. Its shared forward NFA and inline DFA are charged above.
        let onepass_tables_bytes =
            nfa_table_upper_bound(forward.states().len(), stride, forward.pattern_len())
                .map_err(|error| match error {
                    OnePassTablesError::Arithmetic => ProgramHeapError::Arithmetic,
                    OnePassTablesError::Configuration
                    | OnePassTablesError::Compilation
                    | OnePassTablesError::Shape => ProgramHeapError::OnePass,
                })?
                .total;
        let unused_pool_bytes = UnusedPoolHeap::native(config)
            .map_err(|error| match error {
                PoolHeapError::Arithmetic => ProgramHeapError::Arithmetic,
                PoolHeapError::Shape | PoolHeapError::Configuration => ProgramHeapError::Pool,
            })?
            .requested_bytes;
        Self::compose(
            [
                metadata_bytes,
                strategy_arc_bytes,
                nfa_bytes,
                onepass_tables_bytes,
                automatic_prefilter.requested_bytes(),
                unused_pool_bytes,
            ],
            metadata.static_inline_bytes,
            automatic_prefilter,
        )
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

fn require_configuration(config: &Config) -> Result<(), ProgramHeapError> {
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

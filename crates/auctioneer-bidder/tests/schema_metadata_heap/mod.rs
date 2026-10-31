//! Native immutable metadata only, excluding engines, caches and constructor scratch.

use super::schema_heap_layout::{
    LayoutError, NativeField, native_arc_layout, private_struct_slot_bound,
};
use regex_automata::meta::{Config, Regex};
use regex_syntax::{ParserBuilder, hir::Properties};
use std::sync::{Arc, LazyLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MetadataHeapError {
    Shape,
    Arithmetic,
    PatternCount,
    Configuration,
    Syntax,
}

impl From<LayoutError> for MetadataHeapError {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Shape => Self::Shape,
            LayoutError::Arithmetic => Self::Arithmetic,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MetadataLayout {
    regex_fields: [NativeField; 2],
    info_fields: [NativeField; 3],
    props_payload: NativeField,
    static_inline: NativeField,
}

impl MetadataLayout {
    fn native(properties: &Properties) -> Self {
        Self {
            // RegexI: one fat Arc<dyn Strategy> handle and one thin RegexInfo
            // handle. The strategy allocation itself is not metadata.
            regex_fields: [
                NativeField::of::<Arc<dyn Send + Sync>>(),
                NativeField::of::<Arc<()>>(),
            ],
            // RegexInfoI's public sized fields; no private ABI mirror.
            info_fields: [
                NativeField::of::<Config>(),
                NativeField::of::<Vec<Properties>>(),
                NativeField::of::<Properties>(),
            ],
            // regex-syntax 0.8.11 hir/mod.rs:1964-1990,2236: PropertiesI is
            // scalar-only, aligned at most as usize; memory_usage is its exact
            // size, not the Box handle or a recursive heap estimate.
            props_payload: NativeField {
                bytes: properties.memory_usage(),
                alignment: align_of::<usize>(),
            },
            static_inline: NativeField::of::<LazyLock<Option<Regex>>>(),
        }
    }

    fn metadata(self) -> Result<NativeImmutableRegexMetadata, MetadataHeapError> {
        for field in self
            .regex_fields
            .into_iter()
            .chain(self.info_fields)
            .chain([self.props_payload, self.static_inline])
        {
            field.validate()?;
        }
        // RegexInfo::new pushes one clone into a fresh Vec. Pinned native
        // RawVec's amortized minimum is 4 for a Properties-sized element.
        // grow_amortized/finish_grow requests that exact capacity, not Vec's
        // public "at least" contract. The union owns a second PropertiesI Box.
        let props_capacity = 4_usize;
        let props_vec_bytes = self.info_fields[2]
            .bytes
            .checked_mul(props_capacity)
            .ok_or(MetadataHeapError::Arithmetic)?;
        NativeField {
            bytes: props_vec_bytes,
            alignment: self.info_fields[2].alignment,
        }
        .validate()?;
        let regex_arc_bytes =
            native_arc_layout(private_struct_slot_bound(&self.regex_fields)?)?.size();
        // Any strategy RegexInfo references share this allocation.
        let info_arc_bytes =
            native_arc_layout(private_struct_slot_bound(&self.info_fields)?)?.size();
        let props_box_bytes = self.props_payload.bytes;
        let props_union_box_bytes = self.props_payload.bytes;
        let requested_heap_bytes = checked_sum(&[
            regex_arc_bytes,
            info_arc_bytes,
            props_vec_bytes,
            props_box_bytes,
            props_union_box_bytes,
        ])?;
        Ok(NativeImmutableRegexMetadata {
            regex_arc_bytes,
            info_arc_bytes,
            props_capacity,
            props_vec_bytes,
            props_box_bytes,
            props_union_box_bytes,
            requested_heap_bytes,
            static_inline_bytes: self.static_inline.bytes,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeImmutableRegexMetadata {
    pub regex_arc_bytes: usize,
    pub info_arc_bytes: usize,
    pub props_capacity: usize,
    pub props_vec_bytes: usize,
    pub props_box_bytes: usize,
    pub props_union_box_bytes: usize,
    pub requested_heap_bytes: usize,
    pub static_inline_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MetadataPopulation {
    pub owners: usize,
    pub requested_heap_bytes: usize,
    pub static_inline_bytes: usize,
}

impl NativeImmutableRegexMetadata {
    pub(crate) fn from_pattern(
        pattern: &str,
        config: &Config,
        pattern_count: usize,
    ) -> Result<Self, MetadataHeapError> {
        // Config's only possible heap-owning field is an explicitly configured
        // prefilter. Automatic strategy prefilters remain outside this model.
        if config.get_prefilter().is_some() {
            return Err(MetadataHeapError::Configuration);
        }
        if pattern_count != 1 {
            return Err(MetadataHeapError::PatternCount);
        }
        let mut parser = ParserBuilder::new().utf8(true).build();
        let hir = parser
            .parse(pattern)
            .map_err(|_error| MetadataHeapError::Syntax)?;
        MetadataLayout::native(hir.properties()).metadata()
    }

    pub(crate) fn population(
        self,
        helpers: usize,
        hostname: bool,
    ) -> Result<MetadataPopulation, MetadataHeapError> {
        let terms = [
            self.regex_arc_bytes,
            self.info_arc_bytes,
            self.props_vec_bytes,
            self.props_box_bytes,
            self.props_union_box_bytes,
        ];
        if terms.contains(&0)
            || self.static_inline_bytes == 0
            || self.props_capacity != 4
            || self.props_box_bytes != self.props_union_box_bytes
        {
            return Err(MetadataHeapError::Shape);
        }
        if checked_sum(&terms)? != self.requested_heap_bytes {
            return Err(MetadataHeapError::Shape);
        }
        let owners = helpers
            .checked_add(usize::from(hostname))
            .ok_or(MetadataHeapError::Arithmetic)?;
        Ok(MetadataPopulation {
            owners,
            requested_heap_bytes: owners
                .checked_mul(self.requested_heap_bytes)
                .ok_or(MetadataHeapError::Arithmetic)?,
            static_inline_bytes: owners
                .checked_mul(self.static_inline_bytes)
                .ok_or(MetadataHeapError::Arithmetic)?,
        })
    }
}

fn checked_sum(terms: &[usize]) -> Result<usize, MetadataHeapError> {
    terms.iter().try_fold(0_usize, |sum, term| {
        sum.checked_add(*term).ok_or(MetadataHeapError::Arithmetic)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex_automata::{MatchKind, util::prefilter::Prefilter};
    use regex_syntax::parse;

    const SYNTHETIC: MetadataLayout = MetadataLayout {
        regex_fields: [
            NativeField {
                bytes: 16,
                alignment: 8,
            },
            NativeField {
                bytes: 8,
                alignment: 8,
            },
        ],
        info_fields: [
            NativeField {
                bytes: 40,
                alignment: 8,
            },
            NativeField {
                bytes: 24,
                alignment: 8,
            },
            NativeField {
                bytes: 8,
                alignment: 8,
            },
        ],
        props_payload: NativeField {
            bytes: 80,
            alignment: 8,
        },
        static_inline: NativeField {
            bytes: 64,
            alignment: 8,
        },
    };

    #[test]
    fn metadata_literal_terms_charge_two_arcs_capacity_four_and_two_property_boxes() {
        assert_eq!(
            SYNTHETIC.metadata(),
            Ok(NativeImmutableRegexMetadata {
                regex_arc_bytes: 40,
                info_arc_bytes: 88,
                props_capacity: 4,
                props_vec_bytes: 32,
                props_box_bytes: 80,
                props_union_box_bytes: 80,
                requested_heap_bytes: 320,
                static_inline_bytes: 64,
            })
        );
    }

    #[test]
    fn metadata_each_private_field_payload_and_inline_slot_changes_its_own_term() {
        for index in 0_usize..2 {
            let mut layout = SYNTHETIC;
            layout.regex_fields[index].bytes += 8;
            let bound = layout.metadata().unwrap();
            assert_eq!(bound.regex_arc_bytes, 48);
            assert_eq!(bound.info_arc_bytes, 88);
            assert_eq!(bound.requested_heap_bytes, 328);
        }
        for index in 0_usize..3 {
            let mut layout = SYNTHETIC;
            layout.info_fields[index].bytes += 8;
            let bound = layout.metadata().unwrap();
            assert_eq!(bound.regex_arc_bytes, 40);
            assert_eq!(bound.info_arc_bytes, 96);
            assert_eq!(bound.props_vec_bytes, if index == 2 { 64 } else { 32 });
            assert_eq!(
                bound.requested_heap_bytes,
                if index == 2 { 360 } else { 328 }
            );
        }
        let mut payload = SYNTHETIC;
        payload.props_payload.bytes = 88;
        let payload_bound = payload.metadata().unwrap();
        assert_eq!(payload_bound.props_box_bytes, 88);
        assert_eq!(payload_bound.props_union_box_bytes, 88);
        assert_eq!(payload_bound.requested_heap_bytes, 336);
        let mut inline = SYNTHETIC;
        inline.static_inline.bytes = 72;
        let inline_bound = inline.metadata().unwrap();
        assert_eq!(inline_bound.requested_heap_bytes, 320);
        assert_eq!(inline_bound.static_inline_bytes, 72);
    }

    #[test]
    fn metadata_population_counts_the_shared_hostname_once_and_keeps_inline_separate() {
        let bound = SYNTHETIC.metadata().unwrap();
        for (helpers, hostname, owners, heap, inline) in [
            (0_usize, false, 0_usize, 0_usize, 0_usize),
            (0, true, 1, 320, 64),
            (3, false, 3, 960, 192),
            (3, true, 4, 1280, 256),
            (23, true, 24, 7680, 1536),
        ] {
            assert_eq!(
                bound.population(helpers, hostname),
                Ok(MetadataPopulation {
                    owners,
                    requested_heap_bytes: heap,
                    static_inline_bytes: inline,
                })
            );
        }
        for hostname in [false, true] {
            assert_eq!(
                bound.population(usize::MAX, hostname),
                Err(MetadataHeapError::Arithmetic)
            );
        }
        let zero = NativeImmutableRegexMetadata {
            regex_arc_bytes: 0,
            info_arc_bytes: 0,
            props_capacity: 0,
            props_vec_bytes: 0,
            props_box_bytes: 0,
            props_union_box_bytes: 0,
            requested_heap_bytes: 0,
            static_inline_bytes: 0,
        };
        assert_eq!(zero.population(23, true), Err(MetadataHeapError::Shape));
        let mut wrong_capacity = bound;
        wrong_capacity.props_capacity = 1;
        assert_eq!(
            wrong_capacity.population(23, true),
            Err(MetadataHeapError::Shape)
        );
        let mut missing_union = bound;
        missing_union.props_union_box_bytes = 0;
        assert_eq!(
            missing_union.population(23, true),
            Err(MetadataHeapError::Shape)
        );
        let mut wrong_total = bound;
        wrong_total.requested_heap_bytes = 240;
        assert_eq!(
            wrong_total.population(23, true),
            Err(MetadataHeapError::Shape)
        );
        let mut inline_overflow = bound;
        inline_overflow.static_inline_bytes = usize::MAX;
        assert_eq!(
            inline_overflow.population(2, false),
            Err(MetadataHeapError::Arithmetic)
        );
    }

    #[test]
    fn metadata_population_rejects_missing_terms_without_other_shape_failures() {
        let bound = SYNTHETIC.metadata().unwrap();
        for (terms, requested_heap_bytes) in [
            ([0_usize, 88, 32, 80, 80], 280_usize),
            ([40, 0, 32, 80, 80], 232),
            ([40, 88, 0, 80, 80], 288),
            ([40, 88, 32, 0, 0], 160),
        ] {
            let missing = NativeImmutableRegexMetadata {
                regex_arc_bytes: terms[0],
                info_arc_bytes: terms[1],
                props_vec_bytes: terms[2],
                props_box_bytes: terms[3],
                props_union_box_bytes: terms[4],
                requested_heap_bytes,
                ..bound
            };
            assert_eq!(missing.population(23, true), Err(MetadataHeapError::Shape));
        }
        let missing_inline = NativeImmutableRegexMetadata {
            static_inline_bytes: 0,
            ..bound
        };
        assert_eq!(
            missing_inline.population(23, true),
            Err(MetadataHeapError::Shape)
        );
        let unequal_boxes = NativeImmutableRegexMetadata {
            props_union_box_bytes: 88,
            requested_heap_bytes: 328,
            ..bound
        };
        assert_eq!(
            unequal_boxes.population(23, true),
            Err(MetadataHeapError::Shape)
        );
    }

    #[test]
    fn metadata_population_heap_multiplication_overflow_is_independent_of_inline() {
        let bound = NativeImmutableRegexMetadata {
            static_inline_bytes: 1,
            ..SYNTHETIC.metadata().unwrap()
        };
        let helpers = usize::MAX.checked_div(320).unwrap().checked_add(1).unwrap();
        assert_eq!(bound.requested_heap_bytes, 320);
        assert_eq!(
            helpers.checked_mul(bound.static_inline_bytes),
            Some(helpers)
        );
        assert_eq!(
            bound.population(helpers, false),
            Err(MetadataHeapError::Arithmetic)
        );
    }

    #[test]
    fn metadata_every_malformed_field_and_native_allocation_overflow_is_refused() {
        for malformed in [
            NativeField {
                bytes: 0,
                alignment: 8,
            },
            NativeField {
                bytes: 8,
                alignment: 0,
            },
            NativeField {
                bytes: 8,
                alignment: 3,
            },
            NativeField {
                bytes: 9,
                alignment: 8,
            },
        ] {
            for index in 0_usize..7 {
                let mut layout = SYNTHETIC;
                match index {
                    0..=1 => layout.regex_fields[index] = malformed,
                    2..=4 => layout.info_fields[index - 2] = malformed,
                    5 => layout.props_payload = malformed,
                    _ => layout.static_inline = malformed,
                }
                assert_eq!(layout.metadata(), Err(MetadataHeapError::Shape));
            }
        }
        let maximum = usize::try_from(isize::MAX).unwrap();
        let huge = NativeField {
            bytes: maximum - 7,
            alignment: 8,
        };
        for index in 0_usize..7 {
            let mut layout = SYNTHETIC;
            match index {
                0..=1 => layout.regex_fields[index] = huge,
                2..=4 => layout.info_fields[index - 2] = huge,
                5 => {
                    layout.props_payload = NativeField {
                        bytes: usize::MAX - 7,
                        alignment: 8,
                    }
                }
                _ => {
                    layout.static_inline = NativeField {
                        bytes: usize::MAX - 7,
                        alignment: 8,
                    }
                }
            }
            assert_eq!(layout.metadata(), Err(MetadataHeapError::Arithmetic));
        }
        // Distinguish usize multiplication overflow from a product that fits
        // usize but exceeds the native allocation request limit.
        for element_bytes in [1_usize << (usize::BITS - 2), 1_usize << (usize::BITS - 3)] {
            let mut vector_overflow = SYNTHETIC;
            vector_overflow.info_fields[2].bytes = element_bytes;
            assert_eq!(
                vector_overflow.metadata(),
                Err(MetadataHeapError::Arithmetic)
            );
        }
        // Each allocation is legal here, but the five-term sum overflows.
        let mut total_overflow = SYNTHETIC;
        total_overflow.props_payload.bytes = maximum - 7;
        total_overflow.regex_fields[0].bytes = 128;
        assert_eq!(
            total_overflow.metadata(),
            Err(MetadataHeapError::Arithmetic)
        );
    }

    #[test]
    fn metadata_native_public_types_and_static_lazylock_are_compiled_not_mirrored() {
        let hir = parse("^synthetic+$").unwrap();
        let layout = MetadataLayout::native(hir.properties());
        assert_eq!(
            layout.regex_fields[0],
            NativeField::of::<Arc<dyn Send + Sync>>()
        );
        assert_eq!(layout.regex_fields[1], NativeField::of::<Arc<()>>());
        assert_eq!(layout.regex_fields[0].bytes, 16);
        assert_eq!(layout.regex_fields[1].bytes, 8);
        assert_eq!(
            layout.info_fields,
            [
                NativeField::of::<Config>(),
                NativeField::of::<Vec<Properties>>(),
                NativeField::of::<Properties>()
            ]
        );
        assert_eq!(layout.props_payload.bytes, hir.properties().memory_usage());
        assert_eq!(layout.props_payload.alignment, align_of::<usize>());
        assert_eq!(
            layout.static_inline,
            NativeField::of::<LazyLock<Option<Regex>>>()
        );
        let config = Regex::config();
        let bound = NativeImmutableRegexMetadata::from_pattern("^synthetic+$", &config, 1).unwrap();
        assert_eq!(bound, layout.metadata().unwrap());
        assert_eq!(
            bound.static_inline_bytes,
            size_of::<LazyLock<Option<Regex>>>()
        );
        assert_eq!(bound.props_capacity, 4);
    }

    #[test]
    fn metadata_refuses_zero_and_multiple_patterns_and_actual_configured_prefilters() {
        let config = Regex::config();
        for count in [0_usize, 2, usize::MAX] {
            assert_eq!(
                NativeImmutableRegexMetadata::from_pattern("secret-pattern", &config, count),
                Err(MetadataHeapError::PatternCount)
            );
        }
        let prefilter = Prefilter::new(MatchKind::LeftmostFirst, &[b"synthetic"]).unwrap();
        let configured = Regex::config().prefilter(Some(prefilter));
        assert!(configured.get_prefilter().is_some());
        assert_eq!(
            NativeImmutableRegexMetadata::from_pattern("secret-pattern", &configured, 1),
            Err(MetadataHeapError::Configuration)
        );
        let overridden = configured.prefilter(None);
        assert!(overridden.get_prefilter().is_none());
        let accepted =
            NativeImmutableRegexMetadata::from_pattern("synthetic", &overridden, 1).unwrap();
        assert_eq!(accepted.props_capacity, 4);
        assert_eq!(
            NativeImmutableRegexMetadata::from_pattern("[secret-pattern", &config, 1),
            Err(MetadataHeapError::Syntax)
        );
    }
}

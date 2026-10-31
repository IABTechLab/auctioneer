//! Requested allocation layouts for pinned collections, not allocator/RSS bounds.

use std::alloc::Layout;
use std::sync::atomic::AtomicUsize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeField {
    pub bytes: usize,
    pub alignment: usize,
}

impl NativeField {
    pub(crate) fn of<T>() -> Self {
        Self {
            bytes: size_of::<T>(),
            alignment: align_of::<T>(),
        }
    }

    pub(crate) fn validate(self) -> Result<(), LayoutError> {
        if self.bytes == 0
            || !self.alignment.is_power_of_two()
            || !self.bytes.is_multiple_of(self.alignment)
        {
            return Err(LayoutError::Shape);
        }
        Layout::from_size_align(self.bytes, self.alignment)
            .map(|_layout| ())
            .map_err(|_error| LayoutError::Arithmetic)
    }
}

pub(crate) fn private_struct_slot_bound(
    fields: &[NativeField],
) -> Result<NativeField, LayoutError> {
    for field in fields {
        field.validate()?;
    }
    let alignment = fields
        .iter()
        .map(|field| field.alignment)
        .max()
        .ok_or(LayoutError::Shape)?;
    // Maximum-aligned slots cover all natural gaps and tail padding for any
    // field order. This is a bound, not a mirror of private repr(Rust) types.
    let bytes = fields.iter().try_fold(0_usize, |sum, field| {
        let slot = field
            .bytes
            .checked_next_multiple_of(alignment)
            .ok_or(LayoutError::Arithmetic)?;
        sum.checked_add(slot).ok_or(LayoutError::Arithmetic)
    })?;
    let bound = NativeField { bytes, alignment };
    bound.validate()?;
    Ok(bound)
}

pub(crate) fn native_arc_layout(payload: NativeField) -> Result<Layout, LayoutError> {
    payload.validate()?;
    let payload_layout = Layout::from_size_align(payload.bytes, payload.alignment)
        .map_err(|_error| LayoutError::Arithmetic)?;
    native_arc_value_layout(payload_layout)
}

pub(crate) fn native_arc_value_layout(payload: Layout) -> Result<Layout, LayoutError> {
    // Rust 1.98.1 alloc/sync.rs:388-413: repr(C) strong/weak atomics, then
    // data; arcinner_layout_for_value_layout extends and pads that header.
    Layout::new::<[AtomicUsize; 2]>()
        .extend(payload)
        .map(|(layout, _offset)| layout.pad_to_align())
        .map_err(|_error| LayoutError::Arithmetic)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LayoutError {
    Shape,
    Arithmetic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AllocationEnvelope {
    pub retained_bytes: usize,
    pub old_growth_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HeapLayout {
    word_bytes: usize,
    group_bytes: usize,
}

impl HeapLayout {
    pub(crate) fn new(word_bytes: usize, group_bytes: usize) -> Result<Self, LayoutError> {
        if !matches!(word_bytes, 4 | 8)
            || !matches!(group_bytes, 4 | 8 | 16)
            || group_bytes < word_bytes
        {
            return Err(LayoutError::Shape);
        }
        Ok(Self {
            word_bytes,
            group_bytes,
        })
    }

    pub(crate) fn word_bytes(self) -> usize {
        self.word_bytes
    }

    pub(crate) fn repeat_vector(
        self,
        element_bytes: usize,
        entries: usize,
    ) -> Result<usize, LayoutError> {
        if element_bytes == 0 {
            return Err(LayoutError::Shape);
        }
        let bytes = entries
            .checked_mul(element_bytes)
            .ok_or(LayoutError::Arithmetic)?;
        self.check_request(bytes, 1)?;
        Ok(bytes)
    }

    pub(crate) fn fresh_resize_vector(
        self,
        element_bytes: usize,
        entries: usize,
    ) -> Result<usize, LayoutError> {
        if element_bytes == 0 {
            return Err(LayoutError::Shape);
        }
        let minimum = if element_bytes == 1 {
            8
        } else if element_bytes <= 1024 {
            4
        } else {
            1
        };
        let capacity = if entries == 0 {
            0
        } else {
            entries.max(minimum)
        };
        self.repeat_vector(element_bytes, capacity)
    }

    pub(crate) fn append_vector(
        self,
        element_bytes: usize,
        high_water: usize,
    ) -> Result<AllocationEnvelope, LayoutError> {
        if element_bytes == 0 {
            return Err(LayoutError::Shape);
        }
        if high_water == 0 {
            return Ok(AllocationEnvelope {
                retained_bytes: 0,
                old_growth_bytes: 0,
            });
        }
        let minimum = if element_bytes == 1 {
            8
        } else if element_bytes <= 1024 {
            4
        } else {
            1
        };
        // RawVec's amortized branch is max(2*old_capacity, required, minimum).
        // On these append paths old_capacity < required <= lifetime high_water.
        let capacity = high_water
            .checked_mul(2)
            .ok_or(LayoutError::Arithmetic)?
            .max(minimum);
        let retained_bytes = capacity
            .checked_mul(element_bytes)
            .ok_or(LayoutError::Arithmetic)?;
        let old_growth_bytes = if high_water <= minimum {
            0
        } else {
            high_water
                .checked_sub(1)
                .and_then(|entries| entries.checked_mul(element_bytes))
                .ok_or(LayoutError::Arithmetic)?
        };
        self.check_request(retained_bytes, 1)?;
        Ok(AllocationEnvelope {
            retained_bytes,
            old_growth_bytes,
        })
    }

    /// Fixed cache history: new/insert/clear only, with no explicit reserve/shrink.
    pub(crate) fn state_map(self, high_water: usize) -> Result<AllocationEnvelope, LayoutError> {
        if high_water == 0 {
            return Ok(AllocationEnvelope {
                retained_bytes: 0,
                old_growth_bytes: 0,
            });
        }
        // An insertion attempt may reserve before discovering a duplicate key.
        let requested = high_water.checked_add(1).ok_or(LayoutError::Arithmetic)?;
        let buckets = if requested < 4 {
            4
        } else if requested < 8 {
            8
        } else if requested < 15 {
            16
        } else {
            requested
                .checked_mul(8)
                .and_then(|scaled| scaled.checked_div(7))
                .and_then(usize::checked_next_power_of_two)
                .ok_or(LayoutError::Arithmetic)?
        };
        let retained_bytes = self.state_table_bytes(buckets)?;
        let old_growth_bytes = if buckets == 4 {
            0
        } else {
            self.state_table_bytes(buckets.checked_div(2).ok_or(LayoutError::Arithmetic)?)?
        };
        Ok(AllocationEnvelope {
            retained_bytes,
            old_growth_bytes,
        })
    }

    pub(crate) fn arc_slice(self, payload_bytes: usize) -> Result<usize, LayoutError> {
        let inner = self
            .word_bytes
            .checked_mul(2)
            .and_then(|header| header.checked_add(payload_bytes))
            .ok_or(LayoutError::Arithmetic)?;
        let bytes = align_up(inner, self.word_bytes)?;
        self.check_request(bytes, self.word_bytes)?;
        Ok(bytes)
    }

    fn state_table_bytes(self, buckets: usize) -> Result<usize, LayoutError> {
        // State is Arc<[u8]> (two words); LazyStateID is u32, rounded as a pair.
        let pair = self
            .word_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or(LayoutError::Arithmetic)?;
        let pair_bytes = align_up(pair, self.word_bytes)?;
        let controls = align_up(
            pair_bytes
                .checked_mul(buckets)
                .ok_or(LayoutError::Arithmetic)?,
            self.group_bytes,
        )?;
        let bytes = controls
            .checked_add(buckets)
            .and_then(|bytes| bytes.checked_add(self.group_bytes))
            .ok_or(LayoutError::Arithmetic)?;
        self.check_request(bytes, self.group_bytes)?;
        Ok(bytes)
    }

    fn check_request(self, bytes: usize, alignment: usize) -> Result<(), LayoutError> {
        let padding = alignment.checked_sub(1).ok_or(LayoutError::Shape)?;
        let maximum = if self.word_bytes == 4 {
            usize::try_from(i32::MAX).map_err(|_error| LayoutError::Arithmetic)?
        } else {
            usize::MAX.checked_div(2).ok_or(LayoutError::Arithmetic)?
        };
        if bytes
            > maximum
                .checked_sub(padding)
                .ok_or(LayoutError::Arithmetic)?
        {
            return Err(LayoutError::Arithmetic);
        }
        Ok(())
    }
}

fn align_up(bytes: usize, alignment: usize) -> Result<usize, LayoutError> {
    let padding = alignment.checked_sub(1).ok_or(LayoutError::Shape)?;
    bytes
        .checked_add(padding)
        .map(|padded| padded & !padding)
        .ok_or(LayoutError::Arithmetic)
}

#[cfg(test)]
mod tests {
    use super::{
        AllocationEnvelope, HeapLayout, LayoutError, NativeField, native_arc_layout,
        native_arc_value_layout, private_struct_slot_bound,
    };
    use std::alloc::Layout;

    #[test]
    fn compiled_value_arc_layout_funds_zst_headers_without_treating_them_as_absent() {
        assert_eq!(size_of::<usize>(), 8, "native qualification fixture");
        for (bytes, alignment, total, allocation_alignment) in [
            (0_usize, 1_usize, 16_usize, 8_usize),
            (0, 64, 64, 64),
            (1, 1, 24, 8),
            (24, 8, 40, 8),
            (32, 32, 64, 32),
        ] {
            let payload = Layout::from_size_align(bytes, alignment).unwrap();
            let allocation = native_arc_value_layout(payload).unwrap();
            assert_eq!(allocation.size(), total, "{bytes}/{alignment}");
            assert_eq!(
                allocation.align(),
                allocation_alignment,
                "{bytes}/{alignment}"
            );
        }
        assert_eq!(
            native_arc_layout(NativeField {
                bytes: 0,
                alignment: 8
            }),
            Err(LayoutError::Shape),
            "missing private fields must still refuse"
        );
    }

    #[test]
    fn compiled_value_arc_layout_refuses_header_and_alignment_overflow() {
        let maximum = usize::try_from(isize::MAX).unwrap();
        for payload in [
            Layout::from_size_align(maximum, 1).unwrap(),
            Layout::from_size_align(0, 1_usize << (usize::BITS - 1)).unwrap(),
        ] {
            assert_eq!(
                native_arc_value_layout(payload),
                Err(LayoutError::Arithmetic)
            );
        }
    }

    #[test]
    fn metadata_native_fields_validate_shape_and_native_request_limits() {
        assert_eq!(
            NativeField::of::<u32>(),
            NativeField {
                bytes: 4,
                alignment: 4
            }
        );
        assert_eq!(
            NativeField {
                bytes: 24,
                alignment: 8
            }
            .validate(),
            Ok(())
        );
        for field in [
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
            assert_eq!(field.validate(), Err(LayoutError::Shape));
            assert_eq!(private_struct_slot_bound(&[field]), Err(LayoutError::Shape));
            assert_eq!(native_arc_layout(field), Err(LayoutError::Shape));
        }
        let huge = NativeField {
            bytes: usize::MAX - 7,
            alignment: 8,
        };
        assert_eq!(huge.validate(), Err(LayoutError::Arithmetic));
        assert_eq!(native_arc_layout(huge), Err(LayoutError::Arithmetic));
    }

    #[test]
    fn metadata_private_slots_use_literal_maximum_alignment_without_field_order() {
        let fields = [
            NativeField {
                bytes: 3,
                alignment: 1,
            },
            NativeField {
                bytes: 8,
                alignment: 8,
            },
            NativeField {
                bytes: 2,
                alignment: 2,
            },
            NativeField {
                bytes: 12,
                alignment: 4,
            },
        ];
        let expected = Ok(NativeField {
            bytes: 40,
            alignment: 8,
        });
        assert_eq!(private_struct_slot_bound(&fields), expected);
        assert_eq!(
            private_struct_slot_bound(&[fields[3], fields[2], fields[1], fields[0]]),
            expected
        );
        assert_eq!(private_struct_slot_bound(&[]), Err(LayoutError::Shape));
        let huge = NativeField {
            bytes: 1_usize << (usize::BITS - 2),
            alignment: 8,
        };
        assert_eq!(
            private_struct_slot_bound(&[huge; 4]),
            Err(LayoutError::Arithmetic)
        );
        assert_eq!(
            private_struct_slot_bound(&[huge; 2]),
            Err(LayoutError::Arithmetic)
        );
        let maximum = usize::try_from(isize::MAX).unwrap();
        let almost = NativeField {
            bytes: maximum,
            alignment: 1,
        };
        assert_eq!(
            private_struct_slot_bound(&[almost, fields[1]]),
            Err(LayoutError::Arithmetic)
        );
    }

    #[test]
    fn metadata_native_arc_header_and_payload_padding_are_independent() {
        assert_eq!(size_of::<usize>(), 8, "native qualification fixture");
        for (bytes, alignment, total) in [
            (1_usize, 1_usize, 24_usize),
            (8, 8, 24),
            (24, 8, 40),
            (32, 32, 64),
            (64, 64, 128),
        ] {
            let layout = native_arc_layout(NativeField { bytes, alignment }).unwrap();
            assert_eq!(layout.size(), total);
            assert_eq!(layout.align(), alignment.max(8));
        }
        let maximum = usize::try_from(isize::MAX).unwrap();
        assert_eq!(
            native_arc_layout(NativeField {
                bytes: maximum - 7,
                alignment: 8
            }),
            Err(LayoutError::Arithmetic)
        );
    }

    const WORD64: HeapLayout = HeapLayout {
        word_bytes: 8,
        group_bytes: 16,
    };
    const WORD32: HeapLayout = HeapLayout {
        word_bytes: 4,
        group_bytes: 16,
    };

    #[test]
    fn fresh_vector_layouts_distinguish_repeat_and_single_resize_requests() {
        for (element, length, repeated, resized) in [
            (1, 0, 0, 0),
            (1, 1, 1, 8),
            (1, 9, 9, 9),
            (4, 1, 4, 16),
            (4, 4, 16, 16),
            (4, 5, 20, 20),
            (8, 2, 16, 32),
            (8, 6, 48, 48),
            (1025, 2, 2050, 2050),
        ] {
            assert_eq!(WORD64.repeat_vector(element, length), Ok(repeated));
            assert_eq!(WORD64.fresh_resize_vector(element, length), Ok(resized));
        }
    }

    #[test]
    fn fresh_vector_layouts_reject_invalid_elements_and_each_request_overflow() {
        for layout in [WORD32, WORD64] {
            for length in [0, 1] {
                assert_eq!(layout.repeat_vector(0, length), Err(LayoutError::Shape));
                assert_eq!(
                    layout.fresh_resize_vector(0, length),
                    Err(LayoutError::Shape)
                );
            }
            assert_eq!(
                layout.repeat_vector(8, usize::MAX),
                Err(LayoutError::Arithmetic)
            );
            assert_eq!(
                layout.fresh_resize_vector(8, usize::MAX),
                Err(LayoutError::Arithmetic)
            );
        }
        assert_eq!(WORD32.repeat_vector(4, 0x1FFF_FFFF), Ok(0x7FFF_FFFC));
        assert_eq!(
            WORD32.repeat_vector(4, 0x2000_0000),
            Err(LayoutError::Arithmetic)
        );
        assert_eq!(
            WORD32.fresh_resize_vector(4, 0x2000_0000),
            Err(LayoutError::Arithmetic)
        );
        assert_eq!(WORD64.repeat_vector(4, 0x2000_0000), Ok(0x8000_0000));
    }

    #[test]
    fn layout_accepts_only_reviewed_word_and_control_group_dimensions() {
        for (word, group) in [(4, 4), (4, 8), (4, 16), (8, 8), (8, 16)] {
            assert_eq!(HeapLayout::new(word, group).unwrap().word_bytes(), word);
        }
        for (word, group) in [(0, 16), (1, 16), (16, 16), (8, 0), (8, 4), (8, 32)] {
            assert_eq!(HeapLayout::new(word, group), Err(LayoutError::Shape));
        }
    }

    #[test]
    fn append_vector_envelopes_pin_minimum_capacity_and_growth_overlap() {
        for (element, high_water, retained, old) in [
            (1, 0, 0, 0),
            (1, 1, 8, 0),
            (1, 8, 16, 0),
            (1, 9, 18, 8),
            (4, 1, 16, 0),
            (4, 4, 32, 0),
            (4, 5, 40, 16),
            (16, 17, 544, 256),
            (1024, 5, 10_240, 4096),
            (1025, 2, 4100, 1025),
        ] {
            assert_eq!(
                WORD64.append_vector(element, high_water),
                Ok(AllocationEnvelope {
                    retained_bytes: retained,
                    old_growth_bytes: old
                }),
                "element={element}, high_water={high_water}"
            );
        }
        assert_eq!(WORD64.append_vector(0, 1), Err(LayoutError::Shape));
        assert_eq!(
            WORD64.append_vector(4, usize::MAX),
            Err(LayoutError::Arithmetic)
        );
        assert_eq!(
            WORD64.append_vector(usize::MAX, 1),
            Err(LayoutError::Arithmetic)
        );
        assert_eq!(
            WORD32.append_vector(4, 1 << 29_u32),
            Err(LayoutError::Arithmetic)
        );
    }

    #[test]
    fn state_map_envelopes_pin_bucket_boundaries_and_overlapping_old_tables() {
        for (layout, count, retained, old) in [
            (WORD64, 0, 0, 0),
            (WORD64, 1, 116, 0),
            (WORD64, 2, 116, 0),
            (WORD64, 3, 216, 116),
            (WORD64, 7, 416, 216),
            (WORD64, 14, 816, 416),
            (WORD64, 28, 1616, 816),
            (WORD32, 1, 68, 0),
            (WORD32, 3, 120, 68),
            (WORD32, 7, 224, 120),
        ] {
            assert_eq!(
                layout.state_map(count),
                Ok(AllocationEnvelope {
                    retained_bytes: retained,
                    old_growth_bytes: old
                }),
                "word={}, count={count}",
                layout.word_bytes()
            );
        }
        assert_eq!(
            HeapLayout {
                word_bytes: 8,
                group_bytes: 8
            }
            .state_map(1),
            Ok(AllocationEnvelope {
                retained_bytes: 108,
                old_growth_bytes: 0
            })
        );
        for layout in [WORD32, WORD64] {
            assert_eq!(layout.state_map(usize::MAX), Err(LayoutError::Arithmetic));
            assert_eq!(
                layout.state_map(usize::MAX.checked_div(8).unwrap()),
                Err(LayoutError::Arithmetic)
            );
        }
        assert_eq!(
            WORD32.state_map(117_440_511),
            Ok(AllocationEnvelope {
                retained_bytes: 1_744_830_480,
                old_growth_bytes: 872_415_248,
            })
        );
        assert_eq!(WORD32.state_map(0x0700_0000), Err(LayoutError::Arithmetic));
    }

    #[test]
    fn arc_slice_layout_pins_each_header_and_alignment_boundary() {
        for (layout, payload, expected) in [
            (WORD64, 0, 16),
            (WORD64, 1, 24),
            (WORD64, 8, 24),
            (WORD64, 9, 32),
            (WORD64, 16, 32),
            (WORD64, 17, 40),
            (WORD32, 0, 8),
            (WORD32, 1, 12),
            (WORD32, 4, 12),
            (WORD32, 5, 16),
            (WORD32, 9, 20),
        ] {
            assert_eq!(layout.arc_slice(payload), Ok(expected));
        }
        for layout in [WORD32, WORD64] {
            assert_eq!(layout.arc_slice(usize::MAX), Err(LayoutError::Arithmetic));
        }
        assert_eq!(WORD32.arc_slice(1 << 31_u32), Err(LayoutError::Arithmetic));
    }
}

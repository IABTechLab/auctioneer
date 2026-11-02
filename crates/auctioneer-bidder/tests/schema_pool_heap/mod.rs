//! Conditional unused std meta-pool requested heap, not program/cache/RSS/guest bounds.

use super::schema_heap_layout::{LayoutError, NativeField as Field, private_struct_slot_bound};
use regex_automata::meta::{Cache, Config};
use std::alloc::Layout;
use std::{
    cell::UnsafeCell,
    panic::{RefUnwindSafe, UnwindSafe},
    sync::{Mutex, atomic::AtomicUsize},
};

type CacheFactory = Box<dyn Fn() -> Cache + Send + Sync + UnwindSafe + RefUnwindSafe>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PoolHeapError {
    Shape,
    Arithmetic,
    Configuration,
}

impl From<LayoutError> for PoolHeapError {
    fn from(error: LayoutError) -> Self {
        match error {
            LayoutError::Shape => Self::Shape,
            LayoutError::Arithmetic => Self::Arithmetic,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PoolLayout {
    fields: [Field; 4],
    shard: Field,
    closure: Field,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UnusedPoolHeap {
    pub boxed_pool_bytes: usize,
    pub shard_stride: usize,
    pub shards: usize,
    pub shard_bytes: usize,
    pub closure_bytes: usize,
    pub requested_bytes: usize,
}

impl PoolLayout {
    fn native() -> Self {
        Self {
            fields: [
                Field::of::<CacheFactory>(),
                // Pinned std Vec/RawVec headers do not depend on the sized
                // element's alignment; CacheLine itself is private.
                Field::of::<Vec<Mutex<Vec<Box<Cache>>>>>(),
                Field::of::<AtomicUsize>(),
                Field::of::<UnsafeCell<Option<Cache>>>(),
            ],
            shard: Field::of::<Mutex<Vec<Box<Cache>>>>(),
            // meta::Regex::{clone, builder}: one captured Arc<dyn Strategy>.
            // Its existing allocation is shared, not allocated by this closure.
            closure: Field::of::<[usize; 2]>(),
        }
    }

    fn unused(self, capacity: usize) -> Result<UnusedPoolHeap, PoolHeapError> {
        for field in self.fields.into_iter().chain([self.shard, self.closure]) {
            field.validate()?;
        }
        let boxed_pool_bytes = private_struct_slot_bound(&self.fields)?.bytes;
        // util/pool.rs CacheLine is repr(C, align(64)), including natural align.
        let shard_alignment = self.shard.alignment.max(64);
        let shard_stride = round_up(self.shard.bytes, shard_alignment)?;
        let shards = capacity.max(1);
        // Rust 1.98.1 alloc::raw_vec::{with_capacity, try_allocate_in} requests
        // layout_array(capacity, element_layout) and stores exactly capacity.
        // This is not inferred from Vec's public "at least" capacity contract.
        let shard_bytes = shard_stride
            .checked_mul(shards)
            .ok_or(PoolHeapError::Arithmetic)?;
        check_request(shard_bytes, shard_alignment)?;
        let requested_bytes = boxed_pool_bytes
            .checked_add(shard_bytes)
            .and_then(|bytes| bytes.checked_add(self.closure.bytes))
            .ok_or(PoolHeapError::Arithmetic)?;
        // with_capacity leaves owner None and inner Vecs empty, and never calls
        // create: no backing caches or inner Vec buffers exist under this premise.
        Ok(UnusedPoolHeap {
            boxed_pool_bytes,
            shard_stride,
            shards,
            shard_bytes,
            closure_bytes: self.closure.bytes,
            requested_bytes,
        })
    }
}

impl UnusedPoolHeap {
    pub(crate) fn native(config: &Config) -> Result<Self, PoolHeapError> {
        if config.get_pool_capacity() != 1 {
            return Err(PoolHeapError::Configuration);
        }
        PoolLayout::native().unused(config.get_pool_capacity())
    }

    pub(crate) fn population(self, helpers: usize, hostname: bool) -> Result<usize, PoolHeapError> {
        helpers
            .checked_add(usize::from(hostname))
            .and_then(|owners| owners.checked_mul(self.requested_bytes))
            .ok_or(PoolHeapError::Arithmetic)
    }
}

fn round_up(bytes: usize, alignment: usize) -> Result<usize, PoolHeapError> {
    if !alignment.is_power_of_two() {
        return Err(PoolHeapError::Shape);
    }
    bytes
        .checked_next_multiple_of(alignment)
        .ok_or(PoolHeapError::Arithmetic)
}

fn check_request(bytes: usize, alignment: usize) -> Result<(), PoolHeapError> {
    Layout::from_size_align(bytes, alignment)
        .map(|_layout| ())
        .map_err(|_error| PoolHeapError::Arithmetic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex_automata::meta::Regex;

    const SYNTHETIC: PoolLayout = PoolLayout {
        fields: [
            Field {
                bytes: 16,
                alignment: 8,
            },
            Field {
                bytes: 24,
                alignment: 8,
            },
            Field {
                bytes: 8,
                alignment: 8,
            },
            Field {
                bytes: 40,
                alignment: 8,
            },
        ],
        shard: Field {
            bytes: 40,
            alignment: 8,
        },
        closure: Field {
            bytes: 16,
            alignment: 8,
        },
    };

    #[test]
    fn literal_components_and_shard_counts_prevent_omitted_owners() {
        for (capacity, shards, shard_bytes, requested_bytes) in [
            (0, 1, 64, 168),
            (1, 1, 64, 168),
            (2, 2, 128, 232),
            (3, 3, 192, 296),
            (17, 17, 1088, 1192),
        ] {
            assert_eq!(
                SYNTHETIC.unused(capacity),
                Ok(UnusedPoolHeap {
                    boxed_pool_bytes: 88,
                    shard_stride: 64,
                    shards,
                    shard_bytes,
                    closure_bytes: 16,
                    requested_bytes
                })
            );
        }
        for field in 0..4 {
            let mut changed = SYNTHETIC;
            changed.fields[field].bytes += 8;
            let bound = changed.unused(1).unwrap();
            assert_eq!(bound.boxed_pool_bytes, 96);
            assert_eq!(bound.requested_bytes, 176);
        }
        let mut changed = SYNTHETIC;
        changed.shard.bytes = 72;
        assert_eq!(changed.unused(1).unwrap().shard_stride, 128);
        assert_eq!(changed.unused(1).unwrap().requested_bytes, 232);
        changed = SYNTHETIC;
        changed.closure.bytes = 24;
        assert_eq!(changed.unused(1).unwrap().requested_bytes, 176);
    }

    #[test]
    fn padding_covers_every_field_order_without_a_private_rust_mirror() {
        let layout = PoolLayout {
            fields: [
                Field {
                    bytes: 3,
                    alignment: 1,
                },
                Field {
                    bytes: 8,
                    alignment: 8,
                },
                Field {
                    bytes: 2,
                    alignment: 2,
                },
                Field {
                    bytes: 12,
                    alignment: 4,
                },
            ],
            shard: Field {
                bytes: 65,
                alignment: 1,
            },
            closure: Field {
                bytes: 5,
                alignment: 1,
            },
        };
        let bound = layout.unused(1).unwrap();
        assert_eq!(bound.boxed_pool_bytes, 40);
        assert_eq!(bound.shard_stride, 128);
        assert_eq!(bound.requested_bytes, 173);
        let mut orders = 0_usize;
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    for d in 0..4 {
                        let mut indices = [a, b, c, d];
                        indices.sort_unstable();
                        if indices != [0, 1, 2, 3] {
                            continue;
                        }
                        let mut offset = 0_usize;
                        for index in [a, b, c, d] {
                            let field = layout.fields[index];
                            offset = offset
                                .next_multiple_of(field.alignment)
                                .checked_add(field.bytes)
                                .unwrap();
                        }
                        assert!(offset.next_multiple_of(8) <= bound.boxed_pool_bytes);
                        orders += 1;
                    }
                }
            }
        }
        assert_eq!(orders, 24);
        let mut over_aligned = SYNTHETIC;
        over_aligned.shard = Field {
            bytes: 128,
            alignment: 128,
        };
        assert_eq!(over_aligned.unused(2).unwrap().shard_bytes, 256);
    }

    #[test]
    fn every_zero_or_malformed_field_is_refused_even_with_zero_capacity() {
        for malformed in [
            Field {
                bytes: 0,
                alignment: 8,
            },
            Field {
                bytes: 8,
                alignment: 0,
            },
            Field {
                bytes: 8,
                alignment: 3,
            },
            Field {
                bytes: 9,
                alignment: 8,
            },
        ] {
            for index in 0..6 {
                let mut layout = SYNTHETIC;
                match index {
                    0..=3 => layout.fields[index] = malformed,
                    4 => layout.shard = malformed,
                    _ => layout.closure = malformed,
                }
                assert_eq!(layout.unused(0), Err(PoolHeapError::Shape));
            }
        }
        assert_eq!(round_up(1, 0), Err(PoolHeapError::Shape));
        assert_eq!(round_up(1, 3), Err(PoolHeapError::Shape));
        assert_eq!(round_up(0, 64), Ok(0));
        assert_eq!(round_up(64, 64), Ok(64));
        assert_eq!(round_up(65, 64), Ok(128));
    }

    #[test]
    fn checked_padding_sums_multiplication_and_native_request_limits_refuse_overflow() {
        assert_eq!(round_up(usize::MAX, 64), Err(PoolHeapError::Arithmetic));
        assert_eq!(SYNTHETIC.unused(usize::MAX), Err(PoolHeapError::Arithmetic));
        let maximum = usize::try_from(isize::MAX).unwrap();
        assert_eq!(
            SYNTHETIC.unused(maximum.checked_div(64).unwrap().checked_add(1).unwrap()),
            Err(PoolHeapError::Arithmetic)
        );
        let mut layout = SYNTHETIC;
        layout.fields[0] = Field {
            bytes: usize::MAX - 7,
            alignment: 8,
        };
        assert_eq!(layout.unused(1), Err(PoolHeapError::Arithmetic));
        layout.fields = [Field {
            bytes: 1_usize << (usize::BITS - 2),
            alignment: 8,
        }; 4];
        assert_eq!(layout.unused(1), Err(PoolHeapError::Arithmetic));
        layout.fields = [Field {
            bytes: 64,
            alignment: 8,
        }; 4];
        layout.shard = Field {
            bytes: maximum - 63,
            alignment: 64,
        };
        layout.closure = Field {
            bytes: maximum - 7,
            alignment: 8,
        };
        assert_eq!(layout.unused(1), Err(PoolHeapError::Arithmetic));
    }

    #[test]
    fn population_charges_one_fixed_owner_per_static_not_per_hostname_schema() {
        let heap = SYNTHETIC.unused(1).unwrap();
        for (helpers, hostname, bytes) in [
            (0, false, 0),
            (0, true, 168),
            (3, false, 504),
            (3, true, 672),
            (23, true, 4032),
        ] {
            assert_eq!(heap.population(helpers, hostname), Ok(bytes));
        }
        assert_eq!(
            heap.population(usize::MAX, true),
            Err(PoolHeapError::Arithmetic)
        );
        assert_eq!(
            heap.population(usize::MAX, false),
            Err(PoolHeapError::Arithmetic)
        );
    }

    #[test]
    fn native_fields_are_the_actual_compiled_public_types_including_full_owner_slot() {
        let layout = PoolLayout::native();
        assert_eq!(
            layout.fields,
            [
                Field::of::<CacheFactory>(),
                Field::of::<Vec<Mutex<Vec<Box<Cache>>>>>(),
                Field::of::<AtomicUsize>(),
                Field::of::<UnsafeCell<Option<Cache>>>()
            ]
        );
        assert_eq!(layout.shard, Field::of::<Mutex<Vec<Box<Cache>>>>());
        assert_eq!(layout.closure, Field::of::<[usize; 2]>());
        assert_eq!(size_of::<CacheFactory>(), size_of::<[usize; 2]>());
        assert_eq!(
            size_of::<UnsafeCell<Option<Cache>>>(),
            size_of::<Option<Cache>>()
        );
        assert!(layout.fields[3].bytes >= size_of::<Cache>());
        let config = Regex::config().pool_capacity(1);
        assert_eq!(UnusedPoolHeap::native(&config), layout.unused(1));
        for capacity in [0, 2, usize::MAX] {
            assert_eq!(
                UnusedPoolHeap::native(&Regex::config().pool_capacity(capacity)),
                Err(PoolHeapError::Configuration)
            );
        }
    }
}

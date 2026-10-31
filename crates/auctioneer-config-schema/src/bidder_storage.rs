//! Target-layout arithmetic for the generated metadata allocation census.
//!
//! These are requested heap and logical inline-storage counts, not a complete
//! resident bound. Allocator overhead, code, literal pooling/alignment and
//! validator/registry storage require their own qualified contributions.

use super::types::{
    BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration, MediaType, SyncType,
};
pub use super::types::{MetadataHeapShape, MetadataStorageDescriptor};
use std::mem::size_of;
use std::sync::OnceLock;

fn array_bytes<T>(count: u64) -> Option<u64> {
    count.checked_mul(u64::try_from(size_of::<T>()).ok()?)
}

impl MetadataStorageDescriptor {
    /// Requested allocation bytes for the lazy metadata constructor.
    #[must_use]
    pub fn requested_heap_bytes(&self) -> Option<u64> {
        self.heap
            .string_bytes
            .checked_add(array_bytes::<BidderInfo>(self.active_bidders)?)?
            .checked_add(array_bytes::<String>(self.heap.string_elements)?)?
            .checked_add(array_bytes::<MediaType>(self.heap.media_type_elements)?)?
            .checked_add(array_bytes::<SyncType>(self.heap.sync_type_elements)?)
    }

    /// Number of nonzero allocations requested by the constructor.
    #[must_use]
    pub fn heap_allocations(&self) -> Option<u64> {
        self.heap
            .nonempty_strings
            .checked_add(self.heap.nonempty_vectors)?
            .checked_add(u64::from(self.active_bidders != 0))
    }

    /// Logical target storage for the lazy cell and binding/declaration tables.
    /// Excludes string-literal payloads, linker layout and executable code.
    #[must_use]
    pub fn static_layout_bytes(&self) -> Option<u64> {
        array_bytes::<OnceLock<Vec<BidderInfo>>>(1)?
            .checked_add(array_bytes::<&[BidderImplementationBinding]>(1)?)?
            .checked_add(array_bytes::<&[BidderMetadataDeclaration]>(1)?)?
            .checked_add(array_bytes::<BidderImplementationBinding>(
                self.active_bindings,
            )?)?
            .checked_add(array_bytes::<BidderMetadataDeclaration>(self.declarations)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{MetadataHeapShape, MetadataStorageDescriptor};
    use crate::types::{
        BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration, MediaType, SyncType,
    };
    use std::mem::size_of;
    use std::sync::OnceLock;

    #[test]
    fn requested_heap_uses_independent_target_layout_and_payload_populations() {
        let descriptor = MetadataStorageDescriptor {
            active_bidders: 2,
            active_bindings: 3,
            declarations: 5,
            heap: MetadataHeapShape {
                string_bytes: 11,
                nonempty_strings: 13,
                string_elements: 17,
                media_type_elements: 19,
                sync_type_elements: 23,
                nonempty_vectors: 29,
            },
            source_sha256: [0; 32],
        };
        let expected = 11
            + 2 * size_of::<BidderInfo>()
            + 17 * size_of::<String>()
            + 19 * size_of::<MediaType>()
            + 23 * size_of::<SyncType>();
        assert_eq!(
            descriptor.requested_heap_bytes(),
            Some(u64::try_from(expected).unwrap())
        );
        assert_eq!(descriptor.heap_allocations(), Some(43));
        let expected_static = size_of::<OnceLock<Vec<BidderInfo>>>()
            + size_of::<&[BidderImplementationBinding]>()
            + size_of::<&[BidderMetadataDeclaration]>()
            + 3 * size_of::<BidderImplementationBinding>()
            + 5 * size_of::<BidderMetadataDeclaration>();
        assert_eq!(
            descriptor.static_layout_bytes(),
            Some(u64::try_from(expected_static).unwrap())
        );
    }

    #[test]
    fn empty_metadata_has_no_heap_and_retains_only_its_declared_static_layout() {
        let empty = MetadataStorageDescriptor::default();
        assert_eq!(empty.requested_heap_bytes(), Some(0));
        assert_eq!(empty.heap_allocations(), Some(0));
        let static_bytes = size_of::<OnceLock<Vec<BidderInfo>>>()
            + size_of::<&[BidderImplementationBinding]>()
            + size_of::<&[BidderMetadataDeclaration]>();
        assert_eq!(
            empty.static_layout_bytes(),
            Some(u64::try_from(static_bytes).unwrap())
        );
    }

    #[test]
    fn each_requested_storage_population_refuses_checked_overflow() {
        let base = MetadataStorageDescriptor::default();
        for descriptor in [
            MetadataStorageDescriptor {
                active_bidders: u64::MAX,
                ..base
            },
            MetadataStorageDescriptor {
                heap: MetadataHeapShape {
                    string_elements: u64::MAX,
                    ..base.heap
                },
                ..base
            },
            MetadataStorageDescriptor {
                heap: MetadataHeapShape {
                    string_bytes: 1,
                    media_type_elements: u64::MAX,
                    ..base.heap
                },
                ..base
            },
            MetadataStorageDescriptor {
                heap: MetadataHeapShape {
                    string_bytes: 1,
                    sync_type_elements: u64::MAX,
                    ..base.heap
                },
                ..base
            },
        ] {
            assert_eq!(descriptor.requested_heap_bytes(), None);
        }
        for descriptor in [
            MetadataStorageDescriptor {
                active_bindings: u64::MAX,
                ..base
            },
            MetadataStorageDescriptor {
                declarations: u64::MAX,
                ..base
            },
        ] {
            assert_eq!(descriptor.static_layout_bytes(), None);
        }
        for (active_bidders, nonempty_strings, nonempty_vectors) in
            [(1, u64::MAX, 0), (0, u64::MAX, 1)]
        {
            let descriptor = MetadataStorageDescriptor {
                active_bidders,
                heap: MetadataHeapShape {
                    nonempty_strings,
                    nonempty_vectors,
                    ..base.heap
                },
                ..base
            };
            assert_eq!(descriptor.heap_allocations(), None);
        }
    }
}

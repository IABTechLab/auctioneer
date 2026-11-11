//! Config-owned target layout and requested heap, not total physical residency.

#[cfg(test)]
mod tests {
    use auctioneer_config_data::bidder_storage::{MetadataHeapShape, MetadataStorageDescriptor};
    use auctioneer_config_data::types::{
        BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration, Capabilities,
        Compression, MediaType, OpenRtbCapabilities,
    };
    use auctioneer_config_data::usersync::{
        SkipWhen, SyncResponseFormat, SyncType, SyncerConfig, SyncerEndpoint,
    };
    use auctioneer_config_data::{
        bidder_implementation_bindings, bidder_metadata_declarations, bidder_metadata_storage,
        bidders,
    };
    use std::collections::BTreeMap;
    use std::env::consts::ARCH;
    use std::error::Error;
    use std::fs;
    use std::hint::black_box;
    use std::io::{Write as _, stdout};
    use std::mem::{size_of, size_of_val};
    use std::path::Path;
    use std::ptr;
    use std::sync::OnceLock;

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    struct Measurement {
        allocation_calls: usize,
        deallocation_calls: usize,
        reallocation_calls: usize,
        requested_bytes: usize,
        retained_bytes: usize,
        peak_bytes: usize,
        invalid: bool,
    }

    mod alloc_track {
        #![expect(
            unsafe_code,
            reason = "test-only GlobalAlloc instrumentation delegates every operation unchanged to System"
        )]

        use super::Measurement;
        use std::alloc::{GlobalAlloc, Layout, System};
        use std::cell::Cell;

        thread_local! {
            static ENABLED: Cell<bool> = const { Cell::new(false) };
            static COUNTERS: Cell<Measurement> = const { Cell::new(Measurement {
                allocation_calls: 0,
                deallocation_calls: 0,
                reallocation_calls: 0,
                requested_bytes: 0,
                retained_bytes: 0,
                peak_bytes: 0,
                invalid: false,
            }) };
        }

        // These synchronous intervals must not free allocations from outside them.
        pub(super) fn measure<T>(work: impl FnOnce() -> T) -> (T, Measurement) {
            assert!(!ENABLED.with(Cell::get), "measurement must not nest");
            COUNTERS.with(|cell| cell.set(Measurement::default()));
            ENABLED.with(|cell| cell.set(true));
            let value = work();
            ENABLED.with(|cell| cell.set(false));
            let measurement = COUNTERS.with(Cell::get);
            assert!(
                !measurement.invalid,
                "counter overflow or out-of-scope deallocation: {measurement:?}"
            );
            (value, measurement)
        }

        fn add(total: &mut usize, value: usize, invalid: &mut bool) {
            if let Some(sum) = total.checked_add(value) {
                *total = sum;
            } else {
                *invalid = true;
            }
        }

        impl Measurement {
            fn allocate(&mut self, bytes: usize) {
                add(&mut self.allocation_calls, 1, &mut self.invalid);
                add(&mut self.requested_bytes, bytes, &mut self.invalid);
                add(&mut self.retained_bytes, bytes, &mut self.invalid);
                self.peak_bytes = self.peak_bytes.max(self.retained_bytes);
            }

            fn deallocate(&mut self, bytes: usize) {
                add(&mut self.deallocation_calls, 1, &mut self.invalid);
                if let Some(remaining) = self.retained_bytes.checked_sub(bytes) {
                    self.retained_bytes = remaining;
                } else {
                    self.invalid = true;
                }
            }
        }

        fn record(update: impl FnOnce(&mut Measurement)) {
            if ENABLED.try_with(Cell::get).unwrap_or(false) {
                let _recorded = COUNTERS.try_with(|cell| {
                    let mut measurement = cell.get();
                    update(&mut measurement);
                    cell.set(measurement);
                });
            }
        }

        pub(super) struct CountingAllocator;

        // SAFETY: System receives the original pointers and layouts; counters are thread-local.
        unsafe impl GlobalAlloc for CountingAllocator {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                // SAFETY: the caller supplies a valid allocation layout.
                let pointer = unsafe { System.alloc(layout) };
                if !pointer.is_null() {
                    record(|measurement| measurement.allocate(layout.size()));
                }
                pointer
            }

            unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
                // SAFETY: the caller supplies a valid allocation layout.
                let pointer = unsafe { System.alloc_zeroed(layout) };
                if !pointer.is_null() {
                    record(|measurement| measurement.allocate(layout.size()));
                }
                pointer
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                record(|measurement| measurement.deallocate(layout.size()));
                // SAFETY: the caller supplies a live System allocation and its layout.
                unsafe { System.dealloc(ptr, layout) }
            }

            unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
                // SAFETY: the caller supplies a live allocation, its layout and a valid new size.
                let resized = unsafe { System.realloc(ptr, layout, new_size) };
                if !resized.is_null() {
                    record(|measurement| {
                        add(
                            &mut measurement.reallocation_calls,
                            1,
                            &mut measurement.invalid,
                        );
                        // Conservatively count both requests at the resize boundary.
                        measurement.allocate(new_size);
                        measurement.deallocate(layout.size());
                    });
                }
                resized
            }
        }
    }

    #[global_allocator]
    static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

    fn count(value: usize) -> u64 {
        u64::try_from(value).expect("test population fits u64")
    }

    fn add(total: &mut u64, value: u64) {
        *total = total.checked_add(value).expect("test census fits u64");
    }

    fn assert_allocator_control() {
        let (retained, measurement) = alloc_track::measure(|| {
            let transient = black_box(Box::new(black_box([0_u8; 64])));
            let retained = black_box(Box::new(black_box([0_u8; 32])));
            drop(black_box(transient));
            retained
        });
        assert_eq!(measurement.allocation_calls, 2);
        assert_eq!(measurement.deallocation_calls, 1);
        assert_eq!(measurement.reallocation_calls, 0);
        assert_eq!(measurement.requested_bytes, 96);
        assert_eq!(measurement.retained_bytes, 32);
        assert_eq!(measurement.peak_bytes, 96);
        drop(black_box(retained));

        let (resized, growth) = alloc_track::measure(|| {
            let mut buffer = Vec::with_capacity(8);
            buffer.extend_from_slice(&[9_u8; 8]);
            black_box(&mut buffer).reserve_exact(8);
            black_box(buffer)
        });
        assert_eq!(resized.capacity(), 16);
        assert_eq!(growth.allocation_calls, 2);
        assert_eq!(growth.deallocation_calls, 1);
        assert_eq!(growth.reallocation_calls, 1);
        assert_eq!(growth.requested_bytes, 24);
        assert_eq!(growth.retained_bytes, 16);
        assert_eq!(growth.peak_bytes, 24);
        drop(black_box(resized));
    }

    #[derive(Debug, Default)]
    struct HeapWalk {
        shape: MetadataHeapShape,
        requested_bytes: u64,
        allocations: u64,
    }

    impl HeapWalk {
        fn string(&mut self, value: &String) {
            assert_eq!(
                value.capacity(),
                value.len(),
                "literal string has no reserve"
            );
            add(&mut self.shape.string_bytes, count(value.capacity()));
            add(&mut self.requested_bytes, count(value.capacity()));
            if value.capacity() != 0 {
                add(&mut self.shape.nonempty_strings, 1);
                add(&mut self.allocations, 1);
            }
        }

        fn optional_string(&mut self, value: Option<&String>) {
            if let Some(string) = value {
                self.string(string);
            }
        }

        fn vector<T>(&mut self, values: &Vec<T>) -> u64 {
            assert_ne!(size_of::<T>(), 0, "censused elements are not zero-sized");
            assert_eq!(
                values.capacity(),
                values.len(),
                "literal vector has no reserve"
            );
            add(
                &mut self.requested_bytes,
                count(size_of_val(values.as_slice())),
            );
            if values.capacity() != 0 {
                add(&mut self.shape.nonempty_vectors, 1);
                add(&mut self.allocations, 1);
            }
            count(values.capacity())
        }

        fn strings(&mut self, values: &Vec<String>) {
            let elements = self.vector(values);
            add(&mut self.shape.string_elements, elements);
            for value in values {
                self.string(value);
            }
        }

        fn media_types(&mut self, values: &Vec<MediaType>) {
            let elements = self.vector(values);
            add(&mut self.shape.media_type_elements, elements);
        }

        fn capabilities(&mut self, value: &Capabilities) {
            let Capabilities { site, app, dooh } = value;
            self.media_types(site);
            self.media_types(app);
            self.media_types(dooh);
        }

        fn syncer_endpoint(&mut self, value: &SyncerEndpoint) {
            let SyncerEndpoint { url, user_macro } = value;
            self.string(url);
            self.optional_string(user_macro.as_ref());
        }

        fn syncer(&mut self, value: &SyncerConfig) {
            let SyncerConfig {
                key,
                iframe,
                redirect,
                supports,
                enabled,
                skip_when,
                default_response,
            } = value;
            let _: &bool = enabled;
            let _: &Option<SyncResponseFormat> = default_response;
            let _: &Vec<SyncType> = supports;
            self.optional_string(key.as_ref());
            for endpoint in [iframe, redirect].into_iter().flatten() {
                self.syncer_endpoint(endpoint);
            }
            let elements = self.vector(supports);
            add(&mut self.shape.sync_type_elements, elements);
            let SkipWhen { gdpr, gpp_sid } = skip_when;
            let _: &bool = gdpr;
            self.strings(gpp_sid);
        }

        fn openrtb(&mut self, value: &OpenRtbCapabilities) {
            let OpenRtbCapabilities {
                version,
                gpp_supported,
            } = value;
            let _: &bool = gpp_supported;
            self.optional_string(version.as_ref());
        }

        fn bidder(&mut self, value: &BidderInfo) {
            // No rest patterns: adding any field requires an explicit census decision.
            let BidderInfo {
                name,
                alias_of,
                default_enabled,
                endpoint,
                uses_https,
                disabled,
                debug_allow,
                endpoint_compression,
                gvl_vendor_id,
                host_allowlist,
                bidder_timeout_ms,
                endpoint_macros,
                maintainer_email,
                capabilities,
                user_sync,
                openrtb,
            } = value;
            let _: &bool = uses_https;
            let _: &Option<bool> = default_enabled;
            let _: &bool = disabled;
            let _: &bool = debug_allow;
            let _: &Option<Compression> = endpoint_compression;
            let _: &Option<u32> = gvl_vendor_id;
            let _: &Option<u32> = bidder_timeout_ms;
            self.string(name);
            self.optional_string(alias_of.as_ref());
            self.optional_string(endpoint.as_ref());
            if let Some(hosts) = host_allowlist {
                self.strings(hosts);
            }
            self.strings(endpoint_macros);
            self.optional_string(maintainer_email.as_ref());
            self.capabilities(capabilities);
            self.syncer(user_sync);
            self.openrtb(openrtb);
        }
    }

    fn walk_metadata(metadata: &[BidderInfo]) -> HeapWalk {
        // The outer bidder buffer is counted separately from nested vector shapes.
        let mut walked = HeapWalk {
            requested_bytes: count(size_of_val(metadata)),
            allocations: u64::from(!metadata.is_empty()),
            ..HeapWalk::default()
        };
        for bidder in metadata {
            walked.bidder(bidder);
        }
        walked
    }

    type SourceDeclarations = BTreeMap<String, (String, Option<String>, bool)>;

    fn source_declarations() -> Result<SourceDeclarations, Box<dyn Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .with_file_name("auctioneer-bidder")
            .join("src/bidders");
        let mut expected = BTreeMap::new();
        for directory in fs::read_dir(root)? {
            let path = directory?.path().join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let manifest: toml::Table = toml::from_str(&fs::read_to_string(path)?)?;
            let implementation = manifest
                .get("key")
                .and_then(toml::Value::as_str)
                .ok_or("missing source key")?;
            let feature = manifest
                .get("required_feature")
                .map(|value| value.as_str().ok_or("invalid source feature"))
                .transpose()?;
            let native_only = manifest
                .get("native_only")
                .map(|value| value.as_bool().ok_or("invalid native-only declaration"))
                .transpose()?
                .unwrap_or(false);
            if let Some(definitions) = manifest.get("definitions").and_then(toml::Value::as_table) {
                for name in definitions.keys() {
                    if expected
                        .insert(
                            name.clone(),
                            (
                                implementation.to_owned(),
                                feature.map(str::to_owned),
                                native_only,
                            ),
                        )
                        .is_some()
                    {
                        return Err("duplicate source bidder declaration".into());
                    }
                }
            }
        }
        Ok(expected)
    }

    fn assert_roster(
        descriptor: &MetadataStorageDescriptor,
        metadata: &[BidderInfo],
        bindings: &[BidderImplementationBinding],
        declarations: &[BidderMetadataDeclaration],
    ) {
        // Source reads occur only after the cold and warm allocation intervals.
        let expected = source_declarations().unwrap();
        assert!(!expected.is_empty());
        let mut active = Vec::new();
        for (name, (implementation, feature, native_only)) in &expected {
            let enabled = match feature.as_deref() {
                None => true,
                Some("insecure-bidders") => cfg!(feature = "insecure-bidders"),
                Some(unknown) => panic!("add explicit test feature selection for {unknown}"),
            };
            if enabled && !matches!((*native_only, ARCH), (true, "wasm32")) {
                active.push((name.as_str(), implementation.as_str()));
            }
        }
        assert_eq!(descriptor.active_bidders, count(active.len()));
        assert_eq!(descriptor.active_bindings, count(active.len()));
        assert_eq!(descriptor.declarations, count(expected.len()));
        assert_eq!(metadata.len(), active.len());
        assert_eq!(bindings.len(), active.len());
        assert_eq!(declarations.len(), expected.len());
        assert!(
            metadata
                .iter()
                .map(|row| row.name.as_str())
                .eq(active.iter().map(|(name, _)| *name))
        );
        assert!(
            bindings
                .iter()
                .map(|row| (row.bidder, row.implementation))
                .eq(active)
        );
        assert!(
            declarations
                .iter()
                .map(|row| (row.bidder, row.implementation, row.required_feature))
                .eq(expected
                    .iter()
                    .map(|(name, (implementation, feature, _native_only))| (
                        name.as_str(),
                        implementation.as_str(),
                        feature.as_deref()
                    )))
        );
    }

    #[test]
    fn generated_metadata_storage_matches_cold_and_warm_materialization() {
        // This binary has one test, so no earlier test can warm the lazy metadata.
        assert_allocator_control();
        let ((descriptor, bindings, declarations), static_access) = alloc_track::measure(|| {
            (
                black_box(bidder_metadata_storage()),
                black_box(bidder_implementation_bindings()),
                black_box(bidder_metadata_declarations()),
            )
        });
        assert_eq!(static_access, Measurement::default());

        let (metadata, cold) = alloc_track::measure(|| black_box(bidders()));
        let (warm_metadata, warm) = alloc_track::measure(|| black_box(bidders()));
        assert_eq!(warm, Measurement::default());
        assert!(ptr::eq(metadata, warm_metadata));
        assert_roster(&descriptor, metadata, bindings, declarations);
        assert_ne!(descriptor.source_sha256, [0; 32]);

        let requested_bytes = descriptor.requested_heap_bytes().expect("heap census fits");
        let allocations = descriptor
            .heap_allocations()
            .expect("allocation census fits");
        assert!(requested_bytes > 0);
        assert!(allocations > 0);
        assert_eq!(count(cold.requested_bytes), requested_bytes);
        assert_eq!(count(cold.allocation_calls), allocations);
        assert_eq!(
            cold.deallocation_calls, 0,
            "no transient constructor buffers"
        );
        assert_eq!(cold.reallocation_calls, 0, "no constructor buffer growth");
        assert_eq!(count(cold.retained_bytes), requested_bytes);
        assert_eq!(count(cold.peak_bytes), requested_bytes);

        // Walk only after measurement; no generator or serialized fixture supplies expectations.
        let walked = walk_metadata(metadata);
        assert_eq!(walked.shape, descriptor.heap);
        assert_eq!(walked.requested_bytes, requested_bytes);
        assert_eq!(walked.allocations, allocations);
        let static_bytes = [
            size_of::<OnceLock<Vec<BidderInfo>>>(),
            size_of::<&[BidderImplementationBinding]>(),
            size_of::<&[BidderMetadataDeclaration]>(),
            size_of_val(bindings),
            size_of_val(declarations),
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .expect("static layout fits");
        assert_eq!(descriptor.static_layout_bytes(), Some(count(static_bytes)));

        let ((again, warm_bindings, warm_declarations), warm_static) = alloc_track::measure(|| {
            (
                black_box(bidder_metadata_storage()),
                black_box(bidder_implementation_bindings()),
                black_box(bidder_metadata_declarations()),
            )
        });
        assert_eq!(warm_static, Measurement::default());
        assert_eq!(again, descriptor);
        assert!(ptr::eq(bindings, warm_bindings));
        assert!(ptr::eq(declarations, warm_declarations));

        let digest = hex::encode(descriptor.source_sha256);
        writeln!(
            stdout().lock(),
            "bidder metadata storage: active={} heap_bytes={} allocations={} static_layout_bytes={} source_sha256={}",
            descriptor.active_bidders,
            requested_bytes,
            allocations,
            static_bytes,
            digest,
        )
        .expect("write metadata storage measurement evidence");
    }
}

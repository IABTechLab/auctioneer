//! Catalog framework allocation controls, not engine qualification or live C.

#[cfg(test)]
mod tests {
    use auctioneer_bidder::catalog::{
        CatalogBuildIdentity, CatalogCostError, CatalogCostField, CatalogOwner, CatalogRequirement,
        CatalogRequirements, CatalogStorage, QualifiedCatalogContribution, aggregate_catalog_costs,
    };
    use auctioneer_bidder::schema::{
        SchemaInputError, SchemaInputLimits, SchemaInputStats, measure_schema_input,
    };
    #[cfg(not(target_arch = "wasm32"))]
    use auctioneer_config_data::types::{
        BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
    };
    use auctioneer_runtime::CatalogResourceCosts;
    use serde_json::{Value, json};
    use std::array;
    #[cfg(not(target_arch = "wasm32"))]
    use std::cell::Cell;
    use std::hint::black_box;
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64"))]
    use std::panic::catch_unwind;

    #[cfg(not(target_arch = "wasm32"))]
    struct RegistryConstructionFixture {
        infos: &'static [BidderInfo],
        bindings: Vec<BidderImplementationBinding>,
        metadata: Vec<BidderMetadataDeclaration>,
        declarations: Vec<auctioneer_bidder::BidderDeclaration>,
        factories: Vec<auctioneer_bidder::BidderRegistration>,
        aliases: Vec<auctioneer_bidder::RegistryAlias>,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl RegistryConstructionFixture {
        fn new(count: usize) -> Self {
            Self::with_settings(count, false)
        }

        fn with_settings(count: usize, has_settings: bool) -> Self {
            use auctioneer_bidder::{BidderDeclaration, BidderRegistration, RegistryAlias};
            use auctioneer_config_data::types::{
                BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
            };

            fn forbidden_factory() -> Box<dyn auctioneer_bidder::Bidder> {
                panic!("registry validation must not invoke a bidder factory")
            }

            fn identity(kind: &str, index: usize) -> &'static str {
                String::leak(format!("fake_layout_{kind}_{index:04}"))
            }

            let mut infos = Vec::new();
            let mut bindings = Vec::new();
            let mut metadata = Vec::new();
            let mut declarations = Vec::new();
            let mut factories = Vec::new();
            let mut aliases = Vec::new();
            for index in 0..count {
                let implementation = identity("owner", index);
                let base = identity("base", index);
                let child = identity("child", index);
                for name in [base, child] {
                    infos.push(BidderInfo {
                        name: name.into(),
                        endpoint: "https://fake-layout.example.test/bid".into(),
                        ..BidderInfo::default()
                    });
                    bindings.push(BidderImplementationBinding {
                        bidder: name,
                        implementation,
                    });
                    metadata.push(BidderMetadataDeclaration {
                        bidder: name,
                        implementation,
                        required_feature: Some("fake_layout_feature"),
                    });
                }
                declarations.push(BidderDeclaration {
                    key: implementation,
                    required_feature: Some("fake_layout_feature"),
                    has_settings,
                });
                factories.push(BidderRegistration {
                    key: implementation,
                    factory: forbidden_factory,
                    config_validator: has_settings.then_some(observe_config_validation),
                });
                aliases.push(RegistryAlias {
                    name: child,
                    source_parent: base,
                    canonical_base: base,
                });
            }
            Self {
                infos: Box::leak(infos.into_boxed_slice()),
                bindings,
                metadata,
                declarations,
                factories,
                aliases,
            }
        }

        fn inputs(&self) -> auctioneer_bidder::RegistryInputs<'_> {
            auctioneer_bidder::RegistryInputs {
                infos: self.infos,
                bindings: &self.bindings,
                metadata_declarations: &self.metadata,
                factory_declarations: &self.declarations,
                factories: &self.factories,
                aliases: &self.aliases,
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    thread_local! {
        static CONFIG_VALIDATIONS: Cell<usize> = const { Cell::new(0) };
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn observe_config_validation(
        _input: auctioneer_bidder::BidderConfigInput<'_>,
    ) -> Result<(), auctioneer_bidder::BidderConfigError> {
        CONFIG_VALIDATIONS.set(CONFIG_VALIDATIONS.get().saturating_add(1));
        Err(auctioneer_bidder::BidderConfigError {
            field: "fake_field",
            reason: "unexpected_validation_during_construction",
        })
    }

    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64"))]
    fn require_construction_regression_baseline(allocation: Measurement) {
        // Frozen 512-identity requested-heap observations, not an exhaustive
        // bound or a replacement for production resource qualification.
        assert!(
            allocation.requested_bytes <= 565_136,
            "registry construction requested-byte regression"
        );
        assert!(
            allocation.retained_bytes <= 130_368,
            "registry construction retained-byte regression"
        );
        assert!(
            allocation.peak_bytes <= 270_352,
            "registry construction peak-byte regression"
        );
    }

    #[test]
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64"))]
    fn construction_regression_guard_rejects_each_independent_overrun() {
        for (requested, retained, peak) in [
            (565_137_usize, 130_368_usize, 270_352_usize),
            (565_136, 130_369, 270_352),
            (565_136, 130_368, 270_353),
        ] {
            let allocation = Measurement {
                requested_bytes: requested,
                retained_bytes: retained,
                peak_bytes: peak,
                ..Measurement::default()
            };
            assert!(catch_unwind(|| require_construction_regression_baseline(allocation)).is_err());
        }
    }

    #[test]
    #[cfg(all(not(target_arch = "wasm32"), target_pointer_width = "64"))]
    fn populated_configuration_callbacks_preserve_the_construction_baseline() {
        use auctioneer_bidder::{BidderConfigInput, ValidatedRegistryPlan};

        assert_allocator_control();
        CONFIG_VALIDATIONS.set(0);
        let control_result = observe_config_validation(BidderConfigInput {
            requested: None,
            base: None,
            definition: None,
            enabled: false,
        });
        assert!(control_result.is_err());
        assert_eq!(
            CONFIG_VALIDATIONS.get(),
            1,
            "positive control for callback observation"
        );
        CONFIG_VALIDATIONS.set(0);

        let empty_fixture = RegistryConstructionFixture::new(256);
        let (empty_result, empty_allocation) = alloc_track::measure(|| {
            ValidatedRegistryPlan::validate(black_box(empty_fixture.inputs()))
        });
        let empty_plan = empty_result.expect("empty callback baseline must validate");
        assert!(empty_allocation.retained_bytes > 0);
        require_construction_regression_baseline(empty_allocation);
        drop(black_box(empty_plan));

        let fixture = RegistryConstructionFixture::with_settings(256, true);
        let (result, allocation) =
            alloc_track::measure(|| ValidatedRegistryPlan::validate(black_box(fixture.inputs())));
        let plan = result.expect("populated callbacks must preserve registry validation");
        assert!(!allocation.invalid);
        assert_eq!(plan.definitions().len(), 512);
        assert_eq!(plan.factory_keys().len(), 256);
        assert_eq!(
            plan.definitions()
                .filter(|entry| entry.config_validator().is_some())
                .count(),
            512
        );
        assert_eq!(CONFIG_VALIDATIONS.get(), 0);
        assert_eq!(
            allocation, empty_allocation,
            "filling callback slots must not add hidden index allocations"
        );
        require_construction_regression_baseline(allocation);
        drop(black_box(plan));
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn actual_registry_construction_peak_observation() {
        use auctioneer_bidder::ValidatedRegistryPlan;
        use std::io::{Write as _, stderr};

        assert_allocator_control();
        // Bounded observations across insertion populations, not a worst-case
        // bound or an assertion about every possible B-tree split regime.
        for count in [0_usize, 1, 11, 12, 13, 63, 64, 65, 127, 128, 129, 255, 256] {
            let fixture = RegistryConstructionFixture::new(count);
            let inputs = fixture.inputs();
            let (result, allocation) =
                alloc_track::measure(|| ValidatedRegistryPlan::validate(black_box(inputs)));
            let plan = result.expect("synthetic registry must validate");
            assert_eq!(plan.definitions().len(), 2 * count);
            assert_eq!(plan.base_names().len(), count);
            assert_eq!(plan.aliases().len(), count);
            assert_eq!(plan.factory_keys().len(), count);
            assert!(allocation.peak_bytes >= allocation.retained_bytes);
            if count == 0 {
                assert_eq!(allocation, Measurement::default());
            } else {
                assert!(allocation.allocation_calls > 0);
                assert!(allocation.retained_bytes > 0);
            }
            #[cfg(target_pointer_width = "64")]
            if count == 256 {
                require_construction_regression_baseline(allocation);
            }
            // Fixture setup, reporting and final destruction are outside the
            // interval; the returned plan stays alive through measurement end.
            writeln!(
                stderr(),
                "{}",
                serde_json::to_string(&json!({
                    "workload": "actual_registry_construction",
                    "pointer_bytes": size_of::<usize>(),
                    "implementations": count,
                    "definitions": fixture.infos.len(),
                    "aliases": fixture.aliases.len(),
                    "allocation_calls": allocation.allocation_calls,
                    "deallocation_calls": allocation.deallocation_calls,
                    "reallocation_calls": allocation.reallocation_calls,
                    "requested_bytes": allocation.requested_bytes,
                    "retained_bytes": allocation.retained_bytes,
                    "peak_bytes": allocation.peak_bytes,
                    "invalid": allocation.invalid,
                }))
                .unwrap()
            )
            .unwrap();
            drop(black_box(plan));
        }
    }

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

        // Initialize TLS before enabling tracking; never free an out-of-interval allocation.
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

    fn assert_allocator_control() {
        let (retained, box_allocation) = alloc_track::measure(|| {
            let transient = black_box(Box::new(black_box([7_u8; 64])));
            let retained = black_box(Box::new(black_box([9_u8; 32])));
            drop(black_box(transient));
            retained
        });
        assert_eq!(black_box(&retained).as_ref(), &[9; 32]);
        assert_eq!(
            box_allocation,
            Measurement {
                allocation_calls: 2,
                deallocation_calls: 1,
                requested_bytes: 96,
                retained_bytes: 32,
                peak_bytes: 96,
                ..Measurement::default()
            }
        );
        drop(black_box(retained));

        let (zeroed, zeroed_allocation) =
            alloc_track::measure(|| black_box(vec![0_u8; black_box(32)]));
        assert_eq!(black_box(&zeroed).as_slice(), &[0; 32]);
        assert_eq!(
            zeroed_allocation,
            Measurement {
                allocation_calls: 1,
                requested_bytes: 32,
                retained_bytes: 32,
                peak_bytes: 32,
                ..Measurement::default()
            }
        );
        drop(black_box(zeroed));

        let (resized, growth) = alloc_track::measure(|| {
            let mut buffer = Vec::with_capacity(8);
            buffer.extend_from_slice(&[9_u8; 8]);
            black_box(&mut buffer).reserve_exact(8);
            black_box(buffer)
        });
        assert_eq!(black_box(&resized).as_slice(), &[9; 8]);
        assert_eq!(resized.capacity(), 16);
        assert_eq!(
            growth,
            Measurement {
                allocation_calls: 2,
                deallocation_calls: 1,
                reallocation_calls: 1,
                requested_bytes: 24,
                retained_bytes: 16,
                peak_bytes: 24,
                invalid: false,
            }
        );
        drop(black_box(resized));
    }

    #[test]
    fn original_root_field_inspection_retains_no_tree_or_parser_scratch() {
        use auctioneer_runtime::bounded_json::{JsonFieldShape, count_json_nodes_with_field};

        assert_allocator_control();
        for width in [64_usize, 4096, 0x0001_0000] {
            let bytes = serde_json::to_vec(&json!({
                "ext": {"opaque": "s".repeat(width)},
                "ignored": [true, null, {"ext": null}],
            }))
            .unwrap();
            let (result, allocation) =
                alloc_track::measure(|| count_json_nodes_with_field(black_box(&bytes), 12, "ext"));
            assert_eq!(result.unwrap(), (12, JsonFieldShape::Object));
            // Serde's ignore_value uses a byte stack for nested containers;
            // this fixed-depth fixture needs its eight-byte minimum capacity.
            assert_eq!(allocation.retained_bytes, 0, "width={width}");
            assert_eq!(allocation.requested_bytes, 8, "width={width}");
            assert_eq!(allocation.peak_bytes, 8, "width={width}");
        }

        for width in [64_usize, 1024, 0x4000] {
            // Exact escaped wire spelling exercises Serde's reusable key scratch,
            // rather than pre-decoding the input key with json!.
            let mut bytes = b"{\"".to_vec();
            for _ in 0..width {
                bytes.extend_from_slice(b"\\u0065");
            }
            bytes.extend_from_slice(b"\":null,\"ext\":{}}");
            let (result, allocation) =
                alloc_track::measure(|| count_json_nodes_with_field(black_box(&bytes), 5, "ext"));
            assert_eq!(result.unwrap(), (5, JsonFieldShape::Object));
            assert!(
                allocation.requested_bytes > 0,
                "scratch control must allocate"
            );
            assert_eq!(allocation.retained_bytes, 0);
            assert!(allocation.requested_bytes < 4 * bytes.len());
            assert!(allocation.peak_bytes < 2 * bytes.len());
        }
    }

    const OWNERS: [CatalogOwner; 4] = [
        CatalogOwner::Metadata,
        CatalogOwner::Validators,
        CatalogOwner::Registry,
        CatalogOwner::Context,
    ];
    const OWNER_ORDERS: [[usize; 4]; 24] = [
        [0, 1, 2, 3],
        [0, 1, 3, 2],
        [0, 2, 1, 3],
        [0, 2, 3, 1],
        [0, 3, 1, 2],
        [0, 3, 2, 1],
        [1, 0, 2, 3],
        [1, 0, 3, 2],
        [1, 2, 0, 3],
        [1, 2, 3, 0],
        [1, 3, 0, 2],
        [1, 3, 2, 0],
        [2, 0, 1, 3],
        [2, 0, 3, 1],
        [2, 1, 0, 3],
        [2, 1, 3, 0],
        [2, 3, 0, 1],
        [2, 3, 1, 0],
        [3, 0, 1, 2],
        [3, 0, 2, 1],
        [3, 1, 0, 2],
        [3, 1, 2, 0],
        [3, 2, 0, 1],
        [3, 2, 1, 0],
    ];
    const REQUIREMENTS: CatalogRequirements<'static> = CatalogRequirements {
        build: CatalogBuildIdentity {
            target_abi: "catalog-alloc-test-abi",
            features_sha256: [1; 32],
            toolchain_sha256: [2; 32],
            lock_sha256: [3; 32],
        },
        metadata: CatalogRequirement {
            source_sha256: [11; 32],
            qualification_sha256: [12; 32],
        },
        validators: CatalogRequirement {
            source_sha256: [21; 32],
            qualification_sha256: [22; 32],
        },
        registry: CatalogRequirement {
            source_sha256: [31; 32],
            qualification_sha256: [32; 32],
        },
        context: CatalogRequirement {
            source_sha256: [41; 32],
            qualification_sha256: [42; 32],
        },
    };

    const fn costs(values: [u64; 4]) -> CatalogResourceCosts {
        let [
            resident_bytes,
            initialization_bytes,
            schema_validation_per_auction_bytes,
            context_per_auction_bytes,
        ] = values;
        CatalogResourceCosts {
            resident_bytes,
            initialization_bytes,
            schema_validation_per_auction_bytes,
            context_per_auction_bytes,
        }
    }

    fn contributions<'a>(
        required: &CatalogRequirements<'a>,
        storage: [&'a [CatalogStorage]; 4],
    ) -> [QualifiedCatalogContribution<'a>; 4] {
        let requirements = [
            required.metadata,
            required.validators,
            required.registry,
            required.context,
        ];
        array::from_fn(|index| QualifiedCatalogContribution {
            owner: OWNERS[index],
            build: required.build,
            source_sha256: requirements[index].source_sha256,
            qualification_sha256: requirements[index].qualification_sha256,
            storage: storage[index],
        })
    }

    fn assert_aggregate(
        required: &CatalogRequirements<'_>,
        rows: &[QualifiedCatalogContribution<'_>],
        expected: Result<CatalogResourceCosts, CatalogCostError>,
    ) {
        let (actual, allocation) = alloc_track::measure(|| {
            black_box(aggregate_catalog_costs(
                black_box(required),
                black_box(rows),
            ))
        });
        assert_eq!(
            allocation,
            Measurement::default(),
            "catalog outcome: {actual:?}"
        );
        assert_eq!(actual, expected);
    }

    fn exercise_empty_records() {
        let empty_rows = contributions(&REQUIREMENTS, [&[]; 4]);
        assert_aggregate(
            &REQUIREMENTS,
            &[],
            Err(CatalogCostError::MissingOwner(CatalogOwner::Metadata)),
        );
        for order in OWNER_ORDERS {
            let ordered = order.map(|index| empty_rows[index]);
            assert_aggregate(&REQUIREMENTS, &ordered, Ok(costs([0, 0, 0, 0])));
            assert_aggregate(
                &REQUIREMENTS,
                &ordered[..3],
                Err(CatalogCostError::MissingOwner(ordered[3].owner)),
            );
        }

        for build in [
            CatalogBuildIdentity {
                target_abi: "",
                ..REQUIREMENTS.build
            },
            CatalogBuildIdentity {
                features_sha256: [0; 32],
                ..REQUIREMENTS.build
            },
            CatalogBuildIdentity {
                toolchain_sha256: [0; 32],
                ..REQUIREMENTS.build
            },
            CatalogBuildIdentity {
                lock_sha256: [0; 32],
                ..REQUIREMENTS.build
            },
        ] {
            let required = CatalogRequirements {
                build,
                ..REQUIREMENTS
            };
            let invalid_build_rows = contributions(&required, [&[]; 4]);
            assert_aggregate(
                &required,
                &invalid_build_rows,
                Err(CatalogCostError::InvalidBuildIdentity),
            );
        }

        for owner in OWNERS {
            for requirement in [
                CatalogRequirement {
                    source_sha256: [0; 32],
                    qualification_sha256: [99; 32],
                },
                CatalogRequirement {
                    source_sha256: [99; 32],
                    qualification_sha256: [0; 32],
                },
            ] {
                let mut required = REQUIREMENTS;
                let field = match owner {
                    CatalogOwner::Metadata => &mut required.metadata,
                    CatalogOwner::Validators => &mut required.validators,
                    CatalogOwner::Registry => &mut required.registry,
                    CatalogOwner::Context => &mut required.context,
                };
                *field = requirement;
                let invalid_requirement_rows = contributions(&required, [&[]; 4]);
                assert_aggregate(
                    &required,
                    &invalid_requirement_rows,
                    Err(CatalogCostError::InvalidRequirement(owner)),
                );
            }
        }
    }

    fn storage<const N: usize>(tag: u8, values: [u64; 4]) -> [CatalogStorage; N] {
        array::from_fn(|index| {
            let mut identity = [tag; 32];
            identity[0] = u8::try_from(index).unwrap();
            CatalogStorage {
                identity,
                costs: costs(values),
            }
        })
    }

    fn exercise_owner_orders(
        rows: &[QualifiedCatalogContribution<'_>; 4],
        expected: CatalogResourceCosts,
    ) {
        for order in OWNER_ORDERS {
            let ordered = order.map(|index| rows[index]);
            assert_aggregate(&REQUIREMENTS, &ordered, Ok(expected));
            let owner = ordered[3].owner;
            assert_aggregate(
                &REQUIREMENTS,
                &ordered[..3],
                Err(CatalogCostError::MissingOwner(owner)),
            );

            let duplicate = [ordered[0], ordered[1], ordered[2], ordered[3], ordered[3]];
            assert_aggregate(
                &REQUIREMENTS,
                &duplicate,
                Err(CatalogCostError::DuplicateOwner(owner)),
            );

            let mut source_changed = ordered;
            source_changed[3].source_sha256 = [99; 32];
            assert_aggregate(
                &REQUIREMENTS,
                &source_changed,
                Err(CatalogCostError::SourceMismatch(owner)),
            );
            let mut qualification_changed = ordered;
            qualification_changed[3].qualification_sha256 = [99; 32];
            assert_aggregate(
                &REQUIREMENTS,
                &qualification_changed,
                Err(CatalogCostError::QualificationMismatch(owner)),
            );
            for build in [
                CatalogBuildIdentity {
                    target_abi: "another-test-abi",
                    ..REQUIREMENTS.build
                },
                CatalogBuildIdentity {
                    features_sha256: [99; 32],
                    ..REQUIREMENTS.build
                },
                CatalogBuildIdentity {
                    toolchain_sha256: [99; 32],
                    ..REQUIREMENTS.build
                },
                CatalogBuildIdentity {
                    lock_sha256: [99; 32],
                    ..REQUIREMENTS.build
                },
            ] {
                let mut build_changed = ordered;
                build_changed[3].build = build;
                assert_aggregate(
                    &REQUIREMENTS,
                    &build_changed,
                    Err(CatalogCostError::BuildMismatch(owner)),
                );
            }
        }
    }

    fn exercise_size<const N: usize>(expected: [CatalogResourceCosts; 3]) {
        let disjoint = [
            storage::<N>(1, [2, 3, 5, 7]),
            storage::<N>(2, [11, 13, 17, 19]),
            storage::<N>(3, [23, 29, 31, 37]),
            storage::<N>(4, [41, 43, 47, 53]),
        ];
        let pairwise = [disjoint[0], disjoint[0], disjoint[2], disjoint[2]];
        let shared = [disjoint[0]; 4];
        // Shared identities have distinct backing arrays, not merely aliased slices.
        for (storage, total) in [
            (&disjoint, expected[0]),
            (&pairwise, expected[1]),
            (&shared, expected[2]),
        ] {
            let rows = contributions(
                &REQUIREMENTS,
                storage.each_ref().map(<[CatalogStorage; N]>::as_slice),
            );
            exercise_owner_orders(&rows, total);
        }

        for owner in 0..4 {
            let mut invalid = disjoint;
            invalid[owner][0].identity = [0; 32];
            let invalid_rows = contributions(
                &REQUIREMENTS,
                invalid.each_ref().map(<[CatalogStorage; N]>::as_slice),
            );
            assert_aggregate(
                &REQUIREMENTS,
                &invalid_rows,
                Err(CatalogCostError::InvalidStorageIdentity(OWNERS[owner])),
            );
            if N > 1 {
                let mut unordered = disjoint;
                unordered[owner].reverse();
                let unordered_rows = contributions(
                    &REQUIREMENTS,
                    unordered.each_ref().map(<[CatalogStorage; N]>::as_slice),
                );
                assert_aggregate(
                    &REQUIREMENTS,
                    &unordered_rows,
                    Err(CatalogCostError::UnorderedStorage(OWNERS[owner])),
                );
            }
        }

        for (field, overflow, mismatch) in [
            (
                CatalogCostField::Resident,
                [u64::MAX, 0, 0, 0],
                [99, 3, 5, 7],
            ),
            (
                CatalogCostField::Initialization,
                [0, u64::MAX, 0, 0],
                [2, 99, 5, 7],
            ),
            (
                CatalogCostField::SchemaValidation,
                [0, 0, u64::MAX, 0],
                [2, 3, 99, 7],
            ),
            (
                CatalogCostField::Context,
                [0, 0, 0, u64::MAX],
                [2, 3, 5, 99],
            ),
        ] {
            let mut overflowing = disjoint;
            overflowing[3].last_mut().unwrap().costs = costs(overflow);
            let overflow_rows = contributions(
                &REQUIREMENTS,
                overflowing.each_ref().map(<[CatalogStorage; N]>::as_slice),
            );
            let mut conflicting = shared;
            conflicting[3].last_mut().unwrap().costs = costs(mismatch);
            let conflict_rows = contributions(
                &REQUIREMENTS,
                conflicting.each_ref().map(<[CatalogStorage; N]>::as_slice),
            );
            for order in OWNER_ORDERS {
                let overflow_order = order.map(|index| overflow_rows[index]);
                assert_aggregate(
                    &REQUIREMENTS,
                    &overflow_order,
                    Err(CatalogCostError::CostOverflow(field)),
                );
                let conflict_order = order.map(|index| conflict_rows[index]);
                assert_aggregate(
                    &REQUIREMENTS,
                    &conflict_order,
                    Err(CatalogCostError::SharedStorageMismatch),
                );
            }
        }
    }

    #[test]
    fn catalog_aggregation_success_and_refusals_allocate_nothing() {
        // One test owns this process's allocator checks; other threads are excluded by TLS.
        assert_allocator_control();
        exercise_empty_records();
        // Expectations are independent literals, never derived by production arithmetic.
        exercise_size::<1>([
            costs([77, 88, 100, 116]),
            costs([25, 32, 36, 44]),
            costs([2, 3, 5, 7]),
        ]);
        exercise_size::<8>([
            costs([616, 704, 800, 928]),
            costs([200, 256, 288, 352]),
            costs([16, 24, 40, 56]),
        ]);
        exercise_size::<128>([
            costs([9856, 11264, 12800, 14848]),
            costs([3200, 4096, 4608, 5632]),
            costs([256, 384, 640, 896]),
        ]);
    }

    fn assert_input_allocation(
        input: &Value,
        limits: SchemaInputLimits,
        expected: Result<SchemaInputStats, SchemaInputError>,
    ) {
        let (actual, allocation) =
            alloc_track::measure(|| measure_schema_input(black_box(input), black_box(limits)));
        assert_eq!(actual, expected);
        assert_eq!(allocation, Measurement::default());
    }

    #[test]
    fn borrowed_parameter_measurement_and_refusals_allocate_nothing() {
        assert_allocator_control();
        for (count, bytes, nodes) in [(1_usize, 24, 4), (8, 143, 11), (128, 2183, 131)] {
            let input = json!({"a":vec!["\"\\\n\0\u{e9}";count]});
            let original = input.clone();
            let limits = SchemaInputLimits {
                serialized_bytes: bytes,
                nodes,
                depth: 2,
            };
            assert_input_allocation(
                &input,
                limits,
                Ok(SchemaInputStats {
                    serialized_bytes: bytes,
                    nodes,
                    max_depth: 2,
                }),
            );
            for (refused, error) in [
                (
                    SchemaInputLimits {
                        serialized_bytes: bytes.checked_sub(1).unwrap(),
                        ..limits
                    },
                    SchemaInputError::Bytes,
                ),
                (
                    SchemaInputLimits {
                        nodes: nodes.checked_sub(1).unwrap(),
                        ..limits
                    },
                    SchemaInputError::Nodes,
                ),
                (
                    SchemaInputLimits { depth: 1, ..limits },
                    SchemaInputError::Depth,
                ),
            ] {
                assert_input_allocation(&input, refused, Err(error));
            }
            assert_eq!(input, original);
        }
    }

    #[test]
    fn scalar_parameter_measurement_and_refusals_allocate_nothing() {
        assert_allocator_control();
        for (input, bytes, depth) in [
            (json!(null), 4_u64, 0_u32),
            (json!(true), 4, 0),
            (json!(false), 5, 0),
            (json!(i64::MIN), 20, 0),
            (json!(u64::MAX), 20, 0),
            (json!(-0.0_f64), 4, 0),
            (json!(1.25_f64), 4, 0),
            (json!("\"\\\n\0\u{e9}"), 16, 0),
            (json!({}), 2, 1),
            (json!([]), 2, 1),
        ] {
            let limits = SchemaInputLimits {
                serialized_bytes: bytes,
                nodes: 1,
                depth,
            };
            assert_input_allocation(
                &input,
                limits,
                Ok(SchemaInputStats {
                    serialized_bytes: bytes,
                    nodes: 1,
                    max_depth: depth,
                }),
            );
            assert_input_allocation(
                &input,
                SchemaInputLimits {
                    serialized_bytes: bytes.checked_sub(1).unwrap(),
                    ..limits
                },
                Err(SchemaInputError::Bytes),
            );
            assert_input_allocation(
                &input,
                SchemaInputLimits { nodes: 0, ..limits },
                Err(SchemaInputError::Nodes),
            );
        }
    }

    #[test]
    fn oversized_parameter_strings_and_keys_refuse_without_allocating() {
        assert_allocator_control();
        let limits = SchemaInputLimits {
            serialized_bytes: 32,
            nodes: 7,
            depth: 2,
        };
        for size in [4096_usize, 0x0001_0000, 0x0010_0000] {
            let huge = "x".repeat(size);
            let key = huge.clone();
            let inputs = [json!({"a":huge}), json!({key:null})];
            for input in inputs {
                assert_input_allocation(&input, limits, Err(SchemaInputError::Bytes));
            }
        }
    }
}

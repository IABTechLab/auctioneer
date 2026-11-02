//! Checked catalog-cost aggregation before materialization.
//!
//! This module checks independently supplied qualification identities; it does
//! not qualify a bound or infer one from a source size or an allocation sample.
//! Owners must bind their source digest to all relevant schemas, generator and
//! helper inputs, and supply reviewed records for the exact build identity.
//! No production records are installed by this API.

use auctioneer_runtime::CatalogResourceCosts;

/// Common environment of the independently qualified component records.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogBuildIdentity<'a> {
    pub target_abi: &'a str,
    pub features_sha256: [u8; 32],
    pub toolchain_sha256: [u8; 32],
    pub lock_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogOwner {
    Metadata,
    Validators,
    Registry,
    Context,
}

/// Expected source and reviewed record, supplied independently of the record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogRequirement {
    pub source_sha256: [u8; 32],
    pub qualification_sha256: [u8; 32],
}

/// All owners are mandatory, including an explicitly unchanged/empty owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogRequirements<'a> {
    pub build: CatalogBuildIdentity<'a>,
    pub metadata: CatalogRequirement,
    pub validators: CatalogRequirement,
    pub registry: CatalogRequirement,
    pub context: CatalogRequirement,
}

/// Disjoint lifetime charges for one shared storage identity.
/// Equal byte counts do not imply sharing. Equal identities must have equal
/// charges, even when two owners refer to that storage independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogStorage {
    pub identity: [u8; 32],
    pub costs: CatalogResourceCosts,
}

/// A qualification record's scalar projection, not evidence authentication.
/// Storage rows must be strictly ordered by identity within each owner.
/// Artifact/allocator qualification and record-digest verification belong to
/// the build/evidence owners, before publishing this projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualifiedCatalogContribution<'a> {
    pub owner: CatalogOwner,
    pub build: CatalogBuildIdentity<'a>,
    pub source_sha256: [u8; 32],
    pub qualification_sha256: [u8; 32],
    pub storage: &'a [CatalogStorage],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogCostField {
    Resident,
    Initialization,
    SchemaValidation,
    Context,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogCostError {
    InvalidBuildIdentity,
    InvalidRequirement(CatalogOwner),
    MissingOwner(CatalogOwner),
    DuplicateOwner(CatalogOwner),
    BuildMismatch(CatalogOwner),
    SourceMismatch(CatalogOwner),
    QualificationMismatch(CatalogOwner),
    InvalidStorageIdentity(CatalogOwner),
    UnorderedStorage(CatalogOwner),
    SharedStorageMismatch,
    CostOverflow(CatalogCostField),
}

/// Sum each qualified storage identity once without allocating or initializing
/// metadata, validators, registry indexes, bidders or lazy caches.
/// The result feeds the existing runtime equation unchanged: this function
/// applies no auction/auxiliary populations and never rewrites baked defaults.
///
/// # Errors
/// Refuses missing, duplicate, stale or mismatched owners, ambiguous sharing,
/// unordered storage and arithmetic overflow. No partial total is returned.
pub fn aggregate_catalog_costs(
    requirements: &CatalogRequirements<'_>,
    contributions: &[QualifiedCatalogContribution<'_>],
) -> Result<CatalogResourceCosts, CatalogCostError> {
    let build = requirements.build;
    if build.target_abi.is_empty()
        || build.features_sha256 == [0; 32]
        || build.toolchain_sha256 == [0; 32]
        || build.lock_sha256 == [0; 32]
    {
        return Err(CatalogCostError::InvalidBuildIdentity);
    }
    let storage_streams = [
        checked_owner(
            build,
            CatalogOwner::Metadata,
            requirements.metadata,
            contributions,
        )?,
        checked_owner(
            build,
            CatalogOwner::Validators,
            requirements.validators,
            contributions,
        )?,
        checked_owner(
            build,
            CatalogOwner::Registry,
            requirements.registry,
            contributions,
        )?,
        checked_owner(
            build,
            CatalogOwner::Context,
            requirements.context,
            contributions,
        )?,
    ];
    let mut streams = storage_streams.map(|storage| storage.iter().peekable());
    let mut total = CatalogResourceCosts::UNCHANGED;
    // Four sorted owner streams make sharing a bounded, allocation-free merge.
    // Do not build a temporary set or collapse distinct equal-sized storage.
    while let Some(next) = streams
        .iter_mut()
        .filter_map(|stream| stream.peek().map(|storage| storage.identity))
        .min()
    {
        let mut shared = None;
        for stream in &mut streams {
            if let Some(storage) = stream.next_if(|storage| storage.identity == next) {
                if let Some(previous) = shared {
                    if storage.costs != previous {
                        return Err(CatalogCostError::SharedStorageMismatch);
                    }
                } else {
                    total = add_costs(total, storage.costs)?;
                    shared = Some(storage.costs);
                }
            }
        }
    }
    Ok(total)
}

fn checked_owner<'a>(
    build: CatalogBuildIdentity<'_>,
    owner: CatalogOwner,
    required: CatalogRequirement,
    contributions: &'a [QualifiedCatalogContribution<'_>],
) -> Result<&'a [CatalogStorage], CatalogCostError> {
    if required.source_sha256 == [0; 32] || required.qualification_sha256 == [0; 32] {
        return Err(CatalogCostError::InvalidRequirement(owner));
    }
    let mut matches = contributions.iter().filter(|row| row.owner == owner);
    let row = matches
        .next()
        .ok_or(CatalogCostError::MissingOwner(owner))?;
    if matches.next().is_some() {
        return Err(CatalogCostError::DuplicateOwner(owner));
    }
    if row.build != build {
        return Err(CatalogCostError::BuildMismatch(owner));
    }
    if row.source_sha256 != required.source_sha256 {
        return Err(CatalogCostError::SourceMismatch(owner));
    }
    if row.qualification_sha256 != required.qualification_sha256 {
        return Err(CatalogCostError::QualificationMismatch(owner));
    }
    let mut previous = None;
    for storage in row.storage {
        if storage.identity == [0; 32] {
            return Err(CatalogCostError::InvalidStorageIdentity(owner));
        }
        if previous.is_some_and(|identity| identity >= storage.identity) {
            return Err(CatalogCostError::UnorderedStorage(owner));
        }
        previous = Some(storage.identity);
    }
    Ok(row.storage)
}

fn add_costs(
    total: CatalogResourceCosts,
    next: CatalogResourceCosts,
) -> Result<CatalogResourceCosts, CatalogCostError> {
    Ok(CatalogResourceCosts {
        resident_bytes: total
            .resident_bytes
            .checked_add(next.resident_bytes)
            .ok_or(CatalogCostError::CostOverflow(CatalogCostField::Resident))?,
        initialization_bytes: total
            .initialization_bytes
            .checked_add(next.initialization_bytes)
            .ok_or(CatalogCostError::CostOverflow(
                CatalogCostField::Initialization,
            ))?,
        schema_validation_per_auction_bytes: total
            .schema_validation_per_auction_bytes
            .checked_add(next.schema_validation_per_auction_bytes)
            .ok_or(CatalogCostError::CostOverflow(
                CatalogCostField::SchemaValidation,
            ))?,
        context_per_auction_bytes: total
            .context_per_auction_bytes
            .checked_add(next.context_per_auction_bytes)
            .ok_or(CatalogCostError::CostOverflow(CatalogCostField::Context))?,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CatalogBuildIdentity, CatalogCostError, CatalogCostField, CatalogOwner, CatalogRequirement,
        CatalogRequirements, CatalogResourceCosts, CatalogStorage, QualifiedCatalogContribution,
        aggregate_catalog_costs,
    };
    use std::array;

    const OWNERS: [CatalogOwner; 4] = [
        CatalogOwner::Metadata,
        CatalogOwner::Validators,
        CatalogOwner::Registry,
        CatalogOwner::Context,
    ];

    fn requirements() -> CatalogRequirements<'static> {
        CatalogRequirements {
            build: CatalogBuildIdentity {
                target_abi: "native-test-abi",
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
        }
    }

    fn requirement(
        requirements: &CatalogRequirements<'_>,
        owner: CatalogOwner,
    ) -> CatalogRequirement {
        match owner {
            CatalogOwner::Metadata => requirements.metadata,
            CatalogOwner::Validators => requirements.validators,
            CatalogOwner::Registry => requirements.registry,
            CatalogOwner::Context => requirements.context,
        }
    }

    fn contribution<'a>(
        requirements: &CatalogRequirements<'a>,
        owner: CatalogOwner,
        storage: &'a [CatalogStorage],
    ) -> QualifiedCatalogContribution<'a> {
        let required = requirement(requirements, owner);
        QualifiedCatalogContribution {
            owner,
            build: requirements.build,
            source_sha256: required.source_sha256,
            qualification_sha256: required.qualification_sha256,
            storage,
        }
    }

    fn costs(values: [u64; 4]) -> CatalogResourceCosts {
        let [resident, initialization, validation, context] = values;
        CatalogResourceCosts {
            resident_bytes: resident,
            initialization_bytes: initialization,
            schema_validation_per_auction_bytes: validation,
            context_per_auction_bytes: context,
        }
    }

    fn storage(identity: u8, values: [u64; 4]) -> CatalogStorage {
        CatalogStorage {
            identity: [identity; 32],
            costs: costs(values),
        }
    }

    #[test]
    fn every_owner_and_lifetime_reaches_the_total_once() {
        let required = requirements();
        let storage_rows = [
            [storage(1, [1, 2, 3, 5])],
            [storage(2, [7, 11, 13, 17])],
            [storage(3, [19, 23, 29, 31])],
            [storage(4, [37, 41, 43, 47])],
        ];
        let rows =
            array::from_fn::<_, 4, _>(|i| contribution(&required, OWNERS[i], &storage_rows[i]));
        assert_eq!(
            aggregate_catalog_costs(&required, &rows),
            Ok(costs([64, 77, 88, 100]))
        );
        for owner in 0..4 {
            for field in 0..4 {
                let mut changed = storage_rows;
                let mut values = [0; 4];
                values[field] = 1;
                changed[owner][0] = storage(9, values);
                let changed_rows =
                    array::from_fn::<_, 4, _>(|i| contribution(&required, OWNERS[i], &changed[i]));
                let original = [
                    [1, 2, 3, 5],
                    [7, 11, 13, 17],
                    [19, 23, 29, 31],
                    [37, 41, 43, 47],
                ][owner];
                let mut expected: [u64; 4] = [64, 77, 88, 100];
                for index in 0..4 {
                    expected[index] = expected[index].checked_sub(original[index]).unwrap();
                }
                expected[field] = expected[field].checked_add(1).unwrap();
                assert_eq!(
                    aggregate_catalog_costs(&required, &changed_rows),
                    Ok(costs(expected))
                );
            }
        }
    }

    #[test]
    fn sharing_depends_on_identity_not_pointer_or_equal_byte_counts() {
        let required = requirements();
        let common = storage(3, [2, 3, 5, 7]);
        let metadata = [storage(1, [11, 13, 17, 19]), common];
        let validators = [common, storage(9, [23, 29, 31, 37])];
        let registry = [storage(5, [2, 3, 5, 7])];
        let context = [common];
        let mut rows = [
            contribution(&required, CatalogOwner::Metadata, &metadata),
            contribution(&required, CatalogOwner::Validators, &validators),
            contribution(&required, CatalogOwner::Registry, &registry),
            contribution(&required, CatalogOwner::Context, &context),
        ];
        for _ in OWNERS {
            assert_eq!(
                aggregate_catalog_costs(&required, &rows),
                Ok(costs([38, 48, 58, 70]))
            );
            rows.rotate_left(1);
        }
    }

    #[test]
    fn shared_maximum_cost_is_counted_once_without_an_intermediate_overflow() {
        let required = requirements();
        let shared = [storage(1, [u64::MAX; 4])];
        let rows = OWNERS.map(|owner| contribution(&required, owner, &shared));
        assert_eq!(
            aggregate_catalog_costs(&required, &rows),
            Ok(costs([u64::MAX; 4]))
        );
    }

    #[test]
    fn storage_identity_comparison_includes_every_digest_byte() {
        let required = requirements();
        for byte in 0..32 {
            let mut second_identity = [1; 32];
            second_identity[byte] = 2;
            let first = [storage(1, [1, 2, 3, 5])];
            let second = [CatalogStorage {
                identity: second_identity,
                costs: costs([7, 11, 13, 17]),
            }];
            let rows = [
                contribution(&required, CatalogOwner::Metadata, &first),
                contribution(&required, CatalogOwner::Validators, &second),
                contribution(&required, CatalogOwner::Registry, &[]),
                contribution(&required, CatalogOwner::Context, &[]),
            ];
            assert_eq!(
                aggregate_catalog_costs(&required, &rows),
                Ok(costs([8, 13, 16, 22]))
            );
        }
    }

    #[test]
    fn explicit_empty_records_are_not_missing_records() {
        let required = requirements();
        let rows = OWNERS.map(|owner| contribution(&required, owner, &[]));
        assert_eq!(aggregate_catalog_costs(&required, &rows), Ok(costs([0; 4])));
        for owner in OWNERS {
            let remaining = rows
                .into_iter()
                .filter(|row| row.owner != owner)
                .collect::<Vec<_>>();
            assert_eq!(
                aggregate_catalog_costs(&required, &remaining),
                Err(CatalogCostError::MissingOwner(owner))
            );
        }
    }

    #[test]
    fn duplicate_owner_refuses_even_when_both_records_agree() {
        let required = requirements();
        let rows = OWNERS.map(|owner| contribution(&required, owner, &[]));
        for duplicate in rows {
            let mut repeated = rows.to_vec();
            repeated.push(duplicate);
            assert_eq!(
                aggregate_catalog_costs(&required, &repeated),
                Err(CatalogCostError::DuplicateOwner(duplicate.owner))
            );
        }
    }

    #[test]
    fn each_owners_source_and_reviewed_record_are_independently_bound() {
        let required = requirements();
        for owner in OWNERS {
            let mut rows = OWNERS.map(|entry_owner| contribution(&required, entry_owner, &[]));
            let source_row = rows.iter_mut().find(|row| row.owner == owner).unwrap();
            source_row.source_sha256 = [99; 32];
            assert_eq!(
                aggregate_catalog_costs(&required, &rows),
                Err(CatalogCostError::SourceMismatch(owner))
            );
            let record_row = rows.iter_mut().find(|row| row.owner == owner).unwrap();
            record_row.source_sha256 = requirement(&required, owner).source_sha256;
            record_row.qualification_sha256 = [99; 32];
            assert_eq!(
                aggregate_catalog_costs(&required, &rows),
                Err(CatalogCostError::QualificationMismatch(owner))
            );
        }
    }

    #[test]
    fn target_features_toolchain_and_lock_cannot_be_mixed_across_owners() {
        let required = requirements();
        let builds = [
            CatalogBuildIdentity {
                target_abi: "another-abi",
                ..required.build
            },
            CatalogBuildIdentity {
                features_sha256: [99; 32],
                ..required.build
            },
            CatalogBuildIdentity {
                toolchain_sha256: [99; 32],
                ..required.build
            },
            CatalogBuildIdentity {
                lock_sha256: [99; 32],
                ..required.build
            },
        ];
        for owner in OWNERS {
            for build in builds {
                let mut rows = OWNERS.map(|entry_owner| contribution(&required, entry_owner, &[]));
                rows.iter_mut()
                    .find(|row| row.owner == owner)
                    .unwrap()
                    .build = build;
                assert_eq!(
                    aggregate_catalog_costs(&required, &rows),
                    Err(CatalogCostError::BuildMismatch(owner))
                );
            }
        }
    }

    #[test]
    fn unknown_expected_build_facts_do_not_certify_matching_empty_facts() {
        let required = requirements();
        let builds = [
            CatalogBuildIdentity {
                target_abi: "",
                ..required.build
            },
            CatalogBuildIdentity {
                features_sha256: [0; 32],
                ..required.build
            },
            CatalogBuildIdentity {
                toolchain_sha256: [0; 32],
                ..required.build
            },
            CatalogBuildIdentity {
                lock_sha256: [0; 32],
                ..required.build
            },
        ];
        for build in builds {
            let changed = CatalogRequirements { build, ..required };
            let rows = OWNERS.map(|owner| contribution(&changed, owner, &[]));
            assert_eq!(
                aggregate_catalog_costs(&changed, &rows),
                Err(CatalogCostError::InvalidBuildIdentity)
            );
        }
    }

    #[test]
    fn unpublished_required_source_or_record_is_not_an_unchanged_owner() {
        for owner in OWNERS {
            for missing_source in [true, false] {
                let mut required = requirements();
                let field = match owner {
                    CatalogOwner::Metadata => &mut required.metadata,
                    CatalogOwner::Validators => &mut required.validators,
                    CatalogOwner::Registry => &mut required.registry,
                    CatalogOwner::Context => &mut required.context,
                };
                if missing_source {
                    field.source_sha256 = [0; 32];
                } else {
                    field.qualification_sha256 = [0; 32];
                }
                let rows = OWNERS.map(|entry_owner| contribution(&required, entry_owner, &[]));
                assert_eq!(
                    aggregate_catalog_costs(&required, &rows),
                    Err(CatalogCostError::InvalidRequirement(owner))
                );
            }
        }
    }

    #[test]
    fn each_storage_stream_requires_nonempty_unique_ordered_identities() {
        let required = requirements();
        for owner in OWNERS {
            for (invalid_storage, error) in [
                (
                    vec![storage(0, [1; 4])],
                    CatalogCostError::InvalidStorageIdentity(owner),
                ),
                (
                    vec![storage(2, [1; 4]), storage(1, [2; 4])],
                    CatalogCostError::UnorderedStorage(owner),
                ),
                (
                    vec![storage(1, [1; 4]), storage(1, [1; 4])],
                    CatalogCostError::UnorderedStorage(owner),
                ),
            ] {
                let mut rows = OWNERS.map(|entry_owner| contribution(&required, entry_owner, &[]));
                rows.iter_mut()
                    .find(|row| row.owner == owner)
                    .unwrap()
                    .storage = &invalid_storage;
                assert_eq!(aggregate_catalog_costs(&required, &rows), Err(error));
            }
        }
    }

    #[test]
    fn conflicting_shared_storage_refuses_each_lifetime_independently() {
        let required = requirements();
        let original = [storage(1, [2, 3, 5, 7])];
        for values in [[1, 3, 5, 7], [2, 1, 5, 7], [2, 3, 1, 7], [2, 3, 5, 1]] {
            let changed = [storage(1, values)];
            let rows = [
                contribution(&required, CatalogOwner::Metadata, &original),
                contribution(&required, CatalogOwner::Validators, &changed),
                contribution(&required, CatalogOwner::Registry, &[]),
                contribution(&required, CatalogOwner::Context, &[]),
            ];
            assert_eq!(
                aggregate_catalog_costs(&required, &rows),
                Err(CatalogCostError::SharedStorageMismatch)
            );
        }
    }

    #[test]
    fn each_total_refuses_overflow_without_saturating_or_publishing_a_partial_sum() {
        let required = requirements();
        for (index, field) in [
            CatalogCostField::Resident,
            CatalogCostField::Initialization,
            CatalogCostField::SchemaValidation,
            CatalogCostField::Context,
        ]
        .into_iter()
        .enumerate()
        {
            let mut maximum = [0; 4];
            maximum[index] = u64::MAX;
            let mut extra = [0; 4];
            extra[index] = 1;
            let large = [storage(1, maximum)];
            let small = [storage(2, extra)];
            let mut rows = [
                contribution(&required, CatalogOwner::Metadata, &large),
                contribution(&required, CatalogOwner::Validators, &[]),
                contribution(&required, CatalogOwner::Registry, &[]),
                contribution(&required, CatalogOwner::Context, &[]),
            ];
            assert_eq!(
                aggregate_catalog_costs(&required, &rows),
                Ok(costs(maximum))
            );
            rows[3].storage = &small;
            assert_eq!(
                aggregate_catalog_costs(&required, &rows),
                Err(CatalogCostError::CostOverflow(field))
            );
        }
    }

    #[test]
    fn complete_record_validation_precedes_cost_arithmetic() {
        let required = requirements();
        let overflowing = [storage(1, [u64::MAX; 4]), storage(2, [1; 4])];
        let rows = [
            contribution(&required, CatalogOwner::Metadata, &overflowing),
            contribution(&required, CatalogOwner::Validators, &[]),
            contribution(&required, CatalogOwner::Registry, &[]),
        ];
        assert_eq!(
            aggregate_catalog_costs(&required, &rows),
            Err(CatalogCostError::MissingOwner(CatalogOwner::Context))
        );
    }
}

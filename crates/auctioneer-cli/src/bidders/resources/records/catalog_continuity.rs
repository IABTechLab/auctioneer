//! Bounded capture continuity, not physical residency or target qualification.

use super::{AdapterId, Component, Error, PreservedChooserGrowth, aux_population, require, same};

const CHOOSER_INCREMENT_BYTES: u64 = 0x0001_0000;

pub(super) fn verify(
    mut original: super::Audit,
    rebased: &super::Audit,
) -> Result<super::CatalogContinuity, super::Error> {
    require(
        original.code_base_commit == "e71e40aba611215bb9305f82e2449e13fb0f41ed"
            && original.lock_sha256
                == "f20a05604c912c0d9946e7c86020f1e12549abbf6007bfe8d56883dda1a7a1c1"
            && rebased.code_base_commit == "7286bedfd1f1999edae9a5e3e5665b1f95117541"
            && rebased.lock_sha256
                == "aca2f5fc543402752f6e8c39aa79fa0c9269600c8ad08c2a617d81ec27ac6dbc",
    )?;
    require(
        original.adapters.len() == AdapterId::ALL.len()
            && rebased.adapters.len() == AdapterId::ALL.len(),
    )?;
    let mut continuity = super::CatalogContinuity {
        original_code_base_commit: original.code_base_commit.clone(),
        original_sha256: "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39"
            .to_owned(),
        adapters: Vec::new(),
    };
    original
        .code_base_commit
        .clone_from(&rebased.code_base_commit);
    original.lock_sha256.clone_from(&rebased.lock_sha256);
    for ((previous, current), adapter) in original
        .adapters
        .iter_mut()
        .zip(&rebased.adapters)
        .zip(AdapterId::ALL)
    {
        require(previous.report.adapter == adapter && current.report.adapter == adapter)?;
        let component_delta = CHOOSER_INCREMENT_BYTES
            .checked_mul(aux_population(&previous.report, &original.runtime_facts)?)
            .ok_or(Error::ResourceInput)?;
        let original_total_bytes = previous.report.derived.total_bytes;
        let identity = &mut previous.report.identity;
        identity
            .code_base_commit
            .clone_from(&rebased.code_base_commit);
        identity.lock_sha256.clone_from(&rebased.lock_sha256);
        identity
            .source_sha256
            .clone_from(&current.report.identity.source_sha256);
        identity.source_file_count = current.report.identity.source_file_count;
        previous.report.effective_profile_sha256 = super::super::profile_digest(&previous.report)?;
        previous.auxiliary_receipts.chooser = previous
            .auxiliary_receipts
            .chooser
            .checked_add(CHOOSER_INCREMENT_BYTES)
            .ok_or(Error::ResourceInput)?;
        let receipt = chooser_component(previous)?;
        receipt.bytes = receipt
            .bytes
            .checked_add(component_delta)
            .ok_or(Error::ResourceInput)?;
        continuity.adapters.push(PreservedChooserGrowth {
            adapter,
            original_total_bytes,
            rebased_total_bytes: current.report.derived.total_bytes,
            auxiliary_chooser_per_permit_delta_bytes: CHOOSER_INCREMENT_BYTES,
            auxiliary_chooser_component_delta_bytes: component_delta,
        });
    }
    require(same(&original, rebased)?)?;
    Ok(continuity)
}

fn chooser_component(
    target: &mut super::AdapterAudit,
) -> Result<&mut super::ComponentReceipt, Error> {
    let mut receipts = target
        .component_receipts
        .iter_mut()
        .filter(|receipt| receipt.component == Component::AuxChooser);
    let receipt = receipts.next().ok_or(Error::ResourceInput)?;
    require(receipts.next().is_none())?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::super::super::profile_digest;
    use super::super::{AdapterId, Audit, Component, Error, parse};
    use super::verify;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};

    const ORIGINAL: &[u8] = include_bytes!(
        "../../../../../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline.toml"
    );
    const REBASED: &[u8] = include_bytes!(
        "../../../../../auctioneer-core/tests/fixtures/resource_envelopes/catalog-baseline-patched-lock.toml"
    );

    fn captures() -> (Audit, Audit) {
        (parse(ORIGINAL).unwrap(), parse(REBASED).unwrap())
    }

    fn rejects(change: impl FnOnce(&mut Audit, &mut Audit)) {
        let (mut original, mut rebased) = captures();
        change(&mut original, &mut rebased);
        assert!(matches!(
            verify(original, &rebased),
            Err(Error::ResourceInput)
        ));
    }

    fn mutate(audit: &mut Audit, change: impl FnOnce(&mut Value)) {
        let mut value = serde_json::to_value(&*audit).unwrap();
        change(&mut value);
        *audit = serde_json::from_value(value).unwrap();
        for target in &mut audit.adapters {
            target.report.effective_profile_sha256 = profile_digest(&target.report).unwrap();
        }
    }

    fn rejects_rebased_field(pointer: &str, replacement: Value) {
        let (original, mut rebased) = captures();
        mutate(&mut rebased, |value| {
            let field = value.pointer_mut(pointer).unwrap();
            assert_ne!(*field, replacement, "control must change {pointer}");
            *field = replacement;
        });
        assert!(
            matches!(verify(original, &rebased), Err(Error::ResourceInput)),
            "{pointer}"
        );
    }

    #[test]
    fn captures_have_the_recorded_raw_identities() {
        assert_eq!(
            hex::encode(Sha256::digest(ORIGINAL)),
            "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39"
        );
        assert_eq!(
            hex::encode(Sha256::digest(REBASED)),
            "1d20dfa95f09a32a45f106f5503094efd71a718d794814fac353f83f357219d0"
        );
        let (original, rebased) = captures();
        assert_eq!(
            original.code_base_commit,
            "e71e40aba611215bb9305f82e2449e13fb0f41ed"
        );
        assert_eq!(
            rebased.code_base_commit,
            "7286bedfd1f1999edae9a5e3e5665b1f95117541"
        );
        assert_eq!(
            original.lock_sha256,
            "f20a05604c912c0d9946e7c86020f1e12549abbf6007bfe8d56883dda1a7a1c1"
        );
        assert_eq!(
            rebased.lock_sha256,
            "aca2f5fc543402752f6e8c39aa79fa0c9269600c8ad08c2a617d81ec27ac6dbc"
        );
    }

    #[test]
    fn preserves_literal_totals_and_reports_only_chooser_growth() {
        let (original, rebased) = captures();
        let continuity = verify(original, &rebased).unwrap();
        assert_eq!(
            continuity.original_code_base_commit,
            "e71e40aba611215bb9305f82e2449e13fb0f41ed"
        );
        assert_eq!(
            continuity.original_sha256,
            "6da360d57f08267018035cedce6157ada2e955ab08913517a5d6020dee0b9e39"
        );
        let actual: Vec<_> = continuity
            .adapters
            .iter()
            .map(|row| {
                (
                    row.adapter,
                    row.original_total_bytes,
                    row.rebased_total_bytes,
                    row.auxiliary_chooser_per_permit_delta_bytes,
                    row.auxiliary_chooser_component_delta_bytes,
                )
            })
            .collect();
        assert_eq!(
            actual,
            [
                (
                    AdapterId::Axum,
                    2_533_631_672,
                    2_533_631_672,
                    0x0001_0000,
                    0x0002_0000
                ),
                (
                    AdapterId::Cloudflare,
                    138_048_496_272,
                    138_048_496_272,
                    0x0001_0000,
                    0x0008_0000,
                ),
                (
                    AdapterId::Fastly,
                    23_589_646_360,
                    23_589_646_360,
                    0x0001_0000,
                    0x0002_0000,
                ),
                (
                    AdapterId::Spin,
                    23_589_646_360,
                    23_589_646_360,
                    0x0001_0000,
                    0x0002_0000,
                ),
            ]
        );
    }

    #[test]
    fn complete_receipts_fit_the_unchanged_reservations() {
        let (original, rebased) = captures();
        let expected = [
            (AdapterId::Axum, 794_624_u64, 925_696_u64, 2_u64),
            (AdapterId::Cloudflare, 3_178_496, 3_702_784, 8),
            (AdapterId::Fastly, 794_624, 925_696, 2),
            (AdapterId::Spin, 794_624, 925_696, 2),
        ];
        assert_eq!(original.adapters.len(), expected.len());
        assert_eq!(rebased.adapters.len(), expected.len());
        for ((previous, current), (adapter, old_bytes, new_bytes, population)) in original
            .adapters
            .iter()
            .zip(&rebased.adapters)
            .zip(expected)
        {
            assert_eq!(previous.report.adapter, adapter);
            assert_eq!(current.report.adapter, adapter);
            assert_eq!(previous.auxiliary_receipts.chooser, 397_312);
            assert_eq!(current.auxiliary_receipts.chooser, 462_848);
            assert_eq!(
                current
                    .auxiliary_receipts
                    .chooser
                    .checked_sub(previous.auxiliary_receipts.chooser),
                Some(0x0001_0000)
            );
            for (target, bytes) in [(previous, old_bytes), (current, new_bytes)] {
                let receipts: Vec<_> = target
                    .component_receipts
                    .iter()
                    .filter(|receipt| receipt.component == Component::AuxChooser)
                    .collect();
                assert_eq!(receipts.len(), 1);
                assert_eq!(receipts[0].bytes, bytes);
                assert!(
                    target.auxiliary_receipts.total().unwrap()
                        <= target.report.effective_limits.aux_route_peak_bytes
                );
                assert!(!target.report.required_fit);
            }
            assert_eq!(
                new_bytes.checked_sub(old_bytes),
                0x0001_0000_u64.checked_mul(population)
            );
            assert_eq!(
                previous.report.effective_limits.aux_route_peak_bytes,
                current.report.effective_limits.aux_route_peak_bytes
            );
            assert_eq!(
                previous.report.derived.total_bytes,
                current.report.derived.total_bytes
            );
        }
    }

    #[test]
    fn refuses_other_top_level_identities_and_runtime_facts() {
        for (pointer, replacement) in [
            ("/schema_version", json!(2_u16)),
            ("/sdk_revision", json!("0".repeat(40))),
            ("/toolchain", json!("different compiler")),
            ("/features", json!(["application", "different_feature"])),
            ("/runtime_facts/probe_config_min_permits", json!(2_u32)),
            ("/runtime_facts/probe_response_permits", json!(5_u32)),
            ("/runtime_facts/gvl_cache_max_entries", json!(5_usize)),
        ] {
            rejects_rebased_field(pointer, replacement);
        }
    }

    #[test]
    fn every_adapter_retains_its_limits_owners_and_pending_evidence() {
        for index in 0_usize..4 {
            for (suffix, replacement) in [
                ("/report/raw_limits/baseline_bytes", json!(1_u64)),
                ("/owners/0/allocation_owner", json!("changed-owner")),
                ("/pending_facts/0/reason", json!("changed-reason")),
            ] {
                rejects_rebased_field(&format!("/adapters/{index}{suffix}"), replacement);
            }
        }
    }

    #[test]
    fn refuses_non_code_row_identities_even_with_fresh_profile_hashes() {
        for (field, replacement) in [
            ("sdk_revision", json!("0".repeat(40))),
            ("features", json!(["application", "different_feature"])),
            ("declared_toolchain", json!("different declaration")),
            ("compiler_verbose", json!("different compiler")),
            ("compiled_target", json!("different-target")),
            ("reporter_abi", json!("different-reporter-abi")),
            ("target_abi", json!("different-target-abi")),
            ("layout_evidence", json!("host_native_substitute")),
            ("artifact_proof", json!("0".repeat(64))),
            ("dirty", json!(true)),
            ("source_tree_matches_commit", json!(false)),
        ] {
            rejects_rebased_field(&format!("/adapters/0/report/identity/{field}"), replacement);
        }
    }

    #[test]
    fn refuses_other_profile_facts_limits_and_costs() {
        for (field, replacement) in [
            ("platform_profile", json!("different-platform")),
            ("dispatch_profile", json!("different-dispatch")),
            ("memory_scope", json!("different-scope")),
            (
                "platform_facts/ceiling/reason",
                json!("different platform fact"),
            ),
            (
                "dispatch_facts/inbound_population/reason",
                json!("different dispatch fact"),
            ),
            ("raw_limits/aux_route_permits", json!(2_u32)),
            ("effective_limits/aux_route_permits", json!(2_u32)),
            (
                "effective_limits/aux_route_peak_bytes",
                json!(134_283_264_u64),
            ),
            (
                "target_constants/platform_concurrent_ceiling",
                json!(385_u32),
            ),
            ("construction_costs/fixed_per_auction_bytes", json!(801_u64)),
            ("catalog_costs/resident_bytes", json!(1_u64)),
            ("outbound_meta_bytes", json!(4065_u32)),
            ("baked_config_bytes", json!(1585_u64)),
            ("baked_config_sha256", json!("0".repeat(64))),
            ("actual_gvl_cache_entries", json!(5_usize)),
            ("required_fit", json!(true)),
        ] {
            rejects_rebased_field(&format!("/adapters/0/report/{field}"), replacement);
        }
    }

    #[test]
    fn refuses_numeric_term_receipt_and_owner_changes() {
        for (pointer, replacement) in [
            ("/adapters/0/report/terms/baseline", json!(144_769_025_u64)),
            ("/adapters/0/report/terms/auxiliary", json!(537_010_176_u64)),
            (
                "/adapters/0/report/derived/total_bytes",
                json!(2_533_762_744_u64),
            ),
            (
                "/adapters/0/report/derived/isolate_terms/auxiliary",
                json!(537_010_176_u64),
            ),
            (
                "/adapters/0/report/derived/aux_term",
                json!(537_010_176_u64),
            ),
            (
                "/adapters/0/report/auction_receipts/config",
                json!(30_603_265_u64),
            ),
            (
                "/adapters/0/report/derived/auction_charges/config",
                json!(30_603_265_u64),
            ),
            (
                "/adapters/0/component_receipts/0/bytes",
                json!(61_206_529_u64),
            ),
            ("/adapters/0/owners/0/population/value", json!(2_u64)),
            (
                "/adapters/0/owners/0/allocation_owner",
                json!("different owner"),
            ),
            ("/adapters/0/owners/0/source_path", json!("crates/other.rs")),
        ] {
            rejects_rebased_field(pointer, replacement);
        }
    }

    #[test]
    fn refuses_every_non_chooser_auxiliary_receipt_change() {
        for field in [
            "config",
            "body",
            "query",
            "input_headers",
            "cookie",
            "signing",
            "nodes",
            "privacy",
            "urls",
            "retained_urls",
            "output",
        ] {
            rejects_rebased_field(
                &format!("/adapters/0/auxiliary_receipts/{field}"),
                json!(0_u64),
            );
        }
    }

    #[test]
    fn refuses_pending_decision_artifact_and_abi_changes() {
        for (pointer, replacement) in [
            (
                "/adapters/0/pending_facts/0/reason",
                json!("different pending reason"),
            ),
            (
                "/adapters/0/pending_facts/0/owner",
                json!("different pending owner"),
            ),
            (
                "/adapters/0/decisions/0/rationale",
                json!("different decision"),
            ),
            (
                "/adapters/0/decisions/0/class",
                json!("accounting_correction"),
            ),
            (
                "/adapters/0/artifact/reason",
                json!("different artifact evidence"),
            ),
            ("/adapters/0/abi/reason", json!("different ABI evidence")),
            (
                "/adapters/0/owners/0/abi_bound/reason",
                json!("different owner ABI evidence"),
            ),
            (
                "/adapters/0/gap_bytes/reason",
                json!("different gap evidence"),
            ),
            (
                "/adapters/0/report/validation/external_blockers/0",
                json!("different blocker"),
            ),
            (
                "/adapters/0/report/validation/profile_matches_dispatch",
                json!(false),
            ),
        ] {
            rejects_rebased_field(pointer, replacement);
        }
    }

    #[test]
    fn refuses_missing_members_on_either_side() {
        rejects(|original, _rebased| {
            original.adapters.pop().unwrap();
        });
        rejects(|_original, rebased| {
            rebased.adapters.pop().unwrap();
        });
        rejects(|original, rebased| {
            original.adapters.pop().unwrap();
            rebased.adapters.pop().unwrap();
        });
    }

    #[test]
    fn refuses_duplicate_members_on_either_side() {
        rejects(|original, _rebased| {
            original.adapters[1].report.adapter = AdapterId::Axum;
        });
        rejects(|_original, rebased| {
            rebased.adapters[1].report.adapter = AdapterId::Axum;
        });
        rejects(|original, rebased| {
            original.adapters[1].report.adapter = AdapterId::Axum;
            rebased.adapters[1].report.adapter = AdapterId::Axum;
        });
    }

    #[test]
    fn refuses_reordered_members_on_either_side() {
        rejects(|original, _rebased| original.adapters.swap(0, 1));
        rejects(|_original, rebased| rebased.adapters.swap(0, 1));
        rejects(|original, rebased| {
            original.adapters.swap(0, 1);
            rebased.adapters.swap(0, 1);
        });
    }

    #[test]
    fn refuses_unrecorded_commit_and_lock_transitions() {
        rejects(|original, _rebased| original.code_base_commit = "0".repeat(40));
        rejects(|_original, rebased| rebased.code_base_commit = "0".repeat(40));
        rejects(|original, rebased| original.lock_sha256.clone_from(&rebased.lock_sha256));
        rejects(|original, rebased| rebased.lock_sha256.clone_from(&original.lock_sha256));
        rejects(|original, _rebased| original.lock_sha256 = "0".repeat(64));
        rejects(|_original, rebased| rebased.lock_sha256 = "0".repeat(64));
        rejects(|original, rebased| {
            original.lock_sha256 = "0".repeat(64);
            rebased.lock_sha256 = "0".repeat(64);
        });
        rejects_rebased_field(
            "/adapters/0/report/identity/lock_sha256",
            json!("0".repeat(64)),
        );
        rejects_rebased_field(
            "/adapters/0/report/identity/code_base_commit",
            json!("0".repeat(40)),
        );
    }

    #[test]
    fn refuses_zero_incorrect_and_reduced_chooser_increments() {
        for chooser in [397_312_u64, 0x0007_0FFF, 462_849, 397_311] {
            rejects_rebased_field("/adapters/0/auxiliary_receipts/chooser", json!(chooser));
        }
    }

    #[test]
    fn refuses_omitted_doubled_and_unscaled_component_growth() {
        for (adapter_index, bytes) in [
            (0_usize, 794_624_u64),
            (0, 1_056_768),
            (0, 860_160),
            (1, 3_178_496),
            (1, 4_227_072),
            (1, 3_244_032),
        ] {
            rejects(|_original, rebased| {
                let receipt = rebased.adapters[adapter_index]
                    .component_receipts
                    .iter_mut()
                    .find(|receipt| receipt.component == Component::AuxChooser)
                    .unwrap();
                receipt.bytes = bytes;
            });
        }
    }

    #[test]
    fn refuses_missing_or_duplicate_chooser_components() {
        rejects(|_original, rebased| {
            rebased.adapters[0]
                .component_receipts
                .retain(|receipt| receipt.component != Component::AuxChooser);
        });
        rejects(|original, rebased| {
            for audit in [original, rebased] {
                audit.adapters[0]
                    .component_receipts
                    .retain(|receipt| receipt.component != Component::AuxChooser);
            }
        });
        rejects(|original, rebased| {
            for audit in [original, rebased] {
                mutate(audit, |value| {
                    let receipts = value["adapters"][0]["component_receipts"]
                        .as_array_mut()
                        .unwrap();
                    let duplicate = receipts
                        .iter()
                        .find(|receipt| receipt["component"] == "aux_chooser")
                        .unwrap()
                        .clone();
                    receipts.push(duplicate);
                });
            }
        });
    }

    #[test]
    fn refuses_double_charging_the_isolate_reservation() {
        rejects(|_original, rebased| {
            mutate(rebased, |value| {
                let report = &mut value["adapters"][0]["report"];
                report["terms"]["auxiliary"] = json!(537_010_176_u64);
                report["derived"]["isolate_terms"]["auxiliary"] = json!(537_010_176_u64);
                report["derived"]["aux_term"] = json!(537_010_176_u64);
                report["derived"]["total_bytes"] = json!(2_533_762_744_u64);
                report["derived"]["margin_bytes"] = json!(1_761_204_552_u64);
            });
        });
    }

    #[test]
    fn refuses_forged_profile_hashes_without_normalizing_them() {
        rejects(|_original, rebased| {
            rebased.adapters[0].report.effective_profile_sha256 = "0".repeat(64);
        });
        rejects(|original, rebased| {
            rebased.adapters[0]
                .report
                .effective_profile_sha256
                .clone_from(&original.adapters[0].report.effective_profile_sha256);
        });
    }

    #[test]
    fn refuses_chooser_arithmetic_overflow_in_the_original() {
        rejects(|original, _rebased| {
            original.adapters[0].auxiliary_receipts.chooser = u64::MAX;
        });
        rejects(|original, _rebased| {
            original.adapters[0]
                .component_receipts
                .iter_mut()
                .find(|receipt| receipt.component == Component::AuxChooser)
                .unwrap()
                .bytes = u64::MAX;
        });
    }
}

//! Reconstructed native candidates: shallow Arc requests, never complete programs.

use crate::schema_heap_layout::native_arc_value_layout;
use regex_automata::{meta::RegexResourceLayout, util::prefilter::PrefilterResourceLayout};
use serde::{Deserialize, Serialize};
use std::alloc::Layout;
use std::collections::BTreeSet;

/// The caller independently verifies this context before accepting any rows.
/// Matching strings here do not certify an archive, patch, tree or toolchain.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Identity {
    pub engine_version: String,
    pub archive_sha256: String,
    pub patch_sha256: String,
    pub tree_sha256: String,
    pub compiler: String,
    pub target: String,
    pub features: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CandidateOrigin {
    ReconstructedCandidate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OwnerKind {
    Regex,
    PublicPrefilter,
}

/// A source-owned census entry, not an identity inferred from equal layouts.
/// Same-root clones belong to one entry; independent roots need distinct names.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExpectedOwner {
    pub name: String,
    pub kind: OwnerKind,
    pub pattern_config_digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PayloadRole {
    Value,
    RegexPayload,
    MetadataPayload,
    StrategyPayload,
    MatcherPayload,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PayloadRow {
    pub role: PayloadRole,
    pub size: usize,
    pub alignment: usize,
}

impl PayloadRow {
    pub(crate) fn layout(self) -> Result<Layout, ReportError> {
        Layout::from_size_align(self.size, self.alignment).map_err(|_error| ReportError::Layout)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OwnerRow {
    pub owner: ExpectedOwner,
    pub payloads: Vec<PayloadRow>,
}

impl OwnerRow {
    /// Pass the report of the actual candidate; no construction or query here.
    pub(crate) fn regex(
        name: &str,
        pattern_config_digest: &str,
        report: RegexResourceLayout,
    ) -> Self {
        Self {
            owner: ExpectedOwner {
                name: name.to_owned(),
                kind: OwnerKind::Regex,
                pattern_config_digest: pattern_config_digest.to_owned(),
            },
            payloads: [
                (PayloadRole::Value, report.value()),
                (PayloadRole::RegexPayload, report.regex_payload()),
                (PayloadRole::MetadataPayload, report.metadata_payload()),
                (PayloadRole::StrategyPayload, report.strategy_payload()),
            ]
            .into_iter()
            .map(|(role, layout)| PayloadRow {
                role,
                size: layout.size(),
                alignment: layout.align(),
            })
            .collect(),
        }
    }

    /// An accessible public prefilter, never a reconstructed hidden child.
    pub(crate) fn public_prefilter(
        name: &str,
        pattern_config_digest: &str,
        report: PrefilterResourceLayout,
    ) -> Self {
        Self {
            owner: ExpectedOwner {
                name: name.to_owned(),
                kind: OwnerKind::PublicPrefilter,
                pattern_config_digest: pattern_config_digest.to_owned(),
            },
            payloads: [
                (PayloadRole::Value, report.value()),
                (PayloadRole::MatcherPayload, report.matcher_payload()),
            ]
            .into_iter()
            .map(|(role, layout)| PayloadRow {
                role,
                size: layout.size(),
                alignment: layout.align(),
            })
            .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CandidateReport {
    pub identity: Identity,
    pub origin: CandidateOrigin,
    pub owners: Vec<OwnerRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReportError {
    Identity,
    Owners,
    Binding,
    Roles,
    Layout,
    Arithmetic,
}

/// Nonrecursive requests only: three Arcs per regex and one per public prefilter.
/// Excludes public values already embedded elsewhere, all children and caches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShallowCharges {
    pub regex_owners: usize,
    pub public_prefilter_owners: usize,
    pub regex_arc_bytes: usize,
    pub public_prefilter_arc_bytes: usize,
}

impl CandidateReport {
    /// Expected context/census must come from the caller, never from this report.
    /// Charge through `crate::schema_heap_layout::native_arc_value_layout`.
    pub(crate) fn validate(
        &self,
        expected_identity: &Identity,
        expected_owners: &[ExpectedOwner],
    ) -> Result<ShallowCharges, ReportError> {
        if &self.identity != expected_identity {
            return Err(ReportError::Identity);
        }
        if self.owners.len() != expected_owners.len() {
            return Err(ReportError::Owners);
        }
        let mut expected_names = BTreeSet::new();
        for owner in expected_owners {
            if !expected_names.insert(owner.name.as_str()) {
                return Err(ReportError::Owners);
            }
        }
        let mut actual_names = BTreeSet::new();
        for row in &self.owners {
            if !actual_names.insert(row.owner.name.as_str()) {
                return Err(ReportError::Owners);
            }
        }
        let mut charges = ShallowCharges {
            regex_owners: 0,
            public_prefilter_owners: 0,
            regex_arc_bytes: 0,
            public_prefilter_arc_bytes: 0,
        };
        for row in &self.owners {
            let expected = expected_owners
                .iter()
                .find(|owner| owner.name == row.owner.name)
                .ok_or(ReportError::Owners)?;
            if &row.owner != expected {
                return Err(ReportError::Binding);
            }
            let roles: &[PayloadRole] = match row.owner.kind {
                OwnerKind::Regex => &[
                    PayloadRole::Value,
                    PayloadRole::RegexPayload,
                    PayloadRole::MetadataPayload,
                    PayloadRole::StrategyPayload,
                ],
                OwnerKind::PublicPrefilter => &[PayloadRole::Value, PayloadRole::MatcherPayload],
            };
            if row.payloads.len() != roles.len()
                || roles.iter().any(|role| {
                    row.payloads
                        .iter()
                        .filter(|payload| payload.role == *role)
                        .count()
                        != 1
                })
            {
                return Err(ReportError::Roles);
            }
            let mut bytes = 0_usize;
            for payload in &row.payloads {
                let layout = payload.layout()?;
                if payload.role != PayloadRole::Value {
                    let allocation = native_arc_value_layout(layout)
                        .map_err(|_error| ReportError::Arithmetic)?;
                    bytes = bytes
                        .checked_add(allocation.size())
                        .ok_or(ReportError::Arithmetic)?;
                }
            }
            let (owners, total) = match row.owner.kind {
                OwnerKind::Regex => (&mut charges.regex_owners, &mut charges.regex_arc_bytes),
                OwnerKind::PublicPrefilter => (
                    &mut charges.public_prefilter_owners,
                    &mut charges.public_prefilter_arc_bytes,
                ),
            };
            *owners = owners.checked_add(1).ok_or(ReportError::Arithmetic)?;
            *total = total.checked_add(bytes).ok_or(ReportError::Arithmetic)?;
        }
        Ok(charges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn fake_identity() -> Identity {
        Identity {
            engine_version: "fake_engine_version".to_owned(),
            archive_sha256: "fake_archive".to_owned(),
            patch_sha256: "fake_patch".to_owned(),
            tree_sha256: "fake_tree".to_owned(),
            compiler: "fake_compiler".to_owned(),
            target: "fake_native_target".to_owned(),
            features: vec!["fake_alloc".to_owned(), "fake_meta".to_owned()],
        }
    }

    fn fake_expected_owners() -> Vec<ExpectedOwner> {
        vec![
            ExpectedOwner {
                name: "fake_regex".to_owned(),
                kind: OwnerKind::Regex,
                pattern_config_digest: "fake_pattern_config".to_owned(),
            },
            ExpectedOwner {
                name: "fake_public_prefilter".to_owned(),
                kind: OwnerKind::PublicPrefilter,
                pattern_config_digest: "fake_needles_config".to_owned(),
            },
        ]
    }

    fn fake_json() -> Value {
        json!({
            "identity": {
                "engine_version": "fake_engine_version",
                "archive_sha256": "fake_archive",
                "patch_sha256": "fake_patch",
                "tree_sha256": "fake_tree",
                "compiler": "fake_compiler",
                "target": "fake_native_target",
                "features": ["fake_alloc", "fake_meta"]
            },
            "origin": "reconstructed_candidate",
            "owners": [
                {
                    "owner": {
                        "name": "fake_regex", "kind": "regex",
                        "pattern_config_digest": "fake_pattern_config"
                    },
                    "payloads": [
                        {"role": "value", "size": 4096, "alignment": 8},
                        {"role": "regex_payload", "size": 8, "alignment": 8},
                        {"role": "metadata_payload", "size": 16, "alignment": 8},
                        {"role": "strategy_payload", "size": 24, "alignment": 8}
                    ]
                },
                {
                    "owner": {
                        "name": "fake_public_prefilter", "kind": "public_prefilter",
                        "pattern_config_digest": "fake_needles_config"
                    },
                    "payloads": [
                        {"role": "value", "size": 2048, "alignment": 8},
                        {"role": "matcher_payload", "size": 5, "alignment": 1}
                    ]
                }
            ]
        })
    }

    fn fake_report() -> CandidateReport {
        serde_json::from_value(fake_json()).unwrap()
    }

    fn fake_validate(report: &CandidateReport) -> Result<ShallowCharges, ReportError> {
        report.validate(&fake_identity(), &fake_expected_owners())
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_literal_rows_have_three_regex_arcs_and_one_public_matcher_arc() {
        assert_eq!(
            fake_validate(&fake_report()).unwrap(),
            ShallowCharges {
                regex_owners: 1,
                public_prefilter_owners: 1,
                regex_arc_bytes: 96,
                public_prefilter_arc_bytes: 24,
            }
        );
    }

    #[test]
    fn fake_each_identity_key_is_checked_against_an_independent_expected_context() {
        for key in [
            "engine_version",
            "archive_sha256",
            "patch_sha256",
            "tree_sha256",
            "compiler",
            "target",
            "features",
        ] {
            let changed = if key == "features" {
                json!(["fake_other_feature"])
            } else {
                json!("fake_other_identity")
            };
            let mut external_context = serde_json::to_value(fake_identity()).unwrap();
            external_context[key] = changed.clone();
            let expected: Identity = serde_json::from_value(external_context).unwrap();
            let mut value = fake_json();
            value["identity"][key] = changed;
            let report: CandidateReport = serde_json::from_value(value).unwrap();
            assert_eq!(fake_validate(&report), Err(ReportError::Identity), "{key}");
            assert!(
                report.validate(&expected, &fake_expected_owners()).is_ok(),
                "{key}"
            );
        }
    }

    #[test]
    fn fake_feature_additions_and_omissions_do_not_match_the_expected_closure() {
        for features in [
            json!(["fake_alloc"]),
            json!(["fake_alloc", "fake_meta", "fake_extra"]),
            json!(["fake_alloc", "fake_meta", "fake_meta"]),
        ] {
            let mut value = fake_json();
            value["identity"]["features"] = features;
            let report: CandidateReport = serde_json::from_value(value).unwrap();
            assert_eq!(fake_validate(&report), Err(ReportError::Identity));
        }
    }

    #[test]
    fn fake_missing_unknown_and_duplicate_owner_names_refuse() {
        for index in 0..2 {
            let mut missing = fake_report();
            missing.owners.remove(index);
            assert_eq!(fake_validate(&missing), Err(ReportError::Owners));
            let mut unknown = fake_report();
            unknown.owners[index].owner.name = "fake_unknown".to_owned();
            assert_eq!(fake_validate(&unknown), Err(ReportError::Owners));
            let mut duplicate = fake_report();
            duplicate.owners.push(duplicate.owners[index].clone());
            assert_eq!(fake_validate(&duplicate), Err(ReportError::Owners));
        }
        let mut extra = fake_report();
        let mut row = extra.owners[0].clone();
        row.owner.name = "fake_extra".to_owned();
        extra.owners.push(row);
        assert_eq!(fake_validate(&extra), Err(ReportError::Owners));
    }

    #[test]
    fn fake_duplicate_names_cannot_be_disambiguated_by_kind_or_digest() {
        let mut report = fake_report();
        report.owners[1].owner.name = "fake_regex".to_owned();
        assert_eq!(fake_validate(&report), Err(ReportError::Owners));
        let mut expected = fake_expected_owners();
        expected[1].name = "fake_regex".to_owned();
        assert_eq!(
            report.validate(&fake_identity(), &expected),
            Err(ReportError::Owners)
        );
        assert_eq!(
            fake_report().validate(&fake_identity(), &expected),
            Err(ReportError::Owners)
        );
    }

    #[test]
    fn fake_owner_configuration_and_kind_bindings_are_checked_independently() {
        for index in 0..2 {
            let mut report = fake_report();
            report.owners[index].owner.pattern_config_digest = "fake_other_digest".to_owned();
            assert_eq!(fake_validate(&report), Err(ReportError::Binding));
            let mut expected = fake_expected_owners();
            expected[index].kind = match expected[index].kind {
                OwnerKind::Regex => OwnerKind::PublicPrefilter,
                OwnerKind::PublicPrefilter => OwnerKind::Regex,
            };
            assert_eq!(
                fake_report().validate(&fake_identity(), &expected),
                Err(ReportError::Binding)
            );
        }
    }

    #[test]
    fn fake_each_required_role_must_occur_exactly_once() {
        for owner in 0..2 {
            for role in 0..fake_report().owners[owner].payloads.len() {
                let mut missing = fake_report();
                missing.owners[owner].payloads.remove(role);
                assert_eq!(fake_validate(&missing), Err(ReportError::Roles));
                let mut duplicate = fake_report();
                let payload = duplicate.owners[owner].payloads[role];
                duplicate.owners[owner].payloads.push(payload);
                assert_eq!(fake_validate(&duplicate), Err(ReportError::Roles));
            }
        }
    }

    #[test]
    fn fake_known_roles_from_another_owner_kind_are_not_extra_payloads() {
        for (index, role) in [
            (0, PayloadRole::MatcherPayload),
            (1, PayloadRole::RegexPayload),
        ] {
            let mut report = fake_report();
            report.owners[index].payloads.push(PayloadRow {
                role,
                size: 8,
                alignment: 8,
            });
            assert_eq!(fake_validate(&report), Err(ReportError::Roles));
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_owner_and_role_order_do_not_define_identity_or_ownership() {
        let mut report = fake_report();
        report.owners.reverse();
        for row in &mut report.owners {
            row.payloads.reverse();
        }
        let charges = fake_validate(&report).unwrap();
        assert_eq!(charges.regex_arc_bytes, 96);
        assert_eq!(charges.public_prefilter_arc_bytes, 24);
    }

    #[test]
    fn fake_unknown_roles_kinds_and_singleton_origins_cannot_deserialize() {
        for (pointer, value) in [
            ("/owners/0/payloads/0/role", json!("fake_unknown_role")),
            ("/owners/1/payloads/0/role", json!("fake_unknown_role")),
            ("/owners/0/owner/kind", json!("hidden_prefilter")),
            ("/origin", json!("generated_singleton")),
        ] {
            let mut report = fake_json();
            *report.pointer_mut(pointer).unwrap() = value;
            assert!(
                serde_json::from_value::<CandidateReport>(report).is_err(),
                "{pointer}"
            );
        }
    }

    #[test]
    fn fake_unknown_fields_refuse_at_every_structured_boundary() {
        for pointer in [
            "",
            "/identity",
            "/owners/0",
            "/owners/0/owner",
            "/owners/0/payloads/0",
        ] {
            let mut report = fake_json();
            report
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert("fake_extra".to_owned(), json!(1_u8));
            assert!(
                serde_json::from_value::<CandidateReport>(report).is_err(),
                "{pointer}"
            );
        }
    }

    #[test]
    fn fake_all_context_and_row_fields_are_required() {
        for key in [
            "engine_version",
            "archive_sha256",
            "patch_sha256",
            "tree_sha256",
            "compiler",
            "target",
            "features",
        ] {
            let mut report = fake_json();
            report["identity"].as_object_mut().unwrap().remove(key);
            assert!(
                serde_json::from_value::<CandidateReport>(report).is_err(),
                "{key}"
            );
        }
        for (pointer, key) in [
            ("", "identity"),
            ("", "origin"),
            ("", "owners"),
            ("/owners/0", "owner"),
            ("/owners/0", "payloads"),
            ("/owners/0/owner", "name"),
            ("/owners/0/owner", "kind"),
            ("/owners/0/owner", "pattern_config_digest"),
            ("/owners/0/payloads/0", "role"),
            ("/owners/0/payloads/0", "size"),
            ("/owners/0/payloads/0", "alignment"),
        ] {
            let mut report = fake_json();
            report
                .pointer_mut(pointer)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(
                serde_json::from_value::<CandidateReport>(report).is_err(),
                "{pointer}/{key}"
            );
        }
    }

    #[test]
    fn fake_layout_records_use_rust_request_validation_not_private_field_rules() {
        for (size, alignment) in [(0, 1), (0, 64), (3, 8), (24, 32)] {
            let row = PayloadRow {
                role: PayloadRole::StrategyPayload,
                size,
                alignment,
            };
            assert_eq!(
                row.layout().unwrap(),
                Layout::from_size_align(size, alignment).unwrap()
            );
        }
        for (size, alignment) in [
            (1, 0),
            (1, 3),
            (usize::MAX, 1),
            (usize::try_from(isize::MAX).unwrap(), 8),
        ] {
            let row = PayloadRow {
                role: PayloadRole::StrategyPayload,
                size,
                alignment,
            };
            assert_eq!(row.layout(), Err(ReportError::Layout), "{size}/{alignment}");
        }
    }

    #[test]
    fn fake_every_role_including_uncharged_values_requires_a_valid_layout() {
        for owner in 0..2 {
            for role in 0..fake_report().owners[owner].payloads.len() {
                let mut report = fake_report();
                report.owners[owner].payloads[role].alignment = 3;
                assert_eq!(fake_validate(&report), Err(ReportError::Layout));
            }
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_size_changes_have_independent_literal_shallow_totals() {
        for (role, size, total) in [(1, 16, 104), (2, 32, 112), (3, 64, 136)] {
            let mut report = fake_report();
            report.owners[0].payloads[role].size = size;
            let charges = fake_validate(&report).unwrap();
            assert_eq!(charges.regex_arc_bytes, total);
            assert_eq!(charges.public_prefilter_arc_bytes, 24);
        }
        let mut report = fake_report();
        report.owners[1].payloads[1].size = 17;
        let charges = fake_validate(&report).unwrap();
        assert_eq!(charges.regex_arc_bytes, 96);
        assert_eq!(charges.public_prefilter_arc_bytes, 40);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_alignment_alone_changes_header_gaps_and_tail_padding() {
        for (alignment, total) in [(8, 96), (16, 104), (32, 136), (64, 200)] {
            let mut report = fake_report();
            report.owners[0].payloads[1].alignment = alignment;
            assert_eq!(fake_validate(&report).unwrap().regex_arc_bytes, total);
        }
        let mut report = fake_report();
        report.owners[1].payloads[1].alignment = 32;
        assert_eq!(
            fake_validate(&report).unwrap().public_prefilter_arc_bytes,
            64
        );
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_embedded_public_values_are_validated_but_never_charged_again() {
        let mut report = fake_report();
        for row in &mut report.owners {
            row.payloads[0].size = 8192;
            row.payloads[0].alignment = 256;
        }
        let charges = fake_validate(&report).unwrap();
        assert_eq!(charges.regex_arc_bytes, 96);
        assert_eq!(charges.public_prefilter_arc_bytes, 24);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_distinct_roots_with_equal_patterns_and_reports_are_charged_separately() {
        for (count, total) in [(0, 0), (1, 96), (2, 192), (3, 288)] {
            let mut expected = fake_expected_owners();
            let source_owner = expected.remove(0);
            for index in 0..count {
                let mut owner = source_owner.clone();
                owner.name = format!("fake_independent_root_{index}");
                expected.push(owner);
            }
            let mut report = fake_report();
            let template = report.owners.remove(0);
            for index in 0..count {
                let mut row = template.clone();
                row.owner.name = format!("fake_independent_root_{index}");
                report.owners.push(row);
            }
            let charges = report.validate(&fake_identity(), &expected).unwrap();
            assert_eq!(charges.regex_owners, count);
            assert_eq!(charges.regex_arc_bytes, total);
            assert_eq!(charges.public_prefilter_owners, 1);
            assert_eq!(charges.public_prefilter_arc_bytes, 24);
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_public_prefilter_multiplicity_is_not_hidden_child_or_layout_deduplication() {
        let mut expected = fake_expected_owners();
        let mut owner = expected[1].clone();
        owner.name = "fake_second_public_prefilter".to_owned();
        expected.push(owner);
        let mut report = fake_report();
        let mut second = report.owners[1].clone();
        second.owner.name = "fake_second_public_prefilter".to_owned();
        report.owners.push(second);
        let charges = report.validate(&fake_identity(), &expected).unwrap();
        assert_eq!(charges.regex_owners, 1);
        assert_eq!(charges.regex_arc_bytes, 96);
        assert_eq!(charges.public_prefilter_owners, 2);
        assert_eq!(charges.public_prefilter_arc_bytes, 48);
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn fake_zst_payloads_still_fund_headers_but_absent_owners_refuse() {
        let mut report = fake_report();
        for row in &mut report.owners {
            for payload in &mut row.payloads {
                payload.size = 0;
                payload.alignment = 1;
            }
        }
        let charges = fake_validate(&report).unwrap();
        assert_eq!(
            charges.regex_arc_bytes, 48,
            "three live 16-byte Arc headers"
        );
        assert_eq!(charges.public_prefilter_arc_bytes, 16);
        report.owners.remove(0);
        assert_eq!(fake_validate(&report), Err(ReportError::Owners));
    }

    #[test]
    fn fake_arc_header_and_alignment_overflow_refuse_valid_payload_layouts() {
        for (size, alignment) in [
            (usize::try_from(isize::MAX).unwrap(), 1),
            (0, 1_usize << (usize::BITS - 1)),
        ] {
            Layout::from_size_align(size, alignment).unwrap();
            for (owner, role) in [(0, 1), (0, 2), (0, 3), (1, 1)] {
                let mut report = fake_report();
                report.owners[owner].payloads[role].size = size;
                report.owners[owner].payloads[role].alignment = alignment;
                assert_eq!(fake_validate(&report), Err(ReportError::Arithmetic));
            }
        }
    }

    #[test]
    fn fake_aggregation_checks_usize_sums_not_one_isize_allocation_request() {
        for (kind, count) in [(OwnerKind::Regex, 2_usize), (OwnerKind::PublicPrefilter, 4)] {
            let mut expected = fake_expected_owners();
            expected.retain(|owner| owner.kind == kind);
            let mut report = fake_report();
            report.owners.retain(|row| row.owner.kind == kind);
            let size = usize::try_from(isize::MAX).unwrap().checked_div(2).unwrap() & !7;
            for payload in &mut report.owners[0].payloads[1..] {
                payload.size = size;
                payload.alignment = 8;
            }
            report.validate(&fake_identity(), &expected).unwrap();
            let source_owner = expected[0].clone();
            for index in 1..count {
                let mut owner = source_owner.clone();
                owner.name = format!("fake_large_owner_{index}");
                expected.push(owner);
            }
            let template = report.owners[0].clone();
            for index in 1..count {
                let mut row = template.clone();
                row.owner.name = format!("fake_large_owner_{index}");
                report.owners.push(row);
            }
            assert_eq!(
                report.validate(&fake_identity(), &expected),
                Err(ReportError::Arithmetic)
            );
        }
    }

    #[test]
    fn fake_library_reports_are_copy_and_rows_bind_each_actual_typed_getter() {
        use regex_automata::{MatchKind, meta::Regex, util::prefilter::Prefilter};
        use std::fmt::Debug;

        fn copy_report<T: Copy + Clone + Debug + Eq + Send + Sync + Unpin + 'static>(
            value: T,
        ) -> T {
            let copy = value;
            assert_eq!(value, copy);
            copy
        }

        let pattern = "fake_[a-z]+";
        let regex = Regex::new(pattern).unwrap();
        let report = copy_report(regex.resource_layout());
        drop(regex);
        let row = OwnerRow::regex("fake_regex", "fake_pattern_config", report);
        assert_eq!(row.owner, fake_expected_owners().remove(0));
        assert_eq!(row.payloads.len(), 4);
        for (payload, (role, layout)) in row.payloads.iter().zip([
            (PayloadRole::Value, report.value()),
            (PayloadRole::RegexPayload, report.regex_payload()),
            (PayloadRole::MetadataPayload, report.metadata_payload()),
            (PayloadRole::StrategyPayload, report.strategy_payload()),
        ]) {
            assert_eq!(payload.role, role);
            assert_eq!(
                (payload.size, payload.alignment),
                (layout.size(), layout.align())
            );
        }
        assert!(!serde_json::to_string(&row).unwrap().contains(pattern));

        let prefilter = Prefilter::new(MatchKind::LeftmostFirst, &["fake_literal"]).unwrap();
        let prefilter_report = copy_report(prefilter.resource_layout());
        drop(prefilter);
        let prefilter_row = OwnerRow::public_prefilter(
            "fake_public_prefilter",
            "fake_needles_config",
            prefilter_report,
        );
        assert_eq!(prefilter_row.owner, fake_expected_owners().remove(1));
        assert_eq!(prefilter_row.payloads.len(), 2);
        for (payload, (role, layout)) in prefilter_row.payloads.iter().zip([
            (PayloadRole::Value, prefilter_report.value()),
            (
                PayloadRole::MatcherPayload,
                prefilter_report.matcher_payload(),
            ),
        ]) {
            assert_eq!(payload.role, role);
            assert_eq!(
                (payload.size, payload.alignment),
                (layout.size(), layout.align())
            );
        }
        assert!(
            !serde_json::to_string(&prefilter_row)
                .unwrap()
                .contains("fake_literal")
        );
    }
}

//! Build-only plans for unqualified, non-serving schema artifacts.
//!
//! The build adapter supplies discovery-validated declarations and the pinned
//! published/planning-data projection. Runtime activation is not an input.
//! These interfaces deliberately contain neither callable validators nor costs.

use crate::schema_staging_codegen::CANDIDATE_OPTIONS;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Schema<'a> {
    Inherit,
    Owners(&'a [&'a str]),
    LocalSchemaFree,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Declaration<'a> {
    pub name: &'a str,
    pub parent: Option<&'a str>,
    pub implementation: &'a str,
    pub required_feature: Option<&'a str>,
    pub schema: Schema<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Member<'a> {
    pub name: &'a str,
    pub implementation: &'a str,
    pub is_root: bool,
    /// Canonical names, nearest parent first; overrides do not hide ancestry.
    pub ancestry: Vec<&'a str>,
    pub schema_owner: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Selection<'a> {
    /// Case-insensitive public-name order, including ancestry-only members.
    pub members: Vec<Member<'a>>,
    /// Exact, sorted, distinct effective owners of lane-active roots only.
    pub schema_owners: Vec<&'a str>,
}

fn declaration_index<'a, 'd>(
    declarations: &'d [Declaration<'a>],
) -> Result<BTreeMap<String, &'d Declaration<'a>>, String> {
    let mut ordered: Vec<_> = declarations.iter().collect();
    ordered.sort_by_key(|row| row.name.to_ascii_lowercase());
    let mut index = BTreeMap::new();
    for row in ordered {
        let name = row.name.to_ascii_lowercase();
        if index.insert(name.clone(), row).is_some() {
            return Err(format!("duplicate public name: {name}"));
        }
    }
    let mut factory_features = BTreeMap::new();
    for row in index.values() {
        if row.name.trim().is_empty() {
            return Err("empty public name".into());
        }
        if row.implementation.trim().is_empty() {
            return Err(format!("empty implementation: {}", row.name));
        }
        if row
            .required_feature
            .is_some_and(|feature| feature.trim().is_empty())
        {
            return Err(format!("empty feature: {}", row.name));
        }
        match row.schema {
            Schema::Owners([]) => return Err(format!("empty schema owner: {}", row.name)),
            Schema::Owners([owner]) if owner.trim().is_empty() => {
                return Err(format!("empty schema owner: {}", row.name));
            }
            Schema::Owners([_]) | Schema::Inherit => {}
            Schema::Owners(_) => return Err(format!("ambiguous schema owner: {}", row.name)),
            Schema::LocalSchemaFree if row.parent.is_some() => {
                return Err(format!("schema-free alias: {}", row.name));
            }
            Schema::LocalSchemaFree => {}
        }
        if let Some(feature) = factory_features.insert(row.implementation, row.required_feature)
            && feature != row.required_feature
        {
            return Err(format!("factory feature mismatch: {}", row.name));
        }
        if let Some(parent_name) = row.parent {
            let parent = index
                .get(&parent_name.to_ascii_lowercase())
                .ok_or_else(|| format!("missing parent: {} -> {parent_name}", row.name))?;
            if parent.implementation != row.implementation {
                return Err(format!("alias implementation mismatch: {}", row.name));
            }
            if parent.required_feature != row.required_feature {
                return Err(format!("alias feature mismatch: {}", row.name));
            }
        }
    }

    Ok(index)
}

/// Derive S from publication/planning roots, after validating ALL declarations.
/// Public names are ASCII case-insensitive; all owner identities are exact.
///
/// # Errors
/// Refuses unknown roots, invalid identities/ownership, broken/cyclic ancestry,
/// absent/ambiguous schemas, or mismatched factory/alias feature predicates.
pub fn select<'a>(
    declarations: &[Declaration<'a>],
    published: &[&str],
    planned: &[&str],
    enabled_features: &[&str],
) -> Result<Selection<'a>, String> {
    let index = declaration_index(declarations)?;
    // Resolve every declaration, including gated/unselected rows, before filtering.
    let mut resolved = BTreeMap::new();
    for (key, &row) in &index {
        let mut current = row;
        let mut visited = BTreeSet::new();
        let mut ancestry = Vec::new();
        let mut schema_owner = None;
        loop {
            if !visited.insert(current.name) {
                return Err(format!("alias cycle: {}", row.name));
            }
            if current.name != row.name {
                ancestry.push(current.name);
            }
            if schema_owner.is_none()
                && let Schema::Owners([owner]) = current.schema
            {
                schema_owner = Some(*owner);
            }
            let Some(parent) = current.parent else { break };
            current = index
                .get(&parent.to_ascii_lowercase())
                .ok_or_else(|| format!("missing parent: {} -> {parent}", current.name))?;
        }
        if schema_owner.is_none() && row.schema != Schema::LocalSchemaFree {
            return Err(format!("missing schema owner: {}", row.name));
        }
        resolved.insert(
            key.clone(),
            Member {
                name: row.name,
                implementation: row.implementation,
                is_root: false,
                ancestry,
                schema_owner,
            },
        );
    }
    let roots: BTreeSet<_> = published
        .iter()
        .chain(planned)
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut active = BTreeSet::new();
    let mut included = BTreeSet::new();
    let mut owners = BTreeSet::new();
    for root in roots {
        let row = index
            .get(&root)
            .ok_or_else(|| format!("unknown root: {root}"))?;
        if row
            .required_feature
            .is_some_and(|feature| !enabled_features.contains(&feature))
        {
            continue;
        }
        let member = resolved
            .get(&root)
            .ok_or_else(|| format!("unknown root: {root}"))?;
        included.extend(member.ancestry.iter().map(|name| name.to_ascii_lowercase()));
        included.insert(root.clone());
        active.insert(root);
        owners.extend(member.schema_owner);
    }
    Ok(Selection {
        members: resolved
            .into_iter()
            .filter_map(|(key, mut member)| {
                if !included.contains(&key) {
                    return None;
                }
                member.is_root = active.contains(&key);
                Some(member)
            })
            .collect(),
        schema_owners: owners.into_iter().collect(),
    })
}

/// There is intentionally no qualified or numeric-cost state in this slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum Qualification {
    Unqualified,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum InputKind {
    SourceInventory,
    SourceBindings,
    PriorityWave,
    SupplementalWave,
    SupplementalInventory,
    WorkspaceManifest,
    Lockfile,
    GeneratorSource,
    FormatHelperSource,
    Schema,
}

#[derive(Clone, Copy, Debug)]
pub struct Input<'a> {
    pub kind: InputKind,
    /// Stable source-input ID; for Schema, the exact effective owner identity.
    pub id: &'a str,
    pub bytes: &'a [u8],
    pub expected_sha256: &'a str,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct BuildContext<'a> {
    pub target: &'a str,
    pub profile: &'a str,
    pub enabled_features: &'a [&'a str],
    pub compiler_verbose: &'a str,
    pub encoded_rustflags: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InputIdentity {
    pub kind: InputKind,
    pub id: String,
    pub sha256: String,
}

/// An input-bound inspection identity, NOT final linked-artifact qualification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactIdentity {
    pub qualification: Qualification,
    pub sha256: String,
    pub inputs: Vec<InputIdentity>,
}

/// Bind membership, build context, fixed generator options and witnessed inputs.
/// All non-Schema kinds are mandatory; multiple generator source IDs are allowed.
/// Schema inputs must equal S exactly. Input order has no semantic meaning.
///
/// # Errors
/// Refuses missing/duplicate/empty IDs, mismatched digests, incomplete context,
/// missing mandatory inputs, and missing/extra schema owners.
pub fn identify(
    selection: &Selection<'_>,
    context: BuildContext<'_>,
    inputs: &[Input<'_>],
) -> Result<ArtifactIdentity, String> {
    if [context.target, context.profile, context.compiler_verbose]
        .iter()
        .any(|value| value.trim().is_empty())
        || context
            .enabled_features
            .iter()
            .any(|feature| feature.trim().is_empty())
    {
        return Err("incomplete staging build context".into());
    }
    let mut witnessed = BTreeMap::new();
    for input in inputs {
        if input.id.trim().is_empty() {
            return Err("empty staging input identity".into());
        }
        let sha256 = digest(input.bytes);
        if sha256 != input.expected_sha256 {
            return Err(format!("staging input digest mismatch: {}", input.id));
        }
        if witnessed
            .insert(
                (input.kind, input.id),
                InputIdentity {
                    kind: input.kind,
                    id: input.id.to_owned(),
                    sha256,
                },
            )
            .is_some()
        {
            return Err(format!("duplicate staging input identity: {}", input.id));
        }
    }
    for kind in [
        InputKind::SourceInventory,
        InputKind::SourceBindings,
        InputKind::PriorityWave,
        InputKind::SupplementalWave,
        InputKind::SupplementalInventory,
        InputKind::WorkspaceManifest,
        InputKind::Lockfile,
        InputKind::GeneratorSource,
        InputKind::FormatHelperSource,
    ] {
        if !witnessed.keys().any(|(found, _)| *found == kind) {
            return Err(format!("missing staging input kind: {kind:?}"));
        }
    }
    let schemas: BTreeSet<_> = witnessed
        .keys()
        .filter_map(|(kind, id)| (*kind == InputKind::Schema).then_some(*id))
        .collect();
    let expected: BTreeSet<_> = selection.schema_owners.iter().copied().collect();
    if schemas != expected || expected.len() != selection.schema_owners.len() {
        return Err("staging schema input membership differs from S".into());
    }
    let input_identities: Vec<_> = witnessed.into_values().collect();
    let features: BTreeSet<_> = context.enabled_features.iter().copied().collect();
    let mut hash = Sha256::new();
    for field in [
        "auctioneer/non-serving-schema-staging/v1",
        CANDIDATE_OPTIONS,
        context.target,
        context.profile,
        context.compiler_verbose,
        context.encoded_rustflags,
    ] {
        hash_field(&mut hash, field)?;
    }
    hash_field(
        &mut hash,
        &toml::to_string(&BTreeMap::from([("features", features)]))
            .map_err(|error| error.to_string())?,
    )?;
    hash_field(
        &mut hash,
        &toml::to_string(selection).map_err(|error| error.to_string())?,
    )?;
    for input in &input_identities {
        hash_field(
            &mut hash,
            &toml::to_string(input).map_err(|error| error.to_string())?,
        )?;
    }
    Ok(ArtifactIdentity {
        qualification: Qualification::Unqualified,
        sha256: hex::encode(hash.finalize()),
        inputs: input_identities,
    })
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn hash_field(hash: &mut Sha256, field: &str) -> Result<(), String> {
    let length = u64::try_from(field.len()).map_err(|error| error.to_string())?;
    // Preserve the v1 eight-byte little-endian length prefix.
    for shift in (0_u32..u64::BITS).step_by(8) {
        hash.update([u8::try_from((length >> shift) & 0xff).map_err(|error| error.to_string())?]);
    }
    hash.update(field.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactIdentity, BuildContext, Declaration, Input, InputKind, Member, Qualification,
        Schema, Selection, identify, select,
    };
    use sha2::{Digest as _, Sha256};

    fn base<'a>(name: &'a str, owner: &'a [&'a str]) -> Declaration<'a> {
        Declaration {
            name,
            parent: None,
            implementation: "fake_factory",
            required_feature: None,
            schema: Schema::Owners(owner),
        }
    }

    fn alias<'a>(name: &'a str, parent: &'a str) -> Declaration<'a> {
        Declaration {
            parent: Some(parent),
            schema: Schema::Inherit,
            ..base(name, &[])
        }
    }

    #[test]
    fn fake_s_is_published_union_planned_with_nearest_override_and_ancestry() {
        let declarations = [
            base("fake_base", &["fake_original"]),
            Declaration {
                schema: Schema::Owners(&["fake_override"]),
                ..alias("fake_mid", "fake_base")
            },
            alias("fake_leaf", "FAKE_MID"),
            base("fake_published", &["fake_published_schema"]),
            base("fake_source_only", &["fake_not_shipped"]),
        ];
        let selected = select(&declarations, &["fake_published"], &["fake_leaf"], &[])
            .expect("published plus frozen planned roots");
        assert_eq!(
            selected.schema_owners,
            ["fake_override", "fake_published_schema"]
        );
        assert_eq!(
            selected
                .members
                .iter()
                .map(|row| row.name)
                .collect::<Vec<_>>(),
            ["fake_base", "fake_leaf", "fake_mid", "fake_published"]
        );
        let leaf = &selected.members[1];
        assert!(leaf.is_root);
        assert_eq!(leaf.ancestry, ["fake_mid", "fake_base"]);
        assert_eq!(leaf.schema_owner, Some("fake_override"));
        assert!(!selected.members[0].is_root);
        assert!(!selected.members[2].is_root);
        assert_eq!(
            select(
                &declarations.iter().rev().copied().collect::<Vec<_>>(),
                &["FAKE_PUBLISHED", "fake_published"],
                &["fake_leaf"],
                &[]
            )
            .expect("stable order"),
            selected,
        );
    }

    #[test]
    fn fake_s_deduplicates_exact_owner_not_factory_content_or_name_case() {
        let declarations = [
            base("fake_a", &["fake_shared"]),
            base("fake_b", &["fake_shared"]),
            base("fake_c", &["FAKE_shared"]),
        ];
        let selected = select(&declarations, &["fake_a", "fake_b", "fake_c"], &[], &[])
            .expect("distinct public names sharing one factory");
        assert_eq!(selected.schema_owners, ["FAKE_shared", "fake_shared"]);
        assert_eq!(selected.members.len(), 3);
    }

    #[test]
    fn fake_s_feature_lanes_do_not_take_runtime_activation() {
        let declarations = [
            base("fake_secure", &["fake_secure_schema"]),
            Declaration {
                implementation: "fake_local_factory",
                required_feature: Some("fake_optional"),
                schema: Schema::LocalSchemaFree,
                ..base("fake_local", &[])
            },
            Declaration {
                implementation: "fake_optional_factory",
                required_feature: Some("fake_optional"),
                ..base("fake_optional", &["fake_optional_schema"])
            },
        ];
        let published = ["fake_secure", "fake_local", "fake_optional"];
        let off = select(&declarations, &published, &[], &[]).expect("secure");
        assert_eq!(off.schema_owners, ["fake_secure_schema"]);
        let on = select(&declarations, &published, &[], &["fake_optional"]).expect("development");
        assert_eq!(
            on.schema_owners,
            ["fake_optional_schema", "fake_secure_schema"]
        );
        assert_eq!(on.members[0].schema_owner, None);
        assert_eq!(
            select(&declarations, &published, &[], &[]).expect("off again"),
            off
        );
    }

    #[test]
    fn fake_s_rejects_bad_unselected_gated_declarations_before_filtering() {
        let gated = Declaration {
            implementation: "fake_gated_factory",
            required_feature: Some("fake_optional"),
            ..base("fake_gated", &["fake_gated_schema"])
        };
        let invalid = [
            Declaration {
                parent: Some("fake_missing"),
                ..gated
            },
            Declaration {
                parent: Some("FAKE_GATED"),
                ..gated
            },
            Declaration {
                schema: Schema::Owners(&[]),
                ..gated
            },
            Declaration {
                schema: Schema::Owners(&["fake_one", "fake_two"]),
                ..gated
            },
            Declaration {
                schema: Schema::Inherit,
                ..gated
            },
            Declaration { name: "", ..gated },
            Declaration {
                implementation: "",
                ..gated
            },
            Declaration {
                required_feature: Some(""),
                ..gated
            },
        ];
        let reasons = [
            "missing parent: fake_gated -> fake_missing",
            "alias cycle: fake_gated",
            "empty schema owner: fake_gated",
            "ambiguous schema owner: fake_gated",
            "missing schema owner: fake_gated",
            "empty public name",
            "empty implementation: fake_gated",
            "empty feature: fake_gated",
        ];
        for (bad, reason) in invalid.into_iter().zip(reasons) {
            let declarations = [base("fake_secure", &["fake_schema"]), bad];
            for features in [&[][..], &["fake_optional"][..]] {
                assert_eq!(
                    select(&declarations, &["fake_secure"], &[], features)
                        .expect_err("invalid unselected declaration"),
                    reason,
                    "{bad:?}"
                );
            }
        }
    }

    #[test]
    fn fake_s_refuses_ambiguous_names_factory_switches_and_unknown_roots() {
        let cases = [
            vec![base("fake_a", &["fake_one"]), base("FAKE_A", &["fake_two"])],
            vec![
                base("fake_a", &["fake_one"]),
                Declaration {
                    implementation: "fake_other",
                    ..alias("fake_b", "fake_a")
                },
            ],
            vec![
                base("fake_a", &["fake_one"]),
                Declaration {
                    required_feature: Some("fake_optional"),
                    ..base("fake_b", &["fake_two"])
                },
            ],
            vec![
                base("fake_a", &["fake_one"]),
                Declaration {
                    schema: Schema::LocalSchemaFree,
                    ..alias("fake_b", "fake_a")
                },
            ],
        ];
        let reasons = [
            "duplicate public name: fake_a",
            "alias implementation mismatch: fake_b",
            "factory feature mismatch: fake_b",
            "schema-free alias: fake_b",
        ];
        for (declarations, reason) in cases.into_iter().zip(reasons) {
            assert_eq!(
                select(&declarations, &[], &[], &[]).expect_err("invalid declarations"),
                reason
            );
        }
        assert_eq!(
            select(
                &[base("fake_a", &["fake_schema"])],
                &[],
                &["fake_unknown"],
                &[]
            )
            .expect_err("unknown planned root"),
            "unknown root: fake_unknown"
        );
    }

    fn fake_selection() -> Selection<'static> {
        Selection {
            members: vec![Member {
                name: "fake_root",
                implementation: "fake_factory",
                is_root: true,
                ancestry: vec![],
                schema_owner: Some("fake_schema"),
            }],
            schema_owners: vec!["fake_schema"],
        }
    }

    fn context() -> BuildContext<'static> {
        BuildContext {
            target: "fake-target",
            profile: "fake-profile",
            enabled_features: &[],
            compiler_verbose: "fake rustc -vV",
            encoded_rustflags: "fake-flags",
        }
    }

    fn inputs(sha256: &str) -> Vec<Input<'_>> {
        use InputKind::{
            FormatHelperSource, GeneratorSource, Lockfile, PriorityWave, Schema, SourceBindings,
            SourceInventory, SupplementalInventory, SupplementalWave, WorkspaceManifest,
        };
        [
            SourceInventory,
            SourceBindings,
            PriorityWave,
            SupplementalWave,
            SupplementalInventory,
            WorkspaceManifest,
            Lockfile,
            GeneratorSource,
            FormatHelperSource,
            Schema,
        ]
        .map(|kind| Input {
            kind,
            id: if kind == Schema {
                "fake_schema"
            } else {
                "fake_input"
            },
            bytes: b"fake bytes",
            expected_sha256: sha256,
        })
        .to_vec()
    }

    fn identity(
        selection: &Selection<'_>,
        context: BuildContext<'_>,
        inputs: &[Input<'_>],
    ) -> ArtifactIdentity {
        identify(selection, context, inputs).expect("complete fake input identity")
    }

    #[test]
    fn fake_identity_is_explicitly_unqualified_and_input_order_independent() {
        let hash = hex::encode(Sha256::digest(b"fake bytes"));
        let mut inputs = inputs(&hash);
        let expected = identity(&fake_selection(), context(), &inputs);
        assert_eq!(expected.qualification, Qualification::Unqualified);
        assert_eq!(expected.inputs.len(), inputs.len());
        assert_eq!(expected.sha256.len(), 64);
        inputs.reverse();
        assert_eq!(identity(&fake_selection(), context(), &inputs), expected);
    }

    #[test]
    fn fake_identity_changes_with_each_build_context_axis_and_membership() {
        let hash = hex::encode(Sha256::digest(b"fake bytes"));
        let inputs = inputs(&hash);
        let selection = fake_selection();
        let baseline = identity(&selection, context(), &inputs);
        for changed in [
            BuildContext {
                target: "fake-other-target",
                ..context()
            },
            BuildContext {
                profile: "fake-other-profile",
                ..context()
            },
            BuildContext {
                enabled_features: &["fake-feature"],
                ..context()
            },
            BuildContext {
                compiler_verbose: "fake-other-rustc",
                ..context()
            },
            BuildContext {
                encoded_rustflags: "fake-other-flags",
                ..context()
            },
        ] {
            assert_ne!(
                identity(&selection, changed, &inputs).sha256,
                baseline.sha256
            );
        }
        let mut changed = selection;
        changed.members[0].ancestry.push("fake_parent");
        assert_ne!(
            identity(&changed, context(), &inputs).sha256,
            baseline.sha256
        );
    }

    #[test]
    fn fake_identity_requires_all_planning_code_lock_and_schema_inputs() {
        let hash = hex::encode(Sha256::digest(b"fake bytes"));
        let inputs = inputs(&hash);
        for index in 0..inputs.len() {
            let mut missing = inputs.clone();
            missing.remove(index);
            let expected = if inputs[index].kind == InputKind::Schema {
                "staging schema input membership differs from S".to_owned()
            } else {
                format!("missing staging input kind: {:?}", inputs[index].kind)
            };
            assert_eq!(
                identify(&fake_selection(), context(), &missing).expect_err("missing input"),
                expected
            );
        }
        let mut extra = inputs.clone();
        extra.push(Input {
            kind: InputKind::Schema,
            id: "fake_source_only",
            ..inputs[0]
        });
        assert_eq!(
            identify(&fake_selection(), context(), &extra).expect_err("extra schema"),
            "staging schema input membership differs from S"
        );
        extra = inputs.clone();
        extra.push(inputs[0]);
        assert_eq!(
            identify(&fake_selection(), context(), &extra).expect_err("duplicate input"),
            "duplicate staging input identity: fake_input"
        );
        for index in 0..inputs.len() {
            let mut stale = inputs.clone();
            stale[index].bytes = b"fake changed bytes";
            assert_eq!(
                identify(&fake_selection(), context(), &stale).expect_err("stale input"),
                format!("staging input digest mismatch: {}", inputs[index].id)
            );
        }
    }

    #[test]
    fn fake_identity_rebinds_every_changed_input_including_code_helpers_and_lock() {
        let old_hash = hex::encode(Sha256::digest(b"fake bytes"));
        let new_hash = hex::encode(Sha256::digest(b"fake changed bytes"));
        let inputs = inputs(&old_hash);
        let baseline = identity(&fake_selection(), context(), &inputs);
        for index in 0..inputs.len() {
            let mut changed = inputs.clone();
            changed[index].bytes = b"fake changed bytes";
            changed[index].expected_sha256 = &new_hash;
            assert_ne!(
                identity(&fake_selection(), context(), &changed).sha256,
                baseline.sha256,
                "changed {:?}",
                inputs[index].kind
            );
        }
    }
}

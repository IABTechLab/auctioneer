//! Exact child and caller-context correspondence, not numeric proof authority.

use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::{
    ValidationAssignment, ValidationView, exact_assignments,
};
use crate::schema_residency::expanded::DeclarationBody;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Binding,
    ChildCoverage,
    ContextCoverage,
    BodiesUnavailable,
    BodyCoverage,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BodyReceipt {
    owner: String,
    marker: String,
    expansion_sha256: String,
}

/// Private construction proves correspondence only; analysis is still unqualified.
struct CheckedBodies<'a, T> {
    view: ValidationView<'a>,
    receipts: Vec<BodyReceipt>,
    analyses: Vec<T>,
}

impl<T> CheckedBodies<'_, T> {
    fn recheck(&self, _view: ValidationView<'_>) -> Result<(), Refusal> {
        // This stage is intentionally RED until the same-kernel join exists.
        let _: (&ValidationView<'_>, &[BodyReceipt], &[T]) =
            (&self.view, &self.receipts, &self.analyses);
        Err(Refusal::BodiesUnavailable)
    }
}

fn checked_bodies<'a, T>(
    _seed: &CapturedSeed,
    _view: ValidationView<'a>,
    _analyze: impl FnMut(DeclarationBody<'_>) -> Result<T, Refusal>,
) -> Result<CheckedBodies<'a, T>, Refusal> {
    Err(Refusal::BodiesUnavailable)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ContextRoot {
    role: &'static str,
    source_owner: String,
    source_sha256: String,
}

// Pure correspondence checks cannot issue proof authority. The source join
// must obtain its expected rows and binding from the sealed descriptor.
fn check_children(
    expected_binding: &Value,
    actual_binding: &Value,
    expected: &[ValidationAssignment],
    actual: &[ValidationAssignment],
) -> Result<(), Refusal> {
    if expected_binding != actual_binding {
        return Err(Refusal::Binding);
    }
    exact_assignments(expected, actual).map_err(|_error| Refusal::ChildCoverage)
}

fn check_contexts(expected: &[ContextRoot], actual: &[ContextRoot]) -> Result<(), Refusal> {
    let unique_roles = |roots: &[ContextRoot]| {
        let mut roles = BTreeSet::new();
        roots.iter().all(|root| roles.insert(root.role))
    };
    if expected.is_empty()
        || !unique_roles(expected)
        || !unique_roles(actual)
        || expected.iter().collect::<BTreeSet<_>>() != actual.iter().collect::<BTreeSet<_>>()
    {
        return Err(Refusal::ContextCoverage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BodyReceipt, ContextRoot, Refusal, check_children, check_contexts, checked_bodies,
    };
    use crate::schema_build::application::CapturedSeed;
    use crate::schema_residency::descriptor::ValidationAssignment;
    use crate::schema_residency::expanded::DeclarationBody;
    use crate::schema_residency::with_current_descriptor;
    use quote::ToTokens as _;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};

    fn binding() -> Value {
        json!({
            "identity": "fake_capture_a",
            "expanded_graph_sha256": "fake_expansion_a",
            "model_sources": {"fake_model": "fake_model_sha_a"}
        })
    }

    fn assignment(owner: &str, role: &str) -> ValidationAssignment {
        ValidationAssignment {
            child: format!("{owner}::{role}"),
            source_owner: owner.into(),
            role: role.into(),
        }
    }

    fn children() -> Vec<ValidationAssignment> {
        let mut rows = ["cache", "mutable-scratch", "search-fallback"]
            .into_iter()
            .map(|role| assignment("fake_regex_owner", role))
            .collect::<Vec<_>>();
        for owner in ["fake_regex_declaration", "fake_zero_regex_declaration"] {
            for role in ["declaration-work", "declaration-stack"] {
                rows.push(assignment(owner, role));
            }
        }
        rows
    }

    fn contexts() -> Vec<ContextRoot> {
        vec![
            ContextRoot {
                role: "input-measurement",
                source_owner: "fake_preflight".into(),
                source_sha256: "fake_preflight_sha".into(),
            },
            ContextRoot {
                role: "coordination",
                source_owner: "fake_coordinator".into(),
                source_sha256: "fake_coordinator_sha".into(),
            },
        ]
    }

    #[test]
    fn same_capture_is_required() {
        let original = binding();
        let rows = children();
        assert_eq!(check_children(&original, &original, &rows, &rows), Ok(()));
        for key in ["identity", "expanded_graph_sha256", "model_sources"] {
            let mut changed = original.clone();
            assert!(changed.get(key).is_some(), "before-state binding: {key}");
            changed[key] = json!("fake_changed");
            assert_ne!(changed, original);
            assert_eq!(
                check_children(&original, &changed, &rows, &rows),
                Err(Refusal::Binding),
                "changed binding: {key}"
            );
        }
    }

    #[test]
    fn every_child_requires_exactly_one_owner() {
        let parent = binding();
        let original = children();
        assert_eq!(original.len(), 7);
        assert_eq!(
            check_children(&parent, &parent, &original, &original),
            Ok(())
        );
        for index in 0..original.len() {
            let mut missing = original.clone();
            assert_eq!(missing.remove(index), original[index]);
            assert_eq!(
                check_children(&parent, &parent, &original, &missing),
                Err(Refusal::ChildCoverage)
            );
            let mut duplicate = original.clone();
            duplicate.push(original[index].clone());
            assert_eq!(
                check_children(&parent, &parent, &original, &duplicate),
                Err(Refusal::ChildCoverage)
            );
            for (field, replacement) in [
                (
                    "child",
                    ValidationAssignment {
                        child: "fake_unknown_child".into(),
                        ..original[index].clone()
                    },
                ),
                (
                    "source_owner",
                    ValidationAssignment {
                        source_owner: "fake_other_owner".into(),
                        ..original[index].clone()
                    },
                ),
                (
                    "role",
                    ValidationAssignment {
                        role: "fake_other_role".into(),
                        ..original[index].clone()
                    },
                ),
            ] {
                let mut changed = original.clone();
                changed[index] = replacement;
                assert_ne!(changed, original);
                assert_eq!(
                    check_children(&parent, &parent, &original, &changed),
                    Err(Refusal::ChildCoverage),
                    "changed child field: {field}"
                );
            }
        }
        let mut extra = original.clone();
        extra.push(assignment("fake_extra_owner", "declaration-work"));
        assert_eq!(
            check_children(&parent, &parent, &original, &extra),
            Err(Refusal::ChildCoverage)
        );
        assert_eq!(
            check_children(&parent, &parent, &[], &[]),
            Err(Refusal::ChildCoverage)
        );
    }

    #[test]
    fn caller_context_is_not_a_regex_child() {
        let parent = binding();
        let rows = children();
        let original = contexts();
        assert_ne!(original[0].source_owner, original[1].source_owner);
        assert_ne!(original[0].source_sha256, original[1].source_sha256);
        assert_eq!(check_contexts(&original, &original), Ok(()));
        for index in 0..original.len() {
            let mut missing = original.clone();
            assert_eq!(missing.remove(index), original[index]);
            assert_eq!(
                check_contexts(&original, &missing),
                Err(Refusal::ContextCoverage)
            );
            let mut duplicate = original.clone();
            duplicate.push(original[index].clone());
            assert_eq!(
                check_contexts(&original, &duplicate),
                Err(Refusal::ContextCoverage)
            );
            let mut misowned = original.clone();
            misowned[index].source_owner = "fake_other_context".into();
            assert_eq!(
                check_contexts(&original, &misowned),
                Err(Refusal::ContextCoverage)
            );
            let mut stale = original.clone();
            stale[index].source_sha256 = "fake_changed_source".into();
            assert_eq!(
                check_contexts(&original, &stale),
                Err(Refusal::ContextCoverage)
            );
            let mut wrong_role = original.clone();
            wrong_role[index].role = "fake_unknown_context_role";
            assert_eq!(
                check_contexts(&original, &wrong_role),
                Err(Refusal::ContextCoverage)
            );
            let mut disguised = rows.clone();
            disguised.push(assignment(
                &original[index].source_owner,
                original[index].role,
            ));
            assert_eq!(
                check_children(&parent, &parent, &rows, &disguised),
                Err(Refusal::ChildCoverage)
            );
        }
        let mut extra = original.clone();
        extra.push(ContextRoot {
            role: "fake_extra_context",
            source_owner: "fake_extra_context".into(),
            source_sha256: "fake_extra_sha".into(),
        });
        assert_eq!(
            check_contexts(&original, &extra),
            Err(Refusal::ContextCoverage)
        );
        assert_eq!(check_contexts(&[], &[]), Err(Refusal::ContextCoverage));
    }

    #[test]
    fn zero_regex_declarations_still_require_work_and_stack() {
        let parent = binding();
        let original = children();
        assert_eq!(
            check_children(&parent, &parent, &original, &original),
            Ok(())
        );
        let zero_regex = original
            .iter()
            .filter(|row| row.source_owner == "fake_zero_regex_declaration")
            .collect::<Vec<_>>();
        assert_eq!(zero_regex.len(), 2);
        assert_eq!(zero_regex[0].role, "declaration-work");
        assert_eq!(zero_regex[1].role, "declaration-stack");
        for row in zero_regex {
            let mut missing = original.clone();
            let index = missing
                .iter()
                .position(|candidate| candidate == row)
                .unwrap();
            assert_eq!(&missing.remove(index), row);
            assert_eq!(
                check_children(&parent, &parent, &original, &missing),
                Err(Refusal::ChildCoverage),
                "zero-regex obligation: {}",
                row.role
            );
        }
    }

    fn body_shape(body: DeclarationBody<'_>) -> Result<usize, Refusal> {
        assert!(!body.owner.is_empty());
        assert!(!body.marker.is_empty());
        assert_eq!(body.expansion_sha256.len(), 64);
        let raw_sha256 = hex::encode(Sha256::digest(body.tokens.to_string().as_bytes()));
        let pristine: syn::File = syn::parse2(body.tokens.clone()).unwrap();
        if body.syntax.items.is_empty()
            || raw_sha256 != body.expansion_sha256
            || pristine.to_token_stream().to_string() != body.syntax.to_token_stream().to_string()
        {
            return Err(Refusal::BodyCoverage);
        }
        Ok(body.syntax.items.len())
    }

    fn copy_seed(seed: &CapturedSeed) -> CapturedSeed {
        CapturedSeed {
            identity_sha256: seed.identity_sha256.clone(),
            manifest_bytes: seed.manifest_bytes.clone(),
            rust: seed.rust.clone(),
            build_root: seed.build_root.clone(),
            out_dir: seed.out_dir.clone(),
            snapshots: seed.snapshots.clone(),
        }
    }

    #[test]
    fn actual_bodies_require_the_descriptor_capture() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            assert_eq!(view.check_handoff(view.assignments()), Ok(()));
            assert!(!view.selection().is_null());
            let expected = view.graph()["declarations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| BodyReceipt {
                    owner: row["owner"].as_str().unwrap().into(),
                    marker: row["marker"].as_str().unwrap().into(),
                    expansion_sha256: row["expansion_sha256"].as_str().unwrap().into(),
                })
                .collect::<Vec<_>>();
            assert!(!expected.is_empty());
            let mut visited = Vec::new();
            let result = checked_bodies(seed, view, |body| {
                visited.push(BodyReceipt {
                    owner: body.owner.into(),
                    marker: body.marker.into(),
                    expansion_sha256: body.expansion_sha256.into(),
                });
                body_shape(body)
            });
            assert_eq!(
                result.as_ref().map(|bodies| &bodies.receipts),
                Ok(&expected)
            );
            let bodies = result.unwrap();
            assert_eq!(visited, expected);
            assert_eq!(
                visited
                    .iter()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                expected.len()
            );
            assert_eq!(bodies.analyses.len(), expected.len());
            assert!(bodies.analyses.iter().all(|count| *count > 0));
            assert_eq!(bodies.recheck(view), Ok(()));
            assert_eq!(view.binding(), bodies.view.binding());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn body_receipts_reject_changed_owner_and_expansion() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let result = checked_bodies(seed, view, body_shape);
            assert_eq!(
                result.as_ref().map(|bodies| bodies.receipts.is_empty()),
                Ok(false)
            );
            let mut bodies = result.unwrap();
            assert_eq!(bodies.recheck(view), Ok(()));
            assert!(!bodies.receipts.is_empty());
            for index in 0..bodies.receipts.len() {
                let original = bodies.receipts[index].clone();
                bodies.receipts[index].owner = "fake_other_body_owner".into();
                assert_eq!(bodies.recheck(view), Err(Refusal::BodyCoverage));
                bodies.receipts[index] = original.clone();
                assert_eq!(bodies.recheck(view), Ok(()));
                bodies.receipts[index].marker = "FakeOtherMarker".into();
                assert_eq!(bodies.recheck(view), Err(Refusal::BodyCoverage));
                bodies.receipts[index] = original.clone();
                assert_eq!(bodies.recheck(view), Ok(()));
                bodies.receipts[index].expansion_sha256 = "fake_changed_expansion".into();
                assert_eq!(bodies.recheck(view), Err(Refusal::BodyCoverage));
                bodies.receipts[index] = original;
            }
            assert_eq!(bodies.recheck(view), Ok(()));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn seed_and_options_drift_cannot_reuse_body_authority() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            assert_eq!(
                checked_bodies(seed, view, body_shape).map(|b| b.receipts.len()),
                Ok(view.graph()["declarations"].as_array().unwrap().len())
            );
            with_current_descriptor(|other_seed, other_descriptor| {
                let other_view = other_descriptor.validation_view().unwrap();
                assert_ne!(view.binding(), other_view.binding());
                assert!(checked_bodies(other_seed, other_view, body_shape).is_ok());
                assert_eq!(
                    checked_bodies(other_seed, view, body_shape).err(),
                    Some(Refusal::Binding)
                );
                Ok(())
            })?;
            let mut changed = copy_seed(seed);
            let mut syntax = syn::parse_file(std::str::from_utf8(&changed.rust).unwrap()).unwrap();
            let declaration = syntax
                .items
                .iter_mut()
                .find_map(|item| {
                    if let syn::Item::Struct(item) = item {
                        item.attrs.iter_mut().find(|attribute| {
                            attribute
                                .path()
                                .segments
                                .last()
                                .is_some_and(|segment| segment.ident == "validator")
                        })
                    } else {
                        None
                    }
                })
                .unwrap();
            let syn::Meta::List(options) = &mut declaration.meta else {
                panic!("captured validator options");
            };
            // Reorder one complete top-level option, keeping groups and values intact.
            let original = options.tokens.to_string();
            let mut fields = options.tokens.clone().into_iter().collect::<Vec<_>>();
            let comma = fields.iter().position(|token| {
                matches!(token, proc_macro2::TokenTree::Punct(punct) if punct.as_char() == ',')
            }).unwrap();
            let first = fields.drain(..=comma).collect::<Vec<_>>();
            if !matches!(fields.last(), Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == ',') {
                fields.push(proc_macro2::TokenTree::Punct(proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone)));
            }
            fields.extend(first);
            options.tokens = fields.into_iter().collect();
            assert_ne!(options.tokens.to_string(), original);
            changed.rust = syntax.to_token_stream().to_string().into_bytes();
            let mut manifest: Value = serde_json::from_slice(&changed.manifest_bytes).unwrap();
            manifest["emitted_rust_sha256"] =
                json!(hex::encode(Sha256::digest(&changed.rust)));
            changed.manifest_bytes = serde_json::to_vec(&manifest).unwrap();
            assert!(crate::schema_residency::expanded::expand(&changed).is_ok(), "changed capture remains independently valid");
            assert_eq!(
                checked_bodies(&changed, view, body_shape).err(),
                Some(Refusal::Binding)
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn unrelated_nonempty_syntax_cannot_replace_pristine_body() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let mut rejected = 0;
            let result = checked_bodies(seed, view, |body| {
                let unrelated: syn::File = syn::parse_quote!(
                    fn fake_unrelated_body() {}
                );
                assert_eq!(
                    body_shape(DeclarationBody {
                        owner: body.owner,
                        marker: body.marker,
                        expansion_sha256: body.expansion_sha256,
                        tokens: body.tokens,
                        syntax: &unrelated,
                    }),
                    Err(Refusal::BodyCoverage)
                );
                rejected += 1;
                body_shape(body)
            });
            assert_eq!(
                result.as_ref().map(|bodies| bodies.receipts.len()),
                Ok(view.graph()["declarations"].as_array().unwrap().len())
            );
            let bodies = result.unwrap();
            assert!(!bodies.receipts.is_empty());
            assert_eq!(rejected, bodies.receipts.len());
            Ok(())
        })
        .unwrap();
    }
}

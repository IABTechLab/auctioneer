//! Exact child and caller-context correspondence, not numeric proof authority.

use crate::schema_residency::descriptor::ValidationAssignment;
use serde_json::Value;

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Binding,
    ChildCoverage,
    ContextCoverage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContextRoot {
    role: &'static str,
    source_owner: String,
    source_sha256: String,
}

// Compileable tests-first entrypoints. Neither can issue a proof/report.
// The hosted refusal controls must fail before the exact join is implemented.
fn check_children(
    _expected_binding: &Value,
    _actual_binding: &Value,
    _expected: &[ValidationAssignment],
    actual: &[ValidationAssignment],
) -> Result<(), Refusal> {
    actual.first().map(|_row| ()).ok_or(Refusal::ChildCoverage)
}

fn check_contexts(_expected: &[ContextRoot], actual: &[ContextRoot]) -> Result<(), Refusal> {
    actual
        .first()
        .map(|_row| ())
        .ok_or(Refusal::ContextCoverage)
}

#[cfg(test)]
mod tests {
    use super::{ContextRoot, Refusal, check_children, check_contexts};
    use crate::schema_residency::descriptor::ValidationAssignment;
    use serde_json::{Value, json};

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
}

//! Shared generated declarations for inspection and controlled-native admission.
//!
//! Generated declarations and all helpers stay private. Retention is only an
//! inspection aid; it does not establish resource qualification. Native callers
//! must pass the separate installed-reservation and initialization boundary.

#[cfg(not(target_arch = "wasm32"))]
use serde_json::Value;

#[cfg(any(test, feature = "test-support"))]
#[cfg_attr(
    feature = "schema-artifact-staging",
    expect(
        clippy::unnecessary_wraps,
        reason = "the shared inspection API returns None in normal builds"
    )
)]
pub(crate) fn inspection_manifest() -> Option<&'static str> {
    #[cfg(feature = "schema-artifact-staging")]
    {
        Some(include_str!(concat!(
            env!("OUT_DIR"),
            "/schema-staging.json"
        )))
    }
    #[cfg(not(feature = "schema-artifact-staging"))]
    {
        None
    }
}

#[cfg(any(not(target_arch = "wasm32"), feature = "schema-artifact-staging"))]
mod formats {
    use regex_automata::{Input, MatchKind, meta::Regex, util::syntax};
    use std::sync::LazyLock;
    use uuid_simd::AsOut as _;

    pub(super) fn uuid(value: &str) -> bool {
        if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return false;
        }
        let mut decoded = [0_u8; 16];
        uuid_simd::parse_hyphenated(value.as_bytes(), decoded.as_out()).is_ok()
    }

    const HOSTNAME_SOURCE: &str = r"^([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])(\.([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9]))*$";

    static HOSTNAME_PATTERN: LazyLock<Option<Regex>> = LazyLock::new(|| {
        Regex::builder()
            .configure(
                Regex::config()
                    .match_kind(MatchKind::LeftmostFirst)
                    .utf8_empty(true)
                    .nfa_size_limit(Some(0x0001_0000))
                    .hybrid_cache_capacity(0x0001_0000)
                    .pool_capacity(1),
            )
            .syntax(syntax::Config::default().utf8(true))
            .build(HOSTNAME_SOURCE)
            .ok()
    });

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn initialize() -> bool {
        HOSTNAME_PATTERN.is_some()
    }

    pub(super) fn hostname(value: &str) -> bool {
        value.len() < 256
            && HOSTNAME_PATTERN.as_ref().is_some_and(|pattern| {
                let mut cache = pattern.create_cache();
                pattern
                    .search_half_with(&mut cache, &Input::new(value).earliest(true))
                    .is_some()
            })
    }

    #[cfg(test)]
    mod tests {
        use super::{hostname, uuid};

        #[test]
        fn staged_uuid_preserves_lowercase_hyphenated_candidate_semantics() {
            assert!(uuid("123e4567-e89b-12d3-a456-426614174000"));
            for value in [
                "123E4567-e89b-12d3-a456-426614174000",
                "123e4567e89b12d3a456426614174000",
                "123e4567-e89b-12d3-a456-42661417400g",
                "",
            ] {
                assert!(!uuid(value), "{value:?}");
            }
        }

        #[test]
        fn staged_hostname_preserves_candidate_label_and_total_length_guards() {
            for value in ["a.example", "A.example", "localhost", "a-b.example"] {
                assert!(hostname(value), "{value:?}");
            }
            for value in [
                "",
                "-a.example",
                "a-.example",
                "a..example",
                "a.example.",
                "a/b",
            ] {
                assert!(!hostname(value), "{value:?}");
            }
            assert!(!hostname(&"a".repeat(64)));
            let at = [
                "a".repeat(63),
                "a".repeat(63),
                "a".repeat(63),
                "a".repeat(63),
            ]
            .join(".");
            assert_eq!(at.len(), 255);
            assert!(hostname(&at));
            assert!(!hostname(&format!("a.{at}")));
        }
    }
}

#[cfg(any(not(target_arch = "wasm32"), feature = "schema-artifact-staging"))]
#[expect(
    clippy::absolute_paths,
    reason = "the schema converter requires absolute custom-format paths in its generated scopes"
)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/schema-staging.rs"));
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn initialize_native() -> bool {
    generated::retain_for_inspection();
    formats::initialize() && generated::initialize_native()
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn native_owner(name: &str) -> Option<&'static str> {
    generated::native_owner(name)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn native_required(name: &str) -> bool {
    generated::native_required(name)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn validate_native(owner: &str, value: &Value) -> Option<bool> {
    generated::validate_native(owner, value)
}

/// Metadata-only refusal hook. Retains pointers but never calls their targets.
/// This is not an initializer, validator accessor, or resource descriptor.
pub(crate) fn has_unqualified_artifact() -> bool {
    #[cfg(feature = "schema-artifact-staging")]
    {
        generated::retain_for_inspection();
        true
    }
    #[cfg(not(feature = "schema-artifact-staging"))]
    {
        false
    }
}

use regex_automata::{Input, MatchKind, meta::Regex, util::syntax};
use std::sync::LazyLock;
use uuid_simd::AsOut as _;

pub(crate) fn reference_uuid_format(value: &str) -> bool {
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return false;
    }
    let mut decoded = [0_u8; 16];
    uuid_simd::parse_hyphenated(value.as_bytes(), decoded.as_out()).is_ok()
}

pub(crate) const HOSTNAME_SOURCE: &str = r"^([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])(\.([a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]{0,61}[a-zA-Z0-9]))*$";

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
pub(crate) fn initialize_reference_formats() -> bool {
    HOSTNAME_PATTERN.as_ref().is_some()
}

pub(crate) fn reference_hostname_format(value: &str) -> bool {
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
    use super::{
        HOSTNAME_PATTERN, initialize_reference_formats, reference_hostname_format,
        reference_uuid_format,
    };
    use regex_automata::{MatchKind, nfa::thompson::WhichCaptures};

    #[test]
    fn uuid_format_requires_lowercase_hyphenated_input() {
        assert!(reference_uuid_format(
            "123e4567-e89b-12d3-a456-426614174000"
        ));
        for invalid in [
            "123E4567-e89b-12d3-a456-426614174000",
            "123e4567e89b12d3a456426614174000",
            "123e4567-e89b-12d3-a456-42661417400g",
        ] {
            assert!(!reference_uuid_format(invalid));
        }
    }

    #[test]
    fn hostname_initialization_keeps_valid_input_and_the_length_guard() {
        assert!(initialize_reference_formats());
        assert!(reference_hostname_format("a.example"));
        assert!(!reference_hostname_format(&"a".repeat(256)));
    }

    #[test]
    fn hostname_engine_preserves_search_and_compilation_limits() {
        let config = HOSTNAME_PATTERN.as_ref().unwrap().get_config();
        assert_eq!(config.get_match_kind(), MatchKind::LeftmostFirst);
        assert!(config.get_utf8_empty());
        assert_eq!(config.get_nfa_size_limit(), Some(0x0001_0000));
        assert_eq!(config.get_hybrid_cache_capacity(), 0x0001_0000);
        assert_eq!(config.get_pool_capacity(), 1);
        assert!(matches!(config.get_which_captures(), WhichCaptures::All));
        assert!(config.get_byte_classes());
        assert_eq!(config.get_line_terminator(), b'\n');
        assert!(config.get_hybrid());
        assert!(!config.get_dfa());
        assert!(config.get_prefilter().is_none());
    }
}

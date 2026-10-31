//! Pure compiler for public bidder names and explicit implementation bindings.

use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use validator::{Validate, ValidationError};

/// Borrowed metadata declaration; no factory lookup or feature filtering occurs here.
#[derive(Clone, Copy, Debug, Validate)]
pub struct BindingInput<'a> {
    #[validate(
        length(min = 1_u64, max = 128_u64),
        custom(function = "validate_bidder_characters")
    )]
    pub bidder: &'a str,
    #[validate(custom(function = "validate_implementation"))]
    pub implementation: &'a str,
    pub required_feature: Option<&'a str>,
}

/// Fixed, field-oriented binding diagnostics that never retain input values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingError {
    InvalidBidder,
    InvalidImplementation,
    UnknownRequiredFeature,
    ForbiddenRequiredFeature,
    DuplicateBidder,
    FeatureNameCollision,
}

impl Display for BindingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidBidder => {
                "bidder: must be 1..=128 bytes of ASCII letters, digits, '_' or '-'"
            }
            Self::InvalidImplementation => {
                "implementation: must be a lowercase nonkeyword Rust identifier"
            }
            Self::UnknownRequiredFeature => "required_feature: must name a declared feature",
            Self::ForbiddenRequiredFeature => {
                "required_feature: default and test-harness-override are not bidder requirements"
            }
            Self::DuplicateBidder => "bidder: duplicate public identity under ASCII case folding",
            Self::FeatureNameCollision => {
                "known_features: feature names collide after Cargo environment normalization"
            }
        })
    }
}

/// Validate all declarations, then render static active bindings and all declarations.
///
/// # Errors
/// Returns a field-oriented error for malformed or conflicting declarations/features.
pub fn compile_bindings(
    rows: &[BindingInput<'_>],
    known_features: &BTreeSet<String>,
) -> Result<String, BindingError> {
    let mut environment_names = BTreeSet::new();
    for feature in known_features {
        if !environment_names.insert(feature.to_uppercase().replace('-', "_")) {
            return Err(BindingError::FeatureNameCollision);
        }
    }

    let mut public_names = BTreeSet::new();
    for row in rows {
        if let Err(errors) = row.validate() {
            return Err(if errors.field_errors().contains_key("bidder") {
                BindingError::InvalidBidder
            } else {
                BindingError::InvalidImplementation
            });
        }
        if let Some(feature) = row.required_feature {
            if matches!(feature, "default" | "test-harness-override") {
                return Err(BindingError::ForbiddenRequiredFeature);
            }
            if !known_features.contains(feature) {
                return Err(BindingError::UnknownRequiredFeature);
            }
        }
        if !public_names.insert(row.bidder.to_ascii_lowercase()) {
            return Err(BindingError::DuplicateBidder);
        }
    }

    let mut sorted: Vec<_> = rows.iter().collect();
    sorted.sort_unstable_by(|left, right| left.bidder.cmp(right.bidder));
    let mut source = String::from(
        "use crate::types::{BidderImplementationBinding, BidderMetadataDeclaration};\n\n#[must_use]\npub const fn bidder_implementation_bindings() -> &'static [BidderImplementationBinding] {\n    static BINDINGS: &[BidderImplementationBinding] = &[\n",
    );
    for row in &sorted {
        if let Some(feature) = row.required_feature {
            source.push_str("        #[cfg(feature = ");
            source.push_str(&rust_string_literal(feature));
            source.push_str(")]\n");
        }
        source.push_str("        BidderImplementationBinding { bidder: ");
        source.push_str(&rust_string_literal(row.bidder));
        source.push_str(", implementation: ");
        source.push_str(&rust_string_literal(row.implementation));
        source.push_str(" },\n");
    }
    source.push_str(
        "    ];\n    BINDINGS\n}\n\n#[must_use]\npub fn bidder_metadata_declarations() -> &'static [BidderMetadataDeclaration] {\n    static DECLARATIONS: &[BidderMetadataDeclaration] = &[\n",
    );
    for row in &sorted {
        source.push_str("        BidderMetadataDeclaration { bidder: ");
        source.push_str(&rust_string_literal(row.bidder));
        source.push_str(", implementation: ");
        source.push_str(&rust_string_literal(row.implementation));
        source.push_str(", required_feature: ");
        if let Some(feature) = row.required_feature {
            source.push_str("Some(");
            source.push_str(&rust_string_literal(feature));
            source.push(')');
        } else {
            source.push_str("None");
        }
        source.push_str(" },\n");
    }
    source.push_str("    ];\n    DECLARATIONS\n}\n");
    Ok(source)
}

fn rust_string_literal(value: &str) -> String {
    let mut literal = String::from("\"");
    literal.extend(value.chars().flat_map(char::escape_default));
    literal.push('"');
    literal
}

fn validate_bidder_characters(value: &str) -> Result<(), ValidationError> {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Ok(())
    } else {
        Err(ValidationError::new("bidder_characters"))
    }
}

fn validate_implementation(value: &str) -> Result<(), ValidationError> {
    // Match discovery's lexical key rules; syn does not reserve Rust 2024 `gen`.
    if value != "gen"
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && syn::parse_str::<syn::Ident>(value).is_ok()
    {
        Ok(())
    } else {
        Err(ValidationError::new("implementation_identifier"))
    }
}

#[cfg(test)]
mod tests {
    use super::{BindingError, BindingInput, compile_bindings, rust_string_literal};
    use std::collections::BTreeSet;

    fn row<'a>(
        bidder: &'a str,
        implementation: &'a str,
        required_feature: Option<&'a str>,
    ) -> BindingInput<'a> {
        BindingInput {
            bidder,
            implementation,
            required_feature,
        }
    }

    fn features() -> BTreeSet<String> {
        [
            "default",
            "experimental",
            "insecure-bidders",
            "test-harness-override",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    #[test]
    fn empty_catalog_renders_both_static_views_as_valid_rust() {
        let source = compile_bindings(&[], &BTreeSet::new()).unwrap();
        let parsed = syn::parse_file(&source).unwrap();
        let functions: Vec<_> = parsed
            .items
            .iter()
            .filter_map(|item| {
                if let syn::Item::Fn(function) = item {
                    Some(function.sig.ident.to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            functions,
            [
                "bidder_implementation_bindings",
                "bidder_metadata_declarations"
            ]
        );
        assert!(source.starts_with(
            "use crate::types::{BidderImplementationBinding, BidderMetadataDeclaration};\n"
        ));
        assert_eq!(source.matches("crate::types::").count(), 1);
        assert!(source.contains("static BINDINGS: &[BidderImplementationBinding]"));
        assert!(source.contains("static DECLARATIONS: &[BidderMetadataDeclaration]"));
        assert_eq!(source.matches("= &[\n    ];").count(), 2);
        assert_eq!(source.matches("#[must_use]").count(), 2);
    }

    #[test]
    fn public_spelling_and_explicit_implementation_remain_distinct() {
        let source = compile_bindings(
            &[row("fake_fixtureAds", "fake_fixture_ads", None)],
            &features(),
        )
        .unwrap();
        syn::parse_file(&source).unwrap();
        assert_eq!(
            source
                .matches("bidder: \"fake_fixtureAds\", implementation: \"fake_fixture_ads\"")
                .count(),
            2
        );
        assert!(!source.contains("bidder: \"fake_fixture_ads\""));
        assert!(!source.contains("implementation: \"fake_fixtureAds\""));
    }

    #[test]
    fn accepts_public_case_digit_leading_and_boundary_length_without_guessing_a_factory() {
        let boundary = "A".repeat(128);
        for bidder in ["9signal", "A-Z_09", "-", "_", boundary.as_str()] {
            let source =
                compile_bindings(&[row(bidder, "fake_new_factory_7", None)], &features()).unwrap();
            assert_eq!(source.matches(&format!("bidder: \"{bidder}\"")).count(), 2);
            assert_eq!(
                source
                    .matches("implementation: \"fake_new_factory_7\"")
                    .count(),
                2
            );
        }
    }

    #[test]
    fn distinct_public_bases_keep_their_shared_implementation() {
        let source = compile_bindings(
            &[
                row("fake_fixture_alias", "fake_fixture_base", None),
                row("fake_fixture_base", "fake_fixture_base", None),
            ],
            &features(),
        )
        .unwrap();
        assert_eq!(
            source
                .matches("implementation: \"fake_fixture_base\"")
                .count(),
            4
        );
        assert_eq!(source.matches("bidder: \"fake_fixture_alias\"").count(), 2);
        assert_eq!(source.matches("bidder: \"fake_fixture_base\"").count(), 2);
    }

    #[test]
    fn output_is_byte_deterministic_and_sorted_by_exact_public_name() {
        let mut rows = [
            row("fake_zeta", "fake_shared", None),
            row("fake_fixtureAds", "fake_fixture_ads", Some("experimental")),
            row("fake_Alpha", "fake_shared", None),
            row("fake_9signal", "fake_signal", Some("insecure-bidders")),
        ];
        let source = compile_bindings(&rows, &features()).unwrap();
        for _ in 0..rows.len() {
            rows.rotate_left(1);
            assert_eq!(source, compile_bindings(&rows, &features()).unwrap());
        }
        rows.reverse();
        assert_eq!(source, compile_bindings(&rows, &features()).unwrap());
        let positions: Vec<_> = ["fake_9signal", "fake_Alpha", "fake_fixtureAds", "fake_zeta"]
            .into_iter()
            .map(|bidder| source.find(&format!("bidder: \"{bidder}\"")).unwrap())
            .collect();
        assert!(positions.windows(2).all(|pair| pair.first() < pair.get(1)));
    }

    #[test]
    fn only_active_bindings_are_gated_and_all_declarations_retain_the_predicate() {
        let source = compile_bindings(
            &[
                row("fake_secure", "fake_secure", None),
                row("fake_gated", "fake_gated", Some("experimental")),
            ],
            &features(),
        )
        .unwrap();
        let (active, all) = source
            .split_once("pub fn bidder_metadata_declarations")
            .unwrap();
        assert_eq!(
            active.matches("#[cfg(feature = \"experimental\")]").count(),
            1
        );
        assert!(!all.contains("#[cfg"));
        assert!(all.contains(
            "bidder: \"fake_gated\", implementation: \"fake_gated\", required_feature: Some(\"experimental\")"
        ));
        assert!(all.contains(
            "bidder: \"fake_secure\", implementation: \"fake_secure\", required_feature: None"
        ));
    }

    #[test]
    fn feature_strings_are_escaped_as_rust_literals_not_source_fragments() {
        let feature = "quoted\"\\\n\r\t\0\u{2028}";
        let source = compile_bindings(
            &[row("fake_gated", "fake_gated", Some(feature))],
            &BTreeSet::from([feature.to_owned()]),
        )
        .unwrap();
        syn::parse_file(&source).unwrap();
        let literal = rust_string_literal(feature);
        assert_eq!(
            syn::parse_str::<syn::LitStr>(&literal).unwrap().value(),
            feature
        );
        assert!(source.contains(&format!("#[cfg(feature = {literal})]")));
        assert!(source.contains(&format!("required_feature: Some({literal})")));
        assert!(!source.contains(feature));
    }

    #[test]
    fn rust_string_literals_roundtrip_control_characters_quotes_and_unicode() {
        for value in [
            "",
            "fake_fixtureAds",
            "'\"\\\n\r\t\0\u{8}\u{c}",
            "\u{e9}\u{2028}\u{1f600}",
            "\"); compile_error!(\"not source\"); //",
        ] {
            let literal = rust_string_literal(value);
            assert_eq!(
                syn::parse_str::<syn::LitStr>(&literal).unwrap().value(),
                value
            );
        }
    }

    #[test]
    fn rejects_malformed_public_names_including_a_gated_off_row() {
        let oversized = "a".repeat(129);
        for bidder in [
            "",
            "has space",
            "a.b",
            "a/b",
            "a\\b",
            "\"injected\"",
            "a\n",
            "a\0",
            "\u{e9}",
            oversized.as_str(),
        ] {
            for feature in [None, Some("experimental")] {
                assert_eq!(
                    compile_bindings(&[row(bidder, "fake_valid", feature)], &features()),
                    Err(BindingError::InvalidBidder),
                    "bidder {} / feature {}",
                    rust_string_literal(bidder),
                    feature.unwrap_or("none")
                );
            }
        }
    }

    #[test]
    fn rejects_invalid_implementation_keys_and_keywords_even_when_gated_off() {
        for implementation in [
            "",
            "_",
            "FixtureAds",
            "1probe",
            "bad-key",
            "r#type",
            "../probe",
            "type",
            "async",
            "gen",
            "self",
            "true",
            "try",
            "yield",
            "\u{fc}ber",
            "probe\n",
            "probe::factory",
        ] {
            for feature in [None, Some("experimental")] {
                assert_eq!(
                    compile_bindings(
                        &[row("fake_publicName", implementation, feature)],
                        &features()
                    ),
                    Err(BindingError::InvalidImplementation),
                    "implementation {} / feature {}",
                    rust_string_literal(implementation),
                    feature.unwrap_or("none")
                );
            }
        }
    }

    #[test]
    fn implementation_identifiers_follow_discovery_key_semantics() {
        for implementation in ["fake_fixture_ads", "_probe", "fake_probe_2", "union"] {
            let source =
                compile_bindings(&[row("fake_publicName", implementation, None)], &features())
                    .unwrap();
            assert_eq!(
                source
                    .matches(&format!("implementation: \"{implementation}\""))
                    .count(),
                2
            );
        }
    }

    #[test]
    fn rejects_unknown_or_empty_required_feature() {
        for feature in ["absent", "", "Experimental"] {
            assert_eq!(
                compile_bindings(
                    &[row("fake_public", "fake_factory", Some(feature))],
                    &features()
                ),
                Err(BindingError::UnknownRequiredFeature)
            );
        }
    }

    #[test]
    fn rejects_default_and_test_harness_requirements_even_if_declared() {
        for feature in ["default", "test-harness-override"] {
            assert_eq!(
                compile_bindings(
                    &[row("fake_public", "fake_factory", Some(feature))],
                    &features()
                ),
                Err(BindingError::ForbiddenRequiredFeature)
            );
        }
    }

    #[test]
    fn rejects_exact_and_case_folded_public_duplicates_before_any_feature_filtering() {
        for duplicate in ["fake_fixtureAds", "fake_FIXTUREADS", "fake_fixtureads"] {
            for implementation in ["fake_fixture_ads", "fake_different"] {
                for feature in [None, Some("experimental")] {
                    assert_eq!(
                        compile_bindings(
                            &[
                                row("fake_fixtureAds", "fake_fixture_ads", None),
                                row(duplicate, implementation, feature),
                            ],
                            &features()
                        ),
                        Err(BindingError::DuplicateBidder)
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_cargo_environment_feature_collisions_even_without_rows() {
        for names in [["foo-bar", "foo_bar"], ["feature", "FEATURE"]] {
            let known = names.into_iter().map(str::to_owned).collect();
            assert_eq!(
                compile_bindings(&[], &known),
                Err(BindingError::FeatureNameCollision)
            );
        }
    }

    #[test]
    fn accepts_distinct_declared_features_without_a_hardcoded_feature_list() {
        let known = BTreeSet::from(["new-feature".to_owned(), "other_feature".to_owned()]);
        let source = compile_bindings(
            &[
                row("fake_first", "fake_factory", Some("new-feature")),
                row("fake_second", "fake_factory", Some("other_feature")),
            ],
            &known,
        )
        .unwrap();
        assert!(source.contains("#[cfg(feature = \"new-feature\")]"));
        assert!(source.contains("#[cfg(feature = \"other_feature\")]"));
    }

    #[test]
    fn diagnostics_are_fixed_field_oriented_and_do_not_echo_input() {
        for (error, message) in [
            (
                BindingError::InvalidBidder,
                "bidder: must be 1..=128 bytes of ASCII letters, digits, '_' or '-'",
            ),
            (
                BindingError::InvalidImplementation,
                "implementation: must be a lowercase nonkeyword Rust identifier",
            ),
            (
                BindingError::UnknownRequiredFeature,
                "required_feature: must name a declared feature",
            ),
            (
                BindingError::ForbiddenRequiredFeature,
                "required_feature: default and test-harness-override are not bidder requirements",
            ),
            (
                BindingError::DuplicateBidder,
                "bidder: duplicate public identity under ASCII case folding",
            ),
            (
                BindingError::FeatureNameCollision,
                "known_features: feature names collide after Cargo environment normalization",
            ),
        ] {
            assert_eq!(error.to_string(), message);
        }
    }
}

//! Quoted declarations only: no serving registry, initialization, or installation.
//!
//! The owning build adapter must verify schema bytes and provide audited inputs.
//! The existing compile-time converter remains responsible for validator code
//! and compile errors; this API is not a replacement JSON Schema engine.

use crate::schema_staging::Selection;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

pub const CANDIDATE_OPTIONS: &str = "jsonschema=workspace;regex=workspace;cache=explicit;size_limit=65536;dfa_size_limit=65536;validate_formats=true;ignore_unknown_formats=false;is_valid=true;validate=false;iter_errors=false;formats=private_uuid_hostname";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Draft {
    Draft4,
    Draft202012,
    Unsupported,
}

#[derive(Clone, Copy, Debug)]
pub struct SchemaDeclaration<'a> {
    pub owner: &'a str,
    pub path: &'a Path,
    pub draft: Draft,
    pub formats: &'a [&'a str],
    pub unknown_keywords: &'a [&'a str],
    pub local_references: &'a [&'a str],
    pub external_references: &'a [&'a str],
}

/// Private owner-local helpers, not arbitrary caller-supplied callback paths.
#[derive(Clone, Copy, Debug)]
pub struct HelperAvailability {
    pub uuid: bool,
    pub hostname: bool,
}

#[derive(Debug)]
pub struct ValidatorDeclarations {
    pub items: TokenStream,
    pub owners: Vec<String>,
    pub required_formats: Vec<String>,
}

/// Quote exactly S, privately, with the fixed candidate engine/options.
/// Helper paths are fixed to the owning private staging module. The converter
/// requires absolute custom-format paths; no raw callback/FFI/plugin API is emitted.
///
/// # Errors
/// Refuses missing/extra/duplicate owners, unsupported audits, unavailable
/// required helpers and non-normal absolute paths. References are explicitly
/// unsupported in this first candidate, never ignored or partially resolved.
pub fn quote_validators(
    selection: &Selection<'_>,
    schemas: &[SchemaDeclaration<'_>],
    helpers: HelperAvailability,
) -> Result<ValidatorDeclarations, String> {
    let mut by_owner = BTreeMap::new();
    for schema in schemas {
        if schema.owner.trim().is_empty() || by_owner.insert(schema.owner, schema).is_some() {
            return Err("empty or duplicate staged schema owner".into());
        }
    }
    let expected: BTreeSet<_> = selection.schema_owners.iter().copied().collect();
    if expected.len() != selection.schema_owners.len()
        || expected != by_owner.keys().copied().collect()
    {
        return Err("staged declarations differ from S".into());
    }
    let mut items = TokenStream::new();
    let mut owners = Vec::new();
    let mut required_formats = BTreeSet::new();
    for (index, (owner, schema)) in by_owner.into_iter().enumerate() {
        if !schema.unknown_keywords.is_empty()
            || !schema.local_references.is_empty()
            || !schema.external_references.is_empty()
        {
            return Err(format!("unqualified staged keyword/reference: {owner}"));
        }
        let draft = match schema.draft {
            Draft::Draft4 => quote!(Draft4),
            Draft::Draft202012 => quote!(Draft202012),
            Draft::Unsupported => return Err(format!("unqualified staged draft: {owner}")),
        };
        if !schema.path.is_absolute()
            || schema
                .path
                .components()
                .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
            || schema.path.components().collect::<PathBuf>().as_os_str() != schema.path.as_os_str()
        {
            return Err(format!("non-normal staged schema path: {owner}"));
        }
        let path_text = schema.path.to_str().ok_or("non UTF-8 staged schema path")?;
        let path_literal = syn::LitStr::new(path_text, Span::call_site());
        let mut formats = Vec::new();
        let schema_formats: BTreeSet<_> = schema.formats.iter().copied().collect();
        for format in schema_formats {
            let helper = match format {
                "uuid" if helpers.uuid => quote!(crate::schema::staging::formats::uuid),
                "hostname" if helpers.hostname => quote!(crate::schema::staging::formats::hostname),
                _ => {
                    return Err(format!(
                        "unsupported or unavailable staged format: {format}"
                    ));
                }
            };
            formats.push(quote!(#format => #helper));
            required_formats.insert(format.to_owned());
        }
        let marker = format_ident!("StagedSchema{index}");
        items.extend(quote! {
            #[jsonschema::validator(
                path = #path_literal, draft = #draft, validate_formats = true,
                ignore_unknown_formats = false,
                formats = { #(#formats),* },
                pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            struct #marker;
        });
        owners.push(owner.to_owned());
    }
    Ok(ValidatorDeclarations {
        items,
        owners,
        required_formats: required_formats.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Draft, HelperAvailability, SchemaDeclaration, quote_validators};
    use crate::schema_staging::Selection;
    use quote::quote;
    use std::path::Path;

    const HELPERS: HelperAvailability = HelperAvailability {
        uuid: true,
        hostname: true,
    };

    fn selected() -> Selection<'static> {
        Selection {
            members: vec![],
            schema_owners: vec!["fake_a", "fake_b"],
        }
    }

    fn schema(owner: &'static str) -> SchemaDeclaration<'static> {
        SchemaDeclaration {
            owner,
            path: Path::new("/fake/schema.json"),
            draft: Draft::Draft4,
            formats: &[],
            unknown_keywords: &[],
            local_references: &[],
            external_references: &[],
        }
    }

    #[test]
    fn fake_quote_uses_exact_private_roster_and_fixed_candidate_options() {
        let a = SchemaDeclaration {
            formats: &["uuid", "hostname"],
            ..schema("fake_a")
        };
        let b = SchemaDeclaration {
            draft: Draft::Draft202012,
            ..schema("fake_b")
        };
        let generated = quote_validators(&selected(), &[b, a], HELPERS).expect("fake declarations");
        assert_eq!(generated.owners, ["fake_a", "fake_b"]);
        assert_eq!(generated.required_formats, ["hostname", "uuid"]);
        let expected = quote! {
            #[jsonschema::validator(
                path = "/fake/schema.json", draft = Draft4, validate_formats = true,
                ignore_unknown_formats = false,
                formats = { "hostname" => crate::schema::staging::formats::hostname, "uuid" => crate::schema::staging::formats::uuid },
                pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            struct StagedSchema0;
            #[jsonschema::validator(
                path = "/fake/schema.json", draft = Draft202012, validate_formats = true,
                ignore_unknown_formats = false,
                formats = {},
                pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
                methods = { is_valid = true, validate = false, iter_errors = false }
            )]
            struct StagedSchema1;
        };
        assert_eq!(generated.items.to_string(), expected.to_string());
        let syntax = syn::parse2::<syn::File>(generated.items).expect("valid Rust declarations");
        assert_eq!(syntax.items.len(), 2);
        for item in syntax.items {
            let syn::Item::Struct(marker) = item else {
                panic!("declarations must not install/call validators")
            };
            assert!(matches!(marker.vis, syn::Visibility::Inherited));
        }
    }

    #[test]
    fn fake_quote_refuses_missing_extra_and_duplicate_schema_owners() {
        let cases = [
            vec![schema("fake_a")],
            vec![
                schema("fake_a"),
                schema("fake_b"),
                schema("fake_source_only"),
            ],
            vec![schema("fake_a"), schema("fake_b"), schema("fake_a")],
        ];
        for (schemas, expected) in cases.into_iter().zip([
            "staged declarations differ from S",
            "staged declarations differ from S",
            "empty or duplicate staged schema owner",
        ]) {
            assert_eq!(
                quote_validators(&selected(), &schemas, HELPERS).expect_err("invalid owners"),
                expected
            );
        }
    }

    #[test]
    fn fake_quote_refuses_unqualified_audits_without_allow_all_or_fallback() {
        let cases = [
            SchemaDeclaration {
                draft: Draft::Unsupported,
                ..schema("fake_a")
            },
            SchemaDeclaration {
                formats: &["fake_unknown_format"],
                ..schema("fake_a")
            },
            SchemaDeclaration {
                unknown_keywords: &["fake_unknown_keyword"],
                ..schema("fake_a")
            },
            SchemaDeclaration {
                local_references: &["#/$defs/fake"],
                ..schema("fake_a")
            },
            SchemaDeclaration {
                external_references: &["https://fake.invalid/schema"],
                ..schema("fake_a")
            },
        ];
        for (a, expected) in cases.into_iter().zip([
            "unqualified staged draft: fake_a",
            "unsupported or unavailable staged format: fake_unknown_format",
            "unqualified staged keyword/reference: fake_a",
            "unqualified staged keyword/reference: fake_a",
            "unqualified staged keyword/reference: fake_a",
        ]) {
            assert_eq!(
                quote_validators(&selected(), &[a, schema("fake_b")], HELPERS)
                    .expect_err("unsupported audit"),
                expected
            );
        }
    }

    #[test]
    fn fake_quote_refuses_unavailable_helpers_but_does_not_require_unused_helpers() {
        for (format, helpers) in [
            (
                "uuid",
                HelperAvailability {
                    uuid: false,
                    hostname: true,
                },
            ),
            (
                "hostname",
                HelperAvailability {
                    uuid: true,
                    hostname: false,
                },
            ),
        ] {
            let a = SchemaDeclaration {
                formats: &[format],
                ..schema("fake_a")
            };
            assert_eq!(
                quote_validators(&selected(), &[a, schema("fake_b")], helpers)
                    .expect_err("unavailable helper"),
                format!("unsupported or unavailable staged format: {format}")
            );
        }
        let generated = quote_validators(
            &selected(),
            &[schema("fake_a"), schema("fake_b")],
            HelperAvailability {
                uuid: false,
                hostname: false,
            },
        )
        .expect("no formats needed");
        assert!(generated.required_formats.is_empty());
    }

    #[test]
    fn fake_quote_refuses_non_normal_paths_before_emitting_code() {
        for path in [
            "",
            "fake.json",
            "/fake/../schema.json",
            "/fake/./schema.json",
            "/fake//schema.json",
        ] {
            let a = SchemaDeclaration {
                path: Path::new(path),
                ..schema("fake_a")
            };
            assert_eq!(
                quote_validators(&selected(), &[a, schema("fake_b")], HELPERS)
                    .expect_err("non-normal schema path"),
                "non-normal staged schema path: fake_a",
                "{path:?}"
            );
        }
    }

    #[test]
    fn fake_quote_order_is_stable_and_an_empty_lane_emits_nothing() {
        let a = schema("fake_a");
        let b = schema("fake_b");
        let first = quote_validators(&selected(), &[a, b], HELPERS).expect("forward input order");
        let reverse = quote_validators(&selected(), &[b, a], HELPERS).expect("reverse input order");
        assert_eq!(first.items.to_string(), reverse.items.to_string());
        let empty = quote_validators(
            &Selection {
                members: vec![],
                schema_owners: vec![],
            },
            &[],
            HelperAvailability {
                uuid: false,
                hostname: false,
            },
        )
        .expect("empty lane");
        assert!(empty.items.is_empty());
        assert!(empty.owners.is_empty());
        assert!(empty.required_formats.is_empty());
    }
}

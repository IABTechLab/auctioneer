use proc_macro2::TokenStream;
use quote::{ToTokens as _, quote, quote_spanned};
use std::collections::BTreeSet;
use syn::ext::IdentExt as _;
use syn::meta::ParseNestedMeta;
use syn::token::Paren;
use syn::{Attribute, Data, DeriveInput, Field, Fields, LitStr, Path, Token};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "OpenRtbFieldMetadata does not support generics or where clauses",
        ));
    }
    let Data::Struct(model) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "OpenRtbFieldMetadata requires a named-field struct",
        ));
    };
    let Fields::Named(fields) = &model.fields else {
        return Err(syn::Error::new_spanned(
            input,
            "OpenRtbFieldMetadata requires a named-field struct",
        ));
    };
    let (interface, wire_name) = model_options(input)?;
    serde_options(&input.attrs, false)?;
    let entries = fields
        .named
        .iter()
        .map(|field| field_entry(field, &interface))
        .collect::<syn::Result<Vec<_>>>()?;
    let ident = &input.ident;
    let cfg = cfg_attributes(&input.attrs);
    // This source label is diagnostic only; Model::new::<Self> supplies identity.
    let rust_name = quote_spanned! {ident.span()=>
        ::core::concat!(
            ::core::module_path!(), "::", ::core::stringify!(#ident), "@",
            ::core::file!(), ":", ::core::line!(), ":", ::core::column!()
        )
    };
    Ok(quote! {
        #(#cfg)*
        impl #interface::FieldMetadata for #ident {
            const STORAGE: #interface::Storage = #interface::Storage::Typed;
            const MODEL: ::core::option::Option<&'static #interface::Model> =
                ::core::option::Option::Some(&#interface::Model::new::<Self>(
                    #wire_name,
                    #rust_name,
                    &[#(#entries,)*],
                ));
        }
    })
}

fn model_options(input: &DeriveInput) -> syn::Result<(Path, LitStr)> {
    let mut interface = None;
    let mut name = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("openrtb_metadata"))
    {
        let mut nonempty = false;
        attr.parse_nested_meta(|meta| {
            nonempty = true;
            if meta.path.is_ident("crate_path") {
                if interface.is_some() {
                    return Err(meta.error("duplicate openrtb_metadata option `crate_path`"));
                }
                interface = Some(meta.value()?.parse::<Path>()?);
            } else if meta.path.is_ident("name") {
                if name.is_some() {
                    return Err(meta.error("duplicate openrtb_metadata option `name`"));
                }
                name = Some(meta.value()?.parse::<LitStr>()?);
            } else {
                return Err(meta.error(format!(
                    "unknown openrtb_metadata option `{}`",
                    meta.path.to_token_stream(),
                )));
            }
            Ok(())
        })?;
        if !nonempty {
            return Err(syn::Error::new_spanned(
                attr,
                "openrtb_metadata requires at least one option",
            ));
        }
    }
    let wire_name =
        name.unwrap_or_else(|| LitStr::new(&input.ident.unraw().to_string(), input.ident.span()));
    validate_name(&wire_name, "object")?;
    Ok((
        interface.unwrap_or_else(|| syn::parse_quote!(::auctioneer_openrtb::field_metadata)),
        wire_name,
    ))
}

#[derive(Default)]
struct SerdeOptions {
    rename: Option<LitStr>,
    flattened: bool,
}

fn serde_options(attrs: &[Attribute], on_field: bool) -> syn::Result<SerdeOptions> {
    let mut options = SerdeOptions::default();
    let mut seen = BTreeSet::new();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            let option = meta.path.to_token_stream().to_string();
            if !seen.insert(option.clone()) {
                return Err(meta.error(format!("duplicate serde option `{option}`")));
            }
            if meta.path.is_ident("default") {
                if meta.input.peek(Token![=]) {
                    meta.value()?.parse::<LitStr>()?;
                } else {
                    require_flag(&meta)?;
                }
            } else if on_field && meta.path.is_ident("skip_serializing_if") {
                meta.value()?.parse::<LitStr>()?;
            } else if on_field && meta.path.is_ident("rename") {
                if meta.input.peek(Paren) {
                    return Err(meta.error(
                        "directional serde `rename` is unsupported; use `rename = \"...\"`",
                    ));
                }
                let name = meta.value()?.parse::<LitStr>()?;
                validate_name(&name, "field")?;
                options.rename = Some(name);
            } else if on_field && meta.path.is_ident("flatten") {
                require_flag(&meta)?;
                options.flattened = true;
            } else {
                let location = if on_field { "field" } else { "container" };
                return Err(meta.error(format!("unsupported {location} serde option `{option}`")));
            }
            Ok(())
        })?;
    }
    Ok(options)
}

fn require_flag(meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
    if meta.input.peek(Token![=]) || meta.input.peek(Paren) {
        return Err(meta.error(format!(
            "serde `{}` must be a bare flag",
            meta.path.to_token_stream()
        )));
    }
    Ok(())
}

fn field_entry(field: &Field, interface: &Path) -> syn::Result<TokenStream> {
    if let Some(attr) = field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("openrtb_metadata"))
    {
        return Err(syn::Error::new_spanned(
            attr,
            "openrtb_metadata is only supported on the model",
        ));
    }
    let Some(ident) = &field.ident else {
        return Err(syn::Error::new_spanned(
            field,
            "OpenRtbFieldMetadata requires named fields",
        ));
    };
    let options = serde_options(&field.attrs, true)?;
    let rust_name = LitStr::new(&ident.unraw().to_string(), ident.span());
    // A flattened map has no named wire slot; its name is diagnostic only.
    let name = if options.flattened {
        rust_name
    } else {
        options.rename.unwrap_or(rust_name)
    };
    validate_name(&name, "field")?;
    let flattened = options.flattened;
    let ty = &field.ty;
    let cfg = cfg_attributes(&field.attrs);
    Ok(quote! {
        #(#cfg)*
        #interface::Field::of::<#ty>(#name, #flattened)
    })
}

fn cfg_attributes(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    // rustc expands cfg_attr and removes inactive fields before invoking a derive.
    attrs.iter().filter(|attr| attr.path().is_ident("cfg"))
}

fn validate_name(name: &LitStr, kind: &str) -> syn::Result<()> {
    let value = name.value();
    let mut bytes = value.bytes();
    let valid_start = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
    if !valid_start
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || !value.bytes().any(|byte| byte != b'_')
    {
        return Err(syn::Error::new_spanned(
            name,
            format!(
                "invalid metadata {kind} name: expected [A-Za-z_][A-Za-z0-9_]* with a non-underscore character",
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::expand;
    use proc_macro2::TokenStream;
    use quote::{ToTokens as _, quote};

    fn expansion(input: TokenStream) -> TokenStream {
        let declaration = syn::parse2(input).expect("valid derive input");
        let output = expand(&declaration).expect("metadata expansion");
        syn::parse2::<syn::ItemImpl>(output.clone()).expect("valid generated implementation");
        output
    }

    fn refuses(input: TokenStream, expected: &str) {
        let declaration = syn::parse2(input).expect("valid derive input");
        let error = expand(&declaration).expect_err("unsupported metadata input");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?}, got {error}"
        );
    }

    #[test]
    fn emits_constants_with_self_constructor_and_diagnostic_rust_name() {
        let output = expansion(quote! {
            struct Request { id: String, ext: Option<Alias> }
        });
        let expected = quote! {
            impl ::auctioneer_openrtb::field_metadata::FieldMetadata for Request {
                const STORAGE: ::auctioneer_openrtb::field_metadata::Storage =
                    ::auctioneer_openrtb::field_metadata::Storage::Typed;
                const MODEL: ::core::option::Option<&'static ::auctioneer_openrtb::field_metadata::Model> =
                    ::core::option::Option::Some(&::auctioneer_openrtb::field_metadata::Model::new::<Self>(
                        "Request",
                        ::core::concat!(
                            ::core::module_path!(), "::", ::core::stringify!(Request), "@",
                            ::core::file!(), ":", ::core::line!(), ":", ::core::column!()
                        ),
                        &[
                            ::auctioneer_openrtb::field_metadata::Field::of::<String>("id", false),
                            ::auctioneer_openrtb::field_metadata::Field::of::<Option<Alias>>("ext", false),
                        ],
                    ));
            }
        };
        let actual_impl = syn::parse2::<syn::ItemImpl>(output).expect("generated implementation");
        let expected_impl =
            syn::parse2::<syn::ItemImpl>(expected).expect("expected implementation");
        assert_eq!(
            actual_impl.into_token_stream().to_string(),
            expected_impl.into_token_stream().to_string()
        );
    }

    #[test]
    fn supports_explicit_interface_and_catalogue_name() {
        let output = expansion(quote! {
            #[openrtb_metadata(crate_path = crate::field_metadata, name = "EID")]
            struct Eid { source: String }
        })
        .to_string();
        assert!(output.contains("impl crate :: field_metadata :: FieldMetadata for Eid"));
        assert!(output.contains("Model :: new :: < Self > (\"EID\""));
        assert!(!output.contains("auctioneer_openrtb"));
    }

    #[test]
    fn supports_helper_options_in_separate_attributes_and_absolute_paths() {
        let output = expansion(quote! {
            #[openrtb_metadata(name = "UID")]
            #[openrtb_metadata(crate_path = ::renamed::metadata)]
            struct Uid {}
        })
        .to_string();
        assert!(output.contains("impl :: renamed :: metadata :: FieldMetadata for Uid"));
        assert!(output.contains("Model :: new :: < Self > (\"UID\""));
    }

    #[test]
    fn unraws_default_names_and_honors_symmetric_field_renames() {
        let output = expansion(quote! {
            struct r#type {
                r#match: String,
                #[serde(rename = "type")]
                kind: i32,
            }
        })
        .to_string();
        assert!(output.contains("Model :: new :: < Self > (\"type\""));
        assert!(output.contains("Field :: of :: < String > (\"match\" , false)"));
        assert!(output.contains("Field :: of :: < i32 > (\"type\" , false)"));
    }

    #[test]
    fn accepts_defaults_and_omission_conditions_without_omitting_fields() {
        let output = expansion(quote! {
            #[serde(default = "defaults")]
            struct Request {
                #[serde(default, skip_serializing_if = "Option::is_none")]
                first: Option<String>,
                #[serde(default = "empty")]
                second: String,
            }
        })
        .to_string();
        assert!(output.contains("(\"first\" , false)"));
        assert!(output.contains("(\"second\" , false)"));
        expansion(quote! { #[serde(default)] struct Empty {} });
    }

    #[test]
    fn flatten_uses_the_rust_field_name_only_for_diagnostics() {
        let output = expansion(quote! {
            struct Request {
                #[serde(flatten, rename = "not_a_wire_key")]
                r#type: MapAlias,
            }
        })
        .to_string();
        assert!(output.contains("Field :: of :: < MapAlias > (\"type\" , true)"));
        assert!(!output.contains("not_a_wire_key"));
        assert_eq!(output.matches("Field :: of").count(), 1);
    }

    #[test]
    fn passes_actual_types_to_trait_resolution_without_spelling_classification() {
        let output = expansion(quote! {
            struct Request {
                first: shadow::RawValue,
                second: JsonField<Option<Box<Alias<u32>>>>,
                third: <Owner as Trait>::Associated,
                #[serde(flatten)]
                other: RenamedMap,
            }
        })
        .to_string();
        assert!(output.contains("Field :: of :: < shadow :: RawValue >"));
        let ty: syn::Type = syn::parse_quote!(JsonField<Option<Box<Alias<u32>>>>);
        assert!(output.contains(&quote!(Field::of::<#ty>).to_string()));
        assert!(output.contains("Field :: of :: < < Owner as Trait > :: Associated >"));
        assert!(output.contains("Field :: of :: < RenamedMap > (\"other\" , true)"));
        assert_eq!(output.matches("Storage :: Typed").count(), 1);
        assert!(!output.contains("RawJson"));
        assert!(!output.contains("JsonMap"));
    }

    #[test]
    fn preserves_field_order_and_cfg_without_evaluating_conditions() {
        let output = expansion(quote! {
            #[cfg(any(feature = "one", feature = "two"))]
            struct Request {
                #[cfg(feature = "one")]
                #[serde(rename = "wire")]
                first: First,
                #[cfg(not(feature = "one"))]
                #[serde(rename = "wire")]
                second: Second,
            }
        })
        .to_string();
        assert!(output.contains("# [cfg (any (feature = \"one\" , feature = \"two\"))] impl"));
        assert!(output.contains("# [cfg (feature = \"one\")] :: auctioneer_openrtb"));
        assert!(output.contains("# [cfg (not (feature = \"one\"))] :: auctioneer_openrtb"));
        assert!(
            output.find("of :: < First >").expect("first field")
                < output.find("of :: < Second >").expect("second field")
        );
    }

    #[test]
    fn leaves_duplicate_names_and_catch_alls_to_the_cfg_aware_constructor() {
        let output = expansion(quote! {
            struct Request {
                #[serde(rename = "same")]
                first: String,
                #[serde(rename = "same")]
                second: String,
                #[cfg(feature = "one")]
                #[serde(flatten)]
                other: MapAlias,
                #[cfg(not(feature = "one"))]
                #[serde(flatten)]
                alternative: MapAlias,
            }
        })
        .to_string();
        assert_eq!(output.matches("(\"same\" , false)").count(), 2);
        assert_eq!(output.matches(", true)").count(), 2);
    }

    #[test]
    fn refuses_non_named_struct_model_forms() {
        for input in [
            quote! { enum Request { One } },
            quote! { union Request { first: i32, second: u32 } },
            quote! { struct Request(String); },
            quote! { struct Request; },
        ] {
            refuses(input, "requires a named-field struct");
        }
    }

    #[test]
    fn refuses_generic_models_and_where_clauses() {
        for input in [
            quote! { struct Request<T> { value: T } },
            quote! { struct Request<'a> { value: &'a str } },
            quote! { struct Request<const N: usize> { value: [u8; N] } },
            quote! { struct Request where String: Clone { value: String } },
        ] {
            refuses(input, "does not support generics or where clauses");
        }
    }

    #[test]
    fn leaves_owned_projection_and_alias_equally_to_trait_resolution() {
        for tokens in [
            quote! { String },
            quote! { OwnedAlias },
            quote! { <&'static () as Wire>::Owned },
            quote! { Option<<&'static () as Wire>::Owned> },
        ] {
            let ty: syn::Type = syn::parse2(tokens).expect("valid field type");
            let output = expansion(quote! { struct Request { value: #ty } }).to_string();
            let expected = quote! {
                ::auctioneer_openrtb::field_metadata::Field::of::<#ty>("value", false)
            };
            assert!(output.contains(&expected.to_string()));
        }
    }

    #[test]
    fn leaves_reference_field_rejection_to_compiler_trait_resolution() {
        for ty in [quote! { &'static str }, quote! { Option<&'static str> }] {
            let parsed_type: syn::Type = syn::parse2(ty).expect("valid field type");
            let output = expansion(quote! { struct Request { value: #parsed_type } }).to_string();
            let expected = quote! {
                ::auctioneer_openrtb::field_metadata::Field::of::<#parsed_type>("value", false)
            };
            assert!(output.contains(&expected.to_string()));
        }
    }

    #[test]
    fn refuses_unknown_helper_options_and_field_helpers() {
        refuses(
            quote! {
                #[openrtb_metadata(storage = "raw")]
                struct Request {}
            },
            "unknown openrtb_metadata option `storage`",
        );
        refuses(
            quote! {
                struct Request { #[openrtb_metadata(name = "value")] value: String }
            },
            "openrtb_metadata is only supported on the model",
        );
    }

    #[test]
    fn refuses_duplicate_helper_options() {
        refuses(
            quote! {
                #[openrtb_metadata(name = "One")]
                #[openrtb_metadata(name = "Two")]
                struct Request {}
            },
            "duplicate openrtb_metadata option `name`",
        );
        refuses(
            quote! {
                #[openrtb_metadata(crate_path = crate::one, crate_path = crate::two)]
                struct Request {}
            },
            "duplicate openrtb_metadata option `crate_path`",
        );
    }

    #[test]
    fn refuses_invalid_object_and_field_names() {
        for name in [
            "",
            "_",
            "___",
            "two words",
            "1value",
            "value-name",
            "caf\u{e9}",
        ] {
            refuses(
                quote! {
                    #[openrtb_metadata(name = #name)]
                    struct Request {}
                },
                "invalid metadata object name",
            );
            refuses(
                quote! {
                    struct Request { #[serde(rename = #name)] value: String }
                },
                "invalid metadata field name",
            );
        }
        refuses(quote! { struct __ {} }, "invalid metadata object name");
        refuses(
            quote! { struct Request { __: String } },
            "invalid metadata field name",
        );
    }

    #[test]
    fn accepts_valid_identifier_shaped_wire_names() {
        for name in ["EID", "_value", "value_2"] {
            expansion(quote! {
                #[openrtb_metadata(name = #name)]
                struct Request { #[serde(rename = #name)] value: String }
            });
        }
    }

    #[test]
    fn refuses_empty_and_malformed_helper_values() {
        refuses(
            quote! {
                #[openrtb_metadata()]
                struct Request {}
            },
            "requires at least one option",
        );
        for input in [
            quote! { #[openrtb_metadata(name)] struct Request {} },
            quote! { #[openrtb_metadata(name = 42)] struct Request {} },
            quote! { #[openrtb_metadata(crate_path = "crate::metadata")] struct Request {} },
            quote! { #[openrtb_metadata(crate_path = 42)] struct Request {} },
        ] {
            let declaration = syn::parse2(input).expect("valid input with malformed helper");
            expand(&declaration).expect_err("malformed metadata helper");
        }
    }

    #[test]
    fn refuses_directional_renames_even_if_both_directions_agree() {
        for rename in [
            quote! { rename(serialize = "wire") },
            quote! { rename(deserialize = "wire") },
            quote! { rename(serialize = "wire", deserialize = "wire") },
        ] {
            refuses(
                quote! {
                    struct Request { #[serde(#rename)] value: String }
                },
                "directional serde `rename` is unsupported",
            );
        }
    }

    #[test]
    fn refuses_field_skips_codecs_and_other_unsupported_serde_options() {
        for option in [
            quote! { skip },
            quote! { skip_serializing },
            quote! { skip_deserializing },
            quote! { serialize_with = "codec" },
            quote! { deserialize_with = "codec" },
            quote! { with = "codec" },
            quote! { rename_all = "camelCase" },
            quote! { borrow },
            quote! { alias = "other" },
            quote! { unknown },
        ] {
            refuses(
                quote! {
                    struct Request { #[serde(#option)] value: String }
                },
                "unsupported field serde option",
            );
        }
    }

    #[test]
    fn refuses_container_wire_transformations_and_unknown_options() {
        for option in [
            quote! { rename_all = "camelCase" },
            quote! { transparent },
            quote! { from = "Wire" },
            quote! { try_from = "Wire" },
            quote! { into = "Wire" },
            quote! { tag = "kind" },
            quote! { untagged },
            quote! { skip },
            quote! { unknown },
        ] {
            refuses(
                quote! { #[serde(#option)] struct Request { value: String } },
                "unsupported container serde option",
            );
        }
    }

    #[test]
    fn refuses_duplicate_serde_options_across_attributes() {
        for option in [
            quote! { rename = "wire" },
            quote! { flatten },
            quote! { default },
            quote! { skip_serializing_if = "missing" },
        ] {
            refuses(
                quote! {
                    struct Request {
                        #[serde(#option)]
                        #[serde(#option)]
                        value: MapAlias,
                    }
                },
                "duplicate serde option",
            );
        }
    }

    #[test]
    fn refuses_malformed_supported_serde_options() {
        for option in [
            quote! { flatten = true },
            quote! { flatten(other) },
            quote! { default(other) },
            quote! { default = 42 },
            quote! { skip_serializing_if },
            quote! { skip_serializing_if = 42 },
            quote! { rename },
            quote! { rename = 42 },
        ] {
            let input = syn::parse2(quote! {
                struct Request { #[serde(#option)] value: String }
            })
            .expect("valid input with malformed serde option");
            expand(&input).expect_err("malformed serde option");
        }
    }
}

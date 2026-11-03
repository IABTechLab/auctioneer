//! Build-only Rust generation from bidder-owned settings declarations.

use super::build_catalog::{Manifest, model::SettingKind};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::Ident;

/// Reference-only declarations describe types, never runtime eligibility.
/// Render bidder-owned settings types and their bounded accessors.
///
/// # Errors
/// Returns a syntax error if an owning key or field is not a Rust identifier.
pub fn render(manifests: &BTreeMap<String, Manifest>) -> Result<String, syn::Error> {
    let declarations: Vec<_> = manifests
        .values()
        .filter(|row| row.settings.is_some())
        .collect();
    let modules = declarations
        .iter()
        .map(|row| implementation(row))
        .collect::<Result<Vec<_>, _>>()?;
    let keys = declarations
        .iter()
        .map(|row| row.key.as_str())
        .collect::<Vec<_>>();
    let names = keys
        .iter()
        .map(|key| syn::parse_str::<Ident>(key))
        .collect::<Result<Vec<_>, _>>()?;
    let variants = (0..names.len())
        .map(|index| format_ident!("V{index}"))
        .collect::<Vec<_>>();
    let accessors = names
        .iter()
        .zip(&variants)
        .map(|(name, variant)| accessor(name, variant, names.len() == 1))
        .collect::<Vec<_>>();
    let implementation_key = if names.is_empty() {
        quote! {match self.0 {}}
    } else {
        quote! { match &self.0 { #(ImplementationSettings::#variants(_) => #keys,)* } }
    };
    let object_decoder = object_decoder();
    Ok(quote! {
        use ::std::fmt::{Debug, Formatter, Result as FmtResult};
        #object_decoder
        #(#modules)*

        #[derive(Clone, ::serde::Serialize)]
        #[serde(transparent)]
        pub struct BidderSettings(ImplementationSettings);

        #[derive(Clone, ::serde::Deserialize, ::serde::Serialize)]
        #[serde(tag = "implementation", content = "values", deny_unknown_fields)]
        enum ImplementationSettings {
            #(#[serde(rename = #keys)] #variants(#names::Settings),)*
        }

        #[expect(clippy::missing_trait_methods, reason = "serde's default in-place decoding replaces the typed settings")]
        impl<'de> ::serde::Deserialize<'de> for BidderSettings {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserialize_object(deserializer).map(Self)
            }
        }

        impl BidderSettings {
            #[must_use]
            pub fn implementation_key(&self) -> &'static str { #implementation_key }
            #(#accessors)*
        }

        impl ::validator::Validate for BidderSettings {
            fn validate(&self) -> Result<(), ::validator::ValidationErrors> {
                use crate::bidder_settings::ensure_limits;
                ensure_limits(self).map_err(|reason| {
                    let mut errors = ::validator::ValidationErrors::new();
                    errors.add("values", ::validator::ValidationError::new(reason));
                    errors
                })
            }
        }

        impl Debug for BidderSettings {
            fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
                f.debug_struct("BidderSettings")
                    .field("implementation", &self.implementation_key())
                    .field("values", &"<redacted>").finish()
            }
        }
    }.to_string())
}

fn accessor(name: &Ident, variant: &Ident, single: bool) -> TokenStream {
    let method = format_ident!("as_{name}");
    let body = if single {
        quote! {
            let ImplementationSettings::#variant(settings) = &self.0;
            Some(settings)
        }
    } else {
        quote! {
            if let ImplementationSettings::#variant(settings) = &self.0 {
                Some(settings)
            } else {
                None
            }
        }
    };
    quote! {
        #[must_use]
        pub fn #method(&self) -> Option<&#name::Settings> { #body }
    }
}

fn implementation(manifest: &Manifest) -> Result<TokenStream, syn::Error> {
    let name = syn::parse_str::<Ident>(&manifest.key)?;
    let Some(schema) = &manifest.settings else {
        return Ok(TokenStream::new());
    };
    let fields = schema
        .fields
        .keys()
        .map(|field| syn::parse_str::<Ident>(field))
        .collect::<Result<Vec<_>, _>>()?;
    let types = schema
        .fields
        .values()
        .map(|kind| match kind {
            SettingKind::OptionalString | SettingKind::OptionalSecretReference => quote! {String},
            SettingKind::OptionalI64 => quote! {i64},
        })
        .collect::<Vec<_>>();
    let decoders = schema
        .fields
        .values()
        .map(|kind| match kind {
            SettingKind::OptionalSecretReference => quote! {
                #[serde(deserialize_with = "crate::bidder_settings::deserialize_optional_secret_reference")]
            },
            SettingKind::OptionalString | SettingKind::OptionalI64 => TokenStream::new(),
        })
        .collect::<Vec<_>>();
    let getters = fields
        .iter()
        .zip(schema.fields.values())
        .map(|(field, kind)| {
            let (ty, project) = match kind {
                SettingKind::OptionalString | SettingKind::OptionalSecretReference => {
                    (quote! {&'a str}, quote! {.as_deref()})
                }
                SettingKind::OptionalI64 => (quote! {i64}, TokenStream::new()),
            };
            quote! {
                #[must_use]
                pub fn #field(&self) -> Option<#ty> {
                    if let Some(value) = self.requested.and_then(|settings| settings.#field #project) {
                        return Some(value);
                    }
                    self.base?.#field #project
                }
            }
        })
        .collect::<Vec<_>>();
    let settings_label = format!("{}::Settings(<redacted>)", manifest.key);
    let getter_impl = if schema.fields.values().any(|kind| {
        matches!(
            kind,
            SettingKind::OptionalString | SettingKind::OptionalSecretReference
        )
    }) {
        quote! {impl<'a> SettingsRef<'a> { #(#getters)* }}
    } else {
        quote! {impl SettingsRef<'_> { #(#getters)* }}
    };
    let ref_label = format!("{}::SettingsRef(<redacted>)", manifest.key);
    Ok(quote! {
        pub mod #name {
            use ::std::fmt::{Debug, Formatter, Result as FmtResult};
            #[derive(Clone, Default, ::serde::Serialize)]
            #[serde(deny_unknown_fields)]
            pub struct Settings {
                #(#[serde(default, skip_serializing_if = "Option::is_none")] pub #fields: Option<#types>,)*
            }

            #[expect(clippy::missing_trait_methods, reason = "serde's default in-place decoding replaces the typed settings")]
            impl<'de> ::serde::Deserialize<'de> for Settings {
                fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                    #[derive(::serde::Deserialize)]
                    #[serde(deny_unknown_fields)]
                    struct Fields { #(#[serde(default)] #decoders #fields: Option<#types>,)* }
                    let fields: Fields = super::deserialize_object(deserializer)?;
                    Ok(Self {#(#fields: fields.#fields,)*})
                }
            }

            impl Debug for Settings {
                fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult { f.write_str(#settings_label) }
            }

            #[derive(Clone, Copy, Default)]
            pub struct SettingsRef<'a> {
                requested: Option<&'a Settings>,
                base: Option<&'a Settings>,
            }

            #[must_use]
            pub fn resolve<'a>(requested: Option<&'a Settings>, base: Option<&'a Settings>) -> SettingsRef<'a> {
                SettingsRef { requested, base }
            }

            #getter_impl

            impl Debug for SettingsRef<'_> {
                fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult { f.write_str(#ref_label) }
            }
        }
    })
}

fn object_decoder() -> TokenStream {
    quote! {
        fn deserialize_object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
        where D: ::serde::Deserializer<'de>, T: ::serde::Deserialize<'de> {
            use ::std::marker::PhantomData;
            use ::serde::de::{MapAccess, Visitor, value::MapAccessDeserializer};
            struct ObjectVisitor<T>(PhantomData<T>);
            #[expect(clippy::missing_trait_methods, reason = "all non-map visitor defaults must reject their input")]
            impl<'de, T: ::serde::Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
                type Value = T;
                fn expecting(&self, formatter: &mut Formatter<'_>) -> FmtResult {
                    formatter.write_str("a bidder settings object")
                }
                fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
                    T::deserialize(MapAccessDeserializer::new(map))
                }
            }
            deserializer.deserialize_map(ObjectVisitor(PhantomData))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::build_catalog::Manifest;
    use quote::{ToTokens as _, quote};
    use std::collections::BTreeMap;
    use syn::{Item, Visibility};

    fn declaration(key: &str, fields: &str) -> Manifest {
        toml::from_str(&format!("key='{key}'\nfactory=false\n[settings.fields]\n{fields}\n[provenance]\norigin='local'\n")).unwrap()
    }

    #[test]
    fn arbitrary_declarations_generate_typed_fields_without_a_family_list() {
        let manifest = declaration(
            "fake_future_bidder_9",
            "token='optional_string'\nsource='optional_i64'\nreference='optional_secret_reference'",
        );
        let parsed =
            syn::parse_file(&render(&BTreeMap::from([(manifest.key.clone(), manifest)])).unwrap())
                .unwrap();
        let module = parsed
            .items
            .iter()
            .find_map(|item| {
                if let Item::Mod(module) = item
                    && module.ident == "fake_future_bidder_9"
                {
                    Some(module)
                } else {
                    None
                }
            })
            .unwrap();
        let fields = module
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .find_map(|item| {
                if let Item::Struct(structure) = item
                    && structure.ident == "Settings"
                {
                    Some(&structure.fields)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(fields.len(), 3);
        assert_eq!(
            fields
                .iter()
                .map(|field| field.ident.as_ref().unwrap().to_string())
                .collect::<Vec<_>>(),
            ["reference", "source", "token"]
        );
        assert_eq!(
            fields
                .iter()
                .map(|field| field.ty.to_token_stream().to_string())
                .collect::<Vec<_>>(),
            ["Option < String >", "Option < i64 >", "Option < String >"]
        );
    }

    #[test]
    fn secret_reference_only_schema_borrows_with_a_shared_admission_decoder() {
        let manifest = declaration(
            "fake_reference_probe",
            "reference='optional_secret_reference'",
        );
        let parsed =
            syn::parse_file(&render(&BTreeMap::from([(manifest.key.clone(), manifest)])).unwrap())
                .unwrap();
        let module = parsed
            .items
            .iter()
            .find_map(|item| {
                if let Item::Mod(module) = item
                    && module.ident == "fake_reference_probe"
                {
                    Some(&module.content.as_ref().unwrap().1)
                } else {
                    None
                }
            })
            .unwrap();
        let view = module
            .iter()
            .find_map(|item| {
                if let Item::Impl(implementation) = item
                    && implementation.trait_.is_none()
                {
                    Some(implementation)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(view.generics.lifetimes().count(), 1);
        let syn::ImplItem::Fn(getter) = &view.items[0] else {
            panic!("borrowed view must contain a getter");
        };
        assert_eq!(getter.sig.ident, "reference");
        assert_eq!(
            getter.sig.output.to_token_stream().to_string(),
            quote! { -> Option<&'a str> }.to_string(),
        );
        let fields = module
            .iter()
            .filter_map(|item| {
                if let Item::Impl(implementation) = item {
                    Some(&implementation.items)
                } else {
                    None
                }
            })
            .flatten()
            .filter_map(|item| {
                if let syn::ImplItem::Fn(method) = item {
                    Some(&method.block.stmts)
                } else {
                    None
                }
            })
            .flatten()
            .find_map(|statement| {
                if let syn::Stmt::Item(Item::Struct(fields)) = statement
                    && fields.ident == "Fields"
                {
                    Some(&fields.fields)
                } else {
                    None
                }
            })
            .unwrap();
        let field = fields.iter().next().unwrap();
        let mut defaulted = false;
        let mut decoder = None;
        for attribute in &field.attrs {
            attribute
                .parse_nested_meta(|meta| {
                    if meta.path.is_ident("default") {
                        defaulted = true;
                    } else {
                        assert!(meta.path.is_ident("deserialize_with"));
                        decoder = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                    }
                    Ok(())
                })
                .unwrap();
        }
        assert!(defaulted);
        assert_eq!(
            decoder.as_deref(),
            Some("crate::bidder_settings::deserialize_optional_secret_reference"),
        );
    }

    #[test]
    fn opaque_wrapper_and_private_ordinals_cannot_become_public_mapping() {
        let manifest = declaration("fake_probe", "value='optional_string'");
        let parsed =
            syn::parse_file(&render(&BTreeMap::from([(manifest.key.clone(), manifest)])).unwrap())
                .unwrap();
        let wrapper = parsed
            .items
            .iter()
            .find_map(|item| {
                if let Item::Struct(structure) = item
                    && structure.ident == "BidderSettings"
                {
                    Some(structure)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(matches!(wrapper.vis, Visibility::Public(_)));
        assert_eq!(wrapper.fields.len(), 1);
        assert!(
            wrapper
                .fields
                .iter()
                .all(|field| matches!(field.vis, Visibility::Inherited))
        );
        let state = parsed
            .items
            .iter()
            .find_map(|item| {
                if let Item::Enum(enumeration) = item
                    && enumeration.ident == "ImplementationSettings"
                {
                    Some(enumeration)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(matches!(state.vis, Visibility::Inherited));
        assert_eq!(
            state
                .variants
                .iter()
                .map(|variant| variant.ident.to_string())
                .collect::<Vec<_>>(),
            ["V0"]
        );
    }

    #[test]
    fn empty_catalog_generates_a_valid_uninhabited_wrapper_without_waivers() {
        let source = render(&BTreeMap::new()).unwrap();
        let parsed = syn::parse_file(&source).unwrap();
        let state = parsed
            .items
            .iter()
            .find_map(|item| {
                if let Item::Enum(enumeration) = item {
                    Some(enumeration)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(state.variants.is_empty());
        assert!(!source.contains("dead_code"));
        assert!(!source.contains("non_camel_case_types"));
    }

    #[test]
    fn rendering_is_deterministic_and_does_not_promote_declarations() {
        let a = declaration("fake_a__b", "z='optional_string'\na='optional_i64'");
        let b = declaration("fake_a_b", "y='optional_i64'");
        let first = render(&BTreeMap::from([(a.key.clone(), a), (b.key.clone(), b)])).unwrap();
        let reordered_a = declaration("fake_a__b", "a='optional_i64'\nz='optional_string'");
        let reordered_b = declaration("fake_a_b", "y='optional_i64'");
        let second = render(&BTreeMap::from([
            (reordered_b.key.clone(), reordered_b),
            (reordered_a.key.clone(), reordered_a),
        ]))
        .unwrap();
        assert_eq!(first, second);
        for forbidden in ["factory", "endpoint", "serde_json", "untagged", "dead_code"] {
            assert!(!first.contains(forbidden));
        }
        assert_eq!(first.matches("deny_unknown_fields").count(), 5);
    }

    #[test]
    fn invalid_rust_names_return_a_structured_error_instead_of_panicking() {
        for manifest in [
            declaration("not-rust", "source='optional_i64'"),
            declaration("fake_probe", "'not-rust'='optional_string'"),
        ] {
            let error = render(&BTreeMap::from([(manifest.key.clone(), manifest)])).unwrap_err();
            assert_eq!(error.to_string(), "unexpected token");
        }
    }
}

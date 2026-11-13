//! Build-only Rust generation from bidder-owned endpoint declarations.

use super::build_catalog::Manifest;
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use std::collections::BTreeMap;
use syn::Ident;

/// Reference-only declarations describe bindings, never runtime eligibility.
///
/// # Errors
/// Returns a syntax error if an owning key or field is not a Rust identifier.
pub fn render(manifests: &BTreeMap<String, Manifest>) -> Result<String, syn::Error> {
    let modules = manifests
        .values()
        .map(implementation)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { #(#modules)* }.to_string())
}

fn implementation(manifest: &Manifest) -> Result<TokenStream, syn::Error> {
    let Some(schema) = &manifest.endpoint else {
        return Ok(TokenStream::new());
    };
    let name = syn::parse_str::<Ident>(&manifest.key)?;
    let fields = schema
        .fields
        .keys()
        .map(|field| syn::parse_str::<Ident>(field))
        .collect::<Result<Vec<_>, _>>()?;
    let placeholders = schema.fields.values();
    let count = Literal::usize_unsuffixed(fields.len());
    Ok(quote! {
        pub mod #name {
            pub struct Values<'a> {
                #(pub #fields: &'a str,)*
            }

            impl<'a> Values<'a> {
                #[must_use]
                pub const fn as_pairs(&self) -> [(&'static str, &'a str); #count] {
                    [#((#placeholders, self.#fields),)*]
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::build_catalog::Manifest;
    use std::collections::BTreeMap;
    use syn::{
        Expr, GenericArgument, GenericParam, ImplItem, Item, PathArguments, Type, Visibility,
    };

    fn declaration(key: &str, fields: Option<&str>) -> Manifest {
        let endpoint = fields.map_or_else(String::new, |declaration| {
            format!("[endpoint.fields]\n{declaration}\n")
        });
        toml::from_str(&format!(
            "key='{key}'\nfactory=false\n{endpoint}[provenance]\norigin='local'\n"
        ))
        .unwrap()
    }

    fn arbitrary_declaration() -> Manifest {
        declaration(
            "fake_future_bidder_9",
            Some("z_token='Alpha9'\na_route='ZuluToken'"),
        )
    }

    fn generated(manifest: Manifest) -> syn::File {
        syn::parse_file(&render(&BTreeMap::from([(manifest.key.clone(), manifest)])).unwrap())
            .unwrap()
    }

    fn module_items(module: &syn::ItemMod) -> &[Item] {
        assert!(matches!(module.vis, Visibility::Public(_)));
        &module.content.as_ref().unwrap().1
    }

    fn assert_lifetime(generics: &syn::Generics) {
        assert_eq!(generics.params.len(), 1);
        let GenericParam::Lifetime(parameter) = &generics.params[0] else {
            panic!("expected the borrowed values lifetime");
        };
        assert_eq!(parameter.lifetime.ident, "a");
        assert!(parameter.bounds.is_empty());
        assert!(generics.where_clause.is_none());
    }

    fn assert_str_reference(ty: &Type, lifetime: &str) {
        let Type::Reference(reference) = ty else {
            panic!("expected a borrowed string");
        };
        assert!(reference.mutability.is_none());
        assert_eq!(reference.lifetime.as_ref().unwrap().ident, lifetime);
        let Type::Path(path) = reference.elem.as_ref() else {
            panic!("expected str");
        };
        assert!(path.qself.is_none());
        assert!(path.path.is_ident("str"));
    }

    #[test]
    fn arbitrary_family_generates_public_named_borrowed_fields() {
        let parsed = generated(arbitrary_declaration());
        assert_eq!(parsed.items.len(), 1);
        let Item::Mod(module) = &parsed.items[0] else {
            panic!("expected an implementation module");
        };
        assert_eq!(module.ident, "fake_future_bidder_9");
        let items = module_items(module);
        assert_eq!(items.len(), 2);
        let Item::Struct(values) = &items[0] else {
            panic!("expected Values");
        };
        assert_eq!(values.ident, "Values");
        assert!(matches!(values.vis, Visibility::Public(_)));
        assert_lifetime(&values.generics);
        let syn::Fields::Named(fields) = &values.fields else {
            panic!("bindings must require named fields");
        };
        assert_eq!(fields.named.len(), 2);
        assert_eq!(
            fields
                .named
                .iter()
                .map(|field| field.ident.as_ref().unwrap().to_string())
                .collect::<Vec<_>>(),
            ["a_route", "z_token"]
        );
        for field in &fields.named {
            assert!(matches!(field.vis, Visibility::Public(_)));
            assert_str_reference(&field.ty, "a");
        }
    }

    fn assert_values_implementation(implementation: &syn::ItemImpl) {
        assert!(implementation.trait_.is_none());
        assert_lifetime(&implementation.generics);
        let Type::Path(self_type) = implementation.self_ty.as_ref() else {
            panic!("expected Values<'a>");
        };
        assert!(self_type.qself.is_none());
        assert_eq!(self_type.path.segments.len(), 1);
        let segment = &self_type.path.segments[0];
        assert_eq!(segment.ident, "Values");
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            panic!("expected the Values lifetime argument");
        };
        assert_eq!(arguments.args.len(), 1);
        let GenericArgument::Lifetime(lifetime) = &arguments.args[0] else {
            panic!("expected the Values lifetime");
        };
        assert_eq!(lifetime.ident, "a");
    }

    #[test]
    fn as_pairs_is_a_const_shared_borrow_returning_a_fixed_borrowed_array() {
        let parsed = generated(arbitrary_declaration());
        let Item::Mod(module) = &parsed.items[0] else {
            panic!("expected an implementation module");
        };
        let Item::Impl(implementation) = &module_items(module)[1] else {
            panic!("expected the Values implementation");
        };
        assert_values_implementation(implementation);
        assert_eq!(implementation.items.len(), 1);
        let ImplItem::Fn(method) = &implementation.items[0] else {
            panic!("expected as_pairs");
        };
        assert_eq!(method.sig.ident, "as_pairs");
        assert!(matches!(method.vis, Visibility::Public(_)));
        assert!(method.sig.constness.is_some());
        assert!(method.sig.asyncness.is_none());
        assert!(matches!(method.sig.safety, syn::Safety::Default));
        assert!(method.sig.abi.is_none());
        assert!(method.sig.variadic.is_none());
        assert!(method.sig.generics.params.is_empty());
        assert!(method.sig.generics.where_clause.is_none());
        assert_eq!(method.attrs.len(), 1);
        assert!(
            matches!(&method.attrs[0].meta, syn::Meta::Path(path) if path.is_ident("must_use"))
        );
        assert_eq!(method.sig.inputs.len(), 1);
        let syn::FnArg::Receiver(receiver) = &method.sig.inputs[0] else {
            panic!("expected &self");
        };
        assert!(matches!(
            receiver.kind,
            syn::ReceiverKind::Reference(_, None, None)
        ));
        assert!(receiver.mutability.is_none());

        let syn::ReturnType::Type(_, result) = &method.sig.output else {
            panic!("expected the stack array return type");
        };
        let Type::Array(array) = result.as_ref() else {
            panic!("expected a fixed array");
        };
        let Expr::Lit(length_expression) = &array.len else {
            panic!("expected a literal array length");
        };
        let syn::Lit::Int(length) = &length_expression.lit else {
            panic!("expected an integer array length");
        };
        assert_eq!(length.base10_parse::<usize>().unwrap(), 2);
        assert!(length.suffix().is_empty());
        let Type::Tuple(pair) = array.elem.as_ref() else {
            panic!("expected placeholder and value pairs");
        };
        assert_eq!(pair.elems.len(), 2);
        assert_str_reference(&pair.elems[0], "static");
        assert_str_reference(&pair.elems[1], "a");
    }

    #[test]
    fn literal_pairs_preserve_distinct_field_to_placeholder_mappings() {
        let parsed = generated(arbitrary_declaration());
        let Item::Mod(module) = &parsed.items[0] else {
            panic!("expected an implementation module");
        };
        let Item::Impl(implementation) = &module_items(module)[1] else {
            panic!("expected the Values implementation");
        };
        let ImplItem::Fn(method) = &implementation.items[0] else {
            panic!("expected as_pairs");
        };
        assert_eq!(method.block.stmts.len(), 1);
        let syn::Stmt::Expr(Expr::Array(array), None) = &method.block.stmts[0] else {
            panic!("expected only a returned array literal");
        };
        let pairs = array
            .elems
            .iter()
            .map(|entry| {
                let Expr::Tuple(pair) = entry else {
                    panic!("expected a literal pair");
                };
                assert_eq!(pair.elems.len(), 2);
                let Expr::Lit(placeholder_expression) = &pair.elems[0] else {
                    panic!("expected a literal placeholder");
                };
                let syn::Lit::Str(placeholder) = &placeholder_expression.lit else {
                    panic!("expected a string placeholder");
                };
                let Expr::Field(value) = &pair.elems[1] else {
                    panic!("expected a direct field borrow");
                };
                let Expr::Path(base) = value.base.as_ref() else {
                    panic!("expected self as the field owner");
                };
                assert!(base.qself.is_none());
                assert!(base.path.is_ident("self"));
                let syn::Member::Named(field) = &value.member else {
                    panic!("expected a named field");
                };
                (placeholder.value(), field.to_string())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pairs,
            [
                ("ZuluToken".to_owned(), "a_route".to_owned()),
                ("Alpha9".to_owned(), "z_token".to_owned()),
            ]
        );
    }

    #[test]
    fn absent_declarations_emit_no_modules_or_shared_types() {
        assert!(render(&BTreeMap::new()).unwrap().is_empty());
        let undeclared = declaration("fake_undeclared_bidder", None);
        let mut manifests = BTreeMap::from([(undeclared.key.clone(), undeclared)]);
        assert!(render(&manifests).unwrap().is_empty());

        let declared = arbitrary_declaration();
        manifests.insert(declared.key.clone(), declared);
        let parsed = syn::parse_file(&render(&manifests).unwrap()).unwrap();
        assert_eq!(parsed.items.len(), 1);
        let Item::Mod(module) = &parsed.items[0] else {
            panic!("expected only the declared implementation");
        };
        assert_eq!(module.ident, "fake_future_bidder_9");
    }

    #[test]
    fn output_is_deterministic_in_catalog_and_field_key_order() {
        let a = declaration("fake_a__b", Some("z='Alpha9'\na='ZuluToken'"));
        let b = declaration("fake_a_b", Some("value='OtherToken'"));
        let first = render(&BTreeMap::from([(b.key.clone(), b), (a.key.clone(), a)])).unwrap();
        let reordered_a = declaration("fake_a__b", Some("a='ZuluToken'\nz='Alpha9'"));
        let reordered_b = declaration("fake_a_b", Some("value='OtherToken'"));
        let second = render(&BTreeMap::from([
            (reordered_a.key.clone(), reordered_a),
            (reordered_b.key.clone(), reordered_b),
        ]))
        .unwrap();
        assert_eq!(first, second);
        let parsed = syn::parse_file(&first).unwrap();
        let names = parsed
            .items
            .iter()
            .map(|item| {
                let Item::Mod(module) = item else {
                    panic!("expected only implementation modules");
                };
                module.ident.to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(names, ["fake_a__b", "fake_a_b"]);
    }

    #[test]
    fn generated_bindings_have_no_debug_derives_traits_or_owned_storage() {
        let parsed = generated(arbitrary_declaration());
        assert_eq!(parsed.items.len(), 1);
        assert!(parsed.attrs.is_empty());
        for item in &parsed.items {
            let Item::Mod(module) = item else {
                panic!("expected only implementation modules");
            };
            assert!(module.attrs.is_empty());
            for member in module_items(module) {
                if let Item::Struct(values) = member {
                    assert!(values.attrs.is_empty(), "Values must not derive Debug");
                    for field in &values.fields {
                        assert!(field.attrs.is_empty());
                        assert_str_reference(&field.ty, "a");
                    }
                } else if let Item::Impl(implementation) = member {
                    assert!(implementation.attrs.is_empty());
                    assert!(implementation.trait_.is_none(), "no Debug or other traits");
                } else {
                    panic!("unexpected shared type, import, registry or storage");
                }
            }
        }
    }

    #[test]
    fn invalid_rust_names_return_structured_errors_instead_of_panicking() {
        for manifest in [
            declaration("not-rust", Some("route='RouteID'")),
            declaration("type", Some("route='RouteID'")),
            declaration("fake_probe", Some("'not-rust'='RouteID'")),
            declaration("fake_probe", Some("type='RouteID'")),
        ] {
            render(&BTreeMap::from([(manifest.key.clone(), manifest)])).unwrap_err();
        }
    }
}

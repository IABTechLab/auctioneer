//! Syntax DATA only. Parsing does not establish source identity or semantics.

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use std::str::from_utf8;
use syn::parse::{Parse, ParseStream};

pub(in crate::schema_validation) fn parse_file(bytes: &[u8]) -> Result<syn::File, ()> {
    let source = from_utf8(bytes).map_err(|_error| ())?;
    syn::parse_file(source).map_err(|_error| ())
}

pub(in crate::schema_validation) fn tokens(value: &impl ToTokens) -> String {
    value.to_token_stream().to_string()
}

pub(in crate::schema_validation) fn literal_doc(attribute: &syn::Attribute) -> bool {
    if !matches!(&attribute.style, syn::AttrStyle::Outer) || !attribute.path().is_ident("doc") {
        return false;
    }
    let syn::Meta::NameValue(meta) = &attribute.meta else {
        return false;
    };
    matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_)))
}

pub(in crate::schema_validation) fn without_docs(mut item: syn::Item) -> syn::Item {
    if let syn::Item::Fn(function) = &mut item {
        function.attrs.retain(|attribute| !literal_doc(attribute));
        return item;
    }
    if let syn::Item::Const(constant) = &mut item {
        constant.attrs.retain(|attribute| !literal_doc(attribute));
        return item;
    }
    if let syn::Item::Impl(implementation) = &mut item {
        implementation
            .attrs
            .retain(|attribute| !literal_doc(attribute));
        for member in &mut implementation.items {
            if let syn::ImplItem::Fn(function) = member {
                function.attrs.retain(|attribute| !literal_doc(attribute));
            }
        }
    }
    item
}

// Syn 3.0.10 preserves const trait impls as Item::Verbatim. This deliberately
// narrow projection retains the const token, unsafe qualifier, all generic
// bounds and the where clause. It never turns const syntax into ItemImpl.
pub(in crate::schema_validation) struct ConstTraitImpl {
    pub(in crate::schema_validation) attrs: Vec<syn::Attribute>,
    pub(in crate::schema_validation) header: TokenStream,
    pub(in crate::schema_validation) items: Vec<syn::ImplItem>,
}

impl Parse for ConstTraitImpl {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let constness: syn::Token![const] = input.parse()?;
        let unsafety: Option<syn::Token![unsafe]> = input.parse()?;
        let impl_token: syn::Token![impl] = input.parse()?;
        let generics: syn::Generics = input.parse()?;
        let trait_path: syn::Path = input.parse()?;
        let for_token: syn::Token![for] = input.parse()?;
        let self_type: syn::Type = input.parse()?;
        let where_clause: Option<syn::WhereClause> = input.parse()?;
        let content;
        syn::braced!(content in input);
        let mut items = Vec::new();
        while !content.is_empty() {
            items.push(content.parse()?);
        }
        Ok(Self {
            attrs,
            header: quote!(#constness #unsafety #impl_token #generics #trait_path #for_token #self_type #where_clause),
            items,
        })
    }
}

//! Syntax DATA only. Parsing does not establish source identity or semantics.

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};

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

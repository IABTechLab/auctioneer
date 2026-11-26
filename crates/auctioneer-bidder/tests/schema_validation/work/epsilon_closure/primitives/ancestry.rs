//! Supported vendor ancestor recipes, not a resolved builtin capability.
//!
//! All bytes come from the same existing source window. Ordered items and
//! executable attributes match; only literal documentation is ignored. This
//! does not interpret cfgs or macro expansions. In particular the preserved
//! doctest invocation still needs its actual compilation-context witness.
//! Core export/prelude/builtin and actual caller/artifact duties remain open.

use super::Refusal;
use crate::schema_validation::handoff::SourceInputs;
use quote::ToTokens as _;
use std::str::from_utf8;

fn bytes<'a>(inputs: &'a SourceInputs<'_>, unit: &'static str) -> Result<&'a [u8], Refusal> {
    inputs.source(unit).ok_or(Refusal::VendorAncestry(unit))
}

fn literal_docs(attributes: &mut Vec<syn::Attribute>) {
    attributes.retain(|attribute| {
        !matches!(&attribute.meta, syn::Meta::NameValue(meta)
            if attribute.path().is_ident("doc")
            && matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_))))
    });
}

fn recipe(mut file: syn::File, unit: &'static str) -> Result<String, Refusal> {
    if file.shebang.is_some() {
        return Err(Refusal::VendorAncestry(unit));
    }
    literal_docs(&mut file.attrs);
    for item in &mut file.items {
        let attributes = match item {
            syn::Item::ExternCrate(item) => &mut item.attrs,
            syn::Item::Macro(item) => &mut item.attrs,
            syn::Item::Mod(item) => &mut item.attrs,
            syn::Item::Use(item) => &mut item.attrs,
            _ => return Err(Refusal::VendorAncestry(unit)),
        };
        literal_docs(attributes);
    }
    Ok(file.to_token_stream().to_string())
}

fn check_file(
    inputs: &SourceInputs<'_>,
    unit: &'static str,
    expected: syn::File,
) -> Result<(), Refusal> {
    let text = from_utf8(bytes(inputs, unit)?).map_err(|_error| Refusal::VendorAncestry(unit))?;
    let actual = syn::parse_file(text).map_err(|_error| Refusal::VendorAncestry(unit))?;
    if recipe(actual, unit)? != recipe(expected, unit)? {
        return Err(Refusal::VendorAncestry(unit));
    }
    Ok(())
}

// Only these manifest fields are interpreted. Their values do not prove the
// effective flags, feature selection or identity of a compiled dependency.
fn check_manifest(inputs: &SourceInputs<'_>) -> Result<(), Refusal> {
    const UNIT: &str = "RegexManifest";
    let text = from_utf8(bytes(inputs, UNIT)?).map_err(|_error| Refusal::VendorAncestry(UNIT))?;
    let manifest: toml::Value =
        toml::from_str(text).map_err(|_error| Refusal::VendorAncestry(UNIT))?;
    let field = |section: &str, name: &str| {
        manifest
            .get(section)
            .and_then(|table| table.get(name))
            .and_then(toml::Value::as_str)
    };
    if field("package", "edition") != Some("2021") || field("lib", "path") != Some("src/lib.rs") {
        return Err(Refusal::VendorAncestry(UNIT));
    }
    Ok(())
}

pub(super) fn check_vendor_ancestry(inputs: &SourceInputs<'_>) -> Result<(), Refusal> {
    check_manifest(inputs)?;
    check_file(
        inputs,
        "RegexRoot",
        syn::parse_quote! {
            #![no_std]
            #![deny(missing_docs)]
            #![cfg_attr(all(feature = "std", feature = "nfa", feature = "dfa", feature = "hybrid"), deny(rustdoc::broken_intra_doc_links))]
            #![cfg_attr(not(all(feature = "std", feature = "nfa", feature = "dfa", feature = "hybrid")), allow(rustdoc::broken_intra_doc_links))]
            #![cfg_attr(not(all(feature = "std", feature = "nfa", feature = "dfa", feature = "hybrid", feature = "perf-literal-substring", feature = "perf-literal-multisubstring",)), allow(dead_code, unused_imports, unused_variables))]
            #![warn(missing_debug_implementations)]
            #![cfg_attr(docsrs_regex, feature(doc_cfg))]
            #[cfg(not(any(target_pointer_width = "16", target_pointer_width = "32", target_pointer_width = "64")))]
            compile_error!("not supported on non-{16,32,64}, please file an issue");
            #[cfg(any(test, feature = "std"))]
            extern crate std;
            #[cfg(feature = "alloc")]
            extern crate alloc;
            #[cfg(doctest)]
            doc_comment::doctest!("../README.md");
            #[doc(inline)]
            pub use crate::util::primitives::PatternID;
            pub use crate::util::search::*;
            #[macro_use]
            mod macros;
            #[cfg(any(feature = "dfa-search", feature = "dfa-onepass"))]
            pub mod dfa;
            #[cfg(feature = "hybrid")]
            pub mod hybrid;
            #[cfg(feature = "meta")]
            pub mod meta;
            #[cfg(feature = "nfa-thompson")]
            pub mod nfa;
            pub mod util;
        },
    )?;
    check_file(
        inputs,
        "RegexMacros",
        syn::parse_quote! {
            #![allow(unused_macros)]
            macro_rules! log {
                ($($tt:tt)*) => {
                    #[cfg(feature = "logging")]
                    { $($tt)* }
                }
            }
            macro_rules! debug {
                ($($tt:tt)*) => { log!(log::debug!($($tt)*)) }
            }
            macro_rules! trace {
                ($($tt:tt)*) => { log!(log::trace!($($tt)*)) }
            }
        },
    )?;
    check_file(
        inputs,
        "RegexUtil",
        syn::parse_quote! {
            pub mod alphabet;
            #[cfg(feature = "alloc")]
            pub mod captures;
            pub mod escape;
            #[cfg(feature = "alloc")]
            pub mod interpolate;
            pub mod iter;
            pub mod lazy;
            pub mod look;
            #[cfg(feature = "alloc")]
            pub mod pool;
            pub mod prefilter;
            pub mod primitives;
            pub mod start;
            #[cfg(feature = "syntax")]
            pub mod syntax;
            pub mod wire;
            #[cfg(any(feature = "dfa-build", feature = "hybrid"))]
            pub(crate) mod determinize;
            pub(crate) mod empty;
            pub(crate) mod int;
            pub(crate) mod memchr;
            pub(crate) mod search;
            #[cfg(feature = "alloc")]
            pub(crate) mod sparse_set;
            pub(crate) mod unicode_data;
            pub(crate) mod utf8;
        },
    )
}

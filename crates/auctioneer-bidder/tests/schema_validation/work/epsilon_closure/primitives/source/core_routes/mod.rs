//! Supported explicit core routes, not an actual caller/builtin capability.
//!
//! Conditional routing argument: compile this retained source graph with
//! `CoreLib` as core's root using the pinned compiler. The already matched
//! `macro_export` definition supplies the root assert binding. The checked
//! explicit v1 import cannot be replaced by a glob, and the whole supported
//! `rust_2021` module forwards that v1 route. Competing bindings cannot replace
//! these checked bindings in that successful compilation; not every second
//! spelling is illegal. See the Rust Reference's macro path-based scope and
//! use/glob-import rules. This is an inference under the stated premises.
//!
//! This does not prove the actual compilation/source/artifact/cfg premises,
//! vendor-leaf name lookup, derive/intrinsic identities or builtin expansion
//! costs. Existing work/frame/buffer duties and incompleteness remain.
//! <https://doc.rust-lang.org/reference/macros-by-example.html#path-based-scope>
//! <https://doc.rust-lang.org/reference/items/use-declarations.html#glob-imports>.

mod root_attributes;

use super::super::ancestry::literal_docs;
use super::{Refusal, SourceInputs, spelling};
use quote::quote;
use std::str::from_utf8;

const LIB: &str = "CoreLib";
const PRELUDE: &str = "CorePrelude";
const V1: &str = "CorePreludeV1";
const MACROS: &str = "Macros";

fn captured_file(inputs: &SourceInputs<'_>, unit: &'static str) -> Result<syn::File, Refusal> {
    let bytes = inputs.source(unit).ok_or(Refusal::CoreAncestry(unit))?;
    let text = from_utf8(bytes).map_err(|_error| Refusal::CoreAncestry(unit))?;
    syn::parse_file(text).map_err(|_error| Refusal::CoreAncestry(unit))
}

fn check_roots(
    file: &syn::File,
    mut expected: Vec<syn::Attribute>,
    unit: &'static str,
) -> Result<(), Refusal> {
    let mut found = file.attrs.clone();
    literal_docs(&mut found);
    literal_docs(&mut expected);
    if file.shebang.is_some() || spelling(&quote!(#(#found)*)) != spelling(&quote!(#(#expected)*)) {
        return Err(Refusal::CoreAncestry(unit));
    }
    Ok(())
}

fn take_module(
    file: &mut syn::File,
    name: &str,
    unit: &'static str,
) -> Result<syn::ItemMod, Refusal> {
    let index = {
        let mut found = file
            .items
            .iter()
            .enumerate()
            .filter_map(|(position, item)| {
                let syn::Item::Mod(candidate) = item else {
                    return None;
                };
                (candidate.ident == name).then_some(position)
            });
        let first = found.next().ok_or(Refusal::CoreAncestry(unit))?;
        if found.next().is_some() {
            return Err(Refusal::CoreAncestry(unit));
        }
        first
    };
    let syn::Item::Mod(module) = file.items.remove(index) else {
        return Err(Refusal::CoreAncestry(unit));
    };
    Ok(module)
}

fn check_module(
    mut actual: syn::ItemMod,
    mut expected: syn::ItemMod,
    unit: &'static str,
) -> Result<(), Refusal> {
    for declaration in [&mut actual, &mut expected] {
        literal_docs(&mut declaration.attrs);
        if let Some((_, children)) = &mut declaration.content {
            for child in children {
                let syn::Item::Use(import) = child else {
                    return Err(Refusal::CoreAncestry(unit));
                };
                literal_docs(&mut import.attrs);
            }
        }
    }
    if spelling(&actual) != spelling(&expected) {
        return Err(Refusal::CoreAncestry(unit));
    }
    Ok(())
}

fn check_lib(inputs: &SourceInputs<'_>) -> Result<(), Refusal> {
    let mut file = captured_file(inputs, LIB)?;
    check_roots(&file, root_attributes::core_lib(), LIB)?;
    check_module(
        take_module(&mut file, "prelude", LIB)?,
        syn::parse_quote!(
            pub mod prelude;
        ),
        LIB,
    )?;
    check_module(
        take_module(&mut file, "macros", LIB)?,
        syn::parse_quote!(
            #[macro_use]
            mod macros;
        ),
        LIB,
    )
}

fn check_prelude(inputs: &SourceInputs<'_>) -> Result<(), Refusal> {
    let mut file = captured_file(inputs, PRELUDE)?;
    check_roots(&file, root_attributes::prelude(), PRELUDE)?;
    check_module(
        take_module(&mut file, "v1", PRELUDE)?,
        syn::parse_quote!(
            pub mod v1;
        ),
        PRELUDE,
    )?;
    check_module(
        take_module(&mut file, "rust_2021", PRELUDE)?,
        syn::parse_quote! {
            #[stable(feature = "prelude_2021", since = "1.55.0")]
            pub mod rust_2021 {
                #[stable(feature = "prelude_2021", since = "1.55.0")]
                #[doc(no_inline)]
                pub use super::v1::*;
                #[stable(feature = "prelude_2021", since = "1.55.0")]
                #[doc(no_inline)]
                pub use crate::iter::FromIterator;
                #[stable(feature = "prelude_2021", since = "1.55.0")]
                #[doc(no_inline)]
                pub use crate::convert::{TryFrom, TryInto};
            }
        },
        PRELUDE,
    )
}

fn check_v1(inputs: &SourceInputs<'_>) -> Result<(), Refusal> {
    let file = captured_file(inputs, V1)?;
    check_roots(&file, root_attributes::v1(), V1)?;
    let expected: syn::ItemUse = syn::parse_quote! {
        #[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
        #[doc(no_inline)]
        #[expect(deprecated)]
        pub use crate::{
            assert, assert_eq, assert_ne, cfg, column, compile_error, concat,
            debug_assert, debug_assert_eq, debug_assert_ne, file, format_args,
            include, include_bytes, include_str, line, matches, module_path,
            option_env, stringify, todo, r#try, unimplemented, unreachable, write, writeln,
        };
    };
    let found = file
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Use(mut import) = item else {
                return None;
            };
            literal_docs(&mut import.attrs);
            Some(import)
        })
        .filter(|import| spelling(import) == spelling(&expected))
        .count();
    if found != 1 {
        return Err(Refusal::CoreAncestry(V1));
    }
    Ok(())
}

fn check_builtin(mut file: syn::File) -> Result<(), Refusal> {
    check_roots(&file, Vec::new(), MACROS)?;
    let mut builtin = take_module(&mut file, "builtin", MACROS)?;
    let (brace, children) = builtin
        .content
        .take()
        .ok_or(Refusal::CoreAncestry(MACROS))?;
    let direct_asserts = children
        .iter()
        .filter_map(|item| {
            let syn::Item::Macro(definition) = item else {
                return None;
            };
            definition
                .ident
                .as_ref()
                .is_some_and(|name| name == "assert")
                .then_some(())
        })
        .count();
    if direct_asserts != 1 {
        return Err(Refusal::CoreAncestry(MACROS));
    }
    // The same file's assert body passed the existing recipe before entry.
    // Compare every module-header field without copying its large body tree.
    builtin.content = Some((brace, Vec::new()));
    check_module(
        builtin,
        syn::parse_quote!(
            pub(crate) mod builtin {}
        ),
        MACROS,
    )
}

pub(super) fn check(inputs: &SourceInputs<'_>, macros: syn::File) -> Result<(), Refusal> {
    check_lib(inputs)?;
    check_prelude(inputs)?;
    check_v1(inputs)?;
    check_builtin(macros)
}

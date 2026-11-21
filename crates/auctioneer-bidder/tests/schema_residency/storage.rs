//! Exhaustive emitted storage traversal, including function/block-local statics.

use quote::ToTokens as _;
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};
use syn::visit_mut::{self, VisitMut};

const REFUSAL: &str = "unsupported expanded schema storage";

pub(super) struct Storage {
    pub schema_state: Vec<Value>,
    pub readonly: Vec<Value>,
    pub initialization: Vec<String>,
}

struct Owners<'a> {
    namespace: &'a str,
    snapshot: &'a Path,
    programs: BTreeSet<&'a str>,
    scopes: Vec<String>,
    block: usize,
    block_depth: usize,
    state: Storage,
    error: Option<String>,
}

fn path_is(path: &syn::Path, names: &[&str]) -> bool {
    path.segments.len() == names.len()
        && path
            .segments
            .iter()
            .zip(names)
            .all(|(part, name)| part.ident == *name && part.arguments.is_empty())
}

fn json_initializer(expr: &syn::Expr) -> Result<Value, String> {
    let syn::Expr::Call(lazy) = expr else {
        return Err(REFUSAL.into());
    };
    if !matches!(lazy.func.as_ref(), syn::Expr::Path(path) if path_is(&path.path, &["__Lazy", "new"]))
    {
        return Err(REFUSAL.into());
    }
    if lazy.args.len() != 1 {
        return Err(REFUSAL.into());
    }
    let Some(syn::Expr::Closure(closure)) = lazy.args.first() else {
        return Err(REFUSAL.into());
    };
    let syn::Expr::Block(block) = closure.body.as_ref() else {
        return Err(REFUSAL.into());
    };
    let [syn::Stmt::Expr(syn::Expr::MethodCall(expect), None)] = block.block.stmts.as_slice()
    else {
        return Err(REFUSAL.into());
    };
    if expect.method != "expect" || expect.args.len() != 1 {
        return Err(REFUSAL.into());
    }
    let syn::Expr::Call(parse) = expect.receiver.as_ref() else {
        return Err(REFUSAL.into());
    };
    let syn::Expr::Path(path) = parse.func.as_ref() else {
        return Err(REFUSAL.into());
    };
    if path.qself.is_some()
        || path.path.segments.len() != 2
        || path.path.segments[0].ident != "__sj"
        || path.path.segments[1].ident != "from_str"
        || parse.args.len() != 1
    {
        return Err(REFUSAL.into());
    }
    let Some(syn::Expr::Lit(literal)) = parse.args.first() else {
        return Err(REFUSAL.into());
    };
    let syn::Lit::Str(text) = &literal.lit else {
        return Err(REFUSAL.into());
    };
    serde_json::from_str(&text.value()).map_err(|_error| REFUSAL.into())
}

fn type_argument<'a>(ty: &'a syn::Type, name: &str) -> Option<&'a syn::Type> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = path.path.segments.first()?;
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if segment.ident != name || arguments.args.len() != 1 {
        return None;
    }
    match arguments.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

fn type_is(ty: &syn::Type, names: &[&str]) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none()
        && path.path.leading_colon.is_none() && path_is(&path.path, names))
}

fn lazy_kind(ty: &syn::Type) -> Option<&'static str> {
    // Generic types have structural identity; joint `>>` punctuation in a
    // token printer must not change whether a retained owner is recognized.
    let inner = type_argument(ty, "__Lazy")?;
    if type_is(inner, &["__sj", "Value"]) {
        Some("lazy_json_value")
    } else if type_is(inner, &["__sj", "Number"]) {
        Some("lazy_json_number")
    } else if type_argument(inner, "Vec").is_some_and(|value| type_is(value, &["__sj", "Value"])) {
        Some("lazy_json_array")
    } else {
        None
    }
}

impl Owners<'_> {
    fn identity(&self, name: &str) -> String {
        format!("{}::{}::{name}", self.namespace, self.scopes.join("::"))
    }

    fn storage(&mut self, item: &syn::ItemStatic) -> Result<(), String> {
        let name = item.ident.to_string();
        if self.programs.contains(name.as_str()) {
            if self.block_depth != 0
                || !type_argument(&item.ty, "__Lazy")
                    .and_then(|inner| type_argument(inner, "Option"))
                    .is_some_and(|inner| type_is(inner, &["__ra", "meta", "Regex"]))
            {
                return Err(REFUSAL.into());
            }
            return Ok(()); // Constructor/cache bijection is checked separately.
        }
        if let Some(kind) = lazy_kind(&item.ty) {
            let value = json_initializer(&item.expr)?;
            if (kind == "lazy_json_array" && !value.is_array())
                || (kind == "lazy_json_number" && !value.is_number())
            {
                return Err(REFUSAL.into());
            }
            self.state.schema_state.push(json!({"identity":self.identity(&name),
                "kind":kind,"value":value,"initializer_sha256":super::expanded::digest(item.expr.to_token_stream().to_string().as_bytes()),
                "lifetime":"first-validation-to-process-exit"}));
            return Ok(());
        }
        if !matches!(name.as_str(), "PATTERN_PREFIXES" | "PATTERN_LITERALS")
            || item.ty.to_token_stream().to_string() != quote::quote!(&[&str]).to_string()
        {
            return Err(REFUSAL.into());
        }
        let syn::Expr::Reference(reference) = item.expr.as_ref() else {
            return Err(REFUSAL.into());
        };
        let syn::Expr::Array(array) = reference.expr.as_ref() else {
            return Err(REFUSAL.into());
        };
        let strings = array
            .elems
            .iter()
            .map(|expr| match expr {
                syn::Expr::Lit(value) => match &value.lit {
                    syn::Lit::Str(text) => Ok(text.value()),
                    _ => Err(REFUSAL.to_owned()),
                },
                _ => Err(REFUSAL.to_owned()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.state
            .readonly
            .push(json!({"identity":self.identity(&name),
            "kind":"readonly_string_table","strings":strings}));
        Ok(())
    }
}

impl VisitMut for Owners<'_> {
    fn visit_item_mod_mut(&mut self, item: &mut syn::ItemMod) {
        if item.content.is_none() {
            self.error = Some(REFUSAL.into());
            return;
        }
        self.scopes.push(item.ident.to_string());
        visit_mut::visit_item_mod_mut(self, item);
        self.scopes.pop();
    }

    fn visit_item_fn_mut(&mut self, item: &mut syn::ItemFn) {
        self.scopes.push(item.sig.ident.to_string());
        if item.sig.ident == "initialize_regexes" {
            struct References(Vec<String>);
            impl VisitMut for References {
                fn visit_expr_path_mut(&mut self, path: &mut syn::ExprPath) {
                    if let Some(name) = path.path.get_ident()
                        && name.to_string().ends_with("_REGEX")
                    {
                        self.0.push(name.to_string());
                    }
                }
            }
            let mut references = References(Vec::new());
            references.visit_block_mut(&mut item.block);
            self.state.initialization.extend(references.0);
        }
        visit_mut::visit_item_fn_mut(self, item);
        self.scopes.pop();
    }

    fn visit_block_mut(&mut self, block: &mut syn::Block) {
        self.block += 1;
        self.block_depth += 1;
        self.scopes.push(format!("block{}", self.block));
        visit_mut::visit_block_mut(self, block);
        self.scopes.pop();
        self.block_depth -= 1;
    }

    fn visit_item_static_mut(&mut self, item: &mut syn::ItemStatic) {
        if let Err(error) = self.storage(item) {
            self.error = Some(error);
        }
        visit_mut::visit_item_static_mut(self, item);
    }

    fn visit_item_const_mut(&mut self, item: &mut syn::ItemConst) {
        let valid = if let syn::Expr::Macro(expression) = item.expr.as_ref()
            && expression.mac.path.is_ident("include_str")
        {
            syn::parse2::<syn::LitStr>(expression.mac.tokens.clone())
                .is_ok_and(|literal| Path::new(&literal.value()) == self.snapshot)
        } else {
            false
        };
        if valid {
            self.state
                .readonly
                .push(json!({"identity":self.identity("schema_source"),
                "kind":"readonly_schema_source"}));
        } else {
            self.error = Some(REFUSAL.into());
        }
        visit_mut::visit_item_const_mut(self, item);
    }

    fn visit_macro_mut(&mut self, item: &mut syn::Macro) {
        if item.path.is_ident("include_str") {
            if !syn::parse2::<syn::LitStr>(item.tokens.clone())
                .is_ok_and(|literal| Path::new(&literal.value()) == self.snapshot)
            {
                self.error = Some(REFUSAL.into());
            }
            return;
        }
        if !item.path.is_ident("matches") {
            self.error = Some(REFUSAL.into());
            return;
        }
        match syn::parse2::<MatchInput>(item.tokens.clone()) {
            Ok(mut input) => {
                self.visit_expr_mut(&mut input.value);
                self.visit_pat_mut(&mut input.pattern);
                if let Some(guard) = &mut input.guard {
                    self.visit_expr_mut(guard);
                }
            }
            Err(_error) => self.error = Some(REFUSAL.into()),
        }
    }
}

struct MatchInput {
    value: syn::Expr,
    pattern: syn::Pat,
    guard: Option<syn::Expr>,
}

impl syn::parse::Parse for MatchInput {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let value = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let pattern = syn::Pat::parse_multi_with_leading_vert(input)?;
        let guard = if input.peek(syn::Token![if]) {
            input.parse::<syn::Token![if]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        if !input.is_empty() {
            input.parse::<syn::Token![,]>()?;
        }
        Ok(Self {
            value,
            pattern,
            guard,
        })
    }
}

pub(super) fn collect(
    syntax: &mut syn::File,
    namespace: &str,
    snapshot: &Path,
    patterns: &[(String, String)],
) -> Result<Storage, String> {
    let programs = patterns
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
    let mut owners = Owners {
        namespace,
        snapshot,
        programs,
        scopes: Vec::new(),
        block: 0,
        block_depth: 0,
        state: Storage {
            schema_state: Vec::new(),
            readonly: Vec::new(),
            initialization: Vec::new(),
        },
        error: None,
    };
    owners.visit_file_mut(syntax);
    if let Some(error) = owners.error {
        return Err(error);
    }
    if owners.state.initialization.len() != owners.programs.len()
        || owners
            .state
            .initialization
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != owners.programs
    {
        return Err("expanded initialization membership mismatch".into());
    }
    Ok(owners.state)
}

#[cfg(test)]
mod tests {
    use super::collect;
    use std::path::Path;

    fn fixture() -> syn::File {
        syn::parse_quote! {
            mod fake_generated {
                static FAKE_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| None);
                fn initialize_regexes() -> bool { FAKE_REGEX.as_ref().is_some() }
            }
        }
    }

    #[test]
    fn expanded_storage_rejects_shadowed_regex_names_in_function_scopes() {
        let patterns = vec![("FAKE_REGEX".to_owned(), "^fake$".to_owned())];
        assert!(
            collect(
                &mut fixture(),
                "fake_owner",
                Path::new("/fake/schema.json"),
                &patterns
            )
            .is_ok()
        );
        let mut syntax = fixture();
        let syn::Item::Mod(module) = &mut syntax.items[0] else {
            panic!("fixture module");
        };
        module.content.as_mut().unwrap().1.push(syn::parse_quote! {
            fn fake_hidden() {
                static FAKE_REGEX: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| vec![0_u8; 17]);
            }
        });
        assert_eq!(
            collect(
                &mut syntax,
                "fake_owner",
                Path::new("/fake/schema.json"),
                &patterns
            )
            .err()
            .as_deref(),
            Some("unsupported expanded schema storage")
        );
    }

    #[test]
    fn expanded_storage_rejects_unknown_and_nested_storage_macros() {
        for extra in [
            quote::quote! { macro_rules! fake_hidden { () => { static FAKE: Vec<u8> = Vec::new(); } } },
            quote::quote! { fake_hidden!(); },
            quote::quote! { fn fake_guard() -> bool { matches!({ static FAKE: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| vec![0_u8; 17]); true }, true) } },
        ] {
            let mut syntax = fixture();
            let syn::Item::Mod(module) = &mut syntax.items[0] else {
                panic!("fixture module");
            };
            module
                .content
                .as_mut()
                .unwrap()
                .1
                .push(syn::parse2(extra).unwrap());
            let patterns = vec![("FAKE_REGEX".to_owned(), "^fake$".to_owned())];
            assert_eq!(
                collect(
                    &mut syntax,
                    "fake_owner",
                    Path::new("/fake/schema.json"),
                    &patterns
                )
                .err()
                .as_deref(),
                Some("unsupported expanded schema storage")
            );
        }
    }
}

//! Inspect generated constructors and defaults without depending on Rust formatting.

use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::{Expr, Item, Stmt, Token};

pub struct GeneratedConfig(syn::File);

impl GeneratedConfig {
    pub fn parse(source: &str) -> syn::Result<Self> {
        syn::parse_file(source).map(Self)
    }

    fn function(&self, name: &str) -> syn::Result<&syn::ItemFn> {
        self.0
            .items
            .iter()
            .find_map(|item| {
                if let Item::Fn(function) = item
                    && function.sig.ident == name
                {
                    Some(function)
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("missing generated function {name}"),
                )
            })
    }

    pub fn initializer(&self, name: &str) -> syn::Result<&Expr> {
        let function = self.function(name)?;
        if let Some(Stmt::Expr(Expr::MethodCall(call), None)) = function.block.stmts.last()
            && call.method == "get_or_init"
            && call.args.len() == 1
            && let Some(Expr::Closure(closure)) = call.args.first()
        {
            return Ok(&closure.body);
        }
        Err(syn::Error::new_spanned(
            function,
            "expected generated OnceLock initializer",
        ))
    }

    fn bidders(&self) -> syn::Result<Vec<Expr>> {
        let bidders = vector(self.initializer("bidders")?)?;
        for bidder in &bidders {
            if !matches!(bidder, Expr::Struct(record) if record.path.is_ident("BidderInfo")) {
                return Err(syn::Error::new_spanned(
                    bidder,
                    "expected bidder constructor",
                ));
            }
        }
        Ok(bidders)
    }

    pub fn bidder_names(&self) -> syn::Result<Vec<String>> {
        self.bidders()?
            .iter()
            .map(|bidder| string(field(bidder, &["name"])?))
            .collect()
    }

    pub fn bidder(&self, name: &str) -> syn::Result<Option<Expr>> {
        let mut found = None;
        for bidder in self.bidders()? {
            if string(field(&bidder, &["name"])?)? == name {
                if found.is_some() {
                    return Err(syn::Error::new_spanned(
                        bidder,
                        "duplicate bidder constructor",
                    ));
                }
                found = Some(bidder);
            }
        }
        Ok(found)
    }

    pub fn static_rows(&self, function_name: &str, name: &str) -> syn::Result<Vec<Expr>> {
        let function = self.function(function_name)?;
        for statement in &function.block.stmts {
            if let Stmt::Item(Item::Static(table)) = statement
                && table.ident == name
                && let Expr::Reference(borrow) = table.expr.as_ref()
                && let Expr::Array(array) = borrow.expr.as_ref()
            {
                return Ok(array.elems.iter().cloned().collect());
            }
        }
        Err(syn::Error::new_spanned(
            function,
            "missing generated static array",
        ))
    }
}

pub fn field<'a>(mut expression: &'a Expr, names: &[&str]) -> syn::Result<&'a Expr> {
    for name in names {
        let Expr::Struct(record) = expression else {
            return Err(syn::Error::new_spanned(
                expression,
                "expected generated struct literal",
            ));
        };
        expression = record
            .fields
            .iter()
            .find_map(|field| {
                if let syn::Member::Named(ident) = &field.member
                    && ident == *name
                {
                    Some(&field.expr)
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                syn::Error::new_spanned(record, format!("missing generated field {name}"))
            })?;
    }
    Ok(expression)
}

pub fn string(expression: &Expr) -> syn::Result<String> {
    if let Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(value),
        ..
    }) = expression
    {
        return Ok(value.value());
    }
    if let Expr::MethodCall(call) = expression
        && call.method == "to_owned"
        && call.args.is_empty()
    {
        return string(&call.receiver);
    }
    Err(syn::Error::new_spanned(
        expression,
        "expected generated string literal",
    ))
}

pub fn integer(expression: &Expr) -> syn::Result<u64> {
    if let Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(value),
        ..
    }) = expression
    {
        return value.base10_parse();
    }
    Err(syn::Error::new_spanned(
        expression,
        "expected generated nonnegative integer",
    ))
}

pub fn boolean(expression: &Expr) -> syn::Result<bool> {
    if let Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Bool(value),
        ..
    }) = expression
    {
        return Ok(value.value);
    }
    Err(syn::Error::new_spanned(
        expression,
        "expected generated boolean",
    ))
}

pub fn path_is(expression: &Expr, names: &[&str]) -> bool {
    matches!(expression, Expr::Path(path) if path.qself.is_none()
        && path.path.segments.len() == names.len()
        && path.path.segments.iter().zip(names).all(|(segment, name)| segment.ident == *name && matches!(segment.arguments, syn::PathArguments::None)))
}

pub fn some(expression: &Expr) -> syn::Result<&Expr> {
    if let Expr::Call(call) = expression
        && path_is(&call.func, &["Some"])
        && call.args.len() == 1
        && let Some(value) = call.args.first()
    {
        return Ok(value);
    }
    Err(syn::Error::new_spanned(
        expression,
        "expected generated Some value",
    ))
}

pub fn vector(expression: &Expr) -> syn::Result<Vec<Expr>> {
    if let Expr::Macro(vector) = expression
        && vector.mac.path.is_ident("vec")
    {
        return Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(vector.mac.tokens.clone())
            .map(|elements| elements.into_iter().collect());
    }
    if let Expr::Call(call) = expression
        && call.args.is_empty()
        && path_is(&call.func, &["Vec", "new"])
    {
        return Ok(Vec::new());
    }
    Err(syn::Error::new_spanned(
        expression,
        "expected generated vector",
    ))
}

pub fn map_entries(expression: &Expr) -> syn::Result<Vec<(Expr, Expr)>> {
    let invalid = || syn::Error::new_spanned(expression, "expected generated map initializer");
    if let Expr::Call(call) = expression
        && call.args.is_empty()
        && path_is(&call.func, &["BTreeMap", "new"])
    {
        return Ok(Vec::new());
    }
    let Expr::Block(block) = expression else {
        return Err(invalid());
    };
    let [Stmt::Local(local), statements @ .., Stmt::Expr(tail, None)] =
        block.block.stmts.as_slice()
    else {
        return Err(invalid());
    };
    let syn::Pat::Ident(binding) = &local.pat else {
        return Err(invalid());
    };
    let Some(initializer) = &local.init else {
        return Err(invalid());
    };
    let Expr::Call(call) = initializer.expr.as_ref() else {
        return Err(invalid());
    };
    if !call.args.is_empty() || !path_is(&call.func, &["BTreeMap", "new"]) {
        return Err(invalid());
    }
    let name = binding.ident.to_string();
    if !path_is(tail, &[&name]) {
        return Err(invalid());
    }
    statements
        .iter()
        .map(|statement| {
            if let Stmt::Expr(Expr::MethodCall(insert), Some(_)) = statement
                && insert.method == "insert"
                && insert.args.len() == 2
                && path_is(&insert.receiver, &[&name])
                && let Some((key, value)) = insert.args.first().zip(insert.args.last())
            {
                return Ok((key.clone(), value.clone()));
            }
            Err(invalid())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        GeneratedConfig, boolean, field, integer, map_entries, path_is, some, string, vector,
    };
    use proc_macro2::TokenStream;
    use quote::quote;
    use std::panic::catch_unwind;

    fn fixture(bidders: &TokenStream) -> String {
        quote! {
            fn bidders() -> &'static [BidderInfo] {
                static CELL: OnceLock<Vec<BidderInfo>> = OnceLock::new();
                CELL.get_or_init(|| vec![#bidders])
            }
            fn bidder_metadata_declarations() -> &'static [BidderMetadataDeclaration] {
                static DECLARATIONS: &[BidderMetadataDeclaration] = &[
                    BidderMetadataDeclaration { bidder: "fake_probe", implementation: "fake_owner" }
                ];
                DECLARATIONS
            }
        }
        .to_string()
    }

    #[test]
    fn negative_roster_checks_distinguish_declarations_from_executed_constructors() {
        let omitted = fixture(&quote! {
            BidderInfo { name: "fake_active".to_owned(), endpoint: "fake_probe".to_owned() }
        });
        let syntax = GeneratedConfig::parse(&omitted).unwrap();
        assert_eq!(syntax.bidder_names().unwrap(), ["fake_active"]);
        assert!(syntax.bidder("fake_probe").unwrap().is_none());
        let declarations = syntax
            .static_rows("bidder_metadata_declarations", "DECLARATIONS")
            .unwrap();
        assert_eq!(
            string(field(&declarations[0], &["bidder"]).unwrap()).unwrap(),
            "fake_probe"
        );

        let leaked = fixture(&quote! {
            BidderInfo { name: "fake_probe".to_owned() }
        });
        assert!(
            GeneratedConfig::parse(&leaked)
                .unwrap()
                .bidder("fake_probe")
                .unwrap()
                .is_some()
        );
        let refusal = catch_unwind(|| {
            let leaked_syntax = GeneratedConfig::parse(&leaked).unwrap();
            assert!(
                leaked_syntax.bidder("fake_probe").unwrap().is_none(),
                "executed bidder must not pass the negative roster check"
            );
        })
        .unwrap_err();
        assert_eq!(
            refusal.downcast_ref::<&str>().copied(),
            Some("executed bidder must not pass the negative roster check")
        );
    }

    #[test]
    fn roster_values_ignore_whitespace_and_decode_escaped_strings() {
        let source = fixture(&quote! {
            BidderInfo { name: "fake_quoted\"\\\n".to_owned() },
            BidderInfo { name: "fake_other".to_owned() }
        });
        let spaced = source.replace('.', " . ").replace(',', ",\n");
        for variant in [&source, &spaced] {
            assert_eq!(
                GeneratedConfig::parse(variant)
                    .unwrap()
                    .bidder_names()
                    .unwrap(),
                ["fake_quoted\"\\\n", "fake_other"]
            );
        }
    }

    #[test]
    fn missing_or_unsupported_rosters_cannot_satisfy_negative_checks() {
        for (source, expected_error) in [
            ("", "missing generated function bidders"),
            (
                "fn bidders() { unrelated() }",
                "expected generated OnceLock initializer",
            ),
            (
                "fn bidders() { CELL.get_or_init(|| vec![uninspected()]) }",
                "expected bidder constructor",
            ),
        ] {
            assert_eq!(
                GeneratedConfig::parse(source)
                    .unwrap()
                    .bidder_names()
                    .unwrap_err()
                    .to_string(),
                expected_error
            );
        }
        let Err(syntax_error) = GeneratedConfig::parse("fn bidders( {") else {
            panic!("malformed Rust must be rejected");
        };
        assert_eq!(
            syntax_error.to_string(),
            "cannot parse string into token stream"
        );
        assert!(
            GeneratedConfig::parse(&fixture(&quote! {}))
                .unwrap()
                .bidder_names()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn default_values_preserve_map_keys_enums_false_zero_and_empty_vectors() {
        let source = quote! {
            fn host_default() -> &'static HostConfig {
                CELL.get_or_init(|| HostConfig {
                    aliases: {
                        let mut entries = BTreeMap::new();
                        entries.insert("fake_alias".to_owned(), "fake_base".to_owned());
                        entries
                    },
                    purposes: {
                        let mut m = BTreeMap::new();
                        m.insert(5_u8, PurposeEnforcement { enforce_algo: EnforceAlgo::Basic });
                        m
                    },
                    zero: Some(0x0_i64), enabled: Some(false), groups: Some(Vec::new()),
                    numbers: vec![20, 0x38_u8]
                })
            }
        }
        .to_string();
        let syntax = GeneratedConfig::parse(&source).unwrap();
        let host = syntax.initializer("host_default").unwrap();
        let aliases = map_entries(field(host, &["aliases"]).unwrap()).unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(string(&aliases[0].0).unwrap(), "fake_alias");
        assert_eq!(string(&aliases[0].1).unwrap(), "fake_base");
        let purposes = map_entries(field(host, &["purposes"]).unwrap()).unwrap();
        assert_eq!(integer(&purposes[0].0).unwrap(), 5);
        assert!(path_is(
            field(&purposes[0].1, &["enforce_algo"]).unwrap(),
            &["EnforceAlgo", "Basic"]
        ));
        assert!(!path_is(
            field(&purposes[0].1, &["enforce_algo"]).unwrap(),
            &["EnforceAlgo", "Full"]
        ));
        assert_eq!(
            integer(some(field(host, &["zero"]).unwrap()).unwrap()).unwrap(),
            0
        );
        assert!(!boolean(some(field(host, &["enabled"]).unwrap()).unwrap()).unwrap());
        assert!(
            vector(some(field(host, &["groups"]).unwrap()).unwrap())
                .unwrap()
                .is_empty()
        );
        let numbers = vector(field(host, &["numbers"]).unwrap()).unwrap();
        assert_eq!(
            numbers
                .iter()
                .map(integer)
                .collect::<syn::Result<Vec<_>>>()
                .unwrap(),
            [20, 56]
        );
        let Err(field_error) = field(host, &["missing"]) else {
            panic!("missing generated field must be rejected");
        };
        assert_eq!(field_error.to_string(), "missing generated field missing");
        let Err(some_error) = some(&syn::parse_quote!(None)) else {
            panic!("None must not satisfy a generated Some value");
        };
        assert_eq!(some_error.to_string(), "expected generated Some value");
        let Err(vector_error) = vector(&syn::parse_quote!(unknown())) else {
            panic!("unknown calls must not satisfy a generated vector");
        };
        assert_eq!(vector_error.to_string(), "expected generated vector");
        let Err(map_error) = map_entries(&syn::parse_quote!({
            unrelated();
        })) else {
            panic!("unrelated blocks must not satisfy a generated map initializer");
        };
        assert_eq!(map_error.to_string(), "expected generated map initializer");
    }
}

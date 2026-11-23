//! Independently fixed compiler policy, checked against captured attributes.

use crate::schema_qualification::{Draft, audit_schema};
use proc_macro2::TokenStream;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use syn::{Expr, Ident, Lit, Token, braced, parse::Parse, parse::ParseStream, token::Brace};

const REFUSAL: &str = "unqualified captured validator options";

enum Setting {
    Value(Expr),
    Group(TokenStream),
}

struct Settings(BTreeMap<String, Setting>);

impl Parse for Settings {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut fields = BTreeMap::new();
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value = if input.peek(Brace) {
                let contents;
                braced!(contents in input);
                Setting::Group(contents.parse()?)
            } else {
                Setting::Value(input.parse()?)
            };
            if fields.insert(key.to_string(), value).is_some() {
                return Err(syn::Error::new(key.span(), REFUSAL));
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self(fields))
    }
}

impl Settings {
    fn value(&mut self, key: &str) -> Result<Expr, String> {
        match self.0.remove(key) {
            Some(Setting::Value(value)) => Ok(value),
            _ => Err(REFUSAL.into()),
        }
    }

    fn group(&mut self, key: &str) -> Result<TokenStream, String> {
        match self.0.remove(key) {
            Some(Setting::Group(value)) => Ok(value),
            _ => Err(REFUSAL.into()),
        }
    }

    fn flag(&mut self, key: &str, expected: bool) -> Result<(), String> {
        if matches!(self.value(key)?, Expr::Lit(value) if matches!(&value.lit, Lit::Bool(flag) if flag.value == expected))
        {
            Ok(())
        } else {
            Err(REFUSAL.into())
        }
    }

    fn number(&mut self, key: &str, expected: usize) -> Result<(), String> {
        if matches!(self.value(key)?, Expr::Lit(value) if matches!(&value.lit, Lit::Int(number) if number.base10_parse::<usize>().ok() == Some(expected)))
        {
            Ok(())
        } else {
            Err(REFUSAL.into())
        }
    }

    fn name(&mut self, key: &str, expected: &str) -> Result<(), String> {
        if matches!(self.value(key)?, Expr::Path(path) if path.qself.is_none() && path.path.is_ident(expected))
        {
            Ok(())
        } else {
            Err(REFUSAL.into())
        }
    }

    fn finish(self) -> Result<(), String> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(REFUSAL.into())
        }
    }
}

struct Formats(BTreeMap<String, syn::Path>);

impl Parse for Formats {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut formats = BTreeMap::new();
        while !input.is_empty() {
            let name: syn::LitStr = input.parse()?;
            input.parse::<Token![=>]>()?;
            let path = input.parse()?;
            if formats.insert(name.value(), path).is_some() {
                return Err(syn::Error::new(name.span(), REFUSAL));
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self(formats))
    }
}

fn parse<T: Parse>(tokens: TokenStream) -> Result<T, String> {
    syn::parse2(tokens).map_err(|_error| REFUSAL.into())
}

pub(super) fn check(
    tokens: TokenStream,
    path: &Path,
    schema: &Value,
) -> Result<Vec<String>, String> {
    let audit = audit_schema(schema).map_err(|_error| REFUSAL.to_owned())?;
    if !audit.unknown_keywords.is_empty()
        || !audit.local_references.is_empty()
        || !audit.external_references.is_empty()
    {
        return Err(REFUSAL.into());
    }
    let mut settings: Settings = parse(tokens)?;
    let Expr::Lit(literal) = settings.value("path")? else {
        return Err(REFUSAL.into());
    };
    if !matches!(literal.lit, Lit::Str(value) if Path::new(&value.value()) == path) {
        return Err(REFUSAL.into());
    }
    settings.name(
        "draft",
        match audit.draft {
            Draft::Draft4 => "Draft4",
            Draft::Draft202012 => "Draft202012",
            Draft::Unspecified | Draft::Other(_) => return Err(REFUSAL.into()),
        },
    )?;
    settings.flag("validate_formats", true)?;
    settings.flag("ignore_unknown_formats", false)?;
    let Formats(formats) = parse(settings.group("formats")?)?;
    let expected: BTreeSet<_> = audit.formats.iter().map(|row| row.value.as_str()).collect();
    if formats.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(REFUSAL.into());
    }
    for (name, callback) in &formats {
        if !matches!(name.as_str(), "hostname" | "uuid")
            || callback.leading_colon.is_some()
            || !callback
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .eq(["crate", "schema", "staging", "formats", name.as_str()])
            || callback
                .segments
                .iter()
                .any(|part| !part.arguments.is_empty())
        {
            return Err(REFUSAL.into());
        }
    }
    let mut pattern: Settings = parse(settings.group("pattern_options")?)?;
    pattern.name("engine", "regex")?;
    pattern.name("cache", "explicit")?;
    pattern.number("size_limit", 0x0001_0000_usize)?;
    pattern.number("dfa_size_limit", 0x0001_0000_usize)?;
    pattern.finish()?;
    let mut methods: Settings = parse(settings.group("methods")?)?;
    methods.flag("is_valid", true)?;
    methods.flag("validate", false)?;
    methods.flag("iter_errors", false)?;
    methods.flag("evaluate", false)?;
    methods.finish()?;
    settings.finish()?;
    Ok(formats.into_keys().collect())
}

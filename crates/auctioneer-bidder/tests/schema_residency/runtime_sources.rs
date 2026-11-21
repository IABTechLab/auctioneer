//! Fresh runtime-owner receipts. No mirrored test format selects this program.

use super::{descriptor::Refusal, expanded::digest};
use quote::ToTokens as _;
use regex_automata::{MatchKind, meta::Config};
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Component, Path, PathBuf};

pub(super) const STAGING: &str = "crates/auctioneer-bidder/src/schema/staging.rs";
const NATIVE: &str = "crates/auctioneer-bidder/src/schema/native.rs";
const MAX_BYTES: u64 = 1_u64 << 20_u32;
const STAGING_PIN: &str = "81c3809c1c0145d117f5d672557cece154f53d20ae014dc908d99e6b8aaeda5d";
const NATIVE_PIN: &str = "99f830c2ebc7e0d0a697f0cdf07c4bfe97c31c2b07b1cc0eeee1a5dda331d376";
pub(super) const HOSTNAME: &str = "schema::staging::formats::HOSTNAME_PATTERN";
pub(super) const COORDINATOR: &str = "schema::native::INITIALIZED";

#[derive(Debug)]
pub(super) struct Witness {
    workspace: PathBuf,
    pub hostname: String,
    pub config: Config,
    pub staging_sha256: String,
    pub native_sha256: String,
}

fn read_regular(path: &Path) -> Result<Vec<u8>, Refusal> {
    if !path.is_absolute() {
        return Err(Refusal::RuntimeSource("nonabsolute source"));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        if !matches!(component, Component::RootDir | Component::Normal(_)) {
            return Err(Refusal::RuntimeSource("nonnormal source"));
        }
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_error| Refusal::RuntimeSource("source metadata"))?;
        if (current == path && !metadata.is_file()) || (current != path && !metadata.is_dir()) {
            return Err(Refusal::RuntimeSource("nonregular source or ancestor"));
        }
    }
    if current.as_os_str() != path.as_os_str() {
        return Err(Refusal::RuntimeSource("nonnormal source"));
    }
    let file = File::open(path).map_err(|_error| Refusal::RuntimeSource("source open"))?;
    let metadata = file
        .metadata()
        .map_err(|_error| Refusal::RuntimeSource("source metadata"))?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return Err(Refusal::RuntimeSource("source byte cap"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1_u64)
        .read_to_end(&mut bytes)
        .map_err(|_error| Refusal::RuntimeSource("source read"))?;
    if u64::try_from(bytes.len()).map_err(|_error| Refusal::Arithmetic)? > MAX_BYTES {
        return Err(Refusal::RuntimeSource("source byte cap"));
    }
    Ok(bytes)
}

fn syntax(bytes: &[u8], expected: &str) -> Result<syn::File, Refusal> {
    // Pin the entire owning source, not selected comments or named functions.
    // Unknown statics, cfgs, callbacks, guards and coordinator changes all refuse.
    if digest(bytes) != expected {
        return Err(Refusal::RuntimeSource("unreviewed runtime source"));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_error| Refusal::RuntimeSource("runtime UTF-8"))?;
    syn::parse_file(text).map_err(|_error| Refusal::RuntimeSource("runtime AST"))
}

fn bind_format_input(manifest: &Value, sha256: &str) -> Result<(), Refusal> {
    let rows = manifest["identity"]["inputs"]
        .as_array()
        .ok_or(Refusal::RuntimeSource("missing format source input"))?;
    let matching = rows
        .iter()
        .filter(|row| row["kind"] == "FormatHelperSource")
        .collect::<Vec<_>>();
    let [row] = matching.as_slice() else {
        return Err(Refusal::RuntimeSource("format source input coverage"));
    };
    if row["id"] != STAGING || row["sha256"] != sha256 {
        return Err(Refusal::RuntimeSource("stale format source input"));
    }
    Ok(())
}

fn hostname_literal(file: &syn::File) -> Result<String, Refusal> {
    let modules = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Mod(module) if module.ident == "formats" => Some(module),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [module] = modules.as_slice() else {
        return Err(Refusal::RuntimeSource("formats module coverage"));
    };
    let (_, items) = module
        .content
        .as_ref()
        .ok_or(Refusal::RuntimeSource("formats module body"))?;
    let literals = items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Const(item) if item.ident == "HOSTNAME_SOURCE" => Some(item.expr.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [syn::Expr::Lit(literal)] = literals.as_slice() else {
        return Err(Refusal::RuntimeSource("hostname literal coverage"));
    };
    let syn::Lit::Str(pattern) = &literal.lit else {
        return Err(Refusal::RuntimeSource("hostname literal"));
    };
    Ok(pattern.value())
}

fn method<'a>(expr: &'a syn::Expr, name: &str) -> Result<&'a syn::ExprMethodCall, Refusal> {
    match expr {
        syn::Expr::MethodCall(call) if call.method == name => Ok(call),
        _ => Err(Refusal::RuntimeSource("hostname constructor AST")),
    }
}

fn argument(call: &syn::ExprMethodCall) -> Result<&syn::Expr, Refusal> {
    if call.args.len() != 1_usize {
        return Err(Refusal::RuntimeSource("hostname constructor arity"));
    }
    call.args
        .first()
        .ok_or(Refusal::RuntimeSource("hostname constructor argument"))
}

fn integer(expr: &syn::Expr) -> Result<usize, Refusal> {
    if let syn::Expr::Lit(literal) = expr
        && let syn::Lit::Int(value) = &literal.lit
    {
        return value
            .base10_parse()
            .map_err(|_error| Refusal::RuntimeSource("hostname integer"));
    }
    Err(Refusal::RuntimeSource("hostname integer"))
}

fn hostname_config(file: &syn::File) -> Result<Config, Refusal> {
    let storage = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Mod(module) if module.ident == "formats" => {
                module.content.as_ref().and_then(|(_, items)| {
                    items.iter().find_map(|item| match item {
                        syn::Item::Static(item) if item.ident == "HOSTNAME_PATTERN" => Some(item),
                        _ => None,
                    })
                })
            }
            _ => None,
        })
        .ok_or(Refusal::RuntimeSource("hostname cell"))?;
    let syn::Expr::Call(new) = storage.expr.as_ref() else {
        return Err(Refusal::RuntimeSource("hostname lazy constructor"));
    };
    let Some(syn::Expr::Closure(closure)) = new.args.first() else {
        return Err(Refusal::RuntimeSource("hostname lazy closure"));
    };
    let syn::Expr::Block(body) = closure.body.as_ref() else {
        return Err(Refusal::RuntimeSource("hostname lazy body"));
    };
    let [syn::Stmt::Expr(result, None)] = body.block.stmts.as_slice() else {
        return Err(Refusal::RuntimeSource("hostname constructor statements"));
    };
    let ok = method(result, "ok")?;
    let build = method(&ok.receiver, "build")?;
    let syntax = method(&build.receiver, "syntax")?;
    let configure = method(&syntax.receiver, "configure")?;
    let pool = method(argument(configure)?, "pool_capacity")?;
    let hybrid = method(&pool.receiver, "hybrid_cache_capacity")?;
    let nfa = method(&hybrid.receiver, "nfa_size_limit")?;
    let syn::Expr::Call(some) = argument(nfa)? else {
        return Err(Refusal::RuntimeSource("hostname NFA limit"));
    };
    let limit = integer(
        some.args
            .first()
            .ok_or(Refusal::RuntimeSource("hostname NFA limit"))?,
    )?;
    let utf8 = method(&nfa.receiver, "utf8_empty")?;
    let syn::Expr::Lit(literal) = argument(utf8)? else {
        return Err(Refusal::RuntimeSource("hostname UTF-8"));
    };
    let syn::Lit::Bool(enabled) = &literal.lit else {
        return Err(Refusal::RuntimeSource("hostname UTF-8"));
    };
    // Complete source authentication also binds Regex::config defaults, the
    // match kind, syntax configuration, builder target and callback bodies.
    Ok(Config::new()
        .match_kind(MatchKind::LeftmostFirst)
        .utf8_empty(enabled.value)
        .nfa_size_limit(Some(limit))
        .hybrid_cache_capacity(integer(argument(hybrid)?)?)
        .pool_capacity(integer(argument(pool)?)?))
}

/// Shared production-source order check; independent graph oracles stay local.
pub(super) fn initialization_order(source: &[u8]) -> Result<(), String> {
    let refused = "native format initialization order mismatch";
    let text = std::str::from_utf8(source).map_err(|_error| refused)?;
    let file = syn::parse_file(text).map_err(|_error| refused)?;
    let functions = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == "initialize_native" => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [function] = functions.as_slice() else {
        return Err(refused.into());
    };
    let expected: syn::ItemFn = syn::parse_quote! {
        #[cfg(not(target_arch = "wasm32"))]
        pub(super) fn initialize_native() -> bool {
            generated::retain_for_inspection();
            formats::initialize() && generated::initialize_native()
        }
    };
    if function.to_token_stream().to_string() != expected.to_token_stream().to_string() {
        return Err(refused.into());
    }
    Ok(())
}

pub(super) fn capture(workspace: &Path, manifest: &Value) -> Result<Witness, Refusal> {
    let staging = read_regular(&workspace.join(STAGING))?;
    let native = read_regular(&workspace.join(NATIVE))?;
    initialization_order(&staging)
        .map_err(|_error| Refusal::RuntimeSource("native format initialization order"))?;
    let staging_ast = syntax(&staging, STAGING_PIN)?;
    let _native_ast = syntax(&native, NATIVE_PIN)?;
    let staging_sha256 = digest(&staging);
    bind_format_input(manifest, &staging_sha256)?;
    let hostname = hostname_literal(&staging_ast)?;
    let config = hostname_config(&staging_ast)?;
    Ok(Witness {
        workspace: workspace.to_owned(),
        hostname,
        config,
        staging_sha256,
        native_sha256: digest(&native),
    })
}

impl Witness {
    pub(super) fn recheck(&self) -> Result<(), Refusal> {
        let staging = read_regular(&self.workspace.join(STAGING))?;
        let native = read_regular(&self.workspace.join(NATIVE))?;
        syntax(&staging, &self.staging_sha256)?;
        syntax(&native, &self.native_sha256)?;
        Ok(())
    }

    pub(super) fn report(&self) -> Value {
        json!({"staging":{"path":STAGING,"sha256":self.staging_sha256},
            "native":{"path":NATIVE,"sha256":self.native_sha256},
            "scope":"entire owning sources, including formats, guards, cells and coordinator",
            "schedule":"funding/staging guards before OnceLock; hostname before generated hooks",
            "concurrency":"one native coordinator; not a portable guest guarantee"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    const STAGING_BYTES: &[u8] = include_bytes!("../../src/schema/staging.rs");
    const NATIVE_BYTES: &[u8] = include_bytes!("../../src/schema/native.rs");

    #[test]
    fn actual_sources_authenticate_full_ast_and_actual_hostname_literal() {
        assert_eq!(initialization_order(STAGING_BYTES), Ok(()));
        let file = syntax(STAGING_BYTES, STAGING_PIN).unwrap();
        let pattern = hostname_literal(&file).unwrap();
        assert!(pattern.starts_with("^([a-zA-Z0-9]"));
        assert!(pattern.ends_with("))*$"));
        assert!(syntax(NATIVE_BYTES, NATIVE_PIN).is_ok());
        for bytes in [STAGING_BYTES, NATIVE_BYTES] {
            let mut extra = bytes.to_vec();
            extra.extend_from_slice(b"\nstatic UNKNOWN_NATIVE_STATIC: bool = false;\n");
            assert_eq!(
                syntax(&extra, &digest(bytes)).err(),
                Some(Refusal::RuntimeSource("unreviewed runtime source"))
            );
        }
    }

    #[test]
    fn callbacks_builder_order_guards_and_cell_changes_refuse_even_with_updated_input_hash() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().canonicalize().unwrap();
        fs::create_dir_all(workspace.join(STAGING).parent().unwrap()).unwrap();
        fs::write(workspace.join(STAGING), STAGING_BYTES).unwrap();
        fs::write(workspace.join(NATIVE), NATIVE_BYTES).unwrap();
        let original_manifest = json!({"identity":{"inputs":[{
            "kind":"FormatHelperSource","id":STAGING,"sha256":STAGING_PIN
        }]}});
        assert!(capture(&workspace, &original_manifest).is_ok());
        for (bytes, before, after) in [
            (STAGING_BYTES, "value.len() < 256", "value.len() < 257"),
            (STAGING_BYTES, ".pool_capacity(1)", ".pool_capacity(2)"),
            (
                STAGING_BYTES,
                "formats::initialize() && generated::initialize_native()",
                "generated::initialize_native() && formats::initialize()",
            ),
            (
                STAGING_BYTES,
                "pub(super) fn uuid",
                "pub(super) fn changed_uuid",
            ),
            (
                NATIVE_BYTES,
                "!native_schema_funded(costs) || staging_only",
                "staging_only",
            ),
            (
                NATIVE_BYTES,
                "*cell.get_or_init(initialize)",
                "initialize()",
            ),
        ] {
            let text = std::str::from_utf8(bytes).unwrap();
            assert!(text.contains(before));
            let changed = text.replacen(before, after, 1_usize);
            assert_eq!(
                syntax(changed.as_bytes(), &digest(bytes)).err(),
                Some(Refusal::RuntimeSource("unreviewed runtime source"))
            );
            let path = if bytes == STAGING_BYTES {
                STAGING
            } else {
                NATIVE
            };
            fs::write(workspace.join(path), &changed).unwrap();
            let mut rebound = original_manifest.clone();
            if path == STAGING {
                rebound["identity"]["inputs"][0]["sha256"] = json!(digest(changed.as_bytes()));
            }
            let reason = if before == "formats::initialize() && generated::initialize_native()" {
                "native format initialization order"
            } else {
                "unreviewed runtime source"
            };
            assert_eq!(
                capture(&workspace, &rebound).err(),
                Some(Refusal::RuntimeSource(reason)),
                "{before}"
            );
            fs::write(workspace.join(path), bytes).unwrap();
        }
    }

    #[test]
    fn format_input_is_required_unique_and_byte_matched() {
        let row = json!({"kind":"FormatHelperSource","id":STAGING,"sha256":STAGING_PIN});
        let manifest = json!({"identity":{"inputs":[row.clone()]}});
        assert_eq!(bind_format_input(&manifest, STAGING_PIN), Ok(()));
        for inputs in [
            json!([]),
            json!([row.clone(), row]),
            json!([{"kind":"FormatHelperSource","id":STAGING,"sha256":"stale"}]),
        ] {
            assert!(
                bind_format_input(&json!({"identity":{"inputs":inputs}}), STAGING_PIN).is_err()
            );
        }
    }

    #[test]
    fn bounded_reader_refuses_symlink_ancestors_traversal_and_oversized_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join("real")).unwrap();
        fs::write(root.join("real/source.rs"), b"fn actual() {}\n").unwrap();
        assert_eq!(
            read_regular(&root.join("real/source.rs")).unwrap(),
            b"fn actual() {}\n"
        );
        symlink(root.join("real"), root.join("alias")).unwrap();
        assert!(read_regular(&root.join("alias/source.rs")).is_err());
        assert!(read_regular(&root.join("real/../real/source.rs")).is_err());
        let file = File::create(root.join("large.rs")).unwrap();
        file.set_len(MAX_BYTES + 1_u64).unwrap();
        assert!(read_regular(&root.join("large.rs")).is_err());
        assert!(read_regular(&root.join("real")).is_err());
    }
}

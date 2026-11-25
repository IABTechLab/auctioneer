//! Shared test DATA only; source reads and clauses are not artifact authority.

use super::{
    Dispatch, FeatureClause, FeatureData, Roots, ScalarData, SchemaInputLimits, SourceData,
    SourceUnit, derive_scalar_sources,
};
use crate::schema_build::selected::probe_compiler;
use crate::schema_residency::read_regular;
use crate::schema_validation::work::context::check_sources;
use crate::schema_validation::work::{
    BUDGET, ContextSources, DISPATCH_GENERATOR, EXCHANGE_LIMITS, EXCHANGE_RUN, EXCHANGE_SCHEMA,
    EXCHANGE_SPLIT, FORMATS, INPUT, JSON_SIZE, NATIVE,
};
use quote::ToTokens as _;
use sha2::{Digest as _, Sha256};
use std::env::var_os;
use std::path::{Path, PathBuf};
use std::str::from_utf8;

#[derive(Clone, Copy)]
pub(super) enum Root {
    Workspace,
    Registry(&'static str, &'static str),
    RustLibrary,
}

pub(super) const SOURCES: [(SourceUnit, Root, &str); 20] = [
    (
        SourceUnit::Native,
        Root::Workspace,
        "crates/auctioneer-bidder/src/schema/native.rs",
    ),
    (
        SourceUnit::Input,
        Root::Workspace,
        "crates/auctioneer-bidder/src/schema/input.rs",
    ),
    (
        SourceUnit::JsonSize,
        Root::Workspace,
        "crates/auctioneer-runtime/src/json_size.rs",
    ),
    (
        SourceUnit::SerdeFacade,
        Root::Registry("serde", "1.0.229"),
        "src/lib.rs",
    ),
    (
        SourceUnit::SerdeCoreRoot,
        Root::Registry("serde_core", "1.0.229"),
        "src/lib.rs",
    ),
    (
        SourceUnit::SerdeCoreExports,
        Root::Registry("serde_core", "1.0.229"),
        "src/crate_root.rs",
    ),
    (
        SourceUnit::SerdeTraits,
        Root::Registry("serde_core", "1.0.229"),
        "src/ser/mod.rs",
    ),
    (
        SourceUnit::SerdeStr,
        Root::Registry("serde_core", "1.0.229"),
        "src/ser/impls.rs",
    ),
    (
        SourceUnit::JsonRoot,
        Root::Registry("serde_json", "1.0.150"),
        "src/lib.rs",
    ),
    (
        SourceUnit::JsonIo,
        Root::Registry("serde_json", "1.0.150"),
        "src/io/mod.rs",
    ),
    (
        SourceUnit::JsonSerializer,
        Root::Registry("serde_json", "1.0.150"),
        "src/ser.rs",
    ),
    (
        SourceUnit::JsonValue,
        Root::Registry("serde_json", "1.0.150"),
        "src/value/mod.rs",
    ),
    (
        SourceUnit::JsonValueSerialize,
        Root::Registry("serde_json", "1.0.150"),
        "src/value/ser.rs",
    ),
    (
        SourceUnit::JsonNumber,
        Root::Registry("serde_json", "1.0.150"),
        "src/number.rs",
    ),
    (SourceUnit::IoWrite, Root::RustLibrary, "std/src/io/mod.rs"),
    (
        SourceUnit::IoForward,
        Root::RustLibrary,
        "std/src/io/impls.rs",
    ),
    (
        SourceUnit::StrBytes,
        Root::RustLibrary,
        "core/src/str/mod.rs",
    ),
    (
        SourceUnit::Slice,
        Root::RustLibrary,
        "core/src/slice/mod.rs",
    ),
    (
        SourceUnit::SliceIndex,
        Root::RustLibrary,
        "core/src/slice/index.rs",
    ),
    (
        SourceUnit::Default,
        Root::RustLibrary,
        "core/src/default.rs",
    ),
];

pub(super) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(super) fn source_path(roots: &Roots, root: Root, relative: &str) -> PathBuf {
    match root {
        Root::Workspace => roots.workspace.join(relative),
        Root::Registry(package, version) => roots
            .registry
            .join(format!("{package}-{version}"))
            .join(relative),
        Root::RustLibrary => roots.library.join(relative),
    }
}

pub(in crate::schema_validation::work) fn fixture() -> (ScalarData, SchemaInputLimits) {
    let (compiler, verbose) = probe_compiler().expect("existing bounded compiler probe");
    assert!(!verbose.is_empty());
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace")
        .to_owned();
    // Cargo's standard home selection chooses one root. No alternate
    // registry index, source version, discovery walk or download is tried.
    let home = var_os("CARGO_HOME").map_or_else(
        || PathBuf::from(var_os("HOME").expect("Cargo default home")).join(".cargo"),
        PathBuf::from,
    );
    let roots = Roots {
        workspace,
        registry: home.join("registry/src/index.crates.io-1949cf8c6b5b557f"),
        library: compiler
            .parent()
            .and_then(Path::parent)
            .expect("compiler sysroot")
            .join("lib/rustlib/src/rust/library"),
    };
    assert_locked_packages(&roots);
    let mut sources = Vec::new();
    for (unit, root, relative) in SOURCES {
        let path = source_path(&roots, root, relative);
        let before = read_regular(&path).expect("existing bounded regular source reader");
        let after = read_regular(&path).expect("existing bounded regular source reread");
        assert_eq!(before, after);
        sources.push(SourceData {
            unit,
            path,
            sha256: digest(&before),
            before,
            after,
        });
    }
    let data = ScalarData {
        roots,
        sources,
        // Conditional clauses, not the effective features of an artifact.
        features: FeatureData {
            serde_std: Some(true),
            serde_core_std: Some(true),
            json_std: Some(true),
            arbitrary_precision: Some(false),
            docsrs: Some(false),
        },
        pointer_width: Some(64),
        dispatch: Dispatch::NativeScalars,
    };
    (data, native_limits())
}

fn assert_locked_packages(roots: &Roots) {
    let bytes = read_regular(&roots.workspace.join("Cargo.lock")).expect("bounded lock data");
    let lock: toml::Value = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
    let packages = lock.get("package").and_then(toml::Value::as_array).unwrap();
    for (name, version) in [
        ("serde", "1.0.229"),
        ("serde_core", "1.0.229"),
        ("serde_json", "1.0.150"),
    ] {
        let matching: Vec<_> = packages
            .iter()
            .filter(|row| row.get("name").and_then(toml::Value::as_str) == Some(name))
            .collect();
        assert_eq!(matching.len(), 1);
        let row = matching.first().unwrap();
        assert_eq!(
            row.get("version").and_then(toml::Value::as_str),
            Some(version)
        );
        assert_eq!(
            row.get("source").and_then(toml::Value::as_str),
            Some("registry+https://github.com/rust-lang/crates.io-index")
        );
        let path = roots
            .registry
            .join(format!("{name}-{version}"))
            .join("Cargo.toml");
        let manifest_bytes = read_regular(&path).expect("exact locked manifest data");
        let manifest: toml::Value = toml::from_str(from_utf8(&manifest_bytes).unwrap()).unwrap();
        let package = manifest.get("package").unwrap();
        assert_eq!(
            package.get("name").and_then(toml::Value::as_str),
            Some(name)
        );
        assert_eq!(
            package.get("version").and_then(toml::Value::as_str),
            Some(version)
        );
    }
    // These checks identify fixture bytes. They do not authenticate the
    // registry archive or establish the compiled library dependency graph.
}

fn native_limits() -> SchemaInputLimits {
    let sources = ContextSources {
        native: NATIVE,
        budget: BUDGET,
        input: INPUT,
        json_size: JSON_SIZE,
        exchange_schema: EXCHANGE_SCHEMA,
        exchange_run: EXCHANGE_RUN,
        exchange_split: EXCHANGE_SPLIT,
        exchange_limits: EXCHANGE_LIMITS,
        dispatch_generator: DISPATCH_GENERATOR,
        formats: FORMATS,
    };
    let limits = check_sources(&sources).expect("existing source-only recipes");
    assert_eq!(limits.serialized_bytes, 65_536);
    assert_eq!(limits.nodes, 4_096);
    assert_eq!(limits.depth, 32);
    limits
}

pub(super) fn syntax(bytes: &[u8]) -> syn::File {
    syn::parse_file(from_utf8(bytes).unwrap()).unwrap()
}

pub(super) fn row(data: &ScalarData, unit: SourceUnit) -> &SourceData {
    let matching: Vec<_> = data
        .sources
        .iter()
        .filter(|source| source.unit == unit)
        .collect();
    assert_eq!(matching.len(), 1);
    matching.first().copied().unwrap()
}

pub(super) fn implementation_body<'a>(
    file: &'a mut syn::File,
    owner: &syn::Type,
    method: &str,
) -> &'a mut syn::Block {
    let mut matching = Vec::new();
    for item in &mut file.items {
        if let syn::Item::Impl(implementation) = item
            && implementation.self_ty.to_token_stream().to_string()
                == owner.to_token_stream().to_string()
        {
            for member in &mut implementation.items {
                if let syn::ImplItem::Fn(function) = member
                    && function.sig.ident == method
                {
                    matching.push(&mut function.block);
                }
            }
        }
    }
    assert_eq!(matching.len(), 1, "one concrete impl method: {method}");
    matching.pop().unwrap()
}

pub(in crate::schema_validation::work) fn trait_body<'a>(
    file: &'a mut syn::File,
    owner: &str,
    method: &str,
) -> &'a mut syn::Block {
    let mut matching = Vec::new();
    for item in &mut file.items {
        if let syn::Item::Trait(definition) = item
            && definition.ident == owner
        {
            for member in &mut definition.items {
                if let syn::TraitItem::Fn(function) = member
                    && function.sig.ident == method
                {
                    matching.push(function.default.as_mut().expect("provided method"));
                }
            }
        }
    }
    assert_eq!(matching.len(), 1, "one provided trait method: {method}");
    matching.pop().unwrap()
}

pub(in crate::schema_validation::work) fn changed(
    data: &ScalarData,
    unit: SourceUnit,
    edit: impl FnOnce(&mut syn::File),
) -> ScalarData {
    let mut updated = data.clone();
    let source = updated
        .sources
        .iter_mut()
        .find(|source| source.unit == unit)
        .unwrap();
    let original = source.before.clone();
    let mut file = syntax(&source.before);
    edit(&mut file);
    source.before = file.to_token_stream().to_string().into_bytes();
    source.after.clone_from(&source.before);
    source.sha256 = digest(&source.before);
    assert_ne!(source.before, original);
    assert_eq!(source.before, source.after);
    assert_eq!(source.sha256, digest(&source.before));
    updated
}

pub(super) fn feature_slot(features: &mut FeatureData, clause: FeatureClause) -> &mut Option<bool> {
    match clause {
        FeatureClause::SerdeStd => &mut features.serde_std,
        FeatureClause::SerdeCoreStd => &mut features.serde_core_std,
        FeatureClause::JsonStd => &mut features.json_std,
        FeatureClause::ArbitraryPrecision => &mut features.arbitrary_precision,
        FeatureClause::Docsrs => &mut features.docsrs,
    }
}

// These mutators create explicit synthetic DATA controls. They do not reread
// sources, claim an authenticated source window or construct an artifact fact.
pub(in crate::schema_validation::work) fn units() -> [SourceUnit; 20] {
    SOURCES.map(|(unit, _, _)| unit)
}

pub(in crate::schema_validation::work) fn assert_pristine(
    data: &ScalarData,
    limits: SchemaInputLimits,
) {
    assert_eq!(data.sources.len(), 20);
    for (unit, expected) in [
        (SourceUnit::Native, NATIVE),
        (SourceUnit::Input, INPUT),
        (SourceUnit::JsonSize, JSON_SIZE),
    ] {
        let source = row(data, unit);
        assert_eq!(source.before, expected);
        assert_eq!(source.before, source.after);
        assert_eq!(source.sha256, digest(&source.before));
    }
    let result = derive_scalar_sources(data, limits, 0);
    assert_eq!(result.as_ref().map(|_| ()), Ok(()));
    let contract = result.unwrap();
    assert_eq!(contract.raw_utf8_with_quotes_upper, 0);
    assert_eq!(contract.string_virtual_bytes_upper, 393_206);
    assert_eq!(contract.writes_per_nonempty_request_upper, 1);
    assert_eq!(contract.writes_per_empty_request_upper, 0);
    assert_eq!(contract.interrupted_retries_upper, 0);
    assert_eq!(contract.library_artifact_binding, None);
}

pub(in crate::schema_validation::work) fn different_input_bytes(
    data: &ScalarData,
    unit: SourceUnit,
) -> ScalarData {
    assert!(matches!(
        unit,
        SourceUnit::Native | SourceUnit::Input | SourceUnit::JsonSize
    ));
    let original = row(data, unit);
    assert_eq!(original.before, original.after);
    let mut updated = data.clone();
    let source = updated
        .sources
        .iter_mut()
        .find(|source| source.unit == unit)
        .unwrap();
    source.before.push(b' ');
    source.after.clone_from(&source.before);
    source.sha256 = digest(&source.before);
    assert_eq!(source.path, original.path);
    assert_ne!(source.before, original.before);
    assert_eq!(source.before, source.after);
    assert_eq!(source.sha256, digest(&source.before));
    assert_ne!(source.sha256, original.sha256);
    assert_eq!(
        syntax(&source.before).to_token_stream().to_string(),
        syntax(&original.before).to_token_stream().to_string()
    );
    updated
}

pub(in crate::schema_validation::work) fn without_source(
    data: &ScalarData,
    unit: SourceUnit,
) -> ScalarData {
    let original = row(data, unit);
    let mut updated = data.clone();
    let position = updated
        .sources
        .iter()
        .position(|source| source.unit == unit)
        .unwrap();
    assert_eq!(updated.sources.remove(position), *original);
    assert_eq!(
        updated.sources.len().checked_add(1),
        Some(data.sources.len())
    );
    updated
}

pub(in crate::schema_validation::work) fn duplicate_source(
    data: &ScalarData,
    unit: SourceUnit,
) -> ScalarData {
    let original = row(data, unit).clone();
    let mut updated = data.clone();
    updated.sources.push(original);
    assert_eq!(
        updated.sources.len(),
        data.sources.len().checked_add(1).unwrap()
    );
    updated
}

pub(in crate::schema_validation::work) fn changed_record(
    data: &ScalarData,
    unit: SourceUnit,
) -> ScalarData {
    let original = row(data, unit);
    assert_eq!(original.before, original.after);
    let mut updated = data.clone();
    let source = updated
        .sources
        .iter_mut()
        .find(|source| source.unit == unit)
        .unwrap();
    source.before.push(b' ');
    assert_eq!(source.after, original.after);
    assert_eq!(source.sha256, original.sha256);
    assert_ne!(source.before, source.after);
    updated
}

pub(in crate::schema_validation::work) fn wrong_path(
    data: &ScalarData,
    unit: SourceUnit,
) -> ScalarData {
    let original = row(data, unit);
    let mut updated = data.clone();
    let source = updated
        .sources
        .iter_mut()
        .find(|source| source.unit == unit)
        .unwrap();
    source.path.set_file_name("fake_wrong_scalar_source.rs");
    assert_ne!(source.path, original.path);
    assert_eq!(source.before, original.before);
    assert_eq!(source.after, original.after);
    assert_eq!(source.sha256, original.sha256);
    updated
}

pub(in crate::schema_validation::work) fn feature_value(
    data: &ScalarData,
    clause: FeatureClause,
    value: Option<bool>,
) -> ScalarData {
    let mut updated = data.clone();
    let slot = feature_slot(&mut updated.features, clause);
    assert_ne!(*slot, value);
    *slot = value;
    updated
}

pub(in crate::schema_validation::work) fn width_value(
    data: &ScalarData,
    width: Option<u16>,
) -> ScalarData {
    assert_ne!(data.pointer_width, width);
    let mut updated = data.clone();
    updated.pointer_width = width;
    updated
}

pub(in crate::schema_validation::work) fn dispatch_value(
    data: &ScalarData,
    dispatch: Dispatch,
) -> ScalarData {
    assert_ne!(data.dispatch, dispatch);
    let mut updated = data.clone();
    updated.dispatch = dispatch;
    updated
}

pub(in crate::schema_validation::work) fn missing_string_framing(data: &ScalarData) -> ScalarData {
    changed(data, SourceUnit::JsonSerializer, |file| {
        let begin = trait_body(file, "Formatter", "begin_string");
        let original: syn::Block = syn::parse_quote!({ writer.write_all(b"\"") });
        assert_eq!(
            begin.to_token_stream().to_string(),
            original.to_token_stream().to_string()
        );
        *begin = syn::parse_quote!({ writer.write_all(b"") });
    })
}

pub(in crate::schema_validation::work) fn skipped_write_loop(data: &ScalarData) -> ScalarData {
    changed(data, SourceUnit::IoWrite, |file| {
        let body = trait_body(file, "Write", "write_all");
        let syn::Stmt::Expr(syn::Expr::While(loop_), None) = body.stmts.first_mut().unwrap() else {
            panic!("matched Write loop");
        };
        let expected: syn::Expr = syn::parse_quote!(!buf.is_empty());
        assert_eq!(
            loop_.cond.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
        *loop_.cond = syn::parse_quote!(false);
    })
}

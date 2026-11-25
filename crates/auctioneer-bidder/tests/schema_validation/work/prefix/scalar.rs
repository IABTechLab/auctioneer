//! Tests-first concrete string/Counter semantics, not source or artifact authority.
//!
//! Source snapshots, roots, feature clauses, dispatch and pointer width are DATA.
//! Actual use still requires the existing registry authentication and bracketed
//! source-window owner, same-artifact library features and compiler/builtin
//! binding. Fixture equality does not establish any of those joins.
//! Only native `str` and `Value::{Null,Bool,Number}` calls with
//! `CompactFormatter` are in scope.
//! Raw bounds count strings/keys passing the precheck and reaching serialization,
//! including a final serialization whose byte charge fails, not rejected operands.
//! Emitted lengths are virtual Counter bytes, not copied/scanned work. Numeric
//! formatter safety/termination/work, map navigation, ordinary error/unwind
//! cleanup and matched frame facts remain missing. No generic Serialize claim
//! or new error-service exclusion follows from a concrete Counter schedule.

use crate::schema_validation::work::SchemaInputLimits;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SourceUnit {
    Native,
    Input,
    JsonSize,
    SerdeFacade,
    SerdeCoreRoot,
    SerdeCoreExports,
    SerdeTraits,
    SerdeStr,
    JsonRoot,
    JsonIo,
    JsonSerializer,
    JsonValue,
    JsonValueSerialize,
    JsonNumber,
    IoWrite,
    IoForward,
    StrBytes,
    Slice,
    SliceIndex,
    Default,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceData {
    unit: SourceUnit,
    path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Roots {
    workspace: PathBuf,
    registry: PathBuf,
    library: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FeatureClause {
    SerdeStd,
    SerdeCoreStd,
    JsonStd,
    ArbitraryPrecision,
    Docsrs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FeatureData {
    serde_std: Option<bool>,
    serde_core_std: Option<bool>,
    json_std: Option<bool>,
    arbitrary_precision: Option<bool>,
    docsrs: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dispatch {
    NativeScalars,
    GenericSerialize,
    OtherWriter,
    PrettyFormatter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScalarData {
    roots: Roots,
    sources: Vec<SourceData>,
    features: FeatureData,
    pointer_width: Option<u16>,
    dispatch: Dispatch,
}

#[derive(Debug, Eq, PartialEq)]
struct ScalarContract {
    raw_utf8_with_quotes_upper: u64,
    string_virtual_bytes_upper: u64,
    writes_per_nonempty_request_upper: u64,
    writes_per_empty_request_upper: u64,
    interrupted_retries_upper: u64,
    required_sources: BTreeSet<SourceUnit>,
    unresolved: Vec<&'static str>,
    library_artifact_binding: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    MissingSourceProof,
    MissingSource(SourceUnit),
    DuplicateSource(SourceUnit),
    SourcePath(SourceUnit),
    SourceChanged(SourceUnit),
    SourceShape(SourceUnit),
    MissingFeature(FeatureClause),
    FeatureMismatch(FeatureClause),
    MissingWidth,
    UnsupportedWidth(u16),
    UnsupportedDispatch,
    Overflow,
}

// No source recognizer, serializer or arithmetic derivation before hosted RED.
// Zero attempts do not erase source/feature/type duties. Per-request protocol
// ceilings describe a conditional request, not an invocation count or peak.
fn derive_scalar_sources(
    _data: &ScalarData,
    _limits: SchemaInputLimits,
    _attempts: u64,
) -> Result<ScalarContract, Refusal> {
    Err(Refusal::MissingSourceProof)
}

#[cfg(test)]
mod tests {
    use super::{
        Dispatch, FeatureClause, FeatureData, Refusal, Roots, ScalarContract, ScalarData,
        SchemaInputLimits, SourceData, SourceUnit, derive_scalar_sources,
    };
    use crate::schema_build::selected::probe_compiler;
    use crate::schema_residency::read_regular;
    use crate::schema_validation::work::context::check_sources;
    use crate::schema_validation::work::{
        BUDGET, ContextSources, DISPATCH_GENERATOR, EXCHANGE_LIMITS, EXCHANGE_RUN, EXCHANGE_SCHEMA,
        EXCHANGE_SPLIT, FORMATS, INPUT, JSON_SIZE, NATIVE, SchemaInputError, SchemaInputStats,
        measure_schema_input,
    };
    use auctioneer_runtime::serialized_json_bytes;
    use quote::{ToTokens as _, quote};
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeSet;
    use std::env::var_os;
    use std::path::{Path, PathBuf};
    use std::str::from_utf8;

    #[derive(Clone, Copy)]
    enum Root {
        Workspace,
        Registry(&'static str, &'static str),
        RustLibrary,
    }

    const SOURCES: [(SourceUnit, Root, &str); 20] = [
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

    fn required_sources() -> BTreeSet<SourceUnit> {
        BTreeSet::from([
            SourceUnit::Native,
            SourceUnit::Input,
            SourceUnit::JsonSize,
            SourceUnit::SerdeFacade,
            SourceUnit::SerdeCoreRoot,
            SourceUnit::SerdeCoreExports,
            SourceUnit::SerdeTraits,
            SourceUnit::SerdeStr,
            SourceUnit::JsonRoot,
            SourceUnit::JsonIo,
            SourceUnit::JsonSerializer,
            SourceUnit::JsonValue,
            SourceUnit::JsonValueSerialize,
            SourceUnit::JsonNumber,
            SourceUnit::IoWrite,
            SourceUnit::IoForward,
            SourceUnit::StrBytes,
            SourceUnit::Slice,
            SourceUnit::SliceIndex,
            SourceUnit::Default,
        ])
    }

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn source_path(roots: &Roots, root: Root, relative: &str) -> PathBuf {
        match root {
            Root::Workspace => roots.workspace.join(relative),
            Root::Registry(package, version) => roots
                .registry
                .join(format!("{package}-{version}"))
                .join(relative),
            Root::RustLibrary => roots.library.join(relative),
        }
    }

    fn fixture() -> (ScalarData, SchemaInputLimits) {
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
            let manifest: toml::Value =
                toml::from_str(from_utf8(&manifest_bytes).unwrap()).unwrap();
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

    fn syntax(bytes: &[u8]) -> syn::File {
        syn::parse_file(from_utf8(bytes).unwrap()).unwrap()
    }

    fn row(data: &ScalarData, unit: SourceUnit) -> &SourceData {
        let matching: Vec<_> = data
            .sources
            .iter()
            .filter(|source| source.unit == unit)
            .collect();
        assert_eq!(matching.len(), 1);
        matching.first().copied().unwrap()
    }

    fn before(data: &ScalarData) {
        assert!(data.roots.workspace.is_absolute());
        assert!(data.roots.registry.is_absolute());
        assert!(data.roots.library.is_absolute());
        assert_eq!(data.sources.len(), 20);
        assert_eq!(
            data.sources
                .iter()
                .map(|source| source.unit)
                .collect::<BTreeSet<_>>(),
            required_sources()
        );
        for (unit, root, relative) in SOURCES {
            let source = row(data, unit);
            assert_eq!(source.path, source_path(&data.roots, root, relative));
            assert_eq!(source.before, source.after);
            assert_eq!(source.sha256, digest(&source.before));
            assert!(!syntax(&source.before).items.is_empty());
        }
        assert_eq!(row(data, SourceUnit::Native).before, NATIVE);
        assert_eq!(row(data, SourceUnit::Input).before, INPUT);
        assert_eq!(row(data, SourceUnit::JsonSize).before, JSON_SIZE);
        assert_eq!(
            (
                data.features.serde_std,
                data.features.serde_core_std,
                data.features.json_std
            ),
            (Some(true), Some(true), Some(true))
        );
        assert_eq!(
            (data.features.arbitrary_precision, data.features.docsrs),
            (Some(false), Some(false))
        );
        assert_eq!(data.pointer_width, Some(64));
        assert_eq!(data.dispatch, Dispatch::NativeScalars);
        assert_str_dispatch(data);
    }

    fn implementation_body<'a>(
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

    fn trait_body<'a>(file: &'a mut syn::File, owner: &str, method: &str) -> &'a mut syn::Block {
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

    fn assert_str_dispatch(data: &ScalarData) {
        let mut file = syntax(&row(data, SourceUnit::SerdeStr).before);
        let body = implementation_body(&mut file, &syn::parse_quote!(str), "serialize");
        let expected: syn::Block = syn::parse_quote!({ serializer.serialize_str(self) });
        assert_eq!(
            body.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
        let mut serializer = syntax(&row(data, SourceUnit::JsonSerializer).before);
        let begin = trait_body(&mut serializer, "Formatter", "begin_string");
        let quote_body: syn::Block = syn::parse_quote!({ writer.write_all(b"\"") });
        assert_eq!(
            begin.to_token_stream().to_string(),
            quote_body.to_token_stream().to_string()
        );
        let finish = trait_body(&mut serializer, "Formatter", "end_string");
        assert_eq!(
            finish.to_token_stream().to_string(),
            quote_body.to_token_stream().to_string()
        );
    }

    fn expected(raw_upper: u64) -> ScalarContract {
        ScalarContract {
            raw_utf8_with_quotes_upper: raw_upper,
            string_virtual_bytes_upper: 393_206,
            writes_per_nonempty_request_upper: 1,
            writes_per_empty_request_upper: 0,
            interrupted_retries_upper: 0,
            required_sources: required_sources(),
            unresolved: vec![
                "actual registry/feature/artifact join",
                "compiler/builtin and primitive semantic identity",
                "itoa/zmij safety, termination and ordinary work",
                "serde/string/slice ordinary helper work",
                "caller-owned map height/navigation",
                "ordinary error/unwind cleanup",
                "matched helper frame bytes",
            ],
            library_artifact_binding: None,
        }
    }

    fn pending(contract: &ScalarContract) {
        assert_eq!(contract.required_sources, required_sources());
        assert_eq!(contract.library_artifact_binding, None);
        assert_eq!(
            contract.unresolved,
            [
                "actual registry/feature/artifact join",
                "compiler/builtin and primitive semantic identity",
                "itoa/zmij safety, termination and ordinary work",
                "serde/string/slice ordinary helper work",
                "caller-owned map height/navigation",
                "ordinary error/unwind cleanup",
                "matched helper frame bytes",
            ]
        );
    }

    fn changed(
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

    fn refuse_shape(data: &ScalarData, limits: SchemaInputLimits, unit: SourceUnit) {
        for attempts in [1_u64, 0_u64] {
            assert_eq!(
                derive_scalar_sources(data, limits, attempts),
                Err(Refusal::SourceShape(unit))
            );
        }
    }

    fn assert_string_source_mutations(data: &ScalarData, limits: SchemaInputLimits) {
        let unframed = changed(data, SourceUnit::JsonSerializer, |file| {
            let begin = trait_body(file, "Formatter", "begin_string");
            let original: syn::Block = syn::parse_quote!({ writer.write_all(b"\"") });
            assert_eq!(
                begin.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *begin = syn::parse_quote!({ writer.write_all(b"") });
        });
        refuse_shape(&unframed, limits, SourceUnit::JsonSerializer);
        let redirected = changed(data, SourceUnit::SerdeStr, |file| {
            let body = implementation_body(file, &syn::parse_quote!(str), "serialize");
            let original: syn::Block = syn::parse_quote!({ serializer.serialize_str(self) });
            assert_eq!(
                body.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *body = syn::parse_quote!({ serializer.collect_str(self) });
        });
        refuse_shape(&redirected, limits, SourceUnit::SerdeStr);
        let escaped = changed(data, SourceUnit::JsonSerializer, |file| {
            let mut hits = 0_usize;
            for item in &mut file.items {
                if let syn::Item::Static(table) = item
                    && table.ident == "ESCAPE"
                {
                    let syn::Expr::Array(values) = table.expr.as_mut() else {
                        panic!("actual byte escape table");
                    };
                    assert_eq!(values.elems.len(), 256);
                    let first = values.elems.first_mut().unwrap();
                    assert_eq!(first.to_token_stream().to_string(), quote!(UU).to_string());
                    *first = syn::parse_quote!(__);
                    hits = hits.checked_add(1).unwrap();
                }
            }
            assert_eq!(hits, 1);
        });
        refuse_shape(&escaped, limits, SourceUnit::JsonSerializer);
    }

    fn assert_string_witnesses(limits: SchemaInputLimits) {
        let raw = "a".repeat(65_535);
        assert_eq!(raw.len(), 65_535);
        assert_eq!(
            measure_schema_input(&Value::String(raw), limits),
            Err(SchemaInputError::Bytes)
        );
        let ascii = "a".repeat(65_534);
        assert_eq!(serialized_json_bytes(ascii.as_str()).unwrap(), 65_536);
        let nul = "\0".repeat(65_534);
        assert_eq!(nul.len(), 65_534);
        assert_eq!(serialized_json_bytes(nul.as_str()).unwrap(), 393_206);
        // Virtual bytes only; neither this witness nor the contract prices scans.
        assert_eq!(
            measure_schema_input(&Value::String(nul), limits),
            Err(SchemaInputError::Bytes)
        );
        let mixed = json!({"\u{00e9}\"\n":"\u{1f642}\\\t\u{0000}"});
        assert_eq!(
            measure_schema_input(&mixed, limits),
            Ok(SchemaInputStats {
                serialized_bytes: 27,
                nodes: 3,
                max_depth: 1,
            })
        );
        for (text, literal) in [("", 2_u64), ("a", 3), ("\"", 4), ("\0", 8), ("\u{00e9}", 4)] {
            assert_eq!(serialized_json_bytes(text).unwrap(), literal);
        }
    }

    #[test]
    fn string_framing_bounds_rejected_prefixes() {
        let (data, limits) = fixture();
        before(&data);
        let result = derive_scalar_sources(&data, limits, 3);
        assert_eq!(result, Ok(expected(196_608)));
        let contract = result.unwrap();
        assert_eq!(contract.raw_utf8_with_quotes_upper, 196_608);
        assert_eq!(contract.string_virtual_bytes_upper, 393_206);
        pending(&contract);
        assert_eq!(
            derive_scalar_sources(&data, limits, 1),
            Ok(expected(65_536))
        );
        assert_eq!(derive_scalar_sources(&data, limits, 0), Ok(expected(0)));
        assert_string_witnesses(limits);
        assert_string_source_mutations(&data, limits);
        assert_eq!(
            derive_scalar_sources(&data, limits, u64::MAX),
            Err(Refusal::Overflow)
        );
    }

    fn feature_slot(features: &mut FeatureData, clause: FeatureClause) -> &mut Option<bool> {
        match clause {
            FeatureClause::SerdeStd => &mut features.serde_std,
            FeatureClause::SerdeCoreStd => &mut features.serde_core_std,
            FeatureClause::JsonStd => &mut features.json_std,
            FeatureClause::ArbitraryPrecision => &mut features.arbitrary_precision,
            FeatureClause::Docsrs => &mut features.docsrs,
        }
    }

    fn assert_feature_and_width_controls(data: &ScalarData, limits: SchemaInputLimits) {
        for clause in [
            FeatureClause::SerdeStd,
            FeatureClause::SerdeCoreStd,
            FeatureClause::JsonStd,
            FeatureClause::ArbitraryPrecision,
            FeatureClause::Docsrs,
        ] {
            let mut missing = data.clone();
            let original = feature_slot(&mut missing.features, clause).take().unwrap();
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&missing, limits, attempts),
                    Err(Refusal::MissingFeature(clause))
                );
            }
            let mut incompatible = data.clone();
            let slot = feature_slot(&mut incompatible.features, clause);
            assert_eq!(slot.replace(!original), Some(original));
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&incompatible, limits, attempts),
                    Err(Refusal::FeatureMismatch(clause))
                );
            }
        }
        let mut absent_width = data.clone();
        assert_eq!(absent_width.pointer_width.take(), Some(64));
        let mut wide = data.clone();
        assert_eq!(wide.pointer_width.replace(128), Some(64));
        for attempts in [1_u64, 0_u64] {
            assert_eq!(
                derive_scalar_sources(&absent_width, limits, attempts),
                Err(Refusal::MissingWidth)
            );
            assert_eq!(
                derive_scalar_sources(&wide, limits, attempts),
                Err(Refusal::UnsupportedWidth(128))
            );
        }
        for dispatch in [
            Dispatch::GenericSerialize,
            Dispatch::OtherWriter,
            Dispatch::PrettyFormatter,
        ] {
            let mut unsupported = data.clone();
            assert_eq!(unsupported.dispatch, Dispatch::NativeScalars);
            unsupported.dispatch = dispatch;
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&unsupported, limits, attempts),
                    Err(Refusal::UnsupportedDispatch)
                );
            }
        }
    }

    fn assert_writer_source_mutations(data: &ScalarData, limits: SchemaInputLimits) {
        let skipped = changed(data, SourceUnit::IoWrite, |file| {
            let body = trait_body(file, "Write", "write_all");
            let first = body.stmts.first_mut().unwrap();
            let syn::Stmt::Expr(syn::Expr::While(repeated), None) = first else {
                panic!("actual Write::write_all loop");
            };
            assert_eq!(
                repeated.cond.to_token_stream().to_string(),
                quote!(!buf.is_empty()).to_string()
            );
            repeated.cond = Box::new(syn::parse_quote!(false));
        });
        refuse_shape(&skipped, limits, SourceUnit::IoWrite);
        let short = changed(data, SourceUnit::JsonSize, |file| {
            let body = implementation_body(file, &syn::parse_quote!(Counter), "write");
            let tail = body.stmts.last_mut().unwrap();
            let original = syn::Stmt::Expr(syn::parse_quote!(Ok(buf.len())), None);
            assert_eq!(
                tail.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *tail = syn::Stmt::Expr(syn::parse_quote!(Ok(buf.len().saturating_sub(1))), None);
        });
        refuse_shape(&short, limits, SourceUnit::JsonSize);
        let no_forward = changed(data, SourceUnit::IoForward, |file| {
            let body = implementation_body(file, &syn::parse_quote!(&mut W), "write_all");
            let original: syn::Block = syn::parse_quote!({ (**self).write_all(buf) });
            assert_eq!(
                body.to_token_stream().to_string(),
                original.to_token_stream().to_string()
            );
            *body = syn::parse_quote!({ Ok(()) });
        });
        refuse_shape(&no_forward, limits, SourceUnit::IoForward);
    }

    #[test]
    fn concrete_counter_protocol_requires_scalar_dispatch() {
        let (data, limits) = fixture();
        before(&data);
        let result = derive_scalar_sources(&data, limits, 1);
        assert_eq!(result, Ok(expected(65_536)));
        let contract = result.unwrap();
        assert_eq!(contract.writes_per_nonempty_request_upper, 1);
        assert_eq!(contract.writes_per_empty_request_upper, 0);
        assert_eq!(contract.interrupted_retries_upper, 0);
        pending(&contract);
        let mut width_32 = data.clone();
        assert_eq!(width_32.pointer_width.replace(32), Some(64));
        assert_eq!(
            derive_scalar_sources(&width_32, limits, 1),
            Ok(expected(65_536))
        );
        // The protocol theorem still needs its source duties when no request
        // occurs; 1/0/0 is per request, not the zero-attempt aggregate.
        assert_eq!(derive_scalar_sources(&data, limits, 0), Ok(expected(0)));
        assert_feature_and_width_controls(&data, limits);
        assert_writer_source_mutations(&data, limits);
    }

    fn assert_missing_duplicate_and_changed(data: &ScalarData, limits: SchemaInputLimits) {
        for unit in required_sources() {
            let mut missing = data.clone();
            let index = missing
                .sources
                .iter()
                .position(|source| source.unit == unit)
                .unwrap();
            assert_eq!(missing.sources.remove(index).unit, unit);
            let mut duplicate = data.clone();
            duplicate.sources.push(row(data, unit).clone());
            let mut drift = data.clone();
            let source = drift
                .sources
                .iter_mut()
                .find(|source| source.unit == unit)
                .unwrap();
            source.after.push(b'\n');
            assert_ne!(source.before, source.after);
            let mut retargeted = data.clone();
            let target = retargeted
                .sources
                .iter_mut()
                .find(|source| source.unit == unit)
                .unwrap();
            let original_path = target.path.clone();
            target.path.set_file_name("fake_changed_scalar_source.rs");
            assert_ne!(target.path, original_path);
            for attempts in [1_u64, 0_u64] {
                assert_eq!(
                    derive_scalar_sources(&missing, limits, attempts),
                    Err(Refusal::MissingSource(unit))
                );
                assert_eq!(
                    derive_scalar_sources(&duplicate, limits, attempts),
                    Err(Refusal::DuplicateSource(unit))
                );
                assert_eq!(
                    derive_scalar_sources(&drift, limits, attempts),
                    Err(Refusal::SourceChanged(unit))
                );
                assert_eq!(
                    derive_scalar_sources(&retargeted, limits, attempts),
                    Err(Refusal::SourcePath(unit))
                );
            }
            let active_attribute = changed(data, unit, |file| {
                let attribute: syn::Attribute =
                    syn::parse_quote!(#![fake_unapproved_scalar_attribute]);
                file.attrs.push(attribute);
            });
            refuse_shape(&active_attribute, limits, unit);
        }
    }

    fn assert_number_type_mutation(data: &ScalarData, limits: SchemaInputLimits) {
        let changed_number = changed(data, SourceUnit::JsonNumber, |file| {
            let mut hits = 0_usize;
            for item in &mut file.items {
                if let syn::Item::Enum(number) = item
                    && number.ident == "N"
                {
                    for variant in &mut number.variants {
                        if variant.ident == "PosInt" {
                            let syn::Fields::Unnamed(fields) = &mut variant.fields else {
                                panic!("actual positive integer tuple");
                            };
                            assert_eq!(fields.unnamed.len(), 1);
                            let field = fields.unnamed.first_mut().unwrap();
                            assert_eq!(
                                field.ty.to_token_stream().to_string(),
                                quote!(u64).to_string()
                            );
                            field.ty = syn::parse_quote!(u128);
                            hits = hits.checked_add(1).unwrap();
                        }
                    }
                }
            }
            assert_eq!(hits, 1);
        });
        refuse_shape(&changed_number, limits, SourceUnit::JsonNumber);
    }

    #[test]
    fn missing_and_rehashed_scalar_sources_refuse_at_zero_attempts() {
        let (data, limits) = fixture();
        before(&data);
        assert_eq!(derive_scalar_sources(&data, limits, 0), Ok(expected(0)));
        assert_missing_duplicate_and_changed(&data, limits);
        assert_number_type_mutation(&data, limits);
    }
}

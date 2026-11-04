//! Shared test infrastructure must not know production identities or constructors.

#![cfg(all(test, not(target_arch = "wasm32")))]

#[cfg(test)]
mod tests {
    use proc_macro2::{TokenStream, TokenTree};
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Component, Path, PathBuf};
    use syn::parse::Parser as _;
    use syn::punctuated::Punctuated;
    use syn::visit_mut::{self, VisitMut};
    use url::form_urlencoded;

    #[derive(Default)]
    struct ModulePaths(Vec<String>);

    fn selects_module_file(meta: &syn::Meta) -> bool {
        if meta.path().is_ident("path") {
            return true;
        }
        if let syn::Meta::List(list) = meta
            && list.path.is_ident("cfg_attr")
        {
            return list
                .parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
                .unwrap()
                .iter()
                .any(selects_module_file);
        }
        false
    }

    fn generated_rust_include(include: &syn::Macro) -> bool {
        let Ok(syn::Expr::Macro(concat)) = include.parse_body::<syn::Expr>() else {
            return false;
        };
        if !concat.mac.path.is_ident("concat") {
            return false;
        }
        let Ok(arguments) =
            Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated.parse2(concat.mac.tokens)
        else {
            return false;
        };
        let mut parts = arguments.iter();
        let (Some(syn::Expr::Macro(root)), Some(syn::Expr::Lit(suffix)), None) =
            (parts.next(), parts.next(), parts.next())
        else {
            return false;
        };
        if !root.mac.path.is_ident("env") {
            return false;
        }
        let Ok(variable) = root.mac.parse_body::<syn::LitStr>() else {
            return false;
        };
        // SETTINGS_* are generated config compiler-probe directories.
        if !matches!(
            variable.value().as_str(),
            "OUT_DIR" | "SETTINGS_BASE" | "SETTINGS_EXPANDED"
        ) {
            return false;
        }
        let syn::Lit::Str(literal) = &suffix.lit else {
            return false;
        };
        let value = literal.value();
        let Some(relative) = value.strip_prefix('/') else {
            return false;
        };
        let path = Path::new(relative);
        !relative.contains(['\\', ':', '\0'])
            && path
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            && path.extension().is_some_and(|extension| extension == "rs")
    }

    impl VisitMut for ModulePaths {
        fn visit_item_mod_mut(&mut self, i: &mut syn::ItemMod) {
            if i.attrs
                .iter()
                .any(|attribute| selects_module_file(&attribute.meta))
            {
                self.0.push(i.ident.to_string());
            }
            visit_mut::visit_item_mod_mut(self, i);
        }

        fn visit_macro_mut(&mut self, i: &mut syn::Macro) {
            if i.path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "include")
                && !generated_rust_include(i)
            {
                self.0.push("include!".into());
            }
            // Macro bodies are opaque: quoted generator tokens are not active imports.
            visit_mut::visit_macro_mut(self, i);
        }
    }

    fn handwired_modules(source: &str) -> Vec<String> {
        let mut modules = ModulePaths::default();
        modules.visit_file_mut(&mut syn::parse_file(source).unwrap());
        modules.0
    }

    #[test]
    fn ordinary_module_guard_detects_nested_path_attributes_but_not_codegen_tokens() {
        assert_eq!(
            handwired_modules(
                "#[cfg(test)] mod checks { #[path = \"elsewhere.rs\"] mod fixture; }"
            ),
            ["fixture"],
        );
        assert_eq!(
            handwired_modules("#[cfg_attr(test, path = \"elsewhere.rs\")] mod fixture;"),
            ["fixture"],
        );
        assert!(handwired_modules("mod checks { mod fixture; }").is_empty());
        assert!(handwired_modules(
            "fn generated_discovery() { quote::quote! { #[path = #location] mod fake_owner; }; }"
        ).is_empty());
    }

    #[test]
    fn ordinary_module_guard_rejects_private_and_unbounded_includes() {
        for source in [
            r#"include!("private.rs");"#,
            r#"include!("../src/private.rs");"#,
            r#"include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/private.rs"));"#,
            r#"include!(concat!(env!("UNEXPECTED_ROOT"), "/generated.rs"));"#,
            r#"include!(concat!(env!("OUT_DIR"), "/../src/private.rs"));"#,
            r#"include!(concat!(env!("SETTINGS_BASE"), "/nested/../../private.rs"));"#,
            r#"include!(concat!(env!("SETTINGS_EXPANDED"), "/..\\private.rs"));"#,
            r#"include!(concat!(env!("OUT_DIR"), "generated.rs"));"#,
            r#"include!(concat!(env!("OUT_DIR"), "//private.rs"));"#,
            r#"include!(concat!(env!("OUT_DIR"), "/generated.json"));"#,
            r#"include!(concat!(env!("OUT_DIR"), env!("SOURCE_SUFFIX")));"#,
            r#"include!(concat!(env!("OUT_DIR"), "/nested", "/../../private.rs"));"#,
            r#"mod checks { include!("../../src/private.rs"); }"#,
            r#"fn check() { let _ = include!("../src/private.rs"); }"#,
            r#"fn check() { core::include!("../src/private.rs"); }"#,
        ] {
            assert_eq!(handwired_modules(source), ["include!"], "{source}");
        }
    }

    #[test]
    fn ordinary_module_guard_allows_generated_includes_and_opaque_fixtures() {
        for source in [
            r#"include!(concat!(env!("OUT_DIR"), "/generated.rs"));"#,
            r#"include!(concat!(env!("OUT_DIR"), "/nested/generated.rs",));"#,
            r#"include!(concat!(env!("SETTINGS_BASE"), "/bidder_settings.rs"));"#,
            r#"include!(concat!(env!("SETTINGS_EXPANDED"), "/bidder_settings.rs"));"#,
            "mod checks { mod fixture; }",
            r#"fn generate() { quote::quote! { include!("../src/private.rs"); #[path = #location] mod owner; }; }"#,
            r##"const SOURCE: &str = r#"include!("../src/private.rs"); #[path = "private.rs"] mod owner;"#;"##,
            r#"const SOURCE: &str = include_str!("../src/private.rs");"#,
            r#"const SOURCE: &[u8] = include_bytes!("../src/private.rs");"#,
        ] {
            assert!(handwired_modules(source).is_empty(), "{source}");
        }
    }

    #[test]
    fn workspace_uses_ordinary_module_resolution() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut files = Vec::new();
        for entry in fs::read_dir(&root).unwrap() {
            let directory = entry.unwrap().path();
            if directory.join("Cargo.toml").is_file() {
                rust_files(&directory, &mut files);
            }
        }
        assert!(!files.is_empty());
        let violations: Vec<_> = files
            .into_iter()
            .filter_map(|path| {
                let source = fs::read_to_string(&path).unwrap();
                let mut syntax = syn::parse_file(&source)
                    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
                let mut visitor = ModulePaths::default();
                visitor.visit_file_mut(&mut syntax);
                let modules = visitor.0;
                (!modules.is_empty())
                    .then(|| (path.strip_prefix(&root).unwrap().to_owned(), modules))
            })
            .collect();
        assert!(
            violations.is_empty(),
            "nonstandard module wiring: {violations:?}"
        );
    }

    #[derive(Default)]
    struct ProductionNames {
        identities: BTreeSet<String>,
        constructors: BTreeSet<String>,
    }

    impl ProductionNames {
        fn contains_identity(&self, value: &str) -> bool {
            self.identities.contains(&value.to_ascii_lowercase())
        }
    }

    fn production_names(root: &Path) -> ProductionNames {
        let mut names = ProductionNames::default();
        let inventory: toml::Value = toml::from_str(include_str!(
            "conformance/fixtures/reference/catalog/source-inventory.toml"
        ))
        .unwrap();
        names.identities.extend(
            inventory["identity"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["name"].as_str().unwrap().to_ascii_lowercase()),
        );
        names.identities.extend(
            inventory["sync_family"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["key"].as_str().unwrap().to_ascii_lowercase()),
        );
        for entry in fs::read_dir(root).unwrap() {
            let directory = entry.unwrap().path();
            let manifest_path = directory.join("bidder.toml");
            if !manifest_path.is_file() {
                continue;
            }
            let manifest: toml::Value =
                toml::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
            names
                .identities
                .insert(manifest["key"].as_str().unwrap().to_ascii_lowercase());
            if let Some(definitions) = manifest.get("definitions").and_then(toml::Value::as_table) {
                names
                    .identities
                    .extend(definitions.keys().map(|name| name.to_ascii_lowercase()));
            }
            let source = directory.join("mod.rs");
            if !source.is_file() {
                continue;
            }
            let syntax = syn::parse_file(&fs::read_to_string(source).unwrap()).unwrap();
            for item in syntax.items {
                let syn::Item::Impl(implementation) = item else {
                    continue;
                };
                if implementation.trait_.as_ref().is_none_or(|(_, path, _)| {
                    path.segments
                        .last()
                        .is_none_or(|segment| segment.ident != "Bidder")
                }) {
                    continue;
                }
                let syn::Type::Path(path) = implementation.self_ty.as_ref() else {
                    continue;
                };
                names
                    .constructors
                    .insert(path.path.segments.last().unwrap().ident.to_string());
            }
        }
        assert!(!names.identities.is_empty());
        assert!(!names.constructors.is_empty());
        names
    }

    fn references(stream: TokenStream, names: &ProductionNames, found: &mut BTreeSet<String>) {
        let tokens: Vec<_> = stream.into_iter().collect();
        for (start, token) in tokens.iter().enumerate() {
            if !matches!(token, TokenTree::Ident(word) if word == "use") {
                continue;
            }
            let remaining = &tokens[start..];
            let Some(end) = remaining.iter().position(
                |following| matches!(following, TokenTree::Punct(mark) if mark.as_char() == ';'),
            ) else {
                continue;
            };
            if let Ok(import) =
                syn::parse2::<syn::ItemUse>(remaining[..=end].iter().cloned().collect())
            {
                import_references(&import.tree, names, found);
            }
        }
        for path in tokens.windows(4) {
            if let [
                TokenTree::Ident(namespace),
                TokenTree::Punct(first),
                TokenTree::Punct(second),
                owner,
            ] = path
                && (namespace == "bidder_settings" || namespace == "bidder_endpoint")
                && first.as_char() == ':'
                && second.as_char() == ':'
            {
                namespace_owners(owner, names, found);
            }
            if let [
                TokenTree::Ident(owner),
                TokenTree::Punct(first),
                TokenTree::Punct(second),
                TokenTree::Ident(_),
            ] = path
                && first.as_char() == ':'
                && second.as_char() == ':'
                && names.contains_identity(&owner.to_string())
            {
                found.insert(owner.to_string());
            }
        }
        for token in tokens {
            match token {
                TokenTree::Group(group) => references(group.stream(), names, found),
                TokenTree::Ident(ident) => {
                    let name = ident.to_string();
                    if names.constructors.contains(&name) {
                        found.insert(name.clone());
                    }
                    if let Some(owner) = name.strip_prefix("as_")
                        && names.contains_identity(owner)
                    {
                        found.insert(owner.to_owned());
                    }
                }
                TokenTree::Literal(token_literal) => {
                    let Ok(literal) = syn::parse_str::<syn::Lit>(&token_literal.to_string()) else {
                        continue;
                    };
                    let value = if let syn::Lit::Str(value) = literal {
                        value.value()
                    } else if let syn::Lit::ByteStr(value) = literal {
                        String::from_utf8_lossy(&value.value()).into_owned()
                    } else if let syn::Lit::CStr(value) = literal {
                        value.value().to_string_lossy().into_owned()
                    } else {
                        continue;
                    };
                    text_references(&value, names, found);
                    if names.contains_identity(&value) || names.constructors.contains(&value) {
                        found.insert(value.clone());
                    }
                    let query = value
                        .split_once('?')
                        .map_or(value.as_str(), |(_, query)| query);
                    for (_, parameter) in form_urlencoded::parse(query.as_bytes()) {
                        if names.contains_identity(parameter.as_ref()) {
                            found.insert(parameter.into_owned());
                        }
                    }
                    if value.contains('/') || value.contains('\\') {
                        for component in value.split(['/', '\\']) {
                            let stem = component.split('.').next().unwrap();
                            if names.contains_identity(stem) {
                                found.insert(stem.into());
                            }
                        }
                    }
                    // Embedded Rust/JSON fixtures may themselves contain quoted
                    // identities. Tokenize those rather than matching prose words.
                    if let Ok(nested) = value.parse::<TokenStream>() {
                        references(nested, names, found);
                    }
                }
                TokenTree::Punct(_) => (),
            }
        }
    }

    fn import_references(
        tree: &syn::UseTree,
        names: &ProductionNames,
        found: &mut BTreeSet<String>,
    ) {
        let candidate_ident = match tree {
            syn::UseTree::Path(path) => {
                import_references(&path.tree, names, found);
                Some(&path.ident)
            }
            syn::UseTree::Group(group) => {
                for child in &group.items {
                    import_references(child, names, found);
                }
                None
            }
            syn::UseTree::Name(name) => Some(&name.ident),
            syn::UseTree::Rename(rename) => Some(&rename.ident),
            syn::UseTree::Glob(_) => None,
        };
        if let Some(ident) = candidate_ident
            && names.contains_identity(&ident.to_string())
        {
            found.insert(ident.to_string());
        }
    }

    fn namespace_owners(token: &TokenTree, names: &ProductionNames, found: &mut BTreeSet<String>) {
        match token {
            TokenTree::Ident(owner) if names.contains_identity(&owner.to_string()) => {
                found.insert(owner.to_string());
            }
            TokenTree::Group(group) => {
                for child in group.stream() {
                    namespace_owners(&child, names, found);
                }
            }
            TokenTree::Ident(_) | TokenTree::Literal(_) | TokenTree::Punct(_) => (),
        }
    }

    fn text_references(text: &str, names: &ProductionNames, found: &mut BTreeSet<String>) {
        identity_text(text, names, found);
        if let Ok(document) = serde_json::from_str::<Value>(text) {
            json_references(&document, names, found);
        }
        if let Ok(document) = toml::from_str::<toml::Value>(text) {
            toml_references(&document, names, found);
        }
    }

    fn identity_text(text: &str, names: &ProductionNames, found: &mut BTreeSet<String>) {
        // Prose can use ordinary words which are also vendor names. Identity
        // tokens and case/path labels are single words; embedded data is parsed
        // separately, rather than treating every English sentence as metadata.
        if text.chars().any(char::is_whitespace) {
            return;
        }
        for token in
            text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        {
            if names.contains_identity(token) {
                found.insert(token.to_owned());
                continue;
            }
            if token.starts_with("setuid_") || token.starts_with("cookie_sync_") {
                for word in token.split('_') {
                    if names.contains_identity(word) {
                        found.insert(word.to_owned());
                    }
                }
            }
        }
    }

    fn json_references(value: &Value, names: &ProductionNames, found: &mut BTreeSet<String>) {
        match value {
            Value::Object(fields) => {
                for (key, child) in fields {
                    identity_text(key, names, found);
                    if key == "run"
                        && let Some(command) = child.as_str()
                    {
                        command_references(command, names, found);
                    }
                    json_references(child, names, found);
                }
            }
            Value::Array(entries) => {
                for child in entries {
                    json_references(child, names, found);
                }
            }
            Value::String(text) => text_references(text, names, found),
            Value::Null | Value::Bool(_) | Value::Number(_) => (),
        }
    }

    fn command_references(command: &str, names: &ProductionNames, found: &mut BTreeSet<String>) {
        let mut target = false;
        for argument in command.split_whitespace() {
            identity_text(argument, names, found);
            if target {
                for word in argument.split('_') {
                    if names.contains_identity(word) {
                        found.insert(word.to_owned());
                    }
                }
            }
            target = matches!(argument, "--test" | "--bin" | "--example");
        }
    }

    fn owned_test_targets(manifest: &toml::Value) -> bool {
        manifest
            .get("test")
            .and_then(toml::Value::as_array)
            .is_none_or(|targets| {
                targets.iter().all(|target| {
                    target
                        .get("path")
                        .and_then(toml::Value::as_str)
                        .is_none_or(|path| {
                            Path::new(path).starts_with("tests")
                                && !Path::new(path).is_absolute()
                                && !path.contains([':', '\\'])
                                && !Path::new(path)
                                    .components()
                                    .any(|part| matches!(part, Component::ParentDir))
                        })
                })
            })
    }

    fn component_files(root: &Path, files: &mut Vec<PathBuf>) {
        for result in fs::read_dir(root).unwrap() {
            let entry = result.unwrap();
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "component inputs must not be symlinks: {}",
                path.display()
            );
            if kind.is_dir() {
                component_files(&path, files);
                continue;
            }
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| matches!(ext, "rs" | "toml" | "json" | "yml" | "yaml"))
            {
                files.push(path);
            }
        }
    }

    #[test]
    fn other_components_do_not_encode_provider_identities_or_external_test_targets() {
        let owner = Path::new(env!("CARGO_MANIFEST_DIR"));
        let crates = owner.parent().unwrap();
        let names = production_names(&owner.join("src/bidders"));
        let mut files = Vec::new();
        for entry in fs::read_dir(crates).unwrap() {
            let path = entry.unwrap().path();
            if path != owner && path.join("Cargo.toml").is_file() {
                component_files(&path, &mut files);
            }
        }
        component_files(&crates.parent().unwrap().join(".github"), &mut files);
        for entry in fs::read_dir(crates.parent().unwrap()).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| matches!(ext, "toml" | "json" | "yml" | "yaml"))
            {
                files.push(path);
            }
        }
        assert!(files.len() > 100);
        files.sort();
        let mut violations = Vec::new();
        for path in files {
            let relative = path.strip_prefix(crates.parent().unwrap()).unwrap();
            if relative.starts_with("crates/auctioneer-config-data/data/gvl_archive")
                && path.extension().is_some_and(|ext| ext == "json")
            {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap();
            let mut found = BTreeSet::new();
            match path.extension().unwrap().to_str().unwrap() {
                "rs" => references(
                    source
                        .parse()
                        .unwrap_or_else(|error| panic!("parse {}: {error}", path.display())),
                    &names,
                    &mut found,
                ),
                "json" => {
                    if let Ok(value) = serde_json::from_str::<Value>(&source) {
                        json_references(&value, &names, &mut found);
                    } else {
                        identity_text(&source, &names, &mut found);
                    }
                }
                "toml" => {
                    if let Ok(value) = toml::from_str::<toml::Value>(&source) {
                        json_references(&serde_json::to_value(&value).unwrap(), &names, &mut found);
                        if path.file_name().unwrap() == "Cargo.toml" && !owned_test_targets(&value)
                        {
                            found.insert("out-of-crate Cargo test path".into());
                        }
                    } else {
                        identity_text(&source, &names, &mut found);
                    }
                }
                "yaml" | "yml" => {
                    let document: serde_yaml_ng::Value = serde_yaml_ng::from_str(&source).unwrap();
                    json_references(&serde_json::to_value(document).unwrap(), &names, &mut found);
                }
                extension => panic!("unhandled component input extension: {extension}"),
            }
            if !found.is_empty() {
                violations.push((relative.to_owned(), found));
            }
        }
        assert!(
            violations.is_empty(),
            "provider knowledge outside the bidder component: {violations:#?}"
        );
    }

    #[test]
    fn component_boundary_rejects_independent_code_data_and_manifest_controls() {
        let names = ProductionNames {
            identities: BTreeSet::from(["supplier".into()]),
            constructors: BTreeSet::new(),
        };
        for source in [
            serde_json::json!({"supplier":{}}),
            serde_json::json!({"implementation":"supplier"}),
            serde_json::json!({"case":"setuid_SUPPLIER_blank"}),
            serde_json::json!({"source":"https://supplier.example/auction"}),
            serde_json::json!({"config_json": "{\"implementation\": \"supplier\"}"}),
            serde_json::json!({"config_toml": "[bidders.supplier]\nenabled = false\n"}),
        ] {
            let mut found = BTreeSet::new();
            json_references(&source, &names, &mut found);
            assert_eq!(found.len(), 1, "{source}");
        }
        let mut ordinary = BTreeSet::new();
        json_references(
            &serde_json::json!({"fake_supplier": "fake_supplier_alias"}),
            &names,
            &mut ordinary,
        );
        assert!(ordinary.is_empty());
        let workflow: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "jobs:\n  check:\n    steps:\n    - run: cargo test --test supplier_contract\n",
        )
        .unwrap();
        json_references(
            &serde_json::to_value(workflow).unwrap(),
            &names,
            &mut ordinary,
        );
        assert_eq!(ordinary, BTreeSet::from(["supplier".to_owned()]));
        for path in [
            "../supplier/conformance.rs",
            "/supplier/conformance.rs",
            "tests/../../supplier.rs",
            "src/contract.rs",
            "conformance/contract.rs",
        ] {
            let manifest = toml::toml! { [[test]] name = "fake_contract" path = path };
            assert!(!owned_test_targets(&manifest.into()));
        }
        assert!(owned_test_targets(
            &toml::toml! { [[test]] name = "fake_contract" path = "tests/contract.rs" }.into()
        ));
    }

    fn toml_references(
        document: &toml::Value,
        names: &ProductionNames,
        found: &mut BTreeSet<String>,
    ) {
        match document {
            toml::Value::Table(table) => {
                for (key, child) in table {
                    if names.contains_identity(key) {
                        found.insert(key.clone());
                    }
                    toml_references(child, names, found);
                }
            }
            toml::Value::Array(values) => {
                for entry in values {
                    toml_references(entry, names, found);
                }
            }
            toml::Value::String(text) => text_references(text, names, found),
            toml::Value::Integer(_)
            | toml::Value::Float(_)
            | toml::Value::Boolean(_)
            | toml::Value::Datetime(_) => (),
        }
    }

    fn rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
        for result in fs::read_dir(directory).unwrap() {
            let entry = result.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "test input must not be a symlink");
            if kind.is_dir() {
                rust_files(&entry.path(), files);
                continue;
            }
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
            {
                files.push(entry.path());
            }
        }
    }

    #[test]
    fn conformance_sources_stay_in_their_owners_test_directories() {
        let owner = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0_usize;
        let mut misplaced = Vec::new();
        for entry in fs::read_dir(owner.parent().unwrap()).unwrap() {
            let root = entry.unwrap().path();
            if !root.join("Cargo.toml").is_file() {
                continue;
            }
            checked += 1;
            assert!(!root.join("conformance").exists(), "{}", root.display());
            let manifest: toml::Value =
                toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
            assert!(owned_test_targets(&manifest), "{}", root.display());
            let mut files = Vec::new();
            rust_files(&root.join("src"), &mut files);
            for path in files {
                if path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .contains("conformance")
                    && !path
                        .strip_prefix(&root)
                        .unwrap()
                        .components()
                        .any(|component| component == Component::Normal("tests".as_ref()))
                {
                    misplaced.push(path);
                }
            }
        }
        assert!(checked > 10);
        assert!(
            misplaced.is_empty(),
            "misplaced conformance: {misplaced:#?}"
        );
    }

    #[test]
    fn shared_test_sources_use_synthetic_bidder_inputs() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let names = production_names(&crate_root.join("src/bidders"));
        let mut files = Vec::new();
        rust_files(&crate_root.join("tests"), &mut files);
        assert!(!files.is_empty());
        files.sort();
        let mut violations = Vec::new();
        for path in files {
            if path.starts_with(crate_root.join("tests/conformance")) {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap();
            let tokens: TokenStream = source.parse().unwrap();
            let mut found = BTreeSet::new();
            references(tokens, &names, &mut found);
            if !found.is_empty() {
                violations.push((path.strip_prefix(crate_root).unwrap().to_owned(), found));
            }
        }
        assert!(
            violations.is_empty(),
            "production coupling in shared tests: {violations:#?}"
        );
    }

    #[test]
    fn shared_bidder_sources_do_not_depend_on_real_protocol_owners() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = root.join("src");
        let names = production_names(&source.join("bidders"));
        let mut files = Vec::new();
        rust_files(&source, &mut files);
        let mut checked = 0_usize;
        let mut violations = Vec::new();
        for path in files {
            if path
                .parent()
                .unwrap()
                .ancestors()
                .take_while(|parent| *parent != source)
                .any(|parent| parent.join("bidder.toml").is_file())
            {
                continue;
            }
            checked += 1;
            let mut found = BTreeSet::new();
            references(
                fs::read_to_string(&path).unwrap().parse().unwrap(),
                &names,
                &mut found,
            );
            if !found.is_empty() {
                violations.push((path.strip_prefix(root).unwrap().to_owned(), found));
            }
        }
        assert!(checked > 10);
        assert!(
            violations.is_empty(),
            "provider dependencies in shared bidder sources: {violations:#?}"
        );
    }

    #[test]
    fn source_guard_finds_owner_paths_and_aliases_without_test_attributes() {
        let names = ProductionNames {
            identities: BTreeSet::from(["supplier".into()]),
            constructors: BTreeSet::from(["SupplierBidder".into()]),
        };
        for source in [
            "#[cfg(test)] mod checks { use owner::SupplierBidder; }",
            "#[cfg(all(test, not(target_arch = \"wasm32\")))] mod checks { outer! { owner::SupplierBidder } }",
            "#[cfg(test)] use owner::SupplierBidder;",
            "#[cfg(test)] const FACTORY: SupplierBidder = SupplierBidder;",
            "#[cfg(test)] impl FakeHarness { fn make() -> SupplierBidder { SupplierBidder } }",
            "mod nested { #[test] fn contract() { owner::SupplierBidder; } }",
            "mod nested { #[tokio::test] async fn contract() { owner::SupplierBidder; } }",
        ] {
            let mut found = BTreeSet::new();
            references(
                source
                    .parse()
                    .unwrap_or_else(|error| panic!("parse fixture {source}: {error}")),
                &names,
                &mut found,
            );
            assert_eq!(
                found,
                BTreeSet::from(["SupplierBidder".to_owned()]),
                "{source}"
            );
        }
        for source in [
            "fn helper() { super::supplier::create_bidder(); }",
            "fn helper() { supplier::some_helper(); }",
            "fn helper() { crate::bidders::supplier::some_helper(); }",
            "use bidders::supplier as selected; fn helper() { selected::create_bidder(); }",
            "use bidders::supplier::create_bidder as selected; fn helper() { selected(); }",
            "use bidders::{supplier::{create_bidder as selected}};",
            "use bidders::supplier::{create_bidder as selected};",
        ] {
            let mut found = BTreeSet::new();
            references(source.parse().unwrap(), &names, &mut found);
            assert_eq!(found, BTreeSet::from(["supplier".to_owned()]), "{source}");
        }
        let mut found = BTreeSet::new();
        references(
            "use fake_bidder::FakeBidder; fn helper() { fake_bidder::create_bidder(); }"
                .parse()
                .unwrap(),
            &names,
            &mut found,
        );
        assert!(found.is_empty());
    }

    // Semantic owners, not a bidder roster. Captured-source conformance keeps
    // its real inputs; complete module trees are discovered below as well.
    const SEMANTIC_SURFACES: &[&str] = &[
        "auctioneer-bidder/src/request_info.rs",
        "auctioneer-bidder/src/endpoint_builder.rs",
        "auctioneer-bidder/src/bidder_settings.rs",
        "auctioneer-bidder/src/bidder_endpoint.rs",
        "auctioneer-bidder/src/registry/mod.rs",
        "auctioneer-bidder/src/types.rs",
        "auctioneer-config/src/bidder_reconcile.rs",
        "auctioneer-exchange/src/alias.rs",
        "auctioneer-exchange/src/registry_order.rs",
        "auctioneer-exchange/src/plan.rs",
        "auctioneer-exchange/src/response.rs",
        "auctioneer-exchange/src/split/mod.rs",
        "auctioneer-exchange/src/targeting/keys.rs",
        "auctioneer-exchange/src/run/mod.rs",
        "auctioneer-exchange/src/debug/mod.rs",
        "auctioneer-exchange/tests/config_overlay.rs",
        "auctioneer-exchange/tests/determinism.rs",
        "auctioneer-exchange/tests/privacy_enforcement.rs",
        "auctioneer-exchange/tests/run_auction/main.rs",
        "auctioneer-exchange/tests/sampling_nbr.rs",
        "auctioneer-exchange/tests/targeting_prefix/main.rs",
        "auctioneer-bidder/src/test_support/fake_bidder/mod.rs",
        "auctioneer-exchange/tests/auction_support/mod.rs",
        "auctioneer-core/src/config/mod.rs",
        "auctioneer-core/src/bidder_registry.rs",
        "auctioneer-core/src/handlers/cookie_sync.rs",
        "auctioneer-core/src/handlers/setuid.rs",
        "auctioneer-core/src/config/resolve.rs",
        "auctioneer-core/src/config/budgets.rs",
        "auctioneer-core/src/conformance/mod.rs",
        "auctioneer-core/src/usersync/choose.rs",
        "auctioneer-core/src/usersync/evict.rs",
        "auctioneer-core/src/usersync/template.rs",
        "auctioneer-core/src/usersync/wire.rs",
        "auctioneer-core/src/usersync/catalog.rs",
        "auctioneer-core/src/handlers/info_bidders.rs",
        "auctioneer-core/tests/usersync_catalog.rs",
        "auctioneer-core/tests/usersync_choose.rs",
        "auctioneer-core/tests/usersync_template.rs",
        "auctioneer-core/tests/usersync_routes.rs",
        "auctioneer-core/tests/usersync_handlers.rs",
        "auctioneer-core/tests/usersync_http.rs",
        "auctioneer-core/tests/usersync_flow.rs",
        "auctioneer-core/tests/usersync_integrity.rs",
        "auctioneer-core/tests/usersync_precedence.rs",
        "auctioneer-core/tests/usersync_config_lifecycle.rs",
        "auctioneer-core/tests/usersync_handler_alloc.rs",
        "auctioneer-core/tests/auction_config_lifecycle.rs",
        "auctioneer-core/tests/host_alias_overlay_bounds.rs",
        "auctioneer-core/tests/aux_request_body_bounds.rs",
        "auctioneer-core/tests/aux_route_write_deadline.rs",
        "auctioneer-core/tests/config_error_precedence.rs",
        "auctioneer-core/tests/config_parse_factor.rs",
        "auctioneer-core/tests/privacy_wire_signal_400.rs",
        "auctioneer-core/tests/node_budget_on_the_auction_path.rs",
        "auctioneer-core/tests/impression_limit_on_the_auction_path.rs",
        "auctioneer-core/tests/response_oversize_response.rs",
        "auctioneer-core/tests/response_mid_flight_timeout.rs",
        "auctioneer-core/tests/privacy_outbound.rs",
        "auctioneer-core/tests/response_gdpr_applies.rs",
        "auctioneer-core/tests/bidder_settings_conformance.rs",
        "auctioneer-core/tests/usersync_live_catalog.rs",
        "auctioneer-bidder/src/test_support/serving_catalog.rs",
        "auctioneer-core/src/test_support/live_catalog.rs",
        "auctioneer-core/tests/response_slot_classification.rs",
        "auctioneer-core/src/test_support/bidder_fixture.rs",
        "auctioneer-privacy/src/activity.rs",
        "auctioneer-privacy/src/auction_context.rs",
        "auctioneer-privacy/tests/privacy_resource_limits.rs",
        "auctioneer-privacy/tests/stub_invariants.rs",
        "auctioneer-privacy/tests/activity_controls.rs",
        "auctioneer-openrtb/src/ext/imp.rs",
        "auctioneer-openrtb/src/ext/bid.rs",
        "auctioneer-openrtb/src/ext/request_prebid.rs",
        "auctioneer-runtime/src/body_caps.rs",
        "auctioneer-config/src/bindings.rs",
        "auctioneer-config-data/tests/codegen.rs",
        "auctioneer-config-data/tests/smoke.rs",
        "auctioneer-config-data/tests/bidder_storage.rs",
        "auctioneer-config-data/tests/bidder_bindings.rs",
        "auctioneer-config-data/tests/build_failure_harness.rs",
        "auctioneer-config-data/tests/url_safety.rs",
        "auctioneer-config-data/tests/usersync_callback_validation.rs",
        "auctioneer-cli/src/bidders/source.rs",
        "auctioneer-cli/src/bidders/import.rs",
    ];

    #[test]
    fn workspace_shared_semantic_surfaces_use_synthetic_bidder_inputs() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace = crate_root.join("../..");
        let names = production_names(&crate_root.join("src/bidders"));
        assert!(!SEMANTIC_SURFACES.is_empty());
        let mut violations = Vec::new();
        let mut files: BTreeSet<_> = SEMANTIC_SURFACES
            .iter()
            .map(|surface| workspace.join("crates").join(surface))
            .collect();
        // These complete semantic module trees must stay independent as files
        // are added; a hand-maintained selection previously missed new owners.
        for directory in [
            "auctioneer-exchange/src",
            "auctioneer-core/src",
            "auctioneer-core/tests",
            "auctioneer-exchange/tests",
            "auctioneer-adapter-axum/tests",
        ] {
            let mut discovered = Vec::new();
            rust_files(&workspace.join("crates").join(directory), &mut discovered);
            assert!(!discovered.is_empty());
            files.extend(discovered);
        }
        for path in files {
            let source = fs::read_to_string(&path).unwrap();
            let mut found = BTreeSet::new();
            references(
                source
                    .parse()
                    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display())),
                &names,
                &mut found,
            );
            if !found.is_empty() {
                violations.push((path.strip_prefix(&workspace).unwrap().to_owned(), found));
            }
        }
        assert!(
            violations.is_empty(),
            "production coupling in shared semantic surfaces: {violations:#?}"
        );
    }

    #[test]
    fn authored_gdpr_fixtures_use_symbolic_bidder_roles() {
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let names = production_names(&crate_root.join("src/bidders"));
        let fixtures = crate_root.join("../auctioneer-core/tests/fixtures");
        let mut checked = 0_usize;
        let mut violations = Vec::new();
        for entry in fs::read_dir(fixtures).unwrap() {
            let path = entry.unwrap().path();
            if !path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("gdpr_applies")
            {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap();
            let mut found = BTreeSet::new();
            if path.extension().unwrap() == "toml" {
                let document: toml::Value = toml::from_str(&source).unwrap();
                let aliases = document["host"]["aliases"].as_table().unwrap();
                assert!(!aliases.is_empty());
                for (alias, base) in aliases {
                    assert!(alias.starts_with("fake_"));
                    assert!(base.as_str().unwrap().starts_with("fake_"));
                }
                let basic = document["accounts"]["acct-1"]["gdpr"]["basic_enforcement_vendors"]
                    .as_array()
                    .unwrap();
                assert!(!basic.is_empty());
                assert!(
                    basic
                        .iter()
                        .all(|name| name.as_str().unwrap().starts_with("fake_"))
                );
                toml_references(&document, &names, &mut found);
            } else {
                let document: Value = serde_json::from_str(&source).unwrap();
                let mut identities = BTreeSet::new();
                json_fixture_bidder_names(&document, &mut identities);
                assert!(
                    !identities.is_empty(),
                    "fixture must exercise bidder identities"
                );
                assert!(
                    identities.iter().all(|name| name.starts_with("fake_")),
                    "unprefixed fixture names: {identities:?}"
                );
                references(source.parse().unwrap(), &names, &mut found);
            }
            if !found.is_empty() {
                violations.push((path.file_name().unwrap().to_owned(), found));
            }
            checked += 1;
        }
        assert_eq!(
            checked, 5,
            "cover every authored handler fixture in this family"
        );
        assert!(
            violations.is_empty(),
            "authored fixture coupling: {violations:#?}"
        );
    }

    fn json_fixture_bidder_names(value: &Value, found: &mut BTreeSet<String>) {
        match value {
            Value::Object(fields) => {
                for (key, child) in fields {
                    if let Some(name) = ["hb_bidder_", "hb_pb_", "hb_size_"]
                        .into_iter()
                        .find_map(|prefix| key.strip_prefix(prefix))
                    {
                        found.insert(name.to_owned());
                    }
                    if key == "message"
                        && let Some(name) = child.as_str().and_then(|message| {
                            message.strip_prefix("bidder blocked by privacy settings: ")
                        })
                    {
                        found.insert(name.to_owned());
                    }
                    if matches!(key.as_str(), "seat" | "hb_bidder")
                        && let Some(name) = child.as_str()
                    {
                        found.insert(name.to_owned());
                    }
                    if matches!(
                        key.as_str(),
                        "bidder" | "aliasgvlids" | "responsetimemillis" | "warnings"
                    ) && let Some(entries) = child.as_object()
                    {
                        found.extend(
                            entries
                                .keys()
                                .filter(|name| key != "warnings" || name.as_str() != "prebid")
                                .cloned(),
                        );
                    }
                    json_fixture_bidder_names(child, found);
                }
            }
            Value::Array(values) => {
                for child in values {
                    json_fixture_bidder_names(child, found);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => (),
        }
    }

    #[test]
    fn fixture_name_guard_follows_bidder_slots_not_arbitrary_text() {
        let fixture = serde_json::json!({
            "imp": [{"ext":{"prebid":{"bidder":{"fake_request":{}}}}}],
            "seatbid": [{"seat":"fake_seat","targeting":{"hb_bidder":"fake_target", "hb_pb_fake_key":"1.50"}}],
            "aliasgvlids":{"fake_alias":99_u32},
            "responsetimemillis":{"fake_timed":10_u32},
            "warnings":{"prebid":[],"fake_warning":[{"message":"bidder blocked by privacy settings: fake_message"}]},
            "note":"unprefixed prose is not an identity"
        });
        let mut found = BTreeSet::new();
        json_fixture_bidder_names(&fixture, &mut found);
        assert_eq!(
            found,
            BTreeSet::from([
                "fake_request".into(),
                "fake_seat".into(),
                "fake_target".into(),
                "fake_key".into(),
                "fake_message".into(),
                "fake_alias".into(),
                "fake_timed".into(),
                "fake_warning".into()
            ])
        );
        let mut reserved_bidder = BTreeSet::new();
        json_fixture_bidder_names(
            &serde_json::json!({"bidder":{"prebid":{}}}),
            &mut reserved_bidder,
        );
        assert_eq!(reserved_bidder, BTreeSet::from(["prebid".into()]));
        for source in [
            serde_json::json!({"targeting":{"hb_pb_unprefixed":"1.50"}}),
            serde_json::json!({"message":"bidder blocked by privacy settings: unprefixed"}),
        ] {
            let mut suffix = BTreeSet::new();
            json_fixture_bidder_names(&source, &mut suffix);
            assert_eq!(suffix, BTreeSet::from(["unprefixed".into()]));
            assert!(!suffix.iter().all(|name| name.starts_with("fake_")));
        }
    }

    fn nonfake_bidder_literals(stream: TokenStream, found: &mut BTreeSet<String>) {
        let tokens: Vec<_> = stream.into_iter().collect();
        for window in tokens.windows(5) {
            if let [
                TokenTree::Ident(owner),
                TokenTree::Punct(first),
                TokenTree::Punct(second),
                TokenTree::Ident(method),
                TokenTree::Group(args),
            ] = window
                && owner == "BidderName"
                && first.as_char() == ':'
                && second.as_char() == ':'
                && method == "new"
                && let Ok(arguments) =
                    Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated.parse2(args.stream())
                && let Some(syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(name),
                    ..
                })) = arguments.first()
                && !name.value().starts_with("fake_")
            {
                found.insert(name.value());
            }
        }
        for token in tokens {
            if let TokenTree::Group(group) = token {
                nonfake_bidder_literals(group.stream(), found);
            }
        }
    }

    #[test]
    fn workspace_literal_bidder_names_use_fake_prefix_outside_bidder_owners() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut paths = Vec::new();
        for entry in fs::read_dir(&crates).unwrap() {
            let directory = entry.unwrap().path();
            if directory.join("Cargo.toml").is_file() {
                for subtree in ["src", "tests"] {
                    if directory.join(subtree).is_dir() {
                        rust_files(&directory.join(subtree), &mut paths);
                    }
                }
            }
        }
        assert!(!paths.is_empty());
        let mut violations = Vec::new();
        for path in paths {
            let relative = path.strip_prefix(&crates).unwrap();
            if relative.starts_with("auctioneer-bidder/src/bidders") {
                continue;
            }
            let mut found = BTreeSet::new();
            nonfake_bidder_literals(
                fs::read_to_string(&path)
                    .unwrap()
                    .parse()
                    .unwrap_or_else(|error| panic!("parse {}: {error}", path.display())),
                &mut found,
            );
            if !found.is_empty() {
                violations.push((relative.to_owned(), found));
            }
        }
        assert!(
            violations.is_empty(),
            "unprefixed literal test bidder names: {violations:#?}"
        );
    }

    #[test]
    fn fake_prefix_guard_checks_nested_typed_names_not_substrings() {
        let mut found = BTreeSet::new();
        nonfake_bidder_literals("outer! { BidderName::new(\"afake_bad\"); BidderName::new(\"fake-good\"); BidderName::new(\"bad_trailing\",); BidderName::new(\"fake_good\",); }".parse().unwrap(), &mut found);
        assert_eq!(
            found,
            BTreeSet::from([
                "afake_bad".into(),
                "fake-good".into(),
                "bad_trailing".into()
            ])
        );
    }

    #[test]
    fn isolation_guard_checks_nested_macros_constructor_paths_and_escaped_literals() {
        let names = ProductionNames {
            identities: BTreeSet::from(["fake_supplier_fixture".into()]),
            constructors: BTreeSet::from(["FakeSupplierFixtureBidder".into()]),
        };
        for source in [
            "use example::FakeSupplierFixtureBidder;",
            "use auctioneer_bidder::bidder_settings::fake_supplier_fixture;",
            "use auctioneer_bidder::bidder_endpoint::fake_supplier_fixture::Values;",
            "use auctioneer_bidder::bidder_endpoint::{fake_supplier_fixture::Values};",
            "use auctioneer_bidder::bidder_settings::{fake_supplier_fixture as Selected};",
            "use auctioneer_bidder::{bidder_settings::{fake_supplier_fixture::{self, Settings as Selected}}};",
            "let _ = settings.as_fake_supplier_fixture();",
            "outer! { inner!(\"fake_supplier_fixture\") }",
            "include_str!(\"../src/bidders/fake_supplier_fixture/parameters.json\");",
            "let _ = \"fake_supplier\\u{5f}fixture\";",
            "let _ = br#\"{\"bidder\":\"fake_supplier_fixture\"}\"#;",
            "let _ = \"/setuid?bidder=fake_supplier_fixture&uid=probe\";",
            "let _ = \"/setuid?bidder=FAKE_SUPPLIER_FIXTURE&uid=probe\";",
            "let _ = \"bidder=fake_supplier%5Ffixture&uid=probe\";",
            "let _ = \"[bidders.fake_supplier_fixture]\\nenabled=false\";",
        ] {
            let mut found = BTreeSet::new();
            references(source.parse().unwrap(), &names, &mut found);
            assert_eq!(found.len(), 1, "{source}");
        }
        let mut found = BTreeSet::new();
        references(
            "use fake_bidder::FakeBidder; let _ = \"fake_alias\";"
                .parse()
                .unwrap(),
            &names,
            &mut found,
        );
        assert!(found.is_empty());
    }
}

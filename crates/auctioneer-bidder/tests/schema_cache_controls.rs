//! Generated explicit-cache behavior, not a complete resource qualification.

#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
fn target(value: &str) -> bool {
    value == "allowed"
}

#[cfg(test)]
fn test_panic(value: &str) -> bool {
    assert_ne!(value, "panic", "deliberately injected callback panic");
    value == "allowed"
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::Path;
    use std::process::Command;
    use std::sync::Barrier;
    use std::thread;
    use syn::visit_mut::{self, VisitMut};

    fn expanded(schema: &serde_json::Value, options: &str) -> syn::File {
        let literal = syn::LitStr::new(&schema.to_string(), proc_macro2::Span::call_site())
            .token()
            .to_string();
        let attributes = format!("schema = {literal}, {options}");
        syn::parse2(jsonschema_macros_core::expand(
            attributes.parse().unwrap(),
            "pub struct Control;".parse().unwrap(),
        ))
        .unwrap()
    }

    fn expansion_error(options: &str) -> String {
        let syntax = expanded(&json!({}), options);
        let errors: Vec<_> = syntax
            .items
            .iter()
            .filter_map(|item| {
                let syn::Item::Macro(mac) = item else {
                    return None;
                };
                mac.mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "compile_error")
                    .then(|| mac.mac.parse_body::<syn::LitStr>().unwrap().value())
            })
            .collect();
        assert_eq!(errors.len(), 1, "expected one compiler refusal");
        errors.into_iter().next().unwrap()
    }

    #[test]
    fn repeated_and_collapsed_patterns_cannot_bypass_engine_construction_limits() {
        use quote::quote;

        struct Refusals(Vec<String>);
        impl VisitMut for Refusals {
            fn visit_macro_mut(&mut self, i: &mut syn::Macro) {
                if i.path.is_ident("compile_error") {
                    self.0.push(i.parse_body::<syn::LitStr>().unwrap().value());
                }
                visit_mut::visit_macro_mut(self, i);
            }
        }

        for pattern in ["^[a-z]{1,10000000}$", "a{1,10000000}"] {
            for explicit in [false, true] {
                for configured in [false, true] {
                    let mut builder = regex::RegexBuilder::new(pattern);
                    let limits = if configured {
                        builder.size_limit(0x0001_0000).dfa_size_limit(0x0001_0000);
                        quote!(size_limit = 65_536, dfa_size_limit = 65_536,)
                    } else {
                        quote!()
                    };
                    assert!(builder.build().is_err(), "engine must refuse {pattern}");
                    let cache = explicit.then(|| quote!(cache = explicit,));
                    let schema = syn::LitStr::new(
                        &json!({"type": "string", "pattern": pattern}).to_string(),
                        proc_macro2::Span::call_site(),
                    );
                    let mut syntax: syn::File = syn::parse2(jsonschema_macros_core::expand(
                        quote!(
                            schema = #schema, draft = Draft4,
                            pattern_options = { engine = regex, #cache #limits },
                            methods = { is_valid = true, validate = false, iter_errors = false }
                        ),
                        quote!(
                            pub struct BoundedPattern;
                        ),
                    ))
                    .unwrap();
                    let mut errors = Refusals(Vec::new());
                    errors.visit_file_mut(&mut syntax);
                    assert_eq!(
                        errors.0,
                        [format!("Invalid `pattern` regular expression: {pattern}")],
                        "pattern={pattern}, explicit={explicit}, configured={configured}"
                    );
                }
            }
        }
    }

    #[test]
    fn direct_automata_dependency_preserves_the_locked_engine_feature_closure() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let output = Command::new("cargo")
            .args(["metadata", "--offline", "--locked", "--format-version", "1"])
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let features = engine_features(&metadata).unwrap();
        assert_eq!(
            features,
            BTreeSet::from([
                "alloc",
                "dfa-onepass",
                "dfa-search",
                "hybrid",
                "meta",
                "nfa",
                "nfa-backtrack",
                "nfa-pikevm",
                "nfa-thompson",
                "perf",
                "perf-inline",
                "perf-literal",
                "perf-literal-multisubstring",
                "perf-literal-substring",
                "std",
                "syntax",
                "unicode",
                "unicode-age",
                "unicode-bool",
                "unicode-case",
                "unicode-gencat",
                "unicode-perl",
                "unicode-script",
                "unicode-segment",
                "unicode-word-boundary",
            ])
        );
    }

    fn engine_features(metadata: &serde_json::Value) -> Result<BTreeSet<&str>, &'static str> {
        let packages = metadata["packages"]
            .as_array()
            .ok_or("engine package identity")?;
        let mut matching = packages
            .iter()
            .filter(|package| package["name"].as_str() == Some("regex-automata"));
        let package = matching.next().ok_or("engine package identity")?;
        if matching.next().is_some() || package["version"].as_str() != Some("0.4.18") {
            return Err("engine package identity");
        }
        let id = package["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("engine package identity")?;
        let nodes = metadata["resolve"]["nodes"]
            .as_array()
            .ok_or("engine feature node")?;
        let mut matching_nodes = nodes.iter().filter(|node| node["id"].as_str() == Some(id));
        let node = matching_nodes.next().ok_or("engine feature node")?;
        if matching_nodes.next().is_some() {
            return Err("engine feature node");
        }
        let features = node["features"].as_array().ok_or("engine feature node")?;
        let actual = features
            .iter()
            .map(|value| value.as_str().ok_or("engine feature node"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if actual.len() != features.len() {
            return Err("engine feature node");
        }
        Ok(actual)
    }

    #[test]
    fn feature_resolution_refuses_missing_duplicate_and_wrong_packages() {
        let original = json!({"packages": [{"name": "regex-automata", "version": "0.4.18", "id": "path+file:///reviewed#regex-automata@0.4.18"}], "resolve": {"nodes": [{"id": "path+file:///reviewed#regex-automata@0.4.18", "features": ["alloc"]}]}});
        assert_eq!(engine_features(&original), Ok(BTreeSet::from(["alloc"])));
        let mut missing_package = original.clone();
        missing_package["packages"] = json!([]);
        assert_eq!(
            engine_features(&missing_package),
            Err("engine package identity")
        );
        let mut duplicate_package = original.clone();
        duplicate_package["packages"]
            .as_array_mut()
            .unwrap()
            .push(original["packages"][0].clone());
        assert_eq!(
            engine_features(&duplicate_package),
            Err("engine package identity")
        );
        for (field, value) in [
            ("name", "regex"),
            ("version", "0.4.17"),
            ("id", "another-package"),
        ] {
            let mut changed = original.clone();
            changed["packages"][0][field] = json!(value);
            assert_eq!(
                engine_features(&changed),
                Err(if field == "id" {
                    "engine feature node"
                } else {
                    "engine package identity"
                }),
                "{field}"
            );
        }
        let mut duplicate_node = original.clone();
        duplicate_node["resolve"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(original["resolve"]["nodes"][0].clone());
        assert_eq!(engine_features(&duplicate_node), Err("engine feature node"));
        let mut missing_node = original;
        missing_node["resolve"]["nodes"] = json!([]);
        assert_eq!(engine_features(&missing_node), Err("engine feature node"));
    }

    #[test]
    fn cache_options_refuse_unknown_or_duplicate_policy_values() {
        assert_eq!(
            expansion_error("pattern_options = { engine = regex, cache = hidden }"),
            "Unknown cache policy. Expected `explicit`"
        );
        assert_eq!(
            expansion_error(
                "pattern_options = { engine = regex, cache = explicit, cache = explicit }"
            ),
            "Duplicate pattern_options key: `cache`"
        );
    }

    #[test]
    fn explicit_cache_refuses_incompatible_backends_engines_and_error_methods() {
        for (options, expected) in [
            (
                "pattern_options = { engine = fancy_regex, cache = explicit }",
                "`cache = explicit` requires `engine = regex`",
            ),
            (
                "backend = Pyo3, pattern_options = { engine = regex, cache = explicit }",
                "`cache = explicit` requires `backend = SerdeJson`",
            ),
            (
                "pattern_options = { engine = regex, cache = explicit }",
                "`cache = explicit` requires boolean-only methods: `is_valid = true, validate = false, iter_errors = false`",
            ),
            (
                "pattern_options = { engine = regex, cache = explicit }, methods = { is_valid = true, validate = true, iter_errors = false }",
                "`cache = explicit` requires boolean-only methods: `is_valid = true, validate = false, iter_errors = false`",
            ),
            (
                "pattern_options = { engine = regex, cache = explicit }, methods = { is_valid = true, validate = false, iter_errors = true }",
                "`cache = explicit` requires boolean-only methods: `is_valid = true, validate = false, iter_errors = false`",
            ),
            (
                "pattern_options = { engine = regex, cache = explicit }, methods = { is_valid = false, validate = false, iter_errors = false }",
                "At least one of `is_valid`, `validate`, or `iter_errors` must be enabled",
            ),
        ] {
            assert_eq!(expansion_error(options), expected, "{options}");
        }
    }

    #[derive(Default)]
    struct GeneratedShape {
        regexes: BTreeSet<String>,
        caches: BTreeSet<String>,
        searches: usize,
        pooled: Vec<String>,
        thread_locals: usize,
        boolean_helpers: usize,
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Syn visitors intentionally inherit traversal for nodes they do not inspect"
    )]
    impl VisitMut for GeneratedShape {
        fn visit_item_static_mut(&mut self, i: &mut syn::ItemStatic) {
            if i.ident.to_string().ends_with("_REGEX") {
                assert!(self.regexes.insert(i.ident.to_string()));
            }
            visit_mut::visit_item_static_mut(self, i);
        }

        fn visit_item_struct_mut(&mut self, i: &mut syn::ItemStruct) {
            if i.ident == "__SearchCache" {
                for field in &i.fields {
                    if let syn::Type::Path(path) = &field.ty
                        && path.path.segments.last().unwrap().ident == "Cache"
                    {
                        assert!(
                            self.caches
                                .insert(field.ident.as_ref().unwrap().to_string())
                        );
                    }
                }
            }
            visit_mut::visit_item_struct_mut(self, i);
        }

        fn visit_expr_method_call_mut(&mut self, i: &mut syn::ExprMethodCall) {
            match i.method.to_string().as_str() {
                "search_half_with" => {
                    self.searches = self.searches.checked_add(1).unwrap();
                    assert_eq!(i.args.len(), 2);
                    let syn::Expr::Reference(cache) = &i.args[0] else {
                        panic!("search must borrow its owned cache");
                    };
                    assert!(cache.mutability.is_some());
                    let syn::Expr::Field(field) = cache.expr.as_ref() else {
                        panic!("search must select an owned cache field");
                    };
                    assert!(
                        matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("__cache"))
                    );
                }
                "is_match" | "search" | "search_half" | "clone" => {
                    self.pooled.push(i.method.to_string());
                }
                _ => {}
            }
            visit_mut::visit_expr_method_call_mut(self, i);
        }

        fn visit_item_macro_mut(&mut self, i: &mut syn::ItemMacro) {
            if i.mac.path.segments.last().unwrap().ident == "thread_local" {
                self.thread_locals = self.thread_locals.checked_add(1).unwrap();
            }
            visit_mut::visit_item_macro_mut(self, i);
        }

        fn visit_item_fn_mut(&mut self, i: &mut syn::ItemFn) {
            if i.sig.ident != "initialize_regexes"
                && matches!(&i.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(path) if path.path.is_ident("bool")))
            {
                self.boolean_helpers = self.boolean_helpers.checked_add(1).unwrap();
                let syn::FnArg::Typed(input) = i.sig.inputs.last().unwrap() else {
                    panic!("boolean helper has no context parameter");
                };
                assert!(
                    matches!(input.ty.as_ref(), syn::Type::Reference(reference) if reference.mutability.is_some() && matches!(reference.elem.as_ref(), syn::Type::Path(path) if path.path.is_ident("__SearchCache")))
                );
            }
            visit_mut::visit_item_fn_mut(self, i);
        }
    }

    #[test]
    fn generated_pattern_caches_biject_with_deduplicated_helpers_without_ambient_searches() {
        let schema = json!({
            "type": "object",
            "properties": {
                "a": {"type": "string", "pattern": "^[a-z]+$"},
                "b": {"type": "string", "pattern": "^[a-z]+$"},
                "c": {"type": "string", "pattern": "[0-9]+"}
            }
        });
        let mut syntax = expanded(
            &schema,
            "pattern_options = { engine = regex, cache = explicit }, methods = { is_valid = true, validate = false, iter_errors = false }",
        );
        let mut shape = GeneratedShape::default();
        shape.visit_file_mut(&mut syntax);
        assert_eq!(shape.regexes.len(), 2);
        assert_eq!(shape.caches.len(), 2);
        assert_eq!(shape.searches, 2);
        assert!(shape.boolean_helpers > 2);
        assert!(shape.pooled.is_empty(), "{:?}", shape.pooled);
        assert_eq!(shape.thread_locals, 0);
        assert_eq!(
            shape.regexes,
            shape
                .caches
                .iter()
                .map(|field| format!(
                    "{}_REGEX",
                    field.strip_suffix("_cache").unwrap().to_uppercase()
                ))
                .collect()
        );
        let public_cache = syntax
            .items
            .iter()
            .find_map(|item| {
                if let syn::Item::Struct(marker) = item
                    && marker.ident == "ControlSearchCache"
                {
                    Some(marker)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(matches!(public_cache.vis, syn::Visibility::Public(_)));
        assert_eq!(public_cache.fields.len(), 1);
        assert!(
            public_cache
                .fields
                .iter()
                .all(|field| matches!(field.vis, syn::Visibility::Inherited))
        );
    }

    #[test]
    fn generated_reference_and_uri_helpers_have_no_thread_local_state() {
        for (schema, draft, format_options) in [
            (
                serde_json::from_str(include_str!(
                    "support/fake_bidder/schemas/cache/recursive.json"
                ))
                .unwrap(),
                "Draft201909",
                "",
            ),
            (
                serde_json::from_str(include_str!(
                    "support/fake_bidder/schemas/cache/recursive-array.json"
                ))
                .unwrap(),
                "Draft201909",
                "",
            ),
            (
                serde_json::from_str(include_str!(
                    "support/fake_bidder/schemas/cache/dynamic.json"
                ))
                .unwrap(),
                "Draft202012",
                "",
            ),
            (
                json!({"format": "uri"}),
                "Draft4",
                "validate_formats = true,",
            ),
        ] {
            let mut syntax = expanded(
                &schema,
                &format!(
                    "draft = {draft}, {format_options} pattern_options = {{ engine = regex, cache = explicit }}, methods = {{ is_valid = true, validate = false, iter_errors = false }}"
                ),
            );
            let mut shape = GeneratedShape::default();
            shape.visit_file_mut(&mut syntax);
            assert!(shape.boolean_helpers > 0);
            assert_eq!(shape.thread_locals, 0, "{schema}");
            assert!(shape.pooled.is_empty(), "{schema}: {:?}", shape.pooled);
        }
    }

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct ExplicitPatterns;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/initialization.json", draft = Draft4,
        pattern_options = { engine = regex, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct DefaultPatterns;

    #[jsonschema::validator(
        schema = "{}", draft = Draft4,
        pattern_options = { engine = regex, cache = explicit },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Empty;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/cache/branches.json", draft = Draft202012,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Branches;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/cache/recursive.json", draft = Draft201909,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Recursive;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/cache/recursive-array.json", draft = Draft201909,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct RecursiveArray;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/cache/dynamic.json", draft = Draft202012,
        pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct Dynamic;

    #[jsonschema::validator(
        schema = "{\"type\":\"string\",\"format\":\"named\"}", draft = Draft4,
        validate_formats = true, formats = { "named" => crate::target },
        pattern_options = { engine = regex, cache = explicit },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct CustomFormat;

    #[jsonschema::validator(
        schema = "{\"type\":\"string\",\"format\":\"uri\"}", draft = Draft4,
        validate_formats = true,
        pattern_options = { engine = regex, cache = explicit },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct UriFormat;

    #[jsonschema::validator(
        path = "tests/support/fake_bidder/schemas/cache/panic-reference.json", draft = Draft202012,
        validate_formats = true, formats = { "test-panic" => crate::test_panic },
        pattern_options = { engine = regex, cache = explicit },
        methods = { is_valid = true, validate = false, iter_errors = false }
    )]
    struct PanicReference;

    #[test]
    fn nested_branches_property_names_and_pattern_properties_share_explicit_context() {
        let mut cache = Branches::new_search_cache().unwrap();
        for (value, expected) in [
            (
                json!({"choice": "prefix123", "items": [7_i32, "zz"], "names": {"ok_name": "123"}}),
                true,
            ),
            (json!({"choice": "none", "items": [], "names": {}}), false),
            (
                json!({"choice": "123", "items": ["bad"], "names": {}}),
                false,
            ),
            (
                json!({"choice": "123", "items": [], "names": {"bad": "123"}}),
                false,
            ),
            (
                json!({"choice": "123", "items": [], "names": {"ok_name": "bad"}}),
                false,
            ),
            (
                json!({"choice": "123", "items": [], "names": {}, "strict": true}),
                false,
            ),
            (
                json!({"choice": "aaa", "items": [], "names": {}, "strict": true}),
                true,
            ),
        ] {
            assert_eq!(Branches::is_valid(&value, &mut cache), expected, "{value}");
        }
    }

    #[test]
    fn recursive_and_dynamic_helpers_keep_state_inside_each_bundle() {
        let mut recursive = Recursive::new_search_cache().unwrap();
        let mut dynamic = Dynamic::new_search_cache().unwrap();
        for (value, expected) in [
            (
                json!({"value": "root", "children": [{"value": "child"}]}),
                true,
            ),
            (
                json!({"value": "root", "children": [{"value": "UPPER"}]}),
                false,
            ),
            (json!({"value": "root", "children": [{}]}), false),
            (json!({"value": "root", "other": true}), false),
        ] {
            assert_eq!(
                Recursive::is_valid(&value, &mut recursive),
                expected,
                "{value}"
            );
            assert_eq!(Dynamic::is_valid(&value, &mut dynamic), expected, "{value}");
        }
        let mut array = RecursiveArray::new_search_cache().unwrap();
        assert!(RecursiveArray::is_valid(
            &json!(["xxx", ["xx"]]),
            &mut array
        ));
        assert!(!RecursiveArray::is_valid(
            &json!(["xxx", ["bad"]]),
            &mut array
        ));
    }

    #[test]
    fn external_formatter_names_and_owned_uri_memoization_preserve_verdicts() {
        let mut custom = CustomFormat::new_search_cache().unwrap();
        assert!(CustomFormat::is_valid(&json!("allowed"), &mut custom));
        assert!(!CustomFormat::is_valid(&json!("denied"), &mut custom));
        let mut uri = UriFormat::new_search_cache().unwrap();
        for _ in 0_usize..4 {
            assert!(UriFormat::is_valid(
                &json!("https://example.invalid/path"),
                &mut uri
            ));
            assert!(!UriFormat::is_valid(&json!("not a URI"), &mut uri));
        }
    }

    #[test]
    fn caught_callback_panics_cannot_leave_permission_granting_reference_marks() {
        let mut cache = PanicReference::new_search_cache().unwrap();
        let mut input = json!({"value": "panic"});
        let panic = catch_unwind(AssertUnwindSafe(|| {
            PanicReference::is_valid(&input, &mut cache)
        }));
        let _payload = panic.unwrap_err();
        input["value"] = json!("denied");
        assert!(!PanicReference::is_valid(&input, &mut cache));
        input["value"] = json!("allowed");
        assert!(PanicReference::is_valid(&input, &mut cache));
    }

    #[test]
    fn explicit_cache_preserves_literal_pattern_verdicts_and_default_matching() {
        assert!(ExplicitPatterns::initialize_regexes());
        let mut cache = ExplicitPatterns::new_search_cache().unwrap();
        for (input, expected) in [
            (json!({"letters": "abc", "digits": "123"}), true),
            (json!({"letters": "A", "digits": "123"}), false),
            (json!({"letters": "abc", "digits": "x"}), false),
            (json!({}), false),
        ] {
            assert_eq!(ExplicitPatterns::is_valid(&input, &mut cache), expected);
            assert_eq!(DefaultPatterns::is_valid(&input), expected);
        }
    }

    #[test]
    fn patternless_validators_still_require_their_own_typed_cache() {
        let mut cache = Empty::new_search_cache().unwrap();
        for input in [json!(null), json!({}), json!([1_i32]), json!("anything")] {
            assert!(Empty::is_valid(&input, &mut cache));
        }
    }

    #[test]
    fn independently_owned_caches_interleave_without_cross_request_state() {
        let mut first = ExplicitPatterns::new_search_cache().unwrap();
        let mut second = ExplicitPatterns::new_search_cache().unwrap();
        let valid = json!({"letters": "abc", "digits": "123"});
        let invalid = json!({"letters": "A", "digits": "x"});
        for _ in 0_usize..8 {
            assert!(ExplicitPatterns::is_valid(&valid, &mut first));
            assert!(!ExplicitPatterns::is_valid(&invalid, &mut second));
            assert!(!ExplicitPatterns::is_valid(&invalid, &mut first));
            assert!(ExplicitPatterns::is_valid(&valid, &mut second));
        }
    }

    #[test]
    fn concurrent_validations_use_separate_explicit_bundles() {
        let barrier = Barrier::new(4);
        thread::scope(|scope| {
            for index in 0_usize..4 {
                let shared_barrier = &barrier;
                scope.spawn(move || {
                    let mut cache = ExplicitPatterns::new_search_cache().unwrap();
                    let input = if index.is_multiple_of(2) {
                        json!({"letters": "abc", "digits": "123"})
                    } else {
                        json!({"letters": "A", "digits": "x"})
                    };
                    shared_barrier.wait();
                    assert_eq!(
                        ExplicitPatterns::is_valid(&input, &mut cache),
                        index.is_multiple_of(2)
                    );
                });
            }
        });
    }
}

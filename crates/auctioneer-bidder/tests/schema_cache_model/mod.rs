//! Logical NFA dimensions and strict emitted constructors; allocator bounds are separate.

use regex_automata::{
    nfa::thompson::{self, NFA, State, WhichCaptures},
    util::{look::LookMatcher, syntax},
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use syn::punctuated::Punctuated;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NfaDimensions {
    pub states: usize,
    pub slots: usize,
    pub patterns: usize,
    pub branch_frames: usize,
    pub capture_states: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PikeDimensions {
    pub set_entries: usize,
    pub table_entries: usize,
    pub meta_capture_entries: usize,
    pub stack_frames: usize,
}

impl fmt::Display for NfaDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "N={} S={} P={} B={} C={}",
            self.states, self.slots, self.patterns, self.branch_frames, self.capture_states
        )
    }
}

impl fmt::Display for PikeDimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "set-entries={} table-entries={} meta-capture-entries={} stack-frames={}",
            self.set_entries, self.table_entries, self.meta_capture_entries, self.stack_frames
        )
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BoundsError {
    Arithmetic,
    Compilation,
    Shape,
}

impl NfaDimensions {
    pub(crate) fn pike_storage(self) -> Result<PikeDimensions, BoundsError> {
        let set_entries = self.states.checked_mul(4).ok_or(BoundsError::Arithmetic)?;
        let pattern_slots = self
            .patterns
            .checked_mul(2)
            .ok_or(BoundsError::Arithmetic)?;
        let table_entries = self
            .states
            .checked_mul(self.slots)
            .and_then(|entries| entries.checked_add(self.slots.max(pattern_slots)))
            .and_then(|entries| entries.checked_mul(2))
            .ok_or(BoundsError::Arithmetic)?;
        let stack_frames = self
            .branch_frames
            .checked_add(self.capture_states)
            .ok_or(BoundsError::Arithmetic)?
            .max(1);
        Ok(PikeDimensions {
            set_entries,
            table_entries,
            meta_capture_entries: self.slots,
            stack_frames,
        })
    }

    pub(crate) fn from_nfa(nfa: &NFA) -> Result<Self, BoundsError> {
        let mut dimensions = Self {
            states: nfa.states().len(),
            slots: nfa.group_info().slot_len(),
            patterns: nfa.pattern_len(),
            branch_frames: 0,
            capture_states: 0,
        };
        for state in nfa.states() {
            let branches = match state {
                State::Union { alternates } => alternates.len().saturating_sub(1),
                State::BinaryUnion { .. } => 1,
                State::Capture { slot, .. } => {
                    if slot.as_usize() >= dimensions.slots {
                        return Err(BoundsError::Shape);
                    }
                    dimensions.capture_states = dimensions
                        .capture_states
                        .checked_add(1)
                        .ok_or(BoundsError::Arithmetic)?;
                    0
                }
                State::ByteRange { .. }
                | State::Sparse(_)
                | State::Dense(_)
                | State::Look { .. }
                | State::Fail
                | State::Match { .. } => 0,
            };
            dimensions.branch_frames = dimensions
                .branch_frames
                .checked_add(branches)
                .ok_or(BoundsError::Arithmetic)?;
        }
        Ok(dimensions)
    }
}

pub(crate) fn forward_nfa(pattern: &str) -> Result<NFA, BoundsError> {
    compiled_nfa(pattern, false)
}

pub(crate) fn reverse_nfa(pattern: &str) -> Result<NFA, BoundsError> {
    compiled_nfa(pattern, true)
}

fn compiled_nfa(pattern: &str, reverse: bool) -> Result<NFA, BoundsError> {
    let mut look = LookMatcher::new();
    look.set_line_terminator(b'\n');
    // Mirrors meta::Config::to_thompson_config for the emitted builder.
    NFA::compiler()
        .configure(
            thompson::Config::new()
                .utf8(true)
                .reverse(reverse)
                .shrink(false)
                .which_captures(if reverse {
                    WhichCaptures::None
                } else {
                    WhichCaptures::All
                })
                .nfa_size_limit(Some(0x0001_0000))
                .look_matcher(look),
        )
        .syntax(syntax::Config::default().utf8(true))
        .build(pattern)
        .map_err(|_build_error| BoundsError::Compilation)
}

pub(crate) fn emitted_patterns(syntax: &syn::File) -> Result<Vec<(String, String)>, BoundsError> {
    let mut patterns = Vec::new();
    let mut owners = 0_usize;
    collect_patterns(&syntax.items, &mut patterns, &mut owners)?;
    if owners != 1 {
        return Err(BoundsError::Shape);
    }
    Ok(patterns)
}

fn collect_patterns(
    items: &[syn::Item],
    patterns: &mut Vec<(String, String)>,
    owners: &mut usize,
) -> Result<(), BoundsError> {
    let mut local = Vec::new();
    for item in items {
        if let syn::Item::Mod(module) = item {
            let (_, children) = module.content.as_ref().ok_or(BoundsError::Shape)?;
            collect_patterns(children, patterns, owners)?;
            continue;
        }
        if let syn::Item::Static(storage) = item {
            if storage.ident.to_string().ends_with("_REGEX") {
                local.push((
                    storage.ident.to_string(),
                    constructor_pattern(&storage.expr)?,
                ));
            }
            continue;
        }
        if matches!(item, syn::Item::Macro(declaration) if declaration.mac.path.is_ident("compile_error"))
        {
            return Err(BoundsError::Shape);
        }
    }
    let declarations: Vec<_> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Struct(declaration) = item
                && declaration.ident == "__SearchCache"
            {
                Some(declaration)
            } else {
                None
            }
        })
        .collect();
    if declarations.is_empty() && local.is_empty() {
        return Ok(());
    }
    let [declaration] = declarations.as_slice() else {
        return Err(BoundsError::Shape);
    };
    let constructors: BTreeSet<_> = local.iter().map(|(name, _)| name.clone()).collect();
    if constructors.len() != local.len() {
        return Err(BoundsError::Shape);
    }
    reconcile_cache(items, declaration, &constructors)?;
    *owners = owners.checked_add(1).ok_or(BoundsError::Arithmetic)?;
    patterns.extend(local);
    Ok(())
}

fn reconcile_cache(
    items: &[syn::Item],
    declaration: &syn::ItemStruct,
    constructors: &BTreeSet<String>,
) -> Result<(), BoundsError> {
    let mut expected = BTreeMap::new();
    for field in &declaration.fields {
        let syn::Type::Path(ty) = &field.ty else {
            return Err(BoundsError::Shape);
        };
        if ty.qself.is_some()
            || ty
                .path
                .segments
                .iter()
                .any(|part| !part.arguments.is_empty())
            || !ty
                .path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .eq(["__ra", "meta", "Cache"])
        {
            return Err(BoundsError::Shape);
        }
        let name = field.ident.as_ref().ok_or(BoundsError::Shape)?.to_string();
        let symbol = format!(
            "{}_REGEX",
            name.strip_suffix("_cache")
                .ok_or(BoundsError::Shape)?
                .to_uppercase()
        );
        if expected.insert(name, symbol).is_some() {
            return Err(BoundsError::Shape);
        }
    }
    let symbols: BTreeSet<_> = expected.values().cloned().collect();
    if symbols != *constructors || symbols.len() != expected.len() {
        return Err(BoundsError::Shape);
    }
    let factories: Vec<_> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Fn(function) = item
                && function.sig.ident == "new_search_cache"
            {
                Some(function)
            } else {
                None
            }
        })
        .collect();
    let [factory] = factories.as_slice() else {
        return Err(BoundsError::Shape);
    };
    let Some(syn::Stmt::Expr(result, None)) = factory.block.stmts.last() else {
        return Err(BoundsError::Shape);
    };
    let some = call(result, &["Some"], 1)?;
    let syn::Expr::Struct(initializer) = first_argument(&some.args)? else {
        return Err(BoundsError::Shape);
    };
    if !initializer.path.is_ident("__SearchCache") || initializer.rest.is_some() {
        return Err(BoundsError::Shape);
    }
    let mut actual = BTreeMap::new();
    for field in &initializer.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err(BoundsError::Shape);
        };
        let create = method(&field.expr, "create_cache", 0)?;
        let syn::Expr::Try(try_ref) = create.receiver.as_ref() else {
            return Err(BoundsError::Shape);
        };
        let borrow = method(&try_ref.expr, "as_ref", 0)?;
        let syn::Expr::Path(storage) = borrow.receiver.as_ref() else {
            return Err(BoundsError::Shape);
        };
        let symbol = storage
            .path
            .get_ident()
            .ok_or(BoundsError::Shape)?
            .to_string();
        if actual.insert(name.to_string(), symbol).is_some() {
            return Err(BoundsError::Shape);
        }
    }
    if actual != expected {
        return Err(BoundsError::Shape);
    }
    Ok(())
}

fn method<'a>(
    expr: &'a syn::Expr,
    name: &str,
    arguments: usize,
) -> Result<&'a syn::ExprMethodCall, BoundsError> {
    let syn::Expr::MethodCall(invoke) = expr else {
        return Err(BoundsError::Shape);
    };
    if invoke.method == name && invoke.args.len() == arguments {
        Ok(invoke)
    } else {
        Err(BoundsError::Shape)
    }
}

fn call<'a>(
    expr: &'a syn::Expr,
    path: &[&str],
    arguments: usize,
) -> Result<&'a syn::ExprCall, BoundsError> {
    let syn::Expr::Call(invoke) = expr else {
        return Err(BoundsError::Shape);
    };
    if invoke.args.len() == arguments && path_is(&invoke.func, path) {
        Ok(invoke)
    } else {
        Err(BoundsError::Shape)
    }
}

fn path_is(expr: &syn::Expr, names: &[&str]) -> bool {
    matches!(expr, syn::Expr::Path(path) if path.qself.is_none()
        && path.path.segments.len() == names.len()
        && path.path.segments.iter().zip(names).all(|(part, name)| part.ident == *name && part.arguments.is_empty()))
}

fn integer(expr: &syn::Expr, expected: usize) -> Result<(), BoundsError> {
    if matches!(expr, syn::Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Int(value) if value.base10_parse::<usize>().is_ok_and(|actual| actual == expected)))
    {
        Ok(())
    } else {
        Err(BoundsError::Shape)
    }
}

fn first_argument(
    arguments: &Punctuated<syn::Expr, syn::Token![,]>,
) -> Result<&syn::Expr, BoundsError> {
    arguments.first().ok_or(BoundsError::Shape)
}

fn utf8_setting<'a>(expr: &'a syn::Expr, name: &str) -> Result<&'a syn::Expr, BoundsError> {
    let setting = method(expr, name, 1)?;
    if !matches!(first_argument(&setting.args)?, syn::Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Bool(value) if value.value))
    {
        return Err(BoundsError::Shape);
    }
    Ok(&setting.receiver)
}

fn check_config(expr: &syn::Expr) -> Result<(), BoundsError> {
    let pool = method(expr, "pool_capacity", 1)?;
    integer(first_argument(&pool.args)?, 1)?;
    let hybrid = method(&pool.receiver, "hybrid_cache_capacity", 1)?;
    integer(first_argument(&hybrid.args)?, 0x0001_0000)?;
    let nfa = method(&hybrid.receiver, "nfa_size_limit", 1)?;
    let some = call(first_argument(&nfa.args)?, &["Some"], 1)?;
    integer(first_argument(&some.args)?, 0x0001_0000)?;
    let kind = method(utf8_setting(&nfa.receiver, "utf8_empty")?, "match_kind", 1)?;
    if !path_is(
        first_argument(&kind.args)?,
        &["__ra", "MatchKind", "LeftmostFirst"],
    ) {
        return Err(BoundsError::Shape);
    }
    call(&kind.receiver, &["__ra", "meta", "Regex", "config"], 0)?;
    Ok(())
}

fn constructor_pattern(expr: &syn::Expr) -> Result<String, BoundsError> {
    let new = call(expr, &["__Lazy", "new"], 1)?;
    let syn::Expr::Closure(closure) = first_argument(&new.args)? else {
        return Err(BoundsError::Shape);
    };
    let syn::Expr::Block(body) = closure.body.as_ref() else {
        return Err(BoundsError::Shape);
    };
    let [syn::Stmt::Expr(result, None)] = body.block.stmts.as_slice() else {
        return Err(BoundsError::Shape);
    };
    let ok = method(result, "ok", 0)?;
    let build = method(&ok.receiver, "build", 1)?;
    let syntax = method(&build.receiver, "syntax", 1)?;
    call(
        utf8_setting(first_argument(&syntax.args)?, "utf8")?,
        &["__ra", "util", "syntax", "Config", "default"],
        0,
    )?;
    let configure = method(&syntax.receiver, "configure", 1)?;
    check_config(first_argument(&configure.args)?)?;
    call(
        &configure.receiver,
        &["__ra", "meta", "Regex", "builder"],
        0,
    )?;
    let syn::Expr::Lit(literal) = first_argument(&build.args)? else {
        return Err(BoundsError::Shape);
    };
    let syn::Lit::Str(pattern) = &literal.lit else {
        return Err(BoundsError::Shape);
    };
    Ok(pattern.value())
}

#[cfg(test)]
mod tests {
    use super::{BoundsError, NfaDimensions, PikeDimensions, emitted_patterns, forward_nfa};

    const EMITTED: &str = r#"
            mod private {
                static EXAMPLE_REGEX: __Lazy<Option<__ra::meta::Regex>> = __Lazy::new(|| {
                    __ra::meta::Regex::builder()
                        .configure(__ra::meta::Regex::config()
                            .match_kind(__ra::MatchKind::LeftmostFirst).utf8_empty(true)
                            .nfa_size_limit(Some(65536)).hybrid_cache_capacity(65536).pool_capacity(1))
                        .syntax(__ra::util::syntax::Config::default().utf8(true))
                        .build("[0-9]+").ok()
                });
                struct __SearchCache { example_cache: __ra::meta::Cache }
                fn new_search_cache() -> Option<__SearchCache> {
                    if !initialize_regexes() { return None; }
                    Some(__SearchCache { example_cache: EXAMPLE_REGEX.as_ref()?.create_cache() })
                }
            }
        "#;

    #[test]
    fn generated_pattern_extraction_requires_a_literal_in_its_constructor() {
        let syntax = syn::parse_str(EMITTED).unwrap();
        assert_eq!(
            emitted_patterns(&syntax).unwrap(),
            vec![("EXAMPLE_REGEX".to_owned(), "[0-9]+".to_owned())]
        );
        for input in [
            "static EXAMPLE_REGEX: bool = true;",
            "static EXAMPLE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(pattern));",
            "static EXAMPLE_REGEX: Lazy<Regex> = Lazy::new(|| { Regex::builder().build(pattern).ok() });",
            "compile_error!(\"no supported generation\");",
            "mod unexamined;",
        ] {
            assert_eq!(
                emitted_patterns(&syn::parse_str(input).unwrap()),
                Err(BoundsError::Shape)
            );
        }
    }

    #[test]
    fn zero_pattern_schemas_still_require_their_empty_cache_owner_and_factory() {
        let empty = "mod private { struct __SearchCache {} fn new_search_cache() -> Option<__SearchCache> { Some(__SearchCache {}) } }";
        assert_eq!(
            emitted_patterns(&syn::parse_str(empty).unwrap()),
            Ok(Vec::new())
        );
        for changed in [
            empty.replace("struct __SearchCache {}", ""),
            empty.replace("fn new_search_cache()", "fn missing_factory()"),
            empty.replace("Some(__SearchCache {})", "None"),
        ] {
            assert_ne!(changed, empty);
            assert_eq!(
                emitted_patterns(&syn::parse_str(&changed).unwrap()),
                Err(BoundsError::Shape)
            );
        }
    }

    #[test]
    fn each_emitted_builder_policy_must_match_the_nfa_reconstruction() {
        for (before, after) in [
            ("LeftmostFirst", "All"),
            ("utf8_empty(true)", "utf8_empty(false)"),
            ("nfa_size_limit(Some(65536))", "nfa_size_limit(Some(1))"),
            ("hybrid_cache_capacity(65536)", "hybrid_cache_capacity(1)"),
            ("pool_capacity(1)", "pool_capacity(2)"),
            ("utf8(true)", "utf8(false)"),
            ("build(\"[0-9]+\")", "build(runtime_pattern)"),
            ("meta::Regex::builder()", "different::Regex::builder()"),
            (".ok()", ".unwrap()"),
        ] {
            let changed = EMITTED.replace(before, after);
            assert_ne!(changed, EMITTED);
            assert_eq!(
                emitted_patterns(&syn::parse_str(&changed).unwrap()),
                Err(BoundsError::Shape),
                "{before}"
            );
        }
    }

    #[test]
    fn generated_cache_slots_and_initializers_must_biject_with_constructors() {
        for (before, after) in [
            (
                "example_cache: __ra::meta::Cache",
                "example_cache: __ra::meta::Cache, extra_cache: __ra::meta::Cache",
            ),
            ("example_cache: __ra::meta::Cache", ""),
            (
                "example_cache: __ra::meta::Cache",
                "example_cache: __ra::meta::Regex",
            ),
            (
                "example_cache: __ra::meta::Cache",
                "example_cache: __ra::meta::Cache<Extra>",
            ),
            ("EXAMPLE_REGEX.as_ref()", "UNKNOWN_REGEX.as_ref()"),
            (".create_cache()", ".pooled_cache()"),
            ("fn new_search_cache()", "fn missing_constructor()"),
        ] {
            let changed = EMITTED.replace(before, after);
            assert_ne!(changed, EMITTED);
            assert_eq!(
                emitted_patterns(&syn::parse_str(&changed).unwrap()),
                Err(BoundsError::Shape),
                "{before}"
            );
        }
    }

    #[test]
    fn slot_table_scratch_addition_and_final_doubling_refuse_independently() {
        for slots in [usize::MAX, usize::MAX >> 1_usize] {
            let dimensions = NfaDimensions {
                states: 1,
                slots,
                patterns: 0,
                branch_frames: 0,
                capture_states: 0,
            };
            assert_eq!(dimensions.pike_storage(), Err(BoundsError::Arithmetic));
        }
    }

    #[test]
    fn repeated_capture_states_are_counted_separately_from_distinct_slots() {
        let nfa = forward_nfa("(a){3}").unwrap();
        let dimensions = NfaDimensions::from_nfa(&nfa).unwrap();
        assert_eq!(dimensions.slots, 4);
        assert_eq!(dimensions.capture_states, 8);
        assert_eq!(dimensions.patterns, 1);
        assert!(!nfa.is_reverse());
        assert!(nfa.is_utf8());
    }

    #[test]
    fn anchored_alternation_funds_every_pending_branch() {
        let nfa = forward_nfa("^(a|ab|abc)$").unwrap();
        let dimensions = NfaDimensions::from_nfa(&nfa).unwrap();
        assert_eq!(dimensions.branch_frames, 2);
        assert_eq!(dimensions.capture_states, 4);
        assert_eq!(dimensions.pike_storage().unwrap().stack_frames, 6);
    }

    #[test]
    fn invalid_and_over_limit_nfas_are_not_reported_as_zero_cost() {
        for pattern in ["[", "(a){100000}"] {
            assert!(matches!(
                forward_nfa(pattern),
                Err(BoundsError::Compilation)
            ));
        }
    }

    fn independent_dimensions() -> NfaDimensions {
        NfaDimensions {
            states: 17,
            slots: 6,
            patterns: 2,
            branch_frames: 11,
            capture_states: 9,
        }
    }

    #[test]
    fn logical_pike_storage_counts_both_active_sets_and_meta_captures() {
        assert_eq!(
            independent_dimensions().pike_storage(),
            Ok(PikeDimensions {
                set_entries: 68,
                table_entries: 216,
                meta_capture_entries: 6,
                stack_frames: 20,
            })
        );
    }

    #[test]
    fn each_independent_nfa_dimension_changes_only_its_storage_terms() {
        for (dimensions, expected) in [
            (
                NfaDimensions {
                    states: 19,
                    ..independent_dimensions()
                },
                (76, 240, 6, 20),
            ),
            (
                NfaDimensions {
                    slots: 8,
                    ..independent_dimensions()
                },
                (68, 288, 8, 20),
            ),
            (
                NfaDimensions {
                    patterns: 5,
                    ..independent_dimensions()
                },
                (68, 224, 6, 20),
            ),
            (
                NfaDimensions {
                    branch_frames: 13,
                    ..independent_dimensions()
                },
                (68, 216, 6, 22),
            ),
            (
                NfaDimensions {
                    capture_states: 10,
                    ..independent_dimensions()
                },
                (68, 216, 6, 21),
            ),
        ] {
            let actual = dimensions.pike_storage().unwrap();
            assert_eq!(
                (
                    actual.set_entries,
                    actual.table_entries,
                    actual.meta_capture_entries,
                    actual.stack_frames
                ),
                expected,
                "{dimensions:?}"
            );
        }
    }

    #[test]
    fn epsilon_stack_funds_the_initial_exploration_even_without_branches() {
        let dimensions = NfaDimensions {
            states: 1,
            slots: 0,
            patterns: 0,
            branch_frames: 0,
            capture_states: 0,
        };
        assert_eq!(dimensions.pike_storage().unwrap().stack_frames, 1);
    }

    #[test]
    fn each_storage_projection_overflow_refuses_instead_of_wrapping() {
        for dimensions in [
            NfaDimensions {
                states: usize::MAX,
                ..independent_dimensions()
            },
            NfaDimensions {
                slots: usize::MAX,
                ..independent_dimensions()
            },
            NfaDimensions {
                patterns: usize::MAX,
                ..independent_dimensions()
            },
            NfaDimensions {
                branch_frames: usize::MAX,
                ..independent_dimensions()
            },
            NfaDimensions {
                capture_states: usize::MAX,
                ..independent_dimensions()
            },
        ] {
            assert_eq!(dimensions.pike_storage(), Err(BoundsError::Arithmetic));
        }
    }
}

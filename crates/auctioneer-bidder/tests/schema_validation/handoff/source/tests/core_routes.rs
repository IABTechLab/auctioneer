//! Rehashed core-route controls for the existing actual-source consumption case.

use super::{
    CapturedSeed, Event, Fixture, HelperInterpretationRefusal, HelperSourceJoinRefusal,
    SourceError, ValidationView, digest, expected_events, module_mut, replay, with_source_data,
    work,
};
use quote::ToTokens as _;
use std::{rc::Rc, str::from_utf8};

#[derive(Clone, Copy, Debug)]
enum CoreDrift {
    LibPreludePath,
    LibPreludeCfg,
    LibMacrosPath,
    LibMacrosCfg,
    LibCfg,
    PreludeV1Path,
    PreludeV1Cfg,
    PreludeEditionCfg,
    PreludeEditionRoute,
    PreludeCfg,
    V1AssertAlias,
    V1Cfg,
    BuiltinCfg,
    AssertNested,
}

impl CoreDrift {
    const ALL: [Self; 14] = [
        Self::LibPreludePath,
        Self::LibPreludeCfg,
        Self::LibMacrosPath,
        Self::LibMacrosCfg,
        Self::LibCfg,
        Self::PreludeV1Path,
        Self::PreludeV1Cfg,
        Self::PreludeEditionCfg,
        Self::PreludeEditionRoute,
        Self::PreludeCfg,
        Self::V1AssertAlias,
        Self::V1Cfg,
        Self::BuiltinCfg,
        Self::AssertNested,
    ];

    fn unit(self) -> &'static str {
        match self {
            Self::LibPreludePath
            | Self::LibPreludeCfg
            | Self::LibMacrosPath
            | Self::LibMacrosCfg
            | Self::LibCfg => "CoreLib",
            Self::PreludeV1Path
            | Self::PreludeV1Cfg
            | Self::PreludeEditionCfg
            | Self::PreludeEditionRoute
            | Self::PreludeCfg => "CorePrelude",
            Self::V1AssertAlias | Self::V1Cfg => "CorePreludeV1",
            Self::BuiltinCfg | Self::AssertNested => "Macros",
        }
    }
}

fn add_module_path(module: &mut syn::ItemMod, attribute: syn::Attribute) {
    assert!(
        !module
            .attrs
            .iter()
            .any(|existing| existing.path().is_ident("path"))
    );
    module.attrs.push(attribute);
}

fn add_item_cfg(attributes: &mut Vec<syn::Attribute>) {
    assert!(
        !attributes
            .iter()
            .any(|attribute| attribute.path().is_ident("cfg"))
    );
    attributes.push(syn::parse_quote!(#[cfg(any())]));
}

fn add_root_cfg(syntax: &mut syn::File) {
    assert!(
        !syntax
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("cfg"))
    );
    syntax.attrs.push(syn::parse_quote!(#![cfg(any())]));
}

fn redirect_edition(syntax: &mut syn::File) {
    let edition = module_mut(syntax, "rust_2021");
    let (_, children) = edition.content.as_mut().unwrap();
    let pristine: syn::UseTree = syn::parse_quote!(super::v1::*);
    let routes: Vec<_> = children
        .iter_mut()
        .filter_map(|item| {
            let syn::Item::Use(import) = item else {
                return None;
            };
            (import.tree.to_token_stream().to_string() == pristine.to_token_stream().to_string())
                .then_some(import)
        })
        .collect();
    assert_eq!(routes.len(), 1);
    for route in routes {
        assert!(matches!(route.vis, syn::Visibility::Public(_)));
        assert!(route.leading_colon.is_none());
        route.tree = syn::parse_quote!(super::fake_v1::*);
    }
}

fn alias_assert(syntax: &mut syn::File) {
    let mut leaves = Vec::new();
    for item in &mut syntax.items {
        let syn::Item::Use(import) = item else {
            continue;
        };
        let syn::UseTree::Path(root) = &mut import.tree else {
            continue;
        };
        if root.ident != "crate" {
            continue;
        }
        let syn::UseTree::Group(group) = root.tree.as_mut() else {
            continue;
        };
        for leaf in &mut group.items {
            if matches!(leaf, syn::UseTree::Name(name) if name.ident == "assert") {
                assert!(matches!(import.vis, syn::Visibility::Public(_)));
                assert!(import.leading_colon.is_none());
                leaves.push(leaf);
            }
        }
    }
    assert_eq!(leaves.len(), 1);
    let mut renamed = 0_usize;
    for leaf in leaves {
        *leaf = syn::parse_quote!(assert as fake_assert);
        renamed = renamed.checked_add(1).unwrap();
    }
    assert_eq!(renamed, 1);
}

fn nest_assert(syntax: &mut syn::File) {
    let builtin = module_mut(syntax, "builtin");
    let (_, children) = builtin.content.as_mut().unwrap();
    assert!(
        !children
            .iter()
            .any(|item| { matches!(item, syn::Item::Mod(module) if module.ident == "fake_scope") })
    );
    let positions: Vec<_> = children
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let syn::Item::Macro(definition) = item else {
                return None;
            };
            definition
                .ident
                .as_ref()
                .is_some_and(|name| name == "assert")
                .then_some(index)
        })
        .collect();
    assert_eq!(positions.len(), 1);
    // Move the exact item: the old recursive body collector still finds it.
    let moved = children.remove(positions[0]);
    let mut nested: syn::ItemMod = syn::parse_quote!(
        mod fake_scope {}
    );
    nested.content.as_mut().unwrap().1.push(moved);
    children.push(syn::Item::Mod(nested));
}

fn edit_route(syntax: &mut syn::File, change: CoreDrift) {
    match change {
        CoreDrift::LibPreludePath => add_module_path(
            module_mut(syntax, "prelude"),
            syn::parse_quote!(#[path = "fake_prelude.rs"]),
        ),
        CoreDrift::LibPreludeCfg => {
            add_item_cfg(&mut module_mut(syntax, "prelude").attrs);
        }
        CoreDrift::LibMacrosPath => add_module_path(
            module_mut(syntax, "macros"),
            syn::parse_quote!(#[path = "fake_macros.rs"]),
        ),
        CoreDrift::LibMacrosCfg => {
            add_item_cfg(&mut module_mut(syntax, "macros").attrs);
        }
        CoreDrift::LibCfg | CoreDrift::PreludeCfg | CoreDrift::V1Cfg => {
            add_root_cfg(syntax);
        }
        CoreDrift::PreludeV1Path => add_module_path(
            module_mut(syntax, "v1"),
            syn::parse_quote!(#[path = "fake_v1.rs"]),
        ),
        CoreDrift::PreludeV1Cfg => {
            add_item_cfg(&mut module_mut(syntax, "v1").attrs);
        }
        CoreDrift::PreludeEditionCfg => {
            add_item_cfg(&mut module_mut(syntax, "rust_2021").attrs);
        }
        CoreDrift::PreludeEditionRoute => redirect_edition(syntax),
        CoreDrift::V1AssertAlias => alias_assert(syntax),
        CoreDrift::BuiltinCfg => {
            let builtin = module_mut(syntax, "builtin");
            assert!(builtin.content.is_some());
            add_item_cfg(&mut builtin.attrs);
        }
        CoreDrift::AssertNested => nest_assert(syntax),
    }
}

fn core_source_drift(data: &Fixture, change: CoreDrift) -> Fixture {
    let mut changed = Fixture {
        expected: data.expected.clone(),
        observed: data.observed.clone(),
        sources: data.sources.clone(),
    };
    let original = data
        .sources
        .iter()
        .find(|source| source.unit == change.unit())
        .unwrap();
    let record = changed
        .sources
        .iter_mut()
        .find(|source| source.unit == change.unit())
        .unwrap();
    let mut syntax = syn::parse_file(from_utf8(&original.bytes).unwrap()).unwrap();
    edit_route(&mut syntax, change);
    record.bytes = syntax.to_token_stream().to_string().into_bytes();
    assert_ne!(record.bytes, original.bytes, "{change:?} changes source");
    record.sha256 = digest(&record.bytes);
    for unit in ["Nfa", "Sparse", "Ids"] {
        let before = data
            .sources
            .iter()
            .find(|source| source.unit == unit)
            .unwrap();
        let after = changed
            .sources
            .iter()
            .find(|source| source.unit == unit)
            .unwrap();
        assert_eq!(after, before, "helper bodies and fields stay unchanged");
    }
    changed
}

pub(super) fn assert_controls(seed: &CapturedSeed, view: ValidationView<'_>, data: &Fixture) {
    let demands = work::conditional_demands();
    work::assert_demand_setup(&demands);
    let mut checked = 0_usize;
    for change in CoreDrift::ALL {
        let changed = core_source_drift(data, change);
        let mut reads = replay(&changed);
        let checks = Rc::clone(&reads.events);
        let interpretation = Rc::clone(&reads.events);
        let mut costs = None;
        let actual = with_source_data(
            seed,
            view,
            &mut reads,
            |phase| checks.borrow_mut().push(Event::Check(phase)),
            |inputs| {
                interpretation.borrow_mut().push(Event::Interpret);
                costs = Some([
                    work::join_helper_costs(inputs, &demands[3]),
                    work::join_helper_costs(inputs, &demands[5]),
                ]);
                work::check_helper_source_setup(inputs)
            },
        );
        assert_eq!(
            *reads.events.borrow(),
            expected_events(),
            "{change:?}: close on every refusal"
        );
        assert_eq!(
            actual,
            Err(SourceError::Interpretation(
                HelperSourceJoinRefusal::SourceInterpretation(
                    HelperInterpretationRefusal::CoreAncestry(change.unit())
                )
            )),
            "{change:?}"
        );
        for cost in costs.unwrap() {
            assert_eq!(
                cost,
                Err(HelperSourceJoinRefusal::SourceInterpretation(
                    HelperInterpretationRefusal::CoreAncestry(change.unit())
                )),
                "{change:?}"
            );
            checked = checked.checked_add(1).unwrap();
        }
    }
    assert_eq!(checked, 28);
}

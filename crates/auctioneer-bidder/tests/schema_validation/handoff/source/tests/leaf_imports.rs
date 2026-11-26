//! Rehashed Ids import-shape controls in the existing actual-source case.
//!
//! This is not complete builtin-derive resolution. Custom attribute/macro
//! producers and actual derive/caller/cfg/artifact identity remain unproved.

use super::{
    CapturedSeed, Event, Fixture, HelperInterpretationRefusal, HelperSourceJoinRefusal,
    HelperSourceUnit, SourceError, ValidationView, digest, expected_events, replay,
    with_source_data, work,
};
use quote::ToTokens as _;
use std::{rc::Rc, str::from_utf8};

#[derive(Clone, Copy, Debug)]
enum LeafImport {
    Named,
    Renamed,
    GroupedRenamed,
    Glob,
    CoreGroupedRenamed,
    CfgNamed,
}

impl LeafImport {
    const ALL: [Self; 6] = [
        Self::Named,
        Self::Renamed,
        Self::GroupedRenamed,
        Self::Glob,
        Self::CoreGroupedRenamed,
        Self::CfgNamed,
    ];
}

fn import_tokens(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Use(import) = item else {
                return None;
            };
            Some(import.to_token_stream().to_string())
        })
        .collect()
}

fn pristine_imports(syntax: &syn::File) -> Vec<String> {
    let expected: [syn::ItemUse; 3] = [
        syn::parse_quote!(
            use core::num::NonZeroUsize;
        ),
        syn::parse_quote!(
            #[cfg(feature = "alloc")]
            use alloc::vec::Vec;
        ),
        syn::parse_quote!(
            use crate::util::int::{Usize, U16, U32, U64};
        ),
    ];
    let actual = import_tokens(syntax);
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|import| import.to_token_stream().to_string())
            .collect::<Vec<_>>(),
        "exact pristine Ids imports, including attributes and visibility"
    );
    actual
}

fn without_imports(syntax: &syn::File) -> String {
    let mut remaining = syntax.clone();
    remaining
        .items
        .retain(|item| !matches!(item, syn::Item::Use(_)));
    remaining.to_token_stream().to_string()
}

fn core_import_mut(syntax: &mut syn::File) -> &mut syn::ItemUse {
    let pristine: syn::ItemUse = syn::parse_quote!(
        use core::num::NonZeroUsize;
    );
    let expected = pristine.to_token_stream().to_string();
    let mut matching = syntax.items.iter_mut().filter_map(|item| {
        let syn::Item::Use(import) = item else {
            return None;
        };
        (import.to_token_stream().to_string() == expected).then_some(import)
    });
    let import = matching.next().unwrap();
    assert!(matching.next().is_none(), "one exact pristine core import");
    import
}

fn edit_imports(syntax: &mut syn::File, change: LeafImport) {
    let import: syn::ItemUse = match change {
        LeafImport::Named => syn::parse_quote!(
            use fake_derives::PartialEq;
        ),
        LeafImport::Renamed => {
            syn::parse_quote!(
                use fake_derives::WorkfulPartialEq as PartialEq;
            )
        }
        LeafImport::GroupedRenamed => {
            syn::parse_quote!(
                use fake_derives::{WorkfulPartialEq as PartialEq};
            )
        }
        LeafImport::Glob => syn::parse_quote!(
            use fake_derives::*;
        ),
        LeafImport::CoreGroupedRenamed => {
            // This source has a standalone core import, not an existing group.
            // Preserve its leaf and independently anchor the new group to it.
            core_import_mut(syntax).tree = syn::parse_quote!(
                core::{num::NonZeroUsize, clone::Clone as PartialEq}
            );
            return;
        }
        LeafImport::CfgNamed => syn::parse_quote!(
            #[cfg(any())]
            use fake_derives::PartialEq;
        ),
    };
    syntax.items.push(syn::Item::Use(import));
}

fn ids_source(data: &Fixture, edit: impl FnOnce(&mut syn::File)) -> Fixture {
    assert_eq!(data.sources.len(), 20);
    let positions: Vec<_> = data
        .sources
        .iter()
        .enumerate()
        .filter_map(|(index, source)| (source.unit == "Ids").then_some(index))
        .collect();
    assert_eq!(positions.len(), 1);
    let original = &data.sources[positions[0]];
    assert_eq!(original.sha256, digest(&original.bytes));
    let mut changed = Fixture {
        expected: data.expected.clone(),
        observed: data.observed.clone(),
        sources: data.sources.clone(),
    };
    let record = &mut changed.sources[positions[0]];
    assert_eq!(
        *record, *original,
        "exact pristine owned replay before edit"
    );
    let mut syntax = syn::parse_file(from_utf8(&original.bytes).unwrap()).unwrap();
    let imports = pristine_imports(&syntax);
    let declarations = without_imports(&syntax);
    edit(&mut syntax);
    assert_eq!(
        without_imports(&syntax),
        declarations,
        "all Ids declarations, helper bodies and non-import attributes unchanged"
    );
    assert_eq!(
        import_tokens(&syntax).get(1),
        imports.get(1),
        "exact required Vec import and alloc gate unchanged"
    );
    record.bytes = syntax.to_token_stream().to_string().into_bytes();
    assert_ne!(record.bytes, original.bytes, "import control changes bytes");
    record.sha256 = digest(&record.bytes);
    assert_ne!(record.sha256, original.sha256);
    let mut unchanged = 0_usize;
    for (before, after) in data.sources.iter().zip(&changed.sources) {
        if before.unit == "Ids" {
            assert_eq!(after.unit, before.unit);
            assert_eq!(after.path, before.path);
        } else {
            assert_eq!(after, before, "all other records, including Nfa/Sparse");
            unchanged = unchanged.checked_add(1).unwrap();
        }
    }
    assert_eq!(unchanged, 19);
    changed
}

fn assert_documentation_positive(seed: &CapturedSeed, view: ValidationView<'_>, data: &Fixture) {
    let documented = ids_source(data, |syntax| {
        core_import_mut(syntax)
            .attrs
            .push(syn::parse_quote!(#[doc = "Literal import documentation."]));
    });
    let demands = work::conditional_demands();
    work::assert_demand_setup(&demands);
    let mut reads = replay(&documented);
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
            costs = Some(
                demands
                    .iter()
                    .map(|demand| work::join_helper_costs(inputs, demand))
                    .collect::<Vec<_>>(),
            );
            work::check_helper_source_setup(inputs)
        },
    );
    assert_eq!(*reads.events.borrow(), expected_events());
    assert_eq!(actual, Ok(()), "literal import docs normalize positively");
    work::assert_cost_projections(&demands, &costs.unwrap());
}

pub(super) fn assert_controls(seed: &CapturedSeed, view: ValidationView<'_>, data: &Fixture) {
    assert_documentation_positive(seed, view, data);
    let demands = work::conditional_demands();
    work::assert_demand_setup(&demands);
    let mut checked = 0_usize;
    for change in LeafImport::ALL {
        let changed = ids_source(data, |syntax| edit_imports(syntax, change));
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
                // Positive and zero Sparse calls have identical source duties.
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
                    HelperInterpretationRefusal::UnsupportedBody(HelperSourceUnit::Ids)
                )
            )),
            "{change:?}"
        );
        for cost in costs.unwrap() {
            assert_eq!(
                cost,
                Err(HelperSourceJoinRefusal::SourceInterpretation(
                    HelperInterpretationRefusal::UnsupportedBody(HelperSourceUnit::Ids)
                )),
                "{change:?}"
            );
            checked = checked.checked_add(1).unwrap();
        }
    }
    assert_eq!(checked, 12);
}

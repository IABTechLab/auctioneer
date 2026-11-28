//! Common validation proof consumer; no serving or qualification conversion.

mod admission;
mod handoff;
#[cfg(test)]
mod observations;
mod operations;
mod public_names;
mod report;
mod stack;
mod storage;
mod work;

pub(crate) use observations::selected_cache_observations;
pub(crate) use report::current_report;

#[cfg(test)]
mod tests {
    use super::{handoff, operations, storage, work::partition};
    use crate::schema_residency::{
        descriptor::ValidationView, expanded::DeclarationBody, with_current_descriptor,
    };
    use crate::schema_selection_source::{Lane, public_names};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;
    use std::ptr;

    struct BodyAnalysis {
        owner: String,
        marker: String,
        operations: Result<operations::BodyOperations, operations::Refusal>,
        schedule: Option<Result<storage::OwnerSchedule, String>>,
        factory: Option<Result<partition::CoupledFactoryBounds, partition::CoupledRefusal>>,
        input: Option<Result<partition::CoupledFactoryBounds, partition::CoupledRefusal>>,
        searches: Option<Result<partition::CoupledSearchBounds, partition::CoupledRefusal>>,
        bundle: Option<Result<crate::schema_bundle_heap::CacheBundleHeap, String>>,
    }

    struct ProgramHeapAnalysis {
        identity: String,
        heap:
            Result<crate::schema_meta_heap::MetaCacheHeap, crate::schema_meta_heap::MetaHeapError>,
        components: Result<[usize; 4], String>,
    }

    struct BundleAnalysis {
        owner: String,
        result: Result<[usize; 4], String>,
    }

    // Component integration only: existing independent literal cases own the
    // formulas. Reusing these child owners does not issue common-proof credit.
    fn program_heap_components(
        nfas: [&regex_automata::nfa::thompson::NFA; 2],
        layout: crate::schema_heap_layout::HeapLayout,
    ) -> Result<[usize; 4], String> {
        use crate::schema_fallback_heap::FallbackHeap;
        use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
        use crate::schema_hybrid_heap::HybridHeap;
        use crate::schema_meta_eager::CoreEagerHeap;

        let project_direction = |nfa, kind| {
            let dfa = candidate_hybrid(nfa, kind).map_err(|error| format!("{error:?}"))?;
            HybridHeap::from_complete_dfa(&dfa, layout)
                .map_err(|error| format!("{error:?}"))?
                .map(|heap| heap.live_envelope)
                .ok_or_else(|| "conditional recovery unavailable".to_owned())
        };
        Ok([
            CoreEagerHeap::from_nfa(nfas[0], layout)
                .map_err(|error| format!("{error:?}"))?
                .total,
            project_direction(nfas[0], HybridDirection::Forward)?,
            project_direction(nfas[1], HybridDirection::Reverse)?,
            FallbackHeap::from_nfa(nfas[0], layout, 65_536)
                .map_err(|error| format!("{error:?}"))?
                .live_envelope,
        ])
    }

    fn inspect_program_heaps(view: ValidationView<'_>) -> (Vec<ProgramHeapAnalysis>, Vec<String>) {
        use crate::schema_heap_layout::HeapLayout;
        use crate::schema_hybrid_prefix::for_meta_pattern;
        use crate::schema_meta_heap::MetaCacheHeap;

        // Supplied native model DATA, not actual target/layout authority.
        let layout = HeapLayout::new(size_of::<usize>(), 16).unwrap();
        let rows = view
            .programs()
            .map(|(identity, pattern, nfas)| ProgramHeapAnalysis {
                identity: identity.to_owned(),
                heap: MetaCacheHeap::from_nfas(nfas, 65_536, layout, || for_meta_pattern(pattern)),
                components: program_heap_components(nfas, layout),
            })
            .collect();
        let mut expected = view.graph()["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row["programs"].as_array().unwrap())
            .map(|program| program["identity"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        expected.push(view.runtime_hostname().0.to_owned());
        (rows, expected)
    }

    fn assert_program_heaps((rows, expected): (Vec<ProgramHeapAnalysis>, Vec<String>)) {
        assert!(!expected.is_empty());
        {
            let expected_set = expected.iter().collect::<BTreeSet<_>>();
            let actual_set = rows
                .iter()
                .map(|row| &row.identity)
                .collect::<BTreeSet<_>>();
            assert_eq!(expected_set.len(), expected.len());
            assert_eq!(actual_set.len(), rows.len());
            assert_eq!(actual_set, expected_set, "exact graph plus runtime owner");
        }
        let mut asymmetric = false;
        for row in rows {
            assert!(
                row.components.is_ok(),
                "conditional child setup {}: {:?}",
                row.identity,
                row.components
            );
            let components = row.components.unwrap();
            asymmetric |= components[1] != components[2];
            assert_eq!(
                row.heap.map(|heap| [
                    heap.eager_bytes,
                    heap.forward_bytes,
                    heap.reverse_bytes,
                    heap.fallback_bytes,
                ]),
                Ok(components),
                "borrowed program components: {}",
                row.identity
            );
        }
        assert!(
            asymmetric,
            "the source fixture must discriminate directions"
        );
    }

    fn inspect_body(
        body: &DeclarationBody<'_>,
        view: ValidationView<'_>,
        heaps: &[ProgramHeapAnalysis],
    ) -> BodyAnalysis {
        let operations = operations::classify_source_body(body.syntax, &[], &[]);
        let schedule = operations.as_ref().ok().map(|classified| {
            storage::owner_schedule_data(classified.recipe(), view.graph(), body.owner, body.marker)
                .map_err(|error| format!("{error:?}"))
        });
        let factory = schedule.as_ref().and_then(|scheduled| {
            scheduled.as_ref().ok()?;
            let classified = operations.as_ref().ok()?;
            // Supplied, independently varied test DATA. These are not the
            // actual caller populations, installed J cap or artifact facts.
            Some(partition::coupled_factory_bounds(
                classified.recipe(),
                view.graph(),
                body.owner,
                body.marker,
                7,
                partition::CoupledPopulation {
                    factories: 3,
                    validations: 2,
                },
            ))
        });
        let input = factory.as_ref().and_then(|bounded| {
            bounded.as_ref().ok()?;
            let classified = operations.as_ref().ok()?;
            Some(partition::coupled_input_bounds(
                classified.recipe(),
                view.graph(),
                body.owner,
                body.marker,
                7,
                partition::CoupledPopulation {
                    factories: 3,
                    validations: 2,
                },
            ))
        });
        let searches = input.as_ref().and_then(|bounded| {
            bounded.as_ref().ok()?;
            let classified = operations.as_ref().ok()?;
            Some(partition::coupled_search_bounds(
                classified.recipe(),
                view.graph(),
                body.owner,
                body.marker,
                7,
                partition::CoupledPopulation {
                    factories: 3,
                    validations: 2,
                },
            ))
        });
        let bundle = schedule.as_ref().and_then(|scheduled| {
            let owner_schedule = scheduled.as_ref().ok()?;
            Some(inspect_bundle(body, view, owner_schedule, heaps))
        });
        BodyAnalysis {
            owner: body.owner.to_owned(),
            marker: body.marker.to_owned(),
            operations,
            schedule,
            factory,
            input,
            searches,
            bundle,
        }
    }

    // Joins actual membership to conditional native component DATA. This is
    // not a whole-constructor/drop or compiled-wrapper-layout certificate.
    fn inspect_bundle(
        body: &DeclarationBody<'_>,
        view: ValidationView<'_>,
        schedule: &storage::OwnerSchedule,
        heaps: &[ProgramHeapAnalysis],
    ) -> Result<crate::schema_bundle_heap::CacheBundleHeap, String> {
        use operations::Primitive;
        use regex_automata::meta::Cache;

        let declarations = view.graph()["declarations"]
            .as_array()
            .ok_or("declarations")?;
        let mut matching_declarations = declarations.iter().filter(|row| {
            row["owner"].as_str() == Some(body.owner) && row["marker"].as_str() == Some(body.marker)
        });
        let declaration = matching_declarations.next().ok_or("declaration")?;
        if matching_declarations.next().is_some() {
            return Err("duplicate declaration".into());
        }
        let programs = declaration["programs"].as_array().ok_or("programs")?;
        let mut costs = Vec::new();
        for (_, name, _, _) in schedule.initializers() {
            let symbol = name.rsplit_once("::").ok_or("program name")?.1;
            let mut declared = programs
                .iter()
                .filter(|row| row["symbol"].as_str() == Some(symbol));
            let declared_program = declared.next().ok_or("missing declared program")?;
            if declared.next().is_some() {
                return Err("duplicate declared program".into());
            }
            let identity = declared_program["identity"]
                .as_str()
                .ok_or("program identity")?;
            costs.push((name.to_owned(), program_peak(heaps, identity)?));
        }
        if costs.len() != programs.len() {
            return Err("incomplete program population".into());
        }
        let has_hostname = schedule
            .callbacks()
            .any(|(_, kind, _)| kind == Primitive::HostnameFormat);
        let hostname = if has_hostname {
            Some(
                program_peak(heaps, view.runtime_hostname().0)?
                    .checked_add(size_of::<Cache>())
                    .ok_or("hostname inline overflow")?,
            )
        } else {
            None
        };
        let inline = costs
            .len()
            .checked_mul(size_of::<Cache>())
            .ok_or("wrapper inline overflow")?;
        schedule
            .component_bundle(&costs, inline, hostname)
            .map_err(|error| format!("{error:?}"))
    }

    fn program_peak(heaps: &[ProgramHeapAnalysis], identity: &str) -> Result<usize, String> {
        let mut matching = heaps.iter().filter(|row| row.identity == identity);
        let row = matching.next().ok_or("missing borrowed program cost")?;
        if matching.next().is_some() {
            return Err("duplicate borrowed program cost".into());
        }
        row.heap
            .as_ref()
            .map(|heap| heap.peak)
            .map_err(|error| format!("{error:?}"))
    }

    // Independent metadata join, not a second numeric oracle. Literal unit
    // rows own arithmetic; this follows descriptor IDs rather than initializer
    // symbols or program_peak, keeping the lookup under test out of its oracle.
    fn expected_cache_bundles(
        view: ValidationView<'_>,
        heaps: &[ProgramHeapAnalysis],
    ) -> Result<BTreeMap<String, [usize; 4]>, String> {
        use crate::schema_bundle_heap::CacheBundleHeap;
        use regex_automata::meta::Cache;

        let mut by_identity = BTreeMap::new();
        for row in heaps {
            let heap = row.heap.as_ref().map_err(|error| format!("{error:?}"))?;
            if by_identity
                .insert(row.identity.as_str(), heap.peak)
                .is_some()
            {
                return Err("duplicate expected program identity".into());
            }
        }
        let mut bundles = BTreeMap::new();
        for declaration in view.graph()["declarations"]
            .as_array()
            .ok_or("declarations")?
        {
            let mut peaks = Vec::new();
            for program in declaration["programs"].as_array().ok_or("programs")? {
                let identity = program["identity"].as_str().ok_or("program identity")?;
                peaks.push(*by_identity.get(identity).ok_or("expected program cost")?);
            }
            let callback = if declaration["callbacks"]
                .as_array()
                .ok_or("callbacks")?
                .iter()
                .any(|format| format.as_str() == Some("hostname"))
            {
                by_identity
                    .get(view.runtime_hostname().0)
                    .ok_or("expected hostname cost")?
                    .checked_add(size_of::<Cache>())
                    .ok_or("expected hostname overflow")?
            } else {
                0
            };
            let inline = peaks
                .len()
                .checked_mul(size_of::<Cache>())
                .ok_or("expected inline overflow")?;
            let heap = CacheBundleHeap::compose(&peaks, inline, callback)
                .map_err(|error| format!("{error:?}"))?;
            let owner = declaration["owner"].as_str().ok_or("owner")?.to_owned();
            if bundles
                .insert(
                    owner,
                    [
                        heap.pattern_bytes,
                        heap.inline_bytes,
                        heap.format_bytes,
                        heap.peak,
                    ],
                )
                .is_some()
            {
                return Err("duplicate expected owner".into());
            }
        }
        Ok(bundles)
    }

    fn assert_body_setup(analyses: &[BodyAnalysis], view: ValidationView<'_>) {
        assert!(!analyses.is_empty());
        let expected: Vec<_> = view.graph()["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["owner"].as_str().unwrap(),
                    row["marker"].as_str().unwrap(),
                )
            })
            .collect();
        let actual: Vec<_> = analyses
            .iter()
            .map(|row| (row.owner.as_str(), row.marker.as_str()))
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(
            actual.iter().copied().collect::<BTreeSet<_>>().len(),
            actual.len()
        );
        let refusals: Vec<_> = analyses
            .iter()
            .filter_map(|row| {
                row.operations
                    .as_ref()
                    .err()
                    .map(|reason| (row.owner.as_str(), reason))
            })
            .collect();
        assert!(
            refusals.is_empty(),
            "unclassified declarations: {refusals:#?}"
        );
        for row in analyses {
            assert!(
                matches!(&row.schedule, Some(Ok(_))),
                "existing factory owner setup for {}::{}: {:?}",
                row.owner,
                row.marker,
                row.schedule
            );
        }
    }

    // Check the exact borrowed graph before the final whole-feature gate.
    fn assert_owned_program_topologies(view: ValidationView<'_>) {
        use crate::schema_cache_bounds::NfaDimensions;
        use crate::schema_nfa_heap::logical_dimensions;

        let programs = view.programs().collect::<Vec<_>>();
        let again = view.programs().collect::<Vec<_>>();
        // Evaluate every program/direction before asserting any proof result.
        let checked = programs
            .iter()
            .flat_map(|(id, _, nfas)| {
                nfas.iter()
                    .map(move |nfa| (*id, *nfa, logical_dimensions(nfa)))
            })
            .collect::<Vec<_>>();
        assert_eq!(view.recheck(), Ok(()));

        let mut expected = BTreeMap::new();
        for declaration in view.graph()["declarations"].as_array().unwrap() {
            for program in declaration["programs"].as_array().unwrap() {
                let id = program["identity"].as_str().unwrap();
                let pattern = program["pattern"].as_str().unwrap();
                assert!(
                    expected.insert(id, pattern).is_none(),
                    "duplicate graph program: {id}"
                );
            }
        }
        let (hostname, hostname_pattern) = view.runtime_hostname();
        assert!(expected.insert(hostname, hostname_pattern).is_none());
        assert!(!expected.is_empty());
        assert!(!programs.is_empty());
        let actual = programs
            .iter()
            .map(|(id, pattern, _)| (*id, *pattern))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(actual.len(), programs.len(), "duplicate borrowed program");
        assert_eq!(
            actual, expected,
            "exact graph plus checked hostname correspondence"
        );
        assert_eq!(again.len(), programs.len());
        for ((id, pattern, nfas), (again_id, again_pattern, again_nfas)) in
            programs.iter().zip(&again)
        {
            assert_eq!(id, again_id);
            assert_eq!(pattern, again_pattern);
            assert!(ptr::eq(*id, *again_id));
            assert!(ptr::eq(*pattern, *again_pattern));
            assert!(ptr::eq(nfas[0], again_nfas[0]));
            assert!(ptr::eq(nfas[1], again_nfas[1]));
            assert!(!nfas[0].is_reverse(), "forward borrow: {id}");
            assert!(nfas[1].is_reverse(), "reverse borrow: {id}");
        }
        let expected_directions = expected
            .keys()
            .flat_map(|id| [(*id, false), (*id, true)])
            .collect::<BTreeSet<_>>();
        let actual_directions = checked
            .iter()
            .map(|(id, nfa, _)| (*id, nfa.is_reverse()))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_directions.len(),
            checked.len(),
            "duplicate program/direction"
        );
        assert_eq!(actual_directions, expected_directions);
        for (id, nfa, result) in checked {
            let dimensions = NfaDimensions::from_nfa(nfa).unwrap();
            assert_eq!(
                result,
                Ok(dimensions),
                "shared logical checker: {id}, reverse={}",
                nfa.is_reverse()
            );
            let proof = result.unwrap();
            assert_eq!(proof.states, nfa.states().len());
            assert_eq!(proof.patterns, nfa.pattern_len());
            assert_eq!(proof.slots, nfa.group_info().slot_len());
            // Consume the returned n/b/capture dimensions through their
            // existing component owner, not a target ABI or another counter.
            let storage = proof.pike_storage().unwrap();
            assert_eq!(storage.meta_capture_entries, nfa.group_info().slot_len());
            assert_eq!(
                storage.stack_frames,
                proof
                    .branch_frames
                    .checked_add(proof.capture_states)
                    .unwrap()
                    .max(1)
            );
        }
        assert_eq!(view.recheck(), Ok(()));
        // Sparse initialization, epochs, start-cache, search invocations,
        // caller/cleanup and common completion remain unproved here. No new
        // zero values or target facts discharge those source obligations.
    }

    #[test]
    fn current_body_shapes_have_declared_operation_obligations() {
        let (document, projection_rows, bundle_rows, expected_bundles) =
            with_current_descriptor(|seed, descriptor| {
                let view = descriptor.validation_view().unwrap();
                let heap_results = inspect_program_heaps(view);
                let result = handoff::checked_bodies(seed, view, |body| {
                    Ok(inspect_body(&body, view, &heap_results.0))
                });
                assert!(
                    result.is_ok(),
                    "source bodies must first pass the sealed join"
                );
                let bodies = result.unwrap();
                assert_eq!(bodies.recheck(view), Ok(()));
                assert!(!bodies.analyses().is_empty());
                assert_eq!(
                    bodies.analyses().len(),
                    view.graph()["declarations"].as_array().unwrap().len()
                );
                assert_body_setup(bodies.analyses(), view);
                assert_coupled_results(bodies.analyses());
                assert_eq!(bodies.recheck(view), Ok(()));
                assert_owned_program_topologies(view);
                let bundle_results = bodies
                    .analyses()
                    .iter()
                    .map(|row| {
                        let result = row.bundle.as_ref().expect("source owner schedule checked");
                        BundleAnalysis {
                            owner: row.owner.clone(),
                            result: result
                                .as_ref()
                                .map(|heap| {
                                    [
                                        heap.pattern_bytes,
                                        heap.inline_bytes,
                                        heap.format_bytes,
                                        heap.peak,
                                    ]
                                })
                                .map_err(Clone::clone),
                        }
                    })
                    .collect::<Vec<_>>();
                let expected_bundles = expected_cache_bundles(view, &heap_results.0);
                assert_eq!(view.recheck(), Ok(()));
                // Keep whole-feature completion last so an unfinished derivation
                // cannot mask the existing factory, input and search controls.
                let lane = if cfg!(feature = "insecure-bidders") {
                    Lane::Development
                } else {
                    Lane::Secure
                };
                let census = public_names::discover(Path::new(env!("CARGO_MANIFEST_DIR")), lane)?;
                let report = super::current_report(seed, view, &census)?;
                assert_eq!(super::report::check_current_report(&report), Ok(()));
                for field in [
                    "resource_qualification",
                    "physical_fit",
                    "serving_permission",
                ] {
                    assert_eq!(report[field], false);
                }
                Ok((report, heap_results, bundle_results, expected_bundles))
            })
            .unwrap();
        // New result assertions follow both inner and outer capture checks.
        assert_program_heaps(projection_rows);
        assert_actual_bundles(bundle_rows, &expected_bundles.unwrap());
        assert_eq!(document["status"], "CommonSourceComplete");
    }

    fn assert_actual_bundles(
        bundle_rows: Vec<BundleAnalysis>,
        expected_bundles: &BTreeMap<String, [usize; 4]>,
    ) {
        assert!(!bundle_rows.is_empty());
        assert_eq!(expected_bundles.len(), bundle_rows.len());
        assert_eq!(
            bundle_rows
                .iter()
                .map(|row| &row.owner)
                .collect::<BTreeSet<_>>(),
            expected_bundles.keys().collect::<BTreeSet<_>>()
        );
        let mut no_patterns = false;
        let mut with_patterns = false;
        let mut with_callback = false;
        for BundleAnalysis { owner, result } in bundle_rows {
            assert!(
                result.is_ok(),
                "source-scheduled component bundle {owner}: {result:?}"
            );
            let fields = result.unwrap();
            assert_eq!(
                &fields,
                expected_bundles.get(&owner).unwrap(),
                "independent descriptor-ID cost join: {owner}"
            );
            let [patterns, _, callback, _] = fields;
            no_patterns |= patterns == 0;
            with_patterns |= patterns > 0;
            with_callback |= callback > 0;
        }
        assert!(
            no_patterns && with_patterns && with_callback,
            "nondegenerate actual source population"
        );
    }

    // Original independent source-stage checks; no result is weakened or
    // skipped by buffering the new component-bundle results for closing checks.
    fn assert_coupled_results(analyses: &[BodyAnalysis]) {
        for row in analyses {
            let factory_result = row.factory.as_ref().expect("factory owner setup checked");
            assert_eq!(
                factory_result.as_ref().map(|_| ()),
                Ok(()),
                "coupled factory frontier for {}::{}",
                row.owner,
                row.marker
            );
        }
        for row in analyses {
            let input_result = row.input.as_ref().expect("coupled factory setup checked");
            assert_eq!(
                input_result.as_ref().map(|_| ()),
                Ok(()),
                "coupled input frontier for {}::{}",
                row.owner,
                row.marker
            );
        }
        for row in analyses {
            let search_result = row.searches.as_ref().expect("input setup checked");
            assert_eq!(
                search_result.as_ref().map(|_| ()),
                Ok(()),
                "coupled search frontier for {}::{}",
                row.owner,
                row.marker
            );
        }
    }
}

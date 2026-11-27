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
    }

    struct ProgramHeapAnalysis {
        identity: String,
        heap:
            Result<crate::schema_meta_heap::MetaCacheHeap, crate::schema_meta_heap::MetaHeapError>,
        components: Result<[usize; 4], String>,
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

    fn inspect_body(body: &DeclarationBody<'_>, view: ValidationView<'_>) -> BodyAnalysis {
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
        BodyAnalysis {
            owner: body.owner.to_owned(),
            marker: body.marker.to_owned(),
            operations,
            schedule,
            factory,
            input,
            searches,
        }
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
        let (document, projection_rows) = with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let result = handoff::checked_bodies(seed, view, |body| Ok(inspect_body(&body, view)));
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
            // The new factory result is buffered through the same closing
            // checks. Its success will not close input grammar or source work.
            for row in bodies.analyses() {
                let factory_result = row.factory.as_ref().expect("factory owner setup checked");
                assert_eq!(
                    factory_result.as_ref().map(|_| ()),
                    Ok(()),
                    "coupled factory frontier for {}::{}",
                    row.owner,
                    row.marker
                );
            }
            for row in bodies.analyses() {
                let input_result = row.input.as_ref().expect("coupled factory setup checked");
                assert_eq!(
                    input_result.as_ref().map(|_| ()),
                    Ok(()),
                    "coupled input frontier for {}::{}",
                    row.owner,
                    row.marker
                );
            }
            for row in bodies.analyses() {
                let search_result = row.searches.as_ref().expect("input setup checked");
                assert_eq!(
                    search_result.as_ref().map(|_| ()),
                    Ok(()),
                    "coupled search frontier for {}::{}",
                    row.owner,
                    row.marker
                );
            }
            assert_eq!(bodies.recheck(view), Ok(()));
            assert_owned_program_topologies(view);
            let heap_results = inspect_program_heaps(view);
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
            Ok((report, heap_results))
        })
        .unwrap();
        // New result assertions follow both inner and outer capture checks.
        assert_program_heaps(projection_rows);
        assert_eq!(document["status"], "CommonSourceComplete");
    }
}

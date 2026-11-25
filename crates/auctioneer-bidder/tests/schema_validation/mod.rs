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
    use std::collections::BTreeSet;
    use std::path::Path;

    struct BodyAnalysis {
        owner: String,
        marker: String,
        operations: Result<operations::BodyOperations, operations::Refusal>,
        schedule: Option<Result<storage::OwnerSchedule, String>>,
        factory: Option<Result<partition::CoupledFactoryBounds, partition::CoupledRefusal>>,
        input: Option<Result<partition::CoupledFactoryBounds, partition::CoupledRefusal>>,
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
        BodyAnalysis {
            owner: body.owner.to_owned(),
            marker: body.marker.to_owned(),
            operations,
            schedule,
            factory,
            input,
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

    #[test]
    fn current_body_shapes_have_declared_operation_obligations() {
        with_current_descriptor(|seed, descriptor| {
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
            // Declaring primitive obligations is not discharging their work,
            // storage or stack lemmas, nor does it issue completeness authority.
            let lane = if cfg!(feature = "insecure-bidders") {
                Lane::Development
            } else {
                Lane::Secure
            };
            let census = public_names::discover(Path::new(env!("CARGO_MANIFEST_DIR")), lane)?;
            let report = super::current_report(seed, view, &census)?;
            assert_eq!(super::report::check_current_report(&report), Ok(()));
            assert_eq!(report["status"], "Incomplete");
            for field in [
                "resource_qualification",
                "physical_fit",
                "serving_permission",
            ] {
                assert_eq!(report[field], false);
            }
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
            assert_eq!(bodies.recheck(view), Ok(()));
            Ok(())
        })
        .unwrap();
    }
}

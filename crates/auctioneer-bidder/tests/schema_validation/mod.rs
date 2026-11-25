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
    use super::{handoff, operations};
    use crate::schema_residency::with_current_descriptor;
    use crate::schema_selection_source::{Lane, public_names};
    use std::path::Path;

    #[test]
    fn current_body_shapes_have_declared_operation_obligations() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let result = handoff::checked_bodies(seed, view, |body| {
                Ok((
                    body.owner.to_owned(),
                    operations::classify_source_body(body.syntax, &[], &[]),
                ))
            });
            assert!(
                result.is_ok(),
                "source bodies must first pass the sealed join"
            );
            let bodies = result.unwrap();
            assert!(!bodies.analyses().is_empty());
            assert_eq!(
                bodies.analyses().len(),
                view.graph()["declarations"].as_array().unwrap().len()
            );
            let refusals: Vec<_> = bodies
                .analyses()
                .iter()
                .filter_map(|(owner, operations)| {
                    operations.as_ref().err().map(|reason| (owner, reason))
                })
                .collect();
            assert!(
                refusals.is_empty(),
                "unclassified declarations: {refusals:#?}"
            );
            assert_eq!(bodies.recheck(view), Ok(()));
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
            Ok(())
        })
        .unwrap();
    }
}

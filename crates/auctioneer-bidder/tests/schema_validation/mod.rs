//! Common validation proof consumer; no serving or qualification conversion.

mod admission;
mod handoff;
mod operations;
mod public_names;
mod stack;
mod storage;
mod work;

#[cfg(test)]
mod tests {
    use super::{handoff, operations};
    use crate::schema_residency::with_current_descriptor;

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
            Ok(())
        })
        .unwrap();
    }
}

//! Source-bound ownership descriptions; never physical costs or serving admission.

mod auxiliary;
pub(crate) mod descriptor;
pub(crate) mod expanded;
mod growth;
mod initialization;
mod literal_construction;
mod observations;
mod options;
mod program_construction;
mod retained;
pub(super) mod runtime_sources;
mod storage;

#[cfg(test)]
mod tests {
    use crate::schema_selection_source::{
        Lane, independent_membership, independent_source_membership,
    };
    use std::path::Path;

    #[test]
    fn isolated_data_oracle_preserves_current_membership_in_each_feature_lane() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for lane in [Lane::Secure, Lane::Development] {
            assert_eq!(
                independent_source_membership(root, lane).unwrap(),
                independent_membership(lane).unwrap(),
            );
        }
    }
}

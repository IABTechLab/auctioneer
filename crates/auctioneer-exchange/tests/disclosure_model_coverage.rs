//! Host-driven proof: new model slots require explicit scrub coverage.
//!
//! Run the ignored test after fetching the locked dependencies. It checks only
//! scratch copies, never edits a live model, and runs its children serially.

#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod coverage;

#[cfg(test)]
mod tests {
    use super::coverage::{ModelMutation, ScratchWorkspace};

    const MUTATIONS: [ModelMutation; 3] = [
        ModelMutation {
            model: "User",
            source: "crates/auctioneer-openrtb/src/types/user.rs",
            field: "scrub_coverage_user",
            sparse: false,
        },
        ModelMutation {
            model: "Device",
            source: "crates/auctioneer-openrtb/src/types/device.rs",
            field: "scrub_coverage_device",
            sparse: false,
        },
        ModelMutation {
            model: "BrandVersionFields",
            source: "crates/auctioneer-openrtb/src/types/device.rs",
            field: "scrub_coverage_backing",
            sparse: true,
        },
    ];

    #[test]
    #[ignore = "host-driven offline Cargo checks over a disposable workspace; run explicitly in CI"]
    fn new_model_fields_require_exhaustive_scrub_adapter_coverage() {
        let scratch = ScratchWorkspace::new();
        scratch.assert_checks("pristine baseline");
        for mutation in &MUTATIONS {
            scratch.assert_missing_coverage(mutation);
            scratch.assert_checks(&format!("restored {} baseline", mutation.model));
        }
    }

    #[test]
    fn mutations_add_defaultable_fields_to_the_actual_scoped_models() {
        let sources = [
            include_str!("../../auctioneer-openrtb/src/types/user.rs"),
            include_str!("../../auctioneer-openrtb/src/types/device.rs"),
            include_str!("../../auctioneer-openrtb/src/types/device.rs"),
        ];
        for (mutation, source) in MUTATIONS.iter().zip(sources) {
            let changed = mutation.insert_into(source);
            let anchor = format!("pub struct {} {{\n", mutation.model);
            let declaration = if mutation.sparse {
                format!(
                    "    #[serde(default, skip_serializing_if = \"JsonField::is_missing\")]\n    pub {}: JsonField<String>,\n",
                    mutation.field
                )
            } else {
                format!(
                    "    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n    pub {}: Option<String>,\n",
                    mutation.field
                )
            };
            assert!(changed.contains(&format!("{anchor}{declaration}")));
            assert_eq!(changed.matches(mutation.field).count(), 1);
            assert_eq!(changed.replacen(&declaration, "", 1), source);
        }
    }
}

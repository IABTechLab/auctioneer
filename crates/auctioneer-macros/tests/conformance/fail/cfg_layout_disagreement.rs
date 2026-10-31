use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::{
    ExpectedField, ExpectedModel, FieldMetadata, Storage, graph_matches,
};

#[derive(OpenRtbFieldMetadata)]
struct Root {
    value: String,
    #[cfg(all())]
    added_on_target: bool,
    #[cfg(any())]
    removed_on_target: bool,
}

const ROOT: &auctioneer_openrtb::field_metadata::Model = Root::MODEL.unwrap();
const COMMON: ExpectedField<'static> = ExpectedField {
    name: "value",
    storage: Storage::Typed,
    flattened: false,
    child: None,
};
// Frozen host signatures: the target activates one field and deactivates another.
const _: () = assert!(
    graph_matches(
        ROOT,
        &[ExpectedModel {
            wire_name: "Root",
            rust_name: ROOT.rust_name,
            fields: &[COMMON]
        },]
    ),
    "cfg activated a field absent from the host signature"
);
const _: () = assert!(
    graph_matches(
        ROOT,
        &[ExpectedModel {
            wire_name: "Root",
            rust_name: ROOT.rust_name,
            fields: &[
                COMMON,
                ExpectedField {
                    name: "added_on_target",
                    storage: Storage::Typed,
                    flattened: false,
                    child: None
                },
                ExpectedField {
                    name: "removed_on_target",
                    storage: Storage::Typed,
                    flattened: false,
                    child: None
                },
            ]
        },]
    ),
    "cfg removed a field present in the host signature"
);
fn main() {}

use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::{FieldMetadata, Storage};

mod relocation;

#[derive(OpenRtbFieldMetadata)]
struct Root {
    geo: Option<relocation::Geo>,
    ext: relocation::ReexportedRaw,
}

fn main() {
    assert_eq!(Root::STORAGE, Storage::Typed);
    let root = Root::MODEL.expect("compiled root");
    assert_eq!(root.fields.len(), 2);

    let geo = &root.fields[0];
    assert_eq!(geo.name, "geo");
    assert_eq!(geo.storage, Storage::Typed);
    assert!(!geo.flattened);
    let model = geo.child.expect("relocated model");
    assert!(model.same_declaration(relocation::Geo::MODEL.expect("reexported model")));
    assert_eq!(model.wire_name, "Geo");
    assert_eq!(model.fields.len(), 1);
    let ext = &model.fields[0];
    assert_eq!(ext.name, "ext");
    assert_eq!(ext.storage, Storage::RawJson);
    assert!(!ext.flattened);
    assert!(ext.child.is_none());

    let ext = &root.fields[1];
    assert_eq!(ext.name, "ext");
    assert_eq!(ext.storage, Storage::RawJson);
    assert!(!ext.flattened);
    assert!(ext.child.is_none());
}

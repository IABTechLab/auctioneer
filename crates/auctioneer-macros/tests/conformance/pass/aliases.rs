use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::{FieldMetadata, Storage};

mod wire {
    pub use serde_json::value::RawValue as Opaque;
    pub type Raw<T = Opaque> = Option<Box<T>>;
}

#[derive(OpenRtbFieldMetadata)]
struct Root {
    ext: wire::Raw,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

fn main() {
    let model = Root::MODEL.unwrap();
    assert_eq!(model.fields[0].storage, Storage::RawJson);
    assert_eq!(model.fields[1].storage, Storage::JsonMap);
    assert!(model.fields[1].flattened);
}

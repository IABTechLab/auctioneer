use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::{FieldMetadata, Storage};

type RawValue = String;
trait Wire {
    type Owned;
}
impl Wire for &'static () {
    type Owned = String;
}

#[derive(OpenRtbFieldMetadata)]
#[openrtb_metadata(name = "WireRoot", crate_path = auctioneer_openrtb::field_metadata)]
struct Root {
    shadowed: RawValue,
    projected: <&'static () as Wire>::Owned,
}

fn main() {
    let model = Root::MODEL.unwrap();
    assert_eq!(model.wire_name, "WireRoot");
    assert!(
        model
            .fields
            .iter()
            .all(|field| field.storage == Storage::Typed)
    );
}

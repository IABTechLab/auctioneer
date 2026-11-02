use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::{FieldMetadata, Storage};
use auctioneer_openrtb::types::field::JsonField;
use serde_json::value::RawValue;
use std::ops::Deref;

mod wire {
    pub use serde_json::value::RawValue as Opaque;
    pub type Alias = Opaque;
    pub type Raw<T = Opaque> = Option<Box<T>>;
}

mod exported {
    pub use super::wire::{Opaque as ReexportedRaw, Raw as DefaultRaw};
}

macro_rules! raw_value {
    () => {
        RawValue
    };
    (owned) => {
        Box<RawValue>
    };
}

type MacroRaw = raw_value!();
type RawProjection = <Box<RawValue> as Deref>::Target;

#[derive(OpenRtbFieldMetadata)]
struct Child {
    direct: Box<RawValue>,
    alias: Option<Box<wire::Alias>>,
    reexport: JsonField<Box<exported::ReexportedRaw>>,
    default_generic: exported::DefaultRaw,
    projected: Box<<Box<RawValue> as Deref>::Target>,
    projected_alias: Option<JsonField<Box<RawProjection>>>,
    projected_vec: Vec<Box<RawProjection>>,
    macro_alias: Box<MacroRaw>,
    macro_direct: raw_value!(owned),
}

#[derive(OpenRtbFieldMetadata)]
struct Root {
    child: Option<Box<Child>>,
}

fn main() {
    let child = Child::MODEL.expect("compiled raw child");
    let names = [
        "direct",
        "alias",
        "reexport",
        "default_generic",
        "projected",
        "projected_alias",
        "projected_vec",
        "macro_alias",
        "macro_direct",
    ];
    assert_eq!(child.fields.len(), names.len());
    for (field, name) in child.fields.iter().zip(names) {
        assert_eq!(field.name, name);
        assert_eq!(field.storage, Storage::RawJson, "{name}");
        assert!(!field.flattened, "{name}");
        assert!(field.child.is_none(), "{name}");
    }

    assert_eq!(Child::STORAGE, Storage::Typed);
    assert_eq!(Root::STORAGE, Storage::Typed);
    let root = Root::MODEL.expect("compiled typed parent");
    assert_eq!(root.fields.len(), 1);
    let field = &root.fields[0];
    assert_eq!(field.name, "child");
    assert_eq!(field.storage, Storage::Typed);
    assert!(!field.flattened);
    assert!(
        field
            .child
            .expect("raw child model")
            .same_declaration(child)
    );
}

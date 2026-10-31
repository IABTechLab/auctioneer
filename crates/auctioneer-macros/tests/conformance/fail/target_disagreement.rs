use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::{
    ExpectedField, ExpectedModel, FieldMetadata, Identity, Storage, graph_matches,
};
#[derive(OpenRtbFieldMetadata)]
struct Child {
    value: String,
}
#[derive(OpenRtbFieldMetadata)]
struct Root {
    ext: Box<serde_json::value::RawValue>,
    child: Child,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}
const ROOT: &auctioneer_openrtb::field_metadata::Model = Root::MODEL.unwrap();
const CHILD: &auctioneer_openrtb::field_metadata::Model = Child::MODEL.unwrap();
const FIELDS: &[ExpectedField<'static>] = &[
    ExpectedField {
        name: "ext",
        storage: Storage::RawJson,
        flattened: false,
        child: None,
    },
    ExpectedField {
        name: "child",
        storage: Storage::Typed,
        flattened: false,
        child: Some(Identity {
            wire_name: "Child",
            rust_name: CHILD.rust_name,
        }),
    },
    ExpectedField {
        name: "other",
        storage: Storage::JsonMap,
        flattened: true,
        child: None,
    },
];
const EXPECTED: &[ExpectedModel<'static>] = &[
    ExpectedModel {
        wire_name: "Root",
        rust_name: ROOT.rust_name,
        fields: FIELDS,
    },
    ExpectedModel {
        wire_name: "Child",
        rust_name: CHILD.rust_name,
        fields: &[ExpectedField {
            name: "value",
            storage: Storage::Typed,
            flattened: false,
            child: None,
        }],
    },
];
const _: () = assert!(graph_matches(ROOT, EXPECTED));
const _: () = assert!(graph_matches(ROOT, &[]), "missing target graph");
const _: () = assert!(
    graph_matches(
        ROOT,
        &[
            ExpectedModel {
                fields: &[
                    ExpectedField {
                        storage: Storage::Typed,
                        ..FIELDS[0]
                    },
                    FIELDS[1],
                    FIELDS[2],
                ],
                ..EXPECTED[0]
            },
            EXPECTED[1],
        ]
    ),
    "raw storage disagreement"
);
const _: () = assert!(
    graph_matches(
        ROOT,
        &[
            ExpectedModel {
                fields: &[
                    ExpectedField {
                        name: "renamed",
                        ..FIELDS[0]
                    },
                    FIELDS[1],
                    FIELDS[2],
                ],
                ..EXPECTED[0]
            },
            EXPECTED[1],
        ]
    ),
    "field name disagreement"
);
const _: () = assert!(
    graph_matches(
        ROOT,
        &[
            ExpectedModel {
                fields: &[
                    FIELDS[0],
                    FIELDS[1],
                    ExpectedField {
                        flattened: false,
                        ..FIELDS[2]
                    },
                ],
                ..EXPECTED[0]
            },
            EXPECTED[1],
        ]
    ),
    "catch all disagreement"
);
const _: () = assert!(
    graph_matches(
        ROOT,
        &[
            ExpectedModel {
                fields: &[
                    FIELDS[0],
                    ExpectedField {
                        child: None,
                        ..FIELDS[1]
                    },
                    FIELDS[2],
                ],
                ..EXPECTED[0]
            },
            EXPECTED[1],
        ]
    ),
    "child edge disagreement"
);
const _: () = assert!(
    graph_matches(
        ROOT,
        &[
            ExpectedModel {
                rust_name: "different declaration",
                ..EXPECTED[0]
            },
            EXPECTED[1],
        ]
    ),
    "declaration label disagreement"
);
const _: () = assert!(
    graph_matches(
        ROOT,
        &[
            ExpectedModel {
                wire_name: "Different",
                ..EXPECTED[0]
            },
            EXPECTED[1],
        ]
    ),
    "wire object disagreement"
);
const _: () = assert!(
    graph_matches(
        ROOT,
        &[
            EXPECTED[0],
            EXPECTED[1],
            ExpectedModel {
                wire_name: "Extra",
                rust_name: "extra",
                fields: &[]
            },
        ]
    ),
    "unreachable target record"
);
fn main() {}

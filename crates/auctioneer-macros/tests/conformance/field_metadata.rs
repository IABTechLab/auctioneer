use auctioneer_macros::OpenRtbFieldMetadata;
use auctioneer_openrtb::field_metadata::{FieldMetadata as _, Model, Storage};
use std::fs::read_dir;
use std::path::Path;
use trybuild::TestCases;

#[test]
fn separate_declarations_in_one_expansion_remain_distinct() {
    macro_rules! declarations {
        () => {{
            fn first() -> &'static Model {
                #[derive(OpenRtbFieldMetadata)]
                struct Slot {}
                Slot::MODEL.expect("compiled model")
            }
            fn second() -> &'static Model {
                #[derive(OpenRtbFieldMetadata)]
                struct Slot {}
                Slot::MODEL.expect("compiled model")
            }
            (first(), second())
        }};
    }
    let (first, second) = declarations!();
    assert_eq!(
        first.rust_name, second.rust_name,
        "exercise location collision"
    );
    assert!(!first.same_declaration(second));
    assert!(first.same_declaration(first));
}

#[test]
fn owned_projections_and_their_aliases_have_identical_storage() {
    trait Wire {
        type Owned;
    }
    impl Wire for &'static () {
        type Owned = String;
    }
    type Alias = <&'static () as Wire>::Owned;
    #[derive(OpenRtbFieldMetadata)]
    struct Projected {
        direct: <&'static () as Wire>::Owned,
        alias: Alias,
    }
    let fields = Projected::MODEL.expect("compiled model").fields;
    assert_eq!(fields.len(), 2);
    for field in fields {
        assert_eq!(field.storage, Storage::Typed);
        assert!(field.child.is_none());
    }
    let owned = Projected {
        direct: "direct".to_owned(),
        alias: "alias".to_owned(),
    };
    assert_eq!(
        (owned.direct.as_str(), owned.alias.as_str()),
        ("direct", "alias")
    );
}

#[test]
fn compiler_accepts_supported_models_and_refuses_unsupported_models() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (directory, expected) in [("pass", 5), ("fail", 21)] {
        let count = read_dir(root.join("tests/conformance").join(directory))
            .expect("compiler fixture directory")
            .map(|entry| entry.expect("compiler fixture"))
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
            .count();
        assert_eq!(count, expected, "compiler fixture collection {directory}");
    }
    let cases = TestCases::new();
    cases.pass("tests/conformance/pass/*.rs");
    cases.compile_fail("tests/conformance/fail/*.rs");
}

//! Executable checks against independently generated synthetic settings.

pub use auctioneer_bidder::bidder_settings;

pub mod generated {
    include!(concat!(env!("SETTINGS_BASE"), "/bidder_settings.rs"));
}

pub mod endpoints {
    include!(concat!(env!("SETTINGS_BASE"), "/bidder_endpoint.rs"));
}

use generated::{BidderSettings, fake_integer_probe, fake_string_probe};
use serde_json::{Value, json};

fn decode(value: Value) -> Result<BidderSettings, serde_json::Error> {
    serde_json::from_value(value)
}

fn explicit_implementation_and_settings_accept_empty_optional_fields() {
    let settings = decode(json!({"implementation": "fake_integer_probe", "settings": {}})).unwrap();
    assert_eq!(settings.implementation_key(), "fake_integer_probe");
    let typed = settings.as_fake_integer_probe().unwrap();
    assert_eq!(typed.primary_number, None);
    assert_eq!(typed.secondary_number, None);
    assert!(settings.as_fake_string_probe().is_none());
    let string_settings =
        decode(json!({"implementation": "fake_string_probe", "settings": {}})).unwrap();
    assert_eq!(string_settings.implementation_key(), "fake_string_probe");
    assert_eq!(string_settings.as_fake_string_probe().unwrap().token, None);
    assert!(string_settings.as_fake_integer_probe().is_none());
}

fn refuses_ambiguous_missing_or_public_alias_discriminators_and_extra_fields() {
    for value in [
        json!({}),
        json!({"settings": {}}),
        json!({"implementation": "fake_integer_probe", "values": {}}),
        json!({"implementation": "fake_integer_probe", "values": {"primary_number": 5_i64}}),
        json!({"primary_number": 5_i64}),
        json!({"implementation": "fake_integer_probe"}),
        json!({"implementation": "fake_unknown", "settings": {}}),
        json!({"implementation": "V0", "settings": {}}),
        json!({"implementation": "fake_fixture_alias", "settings": {}}),
        json!({"implementation": "fake_integer_probe", "settings": {}, "typo": true}),
        json!({"implementation": "fake_integer_probe", "settings": {"primry_number": 5_i64}}),
        json!({"implementation": "fake_integer_probe", "settings": {"token": "x"}}),
        json!({"implementation": "fake_string_probe", "settings": {"primary_number": 5_i64}}),
        json!({"implementation": "fake_string_probe", "settings": {"token_typo": "x"}}),
        json!({"implementation": "fake_integer_probe", "settings": null}),
        json!({"implementation": "fake_integer_probe", "settings": []}),
        json!({"implementation": 1_i64, "settings": {}}),
        json!(["fake_integer_probe", {}]),
        json!({"implementation": "fake_integer_probe", "settings": [5_i64, 6_i64]}),
    ] {
        assert!(decode(value).unwrap_err().is_data());
    }
}

fn optional_string_has_no_coercion_or_invented_source_regex() {
    for value in [json!(0_i64), json!(true), json!([]), json!({})] {
        assert!(
            decode(json!({
                "implementation": "fake_string_probe", "settings": {"token": value}
            }))
            .unwrap_err()
            .is_data()
        );
    }
    for value in ["", "host-default", "UPPER_09", "not/path-validation"] {
        let settings = decode(json!({
            "implementation": "fake_string_probe", "settings": {"token": value}
        }))
        .unwrap();
        assert_eq!(
            settings.as_fake_string_probe().unwrap().token.as_deref(),
            Some(value)
        );
    }
    let settings = decode(json!({
        "implementation": "fake_string_probe", "settings": {"token": null}
    }))
    .unwrap();
    assert_eq!(settings.as_fake_string_probe().unwrap().token, None);
}

fn optional_signed_i64_has_full_domain_and_strict_types() {
    for field in ["primary_number", "secondary_number"] {
        for value in [i64::MIN, -1, 0, 5, 6, i64::MAX] {
            let settings = decode(json!({
                "implementation": "fake_integer_probe", "settings": { (field): value }
            }))
            .unwrap();
            let typed = settings.as_fake_integer_probe().unwrap();
            let view = fake_integer_probe::resolve(Some(typed), None);
            assert_eq!(
                if field == "primary_number" {
                    view.primary_number()
                } else {
                    view.secondary_number()
                },
                Some(value)
            );
        }
        for value in [
            json!("5"),
            json!(true),
            json!(1.5_f64),
            json!(5.0_f64),
            json!([]),
            json!({}),
            json!(u64::MAX),
        ] {
            assert!(
                decode(json!({
                    "implementation": "fake_integer_probe", "settings": { (field): value }
                }))
                .unwrap_err()
                .is_data()
            );
        }
    }
}

fn borrowed_refs_resolve_each_requested_field_before_base_without_normalization() {
    let base = fake_integer_probe::Settings {
        primary_number: Some(41),
        secondary_number: Some(42),
    };
    let requested = fake_integer_probe::Settings {
        primary_number: Some(0),
        secondary_number: None,
    };
    let view = fake_integer_probe::resolve(Some(&requested), Some(&base));
    assert_eq!(view.primary_number(), Some(0));
    assert_eq!(view.secondary_number(), Some(42));
    let base_view = fake_integer_probe::resolve(None, Some(&base));
    assert_eq!(base_view.primary_number(), Some(41));
    assert_eq!(
        fake_integer_probe::resolve(None, None).primary_number(),
        None
    );
}

fn string_views_preserve_empty_override_and_borrow_inherited_storage() {
    let base = fake_string_probe::Settings {
        token: Some("base-host-value".to_owned()),
    };
    let requested = fake_string_probe::Settings {
        token: Some(String::new()),
    };
    let view = fake_string_probe::resolve(Some(&requested), Some(&base));
    assert_eq!(view.token(), Some(""));
    let absent_override = fake_string_probe::Settings::default();
    let inherited_view = fake_string_probe::resolve(Some(&absent_override), Some(&base));
    let borrowed = inherited_view.token().unwrap();
    assert_eq!(borrowed, "base-host-value");
    assert_eq!(borrowed.as_ptr(), base.token.as_deref().unwrap().as_ptr());
    assert_eq!(fake_string_probe::resolve(None, None).token(), None);
}

fn debug_redacts_payloads_and_borrowed_views() {
    let settings = decode(json!({
        "implementation": "fake_string_probe", "settings": {"token": "private-marker"}
    }))
    .unwrap();
    let typed = settings.as_fake_string_probe().unwrap();
    for debug in [
        format!("{settings:?}"),
        format!("{settings:#?}"),
        format!("{typed:?}"),
        format!("{:?}", fake_string_probe::resolve(Some(typed), None)),
    ] {
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("private-marker"));
    }
    let integer_settings = fake_integer_probe::Settings {
        primary_number: Some(-987_654_321),
        secondary_number: Some(123_456_789),
    };
    for debug in [
        format!("{integer_settings:?}"),
        format!(
            "{:?}",
            fake_integer_probe::resolve(Some(&integer_settings), None)
        ),
    ] {
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("987654321"));
        assert!(!debug.contains("123456789"));
    }
}

fn serialization_retains_explicit_discriminator_and_settings() {
    let original = json!({"implementation": "fake_string_probe", "settings": {"token": "host"}});
    let settings = decode(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(&settings).unwrap(), original);
}

fn standalone_payloads_also_require_objects_and_optional_null_stays_absent() {
    assert!(
        serde_json::from_value::<fake_integer_probe::Settings>(json!([]))
            .unwrap_err()
            .is_data()
    );
    assert!(
        serde_json::from_value::<fake_string_probe::Settings>(json!([]))
            .unwrap_err()
            .is_data()
    );
    let settings = decode(json!({
        "implementation": "fake_integer_probe", "settings": {"primary_number": null, "secondary_number": null}
    }))
    .unwrap();
    let typed = settings.as_fake_integer_probe().unwrap();
    assert_eq!(typed.primary_number, None);
    assert_eq!(typed.secondary_number, None);
}

fn main() {
    generated_endpoint_bindings_preserve_literals_and_borrow_storage();
    explicit_implementation_and_settings_accept_empty_optional_fields();
    refuses_ambiguous_missing_or_public_alias_discriminators_and_extra_fields();
    optional_string_has_no_coercion_or_invented_source_regex();
    optional_signed_i64_has_full_domain_and_strict_types();
    borrowed_refs_resolve_each_requested_field_before_base_without_normalization();
    string_views_preserve_empty_override_and_borrow_inherited_storage();
    debug_redacts_payloads_and_borrowed_views();
    serialization_retains_explicit_discriminator_and_settings();
    standalone_payloads_also_require_objects_and_optional_null_stays_absent();
    println!("settings consumer passed");
}

fn generated_endpoint_bindings_preserve_literals_and_borrow_storage() {
    use endpoints::fake_string_probe::Values;

    const PAIRS: [(&str, &str); 2] = Values {
        a_route: "constant route",
        z_token: "constant token",
    }
    .as_pairs();

    assert_eq!(
        PAIRS,
        [
            ("ZuluRoute", "constant route"),
            ("AlphaToken", "constant token")
        ]
    );
    let route = String::from("caller-owned route");
    let token = String::from("caller-owned token");
    let pairs = Values {
        a_route: &route,
        z_token: &token,
    }
    .as_pairs();
    assert_eq!(
        pairs,
        [
            ("ZuluRoute", "caller-owned route"),
            ("AlphaToken", "caller-owned token")
        ]
    );
    assert!(std::ptr::eq(pairs[0].1.as_ptr(), route.as_ptr()));
    assert!(std::ptr::eq(pairs[1].1.as_ptr(), token.as_ptr()));
    assert_eq!(size_of::<Values<'_>>(), 2 * size_of::<&str>());
}

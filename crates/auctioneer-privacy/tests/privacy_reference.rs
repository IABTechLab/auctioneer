//! Recorded outputs from the pinned Prebid reference, not hand-built expectations.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use auctioneer_privacy::Usp;
    use serde_json::Value;

    fn reference() -> Value {
        let value: Value =
            serde_json::from_str(include_str!("data/privacy/reference.json")).unwrap();
        assert_eq!(
            value["prebid_revision"],
            "d7f8515b86258688304b0d9b6668c6a0e258bc9e"
        );
        assert_eq!(value["go_gpp"], "v0.2.0");
        assert_eq!(value["go"], "go1.23.12");
        assert_eq!(value["unicode"], "15.0.0");
        value
    }

    #[test]
    fn frozen_reference_groups_are_complete_with_unique_scrubs_and_ordered_folds() {
        let fixture = reference();
        for (group, count) in [
            ("activities", 864),
            ("fold_orbits", 1454),
            ("nosale", 18),
            ("scrubs", 9),
            ("selection", 36),
            ("usp", 37),
        ] {
            assert_eq!(fixture[group].as_array().unwrap().len(), count, "{group}");
        }
        let mut names = BTreeSet::new();
        for row in fixture["scrubs"].as_array().unwrap() {
            let name = row["name"].as_str().unwrap();
            assert!(!name.is_empty() && names.insert(name), "{name}");
        }
        let folds: Vec<[u32; 2]> = serde_json::from_value(fixture["fold_orbits"].clone()).unwrap();
        for &[scalar, minimum] in &folds {
            assert!(char::from_u32(scalar).is_some());
            assert!(char::from_u32(minimum).is_some());
            assert!(minimum < scalar);
        }
        assert!(
            folds
                .iter()
                .zip(folds.iter().skip(1))
                .all(|(left, right)| left[0] < right[0])
        );
    }

    #[test]
    fn coppa_reference_is_labeled_as_an_intentional_user_fpd_difference() {
        let fixture = reference();
        let row = fixture["scrubs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == "coppa")
            .unwrap();
        assert_eq!(row["kind"], "intentional-coppa-user-fpd-difference");
        assert_eq!(row["input"]["user"]["keywords"], "interest");
        assert_eq!(
            row["reference_output"]["user"]["keywords"], "interest",
            "do not overwrite the frozen reference output with Auctioneer's stricter policy"
        );
    }

    #[test]
    fn usp_matches_pinned_prebid_validation_and_sale_opt_out() {
        let fixture = reference();
        let rows = fixture["usp"].as_array().unwrap();
        assert_eq!(rows.len(), 37);
        for row in rows {
            let raw = row["value"].as_str().unwrap();
            let parsed = Usp::parse(Some(raw));
            assert_eq!(parsed.is_ok(), row["valid"].as_bool().unwrap(), "{raw:?}");
            assert_eq!(
                parsed.is_err(),
                row["parse_error"].as_bool().unwrap(),
                "{raw:?}"
            );
            let selected = parsed.ok().flatten();
            assert_eq!(
                selected.is_some(),
                row["specified"].as_bool().unwrap(),
                "{raw:?}"
            );
            assert_eq!(
                selected.is_some_and(|usp| usp.sale_opt_out()),
                row["sale_opt_out"].as_bool().unwrap(),
                "{raw:?}"
            );
        }
    }
}

//! Exhaustive qualification against the pinned Go 1.23.12 / Unicode 15 fixture.

#[cfg(test)]
mod tests {
    use auctioneer_config_data::activity::simple_fold_eq;
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::num::NonZeroU32;

    #[derive(Deserialize)]
    struct Reference {
        go: String,
        unicode: String,
        fold_orbits: Vec<(u32, u32)>,
    }

    fn folded(c: char) -> u32 {
        unicode_case_mapping::case_folded(c).map_or(u32::from(c), NonZeroU32::get)
    }

    #[test]
    fn every_unicode_scalar_has_exactly_the_go_simple_fold_equivalence_class() {
        let reference: Reference = serde_json::from_str(include_str!(
            "../../auctioneer-privacy/tests/data/privacy/reference.json"
        ))
        .expect("pinned Go reference");
        assert_eq!(reference.go, "go1.23.12");
        assert_eq!(reference.unicode, "15.0.0");
        assert_eq!(unicode_case_mapping::UNICODE_VERSION, (15, 0, 0));
        let canonical: BTreeMap<u32, u32> = reference.fold_orbits.into_iter().collect();
        let mut actual_classes = BTreeMap::new();
        let mut checked = 0_u32;
        for scalar in 0..=0x0010_ffff {
            let Some(c) = char::from_u32(scalar) else {
                continue;
            };
            let expected = *canonical.get(&scalar).unwrap_or(&scalar);
            let actual = folded(c);
            assert_eq!(
                *canonical.get(&actual).unwrap_or(&actual),
                expected,
                "different orbit for {scalar:x}"
            );
            assert_eq!(
                folded(char::from_u32(expected).expect("Go orbit scalar")),
                actual,
                "split orbit for {scalar:x}"
            );
            if let Some(previous) = actual_classes.insert(actual, expected) {
                assert_eq!(
                    previous, expected,
                    "distinct Go orbits merged for {scalar:x}"
                );
            }
            checked = checked.saturating_add(1);
        }
        assert_eq!(checked, 1_112_064);
    }

    #[test]
    fn equality_uses_simple_not_ascii_full_locale_or_normalized_folding() {
        for left in ["\u{212a}", "K", "k"] {
            for right in ["\u{212a}", "K", "k"] {
                assert!(simple_fold_eq(left, right));
            }
        }
        assert!(!simple_fold_eq("\u{df}", "ss"));
        assert!(simple_fold_eq("\u{1e9e}", "\u{df}"));
        assert!(!simple_fold_eq("\u{130}", "i"));
        assert!(!simple_fold_eq("\u{131}", "I"));
        assert!(!simple_fold_eq("\u{e9}", "e\u{301}"));
        assert!(simple_fold_eq("Future\u{212a}", "futurek"));
        assert!(simple_fold_eq("", ""));
        assert!(!simple_fold_eq("k", "kk"));
    }
}

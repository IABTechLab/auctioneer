#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use auctioneer_privacy::{PrivacyError, ccpa::NoSale};
    use serde_json::Value;

    fn parse(names: &[&str]) -> Result<NoSale, PrivacyError> {
        let namespace = BTreeSet::from([
            "fake_bidder",
            "fake_alias",
            "fake_disabled_alias",
            "fake_K",
            "fake_\u{212a}",
        ]);
        let owned_names = names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>();
        NoSale::validate(&owned_names, |name| namespace.contains(name))
    }

    #[test]
    fn empty_and_exact_names_are_independent_exemptions() {
        let empty = parse(&[]).unwrap();
        assert!(!empty.exempts("fake_bidder"));
        let exemptions = parse(&["fake_alias", "fake_disabled_alias", "fake_alias"]).unwrap();
        assert!(exemptions.exempts("fake_alias"));
        assert!(exemptions.exempts("fake_disabled_alias"));
        assert!(!exemptions.exempts("fake_bidder"));
        assert!(!exemptions.exempts("fake_ALIAS"));
        assert_eq!(
            exemptions,
            parse(&["fake_disabled_alias", "fake_alias"]).unwrap()
        );
    }

    #[test]
    fn wildcard_must_be_the_only_input_entry() {
        assert!(parse(&["*"]).unwrap().exempts("fake_any_bidder"));
        for names in [
            vec!["*", "fake_alias"],
            vec!["fake_alias", "*"],
            vec!["*", "*"],
        ] {
            assert_eq!(parse(&names), Err(PrivacyError::InvalidNoSale));
        }
    }

    #[test]
    fn names_are_case_sensitive_without_simple_fold_or_core_fallback() {
        for name in ["fake_unknown", "fake_BIDDER", "", "fake_k"] {
            assert_eq!(parse(&[name]), Err(PrivacyError::InvalidNoSale));
        }
        let exemptions = parse(&["fake_K"]).unwrap();
        assert!(exemptions.exempts("fake_K"));
        assert!(!exemptions.exempts("fake_\u{212a}"));
        assert!(!exemptions.exempts("fake_k"));
    }

    #[test]
    fn no_sale_has_no_dependency_on_consent_validity_or_policy_switches() {
        let fixture: Value =
            serde_json::from_str(include_str!("data/privacy/reference.json")).unwrap();
        let rows = fixture["nosale"].as_array().unwrap();
        assert_eq!(rows.len(), 18);
        for row in rows {
            let names = row["names"].as_array().map_or_else(Vec::new, |entries| {
                entries
                    .iter()
                    .map(|entry| entry.as_str().unwrap())
                    .collect()
            });
            let owned_names = names
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>();
            let result = NoSale::validate(&owned_names, |name| {
                ["bidder", "alias", "disabled-alias", "K", "\u{212a}"].contains(&name)
            });
            if row["consent"] == "1YYN" {
                assert_eq!(result.is_err(), row["error_kind"] == "error", "{row}");
                if let Ok(exemptions) = result {
                    assert_eq!(!exemptions.exempts("bidder"), row["bidder"]);
                    assert_eq!(!exemptions.exempts("alias"), row["alias"]);
                }
            } else {
                // Auctioneer validates names even where the reference stops at bad USP.
                let invalid = names
                    .iter()
                    .any(|name| matches!(*name, "BIDDER" | "unknown"))
                    || (names.len() > 1 && names.contains(&"*"));
                assert_eq!(result.is_err(), invalid, "{row}");
            }
        }
    }

    #[test]
    fn errors_do_not_disclose_an_invalid_name() {
        let error = parse(&["fake_private_unknown_alias"]).unwrap_err();
        assert_eq!(error, PrivacyError::InvalidNoSale);
        assert!(!format!("{error:?} {error}").contains("fake_private_unknown_alias"));
    }
}

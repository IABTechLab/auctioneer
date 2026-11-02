//! Frozen wire inputs; handler response comparisons live in their unit owner.

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::collections::BTreeSet;
    use time::format_description::well_known::Rfc3339;

    #[test]
    fn captured_wire_inputs_retain_their_reference_time_and_unique_cases() {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../auctioneer-bidder/tests/conformance/fixtures/usersync/wire.json"
        ))
        .unwrap();
        time::OffsetDateTime::parse(corpus["reference"]["now"].as_str().unwrap(), &Rfc3339)
            .unwrap();
        let cases = corpus["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 42);
        let identities: BTreeSet<_> = cases
            .iter()
            .map(|case| case["id"].as_str().unwrap())
            .collect();
        assert_eq!(identities.len(), cases.len());
    }
}

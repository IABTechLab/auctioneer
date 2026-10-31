#[cfg(test)]
mod tests {
    use auctioneer_runtime::serialized_json_bytes;
    use serde_json::json;

    #[test]
    fn counting_serialization_matches_actual_escaped_output() {
        for value in [
            json!(null),
            json!({"a":"\n\"\\\t", "b":[1_i32, false, null]}),
            json!({"unicode":"\u{212a}\u{1f600}"}),
            json!(vec![json!({"":[]}); 1000]),
        ] {
            let expected = u64::try_from(serde_json::to_vec(&value).unwrap().len()).unwrap();
            assert_eq!(serialized_json_bytes(&value).unwrap(), expected);
        }
    }
}

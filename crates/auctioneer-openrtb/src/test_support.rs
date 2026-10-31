//! Shared test helpers. Compiled only for tests.
#![cfg(test)]

use serde::{Serialize, de::DeserializeOwned};

/// Asserts that the type round-trips losslessly through `serde_json`.
///
/// Takes the fixture as a `serde_json::Value` — built with
/// `serde_json::json!`, so a malformed fixture is a compile error rather
/// than a runtime parse failure. The comparison is `Value` against
/// `Value`, so whitespace and key order never trip the assertion.
///
/// # Panics
/// Panics with a descriptive message if parse, serialize, or comparison fails.
pub fn round_trip<T>(json: &serde_json::Value) -> T
where
    T: DeserializeOwned + Serialize,
{
    let parsed: T = serde_json::from_value(json.clone())
        .unwrap_or_else(|err| panic!("parse failed: {err}\nJSON: {json}"));
    let after =
        serde_json::to_value(&parsed).unwrap_or_else(|err| panic!("serialize failed: {err}"));
    assert_eq!(*json, after, "round-trip not lossless");
    parsed
}

//! Types generated from adjacent bidder-owned settings declarations.

use auctioneer_runtime::{json_preflight::preflight, serialized_json_bytes};

include!(concat!(env!("OUT_DIR"), "/bidder_settings.rs"));

/// Predeclared typed-settings limits, independently bounded within a config blob.
pub const MAX_SETTINGS_BYTES: u64 = 16 * 1_024;
pub const MAX_SETTINGS_NODES: u64 = 1_024;
pub const MAX_SETTINGS_DEPTH: u32 = 16;

/// Maximum UTF-8 byte length of an admitted bidder credential reference.
pub const MAX_CREDENTIAL_REFERENCE_BYTES: usize = 128;

/// Deserialize a nonempty, bounded credential reference without ASCII controls.
/// Explicit null remains `None`; generated field defaults also preserve omission.
/// References are not trimmed or otherwise normalized.
///
/// # Errors
/// Returns fixed text for invalid references or deserializer errors.
pub fn deserialize_optional_secret_reference<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: ::serde::Deserializer<'de>,
{
    use ::serde::{Deserialize as _, de::Error as _};

    const ERROR: &str = "invalid credential reference";
    let reference =
        Option::<String>::deserialize(deserializer).map_err(|_error| D::Error::custom(ERROR))?;
    if reference.as_deref().is_some_and(|value| {
        value.is_empty()
            || value.len() > MAX_CREDENTIAL_REFERENCE_BYTES
            || value.bytes().any(|byte| byte.is_ascii_control())
    }) {
        return Err(D::Error::custom(ERROR));
    }
    Ok(reference)
}

/// Check the typed representation without allocating another tree or buffer.
/// The outer config extractor separately funds and caps the original bytes.
///
/// # Errors
/// Returns fixed text on byte, structural or serialization refusal.
pub fn ensure_limits(settings: &BidderSettings) -> Result<(), &'static str> {
    preflight(settings, MAX_SETTINGS_NODES, MAX_SETTINGS_DEPTH)
        .map_err(|_error| "bidder settings exceed structural limits")?;
    let bytes = serialized_json_bytes(settings)
        .map_err(|_error| "bidder settings serialization refused")?;
    if bytes > MAX_SETTINGS_BYTES {
        return Err("bidder settings exceed byte limit");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MAX_CREDENTIAL_REFERENCE_BYTES, deserialize_optional_secret_reference};
    use serde::de::value::{Error, StrDeserializer};
    use serde_json::json;
    use std::iter::once;

    #[derive(Debug, ::serde::Deserialize)]
    struct Reference {
        #[serde(default, deserialize_with = "deserialize_optional_secret_reference")]
        value: Option<String>,
    }

    #[test]
    fn secret_reference_accepts_utf8_byte_boundary_without_normalization() {
        assert_eq!(MAX_CREDENTIAL_REFERENCE_BYTES, 128);
        for value in [
            "a".to_owned(),
            "x".repeat(128),
            "\u{00e9}".repeat(64),
            " ".to_owned(),
            " fake:/reference?version=2 ".to_owned(),
            "\u{0085}".to_owned(),
        ] {
            let decoded: Reference = serde_json::from_value(json!({"value": value})).unwrap();
            assert_eq!(decoded.value.as_deref(), Some(value.as_str()));
        }
    }

    #[test]
    fn secret_reference_preserves_missing_and_null_as_none() {
        for value in [json!({}), json!({"value": null})] {
            let decoded: Reference = serde_json::from_value(value).unwrap();
            assert_eq!(decoded.value, None);
        }
    }

    #[test]
    fn secret_reference_refuses_empty_oversized_controls_and_wrong_types_with_fixed_error() {
        const CANARY: &str = "private-reference-canary";
        let mut invalid = vec![
            json!(""),
            json!("x".repeat(129)),
            json!(format!("{}x", "\u{00e9}".repeat(64))),
            json!(false),
            json!(7_i32),
            json!(1.5_f64),
            json!([CANARY]),
            json!({"private-reference-canary": CANARY}),
        ];
        for control in (0_u8..=31).chain(once(127)) {
            invalid.push(json!(format!("{CANARY}{}suffix", char::from(control))));
        }
        for value in invalid {
            let error = serde_json::from_value::<Reference>(json!({"value": value})).unwrap_err();
            assert!(error.is_data());
            assert_eq!(error.to_string(), "invalid credential reference");
        }
    }

    #[test]
    fn secret_reference_never_forwards_a_raw_deserializer_error() {
        let deserializer = StrDeserializer::<Error>::new("private-reference-canary");
        let error = deserialize_optional_secret_reference(deserializer).unwrap_err();
        assert_eq!(error.to_string(), "invalid credential reference");
    }
}

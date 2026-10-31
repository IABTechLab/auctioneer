//! Closed empty-object wire representation for an internally tagged unit payload.

use serde::de::Error as _;
use serde::ser::SerializeMap as _;
use serde::{Deserialize as _, Deserializer, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn serialize<T: ?Sized, S: Serializer>(
    _unit: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_map(Some(0))?.end()
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    let fields = BTreeMap::<String, Value>::deserialize(deserializer)?;
    if !fields.is_empty() {
        return Err(D::Error::custom("unexpected field in empty object"));
    }
    Ok(())
}

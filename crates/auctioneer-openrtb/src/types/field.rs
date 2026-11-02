//! Optional typed context that preserves JSON presence without retaining input.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum JsonField<T> {
    #[default]
    Missing,
    Null,
    Value(T),
}

impl<T> JsonField<T> {
    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }

    #[must_use]
    pub fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null => None,
        }
    }

    pub fn as_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null => None,
        }
    }
}

impl<T: Serialize> Serialize for JsonField<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Missing | Self::Null => serializer.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for JsonField<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(|value| value.map_or(Self::Null, Self::Value))
    }

    fn deserialize_in_place<D: Deserializer<'de>>(
        deserializer: D,
        place: &mut Self,
    ) -> Result<(), D::Error> {
        *place = Self::deserialize(deserializer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::JsonField;
    use serde::{Deserialize, Serialize};

    #[derive(Default, Deserialize, Serialize)]
    struct Context {
        #[serde(default, skip_serializing_if = "JsonField::is_missing")]
        value: JsonField<i64>,
    }

    #[test]
    fn missing_null_and_typed_value_are_distinct() {
        for json in [
            serde_json::json!({}),
            serde_json::json!({"value": null}),
            serde_json::json!({"value": 2_i64}),
        ] {
            let mut context: Context = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(serde_json::to_value(&context).unwrap(), json);
            assert_eq!(context.value.is_missing(), json.get("value").is_none());
            assert_eq!(context.value.as_ref().copied(), json["value"].as_i64());
            if let Some(value) = context.value.as_mut() {
                *value = 3;
                assert_eq!(serde_json::to_value(&context).unwrap()["value"], 3_i64);
            }
        }
        assert!(serde_json::from_value::<Context>(serde_json::json!({"value": "2"})).is_err());
    }

    #[test]
    fn in_place_deserialization_preserves_null_and_typed_values() {
        let mut field = JsonField::Missing;
        for json in [serde_json::json!(2_i64), serde_json::json!(null)] {
            JsonField::<i64>::deserialize_in_place(json.clone(), &mut field).unwrap();
            assert_eq!(serde_json::to_value(&field).unwrap(), json);
        }
    }
}

//! Require object input while retaining Serde's typed field decoding.

use core::{fmt, marker::PhantomData};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor, value::MapAccessDeserializer},
};

pub(crate) fn deserialize_object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserializer.deserialize_map(ObjectVisitor(PhantomData))
}

struct ObjectVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a configuration object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(ObjectMapDeserializer(map))
    }
}

// MapAccessDeserializer forwards option decoding to visit_map, whereas an
// Option<T> needs visit_some. Preserve that distinction without allowing null
// or an absent required section to become None.
struct ObjectMapDeserializer<A>(A);

impl<'de, A: MapAccess<'de>> Deserializer<'de> for ObjectMapDeserializer<A> {
    type Error = A::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_map(self.0)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        MapAccessDeserializer::new(self.0).deserialize_enum(name, variants, visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf
        unit unit_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}

#[cfg(test)]
mod tests {
    use super::deserialize_object;
    use serde::Deserialize;
    use serde_json::{error::Category, json};

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct FakeObject {
        value: String,
    }

    #[derive(Debug, Deserialize)]
    struct FakeContainer {
        #[serde(deserialize_with = "deserialize_object")]
        object: FakeObject,
    }

    #[derive(Debug, Deserialize)]
    struct FakeOptionalContainer {
        #[serde(deserialize_with = "deserialize_object")]
        object: Option<FakeObject>,
    }

    #[derive(Debug, Deserialize)]
    struct FakeNewtype(FakeObject);

    #[derive(Debug, Deserialize)]
    struct FakeNewtypeContainer {
        #[serde(deserialize_with = "deserialize_object")]
        object: FakeNewtype,
    }

    #[test]
    fn optional_subtree_types_require_a_present_nonnull_object() {
        let parsed: FakeOptionalContainer =
            serde_json::from_value(json!({"object": {"value": "fake_value"}})).unwrap();
        assert_eq!(
            parsed.object,
            Some(FakeObject {
                value: "fake_value".to_owned()
            })
        );
        for document in [json!({}), json!({"object": null})] {
            let error = serde_json::from_value::<FakeOptionalContainer>(document).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
        }
    }

    #[test]
    fn object_guard_preserves_typed_values_without_a_json_tree_conversion() {
        let parsed: FakeContainer =
            serde_json::from_value(json!({"object": {"value": "fake_value"}})).unwrap();
        assert_eq!(
            parsed.object,
            FakeObject {
                value: "fake_value".to_owned()
            }
        );
        let wrapped: FakeNewtypeContainer =
            serde_json::from_value(json!({"object": {"value": "fake_value"}})).unwrap();
        assert_eq!(wrapped.object.0.value, "fake_value");
    }

    #[test]
    fn object_guard_refuses_sequence_null_and_scalar_input() {
        for value in [
            json!(["fake_value"]),
            json!(null),
            json!("fake_value"),
            json!(true),
            json!(1_u64),
        ] {
            let error =
                serde_json::from_value::<FakeContainer>(json!({"object": value})).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
            assert!(error.to_string().contains("a configuration object"));
        }
    }
}

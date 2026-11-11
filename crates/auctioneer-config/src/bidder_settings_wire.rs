//! Strictly claim the bidder selector and its separate typed settings payload.

use core::{fmt, marker::PhantomData};
use serde::{
    Deserialize, Deserializer,
    de::{
        DeserializeSeed, MapAccess, Visitor, value::MapAccessDeserializer, value::StrDeserializer,
    },
};

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum Key {
    Implementation,
    Settings,
}

impl Key {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Implementation => "implementation",
            Self::Settings => "settings",
        }
    }
}

/// A flattened `Option<T>` suppresses deserialization errors. Claim exactly the
/// envelope keys as a struct instead, returning absence only for an empty map.
pub(crate) fn deserialize_optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct SettingsVisitor<T>(PhantomData<fn() -> T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for SettingsVisitor<T> {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bidder implementation and settings object")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let Some(first) = map.next_key::<Key>()? else {
                return Ok(None);
            };
            T::deserialize(MapAccessDeserializer::new(PrefixedMap {
                first: Some(first),
                map,
            }))
            .map(Some)
        }
    }
    // deserialize_struct consumes only these two keys from Serde's flattened
    // map. The outer strict bidder model rejects any unclaimed row fields.
    deserializer.deserialize_struct(
        "BidderSettings",
        &["implementation", "settings"],
        SettingsVisitor(PhantomData),
    )
}

struct PrefixedMap<A> {
    first: Option<Key>,
    map: A,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for PrefixedMap<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        if let Some(key) = self.first.take() {
            return seed
                .deserialize(StrDeserializer::<A::Error>::new(key.as_str()))
                .map(Some);
        }
        self.map.next_key_seed(seed)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        self.map.next_value_seed(seed)
    }
}

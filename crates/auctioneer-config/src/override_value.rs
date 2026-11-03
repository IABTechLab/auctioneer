//! Presence-aware overrides for policy fields that explicitly permit clearing.

use std::{borrow::Cow, collections::BTreeSet, fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{
        self, DeserializeSeed, IgnoredAny, IntoDeserializer as _, MapAccess, SeqAccess, Visitor,
        value::{BorrowedStrDeserializer, MapAccessDeserializer, SeqAccessDeserializer},
    },
    ser::{Error as SerializeError, SerializeMap as _},
};

/// An inherited setting, an explicit replacement, or an explicit clear.
///
/// Containers must use `#[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]`.
/// On the wire, a clear is exactly `{ "clear": true }`; direct null and other
/// objects containing `clear` are rejected, even if `T` would accept them.
/// Only the containing policy determines which fields may use this type.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum FieldOverride<T> {
    /// Retain the inherited setting; represented by an omitted field.
    #[default]
    Inherit,
    /// Replace the inherited setting with this value.
    /// Null, unit and nonfinite roots cannot be serialized; nulls inside
    /// compound values retain the inner type's serialization semantics.
    Value(T),
    /// Remove the inherited optional setting.
    Clear,
}

impl<T> FieldOverride<T> {
    /// Whether a containing serializer should omit this field.
    #[must_use]
    pub const fn is_inherit(&self) -> bool {
        matches!(self, Self::Inherit)
    }
}

impl<T> fmt::Debug for FieldOverride<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inherit => "Inherit",
            Self::Value(_) => "Value(<redacted>)",
            Self::Clear => "Clear",
        })
    }
}

impl<T: Serialize> Serialize for FieldOverride<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Inherit => Err(SerializeError::custom(
                "an inherited override must be omitted by its container",
            )),
            Self::Value(value) => value.serialize(NonNullSerializer(serializer)),
            Self::Clear => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("clear", &true)?;
                map.end()
            }
        }
    }
}

const NULL_OVERRIDE_VALUE_ERROR: &str = "an override value must not serialize as null";

struct NonNullValue<'a, T: ?Sized>(&'a T);

impl<T: ?Sized + Serialize> Serialize for NonNullValue<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(NonNullSerializer(serializer))
    }
}

// Some and newtypes keep the root guard without losing serializer metadata.
// Compound handles are forwarded unchanged so their nested nulls remain valid.
struct NonNullSerializer<S>(S);

impl<S: Serializer> Serializer for NonNullSerializer<S> {
    type Ok = S::Ok;
    type Error = S::Error;
    type SerializeSeq = S::SerializeSeq;
    type SerializeTuple = S::SerializeTuple;
    type SerializeTupleStruct = S::SerializeTupleStruct;
    type SerializeTupleVariant = S::SerializeTupleVariant;
    type SerializeMap = S::SerializeMap;
    type SerializeStruct = S::SerializeStruct;
    type SerializeStructVariant = S::SerializeStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_bool(v)
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i8(v)
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i16(v)
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i32(v)
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i64(v)
    }

    fn serialize_i128(self, v: i128) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_i128(v)
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u8(v)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u16(v)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u32(v)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u64(v)
    }

    fn serialize_u128(self, v: u128) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_u128(v)
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        if !v.is_finite() {
            return Err(SerializeError::custom(NULL_OVERRIDE_VALUE_ERROR));
        }
        self.0.serialize_f32(v)
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        if !v.is_finite() {
            return Err(SerializeError::custom(NULL_OVERRIDE_VALUE_ERROR));
        }
        self.0.serialize_f64(v)
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_char(v)
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_str(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_bytes(v)
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(SerializeError::custom(NULL_OVERRIDE_VALUE_ERROR))
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_some(&NonNullValue(value))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(SerializeError::custom(NULL_OVERRIDE_VALUE_ERROR))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(SerializeError::custom(NULL_OVERRIDE_VALUE_ERROR))
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_unit_variant(name, variant_index, variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0.serialize_newtype_struct(name, &NonNullValue(value))
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.0
            .serialize_newtype_variant(name, variant_index, variant, value)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        self.0.serialize_seq(len)
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.0.serialize_tuple(len)
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.0.serialize_tuple_struct(name, len)
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.0
            .serialize_tuple_variant(name, variant_index, variant, len)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.0.serialize_map(len)
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.0.serialize_struct(name, len)
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        self.0
            .serialize_struct_variant(name, variant_index, variant, len)
    }

    fn collect_str<T: ?Sized + fmt::Display>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        self.0.collect_str(value)
    }

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for FieldOverride<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OverrideVisitor(PhantomData))
    }
}

struct OverrideVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for OverrideVisitor<T> {
    type Value = FieldOverride<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a non-null setting or exactly { clear: true }")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_i128<E: de::Error>(self, v: i128) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_u128<E: de::Error>(self, v: u128) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(BorrowedStrDeserializer::new(v))).map(FieldOverride::Value)
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        T::deserialize(ConcreteValue(v.into_deserializer())).map(FieldOverride::Value)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
        T::deserialize(ConcreteValue(SeqAccessDeserializer::new(seq))).map(FieldOverride::Value)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let first = map.next_key::<OverrideKey<'de>>()?;
        if first.as_ref().is_some_and(|key| key.0 == "clear") {
            if !map.next_value::<bool>()? || map.next_key::<IgnoredAny>()?.is_some() {
                return Err(de::Error::custom(
                    "a clear override must be exactly { clear: true }",
                ));
            }
            return Ok(FieldOverride::Clear);
        }

        // Replay only the first key. Payloads stream into T; the field-local set
        // retains keys solely to reject duplicates before permissive map types can.
        let mut checked = OverrideMap {
            first,
            seen: BTreeSet::new(),
            map,
        };
        let value = T::deserialize(ConcreteValue(MapAccessDeserializer::new(&mut checked)))?;
        if checked.next_key::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(
                "an override value must consume its complete object",
            ));
        }
        Ok(FieldOverride::Value(value))
    }
}

// Serde's access deserializers forward option/newtype requests to deserialize_any.
// A concrete non-null value must instead enter Some/the newtype before replaying.
struct ConcreteValue<D>(D);

impl<'de, D: Deserializer<'de>> Deserializer<'de> for ConcreteValue<D> {
    type Error = D::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.0.deserialize_any(visitor)
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
        self.0.deserialize_enum(name, variants, visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier ignored_any
    }
}

#[derive(Deserialize)]
#[serde(transparent)]
struct OverrideKey<'a>(#[serde(borrow)] Cow<'a, str>);

struct OverrideMap<'de, A> {
    first: Option<OverrideKey<'de>>,
    seen: BTreeSet<Cow<'de, str>>,
    map: A,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for OverrideMap<'de, A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        let Some(OverrideKey(key)) = self
            .first
            .take()
            .map_or_else(|| self.map.next_key(), |key| Ok(Some(key)))?
        else {
            return Ok(None);
        };
        if key == "clear" {
            return Err(de::Error::custom(
                "a clear override must be exactly { clear: true }",
            ));
        }
        if !self.seen.insert(key.clone()) {
            return Err(de::Error::custom("duplicate override value key"));
        }
        match key {
            Cow::Borrowed(borrowed_key) => {
                seed.deserialize(BorrowedStrDeserializer::new(borrowed_key))
            }
            Cow::Owned(owned_key) => seed.deserialize(owned_key.into_deserializer()),
        }
        .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        self.map.next_value_seed(seed)
    }
}

#[cfg(test)]
mod tests {
    use super::FieldOverride;
    use serde::{Deserialize, Serialize, de::DeserializeOwned};
    use serde_json::{Value, error::Category, json};
    use std::{collections::BTreeMap, fmt::Debug};

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    #[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
    struct Setting<T> {
        #[serde(default, skip_serializing_if = "FieldOverride::is_inherit")]
        setting: FieldOverride<T>,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Cookie {
        name: String,
        value: String,
    }

    #[test]
    fn omission_inherits_without_requiring_a_default_inner_value() {
        let parsed: Setting<Cookie> = serde_json::from_value(json!({})).unwrap();
        assert_eq!(parsed.setting, FieldOverride::Inherit);
        assert!(parsed.setting.is_inherit());
        assert_eq!(FieldOverride::<Cookie>::default(), FieldOverride::Inherit);
    }

    #[test]
    fn explicit_booleans_preserve_false() {
        for value in [false, true] {
            let parsed: Setting<bool> = serde_json::from_value(json!({"setting": value})).unwrap();
            assert_eq!(parsed.setting, FieldOverride::Value(value));
            assert!(!parsed.setting.is_inherit());
            assert_eq!(
                serde_json::to_value(parsed).unwrap(),
                json!({"setting": value})
            );
        }
    }

    #[test]
    fn explicit_numbers_preserve_zero() {
        for value in [0_u32, 17_u32] {
            let parsed: Setting<u32> = serde_json::from_value(json!({"setting": value})).unwrap();
            assert_eq!(parsed.setting, FieldOverride::Value(value));
            assert_eq!(
                serde_json::to_value(parsed).unwrap(),
                json!({"setting": value})
            );
        }
    }

    #[test]
    fn explicit_strings_preserve_empty() {
        for value in ["", "example.test"] {
            let parsed: Setting<String> =
                serde_json::from_value(json!({"setting": value})).unwrap();
            assert_eq!(parsed.setting, FieldOverride::Value(value.to_owned()));
            assert_eq!(
                serde_json::to_value(parsed).unwrap(),
                json!({"setting": value})
            );
        }
    }

    #[test]
    fn explicit_lists_preserve_empty() {
        for value in [Vec::<Vec<String>>::new(), vec![vec!["fake_aaa".to_owned()]]] {
            let parsed: Setting<Vec<Vec<String>>> =
                serde_json::from_value(json!({"setting": value})).unwrap();
            assert_eq!(parsed.setting, FieldOverride::Value(value.clone()));
            assert_eq!(
                serde_json::to_value(parsed).unwrap(),
                json!({"setting": value})
            );
        }
    }

    #[test]
    fn ordinary_typed_objects_are_values_not_clear_markers() {
        let parsed: Setting<Cookie> = serde_json::from_value(json!({
            "setting": {"name": "optout", "value": "1"}
        }))
        .unwrap();
        assert_eq!(
            parsed.setting,
            FieldOverride::Value(Cookie {
                name: "optout".to_owned(),
                value: "1".to_owned()
            })
        );
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            json!({"setting": {"name": "optout", "value": "1"}})
        );
    }

    #[test]
    fn empty_maps_remain_explicit_values() {
        let parsed: Setting<BTreeMap<String, bool>> =
            serde_json::from_value(json!({"setting": {}})).unwrap();
        assert_eq!(parsed.setting, FieldOverride::Value(BTreeMap::new()));
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            json!({"setting": {}})
        );
    }

    #[test]
    fn strict_clear_object_is_independent_of_the_inner_type() {
        assert_eq!(
            serde_json::from_value::<Setting<bool>>(json!({"setting": {"clear": true}}))
                .unwrap()
                .setting,
            FieldOverride::Clear
        );
        assert_eq!(
            serde_json::from_value::<Setting<Cookie>>(json!({"setting": {"clear": true}}))
                .unwrap()
                .setting,
            FieldOverride::Clear
        );
        assert_eq!(
            serde_json::from_value::<Setting<Vec<Vec<String>>>>(
                json!({"setting": {"clear": true}})
            )
            .unwrap()
            .setting,
            FieldOverride::Clear
        );
    }

    #[test]
    fn direct_null_is_rejected_even_for_nullable_inner_types() {
        let document = json!({"setting": null});
        for error in [
            serde_json::from_value::<Setting<String>>(document.clone()).unwrap_err(),
            serde_json::from_value::<Setting<Option<String>>>(document.clone()).unwrap_err(),
            serde_json::from_value::<Setting<Value>>(document).unwrap_err(),
        ] {
            assert_eq!(error.classify(), Category::Data);
            assert_eq!(
                error.to_string(),
                "invalid type: null, expected a non-null setting or exactly { clear: true }"
            );
        }
    }

    #[test]
    fn malformed_clear_objects_cannot_fall_through_to_a_permissive_value() {
        for marker in [
            json!({"clear": false}),
            json!({"clear": null}),
            json!({"clear": "true"}),
            json!({"clear": 1_i32}),
            json!({"clear": []}),
            json!({"clear": {}}),
            json!({"clear": true, "extra": false}),
            json!({"before": false, "clear": true}),
        ] {
            let document = json!({"setting": marker});
            for error in [
                serde_json::from_value::<Setting<String>>(document.clone()).unwrap_err(),
                serde_json::from_value::<Setting<Cookie>>(document.clone()).unwrap_err(),
                serde_json::from_value::<Setting<Value>>(document).unwrap_err(),
            ] {
                assert_eq!(error.classify(), Category::Data);
            }
        }
    }

    #[test]
    fn duplicate_clear_keys_are_rejected() {
        // Raw JSON retains duplicate keys that json! would collapse.
        for wire in [
            r#"{"setting":{"clear":true,"clear":true}}"#,
            r#"{"setting":{"clear":false,"clear":true}}"#,
            r#"{"setting":{"clear":true,"\u0063lear":true}}"#,
        ] {
            let error = serde_json::from_str::<Setting<Value>>(wire).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
            assert!(
                error
                    .to_string()
                    .starts_with("a clear override must be exactly { clear: true }")
            );
        }
    }

    #[test]
    fn ordinary_wrong_types_are_rejected_by_the_inner_type() {
        for value in [json!(false), json!(0_i32), json!([]), json!({})] {
            let error =
                serde_json::from_value::<Setting<String>>(json!({"setting": value})).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
        }
        for value in [json!("false"), json!(0_i32), json!([]), json!({})] {
            let error =
                serde_json::from_value::<Setting<bool>>(json!({"setting": value})).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
        }
        for value in [json!(false), json!("0"), json!(-1_i32), json!(1.5_f64)] {
            let error =
                serde_json::from_value::<Setting<u32>>(json!({"setting": value})).unwrap_err();
            assert_eq!(error.classify(), Category::Data);
        }
    }

    #[test]
    fn toml_omission_value_and_clear_remain_distinct() {
        let inherited: Setting<String> = toml::from_str("").unwrap();
        assert_eq!(inherited.setting, FieldOverride::Inherit);
        let empty: Setting<String> = toml::from_str("setting = \"\"\n").unwrap();
        assert_eq!(empty.setting, FieldOverride::Value(String::new()));
        let clear: Setting<String> = toml::from_str("setting = { clear = true }\n").unwrap();
        assert_eq!(clear.setting, FieldOverride::Clear);
        assert_eq!(
            toml::from_str::<Setting<String>>(&toml::to_string(&clear).unwrap()).unwrap(),
            clear
        );
    }

    #[test]
    fn toml_clear_marker_is_strict() {
        for (document, expected) in [
            (
                "setting = { clear = false }\n",
                "a clear override must be exactly { clear: true }",
            ),
            (
                "setting = { clear = \"true\" }\n",
                "invalid type: string \"true\", expected a boolean",
            ),
            (
                "setting = { clear = 1 }\n",
                "invalid type: integer `1`, expected a boolean",
            ),
            (
                "setting = { clear = true, extra = false }\n",
                "a clear override must be exactly { clear: true }",
            ),
            (
                "setting = { before = false, clear = true }\n",
                "a clear override must be exactly { clear: true }",
            ),
        ] {
            let error = toml::from_str::<Setting<Value>>(document).unwrap_err();
            assert_eq!(error.message(), expected);
        }
    }

    #[test]
    fn inherited_fields_are_omitted_and_standalone_inherit_is_not_null() {
        let inherited = Setting::<String> {
            setting: FieldOverride::Inherit,
        };
        assert_eq!(serde_json::to_value(&inherited).unwrap(), json!({}));
        assert_eq!(toml::to_string(&inherited).unwrap(), "");
        let error = serde_json::to_value(FieldOverride::<String>::Inherit).unwrap_err();
        assert_eq!(error.classify(), Category::Data);
        assert_eq!(
            error.to_string(),
            "an inherited override must be omitted by its container"
        );
    }

    #[test]
    fn clear_serializes_as_the_explicit_object_and_round_trips() {
        let clear = Setting::<String> {
            setting: FieldOverride::Clear,
        };
        assert!(!clear.setting.is_inherit());
        let document = serde_json::to_value(&clear).unwrap();
        assert_eq!(document, json!({"setting": {"clear": true}}));
        assert_eq!(
            serde_json::from_value::<Setting<String>>(document).unwrap(),
            clear
        );
    }

    #[test]
    fn programmatic_values_retain_the_inner_serialization_semantics() {
        let error = serde_json::to_value(FieldOverride::Value(None::<String>)).unwrap_err();
        assert_eq!(error.classify(), Category::Data);
        assert_eq!(
            error.to_string(),
            "an override value must not serialize as null"
        );
        assert_eq!(
            serde_json::to_value(FieldOverride::Value(json!({"clear": false}))).unwrap(),
            json!({"clear": false})
        );
        assert!(FieldOverride::<String>::Inherit.is_inherit());
        assert!(!FieldOverride::Value(false).is_inherit());
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    struct Newtype<T>(T);

    #[derive(Debug, Deserialize, Serialize, PartialEq)]
    #[serde(transparent)]
    struct Transparent<T>(T);

    fn assert_null_serialization_rejected<T: Serialize>(value: T) {
        let setting = FieldOverride::Value(value);
        for error in [
            serde_json::to_value(&setting).unwrap_err(),
            serde_json::to_string(&setting).unwrap_err(),
        ] {
            assert_eq!(error.classify(), Category::Data);
            assert_eq!(
                error.to_string(),
                "an override value must not serialize as null"
            );
        }
    }

    fn assert_json_round_trip<T>(value: T, expected: &Value)
    where
        T: Serialize + DeserializeOwned + Debug + PartialEq,
    {
        let setting = FieldOverride::Value(value);
        let serialized = serde_json::to_value(&setting).unwrap();
        assert_eq!(&serialized, expected);
        assert_eq!(
            serde_json::from_value::<FieldOverride<T>>(serialized).unwrap(),
            setting
        );
        let wire = serde_json::to_string(&setting).unwrap();
        assert_eq!(
            serde_json::from_str::<FieldOverride<T>>(&wire).unwrap(),
            setting
        );
    }

    #[test]
    fn programmatic_null_roots_are_rejected_without_echoing_values() {
        #[derive(Serialize)]
        struct PrivateUnit;

        assert_null_serialization_rejected(None::<String>);
        assert_null_serialization_rejected(Some(None::<String>));
        assert_null_serialization_rejected(());
        assert_null_serialization_rejected(PrivateUnit);
        assert_null_serialization_rejected(json!(null));
        assert_null_serialization_rejected(Some(json!(null)));
    }

    #[test]
    fn programmatic_newtype_null_roots_are_rejected() {
        assert_null_serialization_rejected(Newtype(None::<String>));
        assert_null_serialization_rejected(Newtype(()));
        assert_null_serialization_rejected(Transparent(None::<String>));
        assert_null_serialization_rejected(Transparent(()));
        assert_null_serialization_rejected(Some(Newtype(Some(Transparent(None::<String>)))));
    }

    #[test]
    fn programmatic_nonfinite_float_roots_are_rejected() {
        // JSON serializers turn these into null, even without serialize_none/unit.
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_null_serialization_rejected(value);
            assert_null_serialization_rejected(Some(Newtype(value)));
        }
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_null_serialization_rejected(value);
            assert_null_serialization_rejected(Some(Transparent(value)));
        }
    }

    #[test]
    fn programmatic_optional_values_round_trip_without_losing_false_or_empty() {
        for value in [false, true] {
            assert_json_round_trip(Some(value), &json!(value));
        }
        for value in ["", "configured"] {
            assert_json_round_trip(Some(value.to_owned()), &json!(value));
        }
        assert_json_round_trip(Some(Vec::<u32>::new()), &json!([]));
        assert_json_round_trip(Some(BTreeMap::<String, bool>::new()), &json!({}));
        assert_json_round_trip(Some(Some(false)), &json!(false));
        assert_json_round_trip(Newtype(Some(false)), &json!(false));
        assert_json_round_trip(Transparent(Some(String::new())), &json!(""));
        for value in [0.0_f32, -1.5_f32, 1.25_f32] {
            assert_json_round_trip(value, &json!(value));
        }
        for value in [0.0_f64, -1.5_f64, 1.25_f64] {
            assert_json_round_trip(Some(Newtype(value)), &json!(value));
        }
    }

    #[test]
    fn programmatic_compound_values_keep_typed_nested_nulls() {
        #[derive(Debug, Deserialize, Serialize, PartialEq)]
        struct Nested {
            optional: Option<String>,
            unit: (),
            wrapped: Newtype<Option<bool>>,
        }

        #[derive(Debug, Deserialize, Serialize, PartialEq)]
        struct Pair(Option<String>, ());

        #[derive(Debug, Deserialize, Serialize, PartialEq)]
        enum Compound {
            Newtype(Option<String>),
            Tuple(Option<String>, ()),
            Struct { optional: Option<String>, unit: () },
            Unit,
        }

        assert_json_round_trip(
            Nested {
                optional: None,
                unit: (),
                wrapped: Newtype(None),
            },
            &json!({"optional": null, "unit": null, "wrapped": null}),
        );
        assert_json_round_trip(Some(vec![None, Some(false)]), &json!([null, false]));
        assert_json_round_trip((None::<String>, ()), &json!([null, null]));
        assert_json_round_trip(Pair(None, ()), &json!([null, null]));
        assert_json_round_trip(
            BTreeMap::from([("nullable".to_owned(), None::<String>)]),
            &json!({"nullable": null}),
        );
        assert_json_round_trip(Compound::Newtype(None), &json!({"Newtype": null}));
        assert_json_round_trip(Compound::Tuple(None, ()), &json!({"Tuple": [null, null]}));
        assert_json_round_trip(
            Compound::Struct {
                optional: None,
                unit: (),
            },
            &json!({"Struct": {"optional": null, "unit": null}}),
        );
        assert_json_round_trip(Compound::Unit, &json!("Unit"));
    }

    #[test]
    fn debug_redacts_typed_values_without_requiring_debug_on_the_inner_type() {
        struct NotDebug;
        assert_eq!(
            format!("{:?}", FieldOverride::Value(NotDebug)),
            "Value(<redacted>)"
        );
        assert_eq!(
            format!("{:?}", FieldOverride::Value("private-setting")),
            "Value(<redacted>)"
        );
        assert_eq!(format!("{:?}", FieldOverride::<String>::Inherit), "Inherit");
        assert_eq!(format!("{:?}", FieldOverride::<String>::Clear), "Clear");
    }

    #[test]
    fn concrete_nullable_values_and_newtypes_keep_their_inner_form() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Wrapped(Option<Cookie>);

        assert_eq!(
            serde_json::from_value::<FieldOverride<Option<bool>>>(json!(false)).unwrap(),
            FieldOverride::Value(Some(false))
        );
        assert_eq!(
            serde_json::from_value::<FieldOverride<Option<String>>>(json!("")).unwrap(),
            FieldOverride::Value(Some(String::new()))
        );
        assert_eq!(
            serde_json::from_value::<FieldOverride<Option<Vec<u32>>>>(json!([])).unwrap(),
            FieldOverride::Value(Some(Vec::new()))
        );
        assert_eq!(
            serde_json::from_value::<FieldOverride<Wrapped>>(
                json!({"name": "optout", "value": "1"})
            )
            .unwrap(),
            FieldOverride::Value(Wrapped(Some(Cookie {
                name: "optout".to_owned(),
                value: "1".to_owned(),
            })))
        );
        let error = serde_json::from_value::<FieldOverride<Wrapped>>(json!(null)).unwrap_err();
        assert_eq!(error.classify(), Category::Data);
        assert_eq!(
            error.to_string(),
            "invalid type: null, expected a non-null setting or exactly { clear: true }"
        );
    }

    #[test]
    fn borrowed_values_and_keys_do_not_require_owned_deserialization() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Borrowed<'a> {
            value: &'a str,
        }

        let text = serde_json::to_string(&json!("borrowed")).unwrap();
        assert_eq!(
            serde_json::from_str::<FieldOverride<&str>>(&text).unwrap(),
            FieldOverride::Value("borrowed")
        );
        let object = serde_json::to_string(&json!({"value": "borrowed"})).unwrap();
        assert_eq!(
            serde_json::from_str::<FieldOverride<Borrowed<'_>>>(&object).unwrap(),
            FieldOverride::Value(Borrowed { value: "borrowed" })
        );
        assert_eq!(
            serde_json::from_str::<FieldOverride<BTreeMap<&str, &str>>>(&object).unwrap(),
            FieldOverride::Value(BTreeMap::from([("value", "borrowed")]))
        );
    }

    #[test]
    fn nested_nulls_and_clear_keys_belong_to_the_inner_value() {
        for value in [
            json!([null, {"clear": false}]),
            json!({"nested": {"clear": false}, "nullable": null}),
        ] {
            assert_eq!(
                serde_json::from_value::<FieldOverride<Value>>(value.clone()).unwrap(),
                FieldOverride::Value(value)
            );
        }
    }

    #[test]
    fn duplicate_ordinary_value_keys_are_rejected() {
        // Raw JSON preserves duplicate and escaped-equivalent keys that json! collapses.
        for wire in [
            r#"{"setting":{"name":"first","name":"second"}}"#,
            r#"{"setting":{"name":"first","\u006eame":"second"}}"#,
        ] {
            for error in [
                serde_json::from_str::<Setting<Value>>(wire).unwrap_err(),
                serde_json::from_str::<Setting<BTreeMap<String, String>>>(wire).unwrap_err(),
            ] {
                assert_eq!(error.classify(), Category::Data);
                assert!(
                    error
                        .to_string()
                        .starts_with("duplicate override value key")
                );
            }
        }
    }

    #[test]
    fn ignored_object_fields_cannot_hide_a_clear_marker() {
        #[derive(Deserialize)]
        struct Permissive {
            _name: String,
        }

        let error = toml::from_str::<Setting<Permissive>>(
            "setting = { _name = \"optout\", ignored = false, clear = true }\n",
        )
        .map(|parsed| parsed.setting)
        .unwrap_err();
        assert_eq!(
            error.message(),
            "a clear override must be exactly { clear: true }"
        );
    }

    #[test]
    fn equality_and_clone_only_require_the_corresponding_inner_traits() {
        #[derive(Clone, PartialEq, Eq)]
        struct NotDebug(u32);

        fn assert_eq_bound<T: Eq>() {}
        assert_eq_bound::<FieldOverride<NotDebug>>();

        let value = FieldOverride::Value(NotDebug(17));
        assert_eq!(value.clone(), value);
        assert_eq!(
            FieldOverride::<NotDebug>::Inherit.clone(),
            FieldOverride::Inherit
        );
        assert_eq!(
            FieldOverride::<NotDebug>::Clear.clone(),
            FieldOverride::Clear
        );
    }
}

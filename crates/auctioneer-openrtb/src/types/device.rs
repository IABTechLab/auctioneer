//! Device + Geo.

use serde::ser::SerializeMap as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

use crate::ext::misc::DeviceExt;
use crate::field_metadata::{FieldMetadata, Model, Storage};
use crate::types::field::JsonField;

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Device {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ua: Option<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub sua: JsonField<Box<UserAgent>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo: Option<Geo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dnt: Option<i32>,
    /// Limit Ad Tracking (0/1) — set by `prepare()`'s iOS LMT override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lmt: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub devicetype: JsonField<i32>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub make: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub model: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub osv: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub hwv: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub h: JsonField<i64>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub w: JsonField<i64>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub ppi: JsonField<i64>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub pxratio: JsonField<serde_json::Number>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub js: JsonField<i32>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub geofetch: JsonField<i32>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub flashver: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub language: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub langb: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub carrier: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub mccmnc: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub connectiontype: JsonField<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ifa: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<DeviceExt>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// `OpenRTB`'s structured user-agent context; extensions remain unclassified.
#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct UserAgent {
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub browsers: JsonField<Vec<BrandVersion>>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub platform: JsonField<BrandVersion>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub mobile: JsonField<i32>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub architecture: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub bitness: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub model: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub source: JsonField<i32>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub ext: JsonField<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// Sparse browser entries allocate their fields only when populated, so dense
/// arrays of `{}` remain within the inbound parser's allocation allowance.
#[derive(Clone, Debug, Default)]
pub struct BrandVersion {
    fields: Option<Box<BrandVersionFields>>,
}

impl BrandVersion {
    #[must_use]
    pub fn fields(&self) -> Option<&BrandVersionFields> {
        self.fields.as_deref()
    }

    pub fn fields_mut(&mut self) -> Option<&mut BrandVersionFields> {
        self.fields.as_deref_mut()
    }
}

impl FieldMetadata for BrandVersion {
    const STORAGE: Storage = Storage::Typed;
    const MODEL: Option<&'static Model> = BrandVersionFields::MODEL;
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata, name = "BrandVersion")]
pub struct BrandVersionFields {
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub brand: JsonField<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub version: JsonField<Vec<String>>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub ext: JsonField<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

impl Serialize for BrandVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.fields {
            Some(fields) => fields.serialize(serializer),
            None => serializer.serialize_map(Some(0))?.end(),
        }
    }
}

impl<'de> Deserialize<'de> for BrandVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = BrandVersionFields::deserialize(deserializer)?;
        let empty = fields.brand.is_missing()
            && fields.version.is_missing()
            && fields.ext.is_missing()
            && fields.other.is_empty();
        Ok(Self {
            fields: (!empty).then(|| Box::new(fields)),
        })
    }

    fn deserialize_in_place<D: Deserializer<'de>>(
        deserializer: D,
        place: &mut Self,
    ) -> Result<(), D::Error> {
        *place = Self::deserialize(deserializer)?;
        Ok(())
    }
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct Geo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lat: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lon: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ext: Option<Box<RawValue>>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;

    #[test]
    fn brand_metadata_follows_the_wire_fields_not_the_sparse_backing_slot() {
        let model = BrandVersion::MODEL.expect("wire metadata");
        assert_eq!(BrandVersion::STORAGE, Storage::Typed);
        assert_eq!(model.wire_name, "BrandVersion");
        assert_eq!(
            model
                .fields
                .iter()
                .filter(|field| !field.flattened)
                .map(|field| field.name)
                .collect::<Vec<_>>(),
            ["brand", "version", "ext"]
        );
        assert_eq!(
            model
                .fields
                .iter()
                .find(|field| field.name == "ext")
                .unwrap()
                .storage,
            Storage::RawJson
        );
        for json in [
            serde_json::json!({}),
            serde_json::json!({"brand":"Browser", "version":["1"], "ext":{"fake_field":true}}),
            serde_json::json!({"fake_unclassified":null}),
        ] {
            let brand: BrandVersion = serde_json::from_value(json.clone()).unwrap();
            let emitted = serde_json::to_value(brand).unwrap();
            assert_eq!(emitted, json);
            assert!(emitted.get("fields").is_none());
        }
    }

    #[test]
    fn device_geo_country_round_trips() {
        let json = serde_json::json!({"geo":{"country":"USA"}});
        let device: Device = round_trip(&json);
        assert_eq!(device.geo.unwrap().country.as_deref(), Some("USA"));
    }

    #[test]
    fn device_with_atts_round_trips() {
        let json = serde_json::json!({"ua":"UA","ip":"1.2.3.4","ext":{"atts":3_i32}});
        let device: Device = round_trip(&json);
        assert_eq!(device.ext.unwrap().atts, Some(3_i32));
    }

    #[test]
    fn device_context_preserves_integer_numbers_and_explicit_nulls() {
        for pxratio in [serde_json::json!(2_i32), serde_json::json!(2.0_f64)] {
            let json = serde_json::json!({"h": null, "pxratio": pxratio});
            let _: Device = round_trip(&json);
        }
        for field in [
            "sua",
            "devicetype",
            "make",
            "model",
            "osv",
            "hwv",
            "h",
            "w",
            "ppi",
            "pxratio",
            "js",
            "geofetch",
            "flashver",
            "language",
            "langb",
            "carrier",
            "mccmnc",
            "connectiontype",
        ] {
            let mut json = serde_json::json!({});
            json[field] = serde_json::Value::Null;
            let _: Device = round_trip(&json);
        }
    }

    #[test]
    fn device_screen_dimensions_accept_the_full_signed_integer_width() {
        let json = serde_json::json!({"h": i64::MAX, "w": i64::MAX, "ppi": i64::MAX});
        let _: Device = round_trip(&json);
    }

    #[test]
    fn device_context_rejects_wrong_json_types_without_coercion() {
        for json in [
            serde_json::json!({"h": "2556"}),
            serde_json::json!({"h": 2556.0_f64}),
            serde_json::json!({"make": 1_i32}),
            serde_json::json!({"pxratio": "2"}),
        ] {
            assert!(
                serde_json::from_value::<Device>(json.clone()).is_err(),
                "{json}"
            );
        }
    }

    #[test]
    fn structured_context_preserves_null_and_empty_containers() {
        let json = serde_json::json!({"sua": {
            "browsers": [], "platform": null, "mobile": null, "architecture": null,
            "bitness": null, "model": null, "source": null, "ext": null
        }});
        let _: Device = round_trip(&json);
        let brands = serde_json::json!({"sua": {"browsers": [
            {"brand": null, "version": [], "ext": null},
            {"brand": "Browser", "version": null}
        ]}});
        let _: Device = round_trip(&brands);
    }

    #[test]
    fn sparse_brand_entries_preserve_empty_and_unclassified_values() {
        for json in [
            serde_json::json!({}),
            serde_json::json!({"brand": null, "version": [], "ext": null}),
            serde_json::json!({"uid2": null, "empty_object": {}, "empty_array": []}),
        ] {
            let bytes = serde_json::to_vec(&json).unwrap();
            for parsed in [
                serde_json::from_slice::<BrandVersion>(&bytes).unwrap(),
                serde_json::from_value::<BrandVersion>(json.clone()).unwrap(),
            ] {
                assert_eq!(serde_json::to_value(parsed).unwrap(), json);
            }
        }
        let empty: BrandVersion = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(empty.fields().is_none());
    }

    #[test]
    fn interstitial_context_round_trips_presence_and_unknown_fields() {
        for json in [
            serde_json::json!({"ext": {"prebid": null}}),
            serde_json::json!({"ext": {"prebid": {"interstitial": null}}}),
            serde_json::json!({"ext": {"prebid": {"interstitial": {}}}}),
            serde_json::json!({"ext": {"prebid": {"interstitial": {
                "minwidthperc": 80_i64, "minheightperc": 70_i64, "uid2": "opaque-id"
            }, "uid2": "opaque-id"}}}),
        ] {
            let _: Device = round_trip(&json);
        }
    }

    #[test]
    fn structured_user_agent_round_trips_context_and_unclassified_data() {
        let json = serde_json::json!({
            "sua": {
                "browsers": [{ "brand": "Browser", "version": ["1", "2"], "uid2": "id" }],
                "platform": { "brand": "OS", "version": ["15"], "ext": { "uid2": "id" } },
                "mobile": 1_i32, "architecture": "arm", "bitness": "64",
                "model": "Handset", "source": 1_i32,
                "ext": { "vendor": "context" }, "unclassified": { "uid2": "id" }
            }
        });
        let device: Device = round_trip(&json);
        let sua = device.sua.as_ref().expect("typed structured user agent");
        assert_eq!(
            sua.browsers.as_ref().unwrap()[0]
                .fields()
                .unwrap()
                .brand
                .as_ref()
                .map(String::as_str),
            Some("Browser")
        );
        assert!(sua.other.contains_key("unclassified"));
        assert!(device.other.is_empty());
    }
}

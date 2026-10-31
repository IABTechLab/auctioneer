use crate::ConstructionDiagnostic;
use auctioneer_openrtb::types::{bid_request::Imp, field::JsonField};
use serde::{Serialize, Serializer, ser::SerializeMap as _};
use serde_json::{Map, Value};

pub(super) fn unique<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ()> {
    let mut matching = fields
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let selected = matching.next().map(|(_, value)| value);
    if matching.next().is_some() {
        Err(())
    } else {
        Ok(selected)
    }
}
pub(super) fn object(value: Option<&Value>) -> Result<Option<&Map<String, Value>>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(fields)) => Ok(Some(fields)),
        _ => Err(()),
    }
}
fn string(value: Option<&Value>) -> Result<&str, ()> {
    match value {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(text)) => Ok(text),
        _ => Err(()),
    }
}
pub(super) fn integer(value: Option<&Value>) -> Result<i64, ()> {
    match value {
        None | Some(Value::Null) => Ok(0),
        Some(number) => number.as_i64().ok_or(()),
    }
}
pub(super) fn boolean(value: Option<&Value>) -> Result<Option<bool>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        _ => Err(()),
    }
}
fn apis(value: Option<&Value>) -> Result<Option<&[Value]>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items))
            if items.iter().all(|item| {
                integer(Some(item)).is_ok_and(|number| i8::try_from(number).is_ok())
            }) =>
        {
            Ok(Some(items))
        }
        _ => Err(()),
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Parameters<'a> {
    pub placement: i64,
    pub member: i64,
    pub inventory: &'a str,
    pub smaller: Option<bool>,
    pub payment: Option<bool>,
    pub keywords: &'a str,
    pub traffic: &'a str,
    pub click: &'a str,
    pub external_inventory: &'a str,
    pub external_imp: &'a str,
    pub frameworks: Option<&'a [Value]>,
    pub gpid: &'a str,
}
pub(super) struct Impression<'a> {
    pub imp: &'a Imp,
    pub params: Parameters<'a>,
    pub member: i64,
    pub api: Option<&'a [Value]>,
    pub version: &'a str,
}

pub(super) fn prepare(imp: &Imp) -> Result<Impression<'_>, ConstructionDiagnostic> {
    prepare_inner(imp).map_err(|()| ConstructionDiagnostic::InvalidImpressionParameters)
}
fn prepare_inner(imp: &Imp) -> Result<Impression<'_>, ()> {
    let ext = imp.ext.as_ref().ok_or(())?;
    let alias = unique(&ext.other, "bidder")?;
    let value = match (&ext.bidder, alias) {
        (JsonField::Missing | JsonField::Null, None) => None,
        (JsonField::Value(value), None) | (JsonField::Missing, Some(value)) => Some(value),
        _ => return Err(()),
    };
    let fields = object(value)?;
    let get = |name| {
        fields
            .map(|parameters| unique(parameters, name))
            .transpose()
            .map(Option::flatten)
    };
    let params = Parameters {
        placement: integer(get("placement_id")?)?,
        member: integer(get("member")?)?,
        inventory: string(get("inv_code")?)?,
        smaller: boolean(get("allow_smaller_sizes")?)?,
        payment: boolean(get("use_pmt_rule")?)?,
        keywords: string(get("keywords")?)?,
        traffic: string(get("traffic_source_code")?)?,
        click: string(get("pubclick")?)?,
        external_inventory: string(get("ext_inv_code")?)?,
        external_imp: string(get("ext_imp_id")?)?,
        frameworks: apis(get("banner_frameworks")?)?,
        gpid: string(unique(&ext.other, "gpid")?)?,
    };
    let api = if let Some(banner) = &imp.banner {
        apis(banner.other.get("api"))?.or(params.frameworks)
    } else {
        None
    };
    let version = string(imp.other.get("displaymanagerver"))?;
    Ok(Impression {
        imp,
        params,
        member: params.member,
        api,
        version,
    })
}

pub(super) struct ImpressionExtension<'a>(pub Parameters<'a>);
impl Serialize for ImpressionExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("appnexus", &ImpressionNamespace(self.0))?;
        if !self.0.gpid.is_empty() {
            map.serialize_entry("gpid", self.0.gpid)?;
        }
        map.end()
    }
}
struct ImpressionNamespace<'a>(Parameters<'a>);
impl Serialize for ImpressionNamespace<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if self.0.placement != 0 {
            map.serialize_entry("placement_id", &self.0.placement)?;
        }
        if let Some(value) = self.0.smaller {
            map.serialize_entry("allow_smaller_sizes", &value)?;
        }
        if let Some(value) = self.0.payment {
            map.serialize_entry("use_pmt_rule", &value)?;
        }
        for (key, value) in [
            ("keywords", self.0.keywords),
            ("traffic_source_code", self.0.traffic),
            ("pub_click", self.0.click),
            ("ext_inv_code", self.0.external_inventory),
            ("ext_imp_id", self.0.external_imp),
        ] {
            if !value.is_empty() {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::MsftBidder;
    #[cfg(not(target_arch = "wasm32"))]
    use super::super::tests::captured_fixture;
    use super::super::tests::{AllowEndpoints, banner, body, info, request};
    use crate::bidders::harness;
    use crate::{EntryPoint, SinkLimits};
    #[cfg(not(target_arch = "wasm32"))]
    use serde_json::Value;
    use serde_json::json;

    #[test]
    fn present_parameter_structs_accept_source_defaults_without_synthesizing_schema_validity() {
        for ext in [json!({}), json!({"bidder":null}), json!({"bidder":{}})] {
            let request = request(json!([{"id":"i","banner":{},"ext":ext}]));
            assert_eq!(
                body(&request, &info(EntryPoint::Auction))["imp"][0]["ext"],
                json!({"appnexus":{}})
            );
        }
        let request = request(json!([{"id":"i","banner":{}}]));
        let (_, outcome, _) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info(EntryPoint::Auction),
            SinkLimits::new(4096, 1),
            1,
            &AllowEndpoints,
        );
        assert!(outcome.accepted.is_empty());
        assert_eq!(outcome.diagnostics.len(), 1);
    }

    #[test]
    fn one_ascii_equivalent_spelling_binds_but_competitors_filter_only_the_affected_imp() {
        let request = request(json!([
            {"id":"one","banner":{},"ext":{"BIDDER":{"PLACEMENT_ID":1_i32},"GPID":"g"}},
            banner("two",json!({"placement_id":2_i32,"PLACEMENT_ID":null})),
            {"id":"three","banner":{},"ext":{"bidder":null,"BIDDER":null}}
        ]));
        let (_, outcome, _) = harness::drive_with_limits(
            &MsftBidder,
            &request,
            &info(EntryPoint::Auction),
            SinkLimits::new(8192, 3),
            1,
            &AllowEndpoints,
        );
        assert_eq!(outcome.accepted.len(), 1);
        assert_eq!(
            outcome.accepted[0].covered_imp_ids().collect::<Vec<_>>(),
            ["one"]
        );
        assert_eq!(outcome.diagnostics.len(), 2);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn all_source_parameter_verdicts_use_the_canonical_schema_not_the_adapter_defaults() {
        let cases = captured_fixture(
            "fixtures/parameters.json",
            include_str!("fixtures/parameters.json"),
        );
        let schema = captured_fixture("parameters.json", include_str!("parameters.json"));
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft4)
            .build(&schema)
            .unwrap();
        let rows = cases["cases"].as_array().unwrap();
        assert_eq!(rows.len(), 33);
        assert_eq!(rows.iter().filter(|row| row["valid"] == true).count(), 12);
        let mut checked = 0_usize;
        for row in rows {
            let parsed = serde_json::from_str::<Value>(row["input"].as_str().unwrap());
            let valid = parsed.as_ref().is_ok_and(|value| validator.is_valid(value));
            assert_eq!(
                valid,
                row["valid"].as_bool().unwrap(),
                "{} {}",
                row["group"],
                row["index"]
            );
            checked = checked.saturating_add(1);
        }
        assert_eq!(checked, 33);
        let all_parameters = serde_json::from_str::<Value>(include_str!(
            "fixtures/protocol/test/exemplary/all-params.json"
        ))
        .unwrap();
        assert!(!validator.is_valid(&all_parameters["mockBidRequest"]["imp"][0]["ext"]["bidder"]));
    }
}

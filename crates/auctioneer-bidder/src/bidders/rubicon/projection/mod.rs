mod context;
mod impression;
mod targeting;

use self::context::{
    AppProjection, DeviceProjection, RegsProjection, SiteProjection, SourceProjection,
    UserProjection,
};
use self::impression::ImpProjection;
use super::parameters::{Parameters, known};
use crate::correlation::CorrelationMedia;
use auctioneer_openrtb::types::{
    bid_request::{BidRequest, Imp},
    field::JsonField,
};
use auctioneer_runtime::bidder_credentials::BidderCredentials;
use serde::{
    Serialize, Serializer,
    ser::{Error as SerError, SerializeMap},
};
use serde_json::{Map, Value};

pub(super) fn put<M: SerializeMap, T: Serialize + ?Sized>(
    map: &mut M,
    key: &str,
    value: Option<&T>,
) -> Result<(), M::Error> {
    if let Some(present) = value {
        map.serialize_entry(key, present)?;
    }
    Ok(())
}
fn json_field<M: SerializeMap, T: Serialize>(
    map: &mut M,
    key: &str,
    value: &JsonField<T>,
) -> Result<(), M::Error> {
    if !matches!(value, JsonField::Missing) {
        map.serialize_entry(key, value)?;
    }
    Ok(())
}
pub(super) fn preserve<M: SerializeMap>(
    map: &mut M,
    other: &Map<String, Value>,
    omit: &[&str],
) -> Result<(), M::Error> {
    for (key, value) in other {
        if !omit.iter().any(|name| key.eq_ignore_ascii_case(name)) {
            map.serialize_entry(key, value)?;
        }
    }
    Ok(())
}
pub(super) fn ser_error<E: SerError>() -> E {
    E::custom("invalid rubicon request fields")
}

struct Rp<T>(T);
impl<T: Serialize> Serialize for Rp<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("rp", &self.0)?;
        map.end()
    }
}

pub(super) struct Request<'a> {
    pub original: &'a BidRequest,
    pub imp: &'a Imp,
    pub params: Parameters<'a>,
    pub selected: CorrelationMedia,
    pub floor: (Option<f64>, Option<&'static str>),
    pub credentials: &'a BidderCredentials,
    pub external_url: &'a str,
    pub raw_cap: usize,
}

impl Serialize for Request<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let BidRequest {
            id,
            imp: _imp,
            site: inventory_site,
            app,
            dooh,
            device,
            user,
            regs,
            source,
            at,
            tmax,
            cur: _cur,
            test,
            ext: _ext,
            other,
        } = self.original;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        preserve(&mut map, other, &["badv"])?;
        if let Some(badv) = known(other, "badv").map_err(|()| ser_error::<S::Error>())? {
            let values = badv.as_array().ok_or_else(ser_error::<S::Error>)?;
            let limited = values
                .get(..values.len().min(50))
                .ok_or_else(ser_error::<S::Error>)?;
            map.serialize_entry("badv", limited)?;
        }
        put(&mut map, "at", at.as_ref())?;
        put(&mut map, "tmax", tmax.as_ref())?;
        put(&mut map, "test", test.as_ref())?;
        put(&mut map, "dooh", dooh.as_ref())?;
        if let Some(site) = inventory_site {
            map.serialize_entry("site", &SiteProjection(site, self.params))?;
        }
        if let Some(application) = app {
            if inventory_site.is_none() {
                map.serialize_entry("app", &AppProjection(application, self.params))?;
            } else {
                map.serialize_entry("app", application)?;
            }
        }
        if let Some(client_device) = device {
            map.serialize_entry("device", &DeviceProjection(client_device))?;
        }
        if let Some(bid_user) = user {
            map.serialize_entry("user", &UserProjection(bid_user, self.params))?;
        }
        if let Some(regulations) = regs {
            map.serialize_entry("regs", &RegsProjection(regulations))?;
        }
        let legacy = self
            .original
            .ext
            .as_ref()
            .and_then(|ext| ext.other.get("schain"));
        if source.is_some() || legacy.is_some() {
            map.serialize_entry("source", &SourceProjection(source.as_ref(), legacy))?;
        }
        map.serialize_entry("imp", &[ImpProjection(self)])?;
        map.end()
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::bidders::rubicon::test_support::{banner, body, outbound, request};
    use serde_json::json;

    #[test]
    fn blocklists_drop_only_advertiser_domains_beyond_the_source_limit() {
        let mut request = request(&json!([banner("i")]));
        request.other.insert("bcat".into(), json!(["IAB1", "IAB2"]));
        request.other.insert(
            "badv".into(),
            json!(
                (0_i32..51_i32)
                    .map(|i| format!("domain-{i}.example"))
                    .collect::<Vec<_>>()
            ),
        );
        request.cur = vec!["EUR".into()];
        let before = serde_json::to_value(&request).unwrap();
        let calls = outbound(&request);
        assert_eq!(calls.len(), 1);
        let body = body(&calls[0]);
        assert_eq!(body["bcat"], json!(["IAB1", "IAB2"]));
        assert_eq!(body["badv"].as_array().unwrap().len(), 50);
        assert_eq!(body["badv"][49], "domain-49.example");
        assert!(body.get("cur").is_none());
        assert!(body.get("ext").is_none());
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }
}

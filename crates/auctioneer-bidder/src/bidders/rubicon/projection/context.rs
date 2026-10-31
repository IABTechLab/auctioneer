use super::super::parameters::{Parameters, known, object, optional_known};
use super::targeting::{Layer, Segments, Target};
use super::{Rp, json_field, preserve, put, ser_error};
use auctioneer_openrtb::ext::misc::SourceExt;
use auctioneer_openrtb::types::{
    device::Device,
    field::JsonField,
    inventory::{App, Site},
    regs::Regs,
    source::Source,
    user::User,
};
use serde::{Serialize, Serializer, ser::SerializeMap as _};
use serde_json::Value;

#[derive(Serialize)]
struct InventoryRp<T: Serialize> {
    site_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<T>,
}
#[derive(Serialize)]
struct PublisherRp {
    account_id: i64,
}

#[derive(Serialize)]
struct DeviceRp {
    pixelratio: f64,
}

pub(super) struct SiteProjection<'a>(pub &'a Site, pub Parameters<'a>);
impl Serialize for SiteProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Site {
            id,
            name,
            domain,
            page,
            publisher: _publisher,
            ext: _ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        put(&mut map, "id", id.as_ref())?;
        put(&mut map, "name", name.as_ref())?;
        put(&mut map, "domain", domain.as_ref())?;
        put(&mut map, "page", page.as_ref())?;
        preserve(&mut map, other, &[])?;
        let data = other
            .get("content")
            .and_then(Value::as_object)
            .and_then(|content| content.get("data"))
            .and_then(Value::as_array)
            .map(Vec::as_slice);
        let segments = Segments { data, user: false };
        let target = segments
            .iter()
            .next()
            .is_some()
            .then_some(SiteIab(segments));
        map.serialize_entry(
            "ext",
            &Rp(InventoryRp {
                site_id: self.1.site,
                target,
            }),
        )?;
        map.serialize_entry("publisher", &PublisherProjection(self.1.account))?;
        map.end()
    }
}
struct SiteIab<'a>(Segments<'a>);
impl Serialize for SiteIab<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("iab", &self.0)?;
        map.end()
    }
}
struct PublisherProjection(i64);
impl Serialize for PublisherProjection {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("ext", &Rp(PublisherRp { account_id: self.0 }))?;
        map.end()
    }
}
pub(super) struct AppProjection<'a>(pub &'a App, pub Parameters<'a>);
impl Serialize for AppProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let App {
            id,
            name,
            bundle,
            domain,
            publisher: _publisher,
            ext: _ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        put(&mut map, "id", id.as_ref())?;
        put(&mut map, "name", name.as_ref())?;
        put(&mut map, "bundle", bundle.as_ref())?;
        put(&mut map, "domain", domain.as_ref())?;
        preserve(&mut map, other, &[])?;
        map.serialize_entry(
            "ext",
            &Rp(InventoryRp::<SiteIab<'_>> {
                site_id: self.1.site,
                target: None,
            }),
        )?;
        map.serialize_entry("publisher", &PublisherProjection(self.1.account))?;
        map.end()
    }
}
pub(super) struct DeviceProjection<'a>(pub &'a Device);
impl Serialize for DeviceProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Device {
            ua,
            sua,
            ip,
            ipv6,
            geo,
            dnt,
            lmt,
            os,
            devicetype,
            make,
            model,
            osv,
            hwv,
            h,
            w,
            ppi,
            pxratio,
            js,
            geofetch,
            flashver,
            language,
            langb,
            carrier,
            mccmnc,
            connectiontype,
            ifa,
            ext: _ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        put(&mut map, "ua", ua.as_ref())?;
        json_field(&mut map, "sua", sua)?;
        put(&mut map, "ip", ip.as_ref())?;
        put(&mut map, "ipv6", ipv6.as_ref())?;
        put(&mut map, "geo", geo.as_ref())?;
        put(&mut map, "dnt", dnt.as_ref())?;
        put(&mut map, "lmt", lmt.as_ref())?;
        put(&mut map, "os", os.as_ref())?;
        json_field(&mut map, "devicetype", devicetype)?;
        json_field(&mut map, "make", make)?;
        json_field(&mut map, "model", model)?;
        json_field(&mut map, "osv", osv)?;
        json_field(&mut map, "hwv", hwv)?;
        json_field(&mut map, "h", h)?;
        json_field(&mut map, "w", w)?;
        json_field(&mut map, "ppi", ppi)?;
        json_field(&mut map, "pxratio", pxratio)?;
        json_field(&mut map, "js", js)?;
        json_field(&mut map, "geofetch", geofetch)?;
        json_field(&mut map, "flashver", flashver)?;
        json_field(&mut map, "language", language)?;
        json_field(&mut map, "langb", langb)?;
        json_field(&mut map, "carrier", carrier)?;
        json_field(&mut map, "mccmnc", mccmnc)?;
        json_field(&mut map, "connectiontype", connectiontype)?;
        put(&mut map, "ifa", ifa.as_ref())?;
        preserve(&mut map, other, &[])?;
        let ratio = match pxratio {
            JsonField::Value(value) => value.as_f64().unwrap_or_default(),
            JsonField::Missing | JsonField::Null => 0.0_f64,
        };
        map.serialize_entry("ext", &Rp(DeviceRp { pixelratio: ratio }))?;
        map.end()
    }
}
pub(super) struct UserProjection<'a>(pub &'a User, pub Parameters<'a>);
impl Serialize for UserProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let User {
            id,
            buyeruid,
            yob: _yob,
            gender: _gender,
            keywords,
            consent: _consent,
            eids: _eids,
            geo: _geo,
            data,
            ext: _ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        put(&mut map, "id", id.as_ref())?;
        put(&mut map, "buyeruid", buyeruid.as_ref())?;
        put(&mut map, "keywords", keywords.as_ref())?;
        put(&mut map, "data", data.as_ref())?;
        preserve(&mut map, other, &[])?;
        map.serialize_entry("ext", &UserExtension(self.0, self.1))?;
        map.end()
    }
}
struct UserExtension<'a>(&'a User, Parameters<'a>);
impl Serialize for UserExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let extension = self.0.ext.as_ref();
        let rp_value = extension
            .map(|ext| known(&ext.other, "rp"))
            .transpose()
            .map_err(|()| ser_error::<S::Error>())?
            .flatten();
        let rp = object(rp_value).map_err(|()| ser_error::<S::Error>())?;
        let prior = object(optional_known(rp, "target").map_err(|()| ser_error::<S::Error>())?)
            .map_err(|()| ser_error::<S::Error>())?;
        let data = object(extension.and_then(|ext| ext.data.as_ref()))
            .map_err(|()| ser_error::<S::Error>())?;
        let iab = Segments {
            data: self
                .0
                .data
                .as_ref()
                .and_then(Value::as_array)
                .map(Vec::as_slice),
            user: true,
        };
        let target = Target {
            prior,
            layers: [
                self.1.visitor.map(Layer::Map),
                data.map(Layer::Map),
                None,
                None,
            ],
            slot: "",
            dfp: "",
            keywords: &[],
            host: None,
            iab: Some(iab),
        };
        let mut map = serializer.serialize_map(None)?;
        match self.0.eids.as_slice() {
            [] => {
                if let Some(eids) = extension
                    .and_then(|ext| ext.eids.as_ref())
                    .filter(|eids| eids.as_array().is_some_and(|a| !a.is_empty()))
                {
                    map.serialize_entry("eids", eids)?;
                }
            }
            eids => map.serialize_entry("eids", eids)?,
        }
        let consent = self
            .0
            .consent
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                extension
                    .and_then(|ext| known(&ext.other, "consent").ok().flatten())
                    .and_then(Value::as_str)
            });
        if let Some(nonempty_consent) = consent.filter(|s| !s.is_empty()) {
            map.serialize_entry("consent", nonempty_consent)?;
        }
        map.serialize_entry("rp", &UserRp(target))?;
        map.end()
    }
}
struct UserRp<T: Serialize>(T);
impl<T: Serialize> Serialize for UserRp<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("target", &self.0)?;
        map.end()
    }
}
pub(super) struct SourceProjection<'a>(pub Option<&'a Source>, pub Option<&'a Value>);
impl Serialize for SourceProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(Source {
            fd,
            tid,
            pchain,
            ext: _ext,
            other,
        }) = self.0
        {
            put(&mut map, "fd", fd.as_ref())?;
            put(&mut map, "tid", tid.as_ref())?;
            put(&mut map, "pchain", pchain.as_ref())?;
            preserve(&mut map, other, &["schain"])?;
        }
        let direct = self
            .0
            .and_then(|source| source.other.get("schain"))
            .filter(|value| !value.is_null());
        let chain = direct
            .or_else(|| {
                let ext = self.0?.ext.as_ref()?;
                ext.schain.as_ref()
            })
            .or(self.1);
        let ext = self.0.and_then(|source| source.ext.as_ref());
        if chain.is_some() || ext.is_some() {
            map.serialize_entry("ext", &SourceExtension(ext, chain))?;
        }
        map.end()
    }
}
struct SourceExtension<'a>(Option<&'a SourceExt>, Option<&'a Value>);
impl Serialize for SourceExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(ext) = self.0 {
            preserve(&mut map, &ext.other, &["schain"])?;
        }
        put(&mut map, "schain", self.1)?;
        map.end()
    }
}
pub(super) struct RegsProjection<'a>(pub &'a Regs);
impl Serialize for RegsProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Regs {
            coppa,
            gdpr: _gdpr,
            us_privacy: _us_privacy,
            gpp,
            gpp_sid,
            ext: _ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        put(&mut map, "coppa", coppa.as_ref())?;
        put(&mut map, "gpp", gpp.as_ref())?;
        put(&mut map, "gpp_sid", gpp_sid.as_ref())?;
        preserve(&mut map, other, &[])?;
        if self.0.ext.is_some()
            || self.0.gdpr.is_some()
            || self.0.us_privacy.as_ref().is_some_and(|v| !v.is_empty())
        {
            map.serialize_entry("ext", &RegsExtension(self.0))?;
        }
        map.end()
    }
}
struct RegsExtension<'a>(&'a Regs);
impl Serialize for RegsExtension<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if let Some(ext) = &self.0.ext {
            put(&mut map, "dsa", ext.dsa.as_ref())?;
            put(&mut map, "gpc", ext.gpc.as_ref())?;
            preserve(&mut map, &ext.other, &["gdpr", "us_privacy"])?;
        }
        if let Some(gdpr) = self.0.gdpr {
            map.serialize_entry("gdpr", &gdpr)?;
        } else {
            let legacy = self
                .0
                .ext
                .as_ref()
                .and_then(|ext| ext.other.get("gdpr"))
                .filter(|v| !v.is_null());
            put(&mut map, "gdpr", legacy)?;
        }
        if let Some(value) = self.0.us_privacy.as_deref().filter(|v| !v.is_empty()) {
            map.serialize_entry("us_privacy", value)?;
        } else {
            let legacy = self
                .0
                .ext
                .as_ref()
                .and_then(|ext| ext.other.get("us_privacy"))
                .filter(|v| !v.is_null());
            put(&mut map, "us_privacy", legacy)?;
        }
        map.end()
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::bidders::rubicon::test_support::{banner, body, outbound, request};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};

    #[test]
    fn inventory_and_publisher_extensions_use_each_impressions_parameter_ids() {
        let mut second = banner("second");
        second["ext"]["bidder"] = json!({"accountId":"7891","siteId":"283282","zoneId":"8394"});
        let calls = outbound(&request(&json!([banner("first"), second])));
        assert_eq!(calls.len(), 2);
        for (call, account, site, zone) in [
            (&calls[0], 1001_i64, 113_932_i64, 535_510_i64),
            (&calls[1], 7891, 283_282, 8394),
        ] {
            let body = body(call);
            assert_eq!(
                body["site"]["publisher"],
                json!({"ext":{"rp":{"account_id":account}}})
            );
            assert_eq!(body["site"]["ext"], json!({"rp":{"site_id":site}}));
            assert_eq!(body["imp"][0]["ext"]["rp"]["zone_id"], zone);
        }
    }

    #[test]
    fn iab_segments_copy_only_reviewed_site_and_user_taxonomies_in_order() {
        let record = |tax: Value, id: &str| json!({"ext":{"segtax":tax},"segment":[{"id":id}]});
        let mut document = json!({"id":"r","imp":[banner("i")],"site":{"content":{"data":[
            record(json!(1_i32),"s1"),record(json!(2_i32),"s2"),record(json!(5_i32),"s5"),
            record(json!(6_i32),"s6"),record(json!(4_i32),"wrong"),record(json!("1"),"string")
        ]}},"user":{"data":[record(json!(4_i32),"u4"),record(json!(1_i32),"wrong"),
            record(json!("4"),"string"),record(json!(4_i32),"u4")] }});
        document["site"]["publisher"] = json!({"id":"scoped"});
        let request = serde_json::from_value(document).unwrap();
        let calls = outbound(&request);
        assert_eq!(calls.len(), 1);
        let body = body(&calls[0]);
        assert_eq!(
            body["site"]["ext"]["rp"]["target"]["iab"],
            json!(["s1", "s2", "s5", "s6"])
        );
        assert_eq!(
            body["user"]["ext"]["rp"]["target"]["iab"],
            json!(["u4", "u4"])
        );
    }

    #[test]
    fn user_device_source_and_regs_transforms_preserve_scoped_fields_not_restoring_identifiers() {
        let request: BidRequest = serde_json::from_value(json!({"id":"r","imp":[banner("i")],
            "site":{},"device":{"ua":"scoped","pxratio":2.5_f64,"ext":{"discard":true}},
            "user":{"yob":2000_i32,"gender":"f","geo":{"country":"USA"},"consent":"scoped-consent",
                "eids":[{"source":"scoped.example","uids":[{"id":"scoped-id"}]}],
                "ext":{"rp":{"target":{"prior":["keep"]}},"data":{"scoped":true}}},
            "source":{"tid":"scoped-tid","schain":{"ver":"1.0","complete":1_i32,"nodes":[]}},
            "regs":{"gdpr":1_i32,"us_privacy":"1YNN","gpp":"gpp","gpp_sid":[2_i32]}
        }))
        .unwrap();
        let before = serde_json::to_value(&request).unwrap();
        let calls = outbound(&request);
        assert_eq!(calls.len(), 1);
        let body = body(&calls[0]);
        assert_eq!(body["device"]["ext"], json!({"rp":{"pixelratio":2.5_f64}}));
        assert_eq!(body["device"]["ua"], "scoped");
        assert!(body["device"].get("ip").is_none());
        assert_eq!(body["user"]["ext"]["consent"], "scoped-consent");
        assert_eq!(body["user"]["ext"]["eids"], before["user"]["eids"]);
        assert_eq!(
            body["user"]["ext"]["rp"]["target"],
            json!({"prior":["keep"],"scoped":["true"]})
        );
        for removed in ["yob", "gender", "geo", "consent", "eids"] {
            assert!(body["user"].get(removed).is_none());
        }
        assert!(body["user"].get("id").is_none());
        assert_eq!(body["source"]["ext"]["schain"], before["source"]["schain"]);
        assert!(body["source"].get("schain").is_none());
        assert_eq!(body["source"]["tid"], "scoped-tid");
        assert_eq!(body["regs"]["ext"]["gdpr"], 1_i32);
        assert_eq!(body["regs"]["ext"]["us_privacy"], "1YNN");
        assert_eq!(body["regs"]["gpp"], "gpp");
        assert_eq!(body["regs"]["gpp_sid"], json!([2_i32]));
        assert!(body["regs"].get("gdpr").is_none());
        assert!(body["regs"].get("us_privacy").is_none());
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }
}

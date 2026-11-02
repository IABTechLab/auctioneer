use super::super::native::NativeProjection;
use super::super::parameters::{VideoParameters, known};
use super::super::validation::rewarded_inventory;
use super::{Request, Rp, json_field, preserve, put, ser_error};
use crate::correlation::CorrelationMedia;
use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp, Video};
use serde::{Serialize, Serializer, ser::SerializeMap as _};

#[derive(Serialize)]
struct BannerRp {
    mime: &'static str,
}

#[derive(Serialize)]
struct Track {
    mint: &'static str,
    mint_version: &'static str,
}
#[derive(Serialize)]
struct ImpRp<T: Serialize> {
    zone_id: i64,
    target: T,
    track: Track,
}

pub(super) struct ImpProjection<'a, 'b>(pub &'a Request<'b>);
impl Serialize for ImpProjection<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let request = self.0;
        let Imp {
            id,
            banner,
            video,
            native,
            audio: _audio,
            bidfloor: _bidfloor,
            bidfloorcur,
            secure: _secure,
            ext: _ext,
            other,
        } = request.imp;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        preserve(&mut map, other, &["rwdd"])?;
        let rwdd = rewarded_inventory(request.imp).map_err(|()| ser_error::<S::Error>())?;
        if request.selected != CorrelationMedia::Video {
            if rwdd != 0 {
                map.serialize_entry("rwdd", &rwdd)?;
            } else {
                put(&mut map, "rwdd", other.get("rwdd"))?;
            }
        }
        match request.selected {
            CorrelationMedia::Banner => map.serialize_entry(
                "banner",
                &BannerProjection(banner.as_ref().ok_or_else(ser_error::<S::Error>)?),
            )?,
            CorrelationMedia::Video => map.serialize_entry(
                "video",
                &VideoProjection(
                    video.as_ref().ok_or_else(ser_error::<S::Error>)?,
                    request.params.video,
                    rwdd == 1,
                ),
            )?,
            CorrelationMedia::Native => map.serialize_entry(
                "native",
                &NativeProjection(native.as_ref().ok_or_else(ser_error::<S::Error>)?),
            )?,
            CorrelationMedia::Audio => return Err(ser_error()),
        }
        put(&mut map, "bidfloor", request.floor.0.as_ref())?;
        if let Some(code) = request.floor.1 {
            map.serialize_entry("bidfloorcur", code)?;
        } else {
            put(&mut map, "bidfloorcur", bidfloorcur.as_ref())?;
        }
        map.serialize_entry("secure", &1_i32)?;
        map.serialize_entry("ext", &ImpExtension(request))?;
        map.end()
    }
}

fn max_bids(request: &BidRequest) -> Option<i64> {
    let first = request
        .ext
        .as_ref()?
        .prebid
        .as_ref()?
        .multibid
        .as_ref()?
        .as_array()?
        .first()?
        .as_object()?;
    known(first, "maxbids").ok().flatten()?.as_i64()
}
struct ImpExtension<'a, 'b>(&'a Request<'b>);
impl Serialize for ImpExtension<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let request = self.0;
        let ext = request.imp.ext.as_ref().ok_or_else(ser_error::<S::Error>)?;
        let target = request.target().map_err(|()| ser_error::<S::Error>())?;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry(
            "rp",
            &ImpRp {
                zone_id: request.params.zone,
                target,
                track: Track {
                    mint: "",
                    mint_version: "",
                },
            },
        )?;
        for key in ["gpid", "skadn"] {
            if let Some(value) = known(&ext.other, key)
                .map_err(|()| ser_error::<S::Error>())?
                .filter(|value| !value.is_null() && value.as_str() != Some(""))
            {
                map.serialize_entry(key, value)?;
            }
        }
        put(
            &mut map,
            "tid",
            ext.tid.as_deref().filter(|value| !value.is_empty()),
        )?;
        put(&mut map, "maxbids", max_bids(request.original).as_ref())?;
        map.end()
    }
}
struct BannerProjection<'a>(&'a Banner);
impl Serialize for BannerProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Banner {
            format,
            w,
            h,
            pos,
            ext: _ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        if !format.is_empty() {
            map.serialize_entry("format", format)?;
        }
        put(&mut map, "w", w.as_ref())?;
        put(&mut map, "h", h.as_ref())?;
        put(&mut map, "pos", pos.as_ref())?;
        preserve(&mut map, other, &[])?;
        map.serialize_entry("ext", &Rp(BannerRp { mime: "text/html" }))?;
        map.end()
    }
}
struct VideoProjection<'a>(&'a Video, VideoParameters, bool);
impl Serialize for VideoProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Video {
            mimes,
            minduration,
            maxduration,
            w,
            h,
            ext: _ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        json_field(&mut map, "mimes", mimes)?;
        put(&mut map, "minduration", minduration.as_ref())?;
        put(&mut map, "maxduration", maxduration.as_ref())?;
        put(&mut map, "w", w.as_ref())?;
        put(&mut map, "h", h.as_ref())?;
        preserve(&mut map, other, &[])?;
        map.serialize_entry("ext", &VideoExtension(self.1, self.2))?;
        map.end()
    }
}
struct VideoExtension(VideoParameters, bool);
#[derive(Serialize)]
struct VideoRp {
    #[serde(skip_serializing_if = "is_zero")]
    size_id: i64,
}
#[cfg_attr(
    target_pointer_width = "64",
    expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Serde skip_serializing_if callbacks must borrow the field"
    )
)]
fn is_zero(value: &i64) -> bool {
    *value == 0
}
impl Serialize for VideoExtension {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if self.0.skip != 0 {
            map.serialize_entry("skip", &self.0.skip)?;
        }
        if self.0.skipdelay != 0 {
            map.serialize_entry("skipdelay", &self.0.skipdelay)?;
        }
        if self.1 {
            map.serialize_entry("videotype", "rewarded")?;
        }
        map.serialize_entry(
            "rp",
            &VideoRp {
                size_id: self.0.size,
            },
        )?;
        map.end()
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::bidders::rubicon::test_support::{
        banner, body, mixed, native_impression, outbound, request,
    };
    use crate::correlation::CorrelationMedia;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::json;

    #[test]
    fn video_rewarded_and_parameter_extensions_replace_only_source_owned_fields() {
        let mut imp = mixed("i");
        imp["rwdd"] = json!(1_i32);
        imp["video"]["ext"] = json!({"discard":true});
        imp["video"]["keep"] = json!({"scoped":true});
        imp["ext"]["bidder"]["video"] = json!({"size_id":201_i32,"skip":1_i32,"skipdelay":5_i32,
            "language":"not-forwarded", "playerWidth":"640", "playerHeight":480_i32});
        let call = outbound(&request(&json!([imp]))).remove(0);
        let document = body(&call);
        assert_eq!(
            document["imp"][0]["video"]["ext"],
            json!({
                "rp":{"size_id":201_i32},"skip":1_i32,"skipdelay":5_i32,"videotype":"rewarded"
            })
        );
        assert!(document["imp"][0].get("rwdd").is_none());
        assert_eq!(document["imp"][0]["video"]["keep"], json!({"scoped":true}));
        assert_eq!(document["imp"][0]["secure"], 1_i32);
    }

    #[test]
    fn legacy_rewarded_signal_is_upgraded_before_video_transformation() {
        let mut imp = mixed("i");
        imp["ext"]["prebid"] = json!({"is_rewarded_inventory":1_i32});
        let calls = outbound(&request(&json!([imp])));
        assert_eq!(calls.len(), 1);
        assert_eq!(
            body(&calls[0])["imp"][0]["video"]["ext"]["videotype"],
            "rewarded"
        );
        assert!(body(&calls[0])["imp"][0].get("rwdd").is_none());
    }

    #[test]
    fn legacy_rewarded_signal_upgrades_banner_and_native_without_overwriting_explicit_rwdd() {
        for (media, candidate) in [
            ("banner", banner("i")),
            (
                "native",
                native_impression("1.2", &json!({"eventtrackers":[],"plcmttype":2_i32})),
            ),
        ] {
            for explicit in [Some(2_i32), None] {
                let mut imp = candidate.clone();
                imp["ext"]["prebid"] = json!({"is_rewarded_inventory":1_i32});
                if let Some(rwdd) = explicit {
                    imp["rwdd"] = json!(rwdd);
                } else {
                    assert!(imp.get("rwdd").is_none());
                }
                let original = request(&json!([imp]));
                let before = serde_json::to_value(&original).unwrap();
                let calls = outbound(&original);
                assert_eq!(calls.len(), 1);
                let document = body(&calls[0]);
                let projected = &document["imp"][0];
                assert!(projected.get(media).is_some());
                assert_eq!(projected["rwdd"], explicit.unwrap_or(1_i32));
                assert_eq!(serde_json::to_value(&original).unwrap(), before);
            }
        }
    }

    #[test]
    fn split_legacy_rewarded_signal_reaches_each_actual_wire_media() {
        for explicit in [Some(2_i32), None] {
            let mut imp = mixed("i");
            imp["ext"]["prebid"] = json!({"is_rewarded_inventory":1_i32});
            imp["ext"]["bidder"]["bidonmultiformat"] = json!(true);
            if let Some(rwdd) = explicit {
                imp["rwdd"] = json!(rwdd);
            } else {
                assert!(imp.get("rwdd").is_none());
            }
            let original = request(&json!([imp]));
            let before = serde_json::to_value(&original).unwrap();
            let calls = outbound(&original);
            assert_eq!(calls.len(), 3);
            for (call, selected) in calls.iter().zip([
                CorrelationMedia::Banner,
                CorrelationMedia::Video,
                CorrelationMedia::Native,
            ]) {
                assert_eq!(call.meta().imp_correlation(0).unwrap().media(), selected);
                let document = body(call);
                let projected = &document["imp"][0];
                if selected == CorrelationMedia::Video {
                    assert!(projected.get("rwdd").is_none());
                    assert_eq!(
                        projected["video"]["ext"].get("videotype"),
                        explicit.is_none().then_some(&json!("rewarded"))
                    );
                } else {
                    assert_eq!(projected["rwdd"], explicit.unwrap_or(1_i32));
                }
            }
            assert_eq!(serde_json::to_value(&original).unwrap(), before);
        }
    }

    #[test]
    fn banner_accepts_formats_or_positive_explicit_dimensions() {
        for banner_value in [
            json!({"format":[{"w":300_i32,"h":250_i32}]}),
            json!({"w":300_i32,"h":250_i32}),
        ] {
            let mut imp = banner("i");
            imp["banner"] = banner_value;
            let calls = outbound(&request(&json!([imp])));
            assert_eq!(calls.len(), 1);
            assert_eq!(
                body(&calls[0])["imp"][0]["banner"]["ext"],
                json!({"rp":{"mime":"text/html"}})
            );
        }
    }

    #[test]
    fn first_multibid_entry_gpid_skadn_and_tid_are_forwarded_without_request_extension() {
        let mut imp = banner("i");
        imp["ext"]["gpid"] = json!("scoped-gpid");
        imp["ext"]["tid"] = json!("scoped-tid");
        imp["ext"]["skadn"] = json!({"version":"1.0"});
        let request: BidRequest = serde_json::from_value(json!({"id":"r","site":{},"imp":[imp],
            "ext":{"prebid":{"multibid":[{"bidder":"rubicon","maxbids":2_i32},
                {"bidder":"rubicon","maxbids":7_i32}]}}}))
        .unwrap();
        let calls = outbound(&request);
        assert_eq!(calls.len(), 1);
        let body = body(&calls[0]);
        assert_eq!(body["imp"][0]["ext"]["gpid"], "scoped-gpid");
        assert_eq!(body["imp"][0]["ext"]["tid"], "scoped-tid");
        assert_eq!(body["imp"][0]["ext"]["skadn"], json!({"version":"1.0"}));
        assert_eq!(body["imp"][0]["ext"]["maxbids"], 2_i32);
        assert!(body.get("ext").is_none());
    }
}

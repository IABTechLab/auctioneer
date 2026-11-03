use crate::bidders::url::json_request_meta;
use crate::{BidderError, BidderRequestInfo, OutboundRequestSink};
use auctioneer_openrtb::types::bid_request::{BidRequest, BidRequestExt, Imp};
use serde::{
    Serialize, Serializer,
    ser::{SerializeMap as _, SerializeSeq as _},
};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    // `Imp.ext` is typed `Option<ImpExt>` per foundations. The exchange's
    // per-imp split + bidder-param merge stage folds the resolved
    // params into `imp.ext.bidder` (Option<serde_json::Value>), so
    // the bidder reads them directly via typed accessors — no
    // JSON re-parse.
    //
    // Both semantic failures below are resolved BEFORE the push, so
    // a request this bidder has no params for never reserves a
    // call slot, let alone allocates a body.
    let mut has_placement_ids = false;
    for imp in &request.imp {
        let Some(bidder_params) = imp.ext.as_ref().and_then(|ext| ext.bidder.as_ref()) else {
            continue;
        };
        if let Some(placement) = bidder_params.get("placementId") {
            placement
                .as_u64()
                .ok_or_else(|| vec![BidderError::bad_input("placementId must be a u64")])?;
            has_placement_ids = true;
        }
    }
    if !has_placement_ids {
        return Err(vec![BidderError::bad_input(
            "appnexus: at least one imp must carry imp.ext.bidder.placementId",
        )]);
    }

    // Route through the shared endpoint expander so any future
    // `{{.AccountID}}` / `{{.Host}}` substitutions in the AppNexus
    // manifest are validated against the codegen endpoint_macros
    // allow-list. The current AppNexus definition ships a static
    // endpoint with no macros, but the shared contract exposes
    // `endpoint_macros` / `host_allowlist` either way.
    let _outcome = sink.write_json(
        info,
        |meta| json_request_meta(meta, request, info),
        &AppnexusRequest(request),
    );
    Ok(())
}

/// Borrow every subtree; the sink's capped writer owns the only body allocation.
struct AppnexusRequest<'a>(&'a BidRequest);

impl Serialize for AppnexusRequest<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Exhaustive destructuring makes new typed fields a compile-time review point.
        let BidRequest {
            id,
            imp,
            site,
            app,
            dooh,
            device,
            user,
            regs,
            source,
            at,
            tmax,
            cur,
            test,
            ext,
            other,
        } = self.0;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        if !imp.is_empty() {
            map.serialize_entry("imp", imp)?;
        }
        if let Some(site_value) = site {
            map.serialize_entry("site", site_value)?;
        }
        if let Some(app_value) = app {
            map.serialize_entry("app", app_value)?;
        }
        if let Some(dooh_value) = dooh {
            map.serialize_entry("dooh", dooh_value)?;
        }
        if let Some(device_value) = device {
            map.serialize_entry("device", device_value)?;
        }
        if let Some(user_value) = user {
            map.serialize_entry("user", user_value)?;
        }
        if let Some(regs_value) = regs {
            map.serialize_entry("regs", regs_value)?;
        }
        if let Some(source_value) = source {
            map.serialize_entry("source", source_value)?;
        }
        if let Some(at_value) = at {
            map.serialize_entry("at", at_value)?;
        }
        if let Some(tmax_value) = tmax {
            map.serialize_entry("tmax", tmax_value)?;
        }
        if !cur.is_empty() {
            map.serialize_entry("cur", cur)?;
        }
        if let Some(test_value) = test {
            map.serialize_entry("test", test_value)?;
        }
        map.serialize_entry(
            "ext",
            &AppnexusExt {
                ext: ext.as_ref(),
                imp,
            },
        )?;
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

struct AppnexusExt<'a> {
    ext: Option<&'a BidRequestExt>,
    imp: &'a [Imp],
}

impl Serialize for AppnexusExt<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        if let Some(BidRequestExt { prebid, other }) = self.ext {
            if let Some(prebid_value) = prebid {
                map.serialize_entry("prebid", prebid_value)?;
            }
            for (key, value) in other {
                if key != "appnexus" {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        let fields = self
            .ext
            .and_then(|ext| ext.other.get("appnexus"))
            .and_then(serde_json::Value::as_object);
        map.serialize_entry(
            "appnexus",
            &AppnexusNamespace {
                fields,
                imp: self.imp,
            },
        )?;
        map.end()
    }
}

struct AppnexusNamespace<'a> {
    fields: Option<&'a serde_json::Map<String, serde_json::Value>>,
    imp: &'a [Imp],
}

impl Serialize for AppnexusNamespace<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        if let Some(fields) = self.fields {
            for (key, value) in fields {
                if key != "placement_ids" {
                    map.serialize_entry(key, value)?;
                }
            }
        }
        map.serialize_entry("placement_ids", &PlacementIds(self.imp))?;
        map.end()
    }
}

struct PlacementIds<'a>(&'a [Imp]);

impl Serialize for PlacementIds<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        // make_requests validates every placement before the sink can serialize this view.
        for imp in self.0 {
            if let Some(pid) = imp
                .ext
                .as_ref()
                .and_then(|ext| ext.bidder.as_ref())
                .and_then(|bidder| bidder.get("placementId"))
                .and_then(serde_json::Value::as_u64)
            {
                sequence.serialize_element(&pid)?;
            }
        }
        sequence.end()
    }
}

#[cfg(test)]
mod tests {
    use super::super::AppnexusBidder;
    use crate::bidders::harness;
    use crate::{
        Bidder as _, BidderError, BidderRequestInfo, CallBudget, EndpointRejectReason,
        EndpointValidator, OutboundBuildError, OutboundRequestSink, SinkLimits, SinkOutcome,
        bidder::EntryPoint, types::BidderName,
    };
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
    use auctioneer_openrtb::types::field::JsonField;

    fn imp_with_placement(id: &str, pid: u64) -> Imp {
        Imp {
            id: id.into(),
            ext: Some(ImpExt {
                bidder: JsonField::Value(serde_json::json!({ "placementId": pid })),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    fn info_for(template: &'static str) -> BidderRequestInfo<'static> {
        static EMPTY_MACROS: &[&str] = &[];
        static EMPTY_CURRENCY: &[String] = &[];
        BidderRequestInfo::new(
            BidderName::new("appnexus"),
            BidderName::new("appnexus"),
            "acct-x",
            None,
            template,
            EMPTY_MACROS,
            None,
            None,
            EntryPoint::Auction,
            EMPTY_CURRENCY,
        )
    }

    struct AllowAllEndpoints;

    impl EndpointValidator for AllowAllEndpoints {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    fn drive_with_cap(
        request: &BidRequest,
        cap: usize,
    ) -> (Result<(), Vec<BidderError>>, SinkOutcome, u32) {
        let budget = CallBudget::new(1);
        let mut sink =
            OutboundRequestSink::new(SinkLimits::new(cap, 100), &budget, &AllowAllEndpoints);
        let semantic = AppnexusBidder.make_requests(
            request,
            &info_for("https://ib.adnxs.com/openrtb2"),
            &mut sink,
        );
        let outcome = sink.into_outcome();
        (semantic, outcome, budget.remaining())
    }

    fn nested_unknown_tree() -> serde_json::Value {
        serde_json::json!({
            "branches": (0_u32..64_u32).map(|index| serde_json::json!({
                "index": index,
                "nested": [{"payload": "quoted\"\\\n".repeat(64), "flags": [true, null, false]}]
            })).collect::<Vec<_>>()
        })
    }

    fn request_with_all_fields() -> BidRequest {
        serde_json::from_value(serde_json::json!({
            "id": "auc-wire",
            "imp": [{
                "id": "i", "banner": {"w": 300_i32, "h": 250_i32, "ext": {"raw": [1_i32, 2_i32]}},
                "video": {"mimes": ["video/mp4"], "ext": {"raw": true}},
                "native": {"request": "{}", "ext": {"raw": null}},
                "audio": {"mimes": ["audio/mp4"]}, "bidfloor": 1.25_f64, "bidfloorcur": "USD",
                "secure": 1_i32, "unknown_imp": {"keep": [1_i32, 2_i32]},
                "ext": {"bidder": {"placementId": 11_u64}, "unknown_ext": {"keep": true}}
            }],
            "site": {"id": "site", "extra": [1_i32, 2_i32]},
            "app": {"id": "app", "extra": {"keep": true}},
            "dooh": {"id": "dooh", "extra": [3_i32]},
            "device": {"ua": "agent", "extra": {"keep": false}},
            "user": {
                "id": "user", "buyeruid": "buyer", "consent": "consent", "data": {"keep": true},
                "eids": [{"source": "example.test", "uids": [{"id": "uid", "atype": 1_i32}]}],
                "unknown_user": nested_unknown_tree(), "ext": {"unknown_ext": nested_unknown_tree()}
            },
            "regs": {"coppa": 0_i32, "extra": [4_i32]},
            "source": {"tid": "transaction", "extra": {"keep": null}},
            "at": 2_i32, "tmax": 125_i64, "cur": ["USD", "EUR"], "test": 0_i32,
            "ext": {
                "prebid": {
                    "aliases": {"alias": "appnexus"}, "debug": true,
                    "targeting": {"includewinners": false, "extra": [5_i32]},
                    "data": {"keep": [6_i32]}, "unknown_prebid": {"keep": true}
                },
                "other": nested_unknown_tree()
            },
            "unknown_request": nested_unknown_tree()
        }))
        .unwrap()
    }

    #[test]
    fn make_requests_emits_placement_ids_in_ext_appnexus() {
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![imp_with_placement("i-1", 11), imp_with_placement("i-2", 22)],
            ..BidRequest::default()
        };
        let info = info_for("https://ib.adnxs.com/openrtb2");
        let out = harness::drive_ok(&AppnexusBidder, &req, &info);
        let body: serde_json::Value = serde_json::from_slice(out[0].body()).unwrap();
        let ids = body["ext"]["appnexus"]["placement_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_u64().unwrap(), 11);
        assert_eq!(ids[1].as_u64().unwrap(), 22);
    }

    #[test]
    fn placement_transform_preserves_the_rest_of_the_wire_request() {
        for namespace in [
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!(17_i32)),
            Some(serde_json::json!(false)),
            Some(serde_json::json!("replace me")),
            Some(serde_json::json!([1_i32, 2_i32])),
            Some(serde_json::json!({})),
            Some(serde_json::json!({
                "keep": nested_unknown_tree(), "placement_ids": {"discard": nested_unknown_tree()}
            })),
        ] {
            let mut req = request_with_all_fields();
            if let Some(namespace_value) = namespace {
                req.ext
                    .as_mut()
                    .unwrap()
                    .other
                    .insert("appnexus".into(), namespace_value);
            }
            let original = serde_json::to_value(&req).unwrap();
            let mut expected = original.clone();
            if !expected["ext"]["appnexus"].is_object() {
                expected["ext"]["appnexus"] = serde_json::json!({});
            }
            expected["ext"]["appnexus"]["placement_ids"] = serde_json::json!([11_i32]);
            let (semantic, outcome, remaining) = drive_with_cap(&req, 1024 * 1024);
            semantic.unwrap();
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), 1);
            assert_eq!(remaining, 0);
            let actual: serde_json::Value =
                serde_json::from_slice(outcome.accepted[0].body()).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(serde_json::to_value(&req).unwrap(), original);
        }
    }

    #[test]
    fn placement_ids_preserve_order_duplicates_and_u64_boundaries_without_request_ext() {
        let req = BidRequest {
            id: "auc-ids".into(),
            imp: vec![
                imp_with_placement("zero", 0),
                Imp {
                    id: "no-ext".into(),
                    ..Imp::default()
                },
                Imp {
                    id: "no-bidder".into(),
                    ext: Some(ImpExt::default()),
                    ..Imp::default()
                },
                Imp {
                    id: "no-placement".into(),
                    ext: Some(ImpExt {
                        bidder: JsonField::Value(serde_json::json!({"other": 7_i32})),
                        ..ImpExt::default()
                    }),
                    ..Imp::default()
                },
                imp_with_placement("max", u64::MAX),
                imp_with_placement("duplicate", 0),
            ],
            ..BidRequest::default()
        };
        let original = serde_json::to_value(&req).unwrap();
        let out = harness::drive_ok(
            &AppnexusBidder,
            &req,
            &info_for("https://ib.adnxs.com/openrtb2"),
        );
        let actual: serde_json::Value = serde_json::from_slice(out[0].body()).unwrap();
        let mut expected = original.clone();
        expected["ext"] =
            serde_json::json!({"appnexus": {"placement_ids": [0_u64, u64::MAX, 0_u64]}});
        assert_eq!(actual, expected);
        assert_eq!(out[0].covered_imp_ids().count(), req.imp.len());
        assert_eq!(serde_json::to_value(&req).unwrap(), original);
    }

    #[test]
    fn oversized_transformed_request_is_rejected_without_mutating_the_caller() {
        let req = request_with_all_fields();
        let original = serde_json::to_value(&req).unwrap();
        let (semantic, outcome, remaining) = drive_with_cap(&req, 32);
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            vec![OutboundBuildError::OversizeRaw { limit: 32 }]
        );
        assert_eq!(remaining, 1);
        assert_eq!(serde_json::to_value(&req).unwrap(), original);
    }

    #[test]
    fn invalid_placement_ids_fail_before_tiny_body_caps_are_considered() {
        for invalid in [
            serde_json::Value::Null,
            serde_json::json!(-1_i32),
            serde_json::json!(1.5_f64),
            serde_json::json!("11"),
            serde_json::json!(true),
            serde_json::json!([]),
            serde_json::json!({}),
        ] {
            let mut req = request_with_all_fields();
            let mut invalid_imp = imp_with_placement("invalid", 11);
            invalid_imp.ext.as_mut().unwrap().bidder =
                JsonField::Value(serde_json::json!({"placementId": invalid}));
            req.imp.push(invalid_imp);
            let original = serde_json::to_value(&req).unwrap();
            let (semantic, outcome, remaining) = drive_with_cap(&req, 1);
            assert_eq!(
                semantic.unwrap_err(),
                vec![BidderError::bad_input("placementId must be a u64")]
            );
            assert!(outcome.accepted.is_empty());
            assert!(outcome.errors.is_empty());
            assert_eq!(remaining, 1);
            assert_eq!(serde_json::to_value(&req).unwrap(), original);
        }
    }

    #[test]
    fn missing_placement_ids_fail_before_tiny_body_caps_are_considered() {
        let mut req = request_with_all_fields();
        req.imp[0].ext.as_mut().unwrap().bidder =
            JsonField::Value(serde_json::json!({"other": 11_i32}));
        let (semantic, outcome, remaining) = drive_with_cap(&req, 1);
        assert_eq!(
            semantic.unwrap_err(),
            vec![BidderError::bad_input(
                "appnexus: at least one imp must carry imp.ext.bidder.placementId"
            )]
        );
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert_eq!(remaining, 1);
    }

    #[test]
    fn make_requests_rejects_when_no_placement_ids_present() {
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![Imp {
                id: "i-1".into(),
                ..Imp::default()
            }],
            ..BidRequest::default()
        };
        let info = info_for("https://ib.adnxs.com/openrtb2");
        let (semantic, _accepted) = harness::drive(&AppnexusBidder, &req, &info);
        let errs = semantic.expect_err("bidder reports a semantic failure");
        assert_eq!(errs[0].code, 2);
    }
}

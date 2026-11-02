use super::native::{native_eligible, validate_app_fields};
use super::parameters::{int, int8, known, object, optional_known, text};
use crate::BidderError;
use crate::correlation::CorrelationMedia;
use auctioneer_openrtb::types::{
    bid_request::{BidRequest, Imp, Video},
    field::JsonField,
};
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
pub(super) enum Failure {
    Parameters,
    Fields,
    Banner,
    Native,
    Floor,
    Unsupported,
}
impl Failure {
    pub(super) fn error(self) -> BidderError {
        BidderError::bad_input(match self {
            Self::Parameters => "rubicon invalid impression parameters",
            Self::Fields => "rubicon invalid request fields",
            Self::Banner => "rubicon impression requires banner dimensions",
            Self::Native => "rubicon invalid native request",
            Self::Floor => "rubicon floor conversion unavailable",
            Self::Unsupported => "rubicon no eligible impression",
        })
    }
}

pub(super) fn validate_request_fields(request: &BidRequest, imp: &Imp) -> Result<(), ()> {
    if let Some(badv) = known(&request.other, "badv")? {
        badv.as_array().ok_or(())?;
    }
    if let Some(site) = &request.site {
        let fields = site.ext.as_ref().map(|ext| &ext.other);
        object(optional_known(fields, "data")?)?;
    } else if let Some(app) = &request.app {
        if let Some(ext) = &app.ext {
            validate_app_fields(ext)?;
        }
    } else {
        return Err(());
    }
    if let Some(ext) = request.user.as_ref().and_then(|user| user.ext.as_ref()) {
        let rp = object(known(&ext.other, "rp")?)?;
        object(optional_known(rp, "target")?)?;
        object(ext.data.as_ref())?;
    }
    let ext = imp.ext.as_ref().ok_or(())?;
    let rp = object(known(&ext.other, "rp")?)?;
    object(optional_known(rp, "target")?)?;
    known(&ext.other, "skadn")?;
    Ok(())
}

pub(super) fn validate_conversion_fields(request: &BidRequest) -> Result<(), BidderError> {
    validate_rewarded_signals(request)?;
    validate_root_extensions(request)
        .map_err(|()| BidderError::bad_input("rubicon invalid request extension"))?;
    for imp in &request.imp {
        validate_impression_extension(imp)
            .map_err(|()| BidderError::bad_input("rubicon invalid impression extension"))?;
    }
    Ok(())
}

fn array(value: Option<&Value>) -> Result<&[Value], ()> {
    match value {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(_) => Err(()),
    }
}

fn chain_field<'a>(
    fields: Option<&'a Map<String, Value>>,
    name: &str,
    legacy: bool,
) -> Result<Option<&'a Value>, ()> {
    if legacy {
        optional_known(fields, name)
    } else {
        Ok(fields.and_then(|members| members.get(name)))
    }
}

fn validate_supply_chain(value: Option<&Value>, legacy: bool) -> Result<(), ()> {
    let chain = object(value)?;
    int8(chain_field(chain, "complete", legacy)?)?;
    text(chain_field(chain, "ver", legacy)?)?;
    for node_value in array(chain_field(chain, "nodes", legacy)?)? {
        let node = object(Some(node_value))?;
        for name in ["asi", "sid", "rid", "name", "domain"] {
            text(chain_field(node, name, legacy)?)?;
        }
        int8(chain_field(node, "hp", legacy)?)?;
    }
    Ok(())
}

fn validate_legacy_eids(value: Option<&Value>) -> Result<(), ()> {
    for eid_value in array(value)? {
        let eid = object(Some(eid_value))?;
        for name in ["source", "inserter", "matcher"] {
            text(optional_known(eid, name)?)?;
        }
        int(optional_known(eid, "mm")?)?;
        for uid_value in array(optional_known(eid, "uids")?)? {
            let uid = object(Some(uid_value))?;
            text(optional_known(uid, "id")?)?;
            int(optional_known(uid, "atype")?)?;
        }
    }
    Ok(())
}

fn validate_dsa(value: Option<&Value>) -> Result<(), ()> {
    let dsa = object(value)?;
    for name in ["dsarequired", "pubrender", "datatopub"] {
        int8(optional_known(dsa, name)?)?;
    }
    for transparency_value in array(optional_known(dsa, "transparency")?)? {
        let transparency = object(Some(transparency_value))?;
        text(optional_known(transparency, "domain")?)?;
        for parameter_value in array(optional_known(transparency, "dsaparams")?)? {
            int(Some(parameter_value))?;
        }
    }
    Ok(())
}

fn validate_root_extensions(request: &BidRequest) -> Result<(), ()> {
    let request_ext = request.ext.as_ref();
    validate_supply_chain(request_ext.and_then(|ext| ext.other.get("schain")), true)?;
    if let Some(source) = &request.source {
        validate_supply_chain(source.other.get("schain"), false)?;
        validate_supply_chain(
            source.ext.as_ref().and_then(|ext| ext.schain.as_ref()),
            true,
        )?;
    }
    validate_legacy_eids(
        request
            .user
            .as_ref()
            .and_then(|user| user.ext.as_ref())
            .and_then(|ext| ext.eids.as_ref()),
    )?;
    validate_dsa(
        request
            .regs
            .as_ref()
            .and_then(|regs| regs.ext.as_ref())
            .and_then(|ext| ext.dsa.as_ref()),
    )?;
    let multibid = request_ext
        .and_then(|ext| ext.prebid.as_ref())
        .and_then(|prebid| prebid.multibid.as_ref());
    let first = object(array(multibid)?.first())?;
    int(optional_known(first, "maxbids")?)?;
    Ok(())
}

fn validate_impression_extension(imp: &Imp) -> Result<(), ()> {
    let Some(ext) = &imp.ext else {
        return Ok(());
    };
    text(known(&ext.other, "gpid")?)?;
    let data = object(known(&ext.other, "data")?)?;
    text(optional_known(data, "pbadslot")?)?;
    let adserver = object(optional_known(data, "adserver")?)?;
    text(optional_known(adserver, "name")?)?;
    text(optional_known(adserver, "adslot")?)?;
    Ok(())
}

fn complete_video(video: &Video) -> bool {
    matches!(video.mimes, JsonField::Value(_))
        && matches!(video.other.get("protocols"), Some(Value::Array(_)))
        && video.maxduration.is_some_and(|value| value != 0_i32)
        && video
            .other
            .get("linearity")
            .and_then(Value::as_i64)
            .is_some_and(|value| value != 0)
}
pub(super) fn media(imp: &Imp, multi: bool) -> [Option<CorrelationMedia>; 3] {
    if multi {
        return [
            imp.banner.as_ref().map(|_| CorrelationMedia::Banner),
            imp.video.as_ref().map(|_| CorrelationMedia::Video),
            imp.native.as_ref().map(|_| CorrelationMedia::Native),
        ];
    }
    [
        if imp
            .video
            .as_ref()
            .is_some_and(|video| imp.banner.is_none() || complete_video(video))
        {
            Some(CorrelationMedia::Video)
        } else if imp.banner.is_some() {
            Some(CorrelationMedia::Banner)
        } else if imp.native.is_some() {
            Some(CorrelationMedia::Native)
        } else {
            None
        },
        None,
        None,
    ]
}
fn banner_dimensions(imp: &Imp) -> bool {
    imp.banner.as_ref().is_some_and(|banner| {
        !banner.format.is_empty()
            || banner.w.is_some_and(|n| n > 0_i32) && banner.h.is_some_and(|n| n > 0_i32)
    })
}
pub(super) fn media_eligibility(imp: &Imp, selected: CorrelationMedia) -> Result<(), Failure> {
    match selected {
        CorrelationMedia::Banner => banner_dimensions(imp).then_some(()).ok_or(Failure::Banner),
        CorrelationMedia::Native => imp
            .native
            .as_ref()
            .is_some_and(native_eligible)
            .then_some(())
            .ok_or(Failure::Native),
        CorrelationMedia::Video => Ok(()),
        CorrelationMedia::Audio => Err(Failure::Unsupported),
    }
}

fn validate_rewarded_signals(request: &BidRequest) -> Result<(), BidderError> {
    // Conversion is bidder-fatal, including a malformed later impression or
    // a legacy value shadowed by the standard field. Check before secret I/O.
    for imp in &request.imp {
        rewarded_inventory(imp)
            .map_err(|()| BidderError::bad_input("rubicon invalid rewarded inventory signal"))?;
    }
    Ok(())
}
pub(super) fn rewarded_inventory(imp: &Imp) -> Result<i8, ()> {
    let current = int8(imp.other.get("rwdd"))?;
    let legacy_fields = imp
        .ext
        .as_ref()
        .and_then(|ext| ext.prebid.as_ref())
        .map(|prebid| &prebid.other);
    let legacy = int8(optional_known(legacy_fields, "is_rewarded_inventory")?)?;
    Ok(if current == 0 { legacy } else { current })
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::bidders::harness;
    use crate::bidders::rubicon::test_support::{
        Transactions, assert_credential_eligibility, banner, body, context, mixed, outbound,
        request, settings,
    };
    use crate::bidders::rubicon::{RubiconBidder, tests as conformance};
    use crate::correlation::CorrelationMedia;
    use crate::{Bidder as _, BidderRequestInfo, FloorConversion, SinkLimits};
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::{Value, json};
    use std::cell::Cell;

    fn assert_impression_conversion_error(request: &BidRequest) {
        assert_conversion_error(request, "rubicon invalid impression extension");
    }

    fn request_with_root(root: &Value) -> BidRequest {
        let mut wire = json!({"id":"r", "site":{}, "imp":[banner("first"), banner("second")]});
        wire.as_object_mut()
            .unwrap()
            .extend(root.as_object().unwrap().clone());
        serde_json::from_value(wire).unwrap()
    }

    fn assert_root_conversion_error(root: &Value) {
        assert_conversion_error(
            &request_with_root(root),
            "rubicon invalid request extension",
        );
    }

    fn assert_conversion_error(request: &BidRequest, message: &str) {
        let settings = settings(&json!({
            "credentials_secret":"fake_credential_bundle","region":"us-east"
        }));
        let mut info = context();
        info.set_settings(Some("rubicon"), Some(&settings), None)
            .unwrap();
        let no_reference = context();
        let before = serde_json::to_value(request).unwrap();
        for supplied in [&no_reference, &info] {
            let error = RubiconBidder.credential_key(request, supplied).unwrap_err();
            assert_eq!(error.code, 2);
            assert_eq!(error.message, message);
        }
        assert!(info.credentials.is_none());
        conformance::with_source_context(|credential_info| {
            for supplied in [&no_reference, &info, credential_info] {
                assert_no_conversion_transaction(request, supplied, message);
            }
        });
        assert_eq!(serde_json::to_value(request).unwrap(), before);
    }

    fn assert_no_conversion_transaction(
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        message: &str,
    ) {
        let transactions = Transactions(Cell::new(0));
        let (result, outcome, remaining) = harness::drive_with_limits(
            &RubiconBidder,
            request,
            info,
            SinkLimits::new(256 * 1024, 2),
            2,
            &transactions,
        );
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 2);
        assert_eq!(errors[0].message, message);
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert_eq!(remaining, 2);
        assert_eq!(transactions.0.get(), 0);
    }

    fn assert_root_projection(root: &Value) -> Value {
        let request = request_with_root(root);
        let before = serde_json::to_value(&request).unwrap();
        assert_credential_eligibility(&request, FloorConversion::default(), true);
        let calls = outbound(&request);
        assert_eq!(calls.len(), 2);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        body(&calls[0])
    }

    fn chain_roots(chain: &Value) -> [Value; 3] {
        [
            json!({"source":{"schain":chain}}),
            json!({"source":{"ext":{"schain":chain}}}),
            json!({"ext":{"schain":chain}}),
        ]
    }

    fn root_member(root: &Value, parent: &str, name: &str, value: Value) -> Value {
        let mut updated = root.clone();
        updated
            .pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(name.to_owned(), value);
        updated
    }

    #[test]
    fn consumed_root_string_members_vary_type_null_and_case_independently() {
        let groups: [(Value, &str, &[&str], bool); 9] = [
            (
                json!({"source":{"schain":{}}}),
                "/source/schain",
                &["ver"],
                true,
            ),
            (
                json!({"source":{"schain":{"nodes":[{}]}}}),
                "/source/schain/nodes/0",
                &["asi", "sid", "rid", "name", "domain"],
                true,
            ),
            (json!({"ext":{"schain":{}}}), "/ext/schain", &["ver"], false),
            (
                json!({"ext":{"schain":{"nodes":[{}]}}}),
                "/ext/schain/nodes/0",
                &["asi", "sid", "rid", "name", "domain"],
                false,
            ),
            (
                json!({"source":{"ext":{"schain":{}}}}),
                "/source/ext/schain",
                &["ver"],
                false,
            ),
            (
                json!({"source":{"ext":{"schain":{"nodes":[{}]}}}}),
                "/source/ext/schain/nodes/0",
                &["asi", "sid", "rid", "name", "domain"],
                false,
            ),
            (
                json!({"user":{"ext":{"eids":[{}]}}}),
                "/user/ext/eids/0",
                &["source", "inserter", "matcher"],
                false,
            ),
            (
                json!({"user":{"ext":{"eids":[{"uids":[{}]}]}}}),
                "/user/ext/eids/0/uids/0",
                &["id"],
                false,
            ),
            (
                json!({"regs":{"ext":{"dsa":{"transparency":[{}]}}}}),
                "/regs/ext/dsa/transparency/0",
                &["domain"],
                false,
            ),
        ];
        for (root, parent, fields, canonical) in groups {
            assert_root_projection(&root);
            for field in fields {
                for value in [json!(null), json!(""), json!("value")] {
                    assert_root_projection(&root_member(&root, parent, field, value.clone()));
                    if !canonical {
                        assert_root_projection(&root_member(
                            &root,
                            parent,
                            &field.to_ascii_uppercase(),
                            value,
                        ));
                    }
                }
                assert_root_conversion_error(&root_member(&root, parent, field, json!(false)));
                let uppercase =
                    root_member(&root, parent, &field.to_ascii_uppercase(), json!(false));
                if canonical {
                    assert_root_projection(&uppercase);
                } else {
                    assert_root_conversion_error(&uppercase);
                    let lower = root_member(&root, parent, field, json!(null));
                    assert_root_conversion_error(&root_member(
                        &lower,
                        parent,
                        &field.to_ascii_uppercase(),
                        json!(null),
                    ));
                }
            }
        }
    }

    #[test]
    fn consumed_root_signed_byte_members_vary_bounds_null_and_case_independently() {
        let groups: [(Value, &str, &[&str], bool); 7] = [
            (
                json!({"source":{"schain":{}}}),
                "/source/schain",
                &["complete"],
                true,
            ),
            (
                json!({"source":{"schain":{"nodes":[{}]}}}),
                "/source/schain/nodes/0",
                &["hp"],
                true,
            ),
            (
                json!({"ext":{"schain":{}}}),
                "/ext/schain",
                &["complete"],
                false,
            ),
            (
                json!({"ext":{"schain":{"nodes":[{}]}}}),
                "/ext/schain/nodes/0",
                &["hp"],
                false,
            ),
            (
                json!({"source":{"ext":{"schain":{}}}}),
                "/source/ext/schain",
                &["complete"],
                false,
            ),
            (
                json!({"source":{"ext":{"schain":{"nodes":[{}]}}}}),
                "/source/ext/schain/nodes/0",
                &["hp"],
                false,
            ),
            (
                json!({"regs":{"ext":{"dsa":{}}}}),
                "/regs/ext/dsa",
                &["dsarequired", "pubrender", "datatopub"],
                false,
            ),
        ];
        for (root, parent, fields, canonical) in groups {
            for field in fields {
                for value in [json!(null), json!(-128_i32), json!(0_i32), json!(127_i32)] {
                    assert_root_projection(&root_member(&root, parent, field, value.clone()));
                    if !canonical {
                        assert_root_projection(&root_member(
                            &root,
                            parent,
                            &field.to_ascii_uppercase(),
                            value,
                        ));
                    }
                }
                for value in [
                    json!(-129_i32),
                    json!(128_i32),
                    json!(1.0_f64),
                    json!(false),
                ] {
                    assert_root_conversion_error(&root_member(&root, parent, field, value.clone()));
                    let uppercase = root_member(&root, parent, &field.to_ascii_uppercase(), value);
                    if canonical {
                        assert_root_projection(&uppercase);
                    } else {
                        assert_root_conversion_error(&uppercase);
                    }
                }
                if !canonical {
                    let lower = root_member(&root, parent, field, json!(null));
                    assert_root_conversion_error(&root_member(
                        &lower,
                        parent,
                        &field.to_ascii_uppercase(),
                        json!(null),
                    ));
                }
            }
        }
    }

    #[test]
    fn consumed_root_wide_integer_members_vary_bounds_null_and_case_independently() {
        for (root, parent, field, elements) in [
            (
                json!({"user":{"ext":{"eids":[{}]}}}),
                "/user/ext/eids/0",
                "mm",
                false,
            ),
            (
                json!({"user":{"ext":{"eids":[{"uids":[{}]}]}}}),
                "/user/ext/eids/0/uids/0",
                "atype",
                false,
            ),
            (
                json!({"regs":{"ext":{"dsa":{"transparency":[{}]}}}}),
                "/regs/ext/dsa/transparency/0",
                "dsaparams",
                true,
            ),
            (
                json!({"ext":{"prebid":{"multibid":[{}]}}}),
                "/ext/prebid/multibid/0",
                "maxbids",
                false,
            ),
        ] {
            for value in [json!(null), json!(i64::MIN), json!(0_i64), json!(i64::MAX)] {
                let member = if elements { json!([value]) } else { value };
                for name in [field.to_owned(), field.to_ascii_uppercase()] {
                    assert_root_projection(&root_member(&root, parent, &name, member.clone()));
                }
            }
            for value in [
                json!(false),
                json!(1.0_f64),
                json!("1"),
                json!([]),
                json!({}),
                json!(0x8000_0000_0000_0000_u64),
            ] {
                let member = if elements { json!([value]) } else { value };
                for name in [field.to_owned(), field.to_ascii_uppercase()] {
                    assert_root_conversion_error(&root_member(
                        &root,
                        parent,
                        &name,
                        member.clone(),
                    ));
                }
            }
            let lower = root_member(&root, parent, field, json!(null));
            assert_root_conversion_error(&root_member(
                &lower,
                parent,
                &field.to_ascii_uppercase(),
                json!(null),
            ));
        }
    }

    #[test]
    fn consumed_root_chain_types_are_checked_before_credentials_or_sink_work() {
        for malformed in [
            json!(false),
            json!(7_i32),
            json!("chain"),
            json!([]),
            json!({"complete":false}),
            json!({"complete":1.0_f64}),
            json!({"complete":-129_i32}),
            json!({"complete":128_i32}),
            json!({"ver":7_i32}),
            json!({"nodes":{}}),
            json!({"nodes":[false]}),
            json!({"nodes":[{"hp":-129_i32}]}),
            json!({"nodes":[{"hp":128_i32}]}),
            json!({"nodes":[{"hp":1.0_f64}]}),
        ] {
            for root in chain_roots(&malformed) {
                assert_root_conversion_error(&root);
            }
        }
        for field in ["asi", "sid", "rid", "name", "domain"] {
            let mut malformed = json!({"nodes":[{}]});
            malformed["nodes"][0][field] = json!(false);
            for root in chain_roots(&malformed) {
                assert_root_conversion_error(&root);
            }
        }
    }

    #[test]
    fn consumed_root_each_shadowed_chain_location_is_independently_checked() {
        let valid = json!({"complete":1_i32,"ver":"1.0","nodes":[]});
        let invalid = json!({"nodes":[{"hp":128_i32}]});
        for root in [
            json!({"source":{"schain":invalid,"ext":{"schain":valid}},"ext":{"schain":valid}}),
            json!({"source":{"schain":valid,"ext":{"schain":invalid}},"ext":{"schain":valid}}),
            json!({"source":{"schain":valid,"ext":{"schain":valid}},"ext":{"schain":invalid}}),
        ] {
            assert_root_conversion_error(&root);
        }
    }

    #[test]
    fn consumed_root_legacy_eid_and_uid_types_are_checked_even_when_shadowed() {
        for malformed in [
            json!(false),
            json!(7_i32),
            json!("eids"),
            json!({}),
            json!([false]),
            json!([{"mm":1.0_f64}]),
            json!([{"mm":0x8000_0000_0000_0000_u64}]),
            json!([{"uids":{}}]),
            json!([{"uids":[false]}]),
            json!([{"uids":[{"id":false}]}]),
            json!([{"uids":[{"atype":false}]}]),
            json!([{"uids":[{"atype":1.0_f64}]}]),
            json!([{"uids":[{"atype":0x8000_0000_0000_0000_u64}]}]),
        ] {
            for native in [json!([]), json!([{"source":"native","uids":[{"id":"id"}]}])] {
                assert_root_conversion_error(
                    &json!({"user":{"eids":native,"ext":{"eids":malformed}}}),
                );
            }
        }
        for field in ["source", "inserter", "matcher"] {
            let mut malformed = json!([{}]);
            malformed[0][field] = json!(false);
            assert_root_conversion_error(&json!({"user":{"ext":{"eids":malformed}}}));
        }
    }

    #[test]
    fn consumed_root_dsa_types_and_signed_byte_bounds_are_checked() {
        for malformed in [
            json!(false),
            json!(7_i32),
            json!("dsa"),
            json!([]),
            json!({"transparency":{}}),
            json!({"transparency":[false]}),
            json!({"transparency":[{"domain":false}]}),
            json!({"transparency":[{"dsaparams":{}}]}),
            json!({"transparency":[{"dsaparams":[false]}]}),
            json!({"transparency":[{"dsaparams":[1.0_f64]}]}),
            json!({"transparency":[{"dsaparams":[0x8000_0000_0000_0000_u64]}]}),
        ] {
            assert_root_conversion_error(&json!({"regs":{"ext":{"dsa":malformed}}}));
        }
        for field in ["dsarequired", "pubrender", "datatopub"] {
            for value in [
                json!(false),
                json!(1.0_f64),
                json!(-129_i32),
                json!(128_i32),
            ] {
                let mut malformed = json!({});
                malformed[field] = value;
                assert_root_conversion_error(&json!({"regs":{"ext":{"dsa":malformed}}}));
            }
        }
    }

    #[test]
    fn consumed_root_multibid_checks_the_array_and_first_maxbids_only() {
        for malformed in [
            json!(false),
            json!(7_i32),
            json!("multibid"),
            json!({}),
            json!([false]),
            json!([7_i32]),
            json!([[]]),
            json!([{"maxbids":false}]),
            json!([{"maxbids":1.0_f64}]),
            json!([{"maxbids":"2"}]),
            json!([{"maxbids":[]}]),
            json!([{"maxbids":{}}]),
            json!([{"maxbids":0x8000_0000_0000_0000_u64}]),
        ] {
            assert_root_conversion_error(&json!({"ext":{"prebid":{"multibid":malformed}}}));
        }
    }

    #[test]
    fn consumed_root_bad_values_precede_sink_transactions_without_credential_lookup() {
        for root in [
            json!({"source":{"schain":false}}),
            json!({"source":{"ext":{"schain":false}}}),
            json!({"ext":{"schain":false}}),
            json!({"user":{"ext":{"eids":false}}}),
            json!({"regs":{"ext":{"dsa":false}}}),
            json!({"ext":{"prebid":{"multibid":false}}}),
            json!({"ext":{"prebid":{"multibid":[{"maxbids":false}]}}}),
        ] {
            let request = request_with_root(&root);
            let before = serde_json::to_value(&request).unwrap();
            conformance::with_source_context(|info| {
                for supplied in [&context(), info] {
                    assert_no_conversion_transaction(
                        &request,
                        supplied,
                        "rubicon invalid request extension",
                    );
                }
            });
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn consumed_root_missing_null_and_empty_defaults_remain_valid() {
        assert_root_projection(&json!({}));
        for chain in [
            json!(null),
            json!({}),
            json!({"complete":null,"ver":null,"nodes":null}),
            json!({"nodes":[null,{}, {"asi":null,"sid":null,"rid":null,"name":null,"domain":null,"hp":null}]}),
        ] {
            for root in chain_roots(&chain) {
                assert_root_projection(&root);
            }
        }
        for eids in [
            json!(null),
            json!([]),
            json!([null,{}, {"source":null,"mm":null,"uids":null},
            {"uids":[null,{}, {"id":null,"atype":null}]}]),
        ] {
            assert_root_projection(&json!({"user":{"ext":{"eids":eids}}}));
        }
        for dsa in [
            json!(null),
            json!({}),
            json!({"dsarequired":null,"pubrender":null,"datatopub":null,
            "transparency":[null,{}, {"domain":null,"dsaparams":[null]}]}),
        ] {
            assert_root_projection(&json!({"regs":{"ext":{"dsa":dsa}}}));
        }
        for multibid in [
            json!(null),
            json!([]),
            json!([null, {"maxbids":false}]),
            json!([{}]),
            json!([{"maxbids":null}]),
        ] {
            let projected =
                assert_root_projection(&json!({"ext":{"prebid":{"multibid":multibid}}}));
            assert!(projected["imp"][0]["ext"].get("maxbids").is_none());
        }
    }

    #[test]
    fn consumed_root_valid_widths_and_opaque_extensions_are_forwarded_without_enum_checks() {
        let chain = json!({"complete":-128_i32,"ver":"1.0","nodes":[
            {"asi":"asi","sid":"sid","rid":"rid","name":"name","domain":"domain","hp":127_i32,"ext":false},
            {"hp":-128_i32}],"ext":[]});
        for root in chain_roots(&chain) {
            let projected = assert_root_projection(&root);
            assert_eq!(projected["source"]["ext"]["schain"], chain);
        }
        let eids = json!([{"source":"source","inserter":"inserter","matcher":"matcher","mm":i64::MIN,
            "uids":[{"id":"id","atype":i64::MAX,"ext":false},{"atype":i64::MIN}],"ext":[]}, {"mm":i64::MAX}]);
        let dsa = json!({"dsarequired":-128_i32,"pubrender":127_i32,"datatopub":2_i32,
            "transparency":[{"domain":"domain","dsaparams":[i64::MIN,i64::MAX]}],"unused":false});
        let projected = assert_root_projection(
            &json!({"user":{"ext":{"eids":eids}},"regs":{"ext":{"dsa":dsa}}}),
        );
        assert_eq!(projected["user"]["ext"]["eids"], eids);
        assert_eq!(projected["regs"]["ext"]["dsa"], dsa);
        for maxbids in [i64::MIN, 0, 10, i64::MAX] {
            let multibid_projection = assert_root_projection(
                &json!({"ext":{"prebid":{"storedrequest":false,"cache":7_i32,
                "multibid":[{"maxbids":maxbids,"bidder":false,"bidders":7_i32,"targetbiddercodeprefix":[]},
                    false, {"maxbids":[]} ]}}}),
            );
            assert_eq!(multibid_projection["imp"][0]["ext"]["maxbids"], maxbids);
        }
    }

    #[test]
    fn consumed_root_legacy_members_preserve_single_case_and_ambiguity_policy() {
        let chain = json!({"COMPLETE":1_i32,"VER":"1.0","NODES":[{"ASI":"asi","HP":1_i32}]});
        for root in [
            json!({"ext":{"schain":chain}}),
            json!({"source":{"ext":{"schain":chain}}}),
        ] {
            assert_root_projection(&root);
        }
        assert_root_projection(
            &json!({"user":{"ext":{"eids":[{"SOURCE":"source","MM":1_i32,
            "UIDS":[{"ID":"id","ATYPE":1_i32}]}]}},"regs":{"ext":{"dsa":{"DSAREQUIRED":1_i32,
                "TRANSPARENCY":[{"DOMAIN":"domain","DSAPARAMS":[1_i32]}]}}}}),
        );
        let projected =
            assert_root_projection(&json!({"ext":{"prebid":{"multibid":[{"MAXBIDS":2_i32}]}}}));
        assert_eq!(projected["imp"][0]["ext"]["maxbids"], 2_i32);
        for root in [
            json!({"ext":{"schain":{"complete":null,"COMPLETE":null}}}),
            json!({"source":{"ext":{"schain":{"nodes":[{"hp":null,"HP":null}]}}}}),
            json!({"user":{"ext":{"eids":[{"source":null,"SOURCE":null}]}}}),
            json!({"user":{"ext":{"eids":[{"uids":[{"id":null,"ID":null}]}]}}}),
            json!({"regs":{"ext":{"dsa":{"dsarequired":null,"DSAREQUIRED":null}}}}),
            json!({"regs":{"ext":{"dsa":{"transparency":[{"dsaparams":null,"DSAPARAMS":null}]}}}}),
            json!({"ext":{"prebid":{"multibid":[{"maxbids":null,"MAXBIDS":null}]}}}),
        ] {
            assert_root_conversion_error(&root);
        }
    }

    #[test]
    fn consumed_root_native_chain_members_use_only_canonical_case() {
        let chain = json!({"complete":1_i32,"COMPLETE":false,"nodes":[{"hp":1_i32,"HP":false}]});
        let projected = assert_root_projection(&json!({"source":{"schain":chain}}));
        assert_eq!(projected["source"]["ext"]["schain"], chain);
        assert_root_conversion_error(
            &json!({"source":{"schain":{"complete":false,"COMPLETE":1_i32}}}),
        );
    }

    #[test]
    fn credential_preflight_rejects_malformed_target_layers_and_rescues_valid_siblings() {
        let mut valid = banner("valid");
        valid["ext"]["rp"] = json!({"target":{}});
        assert_credential_eligibility(
            &request(&json!([valid.clone()])),
            FloorConversion::default(),
            true,
        );
        for malformed in [
            json!({"rp":{"target":[]}}),
            json!({"rp":[]}),
            json!({"rp":false}),
            json!({"rp":{"target":false}}),
        ] {
            let mut invalid = banner("invalid");
            invalid["ext"]
                .as_object_mut()
                .unwrap()
                .extend(malformed.as_object().unwrap().clone());
            for invalid_first in [false, true] {
                let imps = if invalid_first {
                    json!([invalid.clone(), valid.clone()])
                } else {
                    json!([valid.clone(), invalid.clone()])
                };
                assert_credential_eligibility(&request(&imps), FloorConversion::default(), true);
            }
            assert_credential_eligibility(
                &request(&json!([invalid])),
                FloorConversion::default(),
                false,
            );
        }
    }

    #[test]
    fn malformed_consumed_slot_data_is_bidder_fatal_before_credential_selection() {
        for malformed in [
            json!([]),
            json!(false),
            json!({"pbadslot": 7_i32}),
            json!({"adserver": []}),
            json!({"adserver": {"name": false}}),
            json!({"adserver": {"name": "other", "adslot": []}}),
            json!({"pbadslot": "scoped-slot", "adserver": {"name": "other", "adslot": []}}),
            json!({"pbadslot": "scoped-slot", "adserver": {"name": "gam", "adslot": []}}),
        ] {
            for invalid_parameters in [false, true] {
                let mut invalid = banner("invalid");
                invalid["ext"]["data"] = malformed.clone();
                if invalid_parameters {
                    invalid["ext"]["bidder"] = json!([]);
                }
                for invalid_first in [false, true] {
                    let valid = banner("valid");
                    let imps = if invalid_first {
                        json!([invalid, valid])
                    } else {
                        json!([valid, invalid])
                    };
                    assert_impression_conversion_error(&request(&imps));
                }
                assert_impression_conversion_error(&request(&json!([invalid])));
            }
        }
    }

    #[test]
    fn nonstring_gpid_is_bidder_fatal_even_on_an_otherwise_filtered_impression() {
        for value in [
            json!(false),
            json!(7_i32),
            json!(1.5_f64),
            json!([]),
            json!({}),
        ] {
            for invalid_parameters in [false, true] {
                let mut invalid = banner("invalid");
                invalid["ext"]["gpid"] = value.clone();
                if invalid_parameters {
                    invalid["ext"]["bidder"] = json!([]);
                }
                for invalid_first in [false, true] {
                    let valid = banner("valid");
                    let imps = if invalid_first {
                        json!([invalid, valid])
                    } else {
                        json!([valid, invalid])
                    };
                    assert_impression_conversion_error(&request(&imps));
                }
            }
        }
    }

    #[test]
    fn consumed_impression_extensions_accept_nulls_strings_and_unconsumed_data() {
        for gpid in [Value::Null, json!(""), json!("publisher/gpid")] {
            for data in [
                Value::Null,
                json!({}),
                json!({"pbadslot": null, "adserver": {"name": null, "adslot": null}}),
                json!({"pbadslot": "slot", "adserver": {"name": "gam", "adslot": "other"}, "unconsumed": false}),
            ] {
                let mut imp = banner("valid");
                imp["ext"]["gpid"] = gpid.clone();
                imp["ext"]["data"] = data;
                let request = request(&json!([imp]));
                assert_credential_eligibility(&request, FloorConversion::default(), true);
                let calls = outbound(&request);
                assert_eq!(calls.len(), 1);
                let projected = body(&calls[0]);
                if gpid.is_null() || gpid == json!("") {
                    assert!(projected["imp"][0]["ext"].get("gpid").is_none());
                } else {
                    assert_eq!(projected["imp"][0]["ext"]["gpid"], gpid);
                }
            }
        }
    }

    #[test]
    fn credential_eligibility_missing_inventory_cannot_be_rescued_by_impression_siblings() {
        for count in [1_usize, 2] {
            let imps = (0..count)
                .map(|index| banner(&format!("imp-{index}")))
                .collect::<Vec<_>>();
            let mut request = request(&json!(imps));
            request.site = None;
            assert!(request.app.is_none());
            assert_credential_eligibility(&request, FloorConversion::default(), false);
        }
    }

    #[test]
    fn credential_eligibility_each_inventory_branch_alone_admits_a_valid_impression() {
        let site_request = request(&json!([banner("valid")]));
        assert!(site_request.app.is_none());
        assert_credential_eligibility(&site_request, FloorConversion::default(), true);
        let mut app_request = request(&json!([banner("valid")]));
        app_request.site = None;
        app_request.app = Some(serde_json::from_value(json!({"id":"app"})).unwrap());
        assert_credential_eligibility(&app_request, FloorConversion::default(), true);
    }

    #[test]
    fn credential_eligibility_banner_without_formats_or_dimensions_is_rejected() {
        let mut invalid = banner("invalid");
        invalid["banner"] = json!({});
        assert_credential_eligibility(
            &request(&json!([invalid])),
            FloorConversion::default(),
            false,
        );
    }

    #[test]
    fn credential_eligibility_valid_banner_sibling_rescues_missing_banner_dimensions() {
        for invalid_first in [false, true] {
            let mut invalid = banner("invalid");
            invalid["banner"] = json!({});
            let valid = banner("valid");
            let imps = if invalid_first {
                json!([invalid, valid])
            } else {
                json!([valid, invalid])
            };
            assert_credential_eligibility(&request(&imps), FloorConversion::default(), true);
        }
        let mut explicit = banner("explicit");
        explicit["banner"] = json!({"w":300_i32,"h":250_i32});
        assert_credential_eligibility(
            &request(&json!([explicit])),
            FloorConversion::default(),
            true,
        );
    }

    #[test]
    fn credential_eligibility_split_video_can_rescue_its_invalid_banner() {
        let mut imp = mixed("mixed");
        imp["banner"] = json!({});
        imp.as_object_mut().unwrap().remove("native");
        imp["ext"]["bidder"]["bidonmultiformat"] = json!(true);
        assert_credential_eligibility(&request(&json!([imp])), FloorConversion::default(), true);
    }

    #[test]
    fn multiformat_splits_record_only_actual_banner_video_and_native_wire_media() {
        let mut imp = mixed("i");
        imp["ext"]["bidder"]["bidonmultiformat"] = json!(true);
        let request = request(&json!([imp]));
        let before = serde_json::to_value(&request).unwrap();
        let calls = outbound(&request);
        assert_eq!(calls.len(), 3);
        for (call, (selected, media)) in calls.iter().zip([
            ("banner", CorrelationMedia::Banner),
            ("video", CorrelationMedia::Video),
            ("native", CorrelationMedia::Native),
        ]) {
            let document = body(call);
            for key in ["banner", "video", "native", "audio"] {
                assert_eq!(document["imp"][0].get(key).is_some(), key == selected);
            }
            assert_eq!(call.meta().imp_correlation(0).unwrap().media(), media);
            assert_eq!(call.covered_imp_ids().collect::<Vec<_>>(), ["i"]);
        }
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn video_precedence_requires_each_recommended_field_only_when_a_banner_exists() {
        for missing in ["mimes", "protocols", "maxduration", "linearity"] {
            let mut imp = mixed("i");
            imp["video"].as_object_mut().unwrap().remove(missing);
            let calls = outbound(&request(&json!([imp.clone()])));
            assert_eq!(calls.len(), 1);
            assert!(body(&calls[0])["imp"][0].get("banner").is_some());
            assert!(body(&calls[0])["imp"][0].get("video").is_none());
            imp.as_object_mut().unwrap().remove("banner");
            let video_calls = outbound(&request(&json!([imp])));
            assert_eq!(video_calls.len(), 1);
            assert!(body(&video_calls[0])["imp"][0].get("video").is_some());
        }
        let calls = outbound(&request(&json!([mixed("i")])));
        assert_eq!(calls.len(), 1);
        assert!(body(&calls[0])["imp"][0].get("video").is_some());
    }

    #[test]
    fn malformed_rewarded_conversion_rejects_all_impressions_before_credentials() {
        let settings = settings(&json!({
            "credentials_secret":"fake_credential_bundle","region":"us-east"
        }));
        let mut info = context();
        info.set_settings(Some("rubicon"), Some(&settings), None)
            .unwrap();
        assert!(info.credentials.is_none());
        for value in [
            json!("bad"),
            json!(true),
            json!(1.0_f64),
            json!(128_i64),
            json!(-129_i64),
            json!([]),
            json!({}),
        ] {
            for standard in [false, true] {
                let mut invalid = banner("invalid");
                if standard {
                    invalid["rwdd"] = value.clone();
                } else {
                    invalid["ext"]["prebid"] = json!({"is_rewarded_inventory":value});
                    // Even a winning standard value must not hide a malformed
                    // legacy field: source conversion validates before choosing.
                    invalid["rwdd"] = json!(1_i32);
                }
                for invalid_first in [false, true] {
                    let valid = banner("valid");
                    let imps = if invalid_first {
                        json!([invalid, valid])
                    } else {
                        json!([valid, invalid])
                    };
                    let request = request(&imps);
                    let before = serde_json::to_value(&request).unwrap();
                    let error = RubiconBidder.credential_key(&request, &info).unwrap_err();
                    assert_eq!(error.code, 2);
                    assert_eq!(error.message, "rubicon invalid rewarded inventory signal");
                    conformance::with_source_context(|credential_info| {
                        for supplied in [&info, credential_info] {
                            let (result, calls) =
                                harness::drive(&RubiconBidder, &request, supplied);
                            assert!(calls.is_empty());
                            let errors = result.unwrap_err();
                            assert_eq!(errors.len(), 1);
                            assert_eq!(errors[0].code, 2);
                            assert_eq!(errors[0].message, error.message);
                        }
                    });
                    assert_eq!(serde_json::to_value(&request).unwrap(), before);
                }
            }
        }
    }

    #[test]
    fn rewarded_conversion_preserves_missing_null_and_signed_byte_boundaries() {
        for value in [
            None,
            Some(Value::Null),
            Some(json!(-128_i64)),
            Some(json!(127_i64)),
        ] {
            for standard in [false, true] {
                let mut imp = banner("valid");
                if let Some(present) = value.clone() {
                    if standard {
                        imp["rwdd"] = present;
                    } else {
                        imp["ext"]["prebid"] = json!({"is_rewarded_inventory":present});
                    }
                }
                let request = request(&json!([imp]));
                assert_credential_eligibility(&request, FloorConversion::default(), true);
                let calls = outbound(&request);
                assert_eq!(calls.len(), 1);
                if let Some(Value::Number(expected)) = &value {
                    assert_eq!(
                        body(&calls[0])["imp"][0]["rwdd"].as_i64(),
                        expected.as_i64()
                    );
                }
            }
        }
    }

    #[test]
    fn malformed_rewarded_signals_precede_every_sink_transaction() {
        conformance::with_source_context(|info| {
            for invalid_first in [false, true] {
                let mut invalid = banner("invalid");
                invalid["ext"]["prebid"] = json!({"is_rewarded_inventory":"bad"});
                let valid = banner("valid");
                let bad_request = request(&if invalid_first {
                    json!([invalid, valid])
                } else {
                    json!([valid, invalid])
                });
                let transactions = Transactions(Cell::new(0));
                let (result, outcome, remaining) = harness::drive_with_limits(
                    &RubiconBidder,
                    &bad_request,
                    info,
                    SinkLimits::new(256 * 1024, 2),
                    2,
                    &transactions,
                );
                assert_eq!(result.unwrap_err()[0].code, 2);
                assert!(outcome.accepted.is_empty());
                assert_eq!(remaining, 2);
                assert_eq!(transactions.0.get(), 0);

                let (positive_result, positive_outcome, positive_remaining) =
                    harness::drive_with_limits(
                        &RubiconBidder,
                        &request(&json!([banner("positive")])),
                        info,
                        SinkLimits::new(256 * 1024, 2),
                        2,
                        &transactions,
                    );
                positive_result.unwrap();
                assert_eq!(positive_outcome.accepted.len(), 1);
                assert_eq!(positive_remaining, 1);
                assert_eq!(transactions.0.get(), 1);
            }
        });
    }

    #[test]
    fn zero_or_null_standard_rewarded_flag_does_not_hide_a_valid_legacy_value() {
        for legacy in [-128_i32, 1_i32, 127_i32] {
            for current in [Value::Null, json!(0_i32)] {
                let mut imp = banner("valid");
                imp["rwdd"] = current;
                imp["ext"]["prebid"] = json!({"is_rewarded_inventory":legacy});
                let calls = outbound(&request(&json!([imp])));
                assert_eq!(calls.len(), 1);
                assert_eq!(body(&calls[0])["imp"][0]["rwdd"], legacy);
            }
        }
    }

    #[test]
    fn rewarded_extension_accepts_one_case_variant_and_refuses_ambiguous_keys() {
        for legacy in [Value::Null, json!(0_i32), json!(1_i32)] {
            let mut imp = banner("valid");
            imp["ext"]["prebid"] = json!({"Is_Rewarded_Inventory":legacy});
            let valid = request(&json!([imp]));
            assert_credential_eligibility(&valid, FloorConversion::default(), true);
            let calls = outbound(&valid);
            assert_eq!(calls.len(), 1);
            if legacy == json!(1_i32) {
                assert_eq!(body(&calls[0])["imp"][0]["rwdd"], 1_i32);
            }

            // Equal and null duplicates are still ambiguous, not an ordering rule.
            imp["ext"]["prebid"]["is_rewarded_inventory"] = legacy;
            let invalid = request(&json!([imp, banner("sibling")]));
            conformance::with_source_context(|info| {
                let error = RubiconBidder.credential_key(&invalid, info).unwrap_err();
                assert_eq!(error.code, 2);
                assert_eq!(error.message, "rubicon invalid rewarded inventory signal");
                let (result, refused_calls) = harness::drive(&RubiconBidder, &invalid, info);
                assert!(refused_calls.is_empty());
                let errors = result.unwrap_err();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].message, error.message);
            });
        }
    }
}

//! `imp.ext` — typed Imp extensions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::types::field::JsonField;

/// `imp.ext` (the typed envelope around imp's ext object).
///
/// Three meaningful shapes live here:
/// - `tid` — transaction id; scrubbed when `transmitTid` is denied.
/// - `prebid` — `ExtImpPrebid` (bidder params + per-bidder FPD).
/// - dynamic legacy `imp.ext.<bidderName>` top-level keys — bidder params
///   in their legacy 2.5 location; supplement source for `bidderparams`
///   merge.
/// - `bidder` — *written* by the exchange when hoisting.
#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct ImpExt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tid: Option<String>,
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub prebid: JsonField<ExtImpPrebid>,
    /// Top-level `imp.ext.bidder` — populated by the exchange's hoisting step.
    #[serde(default, skip_serializing_if = "JsonField::is_missing")]
    pub bidder: JsonField<serde_json::Value>,
    /// All other top-level keys, including legacy `imp.ext.<bidderName>`.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

#[derive(
    Clone, Debug, Default, Serialize, Deserialize, auctioneer_macros::OpenRtbFieldMetadata,
)]
#[openrtb_metadata(crate_path = crate::field_metadata)]
pub struct ExtImpPrebid {
    /// `imp.ext.prebid.bidder` — map of bidder name to per-imp params.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bidder: BTreeMap<String, serde_json::Value>,
    /// `imp.ext.prebid.imp` — bidder-specific FPD merged into the bidder-scoped `Imp`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub imp: BTreeMap<String, serde_json::Value>,
    /// Stored-request id — parsed but stored-requests are out of phase 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storedrequest: Option<serde_json::Value>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

// Preserve the previously funded inline layout on every compiled target.
const _: () = {
    use crate::types::bid_request::{Audio, Banner, Imp, Native, Video};
    use std::mem::{align_of, size_of};

    struct PreviousImpExt {
        _tid: Option<String>,
        _prebid: Option<ExtImpPrebid>,
        _bidder: JsonField<serde_json::Value>,
        _other: serde_json::Map<String, serde_json::Value>,
    }

    struct PreviousImp {
        _id: String,
        _banner: Option<Banner>,
        _video: Option<Video>,
        _native: Option<Native>,
        _audio: Option<Audio>,
        _bidfloor: Option<f64>,
        _bidfloorcur: Option<String>,
        _secure: Option<i32>,
        _ext: Option<PreviousImpExt>,
        _other: serde_json::Map<String, serde_json::Value>,
    }

    assert!(
        size_of::<JsonField<ExtImpPrebid>>() == size_of::<Option<ExtImpPrebid>>(),
        "prebid presence must preserve the previous field size"
    );
    assert!(
        align_of::<JsonField<ExtImpPrebid>>() == align_of::<Option<ExtImpPrebid>>(),
        "prebid presence must preserve the previous field alignment"
    );
    assert!(
        size_of::<ImpExt>() == size_of::<PreviousImpExt>(),
        "prebid presence must preserve the previous extension size"
    );
    assert!(
        align_of::<ImpExt>() == align_of::<PreviousImpExt>(),
        "prebid presence must preserve the previous extension alignment"
    );
    assert!(
        size_of::<Option<ImpExt>>() == size_of::<Option<PreviousImpExt>>(),
        "prebid presence must preserve the previous optional extension size"
    );
    assert!(
        align_of::<Option<ImpExt>>() == align_of::<Option<PreviousImpExt>>(),
        "prebid presence must preserve the previous optional extension alignment"
    );
    assert!(
        size_of::<Imp>() == size_of::<PreviousImp>(),
        "prebid presence must preserve the previous impression stride"
    );
    assert!(
        align_of::<Imp>() == align_of::<PreviousImp>(),
        "prebid presence must preserve the previous impression alignment"
    );
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::round_trip;
    use serde_json::error::Category;

    #[test]
    fn imp_ext_prebid_imp_fpd_round_trips() {
        let json = serde_json::json!({"prebid":{"imp":{"fake_bidder":{"banner":{"id":"X"}}}}});
        let ext: ImpExt = round_trip(&json);
        assert!(ext.prebid.as_ref().unwrap().imp.contains_key("fake_bidder"));
    }

    #[test]
    fn imp_ext_with_prebid_bidder_and_legacy_bidder_round_trips() {
        let json = serde_json::json!({
            "tid":"t1",
            "prebid":{"bidder":{"fake_bidder":{"testParam":1_i32}}},
            "fake_bidder":{"legacyParam":true}
        });
        let ext: ImpExt = round_trip(&json);
        assert_eq!(ext.tid.as_deref(), Some("t1"));
        assert!(
            ext.prebid
                .as_ref()
                .unwrap()
                .bidder
                .contains_key("fake_bidder")
        );
        assert!(ext.other.contains_key("fake_bidder"));
    }

    #[test]
    fn scoped_bidder_presence_survives_typed_round_trip() {
        for input in [
            serde_json::json!({}),
            serde_json::json!({"bidder": null}),
            serde_json::json!({"bidder": {}}),
            serde_json::json!({"bidder": false}),
            serde_json::json!({"bidder": []}),
        ] {
            let ext: ImpExt = serde_json::from_value(input.clone()).unwrap();
            assert_eq!(serde_json::to_value(&ext).unwrap(), input);
        }
    }

    #[test]
    fn prebid_presence_survives_typed_round_trip() {
        for input in [
            serde_json::json!({}),
            serde_json::json!({"prebid": null}),
            serde_json::json!({"prebid": {}}),
            serde_json::json!({"PREBID": null}),
            serde_json::json!({"prebid": null, "PREBID": null}),
            serde_json::json!({"prebid": null, "Prebid": {"adunitcode": "kept"}}),
            serde_json::json!({"prebid": {"bidder": {"fake_bidder": {"testParam": 1_i32}}}}),
        ] {
            let ext: ImpExt = round_trip(&input);
            assert_eq!(ext.prebid.is_missing(), input.get("prebid").is_none());
            assert_eq!(
                matches!(ext.prebid, JsonField::Null),
                input.get("prebid") == Some(&serde_json::Value::Null)
            );
            for alias in ["Prebid", "PREBID"] {
                assert_eq!(ext.other.get(alias), input.get(alias));
            }
        }
    }

    #[test]
    fn prebid_presence_rejects_nonobject_nonnull_canonical_values() {
        for value in [
            serde_json::json!(false),
            serde_json::json!(34_i32),
            serde_json::json!("invalid"),
            serde_json::json!([]),
        ] {
            let input = serde_json::json!({"prebid": value});
            let error = serde_json::from_value::<ImpExt>(input)
                .expect_err("a non-object prebid extension must be rejected");
            assert_eq!(error.classify(), Category::Data);
        }
    }

    #[test]
    fn prebid_presence_keeps_typed_child_metadata() {
        use crate::field_metadata::{FieldMetadata, Storage};

        let previous = <Option<ExtImpPrebid> as FieldMetadata>::MODEL.unwrap();
        let current = <JsonField<ExtImpPrebid> as FieldMetadata>::MODEL.unwrap();
        assert!(previous.same_declaration(current));
        let field = <ImpExt as FieldMetadata>::MODEL
            .unwrap()
            .fields
            .iter()
            .find(|field| field.name == "prebid")
            .unwrap();
        assert_eq!(field.storage, Storage::Typed);
        assert!(!field.flattened);
        assert!(field.child.unwrap().same_declaration(current));
    }
}

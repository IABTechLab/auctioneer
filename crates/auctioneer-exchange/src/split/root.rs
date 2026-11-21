//! Select recipient root fields from the canonical typed request before copying.

use crate::{alias::AliasMap, error::AuctionError};
use auctioneer_bidder::BidderName;
use serde_json::{Map, Value};

pub(super) fn project(
    request: &Value,
    bidder: &BidderName,
    aliases: &AliasMap,
) -> Result<Map<String, Value>, AuctionError> {
    let source = request
        .as_object()
        .ok_or_else(|| AuctionError::Internal("grouped request is not an object".into()))?;
    let mut root = Map::new();
    for (key, value) in source {
        match key.as_str() {
            "imp" => {}
            user if user.eq_ignore_ascii_case("user") => {
                root.insert(key.clone(), project_user(value));
            }
            "ext" => {
                root.insert(key.clone(), project_ext(value, bidder, aliases));
            }
            _ => {
                root.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(root)
}

// Visit only the fixed user/ext topology, never attacker-selected input depth.
// Match ext ASCII-case-insensitively, preserving each spelling independently.
fn project_user(source: &Value) -> Value {
    let Some(members) = source.as_object() else {
        return source.clone();
    };
    Value::Object(
        members
            .iter()
            .filter_map(|(key, value)| {
                let projected = if key.eq_ignore_ascii_case("ext") {
                    project_user_ext(value)?
                } else {
                    value.clone()
                };
                Some((key.clone(), projected))
            })
            .collect(),
    )
}

// Skip exact prebid and any extension it empties before copying either. A caller's
// empty extension and any nonempty remainder (including Prebid) remain unchanged.
fn project_user_ext(source: &Value) -> Option<Value> {
    let Some(members) = source.as_object() else {
        return Some(source.clone());
    };
    if members.len() == 1 && members.contains_key("prebid") {
        return None;
    }
    Some(Value::Object(
        members
            .iter()
            .filter(|(key, _)| key.as_str() != "prebid")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    ))
}

fn project_ext(source: &Value, bidder: &BidderName, aliases: &AliasMap) -> Value {
    let Some(members) = source.as_object() else {
        // Normal grouping always serializes a typed extension object.
        // Preserve a corrupt non-object for the existing typed decode.
        return source.clone();
    };
    let mut ext = Map::new();
    for (key, value) in members {
        let projected = if key == "prebid" {
            project_prebid(value, bidder, aliases)
        } else {
            value.clone()
        };
        ext.insert(key.clone(), projected);
    }
    Value::Object(ext)
}

fn project_prebid(source: &Value, bidder: &BidderName, aliases: &AliasMap) -> Value {
    let Some(members) = source.as_object() else {
        // Keep corrupt private values for the same decode path as root ext.
        return source.clone();
    };
    let mut prebid = Map::new();
    for (key, value) in members {
        match key.as_str() {
            "bidderparams" => {
                if let Some(params) = value.get(bidder.as_str()).and_then(Value::as_object)
                    && !params.is_empty()
                {
                    prebid.insert(key.clone(), Value::Object(params.clone()));
                }
            }
            "aliases" => {
                if let Some(base) = aliases.bases.get(bidder)
                    && value.get(bidder.as_str()).and_then(Value::as_str) == Some(base.as_str())
                {
                    let selected = Map::from_iter([(
                        bidder.as_str().to_owned(),
                        Value::String(base.as_str().to_owned()),
                    )]);
                    prebid.insert(key.clone(), Value::Object(selected));
                }
            }
            "alternatebiddercodes"
            | "channel"
            | "currency"
            | "debug"
            | "integration"
            | "multibid"
            | "sdk"
            | "server"
            | "targeting" => {
                prebid.insert(key.clone(), value.clone());
            }
            _ => {}
        }
    }
    Value::Object(prebid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::json;

    fn round_trip_root(extension: Option<Value>, aliases: &AliasMap, expected_ext: Option<Value>) {
        let mut source = json!({
            "id": "recipient-presence", "site": {"id": "fictional-site"},
            "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32}}],
            "source": {"ext": {"schain": {"ver": "1.0", "complete": 1_i32}}}
        });
        if let Some(input_ext) = extension {
            source["ext"] = input_ext;
        }
        let typed: BidRequest = serde_json::from_value(source).unwrap();
        typed.validate().unwrap();
        let canonical = serde_json::to_value(&typed).unwrap();
        let before = serde_json::to_vec(&canonical).unwrap();
        let mut projected = project(&canonical, &BidderName::new("fake_selected/~"), aliases)
            .expect("canonical root is an object");
        assert!(!projected.contains_key("imp"));
        projected.insert("imp".into(), canonical["imp"].clone());
        let result: BidRequest = serde_json::from_value(Value::Object(projected)).unwrap();
        result.validate().unwrap();
        assert_eq!(result.ext.is_some(), expected_ext.is_some());
        assert_eq!(
            result.ext.as_ref().is_some_and(|ext| ext.prebid.is_some()),
            expected_ext
                .as_ref()
                .is_some_and(|ext| ext.get("prebid").is_some())
        );
        let mut expected = canonical.clone();
        if let Some(ext) = expected_ext {
            expected["ext"] = ext;
        }
        assert_eq!(serde_json::to_value(result).unwrap(), expected);
        assert_eq!(serde_json::to_vec(&canonical).unwrap(), before);
    }

    #[test]
    fn extension_presence_is_not_manufactured_by_host_aliases() {
        let mut aliases = AliasMap::default();
        aliases.bases.insert(
            BidderName::new("fake_selected/~"),
            BidderName::new("fake_base"),
        );
        for (input, expected) in [
            (None, None),
            (Some(json!({})), Some(json!({}))),
            (
                Some(json!({"ordinary": {"keep": true}})),
                Some(json!({"ordinary": {"keep": true}})),
            ),
            (Some(json!({"prebid": {}})), Some(json!({"prebid": {}}))),
            (
                Some(json!({"prebid": {"data": {"private": true},
                    "bidderconfig": [{"private": true}], "returnallbidstatus": true}})),
                Some(json!({"prebid": {}})),
            ),
        ] {
            round_trip_root(input, &aliases, expected);
        }
    }

    #[test]
    fn recipient_parameter_shapes_and_debug_forms_round_trip_independently() {
        for params in [
            None,
            Some(Value::Null),
            Some(json!("private-scalar")),
            Some(json!(7_i32)),
            Some(json!(true)),
            Some(json!([{"private": true}])),
            Some(json!({})),
            Some(json!({"own": "selected", "nested": {"keep": true}})),
        ] {
            for debug in [json!(0_i32), json!(1_i32), json!(false), json!(true)] {
                let mut prebid = json!({"debug": debug, "bidderparams": {
                    "fake_base": {"private": "no-base-fallback"},
                    "fake_competitor": {"private": "competitor"}
                }});
                if let Some(selected_params) = &params {
                    prebid["bidderparams"]["fake_selected/~"] = selected_params.clone();
                }
                let mut expected = json!({"debug": debug});
                if params
                    .as_ref()
                    .is_some_and(|value| value.as_object().is_some_and(|object| !object.is_empty()))
                {
                    expected["bidderparams"] = json!({"own": "selected", "nested": {"keep": true}});
                }
                let mut aliases = AliasMap::default();
                aliases.bases.insert(
                    BidderName::new("fake_selected/~"),
                    BidderName::new("fake_base"),
                );
                round_trip_root(
                    Some(json!({"ordinary": "context", "prebid": prebid})),
                    &aliases,
                    Some(json!({"ordinary": "context", "prebid": expected})),
                );
            }
        }
    }

    #[test]
    fn only_an_accepted_matching_request_alias_is_forwarded() {
        for accepted_base in [None, Some("fake_base"), Some("fake_other_base")] {
            let mut aliases = AliasMap::default();
            if let Some(base) = accepted_base {
                aliases
                    .bases
                    .insert(BidderName::new("fake_selected/~"), BidderName::new(base));
            }
            let mut expected = json!({});
            if accepted_base == Some("fake_base") {
                expected["aliases"] = json!({"fake_selected/~": "fake_base"});
            }
            round_trip_root(
                Some(json!({"prebid": {"aliases": {
                    "fake_selected/~": "fake_base", "fake_competitor/~": "fake_other_base"
                }}})),
                &aliases,
                Some(json!({"prebid": expected})),
            );
        }
    }

    #[test]
    fn non_object_grouping_keeps_the_fixed_internal_error() {
        for source in [Value::Null, json!([]), json!("private-root")] {
            let error =
                project(&source, &BidderName::new("fake_base"), &AliasMap::default()).unwrap_err();
            assert!(matches!(error, AuctionError::Internal(message)
                if message == "grouped request is not an object"));
        }
    }

    #[test]
    fn user_prebid_is_removed_without_mutating_the_source() {
        for reserved in [
            json!({"fake_selected": "own-map-canary", "fake_other": "other-map-canary"}),
            Value::Null,
            json!("scalar-map-canary"),
            json!(17_i32),
            json!(true),
            json!(["array-map-canary", {"nested": "other-map-canary"}]),
        ] {
            let source = json!({"id": "uid-projection", "user": {
                "id": "ordinary-id", "buyeruid": "explicit-hint", "keywords": "context",
                "buyeruids": "ordinary-user-sibling", "ext": {
                    "buyeruids": "ordinary-ext-sibling", "data": {"keep": true},
                    "prebid": {"buyeruids": reserved, "keep": {"buyeruids": "nested-context"}}
                }
            }});
            assert_eq!(
                source["user"]["ext"]["prebid"].get("buyeruids"),
                Some(&reserved)
            );
            let before = serde_json::to_vec(&source).unwrap();
            let actual = Value::Object(
                project(
                    &source,
                    &BidderName::new("fake_selected"),
                    &AliasMap::default(),
                )
                .unwrap(),
            );
            assert_eq!(
                actual,
                json!({"id": "uid-projection", "user": {
                    "id": "ordinary-id", "buyeruid": "explicit-hint", "keywords": "context",
                    "buyeruids": "ordinary-user-sibling", "ext": {
                        "buyeruids": "ordinary-ext-sibling", "data": {"keep": true}
                    }
                }})
            );
            assert_eq!(serde_json::to_vec(&source).unwrap(), before);
        }
    }

    #[test]
    fn removing_prebid_drops_only_its_emptied_extension() {
        let mut checked = 0_usize;
        for (user, extension) in [("user", "ext"), ("User", "EXT")] {
            for prebid in [
                json!({}),
                json!({"buyeruids": {}}),
                json!({"buyeruids": {"fake_selected": "uid-canary"}}),
                json!({"keep": "sibling-only"}),
                Value::Null,
                json!([]),
                json!(["malformed-prebid"]),
                json!("malformed-prebid"),
                json!(7_i32),
                json!(true),
            ] {
                let source = json!({(user): {(extension): {"prebid": prebid}}});
                assert_eq!(source[user][extension].get("prebid"), Some(&prebid));
                let before = serde_json::to_vec(&source).unwrap();
                let actual = Value::Object(
                    project(
                        &source,
                        &BidderName::new("fake_selected"),
                        &AliasMap::default(),
                    )
                    .unwrap(),
                );
                assert_eq!(actual, json!({(user): {}}));
                assert_eq!(serde_json::to_vec(&source).unwrap(), before);
                checked += 1;
            }
        }
        assert_eq!(checked, 20);
    }

    #[test]
    fn extension_remainders_and_caller_empty_extensions_are_preserved() {
        let mut checked = 0_usize;
        for (user, source, expected) in [
            (
                "user",
                json!({"user": {"ext": {"prebid": {}, "Prebid": {"keep": true}}}}),
                json!({"user": {"ext": {"Prebid": {"keep": true}}}}),
            ),
            (
                "user",
                json!({"user": {
                    "ext": {"prebid": {}}, "Ext": {"prebid": {}, "ordinary": "kept"}
                }}),
                json!({"user": {"Ext": {"ordinary": "kept"}}}),
            ),
            (
                "user",
                json!({"user": {
                    "ext": {"prebid": {}, "ordinary": "kept"}, "Ext": {"prebid": {}}
                }}),
                json!({"user": {"ext": {"ordinary": "kept"}}}),
            ),
            (
                "user",
                json!({"user": {"ext": {}, "Ext": {"prebid": {}}}}),
                json!({"user": {"ext": {}}}),
            ),
            (
                "user",
                json!({"user": {"ext": {"prebid": {}}, "Ext": {}}}),
                json!({"user": {"Ext": {}}}),
            ),
            (
                "User",
                json!({"User": {"EXT": {"prebid": {}, "Prebid": null}}}),
                json!({"User": {"EXT": {"Prebid": null}}}),
            ),
        ] {
            assert!(
                source[user]
                    .as_object()
                    .unwrap()
                    .values()
                    .any(|value| value.get("prebid").is_some())
            );
            let before = serde_json::to_vec(&source).unwrap();
            let actual = Value::Object(
                project(
                    &source,
                    &BidderName::new("fake_selected"),
                    &AliasMap::default(),
                )
                .unwrap(),
            );
            assert_eq!(actual, expected);
            assert_eq!(serde_json::to_vec(&source).unwrap(), before);
            checked += 1;
        }
        assert_eq!(checked, 6);
    }

    #[test]
    fn all_uid_leaf_spellings_are_contained_by_prebid_removal() {
        let member_sets: &[&[&str]] = &[
            &["BuyerUIDs"],
            &["buyerUids"],
            &["BUYERUIDS"],
            &["buyeruids", "BuyerUIDs", "buyerUids", "BUYERUIDS"],
        ];
        let expected = json!({"user": {
            "id": "ordinary-id", "buyeruid": "explicit-hint",
            "buyeruids": "user-sibling", "ext": {
                "BuyerUIDs": "extension-sibling",
                "Prebid": {"buyeruids": "different-prefix"},
                "prebidx": {"buyeruids": "near-miss"}
            }
        }});
        let mut checked = 0_usize;
        for &members in member_sets {
            for reserved in [
                json!({"fake_first": "first-canary", "fake_second": "second-canary"}),
                Value::Null,
                json!("scalar-canary"),
                json!([{"nested": "array-canary"}]),
            ] {
                let mut source = expected.clone();
                source["user"]["ext"]["prebid"] = json!({
                    "buyeruidsx": "contained-near-miss", "nested": {"BUYERUIDS": "contained-context"}
                });
                let prebid = source["user"]["ext"]["prebid"].as_object_mut().unwrap();
                for &member in members {
                    assert!(prebid.insert(member.to_owned(), reserved.clone()).is_none());
                }
                for &member in members {
                    assert_eq!(prebid.get(member), Some(&reserved));
                }
                let before = serde_json::to_vec(&source).unwrap();
                let actual = Value::Object(
                    project(
                        &source,
                        &BidderName::new("fake_selected"),
                        &AliasMap::default(),
                    )
                    .unwrap(),
                );
                assert_eq!(
                    actual, expected,
                    "members={members:?} reserved={reserved:?}"
                );
                assert_eq!(serde_json::to_vec(&source).unwrap(), before);
                checked += 1;
            }
        }
        assert_eq!(checked, 16);
    }

    #[test]
    fn every_user_and_extension_spelling_loses_only_exact_prebid() {
        let paths: &[(&[&str], &[&str])] = &[
            (&["user"], &["ext"]),
            (&["User"], &["ext"]),
            (&["USER"], &["ext"]),
            (&["user"], &["Ext"]),
            (&["user"], &["EXT"]),
            (&["uSeR"], &["eXt"]),
            (&["user", "User"], &["ext", "Ext"]),
        ];
        let mut checked = 0_usize;
        for &(users, extensions) in paths {
            let mut expected = json!({"userx": {"ext": {"prebid": "root-near-miss"}}});
            for &user in users {
                expected[user] = json!({
                    "buyeruid": "explicit-hint", "extx": {"prebid": "extension-near-miss"}
                });
                for &extension in extensions {
                    expected[user][extension] = json!({
                        "ordinary": "context",
                        "Prebid": {"buyeruids": "case-control"},
                        "prebidx": {"buyeruids": "member-near-miss"}
                    });
                }
            }
            let mut source = expected.clone();
            for &user in users {
                for &extension in extensions {
                    let members = source[user][extension].as_object_mut().unwrap();
                    assert!(
                        members
                            .insert(
                                "prebid".into(),
                                json!({"buyeruids": {"fake_other": "uid-canary"}, "keep": "contained"}),
                            )
                            .is_none()
                    );
                    assert!(members.contains_key("prebid"));
                }
            }
            let before = serde_json::to_vec(&source).unwrap();
            let actual = Value::Object(
                project(
                    &source,
                    &BidderName::new("fake_selected"),
                    &AliasMap::default(),
                )
                .unwrap(),
            );
            assert_eq!(
                actual, expected,
                "users={users:?} extensions={extensions:?}"
            );
            assert_eq!(serde_json::to_vec(&source).unwrap(), before);
            checked += 1;
        }
        assert_eq!(checked, 7);
    }

    #[test]
    fn absent_prebid_preserves_ancestor_presence_and_non_object_values() {
        for source in [
            json!({"id": "no-user"}),
            json!({"user": null}),
            json!({"user": "invalid-user"}),
            json!({"user": {}}),
            json!({"user": {"buyeruid": "explicit-hint"}}),
            json!({"user": {"ext": null}}),
            json!({"user": {"ext": []}}),
            json!({"user": {"ext": {}}}),
            json!({"User": null}),
            json!({"USER": "invalid-user"}),
            json!({"user": {"Ext": null}}),
            json!({"User": {"EXT": []}}),
        ] {
            let before = serde_json::to_vec(&source).unwrap();
            let actual = Value::Object(
                project(
                    &source,
                    &BidderName::new("fake_selected"),
                    &AliasMap::default(),
                )
                .unwrap(),
            );
            assert_eq!(actual, source);
            assert_eq!(serde_json::to_vec(&source).unwrap(), before);
        }
    }

    #[test]
    fn malformed_extensions_preserve_the_existing_typed_decode_error() {
        for malformed in [
            json!("private-extension"),
            json!(7_i32),
            json!(false),
            json!([false]),
        ] {
            for nested in [false, true] {
                let extension = if nested {
                    json!({"ordinary": "keep", "prebid": malformed})
                } else {
                    malformed.clone()
                };
                let source = json!({
                    "id": "malformed-extension", "site": {"id": "fictional-site"},
                    "imp": [{"id": "imp-1", "banner": {"w": 300_i32, "h": 250_i32}}],
                    "ext": extension
                });
                let expected_error = serde_json::from_value::<BidRequest>(source.clone())
                    .expect_err("the original extension is not a typed object");
                let mut projected =
                    project(&source, &BidderName::new("fake_base"), &AliasMap::default()).unwrap();
                assert_eq!(projected["ext"], extension, "nested={nested}");
                projected.insert("imp".into(), source["imp"].clone());
                let actual_error = serde_json::from_value::<BidRequest>(Value::Object(projected))
                    .expect_err("projection must not normalize the corrupt extension");
                assert_eq!(actual_error.to_string(), expected_error.to_string());
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn measured_root(source: &Value) -> (Value, u64, u64) {
        use auctioneer_runtime::test_support::alloc_track;

        let bidder = BidderName::new("fake_selected");
        let aliases = AliasMap::default();
        let _warm = alloc_track::allocated();
        let baseline = alloc_track::reset_peak();
        let before = alloc_track::allocated();
        let result = Value::Object(project(source, &bidder, &aliases).unwrap());
        let requested = alloc_track::allocated().checked_sub(before).unwrap();
        let peak = alloc_track::peak_since(baseline);
        (result, requested, peak)
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn omitted_prebid_nodes_do_not_increase_projection_allocations() {
        for container in [false, true] {
            let mut observations = Vec::new();
            for count in [1_usize, 128, 2_048] {
                let reserved = if container {
                    json!(vec![json!({"nested": ["uid-canary", {}]}); count])
                } else {
                    Value::Object(
                        (0..count)
                            .map(|index| (format!("fake_{index}"), json!("uid-canary")))
                            .collect(),
                    )
                };
                let source = json!({"user": {"buyeruid": "explicit-hint", "ext": {
                    "prebid": {"buyeruids": reserved, "keep": "context"}
                }}});
                assert!(source["user"]["ext"]["prebid"].get("buyeruids").is_some());
                let (result, requested, peak) = measured_root(&source);
                assert_eq!(result, json!({"user": {"buyeruid": "explicit-hint"}}));
                observations.push((requested, peak));
            }
            assert_eq!(observations.len(), 3);
            assert!(observations.iter().all(|value| value == &observations[0]));
        }
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn omitted_user_prebid_payloads_are_not_cloned() {
        let mut checked = 0_usize;
        for (user, extension, keep_ordinary) in [
            ("user", "ext", true),
            ("User", "ext", true),
            ("USER", "ext", true),
            ("user", "Ext", true),
            ("user", "EXT", true),
            ("User", "EXT", true),
            ("user", "ext", false),
            ("User", "EXT", false),
        ] {
            for sibling_only in [false, true] {
                let mut observations = Vec::with_capacity(3);
                let expected = if keep_ordinary {
                    json!({(user): {
                        "buyeruid": "explicit-hint", (extension): {"ordinary": "kept"}
                    }})
                } else {
                    json!({(user): {"buyeruid": "explicit-hint"}})
                };
                for bytes in [128_usize, 1_024, 8_192] {
                    let payload = "x".repeat(bytes);
                    let prebid = if sibling_only {
                        json!({"ordinary": payload})
                    } else {
                        json!({"buyeruids": {"fake_other": payload}, "keep": "contained"})
                    };
                    let mut source = json!({(user): {
                        "buyeruid": "explicit-hint", (extension): {"prebid": prebid}
                    }});
                    if keep_ordinary {
                        source[user][extension]["ordinary"] = json!("kept");
                    }
                    assert_eq!(source[user][extension].get("prebid"), Some(&prebid));
                    let (result, requested, peak) = measured_root(&source);
                    assert_eq!(result, expected);
                    observations.push((requested, peak));
                }
                assert_eq!(observations.len(), 3);
                assert!(observations.iter().all(|value| value == &observations[0]));
                checked += 1;
            }
        }
        assert_eq!(checked, 16);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn omitted_root_payloads_are_not_cloned() {
        #[derive(Debug)]
        enum Omitted {
            CompetitorParams,
            BidderConfig,
            BuyerUids(&'static str),
        }

        for omitted in [
            Omitted::CompetitorParams,
            Omitted::BidderConfig,
            Omitted::BuyerUids("buyeruids"),
            Omitted::BuyerUids("BuyerUIDs"),
            Omitted::BuyerUids("BUYERUIDS"),
        ] {
            let mut observations = Vec::with_capacity(3);
            let mut expected = json!({
                "id": "allocation-root", "ext": {"ordinary": "keep", "prebid": {
                    "bidderparams": {"own": "selected"}
                }}
            });
            for bytes in [128_usize, 1_024, 8_192] {
                let payload = "x".repeat(bytes);
                let mut source = json!({
                    "id": "allocation-root", "imp": [{"not_copied": "source-only"}],
                    "ext": {"ordinary": "keep", "prebid": {
                        "bidderparams": {"fake_selected": {"own": "selected"}}
                    }}
                });
                match omitted {
                    Omitted::CompetitorParams => {
                        source["ext"]["prebid"]["bidderparams"]["fake_competitor"] =
                            json!({"private": payload});
                    }
                    Omitted::BidderConfig => {
                        source["ext"]["prebid"]["bidderconfig"] = json!([{
                            "bidders": ["fake_competitor"],
                            "config": {"ortb2": {"user": {"keywords": payload}}}
                        }]);
                    }
                    Omitted::BuyerUids(member) => {
                        source["user"] = json!({"buyeruid": "explicit-hint", "ext": {
                            "prebid": {(member): {
                                "fake_selected": payload, "fake_other": "competitor-canary"
                            }, "keep": "context"}
                        }});
                        expected["user"] = json!({"buyeruid": "explicit-hint"});
                    }
                }
                let (result, requested, peak) = measured_root(&source);
                observations.push((result, requested, peak));
            }
            assert_eq!(observations.len(), 3);
            for (left, right) in observations.iter().zip(observations.iter().skip(1)) {
                assert_eq!(
                    (left.1, left.2),
                    (right.1, right.2),
                    "omitted allocation growth: {omitted:?}"
                );
            }
            for (result, _, _) in observations {
                assert_eq!(result, expected, "omitted={omitted:?}");
            }
        }

        for user_prefix in [false, true] {
            let mut retained = Vec::with_capacity(3);
            for bytes in [128_usize, 1_024, 8_192] {
                let payload = "x".repeat(bytes);
                let source = if user_prefix {
                    json!({"user": {"ext": {"Prebid": {"ordinary": payload}}}})
                } else {
                    json!({"id": "retained-root", "ext": {"ordinary": payload}})
                };
                let expected = source.clone();
                let (result, requested, peak) = measured_root(&source);
                assert_eq!(result, expected);
                retained.push((u64::try_from(bytes).unwrap(), requested, peak));
            }
            assert_eq!(retained.len(), 3);
            for (smaller, larger) in retained.iter().zip(retained.iter().skip(1)) {
                let payload_growth = larger.0.checked_sub(smaller.0).unwrap();
                assert!(larger.1.checked_sub(smaller.1).unwrap() >= payload_growth);
                assert!(larger.2.checked_sub(smaller.2).unwrap() >= payload_growth);
            }
        }
    }
}

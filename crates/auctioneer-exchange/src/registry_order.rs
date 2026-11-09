//! Participating-bidder set assembly + lexicographic ordering.
//!
//! `run_auction` does NOT call `participating_bidders` — the driver
//! reads its bidder set out of `split_by_bidder`'s per-imp scan
//! (which is authoritative). This helper exists
//! as a public API for callers that want the participating-bidder
//! set without also paying for the split. Sources match `split.rs`
//! so the two never drift.

use crate::alias::AliasMap;
use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::collections::BTreeSet;

/// Lexicographic union of every source that indicates bidder
/// participation on an imp, aligning with the three-source discovery
/// in `split::process_one_imp`:
///
/// - **Modern hoisted:** `imp.ext.bidder.<name>` (written by the
///   exchange during hoisting; when present on the inbound wire it
///   already looks like this shape).
/// - **Modern typed:** `imp.ext.prebid.bidder.<name>`.
/// - **Legacy top-level:** `imp.ext.<name>` (the read slot).
///
/// A key in any of the three slots is counted when it resolves either
/// against `registry` (a base bidder) OR against the supplied resolved
/// `aliases` map (a request- or host-declared alias). Request alias
/// declarations are not rebuilt here. Unregistered top-level passthrough
/// keys (`imp.ext.tid`, `imp.ext.prebid`, arbitrary FPD-adjacent
/// slots) are excluded.
///
/// An alias present in the resolved map but not targeted
/// by any imp does NOT appear here — fanout would ignore it, so the
/// public participation set must agree. `split_by_bidder` only
/// emits `PerBidderRequest` for aliases with imps; matching that
/// invariant keeps this helper from drifting.
#[must_use]
pub fn participating_bidders(
    req: &BidRequest,
    registry: &BidderRegistry,
    aliases: &AliasMap,
) -> Vec<BidderName> {
    let is_participating_name =
        |key: &str| -> bool { registry.get(key).is_some() || aliases.bases.contains_key(key) };

    let mut set: BTreeSet<BidderName> = BTreeSet::new();

    for imp in &req.imp {
        let Some(ext) = imp.ext.as_ref() else {
            continue;
        };

        if let Some(hoisted) = ext.bidder.as_ref().and_then(|val| val.as_object()) {
            for key in hoisted.keys() {
                if is_participating_name(key) {
                    set.insert(BidderName::new(key.as_str()));
                }
            }
        }
        if let Some(prebid) = ext.prebid.as_ref() {
            for key in prebid.bidder.keys() {
                if is_participating_name(key) {
                    set.insert(BidderName::new(key.as_str()));
                }
            }
        }
        // Legacy `imp.ext.<name>` lands in `ext.other` (the flatten
        // catch-all).
        for key in ext.other.keys() {
            if is_participating_name(key) {
                set.insert(BidderName::new(key.as_str()));
            }
        }
    }

    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::{AliasMap, BidRequest, BidderRegistry, participating_bidders};
    use crate::alias::resolve_aliases;
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderName, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::BidderInfo;
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::{BidRequestExt, Imp};
    use auctioneer_openrtb::types::field::JsonField;
    use std::collections::{BTreeMap, BTreeSet};

    fn imp_with_legacy_top_level(id: &str, entries: &[(&str, serde_json::Value)]) -> Imp {
        let mut other = serde_json::Map::new();
        for (name, value) in entries {
            other.insert((*name).to_owned(), value.clone());
        }
        Imp {
            id: id.into(),
            ext: Some(ImpExt {
                other,
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    struct Dummy;
    impl Bidder for Dummy {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            _r: &BidRequest,
            _i: &BidderRequestInfo<'_>,
            _s: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            Ok(())
        }
        fn make_bids(
            &self,
            _r: &BidRequest,
            _q: &OutboundRequestMeta,
            _p: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            Ok(BidderResponse {
                bids: vec![],
                currency: "USD".into(),
                errors: vec![],
                auction_configs: Vec::new(),
            })
        }
    }

    fn registry_with(names: &[&'static str]) -> BidderRegistry {
        let mut reg = BidderRegistry::new();
        for name in names {
            let info: &'static BidderInfo = Box::leak(Box::new(BidderInfo {
                name: (*name).into(),
                ..BidderInfo::default()
            }));
            reg.register(info, Box::new(Dummy))
                .expect("unique test bidder");
        }
        reg
    }

    fn imp_with_legacy_bidder_block(id: &str, bidder_names: &[&str]) -> Imp {
        let mut obj = serde_json::Map::new();
        for name in bidder_names {
            obj.insert((*name).to_owned(), serde_json::json!({}));
        }
        Imp {
            id: id.into(),
            ext: Some(ImpExt {
                bidder: JsonField::Value(serde_json::Value::Object(obj)),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    #[test]
    fn collects_from_imp_ext_bidder() {
        let registry = registry_with(&["fake_aaa", "fake_ccc", "fake_bb"]);
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_bidder_block(
                "i1",
                &["fake_ccc", "fake_aaa"],
            )],
            ..BidRequest::default()
        };
        let names: Vec<String> = participating_bidders(&req, &registry, &AliasMap::default())
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["fake_aaa".to_owned(), "fake_ccc".to_owned()]);
    }

    #[test]
    fn resolved_host_alias_is_discovered_in_each_imp_slot() {
        let registry = registry_with(&["fake_base"]);
        let host_aliases = BTreeMap::from([("fake_host".to_owned(), "fake_base".to_owned())]);
        for ext in [
            serde_json::json!({"bidder": {"fake_host": {}}}),
            serde_json::json!({"prebid": {"bidder": {"fake_host": {}}}}),
            serde_json::json!({"fake_host": {}}),
        ] {
            let req: BidRequest = serde_json::from_value(serde_json::json!({
                "id": "fake_request",
                "imp": [{"id": "fake_imp", "ext": ext}]
            }))
            .expect("valid fake request");
            let aliases = resolve_aliases(&req, &registry, &BTreeSet::new(), &host_aliases)
                .expect("valid host alias");
            assert_eq!(
                aliases.map.bases.get("fake_host"),
                Some(&BidderName::new("fake_base"))
            );
            assert_eq!(
                participating_bidders(&req, &registry, &aliases.map),
                vec![BidderName::new("fake_host")]
            );
        }
    }

    #[test]
    fn alias_targeted_by_imp_appears_as_first_class_entry() {
        let registry = registry_with(&["fake_aaa"]);
        let mut aliases = BTreeMap::new();
        aliases.insert("fake_bz".to_owned(), "fake_aaa".to_owned());
        let req = BidRequest {
            id: "x".into(),
            // Both `fake_aaa` and `fake_bz` are named in the imp's
            // hoisted bidder block.
            imp: vec![imp_with_legacy_bidder_block("i1", &["fake_aaa", "fake_bz"])],
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        };
        let resolved_aliases = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new())
            .expect("valid request alias");
        let names: Vec<String> = participating_bidders(&req, &registry, &resolved_aliases.map)
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["fake_aaa".to_owned(), "fake_bz".to_owned()]);
    }

    #[test]
    fn declared_alias_not_targeted_by_any_imp_is_excluded() {
        // `fake_bz` is declared in `ext.prebid.aliases` but no imp
        // references it. Fanout wouldn't emit a call for it, so the
        // public participation set must not include it either.
        let registry = registry_with(&["fake_aaa"]);
        let mut aliases = BTreeMap::new();
        aliases.insert("fake_bz".to_owned(), "fake_aaa".to_owned());
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_bidder_block("i1", &["fake_aaa"])],
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        };
        let resolved_aliases = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new())
            .expect("valid unused request alias");
        let names: Vec<String> = participating_bidders(&req, &registry, &resolved_aliases.map)
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["fake_aaa".to_owned()]);
    }

    #[test]
    fn unknown_bidder_name_is_filtered_out() {
        let registry = registry_with(&["fake_aaa"]);
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_bidder_block("i1", &["fake_unknown_bidder"])],
            ..BidRequest::default()
        };
        let names = participating_bidders(&req, &registry, &AliasMap::default());
        assert!(names.is_empty());
    }

    #[test]
    fn legacy_top_level_slot_is_discovered() {
        // Legacy `imp.ext.<bidderName>` written by
        // upstream client-side Prebid libraries must also count as
        // participation. Matches `split::process_one_imp`.
        let registry = registry_with(&["fake_aaa", "fake_ccc"]);
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_top_level(
                "i1",
                &[
                    ("fake_aaa", serde_json::json!({"placement_id": 1_i32})),
                    ("tid", serde_json::json!("transaction-id-value")),
                ],
            )],
            ..BidRequest::default()
        };
        let names: Vec<String> = participating_bidders(&req, &registry, &AliasMap::default())
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        // `fake_aaa` is registered → included. `tid` is a
        // passthrough key not registered as a bidder → excluded.
        assert_eq!(names, vec!["fake_aaa".to_owned()]);
    }

    #[test]
    fn legacy_top_level_slot_accepts_request_alias() {
        // An alias declared in `ext.prebid.aliases` is a valid bidder
        // identity even when the top-level slot is used, matching
        // split.rs.
        let registry = registry_with(&["fake_aaa"]);
        let mut aliases = BTreeMap::new();
        aliases.insert("fake_bz".to_owned(), "fake_aaa".to_owned());
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_top_level(
                "i1",
                &[("fake_bz", serde_json::json!({"placement_id": 2_i32}))],
            )],
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        };
        let resolved_aliases = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new())
            .expect("valid legacy-slot request alias");
        let names: Vec<String> = participating_bidders(&req, &registry, &resolved_aliases.map)
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["fake_bz".to_owned()]);
    }

    #[test]
    fn unused_host_aliases_are_excluded_and_targeted_names_are_sorted_once() {
        let registry = registry_with(&["fake_base"]);
        let req: BidRequest = serde_json::from_value(serde_json::json!({
            "id": "fake_request",
            "imp": [
                {"id": "fake_imp_1", "ext": {"bidder": {"fake_host": {}, "fake_base": {}}}},
                {"id": "fake_imp_2", "ext": {"prebid": {"bidder": {"fake_host": {}}}, "fake_host": {}}}
            ]
        }))
        .expect("valid fake request");
        let host_aliases = BTreeMap::from([
            ("fake_host".to_owned(), "fake_base".to_owned()),
            ("fake_unused".to_owned(), "fake_base".to_owned()),
        ]);
        let aliases = resolve_aliases(&req, &registry, &BTreeSet::new(), &host_aliases)
            .expect("valid host aliases");
        assert!(aliases.map.bases.contains_key("fake_unused"));
        assert_eq!(
            participating_bidders(&req, &registry, &aliases.map),
            vec![BidderName::new("fake_base"), BidderName::new("fake_host")]
        );
    }

    #[test]
    fn request_alias_declaration_does_not_override_supplied_resolved_map() {
        let registry = registry_with(&["fake_base"]);
        let req: BidRequest = serde_json::from_value(serde_json::json!({
            "id": "fake_request",
            "imp": [{"id": "fake_imp", "ext": {"bidder": {"fake_alias": {}}}}],
            "ext": {"prebid": {"aliases": {"fake_alias": "fake_base"}}}
        }))
        .expect("valid request-declared alias");
        assert!(participating_bidders(&req, &registry, &AliasMap::default()).is_empty());
        let aliases = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new())
            .expect("valid resolved request alias");
        assert_eq!(
            participating_bidders(&req, &registry, &aliases.map),
            vec![BidderName::new("fake_alias")]
        );
    }
}

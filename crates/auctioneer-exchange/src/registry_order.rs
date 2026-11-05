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
/// against `registry` (a base bidder) OR against `ext.prebid.aliases`
/// (a request-declared alias). Unregistered top-level passthrough
/// keys (`imp.ext.tid`, `imp.ext.prebid`, arbitrary FPD-adjacent
/// slots) are excluded.
///
/// A request alias declared in `ext.prebid.aliases` but not targeted
/// by any imp does NOT appear here — fanout would ignore it, so the
/// public participation set must agree. `split_by_bidder` only
/// emits `PerBidderRequest` for aliases with imps; matching that
/// invariant keeps this helper from drifting.
#[must_use]
pub fn participating_bidders(req: &BidRequest, registry: &BidderRegistry) -> Vec<BidderName> {
    let aliases = request_aliases(req);
    let is_participating_name = |key: &str| -> bool {
        registry.get(key).is_some() || aliases.bases.contains_key(&BidderName::new(key))
    };

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

fn request_aliases(req: &BidRequest) -> AliasMap {
    let mut out = AliasMap::default();
    let Some(prebid) = req.ext.as_ref().and_then(|ext| ext.prebid.as_ref()) else {
        return out;
    };
    for (alias, base) in &prebid.aliases {
        out.bases.insert(
            BidderName::new(alias.as_str()),
            BidderName::new(base.as_str()),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{BidRequest, BidderRegistry, participating_bidders};
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::BidderInfo;
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::{BidRequestExt, Imp};
    use auctioneer_openrtb::types::field::JsonField;
    use std::collections::BTreeMap;

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
        let names: Vec<String> = participating_bidders(&req, &registry)
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["fake_aaa".to_owned(), "fake_ccc".to_owned()]);
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
        let names: Vec<String> = participating_bidders(&req, &registry)
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
        let names: Vec<String> = participating_bidders(&req, &registry)
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
        let names = participating_bidders(&req, &registry);
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
        let names: Vec<String> = participating_bidders(&req, &registry)
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
        let names: Vec<String> = participating_bidders(&req, &registry)
            .into_iter()
            .map(|name| name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["fake_bz".to_owned()]);
    }
}

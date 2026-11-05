//! Alias resolution + alias-validation errors.

use crate::error::AuctionError;
use crate::response::ErrorEntry;
use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_config_data::types::MAX_BIDDER_NAME_BYTES;
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// `Clone` so [`ResolvedAliases`] — and through it `AuctionPlan` — can
/// derive it; both fields are `HashMap<BidderName, _>` of `Clone` types.
#[derive(Clone, Debug, Default)]
pub struct AliasMap {
    /// `alias_name -> base_name`.
    pub bases: HashMap<BidderName, BidderName>,
    /// `alias_name -> gvl_vendor_id` override (from
    /// `ext.prebid.aliasgvlids`).
    pub gvlids: HashMap<BidderName, u32>,
}

/// The alias half of the auction plan: the resolved map plus any
/// warnings resolution produced.
///
/// The map and the warnings travel together because both are per-request
/// alias output and both are consumed inside `run_auction` — the map by
/// the splitter and the call-list builder, the warnings by the same
/// `disabled_bidder_warnings` conveyor the splitter feeds.
#[derive(Clone, Debug, Default)]
pub struct ResolvedAliases {
    pub map: AliasMap,
    /// Populated by the disabled-aware resolver; empty until it lands.
    /// `ErrorEntry` is the same pair `split_by_bidder` produces.
    pub disabled_warnings: Vec<(BidderName, ErrorEntry)>,
    /// Alias names dropped from `map` by the disabled-overlay check —
    /// every name here also has a matching entry in `disabled_warnings`.
    /// Carried separately (rather than requiring a consumer to reverse-
    /// engineer it from the warnings) so downstream code can recognize
    /// "a real alias, currently disabled" as distinct from "never
    /// declared": the `aliasgvlids` guard skips these instead of
    /// reporting them unknown, and the splitter denies participation
    /// before alias or direct-registry resolution, even when the name
    /// shadows a compiled bidder. Competitor-params stripping still
    /// treats the name as a bidder identity.
    pub dropped_alias_names: BTreeSet<BidderName>,
}

impl ResolvedAliases {
    /// Empty scaffold (no request aliases, no warnings).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Wrap an existing `AliasMap` with no warnings — the shape the
    /// request-alias resolver produces today.
    #[must_use]
    pub fn wrap(map: AliasMap) -> Self {
        Self {
            map,
            disabled_warnings: Vec::new(),
            dropped_alias_names: BTreeSet::new(),
        }
    }
}

/// Resolve every alias the request and the host config declare, against
/// the registry.
///
/// Aliases reach an auction from two layers and both land in the SAME
/// [`AliasMap`], so nothing downstream has to know which one declared a
/// given name: `ext.prebid.aliases` on the request, and `[aliases]` in
/// the resolved host config (`host_aliases`). Where both name the same
/// alias key the REQUEST wins outright — its declaration is evaluated
/// and the host row for that key is skipped entirely, so a request alias
/// that is itself dropped cannot leave the host's mapping standing in
/// its place.
///
/// The two layers fail DIFFERENTLY on purpose, because different people
/// control them:
/// - A request-declared alias with an unknown or baked-disabled base is
///   `AuctionError::BadInput` (a 400). The request author controls that
///   text and can fix it per request.
/// - A host-declared alias with an unknown base is DROPPED with a
///   `log::warn!` and no warning code: an operator typo in deployment
///   config must not 400 every auction on the deployment. The alias is
///   simply absent from the map. "Absent" is literal, and it has one
///   edge worth stating: such an alias is NOT recorded in
///   `dropped_alias_names` either, because it never became a bidder
///   identity. So a request whose `aliasgvlids` names it still gets the
///   `BadInput` the unknown-alias guard below gives any undeclared name.
///   That is the consistent reading — from the publisher's side the
///   alias does not exist — but it does mean "a host-alias typo cannot
///   400 an auction" holds only for requests that do not themselves
///   reference the typo'd name.
///
/// The `disabled` set is the overlay-driven kill switch
/// (`[bidders.<name>] enabled = false`), checked in addition to — and
/// independently of — the registry's baked `BidderInfo.disabled` bit.
/// An alias from EITHER layer whose base or own name is disabled is
/// dropped from the returned map and recorded in
/// `ResolvedAliases::disabled_warnings` under the alias's own observable
/// name, so the removal stays visible instead of silent (spec §5.9). A
/// disabled base is a "bidder turned off" signal an operator wants to
/// see, which is why it keeps its code-6 warning on the host layer too,
/// where an unknown base gets none.
/// `ResolvedAliases::dropped_alias_names` carries the same set of names
/// so a dropped alias can still be told apart from one that was never
/// declared, both here (the `aliasgvlids` guard below) and downstream
/// in the splitter.
///
/// # Errors
/// Returns `AuctionError::BadInput` on: unknown base bidder, baked-
/// disabled base bidder, no-op alias (alias name equals base name),
/// unknown `aliasgvlids` entry (excluding one naming an alias dropped
/// for being overlay-disabled — that is a no-op, not an error), or an
/// `aliasgvlids` value outside `1..=u16::MAX`. Every one of those is a
/// judgement on REQUEST-declared input; a host-declared alias never
/// produces an error from this function.
pub fn resolve_aliases(
    req: &BidRequest,
    registry: &BidderRegistry,
    disabled: &BTreeSet<BidderName>,
    host_aliases: &BTreeMap<String, String>,
) -> Result<ResolvedAliases, AuctionError> {
    let mut out = AliasMap::default();
    let mut disabled_warnings: Vec<(BidderName, ErrorEntry)> = Vec::new();
    let mut dropped_alias_names: BTreeSet<BidderName> = BTreeSet::new();
    let request_prebid = req.ext.as_ref().and_then(|ext| ext.prebid.as_ref());

    // `ext.prebid.aliases` — map of `alias_name -> base_name`.
    if let Some(prebid) = request_prebid {
        for (alias_name, base) in &prebid.aliases {
            validate_identifier(alias_name, "alias name")?;
            validate_identifier(base, "alias base")?;
            if alias_name == base {
                return Err(AuctionError::BadInput(format!(
                    "alias {alias_name} is a no-op"
                )));
            }
            let (info, _adapter) = registry.get(base).ok_or_else(|| {
                AuctionError::BadInput(format!("alias {alias_name} targets unknown bidder {base}"))
            })?;
            if info.disabled {
                return Err(AuctionError::BadInput(format!(
                    "alias {alias_name} targets disabled bidder {base}"
                )));
            }
            let alias_key = BidderName::new(alias_name.as_str());
            let base_key = BidderName::new(base.as_str());
            if disabled.contains(&alias_key) || disabled.contains(&base_key) {
                dropped_alias_names.insert(alias_key.clone());
                disabled_warnings.push((
                    alias_key,
                    ErrorEntry {
                        code: 6,
                        message: format!("bidder disabled by overlay: {alias_name}"),
                    },
                ));
                continue;
            }
            out.bases.insert(alias_key, base_key);
        }
    }

    // `[aliases]` from the resolved host config. Runs AFTER the request
    // layer and skips any key the request already spoke for, which is
    // what makes "request wins" hold for a request alias that resolved
    // AND for one the disabled check dropped.
    for (alias_name, base) in host_aliases {
        if request_prebid.is_some_and(|prebid| prebid.aliases.contains_key(alias_name)) {
            continue;
        }
        if validate_identifier(alias_name, "alias name").is_err()
            || validate_identifier(base, "alias base").is_err()
        {
            log::warn!("host alias exceeds the bidder identity byte limit; dropping alias");
            continue;
        }
        let Some((info, _adapter)) = registry.get(base) else {
            // Permissive drop: the operator named a bidder this build
            // does not carry. Failing the request would take down every
            // auction on the deployment over one bad config row.
            log::warn!(
                "host-declared alias {alias_name} targets unknown bidder {base}; dropping the \
                 alias. Fix the `[aliases]` entry in the host config."
            );
            continue;
        };
        let alias_key = BidderName::new(alias_name.as_str());
        let base_key = BidderName::new(base.as_str());
        if info.disabled || disabled.contains(&alias_key) || disabled.contains(&base_key) {
            dropped_alias_names.insert(alias_key.clone());
            disabled_warnings.push((
                alias_key,
                ErrorEntry {
                    code: 6,
                    message: format!("bidder disabled by overlay: {alias_name}"),
                },
            ));
            continue;
        }
        out.bases.insert(alias_key, base_key);
    }

    // `ext.prebid.aliasgvlids` — each entry must reference a declared
    // alias and the value must name a vendor the GVL can hold. "Declared"
    // spans both layers, so an override may target a host-declared alias
    // the request never mentioned. The value checks run first and apply
    // unconditionally: a malformed vendor id is malformed regardless of
    // whether its alias happens to be overlay-disabled. Only after that
    // does a name naming a just-dropped alias get skipped rather than
    // reported unknown — the override is moot for a bidder that will not
    // run, not an error the publisher caused by also sending
    // `aliasgvlids`.
    if let Some(prebid) = request_prebid {
        for (alias_name, gvlid) in &prebid.aliasgvlids {
            validate_identifier(alias_name, "aliasgvlids name")?;
            let alias_key = BidderName::new(alias_name.as_str());
            if *gvlid < 1 {
                return Err(AuctionError::BadInput(format!(
                    "aliasgvlids value for {alias_name} must be >= 1"
                )));
            }
            // The GVL vendor namespace ends at u16::MAX. Rejecting here
            // means a request cannot reach the permission path carrying
            // an id that only a truncation could resolve.
            if *gvlid > u32::from(u16::MAX) {
                return Err(AuctionError::BadInput(format!(
                    "aliasgvlids[{alias_name}]: vendor id {gvlid} exceeds u16::MAX"
                )));
            }
            if dropped_alias_names.contains(&alias_key) {
                continue;
            }
            if !out.bases.contains_key(&alias_key) {
                return Err(AuctionError::BadInput(format!(
                    "aliasgvlids names unknown alias {alias_name}"
                )));
            }
            out.gvlids.insert(alias_key, *gvlid);
        }
    }

    Ok(ResolvedAliases {
        map: out,
        disabled_warnings,
        dropped_alias_names,
    })
}

fn validate_identifier(name: &str, field: &str) -> Result<(), AuctionError> {
    if name.len() > MAX_BIDDER_NAME_BYTES {
        return Err(AuctionError::BadInput(format!(
            "{field} exceeds the {MAX_BIDDER_NAME_BYTES}-byte bidder identity limit"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{AuctionError, BidRequest, BidderName, BidderRegistry, resolve_aliases};
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta, OutboundRequestSink,
    };
    use auctioneer_config_data::types::BidderInfo;
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::BidRequestExt;
    use std::collections::{BTreeMap, BTreeSet};

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

    fn registry_with(names: &[(&'static str, bool /* disabled */)]) -> BidderRegistry {
        let mut reg = BidderRegistry::new();
        for (name, disabled) in names {
            let info: &'static BidderInfo = Box::leak(Box::new(BidderInfo {
                name: (*name).into(),
                disabled: *disabled,
                ..BidderInfo::default()
            }));
            reg.register(info, Box::new(Dummy))
                .expect("unique test bidder");
        }
        reg
    }

    fn req_with_aliases(aliases: &[(&str, &str)]) -> BidRequest {
        let mut alias_map = BTreeMap::new();
        for (key, value) in aliases {
            alias_map.insert((*key).to_owned(), (*value).to_owned());
        }
        BidRequest {
            id: "x".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        }
    }

    fn req_with_alias_and_gvlids(aliases: &[(&str, &str)], gvlids: &[(&str, u32)]) -> BidRequest {
        let mut alias_map = BTreeMap::new();
        for (key, value) in aliases {
            alias_map.insert((*key).to_owned(), (*value).to_owned());
        }
        let mut gvlid_map = BTreeMap::new();
        for (key, value) in gvlids {
            gvlid_map.insert((*key).to_owned(), *value);
        }
        BidRequest {
            id: "x".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    aliasgvlids: gvlid_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        }
    }

    #[test]
    fn rejects_unknown_base() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let req = req_with_aliases(&[("fake_my", "fake_nope")]);
        let err = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap_err();
        assert!(
            matches!(&err, AuctionError::BadInput(msg) if msg.contains("unknown bidder fake_nope"))
        );
    }

    #[test]
    fn oversized_request_alias_is_rejected_before_warning_construction() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let name = "a".repeat(64 * 1024);
        let req = req_with_aliases(&[(&name, "fake_aaa")]);
        let disabled = BTreeSet::from([BidderName::new("fake_aaa")]);
        let err = resolve_aliases(&req, &registry, &disabled, &BTreeMap::new())
            .expect_err("oversized identifiers cannot become per-bid values");
        assert!(matches!(err, AuctionError::BadInput(message) if message.len() < 256));
    }

    #[test]
    fn alias_names_are_bounded_in_bytes_without_restricting_unicode() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let at_limit = "\u{e9}".repeat(32);
        let req = req_with_aliases(&[(&at_limit, "fake_aaa")]);
        let aliases = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new())
            .expect("64-byte names are valid");
        assert!(
            aliases
                .map
                .bases
                .contains_key(&BidderName::new(at_limit.as_str()))
        );
        let too_long = "\u{e9}".repeat(33);
        let oversized = req_with_aliases(&[(&too_long, "fake_aaa")]);
        assert!(matches!(
            resolve_aliases(&oversized, &registry, &BTreeSet::new(), &BTreeMap::new()),
            Err(AuctionError::BadInput(_))
        ));
    }

    #[test]
    fn oversized_alias_base_and_gvl_name_have_bounded_errors() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let huge = "x".repeat(64 * 1024);
        for req in [
            req_with_aliases(&[("fake_alias", &huge)]),
            req_with_alias_and_gvlids(&[], &[(&huge, 42)]),
        ] {
            let err = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new())
                .expect_err("oversized alias fields are rejected");
            assert!(matches!(err, AuctionError::BadInput(message) if message.len() < 256));
        }
    }

    #[test]
    fn rejects_disabled_base() {
        let registry = registry_with(&[("fake_aaa", true)]);
        let req = req_with_aliases(&[("fake_my", "fake_aaa")]);
        let err = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap_err();
        assert!(
            matches!(&err, AuctionError::BadInput(msg) if msg.contains("disabled bidder fake_aaa"))
        );
    }

    #[test]
    fn rejects_no_op_alias() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let req = req_with_aliases(&[("fake_aaa", "fake_aaa")]);
        let err = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap_err();
        assert!(matches!(&err, AuctionError::BadInput(msg) if msg.contains("no-op")));
    }

    #[test]
    fn aliasgvlids_unknown_alias_rejected() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let req = req_with_alias_and_gvlids(&[("fake_my", "fake_aaa")], &[("fake_other", 42)]);
        let err = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap_err();
        assert!(
            matches!(&err, AuctionError::BadInput(msg) if msg.contains("unknown alias fake_other"))
        );
    }

    #[test]
    fn aliasgvlids_zero_rejected() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let req = req_with_alias_and_gvlids(&[("fake_my", "fake_aaa")], &[("fake_my", 0)]);
        let err = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap_err();
        assert!(matches!(&err, AuctionError::BadInput(msg) if msg.contains(">= 1")));
    }

    #[test]
    fn aliasgvlids_above_u16_max_rejected() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let req =
            req_with_alias_and_gvlids(&[("fake_my", "fake_aaa")], &[("fake_my", 0x0001_0000)]);
        let err = resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap_err();
        assert!(
            matches!(&err, AuctionError::BadInput(msg) if msg == "aliasgvlids[fake_my]: vendor id 65536 exceeds u16::MAX")
        );
    }

    #[test]
    fn valid_alias_pair_resolves() {
        let registry = registry_with(&[("fake_aaa", false)]);
        let req = req_with_alias_and_gvlids(&[("fake_my", "fake_aaa")], &[("fake_my", 42)]);
        let resolved =
            resolve_aliases(&req, &registry, &BTreeSet::new(), &BTreeMap::new()).unwrap();
        assert_eq!(
            resolved.map.bases.get(&BidderName::new("fake_my")),
            Some(&BidderName::new("fake_aaa"))
        );
        assert_eq!(
            resolved.map.gvlids.get(&BidderName::new("fake_my")),
            Some(&42)
        );
        assert!(resolved.disabled_warnings.is_empty());
    }
}

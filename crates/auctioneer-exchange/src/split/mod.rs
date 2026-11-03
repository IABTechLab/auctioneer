//! Per-bidder request split. auction core uses `serde_json::Value`
//! ops for the per-imp ext rewrite + FPD merge; typed FPD is deferred.

mod resource;

use crate::{alias::AliasMap, error::AuctionError, response::ErrorEntry};
use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_config::types::MAX_BIDDER_NAME_BYTES;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::json_preflight::{MAX_CONSTRUCTION_DEPTH, preflight};
use serde_json::Value;
use serde_json::map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::mem;

#[derive(Debug)]
pub struct PerBidderRequest {
    pub bidder: BidderName,
    pub base: BidderName,
    pub request: BidRequest,
}

#[derive(Debug, Default)]
pub struct SplitBidders {
    pub per_bidder: Vec<PerBidderRequest>,
    /// Code-6 `BidderTemporarilyDisabled` warnings emitted while
    /// pruning per-imp bidders, carried as wire-shaped `ErrorEntry`
    /// so the code can be a raw `u32: 6`. `run_auction` folds each
    /// entry into `bundles.entry(bidder).warnings`.
    pub disabled_bidder_warnings: Vec<(BidderName, ErrorEntry)>,
}

/// One serialized request and name/index bookkeeping, never per-bidder clones.
#[derive(Debug)]
pub struct GroupedBidders {
    request_value: Value,
    per_bidder_imps: BTreeMap<BidderName, Vec<usize>>,
    disabled_bidder_warnings: Vec<(BidderName, ErrorEntry)>,
}

#[expect(
    clippy::multiple_inherent_impl,
    reason = "projection accounting is isolated in the resource submodule"
)]
impl GroupedBidders {
    /// Read-only participation in deterministic bidder order.
    #[must_use]
    pub fn bidder_imps(&self) -> &BTreeMap<BidderName, Vec<usize>> {
        &self.per_bidder_imps
    }

    /// Drain warnings into auction bookkeeping before releasing the shared tree.
    #[must_use]
    pub fn take_disabled_bidder_warnings(&mut self) -> Vec<(BidderName, ErrorEntry)> {
        mem::take(&mut self.disabled_bidder_warnings)
    }
}

/// Shared identity inputs for participation and competitor stripping.
struct SplitInputs<'a> {
    aliases: &'a AliasMap,
    registry: &'a BidderRegistry,
    /// Alias names `resolve_aliases` dropped from `aliases.bases`
    /// because their base or own name was disabled. Denied before registry
    /// fallback, including compiled-name collisions. Recognized by
    /// `is_known_bidder` as "a real alias, just disabled" so the
    /// competitor-params strip in `build_scoped_imp` still treats the
    /// name as a bidder identity instead of leaking it as passthrough
    /// data into another bidder's outbound body.
    dropped_alias_names: &'a BTreeSet<BidderName>,
}

/// Split a single `BidRequest` into one per-bidder `BidRequest` per
/// participating bidder.
///
/// `disabled` is the overlay-driven kill switch
/// (`[bidders.<name>] enabled = false`, folded into
/// `AuctionPlan.disabled_bidders`), checked in addition to — and
/// independently of — the registry's baked `BidderInfo.disabled` bit.
/// A directly-requested bidder name that is in `disabled` is dropped
/// the same way a baked-disabled one is: excluded from the per-bidder
/// output and recorded in `disabled_bidder_warnings`. `dropped_alias_names`
/// is `ResolvedAliases::dropped_alias_names` — the alias names the
/// resolver already dropped for the same reason. They cannot participate,
/// even when they collide with compiled names, but remain recognized for
/// competitor-parameter stripping (see `SplitInputs`).
///
/// # Errors
/// - `AuctionError::Internal` on JSON serialization failure (would
///   imply a bug in the typed-to-JSON round-trip).
/// - `AuctionError::BadInput` when an imp ends up with zero valid
///   bidders after disabled-bidder pruning, or an explicit bidder
///   identity exceeds the byte limit.
/// - `AuctionError::InvalidImpFirstPartyData` (code 15) when the
///   bidder-scoped imp fails the typed round-trip OR the post-FPD
///   `BidRequest::validate()` (e.g. `imp.ext.prebid.imp.<name> =
///   {"banner": null}` removes the only media type).
pub fn split_by_bidder(
    req: &BidRequest,
    aliases: &AliasMap,
    registry: &BidderRegistry,
    disabled: &BTreeSet<BidderName>,
    dropped_alias_names: &BTreeSet<BidderName>,
) -> Result<SplitBidders, AuctionError> {
    let grouped = group_by_bidder(req, aliases, registry, disabled, dropped_alias_names)?;
    let survivors = grouped.per_bidder_imps.keys().cloned().collect::<Vec<_>>();
    let per_bidder =
        materialize_per_bidder(&grouped, &survivors, aliases, registry, dropped_alias_names)?;
    Ok(SplitBidders {
        per_bidder,
        disabled_bidder_warnings: grouped.disabled_bidder_warnings,
    })
}

/// Discover enabled, known identities without projecting params or FPD.
/// Request-level bidderparams supplement existing participation only.
///
/// # Errors
/// Returns `Internal` on serialization failure, or `BadInput` if an impression
/// has no valid bidders after disabled-bidder pruning or an explicit bidder
/// identity exceeds the byte limit.
pub fn group_by_bidder(
    req: &BidRequest,
    aliases: &AliasMap,
    registry: &BidderRegistry,
    disabled: &BTreeSet<BidderName>,
    dropped_alias_names: &BTreeSet<BidderName>,
) -> Result<GroupedBidders, AuctionError> {
    preflight(req, u64::MAX, MAX_CONSTRUCTION_DEPTH)
        .map_err(|_error| AuctionError::Internal("request construction limit exceeded".into()))?;
    // Preflight every borrowed explicit identity before any request or params
    // copies. Legacy ext keys may instead be arbitrary passthrough.
    for imp in &req.imp {
        if let Some(ext) = &imp.ext
            && (ext
                .bidder
                .as_ref()
                .and_then(Value::as_object)
                .is_some_and(|bidders| bidders.keys().any(|key| key.len() > MAX_BIDDER_NAME_BYTES))
                || ext.prebid.as_ref().is_some_and(|prebid| {
                    prebid
                        .bidder
                        .keys()
                        .any(|key| key.len() > MAX_BIDDER_NAME_BYTES)
                }))
        {
            return Err(AuctionError::BadInput(format!(
                "imp bidder name exceeds the {MAX_BIDDER_NAME_BYTES}-byte bidder identity limit"
            )));
        }
    }
    let request_value: Value = serde_json::to_value(req)
        .map_err(|err| AuctionError::Internal(format!("split serialize: {err}")))?;
    let inputs = SplitInputs {
        aliases,
        registry,
        dropped_alias_names,
    };

    let mut per_bidder_imps = BTreeMap::new();
    let mut disabled_bidder_warnings: Vec<(BidderName, ErrorEntry)> = Vec::new();

    let imps = request_value
        .get("imp")
        .and_then(Value::as_array)
        .ok_or_else(|| AuctionError::Internal("serialized request has no imp array".into()))?;
    for (imp_idx, imp) in imps.iter().enumerate() {
        group_one_imp(
            imp_idx,
            imp,
            &inputs,
            disabled,
            &mut per_bidder_imps,
            &mut disabled_bidder_warnings,
        )?;
    }

    Ok(GroupedBidders {
        request_value,
        per_bidder_imps,
        disabled_bidder_warnings,
    })
}

/// Project and validate only the selected identities, in survivor order.
/// The live caller must select privacy/sampling/cap survivors before calling.
///
/// # Errors
/// Returns code 15 if FPD changes an impression identity, fails the typed
/// roundtrip, or fails request validation;
/// returns `Internal` if a selected name was not grouped.
pub fn materialize_per_bidder(
    grouped: &GroupedBidders,
    survivors: &[BidderName],
    aliases: &AliasMap,
    registry: &BidderRegistry,
    dropped_alias_names: &BTreeSet<BidderName>,
) -> Result<Vec<PerBidderRequest>, AuctionError> {
    let inputs = SplitInputs {
        aliases,
        registry,
        dropped_alias_names,
    };
    let request_value = &grouped.request_value;
    let mut out = Vec::with_capacity(survivors.len());
    // Per-imp supplemental copies are built only for survivors, after privacy,
    // sampling and capping. Projection preflight funds those copies and the
    // typed round-trip; candidates that were discarded never materialize them.
    for bidder_name in survivors {
        let indices = grouped
            .per_bidder_imps
            .get(bidder_name)
            .ok_or_else(|| AuctionError::Internal("ungrouped bidder selected".into()))?;
        let mut imps = Vec::with_capacity(indices.len());
        for &index in indices {
            let imp = request_value
                .get("imp")
                .and_then(|original_imps| original_imps.get(index))
                .ok_or_else(|| {
                    AuctionError::Internal("grouped impression index out of bounds".into())
                })?;
            let ext = imp.get("ext").unwrap_or(&Value::Null);
            // Object keys, not dynamic JSON Pointers: aliases may contain / or ~.
            let name = bidder_name.as_str();
            let mut params = ext
                .get("bidder")
                .and_then(|params| params.get(name))
                .or_else(|| {
                    ext.pointer("/prebid/bidder")
                        .and_then(|params| params.get(name))
                })
                .or_else(|| ext.get(name))
                .cloned()
                .unwrap_or(Value::Null);
            if let Some(supplement) = request_value
                .pointer("/ext/prebid/bidderparams")
                .and_then(|supplements| supplements.get(name))
            {
                merge_supplement(&mut params, supplement);
            }
            let scoped = build_scoped_imp(imp, bidder_name, params, &inputs);
            // Participation and response correlation belong to the original
            // impression. FPD may change context, never move or delete its ID.
            if scoped.get("id") != imp.get("id") {
                return Err(AuctionError::InvalidImpFirstPartyData(
                    "bidder-specific FPD must preserve the original impression id".into(),
                ));
            }
            imps.push(scoped);
        }
        let base = aliases
            .bases
            .get(bidder_name)
            .cloned()
            .unwrap_or_else(|| bidder_name.clone());
        // Do not clone the original imp array just to replace it.
        let mut object = request_value
            .as_object()
            .ok_or_else(|| AuctionError::Internal("grouped request is not an object".into()))?
            .iter()
            .filter(|(key, _)| key.as_str() != "imp")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<serde_json::Map<_, _>>();
        object.insert("imp".into(), Value::Array(imps));
        let br_value = Value::Object(object);
        let bid_request: BidRequest = serde_json::from_value(br_value)
            .map_err(|err| AuctionError::InvalidImpFirstPartyData(err.to_string()))?;
        // Re-validate the bidder-scoped `BidRequest` after FPD merge.
        // Bidder-specific FPD is applied via RFC
        // 7396 merge-patch, so it can legally set a media-type field
        // to `null` and remove it — a valid original imp can become
        // structurally invalid for the bidder. Serde's typed
        // round-trip does NOT enforce the media-type invariant;
        // only `BidRequest::validate()` does. Failure is fatal
        // (code 15 `InvalidImpFirstPartyData`, handler surfaces
        // as HTTP 500).
        bid_request
            .validate()
            .map_err(|err| AuctionError::InvalidImpFirstPartyData(err.to_string()))?;
        out.push(PerBidderRequest {
            bidder: bidder_name.clone(),
            base,
            request: bid_request,
        });
    }
    Ok(out)
}

fn group_one_imp(
    imp_idx: usize,
    imp: &Value,
    inputs: &SplitInputs<'_>,
    disabled: &BTreeSet<BidderName>,
    per_bidder_imps: &mut BTreeMap<BidderName, Vec<usize>>,
    disabled_bidder_warnings: &mut Vec<(BidderName, ErrorEntry)>,
) -> Result<(), AuctionError> {
    let imp_ext = imp.get("ext").unwrap_or(&Value::Null);

    // Compute which bidder names appear on this imp. Three source slots:
    //   1. `imp.ext.bidder.<name>` — modern hoisted slot
    //   2. `imp.ext.prebid.bidder.<name>` — modern typed slot
    //   3. `imp.ext.<name>` — legacy top-level slot. A key is a "bidder name"
    //      only if the base name resolves in the registry OR the name
    //      is a request-declared alias — otherwise the top-level
    //      `imp.ext.tid` / `imp.ext.prebid` / etc. would be
    //      misclassified.
    let mut bidders_on_imp: BTreeSet<&str> = BTreeSet::new();
    if let Some(obj) = imp_ext.pointer("/bidder").and_then(Value::as_object) {
        bidders_on_imp.extend(obj.keys().map(String::as_str));
    }
    if let Some(obj) = imp_ext.pointer("/prebid/bidder").and_then(Value::as_object) {
        bidders_on_imp.extend(obj.keys().map(String::as_str));
    }
    if let Some(obj) = imp_ext.as_object() {
        for key in obj.keys() {
            // Structural keys carry no bidder identity.
            if matches!(key.as_str(), "tid" | "prebid" | "bidder") {
                continue;
            }
            if !is_known_bidder(key, inputs) {
                continue;
            }
            bidders_on_imp.insert(key.as_str());
        }
    }

    let mut imp_landed_with_any_bidder = false;
    for name in bidders_on_imp {
        let bidder_name = BidderName::new(name);
        // A dropped alias must not fall back to a registered bidder with
        // the same name. Its disabled warning already comes from the resolver.
        if inputs.dropped_alias_names.contains(&bidder_name) {
            continue;
        }
        let base = inputs
            .aliases
            .bases
            .get(&bidder_name)
            .unwrap_or(&bidder_name);

        let Some((info, _adapter)) = inputs.registry.get(base.as_str()) else {
            continue;
        };
        // Overlay-driven kill switch, checked on the directly-requested
        // identity — independent of the baked `info.disabled` bit below.
        // A request-declared alias whose base or own name is disabled
        // is already dropped upstream by `resolve_aliases`, so this
        // covers the plain (non-aliased) participation case.
        if disabled.contains(&bidder_name) {
            disabled_bidder_warnings.push((
                bidder_name.clone(),
                ErrorEntry {
                    code: 6,
                    message: format!(
                        "bidder {} disabled by overlay (imp[{imp_idx}])",
                        bidder_name.as_str()
                    ),
                },
            ));
            continue;
        }
        if info.disabled {
            disabled_bidder_warnings.push((
                bidder_name.clone(),
                ErrorEntry {
                    code: 6,
                    message: format!(
                        "bidder {} is disabled (imp[{imp_idx}])",
                        bidder_name.as_str()
                    ),
                },
            ));
            continue;
        }

        imp_landed_with_any_bidder = true;
        per_bidder_imps
            .entry(bidder_name)
            .or_insert_with(|| Vec::with_capacity(1))
            .push(imp_idx);
    }

    if !imp_landed_with_any_bidder {
        return Err(AuctionError::BadInput(format!(
            "imp {imp_id} has no valid bidders",
            imp_id = imp.get("id").and_then(Value::as_str).unwrap_or_default(),
        )));
    }
    Ok(())
}

fn build_scoped_imp(
    imp_value: &Value,
    bidder_name: &BidderName,
    params: Value,
    inputs: &SplitInputs<'_>,
) -> Value {
    // Copy only retained fields; competitor subtrees must not be cloned just
    // to be discarded. Keep non-object shapes intact for merge-patch semantics.
    let mut scoped = if let Some(imp) = imp_value.as_object() {
        Value::Object(
            imp.iter()
                .map(|(key, value)| {
                    let scoped_value = if key == "ext" {
                        clone_scoped_ext(value, inputs)
                    } else {
                        value.clone()
                    };
                    (key.clone(), scoped_value)
                })
                .collect(),
        )
    } else {
        imp_value.clone()
    };
    // Replace `imp.ext.bidder` with this bidder's params only.
    if let Some(ext) = scoped.pointer_mut("/ext").and_then(Value::as_object_mut) {
        ext.insert("bidder".into(), params);
    }
    // Merge bidder-specific FPD from `imp.ext.prebid.imp.<name>`.
    // Object-based lookup (NOT a `format!("/ext/prebid/imp/{name}")`
    // JSON Pointer) so bidder / alias names containing `/` or `~`
    // — which JSON Pointer treats as metacharacters — resolve
    // correctly. Alias validation does not yet forbid
    // those characters, so the safer lookup is the correct fix.
    if let Some(fpd) = imp_value
        .pointer("/ext/prebid/imp")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(bidder_name.as_str()))
    {
        merge_patch(&mut scoped, fpd);
    }
    // Strip every OTHER bidder's competitor params / FPD.
    // Three surfaces:
    //   1. `imp.ext.prebid.bidder` — modern per-bidder param map.
    //   2. `imp.ext.prebid.imp`    — modern per-bidder FPD.
    //   3. `imp.ext.<name>`        — legacy top-level slot. Keep
    //      only entries whose name is not a registered bidder or
    //      alias; drop everything else so competitor legacy params
    //      do not leak into this bidder's outbound body.
    if let Some(prebid) = scoped
        .pointer_mut("/ext/prebid")
        .and_then(Value::as_object_mut)
    {
        prebid.remove("bidder");
        prebid.remove("imp");
    }
    if let Some(ext_obj) = scoped.pointer_mut("/ext").and_then(Value::as_object_mut) {
        ext_obj.retain(|key, _| {
            if matches!(key.as_str(), "tid" | "prebid" | "bidder") {
                return true;
            }
            !is_known_bidder(key, inputs)
        });
    }
    scoped
}

fn clone_scoped_ext(ext: &Value, inputs: &SplitInputs<'_>) -> Value {
    let Some(ext_obj) = ext.as_object() else {
        return ext.clone();
    };
    let mut scoped = serde_json::Map::new();
    for (key, value) in ext_obj {
        // `bidder` is replaced with selected params before FPD. Other bidder
        // slots are always removed after FPD, so their old values cannot affect
        // retained fields. The final strip still removes reintroduced slots.
        if key == "bidder"
            || (!matches!(key.as_str(), "tid" | "prebid") && is_known_bidder(key, inputs))
        {
            continue;
        }
        let scoped_value = if key == "prebid"
            && let Some(prebid) = value.as_object()
        {
            Value::Object(
                prebid
                    .iter()
                    .filter(|(field, _)| !matches!(field.as_str(), "bidder" | "imp"))
                    .map(|(field, child)| (field.clone(), child.clone()))
                    .collect(),
            )
        } else {
            value.clone()
        };
        scoped.insert(key.clone(), scoped_value);
    }
    Value::Object(scoped)
}

/// A key at `imp.ext.<key>` (or an entry in `imp.ext.bidder.<key>` /
/// `imp.ext.prebid.bidder.<key>`) is a "bidder" iff it resolves in the
/// registry as a base bidder, is a currently-live request-declared
/// alias, OR is a request-declared alias `resolve_aliases` dropped for
/// being overlay-disabled. That last case matters here as much as the
/// live ones: a disabled alias is still a real bidder identity, so its
/// legacy top-level slot must keep being recognized — otherwise
/// `build_scoped_imp`'s competitor strip stops treating it as a
/// bidder's own params and leaks it into every OTHER bidder's outbound
/// body instead.
fn is_known_bidder(name: &str, inputs: &SplitInputs<'_>) -> bool {
    if inputs.registry.get(name).is_some() {
        return true;
    }
    let key = BidderName::new(name);
    inputs.aliases.bases.contains_key(&key) || inputs.dropped_alias_names.contains(&key)
}

/// RFC 7396 JSON merge-patch. Used for FPD overrides at imp scope.
fn merge_patch(target: &mut Value, patch: &Value) {
    if patch.is_object() && !target.is_object() {
        *target = Value::Object(serde_json::Map::new());
    }
    if let (Value::Object(target_obj), Value::Object(patch_obj)) = (&mut *target, patch) {
        for (key, value) in patch_obj {
            if value.is_null() {
                target_obj.remove(key);
            } else {
                let entry = target_obj.entry(key.clone()).or_insert(Value::Null);
                merge_patch(entry, value);
            }
        }
    } else {
        *target = patch.clone();
    }
}

/// Supplement merge: only fill missing keys; never overwrite.
fn merge_supplement(target: &mut Value, supplement: &Value) {
    let (Value::Object(target_obj), Value::Object(supplement_obj)) = (&mut *target, supplement)
    else {
        return;
    };
    for (key, value) in supplement_obj {
        match target_obj.entry(key.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(value.clone());
            }
            Entry::Occupied(mut slot) => {
                merge_supplement(slot.get_mut(), value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AliasMap, BTreeSet, BidderName, BidderRegistry, group_by_bidder, materialize_per_bidder,
        merge_patch, merge_supplement, split_by_bidder,
    };
    use auctioneer_bidder::{
        Bidder, BidderError, BidderHttpResponse, BidderRequestInfo, BidderResponse,
        DebugBodySchema, OutboundRequestMeta,
    };
    use auctioneer_config::types::BidderInfo;
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
    use auctioneer_openrtb::types::field::JsonField;

    // Independent clone-first oracle: preserve the pre-optimization sequencing.
    fn clone_first_scoped_imp(
        imp_value: &serde_json::Value,
        bidder_name: &BidderName,
        params: serde_json::Value,
        inputs: &super::SplitInputs<'_>,
    ) -> serde_json::Value {
        use serde_json::Value;

        let mut scoped = imp_value.clone();
        if let Some(ext) = scoped.pointer_mut("/ext").and_then(Value::as_object_mut) {
            ext.insert("bidder".into(), params);
        }
        if let Some(fpd) = imp_value
            .pointer("/ext/prebid/imp")
            .and_then(Value::as_object)
            .and_then(|obj| obj.get(bidder_name.as_str()))
            .cloned()
        {
            merge_patch(&mut scoped, &fpd);
        }
        if let Some(prebid) = scoped
            .pointer_mut("/ext/prebid")
            .and_then(Value::as_object_mut)
        {
            prebid.remove("bidder");
            prebid.remove("imp");
        }
        if let Some(ext_obj) = scoped.pointer_mut("/ext").and_then(Value::as_object_mut) {
            ext_obj.retain(|key, _| {
                if matches!(key.as_str(), "tid" | "prebid" | "bidder") {
                    return true;
                }
                !super::is_known_bidder(key, inputs)
            });
        }
        scoped
    }

    #[test]
    fn scoped_imp_matches_clone_first_for_fpd_merge_and_strip_boundaries() {
        use serde_json::{Value, json};

        let registry = registry_with(&["fake_aaa", "fake_ccc"]);
        let mut aliases = AliasMap::default();
        for name in [
            "fake_selected/~",
            "fake_competitor/~",
            "tid",
            "prebid",
            "bidder",
        ] {
            aliases
                .bases
                .insert(BidderName::new(name), BidderName::new("fake_aaa"));
        }
        let dropped = BTreeSet::from([BidderName::new("fake_disabled/~")]);
        let inputs = super::SplitInputs {
            aliases: &aliases,
            registry: &registry,
            dropped_alias_names: &dropped,
        };
        let patches = [
            Value::Null,
            json!(false),
            json!(7_i64),
            json!("replacement"),
            json!([null, {"ext": {"fake_ccc": "inside-array"}}]),
            json!({}),
            json!({"id": null}),
            json!({"id": "different", "banner": null}),
            json!({"banner": {"w": 640_i32, "h": null}, "added": {"null": null}}),
            json!({"ext": null}),
            json!({"ext": false}),
            json!({"ext": 9_i64}),
            json!({"ext": "replacement"}),
            json!({"ext": [{"prebid": {"imp": "inside-array"}}]}),
            json!({"ext": {}}),
            json!({"ext": {"bidder": null}}),
            json!({"ext": {"bidder": false}}),
            json!({"ext": {"bidder": [null, {"keep_null": null}]}}),
            json!({"ext": {"bidder": {"remove": null, "nested": {"old": null, "new": true}}}}),
            json!({"ext": {"prebid": null}}),
            json!({"ext": {"prebid": false}}),
            json!({"ext": {"prebid": "replacement"}}),
            json!({"ext": {"prebid": [{"bidder": "inside-array"}]}}),
            json!({"ext": {"prebid": {}}}),
            json!({"ext": {"prebid": {"bidder": {"fake_ccc": "reintroduced"}, "imp": false}}}),
            json!({"ext": {"prebid": {"storedrequest": null, "added": {"remove": null, "keep": true}}}}),
            json!({"ext": {
                "fake_aaa": {"reintroduced": true},
                "fake_ccc": {"reintroduced": true},
                "fake_selected/~": {"reintroduced": true},
                "fake_competitor/~": {"reintroduced": true},
                "fake_disabled/~": {"reintroduced": true},
                "unknown": {"old": null, "new": true},
                "tid": "patched", "PREBID": {"imp": "passthrough"}
            }}),
        ];
        for name in ["fake_aaa", "fake_selected/~"] {
            for params in [
                Value::Null,
                json!([null]),
                json!({"remove": true, "nested": {"old": true}}),
            ] {
                for patch in &patches {
                    let imp = json!({
                        "id": "original", "banner": {"w": 300_i32, "h": 250_i32},
                        "ext": {
                            "tid": "original", "bidder": {name: "source", "fake_ccc": "competitor"},
                            "prebid": {
                                "bidder": {name: "typed-source", "fake_ccc": "competitor"},
                                "imp": {name: patch, "fake_ccc": {"competitor": true}},
                                "storedrequest": {"id": "kept"}
                            },
                            "fake_aaa": "legacy", "fake_ccc": "competitor",
                            "fake_selected/~": "legacy", "fake_competitor/~": "competitor",
                            "fake_disabled/~": "competitor", "unknown": {"old": true},
                            "PREBID": null
                        }
                    });
                    assert!(imp["ext"]["prebid"]["imp"].get(name).is_some());
                    assert!(imp["ext"].get("fake_disabled/~").is_some());
                    let original = serde_json::to_vec(&imp).unwrap();
                    let bidder = BidderName::new(name);
                    let expected = clone_first_scoped_imp(&imp, &bidder, params.clone(), &inputs);
                    let actual = super::build_scoped_imp(&imp, &bidder, params.clone(), &inputs);
                    assert_eq!(
                        actual, expected,
                        "name={name}, params={params}, patch={patch}"
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&expected).unwrap()
                    );
                    assert_eq!(serde_json::to_vec(&imp).unwrap(), original);
                }
            }
        }
    }

    #[test]
    fn scoped_imp_matches_clone_first_for_source_presence_and_nonobject_shapes() {
        use serde_json::{Value, json};

        let registry = registry_with(&["fake_aaa"]);
        let aliases = AliasMap::default();
        let dropped = BTreeSet::new();
        let inputs = super::SplitInputs {
            aliases: &aliases,
            registry: &registry,
            dropped_alias_names: &dropped,
        };
        let mut imps = vec![
            Value::Null,
            json!(true),
            json!(7_i64),
            json!("imp"),
            json!([]),
            json!({"id": "i"}),
        ];
        for ext in [
            Value::Null,
            json!(false),
            json!(7_i64),
            json!("ext"),
            json!([]),
            json!({}),
        ] {
            imps.push(json!({"id": "i", "ext": ext}));
        }
        for prebid in [
            Value::Null,
            json!(false),
            json!(7_i64),
            json!("prebid"),
            json!([]),
            json!({}),
            json!({"bidder": null, "imp": null}),
            json!({"bidder": [], "imp": []}),
            json!({"bidder": {"fake_aaa": {}}, "imp": {"unknown": {}}}),
        ] {
            imps.push(json!({"id": "i", "ext": {"prebid": prebid}}));
        }
        for imp in imps {
            for params in [Value::Null, json!({"selected": true})] {
                let original = serde_json::to_vec(&imp).unwrap();
                let bidder = BidderName::new("fake_aaa");
                let expected = clone_first_scoped_imp(&imp, &bidder, params.clone(), &inputs);
                let actual = super::build_scoped_imp(&imp, &bidder, params, &inputs);
                assert_eq!(actual, expected, "source={imp}");
                assert_eq!(
                    serde_json::to_vec(&actual).unwrap(),
                    serde_json::to_vec(&expected).unwrap()
                );
                assert_eq!(serde_json::to_vec(&imp).unwrap(), original);
            }
        }
    }

    #[test]
    fn scoped_materialization_matches_clone_first_params_precedence_and_supplements() {
        use serde_json::{Value, json};

        let registry = registry_with(&["fake_aaa", "fake_ccc"]);
        let mut aliases = AliasMap::default();
        aliases.bases.insert(
            BidderName::new("fake_selected/~"),
            BidderName::new("fake_aaa"),
        );
        let dropped = BTreeSet::new();
        let inputs = super::SplitInputs {
            aliases: &aliases,
            registry: &registry,
            dropped_alias_names: &dropped,
        };
        for name in ["fake_aaa", "fake_selected/~"] {
            for slot in ["hoisted", "typed", "legacy"] {
                for null_params in [false, true] {
                    let selected = if null_params {
                        Value::Null
                    } else {
                        json!({"chosen": slot, "nested": {"keep": 1_i64}})
                    };
                    let mut ext = json!({
                        name: {"chosen": "legacy-decoy"},
                        "bidder": {"fake_ccc": {}},
                        "prebid": {
                            "bidder": {"fake_ccc": {}},
                            "imp": {name: {"banner": {"w": 640_i32}, "ext": {"bidder": {"fpd": true}}}}
                        }
                    });
                    match slot {
                        "hoisted" => {
                            ext["bidder"][name] = selected;
                            ext["prebid"]["bidder"][name] = json!({"chosen": "typed-decoy"});
                        }
                        "typed" => ext["prebid"]["bidder"][name] = selected,
                        "legacy" => ext[name] = selected,
                        _ => panic!("unknown scoped parameter slot: {slot}"),
                    }
                    let request: BidRequest = serde_json::from_value(json!({
                        "id": "request", "site": {"id": "site"},
                        "imp": [{"id": "i", "banner": {"w": 300_i32, "h": 250_i32}, "ext": ext}],
                        "ext": {"prebid": {"bidderparams": {name: {
                            "chosen": "supplement-decoy", "fill": "supplement",
                            "nested": {"keep": 9_i64, "add": 2_i64}
                        }}}}
                    }))
                    .unwrap();
                    let original = serde_json::to_vec(&request).unwrap();
                    let grouped =
                        group_by_bidder(&request, &aliases, &registry, &BTreeSet::new(), &dropped)
                            .unwrap();
                    let grouped_before = serde_json::to_vec(&grouped.request_value).unwrap();
                    let bidder = BidderName::new(name);
                    let params = if null_params {
                        Value::Null
                    } else {
                        json!({
                            "chosen": slot, "fill": "supplement", "nested": {"keep": 1_i64, "add": 2_i64}
                        })
                    };
                    let mut expected_value = grouped.request_value.clone();
                    expected_value["imp"][0] = clone_first_scoped_imp(
                        &grouped.request_value["imp"][0],
                        &bidder,
                        params,
                        &inputs,
                    );
                    let expected: BidRequest = serde_json::from_value(expected_value).unwrap();
                    expected.validate().unwrap();
                    let scoped =
                        materialize_per_bidder(&grouped, &[bidder], &aliases, &registry, &dropped)
                            .unwrap();
                    assert_eq!(scoped.len(), 1);
                    assert_eq!(
                        serde_json::to_value(&scoped[0].request).unwrap(),
                        serde_json::to_value(&expected).unwrap(),
                        "name={name}, slot={slot}, null={null_params}"
                    );
                    assert_eq!(
                        serde_json::to_vec(&scoped[0].request).unwrap(),
                        serde_json::to_vec(&expected).unwrap()
                    );
                    assert_eq!(serde_json::to_vec(&request).unwrap(), original);
                    assert_eq!(
                        serde_json::to_vec(&grouped.request_value).unwrap(),
                        grouped_before
                    );
                }
            }
        }
    }

    #[test]
    fn merge_patch_replaces_arrays_whole() {
        let mut tgt = serde_json::json!({"a": [1_i64, 2_i64, 3_i64], "b": "x"});
        let patch = serde_json::json!({"a": [9_i64], "c": "y"});
        merge_patch(&mut tgt, &patch);
        assert_eq!(tgt, serde_json::json!({"a": [9_i64], "b": "x", "c": "y"}));
    }

    #[test]
    fn merge_patch_null_removes_key() {
        let mut tgt = serde_json::json!({"a": 1_i64, "b": 2_i64});
        let patch = serde_json::json!({"a": null});
        merge_patch(&mut tgt, &patch);
        assert_eq!(tgt, serde_json::json!({"b": 2_i64}));
    }

    #[test]
    fn merge_patch_object_removes_nested_nulls_from_missing_target() {
        let mut target = serde_json::json!({"keep": true});
        let patch = serde_json::json!({
            "added": {"remove": null, "nested": {"remove": null, "keep": 1_i64}}
        });
        merge_patch(&mut target, &patch);
        assert_eq!(
            target,
            serde_json::json!({"keep": true, "added": {"nested": {"keep": 1_i64}}})
        );
    }

    #[test]
    fn merge_patch_object_normalizes_null_target() {
        let mut target = serde_json::Value::Null;
        let patch = serde_json::json!({
            "remove": null, "nested": {"remove": null, "keep": 1_i64}
        });
        merge_patch(&mut target, &patch);
        assert_eq!(target, serde_json::json!({"nested": {"keep": 1_i64}}));
    }

    #[test]
    fn merge_patch_object_normalizes_scalar_and_array_targets() {
        let patch = serde_json::json!({
            "remove": null, "nested": {"remove": null, "keep": 1_i64}
        });
        for mut target in [
            serde_json::json!(true),
            serde_json::json!(7_i64),
            serde_json::json!("old"),
            serde_json::json!([1_i64, null]),
        ] {
            merge_patch(&mut target, &patch);
            assert_eq!(target, serde_json::json!({"nested": {"keep": 1_i64}}));
        }
    }

    #[test]
    fn merge_patch_non_object_patches_replace_target() {
        for patch in [
            serde_json::Value::Null,
            serde_json::json!(true),
            serde_json::json!(7_i64),
            serde_json::json!("new"),
            serde_json::json!([null, {"keep_null": null}]),
        ] {
            let mut target = serde_json::json!({"old": 1_i64});
            merge_patch(&mut target, &patch);
            assert_eq!(target, patch);
        }
    }

    #[test]
    fn supplement_only_fills_missing() {
        let mut tgt = serde_json::json!({"a": 1_i64});
        let sup = serde_json::json!({"a": 99_i64, "b": 2_i64});
        merge_supplement(&mut tgt, &sup);
        // 'a' stays 1; 'b' fills in.
        assert_eq!(tgt, serde_json::json!({"a": 1_i64, "b": 2_i64}));
    }

    // -----------------------------------------------------------------
    // Legacy `imp.ext.<bidderName>` participation + competitor
    // stripping.
    // -----------------------------------------------------------------

    struct Dummy;
    impl Bidder for Dummy {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            _r: &BidRequest,
            _i: &BidderRequestInfo<'_>,
            _sink: &mut auctioneer_bidder::OutboundRequestSink<'_>,
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

    /// Gathering and competitor stripping share the same identity rules,
    /// including aliases dropped by the disabled-bidder resolver.
    #[test]
    fn the_scan_and_the_strip_recognize_the_same_names() {
        let registry = registry_with(&["fake_base_bidder"]);
        let mut aliases = AliasMap::default();
        aliases.bases.insert(
            BidderName::new("fake_live_alias"),
            BidderName::new("fake_base_bidder"),
        );
        let dropped = BTreeSet::from([BidderName::new("fake_disabled_alias")]);
        let inputs = super::SplitInputs {
            aliases: &aliases,
            registry: &registry,
            dropped_alias_names: &dropped,
        };

        for (name, recognized) in [
            ("fake_base_bidder", true),
            ("fake_live_alias", true),
            ("fake_disabled_alias", true),
            ("fake_not_a_bidder", false),
            ("tid", false),
            ("prebid", false),
            ("bidder", false),
            ("", false),
        ] {
            assert_eq!(
                super::is_known_bidder(name, &inputs),
                recognized,
                "unexpected participation recognition for {name:?}",
            );
            // Structural slots have their own rewrite semantics; the
            // competitor strip only operates on identity-bearing keys.
            if matches!(name, "tid" | "prebid" | "bidder") {
                continue;
            }
            let mut ext = serde_json::Map::new();
            ext.insert(name.into(), serde_json::json!({"private_marker": true}));
            let imp = serde_json::json!({"id": "imp", "ext": ext});
            let scoped = super::build_scoped_imp(
                &imp,
                &BidderName::new("fake_base_bidder"),
                serde_json::json!({}),
                &inputs,
            );
            assert_eq!(
                scoped["ext"].get(name).is_none(),
                recognized,
                "gathering and materialization-time stripping disagree about {name:?}",
            );
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

    fn imp_with_legacy_ext(id: &str, entries: serde_json::Map<String, serde_json::Value>) -> Imp {
        use auctioneer_openrtb::types::bid_request::Banner;
        Imp {
            id: id.into(),
            banner: Some(Banner {
                w: Some(300_i32),
                h: Some(250_i32),
                ..Banner::default()
            }),
            ext: Some(ImpExt {
                other: entries,
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    /// Wrap an `Imp` list in a valid `BidRequest` — populates a
    /// stub `site` so inventory-shape validation passes.
    fn req_with(imps: Vec<Imp>) -> BidRequest {
        use auctioneer_openrtb::types::inventory::Site;
        BidRequest {
            id: "auc".into(),
            imp: imps,
            site: Some(Site {
                id: Some("stub".into()),
                ..Site::default()
            }),
            ..BidRequest::default()
        }
    }

    fn req_with_explicit_bidder(slot: &str, name: &str) -> BidRequest {
        let mut bidders = serde_json::Map::new();
        bidders.insert("fake_aaa".into(), serde_json::json!({"placementId": 1_i64}));
        bidders.insert(name.into(), serde_json::json!({"placementId": 2_i64}));
        let ext = match slot {
            "bidder" => serde_json::json!({"bidder": bidders}),
            "prebid.bidder" => serde_json::json!({"prebid": {"bidder": bidders}}),
            _ => panic!("unknown explicit bidder slot"),
        };
        let mut imp = imp_with_legacy_ext("imp-1", serde_json::Map::new());
        imp.ext = serde_json::from_value(ext).unwrap();
        req_with(vec![imp])
    }

    #[test]
    fn scoped_parameter_null_survives_grouping_and_typed_materialization() {
        let registry = registry_with(&["fake_aaa"]);
        for parameters in [serde_json::Value::Null, serde_json::json!({})] {
            for (slot, ext) in [
                ("legacy", serde_json::json!({"fake_aaa":parameters})),
                (
                    "bidder",
                    serde_json::json!({"bidder":{"fake_aaa":parameters}}),
                ),
                (
                    "prebid.bidder",
                    serde_json::json!({"prebid":{"bidder":{"fake_aaa":parameters}}}),
                ),
            ] {
                let mut imp = imp_with_legacy_ext("i", serde_json::Map::new());
                imp.ext = serde_json::from_value(ext).unwrap();
                let request = req_with(vec![imp]);
                let before = serde_json::to_value(&request).unwrap();
                let scoped = split_by_bidder(
                    &request,
                    &AliasMap::default(),
                    &registry,
                    &BTreeSet::new(),
                    &BTreeSet::new(),
                )
                .unwrap();
                assert_eq!(scoped.per_bidder.len(), 1, "{slot}");
                let projected = &scoped.per_bidder[0].request;
                assert_eq!(
                    serde_json::to_value(projected).unwrap()["imp"][0]["ext"]["bidder"],
                    parameters,
                    "{slot}",
                );
                let bidder = &projected.imp[0].ext.as_ref().unwrap().bidder;
                assert!(!bidder.is_missing(), "{slot}");
                assert_eq!(
                    matches!(bidder, JsonField::Null),
                    parameters.is_null(),
                    "{slot}",
                );
                assert_eq!(serde_json::to_value(&request).unwrap(), before);
            }
        }
    }

    #[test]
    fn scoped_prebid_presence_survives_grouping_and_typed_materialization() {
        let registry = registry_with(&["fake_aaa"]);
        for fields in [
            serde_json::json!({}),
            serde_json::json!({"prebid": null}),
            serde_json::json!({"prebid": {}}),
            serde_json::json!({"PREBID": null}),
            serde_json::json!({"prebid": null, "PREBID": null}),
            serde_json::json!({"prebid": null, "Prebid": {"adunitcode": "kept"}}),
        ] {
            let mut ext = fields.clone();
            ext["fake_aaa"] = serde_json::json!({"placementId": 1_i64});
            let mut expected_ext = fields.clone();
            expected_ext["bidder"] = serde_json::json!({"placementId": 1_i64});
            let mut imp = imp_with_legacy_ext("i", serde_json::Map::new());
            imp.ext = serde_json::from_value(ext).unwrap();
            let request = req_with(vec![imp]);
            let before = serde_json::to_value(&request).unwrap();
            let scoped = split_by_bidder(
                &request,
                &AliasMap::default(),
                &registry,
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
            assert_eq!(scoped.per_bidder.len(), 1);
            let projected = &scoped.per_bidder[0].request;
            assert_eq!(
                serde_json::to_value(projected).unwrap()["imp"][0]["ext"],
                expected_ext
            );
            let prebid = &projected.imp[0].ext.as_ref().unwrap().prebid;
            assert_eq!(prebid.is_missing(), fields.get("prebid").is_none());
            assert_eq!(
                matches!(prebid, JsonField::Null),
                fields.get("prebid") == Some(&serde_json::Value::Null)
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn explicit_bidder_maps_accept_64_byte_ascii_and_utf8_names() {
        let registry = registry_with(&["fake_aaa"]);
        for slot in ["bidder", "prebid.bidder"] {
            for name in ["a".repeat(64), "\u{00e9}".repeat(32)] {
                let mut aliases = AliasMap::default();
                aliases
                    .bases
                    .insert(BidderName::new(name.as_str()), BidderName::new("fake_aaa"));
                let req = req_with_explicit_bidder(slot, &name);
                let split = split_by_bidder(
                    &req,
                    &aliases,
                    &registry,
                    &BTreeSet::new(),
                    &BTreeSet::new(),
                )
                .expect("64-byte bidder names must be accepted");
                let scoped = split
                    .per_bidder
                    .iter()
                    .find(|scoped| scoped.bidder.as_str() == name)
                    .expect("the 64-byte alias must participate");
                assert_eq!(scoped.request.imp.len(), 1);
                assert_eq!(
                    scoped.request.imp[0].ext.as_ref().unwrap().bidder,
                    JsonField::Value(serde_json::json!({"placementId": 2_i64}))
                );
            }
        }
    }

    fn assert_explicit_bidder_name_rejected(slot: &str, name: &str) {
        let req = req_with_explicit_bidder(slot, name);
        let err = group_by_bidder(
            &req,
            &AliasMap::default(),
            &registry_with(&["fake_aaa"]),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect_err("overlong explicit bidder names must fail even with a valid bidder");
        let crate::AuctionError::BadInput(message) = err else {
            panic!("expected BadInput, got {err:?}");
        };
        assert!(message.len() <= 128, "bidder-name error must be bounded");
        assert!(message.contains("64"), "error must identify the byte limit");
        assert!(
            !message.contains(name),
            "error must not echo the bidder name"
        );
    }

    #[test]
    fn explicit_bidder_map_rejects_65_byte_ascii_name() {
        for len in [65, 4096] {
            assert_explicit_bidder_name_rejected("bidder", &"a".repeat(len));
        }
    }

    #[test]
    fn explicit_bidder_map_rejects_33_multibyte_characters() {
        assert_explicit_bidder_name_rejected("bidder", &"\u{00e9}".repeat(33));
    }

    #[test]
    fn explicit_prebid_bidder_map_rejects_65_byte_ascii_name() {
        for len in [65, 4096] {
            assert_explicit_bidder_name_rejected("prebid.bidder", &"a".repeat(len));
        }
    }

    #[test]
    fn explicit_prebid_bidder_map_rejects_33_multibyte_characters() {
        assert_explicit_bidder_name_rejected("prebid.bidder", &"\u{00e9}".repeat(33));
    }

    #[test]
    fn long_non_bidder_legacy_ext_keys_are_preserved() {
        let names = ["a".repeat(65), "a".repeat(4096), "\u{00e9}".repeat(33)];
        let passthrough = serde_json::json!({"weather": "sunny"});
        let mut entries = serde_json::Map::new();
        entries.insert("fake_aaa".into(), serde_json::json!({"placementId": 1_i64}));
        for name in &names {
            entries.insert(name.clone(), passthrough.clone());
        }
        let req = req_with(vec![imp_with_legacy_ext("imp-1", entries)]);
        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry_with(&["fake_aaa"]),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("legacy non-bidder keys have no bidder identity byte limit");
        assert_eq!(split.per_bidder.len(), 1);
        let ext = split.per_bidder[0].request.imp[0].ext.as_ref().unwrap();
        for name in &names {
            assert_eq!(ext.other.get(name), Some(&passthrough));
        }
    }

    #[test]
    fn legacy_imp_ext_bidder_name_is_a_participation_source() {
        let registry = registry_with(&["fake_aaa"]);
        let mut entries = serde_json::Map::new();
        entries.insert(
            "fake_aaa".into(),
            serde_json::json!({"placementId": 42_i64}),
        );
        let req = req_with(vec![imp_with_legacy_ext("imp-1", entries)]);
        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("legacy imp.ext.fake_aaa must participate");
        assert_eq!(split.per_bidder.len(), 1);
        assert_eq!(split.per_bidder[0].bidder.as_str(), "fake_aaa");
    }

    #[test]
    fn competitor_legacy_ext_is_stripped_from_scoped_body() {
        let registry = registry_with(&["fake_aaa", "fake_ccc"]);
        // Both bidders declared via legacy top-level slots.
        let mut entries = serde_json::Map::new();
        entries.insert(
            "fake_aaa".into(),
            serde_json::json!({"placementId": 42_i64}),
        );
        entries.insert("fake_ccc".into(), serde_json::json!({"publisherId": "1"}));
        let req = req_with(vec![imp_with_legacy_ext("imp-1", entries)]);
        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(split.per_bidder.len(), 2);
        for pbr in &split.per_bidder {
            let bidder = pbr.bidder.as_str();
            let ext = pbr.request.imp[0].ext.as_ref().unwrap();
            // Neither the self nor the competitor top-level legacy
            // slot survives — everything hoists into `ext.bidder`.
            assert!(
                !ext.other.contains_key("fake_aaa"),
                "top-level ext.fake_aaa must not leak into the {bidder} outbound",
            );
            assert!(
                !ext.other.contains_key("fake_ccc"),
                "top-level ext.fake_ccc must not leak into the {bidder} outbound",
            );
            // Non-bidder top-level keys are preserved.
            assert!(
                ext.bidder.as_ref().is_some(),
                "hoisted ext.bidder is populated"
            );
        }
    }

    #[test]
    fn non_bidder_top_level_ext_keys_are_preserved() {
        let registry = registry_with(&["fake_aaa"]);
        let mut entries = serde_json::Map::new();
        entries.insert(
            "fake_aaa".into(),
            serde_json::json!({"placementId": 42_i64}),
        );
        // A hypothetical non-bidder passthrough key must NOT be
        // stripped as a competitor.
        entries.insert(
            "vendor_hint".into(),
            serde_json::json!({"weather": "sunny"}),
        );
        let req = req_with(vec![imp_with_legacy_ext("imp-1", entries)]);
        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(split.per_bidder.len(), 1);
        let ext = split.per_bidder[0].request.imp[0].ext.as_ref().unwrap();
        assert!(
            ext.other.contains_key("vendor_hint"),
            "non-bidder passthrough key `vendor_hint` must survive scoping",
        );
    }

    #[test]
    fn survivor_fpd_cannot_rewrite_or_delete_an_impression_identity() {
        let registry = registry_with(&["fake_aaa", "fake_ccc"]);
        let aliases = AliasMap::default();
        let dropped = BTreeSet::new();
        for replacement in [
            serde_json::json!("foreign"),
            serde_json::json!("new-identity"),
            serde_json::json!(""),
            serde_json::Value::Null,
            serde_json::json!(7_i64),
        ] {
            let imp: Imp = serde_json::from_value(serde_json::json!({
                "id": "own", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"bidder": {"fake_aaa": {}, "fake_ccc": {}},
                    "prebid": {"imp": {"fake_aaa": {"id": replacement}}}}
            }))
            .unwrap();
            let req = req_with(vec![
                imp,
                imp_with_legacy_ext(
                    "foreign",
                    serde_json::Map::from_iter([("fake_ccc".into(), serde_json::json!({}))]),
                ),
            ]);
            let original = serde_json::to_value(&req).unwrap();
            let grouped =
                group_by_bidder(&req, &aliases, &registry, &BTreeSet::new(), &dropped).unwrap();
            let sibling = materialize_per_bidder(
                &grouped,
                &[BidderName::new("fake_ccc")],
                &aliases,
                &registry,
                &dropped,
            )
            .expect("unselected bidder FPD must not poison its sibling");
            assert_eq!(
                sibling[0]
                    .request
                    .imp
                    .iter()
                    .map(|scoped_imp| scoped_imp.id.as_str())
                    .collect::<Vec<_>>(),
                ["own", "foreign"]
            );
            let err = materialize_per_bidder(
                &grouped,
                &[BidderName::new("fake_aaa")],
                &aliases,
                &registry,
                &dropped,
            )
            .expect_err("survivor FPD must preserve its original impression identity");
            assert!(matches!(
                err,
                crate::AuctionError::InvalidImpFirstPartyData(_)
            ));
            assert_eq!(serde_json::to_value(&req).unwrap(), original);
        }
    }

    #[test]
    fn survivor_fpd_preserves_identity_while_merging_context() {
        for patch in [
            serde_json::json!({"id": "own", "banner": {"w": 640_i32}}),
            serde_json::json!({"banner": {"w": 640_i32}}),
        ] {
            let imp: Imp = serde_json::from_value(serde_json::json!({
                "id": "own", "banner": {"w": 300_i32, "h": 250_i32},
                "ext": {"bidder": {"fake_aaa": {}},
                    "prebid": {"imp": {"fake_aaa": patch}}}
            }))
            .unwrap();
            let req = req_with(vec![imp]);
            let split = split_by_bidder(
                &req,
                &AliasMap::default(),
                &registry_with(&["fake_aaa"]),
                &BTreeSet::new(),
                &BTreeSet::new(),
            )
            .unwrap();
            assert_eq!(split.per_bidder[0].request.imp[0].id, "own");
            assert_eq!(
                split.per_bidder[0].request.imp[0]
                    .banner
                    .as_ref()
                    .unwrap()
                    .w,
                Some(640_i32)
            );
            assert_eq!(req.imp[0].banner.as_ref().unwrap().w, Some(300_i32));
        }
    }

    #[test]
    fn bidder_specific_fpd_that_removes_only_media_type_returns_code_15() {
        // Bidder-specific FPD at `imp.ext.prebid.imp.<name>` is
        // merged via RFC 7396 (null removes the key). A publisher
        // request with `banner: {w: 300, h: 250}` is structurally
        // valid, but if the FPD says `banner: null` for one bidder,
        // that bidder's scoped imp has no media type and fails
        // validation. The failure is fatal and
        // the auction returns AuctionError::InvalidImpFirstPartyData.
        use crate::AuctionError;
        use auctioneer_openrtb::types::bid_request::Banner;

        let registry = registry_with(&["fake_aaa"]);
        let mut bidder_map = serde_json::Map::new();
        bidder_map.insert("fake_aaa".into(), serde_json::json!({"placementId": 1_i64}));
        let mut per_bidder_imp = serde_json::Map::new();
        per_bidder_imp.insert("fake_aaa".into(), serde_json::json!({"banner": null}));
        let imp_ext = serde_json::json!({
            "bidder": bidder_map,
            "prebid": {"imp": per_bidder_imp}
        });

        let imp = Imp {
            id: "imp-1".into(),
            banner: Some(Banner {
                w: Some(300_i32),
                h: Some(250_i32),
                ..Banner::default()
            }),
            ext: serde_json::from_value(imp_ext).unwrap(),
            ..Imp::default()
        };
        let req = req_with(vec![imp]);
        let aliases = AliasMap::default();
        let dropped = BTreeSet::new();
        let grouped = group_by_bidder(&req, &aliases, &registry, &BTreeSet::new(), &dropped)
            .expect("grouping succeeds before FPD construction");
        assert_eq!(grouped.bidder_imps().len(), 1);
        let none = materialize_per_bidder(&grouped, &[], &aliases, &registry, &dropped)
            .expect("unselected invalid FPD is not materialized");
        assert!(none.is_empty());
        let survivors: Vec<_> = grouped.bidder_imps().keys().cloned().collect();
        let err = materialize_per_bidder(&grouped, &survivors, &aliases, &registry, &dropped)
            .expect_err("selected bidder-scoped imp with no media type must fail construction");
        assert!(
            matches!(
                split_by_bidder(&req, &aliases, &registry, &BTreeSet::new(), &dropped),
                Err(AuctionError::InvalidImpFirstPartyData(_)),
            ),
            "the materialized compatibility helper must also reject invalid FPD",
        );
        assert!(
            matches!(err, AuctionError::InvalidImpFirstPartyData(_)),
            "expected InvalidImpFirstPartyData (code 15), got {err:?}",
        );
        // The `Display` message must name the concrete validation
        // failure so the auction handler surfaces it to logs.
        assert!(
            format!("{err}").contains("must declare at least one media type"),
            "InvalidImpFirstPartyData must include the ImpHasNoMediaType detail, got {err}",
        );
    }

    #[test]
    fn fpd_adds_banner_with_null_format_to_video_imp() {
        let imp: Imp = serde_json::from_value(serde_json::json!({
            "id": "imp-1",
            "video": {"mimes": ["video/mp4"], "w": 640_i32, "h": 480_i32},
            "ext": {
                "bidder": {"fake_aaa": {"placementId": 1_i64}},
                "prebid": {
                    "imp": {
                        "fake_aaa": {
                            "banner": {"format": null, "w": 300_i32, "h": 250_i32}
                        }
                    }
                }
            }
        }))
        .unwrap();
        let req = req_with(vec![imp]);
        req.validate().expect("the original video request is valid");
        assert!(req.imp[0].banner.is_none());
        let split = split_by_bidder(
            &req,
            &AliasMap::default(),
            &registry_with(&["fake_aaa"]),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("null format deletes the absent field instead of invalidating the banner");
        split.per_bidder[0].request.validate().unwrap();
        let scoped = &split.per_bidder[0].request.imp[0];
        let banner = scoped.banner.as_ref().expect("FPD adds a banner");
        assert_eq!(banner.w, Some(300_i32));
        assert_eq!(banner.h, Some(250_i32));
        assert!(banner.format.is_empty());
        assert_eq!(
            serde_json::to_value(&scoped.video).unwrap(),
            serde_json::to_value(&req.imp[0].video).unwrap()
        );
    }

    #[test]
    fn fpd_lookup_survives_slash_in_alias_name() {
        use auctioneer_openrtb::types::bid_request::Banner;
        // An alias containing `/` would break a JSON-Pointer-based
        // FPD lookup (`/ext/prebid/imp/my/alias` = navigate to
        // `imp["my"]["alias"]`). Object-based lookup should find it.
        let registry = registry_with(&["fake_aaa"]);
        let mut aliases = AliasMap::default();
        aliases.bases.insert(
            BidderName::new("fake_my/alias"),
            BidderName::new("fake_aaa"),
        );

        // imp.ext.prebid.imp.<name> carries the per-bidder FPD.
        let imp_ext = serde_json::json!({
            "bidder": {"fake_my/alias": {"placementId": 1_i64}},
            "prebid": {
                "imp": {"fake_my/alias": {"fpd_marker": "reached"}}
            }
        });
        let imp = Imp {
            id: "imp-1".into(),
            banner: Some(Banner {
                w: Some(300_i32),
                h: Some(250_i32),
                ..Banner::default()
            }),
            ext: serde_json::from_value(imp_ext).unwrap(),
            ..Imp::default()
        };
        let req = req_with(vec![imp]);
        let split = split_by_bidder(
            &req,
            &aliases,
            &registry,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .unwrap();
        assert_eq!(split.per_bidder.len(), 1);
        // FPD merge-patch applies to the imp ROOT (RFC 7396), so
        // the marker lands in `Imp.other` (the flatten catch-all).
        let scoped = &split.per_bidder[0].request.imp[0];
        assert!(
            scoped.other.contains_key("fpd_marker"),
            "FPD merge must apply for alias names with `/` in them; got {other:?}",
            other = scoped.other,
        );
    }
}

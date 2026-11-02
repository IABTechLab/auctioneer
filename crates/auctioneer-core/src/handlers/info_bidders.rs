//! `GET /info/bidders` + `GET /info/bidders/{name}` handlers.

#[cfg(test)]
use crate::bidder_registry::info_work;
use crate::bidder_registry::{BidderInfoIndex, info_index, registry_handle};
use crate::config::AuctioneerAppConfig;
use crate::config::resolve;
use crate::config::runtime::{RuntimeConfigError, resolve_runtime_config};
use crate::handlers::auction as auction_helpers;
use crate::handlers::aux_egress::with_aux_egress;
use crate::serialize::serialize_json_exact;
use auctioneer_bidder::BidderName;
use auctioneer_config::types::{BidderInfo, MediaType};
use edgezero_core::action;
use edgezero_core::body::Body;
use edgezero_core::context::RequestContext;
use edgezero_core::error::EdgeError;
use edgezero_core::http::{
    HeaderValue, Response, StatusCode, header::CONTENT_TYPE, response_builder,
};
use std::cmp::Ordering;
use std::collections::btree_set;
use std::collections::{BTreeMap, BTreeSet};
use std::iter::Peekable;

const BAD_QUERY_BODY_ENABLEDONLY: &[u8] =
    b"Invalid value for 'enabledonly' query param, must be of boolean type";
const BAD_QUERY_BODY_BASEONLY: &[u8] =
    b"Invalid value for 'baseadaptersonly' query param, must be of boolean type";

#[derive(serde::Deserialize, Default)]
struct InfoBiddersQuery {
    enabledonly: Option<String>,
    baseadaptersonly: Option<String>,
}

#[derive(serde::Deserialize)]
struct InfoBidderPath {
    name: String,
}

/// The overlay-aware catalog every `/info` surface reads: the effective
/// disabled set and the registry-validated host-alias map, both derived
/// from the SAME resolved runtime config the auction route uses, so the
/// three endpoints can never disagree about which bidders exist.
struct InfoCatalog {
    disabled: BTreeSet<BidderName>,
    aliases: BTreeMap<String, String>,
}

impl InfoCatalog {
    /// Built from the RESOLVED host config, never the baked statics:
    /// `resolve_host` is what layers the runtime `[aliases]` overlay
    /// onto the baked map, and reading `host_config_handle().aliases`
    /// directly would leave a pushed alias invisible to `/info`.
    fn from_app_config(
        app_cfg: &AuctioneerAppConfig,
    ) -> Result<Self, auctioneer_bidder::RegistryBuildError> {
        let host_cfg = resolve::resolve_host(app_cfg, auction_helpers::host_config_handle());
        Ok(Self {
            disabled: auction_helpers::disabled_from_overlay(app_cfg, registry_handle()?),
            aliases: auction_helpers::effective_host_aliases(&host_cfg.aliases, info_index()?),
        })
    }
}

/// Both `/info` routes keep the FAIL-CLOSED disposition, deliberately:
/// they are not liveness surfaces, and the whole content of the response
/// is derived from the config, so a blob that fails its integrity check
/// makes the advertised bidder list untrustworthy. Answering anyway would
/// publish a catalog nobody can vouch for. `/status` takes the permissive
/// path instead — see `RuntimeConfigError::degrade_to_default`.
async fn info_catalog(ctx: &RequestContext) -> Result<InfoCatalog, RuntimeConfigError> {
    let app_cfg = resolve_runtime_config(ctx).await?;
    Ok(InfoCatalog::from_app_config(&app_cfg)?)
}

/// Wrapped, like every aux route — see
/// [`with_aux_egress`](crate::handlers::aux_egress::with_aux_egress)
/// for what the wrapper does and why it is one shared call.
#[action]
pub async fn info_bidders(ctx: RequestContext) -> Result<Response, EdgeError> {
    with_aux_egress(ctx, info_bidders_body).await
}

async fn info_bidders_body(ctx: RequestContext) -> Result<Response, EdgeError> {
    let catalog = match info_catalog(&ctx).await {
        Ok(catalog) => catalog,
        Err(err) => return err.into_response(),
    };
    let bidders = match info_index() {
        Ok(bidders) => bidders,
        Err(error) => return RuntimeConfigError::from(error).into_response(),
    };
    let query: InfoBiddersQuery = ctx.query()?;
    let (status, content_type, body) = list(
        bidders,
        &catalog.aliases,
        &catalog.disabled,
        query.enabledonly.as_deref(),
        query.baseadaptersonly.as_deref(),
    );
    build_info_response(status, content_type, body)
}

/// Wrapped, like every aux route — see
/// [`with_aux_egress`](crate::handlers::aux_egress::with_aux_egress)
/// for what the wrapper does and why it is one shared call.
#[action]
pub async fn info_bidder(ctx: RequestContext) -> Result<Response, EdgeError> {
    with_aux_egress(ctx, info_bidder_body).await
}

async fn info_bidder_body(ctx: RequestContext) -> Result<Response, EdgeError> {
    let catalog = match info_catalog(&ctx).await {
        Ok(catalog) => catalog,
        Err(err) => return err.into_response(),
    };
    let path: InfoBidderPath = ctx.path()?;
    let bidders = match info_index() {
        Ok(bidders) => bidders,
        Err(error) => return RuntimeConfigError::from(error).into_response(),
    };
    let (status, content_type, body) =
        detail(bidders, &catalog.aliases, &catalog.disabled, &path.name);
    build_info_response(status, content_type, body)
}

fn build_info_response(
    status_u16: u16,
    content_type: Option<&'static str>,
    body: Vec<u8>,
) -> Result<Response, EdgeError> {
    let status = StatusCode::from_u16(status_u16)
        .map_err(|_err| EdgeError::internal(anyhow::anyhow!("invalid status code")))?;
    let mut builder = response_builder().status(status);
    if let Some(ct) = content_type {
        builder = builder.header(CONTENT_TYPE, HeaderValue::from_static(ct));
    }
    builder
        .body(Body::from_bytes(body))
        .map_err(EdgeError::internal)
}

/// The baked kill switch UNION the runtime overlay's.
///
/// One predicate drives every surface — list visibility, the single
/// detail row, and the `all` aggregate — so a bidder the overlay
/// disabled cannot vanish from `?enabledonly=true` while still
/// reporting `ACTIVE` on `/info/bidders/<name>`.
fn effective_disabled(info: &BidderInfo, disabled: &BTreeSet<BidderName>) -> bool {
    info.disabled || disabled.contains(&BidderName::new(info.name.as_str()))
}

/// An alias is disabled when its BASE is effectively disabled, or when
/// the operator named the ALIAS itself in the overlay. Disabling one
/// alias must not require disabling the base bidder that its siblings
/// also depend on.
fn alias_effective_disabled(
    core_info: &BidderInfo,
    alias_name: &str,
    disabled: &BTreeSet<BidderName>,
) -> bool {
    effective_disabled(core_info, disabled) || disabled.contains(&BidderName::new(alias_name))
}

#[must_use]
pub(crate) fn list(
    bidders: &BidderInfoIndex,
    aliases: &BTreeMap<String, String>,
    disabled: &BTreeSet<BidderName>,
    enabledonly: Option<&str>,
    baseadaptersonly: Option<&str>,
) -> (u16, Option<&'static str>, Vec<u8>) {
    let Some(enabled) = parse_bool(enabledonly) else {
        return (400, None, BAD_QUERY_BODY_ENABLEDONLY.to_vec());
    };
    let Some(base_only) = parse_bool(baseadaptersonly) else {
        return (400, None, BAD_QUERY_BODY_BASEONLY.to_vec());
    };
    let rows = catalog_rows(bidders, aliases, disabled, base_only);
    let names: Vec<_> = rows
        .iter()
        .filter(|row| !enabled || !row.disabled)
        .map(|row| row.name)
        .collect();
    let body = serialize_or_empty(&names);
    (200, Some("application/json"), body)
}

fn parse_bool(value: Option<&str>) -> Option<bool> {
    match value {
        Some("true") => Some(true),
        None | Some("" | "false") => Some(false),
        _ => None,
    }
}

#[must_use]
pub(crate) fn detail(
    bidders: &BidderInfoIndex,
    aliases: &BTreeMap<String, String>,
    disabled: &BTreeSet<BidderName>,
    name: &str,
) -> (u16, Option<&'static str>, Vec<u8>) {
    if name == "all" {
        let rows = catalog_rows(bidders, aliases, disabled, false);
        let body = serialize_or_empty(&AllInfo(&rows));
        return (200, Some("application/json"), body);
    }
    if let Some(core) = aliases.get(name)
        && let Some(core_info) = bidders.get(core)
    {
        let body = serialize_or_empty(&detail_value(
            core_info,
            Some(core.as_str()),
            alias_effective_disabled(core_info, name, disabled),
        ));
        return (200, Some("application/json"), body);
    }
    match bidders.get(name).or_else(|| {
        bidders
            .infos()
            .iter()
            .copied()
            .find(|b| b.name.eq_ignore_ascii_case(name))
    }) {
        Some(info) => {
            let body = serialize_or_empty(&detail_value(
                info,
                info.alias_of.as_deref(),
                effective_disabled(info, disabled),
            ));
            (200, Some("application/json"), body)
        }
        None => (404, None, Vec::new()),
    }
}

struct CatalogRow<'a> {
    name: &'a str,
    info: &'static BidderInfo,
    alias_of: Option<&'a str>,
    disabled: bool,
}

fn render_compare(left: &str, right: &str) -> Ordering {
    #[cfg(test)]
    info_work::record_rendering();
    left.cmp(right)
}

fn ordered_disabled(name: &str, cursor: &mut Peekable<btree_set::Iter<'_, BidderName>>) -> bool {
    while let Some(candidate) = cursor.peek() {
        match render_compare(candidate.as_str(), name) {
            Ordering::Less => {
                cursor.next();
            }
            Ordering::Equal => {
                cursor.next();
                return true;
            }
            Ordering::Greater => return false,
        }
    }
    false
}

/// Merge already ordered metadata and aliases once; aliases win equal keys.
fn catalog_rows<'a>(
    bidders: &BidderInfoIndex,
    aliases: &'a BTreeMap<String, String>,
    disabled: &BTreeSet<BidderName>,
    base_only: bool,
) -> Vec<CatalogRow<'a>> {
    let mut base_disabled = disabled.iter().peekable();
    let base_states = bidders
        .infos()
        .iter()
        .map(|info| {
            let overlay_disabled = ordered_disabled(&info.name, &mut base_disabled);
            info.disabled || overlay_disabled
        })
        .collect::<Vec<_>>();
    let mut alias_disabled = disabled.iter().peekable();
    let mut bases = bidders.infos().iter().zip(&base_states).peekable();
    let mut alias_entries = aliases.iter().peekable();
    let mut rows = Vec::new();
    while bases.peek().is_some() || (!base_only && alias_entries.peek().is_some()) {
        let use_base = if base_only {
            true
        } else {
            match (bases.peek(), alias_entries.peek()) {
                (Some((base, _)), Some((name, _))) => {
                    match render_compare(base.name.as_str(), name.as_str()) {
                        Ordering::Less => true,
                        Ordering::Equal => {
                            bases.next();
                            false
                        }
                        Ordering::Greater => false,
                    }
                }
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => break,
            }
        };
        if use_base {
            if let Some((info, is_disabled)) = bases.next() {
                if base_only && info.alias_of.is_some() {
                    continue;
                }
                rows.push(CatalogRow {
                    name: info.name.as_str(),
                    info,
                    alias_of: info.alias_of.as_deref(),
                    disabled: *is_disabled,
                });
            }
            continue;
        }
        if let Some((name, core)) = alias_entries.next()
            && let Some((position, info)) = bidders.find(core)
            && let Some(base_is_disabled) = base_states.get(position)
        {
            let overlay_disabled = ordered_disabled(name, &mut alias_disabled);
            rows.push(CatalogRow {
                name,
                info,
                alias_of: Some(core),
                disabled: *base_is_disabled || overlay_disabled,
            });
        }
    }
    rows
}

struct AllInfo<'a>(&'a [CatalogRow<'a>]);

impl serde::Serialize for AllInfo<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for row in self.0 {
            map.serialize_entry(
                row.name,
                &detail_value(row.info, row.alias_of, row.disabled),
            )?;
        }
        map.end()
    }
}

fn detail_value(info: &BidderInfo, alias_of: Option<&str>, disabled: bool) -> serde_json::Value {
    let mut capabilities = serde_json::Map::new();
    capabilities.insert(
        "app".into(),
        serde_json::json!({
            "mediaTypes": info
                .capabilities
                .app
                .iter()
                .copied()
                .map(media_type_str)
                .collect::<Vec<_>>(),
        }),
    );
    capabilities.insert(
        "site".into(),
        serde_json::json!({
            "mediaTypes": info
                .capabilities
                .site
                .iter()
                .copied()
                .map(media_type_str)
                .collect::<Vec<_>>(),
        }),
    );
    // Only emit the `dooh` sub-object when the bidder declared any
    // DOOH media types — omit the absent inventory block rather
    // than shipping an empty `mediaTypes` array.
    if !info.capabilities.dooh.is_empty() {
        capabilities.insert(
            "dooh".into(),
            serde_json::json!({
                "mediaTypes": info
                    .capabilities
                    .dooh
                    .iter()
                    .copied()
                    .map(media_type_str)
                    .collect::<Vec<_>>(),
            }),
        );
    }
    let mut value = serde_json::json!({
        "status": if disabled { "DISABLED" } else { "ACTIVE" },
        "usesHttps": info.uses_https,
        "maintainer": { "email": info.maintainer_email.clone().unwrap_or_default() },
        "capabilities": serde_json::Value::Object(capabilities),
    });
    if let Some(core_name) = alias_of
        && let Some(obj) = value.as_object_mut()
    {
        obj.insert("aliasOf".into(), serde_json::json!(core_name));
    }
    value
}

fn media_type_str(media: MediaType) -> &'static str {
    match media {
        MediaType::Banner => "banner",
        MediaType::Video => "video",
        MediaType::Native => "native",
        MediaType::Audio => "audio",
    }
}

fn serialize_or_empty<T>(value: &T) -> Vec<u8>
where
    T: serde::Serialize,
{
    serialize_json_exact(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        AuctioneerAppConfig, BAD_QUERY_BODY_ENABLEDONLY, BidderInfo, BidderName, InfoCatalog,
    };
    use crate::bidder_fixture::fake_bidder::FakeBidder;
    use crate::bidder_registry::BidderInfoIndex;
    use crate::config::resolve::resolve_host;
    use crate::handlers::auction as auction_helpers;
    use auctioneer_bidder::registry::BidderRegistry;
    use auctioneer_config::types::HostConfig;
    use auctioneer_exchange::alias::resolve_aliases;
    use auctioneer_exchange::split::split_by_bidder;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use std::cell::Cell;
    use std::collections::{BTreeMap, BTreeSet};

    fn list(
        bidders: &[&'static BidderInfo],
        aliases: &BTreeMap<String, String>,
        disabled: &BTreeSet<BidderName>,
        enabled: Option<&str>,
        base_only: Option<&str>,
    ) -> (u16, Option<&'static str>, Vec<u8>) {
        super::list(
            &BidderInfoIndex::new(bidders.iter().copied()),
            aliases,
            disabled,
            enabled,
            base_only,
        )
    }

    fn detail(
        bidders: &[&'static BidderInfo],
        aliases: &BTreeMap<String, String>,
        disabled: &BTreeSet<BidderName>,
        name: &str,
    ) -> (u16, Option<&'static str>, Vec<u8>) {
        super::detail(
            &BidderInfoIndex::new(bidders.iter().copied()),
            aliases,
            disabled,
            name,
        )
    }

    #[test]
    fn compiled_alias_metadata_agrees_across_base_filter_detail_and_all() {
        let base = Box::leak(Box::new(BidderInfo {
            name: "fake_base".into(),
            ..BidderInfo::default()
        }));
        let alias = Box::leak(Box::new(BidderInfo {
            name: "fake_alias".into(),
            alias_of: Some("fake_base".into()),
            ..BidderInfo::default()
        }));
        let bidders: [&'static BidderInfo; 2] = [base, alias];
        let aliases = BTreeMap::new();
        let disabled = BTreeSet::new();
        let (base_status, _, base_bytes) = list(&bidders, &aliases, &disabled, None, Some("true"));
        assert_eq!(base_status, 200);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&base_bytes).unwrap(),
            serde_json::json!(["fake_base"])
        );
        let (alias_status, _, alias_bytes) = detail(&bidders, &aliases, &disabled, "fake_alias");
        assert_eq!(alias_status, 200);
        let single: serde_json::Value = serde_json::from_slice(&alias_bytes).unwrap();
        assert_eq!(single["aliasOf"], "fake_base");
        let (all_status, _, all_bytes) = detail(&bidders, &aliases, &disabled, "all");
        assert_eq!(all_status, 200);
        let all: serde_json::Value = serde_json::from_slice(&all_bytes).unwrap();
        assert_eq!(all["fake_alias"], single);
        assert!(all["fake_base"].get("aliasOf").is_none());
    }

    #[test]
    fn json_output_capacity_matches_its_encoded_size() {
        for size in [0, 17, 4097] {
            let text = "\0\n\"\\\u{00e9}".repeat(size);
            let value = [&text, &text];
            let expected = serde_json::to_vec(&value).unwrap();
            let actual = super::serialize_or_empty(&value);
            assert_eq!(actual, expected);
            assert_eq!(actual.capacity(), expected.len());
        }
    }

    struct GrowingJson {
        passes: Cell<usize>,
        text: String,
    }

    impl serde::Serialize for GrowingJson {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let pass = self.passes.get().saturating_add(1);
            self.passes.set(pass);
            serializer.serialize_str(if pass == 1 { "" } else { &self.text })
        }
    }

    #[test]
    fn json_output_growth_keeps_the_empty_body_error_policy() {
        let value = GrowingJson {
            passes: Cell::new(0),
            text: "\0".repeat(4097),
        };
        assert!(super::serialize_or_empty(&value).is_empty());
        assert_eq!(value.passes.get(), 2, "no unbounded retry after refusal");
    }

    #[test]
    fn malformed_json_keeps_the_empty_body_error_policy() {
        let value = BTreeMap::from([((1_u8, 2_u8), 3_u8)]);
        assert!(super::serialize_or_empty(&value).is_empty());
    }

    #[test]
    fn list_detail_and_all_keep_their_wire_bytes_and_response_headers() {
        let bidders = [leak("fake_base", false)];
        let alias = "fake_alias\0\n\"\\\u{00e9}";
        let aliases = BTreeMap::from([(alias.to_owned(), "fake_base".to_owned())]);
        let disabled = nothing_disabled();
        let base = serde_json::json!({
            "status": "ACTIVE", "usesHttps": true, "maintainer": {"email": ""},
            "capabilities": {"app": {"mediaTypes": []}, "site": {"mediaTypes": []}},
        });
        let mut alias_row = base.clone();
        alias_row["aliasOf"] = serde_json::json!("fake_base");
        let all = BTreeMap::from([("fake_base", base.clone()), (alias, alias_row.clone())]);
        for ((status, content_type, body), expected) in [
            (
                list(&bidders, &aliases, &disabled, None, None),
                serde_json::to_vec(&[alias, "fake_base"]).unwrap(),
            ),
            (
                detail(&bidders, &aliases, &disabled, "fake_base"),
                serde_json::to_vec(&base).unwrap(),
            ),
            (
                detail(&bidders, &aliases, &disabled, alias),
                serde_json::to_vec(&alias_row).unwrap(),
            ),
            (
                detail(&bidders, &aliases, &disabled, "all"),
                serde_json::to_vec(&all).unwrap(),
            ),
        ] {
            assert_eq!(body, expected);
            assert_eq!(body.capacity(), expected.len());
            let response = super::build_info_response(status, content_type, body).unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["content-type"], "application/json");
            assert_eq!(response.into_body().into_bytes().unwrap(), expected);
        }
    }

    fn leak(name: &'static str, disabled: bool) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            endpoint: "https://x".into(),
            uses_https: true,
            disabled,
            ..BidderInfo::default()
        }))
    }

    fn empty_aliases() -> BTreeMap<String, String> {
        BTreeMap::new()
    }

    fn nothing_disabled() -> BTreeSet<BidderName> {
        BTreeSet::new()
    }

    #[test]
    fn info_catalog_alias_probes_and_ordered_rendering_obey_independent_bounds() {
        use crate::bidder_registry::info_work;
        for (compiled, inherited, runtime, max_probes) in [
            (512_usize, 0_usize, 0_usize, 0_usize),
            (448, 0, 64, 1_152),
            (384, 64, 64, 2_304),
        ] {
            let infos: Vec<_> = (0..compiled)
                .map(|index| {
                    leak(
                        Box::leak(format!("fake_base_{index:04}").into_boxed_str()),
                        false,
                    )
                })
                .collect();
            let last = &infos.last().unwrap().name;
            let names: Vec<_> = (0..inherited.checked_add(runtime).unwrap())
                .map(|index| (format!("fake_base_{index:04}_alias"), last.clone()))
                .collect();
            let baked_host = HostConfig {
                aliases: names[..inherited].iter().cloned().collect(),
                ..HostConfig::default()
            };
            let app = AuctioneerAppConfig {
                aliases: Some(names[inherited..].iter().cloned().collect()),
                ..AuctioneerAppConfig::default()
            };
            let host = resolve_host(&app, &baked_host);
            assert_eq!(host.aliases.len(), inherited.checked_add(runtime).unwrap());
            let mut expected = infos
                .iter()
                .map(|info| info.name.clone())
                .chain(host.aliases.keys().cloned())
                .collect::<Vec<_>>();
            expected.sort_unstable();
            assert_eq!(expected.len(), 512);
            for aggregate in [false, true] {
                info_work::take();
                let aliases = auction_helpers::effective_host_aliases(
                    &host.aliases,
                    &BidderInfoIndex::new(infos.iter().copied()),
                );
                assert_eq!(aliases.len(), host.aliases.len());
                let (status, _, body) = if aggregate {
                    detail(&infos, &aliases, &nothing_disabled(), "all")
                } else {
                    list(&infos, &aliases, &nothing_disabled(), None, None)
                };
                assert_eq!(status, 200);
                let actual = if aggregate {
                    let rows: BTreeMap<String, serde_json::Value> =
                        serde_json::from_slice(&body).unwrap();
                    for (name, row) in &rows {
                        assert_eq!(row["status"], "ACTIVE");
                        if aliases.contains_key(name) {
                            assert_eq!(row["aliasOf"], last.as_str());
                        }
                    }
                    rows.into_keys().collect::<Vec<_>>()
                } else {
                    names_from(&body)
                };
                assert_eq!(actual, expected);
                let work = info_work::take();
                assert_eq!(work.probes.len(), aliases.len().checked_mul(2).unwrap());
                assert!(
                    work.probes.iter().all(|count| *count > 0 && *count <= 9),
                    "per-probe comparisons: {:?}",
                    work.probes
                );
                assert!(work.probes.iter().sum::<usize>() <= max_probes);
                assert_eq!(
                    work.rendering > 0,
                    !aliases.is_empty(),
                    "the ordered join must be observed independently of activation checks"
                );
                assert!(
                    work.rendering <= 2_048,
                    "ordered-render comparisons: {}",
                    work.rendering
                );
            }
        }
    }

    #[test]
    fn info_catalog_late_and_absent_base_probes_cannot_degenerate_to_scans() {
        use crate::bidder_registry::info_work;
        let infos: Vec<_> = (0..512_usize)
            .map(|index| {
                leak(
                    Box::leak(format!("fake_base_{index:04}").into_boxed_str()),
                    false,
                )
            })
            .collect();
        let index = BidderInfoIndex::new(infos.iter().copied());
        for (name, found) in [
            ("fake_base_0000", true),
            ("fake_base_0511", true),
            ("fake_z_absent", false),
        ] {
            info_work::take();
            assert_eq!(index.get(name).is_some(), found);
            let work = info_work::take();
            assert_eq!(work.probes.len(), 1);
            assert!(
                work.probes[0] > 0 && work.probes[0] <= 10,
                "{name}: {} comparisons",
                work.probes[0]
            );
        }
    }

    #[test]
    fn info_catalog_disabled_activation_join_stays_linear_at_the_catalog_ceiling() {
        use crate::bidder_registry::info_work;
        for (compiled, alias_count) in [(512_usize, 0_usize), (448, 64), (384, 128)] {
            let infos = (0..compiled)
                .map(|ordinal| {
                    leak(
                        Box::leak(format!("fake_base_{ordinal:04}").into_boxed_str()),
                        false,
                    )
                })
                .collect::<Vec<_>>();
            let index = BidderInfoIndex::new(infos.iter().copied());
            let aliases = (0..alias_count)
                .map(|ordinal| {
                    (
                        format!("fake_base_{ordinal:04}_alias"),
                        infos.last().unwrap().name.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let disabled = infos
                .iter()
                .map(|info| BidderName::new(info.name.as_str()))
                .chain(aliases.keys().map(|name| BidderName::new(name.as_str())))
                .collect::<BTreeSet<_>>();
            assert_eq!(disabled.len(), 512);
            for aggregate in [false, true] {
                info_work::take();
                let (_, _, body) = if aggregate {
                    super::detail(&index, &aliases, &disabled, "all")
                } else {
                    super::list(&index, &aliases, &disabled, None, None)
                };
                let work = info_work::take();
                assert_eq!(work.probes.len(), alias_count);
                assert!(
                    work.rendering > 0 && work.rendering <= 2_048,
                    "{compiled}/{alias_count}: {} render comparisons",
                    work.rendering
                );
                let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
                if aggregate {
                    let rows = value.as_object().unwrap();
                    assert_eq!(rows.len(), 512);
                    assert!(rows.values().all(|row| row["status"] == "DISABLED"));
                } else {
                    assert_eq!(value.as_array().unwrap().len(), 512);
                }
            }
            let (_, _, enabled) = super::list(&index, &aliases, &disabled, Some("true"), None);
            assert_eq!(names_from(&enabled), Vec::<String>::new());
        }
    }

    #[test]
    fn info_catalog_projection_and_encoding_fit_the_auxiliary_manifest_reservation() {
        use crate::bidder_registry::info_work;
        use auctioneer_config::types::MediaType;
        use auctioneer_runtime::budget::AUX_MANIFEST_BYTES;
        use auctioneer_runtime::test_support::alloc_track::{peak_since, reset_peak};
        use std::hint::black_box;
        // A positive control makes a missing allocator hook observable.
        let positive = reset_peak();
        let allocation = black_box(vec![0_u8; 0x10_0001]);
        assert!(peak_since(positive) > 0x10_0000);
        drop(allocation);
        for (compiled, alias_count) in [(512_usize, 0_usize), (448, 64), (384, 128)] {
            let infos: Vec<&'static BidderInfo> = (0..compiled)
                .map(|ordinal| {
                    let mut info = BidderInfo {
                        name: format!("fake_base_{ordinal:04}"),
                        uses_https: true,
                        maintainer_email: Some("\0\n\"\\\u{e9}".repeat(32)),
                        ..BidderInfo::default()
                    };
                    for inventory in [
                        &mut info.capabilities.app,
                        &mut info.capabilities.site,
                        &mut info.capabilities.dooh,
                    ] {
                        *inventory = vec![
                            MediaType::Banner,
                            MediaType::Video,
                            MediaType::Native,
                            MediaType::Audio,
                        ];
                    }
                    &*Box::leak(Box::new(info))
                })
                .collect();
            let index = BidderInfoIndex::new(infos.iter().copied());
            let aliases = (0..alias_count)
                .map(|ordinal| {
                    (
                        format!("fake_alias_{ordinal:04}"),
                        infos.last().unwrap().name.clone(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let disabled = nothing_disabled();
            for aggregate in [false, true] {
                info_work::take();
                let baseline = reset_peak();
                let (status, _, body) = if aggregate {
                    super::detail(&index, &aliases, &disabled, "all")
                } else {
                    super::list(&index, &aliases, &disabled, None, None)
                };
                let peak = peak_since(baseline);
                assert_eq!(status, 200);
                assert!(
                    peak <= 0x10_0000,
                    "{compiled}/{} aggregate={aggregate}: {peak} requested bytes",
                    aliases.len()
                );
                assert_eq!(AUX_MANIFEST_BYTES, 0x10_0000);
                let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
                let count = if aggregate {
                    value.as_object().unwrap().len()
                } else {
                    value.as_array().unwrap().len()
                };
                assert_eq!(count, 512);
            }
        }
    }

    /// Resolve the overlay against synthetic metadata for list/detail semantics.
    fn catalog_from(overlay_toml: &str) -> InfoCatalog {
        use crate::bidder_fixture::registry;

        let app: AuctioneerAppConfig = toml::from_str(overlay_toml).expect("overlay TOML parses");
        let host = resolve_host(&app, &HostConfig::default());
        InfoCatalog {
            disabled: auction_helpers::disabled_from_overlay(&app, registry()),
            aliases: auction_helpers::effective_host_aliases(
                &host.aliases,
                &BidderInfoIndex::new(bidders().into_iter()),
            ),
        }
    }

    fn bidders() -> [&'static BidderInfo; 3] {
        [
            leak("fake_aaa", false),
            leak("fake_bbb", false),
            leak("fake_pp", false),
        ]
    }

    fn names_from(body: &[u8]) -> Vec<String> {
        serde_json::from_slice(body).expect("the list body is a JSON array of names")
    }

    fn assert_catalog_matches_auction(bidders: &[&'static BidderInfo], catalog: &InfoCatalog) {
        let (all_status, _, all_body) = detail(bidders, &catalog.aliases, &catalog.disabled, "all");
        assert_eq!(all_status, 200);
        let all: BTreeMap<String, serde_json::Value> =
            serde_json::from_slice(&all_body).expect("all is a bidder catalog");
        let (list_status, _, list_body) =
            list(bidders, &catalog.aliases, &catalog.disabled, None, None);
        assert_eq!(list_status, 200);
        assert_eq!(
            names_from(&list_body),
            all.keys().cloned().collect::<Vec<_>>()
        );

        for (name, row) in &all {
            let (detail_status, _, body) =
                detail(bidders, &catalog.aliases, &catalog.disabled, name);
            assert_eq!(detail_status, 200);
            let single: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(&single, row, "detail and all disagree for {name}");
        }

        let expected_enabled: Vec<String> = all
            .iter()
            .filter(|(_, row)| row["status"] == "ACTIVE")
            .map(|(name, _)| name.clone())
            .collect();
        let (enabled_status, _, enabled_body) = list(
            bidders,
            &catalog.aliases,
            &catalog.disabled,
            Some("true"),
            None,
        );
        assert_eq!(enabled_status, 200);
        let enabled_names = names_from(&enabled_body);
        assert_eq!(
            enabled_names, expected_enabled,
            "enabledonly must follow alias-first detail"
        );

        let mut registry = BidderRegistry::new();
        for info in bidders {
            registry
                .register(info, Box::<FakeBidder>::default())
                .expect("unique test bidder");
        }
        let params: BTreeMap<&str, serde_json::Value> = all
            .keys()
            .map(|name| (name.as_str(), serde_json::json!({})))
            .collect();
        let request: BidRequest = serde_json::from_value(serde_json::json!({
            "id": "catalog-consistency",
            "site": {"page": "https://publisher.example/"},
            "imp": [{
                "id": "imp-1",
                "banner": {"w": 300_u32, "h": 250_u32},
                "ext": { "bidder": params },
            }],
        }))
        .expect("catalog names form a bidder request");
        let resolved = resolve_aliases(&request, &registry, &catalog.disabled, &catalog.aliases)
            .expect("host aliases resolve");
        let split = split_by_bidder(
            &request,
            &resolved.map,
            &registry,
            &catalog.disabled,
            &resolved.dropped_alias_names,
        )
        .expect("the active control bidder keeps the impression eligible");
        let eligible: Vec<String> = split
            .per_bidder
            .iter()
            .map(|bidder_request| bidder_request.bidder.as_str().to_owned())
            .collect();
        assert_eq!(
            enabled_names, eligible,
            "catalog and auction eligibility disagree"
        );
    }

    #[test]
    fn shipped_catalog_wiring_resolves_overlay_aliases_and_disabled_names() {
        let bidders = super::info_index().unwrap().infos();
        let base = bidders.iter().find(|info| !info.disabled).unwrap();
        let app: AuctioneerAppConfig = toml::from_str(&format!(
            "[aliases]\nfake_runtime_conformance_alias = '{}'\n[bidders.{}]\nenabled = false\n",
            base.name, base.name,
        ))
        .unwrap();
        let catalog = InfoCatalog::from_app_config(&app).unwrap();
        assert_eq!(
            catalog.aliases.get("fake_runtime_conformance_alias"),
            Some(&base.name),
        );
        assert!(
            catalog
                .disabled
                .contains(&BidderName::new(base.name.as_str()))
        );
        assert_catalog_matches_auction(bidders, &catalog);
    }

    #[test]
    fn shadowing_alias_with_overlay_disabled_base_matches_detail_all_and_auction() {
        let catalog = catalog_from(
            "[aliases]\nfake_bbb = \"fake_aaa\"\n[bidders.fake_aaa]\nenabled = false\n",
        );
        assert_catalog_matches_auction(&bidders(), &catalog);
    }

    #[test]
    fn shadowing_alias_with_baked_disabled_base_matches_detail_all_and_auction() {
        let bidders = [
            leak("fake_aaa", true),
            leak("fake_bbb", false),
            leak("fake_pp", false),
        ];
        let catalog = InfoCatalog {
            aliases: BTreeMap::from([("fake_bbb".to_owned(), "fake_aaa".to_owned())]),
            disabled: nothing_disabled(),
        };
        assert_catalog_matches_auction(&bidders, &catalog);
    }

    #[test]
    fn shadowing_alias_disabled_by_name_matches_detail_all_and_auction() {
        let catalog = catalog_from(
            "[aliases]\nfake_bbb = \"fake_aaa\"\n[bidders.fake_bbb]\nenabled = false\n",
        );
        assert_catalog_matches_auction(&bidders(), &catalog);
    }

    #[test]
    fn enabled_shadowing_alias_does_not_inherit_compiled_names_baked_disable() {
        let bidders = [
            leak("fake_aaa", false),
            leak("fake_bbb", true),
            leak("fake_pp", false),
        ];
        let catalog = InfoCatalog {
            aliases: BTreeMap::from([("fake_bbb".to_owned(), "fake_aaa".to_owned())]),
            disabled: nothing_disabled(),
        };
        assert_catalog_matches_auction(&bidders, &catalog);
    }

    #[test]
    fn rejected_unknown_base_alias_does_not_shadow_enabled_compiled_name() {
        let catalog = catalog_from("[aliases]\nfake_bbb = \"fake_no_such_bidder\"\n");
        assert!(!catalog.aliases.contains_key("fake_bbb"));
        assert_catalog_matches_auction(&bidders(), &catalog);
    }

    #[test]
    fn base_only_keeps_enabled_compiled_name_when_shadowing_alias_base_is_disabled() {
        for baked_disabled in [false, true] {
            let bidders = [leak("fake_aaa", baked_disabled), leak("fake_bbb", false)];
            let aliases = BTreeMap::from([("fake_bbb".to_owned(), "fake_aaa".to_owned())]);
            let disabled = if baked_disabled {
                nothing_disabled()
            } else {
                BTreeSet::from([BidderName::new("fake_aaa")])
            };
            let (status, _, body) = list(&bidders, &aliases, &disabled, Some("true"), Some("true"));
            assert_eq!(status, 200);
            assert_eq!(names_from(&body), vec!["fake_bbb"]);
        }
    }

    #[test]
    fn base_only_omits_baked_disabled_compiled_name_even_when_shadowing_alias_is_enabled() {
        let bidders = [leak("fake_aaa", false), leak("fake_bbb", true)];
        let aliases = BTreeMap::from([("fake_bbb".to_owned(), "fake_aaa".to_owned())]);
        let (status, _, body) = list(
            &bidders,
            &aliases,
            &nothing_disabled(),
            Some("true"),
            Some("true"),
        );
        assert_eq!(status, 200);
        assert_eq!(names_from(&body), vec!["fake_aaa"]);
    }

    #[test]
    fn list_filters_disabled_when_enabledonly_true() {
        let bids = vec![leak("fake_a", false), leak("fake_b", true)];
        let (status, _, body) = list(
            &bids,
            &empty_aliases(),
            &nothing_disabled(),
            Some("true"),
            Some("false"),
        );
        assert_eq!(status, 200);
        assert_eq!(names_from(&body), vec!["fake_a".to_owned()]);
    }

    #[test]
    fn list_400_on_bad_query() {
        let bids = vec![leak("fake_a", false)];
        let (status, _, body) = list(
            &bids,
            &empty_aliases(),
            &nothing_disabled(),
            Some("yep"),
            None,
        );
        assert_eq!(status, 400);
        assert_eq!(body, BAD_QUERY_BODY_ENABLEDONLY);
    }

    #[test]
    fn detail_unknown_returns_404_empty() {
        let bids = vec![leak("fake_a", false)];
        let (status, _, body) =
            detail(&bids, &empty_aliases(), &nothing_disabled(), "fake_missing");
        assert_eq!(status, 404);
        assert!(body.is_empty());
    }

    #[test]
    fn detail_all_returns_aggregate() {
        let bids = vec![leak("fake_a", false), leak("fake_b", true)];
        let (status, _, body) = detail(&bids, &empty_aliases(), &nothing_disabled(), "all");
        assert_eq!(status, 200);
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(value.get("fake_a").is_some() && value.get("fake_b").is_some());
    }

    #[test]
    fn detail_alias_returns_core_shape_plus_alias_of() {
        let bids = vec![leak("fake_aaa", false)];
        let mut aliases = BTreeMap::new();
        aliases.insert("fake_myalias".to_owned(), "fake_aaa".to_owned());
        let (status, _, body) = detail(&bids, &aliases, &nothing_disabled(), "fake_myalias");
        assert_eq!(status, 200);
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value.get("aliasOf").and_then(|x| x.as_str()),
            Some("fake_aaa"),
        );
    }

    #[test]
    fn info_bidders_lists_runtime_added_aliases() {
        // The overlay carries a resolvable alias and a typo. Listing the
        // typo would advertise a name that `detail` immediately 404s, so
        // `effective_host_aliases` has to drop it on the way in.
        let catalog = catalog_from(
            "[aliases]\nfake_runtime_alias = \"fake_bbb\"\nfake_ghost_alias = \"fake_no_such_bidder\"\n",
        );
        let bidders = &bidders();
        let (status, _, body) = list(
            bidders,
            &catalog.aliases,
            &catalog.disabled,
            Some("false"),
            Some("false"),
        );
        assert_eq!(status, 200);
        let names = names_from(&body);
        assert!(
            names.iter().any(|name| name == "fake_runtime_alias"),
            "an alias pushed through the `[aliases]` overlay must reach /info/bidders, got {names:?}",
        );
        assert!(
            !names.iter().any(|name| name == "fake_ghost_alias"),
            "an alias whose base the registry does not carry must not be advertised, got {names:?}",
        );
        let (detail_status, _, _) = detail(
            bidders,
            &catalog.aliases,
            &catalog.disabled,
            "fake_ghost_alias",
        );
        assert_eq!(
            detail_status, 404,
            "list and detail must agree: an unadvertised alias is also unknown to detail",
        );
    }

    #[test]
    fn info_bidders_omits_runtime_disabled_bidders_when_enabledonly_yes() {
        let catalog = catalog_from("[bidders.fake_bbb]\nenabled = false\n");
        let bidders = &bidders();
        let (status, _, body) = list(
            bidders,
            &catalog.aliases,
            &catalog.disabled,
            Some("true"),
            Some("false"),
        );
        assert_eq!(status, 200);
        let names = names_from(&body);
        assert!(
            !names.iter().any(|name| name == "fake_bbb"),
            "a bidder the overlay disabled must not survive ?enabledonly=true, got {names:?}",
        );
        // The filter is the overlay's opinion, not a blanket emptying.
        assert!(
            names.iter().any(|name| name == "fake_aaa"),
            "sibling bidders the overlay left alone must still be listed, got {names:?}",
        );
    }

    #[test]
    fn info_bidder_by_name_returns_alias_metadata_from_runtime_overlay() {
        let catalog = catalog_from("[aliases]\nfake_runtime_alias = \"fake_bbb\"\n");
        let bidders = &bidders();
        let (status, _, body) = detail(
            bidders,
            &catalog.aliases,
            &catalog.disabled,
            "fake_runtime_alias",
        );
        assert_eq!(
            status, 200,
            "an overlay-declared alias must resolve on /info/bidders/<name>",
        );
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value.get("aliasOf").and_then(serde_json::Value::as_str),
            Some("fake_bbb"),
        );
        assert_eq!(
            value.get("status").and_then(serde_json::Value::as_str),
            Some("ACTIVE"),
        );
    }

    #[test]
    fn overlay_disabled_bidder_reports_disabled_on_its_own_detail_row() {
        // The other half of the effective-disabled contract: filtering a
        // bidder out of `?enabledonly=true` while its own row still said
        // ACTIVE would make the two endpoints disagree.
        let bids = vec![leak("fake_bbb", false)];
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_bbb"));
        let (_, _, body) = detail(&bids, &empty_aliases(), &disabled, "fake_bbb");
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value.get("status").and_then(serde_json::Value::as_str),
            Some("DISABLED"),
        );
    }

    #[test]
    fn alias_disabled_by_its_own_name_reports_disabled_while_its_base_stays_active() {
        let bids = vec![leak("fake_aaa", false)];
        let mut aliases = BTreeMap::new();
        aliases.insert("fake_myalias".to_owned(), "fake_aaa".to_owned());
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_myalias"));

        let (_, _, alias_body) = detail(&bids, &aliases, &disabled, "fake_myalias");
        let alias_value: serde_json::Value = serde_json::from_slice(&alias_body).unwrap();
        assert_eq!(
            alias_value
                .get("status")
                .and_then(serde_json::Value::as_str),
            Some("DISABLED"),
        );

        let (_, _, base_body) = detail(&bids, &aliases, &disabled, "fake_aaa");
        let base_value: serde_json::Value = serde_json::from_slice(&base_body).unwrap();
        assert_eq!(
            base_value.get("status").and_then(serde_json::Value::as_str),
            Some("ACTIVE"),
            "disabling an alias by name must not disable the base every sibling shares",
        );
    }

    /// `status` on each row of the `all` aggregate, which
    /// `detail_all_returns_aggregate` cannot see: it asserts only that
    /// keys exist, and it runs against an empty overlay set, so the whole
    /// aggregate could ignore the overlay and still pass. Both baked and
    /// overlay bidders are present here so a row that reports the wrong
    /// status is distinguishable from one the aggregate simply omits.
    #[test]
    fn detail_all_reports_effective_disabled_on_base_and_alias_rows() {
        let bids = vec![leak("fake_aaa", false), leak("fake_bbb", false)];
        let mut aliases = BTreeMap::new();
        aliases.insert("fake_alias_of_aaa".to_owned(), "fake_aaa".to_owned());
        aliases.insert("fake_alias_by_name".to_owned(), "fake_bbb".to_owned());
        // `fake_aaa` is killed by the overlay; `fake_alias_by_name` is killed
        // under its OWN name while its base `fake_bbb` stays live.
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_aaa"));
        disabled.insert(BidderName::new("fake_alias_by_name"));

        let (status, _, body) = detail(&bids, &aliases, &disabled, "all");
        assert_eq!(status, 200);
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let row_status = |name: &str| -> Option<String> {
            value.get(name)?.get("status")?.as_str().map(str::to_owned)
        };

        assert_eq!(
            row_status("fake_aaa").as_deref(),
            Some("DISABLED"),
            "the aggregate must honour the overlay kill switch, not just the baked bit",
        );
        assert_eq!(
            row_status("fake_bbb").as_deref(),
            Some("ACTIVE"),
            "a bidder the overlay left alone must stay ACTIVE, so the row above is not just \
             everything reporting DISABLED",
        );
        assert_eq!(
            row_status("fake_alias_of_aaa").as_deref(),
            Some("DISABLED"),
            "an alias inherits its base's effective status in the aggregate",
        );
        assert_eq!(
            row_status("fake_alias_by_name").as_deref(),
            Some("DISABLED"),
            "an alias disabled by its OWN name is disabled in the aggregate even though its \
             base is live",
        );
    }

    /// `?enabledonly=true` visibility for an alias the operator disabled
    /// BY NAME. The base is deliberately left enabled, so a check that
    /// consults only the base still lists the alias.
    #[test]
    fn list_omits_alias_disabled_by_its_own_name_when_enabledonly_yes() {
        let bids = vec![leak("fake_aaa", false)];
        let mut aliases = BTreeMap::new();
        aliases.insert("fake_alias_by_name".to_owned(), "fake_aaa".to_owned());
        let mut disabled = BTreeSet::new();
        disabled.insert(BidderName::new("fake_alias_by_name"));

        let (status, _, body) = list(&bids, &aliases, &disabled, Some("true"), Some("false"));
        assert_eq!(status, 200);
        let names = names_from(&body);
        assert!(
            !names.iter().any(|name| name == "fake_alias_by_name"),
            "an alias the overlay disabled by name must not survive ?enabledonly=true, \
             got {names:?}",
        );
        assert!(
            names.iter().any(|name| name == "fake_aaa"),
            "its base was never disabled and must still be listed, got {names:?}",
        );
    }
}

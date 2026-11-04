use crate::fanout_limits::limits as baseline_limits;
use auctioneer_bidder::{
    schema::{native_schema_available, native_schema_required},
    test_support::{
        privacy_wire::PrivacyWireContract,
        serving_catalog::{self, ServingBidder},
    },
};
use auctioneer_config_data::types::BidderInfo;
use auctioneer_runtime::ResolvedIsolateLimits;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    str,
};
use url::{Url, form_urlencoded};

#[derive(Deserialize)]
struct Approval {
    schema_version: u32,
    review_status: String,
    reviewer: String,
    contracts: Vec<Contract>,
}

#[derive(Deserialize)]
struct Contract {
    #[serde(rename = "source_path")]
    path: String,
    #[serde(rename = "source_bytes")]
    bytes: u64,
    #[serde(rename = "source_sha256")]
    sha256: String,
}

#[derive(Deserialize)]
struct Wave {
    bidders: Vec<Member>,
}

#[derive(Deserialize)]
struct Member {
    name: String,
    implementation: String,
    base: Option<String>,
}

#[derive(Clone, Deserialize)]
pub(super) struct FactoryCase {
    pub name: String,
    pub request: Value,
    pub context: Value,
    pub outbound: Vec<Outbound>,
    pub responses: Vec<FixtureResponse>,
}

#[derive(Deserialize)]
struct Factory {
    schema_version: u32,
    implementation: String,
    cases: Vec<FactoryCase>,
}

#[derive(Clone, Deserialize)]
pub(super) struct Outbound {
    pub method: String,
    pub url: String,
    pub body: Value,
    pub imp_ids: Vec<String>,
    #[serde(default)]
    pub package_version_paths: Vec<String>,
    #[serde(default)]
    pub required_headers: Vec<(String, String)>,
    #[serde(default)]
    pub absent_headers: Vec<String>,
}

#[derive(Clone, Deserialize)]
pub(super) struct FixtureResponse {
    pub call: usize,
    pub status: u16,
    pub body: Value,
    pub expected: Expected,
}

#[derive(Clone, Deserialize)]
pub(super) struct Expected {
    pub kind: String,
    #[serde(default)]
    pub bids: Vec<ExpectedBid>,
    #[serde(default)]
    pub errors: Vec<u32>,
}

#[derive(Clone, Deserialize)]
pub(super) struct ExpectedBid {
    pub id: String,
    pub impid: String,
    pub media_type: String,
}

pub(super) struct Flow {
    pub info: &'static BidderInfo,
    pub ancestry: Vec<&'static BidderInfo>,
    pub native_admission: bool,
    pub case: FactoryCase,
    pub ids: BTreeMap<String, String>,
    pub responses: Vec<FixtureResponse>,
    pub privacy_contract: Option<PrivacyWireContract>,
}

impl Flow {
    pub fn name(&self) -> &str {
        &self.info.name
    }

    pub fn account(&self) -> &str {
        self.case.context["account_id"]
            .as_str()
            .expect("factory account id")
    }

    pub fn default_active(&self) -> bool {
        self.ancestry
            .iter()
            .all(|info| !info.disabled && info.default_enabled != Some(false))
    }

    pub fn settings(&self) -> Option<&Value> {
        self.case
            .context
            .get("settings")
            .filter(|value| !value.is_null())
    }

    pub fn credential(&self) -> Option<(&str, &Value)> {
        self.case
            .context
            .get("credentials")
            .filter(|value| !value.is_null())
            .map(|bundle| {
                let reference = self
                    .settings()
                    .and_then(|settings| settings.pointer("/values/credentials_secret"))
                    .and_then(Value::as_str)
                    .expect("credentialed owner declares its normal settings reference");
                (reference, bundle)
            })
    }

    pub fn covered(&self, call: usize) -> Vec<String> {
        self.case.outbound[call]
            .imp_ids
            .iter()
            .map(|id| {
                self.ids
                    .get(id)
                    .expect("owner call covers an owner impression")
                    .clone()
            })
            .collect()
    }
}

fn owner_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../auctioneer-bidder")
        .canonicalize()
        .unwrap()
}

fn approved_members(bidder_root: &Path) -> BTreeMap<String, Member> {
    let wave_root = bidder_root
        .join("tests/conformance/fixtures/waves")
        .canonicalize()
        .unwrap();
    let approval: Approval =
        toml::from_str(&fs::read_to_string(wave_root.join("priority-approval.toml")).unwrap())
            .unwrap();
    assert_eq!(approval.schema_version, 1);
    assert_eq!(approval.review_status, "approved");
    assert_eq!(approval.reviewer, "AG");
    assert_eq!(
        approval.contracts.len(),
        2,
        "source and supplemental approval witnesses"
    );
    let mut members = BTreeMap::new();
    for witness in approval.contracts {
        let path = wave_root.join(&witness.path).canonicalize().unwrap();
        assert_eq!(
            path.parent(),
            Some(wave_root.as_path()),
            "wave cannot escape its owner"
        );
        let bytes = fs::read(path).unwrap();
        assert_eq!(u64::try_from(bytes.len()).unwrap(), witness.bytes);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            witness.sha256,
            "reviewed requirements changed; do not refresh this digest automatically"
        );
        let wave: Wave = toml::from_str(str::from_utf8(&bytes).unwrap()).unwrap();
        for member in wave.bidders {
            assert!(
                members.insert(member.name.clone(), member).is_none(),
                "duplicate selected identity"
            );
        }
    }
    assert_eq!(
        members.len(),
        14,
        "all frozen priority identities including the supplemental member"
    );
    members
}

pub(super) fn flows() -> Vec<Flow> {
    let bidder_root = owner_root();
    let members = approved_members(&bidder_root);
    let published = serving_catalog::published();
    members
        .into_values()
        .enumerate()
        .map(|(ordinal, member)| flow_from_member(&bidder_root, &published, ordinal, member))
        .collect()
}

fn flow_from_member(
    bidder_root: &Path,
    published: &[ServingBidder],
    ordinal: usize,
    member: Member,
) -> Flow {
    let row = published
        .iter()
        .find(|row| row.name() == member.name)
        .expect("every priority flow is published");
    assert_eq!(row.implementation, member.implementation);
    let mut ancestry = Vec::new();
    let mut info = row.info;
    loop {
        assert!(!info.disabled, "source safety veto cannot be waived");
        assert!(
            ancestry.len() < auctioneer_config_data::bidders().len(),
            "compiled alias ancestry is acyclic"
        );
        ancestry.push(info);
        match info.alias_of.as_deref() {
            Some(parent) => {
                info = auctioneer_config_data::bidder(parent).expect("compiled ancestor metadata");
            }
            None => break,
        }
    }
    let native_admission = native_schema_required(&info.name);
    assert!(
        native_schema_available(&info.name),
        "every selected schema owner is available on the funded native path"
    );
    if native_admission {
        assert!(
            ancestry
                .iter()
                .any(|ancestor| ancestor.default_enabled == Some(false)),
            "new controlled-native flows are inactive without runtime opt-in"
        );
    }
    if let Some(base) = member.base {
        assert_eq!(info.name, base);
    }
    let source_root = bidder_root.join("src/bidders").canonicalize().unwrap();
    let owner = source_root.join(row.implementation).canonicalize().unwrap();
    assert_eq!(owner.parent(), Some(source_root.as_path()));
    let fixture_path = owner.join("fixtures/factory.json").canonicalize().unwrap();
    assert!(fixture_path.starts_with(&owner));
    let fixture: Factory = serde_json::from_slice(&fs::read(fixture_path).unwrap()).unwrap();
    let privacy_contract = owner.join("fixtures/privacy.json").exists().then(|| {
        row.privacy_wire_contract()
            .expect("valid owner privacy facts")
    });
    assert_eq!(fixture.schema_version, 1);
    assert_eq!(fixture.implementation, member.implementation);
    let case = fixture
        .cases
        .into_iter()
        .next()
        .expect("owner first factory case");
    assert!(!case.outbound.is_empty());
    let ids: BTreeMap<_, _> = case.request["imp"]
        .as_array()
        .unwrap()
        .iter()
        .map(|imp| {
            let id = imp["id"].as_str().unwrap();
            (id.to_owned(), format!("onboarding_{ordinal}_{id}"))
        })
        .collect();
    assert_eq!(ids.len(), case.request["imp"].as_array().unwrap().len());
    let responses = positive_responses(&case, &ids);
    Flow {
        info: row.info,
        ancestry,
        native_admission,
        case,
        ids,
        responses,
        privacy_contract,
    }
}

fn positive_responses(case: &FactoryCase, ids: &BTreeMap<String, String>) -> Vec<FixtureResponse> {
    (0..case.outbound.len())
        .map(|call| {
            let mut response = case
                .responses
                .iter()
                .find(|response| {
                    response.call == call
                        && response.status == 200
                        && response.expected.kind == "ok"
                        && !response.expected.bids.is_empty()
                        && response.expected.errors.is_empty()
                })
                .expect("each real owner call has a positive error-free response witness")
                .clone();
            for seat in response.body["seatbid"].as_array_mut().unwrap() {
                for bid in seat["bid"].as_array_mut().unwrap() {
                    let original = bid["impid"].as_str().unwrap();
                    if let Some(id) = ids.get(original) {
                        bid["impid"] = json!(id);
                    }
                }
            }
            for bid in &mut response.expected.bids {
                bid.impid = ids
                    .get(&bid.impid)
                    .expect("expected bid has owner coverage")
                    .clone();
            }
            response
        })
        .collect()
}

/// Replay only independently authored changes, never observations of a factory.
/// Objects completely replaced by the witness stay replaced. Unchanged fields
/// instead inherit the composed auction's context (including a valid sibling's).
fn contextualize(
    source: Option<&Value>,
    golden: Option<&Value>,
    input: Option<&Value>,
) -> Option<Value> {
    if source == golden {
        return input.cloned();
    }
    let golden_value = golden?;
    let Some(fields) = golden_value.as_object() else {
        return Some(golden_value.clone());
    };
    let source_fields = source.and_then(Value::as_object);
    if source_fields
        .is_some_and(|old| !old.is_empty() && !old.keys().any(|key| fields.contains_key(key)))
    {
        return Some(golden_value.clone());
    }
    let mut result = input
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(old) = source_fields {
        for key in old.keys().filter(|key| !fields.contains_key(*key)) {
            result.remove(key);
        }
    }
    for (key, value) in fields {
        let old = source_fields.and_then(|old| old.get(key));
        let current = input.and_then(|object| object.get(key));
        match contextualize(old, Some(value), current) {
            Some(projected) => {
                result.insert(key.clone(), projected);
            }
            None => {
                result.remove(key);
            }
        }
    }
    Some(Value::Object(result))
}

fn exchange_context(input: &Value) -> Value {
    let mut context = input.clone();
    // Independent literal expectations for prepare/implicit_fill, before any
    // owner transformation. Do not call preparation or inspect outbound data.
    let fields = context.as_object_mut().unwrap();
    if fields.get("at").is_none_or(Value::is_null) || fields.get("at") == Some(&json!(0_i32)) {
        fields.insert("at".into(), json!(1_i32));
    }
    if fields.get("device").is_none_or(Value::is_null) {
        fields.insert("device".into(), json!({}));
    }
    if fields.get("app").is_none_or(Value::is_null) && fields.get("dooh").is_none_or(Value::is_null)
    {
        if fields.get("site").is_none_or(Value::is_null) {
            fields.insert("site".into(), json!({}));
        }
        let site = fields.get_mut("site").unwrap().as_object_mut().unwrap();
        if site.get("ext").is_none_or(Value::is_null) {
            site.insert("ext".into(), json!({}));
        }
        let ext = site.get_mut("ext").unwrap().as_object_mut().unwrap();
        if ext.get("amp").is_none_or(Value::is_null) {
            ext.insert("amp".into(), json!(0_i32));
        }
    }
    if fields.get("app").is_some_and(|app| !app.is_null()) {
        if fields.get("ext").is_none_or(Value::is_null) {
            fields.insert("ext".into(), json!({}));
        }
        let ext = fields.get_mut("ext").unwrap().as_object_mut().unwrap();
        if ext.get("prebid").is_none_or(Value::is_null) {
            ext.insert("prebid".into(), json!({}));
        }
        let prebid = ext.get_mut("prebid").unwrap().as_object_mut().unwrap();
        if prebid.get("channel").is_none_or(Value::is_null) {
            prebid.insert("channel".into(), json!({"name":"app"}));
        }
    }
    context
}

fn inventory_after_preservation(
    source: Option<&Value>,
    golden: Option<&Value>,
    preserved: &Value,
) -> Value {
    let mut expected = contextualize(source, golden, Some(preserved))
        .expect("owner inventory projection retains the selected inventory");
    for field in ["publisher", "ext"] {
        if let Some(value) = golden.and_then(|inventory| inventory.get(field))
            && source.and_then(|inventory| inventory.get(field)) != Some(value)
        {
            expected[field] = value.clone();
        }
    }
    expected
}

pub(super) fn expected_body(flow: &Flow, ordinal: usize, input: &Value) -> Value {
    let outbound = &flow.case.outbound[ordinal];
    let mut golden = outbound.body.clone();
    for path in &outbound.package_version_paths {
        let version = golden
            .pointer_mut(path)
            .expect("declared owner version path");
        assert_eq!(version.as_str(), Some("$CARGO_PKG_VERSION"));
        *version = json!(env!("CARGO_PKG_VERSION"));
    }
    let imps = golden["imp"].as_array_mut().unwrap();
    for imp in imps.iter_mut() {
        let old_id = imp["id"].as_str().unwrap().to_owned();
        let original = flow.case.request["imp"]
            .as_array()
            .unwrap()
            .iter()
            .find(|original| original["id"] == old_id)
            .unwrap();
        imp["id"] = json!(flow.ids[&old_id]);
        // Public ingress's typed bidder envelope survives splitting as an
        // empty prebid object only for the witnessed passthrough extension.
        if imp.pointer("/ext/bidder").is_some()
            && imp.pointer("/ext/bidder") == original.pointer("/ext/bidder")
        {
            imp["ext"]["prebid"] = json!({});
        }
    }
    let projected_imps = golden.as_object_mut().unwrap().remove("imp").unwrap();
    let mut source = flow.case.request.clone();
    source.as_object_mut().unwrap().remove("imp");
    let mut context = exchange_context(input);
    context.as_object_mut().unwrap().remove("imp");
    let mut expected = contextualize(Some(&source), Some(&golden), Some(&context)).unwrap();
    expected["imp"] = projected_imps;
    if let Some(facts) = &flow.privacy_contract {
        match facts
            .request_extension(context.get("ext"), flow.name())
            .unwrap()
        {
            Some(ext) => expected["ext"] = ext,
            None => {
                expected.as_object_mut().unwrap().remove("ext");
            }
        }
    }
    for inventory in ["site", "app"] {
        if let Some(input_inventory) = context.get(inventory) {
            let preserved = flow.privacy_contract.as_ref().map_or_else(
                || input_inventory.clone(),
                |facts| {
                    facts
                        .preserved_value(&context, &format!("/{inventory}"))
                        .unwrap()
                        .unwrap()
                },
            );
            // Preservation supplies the shared auction context first. The
            // authored protocol delta then remains authoritative over it.
            expected[inventory] = inventory_after_preservation(
                source.get(inventory),
                golden.get(inventory),
                &preserved,
            );
        }
    }
    expected
}

/// The fixture supplies protocol suffixes and macro values; published metadata
/// supplies the real HTTPS destination. Synthetic factory endpoint queries are
/// not production protocol additions.
pub(super) fn expected_url(flow: &Flow, ordinal: usize) -> String {
    let outbound = &flow.case.outbound[ordinal];
    let mut template = flow.case.context["endpoint_template"].as_str().unwrap();
    let mut remainder = outbound.url.as_str();
    let mut bindings = BTreeMap::new();
    while let Some((literal, rest)) = template.split_once("{{.") {
        remainder = remainder
            .strip_prefix(literal)
            .expect("fixture URL matches its endpoint context");
        let (name, after) = rest.split_once("}}").unwrap();
        let next_literal = after.split("{{.").next().unwrap();
        let end = if next_literal.is_empty() {
            remainder.find(['?', '&', '#']).unwrap_or(remainder.len())
        } else {
            remainder
                .find(next_literal)
                .expect("fixture macro terminator")
        };
        let (binding_value, tail) = remainder
            .split_at_checked(end)
            .expect("fixture macro ends at a UTF-8 boundary");
        bindings.insert(name.to_owned(), binding_value.to_owned());
        remainder = tail;
        template = after;
    }
    let suffix = remainder
        .strip_prefix(template)
        .expect("fixture endpoint suffix");
    let mut endpoint = flow.info.endpoint.clone();
    if endpoint.contains("{{.Host}}") && !bindings.contains_key("Host") {
        let region = flow
            .settings()
            .and_then(|settings| settings.pointer("/values/region"))
            .and_then(Value::as_str)
            .expect("explicit regional routing control");
        let hosts: Vec<_> = flow
            .info
            .host_allowlist
            .as_ref()
            .unwrap()
            .iter()
            .filter(|host| {
                host.split('.')
                    .next()
                    .unwrap()
                    .ends_with(&format!("-{region}"))
            })
            .collect();
        assert_eq!(
            hosts.len(),
            1,
            "exactly one complete published host for the authored region"
        );
        bindings.insert("Host".into(), hosts[0].clone());
    }
    for (name, value) in bindings {
        endpoint = endpoint.replace(&format!("{{{{.{name}}}}}"), &value);
    }
    assert!(
        !endpoint.contains("{{"),
        "every published endpoint macro has an independent witness"
    );
    let mut url = Url::parse(&endpoint).unwrap();
    if !suffix.is_empty() {
        assert!(
            suffix.starts_with('?') || suffix.starts_with('&'),
            "only witnessed query additions are admitted"
        );
        url.query_pairs_mut().extend_pairs(form_urlencoded::parse(
            suffix.as_bytes().get(1..).expect("nonempty query suffix"),
        ));
    }
    url.to_string()
}

#[test]
fn authored_changes_survive_composed_context_without_borrowing_actual_output() {
    assert_eq!(
        inventory_after_preservation(
            Some(&json!({"id":"fake-original-inventory"})),
            Some(&json!({"id":"fake-parameter-inventory"})),
            &json!({"publisher":{"id":"fake-preserved-publisher"},"ext":{"amp":0_i32}}),
        ),
        json!({"id":"fake-parameter-inventory","publisher":{"id":"fake-preserved-publisher"},"ext":{"amp":0_i32}}),
    );
    assert_eq!(
        inventory_after_preservation(
            None,
            Some(&json!({"id":"fake-added-inventory"})),
            &json!({"publisher":{"id":"fake-preserved-publisher"}})
        ),
        json!({"id":"fake-added-inventory","publisher":{"id":"fake-preserved-publisher"}}),
    );
    let bare = json!({"id":"fake_request","site":{"publisher":{"id":"fake_account"}}});
    assert_eq!(
        exchange_context(&bare),
        json!({"id":"fake_request","at":1_i32,"device":{},
        "site":{"publisher":{"id":"fake_account"},"ext":{"amp":0_i32}}})
    );
    assert!(
        bare.get("at").is_none(),
        "context expectations do not mutate ingress"
    );
    let explicit =
        json!({"at":2_i32,"device":{"make":"Example"},"site":{"ext":{"amp":1_i32,"keep":true}}});
    assert_eq!(exchange_context(&explicit), explicit);
    assert_eq!(
        exchange_context(&json!({"at":0_i32,"app":{}})),
        json!({"at":1_i32,"app":{},"device":{},"ext":{"prebid":{"channel":{"name":"app"}}}})
    );
    let source = json!({"id":"old", "ext":{"keep":1_i32,"old":2_i32}});
    let golden = json!({"id":"old", "ext":{"keep":1_i32,"protocol":3_i32}});
    let input =
        json!({"id":"sibling", "tmax":1_000_i32,"ext":{"keep":4_i32,"old":2_i32,"context":5_i32}});
    assert_eq!(
        contextualize(Some(&source), Some(&golden), Some(&input)).unwrap(),
        json!({"id":"sibling","tmax":1_000_i32,"ext":{"keep":4_i32,"protocol":3_i32,"context":5_i32}})
    );
    assert_eq!(
        contextualize(
            Some(&json!({"discard":1_i32})),
            Some(&json!({"protocol":2_i32})),
            Some(&json!({"discard":9_i32,"leak":3_i32}))
        ),
        Some(json!({"protocol":2_i32}))
    );
}

pub(super) fn input(flows: &[&Flow]) -> Value {
    let first = flows.first().expect("at least one requested flow");
    let mut request = first.case.request.clone();
    request["tmax"] = json!(1_000_i32);
    request["cur"] = json!(["USD"]);
    if request.get("site").is_none() && request.get("app").is_none() {
        request["site"] = json!({});
    }
    let inventory = if request.get("site").is_some() {
        "site"
    } else {
        "app"
    };
    if request[inventory].get("publisher").is_none() {
        request[inventory]["publisher"] = json!({});
    }
    request[inventory]["publisher"]["id"] = json!(first.account());
    // Public ingress wraps unchanged owner parameters under the observable
    // identity. Only correlation IDs are namespaced for composed requests.
    request["imp"] = Value::Array(
        flows
            .iter()
            .flat_map(|flow| {
                flow.case.request["imp"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|source| {
                        let mut imp = source.clone();
                        imp["id"] = json!(flow.ids[imp["id"].as_str().unwrap()]);
                        let ext = imp["ext"].as_object_mut().unwrap();
                        let params = ext.remove("bidder").expect(
                            "owner parameters are data, not rewritten for schema admission",
                        );
                        let prebid = ext
                            .entry("prebid")
                            .or_insert_with(|| json!({}))
                            .as_object_mut()
                            .unwrap();
                        prebid.insert("bidder".into(), json!({(flow.name()):params}));
                        imp
                    })
                    .collect::<Vec<_>>()
            })
            .collect(),
    );
    request
}

pub(super) fn config(flows: &[&Flow]) -> Value {
    let limits = limits();
    let mut bidders = Map::new();
    for flow in flows {
        for ancestor in &flow.ancestry {
            bidders
                .entry(ancestor.name.clone())
                .or_insert_with(|| json!({"enabled":true}));
        }
        if let Some(settings) = flow.settings() {
            bidders.get_mut(flow.name()).unwrap()["settings"] = settings.clone();
        }
    }
    json!({
        "schema_version": 1_u64,
        "host": {
            "gdpr": {"enabled":false},
            "resource_limits": {
                "default_tmax_ms": 1_000_i32,
                "max_tmax_ms": 1_000_i32,
                "max_bidders_per_auction": limits.max_bidders_per_auction,
                "max_outbound_requests": limits.max_outbound_requests,
                "max_outbound_raw_bytes": limits.max_outbound_raw_bytes,
                "max_outbound_request_bytes": limits.max_outbound_request_bytes,
                "max_transport_bytes": limits.max_transport_bytes,
                "max_decoded_bytes": limits.max_decoded_bytes,
                "max_parsed_nodes": limits.max_parsed_nodes,
            },
        },
        "features": {"gzip_outbound":true},
        "bidders": bidders,
    })
}

pub(super) fn limits() -> ResolvedIsolateLimits {
    let mut limits = baseline_limits();
    // This isolated cohort includes multi-call owners beside healthy siblings.
    // Match the shipped native call ceiling, not the two-call fake fixture.
    limits.max_outbound_requests = 16;
    limits
}

pub(super) fn selected_names(flows: &[&Flow]) -> BTreeSet<String> {
    flows.iter().map(|flow| flow.name().to_owned()).collect()
}

use super::fixtures::{Flow, config};
use auctioneer_config_data::usersync::SyncType;
use auctioneer_core::{
    handlers::usersync_keys::KEYRING_SECRET_NAME,
    host_primitives::{boot_gates, clock_handle},
    test_support::{config_store::runtime_registry, live_catalog::SyncRole, usersync_keys},
    usersync::{
        cookie::MAX_SET_COOKIE_FIELD_BYTES,
        limits::CookieLimits,
        signed::{MAX_KEYRING_BYTES, SignedCookieError},
    },
};
use auctioneer_runtime::aux_work::{
    MAX_SYNC_OUTPUT_BYTES, MAX_SYNC_RESPONSE_HEADER_BYTES, MAX_SYNC_RESPONSE_HEADER_COUNT,
    MAX_SYNC_URL_BYTES,
};
use bytes::Bytes;
use cookie::{Cookie, SameSite};
use edgezero_core::{
    app::App,
    body::Body,
    config_store::BoundedStoreRead,
    http::{HeaderMap, Method, StatusCode, request_builder},
    ingress::{IngressDispatchOutcome, IngressFraming, IngressHeadAccounting},
    secret_store::{SecretError, SecretHandle, SecretStore},
    store_registry::{BoundSecretStore, SecretRegistry, StoreRegistry},
    time::{Deadline, MonotonicClock, MonotonicInstant},
};
use futures::executor::block_on;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use url::{Url, form_urlencoded::byte_serialize};

const ORIGIN: &str = "https://publisher.example";
const CALLBACK_ORIGIN: &str = "https://onboarding-sync.example.test";
const UID: &str = "native+uid&=signed/fixture";

struct SigningStore {
    available: bool,
    reads: AtomicUsize,
}

impl SigningStore {
    fn new(available: bool) -> Arc<Self> {
        Arc::new(Self {
            available,
            reads: AtomicUsize::new(0),
        })
    }
}

#[async_trait::async_trait(?Send)]
impl SecretStore for SigningStore {
    async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
        panic!("generated sync routes must use the bounded keyring read")
    }

    async fn get_bytes_bounded(
        &self,
        store_name: &str,
        key: &str,
        clock: &MonotonicClock,
        deadline: Deadline,
        max_backend_bytes: u64,
        max_value_bytes: u64,
    ) -> Result<BoundedStoreRead<Bytes>, SecretError> {
        assert_eq!(store_name, "onboarding_signing_store");
        assert_eq!(key, KEYRING_SECRET_NAME);
        let cap = u64::try_from(MAX_KEYRING_BYTES).unwrap();
        assert_eq!((max_backend_bytes, max_value_bytes), (cap, cap));
        assert!(!deadline.is_expired_at(clock.now()));
        self.reads.fetch_add(1, Ordering::SeqCst);
        if !self.available {
            return Err(SecretError::Unavailable);
        }
        let value = Bytes::from_static(usersync_keys::SECRET_JSON.as_bytes());
        let backend_bytes = u64::try_from(value.len()).unwrap();
        assert!(backend_bytes <= cap);
        Ok(BoundedStoreRead {
            backend_bytes,
            value: Some(value),
        })
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    fn browser_headers(&self) {
        assert_eq!(self.headers["cache-control"], "no-store");
        assert_eq!(self.headers["access-control-allow-origin"], ORIGIN);
        assert_eq!(self.headers["access-control-allow-credentials"], "true");
        assert!(
            self.headers["vary"]
                .to_str()
                .unwrap()
                .split(',')
                .any(|field| field.trim().eq_ignore_ascii_case("origin"))
        );
    }

    fn no_cookie(&self) {
        assert!(!self.headers.contains_key("set-cookie"));
    }
}

fn dispatch(
    app: &App,
    runtime: &Value,
    store: &Arc<SigningStore>,
    method: Method,
    path: &str,
    request_body: &[u8],
    cookie: Option<&str>,
) -> Reply {
    let preflight = method == Method::OPTIONS;
    let mut builder = request_builder()
        .method(method)
        .uri(path)
        .header("origin", ORIGIN)
        .header("content-type", "application/json")
        .header("user-agent", "Chrome/120.0");
    if preflight {
        builder = builder
            .header("access-control-request-method", "POST")
            .header("access-control-request-headers", "content-type");
    }
    if let Some(cookie_header) = cookie {
        builder = builder.header("cookie", cookie_header);
    }
    let mut request = builder
        .body(Body::from_bytes(request_body.to_vec()))
        .unwrap();
    request.extensions_mut().insert(runtime_registry(runtime));
    let provider: Arc<dyn SecretStore> = Arc::<SigningStore>::clone(store);
    let registry: SecretRegistry = StoreRegistry::single_id(
        "onboarding_signing".into(),
        BoundSecretStore::new(
            SecretHandle::new(provider),
            "onboarding_signing_store".into(),
        ),
    );
    request.extensions_mut().insert(registry);
    let outcome = block_on(app.dispatch_ingress(
        request,
        MonotonicInstant::now(),
        IngressHeadAccounting::HostManaged,
        IngressFraming::HostManaged,
    ))
    .unwrap();
    let IngressDispatchOutcome::Response(mut envelope) = outcome else {
        panic!("generated sync route must answer within the installed envelope");
    };
    let status = envelope.response_mut().status();
    let headers = envelope.response_mut().headers().clone();
    let body = envelope
        .response_mut()
        .body_mut()
        .as_bytes()
        .expect("buffered sync response")
        .to_vec();
    assert!(u64::try_from(body.len()).unwrap() <= MAX_SYNC_OUTPUT_BYTES);
    assert!(u32::try_from(headers.len()).unwrap() <= MAX_SYNC_RESPONSE_HEADER_COUNT);
    assert!(
        u64::try_from(
            headers
                .iter()
                .try_fold(0_usize, |total, (name, value)| total
                    .checked_add(name.as_str().len())?
                    .checked_add(value.len()))
                .unwrap()
        )
        .unwrap()
            <= MAX_SYNC_RESPONSE_HEADER_BYTES
    );
    drop(envelope);
    Reply {
        status,
        headers,
        body,
    }
}

fn runtime(flow: &Flow) -> Value {
    let mut runtime = config(&[flow]);
    runtime["user_sync"] = json!({"external_url":CALLBACK_ORIGIN, "allowed_origins":[ORIGIN],
        "cooperative":{"enabled_by_default":false}});
    runtime["accounts"] = json!({(flow.account()):{}});
    for ancestor in &flow.ancestry {
        runtime["bidders"][&ancestor.name]["usersync_enabled"] = json!(true);
    }
    runtime
}

fn payload(flow: &Flow, kind: Option<SyncType>) -> Vec<u8> {
    let mut request = json!({"account":flow.account(), "bidders":[flow.name()],
        "gdpr":0_i32, "coopSync":false, "limit":1_i32});
    if let Some(selected_kind) = kind {
        // Redirect is the wire's image mode. Excluding the other mode forces
        // both metadata templates through selection when both are declared.
        request["filterSettings"] = json!({
            "iframe":{"filter":if selected_kind == SyncType::Iframe {"include"} else {"exclude"}, "bidders":"*"},
            "image":{"filter":if selected_kind == SyncType::Redirect {"include"} else {"exclude"}, "bidders":"*"}});
    }
    serde_json::to_vec(&request).unwrap()
}

fn selected(reply: &Reply, role: SyncRole) -> String {
    assert_eq!(
        reply.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&reply.body)
    );
    reply.browser_headers();
    reply.no_cookie();
    let json = reply.json();
    let rows = json["bidder_status"]
        .as_array()
        .expect("actual sync selection rows");
    assert_eq!(rows.len(), 1, "{json}");
    assert_eq!(rows[0]["bidder"], role.name());
    assert_eq!(
        rows[0]["usersync"]["type"],
        match role.kind {
            SyncType::Iframe => "iframe",
            SyncType::Redirect => "redirect",
        }
    );
    let url = rows[0]["usersync"]["url"].as_str().unwrap();
    assert!(u64::try_from(url.len()).unwrap() <= MAX_SYNC_URL_BYTES);
    url.to_owned()
}

fn empty_selection(reply: &Reply) {
    assert_eq!(
        reply.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&reply.body)
    );
    reply.browser_headers();
    reply.no_cookie();
    assert!(reply.json()["bidder_status"].as_array().unwrap().is_empty());
}

fn callback_uri(role: SyncRole, flow: &Flow, rendered: &str) -> String {
    // The shared metadata helper independently checks the provider authority,
    // path and declared callback slot before we follow the emitted callback.
    // Substitute before parsing: a declared token such as `#...` is not a
    // fragment after the partner replaces it with the encoded UID.
    let encoded_uid: String = byte_serialize(UID.as_bytes()).collect();
    let completed = role.callback_uri(rendered, &encoded_uid);
    let mut callback = Url::parse(&format!("{CALLBACK_ORIGIN}{completed}")).unwrap();
    assert_eq!(
        callback.origin(),
        Url::parse(CALLBACK_ORIGIN).unwrap().origin()
    );
    let mut uid_count = 0_usize;
    let pairs: Vec<_> = callback
        .query_pairs()
        .map(|(key, value)| {
            if key == "uid" {
                uid_count = uid_count.checked_add(1).unwrap();
                assert_eq!(value, UID);
                (key.into_owned(), UID.to_owned())
            } else {
                (key.into_owned(), value.into_owned())
            }
        })
        .collect();
    assert_eq!(uid_count, 1);
    assert_eq!(
        pairs
            .iter()
            .filter(|(key, _)| key == "bidder")
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>(),
        [flow.name()]
    );
    assert_eq!(
        pairs
            .iter()
            .filter(|(key, _)| key == "account")
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>(),
        [flow.account()]
    );
    callback
        .query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(key, value)| (key, value)));
    assert!(callback.fragment().is_none());
    assert_eq!(
        callback
            .query_pairs()
            .find(|(key, _)| key == "uid")
            .unwrap()
            .1,
        UID
    );
    let uri = format!("{}?{}", callback.path(), callback.query().unwrap());
    assert!(
        u64::try_from(uri.len()).unwrap()
            <= u64::from(boot_gates().unwrap().target.max_request_target_bytes)
    );
    uri
}

fn carried_cookie(reply: &Reply, role: SyncRole) -> String {
    assert_eq!(
        reply.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&reply.body)
    );
    reply.browser_headers();
    let fields: Vec<_> = reply.headers.get_all("set-cookie").iter().collect();
    assert_eq!(
        fields.len(),
        1,
        "one browser-bounded signed UID transaction"
    );
    let field = fields[0].to_str().unwrap();
    assert!(field.len() <= MAX_SET_COOKIE_FIELD_BYTES);
    let cookie = Cookie::parse(field.to_owned()).unwrap();
    assert_eq!(cookie.name(), "uids");
    assert_eq!(cookie.path(), Some("/"));
    assert_eq!(cookie.secure(), Some(true));
    assert_eq!(cookie.same_site(), Some(SameSite::None));
    assert!(cookie.expires_datetime().unwrap() > clock_handle().now());
    assert!(cookie.value().starts_with("v1.test."));
    let decoded = usersync_keys::decode_cookie(cookie.value(), &CookieLimits::default()).unwrap();
    assert_eq!(decoded.cookie.entries().len(), 1);
    assert_eq!(decoded.cookie.uid(role.family()), Some(UID));
    assert!(decoded.cookie.entries()[role.family()].expires() > clock_handle().now());
    format!("{}={}", cookie.name(), cookie.value())
}

fn tampered_cookie(carried: &str) -> String {
    let signed = carried.strip_prefix("uids=").unwrap();
    let mut fields: Vec<_> = signed.split('.').map(str::to_owned).collect();
    assert_eq!(fields.len(), 4);
    let changed = if fields[3].starts_with('A') { "B" } else { "A" };
    fields[3].replace_range(..1, changed);
    let altered = fields.join(".");
    assert_eq!(
        usersync_keys::decode_cookie(&altered, &CookieLimits::default()),
        Err(SignedCookieError::InvalidCookie)
    );
    format!("uids={altered}")
}

fn round_trip(app: &App, flow: &Flow, role: SyncRole) {
    let runtime = runtime(flow);
    let store = SigningStore::new(true);
    let body = payload(flow, Some(role.kind));
    let preflight = dispatch(
        app,
        &runtime,
        &store,
        Method::OPTIONS,
        "/cookie_sync",
        &[],
        None,
    );
    assert_eq!(preflight.status, StatusCode::NO_CONTENT);
    preflight.browser_headers();
    preflight.no_cookie();
    assert_eq!(store.reads.load(Ordering::SeqCst), 0);
    let offered = dispatch(
        app,
        &runtime,
        &store,
        Method::POST,
        "/cookie_sync",
        &body,
        None,
    );
    let rendered = selected(&offered, role);
    assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    let callback = callback_uri(role, flow, &rendered);
    let written = dispatch(app, &runtime, &store, Method::GET, &callback, &[], None);
    let cookie = carried_cookie(&written, role);
    assert_eq!(store.reads.load(Ordering::SeqCst), 2);
    let readback = dispatch(
        app,
        &runtime,
        &store,
        Method::GET,
        "/getuids",
        &[],
        Some(&cookie),
    );
    assert_eq!(readback.status, StatusCode::OK);
    readback.browser_headers();
    readback.no_cookie();
    assert_eq!(readback.json(), json!({"buyeruids":{(role.family()):UID}}));
    assert_eq!(store.reads.load(Ordering::SeqCst), 3);
    let corrupted = tampered_cookie(&cookie);
    let refused = dispatch(
        app,
        &runtime,
        &store,
        Method::GET,
        "/getuids",
        &[],
        Some(&corrupted),
    );
    assert_eq!(refused.status, StatusCode::BAD_REQUEST);
    refused.no_cookie();
    assert_eq!(refused.body, b"invalid UID cookie");
    assert_eq!(store.reads.load(Ordering::SeqCst), 4);
    let already_live = dispatch(
        app,
        &runtime,
        &store,
        Method::POST,
        "/cookie_sync",
        &body,
        Some(&cookie),
    );
    empty_selection(&already_live);
    assert_eq!(store.reads.load(Ordering::SeqCst), 5);
    assert_refusals(app, flow, role, &runtime, &store, &body, &callback);
}

fn assert_refusals(
    app: &App,
    flow: &Flow,
    role: SyncRole,
    runtime: &Value,
    store: &Arc<SigningStore>,
    body: &[u8],
    callback: &str,
) {
    for switch in ["enabled", "usersync_enabled"] {
        let mut disabled = runtime.clone();
        disabled["bidders"][flow.name()][switch] = json!(false);
        empty_selection(&dispatch(
            app,
            &disabled,
            store,
            Method::POST,
            "/cookie_sync",
            body,
            None,
        ));
        let refused = dispatch(app, &disabled, store, Method::GET, callback, &[], None);
        assert_eq!(refused.status, StatusCode::BAD_REQUEST);
        refused.no_cookie();
    }
    let mut denied = runtime.clone();
    denied["accounts"][flow.account()] = json!({"privacy":{"allowactivities":{"syncUser":{
        "default":true,"rules":[{"allow":false,"condition":{"componentName":[flow.name()]}}]}}}});
    empty_selection(&dispatch(
        app,
        &denied,
        store,
        Method::POST,
        "/cookie_sync",
        body,
        None,
    ));
    let denied_callback = dispatch(app, &denied, store, Method::GET, callback, &[], None);
    assert_eq!(
        denied_callback.status,
        StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
    );
    denied_callback.no_cookie();
    let unavailable = SigningStore::new(false);
    let refused = dispatch(app, runtime, &unavailable, Method::GET, callback, &[], None);
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    refused.no_cookie();
    assert_eq!(
        unavailable.reads.load(Ordering::SeqCst),
        1,
        "no fallback or retry of unavailable signing"
    );
    selected(
        &dispatch(
            app,
            runtime,
            store,
            Method::POST,
            "/cookie_sync",
            body,
            None,
        ),
        role,
    );
}

pub(super) fn applicable_templates_use_signed_generated_app_callbacks(app: &App, cohort: &[Flow]) {
    let mut expected = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut new_native = BTreeSet::new();
    for flow in cohort {
        let syncer = &flow.info.user_sync;
        let roles: Vec<_> = [
            (SyncType::Iframe, syncer.iframe.as_ref()),
            (SyncType::Redirect, syncer.redirect.as_ref()),
        ]
        .into_iter()
        .filter(|(kind, _)| {
            syncer.enabled && (syncer.supports.is_empty() || syncer.supports.contains(kind))
        })
        .filter_map(|(kind, endpoint)| {
            endpoint.map(|selected_endpoint| SyncRole {
                info: flow.info,
                endpoint: selected_endpoint,
                kind,
            })
        })
        .collect();
        if roles.is_empty() {
            let runtime = runtime(flow);
            let store = SigningStore::new(true);
            empty_selection(&dispatch(
                app,
                &runtime,
                &store,
                Method::POST,
                "/cookie_sync",
                &payload(flow, None),
                None,
            ));
        }
        for role in roles {
            let mode = match role.kind {
                SyncType::Iframe => "iframe",
                SyncType::Redirect => "redirect",
            };
            let key = (flow.name().to_owned(), mode.to_owned());
            assert!(expected.insert(key.clone()));
            round_trip(app, flow, role);
            assert!(completed.insert(key));
            if flow.native_admission {
                new_native.insert(flow.name().to_owned());
            }
        }
    }
    assert!(
        !expected.is_empty(),
        "priority metadata must contain applicable sync templates"
    );
    assert_eq!(
        completed, expected,
        "every applicable declared template requires a live generated-App round trip"
    );
    assert!(
        !new_native.is_empty(),
        "legacy-only generic sync coverage is not native onboarding acceptance"
    );
    let gates = boot_gates().unwrap();
    assert_eq!(gates.aux.in_use(), 0);
    assert_eq!(gates.aux_work_budget.in_use(), 0);
}

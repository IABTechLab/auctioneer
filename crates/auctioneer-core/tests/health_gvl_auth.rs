//! Runtime policy and secret-store authorization for the GVL health route.

#[cfg(test)]
mod config_funding_support;
#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding_support as config_funding;
    use auctioneer_core::handlers::health_gvl::health_gvl;
    use auctioneer_core::host_primitives::{BearerToken, TestClockHandle};
    use auctioneer_core::test_support::config_store::{context, runtime_registry};
    use auctioneer_runtime::{Clock, FakeClock};
    use bytes::Bytes;
    use edgezero_core::body::Body;
    use edgezero_core::config_store::BoundedStoreRead;
    use edgezero_core::secret_store::{SecretError, SecretHandle, SecretStore};
    use edgezero_core::store_registry::{BoundSecretStore, SecretRegistry, StoreRegistry};
    use edgezero_core::time::{Deadline, MonotonicClock};
    use futures::executor::block_on;
    use http::{HeaderValue, Method, Request, StatusCode, Uri};
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use time::format_description::well_known::Rfc3339;
    use time::{Duration, OffsetDateTime};

    struct TokenStore {
        token: Option<Bytes>,
        calls: AtomicUsize,
        backend_bytes: u64,
    }
    #[async_trait::async_trait(?Send)]
    impl SecretStore for TokenStore {
        async fn get_bytes(&self, _: &str, _: &str) -> Result<Option<Bytes>, SecretError> {
            panic!("health must use bounded reads");
        }
        async fn get_bytes_bounded(
            &self,
            store_name: &str,
            key: &str,
            _: &MonotonicClock,
            _: Deadline,
            max_backend_bytes: u64,
            max_value_bytes: u64,
        ) -> Result<BoundedStoreRead<Bytes>, SecretError> {
            assert_eq!((store_name, key), ("custom_secrets", "custom_gvl_token"));
            assert_eq!((max_backend_bytes, max_value_bytes), (4096, 4096));
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(BoundedStoreRead {
                backend_bytes: self.backend_bytes,
                value: self.token.clone(),
            })
        }
    }
    fn store(token: Option<&[u8]>, backend_bytes: u64) -> Arc<TokenStore> {
        Arc::new(TokenStore {
            token: token.map(Bytes::copy_from_slice),
            calls: AtomicUsize::new(0),
            backend_bytes,
        })
    }

    async fn status_for(bearer: Option<HeaderValue>) -> StatusCode {
        status_with(
            json!({"schema_version": 1_u8, "gvl": {"authenticated_health": {
            "enabled": true, "token_secret": "custom_gvl_token"}}}),
            Some(store(Some(b"t"), 1)),
            bearer,
        )
        .await
    }

    async fn status_with(
        policy: Value,
        store: Option<Arc<TokenStore>>,
        bearer: Option<HeaderValue>,
    ) -> StatusCode {
        config_funding::install();
        let generated =
            OffsetDateTime::parse(auctioneer_config_data::gvl_archive_generated_at(), &Rfc3339)
                .expect("baked generated_at parses as RFC 3339");
        let clock: Arc<dyn Clock + Send + Sync> =
            Arc::new(FakeClock::new(generated.saturating_add(Duration::days(1))));
        let mut builder = Request::builder()
            .method(Method::GET)
            .uri("/_auctioneer/health/gvl".parse::<Uri>().unwrap());
        if let Some(value) = bearer {
            builder = builder.header("authorization", value);
        }
        let mut request = builder.body(Body::from_bytes(Vec::new())).unwrap();
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(runtime_registry(&policy));
        if let Some(selected_store) = store {
            let shared: Arc<dyn SecretStore> = selected_store;
            let registry: SecretRegistry = StoreRegistry::single_id(
                "selected".to_owned(),
                BoundSecretStore::new(SecretHandle::new(shared), "custom_secrets".to_owned()),
            );
            request.extensions_mut().insert(registry);
        }
        let ctx = context(request);
        let response = health_gvl(ctx).await.expect("handler returns Ok");
        let (parts, _body) = response.into_parts();
        assert_eq!(
            parts
                .headers
                .get("cache-control")
                .and_then(|value| value.to_str().ok()),
            Some("no-store"),
            "health responses must not outlive the selected runtime access policy"
        );
        if parts.status == StatusCode::UNAUTHORIZED {
            assert_eq!(
                parts
                    .headers
                    .get("www-authenticate")
                    .and_then(|v| v.to_str().ok()),
                Some("Bearer"),
                "every 401 must carry the WWW-Authenticate challenge"
            );
        }
        parts.status
    }

    #[test]
    fn health_gvl_returns_401_when_token_set_and_missing() {
        // The NBSP case is constructed from raw UTF-8 bytes: a
        // string-based builder rejects non-visible-ASCII BEFORE the
        // handler runs, which would test the builder, not the parser.
        // In the handler, `.to_str()` fails on the obs-text bytes →
        // `presented = None` → 401 — but it must reach the handler to
        // prove it.
        let nbsp = HeaderValue::from_bytes(b"Bearer \xC2\xA0t").expect("obs-text header value");
        let cases: [(Option<HeaderValue>, StatusCode, &str); 11] = [
            (None, StatusCode::UNAUTHORIZED, "no Authorization header"),
            (
                Some(HeaderValue::from_static("Bearer wrong")),
                StatusCode::UNAUTHORIZED,
                "wrong token",
            ),
            (
                Some(HeaderValue::from_static("Bearer t")),
                StatusCode::OK,
                "exact token",
            ),
            (
                Some(HeaderValue::from_static("bearer t")),
                StatusCode::OK,
                "scheme is case-insensitive (lowercase)",
            ),
            (
                Some(HeaderValue::from_static("BEARER t")),
                StatusCode::OK,
                "scheme is case-insensitive (uppercase)",
            ),
            (
                Some(HeaderValue::from_static("Bearer   t")),
                StatusCode::OK,
                "multiple spaces before the credential",
            ),
            (
                Some(HeaderValue::from_static("Bearer\tt")),
                StatusCode::UNAUTHORIZED,
                "tab directly after the scheme (no SP, split_once never matches)",
            ),
            (
                Some(HeaderValue::from_static("Bearer \tt")),
                StatusCode::UNAUTHORIZED,
                "SP then TAB (SP-only trimming leaves the tab on the credential)",
            ),
            (
                Some(nbsp),
                StatusCode::UNAUTHORIZED,
                "Unicode NBSP after SP",
            ),
            (
                Some(HeaderValue::from_static("Bearer ")),
                StatusCode::UNAUTHORIZED,
                "empty credential after trimming",
            ),
            (
                Some(HeaderValue::from_static("Bearer t ")),
                StatusCode::UNAUTHORIZED,
                "trailing space is part of the presented credential",
            ),
        ];
        block_on(async {
            for (bearer, expected, label) in cases {
                assert_eq!(status_for(bearer).await, expected, "case: {label}");
            }
        });
    }

    #[test]
    fn health_policy_and_secret_rotation_apply_per_request() {
        block_on(async {
            let policy = json!({"schema_version": 1_u8, "gvl": {"authenticated_health": {
                "enabled": true, "token_secret": "custom_gvl_token"}}});
            for (token, bearer, expected) in [
                (b"first".as_slice(), "Bearer first", StatusCode::OK),
                (
                    b"second".as_slice(),
                    "Bearer first",
                    StatusCode::UNAUTHORIZED,
                ),
                (b"second".as_slice(), "Bearer second", StatusCode::OK),
            ] {
                assert_eq!(
                    status_with(
                        policy.clone(),
                        Some(store(Some(token), u64::try_from(token.len()).unwrap())),
                        Some(HeaderValue::from_static(bearer))
                    )
                    .await,
                    expected
                );
            }
            let unused = store(Some(b"t"), 1);
            assert_eq!(status_with(json!({"schema_version": 1_u8, "gvl": {"authenticated_health": {"enabled": false}}}),
                Some(Arc::clone(&unused)), None).await, StatusCode::OK);
            assert_eq!(unused.calls.load(Ordering::SeqCst), 0);
            assert_eq!(
                status_with(policy, None, Some(HeaderValue::from_static("Bearer t"))).await,
                StatusCode::UNAUTHORIZED
            );
        });
    }

    #[test]
    fn invalid_policy_and_secret_faults_never_open_health() {
        block_on(async {
            for policy in [
                json!({"schema_version": 1_u8}),
                json!({"schema_version": 1_u8, "gvl": {}}),
                json!({"schema_version": 1_u8, "gvl": {"authenticated_health": {"enabled": true}}}),
                json!({"schema_version": 1_u8, "gvl": {"authenticated_health": {"enabled": false, "token_secret": "custom_gvl_token"}}}),
            ] {
                let unused = store(Some(b"t"), 1);
                assert_eq!(
                    status_with(
                        policy,
                        Some(Arc::clone(&unused)),
                        Some(HeaderValue::from_static("Bearer t"))
                    )
                    .await,
                    StatusCode::UNAUTHORIZED
                );
                assert_eq!(unused.calls.load(Ordering::SeqCst), 0);
            }
            let policy = json!({"schema_version": 1_u8, "gvl": {"authenticated_health": {"enabled": true, "token_secret": "custom_gvl_token"}}});
            for fault in [
                store(None, 0),
                store(Some(b""), 0),
                store(Some(b"t"), 4097),
                store(Some(&vec![b't'; 4097]), 1),
            ] {
                assert_eq!(
                    status_with(
                        policy.clone(),
                        Some(fault),
                        Some(HeaderValue::from_static("Bearer t"))
                    )
                    .await,
                    StatusCode::UNAUTHORIZED
                );
            }
        });
    }

    #[test]
    fn bearer_token_rejects_empty_secret() {
        assert!(
            BearerToken::new("").is_none(),
            "an empty configured secret is a misconfiguration, not a valid token"
        );
        assert!(BearerToken::new("t").is_some());
    }
}

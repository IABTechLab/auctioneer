//! Token-policy binary for `/_auctioneer/health/gvl`.
//!
//! `HEALTH_POLICY` is a process-global `OnceLock`, so this binary owns
//! the Token state; ALL auth request shapes share ONE `#[test]` that
//! installs once and exercises the table — three `#[test]`s racing to
//! install would interleave with the conflicting-install case. The
//! token is adapter-resolved and installed as a typed policy; core
//! never reads `std::env`, so the test installs a mock authenticator
//! wrapped in the Token policy.

#[cfg(test)]
mod tests {
    use auctioneer_core::handlers::health_gvl::health_gvl;
    use auctioneer_core::host_primitives::{
        BearerToken, HealthAccessPolicy, HealthAuth, HealthBootError, TestClockHandle,
        install_health_primitives,
    };
    use auctioneer_runtime::{Clock, FakeClock};
    use edgezero_core::body::Body;
    use edgezero_core::context::RequestContext;
    use edgezero_core::params::PathParams;
    use futures::executor::block_on;
    use http::{HeaderValue, Method, Request, StatusCode, Uri};
    use std::sync::Arc;
    use time::format_description::well_known::Rfc3339;
    use time::{Duration, OffsetDateTime};

    struct FixedToken(&'static str);

    impl HealthAuth for FixedToken {
        fn verify(&self, presented: &str) -> bool {
            // Test double; production impls compare in constant time.
            presented == self.0
        }

        fn identity(&self) -> [u8; 32] {
            let mut id = [0_u8; 32];
            let bytes = self.0.as_bytes();
            let n = bytes.len().min(32);
            id[..n].copy_from_slice(&bytes[..n]);
            id
        }
    }

    fn install_token() {
        install_health_primitives(
            HealthAccessPolicy::Token(Arc::new(FixedToken("t"))),
            "process",
        )
        .expect("identical reinstalls of the token policy are idempotent");
    }

    async fn status_for(bearer: Option<HeaderValue>) -> StatusCode {
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
        let ctx = RequestContext::new(request, PathParams::default());
        let response = health_gvl(ctx).await.expect("handler returns Ok");
        let (parts, _body) = response.into_parts();
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
        install_token();
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
    fn conflicting_health_policy_fails_boot() {
        install_token();
        let conflicting = install_health_primitives(
            HealthAccessPolicy::Token(Arc::new(FixedToken("other"))),
            "process",
        );
        assert!(
            matches!(conflicting, Err(HealthBootError::ConflictingPolicy)),
            "a second install with a DIFFERENT token must fail boot"
        );
        let idempotent = install_health_primitives(
            HealthAccessPolicy::Token(Arc::new(FixedToken("t"))),
            "process",
        );
        assert!(
            idempotent.is_ok(),
            "a second install with the SAME secret is idempotent (identity-compared)"
        );
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

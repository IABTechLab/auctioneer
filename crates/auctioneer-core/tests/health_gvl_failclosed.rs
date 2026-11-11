//! Missing runtime policy fails closed even without installed host primitives.
//! Keep this binary isolated from funded/booted handler tests.

#[cfg(test)]
mod tests {
    use auctioneer_core::handlers::health_gvl::{counter, health_gvl};
    use edgezero_core::body::Body;
    use edgezero_core::context::RequestContext;
    use edgezero_core::params::PathParams;
    use futures::executor::block_on;
    use http::{Method, Request, StatusCode, Uri};
    use time::OffsetDateTime;

    #[test]
    fn health_gvl_returns_401_when_no_runtime_policy_bound() {
        block_on(async {
            for bearer in [None, Some("Bearer sometoken")] {
                let mut builder = Request::builder()
                    .method(Method::GET)
                    .uri("/_auctioneer/health/gvl".parse::<Uri>().unwrap());
                if let Some(value) = bearer {
                    builder = builder.header("authorization", value);
                }
                // No TestClockHandle and no installed clock: the deny
                // path must return before ever touching the clock, the
                // counter, or the payload.
                let request = builder.body(Body::from_bytes(Vec::new())).unwrap();
                let ctx = RequestContext::new(request, PathParams::default());
                let response = health_gvl(ctx).await.expect("handler returns Ok");
                let (parts, body) = response.into_parts();
                assert_eq!(parts.status, StatusCode::UNAUTHORIZED);
                assert_eq!(
                    parts
                        .headers
                        .get("www-authenticate")
                        .and_then(|v| v.to_str().ok()),
                    Some("Bearer"),
                    "a 401 must carry the WWW-Authenticate challenge (RFC 9110)"
                );
                let bytes = body
                    .into_bytes_bounded(1_024)
                    .await
                    .expect("rejection body reads");
                assert_eq!(bytes.as_ref(), b"unauthorized");
            }
            assert_eq!(
                counter().count_last_hour(OffsetDateTime::UNIX_EPOCH),
                0,
                "the deny path must not touch the counter"
            );
        });
    }
}

//! GVL health payload and process-local counter tests with explicit runtime opt-in.

#[cfg(test)]
mod config_funding_support;
#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding_support as config_funding;
    use auctioneer_core::handlers::health_gvl::{counter, gcl_stale_counter, health_gvl};
    use auctioneer_core::host_primitives::{TestClockHandle, install_health_counter_scope};
    use auctioneer_core::test_support::config_store::{context, runtime_registry};
    use auctioneer_runtime::body_caps::{HEALTH_GVL_CAP_BYTES, RouteMatch, match_route, table};
    use auctioneer_runtime::{Clock, FakeClock};
    use edgezero_core::body::Body;
    use futures::executor::block_on;
    use http::{Method, Request, StatusCode, Uri};
    use std::sync::Arc;
    use time::format_description::well_known::Rfc3339;
    use time::{Duration, OffsetDateTime};

    fn install_acknowledged() {
        config_funding::install();
        install_health_counter_scope("isolate").unwrap();
    }

    fn generated_at() -> OffsetDateTime {
        OffsetDateTime::parse(auctioneer_config_data::gvl_archive_generated_at(), &Rfc3339)
            .expect("baked generated_at parses as RFC 3339")
    }

    fn gcl_expires_at() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(auctioneer_config_data::gcl_expires_at_unix_secs())
            .expect("baked GCL expiry is a representable timestamp")
    }

    /// A clock inside BOTH freshness windows — the archive's (months)
    /// and the GCL's (days). The GCL's is by far the shorter, so it is
    /// the binding one; picking a fixed offset off `generated_at` alone
    /// would drift past the GCL expiry as soon as the archive is
    /// refreshed with a GCL whose lifetime is shorter than that offset.
    fn healthy_now() -> OffsetDateTime {
        let one_day_in = generated_at().saturating_add(Duration::days(1));
        let before_gcl_expiry = gcl_expires_at().saturating_sub(Duration::hours(1));
        one_day_in.min(before_gcl_expiry)
    }

    async fn call_at(now: OffsetDateTime) -> (StatusCode, serde_json::Value) {
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(FakeClock::new(now));
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/_auctioneer/health/gvl".parse::<Uri>().unwrap())
            .body(Body::from_bytes(Vec::new()))
            .unwrap();
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(runtime_registry(
            &serde_json::json!({"schema_version": 1_u8,
            "gvl": {"authenticated_health": {"enabled": false}}}),
        ));
        let ctx = context(request);
        let response = health_gvl(ctx).await.expect("handler returns Ok");
        let (parts, body) = response.into_parts();
        let bytes = body
            .into_bytes_bounded(64 * 1024)
            .await
            .expect("payload body reads");
        let payload = serde_json::from_slice(&bytes).expect("payload is JSON");
        (parts.status, payload)
    }

    #[test]
    fn health_gvl_endpoint_registered_in_route_table() {
        let matched = match_route(&Method::GET, "/_auctioneer/health/gvl", table());
        assert_eq!(
            matched,
            RouteMatch::Matched {
                route_id: "health_gvl",
                cap_bytes: HEALTH_GVL_CAP_BYTES,
            },
            "GET /_auctioneer/health/gvl must be a wired route with the 4 KiB no-body cap"
        );
    }

    #[test]
    fn health_gvl_reports_generated_at_from_manifest() {
        install_acknowledged();
        block_on(async {
            let (status, payload) = call_at(healthy_now()).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(payload["status"], "ok");
            assert_eq!(
                payload["generated_at"],
                auctioneer_config_data::gvl_archive_generated_at(),
                "payload must echo the manifest's generated_at verbatim"
            );
            let expected_spec3: Vec<u32> = {
                let mut versions: Vec<u32> = auctioneer_config_data::gvl_archive()
                    .iter()
                    .filter(|entry| entry.spec == 3)
                    .map(|entry| entry.vendor_list_version)
                    .collect();
                versions.sort_unstable();
                versions
            };
            assert_eq!(payload["spec3_versions"], serde_json::json!(expected_spec3));
            let expected_spec2: Vec<u32> = {
                let mut versions: Vec<u32> = auctioneer_config_data::gvl_archive()
                    .iter()
                    .filter(|entry| entry.spec == 2)
                    .map(|entry| entry.vendor_list_version)
                    .collect();
                versions.sort_unstable();
                versions
            };
            assert_eq!(payload["spec2_versions"], serde_json::json!(expected_spec2));
        });
    }

    #[test]
    fn health_gvl_reports_age_within_max_age_days() {
        install_acknowledged();
        block_on(async {
            let now = healthy_now();
            let expected_age = now
                .unix_timestamp()
                .saturating_sub(generated_at().unix_timestamp())
                .div_euclid(86_400)
                .max(0_i64);
            let (status, payload) = call_at(now).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(payload["status"], "ok");
            assert_eq!(payload["age_days"], expected_age);
            assert_eq!(
                payload["max_age_days"],
                auctioneer_config_data::gvl_archive_max_age_days()
            );
        });
    }

    #[test]
    fn health_gvl_stale_archive_returns_503() {
        install_acknowledged();
        block_on(async {
            let past_max = i64::from(auctioneer_config_data::gvl_archive_max_age_days()) + 2;
            let (status, payload) =
                call_at(generated_at().saturating_add(Duration::days(past_max))).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(payload["status"], "stale");
        });
    }

    #[test]
    fn health_gvl_expired_gcl_returns_503_and_stale() {
        install_acknowledged();
        block_on(async {
            // One second past the baked GCL's expiry, with the archive
            // still well inside its own (far longer) freshness window:
            // the archive alone would report `ok`, so only the GCL gate
            // can produce this result.
            let now = gcl_expires_at().saturating_add(Duration::seconds(1));
            let archive_age_days = now
                .unix_timestamp()
                .saturating_sub(generated_at().unix_timestamp())
                .div_euclid(86_400);
            assert!(
                archive_age_days <= i64::from(auctioneer_config_data::gvl_archive_max_age_days()),
                "fixture must isolate the GCL gate: archive age {archive_age_days} is already                  past its own window"
            );
            let (status, payload) = call_at(now).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(payload["status"], "stale");
            assert_eq!(payload["gcl_expired"], true);
        });
    }

    #[test]
    fn health_gvl_unexpired_gcl_reports_expiry_and_stays_ok() {
        install_acknowledged();
        block_on(async {
            let (status, payload) = call_at(healthy_now()).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(payload["status"], "ok");
            assert_eq!(payload["gcl_expired"], false);
            let reported = payload["gcl_expires_at"]
                .as_str()
                .expect("payload carries the GCL expiry as an RFC 3339 string");
            assert_eq!(
                OffsetDateTime::parse(reported, &Rfc3339).expect("gcl_expires_at is RFC 3339"),
                gcl_expires_at()
            );
        });
    }

    #[test]
    fn gcl_stale_counter_is_separate_from_the_unavailable_counter() {
        // 10018 (GVL revision missing) and 10019 (GCL past expiry) are
        // different deployment faults, so a record on one must not move
        // the other's number. Recording at 2001-01-01T00:00:00Z keeps
        // this decades away from every other window read in the binary.
        let base = OffsetDateTime::from_unix_timestamp(978_307_200).expect("valid timestamp");
        let unavailable_before = counter().count_last_hour(base);
        gcl_stale_counter().record(base);
        gcl_stale_counter().record(base);
        assert_eq!(gcl_stale_counter().count_last_hour(base), 2);
        assert_eq!(
            counter().count_last_hour(base),
            unavailable_before,
            "recording a stale-GCL lookup must not move the GVL-unavailable counter"
        );
    }

    #[test]
    fn counter_records_and_count_last_hour_reflects_window() {
        // Direct counter-primitive test. The counter is a
        // process-global singleton shared with the handler tests in
        // this binary, so it records at 2000-01-01T00:00:00Z — decades
        // outside the archive-relative window those tests read, which
        // keeps both sides independent of test ordering.
        let base = OffsetDateTime::from_unix_timestamp(946_684_800).expect("valid timestamp");
        counter().record(base);
        counter().record(base);
        counter().record(base.saturating_add(Duration::minutes(30)));
        counter().record(base.saturating_add(Duration::minutes(59)));
        assert_eq!(
            counter().count_last_hour(base.saturating_add(Duration::minutes(59))),
            4,
            "all four records fall inside the trailing hour"
        );
        assert_eq!(
            counter().count_last_hour(base),
            2,
            "future-labelled buckets must not count toward an earlier window"
        );
        assert_eq!(
            counter().count_last_hour(base.saturating_add(Duration::minutes(119))),
            0,
            "records older than 60 minutes age out"
        );
    }

    #[test]
    fn health_gvl_payload_carries_counter_scope_field() {
        install_acknowledged();
        block_on(async {
            let (status, payload) = call_at(healthy_now()).await;
            assert_eq!(status, StatusCode::OK);
            // This binary installs scope "isolate"; the payload must
            // echo the installed scope so operators can't misread the
            // process-local counter as deployment-wide.
            assert_eq!(payload["counter_scope"], "isolate");
        });
    }
}

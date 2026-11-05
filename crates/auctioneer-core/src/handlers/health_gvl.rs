//! `GET /_auctioneer/health/gvl` handler + the rolling GVL
//! unavailable-lookup counter.
//!
//! The handler is fail-closed: it answers 401 unless the adapter boot
//! installed an explicit `HealthAccessPolicy` (see
//! `host_primitives::registry`). The counters are the process-local
//! primitives the auction handler records into when the privacy
//! resolver emits `GvlVersionUnavailable` or `GclStale` warnings; this
//! module ships the primitives and the read side.
//!
//! Status is `stale` when EITHER the baked GVL archive is past its
//! freshness window OR the bundled Global CMP List is past its expiry.
//! The two have very different lifetimes — the archive window is
//! months, the GCL's RFC 9111 lifetime is days — and either one alone
//! makes every enforced auction fail closed.

use crate::handlers::aux_egress::with_aux_egress;
use crate::host_primitives::registry::{self as host_registry, TestClockHandle};
use crate::serialize::serialize_json_exact;
use auctioneer_runtime::Clock;
use edgezero_core::action;
use edgezero_core::body::Body;
use edgezero_core::context::RequestContext;
use edgezero_core::error::EdgeError;
use edgezero_core::http::{Response, StatusCode, response_builder};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// Rolling count of GVL lookups that found no usable vendor list,
/// bucketed per minute over the trailing hour.
pub struct GvlUnavailableCounter {
    // Ring of (bucket-start minute, count) per-minute buckets covering
    // the trailing 60 minutes. Mutex is uncontended in practice; the
    // counter is read at request time (rare) and written at auction
    // time (frequent but bucketed).
    buckets: Mutex<[(i64, u32); 60]>,
}

impl Default for GvlUnavailableCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl GvlUnavailableCounter {
    /// An empty counter — all buckets zeroed at the epoch minute.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: Mutex::new([(0_i64, 0_u32); 60]),
        }
    }

    /// Record one unavailable lookup observed at `now`.
    pub fn record(&self, now: OffsetDateTime) {
        // `div_euclid` pairs with the `rem_euclid` below so the
        // minute/bucket derivation stays consistent for pre-epoch
        // timestamps.
        let minute = now.unix_timestamp().div_euclid(60);
        // `rem_euclid` keeps the index in 0..60 even for a pre-epoch
        // timestamp.
        let bucket = usize::try_from(minute.rem_euclid(60)).unwrap_or(0);
        // Recover a poisoned guard: the bucket data is fine, only the
        // guard was contaminated by an unrelated test panic.
        let mut b = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        // bucket ∈ 0..60 == array len, so `get_mut` is always `Some`;
        // the `if let` is fail-safe.
        if let Some((bucket_start, count)) = b.get_mut(bucket) {
            if *bucket_start != minute {
                *bucket_start = minute;
                *count = 0;
            }
            *count = count.saturating_add(1);
        }
    }

    /// Sum of the buckets whose start minute falls inside the trailing
    /// hour ending at `now`.
    #[must_use]
    pub fn count_last_hour(&self, now: OffsetDateTime) -> u32 {
        let now_bucket = now.unix_timestamp().div_euclid(60);
        let b = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        b.iter()
            // A plain `now_bucket - start < 60` would accept NEGATIVE
            // deltas (future-labelled buckets) because signed
            // negatives compare less than 60; restrict to the
            // trailing hour with `0..60`. A wrapped/absent
            // `checked_sub` delta is treated as out-of-window.
            .filter(|(start, _)| {
                now_bucket
                    .checked_sub(*start)
                    .is_some_and(|delta| (0_i64..60_i64).contains(&delta))
            })
            .map(|(_, count)| *count)
            .fold(0_u32, u32::saturating_add)
    }
}

/// The process-local counter singleton the health payload reports.
pub fn counter() -> &'static GvlUnavailableCounter {
    static C: OnceLock<GvlUnavailableCounter> = OnceLock::new();
    C.get_or_init(GvlUnavailableCounter::new)
}

/// The stale-GCL counter singleton. Separate from [`counter`]: a
/// bundled Global CMP List past its freshness window (§7) and a GVL
/// revision missing from the archive (§7.3) are different deployment
/// faults with different fixes, so folding both into one number would
/// tell an operator nothing about which one to chase.
pub fn gcl_stale_counter() -> &'static GvlUnavailableCounter {
    static C: OnceLock<GvlUnavailableCounter> = OnceLock::new();
    C.get_or_init(GvlUnavailableCounter::new)
}

/// Wrapped, like every aux route — see
/// [`with_aux_egress`](crate::handlers::aux_egress::with_aux_egress)
/// for what the wrapper does and why it is one shared call.
#[action]
pub async fn health_gvl(ctx: RequestContext) -> Result<Response, EdgeError> {
    with_aux_egress(ctx, health_gvl_body).await
}

async fn health_gvl_body(ctx: RequestContext) -> Result<Response, EdgeError> {
    // Authorize through `health_authorize`, which FAIL-CLOSES when no
    // policy was installed. The token comes from the ADAPTER-installed
    // policy, never `std::env` (empty on Workers).
    //
    // The `Bearer` scheme name is CASE-INSENSITIVE (RFC 9110 §11.1)
    // and may be followed by more than one SPACE, so the scheme token
    // matches case-insensitively and leading SPACES are trimmed off
    // the credential rather than requiring an exact `"Bearer "`
    // prefix (which would wrongly reject `bearer` and `BEARER`). A
    // TAB after the scheme is NOT valid RFC 9110 auth whitespace —
    // only SP is — hence `trim_start_matches(' ')` (SP ONLY), NOT
    // `trim_start()`: `trim_start()` strips tabs and Unicode
    // whitespace, so `Bearer \t<tok>` (space, then tab) would have
    // AUTHENTICATED with the tab silently eaten. With SP-only
    // trimming, a credential that still begins with a tab or Unicode
    // space simply fails the exact token comparison below → 401.
    let presented = ctx
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            let (scheme, rest) = v.split_once(' ')?;
            scheme
                .eq_ignore_ascii_case("bearer")
                .then(|| rest.trim_start_matches(' '))
        });
    if matches!(
        host_registry::health_authorize(presented),
        host_registry::HealthAuthz::Deny
    ) {
        // Covers all three deny paths: no policy installed (the
        // fail-closed default), Token policy with a missing or wrong
        // bearer. A 401 MUST carry a `WWW-Authenticate` challenge
        // (RFC 9110 §11.6.1).
        return response_builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("WWW-Authenticate", "Bearer")
            .body(Body::from_bytes(b"unauthorized".to_vec()))
            .map_err(EdgeError::internal);
    }
    let clock = clock_handle(&ctx);
    let now = clock.now();
    let generated_at_raw = auctioneer_config_data::gvl_archive_generated_at();
    let generated_dt = OffsetDateTime::parse(generated_at_raw, &Rfc3339)
        .map_err(|e| EdgeError::internal(anyhow::anyhow!("manifest.generated_at parse: {e}")))?;
    // Whole days elapsed since the manifest was generated, clamped to
    // zero for a future-stamped manifest. `checked_sub` + `div_euclid`
    // keep the arithmetic total.
    let age_days = now
        .unix_timestamp()
        .checked_sub(generated_dt.unix_timestamp())
        .map_or(0_i64, |secs| secs.div_euclid(86_400))
        .max(0_i64);
    let max_age_days = auctioneer_config_data::gvl_archive_max_age_days();
    let (mut s2, mut s3) = (Vec::new(), Vec::new());
    for entry in auctioneer_config_data::gvl_archive() {
        match entry.spec {
            2 => s2.push(entry.vendor_list_version),
            3 => s3.push(entry.vendor_list_version),
            _ => {}
        }
    }
    s2.sort_unstable();
    s3.sort_unstable();
    let unavailable = counter().count_last_hour(now);
    let gcl_stale_lookups = gcl_stale_counter().count_last_hour(now);
    // A Global CMP List past its expiry authenticates no CMP, so every
    // enforced auction falls back to unusable-consent defaults (§7).
    // That is the same operator-visible outage as a stale archive, and
    // the GCL's RFC 9111 lifetime is far shorter than the archive's
    // freshness window, so health must gate on BOTH or report green
    // through a live fail-closed deployment.
    let gcl_expires_unix = auctioneer_config_data::gcl_expires_at_unix_secs();
    let gcl_expired = now.unix_timestamp() >= gcl_expires_unix;
    // An unrepresentable expiry reads as already stale, matching the
    // runtime `Gcl` boot lowering rather than contradicting it.
    let gcl_expires_at = OffsetDateTime::from_unix_timestamp(gcl_expires_unix)
        .ok()
        .and_then(|dt| dt.format(&Rfc3339).ok());
    // Compare in i64 via `i64::from` — an `as u32` narrowing would be
    // lossy on a negative or huge `age_days`.
    let status_flag = if age_days > i64::from(max_age_days) || gcl_expired {
        "stale"
    } else {
        "ok"
    };
    let payload = serde_json::json!({
        "status": status_flag,
        // The ADAPTER declares its real scope; hardcoding "isolate"
        // would misdescribe Axum (process-scoped) and Fastly
        // (request-sandbox-scoped). `counter_scope_handle()` returns
        // the adapter's installed value:
        //   Cloudflare → "isolate"
        //   Fastly     → "request"   (fresh-sandbox default)
        //   Spin       → "instance"
        //   Axum       → "process"
        "counter_scope": host_registry::counter_scope_handle(),
        "generated_at": generated_at_raw,
        "age_days": age_days,
        "max_age_days": max_age_days,
        "spec2_versions": s2,
        "spec3_versions": s3,
        "unavailable_lookups_last_hour": unavailable,
        "gcl_expires_at": gcl_expires_at,
        "gcl_expired": gcl_expired,
        "gcl_stale_lookups_last_hour": gcl_stale_lookups,
    });
    let status_code = if status_flag == "stale" {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    let body_bytes = serialize_json_exact(&payload)
        .map_err(|_err| EdgeError::internal(anyhow::anyhow!("payload serialise failed")))?;
    response_builder()
        .status(status_code)
        .header("content-type", "application/json")
        .body(Body::from_bytes(body_bytes))
        .map_err(EdgeError::internal)
}

fn clock_handle(ctx: &RequestContext) -> Arc<dyn Clock + Send + Sync> {
    if let Some(handle) = ctx.extensions().get::<TestClockHandle>() {
        return Arc::clone(&handle.0);
    }
    host_registry::clock_handle()
}

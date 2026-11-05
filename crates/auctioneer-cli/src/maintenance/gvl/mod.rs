//! Refreshes `crates/auctioneer-config-data/data/gvl_archive/`: fetches the IAB
//! Global Vendor List corpus (frozen spec-2 list, current spec-3 list,
//! archived spec-3 versions) plus the Global CMP List, validates
//! everything through `auctioneer-gvl-schema`, and atomically swaps the
//! result into place.
//!
//! Transactional by construction: every write goes to this run's unique
//! transaction directory (`data/.gvl_refresh.<pid>.<n>/`), the entire
//! result is validated, and only then is it swapped into `gvl_archive/`
//! under the exclusive archive lock (two renames through the single
//! fixed `gvl_archive.old` aside, each made durable with a parent
//! fsync). Any failure before the swap leaves the checked-in archive
//! byte-for-byte untouched. Two locks, one protocol: `refresh.lock` is
//! held exclusively for the whole run (serializing refreshes so an
//! upstream cache period sees at most one request) while
//! `gvl_archive.lock` guards only the recovery + swap window, so a
//! concurrent build's shared-lock read never waits through a download.
#![expect(
    clippy::print_stderr,
    reason = "operator-facing CLI: progress and failure reporting go to stderr"
)]

use anyhow::{Context as _, bail};
use auctioneer_gvl_schema::{
    GCL_MAX_RAW_BYTES, VersionMeta, WASM_BUNDLE_MAX_RAW_BYTES, check_gcl_consistency,
    recover_gvl_archive, select_retained_versions, validate_gcl_json, validate_gvl_json,
    validate_manifest,
};
use chrono::DateTime;
use fs2::FileExt as _;
use reqwest::blocking::Client;
use reqwest::header::HeaderMap;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;
use std::str;
use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

const SPEC2_CURRENT_URL: &str = "https://vendor-list.consensu.org/v2/vendor-list.json";
const SPEC3_CURRENT_URL: &str = "https://vendor-list.consensu.org/v3/vendor-list.json";
const SPEC3_ARCHIVE_BASE_URL: &str = "https://vendor-list.consensu.org/v3/archives";
const GCL_URL: &str = "https://cmplist.consensu.org/v2/cmp-list.json";

/// Headroom kept under the raw cap so ~4-5 weekly versions of growth
/// never break the build between refreshes (4 MiB).
const GROWTH_MARGIN_BYTES: u64 = 0x0040_0000;

/// Bound on any single decoded GVL blob read — a file that alone
/// exceeds the whole-bundle raw cap could never be bundled.
const GVL_FETCH_MAX_BYTES: u64 = WASM_BUNDLE_MAX_RAW_BYTES;

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Command {
    /// Refresh the validated GVL archive and Global CMP List.
    Refresh(RefreshArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct RefreshArgs {
    /// Retain archive entries this many days back. Default 180. Values
    /// over 180 require --allow-oversize-bundle: the raw-bundle cap
    /// still binds, so in practice the flag admits older-but-still-
    /// fitting spec-3 entries, never a bigger bundle.
    #[arg(long, default_value_t = 180)]
    max_age_days: u32,
    /// Acknowledges retention past 180 days. Recorded in
    /// `manifest.oversize_bundle_ack`, which relaxes the AGE filter only -
    /// build.rs still enforces the raw-bundle cap on every build (and
    /// the wasm 24 MiB hard maximum).
    #[arg(long)]
    allow_oversize_bundle: bool,
}

/// One archive blob staged for the new manifest, with the exact
/// upstream `lastUpdated` string preserved verbatim (build.rs
/// cross-checks the manifest string against the blob's own field
/// byte-for-byte).
struct ArchiveBlob {
    spec: u32,
    version: u32,
    tcf_policy_version: u32,
    last_updated_raw: String,
    last_updated: OffsetDateTime,
    sha256: String,
    bytes: Vec<u8>,
    /// RFC 9111 cache metadata, recorded only on the CURRENT spec-3
    /// list so later refresh runs can reuse it within its advertised
    /// lifetime.
    fetched_at: Option<String>,
    cache_max_age_secs: Option<u64>,
}

/// The caching-relevant response headers, captured verbatim.
struct ResponseCacheMeta {
    /// Every `Cache-Control` header value (a response may carry
    /// several; directives are the comma-joined union).
    cache_control: Vec<String>,
    age: Option<String>,
    date: Option<String>,
    expires: Option<String>,
}

struct FetchResult {
    bytes: Vec<u8>,
    meta: ResponseCacheMeta,
    fetched_at: OffsetDateTime,
    response_delay_secs: u64,
}

/// Lenient view of the LIVE manifest, read only to drive cache reuse
/// decisions. Any parse failure simply disables reuse — the strict
/// validator judges the OUTPUT (step 10), never gates the input.
#[derive(serde::Deserialize)]
struct LiveManifest {
    #[serde(default)]
    gcl: Option<LiveGclRecord>,
    #[serde(default)]
    entries: Vec<LiveEntry>,
}
#[derive(serde::Deserialize)]
struct LiveGclRecord {
    sha256: String,
    last_updated: String,
    fetched_at: String,
    cache_max_age_secs: u64,
}
#[derive(serde::Deserialize)]
struct LiveEntry {
    spec: u32,
    vendor_list_version: u32,
    path: String,
    sha256: String,
    #[serde(default)]
    fetched_at: Option<String>,
    #[serde(default)]
    cache_max_age_secs: Option<u64>,
}

/// Serialization order of these structs IS the canonical manifest key
/// order (serde emits struct fields in declaration order).
#[derive(serde::Serialize)]
struct ManifestOut {
    manifest_version: u32,
    generated_at: String,
    retention: RetentionOut,
    oversize_bundle_ack: bool,
    gcl: GclOut,
    entries: Vec<EntryOut>,
}
#[derive(serde::Serialize)]
struct RetentionOut {
    max_age_days: u32,
}
#[derive(serde::Serialize)]
struct GclOut {
    path: String,
    sha256: String,
    last_updated: String,
    fetched_at: String,
    cache_max_age_secs: u64,
}
#[derive(serde::Serialize)]
struct EntryOut {
    spec: u32,
    vendor_list_version: u32,
    tcf_policy_version: u32,
    last_updated: String,
    path: String,
    sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fetched_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_max_age_secs: Option<u64>,
}

/// Removes this run's OWN transaction directory (by exact path) on both
/// exit paths. Never enumerates or touches any other `.gvl_refresh.*`
/// sibling — a crashed run's orphan is bounded, git-ignored, and left
/// alone, so no pid-reuse or mtime-freshness heuristic is relied on.
struct TxDirGuard {
    path: PathBuf,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for TxDirGuard {
    fn drop(&mut self) {
        // After a successful swap the dir was renamed away; ENOENT here
        // is the expected steady state.
        let _cleanup = fs::remove_dir_all(&self.path);
    }
}

pub(crate) fn dispatch(command: Command) -> i32 {
    let result = match command {
        Command::Refresh(args) => run(&args),
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("Error: {error:#}");
            1
        }
    }
}

fn run(cli: &RefreshArgs) -> anyhow::Result<()> {
    check_retention_flags(cli.max_age_days, cli.allow_oversize_bundle)?;
    let raw_cap = resolve_raw_cap()?;
    let data_dir = data_dir()?;

    // Step 0: the run lock comes FIRST — before the tx dir, before any
    // cache read, before any network access. Held for the whole run.
    let run_lock_path = data_dir.join("refresh.lock");
    let run_lock = fs::File::open(&run_lock_path)
        .with_context(|| format!("open run-lock sentinel {}", run_lock_path.display()))?;
    if run_lock.try_lock_exclusive().is_err() {
        bail!("refresh already running (refresh.lock is held)");
    }

    let tx_dir = data_dir.join(format!(".gvl_refresh.{}.{}", process::id(), tx_nonce()));
    fs::create_dir_all(tx_dir.join("spec2")).context("create tx spec2/")?;
    fs::create_dir_all(tx_dir.join("spec3")).context("create tx spec3/")?;
    let tx_guard = TxDirGuard {
        path: tx_dir.clone(),
    };

    let now = OffsetDateTime::now_utc();
    // Step 1: the retention window.
    let retain_from = now
        .checked_sub(Duration::days(i64::from(cli.max_age_days)))
        .context("retention window underflows the representable time range")?;

    let live_dir = data_dir.join("gvl_archive");
    let live = read_live_manifest(&live_dir);
    let client = Client::builder()
        .timeout(StdDuration::from_mins(2))
        .build()
        .context("build HTTP client")?;

    // Step 2: the current spec-3 list — reusing the recorded cache
    // lifetime first (the IAB requires reusing the current GVL for its
    // advertised max-age; the run lock makes duplicate in-period
    // fetches impossible).
    let current_v3 = fetch_current_spec3(&client, live.as_ref(), &live_dir, now)?;
    eprintln!(
        "current spec-3 list: v{} ({} bytes)",
        current_v3.version,
        current_v3.bytes.len()
    );

    // Step 2b: the Global CMP List, at most once per cache period.
    let gcl = fetch_gcl(&client, live.as_ref(), &live_dir, now)?;
    fs::write(tx_dir.join("gcl.json"), &gcl.bytes).context("write tx gcl.json")?;

    // Step 3: reserve the frozen spec-2 list unconditionally.
    let frozen_v2 = fetch_frozen_spec2(&client, live.as_ref(), &live_dir)?;
    eprintln!(
        "frozen spec-2 list: v{} ({} bytes)",
        frozen_v2.version,
        frozen_v2.bytes.len()
    );

    // Step 4: age-filtered archived spec-3 candidates, newest first.
    let candidates = fetch_spec3_candidates(
        &client,
        live.as_ref(),
        &live_dir,
        current_v3.version,
        retain_from,
    )?;

    // Steps 3b-5: the pure retention decision — cap and margin passed
    // through uncombined so the helper's BudgetUnderflow arm stays
    // reachable. The keep set is exactly what lands in the tx dir.
    let reserved_meta = vec![blob_meta(&frozen_v2), blob_meta(&current_v3)];
    let candidate_meta: Vec<VersionMeta> = candidates.iter().map(blob_meta).collect();
    let kept_meta = select_retained_versions(
        &candidate_meta,
        &reserved_meta,
        raw_cap,
        GROWTH_MARGIN_BYTES,
        retain_from,
    )
    .map_err(|retention_err| {
        anyhow::anyhow!(
            "{retention_err} (raw cap {raw_cap} bytes, growth margin {GROWTH_MARGIN_BYTES} bytes)"
        )
    })?;

    let mut kept_blobs: Vec<&ArchiveBlob> = Vec::with_capacity(kept_meta.len());
    let by_key: BTreeMap<(u32, u32), &ArchiveBlob> = [&frozen_v2, &current_v3]
        .into_iter()
        .chain(candidates.iter())
        .map(|blob| ((blob.spec, blob.version), blob))
        .collect();
    for meta in &kept_meta {
        let blob = by_key
            .get(&(meta.spec, meta.version))
            .context("retention decision returned an unfetched version")?;
        kept_blobs.push(blob);
    }

    // Steps 6-8: per-file digests were computed at fetch; enforce the
    // structural rules on the keep set before writing anything.
    check_keep_set(&kept_blobs)?;

    for blob in kept_blobs.iter().copied() {
        let rel = blob_rel_path(blob);
        fs::write(tx_dir.join(&rel), &blob.bytes).with_context(|| format!("write tx {rel}"))?;
    }

    // Step 9: the manifest, keys in canonical order, entries sorted
    // (spec asc, vendor_list_version desc). generated_at is stamped
    // NOW, after the fetches - the fetch instants recorded above must
    // not postdate it.
    let manifest_json = render_manifest(
        OffsetDateTime::now_utc(),
        cli.max_age_days,
        cli.allow_oversize_bundle,
        &gcl,
        &kept_blobs,
    )?;
    fs::write(tx_dir.join("manifest.json"), &manifest_json).context("write tx manifest.json")?;

    // Step 10: validate the ENTIRE temp result before touching live,
    // with a fresh clock reading - the same call build.rs will make.
    validate_tx_dir(&tx_dir, OffsetDateTime::now_utc())?;

    // Step 11: exclusive archive lock, recover, swap, cleanup.
    swap_tx_into_live(&data_dir, &tx_dir)?;
    drop(tx_guard);
    drop(run_lock);
    eprintln!(
        "gvl_archive refreshed: spec2 v{}, spec3 v{} plus {} archived versions, gcl.json",
        frozen_v2.version,
        current_v3.version,
        kept_meta.len().saturating_sub(2),
    );
    Ok(())
}

/// Values over 180 days need the explicit oversize acknowledgement (see
/// the plan's retention-policy section: the ack relaxes the age filter
/// only; the raw cap still binds).
fn check_retention_flags(max_age_days: u32, allow_oversize_bundle: bool) -> anyhow::Result<()> {
    if max_age_days > 180 && !allow_oversize_bundle {
        bail!(
            "--max-age-days {max_age_days} exceeds 180; pass --allow-oversize-bundle to \
             acknowledge extended retention (the raw-bundle cap still applies — see the \
             GDPR fanout plan's retention policy)"
        );
    }
    Ok(())
}

/// One source of truth for the raw cap: `min(env, 24 MiB)` where an env
/// value ABOVE 24 MiB is rejected — the single committed archive feeds
/// all four adapters and must fit the wasm hard maximum, so the env can
/// only LOWER the cap.
fn resolve_raw_cap() -> anyhow::Result<u64> {
    let Ok(raw_mib) = env::var("AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB") else {
        return Ok(WASM_BUNDLE_MAX_RAW_BYTES);
    };
    let mib: u64 = raw_mib.parse().with_context(|| {
        format!("AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB={raw_mib:?} is not a u64")
    })?;
    let bytes = mib
        .checked_mul(0x0010_0000)
        .context("AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB overflows u64 bytes")?;
    if bytes > WASM_BUNDLE_MAX_RAW_BYTES {
        bail!(
            "AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB={mib} exceeds the 24 MiB wasm hard \
             maximum. The committed archive is a single artifact consumed by all four \
             adapters, so the env value may only LOWER the cap."
        );
    }
    Ok(bytes)
}

fn data_dir() -> anyhow::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-config-data/data");
    dir.canonicalize()
        .with_context(|| format!("resolve data dir {}", dir.display()))
}

fn tx_nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        // Nibble values are 0..=15, so both lookups always hit;
        // `unwrap_or` keeps the indexing checked anyway.
        let hi = usize::from(byte.wrapping_shr(4));
        let lo = usize::from(byte & 0x0F);
        out.push(char::from(HEX.get(hi).copied().unwrap_or(b'0')));
        out.push(char::from(HEX.get(lo).copied().unwrap_or(b'0')));
    }
    out
}

fn blob_meta(blob: &ArchiveBlob) -> VersionMeta {
    VersionMeta {
        spec: blob.spec,
        version: blob.version,
        last_updated: blob.last_updated,
        raw_bytes: u64::try_from(blob.bytes.len()).unwrap_or(u64::MAX),
    }
}

fn blob_rel_path(blob: &ArchiveBlob) -> String {
    format!("spec{}/vendor-list-v{}.json", blob.spec, blob.version)
}

/// Lenient read of the live manifest; `None` disables reuse and forces
/// full fetches.
fn read_live_manifest(live_dir: &Path) -> Option<LiveManifest> {
    let raw = fs::read_to_string(live_dir.join("manifest.json")).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Read the live blob at `rel_path` iff its bytes hash to
/// `expected_sha` (reuse is integrity-gated; anything else refetches).
fn live_blob_if_sha_matches(
    live_dir: &Path,
    rel_path: &str,
    expected_sha: &str,
) -> Option<Vec<u8>> {
    let bytes = fs::read(live_dir.join(rel_path)).ok()?;
    (sha256_hex(&bytes) == expected_sha).then_some(bytes)
}

/// Bounded decoded read: at most `cap + 1` bytes ever materialize (the
/// caller detects over-cap via `len() > cap`). The output buffer is
/// pre-sized to exactly `cap + 1` and extended within that capacity, so
/// it never reallocates — the bound is allocation-enforced, not
/// asserted after the fact. Transport buffers and the gzip decoder's
/// internal state ride on top as bounded library overhead.
fn read_decoded_bounded<R>(mut reader: R, cap: usize) -> io::Result<Vec<u8>>
where
    R: Read,
{
    let limit = cap
        .checked_add(1)
        .ok_or_else(|| io::Error::other("bounded-read cap overflows usize"))?;
    let mut out = Vec::with_capacity(limit);
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            return Ok(out);
        }
        let room = limit.saturating_sub(out.len());
        let take = read.min(room);
        if let Some(filled) = chunk.get(..take) {
            out.extend_from_slice(filled);
        }
        if out.len() >= limit {
            return Ok(out);
        }
    }
}

fn header_values(headers: &HeaderMap, name: &str) -> Vec<String> {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok().map(str::to_owned))
        .collect()
}

fn fetch_bounded(client: &Client, url: &str, cap: usize) -> anyhow::Result<FetchResult> {
    eprintln!("fetching {url}");
    let started = OffsetDateTime::now_utc();
    // reqwest's `gzip` feature sends `Accept-Encoding: gzip` and
    // transparently decodes, so the bounded read below applies to the
    // DECODED stream.
    let response = client
        .get(url)
        .send()
        .with_context(|| format!("GET {url}"))?;
    let fetched_at = OffsetDateTime::now_utc();
    let response_delay_secs = u64::try_from(
        fetched_at
            .unix_timestamp()
            .checked_sub(started.unix_timestamp())
            .unwrap_or(0),
    )
    .unwrap_or(0);
    if !response.status().is_success() {
        bail!("GET {url}: HTTP {}", response.status());
    }
    let meta = ResponseCacheMeta {
        cache_control: header_values(response.headers(), "cache-control"),
        age: header_values(response.headers(), "age").into_iter().next(),
        date: header_values(response.headers(), "date").into_iter().next(),
        expires: header_values(response.headers(), "expires")
            .into_iter()
            .next(),
    };
    let bytes = read_decoded_bounded(response, cap).with_context(|| format!("read {url}"))?;
    if bytes.len() > cap {
        bail!("{url}: decoded body exceeds the {cap}-byte bound");
    }
    Ok(FetchResult {
        bytes,
        meta,
        fetched_at,
        response_delay_secs,
    })
}

/// Parse an HTTP-date header (IMF-fixdate) to epoch seconds.
fn http_date_epoch_secs(raw: &str) -> anyhow::Result<i64> {
    let parsed = DateTime::parse_from_rfc2822(raw)
        .with_context(|| format!("HTTP date {raw:?} is not RFC 2822/IMF-fixdate"))?;
    Ok(parsed.timestamp())
}

/// Extract a cache-control directive's occurrences (name match is
/// case-insensitive; values are returned verbatim, `None` for a
/// value-less directive).
fn directive_occurrences<'meta>(
    cache_control: &'meta [String],
    name: &str,
) -> Vec<Option<&'meta str>> {
    let mut found = Vec::new();
    for header in cache_control {
        for part in header.split(',') {
            let trimmed = part.trim();
            let (directive, value) = match trimmed.split_once('=') {
                Some((name_half, value_half)) => {
                    (name_half, Some(value_half.trim().trim_matches('"')))
                }
                None => (trimmed, None),
            };
            if directive.trim().eq_ignore_ascii_case(name) {
                found.push(value);
            }
        }
    }
    found
}

/// One required, valid occurrence of a seconds-valued directive.
/// Duplicated or malformed occurrences FAIL (the fail-closed arm of
/// RFC 9111's first-occurrence-or-stale rule).
fn seconds_directive(cache_control: &[String], name: &str) -> anyhow::Result<Option<u64>> {
    let occurrences = directive_occurrences(cache_control, name);
    if occurrences.len() > 1 {
        bail!("Cache-Control carries a duplicated {name} directive");
    }
    let Some(first) = occurrences.first() else {
        return Ok(None);
    };
    let Some(raw_value) = first else {
        bail!("Cache-Control {name} directive has no value");
    };
    let secs: u64 = raw_value
        .parse()
        .with_context(|| format!("Cache-Control {name}={raw_value:?} is not a valid u64"))?;
    Ok(Some(secs))
}

/// RFC 9111 freshness for a shared cache: `s-maxage` takes precedence
/// over `max-age`; `no-cache` / `no-store` fail (a baked offline
/// artifact can neither revalidate nor refuse to persist); an invalid
/// value fails (invalid freshness information is stale per §4.2.1, not
/// "as if absent"); `Expires - Date` is consulted only when
/// Cache-Control carries no freshness directive at all. `current_age`
/// is computed per §4.2.3 (Age header plus response delay against
/// Date). Returns the REMAINING lifetime in seconds; stale on arrival
/// fails.
fn freshness_lifetime_secs(
    meta: &ResponseCacheMeta,
    response_delay_secs: u64,
    received_at: OffsetDateTime,
) -> anyhow::Result<u64> {
    if !directive_occurrences(&meta.cache_control, "no-cache").is_empty() {
        bail!(
            "upstream sent Cache-Control: no-cache - reuse requires revalidation, which a \
             baked offline artifact cannot perform; verify upstream headers"
        );
    }
    if !directive_occurrences(&meta.cache_control, "no-store").is_empty() {
        bail!(
            "upstream sent Cache-Control: no-store - the response may not be persisted; \
             verify upstream headers"
        );
    }
    let s_maxage = seconds_directive(&meta.cache_control, "s-maxage")?;
    let max_age = seconds_directive(&meta.cache_control, "max-age")?;
    let freshness: u64 = if let Some(from_cache_control) = s_maxage.or(max_age) {
        from_cache_control
    } else {
        // No Cache-Control freshness at all: Expires is the
        // lower-precedence fallback, never an override.
        let Some(expires_raw) = meta.expires.as_deref() else {
            bail!(
                "upstream response carries no freshness lifetime (no s-maxage, no \
                 max-age, no Expires); verify upstream headers"
            );
        };
        let expires_secs = http_date_epoch_secs(expires_raw)?;
        let date_secs = match meta.date.as_deref() {
            Some(date_raw) => http_date_epoch_secs(date_raw)?,
            None => received_at.unix_timestamp(),
        };
        u64::try_from(expires_secs.saturating_sub(date_secs))
            .context("Expires precedes Date - stale on arrival")?
    };
    let age_value: u64 = match meta.age.as_deref() {
        Some(age_raw) => age_raw
            .parse()
            .with_context(|| format!("Age header {age_raw:?} is not a valid u64"))?,
        None => 0,
    };
    let date_secs = match meta.date.as_deref() {
        Some(date_raw) => http_date_epoch_secs(date_raw)?,
        None => received_at.unix_timestamp(),
    };
    let apparent_age = u64::try_from(
        received_at
            .unix_timestamp()
            .checked_sub(date_secs)
            .unwrap_or(0),
    )
    .unwrap_or(0);
    let corrected_age_value = age_value.saturating_add(response_delay_secs);
    let current_age = apparent_age.max(corrected_age_value);
    if current_age >= freshness {
        bail!(
            "response is stale on arrival (current_age {current_age}s >= freshness \
             {freshness}s); verify upstream headers"
        );
    }
    let remaining = freshness
        .checked_sub(current_age)
        .context("freshness underflow")?;
    let remaining_i64 =
        i64::try_from(remaining).context("cache lifetime overflows the timestamp range")?;
    received_at
        .checked_add(Duration::seconds(remaining_i64))
        .context("fetched_at + cache lifetime overflows the timestamp range")?;
    Ok(remaining)
}

/// Extract the top-level `lastUpdated` string VERBATIM (the manifest
/// must carry it byte-for-byte; re-formatting a parsed instant could
/// drift from the blob).
fn extract_last_updated(raw: &[u8]) -> anyhow::Result<String> {
    let value: Value = serde_json::from_slice(raw).context("blob is not valid JSON")?;
    let last_updated = value
        .get("lastUpdated")
        .and_then(Value::as_str)
        .context("blob has no string lastUpdated")?;
    Ok(last_updated.to_owned())
}

/// Validate a fetched GVL blob against its slot spec and build the
/// staging record.
fn stage_gvl_blob(
    spec: u32,
    bytes: Vec<u8>,
    fetched_at: Option<String>,
    cache_max_age_secs: Option<u64>,
) -> anyhow::Result<ArchiveBlob> {
    let text = str::from_utf8(&bytes).context("GVL blob is not UTF-8")?;
    let validated = validate_gvl_json(text, Some(spec))
        .map_err(|schema_err| anyhow::anyhow!("GVL blob failed schema validation: {schema_err}"))?;
    let last_updated_raw = extract_last_updated(&bytes)?;
    let sha256 = sha256_hex(&bytes);
    Ok(ArchiveBlob {
        spec,
        version: validated.vendor_list_version,
        tcf_policy_version: validated.tcf_policy_version,
        last_updated_raw,
        last_updated: validated.last_updated,
        sha256,
        bytes,
        fetched_at,
        cache_max_age_secs,
    })
}

/// The newest spec-3 entry recorded in the live manifest, if any.
fn newest_live_spec3(live: Option<&LiveManifest>) -> Option<&LiveEntry> {
    live?
        .entries
        .iter()
        .filter(|entry| entry.spec == 3)
        .max_by_key(|entry| entry.vendor_list_version)
}

fn parse_rfc3339_lenient(raw: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(raw, &Rfc3339).ok()
}

/// Step 2: reuse the live current spec-3 list while its recorded
/// RFC 9111 lifetime holds; otherwise fetch it and record the lifetime
/// on its manifest entry.
fn fetch_current_spec3(
    client: &Client,
    live: Option<&LiveManifest>,
    live_dir: &Path,
    now: OffsetDateTime,
) -> anyhow::Result<ArchiveBlob> {
    if let Some(entry) = newest_live_spec3(live)
        && let (Some(fetched_raw), Some(secs)) =
            (entry.fetched_at.as_deref(), entry.cache_max_age_secs)
        && let Some(fetched_at) = parse_rfc3339_lenient(fetched_raw)
        && let Some(expires) = i64::try_from(secs)
            .ok()
            .and_then(|secs_i64| fetched_at.checked_add(Duration::seconds(secs_i64)))
        && now < expires
        && let Some(bytes) = live_blob_if_sha_matches(live_dir, &entry.path, &entry.sha256)
    {
        eprintln!(
            "reusing live spec-3 v{} (fresh until {expires})",
            entry.vendor_list_version
        );
        return stage_gvl_blob(3, bytes, entry.fetched_at.clone(), entry.cache_max_age_secs);
    }
    let fetched = fetch_bounded(
        client,
        SPEC3_CURRENT_URL,
        usize::try_from(GVL_FETCH_MAX_BYTES).unwrap_or(usize::MAX),
    )?;
    let lifetime = freshness_lifetime_secs(
        &fetched.meta,
        fetched.response_delay_secs,
        fetched.fetched_at,
    )
    .with_context(|| format!("{SPEC3_CURRENT_URL}: freshness"))?;
    let fetched_at_rfc3339 = fetched
        .fetched_at
        .format(&Rfc3339)
        .context("format fetched_at")?;
    stage_gvl_blob(3, fetched.bytes, Some(fetched_at_rfc3339), Some(lifetime))
}

/// Step 3 reservation: the frozen spec-2 list (final TCF v2.2 GVL) —
/// reused from the live archive when its bytes still match, else
/// fetched from `/v2/`.
fn fetch_frozen_spec2(
    client: &Client,
    live: Option<&LiveManifest>,
    live_dir: &Path,
) -> anyhow::Result<ArchiveBlob> {
    if let Some(manifest) = live {
        for entry in manifest.entries.iter().filter(|entry| entry.spec == 2) {
            if let Some(bytes) = live_blob_if_sha_matches(live_dir, &entry.path, &entry.sha256) {
                eprintln!("reusing live spec-2 v{}", entry.vendor_list_version);
                return stage_gvl_blob(2, bytes, None, None);
            }
        }
    }
    let fetched = fetch_bounded(
        client,
        SPEC2_CURRENT_URL,
        usize::try_from(GVL_FETCH_MAX_BYTES).unwrap_or(usize::MAX),
    )?;
    stage_gvl_blob(2, fetched.bytes, None, None)
}

/// Step 4: walk the archived spec-3 versions newest-first (excluding
/// the already-reserved current version), stopping at the first version
/// older than the retention window — every older archive is out of the
/// window too, so the walk never continues past it. The budget decision
/// happens afterwards in `select_retained_versions`.
fn fetch_spec3_candidates(
    client: &Client,
    live: Option<&LiveManifest>,
    live_dir: &Path,
    current_version: u32,
    retain_from: OffsetDateTime,
) -> anyhow::Result<Vec<ArchiveBlob>> {
    let live_by_version: BTreeMap<u32, &LiveEntry> = live
        .map(|manifest| {
            manifest
                .entries
                .iter()
                .filter(|entry| entry.spec == 3)
                .map(|entry| (entry.vendor_list_version, entry))
                .collect()
        })
        .unwrap_or_default();
    let mut candidates = Vec::new();
    let mut version = current_version.saturating_sub(1);
    while version >= 1 {
        let reused = live_by_version
            .get(&version)
            .and_then(|entry| live_blob_if_sha_matches(live_dir, &entry.path, &entry.sha256));
        let bytes = if let Some(reused_bytes) = reused {
            eprintln!("reusing live spec-3 archive v{version}");
            reused_bytes
        } else {
            let url = format!("{SPEC3_ARCHIVE_BASE_URL}/vendor-list-v{version}.json");
            fetch_bounded(
                client,
                &url,
                usize::try_from(GVL_FETCH_MAX_BYTES).unwrap_or(usize::MAX),
            )?
            .bytes
        };
        let blob = stage_gvl_blob(3, bytes, None, None)?;
        if blob.last_updated < retain_from {
            eprintln!(
                "archive walk stopped at v{version} ({}): older than the retention window",
                blob.last_updated_raw
            );
            break;
        }
        candidates.push(blob);
        version = version.saturating_sub(1);
    }
    Ok(candidates)
}

/// Step 2b: the GCL — reuse within the recorded cache period, else
/// fetch, bound the DECODED size, validate, and record the RFC 9111
/// lifetime.
fn fetch_gcl(
    client: &Client,
    live: Option<&LiveManifest>,
    live_dir: &Path,
    now: OffsetDateTime,
) -> anyhow::Result<GclStage> {
    if let Some(record) = live.and_then(|manifest| manifest.gcl.as_ref())
        && let Some(fetched_at) = parse_rfc3339_lenient(&record.fetched_at)
        && let Some(expires) = i64::try_from(record.cache_max_age_secs)
            .ok()
            .and_then(|secs| fetched_at.checked_add(Duration::seconds(secs)))
        && now < expires
        && let Some(bytes) = live_blob_if_sha_matches(live_dir, "gcl.json", &record.sha256)
        && validate_gcl_json(str::from_utf8(&bytes).unwrap_or_default()).is_ok()
    {
        eprintln!("reusing live gcl.json (fresh until {expires})");
        return Ok(GclStage {
            sha256: record.sha256.clone(),
            last_updated_raw: record.last_updated.clone(),
            fetched_at_rfc3339: record.fetched_at.clone(),
            cache_max_age_secs: record.cache_max_age_secs,
            bytes,
        });
    }
    let fetched = fetch_bounded(client, GCL_URL, GCL_MAX_RAW_BYTES)?;
    let text = str::from_utf8(&fetched.bytes).context("GCL is not UTF-8")?;
    validate_gcl_json(text)
        .map_err(|schema_err| anyhow::anyhow!("GCL failed schema validation: {schema_err}"))?;
    let lifetime = freshness_lifetime_secs(
        &fetched.meta,
        fetched.response_delay_secs,
        fetched.fetched_at,
    )
    .with_context(|| format!("{GCL_URL}: freshness"))?;
    let fetched_at_rfc3339 = fetched
        .fetched_at
        .format(&Rfc3339)
        .context("format fetched_at")?;
    Ok(GclStage {
        sha256: sha256_hex(&fetched.bytes),
        last_updated_raw: extract_last_updated(&fetched.bytes)?,
        fetched_at_rfc3339,
        cache_max_age_secs: lifetime,
        bytes: fetched.bytes,
    })
}

struct GclStage {
    sha256: String,
    last_updated_raw: String,
    fetched_at_rfc3339: String,
    cache_max_age_secs: u64,
    bytes: Vec<u8>,
}

/// Step 7-8 structural rules on the keep set: no duplicate
/// `(spec, version)` pairs, spec ∈ {2, 3}, positive versions, and
/// manifest-safe relative paths.
fn check_keep_set(kept: &[&ArchiveBlob]) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for blob in kept.iter().copied() {
        if !(blob.spec == 2 || blob.spec == 3) {
            bail!("keep set contains out-of-range spec {}", blob.spec);
        }
        if blob.version < 1 {
            bail!("keep set contains non-positive vendor_list_version");
        }
        if !seen.insert((blob.spec, blob.version)) {
            bail!(
                "keep set contains duplicate (spec {}, version {})",
                blob.spec,
                blob.version
            );
        }
        let rel = blob_rel_path(blob);
        if rel.contains("..") || rel.starts_with('/') {
            bail!("keep set produced unsafe path {rel:?}");
        }
    }
    Ok(())
}

/// Step 9: render the manifest with keys in canonical order and entries
/// sorted `(spec asc, vendor_list_version desc)`, carrying the optional
/// per-entry cache metadata where recorded.
fn render_manifest(
    now: OffsetDateTime,
    max_age_days: u32,
    oversize_bundle_ack: bool,
    gcl: &GclStage,
    kept: &[&ArchiveBlob],
) -> anyhow::Result<String> {
    let mut sorted: Vec<&ArchiveBlob> = kept.to_vec();
    sorted.sort_by(|left, right| {
        left.spec
            .cmp(&right.spec)
            .then_with(|| right.version.cmp(&left.version))
    });
    let entries = sorted
        .iter()
        .copied()
        .map(|blob| EntryOut {
            spec: blob.spec,
            vendor_list_version: blob.version,
            tcf_policy_version: blob.tcf_policy_version,
            last_updated: blob.last_updated_raw.clone(),
            path: blob_rel_path(blob),
            sha256: blob.sha256.clone(),
            fetched_at: blob.fetched_at.clone(),
            cache_max_age_secs: blob.cache_max_age_secs,
        })
        .collect();
    let manifest = ManifestOut {
        manifest_version: 1,
        generated_at: now.format(&Rfc3339).context("format generated_at")?,
        retention: RetentionOut { max_age_days },
        oversize_bundle_ack,
        gcl: GclOut {
            path: "gcl.json".to_owned(),
            sha256: gcl.sha256.clone(),
            last_updated: gcl.last_updated_raw.clone(),
            fetched_at: gcl.fetched_at_rfc3339.clone(),
            cache_max_age_secs: gcl.cache_max_age_secs,
        },
        entries,
    };
    let mut rendered = serde_json::to_string_pretty(&manifest).context("serialize manifest")?;
    rendered.push('\n');
    Ok(rendered)
}

/// Step 10: validate the ENTIRE transaction directory the exact way
/// build.rs will — shared manifest validator, per-entry schema + SHA
/// re-check, and the GCL leg (size, SHA, schema, manifest/blob
/// consistency) — before anything touches the live archive.
fn validate_tx_dir(tx_dir: &Path, now: OffsetDateTime) -> anyhow::Result<()> {
    let manifest_raw =
        fs::read_to_string(tx_dir.join("manifest.json")).context("read tx manifest.json")?;
    let manifest = validate_manifest(&manifest_raw, now)
        .map_err(|manifest_err| anyhow::anyhow!("tx manifest validation: {manifest_err}"))?;
    for entry in &manifest.entries {
        let bytes = fs::read(tx_dir.join(&entry.path))
            .with_context(|| format!("read tx {}", entry.path))?;
        if sha256_hex(&bytes) != entry.sha256 {
            bail!("tx sha mismatch for {}", entry.path);
        }
        let text =
            str::from_utf8(&bytes).with_context(|| format!("tx {} is not UTF-8", entry.path))?;
        validate_gvl_json(text, Some(entry.spec)).map_err(|schema_err| {
            anyhow::anyhow!("tx {} failed schema validation: {schema_err}", entry.path)
        })?;
    }
    let gcl_raw = fs::read_to_string(tx_dir.join("gcl.json")).context("read tx gcl.json")?;
    if gcl_raw.len() > GCL_MAX_RAW_BYTES {
        bail!("tx gcl.json exceeds GCL_MAX_RAW_BYTES");
    }
    if sha256_hex(gcl_raw.as_bytes()) != manifest.gcl.sha256 {
        bail!("tx gcl.json sha mismatch");
    }
    let validated_gcl = validate_gcl_json(&gcl_raw)
        .map_err(|schema_err| anyhow::anyhow!("tx gcl validation: {schema_err}"))?;
    check_gcl_consistency(&manifest, &validated_gcl)
        .map_err(|consistency_err| anyhow::anyhow!("tx gcl consistency: {consistency_err}"))?;
    Ok(())
}

fn sync_dir(dir: &Path) -> anyhow::Result<()> {
    fs::File::open(dir)
        .with_context(|| format!("open {}", dir.display()))?
        .sync_all()
        .with_context(|| format!("fsync {}", dir.display()))?;
    Ok(())
}

/// Step 11: under the EXCLUSIVE archive lock — heal any prior crashed
/// swap via the shared four-case recovery, then swap the transaction
/// directory into place. Two renames through the single fixed
/// `gvl_archive.old` aside when `live` exists; a direct install when it
/// does not (first install / clean state — the unconditional aside
/// rename would fail ENOENT). Each rename is individually atomic, the
/// pair is not: a crash between them leaves the archive at
/// `gvl_archive.old`, which the next exclusive-lock holder heals.
fn swap_tx_into_live(data_dir: &Path, tx_dir: &Path) -> anyhow::Result<()> {
    let lock_path = data_dir.join("gvl_archive.lock");
    let lock_file = fs::File::open(&lock_path)
        .with_context(|| format!("open archive lock {}", lock_path.display()))?;
    if lock_file.try_lock_exclusive().is_err() {
        bail!("refresh already running (gvl_archive.lock is held); discarding tx dir");
    }
    let live_dir = data_dir.join("gvl_archive");
    recover_gvl_archive(&live_dir).context("pre-swap recovery")?;
    let old_dir = data_dir.join("gvl_archive.old");
    if live_dir.exists() {
        fs::rename(&live_dir, &old_dir).context("rename live -> gvl_archive.old")?;
        sync_dir(data_dir)?;
        fs::rename(tx_dir, &live_dir).context("rename tx -> live")?;
        sync_dir(data_dir)?;
        fs::remove_dir_all(&old_dir).context("delete gvl_archive.old")?;
    } else {
        fs::rename(tx_dir, &live_dir).context("rename tx -> live (first install)")?;
        sync_dir(data_dir)?;
    }
    fs2::FileExt::unlock(&lock_file).context("unlock gvl_archive.lock")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use time::macros::datetime;

    /// Test-scoped counting allocator: cumulative per-thread allocated
    /// bytes, so concurrent tests never pollute each other's
    /// measurement. Delegates every operation to `System`.
    mod alloc_track {
        #![expect(
            unsafe_code,
            reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                      measurement backing the compressed-bomb allocation bound"
        )]

        use std::alloc::{GlobalAlloc, Layout, System};
        use std::cell::Cell;

        thread_local! {
            static ALLOCATED: Cell<u64> = const { Cell::new(0) };
        }

        pub fn allocated_on_this_thread() -> u64 {
            ALLOCATED.try_with(Cell::get).unwrap_or(0)
        }

        fn count(bytes: usize) {
            // TLS teardown can make try_with fail; a lost count only
            // relaxes the measurement, never breaks the allocator.
            let _outcome = ALLOCATED.try_with(|cell| {
                cell.set(
                    cell.get()
                        .saturating_add(u64::try_from(bytes).unwrap_or(u64::MAX)),
                );
            });
        }

        pub struct CountingAllocator;

        // SAFETY: every operation delegates verbatim to `System`, which
        // upholds the GlobalAlloc contract; the counting side effect
        // touches only a thread-local counter and never the returned
        // memory.
        unsafe impl GlobalAlloc for CountingAllocator {
            unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
                count(layout.size());
                // SAFETY: same layout contract as the caller's.
                unsafe { System.alloc(layout) }
            }

            unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
                // SAFETY: ptr/layout come from a matching System alloc.
                unsafe { System.dealloc(ptr, layout) }
            }

            unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
                count(layout.size());
                // SAFETY: same layout contract as the caller's.
                unsafe { System.alloc_zeroed(layout) }
            }

            unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
                count(new_size.saturating_sub(layout.size()));
                // SAFETY: ptr/layout come from a matching System alloc;
                // new_size is the caller's requested size.
                unsafe { System.realloc(ptr, layout, new_size) }
            }
        }
    }

    #[global_allocator]
    static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

    static TEST_DIR_NONCE: AtomicU64 = AtomicU64::new(0);

    /// Unique scratch dir under the workspace target tree; removed on
    /// drop.
    struct ScratchDir {
        path: PathBuf,
    }

    impl ScratchDir {
        fn new(label: &str) -> Self {
            let nonce = TEST_DIR_NONCE.fetch_add(1, Ordering::Relaxed);
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/auctioneer-refresh-gvl-archive-tests")
                .join(format!("{label}.{}.{nonce}", process::id()));
            fs::create_dir_all(&path).expect("create scratch dir");
            Self { path }
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _cleanup = fs::remove_dir_all(&self.path);
        }
    }

    fn meta(
        cache_control: &[&str],
        age: Option<&str>,
        date: Option<&str>,
        expires: Option<&str>,
    ) -> ResponseCacheMeta {
        ResponseCacheMeta {
            cache_control: cache_control.iter().map(|s| (*s).to_owned()).collect(),
            age: age.map(str::to_owned),
            date: date.map(str::to_owned),
            expires: expires.map(str::to_owned),
        }
    }

    // Received instant used by the cache-parser tests:
    // Thu, 27 Aug 2026 16:20:00 GMT.
    const RECEIVED: OffsetDateTime = datetime!(2026-08-27 16:20:00 UTC);

    #[test]
    fn lifetime_age_date_corrected() {
        // Date 30s before receipt, Age 100, delay 5:
        // current_age = max(30, 100 + 5) = 105 → 604800 - 105.
        let lifetime = freshness_lifetime_secs(
            &meta(
                &["max-age=604800"],
                Some("100"),
                Some("Thu, 27 Aug 2026 16:19:30 GMT"),
                None,
            ),
            5,
            RECEIVED,
        )
        .expect("fresh response");
        assert_eq!(lifetime, 604_695);
    }

    #[test]
    fn duplicate_max_age_fails() {
        let err = freshness_lifetime_secs(
            &meta(&["max-age=100, max-age=200"], None, None, None),
            0,
            RECEIVED,
        )
        .expect_err("duplicate max-age must fail");
        assert!(err.to_string().contains("duplicated max-age"));
    }

    #[test]
    fn s_maxage_takes_precedence_over_max_age() {
        let lifetime = freshness_lifetime_secs(
            &meta(&["s-maxage=1000, max-age=100"], None, None, None),
            0,
            RECEIVED,
        )
        .expect("fresh response");
        // Shared-cache precedence: s-maxage governs, minus zero age.
        assert_eq!(lifetime, 1000);
    }

    #[test]
    fn duplicate_s_maxage_fails() {
        let err = freshness_lifetime_secs(
            &meta(&["s-maxage=100", "s-maxage=200"], None, None, None),
            0,
            RECEIVED,
        )
        .expect_err("duplicate s-maxage must fail");
        assert!(err.to_string().contains("duplicated s-maxage"));
    }

    #[test]
    fn no_cache_fails() {
        let err = freshness_lifetime_secs(
            &meta(&["no-cache, max-age=604800"], None, None, None),
            0,
            RECEIVED,
        )
        .expect_err("no-cache must fail");
        assert!(err.to_string().contains("no-cache"));
    }

    #[test]
    fn no_store_fails() {
        let err = freshness_lifetime_secs(&meta(&["no-store"], None, None, None), 0, RECEIVED)
            .expect_err("no-store must fail");
        assert!(err.to_string().contains("no-store"));
    }

    #[test]
    fn expires_used_only_without_cc_freshness() {
        // No Cache-Control freshness → Expires - Date applies.
        let from_expires = freshness_lifetime_secs(
            &meta(
                &[],
                None,
                Some("Thu, 27 Aug 2026 16:20:00 GMT"),
                Some("Thu, 27 Aug 2026 17:20:00 GMT"),
            ),
            0,
            RECEIVED,
        )
        .expect("Expires fallback");
        assert_eq!(from_expires, 3600);
        // With a Cache-Control freshness directive, Expires is ignored
        // (lower precedence, never an override).
        let from_cache_control = freshness_lifetime_secs(
            &meta(
                &["max-age=100"],
                None,
                Some("Thu, 27 Aug 2026 16:20:00 GMT"),
                Some("Thu, 27 Aug 2026 17:20:00 GMT"),
            ),
            0,
            RECEIVED,
        )
        .expect("max-age governs");
        assert_eq!(from_cache_control, 100);
    }

    #[test]
    fn stale_on_arrival_fails() {
        let err = freshness_lifetime_secs(
            &meta(&["max-age=100"], Some("100"), None, None),
            0,
            RECEIVED,
        )
        .expect_err("stale on arrival must fail");
        assert!(err.to_string().contains("stale on arrival"));
    }

    #[test]
    fn expiry_overflow_fails() {
        let err = freshness_lifetime_secs(
            &meta(&["max-age=18446744073709551615"], None, None, None),
            0,
            RECEIVED,
        )
        .expect_err("u64::MAX lifetime must fail");
        assert!(err.to_string().contains("overflows the timestamp range"));
    }

    #[test]
    fn invalid_max_age_fails() {
        // Invalid freshness information is stale per RFC 9111 §4.2.1 —
        // never "as if absent".
        let err = freshness_lifetime_secs(&meta(&["max-age=abc"], None, None, None), 0, RECEIVED)
            .expect_err("invalid max-age must fail");
        assert!(err.to_string().contains("not a valid u64"));
    }

    #[test]
    fn refresh_tool_max_age_days_181_without_flag_errors() {
        let err = check_retention_flags(181, false).expect_err("181 days needs the ack flag");
        assert!(err.to_string().contains("--allow-oversize-bundle"));
        check_retention_flags(180, false).expect("default window needs no flag");
        check_retention_flags(181, true).expect("flag admits older entries");
    }

    #[test]
    fn refresh_tool_max_age_days_400_with_flag_writes_ack_true() {
        let gcl = GclStage {
            sha256: "a".repeat(64),
            last_updated_raw: "2026-08-27T16:00:21Z".to_owned(),
            fetched_at_rfc3339: "2026-08-28T00:00:00Z".to_owned(),
            cache_max_age_secs: 604_800,
            bytes: Vec::new(),
        };
        let blob = ArchiveBlob {
            spec: 3,
            version: 174,
            tcf_policy_version: 5,
            last_updated_raw: "2026-08-27T16:00:19Z".to_owned(),
            last_updated: datetime!(2026-08-27 16:00:19 UTC),
            sha256: "b".repeat(64),
            bytes: Vec::new(),
            fetched_at: None,
            cache_max_age_secs: None,
        };
        let rendered = render_manifest(
            datetime!(2026-08-28 00:00:00 UTC),
            400,
            true,
            &gcl,
            &[&blob],
        )
        .expect("render manifest");
        assert!(rendered.contains("\"oversize_bundle_ack\": true"));
    }

    #[test]
    fn manifest_entries_render_spec_asc_version_desc() {
        let gcl = GclStage {
            sha256: "a".repeat(64),
            last_updated_raw: "2026-08-27T16:00:21Z".to_owned(),
            fetched_at_rfc3339: "2026-08-28T00:00:00Z".to_owned(),
            cache_max_age_secs: 604_800,
            bytes: Vec::new(),
        };
        let make = |spec: u32, version: u32| ArchiveBlob {
            spec,
            version,
            tcf_policy_version: 5,
            last_updated_raw: "2026-08-27T16:00:19Z".to_owned(),
            last_updated: datetime!(2026-08-27 16:00:19 UTC),
            sha256: "b".repeat(64),
            bytes: Vec::new(),
            fetched_at: None,
            cache_max_age_secs: None,
        };
        let (v173, v174, v224) = (make(3, 173), make(3, 174), make(2, 224));
        let rendered = render_manifest(
            datetime!(2026-08-28 00:00:00 UTC),
            180,
            false,
            &gcl,
            &[&v173, &v174, &v224],
        )
        .expect("render manifest");
        let spec2_at = rendered.find("vendor-list-v224").expect("spec2 present");
        let v174_at = rendered.find("vendor-list-v174").expect("v174 present");
        let v173_at = rendered.find("vendor-list-v173").expect("v173 present");
        assert!(spec2_at < v174_at && v174_at < v173_at);
    }

    #[test]
    fn generated_180d_corpus_under_raw_cap() {
        // The committed archive (produced by this tool at the default
        // retention) must keep the growth margin free under the raw
        // cap, or weekly growth would break builds between refreshes.
        let archive_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../auctioneer-config-data/data/gvl_archive");
        let manifest_raw =
            fs::read_to_string(archive_dir.join("manifest.json")).expect("read live manifest");
        let live: LiveManifest = serde_json::from_str(&manifest_raw).expect("parse live manifest");
        let mut total: u64 = 0;
        for entry in &live.entries {
            let bytes = fs::read(archive_dir.join(&entry.path)).expect("read archive entry");
            total = total
                .checked_add(u64::try_from(bytes.len()).expect("fits u64"))
                .expect("no overflow");
        }
        let budget = WASM_BUNDLE_MAX_RAW_BYTES
            .checked_sub(GROWTH_MARGIN_BYTES)
            .expect("cap exceeds margin");
        assert!(
            total <= budget,
            "raw corpus {total} bytes exceeds the {budget}-byte size budget"
        );
    }

    #[test]
    fn gcl_compressed_bomb_rejected_at_cap() {
        use flate2::Compression;
        use flate2::read::GzDecoder;
        use flate2::write::GzEncoder;
        use std::io::Write as _;

        const DECODER_ALLOWANCE: u64 = 0x0020_0000;

        fn gzip(payload: &[u8]) -> Vec<u8> {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(payload).expect("gzip write");
            encoder.finish().expect("gzip finish")
        }

        // Build both compressed bodies BEFORE measuring.
        let bomb_gz = gzip(&vec![0_u8; 8 * GCL_MAX_RAW_BYTES]);
        let small_gz = gzip(&[0_u8; 1024]);

        // Baseline: the same decode-and-bound path on a small (1 KiB
        // decoded) response.
        let before_small = alloc_track::allocated_on_this_thread();
        let small = read_decoded_bounded(GzDecoder::new(small_gz.as_slice()), GCL_MAX_RAW_BYTES)
            .expect("bounded read");
        let baseline = alloc_track::allocated_on_this_thread().saturating_sub(before_small);
        assert_eq!(small.len(), 1024);

        let before_bomb = alloc_track::allocated_on_this_thread();
        let decoded = read_decoded_bounded(GzDecoder::new(bomb_gz.as_slice()), GCL_MAX_RAW_BYTES)
            .expect("bounded read");
        let bomb_alloc = alloc_track::allocated_on_this_thread().saturating_sub(before_bomb);

        // The over-cap outcome the fetch path rejects.
        assert!(decoded.len() > GCL_MAX_RAW_BYTES);
        // (a) The decoded buffer never materializes more than cap + 1
        // bytes — the capacity IS the allocation bound.
        assert_eq!(decoded.len(), GCL_MAX_RAW_BYTES + 1);
        assert_eq!(decoded.capacity(), GCL_MAX_RAW_BYTES + 1);
        // (b) Total allocation stays within the measured baseline plus
        // a fixed decoder allowance: the 8x-cap input inflates no
        // buffer anywhere.
        assert!(
            bomb_alloc <= baseline.saturating_add(DECODER_ALLOWANCE),
            "bomb decode allocated {bomb_alloc} bytes, baseline {baseline} + allowance {DECODER_ALLOWANCE}"
        );
    }

    fn write_marker(dir: &Path, name: &str) {
        fs::create_dir_all(dir).expect("create dir");
        fs::write(dir.join(name), b"marker").expect("write marker");
    }

    #[test]
    fn first_refresh_with_no_existing_archive_installs_directly() {
        let scratch = ScratchDir::new("first-install");
        let data = scratch.path.join("data");
        fs::create_dir_all(&data).expect("create data");
        fs::write(data.join("gvl_archive.lock"), b"").expect("create lock sentinel");
        let tx = data.join(".gvl_refresh.test.1");
        write_marker(&tx, "manifest.json");

        swap_tx_into_live(&data, &tx).expect("first install");

        assert!(data.join("gvl_archive/manifest.json").exists());
        assert!(
            !data.join("gvl_archive.old").exists(),
            "no aside dir may be created on first install"
        );
        assert!(!tx.exists());
    }

    #[test]
    fn swap_replaces_existing_live_and_removes_old() {
        let scratch = ScratchDir::new("swap-live");
        let data = scratch.path.join("data");
        fs::create_dir_all(&data).expect("create data");
        fs::write(data.join("gvl_archive.lock"), b"").expect("create lock sentinel");
        write_marker(&data.join("gvl_archive"), "stale.json");
        let tx = data.join(".gvl_refresh.test.2");
        write_marker(&tx, "manifest.json");

        swap_tx_into_live(&data, &tx).expect("swap");

        assert!(data.join("gvl_archive/manifest.json").exists());
        assert!(!data.join("gvl_archive/stale.json").exists());
        assert!(!data.join("gvl_archive.old").exists());
    }

    #[test]
    fn recovery_restores_old_when_live_absent() {
        let scratch = ScratchDir::new("recover-restore");
        let data = scratch.path.join("data");
        write_marker(&data.join("gvl_archive.old"), "manifest.json");

        recover_gvl_archive(&data.join("gvl_archive")).expect("recovery");

        assert!(data.join("gvl_archive/manifest.json").exists());
        assert!(!data.join("gvl_archive.old").exists());
    }

    #[test]
    fn crash_after_second_rename_deletes_stale_old_keeps_new_live() {
        let scratch = ScratchDir::new("recover-delete-old");
        let data = scratch.path.join("data");
        write_marker(&data.join("gvl_archive"), "new.json");
        write_marker(&data.join("gvl_archive.old"), "stale.json");

        recover_gvl_archive(&data.join("gvl_archive")).expect("recovery");

        assert!(
            data.join("gvl_archive/new.json").exists(),
            "the already-installed new archive is kept"
        );
        assert!(
            !data.join("gvl_archive.old").exists(),
            "the stale aside is deleted, not restored"
        );
    }

    #[test]
    fn refresh_is_rejected_while_lock_held() {
        let scratch = ScratchDir::new("lock-held");
        let data = scratch.path.join("data");
        fs::create_dir_all(&data).expect("create data");
        fs::write(data.join("gvl_archive.lock"), b"").expect("create lock sentinel");
        let holder = fs::File::open(data.join("gvl_archive.lock")).expect("open lock");
        holder.lock_exclusive().expect("hold lock");

        let tx = data.join(".gvl_refresh.test.3");
        write_marker(&tx, "manifest.json");
        let err = swap_tx_into_live(&data, &tx).expect_err("swap must be rejected");
        assert!(err.to_string().contains("refresh already running"));

        fs2::FileExt::unlock(&holder).expect("unlock");
    }

    #[test]
    fn build_rs_drops_shared_then_takes_exclusive_to_recover() {
        // The build-script recovery sequence: a shared lock finds
        // `live` absent, is DROPPED (fs2 has no portable upgrade), the
        // lock is re-acquired exclusive, recovery heals `.old → live`,
        // and the read happens while the exclusive is still held.
        let scratch = ScratchDir::new("drop-reacquire");
        let data = scratch.path.join("data");
        write_marker(&data.join("gvl_archive.old"), "manifest.json");
        fs::write(data.join("gvl_archive.lock"), b"").expect("create lock sentinel");

        let shared = fs::File::open(data.join("gvl_archive.lock")).expect("open lock");
        shared.lock_shared().expect("shared lock");
        assert!(!data.join("gvl_archive").exists());
        drop(shared);

        let exclusive = fs::File::open(data.join("gvl_archive.lock")).expect("open lock");
        exclusive.lock_exclusive().expect("exclusive lock");
        recover_gvl_archive(&data.join("gvl_archive")).expect("recovery");
        let recovered = data.join("gvl_archive/manifest.json");
        assert!(recovered.exists(), "read runs under the held exclusive");
        fs2::FileExt::unlock(&exclusive).expect("unlock");
    }

    #[test]
    fn build_rs_does_not_mutate_archive_while_refresh_lock_held() {
        // While the refresh holds the archive lock EXCLUSIVE (its
        // recovery + swap window), a build.rs reader cannot take even
        // the SHARED lock — so it can never observe a half-swapped
        // tree.
        let scratch = ScratchDir::new("reader-blocked");
        let data = scratch.path.join("data");
        fs::create_dir_all(&data).expect("create data");
        fs::write(data.join("gvl_archive.lock"), b"").expect("create lock sentinel");

        let refresh = fs::File::open(data.join("gvl_archive.lock")).expect("open lock");
        refresh.lock_exclusive().expect("refresh holds exclusive");

        let reader = fs::File::open(data.join("gvl_archive.lock")).expect("open lock");
        assert!(
            reader.try_lock_shared().is_err(),
            "a reader must block (not read) while the swap window is held"
        );

        fs2::FileExt::unlock(&refresh).expect("unlock");
        reader
            .try_lock_shared()
            .expect("reader proceeds once the swap window closes");
        fs2::FileExt::unlock(&reader).expect("unlock reader");
    }

    #[test]
    fn dead_holder_advisory_lock_does_not_wedge_next_run() {
        // An advisory flock releases when the holder's descriptor
        // closes (process death included) — a crash never wedges the
        // next run, unlike an O_EXCL create-a-file lock.
        let scratch = ScratchDir::new("dead-holder");
        let data = scratch.path.join("data");
        fs::create_dir_all(&data).expect("create data");
        fs::write(data.join("gvl_archive.lock"), b"").expect("create lock sentinel");

        let holder = fs::File::open(data.join("gvl_archive.lock")).expect("open lock");
        holder.lock_exclusive().expect("hold lock");
        drop(holder); // simulated crash: the OS releases the lock

        let next = fs::File::open(data.join("gvl_archive.lock")).expect("open lock");
        next.try_lock_exclusive()
            .expect("next run acquires the lock immediately");
        fs2::FileExt::unlock(&next).expect("unlock");
    }
}

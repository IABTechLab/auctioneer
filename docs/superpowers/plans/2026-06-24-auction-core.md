# auction core Auction Core — Implementation Plan

> Editorial normalization (2026-11-02): delivery labels and displayed local/branch names were replaced with capability names. Displayed locations are illustrative, not an exact execution transcript or a new run. Recorded revisions, results and qualification limitations are unchanged.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## How to use this plan

Round-25 review Medium 2: this file opens with ~400 lines of **review history** (rounds 12–24) that documents WHY specific design decisions landed. The executable plan starts at [`## Goal`](#goal) (search "## Goal" or jump to roughly line 825). For execution, follow this order:

1. Read the `## Goal`, `## Architecture`, `## Tech Stack`, `## Global Constraints`, and `## Prerequisites` sections that begin at `## Goal`.
2. Implement Tasks 1 through 44 in order. Each task is self-contained — code blocks, file paths, test commands, and verification steps live inline. The historical "Blocker / High / Medium" sections below DO NOT override task content; if you find a discrepancy, the inline task wins AND that is a plan bug worth flagging.
3. Use the review history only when you want background on a design decision — search for "Round-NN review" inline comments inside a task body. Those comments name the round and the finding so you can scroll back here to find the rationale.

The review history is preserved (not deleted) because round-25's own review still cites specific lines inside it (e.g. `[plan (line 466)]`); a future round-26+ revision pass may move it to a separate `docs/superpowers/plans/2026-06-24-auction-core-auction-history.md` file once no live findings reference it.

<details>
<summary><strong>Review-Round Reference Notes (rounds 12–24) — historical, click to expand</strong></summary>

Round-22 review Medium 10: the sections below were originally drafted as a patch-log of "fixes that supersede the inline task content." That framing was risky for agentic execution because two sources of truth could disagree. The inline task content HAS NOW been canonicalized — every code block, file list, and verification step in Tasks 1–44 is the authoritative source. Treat the sections below as REFERENCE NOTES that document why specific design decisions were made (typed-API alignment, EdgeZero macro path resolution, etc.); they do NOT override task content. If you find a discrepancy between a reference note here and an inline task, the inline task wins, AND that is a plan bug worth flagging in the next review.

The original auction core draft was reviewed at HEAD `5a0e9b4` and flagged with five blockers, five highs, and one medium. Successive rounds (rounds 13 through 22) extended the fixes. The inline tasks already incorporate every one of those fixes; the headings below are kept so the implementer can see the design history.

### Blocker 1 — foundations `BidResponse` / `BidRequest` typed-API alignment

The plan was drafted against an earlier shape where `BidResponse.seatbid`, `BidResponse.cur`, and `BidResponse.ext` were `Option`-wrapped and `ext` was `Box<RawValue>`. foundations ships them as flat / typed instead. Apply these translations everywhere they appear in the inline snippets:

| Plan snippet (stale)                                               | foundations reality                      | Fix                                                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------------ | ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `BidResponse.seatbid: Option<Vec<SeatBid>>`                        | `seatbid: Vec<SeatBid>`         | Drop the `Option`. Empty seatbid is `vec![]`, not `None`.                                                                                                                                                                                                                                                                      |
| `for seat in parsed.seatbid.iter().flatten()`                      | —                               | Replace with `for seat in &parsed.seatbid`.                                                                                                                                                                                                                                                                                    |
| `seatbid: if seatbids.is_empty() { None } else { Some(seatbids) }` | —                               | Just `seatbid: seatbids`.                                                                                                                                                                                                                                                                                                      |
| `resp.seatbid.as_ref().expect("seatbid populated")`                | —                               | `&resp.seatbid` (and assert `!is_empty()` separately if needed).                                                                                                                                                                                                                                                               |
| `BidResponse.cur: Option<String>`                                  | `cur: String` (default `""`)    | Drop `Option`. `cur: Some(c)` → `cur: c`. `resp.cur.as_deref() == Some("USD")` → `resp.cur == "USD"`.                                                                                                                                                                                                                          |
| `parsed.cur.clone().unwrap_or_else(\|\| "USD".into())`             | —                               | `if parsed.cur.is_empty() { "USD".into() } else { parsed.cur.clone() }`.                                                                                                                                                                                                                                                       |
| `BidResponse.ext: Option<Box<RawValue>>`                           | `Option<serde_json::Value>`     | Build a `serde_json::Value::Object`; store as `Some(value)`. Reads use `serde_json::to_string(value)` (or direct field navigation) instead of `.get()`.                                                                                                                                                                        |
| `serde_json::value::to_raw_value(&v).ok()`                         | —                               | `Some(v)`.                                                                                                                                                                                                                                                                                                                     |
| `resp.ext.as_ref().unwrap().get()`                                 | —                               | `&serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap()` for substring assertions.                                                                                                                                                                                                                                        |
| `BidRequest.cur: Option<Vec<String>>`                              | `cur: Vec<String>`              | Drop `Option`. `cur: Some(vec!["USD".into()])` → `cur: vec!["USD".into()]`. `req.cur.as_ref().map(\|c\| &c[..])` → `&req.cur[..]`.                                                                                                                                                                                             |
| `BidRequest.ext: Option<Box<RawValue>>`                            | `Option<BidRequestExt>` (typed) | Construct via `Some(BidRequestExt { prebid: Some(ExtRequestPrebid { aliases: …, returnallbidstatus: …, targeting: …, ..Default::default() }), ..Default::default() })`. Field navigation reads typed children (`req.ext.as_ref().and_then(\|e\| e.prebid.as_ref()).and_then(\|p\| p.returnallbidstatus)`), not raw-JSON walks. |
| `Imp.ext: Option<Box<RawValue>>`                                   | `Option<ImpExt>` (typed)        | Same translation: construct typed `ImpExt`; read via typed accessors.                                                                                                                                                                                                                                                          |
| `Bid.ext` raw-JSON merging into `prebid.targeting`                 | `Option<ExtBid>` typed          | Targeting attachment goes through `ExtBid::prebid_mut().targeting_mut()`; do not parse/serialize raw JSON to merge keys.                                                                                                                                                                                                       |

Apply these consistently across all four adapter snippets, the response-builder snippet, all run_auction tests, and the Mocktioneer / determinism E2E fixtures.

### Blocker 2 — `auctioneer-config` codegen updates for `ResourceLimits`

Task 23's `ResourceLimits` extension (`max_tmax_ms`, `max_bidder_response_bytes`, `max_bidders_per_auction`, `max_outbound_requests`) must include codegen + YAML wiring or the generated `HostConfig` will not compile. The task is hereby extended with these mandatory pre-steps before "Step 2: Write the failing tests":

1. **Update the `HostDefaultYaml` / `ResourceLimitsY` deserializer in `crates/auctioneer-config/build.rs`** to read the four new fields alongside `default_tmax_ms`. Each field is a YAML `i64` / `usize`; add `#[serde(default)]` so older fixtures still parse but a missing value in `data/host-defaults.yaml` is treated as `0` (validate-rejected below).
2. **Update the `emit_host_default` writer** to render the four new field assignments into the generated `HostConfig { resource_limits: ResourceLimits { … } }` literal.
3. **Update `crates/auctioneer-config/data/host-defaults.yaml`** with the values listed in Task 23 Step 1.
4. **Add codegen-time validation** in `build.rs` that rejects `max_tmax_ms == 0`, `max_bidder_response_bytes == 0`, `max_bidders_per_auction == 0`, or `max_outbound_requests == 0` — these are required positive caps per §5.3 and a missing/zero value is a config bug, not a usable default. Mirror the `validate_bidder_yaml_enums` pattern: a per-field error message that names the field and the offending value.
5. **Add a build_failure_harness test** that plants a `host-defaults.yaml` with `max_tmax_ms: 0` and confirms the build fails with the validation message. (Use a parallel env-var override pattern to the `AUCTIONEER_BIDDER_INFO_DIR` one — add a `AUCTIONEER_HOST_DEFAULTS_PATH` override under the same `test-harness-override` feature gate.)

After this, the existing Step 2 failing tests (referencing all four new fields) compile.

### Blocker 3 — Harden `validate_endpoint` (Task 22)

The original Task 22 snippet has two §5.4 holes:

1. **Userinfo / port stripping pre-comparison.** The plan strips `userinfo@` and `:port` from the parsed authority before host-equality / allow-list membership. That lets `https://attacker@hb.example.test/openrtb` and `https://hb.example.test:444/openrtb` pass the fixed-host check. The §5.4 rule is "same scheme, same authority" — strip nothing. The corrected approach:

   ```rust
   // Parse via url::Url. Reject malformed URLs up front (already
   // validated at codegen — at runtime this catches macro-expansion
   // mistakes by the adapter).
   let parsed = url::Url::parse(url)
       .map_err(|err| BidderError::failed_to_request_bids(
           format!("invalid URL after macro expansion: {err}")))?;

   // Scheme: must be https (or http only when insecure-bidders is
   // compiled in — see #[cfg] gating below). Verbatim match.
   match parsed.scheme() {
       "https" => {}
       #[cfg(feature = "insecure-bidders")]
       "http" => {}
       other => return Err(BidderError::failed_to_request_bids(
           format!("scheme {other:?} is not allowed"))),
   }

   // Userinfo is forbidden at every layer — block before authority
   // comparison so userinfo cannot mask the host.
   if !parsed.username().is_empty() || parsed.password().is_some() {
       return Err(BidderError::failed_to_request_bids(
           "userinfo is not permitted in bidder URLs".into()));
   }

   // Host comparison: full authority (host + port), not host-only.
   // Build the "canonical authority" string from the parsed URL,
   // including the port iff present.
   let final_authority = match parsed.port() {
       Some(port) => format!("{}:{port}", parsed.host_str().unwrap_or_default()),
       None       => parsed.host_str().unwrap_or_default().to_owned(),
   };

   if let Some(allow) = info.host_allowlist.as_deref() {
       // Host-varying: compare host_str() (host-only, per round-12
       // build-time decision) against the stored entries.
       let host = parsed.host_str().unwrap_or_default();
       if !allow.iter().any(|h| host.eq_ignore_ascii_case(h)) {
           return Err(BidderError::failed_to_request_bids(
               format!("host {host:?} is not in host_allowlist")));
       }
   } else {
       // Fixed-host: compare full authority (host + port) against the
       // BidderInfo.endpoint authority. Adapters cannot freelance a
       // different port either.
       let info_url = url::Url::parse(&info.endpoint)
           .map_err(|_| BidderError::failed_to_request_bids(
               "BidderInfo.endpoint is malformed (codegen bug)".into()))?;
       let info_authority = match info_url.port() {
           Some(port) => format!("{}:{port}", info_url.host_str().unwrap_or_default()),
           None       => info_url.host_str().unwrap_or_default().to_owned(),
       };
       if !final_authority.eq_ignore_ascii_case(&info_authority) {
           return Err(BidderError::failed_to_request_bids(
               format!("authority {final_authority:?} != BidderInfo.endpoint authority {info_authority:?}")));
       }
   }
   ```

2. **`cfg!(feature = "insecure-bidders")` runtime branch.** The original snippet uses `cfg!(feature = "…")` for the scheme/loopback relaxation, which leaves the insecure code compiled into default builds. The spec requires it to compile out: use `#[cfg(feature = "insecure-bidders")]` on the match arm AND on the loopback-literal allow-list helper. The default build must contain ZERO insecure-path code.

   ```rust
   #[cfg(not(feature = "insecure-bidders"))]
   fn loopback_or_alias_is_rejected(host: &str) -> Option<BidderError> {
       if is_loopback_literal(host) || is_localhost_alias_host(host) {
           return Some(BidderError::failed_to_request_bids(
               format!("loopback/alias host {host:?} is not allowed in default builds")));
       }
       None
   }

   #[cfg(feature = "insecure-bidders")]
   fn loopback_or_alias_is_rejected(host: &str) -> Option<BidderError> {
       // Insecure feature on: allow exactly the three loopback literals.
       const ALLOW: &[&str] = &["127.0.0.1", "::1", "localhost"];
       if ALLOW.iter().any(|a| host.eq_ignore_ascii_case(a)) { return None; }
       if is_loopback_literal(host) || is_localhost_alias_host(host) {
           return Some(BidderError::failed_to_request_bids(
               format!("non-literal loopback / alias {host:?} stays rejected even under insecure-bidders")));
       }
       None
   }
   ```

Both fixes get matching tests in `tests/validate_endpoint.rs`:

- `fixed_host_rejects_userinfo` (`https://a@hb.example.test/`)
- `fixed_host_rejects_port_mismatch` (`https://hb.example.test:444/` vs `BidderInfo.endpoint = "https://hb.example.test/"`)
- `host_varying_rejects_userinfo`
- `insecure_feature_is_compiled_out_by_default` (uses `#[cfg(not(feature = "insecure-bidders"))]` to assert the loopback path is unreachable)

### Blocker 4 — Real EdgeZero `#[action]` wiring (rewritten for round-15 review)

Earlier drafts used `pub async fn run_auction_handler(req: ProxyRequest) -> ProxyResponse` plus an `edgezero_core::app! { actions: { … } }` macro form. **The pinned EdgeZero API is different on every count**: the `#[edgezero_core::action]` attribute wraps the function around `RequestContext` and yields `Result<edgezero_core::http::Response, edgezero_core::error::EdgeError>` (see `edgezero-macros/src/action.rs` line 92), and `edgezero_core::app!` is a single-argument macro that reads a manifest path string (see `edgezero-macros/src/app.rs` line 12). `ProxyRequest` / `ProxyResponse` are OUTBOUND types (per `edgezero-core/src/proxy.rs` line 58) and never appear on an inbound action signature.

Handler module shape (replaces Task 38 Step 2):

```rust
// crates/auctioneer-core/src/handlers.rs
use edgezero_core::action;
use edgezero_core::context::RequestContext;
use edgezero_core::http::Response;
use edgezero_core::error::EdgeError;

#[action]
pub async fn auction(ctx: RequestContext) -> Result<Response, EdgeError> {
    // request_start captured FIRST — see Blocker 5 below.
    let clock = SystemClock::new();
    let request_start = clock.now_monotonic();
    auction_inner(ctx, clock, request_start).await
}

#[action]
pub async fn status(ctx: RequestContext) -> Result<Response, EdgeError> { /* … §8.1 status table … */ }

#[action]
pub async fn info_bidders(ctx: RequestContext) -> Result<Response, EdgeError> { /* … */ }

#[action]
pub async fn info_bidder(ctx: RequestContext) -> Result<Response, EdgeError> { /* … */ }
```

And a single crate-root macro invocation registers the manifest:

```rust
// crates/auctioneer-core/src/lib.rs
edgezero_core::app!("../../edgezero.toml");
```

(The path is relative to the macro call site's `CARGO_MANIFEST_DIR`. From `crates/auctioneer-core/Cargo.toml`'s directory, two levels up reaches the workspace root, where `edgezero.toml` lives.)

`edgezero.toml` (replaces Task 35's block) references handlers by a Rust path that resolves AT THE `app!` MACRO'S EXPANSION SITE (the auctioneer-core crate root) under `handler = "..."` and lists methods under `methods = [...]`. Round-18 review Blocker 1 — use `crate::handlers::auction` form; `auctioneer_core::handlers::auction` would require `extern crate self as auctioneer_core;` because Rust does not allow a crate to refer to itself by name inside its own source.

```toml
[[triggers.http]]
handler = "crate::handlers::auction"
methods = ["POST"]
path    = "/openrtb2/auction"

[[triggers.http]]
handler = "crate::handlers::status"
methods = ["GET"]
path    = "/status"

[[triggers.http]]
handler = "crate::handlers::info_bidders"
methods = ["GET"]
path    = "/info/bidders"

[[triggers.http]]
handler = "crate::handlers::info_bidder"
methods = ["GET"]
path    = "/info/bidders/{name}"
```

The manifest schema has no `action`, no per-route `body_cap`, no singular `method` field — those slots from earlier drafts would deserialize as ignored fields (`#[serde(default)]` + missing slot = no-op) and the routes would never bind. Body-cap enforcement is delegated to the ingress hook + the foundational `body_caps::match_route` table per §12 open item 14; the route's `path` is the lookup key.

### Blocker 5 — `request_start` captured at handler entry

The original Task 38 captures `request_start` after `body.read()` and `prepare()`. Per spec §5.3 line 1842, preprocessing time (body read, parse, prepare, alias resolution) counts against the deadline. Capture `request_start` as the FIRST statement in each `#[action]` body:

```rust
use edgezero_core::{
    action,
    body::Body,
    context::RequestContext,
    error::EdgeError,
    http::{response_builder, Response, CONTENT_TYPE},
};
use http::{HeaderValue, StatusCode};

#[action]
pub async fn auction(ctx: RequestContext) -> Result<Response, EdgeError> {
    // §5.3 + §5.8: the clock starts ticking at handler entry, BEFORE
    // body read / parse / prepare — preprocessing time must count
    // against tmax so an adapter doesn't get a bigger budget when the
    // request is large or `prepare` is slow.
    let clock = SystemClock::new();
    let request_start = clock.now_monotonic();
    match run_auction_pipeline(ctx, clock, request_start).await {
        Ok(response) => Ok(response),
        Err(err)     => handler_error_to_response(err),
    }
}

async fn run_auction_pipeline(
    ctx: RequestContext,
    clock: SystemClock,
    request_start: std::time::Duration,
) -> Result<Response, HandlerError> {
    // Round-16 review Blocker 1: bounded read is `Body::into_bytes_bounded`
    // on the pinned EdgeZero API, not `ctx.body_with_cap`. The handler
    // takes `RequestContext` by move so it can split out the body via
    // `into_request()` once the headers have been read.
    let raw_headers = ctx.request().headers().clone();
    let request = ctx.into_request();
    let (_parts, body) = request.into_parts();
    let body_bytes = body.into_bytes_bounded(AUCTION_CAP_BYTES).await
        .map_err(|e| HandlerError::from_body_read_error(e, AUCTION_CAP_BYTES))?;
    let raw: BidRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| HandlerError::bad_request(format!("Invalid request: {e}")))?;
    let headers = headers_from_http(&raw_headers);
    let normalized = prepare(raw, &headers)
        .map_err(|e| HandlerError::bad_request(format!("Invalid request: {e}")))?;
    // FanoutSeam reads the proxy handle BEFORE the body was consumed —
    // see Task 38 Step 3a for the actual hoisting order in the full
    // handler. This preamble snippet sketches the shape; the full
    // listing covers the borrow ordering.
    let _ = (&clock, request_start, normalized);
    unimplemented!("see Task 38 for the full handler body")
}
```

The direct `run_auction` invocation in the Mocktioneer E2E (§5.8 trusted-server reuse) also captures `request_start` BEFORE calling `prepare()`.

### High 6 — Cloudflare-specific Clock / Rng shim task

Add a new Task **38b: Cloudflare host primitives shim** before Task 38, with:

- **Files:** `crates/auctioneer-adapter-cloudflare/src/clock.rs`, `crates/auctioneer-adapter-cloudflare/src/rng.rs`, plus `tests/host_primitives.rs`.
- **Produces:**
  - `pub struct CloudflareClock;` implementing `auctioneer_runtime::clock::Clock` via `worker::Date::now()` for wall-time and a monotonic source derived from `js_sys::Date::now()` / `performance.now()` (whichever is exposed by the workers-rs version pinned at foundations).
  - `pub struct CloudflareRng;` implementing `auctioneer_runtime::rng::Rng` via xorshift64* seeded from `worker::Date::now() XOR worker::js_sys::Math::random()` (round-19 review Medium 11 + spec line 4539). Earlier drafts described a `crypto.getRandomValues` shim — that approach is RESCINDED; the spec pins the dev RNG to seeded xorshift for determinism under test.
- **Wired** by `auctioneer-adapter-cloudflare/src/lib.rs` calling `auctioneer_core::host_primitives::registry::install_host_primitives(...)` at startup (round-19 review Blocker 1 + 2 — the EdgeZero `app!` macro expands inside auctioneer-core, so no adapter-side manifest middleware can resolve; the registry is a core-owned `OnceLock` populated at adapter startup before `run_app::<auctioneer_core::App>(...)`).

Fastly and Spin already wire their own host primitives in their respective adapter crates; this task fills the Cloudflare parity gap.

The corrected Task 38 (auction handler) accepts `clock: &dyn Clock` and `rng: &dyn Rng` from the adapter rather than hardcoding `SystemClock` / `DevRng`.

### High 7 — Real bidder context on `BidderRequestInfo`

The `run_auction` snippet at the original line 5080 stubs out the bidder context (`endpoint_macros = &[]`, `host_allowlist = None`, `gpc = None`). With the Round 12 codegen update, `BidderInfo` carries `endpoint_macros` and `host_allowlist` for real; the auction loop must propagate them:

```rust
// Replaces the original Task 31 inner loop:
let macros: Vec<&str> = info.endpoint_macros.iter().map(String::as_str).collect();
let allow: Option<Vec<&str>> = info.host_allowlist.as_ref()
    .map(|h| h.iter().map(String::as_str).collect());
let info_for_adapter = BidderRequestInfo {
    bidder: info_alias.clone(),
    core_name: info_core,
    account_id: &ctx.account.id,
    endpoint_template: &info.endpoint,
    endpoint_macros: &macros,
    host_allowlist: allow.as_deref(),
    // Round-14 review Blocker GPC: per spec §5.1 step 4 / line 997,
    // the spec rule is `regs.ext.gpc == "1"`, not a header. prepare()
    // already folds the Sec-GPC header into the typed regs.ext.gpc
    // slot during normalization; reading from the prepared body is
    // the only consistent source. AuctionContext does NOT need a
    // separate headers field for this.
    gpc: underlying.regs.as_ref()
        .and_then(|r| r.ext.as_ref())
        .and_then(|e| e.gpc.as_deref()),
    entry_point: EntryPoint::Auction,
    currency: &underlying.cur,            // BidRequest.cur is Vec<String> not Option
};
```

The `gpc` value comes from the normalized request body (`req.regs.ext.gpc`). `prepare()` already folds the `Sec-GPC` HTTP header into that typed slot during normalization, so `run_auction` reads the same `Option<String>` whether the signal arrived as a header or as inline JSON. No additional `headers` field is needed on `AuctionContext`.

### High 8 — Real targeting from request + account

Task 32 + 33 hardcoding `PriceGranularity::Med`, `BidRounding::Down`, `prefix = "hb"`, `include_winners/bidder_keys = true`, `include_format = false`, `preferdeals = false` is non-conformant. Per spec §5.6 + Appendix E.1–E.4 the values come from the request (`req.ext.prebid.targeting`) with account-level fallback. Replace the original "Phase 1 MVP" block in `run_auction` (line 5267) with:

```rust
// Spec §5.6 / Appendix E.1–E.4: targeting block from request, with
// account-level fallback. The request shape is fully typed in foundations
// (ExtRequestTargeting); no raw-JSON parsing.
let targeting_cfg = underlying.ext.as_ref()
    .and_then(|e| e.prebid.as_ref())
    .and_then(|p| p.targeting.as_ref());
let granularity = targeting_cfg
    .and_then(|t| t.pricegranularity.as_ref())
    .map(parse_price_granularity)
    .unwrap_or(PriceGranularity::from_account(&ctx.account.config));
let rounding = BidRounding::from_account(&ctx.account.config);
let include_winners = targeting_cfg
    .and_then(|t| t.includewinners)
    .unwrap_or(true);
let include_bidder_keys = targeting_cfg
    .and_then(|t| t.includebidderkeys)
    .unwrap_or(true);
let include_format = targeting_cfg
    .and_then(|t| t.includeformat)
    .unwrap_or(false);
let prefer_deals = targeting_cfg
    .and_then(|t| t.preferdeals)
    .unwrap_or(false);
let prefix = targeting_cfg
    .and_then(|t| t.prefix.as_deref())
    .unwrap_or("hb");
let truncate_target_attribute = ctx.account.config.truncate_target_attribute;
```

Add `parse_price_granularity` per Appendix E.1 (string aliases `low|med|high|auto|dense|custom`, plus the custom-bucket JSON form). `pick_winner` takes `prefer_deals` (Task 31 inner loop already plumbs the variable through — make it the resolved value, not `false`).

Add corresponding tests in `tests/targeting_drives.rs`:

- `granularity_from_request_overrides_account`
- `prefix_from_request_overrides_default`
- `include_format_off_by_default`
- `prefer_deals_from_request_picks_deal_bid_over_higher_price_no_deal`

### High 9 — Dep-purity vs `tokio` dev-dep conflict

Task 41's dependency-purity assertion fails any library crate transitively depending on `tokio`, while Task 33 adds `tokio` as a dev-dep on `auctioneer-exchange`. Two options; pick option **A** (cheaper):

**Option A (default): drive async tests with `futures::executor::block_on`.** Drop the `tokio` dev-dep on `auctioneer-exchange`. Replace `#[tokio::test] async fn …` with:

```rust
#[test]
fn run_auction_round_trips_generic_bid() {
    futures::executor::block_on(async {
        // … original test body …
    });
}
```

`futures` is already in the workspace deps per the plan's Tech Stack line. No new crates. The purity walk stays strict.

**Option B (fallback): scope the purity walk to `[dependencies]` only.** If a future task genuinely needs `#[tokio::test]` (e.g. multi-task fanout simulation), Task 41 must explicitly skip `[dev-dependencies]` when computing the dep graph and rename to `assert_runtime_purity` so the intent is unambiguous. Document the carve-out in the test's docstring.

Pick A in auction core. Revisit only if GDPR enforcement needs feature-gated tokio for real.

### High 10 — Mocktioneer E2E must cover BOTH paths

Task 40's "Mocktioneer snapshot E2E" claims handler + direct `run_auction` coverage but only exercises direct `run_auction`. Add a second test in the same module that drives the same fixtures through the `#[action]` entry point:

```rust
#[test]
fn mocktioneer_e2e_through_handler() {
    futures::executor::block_on(async {
        let mock_addr = mocktioneer::spawn().await;
        let raw_body = std::fs::read("tests/fixtures/mocktioneer-request.json").unwrap();
        let req = test_proxy_request(
            "POST",
            "/openrtb2/auction",
            raw_body,
            &[("content-type", "application/json")],
        );
        let resp = auctioneer_core::handlers::auction(req).await;
        let body = resp.into_body_bytes().await;
        let snapshot = std::fs::read("tests/fixtures/mocktioneer-response.json").unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            serde_json::from_slice::<serde_json::Value>(&snapshot).unwrap(),
            "handler path must produce the same response as direct run_auction",
        );
    });
}

#[test]
fn mocktioneer_e2e_direct_run_auction() {
    // … the original direct-invocation test, retained verbatim …
}
```

This proves the §5.8 trusted-server-reuse invariant: handler path and direct `run_auction` produce identical byte output.

### Medium 11 — `bidder_timeout` location

The spec line 1202 names `bidder_timeout` as per-bidder `BidderInfo` data, but the plan substitutes the auction-wide `bidder_visible_tmax_ms`. **Decision: add `bidder_timeout_ms: Option<u32>` to `BidderInfo` (build-time only — auction core)**, with semantics "if `Some`, the per-bidder timeout the fanout applies for this bidder; if `None`, fall back to `bidder_visible_tmax_ms`". This matches the spec wording and keeps the change build-time (no runtime override path).

Implementation:

- Add `bidder_timeout_ms: Option<u32>` to `BidderInfo` in `crates/auctioneer-config/src/types.rs`.
- Read `bidderTimeoutMs` from the YAML in `build.rs` (`#[serde(default, rename = "bidderTimeoutMs")]`).
- Render into the generated `BidderInfo` literal alongside `gvl_vendor_id`.
- In `run_auction`'s call-list construction (Task 31 line 5147), replace `timeout: Duration::from_millis(u64::from(bidder_visible_tmax_ms))` with:

  ```rust
  let timeout_ms = info.bidder_timeout_ms
      .map(u64::from)
      .unwrap_or(u64::from(bidder_visible_tmax_ms));
  let timeout = Duration::from_millis(timeout_ms);
  ```

Add one `url_safety` / codegen test that proves the new field round-trips through the YAML.

---

> **Round-23 review Medium 7 — preamble/canonicalization clarification.** The Round-12 type-translation table (Blocker 1 above) is **history-only**. After the round-22 canonicalization pass, every inline task snippet was rewritten to use the foundations typed-API shapes directly — there is no remaining "stale type" inside Tasks 1–44 that needs translation. Implementers should NOT translate inline task code against that table; the table is preserved purely so reviewers can see why a given design landed where it did. The same applies to every "Blocker / High block above": their fixes are already baked into the inline tasks. If you ever spot a snippet that looks like it needs translation, that is a plan bug — flag it instead of silently re-applying the table.

</details>

---

## Goal

**Goal:** Land the auction core as **"auction core development" scope** — every API surface, type, and trait pinned in the spec is wired, every gate the spec calls "build-time" or "primary" is enforced, and every error code reachable from the auction core wiring surfaces correctly. After auction core, `POST /openrtb2/auction`, `GET /status`, `GET /info/bidders`, and `GET /info/bidders/{name}` are wired in `auctioneer-core` as thin shims over the EdgeZero-free, reusable `auctioneer_exchange::run_auction` entry point (spec §5.8), backed by the synchronous `Bidder` trait + 4 reference adapters (`generic`, `appnexus`, `pubmatic`, `mocktioneer`-behind-`insecure-bidders`), the `EndpointBuilder` + `validate_endpoint` SSRF gates (§5.4), the deterministic response builder (Appendix E + E.7), the targeting / price-granularity / winner-selection algorithm (Appendix E.1-E.4), the engine error model with the codes auction core can reach (`AuctionError` → `BadInput` code 2 / `BadServerResponse` 4 / `FailedToRequestBids` 5 / `BidderTemporarilyDisabled` 6 / `TmaxTimeout` 12 / `FailedToUnmarshal` 14 / `InvalidImpFirstPartyData` 15 / `Unknown` 999), and the `auctioneer-privacy` **API-surface-only** stub (`PrivacyContext::allow_all_no_scrub()` + `resolve_sync_privacy` permissive stub). Snapshot Mocktioneer E2E with byte-stable `BidResponse` assertions exercises both the EdgeZero handler path and a direct `run_auction` invocation (proving §5.8 trusted-server reuse).

> **Round-15 review Medium code 10:** `MalformedAcct` (code 10) is REMOVED from the auction core goal. The plan's account resolution is the `DefaultFallback` stub (no config_store wiring — GDPR enforcement deliverable), so the runtime path that produces `MalformedAcct` is not reachable from any auction core entry point. The variant stays in the `AuctionError` enum so GDPR enforcement's config_store wiring can surface it without an API break, but auction core acceptance does NOT assert code-10 behavior. Adding config-store resolution + the corresponding tests is GDPR enforcement scope.

> **Round-13 review High 6 — explicit auction core-dev vs auction core-behavior-complete split.** §5.3 resource-limit enforcement lands in auction core as **partial coverage**: body caps (via the foundational `body_caps::match_route`), `max_bidder_response_bytes`, `max_bidders_per_auction`, `max_outbound_requests`, tmax effective-budget resolution + monotonic deadline anchoring + `remaining == 0` short-circuit are enforced. **AuctionDeadline mid-fanout cancellation and per-bidder `bidder_timeout` enforcement are documented gaps** that land alongside the production `BidderFanout` impl in GDPR enforcement — the dev `ProxyClient + futures::join_all` fanout in auction core does not cancel in-flight calls when the deadline passes, and the per-bidder timeout is materialized into `BidderCall.timeout` (Round-12 Medium 11) but not yet enforced by the dev fanout (which applies a single overall deadline). Task 37 spells this out; the acceptance checklist (end-of-plan) flags these as "auction core development complete" rather than "auction core behavior-complete," and GDPR enforcement's plan opens with the deadline-enforcing fanout as its first task.

**Architecture:** Six new crate additions / wirings on top of the foundational foundation.

- **`auctioneer-privacy` (NEW, API surface only).** The auction-path `PrivacyContext` stub (`allow_all_no_scrub()`) plus the sync API surface (`SyncPrivacyInput`, `SyncPrivacyError`, `SyncerLookup`, `SyncDecision`, `SyncPrivacyContext`, `resolve_sync_privacy`) with a permissive stub (`sync_allowed → Allowed`, `setuid_allowed → Allowed`, `host_cookie_sync_allowed == true`, `activity_allow` permits everything). user sync swaps in the real impl without changing the API surface (spec §7.8 capability-ownership block). No TCF v2 / US-Privacy / GPP parsing here — those land in GDPR enforcement/privacy enforcement/user sync.
- **`auctioneer-bidder` (NEW).** The synchronous, EdgeZero-free `Bidder` trait, `BidderHttpRequest` / `BidderHttpResponse` / `BidderResponse` / `BidderError` / `BidderName` / `BidderRequestInfo` types, the `EndpointBuilder` helper (macro percent-encoding, path-traversal rejection, `host_allowlist` membership lookup), the `BidderRegistry`, and the four reference adapters: `generic` (OpenRTB passthrough), `appnexus` (custom request transform), `pubmatic` (custom response transform + gzip), and `mocktioneer` (under `insecure-bidders` cargo feature, points at `http://127.0.0.1:8787/openrtb2/auction` — see round-24 fixture-alignment note below). Canonical home of the `insecure-bidders` Cargo feature (§5.4 forwarding chain).

> **Round-24 review Medium — Mocktioneer endpoint single source of truth.** The Mocktioneer outbound endpoint is canonicalized to `http://127.0.0.1:8787/openrtb2/auction`. The foundations fixture at `crates/auctioneer-config/data/bidder-info-insecure/mocktioneer.yaml` already ships this value, and Cloudflare's `wrangler dev` defaults to port 8787, so the same URL works against either a hand-rolled mock server or a co-located wrangler-hosted Mocktioneer. Earlier plan drafts referenced `127.0.0.1:9000/openrtb`, which was a pre-foundations placeholder; that URL is wrong and MUST NOT appear in any new code, test, fixture, or documentation. If a future task wants a different port for collision avoidance (e.g. parallel CI), update the YAML fixture FIRST and pull the value from `BidderInfo.endpoint` rather than re-hardcoding.

- **`auctioneer-exchange` (NEW).** The auction engine: `BidderFanout` trait + neutral types (`BidderCall`, `BidderCallResult`, `FanoutError`, `FanoutInternalError`, `AuctionDeadline`, `TmaxSource`, `Compression`), `AuctionContext`, `ResolvedAccount`, `AccountSource`, `AuctionError`, the `validate_endpoint(url, &BidderInfo)` SSRF gate (full §5.4 ruleset), the `run_auction(req, ctx) -> Result<BidResponse, AuctionError>` entry point that performs §5.1 steps 6-11 (privacy preprocess → alias resolution + bidder-param merge + per-imp split → privacy-per-bidder → fan-out → collect → targeting + winners), and the deterministic response builder (Appendix E.7 — bidders processed in §5.9 registry order, `seatbid` sorted by seat, `ext.*` maps sorted by key). EdgeZero-free (uses auction core's neutral `BidderFanout` trait); the production fan-out impl lives in `auctioneer-core`.
- **`auctioneer-config` (MODIFIED).** Add the `EndpointBuilder`-feeding macro list to codegen (`endpoint_macros: &'static [&'static str]`), declare the `insecure-bidders = []` feature, propagate the feature gate into `build.rs` for the syncer-URL template-validation pass (relax to localhost-only allowlist under feature-on). Existing types stay; the foundations bidder fixtures already carry the data — only `bidder_info` accessor exposure widens.
- **`auctioneer-core` (MODIFIED).** Wire `[[triggers.http]]` entries for `/openrtb2/auction` (POST), `/status` (GET), `/info/bidders` (GET), `/info/bidders/{name}` (GET) in `edgezero.toml`. Build the host-environment seam in `auctioneer-core` per §5.7: production `Clock` (`SystemClock`), production `Rng` (target-specific dev `Rng` per §14 prereq list), and the **dev-only** `BidderFanout` impl backed by `ProxyClient` + `futures::future::join_all` (the interim impl per §5.2 with known gaps — no deadline enforcement, Fastly serialized, post-buffer `max_response_bytes`). Implement four handlers as thin shims over `run_auction`. Forward the `insecure-bidders` feature.
- **`auctioneer-adapter-*` (MODIFIED).** Each of the four adapter crates forwards the `insecure-bidders` feature to `auctioneer-core/insecure-bidders` so a single `--features insecure-bidders` on any adapter `cargo build` reaches every gate.

**Tech Stack:** Rust 2021. New workspace dependencies: `async-trait` (for the `dyn`-safe `BidderFanout` trait, §5.2), `futures` (for `join_all` in the dev `BidderFanout`), `flate2` (gzip encoding for `endpoint_compression`), `percent-encoding` (`EndpointBuilder` macro values), `bytes` and `http` (already in workspace). No `tokio` outside the Axum adapter; `auctioneer-exchange` is target-agnostic. No EdgeZero types reach `auctioneer-bidder` or `auctioneer-exchange`. Spec: `docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md`.

**Global Constraints:**

- **TDD.** Tests are written before implementation. Each task: write failing test → run to verify it fails → minimal impl → run to verify it passes → commit.
- **Crate purity.** No library crate may transitively depend on `edgezero-*`, `tokio`, `reqwest`, `fastly`, `worker`, or `spin-sdk`. The dependency-purity test (§11.2) ships in auction core alongside the crates it gates. Only `auctioneer-core` and the four `auctioneer-adapter-*` crates may pull EdgeZero or runtime crates.
- **Async surface.** `Bidder` is synchronous (§4.1). `BidderFanout` is async via `#[async_trait(?Send)]` (§5.2). `run_auction` is `async`; the returned future is `?Send`-friendly (§1.2). No `Send` requirement on futures, anywhere — edge runtimes are single-threaded per request.
- **No EdgeZero types** in any library crate, including `auctioneer-exchange`. The fan-out seam is the neutral `BidderFanout` trait; the `auctioneer-core` impl is the only code that touches EdgeZero outbound types.
- **Determinism.** Identical `NormalizedBidRequest` + identical mocks + `FakeClock` + fixed `SeededRng` seed → byte-identical `BidResponse` across runs and targets. Bidders are processed in §5.9 registry order (single lexicographic sort over participating base + alias names). `seatbid` is sorted by seat; `ext.*` maps are sorted by key.
- **Debug surface suppressed.** `run_auction` never populates `response.ext.debug` in auction core. `ext.warnings.prebid[]` carries no `AccountLevelDebugDisabled` (10002) warning during the auction core through user sync suppression window (round-33 H2 + round-34 GDPR enforcement). Real debug + redaction lands in diagnostic qualification.
- **Privacy is stub-only.** `PrivacyContext::allow_all_no_scrub()` returns a context where every activity is allowed, every `AuctionPermissions` is `AllowAll`, `ccpa_should_enforce == false`, scrub is a no-op. `resolve_sync_privacy` returns a `SyncPrivacyContext` where both per-syncer callbacks return `Allowed`, `host_cookie_sync_allowed == true`, and `activity_allow` permits everything. GDPR enforcement swaps in real GDPR; privacy enforcement swaps in real CCPA/GPP/activities/scrubbing; user sync swaps in the real `resolve_sync_privacy`. The API surface is auction core's contract and does not change between auction core and user sync.
- **Sync routes are not wired in auction core.** `/cookie_sync`, `/setuid`, `/getuids` ship in user sync. auction core-privacy enforcement builds expose only the four auction-shaped routes.
- **Endpoint macro scope (round-25 review Medium 1, revised round-26 review High).** auction core supports exactly THREE endpoint-template tokens: `{{.AccountID}}` (request-derived, `ResolvedAccount.id`), `{{.PublisherID}}` (raw `site.publisher.id` / `app.publisher.id` — round-26 review High; spec line 1331-1333; the two are intentionally distinct), and `{{.Host}}` (the host-allowlist authority selector, set by an explicit `build_outbound_url_with_host(info, host)` call from the adapter). Every privacy macro the spec mentions — `{{.GDPR}}`, `{{.GDPRConsent}}`, `{{.USPrivacy}}`, `{{.GPP}}`, `{{.GPPSID}}`, `{{.RedirectURL}}` — is deferred to user sync because they live on the sync path. Task 22b's codegen guard pins `endpoint_macros` to `["AccountID", "PublisherID"]`; any bidder YAML using a privacy macro on the endpoint (not the syncer URL) fails `cargo build -p auctioneer-config` with a clear "macro X is not in the auction core supported set; see plan constraint L466" message and a pointer to user sync. Sync URLs (which use the privacy macros) are unaffected because they go through `BIDDER_SYNCER_ALLOWED_MACROS` — a separate allow-list per foundations. The auction-path helper `fill_request_macros` fills `AccountID` from `info.account_id` and `PublisherID` from `info.publisher_id`; if `PublisherID` appears in a bidder YAML but the inbound request lacks both `site.publisher.id` and `app.publisher.id`, `EndpointBuilder::build()` returns `MissingMacro("PublisherID")` → code-5 `FailedToRequestBids`, which is the same loud-failure mode as a missing AccountID.
- **Clippy gate.** The workspace `pedantic = "warn" + restriction = "deny"` gate applies. Per-site `#[expect(...)]` is preferred over per-crate `#![allow(...)]` (mirrors foundations). **Round-20 review Spec 11**: every plan snippet in PRODUCTION code that uses `expect("...")`, an `as` cast (any kind), `unwrap()`, or `unwrap_or_default()` on a result that could carry meaning is either (a) tagged with a `#[expect(clippy::expect_used, reason = "...")]` / `#[expect(clippy::as_conversions, reason = "...")]` annotation at the call site, OR (b) rewritten to use `try_from` / typed defaults. When implementing a task, scan its snippet for these patterns BEFORE running `cargo clippy --workspace --all-targets -- -D warnings` — if a `cargo clippy` failure points at a snippet that lacks the expected annotation, that is a plan bug and the implementer should fix the snippet (and the plan) in the same commit. **Round-25 review Blocker 3 — test code is exempted via `clippy.toml`.** The workspace root carries a `clippy.toml` with `allow-unwrap-in-tests = true`, `allow-expect-in-tests = true`, `allow-panic-in-tests = true`, `allow-indexing-slicing-in-tests = true` (foundations source-of-truth). Clippy's "in tests" detection covers both `#[cfg(test)]` blocks AND `tests/*.rs` integration test files — so neither needs per-site `#[expect(clippy::unwrap_used, ...)]` annotations for the corresponding restriction lints. Test snippets MAY freely `.unwrap()` / `.expect()` / index / panic on fixture inputs without annotation. Integration tests in `tests/*.rs` still need a file-level `#![expect(clippy::tests_outside_test_module, reason = "integration tests live in tests/ by convention")]` because that lint is NOT covered by clippy.toml.
- **Reference spec sections (read once before starting):** §4 (Bidder trait + registry + 4 reference adapters), §5.1 (auction flow steps 1-12), §5.2 (fan-out + `BidderFanout` trait), §5.3 (resource limits + tmax), §5.4 (outbound SSRF / `validate_endpoint`), §5.5 (error model), §5.6 (debug — confirm suppressed in auction core), §5.8 (`run_auction`), §5.9 (alias identity contract + registry order), §6 (config codegen — confirm `endpoint_macros` exposure), §7.8 (sync privacy API — auction core ships stub only), §8.1 (auction / status / info endpoints), §11.1 (unit tests — exchange + adapters + SSRF + price/targeting/winner + tmax surfaces), §11.2 (integration — exchange engine, run_auction reuse, alias resolution, determinism, FanoutError mapping, Mocktioneer E2E, dependency purity, ingress hook contract, insecure-bidders snapshot, multi-target build), §14 auction core paragraph, Appendix E.1-E.7 (response shape), Appendix F (HTTP error wire bodies), Appendix G codes 2/4/5/6/10/12/14/15/999/10001/10002/10003/10004/10017/10999.

---

## Prerequisites

- [ ] **Confirm foundations is green at the head of the workspace.**

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 0 errors, 0 warnings. Crates present: `auctioneer-runtime`, `auctioneer-openrtb`, `auctioneer-config`, `auctioneer-core`, four adapter crates, `auctioneer-cli`.

- [ ] **Read the auction core spec sections** listed in Global Constraints. Cross-check Appendix E.7 deterministic-serialization rules against the file you'll write at Task 21 (response builder).

- [ ] **Confirm adapter wrapper conformance wrapper conformance has landed.** The two failing wrappers per §14 adapter wrapper conformance (`auctioneer-adapter-cloudflare`, `auctioneer-adapter-spin`) must already pass `cargo check --target ...` before auction core begins; auction core acceptance includes Cloudflare and Spin in the cross-target build gate. If adapter wrapper conformance was bundled into foundations (per the recent commit history with rounds 1-5 review fixes), no extra prep is needed — just verify each adapter's `cargo check` cleanly.

```
cargo check -p auctioneer-adapter-axum
cargo check -p auctioneer-adapter-cloudflare --target wasm32-unknown-unknown
cargo check -p auctioneer-adapter-fastly --target wasm32-wasip1
cargo check -p auctioneer-adapter-spin --target wasm32-wasip1
```

- [ ] **Confirm reference fixture sources are present.** Mocktioneer fixtures + 4 phase-1 bidder YAML files already shipped in foundations — verify with:

```
ls crates/auctioneer-config/data/bidder-info/
ls crates/auctioneer-config/data/bidder-info-insecure/
```

Expected: `appnexus.yaml`, `generic.yaml`, `pubmatic.yaml` under `bidder-info/`; `mocktioneer.yaml` under `bidder-info-insecure/`.

---

## File Structure

The plan adds two new library crates (`auctioneer-privacy`, `auctioneer-bidder`, `auctioneer-exchange`), modifies `auctioneer-config`, `auctioneer-core`, four adapter crates, and the root `Cargo.toml` + `edgezero.toml`. Files inside each crate are listed by responsibility.

### `crates/auctioneer-privacy/` (new)

- `Cargo.toml` — deps: `auctioneer-openrtb`, `auctioneer-config` (workspace), `auctioneer-runtime`. No EdgeZero. **Features:** `insecure-bidders = ["auctioneer-config/insecure-bidders"]` (forwards into config).
- `src/lib.rs` — re-exports `PrivacyContext`, `AuctionPermissions`, `Activity`, `Component`, `SyncPrivacyInput`, `SyncPrivacyError`, `SyncerLookup`, `SyncDecision`, `SyncPrivacyContext`, `GdprSignalIn`, `SyncEndpoint`, `Signal`, `resolve_sync_privacy`.
- `src/signal.rs` — `Signal` enum (`Yes`, `No`, `Ambiguous`). Placeholder shape; GDPR enforcement swaps in real `Signal` math.
- `src/activity.rs` — `Activity` enum (`FetchBids`, `SyncUser`, `EnrichUfpd`, `TransmitUniqueRequestIds`, `ReportAnalytics`); `Component<'a>` enum (`Bidder(&'a str)`, `Global`, `Module(&'a str)`).
- `src/auction_context.rs` — `PrivacyContext` struct + `allow_all_no_scrub()` constructor + accessors (`activity_allow`, `permissions_for(&str)`, `ccpa_should_enforce`, `apply_scrub`, `take_warnings`).
- `src/auction_permissions.rs` — `AuctionPermissions` enum (`AllowAll`, `DenyBidRequest`, `Partial { … }`).
- `src/sync.rs` — `SyncPrivacyInput`, `SyncPrivacyError`, `SyncerLookup`, `SyncDecision`, `SyncPrivacyContext`, `SyncEndpoint`, `GdprSignalIn`, `resolve_sync_privacy`. auction core stub.
- `tests/stub_invariants.rs` — invariants the auction core stub must guarantee (every activity allows, every bidder allowed, host_cookie_sync_allowed true, etc.).

### `crates/auctioneer-bidder/` (new)

- `Cargo.toml` — deps: `auctioneer-openrtb`, `auctioneer-config` (workspace). **Features:** `insecure-bidders = ["auctioneer-config/insecure-bidders"]`. Canonical declaration site per §5.4.
- `src/lib.rs` — re-exports.
- `src/bidder.rs` — `Bidder` trait (sync), `BidderError`, `EntryPoint` enum.
- `src/types.rs` — `BidderHttpRequest`, `BidderHttpResponse`, `BidderResponse`, `TypedBid`, `BidderName`.
- `src/request_info.rs` — `BidderRequestInfo` (the `#[non_exhaustive]` context per §4.1).
- `src/registry.rs` — `BidderRegistry`, registry assembly helper.
- `src/endpoint_builder.rs` — `EndpointBuilder` (macro percent-encode, path-traversal reject, `host_allowlist` lookup).
- `src/adapters/mod.rs` — re-exports the 3 (+1 feature-gated) reference adapters.
- `src/adapters/generic.rs` — `GenericBidder`.
- `src/adapters/appnexus.rs` — `AppnexusBidder`.
- `src/adapters/pubmatic.rs` — `PubmaticBidder`.
- `src/adapters/mocktioneer.rs` — `#[cfg(feature = "insecure-bidders")] MocktioneerBidder`.
- `tests/endpoint_builder.rs` — `EndpointBuilder` unit + integration tests.
- `tests/adapter_make_requests.rs` — fixture pairs per adapter (round-trip `make_requests` + `make_bids`).

### `crates/auctioneer-exchange/` (new)

- `Cargo.toml` — deps: `auctioneer-openrtb`, `auctioneer-config`, `auctioneer-runtime`, `auctioneer-privacy`, `auctioneer-bidder` (workspace); `async-trait`, `serde_json`. No `tokio`, no `edgezero-*`. **Features:** `insecure-bidders = ["auctioneer-bidder/insecure-bidders"]`.
- `src/lib.rs` — re-exports.
- `src/error.rs` — `AuctionError` enum (`BadInput`, `InvalidImpFirstPartyData`, `FanoutUnavailable`, `Internal`).
- `src/fanout.rs` — `BidderFanout` trait, `BidderCall`, `BidderCallResult`, `FanoutError`, `FanoutInternalError`, `AuctionDeadline`, `TmaxSource`, `Compression`.
- `src/account.rs` — `ResolvedAccount`, `AccountSource`.
- `src/context.rs` — `AuctionContext` (`with_request_start` constructor + direct field access for in-crate tests).
- `src/validate_endpoint.rs` — `validate_endpoint(&str, &BidderInfo) -> Result<(), BidderError>` (§5.4 full ruleset).
- `src/tmax.rs` — `resolve_tmax(req, host) -> (BidRequest.TMax_for_bidder_ms, AuctionDeadline)` (§5.3 four-value taxonomy).
- `src/registry_order.rs` — `participating_bidders(req, registry) -> Vec<BidderName>` (§5.9 lexicographic sort).
- `src/alias.rs` — alias resolution + `aliasgvlids` merge + alias-validation errors (§5.1 step 7).
- `src/split.rs` — per-bidder request split + `imp.ext.prebid.bidderparams` merge + bidder-scoped FPD merge (§5.1 step 7).
- `src/limits.rs` — `max_bidders_per_auction` + `max_outbound_requests` enforcement (§5.3 partial-bidder semantics).
- `src/run.rs` — `run_auction(&NormalizedBidRequest, &AuctionContext)` (§5.8).
- `src/targeting/mod.rs` — re-exports.
- `src/targeting/price_bucket.rs` — price-bucket computation (Appendix E.1, E.2).
- `src/targeting/keys.rs` — `hb_*` key emission + truncation (Appendix E.3).
- `src/targeting/winner.rs` — `is_new_winning_bid` (Appendix E.4).
- `src/response.rs` — `BidResponse` deterministic builder (Appendix E.5, E.6, E.7).
- `src/responsetime.rs` — per-bidder `responsetimemillis` aggregation (§5.2 `make_requests` + `max(call.elapsed)` + `make_bids`).
- `tests/fanout_mock.rs` — mock `BidderFanout` for tests.
- `tests/run_auction.rs` — driver test: invoke `run_auction` directly with a `NormalizedBidRequest`, mock fanout, `FakeClock`, seeded `Rng`.
- `tests/determinism.rs` — snapshot-file E2E test for byte-identical `BidResponse`.
- `tests/validate_endpoint.rs` — full §5.4 ruleset.
- `tests/tmax.rs` — every aspect of §5.3 tmax block.

### `crates/auctioneer-core/` (modified)

- `Cargo.toml` — add deps: `auctioneer-exchange`, `auctioneer-bidder`, `auctioneer-privacy`, `auctioneer-openrtb`, `auctioneer-config`, `auctioneer-runtime`, `serde_json`, `http`, `futures`, `flate2`. Forward `insecure-bidders` via `insecure-bidders = ["auctioneer-bidder/insecure-bidders", "auctioneer-exchange/insecure-bidders"]`.
- `src/lib.rs` — `edgezero_core::app!` plus the handler registration.
- `src/handlers/mod.rs` — re-exports the four handlers.
- `src/handlers/auction.rs` — `auction(ctx) -> Response`.
- `src/handlers/status.rs` — `status(ctx) -> Response`.
- `src/handlers/info_bidders.rs` — `info_bidders(ctx)`, `info_bidders_detail(ctx)`.
- `src/host_primitives/mod.rs` — re-exports.
- `src/host_primitives/clock.rs` — `SystemClock` (dev `Clock` per §14 prereq 2: wall via `SystemTime::now`, monotonic via `Instant::now`).
- `src/host_primitives/rng.rs` — dev `Rng` per §14 prereq 4 (xorshift64* seeded from `SystemTime::now`).
- `src/host_primitives/fanout.rs` — dev `BidderFanout` impl backed by `ProxyClient` + `join_all` (interim per §5.2 with documented gaps).
- `src/serialize.rs` — `BidResponse` → `application/json` body bytes (Appendix E.7 deterministic). Reused by all four handlers.
- `tests/handler_auction.rs` — Axum integration: real `/openrtb2/auction` requests + `BidResponse` assertions.
- `tests/handler_status.rs` — Axum integration: `/status` default 204 + configured 200.
- `tests/handler_info_bidders.rs` — Axum integration: `/info/bidders`, `/info/bidders/{name}`, `/info/bidders/all`, 400 bad-query, 404 unknown.
- `tests/dependency_purity.rs` — `cargo metadata`-driven deny-list test asserting library-crate dependency purity (§11.2).

### `crates/auctioneer-adapter-{axum,cloudflare,fastly,spin}/` (modified)

- Each `Cargo.toml` adds `insecure-bidders = ["auctioneer-core/insecure-bidders"]`.

### `edgezero.toml` (modified)

- Add `[[triggers.http]]` entries: `POST /openrtb2/auction`, `GET /status`, `GET /info/bidders`, `GET /info/bidders/{name}` — four auction-shaped routes per §14 auction core.

### Root `Cargo.toml` (modified)

- Add new workspace members: `auctioneer-privacy`, `auctioneer-bidder`, `auctioneer-exchange`.
- Add new workspace dependencies: `async-trait = "0.1"`, `futures = { version = "0.3", default-features = false }`, `flate2 = "1"`, `percent-encoding = "2"`.

---

## Tasks

### Task 1: Workspace plumbing — new crates + workspace deps

**Files:**

- Modify: `Cargo.toml`

**Interfaces:**

- Consumes: the foundational existing workspace member list.
- Produces: Three new workspace members declared (`auctioneer-privacy`, `auctioneer-bidder`, `auctioneer-exchange`) and four new workspace dependencies (`async-trait`, `futures`, `flate2`, `percent-encoding`).

- [ ] **Step 1: Edit `Cargo.toml`**

Replace the `[workspace] members = [...]` block with:

```toml
[workspace]
members = [
  "crates/auctioneer-core",
  "crates/auctioneer-adapter-axum",
  "crates/auctioneer-adapter-cloudflare",
  "crates/auctioneer-adapter-fastly",
  "crates/auctioneer-adapter-spin",
  "crates/auctioneer-bidder",
  "crates/auctioneer-cli",
  "crates/auctioneer-config",
  "crates/auctioneer-exchange",
  "crates/auctioneer-openrtb",
  "crates/auctioneer-privacy",
  "crates/auctioneer-runtime",
]
resolver = "2"
```

Add to `[workspace.dependencies]` (alphabetical-ish, keep existing entries):

```toml
async-trait = "0.1"
flate2 = { version = "1", default-features = false, features = ["rust_backend"] }
futures = { version = "0.3", default-features = false, features = ["std", "executor"] }  # round-24 review High 2: `executor` enables `futures::executor::block_on` used by determinism + ingress-hook tests (Task 35 / 41). The default-features set already pulls `std`; `executor` adds the single-threaded LocalPool used in synchronous test contexts. No async runtime is added — `block_on` here is test-only.
percent-encoding = "2"
url = "2"   # round-22 Blocker 2: needed by auctioneer-exchange::validate_endpoint (Task 22).
auctioneer-bidder = { path = "crates/auctioneer-bidder" }
auctioneer-config = { path = "crates/auctioneer-config" }
auctioneer-exchange = { path = "crates/auctioneer-exchange" }
auctioneer-openrtb = { path = "crates/auctioneer-openrtb" }
auctioneer-privacy = { path = "crates/auctioneer-privacy" }
auctioneer-runtime = { path = "crates/auctioneer-runtime" }
```

(`time` already lives in the foundational workspace deps — `auctioneer-openrtb` pulls it for `OffsetDateTime` per the foundations source; the inheritance covers every downstream crate that needs it.)

- [ ] **Step 2: Create empty `Cargo.toml` placeholders for the three new crates** so `cargo metadata` resolves the workspace member list. Round-16 review Medium 9: `cargo metadata` walks the `members` list and parses each manifest; it errors out on missing manifests, NOT just on `cargo build`. Land minimal stubs here so Step 3's verification can run.

```bash
mkdir -p crates/auctioneer-privacy/src crates/auctioneer-bidder/src crates/auctioneer-exchange/src
for crate in auctioneer-privacy auctioneer-bidder auctioneer-exchange; do
  cat > "crates/$crate/Cargo.toml" <<EOF
[package]
name = "$crate"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
path = "src/lib.rs"
EOF
  : > "crates/$crate/src/lib.rs"
done
```

Tasks 2–5 replace these stubs with the full crate content; this step exists ONLY so the workspace resolves in Step 3.

- [ ] **Step 3: Verify the workspace parses**

```
cargo metadata --no-deps --format-version 1 > /dev/null
```

Expected: success. (With the placeholders from Step 2 in place, `cargo metadata` resolves all twelve members.)

- [ ] **Step 4: Commit**

```
git add Cargo.toml crates/auctioneer-privacy crates/auctioneer-bidder crates/auctioneer-exchange
git commit -m "auction core: workspace plumbing for new crates + deps (with placeholder Cargo.tomls)"
```

---

### Task 2: `auctioneer-privacy` crate skeleton + module stubs

**Files:**

- Create: `crates/auctioneer-privacy/Cargo.toml`
- Create: `crates/auctioneer-privacy/src/lib.rs`
- Create: `crates/auctioneer-privacy/src/{signal,activity,auction_context,auction_permissions,sync}.rs`

**Interfaces:**

- Consumes: `auctioneer-config::{HostConfig, AccountConfig, BidderInfo}` (already shipped in foundations).
- Produces: Empty module file structure; `cargo check -p auctioneer-privacy` succeeds.

- [ ] **Step 1: Create `crates/auctioneer-privacy/Cargo.toml`**

```toml
[package]
name = "auctioneer-privacy"
version = "0.1.0"
edition = "2021"
publish = false

[lints]
workspace = true

[features]
insecure-bidders = ["auctioneer-config/insecure-bidders"]

[dependencies]
auctioneer-config = { workspace = true }
auctioneer-openrtb = { workspace = true }
auctioneer-runtime = { workspace = true }
```

- [ ] **Step 2: Create `src/lib.rs`**

```rust
//! Privacy & consent API surface. **auction core scope: stub only** — types and
//! function signatures land here in auction core so `auctioneer-exchange` can compile
//! alongside them; the auction-path `PrivacyContext` returns from a
//! permissive constructor (`allow_all_no_scrub`) and `resolve_sync_privacy`
//! returns a context where every gate is `Allowed`. GDPR enforcement / privacy enforcement / user sync swap in
//! the real GDPR / CCPA-GPP-activities / sync impls without changing the
//! API surface. See spec §7 / §7.8 / §14.

mod activity;
mod auction_context;
mod auction_permissions;
mod signal;
mod sync;

pub use activity::{Activity, Component};
pub use auction_context::PrivacyContext;
pub use auction_permissions::AuctionPermissions;
pub use signal::Signal;
pub use sync::{
    GdprSignalIn, SyncDecision, SyncEndpoint, SyncPrivacyContext, SyncPrivacyError,
    SyncPrivacyInput, SyncerLookup, resolve_sync_privacy,
};
```

- [ ] **Step 3: Create each module as a stub**

For each of `signal.rs`, `activity.rs`, `auction_context.rs`, `auction_permissions.rs`, `sync.rs` — create the file with a single line `// stub — implemented in next task` so subsequent tasks can replace contents one at a time without breaking the lib.

- [ ] **Step 4: Verify**

```
cargo check -p auctioneer-privacy
```

Expected: fails because the modules re-export types that don't exist. Empty `lib.rs` of just `// stub` doesn't compile against the `pub use` lines. **Fix:** replace `src/lib.rs` with the empty form `// stub — populated in subsequent tasks` and put the canonical `pub use` block in at Task 7 (after every sub-module has its types).

Edit `src/lib.rs` to:

```rust
//! Privacy & consent API surface (auction core stub).
//! Full module wiring lands in Task 7 once all types exist.
```

Then:

```
cargo check -p auctioneer-privacy
```

Expected: success, zero warnings.

- [ ] **Step 5: Commit**

```
git add Cargo.toml crates/auctioneer-privacy
git commit -m "auctioneer-privacy: crate skeleton (auction core)"
```

---

### Task 3: `Signal` enum

**Files:**

- Modify: `crates/auctioneer-privacy/src/signal.rs`

**Interfaces:**

- Consumes: nothing.
- Produces: `pub enum Signal { Yes, No, Ambiguous }` + helpers.

- [ ] **Step 1: Write the failing test + impl**

Replace `src/signal.rs`:

```rust
//! GDPR `gdpr_applies` signal. See spec §7.1.

/// GDPR `gdpr_applies` signal per §7.1. `Yes(1)` / `No(0)` / `Ambiguous(-1)`.
/// auction core ships the enum shape only; signal-derivation lives in GDPR enforcement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    Yes,
    No,
    Ambiguous,
}

#[cfg(test)]
mod tests {
    use super::Signal;

    #[test]
    fn signal_is_copy_and_eq() {
        let s = Signal::Yes;
        let t = s;
        assert_eq!(s, t);
        assert_ne!(Signal::Yes, Signal::No);
        assert_ne!(Signal::Yes, Signal::Ambiguous);
    }
}
```

- [ ] **Step 2: Run test**

```
cargo test -p auctioneer-privacy signal::tests
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-privacy/src/signal.rs
git commit -m "auctioneer-privacy: Signal enum"
```

---

### Task 4: `Activity` + `Component` enums

**Files:**

- Modify: `crates/auctioneer-privacy/src/activity.rs`

**Interfaces:**

- Consumes: nothing.
- Produces: `pub enum Activity { FetchBids, SyncUser, EnrichUfpd, TransmitUniqueRequestIds, ReportAnalytics }`; `pub enum Component<'a> { Bidder(&'a str), Global, Module(&'a str) }`.

- [ ] **Step 1: Write the test + impl**

Replace `src/activity.rs`:

```rust
//! Privacy-activity model — spec §7.6.

/// The five phase-1 activities. `EnrichUfpd` and `TransmitUniqueRequestIds`
/// are inert per spec §12 open item 11 (deferred to the analytics phase);
/// they exist so callers can pass them without conditional compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Activity {
    FetchBids,
    SyncUser,
    EnrichUfpd,
    TransmitUniqueRequestIds,
    ReportAnalytics,
}

/// A target the activity rule applies to. Lifetime-borrowed so callers can
/// hand-tag bidder names from request-scoped slices without allocating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component<'a> {
    Bidder(&'a str),
    Global,
    Module(&'a str),
}

#[cfg(test)]
mod tests {
    use super::{Activity, Component};

    #[test]
    fn activity_and_component_equality() {
        assert_eq!(Activity::FetchBids, Activity::FetchBids);
        assert_eq!(Component::Bidder("ax"), Component::Bidder("ax"));
        assert_ne!(Component::Bidder("ax"), Component::Bidder("pubmatic"));
        assert_ne!(Component::Global, Component::Bidder("ax"));
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-privacy activity::tests
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-privacy/src/activity.rs
git commit -m "auctioneer-privacy: Activity + Component"
```

---

### Task 5: `AuctionPermissions` enum

**Files:**

- Modify: `crates/auctioneer-privacy/src/auction_permissions.rs`

**Interfaces:**

- Consumes: nothing.
- Produces: `pub enum AuctionPermissions { AllowAll, DenyBidRequest, Partial { … } }`.

- [ ] **Step 1: Write the test + impl**

Replace `src/auction_permissions.rs`:

```rust
//! Per-bidder permissions outcome. See spec §7.3 / Appendix A.4.

/// Per-bidder permission summary the auction core stub returns `AllowAll`; GDPR enforcement swaps in
/// a structured `Partial { … }` carrying purpose/feature decisions per
/// Appendix A.4. Phase-1 callers consume `AllowAll` / `DenyBidRequest`
/// directly; `Partial` flows through to the bidder-scoped scrub matrix
/// (§7.6 / Appendix C).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuctionPermissions {
    /// `AllowBidRequest && PassID && PassGeo && every special feature opted-in`.
    AllowAll,
    /// `AllowBidRequest == false` — bidder is dropped with a non-fatal warning
    /// (Appendix G code 10017 `BidderBlockedByPrivacySettings`).
    DenyBidRequest,
    /// GDPR enforcement — placeholder for the structured Appendix A.4 mapping. Tagged so
    /// adding fields later is non-breaking.
    Partial {
        allow_bid_request: bool,
        pass_id: bool,
        pass_geo: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::AuctionPermissions;

    #[test]
    fn allow_all_is_distinct_from_deny() {
        assert_ne!(AuctionPermissions::AllowAll, AuctionPermissions::DenyBidRequest);
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-privacy auction_permissions::tests
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-privacy/src/auction_permissions.rs
git commit -m "auctioneer-privacy: AuctionPermissions enum"
```

---

### Task 6: `PrivacyContext::allow_all_no_scrub()` stub

**Files:**

- Modify: `crates/auctioneer-privacy/src/auction_context.rs`

**Interfaces:**

- Consumes: `AuctionPermissions`, `Activity`, `Component`.
- Produces: `pub struct PrivacyContext { … }`; `pub fn allow_all_no_scrub() -> Self`; methods `activity_allow`, `permissions_for`, `ccpa_should_enforce`, `apply_scrub`, `take_warnings`.

- [ ] **Step 1: Write the failing test + impl**

Replace `src/auction_context.rs`:

```rust
//! Auction-path privacy context. See spec §7.6 + §11.1.
//!
//! **auction core stub.** `allow_all_no_scrub()` returns a context where:
//! - every activity allows,
//! - every bidder's `AuctionPermissions` is `AllowAll`,
//! - `ccpa_should_enforce == false`,
//! - `apply_scrub` is a no-op,
//! - `take_warnings()` returns empty.
//!
//! GDPR enforcement (GDPR) and privacy enforcement (CCPA / GPP / activities / scrubbing) progressively
//! replace this stub without changing the type's public surface.

use crate::{activity::{Activity, Component}, auction_permissions::AuctionPermissions};
use auctioneer_openrtb::types::bid_request::BidRequest;

/// Auction-path privacy decisions. See spec §7.
#[derive(Debug, Default)]
pub struct PrivacyContext {
    // Crate-private: GDPR enforcement/privacy enforcement add fields here without breaking callers.
    // auction core has no fields — the stub is permissive across the board.
}

impl PrivacyContext {
    /// auction core stub: permissive across the board. Spec §7.6 / §11.1.
    #[must_use]
    pub fn allow_all_no_scrub() -> Self {
        Self {}
    }

    /// Stub: returns `true` for every (activity, component) pair.
    #[must_use]
    pub fn activity_allow(&self, _activity: Activity, _component: Component<'_>) -> bool {
        true
    }

    /// Stub: returns `AllowAll` for every bidder.
    #[must_use]
    pub fn permissions_for(&self, _bidder: &str) -> AuctionPermissions {
        AuctionPermissions::AllowAll
    }

    /// Stub: always `false`.
    #[must_use]
    pub fn ccpa_should_enforce(&self) -> bool {
        false
    }

    /// Stub: no-op. Real impl (privacy enforcement) returns a scrubbed `BidRequest`.
    #[must_use]
    pub fn apply_scrub(&self, req: &BidRequest, _bidder: &str) -> BidRequest {
        req.clone()
    }

    /// Stub: returns empty. Real impl (GDPR enforcement) drains a private buffer of
    /// `NonFatalWarning`s. Called by `run_auction` once per auction.
    #[must_use]
    pub fn take_warnings(&mut self) -> Vec<auctioneer_openrtb::NonFatalWarning> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_allows_every_activity() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        assert!(ctx.activity_allow(Activity::FetchBids, Component::Bidder("ax")));
        assert!(ctx.activity_allow(Activity::SyncUser, Component::Global));
        assert!(ctx.activity_allow(Activity::ReportAnalytics, Component::Module("analytics")));
    }

    #[test]
    fn stub_allows_every_bidder() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        for bidder in &["appnexus", "pubmatic", "generic", "mocktioneer", "unknown_future"] {
            assert_eq!(ctx.permissions_for(bidder), AuctionPermissions::AllowAll);
        }
    }

    #[test]
    fn stub_ccpa_does_not_enforce() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        assert!(!ctx.ccpa_should_enforce());
    }

    #[test]
    fn stub_scrub_is_noop() {
        let ctx = PrivacyContext::allow_all_no_scrub();
        let req = BidRequest::default();
        let scrubbed = ctx.apply_scrub(&req, "ax");
        // The scrub is a no-op stub; we assert structural equality via a debug
        // round-trip rather than a Trait bound that BidRequest doesn't carry.
        assert_eq!(format!("{:?}", req), format!("{:?}", scrubbed));
    }

    #[test]
    fn stub_take_warnings_empty() {
        let mut ctx = PrivacyContext::allow_all_no_scrub();
        assert!(ctx.take_warnings().is_empty());
        assert!(ctx.take_warnings().is_empty());  // drain idempotent
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-privacy auction_context::tests
```

Expected: 5 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-privacy/src/auction_context.rs
git commit -m "auctioneer-privacy: PrivacyContext::allow_all_no_scrub stub"
```

---

### Task 7: Sync API surface — `SyncPrivacyInput`, `SyncPrivacyError`, `SyncerLookup`, `SyncDecision`, `SyncEndpoint`, `GdprSignalIn`

**Files:**

- Modify: `crates/auctioneer-privacy/src/sync.rs`
- Modify: `crates/auctioneer-privacy/src/lib.rs`

**Interfaces:**

- Consumes: `Signal`, `Activity`, `Component`, `auctioneer-config::{BidderInfo, HostConfig, AccountConfig}`.
- Produces: types listed above + the `resolve_sync_privacy(input) -> Result<SyncPrivacyContext, SyncPrivacyError>` permissive stub.

- [ ] **Step 1: Replace `src/sync.rs`**

```rust
//! Sync-endpoint privacy API. See spec §7.8.
//!
//! **auction core ships the API surface only.** `resolve_sync_privacy` returns a
//! `SyncPrivacyContext` where every gate is permissive: both per-syncer
//! callbacks return `Allowed`, `host_cookie_sync_allowed == true`,
//! `activity_allow` permits every (activity, component). user sync replaces
//! this stub with the real impl (TCF v2 / US-Privacy / GPP parsing,
//! GDPR vendor decisions, SkipWhen.gpp_sid matching, BidderSyncAllowed,
//! `setuid_allowed`) — the API surface does NOT change.

use crate::{activity::{Activity, Component}, signal::Signal};
use auctioneer_config::{AccountConfig, BidderInfo, HostConfig};

/// Which sync endpoint is calling. Affects three policy decisions in user sync
/// (see §7.8 + Appendix F); auction core's permissive stub ignores it but still
/// requires the discriminator for forward-compat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncEndpoint {
    CookieSync,
    SetUid,
}

/// Wire-coerced `gdpr` value. Recognized → `Zero` / `One`; absent →
/// `Absent`; unrecognized → `Invalid(<raw bytes>)`. user sync decides the 400
/// based on GPP/SID precedence; auction core ignores the value entirely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GdprSignalIn {
    Absent,
    Zero,
    One,
    Invalid(String),
}

/// `SyncerLookup` identifies a syncer slot to the per-syncer decision
/// callback. The caller resolves bidder-name → BidderInfo + syncer
/// config from the build-time registry (§6).
pub struct SyncerLookup<'a> {
    pub bidder:     &'a BidderInfo,
    pub syncer_key: &'a str,
    /// auction core ships the API shape only; the syncer config struct ships in
    /// user sync. The lookup carries an opaque `&str` placeholder for now so
    /// `auctioneer-privacy` does not need to depend on the user sync `SyncerConfig`
    /// type.
    pub syncer_config_marker: &'a str,
}

/// Per-syncer decision result. `BlockedByRegulationScope` is
/// `/cookie_sync`-only (Appendix B.3 step 9 / round-52 H3); `setuid_allowed`
/// never returns it. auction core's stub returns `Allowed` for every call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncDecision {
    Allowed,
    BlockedByPrivacy,
    BlockedByDisabledUsersync,
    BlockedByRegulationScope,
}

pub struct SyncPrivacyInput<'a> {
    pub gdpr:        GdprSignalIn,
    pub consent:     Option<&'a str>,
    pub us_privacy:  Option<&'a str>,
    pub gpp:         Option<&'a str>,
    pub gpp_sid:     Option<&'a str>,
    pub account:     &'a AccountConfig,
    pub host:        &'a HostConfig,
    pub endpoint:    SyncEndpoint,
    /// user sync wires the GVL here. auction core takes `()` as a zero-sized placeholder so
    /// `auctioneer-privacy` does not depend on GDPR enforcement's `Gvl` type.
    pub gvl_marker:  (),
}

#[derive(Debug, PartialEq, Eq)]
pub enum SyncPrivacyError {
    /// `scope` ∈ {"gdpr", "gdpr_consent", "gdpr_consent_empty"}. user sync
    /// emits per-field; auction core never emits.
    MalformedConsent { scope: &'static str },
    MalformedGpp,
    MalformedGppSid,
    BadInput(String),
}

/// Boxed per-syncer callback. user sync's real impl runs the chooser ladder
/// (Appendix B.3 steps 6-9) for /cookie_sync or the narrower host-cookie
/// path for /setuid; auction core returns `Allowed` for everything.
pub type SyncCallback<'a> =
    Box<dyn for<'l> Fn(&SyncerLookup<'l>) -> SyncDecision + 'a>;

/// Per-component activity callback. HRTB so callers can pass
/// request-borrowed bidder-name slices.
pub type ActivityCallback<'a> =
    Box<dyn for<'c> Fn(Activity, Component<'c>) -> bool + 'a>;

pub struct SyncPrivacyContext<'a> {
    pub parsed_request_signal:     Signal,
    pub normalized_request_signal: Signal,
    pub gdpr_permissions_enabled:  bool,

    // Back-compat aliases (§7.8.1 / round-51 auction core). Identical fields, kept
    // for callers written before the taxonomy was named.
    pub gdpr_signal:               Signal,
    pub gdpr_enforced:             bool,

    pub ccpa_enforce:              bool,
    pub raw_gpp_sid:               Option<&'a str>,
    pub host_cookie_sync_allowed:  bool,

    pub activity_allow:  ActivityCallback<'a>,

    /// /cookie_sync chooser ladder. user sync implements; auction core returns `Allowed`.
    pub sync_allowed:    SyncCallback<'a>,
    /// /setuid narrower decision. user sync implements; auction core returns `Allowed`.
    pub setuid_allowed:  SyncCallback<'a>,
}

/// auction core permissive stub. Never returns an `Err`. user sync swaps in the real impl.
pub fn resolve_sync_privacy(input: SyncPrivacyInput<'_>)
    -> Result<SyncPrivacyContext<'_>, SyncPrivacyError>
{
    let _ = input;
    Ok(SyncPrivacyContext {
        parsed_request_signal:     Signal::Ambiguous,
        normalized_request_signal: Signal::Ambiguous,
        gdpr_permissions_enabled:  false,
        gdpr_signal:               Signal::Ambiguous,
        gdpr_enforced:             false,
        ccpa_enforce:              false,
        raw_gpp_sid:               None,
        host_cookie_sync_allowed:  true,
        activity_allow: Box::new(|_a, _c| true),
        sync_allowed:   Box::new(|_l| SyncDecision::Allowed),
        setuid_allowed: Box::new(|_l| SyncDecision::Allowed),
    })
}
```

- [ ] **Step 2: Wire `lib.rs`**

Replace `src/lib.rs`:

```rust
//! Privacy & consent API surface. **auction core scope: stub only.** See spec §7 / §7.8 / §14.

mod activity;
mod auction_context;
mod auction_permissions;
mod signal;
mod sync;

pub use activity::{Activity, Component};
pub use auction_context::PrivacyContext;
pub use auction_permissions::AuctionPermissions;
pub use auctioneer_openrtb::NonFatalWarning;  // re-exported for convenience
pub use signal::Signal;
pub use sync::{
    ActivityCallback, GdprSignalIn, SyncCallback, SyncDecision, SyncEndpoint,
    SyncPrivacyContext, SyncPrivacyError, SyncPrivacyInput, SyncerLookup,
    resolve_sync_privacy,
};
```

- [ ] **Step 3: Compile**

```
cargo check -p auctioneer-privacy
```

Expected: success, zero warnings.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-privacy/src
git commit -m "auctioneer-privacy: sync API surface + permissive stub"
```

---

### Task 8: `auctioneer-privacy` stub invariants test

**Files:**

- Create: `crates/auctioneer-privacy/tests/stub_invariants.rs`

**Interfaces:**

- Consumes: every public type in `auctioneer-privacy`.
- Produces: end-to-end test asserting the auction core stub guarantees the invariants the spec relies on (so GDPR enforcement/privacy enforcement cannot accidentally weaken them while auction core is the active code path).

- [ ] **Step 1: Create the test**

```rust
//! Spec §11.1 — `PrivacyContext::allow_all_no_scrub()` + `resolve_sync_privacy`
//! invariants. These are the contract the auction core stub MUST satisfy so the
//! auction core can rely on permissive behavior; GDPR enforcement/privacy enforcement/user sync may not narrow
//! the stub without simultaneously narrowing the consumers.
//
// Round-25 review Blocker 3: `clippy.toml` already exempts test code
// from `unwrap_used` / `expect_used` / `panic_in_tests` /
// `indexing_slicing_in_tests`; only the integration-test layout lint
// needs an explicit allow. See constraint L466 for the full
// convention.
#![expect(
    clippy::tests_outside_test_module,
    reason = "integration tests live in tests/ by convention with no #[cfg(test)] wrapper"
)]

use auctioneer_privacy::{
    Activity, AuctionPermissions, Component, GdprSignalIn, PrivacyContext, SyncDecision,
    SyncEndpoint, SyncPrivacyInput, SyncerLookup, resolve_sync_privacy,
};
use auctioneer_config::{AccountConfig, BidderInfo, HostConfig};

#[test]
fn auction_stub_allows_every_known_activity_for_every_known_component() {
    let ctx = PrivacyContext::allow_all_no_scrub();
    for activity in [
        Activity::FetchBids,
        Activity::SyncUser,
        Activity::EnrichUfpd,
        Activity::TransmitUniqueRequestIds,
        Activity::ReportAnalytics,
    ] {
        for component in [
            Component::Global,
            Component::Bidder("appnexus"),
            Component::Bidder("pubmatic"),
            Component::Bidder("generic"),
            Component::Bidder("mocktioneer"),
            Component::Module("analytics"),
        ] {
            assert!(
                ctx.activity_allow(activity, component),
                "stub denied {:?} for {:?}",
                activity, component
            );
        }
    }
}

#[test]
fn auction_stub_returns_allow_all_for_every_bidder() {
    let ctx = PrivacyContext::allow_all_no_scrub();
    for bidder in ["appnexus", "pubmatic", "generic", "mocktioneer", "unknown"] {
        assert_eq!(ctx.permissions_for(bidder), AuctionPermissions::AllowAll);
    }
}

#[test]
fn auction_stub_never_enforces_ccpa() {
    let ctx = PrivacyContext::allow_all_no_scrub();
    assert!(!ctx.ccpa_should_enforce());
}

#[test]
fn auction_stub_take_warnings_is_idempotent_empty() {
    let mut ctx = PrivacyContext::allow_all_no_scrub();
    assert!(ctx.take_warnings().is_empty());
    assert!(ctx.take_warnings().is_empty());
    assert!(ctx.take_warnings().is_empty());
}

#[test]
fn sync_stub_returns_ok_with_permissive_context() {
    let host = HostConfig::default();
    let account = AccountConfig::default();
    let input = SyncPrivacyInput {
        gdpr: GdprSignalIn::Absent,
        consent: None,
        us_privacy: None,
        gpp: None,
        gpp_sid: None,
        account: &account,
        host: &host,
        endpoint: SyncEndpoint::CookieSync,
        gvl_marker: (),
    };
    let ctx = resolve_sync_privacy(input).expect("stub never errors");
    assert!(ctx.host_cookie_sync_allowed);
    assert!((ctx.activity_allow)(Activity::SyncUser, Component::Bidder("ax")));

    let info = BidderInfo {
        name: "ax".into(),
        ..BidderInfo::default()
    };
    let lookup = SyncerLookup {
        bidder: &info,
        syncer_key: "ax",
        syncer_config_marker: "",
    };
    assert_eq!((ctx.sync_allowed)(&lookup), SyncDecision::Allowed);
    assert_eq!((ctx.setuid_allowed)(&lookup), SyncDecision::Allowed);
}

#[test]
fn sync_stub_returns_ok_for_setuid_endpoint_too() {
    let host = HostConfig::default();
    let account = AccountConfig::default();
    let input = SyncPrivacyInput {
        gdpr: GdprSignalIn::One,
        consent: Some(""),
        us_privacy: None,
        gpp: None,
        gpp_sid: None,
        account: &account,
        host: &host,
        endpoint: SyncEndpoint::SetUid,
        gvl_marker: (),
    };
    let _ctx = resolve_sync_privacy(input).expect("stub never errors");
}
```

- [ ] **Step 2: Run**

```
cargo test -p auctioneer-privacy --test stub_invariants
```

Expected: 6 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-privacy/tests/stub_invariants.rs
git commit -m "auctioneer-privacy: stub invariants integration test"
```

---

### Task 9: `auctioneer-bidder` crate skeleton + canonical `insecure-bidders` feature

**Files:**

- Create: `crates/auctioneer-bidder/Cargo.toml`
- Create: `crates/auctioneer-bidder/src/lib.rs`
- Create: `crates/auctioneer-bidder/src/{bidder,types,request_info,registry,endpoint_builder,adapters/mod}.rs`

**Interfaces:**

- Consumes: `auctioneer-openrtb` types, `auctioneer-config::BidderInfo`.
- Produces: Empty module shells; `cargo check -p auctioneer-bidder` succeeds.

- [ ] **Step 1: Create `crates/auctioneer-bidder/Cargo.toml`**

```toml
[package]
name = "auctioneer-bidder"
version = "0.1.0"
edition = "2021"
publish = false

[lints]
workspace = true

[features]
# Canonical home of the insecure-bidders feature (spec §5.4 + round-75 foundations).
# Forwards into auctioneer-config so build.rs's syncer-URL template-validation
# pass also relaxes its scheme rule under the feature.
insecure-bidders = ["auctioneer-config/insecure-bidders"]

[dependencies]
auctioneer-config  = { workspace = true }
auctioneer-openrtb = { workspace = true }
percent-encoding = { workspace = true }
serde       = { workspace = true }
serde_json  = { workspace = true }
```

- [ ] **Step 2: Create `src/lib.rs`**

```rust
//! Adapter framework. See spec §4.

mod adapters;
mod bidder;
mod endpoint_builder;
mod registry;
mod request_info;
mod types;

pub use adapters::{AppnexusBidder, GenericBidder, PubmaticBidder};
#[cfg(feature = "insecure-bidders")]
pub use adapters::MocktioneerBidder;
pub use bidder::{Bidder, BidderError, EntryPoint};
pub use endpoint_builder::{EndpointBuilder, EndpointBuilderError};
pub use registry::BidderRegistry;
pub use request_info::BidderRequestInfo;
pub use types::{BidderHttpRequest, BidderHttpResponse, BidderName, BidderResponse, TypedBid};
```

- [ ] **Step 3: Create each module as a single-line stub**

For `bidder.rs`, `types.rs`, `request_info.rs`, `registry.rs`, `endpoint_builder.rs`, and `adapters/mod.rs` — single-line stub `// stub — implemented in next task`. The `lib.rs` re-exports above will fail to compile until each module exists, so for now temporarily reduce `lib.rs` to:

```rust
//! Adapter framework. See spec §4 (auction core skeleton).
```

We add re-exports incrementally as each module's types land.

- [ ] **Step 4: Verify**

```
cargo check -p auctioneer-bidder
```

Expected: success, zero warnings.

- [ ] **Step 5: Commit**

```
git add Cargo.toml crates/auctioneer-bidder
git commit -m "auctioneer-bidder: crate skeleton + canonical insecure-bidders feature"
```

---

### Task 10: `BidderName` + transport-neutral types

**Files:**

- Modify: `crates/auctioneer-bidder/src/types.rs`

**Interfaces:**

- Consumes: nothing (uses std + serde).
- Produces: `pub struct BidderName(Box<str>)`, `pub struct BidderHttpRequest { method, url, headers: Vec<(String, String)>, body: Vec<u8>, covered_imp_ids: Vec<String> }`, `pub struct BidderHttpResponse { status: u16, headers: Vec<(String, String)>, body: Vec<u8> }`, `pub struct BidderResponse { bids: Vec<TypedBid>, currency: String }`, `pub struct TypedBid { bid: auctioneer_openrtb::types::bid_response::Bid, media_type: MediaType }`.

- [ ] **Step 1: Write tests + impl**

Replace `src/types.rs`:

```rust
//! Transport-neutral types passed across the `Bidder` trait. See spec §4.1.

use auctioneer_config::MediaType;
use auctioneer_openrtb::types::bid_response::Bid;
use std::fmt;

/// Interned-style alias-aware bidder identifier. Round-trips through serde
/// as a plain string so it can appear in `BidderInfo.name` and on the wire.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BidderName(Box<str>);

impl BidderName {
    #[must_use]
    pub fn new(s: impl Into<Box<str>>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BidderName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for BidderName {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Outbound HTTP request shape an adapter's `make_requests` produces.
/// **No transport policy** (timeout, deadline, compression) — those live
/// on `BidderCall` in `auctioneer-exchange` per spec §5.2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidderHttpRequest {
    pub method: String,
    pub url: String,
    /// Preserves order + allows duplicates (e.g. multiple `Set-Cookie`).
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Which `imp.id`s this outbound call covers, for `seatnonbid` mapping
    /// (Appendix E.6). Empty when the adapter does not pre-correlate.
    pub covered_imp_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidderHttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// What an adapter's `make_bids` returns. Currency is the seatbid currency
/// the adapter saw; MVP enforces USD in the exchange (§5.1 step 10).
#[derive(Clone, Debug, PartialEq)]
pub struct BidderResponse {
    pub bids: Vec<TypedBid>,
    pub currency: String,
}

/// A bid plus its inferred media type. Media-type inference is per-adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct TypedBid {
    pub bid: Bid,
    pub media_type: MediaType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidder_name_is_ord_for_registry_ordering() {
        let mut names = vec![
            BidderName::new("pubmatic"),
            BidderName::new("appnexus"),
            BidderName::new("generic"),
        ];
        names.sort();
        assert_eq!(names[0].as_str(), "appnexus");
        assert_eq!(names[1].as_str(), "generic");
        assert_eq!(names[2].as_str(), "pubmatic");
    }

    #[test]
    fn bidder_http_request_clone_preserves_headers_order() {
        let req = BidderHttpRequest {
            method: "POST".into(),
            url: "https://example.test/openrtb".into(),
            headers: vec![
                ("X-A".into(), "1".into()),
                ("X-B".into(), "2".into()),
                ("X-A".into(), "3".into()),
            ],
            body: b"{}".to_vec(),
            covered_imp_ids: vec!["imp-1".into()],
        };
        assert_eq!(req.clone().headers, req.headers);
    }
}
```

Add the `pub use` line back in `lib.rs`:

```rust
//! Adapter framework. See spec §4 (auction core skeleton).
mod types;
pub use types::{BidderHttpRequest, BidderHttpResponse, BidderName, BidderResponse, TypedBid};
```

- [ ] **Step 2: Run tests**

```
cargo test -p auctioneer-bidder types::tests
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/src
git commit -m "auctioneer-bidder: transport-neutral types"
```

---

### Task 11: `BidderError` + `EntryPoint` + `Bidder` trait

**Files:**

- Modify: `crates/auctioneer-bidder/src/bidder.rs`
- Modify: `crates/auctioneer-bidder/src/lib.rs`

**Interfaces:**

- Consumes: `BidderHttpRequest`, `BidderHttpResponse`, `BidderResponse`, and `auctioneer-openrtb::types::bid_request::BidRequest`.
- Produces: synchronous trait per §4.1 + structured `BidderError` carrying numeric codes from Appendix G.

- [ ] **Step 1: Write tests + impl**

Replace `src/bidder.rs`:

```rust
//! `Bidder` trait — adapter contract. See spec §4.1.

use crate::{
    request_info::BidderRequestInfo,
    types::{BidderHttpRequest, BidderHttpResponse, BidderResponse},
};
use auctioneer_openrtb::types::bid_request::BidRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryPoint {
    Auction,
}

/// Per-bidder error / warning. Numeric code from Appendix G; severity is
/// caller-decided (the exchange maps code 6 to a warning, everything
/// else to `ext.errors`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidderError {
    pub code: u32,
    pub message: String,
}

impl BidderError {
    #[must_use] pub fn bad_input(msg: impl Into<String>) -> Self {
        Self { code: 2, message: msg.into() }
    }
    #[must_use] pub fn bad_server_response(msg: impl Into<String>) -> Self {
        Self { code: 4, message: msg.into() }
    }
    #[must_use] pub fn failed_to_request_bids(msg: impl Into<String>) -> Self {
        Self { code: 5, message: msg.into() }
    }
    #[must_use] pub fn failed_to_unmarshal(msg: impl Into<String>) -> Self {
        Self { code: 14, message: msg.into() }
    }
    #[must_use] pub fn unknown(msg: impl Into<String>) -> Self {
        Self { code: 999, message: msg.into() }
    }
}

/// Pure, synchronous adapter contract. No async, no I/O, no EdgeZero types.
/// The exchange owns the network call between `make_requests` and `make_bids`.
pub trait Bidder: Send + Sync {
    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
    ) -> Result<Vec<BidderHttpRequest>, Vec<BidderError>>;

    fn make_bids(
        &self,
        request: &BidRequest,
        http_request: &BidderHttpRequest,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PanickyBidder;
    impl Bidder for PanickyBidder {
        fn make_requests(&self, _r: &BidRequest, _i: &BidderRequestInfo<'_>)
            -> Result<Vec<BidderHttpRequest>, Vec<BidderError>>
        {
            Err(vec![BidderError::failed_to_request_bids("not implemented")])
        }
        fn make_bids(&self, _r: &BidRequest, _q: &BidderHttpRequest, _p: &BidderHttpResponse)
            -> Result<BidderResponse, Vec<BidderError>>
        {
            Err(vec![BidderError::failed_to_unmarshal("not implemented")])
        }
    }

    #[test]
    fn bidder_is_object_safe() {
        let b: &dyn Bidder = &PanickyBidder;
        let _ = b;
    }

    #[test]
    fn bidder_error_constructors_have_correct_codes() {
        assert_eq!(BidderError::bad_input("x").code, 2);
        assert_eq!(BidderError::bad_server_response("x").code, 4);
        assert_eq!(BidderError::failed_to_request_bids("x").code, 5);
        assert_eq!(BidderError::failed_to_unmarshal("x").code, 14);
        assert_eq!(BidderError::unknown("x").code, 999);
    }
}
```

Wire `lib.rs`:

```rust
//! Adapter framework. See spec §4.
mod bidder;
mod types;

pub use bidder::{Bidder, BidderError, EntryPoint};
pub use types::{BidderHttpRequest, BidderHttpResponse, BidderName, BidderResponse, TypedBid};
```

Note: the trait depends on `BidderRequestInfo` which doesn't yet exist. Temporarily inline a minimal stub so `bidder.rs` compiles:

```rust
// at the top of crates/auctioneer-bidder/src/request_info.rs, replace with:
//! `BidderRequestInfo` — stub; full shape lands in Task 12.
#[non_exhaustive]
pub struct BidderRequestInfo<'a> {
    pub _phantom: std::marker::PhantomData<&'a ()>,
}
```

Wire it into `lib.rs`:

```rust
mod request_info;
pub use request_info::BidderRequestInfo;
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-bidder bidder::tests
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/src
git commit -m "auctioneer-bidder: Bidder trait + BidderError"
```

---

### Task 12: `BidderRequestInfo` — full §4.1 shape

**Files:**

- Modify: `crates/auctioneer-bidder/src/request_info.rs`

**Interfaces:**

- Consumes: `BidderName`, `EntryPoint`.
- Produces: `#[non_exhaustive] pub struct BidderRequestInfo<'a> { bidder, core_name, account_id, endpoint_template, endpoint_macros, host_allowlist, gpc, entry_point, currency }` per §4.1.

- [ ] **Step 1: Write the test + impl**

Replace `src/request_info.rs`:

```rust
//! Context passed to `Bidder::make_requests`. See spec §4.1.

use crate::{bidder::EntryPoint, types::BidderName};

/// `#[non_exhaustive]` so adding fields later is non-breaking — see spec §4.1
/// "Fields intentionally omitted in phase 1 but reserved for later."
#[non_exhaustive]
pub struct BidderRequestInfo<'a> {
    /// Alias name (or base name when no aliasing happened).
    pub bidder: BidderName,
    /// Base bidder name (i.e. the adapter's own name in the registry).
    pub core_name: BidderName,
    /// `ResolvedAccount.id` post-fallback. Drives `{{.AccountID}}` macro.
    pub account_id: &'a str,
    /// Round-26 review High: raw `site.publisher.id` / `app.publisher.id`
    /// from the normalized request — whichever is present. Drives the
    /// `{{.PublisherID}}` endpoint macro per spec line 1331-1333. `None`
    /// when neither inventory branch carries a publisher id; an adapter
    /// whose template uses `{{.PublisherID}}` against such a request
    /// will fail `build()` with `MissingMacro("PublisherID")` →
    /// surfaced as code-5 `FailedToRequestBids`. Spec line 1331:
    /// "the two are intentionally distinct" — do NOT fall back to
    /// `account_id`.
    pub publisher_id: Option<&'a str>,
    /// `BidderInfo.endpoint` as a TEMPLATE (round-30 L5). The adapter
    /// constructs the final URL via `EndpointBuilder`; this is NOT
    /// the final outbound URL.
    pub endpoint_template: &'a str,
    /// Macro names permitted by this bidder's endpoint template
    /// (round-29 H3). E.g. `&["AccountID", "PublisherID"]`.
    pub endpoint_macros: &'a [&'a str],
    /// `Some(allowed_hosts)` for host-varying endpoints; `None` for
    /// fixed-host endpoints (§5.4).
    pub host_allowlist: Option<&'a [&'a str]>,
    /// `regs.ext.gpc` value if set on the normalized request.
    pub gpc: Option<&'a str>,
    /// Entry-point discriminator; phase 1 ships only `Auction`.
    pub entry_point: EntryPoint,
    /// `req.cur` (currency list, unchanged).
    pub currency: &'a [String],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_info_builds() {
        let macros: &[&str] = &["AccountID"];
        let currency: &[String] = &[];
        let info = BidderRequestInfo {
            bidder: BidderName::new("ax"),
            core_name: BidderName::new("appnexus"),
            account_id: "acct-42",
            publisher_id: None,
            endpoint_template: "https://example.test/openrtb?acct={{.AccountID}}",
            endpoint_macros: macros,
            host_allowlist: None,
            gpc: None,
            entry_point: EntryPoint::Auction,
            currency,
        };
        assert_eq!(info.bidder.as_str(), "ax");
        assert_eq!(info.core_name.as_str(), "appnexus");
        assert_eq!(info.account_id, "acct-42");
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-bidder request_info::tests
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/src/request_info.rs
git commit -m "auctioneer-bidder: BidderRequestInfo (full §4.1 shape)"
```

---

### Task 13: `EndpointBuilder` — macro percent-encoding + path-traversal reject + host_allowlist

**Files:**

- Modify: `crates/auctioneer-bidder/src/endpoint_builder.rs`
- Create: `crates/auctioneer-bidder/tests/endpoint_builder.rs`

**Interfaces:**

- Consumes: `BidderInfo` (uses only `endpoint`, `endpoint_macros`, `host_allowlist`).
- Produces: `pub struct EndpointBuilder<'a> { … }`; `pub fn new(info: &'a BidderInfo) -> Result<Self, EndpointBuilderError>`; `pub fn set_macro(&mut self, name: &str, value: &str) -> Result<(), EndpointBuilderError>`; `pub fn select_host(&mut self, host: &str) -> Result<(), EndpointBuilderError>`; `pub fn build(self) -> Result<String, EndpointBuilderError>`. Errors: `UnknownMacro`, `PathTraversal`, `HostNotInAllowlist`, `MissingMacro`, `MalformedTemplate`.

- [ ] **Step 1: Write the failing tests in `tests/endpoint_builder.rs`**

```rust
// Round-25 review Blocker 3: `clippy.toml` exempts test code from
// `unwrap_used` / `expect_used` / `panic_in_tests` /
// `indexing_slicing_in_tests`; only the integration-test layout lint
// needs an explicit allow. See constraint L466 for the full
// convention.
#![expect(
    clippy::tests_outside_test_module,
    reason = "integration tests live in tests/ by convention with no #[cfg(test)] wrapper"
)]

use auctioneer_bidder::{EndpointBuilder, EndpointBuilderError};
use auctioneer_config::BidderInfo;

fn info_fixed_host() -> BidderInfo {
    // Round-13 review High 5: endpoint_macros mirrors what codegen
    // would emit for this template.
    BidderInfo {
        name: "ax".into(),
        endpoint: "https://hb.example.test/openrtb?acct={{.AccountID}}".into(),
        uses_https: true,
        host_allowlist: None,
        endpoint_macros: vec!["AccountID".into()],
        ..BidderInfo::default()
    }
}

fn info_host_varying() -> BidderInfo {
    BidderInfo {
        name: "ax".into(),
        endpoint: "https://{{.Host}}/openrtb".into(),
        uses_https: true,
        host_allowlist: Some(vec!["eu.example.test".into(), "us.example.test".into()]),
        // {{.Host}} is excluded from endpoint_macros — it's the
        // authority token, handled via select_host.
        endpoint_macros: Vec::new(),
        ..BidderInfo::default()
    }
}

#[test]
fn fixed_host_substitutes_account_id() {
    let info = info_fixed_host();
    let mut b = EndpointBuilder::new(&info).unwrap();
    b.set_macro("AccountID", "acct-42").unwrap();
    let url = b.build().unwrap();
    assert_eq!(url, "https://hb.example.test/openrtb?acct=acct-42");
}

#[test]
fn macro_values_are_percent_encoded() {
    let info = info_fixed_host();
    let mut b = EndpointBuilder::new(&info).unwrap();
    b.set_macro("AccountID", "a b/c?d#e").unwrap();
    let url = b.build().unwrap();
    // Path/query positions: '/', '?', '#', and ' ' all get encoded.
    assert!(url.contains("acct=a%20b%2Fc%3Fd%23e"), "got: {url}");
}

#[test]
fn unknown_macro_rejected() {
    let info = info_fixed_host();
    let mut b = EndpointBuilder::new(&info).unwrap();
    let err = b.set_macro("PublisherID", "1234").unwrap_err();
    assert!(matches!(err, EndpointBuilderError::UnknownMacro(ref m) if m == "PublisherID"));
}

#[test]
fn path_traversal_value_rejected() {
    let info = info_fixed_host();
    let mut b = EndpointBuilder::new(&info).unwrap();
    // Round-13 review High 4: encoded forms must be rejected too.
    // %2f (encoded `/`) and %252e%252e%252f (double-encoded ../)
    // previously slipped past the literal-`/` split + single-pass
    // `%2e -> .` decode.
    for bad in [
        "..",
        "../../etc/passwd",
        "a/../b",
        "%2e%2e",
        "%2e%2e%2f",
        "..%2fevil",          // encoded slash after raw ..
        "%2e%2e%2f..",         // mixed encoded/raw traversal
        "%252e%252e%252f",     // double-encoded ../
        "a%00b",                // NUL byte truncation
    ] {
        let err = b.set_macro("AccountID", bad).unwrap_err();
        assert!(matches!(err, EndpointBuilderError::PathTraversal(_)), "value: {bad}");
    }
}

#[test]
fn unknown_macro_not_on_codegen_allow_list_rejected() {
    // Round-13 review High 5: even if a macro appears in the
    // template, set_macro must reject it when it's not on the
    // codegen-emitted endpoint_macros allow-list. Sanity check —
    // info_fixed_host's endpoint_macros (populated by codegen)
    // contains "AccountID" only.
    let info = info_fixed_host();
    let mut b = EndpointBuilder::new(&info).unwrap();
    let err = b.set_macro("SyncerKey", "x").unwrap_err();
    assert!(matches!(err, EndpointBuilderError::UnknownMacro(ref m) if m == "SyncerKey"));
}

#[test]
fn host_macro_cannot_be_set_via_set_macro() {
    let info = info_host_varying();
    let mut b = EndpointBuilder::new(&info).unwrap();
    // Host must NEVER be settable via set_macro — it's the authority
    // token, handled exclusively through select_host.
    let err = b.set_macro("Host", "eu.example.test").unwrap_err();
    assert!(matches!(err, EndpointBuilderError::UnknownMacro(_)));
}

#[test]
fn host_varying_requires_host_selection() {
    let info = info_host_varying();
    let b = EndpointBuilder::new(&info).unwrap();
    let err = b.build().unwrap_err();
    assert!(matches!(err, EndpointBuilderError::MissingMacro(ref m) if m == "Host"));
}

#[test]
fn host_varying_off_allowlist_rejected() {
    let info = info_host_varying();
    let mut b = EndpointBuilder::new(&info).unwrap();
    let err = b.select_host("rogue.example.test").unwrap_err();
    assert!(matches!(err, EndpointBuilderError::HostNotInAllowlist(_)));
}

#[test]
fn host_varying_allowlisted_host_accepted() {
    let info = info_host_varying();
    let mut b = EndpointBuilder::new(&info).unwrap();
    b.select_host("eu.example.test").unwrap();
    let url = b.build().unwrap();
    assert_eq!(url, "https://eu.example.test/openrtb");
}
```

- [ ] **Step 2: Verify it fails to compile**

```
cargo test -p auctioneer-bidder --test endpoint_builder 2>&1 | head -20
```

Expected: `EndpointBuilder` undefined; this confirms our test will drive the impl.

- [ ] **Step 3: Implement `src/endpoint_builder.rs`**

```rust
//! `EndpointBuilder` — macro percent-encoding, path-traversal rejection,
//! host_allowlist lookup. See spec §5.4.

use auctioneer_config::BidderInfo;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointBuilderError {
    UnknownMacro(String),
    PathTraversal(String),
    HostNotInAllowlist(String),
    MissingMacro(String),
    MalformedTemplate(String),
}

pub struct EndpointBuilder<'a> {
    template: &'a str,
    host_allowlist: Option<&'a [String]>,
    /// Round-13 review High 5: the codegen-emitted macro allow-list
    /// (BidderInfo.endpoint_macros, populated by build_support's
    /// extract_endpoint_macros scanner in the round-12 codegen pass).
    /// `set_macro` validates `name` against this set so a typo or a
    /// new placeholder added to the template only by an adapter
    /// (without updating bidder-info) is rejected at runtime.
    allowed_macros: &'a [String],
    values: HashMap<String, String>,   // macro name -> encoded substituted value
    host_selected: Option<String>,
}

impl<'a> EndpointBuilder<'a> {
    pub fn new(info: &'a BidderInfo) -> Result<Self, EndpointBuilderError> {
        Ok(Self {
            template: &info.endpoint,
            host_allowlist: info.host_allowlist.as_deref(),
            allowed_macros: &info.endpoint_macros,
            values: HashMap::new(),
            host_selected: None,
        })
    }

    pub fn set_macro(&mut self, name: &str, value: &str) -> Result<(), EndpointBuilderError> {
        // Round-13 review High 5: validate against the codegen
        // allow-list (info.endpoint_macros) — NOT just "does the
        // placeholder appear in the template." Host is the special
        // authority macro and is handled exclusively via
        // `select_host`; it MUST NOT be settable here.
        if name == "Host" {
            return Err(EndpointBuilderError::UnknownMacro(
                "Host is the authority token — use select_host instead".into(),
            ));
        }
        if !self.allowed_macros.iter().any(|allowed| allowed == name) {
            return Err(EndpointBuilderError::UnknownMacro(name.into()));
        }
        // Defense in depth: also reject macros that don't appear in
        // the current template (e.g. the bidder declares a macro it
        // doesn't use; the adapter shouldn't be able to set it).
        let placeholder = format!("{{{{.{name}}}}}");
        if !self.template.contains(&placeholder) {
            return Err(EndpointBuilderError::UnknownMacro(name.into()));
        }
        // Round-13 review High 4: percent-decode the input once,
        // recursively until convergence, before the segment scan.
        // The old `replace("%2e", ".")` missed `%2f` (encoded `/`)
        // entirely, so `..%2f` slipped past the literal-`/` split.
        // After full decoding any `..` or `.` segment is a traversal
        // attempt regardless of how it was encoded.
        let inspect = percent_decode_fully(value);
        if inspect.split(['/', '\\']).any(|seg| seg == ".." || seg == ".") {
            return Err(EndpointBuilderError::PathTraversal(value.into()));
        }
        // Also reject NUL bytes, which some URL parsers truncate on
        // and would let `acct\0evil` smuggle a second path segment.
        if inspect.contains('\0') {
            return Err(EndpointBuilderError::PathTraversal(value.into()));
        }
        // Percent-encode for safe path/query insertion. NON_ALPHANUMERIC
        // is conservative: it encodes every byte that isn't `[A-Za-z0-9]`.
        let encoded: String = utf8_percent_encode(value, NON_ALPHANUMERIC).to_string();
        self.values.insert(name.into(), encoded);
        Ok(())
    }

    pub fn select_host(&mut self, host: &str) -> Result<(), EndpointBuilderError> {
        let allow = self.host_allowlist.ok_or_else(|| {
            EndpointBuilderError::UnknownMacro("Host".into())
        })?;
        if !allow.iter().any(|h| h == host) {
            return Err(EndpointBuilderError::HostNotInAllowlist(host.into()));
        }
        self.host_selected = Some(host.into());
        Ok(())
    }

    pub fn build(self) -> Result<String, EndpointBuilderError> {
        let mut url = self.template.to_owned();
        // Host substitution first if the template references {{.Host}}.
        if url.contains("{{.Host}}") {
            let host = self.host_selected.ok_or_else(|| {
                EndpointBuilderError::MissingMacro("Host".into())
            })?;
            url = url.replace("{{.Host}}", &host);
        }
        // Substitute every value the caller set.
        for (name, encoded) in &self.values {
            let placeholder = format!("{{{{.{name}}}}}");
            url = url.replace(&placeholder, encoded);
        }
        // If anything is still unsubstituted, it's a missing macro.
        if let Some(rest) = url.find("{{.") {
            let end = url[rest..].find("}}").ok_or_else(|| {
                EndpointBuilderError::MalformedTemplate(url[rest..].into())
            })?;
            // Extract the macro name between "{{." and "}}".
            let name = &url[rest + 3..rest + end];
            return Err(EndpointBuilderError::MissingMacro(name.into()));
        }
        Ok(url)
    }
}

/// Round-13 review High 4: percent-decode the input once,
/// recursively until convergence, so chains like `%2525` (encoded
/// `%25`, which decodes to `%`) and double-encoded traversal like
/// `%252e%252e%252f` resolve to `../` before the segment scan.
/// We bound the loop at 8 passes — past that the input is either
/// pathological or fully decoded; either way the scan can run on
/// what we have.
fn percent_decode_fully(input: &str) -> String {
    let mut current = input.to_owned();
    for _ in 0..8 {
        let decoded = percent_encoding::percent_decode_str(&current)
            .decode_utf8_lossy()
            .into_owned();
        if decoded == current { return decoded; }
        current = decoded;
    }
    current
}
```

Add `pub use` in `lib.rs`:

```rust
mod endpoint_builder;
pub use endpoint_builder::{EndpointBuilder, EndpointBuilderError};
```

- [ ] **Step 4: Run tests**

```
cargo test -p auctioneer-bidder --test endpoint_builder
```

Expected: 10 passed (7 originals + 3 added in round-13 review).

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-bidder/src crates/auctioneer-bidder/tests
git commit -m "auctioneer-bidder: EndpointBuilder (macro encode + traversal + host_allowlist)"
```

---

### Task 14: `BidderRegistry`

**Files:**

- Modify: `crates/auctioneer-bidder/src/registry.rs`

**Interfaces:**

- Consumes: `Bidder`, `BidderName`, `auctioneer-config::BidderInfo`.
- Produces: `pub struct BidderRegistry { … }`; methods `pub fn new() -> Self`, `pub fn register(&mut self, info: &'static BidderInfo, adapter: Box<dyn Bidder>)`, `pub fn get(&self, name: &str) -> Option<(&'static BidderInfo, &dyn Bidder)>`, `pub fn base_names(&self) -> Vec<BidderName>` (lexicographic).

- [ ] **Step 1: Write tests + impl**

Replace `src/registry.rs`:

```rust
//! Bidder registry. See spec §4.2.

use crate::{bidder::Bidder, types::BidderName};
use auctioneer_config::BidderInfo;
use std::collections::BTreeMap;

/// Holds `Box<dyn Bidder>` adapter instances paired with their `&'static`
/// `BidderInfo`. `BTreeMap` keyed by `BidderName` so iteration is in
/// §5.9 lexicographic order automatically.
pub struct BidderRegistry {
    entries: BTreeMap<BidderName, RegistryEntry>,
}

struct RegistryEntry {
    info: &'static BidderInfo,
    adapter: Box<dyn Bidder>,
}

impl BidderRegistry {
    #[must_use] pub fn new() -> Self {
        Self { entries: BTreeMap::new() }
    }

    pub fn register(&mut self, info: &'static BidderInfo, adapter: Box<dyn Bidder>) {
        let name = BidderName::new(info.name.as_str());
        self.entries.insert(name, RegistryEntry { info, adapter });
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<(&'static BidderInfo, &dyn Bidder)> {
        self.entries.get(&BidderName::new(name))
            .map(|e| (e.info, e.adapter.as_ref()))
    }

    /// Returns base bidder names in lexicographic order. Used by §5.9 to
    /// construct the participating-set for an auction (combined with
    /// request-defined alias names + host-config aliases at the exchange
    /// boundary).
    #[must_use]
    pub fn base_names(&self) -> Vec<BidderName> {
        self.entries.keys().cloned().collect()
    }
}

impl Default for BidderRegistry {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bidder::{Bidder, BidderError, EntryPoint}, request_info::BidderRequestInfo,
                types::{BidderHttpRequest, BidderHttpResponse, BidderResponse}};
    use auctioneer_openrtb::types::bid_request::BidRequest;

    struct DummyBidder(u32);
    impl Bidder for DummyBidder {
        fn make_requests(&self, _r: &BidRequest, _i: &BidderRequestInfo<'_>)
            -> Result<Vec<BidderHttpRequest>, Vec<BidderError>>
        { Err(vec![BidderError::unknown(format!("{}", self.0))]) }
        fn make_bids(&self, _r: &BidRequest, _q: &BidderHttpRequest, _p: &BidderHttpResponse)
            -> Result<BidderResponse, Vec<BidderError>>
        { Err(vec![]) }
    }

    fn leak_info(name: &'static str) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(),
            ..BidderInfo::default()
        }))
    }

    #[test]
    fn base_names_are_lexicographic() {
        let mut r = BidderRegistry::new();
        r.register(leak_info("pubmatic"), Box::new(DummyBidder(1)));
        r.register(leak_info("appnexus"), Box::new(DummyBidder(2)));
        r.register(leak_info("generic"),  Box::new(DummyBidder(3)));
        let names: Vec<String> = r.base_names().into_iter().map(|n| n.as_str().to_owned()).collect();
        assert_eq!(names, vec!["appnexus", "generic", "pubmatic"]);
    }

    #[test]
    fn get_returns_paired_info_and_adapter() {
        let mut r = BidderRegistry::new();
        r.register(leak_info("ax"), Box::new(DummyBidder(7)));
        let (info, _adapter) = r.get("ax").unwrap();
        assert_eq!(info.name, "ax");
    }

    #[test]
    fn unknown_name_returns_none() {
        let r = BidderRegistry::new();
        assert!(r.get("nope").is_none());
    }
}
```

Wire `lib.rs`:

```rust
mod registry;
pub use registry::BidderRegistry;
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-bidder registry::tests
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/src/registry.rs
git commit -m "auctioneer-bidder: BidderRegistry"
```

---

### Task 15: `GenericBidder`

**Files:**

- Create: `crates/auctioneer-bidder/src/adapters/mod.rs`
- Create: `crates/auctioneer-bidder/src/adapters/generic.rs`

**Interfaces:**

- Consumes: the `Bidder` trait + transport types.
- Produces: `pub struct GenericBidder`; near-verbatim OpenRTB transform — serialize the `BidRequest` to JSON, POST to `info.endpoint_template` (passing through `EndpointBuilder` with no macros if none are declared), decode the response as `BidResponse` and emit one `TypedBid` per `seatbid[*].bid[*]`.

- [ ] **Step 1: Implement adapter `mod.rs`**

```rust
// Round-24 review Blocker 1: `url` is a shared helper module — Generic,
// AppNexus, and Pubmatic all call `url::build_outbound_url(info)` to
// resolve the endpoint template through `EndpointBuilder` against the
// codegen-emitted `endpoint_macros` allow-list. It is declared BEFORE
// the adapter modules so each can `use super::url::build_outbound_url`.
mod url;
mod generic;
pub use generic::GenericBidder;
mod appnexus;
pub use appnexus::AppnexusBidder;
mod pubmatic;
pub use pubmatic::PubmaticBidder;
#[cfg(feature = "insecure-bidders")]
mod mocktioneer;
#[cfg(feature = "insecure-bidders")]
pub use mocktioneer::MocktioneerBidder;
```

Create stub files for `url.rs`, `appnexus.rs`, `pubmatic.rs`, and (conditionally) `mocktioneer.rs` — `url.rs` carries the helper landed in Step 2 below; the three adapters each carry only a stub `pub struct X;` so the module compiles. We fill those in subsequent tasks.

- [ ] **Step 1b: Implement `src/adapters/url.rs`** — shared outbound URL resolver

```rust
//! Shared outbound-URL helper. All adapters route their endpoint
//! template through here so macro expansion, host selection, and the
//! codegen `endpoint_macros` allow-list are enforced uniformly.
//!
//! Round-24 review Blocker 1: this helper used to live as a private
//! `fn` inside `adapters/generic.rs`, which broke for two reasons:
//! (a) AppNexus and Pubmatic could not call it from sibling modules,
//! and (b) the body referenced `auctioneer_bidder::EndpointBuilder` which
//! does NOT resolve when compiling `auctioneer-bidder` itself (the crate
//! cannot name itself by external path). The helper now lives here
//! as `pub(super)` and uses `crate::EndpointBuilder` instead.

use crate::{
    bidder::BidderError,
    endpoint_builder::EndpointBuilder,
    request_info::BidderRequestInfo,
};

/// Build a transient `BidderInfo` shape for the `EndpointBuilder` from
/// the thin `BidderRequestInfo` view. Bridge until the registry hands
/// the full `&'static BidderInfo` down through the trait surface.
pub(super) fn synthesize_info(info: &BidderRequestInfo<'_>) -> auctioneer_config::BidderInfo {
    auctioneer_config::BidderInfo {
        name: info.bidder.as_str().to_owned(),
        endpoint: info.endpoint_template.to_owned(),
        uses_https: info.endpoint_template.starts_with("https://"),
        host_allowlist: info.host_allowlist.map(|hosts|
            hosts.iter().map(|h| (*h).to_owned()).collect()),
        // Round-23 review High 2: carry the codegen-emitted
        // `endpoint_macros` allow-list through so `set_macro` calls
        // (e.g. `AccountID`) are not rejected as `UnknownMacro`.
        endpoint_macros: info.endpoint_macros.iter().map(|m| (*m).to_owned()).collect(),
        ..auctioneer_config::BidderInfo::default()
    }
}

/// Adapters call this to convert a `BidderRequestInfo` (template +
/// macros + optional host_allowlist) into a final URL.
///
/// Round-25 review High — the earlier draft auto-selected
/// `host_allowlist.first()` when the template carried `{{.Host}}`.
/// That violated spec §5.4 (line 1316 / 1334): adapters need
/// REQUEST-DERIVED control over host selection (e.g. Pubmatic
/// publisher-region routing), and silently picking the first
/// allow-list entry would steer every imp to one of N possible
/// regions — wrong AND undetectable. The helper now leaves
/// `{{.Host}}` UNRESOLVED; adapters that need a host call
/// `build_outbound_url_with_host(info, host)` below with an explicit
/// host they DERIVED from the request. If no adapter supplied a host
/// and the template references `{{.Host}}`, `EndpointBuilder::build`
/// returns `MissingMacro("Host")` (round-13 review High 4) which
/// surfaces as a code-5 `FailedToRequestBids` — loud, attributable,
/// and points the implementer at the missing select_host call site.
pub(super) fn build_outbound_url(info: &BidderRequestInfo<'_>) -> Result<String, Vec<BidderError>> {
    // Hold the synthesized BidderInfo on the stack so EndpointBuilder
    // can borrow from it for the duration of this call. The builder
    // never escapes the function, so this is sound.
    let synthesized = synthesize_info(info);
    let mut builder = EndpointBuilder::new(&synthesized)
        .map_err(|e| vec![BidderError::failed_to_request_bids(format!("{e:?}"))])?;
    fill_request_macros(&mut builder, info, &synthesized)?;
    builder.build()
        .map_err(|e| vec![BidderError::failed_to_request_bids(format!("{e:?}"))])
}

/// Same as `build_outbound_url` but with a host the adapter chose
/// for this auction. Use this from a host-varying adapter (e.g.
/// Pubmatic) once it has resolved which region/host the request
/// belongs to. `host` MUST be a member of `info.host_allowlist`;
/// `EndpointBuilder::select_host` enforces this and returns
/// `HostNotInAllowlist` for any other value (round-13 review
/// High 4 / §5.4 line 1334).
pub(super) fn build_outbound_url_with_host(
    info: &BidderRequestInfo<'_>,
    host: &str,
) -> Result<String, Vec<BidderError>> {
    let synthesized = synthesize_info(info);
    let mut builder = EndpointBuilder::new(&synthesized)
        .map_err(|e| vec![BidderError::failed_to_request_bids(format!("{e:?}"))])?;
    fill_request_macros(&mut builder, info, &synthesized)?;
    builder.select_host(host)
        .map_err(|e| vec![BidderError::failed_to_request_bids(format!("{e:?}"))])?;
    builder.build()
        .map_err(|e| vec![BidderError::failed_to_request_bids(format!("{e:?}"))])
}

fn fill_request_macros(
    builder: &mut EndpointBuilder<'_>,
    info: &BidderRequestInfo<'_>,
    synthesized: &auctioneer_config::BidderInfo,
) -> Result<(), Vec<BidderError>> {
    // Round-26 review High: the helper now fills BOTH `AccountID`
    // and `PublisherID` (spec line 1325, 1331 — the spec's auction core test
    // surface at spec line 3938 requires both). Other macros
    // (`{{.GDPR}}`, `{{.GPP}}`, `{{.RedirectURL}}`, etc.) live on the
    // sync path and are user sync; they will never appear in an endpoint
    // template under the auction core codegen guard (Task 22b).
    for name in synthesized.endpoint_macros.iter() {
        match name.as_str() {
            "AccountID" => {
                builder.set_macro("AccountID", info.account_id)
                    .map_err(|e| vec![BidderError::failed_to_request_bids(format!("{e:?}"))])?;
            }
            "PublisherID" => {
                // Spec line 1331-1333: PublisherID is raw
                // site.publisher.id / app.publisher.id — the
                // "intentionally distinct" wording forbids falling
                // back to AccountID. When absent, leave the macro
                // unset so `build()` returns `MissingMacro` →
                // code-5 instead of producing a malformed URL.
                if let Some(pid) = info.publisher_id {
                    builder.set_macro("PublisherID", pid)
                        .map_err(|e| vec![BidderError::failed_to_request_bids(format!("{e:?}"))])?;
                }
            }
            _ => {
                // Future macros (e.g. region keys) belong at the
                // adapter call site, NOT in this shared helper.
                // The codegen guard (Task 22b) already rejects any
                // macro not in the auction core supported set, so this arm
                // is only reachable if the guard's allow-list
                // expanded out of sync with this match.
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Implement `src/adapters/generic.rs`**

```rust
//! Generic OpenRTB passthrough adapter. See spec §4.3 row 1.

use super::url::build_outbound_url;
use crate::{
    bidder::{Bidder, BidderError},
    request_info::BidderRequestInfo,
    types::{BidderHttpRequest, BidderHttpResponse, BidderResponse, TypedBid},
};
use auctioneer_config::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::{BidResponse, Bid}};

pub struct GenericBidder;

impl Bidder for GenericBidder {
    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
    ) -> Result<Vec<BidderHttpRequest>, Vec<BidderError>> {
        // Round-24 review Blocker 1: Generic MUST route through the
        // shared `build_outbound_url` helper so any `{{.AccountID}}` /
        // `{{.Host}}` macro the codegen surfaced gets resolved (and
        // the test `make_requests_substitutes_account_id_macro` below
        // — which uses `{{.AccountID}}` in the template — passes). The
        // earlier `EndpointBuilder::new(...).build()` call skipped
        // `set_macro` entirely and therefore returned `MissingMacro`.
        let url = build_outbound_url(info)?;

        let body = serde_json::to_vec(request)
            .map_err(|e| vec![BidderError::failed_to_request_bids(e.to_string())])?;
        let covered_imp_ids = request.imp.iter().map(|i| i.id.clone()).collect();

        Ok(vec![BidderHttpRequest {
            method: "POST".into(),
            url,
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("Accept".into(),       "application/json".into()),
            ],
            body,
            covered_imp_ids,
        }])
    }

    fn make_bids(
        &self,
        _request: &BidRequest,
        _http_request: &BidderHttpRequest,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        if response.status >= 400 {
            return Err(vec![BidderError::bad_server_response(
                format!("status={}", response.status),
            )]);
        }
        // Some adapters return 204 No Content for "no bids" — emit empty.
        if response.status == 204 || response.body.is_empty() {
            return Ok(BidderResponse { bids: vec![], currency: "USD".into() });
        }
        let parsed: BidResponse = serde_json::from_slice(&response.body)
            .map_err(|e| vec![BidderError::failed_to_unmarshal(e.to_string())])?;
        let mut bids = Vec::new();
        for seat in &parsed.seatbid {
            for b in &seat.bid {
                bids.push(TypedBid {
                    bid: b.clone(),
                    media_type: infer_media_type(b),
                });
            }
        }
        let currency = if parsed.cur.is_empty() { "USD".into() } else { parsed.cur.clone() };
        Ok(BidderResponse { bids, currency })
    }
}

// Round-24 review Blocker 1: `synthesize_info` and `build_outbound_url`
// now live in `crate::adapters::url` (declared in `adapters/mod.rs`)
// so AppNexus / Pubmatic / Generic all share one helper. See the
// imports above (`use super::url::build_outbound_url`).

fn infer_media_type(b: &auctioneer_openrtb::types::bid_response::Bid) -> MediaType {
    // Round-23 review High 3: the earlier draft commented "honors
    // `bid.ext.prebid.type`" but the body always returned Banner,
    // which broke `hb_format` + `mediatypepricegranularity` for any
    // generic-adapter bid that wasn't actually a banner. Probe the
    // same three slots as Pubmatic so the spec contract holds.
    let Some(ext) = b.ext.as_ref() else {
        return MediaType::Banner;
    };
    if let Some(prebid) = ext.prebid.as_ref() {
        if let Some(mt) = prebid.bid_type.as_deref() {
            return parse_media_type_str(mt);
        }
        if let Some(mt) = prebid.other.get("mediatype").and_then(|v| v.as_str()) {
            return parse_media_type_str(mt);
        }
    }
    for (key, media_type) in [
        ("video",  MediaType::Video),
        ("native", MediaType::Native),
        ("audio",  MediaType::Audio),
    ] {
        if ext.other.get(key).is_some() {
            return media_type;
        }
    }
    MediaType::Banner
}

fn parse_media_type_str(name: &str) -> MediaType {
    match name {
        "video"  => MediaType::Video,
        "native" => MediaType::Native,
        "audio"  => MediaType::Audio,
        _        => MediaType::Banner,
    }
}
```

- [ ] **Step 3: Compile**

```
cargo check -p auctioneer-bidder
```

Expected: success (the empty `appnexus.rs` / `pubmatic.rs` / `mocktioneer.rs` placeholders compile because their `pub struct X;` and the matching re-exports are valid).

- [ ] **Step 4: Inline unit test in `generic.rs`**

Append to `src/adapters/generic.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bidder::EntryPoint, types::BidderName};
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};

    fn info_for(template: &str) -> auctioneer_config::BidderInfo {
        auctioneer_config::BidderInfo {
            name: "generic".into(),
            endpoint: template.into(),
            uses_https: template.starts_with("https://"),
            ..auctioneer_config::BidderInfo::default()
        }
    }

    #[test]
    fn make_requests_emits_one_post_with_serialized_request() {
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![Imp { id: "i-1".into(), ..Imp::default() }],
            ..BidRequest::default()
        };
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let info = BidderRequestInfo {
            bidder: BidderName::new("generic"),
            core_name: BidderName::new("generic"),
            account_id: "acct-x",
            publisher_id: None,
            endpoint_template: "https://demo.example.test/openrtb",
            endpoint_macros: macros,
            host_allowlist: None,
            gpc: None,
            entry_point: EntryPoint::Auction,
            currency,
        };
        let out = GenericBidder.make_requests(&req, &info).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].method, "POST");
        assert_eq!(out[0].url, "https://demo.example.test/openrtb");
        assert_eq!(out[0].covered_imp_ids, vec!["i-1".to_string()]);
        let body: serde_json::Value = serde_json::from_slice(&out[0].body).unwrap();
        assert_eq!(body["id"], "auc-1");
    }

    #[test]
    fn make_requests_substitutes_account_id_macro() {
        // Round-23 review High 2: when the endpoint template carries
        // `{{.AccountID}}`, the synthesized BidderInfo MUST list it
        // in endpoint_macros AND build_outbound_url MUST resolve it
        // to the account id (percent-encoded). Without
        // endpoint_macros, EndpointBuilder rejects the macro as
        // `UnknownMacro`.
        let req = BidRequest {
            id: "auc-2".into(),
            imp: vec![Imp { id: "i-1".into(), ..Imp::default() }],
            ..BidRequest::default()
        };
        let macros: &[&str] = &["AccountID"];
        let currency: &[String] = &[];
        let info = BidderRequestInfo {
            bidder: BidderName::new("generic"),
            core_name: BidderName::new("generic"),
            account_id: "acct/space encoded?",
            publisher_id: None,
            endpoint_template: "https://demo.example.test/openrtb?acct={{.AccountID}}",
            endpoint_macros: macros,
            host_allowlist: None,
            gpc: None,
            entry_point: EntryPoint::Auction,
            currency,
        };
        let out = GenericBidder.make_requests(&req, &info).unwrap();
        assert_eq!(out.len(), 1);
        // Percent-encoded by EndpointBuilder (NON_ALPHANUMERIC encoder).
        assert!(
            out[0].url.ends_with("acct=acct%2Fspace%20encoded%3F"),
            "expected encoded account id, got {}",
            out[0].url,
        );
    }

    #[test]
    fn make_bids_returns_empty_on_204() {
        let req  = BidRequest::default();
        let http = BidderHttpRequest {
            method: "POST".into(), url: "https://x".into(),
            headers: vec![], body: vec![], covered_imp_ids: vec![],
        };
        let resp = BidderHttpResponse {
            status: 204, headers: vec![], body: vec![],
        };
        let out = GenericBidder.make_bids(&req, &http, &resp).unwrap();
        assert!(out.bids.is_empty());
    }

    #[test]
    fn make_bids_status_500_emits_bad_server_response() {
        let req  = BidRequest::default();
        let http = BidderHttpRequest {
            method: "POST".into(), url: "https://x".into(),
            headers: vec![], body: vec![], covered_imp_ids: vec![],
        };
        let resp = BidderHttpResponse {
            status: 500, headers: vec![], body: b"oops".to_vec(),
        };
        let errs = GenericBidder.make_bids(&req, &http, &resp).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, 4);
    }

    #[test]
    fn make_bids_parses_seatbid_into_typed_bids() {
        let req  = BidRequest::default();
        let http = BidderHttpRequest {
            method: "POST".into(), url: "https://x".into(),
            headers: vec![], body: vec![], covered_imp_ids: vec![],
        };
        let json = br#"{"id":"r-1","seatbid":[{"seat":"ax","bid":[{"id":"b-1","impid":"i-1","price":1.5}]}],"cur":"USD"}"#;
        let resp = BidderHttpResponse {
            status: 200, headers: vec![], body: json.to_vec(),
        };
        let out = GenericBidder.make_bids(&req, &http, &resp).unwrap();
        assert_eq!(out.bids.len(), 1);
        assert_eq!(out.bids[0].bid.impid, "i-1");
        assert_eq!(out.currency, "USD");
    }
}
```

- [ ] **Step 5: Run**

```
cargo test -p auctioneer-bidder adapters::generic::tests
```

Expected: 4 passed.

- [ ] **Step 6: Commit**

```
git add crates/auctioneer-bidder/src/adapters
git commit -m "auctioneer-bidder: GenericBidder (OpenRTB passthrough)"
```

---

### Task 16: `AppnexusBidder` — custom request transform

**Files:**

- Modify: `crates/auctioneer-bidder/src/adapters/appnexus.rs`

**Interfaces:**

- Consumes: `BidRequest`, `BidderRequestInfo`.
- Produces: `pub struct AppnexusBidder`; transforms per-imp `imp.ext.bidder.placementId` (typed `u64`) into a top-level `req.ext.appnexus.placement_ids` array; otherwise an OpenRTB passthrough.

- [ ] **Step 1: Replace `src/adapters/appnexus.rs`**

```rust
//! AppNexus adapter — custom request transform exemplar. See spec §4.3 row 2.

use super::url::build_outbound_url;
use crate::{
    bidder::{Bidder, BidderError},
    request_info::BidderRequestInfo,
    types::{BidderHttpRequest, BidderHttpResponse, BidderResponse, TypedBid},
};
use auctioneer_config::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};

pub struct AppnexusBidder;

impl Bidder for AppnexusBidder {
    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
    ) -> Result<Vec<BidderHttpRequest>, Vec<BidderError>> {
        // Round-14 review Blocker Imp.ext drift: Imp.ext is typed
        // Option<ImpExt> per foundations (NOT Box<RawValue>). The exchange's
        // per-imp split + bidder-param merge stage folds the resolved
        // params into imp.ext.bidder (Option<serde_json::Value>), so
        // the adapter reads them directly via typed accessors — no
        // JSON re-parse.
        let mut placement_ids: Vec<u64> = Vec::new();
        for imp in &request.imp {
            let Some(bidder_params) = imp.ext.as_ref().and_then(|e| e.bidder.as_ref()) else {
                continue;
            };
            if let Some(placement) = bidder_params.get("placementId") {
                let pid = placement.as_u64().ok_or_else(|| {
                    vec![BidderError::bad_input("placementId must be a u64")]
                })?;
                placement_ids.push(pid);
            }
        }
        if placement_ids.is_empty() {
            return Err(vec![BidderError::bad_input(
                "appnexus: at least one imp must carry imp.ext.bidder.placementId",
            )]);
        }

        // Clone the request with an appended ext.appnexus.placement_ids — we
        // ship the JSON via serde_json::Value so we don't need to type the
        // ext.appnexus field in auctioneer-openrtb.
        let mut wire: serde_json::Value = serde_json::to_value(request)
            .map_err(|e| vec![BidderError::failed_to_request_bids(e.to_string())])?;
        wire["ext"]["appnexus"]["placement_ids"] =
            serde_json::Value::Array(placement_ids.iter()
                .map(|p| serde_json::Value::from(*p)).collect());
        let body = serde_json::to_vec(&wire)
            .map_err(|e| vec![BidderError::failed_to_request_bids(e.to_string())])?;

        // Round-23 review High 1: BidderRequestInfo.endpoint_template
        // is a TEMPLATE per spec line 511 / §5.4 line 1308. Resolve
        // macros + host selection through EndpointBuilder so any
        // `{{.AccountID}}` / `{{.Host}}` substitutions are honored
        // AND validated against the codegen endpoint_macros allow-
        // list. The current AppNexus YAML ships a static endpoint
        // with no macros, but the auction core contract exposes
        // `endpoint_macros` / `host_allowlist` — if a future YAML
        // revision adds macros, this path stays correct (and
        // rejects unknown ones at the gate).
        let url = build_outbound_url(info)?;
        let covered_imp_ids = request.imp.iter().map(|i| i.id.clone()).collect();
        Ok(vec![BidderHttpRequest {
            method: "POST".into(),
            url,
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("Accept".into(),       "application/json".into()),
            ],
            body,
            covered_imp_ids,
        }])
    }

    fn make_bids(
        &self,
        _request: &BidRequest,
        _http_request: &BidderHttpRequest,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        if response.status >= 400 {
            return Err(vec![BidderError::bad_server_response(
                format!("status={}", response.status))]);
        }
        if response.body.is_empty() {
            return Ok(BidderResponse { bids: vec![], currency: "USD".into() });
        }
        let parsed: BidResponse = serde_json::from_slice(&response.body)
            .map_err(|e| vec![BidderError::failed_to_unmarshal(e.to_string())])?;
        let mut bids = Vec::new();
        for seat in &parsed.seatbid {
            for b in &seat.bid {
                bids.push(TypedBid { bid: b.clone(), media_type: MediaType::Banner });
            }
        }
        let currency = if parsed.cur.is_empty() { "USD".into() } else { parsed.cur.clone() };
        Ok(BidderResponse { bids, currency })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bidder::EntryPoint, types::BidderName};
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};

    fn imp_with_placement(id: &str, pid: u64) -> Imp {
        // Round-14 review Blocker Imp.ext drift: typed ImpExt with
        // the legacy bidder block populated (placementId is the
        // resolved appnexus-namespaced param after the per-imp merge).
        Imp {
            id: id.into(),
            ext: Some(ImpExt {
                bidder: Some(serde_json::json!({ "placementId": pid })),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    #[test]
    fn make_requests_emits_placement_ids_in_ext_appnexus() {
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![
                imp_with_placement("i-1", 11),
                imp_with_placement("i-2", 22),
            ],
            ..BidRequest::default()
        };
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let info = BidderRequestInfo {
            bidder: BidderName::new("appnexus"),
            core_name: BidderName::new("appnexus"),
            account_id: "acct-x",
            publisher_id: None,
            endpoint_template: "https://ib.adnxs.com/openrtb2",
            endpoint_macros: macros,
            host_allowlist: None,
            gpc: None,
            entry_point: EntryPoint::Auction,
            currency,
        };
        let out = AppnexusBidder.make_requests(&req, &info).unwrap();
        let body: serde_json::Value = serde_json::from_slice(&out[0].body).unwrap();
        let ids = body["ext"]["appnexus"]["placement_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0].as_u64().unwrap(), 11);
        assert_eq!(ids[1].as_u64().unwrap(), 22);
    }

    #[test]
    fn make_requests_rejects_when_no_placement_ids_present() {
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![Imp { id: "i-1".into(), ..Imp::default() }],
            ..BidRequest::default()
        };
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let info = BidderRequestInfo {
            bidder: BidderName::new("appnexus"),
            core_name: BidderName::new("appnexus"),
            account_id: "acct-x",
            publisher_id: None,
            endpoint_template: "https://ib.adnxs.com/openrtb2",
            endpoint_macros: macros,
            host_allowlist: None,
            gpc: None,
            entry_point: EntryPoint::Auction,
            currency,
        };
        let errs = AppnexusBidder.make_requests(&req, &info).unwrap_err();
        assert_eq!(errs[0].code, 2);
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-bidder adapters::appnexus::tests
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/src/adapters/appnexus.rs
git commit -m "auctioneer-bidder: AppnexusBidder (custom request transform)"
```

---

### Task 17: `PubmaticBidder` — custom response transform + media-type inference

**Files:**

- Modify: `crates/auctioneer-bidder/src/adapters/pubmatic.rs`

**Interfaces:**

- Consumes: `BidRequest`, `BidderRequestInfo`, `BidderHttpResponse`.
- Produces: `pub struct PubmaticBidder`. Custom response transform: media type inferred from `bid.ext.video` (present → video) or `bid.ext.x-prebid.mediatype` (when set explicitly), else banner; preserve `bid.ext.dealtier`.

- [ ] **Step 1: Replace `src/adapters/pubmatic.rs`**

```rust
//! Pubmatic adapter — custom response transform exemplar. See spec §4.3 row 3.

use super::url::build_outbound_url;
use crate::{
    bidder::{Bidder, BidderError},
    request_info::BidderRequestInfo,
    types::{BidderHttpRequest, BidderHttpResponse, BidderResponse, TypedBid},
};
use auctioneer_config::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::{BidResponse, Bid}};

pub struct PubmaticBidder;

impl Bidder for PubmaticBidder {
    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
    ) -> Result<Vec<BidderHttpRequest>, Vec<BidderError>> {
        // Pubmatic forwards the OpenRTB request shape mostly unchanged; the
        // `publisherId` macro lives on the URL, not the body.
        let body = serde_json::to_vec(request)
            .map_err(|e| vec![BidderError::failed_to_request_bids(e.to_string())])?;
        // Round-23 review High 1: route through EndpointBuilder, same
        // rationale as AppNexus above.
        let url = build_outbound_url(info)?;
        let covered_imp_ids = request.imp.iter().map(|i| i.id.clone()).collect();
        Ok(vec![BidderHttpRequest {
            method: "POST".into(),
            url,
            headers: vec![
                ("Content-Type".into(), "application/json".into()),
                ("Accept".into(),       "application/json".into()),
            ],
            body,
            covered_imp_ids,
        }])
    }

    fn make_bids(
        &self,
        _request: &BidRequest,
        _http_request: &BidderHttpRequest,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        if response.status >= 400 {
            return Err(vec![BidderError::bad_server_response(
                format!("status={}", response.status))]);
        }
        if response.body.is_empty() {
            return Ok(BidderResponse { bids: vec![], currency: "USD".into() });
        }
        let parsed: BidResponse = serde_json::from_slice(&response.body)
            .map_err(|e| vec![BidderError::failed_to_unmarshal(e.to_string())])?;
        let mut bids = Vec::new();
        for seat in &parsed.seatbid {
            for b in &seat.bid {
                bids.push(TypedBid {
                    media_type: infer_media_type(b),
                    bid: b.clone(),
                });
            }
        }
        let currency = if parsed.cur.is_empty() { "USD".into() } else { parsed.cur.clone() };
        Ok(BidderResponse { bids, currency })
    }
}

fn infer_media_type(b: &Bid) -> MediaType {
    // Round-15 review Blocker stale typed-API + round-22 review
    // Blocker 3: Bid.ext is `Option<ExtBid>` (typed per foundations), but the
    // Pubmatic wire shape places the media-type hint in any of THREE
    // slots — the typed `ext.prebid.bid_type` field (modern), the
    // catch-all `ext.prebid.other["mediatype"]` (upstream Java convention
    // shipped via the "mediatype" key), and the legacy `ext.other.video`
    // / `ext.other.native` / `ext.other.audio` blocks (older Pubmatic
    // builds). Probe all three so the tests below pass.
    let Some(ext) = b.ext.as_ref() else {
        return MediaType::Banner;
    };
    if let Some(prebid) = ext.prebid.as_ref() {
        // (a) typed `bid_type` field.
        if let Some(mt) = prebid.bid_type.as_deref() {
            return parse_media_type_str(mt);
        }
        // (b) catch-all `ext.prebid.mediatype` string.
        if let Some(mt) = prebid.other.get("mediatype").and_then(|v| v.as_str()) {
            return parse_media_type_str(mt);
        }
    }
    // (c) legacy `ext.other.video` / `.native` / `.audio` blocks.
    for (key, media_type) in [
        ("video",  MediaType::Video),
        ("native", MediaType::Native),
        ("audio",  MediaType::Audio),
    ] {
        if ext.other.get(key).is_some() {
            return media_type;
        }
    }
    MediaType::Banner
}

fn parse_media_type_str(name: &str) -> MediaType {
    match name {
        "video"  => MediaType::Video,
        "native" => MediaType::Native,
        "audio"  => MediaType::Audio,
        _        => MediaType::Banner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_request() -> BidderHttpRequest {
        BidderHttpRequest {
            method: "POST".into(),
            url: "https://hb.pubmatic.com/translator".into(),
            headers: vec![],
            body: vec![],
            covered_imp_ids: vec![],
        }
    }

    #[test]
    fn infers_video_from_ext_video_block() {
        let body = br#"{"id":"r","seatbid":[{"seat":"pubmatic","bid":[{"id":"b1","impid":"i1","price":2.0,"ext":{"video":{"duration":15}}}]}],"cur":"USD"}"#;
        let resp = BidderHttpResponse { status: 200, headers: vec![], body: body.to_vec() };
        let out = PubmaticBidder.make_bids(&BidRequest::default(), &http_request(), &resp).unwrap();
        assert_eq!(out.bids[0].media_type, MediaType::Video);
    }

    #[test]
    fn infers_native_from_ext_prebid_mediatype() {
        let body = br#"{"id":"r","seatbid":[{"seat":"pubmatic","bid":[{"id":"b1","impid":"i1","price":2.0,"ext":{"prebid":{"mediatype":"native"}}}]}],"cur":"USD"}"#;
        let resp = BidderHttpResponse { status: 200, headers: vec![], body: body.to_vec() };
        let out = PubmaticBidder.make_bids(&BidRequest::default(), &http_request(), &resp).unwrap();
        assert_eq!(out.bids[0].media_type, MediaType::Native);
    }

    #[test]
    fn defaults_to_banner_when_no_hint() {
        let body = br#"{"id":"r","seatbid":[{"seat":"pubmatic","bid":[{"id":"b1","impid":"i1","price":1.0}]}],"cur":"USD"}"#;
        let resp = BidderHttpResponse { status: 200, headers: vec![], body: body.to_vec() };
        let out = PubmaticBidder.make_bids(&BidRequest::default(), &http_request(), &resp).unwrap();
        assert_eq!(out.bids[0].media_type, MediaType::Banner);
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-bidder adapters::pubmatic::tests
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/src/adapters/pubmatic.rs
git commit -m "auctioneer-bidder: PubmaticBidder (custom response transform)"
```

---

### Task 18: `MocktioneerBidder` (feature-gated)

**Files:**

- Modify: `crates/auctioneer-bidder/src/adapters/mocktioneer.rs`

**Interfaces:**

- Consumes: `BidRequest`, `BidderRequestInfo`.
- Produces: `#[cfg(feature = "insecure-bidders")] pub struct MocktioneerBidder`. POSTs the request body verbatim to `http://127.0.0.1:8787/openrtb2/auction` (the template), expects deterministic-pricing JSON response.

- [ ] **Step 1: Replace `src/adapters/mocktioneer.rs`**

```rust
//! Mocktioneer adapter — snapshot-file E2E backend. See spec §4.3 row 4.
//!
//! **Feature-gated.** Mocktioneer points at `http://127.0.0.1` which fails
//! `validate_endpoint`'s `https://`-only + localhost-reject rule in default
//! builds (§5.4). The relaxation lives behind `#[cfg(feature =
//! "insecure-bidders")]`; in default builds this module does not exist.

use crate::{
    bidder::{Bidder, BidderError},
    request_info::BidderRequestInfo,
    types::{BidderHttpRequest, BidderHttpResponse, BidderResponse, TypedBid},
};
use auctioneer_config::MediaType;
use auctioneer_openrtb::types::{bid_request::BidRequest, bid_response::BidResponse};

pub struct MocktioneerBidder;

impl Bidder for MocktioneerBidder {
    fn make_requests(
        &self,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
    ) -> Result<Vec<BidderHttpRequest>, Vec<BidderError>> {
        let body = serde_json::to_vec(request)
            .map_err(|e| vec![BidderError::failed_to_request_bids(e.to_string())])?;
        let covered_imp_ids = request.imp.iter().map(|i| i.id.clone()).collect();
        Ok(vec![BidderHttpRequest {
            method: "POST".into(),
            url: info.endpoint_template.into(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body,
            covered_imp_ids,
        }])
    }

    fn make_bids(
        &self,
        _request: &BidRequest,
        _http_request: &BidderHttpRequest,
        response: &BidderHttpResponse,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        if response.status >= 400 {
            return Err(vec![BidderError::bad_server_response(
                format!("status={}", response.status))]);
        }
        if response.body.is_empty() {
            return Ok(BidderResponse { bids: vec![], currency: "USD".into() });
        }
        let parsed: BidResponse = serde_json::from_slice(&response.body)
            .map_err(|e| vec![BidderError::failed_to_unmarshal(e.to_string())])?;
        let mut bids = Vec::new();
        for seat in &parsed.seatbid {
            for b in &seat.bid {
                bids.push(TypedBid { bid: b.clone(), media_type: MediaType::Banner });
            }
        }
        let currency = if parsed.cur.is_empty() { "USD".into() } else { parsed.cur.clone() };
        Ok(BidderResponse { bids, currency })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bidder::EntryPoint, types::BidderName};
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};

    #[test]
    fn make_requests_targets_127_0_0_1_template() {
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![Imp { id: "i-1".into(), ..Imp::default() }],
            ..BidRequest::default()
        };
        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let info = BidderRequestInfo {
            bidder: BidderName::new("mocktioneer"),
            core_name: BidderName::new("mocktioneer"),
            account_id: "acct-x",
            publisher_id: None,
            endpoint_template: "http://127.0.0.1:8787/openrtb2/auction",
            endpoint_macros: macros,
            host_allowlist: None,
            gpc: None,
            entry_point: EntryPoint::Auction,
            currency,
        };
        let out = MocktioneerBidder.make_requests(&req, &info).unwrap();
        assert_eq!(out[0].url, "http://127.0.0.1:8787/openrtb2/auction");
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-bidder --features insecure-bidders adapters::mocktioneer::tests
```

Expected: 1 passed (under the feature). Without the feature the module isn't compiled.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/src/adapters/mocktioneer.rs
git commit -m "auctioneer-bidder: MocktioneerBidder (insecure-bidders feature)"
```

---

### Task 18b: `auctioneer-config` — wire `bidder_timeout_ms` codegen (round-15 review High)

Round-12 review Medium 11 + the preamble revisions table promised `BidderInfo.bidder_timeout_ms: Option<u32>` and Task 31's call-list construction reads it; no concrete task wired the YAML field through codegen. Without this, the field doesn't exist on `BidderInfo` and Task 31 fails to compile.

**Files:**

- Modify: `crates/auctioneer-config/src/types.rs`
- Modify: `crates/auctioneer-config/build.rs`
- Modify: `crates/auctioneer-config/tests/url_safety.rs`
- Modify: `crates/auctioneer-config/tests/build_failure_harness.rs`
- Modify: `crates/auctioneer-config/data/bidder-info/generic.yaml` (and the other three reference YAMLs — leave the field unset to preserve the fall-back to `bidder_visible_tmax_ms`)

**Interfaces:**

- Consumes: optional `bidderTimeoutMs` YAML field on every bidder-info entry.
- Produces: `pub bidder_timeout_ms: Option<u32>` on `BidderInfo`; codegen emits `Some(N)` when the YAML carries the field, else `None`.

- [ ] **Step 1: Extend `BidderInfo`**

Edit `crates/auctioneer-config/src/types.rs`:

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BidderInfo {
    // … existing fields …
    pub host_allowlist: Option<Vec<String>>,
    /// Round-12 review Medium 11 / round-15 review High bidder_timeout_ms:
    /// per-bidder timeout in milliseconds, sourced from the YAML
    /// `bidderTimeoutMs` field. `None` means the runtime falls back to
    /// the auction-wide `bidder_visible_tmax_ms`. Spec §5.3 line 1202
    /// names bidder_timeout as per-bidder BidderInfo data.
    pub bidder_timeout_ms: Option<u32>,
    pub endpoint_macros: Vec<String>,
    // … existing fields …
}
```

- [ ] **Step 2: Extend the YAML deserializer in `build.rs`**

```rust
#[derive(Debug, Deserialize)]
struct BidderInfoYaml {
    // … existing fields …
    #[serde(default, rename = "bidderTimeoutMs")]
    bidder_timeout_ms: Option<u32>,
    // … existing fields …
}
```

- [ ] **Step 3: Render in `emit_bidders`**

```rust
let timeout = match info.bidder_timeout_ms {
    Some(ms) => format!("Some({ms}_u32)"),
    None     => "None".to_owned(),
};
write!(
    out,
    "        BidderInfo {{\n\
    // … existing renders …\n\
    \x20            bidder_timeout_ms: {timeout},\n\
    // … existing renders …\n\
    \x20        }},\n",
)?;
```

- [ ] **Step 4: Codegen-time positivity check + harness fixture**

Reject `bidderTimeoutMs: 0` (a zero per-bidder timeout means the bidder is unreachable; it's a config bug, not a usable value). Mirror the `validate_host_resource_limits` pattern from Task 23.

```rust
fn validate_bidder_timeout(name: &str, info: &BidderInfoYaml) -> BuildResult {
    if let Some(0) = info.bidder_timeout_ms {
        return Err(format!(
            "bidder {name:?} bidderTimeoutMs = 0; must be omitted (fall-back) \
             or > 0. Spec §5.3 line 1202."
        ).into());
    }
    Ok(())
}
```

Add a `build_failure_harness` fixture with `bidderTimeoutMs: 0` and assert the child cargo build fails with the message above.

- [ ] **Step 5: Verify the typed accessor lights up the existing harness suite**

```
cargo test -p auctioneer-config --test url_safety
cargo test -p auctioneer-config --test build_failure_harness
```

Expected: prior passing counts plus the new zero-rejection test.

- [ ] **Step 6: Commit**

```
git add crates/auctioneer-config
git commit -m "auctioneer-config: bidder_timeout_ms codegen (per-bidder timeout per spec §5.3)"
```

---

### Task 18c: `auctioneer-config` — account targeting fields + scoped HostConfig.aliases (round-16 review High 6 + 7)

Run_auction reads `ctx.account.config.truncate_target_attribute` and the targeting block falls back to `"hb"`, but foundations `AccountConfig` has only `disabled`, `debug_allow`, `bid_rounding`. Spec §5.6 / Appendix E.1–E.4 (lines around 3603 and 5234) require account-level `targeting_prefix` + `truncate_target_attribute`. Add them.

`HostConfig.aliases` is referenced by `/info/bidders` (Task 39), but foundations `HostConfig` has no such field. Round-16 decision: **add a minimal `HostConfig.aliases: BTreeMap<String, String>` codegen field** so the `/info/bidders` surface compiles and tests can ship an empty map. Populating non-empty host-config aliases is the GDPR enforcement config_store task; the auction core-shipped `host-defaults.yaml` ships an empty `aliases:` block so existing tests stay unchanged.

**Files:**

- Modify: `crates/auctioneer-config/src/types.rs`
- Modify: `crates/auctioneer-config/build.rs`
- Modify: `crates/auctioneer-config/data/account-defaults.yaml`
- Modify: `crates/auctioneer-config/data/host-defaults.yaml`

- [ ] **Step 1: Extend `AccountConfig`**

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountConfig {
    pub disabled: bool,
    pub debug_allow: bool,
    pub bid_rounding: String,
    /// Round-16 review High 6: account-level targeting prefix per
    /// spec §5.6 / Appendix E.1. Empty string means "fall back to the
    /// hard-coded \"hb\" default at the build_keys call site".
    pub targeting_prefix: String,
    /// Round-16 review High 6: account-level targeting-key length cap
    /// per spec Appendix E.3 line 5234. None = no truncation requested.
    pub truncate_target_attribute: Option<usize>,
}
```

- [ ] **Step 2: Extend `HostConfig`**

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HostConfig {
    pub gdpr: GdprConfig,
    pub ccpa: CcpaConfig,
    pub lmt: LmtConfig,
    pub host_cookie: HostCookieConfig,
    pub status_response: String,
    pub resource_limits: ResourceLimits,
    /// Round-16 review High 7: host-config alias map per spec §9.1.
    /// Phase 1's `host-defaults.yaml` ships an empty map; populating
    /// it is the GDPR enforcement config_store task. /info/bidders reads this field.
    pub aliases: BTreeMap<String, String>,
}
```

- [ ] **Step 3: Update `build.rs`**

Add the deserializer fields + render path for both structs. Mirror the existing pattern:

```rust
#[derive(Debug, Deserialize)]
struct AccountYaml {
    disabled: bool,
    debug_allow: bool,
    bid_rounding: String,
    #[serde(default)]
    targeting_prefix: String,
    #[serde(default)]
    truncate_target_attribute: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct HostDefaultYaml {
    // … existing fields …
    #[serde(default)]
    aliases: BTreeMap<String, String>,
}
```

In `emit_account_default` + `emit_host_default`, render the new fields. The render for `aliases` is a `BTreeMap::from([(\"name\".into(), \"core\".into())])` literal; for an empty map use `BTreeMap::new()`.

- [ ] **Step 4: Update embedded YAML**

`account-defaults.yaml`:

```yaml
disabled: false
debug_allow: true
bid_rounding: down
targeting_prefix: hb
truncate_target_attribute: null
```

`host-defaults.yaml`:

```yaml
# … existing fields …
aliases: {}
```

- [ ] **Step 5: Wire `build_keys` fallback through the account prefix**

In Task 31's targeting block, the prefix fallback now reads:

```rust
let prefix = targeting_cfg
    .and_then(|t| t.prefix.as_deref())
    .or_else(|| {
        let p = ctx.account.config.targeting_prefix.as_str();
        if p.is_empty() { None } else { Some(p) }
    })
    .unwrap_or("hb");
```

(Request-block prefix wins over account-block; account-block falls back to the hard-coded `"hb"` per Appendix E.1.)

- [ ] **Step 6: Tests**

- `account_targeting_prefix_used_when_request_omits_it`
- `request_prefix_overrides_account_prefix`
- `host_defaults_aliases_is_empty_in_phase_1`

- [ ] **Step 7: Commit**

```
git add crates/auctioneer-config
git commit -m "auctioneer-config: AccountConfig targeting fields + HostConfig.aliases scaffolding"
```

---

### Task 19: `auctioneer-exchange` crate skeleton

**Files:**

- Create: `crates/auctioneer-exchange/Cargo.toml`
- Create: `crates/auctioneer-exchange/src/lib.rs`
- Create empty stubs under `src/` for each module listed in the file structure.

**Interfaces:**

- Consumes: every foundations + auction core library crate.
- Produces: `cargo check -p auctioneer-exchange` succeeds against empty modules.

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "auctioneer-exchange"
version = "0.1.0"
edition = "2021"
publish = false

[lints]
workspace = true

[features]
insecure-bidders = ["auctioneer-bidder/insecure-bidders"]

[dependencies]
async-trait    = { workspace = true }
auctioneer-bidder  = { workspace = true }
auctioneer-config  = { workspace = true }
auctioneer-openrtb = { workspace = true }
auctioneer-privacy = { workspace = true }
auctioneer-runtime = { workspace = true }
serde          = { workspace = true }
serde_json     = { workspace = true }
# Round-22 review Blocker 2: production code uses url::Url for the
# §5.4 SSRF gate (Task 22), and run_auction's elapsed-time math reads
# `time::OffsetDateTime` from the Clock trait. `url` and `time` must
# appear at task creation, not when downstream tasks first import them.
time           = { workspace = true }   # round-22 Blocker 2
url            = { workspace = true }   # round-22 Blocker 2

[dev-dependencies]
# Round-22 review Blocker 2: every async-test driver in this crate
# uses futures::executor::block_on (round-12 review High 9). Adding
# the dev-dep now means the tests don't need a follow-up patch.
futures        = { workspace = true }   # round-22 Blocker 2
```

- [ ] **Step 2: Create `src/lib.rs` with empty placeholder**

```rust
//! Auction engine. EdgeZero-free. See spec §5.
```

- [ ] **Step 3: Create empty module files**

For each path under `src/`:

- `error.rs`, `fanout.rs`, `account.rs`, `context.rs`, `validate_endpoint.rs`, `tmax.rs`, `registry_order.rs`, `alias.rs`, `split.rs`, `limits.rs`, `run.rs`, `responsetime.rs`, `response.rs`
- and the `targeting/` submodule with `mod.rs`, `price_bucket.rs`, `keys.rs`, `winner.rs`

create the file with the single line `// stub — implemented in next task`.

- [ ] **Step 4: Verify**

```
cargo check -p auctioneer-exchange
```

Expected: success, zero warnings.

- [ ] **Step 5: Commit**

```
git add Cargo.toml crates/auctioneer-exchange
git commit -m "auctioneer-exchange: crate skeleton"
```

---

### Task 20: `AuctionError` + `AuctionDeadline` + neutral fan-out types

**Files:**

- Modify: `crates/auctioneer-exchange/src/error.rs`
- Modify: `crates/auctioneer-exchange/src/fanout.rs`
- Modify: `crates/auctioneer-exchange/src/lib.rs`

**Interfaces:**

- Consumes: `auctioneer-bidder::{BidderHttpRequest, BidderHttpResponse, BidderError}`, `auctioneer-runtime::Clock`.
- Produces: `pub enum AuctionError { BadInput(String), InvalidImpFirstPartyData(String), FanoutUnavailable, Internal(String) }`; `pub struct BidderCall { request, timeout, max_response_bytes, endpoint_compression }`; `pub struct BidderCallResult { result, elapsed }`; `pub enum FanoutError { Timeout, Unreachable, BadServerResponse, OversizeResponse, Other(String) }`; `pub enum FanoutInternalError { NoOutboundCapability, Internal(String) }`; `pub struct AuctionDeadline { deadline_at, source }`; `pub enum TmaxSource { Request, HostDefault, CappedByHost }`; `pub enum Compression { Gzip }`; `#[async_trait(?Send)] pub trait BidderFanout`.

- [ ] **Step 1: Implement `src/error.rs`**

```rust
//! Engine-domain `AuctionError`. See spec §5.5.

#[derive(Debug)]
pub enum AuctionError {
    /// Registry-dependent validation failure (alias unknown / no-op /
    /// `aliasgvlids` shape / imp-no-bidders after disabled-bidder removal).
    /// Caller maps to `400 Invalid request: <err>` (Appendix F).
    BadInput(String),
    /// Bidder-scoped FPD merge produces an invalid `Imp` (§5.1 step 7).
    /// Caller maps to `500` (Appendix G code 15).
    InvalidImpFirstPartyData(String),
    /// Fan-out itself could not run (no outbound HTTP). Caller maps to `500`.
    FanoutUnavailable,
    /// Genuine internal fault. Caller maps to `5xx`.
    Internal(String),
}

impl std::fmt::Display for AuctionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadInput(m) => write!(f, "BadInput: {m}"),
            Self::InvalidImpFirstPartyData(m) => write!(f, "InvalidImpFirstPartyData: {m}"),
            Self::FanoutUnavailable => f.write_str("FanoutUnavailable"),
            Self::Internal(m) => write!(f, "Internal: {m}"),
        }
    }
}

impl std::error::Error for AuctionError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_carries_message() {
        let e = AuctionError::BadInput("imp foo has no valid bidders".into());
        assert!(e.to_string().contains("imp foo has no valid bidders"));
    }
}
```

- [ ] **Step 2: Implement `src/fanout.rs`**

```rust
//! `BidderFanout` trait + neutral types. See spec §5.2.

use async_trait::async_trait;
use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression { Gzip }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TmaxSource { Request, HostDefault, CappedByHost }

/// Absolute monotonic instant the whole auction must finish by, plus
/// source of the underlying budget.
#[derive(Clone, Copy, Debug)]
pub struct AuctionDeadline {
    pub deadline_at: Duration,
    pub source: TmaxSource,
}

pub struct BidderCall {
    pub request: BidderHttpRequest,
    pub timeout: Duration,
    pub max_response_bytes: usize,
    pub endpoint_compression: Option<Compression>,
}

#[derive(Debug)]
pub struct BidderCallResult {
    pub result: Result<BidderHttpResponse, FanoutError>,
    pub elapsed: Duration,
}

#[derive(Clone, Debug)]
pub enum FanoutError {
    Timeout,
    Unreachable,
    BadServerResponse,
    OversizeResponse,
    Other(String),
}

#[derive(Debug)]
pub enum FanoutInternalError {
    NoOutboundCapability,
    Internal(String),
}

/// Async fan-out seam. `&dyn BidderFanout` is held by the exchange — so
/// the trait carries `async-trait(?Send)` per spec §5.2; native `async fn
/// in trait` is not object-safe.
#[async_trait(?Send)]
pub trait BidderFanout: Send + Sync {
    /// Contract:
    /// - `Ok(results)` ⇒ `results.len() == calls.len()` and `results[i]`
    ///   corresponds positionally to `calls[i]` (no reordering).
    /// - `Err(_)` ⇒ no per-call results are produced; the exchange maps
    ///   to `AuctionError::FanoutUnavailable`.
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_is_object_safe() {
        fn _take(_b: &dyn BidderFanout) {}
    }
}
```

- [ ] **Step 3: Wire `lib.rs`**

```rust
//! Auction engine. EdgeZero-free. See spec §5.
mod error;
mod fanout;

pub use error::AuctionError;
pub use fanout::{
    AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, Compression,
    FanoutError, FanoutInternalError, TmaxSource,
};
```

- [ ] **Step 4: Run**

```
cargo test -p auctioneer-exchange
```

Expected: 2 passed (error display + trait object-safety).

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: AuctionError + BidderFanout trait + neutral types"
```

---

### Task 21: `ResolvedAccount` + `AuctionContext`

**Files:**

- Modify: `crates/auctioneer-exchange/src/account.rs`
- Modify: `crates/auctioneer-exchange/src/context.rs`
- Modify: `crates/auctioneer-exchange/src/lib.rs`

**Interfaces:**

- Consumes: `auctioneer-config::{HostConfig, AccountConfig}`, `auctioneer-runtime::{Clock, Rng}`, `auctioneer-bidder::BidderRegistry`, `auctioneer-privacy::PrivacyContext`, `BidderFanout`.
- Produces: `pub struct ResolvedAccount<'a> { id, source, config }`; `pub enum AccountSource { Provided, DefaultFallback }`; `#[non_exhaustive] pub struct AuctionContext<'a> { … }` with `with_request_start(...)` constructor (§5.8).

- [ ] **Step 1: Implement `src/account.rs`**

```rust
//! Caller-resolved account identity. See spec §5.8.

use auctioneer_config::AccountConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountSource { Provided, DefaultFallback }

pub struct ResolvedAccount<'a> {
    /// Preserves the request-supplied id (or "unknown" if empty/absent)
    /// even on `DefaultFallback` — used for debug/log attribution.
    pub id: String,
    pub source: AccountSource,
    /// Embedded default-account when `source == DefaultFallback`.
    pub config: &'a AccountConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_config::AccountConfig;

    #[test]
    fn carries_id_through_default_fallback() {
        let cfg = AccountConfig::default();
        let acc = ResolvedAccount {
            id: "unknown".into(),
            source: AccountSource::DefaultFallback,
            config: &cfg,
        };
        assert_eq!(acc.id, "unknown");
    }
}
```

- [ ] **Step 2: Implement `src/context.rs`**

```rust
//! `AuctionContext`. See spec §5.8.

use crate::{account::ResolvedAccount, fanout::BidderFanout};
use auctioneer_bidder::BidderRegistry;
use auctioneer_config::HostConfig;
use auctioneer_runtime::{Clock, Rng};
use std::time::Duration;

/// Glue passed to `run_auction`. `#[non_exhaustive]` means external
/// callers MUST use `with_request_start` (which enforces the
/// `request_start` anchor invariant). In-crate tests construct via field
/// literals directly.
#[non_exhaustive]
pub struct AuctionContext<'a> {
    pub request_start: Duration,
    pub host: &'a HostConfig,
    pub account: &'a ResolvedAccount<'a>,
    pub bidders: &'a BidderRegistry,
    pub fanout: &'a dyn BidderFanout,
    pub clock: &'a dyn Clock,
    pub rng: &'a dyn Rng,
    /// Caller-computed union of `req.test==1`, `ext.prebid.debug==1`,
    /// and any caller-side flag (`?debug=1`, header). Suppressed in
    /// auction core through user sync (§5.6) — `run_auction` ignores this flag during the
    /// debug-suppression window.
    pub debug_requested: bool,
}

impl<'a> AuctionContext<'a> {
    #[expect(clippy::too_many_arguments, reason = "explicit constructor enforces request_start anchor (§5.8)")]
    pub fn with_request_start(
        request_start: Duration,
        host: &'a HostConfig,
        account: &'a ResolvedAccount<'a>,
        bidders: &'a BidderRegistry,
        fanout: &'a dyn BidderFanout,
        clock: &'a dyn Clock,
        rng: &'a dyn Rng,
        debug_requested: bool,
    ) -> Self {
        Self {
            request_start, host, account, bidders, fanout, clock, rng,
            debug_requested,
        }
    }
}
```

- [ ] **Step 3: Re-export**

Update `src/lib.rs`:

```rust
mod account;
mod context;

pub use account::{AccountSource, ResolvedAccount};
pub use context::AuctionContext;
```

(Keep existing `mod error;` / `mod fanout;` etc.)

- [ ] **Step 4: Test**

```
cargo test -p auctioneer-exchange
```

Expected: 3 passed.

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: ResolvedAccount + AuctionContext"
```

---

### Task 22: `validate_endpoint` — full §5.4 ruleset

**Files:**

- Modify: `crates/auctioneer-exchange/src/validate_endpoint.rs`
- Create: `crates/auctioneer-exchange/tests/validate_endpoint.rs`

**Interfaces:**

- Consumes: a final outbound URL string + `&BidderInfo`.
- Produces: `pub fn validate_endpoint(url: &str, info: &BidderInfo) -> Result<(), BidderError>`. Default builds enforce HTTPS-only + reject private/loopback/link-local IP literals + reject `localhost`/aliases. Under `insecure-bidders` the rejection narrows to the `127.0.0.1`/`[::1]`/`localhost` allowlist. Fixed-host bidders require host equality; host-varying bidders require `host_allowlist` membership.

- [ ] **Step 1: Write the failing test suite in `tests/validate_endpoint.rs`**

```rust
use auctioneer_config::BidderInfo;
use auctioneer_exchange::validate_endpoint;

fn info_fixed_host_https() -> BidderInfo {
    BidderInfo {
        name: "ax".into(),
        endpoint: "https://hb.example.test/openrtb".into(),
        uses_https: true,
        host_allowlist: None,
        ..BidderInfo::default()
    }
}

fn info_host_varying() -> BidderInfo {
    BidderInfo {
        name: "ax".into(),
        endpoint: "https://{{.Host}}/openrtb".into(),
        uses_https: true,
        host_allowlist: Some(vec!["eu.example.test".into(), "us.example.test".into()]),
        ..BidderInfo::default()
    }
}

#[test]
fn fixed_host_match_accepted() {
    let info = info_fixed_host_https();
    validate_endpoint("https://hb.example.test/openrtb?acct=42", &info).unwrap();
}

#[test]
fn fixed_host_mismatch_rejected() {
    let info = info_fixed_host_https();
    let err = validate_endpoint("https://rogue.example.test/openrtb", &info).unwrap_err();
    assert_eq!(err.code, 5);
}

#[test]
fn host_varying_member_accepted() {
    let info = info_host_varying();
    validate_endpoint("https://us.example.test/openrtb", &info).unwrap();
}

#[test]
fn host_varying_off_allowlist_rejected() {
    let info = info_host_varying();
    let err = validate_endpoint("https://rogue.example.test/openrtb", &info).unwrap_err();
    assert_eq!(err.code, 5);
}

#[test]
fn https_only_in_default_build() {
    let info = info_fixed_host_https();
    let err = validate_endpoint("http://hb.example.test/openrtb", &info).unwrap_err();
    assert_eq!(err.code, 5);
}

#[test]
fn scheme_mismatch_rejected_against_configured_endpoint() {
    // Round-14 review High: under --features insecure-bidders the
    // scheme check (http allowed) passes the outer test, but the
    // outbound scheme still must equal BidderInfo.endpoint's scheme.
    // An adapter that downgrades https→http MUST be rejected.
    let info = info_fixed_host_https();
    let err = validate_endpoint("http://hb.example.test/openrtb", &info).unwrap_err();
    // Code is FailedToRequestBids (5) per Appendix G; message names
    // the scheme mismatch verbatim.
    assert_eq!(err.code, 5);
    assert!(err.message.contains("scheme") && err.message.contains("does not match"));
}

#[test]
fn fixed_host_rejects_userinfo() {
    // Round-13 review Blocker 3: userinfo must not pass the
    // authority check. `parsed.username().is_empty()` short-circuit
    // catches this BEFORE host comparison.
    let info = info_fixed_host_https();
    let err = validate_endpoint("https://attacker@hb.example.test/openrtb", &info).unwrap_err();
    assert_eq!(err.code, 5);
    assert!(err.message.contains("userinfo"));
}

#[test]
fn fixed_host_rejects_port_mismatch() {
    // Round-13 review Blocker 3: full-authority comparison includes
    // port. The previous port-stripping pre-comparison let this slip
    // through.
    let info = info_fixed_host_https();
    let err = validate_endpoint("https://hb.example.test:444/openrtb", &info).unwrap_err();
    assert_eq!(err.code, 5);
    assert!(err.message.contains("authority"));
}

#[test]
fn private_ip_literals_rejected() {
    let info = BidderInfo {
        name: "ax".into(),
        endpoint: "https://10.0.0.1/x".into(),
        uses_https: true,
        host_allowlist: None,
        ..BidderInfo::default()
    };
    let err = validate_endpoint("https://10.0.0.1/x", &info).unwrap_err();
    assert_eq!(err.code, 5);
}

#[test]
fn localhost_literal_rejected_in_default_build() {
    let info = BidderInfo {
        name: "ax".into(),
        endpoint: "https://localhost/x".into(),
        uses_https: true,
        host_allowlist: None,
        ..BidderInfo::default()
    };
    let err = validate_endpoint("https://localhost/x", &info).unwrap_err();
    assert_eq!(err.code, 5);
}

#[test]
fn ipv6_loopback_rejected_in_default_build() {
    let info = BidderInfo {
        name: "ax".into(),
        endpoint: "https://[::1]/x".into(),
        uses_https: true,
        host_allowlist: None,
        ..BidderInfo::default()
    };
    let err = validate_endpoint("https://[::1]/x", &info).unwrap_err();
    assert_eq!(err.code, 5);
}

#[test]
fn ipv6_reserved_literals_rejected_in_default_build() {
    // Round-24 review High 1: the previous `is_private_ip_literal`
    // body parsed IPv6 only after `host.strip_prefix('[')`, but
    // `url::Url::host_str()` already returns the host without
    // brackets — so `fc00::1`, `fe80::1`, `::`, `::ffff:10.0.0.1`,
    // `2001:db8::1` all slipped past. Each of these MUST be rejected
    // with code 5 in default builds.
    for literal in [
        "fc00::1",                  // unique-local
        "fd12:3456:789a::1",        // unique-local
        "fe80::1",                  // link-local
        "::",                       // unspecified
        "ff02::1",                  // multicast
        "2001:db8::1",              // documentation
        "::ffff:10.0.0.1",          // IPv4-mapped private
    ] {
        let url = format!("https://[{literal}]/x");
        let info = BidderInfo {
            name: "ax".into(),
            endpoint: url.clone(),
            uses_https: true,
            host_allowlist: None,
            ..BidderInfo::default()
        };
        let err = validate_endpoint(&url, &info).expect_err(
            "expected reject for IPv6 reserved literal");
        assert_eq!(err.code, 5, "literal {literal}");
    }
}

#[test]
fn ipv4_reserved_literals_rejected_in_default_build() {
    // Round-24 review High 1: broader IPv4 reserved coverage. The
    // earlier helper only matched a handful of RFC 1918 / loopback
    // / link-local ranges; the spec's "private/reserved" set is the
    // full IANA reserved list.
    for literal in [
        "0.0.0.1",          // 0.0.0.0/8 "this network"
        "100.64.0.1",       // 100.64.0.0/10 CGN (RFC 6598)
        "192.0.2.1",        // documentation TEST-NET-1
        "198.18.0.1",       // benchmarking (RFC 2544)
        "198.51.100.1",     // documentation TEST-NET-2
        "203.0.113.1",      // documentation TEST-NET-3
        "240.0.0.1",        // 240.0.0.0/4 reserved
        "255.255.255.255",  // limited broadcast
    ] {
        let url = format!("https://{literal}/x");
        let info = BidderInfo {
            name: "ax".into(),
            endpoint: url.clone(),
            uses_https: true,
            host_allowlist: None,
            ..BidderInfo::default()
        };
        let err = validate_endpoint(&url, &info).expect_err(
            "expected reject for IPv4 reserved literal");
        assert_eq!(err.code, 5, "literal {literal}");
    }
}

#[cfg(feature = "insecure-bidders")]
#[test]
fn loopback_allowed_under_feature() {
    let info = BidderInfo {
        name: "mocktioneer".into(),
        endpoint: "http://127.0.0.1:8787/openrtb2/auction".into(),
        uses_https: false,
        host_allowlist: None,
        ..BidderInfo::default()
    };
    validate_endpoint("http://127.0.0.1:8787/openrtb2/auction", &info).unwrap();
}
```

- [ ] **Step 2: Implement `src/validate_endpoint.rs`**

```rust
//! `validate_endpoint` — full §5.4 ruleset.

use auctioneer_bidder::BidderError;
use auctioneer_config::BidderInfo;

// Round-22 review High 5: dropped the unused `PRIVATE_HOSTNAMES`
// constant. The hardened `validate_endpoint` body below uses the
// shared `is_localhost_alias_host` helper for alias detection
// (round-10 review Medium), so the constant array would clippy-
// trigger `dead_code` on the `restriction = deny` gate.

// Round-12 review Blocker 3: hardened validate_endpoint.
//
// (1) No userinfo / port stripping pre-comparison — `https://a@host/`
//     and `https://host:444/` previously slipped past fixed-host
//     equality. The §5.4 rule is "same scheme, same authority".
// (2) `cfg!(feature = "insecure-bidders")` is replaced with real
//     `#[cfg(feature = "...")]` so the insecure-path code is
//     COMPILED OUT of default builds — the spec requires zero
//     insecure code in production.
pub fn validate_endpoint(url: &str, info: &BidderInfo) -> Result<(), BidderError> {
    let parsed = url::Url::parse(url)
        .map_err(|err| BidderError::failed_to_request_bids(
            format!("invalid URL after macro expansion: {err}")))?;

    // Scheme: must be https. http is allowed ONLY when the
    // insecure-bidders feature is compiled in.
    match parsed.scheme() {
        "https" => {}
        #[cfg(feature = "insecure-bidders")]
        "http" => {}
        other => return Err(BidderError::failed_to_request_bids(
            format!("scheme {other:?} is not allowed"))),
    }

    // Userinfo is forbidden everywhere — block before the authority
    // comparison so userinfo cannot mask the host.
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(BidderError::failed_to_request_bids(
            "userinfo is not permitted in bidder URLs".into()));
    }

    let host = parsed.host_str().ok_or_else(|| BidderError::failed_to_request_bids(
        "missing host after macro expansion".into()))?;
    let final_authority = match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None       => host.to_owned(),
    };

    // Round-14 review High validate_endpoint scheme equality: parse
    // the BidderInfo.endpoint regardless of fixed-host vs host-varying
    // so we can compare the OUTBOUND scheme to the CONFIGURED scheme.
    // Spec §5.4 line 1553 requires "same scheme, same authority" for
    // fixed-host bidders, and host-varying bidders cannot promote
    // http→https either (the scheme is fixed at codegen for the bidder
    // template). Without this check, under insecure-bidders an adapter
    // could ship http:// for an https://-only bidder.
    let info_template = info.endpoint.replace("{{.Host}}", "auctioneer-host-stand-in.invalid");
    let info_url = url::Url::parse(&info_template)
        .map_err(|_| BidderError::failed_to_request_bids(
            "BidderInfo.endpoint is malformed (codegen bug)".into()))?;
    if !parsed.scheme().eq_ignore_ascii_case(info_url.scheme()) {
        return Err(BidderError::failed_to_request_bids(
            format!("scheme {:?} does not match BidderInfo.endpoint scheme {:?}",
                    parsed.scheme(), info_url.scheme())));
    }

    if let Some(allow) = info.host_allowlist.as_deref() {
        // Host-varying: host-only comparison (hostAllowlist is
        // pinned host-only at codegen — round-12 review Medium).
        if !allow.iter().any(|h| host.eq_ignore_ascii_case(h)) {
            return Err(BidderError::failed_to_request_bids(
                format!("host {host:?} is not in host_allowlist")));
        }
    } else {
        // Fixed-host: full authority comparison (host + port).
        // Adapters cannot freelance a different port either.
        let info_authority = match info_url.port() {
            Some(port) => format!(
                "{}:{port}",
                info_url.host_str().unwrap_or_default(),
            ),
            None => info_url.host_str().unwrap_or_default().to_owned(),
        };
        if !final_authority.eq_ignore_ascii_case(&info_authority) {
            return Err(BidderError::failed_to_request_bids(
                format!("authority {final_authority:?} != BidderInfo.endpoint \
                         authority {info_authority:?}")));
        }
    }

    // IP-literal / localhost rejection, with the insecure-bidders
    // allow-list COMPILED IN under the feature only.
    if let Some(err) = loopback_or_alias_is_rejected(host) {
        return Err(err);
    }
    if is_private_ip_literal(host) {
        return Err(BidderError::failed_to_request_bids(
            format!("private IP literal {host:?} not allowed")));
    }
    Ok(())
}

/// Loopback / localhost-alias rejection. Default builds reject
/// every loopback / DNS-trick host. Under the `insecure-bidders`
/// feature, exactly `127.0.0.1`, `::1`, and `localhost` are
/// permitted — every other loopback or alias stays rejected.
#[cfg(not(feature = "insecure-bidders"))]
fn loopback_or_alias_is_rejected(host: &str) -> Option<BidderError> {
    if is_loopback_literal(host) || is_localhost_alias_host(host) {
        return Some(BidderError::failed_to_request_bids(
            format!("loopback / alias host {host:?} is not allowed in default builds")));
    }
    None
}

#[cfg(feature = "insecure-bidders")]
fn loopback_or_alias_is_rejected(host: &str) -> Option<BidderError> {
    const ALLOW: &[&str] = &["127.0.0.1", "::1", "localhost"];
    if ALLOW.iter().any(|a| host.eq_ignore_ascii_case(a)) {
        return None;
    }
    if is_loopback_literal(host) || is_localhost_alias_host(host) {
        return Some(BidderError::failed_to_request_bids(
            format!("non-literal loopback / alias {host:?} stays rejected even under \
                     insecure-bidders")));
    }
    None
}

// Round-12 review: `extract_host` removed — `url::Url::parse`
// gives us the canonical host + port directly. Keeping a
// hand-rolled authority extractor invited the very userinfo /
// port stripping bug Blocker 3 closed.

fn is_loopback_literal(host: &str) -> bool {
    host == "127.0.0.1" || host == "::1" || host.eq_ignore_ascii_case("localhost")
}

fn is_localhost_alias_host(host: &str) -> bool {
    // Same rule as build_support::is_localhost_alias_host
    // (round-10 review). Default builds reject `*.localhost` per
    // RFC 6761 plus the well-known wildcard-DNS services that
    // always resolve to loopback.
    let lower = host.to_ascii_lowercase();
    if lower.ends_with(".localhost") { return true; }
    for apex in &["nip.io", "xip.io", "lvh.me", "localtest.me", "vcap.me"] {
        if lower == *apex { return true; }
        if let Some(prefix) = lower.strip_suffix(*apex) {
            if prefix.ends_with('.') { return true; }
        }
    }
    false
}

// Round-24 review High 1: the previous body had two real holes.
// (a) IPv6 literals from `url::Url::host_str()` are ALREADY
// unbracketed — `[fc00::1]` arrives as `fc00::1`, so the
// `strip_prefix('[')` guard never fired, and IPv6 private/link-local
// /unique-local literals all slipped past. The fix is to attempt
// the IPv6 parse directly on `host`.
// (b) The IPv4 coverage missed the spec's broader reserved ranges:
// CGN (100.64.0.0/10, RFC 6598), benchmarking (198.18.0.0/15,
// RFC 2544), documentation (192.0.2.0/24, 198.51.100.0/24,
// 203.0.113.0/24, RFC 5737), 0.0.0.0/8 (unspecified), 240.0.0.0/4
// (reserved), and 255.255.255.255 (broadcast). The IPv6 coverage
// missed unspecified (`::`), IPv4-mapped (`::ffff:10.0.0.1`),
// documentation (`2001:db8::/32`), and the deprecated IPv4-compat
// space.
fn is_private_ip_literal(host: &str) -> bool {
    if let Ok(v4) = host.parse::<std::net::Ipv4Addr>() {
        return is_reserved_ipv4(v4);
    }
    if let Ok(v6) = host.parse::<std::net::Ipv6Addr>() {
        return is_reserved_ipv6(v6);
    }
    false
}

fn is_reserved_ipv4(v4: std::net::Ipv4Addr) -> bool {
    if v4.is_loopback()                 // 127.0.0.0/8
        || v4.is_private()              // 10/8, 172.16/12, 192.168/16
        || v4.is_link_local()           // 169.254/16
        || v4.is_unspecified()          // 0.0.0.0
        || v4.is_broadcast()            // 255.255.255.255
        || v4.is_documentation()        // 192.0.2/24, 198.51.100/24, 203.0.113/24
        || v4.is_multicast()            // 224/4
    {
        return true;
    }
    let o = v4.octets();
    // 0.0.0.0/8 — "this network" (RFC 791). `is_unspecified()` only
    // catches `0.0.0.0` itself; the rest of the /8 is reserved too.
    if o[0] == 0 { return true; }
    // 100.64.0.0/10 — carrier-grade NAT (RFC 6598).
    if o[0] == 100 && (64..=127).contains(&o[1]) { return true; }
    // 192.0.0.0/24 — IETF protocol assignments (RFC 6890).
    if o[0] == 192 && o[1] == 0 && o[2] == 0 { return true; }
    // 198.18.0.0/15 — benchmarking (RFC 2544).
    if o[0] == 198 && (o[1] == 18 || o[1] == 19) { return true; }
    // 240.0.0.0/4 — reserved (former Class E, RFC 1112). Catches
    // every address with the top nibble = 0xF, including the
    // multicast-adjacent block. `is_broadcast()` already covers
    // 255.255.255.255 but the rest of /4 is also reserved.
    if o[0] >= 240 { return true; }
    false
}

fn is_reserved_ipv6(v6: std::net::Ipv6Addr) -> bool {
    if v6.is_loopback()            // ::1
        || v6.is_unspecified()     // ::
        || v6.is_unique_local()    // fc00::/7
        || v6.is_multicast()       // ff00::/8
    {
        return true;
    }
    let segs = v6.segments();
    // Link-local fe80::/10.
    if segs[0] & 0xffc0 == 0xfe80 { return true; }
    // Documentation 2001:db8::/32 (RFC 3849).
    if segs[0] == 0x2001 && segs[1] == 0x0db8 { return true; }
    // IPv4-mapped ::ffff:0:0/96 — re-validate the embedded v4 so
    // `::ffff:10.0.0.1` and `::ffff:127.0.0.1` are caught.
    if let Some(mapped) = v6.to_ipv4_mapped() {
        return is_reserved_ipv4(mapped);
    }
    // IPv4-compatible ::a.b.c.d (deprecated, RFC 4291 §2.5.5.1) —
    // re-validate the embedded v4 so legacy adapters cannot abuse it.
    if segs[0] == 0 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0
        && segs[4] == 0 && segs[5] == 0
        && !(segs[6] == 0 && segs[7] == 0)
        && !(segs[6] == 0 && segs[7] == 1)
    {
        let bytes = v6.octets();
        let v4 = std::net::Ipv4Addr::new(bytes[12], bytes[13], bytes[14], bytes[15]);
        return is_reserved_ipv4(v4);
    }
    false
}
```

Wire in `lib.rs`:

```rust
mod validate_endpoint;
pub use validate_endpoint::validate_endpoint;
```

- [ ] **Step 3: Run tests**

```
cargo test -p auctioneer-exchange --test validate_endpoint
cargo test -p auctioneer-exchange --features insecure-bidders --test validate_endpoint
```

Expected: first command 8 passed, second command 9 passed (loopback feature test added).

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-exchange/src crates/auctioneer-exchange/tests
git commit -m "auctioneer-exchange: validate_endpoint (§5.4 SSRF gate)"
```

---

### Task 22b: auction core endpoint-macro allow-list codegen guard

**Files:**

- Modify: `crates/auctioneer-config/build.rs`
- Modify: `crates/auctioneer-config/tests/build_failure_harness.rs`

**Interfaces:**

- Consumes: the codegen-emitted `endpoint_macros: Vec<String>` from `build_support::extract_endpoint_macros` (foundations source).
- Produces: a build-time rejection — `cargo build -p auctioneer-config` fails with a clear error if any bidder YAML's endpoint template contains a macro other than the auction core-supported set.

Round-25 review Medium 1 (revised round-26 review High): auction core supports two path/query macros — `{{.AccountID}}` (request-derived) and `{{.PublisherID}}` (raw `site.publisher.id` / `app.publisher.id`, spec line 1331-1333) — plus `{{.Host}}` (handled separately via `host_allowlist`; never appears in `endpoint_macros`). Without a build-time guard, a bidder YAML with an unsupported token (e.g. `{{.GDPR}}` on the endpoint instead of on the syncer URL) compiles successfully — codegen records it in `endpoint_macros` — and only fails at runtime with `EndpointBuilderError::MissingMacro("...")` because no `set_macro` call site exists. That's bad UX (the failure surfaces during the first real auction, not at build) AND a security smell (a typo `{{.AcountID}}` would be silently treated as a different macro instead of being rejected). This task pins the auction core set to `["AccountID", "PublisherID"]` and fails the build for anything else, pointing privacy macros at user sync.

- [ ] **Step 1: Add the auction core allow-list constant + checker in `build.rs`**

Insert near the top of `crates/auctioneer-config/build.rs`, after the existing `BIDDER_SYNCER_ALLOWED_MACROS` import / use site:

```rust
/// Round-25 review Medium 1 + round-26 review High: endpoint-path
/// macro allow-list for auction core. `extract_endpoint_macros` already filters
/// `Host` (it's an authority token, not a path/query fill). Every
/// other token a bidder YAML names in its `endpoint:` field MUST
/// appear in this list, OR the build fails with a pointer to user sync
/// (privacy macros) or GDPR enforcement (region keys). Spec line 3938 makes
/// `PublisherID` an auction core requirement, so this list carries both
/// `AccountID` and `PublisherID`.
const ENDPOINT_MACRO_ALLOWLIST: &[&str] = &["AccountID", "PublisherID"];

fn validate_endpoint_macros(bidder_name: &str, macros: &[String]) -> Result<(), String> {
    for m in macros {
        if !ENDPOINT_MACRO_ALLOWLIST.contains(&m.as_str()) {
            return Err(format!(
                "bidder {bidder_name:?}: endpoint template uses unsupported macro \
                 {{{{.{m}}}}}. auction core supports {{{{.AccountID}}}}, {{{{.PublisherID}}}} \
                 (path/query — round-26 review High), and {{{{.Host}}}} (host_allowlist). \
                 Privacy macros ({{{{.GDPR}}}}, {{{{.GDPRConsent}}}}, {{{{.USPrivacy}}}}, \
                 {{{{.GPP}}}}, {{{{.GPPSID}}}}, {{{{.RedirectURL}}}}) belong on the SYNCER \
                 URL, not the endpoint, and are deferred to user sync. See plan constraint at L466. \
                 Either remove the macro from this YAML's endpoint, move it to the syncer \
                 url field, or wait for user sync."
            ));
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Call the guard from each bidder's codegen path**

Wherever `build.rs` already calls `extract_endpoint_macros(&endpoint)` to populate `BidderInfo.endpoint_macros`, add the validation immediately after:

```rust
let macros = extract_endpoint_macros(&endpoint);
validate_endpoint_macros(&bidder_name, &macros)
    .unwrap_or_else(|err| panic!("build.rs: {err}"));
```

(`panic!` inside `build.rs` aborts the build with the message attached as the failure output; foundations already uses this pattern for the syncer-URL allow-list at `BIDDER_SYNCER_ALLOWED_MACROS`. Mirror it here.)

- [ ] **Step 3: Add a build_failure_harness test that proves the guard fires**

Extend `crates/auctioneer-config/tests/build_failure_harness.rs`:

```rust
#[test]
fn rejects_unsupported_endpoint_macro_at_codegen() {
    // Round-26 review High: PublisherID is now SUPPORTED in auction core, so
    // the failure-mode case uses `{{.GDPR}}` (a privacy macro that
    // belongs on the SYNCER URL, not the endpoint). Plant a temp
    // BidderInfo YAML that uses `{{.GDPR}}` on the endpoint, run a
    // child cargo build under `--features auctioneer-config/test-harness-override`,
    // assert the build fails AND that the failure message names GDPR.
    let dir = tempfile::tempdir().expect("tmpdir");
    std::fs::create_dir_all(dir.path().join("bidder-info")).unwrap();
    std::fs::write(
        dir.path().join("bidder-info/badmacro.yaml"),
        r#"endpoint: "https://example.test/path?gdpr={{.GDPR}}"
maintainer: { email: "x@example.test" }
capabilities:
  site: { mediaTypes: [banner] }
"#,
    ).unwrap();
    let out = build_in_child(dir.path());
    assert!(!out.status.success(), "expected build to fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("GDPR") && stderr.contains("unsupported macro"),
        "expected GDPR rejection, got:\n{stderr}"
    );
}

#[test]
fn accepts_publisher_id_macro_at_codegen() {
    // Round-26 review High: a bidder YAML using `{{.PublisherID}}`
    // MUST build cleanly because spec line 3938 requires auction core coverage.
    let dir = tempfile::tempdir().expect("tmpdir");
    std::fs::create_dir_all(dir.path().join("bidder-info")).unwrap();
    std::fs::write(
        dir.path().join("bidder-info/pubid.yaml"),
        r#"endpoint: "https://example.test/path?pub={{.PublisherID}}"
maintainer: { email: "x@example.test" }
capabilities:
  site: { mediaTypes: [banner] }
"#,
    ).unwrap();
    let out = build_in_child(dir.path());
    assert!(out.status.success(),
        "expected build to succeed; got stderr:\n{}",
        String::from_utf8_lossy(&out.stderr));
}
```

- [ ] **Step 4: Test**

```
cargo test -p auctioneer-config --test build_failure_harness rejects_unsupported_endpoint_macro_at_codegen
cargo test -p auctioneer-config --test build_failure_harness accepts_publisher_id_macro_at_codegen
```

Expected: passes.

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-config/build.rs crates/auctioneer-config/tests/build_failure_harness.rs
git commit -m "auctioneer-config: auction core endpoint macro allow-list codegen guard"
```

---

### Task 23: `tmax.rs` — effective budget resolution + deadline anchoring

**Files:**

- Modify: `crates/auctioneer-exchange/src/tmax.rs`
- Create: `crates/auctioneer-exchange/tests/tmax.rs`
- Round-22 review High 7: this task ALSO modifies `auctioneer-config`. The file list and verification step were narrowed to the `auctioneer-exchange` tmax surface, but the task body extends `ResourceLimits`, the YAML deserializer, the `emit_host_default` writer, the host-defaults.yaml fixture, and adds a build_failure_harness test. List all four:
- Modify: `crates/auctioneer-config/src/types.rs` (extend `ResourceLimits`).
- Modify: `crates/auctioneer-config/build.rs` (extend the `ResourceLimitsY` deserializer + `emit_host_default` writer + `validate_host_resource_limits` codegen-time positivity check; see Step 1d).
- Modify: `crates/auctioneer-config/data/host-defaults.yaml` (Step 1c).
- Modify: `crates/auctioneer-config/tests/build_failure_harness.rs` (Step 1e — zero-rejection fixture).

**Interfaces:**

- Consumes: `BidRequest.tmax` (u32 ms), `HostConfig` (`tmax_default` and `tmax_max` — both in milliseconds — both read from `host.resource_limits.default_tmax_ms` + a new `max_tmax_ms` field added in this task).
- Produces: `pub struct TmaxResolution { pub bidder_visible_tmax_ms: u32, pub deadline: AuctionDeadline }`; `pub fn resolve_tmax(req_tmax: Option<u64>, host: &HostConfig, request_start: Duration) -> TmaxResolution`.

- [ ] **Step 1: Extend `auctioneer-config` types**

Edit `crates/auctioneer-config/src/types.rs` — extend `ResourceLimits`:

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub default_tmax_ms: i64,
    /// Hard ceiling on the resolved auction budget. Spec §5.3.
    pub max_tmax_ms: i64,
    /// Per-bidder response size cap before the bid is dropped with
    /// code 4 `BadServerResponse`. Spec §5.3.
    pub max_bidder_response_bytes: usize,
    /// Maximum bidders selected per auction (§5.3 / §5.9 registry order).
    pub max_bidders_per_auction: usize,
    /// Maximum total outbound requests across all bidders (§5.3).
    pub max_outbound_requests: usize,
}
```

(The foundations `ResourceLimits` already has `default_tmax_ms`. We extend it without disturbing existing tests.)

Round-14 review High Task 23: codegen self-contained. The foundations `build.rs` only reads / emits `default_tmax_ms`; the four new fields need explicit codegen wiring or the generated `HostConfig` will not compile after the type change. Apply these inline edits BEFORE the Step 2 failing tests compile:

- [ ] **Step 1a: Extend `ResourceLimitsY` deserializer in `crates/auctioneer-config/build.rs`**

```rust
#[derive(Debug, Deserialize)]
struct ResourceLimitsY {
    default_tmax_ms: i64,
    #[serde(default)]
    max_tmax_ms: i64,
    #[serde(default)]
    max_bidder_response_bytes: usize,
    #[serde(default)]
    max_bidders_per_auction: usize,
    #[serde(default)]
    max_outbound_requests: usize,
}
```

Each `#[serde(default)]` lets a missing value parse as `0`, which the codegen-time validator below rejects with a precise error message.

- [ ] **Step 1b: Extend `emit_host_default` to render every new field**

In the `write!(out, ...)` that emits `HostConfig { resource_limits: ResourceLimits { ... } }`, replace the existing `resource_limits` line with:

```rust
let tmax    = parsed.resource_limits.default_tmax_ms;
let max_tmax= parsed.resource_limits.max_tmax_ms;
let max_brb = parsed.resource_limits.max_bidder_response_bytes;
let max_bpa = parsed.resource_limits.max_bidders_per_auction;
let max_or  = parsed.resource_limits.max_outbound_requests;
write!(
    out,
    "        resource_limits: ResourceLimits {{\n\
    \x20            default_tmax_ms: {tmax},\n\
    \x20            max_tmax_ms: {max_tmax},\n\
    \x20            max_bidder_response_bytes: {max_brb},\n\
    \x20            max_bidders_per_auction: {max_bpa},\n\
    \x20            max_outbound_requests: {max_or},\n\
    \x20        }},\n",
)?;
```

- [ ] **Step 1c: Bump the embedded defaults in `crates/auctioneer-config/data/host-defaults.yaml`**

```yaml
resource_limits:
  default_tmax_ms: 750
  max_tmax_ms: 1500
  max_bidder_response_bytes: 262144
  max_bidders_per_auction: 50
  max_outbound_requests: 100
```

(Adjust this block alongside whatever existing fields it carries — keep them.)

- [ ] **Step 1d: Codegen-time validation in `build.rs`** — reject zero/missing values for the four new positive caps. Mirror the `validate_bidder_yaml_enums` pattern.

```rust
fn validate_host_resource_limits(rl: &ResourceLimitsY) -> BuildResult {
    for (name, value) in [
        ("default_tmax_ms", rl.default_tmax_ms),
        ("max_tmax_ms",      rl.max_tmax_ms),
    ] {
        if value <= 0 {
            return Err(format!(
                "host-defaults.yaml resource_limits.{name} = {value}; \
                 must be > 0 per spec §5.3"
            ).into());
        }
    }
    for (name, value) in [
        ("max_bidder_response_bytes", rl.max_bidder_response_bytes),
        ("max_bidders_per_auction",   rl.max_bidders_per_auction),
        ("max_outbound_requests",     rl.max_outbound_requests),
    ] {
        if value == 0 {
            return Err(format!(
                "host-defaults.yaml resource_limits.{name} = 0; \
                 must be > 0 per spec §5.3"
            ).into());
        }
    }
    Ok(())
}
```

Call it from `emit_host_default` right after `serde_yaml::from_str` succeeds, before any field is read.

- [ ] **Step 1e: Add a build_failure_harness fixture for a zeroed-out field.** Reuse the `AUCTIONEER_BIDDER_INFO_DIR` env-var-override pattern, but for `host-defaults.yaml`: introduce a parallel `AUCTIONEER_HOST_DEFAULTS_PATH` env-var gated under the same `test-harness-override` feature. The harness writes a `host-defaults.yaml` with `max_tmax_ms: 0` to a tempdir, points the override at that path, and asserts the child cargo build fails with the validation message above.

The Step 2 failing tests below compile only after these edits land. The codegen change is THE BLOCKER for Task 23 — moving on to `tmax.rs` without it produces a generated.rs that no longer compiles.

- [ ] **Step 2: Write the failing tests in `tests/tmax.rs`**

```rust
use auctioneer_config::HostConfig;
use auctioneer_exchange::tmax::{TmaxResolution, resolve_tmax};
use auctioneer_exchange::TmaxSource;
use std::time::Duration;

fn host_with_caps(default_ms: i64, max_ms: i64) -> HostConfig {
    HostConfig {
        resource_limits: auctioneer_config::ResourceLimits {
            default_tmax_ms: default_ms,
            max_tmax_ms: max_ms,
            max_bidder_response_bytes: 262_144,
            max_bidders_per_auction: 50,
            max_outbound_requests: 100,
        },
        ..HostConfig::default()
    }
}

#[test]
fn absent_request_tmax_uses_host_default() {
    let host = host_with_caps(750, 1500);
    let start = Duration::from_secs(10);
    let r = resolve_tmax(None, &host, start);
    assert_eq!(r.bidder_visible_tmax_ms, 750);
    assert_eq!(r.deadline.source, TmaxSource::HostDefault);
    assert_eq!(r.deadline.deadline_at, start + Duration::from_millis(750));
}

#[test]
fn zero_request_tmax_uses_host_default() {
    let host = host_with_caps(750, 1500);
    let start = Duration::from_secs(10);
    let r = resolve_tmax(Some(0), &host, start);
    assert_eq!(r.bidder_visible_tmax_ms, 750);
    assert_eq!(r.deadline.source, TmaxSource::HostDefault);
}

#[test]
fn request_tmax_under_cap_passes_through() {
    let host = host_with_caps(750, 1500);
    let start = Duration::from_secs(10);
    let r = resolve_tmax(Some(900), &host, start);
    assert_eq!(r.bidder_visible_tmax_ms, 900);
    assert_eq!(r.deadline.source, TmaxSource::Request);
}

#[test]
fn request_tmax_over_cap_is_visible_to_bidder_but_deadline_clamped() {
    // §5.3 four-value taxonomy: adapter-visible TMax is NOT clamped,
    // server-side execution deadline IS clamped.
    let host = host_with_caps(750, 1500);
    let start = Duration::from_secs(10);
    let r = resolve_tmax(Some(5000), &host, start);
    assert_eq!(r.bidder_visible_tmax_ms, 5000);
    assert_eq!(r.deadline.source, TmaxSource::CappedByHost);
    assert_eq!(r.deadline.deadline_at, start + Duration::from_millis(1500));
}
```

- [ ] **Step 3: Implement `src/tmax.rs`**

```rust
//! Effective-tmax + auction-deadline resolution. See spec §5.3.

use crate::fanout::{AuctionDeadline, TmaxSource};
use auctioneer_config::HostConfig;
use std::time::Duration;

pub struct TmaxResolution {
    /// What the bidder will see in the outbound `BidRequest.TMax` (§5.3
    /// "adapter-visible TMax"). NOT clamped to `tmax_max`.
    pub bidder_visible_tmax_ms: u32,
    /// Whole-auction execution deadline (`request_start + effective_budget`).
    pub deadline: AuctionDeadline,
}

#[must_use]
pub fn resolve_tmax(req_tmax_ms: Option<u64>, host: &HostConfig, request_start: Duration)
    -> TmaxResolution
{
    // Round-20 review Spec 11: explicit conversion via try_from to
    // satisfy the workspace `-D clippy::restriction` gate (no
    // unannotated `as` casts in production code). `default_tmax_ms`
    // / `max_tmax_ms` are validated > 0 by the build-time
    // `validate_host_resource_limits` check (Task 23), so the
    // conversion is infallible in practice; we still write the
    // safe form so a future zeroed config doesn't underflow.
    let default_ms = u64::try_from(host.resource_limits.default_tmax_ms.max(0)).unwrap_or(0);
    let max_ms     = u64::try_from(host.resource_limits.max_tmax_ms.max(0)).unwrap_or(0);

    let (bidder_visible_ms, effective_ms, source) = match req_tmax_ms {
        None | Some(0) => (default_ms, default_ms, TmaxSource::HostDefault),
        Some(t) if t > max_ms => (t, max_ms, TmaxSource::CappedByHost),
        Some(t) => (t, t, TmaxSource::Request),
    };

    TmaxResolution {
        // Round-21 review High 5: `.min(u64::from(u32::MAX))` is in
        // `0..=u32::MAX` so the conversion cannot truncate; use
        // `try_from` to satisfy the clippy `as_conversions` gate.
        bidder_visible_tmax_ms: u32::try_from(bidder_visible_ms.min(u64::from(u32::MAX))).unwrap_or(u32::MAX),
        deadline: AuctionDeadline {
            deadline_at: request_start + Duration::from_millis(effective_ms),
            source,
        },
    }
}
```

Wire `lib.rs`:

```rust
pub mod tmax;
```

- [ ] **Step 4: Run**

Round-22 review High 7: this task touches auctioneer-config too, so the verification gate runs the config-side tests AS WELL as the exchange-side tmax tests. Both must pass for the commit to land:

```
cargo test -p auctioneer-config --test build_failure_harness
cargo test -p auctioneer-config --test url_safety
cargo test -p auctioneer-exchange --test tmax
cargo check  -p auctioneer-config
cargo check  -p auctioneer-exchange
```

Expected: existing auctioneer-config harness counts + 1 new fixture (`max_tmax_ms=0` rejection); url_safety unchanged; auctioneer-exchange `tmax` = 4 passed.

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-config crates/auctioneer-exchange
git commit -m "auctioneer-exchange: tmax resolution (§5.3 four-value taxonomy)"
```

---

### Task 24: `registry_order.rs` — §5.9 lexicographic participating-bidder set

**Files:**

- Modify: `crates/auctioneer-exchange/src/registry_order.rs`

**Interfaces:**

- Consumes: `&NormalizedBidRequest`, `&BidderRegistry`.
- Produces: `pub fn participating_bidders(req: &BidRequest, registry: &BidderRegistry) -> Vec<BidderName>`. Single lex sort over (a) base names declared on any imp's `imp.ext.bidder.<name>` or `imp.ext.prebid.bidder.<name>`, (b) request-defined alias names from `ext.prebid.aliases`, (c) host/config-declared aliases (none in phase 1 by default).

- [ ] **Step 1: Implement + inline test**

Replace `src/registry_order.rs`:

```rust
//! §5.9 participating-bidder set assembly + lexicographic ordering.

use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::collections::BTreeSet;

/// Lexicographic union of base bidder names + request-defined alias names
/// + host/config-declared aliases that participate in this auction.
/// Phase 1 has no host/config-declared aliases by default.
#[must_use]
pub fn participating_bidders(req: &BidRequest, registry: &BidderRegistry) -> Vec<BidderName> {
    let mut set: BTreeSet<BidderName> = BTreeSet::new();

    // Round-14 review Blocker Imp.ext drift: typed-struct navigation.
    // Imp.ext is Option<ImpExt> with the legacy/modern bidder blocks
    // exposed as imp.ext.bidder (Option<serde_json::Value>) and
    // imp.ext.prebid.bidder (BTreeMap<String, serde_json::Value>).
    // BidRequest.ext is Option<BidRequestExt> with prebid.aliases as
    // BTreeMap<String, String>.
    for imp in &req.imp {
        let Some(ext) = imp.ext.as_ref() else { continue };
        // Legacy slot: imp.ext.bidder.<name> = { params }
        if let Some(legacy) = ext.bidder.as_ref().and_then(|v| v.as_object()) {
            for k in legacy.keys() {
                if registry.get(k).is_some() {
                    set.insert(BidderName::new(k.as_str()));
                }
            }
        }
        // Modern slot: imp.ext.prebid.bidder is BTreeMap<String, Value>
        if let Some(prebid) = ext.prebid.as_ref() {
            for k in prebid.bidder.keys() {
                if registry.get(k).is_some() {
                    set.insert(BidderName::new(k.as_str()));
                }
            }
        }
    }

    // Request-defined aliases (BidRequestExt.prebid.aliases is a
    // BTreeMap<String, String>). Each alias counts as one
    // participating entry even though it shares the base's adapter
    // (§5.9).
    if let Some(prebid) = req.ext.as_ref().and_then(|e| e.prebid.as_ref()) {
        for alias in prebid.aliases.keys() {
            set.insert(BidderName::new(alias.as_str()));
        }
    }

    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_bidder::Bidder;
    // Round-15 review Blocker stale typed-API: the typed foundations ext
    // structs replace the legacy RawValue import.
    use auctioneer_openrtb::ext::imp::ImpExt;
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::{BidRequestExt, Imp};
    use std::collections::BTreeMap;

    struct Dummy;
    impl Bidder for Dummy {
        fn make_requests(&self, _r: &BidRequest, _i: &auctioneer_bidder::BidderRequestInfo<'_>)
            -> Result<Vec<auctioneer_bidder::BidderHttpRequest>, Vec<auctioneer_bidder::BidderError>>
        { Ok(vec![]) }
        fn make_bids(&self, _r: &BidRequest, _q: &auctioneer_bidder::BidderHttpRequest,
                     _p: &auctioneer_bidder::BidderHttpResponse)
            -> Result<auctioneer_bidder::BidderResponse, Vec<auctioneer_bidder::BidderError>>
        { Ok(auctioneer_bidder::BidderResponse { bids: vec![], currency: "USD".into() }) }
    }

    fn registry_with(names: &[&'static str]) -> BidderRegistry {
        let mut r = BidderRegistry::new();
        for n in names {
            let info: &'static _ = Box::leak(Box::new(auctioneer_config::BidderInfo {
                name: (*n).into(),
                ..auctioneer_config::BidderInfo::default()
            }));
            r.register(info, Box::new(Dummy));
        }
        r
    }

    // Round-14 review Blocker Imp.ext drift: tests construct typed
    // ImpExt / BidRequestExt instead of round-tripping through
    // RawValue. The legacy `imp.ext.bidder` slot is
    // Option<serde_json::Value>; ExtRequestPrebid.aliases is a
    // BTreeMap<String, String>.

    fn imp_with_legacy_bidder_block(id: &str, bidder_names: &[&str]) -> Imp {
        let mut obj = serde_json::Map::new();
        for name in bidder_names {
            obj.insert((*name).to_owned(), serde_json::json!({}));
        }
        Imp {
            id: id.into(),
            ext: Some(ImpExt {
                bidder: Some(serde_json::Value::Object(obj)),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }
    }

    #[test]
    fn collects_from_imp_ext_bidder() {
        let registry = registry_with(&["appnexus", "pubmatic", "generic"]);
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_bidder_block("i1", &["pubmatic", "appnexus"])],
            ..BidRequest::default()
        };
        let names: Vec<String> = participating_bidders(&req, &registry)
            .into_iter().map(|n| n.as_str().to_owned()).collect();
        assert_eq!(names, vec!["appnexus".to_string(), "pubmatic".to_string()]);
    }

    #[test]
    fn aliases_appear_as_first_class_entries() {
        let registry = registry_with(&["appnexus"]);
        let mut aliases = BTreeMap::new();
        aliases.insert("myalias".to_owned(), "appnexus".to_owned());
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_bidder_block("i1", &["appnexus"])],
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        };
        let names: Vec<String> = participating_bidders(&req, &registry)
            .into_iter().map(|n| n.as_str().to_owned()).collect();
        assert_eq!(names, vec!["appnexus".to_string(), "myalias".to_string()]);
    }

    #[test]
    fn unknown_bidder_name_is_filtered_out() {
        let registry = registry_with(&["appnexus"]);
        let req = BidRequest {
            id: "x".into(),
            imp: vec![imp_with_legacy_bidder_block("i1", &["unknown_bidder"])],
            ..BidRequest::default()
        };
        let names = participating_bidders(&req, &registry);
        assert!(names.is_empty());
    }
}
```

Wire into `lib.rs`:

```rust
mod registry_order;
pub use registry_order::participating_bidders;
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-exchange registry_order::tests
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: §5.9 participating-bidder set + lexicographic order"
```

---

### Task 25: `alias.rs` — alias resolution + alias-validation errors

**Files:**

- Modify: `crates/auctioneer-exchange/src/alias.rs`

**Interfaces:**

- Consumes: `&BidRequest`, `&BidderRegistry`.
- Produces: `pub fn resolve_aliases(req: &BidRequest, registry: &BidderRegistry) -> Result<AliasMap, AuctionError>` where `AliasMap` records (alias_name → base_name) plus the resolved `aliasgvlids` overrides. Validates: unknown base, disabled base, no-op alias, unknown `aliasgvlids` entry, `aliasgvlids < 1`. Failure → `AuctionError::BadInput`.

- [ ] **Step 1: Implement + inline test**

Replace `src/alias.rs`:

```rust
//! Alias resolution + alias-validation errors. See spec §5.1 step 7 / §5.9.

use crate::error::AuctionError;
use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_openrtb::types::bid_request::BidRequest;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct AliasMap {
    /// alias_name → base_name
    pub bases: HashMap<BidderName, BidderName>,
    /// alias_name → gvl_vendor_id override (from `ext.prebid.aliasgvlids`)
    pub gvlids: HashMap<BidderName, u32>,
}

pub fn resolve_aliases(req: &BidRequest, registry: &BidderRegistry)
    -> Result<AliasMap, AuctionError>
{
    // Round-14 review Blocker Imp.ext drift: BidRequest.ext is
    // Option<BidRequestExt> (typed). Aliases live at
    // req.ext.prebid.aliases as BTreeMap<String, String> and
    // aliasgvlids at req.ext.prebid.aliasgvlids as
    // BTreeMap<String, u32>. No raw-JSON parse.
    let mut out = AliasMap::default();
    let Some(prebid) = req.ext.as_ref().and_then(|e| e.prebid.as_ref()) else {
        return Ok(out);
    };

    // ext.prebid.aliases — map of alias_name → base_name (both typed
    // as String).
    for (alias_name, base) in &prebid.aliases {
        if alias_name == base {
            return Err(AuctionError::BadInput(
                format!("alias {alias_name} is a no-op")));
        }
        // Base bidder must exist in the registry.
        let (info, _adapter) = registry.get(base).ok_or_else(|| AuctionError::BadInput(
            format!("alias {alias_name} targets unknown bidder {base}")))?;
        if info.disabled {
            return Err(AuctionError::BadInput(
                format!("alias {alias_name} targets disabled bidder {base}")));
        }
        out.bases.insert(
            BidderName::new(alias_name.as_str()),
            BidderName::new(base.as_str()),
        );
    }

    // ext.prebid.aliasgvlids — BTreeMap<String, u32>. Each entry must
    // reference a declared alias and the value must be >= 1 (u32, so
    // already non-negative; explicit check for the zero case).
    for (alias_name, gvlid) in &prebid.aliasgvlids {
        let alias_key = BidderName::new(alias_name.as_str());
        if !out.bases.contains_key(&alias_key) {
            return Err(AuctionError::BadInput(
                format!("aliasgvlids names unknown alias {alias_name}")));
        }
        if *gvlid < 1 {
            return Err(AuctionError::BadInput(
                format!("aliasgvlids value for {alias_name} must be >= 1")));
        }
        out.gvlids.insert(alias_key, *gvlid);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_bidder::Bidder;
    // Round-15 review Blocker stale typed-API: replace the stale
    // serde_json::value::RawValue import with the typed foundations ext
    // structs the new test helpers actually use.
    use auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid;
    use auctioneer_openrtb::types::bid_request::BidRequestExt;
    use std::collections::BTreeMap;

    struct Dummy;
    impl Bidder for Dummy {
        fn make_requests(&self, _r: &BidRequest, _i: &auctioneer_bidder::BidderRequestInfo<'_>)
            -> Result<Vec<auctioneer_bidder::BidderHttpRequest>, Vec<auctioneer_bidder::BidderError>>
        { Ok(vec![]) }
        fn make_bids(&self, _r: &BidRequest, _q: &auctioneer_bidder::BidderHttpRequest,
                     _p: &auctioneer_bidder::BidderHttpResponse)
            -> Result<auctioneer_bidder::BidderResponse, Vec<auctioneer_bidder::BidderError>>
        { Ok(auctioneer_bidder::BidderResponse { bids: vec![], currency: "USD".into() }) }
    }

    fn registry_with(names: &[(&'static str, bool /* disabled */)]) -> BidderRegistry {
        let mut r = BidderRegistry::new();
        for (n, d) in names {
            let info: &'static _ = Box::leak(Box::new(auctioneer_config::BidderInfo {
                name: (*n).into(),
                disabled: *d,
                ..auctioneer_config::BidderInfo::default()
            }));
            r.register(info, Box::new(Dummy));
        }
        r
    }

    fn req_with_aliases(aliases: &[(&str, &str)]) -> BidRequest {
        // Round-14 review Blocker Imp.ext drift: BidRequest.ext is
        // Option<BidRequestExt> with prebid.aliases as
        // BTreeMap<String, String>. No RawValue.
        let mut alias_map = BTreeMap::new();
        for (k, v) in aliases {
            alias_map.insert((*k).to_owned(), (*v).to_owned());
        }
        BidRequest {
            id: "x".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        }
    }

    fn req_with_alias_and_gvlids(aliases: &[(&str, &str)], gvlids: &[(&str, u32)]) -> BidRequest {
        let mut alias_map = BTreeMap::new();
        for (k, v) in aliases { alias_map.insert((*k).to_owned(), (*v).to_owned()); }
        let mut gvlid_map = BTreeMap::new();
        for (k, v) in gvlids { gvlid_map.insert((*k).to_owned(), *v); }
        BidRequest {
            id: "x".into(),
            ext: Some(BidRequestExt {
                prebid: Some(ExtRequestPrebid {
                    aliases: alias_map,
                    aliasgvlids: gvlid_map,
                    ..ExtRequestPrebid::default()
                }),
                ..BidRequestExt::default()
            }),
            ..BidRequest::default()
        }
    }

    #[test]
    fn rejects_unknown_base() {
        let registry = registry_with(&[("appnexus", false)]);
        let req = req_with_aliases(&[("my", "nope")]);
        let err = resolve_aliases(&req, &registry).unwrap_err();
        assert!(matches!(err, AuctionError::BadInput(ref m) if m.contains("unknown bidder nope")));
    }

    #[test]
    fn rejects_disabled_base() {
        let registry = registry_with(&[("appnexus", true)]);
        let req = req_with_aliases(&[("my", "appnexus")]);
        let err = resolve_aliases(&req, &registry).unwrap_err();
        assert!(matches!(err, AuctionError::BadInput(ref m) if m.contains("disabled bidder appnexus")));
    }

    #[test]
    fn rejects_no_op_alias() {
        let registry = registry_with(&[("appnexus", false)]);
        let req = req_with_aliases(&[("appnexus", "appnexus")]);
        let err = resolve_aliases(&req, &registry).unwrap_err();
        assert!(matches!(err, AuctionError::BadInput(ref m) if m.contains("no-op")));
    }

    #[test]
    fn aliasgvlids_unknown_alias_rejected() {
        let registry = registry_with(&[("appnexus", false)]);
        let req = req_with_alias_and_gvlids(&[("my", "appnexus")], &[("other", 42)]);
        let err = resolve_aliases(&req, &registry).unwrap_err();
        assert!(matches!(err, AuctionError::BadInput(ref m) if m.contains("unknown alias other")));
    }

    #[test]
    fn aliasgvlids_zero_rejected() {
        let registry = registry_with(&[("appnexus", false)]);
        let req = req_with_alias_and_gvlids(&[("my", "appnexus")], &[("my", 0)]);
        let err = resolve_aliases(&req, &registry).unwrap_err();
        assert!(matches!(err, AuctionError::BadInput(ref m) if m.contains(">= 1")));
    }

    #[test]
    fn valid_alias_pair_resolves() {
        let registry = registry_with(&[("appnexus", false)]);
        let req = req_with_alias_and_gvlids(&[("my", "appnexus")], &[("my", 42)]);
        let m = resolve_aliases(&req, &registry).unwrap();
        assert_eq!(m.bases.get(&BidderName::new("my")), Some(&BidderName::new("appnexus")));
        assert_eq!(m.gvlids.get(&BidderName::new("my")), Some(&42));
    }
}
```

Wire into `lib.rs`:

```rust
mod alias;
pub use alias::{AliasMap, resolve_aliases};
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-exchange alias::tests
```

Expected: 6 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: alias resolution + validation errors"
```

---

### Task 26: `split.rs` — per-bidder request split + bidderparams merge + disabled-bidder removal

**Files:**

- Modify: `crates/auctioneer-exchange/src/split.rs`

**Interfaces:**

- Consumes: `&BidRequest`, `&AliasMap`, `&BidderRegistry`, `&dyn PrivacyContext`-stub.
- Produces: `pub struct SplitBidders { pub per_bidder: Vec<PerBidderRequest>, pub disabled_bidder_warnings: Vec<(BidderName, ErrorEntry)> }` where `PerBidderRequest = { bidder: BidderName, base: BidderName, request: BidRequest }`. Round-19 review High 9: per-bidder warnings are typed as `crate::response::ErrorEntry` (carries the raw `code: u32`) rather than `NonFatalWarning` so the code-6 `BidderTemporarilyDisabled` value can be emitted directly — the foundational `WarningCode` enum only encodes 10001 / 10999, and widening it is out of auction core scope. Per §5.1 step 7: (a) merge request-level `ext.prebid.bidderparams.{name}` into per-imp ext (supplement only — never creates participation); (b) hoist `imp.ext.prebid.bidder.{name}` to `imp.ext.bidder`; (c) merge bidder-specific FPD from `imp.ext.prebid.imp.{name}` into the bidder-scoped imp; (d) re-validate bidder-scoped imp — failure → `AuctionError::InvalidImpFirstPartyData`; (e) drop disabled bidders per imp and push a `(bidder, ErrorEntry { code: 6, message: "bidder {name} is disabled" })` entry into `disabled_bidder_warnings` (run_auction folds these into the matching `bundle.warnings`); (f) reject the auction if any imp ends up with zero remaining bidders.

- [ ] **Step 1: Implement (skeleton + tests)**

Given the volume, this task focuses on the **split mechanics** (alias rewrite + disabled-bidder filter + imp-no-bidders check) using JSON ops; the recursive `imp.ext.prebid.imp.{name}` FPD merge is implemented as a `serde_json::Value` `merge_patch` (RFC 7396) so we don't need typed FPD here in auction core.

Replace `src/split.rs`:

```rust
//! Per-bidder request split (§5.1 step 7). auction core uses serde_json Value ops
//! for the per-imp ext rewrite + FPD merge; typed FPD is deferred.

// Round-21 review Blocker 4: bring `ErrorEntry` into scope (the
// `disabled_bidder_warnings` channel and the constructor below are
// both unqualified). The earlier `NonFatalWarning` / `WarningCode`
// imports are gone — round-19 review High 9 routed disabled-bidder
// warnings through `ErrorEntry` (raw `code: u32`) so the foundations
// `WarningCode` enum doesn't need widening.
use crate::{alias::AliasMap, error::AuctionError, response::ErrorEntry};
use auctioneer_bidder::{BidderName, BidderRegistry};
use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct PerBidderRequest {
    pub bidder: BidderName,
    pub base: BidderName,
    pub request: BidRequest,
}

#[derive(Debug, Default)]
pub struct SplitBidders {
    pub per_bidder: Vec<PerBidderRequest>,
    /// Round-19 review High 9 + round-20 review Blocker 2: code-6
    /// `BidderTemporarilyDisabled` warnings emitted while pruning
    /// per-imp bidders, carried as wire-shaped `ErrorEntry` so the
    /// code can be a raw `u32: 6` (the foundational `WarningCode` enum cannot
    /// encode 6; widening is out of auction core scope). `run_auction` folds
    /// each entry into `bundles.entry(bidder).warnings`.
    pub disabled_bidder_warnings: Vec<(BidderName, ErrorEntry)>,
}

pub fn split_by_bidder(
    req: &BidRequest,
    aliases: &AliasMap,
    registry: &BidderRegistry,
) -> Result<SplitBidders, AuctionError> {
    // Build a per-imp map of bidder_name -> per-bidder imp ext.
    // We work in serde_json::Value space because typed FPD isn't a phase-1
    // requirement — the structural rewrite is enough.
    let request_value: Value = serde_json::to_value(req)
        .map_err(|e| AuctionError::Internal(format!("split serialize: {e}")))?;
    let request_bidder_params = request_value.pointer("/ext/prebid/bidderparams")
        .cloned()
        .unwrap_or(Value::Null);

    // Collect the participating bidder names (base + aliases).
    let participating: Vec<BidderName> = crate::registry_order::participating_bidders(req, registry);

    let mut per_bidder_imps: BTreeMap<BidderName, Vec<Value>> = BTreeMap::new();
    // Round-20 review Blocker 2: renamed to match the
    // `SplitBidders.disabled_bidder_warnings` field name. `ErrorEntry`
    // carries the raw `code: u32` so we can emit code 6 directly.
    let mut disabled_bidder_warnings: Vec<(BidderName, ErrorEntry)> = Vec::new();

    for (imp_idx, imp) in req.imp.iter().enumerate() {
        // Read the imp.ext as JSON.
        let imp_value: Value = serde_json::to_value(imp)
            .map_err(|e| AuctionError::Internal(format!("split imp serialize: {e}")))?;
        let imp_ext = imp_value.pointer("/ext").cloned().unwrap_or(Value::Null);

        // Compute which bidder names appear on this imp. Three
        // source slots per spec §3.1 + §5.1 step 7:
        //   1. imp.ext.bidder.<name> — modern hoisted slot
        //   2. imp.ext.prebid.bidder.<name> — modern typed slot
        //   3. imp.ext.<name> — legacy top-level slot, only counted
        //      when the key resolves as a registered bidder or a
        //      request-declared alias (so imp.ext.tid / imp.ext.prebid
        //      / free-form passthrough keys are not misclassified).
        // Round-80 review High: this third source was missing from
        // earlier drafts, so legacy-only requests were rejected as
        // "no valid bidders" and top-level competitor params leaked
        // across bidder-scoped outbound bodies.
        let mut bidders_on_imp: BTreeMap<BidderName, Value> = BTreeMap::new();
        if let Some(obj) = imp_ext.pointer("/bidder").and_then(|b| b.as_object()) {
            for (k, v) in obj {
                bidders_on_imp.insert(BidderName::new(k.as_str()), v.clone());
            }
        }
        if let Some(obj) = imp_ext.pointer("/prebid/bidder").and_then(|b| b.as_object()) {
            for (k, v) in obj {
                bidders_on_imp.entry(BidderName::new(k.as_str())).or_insert_with(|| v.clone());
            }
        }

        // Merge request-level ext.prebid.bidderparams (supplement only —
        // never creates participation).
        if let Some(req_params) = request_bidder_params.as_object() {
            for (bidder_name, supplement) in req_params {
                let key = BidderName::new(bidder_name.as_str());
                if let Some(existing) = bidders_on_imp.get_mut(&key) {
                    merge_supplement(existing, supplement);
                }
                // Else: no entry on this imp → skip per spec (round-23 H1).
            }
        }

        // Resolve alias rewrite: an alias's outbound request still
        // carries the alias's params. The base+adapter mapping is
        // applied later when calling make_requests.
        for bidder_name in bidders_on_imp.keys().cloned().collect::<Vec<_>>() {
            // Identify whether this is a base or alias.
            let is_alias = aliases.bases.contains_key(&bidder_name);
            let base = if is_alias {
                aliases.bases.get(&bidder_name).cloned().unwrap()
            } else {
                bidder_name.clone()
            };

            // Lookup base info — drop disabled bidders per imp.
            let (info, _adapter) = match registry.get(base.as_str()) {
                Some(t) => t,
                None => {
                    // Unknown base — silently drop (shouldn't happen post
                    // alias resolution).
                    continue;
                }
            };
            if info.disabled {
                // Round-19 review High 9: emit code 6
                // `BidderTemporarilyDisabled` directly per spec line 558
                // / Appendix G line 5403. Earlier drafts used
                // `WarningCode::Unknown` (10999) because the foundational
                // `WarningCode` enum cannot encode 6; widening the
                // enum is out of auction core scope, so we use `ErrorEntry`
                // (raw `code: u32`) here and have run_auction route
                // these into the matching `bundle.warnings`.
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

            // Bidder-scoped imp: take the imp shape, drop other bidders'
            // params from imp.ext, leave only this bidder's params at
            // the hoisted slot `imp.ext.bidder`.
            let mut scoped_imp = imp_value.clone();
            scoped_imp["ext"]["bidder"] = bidders_on_imp[&bidder_name].clone();
            // Merge FPD from imp.ext.prebid.imp.{name} per round-17 diagnostic qualification.
            let fpd_path = format!("/ext/prebid/imp/{}", bidder_name.as_str());
            if let Some(fpd) = imp_value.pointer(&fpd_path) {
                merge_patch(&mut scoped_imp, fpd);
            }
            // Round-22 review High 4: per spec §5.1 step 7 line 882,
            // the bidder-scoped imp MUST NOT carry competitor params
            // / FPD on the wire. After the bidder-specific hoist +
            // merge above, strip every OTHER bidder's entry from
            // `imp.ext.prebid.bidder.{name}` and `imp.ext.prebid.imp
            // .{name}`. The current bidder's own entries already
            // got hoisted up, so removing them too is the safe move
            // (they would be duplicates otherwise).
            if let Some(prebid) = scoped_imp.pointer_mut("/ext/prebid") {
                if let Some(obj) = prebid.as_object_mut() {
                    obj.remove("bidder");
                    obj.remove("imp");
                }
            }
            // The legacy `imp.ext.bidder.{name}` block likewise gets
            // narrowed: we replaced `ext.bidder` with this bidder's
            // params object above, so it carries no competitor data.
            // No further pruning needed there.
            per_bidder_imps.entry(bidder_name).or_default().push(scoped_imp);
        }

        if !participating.iter().any(|p| {
            // Check whether ANY participating bidder claimed this imp.
            // We approximate by checking if any per_bidder_imps entry got
            // an imp for this index — done after the inner loop completes.
            per_bidder_imps.get(p).is_some_and(|imps| imps.iter().any(|i| {
                i.pointer("/id").and_then(|v| v.as_str()) == imp.id.as_str().into()
            }))
        }) {
            return Err(AuctionError::BadInput(
                format!("imp {} has no valid bidders", imp.id)));
        }
    }

    // Materialize per-bidder BidRequests. Round-20 review Blocker 2:
    // `PerBidderRequest` no longer carries `warnings`; the disabled-
    // bidder warnings live on `SplitBidders.disabled_bidder_warnings`
    // and are surfaced into `bundles[bidder].warnings` by run_auction.
    let mut out = SplitBidders::default();
    for (bidder_name, imps) in per_bidder_imps {
        let base = aliases.bases.get(&bidder_name)
            .cloned()
            .unwrap_or_else(|| bidder_name.clone());
        let mut br_value = request_value.clone();
        br_value["imp"] = Value::Array(imps);
        let br: BidRequest = serde_json::from_value(br_value)
            .map_err(|e| AuctionError::InvalidImpFirstPartyData(e.to_string()))?;
        out.per_bidder.push(PerBidderRequest {
            bidder: bidder_name,
            base,
            request: br,
        });
    }
    out.disabled_bidder_warnings = disabled_bidder_warnings;

    Ok(out)
}

/// RFC 7396 JSON merge-patch. Used for FPD overrides at imp scope.
fn merge_patch(target: &mut Value, patch: &Value) {
    if let (Value::Object(t), Value::Object(p)) = (&mut *target, patch) {
        for (k, v) in p {
            if v.is_null() { t.remove(k); }
            else {
                let entry = t.entry(k.clone()).or_insert(Value::Null);
                merge_patch(entry, v);
            }
        }
    } else {
        *target = patch.clone();
    }
}

/// Supplement merge: only fill missing keys; never overwrite (round-23 H1).
fn merge_supplement(target: &mut Value, supplement: &Value) {
    let (Value::Object(t), Value::Object(s)) = (&mut *target, supplement) else {
        return;
    };
    for (k, v) in s {
        // Round-23 review High 4: `t.get_mut(k).unwrap()` after a
        // `contains_key` check is structurally safe but trips the
        // clippy `unwrap_used` gate. Use the entry API instead so
        // the borrow is local + lint-clean.
        match t.entry(k.clone()) {
            serde_json::map::Entry::Vacant(slot) => {
                slot.insert(v.clone());
            }
            serde_json::map::Entry::Occupied(mut slot) => {
                merge_supplement(slot.get_mut(), v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Real split tests live in tests/run_auction.rs once run_auction is wired;
    // we only smoke-test the merge helpers here.

    #[test]
    fn merge_patch_replaces_arrays_whole() {
        let mut t = serde_json::json!({"a": [1,2,3], "b": "x"});
        let p = serde_json::json!({"a": [9], "c": "y"});
        merge_patch(&mut t, &p);
        assert_eq!(t, serde_json::json!({"a": [9], "b": "x", "c": "y"}));
    }

    #[test]
    fn merge_patch_null_removes_key() {
        let mut t = serde_json::json!({"a": 1, "b": 2});
        let p = serde_json::json!({"a": null});
        merge_patch(&mut t, &p);
        assert_eq!(t, serde_json::json!({"b": 2}));
    }

    #[test]
    fn supplement_only_fills_missing() {
        let mut t = serde_json::json!({"a": 1});
        let s = serde_json::json!({"a": 99, "b": 2});
        merge_supplement(&mut t, &s);
        // 'a' stays 1; 'b' fills in.
        assert_eq!(t, serde_json::json!({"a": 1, "b": 2}));
    }

    // Round-23 review Medium-high 6: direct tests for the
    // split-step behaviors flagged by spec line 882 (the highest-
    // risk surface in the auction core auction flow). Each test exercises a
    // distinct spec rule that the helper-only tests above don't
    // cover. The helpers below build the minimal request fixtures
    // each test needs.

    fn dummy_registry(disabled: &[&str]) -> BidderRegistry {
        let mut r = BidderRegistry::new();
        for name in ["appnexus", "pubmatic", "generic"] {
            let info: &'static auctioneer_config::BidderInfo =
                Box::leak(Box::new(auctioneer_config::BidderInfo {
                    name: (*name).into(),
                    endpoint: "https://x".into(),
                    uses_https: true,
                    disabled: disabled.contains(name),
                    ..auctioneer_config::BidderInfo::default()
                }));
            r.register(info, Box::new(crate::Dummy));
        }
        r
    }

    fn aliases() -> AliasMap { AliasMap::default() }

    fn req_with_multibidder_imp() -> BidRequest {
        let mut bidders = serde_json::Map::new();
        bidders.insert("appnexus".into(), serde_json::json!({"placementId": 42}));
        bidders.insert("pubmatic".into(), serde_json::json!({"publisherId": "P-1"}));
        BidRequest {
            id: "auc".into(),
            imp: vec![auctioneer_openrtb::types::bid_request::Imp {
                id: "i-1".into(),
                ext: Some(auctioneer_openrtb::ext::imp::ImpExt {
                    bidder: Some(serde_json::Value::Object(bidders)),
                    ..auctioneer_openrtb::ext::imp::ImpExt::default()
                }),
                ..auctioneer_openrtb::types::bid_request::Imp::default()
            }],
            ..BidRequest::default()
        }
    }

    #[test]
    fn split_strips_competitor_bidder_params() {
        // Round-23 review Medium-high 6 + round-22 review High 4:
        // the bidder-scoped imp.ext.prebid must NOT carry other
        // bidders' params. After split, an imp routed to
        // `appnexus` must not contain `pubmatic`'s `publisherId`.
        let registry = dummy_registry(&[]);
        let split = split_by_bidder(&req_with_multibidder_imp(), &aliases(), &registry).unwrap();
        let appnexus = split.per_bidder.iter()
            .find(|p| p.bidder.as_str() == "appnexus")
            .expect("appnexus participates");
        let wire = serde_json::to_string(&appnexus.request).unwrap();
        assert!(wire.contains("placementId"), "expected appnexus params, got: {wire}");
        assert!(!wire.contains("publisherId"), "leaked competitor params: {wire}");
        assert!(!wire.contains("\"pubmatic\""), "leaked competitor name: {wire}");
    }

    #[test]
    fn split_emits_code_6_for_disabled_bidder() {
        // Spec line 558: disabled bidders are removed with a
        // code-6 BidderTemporarilyDisabled warning.
        let registry = dummy_registry(&["pubmatic"]);
        let split = split_by_bidder(&req_with_multibidder_imp(), &aliases(), &registry).unwrap();
        let warnings = split.disabled_bidder_warnings.iter()
            .filter(|(b, _)| b.as_str() == "pubmatic")
            .collect::<Vec<_>>();
        assert_eq!(warnings.len(), 1, "expected one code-6 warning, got {warnings:?}");
        assert_eq!(warnings[0].1.code, 6);
        // The disabled bidder should NOT appear in per_bidder.
        assert!(split.per_bidder.iter().all(|p| p.bidder.as_str() != "pubmatic"));
    }

    #[test]
    fn split_request_level_bidderparams_do_not_create_participation() {
        // Spec line 882: request-level `ext.prebid.bidderparams`
        // SUPPLEMENTS per-imp params but does NOT create a
        // participating bidder. An imp that mentions only `appnexus`
        // must not produce a `pubmatic` entry even if a
        // `bidderparams.pubmatic` block exists.
        let registry = dummy_registry(&[]);
        let mut req = req_with_multibidder_imp();
        // Trim the imp's bidders down to just appnexus.
        if let Some(ext) = req.imp[0].ext.as_mut() {
            if let Some(serde_json::Value::Object(obj)) = ext.bidder.as_mut() {
                obj.remove("pubmatic");
            }
        }
        // Add a request-level pubmatic bidderparams entry.
        let mut prebid = auctioneer_openrtb::ext::request_prebid::ExtRequestPrebid::default();
        prebid.bidderparams.insert("pubmatic".into(), serde_json::json!({"publisherId": "P-2"}));
        req.ext = Some(auctioneer_openrtb::types::bid_request::BidRequestExt {
            prebid: Some(prebid),
            ..auctioneer_openrtb::types::bid_request::BidRequestExt::default()
        });
        let split = split_by_bidder(&req, &aliases(), &registry).unwrap();
        // pubmatic supplied bidderparams but NOT a per-imp entry,
        // so it MUST NOT participate.
        assert!(
            split.per_bidder.iter().all(|p| p.bidder.as_str() != "pubmatic"),
            "request-level bidderparams must not create participation",
        );
    }

    #[test]
    fn split_no_valid_bidders_returns_bad_input() {
        // Spec line 882 / step 7 (f): if NO bidder survives the
        // participation filter for an imp, the auction is rejected
        // with AuctionError::BadInput.
        let registry = dummy_registry(&["appnexus", "pubmatic", "generic"]);
        let err = split_by_bidder(&req_with_multibidder_imp(), &aliases(), &registry).unwrap_err();
        assert!(matches!(err, AuctionError::BadInput(_)),
            "expected BadInput when no participating bidders survive, got {err:?}");
    }

    // (Code-15 InvalidImpFirstPartyData is tested at the integration
    // layer in Task 32's run_auction snapshot — the bidder-scoped
    // imp re-validation step requires the foundations validator surface and
    // a malformed FPD fixture that's clearer at that level.)
}
```

Wire into `lib.rs`:

```rust
mod split;
pub use split::{PerBidderRequest, SplitBidders, split_by_bidder};
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-exchange split::tests
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: per-bidder request split + merge helpers"
```

---

### Task 27: `limits.rs` — max_bidders_per_auction + max_outbound_requests enforcement

**Files:**

- Modify: `crates/auctioneer-exchange/src/limits.rs`

**Interfaces:**

- Consumes: `Vec<PerBidderRequest>`, `Vec<BidderHttpRequest>` (post-make_requests), `&HostConfig`.
- Produces: `pub fn apply_max_bidders(per_bidder, cap)` truncates in §5.9 registry order; `pub fn apply_max_outbound(calls, cap)` keeps the first N total calls in per-bidder registry order; both return the dropped entries so callers can emit warnings (code **10999 Unknown**, partial-bidder semantics per §5.3).

- [ ] **Step 1: Replace `src/limits.rs`**

```rust
//! `max_bidders_per_auction` + `max_outbound_requests` enforcement.
//! See spec §5.3 partial-bidder semantics.

use crate::split::PerBidderRequest;
use auctioneer_bidder::{BidderHttpRequest, BidderName};

#[derive(Debug)]
pub struct BidderCapDrop {
    pub bidder: BidderName,
    /// `N` from the warning message format.
    pub cap: usize,
}

#[derive(Debug)]
pub struct OutboundCapDrop {
    pub bidder: BidderName,
    /// Count of requests dropped under the bidder's call list (0 means
    /// the bidder is entirely dropped, otherwise it's the truncation
    /// count to surface in the warning text).
    pub dropped_count: usize,
}

#[must_use]
pub fn apply_max_bidders(
    mut per_bidder: Vec<PerBidderRequest>,
    cap: usize,
) -> (Vec<PerBidderRequest>, Vec<BidderCapDrop>) {
    if cap == 0 || per_bidder.len() <= cap {
        return (per_bidder, vec![]);
    }
    // Vec is already in §5.9 registry order by construction (BTreeMap
    // in split.rs iterates in BidderName Ord order).
    let dropped = per_bidder.split_off(cap);
    let drops = dropped.into_iter().map(|p| BidderCapDrop {
        bidder: p.bidder,
        cap,
    }).collect();
    (per_bidder, drops)
}

#[must_use]
pub fn apply_max_outbound(
    pairs: Vec<(BidderName, Vec<BidderHttpRequest>)>,
    cap: usize,
) -> (Vec<(BidderName, Vec<BidderHttpRequest>)>, Vec<OutboundCapDrop>) {
    if cap == 0 {
        return (pairs, vec![]);
    }
    let mut kept: Vec<(BidderName, Vec<BidderHttpRequest>)> = Vec::new();
    let mut drops: Vec<OutboundCapDrop> = Vec::new();
    let mut remaining = cap;

    for (bidder, calls) in pairs {
        if remaining == 0 {
            drops.push(OutboundCapDrop {
                bidder: bidder.clone(),
                dropped_count: calls.len(),
            });
            // Still emit an empty entry so the caller can attribute the
            // bidder's status to the original participating set.
            kept.push((bidder, Vec::new()));
            continue;
        }
        let take = calls.len().min(remaining);
        if take < calls.len() {
            drops.push(OutboundCapDrop {
                bidder: bidder.clone(),
                dropped_count: calls.len() - take,
            });
        }
        let mut taken = calls;
        taken.truncate(take);
        remaining -= take;
        kept.push((bidder, taken));
    }

    (kept, drops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_openrtb::types::bid_request::BidRequest;

    fn pbr(name: &str) -> PerBidderRequest {
        // Round-24 review Blocker 3: `PerBidderRequest` carries only
        // `bidder`, `base`, and `request` — the earlier `warnings`
        // field was lifted out to `SplitBidders.disabled_bidder_warnings`
        // during the round-20 fix so disabled-bidder warnings could
        // survive the apply_max_bidders / privacy filter passes. The
        // pre-fix test fixture below still constructed it and broke
        // `cargo test -p auctioneer-exchange limits::tests`.
        PerBidderRequest {
            bidder: BidderName::new(name),
            base: BidderName::new(name),
            request: BidRequest::default(),
        }
    }

    fn call(url: &str) -> BidderHttpRequest {
        BidderHttpRequest {
            method: "POST".into(),
            url: url.into(),
            headers: vec![],
            body: vec![],
            covered_imp_ids: vec![],
        }
    }

    #[test]
    fn apply_max_bidders_under_cap_is_noop() {
        let input = vec![pbr("a"), pbr("b")];
        let (kept, drops) = apply_max_bidders(input, 5);
        assert_eq!(kept.len(), 2);
        assert!(drops.is_empty());
    }

    #[test]
    fn apply_max_bidders_truncates_to_cap() {
        let input = vec![pbr("a"), pbr("b"), pbr("c"), pbr("d")];
        let (kept, drops) = apply_max_bidders(input, 2);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].bidder.as_str(), "a");
        assert_eq!(kept[1].bidder.as_str(), "b");
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[0].bidder.as_str(), "c");
        assert_eq!(drops[1].bidder.as_str(), "d");
    }

    #[test]
    fn apply_max_outbound_truncates_partial_bidder_calls() {
        let input = vec![
            (BidderName::new("a"), vec![call("u1"), call("u2"), call("u3")]),
            (BidderName::new("b"), vec![call("u4"), call("u5")]),
        ];
        let (kept, drops) = apply_max_outbound(input, 4);
        // 'a' keeps 3, 'b' keeps 1, drop 1 from 'b'.
        assert_eq!(kept[0].1.len(), 3);
        assert_eq!(kept[1].1.len(), 1);
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].bidder.as_str(), "b");
        assert_eq!(drops[0].dropped_count, 1);
    }

    #[test]
    fn apply_max_outbound_drops_entire_later_bidder_when_cap_exhausted() {
        let input = vec![
            (BidderName::new("a"), vec![call("u1"), call("u2")]),
            (BidderName::new("b"), vec![call("u3"), call("u4")]),
        ];
        let (kept, drops) = apply_max_outbound(input, 2);
        assert_eq!(kept[0].1.len(), 2);
        assert_eq!(kept[1].1.len(), 0);
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].bidder.as_str(), "b");
        assert_eq!(drops[0].dropped_count, 2);
    }
}
```

Wire `lib.rs`:

```rust
mod limits;
pub use limits::{BidderCapDrop, OutboundCapDrop, apply_max_bidders, apply_max_outbound};
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-exchange limits::tests
```

Expected: 4 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: resource-limit cap enforcement"
```

---

### Task 28: Price granularity + bucket computation (Appendix E.1 / E.2)

**Files:**

- Modify: `crates/auctioneer-exchange/src/targeting/mod.rs`
- Modify: `crates/auctioneer-exchange/src/targeting/price_bucket.rs`

**Interfaces:**

- Consumes: `f64` CPM, `PriceGranularity`, `BidRounding`, `&dyn Rng`, plus `&AccountConfig` for the account-level fallbacks Task 32 reads.
- Produces: `pub enum PriceGranularity { Low, Med, High, Auto, Dense, Custom { precision: u8, ranges: Vec<Range> } }`; `pub struct Range { min, max, increment }`; `pub enum BidRounding { Down, Up, Nearest, TimeSplit }`; `pub fn price_bucket(cpm: f64, g: &PriceGranularity, mode: BidRounding, rng: &dyn Rng) -> String`; round-18 review Blocker 6 — ALSO `pub fn parse_price_granularity(value: &serde_json::Value) -> PriceGranularity`, `impl PriceGranularity { pub fn from_account(account: &AccountConfig) -> Self }`, and `impl BidRounding { pub fn from_account(account: &AccountConfig) -> Self }`. These are the helpers run_auction (Task 32) imports from `crate::targeting`; the earlier draft only sketched the four core enums.

- [ ] **Step 1: Implement `targeting/mod.rs`**

```rust
pub mod price_bucket;
pub mod keys;
pub mod winner;

// Round-18 review Blocker 4: re-export every targeting symbol that
// `run_auction` (Task 32) imports as `crate::targeting::...`. The
// earlier targeting/mod.rs only exposed TargetingKeys / Winner /
// is_new_winning_bid; build_keys, TargetingInput, pick_winner, and
// the price_bucket helpers MUST be `pub use`d at the module root
// or Task 32 fails to compile.
pub use price_bucket::{
    parse_price_granularity, price_bucket, BidRounding, PriceGranularity, Range,
};
pub use keys::{build_keys, TargetingInput, TargetingKeys};
pub use winner::{is_new_winning_bid, pick_winner, Winner};
```

- [ ] **Step 2: Implement `targeting/price_bucket.rs`**

```rust
//! Price granularity + bucket computation. See Appendix E.1 + E.2.

use auctioneer_runtime::{Rng, gen_range};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidRounding { Down, Up, Nearest, TimeSplit }

#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub increment: f64,
}

#[derive(Clone, Debug)]
pub enum PriceGranularity {
    Low,
    Med,
    High,
    Auto,
    Dense,
    Custom { precision: u8, ranges: Vec<Range> },
}

impl PriceGranularity {
    /// Round-18 review Blocker 6: account-level fallback. Account
    /// `targeting.pricegranularity` is a string OR object; auction core's
    /// `AccountConfig` does not yet model that field, so the auction core dev
    /// fallback is `Med`. GDPR enforcement widens `AccountConfig` and revisits.
    /// Spec §5.6 + Appendix E.1.
    #[must_use]
    pub fn from_account(_account: &auctioneer_config::AccountConfig) -> Self {
        Self::Med
    }
    fn ranges(&self) -> (u8, Vec<Range>) {
        match self {
            Self::Low => (2, vec![Range { min: 0.0, max: 5.0,  increment: 0.5  }]),
            Self::Med => (2, vec![Range { min: 0.0, max: 20.0, increment: 0.1  }]),
            Self::High=> (2, vec![Range { min: 0.0, max: 20.0, increment: 0.01 }]),
            Self::Auto=> (2, vec![
                Range { min: 0.0,  max: 5.0,  increment: 0.05 },
                Range { min: 5.0,  max: 10.0, increment: 0.1  },
                Range { min: 10.0, max: 20.0, increment: 0.5  },
            ]),
            Self::Dense=> (2, vec![
                Range { min: 0.0, max: 3.0,  increment: 0.01 },
                Range { min: 3.0, max: 8.0,  increment: 0.05 },
                Range { min: 8.0, max: 20.0, increment: 0.5  },
            ]),
            Self::Custom { precision, ranges } => (*precision, ranges.clone()),
        }
    }
}

#[must_use]
pub fn price_bucket(cpm: f64, g: &PriceGranularity, mode: BidRounding, rng: &dyn Rng) -> String {
    if cpm.is_nan() || cpm < 0.0 {
        return String::new();
    }
    let (precision, ranges) = g.ranges();
    let bucket_max = ranges.iter().map(|r| r.max).fold(f64::NEG_INFINITY, f64::max);
    if cpm > bucket_max {
        return format_at(bucket_max, precision);
    }
    // Find the range with min <= cpm <= max. On exact boundary the later range wins.
    let mut chosen: Option<Range> = None;
    for r in &ranges {
        if cpm >= r.min && cpm <= r.max {
            chosen = Some(*r);
        }
    }
    let r = match chosen {
        Some(x) => x,
        None    => return String::new(),
    };
    let increments = (cpm - r.min) / r.increment;
    let rounded = match mode {
        BidRounding::Down    => increments.floor(),
        BidRounding::Up      => increments.ceil(),
        BidRounding::Nearest => increments.round(),
        BidRounding::TimeSplit => {
            // Random floor/ceil per bid drawn from injected Rng. Use a
            // deterministic 50/50 split via gen_range(.., 2).
            if gen_range(rng, NonZeroU64::new(2).unwrap()) == 0 {
                increments.floor()
            } else {
                increments.ceil()
            }
        }
    };
    let bucket = rounded * r.increment + r.min;
    format_at(bucket, precision)
}

fn format_at(v: f64, precision: u8) -> String {
    format!("{:.*}", usize::from(precision), v)
}

impl BidRounding {
    /// Round-18 review Blocker 6: account-level fallback per spec
    /// §5.6 — the `bid_rounding` string on `AccountConfig` (already
    /// shipped on foundations) maps to one of `Down` / `Up` / `Nearest` /
    /// `TimeSplit`. Unknown values fall back to `Down` (the upstream
    /// reference auction handling default).
    #[must_use]
    pub fn from_account(account: &auctioneer_config::AccountConfig) -> Self {
        match account.bid_rounding.to_ascii_lowercase().as_str() {
            "up"        | "ceil"    => Self::Up,
            // Round-20 review Spec 10: spec line 5228 maps the
            // legacy boolean `true` to nearest rounding. Earlier
            // drafts dropped the alias.
            "nearest"   | "round"   | "true" => Self::Nearest,
            "timesplit" | "time_split" => Self::TimeSplit,
            _ => Self::Down,
        }
    }
}

/// Round-18 review Blocker 6 + round-19 review High 8: request-
/// level `pricegranularity` JSON per Appendix E.1. The pinned spec
/// (line 5207) specifies:
///   - string aliases: `low | med | medium | high | auto | dense`
///     (case-insensitive; `med` and `medium` are interchangeable
///     per upstream reference targeting),
///   - custom-bucket shape `{"precision": N, "ranges": [...]}`,
///   - precision MUST be in `0..=15`; values outside the range
///     clamp to the nearest endpoint,
///   - ranges MUST have strictly increasing `max`,
///   - a missing `min` defaults to the previous range's `max` (or
///     0.0 for the first range), NOT a blanket 0.0.
///
/// Unrecognized shapes default to `Med` so a malformed request
/// still produces deterministic targeting keys (Appendix E.7
/// byte-stable requirement).
#[must_use]
pub fn parse_price_granularity(value: &serde_json::Value) -> PriceGranularity {
    if let Some(name) = value.as_str() {
        return match name.to_ascii_lowercase().as_str() {
            "low"            => PriceGranularity::Low,
            "med" | "medium" => PriceGranularity::Med,
            "high"           => PriceGranularity::High,
            "auto"           => PriceGranularity::Auto,
            "dense"          => PriceGranularity::Dense,
            _                => PriceGranularity::Med,
        };
    }
    let Some(obj) = value.as_object() else {
        return PriceGranularity::Med;
    };
    // Precision: spec says 0..=15; out-of-range clamps to the
    // nearest endpoint.
    let precision = obj.get("precision")
        .and_then(serde_json::Value::as_u64)
        // Round-20 review Spec 11: `n.min(15)` is in 0..=15 so the
        // u8 cast cannot truncate; use try_from for the clippy
        // `-D restriction` gate anyway.
        .map(|n| u8::try_from(n.min(15)).unwrap_or(2))
        .unwrap_or(2);
    let Some(ranges_json) = obj.get("ranges").and_then(|v| v.as_array()) else {
        return PriceGranularity::Med;
    };
    let mut ranges: Vec<Range> = Vec::new();
    let mut previous_max = 0.0_f64;
    for entry in ranges_json {
        let Some(row) = entry.as_object() else { continue; };
        let max = row.get("max").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let inc = row.get("increment").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        if max <= 0.0 || inc <= 0.0 { continue; }
        // Spec: strictly increasing `max`. Drop the bucket if it
        // does not advance the high-water mark.
        if max <= previous_max { continue; }
        // Spec: missing `min` defaults to the previous range's `max`
        // (or 0.0 for the first range), NOT a blanket 0.0.
        let min = row.get("min").and_then(serde_json::Value::as_f64).unwrap_or(previous_max);
        ranges.push(Range { min, max, increment: inc });
        previous_max = max;
    }
    if ranges.is_empty() {
        return PriceGranularity::Med;
    }
    PriceGranularity::Custom { precision, ranges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_runtime::SeededRng;

    fn rng() -> SeededRng { SeededRng::new(1) }

    #[test]
    fn med_bucket_3_12_floors_to_3_10() {
        assert_eq!(
            price_bucket(3.12, &PriceGranularity::Med, BidRounding::Down, &rng()),
            "3.10",
        );
    }

    #[test]
    fn med_bucket_above_max_returns_max() {
        assert_eq!(
            price_bucket(25.0, &PriceGranularity::Med, BidRounding::Down, &rng()),
            "20.00",
        );
    }

    #[test]
    fn auto_picks_correct_range() {
        // 7.4 → in (5, 10) range with 0.1 → 7.40.
        assert_eq!(
            price_bucket(7.4, &PriceGranularity::Auto, BidRounding::Down, &rng()),
            "7.40",
        );
    }

    #[test]
    fn high_bucket_uses_001() {
        assert_eq!(
            price_bucket(0.123, &PriceGranularity::High, BidRounding::Down, &rng()),
            "0.12",
        );
    }

    #[test]
    fn negative_cpm_returns_empty() {
        assert_eq!(
            price_bucket(-1.0, &PriceGranularity::Med, BidRounding::Down, &rng()),
            "",
        );
    }

    #[test]
    fn timesplit_is_deterministic_under_seeded_rng() {
        let a = price_bucket(3.13, &PriceGranularity::Med, BidRounding::TimeSplit, &SeededRng::new(42));
        let b = price_bucket(3.13, &PriceGranularity::Med, BidRounding::TimeSplit, &SeededRng::new(42));
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 3: Test**

```
cargo test -p auctioneer-exchange targeting::price_bucket::tests
```

Expected: 6 passed.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-exchange/src/targeting
git commit -m "auctioneer-exchange: price granularity + bucket computation (Appendix E.1/E.2)"
```

---

### Task 29: Targeting keys (Appendix E.3)

**Files:**

- Modify: `crates/auctioneer-exchange/src/targeting/keys.rs`

**Interfaces:**

- Consumes: an emitted `Bid` + price-bucket string + bidder name + account-config truncation hints.
- Produces: `pub struct TargetingKeys { pub winner: BTreeMap<String, String>, pub per_bidder: BTreeMap<String, String>, pub warnings: Vec<ErrorEntry> }`; `pub fn build_keys(...) -> TargetingKeys`. Honors `MinKeyLength = 12`, `MaxKeyLength = 20`, prefix fallback to `"hb"` with a `TooLongTargetingPrefix` warning (Appendix G code 10015) emitted via `TargetingKeys.warnings`. Round-13 review Medium 8: previously the interface promised the warning but only returned `winner` + `per_bidder` maps — `warnings` is added so the run_auction caller can fold them into `bundle.warnings`.

- [ ] **Step 1: Implement + test**

Replace `src/targeting/keys.rs`:

```rust
//! Targeting-key emission. See Appendix E.3.

// Round-18 review Blocker 4: ErrorEntry must be imported for the
// `TargetingKeys.warnings: Vec<ErrorEntry>` field. It lives in the
// response module alongside the other wire-shaped helper structs.
use crate::response::ErrorEntry;
use std::collections::BTreeMap;

const MIN_KEY_LENGTH: usize = 12;
const MAX_KEY_LENGTH: usize = 20;

#[derive(Debug, Default)]
pub struct TargetingKeys {
    /// Un-suffixed winner keys (`hb_pb`, `hb_bidder`, …) emitted only for
    /// the overall imp winner.
    pub winner: BTreeMap<String, String>,
    /// Per-bidder keys (`hb_pb_<bidder>`, …).
    pub per_bidder: BTreeMap<String, String>,
    /// Round-13 review Medium 8: Appendix G warnings emitted during
    /// key generation. Currently used for the prefix-fallback case
    /// (`TooLongTargetingPrefix`, code 10015). The caller folds
    /// these into the per-bidder bundle warnings.
    pub warnings: Vec<ErrorEntry>,
}

#[derive(Clone, Debug)]
pub struct TargetingInput<'a> {
    pub prefix: &'a str,
    pub truncate_target_attribute: Option<usize>,
    pub bidder: &'a str,
    pub price_bucket: &'a str,
    pub size_wh: Option<(u32, u32)>,
    pub deal_id: Option<&'a str>,
    pub env: Option<&'a str>,
    pub include_winners: bool,
    pub include_bidder_keys: bool,
    pub include_format: bool,
    /// Round-19 review High 7: spec §5.6 line 5238 — per-bidder keys
    /// are emitted when `include_bidder_keys` OR
    /// (`always_include_deals` AND the bid carries a deal). This
    /// field carries the request-level `alwaysincludedeals` flag
    /// through to build_keys's OR check.
    pub always_include_deals: bool,
    pub format: Option<&'a str>,
    pub is_overall_winner: bool,
}

#[must_use]
pub fn build_keys(input: &TargetingInput<'_>) -> TargetingKeys {
    let mut out = TargetingKeys::default();

    // Effective prefix + max-len. Round-13 review Medium 8: when the
    // configured prefix is too long for the truncation cap, the
    // resolver falls back to "hb" and emits an Appendix G code 10015
    // TooLongTargetingPrefix warning the caller surfaces under
    // `ext.warnings[bidder]`.
    let (effective_prefix, max_len, fallback_warning) =
        resolve_prefix_and_max(input.prefix, input.truncate_target_attribute);
    if let Some(warning) = fallback_warning {
        out.warnings.push(warning);
    }

    // Build the per-bid key set.
    let mut pairs: Vec<(&str, String)> = Vec::new();
    pairs.push(("_pb", input.price_bucket.to_owned()));
    pairs.push(("_bidder", input.bidder.to_owned()));
    if let Some((w, h)) = input.size_wh {
        if w > 0 && h > 0 {
            pairs.push(("_size", format!("{w}x{h}")));
        }
    }
    if let Some(deal) = input.deal_id {
        if !deal.is_empty() {
            pairs.push(("_deal", deal.into()));
        }
    }
    if let Some(env) = input.env {
        pairs.push(("_env", env.into()));
    }
    if input.include_format {
        if let Some(fmt) = input.format {
            pairs.push(("_format", fmt.into()));
        }
    }

    // Round-19 review High 7: per-bidder keys are emitted when
    // `include_bidder_keys` OR (`always_include_deals` AND the bid
    // carries a deal). The earlier draft only honored
    // `include_bidder_keys`.
    let has_deal = input.deal_id.is_some_and(|d| !d.is_empty());
    let emit_per_bidder = input.include_bidder_keys
        || (input.always_include_deals && has_deal);

    for (suffix, value) in pairs {
        // Winner key (un-suffixed).
        if input.include_winners && input.is_overall_winner {
            let k = truncate(format!("{effective_prefix}{suffix}"), max_len);
            out.winner.insert(k, value.clone());
        }
        // Per-bidder key.
        if emit_per_bidder {
            let k = truncate(format!("{effective_prefix}{suffix}_{}", input.bidder), max_len);
            out.per_bidder.insert(k, value);
        }
    }

    out
}

fn resolve_prefix_and_max(
    prefix: &str,
    truncate: Option<usize>,
) -> (String, usize, Option<ErrorEntry>) {
    let cap = truncate.filter(|n| *n >= MIN_KEY_LENGTH).unwrap_or(MAX_KEY_LENGTH);
    // If even the shortest possible key (prefix + _pb + 0 suffix) would
    // exceed the cap, fall back to "hb" (Appendix E.3) AND emit the
    // TooLongTargetingPrefix warning (Appendix G code 10015).
    let smallest_per_bidder = prefix.len() + 4;  // "_pb_X"
    if smallest_per_bidder > cap {
        let warning = ErrorEntry {
            // Round-18 review Blocker 5: Appendix G assigns code
            // 10015 to `TooLongTargetingPrefix`; 10001 is reserved
            // for `InvalidPrivacyConsent`. The earlier draft used
            // the wrong code.
            code: 10015,
            message: format!(
                "configured targeting prefix {prefix:?} exceeds the {cap}-character cap; \
                 falling back to \"hb\""
            ),
        };
        return ("hb".into(), cap, Some(warning));
    }
    (prefix.into(), cap, None)
}

fn truncate(mut s: String, max_len: usize) -> String {
    if s.len() > max_len {
        s.truncate(max_len);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_for(prefix: &str, bidder: &str) -> TargetingInput<'_> {
        TargetingInput {
            prefix,
            truncate_target_attribute: None,
            bidder,
            price_bucket: "1.20",
            size_wh: Some((300, 250)),
            deal_id: None,
            env: None,
            include_winners: true,
            include_bidder_keys: true,
            include_format: false,
            // Round-19 review High 7: deals override stays off by
            // default; the dedicated test below flips it on.
            always_include_deals: false,
            format: None,
            is_overall_winner: true,
        }
    }

    #[test]
    fn emits_winner_and_per_bidder_pb() {
        let out = build_keys(&input_for("hb", "appnexus"));
        assert_eq!(out.winner.get("hb_pb").map(String::as_str), Some("1.20"));
        assert_eq!(out.per_bidder.get("hb_pb_appnexus").map(String::as_str), Some("1.20"));
    }

    #[test]
    fn emits_size_only_when_both_dims_positive() {
        let mut input = input_for("hb", "ax");
        input.size_wh = Some((0, 250));
        let out = build_keys(&input);
        assert!(!out.winner.contains_key("hb_size"));
    }

    #[test]
    fn prefix_too_long_falls_back_to_hb() {
        let mut input = input_for("verylongprefix_abc", "ax");
        input.truncate_target_attribute = Some(16);
        let out = build_keys(&input);
        // hb_pb under the cap.
        assert!(out.per_bidder.contains_key("hb_pb_ax"));
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-exchange targeting::keys::tests
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/src/targeting/keys.rs
git commit -m "auctioneer-exchange: targeting keys (Appendix E.3)"
```

---

### Task 30: Winner selection (Appendix E.4)

**Files:**

- Modify: `crates/auctioneer-exchange/src/targeting/winner.rs`

**Interfaces:**

- Consumes: collected bids (`Vec<(BidderName, TypedBid)>` per imp), `preferdeals` flag, §5.9 registry order.
- Produces: `pub struct Winner { bidder: BidderName, bid_index: usize }`; `pub fn is_new_winning_bid(incumbent, candidate, prefer_deals) -> bool`. Tie-breaking on equal price keeps the incumbent — and because bidders are processed in §5.9 registry order, the incumbent is well-defined.

- [ ] **Step 1: Implement + test**

Replace `src/targeting/winner.rs`:

```rust
//! Winner selection. See Appendix E.4.

use auctioneer_bidder::{BidderName, TypedBid};

#[derive(Clone, Debug)]
pub struct Winner {
    pub bidder: BidderName,
    pub bid_index: usize,
    pub price: f64,
    pub has_deal: bool,
}

/// Returns `true` if `candidate` beats `incumbent` per Appendix E.4.
#[must_use]
pub fn is_new_winning_bid(
    incumbent: Option<&Winner>,
    candidate: &Winner,
    prefer_deals: bool,
) -> bool {
    let Some(inc) = incumbent else { return true; };
    if prefer_deals {
        match (inc.has_deal, candidate.has_deal) {
            (false, true) => return true,
            (true, false) => return false,
            _ => {}
        }
    }
    candidate.price > inc.price
}

/// Per-imp winner reduction. Bidders/bids are presented in §5.9 registry
/// order (caller's responsibility).
#[must_use]
pub fn pick_winner<'a>(
    bidder_bids: &'a [(BidderName, Vec<TypedBid>)],
    imp_id: &str,
    prefer_deals: bool,
) -> Option<Winner> {
    let mut incumbent: Option<Winner> = None;
    for (bidder, bids) in bidder_bids {
        for (idx, b) in bids.iter().enumerate() {
            if b.bid.impid != imp_id { continue; }
            let cand = Winner {
                bidder: bidder.clone(),
                bid_index: idx,
                price: b.bid.price,
                // Round-19 review Blocker 5: foundations `Bid` exposes the field
                // as `deal_id` (with `#[serde(rename = "dealid")]`);
                // earlier drafts called the field `dealid` directly.
                has_deal: !b.bid.deal_id.as_ref().map_or("", String::as_str).is_empty(),
            };
            if is_new_winning_bid(incumbent.as_ref(), &cand, prefer_deals) {
                incumbent = Some(cand);
            }
        }
    }
    incumbent
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_config::MediaType;
    use auctioneer_openrtb::types::bid_response::Bid;

    fn bid(impid: &str, price: f64, deal: Option<&str>) -> TypedBid {
        TypedBid {
            bid: Bid {
                id: "b".into(),
                impid: impid.into(),
                price,
                deal_id: deal.map(String::from),
                ..Bid::default()
            },
            media_type: MediaType::Banner,
        }
    }

    #[test]
    fn higher_price_wins_without_prefer_deals() {
        let bb = vec![
            (BidderName::new("a"), vec![bid("imp1", 1.0, None)]),
            (BidderName::new("b"), vec![bid("imp1", 2.0, None)]),
        ];
        let w = pick_winner(&bb, "imp1", false).unwrap();
        assert_eq!(w.bidder.as_str(), "b");
        assert_eq!(w.price, 2.0);
    }

    #[test]
    fn tie_keeps_incumbent_via_registry_order() {
        let bb = vec![
            (BidderName::new("a"), vec![bid("imp1", 1.0, None)]),
            (BidderName::new("b"), vec![bid("imp1", 1.0, None)]),
        ];
        let w = pick_winner(&bb, "imp1", false).unwrap();
        // 'a' is the incumbent (lex-smaller); tie keeps it.
        assert_eq!(w.bidder.as_str(), "a");
    }

    #[test]
    fn prefer_deals_beats_higher_no_deal() {
        let bb = vec![
            (BidderName::new("a"), vec![bid("imp1", 5.0, None)]),
            (BidderName::new("b"), vec![bid("imp1", 1.0, Some("D-1"))]),
        ];
        let w = pick_winner(&bb, "imp1", true).unwrap();
        assert_eq!(w.bidder.as_str(), "b");
    }
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-exchange targeting::winner::tests
```

Expected: 3 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/src/targeting/winner.rs
git commit -m "auctioneer-exchange: winner selection (Appendix E.4)"
```

---

### Task 31: Deterministic response builder (Appendix E.5 / E.6 / E.7) + responsetime aggregation

**Files:**

- Modify: `crates/auctioneer-exchange/src/response.rs`
- Modify: `crates/auctioneer-exchange/src/responsetime.rs`

**Interfaces:**

- Consumes: per-bidder collected results, the §5.9 ordering, `req.ext.prebid.returnallbidstatus`, `bidder_visible_tmax_ms`, `&dyn Clock`.
- Produces: `pub fn build_response(req, ctx, results) -> BidResponse` (deterministic shape per Appendix E.5 / E.6 / E.7); `pub struct ResponseTimeStat { make_requests, max_call_elapsed, make_bids }` with `pub fn total_ms(&self) -> u32` per §5.2.

- [ ] **Step 1: Implement `responsetime.rs`**

```rust
//! Per-bidder `responsetimemillis` aggregation. See spec §5.2.

use std::time::Duration;

#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseTimeStat {
    pub make_requests: Duration,
    pub max_call_elapsed: Duration,
    pub make_bids: Duration,
}

impl ResponseTimeStat {
    #[must_use]
    pub fn total_ms(&self) -> u32 {
        let total = self.make_requests + self.max_call_elapsed + self.make_bids;
        u32::try_from(total.as_millis()).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn total_sums_three_segments() {
        let s = ResponseTimeStat {
            make_requests: Duration::from_millis(5),
            max_call_elapsed: Duration::from_millis(100),
            make_bids: Duration::from_millis(2),
        };
        assert_eq!(s.total_ms(), 107);
    }
}
```

- [ ] **Step 2: Implement `response.rs`** — full deterministic builder

```rust
//! Deterministic `BidResponse` builder. See Appendix E.5 / E.6 / E.7.
//!
//! Layout invariants (Appendix E.7):
//! - `seatbid` sorted by `seat` (ascending UTF-8).
//! - `ext.errors` / `ext.warnings` / `ext.responsetimemillis` serialize as
//!   sorted-key maps. We use `BTreeMap` to encode this in the type.
//! - Bidders are walked in §5.9 registry order (the caller hands them in
//!   already sorted; we never re-shuffle).

use crate::responsetime::ResponseTimeStat;
use auctioneer_bidder::{BidderName, TypedBid};
use auctioneer_openrtb::types::bid_response::{Bid, BidResponse, SeatBid};
use std::collections::BTreeMap;

/// Per-bidder material the response builder needs. Keyed by `BidderName`
/// in the caller's BTreeMap so iteration is lexicographic.
#[derive(Default)]
pub struct BidderResultBundle {
    pub bids: Vec<TypedBid>,
    pub errors: Vec<ErrorEntry>,
    pub warnings: Vec<ErrorEntry>,
    pub timing: ResponseTimeStat,
    /// True iff at least one outbound call dispatched (i.e. the bidder
    /// produced a `make_requests` result that survived limits + privacy).
    pub dispatched: bool,
}

// Round-13 review Blocker 2: ErrorEntry, NonBidRow, and the other
// helper structs serialized into ext.* are intentionally wire-shaped
// — they MUST derive Serialize (and Deserialize for round-trip
// tests). The previous draft serialized them via serde_json::to_value
// without the derive, which would fail at compile time.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ErrorEntry { pub code: u32, pub message: String }

// Round-24 review Blocker 4: this struct used to carry a `<'a>`
// parameter (anticipating borrowed slices), but every field is owned
// — `BTreeMap`, `Vec`, primitives. Rust rejects unused lifetime
// parameters with `unused_lifetimes`, so the declaration must NOT
// name one. Call sites use the owned form `ResponseInput { ... }`
// and `build_response` signs `ResponseInput<'_>`... see below.
pub struct ResponseInput {
    pub bidder_visible_tmax_ms: u32,
    pub auction_timestamp_ms: u64,
    pub return_all_bid_status: bool,
    /// Caller-built `seatnonbid` rows (empty unless `return_all_bid_status`).
    pub seatnonbid: BTreeMap<BidderName, Vec<NonBidRow>>,
    pub bundles: BTreeMap<BidderName, BidderResultBundle>,
    /// auction-level errors / warnings (e.g. code 12 `TmaxTimeout`).
    pub prebid_errors:   Vec<ErrorEntry>,
    pub prebid_warnings: Vec<ErrorEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NonBidRow { pub imp_id: String, pub statuscode: u32 }

#[must_use]
// Round-23 review High 4: `build_response` is now fallible (returns
// `Result<BidResponse, serde_json::Error>`) so the inner
// `serde_json::to_value` calls on the typed BTreeMaps can `?`-
// propagate instead of relying on `unwrap_or_default()`. The
// `serde_json::Error` discriminator means the handler returns 500
// only if the response builder itself fails — same surface as
// `serialize_response` post-round-18 review Medium 10.
pub fn build_response(input: ResponseInput) -> Result<BidResponse, serde_json::Error> {
    // 1. Build `seatbid` — sorted by seat. A seat is emitted only if its
    //    bidder produced ≥1 bid.
    let mut seatbids: Vec<SeatBid> = Vec::new();
    let mut any_bid = false;
    for (bidder, bundle) in &input.bundles {
        if bundle.bids.is_empty() { continue; }
        any_bid = true;
        let bids: Vec<Bid> = bundle.bids.iter().map(|t| t.bid.clone()).collect();
        // Round-13 review Blocker 1: SeatBid.seat is String (not
        // Option) per foundations, and the struct has no `ext` field —
        // the typed extension surface for the seat-level row sits
        // in SeatBid.other (the serde_json::Map flatten). Build
        // verbatim against the foundations shape.
        seatbids.push(SeatBid {
            bid: bids,
            seat: bidder.as_str().to_owned(),
            group: 0,
            other: serde_json::Map::new(),
        });
    }
    seatbids.sort_by(|left, right| left.seat.cmp(&right.seat));

    // 2. `cur` rule: "USD" iff seatbid non-empty, else "".
    let cur = if any_bid { "USD".to_owned() } else { String::new() };

    // 3. `nbr` rule (Appendix E.5): when there are no live bidders (every
    //    bundle.dispatched == false), set NoBidInvalidRequest. We can't
    //    distinguish "all-failed-after-dispatch" from "no-live" without
    //    that flag.
    // Round-13 review Blocker 1: `not_self_invert()` was a
    // workaround for a borrow-checker confusion that doesn't apply
    // here — the original draft collapsed to one condition. Per
    // Appendix E.5 + upstream reference auction processing, nbr = NoBidInvalidRequest
    // (7) iff every bundle is non-dispatched AND at least one bidder
    // was registered for this auction (`!bundles.is_empty()`).
    let no_live = input.bundles.values().all(|b| !b.dispatched);
    let nbr = if no_live && !input.bundles.is_empty() {
        Some(7)
    } else {
        None
    };

    // 4. ext.errors / ext.warnings (per-bidder + auction-level).
    let mut errors:   BTreeMap<String, Vec<ErrorEntry>> = BTreeMap::new();
    let mut warnings: BTreeMap<String, Vec<ErrorEntry>> = BTreeMap::new();
    for (bidder, bundle) in &input.bundles {
        if !bundle.errors.is_empty() {
            errors.insert(bidder.as_str().to_owned(), bundle.errors.clone());
        }
        if !bundle.warnings.is_empty() {
            warnings.insert(bidder.as_str().to_owned(), bundle.warnings.clone());
        }
    }
    if !input.prebid_errors.is_empty() {
        errors.insert("prebid".into(), input.prebid_errors.clone());
    }
    if !input.prebid_warnings.is_empty() {
        warnings.insert("prebid".into(), input.prebid_warnings.clone());
    }

    // 5. responsetimemillis (per-bidder only — upstream does not include a
    //    "upstream" key here).
    let mut rtm: BTreeMap<String, u32> = BTreeMap::new();
    for (bidder, bundle) in &input.bundles {
        if bundle.dispatched {
            rtm.insert(bidder.as_str().to_owned(), bundle.timing.total_ms());
        }
    }

    // 6. seatnonbid (only when return_all_bid_status). Sort seats lexicographically.
    let seatnonbid_blocks = if input.return_all_bid_status {
        let mut rows: Vec<serde_json::Value> = Vec::new();
        for (bidder, rs) in &input.seatnonbid {
            let nonbids: Vec<serde_json::Value> = rs.iter().map(|n| serde_json::json!({
                "impid": n.imp_id,
                "statuscode": n.statuscode,
            })).collect();
            rows.push(serde_json::json!({
                "seat": bidder.as_str(),
                "nonbid": nonbids,
            }));
        }
        rows.sort_by_key(|v| v.get("seat").and_then(|s| s.as_str()).map(str::to_owned));
        rows
    } else {
        Vec::new()
    };

    // 7. Assemble ext (deterministic via BTreeMap iteration).
    //
    // Round-23 review High 4: serde_json::to_value never fails for
    // these well-typed maps (BTreeMap<String, Vec<ErrorEntry>> and
    // BTreeMap<String, u32>), so the earlier `unwrap_or_default()`
    // silently masked a logic error AND tripped the clippy
    // `unwrap_used` gate. Use `?` to propagate a serde error as
    // BidderError::Internal — build_response is fallible upstream
    // so the surface is already a Result<_, serde_json::Error>.
    let mut ext_obj = serde_json::Map::new();
    if !errors.is_empty() {
        ext_obj.insert("errors".into(), serde_json::to_value(&errors)?);
    }
    if !warnings.is_empty() {
        ext_obj.insert("warnings".into(), serde_json::to_value(&warnings)?);
    }
    if !rtm.is_empty() {
        ext_obj.insert("responsetimemillis".into(), serde_json::to_value(&rtm)?);
    }
    ext_obj.insert("tmaxrequest".into(), serde_json::Value::from(input.bidder_visible_tmax_ms));
    let mut prebid_obj = serde_json::Map::new();
    prebid_obj.insert("auctiontimestamp".into(), serde_json::Value::from(input.auction_timestamp_ms));
    if !seatnonbid_blocks.is_empty() {
        prebid_obj.insert("seatnonbid".into(), serde_json::Value::Array(seatnonbid_blocks));
    }
    ext_obj.insert("prebid".into(), serde_json::Value::Object(prebid_obj));
    // auction core NEVER emits ext.debug — round-33 H2 + round-34 GDPR enforcement suppression.

    let ext_value = Some(serde_json::Value::Object(ext_obj));

    Ok(BidResponse {
        id: "".into(),  // caller fills in from req.id at the call site.
        seatbid: seatbids,
        cur,
        nbr,
        ext: ext_value,
        ..BidResponse::default()
    })
}

// Round-13 review Blocker 1: removed the BoolInvert / not_self_invert
// helper — it was a leftover from an earlier draft and never had a
// real purpose. The nbr computation above is direct boolean logic.

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_config::MediaType;

    fn entry(code: u32, msg: &str) -> ErrorEntry {
        ErrorEntry { code, message: msg.into() }
    }

    #[test]
    fn empty_seatbid_returns_cur_empty_string() {
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::new(),
            prebid_errors: vec![],
            prebid_warnings: vec![],
        };
        let resp = build_response(input).expect("build_response infallible on typed inputs");
        assert_eq!(resp.cur, "");
    }

    #[test]
    fn ext_errors_and_warnings_sorted_by_key() {
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        bundles.insert(BidderName::new("pubmatic"), BidderResultBundle {
            errors: vec![entry(4, "bad response")],
            dispatched: true,
            ..BidderResultBundle::default()
        });
        bundles.insert(BidderName::new("appnexus"), BidderResultBundle {
            warnings: vec![entry(6, "disabled")],
            dispatched: true,
            ..BidderResultBundle::default()
        });
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles,
            prebid_errors: vec![],
            prebid_warnings: vec![],
        };
        let resp = build_response(input).expect("build_response infallible on typed inputs");
        // BidResponse.ext is Option<serde_json::Value>. The map is
        // serialized via BTreeMap → lex order in the JSON output. We
        // confirm both keys appear under their respective sections.
        let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
        assert!(raw.contains(r#""appnexus""#));
        assert!(raw.contains(r#""pubmatic""#));
    }

    #[test]
    fn base_response_does_not_emit_debug() {
        let bundles = BTreeMap::new();
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles,
            prebid_errors: vec![],
            prebid_warnings: vec![],
        };
        let resp = build_response(input).expect("build_response infallible on typed inputs");
        // Round-15 review Blocker stale typed-API: BidResponse.ext is
        // Option<serde_json::Value> (typed), not Option<Box<RawValue>>.
        // Use serde_json::to_string for the substring assertion.
        let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
        assert!(!raw.contains("\"debug\""), "auction core must NEVER emit ext.debug (round-33 H2 + round-34 GDPR enforcement)");
    }
}
```

Wire `lib.rs`:

```rust
mod response;
mod responsetime;
pub use response::{
    BidderResultBundle, ErrorEntry, NonBidRow, ResponseInput, build_response,
};
pub use responsetime::ResponseTimeStat;
```

- [ ] **Step 3: Test**

```
cargo test -p auctioneer-exchange response::tests responsetime::tests
```

Expected: 4 passed.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: deterministic response builder (Appendix E.5/E.6/E.7)"
```

---

### Task 32: `run_auction` — the engine seam

**Files:**

- Modify: `crates/auctioneer-exchange/src/run.rs`
- Modify: `crates/auctioneer-exchange/src/lib.rs`

**Interfaces:**

- Consumes: `&NormalizedBidRequest`, `&AuctionContext`.
- Produces: `pub async fn run_auction(req: &NormalizedBidRequest, ctx: &AuctionContext<'_>) -> Result<BidResponse, AuctionError>`. Executes §5.1 steps 6-11: privacy preprocess (stub) → alias resolution + bidderparams merge + per-imp split → privacy-per-bidder (stub allows all) → tmax resolution + `remaining == 0` short-circuit → `make_requests` per bidder → `validate_endpoint` per outbound → fan-out (with gzip rewrite when `endpoint_compression == Gzip`) → `make_bids` per bidder → collect → currency drop (non-USD) → targeting + winners → deterministic response build. Sets `BidResponse.id = req.id`; populates `responsetimemillis`. Emits `code 12 TmaxTimeout` on expired-budget short-circuit (with per-bidder code 1 entries for would-have-been-called bidders + `seatnonbid` 101 rows when `returnallbidstatus`).

- [ ] **Step 1: Implement (skeleton — full driver wires every step in this file)**

Replace `src/run.rs`:

```rust
//! `run_auction` — the engine seam. See spec §5.8.
//!
//! Performs §5.1 steps 6-11. Steps 1-5 (capture, body read, parse, host
//! + account resolution, `prepare`) and step 12 (serialize) are the
//! caller's responsibility.

use crate::{
    account::ResolvedAccount,
    alias::resolve_aliases,
    context::AuctionContext,
    error::AuctionError,
    fanout::{AuctionDeadline, BidderCall, BidderCallResult, Compression, FanoutError,
             FanoutInternalError, TmaxSource},
    limits::{apply_max_bidders, apply_max_outbound},
    response::{BidderResultBundle, ErrorEntry, NonBidRow, ResponseInput, build_response},
    responsetime::ResponseTimeStat,
    split::split_by_bidder,
    targeting::{BidRounding, PriceGranularity, build_keys, pick_winner, price_bucket,
                TargetingInput},
    tmax::resolve_tmax,
    validate_endpoint::validate_endpoint,
};
use auctioneer_bidder::{Bidder, BidderError, BidderHttpRequest, BidderHttpResponse, BidderName,
                    BidderRegistry, BidderRequestInfo, BidderResponse, EntryPoint, TypedBid};
// Round-17 review Medium 11: MediaType, ExtBid, and ExtBidPrebid were
// used by the inline targeting block without being imported. Pull
// them in here so the snippet compiles.
use auctioneer_config::MediaType;
use auctioneer_openrtb::ext::bid::{ExtBid, ExtBidPrebid};
use auctioneer_openrtb::types::bid_response::BidResponse;
use auctioneer_openrtb::NormalizedBidRequest;
use auctioneer_privacy::PrivacyContext;
use std::collections::BTreeMap;
use std::time::Duration;

pub async fn run_auction<'a>(
    req: &NormalizedBidRequest,
    ctx: &AuctionContext<'a>,
) -> Result<BidResponse, AuctionError> {
    // Round-20 review Blocker 7: bring `Deref` into scope so
    // `req.deref()` resolves. The earlier draft used the method
    // without the trait import, which is a compile error.
    use std::ops::Deref as _;
    let underlying = req.deref().clone();

    // Step 6: privacy preprocess (auction core stub).
    let mut privacy = PrivacyContext::allow_all_no_scrub();

    // Step 7: alias resolution + bidder-param merge + per-imp split.
    let aliases = resolve_aliases(&underlying, ctx.bidders)?;
    let split = split_by_bidder(&underlying, &aliases, ctx.bidders)?;

    // Round-20 review Blocker 2: capture the disabled-bidder
    // warnings BEFORE the per-bidder list is moved into
    // apply_max_bidders, so the lookup below has something to fold
    // into the bundles. The previous version dropped these on the
    // floor — code-6 BidderTemporarilyDisabled warnings never
    // reached ext.warnings.
    let disabled_bidder_warnings = split.disabled_bidder_warnings;

    // Apply `max_bidders_per_auction`.
    let cap = ctx.host.resource_limits.max_bidders_per_auction.max(1);
    let (per_bidder, bidder_cap_drops) = apply_max_bidders(split.per_bidder, cap);

    // Step 8: privacy-per-bidder (stub allows all).
    let live: Vec<_> = per_bidder.into_iter().filter(|p| {
        matches!(privacy.permissions_for(p.bidder.as_str()),
                 auctioneer_privacy::AuctionPermissions::AllowAll | auctioneer_privacy::AuctionPermissions::Partial { allow_bid_request: true, .. })
    }).collect();

    // tmax resolution + remaining-budget short-circuit.
    // Round-24 review Blocker 2: the foundational `BidRequest.tmax` is `Option<i64>`
    // (the OpenRTB wire type), but `resolve_tmax` takes `Option<u64>`
    // (an internal-only millisecond budget that cannot be negative).
    // The `prepare()` validator clamps negative tmax to error during
    // request normalization, so by the time we reach here a negative
    // value is structurally impossible — but we still must perform a
    // checked conversion because `i64 -> u64` is not an `Into`. Treat
    // any residual negative as "no client-supplied tmax" rather than
    // panicking, so a single missed validator path degrades to the
    // host default instead of crashing the auction.
    let req_tmax_ms = underlying.tmax.and_then(|t| u64::try_from(t).ok());
    let TmaxRes { bidder_visible_tmax_ms, deadline } = TmaxRes::from(
        resolve_tmax(req_tmax_ms, ctx.host, ctx.request_start));
    let now = ctx.clock.now_monotonic();
    let remaining = deadline.deadline_at.saturating_sub(now);

    let auction_timestamp_ms = u64::try_from(
        ctx.clock.now().unix_timestamp_nanos() / 1_000_000).unwrap_or(0);

    if remaining == Duration::ZERO {
        // Expired budget — short-circuit. Emit code 12 once + code 1 per
        // would-have-been-called bidder, no fan-out.
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        let mut seatnonbid: BTreeMap<BidderName, Vec<NonBidRow>> = BTreeMap::new();
        for p in &live {
            let mut b = BidderResultBundle::default();
            b.errors.push(ErrorEntry { code: 1, message: "timeout".into() });
            b.dispatched = false;
            bundles.insert(p.bidder.clone(), b);
            seatnonbid.insert(p.bidder.clone(), p.request.imp.iter().map(|imp|
                NonBidRow { imp_id: imp.id.clone(), statuscode: 101 }
            ).collect());
        }
        // Round-21 review High 7: surface disabled-bidder warnings
        // (spec line 558) on the expired-budget path too. The
        // earlier short-circuit dropped these on the floor; only the
        // normal post-fanout path folded them. A request that
        // arrived already-expired AND had a disabled bidder would
        // produce a 200 with no `code: 6` warning in
        // `ext.warnings[bidder]` — wrong per spec.
        for (bidder, warning) in &disabled_bidder_warnings {
            bundles.entry(bidder.clone()).or_default().warnings.push(warning.clone());
        }
        // Round-23 review High 4: build_response now returns Result;
        // propagate via AuctionError::Internal so the handler can
        // surface a real 500 if serde_json::to_value ever fails.
        let resp = build_response(ResponseInput {
            bidder_visible_tmax_ms,
            auction_timestamp_ms,
            return_all_bid_status: returnallbidstatus(&underlying),
            seatnonbid,
            bundles,
            prebid_errors: vec![ErrorEntry { code: 12, message: "tmax expired".into() }],
            prebid_warnings: vec![],
        }).map_err(|e| AuctionError::Internal(format!("build_response: {e}")))?;
        return Ok(with_id(resp, underlying.id));
    }

    // Step 9: fan out. Per-bidder make_requests + validate_endpoint.
    let mut bidder_pairs: Vec<(BidderName, Vec<BidderHttpRequest>)> = Vec::new();
    let mut bidder_pre_errors: BTreeMap<BidderName, Vec<ErrorEntry>> = BTreeMap::new();
    let mut timing: BTreeMap<BidderName, ResponseTimeStat> = BTreeMap::new();

    // Round-14 review Blocker GPC: hoisted out of the per-bidder loop
    // because GPC is auction-scoped (the request's normalized
    // regs.ext.gpc value, identical for every adapter). Both the
    // BidderRequestInfo construction (make_requests loop) and the
    // BidderCall construction (Sec-GPC outbound header loop) read this
    // single value.
    let gpc = underlying.regs.as_ref()
        .and_then(|r| r.ext.as_ref())
        .and_then(|e| e.gpc.as_deref());

    for p in &live {
        let t_mr = ctx.clock.now_monotonic();
        let (info, adapter) = match ctx.bidders.get(p.base.as_str()) {
            Some(t) => t,
            None => continue,
        };
        let info_alias = p.bidder.clone();
        let info_core  = p.base.clone();
        // Round-12 review High 7 + round-14 review Blocker GPC:
        // bidder context comes from the generated BidderInfo and the
        // prepared request body, not from stubs or headers. gpc is
        // sourced from underlying.regs.ext.gpc — `prepare()` folds the
        // Sec-GPC HTTP header into that typed slot during
        // normalization, so this is the single source of truth per
        // spec §5.1 step 4 / line 997.
        let macros: Vec<&str> = info.endpoint_macros.iter().map(String::as_str).collect();
        let allow: Option<Vec<&str>> = info.host_allowlist
            .as_ref()
            .map(|hosts| hosts.iter().map(String::as_str).collect());
        // Round-26 review High: PublisherID resolution per spec line
        // 1331-1333. Site-first, app-fallback. `as_deref()` strips
        // the trailing `Option<String>` wrapper layers down to
        // `Option<&str>`. The spec wording "the two are
        // intentionally distinct" forbids any AccountID fallback,
        // so we leave the slot `None` when neither inventory branch
        // carries a publisher id.
        let publisher_id = underlying.site.as_ref()
            .and_then(|s| s.publisher.as_ref())
            .and_then(|p| p.id.as_deref())
            .or_else(|| underlying.app.as_ref()
                .and_then(|a| a.publisher.as_ref())
                .and_then(|p| p.id.as_deref()));
        let info_for_adapter = BidderRequestInfo {
            bidder: info_alias.clone(),
            core_name: info_core,
            account_id: &ctx.account.id,
            publisher_id,
            endpoint_template: &info.endpoint,
            endpoint_macros: &macros,
            host_allowlist: allow.as_deref(),
            gpc,
            entry_point: EntryPoint::Auction,
            currency: &underlying.cur,
        };
        // Adapter-visible TMax: per §5.3 the bidder sees `bidder_visible_tmax_ms`.
        // Round-24 review Blocker 2: `BidRequest.tmax: Option<i64>`
        // (OpenRTB wire type) but `bidder_visible_tmax_ms: u32` (an
        // internal cap). `u32 -> i64` is widening / infallible.
        let mut req_for_bidder = p.request.clone();
        req_for_bidder.tmax = Some(i64::from(bidder_visible_tmax_ms));

        match adapter.make_requests(&req_for_bidder, &info_for_adapter) {
            Ok(requests) => {
                // §5.4 validate_endpoint on each outbound.
                let mut accepted = Vec::new();
                for r in requests {
                    match validate_endpoint(&r.url, info) {
                        Ok(()) => accepted.push(r),
                        Err(e) => {
                            bidder_pre_errors.entry(p.bidder.clone()).or_default().push(ErrorEntry {
                                code: e.code, message: e.message,
                            });
                        }
                    }
                }
                bidder_pairs.push((p.bidder.clone(), accepted));
            }
            Err(errs) => {
                for e in errs {
                    bidder_pre_errors.entry(p.bidder.clone()).or_default().push(ErrorEntry {
                        code: e.code, message: e.message,
                    });
                }
                bidder_pairs.push((p.bidder.clone(), Vec::new()));
            }
        }
        // Round-18 review High 9: Cloudflare's dev clock can move
        // backward per spec line 1702, so `Duration - Duration` may
        // panic (now < t_mr). Use `saturating_sub` everywhere we do
        // `now_monotonic() - earlier_capture` for diagnostic timing.
        timing.entry(p.bidder.clone()).or_default().make_requests =
            ctx.clock.now_monotonic().saturating_sub(t_mr);
    }

    // Apply `max_outbound_requests`.
    let out_cap = ctx.host.resource_limits.max_outbound_requests.max(1);
    let (bidder_pairs, outbound_drops) = apply_max_outbound(bidder_pairs, out_cap);

    // Build flat call list, preserving (bidder_index, call_index) correlation.
    let mut calls: Vec<BidderCall> = Vec::new();
    let mut correlation: Vec<(BidderName, usize)> = Vec::new();
    for (bidder_name, requests) in &bidder_pairs {
        let (info, _adapter) = match ctx.bidders.get(bidder_name.as_str())
            .or_else(|| aliases.bases.get(bidder_name).and_then(|b| ctx.bidders.get(b.as_str())))
        {
            Some(t) => t,
            None => continue,
        };
        let endpoint_compression = info.endpoint_compression.map(|c| match c {
            auctioneer_config::Compression::Gzip => Compression::Gzip,
        });
        // Round-12 review Medium 11: per-bidder timeout from
        // BidderInfo.bidder_timeout_ms (build-time YAML field added
        // in the round-12 codegen revision); falls back to the
        // auction-wide bidder-visible tmax when unset. Spec §5.3
        // line 1202 names bidder_timeout as per-bidder data.
        let timeout_ms = info.bidder_timeout_ms
            .map(u64::from)
            .unwrap_or(u64::from(bidder_visible_tmax_ms));
        let timeout = Duration::from_millis(timeout_ms);
        for r in requests {
            // Round-13 review High 3 + round-14 review Blocker GPC:
            // per spec §5.4 (line 1003) the EXCHANGE adds Sec-GPC to
            // outbound requests when the resolved request signals
            // GPC=1 — adapters MUST NOT have to set it themselves.
            // GPC is sourced from underlying.regs.ext.gpc (the typed
            // slot prepare() folds the Sec-GPC HTTP header into), so
            // we reuse the same gpc value the BidderRequestInfo was
            // built from. Mutate the per-call request's headers here,
            // before handing off to the fanout. The header carries
            // the literal value "1" when GPC is set; when unset, we
            // add nothing.
            let mut request = r.clone();
            if gpc == Some("1") {
                request.headers.push(("Sec-GPC".to_owned(), "1".to_owned()));
            }
            calls.push(BidderCall {
                request,
                timeout,
                max_response_bytes: ctx.host.resource_limits.max_bidder_response_bytes,
                endpoint_compression,
            });
            correlation.push((bidder_name.clone(), correlation.len()));
        }
    }

    let results = if calls.is_empty() {
        Vec::new()
    } else {
        ctx.fanout.send_all(calls, deadline).await.map_err(|e| match e {
            FanoutInternalError::NoOutboundCapability => AuctionError::FanoutUnavailable,
            FanoutInternalError::Internal(m) => AuctionError::Internal(m),
        })?
    };

    // Step 10: collect — per-bidder make_bids + correlate.
    let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
    let mut seatnonbid: BTreeMap<BidderName, Vec<NonBidRow>> = BTreeMap::new();

    // Pre-attach pre-errors and bidder drops as warnings.
    for (bidder, errs) in bidder_pre_errors {
        bundles.entry(bidder).or_default().errors.extend(errs);
    }
    // Round-20 review Blocker 2: surface the code-6 disabled-bidder
    // warnings from `split_by_bidder` into the per-bidder bundle so
    // `ext.warnings[bidder]` carries them per spec line 558.
    for (bidder, warning) in disabled_bidder_warnings {
        bundles.entry(bidder).or_default().warnings.push(warning);
    }
    for drop_entry in bidder_cap_drops {
        bundles.entry(drop_entry.bidder).or_default().warnings.push(ErrorEntry {
            code: 10999,
            message: format!("bidder dropped: max_bidders_per_auction ({}) exceeded", drop_entry.cap),
        });
    }
    for drop_entry in outbound_drops {
        bundles.entry(drop_entry.bidder).or_default().warnings.push(ErrorEntry {
            code: 10999,
            message: format!("{} outbound request(s) truncated by max_outbound_requests", drop_entry.dropped_count),
        });
    }

    // Walk results in their original order — they're positionally aligned
    // with `correlation` which is itself in registry order (bidder_pairs
    // is ordered by §5.9).
    // Round-21 review High 5: dropped the unused `per_bidder_responses`
    // map carried over from an earlier draft; the make_bids loop reads
    // each response directly off the `BidderCallResult` payloads.
    // Round-24 review High 3: the previous body indexed `correlation[idx]`
    // directly, which is a `clippy::indexing_slicing` (restriction)
    // violation AND swallows the "results / correlation length mismatch
    // is a logic bug" signal at compile time. Zip the two streams so the
    // iterator handles the pairing and a length divergence (which would
    // currently be a panic) becomes a logic-visible drop. Construction-
    // wise `results.len() == correlation.len()` always — every
    // `correlation` push above is matched by a `calls` push that the
    // fanout returns in order.
    let mut per_bidder_results: BTreeMap<BidderName, Vec<BidderCallResult>> = BTreeMap::new();
    for ((bidder, _), r) in correlation.iter().cloned().zip(results.into_iter()) {
        per_bidder_results.entry(bidder).or_default().push(r);
    }

    // make_bids per bidder.
    for (bidder_name, results_for_bidder) in per_bidder_results {
        let max_elapsed = results_for_bidder.iter().map(|r| r.elapsed).max()
            .unwrap_or(Duration::ZERO);
        let bundle = bundles.entry(bidder_name.clone()).or_default();
        bundle.dispatched = true;

        let stat = timing.entry(bidder_name.clone()).or_default();
        stat.max_call_elapsed = max_elapsed;
        let t_mb_start = ctx.clock.now_monotonic();

        // Find the per-bidder request list to pair with each response.
        // Round-24 review High 3: `unwrap_or_default()` here is a real
        // fallback path (a missing bidder_pairs entry IS a logic bug,
        // not "nothing to do") — annotate so the clippy restriction
        // gate accepts it AND a reader knows the empty default is
        // load-bearing rather than accidental. Spec: results that
        // arrive without a matching request_list MUST be dropped, not
        // attributed to a different bidder.
        #[expect(
            clippy::unwrap_or_default,
            reason = "missing bidder_pairs entry => drop responses for this bidder; \
                      the empty Vec funnels through the `request_list.get(i)` continue-arm",
        )]
        let request_list = bidder_pairs.iter().find(|(n, _)| n == &bidder_name)
            .map(|(_, rs)| rs.clone()).unwrap_or_default();

        for (i, call_result) in results_for_bidder.iter().enumerate() {
            let http_request = match request_list.get(i) {
                Some(r) => r.clone(),
                None => continue,
            };
            match &call_result.result {
                Ok(http_response) => {
                    let base_name = aliases.bases.get(&bidder_name).cloned()
                        .unwrap_or_else(|| bidder_name.clone());
                    let (_info, adapter) = match ctx.bidders.get(base_name.as_str()) {
                        Some(t) => t,
                        None => continue,
                    };
                    match adapter.make_bids(&underlying, &http_request, http_response) {
                        Ok(BidderResponse { bids, currency }) => {
                            // Currency-drop (MVP USD only).
                            if currency != "USD" && !currency.is_empty() {
                                bundle.warnings.push(ErrorEntry {
                                    code: 10004,
                                    message: format!("non-USD currency {currency} — bids dropped"),
                                });
                            } else {
                                bundle.bids.extend(bids);
                            }
                        }
                        Err(errs) => {
                            for e in errs {
                                bundle.errors.push(ErrorEntry { code: e.code, message: e.message });
                            }
                        }
                    }
                }
                Err(fe) => {
                    let (err_code, sn_code) = match fe {
                        FanoutError::Timeout            => (1u32, 101u32),
                        FanoutError::Unreachable        => (999, 103),
                        FanoutError::BadServerResponse  => (4, 100),
                        FanoutError::OversizeResponse   => (4, 100),
                        FanoutError::Other(_)           => (999, 100),
                    };
                    bundle.errors.push(ErrorEntry {
                        code: err_code,
                        message: format!("{:?}", fe),
                    });
                    let rows: Vec<NonBidRow> = http_request.covered_imp_ids.iter().map(|id|
                        NonBidRow { imp_id: id.clone(), statuscode: sn_code }
                    ).collect();
                    seatnonbid.entry(bidder_name.clone()).or_default().extend(rows);
                }
            }
        }
        stat.make_bids = ctx.clock.now_monotonic().saturating_sub(t_mb_start);
        bundle.timing = *stat;
    }

    // Step 11: targeting + winners (price-bucket per bid, per-imp winner).
    // Round-12 review High 8: targeting drives off the typed
    // request block (req.ext.prebid.targeting) with account-level
    // fallback per spec §5.6 + Appendix E.1–E.4. NO hardcoded MVP
    // values — every choice is request- or account-driven.
    let targeting_cfg = underlying.ext.as_ref()
        .and_then(|e| e.prebid.as_ref())
        .and_then(|p| p.targeting.as_ref());
    let granularity = targeting_cfg
        .and_then(|t| t.pricegranularity.as_ref())
        .map(parse_price_granularity)
        .unwrap_or_else(|| PriceGranularity::from_account(&ctx.account.config));
    let rounding = BidRounding::from_account(&ctx.account.config);
    let include_winners = targeting_cfg
        .and_then(|t| t.includewinners)
        .unwrap_or(true);
    let include_bidder_keys = targeting_cfg
        .and_then(|t| t.includebidderkeys)
        .unwrap_or(true);
    let include_format = targeting_cfg
        .and_then(|t| t.includeformat)
        .unwrap_or(false);
    let prefer_deals = targeting_cfg
        .and_then(|t| t.preferdeals)
        .unwrap_or(false);
    // Round-18 review Blocker 6 + round-19 review High 7:
    // `alwaysincludedeals` per spec §5.6 line 5238 controls
    // PER-BIDDER KEY EMISSION, NOT winner retention. Per the spec:
    // "Per-bidder keys are emitted when `includebidderkeys` is true,
    // OR when `alwaysincludedeals` is true AND the bid carries a
    // deal." The flag is passed into `build_keys`; the earlier
    // draft's "winner retention" interpretation was wrong AND the
    // value was computed but never threaded through.
    let always_include_deals = targeting_cfg
        .and_then(|t| t.alwaysincludedeals)
        .unwrap_or(false);
    // Round-21 review Spec 8: read `mediatypepricegranularity` from
    // `ExtRequestTargeting.other` (the foundational catch-all `serde_json::Map`)
    // per spec §5.6 line 5213. The earlier round-19 draft scoped this
    // out as an "auction core deviation" pending an GDPR enforcement-scope typed field, but
    // since `other` already preserves the JSON verbatim, auction core can honor
    // the override TODAY without a auctioneer-openrtb edit. Parsed values
    // become a `BTreeMap<&str, PriceGranularity>` keyed by the spec's
    // media-type strings (`banner` / `video` / `native` / `audio`).
    let media_type_granularity: BTreeMap<&str, PriceGranularity> = targeting_cfg
        .and_then(|t| t.other.get("mediatypepricegranularity"))
        .and_then(|v| v.as_object())
        .map(|m| {
            let mut out = BTreeMap::new();
            for key in ["banner", "video", "native", "audio"] {
                if let Some(value) = m.get(key) {
                    out.insert(key, parse_price_granularity(value));
                }
            }
            out
        })
        .unwrap_or_default();
    // Round-17 review Medium 11: prefix resolution per round-16 Task
    // 18c — request prefix → account.targeting_prefix → "hb". The
    // earlier inline snippet skipped the account fallback.
    let prefix = targeting_cfg
        .and_then(|t| t.prefix.as_deref())
        .or_else(|| {
            let p = ctx.account.config.targeting_prefix.as_str();
            if p.is_empty() { None } else { Some(p) }
        })
        .unwrap_or("hb");
    let truncate_target_attribute = ctx.account.config.truncate_target_attribute;

    // Compute price bucket per bid + per-imp winner.
    let bidder_bids_vec: Vec<(BidderName, Vec<TypedBid>)> = bundles.iter()
        .map(|(n, b)| (n.clone(), b.bids.clone())).collect();
    let imps: Vec<&str> = underlying.imp.iter().map(|i| i.id.as_str()).collect();
    for imp_id in &imps {
        let winner = pick_winner(&bidder_bids_vec, imp_id, prefer_deals);
        // For each bid on this imp, set hb_* keys.
        for (bidder_name, bundle) in &mut bundles {
            for b in &mut bundle.bids {
                if b.bid.impid != *imp_id { continue; }
                // Round-13 review High 7: TypedBid carries the
                // resolved media_type — pass it into targeting so
                // `hb_format` is meaningful when the request opts
                // into `includeformat`. Previously `format: None`
                // made the include_format flag a no-op.
                let format_str = match b.media_type {
                    MediaType::Banner => "banner",
                    MediaType::Video  => "video",
                    MediaType::Native => "native",
                    MediaType::Audio  => "audio",
                };
                let format = Some(format_str);
                // Round-21 review Spec 8: per-media-type granularity
                // override resolves before the top-level granularity
                // per spec §5.6 line 5213.
                let effective_granularity = media_type_granularity
                    .get(format_str)
                    .unwrap_or(&granularity);
                let bucket = price_bucket(b.bid.price, effective_granularity, rounding, ctx.rng);
                let is_winner = winner.as_ref().is_some_and(|w| &w.bidder == bidder_name);
                let mut keys = build_keys(&TargetingInput {
                    prefix,
                    truncate_target_attribute,
                    bidder: bidder_name.as_str(),
                    price_bucket: &bucket,
                    // Round-20 review Blocker 7: foundations `Bid` has
                    // `w: Option<i32>` / `h: Option<i32>`, and
                    // `TargetingInput.size_wh` is `(u32, u32)`. `into()`
                    // does not convert `i32 -> u32`. Use `try_from`
                    // and fall back to 0 on negative dimensions (which
                    // the build_keys helper then drops via the
                    // `w > 0 && h > 0` guard).
                    size_wh: Some((
                        u32::try_from(b.bid.w.unwrap_or(0)).unwrap_or(0),
                        u32::try_from(b.bid.h.unwrap_or(0)).unwrap_or(0),
                    )),
                    deal_id: b.bid.deal_id.as_deref(),
                    env: None,
                    include_winners,
                    include_bidder_keys,
                    include_format,
                    // Round-19 review High 7: alwaysincludedeals is a
                    // per-bidder-key inclusion override (spec §5.6
                    // line 5238). build_keys ORs it with
                    // include_bidder_keys when the bid carries a
                    // deal_id.
                    always_include_deals,
                    format,
                    is_overall_winner: is_winner,
                });
                // Round-18 review Blocker 5: surface TargetingKeys.warnings
                // into the per-bidder bundle's warnings vec. The
                // earlier draft computed the warnings but threw them
                // away; the TooLongTargetingPrefix (Appendix G 10015)
                // signal would never have reached `ext.warnings`.
                bundle.warnings.append(&mut keys.warnings);
                // Attach to bid.ext.prebid.targeting via the typed
                // ExtBid path (Bid.ext is Option<ExtBid> per foundations, not
                // RawValue). The targeting field on ExtBidPrebid is
                // BTreeMap<String, String> — no round-trip through raw
                // JSON.
                let mut combined: BTreeMap<String, String> = BTreeMap::new();
                for (k, v) in &keys.winner    { combined.insert((*k).to_owned(), v.clone()); }
                for (k, v) in &keys.per_bidder { combined.insert((*k).to_owned(), v.clone()); }
                let ext = b.bid.ext.get_or_insert_with(ExtBid::default);
                let prebid = ext.prebid.get_or_insert_with(ExtBidPrebid::default);
                // Round-22 review High 6: spec Appendix E line 5295
                // requires `bid.ext.prebid` to ALWAYS carry `type`,
                // `targeting`, and `meta` on every emitted bid. The
                // earlier draft only set `targeting`. Fill the other
                // two slots with the values auction core can produce locally:
                //   - `bid_type` = the resolved media type (already
                //     decided above for `hb_format`)
                //   - `meta` = an empty default `Map` (auction core has no
                //     advertiser/category source yet; GDPR enforcement wires real
                //     meta from the adapter response shape).
                prebid.bid_type = Some(format_str.to_owned());
                if prebid.meta.is_none() {
                    prebid.meta = Some(serde_json::Value::Object(serde_json::Map::new()));
                }
                prebid.targeting = combined;
            }
        }
    }

    // Round-19 review Medium 10: surface
    // NormalizedBidRequest.warnings() (e.g. legacy-25-privacy
    // signals; spec line 453) AND any privacy warnings drained from
    // PrivacyContext into `ext.warnings.prebid[]` per Appendix E.
    // auction core stubs return empty today, but the wiring lands NOW so GDPR enforcement's
    // GDPR / CCPA / GPP enforcement does not need a follow-up patch
    // to surface its warnings.
    let mut prebid_warnings: Vec<ErrorEntry> = Vec::new();
    for warning in req.warnings() {
        prebid_warnings.push(ErrorEntry {
            code: warning.code.as_u32(),
            message: warning.message.clone(),
        });
    }
    for warning in privacy.take_warnings() {
        // Round-20 review Blocker 7: `PrivacyContext::take_warnings`
        // returns `Vec<NonFatalWarning>` (same shape as
        // `NormalizedBidRequest::warnings()`), so the code field is
        // `WarningCode`, not `u32`. Call `as_u32()` for the
        // wire-shaped `ErrorEntry`.
        prebid_warnings.push(ErrorEntry {
            code: warning.code.as_u32(),
            message: warning.message,
        });
    }

    let resp = build_response(ResponseInput {
        bidder_visible_tmax_ms,
        auction_timestamp_ms,
        return_all_bid_status: returnallbidstatus(&underlying),
        seatnonbid,
        bundles,
        prebid_errors: vec![],
        prebid_warnings: std::mem::take(&mut prebid_warnings),
    }).map_err(|e| AuctionError::Internal(format!("build_response: {e}")))?;
    Ok(with_id(resp, underlying.id))
}

fn returnallbidstatus(req: &auctioneer_openrtb::types::bid_request::BidRequest) -> bool {
    // BidRequest.ext is Option<BidRequestExt> (typed) per foundations, not RawValue.
    req.ext.as_ref()
        .and_then(|e| e.prebid.as_ref())
        .and_then(|p| p.returnallbidstatus)
        .unwrap_or(false)
}

fn with_id(mut resp: BidResponse, id: String) -> BidResponse { resp.id = id; resp }

// Internal helper that mirrors `tmax::TmaxResolution` without dragging the
// struct across modules in this skeleton-style task.
struct TmaxRes { bidder_visible_tmax_ms: u32, deadline: AuctionDeadline }
impl From<crate::tmax::TmaxResolution> for TmaxRes {
    fn from(t: crate::tmax::TmaxResolution) -> Self {
        Self {
            bidder_visible_tmax_ms: t.bidder_visible_tmax_ms,
            deadline: t.deadline,
        }
    }
}
```

Wire `lib.rs`:

```rust
mod run;
pub use run::run_auction;
```

- [ ] **Step 2: Compile**

```
cargo check -p auctioneer-exchange
```

Expected: success (deferred to next task for integration tests).

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/src
git commit -m "auctioneer-exchange: run_auction skeleton (§5.1 steps 6-11)"
```

---

### Task 33: `MockBidderFanout` for tests + `run_auction` driver test

**Files:**

- Create: `crates/auctioneer-exchange/tests/run_auction.rs`

**Interfaces:**

- Consumes: `BidderFanout` trait.
- Produces: a deterministic mock returning canned `BidderCallResult`s; an integration test that drives `run_auction` end-to-end with `FakeClock`, `SeededRng`, mock fanout — asserts the returned `BidResponse` shape (200 OK envelope, `ext.responsetimemillis` populated, `seatbid` sorted by seat, `ext.errors` lex-keyed).

- [ ] **Step 1: Implement `tests/run_auction.rs`**

```rust
use async_trait::async_trait;
use auctioneer_bidder::{
    Bidder, BidderError, BidderHttpRequest, BidderHttpResponse, BidderRegistry,
    BidderRequestInfo, BidderResponse, GenericBidder, TypedBid,
};
use auctioneer_config::{AccountConfig, BidderInfo, Compression, HostConfig, MediaType, ResourceLimits};
use auctioneer_exchange::{
    AccountSource, AuctionContext, AuctionDeadline, AuctionError, BidderCall, BidderCallResult,
    BidderFanout, FanoutError, FanoutInternalError, ResolvedAccount, TmaxSource,
    run_auction, validate_endpoint,
};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
use auctioneer_openrtb::{prepare, RequestHeaders};
use auctioneer_runtime::{FakeClock, SeededRng};
use std::time::Duration;

// --- Mocks ---

struct CannedFanout {
    responses: Vec<BidderHttpResponse>,
}

#[async_trait(?Send)]
impl BidderFanout for CannedFanout {
    async fn send_all(&self, calls: Vec<BidderCall>, _deadline: AuctionDeadline)
        -> Result<Vec<BidderCallResult>, FanoutInternalError>
    {
        Ok(calls.iter().enumerate().map(|(i, _c)| BidderCallResult {
            result: Ok(self.responses.get(i).cloned().unwrap_or(BidderHttpResponse {
                status: 204, headers: vec![], body: vec![],
            })),
            elapsed: Duration::from_millis(50),
        }).collect())
    }
}

fn host_with_caps() -> HostConfig {
    HostConfig {
        resource_limits: ResourceLimits {
            default_tmax_ms: 750,
            max_tmax_ms: 1500,
            max_bidder_response_bytes: 262_144,
            max_bidders_per_auction: 50,
            max_outbound_requests: 100,
        },
        ..HostConfig::default()
    }
}

fn leak_info(name: &'static str, endpoint: &'static str) -> &'static BidderInfo {
    Box::leak(Box::new(BidderInfo {
        name: name.into(),
        endpoint: endpoint.into(),
        uses_https: true,
        ..BidderInfo::default()
    }))
}

// Round-14 review Blocker Imp.ext drift + Blocker tokio: helper
// builds a typed Imp.ext with a one-bidder legacy block; helper
// drives async tests via futures::executor::block_on (the auction core
// dependency-purity gate forbids tokio in library crates).
fn imp_with_bidder(id: &str, bidder: &str) -> Imp {
    let mut obj = serde_json::Map::new();
    obj.insert(bidder.to_owned(), serde_json::json!({}));
    Imp {
        id: id.into(),
        ext: Some(ImpExt {
            bidder: Some(serde_json::Value::Object(obj)),
            ..ImpExt::default()
        }),
        ..Imp::default()
    }
}

#[test]
fn run_auction_returns_bid_response_with_responsetime_and_currency() {
    futures::executor::block_on(async {
        // Wire registry with the generic adapter at a fixed endpoint.
        let mut registry = BidderRegistry::new();
        registry.register(leak_info("generic", "https://demo.example.test/openrtb"), Box::new(GenericBidder));

        // Build a BidRequest carrying one imp + a single bidder.
        let req = BidRequest {
            id: "auc-1".into(),
            imp: vec![imp_with_bidder("imp-1", "generic")],
            cur: vec!["USD".into()],
            ..BidRequest::default()
        };
        let headers = RequestHeaders::default();
        let normalized = prepare(req, &headers).expect("prepare succeeds on minimal request");

        let host = host_with_caps();
        let acct = AccountConfig::default();
        let resolved = ResolvedAccount { id: "unknown".into(), source: AccountSource::DefaultFallback, config: &acct };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        let response_body = br#"{"id":"r","seatbid":[{"seat":"generic","bid":[{"id":"b1","impid":"imp-1","price":1.5,"w":300,"h":250}]}],"cur":"USD"}"#;
        let fanout = CannedFanout {
            responses: vec![BidderHttpResponse {
                status: 200, headers: vec![], body: response_body.to_vec(),
            }],
        };

        let ctx = AuctionContext::with_request_start(
            clock.now_monotonic(),
            &host, &resolved, &registry, &fanout, &clock, &rng,
            false,
        );

        let resp = run_auction(&normalized, &ctx).await.expect("run_auction succeeds");
        assert_eq!(resp.id, "auc-1");
        assert_eq!(resp.cur, "USD");
        let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
        assert!(raw.contains("\"responsetimemillis\""));
        assert!(raw.contains("\"tmaxrequest\""));
        assert!(!raw.contains("\"debug\""));  // auction core suppression invariant
        let seatbid = &resp.seatbid;
        assert_eq!(seatbid.len(), 1);
        assert_eq!(seatbid[0].seat, "generic");
    });
}

#[test]
fn run_auction_short_circuits_on_expired_budget() {
    futures::executor::block_on(async {
        let mut registry = BidderRegistry::new();
        registry.register(leak_info("generic", "https://demo.example.test/openrtb"), Box::new(GenericBidder));
        let req = BidRequest {
            id: "auc-2".into(),
            imp: vec![imp_with_bidder("imp-1", "generic")],
            cur: vec!["USD".into()],
            tmax: Some(0),  // → default 750ms
            ..BidRequest::default()
        };
        let normalized = prepare(req, &RequestHeaders::default()).unwrap();
        let host = HostConfig {
            resource_limits: ResourceLimits {
                default_tmax_ms: 1,  // 1ms — guaranteed expired by the time fanout starts
                max_tmax_ms: 1,
                max_bidder_response_bytes: 262_144,
                max_bidders_per_auction: 50,
                max_outbound_requests: 100,
            },
            ..HostConfig::default()
        };
        let acct = AccountConfig::default();
        let resolved = ResolvedAccount { id: "unknown".into(), source: AccountSource::DefaultFallback, config: &acct };
        let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
        let rng = SeededRng::new(42);
        // Capture request_start, then advance past the deadline.
        let start = clock.now_monotonic();
        clock.advance(Duration::from_millis(5));
        let fanout = CannedFanout { responses: vec![] };
        let ctx = AuctionContext::with_request_start(
            start, &host, &resolved, &registry, &fanout, &clock, &rng, false,
        );
        let resp = run_auction(&normalized, &ctx).await.unwrap();
        let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
        // code 12 emitted under ext.errors.prebid
        assert!(raw.contains("\"prebid\""));
        assert!(raw.contains("\"code\":12") || raw.contains("\"code\": 12"));
    });
}
```

- [ ] **Step 2: Dependency-purity gate** — `tokio` is NOT added as a dev-dep on `auctioneer-exchange`. Async tests use `futures::executor::block_on` (the `futures` crate is already a regular workspace dep per the Architecture block), and the purity walk in Task 41 stays strict (it fails on any transitive `tokio` reference, dev-deps included). This is round-12 High 9 + round-14 Blocker tokio: the previous "add tokio as dev-dep" instruction is removed.

Round-14 review Blocker tokio: NO `tokio` dev-dep is added to `auctioneer-exchange`. The library crates stay pure per the dependency-purity gate (Task 41). `futures` provides `executor::block_on` for the test-side async runner. The earlier instruction to add tokio is rescinded.

- [ ] **Step 3: Test**

```
cargo test -p auctioneer-exchange --test run_auction
```

Expected: 2 passed.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-exchange
git commit -m "auctioneer-exchange: run_auction driver test (mock fanout)"
```

---

### Task 34: Deterministic snapshot-output integration test

**Files:**

- Create: `crates/auctioneer-exchange/tests/determinism.rs`
- Create: `crates/auctioneer-exchange/tests/snapshot/two_bidders_single_imp.json`

**Interfaces:**

- Consumes: `run_auction`.
- Produces: an end-to-end test that runs the same request twice (different `BidRequest` instances) with identical mocks/`FakeClock`/`SeededRng` seeds and asserts byte-identical `BidResponse` JSON serialization (Appendix E.7).

- [ ] **Step 1: Implement `tests/determinism.rs`**

```rust
use async_trait::async_trait;
use auctioneer_bidder::{BidderRegistry, GenericBidder};
use auctioneer_config::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
use auctioneer_exchange::{
    AccountSource, AuctionContext, AuctionDeadline, BidderCall, BidderCallResult,
    BidderFanout, FanoutInternalError, ResolvedAccount,
    run_auction,
};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
use auctioneer_openrtb::{prepare, RequestHeaders};
use auctioneer_openrtb::types::bid_response::{BidResponse, SeatBid, Bid};
use auctioneer_runtime::{FakeClock, SeededRng};
use std::time::Duration;

struct CannedFanout { responses: Vec<auctioneer_bidder::BidderHttpResponse> }
#[async_trait(?Send)]
impl BidderFanout for CannedFanout {
    async fn send_all(&self, calls: Vec<BidderCall>, _d: AuctionDeadline)
        -> Result<Vec<BidderCallResult>, FanoutInternalError>
    {
        Ok(calls.iter().enumerate().map(|(i, _)| BidderCallResult {
            result: Ok(self.responses[i % self.responses.len()].clone()),
            elapsed: Duration::from_millis(50),
        }).collect())
    }
}

fn leak(name: &'static str, ep: &'static str) -> &'static BidderInfo {
    Box::leak(Box::new(BidderInfo {
        name: name.into(), endpoint: ep.into(), uses_https: true,
        ..BidderInfo::default()
    }))
}

fn host() -> HostConfig {
    HostConfig {
        resource_limits: ResourceLimits {
            default_tmax_ms: 750, max_tmax_ms: 1500,
            max_bidder_response_bytes: 262_144,
            max_bidders_per_auction: 50, max_outbound_requests: 100,
        },
        ..HostConfig::default()
    }
}

async fn drive() -> BidResponse {
    let mut registry = BidderRegistry::new();
    registry.register(leak("appnexus", "https://ib.adnxs.example.test/openrtb2"), Box::new(GenericBidder));
    registry.register(leak("pubmatic", "https://hb.pubmatic.example.test/openrtb"), Box::new(GenericBidder));

    // Round-14 review Blocker Imp.ext drift: typed ImpExt with the
    // legacy bidder block carrying both bidders.
    let mut bidders = serde_json::Map::new();
    bidders.insert("appnexus".into(), serde_json::json!({}));
    bidders.insert("pubmatic".into(), serde_json::json!({}));
    let req = BidRequest {
        id: "auc-det-1".into(),
        imp: vec![Imp {
            id: "imp-1".into(),
            ext: Some(auctioneer_openrtb::ext::imp::ImpExt {
                bidder: Some(serde_json::Value::Object(bidders)),
                ..auctioneer_openrtb::ext::imp::ImpExt::default()
            }),
            ..Imp::default()
        }],
        cur: vec!["USD".into()],
        ..BidRequest::default()
    };
    let normalized = prepare(req, &RequestHeaders::default()).unwrap();
    let host = host();
    let acct = AccountConfig::default();
    let resolved = ResolvedAccount {
        id: "unknown".into(),
        source: AccountSource::DefaultFallback,
        config: &acct,
    };
    let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
    let rng = SeededRng::new(42);
    let body = br#"{"id":"r","seatbid":[{"seat":"x","bid":[{"id":"b","impid":"imp-1","price":2.5,"w":300,"h":250}]}],"cur":"USD"}"#;
    let fanout = CannedFanout {
        responses: vec![auctioneer_bidder::BidderHttpResponse {
            status: 200, headers: vec![], body: body.to_vec(),
        }],
    };
    let ctx = AuctionContext::with_request_start(
        clock.now_monotonic(), &host, &resolved, &registry, &fanout, &clock, &rng, false,
    );
    run_auction(&normalized, &ctx).await.unwrap()
}

#[test]
fn same_inputs_produce_byte_identical_responses() {
    let (a, b) = futures::executor::block_on(async { (drive().await, drive().await) });
    let bytes_a = serde_json::to_vec(&a).unwrap();
    let bytes_b = serde_json::to_vec(&b).unwrap();
    assert_eq!(bytes_a, bytes_b, "Appendix E.7 byte-stable response requirement");
}

#[test]
fn seatbid_sorted_by_seat_lexicographic() {
    let r = futures::executor::block_on(drive());
    // BidResponse.seatbid is Vec<SeatBid>; SeatBid.seat is String.
    let seats: Vec<&str> = r.seatbid.iter().map(|s| s.seat.as_str()).collect();
    let mut sorted = seats.clone();
    sorted.sort();
    assert_eq!(seats, sorted, "seatbid must be sorted by seat (Appendix E.7)");
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-exchange --test determinism
```

Expected: 2 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-exchange/tests/determinism.rs
git commit -m "auctioneer-exchange: deterministic-serialization E2E (Appendix E.7)"
```

---

### Task 35: Wire `edgezero.toml` route table for the four auction core routes + stub handlers

**Files:**

- Modify: `edgezero.toml`
- Create: `crates/auctioneer-core/src/handlers/mod.rs`
- Modify: `crates/auctioneer-core/src/lib.rs`

**Interfaces:**

- Consumes: `edgezero_core::action`, `edgezero_core::context::RequestContext`, `edgezero_core::error::EdgeError`, `edgezero_core::http::Response`. (Round-27 review Blocker 1: only `action` and `app` are re-exported at the EdgeZero crate root; `RequestContext` and `EdgeError` live under `context::` / `error::`.)
- Produces: four `[[triggers.http]]` entries for `POST /openrtb2/auction`, `GET /status`, `GET /info/bidders`, `GET /info/bidders/{name}` AND four matching `#[edgezero_core::action]` stub handlers in `crates/auctioneer-core/src/handlers/mod.rs`. The stubs return `EdgeError::internal("...not yet wired...")`; Tasks 38 and 39 REPLACE the stub bodies with real impls in the same module.

Round-15 review Blocker EdgeZero API: the pinned EdgeZero manifest schema (see `edgezero-core/src/manifest.rs` line 276) declares each `[[triggers.http]]` entry with `handler = "<fully-qualified-fn-path>"`, `methods = [<list-of-methods>]`, and `path = "<route>"`. There is NO `action` field and NO per-route `body_cap` slot — the macro is `edgezero_core::app!("./edgezero.toml")` taking a manifest path string, and registers handlers by their qualified path. Earlier drafts used `action = "..."`, `method = "..."` (singular), and `body_cap = N`; the macro would silently ignore those fields and the routes would never register.

Round-18 review Blocker 1: the `app!` macro EXPANDS INSIDE auctioneer-core, so Rust resolves `handler = "..."` paths from the expansion site. `auctioneer_core::handlers::auction` only works from outside the crate; from the inside, the crate cannot refer to itself by name without `extern crate self as auctioneer_core;`. Use `crate::handlers::auction` (the form the pinned `edgezero_core::app!` example comment uses); it resolves at every expansion site without an extra alias declaration.

Round-26 review Blocker 1: the on-disk `crates/auctioneer-core/src/lib.rs` ALREADY invokes `edgezero_core::app!("../../edgezero.toml");` (foundations ships it as a single-line invocation against an empty triggers list). Adding manifest triggers WITHOUT same-commit stub handlers leaves the workspace red at Task 35's commit boundary — `cargo check -p auctioneer-core` fails on unresolved `crate::handlers::*` paths even though the Python TOML parse passes. Earlier drafts "fixed" this by skipping cargo check at Task 35, but that hid the breakage rather than closing it. The fix this round: land the four stub handlers (with `EdgeError::internal` bodies) AND the `pub mod handlers;` line AND the manifest triggers in this SINGLE commit, so the app! macro resolves to live functions throughout Tasks 36/37/37b/38 and only the BODIES of the stubs change in 38/39 — never the signatures or paths.

- [ ] **Step 1: Create `crates/auctioneer-core/src/handlers/mod.rs` with stub bodies**

```rust
//! Action handlers for the four auction core routes. Tasks 38 and 39 REPLACE the
//! stub bodies below with real implementations; the signatures and
//! function names MUST stay stable so `edgezero.toml` keeps resolving.
//!
//! Round-26 review Blocker 1: these stubs exist so the manifest
//! triggers added in Task 35 resolve via the `edgezero_core::app!`
//! macro for every commit between Task 35 and Task 39. Without them
//! the workspace would be red for ~5 task boundaries.

// Round-27 review Blocker 1: only `action` and `app` are re-exported
// at the crate root (`pub use edgezero_macros::{action, app};` —
// edgezero-core/src/lib.rs L32). `EdgeError` lives under the `error`
// module, `RequestContext` under `context`. Importing them through
// the root would fail at the first `cargo check -p auctioneer-core` Task
// 35 runs.
use edgezero_core::{action, context::RequestContext, error::EdgeError};
use edgezero_core::http::Response;

#[action]
pub async fn auction(_ctx: RequestContext) -> Result<Response, EdgeError> {
    Err(EdgeError::internal(
        "auction handler not yet wired — Task 38 replaces this body",
    ))
}

#[action]
pub async fn status(_ctx: RequestContext) -> Result<Response, EdgeError> {
    Err(EdgeError::internal(
        "status handler not yet wired — Task 39 replaces this body",
    ))
}

#[action]
pub async fn info_bidders(_ctx: RequestContext) -> Result<Response, EdgeError> {
    Err(EdgeError::internal(
        "info_bidders handler not yet wired — Task 39 replaces this body",
    ))
}

#[action]
pub async fn info_bidder(_ctx: RequestContext) -> Result<Response, EdgeError> {
    Err(EdgeError::internal(
        "info_bidder handler not yet wired — Task 39 replaces this body",
    ))
}
```

- [ ] **Step 1b: Add `pub mod handlers;` to `crates/auctioneer-core/src/lib.rs`**

Append (do NOT remove the existing `edgezero_core::app!("../../edgezero.toml");` line):

```rust
pub mod handlers;
```

The on-disk lib.rs goes from a single-line `edgezero_core::app!(...)` to two lines (macro invocation + handler module). Task 36 below ADDS `pub mod host_primitives;` alongside these; Tasks 37 / 37b / 38 each append their own `pub mod` line for the module they land. No replacement of the file's content, just appends — so every commit between Task 35 and Task 39 keeps the same compiling shape.

- [ ] **Step 2: Edit `edgezero.toml`** — add the four trigger entries

After the commented-out block, insert:

```toml
[[triggers.http]]
handler = "crate::handlers::auction"
methods = ["POST"]
path    = "/openrtb2/auction"

[[triggers.http]]
handler = "crate::handlers::status"
methods = ["GET"]
path    = "/status"

[[triggers.http]]
handler = "crate::handlers::info_bidders"
methods = ["GET"]
path    = "/info/bidders"

[[triggers.http]]
handler = "crate::handlers::info_bidder"
methods = ["GET"]
path    = "/info/bidders/{name}"
```

Body-cap enforcement is the ingress hook's job (foundations `body_caps::match_route` + Task 36's body-cap delivery surface), not a manifest field. The hook reads its cap table from the wired route registrations and applies the 256 KiB / 4 KiB caps from §5.3 before the action runs.

Task 38 replaces the `auction` stub body (this module file) with the real impl; Task 39 replaces the `status` / `info_bidders` / `info_bidder` stubs. Both tasks preserve the function signatures and the `crate::handlers::*` resolution path verbatim, so the manifest never needs another edit.

- [ ] **Step 3: Verify the manifest parses AND auctioneer-core compiles**

First, run the offline TOML parse + schema-shape lint (this guards against drifting back to the pre-pinning manifest schema):

```sh
python3 - <<'PY'
import sys, tomllib
with open('edgezero.toml', 'rb') as fh:
    data = tomllib.load(fh)
triggers = data.get('triggers', {}).get('http', [])
assert len(triggers) == 4, f"expected 4 triggers, got {len(triggers)}"
# Round-23 review Medium-high 5: keys must match EXACTLY (`==`, not
# `>=`). Earlier draft permitted extra/stale keys like `action`,
# `method`, `body_cap` to ride along — those would silently no-op
# under the pinned EdgeZero schema and the routes wouldn't bind.
required = {"handler", "methods", "path"}
optional = {"adapters", "body-mode", "description", "id"}  # per ManifestHttpTrigger
allowed = required | optional
for entry in triggers:
    assert set(entry.keys()) == required \
        or set(entry.keys()).issubset(allowed) and required.issubset(set(entry.keys())), \
        f"unexpected keys in trigger: {set(entry.keys()) - allowed}"
    assert isinstance(entry["methods"], list)
    assert isinstance(entry["handler"], str) and entry["handler"].startswith("crate::handlers::"),\
        f"handler must be a crate::handlers::* path, got {entry['handler']}"
print("edgezero.toml: 4 triggers, schema OK (exact-keyed)")
PY
```

Expected: `edgezero.toml: 4 triggers, schema OK (exact-keyed)`.

Then — **round-26 review Blocker 1** — because the stub handlers from Step 1 + 1b exist AND lib.rs invokes `edgezero_core::app!(...)`, every `crate::handlers::*` path in the manifest now resolves to a live function. Confirm the workspace compiles:

```
cargo check -p auctioneer-core
cargo check --workspace
```

Expected: both green. The same gate runs again at the end of Task 39 to catch any drift introduced by the stub→real swap. If `cargo check` fails here on "unresolved name" pointing at `crate::handlers::...`, that means either Step 1 missed a handler, Step 1b skipped the `pub mod handlers;` append, or the manifest path doesn't match a function name — fix the underlying issue before committing.

- [ ] **Step 4: Commit**

Round-26 review Blocker 1: this commit lands manifest triggers, the `pub mod handlers;` line, and four stub action functions atomically — `cargo check -p auctioneer-core` AND `cargo check --workspace` MUST be green. Tasks 36 / 37 / 37b / 38 each append (do not replace) their own `pub mod` declarations to lib.rs and add modules under `crates/auctioneer-core/src/`, so the green compile shape persists across every subsequent commit.

```
git add edgezero.toml crates/auctioneer-core/src/handlers/mod.rs crates/auctioneer-core/src/lib.rs
git commit -m "auctioneer-core: register auction core routes + stub handlers"
```

---

### Task 36: `auctioneer-core` host primitives — `SystemClock`, dev `Rng`

**Files:**

- Create: `crates/auctioneer-core/src/host_primitives/mod.rs`
- Create: `crates/auctioneer-core/src/host_primitives/clock.rs`
- Create: `crates/auctioneer-core/src/host_primitives/rng.rs`
- Modify: `crates/auctioneer-core/Cargo.toml` — round-22 review Blocker 1. The current `auctioneer-core` manifest only declares `edgezero-core`; `SystemClock` uses `time::OffsetDateTime`, `std::time::{Instant, SystemTime}`, and the `auctioneer_runtime::Clock` trait. Add the two missing deps BEFORE the Step 1 / 2 implementations land or `cargo check` fails:

```toml
[dependencies]
edgezero-core   = { workspace = true }
auctioneer-runtime  = { workspace = true }   # round-22 Blocker 1
time            = { workspace = true }   # round-22 Blocker 1
```

(Subsequent tasks add `async-trait`, `flate2`, `futures`, `auctioneer-{bidder,config,exchange,openrtb,privacy}`, `serde`, `serde_json`, `http` as they land their own code.)

**Interfaces:**

- Consumes: `auctioneer-runtime::{Clock, Rng}`.
- Produces: `pub struct SystemClock` (`now` via `SystemTime::now`, `now_monotonic` via `Instant::now`) for the Axum/Spin/Fastly dev targets. (Cloudflare-shim variant noted but ships under the cloudflare adapter, not here, per §14.) `pub struct DevRng` (xorshift64* seeded from `SystemTime::now().duration_since(UNIX_EPOCH).as_nanos()`).

- [ ] **Step 1: Implement `host_primitives/clock.rs`**

```rust
//! Dev-only `SystemClock`. Spec §14 auction core prerequisite #2.

use auctioneer_runtime::Clock;
// Round-21 review High 5: dropped the unused `Mutex` import that
// older drafts carried over from an earlier interior-mutability
// sketch.
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use time::OffsetDateTime;

pub struct SystemClock {
    epoch: OnceLock<Instant>,
}

impl SystemClock {
    #[must_use] pub const fn new() -> Self { Self { epoch: OnceLock::new() } }
}

impl Default for SystemClock { fn default() -> Self { Self::new() } }

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        let dur = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        // Round-21 review High 5: `as i128` is a clippy-restriction
        // `as_conversions` violation. Use `i128::try_from(u128)`;
        // 128 bits cannot overflow into i128 within the SystemTime
        // domain, so `unwrap_or(0)` is the safe fallback.
        let nanos = i128::try_from(dur.as_nanos()).unwrap_or(0);
        OffsetDateTime::from_unix_timestamp_nanos(nanos)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }
    fn now_monotonic(&self) -> Duration {
        let epoch = *self.epoch.get_or_init(Instant::now);
        Instant::now().saturating_duration_since(epoch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monotonic_does_not_go_backwards() {
        let c = SystemClock::new();
        let a = c.now_monotonic();
        let b = c.now_monotonic();
        assert!(b >= a);
    }
}
```

- [ ] **Step 2: Implement `host_primitives/rng.rs`**

```rust
//! Dev-only `Rng` seeded from system time. Spec §14 auction core prerequisite #4.

use auctioneer_runtime::Rng;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DevRng { state: Mutex<u64> }

impl DevRng {
    #[must_use] pub fn new() -> Self {
        // Round-21 review High 5: `as u64` from `u128` is a clippy
        // `as_conversions` violation. Use `u64::try_from` and mask
        // to the low 64 bits via `wrapping_as`-style fallback.
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_nanos() & u128::from(u64::MAX)).unwrap_or(0x1234_5678))
            .unwrap_or(0x1234_5678);
        Self { state: Mutex::new(nanos | 1) }   // ensure non-zero
    }
}

impl Default for DevRng { fn default() -> Self { Self::new() } }

impl Rng for DevRng {
    fn next_u64(&self) -> u64 {
        // Round-21 review High 5: `expect()` violates the clippy
        // restriction gate (`expect_used`). Use `unwrap_or_else`
        // with `PoisonError::into_inner` so a poisoned mutex still
        // yields a usable state rather than panicking — matches
        // the foundational `seeded_rng.rs` pattern.
        let mut s = self.state.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut x = *s;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *s = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dev_rng_produces_distinct_values() {
        let r = DevRng::new();
        assert_ne!(r.next_u64(), r.next_u64());
    }
}
```

- [ ] **Step 3: `host_primitives/mod.rs`** — round-21 review Blocker 2: Task 36 ONLY declares the modules it actually creates here (`clock`, `rng`). The `fanout` and `registry` modules MUST NOT appear until Task 37 / 37b creates their source files, or `cargo check` errors with `file not found for module`. Each later task adds its own `pub mod` line when it lands its file.

```rust
pub mod clock;
pub mod rng;
pub use clock::SystemClock;
pub use rng::DevRng;
```

Task 37 amends this file to add `pub mod fanout;` + the `DevFanout` / `ProxyClientApi` re-exports. Task 37b amends it again to add `pub mod registry;`. Both amendments are explicit steps in their own tasks; nothing is "stubbed ahead" here.

Wire into `src/lib.rs` — round-26 review Blocker 1: APPEND, do not replace. After Task 35 the file already declares `pub mod handlers;` plus the `edgezero_core::app!(...)` macro invocation; this task adds the `host_primitives` module alongside, NEVER removing the macro line. (Earlier draft replaced the entire file content with a single `pub mod host_primitives;` line, which deleted the macro AND the handlers module — the assumption was that the macro would fail to resolve before Task 39, but the round-26 stub-handlers fix makes the macro resolve cleanly throughout.)

```rust
// crates/auctioneer-core/src/lib.rs — cumulative shape after Task 36:

edgezero_core::app!("../../edgezero.toml");

pub mod handlers;        // landed by Task 35 (stub bodies)
pub mod host_primitives; // landed by THIS task

// Tasks 37 / 37b each append one more `pub mod` line below
// (fanout, registry). Task 38 / 39 only replace handler BODIES;
// they do not touch this lib.rs file beyond their own pub-mod
// appends if a new module lands alongside their handler change.
```

(Round-21 review Blocker 2: `edgezero_proxy_adapter`, `fanout_seam`, and `registry` are added by their respective tasks — Task 37 / 37b each amend `src/lib.rs` to add the `pub mod` declaration for the module it lands. Stubbing them ahead of time would introduce empty placeholder files that the implementer has to remember to flesh out.)

- [ ] **Step 4: Test**

```
cargo test -p auctioneer-core --lib host_primitives::clock
cargo test -p auctioneer-core --lib host_primitives::rng
cargo check -p auctioneer-core
cargo check --workspace
```

Expected: all four commands succeed. With the Task-35 stub handlers in place + the round-26 lib.rs append, `app!` resolves every manifest path; this task ONLY adds clock/rng primitives, no handler-resolution change. (The wider `cargo test -p auctioneer-core --lib host_primitives` umbrella becomes green at end of Task 37b once `fanout` + `registry` files land.)

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-core
git commit -m "auctioneer-core: SystemClock + DevRng (dev host primitives, green checkpoint)"
```

---

### Task 37: Dev `BidderFanout` (interim `ProxyClient` + `join_all`)

> **Round-13 review High 6 — auction core-dev scope, NOT behavior-complete.** This dev fanout ships in auction core deliberately incomplete: `AuctionDeadline` is not enforced mid-fanout (a stuck call cannot be cancelled), and `BidderCall.timeout` (the per-bidder budget materialized by Round-12 Medium 11) is not applied either. Both gaps are closed in GDPR enforcement by the production fanout. auction core verification accepts the gaps; the acceptance checklist at end-of-plan flags them as "auction core dev complete, GDPR enforcement behavior-complete" rather than "auction core done."

**Files:**

- Create: `crates/auctioneer-core/src/host_primitives/fanout.rs`
- Modify: `crates/auctioneer-core/src/host_primitives/mod.rs` — round-21 review Blocker 2. Append the new module declaration and re-exports:

```rust
pub mod fanout;
pub use fanout::{DevFanout, ProxyClientApi};
```

**Interfaces:**

- Consumes: `auctioneer-exchange::{BidderFanout, BidderCall, BidderCallResult, FanoutError, FanoutInternalError, AuctionDeadline}`.
- Produces: `pub struct DevFanout<'h, H>` parameterized over an EdgeZero `ProxyClient`. The interim impl per §5.2:
  - dispatches every call concurrently via `futures::future::join_all`,
  - applies post-buffer `max_response_bytes` (no streaming cap — spec round-69 user sync known gap),
  - records per-call `elapsed` via `Clock::now_monotonic`,
  - **does NOT enforce `AuctionDeadline`** (auction core known gap; GDPR enforcement closes via the production fanout),
  - **does NOT apply per-bidder `BidderCall.timeout`** (auction core known gap; the per-bidder value is materialized into `BidderCall` so the GDPR enforcement fanout can read it without API changes),
  - gzip-encodes the body when `endpoint_compression == Gzip` and rewrites `Content-Encoding` / `Content-Length` headers (round-25 diagnostic qualification).

- [ ] **Step 1: Implement `host_primitives/fanout.rs`**

```rust
//! Dev `BidderFanout` — interim impl per §5.2.
//!
//! **Known gaps documented per spec §5.2:**
//! 1. No `AuctionDeadline` enforcement.
//! 2. No per-bidder `timeout`.
//! 3. Fastly runs serialized under the underlying ProxyClient.
//! 4. `max_response_bytes` enforced post-buffer (not streaming).

use async_trait::async_trait;
use flate2::Compression as GzCompression;
use flate2::write::GzEncoder;
use futures::future::join_all;
use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse};
use auctioneer_exchange::{
    AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, Compression,
    FanoutError, FanoutInternalError,
};
use auctioneer_runtime::Clock;
use std::io::Write;
use std::time::Duration;

/// EdgeZero ProxyClient trait stand-in. The real EdgeZero `ProxyClient`
/// surface is bound at the call site (Task 38 wires it). For the
/// development impl we depend only on a minimal "send_one" surface so
/// auctioneer-core stays portable across EdgeZero versions.
///
/// Round-25 review Blocker 2: the `Send + Sync` supertrait is REQUIRED
/// so the `BidderFanout for DevFanout` impl satisfies its own
/// `Send + Sync` supertrait (see L3688). Without these bounds,
/// `DevFanout<'_, _, EdgeZeroProxyAdapter>: Send + Sync` does not hold
/// because `&P` is Send/Sync only when `P: Sync`. EdgeZero's own
/// `ProxyClient` trait carries the same supertrait pair so we are
/// only restating the contract one level up.
#[async_trait(?Send)]
pub trait ProxyClientApi: Send + Sync {
    async fn send_one(&self, req: &BidderHttpRequest) -> Result<BidderHttpResponse, String>;
}

/// Round-20 review Blocker 6: the `C` generic must be `?Sized` so
/// `FanoutSeam::Production` can instantiate `DevFanout<'_, dyn Clock
/// + Send + Sync, _>`. Without `?Sized`, Rust silently adds `Sized`
/// to type parameters; `dyn Clock` is unsized, so the seam's
/// trait-object clock would fail to compile.
pub struct DevFanout<'h, C: Clock + ?Sized, P: ProxyClientApi> {
    pub client: &'h P,
    pub clock: &'h C,
}

// Round-25 review Blocker 2: BidderFanout (spec line 3692 / plan
// L3688) requires `Send + Sync` on the type. `DevFanout` is
// Send + Sync iff `&P: Send + Sync` AND `&C: Send + Sync`, which
// requires `P: Send + Sync` and `C: Send + Sync` (a `&T` is Send/Sync
// iff `T: Sync`; we ask for `Send` too so `Arc<DevFanout>` round-
// trips). The `ProxyClientApi: Send + Sync` supertrait above
// supplies the `P` side; we add the explicit `C: Send + Sync` bound
// here. EdgeZeroProxyAdapter satisfies these via ProxyHandle (which
// wraps `Arc<dyn ProxyClient>`; `ProxyClient: Send + Sync` so the
// Arc is too), and SystemClock / CloudflareClock both ship Send+Sync
// in foundations — see the compile assertion at the bottom of this file.
#[async_trait(?Send)]
impl<'h, C: Clock + ?Sized + Send + Sync, P: ProxyClientApi> BidderFanout for DevFanout<'h, C, P> {
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        _deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
        let n = calls.len();
        let mut futures_vec = Vec::with_capacity(n);
        for c in &calls {
            futures_vec.push(self.send_one(c));
        }
        let outs = join_all(futures_vec).await;
        Ok(outs)
    }
}

impl<'h, C: Clock + ?Sized + Send + Sync, P: ProxyClientApi> DevFanout<'h, C, P> {
    async fn send_one(&self, c: &BidderCall) -> BidderCallResult {
        let start = self.clock.now_monotonic();
        // Apply gzip + rewrite entity headers when configured.
        let mut req = c.request.clone();
        if c.endpoint_compression == Some(Compression::Gzip) {
            match gzip_encode(&req.body) {
                Ok(encoded) => {
                    req.body = encoded;
                    // Strip adapter-supplied entity headers, then set our own.
                    req.headers.retain(|(k, _)| {
                        !k.eq_ignore_ascii_case("content-encoding")
                            && !k.eq_ignore_ascii_case("content-length")
                    });
                    req.headers.push(("Content-Encoding".into(), "gzip".into()));
                    req.headers.push(("Content-Length".into(), req.body.len().to_string()));
                }
                Err(e) => {
                    return BidderCallResult {
                        result: Err(FanoutError::Other(format!("gzip encode: {e}"))),
                        // Round-18 review High 9: saturating_sub guards
                        // against backward clock movement per spec line
                        // 1702 (Cloudflare dev clock).
                        elapsed: self.clock.now_monotonic().saturating_sub(start),
                    };
                }
            }
        }

        let outcome = self.client.send_one(&req).await;
        let elapsed = self.clock.now_monotonic().saturating_sub(start);
        let result = match outcome {
            Ok(mut resp) => {
                if resp.body.len() > c.max_response_bytes {
                    Err(FanoutError::OversizeResponse)
                } else if resp.status >= 500 {
                    Err(FanoutError::BadServerResponse)
                } else {
                    Ok(resp)
                }
            }
            Err(msg) if msg.to_lowercase().contains("timeout") => Err(FanoutError::Timeout),
            Err(msg) if msg.to_lowercase().contains("unreach") || msg.to_lowercase().contains("dns") =>
                Err(FanoutError::Unreachable),
            Err(msg) => Err(FanoutError::Other(msg)),
        };
        BidderCallResult { result, elapsed }
    }
}

fn gzip_encode(input: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut enc = GzEncoder::new(Vec::new(), GzCompression::default());
    enc.write_all(input)?;
    enc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_runtime::FakeClock;

    struct MockClient {
        resp: BidderHttpResponse,
        fail: Option<String>,
    }
    #[async_trait(?Send)]
    impl ProxyClientApi for MockClient {
        async fn send_one(&self, _req: &BidderHttpRequest) -> Result<BidderHttpResponse, String> {
            match &self.fail {
                Some(m) => Err(m.clone()),
                None => Ok(self.resp.clone()),
            }
        }
    }

    #[test]
    fn timeout_error_is_categorized() {
        futures::executor::block_on(async {
            let client = MockClient {
                resp: BidderHttpResponse { status: 200, headers: vec![], body: vec![] },
                fail: Some("timeout exceeded".into()),
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let fanout = DevFanout { client: &client, clock: &clock };
            let calls = vec![BidderCall {
                request: BidderHttpRequest {
                    method: "POST".into(), url: "https://x".into(),
                    headers: vec![], body: vec![], covered_imp_ids: vec![],
                },
                timeout: Duration::from_millis(100),
                max_response_bytes: 1024,
                endpoint_compression: None,
            }];
            let r = fanout.send_all(calls, AuctionDeadline {
                deadline_at: Duration::from_secs(1), source: auctioneer_exchange::TmaxSource::Request,
            }).await.unwrap();
            assert!(matches!(r[0].result, Err(FanoutError::Timeout)));
        });
    }

    #[test]
    fn oversize_response_categorized() {
        futures::executor::block_on(async {
            let body = vec![0u8; 4096];
            let client = MockClient {
                resp: BidderHttpResponse { status: 200, headers: vec![], body },
                fail: None,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let fanout = DevFanout { client: &client, clock: &clock };
            let calls = vec![BidderCall {
                request: BidderHttpRequest {
                    method: "POST".into(), url: "https://x".into(),
                    headers: vec![], body: vec![], covered_imp_ids: vec![],
                },
                timeout: Duration::from_millis(100),
                max_response_bytes: 100,
                endpoint_compression: None,
            }];
            let r = fanout.send_all(calls, AuctionDeadline {
                deadline_at: Duration::from_secs(1), source: auctioneer_exchange::TmaxSource::Request,
            }).await.unwrap();
            assert!(matches!(r[0].result, Err(FanoutError::OversizeResponse)));
        });
    }
}
```

- [ ] **Step 2: Add deps in `crates/auctioneer-core/Cargo.toml`**

```toml
async-trait     = { workspace = true }
flate2          = { workspace = true }
futures         = { workspace = true }
auctioneer-bidder   = { workspace = true }
auctioneer-config   = { workspace = true }
auctioneer-exchange = { workspace = true }
auctioneer-openrtb  = { workspace = true }
auctioneer-privacy  = { workspace = true }
auctioneer-runtime  = { workspace = true }
serde           = { workspace = true }
serde_json      = { workspace = true }
http            = { workspace = true }
time            = { workspace = true }

[features]
insecure-bidders = [
    "auctioneer-bidder/insecure-bidders",
    "auctioneer-exchange/insecure-bidders",
]

# Round-14 review Blocker tokio: NO tokio dev-dep. Async tests use
# futures::executor::block_on so the workspace dep-purity gate
# (Task 41) stays strict.
```

- [ ] **Step 3: Test**

```
cargo test -p auctioneer-core --lib host_primitives::fanout
```

Expected: 2 passed.

- [ ] **Step 4: Commit**

```
git add crates/auctioneer-core
git commit -m "auctioneer-core: dev BidderFanout (interim ProxyClient + join_all)"
```

---

### Task 37b: Cloudflare host primitives shim (round-15 review High Cloudflare)

The round-12 preamble promised a "Task 38b" for Cloudflare's `Clock` + `Rng` shims but never landed a concrete task. Per spec §14 line 4534 + §5.7, Cloudflare requires its own host-primitive impl (Workers does not expose `std::time::Instant`, `SystemTime`, or `getrandom`). Without this, an `--target wasm32-unknown-unknown -p auctioneer-adapter-cloudflare` build for auction core development still falls back to `DevRng`/`SystemClock` from Task 36 — neither of which compile on Workers.

**Files:**

- Modify: `crates/auctioneer-core/src/host_primitives/mod.rs` — round-21 review Blocker 2. Append the registry module declaration (no re-exports because the public API of `registry` is read through fully-qualified paths in the handler / test code):

```rust
pub mod registry;
```

- Create: `crates/auctioneer-core/src/host_primitives/registry.rs` (round-20 review Blocker 3: the OnceLock seam belongs in auctioneer-core so every adapter binds against the same address).
- Modify: `crates/auctioneer-adapter-cloudflare/Cargo.toml` (add `auctioneer-runtime`, `time`, the `worker` crate's required features) — round-20 review Blocker 8.
- Modify: `crates/auctioneer-adapter-cloudflare/src/lib.rs` (declare `mod clock; mod rng;`; install primitives idempotently from the per-fetch entrypoint).
- Create: `crates/auctioneer-adapter-cloudflare/src/clock.rs`
- Create: `crates/auctioneer-adapter-cloudflare/src/rng.rs`
- Create: `crates/auctioneer-adapter-cloudflare/tests/host_primitives.rs`

**Interfaces:**

- Consumes: `auctioneer-runtime::{Clock, Rng}`, `worker::Date`, `worker::js_sys::Math::random`.
- Produces:
  - `pub struct CloudflareClock` implementing `Clock` via `worker::Date::now()` for wall-time and a monotonic source backed by `worker::js_sys::Date::now()` (Cloudflare Workers doesn't expose `performance.now()` to the guest in non-isolate runtimes; pin the available source in a comment).
  - `pub struct CloudflareRng` implementing `Rng` via xorshift64* seeded from `worker::Date::now() XOR worker::js_sys::Math::random()` per spec line 4539. NOT `DevRng` (which uses `SystemTime`, unavailable on Workers) AND NOT `crypto::getRandomValues` (round-22 review Medium 9: the spec pins seeded xorshift here for determinism under test; earlier drafts described a crypto-backed RNG and are RESCINDED).

- [ ] **Step 1: Implement `clock.rs`** — round-16 review High 8 demands concrete code, not prose. The clock uses `worker::Date::now()` for wall-time (returns `f64` ms-since-epoch) and the same source as a monotonic counter (Workers does not expose `performance.now()` to the guest in non-isolate runtimes, and `Instant::now()` doesn't compile on `wasm32-unknown-unknown` without `js-sys`).

```rust
//! Cloudflare Workers Clock. Spec §5.7 / §14 auction core prerequisite #2.
//!
//! `worker::Date::now()` returns ms since Unix epoch. We use the
//! same source for `now_monotonic` — request lifetime is short
//! enough that JS-side clock skew within a single request is
//! bounded by the worker's invocation slot.

use auctioneer_runtime::Clock;
use std::time::Duration;
use time::OffsetDateTime;
use worker::Date;

pub struct CloudflareClock {
    epoch_ms: u64,
}

impl CloudflareClock {
    #[must_use]
    pub fn new() -> Self {
        let epoch_ms = Date::now().as_millis();
        Self { epoch_ms }
    }
}

impl Default for CloudflareClock {
    fn default() -> Self { Self::new() }
}

impl Clock for CloudflareClock {
    fn now(&self) -> OffsetDateTime {
        let now_ms = Date::now().as_millis();
        // OffsetDateTime::from_unix_timestamp_nanos accepts i128.
        let nanos = i128::from(now_ms).saturating_mul(1_000_000);
        OffsetDateTime::from_unix_timestamp_nanos(nanos)
            .unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }
    fn now_monotonic(&self) -> Duration {
        let now_ms = Date::now().as_millis();
        Duration::from_millis(now_ms.saturating_sub(self.epoch_ms))
    }
}
```

- [ ] **Step 2: Implement `rng.rs`** — round-16 review High 8 + round-19 review Medium 11 + round-22 review Medium 9. Uses xorshift64* seeded from `worker::Date::now() XOR worker::js_sys::Math::random()` per spec line 4539. NOT `SystemTime`-seeded `DevRng` (does not compile on Workers) AND NOT `crypto::getRandomValues` (the spec pins seeded xorshift for test determinism).

```rust
//! Cloudflare Workers Rng. Spec §5.7 / §14 auction core prerequisite #3 +
//! spec line 4539: xorshift64* seeded from
//! `worker::Date::now() XOR Math::random()`.

use auctioneer_runtime::Rng;
use std::sync::Mutex;
use worker::Date;
use worker::js_sys::Math;

pub struct CloudflareRng {
    state: Mutex<u64>,
}

impl CloudflareRng {
    #[must_use]
    pub fn new() -> Self {
        // Spec line 4539: seed from Date.now() XOR Math.random()
        // converted to a u64. xorshift64* requires a non-zero seed;
        // fall back to 1 if XOR yields zero (vanishingly rare).
        let now_ms = Date::now().as_millis();
        // Math::random() returns f64 in [0, 1); multiply by 2^53 to
        // get an integer seed component.
        // Round-21 review High 5: the workspace clippy gate forbids
        // bare `as` conversions. Math::random() returns [0, 1); we
        // scale to 2^53 (the JS-safe integer range) and then
        // truncate via `floor` + `try_from`. A `#[expect]` annotation
        // covers the f64 → integer conversion that has no try_from.
        // Round-22 review High 5: add `clippy::as_conversions` to
        // the expect list — the workspace `restriction` gate denies
        // it AS WELL AS the precision/truncation/sign lints. Without
        // `as_conversions` the `as u64` cast below still trips the
        // gate even with the other three lints expected.
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "Math::random() is [0,1); scaled to 2^53 and floored — value is in 0..=2^53-1, fits u64 losslessly",
        )]
        let math_bits = (Math::random() * f64::from(1_u32 << 30) * f64::from(1_u32 << 23)).floor() as u64;
        let mut seed = now_ms ^ math_bits;
        if seed == 0 { seed = 1; }
        Self { state: Mutex::new(seed) }
    }
}

impl Default for CloudflareRng {
    fn default() -> Self { Self::new() }
}

impl Rng for CloudflareRng {
    /// Round-17 review Blocker 9: the foundational `auctioneer_runtime::Rng` trait
    /// has a SINGLE required method, `next_u64`. `gen_range` and
    /// `shuffle` live as free functions in `auctioneer_runtime::helpers`
    /// — every `Rng` impl gets them automatically. Implement
    /// `next_u64` only.
    ///
    /// Round-19 review Medium 11: aligned with spec line 4539. The
    /// Cloudflare dev RNG is xorshift64* seeded from
    /// `worker::Date::now() XOR Math::random()` (NOT
    /// `crypto.getRandomValues` per call). xorshift gives
    /// deterministic-under-seed behavior the auction core dev tests need
    /// while staying cheap on Workers. GDPR enforcement may upgrade to crypto
    /// once tests stop relying on seed determinism.
    fn next_u64(&self) -> u64 {
        let mut state = self.state.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // xorshift64*; identical to auctioneer_runtime::seeded_rng's
        // step function. Initial state seeded in CloudflareRng::new().
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}
```

- [ ] **Step 3: Wire the shim into the Cloudflare adapter's host-primitives slot** — round-18 review Blocker 2 + 3 + **round-19 review Blocker 1**. The extension types are `Arc<dyn Clock + Send + Sync>` / `Arc<dyn Rng + Send + Sync>` so auctioneer-core can read them without depending on adapter crates.

Round-19 review Blocker 1: the `app!` macro EXPANDS INSIDE auctioneer-core, so a `crate::middleware::install_host_primitives` path in `edgezero.toml` would resolve to `auctioneer_core::middleware::...`, NOT the Cloudflare adapter. The pinned EdgeZero app entry point is a single per-app manifest baked at auctioneer-core build time; the four adapters just call `run_app::<auctioneer_core::App>(...)`. Adapter-side middleware registered via the manifest cannot resolve into the adapter crate.

The realistic seam is therefore CORE-OWNED middleware whose impl is selected by a runtime registry — adapters install their host-primitive `Arc<dyn Clock>` / `Arc<dyn Rng>` into a `OnceLock` at startup, and a single core middleware reads from that lock on every request:

Round-20 review Blocker 4: the registry MUST be idempotent. Cloudflare's actual entrypoint is the `worker::event::fetch` handler that fires per-request (see `crates/auctioneer-adapter-cloudflare/src/lib.rs` line 10) — there is no `_start`. A non-idempotent `OnceLock::set` would panic on the second request. Use `OnceLock::get_or_init` so each adapter can call `install` from inside its per-request handler harmlessly; only the first call constructs the impls.

Round-22 review Medium 8: the SINGLE authoritative test seam is the `TestClockHandle` / `TestRngHandle` typed-extension override (introduced in round-21 review Blocker 1). The handler's `clock_handle(&ctx)` / `rng_handle(&ctx)` helpers in `handlers/auction.rs` check `ctx.request().extensions()` for those handles FIRST and fall back to the global `OnceLock` only when absent. No `override_clock` / `override_rng` free functions, no `test-host-primitives` cargo feature, no `force_install_for_test` unsafe path. (Earlier rounds drafted alternatives; they are SUPERSEDED by this one authoritative seam.)

```rust
// crates/auctioneer-core/src/host_primitives/registry.rs

use auctioneer_runtime::{Clock, Rng};
use std::sync::{Arc, OnceLock};

static CLOCK: OnceLock<Arc<dyn Clock + Send + Sync>> = OnceLock::new();
static RNG:   OnceLock<Arc<dyn Rng   + Send + Sync>> = OnceLock::new();

/// Idempotent install — safe to call from inside a per-request
/// handler. The first call constructs and stores the impl; every
/// subsequent call is a cheap clone-free no-op.
///
/// Cloudflare's per-fetch entrypoint (`worker::event(fetch)`) calls
/// this on every request. Axum / Spin / Fastly call once from main
/// before `run_app`, but the API tolerates either pattern.
pub fn install_host_primitives<C, R>(make_clock: C, make_rng: R)
where
    C: FnOnce() -> Arc<dyn Clock + Send + Sync>,
    R: FnOnce() -> Arc<dyn Rng + Send + Sync>,
{
    CLOCK.get_or_init(make_clock);
    RNG.get_or_init(make_rng);
}

// Round-22 review High 5: `expect()` violates the workspace
// `restriction = deny` clippy gate (`expect_used`). The adapter
// startup contract is "always install before run_app"; a missing
// install is a logic error at the deployment level, NOT a runtime
// failure path the handler should branch on. Annotate the two
// helpers per-site with `#[expect(clippy::expect_used, ...)]`
// matching the foundations pattern in build_support.rs.
#[expect(
    clippy::expect_used,
    reason = "adapter init contract: every adapter calls install_host_primitives before run_app; missing install is a deployment bug, not a runtime branch"
)]
pub(crate) fn clock_handle() -> Arc<dyn Clock + Send + Sync> {
    CLOCK.get()
        .expect("host primitives not installed; adapter must call install_host_primitives")
        .clone()
}
#[expect(
    clippy::expect_used,
    reason = "same as clock_handle above"
)]
pub(crate) fn rng_handle() -> Arc<dyn Rng + Send + Sync> {
    RNG.get()
        .expect("host primitives not installed; adapter must call install_host_primitives")
        .clone()
}

/// Round-21 review Blocker 1: SAFE test seam. The workspace
/// denies `unsafe_code` (see root `Cargo.toml`), so the earlier
/// raw-pointer override is replaced with typed per-request
/// extensions the handler reads BEFORE consulting the global
/// registry. Tests insert these into the `RequestContext`
/// extensions; production code never sees them. No global
/// mutation, no unsafe, and no `test-host-primitives` cargo
/// feature is needed at all.
pub struct TestClockHandle(pub Arc<dyn Clock + Send + Sync>);
pub struct TestRngHandle(pub Arc<dyn Rng + Send + Sync>);
```

The auction handler reads from the OnceLock through `clock_handle()` / `rng_handle()` instead of inspecting request extensions; no manifest middleware is registered.

The Cloudflare adapter's per-fetch entry point becomes:

```rust
// crates/auctioneer-adapter-cloudflare/src/lib.rs
#![cfg_attr(target_arch = "wasm32", no_main)]

#[cfg(target_arch = "wasm32")]
use worker::*;

#[cfg(target_arch = "wasm32")]
mod clock;
#[cfg(target_arch = "wasm32")]
mod rng;

#[cfg(target_arch = "wasm32")]
#[event(fetch)]
pub async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    // Idempotent: only the first fetch in this isolate constructs
    // the clock / rng. Round-20 review Blocker 4.
    auctioneer_core::host_primitives::registry::install_host_primitives(
        || std::sync::Arc::new(clock::CloudflareClock::new()),
        || std::sync::Arc::new(rng::CloudflareRng::new()),
    );
    edgezero_adapter_cloudflare::run_app::<auctioneer_core::App>(req, env, ctx).await
}
```

Axum / Spin / Fastly adapter entrypoints each call the same `install_host_primitives` with `SystemClock` + `DevRng` (or per-target equivalents) — Axum from `main()` before `serve()`, Spin from `#[spin_sdk::http_component]` per request, Fastly from `#[fastly::main]` per request. The idempotent contract means each call site can use whichever pattern matches the runtime's entrypoint shape. Tasks 36b / 36c / 36d cover the three dev adapters' wiring.

- [ ] **Step 3c: Update `crates/auctioneer-adapter-cloudflare/Cargo.toml`** — round-20 review Blocker 8 + round-21 review High 6. The current Cargo.toml keeps `auctioneer-core` as a PATH dep (not a workspace dep — Task 1's `[workspace.dependencies]` only declares `auctioneer-bidder/-config/-exchange/-openrtb/-privacy/-runtime`, not `auctioneer-core`). DO NOT switch to `auctioneer-core = { workspace = true }` — add the WASM-target-specific deps only. The existing `auctioneer-core` path dep stays as-is:

```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
auctioneer-runtime = { workspace = true }   # round-20 Blocker 8
time           = { workspace = true }   # round-20 Blocker 8 (OffsetDateTime)
worker         = { workspace = true, features = ["js"] }
js-sys         = "0.3"                   # Math::random shim
```

(The existing `[dependencies] auctioneer-core = { path = "..." }` line stays. `worker` already pulls `wasm-bindgen` transitively; declare `js-sys` explicitly so `Math::random` resolves without relying on workers-rs's re-export.)

- [ ] **Step 3d: Wire `mod clock; mod rng;` in `crates/auctioneer-adapter-cloudflare/src/lib.rs`** — round-20 review Blocker 8. The existing file currently declares no sub-modules (see lines 1-13). Add the two `mod` declarations under the `#[cfg(target_arch = "wasm32")]` gate (matching the file's existing pattern) so the per-fetch entrypoint can reference `clock::CloudflareClock` / `rng::CloudflareRng`.

- [ ] **Step 4: Cross-target build assertion**

```
cargo check -p auctioneer-adapter-cloudflare --target wasm32-unknown-unknown
```

Expected: success. (Cloudflare's adapter must build on `wasm32-unknown-unknown` for auction core development; the Spin / Fastly adapters cover their own targets per Task 36.)

If the workers-rs API surface differs from what's pinned above (`worker::Date::now()` and `worker::js_sys::Math::random()`), the implementer adapts the call site BUT KEEPS THE CONTRACT: `CloudflareRng::next_u64()` produces values from a seeded xorshift64* state where the seed combines the wall-time-millis value with `Math::random()` per spec line 4539. The Cargo.lock-pinned `worker` crate is the authoritative reference.

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-adapter-cloudflare/src crates/auctioneer-adapter-cloudflare/tests
git commit -m "auctioneer-adapter-cloudflare: Workers Clock + Rng shims (auction core dev prerequisite)"
```

---

### Task 36b: `auctioneer-adapter-axum` — `install_host_primitives` before `run_app`

**Files:**

- Modify: `crates/auctioneer-adapter-axum/src/main.rs`

**Interfaces:**

- Consumes: `auctioneer_core::host_primitives::registry::install_host_primitives`, `auctioneer_core::host_primitives::{SystemClock, DevRng}`, `edgezero_adapter_axum::dev_server::run_app`, `auctioneer_core::App`.
- Produces: an entry point that calls `install_host_primitives` ONCE during process startup, before `run_app` accepts the first request. Without this, the first auction handler fires `expect("host primitives not installed; ...")` from `registry::clock_handle()` / `rng_handle()` — a process-killing panic that masquerades as a 500.

Round-26 review Blocker 2 (declared the task); round-27 review Blocker 2 (fixed the snippet to match the actual on-disk Axum entry point at `crates/auctioneer-adapter-axum/src/main.rs`, which uses `edgezero_adapter_axum::dev_server::run_app::<auctioneer_core::App>()` — synchronous, no args, no `#[tokio::main]`).

- [ ] **Step 1: Edit `crates/auctioneer-adapter-axum/src/main.rs`**

```rust
// The on-disk file currently reads:
//     use edgezero_adapter_axum::dev_server::run_app;
//     fn main() -> anyhow::Result<()> {
//         run_app::<auctioneer_core::App>()
//     }
// Append the install call AHEAD of the run_app invocation — the
// install is idempotent (round-20 review Blocker 4 contract) so a
// repeated call from any future entry-point shape is harmless.

use edgezero_adapter_axum::dev_server::run_app;
use auctioneer_core::host_primitives::registry::install_host_primitives;
use auctioneer_core::host_primitives::{SystemClock, DevRng};

fn main() -> anyhow::Result<()> {
    install_host_primitives(
        || std::sync::Arc::new(SystemClock::default()),
        || std::sync::Arc::new(DevRng::default()),
    );
    run_app::<auctioneer_core::App>()
}
```

- [ ] **Step 2: Test**

```
cargo check -p auctioneer-adapter-axum
cargo build -p auctioneer-adapter-axum
```

Expected: both succeed. (`cargo run` is skipped — adapter binaries are CI-built but not started; the integration tests in `auctioneer-core` cover the runtime behavior.)

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-adapter-axum/src/main.rs
git commit -m "auctioneer-adapter-axum: install host primitives before run_app"
```

---

### Task 36c: `auctioneer-adapter-spin` — per-request `install_host_primitives`

**Files:**

- Modify: `crates/auctioneer-adapter-spin/src/lib.rs`

**Interfaces:**

- Consumes: `auctioneer_core::host_primitives::registry::install_host_primitives`, `auctioneer_core::host_primitives::{SystemClock, DevRng}`, `spin_sdk::http_service`, `spin_sdk::http::{IntoResponse, Request}`, `edgezero_adapter_spin::run_app`, `auctioneer_core::App`.
- Produces: a `#[spin_sdk::http_service]`-wrapped entrypoint that calls `install_host_primitives` BEFORE delegating to `edgezero_adapter_spin::run_app::<auctioneer_core::App>(req).await`. The install is per-request because Spin re-instantiates the Wasm component on each request; the OnceLock idempotency contract still holds within a single component instance and harmlessly no-ops on the second through Nth call within one process.

Round-26 review Blocker 2 (declared the task); round-27 review Blocker 2 (corrected the snippet to match the actual on-disk Spin entry point — `#[http_service] async fn handle(req: Request) -> anyhow::Result<impl IntoResponse>`, NOT the `IncomingRequest` / `ResponseOutparam` shape an earlier draft used).

- [ ] **Step 1: Edit `crates/auctioneer-adapter-spin/src/lib.rs`**

```rust
// The on-disk file currently reads (under `cfg(target_arch = "wasm32")`):
//     use spin_sdk::http::{IntoResponse, Request};
//     use spin_sdk::http_service;
//     #[http_service]
//     async fn handle(req: Request) -> anyhow::Result<impl IntoResponse> {
//         edgezero_adapter_spin::run_app::<auctioneer_core::App>(req).await
//     }
// Append the install call inside `handle` before the run_app
// invocation.

#![cfg_attr(
    target_arch = "wasm32",
    allow(
        unsafe_code,
        reason = "spin's #[http_service] macro generates the unsafe wasm export"
    )
)]

#[cfg(target_arch = "wasm32")]
use auctioneer_core::host_primitives::registry::install_host_primitives;
#[cfg(target_arch = "wasm32")]
use auctioneer_core::host_primitives::{SystemClock, DevRng};
#[cfg(target_arch = "wasm32")]
use spin_sdk::http::{IntoResponse, Request};
#[cfg(target_arch = "wasm32")]
use spin_sdk::http_service;

#[cfg(target_arch = "wasm32")]
#[http_service]
async fn handle(req: Request) -> anyhow::Result<impl IntoResponse> {
    install_host_primitives(
        || std::sync::Arc::new(SystemClock::default()),
        || std::sync::Arc::new(DevRng::default()),
    );
    edgezero_adapter_spin::run_app::<auctioneer_core::App>(req).await
}
```

- [ ] **Step 2: Test**

```
cargo check -p auctioneer-adapter-spin --target wasm32-wasip1
```

Expected: success. (`wasm32-wasip1` is the foundations-pinned Spin target; if it has migrated to `wasm32-wasi-preview2` on the current branch, swap.)

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-adapter-spin/src/lib.rs
git commit -m "auctioneer-adapter-spin: install host primitives per request"
```

---

### Task 36d: `auctioneer-adapter-fastly` — per-request `install_host_primitives`

**Files:**

- Modify: `crates/auctioneer-adapter-fastly/src/main.rs`

**Interfaces:**

- Consumes: `auctioneer_core::host_primitives::registry::install_host_primitives`, `auctioneer_core::host_primitives::{SystemClock, DevRng}`, `fastly::{Error, Request, Response}`, `edgezero_adapter_fastly::run_app`, `auctioneer_core::App`.
- Produces: a `#[fastly::main]`-wrapped entrypoint that calls `install_host_primitives` BEFORE delegating to `edgezero_adapter_fastly::run_app::<auctioneer_core::App>(req)`. Same per-request idempotency as Spin (Fastly Compute@Edge re-instantiates the Wasm guest on each request).

Round-26 review Blocker 2 (declared the task); round-27 review Blocker 2 (corrected the path — the Fastly entry point is `src/main.rs`, NOT `src/lib.rs`; and the symbol is `edgezero_adapter_fastly::run_app`, NOT `serve`).

- [ ] **Step 1: Edit `crates/auctioneer-adapter-fastly/src/main.rs`**

```rust
// The on-disk file currently reads (under `cfg(target_arch = "wasm32")`):
//     use fastly::{Error, Request, Response};
//     #[fastly::main]
//     pub fn main(req: Request) -> Result<Response, Error> {
//         edgezero_adapter_fastly::run_app::<auctioneer_core::App>(req)
//     }
// Append the install call ahead of run_app. The non-wasm32 stub
// (`fn main() { eprintln!("...") }`) is unchanged.

#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "Fastly entrypoint is wasm32-only")
)]

#[cfg(target_arch = "wasm32")]
use fastly::{Error, Request, Response};
#[cfg(target_arch = "wasm32")]
use auctioneer_core::host_primitives::registry::install_host_primitives;
#[cfg(target_arch = "wasm32")]
use auctioneer_core::host_primitives::{SystemClock, DevRng};

#[cfg(target_arch = "wasm32")]
#[fastly::main]
pub fn main(req: Request) -> Result<Response, Error> {
    install_host_primitives(
        || std::sync::Arc::new(SystemClock::default()),
        || std::sync::Arc::new(DevRng::default()),
    );
    edgezero_adapter_fastly::run_app::<auctioneer_core::App>(req)
}

#[cfg(not(target_arch = "wasm32"))]
#[expect(
    clippy::print_stderr,
    reason = "host stub; the real binary only runs on wasm32-wasip1"
)]
fn main() {
    eprintln!("auctioneer-adapter-fastly: target wasm32-wasip1 to run on Fastly.");
}
```

- [ ] **Step 2: Test**

```
cargo check -p auctioneer-adapter-fastly --target wasm32-wasip1
```

Expected: success. (Fastly's Compute@Edge target is currently `wasm32-wasip1` per foundations; check the adapter's `Cargo.toml` for the active target if pinning has changed.)

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-adapter-fastly/src/main.rs
git commit -m "auctioneer-adapter-fastly: install host primitives per request"
```

---

### Task 38: `auction` handler — thin shim over `run_auction`

**Files:**

- Modify: `crates/auctioneer-core/src/handlers/mod.rs` (round-26 review Blocker 1: this file was created in Task 35 with stub `auction` / `status` / `info_bidders` / `info_bidder` action functions; this task REPLACES only the `auction` stub body — and reshapes the module to delegate to sub-modules per round-16 review Blocker 2)
- Create: `crates/auctioneer-core/src/handlers/auction.rs`
- Create: `crates/auctioneer-core/src/serialize.rs`

**Interfaces:**

- Consumes: `EdgeZero RequestContext` (provides body, headers, query), `run_auction`.
- Produces: `pub async fn auction(ctx: RequestContext) -> Result<Response, EdgeError>` (the pinned `#[edgezero_core::action]` signature — `RequestContext` is non-generic and owned, NOT `RequestContext<'_>`; the action returns `Result<Response, EdgeError>`, not bare `Response`). Performs §5.1 steps 1-5 + step 12: capture request_start, reject non-empty Content-Encoding, bounded body read (256 KiB), parse, host resolution (embedded default in auction core — no config_store wiring yet), account resolution (`DefaultFallback` with id `"unknown"`), call `prepare()`, build `AuctionContext`, invoke `run_auction`, serialize per Appendix E.7 / Appendix F.

- [ ] **Step 1: Implement the serializer**

`src/serialize.rs`:

```rust
//! Deterministic `BidResponse` → JSON bytes. Appendix E.7.

use auctioneer_openrtb::types::bid_response::BidResponse;

/// Round-18 review Medium 10: serialization returns a `Result` so the
/// handler can surface a real 500 when serialization fails (which
/// requires a non-serializable `serde_json::Value` in `ext`).
/// The earlier infallible fallback returned a JSON error blob that
/// the handler then wrapped as 200 application/json — silently
/// masking the failure as a malformed-but-successful response.
#[allow(clippy::module_name_repetitions)]
pub fn serialize_response(resp: &BidResponse) -> Result<Vec<u8>, serde_json::Error> {
    // serde_json::to_vec writes BTreeMap-backed maps in sorted-key order.
    // The Appendix E.7 invariants live entirely in the response builder's
    // structure choice; serialization is mechanical.
    serde_json::to_vec(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_openrtb::types::bid_response::BidResponse;
    #[test]
    fn empty_response_serializes() {
        let bytes = serialize_response(&BidResponse::default()).expect("serialize default");
        assert!(!bytes.is_empty());
    }
}
```

- [ ] **Step 2: Replace `handlers/mod.rs`**

Round-16 review Blocker 2: the EdgeZero manifest names each route via `handler = "crate::handlers::auction"` (etc.), which resolves to a FUNCTION ITEM at that path — not a module. `handlers/mod.rs` therefore re-exports the action functions out of their respective sub-modules so the manifest's qualified paths point at callable items.

Round-26 review Blocker 1: the file landed in Task 35 with inline stub functions. This step REPLACES it with the delegating shape below. Tasks 38 (this task — `auction.rs`) and 39 (`status.rs`, `info_bidders.rs`) provide the sub-modules. Until Task 39 lands the `status` / `info_bidders` files, build `handlers/mod.rs` to declare only the modules whose sub-files exist at this commit boundary — i.e. this task's intermediate state declares ONLY `mod auction;` plus pass-through stubs for `status` / `info_bidders` / `info_bidder` so the manifest still resolves. Task 39 swaps the pass-throughs for the real `mod` declarations.

```rust
//! Per-route handlers — thin shims over `auctioneer_exchange::run_auction`.

mod auction;
pub use auction::auction;

// Status + info handlers: still stubs at this task boundary; Task 39
// replaces them with real `mod status;` + `mod info_bidders;` + the
// matching `pub use`s.

// Round-27 review Blocker 1: only `action` and `app` are re-exported
// at the crate root (`pub use edgezero_macros::{action, app};` —
// edgezero-core/src/lib.rs L32). `EdgeError` lives under the `error`
// module, `RequestContext` under `context`. Importing them through
// the root would fail at the first `cargo check -p auctioneer-core` Task
// 35 runs.
use edgezero_core::{action, context::RequestContext, error::EdgeError};
use edgezero_core::http::Response;

#[action]
pub async fn status(_ctx: RequestContext) -> Result<Response, EdgeError> {
    Err(EdgeError::internal(
        "status handler not yet wired — Task 39 replaces this body",
    ))
}

#[action]
pub async fn info_bidders(_ctx: RequestContext) -> Result<Response, EdgeError> {
    Err(EdgeError::internal(
        "info_bidders handler not yet wired — Task 39 replaces this body",
    ))
}

#[action]
pub async fn info_bidder(_ctx: RequestContext) -> Result<Response, EdgeError> {
    Err(EdgeError::internal(
        "info_bidder handler not yet wired — Task 39 replaces this body",
    ))
}
```

- [ ] **Step 3: Implement `handlers/auction.rs`** as a real `#[action]` function — round-14 review Blocker action wiring + Blocker request_start. The function signature matches what `edgezero_core::app!` registers under the bare name `auction`; `request_start` is captured as the FIRST executable statement so preprocessing time (body read, parse, prepare) counts against tmax per spec §5.3 line 1842.

```rust
//! `POST /openrtb2/auction` handler. See spec §5.1 + §8.1.

use crate::{
    fanout_seam::FanoutSeam,
    host_primitives::{DevRng, SystemClock},
    serialize::serialize_response,
};
use edgezero_core::{
    action,
    body::Body,
    context::RequestContext,
    error::EdgeError,
    http::{response_builder, Response, CONTENT_TYPE},
};
use http::{HeaderValue, StatusCode};
use auctioneer_bidder::BidderRegistry;
use auctioneer_config::{AccountConfig, HostConfig};
use auctioneer_exchange::{
    AccountSource, AuctionContext, AuctionError, ResolvedAccount, run_auction,
};
use auctioneer_openrtb::{prepare, RequestHeaders};
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_runtime::{Clock, Rng};
use std::sync::Arc;

const MAX_AUCTION_BODY: usize = 256 * 1024;

/// Round-14 review Blocker request_start + round-15 review Blocker
/// handler types + round-16 review Blocker 1 (real EdgeZero API):
/// the clock starts ticking at handler entry, BEFORE body read /
/// parse / prepare. Per spec §5.3 line 1842 + §5.8, preprocessing
/// time MUST count against tmax so a slow body or `prepare()`
/// cannot quietly hand the bidder a bigger budget. The signature is
/// `RequestContext -> Result<Response, EdgeError>`, the shape the
/// EdgeZero `#[action]` macro expects (see `edgezero-macros/src/
/// action.rs` line 92).
#[action]
pub async fn auction(ctx: RequestContext) -> Result<Response, EdgeError> {
    // Round-18 review Blocker 2: clock + rng come from per-request
    // extensions injected by the adapter's middleware. Each adapter
    // installs the target-correct impl (CloudflareClock on Workers,
    // SystemClock on Axum / Spin / Fastly); auctioneer-core stays
    // adapter-agnostic by reading `Arc<dyn Clock>` / `Arc<dyn Rng>`.
    let clock = clock_handle(&ctx);
    let rng   = rng_handle(&ctx);
    let request_start = clock.now_monotonic();          // <-- FIRST
    match auction_inner(ctx, clock, rng, request_start).await {
        Ok(response) => Ok(response),
        Err(err)     => handler_error_to_response(err),
    }
}

async fn auction_inner(
    ctx: RequestContext,
    clock: Arc<dyn Clock + Send + Sync>,
    rng:   Arc<dyn Rng   + Send + Sync>,
    request_start: std::time::Duration,
) -> Result<Response, HandlerError> {
    // Round-15 review Blocker E2E: bind the fanout seam BEFORE the
    // body is consumed — `FanoutSeam::from_context` reads
    // `ctx.proxy_handle()` and inspects the typed extensions for a
    // `TestFanoutHandle`, and once we move the body out via
    // `into_request()` the RequestContext is gone.
    let fanout = FanoutSeam::from_context(&ctx, clock.as_ref())
        .ok_or_else(HandlerError::fanout_unavailable)?;
    // Snapshot the headers we need for Content-Encoding check and
    // `prepare()` BEFORE consuming the context.
    let raw_headers = ctx.request().headers().clone();
    if let Some(enc) = raw_headers.get(http::header::CONTENT_ENCODING) {
        if !enc.is_empty() {
            return Err(HandlerError::bad_request(format!(
                "Invalid request: Content-Encoding of type {} is not supported",
                enc.to_str().unwrap_or("<unknown>"),
            )));
        }
    }
    // Round-16 review Blocker 1: the ingress hook applies
    // MAX_AUCTION_BODY before delivering the body (round-69 H1 /
    // round-71 privacy enforcement); the defensive recheck here uses the pinned
    // `Body::into_bytes_bounded` API. Consume the body via
    // `ctx.into_request().into_parts()` — there is no
    // `ctx.body_with_cap` shortcut on the pinned EdgeZero surface.
    let request = ctx.into_request();
    let (_parts, body) = request.into_parts();
    let body_bytes = body.into_bytes_bounded(MAX_AUCTION_BODY).await
        .map_err(|e| HandlerError::from_body_read_error(e, MAX_AUCTION_BODY))?;
    let raw: BidRequest = serde_json::from_slice(&body_bytes)
        .map_err(|e| HandlerError::bad_request(format!("Invalid request: {e}")))?;
    let http_headers = headers_from_http(&raw_headers);
    let normalized = prepare(raw, &http_headers)
        .map_err(|e| HandlerError::bad_request(format!("Invalid request: {e}")))?;

    // Round-18 review Blocker 2: clock + rng are already bound from
    // the adapter-installed extensions (see the #[action] entry
    // point); the inner pipeline just borrows the trait objects.
    let registry = registry_handle();
    let host = host_config_handle();
    // auction core: account resolution is a default-fallback stub (config_store
    // is an GDPR enforcement deliverable). See "auction core dev-vs-behavior split"
    // acceptance note — code 10 `MalformedAcct` is therefore NOT a
    // path the auction core surface can reach; it remains in the AuctionError
    // enum so the GDPR enforcement config_store wiring can surface it without an
    // API change.
    let acct = AccountConfig::default();
    let resolved = ResolvedAccount {
        id: "unknown".into(),
        source: AccountSource::DefaultFallback,
        config: &acct,
    };
    let ctx_auction = AuctionContext::with_request_start(
        request_start, host, &resolved, registry, &fanout,
        clock.as_ref(), rng.as_ref(), false,
    );
    let resp = run_auction(&normalized, &ctx_auction).await
        .map_err(HandlerError::from_auction_error)?;

    // Round-16 review Medium 10: build the success response with
    // explicit `application/json` per Appendix F (spec line 5358).
    // `response_with_body` would default a non-empty body to
    // `text/plain; charset=utf-8` (see edgezero-core/src/response.rs
    // line 98), so we use `response_builder()` directly and pin the
    // Content-Type header explicitly.
    // Round-18 review Medium 10: serialize_response is now fallible
    // (returns Result<Vec<u8>, serde_json::Error>); a serialization
    // failure surfaces as `HandlerError::Internal` → 500, NOT a
    // 200 with a JSON error blob in the body.
    let body_bytes = serialize_response(&resp)
        .map_err(|e| HandlerError::Internal(format!("serialize: {e}")))?;
    let content_length = body_bytes.len();
    response_builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .header(http::header::CONTENT_LENGTH, content_length.to_string())
        .body(Body::from_bytes(body_bytes))
        .map_err(|e| HandlerError::Internal(format!("response builder: {e}")))
}

fn handler_error_to_response(err: HandlerError) -> Result<Response, EdgeError> {
    // Round-16 review Blocker 1 + Medium 10: build via
    // `response_builder()` and pin Content-Type explicitly so the
    // Appendix F text-error wire shape is preserved.
    let (status, content_type, body) = err.into_parts();
    response_builder()
        .status(status)
        .header(CONTENT_TYPE, HeaderValue::from_static(content_type))
        .body(Body::from_bytes(body))
        .map_err(EdgeError::internal)
}

/// Handler-side error envelope. Carries the HTTP status + a wire body
/// shaped per Appendix F. `handler_error_to_response` is the
/// fallback path in the `#[action] auction` body.
enum HandlerError {
    BadRequest(String),
    FanoutUnavailable,
    Internal(String),
}

impl HandlerError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
    fn fanout_unavailable() -> Self {
        Self::FanoutUnavailable
    }
    fn from_auction_error(err: AuctionError) -> Self {
        match err {
            AuctionError::BadInput(m) =>
                Self::BadRequest(format!("Invalid request: {m}")),
            AuctionError::InvalidImpFirstPartyData(m) =>
                Self::Internal(format!("Invalid request: {m}")),
            AuctionError::FanoutUnavailable => Self::FanoutUnavailable,
            AuctionError::Internal(m) =>
                Self::Internal(format!("internal: {m}")),
        }
    }
    /// Round-16 review Blocker 1 + round-17 review Blocker 3 +
    /// round-18 review High 8: `Body::into_bytes_bounded` returns
    /// `EdgeError::bad_request("request body too large")` ONLY when
    /// the body exceeded the cap; upstream stream errors come back
    /// as `EdgeError::internal(...)`. Use `err.status()` to
    /// distinguish:
    ///   - 400: oversize body → spec-exact "request size exceeded
    ///     max size of N bytes." wire body per Appendix F line 5349.
    ///   - everything else: unreadable body → "Invalid request: body
    ///     read failed: <inner>" so the operator can see the cause
    ///     (matches upstream's `request_invalid` shape for stream
    ///     failures).
    fn from_body_read_error(err: EdgeError, cap_bytes: usize) -> Self {
        if err.status() == StatusCode::BAD_REQUEST {
            Self::BadRequest(format!(
                "Invalid request: request size exceeded max size of {cap_bytes} bytes."
            ))
        } else {
            Self::Internal(format!("Invalid request: body read failed: {err}"))
        }
    }
    fn from_edge_error(err: EdgeError) -> Self {
        if err.status() == StatusCode::BAD_REQUEST {
            Self::BadRequest(err.to_string())
        } else {
            Self::Internal(err.to_string())
        }
    }
    /// Returns `(status, content_type, body)`. Bodies always carry
    /// a trailing `\n` per Appendix F text-error format.
    fn into_parts(self) -> (StatusCode, &'static str, Vec<u8>) {
        match self {
            Self::BadRequest(m) => {
                let mut body = m.into_bytes();
                body.push(b'\n');
                (StatusCode::BAD_REQUEST, "text/plain; charset=utf-8", body)
            }
            Self::FanoutUnavailable => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "text/plain; charset=utf-8",
                b"outbound http unavailable\n".to_vec(),
            ),
            Self::Internal(m) => {
                let mut body = m.into_bytes();
                body.push(b'\n');
                (StatusCode::INTERNAL_SERVER_ERROR, "text/plain; charset=utf-8", body)
            }
        }
    }
}

/// Registry + host-config handles. These are populated by the
/// adapter crate at app-init time and read by the handler.
///
/// Round-16 review Blocker 4: the registry constructor lives in
/// `auctioneer-bidder` (Task 14), NOT in `auctioneer-core`. Earlier drafts
/// referenced a non-existent `crate::registry::build_registry`;
/// `auctioneer_bidder::build_registry()` is the actual exported
/// constructor.
///
/// Round-17 review Blocker 6: the visibility on `registry_handle` /
/// `host_config_handle` is widened to `pub(super)` so the
/// `info_bidders` action wrappers in the sibling
/// `handlers::info_bidders` module can read them. The `info_bidders`
/// wrapper additionally needs `registry_bidder_list()` and
/// `host_aliases()` adapters — exposing the `&'static BidderInfo`
/// slice + `&BTreeMap<String, String>` references — so the helper
/// functions can be passed by reference without each call site
/// re-deriving them.
pub(super) fn registry_handle() -> &'static BidderRegistry {
    static REGISTRY: std::sync::OnceLock<BidderRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(auctioneer_bidder::build_registry)
}

pub(super) fn host_config_handle() -> &'static HostConfig {
    static HOST: std::sync::OnceLock<HostConfig> = std::sync::OnceLock::new();
    HOST.get_or_init(|| auctioneer_config::host_default().clone())
}

/// Round-17 review Blocker 6: the `&'static [&'static BidderInfo]`
/// slice the `info_bidders` action wrappers pass to `list()` /
/// `detail()`. Caches both the leaked slice AND the registry-derived
/// pointer set on first call.
pub(super) fn registry_bidder_list() -> &'static [&'static auctioneer_config::BidderInfo] {
    static LIST: std::sync::OnceLock<Vec<&'static auctioneer_config::BidderInfo>> =
        std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        registry_handle()
            .base_names()
            .iter()
            .filter_map(|name| registry_handle().get(name.as_str()).map(|(info, _adapter)| info))
            .collect()
    }).as_slice()
}

/// Round-17 review Blocker 6: the host-config alias map (Task 18c
/// adds `HostConfig.aliases`). `/info/bidders` reads this via
/// `host_aliases()`.
pub(super) fn host_aliases() -> &'static std::collections::BTreeMap<String, String> {
    &host_config_handle().aliases
}

/// Round-18 review Blocker 2 + round-19 review Blocker 1 + 2:
/// host primitives come from the core-owned `OnceLock` registry
/// populated by the adapter at startup (NOT from per-request
/// extensions and NOT from a manifest-registered middleware — see
/// Task 37b for the registry impl).
///
/// Round-21 review Blocker 1: tests can override the global
/// registry by installing a typed `TestClockHandle` /
/// `TestRngHandle` extension on the `RequestContext` BEFORE
/// calling the action. The handler checks the extensions first
/// and falls back to the global registry. No unsafe, no global
/// mutation.
fn clock_handle(ctx: &RequestContext)
    -> Arc<dyn auctioneer_runtime::Clock + Send + Sync>
{
    use crate::host_primitives::registry::TestClockHandle;
    if let Some(handle) = ctx.request().extensions().get::<TestClockHandle>() {
        return handle.0.clone();
    }
    crate::host_primitives::registry::clock_handle()
}

fn rng_handle(ctx: &RequestContext)
    -> Arc<dyn auctioneer_runtime::Rng + Send + Sync>
{
    use crate::host_primitives::registry::TestRngHandle;
    if let Some(handle) = ctx.request().extensions().get::<TestRngHandle>() {
        return handle.0.clone();
    }
    crate::host_primitives::registry::rng_handle()
}

fn headers_from_http(h: &http::HeaderMap) -> RequestHeaders<'_> {
    RequestHeaders {
        referer:           h.get("referer").and_then(|v| v.to_str().ok()),
        user_agent:        h.get("user-agent").and_then(|v| v.to_str().ok()),
        dnt:               h.get("dnt").and_then(|v| v.to_str().ok()),
        sec_gpc:           h.get("sec-gpc").and_then(|v| v.to_str().ok()),
        true_client_ip:    h.get("true-client-ip").and_then(|v| v.to_str().ok()),
        x_forwarded_for:   h.get("x-forwarded-for").and_then(|v| v.to_str().ok()),
        x_real_ip:         h.get("x-real-ip").and_then(|v| v.to_str().ok()),
        remote_addr:       None,
        content_encoding:  h.get("content-encoding").and_then(|v| v.to_str().ok()),
    }
}
```

- [ ] **Step 3a: Implement `crates/auctioneer-core/src/fanout_seam.rs`** — round-15 review Blocker E2E. The seam abstracts where the handler's `BidderFanout` comes from so production code uses `ctx.proxy_handle()` (the EdgeZero `ProxyClient` exposed on every action's `RequestContext`) and tests inject a canned fanout via `RequestContext::extensions_mut().insert::<TestFanoutHandle>(...)`. The earlier `StubProxyClient`-that-always-errors is REMOVED; a request without a usable fanout returns `FanoutUnavailable` rather than silently producing fictitious bids.

Round-16 review Blocker 3: the generic arity has to match
`DevFanout<'h, C: Clock, P: ProxyClientApi>` from Task 37 — both
parameters, both bound. The adapter is held by ownership inside
the seam (not borrowed); the seam owns it for the duration of the
auction.

```rust
//! Per-request BidderFanout source. See Blocker E2E note above.

use crate::edgezero_proxy_adapter::EdgeZeroProxyAdapter;
use crate::host_primitives::DevFanout;
use edgezero_core::context::RequestContext;
use auctioneer_exchange::{
    AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
};
use auctioneer_runtime::Clock;

/// Round-19 review Blocker 3: the clock is a `&dyn Clock` trait
/// object so the handler can pass any adapter's clock impl
/// (CloudflareClock on Workers, SystemClock on dev). Earlier drafts
/// stored `&SystemClock` directly, which both prevented compile-on-
/// non-dev (`clock.as_ref()` is `&dyn Clock`, not `&SystemClock`) AND
/// defeated the Cloudflare clock abstraction.
///
/// Production seam OWNS the EdgeZero proxy adapter so its lifetime
/// is bounded by the auction; the dev fanout borrows the adapter +
/// clock via references on the same `'a` lifetime.
pub enum FanoutSeam<'a> {
    Production {
        adapter: EdgeZeroProxyAdapter,
        clock: &'a (dyn Clock + Send + Sync),
    },
    Test(std::sync::Arc<dyn BidderFanout + Send + Sync>),
}

impl<'a> FanoutSeam<'a> {
    // Round-25 review Blocker 1: ctx and clock MUST have INDEPENDENT
    // lifetimes. The previous signature tied `'a` to BOTH, so even
    // though the body only borrows clock and clones the
    // TestFanoutHandle / ProxyHandle (both owned-out), Rust's
    // inference treated ctx as borrowed for the whole seam's
    // lifetime. That made `let fanout = FanoutSeam::from_context(&ctx,
    // clock)?; ctx.into_request();` a borrowck error. Decouple the
    // ctx borrow into `'ctx` (used only for the call) and keep `'a`
    // for the long-lived clock reference held by Production. Returns
    // `FanoutSeam<'a>` carrying ONLY the clock lifetime.
    pub fn from_context<'ctx>(
        ctx: &'ctx RequestContext,
        clock: &'a (dyn Clock + Send + Sync),
    ) -> Option<Self> {
        // 1. Test seam: any test that wants to swap the fanout
        //    installs a TestFanoutHandle into the RequestContext's
        //    extensions (see Mocktioneer E2E for the call site).
        //    `handle.0.clone()` gives an owned Arc — the returned
        //    FanoutSeam::Test variant never borrows from ctx.
        if let Some(handle) = ctx.request().extensions().get::<TestFanoutHandle>() {
            return Some(FanoutSeam::Test(handle.0.clone()));
        }
        // 2. Production seam: `ctx.proxy_handle()` returns
        //    `Option<ProxyHandle>` — owned, cloned from extensions
        //    (see EdgeZero core/context.rs line 97). EdgeZeroProxyAdapter
        //    takes ownership of the handle, so neither field of
        //    FanoutSeam::Production borrows from ctx.
        let proxy = ctx.proxy_handle()?;
        Some(FanoutSeam::Production {
            adapter: EdgeZeroProxyAdapter::new(proxy),
            clock,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl<'a> BidderFanout for FanoutSeam<'a> {
    async fn send_all(&self, calls: Vec<BidderCall>, deadline: AuctionDeadline)
        -> Result<Vec<BidderCallResult>, FanoutInternalError>
    {
        match self {
            FanoutSeam::Production { adapter, clock } => {
                // Round-19 review Blocker 3: DevFanout's clock generic
                // is bound to `Clock`, so `&dyn Clock` satisfies the
                // bound at call sites — DevFanout<'_, dyn Clock, _>
                // composes naturally. The fanout borrows the adapter
                // + the trait-object clock for the auction's lifetime.
                let fanout: DevFanout<'_, dyn Clock + Send + Sync, EdgeZeroProxyAdapter> =
                    DevFanout { client: adapter, clock: *clock };
                fanout.send_all(calls, deadline).await
            }
            FanoutSeam::Test(handle) => handle.send_all(calls, deadline).await,
        }
    }
}

/// Test-side handle a test installs into the RequestContext to swap
/// the fanout. The Mocktioneer handler-path E2E and any future
/// handler tests use this seam to inject a CannedFanout that produces
/// the same byte output as the direct-run_auction path.
pub struct TestFanoutHandle(pub std::sync::Arc<dyn BidderFanout + Send + Sync>);

// Round-25 review Blocker 2: prove `FanoutSeam<'static>: Send + Sync`
// at compile time so `impl BidderFanout for FanoutSeam<'_>` (which
// requires Send + Sync via the trait's supertrait at L3688) actually
// holds. If a future EdgeZeroProxyAdapter regression loses Send/Sync,
// the error appears here rather than as an opaque trait-impl miss at
// the handler call site. `'static` works as the lifetime witness
// because every variant is Send/Sync regardless of the borrowed
// clock's lifetime.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<FanoutSeam<'static>>();
};
```

- [ ] **Step 3b: Implement `crates/auctioneer-core/src/edgezero_proxy_adapter.rs`** — round-16 review Blocker 3: the `ProxyClientApi` shim that maps `BidderHttpRequest` → `ProxyRequest`, calls `ProxyHandle::forward()`, and lowers the resulting `edgezero_core::http::Response` back into `BidderHttpResponse`. This is the concrete code earlier drafts only described.

```rust
//! Adapter from EdgeZero `ProxyHandle` to the dev `BidderFanout`'s
//! `ProxyClientApi` contract. See Task 37 for the trait shape.

use crate::host_primitives::fanout::ProxyClientApi;
use async_trait::async_trait;
use edgezero_core::body::Body;
use edgezero_core::http::{HeaderName, HeaderValue, Method, Uri};
use edgezero_core::proxy::{ProxyHandle, ProxyRequest};
use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse};

pub struct EdgeZeroProxyAdapter {
    proxy: ProxyHandle,
}

impl EdgeZeroProxyAdapter {
    pub fn new(proxy: ProxyHandle) -> Self {
        Self { proxy }
    }
}

#[async_trait(?Send)]
impl ProxyClientApi for EdgeZeroProxyAdapter {
    async fn send_one(&self, req: &BidderHttpRequest)
        -> Result<BidderHttpResponse, String>
    {
        // Round-17 review Blocker 2: ProxyRequest has no `from_parts`
        // constructor. The pinned API offers `ProxyRequest::new(method,
        // uri)` plus the `headers_mut` / `body_mut` setters; use that
        // form here.
        let method: Method = req.method.parse()
            .map_err(|e| format!("invalid method {:?}: {e}", req.method))?;
        let uri: Uri = req.url.parse()
            .map_err(|e| format!("invalid url {:?}: {e}", req.url))?;
        let mut proxy_request = ProxyRequest::new(method, uri);
        // Body: `Body::from_bytes` accepts an owned Vec<u8>.
        *proxy_request.body_mut() = Body::from_bytes(req.body.clone());
        // Headers: insert via the typed HeaderMap exposed by
        // `headers_mut`. Skip headers that fail to parse rather than
        // failing the whole call — an invalid header would already have
        // been rejected upstream.
        let headers = proxy_request.headers_mut();
        for (name, value) in &req.headers {
            let header_name: HeaderName = match name.parse() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let header_value = match HeaderValue::from_str(value) {
                Ok(v) => v,
                Err(_) => continue,
            };
            headers.insert(header_name, header_value);
        }

        // 2. Forward via the EdgeZero ProxyHandle.
        let response = self.proxy.forward(proxy_request).await
            .map_err(|e| format!("forward failed: {e}"))?;

        // 3. Lower the response back into BidderHttpResponse.
        let status = response.status().as_u16();
        let mut headers: Vec<(String, String)> = Vec::new();
        for (k, v) in response.headers() {
            if let Ok(v_str) = v.to_str() {
                headers.push((k.as_str().to_owned(), v_str.to_owned()));
            }
        }
        let (_parts, body) = response.into_parts();
        let body_bytes = body.into_bytes_bounded(usize::MAX).await
            .map_err(|e| format!("read response body: {e}"))?;
        Ok(BidderHttpResponse { status, headers, body: body_bytes.to_vec() })
    }
}

// Round-25 review Blocker 2: compile-time assertion that
// `EdgeZeroProxyAdapter: Send + Sync`. If a future EdgeZero refactor
// loosens `ProxyClient: Send + Sync` to a non-Send bound, this fails
// to compile here — surfacing the regression at the adapter boundary
// rather than at a downstream `BidderFanout for DevFanout` call site
// where the error message would be opaque. The same pattern guards
// `FanoutSeam<'_>: Send + Sync` in the fanout module.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EdgeZeroProxyAdapter>();
};
```

- [ ] **Step 4: Add `async-trait` to dev-deps if not already**

(See Task 37.)

- [ ] **Step 5: Verify**

```
cargo check -p auctioneer-core
```

Expected: success.

- [ ] **Step 6: Commit**

```
git add crates/auctioneer-core
git commit -m "auctioneer-core: auction handler (thin shim over run_auction)"
```

---

### Task 39: `status` + `info_bidders` + `info_bidders_detail` handlers

**Files:**

- Create: `crates/auctioneer-core/src/handlers/status.rs`
- Create: `crates/auctioneer-core/src/handlers/info_bidders.rs`
- Modify: `crates/auctioneer-core/src/handlers/mod.rs` — round-26 review Blocker 1. Drop the three pass-through stubs (`status`, `info_bidders`, `info_bidder`) Task 38 left behind, replace with `mod status; mod info_bidders;` + the matching `pub use`s.
- (Round-26 review Blocker 1: the `edgezero_core::app!(...)` macro line is NOT touched here. Task 35 already put it in `lib.rs`; Task 36 and later only appended `pub mod` declarations. Earlier draft deferred the macro to this task — that's no longer needed because the stub-handler chain keeps the macro resolvable from Task 35 onward.)

**Interfaces:**

- Consumes: `HostConfig`, `BidderRegistry` (per `auctioneer-config`).
- Produces:
  - `status`: 204 with empty body when `host.status_response.is_empty()`, else 200 with the exact bytes (no `Content-Type` per Appendix F / round-67 GDPR enforcement).
  - `info_bidders`: 200 JSON array of bidder names + `enabledonly` / `baseadaptersonly` query handling; 400 with the upstream-exact body on bad query.
  - `info_bidders_detail`: 200 JSON object per §8.1 shape; 404 empty body on unknown name; special name `all` returns the aggregate.

- [ ] **Step 1: Implement `handlers/status.rs`**

```rust
//! `GET /status` — Appendix F + spec §8.1.

use edgezero_core::{
    action,
    body::Body,
    context::RequestContext,
    error::EdgeError,
    http::{response_builder, Response},
};
use http::StatusCode;
use auctioneer_config::HostConfig;

/// Round-17 review Blocker 5: the manifest registers
/// `auctioneer_core::handlers::status`, which MUST resolve to a real
/// `#[action]` function (not the pure-helper `status_inner` below).
/// Earlier drafts left only the helper, so the route would never
/// bind.
#[action]
pub async fn status(_ctx: RequestContext) -> Result<Response, EdgeError> {
    let host = crate::handlers::auction::host_config_handle();
    let (status_code, body) = status_inner(host);
    response_builder()
        .status(status_code)
        .body(Body::from_bytes(body))
        .map_err(EdgeError::internal)
}

/// Testable core. Per Appendix F + round-67 GDPR enforcement, `/status` returns
/// 204 No Content with an empty body when `host.status_response` is
/// empty, else 200 with the exact configured bytes (no Content-Type
/// per spec line 2975).
fn status_inner(host: &HostConfig) -> (StatusCode, Vec<u8>) {
    if host.status_response.is_empty() {
        (StatusCode::NO_CONTENT, Vec::new())
    } else {
        (StatusCode::OK, host.status_response.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_is_204_empty() {
        let host = HostConfig::default();
        assert_eq!(status_inner(&host), (StatusCode::NO_CONTENT, Vec::new()));
    }
    #[test]
    fn configured_is_200_exact_bytes() {
        let host = HostConfig { status_response: "OK".into(), ..HostConfig::default() };
        assert_eq!(status_inner(&host), (StatusCode::OK, b"OK".to_vec()));
    }
}
```

- [ ] **Step 2: Implement `handlers/info_bidders.rs`**

```rust
//! `GET /info/bidders` + `GET /info/bidders/{name}` — Appendix F + spec §8.1.

use auctioneer_config::BidderInfo;
use std::collections::BTreeMap;

const BAD_QUERY_BODY_ENABLEDONLY: &[u8] = b"Invalid value for 'enabledonly' query param, must be of boolean type";
const BAD_QUERY_BODY_BASEONLY:    &[u8] = b"Invalid value for 'baseadaptersonly' query param, must be of boolean type";

// Round-13 review Medium 9 — caller wiring: the `#[action]
// pub async fn info_bidders` handler reads
// `HostConfig.aliases` (the host-config alias map per spec §9.1)
// and passes it as the `aliases` argument. Phase 1's
// host-defaults.yaml ships an empty aliases map; a host operator
// declaring an alias gets it reflected in `/info/bidders` without
// any code change here.
#[must_use]
pub fn list(
    bidders: &[&'static BidderInfo],
    aliases: &BTreeMap<String, String>,
    enabledonly: Option<&str>,
    baseadaptersonly: Option<&str>,
) -> (u16, Option<&'static str>, Vec<u8>) {
    let enabled = match parse_bool(enabledonly) {
        Some(v) => v,
        None    => return (400, None, BAD_QUERY_BODY_ENABLEDONLY.to_vec()),
    };
    let base_only = match parse_bool(baseadaptersonly) {
        Some(v) => v,
        None    => return (400, None, BAD_QUERY_BODY_BASEONLY.to_vec()),
    };
    // Round-13 review Medium 9: per spec §8.1 lines 2979-2982, the
    // response includes configured aliases — and baseadaptersonly=true
    // EXCLUDES them. The previous filter (`!base_only || true`) was a
    // no-op that always passed every base bidder through unchanged AND
    // never emitted aliases at all. Build the union explicitly:
    let mut names: Vec<&str> = bidders.iter()
        .filter(|b| !enabled || !b.disabled)
        .map(|b| b.name.as_str())
        .collect();
    if !base_only {
        // Aliases are name → core-bidder pairs the host config declares
        // (spec §9.1). When the core bidder is disabled and
        // enabledonly is on, the alias is also dropped.
        for (alias_name, core) in aliases {
            let core_disabled = bidders.iter()
                .find(|b| b.name == *core)
                .is_some_and(|b| b.disabled);
            if enabled && core_disabled { continue; }
            names.push(alias_name.as_str());
        }
    }
    names.sort();
    names.dedup();
    // Round-23 review High 4: serializing a `Vec<&str>` is
    // structurally infallible (serde_json only fails on map keys
    // that aren't strings, missing fields, etc.); use `expect` with
    // a per-site `#[expect(clippy::expect_used, reason = ...)]`
    // annotation rather than silently masking the failure with an
    // empty body.
    #[expect(clippy::expect_used, reason = "serializing Vec<&str> is structurally infallible")]
    let body = serde_json::to_vec(&names).expect("serialize Vec<&str>");
    (200, Some("application/json"), body)
}

fn parse_bool(v: Option<&str>) -> Option<bool> {
    match v {
        None | Some("") => Some(false),  // missing = filter off (matches prebid default).
        Some("true")    => Some(true),
        Some("false")   => Some(false),
        _               => None,
    }
}

#[must_use]
pub fn detail(
    bidders: &[&'static BidderInfo],
    aliases: &BTreeMap<String, String>,
    name: &str,
) -> (u16, Option<&'static str>, Vec<u8>) {
    if name == "all" {
        let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
        for b in bidders {
            map.insert(b.name.as_str(), detail_value(b, None));
        }
        // Round-16 review Blocker 5: aliases appear in the aggregate
        // too, with `aliasOf` pointing at the core bidder per spec §8.1
        // line 2979–2982. A configured alias whose core is unknown
        // would only show up via list(); detail("all") omits it
        // because we cannot synthesize the wire shape without the
        // core BidderInfo.
        for (alias_name, core) in aliases {
            if let Some(core_info) = bidders.iter().find(|b| b.name == *core) {
                map.insert(alias_name.as_str(), detail_value(core_info, Some(core.as_str())));
            }
        }
        // Round-24 review High 3: every `serde_json::to_vec` in this
        // file targets a value that is structurally infallible to
        // serialize — `BTreeMap<&str, Value>` and `serde_json::Value`
        // are JSON-native. `unwrap_or_default()` would silently emit
        // `[]` on a (impossible) serialize failure, which would race
        // a "looks like the body was empty" diagnosis past the
        // implementer. Annotate `.expect(...)` so the clippy
        // restriction gate accepts it AND a real failure panics
        // loudly with the call site.
        #[expect(
            clippy::expect_used,
            reason = "serializing BTreeMap<&str, serde_json::Value> is structurally infallible",
        )]
        let body = serde_json::to_vec(&map).expect("serialize info detail-all map");
        return (200, Some("application/json"), body);
    }
    // Round-16 review Blocker 5: aliases must resolve to their core
    // bidder's detail PLUS an `aliasOf` field. Spec §8.1 line 2979–2982
    // requires aliases in detail(); the earlier "no alias map" version
    // returned 404 for any configured alias, which violated the spec.
    if let Some(core) = aliases.get(name) {
        if let Some(core_info) = bidders.iter().find(|b| b.name == *core) {
            #[expect(
                clippy::expect_used,
                reason = "serializing serde_json::Value is structurally infallible",
            )]
            let body = serde_json::to_vec(&detail_value(core_info, Some(core.as_str())))
                .expect("serialize info detail (alias)");
            return (200, Some("application/json"), body);
        }
    }
    match bidders.iter().find(|b| b.name.eq_ignore_ascii_case(name)) {
        Some(b) => {
            #[expect(
                clippy::expect_used,
                reason = "serializing serde_json::Value is structurally infallible",
            )]
            let body = serde_json::to_vec(&detail_value(b, None))
                .expect("serialize info detail (single)");
            (200, Some("application/json"), body)
        }
        None => (404, None, Vec::new()),
    }
}

fn detail_value(b: &BidderInfo, alias_of: Option<&str>) -> serde_json::Value {
    // Round-13 review Medium 10: `usesHttps` comes from the codegen-
    // emitted `BidderInfo.uses_https` field — NOT from a substring
    // check on `endpoint.starts_with("https://")`. The two should
    // always agree (codegen derives uses_https from the endpoint
    // string at build time), but reading the field rather than
    // recomputing keeps the wire response in sync with whatever
    // BidderInfo says is true, and lets a future codegen refactor
    // change the source-of-truth without revisiting this handler.
    //
    // Round-16 review Blocker 5: when `alias_of` is Some, emit the
    // `aliasOf` field per spec §8.1.
    let mut value = serde_json::json!({
        "status": if b.disabled { "DISABLED" } else { "ACTIVE" },
        "usesHttps": b.uses_https,
        "maintainer": { "email": b.maintainer_email.clone().unwrap_or_default() },
        "capabilities": {
            "app":  { "mediaTypes": b.capabilities.app  .iter().map(media_type_str).collect::<Vec<_>>() },
            "site": { "mediaTypes": b.capabilities.site .iter().map(media_type_str).collect::<Vec<_>>() },
        },
    });
    if let Some(core_name) = alias_of {
        value["aliasOf"] = serde_json::json!(core_name);
    }
    value
}

fn media_type_str(m: &auctioneer_config::MediaType) -> &'static str {
    match m {
        auctioneer_config::MediaType::Banner => "banner",
        auctioneer_config::MediaType::Video  => "video",
        auctioneer_config::MediaType::Native => "native",
        auctioneer_config::MediaType::Audio  => "audio",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn leak(name: &'static str, disabled: bool) -> &'static BidderInfo {
        Box::leak(Box::new(BidderInfo {
            name: name.into(), endpoint: "https://x".into(), uses_https: true, disabled,
            ..BidderInfo::default()
        }))
    }
    fn empty_aliases() -> BTreeMap<String, String> { BTreeMap::new() }
    #[test]
    fn list_filters_disabled_when_enabledonly_true() {
        let bids = vec![leak("a", false), leak("b", true)];
        let (status, _, body) = list(&bids, &empty_aliases(), Some("true"), Some("false"));
        assert_eq!(status, 200);
        let names: Vec<String> = serde_json::from_slice(&body).unwrap();
        assert_eq!(names, vec!["a"]);
    }
    #[test]
    fn list_400_on_bad_query() {
        let bids = vec![leak("a", false)];
        let (status, _, body) = list(&bids, &empty_aliases(), Some("yep"), None);
        assert_eq!(status, 400);
        assert_eq!(body, BAD_QUERY_BODY_ENABLEDONLY);
    }
    #[test]
    fn detail_unknown_returns_404_empty() {
        let bids = vec![leak("a", false)];
        let (status, _, body) = detail(&bids, &empty_aliases(), "missing");
        assert_eq!(status, 404);
        assert!(body.is_empty());
    }
    #[test]
    fn detail_all_returns_aggregate() {
        let bids = vec![leak("a", false), leak("b", true)];
        let (status, _, body) = detail(&bids, &empty_aliases(), "all");
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v.get("a").is_some() && v.get("b").is_some());
    }
    #[test]
    fn detail_alias_returns_core_shape_plus_alias_of() {
        // Round-16 review Blocker 5: an alias resolves to its core
        // bidder's detail PLUS an `aliasOf` field per spec §8.1.
        let bids = vec![leak("appnexus", false)];
        let mut aliases = BTreeMap::new();
        aliases.insert("myalias".to_owned(), "appnexus".to_owned());
        let (status, _, body) = detail(&bids, &aliases, "myalias");
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v.get("aliasOf").and_then(|x| x.as_str()), Some("appnexus"));
    }
}
```

- [ ] **Step 2b: Wrap the helpers in real `#[action]` functions** — round-16 review Blocker 5. Task 39's prior version stopped at the helper functions; the manifest registers callable items, so the actual action wrappers belong in this file too.

```rust
//! `#[action]` wrappers for the /info/bidders surface. The manifest
//! at edgezero.toml references these by qualified path; the helpers
//! above are the testable cores.

use edgezero_core::{
    action,
    body::Body,
    context::RequestContext,
    error::EdgeError,
    http::{response_builder, Response, CONTENT_TYPE},
};
use http::{HeaderValue, StatusCode};

#[derive(serde::Deserialize, Default)]
struct InfoBiddersQuery {
    enabledonly:       Option<String>,
    baseadaptersonly:  Option<String>,
}

fn build_info_response(
    status_u16: u16,
    content_type: Option<&'static str>,
    body: Vec<u8>,
) -> Result<Response, EdgeError> {
    let status = StatusCode::from_u16(status_u16)
        .map_err(|_| EdgeError::internal("invalid status code"))?;
    let mut builder = response_builder().status(status);
    // Round-17 review High 7: `/info/bidders` 400 bad-query and 404
    // unknown-detail paths MUST omit Content-Type per spec line 5362.
    // The previous draft inserted `text/plain; charset=utf-8` in the
    // None branch, which contradicted both the comment AND the spec.
    // Only the `Some(_)` branch installs a Content-Type header now.
    if let Some(ct) = content_type {
        builder = builder.header(CONTENT_TYPE, HeaderValue::from_static(ct));
    }
    builder.body(Body::from_bytes(body)).map_err(EdgeError::internal)
}

#[action]
pub async fn info_bidders(ctx: RequestContext) -> Result<Response, EdgeError> {
    let bidders = crate::handlers::auction::registry_bidder_list();
    let aliases = crate::handlers::auction::host_aliases();
    // Round-27 review High: the spec (line 5362) requires invalid
    // `enabledonly` / `baseadaptersonly` to return 400 with NO
    // Content-Type and the EXACT upstream reference bidder-list handling body bytes
    // ("Invalid value for 'enabledonly' query param, must be of
    // boolean type" — NO trailing newline). The round-26 draft
    // added an `EdgeError::bad_request(...)` pre-check, which
    // bypassed the exact upstream bytes already encoded in `list()`'s
    // 400 branch (it constructs the spec-exact body via
    // `BAD_QUERY_BODY_ENABLEDONLY` / `BAD_QUERY_BODY_BASEONLY`).
    // The correct contract: let `list()` produce the response
    // (status + None content-type + spec body), then funnel through
    // `build_info_response` which knows how to emit the body
    // verbatim with no Content-Type.
    //
    // `ctx.query()` still uses `?` because a wholly-unparseable
    // query string (e.g. invalid UTF-8 percent-encoding) is a
    // distinct failure mode the spec does not enumerate; the
    // EdgeZero default of `EdgeError::bad_request("invalid query
    // string: <err>")` is the right surface for that case. The
    // `enabledonly=banana` case parses cleanly to
    // `InfoBiddersQuery { enabledonly: Some("banana"), .. }` and
    // is dispatched to `list()`, which returns the spec-exact 400.
    let query: InfoBiddersQuery = ctx.query()?;
    let (status, ct, body) = list(
        &bidders,
        &aliases,
        query.enabledonly.as_deref(),
        query.baseadaptersonly.as_deref(),
    );
    build_info_response(status, ct, body)
}

#[action]
pub async fn info_bidder(ctx: RequestContext) -> Result<Response, EdgeError> {
    #[derive(serde::Deserialize)]
    struct Path { name: String }
    let path: Path = ctx.path()?;
    let bidders = crate::handlers::auction::registry_bidder_list();
    let aliases = crate::handlers::auction::host_aliases();
    let (status, ct, body) = detail(&bidders, &aliases, &path.name);
    build_info_response(status, ct, body)
}
```

- [ ] **Step 2b: Replace `handlers/mod.rs`** — round-26 review Blocker 1. Drop the Task-38 pass-through stubs, use `mod` declarations + `pub use`:

```rust
//! Per-route handlers — thin shims over `auctioneer_exchange::run_auction`.

mod auction;
mod info_bidders;
mod status;

pub use auction::auction;
pub use info_bidders::{info_bidder, info_bidders};
pub use status::status;
```

- [ ] **Step 3: Test**

```
cargo test -p auctioneer-core --lib handlers
```

Expected: 6 passed.

- [ ] **Step 4: MANDATORY post-Task-39 gate** — round-23 review Medium-high 5 (revised round-26 review Blocker 1). At this point every real `#[action]` body is in place (Task 38 swapped the `auction` stub for the real impl; this task swapped the remaining three stubs for the real impls). `cargo check -p auctioneer-core` MUST still pass. The `edgezero_core::app!(...)` macro has been resolving since Task 35 (against the stub bodies) — the round-26 stub-handlers fix means it never went red between Task 35 and now. This step is the FINAL-FORM gate: if `cargo check` fails here, one of the new sub-module impls broke a function signature or import that the round-26 stubs carried correctly. Fix the underlying bug rather than committing.

```
cargo check -p auctioneer-core
cargo check --workspace
```

Expected: both green. If `cargo check -p auctioneer-core` fails with "unresolved name" pointing at `crate::handlers::...`, that means either the handler isn't `pub use`-exported in `handlers/mod.rs` (round-16 Blocker 2) OR the manifest path doesn't match the function name. Fix the underlying issue; do NOT silence the failure or commit until both checks are green.

- [ ] **Step 5: Commit**

```
git add crates/auctioneer-core
git commit -m "auctioneer-core: status + info_bidders handlers"
```

---

### Task 40: Forward `insecure-bidders` through the four adapter crates

**Files:**

- Modify: `crates/auctioneer-adapter-axum/Cargo.toml`
- Modify: `crates/auctioneer-adapter-cloudflare/Cargo.toml`
- Modify: `crates/auctioneer-adapter-fastly/Cargo.toml`
- Modify: `crates/auctioneer-adapter-spin/Cargo.toml`

**Interfaces:**

- Consumes: `auctioneer-core/insecure-bidders`.
- Produces: each adapter `Cargo.toml` declares `insecure-bidders = ["auctioneer-core/insecure-bidders"]`.

- [ ] **Step 1: Edit each adapter `Cargo.toml`**

In the `[features]` block of each of the four adapter crates, add:

```toml
[features]
# existing features kept (cloudflare / fastly / spin per current toml)
insecure-bidders = ["auctioneer-core/insecure-bidders"]
```

For `auctioneer-adapter-axum`, which has no `[features]` block yet, create one with only the line above.

- [ ] **Step 2: Verify**

```
cargo build --release -p auctioneer-adapter-axum
cargo build --release -p auctioneer-adapter-cloudflare --target wasm32-unknown-unknown
cargo build --release -p auctioneer-adapter-fastly      --target wasm32-wasip1
cargo build --release -p auctioneer-adapter-spin        --target wasm32-wasip1
cargo build -p auctioneer-adapter-axum --features insecure-bidders
```

Expected: all five succeed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-adapter-*/Cargo.toml
git commit -m "adapters: forward insecure-bidders feature to auctioneer-core"
```

---

### Task 41: Dependency-purity test

**Files:**

- Create: `crates/auctioneer-core/tests/dependency_purity.rs`

**Interfaces:**

- Consumes: `cargo metadata`.
- Produces: integration test that fails if any of `auctioneer-openrtb`, `auctioneer-config`, `auctioneer-runtime`, `auctioneer-privacy`, `auctioneer-bidder`, `auctioneer-exchange` transitively depends on `edgezero-*`, `tokio`, `reqwest`, `fastly`, `worker`, or `spin-sdk`.

- [ ] **Step 1: Implement**

```rust
//! Spec §11.2 — library crates must NEVER transitively depend on EdgeZero
//! or runtime crates. Only auctioneer-core and the four adapter crates may.

use std::process::Command;

const LIB_CRATES: &[&str] = &[
    "auctioneer-openrtb", "auctioneer-config", "auctioneer-runtime",
    "auctioneer-privacy", "auctioneer-bidder", "auctioneer-exchange",
];
const FORBIDDEN: &[&str] = &[
    "edgezero-core", "edgezero-adapter-axum", "edgezero-adapter-cloudflare",
    "edgezero-adapter-fastly", "edgezero-adapter-spin", "edgezero-cli",
    "tokio", "reqwest", "fastly", "worker", "spin-sdk",
];

#[test]
fn library_crates_have_no_forbidden_deps() {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("cargo metadata");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let full = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1"])
        .output()
        .expect("cargo metadata");
    let json: serde_json::Value = serde_json::from_slice(&full.stdout).unwrap();
    let packages = json["packages"].as_array().unwrap();
    let resolve = &json["resolve"];
    let nodes = resolve["nodes"].as_array().unwrap();

    let mut violations: Vec<String> = Vec::new();
    for &lib in LIB_CRATES {
        let lib_id = packages.iter()
            .find(|p| p["name"] == lib)
            .map(|p| p["id"].as_str().unwrap().to_owned());
        let Some(lib_id) = lib_id else { continue; };

        let reachable = walk(&lib_id, nodes);
        for dep_id in &reachable {
            let name = packages.iter()
                .find(|p| p["id"] == serde_json::Value::String(dep_id.clone()))
                .and_then(|p| p["name"].as_str()).unwrap_or("");
            if FORBIDDEN.contains(&name) {
                violations.push(format!("library crate `{lib}` transitively depends on `{name}`"));
            }
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

fn walk(start: &str, nodes: &[serde_json::Value]) -> std::collections::BTreeSet<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut stack = vec![start.to_owned()];
    while let Some(id) = stack.pop() {
        if !seen.insert(id.clone()) { continue; }
        if let Some(node) = nodes.iter().find(|n| n["id"] == serde_json::Value::String(id.clone())) {
            if let Some(deps) = node["dependencies"].as_array() {
                for d in deps {
                    stack.push(d.as_str().unwrap().to_owned());
                }
            }
        }
    }
    seen
}
```

- [ ] **Step 2: Test**

```
cargo test -p auctioneer-core --test dependency_purity
```

Expected: 1 passed.

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-core/tests/dependency_purity.rs
git commit -m "auctioneer-core: dependency-purity test (§11.2)"
```

---

### Task 42: `insecure-bidders` snapshot triple-assertion test

**Files:**

- Create: `crates/auctioneer-bidder/tests/insecure_bidders_snapshot.rs`

**Interfaces:**

- Consumes: `cargo expand` output (run via `Command`).
- Produces: spec §11.2 triple-assertion (default build has no insecure literals, feature-on does, default test suite still passes). The simplest form of this test (kept auction core-feasible) is a literal-scan over the crate source under each feature combination via reading file metadata; the `cargo expand` form is documented as a follow-on harness.

- [ ] **Step 1: Implement the three assertions with REAL evidence, not empty-test sentinels.** Round-14 review Medium: the prior version's default-build test was empty and would pass even if `MocktioneerBidder` were accidentally re-exported (the empty `fn` body proves nothing). Replace it with two parallel approaches that together give the spec's release-artifact intent:

  1. **Negative compile assertion via `compile_fail` doctest** — `MocktioneerBidder` MUST be unreferenceable in the default build. A `compile_fail` doctest that _would_ compile if the symbol were public guarantees the build fails at compile time when the gate is bypassed.
  2. **Release-artifact symbol scan** — a default-build release `cargo build -p auctioneer-bidder --release` followed by a string scan of the produced `rlib` for `MocktioneerBidder` proves the linker dropped the gated symbol. Match the spec's release-artifact intent: the FEATURE-OFF binary contains no insecure-path code.

````rust
//! Spec §11.2 — `insecure-bidders` feature-gate triple assertion.
//!
//! The gate is enforced via two independent mechanisms:
//!   1. Source-level: a `compile_fail` doctest proves the symbol is
//!      unreferenceable under default features. If the gate ever
//!      slips, the test stops failing to compile and the suite
//!      breaks.
//!   2. Artifact-level: under default features, a release build of
//!      auctioneer-bidder is scanned for any occurrence of the
//!      `MocktioneerBidder` symbol string. The linker drops gated
//!      code; the test fails if it doesn't.

/// Compile-fail doctest proving the symbol is unreferenceable under
/// default features (the test compiles only when the symbol is
/// unreferenceable, so a future accidental un-gating breaks the
/// doctest harness immediately).
///
/// ```compile_fail
/// // This must fail to compile under default features. If you see a
/// // surprising green here, MocktioneerBidder has leaked out of the
/// // `#[cfg(feature = "insecure-bidders")]` gate.
/// use auctioneer_bidder::MocktioneerBidder;
/// fn _force_use() { let _ = MocktioneerBidder; }
/// ```
#[cfg(not(feature = "insecure-bidders"))]
const _: () = ();

#[test]
#[cfg(not(feature = "insecure-bidders"))]
fn release_artifact_drops_mocktioneer_symbol() {
    // Build a release-profile rlib of auctioneer-bidder under default
    // features. We do this via a child cargo invocation (the same
    // pattern as auctioneer-config's build_failure_harness) so the
    // outer test run's debug build doesn't pollute the scan.
    use std::path::PathBuf;
    use std::process::Command;
    let target_dir: PathBuf = [env!("CARGO_MANIFEST_DIR"), "target/insecure_snapshot"]
        .iter().collect();
    let output = Command::new(env!("CARGO"))
        .args(["build", "--release", "-p", "auctioneer-bidder", "--target-dir"])
        .arg(&target_dir)
        .output()
        .expect("spawn cargo build");
    assert!(
        output.status.success(),
        "release build failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    let rlib = target_dir.join("release").join("libauctioneer_bidder.rlib");
    let bytes = std::fs::read(&rlib).expect("read rlib");
    let needle = b"MocktioneerBidder";
    assert!(
        bytes.windows(needle.len()).all(|w| w != needle),
        "release rlib at {rlib:?} unexpectedly contains the MocktioneerBidder \
         symbol — the #[cfg(feature = \"insecure-bidders\")] gate is leaking."
    );
}

#[test]
#[cfg(feature = "insecure-bidders")]
fn feature_on_exposes_mocktioneer() {
    use auctioneer_bidder::MocktioneerBidder;
    let _adapter = MocktioneerBidder;
}
````

- [ ] **Step 2: Run under both feature sets**

```
cargo test -p auctioneer-bidder --test insecure_bidders_snapshot
cargo test -p auctioneer-bidder --features insecure-bidders --test insecure_bidders_snapshot
```

Expected: 1 passed default (the release-artifact scan, which spawns a child cargo build, plus the `compile_fail` doctest via `cargo test --doc`), 1 passed feature-on (`feature_on_exposes_mocktioneer`).

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-bidder/tests/insecure_bidders_snapshot.rs
git commit -m "auctioneer-bidder: insecure-bidders feature-gate snapshot tests"
```

---

### Task 43: Mocktioneer E2E (both call sites: handler + direct `run_auction`)

**Files:**

- Create: `crates/auctioneer-core/tests/mocktioneer_e2e.rs`

**Interfaces:**

- Consumes: a real Mocktioneer instance OR (CI default) a canned `BidderFanout` whose responses match Mocktioneer's deterministic outputs.
- Produces: end-to-end test that drives both (a) the EdgeZero-shaped `auction` handler logic and (b) a direct `run_auction` invocation against the same `NormalizedBidRequest` and asserts byte-identical `BidResponse` outputs (proving §5.8 trusted-server reuse).

- [ ] **Step 1: Implement**

```rust
//! Spec §11.2 Mocktioneer E2E + §5.8 trusted-server reuse.
//!
//! Offline CI variant uses a canned BidderFanout matching Mocktioneer's
//! deterministic outputs. An optional `--ignored` variant hits a live
//! Mocktioneer at 127.0.0.1:8787/openrtb2/auction (only runs under `--features
//! insecure-bidders` + manual invocation).

#![cfg(feature = "insecure-bidders")]

use async_trait::async_trait;
use auctioneer_bidder::{BidderRegistry, MocktioneerBidder};
use auctioneer_config::{AccountConfig, BidderInfo, HostConfig, ResourceLimits};
use auctioneer_exchange::{
    AccountSource, AuctionContext, AuctionDeadline, BidderCall, BidderCallResult,
    BidderFanout, FanoutInternalError, ResolvedAccount,
    run_auction,
};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
use auctioneer_openrtb::{prepare, RequestHeaders};
use auctioneer_openrtb::types::bid_response::BidResponse;
use auctioneer_runtime::{FakeClock, SeededRng};
use std::time::Duration;

struct CannedFanout { body: Vec<u8> }
#[async_trait(?Send)]
impl BidderFanout for CannedFanout {
    async fn send_all(&self, calls: Vec<BidderCall>, _: AuctionDeadline)
        -> Result<Vec<BidderCallResult>, FanoutInternalError>
    {
        Ok(calls.iter().map(|_| BidderCallResult {
            result: Ok(auctioneer_bidder::BidderHttpResponse {
                status: 200, headers: vec![], body: self.body.clone(),
            }),
            elapsed: Duration::from_millis(10),
        }).collect())
    }
}

fn leak(name: &'static str, ep: &'static str) -> &'static BidderInfo {
    Box::leak(Box::new(BidderInfo {
        name: name.into(), endpoint: ep.into(), uses_https: false,
        ..BidderInfo::default()
    }))
}

fn host() -> HostConfig {
    HostConfig {
        resource_limits: ResourceLimits {
            default_tmax_ms: 750, max_tmax_ms: 1500,
            max_bidder_response_bytes: 262_144,
            max_bidders_per_auction: 50, max_outbound_requests: 100,
        },
        ..HostConfig::default()
    }
}

async fn run(req: BidRequest, body: Vec<u8>) -> BidResponse {
    let mut registry = BidderRegistry::new();
    registry.register(
        leak("mocktioneer", "http://127.0.0.1:8787/openrtb2/auction"),
        Box::new(MocktioneerBidder),
    );
    let normalized = prepare(req, &RequestHeaders::default()).unwrap();
    let host = host();
    let acct = AccountConfig::default();
    let resolved = ResolvedAccount {
        id: "unknown".into(), source: AccountSource::DefaultFallback, config: &acct,
    };
    let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
    let rng = SeededRng::new(42);
    let fanout = CannedFanout { body };
    let ctx = AuctionContext::with_request_start(
        clock.now_monotonic(), &host, &resolved, &registry, &fanout, &clock, &rng, false,
    );
    run_auction(&normalized, &ctx).await.unwrap()
}

// Round-14 review Blocker Imp.ext drift + Blocker tokio: typed
// ImpExt construction + futures::executor::block_on test runner.
fn mocktioneer_request() -> BidRequest {
    let mut bidders = serde_json::Map::new();
    bidders.insert("mocktioneer".into(), serde_json::json!({}));
    BidRequest {
        id: "auc-mock".into(),
        imp: vec![Imp {
            id: "imp-1".into(),
            ext: Some(ImpExt {
                bidder: Some(serde_json::Value::Object(bidders)),
                ..ImpExt::default()
            }),
            ..Imp::default()
        }],
        cur: vec!["USD".into()],
        ..BidRequest::default()
    }
}

#[test]
fn mocktioneer_canned_response_is_deterministic_via_direct_run_auction() {
    futures::executor::block_on(async {
        let body = br#"{"id":"r","seatbid":[{"seat":"x","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#;
        let a = run(mocktioneer_request(), body.to_vec()).await;
        let b = run(mocktioneer_request(), body.to_vec()).await;
        assert_eq!(serde_json::to_vec(&a).unwrap(), serde_json::to_vec(&b).unwrap());
    });
}

// Round-13 review High 10 / round-14 review High Mocktioneer E2E /
// round-15 review Blocker E2E: the second test exercises the
// EdgeZero #[action] handler entry point against the same fixtures
// AND injects the same CannedFanout via the production FanoutSeam
// extension hook (TestFanoutHandle in auctioneer-core::fanout_seam).
// This proves the §5.8 trusted-server-reuse invariant: handler ≡
// direct run_auction → byte-identical response. The earlier
// approach — letting the handler fall back to a stub proxy client
// that always errors — could never produce matching output.
// Round-18 review Blocker 7: helpers used by both halves of the
// handler-path E2E. `build_test_registry` constructs a registry
// containing the MocktioneerBidder at the canned endpoint;
// `host_with_caps` returns a HostConfig with the auction core default caps
// from spec §5.3. The `Arc<dyn Clock>` + `Arc<dyn Rng>` extensions
// installed below are the same shared trait objects the production
// handler reads via `clock_from_extensions` / `rng_from_extensions`.
fn build_test_registry() -> auctioneer_bidder::BidderRegistry {
    let mut r = auctioneer_bidder::BidderRegistry::new();
    let info: &'static auctioneer_config::BidderInfo =
        Box::leak(Box::new(auctioneer_config::BidderInfo {
            name: "mocktioneer".into(),
            endpoint: "http://127.0.0.1:8787/openrtb2/auction".into(),
            uses_https: false,
            ..auctioneer_config::BidderInfo::default()
        }));
    r.register(info, Box::new(auctioneer_bidder::MocktioneerBidder));
    r
}

fn host_with_caps() -> auctioneer_config::HostConfig {
    auctioneer_config::HostConfig {
        resource_limits: auctioneer_config::ResourceLimits {
            default_tmax_ms: 750,
            max_tmax_ms: 1500,
            max_bidder_response_bytes: 262_144,
            max_bidders_per_auction: 50,
            max_outbound_requests: 100,
        },
        ..auctioneer_config::HostConfig::default()
    }
}

#[test]
fn mocktioneer_e2e_through_handler() {
    // Round-17 review Blocker 8 + round-18 review Blocker 7:
    //   - pinned EdgeZero has no `test_support` module; build the
    //     `RequestContext` directly.
    //   - `PathParams` lives at `edgezero_core::params::PathParams`,
    //     NOT `router`.
    //   - `crate::fanout_seam` and `crate::handlers::auction` must
    //     be reachable from the test crate. Task 38's lib.rs makes
    //     both modules `pub` (round-18 Blocker 1 wiring section).
    //   - `ImpExt` is constructed by the test, so import it here.
    //   - Adapter middleware doesn't run in this test, so we
    //     manually install the `Arc<dyn Clock>` / `Arc<dyn Rng>`
    //     extensions the handler expects.
    use edgezero_core::body::Body;
    use edgezero_core::context::RequestContext;
    use edgezero_core::params::PathParams;
    use http::{Method, Request, Uri};
    use auctioneer_core::fanout_seam::TestFanoutHandle;
    use auctioneer_openrtb::ext::imp::ImpExt;
    // Round-19 review Blocker 6: SystemClock + DevRng live in
    // auctioneer_core::host_primitives per Task 36; auctioneer_runtime exports
    // only the Clock / Rng traits + FakeClock + SeededRng.
    use auctioneer_core::host_primitives::{DevRng, SystemClock};
    use auctioneer_runtime::{Clock, Rng};
    use std::sync::Arc;

    let _ = ImpExt::default();  // touch the import so `cargo test` does not warn.

    futures::executor::block_on(async {
        let body = br#"{"id":"r","seatbid":[{"seat":"x","bid":[{"id":"b1","impid":"imp-1","price":1.0,"w":300,"h":250}]}],"cur":"USD"}"#;
        let canned: Arc<dyn auctioneer_exchange::BidderFanout + Send + Sync> =
            Arc::new(CannedFanout { body: body.to_vec() });

        // Direct path (snapshot): call `run_auction` directly with the
        // CannedFanout to produce the byte-for-byte reference.
        let direct = {
            let registry = build_test_registry();
            let host    = host_with_caps();
            let acct    = AccountConfig::default();
            let resolved = ResolvedAccount {
                id: "unknown".into(),
                source: AccountSource::DefaultFallback,
                config: &acct,
            };
            let clock = FakeClock::new(time::OffsetDateTime::UNIX_EPOCH);
            let rng   = SeededRng::new(42);
            let normalized = prepare(mocktioneer_request(), &RequestHeaders::default()).unwrap();
            let ctx = AuctionContext::with_request_start(
                clock.now_monotonic(),
                &host, &resolved, &registry, &*canned, &clock, &rng,
                false,
            );
            run_auction(&normalized, &ctx).await.unwrap()
        };

        // Handler path: install per-request overrides via typed
        // extensions (round-21 review Blocker 1 — the handler now
        // checks `TestClockHandle` / `TestRngHandle` extensions
        // FIRST and falls back to the global registry only if they
        // are absent). No unsafe, no global mutation.
        use auctioneer_core::host_primitives::registry::{TestClockHandle, TestRngHandle};
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock::new());
        let rng:   Arc<dyn Rng   + Send + Sync> = Arc::new(DevRng::new());
        let raw_body = serde_json::to_vec(&mocktioneer_request()).unwrap();
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/openrtb2/auction".parse::<Uri>().unwrap())
            .header("content-type", "application/json")
            .body(Body::from_bytes(raw_body))
            .unwrap();
        request.extensions_mut().insert(TestFanoutHandle(canned.clone()));
        request.extensions_mut().insert(TestClockHandle(clock));
        request.extensions_mut().insert(TestRngHandle(rng));
        let ctx = RequestContext::new(request, PathParams::default());
        let response = auctioneer_core::handlers::auction(ctx).await
            .expect("action returns Ok");
        let (_parts, body) = response.into_parts();
        let handler_body = body.into_bytes_bounded(1 << 20).await.unwrap();

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&handler_body).unwrap(),
            serde_json::to_value(&direct).unwrap(),
            "handler path must produce the same response as direct run_auction (§5.8)",
        );
    });
}
```

- [ ] **Step 2: Run**

```
cargo test -p auctioneer-core --features insecure-bidders --test mocktioneer_e2e
```

Expected: 2 passed (direct + handler paths).

- [ ] **Step 3: Commit**

```
git add crates/auctioneer-core/tests/mocktioneer_e2e.rs
git commit -m "auctioneer-core: Mocktioneer E2E (canned + §5.8 reuse contract)"
```

---

### Task 44: Workspace-wide compile + test + clippy gate

**Files:**

- None modified (verification only).

**Interfaces:**

- Consumes: everything.
- Produces: a green workspace.

- [ ] **Step 1: Whole-workspace build**

```
cargo build --workspace
```

Expected: 0 errors.

- [ ] **Step 2: Whole-workspace test**

```
cargo test --workspace
```

Expected: every test green. Then:

```
cargo test --workspace --features insecure-bidders
```

Expected: all green; the `insecure-bidders`-gated tests now run additionally.

- [ ] **Step 3: Per-target build matrix (per §11.2)**

```
cargo check -p auctioneer-openrtb  --target wasm32-unknown-unknown
cargo check -p auctioneer-openrtb  --target wasm32-wasip1
cargo check -p auctioneer-config   --target wasm32-unknown-unknown
cargo check -p auctioneer-config   --target wasm32-wasip1
cargo check -p auctioneer-runtime  --target wasm32-unknown-unknown
cargo check -p auctioneer-runtime  --target wasm32-wasip1
cargo check -p auctioneer-privacy  --target wasm32-unknown-unknown
cargo check -p auctioneer-privacy  --target wasm32-wasip1
cargo check -p auctioneer-bidder   --target wasm32-unknown-unknown
cargo check -p auctioneer-bidder   --target wasm32-wasip1
cargo check -p auctioneer-exchange --target wasm32-unknown-unknown
cargo check -p auctioneer-exchange --target wasm32-wasip1
cargo check -p auctioneer-adapter-axum
cargo check -p auctioneer-adapter-cloudflare --target wasm32-unknown-unknown
cargo check -p auctioneer-adapter-fastly     --target wasm32-wasip1
cargo check -p auctioneer-adapter-spin       --target wasm32-wasip1
cargo check -p auctioneer-core                       # host triple
cargo check -p auctioneer-core   --target wasm32-unknown-unknown
cargo check -p auctioneer-core   --target wasm32-wasip1
```

Expected: 19 successes (auction core expands the foundations matrix with the three new library crates + adds `auctioneer-core` to it).

- [ ] **Step 4: Per-target release build for production-complete artifacts**

```
cargo build --release -p auctioneer-adapter-axum
cargo build --release -p auctioneer-adapter-cloudflare --target wasm32-unknown-unknown
cargo build --release -p auctioneer-adapter-fastly     --target wasm32-wasip1
cargo build --release -p auctioneer-adapter-spin       --target wasm32-wasip1
```

Expected: 4 successes.

- [ ] **Step 5: Clippy gate**

```
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 6: Final commit**

If any fixes were necessary during Step 5:

```
git add -A
git commit -m "auction core: clippy + final gate"
```

---

### Task 45: Runtime config overlay via `auctioneer.toml` + `[stores.config]`

Round-77 review finding: auction core declared `[stores.config]` and shipped
`AuctioneerAppConfig` but no handler read from the pushed blob — silent
no-op contract violation. This task wires the read path so
`auctioneer config push` actually changes live behavior. See spec §6.1.

**Files:**

- Modify: `crates/auctioneer-core/src/config.rs` — widen
  `AuctioneerAppConfig` to `AuctionConfig` (5 fields) + `TargetingConfig`
  (2 fields) + `StatusConfig` (banner) + `FeatureFlags` (3 kill-
  switches) + `bidders: BTreeMap<String, BidderOverrides>`.
- Create: `crates/auctioneer-core/src/config/resolve.rs` — `resolved_host()`
  / `resolved_account()` / `bidder_enabled()` / `bidder_timeout_ms()`.
- Modify: `crates/auctioneer-core/src/handlers/auction.rs` — call
  `AppConfig::<AuctioneerAppConfig>::from_store(&ctx, "auctioneer_runtime",
None)` before consuming the request; degrade to defaults on
  extractor error; pass merged `HostConfig` / `AccountConfig` into
  `run_auction`.
- Modify: `crates/auctioneer-core/src/handlers/status.rs` — same load
  path; use `resolved_host()` so `[status] banner` overrides
  `HostConfig.status_response`.
- Modify: `auctioneer.toml` — full commented schema with every section,
  every field commented-out to preserve baked defaults.
- Modify: `docs/superpowers/specs/2026-05-21-auctioneer-openrtb-server-design.md`
  §6.1 — document the two-layer merge rule and per-section semantics.
- Create: `crates/auctioneer-exchange/tests/config_overlay.rs` — end-to-
  end assertions that (a) lowering `default_tmax_ms` shrinks the
  emitted `tmaxrequest`, (b) `max_bidders_per_auction = 1` caps the
  seatbid, (c) `targeting.prefix = "myp"` emits `myp_*` keys.

**Interfaces:**

- Consumes: `edgezero_core::extractor::AppConfig::from_store`,
  `auctioneer_config::{host_default, account_default}`.
- Produces: `auctioneer_core::config::{AuctioneerAppConfig, resolve::*}` +
  overlay-aware handlers.

**What deliberately does NOT ship in auction core:**

- `[bidders.<name>] enabled = false` currently logs a warning if the
  baked `BidderInfo.disabled` doesn't already match. Full per-request
  kill-switch enforcement lands in GDPR enforcement alongside the request-scoped
  `RegistryOverlay` seam (dropping a bidder from the auction requires
  mutating the registry per-request, which the current OnceLock
  registry cannot do safely).
- `[bidders.<name>] sample_rate` — value is validated
  (`#[validate(range(min=0.0, max=1.0))]`) and stored; the actual
  sampler wires in GDPR enforcement alongside `apply_sampling(app, registry) ->
Vec<...>`.
- Host-defined bidder aliases (`HostConfig.aliases`) — surfaced in
  `/info/bidders/{name}` (Task 39) but not yet consumed by
  `alias::resolve_aliases`. When the phase-1 YAML ships with `aliases:
{}`, this is a no-op; a future host operator vendoring a non-empty
  `host-defaults.yaml` before GDPR enforcement wires the auction-side resolution
  would see aliased auctions rejected as unknown bidders. GDPR enforcement closes
  this seam.

**Live-behavior surfaces (round-78 review High #1):**

Every `[features]` and `[bidders.<name>]` knob has a real consumer
so `auctioneer config push` actually changes behavior:

- `features.gzip_outbound` → `build_call_list` strips
  `BidderCall.endpoint_compression` when the toggle is `false`. Baked
  YAML `endpoint_compression: gzip` no longer forces gzip.
- `features.returnallbidstatus_default` → replaces the hardcoded
  `false` fallback in `returnallbidstatus()`. Request-level
  `ext.prebid.returnallbidstatus` still wins when present.
- `bidders.<name>.timeout_ms` → `BidderCall.timeout` resolution chain
  becomes `alias override → base-bidder override → baked → bidder_visible_tmax_ms`.
  A request-declared alias inherits the base bidder's `timeout_ms`
  when the alias has no override of its own (round-79 review Medium).
  Consolidated into the pure helper `resolve_bidder_timeout()` so the
  chain is unit-testable.

**Explicitly NOT a toggle in auction core (round-79 review High #1):**

`strict_currency_usd_only` was proposed as a `[features]` knob in
round-78 but the auction core response builder hardcodes `cur = "USD"` for any
non-empty seatbid — flipping the toggle to `false` would let non-USD
bids into a USD-labelled response (a wire-shape contract bug worse
than the no-op it replaced). The toggle is removed; non-USD is
always dropped with a code-10004 warning in auction core. GDPR enforcement fx wires
currency-aware bundles and can reintroduce the toggle with a
meaningful `false` behavior.

**Hard-ceiling clamp (round-78 review High #2):**

`resolve_tmax` in the `None | Some(0)` branch now returns
`effective_ms = default_ms.min(max_ms)`. Prior behavior let a
misconfigured `default_tmax_ms > max_tmax_ms` deploy silently run
past the supposed hard cap when the request omitted `tmax`. The
adapter-visible `tmax` is intentionally NOT clamped (spec §5.3 —
that value advertises the bidder's slice, not the exchange budget).

**Runtime path always degrades, push-time is fail-fast
(round-79 review High #2):**

The round-78 draft treated a present-but-failing extractor as
a 500. That path fires on a fresh Axum dev checkout because
`edgezero.toml` declares `[stores.config]` (so the store binding
is always present) but no local blob has been pushed
(`AppConfigError::ConfigOutOfDate`). `curl -X POST
/openrtb2/auction` on a fresh clone would 500 — exactly the
contract the handler comment claims to preserve.

`load_app_config` in both handlers now degrades to
`AuctioneerAppConfig::default()` on any error, logging at WARN with the
extractor's error message. Push-time is the fail-fast surface —
`auctioneer-cli config push` runs `run_typed_preflight` before writing,
and `--dry-run` produces the same diff without writing. Operators
see the WARN log at any regression to trace the mis-fired push.

**Three-source bidder discovery (round-80 review High):**

`split::process_one_imp` scans three slots per imp in this order —
`imp.ext.bidder.<name>` (exchange-hoisted), `imp.ext.prebid.bidder.<name>`
(typed), and `imp.ext.<name>` (legacy top-level per spec §3.1 `Imp.ext`
row). Competitor stripping (`ext_obj.retain()`) drops the other two
bidders' keys before serialization, and bidder-scoped FPD is looked
up by object key against the same three-source discovery so a
request with `context.data.myx.foo` reaches only `myx`.

**Post-FPD validation (round-81 review High):**

`split_by_bidder` runs `bid_request.validate()` on the per-bidder
`BidRequest` after FPD merge because the merge can invalidate a
request that was fine on ingress (e.g. `context.data.myx = null`
removing a required field via RFC 7396). Validation failure surfaces
as `AuctionError::InvalidImpFirstPartyData` (code 15) with the
`ValidationError` message chain, and `bidder_specific_fpd_that_removes_only_media_type_returns_code_15`
regression-tests the guarantee.

**Host-varying URL helper (round-82 review Medium):**

`crates/auctioneer-bidder/src/adapters/url.rs::build_outbound_url_with_host`
lands as a `pub(super)` sibling to `build_outbound_url` so any bidder
whose `endpoint` template contains `{{.Host}}` and whose baked YAML
declares a `host_allowlist` inherits the SSRF allowlist check from
the shared helper instead of open-coding
`EndpointBuilder::select_host` per adapter (which is how the guard
would drift). Four unit tests cover the allowlisted / rejected /
fixed-host / missing-selection matrix. No shipped auction core bidder is host-
varying so the helper is `#[cfg_attr(not(test), expect(dead_code,
reason = "shared infra for the first host-varying bidder"))]` — the
allowance flips off automatically the moment the first adapter calls
it.

**`participating_bidders` matches three-source discovery (round-82 review Low):**

`crates/auctioneer-exchange/src/registry_order.rs::participating_bidders`
now unions the same three sources `split::process_one_imp` scans plus
request-declared aliases from `ext.prebid.aliases`. A legacy top-
level `imp.ext.<bidderName>` that resolves against the registry OR
against a request alias counts; unregistered top-level passthrough
keys (`imp.ext.tid`, `imp.ext.prebid`) are excluded. Two new tests —
`legacy_top_level_slot_is_discovered` and
`legacy_top_level_slot_accepts_request_alias` — regression-test the
alignment. No production code path depends on the helper (`run_auction`
derives its bidder set out of `split`); the fix is API/doc drift
closure so external callers cannot observe two different
participation sets from the same request.

**`request_start` capture order (round-82 review Low):**

`crates/auctioneer-core/src/handlers/auction.rs::auction` captures
`request_start` immediately after `clock_handle(&ctx)` (extractor
plumbing) and BEFORE `rng_handle(&ctx)`. Rng lookup, fanout
resolution, and config load all happen after the timestamp so their
cost counts against tmax. A short comment on the action documents
the ordering so a later edit that re-inserts work in front of
`request_start` reads as an anti-pattern.

**UTF-8 char-boundary truncation (round-83 review High):**

`crates/auctioneer-exchange/src/targeting/keys.rs::truncate` walks down
to the nearest UTF-8 char boundary before calling
`String::truncate`. Prior behavior panicked when `truncate_target_attribute`
landed inside a multibyte character — bidder / alias names are
arbitrary UTF-8 (`BidderName` has no ASCII invariant; §5.9 sorts
UTF-8 names), so a non-ASCII alias could take the whole handler
down. Regression test
`multibyte_bidder_name_does_not_panic_at_truncate_boundary` covers
the exact off-boundary case.

**Cap-drop warnings on tmax short-circuit (round-83 review Medium):**

`short_circuit_tmax_expired` now accepts `bidder_cap_drops` and
emits the code-10999 `max_bidders_per_auction (N) exceeded` warning
into each dropped bidder's `ext.warnings[<bidder>]` slot, matching
the normal path. Shared helper `bidder_cap_drop_warning(cap)` is the
single source of truth so the two emission sites can no longer
drift. Prior behavior swallowed the cap signal whenever tmax
happened to expire before fanout started.

**`participating_bidders` alias tightening (round-83 review Medium):**

`participating_bidders` no longer inserts every `ext.prebid.aliases`
key unconditionally. An alias is counted only when it also appears
in one of the three imp-level slots (matching
`split::process_one_imp` — an alias declared but not targeted by any
imp never reaches fanout, so the public helper must not report it).
`is_participating_name(key)` is the shared predicate used by all
three slot scans. New regression test
`declared_alias_not_targeted_by_any_imp_is_excluded` locks in the
tighter contract.

**Unclosed `{{` endpoint macros are build-time errors (round-83 review Low):**

`build_support::endpoint_unclosed_macro_prefix` scans an endpoint
template for a `{{` with no matching `}}` and returns the offending
prefix (up to 32 bytes of context) if one exists.
`render_endpoint_macros` calls it before `extract_endpoint_macros`
so a bidder shipping `?acct={{.AccountID` (no closing braces) fails
codegen with a clear "unclosed template macro starting at …"
message. Prior behavior returned whatever macros were parsed so far
and let the malformed template bake into the wasm binary. Coverage:
`child_build_fails_with_unclosed_endpoint_macro` in the build-
failure harness, and `unclosed_endpoint_macro_detected` as a direct
unit test on the helper. The syncer template path already had the
equivalent gate via `MacroScan::Unclosed`.

**Authority-strict `{{.Host}}` validation (round-84 review Medium):**

`{{.Host}}` is defined as the endpoint's authority-substitution
token, never a path or query fill. Prior codegen used a substring
scan (`endpoint.contains("{{.Host}}")`) so a template like
`https://fixed.example/rtb/{{.Host}}` plus a declared `hostAllowlist`
would misclassify a fixed-host bidder as host-varying (the runtime
substring-based substitution would splice a host into the path).
The build-time gate now uses `classify_endpoint_authority(endpoint)
== HostVarying` in `validate_host_allowlist`, and
`render_endpoint_macros` rejects any endpoint where
`endpoint_has_host_macro_outside_authority` fires. Coverage:
`host_macro_outside_authority_is_flagged` (unit test on the helper)
and `child_build_fails_with_host_macro_in_path` (build-failure
harness). The runtime `EndpointBuilder::build` still uses a
substring replace, but the build-time gate guarantees it only sees
templates whose `{{.Host}}` (if any) is in the authority slot.

**`enabled=false` doc drift (round-84 review Low):**

Overlay comments on `BidderOverrides.enabled` and
`resolve::bidder_enabled` overstated auction core behavior — they claimed
`Some(false)` drops the bidder from every auction, but the runtime
actually just emits a WARN log at handler load (per the auction core non-goal
in this task). Comments now describe the observe-only semantic and
point at the GDPR enforcement request-scoped registry overlay for the actual
kill-switch behavior.

**Real multi-bidder cap-drop integration test (round-84 review Low):**

`overlay_capping_max_bidders_drops_extras` previously registered a
single bidder and admitted in its own comment that nothing was
dropped. Rewritten to register two bidders (`generic` + `pubmatic`),
target both from a single imp, push `max_bidders_per_auction = 1`,
and assert that (a) only `generic` (lexicographically first) survives
in `resp.seatbid`, and (b) `pubmatic` carries a code-10999
`max_bidders_per_auction` warning under
`ext.warnings.pubmatic`. This exercises the overlay → cap → drop-
warning path end-to-end.

- [ ] **Step 1: Widen `AuctioneerAppConfig`** — see the struct diff above.
      Every new field `Option<T>` with a `#[validate(...)]` matching the
      underlying spec-defined range.

- [ ] **Step 2: Add `config/resolve.rs`** with the four helpers. All
      return owned values so the exchange never learns about the config
      store.

- [ ] **Step 3: Wire auction + status handlers.** Load the app config
      BEFORE consuming `ctx.into_request()` (the request move
      invalidates the extractor call), then pass the merged types into
      `AuctionContext::with_request_start`.

- [ ] **Step 4: Rewrite `auctioneer.toml`.** Commented-only defaults so
      the fresh checkout produces identical behavior to today.

- [ ] **Step 5: Update spec §6.1** with the runtime-overlay contract.

- [ ] **Step 6: Tests.** `crates/auctioneer-exchange/tests/config_overlay.rs`
      runs the direct-path driver with pre-resolved `HostConfig`/
      `AccountConfig` (simulating the overlay) and asserts the effective
      values reach `run_auction`. Six unit tests cover the schema (parse
  - validate) and four unit tests cover `resolve::*`.

- [ ] **Step 7: Verify + commit**

```
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --features insecure-bidders
```

Expected: 0 warnings; 381 default / 383 insecure passing (round-78
wiring adds `ExchangeFeatures` + `bidder_timeout_override` threading,
tmax clamp, `BidderOverrides` validation, and 6 end-to-end overlay
tests; round-79 removes `strict_currency_usd_only` and adds the
alias-timeout unit test; round-80 adds three-source discovery +
competitor stripping + object-based FPD lookup tests; round-81 adds
`bidder_specific_fpd_that_removes_only_media_type_returns_code_15`;
round-82 adds four host-varying URL helper tests and two
`participating_bidders` legacy-slot tests; round-83 adds the
UTF-8 char-boundary truncate test, the alias-untargeted-by-any-imp
regression, the unclosed-endpoint-macro build-failure harness test,
and the `endpoint_unclosed_macro_prefix` unit test; round-84 adds
the `host_macro_outside_authority_is_flagged` unit test, the
`child_build_fails_with_host_macro_in_path` harness case, and
rewrites the multi-bidder cap-drop overlay test).

```
git add crates/auctioneer-core crates/auctioneer-exchange auctioneer.toml \
        docs/superpowers
git commit -m "auction core Task 45: runtime config overlay via auctioneer.toml"
```

---

## Self-Review

**Spec coverage** — every auction core deliverable per §14:

- `auctioneer-bidder`:
  - `Bidder` trait — Task 11 ✓
  - 4 reference adapters (generic / appnexus / pubmatic / mocktioneer-feature-gated) — Tasks 15–18 ✓
  - `EndpointBuilder` (macro percent-encoding, path-traversal rejection, host_allowlist) — Task 13 ✓
  - Canonical `insecure-bidders` feature — Task 9 ✓
  - `BidderRegistry` — Task 14 ✓
- `auctioneer-exchange`:
  - `BidderFanout` trait + neutral types — Task 20 ✓
  - `validate_endpoint` (full §5.4 ruleset incl. insecure-bidders relaxation) — Task 22 ✓
  - tmax effective-budget resolution (§5.3 four-value taxonomy) — Task 23 ✓
  - `participating_bidders` (§5.9 lexicographic) — Task 24 ✓
  - Alias resolution + alias-validation errors (§5.1 step 7) — Task 25 ✓
  - Per-bidder request split + bidderparams merge + FPD merge + disabled-bidder removal — Task 26 ✓
  - Resource-limit enforcement (`max_bidders_per_auction`, `max_outbound_requests`; body caps via the foundational `body_caps::match_route` already shipped; `max_bidder_response_bytes` enforced in fanout/Task 37; `bidder_timeout` via `BidderCall.timeout`) — Tasks 23, 27, 32, 37 ✓
  - Price granularity + bucket (Appendix E.1, E.2) — Task 28 ✓
  - Targeting keys (Appendix E.3) — Task 29 ✓
  - Winner selection (Appendix E.4) — Task 30 ✓
  - Deterministic response builder (Appendix E.5, E.6, E.7) + responsetime aggregation — Task 31 ✓
  - `run_auction` (§5.8 — steps 6-11 + remaining==0 short-circuit + code 12 / 1 / 101 surfaces) — Task 32 ✓
- `auctioneer-privacy` (API surface only):
  - `PrivacyContext::allow_all_no_scrub()` + invariants — Tasks 6, 8 ✓
  - Sync API surface + permissive stub (`resolve_sync_privacy` always Ok; `sync_allowed → Allowed`, `setuid_allowed → Allowed`; `host_cookie_sync_allowed == true`; permissive `activity_allow`) — Tasks 7, 8 ✓
- `auctioneer-config` extensions:
  - `insecure-bidders` feature forwarded into `build.rs` — verified via Task 9's feature chain + the foundations `auctioneer-config` build script (foundations ships this; auction core forwards via the chain).
  - `ResourceLimits` extended with `max_tmax_ms`, `max_bidder_response_bytes`, `max_bidders_per_auction`, `max_outbound_requests` — Task 23 ✓
- `auctioneer-core`:
  - `[[triggers.http]]` for the four auction core routes — Task 35 ✓
  - `SystemClock` + `DevRng` — Task 36 ✓
  - Dev `BidderFanout` (interim, §5.2 documented gaps) — Task 37 ✓
  - `auction` handler — Task 38 ✓
  - `status`, `info_bidders`, `info_bidders_detail` handlers — Task 39 ✓
  - Dependency-purity test — Task 41 ✓
  - Mocktioneer E2E — Task 43 ✓
- Adapters:
  - `insecure-bidders` forwarding — Task 40 ✓

**auction core NOT-in-scope confirmations** (explicit non-deliverables per the source prompt):

- TCF v2 / GDPR enforcement (GDPR enforcement) — `auctioneer-privacy` ships only the API surface; the permissive stub is the auction core contract.
- CCPA / GPP / activity model / scrubbing (privacy enforcement) — Same — stub only.
- `/cookie_sync` / `/setuid` / `/getuids` / `uids` cookie module / host-cookie import / candidate selection (user sync) — Verified absent; `edgezero.toml` registers only the four auction-shaped routes.
- Debug surface + redaction (diagnostic qualification) — Task 31's `build_response` asserts `ext.debug` is never emitted; Task 32's `run_auction` ignores `ctx.debug_requested`.
- **Round-21 review Spec 8: `targeting.mediatypepricegranularity` IS HONORED IN auction core** by reading it out of the foundational `ExtRequestTargeting.other` (the catch-all `serde_json::Map`) per spec §5.6 line 5213. The earlier round-19/20 "auction core deviation" note is rescinded; auction core supports per-media-type granularity overrides today without waiting for GDPR enforcement's typed-ext widening.

**Round-13 review High 6 — auction core dev-vs-behavior split, explicit:**

- **auction core development complete** (this plan's target — covered by Tasks 1–44):
  - Every API surface, type, and trait pinned by §4.1 / §5 / §6 wired.
  - `validate_endpoint` + `EndpointBuilder` SSRF gates fully enforced (round-12 + round-13 hardening).
  - Body caps (foundations `body_caps::match_route`), `max_bidder_response_bytes`, `max_bidders_per_auction`, `max_outbound_requests`, tmax effective-budget resolution + monotonic deadline anchoring + `remaining == 0` short-circuit all enforced.
  - Targeting / price-granularity / winner-selection per Appendix E.1–E.4 (round-12 request/account-driven).
  - Sec-GPC outbound header added by exchange (round-13 High 3).
  - Mocktioneer E2E covers BOTH the handler path and the direct run_auction path (round-12 High 10).
  - 4 reference adapters + dependency-purity test green.
- **auction core behavior-complete (NOT in auction core; moves to GDPR enforcement fanout task):**
  - `AuctionDeadline` mid-fanout cancellation — Task 37's dev fanout does not cancel in-flight calls when the deadline passes.
  - Per-bidder `BidderCall.timeout` enforcement — Round-12 Medium 11 materialized the per-bidder budget into `BidderCall`, but the dev fanout applies only the overall deadline. GDPR enforcement's production fanout reads the field and enforces it.
  - Streaming `max_response_bytes` (post-buffer is auction core — round-69 user sync known gap).

The acceptance criterion for "auction core done" is the dev-complete set above. The two enforcement gaps move with the production fanout task in GDPR enforcement's plan; auction core's verification suite asserts the materialization (struct fields populated, helpers exposed) but does not assert mid-fanout cancellation.

**Placeholder scan** — no "TODO", "TBD", or "similar to Task N" patterns. Every code step contains the actual code; the few stubs in Task 9 / Task 19 are explicitly named (a single line `// stub — implemented in next task`) and replaced in their successor task.

**Type consistency**:

- `BidderName`, `BidderHttpRequest`, `BidderHttpResponse`, `BidderResponse`, `TypedBid`, `BidderError`, `Bidder` consistent across Tasks 10–18 + Tasks 22, 24, 25, 26, 30, 32, 33, 34.
- `AuctionError`, `AuctionContext`, `ResolvedAccount`, `AccountSource`, `BidderFanout`, `BidderCall`, `BidderCallResult`, `FanoutError`, `FanoutInternalError`, `AuctionDeadline`, `TmaxSource`, `Compression` consistent across Tasks 20–34 + 37–38.
- `PrivacyContext`, `AuctionPermissions`, `Activity`, `Component`, `SyncPrivacyInput`, `SyncDecision`, `SyncerLookup`, `SyncEndpoint`, `GdprSignalIn`, `SyncPrivacyContext` consistent across Tasks 3–8.
- `BidderInfo`, `HostConfig`, `AccountConfig`, `ResourceLimits`, `Compression` consistent across `auctioneer-config` + every consumer.

**Known places where the implementer must look closely:**

- Task 32 (`run_auction`) inlines a helper `TmaxRes` to avoid cross-module visibility friction; the implementer should consider promoting it into `tmax.rs` once the integration test passes.
- Task 38 (`auction` handler) is a real `#[action]` function wired through `edgezero_core::app!` per round-14 review Blocker action wiring. `request_start` is captured as the first executable statement (round-14 review Blocker request_start) so preprocessing time counts against tmax. The supporting `HandlerError` envelope + `registry_handle` / `host_config_handle` seams are defined alongside the action.
- Task 38's `FanoutSeam` (round-15 review Blocker E2E) routes the production fanout through `ctx.proxy_handle()` (the EdgeZero `ProxyClient` exposed on every action's `RequestContext`) and lets tests inject a canned fanout via a `TestFanoutHandle` extension on the same context. A request that arrives without a usable `proxy_handle` AND without a test handle surfaces `FanoutUnavailable` — there is no `StubProxyClient` that silently always errors.

**Granularity check** — 44 tasks (a touch over the spec's recommended 25-35 range, but each task is bite-sized with ~5 TDD steps averaging 2-5 minutes each; we deliberately split the response builder, run_auction skeleton, and integration tests across separate commits so each commit is independently reviewable).

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-06-24-auction-core.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task using `superpowers:subagent-driven-development`. Each task is self-contained (Files + Interfaces + code) so a subagent with only this plan + the spec link can execute end-to-end. Review between tasks; fast iteration. Best when reviewer wants ongoing check-ins and each task's diff is small enough to read inline.

**2. Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`. Batch execution with hard checkpoints at:

- after Task 8 (`auctioneer-privacy` complete),
- after Task 18 (`auctioneer-bidder` adapters complete),
- after Task 30 (`auctioneer-exchange` core helpers complete),
- after Task 34 (`run_auction` + determinism green),
- after Task 39 (handlers green),
- after Task 44 (workspace gate green).

**Recommended:** Subagent-Driven for the four `auctioneer-bidder` adapters (Tasks 15–18) and the `auctioneer-exchange` helpers (Tasks 22–30) since each is independent and well-suited to parallel dispatch.

**Which approach?**
